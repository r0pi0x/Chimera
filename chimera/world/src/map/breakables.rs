//! Breakable brushes for any game, the simulation's side: gibs
//! (small models thrown when a brush breaks, flying with gravity, bouncing
//! off the world and fading out; a broken prop's pieces are physics
//! bodies that tumble and come to rest; neither blocks characters or
//! shots) and windows
//! made of panes (`BrushPanes` on a brush entity node: only the unbroken
//! panes are drawn and hit by shots). What breaks, and when, is the logic
//! layer's; what each gib looks like is a `GibLook` on it, which
//! `render::breakables` draws (with the windows' meshes); games load the gib models (`MapData::gibs`) and draw the
//! shards of shattered glass (`GlassShatter`, `GlassImpact`).
//! Engine space: meters, Y up.
//!
//! Breakables' physics for any game: gibs (`SpawnGibs`: the game's
//! gib lists `MapData::gibs`, flying under gravity, bouncing off the world with
//! their bounce sound by `MapGibPhysics`' chance and volume rule
//! (`gib_bounce_sound`), their look (`GibLook`) and fade (`gib_alpha`) for
//! render, at most `MAX_GIBS`), broken props' pieces
//! (`BreakProp` → the model's `MapBreak`: its own break pieces at its pose
//! moving outward, or chunks of a gib list in its box; `break_pieces`; avian
//! bodies from the piece model's `.phy` hulls cut for contacts
//! (`collision::contact_hulls`), else its box, on
//! `core::RAGDOLL_LAYER`: they tumble and rest on the world and props,
//! characters and shots pass, and sleep once kept still as props do
//! (`prop_physics::SettlesLikeProp`); debris pieces (`MapBreakPiece::debris`,
//! chunks) don't collide with other debris, pieces or ragdolls, as Source's
//! debris group; built headless too), `BrushPanes` (a broken
//! window's unbroken panes: a collider of pane boxes that shots hit and bodies
//! pass, on `core::TRACE_ONLY_LAYER` so no game's characters meet it;
//! render clips the node's meshes to them), falling pane
//! pieces (`FallingPane`: a body of the `PANE_PIECES` list that shatters on
//! what it touches), `GlassShatter` messages for games to draw shards; at a
//! round restart (`round_restart`, an observer of `core::RoundRestart`)
//! gibs and particles go and windows are whole again.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{MapData, MapModel, MapPart};

/// A gib list: the models one break picks from, by name (Source: the
/// propdata "BreakableModels" lists, "MetalChunks"...).
#[derive(Clone, Debug)]
pub struct MapGibSet {
    pub name: String,
    pub models: Vec<MapModel>,
}

/// How gibs move and look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapGibPhysics {
    /// Downward acceleration (m/s²).
    pub gravity: f32,
    /// Velocity kept per bounce, and for glass gibs.
    pub bounce: f32,
    pub glass_bounce: f32,
    /// Opacity of glass gibs (0..1).
    pub glass_alpha: f32,
    /// Seconds a gib takes to fade out after its life.
    pub fade: f32,
    /// Downward acceleration of a broken prop's pieces (m/s²).
    pub prop_gravity: f32,
    /// A flying gib's bounce makes its sound with this chance, at the
    /// entry's volume × min(1, vertical speed / `sound_full_speed` (m/s)),
    /// at a pitch drawn from the entry's range with chance
    /// `sound_random_pitch`, else the range's start.
    pub sound_chance: f32,
    pub sound_full_speed: f32,
    pub sound_random_pitch: f32,
}

impl Default for MapGibPhysics {
    fn default() -> Self {
        Self {
            gravity: 9.81 / 2.0,
            bounce: 0.5,
            glass_bounce: 0.3,
            glass_alpha: 0.5,
            fade: 1.0,
            prop_gravity: 9.81,
            sound_chance: 0.0,
            sound_full_speed: 1.0,
            sound_random_pitch: 0.0,
        }
    }
}

/// One piece to throw.
#[derive(Clone, Debug, PartialEq)]
pub struct GibPiece {
    pub position: Vec3,
    pub velocity: Vec3,
    /// Angular velocity (rad/s, an axis times its rate).
    pub spin: Vec3,
    /// Seconds before fading.
    pub life: f32,
    /// Which model of the list (None: one at random).
    pub model: Option<usize>,
    /// Its rotation (None: a random one). `position` is then where the
    /// model's origin goes, not its centre.
    pub rotation: Option<Quat>,
    /// Stays where it is (a piece the prop holds in place).
    pub frozen: bool,
    /// Its skin family (one its model lacks shows skin 0).
    pub skin: i32,
    /// Fades out between these distances from the view (meters).
    pub fade_dist: Option<(f32, f32)>,
    /// A prop's debris piece: its body collides with the world and props,
    /// not with other debris (pieces, ragdolls).
    pub debris: bool,
}

impl GibPiece {
    /// A piece thrown at a point, spinning, picked at random.
    pub fn thrown(position: Vec3, velocity: Vec3, spin: Vec3, life: f32) -> Self {
        Self {
            position,
            velocity,
            spin,
            life,
            model: None,
            rotation: None,
            frozen: false,
            skin: 0,
            fade_dist: None,
            debris: false,
        }
    }
}

