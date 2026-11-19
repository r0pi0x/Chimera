//! A TF2 gun as it comes into and leaves the hand (owner reports from
//! Ultimate Gun Game, where a kill swaps the gun at once): the Flame
//! Thrower's and the Minigun's loops stop when the gun is taken (a rung's
//! swap: `chimera_sim::weapon::held_sounds`), a Minigun given with attack
//! held starts idle and winds up only once drawn (`Inventory::ready`), and
//! the Flame Thrower's view model leaves its looping `ft_fire` for
//! `ft_idle` after the release instead of shaking in it
//! (`view_anim::ViewModelPlay::attacked`; specs/tf2/viewmodel.md 6, its
//! sequences as weapons_more.md 9.2 times them, on a one-bone stand-in).

use std::sync::Arc;

use bevy::prelude::*;
use chimera_sim::{
    minigame::runtime::take_weapon,
    weapon::{Inventory, WeaponEventKind, give, view_anim::ViewPose},
};
use chimera_world::{
    greybox,
    map::{
        MapModel, MapViewModel, StopSound, ViewAnimator, ViewModels,
        anim::{AnimSet, Animation, Sequence},
    },
};
use tf2::game::weapons::{
    FLAMETHROWER, MINIGUN,
    minigun::{SPIN_UP, Spin, Tf2Minigun},
};

use super::weapons::{holding, run, sim};

/// The stops written for `who` from now on (`read` after every tick:
/// messages last two updates).
struct Stops(bevy::ecs::message::MessageCursor<StopSound>);

impl Stops {
    fn new(sim: &chimera_world::harness::Sim) -> Self {
        Self(sim.app.world().resource::<Messages<StopSound>>().get_cursor_current())
    }
    fn read(&mut self, sim: &chimera_world::harness::Sim, who: Entity) -> Vec<String> {
        let m = sim.app.world().resource::<Messages<StopSound>>();
        self.0.read(m).filter(|s| s.source == who).map(|s| s.entry.clone()).collect()
    }
}

/// The ladder's swap: the gun taken, the next one given and drawn.
fn swap(sim: &mut chimera_world::harness::Sim, p: Entity, old: Entity, id: &str) -> Entity {
    take_weapon(sim.app.world_mut(), p, old);
    give(sim.app.world_mut(), p, id).unwrap()
}

#[test]
fn the_flame_loop_stops_when_a_kill_swaps_the_gun() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], FLAMETHROWER);
    run(&mut sim, p, 30, |s, _| s.intent(p).fire = true);
    let mut stops = Stops::new(&sim);
    let next = swap(&mut sim, p, w, MINIGUN);
    let mut stopped = Vec::new();
    for _ in 0..3 {
        sim.intent(p).fire = true;
        sim.ticks(1);
        stopped.extend(stops.read(&sim, p));
    }
    for entry in ["tf2:Weapon_FlameThrower.FireStart", "tf2:Weapon_FlameThrower.FireLoop"] {
        assert!(stopped.iter().any(|e| e == entry), "{entry} stopped: {stopped:?}");
    }
    // Once: the Minigun in hand stops nothing more while drawn.
    for _ in 0..30 {
        sim.intent(p).fire = true;
        sim.ticks(1);
        assert!(stops.read(&sim, p).is_empty());
    }
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(next));
}

#[test]
fn the_minigun_loops_stop_when_a_kill_swaps_the_gun() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], MINIGUN);
    run(&mut sim, p, 90, |s, _| s.intent(p).fire = true);
    assert!(matches!(sim.app.world().get::<Tf2Minigun>(w).unwrap().spin, Spin::Spun { .. }), "firing");
    let mut stops = Stops::new(&sim);
    swap(&mut sim, p, w, FLAMETHROWER);
    let mut stopped = Vec::new();
    for _ in 0..3 {
        sim.intent(p).fire = true;
        sim.ticks(1);
        stopped.extend(stops.read(&sim, p));
    }
    for entry in ["tf2:Weapon_Minigun.Fire", "tf2:Weapon_Minigun.Spin", "tf2:Weapon_Minigun.WindUp"] {
        assert!(stopped.iter().any(|e| e == entry), "{entry} stopped: {stopped:?}");
    }
    // Death stops them too.
    let mut sim2 = sim_with_minigun_firing();
    let (p2, _) = sim2.1;
    let mut stops = Stops::new(&sim2.0);
    sim2.0.app.world_mut().get_mut::<chimera_core::core::Health>(p2).unwrap().current = 0.0;
    let mut stopped = Vec::new();
    for _ in 0..2 {
        sim2.0.ticks(1);
        stopped.extend(stops.read(&sim2.0, p2));
    }
    assert!(stopped.iter().any(|e| e == "tf2:Weapon_Minigun.Fire"), "a death stops the loop: {stopped:?}");
}

