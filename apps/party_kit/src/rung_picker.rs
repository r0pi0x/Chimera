//! The sandbox's rung picker (feature `present`; Ultimate Gun Game's
//! `sandbox`, `chimera_sim::minigame::rung_command`): a dialog over the
//! match, built as the Host page's Maps picker is (`maps_picker`: the
//! scrim, the plate between the shell's window lines, the white top bar, a
//! window of tree rows around the cursor, folding chevrons, the keys and
//! the actions with the hero last), that puts the player on the rung they
//! choose, so nobody types `rung`.
//!
//! - **The tree**: every game of the ladder (in the ladder's order, its
//!   rung count on the right), under it the gun classes its parts give
//!   (`auto::report`, else the weapon data's `class`: pistols, SMGs, ...,
//!   launchers, grenades; the melee band last), under them the rungs: the
//!   gun's icon (its game's own while the theme prefers them, else the
//!   kit's by class: `signal::gun_icon`, as the kill feed), its name as
//!   the rank-up toast and the kill feed name it (`gun_name`), its id, a
//!   tag for a special one (Grenade; a placed device by its data's
//!   `*:device` hook, Combat Arms' Turret), Current on the player's rung,
//!   the rung's number. At opening every game but the current rung's is
//!   folded and the cursor is on the current rung. The sandbox's sentinel
//!   rung is never listed (`rung_command::usable`). Only the guns a drawn
//!   ladder may deal are listed (`auto::drawable`), one row per gun of a
//!   game by its model (`shown_rungs`: an install's skins and levels
//!   of one model are one row), so the header counts guns ("N guns ·
//!   5 games"); `rung <id>` still reaches every rung of the ladder.
//! - **The filter** (the kit's text field, which always has the keys):
//!   typing narrows the rows to the rungs whose name, id, class, game or
//!   tag hold every word typed, across all games, with their headers open;
//!   the cursor goes to the first match, so Enter picks it.
//! - **Keys**: ↑↓ (PgUp/PgDn by a window) the row, ←→ fold or open a
//!   header, Enter picks the row (on a header: folds or opens it; while
//!   filtering, picks the first rung under it), Esc or a right click
//!   Cancel (Back: closed, nothing changed); the wheel moves the cursor;
//!   the pointer: a rung row picks, a header row or its chevron folds.
//!   The keys are the picker's only while it is open in the sandbox and
//!   neither the console nor chat has them (Enter is then theirs); a
//!   pre-match lobby coming up closes it.
//! - **Picking** goes the `rung` command's way: `rung_command::move_to` on
//!   the local player, on the server (a listen host's or a local match's
//!   world; a network client sends `rung <n>`, which its server checks),
//!   then the dialog closes.
//! - **Opening**: `rung_picker` (console; L in Ultimate Gun Game,
//!   `APP_BINDS`; a row of the Controls page) and the Escape menu's
//!   Change gun, only while a sandbox plays (`rung_command::holds`):
//!   elsewhere the command says why, the entry isn't shown and an open
//!   dialog closes. While open, the mouse is free and the game gets no
//!   input (`esc_menu::sync` counts it in `AppMenu`), and the Escape menu
//!   leaves Esc to it (`took_keys`).
//! - `ui_scene rung_picker [filter | off]` stages it for screenshots,
//!   without a match (made-up rungs of every game) or over one.
//! - Look-book names (`UiParityId`): `rungs.scrim`, `rungs.plate`,
//!   `rungs.title`, `rungs.filter`, `rungs.tree`, `rungs.row.<n>` (the
//!   window's rows from 1) and `.icon`, `.name`, `.tag`, `.current` under it,
//!   `rungs.keys`, `rungs.cancel`, `rungs.pick`.
//!
//! Known shortcuts: over play it has no shell bar or breadcrumb (the
//! Escape menu over play has none yet either: ugg-ui.md, "Left after
//! revision 8"); its plate is the Maps picker's dialog; no entry or exit motion (the Maps picker has none
//! either); the filter's caret stays at its end (←→ fold, as the Maps
//! picker's keys do); no persistent on-screen hint of the key (the party's
//! overlay has no place for one); the listing is made at opening.

use std::collections::{BTreeMap, BTreeSet};

use bevy::{
    ecs::system::SystemParam,
    input::{
        ButtonInput, ButtonState,
        keyboard::KeyboardInput,
        mouse::{MouseScrollUnit, MouseWheel},
    },
    prelude::*,
};
use chimera_client::client::{
    minigame_scoreboard::{GunNames, gun_name},
    signal::{self, Look},
    ui_sound::UiCue,
    widgets::{Caret, Edit, TextKey, text_key},
};
use chimera_core::{
    console::{Console, ConsoleAppExt, Level, command_player},
    game_parts::GunClass,
};
use chimera_sim::{
    minigame::{
        ActiveMinigame,
        ladder::{Ladder, Rung},
        rung_command::{self, HoldRungs},
    },
    weapon::data::{WeaponCatalog, WeaponData},
};
use chimera_ui::{
    Role, Tone, UiParityId, UiStyle,
    controls::{HERO_H, Kind, State, Ui},
    widgets::{self, TagKind},
};

use crate::maps_picker::game_name;

/// Whether a sandbox plays (`rung_command::holds`), for systems.
#[derive(SystemParam)]
pub struct SandboxPlays<'w> {
    hold: Option<Res<'w, HoldRungs>>,
    active: Option<Res<'w, ActiveMinigame>>,
}

impl SandboxPlays<'_> {
    pub fn on(&self) -> bool {
        rung_command::holds(self.hold.as_deref(), self.active.as_deref())
    }
}

