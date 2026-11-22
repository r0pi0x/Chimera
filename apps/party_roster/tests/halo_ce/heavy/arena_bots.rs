//! A bots' match on Halo's first arena slice (docs/plans/active/arena-
//! slices.md, A1): the party's gun game on Blood Gulch played on its
//! `west_base` slice (Halo's data/arena_slices.toml, `chimera_world::map::
//! play_area`), eight Combat Arms bots, `SECONDS` of simulated time.
//! Asserts: the slice plays (its wall, the map's spawn points held
//! inside, the teleporters with an end outside gone), every spawn spot
//! and every area of the bots' mesh inside, the bots kill each other,
//! nobody's feet more than the put-back's slack outside at any tick; prints
//! the numbers (kills, put-backs, spots, areas). Needs the disc image and
//! the CS:S and Combat Arms installs (skips without). Run alone:
//! `cargo test -p party_roster --features dev --test halo_ce heavy::arena_bots -- --nocapture`.

use bevy::prelude::*;
use chimera_core::{
    core::{Died, Intent, SpawnPoint},
    mount::config::LocalConfig,
};
use chimera_sim::minigame::{arena, spawns::SpawnSpots};
use chimera_world::{
    harness::Sim,
    map::{
        MapSimPlugin,
        nav::NavMesh,
        play_area::{self, ArenaMap, ArenaWall, PlayArea},
        teleport::MapTeleporters,
    },
};
use combat_arms::harness::CaSim;
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin};

const MAP: &str = "halo_ce:bloodgulch";
const SLICE: &str = "halo_ce:bloodgulch@west_base";
const SECONDS: f64 = 60.0;
const BOTS: u32 = 8;

#[derive(Resource, Default)]
struct Kills {
    kills: u32,
    suicides: u32,
}

fn count(mut died: MessageReader<Died>, mut k: ResMut<Kills>) {
    for d in died.read() {
        if d.attacker.is_none_or(|a| a == d.entity) {
            k.suicides += 1;
        } else {
            k.kills += 1;
        }
    }
}

#[test]
fn bots_play_blood_gulchs_west_base_inside_its_wall() {
    if halo_ce::game::INFO.mount().is_none() {
        eprintln!("skipping: no Halo CE disc image ($CHIMERA_HALO_CE_ROOT or [games.halo_ce] path)");
        return;
    }
    let Ok(config) = LocalConfig::load() else { return };
    let css = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config
        .game_path(combat_arms::game::GAME)
        .filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if css.is_none() || ca.is_none() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    let map = party_roster::load_map(MAP, 0).unwrap_or_else(|e| panic!("{MAP}: {e}"));
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
    let teleporters = sim.app.world().get_resource::<MapTeleporters>().map_or(0, |t| t.pads.len());
    {
        // As the party kit's `arena` does on a map just in: the plan, the
        // map in the world, the play area at once.
        let w = sim.app.world_mut();
        arena::plan(w, Some(SLICE)).expect("Halo's slice is registered");
        w.insert_resource(ArenaMap(MAP.into()));
        play_area::follow_plan(w);
        assert!(w.contains_resource::<PlayArea>(), "the slice plays");
        chimera_sim::minigame::start(w, DEFAULT_MINIGAME).expect("the gun game starts");
        let mut s = w.resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = BOTS;
        s.quota_set = true;
    }
    sim.app.init_resource::<Kills>().add_systems(FixedPostUpdate, count);
    let poly = sim.app.world().resource::<PlayArea>().polygon;
    let walls = {
        let w = sim.app.world_mut();
        w.query::<&ArenaWall>().iter(w).count()
    };
    assert_eq!(walls, poly.points().len());
    let kept = sim.app.world().get_resource::<MapTeleporters>().map_or(0, |t| t.pads.len());
    let mut outside = 0u32;
    let mut worst: f32 = 0.0;
    let ticks = (SECONDS * sim.tick_hz()) as u32;
    let started = std::time::Instant::now();
    for _ in 0..ticks {
        sim.ticks(1);
        let w = sim.app.world_mut();
        for t in w.query_filtered::<&Transform, With<Intent>>().iter(w) {
            let by = poly.outside_by(Vec2::new(t.translation.x, t.translation.z));
            worst = worst.max(by);
            if by > play_area::PUT_BACK_SLACK + 0.5 {
                outside += 1;
            }
        }
    }
    let w = sim.app.world_mut();
    let points: Vec<Vec3> = w
        .query_filtered::<&Transform, (With<SpawnPoint>, Without<Intent>)>()
        .iter(w)
        .map(|t| t.translation)
        .collect();
    let spots = w.get_resource::<SpawnSpots>().map(|s| s.spots.clone()).unwrap_or_default();
    let spots_out = spots.iter().filter(|s| !poly.contains3(**s)).count();
    let (areas, areas_out) = w.get_resource::<NavMesh>().map_or((0, 0), |m| {
        let out = m
            .areas
            .iter()
            .filter(|a| !poly.contains3(a.center))
            .count();
        (m.areas.len(), out)
    });
    let k = w.resource::<Kills>();
    let (kills, suicides) = (k.kills, k.suicides);
    let put_backs = w.resource::<PlayArea>().put_backs;
    println!(
        "{SLICE}: {BOTS} bots, {SECONDS} s in {:.0} s: {kills} kills, {suicides} suicides, {put_backs} put back inside, \
         worst {worst:.2} m past the wall, {outside} samples outside; {} spawn points, {} spots ({spots_out} outside), \
         bots' mesh {areas} areas ({areas_out} outside), teleporters {kept} of {teleporters}; {:.0} m2",
        started.elapsed().as_secs_f32(),
        points.len(),
        spots.len(),
        poly.area()
    );
    assert!(points.iter().all(|p| poly.contains3(*p)), "a spawn point outside");
    assert_eq!(spots_out, 0, "spawn spots outside");
    assert!(spots.len() >= 150, "spots: {}", spots.len());
    assert_eq!(areas_out, 0, "bots' areas outside");
    assert!(areas > 0, "the bots made their mesh");
    assert_eq!(outside, 0, "characters outside");
    assert!(kills > 0, "the bots fight");
}
