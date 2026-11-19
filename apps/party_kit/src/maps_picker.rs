//! The Host page's Maps picker (the kit's design rules, sections 13, 15
//! and 16; docs/plans/active/ugg-ui.md "Signal revision 8", stage 4): the
//! Maps field (`controls::Ui::dialog_field`, the list-with-chevron icon)
//! opens a dialog over the page, behind an exclusive scrim: every game,
//! under it its groups (Official maps, Custom maps), under them the maps,
//! each with a slanted tri-state box in the progress component's language
//! (filled on, half filled when only some under it are on, a dim outline
//! off). Toggling a game or a group sets everything under it.
//!
//! Arena slices (docs/plans/active/arena-slices.md, A2): the listing's
//! pool has each map's slices after it (`game:map@id`,
//! `chimera_sim::minigame::arena::ArenaSlices::after_maps`), shown as the
//! map's children (a row one step further in, the slice's own display
//! name, `name_slices`; the same row and box as a map's). A map's row sets
//! it and its slices (off: the map named in `mp_maps_off`, its slices off
//! with it); a slice toggles alone, its map as it was: off while its map
//! is on (named), or on while its map is off (marked `+game:map@id`,
//! `ON_MARK`; the map's box then shows some on). The counts are of every
//! entry, maps and slices alike.
//!
//! - **Data** (`MapCatalogue`): where the listing comes from. Today
//!   `LocalCatalogue`: the minigame's pool (the installed and known maps),
//!   all of it Official. **The seam**: the asset service's listing API (by
//!   game, official or custom, other filters) will be another
//!   `MapCatalogue`, put in `MapsSource` in this one's place.
//! - **State** (`MapsPicker`): the maps off (`off`, what Start sends as
//!   `mp_maps_off`, the server's rotation and vote leaving them out:
//!   `chimera_net::minigame_net::MapsOff`), and while open a working copy,
//!   the cursor and the folded games and groups.
//! - **Keys**: ↑↓ the row, Space on or off, ←→ fold or open (← on a map: its group folds, the cursor on its header), Enter Done (the
//!   dialog's hero), Esc or a right click Cancel; the pointer: a row's box
//!   (with a little slop around it) toggles it; on a game's or group's
//!   header the rest of the row (chevron, name, count) folds it, on a map
//!   the whole row toggles. Hover lifts just the area a click would act on.
//!
//! The tree is `chimera_ui::vlist` (recycling, smooth wheel scroll, the
//! keys keep the cursor in view; `bind_rows` fills its slots). ← on a map
//! folds its group and moves the cursor to the group's header.
//!
//! Known shortcuts: no Custom maps yet (the local listing has none); all
//! rows are one height.

use std::collections::BTreeSet;

use bevy::prelude::*;
use chimera_ui::{
    Role, Tone, UiSlant, UiStyle, VCursor, VSection, VShape, VSlot, VirtualLists,
    controls::{HERO_H, Kind, State, Ui},
    vlist::VLine,
};

/// One game's maps, grouped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogueGame {
    /// The id maps are named under (`cs_source`).
    pub id: String,
    pub name: String,
    pub groups: Vec<CatalogueGroup>,
    /// Shown names by entry where not the id's (an arena slice's own,
    /// "West Base": `name_slices`).
    pub names: std::collections::BTreeMap<String, String>,
}

/// A group of a game's maps (Official maps, Custom maps).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogueGroup {
    pub name: String,
    /// `game:map`.
    pub maps: Vec<String>,
}

/// Where the picker's listing comes from (module doc: the seam for the
/// asset service's listing).
pub trait MapCatalogue: Send + Sync {
    /// The games and their maps for `pool` (the minigame's own maps).
    fn listing(&self, pool: &[String]) -> Vec<CatalogueGame>;
}

/// The listing from the installed and known maps: the minigame's pool, by
/// game, all of it Official.
pub struct LocalCatalogue;

