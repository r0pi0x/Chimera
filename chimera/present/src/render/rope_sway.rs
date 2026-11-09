//! Ropes in the wind (specs/cs_source/ropes.md, "Wind"): a rope near the
//! camera keeps simulating after it settles (`map::rope_sway::RopeSway`),
//! its free nodes pushed by gusts (a map without env_wind; one with it
//! isn't swayed, env_wind not being simulated); away from the camera it
//! runs on until it rests and stops. The drawn points are the spline
//! through the nodes, between the last two steps.
//!
//! Ropes in the wind (specs/cs_source/ropes.md "Wind"): a settled rope's
//! simulation stepped again with the spec's gusts (`Gusts`) while the camera is
//! within `rope_wind_dist` (`RopeWind`), its mesh rebuilt from the spline
//! through its nodes. An end parented to an entity (`MapRope::follow`) goes
//! with it, and the rope moves again until it rests.
//!
//! Known shortcuts (docs/tech-debt.md): Ropes (from specs/cs_source/ropes.md)
//! settle at load and sway in the spec's gusts while the camera is within 1000
//! units (`render::rope_sway`; the gusts are random, never compared with
//! CS:S); on maps with an env_wind (cs_italy, de_cbble, de_dust) they hang
//! still, env_wind's own wind not being simulated; no rope collision
//! (`Collide`), drawn points between the last two steps, their light not
//! sampled again as they move. Rope gravity 800 is measured, not specced;
//! dust2's 10-node cable still hangs ~11 px low in the sky_n view. Node light
//! uses our prop probe averaged over six directions (the engine's point-light
//! query is not public). Why it's OK for now: The 5-node cables match CS:S to
//! 1-2 px.

use bevy::prelude::*;

use super::rope_material::rope_mesh;
use crate::map::{
    MapRope, SkyboxCamera, ViewModelCamera, monitor,
    rope_sway::{GUST_BASE, REST_THRESHOLD, RopeSway, STEP, UNIT, WIND_DIST, spline},
    water,
};

/// `rope_wind_dist`: how near (units) the camera must be to a rope for
/// the wind to blow it; 0 keeps every rope still (refcmp's captures).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct RopeWind {
    pub dist: f32,
}

impl Default for RopeWind {
    fn default() -> Self {
        Self { dist: WIND_DIST / UNIT }
    }
}

/// Gusts (no env_wind): the first 1-3 s after creation, each lasting
/// T = 2-3 s, the next starting 3-4 s after the last started; strength
/// 50 s u with s in -1..1 and u a random direction, shaped by
/// 1 - cos(π τ / T).
#[derive(Clone, Debug)]
pub struct Gusts {
    rng: u64,
    /// Seconds until the next gust starts.
    until_next: f32,
    /// The gust blowing: (seconds into it, its length, its vector).
    current: Option<(f32, f32, Vec3)>,
}

impl Gusts {
    pub fn new(seed: u64) -> Self {
        let mut g = Self {
            rng: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
            until_next: 0.0,
            current: None,
        };
        g.until_next = g.between(1.0, 3.0);
        g
    }

    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    fn between(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.random()
    }

    /// The wind's acceleration over the next `dt` seconds (m/s²), engine
    /// space, then the timers advance.
    pub fn advance(&mut self, dt: f32) -> Vec3 {
        if self.until_next <= 0.0 {
            let length = self.between(2.0, 3.0);
            let u = Vec3::new(
                self.between(-1.0, 1.0),
                self.between(-1.0, 1.0),
                self.between(-1.0, 1.0),
            )
            .normalize_or_zero();
            let s = self.between(-1.0, 1.0);
            self.current = Some((0.0, length, u * GUST_BASE * s));
            self.until_next += self.between(3.0, 4.0);
        }
        let accel = match self.current {
            Some((tau, length, v)) if tau < length => v * (1.0 - (std::f32::consts::PI * tau / length).cos()),
            _ => Vec3::ZERO,
        };
        if let Some((tau, ..)) = self.current.as_mut() {
            *tau += dt;
        }
        self.until_next -= dt;
        accel
    }
}

/// A rope that sways: its simulation, gusts, the look it is drawn with and
/// the mesh its strips share.
#[derive(Component)]
pub struct RopeMotion {
    pub sway: RopeSway,
    pub gusts: Gusts,
    pub look: MapRope,
    pub mesh: Handle<Mesh>,
    /// Time not yet stepped, seconds.
    pub pending: f32,
    pub resting: bool,
    /// Each end's parent and where the end sits in its frame (known once
    /// the parent is found).
    pub ends: [Option<RopeEnd>; 2],
}

