//! Contract tests of the weapon frame (`chimera_sim::weapon`) on the test
//! kit's weapons (`chimera_sim::testkit`): trigger timing, magazines and
//! reloads, pellets and damage, with no game's data. A lift of weapon code
//! into Chimera is checked here (`cargo test -p chimera_sim --test it
//! weapon_frame::`) before the games' suites.

use bevy::prelude::*;
use chimera_core::core::Health;
use chimera_sim::{
    movement::placeholder,
    testkit::{self, PISTOL, RIFLE, SHOTGUN},
    weapon::{Magazine, WeaponEvent, WeaponEventKind},
};
use chimera_world::{greybox, harness::Sim};

/// A player holding `weapon`, aimed at a target 6 m ahead (health 10, so
/// a hit's cost reads off it directly).
fn armed(weapon: &str) -> (Sim, Entity, Entity) {
    let mut sim = testkit::kit_sim();
    let p = testkit::with_player(&mut sim, None, weapon);
    let target = sim.spawn_character(greybox::SPAWNS[0] - Vec3::Z * 6.0, placeholder::ID);
    sim.app.world_mut().get_mut::<Health>(target).unwrap().current = 10.0;
    sim.seconds(0.5);
    let chest = sim.position(target) + Vec3::Y * 0.2;
    testkit::aim_at(&mut sim, p, chest);
    (sim, p, target)
}

fn gun(sim: &Sim, p: Entity) -> Entity {
    testkit::active(sim.app.world(), p).unwrap()
}

fn clip(sim: &Sim, p: Entity) -> u32 {
    sim.app.world().get::<Magazine>(gun(sim, p)).unwrap().clip
}

/// Run `ticks`, counting the traces (`Shot`) the weapon in hand left.
fn shots_in(sim: &mut Sim, p: Entity, ticks: u64) -> usize {
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut shots = 0;
    // A tick at a time: messages live two updates.
    for _ in 0..ticks {
        sim.ticks(1);
        shots += cursor
            .read(sim.app.world().resource::<Messages<WeaponEvent>>())
            .filter(|e| e.owner == p && matches!(e.kind, WeaponEventKind::Shot { .. }))
            .count();
    }
    shots
}

/// `shots_in` for `secs` seconds.
fn shots_for(sim: &mut Sim, p: Entity, secs: f64) -> usize {
    let ticks = (secs * sim.tick_hz()).round() as u64;
    shots_in(sim, p, ticks)
}

#[test]
fn the_kit_weapons_are_the_ones_in_hand() {
    for id in [PISTOL, RIFLE, SHOTGUN] {
        let mut sim = testkit::kit_sim();
        let p = testkit::with_player(&mut sim, None, id);
        assert_eq!(testkit::active_id(sim.app.world(), p), Some(id));
    }
}

#[test]
fn a_semi_automatic_fires_once_a_pull() {
    let (mut sim, p, _) = armed(PISTOL);
    sim.intent(p).fire = true;
    // Held for two seconds: the one shot of the pull.
    let shots = shots_for(&mut sim, p, 2.0);
    assert_eq!(shots, 1);
    assert_eq!(clip(&sim, p), 7);
    // Released and pulled again after the cycle: another.
    sim.intent(p).fire = false;
    sim.ticks(2);
    sim.intent(p).fire = true;
    assert_eq!(shots_in(&mut sim, p, 2), 1);
    assert_eq!(clip(&sim, p), 6);
}

#[test]
fn the_cycle_time_spaces_shots() {
    let (mut sim, p, _) = armed(PISTOL);
    // Pulled every other tick for two seconds: one shot per 0.5 s cycle.
    let ticks = (2.0 * sim.tick_hz()) as u64;
    let mut shots = 0;
    for k in 0..ticks {
        sim.intent(p).fire = k % 2 == 0;
        shots += shots_in(&mut sim, p, 1);
    }
    // 4 in two seconds, 5 if one lands on the end.
    assert!((4..=5).contains(&shots), "{shots} shots in 2 s of a 0.5 s cycle");
}

#[test]
fn an_automatic_fires_while_held_at_its_cycle() {
    let (mut sim, p, _) = armed(RIFLE);
    sim.intent(p).fire = true;
    let shots = shots_for(&mut sim, p, 1.0);
    // A 0.1 s cycle: ten a second, give or take the first and the last.
    assert!((9..=11).contains(&shots), "{shots} shots in 1 s of a 0.1 s cycle");
    assert_eq!(clip(&sim, p) as usize, 30 - shots);
}

