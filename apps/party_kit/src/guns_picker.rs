//! The Host page's Guns dialog, version 2 (docs/plans/active/
//! guns-picker-v2.md, steps 3 and 4; the approved mockup "Host Guns
//! Picker" v2): the host's Guns rules (`chimera_sim::minigame::gun_rules`)
//! edited as a preset, the ladder's shape, folding rules and a browser of
//! gun families for single exceptions. The old dialog (one grid of ~2,100
//! cards, `mp_guns_off`) is gone; its guns off are read as bans when the
//! dialog opens and cleared at Done.
//!
//! - **Data** (`GunCatalogue`, `GunsListing`): the gun families a drawn
//!   ladder may deal (`LadderCatalogue`: `auto::report`'s playable games,
//!   without what is Unavailable, `auto::drawable_all`; each gun's family
//!   `auto::family_of`, effective class `auto::effective_class`, stage and
//!   power `auto::powers`), a family named by its shortest member's name.
//!   Made when the dialog opens (`refresh`). Combat Arms' 2,061 weapons
//!   are its 484 families here.
//! - **State** (`GunsPicker`): the host's saved presets, the Host page's
//!   preset menu and, while the dialog is open, the working copy
//!   (`Working`: the rules, the rungs, the preset it started from, the
//!   folds, the search, Group by and Show, the cursor, an undo).
//! - **The dialog** (`spawn`, a `chimera_ui::Modal` with its own scrim:
//!   nothing behind it takes the pointer or the wheel): the preset
//!   dropdown (built-ins, then the host's; Save, Save as, Copy as text,
//!   Paste; a load, a reset or a paste can be undone), the ladder strip
//!   (one cell per rung coloured by stage, the rungs the pool can't fill
//!   hatched, the pool tile: OK, amber when short, red when empty, Done
//!   disabled then), three columns (two below 1600 px: the help tile
//!   goes): **Rules** (folding sections: Games with Solo, their counts and
//!   big toggles; Classes by stage, each stage folding; Ladder: Rungs as a
//!   number field with − and + within the curve's `[rungs]`, Skins and When
//!   short segmented; a reset key on each row that differs from the
//!   preset), the **browser** (search, Group by, Show with the Exceptions
//!   count, a virtualised grid of family cards, weakest first: a click on
//!   one the rules let in bans it, on one they leave out allows it, again
//!   undoes it), the **help tile** (the focused row's help, and which wins:
//!   Unavailable, Ban, Allowed, Rules). Done sets the world's rules (the
//!   server's cvars: `GunRules`, `LadderRungs`), Cancel drops the copy.
//! - **Keys** (`keys`): Esc Cancel (or closes the menu, lets go of the
//!   search), Enter Done, `/` the search, Tab the area (rules, browser),
//!   ↑↓ the row or the card, Space acts on it (a toggle, a fold, a card), ←→
//!   change a value or fold, Ctrl+Z undo.
//! - **Host page** (step 4, `page_field`): the Guns field is a preset
//!   dropdown (its last entry opens the dialog), the row's note the rules'
//!   summary (`gun_rules::summary`).
//!
//! `ui_scene play guns [default|collapsed|classoff|bans|short|menu]`
//! stages the dialog's states (`stage`).
//!
//! Known shortcuts: no per-skin bans yet (a card is a family; a weapon id
//! ban from the console still works); no controller bumpers; the example
//! ladder (Reroll) and the "a joiner lacks a game" state of the mockup are
//! not drawn; the pool and "plays N" are the dialog's estimate (the
//! draw's hand-off between stages, not its jitter); the help tile follows
//! the keyboard's row, not the pointer.

use std::collections::BTreeSet;

use bevy::{prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_client::client::ui_sound::UiCue;
use chimera_core::game_parts::GunClass;
use chimera_sim::minigame::{
    gun_rules::{self, Decided, GunRules, LadderRungs, Preset, Skins, WhenShort},
    power::{Band, PowerCurve},
};
use chimera_ui::{
    Role, Theme, Tone, UiSlant, UiStyle, VCursor, VHit, VNav, VSection, VShape, VSlot, VirtualList, VirtualLists,
    controls::{Kind, State, StepperRange, ToggleSlide, Ui},
    form::BadgeKind,
    text_edit::Caret,
};

/// One gun family (module doc): the guns that are one gun in hand.
#[derive(Clone, Debug, PartialEq)]
pub struct GunFamily {
    /// Its family id (`auto::family_of`): what bans and allows name.
    pub id: String,
    pub game: String,
    /// Its shortest member's name.
    pub name: String,
    /// Its effective class (`auto::effective_class`, the grenades'
    /// `Grenade`).
    pub class: GunClass,
    /// Its stage (`power::band_of`).
    pub band: Band,
    /// Its power score (the browser's weakest-first order).
    pub score: f32,
    /// Its weapons (skins, editions, levels).
    pub members: Vec<String>,
}

/// What the dialog lists (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct GunsListing {
    /// The games, by id and name, in report order.
    pub games: Vec<(String, String)>,
    pub families: Vec<GunFamily>,
    /// Bumped by each `refresh` (the Host page redraws by it).
    pub generation: u64,
}

impl GunsListing {
    pub fn game_ids(&self) -> Vec<&str> {
        self.games.iter().map(|(g, _)| g.as_str()).collect()
    }
}

/// Where the listing comes from.
pub trait GunCatalogue: Send + Sync {
    fn listing(&self, world: &World) -> GunsListing;
}

/// The families drawn ladders may deal (module doc).
pub struct LadderCatalogue;

impl GunCatalogue for LadderCatalogue {
    fn listing(&self, world: &World) -> GunsListing {
        use chimera_sim::{
            minigame::{auto, power::PowerReaders},
            weapon::{
                data::WeaponCatalog,
                names::{GunNameRules, id_name, is_string_token},
            },
        };
        let mut caps = auto::report(world);
        auto::drawable_all(world, &mut caps);
        auto::drop_not_players(world, &mut caps);
        caps.retain(|c| c.playable());
        let empty = WeaponCatalog::default();
        let catalog = world.get_resource::<WeaponCatalog>().unwrap_or(&empty);
        let none = GunNameRules::default();
        let names = world.get_resource::<GunNameRules>().unwrap_or(&none);
        let no_readers = PowerReaders::default();
        let readers = world.get_resource::<PowerReaders>().unwrap_or(&no_readers);
        let curve = PowerCurve::shipped();
        let powers = auto::powers(&caps, catalog, readers, &curve);
        let name = |id: &str| {
            let base = catalog.get(id).and_then(|d| d.hud.name.clone()).filter(|n| !is_string_token(n)).unwrap_or_else(|| id_name(id));
            names.full(id, &base)
        };
        let mut out = GunsListing::default();
        let mut at: std::collections::HashMap<(String, String), usize> = std::collections::HashMap::new();
        for c in &caps {
            out.games.push((c.game.to_string(), crate::maps_picker::game_name(c.game)));
            let guns = c.guns.iter().map(|g| (g.id, g.class)).chain(c.grenades.iter().map(|g| (*g, GunClass::Grenade)));
            for (id, class) in guns {
                let family = auto::family_of(catalog, id);
                let class = if class == GunClass::Grenade { class } else { auto::effective_class(id, class, catalog, readers, &curve) };
                let p = powers.get(id).copied();
                let band = p.map_or(curve.class_band(class), |p| p.band);
                let n = name(id);
                match at.get(&(c.game.to_string(), family.clone())) {
                    Some(&i) => {
                        let f = &mut out.families[i];
                        f.members.push(id.to_string());
                        if n.chars().count() < f.name.chars().count() {
                            f.name = n;
                        }
                    }
                    None => {
                        at.insert((c.game.to_string(), family.clone()), out.families.len());
                        out.families.push(GunFamily {
                            id: family,
                            game: c.game.to_string(),
                            name: n,
                            class,
                            band,
                            score: p.map_or(0.0, |p| p.score),
                            members: vec![id.to_string()],
                        });
                    }
                }
            }
        }
        out
    }
}

/// The catalogue in use.
#[derive(Resource)]
pub struct GunsSource(pub Box<dyn GunCatalogue>);

impl Default for GunsSource {
    fn default() -> Self {
        GunsSource(Box::new(LadderCatalogue))
    }
}

/// Where the host's presets live (`gun_rules::presets_dir`; a test's own).
#[derive(Resource, Clone, Debug)]
pub struct GunsPresetDir(pub Option<std::path::PathBuf>);

impl Default for GunsPresetDir {
    fn default() -> Self {
        GunsPresetDir(gun_rules::presets_dir())
    }
}

/// Lists the families again (`GunsSource` into `GunsListing`).
pub fn refresh(world: &mut World) {
    world.init_resource::<GunsSource>();
    let mut listing = world.resource_scope::<GunsSource, _>(|w, s| s.0.listing(w));
    listing.generation = world.get_resource::<GunsListing>().map_or(1, |l| l.generation + 1);
    world.insert_resource(listing);
}

/// The browser's grouping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Group {
    #[default]
    Game,
    Stage,
    None,
}

/// What the browser shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Show {
    #[default]
    All,
    In,
    Out,
    Exceptions,
}

/// Which column the keys are in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Area {
    #[default]
    Rules,
    Browser,
}

/// A rules row a reset key puts back (`GunsAct::ResetRow`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKey {
    Game(usize),
    Class(GunClass),
    Rungs,
    Skins,
    Short,
}

/// The working copy while the dialog is open (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Working {
    pub rules: GunRules,
    pub rungs: usize,
    /// The app has a rungs setting (`LadderRungs`).
    pub has_rungs: bool,
    /// The preset it started from (or last loaded, saved), "" none.
    pub base: String,
    /// Folded rule sections: `games`, `classes`, `ladder`, `stage:<band>`.
    pub folded: BTreeSet<String>,
    /// The preset menu is open: its active entry.
    pub menu: Option<usize>,
    /// Save as: the name being typed.
    pub naming: Option<(String, Caret)>,
    pub search: String,
    pub search_focus: bool,
    pub caret: Caret,
    pub group: Group,
    pub show: Show,
    pub area: Area,
    /// The rules row with the keys (`rule_rows`).
    pub cursor: usize,
    /// What a load, reset or paste replaced (Ctrl+Z, the toast's Undo).
    pub undo: Option<(GunRules, usize, String)>,
    pub toast: Option<String>,
    /// The toggle flipped last and when (its slab slides).
    pub flip: Option<(String, f64)>,
}

