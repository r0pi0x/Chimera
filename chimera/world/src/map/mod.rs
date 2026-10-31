//! The Map slot: the neutral data any game's map importer produces and the
//! engine spawns, and the map's systems. Units are meters, Y up (README:
//! mounts own each game's units and axes; the engine sees only this).
//!
//! This file holds unloading and changing maps and the loaded map's
//! resources; `registration` registers map's systems (`build`, with what
//! draws and plays the map at fixed places, `MapLayers`; alone as
//! `MapSimPlugin`, the dedicated server's). The rest:
//! `data` (`MapData` and what it holds), `state` (what the logic writes on
//! spawned parts), `spawn` (spawning a map, in phases), `bodies`
//! (character bodies and hitboxes), `props` (placed props after
//! spawning), `materials`, `images`, `collision` and `sky` (built from the
//! data), each re-exported here, plus one submodule per feature.
//!
//! Map's registration (`registration`: `build`, `MapSimPlugin`),
//! `unload_map`/`change_map` (`MapHooks` for the layers above: what they
//! reset at unload, the spawner `change_map` runs; the map-scoped
//! resources, `MapScopedApp::map_scoped`, which `unload_map` removes), the
//! loaded map's resources; re-exports the files below and `core`'s
//! collision types.

use std::{collections::HashMap, sync::Arc};

use avian3d::prelude::*;
use bevy::{asset::RenderAssetUsages, image::ImageSampler, prelude::*};
// Not `bevy::render::render_resource`: its re-exports need the render crates.
use wgpu_types::{Extent3d, TextureDimension, TextureFormat};

use crate::core::{MovingSolid, SpawnPoint, Team};
// Collision-world types live in `core` (the greybox map uses them too).
pub use crate::core::{
    BrushTreeNode, MapBrush, MapBrushCollider, MapBrushTree, MapBrushes, MapTerrain, MapTerrainCollider, MapWater, MapWaterVolume, PropSurface,
};

pub mod anim;
pub mod decal;
pub mod emitters;
pub mod extras;
pub use extras::{MapExtras, SourceEffects};
pub mod entities;
pub mod hazards;
pub use hazards::{SpawnHazardAppExt, SpawnHazards};
pub mod fish;
pub mod fire;
pub use entities::{BrushSurface, MapBrushEntity, MapEntities, MapEntity, MapHull};
pub mod beams;
pub mod breakables;
pub mod buoyancy;
pub mod color_correction;
pub mod contact_filter;
pub mod controllers;
pub mod copies;
pub use breakables::{
    BreakProp, BrushPanes, FallingPane, GlassImpact, GlassShatter, MapBreak, MapBreakPiece, SpawnGibs,
};
pub mod hud;
pub mod impact;
pub mod interp;
pub mod loading;
pub mod lod;
pub mod logic_view;
pub mod loose;
pub mod material_fx;
pub use material_fx::{DetailMode, MapSelfIllum, MapTextureAnim, MapUvTransform, parameter_gamma_to_linear};
pub mod bot_profiles;
pub mod monitor;
pub mod nav;
pub mod nav_gen;
pub mod particles;
pub mod physics_clip;
pub mod prepare;
pub mod placed_ragdoll;
pub mod prop_physics;
pub mod psys;
pub mod radio;
pub mod ragdoll;
pub mod tracer;
pub use ragdoll::{MapCollisionHooks, MapRagdoll, MapRagdollBody, MapRagdollJoint, Ragdoll, RagdollBody, RagdollShot};
pub mod rope_sway;
pub mod sound;
pub mod steam;
pub use sound::{
    GameSound, MapSoundClip, MapSoundEntry, MapSounds, MapSurface, PlaySound, PredictedSound, SoundControl, SoundKey,
    SoundLevel, StartSound, StepRemap,
};
pub mod shells;
pub mod parallel;
pub mod sources;
pub use sources::{KitLoad, MapDressing, MapLoadOptions, MapLoader, MapSources, SpawnTeams, SurfaceNames, SurfaceTable};
pub mod surface_color;
pub mod view_model;
pub mod vis;
pub mod water;
mod data;
pub use data::*;
// Explicit: Bevy's prelude has a `LightProbe` too.
pub use data::LightProbe;
mod state;
pub use state::*;
mod bodies;
pub use bodies::*;
mod props;
pub use props::*;
mod collision;
use collision::*;
mod sky;
pub use sky::*;
mod registration;
pub use registration::{Effects, LayerSystems, MapLayers, MapSimPlugin, Undrawn, build};
mod spawn;
pub use spawn::{Draws, DrawnResource, MapDrawer, MapSpawn, PropSpawn};
pub use spawn::{merged_world, spawn_map};
pub use view_model::{
    DynamicLight, EffectSettings, GameFov, GameViewModelFovs, LateViewModelTextures, MapAttachment, MapViewModel, ViewAnimator, ViewModelAnchor,
    ViewModelCamera, ViewModelEvent, ViewModelEventKind, ViewModelOffset, ViewModelScreen, ViewModelSettings,
    ViewModelSource, ViewModels, add_view_model,
};

