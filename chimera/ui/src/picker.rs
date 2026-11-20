//! The kit's picker (docs/plans/active/shared-picker.md): one component
//! for every "choose from a catalogue" screen, the host's Maps and guns
//! and the Sandbox's Change gun and map, fed by data.
//!
//! - **Data** (`PickerGame`, `PickerGroup`, `PickerItem`): items grouped
//!   by game, a game's items in groups (Official maps, Custom maps; a
//!   group with an empty name has no header row), an item an id, its
//!   shown name, a kit icon, its tags (`facet:value`, what the search
//!   and the chips match) and optionally a parent (an item listed under
//!   another, one step in: an arena slice under its map). A new picker
//!   (characters) is new data, not new UI.
//! - **State** (`PickerState`): the ids off (`off`, the kept set) and,
//!   while open, the working copy (`open`); the current pick (single
//!   mode); the cursor (a row of `rows`), the folded games and groups
//!   (`game`, `game/group`), the search text and the ticked chips.
//!   Rows are what the search and the chips leave; a header's box and
//!   count are of the items it shows.
//! - **Modes** (`PickerMode`): *Multi* (the host's): every row has an on /
//!   off / some box (`Tri`); toggling a game or a group sets everything
//!   under it; an item with children takes them with it, a child toggles
//!   alone, kept on under a parent that is off by the mark `+id`
//!   (`ON_MARK`). Only a header's box (with a little slop) toggles, the
//!   rest of the header folds; an item's whole row or card toggles.
//!   *Single* (the Sandbox's): no boxes; a press on an item picks it
//!   (`PickerAction::Pick`), the current one outlined.
//! - **Looks** (`PickerLook`): *List* (a row a line: the Maps dialog) or
//!   *Cards* (a grid of cards under the games' headers). The container
//!   gives the width (`PickerFrame::w`); the cards' columns and size come
//!   from it and the theme (`card_grid`: a dialog gets fewer columns and
//!   smaller cards than the full screen).
//! - **Build**: `spawn` (the search and the chips when the frame asks for
//!   them, then the kit's recycling list, `vlist`; `spawn_parts` with a
//!   side pane, `PickerFrame::side`, for a detail pane the screen fills);
//!   `prepare` and `update` make the view again without new nodes (a
//!   screen that stays open: the Sandbox's). The slots are filled by
//!   `bind_slots` (one kit system for every open picker, from the
//!   snapshot in `Pickers` by list key). `dialog` is the frame the host's
//!   pickers sit in (the scrim, the plate, the title and counts, the
//!   legend, the keys, All off, Cancel and Done).
//! - **Cards** carry what a catalogue gives: an icon, a badge at the top
//!   left (a gun's game) with a family's count (`PickerItem::family`: one
//!   card for a gun's variants), a mark at the top right (a pinned slot),
//!   a download's bar along the bottom edge (`progress`). What only the
//!   screen can draw goes into empty slots it fills (`PickerSlot`, on
//!   `Added`, after `bind_slots`): a card's art when the item has no kit
//!   icon, its foot (`PickerFrame::foot`: a download's state), a folded
//!   game's header preview.
//! - **Search**: the kit's search field; typing is `PickerState::
//!   search_key` (the kit's `text_edit`: a caret, a selection, copy and
//!   paste) while `search_focus`, the field marked `PickerSearch` for the
//!   screen's pointer.
//! - **Keys**: `nav` (the List look: ↑↓ the row, ←→ fold and open; Cards:
//!   the grid keys of `VListState::nav`, on a header ← folds and → opens
//!   by the picker's folds, which the list's follow).
//! - **Numbers**: the theme's `[layout.picker]` (the kit's base theme,
//!   chimera/ui/data/shell.toml, every app's; a theme may set a key over
//!   it), none in code.
//! - **Actions**: `PickerAction` on the clickable nodes (and as a message
//!   from keys); the screen that owns the state applies them
//!   (`PickerState::toggle`, `fold`, `left`, `right`). In single mode a
//!   card also carries the list's `VHit` (a press selects, a second one is
//!   `VListEvent::Activate`, the kit's double-click rule).
//!
//! The gallery shows it: `ui_gallery picker` (both modes, both frames).
//!
//! Known shortcuts: a card's icon is the kit's class icon, not a render;
//! in the Cards look a game's groups run together under its header.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;

use crate::{
    IconImages, KitFonts, Role, Theme, Tone, UiSlant, UiStyle, UiUnits, VCursor, VSection, VShape, VSlot, VirtualList,
    VirtualLists,
    VHit, VListState, VNav,
    controls::{HERO_H, Hover, Kind, State, Ui, clickable},
    text_edit::{Caret, TextKey},
    text_fit::Ellipsize,
    vlist::VLine,
};

/// The layout table (`[layout.picker]`, chimera/ui/data/shell.toml).
pub const L: &str = "picker";

/// An item of a catalogue.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PickerItem {
    /// What the state stores (`game:map`, a weapon id).
    pub id: String,
    pub name: String,
    /// A kit icon (`IconImages`), drawn on a card.
    pub icon: Option<String>,
    /// Listed under this item, one step in (an arena slice's map).
    pub parent: Option<String>,
    /// `facet:value` (`class:rifle`, `size:large`): the chips and the
    /// search match them.
    pub tags: Vec<String>,
    /// A card's small words at its top left (a gun's game).
    pub badge: Option<String>,
    /// A card's mark at its top right (a pinned slot's number).
    pub mark: Option<String>,
    /// A family card: how many variants it stands for (more than one: the
    /// count beside the badge).
    pub family: usize,
    /// A download's share done, in thousandths: the bar along the card's
    /// bottom edge.
    pub progress: Option<u16>,
}

impl PickerItem {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        PickerItem { id: id.into(), name: name.into(), ..default() }
    }
}

/// A group of a game's items (an empty name: no header row).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PickerGroup {
    pub name: String,
    pub items: Vec<PickerItem>,
}

/// A game's section.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PickerGame {
    pub id: String,
    pub name: String,
    pub groups: Vec<PickerGroup>,
}

impl PickerGame {
    /// Every item, group by group.
    pub fn items(&self) -> impl Iterator<Item = &PickerItem> {
        self.groups.iter().flat_map(|k| k.items.iter())
    }
}

/// A row as shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerRow {
    Game(usize),
    Group(usize, usize),
    Item(usize, usize, usize),
}

/// A box's state: on, some on, off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tri {
    On,
    Mixed,
    Off,
}

/// Multi-select (boxes) or single pick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PickerMode {
    #[default]
    Multi,
    Single,
}

/// Rows or cards.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PickerLook {
    #[default]
    List,
    Cards,
}

/// The mark of a child kept on under a parent that is off (`+id`).
pub const ON_MARK: char = '+';

/// Whether `id` is off in `off` (the stored form): a child marked on is
/// on; else an id named is off, and a child whose parent is named is off
/// with it.
pub fn is_off(off: &BTreeSet<String>, id: &str, parent: Option<&str>) -> bool {
    if off.contains(&format!("{ON_MARK}{id}")) {
        return false;
    }
    off.contains(id) || parent.is_some_and(|p| off.contains(p))
}

