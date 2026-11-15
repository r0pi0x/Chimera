//! Halo's guns pulled as Combat Arms' bots pull a trigger (pressed every
//! other engine tick) under a party app's 64 Hz tick, and their rounds as
//! the engine's `Magazine` shows them to such a bot (apps/party_kit's
//! `heavy::halo_gun_bots` measures the bots themselves):
//!
//! - a release between two Halo ticks is a release (`weapons::systems`,
//!   `release_between_ticks`): the sniper rifle (a trigger that does not
//!   repeat) fires again and again, where every Halo tick saw the latched
//!   press and it fired once; the plasma pistol lets its charge go, where
//!   it charged and held the bolt;
//! - the plasma rifle and pistol (a battery, no magazine of rounds) carry a
//!   `Magazine` whose rounds are the battery's (`systems::battery_rounds`):
//!   rounds while they may fire, none while overheated, where they had no
//!   `Magazine` and the bots took them for empty and never fired.

use bevy::prelude::*;
use chimera_sim::weapon::{Magazine, WeaponEvent, WeaponEventKind, give};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use halo_ce::{
    game::{
        movement,
        weapons::{HaloGun, PLASMA_PISTOL, PLASMA_RIFLE, SNIPER_RIFLE, tick::TriggerMode},
    },
    harness::HaloSim,
};

/// A Spartan holding `id` under a 64 Hz tick, its draw over.
fn armed_at_64(id: &str) -> (Sim, Entity, Entity) {
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    sim.set_tick_interval(1.0 / 64.0);
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.ticks(2);
    let g = give(sim.app.world_mut(), p, id).unwrap();
    sim.seconds(5.0);
    (sim, p, g)
}

/// The rounds `p` fired over `ticks` engine ticks with the trigger pressed
/// every other one, and the longest run of ticks its gun stayed charged.
fn tapped(sim: &mut Sim, p: Entity, g: Entity, ticks: u32) -> (usize, u32) {
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let (mut rounds, mut charged, mut longest) = (0, 0, 0);
    for t in 0..ticks {
        sim.intent(p).fire = t % 2 == 0;
        sim.ticks(1);
        rounds += cursor
            .read(sim.app.world().resource::<Messages<WeaponEvent>>())
            .filter(|e| e.owner == p && matches!(e.kind, WeaponEventKind::Action("fire" | "fire_last" | "fire_charged")))
            .count();
        let gun = sim.app.world().get::<HaloGun>(g).unwrap();
        charged = if gun.trigger == TriggerMode::Charged { charged + 1 } else { 0 };
        longest = longest.max(charged);
    }
    (rounds, longest)
}

#[test]
fn the_sniper_rifle_tapped_every_other_engine_tick_fires_again() {
    let (mut sim, p, g) = armed_at_64(SNIPER_RIFLE);
    // 3 s: its four rounds and a reload's worth.
    let (rounds, _) = tapped(&mut sim, p, g, 192);
    assert!(rounds >= 4, "{rounds} rounds in 3 s");
}

#[test]
fn the_plasma_pistol_tapped_every_other_engine_tick_lets_its_charge_go() {
    let (mut sim, p, g) = armed_at_64(PLASMA_PISTOL);
    let (rounds, longest) = tapped(&mut sim, p, g, 192);
    assert!(rounds >= 6, "{rounds} bolts in 3 s");
    assert!(longest < 32, "held charged for {longest} ticks");
}

#[test]
fn the_battery_weapons_rounds_are_their_batterys() {
    for id in [PLASMA_RIFLE, PLASMA_PISTOL] {
        let (mut sim, p, g) = armed_at_64(id);
        let m = sim.app.world().get::<Magazine>(g).cloned().unwrap_or_else(|| panic!("{id}: a Magazine"));
        assert!(m.clip > 0 && m.clip <= m.size && m.reserve == 0, "{id}: {m:?}");
        // Held (the rifle) or tapped (the pistol) until it overheats: no
        // rounds then.
        let mut overheated = false;
        for t in 0..64 * 20 {
            sim.intent(p).fire = id == PLASMA_RIFLE || t % 2 == 0;
            sim.ticks(1);
            let w = sim.app.world();
            if w.get::<HaloGun>(g).unwrap().overheated {
                // The magazine follows on the next Halo tick at the latest.
                sim.intent(p).fire = false;
                sim.ticks(3);
                let w = sim.app.world();
                if w.get::<HaloGun>(g).unwrap().overheated {
                    assert_eq!(w.get::<Magazine>(g).unwrap().clip, 0, "{id}: rounds while overheated");
                    overheated = true;
                    break;
                }
            }
        }
        assert!(overheated, "{id} never overheated");
        let after = sim.app.world().get::<Magazine>(g).unwrap().clip;
        assert!(after < m.size, "{id}: the battery used: {after} of {}", m.size);
    }
}