/// The loaded map's light field, when the game provides one.
#[derive(Resource, Clone)]
pub struct LightField(pub MapLightField);

/// The loaded map's screen overlay pictures (env_screenoverlay), by
/// lower-case material name; the client draws the one the logic shows
/// (`logic::ScreenFades::overlay`).
#[derive(Resource, Clone, Debug, Default)]
pub struct ScreenOverlayImages(pub HashMap<String, Handle<Image>>);

/// Debug renders, used to compare against reference screenshots.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum MapDebugView {
    #[default]
    Normal,
    /// White surfaces: only baked lighting (lightmaps, prop probes),
    /// multiplied by `scale` (e.g. 0.25 so overbright light fits in 0..1).
    Lighting { scale: f32 },
    /// Textures only, unlit.
    Albedo,
}

#[derive(Resource)]
pub struct PendingMap(Arc<MapData>, MapDebugView, prepare::Prepared);

/// Characters below this height (meters) have fallen out of the map.
#[derive(Resource, Clone, Copy, Debug)]
pub struct KillHeight(pub f32);

/// Falling out of the map kills; characters that can't die (god mode, no
/// health) go back to a spawn point instead. A noclipping character
/// (`core::Noclip`) flies where it likes: below the map too.
#[allow(clippy::type_complexity)]
pub fn fall_out_of_map(
    kill: Option<Res<KillHeight>>,
    mut characters: Query<
        (
            Entity,
            &mut Transform,
            Option<&mut crate::core::Velocity>,
            Option<&crate::core::Health>,
            Has<crate::core::God>,
            Option<&Team>,
        ),
        (With<crate::core::Intent>, Without<crate::core::Noclip>),
    >,
    spawns: Query<(&Transform, &crate::core::SpawnPoint), Without<crate::core::Intent>>,
    mut damage: MessageWriter<crate::core::Damage>,
) {
    let Some(kill) = kill else { return };
    for (e, mut at, velocity, health, god, team) in &mut characters {
        if at.translation.y >= kill.0 {
            continue;
        }
        match health {
            Some(h) if !god => {
                if h.current > 0.0 {
                    damage.write(crate::core::Damage {
                        force: bevy::math::Vec3::ZERO,
                        target: e,
                        attacker: None,
                        amount: h.current.max(1.0) * 1000.0,
                        point: at.translation,
                        dir: Vec3::NEG_Y,
                        hitgroup: crate::core::Hitgroup::Generic,
                        kind: crate::core::DamageKind::Generic,
                        weapon: None,
                    });
                }
            }
            _ => {
                let spawn = spawns
                    .iter()
                    .find(|(_, s)| s.team.is_some() && s.team == team.copied())
                    .or_else(|| spawns.iter().next());
                if let Some((s, _)) = spawn {
                    at.translation = s.translation;
                    if let Some(mut v) = velocity {
                        v.0 = Vec3::ZERO;
                    }
                }
            }
        }
    }
}

