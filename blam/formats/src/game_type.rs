//! The saved game type, a game variant (specs/halo_ce/multiplayer_rules.md,
//! section 1): the 0x68-byte record as a plain struct (`GameVariant`) with
//! the spec's enumerations and flag bits, `from_bytes` and `to_bytes` for
//! the record, and the table of the 26 stock game types the disc's
//! executable builds (`STOCK_GAME_TYPES`, section 2) as constant data.
//!
//! Units, as the spec states them: respawn time, respawn time growth,
//! suicide penalty and the single flag time in ticks (30 per second), health
//! a multiplier (1.0 is 100 percent), score to win in the unit its game type
//! gives (kills, captures, laps, flag points or minutes, section 2's notes).
//!
//! Known shortcuts:
//! - `from_bytes` checks the enumerations are inside the spec's tables
//!   (base type 1 to 5, indicator 0 to 2, weapon set 0 to 13, vehicle set 0
//!   to 4, the oddball and race enumerations) and refuses anything else; it
//!   does not clean a variant the way the game does (section 1.4: pinning
//!   the health to 0.25 to 4, the lives to at least 0 ...). Booleans are any
//!   non-zero byte. The platform signature of a saved file (20 bytes after
//!   the record) is not read.
//! - The stock table gives the values section 2 lists. Where a row names
//!   no value for an option field, the field is 0 (the oddball speed with
//!   the ball of Juggernaut and Team Ball is `Slow`, its code 0): the
//!   spec's rows list only what the builders write. Team Ball's flags are
//!   the intent (35), not what the disc's builder leaves: its entry has
//!   `flags_uncertain` set.
//! - Weapon sets 11 to 13 are the PC game's (reference only, not on the
//!   disc); the type has them so a saved file of the port reads.

use std::io;

use crate::object::bit_flags;
use crate::scenario::coded_enum;

/// A game variant record's size.
pub const VARIANT_SIZE: usize = 0x68;
/// The number of stock game types.
pub const STOCK_COUNT: usize = 26;
/// The name's capacity in UTF-16 units (the 12th is a zero).
pub const NAME_CHARS: usize = 11;
/// Game ticks per second.
pub const TICKS: i32 = 30;

bit_flags! {
    /// The flag bits (record offset 0x20, section 1.3).
    pub struct GameFlags(u32) {
        /// Other players show on the radar.
        OTHER_PLAYERS_ON_RADAR = 0,
        FRIEND_INDICATORS_ON_SCREEN = 1,
        INFINITE_GRENADES = 2,
        /// The menu's "shields no".
        NO_SHIELDS = 3,
        /// Every player is active-camouflaged.
        INVISIBLE_PLAYERS = 4,
        /// Clear is custom starting equipment.
        GENERIC_STARTING_EQUIPMENT = 5,
    }
}

coded_enum! {
    /// The base game type (offset 0x18); 0, 6 and 7 never occur in a game.
    pub enum BaseGameType {
        CaptureTheFlag = 1,
        Slayer = 2,
        Oddball = 3,
        KingOfTheHill = 4,
        Race = 5,
    }
}

coded_enum! {
    /// The objective indicator (offset 0x24).
    pub enum ObjectiveIndicator {
        MotionTracker = 0,
        NavPoints = 1,
        None = 2,
    }
}

coded_enum! {
    /// The weapon set (offset 0x44); 11 to 13 are the PC game's.
    pub enum WeaponSet {
        Normal = 0,
        Pistols = 1,
        Rifles = 2,
        PlasmaWeapons = 3,
        Sniper = 4,
        NoSniping = 5,
        RocketLaunchers = 6,
        Shotguns = 7,
        ShortRange = 8,
        Human = 9,
        /// Reachable only from a saved file or the PC menus.
        NoGrenades = 10,
        Covenant = 11,
        Classic = 12,
        Heavy = 13,
    }
}

coded_enum! {
    /// The vehicle set (offset 0x48).
    pub enum VehicleSet {
        /// All three.
        Default = 0,
        None = 1,
        Warthog = 2,
        Ghost = 3,
        Scorpion = 4,
    }
}

