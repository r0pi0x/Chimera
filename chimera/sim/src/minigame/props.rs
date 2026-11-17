//! Practice props for a sandbox minigame (Ultimate Gun Game's `sandbox`),
//! beside its practice targets (`targets`, which re-exports everything
//! here: the screens name them `targets::Prop` and so on): six bottles on
//! a shelf, five tin cans on a shelf, or a target board, made of the
//! engine's own code-built shapes (`map::shapes::CodeShape`: no game
//! asset, drawn on every map, the greybox too).
//!
//! **Requests** (messages, applied in `Update` as queued world work; the
//! console calls the same functions): `SpawnProp { spec: PropSpec { kind,
//! distance_m }, by }`, `RemoveProp(entity)`, `ClearProps` (separate from
//! `ClearTargets`), `ResetProp(entity)` (every piece standing again and a
//! board's group cleared). A refusal is a `targets::TargetRefused`.
//! Placed as a target is: `distance_m` along the spawner's flat view on the
//! floor found there (`targets::find_spot`: refused with no floor, too
//! steep a floor or a spot inside a wall), turned to face the spawner.
//! **On the range** (`chimera_world::range`'s `RangeLayout` in): bottles
//! and cans stand on the range's own shelf (`RangeLayout::prop_spots`: a
//! row takes as many neighbouring spots as it spans, bottles 3 of 6, cans
//! 2; the next set the next free run) with no shelf of their own, and a
//! board is the range's own board (`RangeLayout::board`: our face's
//! collider just in front of its face, undrawn, the range's rings showing,
//! the marks ours; no posts); they carry `OnRangeFurniture`, their
//! `spec.distance_m` is how far down range that furniture stands (not the
//! distance asked) and a board's `home` is its frame (`BOARD_CENTRE_Y`
//! below the face's middle, not on the floor). When the furniture is
//! taken (a second board, a third row), the prop gets its own shelf or
//! board on the asked distance line at the spawner's side, as a target
//! does there (`range_place`).
//!
//! **What the screens read**: `Prop` (its spec, home, the console's
//! number, its pieces and how many stand) on the prop's entity, a board's
//! `ShotGroup` beside it, and the fact `PropHit` (attacker, prop, piece
//! from 0, the attacker's distance to the prop, simulation time), also in
//! the log.
//!
//! **Solid.** Every part has a static collider of its shape: the shelf,
//! its legs, the board's face and posts on the map's default layer (shots
//! stop on them, characters can't walk through them, as a crate), the
//! bottles and cans on `core::TRACE_ONLY_LAYER` (shots stop on them,
//! characters pass). Weapons' own traces and projectiles meet them like any
//! world surface, so an impact shows on the prop and a projectile that
//! explodes on contact explodes there. The fixed parts name their surface
//! (`core::PropSurface`: wood for the shelf, its legs, the board and its
//! posts), which the games' impact effects read (Combat Arms' drew no
//! hole and no puff on a surface it couldn't name: none showed on the
//! props before); the bottles and cans name none (a hole would stay in
//! the air when the piece is knocked off). The
//! pieces and the face are `core::Damageable` and `map::shapes::Hittable`
//! (Halo's projectiles report them as objects struck).
//!
//! - Bottles (6) and tin cans (5) stand on a shelf about 1 m high. What
//!   strikes a standing piece knocks it: a hitscan bullet whose trace
//!   (`WeaponEventKind::Shot`, `ShotContinued`) stopped on it, or any
//!   game's hit on it as an object (a non-blast `core::Damage` on it: a
//!   projectile's direct hit, a grenade breaking through); a blast
//!   (`weapon::grenade::BlastDealt`, any game's that tells it) knocks every
//!   standing piece within its `outer` radius that it sees: the line from
//!   its centre to the piece clear of the map's world (characters and
//!   props don't block it; the engine's and Halo's sight rule for the
//!   characters they hurt, Combat Arms' meets props too). A knocked piece
//!   leaves the shelf (hidden, struck no more) and its debris flies off
//!   away from what struck it as physics bodies (`PropDebris`, `debris`:
//!   a can whole and tumbling, a bottle broken into `SHARDS` shards of
//!   glass), falling and rolling on the world, gone after `DEBRIS_SECS`;
//!   `standing` falls and a `PropHit` is written (playtest 2026-11-17:
//!   pieces were laid on the floor at once, no physics, nothing broke). When every piece is down they all stand again `RESET_SECS`
//!   later; `ResetProp`, `prop reset` and a new prop stand them at once.
//! - The board: a 1 m square face (on the range, the range's board whole: `BoardFace`, `RangeLayout::board_size`), its centre `BOARD_CENTRE_Y` m above
//!   the floor, rings painted on, on two posts. Each bullet whose trace
//!   stopped on the face (each pellet), each other hit on it as an object
//!   (a projectile's), and a blast whose centre is on the face (a rocket
//!   on it) lands on it: the point in `ShotGroup::hits` (m from the
//!   face's centre, +x right and +y up as the shooter sees it), a mark
//!   drawn there and a `PropHit` (piece 0). A hitscan shot fired at the
//!   board (its aim line, `ShotMade`, crosses the board's plane within
//!   `BOARD_AIM_M` of the face's centre, the nearest board so) whose
//!   bullets all missed the face is a `misses`; a projectile's shot there
//!   (a `ShotMade` with no trace of its shooter that tick) is one when it
//!   hasn't landed on the face `PROJECTILE_MISS_SECS` later (each landing
//!   of that shooter's on the face answers its oldest such shot). The
//!   face's collider stands out to the painted rings' front (`FACE_HIT`),
//!   so a bullet's impact mark lies on them. `spread_m` (extreme
//!   spread: the widest two hits), `mean_radius_m` (the hits' mean
//!   distance from their centre) and `centre` (their mean) are kept
//!   current. Cleared by `ResetProp` and `prop reset`; a new board starts
//!   empty. Other blasts leave the board alone.
//!
//! A new prop shows where it arrives: a disc spreads on the floor under
//! it over `APPEAR_SECS` (`Appearing`, drawn only), then goes.
//!
//! Props exist only while the sandbox plays (`rung_command::holds`): a
//! spawn is refused otherwise, and every prop is removed when it stops or
//! the map is loaded again (`core::MapLoads`). Nothing here is replicated.
//!
//! A spawn and a single removal are the player's changes of the layout
//! (`layout` remembers what stands after each; `Prop::origin` and `dir`
//! are its spawner's spot and line); clearing is not.
//!
//! **Under the crosshair** (`spawn_at_aim`: the sandbox's Add prop button
//! and `prop add <kind>` without a distance): the line from the player's
//! eye along its whole view stops on the map (what a character bumps
//! into); the prop stands there on a floor, or on the floor below half a
//! metre back toward the player on a wall, facing the player, its
//! `spec.distance_m` the flat distance to it. No range furniture and no
//! distance line: where the player points (playtest 2026-11-17).
//!
//! Console: `prop add <bottles|tin_cans|target_board> [distance m]`,
//! `prop remove <n|all>`, `prop list`, `prop group <n>` (a board's shot
//! group), `prop reset <n|all>`, `prop look <n> [piece | shelf]` (turns
//! the player to face prop n's piece, from 0, or its shelf's top between
//! the middle pieces: staging a picture).
//!
//! Known shortcuts:
//! - A projectile's miss is told by time (`PROJECTILE_MISS_SECS`), not by
//!   the projectile's end: a slower one landing later answers the next
//!   shot; one shot's several projectiles (pellets) answer several. A
//!   decal projected onto the map (CS:S's) doesn't reach a prop: its
//!   shapes aren't the map's triangles. A grenade that only
//!   bounces off a piece (Combat Arms', which writes no hit on objects)
//!   doesn't knock it; its blast does.
//! - A knocked piece doesn't tumble: it is placed lying behind the shelf.