#[test]
fn a_hit_costs_its_damage() {
    let (mut sim, p, target) = armed(PISTOL);
    sim.intent(p).fire = true;
    sim.ticks(2);
    let lost = 10.0 - sim.app.world().get::<Health>(target).unwrap().current;
    // 20 hit points of 100, no falloff at 6 m.
    assert!((lost - 0.2).abs() < 1e-4, "lost {lost}");
}

#[test]
fn a_shot_leaves_as_many_traces_as_pellets() {
    let (mut sim, p, _) = armed(SHOTGUN);
    sim.intent(p).fire = true;
    assert_eq!(shots_in(&mut sim, p, 2), 4);
    assert_eq!(clip(&sim, p), 5);
}

#[test]
fn a_reload_swaps_the_clip_after_its_time() {
    let (mut sim, p, _) = armed(RIFLE);
    sim.intent(p).fire = true;
    let fired = shots_in(&mut sim, p, 6) as u32;
    sim.intent(p).fire = false;
    assert!(fired > 0);
    sim.intent(p).reload = true;
    sim.ticks(2);
    sim.intent(p).reload = false;
    // Still loading a second later (2 s reload)...
    sim.seconds(1.0);
    assert_eq!(clip(&sim, p), 30 - fired);
    // ...done after.
    sim.seconds(1.3);
    let m = sim.app.world().get::<Magazine>(gun(&sim, p)).unwrap().clone();
    assert_eq!(m.clip, 30);
    assert_eq!(m.reserve, 60 - fired);
}

#[test]
fn an_empty_clip_stops_the_shots() {
    let (mut sim, p, _) = armed(PISTOL);
    let mut total = 0;
    // Pull and release 9 times, a cycle apart (the ninth on the empty clip; its
    // release starts the reload, which takes longer than the test waits).
    for _ in 0..9 {
        sim.intent(p).fire = true;
        total += shots_in(&mut sim, p, 2);
        sim.intent(p).fire = false;
        sim.seconds(0.6);
    }
    assert_eq!(total, 8, "an 8-round clip");
    assert_eq!(clip(&sim, p), 0);
}

#[test]
fn nothing_fires_before_the_draw_ends() {
    let mut sim = testkit::kit_sim();
    let p = sim.spawn_character(greybox::SPAWNS[0], placeholder::ID);
    sim.seconds(0.5);
    chimera_sim::weapon::give(sim.app.world_mut(), p, RIFLE).unwrap();
    sim.intent(p).fire = true;
    // The rifle draws in 1 s: half of it later, nothing yet.
    assert_eq!(shots_for(&mut sim, p, 0.5), 0);
}

/// A round someone else fired, seen on a client after its shooter's gun
/// changed (the gun game swaps it on the kill): the shot still sounds.
#[test]
fn a_remote_shot_sounds_after_its_shooter_swapped_guns() {
    use chimera_sim::weapon::remote::{RemoteShot, show_held};
    use chimera_world::map::PlaySound;
    let mut sim = testkit::kit_sim();
    sim.app.insert_resource(chimera_core::core::Playback);
    let p = sim.spawn_character(greybox::SPAWNS[0], placeholder::ID);
    show_held(sim.app.world_mut(), p, Some(RIFLE), 0, false);
    sim.ticks(2);
    // The kill's swap reaches the client before the shot's message does.
    show_held(sim.app.world_mut(), p, Some(PISTOL), 0, false);
    sim.app.world_mut().write_message(RemoteShot {
        shooter: p,
        weapon: RIFLE,
        origin: greybox::SPAWNS[0] + Vec3::Y,
        yaw: 0.0,
        pitch: 0.0,
        seed: 1,
        spread: chimera_sim::weapon::SpreadShape::Template(0.0),
        mode: 0,
        flags: 0,
    });
    let mut cursor = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
    sim.ticks(2);
    let heard: Vec<String> = cursor
        .read(sim.app.world().resource::<Messages<PlaySound>>())
        .map(|s| s.entry.clone())
        .collect();
    assert_eq!(heard, ["kit.rifle_fire"], "the shot's sound plays though the gun is gone");
}

/// The weapon events of `ticks` ticks.
fn events_in(sim: &mut Sim, ticks: u64) -> Vec<WeaponEvent> {
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut out = Vec::new();
    for _ in 0..ticks {
        sim.ticks(1);
        out.extend(cursor.read(sim.app.world().resource::<Messages<WeaponEvent>>()).cloned());
    }
    out
}

