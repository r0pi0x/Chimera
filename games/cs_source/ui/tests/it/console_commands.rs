//! The client's console commands on a headless `Sim`: `getpos` and
//! `setpos` follow CS:S's conventions.

use bevy::prelude::*;
use crate::stack::{
    client::console::{HeldActions, client_commands},
    console::Console,
    core::LocalPlayer,
    games::cs_source::movement::{self, SourceMovementPlugin},
    greybox::GreyboxMapPlugin,
    harness::{CsSim, Sim},
};

/// CS:S's conventions (reference captures: `setpos x y z` puts the
/// camera at z + 64 standing, in noclip too; `getpos` printed the view's
/// origin, standing on a floor its height + 64): `getpos` prints the
/// eye, the movement's own view offset above the feet (64 standing, 47
/// ducked), and `setpos` takes the feet.
#[test]
fn getpos_prints_the_eye_and_setpos_takes_the_feet() {
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, SourceMovementPlugin));
    sim.app.init_resource::<HeldActions>();
    client_commands(&mut sim.app);
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    fn run(sim: &mut Sim, line: &str) -> String {
        sim.app.world_mut().resource_mut::<Console>().submit(line);
        sim.ticks(1);
        sim.app.world().resource::<Console>().output.last().map(|l| l.text.clone()).unwrap_or_default()
    }
    let getpos = |sim: &mut Sim| -> Vec3 {
        let line = run(sim, "getpos");
        let n: Vec<f32> = line
            .trim_start_matches("setpos ")
            .split(';')
            .next()
            .unwrap()
            .split_whitespace()
            .map(|w| w.parse().unwrap())
            .collect();
        Vec3::new(n[0], n[1], n[2])
    };
    let feet = |sim: &Sim| (sim.position(p).y + sim.state(p).hull_min.y) / 0.0254;
    sim.seconds(1.0);
    assert!(sim.state(p).on_ground);
    let floor = feet(&sim);
    let at = getpos(&mut sim);
    assert!((at.z - (floor + 64.0)).abs() < 0.02, "standing: {at}, floor {floor}");
    sim.intent(p).crouch = true;
    sim.seconds(1.0);
    let at = getpos(&mut sim);
    assert!((at.z - (feet(&sim) + 47.0)).abs() < 0.02, "ducked: {at} feet {}", feet(&sim));
    sim.intent(p).crouch = false;
    sim.seconds(1.0);
    // setpos on the floor: back where it was, the eye 64 up.
    run(&mut sim, &format!("setpos 10 20 {floor}"));
    let at = getpos(&mut sim);
    assert!(at.distance(Vec3::new(10.0, 20.0, floor + 64.0)) < 0.02, "{at}");
    assert!((feet(&sim) - floor).abs() < 0.02);
    // In noclip too (the dust2 view: the camera at 564).
    run(&mut sim, "noclip");
    run(&mut sim, "setpos -295 1078 500");
    let at = getpos(&mut sim);
    assert!(at.distance(Vec3::new(-295.0, 1078.0, 564.0)) < 0.02, "noclip: {at}");
}

/// The chase camera (the spectator's, `chimera:watchbot`; CS:S's
/// `thirdperson`) with `cam_idealdist 150`, as CS:S's `getpos` shows: the
/// camera sits 150 units straight back along the view from the eye.
#[test]
fn third_person_camera_sits_as_css_does() {
    use avian3d::prelude::SpatialQuery;
    use bevy::ecs::system::RunSystemOnce;
    use crate::stack::client::view::{CameraMode, THIRD_PERSON_PIVOT_UP, camera_offset};
    const U: f32 = 0.0254;
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, SourceMovementPlugin));
    // High over the greybox: nothing in the way.
    let origin = Vec3::new(0.0, 40.0, 12.0);
    let eye = Vec3::Y * 28.0 * U;
    sim.ticks(1);
    let mode = CameraMode {
        third_person: true,
        ..default()
    };
    let at = |sim: &mut Sim, pitch_down: f32, mode: CameraMode| -> Vec3 {
        let look = Quat::from_euler(EulerRot::YXZ, 0.0, -pitch_down.to_radians(), 0.0);
        sim.app
            .world_mut()
            .run_system_once(move |spatial: SpatialQuery| camera_offset(&mode, origin, eye, look, &spatial, []))
            .unwrap()
    };
    // Level: at the eye's height, 150 units back (CS:S's `getpos` in
    // third person at `setang 0`).
    let o = at(&mut sim, 0.0, mode);
    assert!(((o.y - eye.y) / U - THIRD_PERSON_PIVOT_UP).abs() < 0.01);
    assert!((((o.z - eye.z) / U) - 150.0).abs() < 0.1, "150 back");
    // Looking down: straight back along the view.
    let o = at(&mut sim, 15.4, mode);
    let above = (o.y - eye.y) / U;
    assert!((above - 150.0 * 15.4f32.to_radians().sin()).abs() < 0.3, "{above} units above the eye");
    let back = (o.z - eye.z) / U;
    assert!((back - 150.0 * 15.4f32.to_radians().cos()).abs() < 0.1, "{back} back");
}

