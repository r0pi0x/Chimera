//! The logic world of one GoldSrc map as plain data, stepped tick by tick
//! (specs/goldsrc/entities.md): the entities (`Ent`, one per map entity,
//! by its index in `MapData::entities`), the firing model, the players as
//! the logic sees them (`Player`, entity space) and what the logic asks
//! of the engine (`Effect`). The movers' rules are `movers`'.
//!
//! - **Firing** ("Firing"): `fire` uses every entity named, in entity
//!   order, with the activator, the caller and a use type; `fire_targets`
//!   is the shared step (`delay`: a timer that fires later with the
//!   activator if it was a player; `killtarget`; `target`); `UseType::acts`
//!   the receivers' rule; `master_ok` the master question (`multisource`
//!   the only master).
//! - **Logic classes**: `multi_manager` (targets from its other keys, `#`
//!   suffixes cut, sorted by delay, 16 at most; busy unless
//!   multithreaded), `multisource` (its inputs found 0.1 s after spawn),
//!   `trigger_relay` (its own use type, itself the activator),
//!   `trigger_auto` (fires 0.1 s after spawn).
//! - **Triggers**: `trigger_multiple`/`once` (re-arm after `wait`, a
//!   one-shot removed 0.1 s after firing, `message` to the toucher),
//!   `trigger_hurt` (half its per-second damage at most once per 0.5 s
//!   window per player, on/off by use), `trigger_push` (base velocity or
//!   once), `trigger_teleport` (to the first entity named `target`).
//! - **Walls and breakables**: `func_wall` flips its frame by the use-type
//!   rule (the `+a` set), `func_wall_toggle` turns off and on,
//!   `func_breakable` (damage by kind, Only Trigger, Touch, Pressure,
//!   Instant Crowbar, unbreakable glass; breaking: sound, debris, not
//!   solid, nameless, fires `target`, removed 0.1 s later, an explosion of
//!   `explodemagnitude`).
//! - **Looks and sounds**: switchable lights' styles (`styles`: style
//!   number to pattern), `env_sprite` on/off, `env_render` copying render
//!   keys, `ambient_generic` playing, stopping, one-shots.
//! - **CS objectives** (`Objective`): kept as data (zones, the map's
//!   parameters, the armoury's item); a bomb target fires its `target`
//!   when asked (`fire_entity`).
//! - **+use** (Half-Life's rule): the usable entity within 64 units whose
//!   box is most in front of the view (dot above 0.7).
//!
//! A tick (`tick`, "Per-tick order"): the server time advances; each
//! living player's `+use` press and touches; every entity in index order
//! (movers advance on their own clock, others think on server time);
//! the delayed fires due; then the removals due.
//!
//! Known shortcuts (each the spec's assumption or open question):
//! - A round restart re-creates every entity from the map (open question
//!   1: what CS really resets is to measure); the bridge does it.
//! - `+use` follows Half-Life's search (64 units, dot 0.7, no line of
//!   sight) and opens only doors with Use Only (open question 2).
//! - Only players touch, fire and are pushed: hostages, pushables and
//!   other bodies are not entities here (open question 3).
//! - The trigger overlap is the player's box against the trigger's hull 0
//!   regions (open question 10: the clip-hull test is not made).
//! - Light styles: a switchable light's `pattern` key plays through
//!   `map::LightStyles::patterns` (and on network clients, `net`), not
//!   baked into the converted atlas; `tables::letter_brightness` is the spec's letter scale (entities.md; the curve acts on the styles' sum, bsp.md "Drawn light").
//! - Sprites keep their on/off state only: their frames don't animate
//!   (`content::sprites` draws the first); `env_glow` and `cycler_sprite`
//!   have no logic.
//! - `ambient_generic`'s fades, spin-up and LFO are not stepped (it starts
//!   and stops at its volume and pitch); presets are ignored.
//! - `trigger_multiple`'s `message` goes to the toucher as text, not
//!   looked up in `titles.txt` (open question 17).
//! - `env_explosion`, `env_spark`, `env_shooter`, `env_beam`,
//!   `trigger_camera` and `func_tracktrain` have no logic yet.

use std::collections::BTreeMap;

use bevy::math::{Quat, Vec3};

use super::{movers, tables};
use crate::{
    core::{DamageKind, MapBrush},
    map::{MapEntity, entities::entity_rotation},
};

/// An entity: its index in the map's entity list.
pub type Id = usize;

/// How a receiver is told to act.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UseType {
    Off,
    On,
    /// With a number (`+use` sends 1).
    Set(f32),
    Toggle,
}

impl UseType {
    /// `triggerstate`: 0 off, 2 toggle, anything else on.
    pub fn of_state(v: i32) -> Self {
        match v {
            0 => Self::Off,
            2 => Self::Toggle,
            _ => Self::On,
        }
    }

    /// Whether a receiver that is `on` acts: toggle and set always, on
    /// only when off, off only when on.
    pub fn acts(self, on: bool) -> bool {
        match self {
            Self::Toggle | Self::Set(_) => true,
            Self::On => !on,
            Self::Off => on,
        }
    }

    /// The state after it acts on a receiver that is `on`.
    pub fn after(self, on: bool) -> bool {
        match self {
            Self::Toggle | Self::Set(_) => !on,
            Self::On => true,
            Self::Off => false,
        }
    }
}

/// Who started a chain of fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    /// A player (`Player::id`).
    Player(u64),
    Ent(Id),
}

/// A player as the logic sees it, in entity space (units, Z up).
#[derive(Clone, Debug, Default)]
pub struct Player {
    pub id: u64,
    /// Its box's reference point (the engine's character position).
    pub origin: Vec3,
    /// Its box about `origin`.
    pub mins: Vec3,
    pub maxs: Vec3,
    pub velocity: Vec3,
    /// View angles, degrees: pitch (down positive), yaw, roll.
    pub view: Vec3,
    /// The eye above `origin`.
    pub eye: Vec3,
    pub use_key: bool,
    pub alive: bool,
    pub on_ground: bool,
    /// Written by the logic: pushed or carried this tick.
    pub moved: bool,
    /// Written by the logic: moved to a new place (teleported).
    pub teleported: bool,
    /// Written by the logic: a push this tick (`trigger_push`).
    pub base_velocity: Vec3,
    /// Written by the logic: its velocity was set (teleport, a push once).
    pub velocity_set: bool,
    /// Written by the logic: its view was set (teleport).
    pub view_set: bool,
}

impl Player {
    fn centre(&self) -> Vec3 {
        self.origin + (self.mins + self.maxs) / 2.0
    }

    fn half(&self) -> Vec3 {
        (self.maxs - self.mins) / 2.0
    }
}