/// The dialog's state (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct GunsPicker {
    pub open: Option<Working>,
    /// The host's own presets (`gun_rules::saved_in`).
    pub saved: Vec<Preset>,
    /// The Host page's preset menu is open: its active entry.
    pub page_menu: Option<usize>,
    /// The dialog had the keys this frame (the page leaves them).
    pub took_keys: bool,
}

impl GunsPicker {
    /// The built-ins, then the host's.
    pub fn presets(&self) -> Vec<Preset> {
        let mut all = Preset::builtin();
        all.extend(self.saved.iter().cloned());
        all
    }

    /// The Host page's Guns line for `rules` (`gun_rules::summary`).
    pub fn summary(&self, rules: &GunRules, rungs: usize, note: Option<&str>) -> String {
        gun_rules::summary(&self.presets(), rules, rungs, note)
    }
}

/// The dialog's root (tests find it).
#[derive(Component, Clone, Copy, Debug)]
pub struct GunsDialog;

/// A family card (tests find them): its index in `GunsListing::families`.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GunCard(pub usize);

/// The browser's list key (`VirtualLists`).
pub const LIST: &str = "guns_browser";
/// The rules column's scroll view.
pub const RULES_SCROLL: &str = "guns_rules";

/// What a click (or a key) in the dialog or the Host page's Guns field
/// does.
#[derive(Component, Message, Clone, Debug, PartialEq)]
pub enum GunsAct {
    Done,
    Cancel,
    /// Back to the preset it started from.
    Reset,
    ResetRow(RowKey),
    Fold(String),
    /// A game on or off (its index in `GunsListing::games`).
    Game(usize),
    Solo(usize),
    Class(GunClass),
    Rungs(i32),
    Skins(Skins),
    Short(WhenShort),
    Group(Group),
    Show(Show),
    /// A family card (ban or allow; again: back to the rules).
    Card(usize),
    /// The preset menu opens or closes.
    Menu,
    /// The preset menu's entry (`menu_items`).
    MenuPick(usize),
    /// Save as's name typed (Enter).
    SaveAs(String),
    Undo,
    FocusSearch,
    ClearSearch,
    /// The Host page: its preset menu opens or closes.
    PageMenu,
    /// The Host page: its menu's entry (a preset, or the last: the dialog).
    PagePick(usize),
    /// The Host page: the dialog opens.
    Open,
}

/// An entry of the dialog's preset menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuItem {
    Load(usize),
    Save,
    SaveAs,
    Copy,
    Paste,
}

/// The preset menu's entries for `w` (Save only over a saved preset).
pub fn menu_items(picker: &GunsPicker, w: &Working) -> Vec<MenuItem> {
    let all = picker.presets();
    let mut out: Vec<MenuItem> = (0..all.len()).map(MenuItem::Load).collect();
    if picker.saved.iter().any(|p| p.name.eq_ignore_ascii_case(&w.base)) {
        out.push(MenuItem::Save);
    }
    out.extend([MenuItem::SaveAs, MenuItem::Copy, MenuItem::Paste]);
    out
}

fn menu_word(picker: &GunsPicker, item: &MenuItem) -> String {
    match item {
        MenuItem::Load(i) => picker.presets().get(*i).map(|p| p.name.clone()).unwrap_or_default(),
        MenuItem::Save => "Save".into(),
        MenuItem::SaveAs => "Save as".into(),
        MenuItem::Copy => "Copy as text".into(),
        MenuItem::Paste => "Paste".into(),
    }
}

// ---------------------------------------------------------------------
// The model: pure functions of the listing and the working copy.

/// Whether the rules (games, classes) let `f` in.
pub fn by_rules(f: &GunFamily, rules: &GunRules) -> bool {
    rules.game_on(&f.game) && rules.class_on(f.class)
}

/// How the draw decides `f`.
pub fn decided(f: &GunFamily, rules: &GunRules) -> Decided {
    rules.decided(&f.id, by_rules(f, rules))
}

/// The pool as the dialog sees it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PoolView {
    /// Families dealt, by stage.
    pub by_band: [usize; 7],
    /// Families dealt, by game (`GunsListing::games`' order).
    pub by_game: Vec<usize>,
    pub families: usize,
    /// Their skins beyond one a family.
    pub extra_skins: usize,
    /// Families Borrow may take (classes off, games on, not banned).
    pub reserve: usize,
    /// Each stage's rungs at the chosen length (`gun_rules::stage_rungs`).
    pub stages: [usize; 7],
    /// The rungs the next ladder plays (estimate), of `rungs`.
    pub plays: usize,
    pub rungs: usize,
    /// The longest ladder this pool fills without When short.
    pub fills: usize,
    /// Exceptions (banned or allowed families).
    pub exceptions: usize,
}

impl PoolView {
    pub fn empty(&self) -> bool {
        self.families == 0
    }

    pub fn short(&self) -> bool {
        !self.empty() && self.plays < self.rungs
    }
}

/// The pool of `rules` over `listing` for a ladder of `rungs`.
pub fn pool(listing: &GunsListing, rules: &GunRules, rungs: usize) -> PoolView {
    let curve = PowerCurve::shipped();
    let mut p = PoolView { by_game: vec![0; listing.games.len()], rungs, stages: gun_rules::stage_rungs(rungs, &curve), ..default() };
    for f in &listing.families {
        let d = decided(f, rules);
        if d.exception() {
            p.exceptions += 1;
        }
        if d.dealt() {
            p.by_band[f.band.index()] += 1;
            if let Some(g) = listing.games.iter().position(|(g, _)| *g == f.game) {
                p.by_game[g] += 1;
            }
            p.families += 1;
            p.extra_skins += f.members.len().saturating_sub(1);
        } else if d == Decided::Out && rules.game_on(&f.game) {
            p.reserve += 1;
        }
    }
    let melee = p.stages[Band::Melee.index()];
    let gun_rungs = rungs.saturating_sub(melee);
    let guns = p.families;
    let dealt = match rules.short {
        WhenShort::Shorter => gun_rungs.min(guns),
        WhenShort::Repeat => {
            if guns > 0 {
                gun_rungs
            } else {
                0
            }
        }
        WhenShort::Borrow => gun_rungs.min(guns + p.reserve),
    };
    p.plays = dealt + melee;
    let bounds = curve.rungs;
    p.fills = (bounds.min..=bounds.max).rev().find(|n| n.saturating_sub(gun_rules::stage_rungs(*n, &curve)[Band::Melee.index()]) <= guns).unwrap_or(bounds.min);
    p
}

/// A row of the rules column (the keys walk them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleRow {
    Head(&'static str),
    Game(usize),
    Stage(Band),
    Class(GunClass),
    Rungs,
    Skins,
    Short,
}

/// The stages the Classes section lists (the knife's is no class).
pub const STAGES: [Band; 6] = [Band::WarmUp, Band::Rising, Band::Peak, Band::Heavy, Band::Specials, Band::Grenade];

/// Every class in the order the Classes section lists them.
pub fn classes() -> Vec<GunClass> {
    let mut c = GunClass::ALL.to_vec();
    c.push(GunClass::Grenade);
    c
}

/// The classes of stage `band` the listing has (or the rules turned off).
pub fn stage_classes(listing: &GunsListing, rules: &GunRules, band: Band) -> Vec<GunClass> {
    let curve = PowerCurve::shipped();
    classes().into_iter().filter(|c| curve.class_band(*c) == band && (listing.families.iter().any(|f| f.class == *c) || !rules.class_on(*c))).collect()
}

/// The rules rows as shown (folded sections leave their head).
pub fn rule_rows(listing: &GunsListing, w: &Working) -> Vec<RuleRow> {
    let mut out = vec![RuleRow::Head("games")];
    if !w.folded.contains("games") {
        out.extend((0..listing.games.len()).map(RuleRow::Game));
    }
    out.push(RuleRow::Head("classes"));
    if !w.folded.contains("classes") {
        for b in STAGES {
            let cs = stage_classes(listing, &w.rules, b);
            if cs.is_empty() {
                continue;
            }
            out.push(RuleRow::Stage(b));
            if !w.folded.contains(&stage_key(b)) {
                out.extend(cs.into_iter().map(RuleRow::Class));
            }
        }
    }
    out.push(RuleRow::Head("ladder"));
    if !w.folded.contains("ladder") {
        if w.has_rungs {
            out.push(RuleRow::Rungs);
        }
        out.extend([RuleRow::Skins, RuleRow::Short]);
    }
    out
}

pub fn stage_key(b: Band) -> String {
    format!("stage:{}", b.name())
}

/// A stage's words.
pub fn stage_name(b: Band) -> &'static str {
    match b {
        Band::WarmUp => "Warm-up",
        Band::Rising => "Rising",
        Band::Peak => "Peak",
        Band::Heavy => "Heavy",
        Band::Specials => "Specials",
        Band::Grenade => "Grenade",
        Band::Melee => "Melee",
    }
}

/// A class's words.
pub fn class_name(c: GunClass) -> &'static str {
    match c {
        GunClass::Pistol => "Pistols",
        GunClass::Smg => "SMGs",
        GunClass::Shotgun => "Shotguns",
        GunClass::Carbine => "Carbines",
        GunClass::Rifle => "Rifles",
        GunClass::Heavy => "Machine guns",
        GunClass::Marksman => "Marksman rifles",
        GunClass::Sniper => "Sniper rifles",
        GunClass::HeavyPistol => "Heavy pistols",
        GunClass::Launcher => "Launchers",
        GunClass::Flamethrower => "Flamethrowers",
        GunClass::Grenade => "Grenades",
    }
}

/// A game's short name (the pool tile's mix).
pub fn game_short(id: &str) -> String {
    match id {
        "cs_source" => "CS:S".into(),
        "combat_arms" => "CA".into(),
        "tf2" => "TF2".into(),
        "dods" => "DoD:S".into(),
        "halo_ce" => "Halo".into(),
        "hl2dm" => "HL2:DM".into(),
        o => o.to_ascii_uppercase(),
    }
}

