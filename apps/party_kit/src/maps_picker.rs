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
//! map's children (a row one step further in, the slice's id; the same
//! row and box as a map's). A map's row sets it and its slices (off: all
//! of them off, named in `mp_maps_off`); a slice toggles alone, and can be
//! off while its map is on; a slice whose map is off shows off
//! (`is_off`), and turning it on turns its map on with it (its other
//! slices stay off). The counts are of whole maps.
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
//! - **Keys**: ↑↓ the row, Space on or off, ←→ fold or open, Enter Done (the
//!   dialog's hero), Esc or a right click Cancel; the pointer: a row's box
//!   toggles it, its chevron folds it.
//!
//! Known shortcuts: no Custom maps yet (the local listing has none); the
//! list doesn't scroll by the wheel (the keys keep the cursor in view by
//! a window of rows).

use std::collections::BTreeSet;

use bevy::prelude::*;
use chimera_ui::{
    Role, Tone, UiSlant, UiStyle,
    controls::{HERO_H, Kind, State, Ui},
};

/// One game's maps, grouped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogueGame {
    /// The id maps are named under (`cs_source`).
    pub id: String,
    pub name: String,
    pub groups: Vec<CatalogueGroup>,
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
                    });
                    out.len() - 1
                }
            };
            out[i].groups[0].maps.push(m.clone());
        }
        out
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

