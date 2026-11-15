//! The scenario's teleporter pairs as the map's teleporters
//! (specs/halo_ce/scenario.md, "Netgame flags", "Teleporter pairs";
//! multiplayer_rules.md 6.8; movement.md, "Teleporters"), in the engine's
//! game-neutral form (`map::teleport::MapTeleporters`, the map's typed
//! extra), so every game's characters on a Halo map use them.
//!
//! Each teleporter source flag with a target (`Scenario::teleporter_pairs`:
//! the first target flag in block order with the source's team index)
//! is a pad: its source at the source flag, its destination at the
//! target's position, both through `space::point` (feet), the yaws through
//! `space::yaw` (the look turns by target facing minus source facing, the
//! spec's heading rule). The rule's numbers: a unit within 0.5 WU (3D) of a
//! source is taken (`TELEPORT_RADIUS`), the source it arrives beside stays
//! disarmed until it is more than 1 WU away (`TELEPORT_REARM`), the
//! velocity is kept in world axes (movement.md), and a player refused by
//! a unit on the exit for 90 ticks (`TELEFRAG_TICKS`) kills that unit.
//! A source without a target (none on the disc) is no pad.
//!
//! Known shortcuts: "a unit within the exit" is read as a character's feet
//! within the source radius of the target (the spec gives no exit size);
//! the blocked message, the "teleporter activate" sound for the
//! teleported player and the pads' shimmer are not played (the engine's
//! teleporter has no hook for them yet).

use blam_formats::scenario::{NetgameFlag, Scenario, netgame};

use super::space;
use crate::map::{
    MapData,
    teleport::{MapTeleporters, Teleporter},
};

/// Ticks a refused player's count must reach to kill the blocker
/// (multiplayer_rules.md, TELEFRAG_TICKS).
pub const TELEFRAG_TICKS: f32 = 90.0;

/// The netgame flags' teleporters in the engine's form (module doc).
pub fn teleporters(flags: &[NetgameFlag]) -> MapTeleporters {
    let pads = netgame::teleporter_pairs(flags)
        .into_iter()
        .filter_map(|pair| {
            let source = flags.get(pair.source)?;
            let target = flags.get(pair.target?)?;
            Some(Teleporter {
                source: space::point(source.position),
                source_yaw: space::yaw(source.facing),
                dest: space::point(target.position),
                dest_yaw: space::yaw(target.facing),
            })
        })
        .collect();
    MapTeleporters {
        pads,
        radius: netgame::TELEPORT_RADIUS * space::METERS_PER_UNIT,
        rearm: netgame::TELEPORT_REARM * space::METERS_PER_UNIT,
        keep_velocity: true,
        telefrag: Some(TELEFRAG_TICKS * crate::game::TICK_INTERVAL as f32),
    }
}

/// `map`'s teleporters from the scenario (none: no extra).
pub fn add(scenario: &Scenario, map: &mut MapData) {
    let t = teleporters(&scenario.netgame_flags);
    if !t.pads.is_empty() {
        map.extras.insert(t);
    }
}