/// What the logic asks of the engine this tick.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// A one-shot sound (`path` under `sound/`) at a place, or following
    /// an entity.
    Sound {
        path: String,
        at: Vec3,
        entity: Option<Id>,
        volume: f32,
    },
    /// A long sound under `key` started (`path`) or stopped (None).
    Loop {
        key: u64,
        path: Option<String>,
        at: Vec3,
        entity: Option<Id>,
        volume: f32,
    },
    /// Damage (points; negative heals) to a player.
    Damage {
        player: u64,
        points: f32,
        kind: DamageKind,
        by: Option<Id>,
    },
    /// An explosion of a magnitude at a place.
    Explosion { at: Vec3, magnitude: f32, by: Id },
    /// A breakable broke: its debris model.
    Broke { id: Id, gibs: String, dir: Vec3 },
    /// Text for a player (a trigger's `message`).
    Message { player: u64, text: String },
}

/// A brush entity's render keys.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Render {
    pub mode: i32,
    pub amount: f32,
    pub color: [f32; 3],
    pub fx: i32,
}

/// One of an entity's volumes in entity space, local to its origin and
/// unrotated, with its corners.
#[derive(Clone, Debug)]
pub struct Volume {
    pub brush: MapBrush,
    pub corners: Vec<Vec3>,
}

/// One map entity's logic state.
#[derive(Clone, Debug)]
pub struct Ent {
    /// Lower case.
    pub class: String,
    pub targetname: String,
    pub target: String,
    pub killtarget: String,
    pub netname: String,
    pub master: String,
    pub message: String,
    pub delay: f32,
    pub flags: u32,
    /// Entity space, units; a brush entity without `origin` is at 0.
    pub origin: Vec3,
    /// Degrees (pitch, yaw, roll): what it is turned by.
    pub angles: Vec3,
    /// The render frame: 1 shows a brush model's other texture set.
    pub frame: u32,
    pub render: Render,
    /// Its render keys changed (`env_render`).
    pub render_changed: bool,
    /// Gone (killed, broken and removed): fires no longer reach it.
    pub removed: bool,
    pub visible: bool,
    pub solid: bool,
    /// Its volumes (brush entities).
    pub volumes: Vec<Volume>,
    /// The volumes' local bounds.
    pub lo: Vec3,
    pub hi: Vec3,
    /// Server time of its next think (non-movers).
    pub think_at: Option<f64>,
    /// Server time it is removed at.
    pub remove_at: Option<f64>,
    /// A mover's motion (its own clock).
    pub motion: movers::Motion,
    pub kind: Kind,
}

/// What an entity does.
#[derive(Clone, Debug)]
pub enum Kind {
    Inert,
    MultiManager(MultiManager),
    Multisource(Multisource),
    Relay(UseType),
    Auto(UseType),
    Door(Box<movers::Door>),
    Button(Box<movers::Button>),
    Train(Box<movers::Train>),
    PathCorner { wait: f32, speed: f32, fired: bool },
    Rotating(Box<movers::Rotating>),
    Pendulum(Box<movers::Pendulum>),
    Conveyor { speed: f32, dir: Vec3 },
    Wall,
    WallToggle,
    Breakable(Breakable),
    /// A crate (it stays where it spawns: `movers`' shortcut); breakable
    /// with flag 128.
    Pushable(Option<Breakable>),
    Trigger(Trigger),
    Hurt(Hurt),
    Push { speed: f32, dir: Vec3, on: bool },
    Teleport,
    Light { style: u8, pattern: String },
    Sprite { on: bool },
    Render,
    Ambient(Ambient),
    Objective(Objective),
}

#[derive(Clone, Debug, Default)]
pub struct MultiManager {
    /// Names and delays, sorted by delay (stable).
    pub targets: Vec<(String, f32)>,
    /// Running copies: start time, how many fired, the activator.
    pub runs: Vec<(f64, usize, Option<Who>)>,
}

#[derive(Clone, Debug, Default)]
pub struct Multisource {
    /// Its inputs and whether each is on; None until counted.
    pub inputs: Option<Vec<(Id, bool)>>,
}

#[derive(Clone, Debug)]
pub struct Breakable {
    pub health: f32,
    pub material: tables::Material,
    /// `explosion` 1: debris thrown away from the attack.
    pub directed: bool,
    pub gibs: String,
    pub magnitude: f32,
    /// A pressure break is due at this time.
    pub pressure_at: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct Trigger {
    /// Seconds to re-arm; ≤ 0: once.
    pub wait: f32,
    /// Server time it responds again.
    pub armed_at: f64,
    /// Fired once: responds no more.
    pub spent: bool,
}

#[derive(Clone, Debug)]
pub struct Hurt {
    /// Per second; negative heals.
    pub dmg: f32,
    pub kind: DamageKind,
    pub on: bool,
    /// The current 0.5 s window's start and who it hurt.
    pub window: Option<(f64, Vec<u64>)>,
}

#[derive(Clone, Debug)]
pub struct Ambient {
    pub path: String,
    /// 0-1.
    pub volume: f32,
    pub looped: bool,
    pub playing: bool,
    /// Heard everywhere (flag 1).
    pub everywhere: bool,
}

/// A CS objective entity, kept as data.
#[derive(Clone, Debug, PartialEq)]
pub enum Objective {
    /// `func_buyzone` (`team` 0 both, 1 T, 2 CT).
    BuyZone { team: i32 },
    /// `func_bomb_target` or `info_bomb_target`.
    BombTarget,
    /// `func_hostage_rescue` or `info_hostage_rescue`.
    HostageRescue,
    /// `func_vip_safetyzone`.
    VipSafety,
    VipStart,
    Hostage { model: String },
    /// `armoury_entity`: its item number and how many are left.
    Armoury { item: i32, count: i32 },
    /// `info_map_parameters`: `buying`, `bombradius`.
    MapParameters { buying: i32, bomb_radius: f32 },
}

/// The static world's answer to "is this box free?" (entity space), for
/// pushes and carries.
pub trait Solid {
    /// Whether a box (centre, half size) overlaps the static world.
    fn blocked(&self, centre: Vec3, half: Vec3) -> bool;
}

/// A world with nothing in it.
pub struct NoSolid;

impl Solid for NoSolid {
    fn blocked(&self, _: Vec3, _: Vec3) -> bool {
        false
    }
}

/// A delayed fire (`delay`): when, what, how.
#[derive(Clone, Debug)]
struct Delayed {
    at: f64,
    target: String,
    killtarget: String,
    use_type: UseType,
    activator: Option<Who>,
    caller: Id,
}

/// The common keys the engine reads for every class (never a
/// `multi_manager` target).
const COMMON_KEYS: &[&str] = &[
    "classname", "origin", "angles", "angle", "movedir", "model", "targetname", "target", "netname", "message",
    "spawnflags", "speed", "health", "dmg", "skin", "body", "scale", "frame", "framerate", "rendermode",
    "renderamt", "rendercolor", "renderfx", "team", "gravity", "friction",
];

/// `trigger_default_wait`.
pub const TRIGGER_DEFAULT_WAIT: f32 = 0.2;
/// `trigger_remove_delay`.
pub const REMOVE_DELAY: f64 = 0.1;
/// `hurt_interval`.
pub const HURT_INTERVAL: f64 = 0.5;
/// `multi_manager_max`.
pub const MULTI_MANAGER_MAX: usize = 16;
/// `multisource_max`.
pub const MULTISOURCE_MAX: usize = 32;
/// `multisource_setup`, `auto_trigger_delay`.
pub const SETUP_DELAY: f64 = 0.1;
/// `use_radius`, `use_cone`.
pub const USE_RADIUS: f32 = 64.0;
pub const USE_CONE: f32 = 0.7;
/// `teleport_lift`.
pub const TELEPORT_LIFT: f32 = 1.0;
/// `breakable_touch_factor`.
pub const TOUCH_FACTOR: f32 = 0.01;
/// How deep a fire chain may nest in one step (a loop of zero delays
/// stops here).
const MAX_DEPTH: u32 = 64;
/// Fires kept in the log.
const LOG_MAX: usize = 256;

/// The logic world of one map.
#[derive(Clone)]
pub struct World {
    pub ents: Vec<Ent>,
    /// Server time, seconds.
    pub time: f64,
    /// The players during a tick (handed in and back by `tick`).
    pub players: Vec<Player>,
    /// What the engine is to do (taken by the bridge).
    pub effects: Vec<Effect>,
    /// Light styles changed from their map-start patterns, by number.
    pub styles: BTreeMap<u8, String>,
    /// The latest fires (time, name), oldest first.
    pub log: Vec<(f64, String)>,
    delayed: Vec<Delayed>,
    /// Players holding `+use` last tick.
    use_held: Vec<u64>,
    depth: u32,
    rng: u64,
}

/// The last value of a key (case-insensitive), as the spec's "last of a
/// repeated key wins".
pub fn key<'a>(e: &'a MapEntity, k: &str) -> Option<&'a str> {
    e.keyvalues.iter().rev().find(|(n, _)| n.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str())
}

