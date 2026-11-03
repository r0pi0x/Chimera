//! The gun game's Combat Arms rung characters (`combat_arms::game::rungs`,
//! docs/DESIGN.md "Gun game rules"): a drawn body plays with the baseline
//! vest (no correction for a network client at a swap to another body);
//! the infected's rung (Chimera's `ladder::RungCharacter`, with a
//! test-written infected's hand: the install's are the Quarantine hands)
//! is a melee rung's choice, the knife finale's, at the data's odds (never
//! a gun rung; never at odds 0 or with the option off); it turns everyone
//! on it into the infected holding the hands alone, at the baseline's
//! health and movement, Combat Arms' observers of `ladder::OnRung` putting
//! the marker on and playing the infection's moment once on arriving and
//! taking it off once on leaving, and another rung makes them themselves
//! again; a network client sees the same, its own `OnRung` observers
//! firing once per arrival and leaving.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Health, Hitgroup, Intent};
use chimera_net::{
    harness::NetSim,
    net::{memory::LinkConditions, predict::NetGraph},
};
use chimera_sim::{
    minigame::{
        self, MinigameAppExt, Minigames,
        ladder::{self, Ladder, OnRung, RungSeed},
    },
    weapon::{Inventory, Weapon},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        characters::records::Selection,
        data::{CaData, WEAPON_DATA},
        gear::CaGearData,
        effects::CaFxRequest,
        movement::{CaCharacter, CaInfected},
        rungs::{self, CaRungInfected},
    },
    harness::{CaSim, CaSimWith},
};
use party_kit::{DEFAULT_MINIGAME, PartyPartsPlugin};

use super::gun_game::{alive, soldier};
use super::identity::assert_no_errors;

/// The test's infected's hand: the M9 knife's numbers, marked infected.
const HANDS: &str = "combat_arms:test_infected_hands";

/// The built-in data with `HANDS`.
fn data() -> CaData {
    let mut docs: Vec<(String, String)> = WEAPON_DATA.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect();
    let m9 = WEAPON_DATA.iter().find(|(n, _)| *n == "m9.toml").expect("the M9's document").1;
    let hands = m9.replace("id = \"combat_arms:m9_knife\"", &format!("id = \"{HANDS}\"")) + &format!("\n[hooks.\"{}\"]\n", rungs::INFECTED);
    docs.push(("test_infected_hands.toml".into(), hands));
    CaData::builtin_with_weapons(docs)
}

/// The infected always, on the knife finale's rung.
const ALWAYS: &str = "knife_finale = true\nrung_extras = { infected = { odds = 1.0 } }";

/// The gun game as `originals:variant` in `app`, `[rules]` lines `extra`
/// added (the definition's finale is on; without `knife_finale = true` in them it is turned off).
fn add_variant(app: &mut App, extra: &str) {
    let text = app.world().resource::<Minigames>().text(DEFAULT_MINIGAME).unwrap().to_string();
    let finale = extra.contains("knife_finale = true");
    let extra = extra.replace("knife_finale = true", "");
    let text = text
        .replace(&format!("id = \"{DEFAULT_MINIGAME}\""), "id = \"originals:variant\"")
        .replace("\nrung_extras = true\n", "\n")
        .replace("knife_finale = true ", if finale { "knife_finale = true " } else { "knife_finale = false" })
        .replace("[rules]\n", &format!("[rules]\n{extra}\n"));
    app.add_minigames(&[("variant.toml", &text)]).unwrap();
}

/// The gun game on seed `seed`, `[rules]` lines `extra` added.
fn party<B>(base: impl bevy::app::Plugins<B>, extra: &str, seed: u64) -> Sim {
    let mut sim = Sim::new(base, (GreyboxMapPlugin, PartyPartsPlugin));
    sim.app.insert_resource(RungSeed(seed));
    add_variant(&mut sim.app, extra);
    minigame::start(sim.app.world_mut(), "originals:variant").expect("it starts");
    sim
}

fn ladder_of(sim: &Sim) -> Ladder {
    sim.app.world().resource::<Ladder>().clone()
}

