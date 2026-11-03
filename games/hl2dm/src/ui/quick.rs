//! The crosshair, the quick info brackets and the suit zoom overlay
//! (specs/hl2dm/hud.md 6). The brackets beside the crosshair show health
//! (left) and the clip (right) as a two-part bar, blink in `Caution` when
//! health or the clip runs low, and dim when nothing changes; the
//! crosshair is the weapon's own icon at a field of view of 90 or more,
//! else the white default sprite (a quirk kept: the default FOV is 75);
//! the zoom overlay is two rings, tick marks and four darkened corners.

use bevy::prelude::*;

use super::{
    frame::{Align, Draw, Frame, fade},
    hud::{Hl2Hud, Inputs},
    look::Look,
    numeric::static_font,
};
use crate::game::hud::icon_key;

/// Health at or below this warns.
pub const HEALTH_WARN: i32 = 25;
/// The clip, as a share of its size, at or below this warns (sizes over 1).
pub const CLIP_WARN: f32 = 0.25;
/// Seconds without a change before the brackets dim, and the dimming's.
pub const IDLE: f64 = 1.0;
pub const DIM_ALPHA: f32 = 64.0;
pub const DIM_TIME: f64 = 2.0;
pub const FULL_TIME: f64 = 0.5;
pub const ZOOM_FADE_IN: f64 = 0.25;
/// The sounds a warning plays.
pub const LOW_HEALTH_SOUND: &str = "HUDQuickInfo.LowHealth";
pub const LOW_AMMO_SOUND: &str = "HUDQuickInfo.LowAmmo";

/// A warning's blink alpha for a blink phase `k` (the low byte of 255 k:
/// a quirk kept).
pub fn blink_alpha(k: u32) -> u8 {
    (255 * k % 256) as u8
}

/// `k` at client time `t`: |sin 8t| times 128, whole.
pub fn blink_k(t: f64) -> u32 {
    ((8.0 * t).sin().abs() * 128.0).floor() as u32
}

/// The quick info's state.
#[derive(Clone, Debug)]
pub struct Quick {
    health: i32,
    clip: i32,
    pub warn_health: bool,
    pub warn_ammo: bool,
    /// The blink's budget left (255 at the start) and its last frame time.
    pub blink: f32,
    last_frame: f64,
    last_change: f64,
    /// The panel's alpha (0..255) and the ramp it is on.
    pub alpha: f32,
    ramp: (f32, f32, f64, f64),
    /// Sounds to play (taken by the Bevy side).
    pub sounds: Vec<&'static str>,
}

impl Default for Quick {
    fn default() -> Self {
        Quick { health: 100, clip: 0, warn_health: false, warn_ammo: false, blink: 0.0, last_frame: 0.0, last_change: 0.0, alpha: 255.0, ramp: (255.0, 255.0, 0.0, 0.0), sounds: Vec::new() }
    }
}

impl Quick {
    /// A spawn compares with 100 health and no ammo again.
    pub fn reset(&mut self) {
        *self = Quick::default();
    }

    fn ramp_to(&mut self, to: f32, len: f64, now: f64) {
        self.ramp = (self.alpha, to, now, len);
    }

    fn step(&mut self, now: f64) {
        let (from, to, start, len) = self.ramp;
        let k = if len <= 0.0 { 1.0 } else { ((now - start) / len).clamp(0.0, 1.0) as f32 };
        self.alpha = from + (to - from) * k;
    }
}

/// The suit zoom overlay's state.
#[derive(Clone, Debug, Default)]
pub struct ZoomView {
    pub on: bool,
    pub since: f64,
}

