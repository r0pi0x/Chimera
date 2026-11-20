//! A first-person muzzle flash is a world effect (specs/tf2/effects.md 2,
//! particles.md 6): a shot of a TF2 gun whose view model is drawn starts
//! its flash at the view model's muzzle re-projected into the world view
//! (right and up x tan(world FOV/2) / tan(view-model FOV/2)), following
//! the eye, drawn with the world camera (before, the view-model camera
//! drew it, 1.96x too big at FOV 90); with `r_drawviewmodel 0` there is
//! none. In third person the flash is the held model's, in the world.
//! Tracers (effects.md 6) start at the same re-projected muzzle and head
//! for the bullet's end. This machine's own player's flashes ride the
//! animated muzzle: they follow its `AttachmentMark`, placed each frame
//! where the attachment appears. A made-up view model with a `muzzle`
//! attachment on the greybox (`harness::Tf2Sim`).

use bevy::prelude::*;
use chimera_world::map::{
    attached::{AttachPoint, AttachmentMark},
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
    gun_turned(key, Quat::IDENTITY)
}

/// `gun` with its muzzle attachment turned `turn` in the model.
fn gun_turned(key: &str, turn: Quat) -> MapViewModel {
    MapViewModel {
        key: key.into(),
        model: MapModel::default(),
        bones: vec![MapBone { name: "root".into(), parent: None, position: Vec3::ZERO, rotation: Quat::IDENTITY }],
        root: Transform::IDENTITY,
        animations: None,
        right_handed: true,
        allow_flipping: false,
        attachments: vec![MapAttachment { name: "muzzle".into(), bone: 0, local: Transform::from_translation(MUZZLE).with_rotation(turn) }],
        light_origin: Vec3::ZERO,
    }
}

/// The flashes `p`'s SMG starts over a burst, with the view-model camera
/// at `vm_fov` degrees (the world's at 90), the local body shown or not.
fn flashes(vm_fov: f32, third_person: bool) -> (Entity, Vec<StartParticleSystem>) {
    let (_, anchor, starts) = flashes_of(vm_fov, third_person, false);
    (anchor, starts)
}

/// `flashes`, `local`: the shooter this machine's own player; with the sim.
fn flashes_of(vm_fov: f32, third_person: bool, local: bool) -> (chimera_world::harness::Sim, Entity, Vec<StartParticleSystem>) {
    flashes_with(gun(SMG), vm_fov, third_person, local)
}

/// `flashes_of` with the view model `model` (keyed as the SMG).
fn flashes_with(model: MapViewModel, vm_fov: f32, third_person: bool, local: bool) -> (chimera_world::harness::Sim, Entity, Vec<StartParticleSystem>) {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), SMG);
    if local {
        sim.app.world_mut().entity_mut(p).insert(chimera_core::core::LocalPlayer);
        // The map's drawing places the marks (not in a headless sim).
        sim.app.add_systems(Update, chimera_world::map::attached::place_marks.in_set(chimera_world::map::attached::MarksPlaced));
    }
    sim.app.world_mut().insert_resource(ViewModels::of([model]));
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
    (sim, anchor, out)
}

/// The own player's flash follows its muzzle's mark (riding the gun's
/// kick, TF2's lock to the view model's attachment), which sits where the
/// muzzle appears in the world view, turned as the eye.
#[test]
fn the_own_players_flash_rides_the_muzzles_mark() {
    let (mut sim, anchor, starts) = flashes_of(54.0, false, true);
    let flashes: Vec<_> = starts.iter().filter(|s| s.name.starts_with("muzzle_")).collect();
    assert!(!flashes.is_empty(), "the SMG fired and flashed");
    let world = sim.app.world_mut();
    let mut marks = world.query::<(Entity, &AttachmentMark, &Transform)>();
    let (mark, m, at) = marks
        .iter(world)
        .find(|(_, m, _)| m.at == AttachPoint::named("muzzle"))
        .map(|(e, m, t)| (e, m.clone(), *t))
        .expect("a mark on the muzzle");
    assert!(m.shown, "placed this frame");
    let eye = *world.get::<GlobalTransform>(anchor).expect("the eye");
    let want = eye.transform_point(reproject(MUZZLE, 90.0, 54.0));
    assert!(at.translation.abs_diff_eq(want, 1e-3), "at the muzzle as it appears: {} vs {want}", at.translation);
    let ridden = flashes.iter().filter(|s| s.follow.is_some_and(|f| f.entity == mark && f.local == Vec3::ZERO && f.carry)).count();
    // The first shot may come before the mark was placed: it follows the eye.
    assert!(ridden + 1 >= flashes.len() && ridden > 0, "{ridden} of {} on the mark: {flashes:?}", flashes.len());
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

/// A flash turns as its attachment does (Source starts a system with the
/// attachment's orientation; TF2's Grenade Launcher's muzzle has its z
/// along the barrel and its flash's sparks leave along z: turned only by
/// its x toward the aim they went straight up): a muzzle turned half
/// round about the eye's up starts a flash whose x is the eye's left
/// (bevy -X) and whose z is the eye's forward (bevy -Z), in the eye's frame.
#[test]
fn a_flash_turns_as_its_attachment() {
    let turn = Quat::from_rotation_y(std::f32::consts::PI);
    let (_, _, starts) = flashes_with(gun_turned(SMG, turn), 54.0, false, false);
    let flashes: Vec<_> = starts.iter().filter(|s| s.name.starts_with("muzzle_")).collect();
    assert!(!flashes.is_empty(), "the SMG fired and flashed");
    for s in flashes {
        let f = s.follow.expect("follows the eye");
        assert!(f.forward.abs_diff_eq(Vec3::NEG_X, 1e-4), "x: {}", f.forward);
        let up = f.up.expect("its whole orientation");
        assert!(up.abs_diff_eq(Vec3::NEG_Z, 1e-4), "z along the barrel: {up}");
    }
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