/// The browser's sections: (key, title, families), each weakest first.
pub fn browse(listing: &GunsListing, w: &Working) -> Vec<(String, String, Vec<usize>)> {
    let q = w.search.trim().to_lowercase();
    let game_name = |g: &str| listing.games.iter().find(|(id, _)| id == g).map_or(g.to_string(), |(_, n)| n.clone());
    let keep = |f: &GunFamily| {
        let d = decided(f, &w.rules);
        let shown = match w.show {
            Show::All => true,
            Show::In => d.dealt(),
            Show::Out => !d.dealt(),
            Show::Exceptions => d.exception(),
        };
        shown
            && (q.is_empty()
                || f.name.to_lowercase().contains(&q)
                || f.id.to_lowercase().contains(&q)
                || game_name(&f.game).to_lowercase().contains(&q)
                || class_name(f.class).to_lowercase().contains(&q)
                || f.members.iter().any(|m| m.to_lowercase().contains(&q)))
    };
    let mut kept: Vec<usize> = (0..listing.families.len()).filter(|i| keep(&listing.families[*i])).collect();
    kept.sort_by(|a, b| {
        let (a, b) = (&listing.families[*a], &listing.families[*b]);
        a.band.cmp(&b.band).then(a.score.total_cmp(&b.score)).then(a.name.cmp(&b.name))
    });
    match w.group {
        Group::None => vec![("all".into(), String::new(), kept)],
        Group::Game => listing
            .games
            .iter()
            .map(|(g, n)| (format!("game:{g}"), n.clone(), kept.iter().copied().filter(|i| listing.families[*i].game == *g).collect::<Vec<_>>()))
            .filter(|s| !s.2.is_empty())
            .collect(),
        Group::Stage => Band::ALL
            .iter()
            .map(|b| (stage_key(*b), stage_name(*b).to_string(), kept.iter().copied().filter(|i| listing.families[*i].band == *b).collect::<Vec<_>>()))
            .filter(|s| !s.2.is_empty())
            .collect(),
    }
}

/// The preset `w` started from (its rules, its rungs), if any.
fn base_of(picker: &GunsPicker, w: &Working) -> Option<Preset> {
    picker.presets().into_iter().find(|p| p.name.eq_ignore_ascii_case(&w.base))
}

/// The preset field's words: the matching preset's name, or the one it
/// started from "(edited)", or "Custom".
pub fn preset_label(picker: &GunsPicker, w: &Working) -> String {
    let all = picker.presets();
    match gun_rules::matching(&all, &w.rules, w.rungs) {
        Some(p) => p.name.clone(),
        None if !w.base.is_empty() => format!("{} (edited)", w.base),
        None => "Custom".into(),
    }
}

/// Whether row `k` differs from the preset it started from.
pub fn row_changed(picker: &GunsPicker, listing: &GunsListing, w: &Working, k: RowKey) -> bool {
    let base = base_of(picker, w);
    let rules = base.as_ref().map(Preset::rules).unwrap_or_default();
    let rungs = base.as_ref().and_then(|p| p.rungs);
    match k {
        RowKey::Game(i) => listing.games.get(i).is_some_and(|(g, _)| rules.game_on(g) != w.rules.game_on(g)),
        RowKey::Class(c) => rules.class_on(c) != w.rules.class_on(c),
        RowKey::Rungs => rungs.is_some_and(|n| n != w.rungs),
        RowKey::Skins => rules.skins != w.rules.skins,
        RowKey::Short => rules.short != w.rules.short,
    }
}

/// The rungs' bounds (the curve's `[rungs]`).
pub fn rung_bounds() -> chimera_sim::minigame::power::RungBounds {
    PowerCurve::shipped().rungs
}

/// The rules' and the rungs' state in the world (the server's cvars).
fn world_rules(world: &World) -> (GunRules, usize, bool) {
    let rules = world.get_resource::<GunRules>().cloned().unwrap_or_default();
    let rungs = world.get_resource::<LadderRungs>().map(|r| r.0);
    (rules, rungs.unwrap_or(rung_bounds().default), rungs.is_some())
}

/// Reads the host's presets (`GunsPresetDir`).
pub fn load_saved(world: &mut World) {
    let dir = world.get_resource_or_init::<GunsPresetDir>().0.clone();
    let saved = dir.map(|d| gun_rules::saved_in(&d)).unwrap_or_default();
    world.get_resource_or_init::<GunsPicker>().saved = saved;
}

/// Opens the dialog over the world's rules (the old guns off read as
/// bans).
pub fn open(world: &mut World) {
    refresh(world);
    load_saved(world);
    let (mut rules, rungs, has_rungs) = world_rules(world);
    if let Some(off) = world.get_resource::<chimera_sim::minigame::auto::GunsOff>() {
        for id in gun_rules::entries(&off.0) {
            rules.ban = gun_rules::set_listed(&rules.ban, id, true);
        }
    }
    let mut picker = world.resource_mut::<GunsPicker>();
    let all = picker.presets();
    let base = gun_rules::matching(&all, &rules, rungs).map(|p| p.name.clone()).unwrap_or_default();
    picker.open = Some(Working { rules, rungs, has_rungs, base, ..default() });
    picker.page_menu = None;
}

/// The window is narrower than 1600 px (two columns, no help tile).
pub fn narrow(u: &chimera_ui::UiUnits) -> bool {
    u.height < 900.0
}

/// A working copy replaced (a preset, a reset, a paste): undoable.
fn replace(w: &mut Working, rules: GunRules, rungs: Option<usize>, base: &str, toast: String) {
    w.undo = Some((w.rules.clone(), w.rungs, w.base.clone()));
    w.rules = rules;
    if let Some(n) = rungs.filter(|_| w.has_rungs) {
        w.rungs = rung_bounds().clamp(n);
    }
    w.base = base.to_string();
    w.toast = Some(toast);
}

/// What the dialog's and the page's actions do.
pub fn act(world: &mut World) {
    let todo: Vec<GunsAct> = world.resource_mut::<Messages<GunsAct>>().drain().collect();
    for a in todo {
        act_one(world, a);
    }
}

fn cue(world: &mut World, name: &'static str) {
    if let Some(mut m) = world.get_resource_mut::<Messages<UiCue>>() {
        m.write(UiCue::new(name));
    }
}

fn act_one(world: &mut World, a: GunsAct) {
    let now = world.get_resource::<chimera_ui::UiClock>().map_or(0.0, |c| c.now);
    let listing = world.get_resource::<GunsListing>().cloned().unwrap_or_default();
    // The Host page's own.
    match a {
        GunsAct::PageMenu => {
            load_saved(world);
            let mut p = world.resource_mut::<GunsPicker>();
            p.page_menu = if p.page_menu.is_some() { None } else { Some(0) };
            if p.page_menu.is_some() {
                chimera_ui::modal::opening(world, "guns_page");
            }
            cue(world, "ui_select");
            return;
        }
        GunsAct::PagePick(i) => {
            let presets = world.resource::<GunsPicker>().presets();
            world.resource_mut::<GunsPicker>().page_menu = None;
            match presets.get(i) {
                Some(p) => {
                    world.insert_resource(p.rules());
                    if let (Some(n), Some(mut r)) = (p.rungs, world.get_resource_mut::<LadderRungs>()) {
                        r.0 = rung_bounds().clamp(n);
                    }
                    if let Some(mut off) = world.get_resource_mut::<chimera_sim::minigame::auto::GunsOff>() {
                        off.0.clear();
                    }
                }
                None => open(world),
            }
            cue(world, "ui_select");
            return;
        }
        GunsAct::Open => {
            open(world);
            cue(world, "ui_select");
            return;
        }
        _ => {}
    }
    let Some(mut w) = world.resource::<GunsPicker>().open.clone() else { return };
    let picker = world.resource::<GunsPicker>().clone();
    let mut sound = "ui_select";
    let mut opens = false;
    match a {
        GunsAct::Done => {
            if pool(&listing, &w.rules, w.rungs).empty() {
                cue(world, "ui_denied");
                return;
            }
            world.insert_resource(w.rules.clone());
            if let Some(mut r) = world.get_resource_mut::<LadderRungs>() {
                r.0 = w.rungs;
            }
            if let Some(mut off) = world.get_resource_mut::<chimera_sim::minigame::auto::GunsOff>() {
                off.0.clear();
            }
            if let Some(mut m) = world.get_resource_mut::<crate::main_menu::PartyMenu>() {
                m.dirty = true;
            }
            world.resource_mut::<GunsPicker>().open = None;
            cue(world, "ui_select");
            return;
        }
        GunsAct::Cancel => {
            world.resource_mut::<GunsPicker>().open = None;
            cue(world, "ui_back");
            return;
        }
        GunsAct::Reset => {
            let base = base_of(&picker, &w);
            let (rules, rungs) = base.as_ref().map_or((GunRules::default(), None), |p| (p.rules(), p.rungs));
            let name = base.map(|p| p.name).unwrap_or_default();
            replace(&mut w, rules, rungs, &name, "Reset to the preset".into());
        }
        GunsAct::ResetRow(k) => {
            let base = base_of(&picker, &w);
            let rules = base.as_ref().map(Preset::rules).unwrap_or_default();
            match k {
                RowKey::Game(i) => {
                    if let Some((g, _)) = listing.games.get(i) {
                        w.rules.set_game_on(g, rules.game_on(g));
                    }
                }
                RowKey::Class(c) => w.rules.set_class_on(c, rules.class_on(c)),
                RowKey::Rungs => {
                    if let Some(n) = base.as_ref().and_then(|p| p.rungs) {
                        w.rungs = n;
                    }
                }
                RowKey::Skins => w.rules.skins = rules.skins,
                RowKey::Short => w.rules.short = rules.short,
            }
        }
        GunsAct::Fold(k) => {
            if !w.folded.remove(&k) {
                w.folded.insert(k);
            }
        }
        GunsAct::Game(i) => {
            if let Some((g, _)) = listing.games.get(i) {
                let on = w.rules.game_on(g);
                w.rules.set_game_on(g, !on);
                w.flip = Some((format!("game:{g}"), now));
            }
        }
        GunsAct::Solo(i) => {
            if let Some((g, _)) = listing.games.get(i) {
                w.rules.solo(g, &listing.game_ids());
            }
        }
        GunsAct::Class(c) => {
            let on = w.rules.class_on(c);
            w.rules.set_class_on(c, !on);
            w.flip = Some((format!("class:{}", c.name()), now));
        }
        GunsAct::Rungs(d) => {
            let b = rung_bounds();
            let next = b.clamp((w.rungs as i64 + d as i64).max(0) as usize);
            if next == w.rungs {
                sound = "ui_denied";
            }
            w.rungs = next;
        }
        GunsAct::Skins(s) => w.rules.skins = s,
        GunsAct::Short(s) => w.rules.short = s,
        GunsAct::Group(g) => w.group = g,
        GunsAct::Show(s) => w.show = s,
        GunsAct::Card(i) => {
            if let Some(f) = listing.families.get(i) {
                let d = w.rules.cycle_family(&f.id, by_rules(f, &w.rules));
                w.toast = Some(match d {
                    Decided::Banned => format!("Banned {}", f.name),
                    Decided::Allowed => format!("Allowed {}", f.name),
                    _ => format!("{} back to the rules", f.name),
                });
                w.area = Area::Browser;
            }
        }
        GunsAct::Menu => {
            w.menu = if w.menu.is_some() { None } else { Some(0) };
            if w.menu.is_some() {
                // One popover open: the others close (after `w`'s borrow).
                opens = true;
            }
        }
        GunsAct::MenuPick(i) => {
            w.menu = None;
            match menu_items(&picker, &w).get(i).cloned() {
                Some(MenuItem::Load(p)) => {
                    if let Some(p) = picker.presets().get(p) {
                        replace(&mut w, p.rules(), p.rungs, &p.name, format!("Loaded {}", p.name));
                    }
                }
                Some(MenuItem::Save) => {
                    let name = w.base.clone();
                    save(world, &mut w, &name);
                }
                Some(MenuItem::SaveAs) => w.naming = Some((String::new(), Caret::default())),
                Some(MenuItem::Copy) => {
                    let text = Preset::of(&preset_label(&picker, &w), &w.rules, w.has_rungs.then_some(w.rungs)).to_text();
                    clipboard(world, |c| c.set(&text));
                    w.toast = Some("Copied the rules as text".into());
                }
                Some(MenuItem::Paste) => {
                    let text = clipboard(world, |c| c.get()).unwrap_or_default();
                    match Preset::from_text(&text) {
                        Ok(p) => {
                            let name = p.name.clone();
                            replace(&mut w, p.rules(), p.rungs, "", format!("Pasted {}", if name.is_empty() { "rules".into() } else { name }));
                        }
                        Err(_) => {
                            w.toast = Some("The clipboard holds no Guns preset".into());
                            sound = "ui_denied";
                        }
                    }
                }
                None => {}
            }
        }
        GunsAct::SaveAs(name) => save(world, &mut w, &name),
        GunsAct::Undo => match w.undo.take() {
            Some((r, n, b)) => {
                w.rules = r;
                w.rungs = n;
                w.base = b;
                w.toast = Some("Undone".into());
            }
            None => sound = "ui_denied",
        },
        GunsAct::FocusSearch => {
            w.search_focus = true;
            w.caret = Caret::end_of(&w.search);
            w.area = Area::Browser;
        }
        GunsAct::ClearSearch => {
            w.search.clear();
            w.caret = Caret::default();
            w.search_focus = true;
        }
        GunsAct::PageMenu | GunsAct::PagePick(_) | GunsAct::Open => {}
    }
    world.resource_mut::<GunsPicker>().open = Some(w);
    if opens {
        chimera_ui::modal::opening(world, "guns_dialog");
    }
    cue(world, sound);
}

