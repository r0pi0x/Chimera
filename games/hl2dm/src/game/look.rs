//! Where HL2:DM's drawn bodies look (specs/hl2dm/player_models.md 6.4,
//! 11.3; clients only, never networked, never the server's hitboxes):
//! each frame every living body's look target is the eyes of the first
//! other player (dead ones and the local one included) within 128 units,
//! origin to origin, not behind its feet's facing; else a point 512 units
//! ahead. The head turns toward it (`head_yaw` within its ±44.8 degree
//! range, `head_pitch` within -20 to 23) at up to 130 degrees/s, the head
//! counter-turning while the feet turn; the eyes aim at it (`EyeTarget`,
//! drawn by `render::bodies` through `map::face`).
//!
//! Known shortcuts: players are taken in the query's order, not the
//! server's player index.

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