impl Hl2Hud {
    pub(super) fn think_quick(&mut self, inp: &Inputs) {
        let now = self.now;
        let q = &mut self.quick;
        let dt = (now - q.last_frame).max(0.0) as f32;
        q.last_frame = now;
        let weapon = inp.weapon.as_ref();
        let health = inp.health.max(0.0).ceil() as i32;
        let mut changed = false;
        if health != q.health {
            changed = true;
            if health <= HEALTH_WARN {
                if !q.warn_health {
                    q.warn_health = true;
                    q.blink = 255.0;
                    q.sounds.push(LOW_HEALTH_SOUND);
                }
            } else {
                q.warn_health = false;
            }
            q.health = health;
        }
        if let Some(w) = weapon {
            let clip = w.clip.unwrap_or(0);
            if clip != q.clip {
                changed = true;
                if w.max_clip > 1 && clip as f32 / w.max_clip as f32 <= CLIP_WARN {
                    if !q.warn_ammo {
                        q.warn_ammo = true;
                        q.blink = 255.0;
                        q.sounds.push(LOW_AMMO_SOUND);
                    }
                } else {
                    q.warn_ammo = false;
                }
                q.clip = clip;
            }
        }
        // A blink spends its budget; the budget's last frame ends on a
        // quiet phase of the sine.
        if q.blink > 0.0 {
            let loss = 200.0 * dt;
            q.blink -= loss;
            if q.blink <= loss && blink_k(now) >= 40 {
                q.blink = loss.max(0.001);
            } else if q.blink <= 0.0 {
                q.blink = 0.0;
            }
        }
        if changed {
            q.last_change = now;
            q.ramp_to(255.0, FULL_TIME, now);
        } else if !self.zoom.on && now - q.last_change > IDLE && q.ramp.1 != DIM_ALPHA && q.ramp.1 != 0.0 {
            q.ramp_to(DIM_ALPHA, DIM_TIME, now);
        }
        q.step(now);
    }

    pub(super) fn think_zoom(&mut self, inp: &Inputs) {
        if inp.zoomed != self.zoom.on {
            self.zoom.on = inp.zoomed;
            self.zoom.since = self.now;
            let now = self.now;
            if inp.zoomed {
                self.quick.ramp_to(0.0, ZOOM_FADE_IN, now);
            } else {
                self.quick.last_change = now;
                self.quick.ramp_to(255.0, FULL_TIME, now);
            }
        }
    }

    pub(super) fn draw_crosshair(&self, look: &Look, inp: &Inputs, frame: &mut Frame) {
        if !inp.crosshair || inp.hide_crosshair || !inp.alive {
            return;
        }
        let s = look.screen;
        let (cx, cy) = ((s.w / 2.0).round(), (s.h / 2.0).round());
        let white = [255, 255, 255, 255];
        let class = inp.weapon.as_ref().map(|w| w.class.as_str());
        // The icon: the weapon's own at FOV 90 or more, else its `zoom`
        // icon in white, else the default sprite in white.
        let (key, colour) = match class {
            Some(c) if inp.fov >= 90.0 => (Some(icon_key(c, "crosshair")), look.colour("Normal")),
            Some(c) => (Some(icon_key(c, "zoom")), white),
            None => (None, white),
        };
        if let Some(key) = &key
            && let Some((font, ch)) = look.glyph(key)
        {
            let f = static_font(&font);
            let (w, h) = (look.width(f, &ch.to_string()), look.tall(f));
            frame.push(Draw::text(f, h, ch.to_string(), colour, Vec2::new(cx - (w / 2.0).floor(), cy - (h / 2.0).floor()), Align::Left));
            return;
        }
        let sprite = key.filter(|k| look.hud.sprites.contains_key(k)).unwrap_or_else(|| "crosshair_default".into());
        if let Some(sp) = look.hud.sprites.get(&sprite) {
            // Sprites keep their pixel size at every resolution.
            let (w, h) = (sp.rect[2], sp.rect[3]);
            let min = Vec2::new(cx - (w / 2.0).floor(), cy - (h / 2.0).floor());
            frame.push(Draw::Sprite { sprite, rect: Rect::from_corners(min, min + Vec2::new(w, h)), color: white, additive: true, flip: [false; 2] });
        }
    }