/// Spawn protection (`core::SpawnProtected`, Combat Arms' style): a hit
/// on a protected player shows nothing (its trace has no hit: no blood,
/// impact or hit indicator) and deals nothing, in the bullets window and
/// in the damage window after it; once both are over, hits are normal.
#[test]
fn a_spawn_protected_target_shows_and_takes_no_hit() {
    use chimera_core::core::{SimClock, SpawnProtected};
    let (mut sim, p, target) = armed(PISTOL);
    let now = sim.app.world().resource::<SimClock>().now;
    sim.app.world_mut().entity_mut(target).insert(SpawnProtected {
        damage_until: now + 2.0,
        bullets_until: now + 1.0,
    });
    let hits = |events: &[WeaponEvent]| {
        let traced = events
            .iter()
            .filter(|e| matches!(e.kind, WeaponEventKind::Shot { hit: Some(h), .. } | WeaponEventKind::ShotContinued { hit: Some(h), .. } if h == target))
            .count();
        let dealt = events.iter().filter(|e| matches!(e.kind, WeaponEventKind::Hit { target: t, .. } if t == target)).count();
        let shots = events.iter().filter(|e| matches!(e.kind, WeaponEventKind::Shot { .. })).count();
        (shots, traced, dealt)
    };
    // The bullets window, then the damage window after it.
    for _ in 0..2 {
        sim.intent(p).fire = true;
        let (shots, traced, dealt) = hits(&events_in(&mut sim, 2));
        sim.intent(p).fire = false;
        assert!(shots > 0, "it fired");
        assert_eq!((traced, dealt), (0, 0), "no hit shown, none dealt");
        assert_eq!(sim.app.world().get::<Health>(target).unwrap().current, 10.0, "no damage");
        sim.seconds(1.0);
    }
    // Both over: a normal hit.
    sim.app.world_mut().entity_mut(target).remove::<SpawnProtected>();
    sim.intent(p).fire = true;
    let (_, traced, dealt) = hits(&events_in(&mut sim, 2));
    assert!(traced > 0 && dealt > 0, "shown and dealt after it");
    assert!(sim.app.world().get::<Health>(target).unwrap().current < 10.0);
}

/// Spawn protection's bullets window (`core::SpawnProtected::
/// passes_attacks`, user 2026-10-29): a round passes through the
/// protected player as if it weren't there, into the character behind
/// (which takes the damage) or, with no one there, to the wall (its trace
/// ends on the wall, with the normal an impact and a decal are drawn
/// from). Nothing traced or dealt names the protected player.
#[test]
fn a_round_passes_through_a_spawn_protected_target_into_what_is_behind() {
    use chimera_core::core::{SimClock, SpawnProtected};
    let (mut sim, p, target) = armed(PISTOL);
    let behind = sim.spawn_character(sim.position(target) - Vec3::Z * 2.0, placeholder::ID);
    sim.app.world_mut().get_mut::<Health>(behind).unwrap().current = 10.0;
    let now = sim.app.world().resource::<SimClock>().now;
    sim.app.world_mut().entity_mut(target).insert(SpawnProtected {
        damage_until: now + 10.0,
        bullets_until: now + 10.0,
    });
    sim.ticks(2);
    let names = |events: &[WeaponEvent], who: Entity| {
        events
            .iter()
            .filter(|e| match e.kind {
                WeaponEventKind::Shot { hit, .. } | WeaponEventKind::ShotContinued { hit, .. } => hit == Some(who),
                WeaponEventKind::Hit { target: t, .. } => t == who,
                _ => false,
            })
            .count()
    };
    sim.intent(p).fire = true;
    let events = events_in(&mut sim, 2);
    sim.intent(p).fire = false;
    assert_eq!(names(&events, target), 0, "nothing names the protected player");
    assert!(names(&events, behind) >= 2, "the one behind is traced and hit: {events:?}");
    assert_eq!(sim.app.world().get::<Health>(target).unwrap().current, 10.0);
    assert!(sim.app.world().get::<Health>(behind).unwrap().current < 10.0, "the one behind takes it");
    // No one behind: the round ends on the wall past the protected player.
    sim.app.world_mut().despawn(behind);
    sim.seconds(0.5);
    sim.intent(p).fire = true;
    let events = events_in(&mut sim, 2);
    sim.intent(p).fire = false;
    let (to, hit, normal) = events
        .iter()
        .find_map(|e| match e.kind {
            WeaponEventKind::Shot { to, hit, normal, .. } if e.owner == p => Some((to, hit, normal)),
            _ => None,
        })
        .expect("a shot");
    assert!(hit.is_some_and(|h| h != target) && normal.is_some(), "a wall hit, drawn: {hit:?} {normal:?}");
    assert!(to.z < sim.position(target).z - 1.0, "past the protected player: {to}");
    assert_eq!(names(&events, target), 0);
}