coded_enum! {
    /// Oddball: the speed with the ball (offset 0x50).
    pub enum BallSpeed {
        Slow = 0,
        Normal = 1,
        Fast = 2,
    }
}

coded_enum! {
    /// Oddball: a trait with or without the ball (offsets 0x54, 0x58).
    pub enum OddballTrait {
        None = 0,
        Invisible = 1,
        ExtraDamage = 2,
        DamageResistant = 3,
    }
}

coded_enum! {
    /// Oddball: the ball type (offset 0x5C).
    pub enum BallType {
        Normal = 0,
        ReverseTag = 1,
        Juggernaut = 2,
    }
}

coded_enum! {
    /// Race: the race type (offset 0x4C).
    pub enum RaceType {
        Normal = 0,
        AnyOrder = 1,
        Rally = 2,
    }
}

coded_enum! {
    /// Race: how a team's score is made (offset 0x50).
    pub enum TeamScoring {
        Minimum = 0,
        Maximum = 1,
        Sum = 2,
    }
}

/// The game type options union (offsets 0x4C to 0x63): which fields exist
/// depends on the base type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GameTypeOptions {
    CaptureTheFlag {
        assault: bool,
        /// Never read.
        reset_on_capture: bool,
        flag_must_reset: bool,
        flag_at_home_to_score: bool,
        /// Ticks; 0 is off.
        single_flag_time: i32,
    },
    Slayer {
        /// Set means the bonus is off.
        no_death_bonus: bool,
        /// Set means the penalty is off.
        no_kill_penalty: bool,
        kill_in_order: bool,
    },
    Oddball {
        random_start: bool,
        /// Never read.
        ball_spawn_delay: bool,
        speed_with_ball: BallSpeed,
        trait_with_ball: OddballTrait,
        trait_without_ball: OddballTrait,
        ball_type: BallType,
        /// 1 to 16.
        ball_spawn_count: i32,
    },
    KingOfTheHill {
        moving_hill: bool,
    },
    Race {
        race_type: RaceType,
        team_scoring: TeamScoring,
    },
}

impl GameTypeOptions {
    /// The base game type these options belong to.
    pub const fn base(&self) -> BaseGameType {
        match self {
            GameTypeOptions::CaptureTheFlag { .. } => BaseGameType::CaptureTheFlag,
            GameTypeOptions::Slayer { .. } => BaseGameType::Slayer,
            GameTypeOptions::Oddball { .. } => BaseGameType::Oddball,
            GameTypeOptions::KingOfTheHill { .. } => BaseGameType::KingOfTheHill,
            GameTypeOptions::Race { .. } => BaseGameType::Race,
        }
    }
}

/// A game variant's rules: every field of the record but the name. `Copy`,
/// so the stock table can be constant data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rules {
    /// 1 teams, 0 free for all (capture the flag forces teams).
    pub team_play: bool,
    pub flags: GameFlags,
    pub objective_indicator: ObjectiveIndicator,
    pub odd_man_out: bool,
    /// Ticks added per death.
    pub respawn_time_growth: i32,
    /// Ticks.
    pub respawn_time: i32,
    /// Ticks.
    pub suicide_penalty: i32,
    /// 0 is infinite.
    pub number_of_lives: i32,
    /// A multiplier: 1.0 is 100 percent.
    pub health: f32,
    /// Meaning per game type (section 2's notes).
    pub score_to_win: i32,
    pub weapon_set: WeaponSet,
    pub vehicle_set: VehicleSet,
    pub options: GameTypeOptions,
    /// Bit 0: system default (set on every stock variant).
    pub variant_flags: u16,
}

impl Rules {
    /// The base game type (the options' own).
    pub const fn base(&self) -> BaseGameType {
        self.options.base()
    }
}

/// A saved game variant: a name and the rules.
#[derive(Clone, Debug, PartialEq)]
pub struct GameVariant {
    pub name: String,
    pub rules: Rules,
}

fn bad(what: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("game variant: {what}"))
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().expect("4 bytes"))
}

fn i32_at(b: &[u8], o: usize) -> i32 {
    u32_at(b, o) as i32
}

