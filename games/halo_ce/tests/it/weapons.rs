//! Halo's four weapons in the game (`game::weapons`) on `HaloSim` at 30 Hz,
//! held by a Spartan on the greybox: the timelines of
//! specs/halo_ce/weapons.md "Test cases" (shots after 1, 2 and 5 s of
//! holding, the first six shots' ticks, reloads from empty and while held,
//! the shotgun's chamber and shell-by-shell reload, the draw, the melee),
//! the pistol's zoom as the camera's field of view, shots to kill a full
//! Spartan at point-blank with real fire (projectiles.md "Shots to kill"),
//! and the timing under a party app's 64 Hz tick. Tick 0 is the first
//! tick the input in question is down.

use bevy::prelude::*;
use chimera_core::core::{ActionRegistry, Died, Health};
use chimera_sim::weapon::{Inventory, Magazine, WeaponEvent, WeaponEventKind, Zoomed, give};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use halo_ce::{
    game::{
        movement,
        vitality::Vitality,
        weapons::{ASSAULT_RIFLE, HaloFired, HaloGun, PISTOL, PLASMA_RIFLE, SHOTGUN, systems::MELEE_ACTION},
    },
    harness::HaloSim,
};

/// A Spartan on the greybox holding `id` long enough (its draw over, the
/// trigger's idle counter at its cap), looking along -Z.
pub fn armed(id: &str) -> (Sim, Entity, Entity) {
    armed_in(Sim::new(HaloSim, GreyboxMapPlugin), id)
}

fn armed_in(mut sim: Sim, id: &str) -> (Sim, Entity, Entity) {
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.ticks(2);
    let g = give(sim.app.world_mut(), p, id).unwrap();
    sim.seconds(5.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(g));
    (sim, p, g)
}

/// What the shooter's weapon did on each tick, by name.
fn label(k: &WeaponEventKind) -> Option<&'static str> {
    Some(match k {
        WeaponEventKind::Action(a) => a,
        WeaponEventKind::ReloadStarted => "reload_started",
        WeaponEventKind::ShellInserting => "shell",
        WeaponEventKind::Reloaded => "reloaded",
        WeaponEventKind::DryFire => "dry",
        WeaponEventKind::Swing { hit: true, .. } => "swing_hit",
        WeaponEventKind::Swing { hit: false, .. } => "swing_miss",
        _ => return None,
    })
}

/// Run `ticks` ticks, `input(sim, t)` before each; every (tick, event) of
/// `p`'s weapons.
fn record(sim: &mut Sim, p: Entity, ticks: u32, mut input: impl FnMut(&mut Sim, u32)) -> Vec<(u32, &'static str)> {
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut out = Vec::new();
    for t in 0..ticks {
        input(sim, t);
        sim.ticks(1);
        for e in cursor.read(sim.app.world().resource::<Messages<WeaponEvent>>()) {
            if e.owner == p
                && let Some(l) = label(&e.kind)
            {
                out.push((t, l));
            }
        }
    }
    out
}

fn shots(log: &[(u32, &str)]) -> Vec<u32> {
    log.iter().filter(|(_, l)| l.starts_with("fire") || *l == "misfire").map(|(t, _)| *t).collect()
}

fn ticks_of(log: &[(u32, &str)], name: &str) -> Vec<u32> {
    log.iter().filter(|(_, l)| *l == name).map(|(t, _)| *t).collect()
}

fn hold(sim: &mut Sim, p: Entity, ticks: u32) -> Vec<(u32, &'static str)> {
    record(sim, p, ticks, |s, _| s.intent(p).fire = true)
}

fn set_rounds(sim: &mut Sim, g: Entity, clip: u32, reserve: u32) {
    let mut m = sim.app.world_mut().get_mut::<Magazine>(g).unwrap();
    m.clip = clip;
    m.reserve = reserve;
}

