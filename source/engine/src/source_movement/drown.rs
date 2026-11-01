//! Drowning, as measured on a CS:S server (specs/cs_source/movement.md,
//! "Drowning"): a player whose head is under water (Source movement's
//! water level 3) for 12.0 s takes drowning damage, then a hit about every
//! second, 3, 4, then 5 each (+1 a hit, at most 5) until it surfaces or
//! dies. After surfacing, the drowning damage taken comes back 10 at a time
//! about every 2 s (the first within the first second), never more than
//! was taken and never above max health. For a Source movement whose
//! config has `drowning` (CS:S's); HL2:DM's suit drowns its own way.
//!
//! The clocks are strict "later than" tests on the simulation's time, which
//! reproduces the measured ticks at CS:S's 0.015 s: the first hit on the
//! 801st tick under water (12.015 s after the last dry tick), then every
//! 67 ticks (1.005 s); 100 health is dead at 32.1 s. The amount owed grows
//! by the drowning damage health really lost (`core::DamageTaken` of kind
//! `Drown`: not what god mode or a map's damage filter refused).
//!
//! Server-side only (`core::authoritative`): a network client sees its
//! health change through replication. The damage has no attacker (a death
//! counts as a world kill, like a fall), no direction (no damage indicator),
//! no armour absorption and no hurt sound (`pain::hurt_sound`); a death
//! plays `Player.Death`.
//!
//! Known shortcuts (docs/tech-debt.md): Drowning's unmeasured parts are our
//! choices: surfacing resets the 12 s clock and the next hit's damage back to
//! 3; the first +10 comes when the next hit would have been due (at once if
//! that is past), then every 2 s, and only while the head is out of the
//! water; other damage taken meanwhile doesn't change the amount owed; the
//! amount owed is forgotten at death and once health is full again (a round
//! restart); armour doesn't absorb drowning (the measurement had none). The
//! client's own `Player.DrownStart`/`DrownContinue` (if CS:S plays them
//! locally) are not played. Why it's OK for now: the measured timeline (hits,
//! death time, restore steps) matches tick for tick.

use bevy::prelude::*;

use super::{MovementConfigs, SourceMovement};
use crate::core::{Damage, DamageKind, DamageTaken, Health, Hitgroup, SimClock, SimSet};

/// Seconds the head can stay under water before the first hit.
pub const AIR_SECONDS: f64 = 12.0;
/// Seconds between hits.
pub const HIT_SECONDS: f64 = 1.0;
/// The first hit's damage minus one (each hit adds one first), points.
const FIRST_DAMAGE: f32 = 2.0;
/// The most one hit takes, points.
const MAX_DAMAGE: f32 = 5.0;
/// Points given back per restore step, and seconds between steps.
pub const RESTORE_POINTS: f32 = 10.0;
pub const RESTORE_SECONDS: f64 = 2.0;
/// Two times this close count as the same (float noise in the clock).
const SAME_TIME: f64 = 1e-6;
/// Health is normalized: 1.0 is 100 points.
const POINTS: f32 = 100.0;

/// A player's drowning state (on every Source mover).
#[derive(Component, Clone, Debug, Default)]
pub struct Drowning {
    /// When air runs out: 12 s after the last tick the head was dry.
    air_finished: f64,
    /// Before this time no hit (under water) or restore (out of it) comes.
    pain_finished: f64,
    /// The last hit's damage, points.
    damage: f32,
    /// Drowning damage taken and not yet given back, points.
    pub owed: f32,
    /// The state was set from a tick (a new mover starts dry).
    started: bool,
}

pub struct DrowningPlugin;

impl Plugin for DrowningPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                drown.after(SimSet::Movement).before(crate::core::apply_damage),
                owe.after(crate::core::apply_damage),
            )
                .run_if(crate::core::authoritative),
        );
    }
}

/// Hits under water, restores out of it, from this tick's water level.
fn drown(
    clock: Res<SimClock>,
    mut commands: Commands,
    mut players: Query<(
        Entity,
        &SourceMovement,
        &mut Health,
        &Transform,
        Option<&mut Drowning>,
        Option<&crate::slots::MovementSlot>,
    )>,
    mut damage: MessageWriter<Damage>,
    configs: MovementConfigs,
) {
    let now = clock.now;
    for (entity, me, mut health, at, state, slot) in &mut players {
        // A game with drowning of its own (HL2:DM's suit).
        if !configs.of(slot).drowning {
            continue;
        }
        let Some(mut state) = state else {
            commands.entity(entity).try_insert(Drowning::default());
            continue;
        };
        if !state.started || health.current <= 0.0 {
            // Dead (or new): nothing owed, the clocks start over.
            *state = Drowning {
                air_finished: now + AIR_SECONDS,
                damage: FIRST_DAMAGE,
                started: true,
                ..default()
            };
            continue;
        }
        if health.current >= health.max {
            state.owed = 0.0;
        }
        if me.water_level >= 3 {
            if now > state.air_finished + SAME_TIME && now > state.pain_finished + SAME_TIME {
                state.damage = (state.damage + 1.0).min(MAX_DAMAGE);
                state.pain_finished = now + HIT_SECONDS;
                damage.write(Damage {
                    target: entity,
                    attacker: None,
                    amount: state.damage / POINTS,
                    point: at.translation,
                    dir: Vec3::ZERO,
                    hitgroup: Hitgroup::Generic,
                    kind: DamageKind::Drown,
                    weapon: None,
                    force: Vec3::ZERO,
                });
            }
        } else {
            state.air_finished = now + AIR_SECONDS;
            state.damage = FIRST_DAMAGE;
            if state.owed > 0.0 && now > state.pain_finished + SAME_TIME {
                let give = state.owed.min(RESTORE_POINTS);
                state.owed -= give;
                state.pain_finished = now + RESTORE_SECONDS;
                health.current = (health.current + give / POINTS).min(health.max);
            }
        }
    }
}

/// Drowning damage health really lost is owed back.
fn owe(mut taken: MessageReader<DamageTaken>, mut players: Query<&mut Drowning>) {
    for t in taken.read() {
        if t.damage.kind != DamageKind::Drown {
            continue;
        }
        if let Ok(mut state) = players.get_mut(t.damage.target) {
            state.owed += t.damage.amount * POINTS;
        }
    }
}
