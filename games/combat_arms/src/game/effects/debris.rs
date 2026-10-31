//! Prop debris (specs/combat_arms/effects_sounds.md Follow-up (2026-10-31b)
//! 1; world_objects.md Follow-up (2026-10-31b) 4): a destructible world
//! model's death sends every client a debris effect of its `DebrisType`
//! record, which throws pieces of the record's models that fall, bounce,
//! spin, sound and fade on each client by itself.
//!
//! - The table (`CaDebrisTable`): the install's `ATTRIBUTES\DEBRIS.TXT`
//!   (records `Debris0`, `Debris1`, ... until one is missing; a record's
//!   number is its id), else ours (data/debris.toml, the spec's six
//!   records). A map object names a record by `Name`, case ignored.
//! - The message (`CaDebris`): the object's box centre, the record's id
//!   and the death direction (the killing damage's), in the world file's
//!   frame; the server sends it to every client (`objects` turns the
//!   object world's `Fx::Debris` into it).
//! - Pieces (`launch`): N = min(`Number`, 20), × 0.333 or 0.666 at low or
//!   medium `DebrisFXLevel` (truncated, at least 1; the lifetimes scaled
//!   the same); models in turn, scale U(0.8, 1.2) × U(min, max), the
//!   velocity drawn per world component between the bounds the death
//!   direction's frame gives `MinVel`/`MaxVel` (plus the world velocity),
//!   the start offset per world axis, the life, the bounce budget and,
//!   with `Rotate`, the spin speeds. One of the explode sounds at the
//!   effect's place (radius 1000).
//! - Motion (`Piece::step`): the position steps with the velocity, then
//!   the velocity takes ½ × `GravityScale` × (0, −1000, 0) (in a liquid
//!   (0, −500, 0)) × dt; the segment is traced against the level and the
//!   solid objects: a hit puts the piece 0.3 u off the surface, mirrors
//!   the velocity and halves it (a quarter in a liquid); a ground hit
//!   (normal y > 0.6) costs a bounce and rests the piece when |v|² < 225 /
//!   dt; a bounce sound with chance 1/4 while bounces are left; none left:
//!   at rest. Sky or invisible: the piece is gone and the effect's other
//!   pieces don't move that frame. Each bounce re-draws the spin; at rest
//!   a piece lies at its yaw alone.
//! - Fade: alive for its life, then (`Fadetime` > 0.1) its alpha falls
//!   over `Fadetime`; gone at life + `Fadetime`.
//!
//! Everything here is in world units and the world file's axes (+Y up);
//! `CaDebrisPiece` entities carry their piece and are placed in the
//! engine's axes for the front end, which draws them (`ui::effects`).
//!
//! Known shortcuts: a record's `WorldSpaceFX` and `ImpactSpaceFX` are read
//! but not played (no stock record names one); the render style is the
//! front end's alpha blend when `Alpha` < 1 (the `RS\` files aren't read);
//! the low-priority sounds play as any other; the debris offset mode along
//! each piece's velocity (effects_sounds.md Open question 32) isn't there.

use avian3d::prelude::*;
use bevy::prelude::*;
use lithtech_formats::attributes::{Document, Section};
use serde::{Deserialize, Serialize};

use super::{Ground, Rolls, sounds::CaSound};
use crate::core::{Intent, SOLID_LAYERS};
use crate::game::{
    inventory::UNIT,
    objects::geometry::{from_engine, rotation_to_engine, to_engine},
    volumes::{CaVolume, in_liquid},
};

