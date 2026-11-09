//! Create Server: its Server, Game and Bot pages, and the console lines
//! a new game starts with.

use super::*;

/// The difficulty buttons' default (`BotMenu::difficulties`).
pub(super) const NORMAL: usize = 1;
/// Bots a new game offers at most (the box takes two digits).
pub(super) const MAX_BOTS: u8 = 30;

/// Create Server's pages: the token of each tab and our word.
pub const CREATE_TABS: [(&str, &str); 3] = [("#GameUI_Server", "Server"), ("#GameUI_Game", "Game"), ("", "Bot")];

/// A cvar Create Server sets when it starts (the Game page's options
/// from the install's `settings.scr`, the Bot page's cvars, ours): its
/// value shown and the one before.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerCvar {
    pub cvar: String,
    pub label: String,
    pub kind: ServerSettingKind,
    pub value: String,
    /// Its value when the dialog opened (None: Chimera lacks it, greyed).
    pub before: Option<String>,
    /// The page it's on (index into `CREATE_TABS`).
    pub page: usize,
    /// The layout control showing it (Bot page), else a row of the Game
    /// page's list.
    pub field: Option<&'static str>,
}

/// What a new game starts with.
#[derive(Clone, Debug, PartialEq)]
pub struct NewGame {
    /// Index into `GameMenu::maps`.
    pub map: usize,
    /// `< Random Map >` picked: Start plays one of the maps at random
    /// (`GameMenu::random_seed`) instead of `map`.
    pub random: bool,
    /// Include bots, how many (typed), which team they join (`BOT_TEAMS`).
    pub bots_on: bool,
    pub bot_count: String,
    pub bot_team: usize,
    /// Index into `DIFFICULTIES`.
    pub difficulty: usize,
    /// Server Type (index into `SERVER_TYPES`).
    pub visibility: usize,
    /// The Game and Bot pages' cvars.
    pub cvars: Vec<ServerCvar>,
    /// Host a server others can join (Create Server); false: the quick
    /// start's single-player game.
    pub host: bool,
}

impl Default for NewGame {
    fn default() -> Self {
        Self {
            map: 0,
            random: false,
            bots_on: false,
            bot_count: "0".into(),
            bot_team: 0,
            difficulty: NORMAL,
            visibility: 0,
            cvars: Vec::new(),
            host: true,
        }
    }
}

impl NewGame {
    /// The map list's entry shown: 0 `< Random Map >`, else the map's
    /// index + 1 (None without maps).
    pub fn map_entry(&self, maps: usize) -> Option<usize> {
        (maps > 0).then(|| if self.random { 0 } else { self.map.min(maps - 1) + 1 })
    }

    /// Pick an entry of the map list (`map_entry`).
    pub fn set_map_entry(&mut self, k: usize) {
        self.random = k == 0;
        if k > 0 {
            self.map = k - 1;
        }
    }

    /// Bots per team: terrorists, counter-terrorists (any team: split,
    /// the odd one a terrorist).
    pub fn split(&self) -> (u8, u8) {
        if !self.bots_on {
            return (0, 0);
        }
        let n = self.bot_count.trim().parse::<u8>().unwrap_or(0).min(MAX_BOTS);
        match self.bot_team {
            1 => (n, 0),
            2 => (0, n),
            _ => (n.div_ceil(2), n / 2),
        }
    }
}

/// Create Server's Game page: the game's server options (`settings.scr`)
/// in a scrolled list, each its label and control (CS:S's
/// `CPanelListPanel`, where the layout puts it).
pub(super) fn game_options(commands: &mut Commands, ctx: &Ctx, parent: Entity, (w, h): (f32, f32)) {
    let menu = ctx.menu;
    let at = menu
        .layout()
        .and_then(|l| l.get("GameOptions"))
        .map(|c| (coord(c.x), coord(c.y), c.wide, c.tall));
    options_list(commands, ctx, parent, at.unwrap_or((10.0, 12.0, w - 20.0, h - 24.0)));
}