fn code<T>(b: &[u8], o: usize, what: &str, from: impl Fn(i16) -> Option<T>) -> io::Result<T> {
    let raw = i32_at(b, o);
    i16::try_from(raw)
        .ok()
        .and_then(from)
        .ok_or_else(|| bad(format_args!("{what} {raw} at {o:#x} is outside the table")))
}

impl GameVariant {
    /// Read the 0x68-byte record (more bytes after it are ignored).
    pub fn from_bytes(b: &[u8]) -> io::Result<GameVariant> {
        if b.len() < VARIANT_SIZE {
            return Err(bad(format_args!("{} bytes, a record is {VARIANT_SIZE}", b.len())));
        }
        let units: Vec<u16> = (0..NAME_CHARS)
            .map(|i| u16::from_le_bytes([b[2 * i], b[2 * i + 1]]))
            .take_while(|&u| u != 0)
            .collect();
        let name = String::from_utf16(&units).map_err(|_| bad("the name is not UTF-16 text"))?;
        let base = code(b, 0x18, "base game type", BaseGameType::from_code)?;
        let flag = |o: usize| b[o] != 0;
        let options = match base {
            BaseGameType::CaptureTheFlag => GameTypeOptions::CaptureTheFlag {
                assault: flag(0x4C),
                reset_on_capture: flag(0x4D),
                flag_must_reset: flag(0x4E),
                flag_at_home_to_score: flag(0x4F),
                single_flag_time: i32_at(b, 0x50),
            },
            BaseGameType::Slayer => GameTypeOptions::Slayer {
                no_death_bonus: flag(0x4C),
                no_kill_penalty: flag(0x4D),
                kill_in_order: flag(0x4E),
            },
            BaseGameType::Oddball => GameTypeOptions::Oddball {
                random_start: flag(0x4C),
                ball_spawn_delay: flag(0x4D),
                speed_with_ball: code(b, 0x50, "speed with ball", BallSpeed::from_code)?,
                trait_with_ball: code(b, 0x54, "trait with ball", OddballTrait::from_code)?,
                trait_without_ball: code(b, 0x58, "trait without ball", OddballTrait::from_code)?,
                ball_type: code(b, 0x5C, "ball type", BallType::from_code)?,
                ball_spawn_count: i32_at(b, 0x60),
            },
            BaseGameType::KingOfTheHill => GameTypeOptions::KingOfTheHill {
                moving_hill: flag(0x4C),
            },
            BaseGameType::Race => GameTypeOptions::Race {
                race_type: code(b, 0x4C, "race type", RaceType::from_code)?,
                team_scoring: code(b, 0x50, "team scoring", TeamScoring::from_code)?,
            },
        };
        let health = f32::from_bits(u32_at(b, 0x3C));
        if !health.is_finite() {
            return Err(bad("the health is not a number"));
        }
        Ok(GameVariant {
            name,
            rules: Rules {
                team_play: flag(0x1C),
                flags: GameFlags(u32_at(b, 0x20)),
                objective_indicator: code(b, 0x24, "objective indicator", ObjectiveIndicator::from_code)?,
                odd_man_out: flag(0x28),
                respawn_time_growth: i32_at(b, 0x2C),
                respawn_time: i32_at(b, 0x30),
                suicide_penalty: i32_at(b, 0x34),
                number_of_lives: i32_at(b, 0x38),
                health,
                score_to_win: i32_at(b, 0x40),
                weapon_set: code(b, 0x44, "weapon set", WeaponSet::from_code)?,
                vehicle_set: code(b, 0x48, "vehicle set", VehicleSet::from_code)?,
                options,
                variant_flags: u16::from_le_bytes([b[0x64], b[0x65]]),
            },
        })
    }

