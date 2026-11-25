//! A course for the movement's seam tests, 2 000 units over the greybox's
//! floor (nothing of the greybox is near it), in units with Z up and the
//! run along +X: a floor, a 12-unit step, a 20-degree ramp with a drop
//! behind it, a ceiling 50 units up (ducked only), a wall. The same solids
//! two ways: as the map's brushes for the box sweep (`brushes`), and as
//! clip hulls (`BrushHulls`: each solid's planes pushed out by the hull's
//! box, a point traced through them with the box sweep's back-off), so a
//! hull-traced run must go where the swept one goes.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::core::{Intent, MapBrush};
use chimera_movement::{
    ClipHulls, HullContents, HullHit, MapClipHulls, QUAKE_CVARS, QuakeConfig, QuakeMovement, QuakeMovementAppExt, to_engine,
    to_units,
};
use chimera_sim::SimBasePlugins;
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim, map::MapBrushes};

/// The course's origin, units.
pub const BASE: Vec3 = Vec3::new(0.0, 0.0, 2000.0);

/// The transform over the feet, units (the standing box's centre).
pub const ORIGIN_ABOVE_FEET: f32 = 36.0;

pub const STEP_X: f32 = 200.0;
pub const STEP_HEIGHT: f32 = 12.0;
pub const RAMP_X: (f32, f32) = (400.0, 600.0);
pub const CEILING_X: (f32, f32) = (700.0, 800.0);
pub const CEILING_Z: f32 = 50.0;
pub const WALL_X: f32 = 1000.0;

/// How far a trace stops short of a plane, units (the box sweep's).
const BACKOFF: f32 = 1.0 / 32.0;

/// The ramp's top over the floor.
pub fn ramp_top() -> f32 {
    (RAMP_X.1 - RAMP_X.0) * 20f32.to_radians().tan()
}

/// A solid as planes (normal, distance: inside is n . p <= d) in units
/// relative to `BASE`, and its bounds.
#[derive(Clone)]
pub struct Solid {
    pub planes: Vec<(Vec3, f32)>,
    pub min: Vec3,
    pub max: Vec3,
}

fn block(min: Vec3, max: Vec3) -> Solid {
    Solid {
        planes: vec![
            (Vec3::X, max.x),
            (Vec3::NEG_X, -min.x),
            (Vec3::Y, max.y),
            (Vec3::NEG_Y, -min.y),
            (Vec3::Z, max.z),
            (Vec3::NEG_Z, -min.z),
        ],
        min,
        max,
    }
}

/// The course's solids.
pub fn solids() -> Vec<Solid> {
    let a = 20f32.to_radians();
    let slope = Vec3::new(-a.sin(), 0.0, a.cos());
    let mut ramp = block(Vec3::new(RAMP_X.0, -200.0, 0.0), Vec3::new(RAMP_X.1, 200.0, ramp_top()));
    ramp.planes.push((slope, slope.dot(Vec3::new(RAMP_X.0, 0.0, 0.0))));
    vec![
        block(Vec3::new(-200.0, -200.0, -16.0), Vec3::new(1200.0, 200.0, 0.0)),
        block(Vec3::new(STEP_X, -200.0, 0.0), Vec3::new(STEP_X + 100.0, 200.0, STEP_HEIGHT)),
        ramp,
        block(Vec3::new(CEILING_X.0, -200.0, CEILING_Z), Vec3::new(CEILING_X.1, 200.0, CEILING_Z + 30.0)),
        block(Vec3::new(WALL_X, -200.0, 0.0), Vec3::new(WALL_X + 16.0, 200.0, 200.0)),
    ]
}

/// The course as map brushes (engine space).
pub fn brushes() -> Vec<MapBrush> {
    solids()
        .iter()
        .map(|s| {
            let (a, b) = (to_engine(s.min + BASE), to_engine(s.max + BASE));
            MapBrush {
                planes: s
                    .planes
                    .iter()
                    .map(|(n, d)| (Vec3::new(n.x, n.z, -n.y), (d + n.dot(BASE)) * 0.0254))
                    .collect(),
                min: a.min(b),
                max: a.max(b),
                ladder: false,
                surface: None,
            }
        })
        .collect()
}

/// The course as clip hulls: hull 0 the standing box (32 x 32 x 72 about
/// its centre), hull 1 the ducked one (36 high).
pub struct BrushHulls {
    solids: Vec<Solid>,
    boxes: [(Vec3, Vec3); 2],
}

impl BrushHulls {
    pub fn new() -> Self {
        Self {
            solids: solids(),
            boxes: [
                (Vec3::new(-16.0, -16.0, -36.0), Vec3::new(16.0, 16.0, 36.0)),
                (Vec3::new(-16.0, -16.0, -18.0), Vec3::new(16.0, 16.0, 18.0)),
            ],
        }
    }
}

impl ClipHulls for BrushHulls {
    fn boxes(&self) -> &[(Vec3, Vec3)] {
        &self.boxes
    }

