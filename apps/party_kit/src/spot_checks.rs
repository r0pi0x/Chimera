//! Whether the gun game's spawn spots (`chimera_sim::minigame::spawns`,
//! `random_hidden`) are where players play, on the loaded map: the checks
//! `map_fitness` reports (docs/plans/active/map-pool-fitness.md) and the
//! heavy guard runs over the pool (`tests/it/heavy/spawn_spots.rs`).
//!
//! - **Off the map's navigation file**: a spot over none of the file's
//!   areas (1.5 m of slack around each, within 2 m of its floor), on a
//!   map that ships one (Source's `maps/<name>.nav`, `map::MapFloors`):
//!   the floors its author marked as played.
//! - **Stuck**: a standing player's box (Source's 32 by 72 units, the
//!   widest the games make, from a stair step over the feet to the head,
//!   `spawns::standing_box`) meets what characters collide with: the
//!   world, props, the map's player clips (`nav_gen::PlayerClips`).
//! - **Off the main region**: a spot over none of the areas walked to from
//!   a spawn point and back (0.75 m of slack, within 1.5 m of the floor)
//!   of the map's navigation file where it ships one (its links: walks,
//!   jumps, ladders), else of the bots' generated mesh (Combat Arms'
//!   walker, jump-ups included, bounded to the clusters the spots' walking
//!   player, `spawns::walker`, reaches where the map has visibility data).
//!
//! Known shortcuts: without a navigation file the region is the bots'
//! walk, not a player's (no ladders, no crouching): a spot on a
//! ladder-only floor counts as off it.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::core::{CHARACTER_FILTER, SpawnPoint};
use chimera_sim::minigame::spawns;
use chimera_world::map::{
    MapFloors, SPAWN_LIFT,
    nav::NavMesh,
    nav_gen::{self, PlayerClips},
    vis::ActiveVisibility,
};

/// The loaded map's spawn points' feet, sorted.
pub fn spawn_feet(world: &mut World) -> Vec<Vec3> {
    let mut feet: Vec<Vec3> = world
        .query_filtered::<&Transform, With<SpawnPoint>>()
        .iter(world)
        .map(|t| t.translation - Vec3::Y * SPAWN_LIFT)
        .collect();
    feet.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z)));
    feet
}

