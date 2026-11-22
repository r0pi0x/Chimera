//! The lobby's data (specs/combat_arms/lobby.md D, C, O, S): the modes
//! with their ranges and option rules, the maps and the modes each
//! offers, the arms options and the default room titles, as
//! `data/lobby.toml` (ours: the install's room data file is not read).

use std::sync::{Arc, OnceLock};

use serde::Deserialize;

/// data/lobby.toml, built in.
pub const DATA: chimera_core::game_data::GameData = chimera_core::game_data!("data/lobby.toml");

/// Rooms per channel: ids 0-99 (the list shows id + 1).
pub const ROOMS_PER_CHANNEL: u16 = 100;
/// Hard cap for any room.
pub const ROOM_PLAYERS_MAX: u32 = 16;
/// The longest room name the service stores (longer: the room is unnamed).
pub const ROOM_NAME_MAX: usize = 32;
/// The longest password the service stores.
pub const ROOM_PASSWORD_MAX: usize = 8;
/// One-team modes put the first 8 players on Alpha.
pub const ONE_SIDE_ALPHA: usize = 8;
/// Countdown messages before the match may launch.
pub const COUNTDOWN_TICKS: u32 = 5;
/// At this count the service tells the launcher to stop the game server
/// (5 + the service's setting 10).
pub const LAUNCH_TIMEOUT: u32 = 15;
/// At this count the room returns to waiting (15 + the setting 2).
pub const LAUNCH_GIVE_UP: u32 = 17;
/// After the last countdown message, a starting player with no "go" gives
/// up (ms).
pub const CLIENT_GO_TIMEOUT_MS: f64 = 15_000.0;
/// No input for this long in the waiting room: idle (ms).
pub const IDLE_AFTER_MS: f64 = 60_000.0;
/// Lobby players reached by one invite.
pub const INVITE_MAX: usize = 5;
/// The longest chat line the service accepts.
pub const CHAT_MAX: usize = 120;
/// The result screen returns to the room on its own (ms).
pub const RESULT_TIMEOUT_MS: f64 = 60_000.0;
/// The random-map pseudo-map's id.
pub const RANDOM_MAP: u16 = 999;
/// The mode whose room is a Go4CA Cup one (it forces its arms option).
pub const CUP_ARMS: u8 = 10;

/// An arms option's kind (D): "only" keeps what it names, "except" drops
/// it.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ArmsKind {
    None,
    Only,
    Except,
}

/// One entry of `[ArmsOption]`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ArmsOption {
    pub id: u8,
    pub name: String,
    pub kind: ArmsKind,
    pub order: u32,
    /// False: in no list (Duty Roster, quirk 8).
    #[serde(default = "yes")]
    pub listed: bool,
    /// Listed only in the Go4CA Cup mode.
    #[serde(default)]
    pub cup: bool,
}

/// A map (`[Map#]`).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MapRec {
    pub id: u16,
    /// The world's name in the install.
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub size: String,
    /// The room keys it offers.
    pub modes: Vec<String>,
}

/// The lobby's start rule of a mode (S).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct StartRule {
    /// The host's team, then the other team: ready players each needs.
    pub teams: Option<[u32; 2]>,
    /// All ready players together.
    pub total: Option<u32>,
    /// Every player must be on the cup's allowed list (not modelled: no
    /// list offline).
    #[serde(default)]
    pub cup: bool,
}