/// A rung as the dialog lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerRung {
    /// Its place in the ladder (0-based; the HUD's number less one).
    pub index: usize,
    pub id: String,
    pub name: String,
    /// The kit's icon by class (`signal::icon_class`), when the game has
    /// no icon of its own.
    pub icon_class: Option<&'static str>,
    /// Grenade, Turret ...
    pub tag: Option<String>,
}

/// A class of a game's rungs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerClass {
    pub name: String,
    pub rungs: Vec<PickerRung>,
}

/// A game's rungs by class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerGame {
    pub id: String,
    pub name: String,
    pub classes: Vec<PickerClass>,
}

impl PickerGame {
    pub fn count(&self) -> usize {
        self.classes.iter().map(|k| k.rungs.len()).sum()
    }
}

/// What the listing knows of a rung's gun.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GunInfo {
    pub class: Option<GunClass>,
    pub name: String,
    pub icon_class: Option<&'static str>,
    pub tag: Option<String>,
}

/// A class's heading (`melee`: the ladder's melee band).
pub fn class_name(class: Option<GunClass>, melee: bool) -> &'static str {
    if melee {
        return "Melee";
    }
    match class {
        Some(GunClass::Pistol) => "Pistols",
        Some(GunClass::Smg) => "SMGs",
        Some(GunClass::Shotgun) => "Shotguns",
        Some(GunClass::Carbine) => "Carbines",
        Some(GunClass::Rifle) => "Rifles",
        Some(GunClass::Heavy) => "Machine guns",
        Some(GunClass::Marksman) => "Marksman rifles",
        Some(GunClass::Sniper) => "Snipers",
        Some(GunClass::HeavyPistol) => "Heavy pistols",
        Some(GunClass::Launcher) => "Launchers",
        Some(GunClass::Flamethrower) => "Flamethrowers",
        Some(GunClass::Grenade) => "Grenades",
        None => "Other",
    }
}

/// The classes' order: the gun classes weakest first, grenades, melee,
/// then the rest.
fn class_rank(class: Option<GunClass>, melee: bool) -> u16 {
    match (melee, class) {
        (true, _) => 200,
        (false, Some(c)) => c as u16,
        (false, None) => 300,
    }
}

/// The tree of `rungs` (each its index in a ladder and its gun, in order;
/// from `melee_from` on the melee band), each gun as `info` says; a gun
/// twice is listed once (its first rung).
pub fn listing(rungs: &[(usize, &str)], melee_from: usize, info: impl Fn(&str) -> GunInfo) -> Vec<PickerGame> {
    let mut games: Vec<(PickerGame, Vec<u16>)> = Vec::new();
    let mut seen = BTreeSet::new();
    for &(i, id) in rungs {
        let id = &id;
        if !seen.insert(*id) {
            continue;
        }
        let melee = i >= melee_from;
        let gun = info(id);
        let game = id.split_once(':').map_or("", |(g, _)| g);
        let g = match games.iter().position(|(p, _)| p.id == game) {
            Some(g) => g,
            None => {
                games.push((PickerGame { id: game.into(), name: game_name(game), classes: Vec::new() }, Vec::new()));
                games.len() - 1
            }
        };
        let (game, ranks) = &mut games[g];
        let name = class_name(gun.class, melee);
        let k = match game.classes.iter().position(|k| k.name == name) {
            Some(k) => k,
            None => {
                game.classes.push(PickerClass { name: name.into(), rungs: Vec::new() });
                ranks.push(class_rank(gun.class, melee));
                game.classes.len() - 1
            }
        };
        game.classes[k].rungs.push(PickerRung { index: i, id: id.to_string(), name: gun.name, icon_class: gun.icon_class, tag: gun.tag });
    }
    games
        .into_iter()
        .map(|(mut game, ranks)| {
            let mut order: Vec<(u16, PickerClass)> = ranks.into_iter().zip(game.classes).collect();
            order.sort_by_key(|(r, _)| *r);
            game.classes = order.into_iter().map(|(_, k)| k).collect();
            game
        })
        .collect()
}

/// A placed device's tag (its data's `*:device` hook, by its `kind`:
/// Combat Arms' turret).
fn device_tag(d: &WeaponData) -> Option<String> {
    let (_, hook) = d.hooks.iter().find(|(k, _)| k.ends_with(":device"))?;
    let kind = hook.get("kind").and_then(|v| v.as_str()).unwrap_or("device");
    let mut c = kind.chars();
    Some(c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default())
}

/// The rungs of `rungs` the tree lists (module doc, "The tree"): a gun
/// the games lend that a drawn ladder may not deal (`auto::drawable`:
/// harmless, or a game's `DrawExclusions`) left out; one row per gun, a
/// gun being its game's and its model's (`weapon::names::split`: the skin
/// words and the variant tail, levels and editions, left out; without a
/// model, its short name, `GunNameRules::short`), shown
/// by its rung with the shortest full name (the plain one, before the
/// first). A rung no game lends (a ladder's melee) stays.
pub fn shown_rungs<'a>(world: &World, rungs: &[&'a str]) -> Vec<(usize, &'a str)> {
    use chimera_sim::minigame::auto;
    let mut caps = auto::report(world);
    let ids = |caps: &[auto::Capabilities]| -> BTreeSet<&'static str> {
        caps.iter().flat_map(|c| c.guns.iter().map(|g| g.id).chain(c.grenades.iter().copied())).collect()
    };
    let offered = ids(&caps);
    auto::drawable(world, &mut caps);
    let dealt = ids(&caps);
    let catalog = world.get_resource::<WeaponCatalog>();
    let names = world.get_resource::<GunNames>();
    let rules = world.get_resource::<chimera_sim::weapon::names::GunNameRules>();
    let mut kept: Vec<(usize, &'a str, usize)> = Vec::new();
    let mut by_gun: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (i, id) in rungs.iter().enumerate() {
        if offered.contains(id) && !dealt.contains(id) {
            continue;
        }
        let full = gun_name(id, names, catalog);
        // The gun: its model by the names' split (`names::split`: skin words
        // and the variant tail off), else its short name.
        let short = rules.map_or_else(
            || full.clone(),
            |r| {
                let full = r.full(id, &full);
                let s = chimera_sim::weapon::names::split(&full, r.words());
                if s.model_words > 0 { s.base[..s.model_words.min(s.base.len())].join(" ") } else { r.short(id, &full).0 }
            },
        );
        let game = id.split_once(':').map_or("", |(g, _)| g).to_string();
        let len = full.chars().count();
        match by_gun.get(&(game.clone(), short.to_lowercase())) {
            Some(&k) if len < kept[k].2 => kept[k] = (i, id, len),
            Some(_) => {}
            None => {
                by_gun.insert((game, short.to_lowercase()), kept.len());
                kept.push((i, id, len));
            }
        }
    }
    kept.sort_by_key(|k| k.0);
    kept.into_iter().map(|(i, id, _)| (i, id)).collect()
}