/// Pieces of one effect, at most.
pub const MAX_PIECES: u32 = 20;
/// The debris' gravity (u/s², before ½ × `GravityScale`).
pub const GRAVITY: f32 = 1000.0;
pub const LIQUID_GRAVITY: f32 = 500.0;
/// A bounce keeps this much of the mirrored speed (in a liquid).
pub const BOUNCE_KEEP: f32 = 0.5;
pub const LIQUID_BOUNCE_KEEP: f32 = 0.25;
/// How far off a surface a bounce puts a piece (u).
pub const BOUNCE_OFFSET: f32 = 0.3;
/// The rest test's constant (|v|² < REST / dt).
pub const REST: f32 = 225.0;
/// The sounds' radius (u).
pub const SOUND_RADIUS: f32 = 1000.0;
/// A fade shorter than this doesn't fade.
pub const MIN_FADE: f32 = 0.1;
/// The table in the install's attribute archive.
pub const FILE: &str = "attributes/debris.txt";

/// `DebrisFXLevel`.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DebrisLevel {
    Low,
    Medium,
    #[default]
    High,
}

impl DebrisLevel {
    /// The count's and the lifetimes' factor.
    pub fn factor(self) -> f32 {
        match self {
            Self::Low => 0.333,
            Self::Medium => 0.666,
            Self::High => 1.0,
        }
    }
}

/// One record of the debris table (absent keys: scale 1, gravity 1,
/// alpha 1, the rest 0 or empty).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DebrisRecord {
    pub name: String,
    pub surface: u16,
    pub scale: [f32; 2],
    /// (right, forward, up) of the death direction's frame.
    pub vel_min: [f32; 3],
    pub vel_max: [f32; 3],
    /// World (x, y, z).
    pub world_vel_min: [f32; 3],
    pub world_vel_max: [f32; 3],
    pub offset_min: [f32; 3],
    pub offset_max: [f32; 3],
    pub life: [f32; 2],
    pub fade: f32,
    pub number: u32,
    /// Non-zero: spin.
    pub rotate: i32,
    pub bounce: [u32; 2],
    pub gravity: f32,
    pub alpha: f32,
    pub models: Vec<String>,
    pub skins: Vec<String>,
    pub styles: Vec<String>,
    pub bounce_sounds: Vec<String>,
    pub explode_sounds: Vec<String>,
    pub world_fx: String,
    pub impact_fx: String,
}

impl Default for DebrisRecord {
    fn default() -> Self {
        Self {
            name: String::new(),
            surface: 0,
            scale: [1.0, 1.0],
            vel_min: [0.0; 3],
            vel_max: [0.0; 3],
            world_vel_min: [0.0; 3],
            world_vel_max: [0.0; 3],
            offset_min: [0.0; 3],
            offset_max: [0.0; 3],
            life: [0.0; 2],
            fade: 0.0,
            number: 0,
            rotate: 0,
            bounce: [0; 2],
            gravity: 1.0,
            alpha: 1.0,
            models: Vec::new(),
            skins: Vec::new(),
            styles: Vec::new(),
            bounce_sounds: Vec::new(),
            explode_sounds: Vec::new(),
            world_fx: String::new(),
            impact_fx: String::new(),
        }
    }
}

/// The debris table: records by id.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaDebrisTable(pub Vec<DebrisRecord>);

impl CaDebrisTable {
    /// Ours (data/debris.toml).
    pub fn built_in() -> Self {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Doc {
            record: Vec<DebrisRecord>,
        }
        let d: Doc =
            toml::from_str(include_str!("../../../data/debris.toml")).unwrap_or_else(|e| panic!("Combat Arms debris.toml: {e}"));
        Self(d.record)
    }

    /// The install's table (`ATTRIBUTES\DEBRIS.TXT`).
    pub fn parse(doc: &Document) -> Self {
        Self(doc.records("Debris").into_iter().map(record).collect())
    }

    /// The install's table when it can be read, else ours.
    pub fn install() -> Self {
        let read = super::super::INFO
            .mount()
            .and_then(|m| m.read(FILE).ok())
            .and_then(|b| Document::parse(&b).ok())
            .map(|d| Self::parse(&d))
            .filter(|t| !t.0.is_empty());
        read.unwrap_or_else(Self::built_in)
    }

