//! Monitors (Source's func_monitor showing a point_camera's view, public
//! entity docs), for any game: the map's monitor screen texture
//! (`MapData::camera_texture`) is a render target that a camera of our
//! own draws into from the active camera's pose (`map::MonitorCamera`,
//! which the logic writes), only while one is active. What the camera sees
//! is culled for the player's own view (`vis`), as the rest of the map.
//! The sky shows behind it as in the main view: a sky camera of its own
//! (`map::monitor::ScreenSkyCamera`) draws the 2D sky and the 3D skybox
//! from the monitor camera's place scaled into it, first. Its fog is the
//! point_camera's own (`map::MonitorFog`), not the map's.
//! While a map settles every view a monitor may show (`map::TargetViews`,
//! which the game's layer fills) is drawn into the screen by a camera of
//! its own (`warm_target_views`, below the live ones), and the settle
//! waits for each (`target_views_pending`, a `map::loading::LoadPending`
//! count), so what it shows is in before the first view opens
//! (`client::first_view`), not when the camera first turns on.
//!
//! Monitors for any game (Source's func_monitor and point_camera): the map's
//! monitor screen texture (`MapData::camera_texture`, Source `_rt_Camera` /
//! `engine/camerarendertarget`) is a 512² render target (repeating, as monitor
//! faces' coordinates sit whole repeats from 0) that a camera of ours
//! (`ScreenCamera`) draws into from the active camera's pose, only while one is
//! active, over a sky camera of its own (`ScreenSkyCamera`, also a
//! `ScreenCamera`: the 2D sky and the 3D skybox from the camera's place scaled
//! into it); with the point_camera's own fog. While a map settles each
//! view in `map::TargetViews` is warmed by cameras of the same kind
//! (`TargetViewWarm`), held in the settle until drawn `WARM_FRAMES`.

use bevy::{
    camera::RenderTarget,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::TextureFormat,
};

use super::{
    LIGHTMAP_EXPOSURE,
    sky::{MapSkybox, SkyCameraInfo},
};
use crate::map::{
    EMPTY_LAYER, MapPart, MonitorCamera, SKYBOX_LAYER, TargetViews,
    monitor::{ScreenCamera, ScreenSkyCamera},
};

/// The monitor screen's size, pixels.
pub const SCREEN_SIZE: u32 = 512;

/// The render target monitors show.
#[derive(Resource, Clone, Debug)]
pub struct MonitorScreen(pub Handle<Image>);