/// The map's playable area (see `MapData::playable`).
#[derive(Resource)]
pub struct PlayableArea(pub (Vec3, Vec3));

/// The loaded map's presentation settings, for cameras to follow.
#[derive(Resource, Clone, Debug)]
pub struct ActiveMapLook(pub MapLook);

/// The loaded map's name (`MapData::name`), for reports; the client sets
/// it (from `--map` and the `map` command) so the simulation's entities
/// stay as they were (bot seeds come from entity ids).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LoadedMapName(pub String);

/// The loaded map's name (`MapData::name`) and file hash
/// (`MapData::file_hash`), set when it spawns, removed when it unloads.
/// The network handshake compares them with the server's.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct MapFile {
    pub name: String,
    pub hash: Option<[u8; 32]>,
}

/// Marks every entity belonging to the loaded map.
#[derive(Component)]
pub struct MapPart;

/// Height of the capsule center above the feet, so spawns start standing.
pub const SPAWN_LIFT: f32 = 1.0;

/// Remove the loaded map: its entities, its resources and the sky on
/// cameras. Characters stay.
pub fn unload_map(world: &mut World) {
    let parts: Vec<Entity> = world.query_filtered::<Entity, With<MapPart>>().iter(world).collect();
    for e in parts {
        if let Ok(e) = world.get_entity_mut(e) {
            e.despawn();
        }
    }
    let cams: Vec<Entity> = world
        .query_filtered::<Entity, With<bevy::light::Skybox>>()
        .iter(world)
        .collect();
    for c in cams {
        world.entity_mut(c).remove::<bevy::light::Skybox>();
    }
    // The map-scoped resources (`MapScopedApp::map_scoped`).
    let scoped: Vec<fn(&mut World)> = world
        .get_resource::<MapHooks>()
        .map(|h| h.scoped.iter().map(|r| r.remove).collect())
        .unwrap_or_default();
    for remove in scoped {
        remove(world);
    }
    world.insert_resource(MonitorCamera::default());
    if let Some(mut p) = world.get_resource_mut::<particles::Particles>() {
        p.groups.clear();
    }
    breakables::unload(world);
    let animated: Vec<Entity> = world
        .query_filtered::<Entity, With<anim::Animator>>()
        .iter(world)
        .collect();
    for e in animated {
        world.entity_mut(e).remove::<anim::Animator>();
    }
    let modelled: Vec<Entity> = world.query_filtered::<Entity, With<BodyModel>>().iter(world).collect();
    for e in modelled {
        world.entity_mut(e).remove::<(BodyModel, crate::core::Hitboxes)>();
    }
    world.insert_resource(Gravity::default());
    let hooks = world.get_resource::<MapHooks>().map(|h| h.unload.clone()).unwrap_or_default();
    for hook in hooks {
        hook(world);
    }
}

/// What the layers above the map add to loading and unloading maps.
#[derive(Resource, Default)]
pub struct MapHooks {
    /// What they reset when a map is unloaded: `unload_map` runs them
    /// last, in the order they were added (`audio` stops the
    /// soundscape and the long-lived sounds).
    pub unload: Vec<fn(&mut World)>,
    /// Spawns the pending map (`spawn_map` with the drawer
    /// `build` picks: `MapLayers::Drawer`), for `change_map`.
    pub spawn: Option<fn(&mut World)>,
    /// The resources that belong to the loaded map (`MapScopedApp::
    /// map_scoped`): `unload_map` removes each.
    pub scoped: Vec<MapScopedResource>,
    /// What they do to a node copied at run time (`copies::copy_node`),
    /// once it is made, in the order they were added (`render`
    /// draws it by its own meshes, never merged).
    pub copied: Vec<fn(&mut World, Entity)>,
    /// Spawn work a load can do ahead, off the main thread
    /// (`prepare::preparation`): each makes a job from the world as the
    /// load starts.
    pub prepare: Vec<fn(&World) -> prepare::PrepareJob>,
}

