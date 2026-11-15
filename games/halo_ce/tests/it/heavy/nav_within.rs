//! Where Halo's levels are played, for the bots' navigation
//! (`game::maps::collision::nav_within`, the engine's `nav_gen::NavWithin`):
//! on Blood Gulch and Hang 'Em High the map's box is the structure BSP's
//! world bounds (structure_bsp.md's table), and a mesh generated on the
//! loaded level for a walker that climbs as the party's bots do (0.8 m
//! rises), held to it, has every area's chest point (1 m over its centre)
//! inside the level by the collision BSP's point test, in a leaf whose
//! structure leaf has a cluster (collision.md, "Point test";
//! structure_bsp.md, "Leaves"), and none above the drawn level's top;
//! held, it covers every floor an unheld flood without a cell limit
//! reaches (such a walker never leaves the drawn level on these two: the
//! bound is a guard there, and the box lets Blood Gulch's flood grow to
//! its whole floor, `NavGenParams::cell_limit`). Areas and times are
//! printed with `--nocapture`. Passes with a note without the disc image.

use avian3d::prelude::SpatialQuery;
use bevy::{ecs::system::SystemState, math::Vec3};
use blam_formats::{cache::MapFile, collision, structure_bsp::StructureBsp};
use chimera_world::{
    harness::Sim,
    map::{
        MapSimPlugin,
        nav::NavMesh,
        nav_gen::{self, NavGenParams, NavWithin},
    },
};
use halo_ce::{
    game::maps::{self, space},
    harness::HaloSim,
};

use super::maps::{load, mount};

/// The structure BSP's world bounds (structure_bsp.md, "World bounds",
/// world units: x, y, z lower and upper).
const DRAWN: &[(&str, [f32; 6])] = &[
    ("bloodgulch", [5.907, 131.902, -190.219, -45.057, -0.349, 26.233]),
    ("hangemhigh", [5.900, 35.878, -22.823, 15.823, -9.253, 8.183]),
];

fn climber(within: Option<(Vec3, Vec3)>) -> NavGenParams {
    NavGenParams {
        step: 0.8,
        radius: 0.38,
        max_drop: 3.0,
        within,
        ..Default::default()
    }
}

fn m2(m: &NavMesh) -> f32 {
    m.areas.iter().map(|a| (a.max - a.min).x * (a.max - a.min).y).sum()
}

#[test]
fn the_navigation_is_held_inside_the_drawn_level() {
    let Some(mount) = mount() else { return };
    for (name, b) in DRAWN {
        let map = load(&mount, name, false);
        let (p, q) = (space::point([b[0], b[2], b[4]]), space::point([b[1], b[3], b[5]]));
        let (lo, hi) = (p.min(q), p.max(q));
        let within = *map.extras.get::<NavWithin>().expect("the drawn level's box");
        assert!(
            within.min.distance(lo) < 0.2 && within.max.distance(hi) < 0.2,
            "{name}: {within:?} against {lo} {hi}"
        );
        let feet: Vec<Vec3> = map.spawns.iter().map(|(s, _)| *s).collect();
        let bytes = mount.read(&maps::map_path(name)).unwrap();
        let file = MapFile::load(&bytes).unwrap();
        let level = collision::read_level(&file, 0).unwrap();
        let bsp = StructureBsp::read(&file, 0).unwrap();
        let mut sim = Sim::new(HaloSim, MapSimPlugin::new(map));
        sim.ticks(2);
        let w = sim.app.world_mut();
        assert_eq!(
            NavWithin::of(w),
            Some((within.min, within.max)),
            "{name}: the loaded map's box"
        );
        let mut state = SystemState::<SpatialQuery>::new(w);
        let sq = state.get(w).unwrap();
        let t = std::time::Instant::now();
        let free = nav_gen::generate(
            &sq,
            &|_| false,
            &feet,
            &NavGenParams {
                max_cells: 4_000_000,
                ..climber(None)
            },
        );
        let free_s = t.elapsed().as_secs_f32();
        let t = std::time::Instant::now();
        let held = nav_gen::generate(&sq, &|_| false, &feet, &climber(Some((within.min, within.max))));
        let held_s = t.elapsed().as_secs_f32();
        let above = |m: &NavMesh| {
            m.areas
                .iter()
                .filter(|a| a.heights.iter().any(|y| *y > hi.y + 0.1))
                .count()
        };
        eprintln!(
            "{name}: unheld (no cell limit) {} areas {:.0} m2 ({} above the drawn top) in {free_s:.1} s; held {} areas {:.0} m2 ({} above) in {held_s:.1} s",
            free.areas.len(),
            m2(&free),
            above(&free),
            held.areas.len(),
            m2(&held),
            above(&held)
        );
        assert_eq!(above(&held), 0, "{name}: nothing above the drawn level");
        assert!(
            (m2(&held) - m2(&free)).abs() < 1.0,
            "{name}: held, every floor the walker reaches"
        );
        let mut outside = Vec::new();
        for a in &held.areas {
            let chest = a.center.with_y(a.height_at(a.center.x, a.center.z) + 1.0);
            let leaf = level.bsp.point_leaf(space::to_game(chest));
            let cluster = leaf
                .and_then(|l| bsp.leaves.get(l as usize)?.cluster)
                .filter(|c| (*c as usize) < bsp.clusters.len());
            if cluster.is_none() {
                outside.push(chest);
            }
        }
        // A few chest points of areas on a slope may graze a wall's face
        // (an area's centre height is its corners' mean).
        assert!(
            outside.len() * 200 <= held.areas.len(),
            "{name}: {} of {} areas' chests outside the level's clusters: {:?}",
            outside.len(),
            held.areas.len(),
            &outside[..outside.len().min(8)]
        );
    }
}
