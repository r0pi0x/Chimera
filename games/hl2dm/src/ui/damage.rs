//! The damage indicator (specs/hl2dm/hud.md 8): on each damage message
//! the HUD works out which side the hit came from and starts one of the
//! `HudTakeDamage*` events, whose colour variables (`DmgColorLeft`,
//! `DmgHighColorRight`, `DmgFullscreenColor`...) fade the red flashes
//! drawn along the screen's sides, or across it for a heavy hit, drowning
//! and the like. Death starts `HudPlayerDeath`'s long red fade.

use bevy::prelude::*;

use super::{
    frame::{Draw, Frame},
    hud::{Hl2Hud, Inputs},
    look::Look,
};

/// A hit of more health than this is "high".
pub const HIGH: f32 = 25.0;

/// The damage types the message keeps (the others are dropped).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Kinds {
    pub drown: bool,
    pub poison: bool,
    pub burn: bool,
    /// Radiation or acid.
    pub radiation: bool,
}

/// One damage message (4.7): health taken (summed over the tick's hits),
/// armour taken by the last hit, the filtered types and where the last
/// hit came from: its angle (`angle`: 360 ahead, 90 left, 180 behind, 270
/// right), None when it came from the world (a fall) or from straight
/// above or below.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DamageMsg {
    pub health: f32,
    pub armour: f32,
    pub kinds: Kinds,
    pub angle: Option<f32>,
    /// The inflictor is the world (origin 0 0 0, a fall).
    pub world: bool,
}

/// The angle of a hit: `d` the direction to the inflictor, `forward` and
/// `left` the horizontal view directions (unit, any right-handed frame).
/// 360 straight ahead, 90 to the left, 180 behind, 270 to the right; None
/// with no horizontal component (above or below).
pub fn damage_angle(d: Vec3, forward: Vec3, left: Vec3) -> Option<f32> {
    let (a, b) = (d.dot(forward), d.dot(left));
    if a.abs() < 1e-6 && b.abs() < 1e-6 {
        return None;
    }
    let theta = ((-b).atan2(-a) + std::f32::consts::PI).to_degrees();
    // Straight ahead is 360, not 0.
    Some(if theta < 1e-3 { 360.0 } else { theta })
}

/// The event a message starts (the first row that matches), None for no
/// effect.
pub fn damage_event(m: &DamageMsg) -> Option<&'static str> {
    if m.world && !m.kinds.drown {
        return None;
    }
    if m.health <= 0.0 && m.armour <= 0.0 {
        return None;
    }
    let high = m.health > HIGH;
    let theta = m.angle;
    // The bounds are inclusive (a little slack for float noise).
    let within = |lo: f32, hi: f32| theta.is_some_and(|t| t >= lo - 1e-3 && t <= hi + 1e-3);
    Some(if m.kinds.drown {
        "HudTakeDamageDrown"
    } else if m.kinds.poison {
        "HudTakeDamagePoison"
    } else if m.kinds.burn {
        "HudTakeDamageBurn"
    } else if m.kinds.radiation {
        "HudTakeDamageRadiation"
    } else if high && within(45.0, 135.0) {
        "HudTakeDamageHighLeft"
    } else if high && within(225.0, 315.0) {
        "HudTakeDamageHighRight"
    } else if high {
        "HudTakeDamageHigh"
    } else if within(45.0, 135.0) {
        "HudTakeDamageLeft"
    } else if within(225.0, 315.0) {
        "HudTakeDamageRight"
    } else if within(135.0, 225.0) {
        "HudTakeDamageBehind"
    } else {
        "HudTakeDamageFront"
    })
}

/// What the indicator remembers (nothing: its state is the variables).
#[derive(Clone, Debug, Default)]
pub struct Damage;

impl Hl2Hud {
    /// A damage message arrived; `health` is the local health now.
    pub fn damage_message(&mut self, m: &DamageMsg, health: f32) {
        if health <= 0.0 {
            self.fire("HudPlayerDeath");
            return;
        }
        if m.health > 0.0 {
            self.fire("HealthDamageTaken");
        }
        if let Some(e) = damage_event(m) {
            self.fire(e);
        }
    }

