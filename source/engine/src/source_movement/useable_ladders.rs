//! Half-Life 2's ladders (`func_useableladder` with `info_ladder_dismount`,
//! specs/hl2dm/movement.md "Ladders"), for a Source movement whose config
//! turns them on (`SourceMovementConfig::useable_ladders`; HL2:DM's): the
//! map's ladders as lines between two points (`UseableLadders`, built from
//! the map's entities), finding and mounting one (by `+use`, or
//! automatically when walking into it), the forced moves that pull a
//! player onto a ladder or off it onto a dismount point, climbing at 200
//! units/s along the line in the direction the view picks, jumping off,
//! falling off, and the dismounts at the ends and by `+use`. Contents
//! ladders (brushes) are not climbed then (`contents_ladders` off).
//!
//! Across games (docs/DESIGN.md "Ladders across games"): the map's ladder
//! lines are published as `core::MapLadders::lines` (engine space) for the
//! other games' movement, and the other ladders are climbed this way too,
//! as lines (`lines_of_volume`, ours: the specs are silent on mixing): a
//! ladder volume's two climbing faces (its thinnest across, upright) each
//! give a line 18 units out from the face's middle (the hull's half width
//! and Source's 2-unit ladder reach), bottom to top of the face, left over
//! the top (beyond the volume, on the floor it leads to) or at the foot;
//! a face with no room for a player in front of it (a wall) gives none
//! (`fix_ends`). Another game's ladder volumes (`LadderZone`, Combat Arms')
//! always; the map's ladder brushes (`MapLadders::volumes`) only on a map
//! that isn't the movement's game's (`core::foreign_map`), so HL2:DM on its
//! own maps keeps contents ladders solid. They follow the map's own ladders
//! in `UseableLadders` (whose indices stay), sorted, so a client builds the
//! same; they fire no map outputs.
//!
//! Known shortcuts (docs/tech-debt.md): the `Enable`/`Disable` inputs and
//! ladders moving with a parent aren't handled (`StartDisabled` is); a
//! fresh `+use` that mounts or dismounts isn't kept from the map's `+use`
//! (doors, chargers) that tick; the `sv_ladder_useonly` hint isn't shown;
//! the forced move's reserved box blocks other players' movement only (not
//! physics props). Why it's OK for now: stock maps' ladders are plain and
//! never disabled (specs/hl2dm/movement.md, H10).

use bevy::prelude::*;

use super::Mover;
use crate::{
    core::{LadderLine, LadderZone, MapBrush, MapGame, MapLadders, foreign_map, movement_game},
    map::MapEntity,
};

/// Search radius for a ladder, units.
const FIND: f32 = 64.0;
/// Auto-mount cone: distance (units) and half angle (degrees).
const CONE_DIST: f32 = 32.0;
const CONE_ANGLE: f32 = 15.0;
const REALLY_CLOSE: f32 = 4.0;
/// "Near an end" and the end-point mount, units.
const END_RADIUS: f32 = 16.0;
/// Climb speed along the ladder, units/s.
const CLIMB: f32 = 200.0;
/// The direction rule's dead zone (0 under 64 units of height).
const CHANGEOVER: f32 = 0.3;
const FALL_OFF: f32 = 6.0;
/// Jump-off: along the view, and up.
const JUMP_OFF: f32 = 200.0;
const JUMP_OFF_UP: f32 = 50.0;
/// Leaving by `+use` with no dismount point.
const USE_LEAVE: f32 = 100.0;
const USE_LEAVE_VZ: f32 = 50.0;
/// The dismount list: nodes this near the ends or the points sampled along
/// the ladder, at this spacing.
const DISMOUNT_LIST: f32 = 100.0;
const DISMOUNT_SAMPLE: f32 = 40.0;
const DISMOUNT_PICK: f32 = 100.0;
const DISMOUNT_DOT: f32 = 0.5;
const STRICT_DOT: f32 = 0.7;
const STRICT_DIST: f32 = 40.0;
const ALTERNATE_DIST: f32 = 60.0;
/// `along` (`sv_ladderautomountdot`) and "looking along it" for the ends.
const AXIS_DOT: f32 = 0.4;
const ALONG_END: f32 = 0.9;

