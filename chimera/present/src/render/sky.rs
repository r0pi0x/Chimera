//! Drawing the sky: the 2D sky cubemap on the main cameras (`attach_sky`),
//! the 3D skybox's camera following the main one (`follow_sky_camera`) and
//! the 3D skybox's parts shown in place where the map has no sky camera
//! (`show_skybox_in_place`); the loaded map's sky resources (`MapSkybox`,
//! `SkyCameraInfo`, `SkyVis`, `ActiveMapHas3dSky`). The images and the
//! render layers come from `map::sky`.

#[allow(unused_imports)]
use super::LIGHTMAP_EXPOSURE;
use crate::map::*;
use bevy::prelude::*;

#[derive(Resource, Clone)]
pub struct SkyCameraInfo(pub MapSkyCamera);

/// The map's sky visibility, when it has one (see `MapSkyVis`).
#[derive(Resource)]
pub struct SkyVis(pub MapSkyVis);

/// The 3D skybox also exists where it was built, outside the playable area
/// (as in Source, where you can noclip to it). Drawn there only while the
/// camera is outside the playable area too: without the game's visibility
/// data, that keeps it from floating in view from inside the map.
#[allow(clippy::type_complexity)]
pub fn show_skybox_in_place(
    area: Option<Res<PlayableArea>>,
    cameras: Query<
        (&GlobalTransform, &Camera),
        (
            With<Camera3d>,
            Without<monitor::ScreenCamera>,
            Without<SkyboxCamera>,
            Without<ViewModelCamera>,
            Without<water::WaterReflectionCamera>,
        ),
    >,
    mut parts: Query<&mut bevy::camera::visibility::RenderLayers, Without<Camera>>,
    mut outside_before: Local<Option<bool>>,
) {
    let Some(area) = area else { return };
    let Some((eye, _)) = cameras.iter().find(|(_, c)| c.is_active) else {
        return;
    };
    let p = eye.translation();
    let (lo, hi) = area.0;
    let outside = p.cmplt(lo).any() || p.cmpgt(hi).any();
    if *outside_before == Some(outside) {
        return;
    }
    *outside_before = Some(outside);
    let sky = bevy::camera::visibility::RenderLayers::layer(SKYBOX_LAYER);
    let both = bevy::camera::visibility::RenderLayers::from_layers(&[0, SKYBOX_LAYER]);
    for mut layers in &mut parts {
        if layers.intersects(&sky) {
            *layers = if outside { both.clone() } else { sky.clone() };
        }
    }
}

/// Set when the map has a real 3D skybox (not just the 2D-sky camera).
#[derive(Resource)]
pub struct ActiveMapHas3dSky;

/// The map's sky cubemap, for cameras to show.
#[derive(Resource, Clone)]
pub struct MapSkybox(pub Handle<Image>);

/// Give every 3D camera the map's sky.
#[allow(clippy::type_complexity)]
pub fn attach_sky(
    mut commands: Commands,
    sky: Option<Res<MapSkybox>>,
    sky_camera: Option<Res<SkyCameraInfo>>,
    cameras: Query<
        (Entity, Has<SkyboxCamera>),
        (
            With<Camera3d>,
            Without<monitor::ScreenCamera>,
            Without<bevy::light::Skybox>,
            Without<ViewModelCamera>,
        ),
    >,
) {
    let Some(sky) = sky else { return };
    for (cam, is_sky_camera) in &cameras {
        // With a 3D skybox, the 2D sky is drawn behind it by the sky camera.
        if sky_camera.is_some() && !is_sky_camera {
            continue;
        }
        commands.entity(cam).insert(bevy::light::Skybox {
            image: Some(sky.0.clone()),
            // Show the texture at its own brightness (cancel camera exposure).
            brightness: LIGHTMAP_EXPOSURE,
            ..default()
        });
    }
}

