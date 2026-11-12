//! Ladders across games on the real maps (docs/DESIGN.md "Ladders across
//! games"; the light tests: ladders_across_games.rs): on an HL2:DM map
//! (dm_resistance, dm_steamlab) CS:S's and Combat Arms' movements climb its
//! `func_useableladder`s (published as `core::MapLadders::lines`) walking
//! into them, as HL2:DM's own does; on a CS:S map (cs_office) HL2:DM's
//! movement climbs its ladder brushes (as lines). Each ladder is tried from
//! 0.7 m out on four sides, looking 45 degrees up: a side where the
//! character can't stand (the wall) is skipped. Passes with a note without
//! the installs.

use bevy::prelude::*;
use chimera_core::{core::MapLadders, mount::config::LocalConfig};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::harness::CaSim;
use party_kit::PartyPartsPlugin;

const CSS: &str = cs_source::game::movement::ID;
const CA: &str = combat_arms::game::movement::ID;
const HL2: &str = hl2dm::game::movement::ID;

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

/// A character of `movement` dropped at `at` (its feet), settled, still
/// within 0.3 m of there sideways and between `below` m under and 0.3 m
/// over.
fn stand(sim: &mut Sim, movement: &'static str, at: Vec3, below: f32) -> Option<Entity> {
    let p = sim.spawn_character(at + Vec3::Y, movement);
    sim.seconds(1.0);
    let low = sim.position(p).y + sim.state(p).hull_min.y;
    if low < at.y - below || low > at.y + 0.3 || sim.position(p).xz().distance(at.xz()) > 0.3 {
        sim.app.world_mut().despawn(p);
        return None;
    }
    Some(p)
}

/// How far `movement` rose walking into the ladder whose foot is `foot`
/// from each side it can stand on (0.7 m out), the best side.
fn best_climb(sim: &mut Sim, movement: &'static str, foot: Vec3) -> Option<f32> {
    let mut best: Option<f32> = None;
    for dir in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
        let Some(p) = stand(sim, movement, foot + dir * 0.7 + Vec3::Y * 0.05, 1.5) else { continue };
        let rose = climb_from(sim, p, -dir);
        best = Some(best.map_or(rose, |b| b.max(rose)));
    }
    best
}

/// `p` walking toward `to` (horizontal) looking 45 degrees up for 3 s:
/// how far it rose (m); then gone.
fn climb_from(sim: &mut Sim, p: Entity, to: Vec3) -> f32 {
    let start = sim.position(p).y;
    let mut rose = 0.0f32;
    for _ in 0..(3.0 * sim.tick_hz()) as u32 {
        let mut i = sim.intent(p);
        i.yaw = (-to.x).atan2(-to.z);
        i.pitch = std::f32::consts::FRAC_PI_4;
        i.move_axis = Vec2::Y;
        sim.ticks(1);
        rose = rose.max(sim.position(p).y - start);
    }
    sim.app.world_mut().despawn(p);
    rose
}

/// Every ladder line of HL2:DM map `id`: each movement's best climb.
fn lines_climbed(id: &str) {
    let Some(mut sim) = party_on(id) else { return };
    let lines = sim.app.world().resource::<MapLadders>().lines.clone();
    assert!(!lines.is_empty(), "{id} publishes its ladder lines");
    for (k, l) in lines.iter().enumerate() {
        let height = l.top.y - l.bottom.y;
        let mut got = Vec::new();
        for movement in [HL2, CSS, CA] {
            got.push((movement, best_climb(&mut sim, movement, l.bottom)));
        }
        eprintln!("{id} ladder {k} ({height:.1} m) at {:.1}: {got:.2?}", l.bottom);
        // Where HL2:DM's own climbs it, the others do too.
        if got[0].1.is_some_and(|r| r > 1.0) {
            for (m, r) in &got[1..] {
                assert!(r.is_some_and(|r| r > 1.0), "{id} ladder {k}: {m} climbed {r:?}");
            }
        }
    }
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
fn hl2dm_climbs_cs_offices_ladder_brushes() {
    let Some(mut sim) = party_on("cs_source:cs_office") else { return };
    let volumes = sim.app.world().resource::<MapLadders>().volumes.clone();
    assert!(!volumes.is_empty());
    let (mut tried, mut failed) = (0, Vec::new());
    for (k, b) in volumes.iter().enumerate() {
        // Head-on from 0.6 m out on each side of its thin axis (its foot
        // may be under the floor: dropped from 1.5 m over it).
        let size = b.max - b.min;
        let thin = if size.x < size.z { Vec3::X } else { Vec3::Z };
        let centre = (b.min + b.max) / 2.0;
        for side in [1.0, -1.0] {
            let out = thin * side * (size.dot(thin) / 2.0 + 0.6);
            let feet = Vec3::new(centre.x + out.x, b.min.y + 1.5, centre.z + out.z);
            let mut rose = Vec::new();
            for movement in [CSS, HL2] {
                let Some(p) = stand(&mut sim, movement, feet, 2.0) else { continue };
                rose.push(climb_from(&mut sim, p, -thin * side));
            }
            if rose.len() < 2 {
                continue;
            }
            eprintln!("cs_office ladder {k} at {centre:.1} side {side}: CS:S {:.2}, HL2:DM {:.2}", rose[0], rose[1]);
            // Only where CS:S climbs: a ladder side against a wall isn't one.
            if rose[0] > 1.0 {
                tried += 1;
                if rose[1] < 1.0 {
                    failed.push((k, side, rose[1]));
                }
            }
        }
    }
    assert!(tried > 0, "some ladder CS:S climbs");
    assert!(failed.is_empty(), "HL2:DM didn't climb {failed:?} of {tried}");
}
