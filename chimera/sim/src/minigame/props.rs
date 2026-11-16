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
//!
//! **What the screens read**: `Prop` (its spec, home, the console's
//! number, its pieces and how many stand) on the prop's entity, a board's
//! `ShotGroup` beside it, and the fact `PropHit` (attacker, prop, piece
//! from 0, the attacker's distance to the prop, simulation time), also in
//! the log.
//!
//! - Bottles (6) and tin cans (5) stand on a shelf about 1 m high. A
//!   bullet (any game's hitscan trace, `WeaponEventKind::Shot` and
//!   `ShotContinued`) whose line crosses a standing piece's box knocks the
//!   first it crosses; a blast (`weapon::grenade::BlastDealt`, any game's
//!   that tells it) knocks every standing piece within its `outer` radius.
//!   A knocked piece is laid on the floor behind the shelf at once (no
//!   physics: the cheapest that reads clearly), `standing` falls and a
//!   `PropHit` is written. When every piece is down they all stand again
//!   `RESET_SECS` later; `ResetProp`, `prop reset` and a new prop stand
//!   them at once.
//! - The board: a 1 m square face, its centre `BOARD_CENTRE_Y` m above
//!   the floor, rings painted on, on two posts. Each bullet line that
//!   crosses the face lands on it: the point in `ShotGroup::hits` (m from
//!   the face's centre, +x right and +y up as the shooter sees it), a
//!   mark drawn there and a `PropHit` (piece 0). A shot fired at the board
//!   (its aim line, `ShotMade`, crosses the board's plane within
//!   `BOARD_AIM_M` of the face's centre, the nearest board so) whose bullets
//!   all missed the face is a `misses`. `spread_m` (extreme spread: the
//!   widest two hits), `mean_radius_m` (the hits' mean distance from their
//!   centre) and `centre` (their mean) are kept current. Cleared by
//!   `ResetProp` and `prop reset`; a new board starts empty. Blasts leave
//!   the board alone.
//!
//! Props exist only while the sandbox plays (`rung_command::holds`): a
//! spawn is refused otherwise, and every prop is removed when it stops or
//! the map is loaded again (`core::MapLoads`). Nothing here is replicated.
//!
//! Console: `prop add <bottles|tin_cans|target_board> [distance m]`,
//! `prop remove <n|all>`, `prop list`, `prop group <n>` (a board's shot
//! group), `prop reset <n|all>`, `prop look <n> [piece]` (turns the player
//! to face prop n's piece, from 0: staging a picture).
//!
//! Known shortcuts:
//! - Props have no collider: bullets pass through them (the line is
//!   tested against the pieces' boxes and the face's plane, so a hit
//!   counts; the bullet's own impact is on what is behind), grenades fly
//!   through, and a prop blocks no blast or sight.
//! - A game whose bullets are projectiles with no hitscan trace (Halo's
//!   guns, rockets) hits no prop with them; its shots at a board count
//!   neither as hits nor misses (a shot counts as a miss only when its
//!   shooter's bullets drew traces that tick). A blast knocks pieces by
//!   distance alone (no line of sight).
//! - A knocked piece doesn't tumble: it is placed lying behind the shelf.

use bevy::prelude::*;

use super::{
    rung_command,
    targets::{TargetRefused, find_spot},
};
use crate::{
    console::{ConsoleAppExt, command_player},
    core::{Intent, MapLoads, SimClock, SimSet},
    map::shapes::{CodeShape, ShapeKind},
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
/// The shelf's top (m).
const SHELF_TOP: f32 = 1.0;
/// The face's front, in front of the board's centre plane (m).
const FACE_FRONT: f32 = 0.015;

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
    /// The console's number.
    pub number: u32,
    /// Bottles 6, cans 5, board 1.
    pub pieces: u32,
    pub standing: u32,
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
}

/// A mark where a bullet landed on a board.
#[derive(Component, Clone, Copy, Debug)]
struct ShotMark;

/// The map a prop was made on, and since when every piece is down.
#[derive(Component, Clone, Copy, Debug, Default)]
struct PropState {
    map_loads: u32,
    down_since: Option<f64>,
}

/// The console's next number.
#[derive(Resource, Default)]
struct PropCount(u32);

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