/// The infected's rung (exactly one).
fn infected_rung(l: &Ladder) -> usize {
    let rungs: Vec<usize> = (0..l.rungs.len()).filter(|i| l.character(*i).is_some()).collect();
    assert_eq!(rungs.len(), 1, "one infected's rung: {:?}", l.extras);
    rungs[0]
}

fn carried(world: &World, p: Entity) -> Vec<&'static str> {
    world.get::<Inventory>(p).unwrap().weapons.iter().filter_map(|x| world.get::<Weapon>(*x)).map(|x| x.id).collect()
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

/// The infected's marker as it came and went: (player, became or left).
#[derive(Resource, Default)]
struct Moments(Vec<(Entity, &'static str)>);

fn count_moments(app: &mut App) {
    app.init_resource::<Moments>()
        .add_observer(|on: On<Add<CaRungInfected>>, mut m: ResMut<Moments>| m.0.push((on.event_target(), "enter")))
        .add_observer(|on: On<Remove<CaRungInfected>>, mut m: ResMut<Moments>| m.0.push((on.event_target(), "leave")));
}

/// The `OnRung` observers' calls as they came: (player, arrived or left,
/// the rung).
#[derive(Resource, Default)]
struct RungMoments(Vec<(Entity, &'static str, usize)>);

fn count_rung_moments(app: &mut App) {
    app.init_resource::<RungMoments>()
        .add_observer(|on: On<Insert<OnRung>>, q: Query<&OnRung>, mut m: ResMut<RungMoments>| {
            m.0.push((on.event_target(), "arrive", q.get(on.event_target()).unwrap().index));
        })
        .add_observer(|on: On<Discard<OnRung>>, q: Query<&OnRung>, mut m: ResMut<RungMoments>| {
            m.0.push((on.event_target(), "leave", q.get(on.event_target()).unwrap().index));
        });
}

/// How many infection effects were asked for this update.
fn infection_fx(world: &World) -> usize {
    world.resource::<Messages<CaFxRequest>>().iter_current_update_messages().filter(|r| r.group == rungs::INFECTION_FX).count()
}

fn character(world: &World, p: Entity) -> String {
    format!("{:?}", world.get::<CaCharacter>(p).expect("a Combat Arms character"))
}

#[test]
fn the_infected_rung_makes_everyone_on_it_the_infected_with_the_hands_alone_until_the_next_rung() {
    let mut sim = party(CaSimWith(data()), ALWAYS, 3);
    count_moments(&mut sim.app);
    let l = ladder_of(&sim);
    let z = infected_rung(&l);
    assert_eq!(z + 1, l.rungs.len(), "the knife finale's rung");
    let x = l.extras(z).unwrap();
    assert_eq!(x.character.as_ref().map(|c| c.weapon.as_str()), Some(HANDS));
    let body: i32 = x.value(rungs::MODEL).expect("an infected body").parse().unwrap();
    // A player on the other Combat Arms gun rung for the baseline.
    let other = (0..l.rungs.len()).rev().find(|i| *i != z && l.rungs[*i].starts_with("combat_arms:") && l.character(*i).is_none()).unwrap();
    let (a, b, c) = (soldier(&mut sim), soldier(&mut sim), soldier(&mut sim));
    sim.seconds(1.5);
    ladder::set_rung(sim.app.world_mut(), c, other, &l);
    for (i, p) in [a, b].into_iter().enumerate() {
        ladder::set_rung(sim.app.world_mut(), p, z, &l);
        assert_eq!(infection_fx(sim.app.world()), i + 1, "the infection's effect, from Combat Arms' observer");
    }
    sim.seconds(1.0);
    let w = sim.app.world();
    for p in [a, b] {
        assert_eq!(carried(w, p), [HANDS], "the hands alone: no gun, no kit");
        assert!(w.get::<CaRungInfected>(p).is_some(), "the infected");
        assert!(w.get::<CaInfected>(p).is_none(), "no infected rules (knock-back)");
        assert_eq!(w.get::<Selection>(p).map(|s| s.model), Some(body), "the infected's body");
        let h = w.get::<Health>(p).unwrap();
        assert_eq!((h.current, h.max), (1.0, 1.0), "the baseline's health");
        assert_eq!(character(w, p), character(w, c), "the baseline's movement");
    }
    assert_eq!(w.resource::<Moments>().0, [(a, "enter"), (b, "enter")], "once each");
    // b, shot by c, comes back on the rung the infected: no new moment.
    kill(&mut sim, b, c);
    sim.seconds(4.0);
    assert!(alive(&sim, b));
    let w = sim.app.world();
    assert_eq!(w.get::<ladder::Rung>(b).map(|r| r.0), Some(z), "still on the rung");
    assert_eq!(carried(w, b), [HANDS]);
    assert!(w.get::<CaRungInfected>(b).is_some());
    assert_eq!(w.resource::<Moments>().0, [(a, "enter"), (b, "enter")], "nothing for a respawn");
    // a goes down a rung (a gun's): itself again.
    let down = z - 1;
    ladder::set_rung(sim.app.world_mut(), a, down, &l);
    assert_eq!(infection_fx(sim.app.world()), 0, "no effect for leaving");
    sim.seconds(1.0);
    let w = sim.app.world();
    let have = carried(w, a);
    assert!(!have.contains(&HANDS) && have.contains(&l.rungs[down]), "the rung's gun, the hands gone: {have:?}");
    assert!(have.len() >= 2, "the kit's knife back: {have:?}");
    assert!(w.get::<CaRungInfected>(a).is_none());
    assert_eq!(w.resource::<Moments>().0, [(a, "enter"), (b, "enter"), (a, "leave")]);
}

/// Of `n` ladders made in a row on `sim` (each draws anew), how many have
/// the infected's rung; each such rung the last (the knife finale's),
/// and no gun rung's melee weapon a hand.
fn infected_ladders(sim: &mut Sim, n: usize) -> usize {
    let mut found = 0;
    for i in 0..n {
        minigame::start(sim.app.world_mut(), "originals:variant").expect("it starts");
        let l = ladder_of(sim);
        assert!(l.extras.iter().flatten().all(|x| x.melee.as_deref() != Some(HANDS)), "ladder {i}");
        let rungs: Vec<usize> = (0..l.rungs.len()).filter(|r| l.character(*r).is_some()).collect();
        match rungs.as_slice() {
            [] => {}
            [z] => {
                assert_eq!(z + 1, l.rungs.len(), "ladder {i}: only the knife finale's rung");
                found += 1;
            }
            many => panic!("ladder {i}: more than one infected's rung: {many:?}"),
        }
    }
    found
}

#[test]
fn the_infected_is_a_melee_rungs_choice_at_its_odds() {
    // The data's odds (a quarter), over 200 ladders.
    let mut sim = party(CaSimWith(data()), "knife_finale = true", 1);
    let n = infected_ladders(&mut sim, 200);
    assert!((30..=70).contains(&n), "about a quarter of 200: {n}");
    // Tuned.
    let mut sim = party(CaSimWith(data()), "knife_finale = true\nrung_extras = { infected = { odds = 0.75 } }", 1);
    let n = infected_ladders(&mut sim, 200);
    assert!((130..=170).contains(&n), "about three quarters of 200: {n}");
}

#[test]
fn the_infected_never_comes_at_odds_0_with_the_option_off_without_a_melee_rung_or_hands() {
    for extra in [
        "knife_finale = true\nrung_extras = { infected = { odds = 0 } }",
        "knife_finale = true\nrung_extras = { infected = false }",
        "rung_extras = { infected = { odds = 1.0 } }",
    ] {
        let mut sim = party(CaSimWith(data()), extra, 1);
        assert_eq!(infected_ladders(&mut sim, 30), 0, "{extra}");
    }
    let mut plain = party(CaSim, ALWAYS, 3);
    assert_eq!(infected_ladders(&mut plain, 10), 0, "built-in data: no hands");
}

fn link() -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(20),
        jitter: Duration::ZERO,
        loss: 0.0,
    }
}

/// The gun game over the network (`[rules]` lines `extra` added, none: as
/// it ships), its two clients' `OnRung` observers counted.
fn net<B>(base: impl bevy::app::Plugins<B> + Clone + Send + Sync + 'static, extra: Option<&str>) -> NetSim {
    let mut sim = NetSim::new(base, link(), 7, 2, |app| {
        app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin));
    });
    sim.set_frame(TICK_INTERVAL);
    sim.server.app.insert_resource(RungSeed(3));
    let mut id = DEFAULT_MINIGAME;
    if let Some(extra) = extra {
        add_variant(&mut sim.server.app, extra);
        for c in &mut sim.clients {
            add_variant(&mut c.app, extra);
        }
        id = "originals:variant";
    }
    for c in &mut sim.clients {
        count_rung_moments(&mut c.app);
    }
    minigame::start(sim.server.app.world_mut(), id).expect("the gun game starts");
    sim.until_joined(900);
    sim.ticks(300);
    sim
}

