//! The globals tag (`matg`, `globals\globals`; the blocks specs/halo_ce's
//! movement.md, health_shields.md, projectiles.md and multiplayer_rules.md
//! use): the player information block (speeds, accelerations, the airborne
//! acceleration, stun penalties and times), the falling damage block, the
//! multiplayer information block (the references and the 43 announcer
//! sounds), the grenades and the multiplayer weapon list, as plain data in
//! the game's own values, no conversion on load. The other blocks are
//! counts.
//!
//! Units, as movement.md states them: speeds in world units per second,
//! accelerations in world units per second per tick, stun penalties as
//! factors, stun times in seconds, falling distances in world units, the
//! falling velocities in world units per tick (stored precomputed).
//!
//! - `Globals::read(map, tag)` (or `Globals::read_view`).
//! - `AnnouncerEvent` names the 43 multiplayer sounds in the spec's order;
//!   `MultiplayerInfo::sound(event)` looks one up, `AnnouncerEvent::
//!   is_immediate` and `queue_ticks` are section 9's rules.
//! - `Globals::grenade(GrenadeType)` and `Globals::weapon_list`.
//!
//! Known shortcuts:
//! - The specs name the blocks' offsets and some fields' offsets (the run
//!   forward speed at 0x34, the stun fields from 0x80); the other fields'
//!   offsets were read off the disc by matching the values the specs state
//!   (movement.md's constants table in its order; the grenades' fields in
//!   the order multiplayer_rules.md lists them) and are marked "from the
//!   disc" in the docs. Fields the specs name no value for are not read:
//!   the player information's "multiplayer only speed multiplier" (the spec
//!   says 0), the first-person and camera fields, and the unnamed
//!   values at 0x94 to 0xA4.
//! - Each of the player, falling damage and multiplayer information blocks
//!   must hold at least one element; the first is used. The grenades are
//!   capped at 2 elements, the weapon list at 32, the vehicles at 8 and the
//!   announcer sounds at 43.
//! - The blocks the specs do not name are reported by offset and count only
//!   (`OTHER_BLOCKS`); the struct is read as its first 0x1AC bytes. Of the
//!   first person interface block (0x17C) only its first element's arms
//!   model is read (`first_person_arms`, globals.md).

use std::io;

use crate::cache::{Bounds, MapFile, TagEntry, TagRef, View, class};
use crate::object::GrenadeType;
use crate::scenario::coded_enum;

/// How much of the globals' main struct is read (the last block field at
/// 0x1A0 ends here).
pub const GLOBALS_SIZE: usize = 0x1AC;
/// The difficulty block's offset (counted only).
pub const DIFFICULTY_OFFSET: usize = 0x11C;
/// The grenades block's offset and its element size.
pub const GRENADES_OFFSET: usize = 0x128;
pub const GRENADE_SIZE: usize = 0x44;
/// The multiplayer weapon list's offset.
pub const WEAPON_LIST_OFFSET: usize = 0x14C;
/// The multiplayer information block's offset and its element size.
pub const MULTIPLAYER_OFFSET: usize = 0x164;
pub const MULTIPLAYER_SIZE: usize = 0xA0;
/// The player information block's offset and its element size.
pub const PLAYER_OFFSET: usize = 0x170;
pub const PLAYER_SIZE: usize = 0xF4;
/// The falling damage block's offset and its element size.
pub const FALLING_OFFSET: usize = 0x188;
pub const FALLING_SIZE: usize = 0x98;
/// The first person interface block (element 0xC0; globals.md): 0x00 the
/// first-person arms model (`mode`).
pub const FIRST_PERSON_OFFSET: usize = 0x17C;
pub const FIRST_PERSON_SIZE: usize = 0xC0;
/// The announcer sounds in the multiplayer information block.
pub const ANNOUNCER_SOUNDS: usize = 43;
/// Game ticks per second.
pub const TICKS: i32 = 30;
/// The offsets of the other blocks of the globals, counted only (from the
/// disc; the specs name none of them but the difficulty block).
pub const OTHER_BLOCKS: [usize; 10] = [0xF8, 0x104, 0x110, 0x11C, 0x134, 0x140, 0x158, 0x17C, 0x194, 0x1A0];

const MAX_GRENADES: usize = 2;
const MAX_WEAPON_LIST: usize = 32;
const MAX_VEHICLES: usize = 8;

