//! The object world (specs/combat_arms/world_objects.md): every map
//! object the server runs, built from the map's entities (`game::world`
//! passes each object through with its properties; an object's id is its
//! index in `MapEntities`), the command manager (`commands`), message
//! delivery and each class's behaviour. Plain data, driven frame by frame
//! by the ECS bridge (`objects`), which hands it the characters and reads
//! back poses, flags and effects (`Fx`). Everything is in the world
//! file's frame and units (`geometry`).
//!
//! - Messages are delivered synchronously (`deliver`): every object takes
//!   the base messages (`VISIBLE`, `SOLID`, `HIDDEN`, `REMOVE`,
//!   `MEMORIZE`, `REMEMBER`, `NEXT_UPDATE`); world models `ATTACH`,
//!   `DETACH`, `DESTROY`, the damage switches; active world models and
//!   doors their machine's (`movers::Awm`); keyframers, triggers, command
//!   and touch triggers, event counters, explosions theirs; volumes
//!   `ON`/`OFF`; mode objects and controllers are handed to the rules
//!   (`Fx::ObjectMessage`).
//! - A frame (`frame`): the command manager (delays by the frame's
//!   time), then each object whose update is due, in map order.
//! - Characters touch triggers (`touch`), use doors (`use_object`) and
//!   damage world models (`damage`) before the frame.
//!
//! Corrections (2026-10-22) readings: the nudge of switching a volume on is
//! 10 u along the normalised (1, 1, 1); a first `VISIBLE`/`SOLID` message
//! records its own value as what the object "had".
//!
//! Sounds (14) are state: each object's current sound and last placed
//! one-shot (`ObjSound`), which `sound` plays; a keyframer's
//! `ActiveSound` loops on its first object while it is active (the
//! sound is kept through `PAUSE`: the spec stops it only when inactive).
//!
//! Known shortcuts: `SoundFX` keeps its on/off state and restart count (the
//! client's `music::ambient` plays them); `LightGroup` keeps its on/off
//! state and colour, which the bridge shows as the group's light style
//! (`lights`); `SpecialFX` keeps its on/off state only; the event
//! objects (timers, use spots, captions, messages, the objective list,
//! cameras, `TargetTrigger` markers) are `events`'s;
//! `BGMTrigger` only reports its `ON`/`OFF` (`Fx::Music`, `music::trigger`);
//! breakables' `Spawn`, prop debris, `WorldModelDebris` and
//! `CreateExplosion` aren't made (only the death command and the
//! object's removal or hiding); bodies don't fire triggers; `PropType`
//! and `Spawner` take the base messages only; the `ActivateTrigger`
//! use-time gauge is the object world's (the modes' gauges are their
//! own).

use std::collections::HashMap;

use bevy::prelude::*;

use super::{
    commands::{self, CommandHost, CommandState, Ctx, ObjId, Who, statements, tokens},
    events,
    geometry::{Waveform, rotation},
    keyframer::{self, Keyframer},
    movers::{self, Awm, Blocking, CharBox, Frame, Motion, Out, Pose, State},
};

/// Default `SoundRadius` of active world models and keyframers (u).
pub const AWM_SOUND_RADIUS: f32 = 1000.0;
/// Default trigger half-extents (trigger_dims, u).
pub const TRIGGER_DIMS: Vec3 = Vec3::splat(16.0);
/// A timed or attached trigger's update period (s).
pub const TRIGGER_TIMED_PERIOD: f32 = 0.1;
/// Default `SoundRadius` of a trigger's `ActivationSound` (u).
pub const TRIGGER_SOUND_RADIUS: f32 = 200.0;
/// Ours: how close (u) an AI character's box comes to a door it opens by
/// walking into it.
pub const AI_DOOR_REACH: f32 = 8.0;
/// How far (u) switching a volume on moves the players and back
/// (Corrections (2026-10-22) 16).
pub const NUDGE: f32 = 10.0;
/// A destructible dies at or below this many hit points.
pub const BREAKABLE_DEATH_HP: f32 = 1.0;

/// The active world-model classes.
pub const ACTIVE_CLASSES: &[&str] = &[
    "SlidingWorldModel",
    "RotatingWorldModel",
    "SpinningWorldModel",
    "SlidingDoor",
    "RotatingDoor",
    "RotatingSwitch",
    "Breakable",
    "Door",
    "Switch",
    "SlidingSwitch",
    "ActiveWorldModel",
];

/// Classes whose messages the rules (game modes) read.
pub const RULE_CLASSES: &[&str] = &[
    "BombZoneTrigger",
    "FlagZoneTrigger",
    "EscapeZoneTrigger",
    "BullionSaveTrigger",
    "ActivateTrigger",
    "TDMGameControl",
    "FlagItem",
    "GameStartPoint",
];

/// Map-entity keys `game::world` adds to a world model's object.
pub mod keys {
    /// Half-extents of its BSP's box (entity axes, u).
    pub const HALF: &str = "chimera_half";
    /// Whether its world model collides and stops rays (1/0, the
    /// conversion's reading of `Solid` and `Rayhit`).
    pub const SOLID: &str = "chimera_solid";
    pub const RAYHIT: &str = "chimera_rayhit";
    /// A keyframer's keys chunk, hex.
    pub const KEYS: &str = "chimera_keys";
    /// A `LightGroup`'s Chimera light style (`game::world`: its group's
    /// share of the lightmaps).
    pub const LIGHT_STYLE: &str = "chimera_light_style";
}

/// A map entity's keyvalues and, for a world model or a volume, its box
/// centre (file frame): what `ObjectWorld::build` takes per object.
pub type EntityProps = (Vec<(String, String)>, Option<Vec3>);

/// A map object's properties (as the map entity carries them).
#[derive(Clone, Debug, Default)]
pub struct Props(pub Vec<(String, String)>);

impl Props {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str())
    }
    pub fn string(&self, key: &str) -> String {
        self.get(key).unwrap_or("").to_string()
    }
    pub fn real(&self, key: &str) -> Option<f32> {
        self.get(key).and_then(|v| v.trim().parse().ok())
    }
    pub fn real_or(&self, key: &str, default: f32) -> f32 {
        self.real(key).unwrap_or(default)
    }
    pub fn bool(&self, key: &str) -> Option<bool> {
        self.get(key).map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.parse::<f32>().is_ok_and(|x| x != 0.0)
        })
    }
    pub fn bool_or(&self, key: &str, default: bool) -> bool {
        self.bool(key).unwrap_or(default)
    }
    /// A vector property in the file's frame (map entities carry them in
    /// entity axes: x, z, y of the file's).
    pub fn vector(&self, key: &str) -> Option<Vec3> {
        let v = crate::map::entities::parse_vector(self.get(key)?);
        Some(Vec3::new(v.x, v.z, v.y))
    }
    /// A rotation property (pitch, yaw, roll, radians).
    pub fn angles(&self, key: &str) -> Option<Vec3> {
        self.get(key).map(crate::map::entities::parse_vector)
    }
    /// A property by its exact name (case kept): for names that differ
    /// from a map entity key only by case (`TargetName` is not
    /// `targetname`, the object's `Name`).
    pub fn exact(&self, key: &str) -> String {
        self.0.iter().find(|(k, _)| k == key).map_or(String::new(), |(_, v)| v.clone())
    }
    /// The first present of several names.
    fn first(&self, names: &[&str]) -> Option<&str> {
        names.iter().find_map(|n| self.get(n))
    }
}

/// An effect for the bridge to apply.
#[derive(Clone, Debug, PartialEq)]
pub enum Fx {
    /// A server 3D sound: at a point, or attached to an object; `looping`
    /// until the object's next sound.
    Sound {
        file: String,
        at: Vec3,
        attach: Option<ObjId>,
        radius: f32,
        inner: f32,
        looping: bool,
        owner: Option<ObjId>,
    },
    /// Stop the looping transition sound an object started (an active
    /// world model's: Corrections (2026-10-22) 3).
    StopSound { owner: ObjId },
    /// A `BGMTrigger` got `ON` or `OFF` (the bridge checks its mode and
    /// tells the clients).
    Music { id: ObjId, on: bool },
    /// A map explosion (`Explosion`): area damage like a grenade's blast.
    Explode {
        at: Vec3,
        radius: f32,
        damage: f32,
        effect: String,
        attacker: Who,
    },
    /// A character carried by a moving world model: its new box minimum.
    Carry { who: Who, delta: Vec3 },
    /// A message for the rules (mode objects and controllers).
    ObjectMessage {
        id: ObjId,
        class: String,
        name: String,
        message: String,
        sender: Who,
    },
    /// A message to a character (`msg player ...`).
    CharacterMessage { who: Who, message: String },
    /// What every client is told about an event object (`events`).
    Notice { id: ObjId, notice: events::Notice },
    /// A destructible's death sends its prop debris (effects_sounds.md
    /// Follow-up (2026-10-31b) 1): at its box centre, record `debris`
    /// (`DebrisType`, by name), facing the death direction.
    Debris { at: Vec3, debris: String, dir: Vec3 },
    /// A destructible fires its weapon (`CreateExplosion` at death, `FIRE`):
    /// weapon `weapon` (the game's id) from `at` along `dir`, going off in
    /// place when `in_place`, damage x `factor`, by object `shooter`.
    Fire { weapon: i32, at: Vec3, dir: Vec3, in_place: bool, factor: f32, shooter: ObjId },
    /// A line for the console's log.
    Log(String),
}

/// The destructible aggregate (11).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Destructible {
    pub hp: f32,
    pub max: f32,
    pub can_damage: bool,
    pub can_player_damage: bool,
    pub never_destroy: bool,
    pub dead: bool,
    pub damage_command: String,
    pub player_damage_command: String,
    pub trigger_counter: i32,
    pub num_sends: i32,
    pub percent_command: String,
    pub percent_for: f32,
    pub hp_commands: Vec<(String, f32, f32, bool)>,
    pub death_command: String,
    pub player_death_command: String,
    pub remove_on_death: bool,
    /// `HIDDEN 1` saved the damage switches.
    pub hidden_saved: Option<(bool, bool)>,
}

/// A `Trigger` (also a `TargetTrigger`'s).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trigger {
    pub dims: Vec3,
    pub n_activations: u32,
    pub send_delay: f32,
    pub trigger_delay: f32,
    pub commands: Vec<String>,
    pub trigger_touch: bool,
    pub command_touch: String,
    pub player_triggerable: bool,
    pub weighted: bool,
    pub weight1: f32,
    pub timed: bool,
    pub min_time: f32,
    pub max_time: f32,
    pub activation_count: u32,
    pub locked: bool,
    pub sound: String,
    pub sound_radius: f32,
    pub attach_to: String,
    pub player_once: bool,
    // State.
    pub last_attempt: f64,
    pub toucher: Who,
    pub by_touch: bool,
    pub pending: Option<f64>,
    pub counter: u32,
    pub fired: u32,
    pub next_timed: f64,
    pub started: bool,
    pub countdown: f32,
    /// `TargetTrigger`: shown on the radar (`ON`/`OFF`).
    pub shown: bool,
    /// `TargetTrigger`: its marker (team, game type).
    pub marker: Option<events::Marker>,
}

