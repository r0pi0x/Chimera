//! `points = "random_hidden"`: a deathmatch's spawning, spread over the
//! whole map and out of the enemies' sight (`pick`, called by
//! `runtime::spawn_point`); its spots, facing and draws are also
//! `points = "scored"`'s (`spawn_score`, the gun games').
//!
//! - Spots (`SpawnSpots`, made once per loaded map on the first spawn, in a
//!   front end behind the map's loading screen and on a dedicated server
//!   while its new map settles (`settle_spots`, also
//!   before a minigame starts when a registered one spawns on spots), else
//!   a slice a tick from the match's start (`prepare_spots`,
//!   `nav_gen::SliceFloods`: the map's spawn points until they are ready),
//!   and removed with the map, `MapScopedApp::map_scoped`): points on the
//!   floors of the map's navigation mesh: its own (`map::nav::NavMesh`, a
//!   whole kit's), else its navigation file's (`map::MapFloors`, loaded
//!   whatever the kit: the floors its author marked as played; cs_militia's
//!   yard opens onto hills nobody plays) where the generated walk below
//!   reaches them too (`Walk::on_walk`: the file links floors by ladders, jumps
//!   and drops a walker without them never gets to or from, and bots
//!   spawned on cs_militia's high floors, in a part of their own mesh
//!   apart from the others, chased each other without a path), else one
//!   generated from its
//!   collision (`map::nav_gen`) with a walking player about as wide as the
//!   widest (`walker`, its box sliding up to 0.2 m off a cell's centre to
//!   fit a doorway), climbing what the map's game's players climb
//!   (`SpawnClimbs`: Combat Arms', CS:S's and DoD:S's jump-ups, 0.8 m; then bounded
//!   to the clusters the walk reaches where the map has visibility data),
//!   going through the physics props a player gets past
//!   (`map::prop_physics::walked_through`, by the map's game's rule,
//!   `PlayerProps`: multiplayer props players walk through, light ones
//!   their shadow shoves; under DoD:S's turbo physics only debris and the
//!   multiplayer props still asleep since load; a spot is still never inside
//!   one, `stuck_at` meets them), and meeting the map's player clips (their players-only collider,
//!   `core::MapPlayerClips`, which `CHARACTER_FILTER` meets; cs_havana's
//!   clipped yard once held a player). A mesh made for another walker,
//!   `NavMesh::generated`, is not used (Combat Arms' AI volumes link
//!   through closed doors, which once put a player in Blood Money's vault:
//!   a spot must be walked to from the spawn points through the world as
//!   it stands),
//!   a grid
//!   about `SPOT_GRID` m apart in the areas that can be walked to from the
//!   spawn points and back (the mesh's main connected region), each spot
//!   moved a seeded random amount (up to `SPOT_NUDGE` per axis) off the
//!   area's centred grid, in a seeded random order. The nudge keeps eyes
//!   off the map's grid lines: areas are often symmetric about one, and
//!   centred spots put a whole row of eyes in the plane where two faces
//!   meet, which then shows as a vertical line through the frame (de_dust2's
//!   "seam" between a bump-lit and a flat-lit displacement, 2026-10-30). Each spot is checked (`spot_ok`): a floor under
//!   it, a standing player's box (Source's 32 by 72 units, from a stair
//!   step over the feet to the head, `standing_box`; a capsule missed the
//!   corners) meets neither the world nor a player clip (`stuck_at`), it
//!   stands in open space (`open_at`: at least `MIN_HEADROOM` above the
//!   feet, the tallest player's height and some, and under a ceiling
//!   lower than `LOW_CEILING` most level directions open for
//!   `OPEN_REACH`: never in a duct, a crawlspace or a low narrow
//!   passage, as ctf_turbine's vents once gave), it is inside the map (on a map with visibility data, its chest in a
//!   leaf of a cluster: not solid, not the void outside the world), and
//!   outside water, every zone a game registers as hurting or
//!   killing (`SpawnHazards`) and, unless the match is a team one, the
//!   map's team spawn rooms (`SpawnRooms`, `map::spawn_room_zones`: TF2's
//!   `func_respawnroom`s, a team's points, resupply and doors). Without a mesh, or spots, the map's spawn
//!   points are the spots; so they are, at once, on a map that asks for
//!   it (`map::code_maps::SpawnOnPoints`: the range's firing line), facing
//!   its way when it says one (`fixed_yaw`). `candidates` lists every spot without the
//!   `MAX_SPOTS` cap (the sweep of the stock maps). The spots are kept on
//!   disk (`map::spot_cache`, 4 MB, for every game) under a key of the
//!   map's content, the seed, the spawn points, the walker, the play area,
//!   the hazards, the meshes and `SPOT_CACHE_VERSION` (bump it when
//!   `spot_ok` or `raw_spots` change what they find), and read back on a
//!   task on the map's next load: no flood, no tests, the same spots.
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
//!   the spots; `scored` also weighs each down toward the nearest enemy),
//!   turned a random +-10 degrees by the seeded draws, pitched
//!   2 degrees down. About 20 microseconds a spawn
//!   (`facing_costs_well_under_a_millisecond` measures it). `rays = 0`
//!   brings back eight fixed directions.
//! - Determinism: the shuffle and the picks draw on a seeded generator
//!   (`SpawnSeed`, else a hash of the minigame's id), started over with
//!   the match (`reset`); candidates and enemies are walked in a fixed
//!   order. Nothing global.
//!
//! Known shortcuts: a map's first load still walks on the main thread (the
//! flood and the tests need the physics world's queries, which a task
//! can't hold: a slice a frame), only the later loads read the kept
//! spots; sight is three rays per enemy from the eye without a
//! field of view or fog (an enemy facing away still "sees"); the spots are
//! chosen once, so a door that opens later isn't considered; a map whose
//! collision isn't built yet at the first spawn gets the spawn points for
//! up to `BUILD_TRIES` spawns before the result is kept; on a map without
//! a whole kit's mesh the spots are where a player walks and jumps up
//! from the spawn points (no ladders or crouching), on its navigation
//! file's floors where it has one, so a floor reached only by a ladder is
//! never one (a map with a file pays the walk's flood too); "inside the map" is the visibility
//! tree's clusters, so a map without visibility data (Combat Arms')
//! relies on the walk alone.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, platform::collections::HashSet, prelude::*};

