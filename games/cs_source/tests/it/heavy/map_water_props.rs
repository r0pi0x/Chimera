//! de_aztec's oil drums in its canal (`map::buoyancy`), on the user's
//! install (skipped without one): the drums the map places in the water
//! float with their weight of water under the surface (mass over 1000
//! kg/m³ of their hull) and are still within a few seconds, rather than
//! riding on top and bobbing for ever (the 2026-10-28 playtest). Prints
//! each drum's figures (`--nocapture`).

use avian3d::prelude::*;
use bevy::prelude::*;
use crate::stack::{
    core::MapWater,
    games::{
        self, cs_source,
        cs_source::movement::{SourceMovementPlugin, to_source},
    },
    harness::{CsSim, Sim},
    map::{
        MapPlugin, PhysicsProp,
        buoyancy::{WATER_DENSITY, float_line, shape_hulls, submerged},
    },
    mount::config::LocalConfig,
};

#[test]
fn de_aztec_s_drums_float_at_their_depth_and_settle() {
    let installed = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref()))
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    if !installed {
        eprintln!("skipping: no CS:S install configured");
        return;
    }
    let map = games::load_map_lean("cs_source:de_aztec").expect("load de_aztec");
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map), SourceMovementPlugin));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    sim.ticks(2);
    let water: Vec<(Vec3, Vec3)> = sim.app.world().resource::<MapWater>().0.iter().map(|v| (v.brush.min, v.brush.max)).collect();
    let bodies: Vec<Entity> = {
        let w = sim.app.world_mut();
        let mut v: Vec<Entity> = w
            .query_filtered::<(Entity, &RigidBody), With<PhysicsProp>>()
            .iter(w)
            .filter(|(_, r)| r.is_dynamic())
            .map(|(e, _)| e)
            .collect();
        v.sort();
        v
    };
    // The water surface over a body (None: not over water).
    let surface_at = |p: Vec3| {
        water
            .iter()
            .filter(|(lo, hi)| p.x >= lo.x && p.x <= hi.x && p.z >= lo.z && p.z <= hi.z && p.y < hi.y + 1.0)
            .map(|(_, hi)| hi.y)
            .fold(None, |a: Option<f32>, b| Some(a.map_or(b, |a| a.max(b))))
    };
    let tick = cs_source::TICK_INTERVAL;
    let mut tracks: Vec<Vec<f32>> = vec![Vec::new(); bodies.len()];
    for _ in 0..(8.0 / tick) as usize {
        sim.ticks(1);
        for (i, e) in bodies.iter().enumerate() {
            tracks[i].push(sim.app.world().get::<Position>(*e).unwrap().0.y);
        }
    }
    let mut floating = 0;
    for (i, &e) in bodies.iter().enumerate() {
        let w = sim.app.world();
        let (pos, rot) = (w.get::<Position>(e).unwrap().0, w.get::<Rotation>(e).unwrap().0);
        let Some(surface) = surface_at(pos) else { continue };
        let hulls = shape_hulls(w.get::<Collider>(e).unwrap().shape_scaled()).expect("a hull");
        let place = |p: Vec3| pos + rot * p;
        let under = submerged(&hulls, place, surface).0;
        if under <= 0.0 {
            continue;
        }
        floating += 1;
        let mass = w.get::<ComputedMass>(e).unwrap().value();
        // How far it is from floating at its weight of water (units, +:
        // too deep).
        let off = (surface - float_line(&hulls, place, mass / WATER_DENSITY)) / 0.0254;
        let tail = &tracks[i][tracks[i].len() - (1.0 / tick) as usize..];
        let swing = (tail.iter().fold(f32::MIN, |a, b| a.max(*b)) - tail.iter().fold(f32::MAX, |a, b| a.min(*b))) / 0.0254;
        eprintln!(
            "drum at {:.0}: {mass:.1} kg, {under:.4} m³ under (its weight's worth {:.4}: {off:.2} units deeper), \
             last second's swing {swing:.3} units, asleep {}",
            to_source(pos),
            mass / WATER_DENSITY,
            w.get::<Sleeping>(e).is_some()
        );
        assert!(off.abs() < 0.5, "floats {off} units from its depth");
        assert!(swing < 0.1, "still bobbing: {swing} units");
    }
    assert!(floating >= 4, "{floating} drums in the water");
}
