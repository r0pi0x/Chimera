//! HL2:DM's guns on the headless harness (`harness::Hl2Sim`, the
//! greybox), specs/hl2dm/weapons.md's implementer cases (H3, the numbers
//! in the test names): the spawn loadout, switch and draw times, the
//! pistol's hold, click refire and accuracy ramp, the .357, the machine
//! guns' rates and reloads, the shotgun's pump, double shot and
//! shell-by-shell reload, damage to a player with head x3, the holstered
//! auto-reload, the next best weapon on an empty one, and the damage
//! shove (projectiles.md 3). Ticks count from the first shot's (or the
//! action's) tick, at 0.015 s.

use bevy::prelude::*;
use chimera_core::core::{Health, Velocity};
use chimera_sim::{
    testkit,
    weapon::{Inventory, Magazine, SpreadShape, Hitscan, ViewPunch, Weapon, WeaponEvent, WeaponEventKind, WeaponState, give},
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use hl2dm::{
    game::{
        movement,
        weapons::{self, inventory::{Hl2Ammo, Hl2Model}, pistol::Hl2Pistol},
    },
    harness::Hl2Sim,
};

/// The greybox on HL2:DM's harness, a fixed tick per update at
/// HL2:DM's 0.015 s.
fn sim() -> Sim {
    let mut sim = Sim::new(Hl2Sim, GreyboxMapPlugin);
    sim.set_tick_interval(hl2dm::game::TICK_INTERVAL);
    sim
}

/// A player at the greybox's first spawn with its loadout, settled.
fn player(sim: &mut Sim) -> Entity {
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    p
}

/// A player holding `id`, drawn and settled.
fn holding(sim: &mut Sim, id: &str) -> (Entity, Entity) {
    let p = player(sim);
    let w = match weapon_of(sim, p, id) {
        Some(w) => {
            sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
            w
        }
        None => give(sim.app.world_mut(), p, id).unwrap(),
    };
    sim.seconds(1.5);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(weapon_id(sim, w)));
    (p, w)
}

fn weapon_id(sim: &Sim, w: Entity) -> &'static str {
    sim.app.world().get::<Weapon>(w).unwrap().id
}

fn weapon_of(sim: &Sim, p: Entity, id: &str) -> Option<Entity> {
    let w = sim.app.world();
    w.get::<Inventory>(p)?.weapons.iter().copied().find(|e| w.get::<Weapon>(*e).is_some_and(|x| x.id == id))
}

fn mag(sim: &Sim, w: Entity) -> Magazine {
    sim.app.world().get::<Magazine>(w).unwrap().clone()
}

fn set_mag(sim: &mut Sim, w: Entity, clip: u32, reserve: u32) {
    let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
    m.clip = clip;
    m.reserve = reserve;
}

/// Run `n` ticks, `input` setting the player's buttons before each (its
/// tick index), and return what happened on which tick.
fn run(sim: &mut Sim, p: Entity, n: u64, mut input: impl FnMut(&mut Sim, u64)) -> Vec<(u64, WeaponEventKind)> {
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut out = Vec::new();
    for t in 0..n {
        input(sim, t);
        sim.ticks(1);
        let events = sim.app.world().resource::<Messages<WeaponEvent>>();
        for e in cursor.read(events) {
            if e.owner == p && e.shown() {
                out.push((t, e.kind.clone()));
            }
        }
    }
    out
}

fn fired(events: &[(u64, WeaponEventKind)]) -> Vec<u64> {
    events.iter().filter(|(_, k)| matches!(k, WeaponEventKind::Fired { .. })).map(|(t, _)| *t).collect()
}