fn reset_graphs(sim: &mut NetSim) {
    for c in &mut sim.clients {
        *c.app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    }
}

fn all_on(sim: &mut NetSim, rung: usize) {
    let l = sim.server.app.world().resource::<Ladder>().clone();
    for i in 0..2 {
        let p = sim.character_of(i).unwrap();
        ladder::set_rung(sim.server.app.world_mut(), p, rung, &l);
    }
}

/// Both clients walk to and fro for `n` ticks, `at` (tick, what) done on
/// the way.
fn walk(sim: &mut NetSim, n: u32, mut at: impl FnMut(&mut NetSim, u32)) {
    for k in 0..n {
        for i in 0..2 {
            let me = sim.local_player(i).unwrap();
            let mut intent = sim.clients[i].app.world_mut().get_mut::<Intent>(me).unwrap();
            intent.move_axis = if (k / 20) % 2 == 0 { Vec2::Y } else { -Vec2::Y };
        }
        at(sim, k);
        sim.step();
    }
}

#[test]
fn a_network_client_predicts_a_swap_to_another_rung_body_without_a_correction() {
    let mut sim = net(CaSim, None);
    let l = sim.server.app.world().resource::<Ladder>().clone();
    let gear = sim.server.app.world().resource::<CaGearData>().0.clone();
    let ca: Vec<usize> = (0..l.rungs.len()).filter(|i| l.rungs[*i].starts_with("combat_arms:")).collect();
    // A rung body's own vest penalty (what it moved with before).
    let penalty = |i: usize| {
        let body: i32 = l.extras(i).and_then(|x| x.value(rungs::MODEL)?.parse().ok()).unwrap();
        gear.vests.get(&body).map(|v| v.move_penalty)
    };
    let (r1, r2) = ca
        .iter()
        .flat_map(|a| ca.iter().map(move |b| (*a, *b)))
        .find(|(a, b)| penalty(*a) != penalty(*b))
        .expect("two Combat Arms rungs whose bodies' vests move differently");
    all_on(&mut sim, r1);
    sim.ticks(120);
    reset_graphs(&mut sim);
    walk(&mut sim, 150, |sim, k| {
        if k == 40 {
            all_on(sim, r2);
        }
    });
    assert_no_errors(&sim, "a swap to another rung body, walking");
}

