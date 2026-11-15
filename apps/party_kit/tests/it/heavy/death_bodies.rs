//! A death leaves a body on every client, for every game's body
//! (Ultimate Gun Game playtest 7: "deaths on CS:S and Combat Arms rungs
//! show no ragdoll or death animation; the body just disappears", seen
//! by other players). Over the network on de_dust2 with the real bodies
//! (head_matrix.rs's world: a server and two clients, the server with
//! ragdolls off as a dedicated one), client 1's player is killed on each
//! game's rung: on both clients a CS:S or HL2:DM body becomes a ragdoll
//! (`map::ragdoll::Ragdoll`, the drawn body handed to it: `Ragdolled`)
//! and a Combat Arms one leaves its corpse (`CaBody`, drawn as a
//! `map::Corpse`), each kept while the player is dead. Before: a client
//! heard of the death (`Killed`) ticks before the player's health came
//! (replicated changes are paced), so the ragdoll it made saw its owner
//! still alive and went the next tick (CS:S's rules: removed; HL2:DM's:
//! kept as outliving a respawn, then removed as its owner "died again").
//! Passes with a note without the three installs.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Health, Hitgroup};
use chimera_net::harness::NetSim;
use chimera_world::map::{
    Corpse,
    ragdoll::{Ragdoll, RagdollSettings, Ragdolled},
};
use combat_arms::game::characters::CaBody;

use super::head_matrix::{GUNS, net_dust2};

/// What client `i` shows for the dead player `seen` (its copy of client
/// 1's player) on a `game` rung: a ragdoll of it with its drawn body
/// handed over, or a Combat Arms corpse not in `before` (drawn: `Corpse`).
fn body_left(sim: &mut NetSim, i: usize, seen: Entity, game: &str, before: &[Entity]) -> Option<&'static str> {
    let w = sim.clients[i].app.world_mut();
    if game == "combat_arms" {
        let corpses: Vec<bool> =
            w.query::<(Entity, &CaBody, Option<&Corpse>)>().iter(w).filter(|(e, ..)| !before.contains(e)).map(|(.., c)| c.is_some()).collect();
        return match (corpses.is_empty(), corpses.iter().any(|c| *c)) {
            (true, _) => None,
            (false, true) => Some("corpse"),
            (false, false) => Some("corpse, not drawn"),
        };
    }
    let ragdoll = w.query::<&Ragdoll>().iter(w).any(|r| r.owner == seen && !r.placed);
    (ragdoll && w.get::<Ragdolled>(seen).is_some()).then_some("ragdoll")
}

fn corpses(sim: &mut NetSim, i: usize) -> Vec<Entity> {
    let w = sim.clients[i].app.world_mut();
    w.query_filtered::<Entity, With<CaBody>>().iter(w).collect()
}

#[test]
fn a_death_leaves_a_body_on_every_client_for_every_games_body() {
    let Some((mut sim, _, _)) = net_dust2() else { return };
    // As a dedicated server: player ragdolls are the clients'.
    sim.server.app.world_mut().resource_mut::<RagdollSettings>().enabled = 0;
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    let owner = sim.client_id(1);
    let mut wrong = Vec::new();
    for (target, (game, _)) in GUNS.iter().enumerate() {
        {
            let w = sim.server.app.world_mut();
            let l = w.resource::<chimera_sim::minigame::ladder::Ladder>().clone();
            chimera_sim::minigame::ladder::set_rung(w, a, 0, &l);
            chimera_sim::minigame::ladder::set_rung(w, b, target, &l);
        }
        // Alive, past its spawn protection, and on its rung everywhere.
        let alive = sim.until(900, |s| {
            let w = s.server.app.world();
            let now = w.resource::<chimera_core::core::SimClock>().now;
            w.get::<Health>(b).is_some_and(|h| h.current > 0.0)
                && w.get::<chimera_core::core::God>(b).is_none()
                && w.get::<chimera_core::core::SpawnProtected>(b).is_none_or(|p| !p.hides_hits(now))
                && (0..2).all(|i| {
                    let seen = NetSim::owned_by(&mut s.clients[i], Some(owner)).unwrap();
                    let w = s.clients[i].app.world();
                    w.get::<Health>(seen).is_some_and(|h| h.current > 0.0) && w.get::<Ragdolled>(seen).is_none()
                })
        });
        assert!(alive, "{game}: never alive again");
        sim.ticks(120);
        let before = [corpses(&mut sim, 0), corpses(&mut sim, 1)];
        let at = sim.server.app.world().get::<Transform>(b).unwrap().translation;
        sim.server.app.world_mut().write_message(Damage {
            force: Vec3::ZERO,
            target: b,
            attacker: Some(a),
            amount: 1000.0,
            point: at + Vec3::Y * 0.3,
            dir: Vec3::NEG_Z,
            hitgroup: Hitgroup::Chest,
            kind: DamageKind::Bullet,
            weapon: None,
        });
        sim.step();
        let killed = sim.server.app.world().get::<Health>(b).is_some_and(|h| h.current <= 0.0);
        assert!(killed, "{game}: the server's player wasn't killed");
        // On each client within a link's latency and a few ticks, then
        // every tick for 0.75 s (the dead stay dead that long).
        let mut shown: [Option<u32>; 2] = [None; 2];
        let mut went: [Option<u32>; 2] = [None; 2];
        for t in 0..90u32 {
            for i in 0..2 {
                let seen = NetSim::owned_by(&mut sim.clients[i], Some(owner)).unwrap();
                let left = body_left(&mut sim, i, seen, game, &before[i]);
                match (shown[i], left) {
                    (None, Some(what)) => {
                        eprintln!("{game} rung, client {i}: a {what} at tick {t}");
                        shown[i] = Some(t);
                    }
                    (Some(at), None) if went[i].is_none() && t < at + 48 => went[i] = Some(t),
                    _ => {}
                }
            }
            sim.step();
        }
        for i in 0..2 {
            match (shown[i], went[i]) {
                (None, _) => wrong.push(format!("{game} rung, client {i}: no body")),
                (Some(at), Some(t)) => wrong.push(format!("{game} rung, client {i}: the body went {} ticks after it came", t - at)),
                (Some(at), None) if at > 40 => wrong.push(format!("{game} rung, client {i}: the body came {at} ticks late")),
                _ => {}
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
