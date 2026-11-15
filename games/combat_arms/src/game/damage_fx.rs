//! Damage screen effects (specs/combat_arms/effects_sounds.md, Follow-up
//! (2026-11-14) 1 to 6): what lasting damage (burn, poison, ...) does to
//! the burning or poisoned player's own screen, and the server's
//! bookkeeping that starts it.
//!
//! The data (`CaDamageFxTable`): the install's `attributes/damagefx.txt`,
//! records `[DamageFX0]`, `[DamageFX1]`, ... until one is missing (item
//! 2's keys; `AllowMovement` and `AllowInput` read 0, blocking, when a
//! record leaves them out; `MinFXPercent` clamped to 0..1; colours /255;
//! `RotationRandomRange` (0.5, 1.0) by default), read at start-up. No
//! install: no records, so no screen effect at all (nothing of ours).
//!
//! The server (`progress_pass`, item 1): every 0.5 s of simulation time
//! it looks at each character's progressive entries (the grenades'
//! over-time entries, `grenades::CaGrenadeEffects`, and the volumes'
//! lasting damage, `volumes::VolumeDamage`): a damage type is *active*
//! when one of its entries still has time left, *dealing* when that
//! entry deals damage. Then the gear exception: when the worn armour's
//! protection against the type of the last entry looked at (its pieces'
//! torso entries for that class, summed) is 1 or more, that type is
//! dropped from the active set. The sets travel as the replicated
//! `CaProgress` on the character (reliable, on change; a late joiner gets
//! them with the character). A death clears the character's entries and
//! sets at once (and a respawn starts with none).
//!
//! The client (`CaDamageFxView`, the local player's; items 3 and 4): on
//! each change of the local player's active set the table is walked in
//! order; a record whose type turned active starts (not when it already
//! runs or the player is dead): its start sound once and its loop sound
//! (local), the tint cycle from the start of its ramp-up, darkening and
//! wobbles from 0, the HUD picture rising. A record whose type is no
//! longer active stops with a fade over `FadeTime` (the loop sound ends
//! at once). Each tick (`step`, the fixed tick's time) every running
//! record runs its tint cycle (up, on, down, off, carrying the time past
//! a phase's end), darkening (`L += speed dt`, light scale `max(K, 1 -
//! L)` per channel), roll wobble (towards a drawn target, `RotationMax`
//! x U(range), turning at each target, starting downwards) and FOV
//! wobble (bouncing between +-`FOVMax`), the wobbles scaled by the
//! strength `m = p + mu (1 - p)` (`MinFXPercent` p, the movement
//! fraction mu); a fading one fades its tint and darkening linearly and
//! moves its wobbles back to 0. Combined (`CaScreenFx`): the tints by
//! per-channel maximum (with the other tint sources, also by maximum,
//! clamped to 1: `combine_tint`), the light scales by per-channel
//! minimum (white within 0.01: none), the rolls and FOV offsets summed.
//! Any running or fading record with `AllowMovement` 0 stops the local
//! player's movement (and jumps); `AllowInput` 0 all game input
//! (`lock_intent`). `NumJumpsToEscape` n > 0: each fresh jump press
//! counts, the n-th stops the record (with its fade) and asks the server
//! to clear the player's entries of that type (`CaEscape`; a network
//! client sends `CaEscapeRequest`). The local player's death, and
//! `EnableDamageFX` below 1 (`CaDamageFxView::enabled`), stop every
//! record at once, no fade. The HUD picture (`HudIcon`) is shown only by
//! HUD layouts with a `DamageFX` control (the Fireteam and Snowball
//! HUDs: `CaDamageFxView::hud_layout`, off in every mode we have); its
//! alpha rises and falls by 255 / `FadeTime` per second. The spread rule
//! (item 4, "poison or stun forces mu S to 1") reads `CaProgress::dealing`
//! (`forces_spread`).
//!
//! What the screen does with `CaScreenFx` (the tint as an additive light
//! and the light scale as a multiplier over the finished frame, the roll
//! and FOV on the first-person camera) is the front end's
//! (`ui::damage_fx`).
//!
//! Known shortcuts: the per-frame steps run on the fixed tick; the
//! client's other players' effect groups (`3rdPersonFXName` and the rest)
//! and the records' `FXName` groups and sprinkles aren't played; the
//! single-player keys (`AttachCameraToAnimation`, ...) are read but do
//! nothing; infected poison (counted as poison, kept by an escape) has no
//! entries of its own here; the zone of the victim's last hit isn't kept,
//! the gear exception reads the torso; an external camera doesn't remove
//! the tint.

