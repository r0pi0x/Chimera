//! Where a match is played on its map when that is only a part of it
//! (docs/plans/active/arena-slices.md, sections 3 and 7): an arena slice
//! (`ArenaSlice`: a map, an id and display name, a polygon in plan view
//! and a seed point on a floor inside it) and the loaded map's play area
//! made from one (`PlayArea`, map-scoped). Game-neutral: any game may
//! state slices of its maps (the data: `chimera_sim::minigame::arena`).
//!
//! - The polygon (`PlanPolygon`): engine x and z (m), 3 to `MAX_POINTS`
//!   corners, simple (no two edges cross), made counter-clockwise; a
//!   point is inside when its x, z is (`contains`); the nearest point
//!   inside a margin from the edges (`nearest_inside`). Plain data, `Copy`, so
//!   a generated mesh's parameters keep it (`nav_gen::NavGenParams::plan`).
//! - The plan (`ArenaPlan`, not map-scoped): the slice the world plays on
//!   the map it names. The server's is set by the app (the party kit's
//!   `arena` command), a client's from the match on the network
//!   (`chimera_net::minigame_net`, `NetMinigame::arena`). A map's spawn
//!   (`map_spawned`, queued by `spawn`'s resources) notes the map now in
//!   the world (`ArenaMap`, its `LoadedMapName`); `follow_plan` (each
//!   frame and at the spawn) makes the play area when the plan names that
//!   map and takes it away when it no longer does.
//! - Made (`apply`): `PlayArea`; the wall (`ArenaWall`): one static
//!   cuboid collider per edge, `WALL_THICKNESS` thick, `WALL_REACH` above
//!   and below the seed, on the default layer: ordinary solid static
//!   collision every game's movement, traces, projectiles and grenades
//!   meet as they meet a level's wall (the owner's decision 5; not
//!   `MapBrushCollider`, so Source's movement meets it through its
//!   physics casts, not its brush sweep); the map's teleporters with an
//!   end outside dropped (`MapTeleporters`); the map's spawn points
//!   outside are no longer spawn points (`hold_spawn_points`: their
//!   `SpawnPoint` taken off as they appear) and one more stands at the
//!   seed (`ArenaSeedSpawn`), so every flood seeded from the spawn points
//!   starts inside.
//! - A character outside (`outside`: its x, z more than `PUT_BACK_SLACK`
//!   past the polygon) is put back at the nearest spawn point by the map's
//!   tick system (`map::fall_out_of_map`, the authority's), no death
//!   counted, as a character that can't die is put back from below the
//!   map; `PlayArea::put_backs` counts them.
//! - What a generated mesh keeps (`nav_gen::hold`): the box and the
//!   polygon (`held_plan`).
//!
//! Known shortcuts: no lid (the plan: nothing jumps 1000 m); the wall's
//! ends overlap at each corner by half its thickness (a closed outline);
//! a play area changed on a loaded map makes the wall and the spawn
//! points anew but not the spots or the bots' meshes already made (the
//! party kit's command reloads the map instead); the look is
//! `chimera_present`'s (`render::arena`).

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::core::{Intent, SpawnPoint};

use super::{LoadedMapName, MapPart, SPAWN_LIFT, teleport::MapTeleporters};

/// The most corners a slice's polygon has.
pub const MAX_POINTS: usize = 64;
/// The wall's thickness (m), centred on the polygon's edges.
pub const WALL_THICKNESS: f32 = 0.5;
/// How far the wall reaches above and below the slice's seed (m).
pub const WALL_REACH: f32 = 1000.0;
/// How far past an edge (m, plan view) a character is before it is put
/// back.
pub const PUT_BACK_SLACK: f32 = 0.5;

/// A simple polygon in plan view (engine x and z, m), counter-clockwise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanPolygon {
    n: u8,
    pts: [Vec2; MAX_POINTS],
}

