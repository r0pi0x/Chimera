//! DoD:S's prone, its deployed machine guns and the tick's speed cap
//! (specs/dods/movement.md, "Prone", "Getting up", "Stamina and sprint",
//! "View"; damage.md, "Being slowed when hit"; weapons.md, "Alt fire"), a
//! DoD:S player's `DodsStance`, stepped each tick before Source's movement
//! (`step`, predicted) and handed to it as a `SourceStance` (the box, the
//! eye, the speed, no jump, no input):
//!
//! - **Going prone** (`+alt1`, `PRONE_ACTION`, one press, on the ground or
//!   in the air): the eye eases from where it is to 10 over 1.2 s (80
//!   ticks, `GO_PRONE`) by `prone_ease` (1 − (1 − t)^(7/3), the spec's fit
//!   of the measured heights), the move keys doing nothing meanwhile
//!   (`SourceStance::no_input`): the speed the player had goes on, slowed
//!   by friction (pressed sprinting at 305: 199 after 36 ticks), a jump
//!   flies on with the standing box and lands. Begun ducked, the ducked box
//!   (45) is held and the eye eases 34 to 10. On the 80th tick the box
//!   becomes 24 high (`PRONE_HEIGHT`) and the player is prone.
//! - **Prone**: eye 10, crawling at 50 units/s (`PRONE_SPEED`; the sprint
//!   key doesn't change it), the jump key does nothing (measured).
//! - **Getting up** (`+alt1` again): the standing box at once, the eye
//!   eased 10 to 58 over 1.2 s (`GET_UP`) by the same curve, movement
//!   allowed with a cap rising from the crawl's 50 to the run's 220 by a
//!   smoothstep of the time (`rise`: the measured speeds, 7.51 on the press
//!   tick, 95.09 at 28, 172.11 at 52, 218.77 at 76, 220 at 80). **Duck
//!   pressed while prone** gets up to a crouch the same way: the ducked box
//!   at once, the eye to 34, the cap rising to the ducked 73.33; at the end
//!   the player is ducked as if duck were held (`SourceStance::crouch`),
//!   and stands by the usual unduck once it isn't. No jump meanwhile.
//!   **Under a ceiling too low** for the box (72 standing, 45 ducked:
//!   `room`) the key is refused and nothing changes.
//! - **Deployed** (a machine gun in hand, `dods:mg`: `weapons::mg`): the
//!   alt fire while prone deploys it at once, its next shot
//!   `DodsMg::deploy_time` later (1.2163 the .30 cal, 1.1667 the MG42); the
//!   alt fire again undeploys, the next shot `undeploy_time` later (1.3635,
//!   1.1667); so does looking down past 47° or up past 63°
//!   (`PITCH_DOWN_LIMIT`, `PITCH_UP_LIMIT`), the player staying prone.
//!   The yaw limit (`yaw_limit`, 45° either way of the deploy) is the
//!   client's: the player wears a `ViewYawLimit` while deployed
//!   (`yaw_limits`), which the local client holds its view inside; turning
//!   past it never undeploys. **On a ledge**: the alt fire standing or
//!   crouched on the ground deploys on a surface within 12 units ahead of
//!   the box (`ledge_top`) whose top is 10 to 54 units over the feet
//!   standing, 10 to 38 crouched; the eye eases over 0.3 s (`LEDGE_EASE`)
//!   to the top + 16 (at most 58), the standing box held (a crouched
//!   player stands). Deployed, the player doesn't move and the gun's
//!   spread is its deployed one (`dods:gun`). Leaving prone, or another
//!   weapon in hand, undeploys it.
//! - **The speed cap** standing up: the stamina's (`stamina::cap`: 120 +
//!   stamina running, 230 + stamina sprinting), 120 while slowed by a hit
//!   (`hit_slow::DodsSlowed`, `SLOWED_CAP`), 50 with a rocket shouldered
//!   (`weapons::explosives::Shoulder`), 42 sighted (`SIGHTED_CAP`: the
//!   sights' or scope's narrower view on, `gun::DodsSights`); ducked or
//!   ducking the run's 220, which Source's movement scales by a third,
//!   whatever the stamina or the slow. The movement clamps the ground
//!   speed to it the same tick (`movement::config`'s `ground_clamp`).
//!
//! The stance goes on and off with `dods:movement` (`movement::put_on`,
//! `leave`), as its stamina does; a death drops it.
//!
//! Known shortcuts (docs/tech-debt.md):
//! - The speed cap getting up is a smoothstep of the time (the measured
//!   speeds fit it; movement.md, open question 1). Going prone on a ladder
//!   or in water is refused (our choice; unmeasured).
//! - Nothing narrows the deployed yaw limit below 45 (obstacles beside the
//!   gun do in DoD:S, by a rule not found: weapons.md, open question 11);
//!   on a ledge the pitch limits are the prone ones (unmeasured), and the
//!   ledge's surface is the highest of four downward traces 1 to 12 units
//!   ahead of the box.
//! - The speed while the sights rise (168 to about 190 in the 0.3 s) and
//!   the sighted acceleration from rest are unmeasured laws (movement.md,
//!   open question 8): the cap applies once the view is narrow.
//! - The slow's state is the server's (`hit_slow`): a predicting victim
//!   learns its cap from the next correction.

