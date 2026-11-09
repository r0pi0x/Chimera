//! The logic of beams, glows and sparks (specs/source/visual_entities.md):
//! point_spotlight's LightOn/LightOff and outputs (2.5), env_laser's and
//! persistent env_beam's on/off, damage every 0.1 s to the first player
//! on the line and env_beam's touch output (3, 4.1), env_lightglow's
//! inputs, env_spark's timing and OnSpark (8), strike-generator
//! env_beams' strikes (4.2: when, between which points, the damage). The
//! map draws them (`map::beams`, `map::emitters`); what is on reaches it
//! as `map::EntityPart`, a strike as `Effect::BeamStrike`.
//!
//! Covers: point_spotlight LightOn/LightOff, env_laser/env_beam on/off, damage
//! every 0.1 s to the first player on the line and OnTouchedByEntity,
//! env_spark's timing, sound and OnSpark (its sparks as
//! `map::SparkBurst`, which the game draws; the damage as a map's,
//! armour taking its part: `Effect::Hurt`), strike-generator env_beams
//! (4.2: first strike 1 s after spawn or on TurnOn, then every life +
//! StrikeTime or a random part of it, StrikeOnce, random ends among a name's
//! matches, walls within the radius without an end, the damage once per strike
//! to the first player on the line; `Effect::BeamStrike`); drawn by
//! `map::beams` through `EntityPart`.

use bevy::prelude::*;

use super::classes::Class;
use super::value::Value;
use super::visuals::PartKind;
use super::world::{Effect, EntId, LogicEntity, LogicWorld, Who};

/// env_spark "Start ON".
pub const SF_SPARK_START_ON: u32 = 64;
/// Continuous beam damage needs this much time since the last hit.
pub const DAMAGE_PERIOD: f32 = 0.1;

/// A laser's or beam's damage and touch state.
#[derive(Clone, Debug, Default)]
pub struct Zap {
    /// Damage per second.
    pub damage: f32,
    /// Tick of the last damage (or of turning on).
    pub last: i64,
    /// env_beam TouchType (0: none).
    pub touch: i32,
    /// Stopped thinking after OnTouchedByEntity (until TurnOn).
    pub stopped: bool,
    pub laser: bool,
    /// Where the laser aimed last (entity space).
    pub end: Option<Vec3>,
}

/// A strike-generator env_beam (life ≠ 0, or a ring; 4.2).
#[derive(Clone, Debug, Default)]
pub struct Strike {
    /// Each strike's life and the wait after it, seconds.
    pub life: f32,
    pub strike_time: f32,
    /// "Random Strike" (4): the wait is U(0, StrikeTime).
    pub random: bool,
    /// Per strike, to the first thing on the line (with LightningEnd).
    pub damage: f32,
    pub ring: bool,
}

/// env_beam spawnflags.
pub const SF_BEAM_RANDOM: u32 = 4;
pub const SF_BEAM_RING: u32 = 8;

/// Tries a strike without an end makes for a surface (4.2).
const STRIKE_TRIES: usize = 10;

/// A strike generator's settings, if it is one.
pub fn strike(e: &LogicEntity) -> Option<Box<Strike>> {
    if class(e) != "env_beam" {
        return None;
    }
    let life = e.kv_f("life");
    let ring = e.has_flag(SF_BEAM_RING);
    (life != 0.0 || ring).then(|| {
        Box::new(Strike {
            life,
            strike_time: e.kv_f("StrikeTime"),
            random: e.has_flag(SF_BEAM_RANDOM),
            damage: e.kv_f("damage"),
            ring,
        })
    })
}

fn class(e: &LogicEntity) -> String {
    e.classname.to_ascii_lowercase()
}

/// Whether a beam entity starts on.
pub fn starts_on(e: &LogicEntity) -> bool {
    match class(e).as_str() {
        "point_spotlight" => e.has_flag(1),
        _ => e.targetname.is_empty() || e.has_flag(1),
    }
}

/// A laser's or persistent beam's damage state, when it can hurt or be
/// touched.
pub fn zap(e: &LogicEntity) -> Option<Box<Zap>> {
    let c = class(e);
    let laser = c == "env_laser";
    if !laser && (c != "env_beam" || e.kv_f("life") != 0.0 || e.has_flag(SF_BEAM_RING)) {
        return None;
    }
    let damage = e.kv_f("damage");
    let touch = if laser { 0 } else { e.kv_i("TouchType") };
    (damage > 0.0 || touch != 0).then(|| {
        Box::new(Zap {
            damage,
            touch,
            laser,
            ..default()
        })
    })
}

