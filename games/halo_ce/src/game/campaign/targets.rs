//! The campaign's characters as the sandbox's practice targets
//! (`chimera_sim::minigame::targets`, `TargetCharacter`): `halo_ce:elite`,
//! `halo_ce:grunt` and `halo_ce:jackal`, so `target add halo_ce:elite 5`
//! places an Elite. The party's roster registers them right after Halo's
//! Spartan (apps/party_roster/src/target_bodies.rs: the Spartan stays the
//! body a Halo gun alone gives).
//!
//! - **Which guns** (`holds`): those data/campaign.toml gives the kind a
//!   weight for (the Elite the plasma rifle and needler, the Grunt the
//!   needler and plasma pistol, the Jackal the plasma pistol), and only
//!   once its unit is loaded here (`UnitsOnDemand::ready`): until then, or
//!   without its set, it holds none, so a target of it is refused ("can't
//!   hold") and the listing shows it with no guns.
//! - **Its units** (`sandbox_wants`): while the sandbox plays, every
//!   kind's unit is asked for (in the background, once), so the bodies are
//!   offered a few seconds after the sandbox starts.
//! - **Wearing it** (`wear`): the target is forced to that kind
//!   (`ForcedBody`, and its `BodyKind` at once), as `halo_ce:body` forces
//!   a player's; Halo's Spartan takes the force off and sets the kind back
//!   to the Spartan (`wear_spartan`).
//! - **Its fresh numbers** (`vitals`, in `TargetVitals`' units, which
//!   read `core::Health` and `weapon::Armor`): health 1.0 (Halo's health
//!   is the body's fraction of its own maximum, whatever that is: the
//!   Grunt's 40 points are its 1.0), and a full shield (1 over the party
//!   balance's incoming scale, `cross_game::shield_unit`) when the kind's
//!   shield covers its body (the Elite's) or is an arm shield's disc (the
//!   Jackal's), else 0 (the Grunt has none).
//! - **Its second bar now** (`second_bar`): the Jackal's disc is not the
//!   armour value other games' hits and the armour bar see
//!   (`vitality::update`), so the body reports it itself: its
//!   `Vitality::shield` in the same unit; the others' is their `Armor`.

use bevy::prelude::*;
use chimera_sim::minigame::targets::TargetCharacter;

use super::{BodyKind, BodyTables, CampaignData, ForcedBody, UnitsOnDemand};

/// A kind's target id and display name.
pub fn target_id(kind: BodyKind) -> (&'static str, &'static str) {
    match kind {
        BodyKind::Spartan => ("halo_ce:spartan", "Spartan"),
        BodyKind::Elite => ("halo_ce:elite", "Elite"),
        BodyKind::Grunt => ("halo_ce:grunt", "Grunt"),
        BodyKind::Jackal => ("halo_ce:jackal", "Jackal"),
        BodyKind::Marine => ("halo_ce:marine", "Marine"),
        BodyKind::Hunter => ("halo_ce:hunter", "Hunter"),
    }
}

/// The kind of target id `id` (`halo_ce:elite`).
pub fn kind_of(id: &str) -> Option<BodyKind> {
    BodyKind::ALL.into_iter().find(|k| target_id(*k).0.eq_ignore_ascii_case(id))
}

/// Whether a body of target id `id` may hold `gun` here (module doc).
pub fn holds(world: &World, id: &str, gun: &str) -> bool {
    let Some(kind) = kind_of(id) else { return false };
    let ready = world.get_resource::<UnitsOnDemand>().is_some_and(|u| u.ready(kind));
    ready && world.get_resource::<CampaignData>().is_some_and(|d| d.weights(gun).iter().any(|(k, _)| *k == kind))
}

