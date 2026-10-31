//! The Combat Arms bots' generated navigation mesh on CS:S maps that ship
//! none, bounded to the rooms a walking player reaches (`map::nav_gen`,
//! `Bounds`; backlog 2026-10-28, "Bots' generated nav reaches outside
//! de_aztec"): on de_aztec nothing of it is in the jungle behind the
//! walls (the user's spawn spot there is far from it), and bots playing
//! three minutes never walk there; on de_aztec, de_dust2, cs_office and
//! de_port no floor a player walks is lost, and the area counts before
//! and after are printed (`--nocapture`). Passes with a note without both
//! installs.

use std::collections::HashSet;

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::{
    core::{Dead, SpawnPoint},
    mount::config::LocalConfig,
};
use chimera_sim::minigame::spawns;
use chimera_world::{
    harness::Sim,
    map::{
        MapSimPlugin, SPAWN_LIFT,
        nav::{NavArea, NavMesh},
        nav_gen,
        vis::{ActiveVisibility, MapVisibility},
    },
};
use combat_arms::{game::bots::CaBot, harness::CaSim};
use party_kit::{DEFAULT_MINIGAME, PartyPartsPlugin};

/// The gun game on `map` with `bots` Combat Arms bots; None without both
/// installs.
fn party_on(map: &str, bots: u32) -> Option<Sim> {
    let config = LocalConfig::load().ok()?;
    let css = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config.game_path(combat_arms::game::GAME).filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if css.is_none() || ca.is_none() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return None;
    }
    let map = party_kit::load_map(map, 0).unwrap();
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
    chimera_sim::minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    let mut s = sim.app.world_mut().resource_mut::<combat_arms::game::bots::BotSettings>();
    s.quota = bots;
    s.quota_set = true;
    Some(sim)
}

/// The meshes of the loaded map, generated from its spawn points: a
/// walking player's, the bots' walker's unbounded and bounded.
struct Meshes {
    vis: std::sync::Arc<MapVisibility>,
    walk: NavMesh,
    free: NavMesh,
    bound: NavMesh,
}

fn meshes(sim: &mut Sim) -> Meshes {
    let w = sim.app.world_mut();
    let feet: Vec<Vec3> = w
        .query_filtered::<&Transform, With<SpawnPoint>>()
        .iter(w)
        .map(|t| t.translation - Vec3::Y * SPAWN_LIFT)
        .collect();
    let vis = w.get_resource::<ActiveVisibility>().expect("a CS:S map's visibility").0.clone();
    let mut state = SystemState::<SpatialQuery>::new(w);
    let q = state.get(w).unwrap();
    let p = combat_arms::game::bots::brain::nav_params();
    let bounds = nav_gen::Bounds {
        vis: &vis,
        walker: nav_gen::walking_player(),
    };
    Meshes {
        walk: nav_gen::generate(&q, &|_| false, &feet, &spawns::walker()),
        free: nav_gen::generate(&q, &|_| false, &feet, &p),
        bound: nav_gen::generate_within(&q, &|_| false, &feet, &p, Some(bounds)),
        vis,
    }
}

/// Points of `a` half a metre apart (its cells' centres), on its floor.
fn samples(a: &NavArea) -> impl Iterator<Item = Vec3> + '_ {
    let n = ((a.max - a.min) / 0.5).round().as_uvec2();
    (0..n.x).flat_map(move |i| {
        (0..n.y).map(move |k| {
            let (x, z) = (a.min.x + 0.25 + 0.5 * i as f32, a.min.y + 0.25 + 0.5 * k as f32);
            Vec3::new(x, a.height_at(x, z), z)
        })
    })
}

fn size(areas: &[&NavArea]) -> f32 {
    areas.iter().map(|a| (a.max - a.min).x * (a.max - a.min).y).sum()
}

fn covers(m: &NavMesh, p: Vec3) -> bool {
    m.areas.iter().any(|a| a.overlaps(p.x, p.z) && (a.height_at(p.x, p.z) - p.y).abs() < 0.6)
}

/// The jungle: places in clusters a walking player never enters, more
/// than 3 m from the floors it walks.
struct Outside {
    vis: std::sync::Arc<MapVisibility>,
    clusters: HashSet<u32>,
    walk: Vec<(Vec2, Vec2)>,
}

impl Outside {
    fn of(m: &Meshes) -> Self {
        let clusters = m
            .walk
            .areas
            .iter()
            .flat_map(samples)
            .filter_map(|p| m.vis.cluster_at(p + Vec3::Y))
            .collect();
        let walk = m.walk.areas.iter().map(|a| (a.min, a.max)).collect();
        Self {
            vis: m.vis.clone(),
            clusters,
            walk,
        }
    }

