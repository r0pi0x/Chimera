use bevy::prelude::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::*;

/// What a character wants to do this tick. Written by exactly one intent
/// source (local input, a bot brain, later the network) and read by the
/// Movement and Weapon slots. Buttons are level-based ("held"), not edges:
/// implementations that need edges (e.g. no auto-jump) track the previous
/// tick themselves.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Intent {
    /// x = right, y = forward, each in [-1, 1]. Relative to `yaw`.
    pub move_axis: Vec2,
    /// Both keys of an axis held at once (x: left and right, y: forward
    /// and back): `move_axis` is 0 there, but a movement that tells this
    /// from no key (Combat Arms: still "moving", no braking) reads it.
    pub opposed: BVec2,
    /// Absolute look angles in radians. Yaw 0 looks down -Z; pitch up is +.
    pub yaw: f32,
    pub pitch: f32,
    pub jump: bool,
    pub crouch: bool,
    pub sprint: bool,
    /// The walk key (Source's +speed). Shift sets it along with `sprint`;
    /// each movement reads the one it has. `+walk` and `+sprint` set one
    /// each (a game whose walk and sprint are two keys binds those).
    pub walk: bool,
    pub fire: bool,
    /// Secondary attack (Source attack2: stab, zoom, burst toggle).
    pub secondary: bool,
    pub reload: bool,
    /// A weapon pick held this tick; the weapon switches when it changes.
    /// A slot key is the slot (0 = first slot: its first weapon, or the
    /// next one in it after the held one); a pick of one weapon in a slot
    /// (the HUD's weapon selection: `invnext`, the second grenade) is
    /// `Intent::pick` (see `Intent::picked`).
    pub select: Option<u8>,
    /// Switch to the previously held weapon (Source `lastinv`).
    pub last_weapon: bool,
    /// The use key (Source `+use`, E): doors, buttons. Held level; the
    /// logic layer acts on presses.
    pub use_key: bool,
    /// Named actions held this tick (`ActionRegistry`): bit `i` is the
    /// action registered `i`th (by name order). A game's own buttons
    /// (`throw`, `ability`, `emote`) ride here beside the fields above,
    /// which stay as Chimera's own names for the common ones.
    pub actions: u32,
    /// This tick's command number (Source's usercmd `command_number`):
    /// shared randoms (shot spread, recoil) are seeded from it. Stamped
    /// every tick after the rules (`SimSet::Commands`, `number_commands`);
    /// intent sources needn't set it.
    pub command: u32,
}

/// Weapon slots a pick can name (`Intent::select`).
pub const PICK_SLOTS: u8 = 10;
/// Weapons per slot a pick can name.
pub const PICK_DEPTH: u8 = 8;

/// Most named actions a game set can register (`Intent::actions` is a
/// `u32`).
pub const MAX_ACTIONS: usize = 32;

/// The named actions of this app (`ActionAppExt::intent_action`): a game
/// or minigame registers `throw` and binds `+throw` (the client reads
/// the keys bound to it, `chimera_client::binds`); the bit rides in
/// `Intent::actions` and the network command. Kept sorted by name, so
/// the server and its clients agree on the bits whatever order the games
/// registered them in (as `PredictedComponents` does).
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct ActionRegistry {
    names: Vec<String>,
}

impl ActionRegistry {
    /// Add an action (idempotent; names are lower-case). Returns false
    /// when the registry is full.
    pub fn register(&mut self, name: &str) -> bool {
        let name = name.trim().to_lowercase();
        match self.names.binary_search(&name) {
            Ok(_) => true,
            Err(_) if self.names.len() >= MAX_ACTIONS => false,
            Err(at) => {
                self.names.insert(at, name);
                true
            }
        }
    }

    /// The registered names, in bit order.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// An action's bit in `Intent::actions` (0: not registered).
    pub fn bit(&self, name: &str) -> u32 {
        let name = name.trim().to_lowercase();
        self.names.binary_search(&name).map_or(0, |i| 1 << i)
    }
}

/// Registering actions from a plugin.
pub trait ActionAppExt {
    /// Register the named action `name` (see `ActionRegistry`).
    fn intent_action(&mut self, name: &str) -> &mut Self;
}

impl ActionAppExt for App {
    fn intent_action(&mut self, name: &str) -> &mut Self {
        if !self.world_mut().get_resource_or_insert_with(ActionRegistry::default).register(name) {
            warn!("action {name}: no room (at most {MAX_ACTIONS} named actions)");
        }
        self
    }
}

impl Intent {
    /// Whether the named action `name` is held.
    pub fn action(&self, registry: &ActionRegistry, name: &str) -> bool {
        self.actions & registry.bit(name) != 0
    }

    /// The `select` value picking the `nth` weapon (from 0, in inventory
    /// order) in `slot`.
    pub fn pick(slot: u8, nth: u8) -> u8 {
        slot + PICK_SLOTS * (nth.min(PICK_DEPTH - 1) + 1)
    }

    /// A `select` value as its slot and, for a pick, the weapon's place in
    /// the slot (None: a slot key).
    pub fn picked(select: u8) -> (u8, Option<u8>) {
        let (slot, n) = (select % PICK_SLOTS, select / PICK_SLOTS);
        (slot, n.checked_sub(1))
    }

    pub fn look_rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0)
    }

    pub fn yaw_rotation(&self) -> Quat {
        Quat::from_rotation_y(self.yaw)
    }
}