/// A resource that belongs to the loaded map: inserted when it spawns,
/// removed by `unload_map`.
#[derive(Clone, Copy)]
pub struct MapScopedResource {
    pub id: std::any::TypeId,
    pub name: &'static str,
    pub remove: fn(&mut World),
}

/// Registers the resources that belong to the loaded map, once, where the
/// plugin that owns them is built: `unload_map` removes every registered
/// one, so a resource a spawn phase inserts can't outlive its map. One
/// the next map doesn't replace and nothing registers is a leak: the
/// unload test (`chimera_present`'s `map_unload`) fails on any.
pub trait MapScopedApp {
    fn map_scoped<T: Resource>(&mut self) -> &mut Self;
}

impl MapScopedApp for App {
    fn map_scoped<T: Resource>(&mut self) -> &mut Self {
        self.init_resource::<MapHooks>();
        let id = std::any::TypeId::of::<T>();
        let mut hooks = self.world_mut().resource_mut::<MapHooks>();
        if !hooks.scoped.iter().any(|r| r.id == id) {
            hooks.scoped.push(MapScopedResource {
                id,
                name: std::any::type_name::<T>(),
                remove: |world| {
                    world.remove_resource::<T>();
                },
            });
        }
        self
    }
}

/// Replace the loaded map with `data` (spawned now, as at startup).
pub fn change_map(world: &mut World, data: MapData, view: MapDebugView) {
    change_map_with(world, data, prepare::Prepared::default(), view);
}

/// `change_map` with the parts of its spawn already done
/// (`prepare::change_map_prepared`).
fn change_map_with(world: &mut World, data: MapData, prepared: prepare::Prepared, view: MapDebugView) {
    unload_map(world);
    let data = Arc::new(data);
    world.insert_resource(ActiveMapLook(data.look.clone()));
    world.insert_resource(PendingMap(data, view, prepared));
    match world.get_resource::<MapHooks>().and_then(|h| h.spawn) {
        Some(spawn) => spawn(world),
        None => error!("spawning the map: no map spawner (MapPlugin or MapSimPlugin missing)"),
    }
}

/// A brush entity node's (mover, breakable) or a moving prop's mesh
/// bounds in its own space: its `VisClusters` follow it as it moves
/// (`tag_moved_brush_entities`).
#[derive(Component, Clone, Copy, Debug)]
pub struct BrushEntityBounds {
    pub min: Vec3,
    pub max: Vec3,
}

/// Brush entities that moved (and every one on its first frame): the
/// clusters their world bounds touch, for visibility culling. A door or
/// breakable is then hidden like the world around it.
pub fn tag_moved_brush_entities(
    vis: Option<Res<vis::ActiveVisibility>>,
    mut nodes: Query<(&GlobalTransform, &BrushEntityBounds, &mut VisClusters), Changed<GlobalTransform>>,
) {
    let Some(vis) = vis else { return };
    let mut clusters = Vec::new();
    for (at, b, mut tag) in &mut nodes {
        let aabb = bevy::camera::primitives::Aabb::from_min_max(b.min, b.max);
        let centre = at.transform_point(aabb.center.into());
        let m = at.affine().matrix3;
        let half = Vec3::from(aabb.half_extents);
        let extent = Vec3::new(
            m.row(0).abs().dot(half.into()),
            m.row(1).abs().dot(half.into()),
            m.row(2).abs().dot(half.into()),
        );
        clusters.clear();
        vis.0.clusters_in_box(centre - extent, centre + extent, &mut clusters);
        clusters.sort_unstable();
        clusters.dedup();
        // A box in solid only (no clusters) would be drawn always; keep
        // what it had.
        if !clusters.is_empty() && *tag.clusters != clusters[..] {
            tag.clusters = clusters.clone().into_boxed_slice();
        }
    }
}

/// A glow sprite: drawn over everything, faded by how much of its
/// occlusion proxy is visible.
#[derive(Component)]
pub struct GlowSprite {
    pub color: Vec4,
    pub proxy: f32,
}