    fn contains(&self, feet: Vec3) -> bool {
        let in_walked = self.vis.cluster_at(feet + Vec3::Y).is_some_and(|c| self.clusters.contains(&c));
        let near = self.walk.iter().any(|(lo, hi)| {
            let d = (*lo - feet.xz()).max(feet.xz() - *hi).max(Vec2::ZERO);
            d.length() <= 3.0
        });
        !in_walked && !near
    }

    fn areas<'a>(&self, m: &'a NavMesh) -> Vec<&'a NavArea> {
        m.areas.iter().filter(|a| samples(a).any(|p| self.contains(p))).collect()
    }
}

#[test]
fn the_bots_mesh_and_bots_stay_inside_de_aztec() {
    let Some(mut sim) = party_on("cs_source:de_aztec", 4) else { return };
    let m = meshes(&mut sim);
    let outside = Outside::of(&m);
    let (before, after) = (outside.areas(&m.free), outside.areas(&m.bound));
    eprintln!(
        "de_aztec: the jungle unbounded {} areas {:.0} m2, bounded {} areas {:.0} m2",
        before.len(),
        size(&before),
        after.len(),
        size(&after)
    );
    assert!(size(&before) > 2000.0, "the unbounded mesh reaches the jungle");
    assert!(after.is_empty(), "bounded: {:?}", after.iter().map(|a| a.center).collect::<Vec<_>>());
    // The user's spawn spot (setpos 2519.69 2283.46 210.62): on the
    // unbounded mesh, far from the bounded one.
    let spot = Vec3::new(2519.69, 210.62, -2283.46) * 0.0254;
    assert!(m.free.areas.iter().any(|a| a.overlaps(spot.x, spot.z)));
    let near = m.bound.areas.iter().map(|a| {
        let d = (a.min - spot.xz()).max(spot.xz() - a.max).max(Vec2::ZERO);
        d.length()
    });
    assert!(near.fold(f32::MAX, f32::min) > 10.0, "the user's spot is off the bounded mesh");

    // Bots playing: the mesh they make (from the spawn points and the
    // characters) is bounded too, and they never stand in the jungle.
    sim.seconds(8.0);
    {
        let nav = sim.app.world().resource::<NavMesh>();
        assert!(nav.generated, "the bots' generated mesh");
        eprintln!("de_aztec: the bots' own mesh {} areas", nav.areas.len());
        assert!(nav.areas.len() < m.free.areas.len() * 3 / 4, "bounded: {} areas", nav.areas.len());
        assert!(outside.areas(nav).is_empty(), "the bots' own mesh is in the jungle");
    }
    let (mut seen, mut out) = (0, Vec::new());
    for _ in 0..180 {
        sim.seconds(1.0);
        let w = sim.app.world_mut();
        for (t, dead) in w.query_filtered::<(&Transform, Has<Dead>), With<CaBot>>().iter(w) {
            if dead {
                continue;
            }
            seen += 1;
            let feet = t.translation - Vec3::Y * 0.9;
            if outside.contains(feet) {
                out.push(feet);
            }
        }
    }
    eprintln!("de_aztec: {seen} bot positions over three minutes, {} in the jungle", out.len());
    assert!(seen > 400, "{seen} bot positions");
    assert!(out.is_empty(), "bots in the jungle: {out:?}");
}

#[test]
fn bounding_keeps_every_floor_a_player_walks() {
    for map in ["cs_source:de_aztec", "cs_source:de_dust2", "cs_source:cs_office", "cs_source:de_port"] {
        let Some(mut sim) = party_on(map, 0) else { return };
        let m = meshes(&mut sim);
        let lost = m.walk.areas.iter().filter(|a| !covers(&m.bound, a.center)).count();
        let all = |n: &NavMesh| size(&n.areas.iter().collect::<Vec<_>>());
        eprintln!(
            "{map}: walking {} areas {:.0} m2; the bots' unbounded {} areas {:.0} m2, bounded {} areas {:.0} m2; \
             walked areas lost {lost}",
            m.walk.areas.len(),
            all(&m.walk),
            m.free.areas.len(),
            all(&m.free),
            m.bound.areas.len(),
            all(&m.bound)
        );
        assert_eq!(lost, 0, "{map}: a floor a player walks is off the bounded mesh");
        assert!(all(&m.bound) <= all(&m.free) + 1.0);
        assert!(all(&m.bound) >= all(&m.walk), "{map}: the jump-ups add to the walk");
    }
}