/// The tree of `rungs` with what `world` knows of each gun: its class by
/// its game's parts (else its data), its name, icon class and tag; only
/// the rungs `shown_rungs` keeps.
pub fn listing_in(world: &World, rungs: &[&str], melee_from: usize) -> Vec<PickerGame> {
    let mut classes: BTreeMap<&str, GunClass> = BTreeMap::new();
    for c in chimera_sim::minigame::auto::report(world) {
        for g in &c.guns {
            classes.insert(g.id, g.class);
        }
        for g in &c.grenades {
            classes.insert(g, GunClass::Grenade);
        }
    }
    let catalog = world.get_resource::<WeaponCatalog>();
    let names = world.get_resource::<GunNames>();
    listing(&shown_rungs(world, rungs), melee_from, |id| {
        let data = catalog.and_then(|c| c.get(id));
        let class = classes.get(id).copied().or_else(|| data.and_then(|d| d.class));
        let tag = data.and_then(device_tag).or_else(|| (class == Some(GunClass::Grenade)).then(|| "Grenade".to_string()));
        GunInfo {
            class,
            name: gun_name(id, names, catalog),
            icon_class: data.and_then(signal::icon_class),
            tag,
        }
    })
}

/// The playing ladder's tree (its sentinel left out), or None without one.
pub fn listing_of(world: &World) -> Option<Vec<PickerGame>> {
    let ladder = world.get_resource::<Ladder>()?;
    let usable = rung_command::usable(world, ladder);
    let melee_from = ladder.rungs.len().saturating_sub(ladder.melee_rungs);
    Some(listing_in(world, &ladder.rungs[..usable], melee_from))
}

/// A row of the tree as shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickRow {
    Game(usize),
    Class(usize, usize),
    Rung(usize, usize, usize),
}

/// The picker's state (module doc).
#[derive(Resource, Clone, Debug, Default)]
pub struct RungPicker {
    pub open: bool,
    pub games: Vec<PickerGame>,
    /// The player's rung at opening.
    pub current: Option<usize>,
    pub filter: String,
    /// The cursor's row among the shown ones.
    pub cursor: usize,
    /// Folded games (`game`) and classes (`game/class`).
    pub folded: BTreeSet<String>,
    /// The picker had the keys this frame (the Escape menu leaves Esc).
    pub took_keys: bool,
    /// Staged by `ui_scene` outside a sandbox: a pick changes nothing.
    pub staged: bool,
    /// Opened this frame: the key that opened it isn't typed.
    fresh: bool,
}

/// The filter's longest text, bytes.
pub const FILTER_MAX: usize = 48;

/// The rows the dialog shows at once (a window the cursor keeps in view).
pub const WINDOW: usize = 15;

impl RungPicker {
    fn class_key(&self, g: usize, k: usize) -> String {
        format!("{}/{}", self.games[g].id, self.games[g].classes[k].name)
    }

    /// The filter's words, lower case.
    fn words(&self) -> Vec<String> {
        self.filter.to_lowercase().split_whitespace().map(str::to_string).collect()
    }

    /// Whether rung `r` of class `k` of game `g` holds every word.
    fn fits(&self, words: &[String], g: usize, k: usize, r: usize) -> bool {
        let game = &self.games[g];
        let class = &game.classes[k];
        let rung = &class.rungs[r];
        let hay = format!("{} {} {} {} {} {}", rung.name, rung.id, class.name, game.name, game.id, rung.tag.as_deref().unwrap_or("")).to_lowercase();
        words.iter().all(|w| hay.contains(w.as_str()))
    }

    /// Filtering (the filter holds a word).
    pub fn filtering(&self) -> bool {
        !self.filter.trim().is_empty()
    }

    /// The rows shown: folded games and classes hide what is under them;
    /// while filtering, the matching rungs under open headers.
    pub fn rows(&self) -> Vec<PickRow> {
        let words = self.words();
        let filtering = !words.is_empty();
        let mut out = Vec::new();
        for (g, game) in self.games.iter().enumerate() {
            let mut under = Vec::new();
            for (k, class) in game.classes.iter().enumerate() {
                let rungs: Vec<usize> = (0..class.rungs.len()).filter(|r| !filtering || self.fits(&words, g, k, *r)).collect();
                if rungs.is_empty() {
                    continue;
                }
                under.push(PickRow::Class(g, k));
                if filtering || !self.folded.contains(&self.class_key(g, k)) {
                    under.extend(rungs.into_iter().map(|r| PickRow::Rung(g, k, r)));
                }
            }
            if filtering && under.is_empty() {
                continue;
            }
            out.push(PickRow::Game(g));
            if filtering || !self.folded.contains(&game.id) {
                out.extend(under);
            }
        }
        out
    }

