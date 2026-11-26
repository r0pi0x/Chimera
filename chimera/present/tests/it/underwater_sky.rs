//! The sky with the eye under water (spec water.md section 9): the main
//! view draws no sky. With refraction on, the original renders the
//! refraction view (above-water world, 2D and 3D sky) into the back buffer
//! first and the main view clears only depth over it, so the sky shows
//! through the surface from below: the sky camera keeps drawing behind
//! the main camera, which doesn't clear, and the 3D skybox's water is
//! hidden (the refraction view draws no water surfaces; its under side
//! covered the sky in black on de_port). Without refraction (fog-only
//! water, de_nuke's pool, or `r_WaterDrawRefraction 0`) the view clears to
//! the water's fog colour and the sky camera is off.

use bevy::prelude::*;
use chimera_core::core::{MapBrush, MapWater, MapWaterVolume};
use chimera_present::render::{
    fog::SceneFog,
    sky::{SkyCameraInfo, follow_sky_camera},
    water::{MapWaterRender, WaterMaterial, WaterSettings, underwater_fog, update_water},
};
use chimera_world::map::{
    MapSkyCamera, SkyboxCamera,
    water::{MapWaterMaterial, WaterSurface, WaterView},
};

const UNIT: f32 = 0.0254;

/// de_port's sea as the game loads it: a refracting Water material.
fn port_sea() -> MapWaterMaterial {
    MapWaterMaterial {
        name: "nature/water_wasteland002b".into(),
        refract: true,
        fog_color: [21.0 / 255.0, 48.0 / 255.0, 52.0 / 255.0],
        fog_start: 100.0 * UNIT,
        fog_end: 500.0 * UNIT,
        cheap_start: 3000.0 * UNIT,
        cheap_end: 5000.0 * UNIT,
        ..default()
    }
}

/// de_nuke's pool: fog-only water (no refraction view).
fn nuke_pool() -> MapWaterMaterial {
    MapWaterMaterial {
        name: "de_nuke/nukwater_movingplane".into(),
        fog_only: true,
        fog_color: [150.0 / 255.0, 200.0 / 255.0, 200.0 / 255.0],
        fog_start: 64.0 * UNIT,
        fog_end: 512.0 * UNIT,
        ..default()
    }
}

/// A 20 m square of water 5 m deep (surface at y = 0) with a 3D skybox
/// copy of it; the main camera at `eye`. Returns the app, the main camera
/// and the skybox water surface.
fn app(m: MapWaterMaterial, settings: WaterSettings) -> (App, Entity, Entity) {
    let mut app = App::new();
    let fog_only = m.fog_only;
    let (min, max) = (Vec3::new(-10.0, -5.0, -10.0), Vec3::new(10.0, 0.0, 10.0));
    app.insert_resource(Assets::<Mesh>::default())
        .insert_resource(Assets::<WaterMaterial>::default())
        .insert_resource(SceneFog::default())
        .insert_resource(settings)
        .insert_resource(MapWater(vec![MapWaterVolume {
            brush: MapBrush::from_box(min, max),
            slime: false, lava: false, current: None,
        }]))
        .insert_resource(MapWaterRender {
            materials: vec![m],
            reflection: None,
            scene_fog: (Vec4::ZERO, Vec4::ZERO, Vec4::ZERO),
            overlays: Vec::new(),
        })
        .insert_resource(SkyCameraInfo(MapSkyCamera {
            origin: Vec3::new(0.0, 100.0, 0.0),
            scale: 16.0,
            fog: None,
        }))
        .insert_resource(WaterView::default())
        .add_systems(PostUpdate, (update_water, underwater_fog, follow_sky_camera).chain());
    let surface = WaterSurface {
        material: 0,
        min: Vec3::new(min.x, 0.0, min.z),
        max,
        below: false,
        skybox: false,
    };
    if fog_only {
        app.world_mut().spawn(surface);
    } else {
        app.world_mut().spawn((
            surface,
            MeshMaterial3d::<WaterMaterial>(Handle::default()),
            Visibility::default(),
        ));
    }
    // The 3D skybox's sea, under side.
    let skybox_water = app
        .world_mut()
        .spawn((
            WaterSurface {
                material: 0,
                min: Vec3::new(-1.0, 100.0, -1.0),
                max: Vec3::new(1.0, 100.0, 1.0),
                below: true,
                skybox: true,
            },
            MeshMaterial3d::<WaterMaterial>(Handle::default()),
            Visibility::default(),
        ))
        .id();
    let main = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            Camera::default(),
            Projection::Perspective(PerspectiveProjection::default()),
            GlobalTransform::from_translation(Vec3::new(0.0, 3.0, 0.0)),
        ))
        .id();
    (app, main, skybox_water)
}