/// The shared clipboard (or one of the dialog's own).
fn clipboard<R>(world: &mut World, f: impl FnOnce(&mut chimera_ui::clipboard::Clipboard) -> R) -> R {
    let mut c = world.get_resource_or_init::<chimera_ui::clipboard::Clipboard>();
    f(&mut c)
}

/// Saves `w` as preset `name` (`GunsPresetDir`), the saved list read
/// again; the toast says how it went.
fn save(world: &mut World, w: &mut Working, name: &str) {
    let name = name.trim();
    let dir = world.get_resource_or_init::<GunsPresetDir>().0.clone();
    let wrote = match (dir, name.is_empty()) {
        (_, true) => Err("a preset needs a name".to_string()),
        (None, _) => Err("no config folder".to_string()),
        (Some(d), _) => std::fs::create_dir_all(&d)
            .and_then(|_| std::fs::write(d.join(gun_rules::file_name(name)), Preset::of(name, &w.rules, w.has_rungs.then_some(w.rungs)).to_text()))
            .map_err(|e| e.to_string()),
    };
    match wrote {
        Ok(()) => {
            w.base = name.to_string();
            w.toast = Some(format!("Saved {name}"));
        }
        Err(e) => w.toast = Some(format!("Not saved: {e}")),
    }
    let saved = world.get_resource::<GunsPresetDir>().and_then(|d| d.0.clone()).map(|d| gun_rules::saved_in(&d)).unwrap_or_default();
    world.resource_mut::<GunsPicker>().saved = saved;
}

/// Presses on the dialog's and the field's nodes become actions.
pub fn clicks(pressed: Query<(&Interaction, &GunsAct), Changed<Interaction>>, mut out: MessageWriter<GunsAct>) {
    for (i, a) in &pressed {
        if *i == Interaction::Pressed {
            out.write(a.clone());
        }
    }
}

/// The dialog's keys (module doc). Typing goes to the search or the Save
/// as name while they have the keyboard.
#[allow(clippy::too_many_arguments)]
pub fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut typed: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut picker: ResMut<GunsPicker>,
    listing: Option<Res<GunsListing>>,
    pages: Res<crate::signal_pages::SignalPages>,
    mut lists: ResMut<VirtualLists>,
    (mut own, shared): (Local<chimera_ui::clipboard::Clipboard>, Option<ResMut<chimera_ui::clipboard::Clipboard>>),
    mut out: MessageWriter<GunsAct>,
    mut cues: MessageWriter<UiCue>,
) {
    use bevy::input::{ButtonState, keyboard::Key};
    picker.took_keys = picker.open.is_some();
    let typed: Vec<bevy::input::keyboard::KeyboardInput> = typed.read().cloned().collect();
    if picker.open.is_none() {
        return;
    }
    if pages.up != Some(crate::main_menu::MenuPage::Play) {
        if pages.up.is_some() {
            picker.open = None;
        }
        picker.took_keys = false;
        return;
    }
    let empty = GunsListing::default();
    let listing = listing.as_deref().unwrap_or(&empty);
    let clipboard = match shared {
        Some(c) => c.into_inner(),
        None => &mut *own,
    };
    let k = |c: KeyCode| keys.just_pressed(c);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let enter = k(KeyCode::Enter) || k(KeyCode::NumpadEnter);
    let Some(w) = picker.open.as_mut() else { return };
    let text_keys = |text: &mut String, caret: &mut Caret, clipboard: &mut chimera_ui::clipboard::Clipboard| {
        let mut changed = false;
        for e in typed.iter().filter(|e| e.state == ButtonState::Pressed) {
            if let Some(key) = chimera_ui::text_edit::text_key(&e.logical_key, e.key_code, ctrl, shift) {
                changed |= caret.key(text, key, clipboard, 48, false);
            }
        }
        changed
    };
    // Save as: the name.
    if let Some((name, caret)) = w.naming.as_mut() {
        if k(KeyCode::Escape) {
            w.naming = None;
        } else if enter {
            out.write(GunsAct::SaveAs(name.clone()));
            w.naming = None;
        } else {
            text_keys(name, caret, clipboard);
        }
        return;
    }
    if w.search_focus {
        if k(KeyCode::Escape) || enter {
            w.search_focus = false;
        } else {
            let (mut s, mut c) = (w.search.clone(), w.caret);
            if text_keys(&mut s, &mut c, clipboard) {
                w.search = s;
                lists.state(LIST).cursor = None;
            }
            w.caret = c;
        }
        return;
    }
    if let Some(active) = w.menu {
        let n = menu_items(&picker.clone(), picker.open.as_ref().unwrap()).len();
        let w = picker.open.as_mut().unwrap();
        if k(KeyCode::Escape) {
            w.menu = None;
        } else if enter || k(KeyCode::Space) {
            out.write(GunsAct::MenuPick(active));
        } else if k(KeyCode::ArrowDown) {
            w.menu = Some((active + 1).min(n.saturating_sub(1)));
        } else if k(KeyCode::ArrowUp) {
            w.menu = Some(active.saturating_sub(1));
        }
        return;
    }
    if k(KeyCode::Escape) {
        out.write(GunsAct::Cancel);
        return;
    }
    if enter {
        out.write(GunsAct::Done);
        return;
    }
    if ctrl && k(KeyCode::KeyZ) {
        out.write(GunsAct::Undo);
        return;
    }
    if typed.iter().any(|e| e.state == ButtonState::Pressed && matches!(&e.logical_key, Key::Character(c) if c.as_str() == "/")) {
        out.write(GunsAct::FocusSearch);
        return;
    }
    if k(KeyCode::Tab) {
        w.area = if w.area == Area::Rules { Area::Browser } else { Area::Rules };
        if w.area == Area::Browser && lists.state(LIST).cursor.is_none() {
            let first = lists.state(LIST).stops().into_iter().find(|c| matches!(c, VCursor::Item(..)));
            if let Some(c) = first {
                lists.state(LIST).select(c);
            }
        }
        cues.write(UiCue::new("ui_hover"));
        return;
    }
    match w.area {
        Area::Rules => {
            let rows = rule_rows(listing, w);
            let n = rows.len();
            if k(KeyCode::ArrowDown) && w.cursor + 1 < n {
                w.cursor += 1;
                cues.write(UiCue::new("ui_hover"));
            } else if k(KeyCode::ArrowUp) && w.cursor > 0 {
                w.cursor -= 1;
                cues.write(UiCue::new("ui_hover"));
            }
            let Some(row) = rows.get(w.cursor.min(n.saturating_sub(1))).copied() else { return };
            let space = k(KeyCode::Space);
            let dir = if k(KeyCode::ArrowLeft) {
                -1
            } else if k(KeyCode::ArrowRight) {
                1
            } else {
                0
            };
            let act = match row {
                RuleRow::Head(h) if space || dir != 0 => {
                    let folded = w.folded.contains(h);
                    (space || (dir < 0) != folded).then(|| GunsAct::Fold(h.to_string()))
                }
                RuleRow::Stage(b) if space || dir != 0 => {
                    let folded = w.folded.contains(&stage_key(b));
                    (space || (dir < 0) != folded).then(|| GunsAct::Fold(stage_key(b)))
                }
                RuleRow::Game(i) if space || dir != 0 => Some(GunsAct::Game(i)),
                RuleRow::Class(c) if space || dir != 0 => Some(GunsAct::Class(c)),
                RuleRow::Rungs if dir != 0 => Some(GunsAct::Rungs(dir)),
                RuleRow::Skins if space || dir != 0 => Some(GunsAct::Skins(if w.rules.skins == Skins::One { Skins::All } else { Skins::One })),
                RuleRow::Short if space || dir != 0 => {
                    let all = [WhenShort::Shorter, WhenShort::Repeat, WhenShort::Borrow];
                    let at = all.iter().position(|s| *s == w.rules.short).unwrap_or(0) as i32;
                    Some(GunsAct::Short(all[(at + if dir == 0 { 1 } else { dir }).rem_euclid(3) as usize]))
                }
                _ => None,
            };
            if let Some(a) = act {
                out.write(a);
            }
        }
        Area::Browser => {
            let navs = [(KeyCode::ArrowUp, VNav::Up), (KeyCode::ArrowDown, VNav::Down), (KeyCode::ArrowLeft, VNav::Left), (KeyCode::ArrowRight, VNav::Right)];
            for (key, nav) in navs {
                if k(key) && lists.state(LIST).nav(nav) {
                    cues.write(UiCue::new("ui_hover"));
                }
            }
            if k(KeyCode::Space)
                && let Some(VCursor::Item(s, i)) = lists.state(LIST).cursor
                && let Some(f) = browse(listing, w).get(s).and_then(|sec| sec.2.get(i)).copied()
            {
                out.write(GunsAct::Card(f));
            }
        }
    }
}