/// A generic `ActivateTrigger`'s use (game_modes.md M10): a player holding
/// use inside it for `TimeAmount` runs `ActivateCommand`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UseTrigger {
    pub dims: Vec3,
    pub time: f32,
    pub locked: bool,
    pub work: String,
    pub release: String,
    pub activate: String,
    pub user: Option<(Who, f32)>,
}

/// What an object is.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Kind {
    #[default]
    Plain,
    WorldModel,
    Active(Box<Awm>),
    Trigger(Box<Trigger>),
    CommandTrigger { locked: bool, commands: [String; 4] },
    TouchTrigger { dims: Vec3, locked: bool, limit: u32, fired: u32, commands: [String; 4] },
    Use(Box<UseTrigger>),
    EventCounter { value: i32, locked: bool, events: Vec<(i32, String, String)> },
    StartupCommand { command: String },
    KeyFramer(Box<Keyframer>),
    Explosion { radius: f32, damage: f32, effect: String, remove: bool },
    Volume,
    /// `SoundFX`, `Camera`, `SpecialFX`: on or off; `plays`
    /// counts how often it was (re)started (a sound plays again from its
    /// start on each: music_ambience.md 6).
    Switchable { on: bool, plays: u32 },
    /// `BGMTrigger` (music_ambience.md 5): on or off by its messages; the
    /// music itself is the bridge's (`Fx::Music`).
    MusicTrigger { on: bool },
    /// `LightGroup` (world_dat.md "Light groups"): on or off and its
    /// colour (0..1); what it shows is its colour when on, black when off
    /// (`shown`).
    LightGroup { on: bool, color: Vec3 },
    /// The event objects (`events`).
    Timer(Box<events::Timer>),
    LightUse(Box<events::LightUse>),
    Caption { showtime: f32, locked: bool },
    MessageTrigger { showtime: f32 },
    Mission(Vec<events::Objective>),
    Camera(Box<events::Camera>),
}

/// A `LightGroup`'s state at the map's start: `StartOn` (true) and
/// `StartColor`/255 (white).
pub fn light_group_start(p: &Props) -> Kind {
    // Colours are "r g b"; `vector` reads entity axes (x z y).
    let c = p.vector("StartColor").map_or(Vec3::ONE, |c| Vec3::new(c.x, c.z, c.y) / 255.0);
    Kind::LightGroup {
        on: p.bool_or("StartOn", true),
        color: c,
    }
}

impl Kind {
    /// A light group's effective colour: its colour if on, black if off
    /// (a colour that truncates to 0 at 8 bits counts as off).
    pub fn shown(&self) -> Option<Vec3> {
        match self {
            Kind::LightGroup { on, color } => {
                let c = if *on { *color } else { Vec3::ZERO };
                Some(if (c * 255.0).to_array().iter().any(|x| *x as u32 > 0) { c } else { Vec3::ZERO })
            }
            _ => None,
        }
    }
}

/// A sound an object plays (14): its transition sound or a keyframer's
/// active sound (`Object::sound`), or a placed one-shot (`Object::shot`:
/// a trigger's activation sound, a key's sound). Kept while it may still
/// sound, so a machine that starts after (a late joiner) learns of a
/// loop; `serial` tells one start from the next.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjSound {
    pub file: String,
    /// Where it started (file frame, u).
    pub at: Vec3,
    /// The object it follows (an active world model's with `SoundPos`
    /// exactly (0, 0, 0), a keyframer's first object), else it stays at
    /// `at`.
    pub attach: Option<ObjId>,
    /// Outer and inner radius (u).
    pub radius: f32,
    pub inner: f32,
    pub looping: bool,
    pub serial: u32,
}

/// What `MEMORIZE` keeps (the world-model and active layers).
#[derive(Clone, Debug, PartialEq)]
pub struct Memo {
    pub pose: Pose,
    pub flags: (bool, bool, bool),
    pub awm: Option<Box<Awm>>,
}

/// An object attached to another (1, Attachments).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attached {
    pub id: ObjId,
    pub offset: Vec3,
    pub rot: Quat,
}

/// What messages changed in a volume (world_dat.md Corrections
/// (2026-10-22) 3 and 6): the bridge writes them to its `CaVolume`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VolumeMods {
    /// `DamageVal`: the new `Damage`.
    pub damage: Option<f32>,
    /// `VELOCITY` (a jump pad's launch speed, u/s).
    pub speed: Option<f32>,
    /// `LOCK` / `UNLOCK` (a `DamageVolume`).
    pub locked: Option<bool>,
}

/// One map object.
#[derive(Clone, Debug, Default)]
pub struct Object {
    /// A volume's changes by messages.
    pub vol: VolumeMods,
    pub class: String,
    pub name: String,
    pub props: Props,
    pub pose: Pose,
    pub load_pose: Pose,
    pub visible: bool,
    pub solid: bool,
    pub rayhit: bool,
    pub removed: bool,
    /// The flags it had at its first message.
    pub first: Option<(bool, bool, bool)>,
    pub memo: Option<Memo>,
    /// It has a world-model node (drawn and collided by itself).
    pub node: bool,
    pub hp: Option<Destructible>,
    pub damage_on: bool,
    pub player_damage_on: bool,
    pub attached: Vec<Attached>,
    pub parent: Option<ObjId>,
    pub kind: Kind,
    /// The sound it plays now: its transition sound (looping until the
    /// next transition with `LoopSounds`, else the last one-shot), a
    /// keyframer's `ActiveSound` (14).
    pub sound: Option<ObjSound>,
    /// Its last placed one-shot (a trigger's `ActivationSound`, a key's
    /// sound).
    pub shot: Option<ObjSound>,
    /// Pose or flags changed since the bridge last looked.
    pub dirty: bool,
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            pos: Vec3::ZERO,
            rot: Quat::IDENTITY,
            half: Vec3::ZERO,
        }
    }
}

impl Object {
    fn flags(&self) -> (bool, bool, bool) {
        (self.visible, self.solid, self.rayhit)
    }

    /// Whether it's a world model (has a box of its own).
    pub fn is_world_model(&self) -> bool {
        matches!(self.kind, Kind::WorldModel | Kind::Active(_))
    }

    pub fn awm(&self) -> Option<&Awm> {
        match &self.kind {
            Kind::Active(a) => Some(a),
            _ => None,
        }
    }
}

/// A small deterministic generator (the rules' draws are any uniform
/// one: game_modes.md S16).
#[derive(Clone, Debug)]
pub struct Rng(pub u64);

impl Rng {
    pub fn roll(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }
}

/// The object world.
#[derive(Clone, Debug)]
pub struct ObjectWorld {
    pub objects: Vec<Object>,
    pub names: HashMap<String, Vec<ObjId>>,
    pub cmds: CommandState,
    pub chars: Vec<CharBox>,
    pub now: f64,
    pub dt: f32,
    pub frames: u64,
    pub fx: Vec<Fx>,
    pub rng: Rng,
    /// The last character that sent any trigger message.
    pub active_player: Who,
    /// `PlayerOnceActivate`'s list, shared by every trigger.
    pub once: Vec<Who>,
    /// The last sound start's serial (`ObjSound::serial`).
    pub sound_serial: u32,
    /// Lines for the console's log (built while loading).
    pub log: Vec<String>,
    /// The counted objectives' maxima by id (the co-op mission data for
    /// the difficulty: `MissionTrigger`'s `ADD_POINT_MISSION`).
    pub mission_points: std::collections::BTreeMap<String, i32>,
}

impl Default for ObjectWorld {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            names: HashMap::new(),
            cmds: CommandState::default(),
            chars: Vec::new(),
            now: 0.0,
            dt: 0.05,
            frames: 0,
            fx: Vec::new(),
            rng: Rng(0x5eed),
            active_player: Who::Nobody,
            once: Vec::new(),
            sound_serial: 0,
            log: Vec::new(),
            mission_points: Default::default(),
        }
    }
}

fn four(p: &Props, prefix: &str, first: usize) -> [String; 4] {
    std::array::from_fn(|i| p.string(&format!("{prefix}{}", first + i)))
}

/// Parse "x y z" words (a message's arguments) as a vector.
fn words_vector(w: &[String]) -> Vec3 {
    let n = |i: usize| w.get(i).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
    Vec3::new(n(0), n(1), n(2))
}

/// `0`/`1` or `FALSE`/`TRUE`.
fn flag(w: Option<&String>) -> Option<bool> {
    let w = w?;
    if w.eq_ignore_ascii_case("true") {
        return Some(true);
    }
    if w.eq_ignore_ascii_case("false") {
        return Some(false);
    }
    w.parse::<f32>().ok().map(|x| x != 0.0)
}

