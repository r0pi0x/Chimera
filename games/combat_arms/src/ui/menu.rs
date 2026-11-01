//! Combat Arms' in-game menu (Esc; specs/combat_arms/ui_system.md H, M2,
//! S1, I1-I3), in the shell's place (without a game menu the shell's Esc
//! only frees the mouse): a popup read from the game's own layout when
//! the install's can be read (`ui::install::MENU_FILE`: its buttons bound
//! by name, `INSTALL_BUTTONS`, its art and strings the install's), else
//! from ours (`data/ui/ingame_menu.xml`, the game's format), drawn 1:1 in screen px
//! with its first control centred on the screen (⌊W/2⌋ − ⌊w/2⌋), modal
//! (`desk::Desk`'s capture: nothing under it takes a click), its buttons
//! firing on the left button going down with their sounds.
//!
//! - Esc opens and closes it (not while the console is open or chat is
//!   being typed: those take the Esc; never in an app with a menu of its
//!   own, `front_end::AppMenu`, which also counts in `MenuStatus::open`); every other key is swallowed
//!   while it is open. The game doesn't pause (no pause online), the
//!   mouse shows while it is open (`MenuStatus::open`, which the shell
//!   reads: binds and the mouse grab wait), and is taken back when it
//!   closes.
//! - Under it, or with the chat input open (I2, Corrections 4,
//!   `popup_input`): the local player's movement and command flags are
//!   cleared each frame but crouch (a crouch held when it opened stays
//!   down; a sprint ends), its view stays where it was (mouse look
//!   ignored); the world keeps running.
//! - Buttons (the install's, by name, Corrections 5): return to lobby,
//!   report a user, start/stop recording (its label `btn_text_record`
//!   the string `$button_record_start` with the record key's name),
//!   cancel and the title bar's close button (both close it); a button
//!   the layout has but code doesn't bind (the options button) keeps its
//!   normal look and does nothing.
//!
//! `CaMenuStatePlugin` is the menu without a window (`InGameMenu`, the
//! pointer as `MenuPointer`; tests/ui/ui_system.rs); `CaMenuPlugin` adds
//! the window's pointer, the cursor and the drawing. `ingamemenu` (the
//! console) toggles it, for screenshots.
//!
//! Known shortcuts (the spec's most literal reading where it is unclear):
//! - "Return to lobby" quits the game (there is no lobby yet), so our
//!   label says "Leave Game" (the game's layout keeps its own words);
//!   reporting a user and recording have no flow, so their buttons are
//!   disabled (the D look), and no console command records (the label
//!   names the key bound to `record`, none by default).
//! - Interface sounds are counted (`InGameMenu::played`), not played: the
//!   sound files' roles are in the client interface attributes (keyed).
//! - The popup's root takes the pointer over the whole screen (the file
//!   root's 1024 x 1024 is used for alignment only), so a centred popup
//!   past x 1024 still takes clicks.
//! - The other in-game popups (the user report box, message boxes, the
//!   user tooltip, the discard confirmation, the help box) don't exist
//!   yet: only the Esc menu and the chat input stop the player.

use bevy::{prelude::*, window::PrimaryWindow};

use super::{
    CaUi,
    desk::{self, Desk, Event},
    draw::{self, CaFaces},
    layout::{Layout, Sound, xml},
};
use crate::client::{
    fonts::UiFonts,
    front_end::{FrontEndSync, MenuStatus},
};
use crate::console::{ConsoleAppExt, Console};

