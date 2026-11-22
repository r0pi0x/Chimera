//! The gun game's Combat Arms rungs' extras (`combat_arms::game::rungs`
//! on Chimera's `ladder::RungExtrasHook`): on the party's simulation, a
//! Combat Arms rung gives everyone on it the same fitted parts on its
//! gun, the same character and the same melee weapon (in place of the
//! kit's M9), again after a respawn; CS:S's rungs draw a player model (`cs_source::game::rungs`) and keep
//! their knife and the players' own looks; another seed draws others;
//! each option off gives today's (the M9, one's own look, the gun's own
//! parts); over the network a client's player wears the server's look
//! and parts.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Hitgroup};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_sim::{
    minigame::{
        self, MinigameAppExt, Minigames,
        ladder::{self, Ladder, RungSeed},
    },
    testkit,
    weapon::{Inventory, Weapon, data::WeaponCatalog},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        attachments::{CaFitted, CaModTable},
        characters::{CaRungLook, records::Selection},
        inventory::KNIFE as CA_KNIFE,
        rungs,
    },
    harness::CaSim,
};
use cs_source::game::rungs as css;
use chimera_core::core::PlayerClass;
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin};

use crate::play::{alive, rung, soldier};

/// The ladder's M4A1 rung (it accepts parts without the install).
const M4: usize = 7;

/// The gun game with `[rules]` lines `extra` added (as `kit:variant`),
/// started on seed `seed`.
fn party_with(extra: &str, seed: u64) -> Sim {
    party_with_app(extra, seed, |_| {})
}

/// `party_with`, `setup` run on the app before the start.
fn party_with_app(extra: &str, seed: u64, setup: impl FnOnce(&mut App)) -> Sim {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    sim.app.insert_resource(RungSeed(seed));
    setup(&mut sim.app);
    let text = sim.app.world().resource::<Minigames>().text(DEFAULT_MINIGAME).unwrap().to_string();
    let text = text
        .replace(&format!("id = \"{DEFAULT_MINIGAME}\""), "id = \"originals:variant\"")
        .replace("\nrung_extras = true\n", "\n")
        .replace("[rules]\n", &format!("[rules]\n{extra}\n"));
    sim.app.add_minigames(&[("variant.toml", &text)]).unwrap();
    crate::play::no_finale(sim.app.world_mut());
    minigame::start(sim.app.world_mut(), "originals:variant").expect("it starts");
    sim
}

fn ladder_of(sim: &Sim) -> Ladder {
    sim.app.world().resource::<Ladder>().clone()
}

/// Both players in, put on `rung`.
fn two_on(sim: &mut Sim, rung: usize) -> (Entity, Entity) {
    let a = soldier(sim);
    let b = soldier(sim);
    sim.seconds(1.5);
    let l = ladder_of(sim);
    for p in [a, b] {
        ladder::set_rung(sim.app.world_mut(), p, rung, &l);
    }
    sim.seconds(1.0);
    (a, b)
}

fn carried(sim: &Sim, p: Entity) -> Vec<&'static str> {
    let w = sim.app.world();
    w.get::<Inventory>(p).unwrap().weapons.iter().filter_map(|x| w.get::<Weapon>(*x)).map(|x| x.id).collect()
}

/// The melee weapon `p` carries.
fn melee(sim: &Sim, p: Entity) -> Option<&'static str> {
    let catalog = sim.app.world().resource::<WeaponCatalog>();
    carried(sim, p).into_iter().find(|id| catalog.get(id).is_some_and(|w| w.melee.is_some() || w.has_hook(combat_arms::game::inventory::KNIFE_HOOK)))
}

/// The fitted set of `p`'s weapon `id`.
fn fitted(world: &World, p: Entity, id: &str) -> Option<CaFitted> {
    let inv = world.get::<Inventory>(p)?;
    inv.weapons.iter().find(|w| world.get::<Weapon>(**w).is_some_and(|x| x.id == id)).and_then(|w| world.get::<CaFitted>(*w)).copied()
}

/// The record IDs of the parts rung `rung` names, by slot.
fn rung_parts(sim: &Sim, rung: usize) -> Vec<u16> {
    let l = ladder_of(sim);
    let table = sim.app.world().resource::<CaModTable>();
    let parts = l.extras(rung).and_then(|x| x.value(rungs::PARTS)).unwrap_or_default();
    parts.split(',').filter(|p| !p.is_empty()).filter_map(|p| table.find(p)).collect()
}

fn look(sim: &Sim, p: Entity) -> Option<Selection> {
    sim.app.world().get::<Selection>(p).copied()
}

fn kill(sim: &mut Sim, p: Entity, by: Entity) {
    let at = sim.position(p);
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: Some(by),
        amount: 1000.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(2);
}