impl PlanPolygon {
    /// `points` (x, z) as a polygon: 3 to `MAX_POINTS` corners, finite, no
    /// two edges crossing, an area; turned counter-clockwise.
    pub fn new(points: &[Vec2]) -> Result<Self, String> {
        let n = points.len();
        if !(3..=MAX_POINTS).contains(&n) {
            return Err(format!("a polygon has 3 to {MAX_POINTS} corners, not {n}"));
        }
        if points.iter().any(|p| !p.is_finite()) {
            return Err("a corner is not a number".into());
        }
        let mut pts = [Vec2::ZERO; MAX_POINTS];
        pts[..n].copy_from_slice(points);
        let mut poly = Self { n: n as u8, pts };
        let a = poly.signed_area();
        if a.abs() < 1e-3 {
            return Err("the polygon has no area".into());
        }
        if a < 0.0 {
            poly.pts[..n].reverse();
        }
        for i in 0..n {
            for j in i + 1..n {
                // Neighbouring edges share a corner.
                if j == i + 1 || (i == 0 && j == n - 1) {
                    continue;
                }
                let (a, b) = poly.edge(i);
                let (c, d) = poly.edge(j);
                if segments_cross(a, b, c, d) {
                    return Err(format!("edges {i} and {j} cross"));
                }
            }
        }
        Ok(poly)
    }

    /// The corners, counter-clockwise.
    pub fn points(&self) -> &[Vec2] {
        &self.pts[..self.n as usize]
    }

    /// Edge `i`: from corner `i` to the next.
    pub fn edge(&self, i: usize) -> (Vec2, Vec2) {
        let n = self.n as usize;
        (self.pts[i % n], self.pts[(i + 1) % n])
    }

    /// The edges in order.
    pub fn edges(&self) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
        (0..self.n as usize).map(|i| self.edge(i))
    }

    fn signed_area(&self) -> f32 {
        self.edges().map(|(a, b)| a.perp_dot(b)).sum::<f32>() / 2.0
    }

    /// The area (m2).
    pub fn area(&self) -> f32 {
        self.signed_area().abs()
    }

    /// The perimeter (m).
    pub fn perimeter(&self) -> f32 {
        self.edges().map(|(a, b)| a.distance(b)).sum()
    }

    /// The least and greatest x, z.
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let p = self.points();
        (p.iter().fold(Vec2::MAX, |m, q| m.min(*q)), p.iter().fold(Vec2::MIN, |m, q| m.max(*q)))
    }

    /// Whether `p` (x, z) is inside (even-odd; on an edge: either).
    pub fn contains(&self, p: Vec2) -> bool {
        let mut inside = false;
        for (a, b) in self.edges() {
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                inside = !inside;
            }
        }
        inside
    }

    /// Whether `p`'s x, z is inside.
    pub fn contains3(&self, p: Vec3) -> bool {
        self.contains(Vec2::new(p.x, p.z))
    }

    /// The distance (m) from `p` (x, z) to the nearest edge.
    pub fn edge_distance(&self, p: Vec2) -> f32 {
        self.edges()
            .map(|(a, b)| {
                let ab = b - a;
                let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
                p.distance(a + ab * t)
            })
            .fold(f32::MAX, f32::min)
    }

    /// How far (m) `p` (x, z) is outside; 0 inside.
    pub fn outside_by(&self, p: Vec2) -> f32 {
        if self.contains(p) { 0.0 } else { self.edge_distance(p) }
    }

    /// The nearest point of the edges to `p` (x, z).
    pub fn nearest_edge_point(&self, p: Vec2) -> Vec2 {
        let mut best = (f32::MAX, p);
        for (a, b) in self.edges() {
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
            let q = a + ab * t;
            let d = p.distance_squared(q);
            if d < best.0 {
                best = (d, q);
            }
        }
        best.1
    }

    /// `p` (x, z) when inside, else the nearest point inside with `margin`
    /// m to the edges where the polygon is that wide there (less where it
    /// is narrower: the margin halves until the point is inside; at worst
    /// the nearest edge point).
    pub fn nearest_inside(&self, p: Vec2, margin: f32) -> Vec2 {
        if self.contains(p) {
            return p;
        }
        let q = self.nearest_edge_point(p);
        // Inward: from `p` to the edge (p is outside), else the edge's own
        // inward normal (a counter-clockwise polygon's is the left side).
        let inward = (q - p).try_normalize().unwrap_or_else(|| {
            let (a, b) = self.edges().min_by(|x, y| seg_dist(p, *x).total_cmp(&seg_dist(p, *y))).unwrap_or((q, q));
            (b - a).perp().try_normalize().unwrap_or(Vec2::X)
        });
        let mut m = margin.max(0.0);
        for _ in 0..8 {
            // Past a corner the margin is reached further along.
            for k in [1.0, 1.5, 2.0, 3.0] {
                let r = q + inward * m * k;
                if self.contains(r) && self.edge_distance(r) >= m * 0.99 {
                    return r;
                }
            }
            m *= 0.5;
        }
        q + inward * 0.01
    }
}