use avian3d::prelude::{Collider, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{Inventory, WeaponState, Zoomed};
use source_engine::source_movement::{SourceMovement, SourceStance};

use super::{
    movement::{DUCK_HEIGHT, EYE_DUCK, EYE_STAND, RUN_SPEED, STAND_HEIGHT},
    stamina::{self, DodsStamina},
    weapons::{explosives::Shoulder, gun::DodsSights, hit_slow::DodsSlowed, mg::DodsMg},
};
use crate::core::{ActionRegistry, CHARACTER_FILTER, Dead, Intent, MovementState, SimClock, ViewYawLimit};

/// The named action that goes prone and gets up (DoD:S's `+alt1`).
pub const PRONE_ACTION: &str = "alt1";

/// Going prone, seconds (movement.md: 80 ticks, measured).
pub const GO_PRONE: f32 = 1.2;

/// Getting up, to standing or to a crouch, seconds (measured: 80 ticks).
pub const GET_UP: f32 = 1.2;

/// The prone box's height and its eye, units (measured).
pub const PRONE_HEIGHT: f32 = 24.0;
pub const EYE_PRONE: f32 = 10.0;

/// Crawling speed, units/s (measured).
pub const PRONE_SPEED: f32 = 50.0;

/// The cap while slowed by a hit, units/s (damage.md, measured).
pub const SLOWED_CAP: f32 = 120.0;

/// The cap with the sights or scope up, units/s (movement.md, "View").
pub const SIGHTED_CAP: f32 = 42.0;

/// The duck's share of the run (movement.md: 73.33 of 220).
pub const DUCK_FACTOR: f32 = 1.0 / 3.0;

/// The ease's exponent (movement.md, "Prone": the eye curve's fit).
pub const EASE_POWER: f32 = 7.0 / 3.0;

/// A deployed machine gun undeploys looking further down or up than
/// this, degrees (weapons.md, "Alt fire": 46 never, 47-48 did; 66 did).
pub const PITCH_DOWN_LIMIT: f32 = 47.0;
pub const PITCH_UP_LIMIT: f32 = 63.0;

/// Deploying on a ledge (weapons.md, "Alt fire"): the eye's ease, seconds
/// (about 20 ticks), the deployed height over the surface, how far ahead
/// of the box the surface may be, and the tops over the feet that deploy
/// standing and crouched (10-54 and 14-38 deployed; 8, 56 and 44 not),
/// units.
pub const LEDGE_EASE: f32 = 0.3;
pub const LEDGE_ABOVE: f32 = 16.0;
pub const LEDGE_REACH: f32 = 12.0;
pub const LEDGE_STANDING: (f32, f32) = (10.0, 55.0);
pub const LEDGE_CROUCHED: (f32, f32) = (10.0, 41.0);

/// Source's unit, m.
const U: f32 = 0.0254;

/// The box's half width, units.
const HALF_WIDTH: f32 = 16.0;

/// How far through a transition a fraction `t` (0..1) of its time is: an
/// ease out, 1 − (1 − t)^(7/3) (movement.md, "Prone": the eye going down
/// and getting up alike; 0.029, 0.488, 0.801, 0.961 at 1/80, 1/4, 1/2,
/// 3/4 against the measured 0.029, 0.487, 0.800, 0.960).
pub fn prone_ease(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powf(EASE_POWER)
}

/// How far the speed cap getting up has risen at a fraction `t` of its
/// time: a smoothstep, 3t² − 2t³ (the measured speeds while getting up:
/// the cap 50 + 170 × it, module doc).
pub fn rise(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Where a DoD:S player is between standing and prone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stance {
    /// Standing or ducked: Source's own stances.
    #[default]
    Up,
    GoingDown,
    Prone,
    GettingUp,
}

/// A DoD:S player's stance (on with `dods:movement`; predicted).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DodsStance {
    pub stance: Stance,
    /// Seconds into the transition (going down, getting up, or easing to
    /// a ledge's deployed height).
    pub elapsed: f32,
    /// The eye the transition started from, units.
    pub from_eye: f32,
    /// Getting up to a crouch (duck pressed while prone).
    #[serde(default)]
    pub to_crouch: bool,
    /// Going down from a crouch: the ducked box held until prone.
    #[serde(default)]
    pub from_ducked: bool,
    /// A machine gun deployed (prone, or on a ledge: `ledge`), and the
    /// view's yaw (radians, `Intent::yaw`) it was deployed at.
    pub deployed: bool,
    #[serde(default)]
    pub deploy_yaw: f32,
    /// Deployed on a ledge standing or crouched: the deployed eye height
    /// over the feet, units.
    #[serde(default)]
    pub ledge: Option<f32>,
    /// `+alt1`, the alt fire and duck as held last tick (presses act).
    pub prone_key: bool,
    pub alt_key: bool,
    #[serde(default)]
    pub duck_key: bool,
}

impl DodsStance {
    /// Whether the player is fully prone.
    pub fn prone(&self) -> bool {
        self.stance == Stance::Prone
    }

    /// What Source's movement does with it this tick, `cap` the speed cap
    /// standing up (units/s; `step`).
    pub fn source(&self, cap: f32) -> SourceStance {
        match self.stance {
            Stance::Up => match self.ledge {
                Some(eye) => SourceStance {
                    height: Some(STAND_HEIGHT),
                    eye: Some(self.from_eye + (eye - self.from_eye) * prone_ease(self.elapsed / LEDGE_EASE)),
                    max_speed: Some(0.0),
                    no_jump: true,
                    crouch: false,
                    no_input: true,
                },
                None => SourceStance { max_speed: Some(cap), ..default() },
            },
            Stance::GoingDown => SourceStance {
                height: self.from_ducked.then_some(DUCK_HEIGHT),
                eye: Some(self.from_eye + (EYE_PRONE - self.from_eye) * prone_ease(self.elapsed / GO_PRONE)),
                // The keys do nothing; the speed it had decays (never cut).
                max_speed: Some(stamina::SPRINT_TOP),
                no_jump: true,
                crouch: false,
                no_input: true,
            },
            Stance::Prone => SourceStance {
                height: Some(PRONE_HEIGHT),
                eye: Some(EYE_PRONE),
                max_speed: Some(if self.deployed { 0.0 } else { PRONE_SPEED }),
                no_jump: true,
                crouch: false,
                no_input: false,
            },
            Stance::GettingUp => {
                let t = self.elapsed / GET_UP;
                let (eye, top, height) = if self.to_crouch {
                    (EYE_DUCK, RUN_SPEED * DUCK_FACTOR, Some(DUCK_HEIGHT))
                } else {
                    (EYE_STAND, RUN_SPEED, None)
                };
                SourceStance {
                    height,
                    eye: Some(EYE_PRONE + (eye - EYE_PRONE) * prone_ease(t)),
                    max_speed: Some(PRONE_SPEED + (top - PRONE_SPEED) * rise(t)),
                    no_jump: true,
                    crouch: self.to_crouch,
                    no_input: false,
                }
            }
        }
    }
}

/// The world's solids around a player (its own box and other characters
/// left out).
#[derive(bevy::ecs::system::SystemParam)]
pub struct Room<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    characters: Query<'w, 's, (), With<Intent>>,
}