/// Throw gibs from a gib list (a model is picked at random per piece).
#[derive(Message, Clone, Debug)]
pub struct SpawnGibs {
    pub set: String,
    /// Glass: translucent and less bouncy.
    pub glass: bool,
    /// A broken prop's pieces: they fall at `MapGibPhysics::prop_gravity`.
    pub prop: bool,
    pub pieces: Vec<GibPiece>,
    /// Pieces that shatter on the first thing they touch (falling window
    /// panes, with `prop`): the pane's size and the shards' size
    /// (meters).
    pub shatters: Option<(Vec2, f32)>,
    /// The sound entry a flying gib makes when it bounces (`GibSounds`
    /// says how often and how loud).
    pub bounce: Option<String>,
}

/// The most gibs alive at once (Source `cl_phys_props_max`); more are not
/// spawned.
pub const MAX_GIBS: usize = 300;

/// What a prop model breaks into (Source: the `.phy` break pieces, else
/// chunks of a gib list).
#[derive(Clone, Debug, PartialEq)]
pub enum MapBreak {
    /// One piece per entry, each a model of gib list `set`.
    Pieces { set: String, pieces: Vec<MapBreakPiece> },
    /// `count` chunks of gib list `set`, picked among its first
    /// `size_limit + 1` models; each lives a time in `life`.
    Chunks {
        set: String,
        count: usize,
        size_limit: usize,
        life: (f32, f32),
        /// Each list model's bounds centre (its model space).
        centres: Vec<Vec3>,
        /// The chunks' skin family (prop data `breakable_skin`).
        skin: i32,
    },
}

/// One break piece of a prop model.
#[derive(Clone, Debug, PartialEq)]
pub struct MapBreakPiece {
    /// Index in its gib list.
    pub model: usize,
    /// Where its origin sits in the prop's model space (meters).
    pub offset: Vec3,
    /// Its model's bounds centre (its model space).
    pub centre: Vec3,
    /// Seconds before it fades; infinite: never.
    pub life: f32,
    /// Outward speed added (m/s).
    pub burst: f32,
    pub frozen: bool,
    /// Debris (Source's debris collision group, prop_damage.md 7.3):
    /// collides with the world and props, not with other debris (pieces
    /// of any prop, ragdolls); else interactive (collides with those too).
    pub debris: bool,
    /// Fades out between these distances from the view (meters; the
    /// break block's `fademindist`, `fademaxdist`).
    pub fade_dist: Option<(f32, f32)>,
}

/// A prop broke: throw what its model breaks into (`MapModel::breaks`)
/// from where it was, moving as it moved.
#[derive(Message, Clone, Debug)]
pub struct BreakProp {
    /// Index into `MapData::props` (`PropIndex`).
    pub prop: usize,
    pub transform: Transform,
    pub velocity: Vec3,
    /// Angular velocity (rad/s, an axis times its rate).
    pub spin: Vec3,
    /// Its skin family now (None: the one it was placed with).
    pub skin: Option<i32>,
}

/// Every prop's break pieces, bounds and placed skin, from its model
/// (also headless).
#[derive(Resource, Default)]
pub struct PropBreaks(Vec<(Option<MapBreak>, (Vec3, Vec3), i32)>);

pub fn prop_breaks(data: &MapData) -> PropBreaks {
    PropBreaks(
        data.props
            .iter()
            .map(|p| {
                let m = &data.models[p.model];
                (m.breaks.clone(), m.bounds, p.skin)
            })
            .collect(),
    )
}

/// Template chunks fly out at 100 units/s (in meters/s).
const CHUNK_BURST: f32 = 100.0 * 0.0254;
/// Model pieces' velocity is jittered by this fraction either way.
const PIECE_JITTER: f32 = 0.025;