/// A mode (`[Mode#]`).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ModeRec {
    pub id: u16,
    pub key: String,
    pub name: String,
    pub game_type: u32,
    pub sort: u32,
    pub default_map: u16,
    #[serde(default = "yes")]
    pub enable_play: bool,
    #[serde(default)]
    pub single_play: bool,
    /// The lobby has rules for it (modes 20-25 have none).
    #[serde(default = "yes")]
    pub rules: bool,
    #[serde(default)]
    pub one_team: bool,
    /// Min, max, step, recommended players.
    pub players: [u32; 4],
    /// Min, max, step, default goal.
    pub goal: [u32; 4],
    /// The lobby's own wider goal limits (open question 13).
    pub goal_lobby_min: Option<u32>,
    pub goal_lobby_max: Option<u32>,
    pub goal_label: String,
    #[serde(default)]
    pub weapon_use: bool,
    #[serde(default)]
    pub join_combat: bool,
    #[serde(default)]
    pub team_kill: bool,
    #[serde(default)]
    pub view_enemy: bool,
    #[serde(default)]
    pub mutation: bool,
    /// `EnablePerkSkillSetting`: absent means on.
    #[serde(default = "yes")]
    pub perk: bool,
    #[serde(default)]
    pub refuse_ff: bool,
    #[serde(default)]
    pub refuse_cam: bool,
    #[serde(default)]
    pub refuse_perk: bool,
    #[serde(default)]
    pub refuse_arms: bool,
    #[serde(default)]
    pub cam_default: bool,
    #[serde(default = "yes")]
    pub perk_default: bool,
    #[serde(default)]
    pub arms_default: u8,
    /// "Swap Teams" shows On (VIP Escort; the control is dead, quirk 7).
    #[serde(default)]
    pub swap_display: bool,
    pub start: StartRule,
}

fn yes() -> bool {
    true
}

impl ModeRec {
    /// The minimum, maximum and step of the players list.
    pub fn min_players(&self) -> u32 {
        self.players[0]
    }
    pub fn max_players(&self) -> u32 {
        self.players[1]
    }
    pub fn players_inc(&self) -> u32 {
        self.players[2]
    }
    pub fn recommended(&self) -> u32 {
        self.players[3]
    }
    /// The goal list's values and default.
    pub fn goal_values(&self) -> Vec<u32> {
        let [min, max, inc, _] = self.goal;
        let inc = inc.max(1);
        (0..)
            .map(|i| min + i * inc)
            .take_while(|g| *g <= max)
            .collect()
    }
    pub fn goal_default(&self) -> u32 {
        self.goal[3]
    }
    /// The lobby's goal range (wider than the client's in places).
    pub fn lobby_goal_range(&self) -> (u32, u32) {
        (self.goal_lobby_min.unwrap_or(self.goal[0]), self.goal_lobby_max.unwrap_or(self.goal[1]))
    }
    /// The players list: min to max in steps.
    pub fn player_values(&self) -> Vec<u32> {
        let inc = self.players_inc().max(1);
        (0..)
            .map(|i| self.min_players() + i * inc)
            .take_while(|p| *p <= self.max_players())
            .collect()
    }
    /// The goal row is a difficulty (Fireteam).
    pub fn goal_is_difficulty(&self) -> bool {
        self.goal_label == "Difficulty"
    }
    /// Offered in the multiplayer tab.
    pub fn offered_multi(&self) -> bool {
        self.enable_play && self.rules && !self.single_play
    }
    /// Offered in the single tab.
    pub fn offered_single(&self) -> bool {
        self.enable_play && self.rules && self.single_play
    }
    /// A team mode (two teams, balanced by the lobby).
    pub fn two_team(&self) -> bool {
        !self.one_team
    }
    /// The goal list entry's text (C): "N Kills", "N Rounds", "N Min.",
    /// "N Points", NORMAL / HARD / EXTREME for a difficulty; the default
    /// entry " (Default)".
    pub fn goal_text(&self, value: u32) -> String {
        if self.goal_is_difficulty() {
            return ["NORMAL", "HARD", "EXTREME"].get(value as usize).copied().unwrap_or("?").to_string();
        }
        let base = format!("{value} {}", self.goal_label);
        if value == self.goal_default() { format!("{base} (Default)") } else { base }
    }
    /// The players entry's text (C): "n/2 vs n/2" for a team mode, "n
    /// Player(s)", "1 vs n" for bot play.
    pub fn players_text(&self, n: u32) -> String {
        if self.single_play && self.two_team() {
            format!("1 vs {n}")
        } else if self.two_team() {
            format!("{} vs {}", n / 2, n / 2)
        } else {
            format!("{n} Player(s)")
        }
    }
}

