//! The respawn light (specs/hl2dm/maps_entities.md 2): when an item or a
//! placed weapon comes back, a dynamic light of colour (255, 192, 64) and a
//! radius of 32 to 64 units flashes for 0.05 s at the model's first
//! attachment, for the models that have one (the AR2's energy ball box, the
//! RPG round, the .357, AR2, shotgun, RPG and S.L.A.M. world models). Items
//! and weapons aren't drawn yet (M10), so the light stands at the pickup's
//! place, a little above it.
//!
//! The flash itself is the game's (`game::set_piece_fx::RespawnLight`,
//! made from `PickupBack` on the host and sent to network clients).
//!
//! Known shortcuts: the colour is a sixteenth of the spec's, as the muzzle flash's (it burnt
//! walls out); it fades linearly over its life.

use bevy::prelude::*;

use crate::game::set_piece_fx::RespawnLight;

/// How long the flash lives (s).
pub const LIFE: f32 = 0.05;

#[derive(Component)]
pub struct RespawnFlash {
    pub life: f32,
    pub radius: f32,
}

pub fn flash(mut lights: MessageReader<RespawnLight>, mut commands: Commands) {
    for l in lights.read() {
        commands.spawn((
            RespawnFlash { life: LIFE, radius: l.radius },
            PointLight {
                color: Color::srgb(1.0, 0.75, 0.25),
                intensity: 40_000.0 * l.radius,
                range: l.radius * 2.0,
                radius: 0.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(l.at),
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