/// Where a useable ladder comes from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum LadderFrom {
    /// The map's own `func_useableladder`.
    #[default]
    Map,
    /// A ladder brush of the map's (contents), as a line.
    Brush,
    /// Another game's ladder volume (`core::LadderZone`), as a line.
    Zone,
}

/// No map entity (a ladder from a volume): fires no outputs.
pub const NO_ENTITY: usize = usize::MAX;

/// One `func_useableladder` (Source units), or another ladder as one.
#[derive(Clone, Debug, PartialEq)]
pub struct UseableLadder {
    /// Index into the map's entities (its outputs; `NO_ENTITY`: none).
    pub map_index: usize,
    pub top: Vec3,
    pub bottom: Vec3,
    pub enabled: bool,
    /// Spawn flag 1: the checks, output and sound run, then it refuses.
    pub dont_get_on: bool,
    /// Its `info_ladder_dismount` points.
    pub dismounts: Vec<Vec3>,
    /// The end points were fixed up against the world (`fix_ends`).
    pub fixed: bool,
    pub from: LadderFrom,
}

impl UseableLadder {
    fn length(&self) -> f32 {
        (self.top - self.bottom).length()
    }

    fn up(&self) -> Vec3 {
        (self.top - self.bottom).normalize_or_zero()
    }

    /// The closest point of the segment to `p`.
    pub fn closest(&self, p: Vec3) -> Vec3 {
        let d = self.top - self.bottom;
        let l2 = d.length_squared();
        if l2 <= 0.0 {
            return self.bottom;
        }
        let t = ((p - self.bottom).dot(d) / l2).clamp(0.0, 1.0);
        self.bottom + d * t
    }

    /// Distance from `p` to the infinite line.
    fn line_distance(&self, p: Vec3) -> f32 {
        let up = self.up();
        let v = p - self.bottom;
        (v - up * v.dot(up)).length()
    }

    /// The position along the line: 0 at the top, 1 at the bottom.
    fn param(&self, p: Vec3) -> f32 {
        let d = self.bottom - self.top;
        let l2 = d.length_squared();
        if l2 <= 0.0 { 0.0 } else { (p - self.top).dot(d) / l2 }
    }
}

/// The map's ladders, rebuilt when its entities change.
#[derive(Resource, Clone, Debug, Default)]
pub struct UseableLadders(pub Vec<UseableLadder>);

/// The ladders among `entities` (Source units), with their dismount lists
/// (movement.md "The ladder entity").
pub fn ladders_of(entities: &[MapEntity]) -> Vec<UseableLadder> {
    let nodes: Vec<(Vec3, Option<String>)> = entities
        .iter()
        .filter(|e| e.classname().eq_ignore_ascii_case("info_ladder_dismount"))
        .map(|e| (e.origin(), e.get("target").map(str::trim).filter(|t| !t.is_empty()).map(str::to_lowercase)))
        .collect();
    let named = |name: &str| {
        entities
            .iter()
            .position(|e| e.get("targetname").is_some_and(|t| t.eq_ignore_ascii_case(name)))
    };
    entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.classname().eq_ignore_ascii_case("func_useableladder"))
        .map(|(i, e)| {
            let (a, b) = (e.vector("point0"), e.vector("point1"));
            let (top, bottom) = if a.z >= b.z { (a, b) } else { (b, a) };
            let mut samples = vec![top, bottom];
            let up = (top - bottom).normalize_or_zero();
            let len = (top - bottom).length();
            let mut s = DISMOUNT_SAMPLE;
            while s < len {
                samples.push(bottom + up * s);
                s += DISMOUNT_SAMPLE;
            }
            let dismounts = nodes
                .iter()
                .filter(|(p, target)| {
                    // A node naming another entity (the first of that name)
                    // belongs to that one.
                    target.as_deref().is_none_or(|t| named(t) == Some(i))
                        && samples.iter().any(|s| s.distance(*p) <= DISMOUNT_LIST)
                })
                .map(|(p, _)| *p)
                .collect();
            let flags: i64 = e.get("spawnflags").and_then(|v| v.trim().parse().ok()).unwrap_or(0);
            let disabled = e.get("StartDisabled").and_then(|v| v.trim().parse::<i64>().ok()).unwrap_or(0) != 0;
            UseableLadder {
                map_index: i,
                top,
                bottom,
                enabled: !disabled,
                dont_get_on: flags & 1 != 0,
                dismounts,
                fixed: false,
                from: LadderFrom::Map,
            }
        })
        .collect()
}