#[test]
fn a_combat_arms_rung_gives_everyone_on_it_the_same_parts_character_and_melee() {
    // The odds tuned so the M4A1 always gets a sight (the stock odds
    // often keep every slot stock).
    let mut sim = party_with("rung_extras = { attachments = { sight = 1.0 } }", 3);
    let l = ladder_of(&sim);
    let x = l.extras(M4).expect("a Combat Arms rung draws").clone();
    let gun = l.rungs[M4];
    assert_eq!(gun, "combat_arms:m4a1");
    let model: i32 = x.value(rungs::MODEL).expect("a character").parse().unwrap();
    let knife = x.melee.clone().expect("a melee weapon");
    let parts = rung_parts(&sim, M4);
    assert!(!parts.is_empty(), "the tuned odds fit a sight to the M4A1: {x:?}");
    let (a, b) = two_on(&mut sim, M4);
    for p in [a, b] {
        assert_eq!(testkit::active_id(sim.app.world(), p), Some(gun));
        assert_eq!(look(&sim, p).map(|s| s.model), Some(model), "the rung's character");
        assert_eq!(melee(&sim, p), Some(knife.as_str()), "the rung's melee weapon: {:?}", carried(&sim, p));
        let f = fitted(sim.app.world(), p, gun).expect("fitted");
        for id in &parts {
            assert!(f.slots.contains(&Some(*id)), "part {id} fitted: {f:?}");
        }
    }
    let fa = fitted(sim.app.world(), a, gun).unwrap();
    assert_eq!(Some(fa), fitted(sim.app.world(), b, gun), "the same fitted set");
    assert_eq!(look(&sim, a), look(&sim, b), "the same look");
    // A respawn on the rung: the same again (a's kill takes it up one).
    kill(&mut sim, b, a);
    assert!(!alive(&sim, b));
    sim.seconds(4.0);
    assert!(alive(&sim, b));
    assert_eq!(sim.app.world().get::<ladder::Rung>(b).map(|r| r.0), Some(M4));
    assert_eq!(look(&sim, b).map(|s| s.model), Some(model));
    assert_eq!(melee(&sim, b), Some(knife.as_str()));
    assert_eq!(fitted(sim.app.world(), b, gun), Some(fa));
    // The killer's next rung (the M60) has its own.
    let next = l.extras(M4 + 1).unwrap();
    assert_eq!(melee(&sim, a), next.melee.as_deref());
}

#[test]
fn css_rungs_keep_their_knife_and_combat_arms_own_look() {
    let mut sim = party_with("", 3);
    let l = ladder_of(&sim);
    for (i, w) in l.rungs.iter().enumerate() {
        // Every rung draws now: CS:S's a player model, Combat Arms' more.
        let x = l.extras(i).unwrap_or_else(|| panic!("rung {i} ({w}) draws"));
        assert_eq!(x.value(css::CLASS).is_some(), w.starts_with("cs_source:"), "rung {i} ({w}): {x:?}");
        assert_eq!(x.value(rungs::MODEL).is_some(), w.starts_with("combat_arms:"), "rung {i} ({w}): {x:?}");
    }
    let (a, _b) = two_on(&mut sim, 0);
    assert!(carried(&sim, a).contains(&cs_source::game::weapons::KNIFE), "CS:S's knife: {:?}", carried(&sim, a));
    assert!(!CaRungLook::worn(sim.app.world().get::<CaRungLook>(a)), "no Combat Arms look on a CS:S rung");
}

/// The class rung `rung` draws.
fn css_class(sim: &Sim, rung: usize) -> u8 {
    ladder_of(sim).extras(rung).and_then(|x| x.value(css::CLASS)?.parse().ok()).expect("a CS:S rung draws a class")
}

/// `p`'s class (None: none, or 0, left on by a rung that had one).
fn class_of(sim: &Sim, p: Entity) -> Option<u8> {
    sim.app.world().get::<PlayerClass>(p).map(|c| c.0).filter(|c| *c != 0)
}

