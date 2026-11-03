//! Stock maps' physics props in the gravity gun (specs/hl2dm/
//! gravity_gun.md 2-7): on dm_lockdown and dm_overwatch, converted lean
//! from the install, the props the gun can hold (at most 250 kg, not
//! pinned) are counted, and a few of the light, small ones, put in front of a
//! player at a spawn in open space, are grabbed, come to the hold point,
//! are launched at 1500 units/s and punted at the cap or the punt's speed
//! for their mass. Skips without the install.

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::MovementState;
use chimera_world::{
    harness::Sim,
    map::{MapLoadOptions, MapSimPlugin, PhysicsProp, held::Held, prop_physics::FrozenBody},
};
use hl2dm::{
    game::{
        movement,
        weapons::{
            self,
            launch::{self, U, to_source},
            physcannon,
        },
    },
    harness::Hl2Sim,
};

use super::map_stock::{convert, install_mount};

fn press(sim: &mut Sim, p: Entity, fire: bool, secondary: bool, ticks: u64) {
    {
        let mut i = sim.intent(p);
        i.fire = fire;
        i.secondary = secondary;
    }
    sim.ticks(ticks);
    let mut i = sim.intent(p);
    i.fire = false;
    i.secondary = false;
}

/// A yaw (radians) from `eye` with 200 units of open space ahead.
fn open_yaw(sim: &mut Sim, me: Entity, eye: Vec3) -> Option<f32> {
    for k in 0..16 {
        let yaw = k as f32 * std::f32::consts::TAU / 16.0;
        let dir = chimera_sim::weapon::aim_of(yaw, 0.0) * Vec3::NEG_Z;
        let world = sim.app.world_mut();
        let mut state = bevy::ecs::system::SystemState::<SpatialQuery>::new(world);
        let q = state.get(world).unwrap();
        let filter = SpatialQueryFilter::from_excluded_entities([me]).with_mask(chimera_core::core::SOLID_LAYERS);
        if q.cast_ray(eye, Dir3::new(dir).ok()?, 200.0 * U, true, &filter).is_none() {
            return Some(yaw);
        }
    }
    None
}

#[test]
fn stock_props_in_the_gravity_gun() {
    let Some(mount) = install_mount() else { return };
    let p: physcannon::PhysParams = launch::hook_params(weapons::PHYSCANNON, weapons::PHYSCANNON_HOOK);
    for name in ["dm_lockdown", "dm_overwatch"] {
        let data = convert(&mount, name, &MapLoadOptions::lean());
        let spawns: Vec<Vec3> = data.spawns.iter().map(|s| s.0).collect();
        let mut sim = Sim::new(Hl2Sim, MapSimPlugin::new(data));
        sim.set_tick_interval(0.015);
        sim.seconds(3.0);
        // (Each with its largest side, units.)
        let props: Vec<(Entity, f32, bool, f32)> = {
            let w = sim.app.world_mut();
            w.query::<(Entity, Option<&PhysicsProp>, Option<&FrozenBody>)>()
                .iter(w)
                .filter_map(|(e, prop, frozen)| match (prop, frozen) {
                    (Some(p), _) => Some((e, p.mass, false, (p.bounds.1 - p.bounds.0).max_element() / U)),
                    (None, Some(f)) => Some((e, f.physics.mass, true, (f.bounds.1 - f.bounds.0).max_element() / U)),
                    _ => None,
                })
                .collect()
        };
        let holdable = props.iter().filter(|(_, m, pinned, _)| !pinned && *m <= p.max_mass).count();
        eprintln!("{name}: {} physics props, {holdable} the gun can hold", props.len());
        assert!(holdable > 10, "{name}: {holdable} holdable props");

        // A player at a spawn with open space ahead.
        let (player, yaw) = spawns
            .iter()
            .find_map(|s| {
                let pl = sim.spawn_character(*s, movement::ID);
                sim.seconds(0.5);
                let eye = sim.position(pl) + sim.app.world().get::<MovementState>(pl).unwrap().eye_offset;
                match open_yaw(&mut sim, pl, eye) {
                    Some(y) => Some((pl, y)),
                    None => {
                        sim.app.world_mut().despawn(pl);
                        None
                    }
                }
            })
            .expect("a spawn with open space");
        {
            let mut i = sim.intent(player);
            i.yaw = yaw;
            i.pitch = 0.0;
        }
        sim.seconds(0.5);
        let mut light: Vec<(Entity, f32)> = props
            .iter()
            // Light and small (a big board launched at eye height can
            // catch the floor or a wall on its first tick).
            .filter(|(_, m, pinned, side)| !pinned && *m >= 2.0 && *m <= 40.0 && *side <= 40.0)
            .map(|(e, m, ..)| (*e, *m))
            .collect();
        light.truncate(3);
        assert!(!light.is_empty(), "{name}: no light props");
        for (prop, mass) in light {
            let eye = sim.position(player) + sim.app.world().get::<MovementState>(player).unwrap().eye_offset;
            let fwd = chimera_sim::weapon::aim_of(yaw, 0.0) * Vec3::NEG_Z;
            let put = |sim: &mut Sim, at: Vec3| {
                let w = sim.app.world_mut();
                w.get_mut::<Transform>(prop).unwrap().translation = at;
                if let Some(mut pos) = w.get_mut::<Position>(prop) {
                    pos.0 = at;
                }
                if let Some(mut v) = w.get_mut::<LinearVelocity>(prop) {
                    v.0 = Vec3::ZERO;
                }
            };
            // Grabbed from 120 units, held near the eye, launched at 1500.
            put(&mut sim, eye + fwd * 120.0 * U);
            sim.ticks(1);
            press(&mut sim, player, false, true, 2);
            assert!(sim.app.world().get::<Held>(prop).is_some(), "{name}: a {mass} kg prop grabbed");
            sim.seconds(1.0);
            let d = to_source(sim.position(prop) - eye).length();
            assert!(d < 120.0, "{name}: {mass} kg held {d} units from the eye");
            press(&mut sim, player, true, false, 1);
            let v = sim.app.world().get::<LinearVelocity>(prop).unwrap().0.length() / U;
            assert!((v - 1500.0).abs() < 60.0, "{name}: a {mass} kg launch at {v}");
            sim.seconds(1.0);
            // A breakable one may have broken where it landed.
            if sim.app.world().get::<RigidBodyDisabled>(prop).is_some() {
                eprintln!("{name}: the {mass} kg prop broke on landing");
                continue;
            }
            // Punted from 80 units: the cap below 10.7 kg, else the formula.
            put(&mut sim, eye + fwd * 80.0 * U);
            sim.ticks(1);
            press(&mut sim, player, true, false, 1);
            let v = sim.app.world().get::<LinearVelocity>(prop).unwrap().0.length() / U;
            let want = physcannon::punt_speed(&p, mass).min(2000.0);
            eprintln!("{name}: {mass} kg punted at {v} (want about {want})");
            assert!(v > want * 0.6 && v < 2010.0, "{name}: a {mass} kg punt at {v}, want about {want}");
            sim.seconds(1.0);
        }
    }
}
