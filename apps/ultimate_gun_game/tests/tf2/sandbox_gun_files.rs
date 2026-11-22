//! A sandbox gun switch waits for the gun's files (chimera_sim's
//! `minigame::rung_command`, "Waiting for a gun's files", on the seam
//! `minigame::gun_files`, faked here: a gun is not ready while the test
//! says so): the player keeps the old gun meanwhile, the switch lands the
//! frame the files are in, a second pick replaces the first, a gun ready
//! at once (a failed download) switches at once, a target is placed at
//! once and takes its gun when ready, and nothing stays pending once the
//! sandbox ends. On the party's simulation, headless.

use std::collections::BTreeSet;

use bevy::prelude::*;
use chimera_core::{console::Console, core::LocalPlayer};
use chimera_sim::{
    minigame::{
        self,
        gun_files::GunFiles,
        ladder::{Ladder, Rung},
        rung_command::{GunPending, GunPickEnded, GunPickOutcome, HoldRungs, PutOnGun},
        targets::{self, SpawnTarget, Target},
    },
    testkit,
};
use chimera_world::harness::Sim;
use combat_arms::game::movement;
use ultimate_gun_game::sandbox::SANDBOX;

use crate::parts::ugg;

const GUN: &str = "combat_arms:m4a1";
const SNIPER: &str = "tf2:tf_weapon_sniperrifle";
const ROCKETS: &str = "tf2:tf_weapon_rocketlauncher";

/// The fake asset side: the guns whose files are not in, and every ask.
#[derive(Resource, Default)]
struct Files {
    missing: BTreeSet<String>,
    asked: Vec<String>,
}

fn fake(w: &mut World, id: &str) -> bool {
    let mut f = w.resource_mut::<Files>();
    f.asked.push(id.to_string());
    !f.missing.contains(id)
}

#[derive(Resource, Default)]
struct Ended(Vec<GunPickEnded>);

fn keep(mut seen: ResMut<Ended>, mut r: MessageReader<GunPickEnded>) {
    seen.0.extend(r.read().cloned());
}

fn play() -> (Sim, Entity) {
    let mut sim = ugg();
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.app.init_resource::<Ended>().add_systems(Last, keep);
    sim.seconds(1.5);
    run(&mut sim, &format!("rung {GUN}"));
    sim.seconds(0.5);
    assert_eq!(testkit::active_id(sim.app.world(), me), Some(GUN));
    // The seam faked from here (any real hook replaced).
    sim.app.world_mut().insert_resource(Files::default());
    sim.app.world_mut().insert_resource(GunFiles(vec![fake]));
    (sim, me)
}

fn run(sim: &mut Sim, line: &str) -> String {
    let before = sim.app.world().resource::<Console>().output.len();
    sim.app.world_mut().resource_mut::<Console>().submit(line.to_string());
    sim.app.update();
    sim.app.update();
    sim.app.world().resource::<Console>().output[before..].iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
}

fn missing(sim: &mut Sim, guns: &[&str]) {
    sim.app.world_mut().resource_mut::<Files>().missing = guns.iter().map(|g| g.to_string()).collect();
}

fn rung_of(sim: &Sim, gun: &str) -> usize {
    sim.app.world().resource::<Ladder>().rungs.iter().position(|r| *r == gun).expect("on the ladder")
}

fn pending(sim: &Sim, e: Entity) -> Option<&'static str> {
    sim.app.world().get::<GunPending>(e).map(|p| p.gun)
}

fn ended(sim: &mut Sim) -> Vec<(&'static str, GunPickOutcome)> {
    std::mem::take(&mut sim.app.world_mut().resource_mut::<Ended>().0).into_iter().map(|e| (e.gun, e.outcome)).collect()
}

#[test]
fn the_old_gun_is_kept_until_the_files_are_in_and_the_switch_lands_that_frame() {
    let (mut sim, me) = play();
    missing(&mut sim, &[SNIPER]);
    sim.app.world_mut().write_message(PutOnGun { gun: SNIPER.into(), character: None, who: None });
    for _ in 0..40 {
        sim.app.update();
    }
    assert_eq!(testkit::active_id(sim.app.world(), me), Some(GUN), "the old gun meanwhile");
    assert_eq!(pending(&sim, me), Some(SNIPER));
    let p = sim.app.world().get::<GunPending>(me).cloned().unwrap();
    assert_eq!(p.rung, rung_of(&sim, SNIPER));
    let asks = sim.app.world().resource::<Files>().asked.iter().filter(|a| *a == SNIPER).count();
    assert!(asks >= 40, "asked each frame: {asks}");
    missing(&mut sim, &[]);
    sim.app.update();
    assert_eq!(sim.app.world().get::<Rung>(me).map(|r| r.0), Some(rung_of(&sim, SNIPER)), "switched the frame it is ready");
    assert_eq!(pending(&sim, me), None);
    assert_eq!(ended(&mut sim), vec![(SNIPER, GunPickOutcome::Applied)]);
    sim.ticks(5);
    assert_eq!(testkit::active_id(sim.app.world(), me), Some(SNIPER), "in hand once drawn");
    // Held there by the sandbox's rule as any switch.
    sim.seconds(0.5);
    assert_eq!(testkit::active_id(sim.app.world(), me), Some(SNIPER));
}