    /// The 0x68-byte record. The name is cut to `NAME_CHARS` UTF-16 units;
    /// the union bytes a base type does not use and the pads are zero.
    pub fn to_bytes(&self) -> [u8; VARIANT_SIZE] {
        let r = &self.rules;
        let mut b = [0u8; VARIANT_SIZE];
        for (i, u) in self.name.encode_utf16().take(NAME_CHARS).enumerate() {
            b[2 * i..2 * i + 2].copy_from_slice(&u.to_le_bytes());
        }
        let mut put = |o: usize, v: u32| b[o..o + 4].copy_from_slice(&v.to_le_bytes());
        put(0x18, r.base().code() as u32);
        put(0x20, r.flags.bits());
        put(0x24, r.objective_indicator.code() as u32);
        put(0x2C, r.respawn_time_growth as u32);
        put(0x30, r.respawn_time as u32);
        put(0x34, r.suicide_penalty as u32);
        put(0x38, r.number_of_lives as u32);
        put(0x3C, r.health.to_bits());
        put(0x40, r.score_to_win as u32);
        put(0x44, r.weapon_set.code() as u32);
        put(0x48, r.vehicle_set.code() as u32);
        match r.options {
            GameTypeOptions::CaptureTheFlag { single_flag_time, .. } => put(0x50, single_flag_time as u32),
            GameTypeOptions::Oddball {
                speed_with_ball,
                trait_with_ball,
                trait_without_ball,
                ball_type,
                ball_spawn_count,
                ..
            } => {
                put(0x50, speed_with_ball.code() as u32);
                put(0x54, trait_with_ball.code() as u32);
                put(0x58, trait_without_ball.code() as u32);
                put(0x5C, ball_type.code() as u32);
                put(0x60, ball_spawn_count as u32);
            }
            GameTypeOptions::Race {
                race_type,
                team_scoring,
            } => {
                put(0x4C, race_type.code() as u32);
                put(0x50, team_scoring.code() as u32);
            }
            GameTypeOptions::Slayer { .. } | GameTypeOptions::KingOfTheHill { .. } => {}
        }
        b[0x1C] = r.team_play as u8;
        b[0x28] = r.odd_man_out as u8;
        match r.options {
            GameTypeOptions::CaptureTheFlag {
                assault,
                reset_on_capture,
                flag_must_reset,
                flag_at_home_to_score,
                ..
            } => {
                b[0x4C] = assault as u8;
                b[0x4D] = reset_on_capture as u8;
                b[0x4E] = flag_must_reset as u8;
                b[0x4F] = flag_at_home_to_score as u8;
            }
            GameTypeOptions::Slayer {
                no_death_bonus,
                no_kill_penalty,
                kill_in_order,
            } => {
                b[0x4C] = no_death_bonus as u8;
                b[0x4D] = no_kill_penalty as u8;
                b[0x4E] = kill_in_order as u8;
            }
            GameTypeOptions::Oddball {
                random_start,
                ball_spawn_delay,
                ..
            } => {
                b[0x4C] = random_start as u8;
                b[0x4D] = ball_spawn_delay as u8;
            }
            GameTypeOptions::KingOfTheHill { moving_hill } => b[0x4C] = moving_hill as u8,
            GameTypeOptions::Race { .. } => {}
        }
        b[0x64..0x66].copy_from_slice(&r.variant_flags.to_le_bytes());
        b
    }
}

/// One stock game type: its name and rules.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StockGameType {
    pub name: &'static str,
    pub rules: Rules,
    /// Set where the spec says the disc's value is the builder's garbage and
    /// the table gives the intent (Team Ball's flags).
    pub flags_uncertain: bool,
}

impl StockGameType {
    /// The stock type as a saved variant.
    pub fn variant(&self) -> GameVariant {
        GameVariant {
            name: self.name.to_owned(),
            rules: self.rules,
        }
    }
}

const fn secs(s: i32) -> i32 {
    s * TICKS
}

#[allow(clippy::too_many_arguments)]
const fn stock(
    name: &'static str,
    teams: bool,
    flags: u32,
    respawn_s: i32,
    growth_s: i32,
    suicide_s: i32,
    lives: i32,
    health: f32,
    score: i32,
    indicator: ObjectiveIndicator,
    weapon_set: WeaponSet,
    vehicle_set: VehicleSet,
    odd_man_out: bool,
    options: GameTypeOptions,
) -> StockGameType {
    StockGameType {
        name,
        rules: Rules {
            team_play: teams,
            flags: GameFlags(flags),
            objective_indicator: indicator,
            odd_man_out,
            respawn_time_growth: secs(growth_s),
            respawn_time: secs(respawn_s),
            suicide_penalty: secs(suicide_s),
            number_of_lives: lives,
            health,
            score_to_win: score,
            weapon_set,
            vehicle_set,
            options,
            variant_flags: 1,
        },
        flags_uncertain: false,
    }
}