#[test]
fn a_css_rung_gives_everyone_on_it_the_same_player_model() {
    let mut sim = party_with("", 3);
    let class = css_class(&sim, 0);
    let (a, b) = two_on(&mut sim, 0);
    assert_eq!(class_of(&sim, a), Some(class));
    assert_eq!(class_of(&sim, b), Some(class));
    // A respawn on the rung keeps it.
    kill(&mut sim, b, a);
    // (The killer's next rung is a Combat Arms gun's: no CS:S model.)
    assert_eq!(class_of(&sim, a), None, "the killer's next rung");
    sim.seconds(4.0);
    assert!(alive(&sim, b));
    assert_eq!(class_of(&sim, b), Some(class));
    // A late joiner arrives on a rung and wears that rung's.
    let c = soldier(&mut sim);
    sim.seconds(1.5);
    let r = rung(&sim, c);
    assert_eq!(class_of(&sim, c), Some(css_class(&sim, r)), "late join, rung {r}");
    // A Combat Arms rung takes it off again.
    let l = ladder_of(&sim);
    ladder::set_rung(sim.app.world_mut(), b, M4, &l);
    sim.seconds(1.0);
    assert_eq!(class_of(&sim, b), None, "a Combat Arms rung has its own look");
    ladder::set_rung(sim.app.world_mut(), b, 0, &l);
    sim.seconds(1.0);
    assert_eq!(class_of(&sim, b), Some(class));
}

#[test]
fn another_seed_draws_other_player_models() {
    let classes = |seed| {
        let l = ladder_of(&party_with("", seed));
        (0..l.rungs.len())
            .filter(|i| l.rungs[*i].starts_with("cs_source:"))
            .map(|i| l.extras(i).unwrap().value(css::CLASS).unwrap().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(classes(3), classes(3));
    assert_ne!(classes(3), classes(4));
    assert!(classes(3).iter().all(|c| (1..=8).contains(&c.parse::<u8>().unwrap())));
}

#[test]
fn the_character_option_off_keeps_the_players_own_model() {
    for extra in ["rung_extras = { character = false }", "rung_extras = false"] {
        let mut sim = party_with(extra, 3);
        let (a, _) = two_on(&mut sim, 0);
        assert_eq!(class_of(&sim, a), None, "{extra}");
    }
    // The other options off leave it on.
    let mut sim = party_with("rung_extras = { attachments = false, melee = false }", 3);
    let class = css_class(&sim, 0);
    let (a, _) = two_on(&mut sim, 0);
    assert_eq!(class_of(&sim, a), Some(class));
}

#[test]
fn another_seed_draws_other_extras() {
    let one = ladder_of(&party_with("", 3)).extras;
    assert_eq!(one, ladder_of(&party_with("", 3)).extras);
    assert_ne!(one, ladder_of(&party_with("", 4)).extras);
}

#[test]
fn each_option_off_is_todays_behaviour() {
    // All off: the M9, one's own look, the gun's own parts.
    let mut sim = party_with("rung_extras = false", 3);
    assert!(ladder_of(&sim).extras.iter().all(Option::is_none), "none drawn");
    let (a, _) = two_on(&mut sim, M4);
    assert_eq!(melee(&sim, a), Some(CA_KNIFE));
    assert!(!CaRungLook::worn(sim.app.world().get::<CaRungLook>(a)));
    assert_eq!(look(&sim, a).map(|s| s.model), Some(0), "the default look");
    let stock = fitted(sim.app.world(), a, "combat_arms:m4a1").unwrap();
    // One at a time.
    for (key, check) in [("attachments", 0), ("character", 1), ("melee", 2)] {
        let mut sim = party_with(&format!("rung_extras = {{ {key} = false }}"), 3);
        let (a, _) = two_on(&mut sim, M4);
        let w = sim.app.world();
        let parts_stock = fitted(w, a, "combat_arms:m4a1") == Some(stock);
        let own_look = !CaRungLook::worn(w.get::<CaRungLook>(a));
        let m9 = melee(&sim, a) == Some(CA_KNIFE);
        assert_eq!([parts_stock, own_look, m9][check], true, "{key} off");
        let others: Vec<bool> = [parts_stock, own_look, m9].iter().enumerate().filter(|(i, _)| *i != check).map(|(_, v)| *v).collect();
        assert!(others.iter().all(|v| !v), "{key} off leaves the others on: {others:?}");
    }
}

#[test]
fn a_network_client_wears_the_servers_look_and_parts() {
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: std::time::Duration::from_millis(20),
            jitter: std::time::Duration::ZERO,
            loss: 0.0,
        },
        7,
        2,
        |app| {
            app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin));
        },
    );
    sim.set_frame(TICK_INTERVAL);
    sim.server.app.insert_resource(RungSeed(3));
    minigame::start(sim.server.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    sim.until_joined(900);
    sim.ticks(300);
    let l = sim.server.app.world().resource::<Ladder>().clone();
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    for p in [a, b] {
        ladder::set_rung(sim.server.app.world_mut(), p, M4, &l);
    }
    sim.ticks(120);
    let gun = l.rungs[M4];
    let server_look = sim.server.app.world().get::<Selection>(a).copied();
    let server_fit = fitted(sim.server.app.world(), a, gun).map(|f| f.slots);
    assert!(server_look.is_some() && server_fit.is_some());
    assert_eq!(server_look, sim.server.app.world().get::<Selection>(b).copied());
    for i in 0..2 {
        let me = sim.local_player(i).unwrap();
        let w = sim.clients[i].app.world();
        assert_eq!(w.get::<Selection>(me).copied(), server_look, "client {i} wears the rung's look");
        assert_eq!(fitted(w, me, gun).map(|f| f.slots), server_fit, "client {i}'s gun has the rung's parts");
        assert_eq!(w.resource::<Ladder>().extras, l.extras, "client {i} has the server's rung extras");
    }
}