/// Character velocity in meters per second, owned by the active Movement
/// implementation.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, Deref, DerefMut, Serialize, Deserialize)]
#[reflect(Component)]
pub struct Velocity(pub Vec3);

/// Published by the active Movement implementation every tick. Other slots
/// (weapons, HUD, animation) read this instead of depending on a specific
/// movement implementation.
#[derive(Component, Reflect, Default, Clone, Debug, Serialize, Deserialize)]
#[reflect(Component)]
pub struct MovementState {
    pub on_ground: bool,
    pub crouching: bool,
    pub sprinting: bool,
    /// Eye position relative to the character's origin, in meters.
    pub eye_offset: Vec3,
    /// The collision box relative to the character's origin (engine
    /// space, meters), when the movement has one (Source hulls); zero
    /// when it doesn't (the logic layer then uses the collider's bounds).
    pub hull_min: Vec3,
    pub hull_max: Vec3,
    /// The moving solid (`MovingSolid`) the character stands on, if any.
    /// Not sent to a predicting client (entity ids differ).
    #[serde(skip)]
    pub ground: Option<Entity>,
    /// Climbing a ladder (movements that have them).
    pub on_ladder: bool,
    /// The eye's roll from a view punch (radians; positive tilts the view
    /// clockwise, Source's roll): hard landings kick it.
    pub view_roll: f32,
}

/// A velocity that moves a character without being part of its own
/// velocity (Source base velocity: push triggers, leaving a moving
/// platform), engine space, m/s. Movement implementations that model it
/// read and consume it (specs/source/triggers.md, trigger_push).
#[derive(Component, Reflect, Default, Clone, Copy, Debug, Serialize, Deserialize)]
#[reflect(Component)]
pub struct BaseVelocity {
    pub velocity: Vec3,
    /// Set this tick by a push (Source: FL_BASEVELOCITY); the movement
    /// clears it at the start of its next tick.
    pub touched: bool,
    /// Taken off the ground by game logic (push, teleport) this tick: the
    /// movement must not snap it back down before it moves.
    pub unground: bool,
}

/// Per-character gravity multiplier (Source entity gravity, set by
/// trigger_gravity); 0 means normal.
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct EntityGravity(pub f32);

impl Default for EntityGravity {
    fn default() -> Self {
        Self(1.0)
    }
}

/// A solid that moves (doors, platforms): its brushes where they are this
/// tick (engine space) and its velocity (m/s), for movement that sweeps
/// brushes exactly. Kept up to date by whoever moves it (the logic layer).
#[derive(Component, Clone, Debug, Default)]
pub struct MovingSolid {
    pub brushes: Vec<MapBrush>,
    pub velocity: Vec3,
    /// Solid to characters right now (doors with "Passable", disabled
    /// func_brush: false).
    pub solid: bool,
}

// Serde: replicated to clients (`net`).
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
pub struct Health {
    /// 1.0 is a standard player's full health (README normalization rules).
    pub current: f32,
    pub max: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self { current: 1.0, max: 1.0 }
    }
}

#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
pub struct Team(pub u8);

/// How many times the server changed a character's predicted state
/// outside its commands while it lived (a rules' weapon swap: a gun
/// game's rung): a network client predicting it takes the server's state
/// after such a change as a restart, not a prediction error (as it does a
/// spawn or a death), since no command of its led to it. `rebase` counts
/// one.
#[derive(Component, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[component(storage = "SparseSet")]
pub struct Rebased(pub u32);

/// Count a change to `e`'s predicted state the server made outside its
/// commands (`Rebased`).
pub fn rebase(world: &mut World, e: Entity) {
    if let Ok(mut ent) = world.get_entity_mut(e) {
        let n = ent.get::<Rebased>().map_or(0, |r| r.0);
        ent.insert(Rebased(n.wrapping_add(1)));
    }
}

/// Where the server turned a character's look (a spawn facing): counts the
/// turns (`count`, from 1) and keeps the last one. A network client takes
/// the look (`OwnState::look`) and echoes the count in its commands
/// (`NetCmd::look_ack`); until it does, the server keeps this look instead
/// of the command's own (which still has the old one). `set_view` makes one.
#[derive(Component, Default, Clone, Copy, Debug, PartialEq)]
pub struct ViewSet {
    pub count: u8,
    pub yaw: f32,
    pub pitch: f32,
}

/// Turn `e`'s look to `yaw`, `pitch` (radians) as the server: its `Intent`
/// now, its client's view when it hears (`ViewSet`).
pub fn set_view(world: &mut World, e: Entity, yaw: f32, pitch: f32) {
    let Ok(mut ent) = world.get_entity_mut(e) else { return };
    if let Some(mut i) = ent.get_mut::<Intent>() {
        i.yaw = yaw;
        i.pitch = pitch;
    }
    let count = ent.get::<ViewSet>().map_or(0, |v| v.count).wrapping_add(1).max(1);
    ent.insert(ViewSet { count, yaw, pitch });
}

/// A character's own dice seed, for rolls that should repeat run to run
/// (bots' route noise and choices, buying, bullet spread). Entity ids
/// won't do: every spawn shifts them, resources included (since Bevy
/// 0.19 resources are entities). Bots get one from their number and team
/// (`bot::add_bot`); without one, systems fall back to the entity id.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component)]
pub struct Seed(pub u64);

impl Seed {
    /// The seed of `entity`, or one made from its id without a `Seed`.
    pub fn of(seed: Option<&Seed>, entity: Entity) -> u64 {
        seed.map_or(entity.to_bits(), |s| s.0)
    }
}