#[test]
fn a_network_client_sees_the_infected_and_their_return() {
    let mut sim = net(CaSimWith(data()), Some(ALWAYS));
    let l = sim.server.app.world().resource::<Ladder>().clone();
    let z = infected_rung(&l);
    // Each client's own player: arrived on the first rung once.
    let rung_moments = |sim: &mut NetSim, i: usize| -> Vec<(&'static str, usize)> {
        let me = sim.local_player(i).unwrap();
        sim.clients[i].app.world().resource::<RungMoments>().0.iter().filter(|m| m.0 == me).map(|m| (m.1, m.2)).collect()
    };
    for i in 0..2 {
        assert_eq!(rung_moments(&mut sim, i), [("arrive", 0)], "client {i}");
    }
    all_on(&mut sim, z);
    sim.ticks(120);
    for i in 0..2 {
        assert_eq!(rung_moments(&mut sim, i), [("arrive", 0), ("leave", 0), ("arrive", z)], "client {i}");
    }
    let first = sim.character_of(0).unwrap();
    let server_look = sim.server.app.world().get::<Selection>(first).copied();
    for i in 0..2 {
        let me = sim.local_player(i).unwrap();
        let w = sim.clients[i].app.world();
        assert!(w.get::<CaRungInfected>(me).is_some(), "client {i} sees its player infected");
        assert_eq!(carried(w, me), [HANDS], "client {i}: the hands alone");
        assert_eq!(w.get::<Health>(me).map(|h| h.max), Some(1.0), "client {i}: the baseline's health");
        assert_eq!(w.get::<Selection>(me).copied(), server_look);
    }
    reset_graphs(&mut sim);
    all_on(&mut sim, z - 1);
    sim.ticks(120);
    for i in 0..2 {
        let me = sim.local_player(i).unwrap();
        let w = sim.clients[i].app.world();
        assert!(w.get::<CaRungInfected>(me).is_none(), "client {i}: itself again");
        assert!(!carried(w, me).contains(&HANDS));
        assert_eq!(
            rung_moments(&mut sim, i),
            [("arrive", 0), ("leave", 0), ("arrive", z), ("leave", z), ("arrive", z - 1)],
            "client {i}"
        );
    }
    assert_no_errors(&sim, "leaving the infected's rung");
}

