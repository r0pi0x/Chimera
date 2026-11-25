//! Edge friction (`QuakeConfig::edge_friction`; docs/plans/active/
//! shared-movement.md, S9; specs/cs16/movement.md, "Ground friction"): the
//! friction is multiplied while the box, put 16 units ahead along the
//! velocity with its centre at the feet's height and traced 34 units down,
//! meets nothing. On the course's 80-unit block that is with the body's
//! centre past the edge (the probe is as wide as the body); on a floor,
//! and with the factor at 1, the friction is the usual.

use bevy::prelude::*;
use chimera_core::core::Velocity;
use chimera_movement::{QuakeConfig, to_engine, to_units};

use crate::course::{self, World};

/// The speed lost in one tick sliding at 200 units/s along +X from `at`
/// (feet, relative to the course) with no keys, on `config`.
fn loss(config: QuakeConfig, at: Vec3) -> f32 {
    let (mut sim, p) = course::sim(config, World::Brushes);
    course::place(&mut sim, p, at + Vec3::Z * 0.1);
    sim.ticks(10);
    assert!(course::state(&sim, p).on_ground, "standing at {at}");
    sim.app.world_mut().get_mut::<Velocity>(p).unwrap().0 = to_engine(Vec3::X * 200.0);
    sim.ticks(1);
    assert!(course::state(&sim, p).on_ground, "still on the ground at {at}");
    200.0 - to_units(sim.velocity(p)).x
}

#[test]
fn friction_doubles_over_an_edge_and_only_there() {
    let edged = QuakeConfig { edge_friction: 2.0, ..QuakeConfig::reference() };
    let top = course::CEILING_Z + 30.0;
    // On the block, well inside it; and with the centre 4 units past its
    // edge (12 of the box's 32 still on it), the 80-unit drop ahead.
    let inside = Vec3::new(course::CEILING_X.0 + 40.0, 0.0, top);
    let over = Vec3::new(course::CEILING_X.1 + 4.0, 0.0, top);
    let usual = loss(QuakeConfig::reference(), inside);
    assert!(usual > 1.0, "friction slows the slide: {usual}");
    assert!((loss(edged.clone(), inside) - usual).abs() < 1e-3, "no edge ahead: the usual friction");
    assert!((loss(edged.clone(), over) - 2.0 * usual).abs() < 1e-2, "over the edge: twice the friction");
    assert!((loss(QuakeConfig::reference(), over) - usual).abs() < 1e-3, "factor 1: the usual friction over the edge");
    // A drop shorter than the probe reaches (34 units and the box's half
    // height below the feet) is no edge: the 12-unit step.
    let step = Vec3::new(course::STEP_X + 100.0 + 4.0, 0.0, course::STEP_HEIGHT);
    assert!((loss(edged, step) - usual).abs() < 1e-3, "a step down is no edge");
}