fn part(w: &mut LogicWorld, id: EntId) -> Option<&mut super::visuals::Part> {
    match w.get_mut(id).map(|e| &mut e.class) {
        Some(Class::Part(p)) => Some(p),
        _ => None,
    }
}

/// Turn on or off; outputs and thinking as the class does.
fn set_on(w: &mut LogicWorld, id: EntId, on: bool) {
    let tick = w.tick;
    let Some(p) = part(w, id) else { return };
    if p.on == on {
        return;
    }
    p.on = on;
    if let Some(z) = p.zap.as_mut()
        && on
    {
        z.last = tick;
        z.stopped = false;
    }
    let kind = p.kind;
    // A strike generator turned on strikes on the next tick (4.2).
    let thinks = p.zap.is_some() || p.strike.is_some();
    let spotlight = w.get(id).is_some_and(|e| class(e) == "point_spotlight");
    if spotlight {
        let out = if on { "OnLightOn" } else { "OnLightOff" };
        w.fire_output(id, out, Some(Who::Ent(id)), Value::Void);
    }
    match (kind, on) {
        (PartKind::Beam, true) if thinks => w.think_in(id, w.dt),
        (PartKind::Spark | PartKind::Tesla, true) => w.think_in(id, w.dt),
        (_, false) => {
            if let Some(e) = w.get_mut(id) {
                e.next_think = None;
            }
        }
        _ => {}
    }
}

pub(super) fn input(w: &mut LogicWorld, id: EntId, input: &str) -> bool {
    let Some(p) = w.get(id).and_then(|e| match &e.class {
        Class::Part(p) => Some((p.kind, p.on)),
        _ => None,
    }) else {
        return false;
    };
    let spotlight = w.get(id).is_some_and(|e| class(e) == "point_spotlight");
    match (p.0, input) {
        (PartKind::Beam, "lighton") if spotlight => set_on(w, id, true),
        (PartKind::Beam, "lightoff") if spotlight => set_on(w, id, false),
        (PartKind::Beam, "turnon") if !spotlight => set_on(w, id, true),
        (PartKind::Beam, "turnoff") if !spotlight => set_on(w, id, false),
        (PartKind::Beam, "toggle") if !spotlight => set_on(w, id, !p.1),
        // The beam's look (width, noise, colour, scroll): kept as placed.
        // One strike now, on or off, either kind (4.2).
        (PartKind::Beam, "strikeonce") if !spotlight => strike_once(w, id),
        (
            PartKind::Beam,
            "width" | "noise" | "colorredvalue" | "colorgreenvalue" | "colorbluevalue" | "scrollspeed",
        ) => {}
        (PartKind::Glow, "color") => {}
        (PartKind::Spark, "startspark") => set_on(w, id, true),
        (PartKind::Spark, "stopspark") => set_on(w, id, false),
        (PartKind::Spark, "togglespark") => set_on(w, id, !p.1),
        (PartKind::Spark, "sparkonce") => {
            spark(w, id);
            set_on(w, id, false);
        }
        (PartKind::Tesla, "turnon") => set_on(w, id, true),
        (PartKind::Tesla, "turnoff") => set_on(w, id, false),
        (PartKind::Tesla, "dospark") => tesla(w, id),
        _ => return false,
    }
    true
}

/// At activation: a lit spotlight fires OnLightOn; lasers and beams that
/// are on start thinking; a spark that starts on sparks after 0.1 +
/// U(0, 1.5) s.
pub(super) fn activate(w: &mut LogicWorld, id: EntId) {
    let Some((kind, on, zap, strike)) = w.get(id).and_then(|e| match &e.class {
        Class::Part(p) => Some((p.kind, p.on, p.zap.is_some(), p.strike.is_some())),
        _ => None,
    }) else {
        return;
    };
    let spotlight = w.get(id).is_some_and(|e| class(e) == "point_spotlight");
    match kind {
        PartKind::Beam if spotlight && on => w.fire_output(id, "OnLightOn", Some(Who::Ent(id)), Value::Void),
        // The first strike 1 s after spawn (4.2).
        PartKind::Beam if strike && on => w.think_in(id, 1.0),
        PartKind::Beam if zap && on => w.think_in(id, w.dt),
        PartKind::Spark if on => {
            let delay = 0.1 + w.random() * 1.5;
            w.think_in(id, delay);
        }
        _ => {}
    }
}

