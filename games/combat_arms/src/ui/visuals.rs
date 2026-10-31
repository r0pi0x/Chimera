//! Combat Arms' placeholder looks (wave 0), until its models load: the
//! local player's weapon in first person as a box (a long dark one for a
//! gun, a thin light one for a melee weapon) while it has no view model
//! (`game::view_model`: keyed ones without the V2 key, or one still
//! loading: `on_demand`, which this plugin turns on);
//! grenades and impact projectiles in flight without their model as small
//! dark balls (`grenade_balls`; not a flame puff, whose record has no
//! model: nothing is drawn for it until its trail group is specified) and, without the install's effect file,
//! smoke clouds as grey balls of the cloud's size (`smoke_balls`; with it
//! the cloud is the `Smoke_Grenade_01` group's particles, `ui::effects`).
//! Other characters are the shell's capsules (`client::hud`).
//!
//! The seam: Combat Arms' models (players, view models, held weapons)
//! come from its install through its loader (specs/combat_arms/models_ltb.md,
//! characters.md; docs/plans/active/combat-arms.md) as `map` data the presentation already draws
//! (`map::bodies`, `map::view_model`); this module then goes. Choosing a
//! character's body and view-model animation by game data
//! (games/cs_source/src/game/player_anim.rs, view-model events in
//! games/cs_source/src/game/) is a lift candidate for Chimera.

use bevy::prelude::*;

use crate::{
    client::FirstPersonCamera,
    core::LocalPlayer,
    map::{
        ViewModels,
        loose::{LooseAssets, ShownItem},
    },
    weapon::{Hitscan, Inventory, Melee, Weapon, grenade::SmokeCloud},
};

/// The placeholder looks.
pub struct CaVisualsPlugin;

impl Plugin for CaVisualsPlugin {
    fn build(&self, app: &mut App) {
        // The view models load as weapons want them (`view_model::on_demand`).
        app.init_resource::<crate::game::view_model::on_demand::LoadOnDemand>();
        app.add_systems(
            Update,
            (
                box_gun,
                grenade_balls,
                smoke_balls,
            ),
        );
    }
}

/// The box drawn for the local player's weapon, and which weapon it shows.
#[derive(Component)]
pub struct BoxGun(pub Entity);

/// Keep a box in front of the first-person camera for the local player's
/// active weapon: a gun's or a melee weapon's shape, none for anything
/// else (or no weapon, or one the map has a view model for).
#[allow(clippy::too_many_arguments)]
fn box_gun(
    player: Option<Single<&Inventory, With<LocalPlayer>>>,
    kinds: Query<(&Weapon, Has<Hitscan>, Has<Melee>)>,
    view_models: Option<Res<ViewModels>>,
    camera: Option<Single<Entity, With<FirstPersonCamera>>>,
    shown: Query<(Entity, &BoxGun)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let active = player.and_then(|inv| inv.active);
    // A view model loaded since the box went up (`on_demand`) replaces it.
    let modelled = active
        .and_then(|w| kinds.get(w).ok())
        .is_some_and(|(w, ..)| view_models.as_ref().is_some_and(|v| v.get(w.id).is_some()));
    if !modelled && shown.iter().any(|(_, b)| Some(b.0) == active) {
        return;
    }
    for (e, _) in &shown {
        commands.entity(e).despawn();
    }
    let (Some(weapon), Some(camera)) = (active, camera) else {
        return;
    };
    let Ok((_, gun, melee)) = kinds.get(weapon) else {
        return;
    };
    if modelled {
        return;
    }
    // Metres, in the camera's space (-Z ahead, +X right, +Y up).
    let (size, colour, at) = if gun {
        (Vec3::new(0.04, 0.06, 0.45), Color::srgb(0.15, 0.15, 0.17), Vec3::new(0.16, -0.14, -0.45))
    } else if melee {
        (Vec3::new(0.02, 0.04, 0.3), Color::srgb(0.75, 0.75, 0.8), Vec3::new(0.2, -0.15, -0.35))
    } else {
        return;
    };
    let mesh = meshes.add(Cuboid::from_size(size));
    let material = materials.add(StandardMaterial {
        base_color: colour,
        perceptual_roughness: 0.6,
        ..default()
    });
    let entity = commands
        .spawn((
            BoxGun(weapon),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(at),
        ))
        .id();
    commands.entity(*camera).add_child(entity);
}

/// A Combat Arms grenade in flight (its `ShownItem`, on the server's world
/// or a client's) without its model (the projectile record's, `render::
/// loose` draws it; none without the install's V2 key) gets a small dark
/// ball.
fn grenade_balls(
    new: Query<(Entity, &ShownItem), (Added<ShownItem>, Without<Mesh3d>)>,
    models: Option<Res<LooseAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    for (e, item) in &new {
        // A model-less impact projectile (the flame puff) draws no ball:
        // its record has no model (special_weapons.md 4).
        let modelless = crate::game::special::ProjectileParams::is_modelless(&item.0);
        if modelless || !item.0.starts_with(crate::game::GAME) || models.as_ref().is_some_and(|m| m.0.contains_key(&item.0)) {
            continue;
        }
        commands.entity(e).insert((
            Mesh3d(meshes.add(Sphere::new(0.04))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.2, 0.25, 0.15),
                perceptual_roughness: 0.7,
                ..default()
            })),
            Visibility::default(),
        ));
    }
}

/// A smoke cloud is a translucent grey ball of its radius and alpha
/// (when no effect file draws it).
fn smoke_balls(
    mut clouds: Query<(Entity, &SmokeCloud, Option<&mut Transform>, Has<Mesh3d>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    if crate::game::effects::library::library().is_some() {
        return;
    }
    let now = time.elapsed_secs_f64();
    for (e, cloud, transform, drawn) in &mut clouds {
        let t = (now - cloud.started) as f32;
        let r = cloud.radius(t).max(1e-3);
        let pose = Transform::from_translation(cloud.centre).with_scale(Vec3::splat(r));
        if !drawn {
            commands.entity(e).insert((
                Mesh3d(meshes.add(Sphere::new(1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgba(0.7, 0.7, 0.7, 0.8),
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    ..default()
                })),
                pose,
                Visibility::default(),
            ));
        } else if let Some(mut tr) = transform {
            *tr = pose;
        }
    }
}