impl MapCatalogue for LocalCatalogue {
    fn listing(&self, pool: &[String]) -> Vec<CatalogueGame> {
        let mut out: Vec<CatalogueGame> = Vec::new();
        for m in pool {
            let (game, _) = m.split_once(':').unwrap_or(("", m));
            let i = match out.iter().position(|g| g.id == game) {
                Some(i) => i,
                None => {
                    out.push(CatalogueGame {
                        id: game.to_string(),
                        name: game_name(game),
                        groups: vec![CatalogueGroup { name: "Official maps".into(), maps: Vec::new() }],
                        names: Default::default(),
                    });
                    out.len() - 1
                }
            };
            out[i].groups[0].maps.push(m.clone());
        }
        out
    }
}

/// The arena slices of `games` named by their data (`SliceDef::name`,
/// "West Base"), for their rows.
pub fn name_slices(games: &mut [CatalogueGame], slices: Option<&chimera_sim::minigame::arena::ArenaSlices>) {
    let Some(slices) = slices else { return };
    for g in games {
        let entries: Vec<String> = g.groups.iter().flat_map(|k| k.maps.iter()).filter(|m| m.contains('@')).cloned().collect();
        for e in entries {
            if let Ok(d) = slices.find(&e) {
                g.names.insert(e, d.name.clone());
            }
        }
    }
}

/// A game's full name by its id.
pub fn game_name(id: &str) -> String {
    match id {
        "cs_source" => "Counter-Strike: Source".into(),
        "combat_arms" => "Combat Arms".into(),
        "hl2dm" => "Half-Life 2: Deathmatch".into(),
        "halo_ce" => "Halo: Combat Evolved".into(),
        "" => "Other".into(),
        other => other.replace('_', " "),
    }
}

/// The catalogue in use (`LocalCatalogue` until the asset service's).
#[derive(Resource)]
pub struct MapsSource(pub Box<dyn MapCatalogue>);

impl Default for MapsSource {
    fn default() -> Self {
        MapsSource(Box::new(LocalCatalogue))
    }
}

/// The picker's state (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MapsPicker {
    /// The maps off, as Start sends them.
    pub off: BTreeSet<String>,
    /// Open: the working copy, Done keeps it.
    pub open: Option<BTreeSet<String>>,
    /// The cursor's row among the shown ones.
    pub cursor: usize,
    /// Folded games (`game`) and groups (`game/group`).
    pub folded: BTreeSet<String>,
    /// The picker had the keys this frame (the page's own keys leave them).
    pub took_keys: bool,
}

/// A row of the tree as shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeRow {
    Game(usize),
    Group(usize, usize),
    Map(usize, usize, usize),
}

/// A box's state: on, some on, off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tri {
    On,
    Mixed,
    Off,
}

/// An arena slice's map (`game:map@id`: `game:map`); None for a map.
pub fn slice_parent(entry: &str) -> Option<&str> {
    entry.split_once('@').map(|(m, _)| m)
}

/// The mark of an arena slice kept on under a map that is off
/// (`+game:map@id` in the set and in `mp_maps_off`).
pub const ON_MARK: char = '+';

/// Whether `entry` is off in `off` (the stored form, `mp_maps_off`'s): a
/// slice marked on (`+map@id`) is on; else an entry named is off, and a
/// slice whose map is named is off with it.
pub fn is_off(off: &BTreeSet<String>, entry: &str) -> bool {
    if off.contains(&format!("{ON_MARK}{entry}")) {
        return false;
    }
    off.contains(entry) || slice_parent(entry).is_some_and(|p| off.contains(p))
}

/// Sets `entry` on or off in `off`, kept in the stored form: a map off
/// takes its slices with it (their own entries and marks go); a slice
/// under a map that is off is on by its mark, else off by its name.
fn set(off: &mut BTreeSet<String>, games: &[CatalogueGame], entry: &str, on: bool) {
    let mark = format!("{ON_MARK}{entry}");
    match slice_parent(entry) {
        None => {
            let slices: Vec<String> = games
                .iter()
                .flat_map(|g| g.groups.iter().flat_map(|k| k.maps.iter()))
                .filter(|s| slice_parent(s) == Some(entry))
                .cloned()
                .collect();
            for s in slices {
                off.remove(&format!("{ON_MARK}{s}"));
                off.remove(&s);
            }
            if on {
                off.remove(entry);
            } else {
                off.insert(entry.to_string());
            }
        }
        Some(map) => {
            off.remove(&mark);
            off.remove(entry);
            match (on, off.contains(map)) {
                (true, true) => {
                    off.insert(mark);
                }
                (false, false) => {
                    off.insert(entry.to_string());
                }
                _ => {}
            }
        }
    }
}