/// The pieces a broken prop throws (prop_damage.md 7.3, the client
/// branch): its model's break pieces at its pose, in its skin, moving
/// with it (±2.5 %) plus their burst outward and spinning as it spun, or
/// chunks (in the prop data's skin, not spinning) at random points of its
/// box (the smallest axis at its middle) flying out at 100 units/s.
/// `random` gives uniform numbers in [0, 1).
#[allow(clippy::too_many_arguments)]
pub fn break_pieces(
    breaks: &MapBreak,
    bounds: (Vec3, Vec3),
    transform: &Transform,
    velocity: Vec3,
    spin: Vec3,
    skin: i32,
    random: &mut dyn FnMut() -> f32,
) -> Vec<GibPiece> {
    let origin = transform.translation;
    let rot = transform.rotation;
    match breaks {
        MapBreak::Pieces { pieces, .. } => pieces
            .iter()
            .map(|p| {
                let at = origin + rot * p.offset;
                let mid = at + rot * p.centre;
                let out = if at.distance_squared(origin) > 1e-8 {
                    at - origin
                } else {
                    mid - origin
                };
                let jitter = 1.0 + (random() * 2.0 - 1.0) * PIECE_JITTER;
                let v = if p.frozen {
                    Vec3::ZERO
                } else {
                    velocity * jitter + out.normalize_or_zero() * p.burst
                };
                GibPiece {
                    position: at,
                    velocity: v,
                    spin: if p.frozen { Vec3::ZERO } else { spin },
                    life: p.life,
                    model: Some(p.model),
                    rotation: Some(rot),
                    frozen: p.frozen,
                    skin,
                    fade_dist: p.fade_dist,
                    debris: p.debris,
                }
            })
            .collect(),
        MapBreak::Chunks {
            count,
            size_limit,
            life,
            centres,
            skin: chunk_skin,
            ..
        } => {
            let (lo, hi) = bounds;
            let size = hi - lo;
            let smallest = if size.x <= size.y && size.x <= size.z {
                0
            } else if size.y <= size.z {
                1
            } else {
                2
            };
            (0..*count)
                .map(|_| {
                    let mut local = Vec3::new(
                        lo.x + size.x * random(),
                        lo.y + size.y * random(),
                        lo.z + size.z * random(),
                    );
                    local[smallest] = (lo[smallest] + hi[smallest]) / 2.0;
                    let at = origin + rot * local;
                    let pick = ((random() * (*size_limit + 1) as f32) as usize)
                        .min(*size_limit)
                        .min(centres.len().saturating_sub(1));
                    GibPiece {
                        // The chunk's centre goes there.
                        position: at - rot * centres.get(pick).copied().unwrap_or_default(),
                        velocity: velocity + (at - origin).normalize_or_zero() * CHUNK_BURST,
                        spin: Vec3::ZERO,
                        life: life.0 + (life.1 - life.0) * random(),
                        model: Some(pick),
                        rotation: Some(rot),
                        frozen: false,
                        skin: *chunk_skin,
                        fade_dist: None,
                        debris: true,
                    }
                })
                .collect()
        }
    }
}

/// Turn `BreakProp`s into gibs.
pub fn break_props(
    mut events: MessageReader<BreakProp>,
    breaks: Option<Res<PropBreaks>>,
    mut gibs: MessageWriter<SpawnGibs>,
    mut dice: Local<u32>,
) {
    let Some(breaks) = breaks else {
        events.clear();
        return;
    };
    let mut random = || {
        *dice = dice.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (*dice >> 8) as f32 / (1u32 << 24) as f32
    };
    for ev in events.read() {
        let Some((Some(b), bounds, placed_skin)) = breaks.0.get(ev.prop) else {
            continue;
        };
        let set = match b {
            MapBreak::Pieces { set, .. } | MapBreak::Chunks { set, .. } => set.clone(),
        };
        let skin = ev.skin.unwrap_or(*placed_skin);
        let pieces = break_pieces(b, *bounds, &ev.transform, ev.velocity, ev.spin, skin, &mut random);
        if !pieces.is_empty() {
            gibs.write(SpawnGibs {
                set,
                glass: false,
                prop: true,
                pieces,
                shatters: None,
                bounce: None,
            });
        }
    }
}

/// A pane of glass (or tile) shattered: its centre and normal, its size
/// (meters) and the push its shards get (m/s). Games draw the shards.
#[derive(Message, Clone, Debug)]
pub struct GlassShatter {
    pub at: Vec3,
    pub normal: Vec3,
    pub size: Vec2,
    pub velocity: Vec3,
    pub tile: bool,
    /// The shards' size (meters): small for one pane, large for a blast's
    /// burst.
    pub shard: f32,
    /// A falling pane piece shattered where it landed
    /// (`shatter_pane_pieces`; a network server doesn't send these: each
    /// client's own piece shatters).
    pub piece: bool,
}

/// A collapsing pane drops a falling piece (Source: the pane piece
/// model): the pane's reference corner, its axes (along the columns, the
/// rows, the window's normal), its size (meters), the body variant (the
/// `PANE_PIECES` gib list's model) and its spin (rad/s). The piece falls,
/// and shatters (`GlassShatter`) on the first thing it touches.
#[derive(Message, Clone, Debug)]
pub struct FallingPane {
    pub at: Vec3,
    pub axes: [Vec3; 3],
    pub size: Vec2,
    pub body: usize,
    pub spin: Vec3,
    /// The shards' size when it shatters (meters).
    pub shard: f32,
}

/// The gib list of falling pane pieces, one model per body variant.
pub const PANE_PIECES: &str = "PanePieces";
/// The falling pane piece's body variants (random body 0..2).
pub const PANE_PIECE_BODIES: usize = 3;

/// A bullet or club hit shattered the window pane it hit: the hit point
/// and the trace normal (towards the shooter). Games draw the glass
/// impact (specs/cs_source/impact_effects.md section 9).
#[derive(Message, Clone, Copy, Debug)]
pub struct GlassImpact {
    pub at: Vec3,
    pub normal: Vec3,
}

