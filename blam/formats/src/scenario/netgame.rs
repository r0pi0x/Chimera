//! The multiplayer spawn data of the scenario (specs/halo_ce/scenario.md,
//! "Player starting locations", "Starting profiles and starting
//! equipment", "Netgame flags", "Netgame equipment"): game type numbers
//! and the eligibility rule (`GameTypes`), player starts, starting
//! profiles and equipment, netgame flags with their typed enumeration and
//! the teleporter pairing, netgame equipment with its spawn period, and the
//! heading helpers.

use std::io;

use crate::cache::{Field, TagRef, View};

/// Respawn period in seconds when neither the entry nor its collection
/// names one.
pub const ITEM_DEFAULT_PERIOD: i16 = 30;
/// A unit this close (world units) to a source flag is teleported.
pub const TELEPORT_RADIUS: f32 = 0.5;
/// A unit must leave this radius of the flag it arrived at before that flag
/// works again.
pub const TELEPORT_REARM: f32 = 1.0;
/// Game ticks per second (the spec multiplies seconds by this).
pub const TICKS_PER_SECOND: u32 = 30;

/// Game type slot value: all game types.
pub const SLOT_ALL: i16 = 12;
/// Game type slot value: all game types except capture the flag.
pub const SLOT_ALL_BUT_CTF: i16 = 13;
/// Game type slot value: all game types except capture the flag and race.
pub const SLOT_ALL_BUT_CTF_AND_RACE: i16 = 14;

coded_enum! {
    /// A game type in play (the game engine's own numbering). Slot values
    /// 0 (none), 6 and 7 (never used by a map) and the wildcards 12 to 14
    /// are not game types a match runs: see `GameTypes`.
    pub enum GameType {
        CaptureTheFlag = 1,
        Slayer = 2,
        Oddball = 3,
        KingOfTheHill = 4,
        Race = 5,
    }
}

/// Up to four game type numbers (unused slots 0), as stored on player
/// starts, netgame equipment and starting equipment (8 bytes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameTypes(pub [i16; 4]);

impl Field for GameTypes {
    const SIZE: usize = 8;
    fn read(v: &View<'_>, offset: usize) -> io::Result<Self> {
        Ok(GameTypes([
            v.i16(offset)?,
            v.i16(offset + 2)?,
            v.i16(offset + 4)?,
            v.i16(offset + 6)?,
        ]))
    }
}

impl GameTypes {
    /// Eligible for game `g` in play: some slot equals `g`, or is 12, or is
    /// 13 and `g` is not capture the flag, or is 14 and `g` is neither
    /// capture the flag nor race. A 0 slot never matches.
    pub fn eligible_for(&self, g: GameType) -> bool {
        self.eligible_for_number(g.code())
    }

    /// `eligible_for` with the game type as a number (1 to 5); any other
    /// number matches only the wildcards that do not exclude it.
    pub fn eligible_for_number(&self, g: i16) -> bool {
        self.0.iter().any(|&s| match s {
            0 => false,
            SLOT_ALL => true,
            SLOT_ALL_BUT_CTF => g != 1,
            SLOT_ALL_BUT_CTF_AND_RACE => g != 1 && g != 5,
            s => s == g,
        })
    }

    /// Eligible when no game is running (a solo map): only if every slot
    /// is 0.
    pub fn eligible_when_idle(&self) -> bool {
        self.0.iter().all(|&s| s == 0)
    }
}

/// The forward direction of a heading (player start, netgame flag,
/// netgame equipment): `(cos h, sin h, 0)`. The up direction is `(0, 0, 1)`.
pub fn heading_forward(h: f32) -> [f32; 3] {
    [h.cos(), h.sin(), 0.0]
}

/// The heading a teleported unit gets: its old heading plus the target
/// flag's facing minus the source flag's.
pub fn teleport_heading(old: f32, source_facing: f32, target_facing: f32) -> f32 {
    old + target_facing - source_facing
}

crate::tag_struct! {
    /// A player starting location (52 bytes).
    pub struct PlayerStart: 0x34 {
        /// World units; where the player's feet appear.
        0x00 position: [f32; 3],
        /// Heading in radians; forward is `heading_forward(facing)`.
        0x0C facing: f32,
        /// 0 red, 1 blue; 2 and 3 occur.
        0x10 team_index: i16,
        /// Element of the structure BSP block.
        0x12 bsp_index: i16,
        0x14 game_types: GameTypes,
    }
}

impl PlayerStart {
    /// The forward vector of the start's heading.
    pub fn forward(&self) -> [f32; 3] {
        heading_forward(self.facing)
    }

    /// Whether the start is a candidate in game `g`. (Capture the flag
    /// further wants the player's team to equal `team_index`; the rules
    /// live in the game, not here.)
    pub fn eligible_for(&self, g: GameType) -> bool {
        self.game_types.eligible_for(g)
    }
}

crate::tag_struct! {
    /// A starting profile (104 bytes; campaign loadouts, empty on the
    /// multiplayer maps; layout checked on campaign maps).
    pub struct StartingProfile: 104 {
        0x00 name: String,
        0x20 health_modifier: f32,
        0x24 shield_modifier: f32,
        0x28 primary_weapon: TagRef,
        0x38 primary_rounds_loaded: i16,
        0x3A primary_rounds_total: i16,
        0x3C secondary_weapon: TagRef,
        0x4C secondary_rounds_loaded: i16,
        0x4E secondary_rounds_total: i16,
        0x50 frag_grenades: u8,
        0x51 plasma_grenades: u8,
    }
}