/// The picker's state (module doc).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PickerState {
    /// The ids off, as kept (multi).
    pub off: BTreeSet<String>,
    /// Open: the working copy (multi), Done keeps it.
    pub open: Option<BTreeSet<String>>,
    /// The current pick (single).
    pub current: Option<String>,
    /// The cursor's row among the shown ones.
    pub cursor: usize,
    /// Folded games (`game`) and groups (`game/group`).
    pub folded: BTreeSet<String>,
    /// The search's words (every word must match a name, id, game or tag).
    pub search: String,
    /// The ticked chips (`facet:value`): within a facet any, across all.
    pub chips: BTreeSet<String>,
    /// The picker had the keys this frame (the page's own keys leave them).
    pub took_keys: bool,
    /// The search field has the keyboard (typing goes to it).
    pub search_focus: bool,
    /// The search field's caret and selection (chars).
    pub caret: Caret,
}

/// The search's longest text (bytes).
pub const SEARCH_MAX: usize = 48;

/// The facet of a tag (`class` of `class:rifle`).
pub fn facet(tag: &str) -> &str {
    tag.split_once(':').map_or(tag, |(f, _)| f)
}

/// A tag's value (`rifle` of `class:rifle`; shown with `_` as spaces).
pub fn tag_words(tag: &str) -> &str {
    tag.split_once(':').map_or(tag, |(_, v)| v)
}

impl PickerState {
    /// The search field takes the keyboard, the caret at the end.
    pub fn focus_search(&mut self) {
        self.search_focus = true;
        self.caret = Caret::end_of(&self.search);
    }

    /// A key in the search field (`text_edit::text_key`): edits the text
    /// at the caret (at most `SEARCH_MAX` bytes), or copies, cuts or
    /// pastes. A changed search puts the cursor on the first row. Whether
    /// the text changed.
    pub fn search_key(&mut self, key: TextKey, clipboard: &mut crate::clipboard::Clipboard) -> bool {
        let n = self.search.chars().count();
        self.caret = Caret { at: self.caret.at.min(n), anchor: self.caret.anchor.min(n) };
        let mut text = self.search.clone();
        let changed = self.caret.key(&mut text, key, clipboard, SEARCH_MAX, false);
        if changed {
            self.search = text;
            self.cursor = 0;
        }
        changed
    }

    /// Empties the search (its clear key), keeping the keyboard.
    pub fn clear_search(&mut self) {
        self.search.clear();
        self.caret = Caret::default();
        self.cursor = 0;
    }

    /// The search field's caret as drawn (None: not focused).
    pub fn search_cursor(&self) -> Option<crate::controls::TextCursor> {
        self.search_focus.then(|| self.caret.cursor(&self.search))
    }

    fn working(&self) -> &BTreeSet<String> {
        self.open.as_ref().unwrap_or(&self.off)
    }

    /// Whether `item` of `game` passes the search and the chips.
    pub fn shows(&self, game: &PickerGame, item: &PickerItem) -> bool {
        let words: Vec<String> = self.search.split_whitespace().map(str::to_lowercase).collect();
        let hay = || {
            let mut s = format!("{} {} {} {}", item.name, item.id, game.name, game.id);
            for t in &item.tags {
                s.push(' ');
                s.push_str(tag_words(t));
            }
            s.to_lowercase()
        };
        if !words.is_empty() {
            let h = hay();
            if !words.iter().all(|w| h.contains(w.as_str())) {
                return false;
            }
        }
        // Chips: within a facet any, across facets all; `game:<id>` is the
        // item's game.
        let mut facets: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for c in &self.chips {
            facets.entry(facet(c)).or_default().push(c.as_str());
        }
        facets.iter().all(|(f, chips)| {
            chips.iter().any(|c| if *f == "game" { tag_words(c) == game.id } else { item.tags.iter().any(|t| t == c) })
        })
    }

    /// The rows shown (folded games and groups hide what is under them;
    /// the search and the chips hide items, and a game or group left
    /// with none).
    pub fn rows(&self, games: &[PickerGame]) -> Vec<PickerRow> {
        self.rows_folding(games, true)
    }

    /// The rows of a look: the List look's `rows`; the Cards look keeps a
    /// folded game's items (the list folds them away: its cursor and its
    /// unfold keep their places).
    pub fn rows_in(&self, games: &[PickerGame], look: PickerLook) -> Vec<PickerRow> {
        self.rows_folding(games, look == PickerLook::List)
    }

    fn rows_folding(&self, games: &[PickerGame], folding: bool) -> Vec<PickerRow> {
        let mut out = Vec::new();
        for (g, game) in games.iter().enumerate() {
            let shown: Vec<Vec<usize>> = game
                .groups
                .iter()
                .map(|k| k.items.iter().enumerate().filter(|(_, it)| self.shows(game, it)).map(|(i, _)| i).collect())
                .collect();
            if shown.iter().all(Vec::is_empty) {
                continue;
            }
            out.push(PickerRow::Game(g));
            if folding && self.folded.contains(&game.id) {
                continue;
            }
            for (k, group) in game.groups.iter().enumerate() {
                if shown[k].is_empty() {
                    continue;
                }
                if !group.name.is_empty() {
                    out.push(PickerRow::Group(g, k));
                    if folding && self.folded.contains(&format!("{}/{}", game.id, group.name)) {
                        continue;
                    }
                }
                out.extend(shown[k].iter().map(|&i| PickerRow::Item(g, k, i)));
            }
        }
        out
    }