/// The menu's layout (ours, the game's format).
pub const LAYOUT: &str = include_str!("../../data/ui/ingame_menu.xml");
/// Its first control: the box centred on the screen.
pub const BOX: &str = "InGameMenu";
/// The buttons' command ids (the layout's Cmd_id).
pub const CMD_LOBBY: i32 = 1;
pub const CMD_REPORT: i32 = 2;
pub const CMD_RECORD: i32 = 3;
pub const CMD_CANCEL: i32 = 4;
/// The command of a button code doesn't bind: none (it keeps its
/// normal look and does nothing; ui_system.md Corrections 5).
pub const CMD_NONE: i32 = 0;
/// The game's layout's first control (`install::MENU_FILE`).
pub const INSTALL_BOX: &str = "exit_popup";
/// The game's layout's buttons by name, their commands (its `Cmd_id`s
/// aren't the commands: code binds the buttons by name, ui_system.md D),
/// and whether they work here.
pub const INSTALL_BUTTONS: &[(&str, i32, bool)] = &[
    ("button_lobby", CMD_LOBBY, true),
    ("button_userreport", CMD_REPORT, false),
    ("button_record", CMD_RECORD, false),
    ("button_cancel", CMD_CANCEL, true),
    ("button_exit_title", CMD_CANCEL, true),
];
/// The record button's label (`btn_text_record`) and the console command
/// whose key it names.
pub const RECORD_LABEL: &str = "btn_text_record";
pub const RECORD_COMMAND: &str = "record";

/// The record button's label (ui_system.md Corrections 5): the string
/// `$button_record_start` (`$button_record_stop` while recording) filled
/// with the name of the key bound to recording.
pub fn record_label(strings: &super::layout::Strings, key: &str, recording: bool) -> String {
    let id = if recording { "$button_record_stop" } else { "$button_record_start" };
    super::hud::model::fill(&strings.resolve(id), key)
}

/// The in-game menu: its popup on a desk, open or not, the screen it is
/// centred on, and the interface sounds it has played.
#[derive(Resource, Clone, Debug)]
pub struct InGameMenu {
    pub desk: Desk,
    pub sheet: usize,
    pub open: bool,
    pub screen: IVec2,
    pub played: Vec<Sound>,
    /// Buttons fired, by name (for tests and the log).
    pub fired: Vec<String>,
    /// The control centred on the screen.
    pub box_name: &'static str,
}

impl InGameMenu {
    pub fn new(ui: &CaUi) -> Self {
        let install = ui.install.as_ref().and_then(|i| i.menu.clone());
        let installed = install.is_some();
        let mut layout = install.unwrap_or_else(|| xml::read(LAYOUT).expect("data/ui/ingame_menu.xml"));
        layout.check_fonts(&ui.fonts).expect("the in-game menu's fonts");
        let mut disabled: Vec<&str> = vec!["Report", "Record"];
        if installed {
            disabled.clear();
            // Every button's command is code's: a button the layout has
            // but code doesn't bind (the options button) does nothing.
            for id in layout.tree.order() {
                if matches!(layout.widget(id).kind, super::layout::Kind::Button(_)) {
                    layout.widget_mut(id).cmd = CMD_NONE;
                }
            }
            for &(name, cmd, works) in INSTALL_BUTTONS {
                if let Some(id) = layout.find(name) {
                    layout.widget_mut(id).cmd = cmd;
                    if !works {
                        disabled.push(name);
                    }
                }
            }
            if let Some(id) = layout.find(RECORD_LABEL)
                && let super::layout::Kind::Text(t) = &mut layout.widget_mut(id).kind
            {
                t.set = Some(record_label(&ui.strings, "", false));
            }
        }
        let mut desk = Desk::default();
        let sheet = desk.add("ingame_menu", layout, true);
        let mut menu = Self {
            desk,
            sheet,
            open: false,
            screen: IVec2::new(1024, 768),
            played: Vec::new(),
            fired: Vec::new(),
            box_name: if installed { INSTALL_BOX } else { BOX },
        };
        for name in disabled {
            if let Some(id) = menu.layout().find(name) {
                menu.layout_mut().widget_mut(id).enabled = false;
            }
        }
        menu.place();
        menu
    }

    pub fn layout(&self) -> &Layout {
        &self.desk.sheets[self.sheet].layout
    }

    pub fn layout_mut(&mut self) -> &mut Layout {
        &mut self.desk.sheets[self.sheet].layout
    }