fn seg_dist(p: Vec2, (a, b): (Vec2, Vec2)) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Whether segments a-b and c-d cross or touch (not counting collinear
/// overlap at a shared end: the caller skips neighbours).
fn segments_cross(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let side = |p: Vec2, q: Vec2, r: Vec2| (q - p).perp_dot(r - p);
    let (d1, d2) = (side(c, d, a), side(c, d, b));
    let (d3, d4) = (side(a, b, c), side(a, b, d));
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0) && d1 != 0.0 && d2 != 0.0 && d3 != 0.0 && d4 != 0.0
}

/// One arena slice: a part of a map played as a whole one (module doc).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ArenaSlice {
    /// The map's id as the app names it (`halo_ce:bloodgulch`).
    pub map: String,
    /// The slice's id, unique per map (`red_hill`).
    pub id: String,
    /// The name shown to players ("Blood Gulch: Red Hill").
    pub name: String,
    /// The polygon's corners (engine x, z).
    pub points: Vec<Vec2>,
    /// A point on a floor inside (engine space; the floods' seed).
    pub seed: Vec3,
}

impl ArenaSlice {
    /// `map@id`, the slice's name in the pool and on the command line.
    pub fn key(&self) -> String {
        format!("{}@{}", self.map, self.id)
    }

    /// Its polygon, checked (`PlanPolygon::new`), with the seed inside.
    pub fn polygon(&self) -> Result<PlanPolygon, String> {
        let p = PlanPolygon::new(&self.points).map_err(|e| format!("{}: {e}", self.key()))?;
        if !p.contains3(self.seed) {
            return Err(format!("{}: the seed {} is outside the polygon", self.key(), self.seed));
        }
        Ok(p)
    }
}

/// The slice the world plays on the map it names (module doc); None: the
/// whole map.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ArenaPlan(pub Option<ArenaSlice>);

/// The map now spawned in the world (its `LoadedMapName` at the spawn,
/// `map_spawned`); map-scoped.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct ArenaMap(pub String);

/// The loaded map's play area (module doc); map-scoped.
#[derive(Resource, Clone, Debug)]
pub struct PlayArea {
    pub slice: ArenaSlice,
    pub polygon: PlanPolygon,
    /// Characters put back inside so far.
    pub put_backs: u32,
}

impl PlayArea {
    /// Whether a character at `at` is outside by more than the slack.
    pub fn outside(&self, at: Vec3) -> bool {
        self.polygon.outside_by(Vec2::new(at.x, at.z)) > PUT_BACK_SLACK
    }
}

/// One edge's piece of the wall.
#[derive(Component, Clone, Copy, Debug)]
pub struct ArenaWall {
    /// The edge's ends (x, z).
    pub from: Vec2,
    pub to: Vec2,
}

/// The spawn point the play area adds at its seed.
#[derive(Component, Clone, Copy, Debug)]
pub struct ArenaSeedSpawn;

