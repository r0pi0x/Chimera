//! The sandbox's props and blasts (chimera_sim's `minigame::props` and
//! `targets::BlastMeasured`): a bullet line knocks the bottle it crosses, a
//! blast every piece in its reach, and they stand again after the reset;
//! a board keeps where bullets landed on its face (+x right, +y up as the
//! shooter sees it) and counts a shot at it that missed; props are
//! refused outside the sandbox and gone when it stops; an engine blast
//! among three targets is one `BlastMeasured` whose victims' damage is
//! their `HitMeasured`'s. On the party's simulation, headless.

use bevy::prelude::*;
use chimera_core::core::{Intent, LocalPlayer};
use chimera_sim::{
    minigame::{
        self, props,
        targets::{self, BlastMeasured, HitMeasured, Prop, PropHit, PropSpec, ShotGroup, SpawnProp, SpawnTarget},
    },
    weapon::{WeaponEvent, WeaponEventKind, grenade::BlastAt},
};
use chimera_world::harness::Sim;
use combat_arms::game::movement;
use ultimate_gun_game::sandbox::SANDBOX;

use crate::parts::ugg;

const GUN: &str = "combat_arms:m4a1";

#[derive(Resource, Default)]
struct Seen {
    props: Vec<PropHit>,
    blasts: Vec<BlastMeasured>,
    hits: Vec<HitMeasured>,
}

fn keep(mut seen: ResMut<Seen>, mut p: MessageReader<PropHit>, mut b: MessageReader<BlastMeasured>, mut h: MessageReader<HitMeasured>) {
    seen.props.extend(p.read().cloned());
    seen.blasts.extend(b.read().cloned());
    seen.hits.extend(h.read().cloned());
}

fn play() -> (Sim, Entity) {
    let mut sim = ugg();
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.app.init_resource::<Seen>().add_systems(FixedPostUpdate, keep);
    sim.seconds(1.5);
    (sim, me)
}

fn prop(sim: &mut Sim, kind: &str, distance_m: f32) -> Entity {
    let e = props::spawn(sim.app.world_mut(), &SpawnProp { spec: PropSpec { kind: kind.into(), distance_m }, by: None })
        .unwrap_or_else(|e| panic!("the prop: {e}"));
    sim.ticks(2);
    e
}

/// A bullet's line from `me`'s eye through `at` (and 5 m past it), with
/// its `ShotMade`.
fn bullet(sim: &mut Sim, me: Entity, at: Vec3) {
    let from = sim.position(me) + Vec3::Y * 0.6;
    let dir = (at - from).normalize();
    for kind in [
        WeaponEventKind::ShotMade { id: GUN, origin: from, dir },
        WeaponEventKind::Shot { from, to: at + dir * 5.0, hit: None, normal: None },
    ] {
        sim.app.world_mut().write_message(WeaponEvent { owner: me, weapon: Entity::PLACEHOLDER, kind, replay: false });
    }
    sim.ticks(1);
}

/// Prop `e`'s point `local` (its frame: +x right, +y up, +z toward the
/// spawner) in the world.
fn world_of(sim: &Sim, e: Entity, local: Vec3) -> Vec3 {
    sim.app.world().get::<Transform>(e).unwrap().transform_point(local)
}

fn standing(sim: &Sim, e: Entity) -> u32 {
    sim.app.world().get::<Prop>(e).unwrap().standing
}

#[test]
fn bottles_fall_to_bullets_and_blasts_and_stand_again() {
    let (mut sim, me) = play();
    let e = prop(&mut sim, "bottles", 8.0);
    let p = sim.app.world().get::<Prop>(e).cloned().unwrap();
    assert_eq!((p.pieces, p.standing, p.spec.kind.as_str()), (6, 6, "bottles"));
    assert!(((p.home - sim.position(me)).xz().length() - 8.0).abs() < 0.05, "8 m out: {:?}", p.home);
    // The third bottle (from the left, x = -0.125 m) at its middle.
    let third = world_of(&sim, e, Vec3::new(-0.125, 1.12, 0.0));
    bullet(&mut sim, me, third);
    assert_eq!(standing(&sim, e), 5);
    let hit = sim.app.world().resource::<Seen>().props.last().cloned().expect("a PropHit");
    assert_eq!((hit.prop, hit.piece, hit.attacker), (e, 2, Some(me)));
    assert!((hit.distance_m - 8.0).abs() < 1.0, "{hit:?}");
    // The same line again: that bottle is down, the one behind it none.
    bullet(&mut sim, me, third);
    assert_eq!(standing(&sim, e), 5);
    // A blast among them knocks the rest.
    let centre = world_of(&sim, e, Vec3::new(0.0, 1.1, 0.0));
    sim.app.world_mut().write_message(BlastAt {
        at: centre,
        blast: hl2dm::game::weapons::launch::blast(150.0, 300.0, None, None),
        thrower: Some(me),
        weapon: "hl2dm:weapon_rpg",
        inflictor: None,
    });
    sim.ticks(3);
    assert_eq!(standing(&sim, e), 0);
    sim.seconds(props::RESET_SECS + 0.5);
    assert_eq!(standing(&sim, e), 6, "every piece down: they stand again");
}

