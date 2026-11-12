//! `func_clip_vphysics` on `Hl2Sim` (specs/hl2dm/maps_entities.md 11,
//! case 40; gravity_gun.md Edge cases): a clip with no filter stops what
//! moves by physics (a prop, a frag, a dropped weapon, the energy ball)
//! and lets players, bullets and crossbow bolts through. The map is
//! `gravity_gun`'s (a floor at z = 0, the player at the origin looking
//! east, Source units) with a clip wall across x = 100..104 and a world
//! wall at x = 400. Ticks at 0.015 s, counted from the action.

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::Health;
use chimera_sim::weapon::{Inventory, Weapon, drop::drop_weapon, give, projectile::Flyer};
use chimera_world::map::{MapEntity, MapHull};
use hl2dm::game::weapons::{
    self,
    frag::Hl2Grenade,
    launch::{Hl2Rounds, to_engine, to_source},
};

use super::gravity_gun::{Crate, EAST, W, look, map_data, solid, world_of};

/// The clip wall's near and far faces (Source x).
const CLIP: (f32, f32) = (100.0, 104.0);
/// The world wall's near face.
const WALL: f32 = 400.0;

/// The map with `crates` and the clip; the player at the origin.
fn world(crates: &[Crate]) -> W {
    let floor = (Vec3::new(-4096.0, -4096.0, -64.0), Vec3::new(4096.0, 4096.0, 0.0));
    let mut data = map_data(crates, &[], floor);
    solid(&mut data, Vec3::new(WALL, -1024.0, 0.0), Vec3::new(WALL + 32.0, 1024.0, 512.0));
    let (lo, hi) = (Vec3::new(CLIP.0, -1024.0, 0.0), Vec3::new(CLIP.1, 1024.0, 512.0));
    data.entities.push(MapEntity {
        keyvalues: vec![
            ("classname".into(), "func_clip_vphysics".into()),
            ("origin".into(), "0 0 0".into()),
        ],
        hulls: vec![MapHull {
            planes: Vec::new(),
            points: (0..8)
                .map(|k| Vec3::new([lo.x, hi.x][k & 1], [lo.y, hi.y][(k >> 1) & 1], [lo.z, hi.z][(k >> 2) & 1]))
                .collect(),
        }],
        mover: false,
        physics: None,
    });
    world_of(data)
}

/// `w`'s player holding `id`, drawn.
fn holding(w: &mut W, id: &str) -> Entity {
    let p = w.p;
    let gun = {
        let world = w.sim.app.world();
        world
            .get::<Inventory>(p)
            .unwrap()
            .weapons
            .iter()
            .copied()
            .find(|e| world.get::<Weapon>(*e).is_some_and(|x| x.id == id))
    };
    let gun = gun.unwrap_or_else(|| give(w.sim.app.world_mut(), p, id).unwrap());
    w.sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(gun);
    w.sim.seconds(1.5);
    gun
}

/// The farthest east (Source x) `of` gets over `n` ticks, `input` before
/// each (its tick index); None while it doesn't exist.
fn farthest(w: &mut W, n: u64, mut input: impl FnMut(&mut W, u64), mut of: impl FnMut(&mut World) -> Option<Vec3>) -> f32 {
    let mut far = f32::MIN;
    for t in 0..n {
        input(w, t);
        w.sim.ticks(1);
        if let Some(at) = of(w.sim.app.world_mut()) {
            far = far.max(to_source(at).x);
        }
    }
    far
}

