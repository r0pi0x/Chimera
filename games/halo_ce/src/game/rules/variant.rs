//! The game type a match plays, as data (specs/halo_ce/multiplayer_rules.md
//! 1, 2): a game variant (`blam_formats::game_type`), chosen by name from
//! the 26 stock game types the disc's executable builds
//! (`STOCK_GAME_TYPES`; the disc holds no saved game type files: section 2
//! says the stock ones are built in code), by a base type's word for its
//! stock default (`slayer`, `team_slayer`, `ctf`, `king`, `oddball`,
//! `race`), or read from a saved game type file (a path: the 0x68-byte
//! record at the start of a 512-byte file, section 1.1); then cleaned as
//! the game cleans any variant it is given (1.4).

use blam_formats::game_type::{
    BaseGameType, GameTypeOptions, GameVariant, Rules, STOCK_GAME_TYPES, StockGameType,
};

/// The stock game type a base word names (its stock default: the first
/// row of section 2 with that base type and teams).
pub fn default_for(word: &str) -> Option<&'static StockGameType> {
    let name = match word {
        "slayer" | "ffa" => "Slayer",
        "team_slayer" | "tslayer" => "Team Slayer",
        "ctf" | "capture_the_flag" => "CTF",
        "king" | "koth" | "king_of_the_hill" => "King",
        "team_king" => "Team King",
        "oddball" | "ball" => "Oddball",
        "team_ball" | "team_oddball" => "Team Ball",
        "race" => "Race",
        "team_race" => "Team Race",
        _ => return None,
    };
    STOCK_GAME_TYPES.iter().find(|s| s.name == name)
}

/// A name's comparable form: lower case, spaces and dashes as `_`.
fn key(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

/// The variant `name` names: a base word, a stock game type's name (any
/// case; spaces as `_`: `slayer_pro`, `iron_ctf`), or a saved file's
/// path. None: nothing by that name.
pub fn by_name(name: &str) -> Result<GameVariant, String> {
    let k = key(name);
    if let Some(s) = default_for(&k) {
        return Ok(s.variant());
    }
    if let Some(s) = STOCK_GAME_TYPES.iter().find(|s| key(s.name) == k) {
        return Ok(s.variant());
    }
    let path = std::path::Path::new(name.trim());
    if path.is_file() {
        let bytes = std::fs::read(path).map_err(|e| format!("{name}: {e}"))?;
        return GameVariant::from_bytes(&bytes).map_err(|e| format!("{name}: {e}"));
    }
    let names: Vec<String> = STOCK_GAME_TYPES.iter().map(|s| key(s.name)).collect();
    Err(format!("{name}: no such game type (a base word: slayer, team_slayer, ctf, king, oddball, race; a stock name: {}; or a saved game type file)", names.join(", ")))
}

/// A variant as the game uses it (section 1.4): growth, respawn time,
/// suicide penalty and lives at least 0, health 0.25 to 4, capture the flag
/// with teams, the oddball's ball count 0 to 16. (The enumerations are
/// already inside their tables: `GameVariant::from_bytes` refuses others.)
pub fn clean(mut r: Rules) -> Rules {
    r.respawn_time_growth = r.respawn_time_growth.max(0);
    r.respawn_time = r.respawn_time.max(0);
    r.suicide_penalty = r.suicide_penalty.max(0);
    r.number_of_lives = r.number_of_lives.max(0);
    r.health = if r.health.is_finite() { r.health.clamp(0.25, 4.0) } else { 1.0 };
    if r.base() == BaseGameType::CaptureTheFlag {
        r.team_play = true;
    }
    if let GameTypeOptions::Oddball { ball_spawn_count, .. } = &mut r.options {
        *ball_spawn_count = (*ball_spawn_count).clamp(0, 16);
    }
    r
}

/// The mode key of a variant's base type and teams (`rules::MODES`).
pub fn mode_key(r: &Rules) -> &'static str {
    match (r.base(), r.team_play) {
        (BaseGameType::Slayer, false) => "slayer",
        (BaseGameType::Slayer, true) => "team_slayer",
        (BaseGameType::CaptureTheFlag, _) => "ctf",
        (BaseGameType::KingOfTheHill, false) => "king",
        (BaseGameType::KingOfTheHill, true) => "team_king",
        (BaseGameType::Oddball, false) => "oddball",
        (BaseGameType::Oddball, true) => "team_oddball",
        (BaseGameType::Race, false) => "race",
        (BaseGameType::Race, true) => "team_race",
    }
}

/// The game type of `blam_formats::scenario` a variant's base type is (the
/// eligibility of starts, items and equipment).
pub fn game_type(r: &Rules) -> blam_formats::scenario::GameType {
    use blam_formats::scenario::GameType;
    match r.base() {
        BaseGameType::CaptureTheFlag => GameType::CaptureTheFlag,
        BaseGameType::Slayer => GameType::Slayer,
        BaseGameType::Oddball => GameType::Oddball,
        BaseGameType::KingOfTheHill => GameType::KingOfTheHill,
        BaseGameType::Race => GameType::Race,
    }
}

/// Whether the score to win counts minutes (oddball types 0 and 1, king
/// of the hill; section 2's notes): the limit in ticks is then x 1800.
pub fn score_in_minutes(r: &Rules) -> bool {
    match r.options {
        GameTypeOptions::KingOfTheHill { .. } => true,
        GameTypeOptions::Oddball { ball_type, .. } => ball_type != blam_formats::game_type::BallType::Juggernaut,
        _ => false,
    }
}

/// The score that wins, in the unit the game type scores in (ticks for a
/// game in minutes).
pub fn winning_score(r: &Rules) -> i32 {
    if score_in_minutes(r) { r.score_to_win.saturating_mul(1800) } else { r.score_to_win }
}