    /// A record's id by name (case ignored).
    pub fn id(&self, name: &str) -> Option<u32> {
        self.0.iter().position(|r| r.name.eq_ignore_ascii_case(name.trim())).map(|i| i as u32)
    }

    pub fn get(&self, id: u32) -> Option<&DebrisRecord> {
        self.0.get(id as usize)
    }
}

/// One `[Debris<n>]` section as a record.
fn record(s: &Section) -> DebrisRecord {
    let text = |k: &str| s.string(k).map(|b| String::from_utf8_lossy(b).trim().to_string()).unwrap_or_default();
    let n = |k: &str, d: f64| s.number(k, d) as f32;
    let v = |k: &str| s.vector(k).map_or([0.0; 3], |v| v.map(|x| x as f32));
    let list = |prefix: &str, from: usize, to: usize| -> Vec<String> {
        (from..=to).map(|i| text(&format!("{prefix}{i}"))).take_while(|t| !t.is_empty()).collect()
    };
    DebrisRecord {
        name: text("Name"),
        surface: s.int("SurfaceId", 0).max(0) as u16,
        scale: [n("MinScale", 1.0), n("MaxScale", 1.0)],
        vel_min: v("MinVel"),
        vel_max: v("MaxVel"),
        world_vel_min: v("MinWorldVel"),
        world_vel_max: v("MaxWorldVel"),
        offset_min: v("MinDebrisOffset"),
        offset_max: v("MaxDebrisOffset"),
        life: [n("MinLifetime", 0.0), n("MaxLifetime", 0.0)],
        fade: n("Fadetime", 0.0),
        number: s.int("Number", 0).max(0) as u32,
        rotate: s.int("Rotate", 0),
        bounce: [s.int("MinBounce", 0).max(0) as u32, s.int("MaxBounce", 0).max(0) as u32],
        gravity: n("GravityScale", 1.0),
        alpha: n("Alpha", 1.0),
        models: list("Model", 1, 5),
        skins: s.string_list("Skin").iter().map(|b| String::from_utf8_lossy(b).trim().to_string()).collect(),
        styles: s.string_list("RenderStyle").iter().map(|b| String::from_utf8_lossy(b).trim().to_string()).collect(),
        bounce_sounds: list("BounceSnd", 1, 2),
        explode_sounds: list("ExplodeSnd", 1, 2),
        world_fx: text("WorldSpaceFX"),
        impact_fx: text("ImpactSpaceFX"),
    }
}

/// A debris effect, sent to every client (world file's frame, u).
#[derive(Message, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaDebris {
    pub at: Vec3,
    pub record: u32,
    pub dir: Vec3,
}

/// How many pieces a record throws at `level`.
pub fn count(number: u32, level: DebrisLevel) -> u32 {
    let n = number.min(MAX_PIECES);
    ((n as f32 * level.factor()) as u32).max(1)
}

/// The death direction's frame: forward, right, up (up toward +Y; right
/// = up × forward, the file's left-handed frame).
pub fn axes(dir: Vec3) -> (Vec3, Vec3, Vec3) {
    let f = dir.try_normalize().unwrap_or(Vec3::Z);
    let r = Vec3::Y.cross(f).try_normalize().unwrap_or(Vec3::X);
    let u = f.cross(r);
    (f, r, u)
}

/// The velocity bounds a record gives along death direction `dir`.
pub fn velocity_bounds(r: &DebrisRecord, dir: Vec3) -> (Vec3, Vec3) {
    let (f, right, up) = axes(dir);
    let along = |v: [f32; 3]| right * v[0] + f * v[1] + up * v[2];
    (Vec3::from(r.world_vel_min) + along(r.vel_min), Vec3::from(r.world_vel_max) + along(r.vel_max))
}

/// What a trace met.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DebrisHit {
    pub at: Vec3,
    pub normal: Vec3,
    /// Sky or an invisible surface.
    pub removes: bool,
}

