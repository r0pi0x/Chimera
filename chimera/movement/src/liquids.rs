//! GoldSrc's liquids on the player (specs/cs16/water.md; for a movement
//! whose config has `liquid_hazards`): slime and lava hurt on every
//! command (slime 4, lava 10, times the water level), the air timer (12 s
//! with the eye under) and the drowning after it (3, 4, then 5 a second),
//! the gasp on surfacing (`pl_wade1` when the air had run out, `pl_wade2`
//! after more than 3 s under), the bubbles (a 1 in 32 chance a command
//! against the air left) and the recovery, 10 health every 2 s once
//! breathing, of what drowning took. One command's update, before the
//! movement of the same command, so the water level it reads is the one
//! the previous command's movement ended with.
//!
//! Server-side only (`core::authoritative`); its sounds are
//! `core::GameSound`s the server sends on. Damage has no attacker, no
//! direction and, for drowning, never armour.
//!
//! Known shortcuts (docs/tech-debt.md):
//! - Slime's damage is the `Poison` kind and lava's `Burn`: Chimera has no
//!   acid kind, and a game's armour rules read them as it lists them.
//! - The time-based recovery clock is the drowning's own (the original
//!   shares it with other damage over time: none of ours).
//! - The bubble's 1 in 32 draw and its roll are a hash of the character
//!   and the tick, not the game's random numbers.

use bevy::prelude::*;

use super::{config::MovementConfigs, state::QuakeMovement};
use crate::{
    core::{Damage, DamageKind, DamageTaken, Health, Hitgroup, SimClock, SimSet},
    map::{GameSound, PlaySound},
};

/// Air with the eye under, seconds.
pub const AIR_SECONDS: f64 = 12.0;
/// Seconds between drowning hits, and between recovery ticks.
pub const HIT_SECONDS: f64 = 1.0;
pub const RESTORE_SECONDS: f64 = 2.0;
/// The drown counter after breathing, its cap, health points.
const FIRST_COUNTER: f32 = 2.0;
const MAX_COUNTER: f32 = 5.0;
/// Points a recovery tick gives back.
pub const RESTORE_POINTS: f32 = 10.0;
/// Damage per command per water level, points.
pub const SLIME_DAMAGE: f32 = 4.0;
pub const LAVA_DAMAGE: f32 = 10.0;
/// Under this much air left (s) since the dive, the gasp is the short one.
const SHORT_DIVE_LEFT: f64 = 9.0;
/// Bubble sound volume, and the chance a command (1 in).
const BUBBLE_VOLUME: f32 = 0.8;
const BUBBLE_ODDS: u64 = 32;
/// The voice channel's number (a gasp).
const VOICE_CHANNEL: u8 = 2;
/// Health is normalized: 1.0 is 100 points.
const POINTS: f32 = 100.0;
const SAME_TIME: f64 = 1e-6;

/// A character's liquid state.
#[derive(Component, Clone, Debug, Default)]
pub struct LiquidState {
    /// When the air runs out: 12 s after the last command the eye was dry.
    air_finished: f64,
    /// The drowning damage's counter (2 after breathing).
    counter: f32,
    /// No drowning hit before this time.
    pain_finished: f64,
    /// No recovery tick before this time.
    restore_finished: f64,
    /// Drowning damage taken and not yet given back, points.
    pub owed: f32,
    /// The eye was under at the last update.
    was_under: bool,
    started: bool,
}

pub struct LiquidsPlugin;

impl Plugin for LiquidsPlugin {
    fn build(&self, app: &mut App) {
        use chimera_core::character::CharacterPartsExt;
        app.on_every_character::<LiquidState>();
        app.add_systems(
            FixedUpdate,
            (
                liquids.before(SimSet::Movement).before(crate::core::apply_damage),
                owe.after(crate::core::apply_damage),
            )
                .run_if(crate::core::authoritative),
        );
    }
}

/// A draw in 0..bound from the character and the tick.
fn draw(entity: Entity, tick: u64, salt: u64, bound: u64) -> u64 {
    let mut x = entity.to_bits() ^ tick.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ salt.wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x % bound.max(1)
}

