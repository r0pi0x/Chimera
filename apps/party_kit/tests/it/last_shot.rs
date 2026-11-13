//! The gun game's killing round with a Combat Arms gun (playtest
//! 2026-10-28: "on the last shot, I don't hear the bullet on the Combat
//! Arms guns"): the kill swaps the gun for the next rung's on the kill's
//! tick, so the gun entity the round's `WeaponEvent`s name is gone when
//! Combat Arms' effects (`Update`) read them. Its fire sound still plays,
//! once per round, for the shooter and for others, on the one that hosts
//! the match and on network clients.

use bevy::{ecs::message::MessageCursor, prelude::*};
use chimera_core::core::LocalPlayer;
use chimera_net::harness::NetSim;
use chimera_sim::{
    minigame,
    testkit,
    weapon::{WeaponEvent, WeaponEventKind},
};
use combat_arms::game::{
    TICK_INTERVAL,
    effects::{self, data::CaWeaponFx, sounds::CaSound},
    attachments::CaMods,
};

use crate::{gun_game, net_gun_game};

const BERETTA: &str = "combat_arms:beretta_m92fs";

#[derive(Resource, Default)]
struct Heard(Vec<CaSound>);

fn listen(app: &mut App) {
    app.init_resource::<Heard>().add_systems(
        Update,
        (|mut r: MessageReader<CaSound>, mut h: ResMut<Heard>| h.0.extend(r.read().cloned()))
            .after(effects::CaEffectsSet),
    );
}

/// The fire sound file `who`'s gun in hand plays for its shooter (`own`)
/// or for others, in `world`.
fn fire_file(world: &World, who: Entity, own: bool) -> String {
    let w = testkit::active(world, who).expect("a gun in hand");
    let fx = world.get::<CaWeaponFx>(w).expect("a Combat Arms gun");
    effects::fire_sound(fx, world.get::<CaMods>(w), own).expect("a fire sound").0.to_string()
}

fn fired(e: &WeaponEvent, who: Entity) -> bool {
    e.owner == who && !e.replay && matches!(e.kind, WeaponEventKind::Fired { .. })
}

/// `a` kills `b` with the second rung's gun (Combat Arms' Beretta) while
/// `listener` is the local player: the rounds `a` fired and the Beretta
/// fire sounds heard.
fn killing_rounds(shooter_listens: bool) -> (usize, usize) {
    let mut sim = gun_game::party();
    listen(&mut sim.app);
    let a = gun_game::soldier(&mut sim);
    let b = gun_game::soldier(&mut sim);
    let listener = if shooter_listens { a } else { b };
    sim.app.world_mut().entity_mut(listener).insert(LocalPlayer);
    sim.seconds(1.0);
    gun_game::line_up(&mut sim, a, b, 4.0);
    assert!(gun_game::shoot_until_dead(&mut sim, a, b, 8.0), "the Glock kills");
    gun_game::line_up(&mut sim, a, b, 4.0);
    assert_eq!(gun_game::held(&sim, a), Some(BERETTA), "the second rung's gun");
    let file = fire_file(sim.app.world(), a, shooter_listens);
    sim.app.world_mut().resource_mut::<Heard>().0.clear();
    let mut events = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut rounds = 0;
    let ticks = (8.0 * sim.tick_hz()) as u64;
    let mut after = 0;
    for t in 0..ticks {
        let at = sim.position(b);
        testkit::aim_at(&mut sim, a, at);
        sim.intent(a).fire = after == 0 && t % 2 == 0;
        sim.ticks(1);
        rounds += events.read(sim.app.world().resource::<Messages<WeaponEvent>>()).filter(|e| fired(e, a)).count();
        if !gun_game::alive(&sim, b) {
            after += 1;
            if after > 10 {
                break;
            }
        }
    }
    sim.intent(a).fire = false;
    assert!(after > 0, "the Beretta kills");
    assert_ne!(gun_game::held(&sim, a), Some(BERETTA), "the kill swapped the gun");
    let heard = sim.app.world().resource::<Heard>().0.iter().filter(|s| s.file == file).count();
    (rounds, heard)
}

