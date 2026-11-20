//! The sandbox's practice targets (chimera_sim's `minigame::targets`, the
//! bots behind them Combat Arms'): placed at the distance asked on the
//! floor, refused outside the sandbox and past the floor, a protected one
//! measured but unhurt, a killed one back after its delay with its
//! settings, a still one that may not fire neither fires nor moves, and a
//! kill's shots and hits counted. On the party's simulation, headless.

use bevy::prelude::*;
use chimera_core::{
    console::Console,
    core::{Damage, DamageKind, Dead, Health, Hitgroup, LocalPlayer, SimClock},
};
use chimera_sim::{
    minigame::{
        self,
        targets::{self, HitMeasured, KillMeasured, SetTarget, SpawnTarget, Target, TargetMotion, TargetRefused, TargetRespawn},
    },
    weapon::{WeaponEvent, WeaponEventKind},
};
use chimera_world::harness::Sim;
use combat_arms::game::movement;
use ultimate_gun_game::sandbox::SANDBOX;

use crate::parts::ugg;

const GUN: &str = "combat_arms:m4a1";

/// What the facts said, kept.
#[derive(Resource, Default)]
struct Seen {
    hits: Vec<HitMeasured>,
    kills: Vec<KillMeasured>,
    shots: Vec<Entity>,
    refused: Vec<String>,
}

fn keep_refused(mut seen: ResMut<Seen>, mut r: MessageReader<TargetRefused>) {
    seen.refused.extend(r.read().map(|r| r.reason.clone()));
}

fn keep(mut seen: ResMut<Seen>, mut h: MessageReader<HitMeasured>, mut k: MessageReader<KillMeasured>, mut w: MessageReader<WeaponEvent>) {
    seen.hits.extend(h.read().cloned());
    seen.kills.extend(k.read().cloned());
    for e in w.read() {
        if matches!(e.kind, WeaponEventKind::ShotMade { .. } | WeaponEventKind::Fired { .. }) {
            seen.shots.push(e.owner);
        }
    }
}

/// The sandbox playing with the local player `me` (where its spawn put it).
fn play() -> (Sim, Entity) {
    let mut sim = ugg();
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.app.init_resource::<Seen>().add_systems(FixedPostUpdate, keep).add_systems(PostUpdate, keep_refused);
    sim.seconds(1.5);
    (sim, me)
}

fn add(sim: &mut Sim, req: SpawnTarget) -> Entity {
    let e = targets::spawn(sim.app.world_mut(), &req).unwrap_or_else(|e| panic!("the target: {e}"));
    sim.seconds(1.5);
    e
}

fn at(distance_m: f32) -> SpawnTarget {
    SpawnTarget { gun: Some(GUN.into()), distance_m, ..default() }
}

fn hit(sim: &mut Sim, by: Entity, victim: Entity, amount: f32) {
    let point = sim.position(victim);
    sim.app.world_mut().write_message(Damage {
        target: victim,
        attacker: Some(by),
        amount,
        point,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: Some(GUN),
        force: Vec3::ZERO,
    });
}

fn shot(sim: &mut Sim, by: Entity, toward: Vec3) {
    let origin = sim.position(by) + Vec3::Y * 0.6;
    sim.app.world_mut().write_message(WeaponEvent {
        owner: by,
        weapon: Entity::PLACEHOLDER,
        kind: WeaponEventKind::ShotMade { id: GUN, origin, dir: (toward - origin).normalize() },
        replay: false,
    });
}