/// A map spawn point already checked against the play area.
#[derive(Component, Clone, Copy, Debug)]
pub struct HeldSpawn;

/// The polygon a generated mesh is held to on the loaded map, if it plays
/// a slice (`nav_gen::hold`).
pub fn held_plan(world: &World) -> Option<PlanPolygon> {
    world.get_resource::<PlayArea>().map(|a| a.polygon)
}

/// A map's spawn (queued with its resources): the map now in the world,
/// and its play area if the plan names it.
pub fn map_spawned(world: &mut World) {
    let name = world.get_resource::<LoadedMapName>().map(|n| n.0.clone()).unwrap_or_default();
    world.insert_resource(ArenaMap(name));
    follow_plan(world);
}

/// The play area as the plan says for the map in the world (module doc).
pub fn follow_plan(world: &mut World) {
    let Some(map) = world.get_resource::<ArenaMap>().map(|m| m.0.clone()) else { return };
    let want = world
        .get_resource::<ArenaPlan>()
        .and_then(|p| p.0.clone())
        .filter(|s| s.map.eq_ignore_ascii_case(&map));
    let have = world.get_resource::<PlayArea>().map(|a| a.slice.clone());
    if want == have {
        return;
    }
    clear(world);
    if let Some(slice) = want
        && let Err(e) = apply(world, slice)
    {
        warn!("arena: {e}");
    }
}

/// Takes the play area away: its resource and wall; the seed's spawn point.
pub fn clear(world: &mut World) {
    world.remove_resource::<PlayArea>();
    let parts: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<ArenaWall>, With<ArenaSeedSpawn>)>>()
        .iter(world)
        .collect();
    for e in parts {
        world.despawn(e);
    }
    let held: Vec<Entity> = world.query_filtered::<Entity, With<HeldSpawn>>().iter(world).collect();
    for e in held {
        world.entity_mut(e).remove::<HeldSpawn>();
    }
}

/// Plays `slice` on the loaded map (module doc): the play area, its wall,
/// the teleporters and spawn points held inside.
pub fn apply(world: &mut World, slice: ArenaSlice) -> Result<(), String> {
    let polygon = slice.polygon()?;
    for (from, to) in polygon.edges() {
        let mid = (from + to) / 2.0;
        let along = to - from;
        let len = along.length() + WALL_THICKNESS;
        // The box's x along the edge.
        let yaw = -along.y.atan2(along.x);
        world.spawn((
            Name::new("Arena wall"),
            ArenaWall { from, to },
            MapPart,
            RigidBody::Static,
            Collider::cuboid(len, 2.0 * WALL_REACH, WALL_THICKNESS),
            Transform::from_translation(Vec3::new(mid.x, slice.seed.y, mid.y)).with_rotation(Quat::from_rotation_y(yaw)),
        ));
    }
    world.spawn((
        Name::new("Arena seed spawn"),
        ArenaSeedSpawn,
        HeldSpawn,
        MapPart,
        SpawnPoint { team: None },
        Transform::from_translation(slice.seed + Vec3::Y * SPAWN_LIFT),
    ));
    if let Some(mut t) = world.get_resource_mut::<MapTeleporters>() {
        let before = t.pads.len();
        t.pads.retain(|p| polygon.contains3(p.source) && polygon.contains3(p.dest));
        info!("arena {}: {} of {before} teleporters kept", slice.key(), t.pads.len());
    }
    info!(
        "arena {}: {} m2 inside {} corners, a {:.0} m wall",
        slice.key(),
        polygon.area().round(),
        polygon.points().len(),
        polygon.perimeter()
    );
    world.insert_resource(PlayArea {
        slice,
        polygon,
        put_backs: 0,
    });
    hold_now(world);
    Ok(())
}

