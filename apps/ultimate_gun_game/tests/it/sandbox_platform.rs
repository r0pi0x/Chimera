//! A practice target on the range's platform (chimera_sim's
//! `minigame::targets`, `TargetPlace::Platform`, placed by
//! `minigame::range_place::platform_spot`): it stands on the platform's
//! top, faces the firing line and is hit from there (no distance line:
//! `line_m` None, `distance_m` the true one); refused off the range; a
//! layout repeats it on the platform; the console's `platform` and
//! `line`. On the party's simulation, headless.

use bevy::prelude::*;
use chimera_core::{console::Console, core::LocalPlayer};
use chimera_sim::{
    minigame::{
        self,
        layout::{RepeatLayout, SandboxLayout},
        targets::{self, HitMeasured, PlaceTarget, RangeLine, SetTarget, SpawnTarget, Target, TargetPlace, TargetRefused},
    },
    testkit,
};
use chimera_world::{greybox, harness::Sim, range::RangeLayout};
use combat_arms::game::movement;
use ultimate_gun_game::sandbox::SANDBOX;

use crate::parts::ugg;

const GUN: &str = "combat_arms:m4a1";
const SNIPER: &str = "tf2:tf_weapon_sniperrifle";

#[derive(Resource, Default)]
struct Seen {
    hits: Vec<HitMeasured>,
    refused: Vec<String>,
}

fn keep(mut seen: ResMut<Seen>, mut h: MessageReader<HitMeasured>, mut r: MessageReader<TargetRefused>) {
    seen.hits.extend(h.read().cloned());
    seen.refused.extend(r.read().map(|r| r.reason.clone()));
}