#[test]
fn spawn_loadout_1_and_2() {
    let mut sim = sim();
    // A citizen: the crowbar.
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.app.world_mut().entity_mut(p).insert(Hl2Model("models/humans/group03/male_01.mdl".into()));
    sim.seconds(1.5);
    let w = sim.app.world();
    let inv = w.get::<Inventory>(p).unwrap();
    let ids: Vec<&str> = inv.weapons.iter().map(|e| w.get::<Weapon>(*e).unwrap().id).collect();
    assert_eq!(
        ids,
        [weapons::CROWBAR, weapons::PISTOL, weapons::SMG1, weapons::FRAG, weapons::PHYSCANNON],
        "order of acquisition"
    );
    assert_eq!(testkit::active_id(w, p), Some(weapons::PHYSCANNON));
    assert_eq!(inv.last.map(|l| w.get::<Weapon>(l).unwrap().id), Some(weapons::SMG1));
    let frag = weapon_of(&sim, p, weapons::FRAG).unwrap();
    assert_eq!(sim.app.world().get::<hl2dm::game::weapons::launch::Hl2Rounds>(frag).unwrap().count, 2, "2 grenades");
    let (pistol, smg) = (weapon_of(&sim, p, weapons::PISTOL).unwrap(), weapon_of(&sim, p, weapons::SMG1).unwrap());
    assert_eq!((mag(&sim, pistol).clip, mag(&sim, pistol).reserve), (18, 150));
    assert_eq!((mag(&sim, smg).clip, mag(&sim, smg).reserve), (45, 45));
    let pools = sim.app.world().get::<Hl2Ammo>(p).unwrap();
    assert!(pools.0.contains(&("hl2dm:buckshot".into(), 6)) && pools.0.contains(&("hl2dm:357".into(), 6)), "{pools:?}");
    // The Combine (and a player with no model: the DM fallback) the stunstick.
    let q = sim.spawn_character(greybox::SPAWNS[1], movement::ID);
    sim.seconds(1.5);
    assert!(weapon_of(&sim, q, weapons::STUNSTICK).is_some() && weapon_of(&sim, q, weapons::CROWBAR).is_none());
}

#[test]
fn a_switch_takes_the_new_weapons_draw_4() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, weapons::PISTOL);
    let smg = weapon_of(&sim, p, weapons::SMG1).unwrap();
    let ev = run(&mut sim, p, 70, |s, t| {
        if t == 0 {
            s.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(smg);
        }
        s.intent(p).fire = t >= 1;
    });
    assert_eq!(fired(&ev).first(), Some(&56), "first SMG1 shot 56 ticks after the switch");
}

#[test]
fn pistol_held_fires_every_half_second_5() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons::PISTOL);
    let ev = run(&mut sim, p, 600, |s, _| s.intent(p).fire = true);
    let shots = fired(&ev);
    assert_eq!(shots.len(), 18, "{shots:?}");
    assert!(shots[17].abs_diff(567) <= 1, "last at {}", shots[17]);
    assert_eq!(mag(&sim, w).clip, 0);
}

#[test]
fn pistol_clicks_every_8_ticks_6_and_its_penalty_7() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons::PISTOL);
    // Press on 0, 8, 16 ...; let go on the tick after.
    let mut penalties = Vec::new();
    let ev = run(&mut sim, p, 136, |s, t| {
        s.intent(p).fire = t % 8 == 0;
        penalties.push(s.app.world().get::<Hl2Pistol>(w).unwrap().penalty);
    });
    let shots = fired(&ev);
    assert_eq!(shots.len(), 17, "{shots:?}");
    assert!(shots.windows(2).all(|d| d[1] - d[0] == 8), "{shots:?}");
    // Penalties before ticks 8 and 16 (after shot 1's tick 7 decay, after
    // shot 2 and tick 15's): 0.185 and 0.37.
    assert!((penalties[8] - 0.185).abs() < 1e-4, "{}", penalties[8]);
    assert!((penalties[16] - 0.37).abs() < 1e-4, "{}", penalties[16]);
    // A press a tick early (tick 7 after a shot on 0) doesn't fire.
    let mut sim = self::sim();
    let (p, _) = holding(&mut sim, weapons::PISTOL);
    let ev = run(&mut sim, p, 10, |s, t| s.intent(p).fire = t == 0 || t == 7);
    assert_eq!(fired(&ev), [0]);
}