#[test]
fn held_triggers_fire_at_the_specs_ticks() {
    // (weapon, shots in 1 / 2 / 5 s, the first six shots).
    for (id, counts, first) in [
        (ASSAULT_RIFLE, [15, 30, 60], [0, 2, 4, 6, 8, 10]),
        (PISTOL, [4, 7, 12], [0, 9, 18, 27, 36, 45]),
        (PLASMA_RIFLE, [7, 15, 25], [0, 5, 10, 14, 18, 22]),
        (SHOTGUN, [1, 2, 5], [0, 30, 60, 90, 120, 150]),
    ] {
        let (mut sim, p, _) = armed(id);
        let log = hold(&mut sim, p, 180);
        let s = shots(&log);
        let within = |n: u32| s.iter().filter(|t| **t < n).count();
        assert_eq!([within(30), within(60), within(150)], counts, "{id}: {s:?}");
        assert_eq!(&s[..6], &first, "{id}");
    }
}

#[test]
fn the_assault_rifle_reloads_while_held() {
    let (mut sim, p, g) = armed(ASSAULT_RIFLE);
    let log = hold(&mut sim, p, 220);
    let s = shots(&log);
    assert_eq!(s[59], 118, "the magazine's last round");
    assert_eq!(ticks_of(&log, "fire_last"), [118]);
    assert_eq!(ticks_of(&log, "reload_started"), [119]);
    assert_eq!(ticks_of(&log, "reloaded"), [205]);
    assert_eq!(s[60], 207);
    let m = sim.app.world().get::<Magazine>(g).unwrap();
    assert_eq!(m.reserve, 120);
}

#[test]
fn the_pistol_reloads_after_its_recoil_animation() {
    let (mut sim, p, _) = armed(PISTOL);
    let log = hold(&mut sim, p, 110);
    assert_eq!(shots(&log)[11], 99);
    assert_eq!(ticks_of(&log, "reload_started"), [103]);
}

#[test]
fn the_pistol_tapped_every_other_tick_fires_every_ten() {
    let (mut sim, p, _) = armed(PISTOL);
    let log = record(&mut sim, p, 41, |s, t| s.intent(p).fire = t % 2 == 0);
    assert_eq!(shots(&log), [0, 10, 20, 30, 40]);
}

#[test]
fn reloads_from_empty_start_and_end_on_their_ticks() {
    // (weapon, start, rounds in, first shot).
    for (id, start, rounds_in, next) in [(ASSAULT_RIFLE, 1, 87, 89), (PISTOL, 4, 70, 72)] {
        let (mut sim, p, g) = armed(id);
        set_rounds(&mut sim, g, 1, 100);
        // The last round on tick 0, released, held again from tick 80.
        let log = record(&mut sim, p, next + 3, |s, t| s.intent(p).fire = t == 0 || t >= 60);
        assert_eq!(ticks_of(&log, "reload_started"), [start], "{id}");
        assert_eq!(ticks_of(&log, "reloaded"), [rounds_in], "{id}");
        assert_eq!(shots(&log)[..2.min(shots(&log).len())], [0, next], "{id}: {log:?}");
    }
}

#[test]
fn the_shotgun_chambers_after_a_shot_and_reloads_shell_by_shell() {
    let (mut sim, p, g) = armed(SHOTGUN);
    // A shot with shells left: chambering on tick 1; the weapon's own
    // chamber animation holds a reload back until tick 22.
    set_rounds(&mut sim, g, 5, 12);
    let log = record(&mut sim, p, 30, |s, t| {
        let mut i = s.intent(p);
        i.fire = t == 0;
        i.reload = t >= 2;
    });
    assert_eq!(ticks_of(&log, "chamber"), [1]);
    assert_eq!(ticks_of(&log, "reload_started"), [22]);
    // The last shell on tick 0, released: the first shell's 15-frame
    // enter, then one every 11 ticks, the 12th on tick 136.
    let (mut sim, p, g) = armed(SHOTGUN);
    set_rounds(&mut sim, g, 1, 12);
    let log = record(&mut sim, p, 150, |s, t| s.intent(p).fire = t == 0);
    assert_eq!(ticks_of(&log, "reload_started"), [1]);
    assert_eq!(ticks_of(&log, "shell_in"), [15, 26, 37, 48, 59, 70, 81, 92, 103, 114, 125, 136]);
    assert_eq!(sim.app.world().get::<Magazine>(g).unwrap().clip, 12);
}