/// env_spark "Silent" and "Directional".
pub const SF_SPARK_SILENT: u32 = 256;
pub const SF_SPARK_DIRECTIONAL: u32 = 512;

/// One spark (8): the effect at the entity (along its forward with
/// "Directional"), its sound unless silent, OnSpark.
fn spark(w: &mut LogicWorld, id: EntId) {
    if let Some(e) = w.get(id) {
        let dir = if e.has_flag(SF_SPARK_DIRECTIONAL) {
            super::triggers::forward(e.angles)
        } else {
            Vec3::ZERO
        };
        let (at, magnitude, silent) = (e.origin, e.kv_f("Magnitude").clamp(0.0, 15.0), e.has_flag(SF_SPARK_SILENT));
        w.effects.push(Effect::Spark { at, dir, magnitude });
        if !silent {
            w.effects.push(Effect::Sound {
                entry: "DoSpark".into(),
                at,
                volume: None,
                pitch: None,
            });
        }
    }
    w.fire_output(id, "OnSpark", Some(Who::Ent(id)), Value::Void);
}

pub(super) fn think(w: &mut LogicWorld, id: EntId) {
    let Some((kind, on)) = w.get(id).and_then(|e| match &e.class {
        Class::Part(p) => Some((p.kind, p.on)),
        _ => None,
    }) else {
        return;
    };
    if !on {
        return;
    }
    match kind {
        PartKind::Spark => {
            spark(w, id);
            let max = w.get(id).map_or(0.0, |e| e.kv_f("MaxDelay").max(0.0));
            let delay = 0.1 + w.random() * max;
            w.think_in(id, delay);
        }
        PartKind::Tesla => {
            tesla(w, id);
            let (a, b) = w.get(id).map_or((0.5, 2.0), |e| {
                (e.kv_f("interval_min").max(0.0), e.kv_f("interval_max").max(0.0))
            });
            let delay = (a + (b.max(a) - a) * w.random()).max(w.dt);
            w.think_in(id, delay);
        }
        PartKind::Beam if part(w, id).is_some_and(|p| p.strike.is_some()) => {
            let Some(s) = part(w, id).and_then(|p| p.strike.as_deref()).cloned() else { return };
            strike_now(w, id, &s);
            let wait = if s.random { w.random() * s.strike_time } else { s.strike_time };
            w.think_in(id, (s.life + wait).max(w.dt));
        }
        PartKind::Beam => {
            zap_tick(w, id);
            if part(w, id).and_then(|p| p.zap.as_ref()).is_some_and(|z| !z.stopped) {
                w.think_in(id, w.dt);
            }
        }
        _ => {}
    }
}

/// A point_tesla's spark (public entity docs): its arcs (the map draws
/// them, `Effect::Tesla`) and its sound ("m_SoundName", none for "none").
fn tesla(w: &mut LogicWorld, id: EntId) {
    let Some(e) = w.get(id) else { return };
    let (at, index) = (e.origin, e.map_index);
    let sound = e
        .kv("m_SoundName")
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("none"));
    let sound = sound.map(str::to_string);
    if let Some(entity) = index {
        w.effects.push(Effect::Tesla { entity });
    }
    if let Some(entry) = sound {
        w.effects.push(Effect::Sound {
            entry,
            at,
            volume: None,
            pitch: None,
        });
    }
}

