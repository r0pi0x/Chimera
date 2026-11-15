//! The spec's arithmetic of one damage event on one Spartan, and of its
//! tick (specs/halo_ce/health_shields.md "Applying a damage event",
//! "Shield recharge", "The overshield", "Active camouflage", "Stun and
//! pings"), on `Vitality` and `Stun` alone, with no ECS: every quantity an
//! f32 and every operation in the spec's order (the pistol's three-shot
//! kill depends on it, "Quirks"). `super::super::damage` calls `take` for
//! each `HaloHit` on a Halo character; `super` calls `tick` once a tick.
//!
//! Known shortcuts:
//! - Effects the hit would play (shield depleted, body damaged, area
//!   blood) and region damage counters are not kept: the Spartan's model
//!   plays none that changes play, and its one region is never destroyed.
//! - Failing shields are computed (no multiplayer object has them).

use super::{
    Stun, Vitality,
    data::{DamageEffect, SideEffect, Spartan, flags, ticks},
};

/// One damage event as `take` needs it (the target's side of the seam).
#[derive(Clone, Copy, Debug)]
pub struct Event<'a> {
    pub effect: &'a DamageEffect,
    /// A, points: the weapons' side computed it (falloff, the random draw).
    pub amount: f32,
    /// The event scale s (stun scales by it).
    pub scale: f32,
    /// The Spartan's material hit, by index (None: the indirect material:
    /// area hits, melee, falls).
    pub material: Option<usize>,
    /// The hit is friendly (self damage, or a teammate's in a team game).
    pub friendly: bool,
    /// The target is unsuspecting (`unsuspecting`: a melee from behind).
    pub unsuspecting: bool,
    /// The event bypasses shields (riders, scripts).
    pub bypass_shields: bool,
    /// The game multiplier: 1 / clamp(health option, 0.25, 4), times the
    /// traits' (1 by default).
    pub game_multiplier: f32,
}

/// What one event did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outcome {
    /// T > 0: the event did anything at all.
    pub landed: bool,
    /// D_s: the amount the shield took (before its multipliers).
    pub shield_taken: f32,
    /// a: the body's amount, points (k_b x what the shield let through).
    pub body_amount: f32,
    /// x: the body fraction lost (after the modifiers and a headshot's
    /// doubling).
    pub body_fraction: f32,
    /// This hit broke the shield.
    pub depleted: bool,
    /// A "can cause headshots" kill.
    pub headshot: bool,
    /// An instant kill (lethal to the unsuspecting).
    pub instant_kill: bool,
    /// The hard flinch is forced (shield broken, headshot, x > body).
    pub hard_forced: bool,
    /// The hit killed the target (body below 0, or an instant kill).
    pub died: bool,
}

/// The target is unsuspecting of an attacker at `epicenter` (melee:
/// the attacker's centre): `(c_t - c_a) . l > 0`, `l` the target's 3D
/// looking direction ("Melee", "From behind").
pub fn unsuspecting(target_centre: bevy::math::Vec3, epicenter: bevy::math::Vec3, look: bevy::math::Vec3) -> bool {
    (target_centre - epicenter).dot(look) > 0.0
}

/// The melee's scale ("Melee"): `clamp(v_f x 30 / run forward speed, 0,
/// 1)` for the attacker's velocity along its facing `forward` (WU/tick),
/// or the lunge's 1.5 after more than 15 airborne ticks.
pub fn melee_scale(spartan: &Spartan, forward: f32, airborne_ticks: u32) -> f32 {
    if airborne_ticks > spartan.constants.melee_airborne_ticks {
        spartan.constants.melee_lunge_scale
    } else {
        (forward * 30.0 / spartan.globals.run_forward_speed).clamp(0.0, 1.0)
    }
}

