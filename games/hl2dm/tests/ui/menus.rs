//! HL2:DM's Escape and main menus (GameUI's, `source_gameui`, with
//! `hl2dm::ui::MENU_GAME`) as a player meets them: Esc opens the menu and
//! frees the cursor, the game's keys then do nothing; Resume gives both
//! back; Disconnect leaves for the main menu, which stays open. With real
//! key presses through the menu's and the shell's own systems, no window.

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use chimera_client::client::{
    binds,
    console::{self, ConsoleRoot, ConsoleUi},
    front_end,
    input::{FreeLook, MouseSettings, WheelJump, write_local_intent},
    view::FreeCam,
};
use chimera_core::{
    console::{Console, ConsoleAppExt, ConsolePlugin},
    core::{Intent, LocalPlayer},
};
use hl2dm::ui::MENU_GAME;
use source_gameui::game_menu::{self, AfterLoad, GameMenu, MainItem, Page, RegrabCursor};

struct Play {
    app: App,
    window: Entity,
    player: Entity,
}

impl Play {
    /// In a game, the mouse grabbed, W bound to `+forward`.
    fn new() -> Self {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, ConsolePlugin))
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseMotion>()
            .init_resource::<AccumulatedMouseScroll>()
            .init_resource::<ConsoleUi>()
            .init_resource::<WheelJump>()
            .init_resource::<FreeLook>()
            .init_resource::<FreeCam>()
            .init_resource::<MouseSettings>()
            .init_resource::<AfterLoad>()
            .init_resource::<RegrabCursor>()
            .insert_resource(GameMenu { in_game: true, ..GameMenu::for_game(&MENU_GAME) })
            .add_systems(Update, (game_menu::keys.before(console::toggle), console::toggle, game_menu::cursor, write_local_intent).chain())
            // What the shell's `disconnect` ends with.
            .console_command("disconnect", "", |w, _| {
                front_end::left_game(w);
                Ok(None)
            })
            .add_observer(front_end::on_world::<front_end::LeftGame>(game_menu::left_game));
        binds::test_binds(&mut app, &hl2dm::game::INFO);
        app.world_mut().resource_mut::<Console>().binds.insert("w".into(), "+forward".into());
        app.world_mut().spawn((ConsoleRoot, Visibility::Hidden));
        let window = app.world_mut().spawn(CursorOptions { grab_mode: CursorGrabMode::Locked, ..default() }).id();
        let player = app.world_mut().spawn((Intent::default(), LocalPlayer)).id();
        app.update();
        Self { app, window, player }
    }

    fn tap(&mut self, key: KeyCode) {
        self.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
        self.app.update();
        let mut keys = self.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(key);
        keys.clear();
        chimera_core::console::run_queue(self.app.world_mut());
        self.app.update();
    }

    fn hold_forward(&mut self, held: bool) {
        let mut keys = self.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        if held {
            keys.press(KeyCode::KeyW);
        } else {
            keys.release(KeyCode::KeyW);
        }
    }

    fn walking(&self) -> bool {
        self.app.world().get::<Intent>(self.player).unwrap().move_axis.y > 0.0
    }

    fn grabbed(&self) -> bool {
        self.app.world().get::<CursorOptions>(self.window).unwrap().grab_mode != CursorGrabMode::None
    }

    fn menu(&self) -> &GameMenu {
        self.app.world().resource::<GameMenu>()
    }

    fn items(&self) -> Vec<MainItem> {
        self.menu().entries().iter().map(|e| e.item).collect()
    }

    /// Press a left-hand entry as Enter does.
    fn press(&mut self, item: MainItem) {
        let row = self.items().iter().position(|i| *i == item).unwrap_or_else(|| panic!("no {item:?}"));
        self.app.world_mut().resource_mut::<GameMenu>().focus = row;
        self.tap(KeyCode::Enter);
    }
}

#[test]
fn escape_opens_resume_gives_the_game_back() {
    let mut p = Play::new();
    p.hold_forward(true);
    p.app.update();
    assert!(p.walking(), "playing: W walks");
    assert!(p.grabbed());

    // Esc: the menu, the cursor free, the game's keys idle.
    p.tap(KeyCode::Escape);
    assert!(p.menu().open && p.menu().in_game);
    assert_eq!(p.menu().page, Page::Main);
    assert!(!p.grabbed(), "Esc is more than a free cursor: the menu is up");
    assert!(!p.walking(), "the held W does nothing under the menu");
    let items = p.items();
    assert!(items.contains(&MainItem::Resume) && items.contains(&MainItem::Disconnect));
    assert!(items.contains(&MainItem::Options) && items.contains(&MainItem::Quit));

    // Resume: closed, grabbed again, W walks.
    p.press(MainItem::Resume);
    assert!(!p.menu().open);
    p.app.update();
    assert!(p.grabbed(), "Resume grabs the mouse");
    assert!(p.walking(), "and the game has the keys");

    // Esc again closes it, as Resume does.
    p.tap(KeyCode::Escape);
    assert!(p.menu().open);
    p.tap(KeyCode::Escape);
    assert!(!p.menu().open && p.grabbed());
}

#[test]
fn disconnect_leaves_for_the_main_menu() {
    let mut p = Play::new();
    p.tap(KeyCode::Escape);
    p.press(MainItem::Disconnect);
    // The console line ran; the shell said the game is left.
    p.app.update();
    assert!(!p.menu().in_game, "out of the game");
    assert!(p.menu().open, "at the main menu");
    assert!(!p.grabbed());
    let items = p.items();
    assert!(!items.contains(&MainItem::Resume) && !items.contains(&MainItem::Disconnect), "{items:?}");
    assert!(items.contains(&MainItem::FindServers) && items.contains(&MainItem::NewGame));

    // Esc doesn't close the main menu.
    p.tap(KeyCode::Escape);
    assert!(p.menu().open, "the main menu stays");
}

/// HL2:DM's Create Server: no bots, no rounds, no team dialog: only the
/// map and the install's own server options.
#[test]
fn a_new_game_has_nothing_of_cs_sources() {
    let mut m = GameMenu::for_game(&MENU_GAME);
    m.open(Page::Main, vec!["dm_lockdown".into(), "dm_overwatch".into()], None, |_| None);
    assert_eq!(m.create_tabs().len(), 2, "Server and Game: no Bot page");
    m.new_game.map = 1;
    let (lines, after) = m.start_lines().unwrap();
    assert!(after.is_empty(), "no bots to add: {after:?}");
    assert!(lines.iter().all(|l| !l.starts_with("bot_") && !l.starts_with("chimera_rounds")), "{lines:?}");
    assert_eq!(lines.last().map(String::as_str), Some("map dm_overwatch"));
    assert!(lines.iter().any(|l| l.starts_with("maxplayers ")), "Create Server hosts: {lines:?}");
}