#[test]
fn a_second_pick_replaces_the_first_and_a_gun_ready_at_once_switches_at_once() {
    let (mut sim, me) = play();
    missing(&mut sim, &[SNIPER, ROCKETS]);
    let said = run(&mut sim, &format!("rung {SNIPER}"));
    assert!(said.contains("waiting for its files"), "{said}");
    run(&mut sim, &format!("rung {ROCKETS}"));
    assert_eq!(pending(&sim, me), Some(ROCKETS));
    assert_eq!(ended(&mut sim), vec![(SNIPER, GunPickOutcome::Replaced)]);
    // The first one's files come: nothing (only the latest is applied).
    missing(&mut sim, &[ROCKETS]);
    for _ in 0..10 {
        sim.app.update();
    }
    assert_eq!(testkit::active_id(sim.app.world(), me), Some(GUN));
    missing(&mut sim, &[]);
    sim.app.update();
    assert_eq!(sim.app.world().get::<Rung>(me).map(|r| r.0), Some(rung_of(&sim, ROCKETS)), "switched the frame it is ready");
    assert_eq!(ended(&mut sim), vec![(ROCKETS, GunPickOutcome::Applied)]);
    sim.ticks(5);
    assert_eq!(testkit::active_id(sim.app.world(), me), Some(ROCKETS));
    // Ready at once (no asset side, an installed game, a failed download):
    // the switch at once, nothing pending, no fact.
    let said = run(&mut sim, &format!("rung {SNIPER}"));
    assert!(!said.contains("waiting"), "{said}");
    assert_eq!(sim.app.world().get::<Rung>(me).map(|r| r.0), Some(rung_of(&sim, SNIPER)), "at once");
    sim.ticks(5);
    assert_eq!(testkit::active_id(sim.app.world(), me), Some(SNIPER));
    assert_eq!(pending(&sim, me), None);
    assert!(ended(&mut sim).is_empty());
}

#[test]
fn a_target_is_placed_at_once_and_takes_its_gun_when_ready_and_nothing_waits_once_the_sandbox_ends() {
    let (mut sim, me) = play();
    missing(&mut sim, &[SNIPER]);
    let t = targets::spawn(sim.app.world_mut(), &SpawnTarget { gun: Some(SNIPER.into()), distance_m: 8.0, ..default() })
        .unwrap_or_else(|e| panic!("{e}"));
    sim.seconds(1.0);
    let home = sim.app.world().get::<Target>(t).unwrap().home;
    assert!((sim.position(t) - home).xz().length() < 0.5, "placed at once: {} vs {home}", sim.position(t));
    assert_ne!(testkit::active_id(sim.app.world(), t), Some(SNIPER), "not its gun yet");
    assert_eq!(pending(&sim, t), Some(SNIPER));
    missing(&mut sim, &[]);
    sim.app.update();
    assert_eq!(sim.app.world().get::<Rung>(t).map(|r| r.0), Some(rung_of(&sim, SNIPER)), "its rung the frame it is ready");
    sim.ticks(5);
    assert_eq!(testkit::active_id(sim.app.world(), t), Some(SNIPER), "its gun");
    // A pick pending when the sandbox ends: dropped.
    missing(&mut sim, &[ROCKETS]);
    run(&mut sim, &format!("rung {ROCKETS}"));
    assert_eq!(pending(&sim, me), Some(ROCKETS));
    ended(&mut sim);
    sim.app.world_mut().remove_resource::<HoldRungs>();
    sim.app.update();
    sim.app.update();
    let left = sim.app.world_mut().query::<&GunPending>().iter(sim.app.world()).count();
    assert_eq!(left, 0, "nothing pending outside the sandbox");
    assert!(ended(&mut sim).contains(&(ROCKETS, GunPickOutcome::Dropped)));
}

#[test]
fn gun_files_hold_makes_a_switch_wait_until_off() {
    let (mut sim, me) = play();
    run(&mut sim, &format!("gun_files_hold {SNIPER}"));
    run(&mut sim, &format!("rung {SNIPER}"));
    assert_eq!(pending(&sim, me), Some(SNIPER));
    assert_eq!(testkit::active_id(sim.app.world(), me), Some(GUN));
    run(&mut sim, "gun_files_hold off");
    assert_eq!(pending(&sim, me), None);
    assert_eq!(sim.app.world().get::<Rung>(me).map(|r| r.0), Some(rung_of(&sim, SNIPER)));
}
