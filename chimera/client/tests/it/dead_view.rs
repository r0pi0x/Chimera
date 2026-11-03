//! A game's own dead view (`core::OrbitDeadView`: HL2:DM's,
//! specs/hl2dm/rules.md "The dead view") in place of CS:S's death and
//! freeze cams: while the local player is dead the camera sits `distance`
//! units behind the point `height` above its own eye (no ragdoll here)
//! along its view, which still turns; without the resource the death cam
//! is CS:S's.

use bevy::prelude::*;
use chimera_client::client::spectate::{SpecView, SpectateCameraPlugin};
use chimera_core::core::{Damage, Dead, Health, Hitgroup, LocalPlayer, MovementState, OrbitDeadView, SimClock};
use chimera_sim::SimBasePlugins;
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

const UNIT: f32 = 0.0254;

fn dead_view(orbit: bool) -> (Sim, Vec3, Option<(Vec3, Quat)>) {
    let mut sim = Sim::new(SimBasePlugins, (GreyboxMapPlugin, SpectateCameraPlugin));
    if orbit {
        sim.app.insert_resource(OrbitDeadView { distance: 96.0, height: 14.0, wall: 6.0 });
    }
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), chimera_sim::movement::placeholder::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(0.5);
    {
        let mut i = sim.intent(me);
        i.yaw = 0.3;
        i.pitch = -0.2;
    }
    sim.ticks(1);
    let eye = sim.position(me) + sim.app.world().get::<MovementState>(me).unwrap().eye_offset;
    let health = sim.app.world().get::<Health>(me).unwrap().current;
    sim.app.world_mut().write_message(Damage {
        force: Vec3::ZERO,
        target: me,
        attacker: None,
        amount: health + 1.0,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Chest,
        kind: Default::default(),
        weapon: None,
    });
    let now = sim.app.world().resource::<SimClock>().now;
    sim.app.world_mut().entity_mut(me).insert(Dead { since: now });
    sim.ticks(10);
    let pose = sim.app.world().resource::<SpecView>().pose;
    (sim, eye, pose)
}

#[test]
fn the_dead_look_at_themselves_from_96_units_behind() {
    let (_, eye, pose) = dead_view(true);
    let (at, look) = pose.expect("a dead view");
    let want_look = Quat::from_euler(EulerRot::YXZ, 0.3, -0.2, 0.0);
    assert!(look.angle_between(want_look) < 1e-3, "it looks along the view");
    // 96 units back along the view from the eye (open greybox: nothing in
    // the way), the eye being the point without a ragdoll.
    let want = eye + want_look * Vec3::Z * 96.0 * UNIT;
    assert!(at.distance(want) < 0.05, "camera at {at}, want {want}");
}

#[test]
fn without_it_the_death_cam_is_cs_sources() {
    let (_, eye, pose) = dead_view(false);
    // CS:S's: no killer here, so straight to watching (no one: no pose),
    // or its death cam near the eye; never 96 units back.
    if let Some((at, _)) = pose {
        assert!(at.distance(eye) < 1.5, "CS:S's view near the eye: {at} vs {eye}");
    }
}