fn tri(maps: &[&String], off: &BTreeSet<String>) -> Tri {
    let on = maps.iter().filter(|m| !is_off(off, m)).count();
    match on {
        0 => Tri::Off,
        n if n == maps.len() => Tri::On,
        _ => Tri::Mixed,
    }
}

impl MapsPicker {
    /// The rows shown (folded games and groups hide what is under them).
    pub fn rows(&self, games: &[CatalogueGame]) -> Vec<TreeRow> {
        let mut out = Vec::new();
        for (g, game) in games.iter().enumerate() {
            out.push(TreeRow::Game(g));
            if self.folded.contains(&game.id) {
                continue;
            }
            for (k, group) in game.groups.iter().enumerate() {
                if group.maps.is_empty() {
                    continue;
                }
                out.push(TreeRow::Group(g, k));
                if self.folded.contains(&format!("{}/{}", game.id, group.name)) {
                    continue;
                }
                for m in 0..group.maps.len() {
                    out.push(TreeRow::Map(g, k, m));
                }
            }
        }
        out
    }

    /// The maps under a row (a map's own row: it and its arena slices,
    /// listed after it as its children).
    pub fn maps_of<'a>(games: &'a [CatalogueGame], row: &TreeRow) -> Vec<&'a String> {
        match *row {
            TreeRow::Game(g) => games[g].groups.iter().flat_map(|k| k.maps.iter()).collect(),
            TreeRow::Group(g, k) => games[g].groups[k].maps.iter().collect(),
            TreeRow::Map(g, k, m) => {
                let maps = &games[g].groups[k].maps;
                let me = &maps[m];
                let mut out = vec![me];
                if slice_parent(me).is_none() {
                    out.extend(maps.iter().filter(|s| slice_parent(s) == Some(me.as_str())));
                }
                out
            }
        }
    }

    /// A row's box in the working copy (else the kept one).
    pub fn state(&self, games: &[CatalogueGame], row: &TreeRow) -> Tri {
        tri(&Self::maps_of(games, row), self.open.as_ref().unwrap_or(&self.off))
    }

    /// Toggles a row: all under it on, unless all are on (then off). A map
    /// takes its slices with it (on or off); a slice toggles alone, its map
    /// as it was (a slice on under a map that is off: the map's box shows
    /// some on).
    pub fn toggle(&mut self, games: &[CatalogueGame], row: &TreeRow) {
        let maps: Vec<String> = Self::maps_of(games, row).into_iter().cloned().collect();
        let on = self.state(games, row) != Tri::On;
        let off = self.open.get_or_insert_with(|| self.off.clone());
        for m in &maps {
            // A slice whose map is toggled with it goes with the map.
            if slice_parent(m).is_some_and(|p| maps.iter().any(|x| x == p)) {
                continue;
            }
            set(off, games, m, on);
        }
    }

    /// Whether a game or group row is folded.
    pub fn is_folded(&self, games: &[CatalogueGame], row: &TreeRow) -> bool {
        match *row {
            TreeRow::Game(g) => self.folded.contains(&games[g].id),
            TreeRow::Group(g, k) => self.folded.contains(&format!("{}/{}", games[g].id, games[g].groups[k].name)),
            TreeRow::Map(..) => false,
        }
    }

    /// Folds or opens a game or a group row (`open`: show what is under).
    pub fn fold(&mut self, games: &[CatalogueGame], row: &TreeRow, open: bool) {
        let key = match *row {
            TreeRow::Game(g) => games[g].id.clone(),
            TreeRow::Group(g, k) => format!("{}/{}", games[g].id, games[g].groups[k].name),
            TreeRow::Map(..) => return,
        };
        if open {
            self.folded.remove(&key);
        } else {
            self.folded.insert(key);
        }
    }

    /// ←: on a map, folds the group it is in and puts the cursor on that
    /// group's header; on an open game or group, folds it; on a folded
    /// group, goes up to its game's header. True when it did something.
    pub fn left(&mut self, games: &[CatalogueGame]) -> bool {
        let rows = self.rows(games);
        let Some(row) = rows.get(self.cursor).cloned() else { return false };
        let to = match row {
            TreeRow::Map(g, k, _) => {
                self.fold(games, &TreeRow::Group(g, k), false);
                TreeRow::Group(g, k)
            }
            TreeRow::Group(g, k) if self.is_folded(games, &row) => TreeRow::Game(g),
            _ if !self.is_folded(games, &row) => {
                self.fold(games, &row, false);
                row
            }
            _ => return false,
        };
        match self.rows(games).iter().position(|r| *r == to) {
            Some(i) if i != self.cursor || rows.len() != self.rows(games).len() => {
                self.cursor = i;
                true
            }
            _ => false,
        }
    }

    /// →: opens a folded game or group; false on anything else.
    pub fn right(&mut self, games: &[CatalogueGame]) -> bool {
        let rows = self.rows(games);
        match rows.get(self.cursor) {
            Some(row) if !matches!(row, TreeRow::Map(..)) && self.is_folded(games, row) => {
                let row = row.clone();
                self.fold(games, &row, true);
                true
            }
            _ => false,
        }
    }

    /// How many entries are on (maps and arena slices alike: what the draw
    /// can take), of how many, in how many games (the kept set, or the
    /// working copy while open).
    pub fn counts(&self, games: &[CatalogueGame]) -> (usize, usize, usize) {
        let off = self.open.as_ref().unwrap_or(&self.off);
        let all: Vec<&String> = games.iter().flat_map(|g| g.groups.iter().flat_map(|k| k.maps.iter())).collect();
        let on = all.iter().filter(|m| !is_off(off, m)).count();
        let games_on = games
            .iter()
            .filter(|g| g.groups.iter().flat_map(|k| k.maps.iter()).any(|m| !is_off(off, m)))
            .count();
        (on, all.len(), games_on)
    }

    /// The picker's set from `mp_maps_off`'s value (`off_line`'s inverse).
    pub fn off_from_line(line: &str) -> BTreeSet<String> {
        line.split(',').map(str::trim).filter(|m| !m.is_empty()).map(str::to_string).collect()
    }

    /// The Maps field's words: "41 of 138 · 3 games".
    pub fn summary(&self, games: &[CatalogueGame]) -> String {
        let (on, all, g) = self.counts(games);
        format!("{on} of {all} \u{00b7} {g} {}", if g == 1 { "game" } else { "games" })
    }

    /// `mp_maps_off`'s value: the entries off, comma-separated; a map off
    /// takes its slices (`chimera_net::minigame_net::MapsOff::has`), and a
    /// slice kept on under it is `+game:map@id`.
    pub fn off_line(&self) -> String {
        self.off.iter().cloned().collect::<Vec<_>>().join(",")
    }
}