/// A key as a number (absent or bad: 0).
pub fn num(e: &MapEntity, k: &str) -> f32 {
    key(e, k).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0)
}

fn text(e: &MapEntity, k: &str) -> String {
    key(e, k).unwrap_or("").to_string()
}

/// "x y z" (missing parts 0).
pub fn vector(s: &str) -> Vec3 {
    crate::map::entities::parse_vector(s)
}

/// The forward vector of angles (pitch down positive, yaw).
pub fn forward(angles: Vec3) -> Vec3 {
    let (p, y) = (angles.x.to_radians(), angles.y.to_radians());
    // Exact axes for right angles (cos 90° is not 0 in floats).
    let clean = |v: f32| if v.abs() < 1e-6 { 0.0 } else { v };
    Vec3::new(clean(p.cos() * y.cos()), clean(p.cos() * y.sin()), clean(-p.sin()))
}

/// A linear mover's direction from its angles: yaw −1 up, −2 down, else
/// the forward vector.
pub fn movedir(angles: Vec3) -> Vec3 {
    if angles == Vec3::new(0.0, -1.0, 0.0) {
        Vec3::Z
    } else if angles == Vec3::new(0.0, -2.0, 0.0) {
        Vec3::NEG_Z
    } else {
        forward(angles)
    }
}

/// An entity's direction angles: `angles` (or `movedir`, what the
/// conversion renames a mover's `angles` to), else `angle` as a yaw.
pub fn direction_angles(e: &MapEntity) -> Vec3 {
    if let Some(a) = key(e, "movedir").or_else(|| key(e, "angles")) {
        return vector(a);
    }
    key(e, "angle").and_then(|a| a.trim().parse::<f32>().ok()).map_or(Vec3::ZERO, |y| Vec3::new(0.0, y, 0.0))
}

/// A brush entity's volumes (entity space, local to its origin).
pub fn volumes_of(e: &MapEntity) -> Vec<Volume> {
    e.hulls
        .iter()
        .filter(|h| h.planes.len() >= 4)
        .map(|h| Volume {
            brush: MapBrush::from_planes(h.planes.iter().map(|(n, d)| (n.normalize_or_zero(), *d)).collect()),
            corners: h.points.clone(),
        })
        .collect()
}

/// A brush's planes rotated by `rot` about the origin, then moved to
/// `origin`.
pub fn place(v: &Volume, rot: Quat, origin: Vec3) -> MapBrush {
    v.brush.transformed(&v.corners, rot, origin)
}

/// trigger_hurt's `damagetype` bits as a damage kind.
pub fn damage_kind(bits: u32) -> DamageKind {
    if bits & 32 != 0 {
        DamageKind::Fall
    } else if bits & 8 != 0 {
        DamageKind::Burn
    } else if bits & 1 != 0 {
        DamageKind::Crush
    } else if bits & 64 != 0 {
        DamageKind::Blast
    } else if bits & 16384 != 0 {
        DamageKind::Drown
    } else if bits & 262_144 != 0 {
        DamageKind::Radiation
    } else if bits & 131_072 != 0 {
        DamageKind::Poison
    } else if bits & 2 != 0 {
        DamageKind::Bullet
    } else if bits & (4 | 128) != 0 {
        DamageKind::Melee
    } else {
        DamageKind::Generic
    }
}

impl World {
    /// The logic of a map's entities (`MapData::entities`, entity space).
    pub fn new(entities: &[MapEntity], seed: u64) -> Self {
        let mut w = Self {
            ents: Vec::with_capacity(entities.len()),
            time: 0.0,
            players: Vec::new(),
            effects: Vec::new(),
            styles: BTreeMap::new(),
            log: Vec::new(),
            delayed: Vec::new(),
            use_held: Vec::new(),
            depth: 0,
            rng: seed | 1,
        };
        for e in entities {
            let ent = w.spawn(e);
            w.ents.push(ent);
        }
        w
    }