    /// The items under a row that the search and chips leave (an item's
    /// own row: it and its children).
    pub fn items_of<'a>(&self, games: &'a [PickerGame], row: &PickerRow) -> Vec<&'a PickerItem> {
        match *row {
            PickerRow::Game(g) => games[g].items().filter(|i| self.shows(&games[g], i)).collect(),
            PickerRow::Group(g, k) => games[g].groups[k].items.iter().filter(|i| self.shows(&games[g], i)).collect(),
            PickerRow::Item(g, k, i) => {
                let items = &games[g].groups[k].items;
                let me = &items[i];
                let mut out = vec![me];
                if me.parent.is_none() {
                    out.extend(items.iter().filter(|s| s.parent.as_deref() == Some(me.id.as_str())));
                }
                out
            }
        }
    }

    /// The ids under a row (`items_of`).
    pub fn ids_of<'a>(&self, games: &'a [PickerGame], row: &PickerRow) -> Vec<&'a str> {
        self.items_of(games, row).into_iter().map(|i| i.id.as_str()).collect()
    }

    /// A row's box in the working copy (else the kept one).
    pub fn state(&self, games: &[PickerGame], row: &PickerRow) -> Tri {
        let items = self.items_of(games, row);
        let off = self.working();
        let on = items.iter().filter(|i| !is_off(off, &i.id, i.parent.as_deref())).count();
        match on {
            0 => Tri::Off,
            n if n == items.len() => Tri::On,
            _ => Tri::Mixed,
        }
    }

    /// How many under a row are on, of how many (a header's count).
    pub fn count(&self, games: &[PickerGame], row: &PickerRow) -> (usize, usize) {
        let items = self.items_of(games, row);
        let off = self.working();
        (items.iter().filter(|i| !is_off(off, &i.id, i.parent.as_deref())).count(), items.len())
    }

    /// Toggles a row: all under it on, unless all are on (then off). An
    /// item takes its children with it; a child toggles alone, its parent
    /// as it was.
    pub fn toggle(&mut self, games: &[PickerGame], row: &PickerRow) {
        let items: Vec<PickerItem> = self.items_of(games, row).into_iter().cloned().collect();
        let on = self.state(games, row) != Tri::On;
        let all: Vec<&PickerItem> = games.iter().flat_map(|g| g.items()).collect();
        let off = self.open.get_or_insert_with(|| self.off.clone());
        for it in &items {
            // A child toggled with its parent goes with it.
            if it.parent.as_ref().is_some_and(|p| items.iter().any(|x| &x.id == p)) {
                continue;
            }
            set(off, &all, it, on);
        }
    }

    /// Whether a game or group row is folded.
    pub fn is_folded(&self, games: &[PickerGame], row: &PickerRow) -> bool {
        fold_key(games, row).is_some_and(|k| self.folded.contains(&k))
    }

    /// Folds or opens a game or a group row (`open`: show what is under).
    pub fn fold(&mut self, games: &[PickerGame], row: &PickerRow, open: bool) {
        let Some(key) = fold_key(games, row) else { return };
        if open {
            self.folded.remove(&key);
        } else {
            self.folded.insert(key);
        }
    }

    /// ←: on an item, folds the group (or game) it is in and puts the
    /// cursor on that header; on an open game or group, folds it; on a
    /// folded group, goes up to its game's header. True when it did
    /// something.
    pub fn left(&mut self, games: &[PickerGame]) -> bool {
        let rows = self.rows(games);
        let Some(row) = rows.get(self.cursor).copied() else { return false };
        let to = match row {
            PickerRow::Item(g, k, _) => {
                let head = if games[g].groups[k].name.is_empty() { PickerRow::Game(g) } else { PickerRow::Group(g, k) };
                self.fold(games, &head, false);
                head
            }
            PickerRow::Group(g, _) if self.is_folded(games, &row) => PickerRow::Game(g),
            _ if !self.is_folded(games, &row) => {
                self.fold(games, &row, false);
                row
            }
            _ => return false,
        };
        let now = self.rows(games);
        match now.iter().position(|r| *r == to) {
            Some(i) if i != self.cursor || rows.len() != now.len() => {
                self.cursor = i;
                true
            }
            _ => false,
        }
    }

    /// →: opens a folded game or group; false on anything else.
    pub fn right(&mut self, games: &[PickerGame]) -> bool {
        match self.rows(games).get(self.cursor).copied() {
            Some(row) if !matches!(row, PickerRow::Item(..)) && self.is_folded(games, &row) => {
                self.fold(games, &row, true);
                true
            }
            _ => false,
        }
    }

    /// How many items are on, of how many, in how many games (the kept
    /// set, or the working copy while open; the search ignored).
    pub fn counts(&self, games: &[PickerGame]) -> (usize, usize, usize) {
        let off = self.working();
        let on_of = |g: &PickerGame| g.items().filter(|i| !is_off(off, &i.id, i.parent.as_deref())).count();
        let all: usize = games.iter().map(|g| g.items().count()).sum();
        let on: usize = games.iter().map(on_of).sum();
        (on, all, games.iter().filter(|g| on_of(g) > 0).count())
    }

    /// Every item off (All off), in the working copy.
    pub fn all_off(&mut self, games: &[PickerGame]) {
        self.open = Some(games.iter().flat_map(|g| g.items().map(|i| i.id.clone())).collect());
    }

    /// The kept set from a comma-separated line (`off_line`'s inverse).
    pub fn off_from_line(line: &str) -> BTreeSet<String> {
        line.split(',').map(str::trim).filter(|m| !m.is_empty()).map(str::to_string).collect()
    }

    /// The kept set as a comma-separated line (what a cvar carries).
    pub fn off_line(&self) -> String {
        self.off.iter().cloned().collect::<Vec<_>>().join(",")
    }

    /// The item at a row (None for a header).
    pub fn item<'a>(games: &'a [PickerGame], row: &PickerRow) -> Option<&'a PickerItem> {
        match *row {
            PickerRow::Item(g, k, i) => games.get(g)?.groups.get(k)?.items.get(i),
            _ => None,
        }
    }

    /// The facets' chips the data offers: every tag of every item, by
    /// facet in first-seen order, and `game:<id>` when there is more than
    /// one game.
    pub fn chips_of(games: &[PickerGame], facets: &[&str]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for f in facets {
            if *f == "game" {
                if games.len() > 1 {
                    out.extend(games.iter().map(|g| format!("game:{}", g.id)));
                }
                continue;
            }
            for g in games {
                for i in g.items() {
                    for t in &i.tags {
                        if facet(t) == *f && !out.contains(t) {
                            out.push(t.clone());
                        }
                    }
                }
            }
        }
        out
    }
}

fn fold_key(games: &[PickerGame], row: &PickerRow) -> Option<String> {
    match *row {
        PickerRow::Game(g) => Some(games[g].id.clone()),
        PickerRow::Group(g, k) => Some(format!("{}/{}", games[g].id, games[g].groups[k].name)),
        PickerRow::Item(..) => None,
    }
}

/// Sets `it` on or off in `off`, in the stored form: an item off takes
/// its children with it (their own ids and marks go); a child under a
/// parent that is off is on by its mark, else off by its id.
fn set(off: &mut BTreeSet<String>, all: &[&PickerItem], it: &PickerItem, on: bool) {
    let mark = format!("{ON_MARK}{}", it.id);
    match &it.parent {
        None => {
            for s in all.iter().filter(|s| s.parent.as_deref() == Some(it.id.as_str())) {
                off.remove(&format!("{ON_MARK}{}", s.id));
                off.remove(&s.id);
            }
            if on {
                off.remove(&it.id);
            } else {
                off.insert(it.id.clone());
            }
        }
        Some(parent) => {
            off.remove(&mark);
            off.remove(&it.id);
            match (on, off.contains(parent)) {
                (true, true) => {
                    off.insert(mark);
                }
                (false, false) => {
                    off.insert(it.id.clone());
                }
                _ => {}
            }
        }
    }
}

/// What a click (or a key) in a picker does. Indices are rows of
/// `PickerState::rows` (chips: of `PickerView::chips`).
#[derive(Component, Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerAction {
    /// A box (and its hit slop), an item's row or card in multi mode: on
    /// or off.
    Toggle(usize),
    /// A game's or group's header (the chevron, the name, the rest of the
    /// row): fold or open.
    Fold(usize),
    /// An item in single mode: pick it (a card also carries the list's
    /// `VHit`: a press selects, a second press is `VListEvent::Activate`).
    Pick(usize),
    /// A filter chip: tick or untick.
    Chip(usize),
    /// A click on the search field.
    FocusSearch,
    /// The search field's clear key.
    ClearSearch,
    Done,
    Cancel,
    AllOff,
}

