//! A party's spawn protection (`core::SpawnProtected`, its bullets
//! window: user, 2026-10-29) with CS:S's guns, knife and grenades on the
//! greybox: the protected player isn't there for attacks. A round passes
//! through it to the wall behind (its impact decal there, nothing on the
//! player: no blood, flesh impact, flinch or slowdown) or into the player
//! behind it, who takes the hit; an HE in flight passes through it, and a
//! blast next to it deals it nothing and leaves its ears alone.

use bevy::prelude::*;

use crate::stack::{
    core::{Deafened, God, Health, SimClock, SpawnProtected},
    games::cs_source::{
        grenades::HEGRENADE,
        impact_effects::MATERIALS,
        movement::QuakeMovement,
    },
    greybox,
    harness::Sim,
    map::{
        DecalGroup, MapSounds, MapSurface, PlaceDecal, sound::SoundBank,
        particles::{ParticleMaterial, ParticleMaterials, MapParticles},
    },
    movement::placeholder,
    weapon::{Inventory, ViewPunch, Weapon, WeaponEvent, WeaponEventKind, give, grenade::{Projectile, spawn_projectile}},
};

fn sim() -> Sim {
    let mut sim = crate::common::sim();
    let materials = MATERIALS.iter().map(|n| ParticleMaterial { name: n.to_string(), ..default() }).collect();
    sim.app.insert_resource(ParticleMaterials(MapParticles { materials }));
    // Concrete walls, flesh characters (their surfaces' decals and impacts).
    let mut sounds = MapSounds::default();
    for (name, material) in [("default", 'C'), ("flesh", 'F')] {
        sounds.surfaces.insert(
            name.into(),
            MapSurface {
                game_material: material,
                bullet_impact: (material == 'F').then(|| "Flesh.BulletImpact".into()),
                ..default()
            },
        );
    }
    sim.app.insert_resource(SoundBank(std::sync::Arc::new(sounds)));
    sim
}

fn collect<M: Message + Clone>(sim: &mut Sim, ticks: usize) -> Vec<M> {
    let mut cursor = sim.app.world().resource::<Messages<M>>().get_cursor_current();
    let mut out = Vec::new();
    for _ in 0..ticks {
        sim.ticks(1);
        out.extend(cursor.read(sim.app.world().resource::<Messages<M>>()).cloned());
    }
    out
}

/// `p` protected for 10 s, as the minigame runtime protects (`God` too).
fn protect(sim: &mut Sim, p: Entity) {
    let now = sim.app.world().resource::<SimClock>().now;
    sim.app.world_mut().entity_mut(p).insert((
        SpawnProtected {
            damage_until: now + 10.0,
            bullets_until: now + 10.0,
        },
        God,
    ));
}

fn health(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<Health>(p).unwrap().current
}

fn aim_at(sim: &mut Sim, p: Entity, target: Vec3) {
    let eye = sim.position(p) + sim.state(p).eye_offset;
    let d = target - eye;
    let mut i = sim.intent(p);
    i.yaw = (-d.x).atan2(-d.z);
    i.pitch = d.y.atan2(d.xz().length());
}

/// One round of the gun in hand from `shooter` at `target`'s chest: the decals and
/// weapon events of the next ticks.
fn fire(sim: &mut Sim, shooter: Entity, target: Entity) -> (Vec<PlaceDecal>, Vec<WeaponEvent>) {
    let chest = sim.position(target) + Vec3::Y * 0.4;
    aim_at(sim, shooter, chest);
    let mut events = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut decals = sim.app.world().resource::<Messages<PlaceDecal>>().get_cursor_current();
    let (mut seen, mut marks) = (Vec::new(), Vec::new());
    sim.intent(shooter).fire = true;
    for t in 0..3 {
        sim.ticks(1);
        if t == 0 {
            sim.intent(shooter).fire = false;
        }
        seen.extend(events.read(sim.app.world().resource::<Messages<WeaponEvent>>()).cloned());
        marks.extend(decals.read(sim.app.world().resource::<Messages<PlaceDecal>>()).cloned());
    }
    let (decals, events) = (marks, seen);
    (decals, events)
}

fn names(events: &[WeaponEvent], who: Entity) -> bool {
    events.iter().any(|e| match e.kind {
        WeaponEventKind::Shot { hit, .. } | WeaponEventKind::ShotContinued { hit, .. } => hit == Some(who),
        WeaponEventKind::Hit { target, .. } => target == who,
        WeaponEventKind::Swing { at, .. } => at.is_some_and(|a| a.2 == who),
        _ => false,
    })
}