/// One piece.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub at: Vec3,
    pub v: Vec3,
    /// Which of the record's models.
    pub model: usize,
    pub scale: f32,
    pub alpha: f32,
    /// Its life (s).
    pub life: f32,
    pub age: f32,
    pub bounces: i32,
    pub rotate: bool,
    pub angles: Vec3,
    /// Pitch, yaw, roll speeds (rad/s).
    pub spin: Vec3,
    pub resting: bool,
    pub liquid: bool,
}

/// What one step did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PieceStep {
    Moved,
    /// A bounce: on the ground or not, a bounce sound (its index) or none.
    Bounced { ground: bool, sound: Option<usize> },
    /// Hit sky or an invisible surface: gone.
    Removed,
    Resting,
}

fn between(a: f32, b: f32, u: f32) -> f32 {
    a + (b - a) * u
}

fn spin(roll: &mut impl FnMut() -> f32) -> Vec3 {
    use std::f32::consts::PI;
    Vec3::new(between(-PI, PI, roll()), between(-2.0 * PI, 2.0 * PI, roll()), between(-2.0 * PI, 2.0 * PI, roll()))
}

/// The pieces of record `r` at `at` along death direction `dir` (file
/// frame, u) at `level`, with uniform draws from `roll`.
pub fn launch(r: &DebrisRecord, level: DebrisLevel, at: Vec3, dir: Vec3, roll: &mut impl FnMut() -> f32) -> Vec<Piece> {
    let n = count(r.number, level);
    let (lo, hi) = velocity_bounds(r, dir);
    let (omin, omax) = (Vec3::from(r.offset_min), Vec3::from(r.offset_max));
    (0..n)
        .map(|i| {
            let model = if r.models.is_empty() { 0 } else { i as usize % r.models.len() };
            let scale = between(0.8, 1.2, roll()) * between(r.scale[0], r.scale[1], roll());
            let v = Vec3::new(between(lo.x, hi.x, roll()), between(lo.y, hi.y, roll()), between(lo.z, hi.z, roll()));
            let offset = Vec3::new(
                between(omin.x, omax.x, roll()),
                between(omin.y, omax.y, roll()),
                between(omin.z, omax.z, roll()),
            );
            let life = between(r.life[0], r.life[1], roll()) * level.factor();
            let (b0, b1) = (r.bounce[0], r.bounce[1].max(r.bounce[0]));
            let bounces = (b0 + ((b1 - b0 + 1) as f32 * roll()) as u32).min(b1) as i32;
            let rotate = r.rotate != 0;
            Piece {
                at: at + offset,
                v,
                model,
                scale,
                alpha: r.alpha,
                life,
                age: 0.0,
                bounces,
                rotate,
                angles: Vec3::ZERO,
                spin: if rotate { spin(roll) } else { Vec3::ZERO },
                resting: false,
                liquid: false,
            }
        })
        .collect()
}

