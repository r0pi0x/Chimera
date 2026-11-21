//! The over-the-shoulder camera on a server (docs/plans/active/
//! third-person-camera.md, M2) on `NetSim` and the test kit's guns: with
//! `chimera:allow_thirdperson 1` a client in third person aims through its
//! crosshair, its shot hits what the crosshair shows (the client's
//! predicted shot and the server's damage), the server taking only the
//! command's angles as ever; with `0` (the default) the client can't enter
//! third person and aims as in first person.

use std::time::Duration;

use bevy::prelude::*;
use chimera_client::client::{
    input::FreeLook,
    third_person::{Shoulder, ShoulderSettings, ThirdPersonPlugin, angles_of, camera_from_eye, wanted_offsets},
    view::CameraMode,
};
use chimera_core::{
    console::Console,
    core::{Health, Intent, MovementState, ThirdPersonRules},
    slots::Loadout,
};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_sim::{
    SimBasePlugins, movement::placeholder, testkit,
    weapon::{WeaponEvent, WeaponEventKind},
};
use chimera_world::greybox::GreyboxMapPlugin;

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, ThirdPersonPlugin))
        .init_resource::<CameraMode>()
        .init_resource::<FreeLook>()
        .insert_resource(Loadout {
            movement: placeholder::ID,
        });
    testkit::add_weapons(app);
}

/// A server (third person allowed or not) with two joined clients, client
/// 1's player standing in front of client 0's, a little right of its eye.
fn joined(allow: bool) -> (NetSim, Entity) {
    let mut sim = NetSim::new(
        SimBasePlugins,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        7,
        2,
        setup,
    );
    if allow {
        sim.server.app.world_mut().resource_mut::<Console>().submit("chimera:allow_thirdperson 1");
    }
    sim.until_joined(900);
    sim.until_looks_taken(300);
    let a = sim.character_of(0).unwrap();
    let b = sim.character_of(1).unwrap();
    chimera_sim::weapon::give(sim.server.app.world_mut(), a, testkit::RIFLE).expect("the kit's rifle");
    let front = sim.server.position(a) + Vec3::new(ShoulderSettings::default().side, 0.0, -5.0);
    sim.server.app.world_mut().get_mut::<Transform>(b).unwrap().translation = front;
    sim.ticks(150);
    let id = sim.client_id(1);
    let b_on_0 = NetSim::owned_by(&mut sim.clients[0], Some(id)).expect("client 0's copy of b");
    (sim, b_on_0)
}

/// Client 0's camera look that puts the crosshair on `target`: the camera
/// sits by `wanted_offsets`, so iterate (the spot turns with the look).
fn look_at(sim: &NetSim, target: Vec3, me: Entity) -> Vec2 {
    let c = &sim.clients[0];
    let eye = c.position(me) + c.app.world().get::<MovementState>(me).unwrap().eye_offset;
    let shoulder = c.app.world().resource::<Shoulder>();
    let settings = ShoulderSettings::default().scaled(shoulder.scale());
    let mut look = angles_of(target - eye);
    for _ in 0..6 {
        let rot = Quat::from_euler(EulerRot::YXZ, look.x, look.y, 0.0);
        let (pivot, dist) = wanted_offsets(&settings, shoulder.side, 0.0, rot);
        let cam = eye + camera_from_eye(pivot, dist, rot, 0.0);
        look = angles_of(target - cam);
    }
    look
}

#[test]
fn on_a_server_that_allows_it_the_shot_hits_what_the_crosshair_shows() {
    let (mut sim, b_on_0) = joined(true);
    let me = sim.local_player(0).unwrap();
    assert!(sim.until(200, |s| s.clients[0].app.world().resource::<ThirdPersonRules>().allow == 1), "the rule reaches the client");
    sim.clients[0].app.world_mut().resource_mut::<CameraMode>().third_person = true;
    assert!(
        sim.until(200, |s| s.clients[0].app.world().resource::<Shoulder>().blend >= 1.0),
        "the shoulder camera is on"
    );
    let b = sim.character_of(1).unwrap();
    let before = sim.server.app.world().get::<Health>(b).unwrap().current;
    let mut cursor = sim.clients[0].app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut first = None;
    for k in 0..14 {
        let target = sim.clients[0].position(b_on_0) + Vec3::Y * 0.2;
        let look = look_at(&sim, target, me);
        let c = &mut sim.clients[0];
        c.app.world_mut().resource_mut::<Shoulder>().look = Some(look);
        c.app.world_mut().get_mut::<Intent>(me).unwrap().fire = k < 8;
        sim.step();
        for e in cursor.read(sim.clients[0].app.world().resource::<Messages<WeaponEvent>>()) {
            if e.owner == me && e.shown() && let WeaponEventKind::Shot { hit, .. } = e.kind {
                first.get_or_insert(hit);
            }
        }
    }
    // (The rifle's recoil walks the later shots off; the first is the crosshair's.)
    assert_eq!(first, Some(Some(b_on_0)), "the client's first shot hits what the crosshair shows");
    sim.ticks(20);
    assert!(sim.server.app.world().get::<Health>(b).unwrap().current < before, "and the server deals the damage");
    // The command carried the eye-to-target angles, not the camera's.
    let i = sim.clients[0].app.world().get::<Intent>(me).unwrap();
    assert!(i.yaw != 0.0 || i.pitch != 0.0);
}

#[test]
fn on_a_server_that_forbids_it_the_client_stays_in_first_person() {
    let (mut sim, _) = joined(false);
    let me = sim.local_player(0).unwrap();
    sim.clients[0].app.world_mut().resource_mut::<CameraMode>().third_person = true;
    sim.ticks(60);
    let w = sim.clients[0].app.world();
    assert_eq!(w.resource::<ThirdPersonRules>().allow, 0);
    assert!(w.resource::<CameraMode>().held_off, "held off by the server's rule");
    let s = w.resource::<Shoulder>();
    assert!(!s.on && !s.drawn() && s.look.is_none(), "no shoulder camera");
    // The mouse's look is the aim as in first person: nothing turns it.
    let yaw = w.get::<Intent>(me).unwrap().yaw;
    sim.ticks(10);
    assert_eq!(sim.clients[0].app.world().get::<Intent>(me).unwrap().yaw, yaw);
}