/// Walking under a brush edge at head height (mg_jacks_multigames_v1's,
/// a playtest): once the top of the player's box is under the edge, the
/// eye drops below it, though the edge isn't over the eye itself yet (the
/// near plane cut into the brush and showed the map through it).
#[test]
fn eye_drops_under_a_brush_edge_over_the_box() {
    use avian3d::prelude::{Collider, RigidBody, SpatialQuery};
    use bevy::ecs::system::RunSystemOnce;
    use crate::stack::client::view::{CameraMode, Hull, camera_offset_for_hull};
    const U: f32 = 0.0254;
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, SourceMovementPlugin));
    // High over the greybox; the box's centre (origin) 31 units above its
    // feet, the eye 33 above that (CS:S: box 62, eye 64).
    let origin = Vec3::new(0.0, 40.0, 12.0);
    let eye = Vec3::Y * 33.0 * U;
    // The player's box, 32 x 62 units.
    let hull = Hull::of(&Collider::cuboid(32.0 * U, 62.0 * U, 32.0 * U));
    // A slab whose underside is 32.5 units above the origin (between the
    // box's top and the eye), 100 units wide; `edge` units from the
    // origin along +x.
    let slab = |sim: &mut Sim, edge: f32| {
        let w = 100.0 * U;
        sim.app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(w, 8.0 * U, w),
                Transform::from_translation(origin + Vec3::new(edge * U + w / 2.0, (32.5 + 4.0) * U, 0.0)),
            ))
            .id()
    };
    let at = |sim: &mut Sim| -> Vec3 {
        let mode = CameraMode::default();
        sim.app
            .world_mut()
            .run_system_once(move |spatial: SpatialQuery| {
                camera_offset_for_hull(&mode, origin, eye, Quat::IDENTITY, &spatial, [], Some(hull))
            })
            .unwrap()
    };
    // Its edge 20 units away: beside the box, the eye stays.
    let s = slab(&mut sim, 20.0);
    sim.ticks(2);
    assert!((at(&mut sim).y - eye.y).abs() < 1e-4, "edge beside the box: {}", at(&mut sim).y / U);
    sim.app.world_mut().despawn(s);
    // 10 units away: over the box's top (the box is 16 units wide each
    // side), not over the eye: the eye drops under the slab.
    let s = slab(&mut sim, 10.0);
    sim.ticks(2);
    let y = at(&mut sim).y / U;
    assert!(y < 32.5 - 3.0, "eye at {y} units, slab underside at 32.5");
    // A loose item (a dropped gun or the bomb, starting inside its
    // dropper's box) isn't a ceiling: the body doesn't collide with it
    // (playtest 2026-10-14: dropping the bomb standing still lowered the
    // view).
    sim.app.world_mut().despawn(s);
    let item = slab(&mut sim, -50.0);
    sim.app.world_mut().entity_mut(item).insert(avian3d::prelude::CollisionLayers::new(
        crate::stack::core::ITEM_LAYER,
        avian3d::prelude::LayerMask::ALL,
    ));
    sim.ticks(2);
    assert!((at(&mut sim).y - eye.y).abs() < 1e-4, "under a loose item: {}", at(&mut sim).y / U);
}

/// `kill` is CS:S's suicide: the player dies where it stands (god mode or
/// not), so a lone player can end the round, rather than being moved to
/// a spawn point alive.
#[test]
fn kill_is_a_suicide() {
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, SourceMovementPlugin));
    sim.app.init_resource::<HeldActions>();
    client_commands(&mut sim.app);
    let p = sim.spawn_character(Vec3::new(3.0, 1.0, 2.0), movement::ID);
    sim.app.world_mut().entity_mut(p).insert((LocalPlayer, crate::stack::core::God));
    sim.seconds(0.5);
    let before = sim.position(p);
    sim.app.world_mut().resource_mut::<Console>().submit("kill");
    sim.ticks(3);
    assert!(sim.app.world().get::<crate::stack::rules::Dead>(p).is_some(), "kill left the player alive");
    assert!(sim.position(p).distance(before) < 0.1, "kill moved the player");
}
