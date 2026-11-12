//! The host's surprises (`party_kit::surprise`): `surprise airstrike`'s
//! shells fall, blow up and kill with no one credited (no rung up or down),
//! clients see the warning and the shells; `surprise gravity` scales every
//! movement's gravity and puts it back, a client's replicated `sv_gravity`
//! following, its prediction of a CS:S player agreeing with the server.

use bevy::{ecs::message::MessageCursor, prelude::*};
use chimera_core::{
    console::Console,
    core::{Died, Score},
};
use chimera_net::net::predict::NetGraph;
use chimera_sim::{
    minigame::ladder::{Ladder, Rung, set_rung},
    weapon::grenade::Detonated,
};
use chimera_world::map::{logic_view::HudMessages, loose::ShownItem};
use combat_arms::game::{
    movement::MovementData,
    special::{CaObject, Kind, PROJECTILE, ProjectileParams, devices::drop_strike_shell},
};
use party_kit::surprise::{ALARM, AirStrike, BLAST_LOOK, LGB, MoonGravity, NAPALM, WARNING};

use super::gun_game::{OPEN, alive, line_up, party, place, rung, soldier};
use super::net_gun_game::joined;

fn shells(world: &mut World) -> Vec<(Option<Entity>, Vec3, &'static str)> {
    world
        .query::<&CaObject>()
        .iter(world)
        .filter(|o| matches!(o.kind, Kind::Impact(_)))
        .map(|o| (o.owner, o.at(), o.weapon))
        .collect()
}

/// Detonations a world has seen.
#[derive(Resource, Default)]
struct Booms(Vec<(&'static str, Vec3)>);

/// Sounds a client played, by entry.
#[derive(Resource, Default)]
struct Heard(Vec<String>);

fn hear(mut reader: MessageReader<chimera_world::map::PlaySound>, mut heard: ResMut<Heard>) {
    heard.0.extend(reader.read().map(|p| p.entry.clone()));
}

fn count_booms(mut reader: MessageReader<Detonated>, mut booms: ResMut<Booms>) {
    for d in reader.read() {
        booms.0.push((d.weapon, d.at));
    }
}

/// An LGB shell dropped on a player on the fourth rung: it falls from
/// above, blows up on him, he dies with no killer, keeps his rung (no
/// "suicide" rung down), the other player gains nothing, and his death
/// counts on his score.
#[test]
fn an_air_strike_shell_kills_with_no_one_credited() {
    let mut sim = party();
    sim.app.init_resource::<Booms>().add_systems(Last, count_booms);
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.0);
    sim.app.world_mut().entity_mut(b).insert(Rung(3));
    line_up(&mut sim, a, b, 30.0);
    let at = OPEN + Vec3::new(0.0, 0.0, -30.0);
    place(&mut sim, b, at);
    sim.seconds(0.2);
    party_kit::surprise::arm(sim.app.world_mut());
    let deaths = sim.app.world().get::<Score>(b).copied().unwrap_or_default().deaths;
    let mut died = MessageCursor::<Died>::default();
    let ground = at - Vec3::Y * 0.95;
    drop_strike_shell(sim.app.world_mut(), LGB, ground, Some(BLAST_LOOK)).expect("the LGB's shell");
    let first = shells(sim.app.world_mut());
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].0, None, "no owner");
    assert!(first[0].1.y > ground.y + 20.0, "it starts high above: {:?}", first[0].1);
    let mut lowest = first[0].1.y;
    let mut killed = None;
    for _ in 0..(4.0 * sim.tick_hz()) as u32 {
        sim.ticks(1);
        if let Some(s) = shells(sim.app.world_mut()).first() {
            assert!(s.1.y <= lowest + 1e-4, "falling");
            lowest = s.1.y;
        }
        let w = sim.app.world();
        for d in died.read(w.resource::<Messages<Died>>()) {
            if d.entity == b {
                killed = Some(d.clone());
            }
        }
        if killed.is_some() {
            break;
        }
    }
    let d = killed.expect("the shell killed him");
    assert_eq!(d.attacker, None, "credited to no one");
    assert_eq!(d.damage.weapon, Some(LGB));
    assert!(!alive(&sim, b));
    let booms = &sim.app.world().resource::<Booms>().0;
    assert!(booms.iter().any(|(w, p)| *w == BLAST_LOOK && p.distance(ground) < 2.0), "blew up on him, as a rocket: {booms:?}");
    sim.seconds(0.5);
    assert_eq!(rung(&sim, b), 3, "no rung down");
    assert_eq!(rung(&sim, a), 0, "no rung up for anyone");
    let s = sim.app.world().get::<Score>(b).copied().unwrap_or_default();
    assert_eq!(s.deaths, deaths + 1, "his death counts");
}