/// A world model's machine from its class and properties.
fn awm(class: &str, p: &Props, pose: &Pose) -> Awm {
    let door = class.ends_with("Door") || class == "Door";
    let switch = class.ends_with("Switch");
    let spinning = class == "SpinningWorldModel";
    let motion = match p.real("AWMType").map(|x| x as i32) {
        Some(1) => Motion::Sliding,
        Some(2) if spinning => Motion::Spinning,
        Some(2) => Motion::Rotating,
        Some(0) => Motion::Static,
        _ => match class {
            c if c.starts_with("Sliding") => Motion::Sliding,
            "SpinningWorldModel" => Motion::Spinning,
            c if c.starts_with("Rotating") => Motion::Rotating,
            _ => Motion::Static,
        },
    };
    let motion = if spinning { Motion::Spinning } else { motion };
    let b = |door_name: &str, base: &str, default: bool| {
        if door {
            p.bool(door_name).or_else(|| p.bool(base)).unwrap_or(default)
        } else {
            p.bool(base).unwrap_or(default)
        }
    };
    let r = |door_name: &str, base: &str, default: f32| {
        if door {
            p.real(door_name).or_else(|| p.real(base)).unwrap_or(default)
        } else {
            p.real(base).unwrap_or(default)
        }
    };
    let s = |door_name: &str, base: &str| {
        if door { p.first(&[door_name, base]).unwrap_or("").to_string() } else { p.string(base) }
    };
    let activates = door || switch;
    let (dir_default, dist_default, angles_default) = if door {
        (Vec3::Y, 64.0, Vec3::new(0.0, 90.0, 0.0))
    } else if switch {
        (Vec3::Z, 8.0, Vec3::new(0.0, 90.0, 0.0))
    } else if spinning {
        (Vec3::ZERO, 0.0, Vec3::new(0.0, 5.0, 0.0))
    } else {
        (Vec3::ZERO, 0.0, Vec3::ZERO)
    };
    let q = p.angles("Rotation").map_or(Quat::IDENTITY, |a| rotation(a.x, a.y, a.z));
    let mut move_dir = p.vector("MoveDir").unwrap_or(dir_default);
    if move_dir.length_squared() < 0.0001 {
        move_dir = Vec3::Y;
    }
    // Pitch, yaw, roll in degrees.
    let deg = p.vector("RotationAngles").unwrap_or(angles_default);
    let (stored, spin_final) = if motion == Motion::Spinning {
        let w = Vec3::new(
            if deg.x != 0.0 { std::f32::consts::TAU / deg.x } else { 0.0 },
            if deg.y != 0.0 { std::f32::consts::TAU / deg.y } else { 0.0 },
            if deg.z != 0.0 { std::f32::consts::TAU / deg.z } else { 0.0 },
        );
        let on = Vec3::new(
            if deg.x != 0.0 { 359.9 } else { 0.0 },
            if deg.y != 0.0 { 359.9 } else { 0.0 },
            if deg.z != 0.0 { 359.9 } else { 0.0 },
        );
        (on * std::f32::consts::PI / 180.0, w)
    } else {
        (deg * std::f32::consts::PI / 180.0, Vec3::ZERO)
    };
    let (power_on, power_off) = if spinning { (100.0, 0.0) } else { (1.0, 0.0) };
    Awm {
        motion,
        door,
        player_activate: b("PlayerActivate", "PlayerActivate", activates),
        ai_activate: b("AIActivate", "AIActivate", activates),
        trigger_off: b("TriggerClose", "TriggerOff", true),
        remain_on: b("RemainOpen", "RemainOn", true),
        force_move: p.bool_or("ForceMove", false),
        force_on: p.bool_or("ForceMoveOn", false),
        force_off: p.bool_or("ForceMoveOff", false),
        locked: p.bool_or("Locked", false),
        rotate_away: b("OpenAway", "RotateAway", !(switch || spinning)),
        waveform: Waveform::named(p.get("Waveform").unwrap_or("Linear")),
        sounds: [
            s("OpeningSound", "PowerOnSound"),
            s("OpenSound", "OnSound"),
            s("ClosingSound", "PowerOffSound"),
            s("ClosedSound", "OffSound"),
            p.string("LockedSound"),
        ],
        sound_pos: p.vector("SoundPos").unwrap_or(Vec3::ZERO),
        sound_radius: p.real_or("SoundRadius", AWM_SOUND_RADIUS),
        loop_sounds: p.bool_or("LoopSounds", false),
        commands: [
            s("OpeningCommand", "PowerOnCommand"),
            s("OpenCommand", "OnCommand"),
            s("ClosingCommand", "PowerOffCommand"),
            s("ClosedCommand", "OffCommand"),
            p.string("LockedCommand"),
        ],
        activate_type: p.get("ActivateType").unwrap_or(if door { "OpenClose" } else { "<none>" }).to_string(),
        dir: (q * move_dir).normalize_or_zero(),
        dist: p.real_or("MoveDist", dist_default),
        q,
        orig: pose.pos,
        on_pos: pose.pos,
        off_pos: pose.pos,
        pivot: p.vector("Pos").unwrap_or(pose.pos),
        stored_angles: stored,
        on_angles: stored,
        stored_dirs: movers::directions(stored),
        dirs: movers::directions(stored),
        power_on_time: r("OpeningTime", "PowerOnTime", power_on),
        power_off_time: r("ClosingTime", "PowerOffTime", power_off),
        move_delay: p.real_or("MoveDelay", 0.0),
        on_wait: r("OpenWaitTime", "OnWaitTime", 0.0),
        off_wait: r("CloseWaitTime", "OffWaitTime", 0.0),
        multiplier: 1.0,
        door_link: p.string("DoorLink"),
        spin: movers::Spin {
            omega_final: spin_final,
            ..default()
        },
        ..default()
    }
}

/// The destructible properties (11).
fn destructible(class: &str, p: &Props) -> Destructible {
    let mut hp_commands = Vec::new();
    for i in 1..=3 {
        let c = p.string(&format!("HitPointCommand{i}"));
        if !c.is_empty() {
            hp_commands.push((
                c,
                p.real_or(&format!("MinHitPoints{i}"), 0.0),
                p.real_or(&format!("MaxHitPoints{i}"), 0.0),
                false,
            ));
        }
    }
    let max = p.real_or("MaxHitPoints", 100.0);
    Destructible {
        hp: p.real_or("HitPoints", 100.0),
        max,
        can_damage: p.bool_or("CanDamage", true),
        can_player_damage: p.bool_or("CanPlayerDamage", true),
        never_destroy: p.bool_or("NeverDestroy", class != "Breakable"),
        dead: false,
        damage_command: p.string("DamageCommand"),
        player_damage_command: p.string("PlayerDamageCommand"),
        trigger_counter: p.real_or("DamageTriggerCounter", 0.0) as i32,
        num_sends: p.real_or("DamageTriggerNumSends", 1.0) as i32,
        percent_command: p.string("DamagePercentCommand"),
        percent_for: p.real_or("DamagePercentForCommand", 0.5),
        hp_commands,
        death_command: p.string("DeathCommand"),
        player_death_command: p.string("PlayerDeathCommand"),
        remove_on_death: p.bool_or("RemoveOnDeath", true),
        hidden_saved: None,
    }
}

impl ObjectWorld {
    /// The object world of a map's entities (`MapEntity` keyvalues as
    /// `Props`, in map order; `centres[i]` a world model's or volume's
    /// box centre in the file frame, when it has one).
    pub fn build(entities: &[EntityProps]) -> Self {
        let mut w = Self::default();
        for (i, (kv, centre)) in entities.iter().enumerate() {
            let p = Props(kv.clone());
            let class = p.string("classname");
            let name = p.string("targetname");
            let pos = centre.or_else(|| p.vector("origin")).unwrap_or(Vec3::ZERO);
            let half = p.vector(keys::HALF).unwrap_or(Vec3::ZERO);
            let half = Vec3::new(half.x, half.y, half.z).abs();
            let pose = Pose {
                pos,
                rot: if centre.is_some() {
                    Quat::IDENTITY
                } else {
                    p.angles("Rotation").map_or(Quat::IDENTITY, |a| rotation(a.x, a.y, a.z))
                },
                half,
            };
            let node = p.get(keys::SOLID).is_some();
            let solid = p.bool_or(keys::SOLID, false);
            let rayhit = p.bool_or(keys::RAYHIT, false);
            let visible = p.bool_or("Visible", true);
            let mut kind = Kind::Plain;
            if node {
                kind = Kind::WorldModel;
            }
            if ACTIVE_CLASSES.contains(&class.as_str()) {
                kind = Kind::Active(Box::new(awm(&class, &p, &pose)));
            }
            match class.as_str() {
                "Trigger" | "TargetTrigger" => {
                    let commands = (1..=10).map(|k| p.string(&format!("Command{k}"))).collect();
                    let target = class == "TargetTrigger";
                    kind = Kind::Trigger(Box::new(Trigger {
                        dims: p.vector("Dims").map(|d| d.abs()).unwrap_or(TRIGGER_DIMS),
                        n_activations: p.real_or("NumberOfActivations", 1.0).max(0.0) as u32,
                        send_delay: p.real_or("SendDelay", 0.0),
                        trigger_delay: p.real_or("TriggerDelay", 0.0),
                        commands,
                        trigger_touch: p.bool_or("TriggerTouch", false),
                        command_touch: p.string("CommandTouch"),
                        player_triggerable: p.bool_or("PlayerTriggerable", true),
                        weighted: p.bool_or("WeightedTrigger", false),
                        weight1: p.real_or("Message1Weight", 0.5),
                        timed: p.bool_or("TimedTrigger", false),
                        min_time: p.real_or("MinTriggerTime", 0.0),
                        max_time: p.real_or("MaxTriggerTime", 10.0),
                        activation_count: p.real_or("ActivationCount", 1.0).max(1.0) as u32,
                        locked: p.bool_or("Locked", false),
                        sound: p.string("ActivationSound"),
                        sound_radius: p.real_or("SoundRadius", TRIGGER_SOUND_RADIUS),
                        attach_to: p.string("AttachToObject"),
                        player_once: p.bool_or("PlayerOnceActivate", false),
                        last_attempt: f64::NEG_INFINITY,
                        marker: target.then(|| events::Marker::of(&p)),
                        ..default()
                    }));
                }
                "CommandTrigger" => {
                    kind = Kind::CommandTrigger {
                        locked: p.bool_or("Locked", false),
                        commands: four(&p, "Command", 0),
                    }
                }
                "TouchTrigger" => {
                    kind = Kind::TouchTrigger {
                        dims: p.vector("Dims").map(|d| d.abs()).unwrap_or(TRIGGER_DIMS),
                        locked: p.bool_or("Locked", false),
                        limit: p.real_or("NumberOfActivations", 1.0).max(0.0) as u32,
                        fired: 0,
                        commands: four(&p, "Command", 0),
                    }
                }
                "ActivateTrigger" => {
                    kind = Kind::Use(Box::new(UseTrigger {
                        dims: p.vector("Dims").map(|d| d.abs()).unwrap_or(TRIGGER_DIMS),
                        time: p.real_or("TimeAmount", 0.0),
                        locked: p.bool_or("Locked", false),
                        work: p.string("WorkCommand"),
                        release: p.string("ReleaseCommand"),
                        activate: p.string("ActivateCommand"),
                        user: None,
                    }))
                }
                "EventCounter" => {
                    let events = (0..=12)
                        .filter_map(|n| {
                            let v = p.real(&format!("Event{n}Value"))? as i32;
                            (v != -1).then(|| {
                                (
                                    v,
                                    p.string(&format!("Event{n}IncToValCmd")),
                                    p.string(&format!("Event{n}DecToValCmd")),
                                )
                            })
                        })
                        .collect();
                    kind = Kind::EventCounter {
                        value: p.real_or("StartingValue", 0.0) as i32,
                        locked: false,
                        events,
                    }
                }
                "StartupCommand" => {
                    kind = Kind::StartupCommand {
                        command: p.string("Command"),
                    }
                }
                "KeyFramer" => {
                    let keys = p
                        .get(keys::KEYS)
                        .and_then(hex)
                        .and_then(|b| keyframer::parse_keys(&b))
                        .unwrap_or_default();
                    kind = Kind::KeyFramer(Box::new(Keyframer {
                        keys,
                        base: p.string("BaseKeyName"),
                        object_names: p
                            .string("ObjectName")
                            .split(';')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect(),
                        looping: p.bool_or("Looping", false),
                        align: p.bool_or("AlignToPath", false),
                        ignore_offsets: p.bool_or("IgnoreOffsets", false),
                        push: p.bool_or("PushObjects", false),
                        total_time: p.real_or("TotalPathTime", 0.0),
                        wave: Waveform::named(p.get("Wavetype").unwrap_or("Linear")),
                        // (Not `string`: that finds `targetname`, its own
                        // name, and every keyframer faced itself.)
                        target_name: p.exact("TargetName"),
                        target_offset: p.vector("TargetOffset").unwrap_or(Vec3::ZERO),
                        active_sound: p.string("ActiveSound"),
                        sound_radius: p.real_or("SoundRadius", AWM_SOUND_RADIUS),
                        start_active: p.bool_or("StartActive", false),
                        start_paused: p.bool_or("StartPaused", false),
                        forward: true,
                        ..default()
                    }));
                }
                "Explosion" => {
                    kind = Kind::Explosion {
                        radius: p.real_or("DamageRadius", 200.0),
                        damage: p.real_or("MaxDamage", 200.0),
                        effect: p.string("ImpactFXName"),
                        remove: p.bool_or("RemoveWhenDone", true),
                    }
                }
                "LightGroup" => kind = light_group_start(&p),
                "SoundFX" | "SpecialFX" | "ScaleSprite" => {
                    kind = Kind::Switchable {
                        on: p.bool_or("StartOn", true),
                        plays: 0,
                    }
                }
                "TimerTrigger" => kind = Kind::Timer(Box::new(events::Timer::of(&p))),
                "LightActivateTrigger" => kind = Kind::LightUse(Box::new(events::LightUse::of(&p))),
                "CaptionTrigger" => {
                    kind = Kind::Caption {
                        showtime: p.real_or("Showtime", 10.0),
                        locked: false,
                    }
                }
                "MessageTrigger" => {
                    kind = Kind::MessageTrigger {
                        showtime: p.real_or("Showtime", 10.0),
                    }
                }
                "MissionTrigger" => kind = Kind::Mission(Vec::new()),
                "Camera" => kind = Kind::Camera(Box::new(events::Camera::of(&p))),
                "BGMTrigger" => kind = Kind::MusicTrigger { on: false },
                c if super::super::volumes::is_volume(c) => kind = Kind::Volume,
                _ => {}
            }
            let hp = (node || matches!(kind, Kind::Active(_))).then(|| destructible(&class, &p));
            let obj = Object {
                class: class.clone(),
                name: name.clone(),
                pose,
                load_pose: pose,
                visible: if node { visible } else { true },
                solid,
                rayhit,
                node,
                hp,
                damage_on: true,
                player_damage_on: true,
                kind,
                props: p,
                ..default()
            };
            if !name.is_empty() {
                w.names.entry(name.to_ascii_lowercase()).or_default().push(i as ObjId);
            }
            w.objects.push(obj);
        }
        w.resolve();
        w
    }