    fn contents(&self, hull: usize, point: Vec3) -> HullContents {
        let half = self.boxes[hull].1;
        let p = point - BASE;
        let inside = |s: &Solid| s.planes.iter().all(|(n, d)| n.dot(p) - (d + n.abs().dot(half)) < -0.01);
        if self.solids.iter().any(inside) { HullContents::Solid } else { HullContents::Empty }
    }

    fn trace(&self, hull: usize, start: Vec3, end: Vec3) -> HullHit {
        let half = self.boxes[hull].1;
        let (from, delta) = (start - BASE, end - start);
        let mut best = HullHit { fraction: 1.0, end, normal: Vec3::ZERO, start_solid: false, all_solid: false };
        'solid: for s in &self.solids {
            let (mut enter, mut leave) = (-1.0f32, 1.0f32);
            let (mut starts_out, mut normal, mut last) = (false, Vec3::ZERO, -1.0f32);
            for (n, d) in &s.planes {
                let d1 = n.dot(from) - (d + n.abs().dot(half));
                let d2 = d1 + n.dot(delta);
                starts_out |= d1 > 0.0;
                if d1 > 0.0 && (d2 > 0.0 || d2 >= d1 - 1e-3) {
                    continue 'solid;
                }
                if d1 <= 0.0 && d2 <= 0.0 {
                    continue;
                }
                if d1 > d2 {
                    enter = enter.max(((d1 - BACKOFF) / (d1 - d2)).max(0.0));
                    let crossed = d1 / (d1 - d2);
                    if crossed > last {
                        last = crossed;
                        normal = *n;
                    }
                } else {
                    leave = leave.min(((d1 + BACKOFF) / (d1 - d2)).min(1.0));
                }
            }
            if !starts_out {
                return HullHit { fraction: 0.0, end: start, normal: Vec3::ZERO, start_solid: true, all_solid: true };
            }
            if enter < leave && enter > -1.0 && enter < best.fraction {
                best = HullHit { fraction: enter, end: start + delta * enter, normal, start_solid: false, all_solid: false };
            }
        }
        best
    }
}

/// The test movement's id.
pub const ID: &str = "course:movement";

/// How a run meets the course's solids.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum World {
    /// As map brushes (the box sweep).
    Brushes,
    /// As clip hulls alone (no brush of the course in the map).
    Hulls,
    /// As both.
    Both,
}

/// A sim on the greybox with the course as `world` and the test movement
/// on `config`, and a character standing at the course's start.
pub fn sim(config: QuakeConfig, world: World) -> (Sim, Entity) {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins(GreyboxMapPlugin).quake_movement(ID, config, &[QUAKE_CVARS], None);
    });
    sim.repeatable();
    sim.ticks(2);
    if world != World::Hulls {
        let mut all = sim.app.world().resource::<MapBrushes>().0.clone();
        all.extend(brushes());
        sim.app.world_mut().insert_resource(MapBrushes(all));
    }
    if world != World::Brushes {
        sim.app.world_mut().insert_resource(MapClipHulls(Arc::new(BrushHulls::new())));
    }
    let p = sim.spawn_character(to_engine(BASE + Vec3::Z * (ORIGIN_ABOVE_FEET + 1.0)), ID);
    // Facing +X (an intent's yaw 0 looks along +Y).
    sim.intent(p).yaw = -std::f32::consts::FRAC_PI_2;
    sim.ticks(20);
    (sim, p)
}

/// A character's feet (units, relative to the course) and its movement
/// state.
pub fn feet(sim: &Sim, p: Entity) -> Vec3 {
    to_units(sim.position(p)) - Vec3::Z * ORIGIN_ABOVE_FEET - BASE
}

pub fn state(sim: &Sim, p: Entity) -> QuakeMovement {
    sim.app.world().get::<QuakeMovement>(p).unwrap().clone()
}

/// Put a character's feet at `at` (units, relative to the course), still.
pub fn place(sim: &mut Sim, p: Entity, at: Vec3) {
    let to = to_engine(at + BASE + Vec3::Z * ORIGIN_ABOVE_FEET);
    let w = sim.app.world_mut();
    w.get_mut::<Transform>(p).unwrap().translation = to;
    if let Some(mut pos) = w.get_mut::<avian3d::prelude::Position>(p) {
        pos.0 = to;
    }
    w.get_mut::<chimera_core::core::Velocity>(p).unwrap().0 = Vec3::ZERO;
}

/// The course's script for tick `t`: a run over the step and up the ramp,
/// off its top, a jump on the way, ducked under the ceiling, into the wall.
pub fn script(t: u64, feet: Vec3, i: &mut Intent) {
    i.move_axis = Vec2::Y;
    i.jump = t == 40;
    i.crouch = feet.x > CEILING_X.0 - 80.0 && feet.x < CEILING_X.1 + 40.0;
}