#[test]
fn pistol_penalty_held_and_its_cone_8_and_reload_9() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons::PISTOL);
    let ev = run(&mut sim, p, 250, |s, _| s.intent(p).fire = true);
    assert_eq!(fired(&ev).len(), 8);
    let pistol = *sim.app.world().get::<Hl2Pistol>(w).unwrap();
    assert!((pistol.penalty - 1.6).abs() < 1e-4, "{}", pistol.penalty);
    // Let go: the first decay clamps it to 1.5, still the widest cone.
    run(&mut sim, p, 1, |s, _| s.intent(p).fire = false);
    assert_eq!(sim.app.world().get::<Hl2Pistol>(w).unwrap().penalty, 1.5);
    let scan = sim.app.world().get::<Hitscan>(w).unwrap().spread;
    assert_eq!(scan, SpreadShape::Cone(0.05234));
    // The first decay clamps to 1.5; it is back to 0 about 100 ticks on.
    run(&mut sim, p, 110, |_, _| {});
    assert_eq!(sim.app.world().get::<Hl2Pistol>(w).unwrap().penalty, 0.0);
    // Test 9: a reload zeroes it; the clip fills at tick 96.
    sim.app.world_mut().get_mut::<Hl2Pistol>(w).unwrap().penalty = 1.2;
    set_mag(&mut sim, w, 5, 10);
    let ev = run(&mut sim, p, 100, |s, t| s.intent(p).reload = t == 0);
    assert_eq!(sim.app.world().get::<Hl2Pistol>(w).unwrap().penalty, 0.0);
    let done = ev.iter().find(|(_, k)| matches!(k, WeaponEventKind::Reloaded)).map(|(t, _)| *t);
    assert_eq!(done, Some(96));
    assert_eq!((mag(&sim, w).clip, mag(&sim, w).reserve), (15, 0));
}

#[test]
fn an_empty_pistol_dry_fires_then_switches_to_the_smg1_10() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons::PISTOL);
    set_mag(&mut sim, w, 0, 0);
    let ev = run(&mut sim, p, 40, |s, _| s.intent(p).fire = true);
    assert_eq!(ev.iter().find(|(_, k)| matches!(k, WeaponEventKind::DryFire)).map(|(t, _)| *t), Some(0));
    let deployed = ev.iter().find(|(_, k)| matches!(k, WeaponEventKind::Deployed)).map(|(t, _)| *t);
    assert!(deployed.is_some_and(|t| t.abs_diff(25) <= 1), "switched on {deployed:?}");
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(weapons::SMG1));
}

#[test]
fn the_357_fires_every_50_ticks_exactly_11_and_kicks_up_12_and_reloads_13() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons::MAGNUM);
    set_mag(&mut sim, w, 6, 0);
    let mut punch = Vec::new();
    let ev = run(&mut sim, p, 260, |s, _| {
        s.intent(p).fire = true;
        punch.push(s.app.world().get::<ViewPunch>(p).map_or(0.0, |v| v.0.x));
    });
    assert_eq!(fired(&ev), [0, 50, 100, 150, 200, 250]);
    // The punch after the first shot's tick and the next: 2.4 and 4.44
    // degrees up (pitch up is positive in `ViewPunch`).
    assert!((punch[2].to_degrees() - 2.4).abs() < 0.01, "{}", punch[2].to_degrees());
    assert!((punch[3].to_degrees() - 4.4409).abs() < 0.01, "{}", punch[3].to_degrees());
    // Test 13: from 2 in the clip and 6 carried, 245 ticks.
    set_mag(&mut sim, w, 2, 6);
    run(&mut sim, p, 60, |s, _| s.intent(p).fire = false);
    let ev = run(&mut sim, p, 250, |s, t| s.intent(p).reload = t == 0);
    assert_eq!(ev.iter().find(|(_, k)| matches!(k, WeaponEventKind::Reloaded)).map(|(t, _)| *t), Some(245));
    assert_eq!((mag(&sim, w).clip, mag(&sim, w).reserve), (6, 2));
}

#[test]
fn smg1_empties_in_220_ticks_14() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons::SMG1);
    set_mag(&mut sim, w, 45, 0);
    let ev = run(&mut sim, p, 240, |s, _| s.intent(p).fire = true);
    let shots = fired(&ev);
    assert_eq!(shots.len(), 45);
    assert!(shots[44].abs_diff(220) <= 1, "last at {}", shots[44]);
    assert!(shots.windows(2).all(|d| (4..=6).contains(&(d[1] - d[0]))), "{shots:?}");
}

