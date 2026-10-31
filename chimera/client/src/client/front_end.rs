//! The game's front end as the client shell sees it. A game's menus and
//! panels (CS:S's: `games::cs_source::ui`, added by the app) are a part
//! of that game; the shell (console, input, network glue, debug tools)
//! never names them. It tells them what happened through triggered
//! events (`EnteredGame`, `LeftGame`, `Joining`, `JoinFailed`,
//! `MapLoadFailed`, `StartGame`) that each front end observes, and reads what they show through `MenuStatus` and `KeyPanels`
//! (an app's own menu over them: `AppMenu`),
//! which the game's UI keeps in step with its own state (`FrontEndSync`).
//! Without a game UI the events have no observers and the resources are absent or
//! empty: the shell plays with no menus.

use std::collections::BTreeMap;

use bevy::prelude::*;

/// What the shell tells the front ends, as events triggered at the
/// moment it happens (`World::trigger`, then the observers' commands
/// applied: `front_end::entered_game` and the others). Each front end
/// (CS:S's game menu and join menus, Combat Arms' loading screen, the
/// party apps' main menu) observes those it cares about, so several can
/// follow the same moment. Without observers nothing happens.
///
/// A map is in and played (`map`, joined, a server's map change done):
/// the menu closes.
#[derive(Event, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnteredGame;

/// Out of the game (`disconnect`, dropped): the main menu.
#[derive(Event, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LeftGame;

/// A server is being joined (its address or name, then its map): the
/// loading dialog.
#[derive(Event, Clone, Debug, Default, PartialEq, Eq)]
pub struct Joining {
    pub label: String,
}

/// Joining (or the game) failed: the dialog says why
/// (`net::client::JoinProgress::failure`).
#[derive(Event, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JoinFailed;

/// A map failed to load: the loading dialog goes.
#[derive(Event, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapLoadFailed;

/// A game the local player starts (`map`, `--map`): it comes in as
/// `rules::JoinChoice` says (the game's join menus, or on a team).
#[derive(Event, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StartGame;

fn tell<'a, E: Event<Trigger<'a>: Default>>(w: &mut World, event: E) {
    w.trigger(event);
    w.flush();
}

/// Trigger `EnteredGame`.
pub fn entered_game(w: &mut World) {
    tell(w, EnteredGame)
}

/// Trigger `LeftGame`.
pub fn left_game(w: &mut World) {
    tell(w, LeftGame)
}

/// Trigger `Joining`.
pub fn joining(w: &mut World, label: &str) {
    tell(w, Joining { label: label.to_string() })
}

/// Trigger `JoinFailed`.
pub fn show_failure(w: &mut World) {
    tell(w, JoinFailed)
}

/// Trigger `MapLoadFailed`.
pub fn map_load_failed(w: &mut World) {
    tell(w, MapLoadFailed)
}

/// Trigger `StartGame`.
pub fn start_game(w: &mut World) {
    tell(w, StartGame)
}

/// An observer of front-end event `E` that runs `f` on the whole world
/// (a menu's: menus are an allowed whole-world place), at once.
pub fn on_world<E: Event>(f: fn(&mut World)) -> impl FnMut(On<E>, Commands) {
    move |_: On<E>, mut commands: Commands| commands.queue(f)
}

/// The game menu (main menu, Esc menu, its dialogs) as the shell needs to
/// know it: present only when the game has one, kept by it.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct MenuStatus {
    /// Open: it owns the mouse and keys.
    pub open: bool,
    /// A map is loaded and played (false at the main menu).
    pub in_game: bool,
    /// A map or a server is loading (the loading dialog).
    pub loading: bool,
    /// The dialog shows why joining (or the game) failed.
    pub failed: bool,
}

/// An app's own menu over the game's (the party apps' Escape menu,
/// `party_kit::esc_menu`): with this resource in, the app owns Esc (the
/// game UI's own Esc menu stays shut), and while `open` the game UI
/// counts it in `MenuStatus::open`, so the mouse and the binds are the
/// menu's as they are under the game's own.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct AppMenu {
    pub open: bool,
}

/// The game's panels open now (CS:S's `buy`, `team`, `radio` and
/// `spectator` menus), by name, each kept by its panel (`sync_panel`).
/// Those that read the number keys say so: while one is open, the slot
/// keys don't switch weapons.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Panels(pub BTreeMap<&'static str, Panel>);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Panel {
    pub open: bool,
    /// It reads the number keys.
    pub number_keys: bool,
}

impl Panels {
    /// An open panel reads the number keys.
    pub fn number_keys_taken(&self) -> bool {
        self.0.values().any(|p| p.open && p.number_keys)
    }

    pub fn is_open(&self, name: &str) -> bool {
        self.0.get(name).is_some_and(|p| p.open)
    }
}

/// Where a game UI copies its state into `MenuStatus` and `Panels`: in
/// `First`, in `Update` after its own systems, and early in `PostUpdate`;
/// the shell's `PostUpdate` readers run after it.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FrontEndSync;

/// Keep `Panels[name]` as `open` says of panel resource `R` (absent:
/// closed), in `FrontEndSync`; `after` is the panel's last system in
/// `Update`.
pub fn sync_panel<R: Resource, M>(
    app: &mut App,
    name: &'static str,
    number_keys: bool,
    open: fn(&R) -> bool,
    after: impl bevy::ecs::schedule::IntoSystemSet<M>,
) {
    let sync = move |panel: Option<Res<R>>, mut panels: ResMut<Panels>| {
        let now = Panel {
            open: panel.is_some_and(|p| open(&p)),
            number_keys,
        };
        if panels.0.get(name) != Some(&now) {
            panels.0.insert(name, now);
        }
    };
    app.init_resource::<Panels>()
        .add_systems(First, sync.in_set(FrontEndSync))
        .add_systems(Update, sync.in_set(FrontEndSync).after(after))
        .add_systems(PostUpdate, sync.in_set(FrontEndSync));
}
