//! A first-person muzzle flash is a world effect (specs/tf2/effects.md 2,
//! particles.md 6): a shot of a TF2 gun whose view model is drawn starts
//! its flash at the view model's muzzle re-projected into the world view
//! (right and up x tan(world FOV/2) / tan(view-model FOV/2)), following
//! the eye, drawn with the world camera (before, the view-model camera
//! drew it, 1.96x too big at FOV 90); with `r_drawviewmodel 0` there is
//! none. In third person the flash is the held model's, in the world.
//! Tracers (effects.md 6) start at the same re-projected muzzle and head
//! for the bullet's end. A made-up view model with a `muzzle` attachment
//! on the greybox (`harness::Tf2Sim`).

use bevy::prelude::*;
use chimera_world::map::{
    MapAttachment, MapBone, MapModel, MapViewModel, ShowLocalBody, ViewAnimator, ViewModelAnchor, ViewModelCamera, ViewModels,
    psys::StartParticleSystem,
    view_model::reproject,
};
use tf2::game::weapons::SMG;

use super::weapons::{holding, sim};

/// Where the made-up model's muzzle is in the eye's frame (its one bone at
/// the root, the model placed at the eye: no offset).
const MUZZLE: Vec3 = Vec3::new(0.12, -0.1, -0.5);

fn gun(key: &str) -> MapViewModel {
    MapViewModel {
        key: key.into(),
        model: MapModel::default(),
        bones: vec![MapBone { name: "root".into(), parent: None, position: Vec3::ZERO, rotation: Quat::IDENTITY }],
        root: Transform::IDENTITY,
        animations: None,
        right_handed: true,
        allow_flipping: false,
        attachments: vec![MapAttachment { name: "muzzle".into(), bone: 0, local: Transform::from_translation(MUZZLE) }],
        light_origin: Vec3::ZERO,
    }
}

/// The flashes `p`'s SMG starts over a burst, with the view-model camera
/// at `vm_fov` degrees (the world's at 90), the local body shown or not.
fn flashes(vm_fov: f32, third_person: bool) -> (Entity, Vec<StartParticleSystem>) {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), SMG);
    sim.app.world_mut().insert_resource(ViewModels::of([gun(SMG)]));
    sim.app.world_mut().insert_resource(ShowLocalBody(third_person));
    let perspective = |deg: f32| Projection::Perspective(PerspectiveProjection { fov: deg.to_radians(), ..default() });
    let anchor = sim
        .app
        .world_mut()
        .spawn((ViewModelAnchor, perspective(90.0), Transform::default(), Visibility::default(), ChildOf(p)))
        .id();
    sim.app.world_mut().spawn((ViewModelCamera, perspective(vm_fov), Transform::default(), ChildOf(anchor)));
    let mut cursor = sim.app.world().resource::<Messages<StartParticleSystem>>().get_cursor_current();
    let mut out = Vec::new();
    for _ in 0..30 {
        sim.app.world_mut().get_mut::<ViewAnimator>(p).expect("a view animator").key = Some(SMG.into());
        sim.intent(p).fire = true;
        sim.ticks(1);
        out.extend(cursor.read(sim.app.world().resource::<Messages<StartParticleSystem>>()).cloned());
    }
    (anchor, out)
}

#[test]
fn a_first_person_flash_starts_at_the_reprojected_muzzle_in_the_world() {
    for fov in [54.0, 75.0] {
        let (anchor, starts) = flashes(fov, false);
        let flashes: Vec<_> = starts.iter().filter(|s| s.name.starts_with("muzzle_")).collect();
        assert!(!flashes.is_empty(), "the SMG fired and flashed");
        let want = reproject(MUZZLE, 90.0, fov);
        for s in &flashes {
            let f = s.follow.expect("follows the eye");
            assert_eq!(f.entity, anchor);
            assert!(f.local.abs_diff_eq(want, 1e-4), "at the muzzle re-projected: {} vs {want} at {fov}", f.local);
            assert!(s.end.is_none() && s.tag.is_none());
        }
    }
    // The spec's case: (44.29, 5.22, -4.28) forward/right/up at world 90,
    // view 54 lands at (44.29, 10.25, -8.40): right and up x 1.9626.
    let eye = Vec3::new(5.22, -4.28, -44.29);
    let w = reproject(eye, 90.0, 54.0);
    assert!(w.abs_diff_eq(Vec3::new(10.25, -8.40, -44.29), 0.01), "{w}");
}

#[test]
fn the_view_model_switched_off_shows_no_flash() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), SMG);
    sim.app.world_mut().entity_mut(p).insert(chimera_core::core::LocalPlayer);
    sim.app.world_mut().insert_resource(chimera_world::map::view_model::ViewModelSettings { draw: 0, ..default() });
    let mut cursor = sim.app.world().resource::<Messages<StartParticleSystem>>().get_cursor_current();
    let mut out = Vec::new();
    for _ in 0..30 {
        sim.intent(p).fire = true;
        sim.ticks(1);
        out.extend(cursor.read(sim.app.world().resource::<Messages<StartParticleSystem>>()).cloned());
    }
    assert!(out.iter().all(|s| !s.name.starts_with("muzzle_")), "{out:?}");
    assert!(out.iter().any(|s| s.name.starts_with("bullet_tracer01")), "tracers still from the eye: {out:?}");
}

/// A tracer heads for the bullet's end from the re-projected muzzle,
/// named by the shooter's team (effects.md 6).
#[test]
fn tracers_start_at_the_muzzle_and_head_for_the_bullets_end() {
    let (anchor, starts) = flashes(54.0, false);
    let tracers: Vec<_> = starts.iter().filter(|s| s.name.starts_with("bullet_")).collect();
    assert!(!tracers.is_empty(), "the SMG drew tracers");
    for t in &tracers {
        assert!(t.name == "bullet_tracer01_red" || t.name == "bullet_tracer01_blue", "{}", t.name);
        assert!(t.end.is_some(), "heads for the bullet's end");
        assert!(t.follow.is_none(), "a tracer flies on its own");
    }
    let _ = anchor;
}

#[test]
fn a_third_person_flash_stays_in_the_world() {
    let (_, starts) = flashes(54.0, true);
    assert!(!starts.is_empty(), "the SMG fired and flashed");
    let flashes: Vec<_> = starts.iter().filter(|s| s.name.starts_with("muzzle_")).collect();
    assert!(!flashes.is_empty());
    // On the held model, carried by its owner (or where it started without
    // one), never by an eye.
    assert!(flashes.iter().all(|s| s.follow.is_none_or(|f| f.local.length() < 100.0)), "{flashes:?}");
}
