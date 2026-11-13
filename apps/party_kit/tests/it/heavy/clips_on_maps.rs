//! Player clips across games on the real maps (the user's installs;
//! docs/DESIGN.md "Clips across games"; the light tests:
//! clips_across_games.rs). cs_militia's clip walls round the yard and
//! cs_havana's out-of-bounds yard clip, and every other upright player-clip
//! wall of those maps (`core::MapPlayerClips`) over a player's head from
//! the floor: from each side of each such wall where CS:S's movement
//! stands on a floor and is kept out walking into it, HL2:DM's and Combat
//! Arms' (a player and a computer player) are kept out too (their centre's
//! path never meets the brush). Combat Arms' walked through them before
//! (it sweeps colliders, and the clips had none). Passes with a note
//! without the installs. `CHIMERA_CLIP_MAPS` names other maps
//! (comma-separated ids); `CHIMERA_CLIPS_OFF=1` drops the clips' collider
//! (the control: Combat Arms' walkers fail).

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::{
    core::{MapBrush, MapBrushes, MapPlayerClips, Npc, SOLID_LAYERS},
    mount::config::LocalConfig,
};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::harness::CaSim;
use party_kit::PartyPartsPlugin;

const CSS: &str = cs_source::game::movement::ID;
const CA: &str = combat_arms::game::movement::ID;
const HL2: &str = hl2dm::game::movement::ID;

/// Most wall sides tried a map (the walks take a while).
const MOST_SIDES: usize = 40;

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

/// The floor under `at` (engine): a ray down 3 m through what shots meet.
fn floor_under(sim: &mut Sim, at: Vec3) -> Option<f32> {
    let w = sim.app.world_mut();
    let mut state: SystemState<SpatialQuery> = SystemState::new(w);
    let spatial = state.get(w).unwrap();
    spatial
        .cast_ray(at, Dir3::NEG_Y, 3.0, true, &SpatialQueryFilter::from_mask(SOLID_LAYERS))
        .map(|h| at.y - h.distance)
}

/// A character of `movement` dropped at `at` (its feet), settled, still
/// within 0.1 m of there sideways and 0.3 m up or down.
fn stand(sim: &mut Sim, movement: &'static str, at: Vec3, npc: bool) -> Option<Entity> {
    let p = sim.spawn_character(at + Vec3::Y, movement);
    if npc {
        sim.app.world_mut().entity_mut(p).insert(Npc);
    }
    sim.seconds(1.0);
    let feet = sim.position(p).y + sim.state(p).hull_min.y;
    if (feet - at.y).abs() > 0.3 || sim.position(p).xz().distance(at.xz()) > 0.1 {
        sim.app.world_mut().despawn(p);
        return None;
    }
    Some(p)
}

/// Whether the segment `a`..`b` meets the convex brush `brush`.
fn crosses(brush: &MapBrush, a: Vec3, b: Vec3) -> bool {
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    let d = b - a;
    for &(n, dist) in &brush.planes {
        let (start, along) = (n.dot(a) - dist, n.dot(d));
        if along.abs() < 1e-9 {
            if start > 0.0 {
                return false;
            }
        } else {
            let t = -start / along;
            if along < 0.0 {
                lo = lo.max(t);
            } else {
                hi = hi.min(t);
            }
        }
    }
    lo <= hi
}

/// Whether a walker of `movement` standing at `at` got into or through
/// the clip `b` walking along `into` for 2 s (its centre's path meets the
/// brush; sliding along it and round its end doesn't); None when it
/// couldn't stand there.
fn walks_through(sim: &mut Sim, movement: &'static str, npc: bool, at: Vec3, into: Vec3, b: &MapBrush) -> Option<bool> {
    let p = stand(sim, movement, at, npc)?;
    let mut through = false;
    let mut last = sim.position(p);
    for _ in 0..(2.0 * sim.tick_hz()) as u32 {
        let mut i = sim.intent(p);
        i.yaw = (-into.x).atan2(-into.z);
        i.pitch = 0.0;
        i.move_axis = Vec2::Y;
        sim.ticks(1);
        let at = sim.position(p);
        through |= crosses(b, last, at);
        last = at;
    }
    sim.app.world_mut().despawn(p);
    Some(through)
}

/// Each upright player-clip wall's sides on map `id` that stop CS:S's
/// movement: (sides tried, those another walker crossed). None without
/// the installs.
fn clip_walls(id: &str) -> Option<(usize, Vec<String>)> {
    let mut sim = party_on(id)?;
    let clips: Vec<MapBrush> = {
        let w = sim.app.world();
        let brushes = w.resource::<MapBrushes>();
        w.resource::<MapPlayerClips>()
            .0
            .iter()
            .filter_map(|&i| brushes.0.get(i).cloned())
            .collect()
    };
    assert!(!clips.is_empty(), "{id} has player clips");
    if std::env::var_os("CHIMERA_CLIPS_OFF").is_some() {
        // The control: no clip collider, as before the fix.
        sim.app.world_mut().insert_resource(MapPlayerClips(Vec::new()));
        sim.ticks(2);
    }
    let mut tried = 0;
    let mut failed = Vec::new();
    'walls: for (k, b) in clips.iter().enumerate() {
        let size = b.max - b.min;
        // Upright walls a player can walk into: taller than a player.
        if size.y < 2.0 {
            continue;
        }
        let thin = if size.x < size.z { Vec3::X } else { Vec3::Z };
        let centre = (b.min + b.max) / 2.0;
        for side in [1.0, -1.0] {
            if tried >= MOST_SIDES {
                break 'walls;
            }
            let out = thin * side * (size.dot(thin) / 2.0 + 1.0);
            let probe = Vec3::new(centre.x + out.x, b.min.y + 2.5, centre.z + out.z);
            let Some(floor) = floor_under(&mut sim, probe) else { continue };
            // A wall over the head from the floor: no step or jump over it.
            if floor < b.min.y - 0.1 || b.max.y < floor + 2.5 {
                continue;
            }
            let at = Vec3::new(probe.x, floor, probe.z);
            let into = -thin * side;
            // Only where CS:S stands and is kept out.
            if walks_through(&mut sim, CSS, false, at, into, b) != Some(false) {
                continue;
            }
            tried += 1;
            for (who, movement, npc) in [("HL2:DM", HL2, false), ("Combat Arms", CA, false), ("Combat Arms bot", CA, true)] {
                let got = walks_through(&mut sim, movement, npc, at, into, b);
                eprintln!("{id} clip {k} at {centre:.2} side {side}: {who} through {got:?}");
                if got == Some(true) {
                    failed.push(format!("{who} through clip {k} at {centre:.2} side {side}"));
                }
            }
        }
    }
    Some((tried, failed))
}

fn check(id: &str) {
    let Some((tried, failed)) = clip_walls(id) else { return };
    assert!(tried > 0, "{id}: some clip wall side stops CS:S's movement");
    assert!(failed.is_empty(), "{id}, of {tried} sides: {failed:#?}");
}

#[test]
fn every_movement_stays_inside_cs_militias_clip_walls() {
    check("cs_source:cs_militia");
}

#[test]
fn every_movement_stays_out_of_cs_havanas_yard() {
    check("cs_source:cs_havana");
}

/// `CHIMERA_CLIP_MAPS`' maps, when set.
#[test]
fn every_movement_stops_at_other_maps_clips() {
    let Ok(maps) = std::env::var("CHIMERA_CLIP_MAPS") else { return };
    for id in maps.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        check(id);
    }
}