const fn slayer(no_death_bonus: bool, no_kill_penalty: bool, kill_in_order: bool) -> GameTypeOptions {
    GameTypeOptions::Slayer {
        no_death_bonus,
        no_kill_penalty,
        kill_in_order,
    }
}

const fn oddball(
    ball_type: BallType,
    balls: i32,
    speed: BallSpeed,
    with_ball: OddballTrait,
    without_ball: OddballTrait,
) -> GameTypeOptions {
    GameTypeOptions::Oddball {
        random_start: false,
        ball_spawn_delay: false,
        speed_with_ball: speed,
        trait_with_ball: with_ball,
        trait_without_ball: without_ball,
        ball_type,
        ball_spawn_count: balls,
    }
}

const fn ctf(assault: bool, must_reset: bool, at_home: bool) -> GameTypeOptions {
    GameTypeOptions::CaptureTheFlag {
        assault,
        reset_on_capture: false,
        flag_must_reset: must_reset,
        flag_at_home_to_score: at_home,
        single_flag_time: 0,
    }
}

const fn king(moving_hill: bool) -> GameTypeOptions {
    GameTypeOptions::KingOfTheHill { moving_hill }
}

const fn race(race_type: RaceType) -> GameTypeOptions {
    GameTypeOptions::Race {
        race_type,
        team_scoring: TeamScoring::Minimum,
    }
}

use BallSpeed::{Fast, Slow};
use ObjectiveIndicator::{MotionTracker as M, NavPoints as N, None as NoIndicator};
use OddballTrait::{DamageResistant, ExtraDamage, Invisible, None as NoTrait};
use VehicleSet::{None as NoVehicles, Scorpion, Warthog};
use WeaponSet::{Normal, RocketLaunchers, Sniper};