use std::collections::BTreeMap;

use bevy::prelude::*;
use lithtech_formats::attributes::Document;
use serde::{Deserialize, Serialize};

use super::{
    effects::sounds::CaSound,
    grenades::{BLEED, CaGrenadeEffects, OverTimeKind},
    inventory::UNIT,
    volumes::{CaVolume, VolumeDamage, damage_type_number},
    weapons::{BURN, POISON, class_of},
};
use crate::{
    core::{Dead, Died, Health, Intent, LocalPlayer, SimClock, authoritative},
    weapon::{Armor, ArmorPieces},
};

/// The damage-effect attribute file in the attributes archive.
pub const FILE: &str = "attributes/damagefx.txt";
/// The records' prefix.
pub const RECORDS: &str = "DamageFX";
/// The server's progressive pass, s.
pub const PASS: f64 = 0.5;
/// The stun damage type (the spread rule's other type).
pub const STUN: i32 = 12;
/// A light scale this close to white on every channel is none.
pub const WHITE_EPSILON: f32 = 0.01;

// ---------------------------------------------------------------------------
// The data

/// One `[DamageFX#]` record (item 2).
#[derive(Clone, Debug, PartialEq)]
pub struct DamageFxRecord {
    pub name: String,
    pub damage_type: i32,
    pub hud_icon: String,
    pub start_sound: String,
    pub loop_sound: String,
    /// 0..1.
    pub tint: Vec3,
    /// Ramp-up, on, ramp-down, off, s.
    pub tint_times: [f32; 4],
    pub fade: f32,
    /// Radians, radians per second, and the swing's random fraction range.
    pub roll_max: f32,
    pub roll_speed: f32,
    pub roll_range: Vec2,
    pub fov_max: f32,
    pub fov_speed: Vec2,
    /// 0..1.
    pub min_fx: f32,
    /// 0..1.
    pub light_color: Vec3,
    pub light_speed: f32,
    pub allow_movement: bool,
    pub allow_input: bool,
    pub jumps_to_escape: u32,
    pub instant: bool,
}

impl Default for DamageFxRecord {
    fn default() -> Self {
        Self {
            name: String::new(),
            damage_type: 0,
            hud_icon: String::new(),
            start_sound: String::new(),
            loop_sound: String::new(),
            tint: Vec3::ZERO,
            tint_times: [0.0; 4],
            fade: 0.0,
            roll_max: 0.0,
            roll_speed: 0.0,
            roll_range: Vec2::new(0.5, 1.0),
            fov_max: 0.0,
            fov_speed: Vec2::ZERO,
            min_fx: 0.0,
            light_color: Vec3::ZERO,
            light_speed: 0.0,
            allow_movement: true,
            allow_input: true,
            jumps_to_escape: 0,
            instant: false,
        }
    }
}

/// The records, in table order (the index is the record's id).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaDamageFxTable(pub Vec<DamageFxRecord>);

