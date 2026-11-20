//! The Host page's Guns picker (docs/plans/active/shared-picker.md, step
//! 2): the Guns field opens the kit's shared picker (`chimera_ui::picker`)
//! in a dialog, multi-select, as cards (fewer, smaller ones at the
//! dialog's width): every gun a drawn ladder may deal, by game, each with
//! its class icon and an on / off / some box, a game's box setting all of
//! its guns, the class chips filtering the cards. Done keeps the choice;
//! Start sends the guns off as `mp_guns_off`
//! (`chimera_sim::minigame::auto::GunsOff`: drawn ladders leave them out,
//! unless that would leave no gun).
//!
//! - **Data** (`GunCatalogue`): `LadderCatalogue`, the games a ladder
//!   draws from (`auto::report`) and their drawable guns before the
//!   host's choice (`auto::drawable_all`), named as the ladder names them
//!   (`GunNameRules::full`), tagged with their class. Listed when the
//!   dialog opens (`refresh`, into `GunsListing`).
//! - **State** (`GunsPicker`, the picker's `PickerState`): the guns off,
//!   and while open the working copy, the cursor, the folds and the chips.
//! - **Keys** as the Maps dialog's (`signal_pages::picker_keys`): ↑↓ the
//!   card or header, Space on or off, ←→ fold or open, Enter Done, Esc
//!   Cancel.
//!
//! Known shortcuts: grenades aren't listed (they stay in the draw); the
//! search field isn't shown here yet (typing is the Sandbox's first).

use bevy::prelude::*;
use chimera_ui::{
    VirtualLists,
    controls::Ui,
    picker::{self, PickerFrame, PickerGame, PickerGroup, PickerItem, PickerLook, PickerMode, PickerState},
};

/// Where the Guns picker's listing comes from.
pub trait GunCatalogue: Send + Sync {
    fn listing(&self, world: &World) -> Vec<PickerGame>;
}

/// The guns drawn ladders may deal, by game (module doc).
pub struct LadderCatalogue;

impl GunCatalogue for LadderCatalogue {
    fn listing(&self, world: &World) -> Vec<PickerGame> {
        use chimera_sim::{
            minigame::auto,
            weapon::{
                data::WeaponCatalog,
                names::{GunNameRules, id_name, is_string_token},
            },
        };
        let mut caps = auto::report(world);
        auto::drawable_all(world, &mut caps);
        let empty = WeaponCatalog::default();
        let catalog = world.get_resource::<WeaponCatalog>().unwrap_or(&empty);
        let none = GunNameRules::default();
        let rules = world.get_resource::<GunNameRules>().unwrap_or(&none);
        caps.iter()
            .filter(|c| c.playable())
            .map(|c| PickerGame {
                id: c.game.to_string(),
                name: crate::maps_picker::game_name(c.game),
                groups: vec![PickerGroup {
                    name: String::new(),
                    items: c
                        .guns
                        .iter()
                        .map(|g| {
                            let base = catalog
                                .get(g.id)
                                .and_then(|d| d.hud.name.clone())
                                .filter(|n| !is_string_token(n))
                                .unwrap_or_else(|| id_name(g.id));
                            PickerItem {
                                icon: Some(chimera_ui::IconFor::for_class(g.class.name()).to_string()),
                                tags: vec![format!("class:{}", g.class.name())],
                                ..PickerItem::new(g.id, rules.full(g.id, &base))
                            }
                        })
                        .collect(),
                }],
            })
            .collect()
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

/// The listing as last made (`refresh`).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct GunsListing(pub Vec<PickerGame>);

/// The picker's state: the kit's `PickerState`.
#[derive(Resource, Deref, DerefMut, Clone, Debug, Default, PartialEq, Eq)]
pub struct GunsPicker(pub PickerState);

impl GunsPicker {
    /// The Guns field's words: "All guns", "52 of 60 · 4 games".
    pub fn summary(&self, games: &[PickerGame]) -> String {
        if games.is_empty() {
            return if self.off.is_empty() { "All guns".into() } else { format!("{} off", self.off.len()) };
        }
        let (on, all, g) = self.counts(games);
        if on == all {
            return "All guns".into();
        }
        format!("{on} of {all} \u{00b7} {g} {}", if g == 1 { "game" } else { "games" })
    }
}

/// Lists the guns again (`GunsSource` into `GunsListing`).
pub fn refresh(world: &mut World) {
    world.init_resource::<GunsSource>();
    let games = world.resource_scope::<GunsSource, _>(|w, s| s.0.listing(w));
    world.insert_resource(GunsListing(games));
}

/// The dialog's root (tests find it).
#[derive(Component, Clone, Copy, Debug)]
pub struct GunsDialog;

/// The cards' list key (`VirtualLists`, `picker::Pickers`).
pub const LIST: &str = "guns_picker";

/// Spawns the dialog over `root`: the kit's picker dialog titled Guns,
/// cards with the class chips.
pub fn spawn(c: &mut Commands, ui: &Ui, root: Entity, picker: &GunsPicker, games: &[PickerGame], lists: &mut VirtualLists) -> Entity {
    let frame = PickerFrame { look: PickerLook::Cards, mode: PickerMode::Multi, facets: vec!["class".into()], ..default() };
    let keys = [("Space", "On / off"), ("\u{2190}\u{2192}", "Fold / open"), ("\u{2191}\u{2193}", "Card")];
    picker::dialog(c, ui, root, "Guns", LIST, games, &picker.0, frame, &keys, lists, GunsDialog)
}