impl Piece {
    /// One frame of `dt` s (0 while paused): motion, the trace (`trace`
    /// from, to), bounces and spin; `roll` for the sound's chance and the
    /// new spin.
    pub fn step(
        &mut self,
        gravity_scale: f32,
        dt: f32,
        trace: impl Fn(Vec3, Vec3) -> Option<DebrisHit>,
        roll: &mut impl FnMut() -> f32,
    ) -> PieceStep {
        self.age += dt;
        if self.resting || dt <= 0.0 {
            return if self.resting { PieceStep::Resting } else { PieceStep::Moved };
        }
        let from = self.at;
        let to = from + self.v * dt;
        let g = if self.liquid { LIQUID_GRAVITY } else { GRAVITY };
        self.v.y -= 0.5 * gravity_scale * g * dt;
        let Some(hit) = trace(from, to) else {
            self.at = to;
            if self.rotate {
                self.angles += self.spin * dt;
            }
            return PieceStep::Moved;
        };
        if hit.removes {
            return PieceStep::Removed;
        }
        let n = hit.normal.normalize_or_zero();
        self.at = hit.at + n * BOUNCE_OFFSET;
        let keep = if self.liquid { LIQUID_BOUNCE_KEEP } else { BOUNCE_KEEP };
        self.v = (self.v - 2.0 * self.v.dot(n) * n) * keep;
        let ground = n.y > 0.6;
        if ground {
            self.bounces -= 1;
            let t = if dt > 0.5 { 0.1 } else { dt.clamp(0.001, 0.5) };
            if self.v.length_squared() < REST / t {
                self.rest();
            }
        }
        if self.rotate {
            self.spin = spin(roll);
        }
        let mut sound = None;
        if !self.liquid && self.bounces > 0 {
            let chance = roll();
            let which = roll();
            if chance < 0.25 {
                sound = Some(if which < 0.5 { 0 } else { 1 });
            }
        }
        if self.bounces <= 0 {
            self.rest();
        }
        PieceStep::Bounced { ground, sound }
    }

    fn rest(&mut self) {
        self.resting = true;
        self.v = Vec3::ZERO;
        // Lying flat: its yaw alone.
        self.angles = Vec3::new(0.0, self.angles.y, 0.0);
    }

    /// Its rotation (file frame).
    pub fn rotation(&self) -> Quat {
        crate::game::objects::geometry::rotation(self.angles.x, self.angles.y, self.angles.z)
    }

    /// Its alpha with record fade `fade`.
    pub fn alpha_now(&self, fade: f32) -> f32 {
        if self.age <= self.life || fade <= MIN_FADE {
            return self.alpha;
        }
        self.alpha.min(((self.life + fade - self.age) / fade).max(0.0))
    }

    /// Whether it is gone (life + fade over).
    pub fn done(&self, fade: f32) -> bool {
        self.age >= self.life + fade.max(0.0)
    }
}

/// A piece on a client: its effect, its place in it and the record.
#[derive(Component, Clone, Debug)]
pub struct CaDebrisPiece {
    pub effect: u64,
    pub index: u32,
    pub record: u32,
    pub piece: Piece,
}

/// The front end's look of a piece: its record's model, skin and alpha.
impl CaDebrisPiece {
    pub fn model<'a>(&self, table: &'a CaDebrisTable) -> Option<&'a str> {
        table.get(self.record)?.models.get(self.piece.model).map(String::as_str)
    }
}

// ---------------------------------------------------------------------------
// The client's systems.

/// The next effect's number.
#[derive(Resource, Default)]
pub struct DebrisEffects {
    next: u64,
    rolls: Rolls,
}

/// A debris effect arrived: its pieces and its explode sound.
pub fn spawn(
    mut arrived: MessageReader<CaDebris>,
    table: Option<Res<CaDebrisTable>>,
    level: Res<DebrisLevel>,
    volumes: Query<&CaVolume>,
    mut state: ResMut<DebrisEffects>,
    mut sounds: MessageWriter<CaSound>,
    mut commands: Commands,
) {
    if arrived.is_empty() {
        return;
    }
    let owned;
    let table: &CaDebrisTable = match &table {
        Some(t) => t,
        None => {
            owned = CaDebrisTable::install();
            commands.insert_resource(owned.clone());
            &owned
        }
    };
    for d in arrived.read() {
        let Some(r) = table.get(d.record) else { continue };
        state.next += 1;
        let effect = state.next;
        let mut rolls = std::mem::take(&mut state.rolls);
        let pieces = launch(r, *level, d.at, d.dir, &mut || rolls.next());
        if !r.explode_sounds.is_empty() {
            let k = ((rolls.next() * r.explode_sounds.len() as f32) as usize).min(r.explode_sounds.len() - 1);
            sounds.write(CaSound::at(&r.explode_sounds[k], to_engine(d.at, UNIT), SOUND_RADIUS));
        }
        state.rolls = rolls;
        for (i, mut p) in pieces.into_iter().enumerate() {
            p.liquid = in_liquid(volumes.iter(), to_engine(p.at, UNIT));
            commands.spawn((
                Name::new("Debris"),
                Transform::from_translation(to_engine(p.at, UNIT))
                    .with_rotation(rotation_to_engine(p.rotation()))
                    .with_scale(Vec3::splat(p.scale)),
                CaDebrisPiece {
                    effect,
                    index: i as u32,
                    record: d.record,
                    piece: p,
                },
            ));
        }
    }
}