/// Keep a sky camera behind each frame's main camera: at
/// `origin + eye / scale`, same rotation, projection and render target. The
/// main camera then draws the world over it without clearing.
#[allow(clippy::type_complexity)]
pub fn follow_sky_camera(
    mut commands: Commands,
    info: Option<Res<SkyCameraInfo>>,
    vis: Option<Res<SkyVis>>,
    has_3d_sky: Option<Res<ActiveMapHas3dSky>>,
    water_view: Option<Res<water::WaterView>>,
    warm: Option<Res<super::first_view_warm::WarmFirstView>>,
    sky_meshes: Query<
        (&bevy::camera::primitives::Aabb, &GlobalTransform, Option<&bevy::camera::visibility::RenderLayers>),
        (With<Mesh3d>, Without<SkyboxCamera>),
    >,
    mut main: Query<
        (
            &GlobalTransform,
            &Projection,
            Option<&bevy::camera::RenderTarget>,
            Option<&bevy::core_pipeline::tonemapping::Tonemapping>,
            Entity,
            &mut Camera,
        ),
        (
            With<Camera3d>,
            Without<monitor::ScreenCamera>,
            Without<SkyboxCamera>,
            Without<ViewModelCamera>,
            Without<water::WaterReflectionCamera>,
        ),
    >,
    mut sky: Query<
        (
            Entity,
            &mut Transform,
            &mut GlobalTransform,
            &mut Projection,
            &mut Camera,
            &mut bevy::camera::visibility::RenderLayers,
            Option<&bevy::camera::RenderTarget>,
            Option<&bevy::core_pipeline::tonemapping::Tonemapping>,
        ),
        With<SkyboxCamera>,
    >,
) {
    let Some(info) = info else { return };
    let Some((main_tf, projection, target, tonemapping, _main_entity, mut main_camera)) =
        main.iter_mut().find(|(.., c)| c.is_active)
    else {
        return;
    };
    let eye = main_tf.translation();
    let translation = info.0.origin + eye / info.0.scale;
    let rotation = main_tf.rotation();
    // Source: the sky is drawn only from leaves that see it; inside solid
    // the view clears to black (specs/cs_source/shadows_sky.md). Leaves
    // that see no sky show the previous frame in the game; black here.
    let leaf = vis.as_ref().map_or(LeafSky::Sky3d, |v| v.0.at(eye));
    // Under water the main view draws no sky and clears to the water's fog
    // colour, unless the water refracts: then the original's refraction
    // view (above-water world and sky) is already in the back buffer and
    // the main view clears only depth (specs/cs_source/water.md section
    // 9), so the sky camera still draws behind it.
    let under = water_view.as_ref().and_then(|w| w.clear);
    let through = water_view.as_ref().is_some_and(|w| w.sky_through_surface);
    let sky_on = matches!(leaf, LeafSky::Sky2d | LeafSky::Sky3d) && (under.is_none() || through);
    // A map settling behind a cover (`first_view_warm`): the sky camera
    // draws the whole 3D sky from above too, as the overview camera does
    // the world, so its meshes' pipelines compile there (its view's own),
    // not as the player first looks up.
    let warming = warm.is_some() && has_3d_sky.is_some();
    let warm_pose = warming
        .then(|| super::first_view_warm::overview_pose(&sky_meshes, SKYBOX_LAYER))
        .flatten();
    let sky_on = sky_on || warm_pose.is_some();
    let layer = if warm_pose.is_some() || (leaf == LeafSky::Sky3d && has_3d_sky.is_some()) {
        SKYBOX_LAYER
    } else {
        EMPTY_LAYER
    };
    let clear = match (sky_on, under) {
        (true, _) => ClearColorConfig::None,
        (false, Some([r, g, b])) => ClearColorConfig::Custom(Color::srgb(r, g, b)),
        (false, None) => ClearColorConfig::Custom(Color::BLACK),
    };
    let same = match (main_camera.clear_color, clear) {
        (ClearColorConfig::None, ClearColorConfig::None) => true,
        (ClearColorConfig::Custom(a), ClearColorConfig::Custom(b)) => a == b,
        _ => false,
    };
    // In place, never by inserting a copy: a copy made before this frame's
    // `camera_system` computed the camera's target would replace it with
    // one that never got its target or projection (a camera that draws
    // nothing; a client's first-person camera, which is born the frame
    // the sky's clear changes).
    if !same {
        main_camera.clear_color = clear;
    }
    match sky.single_mut() {
        Ok((entity, mut tf, mut global, mut proj, mut camera, mut layers, sky_target, sky_tonemapping)) => {
            if camera.is_active != sky_on {
                camera.is_active = sky_on;
            }
            if !layers.intersects(&bevy::camera::visibility::RenderLayers::layer(layer)) {
                *layers = bevy::camera::visibility::RenderLayers::layer(layer);
            }
            match warm_pose {
                Some((at, _)) => *tf = at,
                None => {
                    tf.translation = translation;
                    tf.rotation = rotation;
                }
            }
            // Propagation has run: set the global transform too (no parent).
            *global = GlobalTransform::from(*tf);
            // Only real changes: a changed projection, target or tonemapping
            // makes Bevy redo the camera's setup.
            let (fov, far) = match warm_pose {
                Some((_, far)) => (std::f32::consts::FRAC_PI_2, far),
                None => (
                    match projection {
                        Projection::Perspective(m) => m.fov,
                        _ => std::f32::consts::FRAC_PI_4,
                    },
                    PerspectiveProjection::default().far,
                ),
            };
            if let (Projection::Perspective(p), Projection::Perspective(main_p)) = (&*proj, projection)
                && (p.fov != fov || p.aspect_ratio != main_p.aspect_ratio || p.far != far)
                && let Projection::Perspective(p) = &mut *proj
            {
                p.fov = fov;
                p.far = far;
                p.aspect_ratio = main_p.aspect_ratio;
            }
            let mut e = commands.entity(entity);
            if let Some(t) = target
                && sky_target.and_then(|c| c.normalize(None)) != t.normalize(None)
            {
                e.insert(t.clone());
            }
            if let Some(t) = tonemapping
                && sky_tonemapping != Some(t)
            {
                e.insert(*t);
            }
        }
        Err(_) => {
            let mut e = commands.spawn((
                Name::new("Sky camera"),
                SkyboxCamera,
                MapPart,
                Camera3d::default(),
                Camera {
                    order: main_camera.order - 1,
                    ..default()
                },
                Projection::Perspective(PerspectiveProjection {
                    near: 0.01,
                    ..default()
                }),
                Transform::from_translation(translation).with_rotation(rotation),
                bevy::camera::visibility::RenderLayers::layer(layer),
            ));
            // Born on the main camera's target: one frame on the default
            // (the window) can pair a window-sized depth with an image.
            if let Some(t) = target {
                e.insert(t.clone());
            }
            if let Some(fog) = &info.0.fog {
                let [r, g, b] = fog.color;
                e.insert(bevy::pbr::DistanceFog {
                    color: Color::srgb(r, g, b),
                    falloff: bevy::pbr::FogFalloff::Linear {
                        start: fog.start / info.0.scale,
                        end: fog.end / info.0.scale,
                    },
                    ..default()
                });
            }
        }
    }
}