#[test]
fn ar2_fires_7_7_6_18_and_reloads_in_105_19() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons::AR2);
    set_mag(&mut sim, w, 30, 0);
    let ev = run(&mut sim, p, 196, |s, _| s.intent(p).fire = true);
    let shots = fired(&ev);
    assert_eq!(shots.len(), 30);
    assert!(shots[29] == 193 || shots[29] == 194, "last at {}", shots[29]);
    let gaps: Vec<u64> = shots.windows(2).map(|d| d[1] - d[0]).collect();
    assert!(gaps.iter().all(|g| *g == 6 || *g == 7) && gaps.iter().sum::<u64>() == shots[29], "{gaps:?}");
    // A round left (no automatic reload, no switch) until the timers pass.
    set_mag(&mut sim, w, 1, 60);
    run(&mut sim, p, 30, |s, _| s.intent(p).fire = false);
    set_mag(&mut sim, w, 0, 60);
    let ev = run(&mut sim, p, 110, |s, t| s.intent(p).reload = t == 0);
    assert_eq!(ev.iter().find(|(_, k)| matches!(k, WeaponEventKind::Reloaded)).map(|(t, _)| *t), Some(105));
    assert_eq!((mag(&sim, w).clip, mag(&sim, w).reserve), (30, 30));
}

#[test]
fn shotgun_pumps_between_shots_20_and_double_shots_21() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons::SHOTGUN);
    set_mag(&mut sim, w, 6, 0);
    let ev = run(&mut sim, p, 300, |s, _| s.intent(p).fire = true);
    assert_eq!(fired(&ev), [0, 59, 118, 177, 236, 295]);
    let pumps: Vec<u64> = ev.iter().filter(|(_, k)| matches!(k, WeaponEventKind::Action("pump"))).map(|(t, _)| *t).collect();
    assert_eq!(&pumps[..3], [23, 82, 141]);
    // The double shot: 12 pellets for two shells, 70 ticks apart.
    let mut sim = self::sim();
    let (p, w) = holding(&mut sim, weapons::SHOTGUN);
    set_mag(&mut sim, w, 6, 0);
    let ev = run(&mut sim, p, 150, |s, _| s.intent(p).secondary = true);
    assert_eq!(fired(&ev), [0, 70, 140]);
    let first_traces = ev.iter().filter(|(t, k)| *t == 0 && matches!(k, WeaponEventKind::Shot { .. })).count();
    assert_eq!(first_traces, 12);
    assert_eq!(mag(&sim, w).clip, 0);
}

#[test]
fn shotgun_reloads_shell_by_shell_23_and_a_shot_stops_it_24() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons::SHOTGUN);
    set_mag(&mut sim, w, 0, 10);
    // Test 23: no buttons, the empty tube reloads by itself from tick 0.
    let mut clips = Vec::new();
    let ev = run(&mut sim, p, 230, |s, _| clips.push(s.app.world().get::<Magazine>(w).unwrap().clip));
    let start = ev.iter().find(|(_, k)| matches!(k, WeaponEventKind::ReloadStarted)).map(|(t, _)| *t).unwrap();
    let shells: Vec<u64> = ev
        .iter()
        .filter(|(_, k)| matches!(k, WeaponEventKind::ShellInserting))
        .map(|(t, _)| *t - start)
        .collect();
    assert_eq!(shells, [34, 61, 88, 115, 142, 169]);
    let done = ev.iter().find(|(_, k)| matches!(k, WeaponEventKind::Reloaded)).map(|(t, _)| *t - start);
    assert_eq!(done, Some(225), "the finish ends (fire possible) at 225");
    assert_eq!((mag(&sim, w).clip, mag(&sim, w).reserve), (6, 4));
    // Test 24: attack pressed on tick 70 stops it after shell 2, the shot
    // on tick 88, the pump 23 ticks on.
    let mut sim = self::sim();
    let (p, w) = holding(&mut sim, weapons::SHOTGUN);
    set_mag(&mut sim, w, 0, 10);
    let mut start = None;
    let ev = run(&mut sim, p, 130, |s, t| {
        let st = s.app.world().get::<WeaponState>(w).unwrap();
        if start.is_none() && st.reloading() {
            start = Some(t - 1);
        }
        s.intent(p).fire = start.is_some_and(|s0| t >= s0 + 70);
    });
    let s0 = start.unwrap();
    let shot = fired(&ev).first().map(|t| t - s0);
    assert_eq!(shot, Some(88));
    assert_eq!(mag(&sim, w).clip, 1);
    let pump = ev.iter().find(|(_, k)| matches!(k, WeaponEventKind::Action("pump"))).map(|(t, _)| t - s0);
    assert_eq!(pump, Some(111));
}

