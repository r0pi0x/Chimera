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

/// Report 04df6301 (2026-11-22): a CS:S player went through one of Chill
/// Out's teleporters and was stuck where it arrived: the exit flags are
/// 0.37 m under their pads' tops, and a box put there is in the floor
/// (`teleport::on_floor`). A CS:S, TF2 and Combat Arms player, through
/// every pad, arrives and walks away from the target afterwards (some
/// heading of four takes it 1 m). Halo's map comes from the disc image or
/// the asset store (`CHIMERA_ASSET_STORE`).
#[test]
fn players_walk_away_from_chill_outs_teleporter_targets() {
    use chimera_world::map::teleport::MapTeleporters;
    let Some(mut sim) = party_on("halo_ce:chillout") else { return };
    let t = sim.app.world().resource::<MapTeleporters>().clone();
    let mut stuck = Vec::new();
    for movement in [cs_source::game::movement::ID, combat_arms::game::movement::ID, tf2::game::movement::ID] {
        for (i, pad) in t.pads.iter().enumerate() {
            let mut best = 0.0f32;
            for k in 0..4 {
                let p = sim.spawn_character(pad.source + Vec3::Y * 0.3, movement);
                let mut arrived = false;
                for _ in 0..60 {
                    sim.ticks(1);
                    if (sim.position(p) - pad.dest).xz().length() < 0.6 {
                        arrived = true;
                        break;
                    }
                }
                sim.seconds(0.3);
                let at = sim.position(p);
                {
                    let mut it = sim.intent(p);
                    it.yaw = pad.dest_yaw + k as f32 * std::f32::consts::FRAC_PI_2;
                    it.move_axis = Vec2::Y;
                }
                sim.seconds(1.5);
                let moved = (sim.position(p) - at).xz().length();
                eprintln!("{movement} pad {i} heading {k}: arrived {arrived} at {at} (target {}), walked {moved} m", pad.dest);
                assert!(arrived, "{movement} pad {i}: arrived");
                best = best.max(moved);
                sim.app.world_mut().despawn(p);
                sim.ticks(1);
            }
            if best < 1.0 {
                stuck.push(format!("{movement} pad {i}: best {best} m"));
            }
        }
    }
    assert!(stuck.is_empty(), "{stuck:#?}");
}

/// Where a ray from `origin` along `dir` meets triangle `a b c`: its
/// distance and whether it meets the front.
fn ray_tri(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<(f32, bool)> {
    let (e1, e2) = (b - a, c - a);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 1e-4).then_some((t, e1.cross(e2).dot(dir) < 0.0))
}

/// Report d5674f6d (2026-11-22): holes along dm_underpass' tracks at
/// (-305, -1244, 6) looking down: its gravel is a brush entity in render
/// mode 4 whose texture's alpha is the reflection's mask, and the mode's
/// cut-out made that alpha holes. The gravel under the spot is opaque, and
/// no surface of the map cuts out an alpha that masks something else.
#[test]
fn dm_underpass_gravel_has_no_holes() {
    use chimera_world::map::{EnvmapMask, MapAlpha};
    let Ok(map) = party_roster::load_map("hl2dm:dm_underpass", 0) else {
        eprintln!("skipping: no dm_underpass");
        return;
    };
    let masked: Vec<&str> = map
        .meshes
        .iter()
        .filter(|m| matches!(m.alpha, MapAlpha::Mask(_)))
        .filter(|m| MapAlpha::alpha_is_a_mask(m.envmap.as_ref().map(|e| e.mask), m.selfillum.is_some()))
        .map(|m| m.material.as_str())
        .collect();
    assert!(masked.is_empty(), "cut out at a mask's alpha: {masked:?}");
    let mut first = None;
    for (i, m) in map.meshes.iter().enumerate() {
        for t in m.indices.chunks(3) {
            let p = [0, 1, 2].map(|k| Vec3::from(m.positions[t[k] as usize]));
            if let Some((d, _)) = ray_tri(source(-306.0, -1250.0, 40.0), Vec3::NEG_Y, p[0], p[1], p[2])
                && first.is_none_or(|(b, _)| d < b)
            {
                first = Some((d, i));
            }
        }
    }
    let (_, i) = first.expect("a floor under the spot");
    let m = &map.meshes[i];
    eprintln!("under the spot: {} {:?}", m.material, m.alpha);
    assert!(m.material.contains("gravelfloor002b"), "the gravel: {}", m.material);
    assert_eq!(m.alpha, MapAlpha::Opaque);
    assert_eq!(m.envmap.as_ref().map(|e| e.mask), Some(EnvmapMask::BaseAlphaInverted));
}
