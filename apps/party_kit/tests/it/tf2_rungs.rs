//! TF2's guns as rungs (docs/plans/active/tf2.md, M2b): the original
//! `originals:gun_game_2fort` (minigames/gun_game_2fort.toml), a ladder
//! that mixes TF2's four guns with CS:S's, HL2:DM's and Combat Arms' (no
//! Halo), on the party's simulation headless on the greybox: a TF2 rung
//! gives its TF2 gun and makes the player TF2's (its identity and
//! movement) and the gun's class (its max health and top speed), a kill
//! with it climbs to the next rung, another game's gun, and the identity
//! follows the gun (the max health back to the party's); the Rocket
//! Launcher's rung kills too, with a rocket.

use bevy::prelude::*;
use chimera_core::{
    core::Health,
    slots::{GameIdentity, MovementSlot},
};
use chimera_sim::minigame::{self, ladder::Ladder};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::harness::CaSim;
use party_kit::PartyPartsPlugin;

use crate::gun_game::{held, line_up, no_finale, plain_rungs, rung, shoot_until_dead, soldier};

fn party() -> Sim {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    plain_rungs(sim.app.world_mut());
    no_finale(sim.app.world_mut());
    minigame::start(sim.app.world_mut(), originals::GUN_GAME_2FORT).expect("the gun game starts");
    sim
}

fn identity(w: &World, e: Entity) -> Option<String> {
    w.get::<GameIdentity>(e).map(|g| g.0.clone())
}

fn movement(w: &World, e: Entity) -> Option<&'static str> {
    w.get::<MovementSlot>(e).map(|m| m.0)
}

fn set_rung(sim: &mut Sim, e: Entity, r: usize) {
    let w = sim.app.world_mut();
    let l = w.resource::<Ladder>().clone();
    minigame::ladder::set_rung(w, e, r, &l);
    sim.seconds(1.2);
}

#[test]
fn a_tf2_rung_gives_its_gun_and_a_kill_climbs_to_the_next_games() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let tf2_rungs: Vec<usize> = (0..ladder.rungs.len()).filter(|r| ladder.rungs[*r].starts_with("tf2:")).collect();
    assert_eq!(tf2_rungs.len(), 4, "TF2's four guns");
    assert!(!ladder.rungs.iter().any(|w| w.starts_with("halo_ce:")), "no Halo");
    // The Scattergun's rung: TF2's gun, TF2's identity and movement.
    let r = tf2_rungs[0];
    set_rung(&mut sim, a, r);
    assert_eq!(held(&sim, a), Some(tf2::game::weapons::SCATTERGUN));
    let w = sim.app.world();
    assert_eq!(identity(w, a).as_deref(), Some("tf2"));
    assert_eq!(movement(w, a), Some(tf2::game::movement::ID));
    // The Scout's: 125 health, 400 units/s.
    let h = *w.get::<Health>(a).unwrap();
    assert!((h.max - 1.25).abs() < 1e-6 && (h.current - 1.25).abs() < 1e-6, "{h:?}");
    let traits = w.get::<source_engine::source_movement::SourceTraits>(a).unwrap();
    assert_eq!(traits.max_speed, Some(400.0));
    // A kill with it: the next rung, another game's gun, its identity.
    line_up(&mut sim, a, b, 3.0);
    assert!(shoot_until_dead(&mut sim, a, b, 10.0), "the Scattergun kills");
    sim.ticks(2);
    assert_eq!(rung(&sim, a), r + 1);
    let next = ladder.rungs[r + 1];
    assert!(!next.starts_with("tf2:"), "{next}");
    assert_eq!(held(&sim, a), Some(next));
    let game = next.split_once(':').unwrap().0;
    assert_eq!(identity(sim.app.world(), a).as_deref(), Some(game), "the identity follows {next}");
    let h = *sim.app.world().get::<Health>(a).unwrap();
    assert!((h.max - 1.0).abs() < 1e-6, "the party's max health back: {h:?}");
    // Each TF2 rung's gun in hand, and TF2's identity with it.
    for r in tf2_rungs {
        set_rung(&mut sim, a, r);
        let gun = ladder.rungs[r];
        assert_eq!(held(&sim, a), Some(gun));
        assert_eq!(identity(sim.app.world(), a).as_deref(), Some("tf2"), "{gun}");
    }
}

#[test]
fn the_rocket_launchers_rung_kills_with_a_rocket() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let r = ladder.rungs.iter().position(|w| *w == tf2::game::weapons::ROCKET_LAUNCHER).unwrap();
    set_rung(&mut sim, a, r);
    assert_eq!(held(&sim, a), Some(tf2::game::weapons::ROCKET_LAUNCHER));
    line_up(&mut sim, a, b, 6.0);
    assert!(shoot_until_dead(&mut sim, a, b, 12.0), "rockets kill");
    sim.ticks(2);
    assert_eq!(rung(&sim, a), r + 1);
}

/// weapons_more.md's eight guns as rungs (the brief's ladder on the
/// greybox, TF2 among the weapon games): each gives its gun and TF2's
/// identity; the Pistol, Revolver, Flame Thrower, Syringe Gun and Grenade
/// Launcher each kill.
#[test]
fn the_next_eight_tf2_guns_are_rungs_and_kill() {
    use tf2::game::weapons::{
        FLAMETHROWER, GRENADE_LAUNCHER, PISTOL_SCOUT, REVOLVER, SMG, SNIPER_RIFLE, STICKY_LAUNCHER, SYRINGE_GUN,
    };
    let guns = [PISTOL_SCOUT, SMG, SNIPER_RIFLE, REVOLVER, GRENADE_LAUNCHER, STICKY_LAUNCHER, FLAMETHROWER, SYRINGE_GUN];
    let text = crate::hl2dm_parts::ladder_text(&guns, false)
        .replace("\"hl2dm\", \"combat_arms\"]", "\"hl2dm\", \"tf2\", \"combat_arms\"]");
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    crate::gun_game::no_finale(sim.app.world_mut());
    use chimera_sim::minigame::MinigameAppExt;
    sim.app.add_minigames(&[("tf2_more.toml", &text)]).unwrap();
    minigame::start(sim.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    for (r, gun) in guns.iter().enumerate() {
        set_rung(&mut sim, a, r);
        assert_eq!(held(&sim, a), Some(*gun));
        assert_eq!(identity(sim.app.world(), a).as_deref(), Some("tf2"), "{gun}");
    }
    for gun in [GRENADE_LAUNCHER, PISTOL_SCOUT, REVOLVER, FLAMETHROWER, SYRINGE_GUN] {
        let r = guns.iter().position(|g| *g == gun).unwrap();
        set_rung(&mut sim, a, r);
        line_up(&mut sim, a, b, 3.0);
        // The Syringe Gun's rung is the last: its kill wins the match.
        assert!(shoot_until_dead(&mut sim, a, b, 12.0), "{gun} kills");
    }
}