/// A render target image for the screen.
pub fn screen_image(images: &mut Assets<Image>) -> Handle<Image> {
    let mut image = Image::new_target_texture(
        SCREEN_SIZE,
        SCREEN_SIZE,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    // Monitor faces' texture coordinates are whole repeats away from 0
    // (59..60): the picture repeats, as the game's textures do.
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    images.add(image)
}

/// Keep the screen camera where the active point_camera is, on only while
/// one is.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive_screen_camera(
    screen: Option<Res<MonitorScreen>>,
    want: Res<MonitorCamera>,
    mut cameras: Query<
        (Entity, &mut Camera, &mut Transform, &mut Projection),
        (With<ScreenCamera>, Without<ScreenSkyCamera>, Without<TargetViewWarm>),
    >,
    mut skies: Query<
        (Entity, &mut Camera, &mut Transform, &mut Projection),
        (With<ScreenSkyCamera>, Without<TargetViewWarm>),
    >,
    sky_info: Option<Res<SkyCameraInfo>>,
    skybox: Option<Res<MapSkybox>>,
    mut commands: Commands,
) {
    let Some(screen) = screen else {
        for (e, ..) in cameras.iter().chain(skies.iter()) {
            commands.entity(e).despawn();
        }
        return;
    };
    // The camera's fog, else none (not the map's).
    let fog = want.1.unwrap_or_default().distance_fog(1.0);
    let Some((at, rotation, fov)) = want.0 else {
        for (_, mut c, ..) in cameras.iter_mut().chain(skies.iter_mut()) {
            if c.is_active {
                c.is_active = false;
            }
        }
        return;
    };
    // The sky camera: the 2D sky, and the 3D skybox from the monitor
    // camera's place scaled into it.
    let has_sky = skybox.is_some() || sky_info.is_some();
    if has_sky {
        let (sky_at, layer) = match &sky_info {
            Some(i) => (i.0.origin + at / i.0.scale, SKYBOX_LAYER),
            None => (at, EMPTY_LAYER),
        };
        let sky_projection = Projection::Perspective(PerspectiveProjection {
            fov: fov.clamp(1.0, 170.0).to_radians(),
            aspect_ratio: 1.0,
            near: 0.01,
            ..default()
        });
        match skies.single_mut() {
            Ok((_, mut c, mut t, mut p)) => {
                if !c.is_active {
                    c.is_active = true;
                }
                t.set_if_neq(Transform::from_translation(sky_at).with_rotation(rotation));
                if let (Projection::Perspective(now), Projection::Perspective(want)) = (&*p, &sky_projection)
                    && (now.fov - want.fov).abs() > 1e-4
                {
                    *p = sky_projection;
                }
            }
            Err(_) => {
                spawn_sky_camera(&mut commands, &screen.0, (sky_at, rotation, fov), layer, skybox.as_deref(), sky_info.as_deref(), (-2, ()));
            }
        }
    }
    let projection = Projection::Perspective(PerspectiveProjection {
        fov: fov.clamp(1.0, 170.0).to_radians(),
        aspect_ratio: 1.0,
        ..default()
    });
    match cameras.single_mut() {
        Ok((e, mut c, mut t, mut p)) => {
            if !c.is_active {
                c.is_active = true;
            }
            if want.is_changed() {
                commands.entity(e).insert(fog);
            }
            t.set_if_neq(Transform::from_translation(at).with_rotation(rotation));
            let fov_now = match &*p {
                Projection::Perspective(pp) => pp.fov,
                _ => -1.0,
            };
            if (fov_now - fov.clamp(1.0, 170.0).to_radians()).abs() > 1e-4 {
                *p = projection;
            }
        }
        Err(_) => {
            commands.spawn(screen_camera(&screen.0, at, rotation, fov, fog, has_sky, -1));
        }
    }
}

/// A camera drawing a view into the monitor screen: before the main view,
/// which shows its picture, at `order` (after its sky camera, whose
/// picture it draws over when there is a sky).
fn screen_camera(
    screen: &Handle<Image>,
    at: Vec3,
    rotation: Quat,
    fov: f32,
    fog: bevy::pbr::DistanceFog,
    has_sky: bool,
    order: isize,
) -> impl Bundle {
    (
        Name::new("Monitor camera"),
        ScreenCamera,
        MapPart,
        Camera3d::default(),
        Camera {
            order,
            clear_color: if has_sky {
                ClearColorConfig::None
            } else {
                Color::BLACK.into()
            },
            ..default()
        },
        RenderTarget::Image(screen.clone().into()),
        fog,
        Projection::Perspective(PerspectiveProjection {
            fov: fov.clamp(1.0, 170.0).to_radians(),
            aspect_ratio: 1.0,
            ..default()
        }),
        Transform::from_translation(at).with_rotation(rotation),
    )
}

/// The sky behind a monitor view: the 2D sky, and the 3D skybox from the
/// view's place scaled into it (`pose`'s place, on `layer`), at `order`,
/// with `extra`.
fn spawn_sky_camera(
    commands: &mut Commands,
    screen: &Handle<Image>,
    (sky_at, rotation, fov): (Vec3, Quat, f32),
    layer: usize,
    skybox: Option<&MapSkybox>,
    sky_info: Option<&SkyCameraInfo>,
    (order, extra): (isize, impl Bundle),
) {
    let mut e = commands.spawn((
        Name::new("Monitor sky camera"),
        ScreenSkyCamera,
        ScreenCamera,
        MapPart,
        Camera3d::default(),
        Camera {
            order,
            clear_color: Color::BLACK.into(),
            ..default()
        },
        RenderTarget::Image(screen.clone().into()),
        Projection::Perspective(PerspectiveProjection {
            fov: fov.clamp(1.0, 170.0).to_radians(),
            aspect_ratio: 1.0,
            near: 0.01,
            ..default()
        }),
        Transform::from_translation(sky_at).with_rotation(rotation),
        bevy::camera::visibility::RenderLayers::layer(layer),
        extra,
    ));
    if let Some(sky) = skybox {
        e.insert(bevy::light::Skybox {
            image: Some(sky.0.clone()),
            brightness: LIGHTMAP_EXPOSURE,
            ..default()
        });
    }
    if let Some((fog, scale)) = sky_info.and_then(|i| i.0.fog.as_ref().map(|f| (f, i.0.scale))) {
        let [r, g, b] = fog.color;
        e.insert(bevy::pbr::DistanceFog {
            color: Color::srgb(r, g, b),
            falloff: bevy::pbr::FogFalloff::Linear {
                start: fog.start / scale,
                end: fog.end / scale,
            },
            ..default()
        });
    }
}