/// Where a laser or beam runs this tick (entity space).
fn line(w: &mut LogicWorld, id: EntId) -> Option<(Vec3, Vec3)> {
    let e = w.get(id)?;
    let laser = class(e) == "env_laser";
    let origin_of = |w: &LogicWorld, name: &str| w.find(name).and_then(|t| w.get(t)).map(|t| t.origin);
    if laser {
        let start = e.origin;
        let name = e.kv("LaserTarget").unwrap_or("").to_string();
        // A random one of the targets, every tick (3).
        let targets: Vec<Vec3> = w
            .resolve(&name, None, None)
            .into_iter()
            .filter_map(|t| match t {
                Who::Ent(t) => w.get(t).map(|t| t.origin),
                Who::Player(p) => w.player(p).map(|p| p.origin),
            })
            .collect();
        let end = if targets.is_empty() {
            part(w, id).and_then(|p| p.zap.as_ref()).and_then(|z| z.end)?
        } else {
            targets[w.random_int(targets.len())]
        };
        if let Some(z) = part(w, id).and_then(|p| p.zap.as_mut()) {
            z.end = Some(end);
        }
        Some((start, end))
    } else {
        let (a, b) = (
            e.kv("LightningStart").unwrap_or("").to_string(),
            e.kv("LightningEnd").unwrap_or("").to_string(),
        );
        Some((origin_of(w, &a)?, origin_of(w, &b)?))
    }
}

/// StrikeOnce (4.2): one strike between the ends, whatever the beam's
/// kind or state (a persistent beam's has its life 0: it stays).
fn strike_once(w: &mut LogicWorld, id: EntId) {
    let Some(e) = w.get(id) else { return };
    if e.kv("LightningEnd").is_none_or(|n| n.trim().is_empty()) {
        return;
    }
    let fallback = Strike {
        life: e.kv_f("life"),
        damage: e.kv_f("damage"),
        ring: e.has_flag(SF_BEAM_RING),
        ..default()
    };
    let s = part(w, id).and_then(|p| p.strike.as_deref()).cloned().unwrap_or(fallback);
    strike_now(w, id, &s);
}

/// A random one of the entities `name` matches: its origin, and whether
/// it is a "static point" (4.1: no parent and no model, or an
/// info_target, info_landmark or path_corner).
fn random_end(w: &mut LogicWorld, name: &str) -> Option<(Vec3, bool)> {
    let ends: Vec<(Vec3, bool)> = w
        .resolve(name, None, None)
        .into_iter()
        .filter_map(|t| match t {
            Who::Ent(t) => w.get(t).map(|t| {
                let point_class = matches!(class(t).as_str(), "info_target" | "info_landmark" | "path_corner");
                let bare = t.kv("parentname").is_none_or(|p| p.trim().is_empty())
                    && t.hulls.is_empty()
                    && t.kv("model").is_none();
                (t.origin, point_class || bare)
            }),
            Who::Player(p) => w.player(p).map(|p| (p.origin, false)),
        })
        .collect();
    if ends.is_empty() {
        return None;
    }
    Some(ends[w.random_int(ends.len())])
}

/// Where a line from `from` along a random direction meets the static
/// world within `radius`, and how far that is (None: nothing in reach).
fn random_hit(w: &mut LogicWorld, from: Vec3, radius: f32) -> Option<(Vec3, f32)> {
    let dir = (Vec3::new(w.random(), w.random(), w.random()) * 2.0 - Vec3::ONE).normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }
    let to = from + dir * radius;
    let f = w
        .collision
        .as_ref()
        .map_or(1.0, |c| c.sweep(Vec3::ZERO, Vec3::ZERO, from, to));
    (f < 1.0).then(|| (from.lerp(to, f), f * radius))
}

