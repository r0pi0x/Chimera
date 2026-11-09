//! HL2:DM's crowbar and stunstick on the headless harness
//! (specs/hl2dm/weapons.md 11, its cases 32-35): 25 and 40 a hit, every
//! 0.4 and 0.8 s hit or miss, the 75-unit line, the box swept short of it
//! counting only a target whose origin is within 45 degrees of the view,
//! no head bonus, the stunstick's deploy sound, nothing dropped.

use bevy::prelude::*;
use chimera_core::core::Health;
use chimera_sim::{
    testkit,
    weapon::{Inventory, Melee, Weapon, WeaponEvent, WeaponEventKind, drop::Undroppable},
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{
        DecalGroup, MapSounds, MapSurface, PlaceDecal, PlaySound,
        sound::SoundBank,
    },
};
use hl2dm::{
    game::{movement, weapons},
    harness::Hl2Sim,
};

/// The greybox on HL2:DM's harness, a fixed tick per update at
/// HL2:DM's 0.015 s.
fn sim() -> Sim {
    let mut sim = Sim::new(Hl2Sim, GreyboxMapPlugin);
    sim.set_tick_interval(hl2dm::game::TICK_INTERVAL);
    sim
}

/// A player holding `id` and a target `ahead` metres in front of it and
/// `aside` to its right (its yaw 0 looks along -Z).
fn duel(id: &str, ahead: f32, aside: f32) -> (Sim, Entity, Entity) {
    let mut sim = sim();
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    let w = sim.app.world().get::<Inventory>(p).unwrap().weapons.iter().copied().find(|e| {
        sim.app.world().get::<Weapon>(*e).is_some_and(|x| x.id == id)
    });
    match w {
        Some(w) => sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w),
        None => {
            chimera_sim::weapon::give(sim.app.world_mut(), p, id).unwrap();
        }
    }
    let t = sim.spawn_character(greybox::SPAWNS[0] + Vec3::new(aside, 0.0, -ahead), movement::ID);
    sim.seconds(1.5);
    let mut i = sim.intent(p);
    i.yaw = 0.0;
    i.pitch = 0.0;
    (sim, p, t)
}

/// Hold `t` where it stands: each hit shoves it 5 units/s a point
/// (projectiles.md 3) and on Source movement it slides out of reach; the
/// spec's kill cases (32, 33) count hits on a target that stays.
fn pin(sim: &mut Sim, t: Entity) {
    sim.app.world_mut().entity_mut(t).insert(chimera_core::core::MapControls {
        time_scale: 0.0,
        ..Default::default()
    });
}

/// Swing with attack held for `n` ticks: the ticks of the swings and
/// whether each hit.
fn swing(sim: &mut Sim, p: Entity, n: u64) -> Vec<(u64, bool)> {
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut out = Vec::new();
    for t in 0..n {
        sim.intent(p).fire = true;
        sim.ticks(1);
        for e in cursor.read(sim.app.world().resource::<Messages<WeaponEvent>>()) {
            if let WeaponEventKind::Swing { hit, .. } = e.kind
                && e.owner == p
                && e.shown()
            {
                out.push((t, hit));
            }
        }
    }
    sim.intent(p).fire = false;
    out
}

/// Effects 6: a crowbar's or stunstick's hit on the world is the generic
/// impact: the surface's bullet-impact sound and the surface's decal, as a
/// bullet's (not CS:S's knife slash, and not silent).
#[test]
fn melee_hits_make_the_surfaces_impact_and_decal() {
    for (id, extra) in [(weapons::CROWBAR, "Weapon_Crowbar.Single"), (weapons::STUNSTICK, "Weapon_StunStick.Melee_Hit")] {
        let (mut sim, p, _) = duel(id, 30.0, 0.0);
        let mut bank = MapSounds::default();
        bank.surfaces.insert(
            "default".into(),
            MapSurface { bullet_impact: Some("Default.BulletImpact".into()), game_material: 'C', ..default() },
        );
        sim.app.insert_resource(SoundBank(std::sync::Arc::new(bank)));
        // At the floor, well within the 75-unit line.
        sim.intent(p).pitch = -70f32.to_radians();
        let mut sounds = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
        let mut decals = sim.app.world().resource::<Messages<PlaceDecal>>().get_cursor_current();
        let (mut heard, mut marks) = (Vec::new(), Vec::new());
        sim.intent(p).fire = true;
        for _ in 0..3 {
            sim.ticks(1);
            let w = sim.app.world();
            heard.extend(sounds.read(w.resource::<Messages<PlaySound>>()).map(|s| s.entry.clone()));
            marks.extend(decals.read(w.resource::<Messages<PlaceDecal>>()).cloned());
        }
        sim.intent(p).fire = false;
        assert!(heard.iter().any(|e| e == "Default.BulletImpact"), "{id}: {heard:?}");
        assert!(heard.iter().any(|e| e == extra), "{id}: {heard:?}");
        assert_eq!(marks.len(), 1, "{id}: {marks:?}");
        assert!(matches!(marks[0].group, DecalGroup::Material('C')), "{id}: {:?}", marks[0].group);
        // And loaded with the map: what isn't plays nothing.
        assert!(hl2dm::game::sounds::entries().iter().any(|e| e == extra), "{id}");
    }
}

