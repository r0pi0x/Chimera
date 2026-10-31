//! JUNK_FLEA2's kill boxes on the user's install (specs/combat_arms/
//! world_dat.md "Moving world models and volumes": a volume's `Hidden`
//! is switched by `ON` / `OFF`). A playtest at `setpos 290.11 120.87
//! 58.06` (beside the drop point of the map's scripted delivery) died a
//! step forward: `Box_Kill01` and `Sub_Box_Kill01` are `EndlessFall`
//! volumes (10⁸/s, no pending time) at floor level that the map's
//! `StartupCommand` switches off as it starts (`msg Box_Kill01 off`) and
//! a trigger switches on for half a second. Passes with a note without
//! `[games.combat_arms] path` in chimera.local.toml.

use bevy::prelude::*;
use chimera_core::{
    core::{Damage, Health},
    mount::{Mount, config::LocalConfig},
};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::{
    game::{
        GAME, INFO,
        movement::{self, movement_data},
        volumes::{CaVolume, volumes_near},
        world,
    },
    harness::CaSim,
};

fn mount() -> Option<Mount> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some((INFO.open)(&install).unwrap())
}

/// Every damage message, for the report.
#[derive(Resource, Default)]
struct Hurt(Vec<Damage>);

fn record(mut r: MessageReader<Damage>, mut hurt: ResMut<Hurt>) {
    hurt.0.extend(r.read().cloned());
}

/// The kill boxes start switched off; a player put at the playtest's
/// `setpos` (CS:S inches, z up: the feet) stands 10 s, then walks 2 s
/// each way (along the playtest's yaw 49.15°, then a quarter turn at a
/// time) and back: no damage at all.
#[test]
fn junk_flea2_kill_boxes_start_off() {
    let Some(mount) = mount() else { return };
    let Some(map) = world::find(&mount, "junk_flea2").and_then(|_| world::load_world(&mount, "junk_flea2").ok())
    else {
        eprintln!("skipping: junk_flea2 not readable");
        return;
    };
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.app.init_resource::<Hurt>().add_systems(Last, record);
    sim.ticks(2);
    {
        let w = sim.app.world_mut();
        let mut q = w.query::<(&Name, &CaVolume)>();
        let boxes: Vec<(String, bool)> = q
            .iter(w)
            .filter(|(n, _)| n.as_str().contains("Box_Kill"))
            .map(|(n, v)| (n.as_str().to_string(), v.hidden))
            .collect();
        assert_eq!(boxes.len(), 2, "{boxes:?}");
        assert!(boxes.iter().all(|(_, hidden)| *hidden), "switched off at the start: {boxes:?}");
    }
    // setpos's feet: CS:S inches, (x, y, z) z up.
    let feet = Vec3::new(290.11, 58.06, -120.87) * 0.0254;
    let s = movement_data().meters_per_unit;
    let p = sim.spawn_character(feet + Vec3::Y * 60.0 * s, movement::ID);
    sim.seconds(10.0);
    let centre = sim.position(p);
    eprintln!("{}", volumes_near(sim.app.world_mut(), centre, 100.0));
    for turn in [0.0, 0.0, 180.0, 90.0, 180.0, 270.0, 0.0] {
        let i = &mut *sim.intent(p);
        // setang's yaw (CS:S degrees) as the intent's.
        i.yaw = (49.15f32 + turn - 90.0).to_radians();
        i.move_axis = Vec2::Y;
        sim.seconds(2.0);
        eprintln!("walked toward {}: centre {}", 49.15 + turn, sim.position(p) / s);
    }
    let hurt = &sim.app.world().resource::<Hurt>().0;
    let health = sim.app.world().get::<Health>(p).unwrap().current;
    assert!(hurt.is_empty() && health == 1.0, "health {health}, damage {:?}", hurt.first());
}
