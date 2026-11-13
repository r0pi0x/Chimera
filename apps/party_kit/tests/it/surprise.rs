//! The host's surprises (`party_kit::surprise`): `surprise airstrike`'s
//! shells fall, blow up and kill with no one credited (no rung up or down),
//! clients hear the siren, see the warning and the shells as their own
//! record and their explosions as the designators'; `surprise gravity`
//! scales every movement's gravity and the physics world's and puts them
//! back, a client's replicated settings following (its ragdolls' and
//! props' gravity too), its prediction of a CS:S and a Combat Arms player
//! agreeing with the server.

use avian3d::prelude::Gravity;
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
    special::{CaObject, DEVICE, DeviceParams, Kind, devices::drop_strike_shell},
};
use party_kit::surprise::{AirStrike, LGB, MoonGravity, NAPALM, WARNING, alarm_entry};

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

/// The drawn key of designator `id`'s shell (its own record's).
fn shell_key(world: &World, id: &str) -> String {
    world
        .resource::<chimera_sim::weapon::data::WeaponCatalog>()
        .get(id)
        .and_then(|d| d.hook::<DeviceParams>(DEVICE))
        .and_then(|d| d.strike)
        .expect("a designator's strike")
        .shell
        .shown_key()
}

/// An LGB shell dropped on a player on the fourth rung: it falls from
/// above nose first, drawn as its own record, blows up on him as the designator's
/// shells do, he dies with no killer, keeps his rung (no
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
    let shell = drop_strike_shell(sim.app.world_mut(), LGB, ground).expect("the LGB's shell");
    let drawn = sim.app.world().get::<ShownItem>(shell).expect("drawn").0.clone();
    assert_eq!(drawn, shell_key(sim.app.world(), LGB), "drawn as its own record");
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
            // Nose first (playtest 2026-11-13: it fell on its side): the
            // shell's model stands upright, its nose its -Y.
            let t = sim.app.world().get::<Transform>(shell).expect("the shell's transform");
            let nose = t.rotation * Vec3::NEG_Y;
            assert!(nose.angle_between(Vec3::NEG_Y).to_degrees() < 3.0, "its nose {nose:?} along its fall");
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
    assert!(booms.iter().any(|(w, p)| *w == LGB && p.distance(ground) < 2.0), "blew up on him, as the LGB's shells: {booms:?}");
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
    drop_strike_shell(sim.app.world_mut(), NAPALM, at + Vec3::new(2.0, -0.95, 0.0)).expect("the napalm shell");
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

/// Over the network: every client hears the siren and sees the warning
/// (map logic's HUD message), the shells in flight (drawn by their own
/// record's key) and their explosions (the designators' weapons: their
/// shells' impact records).
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
        assert!(heard.iter().any(|e| *e == alarm_entry()), "client {i} heard the siren: {heard:?}");
        assert!(!heard.iter().any(|e| e.starts_with("Radio.")), "no radio call: {heard:?}");
    }
    // Drawn as their own record: keys every client's map loads.
    let keys = [shell_key(sim.server.app.world(), LGB), shell_key(sim.server.app.world(), NAPALM)];
    let mut seen = [false; 2];
    for _ in 0..600 {
        sim.ticks(1);
        for (i, c) in sim.clients.iter_mut().enumerate() {
            let w = c.app.world_mut();
            seen[i] |= w.query::<&ShownItem>().iter(w).any(|s| keys.contains(&s.0));
        }
        if sim.server.app.world().resource::<AirStrike>().drops.is_empty()
            && shells(sim.server.app.world_mut()).is_empty()
        {
            break;
        }
    }
    sim.ticks(30);
    for (i, c) in sim.clients.iter().enumerate() {
        assert!(seen[i], "client {i} drew the shells as their record");
        let booms = &c.app.world().resource::<Booms>().0;
        assert_eq!(booms.len(), 5, "client {i}'s explosions: {booms:?}");
        assert!(booms.iter().all(|(w, _)| [LGB, NAPALM].contains(w)), "drawn as the designators' shells: {booms:?}");
    }
}

fn cvar(world: &mut World, name: &str) -> Option<String> {
    let get = world.resource::<Console>().cvar(name)?.get.clone();
    get(world)
}

