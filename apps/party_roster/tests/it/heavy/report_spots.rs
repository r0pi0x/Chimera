//! Places players reported (F10) on other games' maps, checked again at
//! the report's spot: the reporter's character put down where it stood,
//! facing its way, walks past it; what was drawn wrong there is drawn
//! right. Each test passes with a note without the map's install.

use bevy::prelude::*;
use chimera_core::slots::MovementRegistry;
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::harness::CaSim;
use party_roster::PartyPartsPlugin;

/// The party on map `id` (None, with a note, without its install).
fn party_on(id: &str) -> Option<Sim> {
    let map = match party_roster::load_map(id, 0) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("skipping: {id} did not load: {e}");
            return None;
        }
    };
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
    sim.ticks(2);
    Some(sim)
}

/// Source's (x, y, z) units (a report's `setpos`) as engine metres.
fn source(x: f32, y: f32, z: f32) -> Vec3 {
    cs_source::game::movement::to_engine(Vec3::new(x, y, z))
}

/// The intent yaw facing Source yaw `deg` (degrees).
fn yaw_of(deg: f32) -> f32 {
    let r = deg.to_radians();
    let d = (source(r.cos(), r.sin(), 0.0) - source(0.0, 0.0, 0.0)).normalize();
    (-d.x).atan2(-d.z)
}

/// A character's feet, as its movement says.
fn feet(sim: &Sim, movement: &str, e: Entity) -> Vec3 {
    let w = sim.app.world();
    let footing = w.resource::<MovementRegistry>().get(movement).and_then(|m| m.footing).expect("a footing");
    footing(w, e).expect("standing").feet
}

/// The feet of a character of `movement` put down at `at` (feet) walking
/// toward Source yaw `deg` for `secs`.
fn walk(sim: &mut Sim, movement: &'static str, at: Vec3, deg: f32, secs: f32) -> (Vec3, Vec3) {
    let p = sim.spawn_character(at + Vec3::Y * 0.3, movement);
    sim.seconds(0.5);
    let start = feet(sim, movement, p);
    {
        let mut i = sim.intent(p);
        i.yaw = yaw_of(deg);
        i.move_axis = Vec2::Y;
    }
    sim.seconds(secs as f64);
    let end = feet(sim, movement, p);
    sim.app.world_mut().despawn(p);
    sim.ticks(1);
    eprintln!("{movement} from {at}: settled at {start}, walked to {end}");
    (start, end)
}

/// How far `b` is from `a` along Source yaw `deg`, across (metres).
fn along(a: Vec3, b: Vec3, deg: f32) -> f32 {
    let y = yaw_of(deg);
    (b - a).xz().dot(Vec2::new(-y.sin(), -y.cos()))
}

/// Compares two movements walked from one spot: (the map's own, the
/// reporter's) progress along the heading and their end heights.
fn compare(id: &str, own: &'static str, theirs: &'static str, at: Vec3, deg: f32, secs: f32) -> Option<((f32, f32), (f32, f32))> {
    let mut sim = party_on(id)?;
    let (s0, e0) = walk(&mut sim, own, at, deg, secs);
    let (s1, e1) = walk(&mut sim, theirs, at, deg, secs);
    Some(((along(s0, e0, deg), e0.y - s0.y), (along(s1, e1, deg), e1.y - s1.y)))
}

/// Report 0957b0c2 (2026-11-22): a Halo player could not walk up
/// dm_underpass' stairs at (384, -1025, -328), facing 179 degrees; an
/// HL2:DM player walks up them, and so does the Halo player.
#[test]
fn a_halo_player_walks_up_dm_underpass_stairs() {
    let Some(((own, own_up), (halo, halo_up))) = compare(
        "hl2dm:dm_underpass",
        hl2dm::game::movement::ID,
        halo_ce::game::movement::ID,
        source(384.0, -1025.2, -328.0),
        179.0,
        3.0,
    ) else {
        return;
    };
    eprintln!("HL2:DM {own} m along, {own_up} m up; Halo {halo} m along, {halo_up} m up");
    assert!(own_up > 1.0, "the HL2:DM player climbed the stairs");
    assert!(halo_up > own_up - 0.5, "the Halo player climbed them too: {halo_up} m against {own_up}");
}

/// Report dd2a7814 (2026-11-22): something unseen blocked a TF2 player in
/// coldseed's doorway at (-2696, -2043, 37), facing 96 degrees; it walks
/// through as far as a Combat Arms soldier does.
#[test]
fn a_tf2_player_walks_through_coldseeds_doorway() {
    let Some(((own, _), (tf2, _))) = compare(
        "combat_arms:coldseed",
        combat_arms::game::movement::ID,
        tf2::game::movement::ID,
        source(-2696.45, -2043.32, 36.59),
        96.2,
        2.0,
    ) else {
        return;
    };
    eprintln!("Combat Arms {own} m along; TF2 {tf2} m along");
    assert!(own > 2.0, "the soldier walked through");
    assert!(tf2 > own - 0.75, "the TF2 player did too: {tf2} m against {own}");
}