/// Render frames a warming view draws before it counts as warmed: its
/// first frame specializes its pipelines and asks for what it sees; the
/// render world's counts (`client::first_view`) lag a frame and see them
/// pending from there, holding the settle until they are in.
pub const WARM_FRAMES: u32 = 4;

/// A camera drawing one of `map::TargetViews` while a map settles
/// (`warm_target_views`): the view's index, frames drawn.
#[derive(Component, Clone, Copy, Debug)]
pub struct TargetViewWarm {
    pub view: usize,
    pub frames: u32,
}

/// While a map settles: each view a monitor may show (`map::TargetViews`)
/// drawn into the screen by a camera of its own, as the live screen
/// cameras would draw it (the same view: its fog, its sky), below them,
/// so its textures, models and pipelines are in before the first view
/// opens. Made again when the views change; gone with the settle.
#[allow(clippy::too_many_arguments)]
fn warm_target_views(
    settling: Option<Res<crate::map::MapSettling>>,
    screen: Option<Res<MonitorScreen>>,
    views: Res<TargetViews>,
    sky_info: Option<Res<SkyCameraInfo>>,
    skybox: Option<Res<MapSkybox>>,
    mut warm: Query<(Entity, &mut TargetViewWarm)>,
    mut made: Local<Option<Vec<crate::map::TargetView>>>,
    mut commands: Commands,
) {
    let (Some(screen), true) = (screen, settling.is_some()) else {
        for (e, _) in &warm {
            commands.entity(e).try_despawn();
        }
        *made = None;
        return;
    };
    if made.as_ref() == Some(&views.0) {
        for (_, mut w) in &mut warm {
            w.frames = w.frames.saturating_add(1);
        }
        return;
    }
    for (e, _) in &warm {
        commands.entity(e).try_despawn();
    }
    let has_sky = skybox.is_some() || sky_info.is_some();
    for (i, v) in views.0.iter().enumerate() {
        let warm = TargetViewWarm { view: i, frames: 0 };
        // Below the live screen cameras (-2, -1), a sky and a view each.
        let order = -10 - 2 * i as isize;
        if has_sky {
            let (sky_at, layer) = match &sky_info {
                Some(s) => (s.0.origin + v.at / s.0.scale, SKYBOX_LAYER),
                None => (v.at, EMPTY_LAYER),
            };
            spawn_sky_camera(
                &mut commands,
                &screen.0,
                (sky_at, v.rotation, v.fov),
                layer,
                skybox.as_deref(),
                sky_info.as_deref(),
                (order - 1, warm),
            );
        }
        let fog = v.fog.unwrap_or_default().distance_fog(1.0);
        commands.spawn((screen_camera(&screen.0, v.at, v.rotation, v.fov, fog, has_sky, order), warm));
    }
    *made = Some(views.0.clone());
}

/// `map::loading::LoadPending`: the views a monitor may show not drawn
/// for `WARM_FRAMES` yet while the map settles (none without a screen).
pub fn target_views_pending(w: &World) -> usize {
    if !w.contains_resource::<crate::map::MapSettling>() || !w.contains_resource::<MonitorScreen>() {
        return 0;
    }
    let Some(views) = w.get_resource::<TargetViews>() else { return 0 };
    let warmed: std::collections::HashSet<usize> = w
        .try_query::<&TargetViewWarm>()
        .map(|mut q| q.iter(w).filter(|t| t.frames >= WARM_FRAMES).map(|t| t.view).collect())
        .unwrap_or_default();
    views.0.len().saturating_sub(warmed.len())
}

/// Drives the screen cameras (`map_plugin` adds it right after map's own
/// registration, where the map's plugins were added).
pub struct MonitorPlugin;

impl Plugin for MonitorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (drive_screen_camera, warm_target_views).run_if(resource_exists::<Assets<Image>>),
        );
    }
}