/// The whole table.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct LobbyData {
    pub titles: Vec<String>,
    pub arms: Vec<ArmsOption>,
    #[serde(rename = "map")]
    pub maps: Vec<MapRec>,
    #[serde(rename = "mode")]
    pub modes: Vec<ModeRec>,
}

impl LobbyData {
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| format!("lobby.toml: {e}"))
    }

    /// The built-in table, once per process.
    pub fn built_in() -> Arc<Self> {
        static DATA_ONCE: OnceLock<Arc<LobbyData>> = OnceLock::new();
        DATA_ONCE
            .get_or_init(|| Arc::new(Self::parse(&DATA).expect("data/lobby.toml parses")))
            .clone()
    }

    pub fn mode(&self, id: u16) -> Option<&ModeRec> {
        self.modes.iter().find(|m| m.id == id)
    }

    pub fn mode_by_key(&self, key: &str) -> Option<&ModeRec> {
        self.modes.iter().find(|m| m.key == key)
    }

    pub fn map(&self, id: u16) -> Option<&MapRec> {
        self.maps.iter().find(|m| m.id == id)
    }

    pub fn arms_option(&self, id: u8) -> Option<&ArmsOption> {
        self.arms.iter().find(|a| a.id == id)
    }

    /// The modes offered in a tab, by `sort` ascending (D).
    pub fn modes_offered(&self, single: bool) -> Vec<&ModeRec> {
        let mut v: Vec<&ModeRec> = self
            .modes
            .iter()
            .filter(|m| if single { m.offered_single() } else { m.offered_multi() })
            .collect();
        v.sort_by_key(|m| m.sort);
        v
    }

    /// Whether `map` is open for `mode` (its record offers the mode's key).
    pub fn map_open(&self, map: u16, mode: &ModeRec) -> bool {
        self.map(map).is_some_and(|m| m.modes.iter().any(|k| *k == mode.key))
    }

    /// The maps a mode offers, sorted by name (D).
    pub fn maps_for(&self, mode: &ModeRec) -> Vec<&MapRec> {
        let mut v: Vec<&MapRec> = self.maps.iter().filter(|m| m.modes.iter().any(|k| *k == mode.key)).collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    /// The arms options a dropdown lists, by display order (Duty Roster
    /// never; the cup's only in its mode).
    pub fn arms_listed(&self, cup: bool) -> Vec<&ArmsOption> {
        let mut v: Vec<&ArmsOption> = self.arms.iter().filter(|a| a.listed && (!a.cup || cup)).collect();
        v.sort_by_key(|a| a.order);
        v
    }

    /// The name of a map id ("Random Map" for the pseudo-map).
    pub fn map_name(&self, id: u16) -> String {
        if id == RANDOM_MAP {
            return "Random Map".into();
        }
        self.map(id).map_or_else(|| "?".into(), |m| m.name.clone())
    }

    /// The default title for a clock reading (C: picked by the
    /// millisecond clock).
    pub fn default_title(&self, now_ms: u64) -> String {
        self.titles[(now_ms % self.titles.len() as u64) as usize].clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_parses() {
        let d = LobbyData::built_in();
        assert_eq!(d.modes.len(), 26);
        assert_eq!(d.modes_offered(false).len(), 17);
        assert_eq!(d.modes_offered(true).len(), 2);
        // Every mode's default map offers it (except the room-data-only ones).
        for m in d.modes.iter().filter(|m| m.rules && m.enable_play) {
            assert!(d.map_open(m.default_map, m), "{} on its default map", m.key);
        }
    }
}