// ---------------------------------------------------------------------
// Drawing.

/// The dialog's sizes (cq; `[layout.guns]` may set any).
struct Sz<'a>(&'a Theme);

impl Sz<'_> {
    fn g(&self, k: &str, d: f32) -> f32 {
        self.0.layout_opt("guns", k).unwrap_or(d)
    }
}

/// What the dialog's drawing reads besides the state.
pub struct DrawCx<'a> {
    pub listing: &'a GunsListing,
    pub now: f64,
    pub motion: chimera_ui::motion::Motion,
    pub narrow: bool,
    /// The minigame's name (the kicker).
    pub minigame: &'a str,
}

/// Spawns the dialog over `root` (module doc).
pub fn spawn(c: &mut Commands, ui: &Ui, root: Entity, picker: &GunsPicker, cx: &DrawCx, lists: &mut VirtualLists) -> Option<Entity> {
    let w = picker.open.as_ref()?;
    let t = ui.theme;
    let s = Sz(t);
    let listing = cx.listing;
    let pv = pool(listing, &w.rules, w.rungs);
    let scrim = c
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
            BackgroundColor(ui.color(Tone::Ink, s.g("scrim", 0.78))),
            bevy::ui::FocusPolicy::Block,
            ZIndex(40),
            chimera_ui::Modal,
            GunsDialog,
            ChildOf(root),
        ))
        .id();
    let fw = ui.boxed(c, scrim, UiStyle { w: Some(100.0), h: Some(56.25), shrink: Some(0.0), ..UiStyle::row() });
    let (side, top, bottom) = (s.g("side", 2.4), s.g("top", 2.4), s.g("bottom", 2.2));
    let (px, pt, pb) = (s.g("pad_x", 1.8), s.g("pad_t", 1.5), s.g("pad_b", 1.3));
    let plate = c
        .spawn((
            UiStyle { gap: Some(s.g("gap", 0.9)), pad: Some([pt, px, pb, px].into()), ..UiStyle::column() }
                .abs([Some(side), Some(top), Some(side), Some(bottom)])
                .bg(Tone::Ink2)
                .outline(Tone::Fg4, 1.0),
            Node::default(),
            BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.6), Val::ZERO, ui.units.val(2.0), Val::ZERO, ui.units.val(4.0)),
            ChildOf(fw),
        ))
        .id();
    c.spawn((
        Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, height: ui.units.val(0.22), ..default() },
        BackgroundColor(ui.color(Tone::Fg, 1.0)),
        ChildOf(plate),
    ));
    let inner = 100.0 - 2.0 * side - 2.0 * px;

    // The head: the title, and the Host page's line as it will read.
    let head = ui.boxed(c, plate, UiStyle { justify: Some(JustifyContent::SpaceBetween), align: Some(AlignItems::FlexEnd), shrink: Some(0.0), ..UiStyle::row() });
    let title = ui.boxed(c, head, UiStyle { gap: Some(0.2), ..UiStyle::column() });
    ui.text(c, title, Role::Mono, 0.5, Tone::Fg3, &format!("Host \u{00b7} {}", cx.minigame));
    ui.text(c, title, Role::DisplayBold, 1.6, Tone::Fg, "Guns");
    let line = picker.summary(&w.rules, w.rungs, None);
    let right = ui.boxed(c, head, UiStyle { gap: Some(0.3), align: Some(AlignItems::FlexEnd), ..UiStyle::column() });
    ui.text(c, right, Role::Mono, 0.46, Tone::Fg3, "On the Host page");
    ui.text(c, right, Role::Mono, 0.56, Tone::Fg2, &line);

    preset_row(c, ui, plate, picker, w);
    strip(c, ui, plate, listing, &pv, inner);

    // The columns.
    let cols = ui.boxed(c, plate, UiStyle { gap: Some(s.g("col_gap", 1.3)), grow: Some(1.0), align: Some(AlignItems::Stretch), ..UiStyle::row() });
    c.entity(cols).insert(Node { min_height: Val::Px(0.0), ..default() });
    let rules_w = if cx.narrow { s.g("rules_w_narrow", 40.0) } else { s.g("rules_w", 38.0) };
    let help_w = if cx.narrow { 0.0 } else { s.g("help_w", 15.0) };
    rules_column(c, ui, cols, picker, w, &pv, cx, rules_w);
    let browser_w = inner - rules_w - help_w - s.g("col_gap", 1.3) * if cx.narrow { 1.0 } else { 2.0 };
    browser(c, ui, cols, w, listing, &pv, lists, browser_w);
    if !cx.narrow {
        help(c, ui, cols, listing, w, help_w);
    }

    // The foot: the precedence, Reset, Cancel, Done.
    let foot = ui.boxed(c, plate, UiStyle { gap: Some(0.8), shrink: Some(0.0), ..UiStyle::row() });
    let keys = ui.boxed(c, foot, UiStyle { gap: Some(1.0), grow: Some(1.0), ..UiStyle::row() });
    for (k, words) in [("Tab", "Area"), ("/", "Search"), ("Space", "Toggle, ban, allow"), ("Ctrl+Z", "Undo")] {
        let g = ui.boxed(c, keys, UiStyle { gap: Some(0.4), ..UiStyle::row() });
        ui.kbd(c, g, k, 0.5, Tone::Fg2);
        ui.text(c, g, Role::Mono, 0.5, Tone::Fg2, words);
    }
    let reset = ui.button(c, foot, Kind::Tertiary, "Reset to preset", None, chimera_ui::controls::Size::Regular, State::NONE);
    c.entity(reset).insert(GunsAct::Reset);
    let cancel = ui.button(c, foot, Kind::Secondary, "Cancel", Some("Esc"), chimera_ui::controls::Size::Regular, State::NONE);
    c.entity(cancel).insert(GunsAct::Cancel);
    let empty = pv.empty();
    let done = ui.button(c, foot, Kind::Primary, "Done", Some("Enter"), chimera_ui::controls::Size::Regular, State { disabled: empty, ..State::NONE });
    c.entity(done).insert(GunsAct::Done);
    Some(scrim)
}

