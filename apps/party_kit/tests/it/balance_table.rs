//! The party's cross-game table (`party_kit::balance`, data/
//! party_balance.toml) and its host switch (`mp_balance`), on the party's
//! composed simulation:
//!
//! - **Every pair**: one representative gun of each game (CS:S's AK-47,
//!   HL2:DM's SMG1, TF2's scattergun, Combat Arms' M4A1, Halo's assault
//!   rifle) on a body of each game, its first hit's damage (health, and a
//!   Halo body's shield and body counted) with the table against a raw
//!   match: the ratio is today's for every pair (docs/plans/active/
//!   party-balance.md's appendix: Halo's guns 0.75 on others, HL2:DM's 1.5
//!   on every body, every gun 1.5 on a Halo body, 1 otherwise), and Halo
//!   against Halo is the same in both.
//! - **The switch**: `mp_balance 0` on a server is put in force at the next
//!   match's start (the status says so meanwhile), and a client has the
//!   server's value (the replicated cvars).

use bevy::prelude::*;
use chimera_core::{
    console::Console,
    core::{BodyRegion, Health},
    slots::GameIdentity,
};
use chimera_sim::{minigame, weapon::balance::Balance};
use chimera_world::harness::Sim;
use halo_ce::game::{
    damage::HaloHit,
    vitality::{DamageEffects, Mirrored, Vitality, ids},
    weapons,
};

use crate::{
    gun_game::{line_up, soldier},
    halo_balance::{gun_hit, party_table, per_hit},
    halo_ladder::set_rung,
};

/// One gun of each game, its rung in this order.
const GUNS: [&str; 5] = [
    "cs_source:weapon_ak47",
    "hl2dm:weapon_smg1",
    "tf2:tf_weapon_scattergun",
    "combat_arms:m4a1",
    weapons::ASSAULT_RIFLE,
];

/// The games of `GUNS`, a body of each by its rung.
const GAMES: [&str; 5] = ["cs_source", "hl2dm", "tf2", "combat_arms", "halo_ce"];

/// Today's scale of a hit by `a`'s gun on `v`'s body with the table on
/// (the appendix; a raw match is 1). Halo against Halo: unchanged, checked
/// as the same in both.
fn today(a: &str, v: &str) -> f32 {
    let out = match a {
        "halo_ce" => 0.75,
        "hl2dm" => 1.5,
        _ => 1.0,
    };
    let inc = if v == "halo_ce" { 1.5 } else { 1.0 };
    out * inc
}

/// The party playing `GUNS`.
fn party() -> Sim {
    let text = crate::hl2dm_parts::ladder_text(&GUNS, false).replace(
        "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]",
        "weapons = [\"cs_source\", \"hl2dm\", \"tf2\", \"halo_ce\", \"combat_arms\"]",
    );
    crate::halo_ladder::party_with(&text)
}

/// What `v` has: health, and a Halo body's shield and body as Halo counts
/// them (1.0 the shield, 1.0 the body).
fn pool(sim: &Sim, v: Entity) -> f32 {
    let w = sim.app.world();
    match w.get::<Vitality>(v) {
        Some(vit) => vit.shield + vit.body,
        None => w.get::<Health>(v).map_or(0.0, |h| h.current),
    }
}

/// `v` full again: health, no armour (a bare body), a Halo body's shield
/// and body (and what the engine sees of them).
fn refill(sim: &mut Sim, v: Entity) {
    let w = sim.app.world_mut();
    let k = w.get::<chimera_core::core::DamageScale>(v).map_or(1.0, |s| s.0);
    let mut e = w.entity_mut(v);
    e.get_mut::<Health>().unwrap().current = 1.0;
    if e.contains::<Vitality>() {
        {
            let mut vit = e.get_mut::<Vitality>().unwrap();
            vit.shield = 1.0;
            vit.body = 1.0;
        }
        e.get_mut::<chimera_sim::weapon::Armor>().unwrap().amount = 1.0 / k;
        let mut m = e.get_mut::<Mirrored>().unwrap();
        m.health = 1.0;
        m.shield = 1.0 / k;
    } else {
        e.remove::<chimera_sim::weapon::Armor>();
    }
}