/// The console line a Game page row becomes (the options script reader's
/// formats): a check box `"1"` or `"0"`, a number as a decimal with six
/// places clamped to the row's limits, a text quoted (characters a command
/// can't hold removed), a list's selected value (`"0"` when none matches).
pub fn scr_line(c: &ServerCvar, value: &str) -> String {
    let v = match &c.kind {
        ServerSettingKind::Bool => (value.trim().parse::<f32>().is_ok_and(|v| v != 0.0) as u8).to_string(),
        ServerSettingKind::Number { min, max } => {
            let mut n = value.trim().parse::<f32>().unwrap_or(0.0);
            if let Some(min) = min {
                n = n.max(*min);
            }
            if let Some(max) = max {
                n = n.min(*max);
            }
            format!("{n:.6}")
        }
        ServerSettingKind::List(items) => items
            .iter()
            .find(|(_, v)| v.trim() == value.trim())
            .map_or("0".to_string(), |(_, v)| v.clone()),
        ServerSettingKind::Text => value.chars().filter(|c| !matches!(c, '"' | ';' | '\n' | '\r')).collect(),
    };
    format!("{} \"{v}\"", c.cvar)
}

impl GameMenu {
    /// Create Server's cvars: the install's Game page options (else none),
    /// ours, the Bot page's; each with its value now.
    pub(super) fn server_cvars(&self, get: &dyn Fn(&str) -> Option<String>) -> Vec<ServerCvar> {
        let mut out: Vec<ServerCvar> = Vec::new();
        if let Some(ui) = self.ui.0.as_ref() {
            for s in &ui.server_settings {
                let before = get(&s.cvar);
                // Create Server always hosts a server others can join (as
                // CS:S's does): a single-player `maxplayers` (1) shows the
                // game's default instead.
                let single = s.cvar == "maxplayers"
                    && before.as_deref().and_then(|v| v.trim().parse::<u32>().ok()).is_some_and(|n| n <= 1);
                out.push(ServerCvar {
                    cvar: s.cvar.clone(),
                    label: s.label.clone(),
                    kind: s.kind.clone(),
                    value: if single {
                        s.default.clone()
                    } else {
                        before.clone().unwrap_or_else(|| s.default.clone())
                    },
                    before,
                    page: 1,
                    field: None,
                });
            }
        }
        if let Some(rounds) = self.game.rounds_cvar() {
            let before = get(rounds);
            out.push(ServerCvar {
                cvar: rounds.into(),
                label: "Rounds (off: deathmatch; Lucker Party)".into(),
                kind: ServerSettingKind::Bool,
                value: before.clone().unwrap_or_else(|| "0".into()),
                before,
                page: 1,
                field: None,
            });
        }
        for (field, cvar, token) in self.game.bots().map(|b| b.cvars).into_iter().flatten() {
            let before = get(cvar);
            out.push(ServerCvar {
                cvar: cvar.into(),
                label: self.text(token, cvar),
                kind: if cvar == "bot_prefix" {
                    ServerSettingKind::Text
                } else {
                    ServerSettingKind::Bool
                },
                value: before.clone().unwrap_or_default(),
                before,
                page: 2,
                field: Some(field),
            });
        }
        out
    }

