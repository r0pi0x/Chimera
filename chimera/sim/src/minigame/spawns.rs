//! `points = "random_hidden"`: a deathmatch's spawning, spread over the
//! whole map and out of the enemies' sight (`pick`, called by
//! `runtime::spawn_point`).
//!
//! - Spots (`SpawnSpots`, made once per loaded map on the first spawn and
//!   removed with the map, `MapScopedApp::map_scoped`): points on the
//!   floors of the map's navigation mesh (`map::nav::NavMesh`, the map's
//!   own file or volumes; else one generated from its collision,
//!   `map::nav_gen`, with a walking player's step: a mesh someone else
//!   generated, `NavMesh::generated`, is not used, since bots' own climbs
//!   0.8 m ledges a player can't, and once left de_aztec over its walls),
//!   a grid
//!   about `SPOT_GRID` m apart in the areas that can be walked to from the
//!   spawn points and back (the mesh's main connected region), each spot
//!   moved a seeded random amount (up to `SPOT_NUDGE` per axis) off the
//!   area's centred grid, in a seeded random order. The nudge keeps eyes
//!   off the map's grid lines: areas are often symmetric about one, and
//!   centred spots put a whole row of eyes in the plane where two faces
//!   meet, which then shows as a vertical line through the frame (de_dust2's
//!   "seam" between a bump-lit and a flat-lit displacement, 2026-10-30). Each spot is checked (`spot_ok`): a floor under
//!   it, a character's capsule fits there without touching the world, it
//!   is inside the map (on a map with visibility data, its chest in a
//!   leaf of a cluster: not solid, not the void outside the world), and
//!   outside water and every zone a game registers as hurting or
//!   killing (`SpawnHazards`). Without a mesh, or spots, the map's spawn
//!   points are the spots. `candidates` lists every spot without the
//!   `MAX_SPOTS` cap (the sweep of the stock maps).
//! - Picking: from a random place in the spots, the first that is at
//!   least `min_distance` m from every living enemy (a character of
//!   another team) and out of their sight (rays from each enemy's eye to
//!   the spot's head, chest and feet blocked); else the first that is
//!   far enough; else the one farthest from its nearest enemy. Never
//!   fails once the map has a spawn point.
//! - Facing (`face`, the minigame's `[spawn] facing`, `def::Facing`): at
//!   the picked spot, 32 horizontal rays from eye height against the
//!   static world (characters excluded), each capped at 30 m; the
//!   distances are smoothed over 5 neighbours, and the most open direction
//!   is faced (among those within 10% of it, the one toward the middle of
//!   the spots), turned a random +-10 degrees by the seeded draws, pitched
//!   2 degrees down. About 20 microseconds a spawn
//!   (`facing_costs_well_under_a_millisecond` measures it). `rays = 0`
//!   brings back eight fixed directions.
//! - Determinism: the shuffle and the picks draw on a seeded generator
//!   (`SpawnSeed`, else a hash of the minigame's id), started over with
//!   the match (`reset`); candidates and enemies are walked in a fixed
//!   order. Nothing global.
//!
//! Known shortcuts: sight is three rays per enemy from the eye without a
//! field of view or fog (an enemy facing away still "sees"); the spots are
//! chosen once, so a door that opens later isn't considered; a map whose
//! collision isn't built yet at the first spawn gets the spawn points for
//! up to `BUILD_TRIES` spawns before the result is kept; on a map without
//! its own mesh the spots are where a player walks from the spawn points
//! (no jumps, ladders or crouching), so a ledge reached by a jump is never
//! one; "inside the map" is the visibility tree's clusters, so a map
//! without visibility data (Combat Arms') relies on the walk alone.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};