    /// The rung of a row.
    pub fn rung(&self, row: PickRow) -> Option<&PickerRung> {
        match row {
            PickRow::Rung(g, k, r) => self.games.get(g)?.classes.get(k)?.rungs.get(r),
            _ => None,
        }
    }

    /// How many rungs are listed, and how many match the filter.
    pub fn counts(&self) -> (usize, usize) {
        let all = self.games.iter().map(PickerGame::count).sum();
        let shown = self.rows().iter().filter(|r| matches!(r, PickRow::Rung(..))).count();
        (all, shown)
    }

    pub fn is_folded(&self, row: PickRow) -> bool {
        match row {
            PickRow::Game(g) => self.folded.contains(&self.games[g].id),
            PickRow::Class(g, k) => self.folded.contains(&self.class_key(g, k)),
            PickRow::Rung(..) => false,
        }
    }

    /// Folds or opens a header (`open`: show what is under it).
    pub fn fold(&mut self, row: PickRow, open: bool) {
        let key = match row {
            PickRow::Game(g) => self.games[g].id.clone(),
            PickRow::Class(g, k) => self.class_key(g, k),
            PickRow::Rung(..) => return,
        };
        if open {
            self.folded.remove(&key);
        } else {
            self.folded.insert(key);
        }
    }

    /// The filter set; the cursor on the first match (the top without one).
    pub fn set_filter(&mut self, text: &str) {
        self.filter = text.to_string();
        let rows = self.rows();
        self.cursor = rows.iter().position(|r| matches!(r, PickRow::Rung(..))).unwrap_or(0);
    }

    /// Open on `games`: every game folded but the current rung's, the
    /// cursor on the current rung.
    pub fn open_with(&mut self, games: Vec<PickerGame>, current: Option<usize>) {
        let here = games.iter().position(|g| g.classes.iter().any(|k| k.rungs.iter().any(|r| Some(r.index) == current)));
        *self = RungPicker {
            open: true,
            folded: games.iter().enumerate().filter(|(g, _)| Some(*g) != here.or(Some(0))).map(|(_, g)| g.id.clone()).collect(),
            games,
            current,
            fresh: true,
            ..default()
        };
        let rows = self.rows();
        self.cursor = rows.iter().position(|r| self.rung(*r).is_some_and(|r| Some(r.index) == current)).unwrap_or(0);
    }

    pub fn close(&mut self) {
        self.open = false;
        self.staged = false;
        self.filter.clear();
    }

    /// What Enter (or a click) on row `i` does: the rung to pick, else
    /// None (a header: fold it, or while filtering its first rung).
    pub fn choice(&self, i: usize) -> Option<usize> {
        let rows = self.rows();
        let row = *rows.get(i)?;
        if let Some(r) = self.rung(row) {
            return Some(r.index);
        }
        if self.filtering() {
            return rows[i..].iter().find_map(|r| self.rung(*r)).map(|r| r.index);
        }
        None
    }
}

/// What a click (or a key) in the dialog does.
#[derive(Component, Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RungPickerAction {
    /// A row: a rung picks it, a header folds or opens.
    Row(usize),
    /// A header's chevron.
    Fold(usize),
    /// The cursor's row (Enter, the Pick button).
    Pick,
    Cancel,
}

/// The dialog's root (tests find it).
#[derive(Component, Clone, Copy, Debug)]
pub struct RungPickerDialog;

/// Over the HUDs, the match-end screen and the scoreboard, as the Escape
/// menu; under the console.
pub const PICKER_Z: i32 = crate::esc_menu::MENU_Z;

/// Puts the local player on rung `index` (0-based) as `rung` does
/// (module doc); only while a sandbox plays.
pub fn pick(world: &mut World, index: usize) -> Result<(), String> {
    if !rung_command::holding(world) {
        return Err("only in the sandbox (`sandbox`)".into());
    }
    if world.get_resource::<chimera_core::core::NetRole>() == Some(&chimera_core::core::NetRole::Client) {
        world.resource_mut::<Console>().submit(format!("rung {}", index + 1));
        return Ok(());
    }
    let ladder = world.get_resource::<Ladder>().cloned().ok_or("no ladder plays")?;
    if index >= rung_command::usable(world, &ladder) {
        return Err(format!("no rung {}", index + 1));
    }
    let e = command_player(world)?;
    rung_command::move_to(world, e, index, &ladder);
    Ok(())
}

/// Opens the dialog on the playing ladder (only while a sandbox plays).
pub fn open(world: &mut World) -> Result<(), String> {
    if !rung_command::holding(world) {
        return Err("only in the sandbox (`sandbox`)".into());
    }
    let games = listing_of(world).ok_or("no ladder plays")?;
    let current = command_player(world).ok().and_then(|e| world.get::<Rung>(e)).map(|r| r.0);
    world.get_resource_or_init::<RungPicker>().open_with(games, current);
    Ok(())
}

pub struct RungPickerPlugin;