use bevy::prelude::*;

use super::{
    rung_command,
    targets::{TargetRefused, find_spot},
};
use avian3d::prelude::{AngularVelocity, Collider, ColliderOf, CollisionLayers, LayerMask, LinearVelocity, RigidBody, SpatialQuery, SpatialQueryFilter};

use crate::{
    console::{ConsoleAppExt, command_player},
    core::{Damage, DamageKind, Damageable, Intent, MapLoads, PropSurface, SOLID_LAYERS, SimClock, SimSet, TRACE_ONLY_LAYER},
    map::shapes::{CodeShape, Hittable, ShapeKind},
    weapon::{WeaponEvent, WeaponEventKind, grenade::BlastDealt},
};

/// The kinds of prop.
pub const KINDS: [&str; 3] = ["bottles", "tin_cans", "target_board"];
/// Every piece down: they stand again this long after (s).
pub const RESET_SECS: f64 = 3.0;
/// The board face's half size (m) and its centre's height (m).
pub const BOARD_HALF: f32 = 0.5;
pub const BOARD_CENTRE_Y: f32 = 1.3;
/// A shot whose aim line crosses the board's plane this close to the
/// face's centre was fired at it (m).
pub const BOARD_AIM_M: f32 = 1.0;
/// A projectile fired at a board that hasn't landed on it this long after
/// is a miss (s).
pub const PROJECTILE_MISS_SECS: f64 = 4.0;
/// The shelf's top (m).
const SHELF_TOP: f32 = 1.0;
/// A knocked piece's debris (a can, a bottle's shards) lies this long (s).
pub const DEBRIS_SECS: f64 = 4.0;
/// A bottle breaks into this many shards.
pub const SHARDS: u32 = 6;
/// A new prop grows from nothing to its size over this long (s, real
/// time: it shows where it was placed, not just there).
pub const APPEAR_SECS: f32 = 0.25;
/// A knocked can's speed away from the shot (m/s) and up.
const KNOCK_SPEED: f32 = 3.0;
const KNOCK_UP: f32 = 1.5;
/// Bottles and cans stand this far apart on a shelf (m).
const PIECE_STEP: f32 = 0.25;
/// The face's front, in front of the board's centre plane (m).
const FACE_FRONT: f32 = 0.015;
/// The face collider's front (m): the painted rings' front.
const FACE_HIT: f32 = 0.03;

/// What to place.
#[derive(Clone, Debug, PartialEq)]
pub struct PropSpec {
    /// `"bottles"`, `"tin_cans"` or `"target_board"`.
    pub kind: String,
    pub distance_m: f32,
}

/// Place a prop (module doc, "Requests").
#[derive(Message, Clone, Debug, PartialEq)]
pub struct SpawnProp {
    pub spec: PropSpec,
    /// Whose position and view it is placed from (None: the local player).
    pub by: Option<Entity>,
}

/// Remove one prop.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct RemoveProp(pub Entity);

/// Remove every prop.
#[derive(Message, Clone, Copy, Debug, Default, PartialEq)]
pub struct ClearProps;

/// Stand every piece of a prop again and clear a board's group.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct ResetProp(pub Entity);

/// A prop, on its entity (its pieces are its children).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Prop {
    pub spec: PropSpec,
    /// Its foot on the floor (m).
    pub home: Vec3,
    /// Where its spawner stood (its centre) and the flat unit direction
    /// it was placed along (`layout` repeats it from them).
    pub origin: Vec3,
    pub dir: Vec3,
    /// The console's number.
    pub number: u32,
    /// Bottles 6, cans 5, board 1.
    pub pieces: u32,
    pub standing: u32,
}

/// A prop's arrival mark: a disc spreading on the floor where it was
/// placed for `APPEAR_SECS`, then gone (`appear`; seconds so far).
#[derive(Component, Clone, Copy, Debug)]
pub struct Appearing(pub f32);

/// A knocked piece's debris: a physics body (a can, or a bottle's shard)
/// thrown off by what struck it, gone at `until` (`SimClock` s).
#[derive(Component, Clone, Copy, Debug)]
pub struct PropDebris {
    pub until: f64,
}

/// A hit on a prop: a piece knocked, or a bullet on the board's face.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct PropHit {
    pub attacker: Option<Entity>,
    pub prop: Entity,
    /// From 0 (the board's face is 0).
    pub piece: u32,
    /// The attacker's distance to the prop (m; 0 without one).
    pub distance_m: f32,
    /// `SimClock` s.
    pub time: f64,
}

/// A board's shot group (module doc).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct ShotGroup {
    /// On the face, m from its centre, +x right +y up.
    pub hits: Vec<Vec2>,
    pub misses: u32,
    /// Extreme spread: the widest two hits apart (m).
    pub spread_m: f32,
    pub mean_radius_m: f32,
    pub centre: Vec2,
}

impl ShotGroup {
    /// Add a hit and keep the figures current.
    pub fn add(&mut self, at: Vec2) {
        self.hits.push(at);
        let n = self.hits.len() as f32;
        self.centre = self.hits.iter().copied().sum::<Vec2>() / n;
        self.mean_radius_m = self.hits.iter().map(|h| h.distance(self.centre)).sum::<f32>() / n;
        let mut spread = 0.0f32;
        for (i, a) in self.hits.iter().enumerate() {
            for b in &self.hits[i + 1..] {
                spread = spread.max(a.distance(*b));
            }
        }
        self.spread_m = spread;
    }
}

/// A piece of a prop: its index, whether it stands, where it stands (in
/// the prop's frame) and its box for hits (half extents about that).
#[derive(Component, Clone, Copy, Debug)]
struct PropPiece {
    prop: Entity,
    index: u32,
    standing: bool,
    stand: Transform,
    half: Vec3,
    board: bool,
    /// Where a knocked piece lies (the prop's frame): the floor's height
    /// and how far behind the shelf's middle.
    floor_y: f32,
    behind: f32,
}

/// A prop on the range's own furniture (`range_place`): the shelf's spots
/// it takes (`RangeLayout::prop_spots`' indices), or the board.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct OnRangeFurniture {
    pub spots: Vec<usize>,
    pub board: bool,
}

/// A board's scored face: its half width and half height (m). The default
/// is `BOARD_HALF` square; on the range it is the range's board whole
/// (`RangeLayout::board_size`), so the painted edge counts.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct BoardFace(pub Vec2);

/// A mark where a bullet landed on a board.
#[derive(Component, Clone, Copy, Debug)]
struct ShotMark;

/// The map a prop was made on, since when every piece is down, and a
/// board's projectile shots still in flight (shooter, when they count as
/// a miss).
#[derive(Component, Clone, Debug, Default)]
struct PropState {
    map_loads: u32,
    down_since: Option<f64>,
    in_flight: Vec<(Entity, f64)>,
}