use crate::{
    core::{
        CHARACTER_FILTER, Connecting, Dead, Health, Intent, MapBrush, MapWater, SOLID_LAYERS, SpawnPoint, Spectating,
        Team,
    },
    map::{
        SPAWN_LIFT,
        nav::{NavMesh, flags},
        nav_gen::{self, NavGenParams},
        vis::{ActiveVisibility, MapVisibility},
    },
    weapon::random::{Rng, crc32},
};

use super::def::Facing;

/// The grid of spots in a navigation area (m).
pub const SPOT_GRID: f32 = 1.5;
/// The most a spot moves off its area's centred grid, either way on each
/// axis (m).
pub const SPOT_NUDGE: f32 = SPOT_GRID / 4.0;
/// Mixed into the seed for the nudges' draws.
const SPOT_NUDGE_SEED: u32 = 0x5EA4_0FF5;
/// Most spots kept for a map.
pub const MAX_SPOTS: usize = 1500;
/// Spawns that may find the collision not ready before the spawn points
/// are kept as the spots.
pub const BUILD_TRIES: u8 = 3;
/// A character's capsule: radius and standing height (m).
const RADIUS: f32 = 0.4;
const HEIGHT: f32 = 1.8;
/// An enemy's eye above its centre (m).
const EYE: f32 = 0.7;
/// How far above the floor the capsule is tested (clear of the floor's
/// own surface).
const LIFT: f32 = 0.05;

/// Seeds the spawn draws of a match (else a hash of the minigame's id).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnSeed(pub u32);

pub use crate::map::{SpawnHazardAppExt, SpawnHazards};

/// The loaded map's spawn spots (removed when the map unloads).
#[derive(Resource, Clone, Debug, Default)]
pub struct SpawnSpots {
    /// Feet positions, in the order they are tried.
    pub spots: Vec<Vec3>,
    /// Made from the navigation mesh (else the map's spawn points).
    pub from_nav: bool,
    /// Kept: not to be made again.
    pub ready: bool,
    /// Spawns that found nothing to keep yet.
    pub tries: u8,
}

/// The generator of a match's draws.
#[derive(Resource, Clone, Debug)]
pub struct SpawnDraws(Rng);

/// The match starts over: the draws begin again.
pub fn reset(world: &mut World) {
    world.remove_resource::<SpawnDraws>();
}

fn seed_of(world: &World) -> u32 {
    world.get_resource::<SpawnSeed>().map(|s| s.0).unwrap_or_else(|| {
        let id = world.get_resource::<super::ActiveMinigame>().map_or("", |a| a.def.id.as_str());
        crc32(id.as_bytes())
    })
}