fn set_eye(app: &mut App, main: Entity, eye: Vec3) {
    *app.world_mut().get_mut::<GlobalTransform>(main).unwrap() = GlobalTransform::from_translation(eye);
    // Two frames: the sky camera is spawned by commands on the first.
    app.update();
    app.update();
}

/// (sky camera drawing, the main camera's clear colour).
fn sky(app: &mut App, main: Entity) -> (bool, ClearColorConfig) {
    let on = app
        .world_mut()
        .query_filtered::<&Camera, With<SkyboxCamera>>()
        .single(app.world())
        .unwrap()
        .is_active;
    (on, app.world().get::<Camera>(main).unwrap().clear_color.clone())
}

fn shown(app: &App, e: Entity) -> bool {
    *app.world().get::<Visibility>(e).unwrap() != Visibility::Hidden
}

#[test]
fn refracting_water_keeps_the_sky_behind_the_view() {
    let (mut app, main, skybox_water) = app(port_sea(), WaterSettings::default());
    set_eye(&mut app, main, Vec3::new(0.0, 3.0, 0.0));
    assert!(
        matches!(sky(&mut app, main), (true, ClearColorConfig::None)),
        "above water"
    );
    assert!(shown(&app, skybox_water));

    set_eye(&mut app, main, Vec3::new(0.0, -0.5, 0.0));
    let view = app.world().resource::<WaterView>().clone();
    assert_eq!(view.under, Some(0));
    assert!(view.sky_through_surface);
    let under = sky(&mut app, main);
    assert!(
        matches!(under, (true, ClearColorConfig::None)),
        "under refracting water the sky is still drawn behind (the refraction view's): {under:?}"
    );
    assert!(!shown(&app, skybox_water), "no skybox water under water");

    set_eye(&mut app, main, Vec3::new(0.0, 3.0, 0.0));
    assert!(shown(&app, skybox_water), "back above: the skybox water again");
}

#[test]
fn without_refraction_the_view_clears_to_the_fog_colour() {
    let off = WaterSettings {
        draw_refraction: 0,
        ..default()
    };
    for (name, m, settings) in [
        ("fog-only water", nuke_pool(), WaterSettings::default()),
        ("r_WaterDrawRefraction 0", port_sea(), off),
    ] {
        let fog = m.fog_color;
        let (mut app, main, _) = app(m, settings);
        set_eye(&mut app, main, Vec3::new(0.0, -0.5, 0.0));
        assert!(!app.world().resource::<WaterView>().sky_through_surface, "{name}");
        let (on, clear) = sky(&mut app, main);
        assert!(!on, "{name}: no sky");
        let ClearColorConfig::Custom(c) = clear else {
            panic!("{name}: {clear:?}")
        };
        assert_eq!(c, Color::srgb(fog[0], fog[1], fog[2]), "{name}");
    }
}

/// Changing the main camera's clear colour edits the camera in place: a
/// copy inserted in its stead (cloned before that frame's `camera_system`
/// computed the camera's target and projection) left a client's
/// first-person camera, born the frame the sky's clear changed, drawing
/// nothing for good (a networked player saw only the 3D skybox).
#[test]
fn the_main_camera_is_edited_in_place_not_replaced() {
    #[derive(Resource, Default)]
    struct Replaced(u32);
    let (mut app, main, _) = app(nuke_pool(), WaterSettings::default());
    app.init_resource::<Replaced>()
        .add_observer(|_: On<Discard<Camera>>, mut n: ResMut<Replaced>| n.0 += 1);
    set_eye(&mut app, main, Vec3::new(0.0, -0.5, 0.0));
    assert!(matches!(sky(&mut app, main).1, ClearColorConfig::Custom(_)), "the clear changed");
    set_eye(&mut app, main, Vec3::new(0.0, 3.0, 0.0));
    assert_eq!(app.world().resource::<Replaced>().0, 0, "the camera component was never replaced");
}