    pub(super) fn draw_damage(&self, look: &Look, inp: &Inputs, frame: &mut Frame) {
        const P: &str = "HudDamageIndicator";
        if inp.hide_health {
            return;
        }
        let c = |v: &str| self.anim.rgba(P, v).unwrap_or([0; 4]);
        let (left, right, high_left, high_right, full) =
            (c("DmgColorLeft"), c("DmgColorRight"), c("DmgHighColorLeft"), c("DmgHighColorRight"), c("DmgFullscreenColor"));
        if [left, right, high_left, high_right, full].iter().all(|c| c[3] == 0) {
            return;
        }
        let s = look.screen;
        let screen = Rect::new(0.0, 0.0, s.w, s.h);
        if full[3] > 0 {
            // Added to the screen (`vgui/white_additive`) when the look has
            // it: a red that grows to full leaves the scene showing.
            if look.hud.sprites.contains_key("white_additive") {
                frame.push(Draw::Sprite { sprite: "white_additive".into(), rect: screen, color: full, additive: true, flip: [false; 2] });
            } else {
                frame.push(Draw::Fade { rect: screen, color: full, from: 1.0, to: 1.0 });
            }
        }
        let high = high_left[3] > left[3] || high_right[3] > right[3];
        let half = (s.w / 2.0).floor();
        if high {
            // The high shape: from the edge to the centre line.
            if high_left[3] > 0 {
                frame.push(Draw::Fade { rect: Rect::new(0.0, 0.0, half, s.h), color: high_left, from: 1.0, to: 0.0 });
            }
            if high_right[3] > 0 {
                frame.push(Draw::Fade { rect: Rect::new(half, 0.0, s.w, s.h), color: high_right, from: 0.0, to: 1.0 });
            }
        } else {
            // The normal shape: between x1 and x2, its outer edge faint
            // and its inner edge full (the trapezoid's slanted ends are
            // a straight band here).
            let (x1, wide) = (look.key(P, "dmg_xpos"), look.key(P, "dmg_wide"));
            let (y, t1, t2) = (look.key(P, "dmg_ypos"), look.key(P, "dmg_tall1"), look.key(P, "dmg_tall2"));
            let (top, bottom) = (s.scaled(y + (t1 - t2) / 4.0), s.scaled(y + t1 - (t1 - t2) / 4.0));
            let (xa, xb) = (s.scaled(x1), s.scaled(x1 + wide));
            if left[3] > 0 {
                frame.push(Draw::Fade { rect: Rect::new(xa, top, xb, bottom), color: left, from: 0.0, to: 1.0 });
            }
            if right[3] > 0 {
                frame.push(Draw::Fade { rect: Rect::new(s.w - xb, top, s.w - xa, bottom), color: right, from: 1.0, to: 0.0 });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Source's frame: x ahead, y left (yaw 0), the eye at the origin.
    fn angle(at: Vec3) -> Option<f32> {
        damage_angle(at.normalize(), Vec3::X, Vec3::Y)
    }

    fn event(at: Vec3, health: f32) -> Option<&'static str> {
        damage_event(&DamageMsg { health, armour: 0.0, angle: angle(at), ..Default::default() })
    }

    /// Spec test 45: the angle and the event of five hits.
    #[test]
    fn the_hit_decides_the_side() {
        assert_eq!(angle(Vec3::new(0.0, 100.0, 0.0)), Some(90.0));
        assert_eq!(angle(Vec3::new(-100.0, 0.0, 0.0)), Some(180.0));
        assert!((angle(Vec3::new(100.0, 100.0, 0.0)).unwrap() - 45.0).abs() < 1e-3);
        assert!((angle(Vec3::new(100.0, -100.0, 0.0)).unwrap() - 315.0).abs() < 1e-3);
        assert_eq!(angle(Vec3::new(100.0, 0.0, 0.0)), Some(360.0));
        assert_eq!(event(Vec3::new(0.0, 100.0, 0.0), 10.0), Some("HudTakeDamageLeft"));
        assert_eq!(event(Vec3::new(-100.0, 0.0, 0.0), 10.0), Some("HudTakeDamageBehind"));
        assert_eq!(event(Vec3::new(100.0, 100.0, 0.0), 10.0), Some("HudTakeDamageLeft"));
        assert_eq!(event(Vec3::new(100.0, -100.0, 0.0), 10.0), Some("HudTakeDamageRight"));
        assert_eq!(event(Vec3::new(100.0, 0.0, 0.0), 10.0), Some("HudTakeDamageFront"));
    }

    /// Spec tests 46, 47 and 49: heavy hits, and a fall's nothing.
    #[test]
    fn heavy_hits_and_falls() {
        assert_eq!(event(Vec3::new(0.0, 100.0, 0.0), 30.0), Some("HudTakeDamageHighLeft"));
        assert_eq!(event(Vec3::new(100.0, 0.0, 0.0), 30.0), Some("HudTakeDamageHigh"));
        assert_eq!(damage_event(&DamageMsg { health: 10.0, world: true, ..Default::default() }), None);
        // Drowning is a world hit that still flashes.
        assert_eq!(
            damage_event(&DamageMsg { health: 2.0, world: true, kinds: Kinds { drown: true, ..Default::default() }, ..Default::default() }),
            Some("HudTakeDamageDrown")
        );
        // From above: no side.
        assert_eq!(angle(Vec3::Z), None);
        // ... and starts the empty front event: no flash.
        assert_eq!(event(Vec3::Z, 10.0), Some("HudTakeDamageFront"));
    }
}
