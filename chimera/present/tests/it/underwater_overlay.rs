//! The under-water overlay (`render::water`, spec water.md section 9): the
//! map's `$underwateroverlay` warp is spawned hidden on `VIEW_MODEL_LAYER`,
//! shown while the eye is in its water's volume and hidden above the
//! surface, and turns the view-model camera on that draws it
//! (`render::view_model::view_model_cameras_draw`) with no view model there
//! (dead, spectating, `r_drawviewmodel 0`).

use bevy::{camera::visibility::RenderLayers, prelude::*};
use chimera_core::core::{MapBrush, MapWater, MapWaterVolume};
use chimera_present::render::{
    view_model::view_model_cameras_draw,
    water::{
        MapWaterRender, ScreenWarpMaterial, UnderwaterOverlay, WaterMaterial, show_overlays, spawn_overlays,
        update_water,
    },
};
use chimera_world::map::{
    ViewModelAnchor, ViewModelCamera,
    view_model::VIEW_MODEL_LAYER,
    water::{MapScreenWarp, MapWaterMaterial, WaterSurface, WaterView},
};

/// Bevy's visibility propagation for the overlay (a root entity).
fn propagate(mut q: Query<(&Visibility, &mut InheritedVisibility), Without<ChildOf>>) {
    for (v, mut inherited) in &mut q {
        let want = if *v == Visibility::Hidden {
            InheritedVisibility::HIDDEN
        } else {
            InheritedVisibility::VISIBLE
        };
        if *inherited != want {
            *inherited = want;
        }
    }
}

/// A 20 m square of water 5 m deep, its surface at y = 0, with an
/// overlay; the eye (the main camera, the view model's anchor) at `eye`.
fn app(eye: Vec3) -> (App, Entity, Entity) {
    let mut app = App::new();
    let warp = MapScreenWarp {
        name: "effects/water_warp01".into(),
        normal_frames: vec![0],
        frame_rate: 0.0,
        refract_amount: 0.04,
        tint: [1.0; 3],
        blur: 1.0,
        scale: Vec2::ONE,
        scroll: Vec2::ZERO,
    };
    let (min, max) = (Vec3::new(-10.0, -5.0, -10.0), Vec3::new(10.0, 0.0, 10.0));
    app.insert_resource(Assets::<Mesh>::default())
        .insert_resource(Assets::<WaterMaterial>::default())
        .insert_resource(Assets::<ScreenWarpMaterial>::default())
        .insert_resource(MapWater(vec![MapWaterVolume {
            brush: MapBrush::from_box(min, max),
            slime: false, lava: false, current: None,
        }]))
        .insert_resource(MapWaterRender {
            materials: vec![MapWaterMaterial {
                above_water: true,
                underwater_overlay: Some(warp.clone()),
                ..default()
            }],
            reflection: None,
            scene_fog: (Vec4::ZERO, Vec4::ZERO, Vec4::ZERO),
            overlays: vec![(0, warp, Handle::default())],
        })
        .insert_resource(WaterView::default())
        .add_systems(Update, spawn_overlays)
        .add_systems(
            PostUpdate,
            (update_water, show_overlays, propagate, view_model_cameras_draw).chain(),
        );
    app.world_mut().spawn((
        WaterSurface {
            material: 0,
            min: Vec3::new(min.x, 0.0, min.z),
            max,
            below: false,
            skybox: false,
        },
        MeshMaterial3d::<WaterMaterial>(Handle::default()),
        Visibility::default(),
    ));
    let main = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            Camera::default(),
            Projection::Perspective(PerspectiveProjection::default()),
            GlobalTransform::from_translation(eye),
            ViewModelAnchor,
        ))
        .id();
    let view_model = app
        .world_mut()
        .spawn((Camera::default(), ViewModelCamera, ChildOf(main)))
        .id();
    (app, main, view_model)
}

fn overlay(app: &mut App) -> (Visibility, RenderLayers) {
    let mut q = app
        .world_mut()
        .query_filtered::<(&Visibility, &RenderLayers), With<UnderwaterOverlay>>();
    let all: Vec<_> = q.iter(app.world()).map(|(v, l)| (*v, l.clone())).collect();
    assert_eq!(all.len(), 1, "one overlay for the one material that has one");
    all[0].clone()
}

fn camera_on(app: &App, camera: Entity) -> bool {
    app.world().get::<Camera>(camera).unwrap().is_active
}

fn set_eye(app: &mut App, main: Entity, eye: Vec3) {
    *app.world_mut().get_mut::<GlobalTransform>(main).unwrap() = GlobalTransform::from_translation(eye);
}

#[test]
fn shown_under_water_hidden_above() {
    let (mut app, main, view_model) = app(Vec3::new(0.0, -2.0, 0.0));
    app.update();
    let (visibility, layers) = overlay(&mut app);
    assert_eq!(
        layers,
        RenderLayers::layer(VIEW_MODEL_LAYER),
        "drawn by the view-model camera"
    );
    assert_eq!(visibility, Visibility::Visible, "eye 2 m under");
    assert_eq!(app.world().resource::<WaterView>().under, Some(0));
    assert!(
        camera_on(&app, view_model),
        "the overlay alone turns the view-model camera on"
    );

    // Head just above the surface (the near plane may cross it: the
    // intersection view, not under water).
    set_eye(&mut app, main, Vec3::new(0.0, 0.1, 0.0));
    app.update();
    assert_eq!(overlay(&mut app).0, Visibility::Hidden, "eye 10 cm above");
    assert!(!camera_on(&app, view_model), "nothing else on its layer");

    // Back under, then beside the pool at the same depth.
    set_eye(&mut app, main, Vec3::new(0.0, -0.1, 0.0));
    app.update();
    assert_eq!(overlay(&mut app).0, Visibility::Visible, "eye 10 cm under");
    assert!(camera_on(&app, view_model));
    set_eye(&mut app, main, Vec3::new(12.0, -2.0, 0.0));
    app.update();
    assert_eq!(overlay(&mut app).0, Visibility::Hidden, "outside the volume");
}