coded_enum! {
    /// The multiplayer information's 43 announcer sounds, in the spec's
    /// order (multiplayer_rules.md, section 9).
    pub enum AnnouncerEvent {
        PlayBall = 0,
        GameOver = 1,
        OneMinuteToWin = 2,
        ThirtySecondsToWin = 3,
        RedTeamMinuteToWin = 4,
        RedTeam30ToWin = 5,
        BlueTeamMinuteToWin = 6,
        BlueTeam30ToWin = 7,
        BlueTeamHasTheFlag = 8,
        BlueTeamFlagReturned = 9,
        BlueTeamScore = 10,
        RedTeamHasTheFlag = 11,
        RedTeamFlagReturned = 12,
        RedTeamScore = 13,
        DoubleKill = 14,
        TripleKill = 15,
        Killtacular = 16,
        RunningRiot = 17,
        KillingSpree = 18,
        Oddball = 19,
        Race = 20,
        Slayer = 21,
        CaptureTheFlag = 22,
        Warthog = 23,
        Ghost = 24,
        Scorpion = 25,
        /// Immediate: a race flag touched.
        CountdownTimer = 26,
        /// Immediate: a local player teleports.
        TeleporterActivate = 27,
        /// Immediate: capture the flag scoring refused.
        FlagFailure = 28,
        /// Immediate: the respawn timer at 90, 60, 30.
        CountdownForRespawn = 29,
        /// Immediate: the hill moves, the ball resets.
        HillMove = 30,
        /// Immediate: the respawn timer at 1.
        PlayerRespawn = 31,
        TeamKingOfTheHill = 32,
        TeamOddball = 33,
        TeamRace = 34,
        TeamSlayer = 35,
        KingOfTheHill = 36,
        BlueTeamCtf = 37,
        RedTeamCtf = 38,
        HillContested = 39,
        HillControlled = 40,
        /// The spec found no trigger.
        HillOccupied = 41,
        /// Immediate: a race lap, a score chime every 5 seconds.
        CountdownTimerEnd = 42,
    }
}

impl AnnouncerEvent {
    /// Immediate lines (26 to 31 and 42) play at once; the others queue.
    pub fn is_immediate(self) -> bool {
        matches!(self.code(), 26..=31 | 42)
    }

    /// The announcer queue delay of a queueable line whose sound's stored
    /// longest permutation length is `length_ms`: `length x 30 / 1000 + 5`
    /// ticks, integer division.
    pub fn queue_ticks(length_ms: i32) -> i32 {
        length_ms * TICKS / 1000 + 5
    }
}

crate::tag_struct! {
    /// A tag reference as a block element (16 bytes).
    pub struct RefElement: 16 {
        0x00 tag: TagRef,
    }
}

crate::tag_struct! {
    /// The player information element (0xF4 bytes; offsets from the disc
    /// unless the docs say a spec gives them).
    pub struct PlayerInformation: 0xF4 {
        /// `bipd`: the player unit (`characters\cyborg\cyborg`).
        0x00 unit: TagRef,
        /// WU per second; only in the "alert" stance.
        0x2C walking_speed: f32,
        /// Factor; a debug option only.
        0x30 double_speed_multiplier: f32,
        /// WU per second (movement.md: 2.25).
        0x34 run_forward_speed: f32,
        /// WU per second.
        0x38 run_backward_speed: f32,
        /// WU per second.
        0x3C run_sideways_speed: f32,
        /// WU per second per tick.
        0x40 run_acceleration: f32,
        /// WU per second.
        0x44 sneak_forward_speed: f32,
        /// WU per second.
        0x48 sneak_backward_speed: f32,
        /// WU per second.
        0x4C sneak_sideways_speed: f32,
        /// WU per second per tick.
        0x50 sneak_acceleration: f32,
        /// WU per second per tick.
        0x54 airborne_acceleration: f32,
        /// WU: where a thrown grenade starts relative to the camera
        /// (forward, left, up): (0, 0.05, 0) on the disc.
        0x68 grenade_origin: [f32; 3],
        /// Factor (movement.md; health_shields.md gives 0x80).
        0x80 stun_movement_penalty: f32,
        /// Factor (0x84).
        0x84 stun_turning_penalty: f32,
        /// Factor (0x88).
        0x88 stun_jumping_penalty: f32,
        /// Seconds (0x8C).
        0x8C minimum_stun_time: f32,
        /// Seconds (0x90).
        0x90 maximum_stun_time: f32,
        /// Seconds: the wait before the first-person weapon may pose,
        /// drawn between the bounds (globals.md: 10 and 15 on the disc).
        0x9C first_person_idle_time: Bounds<f32>,
        /// Fraction: the chance a draw ends the wait without posing (0.25
        /// on the disc).
        0xA4 first_person_idle_skip: f32,
    }
}