/// What a click (or a key) in the dialog does.
#[derive(Component, Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerAction {
    /// A row's box (and its hit slop): on or off.
    Toggle(usize),
    /// A game's or group's header (the chevron, the name, the rest of the
    /// row): fold or open.
    Fold(usize),
    Done,
    Cancel,
    AllOff,
}

/// A header's box with its hit slop (tests find it).
#[derive(Component, Clone, Copy, Debug)]
pub struct TriHit;

/// The dialog's root (tests find it).
#[derive(Component, Clone, Copy, Debug)]
pub struct MapsDialog;

/// A tree row's tri-state box (its state, for tests).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriBox(pub Tri);

/// The tree's list key (`VirtualLists`, `ScrollViews`).
pub const LIST: &str = "maps_picker";

/// A tree row's height, cq (all rows alike: the list recycles by line).
pub const ROW_H: f32 = 1.95;

/// What the dialog shows, as built: the listing and the rows of the tree
/// (`bind_rows` fills the list's slots from it).
#[derive(Resource, Clone, Debug, Default)]
pub struct PickerRows {
    pub games: Vec<CatalogueGame>,
    pub rows: Vec<TreeRow>,
}

/// Spawns the dialog over `root` (a full-screen node): the scrim, the
/// plate (design rules 13: 26 cq in from each side, the window's top and
/// foot), the title and counts, the legend, the tree, the keys and the
/// actions with Done the hero.
pub fn spawn(
    c: &mut Commands,
    ui: &Ui,
    root: Entity,
    picker: &MapsPicker,
    games: &[CatalogueGame],
    lists: &mut VirtualLists,
) -> Entity {
    let sh = chimera_ui::shell::ShellLayout::of(ui.theme);
    let scrim = c
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
            BackgroundColor(ui.color(Tone::Ink, 0.72)),
            bevy::ui::FocusPolicy::Block,
            ZIndex(40),
            MapsDialog,
            ChildOf(root),
        ))
        .id();
    let frame = ui.boxed(c, scrim, UiStyle { w: Some(100.0), h: Some(56.25), shrink: Some(0.0), ..UiStyle::row() });
    let plate = c
        .spawn((
            UiStyle { gap: Some(0.8), pad: Some([1.6, 1.8, 1.4, 1.8].into()), ..UiStyle::column() }
                .abs([Some(26.0), Some(sh.win_top), Some(26.0), Some(sh.win_bottom)])
                .bg(Tone::Ink2)
                .outline(Tone::Fg4, 1.0),
            Node::default(),
            BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.6), Val::ZERO, ui.units.val(2.0), Val::ZERO, ui.units.val(4.0)),
            ChildOf(frame),
        ))
        .id();
    c.spawn((
        Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, height: ui.units.val(0.22), ..default() },
        BackgroundColor(ui.color(Tone::Fg, 1.0)),
        ChildOf(plate),
    ));
    let (on, all, _) = picker.counts(games);
    let head = ui.boxed(c, plate, UiStyle { gap: Some(1.2), align: Some(AlignItems::Baseline), ..UiStyle::row() });
    ui.text(c, head, Role::DisplayBold, 1.6, Tone::Fg, "Maps");
    ui.text(c, head, Role::Mono, 0.56, Tone::Fg2, &format!("{on} of {all} on"));
    // The legend.
    let legend = ui.boxed(c, plate, UiStyle { gap: Some(0.5), ..UiStyle::row() });
    for (t, words) in [(Tri::On, "On"), (Tri::Mixed, "Some on"), (Tri::Off, "Off")] {
        tri_box(c, ui, legend, t);
        let w = ui.boxed(c, legend, UiStyle { margin: Some([0.0, 0.8, 0.0, 0.0].into()), ..UiStyle::row() });
        ui.text(c, w, Role::Mono, 0.56, Tone::Fg2, words);
    }
    // The tree: the kit's recycling list, one line a row (`bind_rows`
    // fills its slots), scrolled by the wheel and kept on the cursor by
    // the keys.
    let rows = picker.rows(games);
    {
        let st = lists.state(LIST);
        st.set_shape(VShape::list(ROW_H));
        st.set_sections(vec![VSection::plain("maps", rows.len())]);
        let at = if rows.is_empty() { None } else { Some(VCursor::Item(0, picker.cursor.min(rows.len() - 1))) };
        if st.cursor != at {
            st.cursor = at;
            st.reveal = true;
        }
        st.bump();
    }
    let tree = ui.virtual_list(c, plate, LIST, UiStyle { grow: Some(1.0), ..UiStyle::row() }, 40.0, VShape::list(ROW_H));
    c.entity(tree.scroll.frame).insert(Node { min_height: Val::Px(0.0), ..default() });
    c.insert_resource(PickerRows { games: games.to_vec(), rows });
    // The keys, then the actions: All off at the left, Cancel, Done.
    let keys = ui.boxed(c, plate, UiStyle { gap: Some(1.4), ..UiStyle::row() });
    for (k, w) in [("Space", "On / off"), ("\u{2190}\u{2192}", "Fold / open"), ("\u{2191}\u{2193}", "Row")] {
        let g = ui.boxed(c, keys, UiStyle { gap: Some(0.45), ..UiStyle::row() });
        ui.kbd(c, g, k, 0.54, Tone::Fg2);
        ui.text(c, g, Role::Mono, 0.54, Tone::Fg2, w);
    }
    let acts = ui.boxed(c, plate, UiStyle { gap: Some(0.7), ..UiStyle::row() });
    let all_off = ui.button(c, acts, Kind::Ghost, "All off", None, 2.0, State::NONE);
    c.entity(all_off).insert(PickerAction::AllOff);
    ui.boxed(c, acts, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    let cancel = ui.button(c, acts, Kind::Secondary, "Cancel", Some("Esc"), 2.4, State::NONE);
    c.entity(cancel).insert(PickerAction::Cancel);
    let done = ui.button(c, acts, Kind::Primary, "Done", Some("Enter"), HERO_H, State::NONE);
    c.entity(done).insert(PickerAction::Done);
    scrim
}