#[test]
fn a_target_stands_on_the_floor_at_the_distance_asked() {
    let (mut sim, me) = play();
    let e = add(&mut sim, at(10.0));
    let t = sim.app.world().get::<Target>(e).cloned().expect("a target");
    let mine = sim.position(me);
    assert!(((t.home - mine).xz().length() - 10.0).abs() < 0.05, "10 m out: {:?} from {mine:?}", t.home);
    let yaw = sim.app.world().get::<chimera_core::core::Intent>(me).unwrap().yaw;
    let view = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    assert!((t.home - mine).xz().normalize().dot(view.xz()) > 0.999, "along the view {view:?}: {:?}", t.home);
    assert_eq!((t.gun.as_str(), t.game.as_str(), t.character.as_str()), (GUN, "combat_arms", "combat_arms:default"));
    let body = sim.position(e);
    assert!((body - t.home).xz().length() < 0.5, "the body at home: {body:?} {:?}", t.home);
    assert!(sim.app.world().get::<Dead>(e).is_none(), "alive");
    // Past the greybox's floor: refused, nothing added.
    let far = targets::spawn(sim.app.world_mut(), &at(5000.0)).unwrap_err();
    assert!(far.contains("no floor"), "{far}");
    let list = {
        let before = sim.app.world().resource::<Console>().output.len();
        sim.app.world_mut().resource_mut::<Console>().submit("target list");
        sim.ticks(2);
        sim.app.world().resource::<Console>().output[before..].iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
    };
    assert!(list.contains("1 targets") && list.contains(GUN), "{list}");
}

#[test]
fn targets_are_refused_outside_the_sandbox() {
    let mut sim = ugg();
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(0.5);
    let e = targets::spawn(sim.app.world_mut(), &at(10.0)).unwrap_err();
    assert!(e.contains("sandbox"), "{e}");
}

#[test]
fn a_protected_target_is_measured_but_keeps_its_health() {
    let (mut sim, me) = play();
    let e = add(&mut sim, SpawnTarget { protected: true, ..at(8.0) });
    for _ in 0..4 {
        hit(&mut sim, me, e, 0.4);
        sim.ticks(2);
    }
    sim.ticks(2);
    let h = sim.app.world().get::<Health>(e).copied().unwrap();
    assert!(h.current >= h.max - 1e-4 && sim.app.world().get::<Dead>(e).is_none(), "unhurt: {h:?}");
    let seen = sim.app.world().resource::<Seen>();
    let on: Vec<&HitMeasured> = seen.hits.iter().filter(|m| m.victim == e).collect();
    assert_eq!(on.len(), 4, "every hit measured: {:?}", seen.hits);
    assert!(on.iter().all(|m| m.damage > 0.0 && m.attacker == Some(me) && m.gun == Some(GUN)), "{on:?}");
    assert!(on.iter().all(|m| (m.distance_m - 8.0).abs() < 1.0), "{on:?}");
    assert!(seen.kills.is_empty(), "no kill");
}

#[test]
fn a_killed_target_returns_after_its_delay_with_its_settings() {
    let (mut sim, me) = play();
    let e = add(&mut sim, SpawnTarget { motion: TargetMotion::Strafes, respawn_secs: 2.0, ..at(12.0) });
    let before = sim.app.world().get::<Target>(e).cloned().unwrap();
    hit(&mut sim, me, e, 50.0);
    sim.ticks(3);
    let now = sim.app.world().resource::<SimClock>().now;
    let back = sim.app.world().get::<TargetRespawn>(e).copied().expect("dead, its return set");
    assert!(sim.app.world().get::<Dead>(e).is_some());
    assert!((back.at - now - 2.0).abs() < 0.2, "2 s after the death: {back:?} now {now}");
    sim.seconds(1.0);
    assert!(sim.app.world().get::<Dead>(e).is_some(), "still dead a second on");
    sim.seconds(1.5);
    assert!(sim.app.world().get::<Dead>(e).is_none() && sim.app.world().get::<TargetRespawn>(e).is_none(), "back");
    let after = sim.app.world().get::<Target>(e).cloned().unwrap();
    assert_eq!(after, before, "the same settings");
    let body = sim.position(e);
    assert!((body - before.home).xz().length() < targets::STRAFE_HALF + 1.0, "at home: {body:?} {:?}", before.home);
}

#[test]
fn a_still_target_that_may_not_fire_neither_fires_nor_moves() {
    let (mut sim, me) = play();
    let e = add(&mut sim, at(8.0));
    let from = sim.position(e);
    sim.seconds(6.0);
    let to = sim.position(e);
    assert!((to - from).xz().length() < 0.3, "it stood: {from:?} -> {to:?}");
    let seen = sim.app.world().resource::<Seen>();
    assert!(!seen.shots.contains(&e), "it never fired");
    assert!(sim.app.world().get::<Dead>(me).is_none(), "the player is unhurt");
    // It faces back toward where it was placed from, level.
    let t = sim.app.world().get::<Target>(e).cloned().unwrap();
    let i = sim.app.world().get::<chimera_core::core::Intent>(e).cloned().unwrap();
    let facing = Vec3::new(-i.yaw.sin(), 0.0, -i.yaw.cos());
    assert!(facing.dot(-t.dir) > 0.999 && i.pitch == 0.0, "facing {facing:?} against its line {:?}", t.dir);
}

