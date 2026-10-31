//! `MapPlugin`: the map's systems (`map::build`, crate `chimera_world`)
//! with what draws the map (`render`) and plays its sounds (`audio`) at
//! the places map's registration keeps for them (`map::MapLayers`,
//! `Present` here), in one fixed order (the order decides how Bevy
//! orders systems that aren't ordered explicitly; resources are entities,
//! so their insert order decides their ids). Then render's part of
//! unloading (`render::unload_map`, a `MapHooks::unload` hook) and of
//! copying nodes (`MapHooks::copied`). The map spawns with
//! `render::MapAssets` (at startup and for `change_map`). The type lives
//! here, beside its `impl Plugin`, and is re-exported as
//! `mashup::map::MapPlugin`, what the game and tests add; the dedicated
//! server adds `map::MapSimPlugin` instead (nothing drawn or played).

use std::sync::{Arc, Mutex};

use bevy::prelude::*;

#[allow(unused_imports)]
use crate::map::*;
use crate::{audio, map, render};

/// Spawns a loaded map: collision always, meshes only when rendering is
/// present (same pattern as the greybox, so maps load in headless tests).
pub struct MapPlugin {
    /// The map to spawn at startup; None starts without one (another map
    /// can be loaded later with `change_map`).
    ///
    /// Taken when the plugin is built, so the pending map is the data's
    /// only owner and `spawn_map` can free its bulk once spawned (a plugin
    /// keeps its own fields for the app's life).
    pub data: Mutex<Option<Arc<MapData>>>,
    pub view: MapDebugView,
}

impl MapPlugin {
    pub fn new(data: MapData) -> Self {
        Self {
            data: Mutex::new(Some(Arc::new(data))),
            view: MapDebugView::Normal,
        }
    }

    /// `new` from a map already shared (`Arc`), without converting it again
    /// or deep-cloning it: the plugin holds the same map, so its bulk stays
    /// with the other owners instead of being freed after the spawn (the
    /// test map cache and an app that reuses a converted map). The map is
    /// immutable once built.
    pub fn from_arc(data: Arc<MapData>) -> Self {
        Self {
            data: Mutex::new(Some(data)),
            view: MapDebugView::Normal,
        }
    }

    /// The map systems and materials without a map loaded yet.
    pub fn empty() -> Self {
        Self {
            data: Mutex::new(None),
            view: MapDebugView::Normal,
        }
    }
}

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        let data = self.data.lock().ok().and_then(|mut d| d.take());
        map::build(app, data.as_ref(), self.view, &Present);
        render::scoped_resources(app);
        let mut hooks = app.world_mut().resource_mut::<MapHooks>();
        hooks.unload.push(render::unload_map);
        hooks.copied.push(render::merge::show_copied_pieces);
        // The images are built on the loading thread (`map::prepare`).
        hooks.prepare.push(render::prepare_images);
    }
}

/// Render and audio at their places in map's registration.
struct Present;

impl MapLayers for Present {
    type Drawer = render::MapAssets<'static>;

    fn ragdolls(&self, app: &mut App) {
        app.add_systems(
            Update,
            (render::bodies::adopt_bodies, render::bodies::pose_ragdolls)
                .chain()
                .after(DriveAnimation)
                .run_if(resource_exists::<render::bodies::PlayModels>),
        );
    }

    fn light_styles(&self, app: &mut App) {
        app.add_plugins(render::light_styles::LightStylesPlugin);
    }

    fn screens_and_sound(&self, app: &mut App) {
        app.add_plugins(render::monitor::MonitorPlugin);
        app.add_plugins(audio::SoundPlugin);
    }

    fn vis_resources(&self, app: &mut App) {
        app.init_resource::<render::vis::NoVis>()
            .init_resource::<render::vis::PortalsOpenAll>()
            .init_resource::<render::vis::Occlusion>()
            .init_resource::<render::sky_occluder::SkyOcclusion>()
            .init_resource::<render::vis::VisStats>();
    }

    fn decal_resources(&self, app: &mut App) {
        app.add_message::<render::decal::ClearDecals>()
            .init_resource::<render::decal::DecalSettings>();
    }

    fn tracer_resources(&self, app: &mut App) {
        app.init_resource::<render::tracer::PendingTracers>();
    }

    fn lighting(&self, app: &mut App) {
        app.insert_resource(GlobalAmbientLight {
            brightness: 600.0,
            // Baked lighting already includes the map's ambient light.
            affects_lightmapped_meshes: false,
            ..default()
        });
    }

    fn panes(&self, app: &mut App) {
        app.add_systems(
            FixedPostUpdate,
            (render::breakables::restore_panes, render::breakables::clip_panes)
                .chain()
                .after(breakables::update_panes),
        );
    }

    // What the logic switches: sprites and dust, lights, prop skins,
    // bodies and sequences.
    fn looks(&self) -> Option<[LayerSystems; 2]> {
        Some([
            (
                render::parts::switch_parts
                    .before(render::glow::glow_visibility)
                    .before(render::dust::update_dust)
                    .before(render::steam::update_steam),
                (render::light_styles::relight, render::light_styles::animate).chain(),
                render::props::apply_prop_looks,
            )
                .into_configs(),
            (
                render::beams::follow_eye
                    .after(render::parts::switch_parts)
                    .before(render::glow::glow_visibility),
                render::beams::animate_beams.after(render::parts::switch_parts),
                render::rope_sway::sway_ropes,
                render::texture_anim::animate_textures,
            )
                .into_configs(),
        ])
    }