impl Room<'_, '_> {
    /// Whether a box `height` units high fits standing at `feet` (m).
    pub fn fits(&self, me: Entity, feet: Vec3, height: f32) -> bool {
        // A hair off the floor and narrower, so the floor and walls it
        // touches don't count.
        let h = height - 1.0;
        let shape = Collider::cuboid((HALF_WIDTH * 2.0 - 0.5) * U, h * U, (HALF_WIDTH * 2.0 - 0.5) * U);
        let at = feet + Vec3::Y * (0.5 + h / 2.0) * U;
        let filter = SpatialQueryFilter::from_mask(CHARACTER_FILTER).with_excluded_entities([me]);
        self.spatial.shape_intersections(&shape, at, Quat::IDENTITY, &filter).into_iter().all(|e| self.characters.contains(e))
    }

    /// The top (units over `feet`) of the highest surface within
    /// `LEDGE_REACH` ahead of the box looking along `yaw`: downward traces
    /// from 64 units up at 1, 4, 8 and 12 units past the box's front (a
    /// trace starting inside a solid reads 64).
    pub fn ledge_top(&self, me: Entity, feet: Vec3, yaw: f32) -> Option<f32> {
        const FROM: f32 = 64.0;
        let ahead = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
        let filter = SpatialQueryFilter::from_mask(CHARACTER_FILTER).with_excluded_entities([me]);
        [1.0, 4.0, 8.0, LEDGE_REACH]
            .into_iter()
            .filter_map(|d| {
                let start = feet + ahead * (HALF_WIDTH + d) * U + Vec3::Y * FROM * U;
                let hit = self.spatial.cast_ray(start, Dir3::NEG_Y, FROM * U, true, &filter)?;
                (!self.characters.contains(hit.entity)).then_some(FROM - hit.distance / U)
            })
            .reduce(f32::max)
    }
}

