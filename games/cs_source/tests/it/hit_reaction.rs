//! Being hit in CS:S (`games::cs_source::hit_reaction`, specs/cs_source/
//! weapons.md M18): the victim's slowdown and view flinch.
//!
//! - A runner at 250 (knife) hit in the chest by a USP slows as the probe
//!   measured: 235.0, 220.9, 207.65, ... 143.25, then 136.25 and +1.25 a
//!   tick back to 250 (within 0.05); its view pitches up by a tenth of
//!   the damage.
//! - Rifles set 0.65, legs 0.5 from anything; a bare head flinches
//!   (−12, 0, ±9), a helmeted head and a vest-covered or leg hit not.
//! - Over the network the victim's client gets the slowdown and flinch
//!   with its own state: after the one correction its prediction is the
//!   server's bit for bit.
//!
//! `cargo test --features dev --test it hit_reaction:: -- --nocapture`
//! prints the speeds.

use std::{collections::HashMap, time::Duration};

use avian3d::prelude::{ColliderAabb, Position};
use bevy::prelude::*;
use crate::stack::{
    core::{FriendlyFire, Health, Intent, MovementState, PredictedComponents, SimTick},
    games::cs_source::{
        TICK_INTERVAL,
        movement::{self, QuakeMovement, SourceMovementPlugin, to_engine, to_source},
        weapons::{AK47, CsWeaponsPlugin, DEAGLE, KNIFE, USP},
    },
    greybox::GreyboxMapPlugin,
    harness::{CsSim, NetSim, Sim},
    map::{MapBrush, MapBrushes},
    net::{
        memory::LinkConditions,
        predict::{NetGraph, PredictionHistory},
    },
    slots::Loadout,
    weapon::{Armor, Inventory, StartingWeapons, ViewPunch, give},
};

fn flat_floor(mut commands: Commands) {
    let (a, b) = (
        to_engine(Vec3::new(-16384.0, -16384.0, -64.0)),
        to_engine(Vec3::new(16384.0, 16384.0, 0.0)),
    );
    commands.insert_resource(MapBrushes(vec![MapBrush::from_box(a.min(b), a.max(b))]));
}