/// The preset dropdown (or Save as's name), what differs and the toast.
fn preset_row(c: &mut Commands, ui: &Ui, plate: Entity, picker: &GunsPicker, w: &Working) {
    let row = ui.boxed(c, plate, UiStyle { gap: Some(1.2), shrink: Some(0.0), ..UiStyle::row() });
    let lab = ui.boxed(c, row, UiStyle { gap: Some(0.5), ..UiStyle::row() });
    ui.text(c, lab, Role::Mono, 0.5, Tone::Fg3, "Preset");
    if let Some((name, caret)) = &w.naming {
        let f = ui.text_field(c, row, name, "Name the preset, Enter saves", Some(caret.cursor(name)), false, 22.0, State::focused(true));
        let _ = f;
    } else {
        let label = preset_label(picker, w);
        let items = menu_items(picker, w);
        let words: Vec<String> = items.iter().map(|i| menu_word(picker, i)).collect();
        let (opts, sel): (Vec<&str>, usize) = match w.menu {
            Some(_) => (words.iter().map(String::as_str).collect(), picker.presets().iter().position(|p| p.name.eq_ignore_ascii_case(&w.base)).unwrap_or(0)),
            None => (vec![label.as_str()], 0),
        };
        let (field, options) = ui.dropdown(c, row, &opts, sel, w.menu, 22.0, State::NONE);
        c.entity(field).insert(GunsAct::Menu);
        for (i, o) in options.into_iter().enumerate() {
            c.entity(o).insert(GunsAct::MenuPick(i));
        }
    }
    let edited = gun_rules::matching(&picker.presets(), &w.rules, w.rungs).is_none();
    let note = if edited { "Changed from the preset: save it, or reset below" } else { "The preset as it is" };
    ui.text(c, row, Role::Mono, 0.5, Tone::Fg3, note);
    ui.boxed(c, row, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    if let Some(t) = w.toast.as_deref() {
        ui.text(c, row, Role::Mono, 0.56, Tone::Fg2, t);
        if w.undo.is_some() {
            let u = ui.button(c, row, Kind::Tertiary, "Undo", Some("Ctrl+Z"), chimera_ui::controls::Size::Small, State::NONE);
            c.entity(u).insert(GunsAct::Undo);
        }
    }
}

/// A stage's cell colour.
fn stage_tone(b: Band) -> (Tone, f32) {
    match b {
        Band::WarmUp => (Tone::Acc, 0.35),
        Band::Rising => (Tone::Acc, 0.55),
        Band::Peak => (Tone::Acc, 0.8),
        Band::Heavy => (Tone::Acc, 1.0),
        Band::Specials => (Tone::Hero, 1.0),
        Band::Grenade => (Tone::Warn, 0.8),
        Band::Melee => (Tone::Fg, 0.9),
    }
}

/// The ladder strip: a cell a rung by stage, the pool tile.
fn strip(c: &mut Commands, ui: &Ui, plate: Entity, listing: &GunsListing, pv: &PoolView, inner: f32) {
    let row = ui.boxed(c, plate, UiStyle { gap: Some(1.3), shrink: Some(0.0), ..UiStyle::row() });
    let tile_w = 21.0;
    let cells_w = inner - tile_w - 1.3;
    let col = ui.boxed(c, row, UiStyle { w: Some(cells_w), gap: Some(0.5), ..UiStyle::column() });
    ui.text(c, col, Role::Mono, 0.48, Tone::Fg3, &format!("Ladder shape \u{00b7} {} rungs \u{00b7} each match draws new guns into these slots", pv.rungs));
    let cells = ui.boxed(c, col, UiStyle { gap: Some(0.15), ..UiStyle::row() });
    let n = pv.rungs.max(1);
    let cw = (cells_w - 0.15 * (n as f32 - 1.0)) / n as f32;
    // The rungs the pool can't fill: the last gun rungs before the knife.
    let melee = pv.stages[Band::Melee.index()];
    let missing = pv.rungs.saturating_sub(pv.plays);
    let mut i = 0;
    for b in Band::ALL {
        for _ in 0..pv.stages[b.index()] {
            let guns = pv.rungs.saturating_sub(melee);
            let lost = i + missing.min(guns) >= guns && i < guns;
            let (tone, a) = if lost { (Tone::Fg, 0.06) } else { stage_tone(b) };
            let cell = ui.boxed(c, cells, UiStyle { w: Some(cw), h: Some(1.5), justify: Some(JustifyContent::Center), shrink: Some(0.0), ..UiStyle::row() });
            c.entity(cell).insert(UiSlant::new(tone, a));
            if lost {
                c.entity(cell).insert(UiSlant::new(Tone::Warn, 0.9).outline(ui.units.phys(ui.units.hairline())));
            }
            let word = if b == Band::Melee { "Knife".to_string() } else { (i + 1).to_string() };
            if cw > 1.2 || b == Band::Melee {
                let ink = if lost { Tone::Warn } else if a > 0.7 { Tone::Ink } else { Tone::Fg };
                ui.text(c, cell, Role::Mono, 0.42, ink, &word);
            }
            i += 1;
        }
    }
    let legend = ui.boxed(c, col, UiStyle { gap: Some(1.0), ..UiStyle::row() });
    for b in Band::ALL {
        let r = pv.stages[b.index()];
        if r == 0 {
            continue;
        }
        let g = ui.boxed(c, legend, UiStyle { gap: Some(0.3), ..UiStyle::row() });
        let (tone, a) = stage_tone(b);
        let m = ui.boxed(c, g, UiStyle { w: Some(0.5), h: Some(0.5), ..UiStyle::row() });
        c.entity(m).insert(UiSlant::new(tone, a));
        let pool = if b == Band::Melee { "knife finale".to_string() } else { format!("{} guns", pv.by_band[b.index()]) };
        let warn = b != Band::Melee && pv.by_band[b.index()] == 0;
        ui.text(c, g, Role::Mono, 0.42, if warn { Tone::Warn } else { Tone::Fg3 }, &format!("{} \u{00b7} {r} \u{00b7} {pool}", stage_name(b)));
    }
    // The pool tile.
    let (edge, chip, chip_tone) = if pv.empty() {
        (Tone::Threat, "Empty", Tone::Threat)
    } else if pv.short() {
        (Tone::Warn, "Short", Tone::Warn)
    } else {
        (Tone::Fg5, "OK", Tone::Good)
    };
    let tile = ui.boxed(c, row, UiStyle { w: Some(tile_w), gap: Some(0.35), pad: Some([0.7, 0.9].into()), shrink: Some(0.0), ..UiStyle::column() }.bg(Tone::Ink).outline(edge, 1.0));
    let top = ui.boxed(c, tile, UiStyle { gap: Some(0.6), align: Some(AlignItems::Baseline), ..UiStyle::row() });
    ui.text(c, top, Role::DisplayBold, 1.2, if pv.empty() { Tone::Threat } else { Tone::Fg }, &format!("{} guns", pv.families));
    if pv.extra_skins > 0 {
        ui.text(c, top, Role::Mono, 0.46, Tone::Fg3, &format!("+{} skins", pv.extra_skins));
    }
    ui.boxed(c, top, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    ui.text(c, top, Role::Mono, 0.5, chip_tone, chip);
    let words = if pv.empty() {
        "No gun is left: Done is off until one is".to_string()
    } else if pv.short() {
        format!("Plays {} of {} rungs: When short decides", pv.plays, pv.rungs)
    } else {
        format!("Fills {} of {} rungs", pv.plays, pv.rungs)
    };
    ui.text(c, tile, Role::Mono, 0.48, if pv.empty() { Tone::Threat } else if pv.short() { Tone::Warn } else { Tone::Fg2 }, &words);
    let mix: Vec<String> = listing.games.iter().zip(&pv.by_game).filter(|(_, n)| **n > 0).map(|((g, _), n)| format!("{} {n}", game_short(g))).collect();
    if !mix.is_empty() {
        ui.text(c, tile, Role::Mono, 0.42, Tone::Fg3, &format!("Game mix even: {}", mix.join(" \u{00b7} ")));
    }
}

/// A rules section's head: the chevron, the title, its summary, a badge.
#[allow(clippy::too_many_arguments)]
fn head(c: &mut Commands, ui: &Ui, parent: Entity, key: &str, title: &str, note: &str, badge: Option<(String, BadgeKind)>, folded: bool, focus: bool, sub: bool) {
    let h = ui.item(c, parent, UiStyle { gap: Some(0.6), pad: Some([0.0, 1.0, 0.0, if sub { 1.6 } else { 0.6 }].into()), h: Some(if sub { 2.0 } else { 2.4 }), shrink: Some(0.0), ..UiStyle::row() }, State::focused(focus), false);
    c.entity(h).insert(GunsAct::Fold(key.to_string()));
    ui.chevron(c, h, Tone::Fg2, !folded);
    if !sub {
        ui.mark(c, h, Tone::Acc, 0.42, 0.7);
    }
    ui.text(c, h, if sub { Role::DisplayRegular } else { Role::DisplayBold }, if sub { 0.7 } else { 0.82 }, Tone::Fg, title);
    ui.text(c, h, Role::Mono, 0.46, Tone::Fg3, note);
    ui.boxed(c, h, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    if let Some((b, kind)) = badge {
        ui.badge(c, h, &b, kind);
    }
}

/// A rule row: the label and note, the control column, the reset key.
#[allow(clippy::too_many_arguments)]
fn rule_row(c: &mut Commands, ui: &Ui, parent: Entity, label: &str, note: &str, st: State, changed: Option<RowKey>, ctl_w: f32) -> Entity {
    // The kit's setting row, its column as wide as this dialog's rules
    // column allows (the form's is wider).
    let look = chimera_ui::form::RowLook { note: Some(note), changed: changed.is_some(), min_h: 2.6, control_w: ctl_w, ..Default::default() };
    let sr = ui.setting_row(c, parent, label, look, st);
    if let (Some(k), Some(r)) = (changed, sr.reset) {
        c.entity(r).insert(GunsAct::ResetRow(k));
    }
    sr.control
}

/// The rules column (module doc).
#[allow(clippy::too_many_arguments)]
fn rules_column(c: &mut Commands, ui: &Ui, cols: Entity, picker: &GunsPicker, w: &Working, pv: &PoolView, cx: &DrawCx, width: f32) {
    let listing = cx.listing;
    let col = ui.boxed(c, cols, UiStyle { w: Some(width), gap: Some(0.4), shrink: Some(0.0), ..UiStyle::column() });
    c.entity(col).insert(Node { min_height: Val::Px(0.0), ..default() });
    ui.text(c, col, Role::Mono, 0.48, Tone::Fg3, "Rules \u{00b7} what a ladder may deal");
    let sc = ui.scroll(c, col, RULES_SCROLL, UiStyle { grow: Some(1.0), ..UiStyle::row() }, UiStyle::column());
    // The frame may shrink below its rows (it scrolls them).
    c.entity(sc.frame).insert(Node { min_height: Val::Px(0.0), flex_grow: 1.0, ..default() });
    let list = sc.content;
    let rows = rule_rows(listing, w);
    let focus_of = |r: RuleRow| w.area == Area::Rules && rows.get(w.cursor) == Some(&r);
    // Toggles and the stepper one width, the segmented choices another.
    let ctl_w = 7.5;
    let seg_w = 11.5;
    let slide = |key: String| {
        w.flip.as_ref().filter(|(k, _)| *k == key).map(|(_, since)| ToggleSlide { since: *since, now: cx.now, motion: cx.motion })
    };
    let changed = |k: RowKey| row_changed(picker, listing, w, k).then_some(k);
    // Games.
    let on = listing.games.iter().filter(|(g, _)| w.rules.game_on(g)).count();
    let names: Vec<String> = listing.games.iter().filter(|(g, _)| w.rules.game_on(g)).map(|(g, _)| game_short(g)).collect();
    let games_note = format!("{on} of {} on \u{00b7} {}", listing.games.len(), names.join(", "));
    let badge = (on < listing.games.len()).then(|| (format!("{on}/{}", listing.games.len()), BadgeKind::Hero));
    head(c, ui, list, "games", "Games", &games_note, badge, w.folded.contains("games"), focus_of(RuleRow::Head("games")), false);
    if !w.folded.contains("games") {
        for (i, (g, name)) in listing.games.iter().enumerate() {
            let fams: Vec<&GunFamily> = listing.families.iter().filter(|f| f.game == *g).collect();
            let dealt = fams.iter().filter(|f| decided(f, &w.rules).dealt()).count();
            let skins: usize = fams.iter().map(|f| f.members.len()).sum();
            let note = if skins > fams.len() { format!("{dealt} of {} in \u{00b7} {skins} skins", fams.len()) } else { format!("{dealt} of {} in", fams.len()) };
            let st = State::focused(focus_of(RuleRow::Game(i)));
            let ctl = rule_row(c, ui, list, name, &note, st, changed(RowKey::Game(i)), ctl_w + 3.6);
            let solo = ui.button(c, ctl, Kind::Tertiary, "Solo", None, chimera_ui::controls::Size::Regular, State::NONE);
            c.entity(solo).insert(GunsAct::Solo(i));
            let tg = ui.toggle_sized(c, ctl, w.rules.game_on(g), State::NONE, slide(format!("game:{g}")), ("Off", "On"), ctl_w, chimera_ui::controls::Size::Regular);
            c.entity(tg).insert(GunsAct::Game(i));
        }
    }
    // Classes, by stage.
    let off = classes().into_iter().filter(|k| !w.rules.class_on(*k)).count();
    let note = if off == 0 { "all on".to_string() } else { format!("{off} off") };
    let badge = (off > 0).then(|| (off.to_string(), BadgeKind::Hero));
    head(c, ui, list, "classes", "Classes", &note, badge, w.folded.contains("classes"), focus_of(RuleRow::Head("classes")), false);
    if !w.folded.contains("classes") {
        for b in STAGES {
            let cs = stage_classes(listing, &w.rules, b);
            if cs.is_empty() {
                continue;
            }
            let rungs = pv.stages[b.index()];
            let guns = pv.by_band[b.index()];
            let note = format!("{rungs} rungs \u{00b7} {guns} guns");
            let badge = (rungs > 0 && guns == 0).then(|| ("!".to_string(), BadgeKind::Hero));
            let key = stage_key(b);
            head(c, ui, list, &key, stage_name(b), &note, badge, w.folded.contains(&key), focus_of(RuleRow::Stage(b)), true);
            if w.folded.contains(&key) {
                continue;
            }
            let group = ui.boxed(c, list, UiStyle { margin: Some([0.0, 0.0, 0.0, 1.4].into()), ..UiStyle::column() });
            for k in cs {
                let n = listing.families.iter().filter(|f| f.class == k && f.game.as_str() != "" && w.rules.game_on(&f.game)).count();
                let st = State::focused(focus_of(RuleRow::Class(k)));
                let ctl = rule_row(c, ui, group, class_name(k), &format!("{n} guns"), st, changed(RowKey::Class(k)), ctl_w);
                let tg = ui.toggle_sized(c, ctl, w.rules.class_on(k), State::NONE, slide(format!("class:{}", k.name())), ("Off", "On"), ctl_w, chimera_ui::controls::Size::Regular);
                c.entity(tg).insert(GunsAct::Class(k));
            }
        }
    }
    // The ladder.
    let short = match w.rules.short {
        WhenShort::Shorter => "shorter",
        WhenShort::Repeat => "repeat",
        WhenShort::Borrow => "borrow",
    };
    let note = format!("{} rungs \u{00b7} skins {} \u{00b7} short: {short}", w.rungs, w.rules.skins);
    head(c, ui, list, "ladder", "Ladder", &note, pv.short().then(|| ("!".to_string(), BadgeKind::Hero)), w.folded.contains("ladder"), focus_of(RuleRow::Head("ladder")), false);
    if !w.folded.contains("ladder") {
        if w.has_rungs {
            let b = rung_bounds();
            let note = if pv.plays < w.rungs { format!("plays {} \u{00b7} the pool fills {}", pv.plays, pv.fills) } else { format!("{} to {} \u{00b7} the pool fills {}", b.min, b.max, pv.fills) };
            let ctl = rule_row(c, ui, list, "Rungs", &note, State::focused(focus_of(RuleRow::Rungs)), changed(RowKey::Rungs), ctl_w);
            let range = StepperRange::new().range(b.min as f32, b.max as f32).step(1.0).reached(w.rungs <= b.min, w.rungs >= b.max);
            let (minus, plus) = ui.stepper(c, ctl, &w.rungs.to_string(), range, State::NONE);
            c.entity(minus).insert(GunsAct::Rungs(-1));
            c.entity(plus).insert(GunsAct::Rungs(1));
        }
        let ctl = rule_row(c, ui, list, "Skins", "skins, editions", State::focused(focus_of(RuleRow::Skins)), changed(RowKey::Skins), seg_w);
        let sel = if w.rules.skins == Skins::One { 0 } else { 1 };
        for (i, e) in ui.segmented_sized(c, ctl, &["One a gun", "Every skin"], Some(sel), &[], State::NONE, Some(seg_w), chimera_ui::controls::Size::Regular).into_iter().enumerate() {
            c.entity(e).insert(GunsAct::Skins(if i == 0 { Skins::One } else { Skins::All }));
        }
        let ctl = rule_row(c, ui, list, "When short", "too few guns", State::focused(focus_of(RuleRow::Short)), changed(RowKey::Short), seg_w);
        let all = [WhenShort::Shorter, WhenShort::Repeat, WhenShort::Borrow];
        let sel = all.iter().position(|s| *s == w.rules.short);
        for (i, e) in ui.segmented_sized(c, ctl, &["Shorter", "Repeat", "Borrow"], sel, &[], State::NONE, Some(seg_w), chimera_ui::controls::Size::Regular).into_iter().enumerate() {
            c.entity(e).insert(GunsAct::Short(all[i]));
        }
    }
}

/// The card grid's columns and each card's width (`VirtualList`'s view).
#[derive(Component, Clone, Copy, Debug)]
pub struct CardsW(pub f32, pub usize);

/// The browser: search, Group by, Show, the cards.
#[allow(clippy::too_many_arguments)]
fn browser(c: &mut Commands, ui: &Ui, cols: Entity, w: &Working, listing: &GunsListing, pv: &PoolView, lists: &mut VirtualLists, width: f32) {
    let col = ui.boxed(c, cols, UiStyle { w: Some(width), gap: Some(0.6), shrink: Some(0.0), ..UiStyle::column() });
    c.entity(col).insert(Node { min_height: Val::Px(0.0), ..default() });
    let caret = w.search_focus.then(|| w.caret.cursor(&w.search));
    let st = State::focused(w.search_focus);
    let sp = ui.search_field(c, col, &w.search, "Find a gun, a skin, a game", caret, width, st);
    c.entity(sp.field).insert(GunsAct::FocusSearch);
    if let Some(x) = sp.clear {
        c.entity(x).insert(GunsAct::ClearSearch);
    }
    let bar = ui.boxed(c, col, UiStyle { gap: Some(0.6), ..UiStyle::row() });
    let lab = ui.boxed(c, bar, UiStyle { w: Some(4.5), shrink: Some(0.0), ..UiStyle::row() });
    ui.text(c, lab, Role::Mono, 0.46, Tone::Fg3, "Group by");
    let groups = [Group::Game, Group::Stage, Group::None];
    let gsel = groups.iter().position(|g| *g == w.group);
    for (i, e) in ui.segmented_sized(c, bar, &["Game", "Stage", "None"], gsel, &[], State::NONE, Some(10.0), chimera_ui::controls::Size::Small).into_iter().enumerate() {
        c.entity(e).insert(GunsAct::Group(groups[i]));
    }
    let bar = ui.boxed(c, col, UiStyle { gap: Some(0.6), ..UiStyle::row() });
    let lab = ui.boxed(c, bar, UiStyle { w: Some(4.5), shrink: Some(0.0), ..UiStyle::row() });
    ui.text(c, lab, Role::Mono, 0.46, Tone::Fg3, "Show");
    let shows = [Show::All, Show::In, Show::Out, Show::Exceptions];
    let ssel = shows.iter().position(|s| *s == w.show);
    let exc = format!("Exceptions {}", pv.exceptions);
    for (i, e) in ui.segmented_sized(c, bar, &["All", "In", "Out", &exc], ssel, &[], State::NONE, Some((width - 5.1).min(24.0)), chimera_ui::controls::Size::Small).into_iter().enumerate() {
        c.entity(e).insert(GunsAct::Show(shows[i]));
    }
    let sections = browse(listing, w);
    let shown: usize = sections.iter().map(|s| s.2.len()).sum();
    ui.text(
        c,
        col,
        Role::Mono,
        0.44,
        Tone::Fg3,
        &format!("{shown} of {} gun families \u{00b7} weakest first \u{00b7} click: in \u{2192} ban, out \u{2192} allowed", listing.families.len()),
    );
    // The grid.
    let gap = 0.45;
    let cols_n = ((width - 1.2) / 9.5).floor().max(1.0) as usize;
    let card_w = (width - 1.2 - gap * (cols_n as f32 - 1.0)) / cols_n as f32;
    let st = lists.state(LIST);
    st.set_shape(VShape::grid(cols_n, 1.9, 4.4, gap));
    st.set_sections(sections.iter().map(|(k, title, items)| if title.is_empty() { VSection::plain(k.clone(), items.len()) } else { VSection::new(k.clone(), items.len()) }).collect());
    st.bump();
    if w.area != Area::Browser {
        st.cursor = None;
    }
    let parts = ui.virtual_list(c, col, LIST, UiStyle { grow: Some(1.0), ..UiStyle::row() }, 34.0, VShape::grid(cols_n, 1.9, 4.4, gap));
    c.entity(parts.scroll.frame).insert(Node { min_height: Val::Px(0.0), flex_grow: 1.0, ..default() });
    c.entity(parts.scroll.view).insert(CardsW(card_w, cols_n));
}

/// Fills the browser's slots: a section's header, or a row of cards.
#[allow(clippy::too_many_arguments)]
pub fn bind_cards(
    theme: Option<Res<Theme>>,
    fonts: Option<Res<chimera_ui::KitFonts>>,
    units: Option<Res<chimera_ui::UiUnits>>,
    picker: Option<Res<GunsPicker>>,
    listing: Option<Res<GunsListing>>,
    lists: Res<VirtualLists>,
    list_q: Query<(&VirtualList, Option<&CardsW>)>,
    slots: Query<(Entity, &VSlot), Changed<VSlot>>,
    mut c: Commands,
) {
    let (Some(theme), Some(units), Some(picker), Some(listing)) = (theme, units, picker, listing) else { return };
    let Some(w) = picker.open.as_ref() else { return };
    if slots.is_empty() {
        return;
    }
    let ui = Ui::new(&theme, fonts.as_deref(), *units);
    let Some(st) = lists.get(LIST) else { return };
    let sections = browse(&listing, w);
    for (slot, s) in &slots {
        let Ok((list, cw)) = list_q.get(s.list) else { continue };
        if list.key != LIST {
            continue;
        }
        c.entity(slot).despawn_children();
        let Some(line) = s.line.and_then(|l| st.layout.lines.get(l)).copied() else { continue };
        let (card_w, _) = cw.map_or((9.0, 4), |c| (c.0, c.1));
        match line {
            chimera_ui::vlist::VLine::Header(sec) => {
                let Some((_, title, items)) = sections.get(sec) else { continue };
                let dealt = items.iter().filter(|i| decided(&listing.families[**i], &w.rules).dealt()).count();
                let h = ui.item(&mut c, slot, UiStyle { gap: Some(0.6), pad: Some([0.0, 0.6].into()), grow: Some(1.0), ..UiStyle::row() }, State::NONE, false);
                c.entity(h).insert(VHit { slot, col: None });
                ui.chevron(&mut c, h, Tone::Fg2, !st.is_folded(sec));
                ui.text(&mut c, h, Role::DisplayBold, 0.72, Tone::Fg, title);
                ui.text(&mut c, h, Role::Mono, 0.46, Tone::Fg3, &format!("{dealt} of {} in", items.len()));
            }
            chimera_ui::vlist::VLine::Row { section, first, count } => {
                let Some((_, _, items)) = sections.get(section) else { continue };
                let r = ui.boxed(&mut c, slot, UiStyle { gap: Some(0.45), grow: Some(1.0), ..UiStyle::row() });
                for k in 0..count {
                    let Some(&fi) = items.get(first + k) else { continue };
                    let f = &listing.families[fi];
                    let cursor = st.cursor == Some(VCursor::Item(section, first + k));
                    card(&mut c, &ui, r, f, fi, &w.rules, card_w, cursor);
                }
            }
        }
    }
}

/// A family card.
#[allow(clippy::too_many_arguments)]
fn card(c: &mut Commands, ui: &Ui, parent: Entity, f: &GunFamily, index: usize, rules: &GunRules, w: f32, cursor: bool) {
    let d = decided(f, rules);
    let (edge, ea, word, word_tone, ink) = match d {
        Decided::In => (Tone::Fg5, 1.0, "In", Tone::Fg2, Tone::Fg),
        Decided::Out => (Tone::Fg5, 0.5, "Out", Tone::Fg4, Tone::Fg3),
        Decided::Banned => (Tone::Threat, 0.9, "Banned", Tone::Threat, Tone::Fg3),
        Decided::Allowed => (Tone::Good, 0.9, "Allowed", Tone::Good, Tone::Fg),
    };
    let style = UiStyle { w: Some(w), h: Some(4.4), gap: Some(0.2), pad: Some([0.45, 0.55].into()), shrink: Some(0.0), justify: Some(JustifyContent::SpaceBetween), ..UiStyle::column() }
        .outline(if cursor { Tone::Acc } else { edge }, ea)
        .bg_a(Tone::Fg, if cursor { 0.1 } else if d.dealt() { 0.05 } else { 0.015 });
    let card = ui.item(c, parent, style, State::NONE, false);
    c.entity(card).insert((GunsAct::Card(index), GunCard(index)));
    let top = ui.boxed(c, card, UiStyle { justify: Some(JustifyContent::SpaceBetween), ..UiStyle::row() });
    ui.text(c, top, Role::Mono, 0.4, Tone::Fg3, class_name(f.class));
    ui.text(c, top, Role::Mono, 0.42, word_tone, word);
    let name = chimera_ui::ellipsize(&f.name, (w - 1.1).max(2.0), |t| t.chars().count() as f32 * 0.42);
    ui.text(c, card, Role::DisplayBold, 0.62, ink, &name);
    let foot = ui.boxed(c, card, UiStyle { justify: Some(JustifyContent::SpaceBetween), ..UiStyle::row() });
    let curve = PowerCurve::shipped();
    let stage = if curve.class_band(f.class) != f.band { format!("plays as {}", stage_name(f.band).to_lowercase()) } else { format!("{} stage", stage_name(f.band).to_lowercase()) };
    ui.text(c, foot, Role::Mono, 0.38, Tone::Fg3, &stage);
    if f.members.len() > 1 {
        ui.badge(c, foot, &f.members.len().to_string(), BadgeKind::Quiet);
    }
}

/// The help tile: the focused row's help, which wins.
fn help(c: &mut Commands, ui: &Ui, cols: Entity, listing: &GunsListing, w: &Working, width: f32) {
    let col = ui.boxed(c, cols, UiStyle { w: Some(width), gap: Some(0.9), shrink: Some(0.0), ..UiStyle::column() });
    let (title, text) = match (w.area, rule_rows(listing, w).get(w.cursor)) {
        (Area::Browser, _) => ("Browser", "Single guns are exceptions. A click on a gun the rules let in bans it, with its skins; on one they leave out allows it. Again puts it back to the rules."),
        (_, Some(RuleRow::Head("games") | RuleRow::Game(_))) => ("Games", "Which games lend guns. A game that appears later is on. Solo keeps one game alone; again turns every game on."),
        (_, Some(RuleRow::Head("classes") | RuleRow::Stage(_) | RuleRow::Class(_))) => ("Classes", "The kinds of gun a ladder may deal, by the stage they play in. A pistol that hits like a heavy pistol counts as one."),
        (_, Some(RuleRow::Rungs)) => ("Rungs", "Rungs in all, the knife finale's too, 5 to 40. The note says how many the pool fills without When short."),
        (_, Some(RuleRow::Skins)) => ("Skins", "One a gun: a gun's skins and editions are one candidate, its skin drawn at the deal. Every skin: each its own."),
        (_, Some(RuleRow::Short)) => ("When short", "Shorter plays fewer rungs. Repeat deals the stages' guns again, spread out. Borrow takes classes turned off, from games that are on."),
        _ => ("Guns", "Point at a row or a card for its help. Rules decide most of the ladder; single guns are exceptions in the browser."),
    };
    let tile = ui.boxed(c, col, UiStyle { gap: Some(0.4), pad: Some([0.8, 0.9].into()), ..UiStyle::column() }.bg(Tone::Ink).outline(Tone::Fg5, 1.0));
    ui.text(c, tile, Role::Mono, 0.44, Tone::Fg3, "Rules");
    ui.text(c, tile, Role::DisplayBold, 0.8, Tone::Fg, title);
    ui.paragraph(c, tile, Role::DisplayRegular, 0.54, Tone::Fg2, text);
    let wins = ui.boxed(c, col, UiStyle { gap: Some(0.35), pad: Some([0.8, 0.9].into()), ..UiStyle::column() }.bg(Tone::Ink).outline(Tone::Fg5, 1.0));
    ui.text(c, wins, Role::Mono, 0.44, Tone::Fg3, "Which wins, first to last");
    for (i, (k, words)) in [
        ("Unavailable", "data excludes it, or a player lacks its game"),
        ("Ban", "you ruled it out; beats Allowed"),
        ("Allowed", "you let it in against the rules"),
        ("Rules", "games and classes decide the rest"),
    ]
    .iter()
    .enumerate()
    {
        let r = ui.boxed(c, wins, UiStyle { gap: Some(0.5), ..UiStyle::row() });
        ui.text(c, r, Role::Mono, 0.44, Tone::Fg3, &(i + 1).to_string());
        ui.text(c, r, Role::DisplayBold, 0.5, Tone::Fg, k);
        ui.paragraph(c, wins, Role::Mono, 0.4, Tone::Fg3, words);
    }
}

/// The Host page's Guns field (step 4): the preset dropdown, its last
/// entry the dialog.
pub fn page_field(c: &mut Commands, ui: &Ui, vl: Entity, picker: &GunsPicker, rules: &GunRules, rungs: usize, st: State) {
    let all = picker.presets();
    let label = gun_rules::matching(&all, rules, rungs).map_or("Custom".to_string(), |p| p.name.clone());
    let mut words: Vec<String> = all.iter().map(|p| p.name.clone()).collect();
    words.push("Edit the rules".into());
    let (opts, sel): (Vec<&str>, usize) = match picker.page_menu {
        Some(_) => (words.iter().map(String::as_str).collect(), all.iter().position(|p| p.name == label).unwrap_or(0)),
        None => (vec![label.as_str()], 0),
    };
    let (field, options) = ui.dropdown(c, vl, &opts, sel, picker.page_menu, ui.form().control_w, st);
    c.entity(field).insert(GunsAct::PageMenu);
    for (i, o) in options.into_iter().enumerate() {
        c.entity(o).insert(GunsAct::PagePick(i));
    }
}

/// `ui_scene play guns [default|collapsed|classoff|bans|short|menu]`:
/// the dialog open in that state (module doc).
pub fn stage(world: &mut World, state: &str) {
    open(world);
    let listing = world.resource::<GunsListing>().clone();
    let Some(mut w) = world.resource::<GunsPicker>().open.clone() else { return };
    match state {
        "collapsed" => {
            w.folded.insert("games".into());
            if let Some((g, _)) = listing.games.first() {
                let mut lists = world.resource_mut::<VirtualLists>();
                let st = lists.state(LIST);
                st.folded.insert(format!("game:{g}"));
            }
        }
        "classoff" => {
            w.rules.set_class_on(GunClass::Sniper, false);
            w.rules.set_class_on(GunClass::Launcher, false);
            w.rules.set_class_on(GunClass::Flamethrower, false);
            w.cursor = rule_rows(&listing, &w).iter().position(|r| *r == RuleRow::Class(GunClass::Sniper)).unwrap_or(0);
        }
        "bans" => {
            let ins: Vec<&GunFamily> = listing.families.iter().filter(|f| by_rules(f, &w.rules) && f.class != GunClass::Grenade).collect();
            for f in ins.iter().step_by(7).take(4) {
                w.rules.cycle_family(&f.id, true);
            }
            w.rules.set_class_on(GunClass::Launcher, false);
            if let Some(f) = listing.families.iter().find(|f| f.class == GunClass::Launcher) {
                w.rules.cycle_family(&f.id, false);
            }
            w.show = Show::Exceptions;
            w.area = Area::Browser;
        }
        "short" => {
            // The game with the fewest guns alone, a long ladder.
            let fewest = listing.games.iter().min_by_key(|(g, _)| listing.families.iter().filter(|f| f.game == *g).count()).map(|(g, _)| g.clone());
            if let Some(g) = fewest {
                w.rules.solo(&g, &listing.game_ids());
            }
            w.rungs = 21;
        }
        "menu" => w.menu = Some(1),
        _ => {}
    }
    world.resource_mut::<GunsPicker>().open = Some(w);
}

/// The dialog's systems (the Host page's plugin adds them in its chain).
pub fn plugin(app: &mut App) {
    app.init_resource::<GunsPicker>()
        .init_resource::<GunsSource>()
        .init_resource::<GunsListing>()
        .init_resource::<GunsPresetDir>()
        .add_message::<GunsAct>()
        .add_systems(Startup, load_saved)
        .add_systems(PostUpdate, bind_cards.after(chimera_ui::vlist::place_slots).before(bevy::ui::UiSystems::Prepare));
}
