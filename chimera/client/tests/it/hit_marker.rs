//! The opt-in generic hit marker (`client::hit_marker`): off by default
//! (`cl_hitmarker 0`); on, a hit that dealt damage shows on the shooter
//! (host and network client both arrive as the shooter's own
//! `WeaponEvent::Hit`) with a headshot and a kill variant; nothing for the
//! victim, others' hits and kills, a zero-damage hit, a prop, or while
//! the HUD draws its own.

use bevy::prelude::*;
use chimera_client::client::{
    hit_marker::{HitKind, HitMarkerPlugin, HitMarkerSetting, HitMarkerView},
    hud::GameDraws,
};
use chimera_core::core::{Damage, DamageKind, Died, Health, Hitgroup, Intent, LocalPlayer};
use chimera_sim::{
    SimBasePlugins,
    weapon::{WeaponEvent, WeaponEventKind},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

struct World3 {
    sim: Sim,
    me: Entity,
    other: Entity,
    prop: Entity,
    breakable: Entity,
}

fn world(on: bool) -> World3 {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins((GreyboxMapPlugin, HitMarkerPlugin));
    });
    if on {
        sim.app.world_mut().resource_mut::<HitMarkerSetting>().on = 1;
    }
    let w = sim.app.world_mut();
    let me = w.spawn((LocalPlayer, Health::default(), Intent::default())).id();
    let other = w.spawn((Health::default(), Intent::default())).id();
    let prop = w.spawn_empty().id();
    // Door-like: has health but is no character (no `Intent`).
    let breakable = w.spawn(Health::default()).id();
    sim.ticks(2);
    World3 { sim, me, other, prop, breakable }
}

fn killing_blow() -> Damage {
    Damage {
        force: Vec3::ZERO,
        target: Entity::PLACEHOLDER,
        attacker: None,
        amount: 1.0,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
    }
}

fn hit(w: &mut World3, owner: Entity, target: Entity, amount: f32, hitgroup: Hitgroup) {
    w.sim.app.world_mut().write_message(WeaponEvent {
        owner,
        weapon: owner,
        kind: WeaponEventKind::Hit { target, amount, hitgroup },
        replay: false,
    });
    w.sim.ticks(1);
}

fn shown(w: &World3) -> (bool, HitKind) {
    let v = *w.sim.app.world().resource::<HitMarkerView>();
    (v.left > 0.0, v.kind)
}

#[test]
fn it_is_off_by_default() {
    let mut w = world(false);
    assert_eq!(w.sim.app.world().resource::<HitMarkerSetting>().on, 0);
    let (me, other) = (w.me, w.other);
    hit(&mut w, me, other, 30.0, Hitgroup::Head);
    assert!(!shown(&w).0);
}

#[test]
fn a_hit_headshot_and_kill_show_on_the_shooter() {
    let mut w = world(true);
    let (me, other) = (w.me, w.other);
    hit(&mut w, me, other, 30.0, Hitgroup::Chest);
    assert_eq!(shown(&w), (true, HitKind::Hit));
    // Fades out.
    w.sim.seconds(1.0);
    assert!(!shown(&w).0);
    hit(&mut w, me, other, 90.0, Hitgroup::Head);
    assert_eq!(shown(&w), (true, HitKind::Headshot));
    w.sim.seconds(1.0);
    hit(&mut w, me, other, 30.0, Hitgroup::Chest);
    w.sim.app.world_mut().write_message(Died {
        entity: other,
        attacker: Some(me),
        damage: killing_blow(),
    });
    w.sim.ticks(1);
    assert_eq!(shown(&w), (true, HitKind::Kill));
    // A later plain hit doesn't weaken it while it is up.
    hit(&mut w, me, other, 10.0, Hitgroup::Chest);
    assert_eq!(shown(&w).1, HitKind::Kill);
}

#[test]
fn nothing_for_the_victim_a_prop_no_damage_or_a_hud_with_its_own() {
    let mut w = world(true);
    let (me, other, prop) = (w.me, w.other, w.prop);
    // The victim: someone else's hit on me, my death by another.
    hit(&mut w, other, me, 30.0, Hitgroup::Head);
    w.sim.app.world_mut().write_message(Died {
        entity: me,
        attacker: Some(other),
        damage: killing_blow(),
    });
    w.sim.ticks(1);
    assert!(!shown(&w).0, "victim");
    hit(&mut w, me, prop, 30.0, Hitgroup::Generic);
    assert!(!shown(&w).0, "prop");
    let breakable = w.breakable;
    hit(&mut w, me, breakable, 30.0, Hitgroup::Generic);
    assert!(!shown(&w).0, "a breakable with health is no character");
    hit(&mut w, me, other, 0.0, Hitgroup::Chest);
    assert!(!shown(&w).0, "protected: no damage");
    w.sim.app.world_mut().insert_resource(GameDraws {
        hit_marker: true,
        ..default()
    });
    hit(&mut w, me, other, 30.0, Hitgroup::Chest);
    assert!(!shown(&w).0, "the HUD's game draws its own");
}

/// Others' fights (a host's bots shooting and killing each other) show
/// nothing on the local player's screen, with the marker on.
#[test]
fn nothing_for_others_hits_or_kills() {
    let mut w = world(true);
    let (me, other) = (w.me, w.other);
    let third = w.sim.app.world_mut().spawn((Health::default(), Intent::default())).id();
    w.sim.ticks(1);
    hit(&mut w, other, third, 30.0, Hitgroup::Head);
    hit(&mut w, third, other, 30.0, Hitgroup::Chest);
    w.sim.app.world_mut().write_message(Died {
        entity: third,
        attacker: Some(other),
        damage: killing_blow(),
    });
    w.sim.app.world_mut().write_message(Died {
        entity: other,
        attacker: None,
        damage: killing_blow(),
    });
    w.sim.ticks(1);
    assert!(!shown(&w).0, "others' hits and kills");
    hit(&mut w, me, other, 30.0, Hitgroup::Chest);
    assert_eq!(shown(&w), (true, HitKind::Hit), "mine still shows");
}