impl Plugin for RungPickerPlugin {
    fn build(&self, app: &mut App) {
        crate::ui_cues::ensure_message(app);
        chimera_client::client::ui_scene::UiSceneHooks::add(app.world_mut(), "rung_picker", scene);
        app.init_resource::<RungPicker>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<RungPickerAction>()
            .add_message::<KeyboardInput>()
            .add_message::<MouseWheel>()
            .console_command(
                "chimera:rung_picker",
                "rung_picker: open or close the sandbox's rung picker (L; the Escape menu's Change gun): pick any rung of the \
                 ladder from a list (only in the sandbox).",
                |w, _| {
                    if w.get_resource::<RungPicker>().is_some_and(|p| p.open) {
                        w.resource_mut::<RungPicker>().close();
                        return Ok(None);
                    }
                    open(w).map(|_| None)
                },
            )
            .add_systems(
                Update,
                (keys, typing, act).chain().before(crate::esc_menu::EscMenuSystems).before(crate::main_menu::MainMenuSystems),
            )
            .add_systems(Update, crate::ui_cues::hover::<RungPickerAction>.in_set(crate::esc_menu::EscMenuSystems))
            .add_systems(
                Update,
                draw.after(crate::esc_menu::EscMenuSystems)
                    .run_if(resource_exists::<chimera_ui::Theme>)
                    .run_if(resource_exists::<chimera_ui::UiUnits>),
            );
    }
}

/// `ui_scene rung_picker [filter | off]`: the dialog over the match (its
/// ladder's rungs) or, without one, over made-up rungs of every game.
fn scene(w: &mut World, args: &[String]) -> Result<(), String> {
    if args.first().is_some_and(|a| a == "off") {
        w.resource_mut::<RungPicker>().close();
        return Ok(());
    }
    let sandbox = rung_command::holding(w);
    let games = listing_of(w).filter(|g| !g.is_empty()).unwrap_or_else(|| made_up(w));
    let current = command_player(w).ok().and_then(|e| w.get::<Rung>(e)).map(|r| r.0).or(Some(3));
    let mut p = w.resource_mut::<RungPicker>();
    p.open_with(games, current);
    p.staged = !sandbox;
    if !args.is_empty() {
        p.set_filter(&args.join(" "));
    }
    Ok(())
}

/// Made-up rungs for `ui_scene`: a few of each game's guns, Halo's all.
fn made_up(w: &World) -> Vec<PickerGame> {
    const IDS: &[&str] = &[
        "cs_source:weapon_glock",
        "cs_source:weapon_deagle",
        "cs_source:weapon_mp5navy",
        "cs_source:weapon_m3",
        "cs_source:weapon_ak47",
        "cs_source:weapon_awp",
        "cs_source:weapon_hegrenade",
        "combat_arms:m4a1",
        "combat_arms:auto-turret",
        "hl2dm:weapon_smg1",
        "hl2dm:weapon_crossbow",
        "halo_ce:plasma_pistol",
        "halo_ce:plasma_rifle",
        "halo_ce:needler",
        "halo_ce:shotgun",
        "halo_ce:assault_rifle",
        "halo_ce:sniper_rifle",
        "halo_ce:pistol",
        "halo_ce:rocket_launcher",
        "halo_ce:frag_grenade",
        "combat_arms:knife",
    ];
    listing_in(w, IDS, IDS.len() - 1)
}

/// The keys while it is open (module doc), before the Escape menu's.
#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut wheel: MessageReader<MouseWheel>,
    sandbox: SandboxPlays,
    (console, chat, lobby): (
        Option<Res<chimera_client::client::console::ConsoleUi>>,
        Option<Res<chimera_client::client::chat::ChatInput>>,
        Option<Res<chimera_client::client::prematch_lobby::LobbyView>>,
    ),
    mut picker: ResMut<RungPicker>,
    mut out: MessageWriter<RungPickerAction>,
    mut cues: MessageWriter<UiCue>,
) {
    let notches: f32 = wheel.read().map(|w| if w.unit == MouseScrollUnit::Line { w.y } else { w.y / 40.0 }).sum();
    picker.took_keys = picker.open;
    if !picker.open {
        return;
    }
    // Never outside a sandbox (a match change closes it), nor over a
    // pre-match lobby (whose Enter is chat's).
    if (!picker.staged && !sandbox.on()) || crate::main_menu::owned_by_lobby(lobby.as_deref()) {
        picker.close();
        return;
    }
    // The console or chat has the keys: Enter is theirs.
    if console.is_some_and(|c| c.open) || chat.is_some_and(|c| c.open.is_some()) {
        picker.took_keys = false;
        return;
    }
    let k = |c: KeyCode| keys.just_pressed(c);
    let rows = picker.rows();
    if k(KeyCode::Escape) || mouse.is_some_and(|m| m.just_pressed(MouseButton::Right)) {
        out.write(RungPickerAction::Cancel);
        return;
    }
    if k(KeyCode::Enter) || k(KeyCode::NumpadEnter) {
        out.write(RungPickerAction::Pick);
        return;
    }
    let step: i64 = if k(KeyCode::ArrowUp) {
        -1
    } else if k(KeyCode::ArrowDown) {
        1
    } else if k(KeyCode::PageUp) {
        -(WINDOW as i64 - 1)
    } else if k(KeyCode::PageDown) {
        WINDOW as i64 - 1
    } else {
        -(notches.round() as i64) * 3
    };
    if step != 0 && !rows.is_empty() {
        let next = (picker.cursor as i64 + step).clamp(0, rows.len() as i64 - 1) as usize;
        if next != picker.cursor {
            picker.cursor = next;
            cues.write(UiCue::new("ui_hover"));
        }
    } else if (k(KeyCode::ArrowLeft) || k(KeyCode::ArrowRight))
        && let Some(row) = rows.get(picker.cursor).copied()
        && !matches!(row, PickRow::Rung(..))
        && !picker.filtering()
    {
        picker.fold(row, k(KeyCode::ArrowRight));
        cues.write(UiCue::new("ui_select"));
    }
}