/// A header's box with its hit slop (tests find it).
#[derive(Component, Clone, Copy, Debug)]
pub struct TriHit;

/// A row's tri-state box (its state, for tests).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriBox(pub Tri);

/// A card (its row, for tests).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickerCard(pub usize);

/// A card's item id (the screen finds its cards by it).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct PickerId(pub String);

/// A picker's search field (its list key): the screen's pointer system
/// places the caret in it (`controls::TextPress`).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct PickerSearch(pub String);

/// The parts of a card or a header the screen that owns the picker draws
/// (`PickerSlot`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerPart {
    /// A card's picture, when its item has no kit icon (a gun's own icon).
    Art,
    /// A card's foot (`PickerFrame::foot`: a download's state, a size).
    Foot,
    /// A folded game's header, after its count (a preview of what's in it).
    Head,
}

/// An empty node in a card or a header for the owning screen to fill (on
/// `Added<PickerSlot>`, after `bind_slots`): the list's key, the item's
/// id (a header's: the game's), the part.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct PickerSlot {
    pub list: String,
    pub id: String,
    pub part: PickerPart,
}

/// The container's say: the width it gives (cq), the look, and whether
/// the search and the chips show (with the facets the chips offer).
#[derive(Clone, Debug, Default)]
pub struct PickerFrame {
    pub w: f32,
    pub look: PickerLook,
    pub mode: PickerMode,
    pub search: bool,
    /// The facets the chips offer (`class`, `game`, `size`), in order.
    pub facets: Vec<String>,
    /// The search field's placeholder.
    pub placeholder: String,
    /// The search field's caret while typing (None: the state's,
    /// `PickerState::search_cursor`).
    pub caret: Option<crate::controls::TextCursor>,
    /// Cards: a foot slot (`PickerPart::Foot`) in place of the tags line.
    pub foot: bool,
    /// A side pane this wide (cq) at the list's right (`PickerParts::side`:
    /// a detail pane the screen fills); the list has the rest of `w`.
    pub side: Option<f32>,
    /// The list's cursor is the screen's (it moves it: the grid keys, a
    /// press); `prepare` leaves it where it is (`state.cursor` unused).
    pub list_cursor: bool,
}

/// The cards' grid for a width: columns, a card's width and height (cq).
/// The widest card that fits `card_max_w` sets the columns; a narrower
/// frame gets fewer, smaller cards (never under `card_min_w`, one column
/// at least).
pub fn card_grid(theme: &Theme, w: f32) -> (usize, f32, f32) {
    let (min_w, max_w, gap, aspect) = (
        theme.layout(L, "card_min_w"),
        theme.layout(L, "card_max_w"),
        theme.layout(L, "card_gap"),
        theme.layout(L, "card_aspect"),
    );
    let each = |cols: usize| (w - gap * (cols as f32 - 1.0)) / (cols as f32);
    let mut cols = ((w + gap) / (max_w + gap)).ceil().max(1.0) as usize;
    while cols > 1 && each(cols) < min_w {
        cols -= 1;
    }
    let card_w = each(cols).max(0.0);
    (cols, card_w, card_w * aspect)
}

/// The list's width in a frame (less the side pane and its gap).
pub fn list_w(theme: &Theme, frame: &PickerFrame) -> f32 {
    frame.side.map_or(frame.w, |s| frame.w - s - theme.layout(L, "side_gap")).max(0.0)
}

/// An open picker as built (`bind_slots` fills its list's slots from it).
#[derive(Clone, Debug, Default)]
pub struct PickerView {
    pub games: Vec<PickerGame>,
    pub rows: Vec<PickerRow>,
    pub state: PickerState,
    pub frame: PickerFrame,
    /// Cards: each section's header row and its items' rows.
    pub sections: Vec<(usize, Vec<usize>)>,
    pub card: (usize, f32, f32),
    /// The chips shown (`PickerAction::Chip` indexes them).
    pub chips: Vec<String>,
    /// Its list's key.
    pub list_key: String,
}

impl PickerView {
    /// The row at a list cursor (Cards: a section's header or a card; List:
    /// the line).
    pub fn row_at(&self, c: VCursor) -> Option<usize> {
        match (self.frame.look, c) {
            (PickerLook::List, VCursor::Item(_, i)) => (i < self.rows.len()).then_some(i),
            (PickerLook::List, VCursor::Header(_)) => None,
            (PickerLook::Cards, VCursor::Header(s)) => self.sections.get(s).map(|x| x.0),
            (PickerLook::Cards, VCursor::Item(s, j)) => self.sections.get(s)?.1.get(j).copied(),
        }
    }

    /// The list cursor of a row (`row_at`'s inverse).
    pub fn cursor_of(&self, row: usize) -> Option<VCursor> {
        cursor_of(self.frame.look, &self.rows, &self.sections, row)
    }

    /// The item at a list cursor.
    pub fn item_at(&self, c: VCursor) -> Option<&PickerItem> {
        PickerState::item(&self.games, self.rows.get(self.row_at(c)?)?)
    }
}

fn cursor_of(look: PickerLook, rows: &[PickerRow], sections: &[(usize, Vec<usize>)], row: usize) -> Option<VCursor> {
    match look {
        PickerLook::List => (row < rows.len()).then_some(VCursor::Item(0, row)),
        PickerLook::Cards => sections.iter().enumerate().find_map(|(s, (h, items))| {
            if *h == row { Some(VCursor::Header(s)) } else { items.iter().position(|r| *r == row).map(|j| VCursor::Item(s, j)) }
        }),
    }
}

/// Every open picker by its list key.
#[derive(Resource, Clone, Debug, Default)]
pub struct Pickers(pub BTreeMap<String, PickerView>);

/// What `spawn_parts` built.
#[derive(Clone, Copy, Debug)]
pub struct PickerParts {
    /// The list's scroll frame.
    pub list: Entity,
    /// The side pane (`PickerFrame::side`).
    pub side: Option<Entity>,
}

