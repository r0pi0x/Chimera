//! Halo's grenades in the party apps (`halo_ce::game::weapons::grenades::
//! PartyGrenades`, the console's `halo_party_grenades`, registered by
//! Halo's party bundle), on `halo_ladder`'s mixed ladder: by default a
//! player on a Halo rung carries no frag and no plasma grenades (on its
//! first Halo rung, after a rung change to another game's gun and back,
//! and after a respawn), its grenade type stays none (the HUD draws no
//! grenade plate), and the grenade and rotate-grenades buttons throw and
//! change nothing; with `halo_party_grenades 1` a new life carries Halo's
//! four frags (two players: the cap is 4) and the button throws one; back
//! at 0 the grenades are gone at once. Halo's own simulation never
//! registers the switch (its players keep their grenades). And a Halo
//! zoom keeps its holder's view in the party (Combat Arms' zoom step
//! leaves a gun's own zoom alone).

use bevy::prelude::*;
use chimera_core::{
    console,
    core::ActionRegistry,
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use halo_ce::game::weapons::{
    HaloHands,
    grenades::{GRENADE_ACTION, PartyGrenades, SWITCH_ACTION},
};

use crate::{
    gun_game::{alive, line_up, shoot_until_dead, soldier},
    halo_ladder::{party, set_rung},
};

fn hands(sim: &Sim, e: Entity) -> HaloHands {
    *sim.app.world().get::<HaloHands>(e).expect("Halo's hands")
}

fn command(sim: &mut Sim, line: &str) {
    let words: Vec<String> = line.split(' ').map(str::to_string).collect();
    console::execute(sim.app.world_mut(), &words, 0);
}

/// Hold the grenade and the rotate-grenades buttons for `secs`, pressing
/// each anew every few ticks.
fn press_both(sim: &mut Sim, e: Entity, secs: f32) {
    let reg = sim.app.world().resource::<ActionRegistry>().clone();
    let bits = reg.bit(GRENADE_ACTION) | reg.bit(SWITCH_ACTION);
    assert!(reg.bit(GRENADE_ACTION) != 0 && reg.bit(SWITCH_ACTION) != 0, "the buttons are registered");
    for i in 0..(f64::from(secs) * sim.tick_hz()) as u32 {
        let down = (i / 4) % 2 == 0;
        let mut intent = sim.intent(e);
        intent.actions = if down { intent.actions | bits } else { intent.actions & !bits };
        sim.ticks(1);
    }
    sim.intent(e).actions &= !bits;
}

/// The thrown grenades in flight (Halo's projectiles of the grenades).
fn grenades_flying(sim: &mut Sim) -> usize {
    let w = sim.app.world_mut();
    use halo_ce::game::weapons::systems::{FRAG_GRENADE, PLASMA_GRENADE};
    w.query::<&halo_ce::game::weapons::HaloProjectile>().iter(w).filter(|p| p.id == FRAG_GRENADE || p.id == PLASMA_GRENADE).count()
}

#[test]
fn a_halo_rung_carries_no_grenades_by_default_and_the_switch_gives_them_back() {
    let mut sim = party();
    assert_eq!(sim.app.world().get_resource::<PartyGrenades>(), Some(&PartyGrenades(false)), "off by default");
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    // The first Halo rung (the assault rifle).
    set_rung(&mut sim, a, 1);
    sim.seconds(1.2);
    let h = hands(&sim, a);
    assert!(h.armed, "a Halo life");
    assert_eq!(h.throw.counts, [0, 0], "no grenades on a Halo rung");
    assert_eq!(h.throw.selected(), None, "no grenade type: no plate on the HUD");
    // The buttons do nothing.
    press_both(&mut sim, a, 1.5);
    let h = hands(&sim, a);
    assert_eq!((h.throw.counts, h.throw.frame, h.throw.selected()), ([0, 0], None, None), "nothing thrown or switched");
    assert!(!h.grenade_button && !h.switch_button);
    assert_eq!(grenades_flying(&mut sim), 0);
    // Another game's rung and back to Halo's (the pistol).
    set_rung(&mut sim, a, 2);
    sim.seconds(0.6);
    set_rung(&mut sim, a, 3);
    sim.seconds(1.2);
    assert_eq!(hands(&sim, a).throw.counts, [0, 0], "none after a rung change");
    // A respawn: shot dead by the other player (CS:S's Glock), back on
    // its Halo rung.
    set_rung(&mut sim, b, 0);
    sim.seconds(0.6);
    line_up(&mut sim, b, a, 3.0);
    assert!(shoot_until_dead(&mut sim, b, a, 15.0), "killed");
    for _ in 0..40 {
        sim.seconds(0.25);
        if alive(&sim, a) && hands(&sim, a).armed {
            break;
        }
    }
    assert!(alive(&sim, a) && hands(&sim, a).armed, "a new Halo life");
    assert_eq!(hands(&sim, a).throw.counts, [0, 0], "none after a respawn");
    set_rung(&mut sim, a, 1);
    sim.seconds(1.2);
    assert_eq!(hands(&sim, a).throw.counts, [0, 0], "none on the next Halo rung after the respawn");

    // On: a new life carries Halo's frags, and the button throws one.
    command(&mut sim, "halo_party_grenades 1");
    assert_eq!(sim.app.world().get_resource::<PartyGrenades>(), Some(&PartyGrenades(true)));
    sim.app.world_mut().get_mut::<HaloHands>(a).unwrap().armed = false;
    sim.seconds(0.5);
    let h = hands(&sim, a);
    assert_eq!(h.throw.counts, [4, 0], "Halo's four frags: {h:?}");
    let reg = sim.app.world().resource::<ActionRegistry>().clone();
    sim.intent(a).actions |= reg.bit(GRENADE_ACTION);
    sim.seconds(0.6);
    sim.intent(a).actions &= !reg.bit(GRENADE_ACTION);
    assert_eq!(hands(&sim, a).throw.counts, [3, 0], "one frag thrown");
    assert_eq!(hands(&sim, a).throw.selected(), Some(0), "the frag plate");
    // Off again: gone at once.
    command(&mut sim, "halo_party_grenades 0");
    sim.seconds(1.5);
    assert_eq!(hands(&sim, a).throw.counts, [0, 0], "taken away");
    assert_eq!(hands(&sim, a).throw.selected(), None);
}

#[test]
fn halos_own_simulation_has_no_party_switch() {
    let mut sim = Sim::new(halo_ce::harness::HaloSim, GreyboxMapPlugin);
    sim.ticks(2);
    assert!(sim.app.world().get_resource::<PartyGrenades>().is_none(), "Halo's own app keeps its grenades");
}

/// A Halo zoom keeps its view in the party: Combat Arms' zoom step (the
/// party's base game) took the holder's `Zoomed` off every tick for any
/// gun not its own and without the engine's `Zoom`, so a Halo sniper's
/// scope showed its mask over an unzoomed camera with the gun in view; it
/// now leaves a gun's own zoom (`weapon::OwnZoom`) alone.
#[test]
fn a_halo_zoom_keeps_its_holders_view_in_the_party() {
    use chimera_sim::weapon::{Inventory, Zoomed, give};
    let mut sim = party();
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    set_rung(&mut sim, a, 1);
    sim.seconds(1.0);
    let w = sim.app.world_mut();
    let sniper = give(w, a, halo_ce::game::weapons::SNIPER_RIFLE).expect("the sniper rifle");
    w.get_mut::<Inventory>(a).unwrap().active = Some(sniper);
    sim.seconds(1.5);
    sim.intent(a).secondary = true;
    sim.seconds(0.2);
    sim.intent(a).secondary = false;
    sim.seconds(0.5);
    let z = sim.app.world().get::<Zoomed>(a).copied();
    assert!(z.is_some_and(|z| z.fov < 40.0), "zoomed: {z:?}");
}