/// The console's next number.
#[derive(Resource, Default)]
struct PropCount(u32);

/// Every collider of a prop (its parts and pieces): blasts' sight lines
/// leave them out.
#[derive(Component, Clone, Copy, Debug)]
struct PropPart;

/// The collider of a shape.
fn collider(kind: ShapeKind) -> Collider {
    match kind {
        ShapeKind::Box(half) => Collider::cuboid(half.x * 2.0, half.y * 2.0, half.z * 2.0),
        ShapeKind::Cylinder { radius, half_height } => Collider::cylinder(radius, half_height * 2.0),
    }
}

fn shape(kind: ShapeKind, colour: Color, at: Transform) -> (CodeShape, Transform) {
    (CodeShape { kind, colour }, at)
}

/// A shape lying in the face's plane (its axis along +Z, toward the
/// shooter).
fn disc(radius: f32, z: f32, at: Vec2, colour: Color) -> (CodeShape, Transform) {
    shape(
        ShapeKind::Cylinder { radius, half_height: 0.002 },
        colour,
        Transform::from_xyz(at.x, at.y, z).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    )
}

/// Place a prop now (a player's change: `layout` remembers it); its entity.
pub fn spawn(world: &mut World, req: &SpawnProp) -> Result<Entity, String> {
    if !rung_command::holds(world) {
        return Err("props are the sandbox's: start it with `sandbox`".into());
    }
    let by = match req.by {
        Some(e) => e,
        None => command_player(world).map_err(|_| "no player to place it in front of".to_string())?,
    };
    let origin = world.get::<Transform>(by).ok_or("the player has no place")?.translation;
    let yaw = world.get::<Intent>(by).map_or(0.0, |i| i.yaw);
    let dir = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let e = spawn_from(world, &req.spec, origin, dir)?;
    super::layout::record(world);
    Ok(e)
}

/// How far the crosshair reaches for `spawn_at_aim` (m).
pub const AIM_REACH_M: f32 = 120.0;

/// Place a prop of `kind` where the player's crosshair meets the map (a
/// player's change: `layout` remembers it); its entity. The line from the
/// eye along the full view (yaw and pitch) stops on the first surface a
/// character would stand on or bump into; on a floor the prop stands
/// there, on a wall it stands on the floor below, half a metre back
/// toward the player. Neither the range's furniture nor a distance line:
/// where the player points. Refused when the line meets nothing within
/// `AIM_REACH_M` (the sky, the void) or the floor found can't hold it.
pub fn spawn_at_aim(world: &mut World, kind: &str, by: Option<Entity>) -> Result<Entity, String> {
    if !rung_command::holds(world) {
        return Err("props are the sandbox's: start it with `sandbox`".into());
    }
    let by = match by {
        Some(e) => e,
        None => command_player(world).map_err(|_| "no player to place it in front of".to_string())?,
    };
    let origin = world.get::<Transform>(by).ok_or("the player has no place")?.translation;
    let lift = world.get::<crate::core::MovementState>(by).map_or(Vec3::Y * 0.6, |s| s.eye_offset);
    let (yaw, pitch) = world.get::<Intent>(by).map_or((0.0, 0.0), |i| (i.yaw, i.pitch));
    let home = aim_spot(world, origin + lift, yaw, pitch)?;
    let flat = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let spec = PropSpec { kind: kind.to_string(), distance_m: (home - origin).xz().length() };
    let e = build(world, &spec, origin, flat, Some(home))?;
    super::layout::record(world);
    Ok(e)
}

