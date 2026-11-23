//! Spectators of a minigame (docs/plans/active/ugg-ui.md, "Spectators"):
//! a player who watches instead of playing. `watch` takes a character
//! out of play (`core::Spectating`, no team, dead with no weapons, not
//! solid, out of the minigame), `play` puts it back: the runtime's `join`
//! then takes it in as a new player (in the pre-match lobby it is staged
//! with everyone, `runtime::stage`). Every primitive already leaves a
//! `Spectating` character alone (`runtime::players`, the lobby's hold,
//! respawning, the results, the map vote's count), so a spectator never
//! spawns, scores, stands in the standings or holds a match up.
//!
//! The network's side (who may switch, the cap, joining as one, the
//! lobby's rows) is `chimera_net::spectators`; the client's camera (first
//! person, chase, free) is `chimera_client::client::spectate`'s, which a
//! replicated `Spectating` starts free.
//!
//! Known shortcuts: no death or kill is counted for the switch, and the
//! player's score is kept for its next time in play (as CS:S's team
//! switches keep it).

use avian3d::prelude::ColliderDisabled;
use bevy::prelude::*;

use super::runtime::{InMinigame, Staged};
use crate::core::{Dead, Health, SimClock, Spectating, Team, Velocity};

/// `e` out of play, watching: its protection and weapons gone, dead at no
/// health (since now), not solid, on no team, out of the minigame and the
/// lobby's hold. Ok(false) when it already watches.
pub fn watch(world: &mut World, e: Entity) -> Result<bool, String> {
    if world.get_entity(e).is_err() {
        return Err("no such player".into());
    }
    if world.get::<Spectating>(e).is_some() {
        return Ok(false);
    }
    super::runtime::unprotect(world, e);
    crate::weapon::equip::strip(world, e);
    let now = world.get_resource::<SimClock>().map_or(0.0, |c| c.now);
    let mut ent = world.entity_mut(e);
    if let Some(mut h) = ent.get_mut::<Health>() {
        h.current = 0.0;
    }
    if let Some(mut v) = ent.get_mut::<Velocity>() {
        v.0 = Vec3::ZERO;
    }
    if ent.get::<Dead>().is_none() {
        ent.insert(Dead { since: now });
    }
    ent.remove::<(InMinigame, Staged)>().insert((Spectating, Team(0), ColliderDisabled));
    Ok(true)
}

/// `e` back into play: no longer watching, on no team yet; the runtime's
/// `join` gives it the mode's team and spawns it (or the lobby stages it
/// with everyone). Ok(false) when it wasn't watching.
pub fn play(world: &mut World, e: Entity) -> Result<bool, String> {
    if world.get_entity(e).is_err() {
        return Err("no such player".into());
    }
    if world.entity_mut(e).take::<Spectating>().is_none() {
        return Ok(false);
    }
    world.entity_mut(e).insert(Team(0));
    Ok(true)
}

/// The characters watching now (any order).
pub fn spectators(world: &mut World) -> Vec<Entity> {
    world.query_filtered::<Entity, With<Spectating>>().iter(world).collect()
}
