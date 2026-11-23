//! Each game's own keys and options for the Options screen's Games tab
//! (docs/plans/active/settings-games.md, step 3): read from the data each
//! game crate ships (`GameBundle::player_options`, its
//! `data/player_options.toml`), so a game adds a key or an option without
//! touching the page's code.
//!
//! - **Actions** (`GameAction`): a key row on the game's Keys page, in
//!   its `group`; its keys are the game's own layer's (`Console::layers`,
//!   `bindgame`) or else the shared binds' (`Console::effective_binds`:
//!   until changed it follows the shared key). Changed: its keys in the
//!   layer differ from the game's shipped ones (`Console::layer_defaults`);
//!   reset puts those back (`reset_bind_line`). `planned`: bindable, not
//!   in the game yet (said in the row and the help).
//! - **Options** (`GameOption`) on the Gameplay or the Look and sound
//!   page: a cvar of the game's own (a `host::Setting`; changed when it
//!   differs from its default, reset sets the default), or an override of
//!   a shared setting (`overrides`: the console's game cvar layer,
//!   `setgame`; changed while the game has its own value, reset is
//!   `resetgame`). `origin = "chimera"`: ours, not the original game's
//!   (the Chimera mark, the help says so).
//!
//! The rail lists the party's games in `BUNDLES` order.
//!
//! Known shortcuts: TF2's alt-fire actions (zoom, spin up, detonate,
//! airblast) are one command, `+attack2`, so their keys change together
//! (per-gun keys would need the input to know the gun's game action).

use std::sync::OnceLock;

use crate::host::{Setting, setting_of};

/// A game's page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Page {
    Keys,
    Gameplay,
    Look,
}

impl Page {
    pub const ALL: [Page; 3] = [Page::Keys, Page::Gameplay, Page::Look];

    pub fn label(self) -> &'static str {
        match self {
            Page::Keys => "Keys",
            Page::Gameplay => "Gameplay",
            Page::Look => "Look and sound",
        }
    }

    fn parse(s: &str) -> Option<Page> {
        match s {
            "keys" => Some(Page::Keys),
            "gameplay" => Some(Page::Gameplay),
            "look" => Some(Page::Look),
            _ => None,
        }
    }
}

/// A key row (module doc).
#[derive(Clone, Debug, PartialEq)]
pub struct GameAction {
    pub group: String,
    pub label: String,
    pub note: Option<String>,
    pub command: String,
    pub help: String,
    pub planned: bool,
}

/// An option row (module doc).
#[derive(Clone, Debug, PartialEq)]
pub struct GameOption {
    pub page: Page,
    /// Its cvar (`setting.cvar`): the game's own, or the shared one it
    /// overrides.
    pub setting: Setting,
    /// An override of a shared setting (`setgame`).
    pub overrides: bool,
    /// Ours, not the original game's.
    pub ours: bool,
}

/// One game's page data.
#[derive(Clone, Debug, PartialEq)]
pub struct GameOptions {
    /// The game's id (its console namespace, `bindgame`'s and `setgame`'s).
    pub id: &'static str,
    pub name: String,
    /// The short name ("TF2 default").
    pub short: String,
    pub actions: Vec<GameAction>,
    pub options: Vec<GameOption>,
}

impl GameOptions {
    pub fn options_on(&self, page: Page) -> impl Iterator<Item = &GameOption> {
        self.options.iter().filter(move |o| o.page == page)
    }
}

