//! The announcer in a game made of several (specs/combat_arms/
//! kill_streaks.md; `chimera_client::client::announcer`): the party's
//! default pack is Combat Arms', drawn by the shell whatever the local
//! player's game, so a soldier holding a CS:S gun (CS:S's identity and
//! HUD) gets Combat Arms' FIRST KILL, then DOUBLE KILL with its voice;
//! with `sv_announcer none`, nothing. A real groin hit (one round from a
//! CS:S gun through the shared hit code, one from a Combat Arms gun) that
//! kills is a nutshot, with the female voice when the killer's body is a woman's. Headless, on the party's simulation
//! with both HUDs (`hud::party_with_huds`).

use bevy::prelude::*;
use chimera_client::client::announcer::{AnnouncerPlugin, AnnouncerView};
use chimera_core::{
    announcer::ActiveAnnouncer,
    console::Console,
    core::{BodyRegion, Damage, DamageKind, Hitgroup},
};
use chimera_world::harness::Sim;

use crate::{
    cross_game::shape_at,
    gun_game::{held, place},
    hud::{hud, party_with_huds_and, swap_to},
};

fn kill(sim: &mut Sim, me: Entity, victim: Entity) {
    let at = sim.position(victim);
    sim.app.world_mut().write_message(Damage {
        target: victim,
        attacker: Some(me),
        amount: 5.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: Some(cs_source::game::weapons::GLOCK),
        force: Vec3::ZERO,
    });
    sim.ticks(2);
}

/// The banner showing and the voices played so far.
fn shown(sim: &Sim) -> (Option<String>, Vec<String>) {
    let view = sim.app.world().resource::<AnnouncerView>();
    (view.player.shown.as_ref().map(|s| s.request.image.clone()), view.voices.clone())
}

/// Two kills a second apart with the Glock (back on the first rung
/// between them, so the CS:S gun and identity stay).
fn double_kill(sim: &mut Sim, me: Entity, first: Entity, second: Entity) -> [(Option<String>, Vec<String>); 2] {
    assert_eq!(held(sim, me), Some(cs_source::game::weapons::GLOCK));
    assert_eq!(hud(sim), cs_source::game::GAME, "CS:S's identity and HUD");
    kill(sim, me, first);
    let one = shown(sim);
    swap_to(sim, me, 0);
    assert_eq!(held(sim, me), Some(cs_source::game::weapons::GLOCK));
    assert_eq!(hud(sim), cs_source::game::GAME);
    kill(sim, me, second);
    [one, shown(sim)]
}