#[allow(clippy::too_many_arguments)]
fn liquids(
    clock: Res<SimClock>,
    mut commands: Commands,
    mut players: Query<(
        Entity,
        &QuakeMovement,
        &mut Health,
        &Transform,
        Option<&mut LiquidState>,
        Option<&crate::slots::MovementSlot>,
    )>,
    mut damage: MessageWriter<Damage>,
    mut sounds: MessageWriter<GameSound>,
    configs: MovementConfigs,
) {
    let now = clock.now;
    for (entity, me, mut health, at, state, slot) in &mut players {
        let cfg = configs.of(slot);
        let Some(names) = cfg.liquid_sounds.filter(|_| cfg.liquid_hazards) else {
            continue;
        };
        let Some(mut state) = state else {
            commands.entity(entity).try_insert(LiquidState::default());
            continue;
        };
        if !state.started || health.current <= 0.0 {
            *state = LiquidState {
                air_finished: now + AIR_SECONDS,
                counter: FIRST_COUNTER,
                started: true,
                ..default()
            };
            continue;
        }
        if health.current >= health.max {
            state.owed = 0.0;
        }
        let say = |sounds: &mut MessageWriter<GameSound>, entry: &str, volume: f32, channel: u8| {
            sounds.write(GameSound(PlaySound {
                pitch: None,
                entry: entry.to_string(),
                at: Some(at.translation),
                volume: Some(volume),
                source: Some(entity),
                channel: Some(channel),
            }));
        };
        let level = f32::from(me.water_level);
        // Slime and lava, every command.
        if me.water_level > 0 && (me.in_slime || me.in_lava) {
            let (amount, kind) = if me.in_lava {
                (LAVA_DAMAGE * level, DamageKind::Burn)
            } else {
                (SLIME_DAMAGE * level, DamageKind::Poison)
            };
            damage.write(Damage {
                target: entity,
                attacker: None,
                amount: amount / POINTS,
                point: at.translation,
                dir: Vec3::ZERO,
                hitgroup: Hitgroup::Generic,
                kind,
                weapon: None,
                force: Vec3::ZERO,
            });
        }
        // The air.
        if me.water_level >= 3 {
            state.was_under = true;
            if now > state.air_finished + SAME_TIME && now > state.pain_finished + SAME_TIME {
                state.counter = (state.counter + 1.0).min(MAX_COUNTER);
                state.pain_finished = now + HIT_SECONDS;
                damage.write(Damage {
                    target: entity,
                    attacker: None,
                    amount: state.counter / POINTS,
                    point: at.translation,
                    dir: Vec3::ZERO,
                    hitgroup: Hitgroup::Generic,
                    kind: DamageKind::Drown,
                    weapon: None,
                    force: Vec3::ZERO,
                });
            }
        } else {
            if state.was_under {
                if now > state.air_finished {
                    say(&mut sounds, names.1, 1.0, VOICE_CHANNEL);
                } else if state.air_finished - now < SHORT_DIVE_LEFT {
                    say(&mut sounds, names.2, 1.0, VOICE_CHANNEL);
                }
            }
            state.was_under = false;
            state.air_finished = now + AIR_SECONDS;
            state.counter = FIRST_COUNTER;
            // The time-based recovery: a tick at most every 2 s.
            if state.owed > 0.0 && now + SAME_TIME >= state.restore_finished {
                let give = state.owed.min(RESTORE_POINTS);
                state.owed -= give;
                state.restore_finished = now + RESTORE_SECONDS;
                health.current = (health.current + give / POINTS).min(health.max);
            }
        }
        // Bubbles: 1 in 32 a command in a liquid, then a roll of 0-11
        // against the whole seconds of air left.
        if me.water_level > 0 && draw(entity, clock.tick, 1, BUBBLE_ODDS) == 0 {
            let left = (state.air_finished - now).max(0.0).trunc() as u64;
            if draw(entity, clock.tick, 2, 12) >= left {
                say(&mut sounds, names.0, BUBBLE_VOLUME, cfg.sound_channel);
            }
        }
    }
}

/// Drowning damage health really lost is owed back.
fn owe(mut taken: MessageReader<DamageTaken>, mut players: Query<&mut LiquidState>) {
    for t in taken.read() {
        if t.damage.kind != DamageKind::Drown {
            continue;
        }
        if let Ok(mut state) = players.get_mut(t.damage.target) {
            state.owed += t.damage.amount * POINTS;
        }
    }
}