/// A convex brush's corners (engine space): where three of its planes meet
/// inside all the others.
fn corners(b: &MapBrush) -> Vec<Vec3> {
    let p = &b.planes;
    let mut out: Vec<Vec3> = Vec::new();
    for i in 0..p.len() {
        for j in i + 1..p.len() {
            for k in j + 1..p.len() {
                let (a, c, e) = (p[i], p[j], p[k]);
                let det = a.0.dot(c.0.cross(e.0));
                if det.abs() < 1e-6 {
                    continue;
                }
                let v = (c.0.cross(e.0) * a.1 + e.0.cross(a.0) * c.1 + a.0.cross(c.0) * e.1) / det;
                if p.iter().all(|(n, d)| n.dot(v) <= d + 1e-3) && !out.iter().any(|o| o.distance(v) < 1e-3) {
                    out.push(v);
                }
            }
        }
    }
    out
}

/// A ladder volume (engine space) as useable ladders (Source units), one
/// per climbing face: its upright faces thinnest across (a slab's two big
/// faces; a square post's four), each a line 18 units out from the face's
/// middle from its bottom to its top, with two dismount points: over the
/// top beyond the volume (20 units past its far side) and at the foot (8
/// out). Enabled until `fix_ends` finds no room in front.
pub fn lines_of_volume(b: &MapBrush, from: LadderFrom) -> Vec<UseableLadder> {
    let v = corners(b);
    if v.len() < 4 {
        return Vec::new();
    }
    let across = |n: Vec3| {
        let (lo, hi) = v.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(n.dot(*p)), hi.max(n.dot(*p))));
        hi - lo
    };
    let sides: Vec<(Vec3, f32)> = b.planes.iter().copied().filter(|(n, _)| n.y.abs() < 0.7).collect();
    let thin = sides.iter().map(|(n, _)| across(*n)).fold(f32::MAX, f32::min);
    let u = super::METERS_PER_UNIT;
    let mut out = Vec::new();
    for (n, d) in sides {
        if across(n) > thin + 0.01 {
            continue;
        }
        let face: Vec<Vec3> = v.iter().copied().filter(|p| (n.dot(*p) - d).abs() < 1e-3).collect();
        if face.len() < 3 {
            continue;
        }
        let flat = Vec3::new(n.x, 0.0, n.z).normalize_or_zero();
        let mid = face.iter().copied().sum::<Vec3>() / face.len() as f32;
        let lo = face.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let hi = face.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let out_by = flat * (super::HALF_WIDTH + super::LADDER_REACH) * u;
        let bottom = Vec3::new(mid.x, lo, mid.z) + out_by;
        let top = Vec3::new(mid.x, hi, mid.z) + out_by;
        let over = Vec3::new(mid.x, hi, mid.z) - flat * (thin + (super::HALF_WIDTH + 4.0) * u);
        let foot = bottom + flat * 8.0 * u;
        out.push(UseableLadder {
            map_index: NO_ENTITY,
            top: super::to_source(top),
            bottom: super::to_source(bottom),
            enabled: true,
            dont_get_on: false,
            dismounts: vec![super::to_source(over), super::to_source(foot)],
            fixed: false,
            from,
        });
    }
    out
}

/// A ladder line in engine space (`core::MapLadders::lines`) from a map's
/// useable ladder (Source units).
pub fn engine_line(l: &UseableLadder) -> LadderLine {
    LadderLine {
        bottom: super::to_engine(l.bottom),
        top: super::to_engine(l.top),
        dismounts: l.dismounts.iter().map(|d| super::to_engine(*d)).collect(),
    }
}

/// A forced move (mounting or dismounting): from `start` to `goal` at the
/// tick's movement cap, `ticks` run so far; `ladder` is where it ends on.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ForcedMove {
    pub start: Vec3,
    pub goal: Vec3,
    /// Seconds it takes.
    pub time: f32,
    pub ticks: u32,
    /// The ladder it ends on (a mount), or none (a dismount).
    pub ladder: Option<u32>,
}

/// What the ladder step left for the rest of the tick.
pub(super) enum LadderStep {
    /// Nothing more this tick (a forced move runs or started).
    Done,
    /// On a ladder: its move.
    Climb,
    /// The walking move.
    Walk,
}