/// A window on a brush entity node, as a grid of panes, in the node's own
/// space: the lower-left corner, one pane's steps along the columns and
/// rows, and the brush's depth behind the face (`depth.0..depth.1` along
/// the face normal from the corner). Only unbroken panes are drawn and
/// collide (when `active`: the window broke; until then the brush is
/// whole).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct BrushPanes {
    pub corner: Vec3,
    pub u: Vec3,
    pub v: Vec3,
    pub depth: (f32, f32),
    pub cols: usize,
    pub rows: usize,
    pub broken: Vec<bool>,
}

impl BrushPanes {
    pub fn normal(&self) -> Vec3 {
        self.u.cross(self.v).normalize_or_zero()
    }

    /// Pane coordinates of a node-space point.
    pub fn pane_at(&self, p: Vec3) -> Vec2 {
        let d = p - self.corner;
        Vec2::new(
            d.dot(self.u) / self.u.length_squared().max(1e-12),
            d.dot(self.v) / self.v.length_squared().max(1e-12),
        )
    }

    pub fn whole(&self, c: usize, r: usize) -> bool {
        !self.broken[r * self.cols + c]
    }

    /// One box per unbroken pane: (centre, rotation, size), node space.
    pub fn boxes(&self) -> Vec<(Vec3, Quat, Vec3)> {
        let n = self.normal();
        let (ud, vd) = (self.u.normalize_or_zero(), self.v.normalize_or_zero());
        // Panes are rectangles; keep the basis orthonormal anyway.
        let vd = (vd - n * vd.dot(n) - ud * vd.dot(ud)).normalize_or_zero();
        let rot = Quat::from_mat3(&Mat3::from_cols(ud, vd, n));
        let thick = (self.depth.1 - self.depth.0).abs().max(1e-3);
        let mid = (self.depth.0 + self.depth.1) / 2.0;
        let mut out = Vec::new();
        for r in 0..self.rows {
            for c in 0..self.cols {
                if !self.whole(c, r) {
                    continue;
                }
                let centre = self.corner + self.u * (c as f32 + 0.5) + self.v * (r as f32 + 0.5) + n * mid;
                out.push((centre, rot, Vec3::new(self.u.length(), self.v.length(), thick)));
            }
        }
        out
    }
}

/// Keep a window's collider to its unbroken panes (`render::breakables`
/// its meshes).
pub fn update_panes(nodes: Query<(Entity, &BrushPanes), Changed<BrushPanes>>, mut commands: Commands) {
    for (node, panes) in &nodes {
        let boxes = panes.boxes();
        if boxes.is_empty() {
            commands.entity(node).insert(ColliderDisabled);
        } else {
            let shapes = boxes
                .iter()
                .map(|(c, r, s)| (*c, *r, Collider::cuboid(s.x, s.y, s.z)))
                .collect();
            commands
                .entity(node)
                .insert((
                    Collider::compound(shapes),
                    // Non-solid once broken: shots still hit the panes;
                    // physics bodies and every game's characters pass
                    // (and shatter them by touch). Traces only: movement
                    // that sweeps colliders (`CHARACTER_FILTER`, Combat
                    // Arms') stopped at a cracked window.
                    CollisionLayers::new(crate::core::TRACE_ONLY_LAYER, LayerMask::NONE),
                ))
                .remove::<ColliderDisabled>();
        }
    }
}

// ---------------------------------------------------------------- gibs

/// Gib models and how they move (`render::breakables` draws them).
#[derive(Resource)]
pub struct GibAssets {
    sets: Vec<(String, Vec<GibModel>)>,
    pub physics: MapGibPhysics,
    pub light_scale: f32,
}

/// A gib model's bounds' centre, its smallest half size (model space),
/// and the body a prop's piece gets: its collider about the centre, mass
/// (kg), damping and surface.
struct GibModel {
    centre: Vec3,
    radius: f32,
    body: GibBody,
}

#[derive(Clone)]
struct GibBody {
    collider: Collider,
    mass: f32,
    damping: (f32, f32),
    /// Surface property (impact sounds).
    surface: Option<String>,
}

/// A piece's body: its collision model's convex pieces (Source's client
/// prop pieces are physics objects), else its bounds' box; placed about
/// the bounds' centre.
fn gib_body(model: &MapModel) -> GibBody {
    let (lo, hi) = model.bounds;
    let centre = (lo + hi) / 2.0;
    let hulls: Vec<(Vec3, Quat, Collider)> = model
        .collision
        .iter()
        .flat_map(|c| &c.pieces)
        // Cut for contacts (`collision::contact_hulls`), as props' are.
        .flat_map(|p| super::collision::contact_hulls(&p.points))
        .map(|hull| (-centre, Quat::IDENTITY, hull))
        .collect();
    let collider = if hulls.is_empty() {
        let size = (hi - lo).max(Vec3::splat(0.02));
        Collider::cuboid(size.x, size.y, size.z)
    } else {
        Collider::compound(hulls)
    };
    let c = model.collision.as_ref();
    GibBody {
        collider,
        mass: c.map_or(1.0, |c| c.mass).max(0.1),
        damping: c.map_or((0.0, 0.0), |c| (c.damping, c.rotdamping)),
        surface: c
            .map(|c| c.surfaceprop.clone())
            .or_else(|| model.surfaceprop.clone())
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty()),
    }
}