/// Each DoD:S player's stance this tick, before Source's movement: the
/// presses of `+alt1` (prone, up), duck while prone (up to a crouch) and
/// the alt fire with a machine gun (deploy, undeploy), the transitions'
/// time, the tick's speed cap, and what the movement does with it
/// (`SourceStance`).
#[allow(clippy::type_complexity)]
pub fn step(
    mut players: Query<(
        (Entity, &Intent, &Transform, &MovementState),
        &mut DodsStance,
        &mut SourceStance,
        &SourceMovement,
        Option<&Inventory>,
        Option<&DodsStamina>,
        Option<&DodsSlowed>,
        Has<Zoomed>,
        Has<Dead>,
    )>,
    mut guns: Query<(&DodsMg, &mut WeaponState)>,
    shoulders: Query<&Shoulder>,
    sights: Query<(), With<DodsSights>>,
    room: Room,
    registry: Option<Res<ActionRegistry>>,
    clock: Res<SimClock>,
) {
    let dt = clock.dt();
    for ((me_e, intent, transform, state), mut st, mut source, me, inv, stamina, slowed, zoomed, dead) in &mut players {
        let mut s = *st;
        let mut last_rise = None;
        let feet = transform.translation + Vec3::Y * state.hull_min.y;
        let prone_key = registry.as_deref().is_some_and(|r| intent.action(r, PRONE_ACTION));
        let pressed = prone_key && !s.prone_key;
        let alt_pressed = intent.secondary && !s.alt_key;
        let duck_pressed = intent.crouch && !s.duck_key;
        s.prone_key = prone_key;
        s.alt_key = intent.secondary;
        s.duck_key = intent.crouch;
        if dead {
            s = DodsStance { prone_key, alt_key: intent.secondary, duck_key: intent.crouch, ..default() };
        } else {
            match s.stance {
                Stance::Up => {
                    // On the ground or in the air (measured), not on a
                    // ladder or in water (ours).
                    if pressed && me.ladder.is_none() && me.water_level < 2 {
                        s.stance = Stance::GoingDown;
                        s.from_eye = me.eye;
                        s.from_ducked = me.ducked && s.ledge.is_none();
                        // The press's tick is the first of the 80.
                        s.elapsed = dt;
                    } else if s.ledge.is_some() {
                        s.elapsed += dt;
                    }
                }
                Stance::GoingDown | Stance::GettingUp => {
                    s.elapsed += dt;
                }
                Stance::Prone => {
                    // Refused under a ceiling too low for the box.
                    let height = if pressed { STAND_HEIGHT } else { DUCK_HEIGHT };
                    if (pressed || duck_pressed) && room.fits(me_e, feet, height) {
                        s.stance = Stance::GettingUp;
                        s.to_crouch = !pressed;
                        s.elapsed = dt;
                    }
                }
            }
            // A transition ends on the tick its time is reached (the 80th).
            let ends = |t: f32| s.elapsed >= t - dt * 0.5;
            if s.stance == Stance::GettingUp && ends(GET_UP) {
                // Its last tick still eases (the eye at its end).
                last_rise = Some(DodsStance { elapsed: GET_UP, ..s });
            }
            if s.stance == Stance::GoingDown && ends(GO_PRONE) {
                s.stance = Stance::Prone;
                s.elapsed = 0.0;
                s.from_ducked = false;
            } else if s.stance == Stance::GettingUp && ends(GET_UP) {
                s.stance = Stance::Up;
                s.elapsed = 0.0;
                s.to_crouch = false;
            }
        }
        // The machine gun in hand: deployed by the alt fire prone or on a
        // ledge, undeployed by it again or by the pitch past its limits.
        let active = inv.and_then(|i| i.active);
        let up = s.stance == Stance::Up && me.on_ground;
        match active.and_then(|g| guns.get_mut(g).ok()) {
            Some((mg, mut gun)) if !dead && (s.prone() || up) => {
                let toggled = if s.deployed {
                    let past_pitch = intent.pitch < -PITCH_DOWN_LIMIT.to_radians() || intent.pitch > PITCH_UP_LIMIT.to_radians();
                    if alt_pressed || past_pitch {
                        s.deployed = false;
                        s.ledge = None;
                        true
                    } else {
                        false
                    }
                } else if alt_pressed && s.prone() {
                    s.deployed = true;
                    true
                } else if alt_pressed {
                    let (low, high) = if me.ducked || me.ducking { LEDGE_CROUCHED } else { LEDGE_STANDING };
                    match room.ledge_top(me_e, feet, intent.yaw).filter(|t| (low..=high).contains(t)) {
                        Some(top) => {
                            s.deployed = true;
                            s.ledge = Some((top + LEDGE_ABOVE).min(EYE_STAND));
                            s.from_eye = me.eye;
                            s.elapsed = dt;
                            true
                        }
                        None => false,
                    }
                } else {
                    false
                };
                if toggled {
                    s.deploy_yaw = intent.yaw;
                    let wait = if s.deployed { mg.deploy_time } else { mg.undeploy_time };
                    let ready = clock.now + wait as f64;
                    if gun.next_primary < ready {
                        gun.next_primary = ready;
                    }
                }
            }
            _ => {
                s.deployed = false;
                s.ledge = None;
            }
        }
        if *st != s {
            *st = s;
        }
        // The cap standing up (module doc), from the stamina before this
        // tick's change (`stamina::step` runs after).
        let ducked = me.ducked || me.ducking || intent.crouch;
        let shouldered = active.and_then(|a| shoulders.get(a).ok()).is_some_and(|sh| sh.shouldered);
        let sighted = zoomed && active.is_some_and(|a| sights.contains(a));
        let mut cap = if ducked {
            if shouldered { (Shoulder::MAX_SPEED / DUCK_FACTOR).min(RUN_SPEED) } else { RUN_SPEED }
        } else {
            let keys = stamina::sprint_keys(intent, me, Some(&s));
            let mut c = stamina::cap(stamina.map_or(stamina::MAX, |s| s.value), keys);
            if slowed.is_some_and(|sl| sl.at(clock.now)) {
                c = c.min(SLOWED_CAP);
            }
            if shouldered {
                c = c.min(Shoulder::MAX_SPEED);
            }
            c
        };
        if sighted {
            cap = cap.min(if ducked { SIGHTED_CAP / DUCK_FACTOR } else { SIGHTED_CAP });
        }
        let new = last_rise.unwrap_or(s).source(cap);
        if *source != new {
            *source = new;
        }
    }
}