/// A napalm shell leaves its burning sphere; standing in it burns.
#[test]
fn a_napalm_shell_burns() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.0);
    line_up(&mut sim, a, b, 30.0);
    let at = OPEN + Vec3::new(0.0, 0.0, -30.0);
    place(&mut sim, b, at);
    sim.seconds(0.2);
    // Beside him: no blast, the sphere (350 u) reaches him.
    drop_strike_shell(sim.app.world_mut(), NAPALM, at + Vec3::new(2.0, -0.95, 0.0), Some(BLAST_LOOK)).expect("the napalm shell");
    let mut burnt = false;
    for _ in 0..(5.0 * sim.tick_hz()) as u32 {
        sim.ticks(1);
        let w = sim.app.world_mut();
        burnt |= w.query::<&combat_arms::game::grenades::CaDamageSphere>().iter(w).next().is_some();
        if !alive(&sim, b) {
            break;
        }
    }
    assert!(burnt, "a burning sphere");
    assert!(!alive(&sim, b), "80 HP/s burns him down");
}

/// `surprise airstrike 6 1`: nothing for the warning, then six shells of
/// the two designators within the second after it.
#[test]
fn the_command_drops_its_shells_after_the_warning() {
    let mut sim = party();
    let _a = soldier(&mut sim);
    sim.seconds(1.0);
    sim.app.world_mut().resource_mut::<Console>().submit("surprise airstrike 6 1");
    sim.ticks(2);
    sim.seconds(WARNING - 0.2);
    assert_eq!(sim.app.world().resource::<AirStrike>().dropped, 0, "the warning first");
    sim.seconds(1.5);
    let s = sim.app.world().resource::<AirStrike>().clone();
    assert_eq!(s.dropped, 6, "six shells dropped: {s:?}");
    assert!(s.drops.is_empty());
}

/// Over the network: every client sees the warning (map logic's HUD
/// message), the shells in flight (drawn by their record's key) and their
/// explosions.
#[test]
fn clients_see_the_warning_the_shells_and_the_blasts() {
    let mut sim = joined();
    for c in &mut sim.clients {
        c.app.init_resource::<Booms>().init_resource::<Heard>().add_systems(Last, (count_booms, hear));
    }
    sim.server.app.world_mut().resource_mut::<Console>().submit("surprise airstrike 5 0.5");
    sim.ticks(30);
    for (i, c) in sim.clients.iter().enumerate() {
        let hud = c.app.world().resource::<HudMessages>();
        let shown: Vec<String> = hud.channels.iter().flatten().map(|(m, _)| m.text.clone()).collect();
        assert!(shown.iter().any(|t| t.contains("AIR STRIKE")), "client {i}'s HUD: {shown:?}");
        let heard = &c.app.world().resource::<Heard>().0;
        assert!(heard.iter().any(|e| e == ALARM), "client {i} heard the alarm: {heard:?}");
    }
    // Drawn as the RPG-7's rocket: a key every client's map loads.
    let rocket = sim
        .server
        .app
        .world()
        .resource::<chimera_sim::weapon::data::WeaponCatalog>()
        .get(BLAST_LOOK)
        .and_then(|d| d.hook::<ProjectileParams>(PROJECTILE))
        .expect("the rocket's projectile")
        .shown_key();
    let mut seen = [false; 2];
    for _ in 0..600 {
        sim.ticks(1);
        for (i, c) in sim.clients.iter_mut().enumerate() {
            let w = c.app.world_mut();
            seen[i] |= w.query::<&ShownItem>().iter(w).any(|s| s.0 == rocket);
        }
        if sim.server.app.world().resource::<AirStrike>().drops.is_empty()
            && shells(sim.server.app.world_mut()).is_empty()
        {
            break;
        }
    }
    sim.ticks(30);
    for (i, c) in sim.clients.iter().enumerate() {
        assert!(seen[i], "client {i} drew the shells as rockets");
        let booms = &c.app.world().resource::<Booms>().0;
        assert_eq!(booms.len(), 5, "client {i}'s explosions: {booms:?}");
        assert!(booms.iter().all(|(w, _)| *w == BLAST_LOOK), "drawn as the rocket's: {booms:?}");
    }
}

fn cvar(world: &mut World, name: &str) -> Option<String> {
    let get = world.resource::<Console>().cvar(name)?.get.clone();
    get(world)
}