    /// Centre the first control on the screen, 1:1.
    pub fn place(&mut self) {
        let (screen, name) = (self.screen, self.box_name);
        let l = self.layout_mut();
        if let Some(id) = l.find(name) {
            l.centre(id, screen);
        }
    }

    /// Open or close it.
    pub fn set_open(&mut self, open: bool) {
        if open == self.open {
            return;
        }
        self.open = open;
        if open {
            self.place();
            self.desk.show(self.sheet, true);
        } else {
            self.desk.hide(self.sheet);
        }
    }
}

/// The pointer in screen px (None: outside the window).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct MenuPointer(pub Option<IVec2>);

/// The menu's systems.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MenuSystems;

/// The local player's input under a popup or the chat input (after the
/// shell writes it).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PopupInput;

/// While the Esc menu or the chat input is open (ui_system.md I2,
/// Corrections 4): every movement and command flag cleared but crouch (a
/// crouch held when it opened stays down; sprint ends), the view kept
/// where it was (mouse look ignored); the world keeps running. `last` is
/// the input of the last frame it was closed.
fn popup_input(
    menu: Res<InGameMenu>,
    options: Option<Res<crate::game::options::CaOptions>>,
    chat: Option<Res<crate::client::chat::ChatInput>>,
    talk: Option<Res<crate::game::chat::CaChat>>,
    lobby: Option<Res<super::lobby::LobbyScreen>>,
    mut player: Query<&mut crate::core::Intent, With<crate::core::LocalPlayer>>,
    mut last: Local<Option<(bool, f32, f32)>>,
    mut held: Local<Option<(bool, f32, f32)>>,
) {
    let Ok(mut intent) = player.single_mut() else { return };
    let open = menu.open
        || options.is_some_and(|o| o.open)
        || chat.is_some_and(|c| c.open.is_some())
        || talk.is_some_and(|c| c.is_open())
        || lobby.is_some_and(|l| l.open);
    if !open {
        *held = None;
        *last = Some((intent.crouch, intent.yaw, intent.pitch));
        return;
    }
    let (crouch, yaw, pitch) = *held.get_or_insert(last.unwrap_or((intent.crouch, intent.yaw, intent.pitch)));
    let i = &mut *intent;
    *i = crate::core::Intent {
        crouch,
        yaw,
        pitch,
        command: i.command,
        ..default()
    };
}

/// The record button's label names the key bound to recording (when the
/// menu opens).
fn record_key(mut menu: ResMut<InGameMenu>, ui: Res<CaUi>, console: Option<Res<Console>>) {
    if !menu.is_changed() || !menu.open {
        return;
    }
    let key = console
        .as_deref()
        .and_then(|c| crate::client::binds::keys_for(&c.binds, RECORD_COMMAND).first().map(|k| k.to_uppercase()))
        .unwrap_or_default();
    let label = record_label(&ui.strings, &key, false);
    let m = menu.bypass_change_detection();
    if let Some(id) = m.layout().find(RECORD_LABEL)
        && let super::layout::Kind::Text(t) = &mut m.layout_mut().widget_mut(id).kind
    {
        t.set = Some(label);
    }
}

/// The menu without a window.
pub struct CaMenuStatePlugin;

impl Plugin for CaMenuStatePlugin {
    fn build(&self, app: &mut App) {
        let ui = app.world().get_resource::<CaUi>().cloned().unwrap_or_default();
        app.insert_resource(InGameMenu::new(&ui))
            .insert_resource(ui)
            .init_resource::<MenuPointer>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_systems(Update, (keys, pointer, act, record_key).chain().in_set(MenuSystems))
            .add_systems(Update, popup_input.in_set(PopupInput).after(MenuSystems))
            .console_command("combat_arms:ingamemenu", "Open or close Combat Arms' in-game menu (Esc).", |w, _| {
                let mut m = w.resource_mut::<InGameMenu>();
                let open = !m.open;
                m.set_open(open);
                Ok(None)
            });
        // The shell's view of it (`client::front_end`): no main menu, so
        // always in the game.
        app.insert_resource(MenuStatus {
            in_game: true,
            ..default()
        })
        .add_systems(First, sync_status.in_set(FrontEndSync))
        .add_systems(Update, sync_status.in_set(FrontEndSync).after(MenuSystems))
        .add_systems(PostUpdate, sync_status.in_set(FrontEndSync));
    }
}