/// Place a prop now; its entity.
pub fn spawn(world: &mut World, req: &SpawnProp) -> Result<Entity, String> {
    if !rung_command::holds(world) {
        return Err("props are the sandbox's: start it with `sandbox`".into());
    }
    let kind = KINDS
        .iter()
        .find(|k| k.eq_ignore_ascii_case(&req.spec.kind))
        .ok_or_else(|| format!("no prop {:?} (one of {})", req.spec.kind, KINDS.join(", ")))?;
    let by = match req.by {
        Some(e) => e,
        None => command_player(world).map_err(|_| "no player to place it in front of".to_string())?,
    };
    let origin = world.get::<Transform>(by).ok_or("the player has no place")?.translation;
    let yaw = world.get::<Intent>(by).map_or(0.0, |i| i.yaw);
    let dir = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let home = find_spot(world, origin, dir, req.spec.distance_m)?;
    let to = origin - home;
    let facing = Quat::from_rotation_y(to.x.atan2(to.z));
    let number = {
        let mut c = world.get_resource_or_init::<PropCount>();
        c.0 += 1;
        c.0
    };
    let pieces = match *kind {
        "bottles" => 6,
        "tin_cans" => 5,
        _ => 1,
    };
    let map_loads = world.get_resource::<MapLoads>().map_or(0, |m| m.0);
    let spec = PropSpec { kind: kind.to_string(), distance_m: req.spec.distance_m };
    let prop = world
        .spawn((
            Name::new(format!("Prop {number} ({kind})")),
            Transform::from_translation(home).with_rotation(facing),
            Visibility::default(),
            Prop { spec, home, number, pieces, standing: pieces },
            PropState { map_loads, ..default() },
        ))
        .id();
    let wood = Color::srgb(0.45, 0.30, 0.18);
    let part = |world: &mut World, (s, t): (CodeShape, Transform)| world.spawn((s, t, ChildOf(prop))).id();
    if *kind == "target_board" {
        world.entity_mut(prop).insert(ShotGroup::default());
        for x in [-0.4, 0.4] {
            part(world, shape(ShapeKind::Box(Vec3::new(0.03, 0.9, 0.03)), wood, Transform::from_xyz(x, 0.9, -0.05)));
        }
        let stand = Transform::from_xyz(0.0, BOARD_CENTRE_Y, 0.0);
        let face = part(world, shape(ShapeKind::Box(Vec3::new(BOARD_HALF, BOARD_HALF, FACE_FRONT)), Color::srgb(0.92, 0.9, 0.82), stand));
        world.entity_mut(face).insert(PropPiece {
            prop,
            index: 0,
            standing: true,
            stand,
            half: Vec3::new(BOARD_HALF, BOARD_HALF, FACE_FRONT),
            board: true,
        });
        let black = Color::srgb(0.05, 0.05, 0.05);
        let cream = Color::srgb(0.92, 0.9, 0.82);
        for (i, (r, c)) in [(0.4, black), (0.3, cream), (0.2, black), (0.1, cream), (0.04, Color::srgb(0.8, 0.1, 0.1))].into_iter().enumerate() {
            let (s, t) = disc(r, FACE_FRONT + 0.002 * (i as f32 + 1.0), Vec2::ZERO, c);
            world.spawn((s, t, ChildOf(face)));
        }
    } else {
        // The shelf.
        part(world, shape(ShapeKind::Box(Vec3::new(0.8, 0.02, 0.15)), wood, Transform::from_xyz(0.0, SHELF_TOP - 0.02, 0.0)));
        for x in [-0.72, 0.72] {
            part(world, shape(ShapeKind::Box(Vec3::new(0.03, 0.48, 0.12)), wood, Transform::from_xyz(x, 0.48, 0.0)));
        }
        let bottles = *kind == "bottles";
        let (r, hh, step) = if bottles { (0.04, 0.12, 0.25) } else { (0.033, 0.06, 0.25) };
        let colour = if bottles { Color::srgb(0.12, 0.45, 0.18) } else { Color::srgb(0.75, 0.75, 0.78) };
        for i in 0..pieces {
            let x = (i as f32 - (pieces - 1) as f32 / 2.0) * step;
            let stand = Transform::from_xyz(x, SHELF_TOP + hh, 0.0);
            let piece = part(world, shape(ShapeKind::Cylinder { radius: r, half_height: hh }, colour, stand));
            let half = if bottles { Vec3::new(r, hh + 0.04, r) } else { Vec3::new(r, hh, r) };
            world.entity_mut(piece).insert(PropPiece { prop, index: i, standing: true, stand, half, board: false });
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

/// Remove prop `e`.
pub fn remove(world: &mut World, e: Entity) {
    if world.get::<Prop>(e).is_some()
        && let Ok(ent) = world.get_entity_mut(e)
    {
        ent.despawn();
    }
}

/// Remove every prop; how many.
pub fn clear(world: &mut World) -> usize {
    let all: Vec<Entity> = all_props(world).into_iter().map(|(e, _)| e).collect();
    for e in &all {
        remove(world, *e);
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
        }
    }
}

/// Where a segment from `a` to `b` (the prop's frame) enters the box
/// `half` about `c`: the fraction along it.
fn segment_box(a: Vec3, b: Vec3, c: Vec3, half: Vec3) -> Option<f32> {
    let d = b - a;
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for i in 0..3 {
        let (o, v, min, max) = (a[i], d[i], c[i] - half[i], c[i] + half[i]);
        if v.abs() < 1e-9 {
            if o < min || o > max {
                return None;
            }
        } else {
            let (t1, t2) = ((min - o) / v, (max - o) / v);
            lo = lo.max(t1.min(t2));
            hi = hi.min(t1.max(t2));
            if lo > hi {
                return None;
            }
        }
    }
    Some(lo)
}

/// The shot lines and aims of a tick, and the blasts, on the props.
#[allow(clippy::type_complexity)]
fn prop_hits(
    (mut events, mut blasts): (MessageReader<WeaponEvent>, MessageReader<BlastDealt>),
    clock: Res<SimClock>,
    mut props: Query<(Entity, &Transform, &mut Prop, &mut PropState, Option<&mut ShotGroup>)>,
    mut pieces: Query<(Entity, &mut PropPiece, &mut Transform), (Without<Prop>, Without<Intent>)>,
    shooters: Query<&Transform, (With<Intent>, Without<Prop>)>,
    mut out: MessageWriter<PropHit>,
    mut commands: Commands,
) {
    let now = clock.now;
    let mut lines: Vec<(Entity, Vec3, Vec3)> = Vec::new();
    let mut aims: Vec<(Entity, Vec3, Vec3)> = Vec::new();
    for ev in events.read() {
        if ev.replay {
            continue;
        }
        match ev.kind {
            WeaponEventKind::Shot { from, to, .. } | WeaponEventKind::ShotContinued { from, to, .. } => lines.push((ev.owner, from, to)),
            WeaponEventKind::ShotMade { origin, dir, .. } => aims.push((ev.owner, origin, dir)),
            _ => {}
        }
    }
    let blasts: Vec<BlastDealt> = blasts.read().cloned().collect();
    if lines.is_empty() && aims.is_empty() && blasts.is_empty() {
        return;
    }
    let frames: Vec<(Entity, Transform)> = props.iter().map(|(e, t, ..)| (e, *t)).collect();
    let frame = |p: Entity| frames.iter().find(|(e, _)| *e == p).map(|(_, t)| *t);
    let local = |t: &Transform, p: Vec3| t.rotation.inverse() * (p - t.translation);
    let distance = |a: Option<Entity>, p: Entity| {
        a.and_then(|a| shooters.get(a).ok()).zip(frame(p)).map_or(0.0, |(s, t)| s.translation.distance(t.translation))
    };
    // Knocked pieces and landings: (attacker, prop, piece, landing on the face).
    let mut hits: Vec<(Option<Entity>, Entity, Entity, Option<Vec2>)> = Vec::new();
    let mut landed: Vec<(Entity, Entity)> = Vec::new();
    for (owner, from, to) in &lines {
        let mut best: Option<(f32, Entity, Entity, Option<Vec2>)> = None;
        for (pe, piece, _) in &pieces {
            if !piece.standing {
                continue;
            }
            let Some(t) = frame(piece.prop) else { continue };
            let (a, b) = (local(&t, *from), local(&t, *to));
            let found = if piece.board {
                // The face's front plane, crossed from the front.
                let (za, zb) = (a.z - FACE_FRONT, b.z - FACE_FRONT);
                (za > 0.0 && zb <= 0.0).then(|| za / (za - zb)).and_then(|f| {
                    let p = a.lerp(b, f);
                    let on = Vec2::new(p.x, p.y - BOARD_CENTRE_Y);
                    (on.x.abs() <= BOARD_HALF && on.y.abs() <= BOARD_HALF).then_some((f, Some(on)))
                })
            } else {
                segment_box(a, b, piece.stand.translation, piece.half).map(|f| (f, None))
            };
            if let Some((f, on)) = found
                && best.is_none_or(|(bf, ..)| f < bf)
            {
                best = Some((f, piece.prop, pe, on));
            }
        }
        if let Some((_, prop, pe, on)) = best {
            hits.push((Some(*owner), prop, pe, on));
            if on.is_some() {
                landed.push((*owner, prop));
            }
        }
    }
    for b in &blasts {
        for (pe, piece, _) in &pieces {
            let Some(t) = frame(piece.prop) else { continue };
            let at = t.transform_point(piece.stand.translation);
            if piece.standing && !piece.board && at.distance(b.at) <= b.outer {
                hits.push((b.attacker, piece.prop, pe, None));
            }
        }
    }
    // Shots at a board whose shooter's bullets all missed its face.
    let mut at_board: Vec<(Entity, Entity)> = Vec::new();
    for (owner, origin, dir) in &aims {
        if !lines.iter().any(|(o, ..)| o == owner) {
            continue;
        }
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
            at_board.push((*owner, board));
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
                // Off the shelf: lying on the floor behind it.
                piece.standing = false;
                let lie = Quat::from_rotation_y(0.7 * index as f32) * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
                *tf = Transform::from_xyz(piece.stand.translation.x, piece.half.x, -0.45 - 0.08 * (index % 3) as f32).with_rotation(lie);
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

const USAGE: &str = "prop add <bottles|tin_cans|target_board> [distance m] | prop remove <n|all> | prop list | \
    prop group <n> | prop reset <n|all> | prop look <n> [piece]";

fn command(world: &mut World, a: &[String]) -> Result<Option<String>, String> {
    let sub = a.first().map(|s| s.to_ascii_lowercase()).ok_or(USAGE)?;
    match sub.as_str() {
        "add" => {
            let kind = a.get(1).ok_or(USAGE)?.clone();
            let distance_m = match a.get(2) {
                Some(d) => d.trim_end_matches('m').parse().map_err(|_| format!("not a distance: {d}"))?,
                None => 10.0,
            };
            let e = spawn(world, &SpawnProp { spec: PropSpec { kind, distance_m }, by: None })?;
            let p = world.get::<Prop>(e).cloned().ok_or("gone")?;
            Ok(Some(format!("added {}", describe(world, e, &p).trim_start())))
        }
        "remove" => {
            let all = pick(world, a.get(1).map_or("all", String::as_str))?;
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
            let k: u32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            let tf = *world.get::<Transform>(e).ok_or("the prop has no place")?;
            let spot = world
                .query::<&PropPiece>()
                .iter(world)
                .find(|p| p.prop == e && p.index == k)
                .map(|p| p.stand.translation)
                .ok_or_else(|| format!("no piece {k}"))?;
            let me = command_player(world)?;
            let eye = world.get::<Transform>(me).ok_or("the player has no place")?.translation + Vec3::Y * 0.6;
            let d = tf.transform_point(spot) - eye;
            let (yaw, pitch) = ((-d.x).atan2(-d.z), d.y.atan2(d.xz().length()));
            crate::core::set_view(world, me, yaw, pitch);
            Ok(Some(format!("looking at prop {} piece {k}, {:.1} m away", a[1], d.length())))
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
        .add_systems(Update, apply_requests)
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

    #[test]
    fn a_segment_enters_a_box_where_it_meets_its_face() {
        let f = segment_box(Vec3::new(0.0, 0.0, 2.0), Vec3::new(0.0, 0.0, -2.0), Vec3::ZERO, Vec3::splat(0.5)).unwrap();
        assert!((f - 0.375).abs() < 1e-6);
        assert!(segment_box(Vec3::new(1.0, 0.0, 2.0), Vec3::new(1.0, 0.0, -2.0), Vec3::ZERO, Vec3::splat(0.5)).is_none());
    }
}