/// `surprise gravity 2 0.2`: CS:S's `sv_gravity`, HL2:DM's, Combat
/// Arms' movement gravity and the physics world's at a fifth, then back;
/// the clients take them all (replicated): their prediction's and their
/// own ragdolls' and props' gravity.
#[test]
fn moon_gravity_applies_and_reverts_on_server_and_clients() {
    let mut sim = joined();
    let server = sim.server.app.world_mut();
    let before = cvar(server, "cs_source:sv_gravity").expect("CS:S's sv_gravity");
    let hl2 = cvar(server, "hl2dm:sv_gravity").expect("HL2:DM's sv_gravity");
    let ca = server.resource::<MovementData>().gravity;
    let physics = server.resource::<Gravity>().0;
    let b: f32 = before.parse().unwrap();
    server.resource_mut::<Console>().submit("surprise gravity 2 0.2");
    sim.ticks(20);
    let server = sim.server.app.world_mut();
    assert_eq!(cvar(server, "cs_source:sv_gravity").unwrap().parse::<f32>().unwrap(), b * 0.2);
    assert_eq!(cvar(server, "hl2dm:sv_gravity").unwrap().parse::<f32>().unwrap(), hl2.parse::<f32>().unwrap() * 0.2);
    assert_eq!(server.resource::<MovementData>().gravity, ca * 0.2);
    assert!((server.resource::<Gravity>().0 - physics * 0.2).length() < 1e-4, "the server's physics");
    for i in 0..2 {
        let w = sim.clients[i].app.world_mut();
        assert_eq!(cvar(w, "cs_source:sv_gravity").unwrap().parse::<f32>().unwrap(), b * 0.2, "client {i} predicts with it");
        assert!((w.resource::<MovementData>().gravity - ca * 0.2).abs() < 1e-3, "client {i}'s Combat Arms movement");
        assert!(
            (w.resource::<Gravity>().0 - physics * 0.2).length() < 1e-4,
            "client {i}'s ragdolls and props fall at the server's: {:?}",
            w.resource::<Gravity>().0
        );
        let hud = w.resource::<HudMessages>();
        assert!(hud.channels.iter().flatten().any(|(m, _)| m.text.starts_with("Moon gravity!")), "client {i}'s banner: {:?}", hud.channels);
    }
    sim.ticks(200);
    let server = sim.server.app.world_mut();
    assert!(server.resource::<MoonGravity>().until.is_none(), "over");
    assert_eq!(cvar(server, "cs_source:sv_gravity").unwrap(), before);
    assert_eq!(cvar(server, "hl2dm:sv_gravity").unwrap(), hl2);
    assert_eq!(server.resource::<MovementData>().gravity, ca);
    assert_eq!(server.resource::<Gravity>().0, physics);
    sim.ticks(20);
    for i in 0..2 {
        let w = sim.clients[i].app.world_mut();
        assert_eq!(cvar(w, "cs_source:sv_gravity").unwrap(), before, "client {i} back");
        assert_eq!(w.resource::<MovementData>().gravity, ca, "client {i}'s Combat Arms movement back");
        assert_eq!(w.resource::<Gravity>().0, physics, "client {i}'s physics back");
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
/// server agrees with it after the switch; on a Combat Arms rung it
/// predicts with the replicated `combat_arms:sv_gravity`: no corrections
/// either.
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
    assert_eq!(errors, 0, "Combat Arms' movement predicted with the server's gravity (worst {worst} m)");
}

/// Effect groups and Combat Arms sounds a world asked for.
#[derive(Resource, Default)]
struct Shown {
    groups: Vec<String>,
    sounds: Vec<String>,
}

fn show(
    mut fx: MessageReader<combat_arms::game::effects::CaFxRequest>,
    mut sounds: MessageReader<combat_arms::game::effects::sounds::CaSound>,
    mut shown: ResMut<Shown>,
) {
    shown.groups.extend(fx.read().map(|r| r.group.clone()));
    shown.sounds.extend(sounds.read().map(|s| s.file.clone()));
}

/// A listen server's host (playtest 2026-11-13: the host saw no warning,
/// heard no siren and hardly any explosion): the surprise shows on the
/// host's own HUD and screen, the siren plays there, and the shells blow
/// up there as the designators' do (their impact records' groups and
/// sounds), as on a network client; moon gravity's banner too.
#[test]
fn a_listen_hosts_own_player_sees_and_hears_the_surprises() {
    let mut sim = joined();
    let host = sim.server.spawn_character(Vec3::new(0.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.server.app.world_mut().entity_mut(host).insert(chimera_core::core::LocalPlayer);
    sim.server
        .app
        .init_resource::<Booms>()
        .init_resource::<Heard>()
        .init_resource::<Shown>()
        .add_systems(Last, (count_booms, hear))
        .add_systems(PostUpdate, show);
    sim.ticks(30);
    sim.server.app.world_mut().resource_mut::<Console>().submit("surprise airstrike 5 0.5");
    sim.ticks(30);
    let w = sim.server.app.world();
    let shown: Vec<String> = w
        .get_resource::<HudMessages>()
        .map(|h| h.channels.iter().flatten().map(|(m, _)| m.text.clone()).collect())
        .unwrap_or_default();
    assert!(shown.iter().any(|t| t.contains("AIR STRIKE")), "the host's HUD: {shown:?}");
    let fades = w.get_resource::<chimera_world::map::logic_view::ScreenFades>();
    assert!(fades.is_some_and(|f| f.fades.iter().any(|f| f.0.color == [255, 0, 0])), "the host's red flash");
    let heard = &w.resource::<Heard>().0;
    assert!(heard.iter().any(|e| *e == alarm_entry()), "the host heard the siren: {heard:?}");
    for _ in 0..600 {
        sim.ticks(1);
        if sim.server.app.world().resource::<AirStrike>().drops.is_empty()
            && shells(sim.server.app.world_mut()).is_empty()
        {
            break;
        }
    }
    sim.ticks(30);
    let w = sim.server.app.world();
    let booms = &w.resource::<Booms>().0;
    assert_eq!(booms.len(), 5, "the shells blew up: {booms:?}");
    // A shell blows up on the host's screen as its designator's data says
    // (its `combat_arms:effects` impact record: the install's
    // `Dirt_Burster` for the LGB), though nobody here holds the
    // designator. Without the install the data has no record: a
    // designator of the LGB's with a built-in one.
    use chimera_sim::weapon::data::{WeaponCatalog, WeaponSet};
    use combat_arms::game::effects::data::{CaFxTables, CaWeaponFx, EFFECTS};
    let doc = "id = \"combat_arms:test_strike\"\nbase = \"combat_arms:lgb\"\n\
               [hooks.\"combat_arms:effects\"]\nimpact = \"flashbang_exp\"\n";
    let w = sim.server.app.world_mut();
    let known: Vec<&WeaponSet> = w.resource::<WeaponCatalog>().sets.clone();
    let set: &'static WeaponSet = Box::leak(Box::new(WeaponSet::parse(&[("test", doc)], &known).expect("the test designator")));
    w.resource_mut::<WeaponCatalog>().sets.push(set);
    let fx = w.resource::<WeaponCatalog>().get("combat_arms:test_strike").and_then(|d| d.hook::<CaWeaponFx>(EFFECTS)).expect("its effects");
    let record = fx.impact_of(w.resource::<CaFxTables>()).expect("its impact record").clone();
    *w.resource_mut::<Shown>() = Shown::default();
    let at = w.get::<Transform>(host).unwrap().translation;
    drop_strike_shell(w, "combat_arms:test_strike", at + Vec3::new(3.0, -0.95, 0.0)).expect("a shell");
    sim.ticks(240);
    let shown = sim.server.app.world().resource::<Shown>();
    assert!(shown.groups.contains(&record.fx), "the explosion ({}) drawn on the host: {:?}", record.fx, shown.groups);
    assert!(shown.sounds.contains(&record.sound), "the explosion ({}) heard on the host: {:?}", record.sound, shown.sounds);
    // Moon gravity's banner.
    sim.server.app.world_mut().resource_mut::<Console>().submit("surprise gravity 8 0.2");
    sim.ticks(20);
    let hud = sim.server.app.world().resource::<HudMessages>();
    assert!(hud.channels.iter().flatten().any(|(m, _)| m.text.starts_with("Moon gravity!")), "the host's banner: {:?}", hud.channels);
}