/// The view of a picker and its list's state (`spawn` and `update`): the
/// rows (`rows_in`), the cards' sections (a folded game's folded in the
/// list too, so only its header shows and the grid keys stop on it), the
/// shape and the cursor.
pub fn prepare(theme: &Theme, key: &str, games: &[PickerGame], state: &PickerState, frame: PickerFrame, lists: &mut VirtualLists) -> PickerView {
    let l = |k: &str| theme.layout(L, k);
    let rows = state.rows_in(games, frame.look);
    let chips = PickerState::chips_of(games, &frame.facets.iter().map(String::as_str).collect::<Vec<_>>());
    let (sections, shape, vsections, folded) = match frame.look {
        PickerLook::List => (Vec::new(), VShape::list(l("row_h")), vec![VSection::plain(key, rows.len())], Vec::new()),
        PickerLook::Cards => {
            let (cols, _, card_h) = card_grid(theme, list_w(theme, &frame));
            let mut sections: Vec<(usize, Vec<usize>)> = Vec::new();
            let mut folded = Vec::new();
            for (r, row) in rows.iter().enumerate() {
                match row {
                    PickerRow::Game(g) => {
                        let k = format!("{key}/{}", games[*g].id);
                        if state.folded.contains(&games[*g].id) {
                            folded.push(k);
                        }
                        sections.push((r, Vec::new()));
                    }
                    PickerRow::Item(..) => {
                        if let Some(s) = sections.last_mut() {
                            s.1.push(r);
                        }
                    }
                    PickerRow::Group(..) => {}
                }
            }
            let vs = sections.iter().zip(rows.iter().filter(|r| matches!(r, PickerRow::Game(_)))).map(|((_, items), r)| {
                let PickerRow::Game(g) = r else { unreachable!() };
                VSection::new(format!("{key}/{}", games[*g].id), items.len())
            });
            (sections.clone(), VShape::grid(cols, l("head_h"), card_h, l("card_gap")), vs.collect(), folded)
        }
    };
    let at = if rows.is_empty() || frame.list_cursor { None } else { cursor_of(frame.look, &rows, &sections, state.cursor.min(rows.len() - 1)) };
    let st = lists.state(key);
    st.set_shape(shape);
    // The list's folds are the picker's (Cards; set before the sections so
    // the layout is made once).
    let mine: BTreeSet<String> = folded.into_iter().collect();
    if frame.look == PickerLook::Cards && st.folded != mine {
        st.folded = mine;
        st.sections.clear();
    }
    st.set_sections(vsections);
    if !frame.list_cursor && st.cursor != at {
        st.cursor = at;
        st.reveal = true;
    }
    st.bump();
    PickerView { games: games.to_vec(), rows, state: state.clone(), card: card_grid(theme, list_w(theme, &frame)), frame, sections, chips, list_key: key.to_string() }
}

/// Makes an open picker show `games` and `state` again without building
/// it anew (the screen keeps its nodes; the slots are bound again).
pub fn update(theme: &Theme, pickers: &mut Pickers, key: &str, games: &[PickerGame], state: &PickerState, frame: PickerFrame, lists: &mut VirtualLists) {
    let view = prepare(theme, key, games, state, frame, lists);
    pickers.0.insert(key.to_string(), view);
}

/// The grid keys (`VListState::nav`) on a picker: ↑↓ the line, ←→ in
/// reading order; on a game's header ← folds and → opens it (the
/// picker's folds), then → steps in. The List look: ↑↓ the row, ←→
/// `left` and `right`. The cursor follows (`state.cursor`); true when
/// something moved or folded.
pub fn nav(state: &mut PickerState, view: &PickerView, list: &mut VListState, k: VNav) -> bool {
    let games = &view.games;
    if view.frame.look == PickerLook::List {
        let n = view.rows.len();
        if n == 0 {
            return false;
        }
        let page = (list.view_h / list.shape.map_or(2.0, |s| s.row_h)).floor().max(1.0) as usize;
        let to = match k {
            VNav::Up => state.cursor.saturating_sub(1),
            VNav::Down => (state.cursor + 1).min(n - 1),
            VNav::PageUp => state.cursor.saturating_sub(page),
            VNav::PageDown => (state.cursor + page).min(n - 1),
            VNav::Home => 0,
            VNav::End => n - 1,
            VNav::Left => return state.left(games),
            VNav::Right => return state.right(games),
        };
        let moved = to != state.cursor;
        state.cursor = to;
        return moved;
    }
    let row = state.cursor.min(view.rows.len().saturating_sub(1));
    if let (Some(PickerRow::Game(_)), VNav::Left | VNav::Right) = (view.rows.get(row), k) {
        let open = k == VNav::Right;
        if state.is_folded(games, &view.rows[row]) == open {
            fold(state, view, list, row, open);
            return true;
        }
    }
    if list.cursor.is_none() {
        list.cursor = view.cursor_of(row);
    }
    let moved = list.nav(k);
    if let Some(r) = list.cursor.and_then(|c| view.row_at(c)) {
        state.cursor = r;
    }
    moved
}

/// Folds or opens a game's header row in the Cards look, in the picker's
/// state and its list at once (the List look: the state's alone).
pub fn fold(state: &mut PickerState, view: &PickerView, list: &mut VListState, row: usize, open: bool) {
    let Some(r) = view.rows.get(row).copied() else { return };
    state.fold(&view.games, &r, open);
    if let Some(VCursor::Header(s)) = view.cursor_of(row) {
        list.fold(s, !open);
    }
}

/// A Cards picker's folded games as its list has them (`{key}/{game}`
/// sections folded): a screen whose list is the truth (the Sandbox's,
/// staged and tested by the list) reads them into its state.
pub fn folds_of(list: &VListState, key: &str) -> BTreeSet<String> {
    let p = format!("{key}/");
    list.folded.iter().filter_map(|k| k.strip_prefix(&p)).map(str::to_string).collect()
}

/// Builds a picker under `parent` (module doc): the search and the chips
/// when `frame` asks for them, then the list or the grid, its state in
/// `lists[key]` (the cursor on `state.cursor`). Returns the list's
/// scroll frame.
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    c: &mut Commands,
    ui: &Ui,
    parent: Entity,
    key: &str,
    games: &[PickerGame],
    state: &PickerState,
    frame: PickerFrame,
    lists: &mut VirtualLists,
    list_style: UiStyle,
) -> Entity {
    spawn_parts(c, ui, parent, key, games, state, frame, lists, list_style).list
}

/// `spawn`, with the side pane (`PickerFrame::side`) when asked for.
#[allow(clippy::too_many_arguments)]
pub fn spawn_parts(
    c: &mut Commands,
    ui: &Ui,
    parent: Entity,
    key: &str,
    games: &[PickerGame],
    state: &PickerState,
    frame: PickerFrame,
    lists: &mut VirtualLists,
    list_style: UiStyle,
) -> PickerParts {
    let t = ui.theme;
    let l = |k: &str| t.layout(L, k);
    let view = prepare(t, key, games, state, frame, lists);
    let frame = &view.frame;
    if frame.search || !view.chips.is_empty() {
        let bar = ui.boxed(c, parent, UiStyle { gap: Some(l("bar_gap")), align: Some(AlignItems::Center), ..UiStyle::row() });
        c.entity(bar).insert(Node { flex_wrap: FlexWrap::Wrap, ..default() });
        if frame.search {
            let caret = frame.caret.or(state.search_cursor());
            let f = ui.search_field(c, bar, &state.search, &frame.placeholder, caret, l("search_w").min(frame.w), State::focused(caret.is_some()));
            c.entity(f.field).insert((PickerAction::FocusSearch, PickerSearch(key.to_string()), clickable()));
            if let Some(k) = f.clear {
                c.entity(k).insert(PickerAction::ClearSearch);
            }
        }
        for (i, chip) in view.chips.iter().enumerate() {
            let on = state.chips.contains(chip);
            let words = if facet(chip) == "game" {
                games.iter().find(|g| g.id == tag_words(chip)).map_or(tag_words(chip).to_string(), |g| g.name.clone())
            } else {
                tag_words(chip).replace('_', " ")
            };
            let ch = ui.boxed(
                c,
                bar,
                UiStyle { h: Some(l("chip_h")), pad: Some([0.0, l("chip_pad"), 0.0, l("chip_pad")].into()), align: Some(AlignItems::Center), ..UiStyle::row() }
                    .bg_a(Tone::Fg, if on { 1.0 } else { 0.0 })
                    .outline(if on { Tone::Fg } else { Tone::Fg4 }, 1.0),
            );
            let bg = if on { ui.color(Tone::Fg, 1.0) } else { ui.color(Tone::Fg, 0.0) };
            c.entity(ch).insert((PickerAction::Chip(i), Hover { base: bg, over: if on { bg } else { ui.color(Tone::Fg, l("hover")) } }, clickable()));
            ui.text(c, ch, Role::Mono, l("chip_size"), if on { Tone::Ink } else { Tone::Fg2 }, &words);
        }
    }
    let (holder, side) = match frame.side {
        Some(w) => {
            let row = ui.boxed(c, parent, UiStyle { gap: Some(l("side_gap")), grow: Some(1.0), align: Some(AlignItems::Stretch), ..UiStyle::row() });
            c.entity(row).insert(Node { min_height: Val::Px(0.0), ..default() });
            (row, Some(w))
        }
        None => (parent, None),
    };
    let shape = lists.state(key).shape.unwrap_or(VShape::list(l("row_h")));
    let list = ui.virtual_list(c, holder, key, list_style, l("view_h"), shape);
    c.entity(list.scroll.frame).insert(Node { min_height: Val::Px(0.0), min_width: Val::Px(0.0), ..default() });
    let side = side.map(|w| ui.boxed(c, holder, UiStyle { w: Some(w), shrink: Some(0.0), ..UiStyle::column() }));
    let key = key.to_string();
    c.queue(move |w: &mut World| {
        w.get_resource_or_init::<Pickers>().0.insert(key, view);
    });
    PickerParts { list: list.scroll.frame, side }
}