use crate::{
    core::{
        CHARACTER_FILTER, Connecting, Dead, Health, Intent, MapBrush, MapGame, MapWater, SOLID_LAYERS, SpawnPoint,
        Spectating, Team,
    },
    map::{
        MapFloors, MapPropCollider, PhysicsProp, SPAWN_LIFT,
        code_maps::SpawnOnPoints,
        nav::{NavMesh, flags},
        nav_gen::{self, NavGenParams},
        play_area::{self, PlanPolygon},
        prop_physics,
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
/// The standing player a spot must hold: the widest the hosted games make
/// (Source's hull, 32 units square and 72 tall; Combat Arms' is 30 by
/// 100 of its 0.0185 m units), half its width and its height (m).
pub const HALF_WIDTH: f32 = 16.0 * 0.0254;
pub const HEIGHT: f32 = 72.0 * 0.0254;
/// The box is tested from a stair step over the feet (18 units: a slope or
/// a stair under its corners is walked off, not stuck in) to the head.
pub const STEP: f32 = 18.0 * 0.0254;
/// An enemy's eye above its centre (m).
pub const EYE: f32 = 0.7;
/// How much shorter than the step-to-head span the tested box is (half
/// above, half below: clear of the step's and the ceiling's surfaces).
const SKIN: f32 = 0.05;

/// The box `spot_ok` tests: half extents, and its centre above the feet.
pub fn standing_box() -> (Vec3, f32) {
    (
        Vec3::new(HALF_WIDTH, (HEIGHT - STEP - SKIN) / 2.0, HALF_WIDTH),
        STEP + (HEIGHT - STEP) / 2.0,
    )
}

/// Whether a player standing at `feet` (its box from a step up to the
/// head, `standing_box`) meets what characters collide with (`filter`:
/// the characters' layers, the player clips' collider among them).
pub fn stuck_at(q: &SpatialQuery, filter: &SpatialQueryFilter, feet: Vec3) -> bool {
    let (half, up) = standing_box();
    let mut blocked = false;
    q.shape_intersections_callback(
        &Collider::cuboid(half.x * 2.0, half.y * 2.0, half.z * 2.0),
        feet + Vec3::Y * up,
        Quat::IDENTITY,
        filter,
        |_| {
            blocked = true;
            false
        },
    );
    blocked
}

/// Seeds the spawn draws of a match (else a hash of the minigame's id).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnSeed(pub u32);

pub use crate::map::{
    SpawnClimbAppExt, SpawnClimbs, SpawnHazardAppExt, SpawnHazards, SpawnRoomAppExt, SpawnRooms, spawn_room_zones,
};

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

/// The generator of a match's draws, and what `points = "scored"`
/// remembers over the match (`spawn_score::Memory`: recent danger, each
/// player's last spawn, the match start's placements).
#[derive(Resource, Clone, Debug)]
pub struct SpawnDraws {
    pub(super) rng: Rng,
    pub(super) memory: super::spawn_score::Memory,
}

impl SpawnDraws {
    pub(super) fn new(world: &World) -> Self {
        Self {
            rng: Rng::new(seed_of(world) ^ 0x5eed),
            memory: Default::default(),
        }
    }
}

/// The match starts over: the draws begin again.
pub fn reset(world: &mut World) {
    world.remove_resource::<SpawnDraws>();
}

pub(super) fn seed_of(world: &World) -> u32 {
    world.get_resource::<SpawnSeed>().map(|s| s.0).unwrap_or_else(|| {
        let id = world.get_resource::<super::ActiveMinigame>().map_or("", |a| a.def.id.as_str());
        crc32(id.as_bytes())
    })
}

/// The map's spawn points' feet, in a fixed order.
pub(super) fn spawn_feet(world: &mut World) -> Vec<Vec3> {
    let mut points: Vec<Vec3> = world
        .query_filtered::<&Transform, (With<SpawnPoint>, Without<Intent>)>()
        .iter(world)
        .map(|t| t.translation - Vec3::Y * SPAWN_LIFT)
        .collect();
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z)));
    points
}

