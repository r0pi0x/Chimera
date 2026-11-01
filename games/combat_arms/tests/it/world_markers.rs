//! specs/combat_arms/world_objects.md Follow-up (2026-10-31b) 1.6: the
//! `TargetTrigger` markers' icons from the trigger resource table
//! (`game::objects::markers`): cases 35 (by team, the distance text) and
//! 37 (by name: no record, no icon). The install's table: `heavy::
//! world_objects_2`.

use bevy::prelude::*;
use chimera_core::core::{LocalPlayer, NetRole, Team};
use chimera_world::harness::Sim;
use combat_arms::game::objects::{
    CaObjects,
    events::MarkerTeam,
    markers::{self, CaMarkers, TriggerResources, distance_text, guide_alpha, icons},
};
use lithtech_formats::attributes::Document;

use crate::world_objects::{T, obj, v};

const TABLE: &[u8] = br#"
[Trigger0] Name="Indicator_A00" CheckTeam=1 ShowDist=1 AllyTargetGuideTex="D" EnemyTargetGuideTex="A" TargetLargeMapText="M"
[Trigger1] Name="Indicator_EZone_unable" ShowDist=1 TargetGuideTex="UI_HUD_Common\Guide\Guide_area_Exit_notOpen.DTX" TargetLargeMapText="UI_HUD_Map\icon\area_Exit_notOpen.dtx"
[Trigger2] Name="indicator_a00" TargetGuideTex="lower case"
"#;

fn table() -> TriggerResources {
    TriggerResources::parse(&Document::parse(TABLE).unwrap())
}

/// 35. A `CheckTeam` record on a JSF marker: a JSF viewer gets guide D, a
/// Creed viewer guide A, both map icon M; the guide reads `12 M` at 500 u.
/// A `<none>` marker: every viewer gets A. Blocked: alpha 125.
#[test]
fn t35_marker_icons() {
    let t = table();
    assert_eq!(t.0.len(), 3);
    let r = t.find("Indicator_A00").expect("by its exact name");
    let jsf = MarkerTeam::Team("JSF".into());
    let a = icons(r, &jsf, Some(1));
    assert_eq!((a.guide.as_deref(), a.map.as_deref(), a.show_dist), (Some("D"), Some("M"), true));
    let e = icons(r, &jsf, Some(2));
    assert_eq!((e.guide.as_deref(), e.map.as_deref()), (Some("A"), Some("M")));
    for viewer in [Some(1), Some(2), None] {
        assert_eq!(icons(r, &MarkerTeam::Nobody, viewer).guide.as_deref(), Some("A"), "{viewer:?}");
    }
    assert_eq!(distance_text(500.0), "12 M");
    assert_eq!((guide_alpha(false), guide_alpha(true)), (255, 125));
    // Without CheckTeam: the plain pictures for everyone.
    let z = t.find("Indicator_EZone_unable").unwrap();
    assert_eq!(icons(z, &jsf, Some(2)).guide.as_deref(), Some("UI_HUD_Common\\Guide\\Guide_area_Exit_notOpen.DTX"));
    // Case counts in the name.
    assert_eq!(t.find("indicator_a00").unwrap().guide, "lower case");
}

/// The object world of `objects` in a bare ECS world as a network client
/// (the server's word decides which markers are on), its local player on
/// team `team`.
fn client(t: T, team: u8) -> World {
    let mut w = World::new();
    let mut o = CaObjects::default();
    o.world = t.w;
    o.scale = 0.018;
    w.insert_resource(o);
    w.insert_resource(table());
    w.insert_resource(NetRole::Client);
    w.spawn((LocalPlayer, Team(team)));
    w
}

/// 37. A marker without a record (`WeaponSpawnMarker00`) turned on: no
/// guide and no map icon. One with a record: its icons, at its place; its
/// own texture properties are never used.
#[test]
fn t37_marker_name_lookup() {
    let mut t = T::new(vec![
        obj("TargetTrigger", "WeaponSpawnMarker00", &[("HUDTexture", "x.dtx".into()), ("origin", v(0.0, 0.0, 0.0))]),
        obj(
            "TargetTrigger",
            "Indicator_A00",
            &[("Team", "JSF".into()), ("HUDTexture", "x.dtx".into()), ("origin", v(100.0, 0.0, 0.0))],
        ),
    ]);
    t.send("WeaponSpawnMarker00", "ON");
    t.send("Indicator_A00", "ON");
    let w = client(t, 2);
    let shown = &markers::shown(w.resource::<CaObjects>(), w.resource::<TriggerResources>(), true, None, Some(2)).0;
    assert_eq!(shown.len(), 1, "{shown:?}");
    assert_eq!(shown[0].name, "Indicator_A00");
    assert_eq!(shown[0].icons.guide.as_deref(), Some("A"), "the enemy's picture");
    assert!((shown[0].at.x - 1.8).abs() < 1e-4, "at its place: {}", shown[0].at);
}

/// The small world with a VIP Escort marker, a TDM one and a camera.
fn marked_world() -> lithtech_formats::world::World {
    use lithtech_formats::world::{Object, Value};
    let mut w = crate::world::small_world();
    let prop = crate::world::prop;
    for (name, game_type) in [("Indicator_A00", "VIPEscort"), ("Indicator_EZone_unable", "HeadQuaters")] {
        w.objects.push(Object {
            class: "TargetTrigger".into(),
            properties: vec![
                prop("Name", Value::String(name.into())),
                prop("Pos", Value::Vector([0.0, 50.0, 200.0])),
                prop("GameType", Value::String(game_type.into())),
                prop("Team", Value::String("JSF".into())),
            ],
        });
    }
    w.objects.push(Object {
        class: "Camera".into(),
        properties: vec![
            prop("Name", Value::String("Camera0".into())),
            prop("Pos", Value::Vector([0.0, 100.0, 0.0])),
            prop("FovY", Value::Real(75.0)),
        ],
    });
    w
}

