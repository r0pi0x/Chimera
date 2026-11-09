//! The HUD fed by the running game (`ui::systems`, on `Hl2Sim` and the
//! greybox with the stand-in look): the secondary ammo panel's counts and
//! table (hud.md 4.6), target ID's ray (hud.md 11), the history's rows
//! from the server's messages (hud.md 9).

use bevy::prelude::*;
use chimera_core::core::{Health, Intent, LocalPlayer};
use chimera_sim::weapon::{Inventory, give};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use hl2dm::{
    game::{
        hud_notes::{HudPickup, PickupNote},
        movement::ID,
        rules::COMBINE,
        standing::{Hl2MatchView, Hl2Team},
        weapons::{self, launch::Hl2Rounds},
    },
    harness::Hl2Sim,
    ui::{
        frame::Draw,
        systems::{self, Hl2dmHud, HudCvars},
    },
};

use crate::common::base;

struct Fed {
    sim: Sim,
    me: Entity,
}

/// Me with the HUD's systems, far above the greybox's clutter.
fn fed() -> Fed {
    let mut sim = Sim::new(Hl2Sim, GreyboxMapPlugin);
    let (hud, look) = base();
    let mut state = Hl2dmHud::default();
    state.hud = Some(hud);
    state.look = Some(look);
    sim.app
        .insert_resource(state)
        .init_resource::<HudCvars>()
        .add_systems(Update, (systems::target_id, systems::collect).chain());
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.ticks(3);
    Fed { sim, me }
}

impl Fed {
    fn hold(&mut self, id: &str) -> Entity {
        let w = give(self.sim.app.world_mut(), self.me, id).unwrap();
        self.sim.app.world_mut().get_mut::<Inventory>(self.me).unwrap().wanted = Some(w);
        self.sim.ticks(10);
        w
    }

    fn state(&self) -> &Hl2dmHud {
        self.sim.app.world().resource::<Hl2dmHud>()
    }

    fn secondary(&self) -> Option<(String, u32)> {
        self.state().inputs.weapon.as_ref().and_then(|w| w.secondary.clone())
    }
}

/// The table in hud.md 4.6: SMG1 and AR2 show their second type, the
/// S.L.A.M. its count with no ammo panel, the frag and RPG theirs as ammo.
#[test]
fn the_secondary_ammo_table() {
    let mut f = fed();
    let smg = f.hold(weapons::SMG1);
    f.sim.app.world_mut().get_mut::<Hl2Rounds>(smg).unwrap().count = 2;
    f.sim.ticks(2);
    assert_eq!(f.secondary(), Some(("hl2dm:smg1_grenade".into(), 2)));
    assert!(f.state().inputs.weapon.as_ref().unwrap().uses_primary);
    // The panel shows: its count is drawn at r76 (a "2" elsewhere, such as
    // a selection number, doesn't count).
    f.sim.ticks(40);
    let hud = f.state().hud.as_ref().unwrap();
    assert_eq!(hud.anim.number("HudAmmoSecondary", "Alpha"), Some(255.0));
    let shown = f.state().frame.0.iter().any(|d| matches!(d, Draw::Text { text, at, .. } if text == "2" && at.x > 640.0 - 76.0));
    assert!(shown, "the panel's digits drawn");

    let ar2 = f.hold(weapons::AR2);
    f.sim.app.world_mut().get_mut::<Hl2Rounds>(ar2).unwrap().count = 1;
    f.sim.ticks(2);
    assert_eq!(f.secondary(), Some(("hl2dm:ar2altfire".into(), 1)));

    let slam = f.hold(weapons::SLAM);
    let count = f.sim.app.world().get::<Hl2Rounds>(slam).unwrap().count;
    assert_eq!(f.secondary(), Some(("hl2dm:slam".into(), count)));
    assert!(!f.state().inputs.weapon.as_ref().unwrap().uses_primary, "no ammo panel");

    let frag = f.hold(weapons::FRAG);
    f.sim.app.world_mut().get_mut::<Hl2Rounds>(frag).unwrap().count = 4;
    f.sim.ticks(2);
    let w = f.state().inputs.weapon.clone().unwrap();
    assert_eq!(f.secondary(), None);
    assert!(w.uses_primary && w.clip.is_none());
    assert_eq!(w.ammo_value(), 4);
}