#[test]
fn a_cs_soldiers_double_kill_gets_combat_arms_announcer() {
    let (mut sim, me, other) = party_with_huds_and(|app| {
        app.add_plugins(AnnouncerPlugin);
    });
    let third = sim.spawn_character(Vec3::new(-3.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.seconds(1.5);
    let active = sim.app.world().resource::<ActiveAnnouncer>().0.clone();
    assert_eq!(active.map(|p| p.id.clone()).as_deref(), Some("combat_arms"), "the party's default");
    let [one, two] = double_kill(&mut sim, me, other, third);
    assert_eq!(one.0.as_deref(), Some("UI_HUD_Message/Effect/FirstKill.dtx"), "the match's first kill");
    assert_eq!(one.1, ["UI_SND/FirstKill.wav"]);
    // FIRST KILL (level 6) still shows: the double kill's banner (5) is
    // dropped, its voice plays all the same (kill_streaks.md case 17).
    assert_eq!(two.0.as_deref(), Some("UI_HUD_Message/Effect/FirstKill.dtx"));
    assert_eq!(two.1, ["UI_SND/FirstKill.wav", "UI_SND/doublekill.wav"]);
    // Gone after its 1.4 s.
    sim.seconds(1.5);
    assert_eq!(shown(&sim).0, None);
}

/// kill_streaks.md B2a: after the first kill, a body kill more than 5 s
/// later is a plain kill: "KILLS" with the server's match kill total (2)
/// carried over the facts, level 1, silent in the stock pack.
#[test]
fn a_plain_kill_shows_kills_with_the_total() {
    let (mut sim, me, other) = party_with_huds_and(|app| {
        app.add_plugins(AnnouncerPlugin);
    });
    let third = sim.spawn_character(Vec3::new(-3.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.seconds(1.5);
    kill(&mut sim, me, other);
    assert_eq!(shown(&sim).0.as_deref(), Some("UI_HUD_Message/Effect/FirstKill.dtx"));
    sim.seconds(6.0);
    swap_to(&mut sim, me, 0);
    kill(&mut sim, me, third);
    let view = sim.app.world().resource::<AnnouncerView>();
    let s = view.player.shown.as_ref().expect("a banner");
    assert_eq!(s.request.image, "UI_HUD_Message/Effect/Kills.dtx");
    assert_eq!((s.request.number, s.request.level), (Some(2), 1));
    assert_eq!(view.voices, ["UI_SND/FirstKill.wav"], "no voice for the plain kill");
}

#[test]
fn no_announcer_when_the_server_says_none() {
    let (mut sim, me, other) = party_with_huds_and(|app| {
        app.add_plugins(AnnouncerPlugin);
    });
    let third = sim.spawn_character(Vec3::new(-3.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.app.world_mut().resource_mut::<Console>().submit("sv_announcer none");
    sim.seconds(1.5);
    assert!(sim.app.world().resource::<ActiveAnnouncer>().0.is_none());
    let [one, two] = double_kill(&mut sim, me, other, third);
    assert_eq!(one, (None, Vec::new()));
    assert_eq!(two, (None, Vec::new()));
}

/// The centre of `victim`'s groin sphere ("Nuts") in the world, on its
/// body's table.
fn groin_of(sim: &Sim, victim: Entity) -> Vec3 {
    use combat_arms::game::weapons::{FEMALE, MALE, body_geometry};
    let w = sim.app.world();
    let table = *w.resource::<chimera_core::core::FemaleBodies>().0.contains(&victim).then_some(&*FEMALE).unwrap_or(&*MALE);
    let (_, _, spheres) = body_geometry(
        w.get::<Transform>(victim).unwrap(),
        w.get::<chimera_core::core::Intent>(victim).unwrap(),
        w.get::<chimera_core::core::MovementState>(victim).unwrap(),
        w.get::<avian3d::prelude::ColliderAabb>(victim),
        table,
    )
    .expect("a body");
    spheres[table.iter().position(|s| s.location == combat_arms::game::weapons::Location::Groin).expect("the groin sphere")].0
}

/// One real round from `me` at the centre of `victim`'s groin sphere,
/// the victim a round from dead; the kills announced so far.
fn groin_round(sim: &mut Sim, me: Entity, victim: Entity, region: chimera_core::core::BodyRegion) -> Vec<chimera_core::announcer::KillAnnounced> {
    sim.app.world_mut().get_mut::<chimera_core::core::Health>(victim).unwrap().current = 0.001;
    let at = shape_at(sim, victim, region);
    chimera_sim::testkit::aim_at(sim, me, at);
    sim.ticks(1);
    sim.intent(me).fire = true;
    sim.ticks(1);
    sim.intent(me).fire = false;
    sim.ticks(6);
    std::mem::take(&mut sim.app.world_mut().resource_mut::<Announced>().0)
}

#[derive(Resource, Default)]
struct Announced(Vec<chimera_core::announcer::KillAnnounced>);

fn note(mut kills: MessageReader<chimera_core::announcer::KillAnnounced>, mut out: ResMut<Announced>) {
    out.0.extend(kills.read().cloned());
}

/// A party with the announcer, `me` (the local player) at rung `rung`
/// and `me` a woman's body when `female` (the groin voice goes by the killer's).
fn groin_party(rung: usize, female: bool) -> (Sim, Entity, Entity) {
    let (mut sim, me, other) = party_with_huds_and(|app| {
        app.add_plugins(AnnouncerPlugin).init_resource::<Announced>().add_systems(Last, note);
    });
    if rung != 0 {
        swap_to(&mut sim, me, rung);
    }
    // The victim in Combat Arms' body (a Combat Arms gun in hand: the
    // body follows the gun's game), with its hit spheres as hitboxes.
    swap_to(&mut sim, other, 1);
    if female {
        sim.app.world_mut().insert_resource(combat_arms::game::characters::CaOwnLook(combat_arms::game::characters::records::Selection { face: 6, ..default() }));
    }
    // Standing on the floor 3 m apart (the spawns are far apart).
    place(&mut sim, me, Vec3::new(0.0, 1.0, 0.0));
    place(&mut sim, other, Vec3::new(0.0, 1.0, -3.0));
    sim.intent(other).yaw = std::f32::consts::PI;
    sim.seconds(1.5);
    let w = sim.app.world();
    assert_eq!(w.resource::<chimera_core::core::FemaleBodies>().0.contains(&me), female);
    (sim, me, other)
}

/// A groin kill, the match's first: one event, FIRST KILL, with the nut
/// over it and only its voice (kill_streaks.md A2, A6; the groin line is
/// only the fall-back of an event without a voice).
fn nutshot(sim: &Sim, kills: &[chimera_core::announcer::KillAnnounced], female: bool) {
    use chimera_core::announcer::SpecialShot;
    assert_eq!(kills.len(), 1, "one kill announced");
    assert_eq!(kills[0].facts.shot, SpecialShot::Groin);
    assert_eq!(kills[0].facts.killer_female, female);
    let (banner, voices) = shown(sim);
    assert_eq!(banner.as_deref(), Some("UI_HUD_Message/Effect/FirstKill.dtx"));
    assert_eq!(top(sim).as_deref(), Some("UI_HUD_Message/Effect/Nut.dtx"));
    assert_eq!(voices, ["UI_SND/FirstKill.wav"], "one voice");
}

fn top(sim: &Sim) -> Option<String> {
    sim.app.world().resource::<AnnouncerView>().player.shown.as_ref().and_then(|s| s.request.top.clone())
}

#[test]
fn a_cs_sources_gun_in_the_groin_is_a_nutshot() {
    let (mut sim, me, other) = groin_party(0, false);
    assert_eq!(held(&sim, me), Some(cs_source::game::weapons::GLOCK));
    let kills = groin_round(&mut sim, me, other, BodyRegion::Groin);
    nutshot(&sim, &kills, false);
}

#[test]
fn a_combat_arms_gun_in_the_groin_is_a_nutshot() {
    let (mut sim, me, other) = groin_party(1, false);
    assert!(held(&sim, me).is_some_and(|g| g.starts_with("combat_arms:")));
    let kills = groin_round(&mut sim, me, other, BodyRegion::Groin);
    nutshot(&sim, &kills, false);
}

/// Combat Arms' female skeleton has no groin sphere (its data has none),
/// so no real hit there is a groin one: a real kill by her says the
/// body is a woman's, and the same kill as a lone groin shot (not the
/// match's first), sent on to the shell, plays the female line, and a
/// man's the male one (the killer's body, not the victim's).
#[test]
fn a_womans_groin_kill_plays_the_female_voice() {
    use chimera_core::announcer::{KillAnnounced, SpecialShot};
    let (mut sim, me, other) = groin_party(1, true);
    let kills = groin_round(&mut sim, me, other, BodyRegion::Head);
    assert_eq!(kills.len(), 1);
    assert!(kills[0].facts.killer_female, "her body");
    assert_eq!(kills[0].facts.shot, SpecialShot::Head);
    sim.seconds(2.0);
    let mut groin: KillAnnounced = kills[0].clone();
    groin.facts.shot = SpecialShot::Groin;
    groin.facts.first = false;
    groin.facts.multi = 1;
    sim.app.world_mut().write_message(groin.clone());
    sim.ticks(2);
    let (banner, voices) = shown(&sim);
    assert_eq!(banner.as_deref(), Some("UI_HUD_Message/Effect/NutShot.dtx"));
    assert_eq!(top(&sim), None, "a nutshot has no nut over it");
    assert_eq!(voices.last().map(String::as_str), Some("UI_SND/NutShot_Female.wav"), "{voices:?}");
    groin.facts.killer_female = false;
    sim.app.world_mut().write_message(groin);
    sim.ticks(2);
    assert_eq!(shown(&sim).1.last().map(String::as_str), Some("UI_SND/NutShot_Male.wav"));
}