/// Typing into the filter (the field always has the keys): characters,
/// Backspace, Ctrl+A (clears), Ctrl+V (pastes).
fn typing(
    mut events: MessageReader<KeyboardInput>,
    held: Res<ButtonInput<KeyCode>>,
    (console, chat): (Option<Res<chimera_client::client::console::ConsoleUi>>, Option<Res<chimera_client::client::chat::ChatInput>>),
    mut picker: ResMut<RungPicker>,
    mut clipboard: Local<chimera_ui::clipboard::Clipboard>,
    shared_clipboard: Option<ResMut<chimera_ui::clipboard::Clipboard>>,
) {
    if !picker.open || console.is_some_and(|c| c.open) || chat.is_some_and(|c| c.open.is_some()) {
        events.clear();
        return;
    }
    if std::mem::take(&mut picker.fresh) {
        events.clear();
        return;
    }
    let clipboard = match shared_clipboard {
        Some(c) => c.into_inner(),
        None => &mut *clipboard,
    };
    let shift = held.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let ctrl = held.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let mut text = picker.filter.clone();
    for e in events.read() {
        if e.state != ButtonState::Pressed {
            continue;
        }
        let Some(key) = text_key(&e.logical_key, e.key_code, ctrl, shift) else { continue };
        if matches!(key, TextKey::Edit(Edit::Left(_) | Edit::Right(_) | Edit::Home(_) | Edit::End(_) | Edit::To(..))) {
            continue;
        }
        if key == TextKey::Edit(Edit::SelectAll) {
            // Ctrl+A clears it (the caret stays at the end: module doc).
            text.clear();
            continue;
        }
        Caret::end_of(&text).key(&mut text, key, clipboard, FILTER_MAX, false);
    }
    if text != picker.filter {
        picker.set_filter(&text);
    }
}

/// Clicks and what the actions do; a pick moves the player through the
/// world once (`pick`, queued).
fn act(
    mut actions: ResMut<Messages<RungPickerAction>>,
    pressed: Query<(&Interaction, &RungPickerAction), Changed<Interaction>>,
    mut picker: ResMut<RungPicker>,
    mut cues: MessageWriter<UiCue>,
    mut commands: Commands,
) {
    let mut todo: Vec<RungPickerAction> = actions.drain().collect();
    todo.extend(pressed.iter().filter(|(i, _)| **i == Interaction::Pressed).map(|(_, a)| *a));
    if todo.is_empty() || !picker.open {
        return;
    }
    for a in todo {
        let at = match a {
            RungPickerAction::Cancel => {
                picker.close();
                cues.write(UiCue::new("ui_back"));
                return;
            }
            RungPickerAction::Pick => picker.cursor,
            RungPickerAction::Row(i) | RungPickerAction::Fold(i) => i,
        };
        let rows = picker.rows();
        let Some(row) = rows.get(at).copied() else { continue };
        picker.cursor = at;
        let choice = if matches!(a, RungPickerAction::Fold(_)) { None } else { picker.choice(at) };
        match choice {
            Some(index) => {
                let staged = picker.staged;
                picker.close();
                cues.write(UiCue::new("ui_select"));
                if !staged {
                    commands.queue(move |w: &mut World| {
                        if let Err(e) = pick(w, index) {
                            w.resource_mut::<Console>().print(Level::Error, format!("rung_picker: {e}"));
                        }
                    });
                }
                return;
            }
            None if !matches!(row, PickRow::Rung(..)) && !picker.filtering() => {
                let open = picker.is_folded(row);
                picker.fold(row, open);
                cues.write(UiCue::new("ui_select"));
            }
            None => {}
        }
    }
}

/// What the dialog last showed: rebuilt when it differs.
#[derive(PartialEq)]
struct Shown {
    cursor: usize,
    filter: String,
    folded: BTreeSet<String>,
    current: Option<usize>,
    games: usize,
    units: (u32, u32),
    side: u32,
}

/// The plate's inset from each side of the 16:9 frame, cq: two thirds of
/// the frame wide at 16:9 (16.5 cq in), nearly all of it at 4:3 (the
/// shell's margin), in between by the window's aspect.
pub fn side_inset(aspect: f32, margin: f32) -> f32 {
    let t = ((aspect - 4.0 / 3.0) / (16.0 / 9.0 - 4.0 / 3.0)).clamp(0.0, 1.0);
    margin + t * (SIDE_16_9 - margin)
}

/// A rung row's fixed columns at the right, cq: the tag, Current.
pub const TAG_COL: f32 = 4.4;
pub const CURRENT_COL: f32 = 3.8;

/// `side_inset` at 16:9.
pub const SIDE_16_9: f32 = 16.5;

fn draw(
    picker: Res<RungPicker>,
    look: Look,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    roots: Query<Entity, With<RungPickerDialog>>,
    mut shown: Local<Option<Shown>>,
    mut commands: Commands,
) {
    if !picker.open {
        if shown.take().is_some() {
            for e in &roots {
                commands.entity(e).despawn();
            }
        }
        return;
    }
    let u = *look.units;
    let aspect = windows.iter().next().map_or(16.0 / 9.0, |w| w.width() / w.height().max(1.0));
    let side = side_inset(aspect, chimera_ui::shell::ShellLayout::of(&look.theme).margin);
    let now = Shown {
        side: (side * 100.0) as u32,
        cursor: picker.cursor,
        filter: picker.filter.clone(),
        folded: picker.folded.clone(),
        current: picker.current,
        games: picker.games.len(),
        units: ((u.height * 8.0) as u32, (u.scale_factor * 1000.0) as u32),
    };
    if shown.as_ref() == Some(&now) {
        return;
    }
    for e in &roots {
        commands.entity(e).despawn();
    }
    let ui = Ui::new(&look.theme, look.fonts.as_deref(), u);
    spawn(&mut commands, &ui, &look, &picker, side);
    *shown = Some(now);
}