impl Vitality {
    /// Apply one event (the spec's steps 1 and 3 to 8; the chain of step 2
    /// is the player alone). The caller handles the aftermath (step 9).
    pub fn take(&mut self, spartan: &Spartan, e: &Event) -> Outcome {
        let model = &spartan.model;
        let c = &spartan.constants;
        let fx = e.effect;
        let mut out = Outcome::default();
        // 1. The amount.
        let mut t = e.amount * e.game_multiplier;
        if t <= 0.0 {
            return out;
        }
        out.landed = true;
        // 4. Instant kills.
        if fx.side_effect == SideEffect::LethalToTheUnsuspecting && e.unsuspecting && !self.dead {
            self.body = 0.0;
            self.dead = true;
            out.instant_kill = true;
            out.died = true;
        }
        // 5. Material.
        let index = e.material.unwrap_or(model.indirect_material);
        let m = model.materials.get(index).or(model.materials.get(model.indirect_material));
        let (head, leak, k_s, k_b, kind) = m.map_or((false, 0.0, 1.0, 1.0, 21), |m| (m.head, m.leak, m.shield, m.body, m.kind));
        let mu_s = fx.modifier(model.shield_material_type);
        let mu_b = fx.modifier(kind);
        let s_max = model.maximum_shield;
        // 6. Shield.
        let skips = fx.has(flags::SKIPS_SHIELDS) || e.bypass_shields || s_max <= 0.0;
        let shield_ran = !skips && self.shield > 0.0;
        if shield_ran {
            let t_in = t;
            let friendly = e.friendly && model.always_shields_friendly();
            let mut d_s = if friendly { t } else { (1.0 - leak) * t };
            let threshold = model.shield_failure_threshold;
            if !friendly && threshold > 0.0 && self.shield <= threshold {
                // Linear transition (the Spartan has none: no multiplayer
                // object fails).
                let l = model.failing_shield_leak;
                d_s *= (1.0 - l) * (self.shield / threshold) + l;
            }
            let mut p = 0.0;
            if self.overcharging {
                d_s = t;
                t = 0.0;
            } else {
                d_s = d_s.max(0.0);
                t -= d_s;
                p = (k_s * d_s) * mu_s;
                let delta = p * (1.0 / s_max);
                if delta > self.shield || fx.side_effect == SideEffect::Emp {
                    let excess = p - s_max * self.shield;
                    if excess > 0.0 {
                        t += excess;
                    }
                    self.shield = 0.0;
                    if !self.depleted {
                        self.depleted = true;
                        self.current_shield_damage = 0.0;
                        out.depleted = true;
                    }
                } else {
                    self.shield -= delta;
                }
            }
            if self.overcharging || p >= c.negligible {
                self.shield_fade = Some(0);
                if !self.depleted {
                    self.current_shield_damage = 1.0;
                }
                self.recent_shield_damage = (self.recent_shield_damage + (t_in - t) / s_max).min(1.0);
            }
            out.shield_taken = d_s;
        }
        // 7. Shield delay.
        if (shield_ran && out.shield_taken >= model.minimum_stun_damage) || self.shield == 0.0 {
            self.delay = spartan.shield_delay();
        }
        // 8. Body.
        if !fx.has(flags::ONLY_HURTS_SHIELDS) {
            let a = k_b * t;
            let mut x = a;
            if e.friendly {
                x = (1.0 - model.friendly_damage_resistance) * a;
            }
            x *= 1.0 / model.maximum_body;
            x *= mu_b;
            if a > 0.0 && head {
                if fx.has(flags::CAN_CAUSE_HEADSHOTS) {
                    self.body = 0.0;
                    out.headshot = true;
                    out.hard_forced = true;
                } else if fx.has(flags::CAN_CAUSE_MULTIPLAYER_HEADSHOTS) {
                    x *= 2.0;
                    if x > self.body {
                        out.hard_forced = true;
                    }
                }
            }
            self.body -= x;
            self.body_fade = Some(0);
            self.current_body_damage = (self.current_body_damage + x).min(1.0);
            self.recent_body_damage = (self.recent_body_damage + x).min(1.0);
            if self.body * model.maximum_body < 0.0 && !self.dead {
                self.dead = true;
                out.died = true;
            }
            out.body_amount = a;
            out.body_fraction = x;
        }
        out.hard_forced |= out.depleted;
        // Camouflage loses the effect's camouflage damage on every hit.
        if self.camouflaged {
            self.camo = (self.camo - fx.camo).max(0.0);
        }
        out
    }