    /// After every object exists: rotation points, attachments, the
    /// starting states.
    fn resolve(&mut self) {
        for i in 0..self.objects.len() {
            let point = match &self.objects[i].kind {
                Kind::Active(_) => self.objects[i].props.string("RotationPoint"),
                _ => continue,
            };
            if point.is_empty() {
                continue;
            }
            let Some(&p) = self.names.get(&point.to_ascii_lowercase()).and_then(|v| v.first()) else {
                continue;
            };
            let at = self.objects[p as usize].pose.pos;
            if self.objects[p as usize].class == "Point" {
                self.objects[p as usize].removed = true;
            } else {
                self.log.push(format!("RotationPoint {point} is a {}", self.objects[p as usize].class));
            }
            if let Kind::Active(a) = &mut self.objects[i].kind {
                a.pivot = at;
            }
        }
        // Attachments.
        for i in 0..self.objects.len() {
            let list = self.objects[i].props.string("Attachments");
            for n in list.split(';').map(str::trim).filter(|n| !n.is_empty()) {
                self.attach(i as ObjId, n);
            }
        }
        // Starting states (silent unless they start on).
        let now = 0.0;
        for i in 0..self.objects.len() {
            let start_hidden = self.objects[i].node && self.objects[i].props.bool_or("StartHidden", false);
            if start_hidden {
                self.objects[i].first = Some(self.objects[i].flags());
                self.objects[i].visible = false;
                self.objects[i].solid = false;
                self.objects[i].rayhit = false;
                self.objects[i].dirty = true;
            }
            let pose = self.objects[i].pose;
            let start_on = {
                let p = &self.objects[i].props;
                let door = self.objects[i].class.ends_with("Door");
                if door {
                    p.bool("StartOpen").or_else(|| p.bool("StartOn")).unwrap_or(false)
                } else {
                    p.bool_or("StartOn", false)
                }
            };
            if let Kind::Active(a) = &mut self.objects[i].kind {
                let mut out = Vec::new();
                a.start(start_on, &pose, now, &mut out);
                self.apply_outs(i as ObjId, out, Who::Nobody);
            }
            if let Kind::Trigger(t) = &mut self.objects[i].kind
                && t.timed
            {
                t.next_timed = t.min_time as f64;
            }
        }
    }

    /// Attach every object named `name` to `id` at its current offset.
    pub fn attach(&mut self, id: ObjId, name: &str) {
        let ids: Vec<ObjId> = self.named_all(name);
        let base = self.objects[id as usize].pose;
        for o in ids {
            if o == id || self.objects[id as usize].attached.iter().any(|a| a.id == o) {
                continue;
            }
            let p = self.objects[o as usize].pose;
            let inv = base.rot.inverse();
            self.objects[id as usize].attached.push(Attached {
                id: o,
                offset: inv * (p.pos - base.pos),
                rot: inv * p.rot,
            });
            self.objects[o as usize].parent = Some(id);
        }
    }

    /// Every object of name `name` (as the engine compares names).
    pub fn named_all(&self, name: &str) -> Vec<ObjId> {
        let n = name.trim();
        let n = if n.starts_with('<') { n.split_once('>').map_or(n, |(_, r)| r) } else { n };
        self.names.get(&n.to_ascii_lowercase()).cloned().unwrap_or_default()
    }

    /// Move object `id` to `pose` (its attachments follow).
    pub fn set_pose(&mut self, id: ObjId, pose: Pose) {
        let o = &mut self.objects[id as usize];
        if o.pose == pose {
            return;
        }
        o.pose = pose;
        o.dirty = true;
        let attached = o.attached.clone();
        for a in attached {
            let half = self.objects[a.id as usize].pose.half;
            self.set_pose(
                a.id,
                Pose {
                    pos: pose.pos + pose.rot * a.offset,
                    rot: (pose.rot * a.rot).normalize(),
                    half,
                },
            );
        }
    }

    fn character_pos(&self, who: Who) -> Option<Vec3> {
        self.chars.iter().find(|c| c.who == who).map(CharBox::centre)
    }

    /// Run a mover's outputs: commands (sender the world model, target
    /// the activator), sounds, the door link.
    fn apply_outs(&mut self, id: ObjId, outs: Vec<Out>, activator: Who) {
        for o in outs {
            match o {
                Out::Command(c) => {
                    let target = match activator {
                        Who::Object(a) => Some(a),
                        _ => None,
                    };
                    let ctx = Ctx {
                        sender: Who::Object(id),
                        target,
                        ..Ctx::default()
                    };
                    self.run(&c, ctx);
                }
                Out::Sound { file, looping } => {
                    let o = &self.objects[id as usize];
                    let Some(a) = o.awm() else { continue };
                    let attached = a.sound_pos == Vec3::ZERO;
                    let s = ObjSound {
                        file,
                        at: o.pose.pos + a.sound_pos,
                        attach: attached.then_some(id),
                        radius: a.sound_radius,
                        inner: a.sound_radius * movers::SOUND_INNER_FRACTION,
                        looping,
                        serial: 0,
                    };
                    self.play(id, s, false);
                }
                Out::StopSound => self.stop_loop(id),
                Out::DoorLink(_) => {}
            }
        }
    }

    /// Start sound `s` of object `id`: its transition (or active) sound,
    /// replacing the one before, or (`shot`) a placed one-shot.
    pub fn play(&mut self, id: ObjId, mut s: ObjSound, shot: bool) {
        self.sound_serial = self.sound_serial.wrapping_add(1);
        s.serial = self.sound_serial;
        self.fx.push(Fx::Sound {
            file: s.file.clone(),
            at: s.at,
            attach: s.attach,
            radius: s.radius,
            inner: s.inner,
            looping: s.looping,
            owner: (!shot).then_some(id),
        });
        let o = &mut self.objects[id as usize];
        if shot {
            o.shot = Some(s);
        } else {
            o.sound = Some(s);
        }
    }

    /// Stop object `id`'s looping sound (a one-shot plays to its end:
    /// Corrections (2026-10-22) 3).
    pub fn stop_loop(&mut self, id: ObjId) {
        self.fx.push(Fx::StopSound { owner: id });
        let o = &mut self.objects[id as usize];
        if o.sound.as_ref().is_some_and(|s| s.looping) {
            o.sound = None;
        }
    }

    /// A keyframer's `ActiveSound` (9, 14): looping on its first object
    /// while it is active, stopped when it isn't.
    fn active_sound(&mut self, id: ObjId) {
        let Kind::KeyFramer(k) = &self.objects[id as usize].kind else { return };
        if k.active_sound.is_empty() {
            return;
        }
        let playing = self.objects[id as usize].sound.is_some();
        if k.active == playing {
            return;
        }
        if !k.active {
            self.stop_loop(id);
            return;
        }
        let (file, radius) = (k.active_sound.clone(), k.sound_radius);
        let Some(&first) = self.controlled(k).first() else { return };
        let s = ObjSound {
            file,
            at: self.objects[first as usize].pose.pos,
            attach: Some(first),
            radius,
            inner: radius * movers::SOUND_INNER_FRACTION,
            looping: true,
            serial: 0,
        };
        self.play(id, s, false);
    }

    /// Run a command string (its failure, if any, logged).
    pub fn run(&mut self, text: &str, ctx: Ctx) {
        if let Err(e) = commands::run(self, text, ctx) {
            self.fx.push(Fx::Log(format!("command stopped: {} ({text})", e.0)));
        }
    }

