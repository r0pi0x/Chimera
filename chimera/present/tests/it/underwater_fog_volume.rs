//! Fog-only water (`MapWaterMaterial::fog_only`, spec water.md sections 9
//! and 10): a water brush whose material isn't the Water shader (de_nuke's
//! additive LightmappedGeneric pool) is drawn as the world, but its volume
//! still fogs the view while the eye is in it: `update_water` finds the
//! eye in it from its surface's bounds alone (no water material), and
//! `underwater_fog` gives the whole view (above the surface too: no
//! refraction view) the material's range fog. No intersection view above
//! it, and main cameras get no depth prepass for it. Water-shader water
//! keeps the fog below its surface only.

use bevy::prelude::*;
use chimera_core::core::{MapBrush, MapWater, MapWaterVolume};
use chimera_present::render::{
    fog::{FogUniform, SceneFog, WATER_FOG_OFF, WATER_FOG_RANGE, fog_at},
    water::{MapWaterRender, WaterMaterial, prepare_main_cameras, underwater_fog, update_water},
};
use chimera_world::map::water::{MapWaterMaterial, WaterSurface, WaterView};

const UNIT: f32 = 0.0254;

/// de_nuke's pool material as the game loads it: {150 200 200}, 64 to 512
/// units.
fn nuke_pool() -> MapWaterMaterial {
    MapWaterMaterial {
        name: "de_nuke/nukwater_movingplane".into(),
        fog_only: true,
        refract: false,
        fog_color: [150.0 / 255.0, 200.0 / 255.0, 200.0 / 255.0],
        fog_start: 64.0 * UNIT,
        fog_end: 512.0 * UNIT,
        ..default()
    }
}

/// A 20 m square of water 5 m deep, its surface at y = 0, of material
/// `m`; the eye looking straight ahead at `eye`. Fog-only surfaces are
/// spawned as the drawing spawns them: bounds only.
fn app(m: MapWaterMaterial, eye: Vec3) -> (App, Entity) {
    let mut app = App::new();
    let fog_only = m.fog_only;
    let (min, max) = (Vec3::new(-10.0, -5.0, -10.0), Vec3::new(10.0, 0.0, 10.0));
    app.insert_resource(Assets::<Mesh>::default())
        .insert_resource(Assets::<WaterMaterial>::default())
        .insert_resource(SceneFog::default())
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
        .insert_resource(WaterView::default())
        .add_systems(Update, prepare_main_cameras)
        .add_systems(PostUpdate, (update_water, underwater_fog).chain());
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
    let main = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            Camera::default(),
            Projection::Perspective(PerspectiveProjection::default()),
            GlobalTransform::from_translation(eye),
        ))
        .id();
    (app, main)
}

fn set_eye(app: &mut App, main: Entity, eye: Vec3) {
    *app.world_mut().get_mut::<GlobalTransform>(main).unwrap() = GlobalTransform::from_translation(eye);
}

fn scene_fog(app: &App) -> FogUniform {
    app.world().resource::<SceneFog>().0
}

#[test]
fn fog_only_water_fogs_the_whole_view_under_water() {
    let (mut app, main) = app(nuke_pool(), Vec3::new(0.0, -1.5, 0.0));
    app.update();
    let view = app.world().resource::<WaterView>().clone();
    assert_eq!(view.under, Some(0), "eye in the pool's volume");
    assert_eq!(view.clear, Some(nuke_pool().fog_color), "cleared to its fog colour");
    let f = scene_fog(&app);
    assert_eq!(f.water_color.w, WATER_FOG_RANGE);
    let eye = Vec3::new(0.0, -1.5, 0.0);
    // 256 units away (halfway through 64..512 is 192/448): range fog,
    // squared, below the surface and above it alike.
    let d = 256.0 * UNIT;
    let want = (192.0f32 / 448.0).powi(2);
    for point in [Vec3::new(0.0, -1.5, -d), Vec3::new(0.0, 3.0, -d)] {
        let (colour, amount) = fog_at(&f, eye, point, d);
        assert!((amount - want).abs() < 1e-4, "{point}: {amount} vs {want}");
        let lin = nuke_pool().fog_color.map(|c| c.powf(2.2));
        assert!((colour - Vec3::from(lin)).length() < 1e-4);
    }
    assert!(
        app.world()
            .get::<bevy::core_pipeline::prepass::DepthPrepass>(main)
            .is_none(),
        "no depth prepass for water drawn as the world"
    );

    // Head 2 cm above the surface: the near plane crosses it, but fog-only
    // water has no intersection view, so no water fog at all.
    set_eye(&mut app, main, Vec3::new(0.0, 0.02, 0.0));
    app.update();
    let view = app.world().resource::<WaterView>().clone();
    assert_eq!((view.under, view.intersect), (None, None));
    assert_eq!(scene_fog(&app).water_color.w, WATER_FOG_OFF);
}

#[test]
fn water_shader_water_keeps_fog_below_its_surface() {
    let m = MapWaterMaterial {
        fog_color: [0.1, 0.2, 0.2],
        fog_start: 100.0 * UNIT,
        fog_end: 500.0 * UNIT,
        ..default()
    };
    let (mut app, main) = app(m, Vec3::new(0.0, -1.5, 0.0));
    app.update();
    assert_eq!(app.world().resource::<WaterView>().under, Some(0));
    let f = scene_fog(&app);
    let eye = Vec3::new(0.0, -1.5, 0.0);
    let d = 300.0 * UNIT;
    assert!(fog_at(&f, eye, Vec3::new(0.0, -1.5, -d), d).1 > 0.2, "below: water fog");
    assert_eq!(fog_at(&f, eye, Vec3::new(0.0, 3.0, -d), d).1, 0.0, "above: the world's (none)");
    assert!(
        app.world()
            .get::<bevy::core_pipeline::prepass::DepthPrepass>(main)
            .is_some(),
        "its refraction reads a depth prepass"
    );
    set_eye(&mut app, main, Vec3::new(0.0, 0.02, 0.0));
    app.update();
    assert_eq!(
        app.world().resource::<WaterView>().intersect.map(|i| i.0),
        Some(0),
        "the intersection view"
    );
}