fn sync_status(
    menu: Res<InGameMenu>,
    options: Option<Res<crate::game::options::CaOptions>>,
    lobby: Option<Res<super::lobby::LobbyScreen>>,
    app_menu: Option<Res<crate::client::front_end::AppMenu>>,
    mut status: ResMut<MenuStatus>,
) {
    let options_open = options.as_ref().is_some_and(|o| o.open);
    let lobby_open = lobby.as_ref().is_some_and(|l| l.open);
    let app_open = app_menu.as_ref().is_some_and(|a| a.open);
    if menu.is_changed()
        || options.is_some_and(|o| o.is_changed())
        || lobby.is_some_and(|l| l.is_changed())
        || app_menu.is_some_and(|a| a.is_changed())
    {
        status.set_if_neq(MenuStatus {
            open: menu.open || options_open || lobby_open || app_open,
            in_game: true,
            ..default()
        });
    }
}

/// Esc opens and closes it, unless the console or chat takes the key.
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    console: Option<Res<crate::client::console::ConsoleUi>>,
    chat: Option<Res<crate::client::chat::ChatInput>>,
    options: Option<Res<crate::game::options::CaOptions>>,
    talk: Option<Res<crate::game::chat::CaChat>>,
    lobby: Option<Res<super::lobby::LobbyScreen>>,
    app_menu: Option<Res<crate::client::front_end::AppMenu>>,
    mut menu: ResMut<InGameMenu>,
) {
    // An app's own menu takes Esc (the party apps' Escape menu).
    if app_menu.is_some() {
        return;
    }
    let busy = console.is_some_and(|c| c.open)
        || lobby.is_some_and(|l| l.open)
        || chat.is_some_and(|c| c.open.is_some())
        || options.is_some_and(|o| o.open)
        || talk.is_some_and(|c| c.is_open());
    if keys.just_pressed(KeyCode::Escape) && !busy {
        let open = !menu.open;
        menu.set_open(open);
    }
}

/// Each widget's hover and held states.
fn states(l: &Layout) -> Vec<(bool, bool)> {
    l.tree
        .order()
        .into_iter()
        .map(|i| (l.widget(i).hover, l.widget(i).held))
        .collect()
}

/// The pointer: hover, and the left button down and up.
fn pointer(pointer: Res<MenuPointer>, mouse: Res<ButtonInput<MouseButton>>, ui: Res<CaUi>, mut menu: ResMut<InGameMenu>) {
    if !menu.open {
        return;
    }
    let Some(p) = pointer.0 else { return };
    let ctx = ui.ctx();
    let m = menu.bypass_change_detection();
    let before = states(m.layout());
    m.desk.pointer_moved(p, &ctx);
    if mouse.just_pressed(MouseButton::Left) {
        m.desk.left_down(p, &ctx);
    }
    if mouse.just_released(MouseButton::Left) {
        m.desk.left_up();
    }
    let after = states(m.layout());
    if before != after || !m.desk.events.is_empty() {
        menu.set_changed();
    }
}