    /// Deliver `message` to object `id` now, from `ctx.sender`.
    pub fn deliver(&mut self, id: ObjId, message: &str, ctx: Ctx) {
        let Some(o) = self.objects.get(id as usize) else { return };
        if o.removed {
            return;
        }
        let t = tokens(message);
        let Some(first) = t.first() else { return };
        let word = first.text.to_ascii_uppercase();
        let args: Vec<String> = t[1..].iter().map(|t| t.text.clone()).collect();
        if matches!(ctx.sender, Who::Character(_)) {
            self.active_player = ctx.sender;
        }
        let o = &mut self.objects[id as usize];
        let fresh = o.first.is_none();
        if fresh {
            o.first = Some((o.visible, o.solid, o.rayhit));
        }
        // Base messages.
        match word.as_str() {
            "VISIBLE" => {
                if let Some(v) = flag(args.first()) {
                    o.visible = v;
                    o.dirty = true;
                    // Our reading: as a first message it sets what the
                    // object "had" (Corrections (2026-10-22) 4's example
                    // keeps it visible through hidden 1 / hidden 0).
                    if fresh && let Some(f) = &mut o.first {
                        f.0 = v;
                    }
                }
                return;
            }
            "SOLID" => {
                if let Some(v) = flag(args.first()) {
                    o.solid = v;
                    if fresh && let Some(f) = &mut o.first {
                        f.1 = v;
                    }
                    // Corrections (2026-10-22) 4: solid 0 clears ray-hit
                    // too; solid 1 restores it if it had it at first.
                    if !v {
                        o.rayhit = false;
                    } else if o.first.is_some_and(|f| f.2) {
                        o.rayhit = true;
                    }
                    o.dirty = true;
                }
                return;
            }
            "HIDDEN" => {
                if let Some(v) = flag(args.first()) {
                    if v {
                        o.visible = false;
                        o.solid = false;
                        o.rayhit = false;
                        if let Some(d) = &mut o.hp {
                            d.hidden_saved.get_or_insert((d.can_damage, d.never_destroy));
                            d.can_damage = false;
                            d.never_destroy = true;
                        }
                    } else {
                        // Corrections (2026-10-22) 4: adds back what it had
                        // at its first message, never clears a flag.
                        let (v, s, r) = o.first.unwrap_or((true, true, true));
                        o.visible |= v;
                        o.solid |= s;
                        o.rayhit |= r;
                        if let Some(d) = &mut o.hp
                            && let Some((c, n)) = d.hidden_saved.take()
                        {
                            d.can_damage = c;
                            d.never_destroy = n;
                        }
                    }
                    o.dirty = true;
                }
                return;
            }
            "REMOVE" => {
                self.remove(id);
                return;
            }
            "MEMORIZE" => {
                o.memo = Some(Memo {
                    pose: o.pose,
                    flags: o.flags(),
                    awm: match &o.kind {
                        Kind::Active(a) => Some(a.clone()),
                        _ => None,
                    },
                });
                return;
            }
            "REMEMBER" => {
                // Corrections (2026-10-22) 15: only the logic state comes
                // back; the object isn't moved and its flags stay.
                if let Some(m) = o.memo.clone() {
                    let mut out = Vec::new();
                    if let (Kind::Active(a), Some(saved)) = (&mut o.kind, m.awm) {
                        *a = saved;
                        a.restart_sound(&mut out);
                    }
                    o.dirty = true;
                    let activator = o.awm().map_or(Who::Nobody, |a| a.activator);
                    self.apply_outs(id, out, activator);
                }
                return;
            }
            "NEXT_UPDATE" => return,
            _ => {}
        }
        // World-model messages.
        if o.is_world_model() {
            match word.as_str() {
                "ATTACH" => {
                    for n in &args {
                        self.attach(id, n);
                    }
                    return;
                }
                "DETACH" => {
                    let o = &mut self.objects[id as usize];
                    let gone: Vec<ObjId> = if args.is_empty() {
                        o.attached.drain(..).map(|a| a.id).collect()
                    } else {
                        let names: Vec<String> = args.iter().map(|a| a.to_ascii_lowercase()).collect();
                        let mut gone = Vec::new();
                        let all = std::mem::take(&mut o.attached);
                        for a in all {
                            let n = self.objects[a.id as usize].name.to_ascii_lowercase();
                            if names.contains(&n) {
                                gone.push(a.id);
                            } else {
                                self.objects[id as usize].attached.push(a);
                            }
                        }
                        gone
                    };
                    for g in gone {
                        self.objects[g as usize].parent = None;
                    }
                    return;
                }
                "DESTROY" => {
                    if let Some(d) = &mut o.hp {
                        d.never_destroy = false;
                    }
                    return;
                }
                "DAMAGEON" | "DAMAGEOFF" => {
                    o.damage_on = word == "DAMAGEON";
                    return;
                }
                "PLAYERDAMAGEON" | "PLAYERDAMAGEOFF" => {
                    o.player_damage_on = word == "PLAYERDAMAGEON";
                    return;
                }
                "AIDAMAGEON" | "AIDAMAGEOFF" => return,
                "MAXHITPOINT" => {
                    if let (Some(d), Some(h)) = (&mut o.hp, args.first().and_then(|a| a.parse::<f32>().ok())) {
                        d.max = h;
                        d.hp = d.hp.min(h);
                    }
                    return;
                }
                "ACTIVATE" if o.parent.is_some() && !matches!(o.kind, Kind::Active(_)) => {
                    let parent = o.parent.unwrap();
                    self.deliver(parent, message, ctx);
                    return;
                }
                _ => {}
            }
        }
        self.class_message(id, &word, &args, message, ctx);
    }

    /// Remove object `id` (not players): gone from the world, its
    /// attachments with it.
    pub fn remove(&mut self, id: ObjId) {
        let o = &mut self.objects[id as usize];
        if o.removed {
            return;
        }
        o.removed = true;
        o.visible = false;
        o.solid = false;
        o.rayhit = false;
        o.dirty = true;
        // Ours: a removed object's loop goes with it.
        if o.sound.as_ref().is_some_and(|s| s.looping) {
            o.sound = None;
        }
        let attached: Vec<ObjId> = o.attached.drain(..).map(|a| a.id).collect();
        let remove_attachments = o.props.bool_or("RemoveAttachments", true);
        for a in attached {
            if remove_attachments {
                self.remove(a);
            }
        }
    }

    /// Messages of each class.
    fn class_message(&mut self, id: ObjId, word: &str, args: &[String], message: &str, ctx: Ctx) {
        let now = self.now;
        let class = self.objects[id as usize].class.clone();
        if RULE_CLASSES.contains(&class.as_str()) || class.ends_with("GameControl") {
            let name = self.objects[id as usize].name.clone();
            self.fx.push(Fx::ObjectMessage {
                id,
                class: class.clone(),
                name,
                message: message.to_string(),
                sender: ctx.sender,
            });
        }
        let mut kind = std::mem::take(&mut self.objects[id as usize].kind);
        match &mut kind {
            Kind::Active(a) => {
                let pose = self.objects[id as usize].pose;
                let f = Frame {
                    now,
                    dt: self.dt,
                    blocking: Blocking {
                        chars: &self.chars,
                        solid: self.objects[id as usize].solid,
                    },
                };
                let mut out = Vec::new();
                let mut trigger_with: Option<Who> = None;
                match word {
                    "TRIGGER" => trigger_with = Some(a.activator),
                    "ON" if matches!(a.state, State::Off | State::PowerOff) => trigger_with = Some(ctx.sender),
                    "OFF" if matches!(a.state, State::On | State::PowerOn) => trigger_with = Some(ctx.sender),
                    "ACTIVATE" => {
                        let ok = match ctx.sender {
                            Who::Character(_) => {
                                let player =
                                    self.chars.iter().find(|c| c.who == ctx.sender).is_none_or(|c| c.player);
                                if player { a.player_activate } else { a.ai_activate }
                            }
                            _ => false,
                        };
                        if ok {
                            trigger_with = Some(ctx.sender);
                        }
                    }
                    "LOCK" => a.locked = true,
                    "UNLOCK" => a.locked = false,
                    "PAUSE" => a.pause(&pose),
                    "RESUME" => a.resume(&pose),
                    "SETPOWERONMULTIPLIER" => {
                        if let Some(m) = args.first().and_then(|x| x.parse::<f32>().ok()) {
                            // Replaces (Corrections (2026-10-22) 5).
                            a.multiplier = m;
                        }
                    }
                    "INIT" => {
                        let at = a.init();
                        self.objects[id as usize].kind = kind;
                        self.set_pose(id, Pose { pos: at, ..pose });
                        return;
                    }
                    "TRIGGERCLOSE"
                        if a.door
                            && matches!(a.state, State::On | State::PowerOn)
                            && now > a.stop_time + a.on_wait as f64 =>
                    {
                        a.enter(State::PowerOff, &pose, None, now, &mut out);
                    }
                    _ => {}
                }
                if let Some(who) = trigger_with {
                    if word != "TRIGGER" || who != Who::Nobody {
                        a.activator = who;
                    }
                    let at = match a.activator {
                        Who::Object(o) => self.objects.get(o as usize).map(|o| o.pose.pos),
                        w => self.chars.iter().find(|c| c.who == w).map(CharBox::centre),
                    };
                    a.trigger(&f, &pose, at, &mut out);
                    // Door link (Corrections (2026-10-22) 2): any trigger
                    // that leaves an unlocked door in PowerOn, unless a
                    // bot is the activator, tells the linked door.
                    let by_bot = self.chars.iter().any(|c| c.who == a.activator && !c.player);
                    if a.door && !a.locked && a.state == State::PowerOn && !by_bot && !a.door_link.is_empty() {
                        out.push(Out::DoorLink(a.door_link.clone()));
                    }
                }
                let activator = a.activator;
                self.objects[id as usize].kind = kind;
                let links: Vec<String> = out
                    .iter()
                    .filter_map(|o| match o {
                        Out::DoorLink(n) => Some(n.clone()),
                        _ => None,
                    })
                    .collect();
                self.apply_outs(id, out, activator);
                for n in links {
                    // The first object of that name only.
                    if let Some(d) = self.named_all(&n).first().copied() {
                        self.link_open(d, activator);
                    }
                }
                return;
            }
            Kind::Trigger(t) => match word {
                "TRIGGER" => {
                    self.objects[id as usize].kind = kind;
                    self.fire(id, ctx.sender, false);
                    return;
                }
                "LOCK" => t.locked = true,
                "UNLOCK" => {
                    t.locked = false;
                    let (pos, dims) = (self.objects[id as usize].pose.pos, t.dims);
                    let inside: Vec<Who> = self
                        .chars
                        .iter()
                        .filter(|c| c.alive)
                        .filter(|c| (c.centre() - pos).abs().cmplt(dims).all())
                        .map(|c| c.who)
                        .collect();
                    self.objects[id as usize].kind = kind;
                    for w in inside {
                        self.fire(id, w, true);
                    }
                    return;
                }
                "CLEARPLAYERLIST" => self.once.clear(),
                "ON" | "OFF" => {
                    t.shown = word == "ON";
                    if let Some(m) = &mut t.marker {
                        m.on = t.shown;
                        self.objects[id as usize].dirty = true;
                    }
                }
                _ => {}
            },
            Kind::CommandTrigger { locked, commands } => match word {
                "TRIGGER" if !*locked => {
                    let cmds = commands.clone();
                    self.objects[id as usize].kind = kind;
                    for c in cmds.iter().filter(|c| !c.is_empty()) {
                        self.run(c, Ctx { sender: ctx.sender, target: Some(id), ..Ctx::default() });
                    }
                    return;
                }
                "LOCK" => *locked = true,
                "UNLOCK" => *locked = false,
                _ => {}
            },
            Kind::TouchTrigger { locked, .. } => match word {
                "LOCK" => *locked = true,
                "UNLOCK" => *locked = false,
                _ => {}
            },
            Kind::Use(u) => match word {
                "LOCK" => {
                    u.locked = true;
                    u.user = None;
                }
                "UNLOCK" => u.locked = false,
                _ => {}
            },
            Kind::EventCounter { locked, .. } => {
                let up = matches!(word, "INC" | "INCREMENT");
                let down = matches!(word, "DEC" | "DECREMENT");
                if word == "LOCK" {
                    *locked = true;
                } else if word == "UNLOCK" {
                    *locked = false;
                } else if up || down {
                    // A whole number of single steps (Corrections
                    // (2026-10-22) 14).
                    let n = args.first().and_then(|a| a.parse::<f32>().ok()).map_or(1, |n| n as i32);
                    self.objects[id as usize].kind = kind;
                    self.count_steps(id, up, n);
                    return;
                }
            }
            Kind::KeyFramer(k) => {
                if !k.built {
                    if matches!(word, "ON") {
                        k.start_active = true;
                    } else if matches!(word, "OFF") {
                        k.start_active = false;
                    } else {
                        k.queued.push(message.to_string());
                    }
                } else {
                    self.objects[id as usize].kind = kind;
                    self.keyframer_message(id, word, args);
                    return;
                }
            }
            Kind::Explosion { .. } if matches!(word, "ON" | "START") => {
                self.objects[id as usize].kind = kind;
                self.explode(id, ctx.sender);
                return;
            }
            Kind::Volume => {
                let ob = &mut self.objects[id as usize];
                let number = args.first().and_then(|a| a.parse::<f32>().ok());
                match word {
                    "ON" => {
                        ob.visible = true;
                        ob.dirty = true;
                        // Switching on nudges every player so containers
                        // and touches are re-evaluated (Corrections
                        // (2026-10-22) 16).
                        self.objects[id as usize].kind = kind;
                        self.nudge_players();
                        return;
                    }
                    "OFF" => {
                        ob.visible = false;
                        ob.dirty = true;
                    }
                    "DAMAGEVAL" => {
                        ob.vol.damage = number;
                        ob.dirty = true;
                    }
                    "VELOCITY" if class == "JumpVolume" => {
                        ob.vol.speed = number;
                        ob.dirty = true;
                    }
                    "LOCK" | "UNLOCK" => {
                        ob.vol.locked = Some(word == "LOCK");
                        ob.dirty = true;
                    }
                    _ => {}
                }
            }
            Kind::Switchable { on, plays } => {
                let sound = self.objects[id as usize].class == "SoundFX";
                match word {
                    "ON" | "START" => {
                        *on = true;
                        *plays += 1;
                    }
                    "OFF" | "KILL" => *on = false,
                    // A sound's TRIGGER starts it again; the others' flips.
                    "TRIGGER" if sound => {
                        *on = true;
                        *plays += 1;
                    }
                    "TOGGLE" | "TRIGGER" => {
                        *on = !*on;
                        *plays += *on as u32;
                    }
                    _ => {}
                }
            }
            // world_dat.md "Light groups": `COLOR v` (every channel v/255)
            // or `COLOR r g b` (a negative value keeps that channel)
            // changes the colour without switching the group on.
            Kind::LightGroup { on, color } => match word {
                "ON" => *on = true,
                "OFF" => *on = false,
                "TOGGLE" => *on = !*on,
                "COLOR" => {
                    let n: Vec<f32> = args.iter().filter_map(|a| a.parse().ok()).collect();
                    let want = match n.as_slice() {
                        [v] => [*v; 3],
                        [r, g, b, ..] => [*r, *g, *b],
                        _ => [-1.0; 3],
                    };
                    for (k, v) in want.into_iter().enumerate() {
                        if v >= 0.0 {
                            color[k] = v / 255.0;
                        }
                    }
                }
                _ => {}
            },
            Kind::MusicTrigger { on } => match word {
                "ON" => {
                    *on = true;
                    self.fx.push(Fx::Music { id, on: true });
                }
                "OFF" => {
                    *on = false;
                    self.fx.push(Fx::Music { id, on: false });
                }
                _ => {}
            },
            k => {
                self.event_message(id, k, word, args);
            }
        }
        self.objects[id as usize].kind = kind;
    }

