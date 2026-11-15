//! Halo's maps in the party (the disc image, `$CHIMERA_HALO_CE_ROOT` or
//! `[games.halo_ce] path`; docs/plans/active/halo-ce.md, M1): every map of
//! Halo's pool (the disc's multiplayer maps its data/map_pool.toml allows,
//! `auto::report` over the installed maps) loads through the party's map
//! sources with spawns for free-for-all and, where the map has capture the
//! flag starts, for both of Combat Arms' teams; and on Hang 'Em High a
//! Combat Arms soldier (another game's body, movement and gun) stands on
//! the floor, walks a few metres without falling through or sticking, and
//! a hitscan round at a wall hits the map with a surface Combat Arms'
//! impact table knows (Halo's `metal`, its Metal), its impact drawn there.
//! Every pool map's spawn points reach each other on the bots' mesh with
//! its teleporters as links. Each test passes with a note without the
//! disc image.

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::core::{SOLID_LAYERS, SpawnPoint};
use chimera_sim::{minigame::auto, testkit};
use chimera_world::{
    harness::Sim,
    map::{MapLoadOptions, MapSimPlugin, SurfaceNames, sound::SurfaceGrid},
};
use combat_arms::{
    game::{
        effects::{self, data::CaFxTables, sounds::CaSound},
        movement,
        rules::state::{ALPHA, BRAVO},
    },
    harness::CaSim,
};
use party_kit::PartyPartsPlugin;

fn disc() -> bool {
    let there = halo_ce::game::INFO.mount().is_some();
    if !there {
        eprintln!("skipping: no Halo CE disc image ($CHIMERA_HALO_CE_ROOT or [games.halo_ce] path)");
    }
    there
}

/// Halo's maps in the party's pool, as the apps find them.
fn halo_pool() -> Vec<String> {
    let mut sim = Sim::new(CaSim, PartyPartsPlugin);
    let w = sim.app.world_mut();
    auto::find_installed_maps(w);
    auto::report(w).into_iter().find(|c| c.game == halo_ce::game::GAME).map(|c| c.maps).unwrap_or_default()
}

#[test]
fn every_halo_pool_map_loads_with_spawns_for_both_teams_and_free_for_all() {
    if !disc() {
        return;
    }
    let pool = halo_pool();
    eprintln!("Halo's pool: {pool:?}");
    for must in ["halo_ce:hangemhigh", "halo_ce:bloodgulch"] {
        assert!(pool.iter().any(|m| m == must), "{must} in the pool: {pool:?}");
    }
    for id in &pool {
        let map = party_kit::load_map_with(id, &MapLoadOptions::lean()).unwrap_or_else(|e| panic!("{id}: {e}"));
        let of = |t: Option<u8>| map.spawns.iter().filter(|(_, team)| team.map(|t| t.0) == t).count();
        let (free, alpha, bravo) = (of(None), of(Some(ALPHA)), of(Some(BRAVO)));
        eprintln!("{id}: {} spawns: {free} without a team, {alpha} Alpha, {bravo} Bravo", map.spawns.len());
        assert_eq!(free + alpha + bravo, map.spawns.len(), "{id}: no other team");
        assert!(map.spawns.len() >= 8, "{id}: free-for-all takes every spawn");
        assert!(alpha > 0 && bravo > 0, "{id}: both teams' bases");
        assert_eq!(map.impacts.game.as_deref(), Some(halo_ce::game::GAME));
        assert!(!map.surface_names.is_empty(), "{id}: its surfaces as Combat Arms'");
    }
}