/// Case 40, the physics objects: a punted crate, a thrown frag, a thrown
/// pistol and the AR2's energy ball all fly east at the clip and stay on
/// the near side of it (none of them would reach the wall at 400 later
/// than 1 s: 1100, 1200, the throw's and 1000 units/s).
#[test]
fn a_physics_clip_stops_props_frags_weapons_and_the_ball() {
    // The crate: punted at 1100 units/s from 60 ahead.
    let mut w = world(&[Crate::ahead(60.0)]);
    let crate_node = w.node(0);
    let far = farthest(
        &mut w,
        120,
        |w, t| w.sim.intent(w.p).fire = t == 0,
        |world| world.get::<Position>(crate_node).map(|p| p.0),
    );
    assert!(w.velocity(0).x.abs() < 1100.0, "it was punted and stopped");
    assert!(far + 10.0 < CLIP.0 + 1.0, "the crate's face stays before the clip: centre {far}");

    // The frag: tapped, thrown on tick 15 at 1200 units/s.
    let mut w = world(&[]);
    let frag = holding(&mut w, weapons::FRAG);
    w.sim.app.world_mut().get_mut::<Hl2Rounds>(frag).unwrap().count = 1;
    let far = farthest(
        &mut w,
        100,
        |w, t| w.sim.intent(w.p).fire = t == 0,
        |world| {
            world
                .query_filtered::<&Position, With<Hl2Grenade>>()
                .iter(world)
                .next()
                .map(|p| p.0)
        },
    );
    assert!(far > 20.0 && far < CLIP.0, "the frag flew and stopped at the clip: {far}");

    // A dropped weapon: the pistol, thrown along the view (a 0.2 m box:
    // no models here to size it).
    let mut w = world(&[]);
    w.sim.app.init_resource::<chimera_world::map::loose::LooseAssets>();
    holding(&mut w, weapons::PISTOL);
    let p = w.p;
    let loose = drop_weapon(w.sim.app.world_mut(), p, true).expect("dropped");
    let far = farthest(&mut w, 120, |_, _| {}, |world| world.get::<Position>(loose).map(|p| p.0));
    assert!(far > 20.0 && far < CLIP.0, "the pistol stopped at the clip: {far}");

    // The energy ball: charged 0.5 s, then 1000 units/s east; it bounces
    // back off the clip.
    let mut w = world(&[]);
    let ar2 = holding(&mut w, weapons::AR2);
    w.sim.app.world_mut().get_mut::<Hl2Rounds>(ar2).unwrap().count = 1;
    let mut seen = false;
    let far = farthest(
        &mut w,
        100,
        |w, t| w.sim.intent(w.p).secondary = t == 0,
        |world| {
            let at = world
                .query::<(&Flyer, &Transform)>()
                .iter(world)
                .find(|(f, _)| f.weapon == weapons::AR2)
                .map(|(_, t)| t.translation);
            seen |= at.is_some();
            at
        },
    );
    assert!(seen, "the ball was launched");
    assert!(far > 20.0 && far < CLIP.0, "the ball turned at the clip: {far}");
}

/// Case 40, what passes: the player walks through the clip; a pistol
/// bullet and a crossbow bolt cross it to another player behind it.
#[test]
fn players_bullets_and_bolts_pass_a_physics_clip() {
    let mut w = world(&[]);
    let start = to_source(w.sim.position(w.p));
    w.sim.intent(w.p).move_axis = Vec2::new(0.0, 1.0);
    w.sim.seconds(2.0);
    w.sim.intent(w.p).move_axis = Vec2::ZERO;
    let x = to_source(w.sim.position(w.p)).x;
    assert!(x - start.x > CLIP.1 + 16.0, "walked through the clip to {x}");

    // Shots: a target behind the clip, the shooter before it.
    let mut w = world(&[]);
    let target = w.sim.spawn_character(to_engine(Vec3::new(200.0, 0.0, 36.0)), hl2dm::game::movement::ID);
    w.sim.seconds(1.0);
    look(&mut w.sim, w.p, EAST, 0.0);
    let health = |w: &W| w.sim.app.world().get::<Health>(target).unwrap().current;
    holding(&mut w, weapons::PISTOL);
    let before = health(&w);
    w.sim.intent(w.p).fire = true;
    w.sim.ticks(1);
    w.sim.intent(w.p).fire = false;
    w.sim.ticks(2);
    assert!(health(&w) < before, "the bullet hit the target behind the clip");

    holding(&mut w, weapons::CROSSBOW);
    let before = health(&w);
    let mut hit_at = None;
    for t in 0..60 {
        w.sim.intent(w.p).fire = t == 0;
        w.sim.ticks(1);
        if health(&w) < before && hit_at.is_none() {
            hit_at = Some(t);
        }
    }
    assert!(hit_at.is_some(), "the bolt crossed the clip into the target");
}