/// What the buttons do; their sounds counted.
fn act(
    mut menu: ResMut<InGameMenu>,
    console: Option<ResMut<Console>>,
    lobby: Option<ResMut<crate::game::lobby::CaLobbyClient>>,
) {
    let mut lobby = lobby;
    if menu.desk.events.is_empty() {
        return;
    }
    let events = menu.desk.take_events();
    menu.played.extend(desk::sounds(&events));
    let mut console = console;
    for e in events {
        let Event::Fired { name, cmd, .. } = e else { continue };
        menu.fired.push(name);
        match cmd {
            CMD_CANCEL => menu.set_open(false),
            CMD_LOBBY => {
                menu.set_open(false);
                // With a lobby, EXIT returns to the room (X); without, the
                // game quits.
                match lobby.as_mut().filter(|l| l.0.active && l.0.room.is_some()) {
                    Some(l) => l.0.leave_match(),
                    None => {
                        if let Some(c) = console.as_mut() {
                            c.submit("quit");
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// The menu with the window: its pointer, the cursor, the drawing.
pub struct CaMenuPlugin;

impl Plugin for CaMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CaMenuStatePlugin)
            .add_systems(Startup, spawn)
            // Before the console's toggle: the Esc that closes the console
            // doesn't open the menu.
            .configure_sets(Update, MenuSystems.before(crate::client::console::toggle))
            .configure_sets(Update, PopupInput.after(crate::client::input::write_local_intent))
            .add_systems(Update, window_pointer.before(MenuSystems))
            .add_systems(Update, (cursor, draw_menu).after(MenuSystems));
    }
}

/// The window's size and pointer (logical px, as the UI lays out).
fn window_pointer(
    window: Query<&Window, With<PrimaryWindow>>,
    mut pointer: ResMut<MenuPointer>,
    mut menu: ResMut<InGameMenu>,
) {
    let Ok(w) = window.single() else { return };
    let screen = IVec2::new(w.resolution.width() as i32, w.resolution.height() as i32);
    if screen.x > 0 && screen.y > 0 && menu.screen != screen {
        menu.screen = screen;
        menu.place();
    }
    pointer.set_if_neq(MenuPointer(w.cursor_position().map(|p| p.floor().as_ivec2())));
}

/// The mouse shows while it is open and is taken back when it closes
/// (once the button is up, unless the console is open).
fn cursor(
    menu: Res<InGameMenu>,
    options: Option<Res<crate::game::options::CaOptions>>,
    lobby: Option<Res<super::lobby::LobbyScreen>>,
    mouse: Res<ButtonInput<MouseButton>>,
    ui: Option<Res<crate::client::console::ConsoleUi>>,
    cursor: Option<Single<&mut bevy::window::CursorOptions>>,
    mut regrab: Local<bool>,
) {
    let Some(mut cursor) = cursor else { return };
    if menu.open || options.is_some_and(|o| o.open) || lobby.is_some_and(|l| l.open) {
        if crate::client::input::cursor_grabbed(&cursor) {
            crate::client::input::release_cursor(&mut cursor);
        }
        *regrab = true;
    } else if *regrab && !mouse.pressed(MouseButton::Left) {
        *regrab = false;
        if !ui.is_some_and(|u| u.open) {
            crate::client::input::capture_cursor(&mut cursor);
        }
    }
}

#[derive(Component)]
struct MenuRoot;

fn spawn(mut commands: Commands) {
    commands.spawn((
        MenuRoot,
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0),
            height: percent(100.0),
            ..default()
        },
        // Over the HUD.
        GlobalZIndex(60),
        Visibility::Hidden,
    ));
}

fn draw_menu(
    menu: Res<InGameMenu>,
    ui: Res<CaUi>,
    fonts: Option<Res<UiFonts>>,
    faces: Option<Res<CaFaces>>,
    art: Option<Res<draw::CaArt>>,
    root: Single<(Entity, &mut Visibility), With<MenuRoot>>,
    mut commands: Commands,
) {
    if !menu.is_changed() && !faces.as_ref().is_some_and(|f| f.is_changed()) && !art.as_ref().is_some_and(|a| a.is_changed()) {
        return;
    }
    let (root, mut vis) = root.into_inner();
    *vis = if menu.open { Visibility::Visible } else { Visibility::Hidden };
    commands.entity(root).despawn_related::<Children>();
    if !menu.open {
        return;
    }
    let draws = menu.layout().draw(&ui.ctx());
    commands.entity(root).with_children(|p| {
        draw::spawn_draws(p, &draws, &ui.stand_ins, art.as_deref(), fonts.as_deref(), faces.as_deref());
    });
}