    /// An event counter takes `n` single steps, each followed by its
    /// matching commands before the next; a `LOCK` the commands send
    /// stops the rest (Corrections (2026-10-22) 14).
    fn count_steps(&mut self, id: ObjId, up: bool, n: i32) {
        for _ in 0..n.max(0) {
            let Kind::EventCounter { value, locked, events } = &mut self.objects[id as usize].kind else { return };
            if *locked {
                return;
            }
            let run: Vec<String> = if up {
                *value += 1;
                events.iter().filter(|e| e.0 == *value).map(|e| e.1.clone()).collect()
            } else if *value > 0 {
                *value -= 1;
                events.iter().filter(|e| e.0 == *value).map(|e| e.2.clone()).collect()
            } else {
                Vec::new()
            };
            for c in run.iter().filter(|c| !c.is_empty()) {
                self.run(c, Ctx { sender: Who::Object(id), target: Some(id), ..Ctx::default() });
            }
        }
    }

    /// The linked door is told (7): the same activator; triggered when
    /// off or closing.
    fn link_open(&mut self, id: ObjId, activator: Who) {
        let pose = self.objects[id as usize].pose;
        let at = self.character_pos(activator);
        let mut kind = std::mem::take(&mut self.objects[id as usize].kind);
        let mut out = Vec::new();
        if let Kind::Active(a) = &mut kind
            && matches!(a.state, State::Off | State::PowerOff)
        {
            a.activator = activator;
            let f = Frame {
                now: self.now,
                dt: self.dt,
                blocking: Blocking {
                    chars: &self.chars,
                    solid: self.objects[id as usize].solid,
                },
            };
            a.trigger(&f, &pose, at, &mut out);
        }
        self.objects[id as usize].kind = kind;
        self.apply_outs(id, out, activator);
    }

    /// A map explosion goes off.
    fn explode(&mut self, id: ObjId, sender: Who) {
        let o = &self.objects[id as usize];
        let Kind::Explosion {
            radius,
            damage,
            effect,
            remove,
        } = &o.kind
        else {
            return;
        };
        self.fx.push(Fx::Explode {
            at: o.pose.pos,
            radius: *radius,
            damage: *damage,
            effect: effect.clone(),
            attacker: sender,
        });
        if *remove {
            self.remove(id);
        }
    }

    /// A trigger is fired (12, "Firing"): by touch or by `TRIGGER`.
    pub fn fire(&mut self, id: ObjId, who: Who, by_touch: bool) {
        let now = self.now;
        let Kind::Trigger(t) = &mut self.objects[id as usize].kind else { return };
        if now + 1e-9 < t.last_attempt + t.trigger_delay as f64 {
            return;
        }
        t.last_attempt = now;
        if t.player_once && t.pending.is_some() && self.once.contains(&who) {
            return;
        }
        t.toucher = who;
        t.by_touch = by_touch;
        if matches!(who, Who::Character(_)) {
            self.active_player = who;
        }
        if t.pending.is_none() && !t.locked {
            t.pending = Some(now + t.send_delay as f64);
            if t.send_delay == 0.0 && t.trigger_delay == 0.0 {
                self.activate(id);
            } else {
                t.countdown = 0.0;
            }
        }
    }

    /// A trigger activates (12, "Activation").
    fn activate(&mut self, id: ObjId) {
        let roll = self.rng.roll();
        let pos = self.objects[id as usize].pose.pos;
        let Kind::Trigger(t) = &mut self.objects[id as usize].kind else { return };
        t.pending = None;
        let who = std::mem::take(&mut t.toucher);
        let by_touch = t.by_touch;
        t.counter += 1;
        if t.counter < t.activation_count {
            return;
        }
        t.counter = 0;
        if t.n_activations > 0 && t.fired >= t.n_activations {
            return;
        }
        t.fired += 1;
        if t.player_once && matches!(who, Who::Character(_)) && !self.once.contains(&who) {
            self.once.push(who);
        }
        let sound = (!t.sound.is_empty()).then(|| ObjSound {
            file: t.sound.clone(),
            at: pos,
            attach: None,
            radius: t.sound_radius,
            inner: t.sound_radius * movers::SOUND_INNER_FRACTION,
            looping: false,
            serial: 0,
        });
        let mut run: Vec<String> = Vec::new();
        for (k, c) in t.commands.iter().enumerate() {
            if t.weighted && k < 2 {
                let first = roll < t.weight1;
                if (k == 0) != first {
                    continue;
                }
            }
            run.push(c.clone());
        }
        let touch = (by_touch && t.trigger_touch).then(|| t.command_touch.clone());
        if let Some(s) = sound {
            self.play(id, s, true);
        }
        let ctx = Ctx {
            sender: who,
            target: Some(id),
            ..Ctx::default()
        };
        for c in run.iter().filter(|c| !c.is_empty()) {
            self.run(c, ctx);
        }
        if let Some(c) = touch.filter(|c| !c.is_empty()) {
            // The toucher is both sender and default target (Corrections
            // (2026-10-22) 9).
            self.run(c.as_str(), Ctx { sender: who, target: None, target_char: who });
        }
    }

    /// A character's box meets the touch triggers it overlaps (players
    /// only: bots never fire triggers by touch).
    pub fn touches(&mut self) {
        let chars = self.chars.clone();
        for c in chars.iter().filter(|c| c.alive && c.player) {
            self.touch_with(c, &mut Vec::new());
        }
    }

    /// Switching a volume on moves every player 10 u along (+1, +1, +1)
    /// and straight back: the triggers its box touches in either place
    /// are touch-fired once (Corrections (2026-10-22) 16; the move is the
    /// length 10 along that direction, ours).
    pub fn nudge_players(&mut self) {
        let chars = self.chars.clone();
        let by = Vec3::ONE.normalize() * NUDGE;
        for c in chars.iter().filter(|c| c.alive && c.player) {
            let mut done = Vec::new();
            self.touch_with(&CharBox { min: c.min + by, max: c.max + by, ..*c }, &mut done);
            self.touch_with(c, &mut done);
        }
    }

