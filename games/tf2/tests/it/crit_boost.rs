//! Crit boosts (specs/tf2/effects.md 14, `weapons::crit_boost` and the
//! Flame Thrower's `flame::sounds`) on the greybox: the boost's
//! `Weapon_General.CritPower` loop and `critgun_weaponmodel_<team>` glow
//! start with it, the loop goes on across a switch to another boostable
//! weapon while the glow moves, both stop at once when it ends; a Flame
//! Thrower burst turning critical swaps its loops with no start or end.

use bevy::prelude::*;
use chimera_sim::weapon::{Inventory, give};
use chimera_world::map::{
    PlaySound, StopSound,
    psys::{StartParticleSystem, StopParticleSystems},
};
use tf2::game::weapons::{BONESAW, FLAMETHROWER, SYRINGE_GUN, crit_boost::CritBoosted};

use super::weapons::{holding, sim};

/// What the tick sent: sounds played, sounds stopped, systems started
/// (name, tag), systems stopped (tag).
#[derive(Default, Debug)]
struct Heard {
    played: Vec<String>,
    stopped: Vec<String>,
    started: Vec<(String, Option<Entity>)>,
    stopped_tags: Vec<Entity>,
}

fn run(sim: &mut chimera_world::harness::Sim, ticks: u32, mut input: impl FnMut(&mut chimera_world::harness::Sim, u32)) -> Heard {
    let w = sim.app.world();
    let mut a = w.resource::<Messages<PlaySound>>().get_cursor_current();
    let mut b = w.resource::<Messages<StopSound>>().get_cursor_current();
    let mut c = w.resource::<Messages<StartParticleSystem>>().get_cursor_current();
    let mut d = w.resource::<Messages<StopParticleSystems>>().get_cursor_current();
    let mut h = Heard::default();
    for t in 0..ticks {
        input(sim, t);
        sim.ticks(1);
        let w = sim.app.world();
        h.played.extend(a.read(w.resource::<Messages<PlaySound>>()).map(|s| s.entry.clone()));
        h.stopped.extend(b.read(w.resource::<Messages<StopSound>>()).map(|s| s.entry.clone()));
        h.started.extend(c.read(w.resource::<Messages<StartParticleSystem>>()).map(|s| (s.name.clone(), s.tag)));
        h.stopped_tags.extend(d.read(w.resource::<Messages<StopParticleSystems>>()).map(|s| s.tag));
    }
    h
}

fn count(v: &[String], needle: &str) -> usize {
    v.iter().filter(|s| s.contains(needle)).count()
}

/// Test cases: a boosted Medic's loop and glow; switching to the Bonesaw
/// keeps the loop, moves the glow; the boost ending cuts both.
#[test]
fn the_boost_loops_crit_power_and_glows_on_the_held_weapon() {
    let mut sim = sim();
    let (p, gun) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), SYRINGE_GUN);
    let quiet = run(&mut sim, 10, |_, _| {});
    assert_eq!(count(&quiet.played, "CritPower"), 0, "no boost, no loop");
    sim.app.world_mut().entity_mut(p).insert(CritBoosted);
    let on = run(&mut sim, 10, |_, _| {});
    assert_eq!(count(&on.played, "Weapon_General.CritPower"), 1, "{on:?}");
    let glows: Vec<_> = on.started.iter().filter(|(n, _)| n.starts_with("critgun_weaponmodel_")).collect();
    assert_eq!(glows.len(), 1, "{on:?}");
    assert_eq!(glows[0].1, Some(gun), "on the gun");
    // To the Bonesaw: the loop goes on, the glow moves.
    let saw = give(sim.app.world_mut(), p, BONESAW).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(saw);
    let switched = run(&mut sim, 80, |_, _| {});
    assert_eq!(count(&switched.played, "CritPower"), 0, "not restarted: {switched:?}");
    assert_eq!(count(&switched.stopped, "CritPower"), 0, "not stopped: {switched:?}");
    assert!(switched.stopped_tags.contains(&gun), "the gun's glow stopped: {switched:?}");
    assert!(switched.started.iter().any(|(n, t)| n.starts_with("critgun_weaponmodel_") && *t == Some(saw)), "{switched:?}");
    // The boost ends: both cut.
    sim.app.world_mut().entity_mut(p).remove::<CritBoosted>();
    let off = run(&mut sim, 5, |_, _| {});
    assert_eq!(count(&off.stopped, "Weapon_General.CritPower"), 1, "{off:?}");
    assert!(off.stopped_tags.contains(&saw), "{off:?}");
}

/// Test case: a Flame Thrower burst turning critical while firing: FireLoop
/// cut, FireLoopCrit at once, no FireStart, no FireEnd.
#[test]
fn a_burst_turning_critical_swaps_the_flame_loops() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), FLAMETHROWER);
    let start = run(&mut sim, 30, |s, _| s.intent(p).fire = true);
    assert_eq!(count(&start.played, "FireThrower.FireStart") + count(&start.played, "FlameThrower.FireStart"), 1, "{start:?}");
    assert_eq!(count(&start.played, "FlameThrower.FireLoop") - count(&start.played, "FireLoopCrit"), 1, "{start:?}");
    sim.app.world_mut().entity_mut(p).insert(CritBoosted);
    let swap = run(&mut sim, 30, |s, _| s.intent(p).fire = true);
    assert_eq!(count(&swap.played, "FlameThrower.FireLoopCrit"), 1, "{swap:?}");
    assert_eq!(count(&swap.played, "FireStart"), 0, "no start: {swap:?}");
    assert_eq!(count(&swap.played, "FireEnd"), 0, "no end: {swap:?}");
    assert!(swap.stopped.iter().any(|s| s.ends_with("FlameThrower.FireLoop")), "the loop cut: {swap:?}");
    assert!(swap.stopped.iter().any(|s| s.ends_with("FireStart")), "the start cut: {swap:?}");
    let end = run(&mut sim, 30, |s, _| s.intent(p).fire = false);
    assert_eq!(count(&end.played, "FireEnd"), 1, "{end:?}");
}

/// effects.md 14.1: another player's glow stands on his held weapon's
/// hitboxes (its `$hitbox` attachments, where they are drawn): the weapon
/// gets them as its `ParticleModel` each frame, control point 0 at his
/// origin, drawn in the world pass.
#[test]
fn others_glow_stands_on_their_held_weapons_hitboxes_14_1() {
    use chimera_world::map::{psys::ParticleModel, view_model::HeldAttachment};
    let mut sim = sim();
    let (p, gun) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), SYRINGE_GUN);
    let at = Transform::from_translation(Vec3::new(0.2, 1.3, -0.4)).with_scale(Vec3::new(0.1, 0.2, 0.3));
    let name = source_engine::content::props::HITBOX_ATTACHMENT.to_string();
    sim.app.world_mut().spawn((HeldAttachment { owner: p, index: 3, name }, GlobalTransform::from(at)));
    sim.app.world_mut().entity_mut(p).insert(CritBoosted);
    run(&mut sim, 3, |_, _| {});
    let m = sim.app.world().get::<ParticleModel>(gun).expect("the gun carries its glow's model").clone();
    assert_eq!(m.boxes, vec![at.to_matrix()]);
    assert!(!m.view_model);
    let origin = sim.app.world().get::<Transform>(p).unwrap().translation;
    assert_eq!(m.origin, origin);
}