#[test]
fn a_network_client_sees_a_css_rungs_player_model() {
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: std::time::Duration::from_millis(20),
            jitter: std::time::Duration::ZERO,
            loss: 0.0,
        },
        7,
        2,
        |app| {
            app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin));
        },
    );
    sim.set_frame(TICK_INTERVAL);
    sim.server.app.insert_resource(RungSeed(3));
    minigame::start(sim.server.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    sim.until_joined(900);
    sim.ticks(300);
    let l = sim.server.app.world().resource::<Ladder>().clone();
    let class: u8 = l.extras(0).and_then(|x| x.value(css::CLASS)?.parse().ok()).expect("a class");
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    for p in [a, b] {
        ladder::set_rung(sim.server.app.world_mut(), p, 0, &l);
    }
    sim.ticks(120);
    for p in [a, b] {
        assert_eq!(sim.server.app.world().get::<PlayerClass>(p).map(|c| c.0), Some(class));
    }
    for i in 0..2 {
        let me = sim.local_player(i).unwrap();
        let w = sim.clients[i].app.world();
        assert_eq!(w.get::<PlayerClass>(me).map(|c| c.0), Some(class), "client {i} wears the rung's model");
    }
}

/// The bodies `cs_source::game::bsp::add_bodies` loads, as stand-ins with
/// no mesh: the T default (team 1), the CT default (no team), then
/// classes 2-4 and 6-8 by their team and class.
fn css_bodies() -> chimera_world::map::CharacterModels {
    use chimera_world::map::MapCharacterModel;
    let body = |team: Option<u8>, class: Option<u8>| MapCharacterModel {
        game: Some("cs_source"),
        team: team.map(chimera_core::core::Team),
        name: None,
        class,
        model: default(),
        hitboxes: Vec::new(),
        bones: Vec::new(),
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    };
    let mut v = vec![body(Some(1), None), body(None, None)];
    for c in [2u8, 3, 4, 6, 7, 8] {
        v.push(body(Some(1 + (c - 1) / 4), Some(c)));
    }
    chimera_world::map::CharacterModels(std::sync::Arc::new(v))
}

#[test]
fn a_css_rungs_player_wears_its_models_body() {
    use chimera_world::map::{BodyModel, MapSimPlugin};
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, MapSimPlugin::empty(), PartyPartsPlugin));
    sim.app.insert_resource(css_bodies());
    sim.app.insert_resource(RungSeed(3));
    minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    let (a, b) = two_on(&mut sim, 0);
    let class = css_class(&sim, 0);
    // The model for the class: the tagged one, or a side's default.
    let want = match class {
        1 => 0,
        5 => 1,
        2..=4 => class as usize,
        _ => class as usize - 1,
    };
    for p in [a, b] {
        assert_eq!(sim.app.world().get::<BodyModel>(p).map(|m| m.0), Some(want), "class {class}");
    }
}

/// The rung line names a rung's parts by their display names (the
/// install's string table, `rungs::CaPartNames`, which the front end
/// puts there), shortened; without them, readable record names.
#[test]
fn the_rung_line_names_the_parts_by_display_name() {
    let tuned = "rung_extras = { attachments = { sight = 1.0, muzzle = 0.0, magazine = 0.0, bullet = 0.0, mech_sight = 0.0 } }";
    let sim = party_with(tuned, 3);
    let parts = ladder_of(&sim).extras(M4).and_then(|x| x.value(rungs::PARTS)).unwrap_or_default().to_string();
    assert!(!parts.is_empty() && !parts.contains(','), "one sight: {parts}");
    let plain = ladder_of(&sim).extras(M4).and_then(|x| x.label.clone()).unwrap();
    assert_eq!(plain, rungs::readable(&parts), "no install strings: the record name made readable");
    assert!(!plain.contains('_'));
    let named = party_with_app(tuned, 3, |app| {
        app.insert_resource(rungs::CaPartNames::new([(parts.clone(), "Test Reflex Sight II (AR) ".to_string())]));
    });
    assert_eq!(ladder_of(&named).extras(M4).and_then(|x| x.label.clone()).as_deref(), Some("Test Reflex Sight II"));
}
