//! DoD:S's rungs in the Ultimate Gun Game (docs/plans/active/dods.md, M3b):
//! a DoD:S gun's rung makes its holder DoD:S's with the body of the class
//! carrying it (classes.md, "Loadouts"; `dods::game::classes`), DoD:S's
//! movement and DoD:S's HUD; a gun of several classes takes its rung's drawn
//! class (`RungClass`), else its own; leaving DoD:S for another game's gun
//! takes the body and the HUD off. On the party's simulation, headless.

use bevy::prelude::*;
use chimera_core::{
    console::Console,
    core::LocalPlayer,
    gameplay::Gameplay,
    slots::{GameIdentity, MovementSlot},
};
use chimera_sim::minigame::{self, ladder::Ladder};
use chimera_world::{greybox, harness::Sim, map::BodyName};
use combat_arms::game::movement;
use dods::game::classes::{self, RungClass};
use ultimate_gun_game::sandbox::SANDBOX;

use crate::parts::ugg;

/// The sandbox on the greybox with the local player `me` in it.
fn play() -> (Sim, Entity) {
    let mut sim = ugg();
    let w = sim.app.world_mut();
    greybox::unload(w);
    greybox::select(w, "greybox");
    greybox::respawn(w);
    sim.ticks(2);
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(1.5);
    (sim, me)
}

fn rung(sim: &mut Sim, gun: &str) -> String {
    let before = sim.app.world().resource::<Console>().output.len();
    sim.app.world_mut().resource_mut::<Console>().submit(format!("rung {gun}"));
    sim.seconds(1.0);
    sim.app.world().resource::<Console>().output[before..].iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
}

fn body(sim: &Sim, e: Entity) -> Option<String> {
    sim.app.world().get::<BodyName>(e).map(|n| n.0.clone())
}

fn hud(sim: &Sim) -> &'static str {
    sim.app.world().resource::<Gameplay>().hud
}

#[test]
fn a_dods_rung_gives_the_class_body_movement_and_hud() {
    let (mut sim, me) = play();
    for (gun, model) in [
        ("dods:weapon_mg42", "models/player/german_mg.mdl"),
        ("dods:weapon_bazooka", "models/player/american_rocket.mdl"),
        ("dods:weapon_k98_scoped", "models/player/german_sniper.mdl"),
    ] {
        let said = rung(&mut sim, gun);
        assert_eq!(sim.app.world().get::<GameIdentity>(me).map(|i| i.0.to_string()).as_deref(), Some("dods"), "{gun}: {said}");
        assert_eq!(sim.app.world().get::<MovementSlot>(me).map(|m| m.0), Some("dods:movement"), "{gun}");
        assert_eq!(body(&sim, me).as_deref(), Some(model), "{gun}'s class body");
        assert_eq!(hud(&sim), "dods", "{gun}: DoD:S's HUD");
    }
    // Another game's gun: the class body and DoD:S's HUD go.
    let said = rung(&mut sim, "cs_source:weapon_ak47");
    assert_ne!(hud(&sim), "dods", "{said}");
    let name = body(&sim, me).unwrap_or_default();
    assert!(!name.starts_with("models/player/german") && !name.starts_with("models/player/american"), "{name}");
}

#[test]
fn a_shared_gun_wears_its_rungs_drawn_class() {
    let (mut sim, me) = play();
    const P38: &str = "dods:weapon_p38";
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let at = ladder.rungs.iter().position(|r| *r == P38).expect("the P38's rung");
    let drawn = classes::drawn_class(ladder.extras(at)).expect("a class drawn").to_string();
    let table = classes::classes();
    rung(&mut sim, P38);
    assert_eq!(body(&sim, me).as_deref(), Some(table.get(&drawn).unwrap().model()), "the drawn {drawn}");
    // Another of the P38's classes drawn: its body.
    let other = ["dods:axis_assault", "dods:axis_sniper", "dods:axis_mg"].into_iter().find(|c| *c != drawn).unwrap();
    sim.app.world_mut().entity_mut(me).insert(RungClass(other.into()));
    sim.seconds(0.5);
    assert_eq!(body(&sim, me).as_deref(), Some(table.get(other).unwrap().model()), "{other}");
    // A drawn class the gun can't be: the gun's own.
    sim.app.world_mut().entity_mut(me).insert(RungClass("dods:us_mg".into()));
    sim.seconds(0.5);
    assert_eq!(body(&sim, me).as_deref(), Some("models/player/german_assault.mdl"));
}

/// The ladder's DoD:S rungs each drew a class their gun can be, with that
/// class's melee weapon (the knife, the spade; the Assault classes none).
#[test]
fn the_ladders_dods_rungs_draw_their_classes() {
    let (sim, _) = play();
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let table = classes::classes();
    let mut seen = 0;
    for (i, gun) in ladder.rungs.iter().enumerate().filter(|(_, g)| g.starts_with("dods:")) {
        let Some(drawn) = classes::drawn_class(ladder.extras(i)) else { continue };
        seen += 1;
        assert!(table.of_gun_all(gun).iter().any(|c| c.id == drawn), "{gun} drew {drawn}");
        assert_eq!(ladder.extras(i).unwrap().melee, table.get(drawn).unwrap().melee, "{gun}'s melee");
    }
    assert!(seen > 0, "no DoD:S rung drew a class: {:?}", ladder.rungs);
}
