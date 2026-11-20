//! The Host page's Maps picker (the kit's design rules, sections 13, 15
//! and 16; docs/plans/active/ugg-ui.md "Signal revision 8", stage 4): the
//! Maps field (`controls::Ui::dialog_field`, the list-with-chevron icon)
//! opens a dialog over the page, behind an exclusive scrim: every game,
//! under it its groups (Official maps, Custom maps), under them the maps,
//! each with a slanted tri-state box in the progress component's language
//! (filled on, half filled when only some under it are on, a dim outline
//! off). Toggling a game or a group sets everything under it. The dialog,
//! the tree, its boxes, toggles and folds are the kit's shared picker
//! (`chimera_ui::picker`, multi-select in its List look, in
//! `picker::dialog`; docs/plans/active/shared-picker.md): this module is
//! the maps' data and the dialog's words.
//!
//! Arena slices (docs/plans/active/arena-slices.md, A2): the listing's
//! pool has each map's slices after it (`game:map@id`,
//! `chimera_sim::minigame::arena::ArenaSlices::after_maps`), shown as the
//! map's children (the picker's `PickerItem::parent`: a row one step
//! further in, the slice's own display name, `name_slices`). A map's row
//! sets it and its slices (off: the map named in `mp_maps_off`, its
//! slices off with it); a slice toggles alone, its map as it was: off
//! while its map is on (named), or on while its map is off (marked
//! `+game:map@id`, `ON_MARK`; the map's box then shows some on). The
//! counts are of every entry, maps and slices alike.
//!
//! - **Data** (`MapCatalogue`): where the listing comes from. Today
//!   `LocalCatalogue`: the minigame's pool (the installed and known maps),
//!   all of it Official. **The seam**: the asset service's listing API (by
//!   game, official or custom, other filters) will be another
//!   `MapCatalogue`, put in `MapsSource` in this one's place.
//! - **State** (`MapsPicker`, the picker's `PickerState`): the maps off
//!   (`off`, what Start sends as `mp_maps_off`, the server's rotation and
//!   vote leaving them out: `chimera_net::minigame_net::MapsOff`), and
//!   while open a working copy, the cursor and the folded games and
//!   groups.
//! - **Keys**: ↑↓ the row, Space on or off, ←→ fold or open (← on a map: its group folds, the cursor on its header), Enter Done (the
//!   dialog's hero), Esc or a right click Cancel; the pointer: a row's box
//!   (with a little slop around it) toggles it; on a game's or group's
//!   header the rest of the row (chevron, name, count) folds it, on a map
//!   the whole row toggles. Hover lifts just the area a click would act on.
//!
//! Known shortcuts: no Custom maps yet (the local listing has none); all
//! rows are one height; no search or chips in this dialog yet (the
//! picker has them; the maps' tags, size and the like, are to come).

use bevy::prelude::*;
use chimera_ui::{
    VirtualLists,
    controls::Ui,
    picker::{self, PickerFrame, PickerGame, PickerGroup, PickerItem, PickerLook, PickerMode, PickerState},
};

pub use chimera_ui::picker::{ON_MARK, PickerAction, PickerRow as TreeRow, Tri, TriBox, TriHit};

/// One game's maps, grouped (the picker's data).
pub type CatalogueGame = PickerGame;

/// Where the picker's listing comes from (module doc: the seam for the
/// asset service's listing).
pub trait MapCatalogue: Send + Sync {
    /// The games and their maps for `pool` (the minigame's own maps).
    fn listing(&self, pool: &[String]) -> Vec<CatalogueGame>;
}

/// The listing from the installed and known maps: the minigame's pool, by
/// game, all of it Official.
pub struct LocalCatalogue;

/// A map's or a slice's item: shown by its name after the game (a
/// slice: its id, until `name_slices`), a slice under its map.
pub fn map_item(entry: &str) -> PickerItem {
    let (name, parent) = match entry.split_once('@') {
        Some((map, id)) => (id.to_string(), Some(map.to_string())),
        None => (entry.rsplit(':').next().unwrap_or("").to_string(), None),
    };
    PickerItem { parent, ..PickerItem::new(entry, name) }
}

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
                        groups: vec![PickerGroup { name: "Official maps".into(), items: Vec::new() }],
                    });
                    out.len() - 1
                }
            };
            out[i].groups[0].items.push(map_item(m));
        }
        out
    }
}