fn sim_with_minigun_firing() -> (chimera_world::harness::Sim, (Entity, Entity)) {
    let mut sim = sim();
    let pw = holding(&mut sim, greybox::SPAWNS[0], MINIGUN);
    run(&mut sim, pw.0, 90, |s, _| s.intent(pw.0).fire = true);
    (sim, pw)
}

/// Given with attack held (the previous gun firing as the kill swaps it):
/// idle, upright and silent through the draw (0.5 s, weapons.md 3), then
/// it winds up as a fresh press would.
#[test]
fn a_minigun_given_with_attack_held_starts_idle_and_winds_up_once_drawn() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], FLAMETHROWER);
    run(&mut sim, p, 30, |s, _| s.intent(p).fire = true);
    let m = swap(&mut sim, p, w, MINIGUN);
    let mut during = Vec::new();
    let events = run(&mut sim, p, 30, |s, _| {
        s.intent(p).fire = true;
        let g = s.app.world().get::<Tf2Minigun>(m).unwrap();
        during.push((g.spin, s.app.world().get::<ViewPose>(m).copied()));
    });
    for (t, (spin, pose)) in during.iter().enumerate() {
        assert_eq!(*spin, Spin::Idle, "tick {t}: idle while drawn");
        assert_eq!(*pose, Some(ViewPose(0)), "tick {t}: upright while drawn");
    }
    assert!(!events.iter().any(|(_, k)| matches!(k, WeaponEventKind::Action(SPIN_UP))), "no wind-up during the draw");
    let events = run(&mut sim, p, 30, |s, _| s.intent(p).fire = true);
    let up: Vec<u64> = events.iter().filter(|(_, k)| matches!(k, WeaponEventKind::Action(SPIN_UP))).map(|(t, _)| *t).collect();
    assert_eq!(up.len(), 1, "it winds up once drawn: {events:?}");
    assert!(matches!(sim.app.world().get::<Tf2Minigun>(m).unwrap().spin, Spin::Up { .. }));
}

/// (name, frames, fps, looping): `c_pyro_arms`' Flame Thrower sequences
/// (weapons_more.md 9.2: `ft_draw` 1.0, `ft_idle` 2.0 loop, `ft_fire` 0.3
/// loop).
const FT: [(&str, usize, f32, bool); 3] = [("ft_draw", 31, 30.0, false), ("ft_idle", 61, 30.0, true), ("ft_fire", 10, 30.0, true)];

fn flame_view_model() -> MapViewModel {
    let defaults = vec![(Quat::IDENTITY, Vec3::ZERO)];
    let mut set = AnimSet { bases: vec![defaults.clone()], defaults, ..default() };
    for (i, (name, frames, fps, looping)) in FT.iter().enumerate() {
        set.animations.push(Animation { name: format!("a_{name}"), fps: *fps, frames: *frames, ..default() });
        set.sequences.push(Sequence {
            name: name.to_string(),
            activity: "ACT_PRIMARY_VM_IDLE".into(),
            activity_weight: 1,
            looping: *looping,
            fade_in: 0.2,
            fade_out: 0.2,
            grid: (1, 1),
            anims: vec![i],
            bone_weights: vec![1.0],
            ..default()
        });
    }
    MapViewModel {
        key: FLAMETHROWER.to_string(),
        model: MapModel::default(),
        bones: Vec::new(),
        root: Transform::default(),
        animations: Some(Arc::new(set)),
        right_handed: true,
        allow_flipping: false,
        attachments: Vec::new(),
        light_origin: Vec3::ZERO,
    }
}

fn sequence(sim: &chimera_world::harness::Sim, p: Entity) -> Option<String> {
    sim.app.world().get::<ViewAnimator>(p).and_then(|v| v.sequence().map(str::to_string))
}

/// Released, `ft_fire` plays out one length (0.3 s) and `ft_idle`
/// follows, staying put: before, the looping fire sequence never ended
/// and the arms shook in it until the next draw.
#[test]
fn the_flame_throwers_view_model_idles_after_the_release() {
    let mut sim = sim();
    sim.app.insert_resource(ViewModels::of(vec![flame_view_model()]));
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], FLAMETHROWER);
    assert_eq!(sequence(&sim, p).as_deref(), Some("ft_idle"), "idle after the draw");
    run(&mut sim, p, 60, |s, _| s.intent(p).fire = true);
    assert_eq!(sequence(&sim, p).as_deref(), Some("ft_fire"), "fire loops while held");
    let mut seen = Vec::new();
    run(&mut sim, p, 120, |s, _| {
        s.intent(p).fire = false;
        seen.push(sequence(s, p));
    });
    let idle_at = seen.iter().position(|s| s.as_deref() == Some("ft_idle")).expect("idles after the release");
    // 0.3 s at 0.015 s ticks: about 20 ticks, never much later.
    assert!(idle_at <= 24, "idle after {idle_at} ticks");
    assert!(seen[idle_at..].iter().all(|s| s.as_deref() == Some("ft_idle")), "stays idle: {seen:?}");
}
