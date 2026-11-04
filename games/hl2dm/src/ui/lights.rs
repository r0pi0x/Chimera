//! The respawn light (specs/hl2dm/maps_entities.md 2): when an item or a
//! placed weapon comes back, a dynamic light of colour (255, 192, 64) and a
//! radius of 32 to 64 units flashes for 0.05 s at the model's first
//! attachment, for the models that have one (the AR2's energy ball box, the
//! RPG round, the .357, AR2, shotgun, RPG and S.L.A.M. world models). Items
//! and weapons aren't drawn yet (M10), so the light stands at the pickup's
//! place, a little above it.
//!
//! Known shortcuts: only the server's own client (a listen server or the
//! standalone game) sees it: `PickupBack` isn't sent to network clients;
//! the colour is a sixteenth of the spec's, as the muzzle flash's (it burnt
//! walls out); it fades linearly over its life.

use bevy::prelude::*;
use chimera_sim::weapon::{Weapon, respawn::PickupBack};

use crate::game::{
    pickups::{Item, Pickup},
    weapons::{AR2, MAGNUM, RPG, SHOTGUN},
};

const UNIT: f32 = 0.0254;
/// How long the flash lives (s).
pub const LIFE: f32 = 0.05;

/// Whether a respawned pickup's model has an attachment to flash at.
pub fn has_attachment(item: Option<&Pickup>, weapon: Option<&str>) -> bool {
    if let Some(p) = item {
        return matches!(&p.item, Item::Ammo { ammo, .. } if ammo.ends_with("ar2altfire") || ammo.ends_with("rpg_round"));
    }
    weapon.is_some_and(|w| [MAGNUM, AR2, SHOTGUN, RPG, "hl2dm:weapon_slam"].contains(&w))
}

#[derive(Component)]
pub struct RespawnFlash {
    pub life: f32,
    pub radius: f32,
}

pub fn flash(
    mut back: MessageReader<PickupBack>,
    things: Query<(Option<&Pickup>, Option<&Weapon>)>,
    mut commands: Commands,
    mut dice: Local<u32>,
) {
    for b in back.read() {
        if b.relocated {
            continue;
        }
        let Ok((item, weapon)) = things.get(b.entity) else { continue };
        if !has_attachment(item, weapon.map(|w| w.id)) {
            continue;
        }
        *dice = dice.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let u = (*dice >> 8) as f32 / (1u32 << 24) as f32;
        let radius = (32.0 + 32.0 * u) * UNIT;
        commands.spawn((
            RespawnFlash { life: LIFE, radius },
            PointLight {
                color: Color::srgb(1.0, 0.75, 0.25),
                intensity: 40_000.0 * radius,
                range: radius * 2.0,
                radius: 0.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(b.at + Vec3::Y * 0.2),
        ));
    }
}

pub fn fade(mut flashes: Query<(Entity, &mut RespawnFlash, &mut PointLight)>, time: Res<Time>, mut commands: Commands) {
    for (e, mut f, mut light) in &mut flashes {
        f.life -= time.delta_secs();
        if f.life <= 0.0 {
            commands.entity(e).despawn();
        } else {
            light.range = f.radius * 2.0 * (f.life / LIFE);
        }
    }
}