/// Each frame: every piece's motion, bounces, sounds and fade.
#[allow(clippy::too_many_arguments)]
pub fn step(
    time: Res<Time>,
    table: Option<Res<CaDebrisTable>>,
    mut pieces: Query<(Entity, &mut CaDebrisPiece, &mut Transform)>,
    characters: Query<(), With<Intent>>,
    spatial: SpatialQuery,
    grid: Ground,
    tables: Res<super::data::CaFxTables>,
    mut state: ResMut<DebrisEffects>,
    mut sounds: MessageWriter<CaSound>,
    mut commands: Commands,
) {
    let Some(table) = table else { return };
    let dt = time.delta_secs();
    let filter = SpatialQueryFilter::from_mask(SOLID_LAYERS);
    let mut order: Vec<(u64, u32, Entity)> = pieces.iter().map(|(e, p, _)| (p.effect, p.index, e)).collect();
    order.sort_unstable();
    let mut halted: Option<u64> = None;
    let mut rolls = std::mem::take(&mut state.rolls);
    for (effect, _, e) in order {
        let Ok((_, mut p, mut t)) = pieces.get_mut(e) else { continue };
        let Some(r) = table.get(p.record) else { continue };
        let step_dt = if halted == Some(effect) { 0.0 } else { dt };
        let trace = |from: Vec3, to: Vec3| {
            let (a, b) = (to_engine(from, UNIT), to_engine(to, UNIT));
            let d = b - a;
            let dir = Dir3::new(d).ok()?;
            // Characters are passed through; non-solid objects aren't on
            // the solid layers.
            let hit = spatial.cast_ray_predicate(a, dir, d.length(), true, &filter, &|x| !characters.contains(x))?;
            let at_e = a + d * (hit.distance / d.length());
            let surface = tables.surface(grid.hit(hit.entity, at_e));
            let n = from_engine(hit.normal, 1.0);
            Some(DebrisHit {
                at: from_engine(at_e, UNIT),
                normal: n,
                removes: surface.sky || surface.invisible,
            })
        };
        let gravity = r.gravity;
        let out = p.piece.step(gravity, step_dt, trace, &mut || rolls.next());
        match out {
            PieceStep::Removed => {
                halted = Some(effect);
                commands.entity(e).despawn();
                continue;
            }
            PieceStep::Bounced { sound: Some(k), .. } => {
                if let Some(f) = r.bounce_sounds.get(k).or_else(|| r.bounce_sounds.first()) {
                    sounds.write(CaSound::at(f, to_engine(p.piece.at, UNIT), SOUND_RADIUS));
                }
            }
            _ => {}
        }
        if p.piece.done(r.fade) {
            commands.entity(e).despawn();
            continue;
        }
        t.translation = to_engine(p.piece.at, UNIT);
        t.rotation = rotation_to_engine(p.piece.rotation());
    }
    state.rolls = rolls;
}

/// `debrisfxlevel 0|1|2` (low, medium, high).
pub fn set_level(world: &mut World, v: &str) -> Result<(), String> {
    let level = match v.trim() {
        "0" => DebrisLevel::Low,
        "1" => DebrisLevel::Medium,
        "2" => DebrisLevel::High,
        _ => return Err("debrisfxlevel is 0, 1 or 2".into()),
    };
    world.insert_resource(level);
    Ok(())
}