/// The sandbox playing on `map` (`range` or `greybox`) with the local
/// player `me` just behind the firing line (the greybox: at its middle).
fn play(map: &str) -> (Sim, Entity, Option<RangeLayout>) {
    let mut sim = ugg();
    let w = sim.app.world_mut();
    greybox::unload(w);
    greybox::select(w, map);
    greybox::respawn(w);
    sim.ticks(2);
    let l = sim.app.world().get_resource::<RangeLayout>().cloned();
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let at = l.as_ref().map_or(Vec3::new(0.0, 1.0, 0.0), |l| l.point(-0.5, 0.0) + Vec3::Y);
    let me = sim.spawn_character(at, movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.app.init_resource::<Seen>().add_systems(FixedPostUpdate, keep).add_systems(Last, keep);
    sim.seconds(1.5);
    (sim, me, l)
}

fn run(sim: &mut Sim, line: &str) -> String {
    let before = sim.app.world().resource::<Console>().output.len();
    sim.app.world_mut().resource_mut::<Console>().submit(line.to_string());
    sim.ticks(3);
    sim.app.world().resource::<Console>().output[before..].iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
}

fn target(sim: &Sim, e: Entity) -> Target {
    sim.app.world().get::<Target>(e).cloned().expect("a target")
}

/// On the platform's top: within the radius assumed (1.2 m) of its centre,
/// at its height.
fn on_platform(l: &RangeLayout, p: Vec3) -> bool {
    (p.xz() - l.platform_top.xz()).length() < 1.3 && (p.y - l.platform_top.y).abs() < 0.3
}

#[test]
fn a_platform_target_stands_on_the_platform_and_is_hit_from_the_firing_line() {
    let (mut sim, me, l) = play("range");
    let l = l.expect("the range is in");
    let e = targets::spawn_placed(
        sim.app.world_mut(),
        &SpawnTarget { gun: Some(GUN.into()), protected: true, ..default() },
        TargetPlace::Platform,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    sim.seconds(2.0);
    let t = target(&sim, e);
    assert_eq!(t.place, TargetPlace::Platform);
    assert!(on_platform(&l, t.home), "its home on the platform's top: {} vs {}", t.home, l.platform_top);
    let feet = sim.position(e) - Vec3::Y * chimera_world::map::SPAWN_LIFT;
    assert!((feet.xz() - t.home.xz()).length() < 0.5 && feet.y > l.platform_top.y - 0.5, "the body up there: {feet}");
    // From the firing line's middle: its true flat distance, no line.
    let origin = l.point(0.0, 0.0) + Vec3::Y;
    assert!((t.origin - origin).length() < 0.05, "{}", t.origin);
    assert!((t.distance_m - (t.home - origin).xz().length()).abs() < 0.05, "{}", t.distance_m);
    assert!(sim.app.world().get::<RangeLine>(e).is_none());
    // Hit from the firing line with a hitscan gun.
    run(&mut sim, &format!("rung {SNIPER}"));
    sim.seconds(1.5);
    assert_eq!(testkit::active_id(sim.app.world(), me), Some(SNIPER));
    sim.app.world_mut().resource_mut::<Seen>().hits.clear();
    let at = sim.position(e);
    testkit::aim_at(&mut sim, me, at);
    let mut hit = None;
    for i in 0..200 {
        sim.intent(me).fire = i % 20 < 2;
        sim.ticks(1);
        hit = sim.app.world().resource::<Seen>().hits.iter().find(|h| h.victim == e && h.attacker == Some(me)).cloned();
        if hit.is_some() {
            break;
        }
    }
    sim.intent(me).fire = false;
    let hit = hit.expect("hit from the firing line");
    assert_eq!(hit.line_m, None, "a platform target stands on no distance line");
    let true_distance = sim.position(me).distance(sim.position(e));
    assert!((hit.distance_m - true_distance).abs() < 0.5, "{} vs {true_distance}", hit.distance_m);
    // A moving platform target: refused; back on a line, and up again.
    let said = run(&mut sim, "target set 1 strafe");
    assert!(said.to_lowercase().contains("platform"), "{said}");
    sim.app.world_mut().write_message(SetTarget { place: Some(TargetPlace::Line), distance_m: Some(20.0), ..SetTarget::of(e) });
    sim.seconds(1.0);
    let t = target(&sim, e);
    assert_eq!((t.place, t.distance_m), (TargetPlace::Line, 20.0));
    assert_eq!(sim.app.world().get::<RangeLine>(e).map(|r| r.distance_m), Some(20.0));
    let said = run(&mut sim, "target set 1 platform");
    assert!(said.contains("platform"), "{said}");
    sim.seconds(1.0);
    assert!(on_platform(&l, target(&sim, e).home));
    assert!(sim.app.world().get::<RangeLine>(e).is_none());
}

#[test]
fn the_platform_is_refused_off_the_range() {
    let (mut sim, _me, l) = play("greybox");
    assert!(l.is_none(), "not the range");
    let e = targets::spawn_placed(sim.app.world_mut(), &SpawnTarget { gun: Some(GUN.into()), ..default() }, TargetPlace::Platform)
        .unwrap_err();
    assert!(e.contains("platform") && e.contains("range"), "{e}");
    sim.app.world_mut().write_message(PlaceTarget {
        target: SpawnTarget { gun: Some(GUN.into()), ..default() },
        place: TargetPlace::Platform,
    });
    sim.ticks(3);
    assert!(sim.app.world().resource::<Seen>().refused.iter().any(|r| r.contains("platform")), "a TargetRefused");
    let said = run(&mut sim, &format!("target add {GUN} platform"));
    assert!(said.contains("platform") && !said.contains("added"), "{said}");
    assert!(targets::all_targets(sim.app.world_mut()).is_empty());
}

#[test]
fn a_layout_repeats_the_platform_target() {
    let (mut sim, _me, l) = play("range");
    let l = l.expect("the range is in");
    let said = run(&mut sim, &format!("target add {GUN} platform"));
    assert!(said.contains("added"), "{said}");
    run(&mut sim, &format!("target add {GUN} 20"));
    let layout = sim.app.world().resource::<SandboxLayout>().clone();
    assert_eq!(layout.targets.iter().map(|t| t.place).collect::<Vec<_>>(), vec![TargetPlace::Platform, TargetPlace::Line]);
    sim.app.world_mut().write_message(RepeatLayout::default());
    sim.seconds(1.5);
    let ts: Vec<Target> = targets::all_targets(sim.app.world_mut()).into_iter().map(|(_, t)| t).collect();
    assert_eq!(ts.len(), 2, "{ts:?}");
    assert!(ts[0].place == TargetPlace::Platform && on_platform(&l, ts[0].home), "{:?}", ts[0]);
    assert!(ts[1].place == TargetPlace::Line && (ts[1].distance_m - 20.0).abs() < 1e-3, "{:?}", ts[1]);
}