/// One strike (4.2): the line it runs (between the ends; from the start
/// to a wall within the radius; between two walls around the beam); the
/// map draws it, and with both ends the first player on it takes the
/// damage once.
fn strike_now(w: &mut LogicWorld, id: EntId, s: &Strike) {
    let Some(e) = w.get(id) else { return };
    let (origin, index) = (e.origin, e.map_index);
    let name = |k: &str| e.kv(k).map(str::trim).filter(|n| !n.is_empty()).map(str::to_string);
    let (start, end) = (name("LightningStart"), name("LightningEnd"));
    let radius = e.kv_f("Radius");
    let line = match (start, end) {
        (Some(a), Some(b)) => {
            let (Some((a, a_static)), Some((b, b_static))) = (random_end(w, &a), random_end(w, &b)) else {
                return;
            };
            // A ring needs two entities (with a static point: nothing).
            if s.ring && (a_static || b_static) {
                return;
            }
            if s.damage > 0.0
                && let Some(p) = first_player(w, a, b)
            {
                // As a laser's (energy beam damage: armour takes its part).
                w.effects.push(Effect::Hurt {
                    target: p,
                    amount: s.damage,
                    armour: true,
                    kind: crate::core::DamageKind::Generic,
                });
            }
            Some((a, b))
        }
        (Some(a), None) => {
            let Some((from, _)) = random_end(w, &a) else { return };
            (0..STRIKE_TRIES).find_map(|_| {
                random_hit(w, from, radius)
                    .filter(|(_, d)| *d >= radius / 10.0)
                    .map(|(hit, _)| (from, hit))
            })
        }
        (None, None) => (0..STRIKE_TRIES).find_map(|_| {
            let (a, _) = random_hit(w, origin, radius)?;
            let (b, _) = random_hit(w, origin, radius)?;
            let (da, db) = ((a - origin).normalize_or_zero(), (b - origin).normalize_or_zero());
            let sees = w
                .collision
                .as_ref()
                .is_none_or(|c| c.sweep(Vec3::ZERO, Vec3::ZERO, a, b) >= 1.0);
            (da.dot(db) <= 0.0 && a.distance(b) >= radius / 10.0 && sees).then_some((a, b))
        }),
        (None, Some(_)) => None,
    };
    if let (Some((start, end)), Some(entity)) = (line, index) {
        w.effects.push(Effect::BeamStrike {
            entity,
            start,
            end,
            life: s.life,
        });
    }
}

/// Where a segment enters a box, as a fraction (None: it misses).
fn segment_box(a: Vec3, b: Vec3, lo: Vec3, hi: Vec3) -> Option<f32> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if a[i] < lo[i] || a[i] > hi[i] {
                return None;
            }
            continue;
        }
        let (u, v) = ((lo[i] - a[i]) / d[i], (hi[i] - a[i]) / d[i]);
        t0 = t0.max(u.min(v));
        t1 = t1.min(u.max(v));
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

/// The first player on the line before the world stops it.
fn first_player(w: &LogicWorld, a: Vec3, b: Vec3) -> Option<Entity> {
    let wall = w
        .collision
        .as_ref()
        .map_or(1.0, |c| c.sweep(Vec3::ZERO, Vec3::ZERO, a, b));
    w.players
        .iter()
        .filter(|p| p.alive)
        .filter_map(|p| segment_box(a, b, p.origin + p.mins, p.origin + p.maxs).map(|t| (t, p.entity)))
        .filter(|(t, _)| *t <= wall)
        .min_by(|x, y| x.0.total_cmp(&y.0))
        .map(|(_, e)| e)
}

/// A laser's or beam's tick (3 step 4, 4.1): damage × elapsed to the
/// first player on the line every 0.1 s; a touch-type beam fires
/// OnTouchedByEntity when a player crosses it, and stops thinking.
fn zap_tick(w: &mut LogicWorld, id: EntId) {
    let Some(z) = part(w, id).and_then(|p| p.zap.as_deref()).cloned() else { return };
    let Some((a, b)) = line(w, id) else { return };
    if (b - a).length() < 0.1 {
        return;
    }
    let tick = w.tick;
    let elapsed = (tick - z.last) as f32 * w.dt;
    if z.damage > 0.0 && elapsed >= DAMAGE_PERIOD - 1e-4 {
        if let Some(p) = first_player(w, a, b) {
            // Armour takes its part, health whole points (measured
            // 2026-10-18, visual_entities.md Q3: 10.5 a hit took 10, or 5
            // and 3 armour from 100).
            w.effects.push(Effect::Hurt {
                target: p,
                amount: z.damage * elapsed,
                armour: true,
                kind: crate::core::DamageKind::Generic,
            });
        }
        if let Some(z) = part(w, id).and_then(|p| p.zap.as_mut()) {
            z.last = tick;
        }
    }
    // TouchType 1/3/4: players (no NPCs here).
    if matches!(z.touch, 1 | 3 | 4)
        && let Some(p) = first_player(w, a, b)
    {
        let filter = w.get(id).and_then(|e| e.kv("filtername")).unwrap_or("").to_string();
        let passes = filter.is_empty()
            || w.find(&filter)
                .is_none_or(|f| super::classes::filter_passes(w, f, Some(Who::Player(p))));
        if passes {
            w.fire_output(id, "OnTouchedByEntity", Some(Who::Player(p)), Value::Void);
            if let Some(z) = part(w, id).and_then(|p| p.zap.as_mut()) {
                z.stopped = true;
            }
        }
    }
}