/// `e`'s spawn: a spot as the module docs say; None when the map has no
/// spawn point at all.
pub fn pick(world: &mut World, e: Entity, min_distance: f32, facing: Facing) -> Option<Transform> {
    let points = spawn_feet(world);
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
    let fixed = fixed_yaw(world);
    let mut draws = world.remove_resource::<SpawnDraws>().unwrap_or_else(|| SpawnDraws::new(world));
    let characters: Vec<Entity> = world.iter_entities().filter(|r| r.contains::<Intent>()).map(|r| r.id()).collect();
    let mut state = SystemState::<SpatialQuery>::new(world);
    let Ok(q) = state.get(world) else {
        world.insert_resource(draws);
        return None;
    };
    let spots = world.resource::<SpawnSpots>();
    let n = spots.spots.len();
    let start = ((draws.rng.next() * n as f32) as usize).min(n.saturating_sub(1));
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
        let (yaw, pitch) = match fixed {
            Some(yaw) => (yaw, 0.0),
            None => face(&q, &sight, s, &facing, centroid, None, &mut draws.rng),
        };
        Transform::from_translation(s + Vec3::Y * SPAWN_LIFT)
            .with_rotation(Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0))
    });
    world.insert_resource(draws);
    at
}

/// The mesh the spots are drawn on where the map brings one: its own
/// navigation mesh (a whole kit's), else its navigation file's floors
/// (`MapFloors`); a generated one (another walker's, bots') is not used.
pub(super) fn map_mesh<'a>(nav: Option<&'a NavMesh>, floors: Option<&'a MapFloors>) -> Option<&'a NavMesh> {
    nav.filter(|n| !n.generated).or(floors.map(|f| &*f.0))
}

/// Whether the spots need the generated walk (`walker_for`): as their
/// mesh, or to keep the navigation file's spots where it reaches; not
/// where the map's own mesh (a whole kit's) is used.
fn needs_walk(nav: Option<&NavMesh>) -> bool {
    nav.is_none_or(|n| n.generated)
}

/// The walk's main region (`region`) for keeping the file's spots on it
/// (`on_walk`).
struct Walk<'a> {
    mesh: &'a NavMesh,
    inside: Vec<bool>,
}

impl<'a> Walk<'a> {
    fn new(mesh: &'a NavMesh, points: &[Vec3]) -> Self {
        Self {
            inside: region(mesh, points),
            mesh,
        }
    }

    /// Whether `p` (a point of the file's floors) is on the walk's main
    /// region: over one of its areas (or within `ON_WALK_SLACK` m of the
    /// nearest), within `ON_WALK_HEIGHT` m of its floor.
    fn on_walk(&self, p: Vec3) -> bool {
        let Some(i) = self.mesh.area_at(p + Vec3::Y * 0.3).or_else(|| self.mesh.nearest_area(p)) else {
            return false;
        };
        let a = &self.mesh.areas[i];
        let c = a.closest_point(p);
        self.inside.get(i).copied().unwrap_or(false)
            && c.xz().distance(p.xz()) <= ON_WALK_SLACK
            && (c.y - p.y).abs() <= ON_WALK_HEIGHT
    }
}

/// How far off the walk's areas a file's spot may be (m): the walk's grid
/// cells are half a metre, the file's floors drawn by hand.
const ON_WALK_SLACK: f32 = 0.5;
/// How far above or below the walk's floor (m): a stair's step.
const ON_WALK_HEIGHT: f32 = 1.0;

/// `raw_spots` on `nav`, keeping only those on `walk`'s main region where
/// given.
fn raw_spots_on(nav: &NavMesh, walk: Option<&Walk>, points: &[Vec3], seed: u32) -> Vec<(f32, f32, f32)> {
    let mut raw = raw_spots(nav, points, seed);
    if let Some(walk) = walk {
        raw.retain(|&(x, z, y)| walk.on_walk(Vec3::new(x, y, z)));
    }
    raw
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
    Some(face(&q, &filter, feet, facing, centroid, None, &mut Rng::new(seed)))
}