/// Target `e` wears the body of target id `id` (module doc).
pub fn wear(world: &mut World, e: Entity, id: &str) {
    let Some(kind) = kind_of(id) else { return };
    world.get_resource_or_init::<UnitsOnDemand>().wanted.insert(kind);
    let Ok(mut ent) = world.get_entity_mut(e) else { return };
    if ent.get::<ForcedBody>() != Some(&ForcedBody(kind)) {
        ent.insert(ForcedBody(kind));
    }
    if ent.get::<BodyKind>() != Some(&kind) {
        ent.insert(kind);
    }
}

/// Target `e` wears Halo's Spartan: a campaign body forced on it goes.
pub fn wear_spartan(world: &mut World, e: Entity, _: &str) {
    if let Ok(mut ent) = world.get_entity_mut(e) {
        if ent.contains::<ForcedBody>() {
            ent.remove::<ForcedBody>();
        }
        // Kept on, the Spartan (`campaign::choose`).
        if ent.get::<BodyKind>().is_some_and(|k| *k != BodyKind::Spartan) {
            ent.insert(BodyKind::Spartan);
        }
    }
}

/// A fresh body of target id `id`: health and shield (module doc).
pub fn vitals(world: &World, id: &str) -> (f32, f32) {
    let scale = crate::game::cross_game::incoming_in(world, id);
    let full = crate::game::cross_game::shield_unit(1.0, scale);
    let fresh;
    let spartan = match world.get_resource::<crate::game::vitality::Spartan>() {
        Some(s) => s,
        None => {
            fresh = crate::game::vitality::Spartan::load();
            &fresh
        }
    };
    let table = super::vitality_of(spartan, world.get_resource::<BodyTables>(), kind_of(id).as_ref());
    let disc = kind_of(id).is_some_and(|k| has_disc(world, k));
    (1.0, if table.shield_covers_body() || disc { full } else { 0.0 })
}

fn has_disc(world: &World, kind: BodyKind) -> bool {
    let fresh;
    let data = match world.get_resource::<CampaignData>() {
        Some(d) => d,
        None => {
            fresh = CampaignData::builtin();
            &fresh
        }
    };
    data.body(kind).is_some_and(|d| d.shield_disc.is_some())
}

/// Target `e`'s second bar now when it is an arm shield's disc (module
/// doc): its shield fraction in `Armor`'s unit; None for any other body.
pub fn second_bar(world: &World, e: Entity) -> Option<f32> {
    let kind = *world.get::<BodyKind>(e)?;
    if !has_disc(world, kind) {
        return None;
    }
    let v = world.get::<crate::game::vitality::Vitality>(e)?;
    let scale = crate::game::cross_game::incoming_in(world, target_id(kind).0);
    Some(crate::game::cross_game::shield_unit(v.shield.max(0.0), scale))
}

/// The campaign kinds data/campaign.toml has a row for, as target bodies
/// (the party kit registers them after the Spartan).
pub fn characters() -> Vec<TargetCharacter> {
    let data = CampaignData::builtin();
    BodyKind::ALL
        .into_iter()
        .filter(|k| *k != BodyKind::Spartan && data.body(*k).is_some())
        .map(|k| {
            let (id, name) = target_id(k);
            TargetCharacter { id, name, game: crate::game::GAME, holds, wear, vitals, second_bar: Some(second_bar) }
        })
        .collect()
}

/// While the sandbox plays, every kind's unit is wanted (module doc).
pub fn sandbox_wants(
    hold: Option<Res<chimera_sim::minigame::rung_command::HoldRungs>>,
    active: Option<Res<chimera_sim::minigame::ActiveMinigame>>,
    data: Res<CampaignData>,
    mut units: ResMut<UnitsOnDemand>,
) {
    if !hold.as_deref().zip(active.as_deref()).is_some_and(|(h, a)| a.def.id == h.0) {
        return;
    }
    for k in BodyKind::ALL.into_iter().filter(|k| *k != BodyKind::Spartan && data.body(*k).is_some()) {
        if !units.wanted.contains(&k) {
            units.wanted.insert(k);
        }
    }
}