/// The 26 stock game types in the order of section 2's table (and of the
/// executable's builders). Times are the table's seconds in ticks; lives 0
/// is infinite; health 1.0 is the table's 100 percent.
pub const STOCK_GAME_TYPES: [StockGameType; STOCK_COUNT] = [
    stock(
        "Slayer",
        false,
        3,
        0,
        0,
        10,
        0,
        1.0,
        15,
        M,
        Normal,
        Warthog,
        false,
        slayer(false, false, false),
    ),
    stock(
        "Slayer Pro",
        false,
        35,
        0,
        0,
        15,
        0,
        1.0,
        25,
        M,
        Normal,
        Warthog,
        false,
        slayer(true, true, false),
    ),
    stock(
        "Elimination",
        false,
        3,
        0,
        0,
        10,
        1,
        1.0,
        25,
        M,
        Normal,
        Warthog,
        false,
        slayer(false, false, false),
    ),
    stock(
        "Phantoms",
        false,
        18,
        5,
        0,
        5,
        0,
        1.0,
        10,
        N,
        Normal,
        Warthog,
        false,
        slayer(true, true, true),
    ),
    stock(
        "Endurance",
        false,
        3,
        0,
        10,
        10,
        5,
        1.0,
        10,
        M,
        Normal,
        Warthog,
        true,
        slayer(false, false, false),
    ),
    stock(
        "Rockets",
        false,
        34,
        0,
        0,
        10,
        0,
        1.0,
        25,
        N,
        RocketLaunchers,
        Warthog,
        false,
        slayer(false, false, false),
    ),
    stock(
        "Snipers",
        false,
        34,
        0,
        5,
        10,
        0,
        1.0,
        15,
        N,
        Sniper,
        Warthog,
        false,
        slayer(false, false, false),
    ),
    stock(
        "Oddball",
        false,
        3,
        5,
        0,
        5,
        0,
        1.0,
        2,
        N,
        Normal,
        NoVehicles,
        false,
        oddball(BallType::Normal, 1, Slow, NoTrait, NoTrait),
    ),
    stock(
        "Reverse Tag",
        false,
        2,
        5,
        0,
        5,
        0,
        1.0,
        2,
        N,
        Normal,
        NoVehicles,
        false,
        oddball(BallType::ReverseTag, 1, Slow, NoTrait, NoTrait),
    ),
    stock(
        "Accumulate",
        false,
        2,
        5,
        0,
        5,
        0,
        1.0,
        5,
        NoIndicator,
        Normal,
        NoVehicles,
        false,
        oddball(BallType::ReverseTag, 16, Slow, NoTrait, NoTrait),
    ),
    stock(
        "Juggernaut",
        false,
        3,
        5,
        0,
        5,
        0,
        1.0,
        10,
        N,
        Normal,
        Warthog,
        false,
        oddball(BallType::Juggernaut, 1, Slow, ExtraDamage, NoTrait),
    ),
    stock(
        "Stalker",
        false,
        2,
        5,
        0,
        5,
        0,
        1.0,
        10,
        M,
        Normal,
        Warthog,
        false,
        oddball(BallType::Juggernaut, 1, Fast, Invisible, DamageResistant),
    ),
    stock(
        "King",
        false,
        3,
        5,
        0,
        5,
        0,
        1.0,
        2,
        N,
        Normal,
        Warthog,
        false,
        king(false),
    ),
    stock(
        "King Pro",
        false,
        35,
        10,
        0,
        15,
        0,
        1.0,
        2,
        N,
        Normal,
        Warthog,
        false,
        king(false),
    ),
    stock(
        "Crazy King",
        false,
        3,
        0,
        0,
        5,
        0,
        1.0,
        2,
        N,
        Normal,
        Warthog,
        false,
        king(true),
    ),
    stock(
        "Race",
        false,
        3,
        0,
        0,
        10,
        0,
        1.0,
        3,
        N,
        Normal,
        Warthog,
        false,
        race(RaceType::Normal),
    ),
    stock(
        "Rally",
        false,
        3,
        0,
        0,
        10,
        0,
        1.0,
        15,
        N,
        Normal,
        Warthog,
        false,
        race(RaceType::Rally),
    ),
    stock(
        "CTF",
        true,
        3,
        10,
        0,
        5,
        0,
        1.0,
        3,
        N,
        Normal,
        Warthog,
        false,
        ctf(false, false, false),
    ),
    stock(
        "Invasion",
        true,
        3,
        0,
        0,
        5,
        5,
        1.0,
        3,
        N,
        Normal,
        Warthog,
        false,
        ctf(true, false, false),
    ),
    stock(
        "Iron CTF",
        true,
        3,
        15,
        0,
        5,
        0,
        2.0,
        3,
        N,
        Normal,
        Scorpion,
        false,
        ctf(false, true, false),
    ),
    stock(
        "CTF Pro",
        true,
        35,
        10,
        0,
        15,
        0,
        1.0,
        3,
        N,
        Normal,
        Warthog,
        false,
        ctf(false, false, true),
    ),
    stock(
        "Team Race",
        true,
        3,
        0,
        0,
        10,
        0,
        1.0,
        3,
        N,
        Normal,
        Warthog,
        false,
        race(RaceType::Normal),
    ),
    stock(
        "Team Rally",
        true,
        3,
        0,
        0,
        10,
        0,
        1.0,
        5,
        N,
        Normal,
        Warthog,
        false,
        race(RaceType::Rally),
    ),
    StockGameType {
        flags_uncertain: true,
        ..stock(
            "Team Ball",
            true,
            35,
            10,
            0,
            5,
            0,
            1.0,
            2,
            N,
            Normal,
            NoVehicles,
            false,
            oddball(BallType::Normal, 1, Slow, NoTrait, NoTrait),
        )
    },
    stock(
        "Team King",
        true,
        3,
        10,
        0,
        5,
        0,
        1.0,
        2,
        N,
        Normal,
        Warthog,
        false,
        king(true),
    ),
    stock(
        "Team Slayer",
        true,
        3,
        10,
        0,
        10,
        0,
        1.0,
        50,
        M,
        Normal,
        Warthog,
        false,
        slayer(false, false, false),
    ),
];
