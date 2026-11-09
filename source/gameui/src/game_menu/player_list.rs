//! The player list (CS:S's `CPlayerListDialog`, the game menu's "Player
//! List" in a game; `PlayerListDialog.res`): every player by name with
//! its properties ("Muted"), one selected at a time; "Mute in-game voice"
//! (the layout's `MuteButton`, the default button) mutes the selected
//! player's voice for the session, and once muted reads "Unmute in-game
//! voice" (`voice_mute` / `voice_unmute <userid>`, `net::voice::VoiceMutes`);
//! Close. The local player and bots can't be muted (the button greys).
//!
//! Known shortcuts (docs/tech-debt.md): Player list (`game_menu::player_list`):
//! the properties column's words ("Muted", "Bot") and the title's server name
//! (`hostname` as this process knows it) are guesses (CS:S sets them in code:
//! capture C-voice); no friends column (no Steam).

use super::*;

/// A player as the player list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedPlayer {
    /// The user id (`NetCharacter::owner`); None for a bot.
    pub id: Option<u64>,
    pub name: String,
    /// This client's own player.
    pub me: bool,
    /// Muted here (`VoiceMutes`).
    pub muted: bool,
}

impl ListedPlayer {
    /// Whether its voice can be muted: another player, not a bot.
    pub fn mutable(&self) -> bool {
        self.id.is_some() && !self.me
    }
}

impl GameMenu {
    /// The player list's rows: a row per player, Mute, Close.
    pub(super) fn player_list_rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = self
            .roster
            .iter()
            .map(|p| Row::Player {
                name: p.name.clone(),
                properties: if p.muted {
                    "Muted".into()
                } else if p.id.is_none() && !p.me {
                    "Bot".into()
                } else {
                    String::new()
                },
            })
            .collect();
        let selected = self.selected_player();
        let muted = selected.is_some_and(|p| p.muted);
        rows.push(Row::Button {
            label: if muted {
                self.text("#GameUI_UnmuteIngameVoice", "Unmute in-game voice")
            } else {
                self.text("#GameUI_MuteIngameVoice", "Mute in-game voice")
            },
            action: self.mute_action().unwrap_or(Action::Run(String::new(), false)),
            enabled: selected.is_some_and(ListedPlayer::mutable),
        });
        rows.push(Self::button_row(&self.text("#GameUI_Close", "Close"), Action::Back));
        rows
    }

    /// The selected player (`key_row`), if any.
    pub(super) fn selected_player(&self) -> Option<&ListedPlayer> {
        self.key_row.and_then(|r| self.roster.get(r))
    }

    /// Mute or unmute the selected player (None: nobody it can mute).
    pub(super) fn mute_action(&self) -> Option<Action> {
        let p = self.selected_player().filter(|p| p.mutable())?;
        let verb = if p.muted { "voice_unmute" } else { "voice_mute" };
        Some(Action::Run(format!("{verb} {}", p.id?), false))
    }

    /// The players now (`roster`); the selection follows its player.
    pub fn set_roster(&mut self, roster: Vec<ListedPlayer>) {
        if self.roster == roster {
            return;
        }
        let selected = self.selected_player().map(|p| (p.id, p.name.clone()));
        self.roster = roster;
        if self.page == Page::PlayerList {
            self.key_row = selected.and_then(|s| self.roster.iter().position(|p| (p.id, p.name.clone()) == s));
            let n = self.rows().len();
            if self.focus >= n {
                self.focus = self.first_focusable_from(0, 1);
            }
        }
    }

    /// The player list's title: CS:S's "CURRENT PLAYERS - %server%".
    pub(super) fn player_list_title(&self) -> String {
        match self.game_text("#GameUI_PlayerListDialogTitle") {
            Some(t) if !self.server_name.is_empty() => t.replace("%server%", &self.server_name),
            _ => self.text("#GameUI_CurrentPlayers", "Current Players"),
        }
    }
}