/// Gib models (`MapData::gibs`): how each is placed and its body.
pub fn build_assets(data: &MapData) -> Option<GibAssets> {
    if data.gibs.is_empty() {
        return None;
    }
    let sets = data
        .gibs
        .iter()
        .map(|s| {
            let models = s
                .models
                .iter()
                .map(|model| {
                    let (lo, hi) = model.bounds;
                    GibModel {
                        centre: (lo + hi) / 2.0,
                        radius: ((hi - lo) / 2.0).min_element().max(0.0),
                        body: gib_body(model),
                    }
                })
                .collect();
            (s.name.clone(), models)
        })
        .collect();
    Some(GibAssets {
        sets,
        physics: data.gib_physics.unwrap_or_default(),
        light_scale: data.look.light_scale,
    })
}

/// A broken prop's piece with a surface: a client-side physics object
/// whose contacts make impact sounds.
#[derive(Component, Clone, Copy, Debug)]
pub struct PieceBody;

/// A flying (or resting) gib.
#[derive(Component)]
pub struct FlyingGib {
    pub state: GibState,
    pub life: f32,
    pub glass: bool,
    /// Falls at the prop pieces' gravity.
    prop: bool,
    /// Kept this far off what it bounces on (its smallest half size).
    radius: f32,
    /// A physics body moves it (a prop's piece), not `state`.
    body: bool,
    /// The sound entry it makes when it bounces.
    bounce: Option<String>,
}

/// What a gib looks like, for `render::breakables` to draw (none
/// is drawn without it): model `model` of gib list `set` (indices in
/// `MapData::gibs`) in skin family `skin` (one its model lacks shows skin
/// 0), its origin `offset` from the gib's centre, glass (translucent),
/// faded out between `fade` distances from the view (meters).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct GibLook {
    pub set: usize,
    pub model: usize,
    pub skin: i32,
    pub glass: bool,
    pub offset: Vec3,
    pub fade: Option<(f32, f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GibState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: Quat,
    pub spin: Vec3,
    pub age: f32,
    pub resting: bool,
}

/// One step of a gib: fall, spin and move, or bounce off `hit` (point and
/// normal of the first surface on the way), keeping `bounce` of its
/// velocity; it comes to rest on a floor once slow. On a hit, returns the
/// vertical speed it hit with (m/s).
pub fn gib_step(s: &mut GibState, dt: f32, hit: Option<(Vec3, Vec3)>, gravity: f32, bounce: f32) -> Option<f32> {
    s.age += dt;
    if s.resting {
        return None;
    }
    s.rotation = (Quat::from_scaled_axis(s.spin * dt) * s.rotation).normalize();
    match hit {
        None => {
            s.position += s.velocity * dt;
            s.velocity.y -= gravity * dt;
            None
        }
        Some((point, normal)) => {
            let vertical = s.velocity.y;
            s.position = point;
            s.velocity = (s.velocity - 2.0 * s.velocity.dot(normal) * normal) * bounce;
            s.spin *= bounce;
            if normal.y > 0.7 && s.velocity.length() < gravity * 0.1 {
                s.velocity = Vec3::ZERO;
                s.spin = Vec3::ZERO;
                s.resting = true;
            }
            Some(vertical)
        }
    }
}

/// The sound of a gib's bounce at vertical speed `vz` (m/s): None when the
/// chance draw `roll` misses, else its volume and pitch (the entry's
/// range start, or `pitch_draw` from the range when `pitch_roll` says
/// so). Specs: breakables.md "Breaking" 5 and view_models.md 8 (the
/// shared temporary-entity bounce rule).
pub fn gib_bounce_sound(
    p: &MapGibPhysics,
    entry: &super::sound::MapSoundEntry,
    vz: f32,
    roll: f32,
    pitch_roll: f32,
    pitch_draw: f32,
) -> Option<(f32, f32)> {
    if roll >= p.sound_chance {
        return None;
    }
    let volume = entry.volume.draw(0.5) * (vz.abs() / p.sound_full_speed.max(1e-6)).min(1.0);
    let pitch = if pitch_roll < p.sound_random_pitch {
        entry.pitch.draw(pitch_draw)
    } else {
        entry.pitch.start
    };
    Some((volume, pitch))
}

/// A gib's opacity at `age` (fading out over `fade` after `life`); None
/// once gone.
pub fn gib_alpha(age: f32, life: f32, fade: f32) -> Option<f32> {
    if age < life {
        return Some(1.0);
    }
    let a = 1.0 - (age - life) / fade.max(1e-6);
    (a > 0.0).then_some(a)
}