/// `e`'s spawn: a spot as the module docs say; None when the map has no
/// spawn point at all.
pub fn pick(world: &mut World, e: Entity, min_distance: f32, facing: Facing) -> Option<Transform> {
    let mut points: Vec<Vec3> = world
        .query_filtered::<(Entity, &Transform), (With<SpawnPoint>, Without<Intent>)>()
        .iter(world)
        .map(|(_, t)| t.translation - Vec3::Y * SPAWN_LIFT)
        .collect();
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z)));
    if points.is_empty() {
        return None;
    }
    let team = world.get::<Team>(e).copied();
    let mut enemies: Vec<(Entity, Vec3)> = world
        .query_filtered::<(Entity, &Transform, &Health, Option<&Team>), (
            With<Intent>,
            Without<Dead>,
            Without<Connecting>,
            Without<Spectating>,
        )>()
        .iter(world)
        .filter(|(o, _, h, t)| *o != e && h.current > 0.0 && (team.is_none() || t.copied() != team))
        .map(|(o, t, ..)| (o, t.translation))
        .collect();
    enemies.sort_by_key(|(o, _)| *o);
    let eyes: Vec<Vec3> = enemies.iter().map(|(_, c)| *c + Vec3::Y * EYE).collect();
    let centres: Vec<Vec3> = enemies.iter().map(|(_, c)| *c).collect();

    ensure_spots(world, &points);
    let seed = seed_of(world);
    let mut draws = world.remove_resource::<SpawnDraws>().unwrap_or_else(|| SpawnDraws(Rng::new(seed ^ 0x5eed)));
    let characters: Vec<Entity> = world.iter_entities().filter(|r| r.contains::<Intent>()).map(|r| r.id()).collect();
    let mut state = SystemState::<SpatialQuery>::new(world);
    let Ok(q) = state.get(world) else {
        world.insert_resource(draws);
        return None;
    };
    let spots = world.resource::<SpawnSpots>();
    let n = spots.spots.len();
    let start = ((draws.0.next() * n as f32) as usize).min(n.saturating_sub(1));
    let sight = SpatialQueryFilter::from_mask(SOLID_LAYERS);
    let sight = sight.with_excluded_entities(characters);

    let seen = |spot: Vec3| {
        let targets = [spot + Vec3::Y * 1.7, spot + Vec3::Y * 1.0, spot + Vec3::Y * 0.1];
        eyes.iter().any(|eye| {
            targets.iter().any(|t| {
                let d = *t - *eye;
                let len = d.length();
                match Dir3::new(d) {
                    Ok(dir) => q.cast_ray(*eye, dir, (len - 0.05).max(0.0), true, &sight).is_none(),
                    Err(_) => true,
                }
            })
        })
    };
    let nearest = |spot: Vec3| centres.iter().map(|c| c.distance(spot + Vec3::Y * 0.9)).fold(f32::INFINITY, f32::min);

    let mut far_enough: Option<Vec3> = None;
    let mut farthest: Option<(f32, Vec3)> = None;
    let mut chosen: Option<Vec3> = None;
    for i in 0..n {
        let spot = spots.spots[(start + i) % n];
        let d = nearest(spot);
        if d >= min_distance {
            if !seen(spot) {
                chosen = Some(spot);
                break;
            }
            far_enough.get_or_insert(spot);
        } else if farthest.is_none_or(|(b, _)| d > b) {
            farthest = Some((d, spot));
        }
    }
    let spot = chosen.or(far_enough).or(farthest.map(|(_, s)| s));
    let centroid = (!spots.spots.is_empty()).then(|| spots.spots.iter().sum::<Vec3>() / spots.spots.len() as f32);
    let at = spot.map(|s| {
        let (yaw, pitch) = face(&q, &sight, s, &facing, centroid, &mut draws.0);
        Transform::from_translation(s + Vec3::Y * SPAWN_LIFT)
            .with_rotation(Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0))
    });
    world.insert_resource(draws);
    at
}

/// The yaw and pitch (radians) a spawn at `feet` would face, for a seed
/// (the draws start there): what `pick` does at its spot, for tests and
/// tools. None without a physics world.
pub fn facing_at(world: &mut World, feet: Vec3, facing: &Facing, seed: u32) -> Option<(f32, f32)> {
    let characters: Vec<Entity> = world.iter_entities().filter(|r| r.contains::<Intent>()).map(|r| r.id()).collect();
    let centroid = world
        .get_resource::<SpawnSpots>()
        .filter(|s| !s.spots.is_empty())
        .map(|s| s.spots.iter().sum::<Vec3>() / s.spots.len() as f32);
    let mut state = SystemState::<SpatialQuery>::new(world);
    let q = state.get(world).ok()?;
    let filter = SpatialQueryFilter::from_mask(SOLID_LAYERS).with_excluded_entities(characters);
    Some(face(&q, &filter, feet, facing, centroid, &mut Rng::new(seed)))
}