/// `game`'s data from its `player_options.toml`.
pub fn parse(id: &'static str, text: &str) -> Result<GameOptions, String> {
    let doc: toml::Table = toml::from_str(text).map_err(|e| format!("{id}: {e}"))?;
    let game = doc.get("game").and_then(|g| g.as_table());
    let gs = |k: &str| game.and_then(|g| g.get(k)).and_then(|v| v.as_str()).map(str::to_string);
    let name = gs("name").unwrap_or_else(|| id.to_string());
    let short = gs("short").unwrap_or_else(|| name.clone());
    let mut actions = Vec::new();
    for a in doc.get("action").and_then(|a| a.as_array()).into_iter().flatten() {
        let t = a.as_table().ok_or_else(|| format!("{id}: an action is a table"))?;
        let s = |k: &str| t.get(k).and_then(|v| v.as_str()).map(str::to_string);
        let label = s("label").ok_or_else(|| format!("{id}: an action without a label"))?;
        actions.push(GameAction {
            group: s("group").unwrap_or_else(|| "Keys".into()),
            command: s("command").ok_or_else(|| format!("{id}: {label}: no command"))?,
            note: s("note"),
            help: s("help").unwrap_or_default(),
            planned: t.get("planned").and_then(|v| v.as_bool()).unwrap_or(false),
            label,
        });
    }
    let mut options = Vec::new();
    for o in doc.get("option").and_then(|o| o.as_array()).into_iter().flatten() {
        let t = o.as_table().ok_or_else(|| format!("{id}: an option is a table"))?;
        let over = t.get("overrides").and_then(|v| v.as_str());
        let mut v = o.clone();
        if let (Some(over), Some(t)) = (over, v.as_table_mut()) {
            t.insert("cvar".into(), toml::Value::String(over.to_string()));
        }
        let setting = setting_of(&v).map_err(|e| format!("{id}: {e}"))?;
        let page = t.get("page").and_then(|v| v.as_str()).and_then(Page::parse).filter(|p| *p != Page::Keys);
        let page = page.ok_or_else(|| format!("{id}: {}: page is gameplay or look", setting.label))?;
        let ours = match t.get("origin").and_then(|v| v.as_str()).unwrap_or("game") {
            "game" => false,
            "chimera" => true,
            other => return Err(format!("{id}: {}: origin {other:?} (game or chimera)", setting.label)),
        };
        options.push(GameOption { page, setting, overrides: over.is_some(), ours });
    }
    Ok(GameOptions { id, name, short, actions, options })
}

/// The pages, read once from the first party installed (a process runs
/// one party: its roster's), and the Controls page's tabs made of them
/// (`controls`).
static GAMES: OnceLock<Installed> = OnceLock::new();

struct Installed {
    games: Vec<GameOptions>,
    #[cfg(feature = "present")]
    controls: Vec<crate::controls::GameRows>,
}

/// Read `party`'s games' pages (`PartyPartsPlugin` does, at build): the
/// first party's stay for the process.
pub fn install(party: &crate::Party) {
    GAMES.get_or_init(|| {
        let games: Vec<GameOptions> = party
            .bundles
            .iter()
            .filter_map(|b| b.player_options.map(|t| parse(b.info.id, t.text()).unwrap_or_else(|e| panic!("data/player_options.toml: {e}"))))
            .collect();
        #[cfg(feature = "present")]
        let controls = std::iter::once(party.base)
            .chain(party.guests.iter().copied())
            .filter_map(|b| games.iter().find(|g| g.id == b.info.id))
            .filter_map(controls_of)
            .collect();
        Installed {
            games,
            #[cfg(feature = "present")]
            controls,
        }
    });
}

/// A game's tab on the Controls page (`controls::GameRows`) from its page
/// data: its name, its actions that are in the game (`planned` ones left
/// out; one row per command, the first action's label), its own cvars
/// with a range (the gun view FOV: not the shared settings it
/// overrides). None: it has no such row. Read once per process (the
/// strings live for it).
#[cfg(feature = "present")]
fn controls_of(g: &GameOptions) -> Option<crate::controls::GameRows> {
    use crate::controls::{GameRows, Row, Setting};
    let keep = |s: &str| -> &'static str { Box::leak(s.to_string().into_boxed_str()) };
    let mut rows: Vec<Row> = Vec::new();
    for a in g.actions.iter().filter(|a| !a.planned) {
        if !rows.iter().any(|r| r.command == a.command) {
            rows.push(Row { label: keep(&a.label), command: keep(&a.command) });
        }
    }
    let settings: Vec<Setting> = g
        .options
        .iter()
        .filter(|o| !o.overrides)
        .filter_map(|o| match o.setting.pick {
            crate::host::Pick::Range { min, max, step } => {
                Some(Setting { label: keep(&o.setting.label), cvar: keep(&o.setting.cvar), min, max, step })
            }
            crate::host::Pick::Choices(_) => None,
        })
        .collect();
    (!rows.is_empty() || !settings.is_empty()).then(|| GameRows {
        game: g.id,
        label: keep(&g.name),
        rows: Box::leak(rows.into_boxed_slice()),
        settings: Box::leak(settings.into_boxed_slice()),
    })
}

/// The Controls page's game tabs (`controls::GameRows`), the base game's
/// first, then the guests' (those with rows); none before a party is
/// installed (`install`).
#[cfg(feature = "present")]
pub fn controls() -> &'static [crate::controls::GameRows] {
    GAMES.get().map_or(&[], |i| i.controls.as_slice())
}

/// The party's games' pages, in its `bundles` order (those that ship
/// data); none before a party is installed (`install`).
pub fn games() -> &'static [GameOptions] {
    GAMES.get().map_or(&[], |i| i.games.as_slice())
}

/// The game `id`'s page.
pub fn game(id: &str) -> Option<&'static GameOptions> {
    games().iter().find(|g| g.id == id)
}
