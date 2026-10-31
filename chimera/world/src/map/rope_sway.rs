//! Ropes' simulation (specs/cs_source/ropes.md, "Wind"), for the loaders
//! (settling at load) and `render::rope_sway` (swaying in the wind
//! after): the step is the settling one (`Verlet` with damping, three
//! max-length link sweeps, ends pinned) in engine space, and the drawn
//! points are the spline through the nodes.
//!
//! Ropes' simulation (specs/cs_source/ropes.md "Wind"): `RopeSway` (a settled
//! rope's nodes, stepped; whether the camera is near enough for the wind),
//! `spline` for the loaders and the drawing.

use bevy::prelude::*;

/// Fixed simulation step, seconds.
pub const STEP: f32 = 1.0 / 50.0;
const DAMPING: f32 = 0.98;
const SWEEPS: usize = 3;
const MAX_SUBDIV: usize = 7;
pub const UNIT: f32 = 0.0254;
/// Wind only within this of the line between the ends (rope_wind_dist's
/// default).
pub const WIND_DIST: f32 = 1000.0 * UNIT;

/// A gust's strength before its envelope, m/s².
pub const GUST_BASE: f32 = 50.0 * UNIT;
/// A node is still moving while its squared step displacement exceeds this.
pub const REST_THRESHOLD: f32 = 0.03 * UNIT * UNIT;

/// The drawn points: every node, with `subdiv` (clamped to 0-7) uniform
/// Catmull-Rom points between neighbours. Also returns, per point, the
/// index of the node before it and how far it is toward the next one.
pub fn spline(nodes: &[Vec3], subdiv: usize) -> Vec<(Vec3, usize, f32)> {
    let s = subdiv.min(MAX_SUBDIV);
    let n = nodes.len();
    let mut out = vec![(nodes[0], 0, 0.0)];
    for i in 0..n - 1 {
        let p0 = nodes[i.saturating_sub(1)];
        let (p1, p2) = (nodes[i], nodes[i + 1]);
        let p3 = nodes[(i + 2).min(n - 1)];
        for k in 1..=s {
            let t = k as f32 / (s + 1) as f32;
            let c = p1
                + (p2 - p0) * (t / 2.0)
                + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * (t * t / 2.0)
                + (-p0 + p1 * 3.0 - p2 * 3.0 + p3) * (t * t * t / 2.0);
            out.push((c, i, t));
        }
        out.push((p2, i + 1, 0.0));
    }
    out
}

/// A settled rope that can move again: its nodes (engine space, meters),
/// their positions a step ago, the link's maximum length and gravity.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RopeSway {
    pub nodes: Vec<Vec3>,
    pub previous: Vec<Vec3>,
    pub link: f32,
    /// m/s².
    pub gravity: Vec3,
    pub subdiv: usize,
    /// Gusts push it (no NoWind key, no env_wind in the map).
    pub wind: bool,
}

impl RopeSway {
    /// One fixed step with `accel` (gravity plus wind, m/s²) on every node
    /// (the ends are pinned again); returns the largest squared node
    /// displacement.
    pub fn step(&mut self, accel: Vec3) -> f32 {
        let (start, end) = (self.nodes[0], *self.nodes.last().unwrap());
        // Acceleration times step²/2, not step² (the spec's quirk).
        let push = accel * STEP * STEP / 2.0;
        for (p, q) in self.nodes.iter_mut().zip(self.previous.iter_mut()) {
            let next = *p + (*p - *q) * DAMPING + push;
            *q = *p;
            *p = next;
        }
        let last = self.nodes.len() - 1;
        for _ in 0..SWEEPS {
            for k in 0..last {
                let d = self.nodes[k] - self.nodes[k + 1];
                let len = d.length();
                if len > self.link {
                    let fix = d * (0.5 * (1.0 - self.link / len));
                    self.nodes[k] -= fix;
                    self.nodes[k + 1] += fix;
                }
            }
            self.nodes[0] = start;
            self.nodes[last] = end;
        }
        self.nodes
            .iter()
            .zip(&self.previous)
            .map(|(p, q)| p.distance_squared(*q))
            .fold(0.0, f32::max)
    }

    /// Whether `eye` is close enough for the wind to blow it: within
    /// `dist` (meters; `WIND_DIST` by default) of the line between its
    /// ends.
    pub fn in_wind(&self, eye: Vec3, dist: f32) -> bool {
        let (a, b) = (self.nodes[0], *self.nodes.last().unwrap());
        let d = b - a;
        let t = if d.length_squared() > 0.0 {
            ((eye - a).dot(d) / d.length_squared()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.wind && eye.distance(a + d * t) < dist
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rope() -> RopeSway {
        // 256 units across, 4 links of 72 units (Slack 132): a sag.
        let a = Vec3::ZERO;
        let b = Vec3::X * 256.0 * UNIT;
        let nodes: Vec<Vec3> = (0..5).map(|i| a.lerp(b, i as f32 / 4.0)).collect();
        RopeSway {
            previous: nodes.clone(),
            nodes,
            link: 72.0 * UNIT,
            gravity: Vec3::NEG_Y * 800.0 * UNIT,
            subdiv: 2,
            wind: true,
        }
    }

    #[test]
    fn a_rope_settles_and_the_wind_moves_it_sideways() {
        let mut r = rope();
        for _ in 0..2000 {
            r.step(r.gravity);
        }
        let settled = r.nodes.clone();
        assert!(settled[2].y < -0.1, "it sags: {}", settled[2]);
        // Still: a step changes nothing measurable.
        assert!(r.step(r.gravity) <= REST_THRESHOLD);
        // A steady sideways push (a gust's peak) moves the middle off the
        // plane of its ends, the ends stay pinned.
        for _ in 0..200 {
            r.step(r.gravity + Vec3::Z * 2.0 * GUST_BASE);
        }
        assert!(r.nodes[2].z > 0.01, "{}", r.nodes[2]);
        assert_eq!((r.nodes[0], r.nodes[4]), (settled[0], settled[4]));
    }

    #[test]
    fn wind_only_near_the_camera_and_unless_off() {
        let mut r = rope();
        assert!(r.in_wind(Vec3::new(1.0, 2.0, 0.0), WIND_DIST));
        assert!(!r.in_wind(Vec3::new(3.0, 0.0, WIND_DIST + 0.1), WIND_DIST));
        // rope_wind_dist 0: still.
        assert!(!r.in_wind(Vec3::new(1.0, 2.0, 0.0), 0.0));
        r.wind = false;
        assert!(!r.in_wind(Vec3::new(1.0, 2.0, 0.0), WIND_DIST));
    }
}