#[test]
fn a_drawn_weapon_first_fires_after_its_ready_animation() {
    for (id, first) in [(ASSAULT_RIFLE, 28), (PISTOL, 34), (SHOTGUN, 22), (PLASMA_RIFLE, 28)] {
        let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
        let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
        sim.ticks(2);
        // Given and drawn on tick 0, the trigger held from then.
        give(sim.app.world_mut(), p, id).unwrap();
        let log = hold(&mut sim, p, 40);
        assert_eq!(shots(&log).first(), Some(&first), "{id}");
    }
}

/// A second Spartan `metres` ahead of `p` (along -Z).
fn target(sim: &mut Sim, p: Entity, metres: f32) -> Entity {
    let at = sim.position(p) - Vec3::Z * metres;
    let t = sim.spawn_character(at, movement::ID);
    sim.ticks(2);
    t
}

fn melee_bit(sim: &Sim) -> u32 {
    sim.app.world().resource::<ActionRegistry>().bit(MELEE_ACTION)
}

#[test]
fn a_melee_hits_on_its_key_frame_and_frees_the_trigger_after() {
    for (id, hit, free) in [(ASSAULT_RIFLE, 4, 29), (PISTOL, 5, 35)] {
        let (mut sim, p, _) = armed(id);
        let t = target(&mut sim, p, 1.5);
        let bit = melee_bit(&sim);
        let shield = sim.app.world().get::<Vitality>(t).unwrap().shield;
        let log = record(&mut sim, p, free + 2, |s, k| {
            let mut i = s.intent(p);
            i.actions = if k == 0 { bit } else { 0 };
            i.fire = k >= 1;
        });
        assert_eq!(ticks_of(&log, "melee"), [0], "{id}");
        assert_eq!(ticks_of(&log, "swing_hit"), [hit], "{id}");
        assert_eq!(shots(&log).first(), Some(&free), "{id}");
        assert!(sim.app.world().get::<Vitality>(t).unwrap().shield < shield, "{id}: the melee took shield");
    }
}

/// weapons.md 13: the melee's amount is (1 - s) minimum + U s with the
/// damage scale s from the attacker's forward speed, so a standing melee
/// (s = 0) deals the effect's minimum.
#[test]
fn a_standing_melee_deals_the_minimum() {
    use halo_ce::game::{damage::HaloHit, vitality::DamageEffects};
    for id in [ASSAULT_RIFLE, PISTOL] {
        let (mut sim, p, _) = armed(id);
        let t = target(&mut sim, p, 1.5);
        let bit = melee_bit(&sim);
        let mut cursor = sim.app.world().resource::<Messages<HaloHit>>().get_cursor_current();
        let mut hits = Vec::new();
        for k in 0..10 {
            sim.intent(p).actions = if k == 0 { bit } else { 0 };
            sim.ticks(1);
            hits.extend(cursor.read(sim.app.world().resource::<Messages<HaloHit>>()).filter(|h| h.target == t).cloned());
        }
        assert_eq!(hits.len(), 1, "{id}: one melee hit");
        let fx = sim.app.world().resource::<DamageEffects>().get(hits[0].effect).unwrap().clone();
        assert_eq!((hits[0].scale, hits[0].amount), (0.0, fx.lower), "{id}: the minimum");
    }
}

#[test]
fn the_pistols_zoom_halves_the_field_of_view() {
    let (mut sim, p, g) = armed(PISTOL);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.ticks(1);
    // The nominal 35 degrees as the camera's: vertical 22.73 degrees, 0.39672 rad (weapons.md 12).
    let z = *sim.app.world().get::<Zoomed>(p).expect("zoomed");
    let v = chimera_world::map::view_model::vertical_fov(z.fov);
    assert!((v.to_radians() - 0.39672).abs() < 1e-5, "{} -> {v}", z.fov);
    assert_eq!(sim.app.world().get::<HaloGun>(g).unwrap().zoom, 0);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.ticks(1);
    assert!(sim.app.world().get::<Zoomed>(p).is_none());
}