impl CaDamageFxTable {
    /// Read the records (until one is missing).
    pub fn read(doc: &Document) -> Self {
        let text = |b: Option<&[u8]>| b.map(|b| b.iter().map(|&c| c as char).collect::<String>()).unwrap_or_default();
        let colour = |v: Option<[f64; 3]>| v.map_or(Vec3::ZERO, |v| Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32) / 255.0);
        Self(
            doc.records(RECORDS)
                .into_iter()
                .map(|s| {
                    let n = |k: &str| s.number(k, 0.0) as f32;
                    DamageFxRecord {
                        name: text(s.string("Name")),
                        damage_type: s.int("DamageType", 0),
                        hud_icon: text(s.string("HudIcon")),
                        start_sound: text(s.string("StartSound")),
                        loop_sound: text(s.string("LoopSound")),
                        tint: colour(s.vector("TintColor")),
                        tint_times: [n("TintRampUpTime"), n("TintOnTime"), n("TintRampDownTime"), n("TintOffTime")],
                        fade: n("FadeTime"),
                        roll_max: n("RotationMax"),
                        roll_speed: n("RotationSpeed"),
                        roll_range: s
                            .vector("RotationRandomRange")
                            .map_or(Vec2::new(0.5, 1.0), |v| Vec2::new(v[0] as f32, v[1] as f32)),
                        fov_max: n("FOVMax"),
                        fov_speed: Vec2::new(n("FOVXSpeed"), n("FOVYSpeed")),
                        min_fx: n("MinFXPercent").clamp(0.0, 1.0),
                        light_color: colour(s.vector("LightScaleColor")),
                        light_speed: n("LightScaleSpeed"),
                        // No default: a record without them blocks both.
                        allow_movement: s.int("AllowMovement", 0) != 0,
                        allow_input: s.int("AllowInput", 0) != 0,
                        jumps_to_escape: s.int("NumJumpsToEscape", 0).max(0) as u32,
                        instant: s.int("InstantEffect", 0) != 0,
                    }
                })
                .collect(),
        )
    }

    /// Parse a document's text.
    pub fn parse(text: &str) -> Result<Self, String> {
        Document::parse(text.as_bytes()).map(|d| Self::read(&d)).map_err(|e| e.to_string())
    }

    /// The install's records, or none.
    pub fn load(data: &crate::game::data::CaDataInner) -> Self {
        if !data.builtin_only()
            && let Some(mount) = data.mount()
            && let Ok(bytes) = mount.read(FILE)
        {
            match Document::parse(&bytes) {
                Ok(d) => return Self::read(&d),
                Err(e) => warn!("combat_arms damage effects: {FILE}: {e}; no screen effects"),
            }
        }
        Self::default()
    }

    /// Whether a record reacts to damage type `t`.
    pub fn has_type(&self, t: i32) -> bool {
        self.0.iter().any(|r| r.damage_type == t)
    }
}

// ---------------------------------------------------------------------------
// The server's progressive pass (item 1)

/// A damage type's bit (types 0..63; others have none).
pub fn bit(t: i32) -> u64 {
    if (0..64).contains(&t) { 1u64 << t } else { 0 }
}

/// A character's progressive damage types (the server's, replicated):
/// active (an entry with time left) and dealing (its damage non-zero) as
/// bits by damage type. On every character from its spawn
/// (`on_every_character`).
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaProgress {
    pub active: u64,
    pub dealing: u64,
}

impl CaProgress {
    pub fn is_active(&self, t: i32) -> bool {
        self.active & bit(t) != 0
    }
}

/// When the next pass is due.
#[derive(Resource, Default)]
pub struct ProgressPass {
    pub next: Option<f64>,
}

/// A grenade entry's damage type.
pub fn over_time_type(k: OverTimeKind) -> i32 {
    match k {
        OverTimeKind::Burn => BURN,
        OverTimeKind::Poison => POISON,
        OverTimeKind::Bleed => BLEED,
    }
}

/// The worn gear's protection against damage type `t` (the torso
/// entries of its class, summed over the pieces).
pub fn protection(armor: Option<&Armor>, pieces: Option<&ArmorPieces>, t: i32) -> f32 {
    let Some(armor) = armor else { return 0.0 };
    let class = class_of(t);
    let (entries, _) = crate::weapon::coverage(armor, pieces);
    entries.filter(|c| c.class == Some(class)).map(|c| c.torso).sum()
}