#[test]
fn a_kill_counts_its_shots_and_hits() {
    let (mut sim, me) = play();
    let e = add(&mut sim, at(10.0));
    let c = sim.position(e);
    shot(&mut sim, me, c);
    hit(&mut sim, me, e, 0.3);
    sim.ticks(3);
    // A near miss: at the target, no hit.
    shot(&mut sim, me, c + Vec3::X * 0.5);
    sim.ticks(3);
    shot(&mut sim, me, c);
    hit(&mut sim, me, e, 50.0);
    sim.ticks(3);
    let seen = sim.app.world().resource::<Seen>();
    let k = seen.kills.iter().find(|k| k.victim == e).unwrap_or_else(|| panic!("the kill: {:?}", seen.kills));
    assert_eq!((k.shots_fired, k.shots_hit, k.headshots), (3, 2, 0), "{k:?}");
    assert_eq!(k.attacker, Some(me));
    assert!(k.from_first_shot > 0.05 && (k.from_first_shot - k.from_first_hit).abs() < 1e-4, "{k:?}");
    assert!((k.distance_m - 10.0).abs() < 1.0 && k.total_damage > 0.3, "{k:?}");
    // A target's death counts for no one (no score, no rung).
    let score = sim.app.world().get::<chimera_core::core::Score>(me).map_or(0, |s| s.kills);
    assert_eq!(score, 0, "no kill counted for a practice target");
}

fn home_of(sim: &Sim, e: Entity) -> Vec3 {
    sim.app.world().get::<Target>(e).unwrap().home
}

#[test]
fn a_walking_target_wanders_and_stays_near_home() {
    let (mut sim, _) = play();
    let e = add(&mut sim, SpawnTarget { motion: TargetMotion::Walks, ..at(10.0) });
    let home = home_of(&sim, e);
    let (mut far, mut path, mut last) = (0.0f32, 0.0f32, sim.position(e));
    for _ in 0..20 {
        sim.seconds(0.5);
        let p = sim.position(e);
        far = far.max((p - home).xz().length());
        path += (p - last).xz().length();
        last = p;
    }
    assert!(path > 2.0, "it walked: {path} m");
    assert!(far <= targets::WALK_RADIUS + targets::LEASH + 0.5, "near home: {far} m");
}

#[test]
fn a_target_that_shoots_back_fights_from_its_spot_and_its_hits_are_measured() {
    let (mut sim, me) = play();
    let e = add(&mut sim, SpawnTarget { shoots_back: true, ..at(8.0) });
    let home = home_of(&sim, e);
    // It fights as its game's bot does, seeing what its bot looks at (one
    // left alone looks the way its patrol would go, away from the player
    // from some spots): the player stands in that look, 8 m off.
    let yaw = sim.app.world().get::<chimera_core::core::Intent>(e).unwrap().yaw;
    let to = sim.position(e) + Vec3::new(-yaw.sin(), 0.0, -yaw.cos()) * 8.0;
    sim.app.world_mut().entity_mut(me).insert((
        Transform::from_translation(to),
        avian3d::prelude::Position(to),
        chimera_core::core::Velocity(Vec3::ZERO),
    ));
    let mut hit_me = false;
    for _ in 0..40 {
        sim.seconds(0.5);
        let seen = sim.app.world().resource::<Seen>();
        if seen.hits.iter().any(|h| h.attacker == Some(e) && h.victim == me) {
            hit_me = true;
            break;
        }
    }
    let seen = sim.app.world().resource::<Seen>();
    assert!(seen.shots.contains(&e), "it fired");
    assert!(hit_me, "its hits on the player measured: {:?}", seen.hits);
    assert!((sim.position(e) - home).xz().length() < 1.5, "from where it stands");
}