/// Spawn thrown gibs (at most `MAX_GIBS` alive), each with its look
/// (`GibLook`) for render to draw.
pub fn spawn_gibs(
    mut events: MessageReader<SpawnGibs>,
    assets: Option<Res<GibAssets>>,
    alive: Query<(), With<FlyingGib>>,
    gravity: Option<Res<Gravity>>,
    mut dice: Local<u32>,
    mut commands: Commands,
) {
    let Some(assets) = assets else {
        events.clear();
        return;
    };
    // Prop pieces fall at their own gravity.
    let world_gravity = gravity.map_or(9.81, |g| g.0.length());
    let gravity_scale = if world_gravity > 0.0 {
        assets.physics.prop_gravity / world_gravity
    } else {
        1.0
    };
    let mut count = alive.iter().count();
    for ev in events.read() {
        let Some((set, (_, models))) = assets
            .sets
            .iter()
            .enumerate()
            .find(|(_, (n, _))| n.eq_ignore_ascii_case(&ev.set))
        else {
            continue;
        };
        if models.is_empty() {
            continue;
        }
        for piece in &ev.pieces {
            if count >= MAX_GIBS {
                break;
            }
            count += 1;
            *dice = dice.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let index = piece.model.map_or((*dice >> 8) as usize, |m| m.min(models.len() - 1)) % models.len();
            let model = &models[index];
            // Where it's lit and starts: its centre.
            let (rotation, centre) = match piece.rotation {
                Some(r) => (r, piece.position + r * model.centre),
                None => (
                    Quat::from_rotation_y(*dice as f32 * 1e-3) * Quat::from_rotation_x(*dice as f32 * 7e-4),
                    piece.position,
                ),
            };
            let offset = if piece.rotation.is_some() {
                -model.centre
            } else {
                Vec3::ZERO
            };
            let state = GibState {
                position: centre,
                velocity: piece.velocity,
                rotation,
                spin: piece.spin,
                age: 0.0,
                resting: piece.frozen,
            };
            // A prop's pieces are physics bodies (Source's client props):
            // they tumble, collide with the world and props, and come to
            // rest; characters and shots pass through them (the ragdoll
            // layer). Debris pieces (most) don't collide with other debris
            // (pieces of any prop, ragdolls: Source's debris group), so a
            // broken prop's pieces fall through each other onto the floor
            // rather than piling into a stack that rocks on itself; an
            // interactive piece collides with those too. Brush gibs fly on
            // their own.
            let body = ev.prop;
            let mut gib = commands.spawn((
                Name::new("Gib"),
                MapPart,
                FlyingGib {
                    state,
                    life: piece.life,
                    glass: ev.glass,
                    prop: ev.prop,
                    radius: if ev.prop { model.radius } else { 0.0 },
                    body,
                    bounce: ev.bounce.clone(),
                },
                GibLook {
                    set,
                    model: index,
                    skin: piece.skin,
                    glass: ev.glass,
                    offset,
                    fade: piece.fade_dist,
                },
                Transform::from_translation(centre).with_rotation(rotation),
                Visibility::default(),
            ));
            if body {
                let b = &model.body;
                // Its contacts make impact sounds (a client-side physics
                // object's, specs/cs_source/sounds.md 4).
                if let Some(surface) = &b.surface {
                    gib.insert((crate::core::PropSurface(surface.clone()), PieceBody, CollisionEventsEnabled));
                }
                // Before its collider: the contact tree reads it when the
                // collider joins.
                if let Some((size, shard)) = ev.shatters {
                    gib.insert((PanePiece { size, shard }, CollisionEventsEnabled));
                }
                gib.insert((
                    if piece.frozen {
                        RigidBody::Static
                    } else {
                        RigidBody::Dynamic
                    },
                    b.collider.clone(),
                    CollisionLayers::new(
                        crate::core::RAGDOLL_LAYER,
                        if piece.debris {
                            LayerMask::DEFAULT
                        } else {
                            LayerMask::DEFAULT | crate::core::RAGDOLL_LAYER
                        },
                    ),
                    Mass(b.mass),
                    LinearVelocity(piece.velocity),
                    AngularVelocity(piece.spin),
                    LinearDamping(b.damping.0),
                    AngularDamping(b.damping.1.max(0.1)),
                    Friction::new(0.8).with_combine_rule(CoefficientCombine::Multiply),
                    Restitution::new(0.25),
                    GravityScale(gravity_scale),
                    MaxLinearSpeed(2000.0 * 0.0254),
                    MaxAngularSpeed(3600f32.to_radians()),
                    // Asleep once kept still, as props (a light shard
                    // rocking on its curve slows down but may not get
                    // under avian's own threshold).
                    super::prop_physics::SettlesLikeProp,
                ));
            }
        }
    }
}

/// A falling window pane piece: it shatters on the first thing it
/// touches (the pane's size and the shards', meters).
#[derive(Component, Clone, Copy, Debug)]
pub struct PanePiece {
    pub size: Vec2,
    pub shard: f32,
}

/// Seconds a falling pane piece lasts if it never touches anything.
const PANE_PIECE_LIFE: f32 = 5.0;