/// Every Halo pool map's spawn points are mutually reachable on the bots'
/// generated mesh (`spot_checks::bots_mesh`, Combat Arms' walker, held to
/// the drawn level), through its own links: walks and the map's
/// teleporters (`nav_gen::link_map`, the graph the bots plan on): a map
/// cut in parts only its teleporters join measures as one.
#[test]
fn every_halo_pool_maps_spawns_reach_each_other_through_its_teleporters() {
    use party_kit::spot_checks::{bots_mesh, region, spawn_feet};
    if !disc() {
        return;
    }
    let mut failures = Vec::new();
    for id in halo_pool() {
        let mut sim = party_on(&id);
        let w = sim.app.world_mut();
        let feet = spawn_feet(w);
        let nav = bots_mesh(w, &feet);
        let links = nav.teleports.len();
        let (inside, starts) = region(&nav, &feet);
        let off = starts.iter().filter(|s| !s.is_some_and(|a| inside[a])).count();
        eprintln!("{id}: {} spawns, {} areas, {links} teleporter links, {off} spawns off the main region", feet.len(), nav.areas.len());
        if off > 0 {
            failures.push(format!("{id}: {off} of {} spawns off the main region", feet.len()));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The party on Halo map `id` (as `impacts.rs` sets it up).
fn party_on(id: &str) -> Sim {
    let map = party_kit::load_map(id, 0).unwrap_or_else(|e| panic!("{id}: {e}"));
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
    sim.ticks(2);
    sim
}

/// The first point of the static world `reach` m or less from `from`
/// along one of eight level directions, at least `min` m away.
fn level_hit(sim: &mut Sim, p: Entity, from: Vec3, min: f32, reach: f32, clear: bool) -> Option<Vec3> {
    let mut state: SystemState<SpatialQuery> = SystemState::new(sim.app.world_mut());
    let spatial = state.get(sim.app.world()).unwrap();
    let filter = SpatialQueryFilter::from_excluded_entities([p]).with_mask(SOLID_LAYERS);
    (0..8).find_map(|i| {
        let a = i as f32 * std::f32::consts::FRAC_PI_4;
        let dir = Dir3::new(Vec3::new(a.cos(), 0.0, a.sin())).ok()?;
        match spatial.cast_ray(from, dir, reach, true, &filter) {
            // A way clear for `min` m: the point `min` along it.
            None if clear => Some(from + *dir * min),
            Some(hit) if clear && hit.distance > min + 1.0 => Some(from + *dir * min),
            // A wall between `min` and `reach`.
            Some(hit) if !clear && hit.distance > min => Some(from + *dir * hit.distance),
            _ => None,
        }
    })
}

#[test]
fn a_combat_arms_soldier_walks_and_shoots_on_hang_em_high() {
    if !disc() {
        return;
    }
    let mut sim = party_on("halo_ce:hangemhigh");
    let at = {
        let w = sim.app.world_mut();
        let mut q = w.query::<(&Transform, &SpawnPoint)>();
        q.iter(w).next().map(|(t, _)| t.translation).expect("a spawn point")
    };
    let p = sim.spawn_character(at, movement::ID);
    sim.seconds(1.5);
    // Standing on the floor at the spawn.
    let stood = sim.position(p);
    assert!(sim.state(p).on_ground, "on the ground at {stood}");
    assert!((stood.xz() - at.xz()).length() < 0.5 && stood.y > at.y - 1.0, "stood at {stood}, spawned at {at}");
    // Walking a few metres along a clear way.
    let chest = stood + Vec3::Y * 0.5;
    let goal = level_hit(&mut sim, p, chest, 5.0, 12.0, true).expect("a way clear for 6 m");
    testkit::aim_at(&mut sim, p, goal);
    sim.intent(p).pitch = 0.0;
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.seconds(1.0);
    let walked = sim.position(p);
    let moved = (walked.xz() - stood.xz()).length();
    eprintln!("walked from {stood} to {walked}: {moved:.2} m");
    assert!(moved > 2.5, "walked {moved:.2} m: stuck?");
    assert!(sim.state(p).on_ground && (walked.y - stood.y).abs() < 1.5, "on the floor after the walk: {walked}");
    // A Combat Arms gun's round at a wall: its impact there, on a surface
    // its table knows.
    chimera_sim::weapon::give(sim.app.world_mut(), p, "combat_arms:m4a1");
    sim.seconds(1.5);
    let eye = sim.position(p) + sim.state(p).eye_offset;
    let wall = level_hit(&mut sim, p, eye, 2.0, 25.0, false).expect("a wall within 25 m");
    let surface = {
        let w = sim.app.world();
        effects::world_surface_named(w.get_resource::<SurfaceGrid>(), w.get_resource::<SurfaceNames>(), wall)
    };
    assert_eq!(surface, CaFxTables::built_in().surface_id("Metal").unwrap(), "Hang 'Em High's metal as Combat Arms' Metal");
    #[derive(Resource, Default)]
    struct Heard(Vec<CaSound>);
    sim.app.init_resource::<Heard>().add_systems(Last, |mut r: MessageReader<CaSound>, mut h: ResMut<Heard>| {
        h.0.extend(r.read().cloned());
    });
    testkit::aim_at(&mut sim, p, wall);
    sim.ticks(2);
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    sim.seconds(0.5);
    let heard = &sim.app.world().resource::<Heard>().0;
    assert!(
        heard.iter().any(|s| s.at.is_some_and(|a| a.distance(wall) < 0.5)),
        "Combat Arms' impact at the wall {wall}: {:?}",
        heard.iter().map(|s| (&s.file, s.at)).collect::<Vec<_>>()
    );
}

/// Halo's teleporters move another game's characters: a Combat Arms
/// soldier and a CS:S player, each set down on every source pad of Chill
/// Out (3 pairs) and Wizard (4), arrive at the pair's target, standing on
/// the ground there, their velocity kept and their look turned by the
/// target's facing minus the source's.
#[test]
fn other_games_characters_use_halos_teleporters() {
    use chimera_world::map::teleport::MapTeleporters;
    if !disc() {
        return;
    }
    let mut failures = Vec::new();
    for id in ["halo_ce:chillout", "halo_ce:wizard"] {
        let mut sim = party_on(id);
        let t = sim.app.world().resource::<MapTeleporters>().clone();
        for movement in [movement::ID, "cs_source:movement"] {
            for (i, pad) in t.pads.iter().enumerate() {
                let p = sim.spawn_character(pad.source + Vec3::Y * 1.2, movement);
                let yaw = sim.intent(p).yaw;
                let mut arrived = false;
                for _ in 0..40 {
                    sim.ticks(1);
                    if (sim.position(p) - pad.dest).xz().length() < 0.5 {
                        arrived = true;
                        break;
                    }
                }
                let turned = sim.intent(p).yaw;
                sim.seconds(1.0);
                let at = sim.position(p);
                let ground = sim.state(p).on_ground;
                let want = (yaw + pad.dest_yaw - pad.source_yaw).rem_euclid(std::f32::consts::TAU);
                let turn_ok = (turned - want).abs() < 1e-3 || (turned - want).abs() > std::f32::consts::TAU - 1e-3;
                eprintln!("{id} {movement} pad {i}: arrived {arrived}, at {at} (target {}), on the ground {ground}, turned {turn_ok}", pad.dest);
                if !(arrived && ground && turn_ok && (at - pad.dest).xz().length() < 1.0 && (at.y - pad.dest.y).abs() < 2.0) {
                    failures.push(format!("{id} {movement} pad {i}: arrived {arrived}, at {at}, target {}, ground {ground}, turned {turn_ok}", pad.dest));
                }
                sim.app.world_mut().despawn(p);
                sim.ticks(1);
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