/// Fills the tree's slots (`spawn`): a slot's line is a row of the tree;
/// rebound only when another row scrolls into it.
#[allow(clippy::too_many_arguments)]
pub fn bind_rows(
    theme: Res<chimera_ui::Theme>,
    fonts: Option<Res<chimera_ui::KitFonts>>,
    units: Res<chimera_ui::UiUnits>,
    picker: Res<MapsPicker>,
    built: Res<PickerRows>,
    lists: Res<VirtualLists>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut icons: Option<ResMut<chimera_ui::IconImages>>,
    slots: Query<(Entity, &VSlot), Changed<VSlot>>,
    mut c: Commands,
) {
    let Some(st) = lists.get(LIST) else { return };
    let ui = Ui::new(&theme, fonts.as_deref(), *units);
    let (games, rows) = (&built.games, &built.rows);
    for (slot, s) in &slots {
        c.entity(slot).despawn_children();
        let Some(VLine::Row { first, .. }) = s.line.and_then(|l| st.layout.lines.get(l)).copied() else { continue };
        let Some(row) = rows.get(first) else { continue };
        let res = match (images.as_deref_mut(), icons.as_deref_mut()) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        };
        tree_row(&mut c, &ui, slot, picker.as_ref(), games, first, row, res);
    }
}