/// The ladders' end points out of the floor they touch (a standing hull
/// swept down from 4 units above), once per map; a ladder made from a
/// volume with no room for a standing player halfway up its line (the
/// face against a wall) is off.
pub(super) fn fix_ends(trace: &super::Tracer, ladders: &mut [UseableLadder]) {
    for l in ladders.iter_mut().filter(|l| !l.fixed) {
        if l.from != LadderFrom::Map && trace.solid(false, (l.top + l.bottom) / 2.0) {
            l.enabled = false;
        }
        for p in [&mut l.top, &mut l.bottom] {
            let tr = trace.sweep(false, *p + Vec3::Z * 4.0, *p);
            if !tr.start_solid {
                *p = tr.end;
            }
        }
        l.fixed = true;
    }
}

impl Mover<'_, '_, '_, '_> {
    fn ladder_at(&self, i: u32) -> Option<&UseableLadder> {
        self.ladders.get(i as usize)
    }

    fn fire(&mut self, map_index: usize, output: &'static str) {
        if map_index == NO_ENTITY {
            return;
        }
        self.outputs.push((map_index, output));
    }

    /// Leave the ladder (walking, velocity as it is), with its output.
    fn leave_ladder(&mut self) {
        if let Some(l) = self.me.useable.take().and_then(|i| self.ladder_at(i).map(|l| l.map_index)) {
            self.fire(l, "OnPlayerGotOffLadder");
        }
    }

    /// The ladder part of the tick (movement.md "Per-tick order" 7).
    /// `f`/`s` the move input after the rescale and the duck scale, `cap`
    /// the movement cap M.
    pub(super) fn ladder_step(&mut self, intent: &crate::core::Intent, f: f32, s: f32, cap: f32) -> LadderStep {
        let fresh_use = intent.use_key && !self.me.use_held;
        // a. A forced move runs on.
        if let Some(mut m) = self.me.forced {
            m.ticks += 1;
            let k = m.ticks as f32 * self.dt;
            if k <= m.time {
                self.feet = m.start + (m.goal - m.start) * (k / m.time).min(1.0);
                self.me.forced = Some(m);
                return LadderStep::Done;
            }
            self.feet = m.goal;
            self.me.forced = None;
            self.v = Vec3::ZERO;
            self.me.fall_speed = 0.0;
            match m.ladder {
                Some(i) => self.me.useable = Some(i),
                None => {
                    // A dismount's arrival: the walking move without input.
                    self.categorize();
                    return LadderStep::Walk;
                }
            }
        }
        // b. Dropped when disabled or pushed.
        if let Some(i) = self.me.useable {
            let enabled = self.ladder_at(i).is_some_and(|l| l.enabled);
            if !enabled || self.base.length() > 1.0 {
                self.me.useable = None;
                return LadderStep::Walk;
            }
        }
        let (forward, right) = Self::view(intent);
        let Some(i) = self.me.useable else {
            // c. Find one and mount it.
            return if self.try_mount(intent, fresh_use, f, s, cap, forward) {
                LadderStep::Done
            } else {
                LadderStep::Walk
            };
        };
        let Some(l) = self.ladder_at(i).cloned() else {
            self.me.useable = None;
            return LadderStep::Walk;
        };
        // d. Jump off.
        if intent.jump {
            let dir = if f < 0.0 { -forward } else { forward };
            self.v = dir * JUMP_OFF;
            if forward.z >= 0.0 {
                self.v.z += JUMP_OFF_UP;
            }
            self.leave_ladder();
            return LadderStep::Walk;
        }
        let fs = (intent.move_axis.y > 0.0) as i32 - (intent.move_axis.y < 0.0) as i32;
        let rs = (intent.move_axis.x > 0.0) as i32 - (intent.move_axis.x < 0.0) as i32;
        if fs == 0 && rs == 0 {
            self.v = Vec3::ZERO;
            return LadderStep::Climb;
        }
        if l.line_distance(self.feet) > FALL_OFF {
            self.leave_ladder();
            return LadderStep::Walk;
        }
        let u = (forward * fs as f32 + right * rs as f32).normalize_or_zero();
        let c = if (l.top.z - l.bottom.z).abs() < 64.0 { 0.0 } else { CHANGEOVER };
        let up = l.up();
        let down = if u.z >= 0.0 { up.dot(u) < -c } else { -up.dot(u) > c };
        self.v = up * if down { -CLIMB } else { CLIMB };
        LadderStep::Climb
    }

