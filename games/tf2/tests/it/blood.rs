//! TF2's blood on a player hit (specs/tf2/effects.md 8, its test cases;
//! `weapons::blood`) on the greybox: one burst per damage event (a
//! Scattergun's pellets on one victim one burst, a stain trace set per
//! pellet by its own 6 points), at the hit point for bullets, at the
//! attacker's eye + 48 along his view for melee; none from the Flame
//! Thrower or for a teammate with friendly fire off.

use bevy::prelude::*;
use chimera_core::core::{FriendlyFire, MovementState, Team};
use chimera_sim::testkit;
use chimera_world::{greybox, harness::Sim};
use tf2::game::weapons::{
    FLAMETHROWER, SCATTERGUN, SMG, WRENCH,
    blood::{Tf2Blood, Tf2Stains},
};

use super::weapons::{hold_still, holding, run, sim, target};

const U: f32 = 0.0254;

/// `id` fired once at a target `units` ahead (held `ticks` long): its
/// bursts and stains, the shooter and the target.
fn shoot(id: &str, units: f32, ticks: u64, setup: impl FnOnce(&mut Sim, Entity, Entity)) -> (Vec<Tf2Blood>, Vec<Tf2Stains>, Entity, Entity, Sim) {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, id);
    let t = target(&mut sim, from, units);
    hold_still(&mut sim, t);
    setup(&mut sim, p, t);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let mut bursts = sim.app.world().resource::<Messages<Tf2Blood>>().get_cursor_current();
    let mut stains = sim.app.world().resource::<Messages<Tf2Stains>>().get_cursor_current();
    let (mut b, mut s) = (Vec::new(), Vec::new());
    for k in 0..ticks {
        run(&mut sim, p, 1, |s, _| s.intent(p).fire = k == 0);
        let w = sim.app.world();
        b.extend(bursts.read(w.resource::<Messages<Tf2Blood>>()).copied());
        s.extend(stains.read(w.resource::<Messages<Tf2Stains>>()).copied());
    }
    (b, s, p, t, sim)
}

#[test]
fn an_smg_bullet_makes_one_burst_at_the_hit_and_one_stain_trace_set() {
    let (b, s, p, t, sim) = shoot(SMG, 180.0, 20, |_, _, _| {});
    assert_eq!(b.len(), 1, "{b:?}");
    assert!(b[0].at.distance(sim.position(t)) < 40.0 * U, "on the body: {:?}", b[0]);
    assert!(b[0].toward.dot((sim.position(p) - sim.position(t)).normalize()) > 0.9, "toward the shooter: {:?}", b[0]);
    assert!(!b[0].under_water);
    assert_eq!(s.len(), 1);
    assert!((s[0].points - 8.0).abs() < 0.01, "the SMG's own 8: {}", s[0].points);
}

#[test]
fn scattergun_pellets_on_one_victim_are_one_burst() {
    let (b, s, ..) = shoot(SCATTERGUN, 100.0, 20, |_, _, _| {});
    assert_eq!(b.len(), 1, "one damage event: {b:?}");
    assert!(s.len() >= 2, "a stain set per pellet: {}", s.len());
    assert!(s.iter().all(|s| (s.points - 6.0).abs() < 0.01), "{s:?}");
}

#[test]
fn a_wrench_hit_bleeds_at_the_eye_48_ahead() {
    let (b, s, p, _, sim) = shoot(WRENCH, 40.0, 40, |_, _, _| {});
    assert_eq!(b.len(), 1, "{b:?}");
    let eye = sim.position(p) + sim.app.world().get::<MovementState>(p).unwrap().eye_offset;
    let off = b[0].at.distance(eye) / U;
    assert!((off - 48.0).abs() < 1.0, "48 ahead of the eye: {off}");
    assert_eq!(s.len(), 1);
    assert!(s[0].points >= 25.0, "4 traces: {}", s[0].points);
}

#[test]
fn flames_and_teammates_never_bleed() {
    let (b, s, ..) = shoot(FLAMETHROWER, 100.0, 40, |_, _, _| {});
    assert!(b.is_empty() && s.is_empty(), "{b:?}");
    let (b, s, ..) = shoot(SMG, 180.0, 20, |sim, p, t| {
        sim.app.world_mut().entity_mut(p).insert(Team(2));
        sim.app.world_mut().entity_mut(t).insert(Team(2));
        sim.app.world_mut().insert_resource(FriendlyFire(0));
    });
    assert!(b.is_empty() && s.is_empty(), "{b:?}");
}

/// effects.md 8b's rules for the body decals: a TF2 character healed to
/// full (health rose, at least the max) has them all removed
/// (`ClearBodyDecals`, drawn by Chimera's `render::body_decals`); a rise
/// short of full or a fall does nothing; its ragdoll keeps none
/// (`BodyDecalsOffRagdoll`).
#[test]
fn healed_to_full_clears_body_decals_and_the_ragdoll_keeps_none() {
    use chimera_core::core::Health;
    use chimera_world::map::{BodyDecalsOffRagdoll, ClearBodyDecals};
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, SMG);
    sim.ticks(2);
    assert!(sim.app.world().get::<BodyDecalsOffRagdoll>(p).is_some(), "a TF2 character's ragdoll keeps none");
    let mut cleared = sim.app.world().resource::<Messages<ClearBodyDecals>>().get_cursor_current();
    let mut heal = |sim: &mut Sim, to: f32| {
        let max = sim.app.world().get::<Health>(p).unwrap().max;
        sim.app.world_mut().get_mut::<Health>(p).unwrap().current = to * max;
        sim.ticks(1);
        cleared.read(sim.app.world().resource::<Messages<ClearBodyDecals>>()).filter(|c| c.0 == p).count()
    };
    assert_eq!(heal(&mut sim, 0.4), 0, "hurt");
    assert_eq!(heal(&mut sim, 0.9), 0, "healed, not to full");
    assert_eq!(heal(&mut sim, 1.0), 1, "healed to full");
    assert_eq!(heal(&mut sim, 1.0), 0, "unchanged");
}