/// A rope end that follows its parent entity.
#[derive(Clone, Copy, Debug)]
pub struct RopeEnd {
    pub entity: usize,
    pub on_parent: Option<(Entity, Vec3)>,
}

impl RopeEnd {
    pub fn of(follow: Option<usize>) -> Option<Self> {
        follow.map(|entity| Self { entity, on_parent: None })
    }
}

/// Keep ropes' parented ends on their parents; a moved end wakes the rope.
pub fn follow_rope_ends(
    mut ropes: Query<&mut RopeMotion>,
    parents: Query<(Entity, &crate::map::PropEntity, &GlobalTransform, Option<&crate::map::PropHome>)>,
    placed: Query<&GlobalTransform>,
) {
    for mut r in &mut ropes {
        let last = r.sway.nodes.len() - 1;
        for (k, node) in [(0, 0), (1, last)] {
            let Some(mut end) = r.ends[k] else { continue };
            if end.on_parent.is_none() {
                // Where the end was placed, in the parent's frame there.
                let Some((e, _, at, home)) = parents.iter().find(|(_, p, ..)| p.0 == end.entity) else { continue };
                let frame = home.map_or(at.compute_transform(), |h| h.0);
                end.on_parent = Some((e, frame.rotation.inverse() * (r.sway.nodes[node] - frame.translation)));
                r.ends[k] = Some(end);
            }
            let Some((parent, local)) = end.on_parent else { continue };
            let Ok(at) = placed.get(parent) else { continue };
            let pin = at.translation() + at.rotation() * local;
            if pin.distance_squared(r.sway.nodes[node]) > REST_THRESHOLD {
                r.sway.nodes[node] = pin;
                r.resting = false;
            }
        }
    }
}

/// Steps ropes the wind blows or that haven't come to rest, and redraws
/// them from their nodes between the last two steps.
pub fn sway_ropes(
    time: Res<Time>,
    cameras: Query<
        &GlobalTransform,
        (
            With<Camera3d>,
            Without<monitor::ScreenCamera>,
            Without<SkyboxCamera>,
            Without<ViewModelCamera>,
            Without<water::WaterReflectionCamera>,
        ),
    >,
    mut ropes: Query<&mut RopeMotion>,
    mut meshes: ResMut<Assets<Mesh>>,
    wind: Option<Res<RopeWind>>,
) {
    let Some(eye) = cameras.iter().next().map(|c| c.translation()) else {
        return;
    };
    let dist = wind.map_or(WIND_DIST, |w| w.dist * UNIT);
    let dt = time.delta_secs().min(0.1);
    for mut r in &mut ropes {
        let windy = r.sway.in_wind(eye, dist);
        if !windy && r.resting {
            continue;
        }
        r.pending += dt;
        while r.pending >= STEP {
            r.pending -= STEP;
            let wind = if windy { r.gusts.advance(STEP) } else { Vec3::ZERO };
            let gravity = r.sway.gravity;
            let moved = r.sway.step(gravity + wind);
            r.resting = !windy && moved <= REST_THRESHOLD;
            if r.resting {
                r.pending = 0.0;
                break;
            }
        }
        // Drawn between the last step and the next.
        let alpha = (r.pending / STEP).clamp(0.0, 1.0);
        let nodes: Vec<Vec3> = r
            .sway
            .previous
            .iter()
            .zip(&r.sway.nodes)
            .map(|(q, p)| q.lerp(*p, if r.resting { 1.0 } else { alpha }))
            .collect();
        let points: Vec<Vec3> = spline(&nodes, r.sway.subdiv).into_iter().map(|(p, ..)| p).collect();
        if points.len() != r.look.points.len() {
            continue;
        }
        r.look.points = points;
        let mesh = rope_mesh(&r.look);
        let handle = r.mesh.clone();
        let _ = meshes.insert(&handle, mesh);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gusts_follow_the_spec_timing_and_envelope() {
        let mut g = Gusts::new(5);
        let first = g.until_next;
        assert!((1.0..=3.0).contains(&first), "{first}");
        // Nothing before the first gust.
        let mut t = 0.0;
        while t + STEP < first {
            assert_eq!(g.advance(STEP), Vec3::ZERO);
            t += STEP;
        }
        // Then a gust: at most 100 units/s² (twice the base), back to 0
        // after its 2-3 s.
        let mut peak: f32 = 0.0;
        let mut blew = 0.0;
        for _ in 0..(3.2 / STEP) as usize {
            let a = g.advance(STEP);
            peak = peak.max(a.length());
            if a != Vec3::ZERO {
                blew += STEP;
            }
        }
        assert!(peak > 0.0 && peak <= 2.0 * GUST_BASE + 1e-6, "{peak}");
        assert!((1.9..=3.1).contains(&blew), "{blew}");
    }
}
