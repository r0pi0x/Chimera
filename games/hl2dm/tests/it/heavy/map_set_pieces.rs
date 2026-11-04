//! Each stock map's set pieces as HL2:DM's game plays them (specs/hl2dm/
//! maps_entities.md 0, 5, 8-16): no entity class of the spec's census left
//! without a handler, the respawnable props (329), dm_resistance's 7
//! flares, the ragdoll managers, dm_underpass's camera model, and
//! dm_runoff's strike (the second one fires the six canisters its template
//! spawned) and dm_lockdown's fan seen by its sensor. Converted lean from
//! the install; skips without one.

use bevy::prelude::*;
use chimera_core::core::SimClock;
use chimera_world::{
    harness::Sim,
    map::{MapData, MapLoadOptions, MapSimPlugin},
};
use hl2dm::{
    game::{STOCK_MAPS, set_pieces::RagdollLimit},
    harness::Hl2Sim,
};
use source_logic::logic::{Logic, Value, classes::Class};

use super::map_stock::{convert, install_mount};

/// Classes that may stay without a logic class of their own: the game's
/// (pickups, chargers, ladders, spawns, canisters, the manager), the
/// loader's (props, lights, sounds, ropes, the sky and fog) and the
/// engine's (visibility, tone map), as the census (maps_entities.md 0)
/// lists them.
fn passive(class: &str) -> bool {
    class.starts_with("item_")
        || class.starts_with("weapon_")
        || class.starts_with("info_")
        || class.starts_with("light")
        || class.starts_with("env_soundscape")
        || [
            "worldspawn",
            "func_useableladder",
            "func_clip_vphysics",
            "func_illusionary",
            "func_wall",
            "func_lod",
            "func_occluder",
            "func_areaportal",
            "func_brush",
            "game_ragdoll_manager",
            "env_headcrabcanister",
            "env_fog_controller",
            "env_tonemap_controller",
            "env_sun",
            "shadow_control",
            "sky_camera",
            "water_lod_control",
            "keyframe_rope",
            "move_rope",
            "ambient_generic",
            "infodecal",
            "point_spotlight",
            "prop_static",
            "npc_combine_camera",
            "func_breakable_surf",
        ]
        .contains(&class)
}

fn sim_of(map: MapData) -> Sim {
    let mut sim = Sim::new(Hl2Sim, MapSimPlugin::new(map));
    sim.set_tick_interval(0.015);
    sim
}

fn fire(sim: &mut Sim, name: &str) {
    sim.app
        .world_mut()
        .resource_mut::<Logic>()
        .world
        .queue_input(name, "Trigger", Value::Void, 0.0, None);
}

fn count_fired(sim: &Sim, output: &str) -> usize {
    sim.app.world().resource::<Logic>().world.fired.iter().filter(|(_, _, o)| o == output).count()
}

#[test]
fn every_class_of_the_census_is_handled() {
    let Some(mount) = install_mount() else { return };
    // Respawnable props, ragdoll limits per map (maps_entities.md 0, 12).
    let respawnable = [80, 14, 68, 14, 45, 50, 23, 35];
    let mut total = 0;
    for (name, want) in STOCK_MAPS.iter().zip(respawnable) {
        let data = convert(&mount, name, &MapLoadOptions::lean());
        let mut sim = sim_of(data);
        sim.seconds(2.0);
        let logic = sim.app.world().resource::<Logic>();
        let mut unhandled = std::collections::BTreeMap::<String, usize>::new();
        let mut respawn = 0;
        for id in logic.world.ids() {
            let e = logic.world.get(id).unwrap();
            let class = e.classname.to_ascii_lowercase();
            if matches!(e.class, Class::None) && !passive(&class) {
                *unhandled.entry(class.clone()).or_default() += 1;
            }
            if logic.world.prop(id).is_some_and(|p| p.respawn.is_some()) {
                respawn += 1;
            }
        }
        eprintln!("{name}: {respawn} respawnable props, unhandled {unhandled:?}");
        assert!(unhandled.is_empty(), "{name}: {unhandled:?}");
        assert_eq!(respawn, want, "{name}: respawnable props");
        total += respawn;
        let ragdolls = sim.app.world().resource::<RagdollLimit>().current();
        let expect = if ["dm_lockdown", "dm_powerhouse"].contains(name) { 4 } else { 8 };
        assert_eq!(ragdolls, expect, "{name}: ragdoll limit");
    }
    assert_eq!(total, 329);
}

