//! A map with a monitor keeps its first view closed until what the
//! monitor may show is warmed (`render::monitor`'s `warm_target_views`,
//! `map::TargetViews`): while the map settles behind its loading screen
//! each view a monitor camera may show is drawn into the screen by a
//! camera of its own, and the settle's pending work
//! (`map::loading::LoadPending`, which `client::first_view` waits on)
//! counts each view until it has drawn `WARM_FRAMES` frames; the render
//! world's counts then hold the gate while that view's meshes, materials
//! and pipelines come in. Headless: the camera and the count, not the
//! drawing. The live screen camera (`map::MonitorCamera`) is apart from
//! the warming ones, and they go with the settle.

use bevy::{camera::RenderTarget, prelude::*};
use chimera_present::{
    MapPlugin,
    render::{
        beams::BeamMaterial,
        detail::DetailMaterial,
        monitor::{MonitorScreen, TargetViewWarm, WARM_FRAMES},
        prop_material::PropMaterial,
        rope_material::RopeMaterial,
        shadows::ShadowMaterial,
        sprite_material::SpriteMaterial,
        water::WaterMaterial,
        world_material::WorldMaterial,
    },
};
use chimera_world::{
    harness::Sim,
    map::{
        MapDebugView, MapSettling, MapTexture, MonitorCamera, MonitorFog, TargetView, TargetViews, change_map,
        loading::LoadPending, monitor::ScreenCamera,
    },
};

use crate::prop_shadows::{base, map};

fn sim() -> Sim {
    let mut sim = Sim::with(base(), |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<WaterMaterial>()
            .init_asset::<BeamMaterial>()
            .init_asset::<DetailMaterial>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .add_plugins(MapPlugin::empty());
    });
    sim.repeatable();
    sim
}

/// The settle's pending work by name (what `client::first_view` waits on).
fn pending(w: &World) -> Vec<(&'static str, usize)> {
    w.resource::<LoadPending>().counts(w).into_iter().filter(|(_, n)| *n > 0).collect()
}

fn warming(w: &mut World) -> Vec<(TargetViewWarm, Transform, Option<Handle<Image>>)> {
    w.query::<(&TargetViewWarm, &Transform, &RenderTarget)>()
        .iter(w)
        .map(|(t, tr, rt)| {
            let image = match rt {
                RenderTarget::Image(i) => Some(i.handle.clone()),
                _ => None,
            };
            (*t, *tr, image)
        })
        .collect()
}

#[test]
fn a_monitor_cameras_view_holds_the_first_view_until_warmed() {
    let mut sim = sim();
    // A map whose texture 0 is the monitor screen (Source's
    // `_rt_Camera`): its render target and `MonitorScreen`.
    let mut data = map();
    data.name = "test:monitor".into();
    data.textures = vec![MapTexture {
        name: "_rt_camera".into(),
        srgb: true,
        mips: Vec::new(),
        width: 4,
        height: 4,
        rgba8: vec![0; 64],
        bc: None,
    }];
    data.camera_texture = Some(0);
    change_map(sim.app.world_mut(), data, MapDebugView::Normal);
    sim.run_until_idle(600).unwrap();
    let screen = sim.app.world().get_resource::<MonitorScreen>().expect("the map's monitor screen").0.clone();
    // Behind the loading screen; the game's layer names the view its
    // monitor may show (a point_camera, off for now: no live camera).
    let view = TargetView {
        at: Vec3::new(3.0, 2.0, -1.0),
        rotation: Quat::from_rotation_y(0.5),
        fov: 70.0,
        fog: Some(MonitorFog { enabled: true, start: 1.0, end: 9.0, ..default() }),
    };
    {
        let w = sim.app.world_mut();
        w.insert_resource(MapSettling);
        w.insert_resource(TargetViews(vec![view]));
    }
    assert_eq!(sim.app.world().resource::<MonitorCamera>().0, None, "no live monitor camera");

    // Held from the first frame until the view has drawn its frames.
    let mut frames = 0;
    loop {
        sim.settle_frames(1);
        frames += 1;
        let w = sim.app.world_mut();
        let p = pending(w);
        let cams = warming(w);
        assert_eq!(cams.len(), 1, "one camera draws the view (no sky on this map)");
        let (warm, at, image) = &cams[0];
        assert_eq!(warm.view, 0);
        assert!(at.translation.distance(view.at) < 1e-5 && at.rotation.angle_between(view.rotation) < 1e-5, "{at:?}");
        assert_eq!(image.as_ref(), Some(&screen), "it draws into the monitor screen");
        if p.is_empty() {
            break;
        }
        assert_eq!(p, vec![("monitor views warming", 1)], "frame {frames}");
        assert!(frames <= WARM_FRAMES + 1, "still pending after {frames} frames");
    }
    assert!(frames >= WARM_FRAMES, "warmed after {frames} frames, before it drew {WARM_FRAMES}");

    // The live camera turning on is a camera of its own; the warming one
    // stays out of it.
    sim.app.world_mut().insert_resource(MonitorCamera(Some((Vec3::ZERO, Quat::IDENTITY, 90.0)), None));
    sim.settle_frames(2);
    {
        let w = sim.app.world_mut();
        let screens = w.query_filtered::<Entity, With<ScreenCamera>>().iter(w).count();
        assert_eq!(screens, 2, "the live screen camera and the warming one");
        assert_eq!(warming(w).len(), 1);
    }

    // A second view named later is warmed too, holding the settle again.
    let mut second = view;
    second.at = Vec3::new(-5.0, 1.0, 4.0);
    sim.app.world_mut().insert_resource(TargetViews(vec![view, second]));
    sim.settle_frames(1);
    assert_eq!(pending(sim.app.world()), vec![("monitor views warming", 2)]);
    assert_eq!(warming(sim.app.world_mut()).len(), 2);
    sim.settle_frames(WARM_FRAMES + 1);
    assert!(pending(sim.app.world()).is_empty());

    // The settle over: the warming cameras go, nothing is pending.
    sim.app.world_mut().remove_resource::<MapSettling>();
    sim.app.update();
    let w = sim.app.world_mut();
    assert!(warming(w).is_empty(), "the warming cameras went with the settle");
    assert!(pending(w).is_empty());
}