/// Spawns the dialog: the scrim, the plate (the Maps picker's, `side` cq
/// in from each side: `side_inset`; the shell window's top and foot), the
/// title and counts, the filter, the tree, the keys and the actions with
/// Pick the hero.
pub fn spawn(c: &mut Commands, ui: &Ui, look: &Look, picker: &RungPicker, side: f32) -> Entity {
    let sh = chimera_ui::shell::ShellLayout::of(ui.theme);
    let scrim = c
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(ui.color(Tone::Ink, 0.72)),
            bevy::ui::FocusPolicy::Block,
            GlobalZIndex(PICKER_Z),
            RungPickerDialog,
            UiParityId::new("rungs.scrim"),
        ))
        .id();
    let frame = ui.boxed(c, scrim, UiStyle { w: Some(100.0), h: Some(56.25), shrink: Some(0.0), ..UiStyle::row() });
    let plate = c
        .spawn((
            UiStyle { gap: Some(0.8), pad: Some([1.6, 1.8, 1.4, 1.8].into()), ..UiStyle::column() }
                .abs([Some(side), Some(sh.win_top), Some(side), Some(sh.win_bottom)])
                .bg(Tone::Ink2)
                .outline(Tone::Fg4, 1.0),
            Node::default(),
            BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.6), Val::ZERO, ui.units.val(2.0), Val::ZERO, ui.units.val(4.0)),
            UiParityId::new("rungs.plate"),
            ChildOf(frame),
        ))
        .id();
    c.spawn((
        Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, height: ui.units.val(0.22), ..default() },
        BackgroundColor(ui.color(Tone::Fg, 1.0)),
        ChildOf(plate),
    ));
    let (all, matching) = picker.counts();
    let head = ui.boxed(c, plate, UiStyle { gap: Some(1.2), align: Some(AlignItems::Baseline), ..UiStyle::row() });
    let title = ui.text(c, head, Role::DisplayBold, 1.6, Tone::Fg, "Change gun");
    c.entity(title).insert(UiParityId::new("rungs.title"));
    let games = picker.games.len();
    ui.text(
        c,
        head,
        Role::Mono,
        0.56,
        Tone::Fg2,
        &format!("{all} guns \u{00b7} {games} {} \u{00b7} you stay on the one you pick", if games == 1 { "game" } else { "games" }),
    );
    // The filter, with the keys at once.
    let line = ui.boxed(c, plate, UiStyle { gap: Some(1.0), ..UiStyle::row() });
    let field = ui.text_field(
        c,
        line,
        &picker.filter,
        "Type to filter: name, id, class or game",
        // Empty: the placeholder (the field still underlined as focused).
        (!picker.filter.is_empty()).then(|| picker.filter.len().into()),
        false,
        // Half the plate's inside, growing with it.
        (100.0 - 2.0 * side - 3.6) * 0.5,
        State::focused(true),
    );
    c.entity(field).insert(UiParityId::new("rungs.filter"));
    if picker.filtering() {
        ui.text(c, line, Role::Mono, 0.56, Tone::Fg2, &format!("{matching} of {all} match"));
    }
    // The tree, a window of rows around the cursor.
    let tree = c
        .spawn((
            UiStyle { grow: Some(1.0), ..UiStyle::column() },
            Node { overflow: Overflow::clip(), ..default() },
            UiParityId::new("rungs.tree"),
            ChildOf(plate),
        ))
        .id();
    let rows = picker.rows();
    if rows.is_empty() {
        let none = ui.boxed(c, tree, UiStyle { h: Some(2.05), pad: Some([0.0, 0.6].into()), ..UiStyle::row() });
        ui.text(c, none, Role::Mono, 0.62, Tone::Fg2, &format!("No gun matches \u{201c}{}\u{201d}", picker.filter.trim()));
    }
    let first = picker.cursor.saturating_sub(WINDOW - 1).min(rows.len().saturating_sub(WINDOW));
    for (n, (i, row)) in rows.iter().enumerate().skip(first).take(WINDOW).enumerate() {
        tree_row(c, ui, look, picker, tree, i, *row, n + 1);
    }
    // The keys, then the actions: Cancel, Pick.
    let keys = ui.boxed(c, plate, UiStyle { gap: Some(1.4), ..UiStyle::row() });
    c.entity(keys).insert(UiParityId::new("rungs.keys"));
    for (k, w) in [("Type", "Filter"), ("\u{2191}\u{2193}", "Row"), ("\u{2190}\u{2192}", "Fold / open")] {
        let g = ui.boxed(c, keys, UiStyle { gap: Some(0.45), ..UiStyle::row() });
        ui.kbd(c, g, k, 0.54, Tone::Fg2);
        ui.text(c, g, Role::Mono, 0.54, Tone::Fg2, w);
    }
    let acts = ui.boxed(c, plate, UiStyle { gap: Some(0.7), ..UiStyle::row() });
    ui.boxed(c, acts, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    let cancel = ui.button(c, acts, Kind::Secondary, "Cancel", Some("Esc"), 2.4, State::NONE);
    c.entity(cancel).insert((RungPickerAction::Cancel, UiParityId::new("rungs.cancel")));
    let go = ui.button(c, acts, Kind::Primary, "Pick", Some("Enter"), HERO_H, State::NONE);
    c.entity(go).insert((RungPickerAction::Pick, UiParityId::new("rungs.pick")));
    scrim
}

