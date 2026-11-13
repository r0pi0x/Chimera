//! Ladders across games on the real maps (docs/DESIGN.md "Ladders across
//! games"; the light tests: ladders_across_games.rs): a character climbs
//! another game's map's ladders up onto the floor they lead to. On an
//! HL2:DM map (dm_resistance, dm_steamlab) CS:S's and Combat Arms'
//! movements climb each `func_useableladder` (published as
//! `core::MapLadders::lines`) walking into it, from some side, and end
//! standing on the floor of its top dismount point; on CS:S maps
//! (cs_office, de_nuke; `CHIMERA_LADDER_MAPS` names others) HL2:DM's
//! (each ladder brush as lines) and Combat Arms' (as `Ladder` volumes)
//! climb each ladder brush from each side CS:S's does and end standing on
//! the floor CS:S's does. A climber walks into the ladder looking 45
//! degrees up and lets go once it stands on a floor a metre over its start
//! (`climb_to_top`); Combat Arms', once on a brush, faces the face it is
//! on (`into_face`). Passes with a note without the installs.
//! `CHIMERA_LADDER_CROSS_TRACE=1` prints each climber's ticks, HL2:DM's
//! ladder lines and the brushes by where each climb ended;
//! `CHIMERA_LADDER_ONLY=<k>` tries ladder brush k only.

use bevy::prelude::*;
use chimera_core::{
    core::{MapBrush, MapLadders},
    mount::config::LocalConfig,
};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::harness::CaSim;
use cs_source::game::movement::{SourceMovement, UseableLadders};
use party_kit::PartyPartsPlugin;

const CSS: &str = cs_source::game::movement::ID;
const CA: &str = combat_arms::game::movement::ID;
const HL2: &str = hl2dm::game::movement::ID;

/// How far (m) a climber may end from the height the map's own game's
/// climber ends at: a stair step.
const SAME_FLOOR: f32 = 18.0 * 0.0254;

/// How far over CS:S's floor another climber may end on a CS:S map: a
/// floor beyond a ledge at the ladder's top that CS:S's climber stopped
/// on, as high as HL2:DM's lines reach (`useable_ladders::to_floors`).
const OVER_LEDGE: f32 = 36.0 * 0.0254;

/// The party on map `id`; None (with a note) without the installs.
fn party_on(id: &str) -> Option<Sim> {
    let config = LocalConfig::load().ok()?;
    let ca = config
        .game_path(combat_arms::game::GAME)
        .filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    let map = party_kit::load_map(id, 0).ok();
    if ca.is_none() || map.is_none() {
        eprintln!("skipping: needs the installs for {id}");
        return None;
    }
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map.unwrap()), PartyPartsPlugin));
    sim.ticks(3);
    Some(sim)
}

fn feet(sim: &Sim, p: Entity) -> f32 {
    sim.position(p).y + sim.state(p).hull_min.y
}

/// A character of `movement` dropped at `at` (its feet), settled, still
/// within 0.3 m of there sideways and between `below` m under and 0.3 m
/// over.
fn stand(sim: &mut Sim, movement: &'static str, at: Vec3, below: f32) -> Option<Entity> {
    let p = sim.spawn_character(at + Vec3::Y, movement);
    sim.seconds(1.0);
    let low = feet(sim, p);
    if low < at.y - below || low > at.y + 0.3 || sim.position(p).xz().distance(at.xz()) > 0.3 {
        sim.app.world_mut().despawn(p);
        return None;
    }
    Some(p)
}

/// Where a climb came to.
#[derive(Clone, Copy, Debug)]
struct Climb {
    /// The most it rose (m).
    rose: f32,
    /// Its feet at the end (engine height, m).
    end: f32,
    /// Where it ended (engine, m).
    at: Vec3,
    /// Standing on a floor at the end (off the ladder), a metre or more
    /// over its start.
    up: bool,
}

impl Climb {
    /// Standing on a floor at height `y` (within a stair step).
    fn on_floor(&self, y: f32) -> bool {
        self.up && (self.end - y).abs() <= SAME_FLOOR
    }

    /// Standing on a floor at height `y`, or on one over it the climb
    /// reaches (over a ledge at the ladder's top: `OVER_LEDGE`).
    fn on_or_over(&self, y: f32) -> bool {
        self.up && self.end >= y - SAME_FLOOR && self.end <= y + OVER_LEDGE
    }
}

fn tracing() -> bool {
    chimera_core::env::var_os("CHIMERA_LADDER_CROSS_TRACE").is_some()
}

