//! What every camera's frame costs the render world, cut where nothing
//! shows it (`CamerasPlugin`): mesh instances batched on the CPU for each
//! view instead of Bevy's GPU preprocessing (`NoIndirectDrawing` on every
//! camera), and only the last camera drawing into a target copies the
//! frame out to it (`skip_covered_outputs`). While nothing in the world is
//! clustered (no point, spot or rect light, light probe or clustered
//! decal), every 3D camera clusters into one cluster instead of Bevy's
//! default grid (`cluster_by_lights`): the grid's thousands of empty
//! clusters were assigned, extracted and uploaded every frame for every
//! camera (8% of the render world, docs/performance.md, "Render
//! performance"). The cluster count is a uniform, not a shader setting, so
//! the switch back when the first muzzle flash lights up changes no
//! pipeline; and one cluster draws any lights correctly, only slower, so a
//! light spawned after the switch in its frame is still drawn.
//!
//! GPU preprocessing (Bevy's default where compute shaders run) builds
//! each view's instance data and indirect draw parameters on the GPU:
//! per view and per frame, bind groups made again and several compute
//! passes (bin unpacking, uniform allocation, culling, indirect
//! parameters) over the mesh instances. A party match draws four cameras
//! (the 3D sky, the world, the view model, the UI); batching each one's
//! visible meshes on the CPU instead cut the render world's instructions
//! a frame by 44-57% with a few hundred meshes drawn (docs/performance.md,
//! "Party frame-rate pass", second pass). For A/B:
//! `CHIMERA_GPU_PREPROCESS=1` keeps Bevy's way, `CHIMERA_SKIP_OUTPUTS=0`
//! every camera's copy.
//!
//! Every camera that draws into a window or image renders into the
//! target's intermediate ("main") textures, shared by all the cameras on
//! that target with the same HDR and MSAA settings, and then by default
//! copies them out to the target (Bevy's `upscaling` pass, a full-screen
//! blit). A party match draws four (the 3D sky, the world, the view model,
//! the UI over them): the first three's copies are overwritten by the next
//! camera's. `CameraOutputMode::Skip` leaves the copy to the last one,
//! which then replaces what the target holds (`BlendState::REPLACE`):
//! Bevy blends every copy but a target's bottom camera's over it, and with
//! the bottom copies skipped that blended the frame over the window's
//! clear colour wherever its alpha is below one (de_piranesi's sky: grey,
//! the sky showing only at the edges of alpha-to-coverage foliage).
//!
//! Known shortcuts: CPU batching was measured on stock maps (a few hundred
//! meshes drawn); on a map with thousands of meshes in view the GPU path
//! may win again, unmeasured (`refcmp bench` with and without
//! `CHIMERA_GPU_PREPROCESS=1`). Why it's OK for now: the party plays stock
//! maps, and the switch is one variable.

use bevy::{
    camera::{CameraOutputMode, Hdr, NormalizedRenderTarget, RenderTarget},
    prelude::*,
    render::{render_resource::BlendState, view::NoIndirectDrawing},
    window::PrimaryWindow,
};

/// Every camera batched on the CPU (unless `CHIMERA_GPU_PREPROCESS=1`) and
/// only the last one on each target copying out to it.
pub struct CamerasPlugin;

impl Plugin for CamerasPlugin {
    fn build(&self, app: &mut App) {
        if !gpu_preprocessing() {
            // At spawn (Bevy reads it then; added later it is unspecified).
            app.register_required_components::<Camera, NoIndirectDrawing>();
        }
        app.add_systems(
            PostUpdate,
            cluster_by_lights.before(bevy::light::SimulationLightSystems::AssignLightsToClusters),
        );
        // After everything that turns cameras on or off (PostUpdate's view
        // model and sky cameras), before extraction.
        if chimera_core::env::var("CHIMERA_SKIP_OUTPUTS").is_ok_and(|v| v == "0") {
            return;
        }
        app.add_systems(Last, skip_covered_outputs);
    }
}

/// One light cluster per 3D camera while nothing is clustered, Bevy's
/// default grid while something is (see the module doc). Cameras with a
/// clustering of their own (any but one or the default) are left alone.
#[allow(clippy::type_complexity)]
pub fn cluster_by_lights(
    clustered: Query<
        (),
        Or<(
            With<PointLight>,
            With<SpotLight>,
            With<bevy::light::RectLight>,
            With<bevy::light::LightProbe>,
            With<bevy::light::ClusteredDecal>,
        )>,
    >,
    cameras: Query<(Entity, Option<&bevy::light::cluster::ClusterConfig>), With<Camera3d>>,
    mut commands: Commands,
) {
    use bevy::light::cluster::ClusterConfig;
    let single = clustered.is_empty();
    for (camera, config) in &cameras {
        match (config, single) {
            (None, true) => {
                commands.entity(camera).insert(ClusterConfig::Single);
            }
            (Some(ClusterConfig::Single), false) => {
                commands.entity(camera).remove::<ClusterConfig>();
            }
            _ => {}
        }
    }
}

/// `CHIMERA_GPU_PREPROCESS=1`: Bevy's GPU mesh preprocessing and indirect
/// draws for every camera (the default before; for comparisons).
pub fn gpu_preprocessing() -> bool {
    chimera_core::env::var("CHIMERA_GPU_PREPROCESS").is_ok_and(|v| v == "1")
}

/// Set on a camera whose copy to its target `skip_covered_outputs` turned
/// off, so it can be turned back on (other cameras' output modes are left
/// as they are).
#[derive(Component)]
pub struct OutputSkipped;