/// The pass (every `PASS` s of simulation time, the first at the first
/// tick): each character's sets from its entries, the gear exception,
/// written when they change.
#[allow(clippy::type_complexity)]
pub fn progress_pass(
    clock: Res<SimClock>,
    mut pass: ResMut<ProgressPass>,
    grenades: Option<Res<CaGrenadeEffects>>,
    volumes_state: Option<Res<VolumeDamage>>,
    volumes: Query<&CaVolume>,
    characters: Query<(Entity, Option<&CaProgress>, Option<&Armor>, Option<&ArmorPieces>, Has<Dead>, Option<&Health>), With<Intent>>,
    mut commands: Commands,
) {
    let now = clock.now;
    let due = *pass.next.get_or_insert(now);
    if now + 1e-9 < due {
        return;
    }
    let mut next = due;
    while next <= now + 1e-9 {
        next += PASS;
    }
    pass.next = Some(next);
    // Every entry in force: (victim, type, deals).
    let mut entries: BTreeMap<Entity, Vec<(i32, bool)>> = BTreeMap::new();
    if let Some(g) = grenades.as_deref() {
        for o in &g.over_time {
            if o.remaining > 1e-4 {
                entries.entry(o.victim).or_default().push((over_time_type(o.kind), o.rate != 0.0));
            }
        }
    }
    if let Some(v) = volumes_state.as_deref() {
        for (p, ve, _) in v.progressive(now) {
            if let Some(t) = volumes.get(ve).ok().and_then(|v| damage_type_number(&v.damage_type)) {
                entries.entry(p).or_default().push((t, true));
            }
        }
    }
    for (e, had, armor, pieces, dead, health) in &characters {
        let alive = !dead && health.is_none_or(|h| h.current > 0.0);
        let mut sets = CaProgress::default();
        if alive && let Some(list) = entries.get(&e) {
            for &(t, deals) in list {
                sets.active |= bit(t);
                if deals {
                    sets.dealing |= bit(t);
                }
            }
            // The gear exception: the last entry's type only.
            if let Some(&(last, _)) = list.last()
                && protection(armor, pieces, last) >= 1.0
            {
                sets.active &= !bit(last);
            }
        }
        if had.copied().unwrap_or_default() != sets {
            commands.entity(e).insert(sets);
        }
    }
}

/// A death clears the victim's progressive entries and its sets.
pub fn clear_on_death(
    mut died: MessageReader<Died>,
    mut grenades: Option<ResMut<CaGrenadeEffects>>,
    mut volumes: Option<ResMut<VolumeDamage>>,
    mut commands: Commands,
) {
    for d in died.read() {
        if let Some(g) = grenades.as_deref_mut() {
            g.over_time.retain(|o| o.victim != d.entity);
        }
        if let Some(v) = volumes.as_deref_mut() {
            v.clear(d.entity, |_| true);
        }
        if let Ok(mut e) = commands.get_entity(d.entity) {
            e.insert(CaProgress::default());
        }
    }
}

/// A player escaped damage type `damage_type` by jumping (item 4): the
/// server clears that player's entries whose own type is that type.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct CaEscape {
    pub player: Entity,
    pub damage_type: i32,
}

/// A network client's escape, to the server.
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct CaEscapeRequest {
    pub damage_type: i32,
}

/// The server: clear the escaped entries.
pub fn apply_escapes(
    mut escapes: MessageReader<CaEscape>,
    mut grenades: Option<ResMut<CaGrenadeEffects>>,
    mut volumes: Option<ResMut<VolumeDamage>>,
    volume_q: Query<&CaVolume>,
) {
    for e in escapes.read() {
        if let Some(g) = grenades.as_deref_mut() {
            g.over_time.retain(|o| o.victim != e.player || over_time_type(o.kind) != e.damage_type);
        }
        if let Some(v) = volumes.as_deref_mut() {
            v.clear(e.player, |ve| {
                volume_q.get(ve).ok().and_then(|v| damage_type_number(&v.damage_type)) == Some(e.damage_type)
            });
        }
    }
}

/// The spread rule (item 4, weapon_firing.md Spread): a poison or stun
/// record running while its type is dealing forces mu S to 1. The
/// server reads the sets as the client's record would run (a record of
/// that type exists and the type is active and dealing).
pub fn forces_spread(table: &CaDamageFxTable, p: &CaProgress) -> bool {
    [POISON, STUN]
        .into_iter()
        .any(|t| table.has_type(t) && p.active & p.dealing & bit(t) != 0)
}

// ---------------------------------------------------------------------------
// The client's records (items 3 and 4)

/// One running (or fading) record's state.
#[derive(Clone, Debug, PartialEq)]
pub struct Running {
    pub record: usize,
    /// Tint phase (0 up, 1 on, 2 down, 3 off) and the time in it.
    pub phase: usize,
    pub phase_t: f32,
    pub tint: Vec3,
    /// Darkening L (0..1).
    pub light: f32,
    pub roll: f32,
    roll_target: f32,
    /// -1 moving down, +1 up; 0 not started.
    roll_dir: f32,
    pub fov: Vec2,
    fov_dir: Vec2,
    /// 0..255.
    pub hud_alpha: f32,
    /// Time since the stop, while fading.
    pub fading: Option<f32>,
    stop_tint: Vec3,
    stop_light: f32,
    pub jumps: u32,
}