/// The yaw and pitch (radians) a spawn at `feet` faces: `Facing`'s rule.
/// With `rays == 0`, the longest of eight fixed directions, level.
/// `away`: what `scored` turns from: each direction's openness weighed
/// down by up to `Away::share` the more it looks at `Away::nearest`, and
/// to almost nothing within `Away::cone` degrees of any of `Away::seen`
/// (enemies in sight of the spot).
pub(super) fn face(
    q: &SpatialQuery,
    filter: &SpatialQueryFilter,
    feet: Vec3,
    f: &Facing,
    centroid: Option<Vec3>,
    away: Option<&Away>,
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
    let flat = |at: Vec3| Dir3::new(Vec3::new(at.x - feet.x, 0.0, at.z - feet.z)).ok();
    let to_enemy = away.and_then(|a| a.nearest.and_then(flat).map(|d| (d, a.share)));
    let seen: Vec<Dir3> = away.map_or(Vec::new(), |a| a.seen.iter().filter_map(|p| flat(*p)).collect());
    let cone = away.map_or(1.0, |a| a.cone.to_radians().cos());
    let smooth: Vec<f32> = (0..n)
        .map(|k| {
            let sum: f32 = (0..=2 * half).map(|t| free[(k + n + t - half) % n]).sum();
            let open = sum / (2 * half + 1) as f32;
            let dir = Quat::from_rotation_y(yaw_of(k)) * Vec3::NEG_Z;
            let open = match to_enemy {
                Some((d, w)) => open * (1.0 - w * dir.dot(*d).max(0.0)),
                None => open,
            };
            if seen.iter().any(|d| dir.dot(**d) >= cone) { open * 0.01 } else { open }
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

/// What a `scored` spawn turns from (`face`).
#[derive(Clone, Debug, Default)]
pub struct Away {
    pub nearest: Option<Vec3>,
    pub share: f32,
    pub seen: Vec<Vec3>,
    /// Half the field of view and some (degrees).
    pub cone: f32,
}

/// The map's spots, made now if they aren't kept yet. `points`: the map's
/// spawn points' feet.
pub(super) fn ensure_spots(world: &mut World, points: &[Vec3]) {
    if world.get_resource::<SpawnSpots>().is_some_and(|s| s.ready) {
        return;
    }
    if world.contains_resource::<SpawnOnPoints>() {
        world.insert_resource(points_as_spots(points.to_vec()));
        return;
    }
    // A front end makes them a slice a tick (`prepare_spots`): the map's
    // spawn points until they are ready.
    if world.contains_resource::<nav_gen::SliceFloods>() {
        if world.get_resource::<SpawnSpots>().is_none_or(|s| s.spots.is_empty()) {
            let mut spots = world.remove_resource::<SpawnSpots>().unwrap_or_default();
            spots.spots = points.to_vec();
            world.insert_resource(spots);
        }
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

/// The yaw every spawn faces on a map that spawns at its points and says
/// which way (`SpawnOnPoints::facing`: the range's down range); None:
/// `face`'s rule.
pub(super) fn fixed_yaw(world: &World) -> Option<f32> {
    let dir = world.get_resource::<SpawnOnPoints>()?.facing?;
    (dir.xz().length_squared() > 1e-6).then(|| (-dir.x).atan2(-dir.z))
}

/// The map's spawn points as the spots, kept (`SpawnOnPoints`).
fn points_as_spots(points: Vec<Vec3>) -> SpawnSpots {
    SpawnSpots {
        spots: points,
        from_nav: false,
        ready: true,
        tries: 0,
    }
}

/// The spots on their way in a front end (`prepare_spots`, `settle_spots`):
/// the same steps as `build`, so the same spots from the same seed.
#[derive(Resource)]
pub struct SpotsJob {
    points: Vec<Vec3>,
    seed: u32,
    hazards: Vec<MapBrush>,
    /// The flood of the spots' mesh, where the map has none of its own.
    flood: Option<nav_gen::NavGenJob>,
    /// Mesh points to test, the shuffled order, and how far along.
    raw: Vec<(f32, f32, f32)>,
    at: usize,
    ready_to_test: bool,
    found: Vec<Vec3>,
    /// The match's slice: spots only inside (`map::play_area`).
    plan: Option<PlanPolygon>,
    /// Where the spots are kept on disk and their key (`map::spot_cache`).
    cache: Option<(std::path::PathBuf, String)>,
    /// The kept spots on their way in from the disk, on a task.
    read: Option<bevy::tasks::Task<Option<Vec<Vec3>>>>,
    /// The spots came from the disk: nothing to write back.
    from_disk: bool,
}

/// The version of the spots' code for their key on disk
/// (`map::spot_cache`): bumped when `spot_ok`, `raw_spots` or what they
/// call change what they find. The constants they read are in the key.
const SPOT_CACHE_VERSION: &str = "2";

/// A digest of a mesh for the key: its areas' ids, flags and boxes.
fn mesh_digest(mesh: Option<&NavMesh>) -> Vec<u8> {
    let Some(m) = mesh else { return vec![0] };
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |v: u32| {
        for b in v.to_le_bytes() {
            h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
    };
    for a in &m.areas {
        eat(a.id);
        eat(a.flags as u32);
        for v in [a.min.x, a.min.y, a.max.x, a.max.y].into_iter().chain(a.heights) {
            eat(v.to_bits());
        }
    }
    let mut out = vec![1, m.generated as u8];
    out.extend((m.areas.len() as u64).to_le_bytes());
    out.extend(h.to_le_bytes());
    out
}

/// The spots' key on disk, with where (`map::spot_cache`): everything that
/// decides them, so another map, seed, walker, play area, hazard or mesh
/// is another entry.
#[allow(clippy::too_many_arguments)]
fn cache_of(
    world: &World,
    points: &[Vec3],
    seed: u32,
    walker: &(NavGenParams, Option<NavGenParams>),
    hazards: &[MapBrush],
    plan: Option<PlanPolygon>,
) -> Option<(std::path::PathBuf, String)> {
    let dir = match world.get_resource::<crate::map::spot_cache::SpotCacheDir>() {
        Some(d) => d.0.clone(),
        None => crate::map::spot_cache::dir(),
    }?;
    let map = world.get_resource::<crate::map::MapFile>()?;
    let bits: Vec<u8> = points.iter().flat_map(|p| p.to_array().map(|v| v.to_bits().to_le_bytes())).flatten().collect();
    let hazards = postcard::to_allocvec(hazards).ok()?;
    let consts = format!("{MAX_SPOTS} {SPOT_GRID} {SPOT_NUDGE} {SPOT_NUDGE_SEED} {SPAWN_LIFT} {BUILD_TRIES}");
    let vis = world.get_resource::<ActiveVisibility>().map_or(0, |v| v.0.nodes.len());
    let key = crate::map::spot_cache::key(
        map,
        &[
            ("seed", seed.to_le_bytes().to_vec()),
            ("points", bits),
            ("walker", format!("{walker:?}").into_bytes()),
            ("plan", format!("{plan:?}").into_bytes()),
            ("hazards", hazards),
            ("consts", consts.into_bytes()),
            ("vis", vis.to_le_bytes().to_vec()),
            ("nav", mesh_digest(world.get_resource::<NavMesh>())),
            ("floors", mesh_digest(world.get_resource::<MapFloors>().map(|f| &*f.0))),
        ],
        SPOT_CACHE_VERSION,
    )?;
    Some((dir, key))
}

/// Ticks between tries when the collision wasn't there yet.
const SPOT_RETRY_TICKS: u8 = 30;

/// Mesh points tested for a cell of the flood (a ray and a capsule each, a
/// third of a cell's work).
const SPOTS_PER_CELL: usize = 1;

impl SpotsJob {
    fn new(world: &mut World) -> Option<Self> {
        let mut points: Vec<Vec3> = world
            .query_filtered::<&Transform, (With<SpawnPoint>, Without<Intent>)>()
            .iter(world)
            .map(|t| t.translation - Vec3::Y * SPAWN_LIFT)
            .collect();
        if points.is_empty() {
            return None;
        }
        points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z)));
        // A map that spawns at its points only: they are the spots, at once.
        if world.contains_resource::<SpawnOnPoints>() {
            world.insert_resource(points_as_spots(points));
            return None;
        }
        let hazards = keep_out_zones(world);
        let walker = walker_for(world);
        let flood = needs_walk(world.get_resource::<NavMesh>()).then(|| nav_gen::NavGenJob::new(&points, &walker.0, walker.1));
        let plan = play_area::held_plan(world);
        let seed = seed_of(world);
        // Kept on disk from an earlier load: read back on a task.
        let cache = cache_of(world, &points, seed, &walker, &hazards, plan);
        let read = cache.clone().filter(|(d, k)| crate::map::spot_cache::has(d, k)).map(|(dir, key)| {
            let pool = bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
            chimera_core::tasks::spawn(&pool, async move {
                let spots = crate::map::spot_cache::read(&dir, &key).filter(|s| !s.is_empty());
                if spots.is_none() {
                    crate::map::spot_cache::remove(&dir, &key);
                }
                spots
            })
        });
        Some(Self {
            seed,
            points,
            hazards,
            flood,
            raw: Vec::new(),
            at: 0,
            ready_to_test: false,
            found: Vec::new(),
            plan,
            cache,
            read,
            from_disk: false,
        })
    }

    /// Up to `cells` cells' worth more; true when every point is tested.
    fn slice(
        &mut self,
        q: &SpatialQuery,
        characters: &[Entity],
        props: &HashSet<Entity>,
        nav: Option<&NavMesh>,
        vis: Option<&MapVisibility>,
        cells: usize,
        wait: bool,
    ) -> bool {
        if let Some(mut task) = self.read.take() {
            match chimera_core::tasks::poll(&mut task, wait) {
                None => {
                    self.read = Some(task);
                    return false;
                }
                Some(Some(spots)) => {
                    self.found = spots;
                    self.flood = None;
                    self.from_disk = true;
                    return true;
                }
                // Gone or broken: made as if it was never kept.
                Some(None) => {}
            }
        }
        if let Some(mut flood) = self.flood.take() {
            match flood.step(q, &|e| characters.contains(&e) || props.contains(&e), vis, cells) {
                Some(walk) => {
                    // The file's floors where the walk reaches them, else
                    // the walk.
                    self.raw = match nav {
                        Some(file) => raw_spots_on(file, Some(&Walk::new(&walk, &self.points)), &self.points, self.seed),
                        None => raw_spots(&walk, &self.points, self.seed),
                    };
                    self.ready_to_test = true;
                }
                None => self.flood = Some(flood),
            }
            return false;
        }
        if !self.ready_to_test {
            // The map's own mesh.
            let Some(nav) = nav else { return true };
            self.raw = raw_spots(nav, &self.points, self.seed);
            self.ready_to_test = true;
        }
        let floor = SpatialQueryFilter::from_mask(CHARACTER_FILTER).with_excluded_entities(characters.iter().copied());
        let end = self.at.saturating_add(cells.saturating_mul(SPOTS_PER_CELL)).min(self.raw.len());
        for k in self.at..end {
            if self.found.len() >= MAX_SPOTS {
                break;
            }
            let (x, z, y) = self.raw[k];
            if self.plan.is_some_and(|p| !p.contains(Vec2::new(x, z))) {
                continue;
            }
            if let Some(feet) = spot_ok(q, &floor, &self.hazards, vis, Vec3::new(x, y, z)) {
                self.found.push(feet);
            }
        }
        self.at = end;
        self.at >= self.raw.len() || self.found.len() >= MAX_SPOTS
    }

    /// The spots it found (none: the map's spawn points, and another try if
    /// any are left).
    fn finish(self, tries: u8) -> SpawnSpots {
        let tries = tries + 1;
        let empty = self.found.is_empty();
        let how = if self.from_disk { "read back from the disk" } else { "made a slice at a time" };
        info!("spawn spots: {} {how} (try {tries})", self.found.len());
        if let (Some((dir, key)), false, false) = (self.cache, self.from_disk, empty) {
            // Kept for the map's next load, written on a task.
            let spots = self.found.clone();
            let pool = bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
            chimera_core::tasks::spawn(&pool, async move {
                crate::map::spot_cache::write(&dir, &key, &spots, crate::map::spot_cache::CAP);
            })
            .detach();
        }
        SpawnSpots {
            from_nav: !empty,
            ready: !empty || tries >= BUILD_TRIES,
            tries,
            spots: if empty { self.points } else { self.found },
        }
    }
}

/// What spots keep out of: every registered hazard (`SpawnHazards`), the
/// map's water and, unless the match playing is a team one, its team spawn
/// rooms (`map::spawn_room_zones`: TF2's, whose doors and resupply are a
/// team's; a free-for-all spawns anyone anywhere but there).
fn keep_out_zones(world: &mut World) -> Vec<MapBrush> {
    let mut zones: Vec<MapBrush> = Vec::new();
    let sources: Vec<_> = world.get_resource::<SpawnHazards>().map(|h| h.0.clone()).unwrap_or_default();
    for source in sources {
        zones.extend(source(world));
    }
    if let Some(water) = world.get_resource::<MapWater>() {
        zones.extend(water.0.iter().map(|v| v.brush.clone()));
    }
    let teams = world
        .get_resource::<super::ActiveMinigame>()
        .is_some_and(|a| a.def.teams.teams() != crate::mode::Teams::FreeForAll);
    if !teams {
        zones.extend(crate::map::spawn_room_zones(world));
    }
    zones
}

fn on_spots(def: &super::MinigameDef) -> bool {
    matches!(def.spawn.points, super::def::Points::RandomHidden | super::def::Points::Scored)
}

/// Whether the match playing spawns on spots.
fn spawns_on_spots(active: Option<&super::ActiveMinigame>) -> bool {
    active.is_some_and(|a| on_spots(a.def))
}

/// Whether a map loading now will be played on spots: the match playing
/// spawns on them, or, with none playing yet, a registered definition
/// does (a party app's menu loads the map, then starts its minigame).
fn loads_for_spots(world: &World) -> bool {
    match world.get_resource::<super::ActiveMinigame>() {
        Some(a) => on_spots(a.def),
        None => world.get_resource::<super::Minigames>().is_some_and(|m| m.all().any(on_spots)),
    }
}

/// Make the map's spots a slice a tick where floods are sliced
/// (`nav_gen::SliceFloods`), from the first tick the match plays: the first
/// respawn doesn't stop the frame for them (over a thousand million
/// instructions on de_dust2, docs/performance.md, "Nothing blocks the main
/// thread"). `settle_spots` does it behind the loading screen where it can.
#[allow(clippy::too_many_arguments)]
pub fn prepare_spots(
    floods: Option<Res<nav_gen::SliceFloods>>,
    active: Option<Res<super::ActiveMinigame>>,
    spots: Option<Res<SpawnSpots>>,
    job: Option<ResMut<SpotsJob>>,
    nav: Option<Res<NavMesh>>,
    floors: Option<Res<MapFloors>>,
    vis: Option<Res<ActiveVisibility>>,
    characters: Query<Entity, With<Intent>>,
    props: Query<
        (
            Entity,
            Option<&PhysicsProp>,
            Has<prop_physics::StartAsleep>,
            Has<prop_physics::DebrisProp>,
        ),
        With<MapPropCollider>,
    >,
    game: Option<Res<MapGame>>,
    rules: Option<Res<prop_physics::PlayerPropRules>>,
    q: SpatialQuery,
    wait_tasks: Option<Res<chimera_core::tasks::WaitForTasks>>,
    mut wait: Local<u8>,
    mut commands: Commands,
) {
    if floods.is_none() || !spawns_on_spots(active.as_deref()) || spots.as_ref().is_some_and(|s| s.ready) {
        return;
    }
    if *wait > 0 {
        *wait -= 1;
        return;
    }
    let Some(mut job) = job else {
        commands.queue(|w: &mut World| {
            if let Some(job) = SpotsJob::new(w) {
                w.insert_resource(job);
            }
        });
        return;
    };
    let characters: Vec<Entity> = characters.iter().collect();
    // The map's game's rule, as `prop_physics::walked_through`.
    let rule = game.zip(rules).map_or(prop_physics::PlayerProps::Shadow, |(g, r)| r.of(&g.0));
    let props: HashSet<Entity> = props
        .iter()
        .filter(|(_, p, asleep, debris)| prop_physics::prop_passes(*p, prop_physics::PropGroup { asleep: *asleep, debris: *debris }, rule))
        .map(|(e, ..)| e)
        .collect();
    let mesh = map_mesh(nav.as_deref(), floors.as_deref());
    if job.slice(&q, &characters, &props, mesh, vis.as_ref().map(|v| &*v.0), nav_gen::CELLS_PER_TICK, wait_tasks.is_some()) {
        let tries = spots.as_ref().map_or(0, |s| s.tries);
        commands.queue(move |w: &mut World| {
            if let Some(job) = w.remove_resource::<SpotsJob>() {
                w.insert_resource(job.finish(tries));
            }
        });
        // The collision may not have been built yet: another try later.
        if job.found.is_empty() {
            *wait = SPOT_RETRY_TICKS;
        }
    }
}

/// `prepare_spots` behind the loading screen (`map::MapHooks::settle`), a
/// larger slice a frame: for the match playing on spots, or, with none
/// playing yet, when a registered definition spawns on them
/// (`loads_for_spots`: a party app's menu loads the map before it starts
/// the minigame, whose first spawns then came at the map's spawn points
/// for the 8 s the slices took on de_dust2, bots in each other's view).
pub fn settle_spots(world: &mut World) -> bool {
    if !world.contains_resource::<nav_gen::SliceFloods>()
        || !loads_for_spots(world)
        || world.get_resource::<SpawnSpots>().is_some_and(|s| s.ready)
    {
        return true;
    }
    let Some(mut job) = world.remove_resource::<SpotsJob>().or_else(|| SpotsJob::new(world)) else { return true };
    let characters: Vec<Entity> = world.iter_entities().filter(|r| r.contains::<Intent>()).map(|r| r.id()).collect();
    let props = prop_physics::walked_through(world);
    let vis = world.get_resource::<ActiveVisibility>().map(|v| v.0.clone());
    let wait = chimera_core::tasks::waits(world);
    let done = {
        let mut state = SystemState::<SpatialQuery>::new(world);
        let Ok(q) = state.get(world) else { return true };
        let mesh = map_mesh(world.get_resource::<NavMesh>(), world.get_resource::<MapFloors>());
        job.slice(&q, &characters, &props, mesh, vis.as_deref(), nav_gen::CELLS_PER_LOAD_FRAME, wait)
    };
    if !done {
        world.insert_resource(job);
        return false;
    }
    let tries = world.get_resource::<SpawnSpots>().map_or(0, |s| s.tries);
    let spots = job.finish(tries);
    let ready = spots.ready;
    world.insert_resource(spots);
    ready || !world.contains_resource::<nav_gen::SliceFloods>()
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
/// own: a player walking (the default step) about as wide as the widest
/// player (`nav_gen::walking_player`), its box standing up to 0.2 m off a
/// cell's centre to fit (`NavGenParams::slide`: a doorway the grid's
/// centres miss).
pub fn walker() -> NavGenParams {
    NavGenParams {
        slide: 0.2,
        ..nav_gen::walking_player()
    }
}

/// `walker` on the loaded map: climbing what its game's players climb
/// (`SpawnClimbs`, by `core::MapGame`); with the walk that bounds it where
/// it climbs more than a walk and the map has visibility data
/// (`nav_gen::Bounds`: only the clusters a walking player reaches, so a
/// jump over a low wall doesn't leave the map).
pub fn walker_for(world: &World) -> (NavGenParams, Option<NavGenParams>) {
    let mut p = walker();
    nav_gen::hold(&mut p, world);
    if let (Some(game), Some(climbs)) = (world.get_resource::<MapGame>(), world.get_resource::<SpawnClimbs>())
        && let Some(h) = climbs.of(&game.0)
    {
        p.step = p.step.max(h);
    }
    let vis = world.get_resource::<ActiveVisibility>().is_some_and(|v| !v.0.nodes.is_empty());
    (p, (p.step > walker().step && vis).then(walker))
}

/// The spots of the loaded map, at most `cap` (`ready` unset; empty
/// without a mesh).
fn build(world: &mut World, points: &[Vec3], cap: usize) -> SpawnSpots {
    let seed = seed_of(world);
    let hazards = keep_out_zones(world);
    let vis = world.get_resource::<ActiveVisibility>().map(|v| v.0.clone());
    let (walker, bound) = walker_for(world);
    let characters: Vec<Entity> = world.iter_entities().filter(|r| r.contains::<Intent>()).map(|r| r.id()).collect();
    let mut passes = prop_physics::walked_through(world);
    passes.extend(characters.iter().copied());
    let mut state = SystemState::<SpatialQuery>::new(world);
    let Ok(q) = state.get(world) else { return SpawnSpots::default() };
    let ignore = |e: Entity| passes.contains(&e);
    // The map's own mesh; one generated for another walker (bots') may
    // climb where a player can't: ours instead, which also keeps the
    // file's spots where it reaches.
    let walk = needs_walk(world.get_resource::<NavMesh>()).then(|| {
        let bounds = bound.zip(vis.as_deref()).map(|(walker, vis)| nav_gen::Bounds { vis, walker });
        nav_gen::generate_within(&q, &ignore, points, &walker, bounds)
    });
    let raw = match (map_mesh(world.get_resource::<NavMesh>(), world.get_resource::<MapFloors>()), &walk) {
        (Some(file), Some(walk)) => raw_spots_on(file, Some(&Walk::new(walk, points)), points, seed),
        (Some(own), None) => raw_spots(own, points, seed),
        (None, Some(walk)) => raw_spots(walk, points, seed),
        (None, None) => Vec::new(),
    };
    let floor = SpatialQueryFilter::from_mask(CHARACTER_FILTER).with_excluded_entities(characters.iter().copied());
    let mut spots = Vec::new();
    for (x, z, y) in raw {
        if spots.len() >= cap {
            break;
        }
        if walker.plan.is_some_and(|p| !p.contains(Vec2::new(x, z))) {
            continue;
        }
        if let Some(feet) = spot_ok(&q, &floor, &hazards, vis.as_deref(), Vec3::new(x, y, z)) {
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

/// The points to test on mesh `nav`, in the order tried (a seeded shuffle):
/// (x, z, height).
fn raw_spots(nav: &NavMesh, points: &[Vec3], seed: u32) -> Vec<(f32, f32, f32)> {
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
    raw
}

/// The feet of a valid standing spot near the mesh point `p`, if there is
/// one: a walkable floor within 0.5 m of the mesh's height (the world's
/// or a player clip's top), a standing player's box clear of what
/// characters meet (`stuck_at`), in open space (`open_at`: headroom, and
/// room around under a low ceiling), inside the map (`vis`: the chest in
/// a cluster's leaf), and no hazard at the feet, chest or head.
fn spot_ok(q: &SpatialQuery, filter: &SpatialQueryFilter, hazards: &[MapBrush], vis: Option<&MapVisibility>, p: Vec3) -> Option<Vec3> {
    let from = p + Vec3::Y * 0.6;
    let hit = q.cast_ray(from, Dir3::NEG_Y, 1.2, true, filter)?;
    if hit.normal.y < 0.7 || hit.distance < 0.05 {
        return None;
    }
    let feet = Vec3::new(p.x, from.y - hit.distance, p.z);
    if !inside_map(vis, feet) || stuck_at(q, filter, feet) || !open_at(q, filter, feet) {
        return None;
    }
    let bad = [0.1, 1.0, 1.7].iter().any(|h| {
        let at = feet + Vec3::Y * *h;
        hazards.iter().any(|b| b.contains(at))
    });
    (!bad).then_some(feet)
}

/// The least headroom a spot has (m, from the feet to what is above):
/// the tallest standing player the hosted games make and some (TF2's 82
/// units, 2.08 m), so nobody spawns under a ceiling it can't stand under
/// (a crawlspace a 72-unit box still fits).
pub const MIN_HEADROOM: f32 = 2.2;
/// Headroom (m) under which a spot also needs room around it
/// (`OPEN_REACH`, `OPEN_DIRECTIONS`): a ceiling this low over a floor
/// walled in on most sides is a duct or a crawlspace, not a place to
/// spawn (ctf_turbine's 96-unit vent ducts, 2.44 m inside, where a player
/// once spawned).
pub const LOW_CEILING: f32 = 2.6;
/// How far (m) a level ray from the chest must run free for its
/// direction to count as open.
pub const OPEN_REACH: f32 = 3.0;
/// Of the eight level directions, how many a spot under a low ceiling
/// (`LOW_CEILING`) needs open (`OPEN_REACH`): a duct's middle has two
/// (along it), its corners two, its crossings four at most, a step inside
/// its mouth four; a low room's floor all eight, along a wall five, in a
/// corner three; a low hallway narrower than about 4 m two.
pub const OPEN_DIRECTIONS: usize = 5;

/// Whether `feet` stands in open space (the module doc's open-space
/// filter): at least `MIN_HEADROOM` of headroom, and, under a ceiling
/// lower than `LOW_CEILING`, at least `OPEN_DIRECTIONS` of eight level
/// rays from the chest running `OPEN_REACH` free (a duct's or a
/// crawlspace's walls close in on most sides, a low room's don't). What
/// characters meet counts (`filter`: the player clips too).
pub fn open_at(q: &SpatialQuery, filter: &SpatialQueryFilter, feet: Vec3) -> bool {
    let from = feet + Vec3::Y * 0.1;
    let headroom = q.cast_ray(from, Dir3::Y, LOW_CEILING, true, filter).map_or(f32::INFINITY, |h| h.distance + 0.1);
    if headroom < MIN_HEADROOM {
        return false;
    }
    if headroom >= LOW_CEILING {
        return true;
    }
    let chest = feet + Vec3::Y * 1.0;
    let open = (0..8)
        .filter(|k| {
            let dir = Quat::from_rotation_y(*k as f32 * std::f32::consts::FRAC_PI_4) * Vec3::NEG_Z;
            Dir3::new(dir).ok().is_some_and(|d| q.cast_ray(chest, d, OPEN_REACH, true, filter).is_none())
        })
        .count();
    open >= OPEN_DIRECTIONS
}

/// Whether a character standing at `feet` is inside the map: with
/// visibility data, its chest is in a leaf of some cluster (not in solid
/// or the void outside the world); without, anywhere.
pub fn inside_map(vis: Option<&MapVisibility>, feet: Vec3) -> bool {
    vis.is_none_or(|v| v.nodes.is_empty() || v.cluster_at(feet + Vec3::Y * 1.0).is_some())
}

/// Whether the spots' region counts what is reached by a one-way drop (a
/// ledge's fall of 0.9-1.8 m with no way back): on (2026-11-24, the user:
/// "we can try it, revert later if not good"), the region is every area
/// walked to from the spawn points; off, only those walked to and back
/// (`region_with`). Flip it to revert; `map_fitness` and
/// `spot_checks::region_linked` follow it.
pub const DROPS_REACHABLE: bool = true;

/// The areas of `nav` a player can walk to from the spawn `points`
/// (`DROPS_REACHABLE`), or and back.
fn region(nav: &NavMesh, points: &[Vec3]) -> Vec<bool> {
    region_with(nav, points, DROPS_REACHABLE)
}

/// The areas of `nav` a player can walk to from the spawn `points`; with
/// `drops` off, and back too (all that can be walked to, if none can walk
/// back): the mesh's main connected region.
pub fn region_with(nav: &NavMesh, points: &[Vec3], drops: bool) -> Vec<bool> {
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
    if !drops && both.iter().any(|x| *x) { both } else { f }
}