#[test]
fn a_board_keeps_the_shot_group_and_its_misses() {
    let (mut sim, me) = play();
    let e = prop(&mut sim, "target_board", 10.0);
    let face = |x: f32, y: f32| Vec3::new(x, props::BOARD_CENTRE_Y + y, 0.0);
    for (x, y) in [(0.1, 0.0), (-0.1, 0.0), (0.0, 0.2)] {
        let at = world_of(&sim, e, face(x, y));
        bullet(&mut sim, me, at);
    }
    // At the board, wide of its face: a miss.
    let wide = world_of(&sim, e, face(0.8, 0.0));
    bullet(&mut sim, me, wide);
    let g = sim.app.world().get::<ShotGroup>(e).cloned().expect("a board's group");
    assert_eq!(g.hits.len(), 3, "{g:?}");
    assert!((g.hits[0] - Vec2::new(0.1, 0.0)).length() < 0.01, "+x is right as the shooter sees it: {g:?}");
    assert!((g.hits[2] - Vec2::new(0.0, 0.2)).length() < 0.01, "+y up: {g:?}");
    assert_eq!(g.misses, 1);
    assert!((g.spread_m - 0.1f32.hypot(0.2)).abs() < 0.01, "the widest two: {g:?}");
    assert!((g.centre - Vec2::new(0.0, 0.2 / 3.0)).length() < 0.01, "{g:?}");
    // `prop group` says it; `prop reset` clears it.
    sim.app.world_mut().resource_mut::<chimera_core::console::Console>().submit("prop reset 1");
    sim.ticks(2);
    assert_eq!(sim.app.world().get::<ShotGroup>(e).cloned().unwrap(), ShotGroup::default());
}

#[test]
fn props_are_the_sandboxs_only() {
    let mut sim = ugg();
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(0.5);
    let req = SpawnProp { spec: PropSpec { kind: "tin_cans".into(), distance_m: 8.0 }, by: None };
    assert!(props::spawn(sim.app.world_mut(), &req).unwrap_err().contains("sandbox"));
    let (mut sim, _) = play();
    let e = prop(&mut sim, "tin_cans", 8.0);
    assert_eq!(sim.app.world().get::<Prop>(e).unwrap().pieces, 5);
    let far = props::spawn(sim.app.world_mut(), &SpawnProp { spec: PropSpec { kind: "tin_cans".into(), distance_m: 5000.0 }, by: None });
    assert!(far.unwrap_err().contains("no floor"));
    // The sandbox's rule gone (another minigame plays): the props go.
    sim.app.world_mut().remove_resource::<minigame::rung_command::HoldRungs>();
    sim.seconds(0.5);
    assert!(sim.app.world().get::<Prop>(e).is_none(), "gone when the sandbox stops");
}

#[test]
fn a_blast_among_three_targets_is_measured_as_their_hits() {
    let (mut sim, me) = play();
    let mut ts = Vec::new();
    for (yaw, d) in [(0.0f32, 6.0), (0.15, 7.5), (-0.2, 9.0)] {
        sim.app.world_mut().get_mut::<Intent>(me).unwrap().yaw = yaw;
        let req = SpawnTarget { gun: Some(GUN.into()), distance_m: d, ..default() };
        ts.push(targets::spawn(sim.app.world_mut(), &req).unwrap_or_else(|e| panic!("{e}")));
    }
    sim.seconds(1.5);
    let at = sim.position(ts[0]) + Vec3::new(0.0, -0.5, 1.0);
    sim.app.world_mut().write_message(BlastAt {
        at,
        blast: hl2dm::game::weapons::launch::blast(150.0, 300.0, None, None),
        thrower: Some(me),
        weapon: "hl2dm:weapon_rpg",
        inflictor: None,
    });
    sim.ticks(3);
    let seen = sim.app.world().resource::<Seen>();
    assert_eq!(seen.blasts.len(), 1, "one per blast: {:?}", seen.blasts);
    let b = &seen.blasts[0];
    assert_eq!((b.attacker, b.gun, b.inner), (Some(me), Some("hl2dm:weapon_rpg"), 0.0));
    assert!(b.outer > 5.0 && b.max_damage > 1.0, "{b:?}");
    assert_eq!(b.edge_damage, 0.0, "HL2's falloff is linear to nothing at the radius");
    let mut on_targets = 0;
    for t in &ts {
        let Some(h) = b.hits.iter().find(|h| h.victim == *t) else { continue };
        on_targets += 1;
        let hm = seen.hits.iter().find(|m| m.victim == *t && m.attacker == Some(me)).expect("its HitMeasured");
        assert!((h.damage - hm.damage).abs() < 1e-4, "{h:?} vs {hm:?}");
        assert_eq!(h.killed, hm.health_after <= 0.0, "{h:?} {hm:?}");
    }
    assert_eq!(on_targets, 3, "{b:?}");
    let mut d: Vec<f32> = b.hits.iter().filter(|h| ts.contains(&h.victim)).map(|h| h.distance_m).collect();
    d.sort_by(f32::total_cmp);
    assert!(d[0] < d[1] && d[1] < d[2], "three distances: {d:?}");
}