/// The map's spawn points outside the play area stop being spawn points
/// (each checked once, as it appears).
pub fn hold_spawn_points(
    area: Option<Res<PlayArea>>,
    points: Query<(Entity, &Transform), (With<SpawnPoint>, Without<Intent>, Without<HeldSpawn>)>,
    mut commands: Commands,
) {
    let Some(area) = area else { return };
    for (e, t) in &points {
        if area.polygon.contains3(t.translation) {
            commands.entity(e).insert(HeldSpawn);
        } else {
            commands.entity(e).insert(HeldSpawn).remove::<SpawnPoint>();
        }
    }
}

fn hold_now(world: &mut World) {
    let Some(poly) = held_plan(world) else { return };
    let points: Vec<(Entity, Vec3)> = world
        .query_filtered::<(Entity, &Transform), (With<SpawnPoint>, Without<Intent>, Without<HeldSpawn>)>()
        .iter(world)
        .map(|(e, t)| (e, t.translation))
        .collect();
    let mut out = 0;
    for (e, at) in points {
        let mut ent = world.entity_mut(e);
        ent.insert(HeldSpawn);
        if !poly.contains3(at) {
            ent.remove::<SpawnPoint>();
            out += 1;
        }
    }
    if out > 0 {
        info!("arena: {out} of the map's spawn points are outside");
    }
}

/// The plan followed when it changes (a client's arrives after its map;
/// a map's spawn follows it itself, `map_spawned`).
pub fn follow_plan_system(mut commands: Commands) {
    commands.queue(follow_plan);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> PlanPolygon {
        PlanPolygon::new(&[Vec2::new(0.0, 0.0), Vec2::new(0.0, 10.0), Vec2::new(10.0, 10.0), Vec2::new(10.0, 0.0)]).unwrap()
    }

    #[test]
    fn a_polygon_is_counter_clockwise_and_knows_inside() {
        let p = square();
        assert!(p.signed_area() > 0.0, "turned counter-clockwise");
        assert_eq!(p.area(), 100.0);
        assert!(p.contains(Vec2::new(5.0, 5.0)));
        assert!(!p.contains(Vec2::new(11.0, 5.0)));
        assert_eq!(p.outside_by(Vec2::new(5.0, 5.0)), 0.0);
        assert!((p.outside_by(Vec2::new(12.0, 5.0)) - 2.0).abs() < 1e-5);
        assert!((p.edge_distance(Vec2::new(5.0, 9.0)) - 1.0).abs() < 1e-5);
        // Back inside, a metre from the edge it was nearest.
        assert_eq!(p.nearest_inside(Vec2::new(5.0, 5.0), 1.0), Vec2::new(5.0, 5.0));
        let r = p.nearest_inside(Vec2::new(14.0, 5.0), 1.0);
        assert!((r - Vec2::new(9.0, 5.0)).length() < 1e-4, "{r}");
        let r = p.nearest_inside(Vec2::new(-3.0, -3.0), 1.0);
        assert!(p.contains(r) && p.edge_distance(r) >= 0.99, "past a corner: {r}");
    }

    #[test]
    fn bad_polygons_are_refused() {
        assert!(PlanPolygon::new(&[Vec2::ZERO, Vec2::X]).is_err(), "two corners");
        assert!(PlanPolygon::new(&[Vec2::ZERO, Vec2::X, Vec2::X * 2.0]).is_err(), "no area");
        // A bow tie: edges 0 and 2 cross.
        let bow = [Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(10.0, 0.0), Vec2::new(0.0, 10.0)];
        assert!(PlanPolygon::new(&bow).is_err());
        assert!(PlanPolygon::new(&vec![Vec2::ZERO; MAX_POINTS + 1]).is_err());
    }

    #[test]
    fn a_slice_wants_its_seed_inside() {
        let mut s = ArenaSlice {
            map: "g:m".into(),
            id: "a".into(),
            name: "M: A".into(),
            points: square().points().to_vec(),
            seed: Vec3::new(5.0, 1.0, 5.0),
        };
        assert!(s.polygon().is_ok());
        assert_eq!(s.key(), "g:m@a");
        s.seed.x = 20.0;
        assert!(s.polygon().is_err());
    }
}
