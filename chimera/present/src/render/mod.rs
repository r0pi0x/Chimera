//! Presentation of the map: what draws the parts `map` spawns and
//! simulates. Nothing in the simulation uses it (`checks/tests/it/architecture.rs`);
//! `map_plugin` builds it into `map::MapPlugin`; only `map_plugin`,
//! `client` and games' `ui/` use it.
//!
//! The drawn phases of spawning a map (`spawn`, through `map::MapDrawer`);
//! what the simulation makes in play, drawn from its drawn-part requests
//! (`bodies`, `loose`, `breakables`, `props`, `merge`: docs/ARCHITECTURE.md,
//! "Drawn-part requests"); the effects, decals, water, shadows, culling,
//! sky, light styles, monitor screens and view models; what every
//! camera's frame costs (`cameras`); the order blended
//! meshes draw in (`blend_order`); the materials
//! (`world_material`, `prop_material`, `sprite_material`, `rope_material`,
//! `probe_lit`, `fog`, the `materials` and `images` helpers) and their
//! shaders. `unload_map` is render's part of `map::unload_map`.

use bevy::prelude::*;

pub mod beams;
pub mod blend_order;
pub mod cameras;
pub mod bodies;
pub mod breakables;
pub mod color_correction;
pub mod decal;
pub mod detail;
pub mod emitters;
pub mod fog;
pub mod dust;
pub mod fish;
pub mod glow;
pub mod idle_lights;
mod images;
pub use images::*;
pub mod light_styles;
pub mod lod;
pub mod merge;
mod materials;
pub use materials::*;
pub mod loose;
pub mod monitor;
pub mod particles;
pub mod parts;
pub mod probe_lit;
pub mod prop_material;
pub mod props;
pub mod psys;
pub mod rope_material;
pub mod rope_sway;
pub mod shadows;
pub mod shells;
pub mod skin_frames;
pub mod sky;
pub mod sky_occluder;
pub mod spray;
pub mod sprite_material;
pub mod steam;
mod spawn;
pub mod texture_anim;
pub mod tint;
pub mod tracer;
pub mod view_model;
pub mod vis;
pub mod water;
pub mod world_material;
pub use spawn::{Drawn, ImageParts, MapAssets, prepare_images};

/// What `map::unload_map` removes of render's besides its map-scoped
/// resources (`scoped_resources`): the entities it made (a `map::MapHooks::unload` hook, added by
/// `map_plugin`).
pub fn unload_map(world: &mut World) {
    let drawn: Vec<Entity> = world
        .query_filtered::<Entity, With<bodies::CharacterBody>>()
        .iter(world)
        .collect();
    for b in drawn {
        world.entity_mut(b).despawn();
    }
    view_model::unload(world);
    shells::unload(world);
}

/// Render's resources that belong to the loaded map: `map::unload_map`
/// removes them (`map::MapScopedApp::map_scoped`, registered by
/// `map_plugin`).
pub fn scoped_resources(app: &mut App) {
    use crate::map::MapScopedApp;
    app.map_scoped::<light_styles::StyledLightmaps>()
        .map_scoped::<decal::DecalSurfaces>()
        .map_scoped::<decal::DecalAssets>()
        .map_scoped::<decal::ImpactDecals>()
        .map_scoped::<psys::ParticleSystemDefs>()
        .map_scoped::<monitor::MonitorScreen>()
        .map_scoped::<water::MapWaterRender>()
        .map_scoped::<sky::MapSkybox>()
        .map_scoped::<sky::SkyCameraInfo>()
        .map_scoped::<sky::ActiveMapHas3dSky>()
        .map_scoped::<sky::SkyVis>()
        .map_scoped::<shadows::ShadowState>()
        .map_scoped::<shadows::ShadowSource>()
        .map_scoped::<particles::ParticleAssets>()
        .map_scoped::<fog::SceneFog>()
        .map_scoped::<bodies::PlayModels>()
        .map_scoped::<bodies::HeldAssets>()
        .map_scoped::<bodies::HeldLateBase>()
        .map_scoped::<texture_anim::TextureAnims>()
        .map_scoped::<probe_lit::ProbeLightScale>();
}
