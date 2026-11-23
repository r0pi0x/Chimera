//! Timed conditions on a character that more than one game has: a
//! bleeding wound (damage over time), a speed boost and a slow. A game gives them
//! (its weapon's hit, its rule) and reads them (its movement's max speed);
//! the ticks are the engine's (`add`, the server's).
//!
//! - **Bleeding** (`Bleeding`, `bleed`): one wound per victim, credited to
//!   whoever caused it with what. It deals `amount` every `period` (each
//!   next tick `period` after the last one's tick: on a fixed tick the
//!   period rounds up to whole ticks, TF2's 0.5 s landing every 34 ticks
//!   of 0.015 s), the first in the tick of the hit that caused it
//!   (`Bleeding::start`), and ticks while its next tick is before its
//!   end. A new wound on one already bleeding adds no tick: its end moves
//!   to the new hit's time plus its duration, the cadence stays
//!   (`Bleeding::refresh`). Its ticks are `DamageKind::Bleed`, naming the
//!   weapon; a dead victim's wound is gone.
//! - **Speed boost** (`SpeedBoost`): until `until` (sim time), a max speed
//!   × `mult`, the gain at most `cap` (in the units of the base the game
//!   asks with, `SpeedBoost::factor`); a later boost keeps the later end.
//!   The game's movement applies it; it is removed when it ends
//!   (`boost_ends`, first in each predicted tick, `Predict::Select`) or
//!   the character dies. It is predicted state (`predicted_net`): the
//!   server sends a client's own player's with its movement, so the
//!   client's prediction moves at the boosted speed and ends it on the
//!   server's tick; its start (a hit, the server's) is taken as a
//!   correction once.
//! - **Slow** (`Slowed`): until `until`, a max speed × `factor` (0: no
//!   movement of his own at all: TF2's Loose Cannon's stop; 0.5: the
//!   Sandman's ball); a new slow keeps the later end and the stronger
//!   factor (`Slowed::with`). Applied, ended and predicted as the boost
//!   (`boost_ends`, `predicted_net`).
//!
//! Known shortcuts: a bleeding wound is the server's part (not
//! replicated). A wound given by a hit's after-effects (after the tick's
//! damage) deals its first tick one server tick after the hit.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::core::{Damage, DamageKind, Health, Hitgroup, PredictedAppExt, SimClock};

/// Slack for "at or after" on sim times (s).
const EPS: f64 = 1e-6;

/// A bleeding wound (module doc).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Bleeding {
    pub attacker: Option<Entity>,
    pub weapon: Option<&'static str>,
    /// Damage per tick (normalized, as `Health`).
    pub amount: f32,
    /// Seconds between ticks.
    pub period: f32,
    /// The next tick (sim time).
    pub next: f64,
    /// It ticks while its next tick is before this.
    pub end: f64,
}

impl Bleeding {
    /// A wound from a hit at `now` lasting `duration` s: its first tick
    /// now.
    pub fn start(now: f64, duration: f32, amount: f32, period: f32, attacker: Option<Entity>, weapon: Option<&'static str>) -> Self {
        Self { attacker, weapon, amount, period, next: now, end: now + duration as f64 }
    }

    /// `old` (if any) hit again at `now`: the new end, the old cadence
    /// (module doc); none: a fresh wound.
    pub fn refresh(old: Option<&Bleeding>, new: Bleeding, now: f64) -> Bleeding {
        match old {
            Some(o) if o.next < o.end - EPS => Bleeding { next: o.next, end: o.end.max(now + (new.end - now)), ..new },
            _ => new,
        }
    }
}

/// A speed boost (module doc).
#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeedBoost {
    pub until: f64,
    pub mult: f32,
    pub cap: f32,
}

impl SpeedBoost {
    /// The factor on a max speed for a character whose own base is
    /// `base` (the cap's units): `mult`, at most (base + cap) / base.
    pub fn factor(&self, base: f32) -> f32 {
        if base <= 0.0 {
            return self.mult;
        }
        self.mult.min((base + self.cap) / base)
    }

    /// The boost `new` on top of `old`: the later end.
    pub fn longer(old: Option<&SpeedBoost>, new: SpeedBoost) -> SpeedBoost {
        match old {
            Some(o) if o.until > new.until => SpeedBoost { until: o.until, ..new },
            _ => new,
        }
    }
}