/// HL2:DM's rung characters (`hl2dm::game::rungs`): on the mixed ladder
/// (`hl2dm_parts::LADDER`, rung extras on) each HL2:DM rung dresses
/// everyone on it in one of HL2:DM's player models (`Hl2Model`, worn by
/// name: `BodyName`), and a network client sees the server's choice on
/// its own player and on the others (`Hl2Model` replicated, the net part
/// `hl2dm_bodies`); a rung of another game's gun takes it off on every
/// side.
#[test]
fn hl2dm_rungs_dress_their_players_on_server_and_clients() {
    use hl2dm::game::weapons::inventory::Hl2Model;
    let text = crate::hl2dm_parts::mixed_text(true);
    let setup = move |app: &mut App| {
        app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin));
        minigame::MinigameAppExt::add_minigames(app, &[("hl2dm.toml", &text)]).unwrap();
    };
    let mut sim = NetSim::new(combat_arms::harness::CaSim, link(), 7, 2, setup);
    sim.set_frame(TICK_INTERVAL);
    crate::gun_game::no_finale(sim.server.app.world_mut());
    minigame::start(sim.server.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    sim.until_joined(900);
    sim.ticks(300);
    let model = |w: &World, e: Entity| w.get::<Hl2Model>(e).map(|m| m.0.clone());
    let name = |w: &World, e: Entity| w.get::<chimera_world::map::BodyName>(e).map(|m| m.0.clone());
    for rung in [0usize, 2, 4] {
        all_on(&mut sim, rung);
        sim.ticks(120);
        let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
        let want = model(sim.server.app.world(), a);
        assert!(want.is_some(), "rung {rung}: the server dresses a");
        assert_eq!(want, model(sim.server.app.world(), b), "rung {rung}: everyone on it alike");
        for i in 0..2 {
            let me = sim.local_player(i).unwrap();
            let w = sim.clients[i].app.world();
            assert_eq!(model(w, me), want, "rung {rung}: client {i} sees its own model");
            let named = want.clone().filter(|m| m != hl2dm::game::weapons::inventory::DEFAULT_MODEL);
            assert_eq!(name(w, me), named, "rung {rung}: client {i} wears it by name");
        }
        // Client 1 sees player a dressed alike.
        let id0 = sim.client_id(0);
        let a_seen = NetSim::owned_by(&mut sim.clients[1], Some(id0)).expect("client 1 sees player a");
        assert_eq!(model(sim.clients[1].app.world(), a_seen), want, "rung {rung}: client 1 sees a's model");
    }
    // A CS:S rung: the model comes off everywhere.
    all_on(&mut sim, 1);
    sim.ticks(120);
    let a = sim.character_of(0).unwrap();
    assert_eq!(model(sim.server.app.world(), a), None);
    let me = sim.local_player(0).unwrap();
    assert_eq!(model(sim.clients[0].app.world(), me), None);
    assert_eq!(name(sim.clients[0].app.world(), me), None);
}