/// The yaw and pitch (radians) a spawn at `feet` faces: `Facing`'s rule.
/// With `rays == 0`, the longest of eight fixed directions, level.
fn face(
    q: &SpatialQuery,
    filter: &SpatialQueryFilter,
    feet: Vec3,
    f: &Facing,
    centroid: Option<Vec3>,
    rng: &mut Rng,
) -> (f32, f32) {
    let eye = feet + Vec3::Y * 1.6;
    let n = if f.rays == 0 { 8 } else { f.rays as usize };
    let range = if f.rays == 0 { 30.0 } else { f.range.max(1.0) };
    let yaw_of = |k: usize| k as f32 * std::f32::consts::TAU / n as f32;
    let free: Vec<f32> = (0..n)
        .map(|k| {
            let dir = Quat::from_rotation_y(yaw_of(k)) * Vec3::NEG_Z;
            Dir3::new(dir)
                .ok()
                .and_then(|d| q.cast_ray(eye, d, range, true, filter))
                .map_or(range, |h| h.distance)
        })
        .collect();
    if f.rays == 0 {
        let mut best = (-1.0f32, 0usize);
        for (k, d) in free.iter().enumerate() {
            if *d > best.0 + 1e-3 {
                best = (*d, k);
            }
        }
        return (yaw_of(best.1), 0.0);
    }
    // Each direction's distance averaged with its neighbours' (a ring).
    let half = ((f.smooth.max(1) as usize - 1) / 2).min(n / 2);
    let smooth: Vec<f32> = (0..n)
        .map(|k| {
            let sum: f32 = (0..=2 * half).map(|t| free[(k + n + t - half) % n]).sum();
            sum / (2 * half + 1) as f32
        })
        .collect();
    let top = smooth.iter().copied().fold(0.0f32, f32::max);
    let mut pick = (0..n).max_by(|a, b| smooth[*a].total_cmp(&smooth[*b]).then(b.cmp(a))).unwrap_or(0);
    if f.tie > 0.0
        && let Some(c) = centroid
        && let Some(to) = Dir3::new(Vec3::new(c.x - feet.x, 0.0, c.z - feet.z)).ok()
    {
        let toward = |k: usize| (Quat::from_rotation_y(yaw_of(k)) * Vec3::NEG_Z).dot(*to);
        pick = (0..n)
            .filter(|k| smooth[*k] >= top * (1.0 - f.tie))
            .max_by(|a, b| toward(*a).total_cmp(&toward(*b)).then(b.cmp(a)))
            .unwrap_or(pick);
    }
    let turn = rng.range(-f.jitter, f.jitter).to_radians();
    (yaw_of(pick) + turn, f.pitch.to_radians())
}

/// The map's spots, made now if they aren't kept yet. `points`: the map's
/// spawn points' feet.
fn ensure_spots(world: &mut World, points: &[Vec3]) {
    if world.get_resource::<SpawnSpots>().is_some_and(|s| s.ready) {
        return;
    }
    let tries = world.get_resource::<SpawnSpots>().map_or(0, |s| s.tries);
    let mut spots = build(world, points, MAX_SPOTS);
    spots.tries = tries + 1;
    if spots.spots.is_empty() {
        spots.spots = points.to_vec();
        spots.from_nav = false;
        // The collision may not be built yet: try again a few times.
        spots.ready = spots.tries >= BUILD_TRIES;
    } else {
        spots.ready = true;
    }
    world.insert_resource(spots);
}

/// Every spot of the loaded map, in the order tried (no `MAX_SPOTS`
/// cap; empty without a mesh): what `pick` may choose from, for checks.
pub fn candidates(world: &mut World) -> Vec<Vec3> {
    let mut points: Vec<Vec3> = world
        .query_filtered::<&Transform, (With<SpawnPoint>, Without<Intent>)>()
        .iter(world)
        .map(|t| t.translation - Vec3::Y * SPAWN_LIFT)
        .collect();
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z)));
    build(world, &points, usize::MAX).spots
}

/// The walker that makes the spots' mesh when the map has none of its
/// own: a player walking (the default step), a little wider than one
/// (`nav_gen::walking_player`).
pub fn walker() -> NavGenParams {
    nav_gen::walking_player()
}

