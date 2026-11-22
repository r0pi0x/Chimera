//! CS:S's guns as the HUD shows them (the simulation's own tests:
//! tests/it/cs_guns.rs): switching fire mode prints the mode in the
//! centre of the screen (`ui::objectives_hud`).

use bevy::prelude::*;
use crate::stack::{
    core::LocalPlayer,
    games::cs_source::{
        TICK_INTERVAL,
        ui::objectives_hud::{BombHudStatePlugin, centre_text},
        weapons::{CsWeaponsPlugin, FAMAS, GLOCK, data},
    },
    greybox::{self, GreyboxMapPlugin},
    harness::{CsSim, Sim},
    movement::placeholder,
    weapon::{AltModes, Inventory, Weapon, give},
};

fn active(sim: &Sim, p: Entity) -> Entity {
    sim.app
        .world()
        .get::<Inventory>(p)
        .unwrap()
        .active
        .expect("no active weapon")
}

fn mode(sim: &Sim, p: Entity) -> u8 {
    sim.app.world().get::<AltModes>(active(sim, p)).unwrap().current
}

/// A shooter holding `id`, drawn (one tick to give, then the draw time).
fn shooter_with(sim: &mut Sim, id: &str) -> Entity {
    let p = sim.spawn_character(greybox::SPAWNS[0], placeholder::ID);
    sim.ticks(1);
    give(sim.app.world_mut(), p, id).unwrap();
    sim.ticks(1);
    assert_eq!(sim.app.world().get::<Weapon>(active(sim, p)).unwrap().id, id);
    let draw = data(id).unwrap().draw;
    sim.seconds(draw as f64 + 0.05);
    p
}

fn press2(sim: &mut Sim, p: Entity) {
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
}

/// Switching fire mode is silent and prints what it switched to
/// (playtest 2026-10-14, as CS:S: "Switched to burst-fire mode", back to
/// "semi-automatic" on the Glock and "automatic" on the FAMAS; the
/// install's `Cstrike_TitlesTXT_Switch_To_*`, the tokens without one).
#[test]
fn burst_switches_are_silent_and_print_the_mode() {
    #[derive(Resource, Default)]
    struct Heard(Vec<String>);
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, CsWeaponsPlugin, BombHudStatePlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app.init_resource::<Heard>().add_systems(
        Last,
        |mut a: MessageReader<crate::stack::map::PlaySound>, mut b: MessageReader<crate::stack::map::GameSound>, mut h: ResMut<Heard>| {
            h.0.extend(a.read().map(|s| s.entry.clone()));
            h.0.extend(b.read().map(|s| s.0.entry.clone()));
        },
    );
    for (id, normal) in [(GLOCK, "Switch_To_SemiAuto"), (FAMAS, "Switch_To_FullAuto")] {
        let p = shooter_with(&mut sim, id);
        sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
        sim.app.world_mut().resource_mut::<Heard>().0.clear();
        press2(&mut sim, p);
        sim.ticks(2);
        assert_eq!(mode(&sim, p), 1);
        let shown = centre_text(sim.app.world()).unwrap_or_default();
        assert!(shown.to_lowercase().contains("burst"), "{id}: {shown:?}");
        sim.seconds(0.4);
        press2(&mut sim, p);
        sim.ticks(2);
        assert_eq!(mode(&sim, p), 0);
        let shown = centre_text(sim.app.world()).unwrap_or_default();
        assert!(shown.contains(normal), "{id}: {shown:?}");
        let heard = &sim.app.world().resource::<Heard>().0;
        assert!(heard.is_empty(), "{id}: switching played {heard:?}");
        sim.app.world_mut().entity_mut(p).remove::<LocalPlayer>();
    }
}