    /// Create Server's pages in this game (the Bot page with bots).
    pub fn create_tabs(&self) -> &'static [(&'static str, &'static str)] {
        if self.game.bots().is_some() { &CREATE_TABS } else { &CREATE_TABS[..2] }
    }

    /// Show a page of Create Server.
    pub fn set_create_tab(&mut self, tab: usize) {
        if self.page != Page::NewGame {
            return;
        }
        self.create_tab = tab.min(self.create_tabs().len() - 1);
        self.combo = None;
        self.scroll = 0;
        self.focus = self.first_focusable_from(0, 1);
        self.focus_changed();
    }

    /// Start the new game set up in Create Server: from a game, close (the
    /// loading dialog takes over once the load starts: `begin_loading`);
    /// from the main menu, show it loading.
    pub(super) fn start(&mut self, out: &mut Outcome) {
        let Some((lines, after)) = self.start_lines() else {
            return;
        };
        out.lines = lines;
        out.after_load = after;
        if self.in_game {
            self.close(out);
        } else {
            self.loading = self.chosen_map().and_then(|m| self.maps.get(m)).cloned();
            self.page = Page::Main;
        }
    }

    /// A quick start (`chimera_quickstart`): rounds on the game's quick
    /// map with its normal bots, started at once as Create Server's Start
    /// would (single player).
    pub fn quick_start(&mut self, out: &mut Outcome) {
        self.quick_setup();
        self.start(out);
    }

    /// The quick start's settings in Create Server.
    pub(super) fn quick_setup(&mut self) {
        let Some(quick) = self.game.quick_start() else { return };
        let map = self.maps.iter().position(|m| m == quick.map).unwrap_or(0);
        self.new_game.map = map;
        self.new_game.random = false;
        self.new_game.bots_on = true;
        self.new_game.bot_count = quick.bots.to_string();
        self.new_game.bot_team = 0;
        self.new_game.difficulty = NORMAL;
        self.new_game.host = false;
        // Rounds; the rest as they are.
        let rounds = self.game.rounds_cvar();
        for c in &mut self.new_game.cvars {
            c.value = if rounds == Some(c.cvar.as_str()) {
                "1".into()
            } else {
                c.before.clone().unwrap_or_default()
            };
        }
    }

    /// The value of a cvar Create Server sets, as shown.
    pub(super) fn server_cvar(&self, cvar: &str) -> Option<&ServerCvar> {
        self.new_game.cvars.iter().find(|c| c.cvar == cvar)
    }

    /// The map a new game plays: the one picked, or with `< Random Map >`
    /// one of them by `random_seed`.
    pub fn chosen_map(&self) -> Option<usize> {
        let n = self.maps.len();
        if n == 0 {
            return None;
        }
        Some(if self.new_game.random {
            (self.random_seed % n as u64) as usize
        } else {
            self.new_game.map.min(n - 1)
        })
    }

    /// A new game's console lines: settings and the map now, the bots once
    /// the map is in (they spawn at its spawn points).
    pub fn start_lines(&self) -> Option<(Vec<String>, Vec<String>)> {
        let ng = &self.new_game;
        let map = self.maps.get(self.chosen_map()?)?;
        let bots = self.game.bots();
        let rounds_cvar = self.game.rounds_cvar();
        let mut lines = Vec::new();
        // Server Type first (the real start sequence's order).
        if self.game.server_types() {
            lines.extend(self.visibility_lines());
        }
        if bots.is_some() {
            lines.push("bot_kick".to_string());
        }
        if let Some(cvar) = rounds_cvar {
            let rounds = self.server_cvar(cvar).is_some_and(|c| c.value.trim().parse::<f32>().is_ok_and(|v| v != 0.0));
            lines.push(format!("{cvar} {}", rounds as u8));
        }
        // The Game and Bot pages' cvars that changed.
        for c in &ng.cvars {
            if rounds_cvar != Some(c.cvar.as_str())
                && let Some(b) = c.before.as_ref()
            {
                let (now, was) = (scr_line(c, &c.value), scr_line(c, b));
                if now != was {
                    lines.push(now);
                }
            }
        }
        // Hosting needs `maxplayers` above 1 (else the map loads single
        // player and nobody can join): the Game page's value, else 32.
        if ng.host {
            let max = self
                .server_cvar("maxplayers")
                .and_then(|c| c.value.trim().parse::<u32>().ok())
                .filter(|n| *n > 1)
                .unwrap_or(32);
            lines.retain(|l| !l.starts_with("maxplayers "));
            lines.push(format!("maxplayers {max}"));
        }
        if let Some(b) = bots {
            let (_, reaction, aim, turn) = b.difficulties[ng.difficulty];
            lines.extend([
                format!("bot_reaction {reaction}"),
                format!("bot_aim_error {aim}"),
                format!("bot_turn_rate {turn}"),
            ]);
        }
        lines.push(format!("map {}", crate::console::quote(map)));
        let (t, ct) = ng.split();
        let after = std::iter::repeat_n("bot_add 1".to_string(), t as usize)
            .chain(std::iter::repeat_n("bot_add 2".to_string(), ct as usize))
            .collect();
        Some((lines, after))
    }
}

impl GameMenu {
    /// Create Server's rows: the open page's controls, Start, Cancel.
    pub(super) fn create_server_rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = match self.create_tab {
            0 => {
                let mut r = vec![self.control_row(Field::Map)];
                if self.game.server_types() && self.layout().is_none_or(|l| l.get("VisibilityType").is_some()) {
                    r.push(self.visibility_row());
                }
                if let Some(b) = self.game.bots() {
                    r.push(self.control_row(Field::BotsOn));
                    if self.new_game.bots_on {
                        r.push(self.control_row(Field::BotCount));
                        r.extend((0..b.difficulties.len()).map(|k| self.control_row(Field::Difficulty(k))));
                    }
                }
                r
            }
            page => {
                let mut r = Vec::new();
                if page == 2 {
                    r.push(self.control_row(Field::BotTeam));
                }
                for (i, c) in self.new_game.cvars.iter().enumerate() {
                    if c.page == page && c.before.is_some() {
                        r.push(self.control_row(Field::ServerCvar(i)));
                    }
                }
                r
            }
        };
        rows.push(Row::Button {
            label: self.text("#GameUI_Start", "Start"),
            action: Action::Start,
            enabled: !self.maps.is_empty(),
        });
        rows.push(self.cancel_row());
        rows
    }
}