/// Set on a camera whose copy `skip_covered_outputs` made replace the
/// target (it copies out for cameras below it that skip theirs), so it can
/// be turned back to Bevy's default.
#[derive(Component)]
pub struct OutputReplaces;

/// The copy out that replaces what the target holds.
const REPLACE: CameraOutputMode = CameraOutputMode::Write {
    blend_state: Some(BlendState::REPLACE),
    clear_color: ClearColorConfig::Default,
};

/// `CameraOutputMode` has no `PartialEq`: the modes this module sets.
fn same_output(a: &CameraOutputMode, b: &CameraOutputMode) -> bool {
    match (a, b) {
        (CameraOutputMode::Skip, CameraOutputMode::Skip) => true,
        (
            CameraOutputMode::Write {
                blend_state: x,
                clear_color: ClearColorConfig::Default,
            },
            CameraOutputMode::Write {
                blend_state: y,
                clear_color: ClearColorConfig::Default,
            },
        ) => x == y,
        _ => false,
    }
}

fn is_default_output(mode: &CameraOutputMode) -> bool {
    matches!(
        mode,
        CameraOutputMode::Write {
            blend_state: None,
            clear_color: ClearColorConfig::Default
        }
    )
}

/// Which cameras share intermediate textures: the same target, HDR or
/// not, the same MSAA (Bevy's `main_texture_key`, less the texture usages,
/// which no camera here changes).
fn texture_key(
    target: Option<&RenderTarget>,
    window: Option<Entity>,
    hdr: bool,
    msaa: Option<&Msaa>,
) -> Option<(NormalizedRenderTarget, bool, u32)> {
    let target = target.cloned().unwrap_or_default().normalize(window)?;
    Some((target, hdr, msaa.copied().unwrap_or_default().samples()))
}

/// Turn off the copy out to the target of every active camera that a
/// later active camera on the same intermediate textures, drawing the
/// whole target, copies out after it (`CameraOutputMode::Skip`); turn it
/// back on once nothing later does (the view model's or the UI's camera
/// off, a capture drawing the world into an image). The camera left
/// copying for skipped ones replaces the target (`BlendState::REPLACE`)
/// unless a lower camera on the target still copies out its own: Bevy
/// alpha-blends every copy but the target's bottom camera's, which with
/// the bottom ones skipped would blend the frame over the clear colour.
/// Runs last in the frame, after everything that turns cameras on and
/// off, so the frame extracted always has one copy per target. Writes
/// only on change.
#[allow(clippy::type_complexity)]
pub fn skip_covered_outputs(
    window: Query<Entity, With<PrimaryWindow>>,
    mut cameras: Query<(
        Entity,
        &mut Camera,
        Option<&RenderTarget>,
        Has<Hdr>,
        Option<&Msaa>,
        Has<OutputSkipped>,
        Has<OutputReplaces>,
    )>,
    mut commands: Commands,
) {
    let window = window.iter().next();
    type Key = (NormalizedRenderTarget, bool, u32);
    // The highest order of a whole-target camera, per texture key.
    let mut last: Vec<(Key, isize)> = Vec::new();
    for (_, camera, target, hdr, msaa, ..) in &cameras {
        if !camera.is_active || camera.viewport.is_some() {
            continue;
        }
        let Some(key) = texture_key(target, window, hdr, msaa) else {
            continue;
        };
        match last.iter_mut().find(|(k, _)| *k == key) {
            Some((_, order)) => *order = (*order).max(camera.order),
            None => last.push((key, camera.order)),
        }
    }
    // Per active camera this frame: its texture key, order, and whether it
    // skips its copy.
    let mut drawn: Vec<(Entity, Key, isize, bool)> = Vec::new();
    for (entity, camera, target, hdr, msaa, skipped, replaces) in &cameras {
        if !camera.is_active {
            continue;
        }
        let Some(key) = texture_key(target, window, hdr, msaa) else {
            continue;
        };
        let covered = last
            .iter()
            .find(|(k, _)| *k == key)
            .is_some_and(|(_, order)| *order > camera.order);
        // Only cameras copying the default way (or as this made them): one
        // with its own output mode (a blend) keeps it.
        let ours = skipped || replaces || is_default_output(&camera.output_mode);
        drawn.push((entity, key, camera.order, covered && ours));
    }
    for (entity, mut camera, .., skipped, replaces) in &mut cameras {
        let Some(&(_, ref key, order, skip)) = drawn.iter().find(|(e, ..)| *e == entity) else {
            // Off: as Bevy would have it (it copies nothing while off).
            if skipped || replaces {
                camera.output_mode = CameraOutputMode::default();
                commands.entity(entity).remove::<(OutputSkipped, OutputReplaces)>();
            }
            continue;
        };
        // A copier replaces the target when every camera below it on the
        // target skips its copy and one of them shares its textures.
        let below = || drawn.iter().filter(|(_, k, o, _)| k.0 == key.0 && *o < order);
        let replace = !skip && below().all(|(.., s)| *s) && below().any(|(_, k, ..)| k == key);
        let want = match (skip, replace) {
            (true, _) => CameraOutputMode::Skip,
            (false, true) => REPLACE,
            (false, false) => CameraOutputMode::default(),
        };
        if !(skipped || replaces || is_default_output(&camera.output_mode)) {
            // Its own output mode: left alone.
            continue;
        }
        if !same_output(&camera.output_mode, &want) {
            camera.output_mode = want;
        }
        if skip != skipped || replace != replaces {
            let mut e = commands.entity(entity);
            e.remove::<(OutputSkipped, OutputReplaces)>();
            if skip {
                e.insert(OutputSkipped);
            } else if replace {
                e.insert(OutputReplaces);
            }
        }
    }
}