/// Falling panes become pane pieces: physics debris of the
/// `PANE_PIECES` list in the pane's orientation, falling from its
/// corner (the model's origin there) and spinning.
pub fn fall_panes(mut events: MessageReader<FallingPane>, mut gibs: MessageWriter<SpawnGibs>) {
    for ev in events.read() {
        let [u, v, n] = ev.axes.map(Vec3::normalize_or_zero);
        // The piece model is a square in its own Y/Z plane (Source axes;
        // engine -Z/Y), X its normal: those go onto the pane's axes.
        let rotation = Quat::from_mat3(&Mat3::from_cols(n, v, -u));
        let mut piece = GibPiece::thrown(ev.at, Vec3::ZERO, ev.spin, PANE_PIECE_LIFE);
        piece.model = Some(ev.body);
        piece.rotation = Some(rotation);
        gibs.write(SpawnGibs {
            set: PANE_PIECES.to_string(),
            glass: false,
            prop: true,
            pieces: vec![piece],
            shatters: Some((ev.size, ev.shard)),
            bounce: None,
        });
    }
}

/// Pane pieces that touched something shatter there.
pub fn shatter_pane_pieces(
    mut started: MessageReader<CollisionStart>,
    pieces: Query<(&PanePiece, &Position, &Rotation, &LinearVelocity)>,
    mut shatter: MessageWriter<GlassShatter>,
    mut commands: Commands,
    mut done: Local<Vec<Entity>>,
) {
    done.clear();
    for s in started.read() {
        for e in [s.body1.unwrap_or(s.collider1), s.body2.unwrap_or(s.collider2)] {
            let Ok((piece, at, rot, v)) = pieces.get(e) else { continue };
            if done.contains(&e) {
                continue;
            }
            done.push(e);
            shatter.write(GlassShatter {
                at: at.0,
                normal: rot.0 * Vec3::X,
                size: piece.size,
                velocity: v.0,
                tile: false,
                shard: piece.shard,
                piece: true,
            });
            commands.entity(e).try_despawn();
        }
    }
}

/// Move gibs, bounce them off the world (not characters) with their
/// bounce sound, fade them out.
#[allow(clippy::too_many_arguments)]
pub fn fly_gibs(
    time: Res<Time>,
    assets: Option<Res<GibAssets>>,
    sounds: Option<Res<super::sound::SoundBank>>,
    spatial: SpatialQuery,
    characters: Query<Entity, With<crate::core::Intent>>,
    mut gibs: Query<(Entity, &mut FlyingGib, &mut Transform)>,
    mut play: MessageWriter<super::sound::PlaySound>,
    mut dice: Local<u32>,
    mut commands: Commands,
) {
    let mut unit = || {
        *dice = dice.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (*dice >> 8) as f32 / (1u32 << 24) as f32
    };
    let Some(assets) = assets else { return };
    let p = assets.physics;
    let dt = time.delta_secs().min(0.1);
    let filter = SpatialQueryFilter::from_excluded_entities(characters.iter()).with_mask(crate::core::NOT_SHADOW);
    for (e, mut gib, mut transform) in &mut gibs {
        if gib.body {
            // Its body moves it; it only fades.
            gib.state.age += dt;
            if gib_alpha(gib.state.age, gib.life, p.fade).is_none() {
                commands.entity(e).try_despawn();
            }
            continue;
        }
        let bounce = if gib.glass { p.glass_bounce } else { p.bounce };
        let gravity = if gib.prop { p.prop_gravity } else { p.gravity };
        let s = gib.state;
        let r = gib.radius;
        let travel = s.velocity * dt;
        let hit = if s.resting || dt <= 0.0 {
            None
        } else {
            Dir3::new(travel).ok().and_then(|dir| {
                spatial
                    .cast_ray(s.position, dir, travel.length() + r, true, &filter)
                    .map(|h| {
                        let back = (h.distance - r).max(0.0);
                        (s.position + dir * back + h.normal * 0.01, h.normal)
                    })
            })
        };
        if let Some(vz) = gib_step(&mut gib.state, dt, hit, gravity, bounce)
            && let (Some(name), Some(sounds)) = (gib.bounce.as_deref(), sounds.as_ref())
            && let Some(entry) = sounds.0.entry(name)
            && let Some((volume, pitch)) = gib_bounce_sound(&p, entry, vz, unit(), unit(), unit())
        {
            let mut sound = super::sound::PlaySound::at(name, gib.state.position);
            sound.volume = Some(volume);
            sound.pitch = Some(pitch);
            play.write(sound);
        }
        if gib_alpha(gib.state.age, gib.life, p.fade).is_none() {
            commands.entity(e).try_despawn();
            continue;
        }
        if transform.translation != gib.state.position || transform.rotation != gib.state.rotation {
            transform.translation = gib.state.position;
            transform.rotation = gib.state.rotation;
        }
    }
}

/// A round restart (an observer of `core::RoundRestart`, triggered where
/// the rules count `core::RoundRestarts` up, inside their step): gibs and
/// particles (glass shards) go, and windows drawn as panes are whole
/// again: their meshes uncut and their collider the brush's volumes. The
/// logic layer puts the nodes themselves back (shown, solid, placed).
pub fn round_restart(
    _restart: On<crate::core::RoundRestart>,
    gibs: Query<Entity, With<FlyingGib>>,
    windows: Query<(Entity, &super::MapBrushEntity), With<BrushPanes>>,
    particles: Option<ResMut<super::particles::Particles>>,
    mut commands: Commands,
) {
    for g in &gibs {
        commands.entity(g).despawn();
    }
    if let Some(mut p) = particles {
        p.groups.clear();
    }
    for (node, index) in &windows {
        let index = index.0;
        commands.queue(move |w: &mut World| make_whole(w, node, index));
    }
}

