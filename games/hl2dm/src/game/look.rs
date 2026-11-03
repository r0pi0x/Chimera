//! Where HL2:DM's drawn bodies look (specs/hl2dm/player_models.md 6.4,
//! 11.3; clients only, never networked, never the server's hitboxes):
//! each frame every living body's look target is the eyes of the first
//! other player (dead ones and the local one included) within 128 units,
//! origin to origin, not behind its feet's facing; else a point 512 units
//! ahead. The head turns toward it (`head_yaw` within its ±44.8 degree
//! range, `head_pitch` within -20 to 23) at up to 130 degrees/s, the head
//! counter-turning while the feet turn; the eyes aim at it (`EyeTarget`,
//! drawn by `render::bodies` through `map::face`). Citizens blink (11.2:
//! every U(1.5, 4) s a blink of `blink_duration` 0.2 s, shut in 32 ms),
//! every other controller at 0: the face's rules, FACS lids and mesh flex
//! weights each frame (`faces`: `map::FaceWeights`, which the drawn face
//! meshes follow).
//!
//! Known shortcuts: players are taken in the query's order, not the
//! server's player index; the lids take the eyes' rest gaze (not the eye
//! aim of 11.3); `flex_smooth` isn't applied; a ragdoll's face keeps the
//! weights it had at death (spec 10.2: forced to rest).

use bevy::prelude::*;
use source_engine::player_anim::normalize;

use super::player_anim::Hl2PlayerAnim;
use crate::{
    core::{Health, Intent, MovementState},
    map::{EyeTarget, anim::Animator},
};

/// Source units in a meter.
const UNITS: f32 = 1.0 / 0.0254;
/// Other players within this are looked at (units).
pub const LOOK_RADIUS: f32 = 128.0;
/// Otherwise the target is this far ahead (units).
pub const LOOK_AHEAD: f32 = 512.0;
/// The head's turn rate (degrees/s).
pub const HEAD_TURN_RATE: f32 = 130.0;
pub const HEAD_YAW_RANGE: f32 = 44.8187;
pub const HEAD_PITCH_RANGE: (f32, f32) = (-20.0, 23.0);

/// A body's head (degrees) and the feet yaw it last saw.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Head {
    pub yaw: f32,
    pub pitch: f32,
    pub feet: f32,
}

/// Engine forward of Source yaw `yaw` (degrees).
pub fn forward(yaw: f32) -> Vec3 {
    let r = yaw.to_radians();
    Vec3::new(-r.sin(), 0.0, -r.cos())
}

/// Source yaw and pitch (degrees, pitch positive down) of engine vector
/// `d`.
pub fn angles(d: Vec3) -> (f32, f32) {
    let yaw = (-d.x).atan2(-d.z).to_degrees();
    let pitch = -d.y.atan2(Vec2::new(d.x, d.z).length()).to_degrees();
    (yaw, pitch)
}

/// `current` moved toward `goal` by at most `step` degrees, the short way.
pub fn approach(goal: f32, current: f32, step: f32) -> f32 {
    let d = normalize(goal - current);
    if d.abs() <= step { goal } else { normalize(current + step * d.signum()) }
}

/// One head update toward world target `target` from eye `eye`, feet yaw
/// `feet` (§6.4 steps 4-5): the new head yaw and pitch.
pub fn turn_head(head: &mut Head, eye: Vec3, target: Vec3, feet: f32, dt: f32) {
    let (yaw, pitch) = angles(target - eye);
    let step = HEAD_TURN_RATE * dt;
    let want = normalize(yaw - feet).clamp(-HEAD_YAW_RANGE, HEAD_YAW_RANGE);
    let turned = approach(want, head.yaw, step);
    head.yaw = normalize(turned - normalize(feet - head.feet));
    head.feet = feet;
    let want = normalize(pitch).clamp(HEAD_PITCH_RANGE.0, HEAD_PITCH_RANGE.1);
    head.pitch = approach(want, head.pitch, step);
}