#[test]
fn a_new_distance_replaces_the_target_along_its_line() {
    let (mut sim, _) = play();
    let e = add(&mut sim, at(8.0));
    let t = sim.app.world().get::<Target>(e).cloned().unwrap();
    sim.app.world_mut().write_message(SetTarget { distance_m: Some(14.0), ..SetTarget::of(e) });
    sim.seconds(1.5);
    let now = sim.app.world().get::<Target>(e).cloned().unwrap();
    let want = t.origin + t.dir * 14.0;
    assert!((now.home - want).xz().length() < 0.05 && now.distance_m == 14.0, "{:?} vs {want:?}", now.home);
    assert!((sim.position(e) - now.home).xz().length() < 0.5, "the body moved there");
    // A refused request is a message too.
    sim.app.world_mut().write_message(SpawnTarget { distance_m: 5000.0, gun: Some(GUN.into()), ..default() });
    sim.seconds(0.2);
    let seen = sim.app.world().resource::<Seen>();
    assert!(seen.refused.iter().any(|r| r.contains("no floor")), "{:?}", seen.refused);
}

#[test]
fn a_target_stands_and_is_hit_100_m_away_on_the_greybox_diagonal() {
    let (mut sim, me) = play();
    // The greybox's longest clear line: corner to corner (its walls are
    // 80 m apart), about 107 m from one corner's player.
    let corner = Transform::from_xyz(-38.0, 1.0, -38.0);
    chimera_sim::spawning::revive(sim.app.world_mut(), me, Some(corner));
    chimera_core::core::set_view(sim.app.world_mut(), me, -3.0 * std::f32::consts::FRAC_PI_4, 0.0);
    sim.ticks(2);
    let e = add(&mut sim, at(100.0));
    let t = sim.app.world().get::<Target>(e).cloned().unwrap();
    assert!((t.home.xz() - Vec2::new(-38.0 + 70.71, -38.0 + 70.71)).length() < 0.5, "{:?}", t.home);
    assert!(targets::spawn(sim.app.world_mut(), &at(115.0)).is_err(), "past the far corner: refused");
    hit(&mut sim, me, e, 0.2);
    sim.ticks(3);
    let seen = sim.app.world().resource::<Seen>();
    let h = seen.hits.iter().find(|h| h.victim == e).expect("measured");
    assert!((h.distance_m - 100.0).abs() < 1.5, "{h:?}");
}

#[test]
fn a_tf2_target_wears_a_class_other_than_its_rung_drew() {
    let (mut sim, _) = play();
    const SHOTGUN: &str = "tf2:tf_weapon_shotgun_primary";
    let ladder = sim.app.world().resource::<chimera_sim::minigame::ladder::Ladder>().clone();
    let rung = ladder.rungs.iter().position(|r| *r == SHOTGUN).expect("the shotgun's rung");
    let drawn = ladder.extras(rung).and_then(|x| x.value("class")).map(str::to_string);
    let (class, health) = [("soldier", 2.0), ("pyro", 1.75), ("heavy", 3.0), ("engineer", 1.25)]
        .into_iter()
        .find(|(c, _)| drawn.as_deref() != Some(*c))
        .unwrap();
    let e = add(&mut sim, SpawnTarget { character: Some(format!("tf2:{class}")), gun: Some(SHOTGUN.into()), ..at(10.0) });
    sim.seconds(1.0);
    let h = sim.app.world().get::<Health>(e).copied().unwrap();
    assert!((h.max - health).abs() < 1e-3, "{class} (drawn {drawn:?}): {h:?}");
    assert_eq!(sim.app.world().get::<chimera_sim::weapon::Armor>(e).map(|a| a.amount).unwrap_or(0.0), 0.0, "no other game's armour");
}

#[test]
fn an_unarmoured_target_has_no_second_bar() {
    let (mut sim, me) = play();
    let e = add(&mut sim, SpawnTarget { character: Some("halo_ce:spartan".into()), armored: false, distance_m: 8.0, ..default() });
    sim.seconds(1.0);
    let v = *sim.app.world().get::<targets::TargetVitals>(e).unwrap();
    assert_eq!((v.shield, v.shield_max), (0.0, 0.0), "{v:?}");
    hit(&mut sim, me, e, 0.3);
    sim.ticks(3);
    let h = sim.app.world().resource::<Seen>().hits.iter().find(|h| h.victim == e).cloned().expect("measured");
    assert!(h.health_after < h.health_before && h.shield_before == 0.0, "the body takes it: {h:?}");
    // On again: the body's game gives its own back after the next placing.
    let t = sim.app.world().get::<Target>(e).cloned().unwrap();
    assert!(!t.armored && t.game_name == "Halo: Combat Evolved", "{t:?}");
}