impl Running {
    fn new(record: usize) -> Self {
        Self {
            record,
            phase: 0,
            phase_t: 0.0,
            tint: Vec3::ZERO,
            light: 0.0,
            roll: 0.0,
            roll_target: 0.0,
            roll_dir: 0.0,
            fov: Vec2::ZERO,
            fov_dir: Vec2::ONE,
            hud_alpha: 0.0,
            fading: None,
            stop_tint: Vec3::ZERO,
            stop_light: 0.0,
            jumps: 0,
        }
    }
}

/// The tint cycle's colour after `t` s in phase `phase` of `r`.
fn phase_tint(r: &DamageFxRecord, phase: usize, t: f32) -> Vec3 {
    let d = r.tint_times[phase];
    match phase {
        0 if d > 0.0 => r.tint * (t / d).min(1.0),
        0 => r.tint,
        1 => r.tint,
        2 if d > 0.0 => r.tint * (1.0 - t / d).max(0.0),
        _ => Vec3::ZERO,
    }
}

/// A sound the records ask for.
#[derive(Clone, Debug, PartialEq)]
pub enum FxSound {
    Once(String),
    Loop { record: usize, file: String },
    StopLoop { record: usize },
}

/// The local player's damage effects.
#[derive(Resource, Clone, Debug)]
pub struct CaDamageFxView {
    pub running: Vec<Running>,
    /// The active set last seen.
    pub seen: u64,
    /// `EnableDamageFX` >= 1.
    pub enabled: bool,
    /// The HUD layout has a `DamageFX` control (Fireteam, Snowball).
    pub hud_layout: bool,
    /// The wobbles' random draws.
    seed: u32,
    /// The jump key was down last tick.
    jump_held: bool,
    pub sounds: Vec<FxSound>,
    /// Types escaped by jumping, for the server.
    pub escaped: Vec<i32>,
}

impl Default for CaDamageFxView {
    fn default() -> Self {
        Self {
            running: Vec::new(),
            seen: 0,
            enabled: true,
            hud_layout: false,
            seed: 0,
            jump_held: false,
            sounds: Vec::new(),
            escaped: Vec::new(),
        }
    }
}

impl CaDamageFxView {
    fn draw(&mut self, range: Vec2) -> f32 {
        self.seed = self.seed.wrapping_add(1);
        let u = crate::weapon::random::shared_random(self.seed, "combat_arms:damage_fx", 0).next();
        range.x + (range.y - range.x) * u
    }

    /// The record of `index`, running and not fading.
    pub fn is_running(&self, index: usize) -> bool {
        self.running.iter().any(|r| r.record == index && r.fading.is_none())
    }

    /// A change of the active set (item 3): start the records whose type
    /// turned active, fade those whose type isn't.
    pub fn sets_changed(&mut self, table: &CaDamageFxTable, active: u64, alive: bool) {
        self.seen = active;
        if !self.enabled {
            return;
        }
        for (i, rec) in table.0.iter().enumerate() {
            let on = active & bit(rec.damage_type) != 0;
            if on && alive && !self.is_running(i) {
                self.start(table, i);
            } else if !on && self.is_running(i) {
                self.stop(table, i);
            }
        }
    }

    /// Start record `i`.
    pub fn start(&mut self, table: &CaDamageFxTable, i: usize) {
        let rec = &table.0[i];
        self.running.retain(|r| r.record != i);
        self.running.push(Running::new(i));
        if !rec.start_sound.is_empty() {
            self.sounds.push(FxSound::Once(rec.start_sound.clone()));
        }
        if !rec.loop_sound.is_empty() {
            self.sounds.push(FxSound::Loop {
                record: i,
                file: rec.loop_sound.clone(),
            });
        }
    }

    /// Stop record `i` with its fade.
    pub fn stop(&mut self, table: &CaDamageFxTable, i: usize) {
        let Some(r) = self.running.iter_mut().find(|r| r.record == i && r.fading.is_none()) else {
            return;
        };
        r.fading = Some(0.0);
        r.stop_tint = r.tint;
        r.stop_light = r.light;
        if !table.0[i].loop_sound.is_empty() {
            self.sounds.push(FxSound::StopLoop { record: i });
        }
    }