/// The deployed gun's yaw limit as the player's `ViewYawLimit` (the local
/// client holds its view inside it; the server doesn't clamp), after
/// `step`.
pub fn yaw_limits(
    players: Query<(Entity, &DodsStance, Option<&Inventory>, Option<&ViewYawLimit>)>,
    guns: Query<&DodsMg>,
    mut commands: Commands,
) {
    for (e, s, inv, had) in &players {
        let want = s
            .deployed
            .then(|| inv.and_then(|i| i.active).and_then(|g| guns.get(g).ok()))
            .flatten()
            .map(|mg| ViewYawLimit { centre: s.deploy_yaw, half: mg.yaw_limit.to_radians() });
        if had != want.as_ref() {
            match want {
                Some(l) => commands.entity(e).insert(l),
                None => commands.entity(e).remove::<ViewYawLimit>(),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// movement.md, "Prone": the eye after n ticks of going prone, and of
    /// getting up (10 to 58), on the spec's curve 1 − (1 − t)^(7/3).
    #[test]
    fn the_ease_fits_the_measured_eye_heights() {
        let down = [(1, 56.61), (2, 55.25), (9, 46.38), (19, 35.57), (29, 26.87), (39, 20.16), (49, 15.31), (59, 12.15), (69, 10.47), (79, 10.0)];
        for (n, eye) in down {
            let ours = 58.0 + (10.0 - 58.0) * prone_ease(n as f32 / 80.0);
            assert!((ours - eye).abs() < 0.1, "tick {n}: {ours} against {eye}");
        }
        let up = [(1, 11.38), (20, 33.38), (40, 48.40), (56, 55.07), (71, 57.70)];
        for (n, eye) in up {
            let ours = 10.0 + 48.0 * prone_ease(n as f32 / 80.0);
            assert!((ours - eye).abs() < 0.1, "getting up, tick {n}: {ours} against {eye}");
        }
    }

    /// movement.md, "Getting up": the cap rises from 50 to 220 (the
    /// measured speeds track it: 95.09 at tick 28, 172.11 at 52, 218.77 at
    /// 76, 220 at 80; a speed lags its rising cap by a little).
    #[test]
    fn getting_up_raises_the_cap_by_a_smoothstep() {
        let at = |k: f32| PRONE_SPEED + (RUN_SPEED - PRONE_SPEED) * rise(k / 80.0);
        assert!((at(1.0) - 50.08).abs() < 0.01);
        for (k, v) in [(52.0, 172.11), (64.0, 202.32), (76.0, 218.77), (80.0, 220.0)] {
            assert!((at(k) - v).abs() < 1.0, "tick {k}: {} against {v}", at(k));
        }
    }

    #[test]
    fn prone_holds_the_low_box_crawls_and_never_jumps() {
        let s = DodsStance { stance: Stance::Prone, ..default() };
        let src = s.source(220.0);
        assert_eq!((src.height, src.eye, src.max_speed, src.no_jump), (Some(24.0), Some(10.0), Some(50.0), true));
        let deployed = DodsStance { deployed: true, ..s };
        assert_eq!(deployed.source(220.0).max_speed, Some(0.0));
        assert_eq!(DodsStance::default().source(170.0), SourceStance { max_speed: Some(170.0), ..default() });
    }
}
