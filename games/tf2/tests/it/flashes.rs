//! A first-person muzzle flash is the view model's: a shot of a TF2 gun
//! whose view model is drawn starts its flash on the view-model camera's
//! layer (`StartParticleSystem::view_model`) at the muzzle where the
//! model is drawn, following the eye, the same point at any view-model
//! field of view (before, it was re-projected into the world view and
//! drawn with the world camera's field of view, so it slid off the barrel
//! as the view-model FOV changed). In third person the flash is the held
//! model's, in the world. A made-up view model with a `muzzle` attachment
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
fn a_first_person_flash_is_on_the_view_model_at_any_view_model_fov() {
    let mut seen = Vec::new();
    for fov in [54.0, 75.0] {
        let (anchor, starts) = flashes(fov, false);
        assert!(!starts.is_empty(), "the SMG fired and flashed");
        for s in &starts {
            assert!(s.view_model, "drawn by the view-model camera: {s:?}");
            let f = s.follow.expect("follows the eye");
            assert_eq!(f.entity, anchor);
            assert!(f.local.abs_diff_eq(MUZZLE, 1e-4), "at the muzzle as drawn, not re-projected: {} at {fov}", f.local);
        }
        seen.push(starts[0].follow.unwrap().local);
    }
    assert!(seen[0].abs_diff_eq(seen[1], 1e-5), "the same point at either FOV: {seen:?}");
    // What the old world-pass flash took: a point that moves with the FOV.
    assert!(!reproject(MUZZLE, 90.0, 54.0).abs_diff_eq(reproject(MUZZLE, 90.0, 75.0), 1e-3));
}

#[test]
fn a_third_person_flash_stays_in_the_world() {
    let (_, starts) = flashes(54.0, true);
    assert!(!starts.is_empty(), "the SMG fired and flashed");
    assert!(starts.iter().all(|s| !s.view_model && s.follow.is_none()), "{starts:?}");
}