/// Where a prop stands for a crosshair at `eye` looking along `yaw` and
/// `pitch` (`spawn_at_aim`).
pub fn aim_spot(world: &mut World, eye: Vec3, yaw: f32, pitch: f32) -> Result<Vec3, String> {
    let dir = Vec3::new(-yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
    let Ok(dir3) = Dir3::new(dir) else { return Err("no view to aim along".into()) };
    let characters: Vec<Entity> = world.query_filtered::<Entity, With<Intent>>().iter(world).collect();
    let mut state: bevy::ecs::system::SystemState<SpatialQuery> = bevy::ecs::system::SystemState::new(world);
    let hit = {
        let q = state.get(world).map_err(|e| e.to_string())?;
        let filter = SpatialQueryFilter::from_mask(crate::core::CHARACTER_FILTER).with_excluded_entities(characters);
        q.cast_ray(eye, dir3, AIM_REACH_M, true, &filter)
    };
    let Some(hit) = hit else {
        return Err(format!("nothing under the crosshair within {AIM_REACH_M:.0} m"));
    };
    let at = eye + dir * hit.distance;
    let from = if hit.normal.y >= 0.7 {
        // A floor: stand right there (cast from just above it).
        at + Vec3::Y * 0.3
    } else {
        // A wall or a ceiling: the floor below, a little back toward the
        // player so the prop's shelf clears the wall.
        let back = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
        at - back * 0.5 + Vec3::Y * 0.1
    };
    super::targets::stand_below(world, from, "under the crosshair")
}

/// Place a prop as if from `origin` looking along flat `dir` (`spawn`;
/// the layout's repeat), not remembered.
pub fn spawn_from(world: &mut World, spec: &PropSpec, origin: Vec3, dir: Vec3) -> Result<Entity, String> {
    build(world, spec, origin, dir, None)
}

/// Build a prop: at `aimed` (`spawn_at_aim`'s spot) when given, else as
/// `spawn_from` places it.
fn build(world: &mut World, spec: &PropSpec, origin: Vec3, dir: Vec3, aimed: Option<Vec3>) -> Result<Entity, String> {
    if !rung_command::holds(world) {
        return Err("props are the sandbox's: start it with `sandbox`".into());
    }
    let kind = KINDS
        .iter()
        .find(|k| k.eq_ignore_ascii_case(&spec.kind))
        .ok_or_else(|| format!("no prop {:?} (one of {})", spec.kind, KINDS.join(", ")))?;
    let pieces = match *kind {
        "bottles" => 6,
        "tin_cans" => 5,
        _ => 1,
    };
    // On the range: its own shelf or board first, else a spot on the asked
    // distance line at the spawner's side (`range_place`); elsewhere along
    // the spawner's view.
    let range = super::range_place::layout(world).filter(|_| aimed.is_none());
    let furniture = match &range {
        Some(l) => range_furniture(world, l, kind, pieces),
        None => None,
    };
    let (frame, origin, dir, distance_m) = match (&range, &furniture) {
        _ if aimed.is_some() => (aimed.unwrap_or_default(), origin, dir, spec.distance_m),
        (Some(l), Some((frame, _))) => {
            let a = super::range_place::along_side(l, *frame);
            (*frame, super::range_place::origin_at(l, a.y), l.dir, a.x)
        }
        (Some(l), None) => {
            let side = super::range_place::along_side(l, origin).y;
            let reach = super::range_place::Footprint::of_prop(kind);
            let s = super::range_place::line_spot(world, l, spec.distance_m, side, reach, None)?;
            (s.home, s.origin, l.dir, spec.distance_m)
        }
        (None, _) => (find_spot(world, origin, dir, spec.distance_m)?, origin, dir, spec.distance_m),
    };
    let home = frame;
    let to = origin - home;
    let facing = Quat::from_rotation_y(to.x.atan2(to.z));
    let number = {
        let mut c = world.get_resource_or_init::<PropCount>();
        c.0 += 1;
        c.0
    };
    let map_loads = world.get_resource::<MapLoads>().map_or(0, |m| m.0);
    let spec = PropSpec { kind: kind.to_string(), distance_m };
    let prop = world
        .spawn((
            Name::new(format!("Prop {number} ({kind})")),
            Transform::from_translation(home).with_rotation(facing),
            Visibility::default(),
            Prop { spec, home, origin, dir, number, pieces, standing: pieces },
            PropState { map_loads, ..default() },
        ))
        .id();
    // Where it arrives: a disc on the floor spreading out under it, a
    // quarter second (drawn only: no collider; `appear`).
    world.spawn((
        Name::new(format!("Prop {number} arriving")),
        CodeShape { kind: ShapeKind::Cylinder { radius: 0.9, half_height: 0.004 }, colour: Color::srgb(1.0, 0.85, 0.35) },
        Transform::from_translation(home + Vec3::Y * 0.01).with_scale(Vec3::new(0.05, 1.0, 0.05)),
        Visibility::default(),
        Appearing(0.0),
    ));
    let on_furniture = furniture.is_some();
    if let Some((_, f)) = furniture {
        world.entity_mut(prop).insert(f);
    }
    let wood = Color::srgb(0.45, 0.30, 0.18);
    // Every part is solid: a static collider of its shape on the map's
    // default layer (shots stop on it, characters can't walk through a
    // shelf or the board), the pieces moved to traces only below.
    // Each part names its surface (`core::PropSurface`), so every game's
    // impact effects and marks know what they hit (Combat Arms' draw
    // none on a surface they can't name).
    let part = |world: &mut World, (s, t): (CodeShape, Transform), surface: Option<&str>| {
        let c = collider(s.kind);
        let e = world.spawn((s, t, ChildOf(prop), RigidBody::Static, c, PropPart)).id();
        if let Some(surface) = surface {
            world.entity_mut(e).insert(PropSurface(surface.into()));
        }
        e
    };
    if *kind == "target_board" {
        // The scored face: the range's board whole, else the default.
        let half2 = range.as_ref().filter(|_| on_furniture).map_or(Vec2::splat(BOARD_HALF), |l| l.board_size / 2.0);
        world.entity_mut(prop).insert((ShotGroup::default(), BoardFace(half2)));
        let stand = Transform::from_xyz(0.0, BOARD_CENTRE_Y, 0.0);
        let face = if on_furniture {
            // The range's own board: its face and rings are the range's;
            // ours is only the collider in front of them, undrawn (the
            // marks still are).
            world
                .spawn((stand, Visibility::default(), ChildOf(prop), RigidBody::Static, PropPart, PropSurface("wood".into())))
                .id()
        } else {
            for x in [-0.4, 0.4] {
                part(world, shape(ShapeKind::Box(Vec3::new(0.03, 0.9, 0.03)), wood, Transform::from_xyz(x, 0.9, -0.05)), Some("wood"));
            }
            part(world, shape(ShapeKind::Box(Vec3::new(BOARD_HALF, BOARD_HALF, FACE_FRONT)), Color::srgb(0.92, 0.9, 0.82), stand), Some("wood"))
        };
        // Its collider's front is the painted rings' (`FACE_HIT`), so a
        // bullet's mark lands on them, not hidden behind them.
        world.entity_mut(face).insert((
            Collider::cuboid(half2.x * 2.0, half2.y * 2.0, FACE_HIT * 2.0),
            PropPiece {
                prop,
                index: 0,
                standing: true,
                stand,
                half: Vec3::new(half2.x, half2.y, FACE_FRONT),
                board: true,
                floor_y: 0.0,
                behind: 0.0,
            },
            Damageable,
            Hittable,
        ));
        let black = Color::srgb(0.05, 0.05, 0.05);
        let cream = Color::srgb(0.92, 0.9, 0.82);
        let rings = if on_furniture { &[][..] } else { &[(0.4, black), (0.3, cream), (0.2, black), (0.1, cream), (0.04, Color::srgb(0.8, 0.1, 0.1))][..] };
        for (i, (r, c)) in rings.iter().copied().enumerate() {
            let (s, t) = disc(r, FACE_FRONT + 0.002 * (i as f32 + 1.0), Vec2::ZERO, c);
            world.spawn((s, t, ChildOf(face)));
        }
    } else {
        // The shelf (the range's own when it stands on that: its frame then
        // puts `SHELF_TOP` on the range shelf's top).
        let shelf_top = if on_furniture { range.as_ref().map_or(SHELF_TOP, |l| l.shelf_top) } else { SHELF_TOP };
        let shelf_half_depth = range.as_ref().map_or(0.4, |l| l.shelf_size.z / 2.0);
        let (floor_y, behind) = if on_furniture { (-world.get::<Transform>(prop).map_or(0.0, |t| t.translation.y), shelf_half_depth + 0.05) } else { (0.0, 0.45) };
        if !on_furniture {
            part(world, shape(ShapeKind::Box(Vec3::new(0.8, 0.02, 0.15)), wood, Transform::from_xyz(0.0, SHELF_TOP - 0.02, 0.0)), Some("wood"));
            for x in [-0.72, 0.72] {
                part(world, shape(ShapeKind::Box(Vec3::new(0.03, 0.48, 0.12)), wood, Transform::from_xyz(x, 0.48, 0.0)), Some("wood"));
            }
        }
        let bottles = *kind == "bottles";
        let (r, hh, step) = if bottles { (0.04, 0.12, PIECE_STEP) } else { (0.033, 0.06, PIECE_STEP) };
        let colour = if bottles { Color::srgb(0.12, 0.45, 0.18) } else { Color::srgb(0.75, 0.75, 0.78) };
        for i in 0..pieces {
            let x = (i as f32 - (pieces - 1) as f32 / 2.0) * step;
            let stand = Transform::from_xyz(x, shelf_top + hh, 0.0);
            // No surface named: a game's hole there would stay in the air
            // when the piece is knocked off (Combat Arms' marks are the
            // world's), so a piece shows none.
            let piece = part(world, shape(ShapeKind::Cylinder { radius: r, half_height: hh }, colour, stand), None);
            let half = if bottles { Vec3::new(r, hh + 0.04, r) } else { Vec3::new(r, hh, r) };
            // Shots strike it (traces only: characters pass it, it can't
            // hold them up).
            world.entity_mut(piece).insert((
                PropPiece { prop, index: i, standing: true, stand, half, board: false, floor_y, behind },
                CollisionLayers::new(TRACE_ONLY_LAYER, LayerMask::NONE),
                Damageable,
                Hittable,
            ));
            let extra = if bottles {
                shape(ShapeKind::Cylinder { radius: 0.016, half_height: 0.04 }, colour, Transform::from_xyz(0.0, hh + 0.04, 0.0))
            } else {
                shape(ShapeKind::Cylinder { radius: r + 0.001, half_height: hh * 0.5 }, Color::srgb(0.75, 0.12, 0.1), Transform::IDENTITY)
            };
            world.spawn((extra.0, extra.1, ChildOf(piece)));
        }
    }
    Ok(prop)
}

/// A spot on the range's own furniture for a prop of `kind` with
/// `pieces`, if one is free: its frame's place (pieces then stand on the
/// shelf's top, a board's face on the range board's) and what it takes.
/// Bottles and cans take as many neighbouring shelf spots as their row
/// spans (the first free run); a board the range's board.
fn range_furniture(world: &mut World, l: &chimera_world::range::RangeLayout, kind: &str, pieces: u32) -> Option<(Vec3, OnRangeFurniture)> {
    let used: Vec<OnRangeFurniture> = world.query::<&OnRangeFurniture>().iter(world).cloned().collect();
    if kind == "target_board" {
        if used.iter().any(|u| u.board) {
            return None;
        }
        return Some((l.board - Vec3::Y * BOARD_CENTRE_Y, OnRangeFurniture { spots: Vec::new(), board: true }));
    }
    let spots = &l.prop_spots;
    if spots.is_empty() {
        return None;
    }
    let spacing = if spots.len() >= 2 { spots[0].distance(spots[1]) } else { f32::INFINITY };
    // The row's span: the pieces' step and a piece's width, a little room.
    let span = (pieces.saturating_sub(1)) as f32 * PIECE_STEP + 0.1 + 0.1;
    let need = ((span / spacing).ceil() as usize).clamp(1, spots.len());
    let taken: Vec<usize> = used.iter().flat_map(|u| u.spots.iter().copied()).collect();
    let first = (0..=spots.len() - need).find(|&i| (i..i + need).all(|k| !taken.contains(&k)))?;
    let centre = spots[first..first + need].iter().copied().sum::<Vec3>() / need as f32;
    Some((centre - Vec3::Y * l.shelf_top, OnRangeFurniture { spots: (first..first + need).collect(), board: false }))
}

/// Remove prop `e` (a player's change: `layout` remembers what is left).
pub fn remove(world: &mut World, e: Entity) {
    if remove_quiet(world, e) {
        super::layout::record(world);
    }
}

fn remove_quiet(world: &mut World, e: Entity) -> bool {
    if world.get::<Prop>(e).is_some()
        && let Ok(ent) = world.get_entity_mut(e)
    {
        ent.despawn();
        return true;
    }
    false
}

/// Remove every prop; how many (the remembered layout stays).
pub fn clear(world: &mut World) -> usize {
    let all: Vec<Entity> = all_props(world).into_iter().map(|(e, _)| e).collect();
    for e in &all {
        remove_quiet(world, *e);
    }
    all.len()
}

/// Every prop, by number.
pub fn all_props(world: &mut World) -> Vec<(Entity, Prop)> {
    let mut all: Vec<(Entity, Prop)> = world.query::<(Entity, &Prop)>().iter(world).map(|(e, p)| (e, p.clone())).collect();
    all.sort_by_key(|(_, p)| p.number);
    all
}

/// Stand every piece of prop `e` again and clear its group (`ResetProp`).
pub fn reset(world: &mut World, e: Entity) {
    let pieces: Vec<(Entity, Transform)> = world
        .query::<(Entity, &PropPiece)>()
        .iter(world)
        .filter(|(_, p)| p.prop == e)
        .map(|(pe, p)| (pe, p.stand))
        .collect();
    for (pe, stand) in pieces {
        if let Some(mut p) = world.get_mut::<PropPiece>(pe) {
            p.standing = true;
        }
        if let Some(mut t) = world.get_mut::<Transform>(pe) {
            *t = stand;
        }
        // Back on the shelf: shown and struck again (`debris` took it off).
        let board = world.get::<PropPiece>(pe).is_some_and(|p| p.board);
        if !board && let Ok(mut ent) = world.get_entity_mut(pe) {
            ent.insert((Visibility::Inherited, CollisionLayers::new(TRACE_ONLY_LAYER, LayerMask::NONE)));
        }
    }
    let marks: Vec<Entity> = world.query_filtered::<(Entity, &ChildOf), With<ShotMark>>().iter(world).map(|(m, _)| m).collect();
    let face: Vec<Entity> = world.query::<(Entity, &PropPiece)>().iter(world).filter(|(_, p)| p.prop == e && p.board).map(|(f, _)| f).collect();
    for m in marks {
        if world.get::<ChildOf>(m).is_some_and(|c| face.contains(&c.parent())) {
            world.entity_mut(m).despawn();
        }
    }
    if let Ok(mut ent) = world.get_entity_mut(e) {
        if let Some(mut p) = ent.get_mut::<Prop>() {
            p.standing = p.pieces;
        }
        if let Some(mut g) = ent.get_mut::<ShotGroup>() {
            *g = ShotGroup::default();
        }
        if let Some(mut s) = ent.get_mut::<PropState>() {
            s.down_since = None;
            s.in_flight.clear();
        }
    }
}

/// What prop hits read beside the props: the shooters, the colliders'
/// bodies, the world for a blast's sight line.
#[derive(bevy::ecs::system::SystemParam)]
struct PropWorld<'w, 's> {
    shooters: Query<'w, 's, (Entity, &'static Transform), (With<Intent>, Without<Prop>, Without<PropPiece>)>,
    colliders: Query<'w, 's, &'static ColliderOf>,
    parts: Query<'w, 's, Entity, With<PropPart>>,
    spatial: SpatialQuery<'w, 's>,
}

impl PropWorld<'_, '_> {
    /// The body a collider (or a body) `e` belongs to.
    fn body(&self, e: Entity) -> Entity {
        self.colliders.get(e).map_or(e, |c| c.body)
    }

    /// Whether the line from a blast at `from` to `to` is clear of the
    /// map's world: characters and props never block it (module doc).
    fn clear(&self, from: Vec3, to: Vec3) -> bool {
        let Ok((dir, len)) = Dir3::new_and_length(to - from) else { return true };
        let skip: Vec<Entity> = self.shooters.iter().map(|(e, _)| e).chain(self.parts.iter()).collect();
        let filter = SpatialQueryFilter::from_mask(SOLID_LAYERS).with_excluded_entities(skip);
        let colliders = &self.colliders;
        let shooters = &self.shooters;
        let parts = &self.parts;
        self.spatial
            .cast_ray_predicate(from, dir, len, true, &filter, &|c| {
                let body = colliders.get(c).map_or(c, |o| o.body);
                !shooters.contains(body) && !parts.contains(body)
            })
            .is_none()
    }
}

/// The shots, projectiles and blasts of a tick on the props.
#[allow(clippy::type_complexity)]
fn prop_hits(
    (mut events, mut blasts, mut damage): (MessageReader<WeaponEvent>, MessageReader<BlastDealt>, MessageReader<Damage>),
    clock: Res<SimClock>,
    mut props: Query<(Entity, &Transform, &mut Prop, &mut PropState, Option<&mut ShotGroup>)>,
    mut pieces: Query<(Entity, &mut PropPiece, &mut Transform), (Without<Prop>, Without<Intent>)>,
    world: PropWorld,
    mut out: MessageWriter<PropHit>,
    shapes: Query<&CodeShape>,
    faces: Query<&BoardFace>,
    mut commands: Commands,
) {
    let now = clock.now;
    let mut lines: Vec<(Entity, Vec3, Entity)> = Vec::new();
    let mut traced: Vec<Entity> = Vec::new();
    let mut aims: Vec<(Entity, Vec3, Vec3)> = Vec::new();
    for ev in events.read() {
        if ev.replay {
            continue;
        }
        match ev.kind {
            WeaponEventKind::Shot { to, hit, .. } | WeaponEventKind::ShotContinued { to, hit, .. } => {
                traced.push(ev.owner);
                if let Some(h) = hit {
                    lines.push((ev.owner, to, world.body(h)));
                }
            }
            WeaponEventKind::ShotMade { origin, dir, .. } => aims.push((ev.owner, origin, dir)),
            _ => {}
        }
    }
    // What struck a piece as an object: every game's hit on a `Damageable`
    // (a bullet, a projectile's direct hit; blasts are below).
    let struck: Vec<(Option<Entity>, Vec3, Entity)> = damage
        .read()
        .filter(|d| d.kind != DamageKind::Blast)
        .map(|d| (d.attacker, d.point, world.body(d.target)))
        .filter(|(.., e)| pieces.contains(*e))
        .collect();
    let blasts: Vec<BlastDealt> = blasts.read().cloned().collect();
    // Projectiles fired at a board that never landed on it: misses.
    for (.., mut state, group) in &mut props {
        if state.in_flight.iter().any(|(_, at)| *at <= now) {
            let late = state.in_flight.iter().filter(|(_, at)| *at <= now).count() as u32;
            state.in_flight.retain(|(_, at)| *at > now);
            if let Some(mut g) = group {
                g.misses += late;
            }
        }
    }
    if lines.is_empty() && aims.is_empty() && blasts.is_empty() && struck.is_empty() {
        return;
    }
    let frames: Vec<(Entity, Transform)> = props.iter().map(|(e, t, ..)| (e, *t)).collect();
    let frame = |p: Entity| frames.iter().find(|(e, _)| *e == p).map(|(_, t)| *t);
    let local = |t: &Transform, p: Vec3| t.rotation.inverse() * (p - t.translation);
    let distance = |a: Option<Entity>, p: Entity| {
        a.and_then(|a| world.shooters.get(a).ok()).zip(frame(p)).map_or(0.0, |((_, s), t)| s.translation.distance(t.translation))
    };
    // Where a point lands on a board's face (m from its centre), if on it.
    let on_face = |prop: Entity, p: Vec3| {
        let l = local(&frame(prop)?, p);
        let on = Vec2::new(l.x, l.y - BOARD_CENTRE_Y);
        let half = faces.get(prop).map_or(Vec2::splat(BOARD_HALF), |f| f.0);
        let edge = half + 0.01;
        (on.x.abs() <= edge.x && on.y.abs() <= edge.y && l.z.abs() <= 0.3).then_some(on.clamp(-half, half))
    };
    // Knocked pieces and landings: (attacker, prop, piece, landing on the face).
    type Hits = Vec<(Option<Entity>, Entity, Entity, Option<Vec2>)>;
    let mut hits: Hits = Vec::new();
    let mut landed: Vec<(Entity, Entity)> = Vec::new();
    let hit = |a: Option<Entity>, pe: Entity, at: Vec3, hits: &mut Hits, landed: &mut Vec<(Entity, Entity)>| {
        let Ok((_, piece, _)) = pieces.get(pe) else { return };
        let on = if piece.board { on_face(piece.prop, at) } else { None };
        if piece.board && on.is_none() {
            return;
        }
        hits.push((a, piece.prop, pe, on));
        if let (Some(a), Some(_)) = (a, on) {
            landed.push((a, piece.prop));
        }
    };
    // Bullets' traces that stopped on a piece: each pellet where it struck.
    for (owner, to, body) in &lines {
        if pieces.contains(*body) {
            hit(Some(*owner), *body, *to, &mut hits, &mut landed);
        }
    }
    // Other games' and projectiles' hits (a shooter whose bullets drew a
    // trace on that piece this tick is counted by the trace).
    for (a, at, pe) in &struck {
        let by_trace = a.is_some_and(|a| lines.iter().any(|(o, _, b)| *o == a && b == pe));
        if !by_trace {
            hit(*a, *pe, *at, &mut hits, &mut landed);
        }
    }
    let mut blast_from: Vec<(Entity, Vec3)> = Vec::new();
    // Blasts: every standing piece within the outer radius in the blast's
    // sight; a blast on a board's face (a rocket's) lands there.
    for b in &blasts {
        for (pe, piece, _) in &pieces {
            let Some(t) = frame(piece.prop) else { continue };
            let at = t.transform_point(piece.stand.translation);
            if piece.board {
                let already = b.attacker.is_some_and(|a| landed.contains(&(a, piece.prop)));
                if !already && local(&t, b.at).z.abs() <= 0.3 && on_face(piece.prop, b.at).is_some() {
                    hits.push((b.attacker, piece.prop, pe, on_face(piece.prop, b.at)));
                }
            } else if piece.standing && at.distance(b.at) <= b.outer && world.clear(b.at, at) {
                hits.push((b.attacker, piece.prop, pe, None));
                blast_from.push((pe, b.at));
            }
        }
    }
    // Shots at a board whose shooter's bullets all missed its face; a
    // projectile's (no trace this tick) waits for its landing.
    let mut at_board: Vec<(Entity, Entity)> = Vec::new();
    let mut flying: Vec<(Entity, Entity)> = Vec::new();
    for (owner, origin, dir) in &aims {
        let mut nearest: Option<(f32, Entity)> = None;
        for (e, t, p, ..) in &props {
            if p.spec.kind != "target_board" {
                continue;
            }
            let (a, d) = (local(t, *origin), t.rotation.inverse() * *dir);
            if d.z.abs() < 1e-6 {
                continue;
            }
            let along = (FACE_FRONT - a.z) / d.z;
            let p = a + d * along;
            if along > 0.0 && Vec2::new(p.x, p.y - BOARD_CENTRE_Y).length() <= BOARD_AIM_M && nearest.is_none_or(|(n, _)| along < n) {
                nearest = Some((along, e));
            }
        }
        if let Some((_, board)) = nearest {
            if traced.contains(owner) {
                at_board.push((*owner, board));
            } else {
                flying.push((*owner, board));
            }
        }
    }
    for (owner, board) in flying {
        if let Ok((.., mut state, _)) = props.get_mut(board) {
            state.in_flight.push((owner, now + PROJECTILE_MISS_SECS));
        }
    }
    // A projectile's landing on a board answers its oldest shot there.
    for (a, prop, _, on) in &hits {
        if let (Some(a), Some(_)) = (a, on)
            && !traced.contains(a)
            && let Ok((.., mut state, _)) = props.get_mut(*prop)
            && let Some(i) = state.in_flight.iter().position(|(o, _)| o == a)
        {
            state.in_flight.remove(i);
        }
    }
    // One miss per shot at it beyond the bullets that landed on it.
    let mut pairs = at_board.clone();
    pairs.sort();
    pairs.dedup();
    for pair in pairs {
        let aimed = at_board.iter().filter(|x| **x == pair).count();
        let missed = aimed.saturating_sub(landed.iter().filter(|x| **x == pair).count());
        if missed > 0
            && let Ok((.., Some(mut g))) = props.get_mut(pair.1)
        {
            g.misses += missed as u32;
        }
    }
    for (attacker, prop, pe, on) in hits {
        let Ok((_, mut piece, mut tf)) = pieces.get_mut(pe) else { continue };
        if !piece.standing {
            continue;
        }
        let index = piece.index;
        match on {
            Some(at) => {
                if let Ok((.., Some(mut g))) = props.get_mut(prop) {
                    g.add(at);
                }
                // A bright paster, seen on the black rings too.
                let (s, t) = disc(0.015, FACE_FRONT + 0.012, Vec2::new(at.x, at.y), Color::srgb(1.0, 0.85, 0.0));
                commands.spawn((s, t, ShotMark, ChildOf(pe)));
            }
            None => {
                // Knocked: the piece goes from the shelf (hidden, no longer
                // struck) and its debris flies off as physics bodies, away
                // from what struck it: a can whole, a bottle in shards.
                piece.standing = false;
                *tf = piece.stand;
                commands.entity(pe).insert((Visibility::Hidden, CollisionLayers::NONE));
                let frame = frames.iter().find(|(e, _)| *e == prop).map_or(Transform::IDENTITY, |(_, t)| *t);
                let at = frame.mul_transform(piece.stand);
                let from = blast_from
                    .iter()
                    .find(|(p, _)| *p == pe)
                    .map(|(_, f)| *f)
                    .or_else(|| attacker.and_then(|a| world.shooters.get(a).ok()).map(|(_, t)| t.translation));
                let away = from.map_or(frame.rotation * Vec3::NEG_Z, |f| (at.translation - f).with_y(0.0).normalize_or(frame.rotation * Vec3::NEG_Z));
                if let Some(shape) = shapes.get(pe).ok().cloned() {
                    debris(&mut commands, shape, at, away, piece.half, now, index);
                }
                if let Ok((_, _, mut p, mut s, _)) = props.get_mut(prop) {
                    p.standing = p.standing.saturating_sub(1);
                    if p.standing == 0 {
                        s.down_since = Some(now);
                    }
                }
            }
        }
        let hit = PropHit { attacker, prop, piece: index, distance_m: distance(attacker, prop), time: now };
        info!("PropHit attacker={:?} prop={prop:?} piece={index} distance={:.1}m t={now:.3}", attacker, hit.distance_m);
        out.write(hit);
    }
}

/// A knocked piece's debris at `at` (world), thrown along `away` (flat,
/// unit): a can whole, tumbling; a bottle (its `half` taller than its
/// body: the neck) in `SHARDS` shards of its glass. Physics bodies that
/// fall and roll on the world, gone after `DEBRIS_SECS`; on the debris
/// layer (`core::RAGDOLL_LAYER`, colliding with the world and props only:
/// characters and shots pass them).
fn debris(commands: &mut Commands, shape: CodeShape, at: Transform, away: Vec3, half: Vec3, now: f64, index: u32) {
    let until = now + DEBRIS_SECS;
    let layers = CollisionLayers::new(crate::core::RAGDOLL_LAYER, LayerMask::DEFAULT);
    let side = Vec3::new(-away.z, 0.0, away.x);
    let bottle = half.y > match shape.kind {
        ShapeKind::Cylinder { half_height, .. } => half_height + 0.01,
        ShapeKind::Box(h) => h.y + 0.01,
    };
    if !bottle {
        let v = away * KNOCK_SPEED + Vec3::Y * KNOCK_UP + side * (0.4 * ((index % 3) as f32 - 1.0));
        commands.spawn((
            Name::new("Prop debris"),
            shape,
            at,
            Visibility::default(),
            RigidBody::Dynamic,
            collider(shape.kind),
            layers,
            LinearVelocity(v),
            AngularVelocity(side * 12.0 + Vec3::Y * 3.0),
            PropDebris { until },
        ));
        return;
    }
    // The glass breaks: shards fanned out from where it stood.
    for k in 0..SHARDS {
        let a = k as f32 / SHARDS as f32 * std::f32::consts::TAU + index as f32;
        let out = (side * a.cos() + away * (0.6 + 0.4 * a.sin())).normalize_or(away);
        let size = Vec3::new(0.012 + 0.004 * (k % 3) as f32, 0.02 + 0.01 * (k % 2) as f32, 0.006);
        let shard = CodeShape { kind: ShapeKind::Box(size), colour: shape.colour };
        let place = Transform::from_translation(at.translation + out * 0.03 + Vec3::Y * (0.02 * k as f32 - 0.05)).with_rotation(at.rotation * Quat::from_rotation_y(a));
        commands.spawn((
            Name::new("Prop debris"),
            shard,
            place,
            Visibility::default(),
            RigidBody::Dynamic,
            collider(shard.kind),
            layers,
            LinearVelocity(out * (1.5 + 0.3 * (k % 3) as f32) + Vec3::Y * (1.0 + 0.2 * k as f32)),
            AngularVelocity(Vec3::new(9.0, 5.0 + k as f32, 7.0)),
            PropDebris { until },
        ));
    }
}

/// Debris past its time goes (and all of it with the sandbox).
fn clear_debris(clock: Res<SimClock>, debris: Query<(Entity, &PropDebris)>, sandbox: (Option<Res<rung_command::HoldRungs>>, Option<Res<super::ActiveMinigame>>), mut commands: Commands) {
    let plays = rung_command::sandbox_plays(sandbox.0, sandbox.1);
    for (e, d) in &debris {
        if !plays || clock.now >= d.until {
            commands.entity(e).try_despawn();
        }
    }
}

/// New props' arrival marks spread out and go (`Appearing`, real time,
/// eased out).
fn appear(time: Res<Time<Real>>, mut marks: Query<(Entity, &mut Appearing, &mut Transform)>, mut commands: Commands) {
    for (e, mut a, mut t) in &mut marks {
        a.0 += time.delta_secs();
        let f = (a.0 / APPEAR_SECS).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - f).powi(3);
        t.scale = Vec3::new(eased.max(0.05), 1.0, eased.max(0.05));
        if f >= 1.0 {
            commands.entity(e).try_despawn();
        }
    }
}

/// Props whose pieces are all down stand again after `RESET_SECS`; every
/// prop goes when the sandbox stops or the map loads again.
fn settle(
    clock: Res<SimClock>,
    loads: Option<Res<MapLoads>>,
    sandbox: (Option<Res<rung_command::HoldRungs>>, Option<Res<super::ActiveMinigame>>),
    props: Query<(Entity, &PropState)>,
    mut commands: Commands,
) {
    let loads = loads.map_or(0, |m| m.0);
    let plays = rung_command::sandbox_plays(sandbox.0, sandbox.1);
    for (e, s) in &props {
        if !plays || s.map_loads != loads {
            commands.entity(e).try_despawn();
        } else if s.down_since.is_some_and(|t| clock.now >= t + RESET_SECS) {
            commands.queue(move |w: &mut World| reset(w, e));
        }
    }
}

fn describe(world: &World, e: Entity, p: &Prop) -> String {
    let what = match world.get::<ShotGroup>(e) {
        Some(g) => format!("{} hits, {} misses", g.hits.len(), g.misses),
        None => format!("{}/{} standing", p.standing, p.pieces),
    };
    format!("{:>3}  {} at {:.0} m: {what}", p.number, p.spec.kind, p.spec.distance_m)
}

/// `prop group <n>`'s text.
fn group_text(g: &ShotGroup, number: u32) -> String {
    let hits: Vec<String> = g.hits.iter().map(|h| format!("({:+.3}, {:+.3})", h.x, h.y)).collect();
    format!(
        "board {number}: {} hits, {} misses, extreme spread {:.3} m, mean radius {:.3} m, centre ({:+.3}, {:+.3}) m; hits (m, +x right +y up): {}",
        g.hits.len(),
        g.misses,
        g.spread_m,
        g.mean_radius_m,
        g.centre.x,
        g.centre.y,
        if hits.is_empty() { "none".into() } else { hits.join(" ") }
    )
}

/// The props a `<n|all>` names.
fn pick(world: &mut World, which: &str) -> Result<Vec<Entity>, String> {
    let all = all_props(world);
    if which.eq_ignore_ascii_case("all") {
        return Ok(all.into_iter().map(|(e, _)| e).collect());
    }
    let n: u32 = which.parse().map_err(|_| format!("not a prop number: {which} (`prop list`)"))?;
    all.into_iter().find(|(_, p)| p.number == n).map(|(e, _)| vec![e]).ok_or_else(|| format!("no prop {n} (`prop list`)"))
}

const USAGE: &str = "prop add <bottles|tin_cans|target_board> [distance m] (none: under the crosshair) | prop remove <n|all> | prop list | \
    prop group <n> | prop reset <n|all> | prop look <n> [piece | shelf]";

fn command(world: &mut World, a: &[String]) -> Result<Option<String>, String> {
    let sub = a.first().map(|s| s.to_ascii_lowercase()).ok_or(USAGE)?;
    match sub.as_str() {
        "add" => {
            let kind = a.get(1).ok_or(USAGE)?.clone();
            // No distance: under the crosshair; a distance: on that line.
            let e = match a.get(2) {
                Some(d) => {
                    let distance_m = d.trim_end_matches('m').parse().map_err(|_| format!("not a distance: {d}"))?;
                    spawn(world, &SpawnProp { spec: PropSpec { kind, distance_m }, by: None })?
                }
                None => spawn_at_aim(world, &kind, None)?,
            };
            let p = world.get::<Prop>(e).cloned().ok_or("gone")?;
            Ok(Some(format!("added {}", describe(world, e, &p).trim_start())))
        }
        "remove" => {
            let which = a.get(1).map_or("all", String::as_str);
            // All of them is a clear: the remembered layout stays.
            if which.eq_ignore_ascii_case("all") {
                return Ok(Some(format!("removed {} props", clear(world))));
            }
            let all = pick(world, which)?;
            for e in &all {
                remove(world, *e);
            }
            Ok(Some(format!("removed {} props", all.len())))
        }
        "reset" => {
            let all = pick(world, a.get(1).map_or("all", String::as_str))?;
            for e in &all {
                reset(world, *e);
            }
            Ok(Some(format!("reset {} props", all.len())))
        }
        "group" => {
            let e = *pick(world, a.get(1).ok_or("prop group <n>")?)?.first().ok_or("no prop")?;
            let p = world.get::<Prop>(e).cloned().ok_or("no prop")?;
            let g = world.get::<ShotGroup>(e).ok_or_else(|| format!("prop {} is no target board", p.number))?;
            Ok(Some(group_text(g, p.number)))
        }
        "look" => {
            // Turn the player to face prop n (its piece k, from 0; the
            // board's centre): staging a scene.
            let e = *pick(world, a.get(1).ok_or("prop look <n> [piece]")?)?.first().ok_or("no prop")?;
            let tf = *world.get::<Transform>(e).ok_or("the prop has no place")?;
            // `shelf`: the shelf's top between the middle pieces.
            let shelf = a.get(2).is_some_and(|s| s.eq_ignore_ascii_case("shelf"));
            let k: u32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            let spot = if shelf {
                let p = world.get::<Prop>(e).ok_or("no prop")?;
                if p.spec.kind == "target_board" {
                    return Err("a board has no shelf".into());
                }
                let gap = if p.pieces % 2 == 0 { 0.0 } else { 0.125 };
                Vec3::new(gap, SHELF_TOP - 0.005, 0.0)
            } else {
                world
                    .query::<&PropPiece>()
                    .iter(world)
                    .find(|p| p.prop == e && p.index == k)
                    .map(|p| p.stand.translation)
                    .ok_or_else(|| format!("no piece {k}"))?
            };
            let me = command_player(world)?;
            // From its eye (its movement's eye offset; 0.6 m above its
            // centre without one), so a small piece is in the sights.
            let lift = world.get::<crate::core::MovementState>(me).map_or(Vec3::Y * 0.6, |s| s.eye_offset);
            let eye = world.get::<Transform>(me).ok_or("the player has no place")?.translation + lift;
            let d = tf.transform_point(spot) - eye;
            let (yaw, pitch) = ((-d.x).atan2(-d.z), d.y.atan2(d.xz().length()));
            crate::core::set_view(world, me, yaw, pitch);
            let what = if shelf { "its shelf".to_string() } else { format!("piece {k}") };
            Ok(Some(format!("looking at prop {} {what}, {:.1} m away", a[1], d.length())))
        }
        "list" => {
            let all = all_props(world);
            if all.is_empty() {
                return Ok(Some(format!("no props ({USAGE})")));
            }
            let lines: Vec<String> = all.iter().map(|(e, p)| describe(world, *e, p)).collect();
            Ok(Some(format!("{} props:\n{}", all.len(), lines.join("\n"))))
        }
        _ => Err(USAGE.into()),
    }
}

/// The requests (messages), applied as world work.
fn apply_requests(
    mut spawns: MessageReader<SpawnProp>,
    mut removes: MessageReader<RemoveProp>,
    mut clears: MessageReader<ClearProps>,
    mut resets: MessageReader<ResetProp>,
    mut commands: Commands,
) {
    for r in spawns.read().cloned() {
        commands.queue(move |world: &mut World| {
            if let Err(reason) = spawn(world, &r) {
                warn!("prop: {reason}");
                world.write_message(TargetRefused { reason });
            }
        });
    }
    for r in removes.read().copied() {
        commands.queue(move |world: &mut World| remove(world, r.0));
    }
    for r in resets.read().copied() {
        commands.queue(move |world: &mut World| reset(world, r.0));
    }
    if clears.read().count() > 0 {
        commands.queue(|world: &mut World| {
            clear(world);
        });
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<SpawnProp>()
        .add_message::<RemoveProp>()
        .add_message::<ClearProps>()
        .add_message::<ResetProp>()
        .add_message::<PropHit>()
        .add_message::<TargetRefused>()
        .add_message::<WeaponEvent>()
        .add_message::<BlastDealt>()
        .add_message::<Damage>()
        .add_systems(Update, (apply_requests, appear))
        .add_systems(FixedUpdate, clear_debris.run_if(any_with_component::<PropDebris>))
        .add_systems(
            FixedUpdate,
            (prop_hits.after(crate::core::apply_damage).after(SimSet::Weapons), settle.after(SimSet::Rules).before(SimSet::Commands))
                .run_if(crate::core::authoritative)
                .run_if(any_with_component::<Prop>),
        )
        .console_command("chimera:prop", &format!("Practice props in the sandbox: {USAGE}"), command);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_keeps_its_spread_mean_radius_and_centre() {
        let mut g = ShotGroup::default();
        for h in [Vec2::new(0.1, 0.0), Vec2::new(-0.1, 0.0), Vec2::new(0.0, 0.1), Vec2::new(0.0, -0.1)] {
            g.add(h);
        }
        assert!(g.centre.length() < 1e-6);
        assert!((g.mean_radius_m - 0.1).abs() < 1e-6);
        assert!((g.spread_m - 0.2).abs() < 1e-6);
    }
}