/// `surprise gravity 2 0.2`: CS:S's `sv_gravity` (replicated: the clients
/// take it), HL2:DM's and Combat Arms' movement gravity at a fifth, then
/// back.
#[test]
fn moon_gravity_applies_and_reverts_on_server_and_clients() {
    let mut sim = joined();
    let server = sim.server.app.world_mut();
    let before = cvar(server, "cs_source:sv_gravity").expect("CS:S's sv_gravity");
    let hl2 = cvar(server, "hl2dm:sv_gravity").expect("HL2:DM's sv_gravity");
    let ca = server.resource::<MovementData>().gravity;
    let b: f32 = before.parse().unwrap();
    server.resource_mut::<Console>().submit("surprise gravity 2 0.2");
    sim.ticks(20);
    let server = sim.server.app.world_mut();
    assert_eq!(cvar(server, "cs_source:sv_gravity").unwrap().parse::<f32>().unwrap(), b * 0.2);
    assert_eq!(cvar(server, "hl2dm:sv_gravity").unwrap().parse::<f32>().unwrap(), hl2.parse::<f32>().unwrap() * 0.2);
    assert_eq!(server.resource::<MovementData>().gravity, ca * 0.2);
    for i in 0..2 {
        let w = sim.clients[i].app.world_mut();
        assert_eq!(cvar(w, "cs_source:sv_gravity").unwrap().parse::<f32>().unwrap(), b * 0.2, "client {i} predicts with it");
        let hud = w.resource::<HudMessages>();
        assert!(hud.channels.iter().flatten().any(|(m, _)| m.text.starts_with("Moon gravity!")), "client {i}'s banner: {:?}", hud.channels);
    }
    sim.ticks(200);
    let server = sim.server.app.world_mut();
    assert!(server.resource::<MoonGravity>().until.is_none(), "over");
    assert_eq!(cvar(server, "cs_source:sv_gravity").unwrap(), before);
    assert_eq!(cvar(server, "hl2dm:sv_gravity").unwrap(), hl2);
    assert_eq!(server.resource::<MovementData>().gravity, ca);
    sim.ticks(20);
    for i in 0..2 {
        let w = sim.clients[i].app.world_mut();
        assert_eq!(cvar(w, "cs_source:sv_gravity").unwrap(), before, "client {i} back");
        let hud = w.resource::<HudMessages>();
        assert!(hud.channels.iter().flatten().any(|(m, _)| m.text.starts_with("Gravity is back")), "client {i}'s end");
    }
}

/// Jumps of client 0 for `ticks` ticks: its prediction errors and the
/// worst one, m.
fn jumps(sim: &mut chimera_net::harness::NetSim, ticks: u32) -> (u64, f32) {
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    for k in 0..ticks {
        let me = sim.local_player(0).unwrap();
        let mut w = sim.clients[0].app.world_mut().entity_mut(me);
        let mut i = w.get_mut::<chimera_core::core::Intent>().unwrap();
        i.jump = k % 90 < 3;
        i.move_axis = if k % 180 < 90 { Vec2::Y } else { -Vec2::Y };
        sim.step();
    }
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    (g.errors, g.worst_error)
}

/// A client playing CS:S's movement (the first rung's Glock) jumps under
/// moon gravity: it predicts with the replicated `sv_gravity`, so the
/// server agrees with it after the switch. A Combat Arms rung's movement
/// has no replicated gravity: the server corrects it (the error is
/// printed, and stays small).
#[test]
fn a_cs_source_players_prediction_agrees_under_moon_gravity() {
    let mut sim = joined();
    sim.server.app.world_mut().resource_mut::<Console>().submit("surprise gravity 30 0.2");
    sim.ticks(30);
    let (errors, worst) = jumps(&mut sim, 360);
    assert_eq!(errors, 0, "CS:S's movement predicted with the server's gravity (worst {worst} m)");
    // A Combat Arms gun's rung: Combat Arms' movement.
    let them = sim.character_of(0).unwrap();
    let server = sim.server.app.world_mut();
    let ladder = server.resource::<Ladder>().clone();
    let ca = ladder.rungs.iter().position(|r| r.starts_with("combat_arms:")).expect("a Combat Arms rung");
    set_rung(server, them, ca, &ladder);
    sim.ticks(120);
    let (errors, worst) = jumps(&mut sim, 360);
    println!("Combat Arms' movement under moon gravity: {errors} prediction errors in 360 ticks, worst {worst} m");
    assert!(worst < 0.5, "the corrections stay small: {worst} m");
}