/// The spots of the loaded map, at most `cap` (`ready` unset; empty
/// without a mesh).
fn build(world: &mut World, points: &[Vec3], cap: usize) -> SpawnSpots {
    let seed = seed_of(world);
    let mut hazards: Vec<MapBrush> = Vec::new();
    let sources: Vec<_> = world.get_resource::<SpawnHazards>().map(|h| h.0.clone()).unwrap_or_default();
    for source in sources {
        hazards.extend(source(world));
    }
    if let Some(water) = world.get_resource::<MapWater>() {
        hazards.extend(water.0.iter().map(|v| v.brush.clone()));
    }
    let vis = world.get_resource::<ActiveVisibility>().map(|v| v.0.clone());
    let characters: Vec<Entity> = world.iter_entities().filter(|r| r.contains::<Intent>()).map(|r| r.id()).collect();
    let mut state = SystemState::<SpatialQuery>::new(world);
    let Ok(q) = state.get(world) else { return SpawnSpots::default() };
    let ignore = |e: Entity| characters.contains(&e);
    let generated;
    // The map's own mesh; one generated for another walker (bots') may
    // climb where a player can't: ours instead.
    let nav: &NavMesh = match world.get_resource::<NavMesh>().filter(|n| !n.generated) {
        Some(n) => n,
        None => {
            generated = nav_gen::generate(&q, &ignore, points, &walker());
            &generated
        }
    };
    let mut rng = Rng::new(seed);
    // Its own draws, so the shuffle's stay as they were.
    let mut nudge = Rng::new(seed ^ SPOT_NUDGE_SEED);
    let mut raw: Vec<(f32, f32, f32)> = Vec::new();
    for (i, ok) in region(nav, points).into_iter().enumerate() {
        let a = &nav.areas[i];
        if !ok || a.has(flags::CROUCH) {
            continue;
        }
        let (size, m) = (a.max - a.min, 0.5);
        // Spots a grid apart, centred in the area (one at its centre if
        // it is narrow).
        let axis = |len: f32| {
            let n = if len <= 2.0 * m { 1 } else { ((len - 2.0 * m) / SPOT_GRID).floor() as u32 + 1 };
            (n, (len - (n - 1) as f32 * SPOT_GRID) / 2.0)
        };
        let ((nx, ox), (nz, oz)) = (axis(size.x), axis(size.y));
        // Each spot moved off the centred grid by up to `SPOT_NUDGE`,
        // staying `m` inside the area (a quarter of a narrow one's width):
        // areas are often symmetric about a line of the map's own grid,
        // where its faces meet, and an eye in that plane sees the edge as
        // a vertical line through the frame (de_dust2's "seam").
        // Drawn within the part of the nudge's reach that is inside (not
        // clamped, which would stack spots on the inset's line).
        let nudged = |lo: f32, hi: f32, at: f32, r: f32| {
            let inset = m.min((hi - lo) / 4.0);
            let (a, b) = ((at - SPOT_NUDGE).max(lo + inset), (at + SPOT_NUDGE).min(hi - inset));
            if a < b { a + r * (b - a) } else { at }
        };
        for ix in 0..nx {
            for iz in 0..nz {
                let (x, z) = (a.min.x + ox + ix as f32 * SPOT_GRID, a.min.y + oz + iz as f32 * SPOT_GRID);
                let x = nudged(a.min.x, a.max.x, x, nudge.next());
                let z = nudged(a.min.y, a.max.y, z, nudge.next());
                raw.push((x, z, a.height_at(x, z)));
            }
        }
    }
    // A seeded shuffle (Fisher-Yates).
    for i in (1..raw.len()).rev() {
        let j = ((rng.next() * (i + 1) as f32) as usize).min(i);
        raw.swap(i, j);
    }
    let floor = SpatialQueryFilter::from_mask(CHARACTER_FILTER).with_excluded_entities(characters.iter().copied());
    let capsule = Collider::capsule(RADIUS, HEIGHT - 2.0 * RADIUS - LIFT);
    let mut spots = Vec::new();
    for (x, z, y) in raw {
        if spots.len() >= cap {
            break;
        }
        if let Some(feet) = spot_ok(&q, &floor, &capsule, &hazards, vis.as_deref(), Vec3::new(x, y, z)) {
            spots.push(feet);
        }
    }
    SpawnSpots {
        from_nav: !spots.is_empty(),
        spots,
        ready: false,
        tries: 0,
    }
}