#[test]
fn flares_and_the_camera_are_drawn() {
    let Some(mount) = install_mount() else { return };
    let resistance = convert(&mount, "dm_resistance", &MapLoadOptions::default());
    let flares = resistance
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.classname() == "env_flare")
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    assert_eq!(flares.len(), 7);
    let drawn = resistance.sprites.iter().filter(|s| s.entity.is_some_and(|e| flares.contains(&e))).count();
    assert_eq!(drawn, 14, "a red flare and a white core each");
    let underpass = convert(&mount, "dm_underpass", &MapLoadOptions::default());
    // 692 props before the ceiling camera stood in as a model.
    assert_eq!(underpass.props.len(), 693, "the ceiling camera's model");
}

/// dm_runoff: the first strike fires nothing (its template removed the
/// map's canisters), the second fires the six it spawned, and they land 4 s
/// after.
#[test]
fn runoff_strike() {
    let Some(mount) = install_mount() else { return };
    let mut sim = sim_of(convert(&mount, "dm_runoff", &MapLoadOptions::lean()));
    sim.app.world_mut().resource_mut::<Logic>().world.record = true;
    sim.seconds(1.0);
    fire(&mut sim, "explosion_sequence");
    // The relay is locked until its longest delay (10 s) has passed.
    sim.seconds(12.0);
    assert_eq!(count_fired(&sim, "OnLaunched"), 0, "the first strike: no canisters");
    fire(&mut sim, "explosion_sequence");
    sim.seconds(1.0);
    assert_eq!(count_fired(&sim, "OnLaunched"), 6);
    assert_eq!(count_fired(&sim, "OnImpacted"), 0);
    sim.seconds(3.5);
    assert_eq!(count_fired(&sim, "OnImpacted"), 6, "together with the killers, 4 s on");
    let _ = sim.app.world().resource::<SimClock>().tick;
}

/// dm_lockdown's fan: the motor ramps it to 1500 deg/s over a second and
/// holds it there (the fan, hinged to the world with "No Collision",
/// passes through its housing and the static props in it) until
/// `fan_disable` 5 s on; the sensor fires `OnGreaterThan` once and
/// `OnLessThan` after the motor stops.
#[test]
fn lockdown_fan_is_watched() {
    let Some(mount) = install_mount() else { return };
    let mut sim = sim_of(convert(&mount, "dm_lockdown", &MapLoadOptions::lean()));
    sim.app.world_mut().resource_mut::<Logic>().world.record = true;
    sim.seconds(1.0);
    sim.app
        .world_mut()
        .resource_mut::<Logic>()
        .world
        .queue_input("fan_enable", "Trigger", Value::Void, 0.0, None);
    let spin = |sim: &Sim| {
        let l = sim.app.world().resource::<Logic>();
        let fan = l.world.find("fan").expect("the fan");
        l.world.prop(fan).map_or(0.0, |p| p.spin.length())
    };
    // On at +0.1 s, at speed from about +1.1 s.
    sim.seconds(1.5);
    let mut held = 0;
    let mut low = f32::MAX;
    for _ in 0..30 {
        sim.seconds(0.1);
        let v = spin(&sim);
        low = low.min(v);
        held += usize::from(v > 1450.0);
    }
    eprintln!("fan from 1.5 s to 4.5 s: {held} of 30 samples above 1450, lowest {low}");
    // A healthkit lying in the fan's way gets batted: a dip of a tick
    // (the sensor's 0.2 s interval lets it pass).
    assert!(held >= 28, "the fan holds its speed: {held} of 30 above 1450 (lowest {low})");
    assert_eq!(count_fired(&sim, "OnGreaterThan"), 1, "the sensor saw it pass 1400");
    assert_eq!(count_fired(&sim, "OnLessThan"), 0);
    // `fan_disable` at +5 s: the fan coasts down past 1400.
    sim.seconds(3.0);
    assert!(spin(&sim) < 1400.0);
    assert_eq!(count_fired(&sim, "OnLessThan"), 1, "and saw it drop below");
}
