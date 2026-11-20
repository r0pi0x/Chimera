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
//! - **Keys** (`signal_pages::picker_keys`): the grid keys
//!   (`picker::nav`: ←→ in reading order, ↑↓ the line, on a game's header
//!   ← folds and → opens it), Space on or off, `/` the search, Enter
//!   Done, Esc Cancel.
//!
//! The search field (`/` or a click; typing filters by name, id, game or
//! class) and the chips (the classes, the games) sit over the cards; the
//! grenades a ladder may deal are cards too (class `grenade`), after a
//! game's guns.

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
        use chimera_sim::minigame::auto;
        let mut caps = auto::report(world);
        auto::drawable_all(world, &mut caps);
        caps.retain(|c| c.playable());
        games_of(world, &caps)
    }
}

/// The picker's games of a ladder's capabilities: a game's guns, then its
/// grenades (a drawn ladder's last rungs), each named as the ladder names
/// it (`GunNameRules::full`, the weapon data's HUD name) and tagged with
/// its class.
pub fn games_of(world: &World, caps: &[chimera_sim::minigame::auto::Capabilities]) -> Vec<PickerGame> {
    use chimera_core::game_parts::GunClass;
    use chimera_sim::weapon::{
        data::WeaponCatalog,
        names::{GunNameRules, id_name, is_string_token},
    };
    let empty = WeaponCatalog::default();
    let catalog = world.get_resource::<WeaponCatalog>().unwrap_or(&empty);
    let none = GunNameRules::default();
    let rules = world.get_resource::<GunNameRules>().unwrap_or(&none);
    let name = |id: &str| {
        let base = catalog.get(id).and_then(|d| d.hud.name.clone()).filter(|n| !is_string_token(n)).unwrap_or_else(|| id_name(id));
        rules.full(id, &base)
    };
    let item = |id: &str, class: &str| PickerItem {
        icon: Some(chimera_ui::IconFor::for_class(class).to_string()),
        tags: vec![format!("class:{class}")],
        ..PickerItem::new(id, name(id))
    };
    caps.iter()
        .map(|c| PickerGame {
            id: c.game.to_string(),
            name: crate::maps_picker::game_name(c.game),
            groups: vec![PickerGroup {
                name: String::new(),
                items: c.guns.iter().map(|g| item(g.id, g.class.name())).chain(c.grenades.iter().map(|g| item(g, GunClass::Grenade.name()))).collect(),
            }],
        })
        .collect()
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
    let frame = PickerFrame {
        look: PickerLook::Cards,
        mode: PickerMode::Multi,
        search: true,
        facets: vec!["class".into(), "game".into()],
        placeholder: "Find a gun, a game or a class".into(),
        ..default()
    };
    let keys = [("Space", "On / off"), ("\u{2190}\u{2192}\u{2191}\u{2193}", "Card; fold / open on a game"), ("/", "Search")];
    picker::dialog(c, ui, root, "Guns", LIST, games, &picker.0, frame, &keys, lists, GunsDialog)
}