    /// Find the nearest ladder in reach and mount it by `+use` or walking
    /// into it (movement.md "Finding a ladder", "Mounting").
    fn try_mount(&mut self, intent: &crate::core::Intent, fresh_use: bool, f: f32, s: f32, cap: f32, forward: Vec3) -> bool {
        let half = if self.me.ducked { self.trace.heights.1 } else { self.trace.heights.0 } / 2.0;
        let origin = self.feet;
        let mut best: Option<(u32, Vec3, f32)> = None;
        for (i, l) in self.ladders.iter().enumerate() {
            // The map's ladder brushes only on another game's map.
            if !l.enabled || (l.from == LadderFrom::Brush && !self.foreign_map) {
                continue;
            }
            let c = l.closest(origin);
            let d = c.distance(origin);
            if d > FIND || best.is_some_and(|(_, _, b)| b <= d) {
                continue;
            }
            let clear = |from: Vec3| !self.trace.sweep_box(Vec3::ZERO, Vec3::ZERO, from, c).hit();
            if clear(origin) || clear(origin + Vec3::Z * half) {
                best = Some((i as u32, c, d));
            }
        }
        let Some((i, c, d)) = best else { return false };
        let l = self.ladders[i as usize].clone();
        let useonly = self.cfg.ladder_use_only;
        let g = Vec3::new(forward.x, forward.y, 0.0);
        let mounts = if fresh_use {
            let from = origin - g * 8.0;
            let to = Vec3::new(c.x - from.x, c.y - from.y, 0.0).normalize_or_zero();
            to.dot(g.normalize_or_zero()) >= if useonly { -0.99 } else { 0.0 }
        } else if !useonly && f > 0.0 {
            let flat_to = Vec3::new(c.x - origin.x, c.y - origin.y, 0.0);
            let cos_phi = if flat_to.length_squared() > 0.0 && g.length_squared() > 0.0 {
                flat_to.normalize().dot(g.normalize())
            } else {
                0.0
            };
            let phi = cos_phi.clamp(-1.0, 1.0).acos().to_degrees();
            let strafing = s.abs() >= 1.0;
            let along = l.up().dot(forward).abs() > AXIS_DOT;
            let cone = d > 0.0 && d < CONE_DIST && (phi < CONE_ANGLE || d < REALLY_CLOSE) && ((cos_phi > 0.0 && !strafing) || along);
            let end = (origin.distance(l.bottom) <= END_RADIUS && l.up().dot(forward) > AXIS_DOT)
                || (origin.distance(l.top) <= END_RADIUS && -l.up().dot(forward) > AXIS_DOT);
            cone || end
        } else {
            false
        };
        let _ = intent;
        if !mounts {
            return false;
        }
        self.fire(l.map_index, "OnPlayerGotOnLadder");
        self.sounds.push(("Ladder.StepRight".into(), self.feet, None));
        if l.dont_get_on {
            return false;
        }
        if self.trace.solid(self.me.ducked, c) && self.trace.solid(self.me.ducked, c + Vec3::Z) {
            return false;
        }
        self.start_forced(c, cap, Some(i));
        true
    }

    fn start_forced(&mut self, goal: Vec3, cap: f32, ladder: Option<u32>) {
        let time = (goal.distance(self.feet) / cap.max(1e-3)).max(0.001);
        self.me.forced = Some(ForcedMove {
            start: self.feet,
            goal,
            time,
            ticks: 0,
            ladder,
        });
    }