impl PlayerInformation {
    /// The maximum stun time in ticks, truncated (1.25 s is 37).
    pub fn maximum_stun_ticks(&self) -> i32 {
        (self.maximum_stun_time * TICKS as f32) as i32
    }

    /// The first-person idle wait's bounds in ticks (seconds x 30,
    /// truncated: 300 and 450 on the disc).
    pub fn first_person_idle_ticks(&self) -> (i32, i32) {
        let t = |s: f32| (s * TICKS as f32) as i32;
        (t(self.first_person_idle_time.lower), t(self.first_person_idle_time.upper))
    }
}

crate::tag_struct! {
    /// The falling damage element (0x98 bytes; the fields' offsets are
    /// from the disc, matched to movement.md's values).
    pub struct FallingDamage: 0x98 {
        /// WU: harmful falling distance, lower and upper bound (3 and 6).
        0x08 harmful_falling_distance: Bounds<f32>,
        /// `jpt!`: `globals\falling`.
        0x10 falling_damage: TagRef,
        /// WU: the maximum falling distance (16.5).
        0x28 maximum_falling_distance: f32,
        /// `jpt!`: `globals\distance`.
        0x2C distance_damage: TagRef,
        /// `jpt!`: `globalsehicle_hit_environment`.
        0x3C vehicle_hit_environment_damage: TagRef,
        /// `jpt!`: `globalsehicle_killed_unit`.
        0x4C vehicle_killed_unit_damage: TagRef,
        /// `jpt!`: `globalsehicle_collision`.
        0x5C vehicle_collision_damage: TagRef,
        /// `jpt!`: `globalslaming_death`.
        0x6C flaming_death_damage: TagRef,
        /// WU per tick: the velocity of a fall of the maximum distance,
        /// precomputed (0.3430028).
        0x8C maximum_falling_velocity: f32,
        /// WU per tick: the harmful velocities, precomputed (0.14625688 and
        /// 0.20683846).
        0x90 harmful_falling_velocity: Bounds<f32>,
    }
}

crate::tag_struct! {
    /// A grenade type's globals entry (0x44 bytes; the first two fields'
    /// offsets follow the order multiplayer_rules.md lists them, the rest
    /// are from the disc).
    pub struct GrenadeInfo: 0x44 {
        /// Count: the most a unit carries (4 on the disc).
        0x00 maximum_count: i16,
        /// Count: the multiplayer spawn default (0 on the disc; the
        /// reference never reads it).
        0x02 mp_spawn_default: i16,
        /// `effe`: the throwing effect.
        0x04 throwing_effect: TagRef,
        /// `grhi`: the HUD interface.
        0x14 hud_interface: TagRef,
        /// `eqip`: the pickup item.
        0x24 equipment: TagRef,
        /// `proj`: the grenade projectile.
        0x34 projectile: TagRef,
    }
}

crate::tag_struct! {
    struct RawMultiplayer: 0xA0 {
        0x00 flag: TagRef,
        0x10 unit: TagRef,
        0x20 vehicles: Vec<RefElement>,
        0x2C hill_shader: TagRef,
        0x3C flag_shader: TagRef,
        0x4C ball: TagRef,
        0x5C sounds: Vec<RefElement>,
    }
}

/// The multiplayer information element (0xA0 bytes, multiplayer_rules.md
/// section 11.1).
#[derive(Clone, Debug, PartialEq)]
pub struct MultiplayerInfo {
    /// `weap`: the flag.
    pub flag: TagRef,
    /// `bipd`: the multiplayer player's unit.
    pub unit: TagRef,
    /// `vehi`: the Warthog, the Ghost and the Scorpion, in that order.
    pub vehicles: Vec<TagRef>,
    /// The hill's shader.
    pub hill_shader: TagRef,
    /// The flag's shader.
    pub flag_shader: TagRef,
    /// `weap`: the ball.
    pub ball: TagRef,
    /// `snd!`: the announcer sounds in `AnnouncerEvent` order.
    pub sounds: Vec<TagRef>,
}

impl MultiplayerInfo {
    /// The announcer sound of an event (`None` when the block is shorter).
    pub fn sound(&self, event: AnnouncerEvent) -> Option<&TagRef> {
        self.sounds.get(event.code() as usize)
    }
}