/// A target 1.6 m in front of `p` (no armour), and where to aim at its body
/// and head.
fn target(sim: &mut Sim) -> (Entity, Vec3, Vec3) {
    let t = sim.spawn_character(greybox::SPAWNS[0] - Vec3::Z * 1.6, movement::ID);
    sim.seconds(1.0);
    let at = sim.position(t);
    (t, at + Vec3::Y * 0.2, at + Vec3::Y * 0.62)
}

/// Shots of `id` (each its own press, the weapon refilled) to kill a
/// 100-health target at `aim` (`body` or head).
fn shots_to_kill(id: &str, head: bool) -> usize {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, id);
    let (t, body, head_at) = target(&mut sim);
    let aim = if head { head_at } else { body };
    for n in 1..40 {
        testkit::aim_at(&mut sim, p, aim);
        set_mag(&mut sim, w, 6, 0);
        run(&mut sim, p, 2, |s, k| s.intent(p).fire = k == 0);
        run(&mut sim, p, 120, |s, _| s.intent(p).fire = false);
        if sim.app.world().get::<Health>(t).unwrap().current <= 0.0 {
            return n;
        }
        // The target back where it was (shoves move it).
        let at = greybox::SPAWNS[0] - Vec3::Z * 1.6;
        sim.app.world_mut().get_mut::<Transform>(t).unwrap().translation.x = at.x;
        sim.app.world_mut().get_mut::<Transform>(t).unwrap().translation.z = at.z;
        sim.app.world_mut().get_mut::<Velocity>(t).unwrap().0 = Vec3::ZERO;
    }
    panic!("{id} never killed");
}

#[test]
fn shots_to_kill_26() {
    for (id, body, head) in [
        (weapons::PISTOL, 13, 5),
        (weapons::MAGNUM, 2, 1),
        (weapons::SMG1, 20, 7),
        (weapons::AR2, 10, 4),
    ] {
        assert_eq!(shots_to_kill(id, false), body, "{id} body");
        assert_eq!(shots_to_kill(id, true), head, "{id} head");
    }
    // The shotgun's seven pellets at 1.6 m: two shots on the body (63 each
    // when all land; the cone may lose some at this range).
    let n = shots_to_kill(weapons::SHOTGUN, false);
    assert!((2..=3).contains(&n), "shotgun {n}");
}

#[test]
fn a_holstered_clip_tops_up_after_3_seconds_49() {
    let mut sim = sim();
    let (p, smg) = holding(&mut sim, weapons::SMG1);
    set_mag(&mut sim, smg, 10, 100);
    let pistol = weapon_of(&sim, p, weapons::PISTOL).unwrap();
    let mut clips = Vec::new();
    run(&mut sim, p, 210, |s, t| {
        if t == 0 {
            s.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(pistol);
        }
        clips.push(s.app.world().get::<Magazine>(smg).unwrap().clip);
    });
    // Filled at the first tick more than 3 s after the switch (201); the
    // clips read before each tick.
    let filled = clips.iter().position(|c| *c == 45).unwrap() as u64 - 1;
    assert!(filled.abs_diff(201) <= 1, "filled on tick {filled}");
    assert_eq!(mag(&sim, smg).reserve, 65);
}

#[test]
fn a_never_drawn_weapon_tops_up_at_once_50() {
    let mut sim = sim();
    let p = player(&mut sim);
    // Past the game's first 3 s (a never-drawn weapon was put away at 0).
    sim.seconds(2.0);
    let gun = give(sim.app.world_mut(), p, weapons::SHOTGUN).unwrap();
    // Not drawn: back to what was in hand.
    let smg = weapon_of(&sim, p, weapons::SMG1).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(smg);
    set_mag(&mut sim, gun, 2, 6);
    sim.ticks(2);
    assert_eq!((mag(&sim, gun).clip, mag(&sim, gun).reserve), (6, 2));
}

#[test]
fn a_pistol_hit_shoves_the_victim_40_units_a_second() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons::PISTOL);
    let (t, body, _) = target(&mut sim);
    testkit::aim_at(&mut sim, p, body);
    set_mag(&mut sim, w, 1, 0);
    let mut speeds = Vec::new();
    run(&mut sim, p, 3, |s, k| {
        s.intent(p).fire = k == 0;
        speeds.push(s.velocity(t).xz().length() / 0.0254);
    });
    speeds.push(sim.velocity(t).xz().length() / 0.0254);
    let hit = sim.app.world().get::<Health>(t).unwrap().current;
    assert!((hit - 0.92).abs() < 1e-4, "health {hit}");
    // After the shot's tick its velocity has the shove (the placeholder
    // movement takes it away again the next tick).
    assert!((speeds[1] - 40.0).abs() < 2.0, "{speeds:?}");
}