    /// The ladder move (movement.md "The ladder move").
    pub(super) fn ladder_move(&mut self, intent: &crate::core::Intent, f: f32, s: f32, cap: f32) {
        let fresh_use = intent.use_key && !self.me.use_held;
        self.water_check();
        self.me.jump_held = false;
        self.set_ground(false, None);
        let Some(l) = self.me.useable.and_then(|i| self.ladder_at(i)).cloned() else { return };
        let (forward, _) = Self::view(intent);
        let (before_feet, before_v) = (self.feet, self.v);
        let t = l.param(self.feet);
        self.slide();
        let t2 = l.param(self.feet);
        let len = l.length().max(1e-3);
        let pressing = f != 0.0 || s != 0.0;
        let along = pressing && forward.dot(l.up()).abs() > ALONG_END;
        let near = self.feet.distance(l.top) <= END_RADIUS || self.feet.distance(l.bottom) <= END_RADIUS;
        let reach = CLIMB * self.dt;
        let at_end = self.feet.distance(l.top) <= reach || self.feet.distance(l.bottom) <= reach;
        let would_leave = (t2 < -1.0 / len && t2 < t) || (t2 > 1.0 + 1.0 / len && t2 > t);
        let use_dismount = fresh_use || (at_end && pressing && !along && self.cfg.ladder_auto_dismount);
        let put_back = |me: &mut Self| {
            me.feet = before_feet;
            me.v = before_v;
        };
        if self.me.water_level == 3 && !use_dismount && self.dismount(&l, forward, cap, true, false) {
            return;
        }
        if !use_dismount {
            if would_leave {
                put_back(self);
            }
            return;
        }
        if !would_leave && !near {
            if fresh_use && self.dismount(&l, forward, cap, false, true) {
                return;
            }
            if fresh_use {
                self.v = forward * USE_LEAVE;
                self.v.z = USE_LEAVE_VZ;
                self.leave_ladder();
            }
            return;
        }
        if self.dismount(&l, forward, cap, false, fresh_use) {
            return;
        }
        if would_leave {
            if fresh_use {
                self.v = forward * USE_LEAVE;
                self.v.z = USE_LEAVE_VZ;
                self.leave_ladder();
            } else {
                put_back(self);
            }
        }
    }

    /// Choose a dismount point and start the forced move there
    /// (movement.md "Choosing a dismount node").
    fn dismount(&mut self, l: &UseableLadder, forward: Vec3, cap: f32, strict: bool, alternate: bool) -> bool {
        let eye = self.feet + Vec3::Z * self.me.eye;
        let mut nodes: Vec<Vec3> = l.dismounts.iter().copied().filter(|n| n.distance(self.feet) <= DISMOUNT_PICK).collect();
        nodes.sort_by(|a, b| a.distance(self.feet).total_cmp(&b.distance(self.feet)));
        let (mut best, mut best_score) = (None, f32::NEG_INFINITY);
        let (mut alt, mut alt_dist) = (None, f32::INFINITY);
        for n in nodes {
            let spot = n + Vec3::Z;
            if self.trace.solid(self.me.ducked, spot) {
                continue;
            }
            let flat = Vec3::new(spot.x - eye.x, spot.y - eye.y, 0.0);
            let dist = flat.length();
            let score = flat.normalize_or_zero().dot(forward);
            if score < DISMOUNT_DOT {
                if dist < alt_dist {
                    alt = Some(spot);
                    alt_dist = dist;
                }
                continue;
            }
            if strict && (score < STRICT_DOT || dist > STRICT_DIST) {
                continue;
            }
            if score > best_score {
                best = Some(spot);
                best_score = score;
            }
        }
        let spot = best.or(alt.filter(|_| alternate && alt_dist <= ALTERNATE_DIST));
        let Some(spot) = spot else { return false };
        self.leave_ladder();
        self.start_forced(spot, cap, None);
        true
    }
}

