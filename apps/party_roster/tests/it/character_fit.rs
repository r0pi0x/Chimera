//! Characters fit the map's game (`chimera_core::envelope`; Source's
//! `QuakeConfig::fitted`, Combat Arms' `movement::body`): on the
//! party's simulation with every game's movement, a TF2-sized player (48 u
//! wide, 82 tall) on a Combat Arms map walks through greyboxes of the
//! narrowest doorways measured on Combat Arms' maps (specs/combat_arms/
//! movement.md "Door sizes": training's 136 x 56 u, graveyard's 104 x 60 u,
//! in Combat Arms units at its data's scale) standing, its box no wider
//! and no taller than Combat Arms' hull and its eye 0.1 m under the top;
//! without Combat Arms' envelope the same doorway stops it. A Combat Arms
//! player's body and eye are the same on a TF2 map as on none (it is the
//! smaller one every way); on a CS:S map it keeps its width and its eyes
//! and takes CS:S's lower box heights (62 u standing, 45 ducked: CS:S's
//! own movement box, its eye 64 u over it). TF2 and HL2:DM players on a
//! CS:S map take its 62 u box and see from CS:S's own 64 u eye, not lower.

use bevy::prelude::*;
use chimera_core::{
    core::{MapGame, MovementState},
    envelope::{EYE_MARGIN, MovementEnvelopes, map_envelope},
};
use chimera_world::{
    harness::Sim,
    map::{MapBrush, MapBrushes},
};
use combat_arms::{game::movement::MovementData, harness::CaSim};
use party_roster::PartyPartsPlugin;

/// Combat Arms' units (metres) at its data's scale.
fn ca_unit(sim: &Sim) -> f32 {
    sim.app.world().resource::<MovementData>().meters_per_unit
}

/// A floor at y = 0 and, at x = 2 m, a wall 0.3 m thick with one doorway
/// `width` x `height` (Combat Arms units) centred on z = 0.
fn doorway_level(u: f32, width: f32, height: f32) -> MapBrushes {
    let (w, h) = (width * u / 2.0, height * u);
    let b = |lo: Vec3, hi: Vec3| MapBrush { surface: Some("concrete".into()), ..MapBrush::from_box(lo, hi) };
    MapBrushes(vec![
        b(Vec3::new(-50.0, -1.0, -50.0), Vec3::new(50.0, 0.0, 50.0)),
        b(Vec3::new(2.0, 0.0, -50.0), Vec3::new(2.3, 10.0, -w)),
        b(Vec3::new(2.0, 0.0, w), Vec3::new(2.3, 10.0, 50.0)),
        b(Vec3::new(2.0, h, -w), Vec3::new(2.3, 10.0, w)),
    ])
}

/// The party's simulation on a map of `game` (None: no map game).
fn on_map(game: Option<&str>) -> Sim {
    let mut sim = Sim::new(CaSim, PartyPartsPlugin);
    if let Some(g) = game {
        sim.app.world_mut().insert_resource(MapGame(g.into()));
    }
    sim.ticks(2);
    sim
}

/// A player of `movement` standing at the origin, settled.
fn player(sim: &mut Sim, movement: &'static str) -> Entity {
    let p = sim.spawn_character(Vec3::new(0.0, 1.2, 0.0), movement);
    sim.seconds(1.0);
    assert!(sim.state(p).on_ground, "{movement} on the floor");
    p
}

/// The feet's height (the box's bottom).
fn feet(sim: &Sim, p: Entity) -> f32 {
    sim.position(p).y + sim.state(p).hull_min.y
}

/// A TF2 player runs at +x through the doorway for 2 s: where it got to.
fn tf2_through(sim: &mut Sim, width: f32, height: f32) -> (f32, MovementState, f32) {
    let u = ca_unit(sim);
    sim.app.world_mut().insert_resource(doorway_level(u, width, height));
    let p = player(sim, tf2::game::movement::ID);
    let mut lowest_gap = f32::MAX;
    {
        let mut i = sim.intent(p);
        i.yaw = -std::f32::consts::FRAC_PI_2;
        i.move_axis = Vec2::Y;
    }
    for _ in 0..(2.0 * sim.tick_hz()) as u32 {
        sim.ticks(1);
        let s = sim.state(p);
        lowest_gap = lowest_gap.min((s.hull_max.y - s.eye_offset.y).abs());
        assert!(!s.crouching, "it walks through standing");
    }
    (sim.position(p).x, sim.state(p), lowest_gap)
}

#[test]
fn a_tf2_player_walks_through_combat_arms_narrowest_doorways() {
    for (width, height) in [(56.0, 136.0), (60.0, 104.0)] {
        let mut sim = on_map(Some(combat_arms::game::GAME));
        let ca = combat_arms::game::movement::envelope(sim.app.world().resource::<MovementData>());
        assert_eq!(map_envelope(sim.app.world()), Some(ca));
        let (x, s, gap) = tf2_through(&mut sim, width, height);
        assert!(x > 3.0, "through the {width} x {height} u doorway: x {x}");
        let half = s.hull_max.x.max(s.hull_max.z);
        assert!(half <= ca.radius + 1e-4, "no wider than Combat Arms' hull: {half}");
        let tall = s.hull_max.y - s.hull_min.y;
        assert!(tall <= ca.height + 1e-4, "no taller than Combat Arms' hull: {tall}");
        assert!(gap >= EYE_MARGIN - 1e-4, "the eye stays {EYE_MARGIN} m under the box's top: {gap}");
    }
}