const KNIFE: &str = r#"
id = "kit:knife"
base = "kit:base"
slot = 2
draw = 0.5

[melee.primary]
range = 2.5
hull = 0.3
facing_cos = 0.7
damage = 5.0
hit_refire = 0.5
miss_refire = 0.4
miss_refire_other = 0.5
force = 0.0
"#;

/// A swing passes through a spawn-protected player too (its bullets
/// window): the character behind it takes the hit.
#[test]
fn a_swing_passes_through_a_spawn_protected_target() {
    use chimera_core::core::{SimClock, SpawnProtected};
    use chimera_sim::weapon::data::WeaponDataAppExt;
    let mut sim = testkit::kit_sim();
    sim.app.add_weapon_data(&[("knife.toml", KNIFE)]).expect("the knife loads");
    let p = testkit::with_player(&mut sim, None, "kit:knife");
    let front = sim.position(p) - Vec3::Z * 0.9;
    let target = sim.spawn_character(front, placeholder::ID);
    let behind = sim.spawn_character(front - Vec3::Z * 0.9, placeholder::ID);
    for e in [target, behind] {
        sim.app.world_mut().get_mut::<Health>(e).unwrap().current = 10.0;
    }
    sim.seconds(1.0);
    let now = sim.app.world().resource::<SimClock>().now;
    sim.app.world_mut().entity_mut(target).insert(SpawnProtected {
        damage_until: now + 10.0,
        bullets_until: now + 10.0,
    });
    let chest = sim.position(behind) + Vec3::Y * 0.2;
    testkit::aim_at(&mut sim, p, chest);
    sim.intent(p).fire = true;
    let events = events_in(&mut sim, 2);
    sim.intent(p).fire = false;
    let at = events.iter().find_map(|e| match e.kind {
        WeaponEventKind::Swing { at, .. } => Some(at.map(|a| a.2)),
        _ => None,
    });
    assert_eq!(at, Some(Some(behind)), "the swing lands behind: {events:?}");
    assert_eq!(sim.app.world().get::<Health>(target).unwrap().current, 10.0);
    assert!(sim.app.world().get::<Health>(behind).unwrap().current < 10.0);
}

/// On a client, others' rounds traced again (`RemoteShot`) show no hit on
/// a player its server protects (the replicated `SpawnProtected`).
#[test]
fn a_remote_shot_shows_no_hit_on_a_protected_player() {
    use chimera_core::core::{SimClock, SpawnProtected};
    use chimera_sim::weapon::remote::{RemoteShot, show_held};
    let mut sim = testkit::kit_sim();
    sim.app.insert_resource(chimera_core::core::Playback);
    let shooter = sim.spawn_character(greybox::SPAWNS[0], placeholder::ID);
    let target = sim.spawn_character(greybox::SPAWNS[0] - Vec3::Z * 6.0, placeholder::ID);
    show_held(sim.app.world_mut(), shooter, Some(RIFLE), 0, false);
    sim.seconds(0.5);
    let origin = sim.position(shooter) + Vec3::Y * 0.6;
    let d = sim.position(target) + Vec3::Y * 0.2 - origin;
    let shot = RemoteShot {
        shooter,
        weapon: RIFLE,
        origin,
        yaw: (-d.x).atan2(-d.z),
        pitch: d.y.atan2(d.xz().length()),
        seed: 1,
        spread: chimera_sim::weapon::SpreadShape::Template(0.0),
        mode: 0,
        flags: 0,
    };
    let traced = |sim: &mut Sim| {
        sim.app.world_mut().write_message(shot.clone());
        events_in(sim, 2)
            .iter()
            .filter(|e| matches!(e.kind, WeaponEventKind::Shot { hit: Some(h), .. } if h == target))
            .count()
    };
    assert_eq!(traced(&mut sim), 1, "unprotected: the hit shows");
    let now = sim.app.world().resource::<SimClock>().now;
    sim.app.world_mut().entity_mut(target).insert(SpawnProtected {
        damage_until: now + 4.0,
        bullets_until: now + 3.0,
    });
    assert_eq!(traced(&mut sim), 0, "protected: nothing shows");
}
