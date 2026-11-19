//! A Halo map's falling rule for every body on it (specs/halo_ce/
//! multiplayer_rules.md 6.9 "Leaving the map", movement.md section 6): on
//! every 30 Hz tick a living player falls faster than the globals'
//! "maximum falling velocity" (0.3430028 WU/t, `MovementData::fall_v_kill`)
//! it takes `globals\distance` (105 points, no instigator,
//! `damage::HaloHit::lethal_fall`). A full Spartan dies on the second such
//! tick, so a fall off Boarding Action kills about 17 WU down, in the air,
//! not 296 WU down on the level's invisible floor.
//!
//! Halo's own movement deals it in its tick (`tick::tick`, step 15, one hit
//! a 30 Hz tick: `ticks::halo_ticks`). Here: every other body on a Halo map
//! (the party apps' rule, a map's environment hurts every body whatever
//! game it is from): a player (`Intent`, `Health`) not on Halo's movement,
//! not dead, carried or seated, whose `Velocity` points down faster than
//! the same speed in metres a second (31.36 m/s), takes the same hit on
//! each 30 Hz tick its engine tick crosses, on the server. On another
//! game's body the hit is an unshielded Spartan's (`damage`: 105 of 75, a
//! death unless its armour holds) and, as every fall, is not scaled by the
//! cross-game table (`damage::deliver_hits`). The death is an ordinary one
//! with no killer: a recent attacker's credit or the suicide's
//! (`damage::Attackers`, the rules').
//!
//! Known shortcuts:
//! - Another game's body is tested on the velocity its movement stored,
//!   which for Source's movements is clamped at 3500 units a second (well
//!   above the threshold).

use bevy::prelude::*;

use super::{HaloMovement, MovementData};
use crate::core::{Carried, Dead, Health, Intent, MapGame, MovementState, SimClock, Velocity};
use crate::game::{damage::HaloHit, vehicles::Seated, vitality::DamageEffects};

/// The threshold in engine units (m/s, downward).
pub fn kill_speed(d: &MovementData) -> f32 {
    d.fall_v_kill * crate::game::METERS_PER_UNIT * super::data::TICKS_PER_SECOND
}

/// Whether the loaded map is Halo's.
pub fn on_halo_map(map: Option<Res<MapGame>>) -> bool {
    map.is_some_and(|m| m.0 == crate::game::GAME)
}

/// Every other body's too-fast fall (module doc).
#[allow(clippy::type_complexity)]
pub fn other_bodies(
    q: Query<
        (Entity, &Velocity, &Health, Option<&MovementState>),
        (With<Intent>, Without<HaloMovement>, Without<Dead>, Without<Carried>, Without<Seated>),
    >,
    data: Res<MovementData>,
    clock: Res<SimClock>,
    effects: Res<DamageEffects>,
    mut hits: MessageWriter<HaloHit>,
) {
    let n = crate::game::ticks::halo_ticks(&clock);
    if n == 0 {
        return;
    }
    let kill = kill_speed(&data);
    for (e, v, health, state) in &q {
        if health.current <= 0.0 || state.is_some_and(|s| s.on_ground) || v.0.y >= -kill {
            continue;
        }
        for _ in 0..n {
            hits.write(HaloHit::lethal_fall(&effects, e));
        }
    }
}