    /// Stop every record at once, no fade (death, `EnableDamageFX` 0).
    pub fn clear(&mut self, table: &CaDamageFxTable) {
        for r in std::mem::take(&mut self.running) {
            if table.0.get(r.record).is_some_and(|rec| !rec.loop_sound.is_empty()) && r.fading.is_none() {
                self.sounds.push(FxSound::StopLoop { record: r.record });
            }
        }
    }

    /// A tick of `dt` s; `jump` the jump key (held).
    pub fn step(&mut self, table: &CaDamageFxTable, dt: f32, jump: bool) {
        let fresh = jump && !self.jump_held;
        self.jump_held = jump;
        let mut escaped = Vec::new();
        let mut draws = Vec::new();
        for r in &mut self.running {
            let rec = &table.0[r.record];
            match r.fading.as_mut() {
                None => {
                    // The tint cycle, the time past a phase carried on.
                    if rec.tint_times.iter().any(|d| *d > 0.0) {
                        r.phase_t += dt;
                        let mut guard = 0;
                        while r.phase_t >= rec.tint_times[r.phase] && guard < 64 {
                            r.phase_t -= rec.tint_times[r.phase];
                            r.phase = (r.phase + 1) % 4;
                            guard += 1;
                        }
                        r.tint = phase_tint(rec, r.phase, r.phase_t);
                    } else {
                        r.tint = rec.tint;
                    }
                    r.light = (r.light + rec.light_speed * dt).min(1.0);
                    // Roll: towards the target, turning at it.
                    if rec.roll_speed > 0.0 && rec.roll_max > 0.0 {
                        if r.roll_dir == 0.0 {
                            r.roll_dir = -1.0;
                            draws.push((r.record, -1.0));
                        } else {
                            r.roll += r.roll_dir * rec.roll_speed * dt;
                            if (r.roll_dir < 0.0 && r.roll <= r.roll_target) || (r.roll_dir > 0.0 && r.roll >= r.roll_target) {
                                r.roll = r.roll_target;
                                r.roll_dir = -r.roll_dir;
                                draws.push((r.record, r.roll_dir));
                            }
                        }
                    }
                    // FOV: bouncing between the limits.
                    if rec.fov_max > 0.0 {
                        for k in 0..2 {
                            r.fov[k] += r.fov_dir[k] * rec.fov_speed[k] * dt;
                            if r.fov[k].abs() >= rec.fov_max {
                                r.fov[k] = rec.fov_max * r.fov[k].signum();
                                r.fov_dir[k] = -r.fov_dir[k];
                            }
                        }
                    }
                    r.hud_alpha = if rec.fade > 0.0 { (r.hud_alpha + 255.0 / rec.fade * dt).min(255.0) } else { 255.0 };
                    // Escape by jumping.
                    if rec.jumps_to_escape > 0 && fresh {
                        r.jumps += 1;
                        if r.jumps >= rec.jumps_to_escape {
                            escaped.push(r.record);
                        }
                    }
                }
                Some(t) => {
                    *t += dt;
                    let k = if rec.fade > 0.0 { (1.0 - *t / rec.fade).max(0.0) } else { 0.0 };
                    r.tint = r.stop_tint * k;
                    r.light = r.stop_light * k;
                    let back = |v: f32, speed: f32| {
                        if v.abs() <= speed * dt { 0.0 } else { v - v.signum() * speed * dt }
                    };
                    r.roll = back(r.roll, rec.roll_speed);
                    r.fov = Vec2::new(back(r.fov.x, rec.fov_speed.x), back(r.fov.y, rec.fov_speed.y));
                    r.hud_alpha = if rec.fade > 0.0 { (r.hud_alpha - 255.0 / rec.fade * dt).max(0.0) } else { 0.0 };
                }
            }
        }
        // New roll targets.
        for (record, dir) in draws {
            let range = table.0[record].roll_range;
            let max = table.0[record].roll_max;
            let u = self.draw(range);
            if let Some(r) = self.running.iter_mut().find(|r| r.record == record) {
                r.roll_target = dir * max * u;
            }
        }
        for i in escaped {
            self.stop(table, i);
            self.escaped.push(table.0[i].damage_type);
        }
        // A fade over: gone.
        self.running.retain(|r| {
            let fade = table.0[r.record].fade;
            r.fading.is_none_or(|t| t + 1e-5 < fade)
        });
    }