/// Six item collection references (starting equipment's, 96 bytes); null
/// ones keep the class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemCollectionRefs(pub [TagRef; 6]);

impl Field for ItemCollectionRefs {
    const SIZE: usize = 96;
    fn read(v: &View<'_>, offset: usize) -> io::Result<Self> {
        Ok(ItemCollectionRefs([
            v.tag_ref(offset)?,
            v.tag_ref(offset + 16)?,
            v.tag_ref(offset + 32)?,
            v.tag_ref(offset + 48)?,
            v.tag_ref(offset + 64)?,
            v.tag_ref(offset + 80)?,
        ]))
    }
}

crate::tag_struct! {
    /// Starting equipment (204 bytes): what a player holds when spawning in
    /// a multiplayer game.
    pub struct StartingEquipment: 0xCC {
        /// Bit 0: no grenades; bit 1: plasma grenades in place of
        /// fragmentation grenades.
        0x00 flags: u32,
        0x04 game_types: GameTypes,
        /// The game visits the first five only.
        0x3C item_collections: ItemCollectionRefs,
    }
}

impl StartingEquipment {
    pub const NO_GRENADES: u32 = 1;
    pub const PLASMA_GRENADES: u32 = 2;
}

coded_enum! {
    /// A netgame flag's type; what `NetgameFlag::team_index` counts depends
    /// on it.
    pub enum NetgameFlagType {
        /// Team index is the owning team.
        CaptureTheFlag = 0,
        CaptureTheFlagVehicle = 1,
        /// Team index is the ball's number.
        OddballBallSpawn = 2,
        /// Team index is the checkpoint's sequence number.
        RaceTrack = 3,
        RaceVehicle = 4,
        Bank = 5,
        /// Team index is the pair number.
        TeleporterSource = 6,
        /// Team index is the pair number.
        TeleporterTarget = 7,
        /// Team index is the hill's number.
        Hill = 8,
    }
}

crate::tag_struct! {
    /// A netgame flag (148 bytes).
    pub struct NetgameFlag: 0x94 {
        0x00 position: [f32; 3],
        /// Heading in radians.
        0x0C facing: f32,
        0x10 flag_type: NetgameFlagType,
        0x12 team_index: i16,
        /// Class `itmc`; null on every disc flag.
        0x14 reference: TagRef,
    }
}

/// A teleporter source flag and the target of its pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeleporterPair {
    /// The shared team index (the pair number).
    pub pair: i16,
    /// Index of the source flag in the netgame flags block.
    pub source: usize,
    /// Index of the first target flag (block order) with the same pair
    /// number; `None` when the pair has no target.
    pub target: Option<usize>,
}

/// Pair every teleporter source flag with the first target flag of the same
/// team index.
pub fn teleporter_pairs(flags: &[NetgameFlag]) -> Vec<TeleporterPair> {
    flags
        .iter()
        .enumerate()
        .filter(|(_, f)| f.flag_type == NetgameFlagType::TeleporterSource)
        .map(|(source, f)| TeleporterPair {
            pair: f.team_index,
            source,
            target: flags
                .iter()
                .position(|t| t.flag_type == NetgameFlagType::TeleporterTarget && t.team_index == f.team_index),
        })
        .collect()
}

crate::tag_struct! {
    /// A netgame equipment entry (144 bytes): an item spawn.
    pub struct NetgameEquipment: 0x90 {
        /// Bit 0: levitate.
        0x00 flags: u32,
        0x04 game_types: GameTypes,
        /// Stored, not read by the game.
        0x0C team_index: i16,
        /// Seconds; 0 means use the collection's.
        0x0E spawn_time: i16,
        0x40 position: [f32; 3],
        /// Heading in radians; stored and ignored by the disc build.
        0x4C facing: f32,
        /// Class `itmc`.
        0x50 item_collection: TagRef,
    }
}

impl NetgameEquipment {
    pub const LEVITATE: u32 = 1;

    /// The item is created at rest, hanging in the air.
    pub fn levitate(&self) -> bool {
        self.flags & Self::LEVITATE != 0
    }

    /// The entry's period in seconds: its own spawn time if not 0, else
    /// its collection's (`collection_spawn_time`) if not 0, else 30.
    pub fn period_seconds(&self, collection_spawn_time: i16) -> i16 {
        if self.spawn_time != 0 {
            self.spawn_time
        } else if collection_spawn_time != 0 {
            collection_spawn_time
        } else {
            ITEM_DEFAULT_PERIOD
        }
    }

    /// The period in ticks.
    pub fn period_ticks(&self, collection_spawn_time: i16) -> u32 {
        self.period_seconds(collection_spawn_time).max(1) as u32 * TICKS_PER_SECOND
    }

    /// An item is created at game tick `tick` (an exact multiple of the
    /// period).
    pub fn spawns_at_tick(&self, tick: u32, collection_spawn_time: i16) -> bool {
        tick.is_multiple_of(self.period_ticks(collection_spawn_time))
    }

    pub fn eligible_for(&self, g: GameType) -> bool {
        self.game_types.eligible_for(g)
    }
}