fn health(sim: &Sim, e: Entity) -> f32 {
    sim.app.world().get::<Health>(e).unwrap().current * 100.0
}

#[test]
fn crowbar_kills_in_four_hits_27_ticks_apart_32() {
    let (mut sim, p, t) = duel(weapons::CROWBAR, 1.2, 0.0);
    pin(&mut sim, t);
    let swings = swing(&mut sim, p, 90);
    let hits: Vec<u64> = swings.iter().filter(|(_, h)| *h).map(|(t, _)| *t).collect();
    assert_eq!(&hits[..4], [0, 27, 54, 81], "{swings:?}");
    assert!(health(&sim, t) <= 0.0);
}

#[test]
fn stunstick_kills_in_three_hits_54_ticks_apart_33() {
    let (mut sim, p, t) = duel(weapons::STUNSTICK, 1.2, 0.0);
    pin(&mut sim, t);
    let swings = swing(&mut sim, p, 110);
    let hits: Vec<u64> = swings.iter().filter(|(_, h)| *h).map(|(t, _)| *t).collect();
    assert_eq!(&hits[..3], [0, 54, 108], "{swings:?}");
    assert!(health(&sim, t) <= 0.0);
}

#[test]
fn a_hit_in_front_does_25_whatever_the_part_34() {
    let (mut sim, p, t) = duel(weapons::CROWBAR, 1.2, 0.0);
    // At the head height: no head bonus (generic hitgroup).
    let head = sim.position(t) + Vec3::Y * 0.62;
    testkit::aim_at(&mut sim, p, head);
    let swings = swing(&mut sim, p, 1);
    assert_eq!(swings, [(0, true)]);
    sim.ticks(1);
    assert!((health(&sim, t) - 75.0).abs() < 1e-3, "{}", health(&sim, t));
}

#[test]
fn a_miss_swings_at_the_same_rate_and_the_box_needs_the_target_in_front_35() {
    // Far beyond the 75-unit line: misses every 27 ticks.
    let (mut sim, p, _) = duel(weapons::CROWBAR, 3.0, 0.0);
    let swings = swing(&mut sim, p, 60);
    assert_eq!(swings, [(0, false), (27, false), (54, false)]);
    // Beside the line, the box reaches it but its origin is more than 45
    // degrees off the view (the spec's case 35 has the feet as the origin;
    // the placeholder's is its middle, so the target stands farther aside
    // here): no hit.
    let (mut sim, p, t) = duel(weapons::CROWBAR, 0.6, 0.9);
    let before = health(&sim, t);
    assert_eq!(swing(&mut sim, p, 1), [(0, false)]);
    assert_eq!(health(&sim, t), before);
    // Within 45 degrees though the line misses (16 units of box): a hit.
    let (mut sim, p, t) = duel(weapons::CROWBAR, 1.0, 0.55);
    assert_eq!(swing(&mut sim, p, 1), [(0, true)], "{} m aside", 0.55);
    sim.ticks(1);
    assert!((health(&sim, t) - 75.0).abs() < 1e-3);
}

#[test]
fn melee_weapons_are_never_dropped_44() {
    for id in [weapons::CROWBAR, weapons::STUNSTICK] {
        let mut sim = sim();
        let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
        let e = chimera_sim::weapon::give(sim.app.world_mut(), p, id).unwrap();
        assert!(sim.app.world().get::<Undroppable>(e).is_some(), "{id}");
        assert!(sim.app.world().get::<Melee>(e).is_some_and(|m| !m.primary.hitgroups && m.secondary.is_none()));
    }
}