/// The shooter, playing where the match runs, hears its own killing round.
#[test]
fn your_own_killing_round_with_a_combat_arms_gun_sounds() {
    let (rounds, heard) = killing_rounds(true);
    assert!(rounds > 1, "rounds fired: {rounds}");
    assert_eq!(heard, rounds, "one fire sound per round, the killing one too");
}

/// The victim, playing where the match runs, hears the killing round.
#[test]
fn others_killing_round_with_a_combat_arms_gun_sounds() {
    let (rounds, heard) = killing_rounds(false);
    assert!(rounds > 1, "rounds fired: {rounds}");
    assert_eq!(heard, rounds, "one fire sound per round, the killing one too");
}

/// The same on network clients: client 0's own predicted killing round with
/// the Beretta sounds once on it, and each round the server fired sounds
/// once on client 1, the killing one too.
#[test]
fn a_killing_round_with_a_combat_arms_gun_sounds_on_clients() {
    let mut sim = net_gun_game::joined();
    for c in &mut sim.clients {
        listen(&mut c.app);
    }
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    let ladder = sim.server.app.world().resource::<minigame::ladder::Ladder>().rungs.clone();
    assert_eq!(ladder[1], BERETTA);
    net_gun_game::kill_once(&mut sim, 0, a, b, &ladder);
    assert_eq!(net_gun_game::held(&mut sim, 0), Some(BERETTA));
    assert!(
        sim.until(600, |s| net_gun_game::server_alive(s, b)
            && s.server.app.world().get::<chimera_core::core::God>(b).is_none()),
        "the victim is back"
    );
    sim.ticks(30);
    net_gun_game::face_off(&mut sim, a, b);
    sim.ticks(90);
    let me = sim.local_player(0).unwrap();
    let id0 = sim.client_id(0);
    let a_seen = NetSim::owned_by(&mut sim.clients[1], Some(id0)).expect("client 1 sees player a");
    let own_file = fire_file(sim.clients[0].app.world(), me, true);
    let their_file = fire_file(sim.clients[1].app.world(), a_seen, false);
    for c in &mut sim.clients {
        c.app.world_mut().resource_mut::<Heard>().0.clear();
    }
    let cursor = |w: &World| -> MessageCursor<WeaponEvent> { w.resource::<Messages<WeaponEvent>>().get_cursor_current() };
    let mut own = cursor(sim.clients[0].app.world());
    let mut served = cursor(sim.server.app.world());
    let (mut mine, mut server_rounds) = (0, 0);
    let mut dead_for = 0;
    let mut count = |sim: &mut NetSim, mine: &mut usize, server_rounds: &mut usize| {
        *mine += own.read(sim.clients[0].app.world().resource::<Messages<WeaponEvent>>()).filter(|e| fired(e, me)).count();
        *server_rounds +=
            served.read(sim.server.app.world().resource::<Messages<WeaponEvent>>()).filter(|e| fired(e, a)).count();
    };
    for t in 0..(10.0 / TICK_INTERVAL) as u64 {
        let target = sim.server.position(b);
        testkit::aim_at(&mut sim.clients[0], me, target);
        sim.clients[0].intent(me).fire = dead_for == 0 && t % 2 == 0;
        sim.step();
        count(&mut sim, &mut mine, &mut server_rounds);
        if !net_gun_game::server_alive(&sim, b) {
            dead_for += 1;
            if dead_for > 6 {
                break;
            }
        }
    }
    sim.clients[0].intent(me).fire = false;
    assert!(dead_for > 0, "the Beretta kills");
    // The swap, the echoes and any corrections.
    for _ in 0..60 {
        sim.step();
        count(&mut sim, &mut mine, &mut server_rounds);
    }
    assert_ne!(net_gun_game::held(&mut sim, 0), Some(BERETTA), "the kill swapped the gun");
    let heard = |sim: &NetSim, i: usize, file: &str| {
        sim.clients[i].app.world().resource::<Heard>().0.iter().filter(|s| s.file == file).count()
    };
    assert!(mine > 1, "rounds fired: {mine}");
    assert_eq!(heard(&sim, 0, &own_file), mine, "the shooter hears each of its own rounds once, the killing one too");
    assert_eq!(heard(&sim, 1, &their_file), server_rounds, "the other client hears each round the server fired once");
}