/// Target ID's ray: another player straight ahead within 1500 units reads
/// "Enemy: name"; past it (test 63), or looking away, nothing.
#[test]
fn target_id_from_the_view_ray() {
    let mut f = fed();
    let units = 0.0254;
    let other = f.sim.spawn_character(Vec3::new(0.0, 1.0, -300.0 * units), ID);
    f.sim.app.world_mut().entity_mut(other).insert(Name::new("Bo"));
    let me = f.me;
    let look_at = |f: &mut Fed, yaw: f32| {
        f.sim.app.world_mut().get_mut::<Intent>(me).unwrap().yaw = yaw;
        f.sim.ticks(2);
    };
    look_at(&mut f, 0.0);
    let t = f.state().hud.as_ref().unwrap().notices.target.clone();
    assert_eq!(t.as_ref().map(|t| t.name.as_str()), Some("Bo"), "{t:?}");
    assert_eq!(t.unwrap().friend, None, "DM: every player an enemy");
    let drew = f.state().frame.0.iter().any(|d| matches!(d, Draw::Text { text, .. } if text.contains("Bo")));
    assert!(drew, "the line is drawn");

    // Looking away, it's gone at once.
    look_at(&mut f, std::f32::consts::PI);
    assert!(f.state().hud.as_ref().unwrap().notices.target.is_none());

    // Out of reach.
    look_at(&mut f, 0.0);
    f.sim.app.world_mut().get_mut::<Transform>(other).unwrap().translation.z = -1600.0 * units;
    f.sim.ticks(2);
    assert!(f.state().hud.as_ref().unwrap().notices.target.is_none(), "beyond 1500 units");

    // A dead one isn't a target.
    f.sim.app.world_mut().get_mut::<Transform>(other).unwrap().translation.z = -300.0 * units;
    f.sim.app.world_mut().get_mut::<Health>(other).unwrap().current = 0.0;
    f.sim.ticks(2);
    assert!(f.state().hud.as_ref().unwrap().notices.target.is_none(), "dead");
}

/// Test 62: a teammate in team play at 100 health reads "Friend: name
/// Health: 1%" at y 260 in its team's colour; `hud_centerid` 0 moves it.
#[test]
fn target_id_of_a_teammate() {
    let mut f = fed();
    let units = 0.0254;
    let other = f.sim.spawn_character(Vec3::new(0.0, 1.0, -600.0 * units), ID);
    let me = f.me;
    let world = f.sim.app.world_mut();
    world.entity_mut(other).insert((Name::new("Alex"), Hl2Team(COMBINE)));
    world.entity_mut(me).insert(Hl2Team(COMBINE));
    world.spawn(Hl2MatchView { teamplay: true, ..default() });
    world.get_mut::<Intent>(me).unwrap().yaw = 0.0;
    f.sim.ticks(2);
    match f.state().frame.find_text("Friend: Alex Health: 1%") {
        Some(Draw::Text { at, color, .. }) => assert_eq!((*at, *color), (Vec2::new(320.0, 260.0), [153, 204, 255, 255])),
        other => panic!("{other:?}"),
    }
    f.sim.app.world_mut().resource_mut::<HudCvars>().centerid = 0;
    f.sim.ticks(2);
    match f.state().frame.find_text("Friend: Alex Health: 1%") {
        Some(Draw::Text { at, .. }) => assert_eq!(*at, Vec2::new(10.0, 420.0)),
        other => panic!("{other:?}"),
    }
}