/// One tree row in `slot`.
#[allow(clippy::too_many_arguments)]
fn tree_row(
    c: &mut Commands,
    ui: &Ui,
    slot: Entity,
    picker: &MapsPicker,
    games: &[CatalogueGame],
    i: usize,
    row: &TreeRow,
    mut res: Option<(&mut Assets<Image>, &mut chimera_ui::IconImages)>,
) {
    let focus = i == picker.cursor;
    // A game's or group's count: its entries, maps and slices alike.
    let count_of = |row: &TreeRow| {
        let maps = MapsPicker::maps_of(games, row);
        let off = picker.open.as_ref().unwrap_or(&picker.off);
        Some(format!("{} of {}", maps.iter().filter(|m| !is_off(off, m)).count(), maps.len()))
    };
    let (depth, name, count) = match *row {
        TreeRow::Game(g) => (0, games[g].name.clone(), count_of(row)),
        TreeRow::Group(g, k) => (1, games[g].groups[k].name.clone(), count_of(row)),
        // An arena slice: a child of its map, one step in, by its display
        // name (`CatalogueGame::names`), else its id.
        TreeRow::Map(g, k, m) => match games[g].groups[k].maps[m].split_once('@') {
            Some((_, id)) => (3, games[g].names.get(&games[g].groups[k].maps[m]).cloned().unwrap_or_else(|| id.to_string()), None),
            None => (2, games[g].groups[k].maps[m].rsplit(':').next().unwrap_or("").to_string(), None),
        },
    };
    let pad_l = [0.6, 2.0, 3.4, 4.8][depth];
    let bg = if focus { 0.06 } else if depth == 0 { 0.03 } else { 0.0 };
    let header = !matches!(row, TreeRow::Map(..));
    let r = c
        .spawn((
            UiStyle { gap: Some(0.7), pad: Some([0.0, 0.6, 0.0, pad_l].into()), grow: Some(1.0), ..UiStyle::row() }.bg_a(Tone::Fg, bg),
            Node::default(),
            // A header folds; a map's row toggles.
            if header { PickerAction::Fold(i) } else { PickerAction::Toggle(i) },
            chimera_ui::controls::Hover { base: ui.color(Tone::Fg, bg), over: ui.color(Tone::Fg, bg + 0.05) },
            chimera_ui::controls::clickable(),
            ChildOf(slot),
        ))
        .id();
    ui.hairline(c, r, Tone::Fg5, 1.0);
    if focus {
        ui.focus_bar(c, r, 0.24);
    }
    if !matches!(row, TreeRow::Map(..)) {
        let open = !picker.is_folded(games, row);
        let chev = ui.boxed(c, r, UiStyle { w: Some(0.8), shrink: Some(0.0), ..UiStyle::row() });
        if let Some((images, icons)) = res.as_mut() {
            ui.icon(c, chev, images, icons, if open { "chevron" } else { "chev_right" }, 0.5, Tone::Fg2);
        }
    }
    if header {
        // The box with a hit slop; the negative margin keeps the layout.
        let slop = 0.45;
        let hit = ui.boxed(
            c,
            r,
            UiStyle { pad: Some([slop, slop, slop, slop].into()), margin: Some([-slop, -slop, -slop, -slop].into()), shrink: Some(0.0), ..UiStyle::row() }
                .bg_a(Tone::Fg, 0.0),
        );
        c.entity(hit).insert((
            PickerAction::Toggle(i),
            TriHit,
            chimera_ui::controls::Hover { base: ui.color(Tone::Fg, 0.0), over: ui.color(Tone::Fg, 0.14) },
            chimera_ui::controls::clickable(),
        ));
        tri_box(c, ui, hit, picker.state(games, row));
    } else {
        tri_box(c, ui, r, picker.state(games, row));
    }
    let (role, size) = match depth {
        0 => (Role::DisplayBold, 0.9),
        1 => (Role::DisplayRegular, 0.8),
        _ => (Role::Mono, 0.62),
    };
    ui.text(c, r, role, size, Tone::Fg, &name);
    ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    if let Some(n) = count {
        ui.text(c, r, Role::Mono, 0.52, Tone::Fg2, &n);
    }
}