/// The feet of a valid standing spot near the mesh point `p`, if there is
/// one: a walkable floor within 0.5 m of the mesh's height, the capsule
/// clear of the world, inside the map (`vis`: the chest in a cluster's
/// leaf), and no hazard at the feet, chest or head.
fn spot_ok(
    q: &SpatialQuery,
    filter: &SpatialQueryFilter,
    capsule: &Collider,
    hazards: &[MapBrush],
    vis: Option<&MapVisibility>,
    p: Vec3,
) -> Option<Vec3> {
    let from = p + Vec3::Y * 0.6;
    let hit = q.cast_ray(from, Dir3::NEG_Y, 1.2, true, filter)?;
    if hit.normal.y < 0.7 || hit.distance < 0.05 {
        return None;
    }
    let feet = Vec3::new(p.x, from.y - hit.distance, p.z);
    if !inside_map(vis, feet) {
        return None;
    }
    let centre = feet + Vec3::Y * (LIFT + (HEIGHT - LIFT) / 2.0);
    let mut blocked = false;
    q.shape_intersections_callback(capsule, centre, Quat::IDENTITY, filter, |_| {
        blocked = true;
        false
    });
    if blocked {
        return None;
    }
    let bad = [0.1, 1.0, 1.7].iter().any(|h| {
        let at = feet + Vec3::Y * *h;
        hazards.iter().any(|b| b.contains(at))
    });
    (!bad).then_some(feet)
}

/// Whether a character standing at `feet` is inside the map: with
/// visibility data, its chest is in a leaf of some cluster (not in solid
/// or the void outside the world); without, anywhere.
pub fn inside_map(vis: Option<&MapVisibility>, feet: Vec3) -> bool {
    vis.is_none_or(|v| v.nodes.is_empty() || v.cluster_at(feet + Vec3::Y * 1.0).is_some())
}

/// The areas of `nav` a player can walk to from the spawn `points` and
/// back (all that can be walked to, if none can walk back): the mesh's
/// main connected region.
fn region(nav: &NavMesh, points: &[Vec3]) -> Vec<bool> {
    let n = nav.areas.len();
    if n == 0 {
        return Vec::new();
    }
    let mut forward = vec![Vec::new(); n];
    let mut back = vec![Vec::new(); n];
    for (i, a) in nav.areas.iter().enumerate() {
        for (j, _) in &a.links {
            forward[i].push(*j);
            back[*j].push(i);
        }
    }
    let starts: Vec<usize> = points.iter().filter_map(|p| nav.area_at(*p + Vec3::Y * 0.1)).collect();
    let starts = if starts.is_empty() {
        points.iter().filter_map(|p| nav.nearest_area(*p)).collect()
    } else {
        starts
    };
    let reach = |edges: &[Vec<usize>]| {
        let mut seen = vec![false; n];
        let mut stack = starts.clone();
        for s in &stack {
            seen[*s] = true;
        }
        while let Some(i) = stack.pop() {
            for j in &edges[i] {
                if !seen[*j] {
                    seen[*j] = true;
                    stack.push(*j);
                }
            }
        }
        seen
    };
    let (f, b) = (reach(&forward), reach(&back));
    let both: Vec<bool> = f.iter().zip(&b).map(|(a, b)| *a && *b).collect();
    if both.iter().any(|x| *x) { both } else { f }
}