#[test]
fn a_round_passes_through_a_protected_player_to_the_wall_and_the_player_behind() {
    let mut sim = sim();
    // 6 m in front of the north wall, shot from 8 m further.
    let target = sim.spawn_character(Vec3::new(0.0, 1.0, greybox::NORTH_WALL_Z + 6.0), placeholder::ID);
    let shooter = sim.spawn_character(Vec3::new(0.0, 1.0, greybox::NORTH_WALL_Z + 14.0), placeholder::ID);
    // Its starting gun, drawn.
    sim.seconds(1.5);
    protect(&mut sim, target);
    let full = health(&sim, target);
    let (decals, events) = fire(&mut sim, shooter, target);
    assert!(!names(&events, target), "nothing traced or dealt names the protected player: {events:?}");
    assert_eq!(health(&sim, target), full);
    assert!(!decals.is_empty(), "the round marks the wall behind: {events:?}");
    for d in &decals {
        assert!((d.point.z - greybox::NORTH_WALL_Z).abs() < 0.3, "on the north wall, not the player: {d:?}");
        assert!(!matches!(&d.group, DecalGroup::Named(n) if n == "Blood"), "no blood: {d:?}");
    }
    let w = sim.app.world();
    assert_eq!(w.get::<ViewPunch>(target).map_or(Vec3::ZERO, |p| p.0), Vec3::ZERO, "no flinch");
    assert!(w.get::<QuakeMovement>(target).is_none_or(|m| m.velocity_modifier == 1.0), "no slowdown");
    // A player between it and the wall takes the round.
    let behind = sim.spawn_character(Vec3::new(0.0, 1.0, greybox::NORTH_WALL_Z + 3.0), placeholder::ID);
    sim.seconds(0.5);
    let before = health(&sim, behind);
    let (_, events) = fire(&mut sim, shooter, target);
    assert!(!names(&events, target), "{events:?}");
    assert!(names(&events, behind), "the one behind is hit: {events:?}");
    assert!(health(&sim, behind) < before, "and takes the damage");
    assert_eq!(health(&sim, target), full);
}

/// The HE `thrower` holds.
fn he(sim: &mut Sim, thrower: Entity) -> Entity {
    give(sim.app.world_mut(), thrower, HEGRENADE).unwrap();
    sim.seconds(1.5);
    let w = sim.app.world();
    w.get::<Inventory>(thrower)
        .unwrap()
        .weapons
        .iter()
        .copied()
        .find(|e| w.get::<Weapon>(*e).is_some_and(|x| x.id == HEGRENADE))
        .unwrap()
}

/// An HE flying at a player's chest from 1.5 m: where it is 0.25 s later
/// (it bounces off an unprotected player, passes a protected one).
fn thrown_at(protected: bool) -> (f32, f32) {
    let mut sim = sim();
    let target = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), placeholder::ID);
    let thrower = sim.spawn_character(Vec3::new(6.0, 1.0, 6.0), placeholder::ID);
    sim.ticks(1);
    let weapon = he(&mut sim, thrower);
    if protected {
        protect(&mut sim, target);
    }
    let chest = sim.position(target) + Vec3::Y * 0.3;
    let g = spawn_projectile(sim.app.world_mut(), weapon, chest + Vec3::Z * 1.5, Vec3::NEG_Z * 12.0).unwrap();
    sim.seconds(0.25);
    let at = sim.app.world().get::<Transform>(g).unwrap().translation;
    let v = sim.app.world().get::<Projectile>(g).unwrap().velocity;
    (at.z - chest.z, v.z)
}

#[test]
fn a_grenade_flies_through_a_protected_player() {
    let (z, v) = thrown_at(false);
    assert!(z > 0.0 && v > 0.0, "an unprotected player bounces it back: {z} {v}");
    let (z, v) = thrown_at(true);
    assert!(z < -1.0 && v < 0.0, "through the protected one: {z} {v}");
}

#[test]
fn a_blast_next_to_a_protected_player_deals_nothing_and_leaves_its_ears() {
    let mut sim = sim();
    let target = sim.spawn_character(Vec3::new(0.0, 0.9, 10.0), placeholder::ID);
    let thrower = sim.spawn_character(Vec3::new(36.0, 1.0, 36.0), placeholder::ID);
    sim.ticks(1);
    let weapon = he(&mut sim, thrower);
    protect(&mut sim, target);
    let full = health(&sim, target);
    spawn_projectile(sim.app.world_mut(), weapon, Vec3::new(0.0, 0.05, 10.0 - 1.5), Vec3::ZERO).unwrap();
    let ticks = (1.8 * sim.tick_hz()) as usize;
    let deaf = collect::<Deafened>(&mut sim, ticks);
    assert_eq!(health(&sim, target), full);
    assert!(deaf.iter().all(|d| d.target != target), "{deaf:?}");
}