#[test]
fn the_bodies_list_their_numbers_guns_and_full_game_names() {
    let (sim, _) = play();
    let all = targets::characters(sim.app.world());
    let of = |id: &str| all.iter().find(|c| c.id == id).cloned().unwrap_or_else(|| panic!("{id} in {all:?}"));
    let spartan = of("halo_ce:spartan");
    assert_eq!(spartan.game_name, "Halo: Combat Evolved");
    assert!(spartan.health_max == 1.0 && spartan.shield_max > 0.0, "{spartan:?}");
    assert!(spartan.guns.iter().any(|g| g == "halo_ce:sniper_rifle") && spartan.guns.iter().all(|g| g.starts_with("halo_ce:")));
    let heavy = of("tf2:heavy");
    assert!((heavy.health_max - 3.0).abs() < 1e-3 && heavy.shield_max == 0.0, "{heavy:?}");
    assert!(heavy.guns.iter().any(|g| g == "tf2:tf_weapon_shotgun_primary"), "{heavy:?}");
    let ca = of("combat_arms:default");
    assert_eq!((ca.game_name.as_str(), ca.game_tag.as_str(), ca.shield_max), ("Combat Arms", "CA", 1.0));
}

/// What a target's shots were aimed along (yaw, pitch), kept.
#[derive(Resource, Default)]
struct Aims(Vec<(Entity, f32, f32)>);

fn keep_aims(mut aims: ResMut<Aims>, mut w: MessageReader<WeaponEvent>) {
    for e in w.read() {
        if let WeaponEventKind::Fired { yaw, pitch, .. } = e.kind {
            aims.0.push((e.owner, yaw, pitch));
        }
    }
}

/// `ahead` (TF2's `bot_forceattack`, a staging mode for pictures): its
/// trigger held, every shot level along its line back toward where it
/// was placed from, whoever stands there or not.
#[test]
fn a_target_firing_ahead_holds_its_trigger_level_along_its_line() {
    for (body, gun) in [(None, GUN), (Some("tf2:pyro"), "tf2:tf_weapon_flamethrower"), (Some("tf2:heavy"), "tf2:tf_weapon_minigun")] {
        fires_ahead(body, gun);
    }
}