#[test]
fn the_pistol_has_spread_and_the_357_none() {
    let mut sim = sim();
    let (_, w) = holding(&mut sim, weapons::MAGNUM);
    assert_eq!(sim.app.world().get::<Hitscan>(w).unwrap().spread, SpreadShape::Cone(0.0));
    let (_, w) = holding(&mut sim, weapons::PISTOL);
    assert_eq!(sim.app.world().get::<Hitscan>(w).unwrap().spread, SpreadShape::Cone(0.00873));
}

/// Put `p` in the greybox's tank, standing on its floor with its eyes
/// under its 2.8 m of water, and let the movement see it.
fn dunk(sim: &mut Sim, p: Entity) {
    let c = (greybox::TANK_MIN + greybox::TANK_MAX) / 2.0;
    // The hull's centre, 36 units over the feet.
    let at = Vec3::new(c.x, 0.95, c.y);
    let w = sim.app.world_mut();
    w.get_mut::<Transform>(p).unwrap().translation = at;
    if let Some(mut pos) = w.get_mut::<avian3d::prelude::Position>(p) {
        pos.0 = at;
    }
    sim.app.world_mut().get_mut::<Velocity>(p).unwrap().0 = Vec3::ZERO;
    sim.ticks(4);
    let level = sim.app.world().get::<source_engine::source_movement::SourceMovement>(p).unwrap().water_level;
    assert_eq!(level, 3, "eyes under water");
}

/// Test 52: with the eyes under water the .357, the SMG1, the AR2 and the
/// shotgun (both shots) refuse attack: their empty sound, no shot, no
/// round used, the next try 14 ticks on (0.2 s); the pistol fires. The
/// SMG1's grenade refuses too (projectiles.md 4: a dry fire, its empty
/// sound, none used).
#[test]
fn guns_refuse_under_water_but_the_pistol_52() {
    use chimera_world::map::PlaySound;
    for (id, secondary, fires) in [
        (weapons::MAGNUM, false, false),
        (weapons::SMG1, false, false),
        (weapons::AR2, false, false),
        (weapons::SHOTGUN, false, false),
        (weapons::SHOTGUN, true, false),
        (weapons::PISTOL, false, true),
    ] {
        let mut sim = sim();
        let (p, w) = holding(&mut sim, id);
        dunk(&mut sim, p);
        let clip = mag(&sim, w).clip;
        let empty = sim.app.world().get::<chimera_sim::weapon::WeaponSounds>(w).and_then(|s| s.empty.clone()).unwrap();
        let mut cursor = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
        let mut clicks = Vec::new();
        let ran = run(&mut sim, p, 30, |s, t| {
            if t > 0 {
                let sounds = s.app.world().resource::<Messages<PlaySound>>();
                clicks.extend(cursor.read(sounds).filter(|m| m.entry == empty && m.source == Some(p)).map(|_| t - 1));
            }
            let mut i = s.intent(p);
            if secondary {
                i.secondary = true;
            } else {
                i.fire = true;
            }
        });
        if fires {
            assert!(!fired(&ran).is_empty(), "{id} fires under water");
            continue;
        }
        assert_eq!(fired(&ran), Vec::<u64>::new(), "{id} (attack2 {secondary})");
        assert_eq!(mag(&sim, w).clip, clip, "{id}: no round used");
        assert_eq!(clicks, [0, 14, 28], "{id} (attack2 {secondary}): its empty sound every 0.2 s");
    }

    // The SMG1's grenade: a dry fire.
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons::SMG1);
    sim.app.world_mut().get_mut::<hl2dm::game::weapons::launch::Hl2Rounds>(w).unwrap().count = 1;
    dunk(&mut sim, p);
    let ran = run(&mut sim, p, 3, |s, t| s.intent(p).secondary = t == 0);
    assert!(ran.iter().any(|(_, k)| matches!(k, WeaponEventKind::DryFire)), "{ran:?}");
    assert_eq!(sim.app.world().get::<hl2dm::game::weapons::launch::Hl2Rounds>(w).unwrap().count, 1);
}