/// A wall between hides the target (the first thing the line hits isn't a
/// player); so does starting inside the wall.
#[test]
fn target_id_stops_at_the_world() {
    use avian3d::prelude::{Collider, RigidBody};
    let mut f = fed();
    let units = 0.0254;
    let other = f.sim.spawn_character(Vec3::new(0.0, 1.0, -300.0 * units), ID);
    f.sim.app.world_mut().entity_mut(other).insert(Name::new("Bo"));
    let me = f.me;
    f.sim.app.world_mut().get_mut::<Intent>(me).unwrap().yaw = 0.0;
    f.sim.ticks(2);
    assert!(f.state().hud.as_ref().unwrap().notices.target.is_some(), "clear at first");
    f.sim.app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(4.0, 4.0, 0.1),
        Transform::from_translation(Vec3::new(0.0, 1.0, -100.0 * units)),
    ));
    f.sim.ticks(3);
    assert!(f.state().hud.as_ref().unwrap().notices.target.is_none(), "behind a wall");
}

/// The history's rows come from the server's messages (tests 54, 56): a
/// battery's and a kit's class with icons (a vial with none adds nothing),
/// "FULL" for a refused type; others' notes aren't ours. A plain health
/// rise (a charger's) adds no row.
#[test]
fn history_rows_from_the_servers_messages() {
    let mut f = fed();
    f.sim.ticks(2);
    let (me, world) = (f.me, f.sim.app.world_mut());
    world.write_message(HudPickup { player: me, note: PickupNote::AmmoDenied("hl2dm:smg1".into()) });
    world.write_message(HudPickup { player: me, note: PickupNote::Item("item_healthvial".into()) });
    world.write_message(HudPickup { player: Entity::from_raw_u32(99999).unwrap(), note: PickupNote::AmmoDenied("hl2dm:ar2".into()) });
    world.get_mut::<Health>(me).unwrap().current = 0.4;
    f.sim.ticks(2);
    f.sim.app.world_mut().get_mut::<Health>(me).unwrap().current = 0.65;
    f.sim.ticks(2);
    let h = &f.state().hud.as_ref().unwrap().history;
    let denied: Vec<_> = h.entries.iter().filter(|e| e.denied).map(|e| e.ammo.clone().unwrap()).collect();
    assert_eq!(denied, vec!["hl2dm:smg1".to_string()]);
    assert_eq!(h.entries.iter().filter(|e| e.weapon.is_none() && !e.denied && e.ammo.is_none()).count(), 0, "no item row from health steps");

    // A battery and a kit name their classes: a row each, with their icons.
    let world = f.sim.app.world_mut();
    world.write_message(HudPickup { player: me, note: PickupNote::Item("item_battery".into()) });
    world.write_message(HudPickup { player: me, note: PickupNote::Item("item_healthkit".into()) });
    f.sim.ticks(2);
    let h = &f.state().hud.as_ref().unwrap().history;
    let icons: Vec<_> = h.entries.iter().filter(|e| e.weapon.is_none() && !e.denied && e.ammo.is_none()).map(|e| e.icon.map(|i| i.1)).collect();
    assert_eq!(icons, vec![Some('*'), Some('+')]);
}

/// Ammo that rises counts a weapon's own rounds too: a grenade picked up
/// is a history row of 1.
#[test]
fn own_rounds_rising_add_a_row() {
    let mut f = fed();
    let smg = f.hold(weapons::SMG1);
    f.sim.app.world_mut().get_mut::<Hl2Rounds>(smg).unwrap().count = 1;
    f.sim.ticks(2);
    f.sim.app.world_mut().get_mut::<Hl2Rounds>(smg).unwrap().count = 2;
    f.sim.ticks(2);
    let h = &f.state().hud.as_ref().unwrap().history;
    assert!(h.entries.iter().any(|e| e.ammo.as_deref() == Some("hl2dm:smg1_grenade") && e.count == Some(1)), "{:?}", h.entries);
}

/// hud.md 13: the scoreboard's label is the server's name (`hostname`, as
/// the message of the day brings it to a client).
#[test]
fn the_scoreboard_names_the_server() {
    let mut f = fed();
    f.sim
        .app
        .init_resource::<chimera_client::client::console::HeldActions>()
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(chimera_net::net::ServerMotd { server: "Our server".into(), ..Default::default() })
        .add_systems(Update, systems::collect_board);
    f.sim.ticks(2);
    assert_eq!(f.state().board.as_ref().unwrap().server_name, "Our server");
}