    /// The touch triggers `c`'s box overlaps (not those in `done`, which
    /// gets the ones fired).
    fn touch_with(&mut self, c: &CharBox, done: &mut Vec<ObjId>) {
        for id in 0..self.objects.len() as ObjId {
            let o = &self.objects[id as usize];
            if o.removed || done.contains(&id) {
                continue;
            }
            let pos = o.pose.pos;
            let inside = |dims: Vec3| c.min.cmplt(pos + dims).all() && c.max.cmpgt(pos - dims).all();
            match &o.kind {
                Kind::Trigger(t) if t.player_triggerable && inside(t.dims) => {
                    done.push(id);
                    self.fire(id, c.who, true);
                }
                Kind::TouchTrigger { dims, locked, limit, fired, commands } if !*locked && inside(*dims) => {
                    if *limit > 0 && *fired >= *limit {
                        continue;
                    }
                    done.push(id);
                    let cmds = commands.clone();
                    if let Kind::TouchTrigger { fired, .. } = &mut self.objects[id as usize].kind {
                        *fired += 1;
                    }
                    self.active_player = c.who;
                    for cmd in cmds.iter().filter(|c| !c.is_empty()) {
                        self.run(cmd, Ctx { sender: c.who, target: Some(id), ..Ctx::default() });
                    }
                }
                _ => {}
            }
        }
    }

    /// The generic use triggers (`ActivateTrigger`): players holding use
    /// inside each; `WorkCommand` when one starts, `ReleaseCommand` when
    /// they stop, `ActivateCommand` once the use time is reached.
    pub fn use_triggers(&mut self) {
        let dt = self.dt;
        for id in 0..self.objects.len() as ObjId {
            let pos = self.objects[id as usize].pose.pos;
            let Kind::Use(u) = &self.objects[id as usize].kind else { continue };
            if u.locked || self.objects[id as usize].removed {
                continue;
            }
            let dims = u.dims;
            let holding = |c: &CharBox| {
                c.alive && c.player && c.using && c.min.cmplt(pos + dims).all() && c.max.cmpgt(pos - dims).all()
            };
            let current = u.user.map(|(w, _)| w);
            let still = current.is_some_and(|w| self.chars.iter().any(|c| c.who == w && holding(c)));
            let mut run: Vec<(String, Who)> = Vec::new();
            let Kind::Use(u) = &mut self.objects[id as usize].kind else { continue };
            if still {
                let (w, t) = u.user.as_mut().unwrap();
                *t += dt;
                if *t + 1e-6 >= u.time {
                    run.push((u.activate.clone(), *w));
                    u.user = None;
                }
            } else {
                if let Some(w) = current {
                    run.push((u.release.clone(), w));
                    u.user = None;
                }
                if let Some(c) = self.chars.iter().find(|c| holding(c)) {
                    u.user = Some((c.who, 0.0));
                    run.push((u.work.clone(), c.who));
                }
            }
            for (c, w) in run {
                if !c.is_empty() {
                    self.active_player = w;
                    self.run(&c, Ctx { sender: w, target: Some(id), ..Ctx::default() });
                }
            }
        }
    }

    /// Bots open a closed, unlocked door they walk into (bots.md 3,
    /// "Doors": `AIActivate`): an AI character whose box comes within
    /// `AI_DOOR_REACH` of an activatable world model that is off sends it
    /// `ACTIVATE`.
    pub fn ai_doors(&mut self) {
        let chars: Vec<CharBox> = self.chars.iter().filter(|c| c.alive && !c.player).copied().collect();
        for c in chars {
            for id in 0..self.objects.len() as ObjId {
                let o = &self.objects[id as usize];
                let Some(a) = o.awm() else { continue };
                if o.removed || !a.ai_activate || a.locked || a.state != State::Off {
                    continue;
                }
                let reach = Vec3::splat(AI_DOOR_REACH);
                if !o.pose.obb().overlaps(c.min - reach, c.max + reach) {
                    continue;
                }
                self.deliver(id, "ACTIVATE", Ctx { sender: c.who, target: None, ..Ctx::default() });
            }
        }
    }

    /// A character's use key on object `id` (the use ray hit it).
    pub fn use_object(&mut self, id: ObjId, who: Who) {
        let o = &self.objects[id as usize];
        let target = match (o.awm(), o.parent) {
            (Some(_), _) => id,
            (None, Some(p)) => p,
            _ => return,
        };
        if self.objects[target as usize].awm().is_some_and(Awm::activatable) {
            self.deliver(target, "ACTIVATE", Ctx { sender: who, target: None, ..Ctx::default() });
        }
    }

    /// Damage to world model `id` (11, hit points), from `attacker` (a
    /// player when `player`).
    pub fn damage(&mut self, id: ObjId, amount: f32, attacker: Who, player: bool) {
        let o = &mut self.objects[id as usize];
        if o.removed || !o.damage_on || (player && !o.player_damage_on) {
            return;
        }
        let Some(d) = o.hp.as_mut() else { return };
        if d.dead || !d.can_damage || (player && !d.can_player_damage) || amount <= 0.0 {
            return;
        }
        d.hp = (d.hp - amount).max(0.0);
        let mut run: Vec<String> = Vec::new();
        if d.trigger_counter > 0 {
            d.trigger_counter -= 1;
        } else if d.num_sends > 0 {
            d.num_sends -= 1;
            run.push(d.damage_command.clone());
            if player {
                run.push(d.player_damage_command.clone());
            }
        }
        if d.hp < d.max * d.percent_for {
            run.push(d.percent_command.clone());
        }
        let frac = if d.max > 0.0 { d.hp / d.max } else { 0.0 };
        for (cmd, lo, hi, inside) in &mut d.hp_commands {
            let now_in = frac >= *lo && frac <= *hi;
            if now_in && !*inside {
                run.push(cmd.clone());
            }
            *inside = now_in;
        }
        let dies = d.hp <= BREAKABLE_DEATH_HP && !d.never_destroy;
        let mut death: Vec<String> = Vec::new();
        let mut remove = false;
        if dies {
            d.hp = 0.0;
            d.dead = true;
            death.push(d.death_command.clone());
            if player {
                death.push(d.player_death_command.clone());
            }
            remove = d.remove_on_death;
        }
        let ctx = Ctx {
            sender: attacker,
            target: Some(id),
            ..Ctx::default()
        };
        for c in run.iter().filter(|c| !c.is_empty()) {
            self.run(c, ctx);
        }
        if dies {
            for c in death.iter().filter(|c| !c.is_empty()) {
                self.run(
                    c,
                    Ctx {
                        sender: Who::Object(id),
                        target: Some(id),
                        ..Ctx::default()
                    },
                );
            }
            if remove {
                self.remove(id);
            } else {
                let o = &mut self.objects[id as usize];
                o.visible = false;
                o.solid = false;
                o.rayhit = false;
                o.dirty = true;
                let gone: Vec<ObjId> = o.attached.drain(..).map(|a| a.id).collect();
                for g in gone {
                    self.objects[g as usize].parent = None;
                }
            }
        }
    }

    /// One server frame: the command manager, then each object's update.
    pub fn frame(&mut self, now: f64, dt: f32) {
        self.now = now;
        self.dt = dt;
        self.frames += 1;
        commands::tick(self, dt);
        for id in 0..self.objects.len() as ObjId {
            if self.objects[id as usize].removed {
                continue;
            }
            self.update(id);
            if matches!(self.objects[id as usize].kind, Kind::KeyFramer(_)) {
                self.active_sound(id);
            }
        }
    }

    /// One object's update, when due.
    fn update(&mut self, id: ObjId) {
        let (now, dt) = (self.now, self.dt);
        let first_frame = self.frames == 1;
        let mut kind = std::mem::take(&mut self.objects[id as usize].kind);
        match &mut kind {
            Kind::Active(a) => {
                let Some(c) = a.countdown.as_mut() else {
                    self.objects[id as usize].kind = kind;
                    return;
                };
                *c -= dt;
                if *c > 0.0 {
                    self.objects[id as usize].kind = kind;
                    return;
                }
                let pose = self.objects[id as usize].pose;
                let solid = self.objects[id as usize].solid;
                let chars = std::mem::take(&mut self.chars);
                let mut out = Vec::new();
                let moved = movers::update_when_due(a, &pose, solid, &chars, now, dt, &mut out);
                let activator = a.activator;
                self.chars = chars;
                self.objects[id as usize].kind = kind;
                if let Some((next, riders)) = moved {
                    self.carry(&riders, &pose, &next);
                    self.set_pose(id, next);
                }
                self.apply_outs(id, out, activator);
                return;
            }
            Kind::Trigger(t) => {
                if !t.started {
                    t.started = true;
                    if !t.attach_to.is_empty() {
                        let name = t.attach_to.clone();
                        self.objects[id as usize].kind = kind;
                        if let Some(&p) = self.named_all(&name).first() {
                            let at = self.objects[p as usize].pose.pos;
                            self.objects[id as usize].pose.pos = at;
                            let a = Attached {
                                id,
                                offset: Vec3::ZERO,
                                rot: Quat::IDENTITY,
                            };
                            self.objects[p as usize].attached.push(a);
                        }
                        kind = std::mem::take(&mut self.objects[id as usize].kind);
                    }
                }
                let Kind::Trigger(t) = &mut kind else { unreachable!() };
                let due = t.pending.is_some_and(|at| now + 1e-9 >= at);
                let timed = t.timed && now + 1e-9 >= t.next_timed;
                if timed {
                    let (lo, hi) = (t.min_time, t.max_time);
                    t.next_timed = now + (lo + (hi - lo) * self.rng.roll()) as f64;
                }
                self.objects[id as usize].kind = kind;
                if due {
                    self.activate(id);
                }
                if timed {
                    self.fire(id, Who::Nobody, false);
                }
                return;
            }
            Kind::StartupCommand { command } if first_frame => {
                let c = command.clone();
                self.objects[id as usize].kind = kind;
                self.run(&c, Ctx { sender: Who::Object(id), target: Some(id), ..Ctx::default() });
                self.remove(id);
                return;
            }
            Kind::KeyFramer(_) => {
                self.objects[id as usize].kind = kind;
                self.keyframer_update(id);
                return;
            }
            k => {
                self.event_update(id, k);
            }
        }
        self.objects[id as usize].kind = kind;
    }

    /// Characters standing on a world model that moved from `from` to
    /// `to` move with it (10).
    fn carry(&mut self, riders: &[usize], from: &Pose, to: &Pose) {
        for &i in riders {
            let c = &mut self.chars[i];
            let delta = movers::carried(c, from, to);
            c.min += delta;
            c.max += delta;
            self.fx.push(Fx::Carry { who: c.who, delta });
        }
    }