/// One row of the tree: a game or class header (chevron, name, count) or
/// a rung (icon, name, id, tag, Current, number); `n` its place in the
/// window, from 1.
#[allow(clippy::too_many_arguments)]
fn tree_row(c: &mut Commands, ui: &Ui, look: &Look, picker: &RungPicker, tree: Entity, i: usize, row: PickRow, n: usize) {
    let focus = i == picker.cursor;
    let depth = match row {
        PickRow::Game(_) => 0,
        PickRow::Class(..) => 1,
        PickRow::Rung(..) => 2,
    };
    let h = if depth == 2 { 1.9 } else { 2.05 };
    let pad_l = [0.6, 2.0, 3.4][depth];
    let rest = if depth == 0 { 0.03 } else { 0.0 };
    let base = ui.color(Tone::Fg, if focus { 0.06 } else { rest });
    let over = ui.color(Tone::Fg, if focus { 0.06 } else { rest + 0.035 });
    let id = |part: &str| UiParityId::new(format!("rungs.row.{n}{part}"));
    let r = c
        .spawn((
            UiStyle { h: Some(h), gap: Some(0.7), pad: Some([0.0, 0.6, 0.0, pad_l].into()), shrink: Some(0.0), ..UiStyle::row() },
            Node::default(),
            BackgroundColor(base),
            chimera_ui::controls::Hover { base, over },
            RungPickerAction::Row(i),
            chimera_ui::controls::clickable(),
            id(""),
            ChildOf(tree),
        ))
        .id();
    ui.hairline(c, r, Tone::Fg5, 1.0);
    if focus {
        ui.focus_bar(c, r, 0.24);
    }
    let count = |c: &mut Commands, n: usize| {
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        ui.text(c, r, Role::Mono, 0.52, Tone::Fg2, &n.to_string());
    };
    let chevron = |c: &mut Commands| {
        let open = picker.filtering() || !picker.is_folded(row);
        let chev = ui.boxed(c, r, UiStyle { w: Some(0.8), shrink: Some(0.0), ..UiStyle::row() });
        c.entity(chev).insert((RungPickerAction::Fold(i), chimera_ui::controls::clickable()));
        let h = look.px(0.5);
        let tone = look.tone(Tone::Fg2);
        c.entity(chev).with_children(|p| {
            signal::icons::spawn(p, look, if open { "chevron" } else { "chev_right" }, h, tone);
        });
    };
    match row {
        PickRow::Game(g) => {
            chevron(c);
            ui.text(c, r, Role::DisplayBold, 0.9, Tone::Fg, &picker.games[g].name);
            count(c, picker.games[g].count());
        }
        PickRow::Class(g, k) => {
            chevron(c);
            let class = &picker.games[g].classes[k];
            ui.text(c, r, Role::DisplayRegular, 0.8, Tone::Fg, &class.name);
            count(c, class.rungs.len());
        }
        PickRow::Rung(..) => {
            let Some(rung) = picker.rung(row) else { return };
            // The icon in a fixed slot, so the names line up.
            let slot = c
                .spawn((
                    UiStyle { w: Some(3.4), h: Some(1.3), shrink: Some(0.0), ..UiStyle::row() }.justify(JustifyContent::Center),
                    Node::default(),
                    Pickable::IGNORE,
                    id(".icon"),
                    ChildOf(r),
                ))
                .id();
            let (ih, tone) = (look.px(1.05), look.tone(if focus { Tone::Fg } else { Tone::Fg2 }));
            let gun = rung.id.clone();
            let class = rung.icon_class;
            c.entity(slot).with_children(|p| {
                signal::gun_icon(p, look, &gun, class, ih, tone);
            });
            // The name and id take the room left and are cut at its end (a
            // narrow window, 4:3), so the tags and numbers stay aligned.
            let body = c
                .spawn((
                    // Its width the room left (a zero basis, grown), not
                    // its text's; a plain node (a `UiStyle` writes the
                    // whole `Node`, its clip and basis lost).
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: ui.units.val(0.7),
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        flex_basis: Val::Px(0.0),
                        min_width: Val::Px(0.0),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    Pickable::IGNORE,
                    ChildOf(r),
                ))
                .id();
            let name = ui.text(c, body, Role::DisplayRegular, 0.8, Tone::Fg, &rung.name);
            c.entity(name).insert(id(".name"));
            // The id in the secondary text's tone (readable at 720p).
            ui.text(c, body, Role::Mono, 0.5, Tone::Fg2, &rung.id);
            // The tag, Current and the number in fixed columns at the right,
            // so rows line up.
            let col = |c: &mut Commands, w: f32| ui.boxed(c, r, UiStyle { w: Some(w), shrink: Some(0.0), ..UiStyle::row() });
            let tags = col(c, TAG_COL);
            if let Some(t) = &rung.tag {
                let (node, label) = widgets::tag(ui.theme, ui.kit, &ui.units, TagKind::Accent, t, 0.46);
                let tag = c.spawn((node, Pickable::IGNORE, id(".tag"), ChildOf(tags))).id();
                c.spawn((label, ChildOf(tag)));
            }
            let now = col(c, CURRENT_COL);
            if picker.current == Some(rung.index) {
                let (node, label) = widgets::tag(ui.theme, ui.kit, &ui.units, TagKind::You, "Current", 0.46);
                let tag = c.spawn((node, Pickable::IGNORE, id(".current"), ChildOf(now))).id();
                c.spawn((label, ChildOf(tag)));
            }
            let num = ui.boxed(c, r, UiStyle { w: Some(2.8), shrink: Some(0.0), ..UiStyle::row() }.justify(JustifyContent::FlexEnd));
            ui.text(c, num, Role::Mono, 0.52, Tone::Fg2, &format!("#{}", rung.index + 1));
        }
    }
}