    /// A random number in [0, 1).
    pub fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    /// One entity from its keys.
    fn spawn(&mut self, e: &MapEntity) -> Ent {
        let class = e.classname().to_ascii_lowercase();
        let volumes = volumes_of(e);
        let (lo, hi) = volumes
            .iter()
            .flat_map(|v| &v.corners)
            .fold((Vec3::MAX, Vec3::MIN), |(a, b), p| (a.min(*p), b.max(*p)));
        let (lo, hi) = if volumes.is_empty() { (Vec3::ZERO, Vec3::ZERO) } else { (lo, hi) };
        let color = key(e, "rendercolor").map_or([255.0; 3], |c| vector(c).to_array());
        let mut ent = Ent {
            targetname: text(e, "targetname"),
            target: text(e, "target"),
            killtarget: text(e, "killtarget"),
            netname: text(e, "netname"),
            master: text(e, "master"),
            message: text(e, "message"),
            delay: num(e, "delay"),
            flags: num(e, "spawnflags") as u32,
            origin: key(e, "origin").map_or(Vec3::ZERO, vector),
            // A point entity keeps its angles (a teleport destination's);
            // a brush entity is turned only by its mover class.
            angles: if e.hulls.is_empty() { direction_angles(e) } else { Vec3::ZERO },
            frame: 0,
            render: Render {
                mode: num(e, "rendermode") as i32,
                amount: key(e, "renderamt").map_or(255.0, |_| num(e, "renderamt")),
                color,
                fx: num(e, "renderfx") as i32,
            },
            render_changed: false,
            removed: false,
            visible: true,
            solid: true,
            volumes,
            lo,
            hi,
            think_at: None,
            remove_at: None,
            motion: movers::Motion::default(),
            kind: Kind::Inert,
            class,
        };
        ent.kind = match ent.class.as_str() {
            "multi_manager" => {
                let mut targets: Vec<(String, f32)> = e
                    .keyvalues
                    .iter()
                    .filter(|(k, _)| {
                        !COMMON_KEYS.iter().any(|c| c.eq_ignore_ascii_case(k)) && !k.starts_with("chimera_")
                    })
                    .map(|(k, v)| (k.split('#').next().unwrap_or("").to_string(), v.trim().parse().unwrap_or(0.0)))
                    .take(MULTI_MANAGER_MAX)
                    .collect();
                targets.sort_by(|a, b| a.1.total_cmp(&b.1));
                Kind::MultiManager(MultiManager { targets, runs: Vec::new() })
            }
            "multisource" => {
                ent.think_at = Some(SETUP_DELAY);
                Kind::Multisource(Multisource::default())
            }
            "trigger_relay" => Kind::Relay(UseType::of_state(num(e, "triggerstate") as i32)),
            "trigger_auto" => {
                ent.think_at = Some(SETUP_DELAY);
                Kind::Auto(UseType::of_state(num(e, "triggerstate") as i32))
            }
            "func_wall" => Kind::Wall,
            "func_wall_toggle" => {
                if ent.flags & 1 != 0 {
                    ent.visible = false;
                    ent.solid = false;
                }
                Kind::WallToggle
            }
            "func_illusionary" | "func_ladder" => {
                ent.solid = false;
                Kind::Inert
            }
            "func_conveyor" => {
                if ent.flags & 2 != 0 {
                    ent.solid = false;
                }
                let speed = if num(e, "speed") == 0.0 { 100.0 } else { num(e, "speed") };
                Kind::Conveyor { speed, dir: forward(direction_angles(e)) }
            }
            "func_breakable" => Kind::Breakable(breakable(e)),
            "func_pushable" => Kind::Pushable((ent.flags & 128 != 0).then(|| breakable(e))),
            "trigger_multiple" | "trigger_once" => {
                ent.solid = false;
                ent.visible = false;
                let wait = if ent.class == "trigger_once" {
                    -1.0
                } else if num(e, "wait") == 0.0 {
                    TRIGGER_DEFAULT_WAIT
                } else {
                    num(e, "wait")
                };
                Kind::Trigger(Trigger { wait, armed_at: 0.0, spent: false })
            }
            "trigger_hurt" => {
                ent.solid = false;
                ent.visible = false;
                Kind::Hurt(Hurt {
                    dmg: num(e, "dmg"),
                    kind: damage_kind(num(e, "damagetype") as u32),
                    on: ent.flags & 2 == 0,
                    window: None,
                })
            }
            "trigger_push" => {
                ent.solid = false;
                ent.visible = false;
                let speed = if num(e, "speed") == 0.0 { 100.0 } else { num(e, "speed") };
                let a = direction_angles(e);
                let dir = if a == Vec3::ZERO { Vec3::X } else { movedir(a) };
                Kind::Push { speed, dir, on: ent.flags & 2 == 0 }
            }
            "trigger_teleport" => {
                ent.solid = false;
                ent.visible = false;
                Kind::Teleport
            }
            "light" | "light_spot" => {
                let style = num(e, "style") as i32;
                if ent.targetname.is_empty() || !(32..=255).contains(&style) {
                    Kind::Inert
                } else {
                    let pattern = key(e, "pattern").filter(|p| !p.is_empty()).unwrap_or("m").to_string();
                    let now = if ent.flags & 1 != 0 { "a".to_string() } else { pattern.clone() };
                    self.styles.insert(style as u8, now);
                    Kind::Light { style: style as u8, pattern }
                }
            }
            "env_sprite" => Kind::Sprite { on: ent.targetname.is_empty() || ent.flags & 1 != 0 },
            "env_render" => Kind::Render,
            "ambient_generic" => {
                let path = ent.message.trim().trim_start_matches('*').to_string();
                if path.is_empty() {
                    ent.removed = true;
                    Kind::Inert
                } else {
                    let looped = ent.flags & 32 == 0;
                    let playing = looped && ent.flags & 16 == 0;
                    Kind::Ambient(Ambient {
                        path,
                        volume: (num(e, "health") * 10.0).clamp(0.0, 100.0) / 100.0,
                        looped,
                        playing,
                        everywhere: ent.flags & 1 != 0,
                    })
                }
            }
            "func_buyzone" => Kind::Objective(Objective::BuyZone { team: num(e, "team") as i32 }),
            "func_bomb_target" | "info_bomb_target" => Kind::Objective(Objective::BombTarget),
            "func_hostage_rescue" | "info_hostage_rescue" => Kind::Objective(Objective::HostageRescue),
            "func_vip_safetyzone" => Kind::Objective(Objective::VipSafety),
            "info_vip_start" => Kind::Objective(Objective::VipStart),
            "hostage_entity" => Kind::Objective(Objective::Hostage { model: text(e, "model") }),
            "armoury_entity" => Kind::Objective(Objective::Armoury {
                item: num(e, "item") as i32,
                count: if key(e, "count").is_some() { num(e, "count") as i32 } else { 1 },
            }),
            "info_map_parameters" => Kind::Objective(Objective::MapParameters {
                buying: num(e, "buying") as i32,
                bomb_radius: if num(e, "bombradius") > 0.0 { num(e, "bombradius") } else { 500.0 },
            }),
            _ => movers::spawn(&mut ent, e).unwrap_or(Kind::Inert),
        };
        if matches!(ent.kind, Kind::Objective(_)) {
            ent.solid = false;
            ent.visible = false;
        }
        ent
    }

    /// The entity `id`, if it is still there.
    pub fn get(&self, id: Id) -> Option<&Ent> {
        self.ents.get(id).filter(|e| !e.removed)
    }