/// The areas of `nav` walked to from the `feet` and back, and the area each
/// foot stands in (or is nearest).
pub fn region(nav: &NavMesh, feet: &[Vec3]) -> (Vec<bool>, Vec<Option<usize>>) {
    let n = nav.areas.len();
    let mut forward = vec![Vec::new(); n];
    let mut back = vec![Vec::new(); n];
    for (i, a) in nav.areas.iter().enumerate() {
        for (j, _) in &a.links {
            forward[i].push(*j);
            back[*j].push(i);
        }
    }
    let starts: Vec<Option<usize>> = feet
        .iter()
        .map(|p| nav.area_at(*p + Vec3::Y * 0.1).or_else(|| nav.nearest_area(*p)))
        .collect();
    let seeds: Vec<usize> = starts.iter().flatten().copied().collect();
    let reach = |edges: &[Vec<usize>]| {
        let mut seen = vec![false; n];
        let mut stack = seeds.clone();
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
    (f.iter().zip(&b).map(|(a, b)| *a && *b).collect(), starts)
}

/// The bots' generated mesh of the loaded map (Combat Arms' walker, its
/// box sliding to fit a doorway, bounded where the map has visibility
/// data), from the `feet`, meeting the player clips (the played space:
/// the bots' own walk passes them).
pub fn bots_mesh(world: &mut World, feet: &[Vec3]) -> NavMesh {
    // Fitting doorways as the spots' walker does (`NavGenParams::slide`,
    // half its radius): else a gap the spots' walk takes and the grid's
    // centres miss leaves spots off a region they are on (gg_lego_mafia_arena).
    let mut p = combat_arms::game::bots::brain::nav_params();
    p.slide = p.radius / 2.0;
    let clips = PlayerClips::of(world);
    let vis = world.get_resource::<ActiveVisibility>().map(|v| v.0.clone());
    let mut state = SystemState::<SpatialQuery>::new(world);
    let q = state.get(world).expect("the spatial query");
    let bounds = vis.as_deref().map(|vis| nav_gen::Bounds {
        vis,
        walker: spawns::walker(),
    });
    nav_gen::generate_within(&nav_gen::Clipped { q: &q, clips: &clips }, &|_| false, feet, &p, bounds)
}

/// Whether `q` is over an area of `nav` that `inside` keeps (0.75 m of
/// slack, within 1.5 m of its floor).
pub fn on_region(nav: &NavMesh, inside: &[bool], q: Vec3) -> bool {
    nav.areas.iter().enumerate().any(|(i, a)| {
        let (lo, hi) = (a.min - Vec2::splat(0.75), a.max + Vec2::splat(0.75));
        inside[i] && q.x >= lo.x && q.x <= hi.x && q.z >= lo.y && q.z <= hi.y && (a.height_at(q.x, q.z) - q.y).abs() < 1.5
    })
}

/// Whether `q` is off the navigation file `nav` (1.5 m of slack around
/// each area, within 2 m of its floor).
pub fn off_file(nav: &NavMesh, q: Vec3) -> bool {
    !nav.areas.iter().any(|a| {
        let (lo, hi) = (a.min - Vec2::splat(1.5), a.max + Vec2::splat(1.5));
        q.x >= lo.x && q.x <= hi.x && q.z >= lo.y && q.z <= hi.y && (a.height_at(q.x, q.z) - q.y).abs() < 2.0
    })
}

/// The map's navigation file, where it ships one.
pub fn file_nav(world: &World) -> Option<std::sync::Arc<NavMesh>> {
    world.get_resource::<MapFloors>().map(|f| f.0.clone()).filter(|n| !n.areas.is_empty())
}

/// What a standing player's box at `feet` meets (`spots::standing_box`):
/// the colliders, and `Entity::PLACEHOLDER` for a player clip.
pub fn box_meets(q: &SpatialQuery, clips: &PlayerClips, feet: Vec3) -> Vec<Entity> {
    let (half, up) = spawns::standing_box();
    let centre = feet + Vec3::Y * up;
    let mut got = Vec::new();
    if clips.overlaps_box(centre, half) {
        got.push(Entity::PLACEHOLDER);
    }
    let filter = SpatialQueryFilter::from_mask(CHARACTER_FILTER);
    q.shape_intersections_callback(
        &Collider::cuboid(half.x * 2.0, half.y * 2.0, half.z * 2.0),
        centre,
        Quat::IDENTITY,
        &filter,
        |e| {
            got.push(e);
            true
        },
    );
    got
}

/// The spots of `spots` a standing player is stuck in, with what each meets.
pub fn stuck(world: &mut World, spots: &[Vec3]) -> Vec<(Vec3, Vec<Entity>)> {
    let clips = PlayerClips::of(world);
    let mut state = SystemState::<SpatialQuery>::new(world);
    let q = state.get(world).expect("the spatial query");
    spots
        .iter()
        .filter_map(|f| Some((*f, box_meets(&q, &clips, *f))).filter(|(_, h)| !h.is_empty()))
        .collect()
}

/// The checks over the loaded map's spots.
#[derive(Clone, Debug, Default)]
pub struct SpotReport {
    /// What `random_hidden` picks from (`spawns::candidates`; none: the
    /// spawn points).
    pub spots: Vec<Vec3>,
    /// Whether they are the spawn points (no spots).
    pub points_only: bool,
    pub off_region: Vec<Vec3>,
    pub off_file: Vec<Vec3>,
    pub stuck: Vec<(Vec3, Vec<Entity>)>,
    pub has_file: bool,
}

/// The mesh whose main region the spots must be on: the map's navigation
/// file, else `bots` (the bots' mesh, `bots_mesh`).
pub fn region_mesh<'a>(file: Option<&'a NavMesh>, bots: &'a NavMesh) -> &'a NavMesh {
    file.unwrap_or(bots)
}

/// `SpotReport` of the loaded map (its spots made now) from its spawn
/// points `feet`, the bots' mesh given (`bots_mesh`).
pub fn report(world: &mut World, feet: &[Vec3], bots: &NavMesh) -> SpotReport {
    let candidates = spawns::candidates(world);
    let points_only = candidates.is_empty();
    let spots = if points_only { feet.to_vec() } else { candidates };
    let file = file_nav(world);
    let mesh = region_mesh(file.as_deref(), bots);
    let (inside, _) = region(mesh, feet);
    SpotReport {
        off_region: spots.iter().copied().filter(|q| !on_region(mesh, &inside, *q)).collect(),
        off_file: file.as_ref().map_or_else(Vec::new, |n| spots.iter().copied().filter(|q| off_file(n, *q)).collect()),
        stuck: stuck(world, &spots),
        has_file: file.is_some(),
        points_only,
        spots,
    }
}

/// `points` each dropped to the floor under it (within 6 m; else where it
/// is): where a spawn there lands (spawn points float over their floors:
/// de_piranesi's up to about 1.7 m, Combat Arms' hellgate's 3.2 m).
pub fn dropped(world: &mut World, points: &[Vec3]) -> Vec<Vec3> {
    let clips = PlayerClips::of(world);
    let mut state = SystemState::<SpatialQuery>::new(world);
    let q = state.get(world).expect("the spatial query");
    let filter = SpatialQueryFilter::from_mask(CHARACTER_FILTER);
    points
        .iter()
        .map(|p| {
            let from = *p + Vec3::Y * 0.1;
            let world = q.cast_ray(from, Dir3::NEG_Y, 6.0, true, &filter).map(|h| h.distance);
            let clip = clips.sweep(Vec3::ZERO, from, Dir3::NEG_Y, world.unwrap_or(6.0)).map(|h| h.0);
            clip.or(world).map_or(*p, |d| from - Vec3::Y * d)
        })
        .collect()
}