/// `p` walking toward `to` (horizontal) looking 45 degrees up, for up to
/// `secs` (from where it is: on a floor, or in the air), letting go once
/// it stands on a floor a metre over its start; then still for half a
/// second, and gone. Near the ladder's top (`top`, engine height: within
/// 0.3 m) or stalled on it (not rising for a quarter second), it looks
/// level toward the floor there, as a player would: `aim` (engine; else
/// straight on). At the foot it presses `+use` every 16 ticks.
#[allow(clippy::too_many_arguments)]
fn climb_to_top(sim: &mut Sim, p: Entity, to: Vec3, aim: Option<Vec3>, top: f32, brush: Option<&MapBrush>, secs: f32) -> Climb {
    let ca = sim.app.world().get::<combat_arms::game::movement::CaMovement>(p).is_some();
    let start = feet(sim, p);
    let (mut rose, mut still, mut k) = (0.0f32, 0u32, 0u32);
    let stall = (0.25 * sim.tick_hz()) as u32;
    for _ in 0..(secs as f64 * sim.tick_hz()) as u32 {
        let level = still >= stall || feet(sim, p) >= top - 0.3;
        let dir = match aim {
            Some(a) if level => (a - sim.position(p)).with_y(0.0).normalize_or(to),
            _ => to,
        };
        // Combat Arms' climber, on a ladder brush, faces the face it is on.
        let dir = match brush.filter(|_| ca && sim.state(p).on_ladder) {
            Some(b) => into_face(b, sim.position(p)).unwrap_or(dir),
            None => dir,
        };
        // Off the ladder at the foot, `+use` now and then, as HL2:DM's
        // players get on one at an angle.
        let press = !sim.state(p).on_ladder && rose < 0.3 && k % 16 == 0;
        k += 1;
        let mut i = sim.intent(p);
        i.yaw = (-dir.x).atan2(-dir.z);
        i.pitch = if level { 0.0 } else { std::f32::consts::FRAC_PI_4 };
        i.move_axis = Vec2::Y;
        i.use_key = press;
        sim.ticks(1);
        let now = feet(sim, p) - start;
        let s = sim.state(p);
        if now > rose + 0.01 {
            rose = now;
            still = 0;
        } else if now > 0.5 && !s.on_ground {
            still += 1;
        }
        if tracing() && let Some(m) = sim.app.world().get::<SourceMovement>(p) {
            let f = m.forced.map(|f| f.goal);
            eprintln!("  feet {now:.3} useable {:?} forced {f:?} ladder {:?} ground {}", m.useable, m.ladder, s.on_ground);
        }
        if tracing() && let Some(m) = sim.app.world().get::<combat_arms::game::movement::CaMovement>(p) {
            let v = sim.app.world().get::<chimera_core::core::Velocity>(p).map(|v| v.0).unwrap_or_default();
            eprintln!("  feet {now:.3} at {:.2} v {v:.2} ca ladder {} ground {} state ground {} eye {}", sim.position(p), m.on_ladder, m.on_ground, s.on_ground, m.eye_free);
        }
        if s.on_ground && !s.on_ladder && now > 1.0 {
            break;
        }
    }
    if tracing() {
        for l in &sim.app.world().resource::<UseableLadders>().0 {
            eprintln!("  line {:?} {:?}..{:?} on {} dismounts {:?}", l.from, l.bottom, l.top, l.enabled, l.dismounts);
        }
        let (c, s) = (sim.position(p), sim.state(p));
        let (lo, hi) = (c + s.hull_min - Vec3::splat(0.05), c + s.hull_max + Vec3::splat(0.05));
        for b in &sim.app.world().resource::<chimera_core::core::MapBrushes>().0 {
            if b.min.cmple(hi).all() && b.max.cmpge(lo).all() {
                eprintln!("  near {:.2}..{:.2} ladder {} planes {:.3?}", b.min, b.max, b.ladder, b.planes);
            }
        }
    }
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.seconds(0.5);
    let s = sim.state(p);
    let end = feet(sim, p);
    let c = Climb { rose, end, at: sim.position(p), up: s.on_ground && !s.on_ladder && end - start > 1.0 };
    sim.app.world_mut().despawn(p);
    c
}