    /// The objects a keyframer moves.
    pub fn controlled(&self, k: &Keyframer) -> Vec<ObjId> {
        k.object_names.iter().flat_map(|n| self.named_all(n)).filter(|o| !self.objects[*o as usize].removed).collect()
    }

    /// A keyframer's update (9).
    fn keyframer_update(&mut self, id: ObjId) {
        let dt = self.dt;
        let Kind::KeyFramer(mut k) = std::mem::take(&mut self.objects[id as usize].kind) else { return };
        if !k.built {
            k.built = true;
            // The offsets from key 0 (stored on the keyframer's own list).
            let queued = std::mem::take(&mut k.queued);
            if k.start_active {
                k.activate();
                k.paused = k.start_paused;
            }
            k.offsets = self.offsets(&k);
            self.objects[id as usize].kind = Kind::KeyFramer(k);
            for m in queued {
                let t = tokens(&m);
                if let Some(first) = t.first() {
                    let args: Vec<String> = t[1..].iter().map(|t| t.text.clone()).collect();
                    self.keyframer_message(id, &first.text.to_ascii_uppercase(), &args);
                }
            }
            return;
        }
        if !k.active || k.paused {
            self.objects[id as usize].kind = Kind::KeyFramer(k);
            return;
        }
        let f = k.frame(dt);
        k.snap = false;
        let keys = k.keys.clone();
        self.objects[id as usize].kind = Kind::KeyFramer(k);
        for &i in &f.passed {
            self.pass_key(id, &keys[i]);
        }
        if let Some((pos, rot)) = f.place {
            self.place_controlled(id, pos, rot, true);
        }
        if let Some(c) = f.arrived.filter(|c| !c.is_empty()) {
            self.run(&c, Ctx { sender: Who::Object(id), target: Some(id), ..Ctx::default() });
        }
    }

    /// A key's sound and command.
    fn pass_key(&mut self, id: ObjId, key: &keyframer::Key) {
        if !key.sound.is_empty() {
            let s = ObjSound {
                file: key.sound.clone(),
                at: key.pos,
                attach: None,
                radius: key.sound_radius,
                inner: key.sound_radius * movers::SOUND_INNER_FRACTION,
                looping: false,
                serial: 0,
            };
            self.play(id, s, true);
        }
        if !key.command.is_empty() {
            self.run(&key.command, Ctx { sender: Who::Object(id), target: Some(id), ..Ctx::default() });
        }
    }

    /// Each controlled object's offset from key 0 (position, and the
    /// rotation's quaternion difference component-wise, Quirk 7).
    fn offsets(&self, k: &Keyframer) -> Vec<(ObjId, Vec3, Vec4)> {
        let Some(k0) = k.keys.first() else { return Vec::new() };
        let r0 = k0.rot();
        self.controlled(k)
            .into_iter()
            .map(|o| {
                let p = self.objects[o as usize].pose;
                (o, p.pos - k0.pos, Vec4::from(p.rot) - Vec4::from(r0))
            })
            .collect()
    }

    /// Where keyframer `k`'s target is for its objects to face (its
    /// position plus `TargetOffset`), when it has one.
    pub fn keyframer_target(&self, k: &Keyframer) -> Option<Vec3> {
        if k.target_name.is_empty() {
            return None;
        }
        self.named_all(&k.target_name).first().map(|t| self.objects[*t as usize].pose.pos + k.target_offset)
    }

    /// Put the controlled objects on the path point (`keyframer::
    /// placements`); with `carry` (a frame's move, not `GOTO`'s jump)
    /// characters standing on a solid world model among them move with it
    /// (10: the engine carries on any move).
    fn place_controlled(&mut self, id: ObjId, pos: Vec3, rot: Quat, carry: bool) {
        let Kind::KeyFramer(k) = &self.objects[id as usize].kind else { return };
        let objects: Vec<(ObjId, Pose)> =
            self.controlled(k).into_iter().map(|o| (o, self.objects[o as usize].pose)).collect();
        let places = keyframer::placements(k, pos, rot, &objects, self.keyframer_target(k));
        for ((o, old), new) in objects.into_iter().zip(places) {
            if carry && new != old {
                let ob = &self.objects[o as usize];
                let riders = Blocking {
                    chars: &self.chars,
                    solid: ob.solid && ob.is_world_model(),
                }
                .riders(&old);
                self.carry(&riders, &old, &new);
            }
            self.set_pose(o, new);
        }
    }

    /// A keyframer's messages after its first update.
    fn keyframer_message(&mut self, id: ObjId, word: &str, args: &[String]) {
        let Kind::KeyFramer(mut k) = std::mem::take(&mut self.objects[id as usize].kind) else { return };
        let mut goto: Option<usize> = None;
        match word {
            "ON" => k.activate(),
            "OFF" => k.active = false,
            "PAUSE" => k.paused = true,
            "RESUME" => {
                k.paused = false;
                k.active = true;
            }
            "FORWARD" => {
                if !k.forward {
                    k.toggle_dir();
                }
            }
            "BACKWARD" | "REVERSE" => {
                if k.forward {
                    k.toggle_dir();
                }
            }
            "TOGGLEDIR" => k.toggle_dir(),
            "GOTO" => {
                if let Some(i) = args.first().and_then(|n| keyframer::find_key(&k.base, k.keys.len(), n)) {
                    k.goto(i);
                    goto = Some(i);
                }
            }
            "MOVETO" => {
                if let Some(i) = args.first().and_then(|n| keyframer::find_key(&k.base, k.keys.len(), n)) {
                    let cmd = args.get(1).cloned().unwrap_or_default();
                    k.active = true;
                    k.paused = false;
                    let times = k.times();
                    let behind = if k.forward { times[i] < k.t } else { times[i] > k.t };
                    if behind {
                        k.toggle_dir();
                    }
                    k.dest = Some((i, cmd));
                }
            }
            "TARGET" => k.target_name = args.first().cloned().unwrap_or_default(),
            "CLEARTARGET" => k.target_name.clear(),
            "TARGETOFFSET" => k.target_offset = words_vector(args),
            _ => {}
        }
        let keys = k.keys.clone();
        self.objects[id as usize].kind = Kind::KeyFramer(k);
        if let Some(i) = goto {
            let key = &keys[i];
            self.place_controlled(id, key.pos, key.rot(), false);
            self.pass_key(id, key);
        }
        self.active_sound(id);
    }

    /// A summary of object `id` for the console.
    pub fn describe(&self, id: ObjId) -> String {
        let o = &self.objects[id as usize];
        let mut s = format!(
            "[{id}] {} {:?}: pos ({:.1}, {:.1}, {:.1}) visible {} solid {} rayhit {}{}",
            o.class, o.name, o.pose.pos.x, o.pose.pos.y, o.pose.pos.z, o.visible, o.solid, o.rayhit,
            if o.removed { " REMOVED" } else { "" }
        );
        match &o.kind {
            Kind::Active(a) => s += &format!(
                " | {:?} {:?} angles ({:.1}, {:.1}, {:.1})° tau {:.2}{}",
                a.motion,
                a.state,
                a.angles.x.to_degrees(),
                a.angles.y.to_degrees(),
                a.angles.z.to_degrees(),
                a.tau,
                if a.locked { " LOCKED" } else { "" }
            ),
            Kind::Trigger(t) => s += &format!(" | fired {} pending {:?} locked {}", t.fired, t.pending, t.locked),
            Kind::EventCounter { value, .. } => s += &format!(" | value {value}"),
            Kind::KeyFramer(k) => {
                s += &format!(" | keys {} active {} t {:.2} next {}", k.keys.len(), k.active, k.t, k.next)
            }
            Kind::Switchable { on, plays } => s += &format!(" | on {on} plays {plays}"),
            Kind::LightGroup { on, color } => s += &format!(" | on {on} colour {color}"),
            Kind::MusicTrigger { on } => s += &format!(" | music {on}"),
            Kind::Timer(t) => s += &format!(" | timer T {} running {}", t.time, t.started.is_some()),
            Kind::LightUse(u) => s += &format!(" | working {} done {} locked {}", u.count, u.completions, u.locked),
            Kind::Mission(l) => s += &format!(" | objectives {l:?}"),
            Kind::Camera(c) => s += &format!(" | live {} fov {}", c.live, c.fov),
            _ => {}
        }
        if let Some(d) = &o.hp
            && !d.never_destroy
        {
            s += &format!(" | hp {}/{}{}", d.hp, d.max, if d.dead { " DEAD" } else { "" });
        }
        s
    }
}

/// Hex text as bytes.
pub fn hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    s.len().is_multiple_of(2).then_some(())?;
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

impl CommandHost for ObjectWorld {
    fn commands(&mut self) -> &mut CommandState {
        &mut self.cmds
    }

    fn send(&mut self, targets: &str, message: &str, ctx: Ctx) {
        let names: Vec<String> = if targets.trim().is_empty() {
            vec![String::new()]
        } else {
            targets.split_whitespace().map(str::to_string).collect()
        };
        for n in names {
            let lower = n.to_ascii_lowercase();
            let chars: Vec<Who> = match lower.as_str() {
                "player" => self.chars.iter().filter(|c| c.player).map(|c| c.who).collect(),
                "activeplayer" => vec![self.active_player].into_iter().filter(|w| *w != Who::Nobody).collect(),
                "otherplayers" => {
                    self.chars.iter().filter(|c| c.player && c.who != self.active_player).map(|c| c.who).collect()
                }
                "ai" => self.chars.iter().filter(|c| !c.player).map(|c| c.who).collect(),
                "sender" => match ctx.sender {
                    Who::Character(_) => vec![ctx.sender],
                    Who::Object(o) => {
                        self.deliver(o, message, ctx);
                        continue;
                    }
                    Who::Nobody => vec![],
                },
                "" => {
                    if let Some(t) = ctx.target {
                        self.deliver(t, message, ctx);
                        continue;
                    }
                    match ctx.target_char {
                        Who::Character(_) => vec![ctx.target_char],
                        _ => continue,
                    }
                }
                _ => {
                    for id in self.named_all(&n) {
                        self.deliver(id, message, ctx);
                    }
                    continue;
                }
            };
            for who in chars {
                self.fx.push(Fx::CharacterMessage {
                    who,
                    message: message.to_string(),
                });
            }
        }
    }

    fn send_to(&mut self, object: ObjId, message: &str, ctx: Ctx) {
        self.deliver(object, message, ctx);
    }

    fn named(&self, name: &str) -> Option<ObjId> {
        self.named_all(name).first().copied()
    }

    fn random(&mut self) -> f32 {
        self.rng.roll()
    }

    fn log(&mut self, line: String) {
        self.fx.push(Fx::Log(line));
    }
}

/// Split a message's `;` parts (several messages in one group).
pub fn message_parts(text: &str) -> Vec<String> {
    statements(text)
}