/// The first hit's damage of gun `g` (its own game's number) on `v`.
fn first_hit(sim: &mut Sim, v: Entity, g: usize) -> f32 {
    refill(sim, v);
    sim.ticks(1);
    let before = pool(sim, v);
    if GAMES[g] == "halo_ce" {
        let amount = sim.app.world().resource::<DamageEffects>().get(ids::ASSAULT_RIFLE_BULLET).unwrap().amount(1.0, 0.5);
        let at = sim.position(v);
        let hit = HaloHit::direct(v, ids::ASSAULT_RIFLE_BULLET, amount, 1.0, Some(BodyRegion::Chest), at, Vec3::NEG_Z, None);
        sim.app.world_mut().write_message(hit.weapon(weapons::ASSAULT_RIFLE));
        sim.ticks(1);
    } else {
        let (raw, _) = per_hit(sim, GUNS[g]);
        gun_hit(sim, v, GUNS[g], raw, BodyRegion::Chest);
    }
    before - pool(sim, v)
}

/// Every pair's first hit with the table and in a raw match: (attacker,
/// victim, on, raw, the gun's own number: 0 for Halo's).
fn pairs() -> Vec<(usize, usize, f32, f32, f32)> {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    let mut out = Vec::new();
    for (vr, victim) in GAMES.iter().enumerate() {
        set_rung(&mut sim, b, vr);
        sim.seconds(1.2);
        line_up(&mut sim, a, b, 3.0);
        let w = sim.app.world();
        assert_eq!(w.get::<GameIdentity>(b).map(|g| g.0.as_str()), Some(*victim), "the victim is {victim}'s");
        for g in 0..GUNS.len() {
            let mut got = [0.0; 2];
            for (k, on) in [true, false].into_iter().enumerate() {
                sim.app.insert_resource(party_table(on));
                // The body's held incoming follows (`cross_game::keep_incoming`).
                sim.ticks(2);
                got[k] = first_hit(&mut sim, b, g);
            }
            let own = if GAMES[g] == "halo_ce" { 0.0 } else { per_hit(&sim, GUNS[g]).0 };
            out.push((g, vr, got[0], got[1], own));
        }
    }
    out
}

/// Today's numbers give today's results for every pair; a raw match is
/// each game's own number; Halo against Halo is the same in both.
#[test]
fn every_pair_takes_today_s_scale_and_raw_its_own_game_s() {
    let rows = pairs();
    let table = party_kit::balance::table();
    for (g, v, on, raw, own) in rows {
        let (a, vg) = (GAMES[g], GAMES[v]);
        println!("{a} on {vg}: raw {:.2}, table {:.2}", raw * 100.0, on * 100.0);
        assert!(raw > 0.0, "{a} on {vg}: a hit");
        if a != "halo_ce" && vg == "cs_source" {
            // Raw on a bare CS:S body: the gun's own game's number. (The
            // other bodies have their rules: whole points on HL2:DM's and
            // TF2's, Combat Arms' gear.)
            assert!((raw - own).abs() < 0.011, "{a} on {vg}: raw {raw} own {own}");
        }
        if a == "halo_ce" && vg == "halo_ce" {
            assert!((on - raw).abs() < 1e-5, "Halo on Halo: {on} {raw}");
            continue;
        }
        let want = today(a, vg);
        assert_eq!(table.pair(a, Some(vg), None), want, "{a} on {vg}: the table's");
        // Within a point and a half: HL2:DM's and TF2's health take whole
        // points, the fraction carried to the next hit.
        assert!((on - raw * want).abs() < 0.016, "{a} on {vg}: {on} / {raw} = {}, today {want}", on / raw);
    }
}

/// `mp_balance 0` on a server: in force from the next match (the status
/// says it meanwhile), its client has the server's value; back to 1, on
/// again from the match after.
#[test]
fn mp_balance_applies_from_the_next_match_and_reaches_clients() {
    let mut sim = crate::net_party::party_and(1, None, |_| {});
    sim.until_joined(900);
    sim.ticks(10);
    let on = |s: &chimera_net::harness::NetSim| s.server.app.world().resource::<Balance>().on;
    assert!(on(&sim), "on by default");
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_balance 0");
    sim.ticks(10);
    assert!(on(&sim), "not during the match");
    let status = chimera_net::net::status(sim.server.app.world_mut());
    assert!(status.contains("balance : on") && status.contains("mp_balance 0 from the next match"), "{status}");
    assert!(
        sim.until(300, |s| {
            let b = s.clients[0].app.world().resource::<Balance>();
            b.switch == 0 && !b.on
        }),
        "the client has the server's value"
    );
    let id = sim.server.app.world().resource::<minigame::ActiveMinigame>().def.id.clone();
    minigame::start(sim.server.app.world_mut(), &id).unwrap();
    sim.ticks(5);
    assert!(!on(&sim), "the next match is raw");
    assert!(chimera_net::net::status(sim.server.app.world_mut()).contains("balance : off"));
}