/// Each drawn HL2:DM body's look target, head and eyes (§6.4).
#[allow(clippy::type_complexity)]
pub fn look(
    time: Res<Time>,
    mut bodies: Query<(
        Entity,
        &Hl2PlayerAnim,
        &mut Animator,
        &GlobalTransform,
        &MovementState,
        (Option<&mut Head>, Option<&mut EyeTarget>),
        Option<&Health>,
    )>,
    players: Query<(Entity, &GlobalTransform, &MovementState), With<Intent>>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    let all: Vec<(Entity, Vec3, Vec3)> = players
        .iter()
        .map(|(e, t, m)| (e, t.translation(), t.translation() + m.eye_offset))
        .collect();
    for (e, state, mut animator, at, movement, (head, eyes), health) in &mut bodies {
        if health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let origin = at.translation();
        let eye = origin + movement.eye_offset;
        let ahead = forward(state.render_yaw);
        let target = all
            .iter()
            .filter(|(o, ..)| *o != e)
            .find(|(_, p, _)| {
                let d = *p - origin;
                d.length() * UNITS <= LOOK_RADIUS && ahead.dot(d) >= 0.0
            })
            .map_or(origin + ahead * (LOOK_AHEAD / UNITS), |(_, _, eye)| *eye);
        let (Some(mut head), Some(mut eyes)) = (head, eyes) else {
            commands.entity(e).insert((Head { feet: state.render_yaw, ..default() }, EyeTarget(target)));
            continue;
        };
        eyes.0 = target;
        turn_head(&mut head, eye, target, state.render_yaw, dt);
        let set = animator.set.clone();
        for (name, v) in [("head_yaw", head.yaw), ("head_pitch", head.pitch)] {
            if let Some(p) = set.param(name) {
                animator.set_param(p, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_and_forward_agree() {
        for yaw in [0.0f32, 30.0, 90.0, -120.0] {
            let (y, p) = angles(forward(yaw));
            assert!((normalize(y - yaw)).abs() < 1e-3 && p.abs() < 1e-3, "{yaw}: {y} {p}");
        }
        // Down is positive pitch.
        assert!((angles(Vec3::new(0.0, -1.0, -1.0)).1 - 45.0).abs() < 1e-3);
    }
}

/// `blink_duration` (s).
pub const BLINK_DURATION: f32 = 0.2;
/// Seconds between blinks: uniform in this range.
pub const BLINK_INTERVAL: (f32, f32) = (1.5, 4.0);

/// The blink controller's value `left` seconds before the blink's end
/// (spec 11.2).
pub fn blink_weight(left: f32) -> f32 {
    let u = left * std::f32::consts::FRAC_PI_2 / BLINK_DURATION;
    if u <= 0.0 {
        return 0.0;
    }
    let c = u.cos();
    if !(0.0 < c && c < 1.0) {
        return 0.0;
    }
    let b = 2.0 * c.sqrt();
    (if b > 1.0 { 2.0 - b } else { b }).clamp(0.0, 1.0)
}

/// A body's blinking: when the current blink ends and the next starts,
/// and its generator.
#[derive(Component, Clone, Copy, Debug)]
pub struct Blink {
    pub end: f64,
    pub next: f64,
    seed: u64,
}

impl Blink {
    fn roll(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        let u = (self.seed >> 40) as f32 / (1u64 << 24) as f32;
        BLINK_INTERVAL.0 + u * (BLINK_INTERVAL.1 - BLINK_INTERVAL.0)
    }
}

/// The face's descriptors at controller values `values` (the rules, then
/// the lids) and its mesh flexes' weights.
pub fn face_weights(face: &crate::map::MapFace, values: &[f32]) -> Vec<[f32; 2]> {
    let mut d = source_engine::content::face::run_rules(face, values);
    source_engine::content::face::eyelids(face, &mut d);
    source_engine::content::face::flex_weights(face, &d)
}

/// Each drawn HL2:DM body with a face: its blink and flex weights.
#[allow(clippy::type_complexity)]
pub fn faces(
    time: Res<Time>,
    models: Option<Res<crate::map::CharacterModels>>,
    mut bodies: Query<
        (Entity, &crate::map::BodyModel, Option<&mut Blink>, Option<&mut crate::map::FaceWeights>, Option<&Health>),
        With<Hl2PlayerAnim>,
    >,
    mut commands: Commands,
) {
    let Some(models) = models else { return };
    let now = time.elapsed_secs_f64();
    for (e, body, blink, weights, health) in &mut bodies {
        let Some(face) = models.0.get(body.0).and_then(|m| m.model.face.as_ref()) else { continue };
        if health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let Some(mut blink) = blink else {
            let mut b = Blink { end: 0.0, next: now, seed: e.to_bits() | 1 };
            b.next = now + b.roll() as f64;
            commands.entity(e).insert(b);
            continue;
        };
        if now >= blink.next {
            blink.end = now + BLINK_DURATION as f64;
            blink.next = now + blink.roll() as f64;
        }
        let values: Vec<f32> = face
            .controllers
            .iter()
            .map(|c| if c.name.eq_ignore_ascii_case("blink") { blink_weight((blink.end - now) as f32) } else { 0.0 })
            .collect();
        let w = face_weights(face, &values);
        match weights {
            Some(mut x) => {
                if x.0 != w {
                    x.0 = w;
                }
            }
            None => {
                commands.entity(e).insert(crate::map::FaceWeights(w));
            }
        }
    }
}