/// Each of `movements` from `feet` walking toward `to` (on ladder brush
/// `brush`, if one): None where it can't stand there.
#[allow(clippy::too_many_arguments)]
fn climbs<const N: usize>(
    sim: &mut Sim,
    movements: [&'static str; N],
    feet: Vec3,
    below: f32,
    to: Vec3,
    aim: Option<Vec3>,
    top: f32,
    brush: Option<&MapBrush>,
) -> [Option<Climb>; N] {
    movements.map(|m| stand(sim, m, feet, below).map(|p| climb_to_top(sim, p, to, aim, top, brush, 6.0)))
}

/// The level direction into the side face of ladder brush `b` nearest to
/// `at` (the one it is furthest out of).
fn into_face(b: &MapBrush, at: Vec3) -> Option<Vec3> {
    b.planes
        .iter()
        .filter(|(n, _)| n.y.abs() < 0.7)
        .max_by(|(n, d), (m, e)| (n.dot(at) - d).total_cmp(&(m.dot(at) - e)))
        .map(|(n, _)| -n.with_y(0.0).normalize())
}

/// Every ladder line of HL2:DM map `id` with a dismount point over its
/// middle (its top floor: the highest), tried from each side 0.7 m out:
/// CS:S and Combat Arms each stand on that floor from some side.
fn lines_climbed(id: &str) {
    let Some(mut sim) = party_on(id) else { return };
    let lines = sim.app.world().resource::<MapLadders>().lines.clone();
    assert!(!lines.is_empty(), "{id} publishes its ladder lines");
    let (mut tried, mut failed) = (0, Vec::new());
    for (k, l) in lines.iter().enumerate() {
        let mid = (l.bottom.y + l.top.y) / 2.0;
        let top = l.dismounts.iter().filter(|d| d.y > mid).max_by(|a, b| a.y.total_cmp(&b.y)).copied();
        let floor = top.map_or(f32::NAN, |t| t.y);
        let what = format!("{id} ladder {k} ({:.1} m) at {:.1}", l.top.y - l.bottom.y, l.bottom);
        if floor.is_nan() {
            eprintln!("{what}: no dismount point at the top");
            continue;
        }
        tried += 1;
        let mut reached = [false; 3];
        for dir in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
            let got = climbs(&mut sim, [HL2, CSS, CA], l.bottom + dir * 0.7 + Vec3::Y * 0.05, 1.5, -dir, top, l.top.y, None);
            eprintln!("{what} from {dir}, top floor {floor:.2}: {got:.2?}");
            for (r, c) in reached.iter_mut().zip(&got) {
                *r |= c.is_some_and(|c| c.on_floor(floor));
            }
        }
        eprintln!("{what}: on the top floor (HL2:DM, CS:S, Combat Arms) {reached:?}");
        for (m, r) in [CSS, CA].iter().zip(&reached[1..]) {
            if !r {
                failed.push(format!("{what}: {m}"));
            }
        }
    }
    assert!(tried > 0, "{id}: a ladder with a top dismount point");
    assert!(failed.is_empty(), "{id}: not on the top floor from any side, of {tried}:\n{}", failed.join("\n"));
}

/// Every ladder brush of CS:S map `id`, head-on from 0.6 m out on each
/// side of its box's thinner axis (a ladder 3 m or taller is also
/// grabbed in the air a third of the way up where not from the floor,
/// printed only): where CS:S's climber gets up onto a floor, HL2:DM's and
/// Combat Arms' end standing on it too.
fn brushes_climbed(id: &str) {
    let Some((tried, failed)) = brush_failures(id) else { return };
    assert!(tried > 0, "{id}: CS:S climbs some ladder");
    assert!(failed.is_empty(), "{id}: not on CS:S's floor:\n{}", failed.join("\n"));
}