    /// The entities named `name`, in entity order.
    pub fn named(&self, name: &str) -> Vec<Id> {
        if name.is_empty() {
            return Vec::new();
        }
        (0..self.ents.len()).filter(|&i| !self.ents[i].removed && self.ents[i].targetname == name).collect()
    }

    /// A light style's pattern now (map-start presets, else what a light
    /// set; None: unknown, normal).
    pub fn style(&self, style: u8) -> Option<&str> {
        self.styles.get(&style).map(String::as_str).or_else(|| tables::preset_style(style))
    }

    /// The CS objectives, as data.
    pub fn objectives(&self) -> Vec<(Id, &Objective)> {
        self.ents
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match &e.kind {
                Kind::Objective(o) => Some((i, o)),
                _ => None,
            })
            .collect()
    }

    /// Its volumes where it is now (entity space).
    pub fn brushes(&self, id: Id) -> Vec<MapBrush> {
        let e = &self.ents[id];
        let rot = entity_rotation(e.angles);
        e.volumes.iter().map(|v| place(v, rot, e.origin)).collect()
    }

    /// Its bounds where it is now (entity space).
    pub fn bounds(&self, id: Id) -> (Vec3, Vec3) {
        self.brushes(id)
            .iter()
            .fold((Vec3::MAX, Vec3::MIN), |(a, b), br| (a.min(br.min), b.max(br.max)))
    }

    fn log_fire(&mut self, name: &str) {
        if self.log.len() >= LOG_MAX {
            self.log.remove(0);
        }
        self.log.push((self.time, name.to_string()));
    }

    // ---- Firing ----

    /// Fire name `name`: use every entity so named, in entity order.
    pub fn fire(&mut self, name: &str, activator: Option<Who>, caller: Option<Id>, use_type: UseType) {
        if name.is_empty() || self.depth >= MAX_DEPTH {
            return;
        }
        self.log_fire(name);
        self.depth += 1;
        for id in self.named(name) {
            // Removed by an earlier receiver of this fire: not reached.
            if !self.ents[id].removed {
                self.use_entity(id, activator, caller, use_type);
            }
        }
        self.depth -= 1;
    }

    /// The shared step ("fire my targets") of entity `id`.
    pub fn fire_targets(&mut self, id: Id, activator: Option<Who>, use_type: UseType) {
        let e = &self.ents[id];
        let (target, killtarget, delay) = (e.target.clone(), e.killtarget.clone(), e.delay);
        self.fire_with(id, &target, &killtarget, delay, activator, use_type);
    }

    /// The shared step with an explicit target (doors' `netname`).
    pub fn fire_with(
        &mut self,
        id: Id,
        target: &str,
        killtarget: &str,
        delay: f32,
        activator: Option<Who>,
        use_type: UseType,
    ) {
        if target.is_empty() && killtarget.is_empty() {
            return;
        }
        if delay != 0.0 {
            self.delayed.push(Delayed {
                at: self.time + f64::from(delay),
                target: target.to_string(),
                killtarget: killtarget.to_string(),
                use_type,
                activator: activator.filter(|a| matches!(a, Who::Player(_))),
                caller: id,
            });
            return;
        }
        self.kill(killtarget);
        self.fire(target, activator, Some(id), use_type);
    }

    /// Remove every entity named `name`.
    pub fn kill(&mut self, name: &str) {
        for id in self.named(name) {
            self.remove(id);
        }
    }

    /// Remove an entity now.
    pub fn remove(&mut self, id: Id) {
        let e = &mut self.ents[id];
        e.removed = true;
        e.visible = false;
        e.solid = false;
        if let Kind::Ambient(a) = &mut e.kind
            && a.playing
        {
            a.playing = false;
            self.effects.push(Effect::Loop { key: sound_key(id, 1), path: None, at: Vec3::ZERO, entity: Some(id), volume: 0.0 });
        }
    }

    /// Whether the master named `master` lets an entity act.
    pub fn master_ok(&self, master: &str) -> bool {
        if master.is_empty() {
            return true;
        }
        for id in self.named(master) {
            if let Kind::Multisource(m) = &self.ents[id].kind {
                return m.inputs.as_ref().is_some_and(|i| i.iter().all(|(_, on)| *on));
            }
        }
        true
    }

    /// Fire an entity's targets for the game's rules (a bomb target's
    /// when the bomb explodes there).
    pub fn fire_entity(&mut self, id: Id, activator: Option<Who>) {
        if self.get(id).is_some() {
            self.fire_targets(id, activator, UseType::Toggle);
        }
    }

    /// Use entity `id`.
    pub fn use_entity(&mut self, id: Id, activator: Option<Who>, caller: Option<Id>, use_type: UseType) {
        if self.ents[id].removed {
            return;
        }
        let kind = std::mem::discriminant(&self.ents[id].kind);
        match &mut self.ents[id].kind {
            Kind::MultiManager(m) => {
                if !m.runs.is_empty() && self.ents[id].flags & 1 == 0 {
                    return;
                }
                let now = self.time;
                if let Kind::MultiManager(m) = &mut self.ents[id].kind {
                    m.runs.push((now, 0, activator));
                }
                self.run_manager(id);
            }
            Kind::Multisource(m) => {
                let Some(caller) = caller else { return };
                let Some(inputs) = m.inputs.as_mut() else { return };
                let Some(slot) = inputs.iter_mut().find(|(c, _)| *c == caller) else { return };
                slot.1 = !slot.1;
                if inputs.iter().all(|(_, on)| *on) {
                    self.fire_targets(id, None, UseType::Toggle);
                }
            }
            Kind::Relay(state) => {
                let state = *state;
                self.fire_targets(id, Some(Who::Ent(id)), state);
                if self.ents[id].flags & 1 != 0 {
                    self.remove(id);
                }
            }
            Kind::Wall => {
                let on = self.ents[id].frame != 0;
                if use_type.acts(on) {
                    self.ents[id].frame = u32::from(use_type.after(on));
                }
            }
            Kind::WallToggle => {
                let on = self.ents[id].visible;
                if use_type.acts(on) {
                    let on = use_type.after(on);
                    self.ents[id].visible = on;
                    self.ents[id].solid = on;
                }
            }
            Kind::Breakable(_) | Kind::Pushable(Some(_)) => {
                let yaw = self.ents[id].angles.y;
                self.break_now(id, forward(Vec3::new(0.0, yaw, 0.0)));
            }
            Kind::Hurt(h) => h.on = !h.on,
            Kind::Push { on, .. } => *on = !*on,
            Kind::Light { style, pattern } => {
                let (style, pattern) = (*style, pattern.clone());
                let on = self.style(style).is_none_or(|p| p != "a");
                if use_type.acts(on) {
                    let now = if use_type.after(on) { pattern } else { "a".to_string() };
                    self.styles.insert(style, now);
                }
            }
            Kind::Sprite { on } => {
                if use_type.acts(*on) {
                    *on = use_type.after(*on);
                    self.ents[id].frame = 0;
                }
            }
            Kind::Render => self.copy_render(id),
            Kind::Ambient(_) => self.ambient_use(id, use_type),
            Kind::Conveyor { speed, .. } => *speed = -*speed,
            _ => {
                let _ = kind;
                self.mover_use(id, activator, caller, use_type);
            }
        }
    }

    /// Fire a multi_manager's names whose delays have elapsed.
    fn run_manager(&mut self, id: Id) {
        let Kind::MultiManager(m) = &self.ents[id].kind else { return };
        let (targets, mut runs) = (m.targets.clone(), m.runs.clone());
        let now = self.time;
        for run in &mut runs {
            while let Some((name, delay)) = targets.get(run.1) {
                if run.0 + f64::from(*delay) > now + 1e-9 {
                    break;
                }
                run.1 += 1;
                self.fire(name, run.2, Some(id), UseType::Toggle);
            }
        }
        runs.retain(|r| r.1 < targets.len());
        if let Kind::MultiManager(m) = &mut self.ents[id].kind {
            m.runs = runs;
        }
    }

    /// `env_render`: its render keys onto every entity named `target`.
    fn copy_render(&mut self, id: Id) {
        let (render, flags, target) = (self.ents[id].render, self.ents[id].flags, self.ents[id].target.clone());
        for t in self.named(&target) {
            let r = &mut self.ents[t].render;
            if flags & 1 == 0 {
                r.fx = render.fx;
            }
            if flags & 2 == 0 {
                r.amount = render.amount;
            }
            if flags & 4 == 0 {
                r.mode = render.mode;
            }
            if flags & 8 == 0 {
                r.color = render.color;
            }
            self.ents[t].render_changed = true;
        }
    }

    fn ambient_use(&mut self, id: Id, use_type: UseType) {
        let at = self.ents[id].origin;
        let Kind::Ambient(a) = &mut self.ents[id].kind else { return };
        if let UseType::Set(_) = use_type
            && a.playing
        {
            // Only a pitch change (not modelled: module doc).
            return;
        }
        let entity = (!a.everywhere).then_some(id);
        if a.playing {
            a.playing = false;
            self.effects.push(Effect::Loop { key: sound_key(id, 1), path: None, at, entity, volume: 0.0 });
        } else if a.looped {
            a.playing = true;
            let (path, volume) = (a.path.clone(), a.volume);
            self.effects.push(Effect::Loop { key: sound_key(id, 1), path: Some(path), at, entity, volume });
        } else {
            let (path, volume) = (a.path.clone(), a.volume);
            self.effects.push(Effect::Sound { path, at, entity, volume });
        }
    }

    /// Every wave (a path under `sound/`) the entities may play, each
    /// once, in entity order: the movers' sounds by their tables, the
    /// ambient sounds, the breakables' break sounds, and `+use`'s two
    /// (what a map's sound bank needs, `content::sounds::add_map`).
    pub fn sound_paths(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut add = |p: Option<&String>| {
            if let Some(p) = p.map(|p| p.trim().to_ascii_lowercase())
                && !p.is_empty()
                && !out.contains(&p)
            {
                out.push(p);
            }
        };
        let fixed = ["common/wpn_select.wav".to_string(), "common/wpn_denyselect.wav".to_string()];
        fixed.iter().for_each(|p| add(Some(p)));
        for e in &self.ents {
            match &e.kind {
                Kind::Door(d) => [&d.move_sound, &d.stop_sound, &d.locked_sound, &d.unlocked_sound].into_iter().for_each(|p| add(p.as_ref())),
                Kind::Button(b) => [&b.sound, &b.locked_sound, &b.unlocked_sound].into_iter().for_each(|p| add(p.as_ref())),
                Kind::Train(t) => [&t.move_sound, &t.stop_sound].into_iter().for_each(|p| add(p.as_ref())),
                Kind::Rotating(r) => add(r.sound.as_ref()),
                Kind::Ambient(a) => add(Some(&a.path)),
                Kind::Breakable(b) | Kind::Pushable(Some(b)) => {
                    b.material.break_sounds().iter().for_each(|p| add(Some(&p.to_string())))
                }
                _ => {}
            }
        }
        out
    }

    /// The looped ambient sounds playing now (map start: those not
    /// started silent), for a late start or a client.
    pub fn ambient_loops(&self) -> Vec<(u64, String, Vec3, f32)> {
        self.ents
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match &e.kind {
                Kind::Ambient(a) if a.playing && !e.removed => Some((sound_key(i, 1), a.path.clone(), e.origin, a.volume)),
                _ => None,
            })
            .collect()
    }

    // ---- Breakables ----

    /// Damage `points` of `kind` to entity `id` (a breakable's rule;
    /// shootable buttons press).
    pub fn damage(&mut self, id: Id, points: f32, kind: DamageKind, from: Vec3) {
        if self.ents[id].removed {
            return;
        }
        if let Kind::Button(_) = self.ents[id].kind {
            self.button_damaged(id);
            return;
        }
        let flags = self.ents[id].flags;
        let (centre, origin) = {
            let (lo, hi) = self.bounds(id);
            ((lo + hi) / 2.0, self.ents[id].origin)
        };
        let _ = origin;
        let b = match &mut self.ents[id].kind {
            Kind::Breakable(b) | Kind::Pushable(Some(b)) => b,
            _ => return,
        };
        // Only Trigger; unbreakable glass.
        if flags & 1 != 0 || b.material == tables::Material::UnbreakableGlass {
            return;
        }
        let melee = kind == DamageKind::Melee;
        let d = if melee && flags & 256 != 0 {
            b.health
        } else if melee {
            points * 2.0
        } else if kind == DamageKind::Poison {
            points * 0.1
        } else {
            points
        };
        b.health -= d;
        if b.health <= 0.0 {
            self.break_now(id, (from - centre).normalize_or_zero());
        }
    }

    /// A breakable breaks now; `dir` the attack's direction for debris.
    pub fn break_now(&mut self, id: Id, dir: Vec3) {
        let (lo, hi) = self.bounds(id);
        let centre = (lo + hi) / 2.0;
        let (sounds, gibs, directed, magnitude, health) = match &self.ents[id].kind {
            Kind::Breakable(b) | Kind::Pushable(Some(b)) => {
                (b.material.break_sounds(), b.gibs.clone(), b.directed, b.magnitude, b.health)
            }
            _ => return,
        };
        if self.ents[id].remove_at.is_some() {
            return;
        }
        let pick = (self.random() * sounds.len() as f32) as usize % sounds.len().max(1);
        let volume = (0.85 + 0.15 * self.random() + health.abs() / 100.0).min(1.0);
        self.effects.push(Effect::Sound { path: sounds[pick].to_string(), at: centre, entity: None, volume });
        self.effects.push(Effect::Broke { id, gibs, dir: if directed { dir } else { Vec3::ZERO } });
        let e = &mut self.ents[id];
        e.solid = false;
        e.visible = false;
        e.targetname.clear();
        e.remove_at = Some(self.time + REMOVE_DELAY);
        self.fire_targets(id, None, UseType::Toggle);
        if magnitude > 0.0 {
            self.effects.push(Effect::Explosion { at: centre, magnitude, by: id });
        }
    }

    // ---- The tick ----

    /// One server frame of `dt` seconds with these players (handed back
    /// with what the logic did to them); `solid` the static world.
    pub fn tick(&mut self, dt: f64, players: &mut Vec<Player>, solid: &dyn Solid) {
        self.time += dt;
        self.players = std::mem::take(players);
        for p in &mut self.players {
            p.moved = false;
            p.teleported = false;
            p.base_velocity = Vec3::ZERO;
            p.velocity_set = false;
            p.view_set = false;
        }
        // 1. The players' commands: +use, then touches.
        let held: Vec<u64> = self.players.iter().filter(|p| p.alive && p.use_key).map(|p| p.id).collect();
        for k in 0..self.players.len() {
            let p = &self.players[k];
            if p.alive && p.use_key && !self.use_held.contains(&p.id) {
                self.player_use(k);
            }
        }
        self.use_held = held;
        for k in 0..self.players.len() {
            if self.players[k].alive {
                self.touches(k);
            }
        }
        // 2. Every entity in index order.
        for id in 0..self.ents.len() {
            if self.ents[id].removed {
                continue;
            }
            if self.ents[id].motion.mover {
                self.run_mover(id, dt, solid);
            } else if let Some(at) = self.ents[id].think_at
                && at <= self.time + 1e-9
            {
                self.ents[id].think_at = None;
                self.think(id);
            }
            if let Kind::MultiManager(m) = &self.ents[id].kind
                && !m.runs.is_empty()
            {
                self.run_manager(id);
            }
        }
        // 3. The delayed fires due, in the order they were made.
        let mut i = 0;
        while i < self.delayed.len() {
            if self.delayed[i].at <= self.time + 1e-9 {
                let d = self.delayed.remove(i);
                self.kill(&d.killtarget);
                self.fire(&d.target, d.activator, Some(d.caller), d.use_type);
            } else {
                i += 1;
            }
        }
        // 4. Removals due.
        for id in 0..self.ents.len() {
            if let Some(at) = self.ents[id].remove_at
                && at <= self.time + 1e-9
                && !self.ents[id].removed
            {
                self.remove(id);
            }
        }
        *players = std::mem::take(&mut self.players);
    }

    /// A non-mover's think.
    fn think(&mut self, id: Id) {
        match &self.ents[id].kind {
            Kind::Multisource(_) => {
                let name = self.ents[id].targetname.clone();
                let mut inputs: Vec<(Id, bool)> = Vec::new();
                if !name.is_empty() {
                    for (i, e) in self.ents.iter().enumerate() {
                        if !e.removed && e.target == name {
                            inputs.push((i, false));
                        }
                    }
                    for (i, e) in self.ents.iter().enumerate() {
                        if let Kind::MultiManager(m) = &e.kind
                            && !e.removed
                            && m.targets.iter().any(|(t, _)| *t == name)
                            && !inputs.iter().any(|(j, _)| *j == i)
                        {
                            inputs.push((i, false));
                        }
                    }
                }
                inputs.truncate(MULTISOURCE_MAX);
                if let Kind::Multisource(m) = &mut self.ents[id].kind {
                    m.inputs = Some(inputs);
                }
            }
            Kind::Auto(state) => {
                let state = *state;
                self.fire_targets(id, Some(Who::Ent(id)), state);
                if self.ents[id].flags & 1 != 0 {
                    self.remove(id);
                }
            }
            Kind::Breakable(_) | Kind::Pushable(Some(_)) => {
                // A pressure break due.
                self.break_now(id, Vec3::ZERO);
            }
            _ => {}
        }
    }

    // ---- Players ----

    /// Whether a player's box overlaps brush `b` by more than `skin`
    /// (negative: comes within).
    fn player_in(p: &Player, b: &MapBrush, skin: f32) -> bool {
        // The brush's bounds test takes no skin: a negative one grows the box.
        let grow = (-skin).max(0.0);
        b.overlaps_box(p.centre(), p.half() + Vec3::splat(grow), skin.max(0.0))
    }

    /// Player `k`'s touches this tick.
    fn touches(&mut self, k: usize) {
        for id in 0..self.ents.len() {
            let e = &self.ents[id];
            if e.removed || e.volumes.is_empty() {
                continue;
            }
            let touchy = matches!(
                e.kind,
                Kind::Trigger(_) | Kind::Hurt(_) | Kind::Push { .. } | Kind::Teleport | Kind::Door(_) | Kind::Button(_)
                    | Kind::Breakable(_) | Kind::Conveyor { .. }
            );
            if !touchy {
                continue;
            }
            let brushes = self.brushes(id);
            let p = &self.players[k];
            match &self.ents[id].kind {
                // Solid ones are touched within a unit (walked into).
                Kind::Door(_) | Kind::Button(_) | Kind::Breakable(_) => {
                    if self.ents[id].solid && brushes.iter().any(|b| Self::player_in(p, b, -1.0)) {
                        self.touch_solid(id, k);
                    }
                }
                // Riders of a conveyor are moved along it.
                Kind::Conveyor { speed, dir } => {
                    let (speed, dir) = (*speed, *dir);
                    let mut below = p.clone();
                    below.origin.z -= 2.0;
                    if self.ents[id].solid
                        && self.ents[id].flags & 1 == 0
                        && p.on_ground
                        && brushes.iter().any(|b| Self::player_in(&below, b, 0.01))
                    {
                        self.players[k].base_velocity += dir * speed;
                    }
                }
                _ => {
                    if brushes.iter().any(|b| Self::player_in(p, b, 0.0)) {
                        self.touch_trigger(id, k);
                    }
                }
            }
            if self.players[k].teleported {
                return;
            }
        }
    }

    fn touch_solid(&mut self, id: Id, k: usize) {
        match &self.ents[id].kind {
            Kind::Door(_) => self.door_touch(id, k),
            Kind::Button(_) => self.button_touch(id, k),
            Kind::Breakable(_) => {
                let flags = self.ents[id].flags;
                let p = &self.players[k];
                let speed = p.velocity.length();
                let (_, hi) = self.bounds(id);
                let on_top = (p.origin.z + p.mins.z - hi.z).abs() <= 2.0;
                let health = match &self.ents[id].kind {
                    Kind::Breakable(b) => b.health,
                    _ => return,
                };
                if flags & 2 != 0 && speed * TOUCH_FACTOR >= health {
                    let (pid, dmg) = (p.id, speed * TOUCH_FACTOR);
                    let dir = p.velocity.normalize_or_zero();
                    self.break_now(id, dir);
                    self.effects.push(Effect::Damage { player: pid, points: dmg / 4.0, kind: DamageKind::Melee, by: Some(id) });
                } else if flags & 4 != 0 && on_top && self.ents[id].think_at.is_none() {
                    let delay = if self.ents[id].delay == 0.0 { 0.1 } else { self.ents[id].delay };
                    self.ents[id].think_at = Some(self.time + f64::from(delay));
                }
            }
            _ => {}
        }
    }

    fn touch_trigger(&mut self, id: Id, k: usize) {
        let now = self.time;
        let flags = self.ents[id].flags;
        let pid = self.players[k].id;
        match &mut self.ents[id].kind {
            Kind::Trigger(t) => {
                // Players fire it unless "No Clients".
                if flags & 2 != 0 || t.spent || now + 1e-9 < t.armed_at {
                    return;
                }
                if !self.master_ok(&self.ents[id].master) {
                    return;
                }
                let Kind::Trigger(t) = &mut self.ents[id].kind else { return };
                if t.wait > 0.0 {
                    t.armed_at = now + f64::from(t.wait);
                } else {
                    t.spent = true;
                    self.ents[id].remove_at = Some(now + REMOVE_DELAY);
                }
                self.fire_targets(id, Some(Who::Player(pid)), UseType::Toggle);
                let message = self.ents[id].message.clone();
                if !message.is_empty() {
                    self.effects.push(Effect::Message { player: pid, text: message });
                }
            }
            Kind::Hurt(h) => {
                if !h.on || flags & 8 != 0 {
                    return;
                }
                let window = h.window.get_or_insert((now, Vec::new()));
                if now >= window.0 + HURT_INTERVAL - 1e-9 {
                    *window = (now, Vec::new());
                }
                if window.1.contains(&pid) {
                    return;
                }
                window.1.push(pid);
                let (points, kind) = (h.dmg * HURT_INTERVAL as f32, h.kind);
                if !self.master_ok(&self.ents[id].master) {
                    return;
                }
                self.effects.push(Effect::Damage { player: pid, points, kind, by: Some(id) });
                if !self.ents[id].target.is_empty() {
                    self.fire_targets(id, Some(Who::Player(pid)), UseType::Toggle);
                    if flags & 1 != 0 {
                        self.ents[id].target.clear();
                    }
                }
            }
            Kind::Push { speed, dir, on } => {
                if !*on {
                    return;
                }
                let v = *dir * *speed;
                if flags & 1 != 0 {
                    let p = &mut self.players[k];
                    p.velocity += v;
                    p.velocity_set = true;
                    if p.velocity.z > 0.0 {
                        p.on_ground = false;
                    }
                    self.remove(id);
                } else {
                    self.players[k].base_velocity += v;
                }
            }
            Kind::Teleport => {
                if flags & 2 != 0 || !self.master_ok(&self.ents[id].master) {
                    return;
                }
                let target = self.ents[id].target.clone();
                let Some(&dest) = self.named(&target).first() else { return };
                let (at, angles) = (self.ents[dest].origin, self.ents[dest].angles);
                let p = &mut self.players[k];
                let half = -p.mins.z;
                p.origin = at + Vec3::Z * (half + TELEPORT_LIFT);
                p.view = angles;
                p.velocity = Vec3::ZERO;
                p.velocity_set = true;
                p.view_set = true;
                p.teleported = true;
                p.moved = true;
                p.on_ground = false;
            }
            _ => {}
        }
    }

    /// Player `k` pressed `+use`: the usable entity most in front of the
    /// view within reach (module doc).
    fn player_use(&mut self, k: usize) {
        let p = self.players[k].clone();
        let eye = p.origin + p.eye;
        let fwd = forward(p.view);
        let mut best: Option<(f32, Id)> = None;
        for id in 0..self.ents.len() {
            if !self.usable(id) {
                continue;
            }
            let (lo, hi) = self.bounds(id);
            let nearest = p.origin.clamp(lo, hi);
            if nearest.distance(p.origin) > USE_RADIUS {
                continue;
            }
            let half = (hi - lo) / 2.0;
            let d = (lo + hi) / 2.0 - eye;
            let los = Vec3::new(shrink(d.x, half.x), shrink(d.y, half.y), shrink(d.z, half.z));
            let dot = los.normalize_or_zero().dot(fwd);
            if dot > USE_CONE && best.is_none_or(|(b, _)| dot > b) {
                best = Some((dot, id));
            }
        }
        let sound = if best.is_some() { "common/wpn_select.wav" } else { "common/wpn_denyselect.wav" };
        self.effects.push(Effect::Sound { path: sound.into(), at: p.origin, entity: None, volume: 0.4 });
        if let Some((_, id)) = best {
            self.use_entity(id, Some(Who::Player(p.id)), None, UseType::Set(1.0));
        }
    }

    /// Whether `+use` finds entity `id`.
    fn usable(&self, id: Id) -> bool {
        let e = &self.ents[id];
        if e.removed {
            return false;
        }
        match &e.kind {
            Kind::Button(b) => !b.shootable && e.flags & 256 == 0,
            Kind::Pushable(_) => true,
            Kind::Door(_) => e.flags & 256 != 0,
            _ => false,
        }
    }

    /// The origin a player or entity acts from, and its view (rotating
    /// doors' direction).
    pub fn activator_pose(&self, who: Who) -> Option<(Vec3, Vec3)> {
        match who {
            Who::Player(id) => self.players.iter().find(|p| p.id == id).map(|p| (p.origin, p.view)),
            Who::Ent(id) => self.ents.get(id).map(|e| (e.origin, e.angles)),
        }
    }
}

/// A component of the eye-to-box vector shortened by the box's half
/// size (0 inside).
fn shrink(d: f32, half: f32) -> f32 {
    d.signum() * (d.abs() - half).max(0.0)
}

/// A sound key for entity `id`'s channel `channel` (0 a mover's moving
/// sound, 1 an ambient sound).
pub fn sound_key(id: Id, channel: u64) -> u64 {
    (1 << 61) | ((id as u64) << 2) | channel
}

fn breakable(e: &MapEntity) -> Breakable {
    let material = tables::Material::of(num(e, "material") as i32);
    let gibs = key(e, "gibmodel").filter(|g| !g.is_empty()).map_or_else(|| material.gibs().to_string(), str::to_string);
    Breakable {
        health: num(e, "health"),
        material,
        directed: num(e, "explosion") as i32 == 1,
        gibs,
        magnitude: num(e, "explodemagnitude"),
        pressure_at: None,
    }
}