/// The dialog a host's picker sits in (design rules 13): the scrim over
/// `root`, the plate (`dialog_side` in from each side, the window's top
/// and foot), the title and counts, the legend, the picker, the keys and
/// the actions with Done the hero. `marker` goes on the scrim.
#[allow(clippy::too_many_arguments)]
pub fn dialog(
    c: &mut Commands,
    ui: &Ui,
    root: Entity,
    title: &str,
    key: &str,
    games: &[PickerGame],
    state: &PickerState,
    mut frame: PickerFrame,
    keys: &[(&str, &str)],
    lists: &mut VirtualLists,
    marker: impl Bundle,
) -> Entity {
    let t = ui.theme;
    let l = |k: &str| t.layout(L, k);
    let sh = crate::shell::ShellLayout::of(t);
    let scrim = c
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
            BackgroundColor(ui.color(Tone::Ink, l("scrim"))),
            bevy::ui::FocusPolicy::Block,
            ZIndex(40),
            marker,
            ChildOf(root),
        ))
        .id();
    let fw = ui.boxed(c, scrim, UiStyle { w: Some(100.0), h: Some(56.25), shrink: Some(0.0), ..UiStyle::row() });
    let side = l("dialog_side");
    let (pt, pr, pb, pl) = (l("dialog_pad_t"), l("dialog_pad_x"), l("dialog_pad_b"), l("dialog_pad_x"));
    let plate = c
        .spawn((
            UiStyle { gap: Some(l("dialog_gap")), pad: Some([pt, pr, pb, pl].into()), ..UiStyle::column() }
                .abs([Some(side), Some(sh.win_top), Some(side), Some(sh.win_bottom)])
                .bg(Tone::Ink2)
                .outline(Tone::Fg4, 1.0),
            Node::default(),
            BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, l("shadow_a")), Val::ZERO, ui.units.val(l("shadow_y")), Val::ZERO, ui.units.val(l("shadow_blur"))),
            ChildOf(fw),
        ))
        .id();
    c.spawn((
        Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, height: ui.units.val(l("top_line")), ..default() },
        BackgroundColor(ui.color(Tone::Fg, 1.0)),
        ChildOf(plate),
    ));
    let (on, all, _) = state.counts(games);
    let head = ui.boxed(c, plate, UiStyle { gap: Some(l("title_gap")), align: Some(AlignItems::Baseline), ..UiStyle::row() });
    ui.text(c, head, Role::DisplayBold, l("title"), Tone::Fg, title);
    ui.text(c, head, Role::Mono, l("meta"), Tone::Fg2, &format!("{on} of {all} on"));
    if frame.mode == PickerMode::Multi {
        let legend = ui.boxed(c, plate, UiStyle { gap: Some(l("legend_gap")), ..UiStyle::row() });
        for (tri, words) in [(Tri::On, "On"), (Tri::Mixed, "Some on"), (Tri::Off, "Off")] {
            tri_box(c, ui, legend, tri);
            let w = ui.boxed(c, legend, UiStyle { margin: Some([0.0, l("legend_after"), 0.0, 0.0].into()), ..UiStyle::row() });
            ui.text(c, w, Role::Mono, l("meta"), Tone::Fg2, words);
        }
    }
    // The plate's inner width is the picker's.
    frame.w = 100.0 - 2.0 * side - pl - pr;
    spawn(c, ui, plate, key, games, state, frame, lists, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    let krow = ui.boxed(c, plate, UiStyle { gap: Some(l("keys_gap")), ..UiStyle::row() });
    for (k, w) in keys {
        let g = ui.boxed(c, krow, UiStyle { gap: Some(l("keys_inner")), ..UiStyle::row() });
        ui.kbd(c, g, k, l("keys_size"), Tone::Fg2);
        ui.text(c, g, Role::Mono, l("keys_size"), Tone::Fg2, w);
    }
    let acts = ui.boxed(c, plate, UiStyle { gap: Some(l("acts_gap")), ..UiStyle::row() });
    let all_off = ui.button(c, acts, Kind::Ghost, "All off", None, l("ghost_h"), State::NONE);
    c.entity(all_off).insert(PickerAction::AllOff);
    ui.boxed(c, acts, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    let cancel = ui.button(c, acts, Kind::Secondary, "Cancel", Some("Esc"), l("cancel_h"), State::NONE);
    c.entity(cancel).insert(PickerAction::Cancel);
    let done = ui.button(c, acts, Kind::Primary, "Done", Some("Enter"), HERO_H, State::NONE);
    c.entity(done).insert(PickerAction::Done);
    scrim
}

/// Fills every open picker's slots (`spawn`): a slot's line is a row (the
/// List look) or a header or a row of cards (Cards); rebound only when
/// another line scrolls into it.
#[allow(clippy::too_many_arguments)]
pub fn bind_slots(
    theme: Res<Theme>,
    fonts: Option<Res<KitFonts>>,
    units: Res<UiUnits>,
    pickers: Option<Res<Pickers>>,
    lists: Res<VirtualLists>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut icons: Option<ResMut<IconImages>>,
    list_q: Query<&VirtualList>,
    slots: Query<(Entity, &VSlot), Changed<VSlot>>,
    mut c: Commands,
) {
    let Some(pickers) = pickers else { return };
    let ui = Ui::new(&theme, fonts.as_deref(), *units);
    for (slot, s) in &slots {
        let Ok(list) = list_q.get(s.list) else { continue };
        let (Some(view), Some(st)) = (pickers.0.get(&list.key), lists.get(&list.key)) else { continue };
        c.entity(slot).despawn_children();
        let Some(line) = s.line.and_then(|l| st.layout.lines.get(l)).copied() else { continue };
        let mut res = match (images.as_deref_mut(), icons.as_deref_mut()) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        };
        match (view.frame.look, line) {
            (PickerLook::List, VLine::Row { first, .. }) => {
                if view.rows.get(first).is_some() {
                    list_row(&mut c, &ui, slot, view, first, res.as_mut().map(|(a, b)| (&mut **a, &mut **b)));
                }
            }
            (PickerLook::Cards, VLine::Header(sec)) => {
                if let Some((h, _)) = view.sections.get(sec) {
                    list_row(&mut c, &ui, slot, view, *h, res.as_mut().map(|(a, b)| (&mut **a, &mut **b)));
                }
            }
            (PickerLook::Cards, VLine::Row { section, first, count }) => {
                let Some((_, items)) = view.sections.get(section) else { continue };
                let r = ui.boxed(&mut c, slot, UiStyle { gap: Some(theme.layout(L, "card_gap")), grow: Some(1.0), ..UiStyle::row() });
                for j in first..(first + count).min(items.len()) {
                    let at = st.cursor == Some(VCursor::Item(section, j));
                    let hit = VHit { slot, col: Some(j - first) };
                    card(&mut c, &ui, r, view, items[j], at, hit, res.as_mut().map(|(a, b)| (&mut **a, &mut **b)));
                }
            }
            _ => {}
        }
    }
}