    /// The tick's damage update ("Shield recharge", "Damage fades") and
    /// the unit's camouflage step and powerup timer ("Active
    /// camouflage"), in the spec's order for a living Spartan.
    pub fn tick(&mut self, spartan: &Spartan) {
        let c = &spartan.constants;
        // The unit update: camouflage.
        if self.camouflaged {
            let rate = match self.camo_regrowth {
                Some(r) if r != 0.0 => r,
                _ => c.camo_step,
            };
            self.camo = (self.camo + rate).min(1.0);
            if self.camo >= 1.0 {
                self.camo_regrowth = None;
            }
        } else {
            self.camo = (self.camo - c.camo_step).max(0.0);
        }
        // The damage update: the shield.
        self.charging = false;
        if !self.dead && spartan.model.maximum_shield > 0.0 {
            if self.overcharging {
                self.shield += c.overcharge_step;
                if self.shield >= c.overshield_max {
                    self.shield = c.overshield_max;
                    self.overcharging = false;
                }
                self.charging = self.shield <= 1.0;
            } else if self.shield > 1.0 {
                if self.shield - 1.0 < c.overshield_drain {
                    self.shield = 1.0;
                } else {
                    self.shield -= c.overshield_drain;
                }
            } else if self.shield < 1.0 {
                if self.delay == 0 {
                    if self.depleted {
                        self.depleted = false;
                    }
                    self.shield += spartan.model.recharge_velocity;
                    if self.shield > 1.0 {
                        self.shield = 1.0;
                    }
                    self.charging = self.shield <= 1.0;
                } else {
                    self.delay -= 1;
                }
            }
        }
        // The fades.
        fade(&mut self.shield_fade, &mut self.current_shield_damage, &mut self.recent_shield_damage, c.damage_fade, c.recent_hold);
        fade(&mut self.body_fade, &mut self.current_body_damage, &mut self.recent_body_damage, c.damage_fade, c.recent_hold);
        // The players' update: the camouflage timer.
        if self.camo_ticks > 0 {
            self.camo_ticks -= 1;
            if self.camo_ticks == 0 {
                self.camouflaged = false;
            }
        }
    }

    /// A health pack ("Health"): if alive and hurt, the body to full at
    /// once; false (not picked up) otherwise.
    pub fn heal(&mut self) -> bool {
        if self.dead || self.body >= 1.0 {
            return false;
        }
        self.body = 1.0;
        true
    }

    /// An overshield ("The overshield"): only if the shield is at most 1;
    /// the charge-up starts from the next update.
    pub fn overshield(&mut self, spartan: &Spartan) -> bool {
        if self.dead || self.shield > 1.0 {
            return false;
        }
        self.overcharging = true;
        if self.shield == 0.0 {
            self.shield = spartan.constants.overshield_floor;
        }
        self.delay = 0;
        true
    }

    /// Active camouflage ("Active camouflage"): refused while camouflaged;
    /// the timer is the powerup's time in ticks (1350).
    pub fn camouflage(&mut self, spartan: &Spartan) -> bool {
        if self.dead || self.camouflaged {
            return false;
        }
        self.camouflaged = true;
        self.camo_ticks = ticks(spartan.powerups.active_camouflage.time);
        true
    }

    /// The holder fired (multiplayer): with the weapon's "active camo
    /// ding" (0: the default 0.1; None: it "does not depower active
    /// camo") and "active camo regrowth rate" (per tick).
    pub fn fired(&mut self, spartan: &Spartan, ding: Option<f32>, regrowth: f32) {
        let c = &spartan.constants;
        let Some(ding) = ding else { return };
        if self.camo >= c.camo_fire_floor {
            let ding = if ding == 0.0 { c.camo_fire_default } else { ding };
            self.camo = (self.camo - ding).max(c.camo_fire_floor);
            self.camo_regrowth = Some(regrowth);
        }
    }

    /// Death: no more shield updates; camouflage lost.
    pub fn die(&mut self) {
        self.dead = true;
        self.camouflaged = false;
        self.camo_ticks = 0;
        self.overcharging = false;
    }
}

fn fade(timer: &mut Option<u32>, current: &mut f32, recent: &mut f32, step: f32, hold: u32) {
    let Some(t) = timer.as_mut() else { return };
    *t += 1;
    *current = (*current - step).max(0.0);
    if *t >= hold {
        *recent = (*recent - step).max(0.0);
    }
    if *current == 0.0 && *recent == 0.0 {
        *timer = None;
    }
}

impl Stun {
    /// A hit of `effect` at scale `s` on a player's unit ("Stun and
    /// pings").
    pub fn hit(&mut self, spartan: &Spartan, effect: &DamageEffect, s: f32) {
        let [stun, maximum, time] = effect.stun;
        if stun <= 0.0 {
            return;
        }
        let i = (stun * s).max(0.0);
        let cap = (maximum * s).clamp(0.0, 1.0);
        if self.value < cap {
            self.value = (self.value + i).min(cap);
        }
        self.ticks = self.ticks.max(ticks(spartan.globals.minimum_stun_time)) + ticks(time);
        self.ticks = self.ticks.min(ticks(spartan.globals.maximum_stun_time));
    }

    /// The unit update's step: the counter down, the stun to 0 with it.
    pub fn step(&mut self) {
        if self.ticks > 0 {
            self.ticks -= 1;
            if self.ticks == 0 {
                self.value = 0.0;
            }
        }
    }

    /// What movement multiplies the wanted speed by (movement.md: the
    /// stun movement penalty 0.8).
    pub fn speed_scale(&self, spartan: &Spartan) -> f32 {
        1.0 - spartan.globals.stun_movement_penalty * self.value
    }
}