/// Whether `entry` is off: named, or a slice whose map is.
pub fn is_off(off: &BTreeSet<String>, entry: &str) -> bool {
    off.contains(entry) || slice_parent(entry).is_some_and(|p| off.contains(p))
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
    /// takes its slices with it; a slice turned on turns its map on (a
    /// slice is never on with its map off; the map's other slices stay as
    /// they were).
    pub fn toggle(&mut self, games: &[CatalogueGame], row: &TreeRow) {
        let maps: Vec<String> = Self::maps_of(games, row).into_iter().cloned().collect();
        let all_on = self.state(games, row) == Tri::On;
        let off = self.open.get_or_insert_with(|| self.off.clone());
        for m in maps {
            if all_on {
                off.insert(m);
            } else {
                if let Some(p) = slice_parent(&m)
                    && off.remove(p)
                {
                    // Its map back on, alone: its other slices stay off.
                    let siblings: Vec<String> = games
                        .iter()
                        .flat_map(|g| g.groups.iter().flat_map(|k| k.maps.iter()))
                        .filter(|s| slice_parent(s) == Some(p) && **s != m)
                        .cloned()
                        .collect();
                    off.extend(siblings);
                }
                off.remove(&m);
            }
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

    /// How many maps are on, of how many, in how many games (the kept set,
    /// or the working copy while open); arena slices aren't counted.
    pub fn counts(&self, games: &[CatalogueGame]) -> (usize, usize, usize) {
        let off = self.open.as_ref().unwrap_or(&self.off);
        let whole = |m: &&String| slice_parent(m).is_none();
        let all: Vec<&String> = games.iter().flat_map(|g| g.groups.iter().flat_map(|k| k.maps.iter())).filter(whole).collect();
        let on = all.iter().filter(|m| !off.contains(**m)).count();
        let games_on = games
            .iter()
            .filter(|g| g.groups.iter().flat_map(|k| k.maps.iter()).filter(whole).any(|m| !off.contains(m)))
            .count();
        (on, all.len(), games_on)
    }

    /// The Maps field's words: "41 of 138 · 3 games".
    pub fn summary(&self, games: &[CatalogueGame]) -> String {
        let (on, all, g) = self.counts(games);
        format!("{on} of {all} \u{00b7} {g} {}", if g == 1 { "game" } else { "games" })
    }

    /// `mp_maps_off`'s value.
    pub fn off_line(&self) -> String {
        self.off.iter().cloned().collect::<Vec<_>>().join(",")
    }
}

/// What a click (or a key) in the dialog does.
#[derive(Component, Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerAction {
    /// A row's box: on or off.
    Toggle(usize),
    /// A row's chevron: fold or open.
    Fold(usize),
    Done,
    Cancel,
    AllOff,
}

/// The dialog's root (tests find it).
#[derive(Component, Clone, Copy, Debug)]
pub struct MapsDialog;

/// A tree row's tri-state box (its state, for tests).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriBox(pub Tri);

/// The rows the dialog shows at once (a window the cursor keeps in view).
pub const WINDOW: usize = 17;

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
    mut res: Option<(&mut Assets<Image>, &mut chimera_ui::IconImages)>,
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
    ui.text(c, head, Role::Mono, 0.56, Tone::Fg2, &format!("{on} of {all} on \u{00b7} the rotation shuffles them"));
    // The legend.
    let legend = ui.boxed(c, plate, UiStyle { gap: Some(0.5), ..UiStyle::row() });
    for (t, words) in [(Tri::On, "On"), (Tri::Mixed, "Some on"), (Tri::Off, "Off")] {
        tri_box(c, ui, legend, t);
        let w = ui.boxed(c, legend, UiStyle { margin: Some([0.0, 0.8, 0.0, 0.0].into()), ..UiStyle::row() });
        ui.text(c, w, Role::Mono, 0.56, Tone::Fg2, words);
    }
    ui.boxed(c, legend, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    ui.text(c, legend, Role::Mono, 0.56, Tone::Fg2, "A game or group sets all under it");
    // The tree, a window of rows around the cursor.
    let tree = c
        .spawn((
            UiStyle { grow: Some(1.0), ..UiStyle::column() },
            Node { overflow: Overflow::clip(), ..default() },
            ChildOf(plate),
        ))
        .id();
    let rows = picker.rows(games);
    let first = picker.cursor.saturating_sub(WINDOW - 1).min(rows.len().saturating_sub(WINDOW));
    for (i, row) in rows.iter().enumerate().skip(first).take(WINDOW) {
        let focus = i == picker.cursor;
        // A game's or group's count: its whole maps (slices aside).
        let count_of = |row: &TreeRow| {
            let maps: Vec<&String> = MapsPicker::maps_of(games, row).into_iter().filter(|m| slice_parent(m).is_none()).collect();
            let off = picker.open.as_ref().unwrap_or(&picker.off);
            Some(format!("{} of {}", maps.iter().filter(|m| !off.contains(**m)).count(), maps.len()))
        };
        let (depth, h, name, count) = match *row {
            TreeRow::Game(g) => (0, 2.05, games[g].name.clone(), count_of(row)),
            TreeRow::Group(g, k) => (1, 2.05, games[g].groups[k].name.clone(), count_of(row)),
            // An arena slice: a child of its map, one step in, its id.
            TreeRow::Map(g, k, m) => match games[g].groups[k].maps[m].split_once('@') {
                Some((_, id)) => (3, 1.75, id.to_string(), None),
                None => (2, 1.75, games[g].groups[k].maps[m].rsplit(':').next().unwrap_or("").to_string(), None),
            },
        };
        let pad_l = [0.6, 2.0, 3.4, 4.8][depth];
        let bg = if focus { 0.06 } else if depth == 0 { 0.03 } else { 0.0 };
        let r = c
            .spawn((
                UiStyle { h: Some(h), gap: Some(0.7), pad: Some([0.0, 0.6, 0.0, pad_l].into()), shrink: Some(0.0), ..UiStyle::row() }.bg_a(Tone::Fg, bg),
                Node::default(),
                PickerAction::Toggle(i),
                chimera_ui::controls::clickable(),
                ChildOf(tree),
            ))
            .id();
        ui.hairline(c, r, Tone::Fg5, 1.0);
        if focus {
            ui.focus_bar(c, r, 0.24);
        }
        if !matches!(row, TreeRow::Map(..)) {
            let open = match *row {
                TreeRow::Game(g) => !picker.folded.contains(&games[g].id),
                TreeRow::Group(g, k) => !picker.folded.contains(&format!("{}/{}", games[g].id, games[g].groups[k].name)),
                TreeRow::Map(..) => false,
            };
            let chev = ui.boxed(c, r, UiStyle { w: Some(0.8), shrink: Some(0.0), ..UiStyle::row() });
            c.entity(chev).insert((PickerAction::Fold(i), chimera_ui::controls::clickable()));
            if let Some((images, icons)) = res.as_mut() {
                ui.icon(c, chev, images, icons, if open { "chevron" } else { "chev_right" }, 0.5, Tone::Fg2);
            }
        }
        tri_box(c, ui, r, picker.state(games, row));
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
        let (big, west) = (row_of(&p, &games, "g:big"), row_of(&p, &games, "g:big@west"));
        assert_eq!(MapsPicker::maps_of(&games, &big).len(), 3, "a map's row takes its slices");
        // A slice off while its map is on: the map shows some on.
        p.toggle(&games, &west);
        assert_eq!(p.state(&games, &west), Tri::Off);
        assert_eq!(p.state(&games, &big), Tri::Mixed);
        assert_eq!(p.counts(&games), (2, 2, 1), "whole maps counted");
        // The map on again: all of it on; off: its slices off with it.
        p.toggle(&games, &big);
        assert_eq!(p.state(&games, &big), Tri::On);
        p.toggle(&games, &big);
        let off = p.open.clone().unwrap();
        assert!(["g:big", "g:big@west", "g:big@east"].iter().all(|m| off.contains(*m)), "{off:?}");
        assert_eq!(p.state(&games, &row_of(&p, &games, "g:big@east")), Tri::Off);
        // A slice turned on brings its map on, its sibling stays off.
        p.toggle(&games, &west);
        let off = p.open.clone().unwrap();
        assert!(!off.contains("g:big") && !off.contains("g:big@west") && off.contains("g:big@east"), "{off:?}");
        // A slice whose map is off shows off even when not named.
        p.open = Some(["g:big".to_string()].into_iter().collect());
        assert_eq!(p.state(&games, &west), Tri::Off);
    }

    #[test]
    fn mp_maps_off_names_a_slice_and_round_trips() {
        let games = games();
        let mut p = MapsPicker::default();
        p.toggle(&games, &row_of(&p, &games, "g:big@east"));
        p.off = p.open.take().unwrap();
        assert_eq!(p.off_line(), "g:big@east");
        let off = chimera_net::minigame_net::MapsOff(p.off_line());
        assert!(off.has("g:big@east") && !off.has("g:big@west") && !off.has("g:big"));
        // A map off takes its slices, named or not.
        let off = chimera_net::minigame_net::MapsOff("g:big".into());
        assert!(off.has("g:big@west") && off.has("g:big@east") && !off.has("g:small"));
        // Back from the line: the same set.
        let back: BTreeSet<String> = p.off_line().split(',').map(str::to_string).collect();
        assert_eq!(back, p.off);
    }
}