    /// The combined screen effect at movement fraction `mu`.
    pub fn screen(&self, table: &CaDamageFxTable, mu: f32) -> CaScreenFx {
        let mut out = CaScreenFx {
            light_scale: Vec3::ONE,
            ..default()
        };
        for r in &self.running {
            let rec = &table.0[r.record];
            let m = strength(rec.min_fx, mu);
            out.tint = out.tint.max(r.tint);
            let scale = rec.light_color.max(Vec3::splat(1.0 - r.light));
            out.light_scale = out.light_scale.min(scale);
            out.roll += r.roll * m;
            out.fov += r.fov * m;
            out.move_locked |= !rec.allow_movement;
            out.input_locked |= !rec.allow_input;
            if self.hud_layout && !rec.hud_icon.is_empty() && r.hud_alpha > 0.0 {
                out.hud = Some((rec.hud_icon.clone(), r.hud_alpha));
            }
        }
        if (out.light_scale - Vec3::ONE).abs().max_element() <= WHITE_EPSILON {
            out.light_scale = Vec3::ONE;
        }
        out
    }
}

/// The wobbles' strength factor `m = p + mu (1 - p)` (item 4), p the
/// record's `MinFXPercent` (0..1), mu the movement fraction.
pub fn strength(min_fx: f32, mu: f32) -> f32 {
    let p = min_fx.clamp(0.0, 1.0);
    p + mu.clamp(0.0, 1.0) * (1.0 - p)
}

/// What the damage effects do to the local player's screen this tick.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct CaScreenFx {
    /// The damage slot of the screen tint (an additive light, 0..1).
    pub tint: Vec3,
    /// The scene's light scale (multiplied; white: none).
    pub light_scale: Vec3,
    /// Camera roll, radians.
    pub roll: f32,
    /// FOV offsets (x, y), degrees.
    pub fov: Vec2,
    pub move_locked: bool,
    pub input_locked: bool,
    /// The HUD picture and its alpha (0..255).
    pub hud: Option<(String, f32)>,
}

impl Default for CaScreenFx {
    fn default() -> Self {
        Self {
            tint: Vec3::ZERO,
            light_scale: Vec3::ONE,
            roll: 0.0,
            fov: Vec2::ZERO,
            move_locked: false,
            input_locked: false,
            hud: None,
        }
    }
}

/// The screen tint from the damage slot and the other sources (flash):
/// per channel the maximum, at most 1.
pub fn combine_tint(slots: &[Vec3]) -> Vec3 {
    slots.iter().fold(Vec3::ZERO, |a, b| a.max(*b)).min(Vec3::ONE)
}

/// The key the records' loop sounds play under.
pub fn loop_key(record: usize) -> crate::map::SoundKey {
    crate::map::SoundKey(0xDA4A_6E00_0000_0000 | record as u64)
}

/// The local player's records, each tick: the set changes, the step, the
/// death clear, the screen, the sounds and escapes.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn local_effects(
    table: Res<CaDamageFxTable>,
    mut view: ResMut<CaDamageFxView>,
    mut screen: ResMut<CaScreenFx>,
    me: Query<
        (Entity, Option<&CaProgress>, &Intent, Option<&Health>, Has<Dead>, Option<&crate::core::Velocity>, Option<&crate::game::movement::CaMovement>),
        With<LocalPlayer>,
    >,
    clock: Res<SimClock>,
    mut sounds: MessageWriter<CaSound>,
    mut loops: MessageWriter<crate::game::view_model::attached::CaLoopSound>,
    mut escapes: MessageWriter<CaEscape>,
    mut requests: MessageWriter<CaEscapeRequest>,
    role: Option<Res<crate::core::NetRole>>,
) {
    let Ok((e, progress, intent, health, dead, state, movement)) = me.single() else {
        if !view.running.is_empty() {
            view.clear(&table);
        }
        *screen = CaScreenFx::default();
        return;
    };
    let alive = !dead && health.is_none_or(|h| h.current > 0.0);
    if !alive || !view.enabled {
        if !view.running.is_empty() {
            view.clear(&table);
        }
        view.seen = 0;
    } else {
        let active = progress.map_or(0, |p| p.active);
        if active != view.seen {
            view.sets_changed(&table, active, alive);
        }
    }
    let mu = match (state, movement) {
        (Some(s), Some(m)) => crate::game::weapons::movement_fraction(s.0.length() / UNIT, m.speed_limit),
        (Some(s), None) => crate::game::weapons::movement_fraction(s.0.length() / UNIT, crate::game::weapons::RUN_SPEED),
        _ => 0.0,
    };
    view.step(&table, clock.dt(), intent.jump);
    *screen = view.screen(&table, mu);
    for s in std::mem::take(&mut view.sounds) {
        match s {
            FxSound::Once(f) => {
                sounds.write(CaSound::local(f));
            }
            FxSound::Loop { record, file } => {
                loops.write(crate::game::view_model::attached::CaLoopSound {
                    key: loop_key(record),
                    sound: Some(CaSound::local(file)),
                    follow: None,
                    once: false,
                });
            }
            FxSound::StopLoop { record } => {
                loops.write(crate::game::view_model::attached::CaLoopSound {
                    key: loop_key(record),
                    sound: None,
                    follow: None,
                    once: false,
                });
            }
        }
    }
    let client = role.is_some_and(|r| *r == crate::core::NetRole::Client);
    for t in std::mem::take(&mut view.escaped) {
        if client {
            requests.write(CaEscapeRequest { damage_type: t });
        } else {
            escapes.write(CaEscape { player: e, damage_type: t });
        }
    }
}