/// The arena slices of `games` named by their data (`SliceDef::name`,
/// "West Base"), for their rows.
pub fn name_slices(games: &mut [CatalogueGame], slices: Option<&chimera_sim::minigame::arena::ArenaSlices>) {
    let Some(slices) = slices else { return };
    for g in games {
        for k in &mut g.groups {
            for it in k.items.iter_mut().filter(|i| i.parent.is_some()) {
                if let Ok(d) = slices.find(&it.id) {
                    it.name = d.name.clone();
                }
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

/// The picker's state (module doc): the kit's `PickerState`.
#[derive(Resource, Deref, DerefMut, Clone, Debug, Default, PartialEq, Eq)]
pub struct MapsPicker(pub PickerState);

/// An arena slice's map (`game:map@id`: `game:map`); None for a map.
pub fn slice_parent(entry: &str) -> Option<&str> {
    entry.split_once('@').map(|(m, _)| m)
}

impl MapsPicker {
    /// The Maps field's words: "41 of 138 · 3 games".
    pub fn summary(&self, games: &[CatalogueGame]) -> String {
        let (on, all, g) = self.counts(games);
        format!("{on} of {all} \u{00b7} {g} {}", if g == 1 { "game" } else { "games" })
    }

    /// The picker's set from `mp_maps_off`'s value (`off_line`'s inverse).
    pub fn off_from_line(line: &str) -> std::collections::BTreeSet<String> {
        PickerState::off_from_line(line)
    }
}

/// The dialog's root (tests find it).
#[derive(Component, Clone, Copy, Debug)]
pub struct MapsDialog;

/// The tree's list key (`VirtualLists`, `ScrollViews`, `picker::Pickers`).
pub const LIST: &str = "maps_picker";

/// Spawns the dialog over `root` (a full-screen node): the kit's picker
/// dialog (`picker::dialog`) titled Maps, its keys.
pub fn spawn(c: &mut Commands, ui: &Ui, root: Entity, picker: &MapsPicker, games: &[CatalogueGame], lists: &mut VirtualLists) -> Entity {
    let frame = PickerFrame { look: PickerLook::List, mode: PickerMode::Multi, ..default() };
    let keys = [("Space", "On / off"), ("\u{2190}\u{2192}", "Fold / open"), ("\u{2191}\u{2193}", "Row")];
    picker::dialog(c, ui, root, "Maps", LIST, games, &picker.0, frame, &keys, lists, MapsDialog)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// One game: a map with two slices, and another map.
    fn games() -> Vec<CatalogueGame> {
        let pool: Vec<String> = ["g:big", "g:big@west", "g:big@east", "g:small"].iter().map(|s| s.to_string()).collect();
        LocalCatalogue.listing(&pool)
    }

    fn row_of(p: &MapsPicker, games: &[CatalogueGame], entry: &str) -> TreeRow {
        p.rows(games).into_iter().find(|r| matches!(r, TreeRow::Item(..)) && p.ids_of(games, r)[0] == entry).unwrap()
    }

    #[test]
    fn a_maps_slices_are_its_children_and_go_off_with_it() {
        let games = games();
        let mut p = MapsPicker::default();
        assert_eq!(p.rows(&games).len(), 2 + 4, "a game, a group, two maps and two slices");
        let (big, west, east) = (row_of(&p, &games, "g:big"), row_of(&p, &games, "g:big@west"), row_of(&p, &games, "g:big@east"));
        assert_eq!(p.ids_of(&games, &big).len(), 3, "a map's row takes its slices");
        assert_eq!(p.counts(&games), (4, 4, 1));
        p.toggle(&games, &west);
        assert_eq!(p.state(&games, &west), Tri::Off);
        assert_eq!(p.state(&games, &big), Tri::Mixed);
        assert_eq!(p.state(&games, &TreeRow::Game(0)), Tri::Mixed, "the game's box agrees with its count");
        assert_eq!(p.counts(&games), (3, 4, 1));
        p.toggle(&games, &big);
        assert_eq!(p.state(&games, &big), Tri::On);
        p.toggle(&games, &big);
        assert_eq!(p.open.clone().unwrap(), BTreeSet::from(["g:big".to_string()]), "the map named, its slices with it");
        assert_eq!(p.state(&games, &east), Tri::Off);
        assert_eq!(p.counts(&games), (1, 4, 1));
        p.toggle(&games, &west);
        assert_eq!(p.open.clone().unwrap(), BTreeSet::from(["g:big".to_string(), "+g:big@west".to_string()]));
        assert_eq!((p.state(&games, &west), p.state(&games, &east)), (Tri::On, Tri::Off));
        assert_eq!(p.state(&games, &big), Tri::Mixed);
        assert_eq!(p.counts(&games), (2, 4, 1));
        p.toggle(&games, &west);
        assert_eq!(p.open.clone().unwrap(), BTreeSet::from(["g:big".to_string()]));
    }

    #[test]
    fn mp_maps_off_round_trips_a_slice_on_under_a_map_off() {
        let games = games();
        let mut p = MapsPicker::default();
        let big = row_of(&p, &games, "g:big");
        p.toggle(&games, &big);
        let east = row_of(&p, &games, "g:big@east");
        p.toggle(&games, &east);
        p.off = p.open.take().unwrap();
        assert_eq!(p.off_line(), "+g:big@east,g:big");
        let off = chimera_net::minigame_net::MapsOff(p.off_line());
        assert!(off.has("g:big") && off.has("g:big@west") && !off.has("g:big@east") && !off.has("g:small"));
        assert_eq!(MapsPicker::off_from_line(&p.off_line()), p.off, "back from the line");
        let off = chimera_net::minigame_net::MapsOff("g:big@east".into());
        assert!(off.has("g:big@east") && !off.has("g:big@west") && !off.has("g:big"));
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
        let name = |id: &str| games[0].items().find(|i| i.id == id).unwrap().name.clone();
        assert_eq!(name("g:big@west"), "West Base");
        assert_eq!(name("g:big@east"), "east", "not in the data: its id");
        assert_eq!(name("g:big"), "big");
    }
}