    fn gibs(&self) -> Option<[LayerSystems; 2]> {
        Some([
            render::breakables::draw_gibs.into_configs(),
            render::breakables::fade_gibs.into_configs(),
        ])
    }

    fn effects(&self) -> Option<Effects> {
        Some(Effects {
            before: (
                render::sky::attach_sky,
                render::glow::glow_visibility,
                render::dust::update_dust,
                render::steam::update_steam,
            )
                .into_configs(),
            tracers: render::tracer::draw_tracers.into_configs(),
            emitters: (
                render::emitters::update_trails,
                render::emitters::update_smokestacks,
                render::emitters::update_teslas,
                render::emitters::update_strikes,
                render::color_correction::update,
                render::emitters::update_embers,
                render::emitters::update_muzzle_flashes,
                render::fish::swim,
                render::psys::update_systems,
                render::psys::run_started_systems,
            )
                .after(particles::ParticleSet::Step)
                .before(particles::ParticleSet::Draw),
            draw: render::particles::draw_particles
                .run_if(
                    resource_exists::<Assets<Mesh>>
                        .and_then(resource_exists::<Assets<render::particles::ParticleDrawMaterial>>),
                )
                .in_set(particles::ParticleSet::Draw),
            after: (
                render::sky::show_skybox_in_place,
                (
                    render::decal::place_decals,
                    render::decal::drop_brush_decals,
                    render::decal::clear_decals,
                    render::spray::draw_sprays,
                )
                    .chain(),
                (
                    render::bodies::build_late_bodies.before(render::bodies::attach_bodies),
                    render::bodies::attach_bodies,
                    render::bodies::show_local_body,
                    render::bodies::turn_bodies.after(DriveAnimation),
                    render::bodies::pose_bodies.after(DriveAnimation),
                    render::bodies::build_late_held.before(render::bodies::attach_held),
                    render::bodies::attach_held,
                    render::probe_lit::relight,
                    render::loose::attach_shown,
                    render::loose::draw_loose,
                )
                    .run_if(resource_exists::<render::bodies::PlayModels>),
                // View models added with no map spawned (the greybox) get
                // drawing assets of their own first.
                render::view_model::assets_without_map
                    .before(render::view_model::build_late)
                    .after(DriveAnimation),
                (
                    render::view_model::build_late,
                    render::view_model::draw_view_models,
                    render::view_model::view_model_screens,
                )
                    .chain()
                    .after(DriveAnimation)
                    .run_if(resource_exists::<render::view_model::ViewModelAssets>),
                (
                    render::view_model::muzzle_flashes,
                    render::view_model::size_flash_sprites,
                    render::view_model::age_effects,
                    render::shells::eject,
                    render::shells::fly,
                )
                    .chain()
                    .after(DriveAnimation)
                    .after(render::view_model::draw_view_models),
            )
                .into_configs(),
        })
    }

    fn drawing(&self, app: &mut App) {
        // Blended meshes at the same depth in an order of their own, not
        // the one earlier frames left.
        if !app.is_plugin_added::<render::blend_order::BlendOrderPlugin>() {
            app.add_plugins(render::blend_order::BlendOrderPlugin);
        }
        app.add_systems(
            PostUpdate,
            (
                render::shadows::apply_shadow_detail,
                render::shadows::update_prop_shadows,
                render::shadows::update_character_blobs,
            )
                .chain()
                .run_if(resource_exists::<Assets<Mesh>>.and_then(resource_exists::<Assets<Image>>))
                .after(bevy::transform::TransformSystems::Propagate)
                // A rewritten shadow mesh is announced this frame:
                // announced a frame late, the render world (which
                // takes a RENDER_WORLD-only mesh's data) took the
                // rewrite on an earlier announcement and then found the
                // late one empty ("cannot be extracted: already
                // extracted", logged each time a moving prop stopped).
                .before(bevy::asset::AssetEventSystems),
        )
        .add_systems(
            PostUpdate,
            // Props' levels of detail from where the eye is this frame.
            render::lod::switch_lods.after(bevy::transform::TransformSystems::Propagate),
        )
        .add_systems(
            PostUpdate,
            // This frame's visibility of what is on the view-model layer
            // (propagated), before the cameras check what they see.
            render::view_model::view_model_cameras_draw
                .in_set(render::view_model::ViewModelCameraSet)
                .after(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate)
                .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
        )
        .add_systems(Update, render::bodies::uncull_placed_ragdolls)
        // Sprite skins: each drawn copy's frame on its own clock.
        .add_observer(render::skin_frames::start_clock)
        .add_systems(Update, render::skin_frames::advance)
        .add_systems(
            PostUpdate,
            // After propagation, so it sees this frame's (interpolated)
            // eye rather than the last physics tick's; before frusta.
            render::sky::follow_sky_camera
                .after(bevy::transform::TransformSystems::Propagate)
                // This frame's `WaterView` (under water or not).
                .after(render::water::update_water)
                .before(bevy::camera::visibility::VisibilitySystems::UpdateFrusta),
        );
    }

    fn culling(&self) -> Option<[LayerSystems; 2]> {
        Some([
            (
                render::tint::apply_brush_tints,
                (render::tint::fade_protected, render::tint::apply_character_looks).chain(),
                render::merge::sync_merged_brushes,
            )
                .chain(),
            (render::vis::cull, render::vis::fade_windows).chain(),
        ])
    }
}