/// A slow (module doc).
#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slowed {
    pub until: f64,
    pub factor: f32,
}

impl Slowed {
    /// `new` on top of `old`: the later end, the stronger slow.
    pub fn with(old: Option<&Slowed>, new: Slowed) -> Slowed {
        match old {
            Some(o) => Slowed { until: o.until.max(new.until), factor: o.factor.min(new.factor) },
            None => new,
        }
    }

    /// The factor on a max speed now (1 once it ended).
    pub fn factor_at(s: Option<&Slowed>, now: f64) -> f32 {
        s.filter(|s| s.until > now + EPS).map_or(1.0, |s| s.factor)
    }
}

/// Every wound's ticks due (the server's).
pub fn bleed(mut q: Query<(Entity, &mut Bleeding, &Health)>, clock: Res<SimClock>, mut damage: MessageWriter<Damage>, mut commands: Commands) {
    let now = clock.now;
    for (e, mut b, h) in &mut q {
        if h.current <= 0.0 {
            commands.entity(e).remove::<Bleeding>();
            continue;
        }
        let mut alive = true;
        while b.next <= now + EPS {
            if b.next >= b.end - EPS {
                alive = false;
                break;
            }
            damage.write(Damage {
                target: e,
                attacker: b.attacker,
                amount: b.amount,
                point: Vec3::ZERO,
                dir: Vec3::Y,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Bleed,
                weapon: b.weapon,
                force: Vec3::ZERO,
            });
            // The next from this tick's time (module doc).
            b.next = now + b.period as f64;
        }
        if !alive || b.next >= b.end - EPS {
            commands.entity(e).remove::<Bleeding>();
        }
    }
}

/// Boosts and slows that ended, or whose character died, are taken off
/// (every machine, in the predicted tick: a client ends its own player's
/// on the server's tick).
pub fn boost_ends(
    q: Query<(Entity, Option<&SpeedBoost>, Option<&Slowed>, Option<&Health>), Or<(With<SpeedBoost>, With<Slowed>)>>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    for (e, b, s, h) in &q {
        let dead = h.is_some_and(|h| h.current <= 0.0);
        if b.is_some_and(|b| dead || b.until <= clock.now + EPS) {
            commands.entity(e).remove::<SpeedBoost>();
        }
        if s.is_some_and(|s| dead || s.until <= clock.now + EPS) {
            commands.entity(e).remove::<Slowed>();
        }
    }
}

/// The ticks, once per app (the weapons' damage applied after them in
/// the tick: `core::apply_damage`).
pub fn add(app: &mut App) {
    #[derive(Resource)]
    struct Added;
    if app.world().contains_resource::<Added>() {
        return;
    }
    app.insert_resource(Added)
        .add_systems(
            FixedUpdate,
            bleed
                .after(crate::core::SimSet::Weapons)
                .before(crate::core::apply_damage)
                .run_if(crate::core::authoritative),
        )
        .add_systems(crate::core::Predict::Select, boost_ends)
        .predicted_net::<SpeedBoost>()
        .predicted_net::<Slowed>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_boost_caps_its_gain_and_keeps_the_later_end() {
        let b = SpeedBoost { until: 3.0, mult: 1.4, cap: 105.0 };
        assert!((400.0 * b.factor(400.0) - 505.0).abs() < 1e-3);
        assert!((240.0 * b.factor(240.0) - 336.0).abs() < 1e-3);
        let later = SpeedBoost::longer(Some(&SpeedBoost { until: 5.0, ..b }), b);
        assert_eq!(later.until, 5.0);
    }

    #[test]
    fn a_refreshed_wound_keeps_its_cadence() {
        let a = Bleeding::start(0.0, 5.0, 0.04, 0.5, None, None);
        let mut ticking = a;
        ticking.next = 1.53;
        let r = Bleeding::refresh(Some(&ticking), Bleeding::start(1.29, 5.0, 0.04, 0.5, None, None), 1.29);
        assert_eq!(r.next, 1.53);
        assert!((r.end - 6.29).abs() < 1e-9);
    }
}