/// A window node back to its whole brush (`render::breakables`
/// draws its meshes whole again).
pub fn make_whole(world: &mut World, node: Entity, index: usize) {
    let collider = world
        .get_resource::<super::MapEntities>()
        .and_then(|m| Some(super::entities::brush_collider(m.entities.get(index)?, m.scale)))
        .flatten();
    let mut e = world.entity_mut(node);
    e.remove::<(BrushPanes, ColliderDisabled)>();
    e.insert(CollisionLayers::default());
    if let Some(c) = collider {
        e.insert(c);
    }
}

/// Remove gibs (map unload).
pub fn unload(world: &mut World) {
    world.remove_resource::<GibAssets>();
    let gibs: Vec<Entity> = world.query_filtered::<Entity, With<FlyingGib>>().iter(world).collect();
    for g in gibs {
        world.entity_mut(g).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panes(cols: usize, rows: usize) -> BrushPanes {
        BrushPanes {
            corner: Vec3::ZERO,
            u: Vec3::X,
            v: Vec3::Y,
            depth: (-0.1, 0.0),
            cols,
            rows,
            broken: vec![false; cols * rows],
        }
    }

    #[test]
    fn pane_boxes_cover_unbroken_panes() {
        let mut p = panes(3, 2);
        assert_eq!(p.boxes().len(), 6);
        p.broken[0] = true;
        let boxes = p.boxes();
        assert_eq!(boxes.len(), 5);
        let (c, _, s) = boxes[0];
        assert!((c - Vec3::new(1.5, 0.5, -0.05)).length() < 1e-5, "{c}");
        assert!((s - Vec3::new(1.0, 1.0, 0.1)).length() < 1e-5);
    }

    #[test]
    fn gib_bounce_sounds_follow_the_shared_rule() {
        // One bounce in six, volume × min(1, |vz| / full), pitch from the
        // range one time in four, else its start.
        let p = MapGibPhysics {
            sound_chance: 1.0 / 6.0,
            sound_full_speed: 450.0 * 0.0254,
            sound_random_pitch: 0.25,
            ..default()
        };
        let entry = crate::map::MapSoundEntry {
            waves: vec![0],
            volume: crate::map::sound::Interval::fixed(0.8),
            pitch: crate::map::sound::Interval {
                start: 90.0,
                range: 20.0,
            },
            level: crate::map::SoundLevel::Db(75.0),
            channel: 0,
            dry: false,
        };
        let full = 450.0 * 0.0254;
        assert_eq!(gib_bounce_sound(&p, &entry, -full, 0.5, 0.0, 0.0), None, "missed the 1/6");
        let (v, pitch) = gib_bounce_sound(&p, &entry, -full / 2.0, 0.1, 0.9, 0.5).unwrap();
        assert!((v - 0.4).abs() < 1e-5 && pitch == 90.0, "{v} {pitch}");
        let (v, pitch) = gib_bounce_sound(&p, &entry, -2.0 * full, 0.1, 0.1, 0.5).unwrap();
        assert!((v - 0.8).abs() < 1e-5 && pitch == 100.0, "{v} {pitch}");
        // A hit reports the vertical speed it came in with.
        let mut s = GibState {
            position: Vec3::ZERO,
            velocity: Vec3::new(0.0, -3.0, 0.0),
            rotation: Quat::IDENTITY,
            spin: Vec3::ZERO,
            age: 0.0,
            resting: false,
        };
        assert_eq!(gib_step(&mut s, 0.1, Some((Vec3::ZERO, Vec3::Y)), 1.0, 0.3), Some(-3.0));
        assert_eq!(gib_step(&mut s, 0.1, None, 1.0, 0.3), None);
    }

    #[test]
    fn gibs_fall_bounce_and_rest() {
        let mut s = GibState {
            position: Vec3::Y,
            velocity: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            spin: Vec3::ZERO,
            age: 0.0,
            resting: false,
        };
        gib_step(&mut s, 0.1, None, 5.0, 0.3);
        assert!((s.velocity.y + 0.5).abs() < 1e-5);
        gib_step(&mut s, 0.1, Some((Vec3::ZERO, Vec3::Y)), 1.0, 0.3);
        assert!(
            (s.velocity.y - 0.15).abs() < 1e-5,
            "bounced up at 0.3: {}",
            s.velocity.y
        );
        s.velocity = Vec3::new(0.0, -0.1, 0.0);
        gib_step(&mut s, 0.1, Some((Vec3::ZERO, Vec3::Y)), 5.0, 0.3);
        assert!(s.resting);
        assert_eq!(gib_alpha(1.0, 2.5, 1.0), Some(1.0));
        assert_eq!(gib_alpha(3.0, 2.5, 1.0), Some(0.5));
        assert_eq!(gib_alpha(3.6, 2.5, 1.0), None);
    }
}