/// A flat floor, CS:S movement and weapons at CS:S's tick; everyone
/// starts with the knife.
fn sim() -> Sim {
    let mut sim = Sim::with(CsSim, |app| {
        app.insert_resource(StartingWeapons {
            team: vec![],
            all: vec![KNIFE],
        })
        .add_systems(Startup, flat_floor)
        .add_plugins((SourceMovementPlugin, CsWeaponsPlugin));
    });
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

fn speed(sim: &Sim, p: Entity) -> f32 {
    to_source(sim.velocity(p)).truncate().length()
}

fn modifier(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<QuakeMovement>(p).unwrap().velocity_modifier
}

/// The view punch in Source angles (pitch, yaw, roll; degrees).
fn punch(sim: &Sim, p: Entity) -> Vec3 {
    let v = sim.app.world().get::<ViewPunch>(p).map_or(Vec3::ZERO, |v| v.0);
    Vec3::new(-v.x, v.y, v.z).map(f32::to_degrees)
}

/// Aim `shooter` at the point `frac` up `target`'s bounds (0 feet, 1 top),
/// `ahead` meters past where it is now along its velocity.
fn aim(sim: &mut Sim, shooter: Entity, target: Entity, frac: f32, ticks_ahead: f32) {
    let b = *sim.app.world().get::<ColliderAabb>(target).unwrap();
    let lead = sim.velocity(target) * TICK_INTERVAL as f32 * ticks_ahead;
    let mut point = sim.position(target) + lead;
    point.y = b.min.y + (b.max.y - b.min.y) * frac;
    let eye = sim.position(shooter) + sim.state(shooter).eye_offset;
    let d = point - eye;
    let mut i = sim.intent(shooter);
    i.yaw = (-d.x).atan2(-d.z);
    i.pitch = d.y.atan2(d.xz().length());
}

fn shoot(sim: &mut Sim, shooter: Entity) {
    sim.intent(shooter).fire = true;
    sim.ticks(1);
    sim.intent(shooter).fire = false;
}

/// M18's trace: the speed on each tick from the first slowed one.
fn m18_speeds(n: usize) -> Vec<f32> {
    let first = [235.0, 220.9, 207.65, 195.19, 183.48, 172.47, 162.12, 152.39, 143.25];
    (0..n)
        .map(|k| match first.get(k) {
            Some(v) => *v,
            None => (250.0 * (0.5 + 0.005 * k as f32)).min(250.0),
        })
        .collect()
}

#[test]
fn a_usp_hit_slows_a_runner_as_m18_measured() {
    let mut sim = sim();
    // The runner faces Source +X (yaw −90°); the shooter stands behind it.
    let runner = sim.spawn_character(to_engine(Vec3::new(0.0, 0.0, 36.5)), movement::ID);
    let shooter = sim.spawn_character(to_engine(Vec3::new(-96.0, 0.0, 36.5)), movement::ID);
    sim.ticks(2);
    give(sim.app.world_mut(), shooter, USP).unwrap();
    sim.seconds(1.2);
    sim.intent(runner).yaw = -std::f32::consts::FRAC_PI_2;
    sim.intent(runner).move_axis = Vec2::Y;
    sim.ticks(60);
    assert!(
        (speed(&sim, runner) - 250.0).abs() < 1e-3,
        "running at {}",
        speed(&sim, runner)
    );
    aim(&mut sim, shooter, runner, 0.72, 1.0);
    shoot(&mut sim, shooter);
    let lost = (1.0 - sim.app.world().get::<Health>(runner).unwrap().current) * 100.0;
    assert!(lost > 0.0, "the shot hit");
    // Set after the hit tick's move: the next tick moves at 0.5.
    assert_eq!(modifier(&sim, runner), 0.5);
    let p = punch(&sim, runner);
    println!("USP chest hit: {lost:.0} hp, punch {p:?}");
    assert!(p.x < 0.0 && p.y == 0.0 && p.z == 0.0, "pitch up only: {p:?}");
    let kick = -p.x;
    assert!(
        kick >= lost * 0.1 - 1e-4 && kick < (lost + 1.0) * 0.1,
        "a tenth of the damage: {kick} for {lost}"
    );
    let want = m18_speeds(101);
    let mut got = Vec::new();
    for _ in 0..want.len() {
        sim.ticks(1);
        got.push(speed(&sim, runner));
    }
    println!("speeds: {:?}", &got[..14]);
    for (k, (g, w)) in got.iter().zip(&want).enumerate() {
        assert!((g - w).abs() < 0.05, "tick {k} after the hit: {g} (M18 {w})");
    }
    assert_eq!(modifier(&sim, runner), 1.0);
}

/// A shot at `frac` of a standing target's height with `gun`, the target
/// wearing `armor`: its velocity modifier and punch (Source degrees) at
/// the end of the hit tick.
fn hit(gun: &str, frac: f32, armor: Option<Armor>) -> (f32, Vec3) {
    let mut sim = sim();
    let target = sim.spawn_character(to_engine(Vec3::new(0.0, 0.0, 36.5)), movement::ID);
    let shooter = sim.spawn_character(to_engine(Vec3::new(-200.0, 0.0, 36.5)), movement::ID);
    if let Some(a) = armor {
        sim.app.world_mut().entity_mut(target).insert(a);
    }
    // Enough to live through a headshot (the dead don't slow down).
    sim.app.world_mut().get_mut::<Health>(target).unwrap().current = 5.0;
    sim.ticks(2);
    give(sim.app.world_mut(), shooter, gun).unwrap();
    sim.seconds(1.5);
    aim(&mut sim, shooter, target, frac, 0.0);
    let before = sim.app.world().get::<Health>(target).unwrap().current;
    shoot(&mut sim, shooter);
    assert!(
        sim.app.world().get::<Health>(target).unwrap().current < before,
        "{gun} at {frac} missed"
    );
    (modifier(&sim, target), punch(&sim, target))
}

const KEVLAR: Armor = Armor {
    amount: 1.0,
    helmet: false,
};
const HELMET: Armor = Armor {
    amount: 1.0,
    helmet: true,
};

#[test]
fn rifles_slow_less_legs_always_half_and_heads_flinch() {
    let (m, p) = hit(AK47, 0.72, None);
    assert_eq!(m, 0.65, "a rifle in the chest");
    // 36 × 0.98^(200/500) = 35.73 before truncation: −3.573.
    assert!(
        (p.x + 3.573).abs() < 0.01 && p.y == 0.0 && p.z == 0.0,
        "a tenth of the damage: {p:?}"
    );
    let (m, p) = hit(DEAGLE, 0.72, None);
    assert_eq!(m, 0.5, "a pistol, whatever its damage");
    assert_eq!(p, Vec3::new(-4.0, 0.0, 0.0), "44 damage and more: the 4° cap");
    let (m, p) = hit(AK47, 0.2, None);
    assert_eq!(m, 0.5, "a rifle in the leg");
    assert_eq!(p, Vec3::ZERO, "no flinch for legs");
    let (m, p) = hit(USP, 0.2, None);
    assert_eq!(m, 0.5);
    assert_eq!(p, Vec3::ZERO);
    // Armour doesn't change the slowdown; the vest takes the flinch.
    let (m, p) = hit(AK47, 0.72, Some(KEVLAR));
    assert_eq!(m, 0.65);
    assert_eq!(p, Vec3::ZERO, "the vest took part: no flinch");
    // The head: bare or with a vest but no helmet, −12 pitch and ±9 roll.
    for armor in [None, Some(KEVLAR)] {
        let (m, p) = hit(USP, 0.92, armor);
        assert_eq!(m, 0.5);
        assert!(
            (p.x + 12.0).abs() < 1e-4 && p.y == 0.0 && (p.z.abs() - 9.0).abs() < 1e-4,
            "{p:?}"
        );
    }
    // A helmet: no flinch (unmeasured, our choice).
    let (_, p) = hit(USP, 0.92, Some(HELMET));
    assert_eq!(p, Vec3::ZERO);
}

#[test]
fn the_flinch_decays_with_the_punch_rule() {
    let mut sim = sim();
    let p = sim.spawn_character(to_engine(Vec3::new(0.0, 0.0, 36.5)), movement::ID);
    sim.ticks(2);
    let head = crate::stack::games::cs_source::hit_reaction::to_view_punch(Vec3::new(-12.0, 0.0, 9.0));
    sim.app.world_mut().entity_mut(p).insert(head);
    sim.ticks(1);
    let after = punch(&sim, p);
    assert!(
        (after - Vec3::new(-11.79, 0.0, 8.8425)).abs().max_element() < 1e-3,
        "{after:?}"
    );
    sim.seconds(2.0);
    assert_eq!(punch(&sim, p), Vec3::ZERO);
}

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, SourceMovementPlugin, CsWeaponsPlugin))
        .insert_resource(Loadout { movement: movement::ID });
}

