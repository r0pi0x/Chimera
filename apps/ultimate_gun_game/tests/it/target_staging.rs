//! The sandbox targets' staging modes for animation pictures
//! (chimera_sim's `minigame::targets`: `jumps`, `crouches`, `runs`; TF2's
//! `bot_jump`, `bot_crouch` and a bot's forced movement, tools/tf2ref's
//! `anim.scene`), set from the console as a scene sets them. On the
//! party's simulation, headless.

use bevy::prelude::*;
use chimera_core::{
    console::Console,
    core::{LocalPlayer, MovementState},
};
use chimera_sim::minigame::{
    self,
    targets::{self, RUN_HALF, RunDir, SpawnTarget, Target},
};
use chimera_world::harness::Sim;
use combat_arms::game::movement;
use ultimate_gun_game::sandbox::SANDBOX;

use crate::parts::ugg;

/// A TF2 Demoman with his Grenade Launcher (TF2's movement, as the
/// scenes stage him).
const BODY: &str = "tf2:demoman";
const GUN: &str = "tf2:tf_weapon_grenadelauncher";

/// The sandbox with one still target 10 m ahead of the local player.
fn staged() -> (Sim, Entity) {
    let mut sim = ugg();
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(1.5);
    let e = targets::spawn(sim.app.world_mut(), &SpawnTarget { character: Some(BODY.into()), gun: Some(GUN.into()), distance_m: 10.0, ..default() })
        .unwrap_or_else(|e| panic!("the target: {e}"));
    sim.seconds(1.5);
    (sim, e)
}

/// A console line's reply (or error).
fn say(sim: &mut Sim, line: &str) -> String {
    let before = sim.app.world().resource::<Console>().output.len();
    sim.app.world_mut().resource_mut::<Console>().submit(line);
    sim.ticks(2);
    sim.app.world().resource::<Console>().output[before..].iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
}

fn target(sim: &Sim, e: Entity) -> Target {
    sim.app.world().get::<Target>(e).cloned().expect("a target")
}

#[test]
fn a_crouching_target_stays_crouched_until_told() {
    let (mut sim, e) = staged();
    let reply = say(&mut sim, "target set 1 crouch");
    assert!(reply.contains("crouched"), "{reply}");
    sim.seconds(1.0);
    assert!(target(&sim, e).crouches);
    assert!(sim.app.world().get::<MovementState>(e).unwrap().crouching, "crouched");
    say(&mut sim, "target set 1 nocrouch");
    sim.seconds(1.0);
    assert!(!sim.app.world().get::<MovementState>(e).unwrap().crouching, "standing again");
}

#[test]
fn a_jumping_target_jumps_again_on_each_landing_in_place() {
    let (mut sim, e) = staged();
    let home = target(&sim, e).home;
    let floor = sim.position(e).y;
    say(&mut sim, "target set 1 jump");
    let (mut takeoffs, mut was_on, mut top) = (0, true, floor);
    for _ in 0..(4 * 64) {
        sim.ticks(1);
        let on = sim.app.world().get::<MovementState>(e).unwrap().on_ground;
        if was_on && !on {
            takeoffs += 1;
        }
        was_on = on;
        top = top.max(sim.position(e).y);
    }
    assert!(takeoffs >= 3, "jumped again and again in 4 s: {takeoffs}");
    assert!(top > floor + 0.3, "off the floor: up to {top} from {floor}");
    assert!((sim.position(e) - home).xz().length() < 1.0, "in place");
    say(&mut sim, "target set 1 nojump");
    sim.seconds(1.5);
    let rest = sim.position(e).y;
    sim.seconds(1.0);
    assert!((sim.position(e).y - rest).abs() < 0.01, "on the floor again");
}

#[test]
fn a_running_target_runs_one_way_as_it_faces_over_its_track() {
    for (word, dir) in [("forward", RunDir::Forward), ("left", RunDir::Left), ("back", RunDir::Back)] {
        let (mut sim, e) = staged();
        let reply = say(&mut sim, &format!("target set 1 run {word}"));
        assert!(reply.contains(&format!("runs {word}")), "{reply}");
        let t = target(&sim, e);
        assert_eq!(t.runs, Some(dir));
        // It faces back along its line (toward the spawner): forward is
        // -dir, left is the spawner's right.
        let way = match dir {
            RunDir::Forward => -t.dir,
            RunDir::Back => t.dir,
            RunDir::Left => Vec3::new(-t.dir.z, 0.0, t.dir.x),
            RunDir::Right => Vec3::new(t.dir.z, 0.0, -t.dir.x),
        };
        // A new run starts at its track's start, behind home.
        let start = (sim.position(e) - t.home).dot(way);
        assert!((start + RUN_HALF).abs() < 0.5, "{word}: from the track's start: {start}");
        let (mut wraps, mut moving, mut last) = (0, 0, sim.position(e));
        for _ in 0..(4 * 64) {
            sim.ticks(1);
            let now = sim.position(e);
            let step = (now - last).xz();
            if step.length() > 2.0 {
                wraps += 1;
            } else if step.length() > 0.02 {
                moving += 1;
                assert!(step.normalize().dot(way.xz()) > 0.9, "{word}: each step {way:?}: {step:?}");
            }
            let along = (now - t.home).dot(way);
            assert!(along <= RUN_HALF + 0.3 && along >= -RUN_HALF - 0.3, "{word}: on its track: {along}");
            last = now;
        }
        assert!(moving > 100, "{word}: running ({moving} ticks)");
        assert!(wraps >= 1, "{word}: put back at the start");
        // A motion ends it.
        say(&mut sim, "target set 1 still");
        assert_eq!(target(&sim, e).runs, None);
    }
}

#[test]
fn a_run_needs_a_direction() {
    let (mut sim, e) = staged();
    let reply = say(&mut sim, "target set 1 run up");
    assert!(reply.contains("run <forward|back|left|right>"), "{reply}");
    assert_eq!(target(&sim, e).runs, None);
}