/// One row of the List look (or a header of the Cards look) in `slot`.
fn list_row(c: &mut Commands, ui: &Ui, slot: Entity, view: &PickerView, i: usize, res: Option<(&mut Assets<Image>, &mut IconImages)>) {
    let t = ui.theme;
    let l = |k: &str| t.layout(L, k);
    let (games, picker, row) = (&view.games, &view.state, &view.rows[i]);
    let multi = view.frame.mode == PickerMode::Multi;
    let focus = i == picker.cursor;
    let count_of = |row: &PickerRow| {
        let (on, n) = picker.count(games, row);
        Some(if multi { format!("{on} of {n}") } else { format!("{n}") })
    };
    let grouped = |g: usize, k: usize| !games[g].groups[k].name.is_empty();
    let (depth, name, count) = match *row {
        PickerRow::Game(g) => (0, games[g].name.clone(), count_of(row)),
        PickerRow::Group(g, k) => (1, games[g].groups[k].name.clone(), count_of(row)),
        PickerRow::Item(g, k, m) => {
            let it = &games[g].groups[k].items[m];
            let base = if grouped(g, k) { 2 } else { 1 };
            (base + it.parent.is_some() as usize, it.name.clone(), None)
        }
    };
    let pad_l = [l("pad_game"), l("pad_group"), l("pad_item"), l("pad_child")][depth];
    let bg = if focus { l("bg_focus") } else if depth == 0 { l("bg_game") } else { 0.0 };
    let header = !matches!(row, PickerRow::Item(..));
    let current = !multi && PickerState::item(games, row).is_some_and(|it| picker.current.as_deref() == Some(it.id.as_str()));
    let act = match (header, multi) {
        (true, _) => PickerAction::Fold(i),
        (false, true) => PickerAction::Toggle(i),
        (false, false) => PickerAction::Pick(i),
    };
    let r = c
        .spawn((
            UiStyle { gap: Some(l("row_gap")), pad: Some([0.0, l("row_pad_r"), 0.0, pad_l].into()), grow: Some(1.0), ..UiStyle::row() }.bg_a(Tone::Fg, bg),
            Node::default(),
            act,
            Hover { base: ui.color(Tone::Fg, bg), over: ui.color(Tone::Fg, bg + l("hover")) },
            clickable(),
            ChildOf(slot),
        ))
        .id();
    ui.hairline(c, r, Tone::Fg5, 1.0);
    if focus {
        ui.focus_bar(c, r, l("focus_bar"));
    }
    if header {
        let open = !picker.is_folded(games, row);
        let chev = ui.boxed(c, r, UiStyle { w: Some(l("chev_w")), shrink: Some(0.0), ..UiStyle::row() });
        if let Some((images, icons)) = res {
            ui.icon(c, chev, images, icons, if open { "chevron" } else { "chev_right" }, l("chev_h"), Tone::Fg2);
        }
    }
    if multi {
        if header {
            // The box with a hit slop; the negative margin keeps the layout.
            let slop = l("slop");
            let hit = ui.boxed(
                c,
                r,
                UiStyle { pad: Some([slop, slop, slop, slop].into()), margin: Some([-slop, -slop, -slop, -slop].into()), shrink: Some(0.0), ..UiStyle::row() }
                    .bg_a(Tone::Fg, 0.0),
            );
            c.entity(hit).insert((
                PickerAction::Toggle(i),
                TriHit,
                Hover { base: ui.color(Tone::Fg, 0.0), over: ui.color(Tone::Fg, l("slop_hover")) },
                clickable(),
            ));
            tri_box(c, ui, hit, picker.state(games, row));
        } else {
            tri_box(c, ui, r, picker.state(games, row));
        }
    }
    let (role, size) = match depth {
        0 => (Role::DisplayBold, l("name_game")),
        1 if header => (Role::DisplayRegular, l("name_group")),
        _ => (Role::Mono, l("name_item")),
    };
    ui.text(c, r, role, size, if current { Tone::AccText } else { Tone::Fg }, &name);
    ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    if let Some(n) = count {
        ui.text(c, r, Role::Mono, l("count"), Tone::Fg2, &n);
    }
    // A folded game's header in the Cards look: the screen's preview.
    if let (PickerRow::Game(g), PickerLook::Cards) = (row, view.frame.look)
        && picker.is_folded(games, row)
    {
        let list = view_key(view);
        let p = ui.boxed(c, r, UiStyle { gap: Some(l("head_gap")), align: Some(AlignItems::Center), ..UiStyle::row() });
        c.entity(p).insert(PickerSlot { list, id: games[*g].id.clone(), part: PickerPart::Head });
    }
}

/// A view's list key.
fn view_key(view: &PickerView) -> String {
    view.list_key.clone()
}