fn client_intent(sim: &mut NetSim, i: usize) -> Mut<'_, Intent> {
    let me = sim.local_player(i).unwrap();
    sim.clients[i].app.world_mut().get_mut::<Intent>(me).unwrap()
}

fn place(sim: &mut NetSim, i: usize, at: Vec3) {
    let e = sim.character_of(i).unwrap();
    let world = sim.server.app.world_mut();
    world.get_mut::<Transform>(e).unwrap().translation = at;
    if let Some(mut p) = world.get_mut::<Position>(e) {
        p.0 = at;
    }
    *client_intent(sim, i) = Intent::default();
}

#[test]
fn the_victims_client_predicts_its_slowdown_after_the_hit() {
    let conditions = LinkConditions {
        latency: Duration::from_millis(60),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CsSim, conditions, 61, 2, setup);
    sim.until_joined(600);
    sim.ticks(320);
    sim.server.app.insert_resource(FriendlyFire(1));
    let victim = sim.character_of(0).unwrap();
    // The victim runs away (-Z) down the shooter's line of fire.
    place(&mut sim, 0, Vec3::new(0.0, 1.0, 6.0));
    place(&mut sim, 1, Vec3::new(0.0, 1.0, 12.0));
    sim.ticks(80);
    let shooter = sim.local_player(1).unwrap();
    assert!(
        sim.clients[1]
            .app
            .world()
            .get::<Inventory>(shooter)
            .unwrap()
            .active
            .is_some()
    );
    let mut server_states: HashMap<u64, Vec<u8>> = HashMap::new();
    let (mut compared, mut after_hit, mut least, mut flinched) = (0, 0, 1.0f32, false);
    let mut hit_tick = None;
    let mut wrong = std::collections::BTreeSet::new();
    let errors_before = sim.clients[0].app.world().resource::<NetGraph>().errors;
    for k in 0..200u32 {
        client_intent(&mut sim, 0).move_axis = if k < 120 { Vec2::Y } else { Vec2::ZERO };
        // The shooter aims at the victim's chest as it draws it.
        let id = sim.client_id(0);
        let seen = NetSim::owned_by(&mut sim.clients[1], Some(id)).unwrap();
        let me = sim.local_player(1).unwrap();
        let w = sim.clients[1].app.world();
        let eye = w.get::<Transform>(me).unwrap().translation + w.get::<MovementState>(me).unwrap().eye_offset;
        let d = w.get::<Transform>(seen).unwrap().translation + Vec3::Y * 0.2 - eye;
        let mut i = client_intent(&mut sim, 1);
        i.yaw = (-d.x).atan2(-d.z);
        i.pitch = d.y.atan2(d.xz().length());
        i.fire = k == 40;
        sim.step();
        let world = sim.server.app.world();
        let tick = world.resource::<SimTick>().0;
        let m = world.get::<QuakeMovement>(victim).unwrap().velocity_modifier;
        if m < 1.0 && hit_tick.is_none() {
            hit_tick = Some(tick);
        }
        least = least.min(m);
        server_states.insert(tick, world.resource::<PredictedComponents>().encode(world, victim));
        let me = sim.local_player(0).unwrap();
        let ours = sim.clients[0].app.world();
        flinched |= ours.get::<ViewPunch>(me).is_some_and(|p| p.0 != Vec3::ZERO);
        // The client can't know of the hit: its first prediction of the
        // ticks from the hit on is wrong until the server's state puts it
        // right (re-running the commands since). A tick that differs is
        // compared again on later steps.
        let history = &ours.resource::<PredictionHistory>().0;
        for p in history.iter().filter(|p| !p.state.is_empty()) {
            let Some(s) = server_states.get(&p.tick) else { continue };
            if *s == p.state {
                server_states.remove(&p.tick);
                wrong.remove(&p.tick);
                compared += 1;
                if hit_tick.is_some_and(|h| p.tick > h) {
                    after_hit += 1;
                }
            } else {
                wrong.insert(p.tick);
            }
        }
    }
    let hit = hit_tick.expect("the shot hit");
    // What stayed wrong: only the ticks from the hit to the first state the
    // server sent after it (it sends one every few ticks: `cl_updaterate`),
    // which put the client right; every tick predicted after that is the
    // server's.
    assert!(
        wrong.iter().all(|t| (hit..hit + 5).contains(t)),
        "predictions never put right: {wrong:?} (hit at {hit})"
    );
    let errors = sim.clients[0].app.world().resource::<NetGraph>().errors - errors_before;
    println!(
        "hit at {hit_tick:?}: modifier down to {least}, {compared} ticks compared ({after_hit} after the hit), {errors} prediction errors"
    );
    assert!((0.64..=0.66).contains(&least), "the AK-47 sets 0.65: {least}");
    assert!(flinched, "the victim's view flinched on its client");
    assert!(after_hit > 100, "the slowed ticks were predicted ({after_hit})");
    assert!((1..=2).contains(&errors), "one correction for the hit ({errors})");
}
