//! The map's switchable lights as drawn (specs/combat_arms/world_dat.md
//! "Light groups"): `game::world` turns each light group a `LightGroup`
//! object names into a Chimera light style (what the group adds to the
//! lightmaps it covers and to vertex-lit corners, with the other groups
//! as at the start) and tells the object its style; `show` writes the
//! groups' state from the object world (the server's, on a client from
//! `net`) to Chimera's `LightStyles`, which relights the lightmaps when it
//! changes. A group shows its full colour while it is on with a colour
//! that is not black at 8 bits, nothing while off.
//!
//! Known shortcuts (docs/tech-debt.md): a style adds the group at its
//! `StartColor`, so `COLOR` with another colour shows the start colour
//! (no stock map sends `COLOR` to a light group); what a group adds is
//! worked out with the other groups as they start (where two groups light
//! one texel past 255 the sum differs); the light grid's fix-ups (how
//! characters and props are lit) keep the map's start.

use bevy::prelude::*;

use super::{
    CaObjects,
    runtime::{ObjectWorld, keys},
};
use crate::map::LightStyles;

/// The light styles the object world's light groups want now (none on a
/// map without switchable groups).
pub fn wanted(world: &ObjectWorld) -> Vec<(u8, bool)> {
    world
        .objects
        .iter()
        .filter_map(|o| {
            let style: u8 = o.props.get(keys::LIGHT_STYLE)?.trim().parse().ok()?;
            Some((style, o.kind.shown()? != Vec3::ZERO))
        })
        .collect()
}

/// Write the groups' state where the map relights from.
pub fn show(objects: Res<CaObjects>, styles: Option<Res<LightStyles>>, mut commands: Commands) {
    if !objects.is_changed() {
        return;
    }
    let want = wanted(&objects.world);
    if want.is_empty() {
        return;
    }
    if styles.is_none_or(|s| s.styles != want) {
        commands.insert_resource(LightStyles { styles: want });
    }
}