/// The locks on the local player's intent (item 4): no movement (or
/// jumps) while a record blocks it; no game input at all while one
/// blocks input (the look stays).
pub fn lock_intent(screen: Res<CaScreenFx>, mut me: Query<&mut Intent, With<LocalPlayer>>) {
    if !screen.move_locked && !screen.input_locked {
        return;
    }
    for mut i in &mut me {
        if screen.input_locked {
            *i = Intent {
                yaw: i.yaw,
                pitch: i.pitch,
                command: i.command,
                ..default()
            };
        } else {
            i.move_axis = Vec2::ZERO;
            i.jump = false;
        }
    }
}

/// The network client's escapes, on the server.
fn receive_escapes(
    mut requests: MessageReader<bevy_replicon::prelude::FromClient<CaEscapeRequest>>,
    players: Query<&crate::net::server::Player>,
    mut escapes: MessageWriter<CaEscape>,
) {
    for r in requests.read() {
        let Some(client) = r.client_id.entity() else { continue };
        let Ok(p) = players.get(client) else { continue };
        escapes.write(CaEscape {
            player: p.character,
            damage_type: r.message.damage_type,
        });
    }
}

pub struct CaDamageFxPlugin;

impl Plugin for CaDamageFxPlugin {
    fn build(&self, app: &mut App) {
        use crate::net::NetAppExt;
        use chimera_core::character::CharacterPartsExt;
        let table = CaDamageFxTable::load(&crate::game::data::CaData::of(app));
        // On every character from its spawn (empty sets), so a first hurt
        // or death doesn't move it to a new archetype (a match after
        // match would keep making them: ultimate_gun_game `session`).
        app.on_every_character::<CaProgress>();
        app.insert_resource(table)
            .init_resource::<ProgressPass>()
            .init_resource::<CaDamageFxView>()
            .init_resource::<CaScreenFx>()
            .add_message::<CaEscape>()
            .add_message::<CaEscapeRequest>()
            .add_message::<Died>()
            .add_message::<CaSound>()
            .add_message::<crate::game::view_model::attached::CaLoopSound>()
            .add_systems(FixedPreUpdate, lock_intent)
            // These read the tick's entries, deaths and the local player and
            // write only their own state (the entries they clear, the sets,
            // the local records and screen): either order against the
            // tick's other systems is right.
            // At the tick's end (its damage and deaths applied).
            .add_systems(
                FixedLast,
                (
                    (clear_on_death, apply_escapes, progress_pass).chain().run_if(authoritative),
                    local_effects.after(progress_pass),
                )
                    .ambiguous_with_all(),
            );
        app.net_part("combat_arms_damage_fx", |app| {
            use bevy_replicon::prelude::*;
            app.replicate::<CaProgress>()
                .add_client_message::<CaEscapeRequest>(Channel::Ordered)
                .add_systems(
                    PreUpdate,
                    receive_escapes
                        .after(ServerSystems::Receive)
                        .run_if(in_state(ServerState::Running)),
                );
        });
    }
}
