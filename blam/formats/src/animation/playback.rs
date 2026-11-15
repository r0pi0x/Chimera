//! Stepping an animation once a tick (specs/halo_ce/animations.md,
//! "Playback") and the first-person view's smoothing step ("Smoothed
//! inputs"): `Playback` holds the current frame and `advance` moves it by
//! one tick and reports what happened (looped, restarted, ends next tick,
//! key frame, running) in the spec's order of precedence;
//! `Playback::sound_starts` is the sound rule; `smooth_step` is the shared
//! position and velocity step. Pure functions of the animation's numbers.

use super::Animation;

/// What one `Playback::advance` reported. Exclusive, checked in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayEvent {
    /// The frame reached the end and went back to the loop frame.
    Looped,
    /// A play-once animation reached the end: the frame is 0 again and the
    /// owner chooses the next permutation of the slot.
    Restarted,
    /// The frame is now the last one of a play-once animation.
    EndsNext,
    /// The frame is the key frame or the second key frame (never 0, and
    /// never on the last frame of a play-once animation).
    KeyFrame,
    Running,
}

/// A playing animation's state: the current frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Playback {
    pub frame: usize,
}

impl Playback {
    /// Starting an animation sets the frame to 0.
    pub fn start() -> Playback {
        Playback { frame: 0 }
    }

    /// Whether the animation's sound starts now: it has one and the
    /// current frame is its sound frame. Call it before `advance` (so a
    /// sound at frame 0 starts one tick after the animation began).
    pub fn sound_starts(&self, animation: &Animation) -> bool {
        animation.sound.is_some() && self.frame as i64 == i64::from(animation.sound_frame)
    }

    /// One tick: `f = f + 1`; at `f >= N` loop to `min(L, N - 1)` when the
    /// loop frame `L` is positive (a loop frame at or past the end holds
    /// the last frame for ever) or restart at 0; else report "ends next
    /// tick" at `f + 1 = N` of a play-once animation, a key frame, or
    /// running.
    pub fn advance(&mut self, animation: &Animation) -> PlayEvent {
        let n = animation.frame_count.max(0) as usize;
        let l = animation.loop_frame;
        self.frame += 1;
        if self.frame >= n {
            if l > 0 {
                self.frame = (l as usize).min(n.saturating_sub(1));
                return PlayEvent::Looped;
            }
            self.frame = 0;
            return PlayEvent::Restarted;
        }
        if self.frame + 1 == n && l == 0 {
            return PlayEvent::EndsNext;
        }
        let f = self.frame as i64;
        if f == i64::from(animation.key_frame) || f == i64::from(animation.second_key_frame) {
            return PlayEvent::KeyFrame;
        }
        PlayEvent::Running
    }
}

/// One smoothing step toward `target` of the first-person view's inputs
/// (position, turning, push back): position `p`, velocity `v`, maximum
/// change `a` per tick, maximum speed `m`, bounds `lo..=hi`. Returns the
/// new `(p, v)`. Let `d = target - p`; if `|d - v| <= min(a, m)` the
/// velocity is 0 and the position the (clamped) target; otherwise the
/// wanted speed `u` is `m` if `2 a |d| >= m^2` else `sqrt(2 a |d|)`, with
/// the sign of `d`, `e = u - v` clamped to `-a..=a`, `v += e` (not
/// clamped) and `p += v + e / 2` (clamped).
pub fn smooth_step(p: f32, v: f32, target: f32, a: f32, m: f32, lo: f32, hi: f32) -> (f32, f32) {
    let d = target - p;
    if (d - v).abs() <= a.min(m) {
        return (target.clamp(lo, hi), 0.0);
    }
    let mut u = if 2.0 * a * d.abs() >= m * m {
        m
    } else {
        (2.0 * a * d.abs()).sqrt()
    };
    if d < 0.0 {
        u = -u;
    }
    let e = (u - v).clamp(-a, a);
    let v = v + e;
    ((p + v + e / 2.0).clamp(lo, hi), v)
}