/// Keep `UseableLadders` the map's (`map::MapEntities`), published as
/// `MapLadders::lines`, followed by the other ladders as lines: every
/// game's ladder volume (`LadderZone`), and the map's ladder brushes when
/// a movement climbing useable ladders isn't the map's game's (sorted by
/// place: the same on every machine).
#[allow(clippy::too_many_arguments)]
pub fn build_ladders(
    entities: Option<Res<crate::map::MapEntities>>,
    mut ladders: ResMut<UseableLadders>,
    published: Option<ResMut<MapLadders>>,
    zones: Query<Ref<LadderZone>>,
    mut gone: RemovedComponents<LadderZone>,
    game: Option<Res<MapGame>>,
    configs: Option<Res<super::SourceMovementConfigs>>,
    mut last_game: Local<Option<String>>,
) {
    let mut published = published;
    let own_changed = match &entities {
        Some(e) => e.is_changed(),
        None => ladders.0.iter().any(|l| l.from == LadderFrom::Map),
    };
    if own_changed {
        let own = entities.as_ref().map(|e| ladders_of(&e.entities)).unwrap_or_default();
        if let Some(p) = published.as_mut() {
            p.lines = own.iter().map(engine_line).collect();
        }
        ladders.0 = own;
    }
    let zones_changed = zones.iter().any(|z| z.is_changed()) | (gone.read().count() > 0);
    let volumes_changed = published.as_ref().is_some_and(|p| p.is_changed());
    let now = game.as_ref().map(|g| g.0.clone());
    let game_changed = *last_game != now;
    *last_game = now;
    if !(own_changed || zones_changed || volumes_changed || game_changed) {
        return;
    }
    // The map's brushes as lines only for a movement of another game.
    let useable = configs
        .iter()
        .flat_map(|o| o.0.iter().map(|(id, c)| (movement_game(id), c)))
        .filter(|(_, c)| c.useable_ladders);
    let brushes = useable.map(|(g, _)| g).any(|g| foreign_map(game.as_deref(), g));
    let mut more: Vec<UseableLadder> = zones.iter().flat_map(|z| lines_of_volume(&z.0, LadderFrom::Zone)).collect();
    if brushes {
        if let Some(p) = published.as_ref() {
            more.extend(p.volumes.iter().flat_map(|b| lines_of_volume(b, LadderFrom::Brush)));
        }
    }
    let key = |l: &UseableLadder| (l.from, [l.bottom.x, l.bottom.y, l.bottom.z, l.top.z].map(f32::to_bits));
    more.sort_by_key(key);
    // Replaced only when they differ (a change elsewhere keeps their
    // fixed ends and the indices of those climbed).
    let keep = ladders.0.iter().filter(|l| l.from == LadderFrom::Map).count();
    let unfixed = |l: &UseableLadder| (l.from, l.dismounts.clone());
    if ladders.0[keep..].iter().map(unfixed).ne(more.iter().map(unfixed)) {
        ladders.0.truncate(keep);
        ladders.0.extend(more);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn entity(pairs: &[(&str, &str)]) -> MapEntity {
        MapEntity {
            keyvalues: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            ..Default::default()
        }
    }

    /// The ladder's ends (the higher point is the top), flags, and its
    /// dismount list: nodes within 100 of the ends or of the points every
    /// 40 up, not those naming another entity.
    #[test]
    fn ladders_and_their_dismount_points() {
        let ents = vec![
            entity(&[("classname", "func_useableladder"), ("targetname", "a"), ("point0", "0 0 8"), ("point1", "0 0 264")]),
            entity(&[("classname", "func_useableladder"), ("targetname", "b"), ("point0", "1000 0 0"), ("point1", "1000 0 100"), ("spawnflags", "1"), ("StartDisabled", "1")]),
            entity(&[("classname", "info_ladder_dismount"), ("origin", "48 0 264")]),
            // Beside the middle of a: within 100 of the point 120 up.
            entity(&[("classname", "info_ladder_dismount"), ("origin", "90 0 130")]),
            // Too far from anything of a.
            entity(&[("classname", "info_ladder_dismount"), ("origin", "300 0 130")]),
            // Near a, but naming b.
            entity(&[("classname", "info_ladder_dismount"), ("origin", "40 0 8"), ("target", "b")]),
        ];
        let l = ladders_of(&ents);
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].top, l[0].bottom), (Vec3::new(0.0, 0.0, 264.0), Vec3::new(0.0, 0.0, 8.0)));
        assert_eq!(l[0].dismounts, [Vec3::new(48.0, 0.0, 264.0), Vec3::new(90.0, 0.0, 130.0)]);
        assert!(l[0].enabled && !l[0].dont_get_on);
        assert!(!l[1].enabled && l[1].dont_get_on);
        assert!(l[1].dismounts.is_empty());
        // The line: closest points and the parameter from the top.
        assert_eq!(l[0].closest(Vec3::new(-20.0, 0.0, 0.0)), Vec3::new(0.0, 0.0, 8.0));
        assert!((l[0].param(Vec3::new(5.0, 0.0, 136.0)) - 0.5).abs() < 1e-6);
        assert!((l[0].line_distance(Vec3::new(3.0, 4.0, 50.0)) - 5.0).abs() < 1e-6);
    }
}