/// `brushes_climbed`'s sides tried (where CS:S got up) and failures on
/// `id` (None without the installs).
fn brush_failures(id: &str) -> Option<(usize, Vec<String>)> {
    let mut sim = party_on(id)?;
    let volumes = sim.app.world().resource::<MapLadders>().volumes.clone();
    if volumes.is_empty() {
        eprintln!("{id}: no ladder brushes");
        return Some((0, Vec::new()));
    }
    let (mut tried, mut failed) = (0, Vec::new());
    let only: Option<usize> = chimera_core::env::var("CHIMERA_LADDER_ONLY").ok().and_then(|s| s.parse().ok());
    for (k, b) in volumes.iter().enumerate() {
        if only.is_some_and(|o| o != k) {
            continue;
        }
        if tracing() {
            eprintln!("{id} ladder {k}: {:.2}..{:.2} planes {:.3?}", b.min, b.max, b.planes);
        }
        let size = b.max - b.min;
        let thin = if size.x < size.z { Vec3::X } else { Vec3::Z };
        let thick = size.dot(thin);
        let centre = (b.min + b.max) / 2.0;
        for side in [1.0, -1.0] {
            // Its foot may be under the floor: dropped from 1.5 m over it.
            let out = thin * side * (thick / 2.0 + 0.6);
            let feet = Vec3::new(centre.x + out.x, b.min.y + 1.5, centre.z + out.z);
            let got = climbs(&mut sim, [CSS, HL2, CA], feet, 2.0, -thin * side, None, b.max.y, Some(b));
            let face = (-thin * side, centre.dot(-thin * side) - thick / 2.0);
            let what = format!("{id} ladder {k} ({:.1} m) at {centre:.1} side {side}", size.y);
            if compare(&what, got, face, &mut tried, &mut failed) || size.y < 3.0 {
                continue;
            }
            // Not from the floor in front (something in the way): grabbing
            // it in the air a third of the way up, as on a train car's.
            let at = Vec3::new(centre.x, b.min.y + size.y / 3.0 + 0.92, centre.z)
                + thin * side * (thick / 2.0 + 17.0 * 0.0254);
            let got = [CSS, HL2, CA].map(|m| {
                let p = sim.spawn_character(at, m);
                climb_to_top(&mut sim, p, -thin * side, None, b.max.y, Some(b), 6.0)
            });
            // Printed only: a ladder over another is got onto from it.
            let (mut t, mut f) = (0, Vec::new());
            compare(&format!("{what}, in the air"), got.map(Some), face, &mut t, &mut f);
        }
    }
    let missed = |m: &str| failed.iter().filter(|f: &&String| f.contains(&format!(": {m} "))).count();
    eprintln!(
        "{id}: on CS:S's floor in {tried}: HL2:DM {}, Combat Arms {}",
        tried - missed("HL2:DM"),
        tried - missed("Combat Arms")
    );
    Some((tried, failed))
}

/// Where CS:S's climber (`got[0]`) got up onto a floor (its middle at
/// most 0.1 m short of the ladder's near face: `face`, its inward
/// direction and where along it the face is), HL2:DM's (`got[1]`) and
/// Combat Arms' (`got[2]`) must stand on it too, or over it
/// (`Climb::on_or_over`).
fn compare(what: &str, got: [Option<Climb>; 3], face: (Vec3, f32), tried: &mut usize, failed: &mut Vec<String>) -> bool {
    eprintln!("{what}: {got:.2?}");
    // Up onto a floor at the ladder's face or beyond it, not perched on
    // the ladder's top in front of a wall.
    let Some(css) = got[0].filter(|c| c.up && c.at.dot(face.0) >= face.1 - 0.1) else { return false };
    *tried += 1;
    let on = got.map(|c| c.is_some_and(|c| c.on_or_over(css.end)));
    eprintln!("{what}: on CS:S's floor (CS:S, HL2:DM, Combat Arms) {on:?}");
    for (k, m) in [(1, "HL2:DM"), (2, "Combat Arms")] {
        if !on[k] {
            failed.push(format!("{what}: {m} {:?}, CS:S {css:?}", got[k]));
        }
    }
    true
}

#[test]
fn css_and_combat_arms_climb_dm_resistances_ladder() {
    lines_climbed("hl2dm:dm_resistance");
}

#[test]
fn css_and_combat_arms_climb_dm_steamlabs_ladders() {
    lines_climbed("hl2dm:dm_steamlab");
}

#[test]
fn hl2dm_and_combat_arms_climb_cs_offices_ladders() {
    brushes_climbed("cs_source:cs_office");
}

#[test]
fn hl2dm_and_combat_arms_climb_de_nukes_ladders() {
    // Its ladder 0 (outside): the others' tops (the vents', the one
    // between walls) CS:S's climber doesn't get off this way.
    brushes_climbed("cs_source:de_nuke");
}

/// The CS:S maps `CHIMERA_LADDER_MAPS` names (comma-separated, without
/// the `cs_source:`), the same way; nothing without it.
#[test]
fn hl2dm_and_combat_arms_climb_listed_maps_ladders() {
    let Ok(list) = chimera_core::env::var("CHIMERA_LADDER_MAPS") else { return };
    let failed: Vec<String> =
        list.split(',').map(str::trim).filter(|s| !s.is_empty()).flat_map(|n| brush_failures(&format!("cs_source:{n}")).map(|r| r.1).unwrap_or_default()).collect();
    assert!(failed.is_empty(), "not on CS:S's floor:\n{}", failed.join("\n"));
}