/// 36 and 1.7 over the network: the server switches both markers on in
/// VIP Escort: a client hears of the `VIPEscort` one only (and shows its
/// record's icons), not of the `HeadQuaters` one; a camera's live flag
/// and field of view reach the client, a late joiner too.
#[test]
fn clients_get_markers_of_the_mode_and_cameras() {
    use std::time::Duration;

    use chimera_net::{harness::NetSim, net::memory::LinkConditions};
    use chimera_world::map::MapSimPlugin;
    use combat_arms::{
        game::{TICK_INTERVAL, objects::{CaObjectMessage, runtime::Kind}, world},
        harness::{CaSim, with_mode},
    };
    let map = world::convert(&marked_world(), "marked", 0.01, &|_| None);
    let link = LinkConditions {
        latency: Duration::from_millis(40),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 3, 1, move |app| {
        app.add_plugins(MapSimPlugin::new(map.clone()));
        app.insert_resource(table());
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    with_mode(sim.server.app.world_mut(), "ESC");
    sim.ticks(30);
    for (target, message) in [("Indicator_A00", "on"), ("Indicator_EZone_unable", "on"), ("Camera0", "on"), ("Camera0", "fov 55 41.25 2")] {
        sim.server.app.world_mut().write_message(CaObjectMessage {
            target: target.into(),
            message: message.into(),
        });
    }
    sim.ticks(30);
    let seen = |app: &mut App| {
        let w = app.world_mut();
        let names: Vec<String> = w.resource::<CaMarkers>().0.iter().map(|m| m.name.clone()).collect();
        let o = w.resource::<CaObjects>();
        let cam = o.named("Camera0").unwrap();
        let Kind::Camera(c) = &o.world.objects[cam as usize].kind else { panic!() };
        (names, c.live, c.fov)
    };
    assert_eq!(seen(&mut sim.server.app), (vec!["Indicator_A00".to_string()], true, Vec2::new(55.0, 41.25)));
    assert_eq!(seen(&mut sim.clients[0].app), (vec!["Indicator_A00".to_string()], true, Vec2::new(55.0, 41.25)), "the client");
    sim.add_client(|w| {
        w.insert_resource(table());
    });
    sim.until_joined(600);
    sim.ticks(30);
    assert_eq!(seen(&mut sim.clients[1].app).0, vec!["Indicator_A00".to_string()], "a late joiner");
}

/// 1.6, the guide's line test: a marker 5 m in front of the local player
/// is drawn opaque while nothing is between them; a wall across the line
/// dims it to alpha 125, and taking the wall away brings it back. A
/// floor under the point (the marker on it) doesn't block.
#[test]
fn guide_dims_when_its_line_is_blocked() {
    use combat_arms::game::{movement, objects::geometry::from_engine};
    let mut sim = crate::special_weapons::sim();
    let scale = movement::movement_data().meters_per_unit;
    let p = sim.spawn_character(Vec3::new(0.0, 0.05, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(p).insert((LocalPlayer, Team(2)));
    sim.seconds(0.5);
    let feet = sim.position(p);
    let at = |x: f32, y: f32, z: f32| {
        let q = from_engine(Vec3::new(x, y, z), scale);
        v(q.x, q.y, q.z)
    };
    let mut t = T::new(vec![
        obj("TargetTrigger", "Indicator_A00", &[("Team", "JSF".into()), ("origin", at(0.0, feet.y + 1.5, -5.0))]),
        obj("TargetTrigger", "Indicator_EZone_unable", &[("origin", at(3.0, 0.0, -5.0))]),
    ]);
    t.send("Indicator_A00", "ON");
    t.send("Indicator_EZone_unable", "ON");
    {
        let w = sim.app.world_mut();
        let mut o = w.resource_mut::<CaObjects>();
        o.world = t.w;
        o.scale = scale;
        w.insert_resource(table());
        w.insert_resource(NetRole::Client);
    }
    let blocked = |sim: &mut Sim| -> Vec<(String, bool, u8)> {
        sim.ticks(2);
        let m = sim.app.world().resource::<CaMarkers>();
        m.0.iter().map(|s| (s.name.clone(), s.blocked, guide_alpha(s.blocked))).collect()
    };
    let open = blocked(&mut sim);
    assert_eq!(
        open,
        [("Indicator_A00".to_string(), false, 255), ("Indicator_EZone_unable".to_string(), false, 255)],
        "nothing in the way; the one on the floor isn't blocked by it"
    );
    let wall = crate::special_weapons::solid(&mut sim, Vec3::new(-1.0, 0.0, -2.6), Vec3::new(1.0, 4.0, -2.4), None);
    let shut = blocked(&mut sim);
    assert_eq!((shut[0].1, shut[0].2), (true, 125), "the wall blocks the first: {shut:?}");
    assert!(!shut[1].1, "not the second, beside the wall: {shut:?}");
    sim.app.world_mut().despawn(wall);
    sim.ticks(2);
    assert!(!blocked(&mut sim)[0].1, "open again");
}