/// One card of the Cards look: its badge and mark (and in multi mode its
/// box) at the top, the icon (or the screen's art, `PickerPart::Art`),
/// the name, the first tags (or the screen's foot, `PickerFrame::foot`),
/// a download's bar along the bottom edge; in single mode the cursor's in
/// the accent outline (the Sandbox's rule: the keyboard cursor is the
/// card's accent outline) and the current one in the plain one, in multi
/// mode the cursor's in the plain one.
#[allow(clippy::too_many_arguments)]
fn card(c: &mut Commands, ui: &Ui, parent: Entity, view: &PickerView, i: usize, at: bool, hit: VHit, res: Option<(&mut Assets<Image>, &mut IconImages)>) {
    let t = ui.theme;
    let l = |k: &str| t.layout(L, k);
    let (games, picker, row) = (&view.games, &view.state, &view.rows[i]);
    let Some(it) = PickerState::item(games, row) else { return };
    let (_, w, h) = view.card;
    let multi = view.frame.mode == PickerMode::Multi;
    let current = !multi && picker.current.as_deref() == Some(it.id.as_str());
    let base = l(if at { "card_bg_at" } else { "card_bg" });
    let pad = l("card_pad");
    let mut style = UiStyle { w: Some(w), h: Some(h), shrink: Some(0.0), gap: Some(l("card_inner_gap")), pad: Some([pad, pad, pad, pad].into()), ..UiStyle::column() }
        .bg_a(Tone::Fg, base);
    style = match (multi, at, current) {
        (false, true, _) => style.outline(Tone::Acc, l("card_cursor")),
        (false, false, true) | (true, true, _) => style.outline(Tone::Fg2, l("card_outline")),
        _ => style.outline(Tone::Fg5, l("card_line")),
    };
    let k = c
        .spawn((
            style,
            Node { overflow: Overflow::clip(), ..default() },
            if multi { PickerAction::Toggle(i) } else { PickerAction::Pick(i) },
            PickerCard(i),
            PickerId(it.id.clone()),
            Hover { base: ui.color(Tone::Fg, base), over: ui.color(Tone::Fg, base + l("hover")) },
            clickable(),
            ChildOf(parent),
        ))
        .id();
    if !multi {
        c.entity(k).insert(hit);
    }
    if !multi && at {
        // The cursor: an accent line inside the outline.
        let sf = ui.units.scale_factor;
        let px = (l("card_cursor_px") * sf).round() / sf;
        c.spawn((
            Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, border: UiRect::all(Val::Px(px)), ..default() },
            BorderColor::all(ui.color(Tone::Acc, 1.0)),
            Pickable::IGNORE,
            ChildOf(k),
        ));
    }
    // The top: the badge (and a family's count) at the left, the mark and
    // the box (multi; the game is the section's header) at the right.
    let family = (it.family > 1).then(|| format!("{} variants", it.family));
    if multi || it.badge.is_some() || it.mark.is_some() || family.is_some() {
        let top = ui.boxed(c, k, UiStyle { justify: Some(JustifyContent::SpaceBetween), align: Some(AlignItems::Center), shrink: Some(0.0), ..UiStyle::row() });
        let words: Vec<&str> = it.badge.iter().map(String::as_str).chain(family.as_deref()).collect();
        let b = ui.text(c, top, Role::Mono, l("badge"), Tone::Fg3, &words.join(" \u{00b7} "));
        if family.is_some() {
            c.entity(b).insert(PickerFamily(it.family));
        }
        let right = ui.boxed(c, top, UiStyle { gap: Some(l("card_inner_gap")), align: Some(AlignItems::Center), ..UiStyle::row() });
        if let Some(m) = &it.mark {
            let mw = l("mark_w");
            let pm = ui.boxed(c, right, UiStyle { w: Some(mw), h: Some(mw), justify: Some(JustifyContent::Center), align: Some(AlignItems::Center), ..UiStyle::row() }.bg_a(Tone::Fg, l("mark_bg")).outline(Tone::Fg3, 1.0));
            ui.text(c, pm, Role::Mono, l("mark_size"), Tone::Fg, m);
        }
        if multi {
            tri_box(c, ui, right, picker.state(games, row));
        }
    }
    let art = ui.boxed(c, k, UiStyle { grow: Some(1.0), justify: Some(JustifyContent::Center), align: Some(AlignItems::Center), ..UiStyle::row() });
    let list = view_key(view);
    match (&it.icon, res) {
        (Some(icon), Some((images, icons))) => {
            ui.icon(c, art, images, icons, icon, h * l("card_icon"), if current { Tone::AccText } else { Tone::Fg2 });
        }
        (Some(_), None) => {}
        (None, _) => {
            c.entity(art).insert(PickerSlot { list: list.clone(), id: it.id.clone(), part: PickerPart::Art });
        }
    }
    let n = ui.text(c, k, Role::DisplayBold, l("card_name"), if current { Tone::AccText } else { Tone::Fg }, &it.name);
    c.entity(n).insert(Ellipsize::in_parent(it.name.clone()));
    if view.frame.foot {
        let f = ui.boxed(c, k, UiStyle { h: Some(l("foot_h")), gap: Some(l("card_inner_gap")), align: Some(AlignItems::Center), shrink: Some(0.0), ..UiStyle::row() });
        c.entity(f).insert(PickerSlot { list, id: it.id.clone(), part: PickerPart::Foot });
    } else {
        let tags: Vec<String> = it.tags.iter().take(2).map(|t| tag_words(t).replace('_', " ")).collect();
        if !tags.is_empty() {
            let words = tags.join(" \u{00b7} ");
            let tt = ui.text(c, k, Role::Mono, l("card_tag"), Tone::Fg2, &words);
            c.entity(tt).insert(Ellipsize::in_parent(words));
        }
    }
    // A download's bar along the bottom edge (the cursor is an outline, so
    // they never stack).
    if let Some(p) = it.progress.map(|p| f32::from(p) / 1000.0)
        && !at
    {
        let edge = c
            .spawn((
                Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, bottom: Val::ZERO, height: ui.units.val(l("edge_h")), ..default() },
                BackgroundColor(ui.color(Tone::Fg5, 1.0)),
                PickerEdge(p),
                ChildOf(k),
            ))
            .id();
        c.spawn((Node { width: Val::Percent(p.clamp(0.0, 1.0) * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(ui.color(Tone::Fg2, 1.0)), ChildOf(edge)));
    }
}

/// A family card's count (tests find it).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickerFamily(pub usize);

/// A card's download bar (its share; tests find it).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct PickerEdge(pub f32);

/// A slanted tri-state box (`box_w` x `box_h` cq).
pub fn tri_box(c: &mut Commands, ui: &Ui, parent: Entity, t: Tri) {
    let th = ui.theme;
    let l = |k: &str| th.layout(L, k);
    let u = ui.units;
    let (bw, bh) = (l("box_w"), l("box_h"));
    let lean = bh * th.tan();
    let b = c
        .spawn((
            UiStyle { w: Some(bw + lean), h: Some(bh), shrink: Some(0.0), ..UiStyle::row() },
            Node { overflow: Overflow::clip(), ..default() },
            TriBox(t),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    let full = || Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, right: Val::ZERO, ..default() };
    let px = (l("box_line") * u.phys(u.height) / 1080.0).round().max(1.0);
    match t {
        Tri::On => {
            c.spawn((full(), UiSlant::new(Tone::Fg, 1.0), ChildOf(b)));
        }
        Tri::Mixed => {
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::ZERO,
                    top: Val::ZERO,
                    bottom: Val::ZERO,
                    width: Val::Percent(l("box_mixed")),
                    ..default()
                },
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

/// The picker's systems (in the kit's plugin).
pub struct PickerPlugin;

impl Plugin for PickerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pickers>()
            .add_message::<PickerAction>()
            .add_systems(PostUpdate, bind_slots.after(crate::vlist::place_slots).before(bevy::ui::UiSystems::Prepare));
    }
}
