//! The engine's presentation (docs/ARCHITECTURE.md, "Crates"): what draws
//! the map and what it simulates (`render`: the drawn phases of spawning,
//! drawn-part requests fulfilled, effects, culling, water, shadows, sky,
//! view models, materials and shaders) and what plays its sounds
//! (`audio`), beside the simulation and above it: it uses the map
//! (`chimera_world`) and the engine's base, and nothing of the simulation
//! uses it. An edit here doesn't rebuild the map, and an edit in the
//! simulation above the map (`mashup`) doesn't rebuild this.
//!
//! `MapPlugin` (`map_plugin`) registers map's systems (`map::build`) with
//! audio's and render's at the places map keeps for them
//! (`map::MapLayers`), in one fixed order. The game crate re-exports
//! `render` and `audio` at their old paths and `MapPlugin` as
//! `mashup::map::MapPlugin`, when built with its `present` feature (the
//! dedicated server is built without this crate: `map::MapSimPlugin`).
//!
//! `assets`: the games' installs as asset sources (`<game>://<path>`,
//! `InstallSourcesPlugin`, added by each app before `DefaultPlugins`).

pub mod assets;
pub mod audio;
mod map_plugin;
pub mod render;

pub use map_plugin::MapPlugin;

/// What drawing the map needs and no map plugin adds: the render
/// materials, particles, shadows and the rest of `render`'s plugins, in
/// one group for every app (the three games' apps listed the same twelve
/// by hand; a missing one draws nothing and fails no test). The app adds
/// it after its game's plugins, where it listed them.
pub struct PresentationPlugins;

impl bevy::app::PluginGroup for PresentationPlugins {
    fn build(self) -> bevy::app::PluginGroupBuilder {
        bevy::app::PluginGroupBuilder::start::<Self>()
            .add(render::cameras::CamerasPlugin)
            .add(render::idle_lights::IdleLightsPlugin)
            .add(render::world_material::WorldMaterialPlugin)
            .add(render::rope_material::RopeMaterialPlugin)
            .add(render::sprite_material::SpriteMaterialPlugin)
            .add(render::prop_material::PropMaterialPlugin)
            .add(render::shadows::ShadowMaterialPlugin)
            .add(render::dust::DustMaterialPlugin)
            .add(render::decal::DecalMaterialPlugin)
            .add(render::spray::SprayMaterialPlugin)
            .add(render::water::WaterMaterialPlugin)
            .add(render::steam::HeatWaveMaterialPlugin)
            .add(render::particles::ParticleMaterialPlugin)
            .add(render::beams::BeamMaterialPlugin)
            .add(render::sky_occluder::SkyOccluderPlugin)
            .add(render::detail::DetailMaterialPlugin)
            .add(audio::clip_asset::SoundAssetPlugin)
    }
}

// `crate::core` and `crate::map` inside, as in the game crate.
use chimera_core::core;
use chimera_world::map;