/// A slanted tri-state box, 1.3 x .75 cq.
fn tri_box(c: &mut Commands, ui: &Ui, parent: Entity, t: Tri) {
    let u = ui.units;
    let lean = 0.75 * ui.theme.tan();
    let b = c
        .spawn((
            UiStyle { w: Some(1.3 + lean), h: Some(0.75), shrink: Some(0.0), ..UiStyle::row() },
            Node { overflow: Overflow::clip(), ..default() },
            TriBox(t),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    let full = || Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, right: Val::ZERO, ..default() };
    let px = (1.5 * u.phys(u.height) / 1080.0).round().max(1.0);
    match t {
        Tri::On => {
            c.spawn((full(), UiSlant::new(Tone::Fg, 1.0), ChildOf(b)));
        }
        Tri::Mixed => {
            c.spawn((
                Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, width: Val::Percent(55.0), ..default() },
                UiSlant::new(Tone::Fg2, 1.0),
                ChildOf(b),
            ));
            c.spawn((full(), UiSlant::new(Tone::Fg2, 1.0).outline(px), ChildOf(b)));
        }
        Tri::Off => {
            c.spawn((full(), UiSlant::new(Tone::Fg4, 1.0).outline(px), ChildOf(b)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One game: a map with two slices, and another map.
    fn games() -> Vec<CatalogueGame> {
        let pool: Vec<String> = ["g:big", "g:big@west", "g:big@east", "g:small"].iter().map(|s| s.to_string()).collect();
        LocalCatalogue.listing(&pool)
    }

    fn row_of(p: &MapsPicker, games: &[CatalogueGame], entry: &str) -> TreeRow {
        p.rows(games)
            .into_iter()
            .find(|r| matches!(r, TreeRow::Map(..)) && MapsPicker::maps_of(games, r)[0] == entry)
            .unwrap()
    }

    #[test]
    fn a_maps_slices_are_its_children_and_go_off_with_it() {
        let games = games();
        let mut p = MapsPicker::default();
        // The slices are rows of their own, after their map.
        assert_eq!(p.rows(&games).len(), 2 + 4, "a game, a group, two maps and two slices");
        let (big, west, east) = (row_of(&p, &games, "g:big"), row_of(&p, &games, "g:big@west"), row_of(&p, &games, "g:big@east"));
        assert_eq!(MapsPicker::maps_of(&games, &big).len(), 3, "a map's row takes its slices");
        // Every entry counts: 4 of 4, then a slice off: 3 of 4.
        assert_eq!(p.counts(&games), (4, 4, 1));
        p.toggle(&games, &west);
        assert_eq!(p.state(&games, &west), Tri::Off);
        assert_eq!(p.state(&games, &big), Tri::Mixed);
        assert_eq!(p.state(&games, &TreeRow::Game(0)), Tri::Mixed, "the game's box agrees with its count");
        assert_eq!(p.counts(&games), (3, 4, 1));
        // The map on again: all of it on; off: its slices off with it.
        p.toggle(&games, &big);
        assert_eq!(p.state(&games, &big), Tri::On);
        p.toggle(&games, &big);
        assert_eq!(p.open.clone().unwrap(), BTreeSet::from(["g:big".to_string()]), "the map named, its slices with it");
        assert_eq!(p.state(&games, &east), Tri::Off);
        assert_eq!(p.counts(&games), (1, 4, 1));
        // A slice on under the map that is off: only the slice, the map's
        // box some on.
        p.toggle(&games, &west);
        assert_eq!(p.open.clone().unwrap(), BTreeSet::from(["g:big".to_string(), "+g:big@west".to_string()]));
        assert_eq!((p.state(&games, &west), p.state(&games, &east)), (Tri::On, Tri::Off));
        assert_eq!(p.state(&games, &big), Tri::Mixed);
        assert_eq!(p.counts(&games), (2, 4, 1));
        // Off again: the mark goes.
        p.toggle(&games, &west);
        assert_eq!(p.open.clone().unwrap(), BTreeSet::from(["g:big".to_string()]));
    }

    #[test]
    fn mp_maps_off_round_trips_a_slice_on_under_a_map_off() {
        let games = games();
        let mut p = MapsPicker::default();
        p.toggle(&games, &row_of(&p, &games, "g:big"));
        p.toggle(&games, &row_of(&p, &games, "g:big@east"));
        p.off = p.open.take().unwrap();
        // The stored form: the map, and the slice kept on marked.
        assert_eq!(p.off_line(), "+g:big@east,g:big");
        let off = chimera_net::minigame_net::MapsOff(p.off_line());
        assert!(off.has("g:big") && off.has("g:big@west") && !off.has("g:big@east") && !off.has("g:small"));
        assert_eq!(MapsPicker::off_from_line(&p.off_line()), p.off, "back from the line");
        // A slice off under a map that is on.
        let off = chimera_net::minigame_net::MapsOff("g:big@east".into());
        assert!(off.has("g:big@east") && !off.has("g:big@west") && !off.has("g:big"));
        // An old line (a map, no marks): its slices off with it, as before.
        let off = chimera_net::minigame_net::MapsOff("g:big".into());
        assert!(off.has("g:big@west") && off.has("g:big@east") && !off.has("g:small"));
    }

    #[test]
    fn a_slice_row_is_named_by_its_data() {
        let mut games = games();
        let text = "[[slice]]
map = \"big\"
id = \"west\"
name = \"West Base\"
                    points = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]
seed = [5.0, 1.0, 5.0]
";
        let slices = chimera_sim::minigame::arena::ArenaSlices(chimera_sim::minigame::arena::parse("g", text).unwrap());
        name_slices(&mut games, Some(&slices));
        assert_eq!(games[0].names.get("g:big@west").map(String::as_str), Some("West Base"));
        assert!(!games[0].names.contains_key("g:big@east"), "not in the data: its id");
    }
}