fn fires_ahead(body: Option<&str>, gun: &str) {
    let (mut sim, me) = play();
    sim.app.init_resource::<Aims>().add_systems(FixedPostUpdate, keep_aims);
    let e = add(&mut sim, SpawnTarget { character: body.map(Into::into), gun: Some(gun.into()), ..at(8.0) });
    sim.app.world_mut().write_message(SetTarget { fires_ahead: Some(true), ..SetTarget::of(e) });
    // The player steps out of its line: it fires all the same.
    let away = sim.position(me) + Vec3::new(6.0, 0.0, 0.0);
    sim.app.world_mut().entity_mut(me).insert((Transform::from_translation(away), avian3d::prelude::Position(away)));
    sim.seconds(2.0);
    let t = sim.app.world().get::<Target>(e).cloned().unwrap();
    assert!(t.fires_ahead);
    let line_yaw = t.dir.x.atan2(t.dir.z);
    let aims: Vec<_> = sim.app.world().resource::<Aims>().0.iter().filter(|a| a.0 == e).copied().collect();
    assert!(aims.len() > 3, "it kept firing: {aims:?}");
    for (_, yaw, pitch) in &aims {
        let dyaw = (yaw - line_yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        assert!(dyaw.abs() < 0.05 && pitch.abs() < 0.05, "{gun} level along its line: yaw {yaw} (line {line_yaw}), pitch {pitch}");
    }
    sim.app.world_mut().write_message(SetTarget { fires_ahead: Some(false), ..SetTarget::of(e) });
    sim.seconds(0.5);
    let before = sim.app.world().resource::<Aims>().0.len();
    sim.seconds(1.0);
    assert_eq!(sim.app.world().resource::<Aims>().0.len(), before, "it stopped");
}

/// What a target's trigger read where the guns' selection stage sees it
/// (the Minigun's spin reads `Intent::fire` there).
#[derive(Resource, Default)]
struct TriggerAtSelect(Vec<Entity>);

fn keep_trigger_at_select(mut seen: ResMut<TriggerAtSelect>, q: Query<(Entity, &chimera_core::core::Intent), With<Target>>) {
    seen.0.extend(q.iter().filter(|(_, i)| i.fire || i.secondary).map(|(e, _)| e));
}

/// A target that may not fire has its trigger released before the
/// guns' selection stage too (`core::Predict::Select`: TF2's Minigun
/// spins up there), not only before the weapons fire: its brain, facing
/// the player, wants to fire every tick.
#[test]
fn a_target_that_may_not_fire_has_its_trigger_released_before_the_guns_look() {
    let (mut sim, _me) = play();
    sim.app
        .init_resource::<TriggerAtSelect>()
        .add_systems(chimera_core::core::Predict::Select, keep_trigger_at_select.before(chimera_sim::weapon::SelectWeapons));
    let e = add(&mut sim, at(8.0));
    sim.app.world_mut().resource_mut::<TriggerAtSelect>().0.clear();
    sim.seconds(4.0);
    let held = sim.app.world().resource::<TriggerAtSelect>().0.iter().filter(|&&x| x == e).count();
    assert_eq!(held, 0, "its trigger seen held at selection in {held} ticks");
    // With `ahead` it is held there.
    sim.app.world_mut().write_message(SetTarget { fires_ahead: Some(true), ..SetTarget::of(e) });
    sim.seconds(1.0);
    assert!(sim.app.world().resource::<TriggerAtSelect>().0.contains(&e), "held when firing ahead");
    sim.app.world_mut().write_message(SetTarget { fires_ahead: Some(false), ..SetTarget::of(e) });
    sim.seconds(0.5);
    sim.app.world_mut().resource_mut::<TriggerAtSelect>().0.clear();
    sim.seconds(2.0);
    assert!(!sim.app.world().resource::<TriggerAtSelect>().0.contains(&e), "released again after noahead");
}

/// `target set <n> alt` presses attack2 once (a scope raised for pictures
/// of TF2's laser dot) and `pitch` tilts its look: a Sniper target scopes
/// in, stays scoped, faces 25 degrees down, and its rifle's charge others
/// see (`HeldGauge`) rises as TF2's charge does (50/s after 0.3 s, of 150).
#[test]
fn a_sniper_target_scopes_in_on_alt_and_shows_its_charge() {
    use chimera_sim::weapon::{AltModes, HeldGauge, Inventory};
    let (mut sim, _me) = play();
    let e = add(&mut sim, SpawnTarget { character: Some("tf2:sniper".into()), gun: Some("tf2:tf_weapon_sniperrifle".into()), ..at(8.0) });
    let gun = |sim: &Sim| sim.app.world().get::<Inventory>(e).and_then(|i| i.active).expect("its rifle");
    let mode = |sim: &Sim| sim.app.world().get::<AltModes>(gun(sim)).map_or(0, |m| m.current);
    let gauge = |sim: &Sim| sim.app.world().get::<HeldGauge>(gun(sim)).map_or(-1.0, |g| g.0);
    assert_eq!((mode(&sim), gauge(&sim)), (0, 0.0));
    for line in ["target set all pitch 25", "target set all alt"] {
        sim.app.world_mut().resource_mut::<Console>().submit(line);
    }
    sim.seconds(1.3);
    assert_eq!(mode(&sim), 1, "scoped");
    let g = gauge(&sim);
    assert!((g - 1.0 / 3.0).abs() < 0.05, "a third charged 1.3 s in: {g}");
    let pitch = sim.app.world().get::<chimera_core::core::Intent>(e).unwrap().pitch;
    assert!((pitch + 25f32.to_radians()).abs() < 1e-4, "25 degrees down: {pitch}");
    sim.seconds(2.5);
    assert_eq!(mode(&sim), 1, "still scoped: one press");
    assert_eq!(gauge(&sim), 1.0, "full");
}
