//! The shield's hit bookkeeping of one local player (specs/halo_ce/hud.md
//! 5): L (the shield last seen), f (a fade time or -1) and t (a tick).
//! The HUD's hit mark and the warning sounds' "the shield fell" bit
//! (`audio::warnings`) both read it; it lives here, in the game's
//! layer, so the sounds need no HUD.
//!
//! While an overshield drains (above 1.0) the game lowers L with it (hud.md
//! 5), so a drain never marks a hit nor plays `shield_hit`: a fall above
//! 1.0 of at most `DRAIN_ALLOWANCE` (a few ticks' drain, as a network
//! client may see several at once) moves L along.

/// Halo ticks a second.
const TICKS: f32 = 30.0;
/// Ticks the lost stretch of the bar stays marked after the first loss.
pub const SHIELD_HIT_HOLD: i64 = 15;
/// The largest fall above 1.0 taken for the overshield's drain: four
/// ticks of `data/vitality.toml`'s `overshield_drain` (1/1350 a tick; a
/// test checks the two agree).
pub const DRAIN_ALLOWANCE: f32 = 4.0 / 1350.0;

/// The bookkeeping itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShieldMark {
    pub last: f32,
    pub fade: f32,
    pub tick: i64,
}

impl Default for ShieldMark {
    fn default() -> Self {
        ShieldMark { last: -1.0, fade: -1.0, tick: 0 }
    }
}

impl ShieldMark {
    /// One tick with the shield at `s` on tick `now`.
    pub fn update(&mut self, s: f32, now: i64) {
        if self.last < 0.0 {
            *self = ShieldMark { last: s, fade: -1.0, tick: now };
            return;
        }
        if self.last > 1.0 && s >= 1.0 - 1e-6 && self.last - s <= DRAIN_ALLOWANCE + 1e-6 {
            // The overshield's drain: L follows it (rule 3).
            self.last = s;
            if self.fade > 0.0 {
                self.fade += (now - self.tick) as f32 / TICKS;
            }
            self.tick = now;
            return;
        }
        if self.last > s {
            if self.fade < 0.0 || self.fade > 1.0 {
                self.tick = now;
            }
            if now - self.tick < SHIELD_HIT_HOLD {
                self.fade = 0.0;
            } else {
                self.last = s;
                self.fade += (now - self.tick) as f32 / TICKS;
                self.tick = now;
            }
        } else if self.last < s {
            *self = ShieldMark { last: s, fade: -1.0, tick: now };
        } else {
            if self.fade > 0.0 {
                self.fade += (now - self.tick) as f32 / TICKS;
            }
            self.tick = now;
        }
    }
}