    pub(super) fn draw_quick(&self, look: &Look, inp: &Inputs, frame: &mut Frame) {
        if !inp.crosshair || inp.hide_crosshair || !inp.alive || inp.weapon.is_none() {
            return;
        }
        let s = look.screen;
        let (cx, cy) = (s.w / 2.0, s.h / 2.0);
        let panel = self.quick.alpha / 255.0;
        let normal = look.colour("Normal");
        let h_b = look.tall("QuickInfo");
        // The centre sprite, drawn even with `hud_quickinfo` 0.
        if let Some(sp) = look.hud.sprites.get("crosshair") {
            let min = Vec2::new(cx, cy - (h_b / 2.0).floor());
            frame.push(Draw::Sprite {
                sprite: "crosshair".into(),
                rect: Rect::from_corners(min, min + Vec2::new(sp.rect[2], sp.rect[3])),
                color: fade(normal, 138.0 / 255.0 * panel),
                additive: true,
                flip: [false; 2],
            });
        }
        if !inp.quickinfo {
            return;
        }
        let w = inp.weapon.as_ref().expect("checked");
        let health_p = 1.0 - (inp.health / 100.0).clamp(0.0, 1.0);
        let clip_p = match w.clip {
            Some(c) if w.max_clip > 0 => 1.0 - (c as f32 / w.max_clip as f32).clamp(0.0, 1.0),
            _ => 0.0,
        };
        let (wl, wr) = (look.width("QuickInfo", "("), look.width("QuickInfo", ")"));
        let y = (cy - (h_b / 2.0).floor()).floor();
        let t = self.now;
        let (k, blinking) = (blink_k(t), self.quick.blink > 0.0);
        let caution = look.colour("Caution");
        for (x, p, bracket, full, empty, warn) in [
            (cx - 2.0 * wl, health_p, "(", "[", "{", self.quick.warn_health),
            (cx + wr, clip_p, ")", "]", "}", self.quick.warn_ammo),
        ] {
            let at = Vec2::new(x, y);
            let glyph_w = look.width("QuickInfo", full).max(look.width("QuickInfo", bracket));
            let split = y + (h_b * p).round();
            let top = Rect::from_corners(at - Vec2::new(0.0, 0.0), Vec2::new(x + glyph_w.max(1.0) + 4.0, split));
            let bottom = Rect::from_corners(Vec2::new(x, split), Vec2::new(x + glyph_w.max(1.0) + 4.0, y + h_b));
            let alpha_blink = fade(caution, blink_alpha(k) as f32 / 255.0 * panel);
            if warn && blinking {
                // The whole "full" glyph in Caution.
                frame.push(Draw::text("QuickInfo", h_b, full, alpha_blink, at, Align::Left));
                continue;
            }
            let (part_full, part_empty) = if warn { (alpha_blink, alpha_blink) } else { (fade(normal, 138.0 / 255.0 * panel), fade(normal, 138.0 / 255.0 * panel)) };
            if p > 0.0 {
                frame.push(Draw::text("QuickInfo", h_b, empty, part_empty, at, Align::Left).clipped(top));
            }
            if p < 1.0 {
                frame.push(Draw::text("QuickInfo", h_b, full, part_full, at, Align::Left).clipped(bottom));
            }
        }
    }

    pub(super) fn draw_zoom(&self, look: &Look, _inp: &Inputs, frame: &mut Frame) {
        let z = &self.zoom;
        let s = look.screen;
        let t = (self.now - z.since).max(0.0);
        let k = (t / 0.4).clamp(0.0, 1.0) as f32;
        if !z.on && (k >= 1.0 || z.since == 0.0) {
            return;
        }
        let (alpha, r) = if z.on { (k, k) } else { ((1.0 - k) / 4.0, 1.0 - k / 2.0) };
        let u = s.u();
        let (cx, cy) = ((s.w / 2.0).round(), (s.h / 2.0).round());
        let colour = fade(look.colour("ZoomReticleColor"), 64.0 / 255.0 * alpha);
        for radius in [66.0, 74.0] {
            frame.push(Draw::Ring { centre: Vec2::new(cx, cy), radius: radius * u * r, color: colour });
        }
        let g = 16.0 * u * r.max(0.1);
        let dash = s.scaled(look.key("HudZoom", "DashHeight"));
        let limit = (cx.max(s.w - cx) / g).floor() as i32;
        let mut k_i = 2;
        while k_i < limit {
            for sign in [-1.0, 1.0] {
                let x = (cx + sign * k_i as f32 * g).round();
                let min = Vec2::new(x, cy - (dash / 2.0).floor());
                frame.push(Draw::Box { rect: Rect::from_corners(min, min + Vec2::new(1.0, dash)), color: colour, round: false });
            }
            k_i += 1;
        }
        // The four quarters of `vgui/zoom`, mirrored so its corner sits at
        // the screen's corners, in black at the overlay's alpha.
        let shade = [0, 0, 0, (alpha * 255.0).round() as u8];
        for (qx, qy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let min = Vec2::new(qx * cx, qy * cy);
            let rect = Rect::from_corners(min, min + Vec2::new(cx, cy));
            frame.push(Draw::Sprite { sprite: "zoom".into(), rect, color: shade, additive: false, flip: [qx > 0.0, qy > 0.0] });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec test 36: k of 0, 1, 64 and 128 gives alphas 0, 255, 192 and 128.
    #[test]
    fn the_blink_alpha_is_the_low_byte() {
        assert_eq!([0, 1, 64, 128].map(blink_alpha), [0, 255, 192, 128]);
    }
}