/// Fire `id` at a full Spartan 1.5 m ahead, aimed at `height` of its
/// box's height (the head from 0.83 up: `weapon::hitgroup_at`), until it
/// dies: the shots it took.
fn shots_to_kill(id: &str, height: f32, held: bool) -> u32 {
    let (mut sim, p, g) = armed(id);
    let size = sim.app.world().get::<Magazine>(g).map_or(0, |m| m.size);
    if size > 0 {
        set_rounds(&mut sim, g, size, 600);
    }
    let t = target(&mut sim, p, 1.5);
    let eye = sim.position(p) + sim.state(p).eye_offset;
    let b = *sim.app.world().get::<avian3d::prelude::ColliderAabb>(t).expect("the target's box");
    let aim = sim.position(t).with_y(b.min.y + (b.max.y - b.min.y) * height) - eye;
    {
        let i = &mut *sim.intent(p);
        i.yaw = (-aim.x).atan2(-aim.z);
        i.pitch = (aim.y / aim.xz().length()).atan();
    }
    let mut died = sim.app.world().resource::<Messages<Died>>().get_cursor_current();
    let mut fired = sim.app.world().resource::<Messages<HaloFired>>().get_cursor_current();
    let mut n = 0;
    for k in 0..600 {
        // A semi-automatic hand: a tap every 10 ticks; else held.
        sim.intent(p).fire = if held { true } else { k % 10 == 0 };
        sim.ticks(1);
        n += fired.read(sim.app.world().resource::<Messages<HaloFired>>()).filter(|f| f.owner == p).count() as u32;
        if died.read(sim.app.world().resource::<Messages<Died>>()).any(|d| d.entity == t) {
            return n;
        }
    }
    panic!("{id}: no kill in 600 ticks ({n} shots, health {})", sim.app.world().get::<Health>(t).unwrap().current);
}

#[test]
fn shots_to_kill_a_full_spartan_at_point_blank() {
    // projectiles.md "Shots to kill": pistol 5 to the body, 3 with the
    // third to the head (here every shot at the head); the assault rifle
    // 16; the plasma rifle 14 to 15 to the body; the shotgun one shot.
    let body = 0.7;
    let head = 0.92;
    assert_eq!(shots_to_kill(PISTOL, body, false), 5);
    assert_eq!(shots_to_kill(PISTOL, head, false), 3);
    assert_eq!(shots_to_kill(ASSAULT_RIFLE, body, true), 16);
    let pr = shots_to_kill(PLASMA_RIFLE, body, true);
    assert!((14..=15).contains(&pr), "plasma rifle: {pr}");
    assert_eq!(shots_to_kill(SHOTGUN, body, true), 1);
}

#[test]
fn under_a_64_hz_tick_the_rates_stay_the_specs() {
    // The assault rifle's rounds in 2 s and the pistol's cycle, within a
    // tick's worth.
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    sim.set_tick_interval(1.0 / 64.0);
    let (mut sim, p, _) = armed_in(sim, ASSAULT_RIFLE);
    let log = hold(&mut sim, p, 128);
    let n = shots(&log).len() as i32;
    assert!((n - 30).abs() <= 1, "{n} rounds in 2 s");
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    sim.set_tick_interval(1.0 / 64.0);
    let (mut sim, p, _) = armed_in(sim, PISTOL);
    let s = shots(&hold(&mut sim, p, 128));
    for w in s.windows(2) {
        let secs = (w[1] - w[0]) as f64 / 64.0;
        assert!((secs - 9.0 / 30.0).abs() <= 1.0 / 64.0 + 1e-9, "{s:?}");
    }
}
