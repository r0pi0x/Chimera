//! Halo's effect lights drawn (`game::fx::HaloLight`; effects.md
//! "Lights"): each request a point light the world's shaders take
//! (`map::view_model::DynamicLight`, on the world's and the view model's
//! layers), its colour falling linearly to black over its life (the
//! muzzle light's 1, 2/3, 1/3, 0 over three ticks), then gone.
//!
//! Known shortcuts: the engine's ageing of dynamic lights also shrinks the
//! radius over the life (Halo's keeps its radius).

use bevy::{camera::visibility::RenderLayers, prelude::*};

use crate::game::fx::HaloLight;
use crate::map::view_model::{DynamicLight, VIEW_MODEL_LAYER};

/// A light of ours and its full colour.
#[derive(Component)]
struct Lit(Vec3);

fn spawn(mut requests: MessageReader<HaloLight>, mut commands: Commands) {
    for l in requests.read() {
        commands.spawn((
            Name::new("halo_ce effect light"),
            PointLight {
                color: Color::LinearRgba(LinearRgba::rgb(l.color.x, l.color.y, l.color.z)),
                intensity: 4.0 * std::f32::consts::PI,
                range: l.radius,
                radius: 0.0,
                shadow_maps_enabled: false,
                ..default()
            },
            DynamicLight { radius: l.radius, life: l.seconds.max(1e-3), age: 0.0 },
            RenderLayers::from_layers(&[0, VIEW_MODEL_LAYER]),
            Transform::from_translation(l.at),
            Lit(l.color),
        ));
    }
}

fn fade(mut lights: Query<(Entity, &DynamicLight, &mut PointLight, &Lit)>, mut commands: Commands) {
    for (e, d, mut p, lit) in &mut lights {
        if d.age >= d.life {
            commands.entity(e).despawn();
            continue;
        }
        let k = 1.0 - d.age / d.life;
        p.color = Color::LinearRgba(LinearRgba::rgb(lit.0.x * k, lit.0.y * k, lit.0.z * k));
    }
}

/// The lights' systems.
pub fn plugin(app: &mut App) {
    app.add_message::<HaloLight>().add_systems(Update, (spawn, fade));
}