#[test]
fn without_combat_arms_envelope_the_doorway_stops_a_tf2_player() {
    let mut sim = on_map(Some(combat_arms::game::GAME));
    sim.app.world_mut().resource_mut::<MovementEnvelopes>().0.retain(|(g, _)| *g != combat_arms::game::GAME);
    let (x, s, _) = tf2_through(&mut sim, 56.0, 136.0);
    assert!(x < 2.0, "the 1.22 m box stops at the doorway: x {x}");
    assert!((s.hull_max.x - 24.0 * 0.0254).abs() < 1e-3, "TF2's own box: {:?}", s.hull_max);
}

/// A Combat Arms player's box and eye, standing and crouched.
fn ca_body(game: Option<&str>) -> ([MovementState; 2], f32) {
    let mut sim = on_map(game);
    // Combat Arms' mover collides with physics: a static floor (top y = 0).
    sim.app.world_mut().spawn((
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(100.0, 2.0, 100.0),
        Transform::from_xyz(0.0, -1.0, 0.0),
    ));
    let p = player(&mut sim, combat_arms::game::movement::ID);
    let stand = sim.state(p);
    sim.intent(p).crouch = true;
    sim.seconds(1.0);
    let crouch = sim.state(p);
    ([stand, crouch], feet(&sim, p))
}

#[test]
fn a_combat_arms_player_keeps_its_body_on_a_tf2_map_and_its_width_on_cs_sources() {
    let ([stand, crouch], _) = ca_body(None);
    let ([on_tf2, crouch_tf2], _) = ca_body(Some(tf2::game::GAME));
    let ([on_cs, crouch_cs], _) = ca_body(Some(cs_source::game::GAME));
    let body = |s: &MovementState| (s.hull_min, s.hull_max, s.eye_offset);
    // TF2's box is bigger every way: Combat Arms' own body, to the bit.
    assert_eq!(body(&on_tf2), body(&stand), "standing on a TF2 map");
    assert_eq!(body(&crouch_tf2), body(&crouch), "crouched on a TF2 map");
    // CS:S's box is wider but lower (62 u standing, 45 ducked: 1.57 m and
    // 1.14 m, specs/cs_source/movement.md "Hulls, eyes and ducking", the
    // box CS:S moves with): the width stays Combat Arms', the heights are
    // CS:S's, and the eyes stay Combat Arms' own (1.54 m and 1.11 m, under
    // CS:S's own 64 u and 47 u eyes, so the rule leaves them).
    let cs = |u: f32| u * 0.0254;
    let eye = |s: &MovementState| s.eye_offset.y - s.hull_min.y;
    for (s, own, tall, what) in [(&on_cs, &stand, cs(62.0), "standing"), (&crouch_cs, &crouch, cs(45.0), "crouched")] {
        assert_eq!(s.hull_max.x, own.hull_max.x, "{what}: as narrow as its own");
        let h = s.hull_max.y - s.hull_min.y;
        assert!((h - tall).abs() < 1e-3, "{what}: CS:S's height {tall}, got {h}");
        assert!((eye(s) - eye(own)).abs() < 1e-4, "{what}: its own eye {}, got {}", eye(own), eye(s));
    }
    assert!(eye(&stand) < cs(64.0) && eye(&crouch) < cs(47.0));
}

/// A Source player's eye above its feet, and its box's height, on a map of
/// `game`.
fn source_body(movement: &'static str, game: Option<&str>) -> (f32, f32) {
    let mut sim = on_map(game);
    let level = doorway_level(ca_unit(&sim), 1000.0, 1000.0);
    sim.app.world_mut().insert_resource(level);
    let p = player(&mut sim, movement);
    let s = sim.state(p);
    (s.eye_offset.y - s.hull_min.y, s.hull_max.y - s.hull_min.y)
}

/// TF2's and HL2:DM's players on a CS:S map: CS:S's 62 u box, and the eye
/// at CS:S's own 64 u (above that box, as CS:S's is), not 0.1 m under it.
#[test]
fn bigger_source_players_on_a_cs_source_map_see_from_its_own_eye() {
    let u = 0.0254;
    for movement in [tf2::game::movement::ID, hl2dm::game::movement::ID] {
        let (own_eye, _) = source_body(movement, None);
        let (eye, tall) = source_body(movement, Some(cs_source::game::GAME));
        assert!((tall - 62.0 * u).abs() < 1e-3, "{movement}: CS:S's box, got {tall}");
        let want = own_eye.min(64.0 * u);
        assert!((eye - want).abs() < 1e-3, "{movement}: the eye {want} (CS:S's own 64 u or its own), got {eye}");
        assert!(eye > tall, "{movement}: the eye over the box, as CS:S's");
    }
}