/// The globals tag, the blocks the specs use.
#[derive(Clone, Debug, PartialEq)]
pub struct Globals {
    /// Frag first, then plasma.
    pub grenades: Vec<GrenadeInfo>,
    /// `weap` and `eqip`: the multiplayer weapon list (14 on the disc).
    pub weapon_list: Vec<TagRef>,
    pub multiplayer: MultiplayerInfo,
    pub player: PlayerInformation,
    pub falling: FallingDamage,
    /// The element count of the difficulty block (0x11C).
    pub difficulty_count: usize,
    /// The element counts of the other blocks, by offset (`OTHER_BLOCKS`).
    pub block_counts: Vec<(usize, usize)>,
    /// The first-person arms model (the first person interface's first
    /// element; None without one).
    pub first_person_arms: Option<TagRef>,
}

fn first<T: Clone>(v: &View<'_>, offset: usize, what: &str, items: &[T]) -> io::Result<T> {
    items
        .first()
        .cloned()
        .ok_or_else(|| v.error(offset, format_args!("the {what} block is empty")))
}

fn refs(v: &View<'_>, offset: usize, what: &str, items: Vec<RefElement>, max: usize) -> io::Result<Vec<TagRef>> {
    if items.len() > max {
        return Err(v.error(offset, format_args!("{} {what} over the maximum {max}", items.len())));
    }
    Ok(items.into_iter().map(|e| e.tag).collect())
}

impl Globals {
    /// Read the globals tag (`globals\globals`).
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Globals> {
        if tag.class != class::GLOBALS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {} `{}` is not the globals", map.header().name, tag.class, tag.path),
            ));
        }
        Self::read_view(&map.tag_view(tag, GLOBALS_SIZE)?)
    }

    /// Read from a view of the tag's main struct (`GLOBALS_SIZE` bytes).
    pub fn read_view(v: &View<'_>) -> io::Result<Globals> {
        let grenades = v
            .block_max(GRENADES_OFFSET, GRENADE_SIZE, MAX_GRENADES)?
            .read_all::<GrenadeInfo>()?;
        let weapon_list = refs(
            v,
            WEAPON_LIST_OFFSET,
            "weapon list entries",
            v.block_max(WEAPON_LIST_OFFSET, 16, MAX_WEAPON_LIST)?.read_all()?,
            MAX_WEAPON_LIST,
        )?;
        let raw = first(
            v,
            MULTIPLAYER_OFFSET,
            "multiplayer information",
            &v.block_max(MULTIPLAYER_OFFSET, MULTIPLAYER_SIZE, 1)?
                .read_all::<RawMultiplayer>()?,
        )?;
        let multiplayer = MultiplayerInfo {
            flag: raw.flag,
            unit: raw.unit,
            vehicles: refs(v, MULTIPLAYER_OFFSET, "vehicles", raw.vehicles, MAX_VEHICLES)?,
            hill_shader: raw.hill_shader,
            flag_shader: raw.flag_shader,
            ball: raw.ball,
            sounds: refs(v, MULTIPLAYER_OFFSET, "announcer sounds", raw.sounds, ANNOUNCER_SOUNDS)?,
        };
        let player = first(
            v,
            PLAYER_OFFSET,
            "player information",
            &v.block_max(PLAYER_OFFSET, PLAYER_SIZE, 1)?
                .read_all::<PlayerInformation>()?,
        )?;
        let falling = first(
            v,
            FALLING_OFFSET,
            "falling damage",
            &v.block_max(FALLING_OFFSET, FALLING_SIZE, 1)?
                .read_all::<FallingDamage>()?,
        )?;
        let mut block_counts = Vec::with_capacity(OTHER_BLOCKS.len());
        for off in OTHER_BLOCKS {
            let raw = v.raw_block(off)?;
            let count = usize::try_from(raw.count)
                .map_err(|_| v.error(off, format_args!("negative block count {}", raw.count)))?;
            block_counts.push((off, count));
        }
        let difficulty_count = block_counts
            .iter()
            .find(|(o, _)| *o == DIFFICULTY_OFFSET)
            .map_or(0, |(_, c)| *c);
        let fp = v.block_max(FIRST_PERSON_OFFSET, FIRST_PERSON_SIZE, 1)?;
        let first_person_arms = if !fp.is_empty() { Some(fp.get(0)?.tag_ref(0)?) } else { None };
        Ok(Globals {
            first_person_arms,
            grenades,
            weapon_list,
            multiplayer,
            player,
            falling,
            difficulty_count,
            block_counts,
        })
    }

    /// The grenades block's entry for a grenade type.
    pub fn grenade(&self, kind: GrenadeType) -> Option<&GrenadeInfo> {
        self.grenades.get(kind.code() as usize)
    }
}
