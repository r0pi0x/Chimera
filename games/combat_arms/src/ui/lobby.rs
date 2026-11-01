//! Combat Arms' lobby screens (specs/combat_arms/lobby.md F, N, L, C, R, O,
//! T, S, E): the room list, the room creation popup, the waiting room and
//! the inventory ("My Character"), on the UI system's layouts. They are a
//! view of the client's lobby (`game::lobby::CaLobbyClient`): each frame
//! the controls are set from it, and a click or a key calls the client's
//! own functions (`ClientLobby::join`, `change_team`, `f5`, `change` ...)
//! whose requests `game::lobby::host` sends.
//!
//! - The screen opens when the lobby answers (a server with the lobby on)
//!   and the player is not in a match; it closes at "go" and opens again
//!   when the match is over or the player leaves it. Esc's menu, in a
//!   match, returns to the room (X).
//! - Pages: the room list (`gameLobby`), the waiting room (`gameroom`),
//!   the inventory (`inventory`); popups on top (modal): the creation
//!   popup (`CreateWnd`), the password box (`LockGameRoom`), a message
//!   box (`MessageBox`).
//! - Drop-downs are cycled by a click on their button, as the options
//!   screen's; edit boxes take the typed characters when clicked.
//! - The inventory edits a `Loadout` (both sets); every change goes to the
//!   gear's console settings (`CaOwnLook`, `CaOwnGear`) and to the lobby
//!   (`Request::Loadout`).
//!
//! `CaLobbyStatePlugin` is the screen without a window (tests/ui/lobby.rs);
//! `CaLobbyScreenPlugin` adds the drawing. The `lobby` console command
//! opens the lobby on this process (`ca_lobby 1`).
//!
//! Known shortcuts (docs/tech-debt.md):
//! - The install's layouts (GAMELOBBY.XML, POPUP_CREATEROOM.XML,
//!   GAMEROOM.XML, INVENTORY.XML ...) are not read: the screens are ours,
//!   in the game's layout format with the spec's control names and flat
//!   stand-in art, with or without the install. The tooltips, the hover
//!   panel, the user menu (right click), the shop, clan and jobs screens,
//!   the server select, the connecting screen, the quick-join options
//!   are not there (the Start menu has the option entry only); Change Room Name and Change PW
//!   are not (the service takes the requests, `Setting::Name`).
//! - The list has 18 rows and no scroll bar; combo boxes are cycled; the
//!   mode filter cycles through the modes (no Pure Combat entries);
//!   attachments are not edited (the set keeps whatever it holds).
//! - Edit boxes take the service's limits (name 32, password 8), not the
//!   layouts' MaxLength (open question 4).
//! - The room chat is a plain input line (the match chat's bar, `talk`,
//!   its radio and whisper commands, is not used in the lobby); the
//!   countdown sound is not played.

pub mod layouts;

use std::sync::Arc;

use chimera_client::client::dialog::{Button, Dialog, Outcome, Press};

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    window::PrimaryWindow,
};

use self::layouts::{CHAT_LINES, ITEM_ROWS, LIST_ROWS, TEAM_ROWS};
use super::{
    CaUi,
    desk::{self, Desk, Event},
    draw::{self, CaFaces},
    layout::{Kind as WKind, Layout, PxRect, Sound, xml},
    menu::MenuPointer,
};
use crate::{
    client::fonts::UiFonts,
    game::{
        chat,
        lobby::{
            CaLobbyClient,
            check::{self, CreateForm, Primary},
            items::{Catalog, Kind, Owned},
            list::{Column, Joining},
            loadout::{self, Loadout, SetKind, Slot},
            room::*,
        },
    },
};

/// The pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    List,
    Room,
    Inventory,
}

/// The sheets, in the desk's order.
const NAMES: [&str; 7] = ["gameLobby", "gameroom", "inventory", "CreateWnd", "LockGameRoom", "MessageBox", "StartMenu"];
const S_LIST: usize = 0;
const S_ROOM: usize = 1;
const S_INV: usize = 2;
const S_CREATE: usize = 3;
const S_PASS: usize = 4;
const S_MSG: usize = 5;
/// The start menu popup (`StartButton`, `POPUP_STARTMENU.XML`).
const S_START: usize = 6;

/// Which edit box takes the typed characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    None,
    RoomName,
    RoomPassword,
    JoinPassword,
    Chat,
}

/// A key typed.
#[derive(Clone, Debug, PartialEq)]
pub enum Typed {
    Text(String),
    Backspace,
    Enter,
    Escape,
    F5,
}

/// The nav bar's and the lists' buttons that do the same thing on every
/// page.
const SORTS: [(&str, Column); 8] = [
    ("Sort_Num", Column::Num),
    ("Sort_Title", Column::Title),
    ("Sort_Map", Column::Map),
    ("Sort_Mode", Column::Mode),
    ("Sort_Mutation", Column::Mutation),
    ("Sort_Arms", Column::Arms),
    ("Sort_Players", Column::Players),
    ("Sort_Status", Column::Status),
];

/// The inventory's slot buttons.
const SLOT_BUTTONS: [(&str, Slot); 18] = [
    ("equip_primary", Slot::Primary),
    ("equip_secondary", Slot::Secondary),
    ("equip_Melee", Slot::Melee),
    ("equip_Support", Slot::Support),
    ("equip_backpackA", Slot::Backpack(0)),
    ("equip_backpackB", Slot::Backpack(1)),
    ("equip_backpackC", Slot::Backpack(2)),
    ("equip_backpackD", Slot::Backpack(3)),
    ("equip_character", Slot::Character),
    ("equip_head", Slot::Head),
    ("equip_face", Slot::Face),
    ("equip_uniform", Slot::Uniform),
    ("equip_vest", Slot::Vest),
    ("equip_gear_backpack", Slot::BackpackGear),
    ("equip_perk0", Slot::Ability(0)),
    ("equip_perk1", Slot::Ability(1)),
    ("equip_perk2", Slot::Ability(2)),
    ("equip_perk3", Slot::Ability(3)),
];

/// The option pairs of the waiting room: name, the setting for On / Off,
/// its place in `check::option_enabled`.
const OPTIONS: [(&str, usize); 6] = [("Join", 0), ("FriendlyFire", 1), ("KillCam", 2), ("Perk", 3), ("Mutation", 4), ("TeamChange", 99)];

/// The lobby screens.
#[derive(Resource, Clone)]
pub struct LobbyScreen {
    pub desk: Desk,
    sheets: [usize; 7],
    pub open: bool,
    pub screen: IVec2,
    pub page: Page,
    pub create: Option<CreateForm>,
    /// The start menu popup is up (`StartButton`).
    pub start_menu: bool,
    pub password_room: Option<RoomId>,
    pub focus: Focus,
    pub chat_input: String,
    /// The password box for a locked room (`password_room`).
    pub password_box: Dialog,
    /// Keys typed since the last frame (the window's, or a test's).
    pub typed: Vec<Typed>,
    pub loadout: Loadout,
    pub owned: Owned,
    pub catalog: Arc<Catalog>,
    pub inv_set: SetKind,
    pub inv_slot: Slot,
    pub inv_page: usize,
    pub inv_pick: Option<i32>,
    pub inv_note: String,
    /// Buttons fired, by name; sounds played (for tests and the log).
    pub fired: Vec<String>,
    pub played: Vec<Sound>,
    pub changed: bool,
    last_click: Option<(String, f64)>,
    last_draw: Vec<super::layout::Draw>,
}

impl LobbyScreen {
    pub fn new(ui: &CaUi, catalog: Arc<Catalog>) -> Self {
        let mut desk = Desk::default();
        let texts = [
            layouts::lobby(),
            layouts::room(),
            layouts::inventory(),
            layouts::create(),
            layouts::password(),
            layouts::message(),
            layouts::start_menu(),
        ];
        let mut sheets = [0; 7];
        for (i, t) in texts.iter().enumerate() {
            let l: Layout = xml::read(t).unwrap_or_else(|e| panic!("lobby layout {}: {e:?}", NAMES[i]));
            l.check_fonts(&ui.fonts).expect("the lobby screens' fonts");
            sheets[i] = desk.add(NAMES[i], l, true);
        }
        let mut s = Self {
            desk,
            sheets,
            open: false,
            screen: IVec2::new(1024, 768),
            page: Page::List,
            create: None,
            start_menu: false,
            password_room: None,
            focus: Focus::None,
            chat_input: String::new(),
            password_box: Dialog::password("Password", "", 8),
            typed: Vec::new(),
            loadout: Loadout::new_player(&catalog),
            owned: Owned::all(),
            catalog,
            inv_set: SetKind::Normal,
            inv_slot: Slot::Primary,
            inv_page: 0,
            inv_pick: None,
            inv_note: String::new(),
            fired: Vec::new(),
            played: Vec::new(),
            changed: true,
            last_click: None,
            last_draw: Vec::new(),
        };
        s.place();
        s
    }

    /// Centre the 1024 x 768 menu on the screen (every sheet's first
    /// control moves with it).
    pub fn place(&mut self) {
        for i in 0..7 {
            let l = &mut self.desk.sheets[self.sheets[i]].layout;
            let first = l.tree.get(0).children.first().copied().unwrap_or(0);
            let base = match i {
                S_CREATE => IVec2::new(262, 160),
                S_PASS => IVec2::new(362, 300),
                S_MSG => IVec2::new(312, 300),
                S_START => IVec2::new(332, 66),
                _ => IVec2::ZERO,
            };
            let off = crate::client::canvas::centre(self.screen, IVec2::new(1024, 768));
            l.tree.get_mut(first).pos = base + off;
        }
    }

    pub fn layout(&self, i: usize) -> &Layout {
        &self.desk.sheets[self.sheets[i]].layout
    }

    /// A control by name: its sheet and id.
    ///
    /// Several sheets name the same control (the spec's names repeat
    /// across its files); the shown ones win, the popups before the pages.
    pub fn find(&self, name: &str) -> Option<(usize, usize)> {
        let shown = |i: usize| self.desk.sheets[self.sheets[i]].shown;
        [S_MSG, S_PASS, S_CREATE, S_START, S_LIST, S_ROOM, S_INV]
            .into_iter()
            .filter(|i| shown(*i))
            .chain([S_LIST, S_ROOM, S_INV, S_CREATE, S_PASS, S_MSG, S_START])
            .find_map(|i| self.layout(i).find(name).map(|id| (i, id)))
    }

    fn widget_mut(&mut self, name: &str) -> Option<&mut super::layout::Widget> {
        let (i, id) = self.find(name)?;
        Some(self.desk.sheets[self.sheets[i]].layout.widget_mut(id))
    }

    pub fn set_text(&mut self, name: &str, s: &str) {
        if let Some(w) = self.widget_mut(name)
            && let WKind::Text(t) = &mut w.kind
            && t.set.as_deref() != Some(s)
        {
            t.set = Some(s.to_string());
        }
    }

    fn set_colour(&mut self, name: &str, c: [u8; 4]) {
        if let Some(w) = self.widget_mut(name)
            && let WKind::Text(t) = &mut w.kind
        {
            t.colour = c;
        }
    }

    /// A button's label (its `_text` child).
    pub fn set_label(&mut self, name: &str, s: &str) {
        self.set_text(&format!("{name}_text"), s);
    }

    pub fn text_of(&self, name: &str, ui: &CaUi) -> Option<String> {
        let (i, id) = self.find(name)?;
        let l = self.layout(i);
        l.widget(id).text().map(|_| l.string_of(id, &ui.strings))
    }

    pub fn label_of(&self, name: &str, ui: &CaUi) -> Option<String> {
        self.text_of(&format!("{name}_text"), ui)
    }

    pub fn set_selected(&mut self, name: &str, on: bool) {
        if let Some(w) = self.widget_mut(name) {
            w.selected = on;
        }
    }

    pub fn set_enabled(&mut self, name: &str, on: bool) {
        if let Some(w) = self.widget_mut(name) {
            w.enabled = on;
        }
    }

    pub fn enabled(&self, name: &str) -> Option<bool> {
        let (i, id) = self.find(name)?;
        Some(self.layout(i).widget(id).enabled)
    }

    pub fn selected(&self, name: &str) -> Option<bool> {
        let (i, id) = self.find(name)?;
        Some(self.layout(i).widget(id).selected)
    }

    pub fn set_shown(&mut self, name: &str, on: bool) {
        if let Some((i, id)) = self.find(name) {
            self.desk.sheets[self.sheets[i]].layout.tree.get_mut(id).show = on;
        }
    }

    pub fn shown(&self, name: &str) -> Option<bool> {
        let (i, id) = self.find(name)?;
        Some(self.layout(i).tree.get(id).show)
    }

    pub fn rect_of(&self, name: &str, ui: &CaUi) -> Option<PxRect> {
        let (i, id) = self.find(name)?;
        Some(self.layout(i).hit_rect(id, &ui.ctx()))
    }

    /// The page's sheet.
    fn page_sheet(&self) -> usize {
        match self.page {
            Page::List => S_LIST,
            Page::Room => S_ROOM,
            Page::Inventory => S_INV,
        }
    }

    /// Which popup is up, if any.
    pub fn popup(&self, message: bool) -> Option<usize> {
        if message {
            Some(S_MSG)
        } else if self.password_room.is_some() {
            Some(S_PASS)
        } else if self.create.is_some() {
            Some(S_CREATE)
        } else if self.start_menu {
            Some(S_START)
        } else {
            None
        }
    }

    /// Type text into whatever has focus (a test's keyboard).
    pub fn type_text(&mut self, s: &str) {
        self.typed.push(Typed::Text(s.to_string()));
    }

    pub fn press(&mut self, key: Typed) {
        self.typed.push(key);
    }
}

// ---------------------------------------------------------------------------
// The client's side

fn client<R>(world: &mut World, f: impl FnOnce(&mut crate::game::lobby::client::ClientLobby) -> R) -> R {
    f(&mut world.resource_mut::<CaLobbyClient>().0)
}

fn now(world: &World) -> f64 {
    chat::now_ms(world)
}

/// The 1st Arms check of the player's loadout for the room they are in.
fn arms_check(screen: &LobbyScreen, c: &crate::game::lobby::client::ClientLobby) -> Result<(), String> {
    let Some(room) = &c.room else { return Ok(()) };
    let kind = if room.settings.pure_combat { SetKind::PureCombat } else { SetKind::Normal };
    loadout::ready_check(&c.data, &screen.catalog, screen.loadout.set(kind), room.settings.arms)
}

/// The loadout goes to the gear's settings and to the lobby.
pub fn apply_loadout(world: &mut World, screen: &LobbyScreen) {
    let kind = SetKind::Normal;
    let base = world.get_resource::<crate::game::characters::CaOwnLook>().map(|l| l.0).unwrap_or_default();
    let (look, gear) = loadout::to_gear(&screen.catalog, screen.loadout.set(kind), base, screen.loadout.extender);
    if let Some(mut l) = world.get_resource_mut::<crate::game::characters::CaOwnLook>() {
        l.0 = look;
    }
    if let Some(mut g) = world.get_resource_mut::<crate::game::gear::CaOwnGear>() {
        let join = |v: &[i32]| v.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(" ");
        g.perks = join(&gear.perks);
        g.items = join(&gear.items);
        g.skill_extender = gear.extender as i32;
    }
    let l = screen.loadout.clone();
    client(world, |c| c.send(Request::Loadout(Box::new(l))));
}

// ---------------------------------------------------------------------------
// What the controls show

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

const GREY: [u8; 4] = [120, 120, 120, 255];
const WHITE: [u8; 4] = [222, 222, 222, 255];
const GOLD: [u8; 4] = [255, 198, 0, 255];

fn mode_name(data: &crate::game::lobby::data::LobbyData, id: u16) -> String {
    data.mode(id).map_or("?".into(), |m| m.name.clone())
}

/// Set every control from the client's state.
pub fn sync(world: &mut World, screen: &mut LobbyScreen) {
    let c = world.resource::<CaLobbyClient>().0.clone();
    // The page, from the room.
    if screen.page != Page::Inventory {
        screen.page = if c.room.is_some() { Page::Room } else { Page::List };
    }
    // Which sheets show.
    let want_page = screen.page_sheet();
    let popup = screen.popup(c.message.is_some());
    for i in [S_LIST, S_ROOM, S_INV] {
        let on = i == want_page;
        let id = screen.sheets[i];
        if on != screen.desk.sheets[id].shown {
            if on {
                screen.desk.show(id, false);
            } else {
                screen.desk.hide(id);
            }
        }
    }
    for (i, name) in [(S_CREATE, "create"), (S_PASS, "pass"), (S_MSG, "msg"), (S_START, "start")] {
        let on = popup == Some(i) || (name == "create" && screen.create.is_some() && popup != Some(S_MSG) && popup.is_some() && i == S_CREATE);
        let id = screen.sheets[i];
        if on != screen.desk.sheets[id].shown {
            if on {
                screen.desk.show(id, true);
            } else {
                screen.desk.hide(id);
            }
        }
    }
    match screen.page {
        Page::List => sync_list(screen, &c),
        Page::Room => sync_room(screen, &c),
        Page::Inventory => sync_inventory(screen, &c),
    }
    sync_nav(screen, &c);
    if screen.create.is_some() {
        sync_create(screen);
    }
    if screen.password_room.is_some() {
        let stars = screen.password_box.shown_input();
        screen.set_label("input_password", &stars);
    }
    if let Some(m) = &c.message {
        screen.set_text("message_text", m);
    }
}

fn sync_nav(screen: &mut LobbyScreen, c: &crate::game::lobby::client::ClientLobby) {
    let in_room = c.room.is_some();
    for name in ["button_lobby", "button_gameroom", "button_character", "StartButton"] {
        screen.set_enabled(name, true);
    }
    // "Gameroom" returns to the waiting room; Lobby leaves it. Shown as
    // the page the player is on.
    screen.set_enabled("button_gameroom", in_room);
    screen.set_selected("button_lobby", screen.page == Page::List);
    screen.set_selected("button_gameroom", screen.page == Page::Room);
    screen.set_selected("button_character", screen.page == Page::Inventory);
    screen.set_enabled("StartButton", true);
}

fn sync_list(screen: &mut LobbyScreen, c: &crate::game::lobby::client::ClientLobby) {
    let rows = c.list.visible();
    let first = c.list.scroll.min(rows.len().saturating_sub(1));
    for i in 0..LIST_ROWS {
        let name = format!("row_{i}");
        let Some(r) = rows.get(first + i) else {
            screen.set_shown(&name, false);
            continue;
        };
        screen.set_shown(&name, true);
        let row = c.list.row(r);
        let cells = [
            row.num.clone(),
            format!("{}{}", if row.locked { "[L] " } else { "" }, truncate(&row.title, 30)),
            row.map.clone(),
            row.mode.clone(),
            if row.mutation { "SS".into() } else { String::new() },
            row.arms.clone(),
            row.players.clone(),
            row.status.clone(),
        ];
        let colour = if row.disabled { GREY } else { WHITE };
        for (j, t) in cells.iter().enumerate() {
            screen.set_text(&format!("{name}_c{j}"), t);
            screen.set_colour(&format!("{name}_c{j}"), colour);
        }
        screen.set_selected(&name, c.list.selected == Some(r.id));
    }
    screen.set_selected("Check_join", c.list.filters.joinable);
    screen.set_selected("Check_pw", c.list.filters.no_password);
    screen.set_selected("Check_official", c.list.filters.pure_combat);
    let mode = c.list.filters.mode.map_or("All modes".to_string(), |(m, _)| mode_name(&c.data, m));
    let map = c.list.filters.map.map_or("All maps".to_string(), |m| c.data.map_name(m));
    screen.set_label("mode_combo", &mode);
    screen.set_label("map_combo", &map);
    screen.set_text("list_count", &format!("{} rooms", rows.len()));
    screen.set_enabled("button_Join", c.list.selected.is_some());
    sync_chat(screen, c);
}

fn sync_chat(screen: &mut LobbyScreen, c: &crate::game::lobby::client::ClientLobby) {
    let lines: Vec<_> = c.chat.iter().rev().take(CHAT_LINES).rev().collect();
    for i in 0..CHAT_LINES {
        let (t, sys) = lines.get(i).map_or((String::new(), false), |l| (l.text.clone(), l.system));
        screen.set_text(&format!("chat_line_{i}"), &t);
        screen.set_colour(&format!("chat_line_{i}"), if sys { [255, 80, 80, 255] } else { WHITE });
    }
    let cursor = if screen.focus == Focus::Chat { "_" } else { "" };
    let shown = format!("{}{cursor}", screen.chat_input);
    screen.set_label("chat_input", &shown);
}

fn sync_room(screen: &mut LobbyScreen, c: &crate::game::lobby::client::ClientLobby) {
    let Some(room) = &c.room else { return };
    let data = &c.data;
    let me = c.me;
    let mode = data.mode(room.settings.mode);
    screen.set_text("room_title", &format!("{}  [{}]", room.settings.name, room.id + 1));
    screen.set_text("mode_name", &mode.map_or(String::new(), |m| m.name.clone()));
    let one_team = mode.is_some_and(|m| m.one_team);
    for (prefix, team) in [("alpha", Team::Alpha), ("bravo", Team::Bravo)] {
        let seats: Vec<&Seat> = room.seats.iter().filter(|s| s.team == team).collect();
        for i in 0..TEAM_ROWS {
            let name = format!("{prefix}_{i}");
            match seats.get(i) {
                Some(s) => {
                    screen.set_shown(&name, true);
                    let crown = if s.id == room.host { "* " } else { "" };
                    screen.set_text(&format!("{name}_c0"), &format!("{crown}{}", s.name));
                    let status = match (s.state, s.extra) {
                        (PlayerState::Waiting, Extra::Outfitting) => "Outfitting",
                        (PlayerState::Waiting, Extra::Idle) => "Idle",
                        (st, _) => st.text(),
                    };
                    screen.set_text(&format!("{name}_c1"), status);
                    screen.set_colour(&format!("{name}_c0"), if s.id == me { GOLD } else { WHITE });
                }
                None => screen.set_shown(&name, false),
            }
        }
    }
    screen.set_shown("Btn_Alpha", !one_team);
    screen.set_shown("Btn_Bravo", !one_team);
    screen.set_enabled("button_teamChange", !one_team);
    let my_team = room.seat(me).map(|s| s.team);
    screen.set_selected("Btn_Alpha", my_team == Some(Team::Alpha));
    screen.set_selected("Btn_Bravo", my_team == Some(Team::Bravo));
    // READY or START in the same place.
    let primary = check::primary_button(room, me);
    screen.set_shown("button_game_ready", primary == Primary::Ready);
    screen.set_shown("button_game_start", primary == Primary::Start);
    let busy = room.state == RoomState::Starting && !c.buttons_enabled;
    let am_ready = room.seat(me).is_some_and(|s| s.state == PlayerState::Ready);
    screen.set_enabled("button_game_ready", !busy);
    screen.set_enabled("button_game_start", !busy);
    screen.set_selected("button_game_ready", am_ready);
    // The settings window.
    let s = &room.settings;
    let enabled = check::option_enabled(data, room, me);
    let host_waiting = room.host == me && room.state == RoomState::Waiting;
    screen.set_label("drop_mode", &mode_name(data, s.mode));
    screen.set_label("drop_map", &data.map_name(s.map));
    screen.set_label("drop_goal", &mode.map_or(String::new(), |m| m.goal_text(s.goal)));
    screen.set_label("drop_players", &mode.map_or(String::new(), |m| m.players_text(s.max_players)));
    screen.set_label("drop_arms", &data.arms_option(s.arms[0]).map_or(String::new(), |a| a.name.clone()));
    for n in ["drop_mode", "drop_map", "drop_goal", "drop_players"] {
        screen.set_enabled(n, host_waiting);
    }
    screen.set_enabled("drop_arms", enabled[5]);
    screen.set_enabled("drop_second_arms", false);
    let values = [s.join_mid, s.friendly_fire, s.kill_cam, s.abilities, s.super_soldiers, s.swap_display];
    for (n, (name, idx)) in OPTIONS.iter().enumerate() {
        let on = values[n];
        let en = enabled.get(*idx).copied().unwrap_or(false);
        screen.set_selected(&format!("{name}_On"), on);
        screen.set_selected(&format!("{name}_Off"), !on);
        screen.set_enabled(&format!("{name}_On"), en);
        screen.set_enabled(&format!("{name}_Off"), en);
    }
    screen.set_text(
        "room_info",
        &format!(
            "{} players, {}",
            room.seats.len(),
            match room.state {
                RoomState::Waiting => "waiting",
                RoomState::Starting => "starting",
                RoomState::InGame => "in game",
                RoomState::Closing => "closing",
            }
        ),
    );
    sync_chat(screen, c);
}

fn sync_create(screen: &mut LobbyScreen) {
    let Some(f) = screen.create.clone() else { return };
    screen.set_selected("tab_multi", !f.single);
    screen.set_selected("tab_single", f.single);
    screen.set_enabled("tab_single", !f.modes_single().is_empty());
    let cursor = |focus: Focus| if screen.focus == focus { "_" } else { "" };
    let name = format!("{}{}", f.name, cursor(Focus::RoomName));
    let pw = format!("{}{}", "*".repeat(f.password.chars().count()), cursor(Focus::RoomPassword));
    screen.set_label("Edit_RoomName", &name);
    screen.set_label("Edit_RoomPW", &pw);
    screen.set_selected("UsePW", f.use_password);
    screen.set_shown("UsePW", f.rows_shown());
    screen.set_shown("pw_label", f.rows_shown());
    screen.set_shown("Edit_RoomPW", f.rows_shown());
    screen.set_label("drop_mode", &f.mode_rec().name);
    screen.set_label("drop_map", &f.map_text());
    screen.set_text("goal_label", if f.single && !f.mode_rec().goal_is_difficulty() { "Goal" } else { f.goal_title() });
    let goal = f.goals().into_iter().find(|(g, _)| *g == f.goal).map(|(_, t)| t).unwrap_or_default();
    // On the single tab the second dropdown is the difficulty.
    screen.set_label("drop_goal", &goal);
    let players = f.player_options().into_iter().find(|(p, _)| *p == f.players).map(|(_, t)| t).unwrap_or_default();
    screen.set_label("Combo_Players", &players);
    screen.set_text("arms_label", if f.single { "Difficulty" } else { "1st Arms" });
    let arms = if f.single {
        ["NORMAL", "HARD", "EXTREME"][f.difficulty.min(2) as usize].to_string()
    } else {
        f.arms_options().into_iter().find(|(id, _)| *id == f.arms[0]).map(|(_, t)| t).unwrap_or_default()
    };
    screen.set_label("drop_arms", &arms);
    screen.set_enabled("drop_arms", f.single || f.arms_enabled());
    screen.set_shown("drop_second_arms", f.rows_shown());
    screen.set_shown("second_label", f.rows_shown());
    screen.set_enabled("drop_second_arms", false);
}

/// What fits the inventory's chosen slot: item numbers, in the list's
/// order, with their names.
pub fn eligible(screen: &LobbyScreen) -> Vec<(i32, String)> {
    if let Slot::Ability(_) = screen.inv_slot {
        return screen
            .catalog
            .gear
            .perks
            .iter()
            .filter_map(|p| p.items.first().map(|i| (*i, p.name.clone())))
            .collect();
    }
    screen
        .catalog
        .items
        .values()
        .filter(|i| loadout::slot_accepts(screen.inv_slot, i.kind))
        .filter(|i| screen.inv_set == SetKind::Normal || i.pure_combat)
        .map(|i| (i.no, i.name.clone()))
        .collect()
}

fn slot_label(screen: &LobbyScreen, slot: Slot) -> String {
    let set = screen.loadout.set(screen.inv_set);
    let name = |n: i32| screen.catalog.get(n).map_or(format!("#{n}"), |i| i.name.clone());
    match slot {
        Slot::Ability(i) => set
            .abilities
            .get(i as usize)
            .and_then(|n| screen.catalog.gear.perks.iter().find(|p| p.items.contains(n)))
            .map_or("-".into(), |p| p.name.clone()),
        Slot::Backpack(i) if (i as usize) >= set.usable_bag_slots(&screen.catalog) => "(no slot)".into(),
        s => set.get(s).map_or("-".into(), name),
    }
}

fn sync_inventory(screen: &mut LobbyScreen, _c: &crate::game::lobby::client::ClientLobby) {
    screen.set_selected("tab_Normal", screen.inv_set == SetKind::Normal);
    screen.set_selected("tab_Official", screen.inv_set == SetKind::PureCombat);
    for (name, slot) in SLOT_BUTTONS {
        let label = slot_label(screen, slot);
        screen.set_label(name, &label);
        screen.set_selected(name, screen.inv_slot == slot);
        let usable = match slot {
            Slot::Backpack(i) => (i as usize) < screen.loadout.set(screen.inv_set).usable_bag_slots(&screen.catalog),
            Slot::Ability(i) => i < if screen.loadout.extender { 4 } else { 3 },
            _ => true,
        };
        screen.set_enabled(name, usable);
    }
    let items = eligible(screen);
    let pages = items.len().div_ceil(ITEM_ROWS).max(1);
    screen.inv_page = screen.inv_page.min(pages - 1);
    screen.set_text("list_title", &format!("Items ({}/{})", screen.inv_page + 1, pages));
    let equipped = screen.loadout.set(screen.inv_set).equipped();
    let abilities = screen.loadout.set(screen.inv_set).abilities.clone();
    for i in 0..ITEM_ROWS {
        let name = format!("item_{i}");
        match items.get(screen.inv_page * ITEM_ROWS + i) {
            Some((no, n)) => {
                screen.set_shown(&name, true);
                let on = equipped.contains(no) || abilities.contains(no);
                screen.set_label(&name, &format!("{}{n}", if on { "* " } else { "" }));
                screen.set_selected(&name, screen.inv_pick == Some(*no));
            }
            None => screen.set_shown(&name, false),
        }
    }
    screen.set_enabled("list_prev", screen.inv_page > 0);
    screen.set_enabled("list_next", screen.inv_page + 1 < pages);
    screen.set_enabled("button_equip", screen.inv_pick.is_some());
    let note = screen.inv_note.clone();
    screen.set_text("inv_note", &note);
}

// ---------------------------------------------------------------------------
// What the controls do

fn next_in<T: PartialEq + Copy>(list: &[T], cur: T) -> Option<T> {
    if list.is_empty() {
        return None;
    }
    let i = list.iter().position(|x| *x == cur).map_or(0, |i| (i + 1) % list.len());
    Some(list[i])
}

fn do_op(world: &mut World, screen: &mut LobbyScreen, name: &str) {
    let t = now(world);
    client(world, |c| c.input(t));
    // Popups first: they are modal.
    if let Some(m) = world.resource::<CaLobbyClient>().0.message.clone() {
        if name == "button_msg_ok" && Dialog::message("", &m).handle(Press::Click(Button::Ok)) == Some(Outcome::Ok) {
            client(world, |c| c.message = None);
        }
        return;
    }
    if screen.password_room.is_some() {
        let press = match name {
            "button_ok" => Some(Press::Click(Button::Ok)),
            "button_cancel" => Some(Press::Click(Button::Cancel)),
            "input_password" => {
                screen.focus = Focus::JoinPassword;
                None
            }
            _ => None,
        };
        match press.and_then(|p| screen.password_box.handle(p)) {
            Some(Outcome::Submit(pw)) => {
                let room = screen.password_room.take().unwrap();
                screen.password_box.clear();
                client(world, |c| c.enter(room, &pw));
                screen.focus = Focus::None;
            }
            Some(_) => {
                screen.password_room = None;
                screen.password_box.clear();
                screen.focus = Focus::None;
            }
            None => {}
        }
        return;
    }
    if screen.create.is_some() {
        create_op(world, screen, name);
        return;
    }
    // The start menu: its option entry opens the options screen as a modal
    // over the current page; the room state is not touched (corrections
    // 2026-10-22, options 1).
    if screen.start_menu {
        match name {
            "button_start_option" => {
                screen.start_menu = false;
                let _ = world.resource_mut::<crate::console::Console>().submit("options");
            }
            "button_start_close" | "StartButton" => screen.start_menu = false,
            _ => {}
        }
        return;
    }
    // The nav bar.
    match name {
        "button_lobby" | "button_lobby_room" => {
            if screen.page == Page::Inventory {
                leave_inventory(world, screen);
            }
            if world.resource::<CaLobbyClient>().0.room.is_some() {
                client(world, |c| c.send(Request::Leave));
            }
            return;
        }
        "button_gameroom" => {
            if screen.page == Page::Inventory {
                leave_inventory(world, screen);
            }
            return;
        }
        "button_character" => {
            if screen.page != Page::Inventory {
                client(world, |c| c.open_outfitting());
                screen.page = Page::Inventory;
                screen.inv_note.clear();
            }
            return;
        }
        "StartButton" => {
            screen.start_menu = true;
            return;
        }
        _ => {}
    }
    match screen.page {
        Page::List => list_op(world, screen, name, t),
        Page::Room => room_op(world, screen, name),
        Page::Inventory => inventory_op(world, screen, name, t),
    }
}

fn leave_inventory(world: &mut World, screen: &mut LobbyScreen) {
    screen.page = Page::List;
    client(world, |c| c.close_outfitting());
    apply_loadout(world, screen);
}

fn list_op(world: &mut World, screen: &mut LobbyScreen, name: &str, t: f64) {
    if let Some((_, col)) = SORTS.iter().find(|(n, _)| *n == name) {
        client(world, |c| c.list.click(*col));
        return;
    }
    if let Some(i) = name.strip_prefix("row_").and_then(|n| n.parse::<usize>().ok()) {
        let id = client(world, |c| {
            let rows = c.list.visible();
            let first = c.list.scroll.min(rows.len().saturating_sub(1));
            rows.get(first + i).map(|r| r.id)
        });
        let Some(id) = id else { return };
        client(world, |c| c.list.selected = Some(id));
        // A double-click selects and joins.
        let double = screen.last_click.as_ref().is_some_and(|(n, at)| n == name && t - at < 500.0);
        screen.last_click = Some((name.to_string(), t));
        if double {
            join(world, screen, Some(id));
        }
        return;
    }
    match name {
        "Check_join" => client(world, |c| {
            c.list.filters.joinable ^= true;
            c.list.rebuild();
        }),
        "Check_pw" => client(world, |c| {
            c.list.filters.no_password ^= true;
            c.list.rebuild();
        }),
        "Check_official" => client(world, |c| {
            c.list.filters.pure_combat ^= true;
            c.list.rebuild();
        }),
        "mode_combo" => client(world, |c| {
            let mut modes: Vec<Option<(u16, bool)>> = vec![None];
            modes.extend(c.data.modes_offered(false).iter().map(|m| Some((m.id, false))));
            let cur = c.list.filters.mode;
            let next = next_in(&modes, cur).flatten();
            c.list.filter_mode(next);
        }),
        "map_combo" => client(world, |c| {
            let mut maps: Vec<Option<u16>> = vec![None];
            maps.extend(c.data.maps.iter().map(|m| Some(m.id)));
            let cur = c.list.filters.map;
            c.list.filters.map = next_in(&maps, cur).flatten();
            c.list.rebuild();
        }),
        "button_Join" => {
            let id = client(world, |c| c.list.selected);
            join(world, screen, id);
        }
        "button_create" => {
            let (data, limit) = client(world, |c| (c.data.clone(), 16));
            screen.create = Some(CreateForm::open(data, false, t as u64, limit));
            screen.focus = Focus::None;
        }
        "chat_input" => screen.focus = Focus::Chat,
        _ => {}
    }
}

fn join(world: &mut World, screen: &mut LobbyScreen, id: Option<RoomId>) {
    match client(world, |c| c.join(id)) {
        Some(Joining::Send(room)) => client(world, |c| c.enter(room, "")),
        Some(Joining::Password(room)) => {
            screen.password_room = Some(room);
            screen.password_box.clear();
            screen.focus = Focus::JoinPassword;
        }
        None => {}
    }
}

fn create_op(world: &mut World, screen: &mut LobbyScreen, name: &str) {
    let Some(f) = screen.create.as_mut() else { return };
    match name {
        "tab_multi" | "tab_single" => {
            let single = name == "tab_single";
            if f.single != single && (!single || !f.modes_single().is_empty()) {
                let now = 0;
                let (data, limit) = (f.data().clone(), f.limit);
                let mut n = CreateForm::open(data, single, now, limit);
                n.name = f.name.clone();
                *f = n;
            }
        }
        "UsePW" => f.use_password ^= true,
        "Edit_RoomName" => screen.focus = Focus::RoomName,
        "Edit_RoomPW" => screen.focus = Focus::RoomPassword,
        "drop_mode" => {
            let ids: Vec<u16> = f.modes().iter().map(|m| m.id).collect();
            if let Some(n) = next_in(&ids, f.mode) {
                f.set_mode(n);
            }
        }
        "drop_map" => {
            let ids: Vec<u16> = f.maps().iter().map(|(i, _)| *i).collect();
            if let Some(n) = next_in(&ids, f.map) {
                f.set_map(n);
            }
        }
        "drop_goal" => {
            let g: Vec<u32> = f.goals().iter().map(|(g, _)| *g).collect();
            if let Some(n) = next_in(&g, f.goal) {
                f.set_goal(n);
            }
        }
        "Combo_Players" => {
            let p: Vec<u32> = f.player_options().iter().map(|(p, _)| *p).collect();
            if let Some(n) = next_in(&p, f.players) {
                f.set_players(n);
            }
        }
        "drop_arms" => {
            if f.single {
                f.difficulty = (f.difficulty + 1) % 3;
            } else {
                let a: Vec<u8> = f.arms_options().iter().map(|(a, _)| *a).collect();
                if let Some(n) = next_in(&a, f.arms[0]) {
                    f.set_arms(n);
                }
            }
        }
        "button_cancel" => {
            screen.create = None;
            screen.focus = Focus::None;
        }
        "CreateRoom_ok" => submit_create(world, screen),
        _ => {}
    }
}

/// OK in the creation popup (C): the name checks, then the request; the
/// popup closes. The single tab builds a room for the client alone (bot
/// play: not served by the lobby), so it only closes.
fn submit_create(world: &mut World, screen: &mut LobbyScreen) {
    let Some(f) = screen.create.clone() else { return };
    let words = world.get_resource::<chat::CaChat>().map(|c| c.0.settings.words.clone());
    let bad = |n: &str| words.as_ref().is_some_and(|w| w.filter(n) != n);
    match f.submit(&bad) {
        Err(r) => client(world, |c| c.message = Some(r.text())),
        Ok(spec) => {
            if f.single {
                // Bot play: the client hosts a local match itself.
                client(world, |c| c.message = Some("Bot play is not served by the lobby.".into()));
            } else {
                client(world, |c| c.send(Request::Create(spec)));
            }
            screen.create = None;
            screen.focus = Focus::None;
        }
    }
}

fn room_op(world: &mut World, screen: &mut LobbyScreen, name: &str) {
    let check_ = {
        let c = world.resource::<CaLobbyClient>();
        arms_check(screen, &c.0)
    };
    match name {
        "button_game_ready" | "button_game_start" => client(world, |c| c.f5(check_)),
        "Btn_Alpha" => client(world, |c| c.change_team(Some(Team::Alpha))),
        "Btn_Bravo" => client(world, |c| c.change_team(Some(Team::Bravo))),
        "button_teamChange" => client(world, |c| c.change_team(None)),
        "button_invite" => client(world, |c| c.send(Request::Invite)),
        "chat_input" => screen.focus = Focus::Chat,
        "drop_mode" => {
            let next = client(world, |c| {
                let r = c.room.as_ref()?;
                let ids: Vec<u16> = c.data.modes_offered(false).iter().map(|m| m.id).collect();
                next_in(&ids, r.settings.mode)
            });
            if let Some(m) = next {
                client(world, |c| c.change(Setting::Mode(m)));
            }
        }
        "drop_map" => {
            let next = client(world, |c| {
                let r = c.room.as_ref()?;
                let mode = c.data.mode(r.settings.mode)?;
                let ids: Vec<u16> = c.data.maps_for(mode).iter().map(|m| m.id).collect();
                next_in(&ids, r.settings.map)
            });
            if let Some(m) = next {
                client(world, |c| c.change(Setting::Map(m)));
            }
        }
        "drop_goal" => {
            let next = client(world, |c| {
                let r = c.room.as_ref()?;
                let mode = c.data.mode(r.settings.mode)?;
                next_in(&mode.goal_values(), r.settings.goal)
            });
            if let Some(g) = next {
                client(world, |c| c.change(Setting::Goal(g)));
            }
        }
        "drop_players" => {
            let next = client(world, |c| {
                let r = c.room.as_ref()?;
                let mode = c.data.mode(r.settings.mode)?;
                next_in(&mode.player_values(), r.settings.max_players)
            });
            if let Some(n) = next {
                client(world, |c| c.change(Setting::Players(n)));
            }
        }
        "drop_arms" => {
            let next = client(world, |c| {
                let r = c.room.as_ref()?;
                let cup = c.data.mode(r.settings.mode).is_some_and(|m| m.arms_default == 10);
                let ids: Vec<u8> = c.data.arms_listed(cup).iter().map(|a| a.id).collect();
                next_in(&ids, r.settings.arms[0])
            });
            if let Some(a) = next {
                client(world, |c| c.change(Setting::Arms(a, 0)));
            }
        }
        _ => {
            for (opt, _) in OPTIONS {
                let on = name == format!("{opt}_On");
                if !on && name != format!("{opt}_Off") {
                    continue;
                }
                let what = match opt {
                    "Join" => Setting::JoinMid(on),
                    "FriendlyFire" => Setting::FriendlyFire(on),
                    "KillCam" => Setting::KillCam(on),
                    "Perk" => Setting::Abilities(on),
                    "Mutation" => Setting::SuperSoldiers(on),
                    _ => return,
                };
                client(world, |c| c.change(what));
            }
        }
    }
}

fn inventory_op(world: &mut World, screen: &mut LobbyScreen, name: &str, t: f64) {
    if let Some((_, slot)) = SLOT_BUTTONS.iter().find(|(n, _)| *n == name) {
        screen.inv_slot = *slot;
        screen.inv_page = 0;
        screen.inv_pick = None;
        screen.inv_note.clear();
        return;
    }
    if let Some(i) = name.strip_prefix("item_").and_then(|n| n.parse::<usize>().ok()) {
        let items = eligible(screen);
        let Some((no, _)) = items.get(screen.inv_page * ITEM_ROWS + i) else { return };
        screen.inv_pick = Some(*no);
        let double = screen.last_click.as_ref().is_some_and(|(n, at)| n == name && t - at < 500.0);
        screen.last_click = Some((name.to_string(), t));
        if double {
            equip_pick(world, screen);
        }
        return;
    }
    match name {
        "tab_Normal" => {
            screen.inv_set = SetKind::Normal;
            screen.inv_pick = None;
        }
        "tab_Official" => {
            screen.inv_set = SetKind::PureCombat;
            screen.inv_pick = None;
        }
        "list_prev" => screen.inv_page = screen.inv_page.saturating_sub(1),
        "list_next" => screen.inv_page += 1,
        "button_equip" => equip_pick(world, screen),
        "button_release" => {
            let (set, slot) = (screen.inv_set, screen.inv_slot);
            let catalog = screen.catalog.clone();
            screen.inv_note = match screen.loadout.release(&catalog, set, slot) {
                Ok(()) => String::new(),
                Err(m) => m.to_string(),
            };
            apply_loadout(world, screen);
        }
        "button_inv_back" => leave_inventory(world, screen),
        _ => {}
    }
}

fn equip_pick(world: &mut World, screen: &mut LobbyScreen) {
    let Some(item) = screen.inv_pick else { return };
    let (set, slot) = (screen.inv_set, screen.inv_slot);
    let catalog = screen.catalog.clone();
    let owned = screen.owned.clone();
    let r = match slot {
        Slot::Ability(_) => screen.loadout.equip_ability(&catalog.gear, set, item),
        _ => screen.loadout.equip(&catalog, &owned, set, slot, item),
    };
    screen.inv_note = r.err().map(str::to_string).unwrap_or_default();
    apply_loadout(world, screen);
}

// ---------------------------------------------------------------------------
// The frame

fn states(l: &Layout) -> Vec<(bool, bool)> {
    l.tree.order().into_iter().map(|i| (l.widget(i).hover, l.widget(i).held)).collect()
}

fn pointer(world: &mut World, screen: &mut LobbyScreen, ui: &CaUi) {
    let Some(p) = world.resource::<MenuPointer>().0 else { return };
    let (down, up) = {
        let m = world.resource::<ButtonInput<MouseButton>>();
        (m.just_pressed(MouseButton::Left), m.just_released(MouseButton::Left))
    };
    let ctx = ui.ctx();
    let before: Vec<_> = (0..7).map(|i| states(screen.layout(i))).collect();
    screen.desk.pointer_moved(p, &ctx);
    if down {
        screen.desk.left_down(p, &ctx);
    }
    if up {
        screen.desk.left_up();
    }
    let after: Vec<_> = (0..7).map(|i| states(screen.layout(i))).collect();
    if before != after {
        screen.changed = true;
    }
}

fn act(world: &mut World, screen: &mut LobbyScreen) {
    if screen.desk.events.is_empty() {
        return;
    }
    let events = screen.desk.take_events();
    screen.played.extend(desk::sounds(&events));
    for e in events {
        let Event::Fired { name, .. } = e else { continue };
        screen.fired.push(name.clone());
        do_op(world, screen, &name);
        screen.changed = true;
    }
}

/// The typed characters go to the focused edit box.
fn typing(world: &mut World, screen: &mut LobbyScreen) {
    let keys = std::mem::take(&mut screen.typed);
    if world.resource::<ButtonInput<KeyCode>>().just_pressed(KeyCode::F5) {
        let c = world.resource::<CaLobbyClient>();
        let check_ = arms_check(screen, &c.0);
        if screen.page == Page::Room && screen.create.is_none() {
            client(world, |c| c.f5(check_));
        }
    }
    for k in keys {
        screen.changed = true;
        match k {
            Typed::F5 => {
                let check_ = arms_check(screen, &world.resource::<CaLobbyClient>().0);
                client(world, |c| c.f5(check_));
            }
            Typed::Text(s) => {
                let s: String = s.chars().filter(|c| !c.is_control()).collect();
                match screen.focus {
                    Focus::RoomName => {
                        if let Some(f) = screen.create.as_mut() {
                            f.name.extend(s.chars().take(32usize.saturating_sub(f.name.chars().count())));
                        }
                    }
                    Focus::RoomPassword => {
                        if let Some(f) = screen.create.as_mut() {
                            f.password.extend(s.chars().take(8usize.saturating_sub(f.password.chars().count())));
                        }
                    }
                    Focus::JoinPassword => {
                        screen.password_box.handle(Press::Type(s));
                    }
                    Focus::Chat => {
                        let room = 120usize.saturating_sub(screen.chat_input.chars().count());
                        screen.chat_input.extend(s.chars().take(room));
                    }
                    Focus::None => {}
                }
            }
            Typed::Backspace => match screen.focus {
                Focus::RoomName => {
                    if let Some(f) = screen.create.as_mut() {
                        f.name.pop();
                    }
                }
                Focus::RoomPassword => {
                    if let Some(f) = screen.create.as_mut() {
                        f.password.pop();
                    }
                }
                Focus::JoinPassword => {
                    screen.password_box.handle(Press::Backspace);
                }
                Focus::Chat => {
                    screen.chat_input.pop();
                }
                Focus::None => {}
            },
            Typed::Enter => match screen.focus {
                Focus::Chat => {
                    let text = std::mem::take(&mut screen.chat_input);
                    let t = now(world);
                    // The repeat lock and the rate limit are the match
                    // chat's (radio_chat.md L).
                    let ok = world.get_resource_mut::<chat::CaChat>().is_none_or(|mut c| c.0.lobby_send(&text, t));
                    if ok {
                        client(world, |c| c.say(&text));
                    }
                }
                Focus::JoinPassword => do_op(world, screen, "button_ok"),
                Focus::RoomName | Focus::RoomPassword => do_op(world, screen, "CreateRoom_ok"),
                Focus::None => {}
            },
            Typed::Escape => {
                if world.resource::<CaLobbyClient>().0.message.is_some() {
                    client(world, |c| c.message = None);
                } else if screen.password_room.is_some() {
                    do_op(world, screen, "button_cancel");
                } else if screen.create.is_some() {
                    do_op(world, screen, "button_cancel");
                } else if screen.start_menu {
                    screen.start_menu = false;
                } else {
                    screen.focus = Focus::None;
                }
            }
        }
    }
}

/// The screen's frame: shown when the lobby wants it, its controls set,
/// its input.
pub fn update(world: &mut World) {
    if !world.contains_resource::<CaUi>() {
        return;
    }
    world.resource_scope(|world, mut screen: Mut<LobbyScreen>| {
        let screen = screen.bypass_change_detection();
        let t = now(world);
        let (active, in_match) = {
            let c = &world.resource::<CaLobbyClient>().0;
            (c.active, c.seat().is_some_and(|s| s.state == PlayerState::InGame))
        };
        // The expired-items notice on the next visit to the waiting room.
        let want = active && !in_match;
        if want != screen.open {
            screen.open = want;
            screen.changed = true;
            if !want {
                for i in 0..7 {
                    let id = screen.sheets[i];
                    screen.desk.hide(id);
                }
            } else {
                screen.page = Page::List;
                // The player's loadout goes to the gear and the lobby.
                let snapshot = screen.clone();
                apply_loadout(world, &snapshot);
            }
        }
        if !want {
            // In the match: take go.
            client(world, |c| c.go = None);
            return;
        }
        // Cloned only while the lobby is up: the install's string table
        // makes it milliseconds a frame (docs/performance.md, "Lucker
        // Party").
        let ui = world.resource::<CaUi>().clone();
        if !world.resource::<CaLobbyClient>().0.expired.is_empty() && screen.page == Page::Room {
            let gone = std::mem::take(&mut world.resource_mut::<CaLobbyClient>().0.expired);
            let names: Vec<String> = gone.iter().map(|n| screen.catalog.get(*n).map_or(format!("#{n}"), |i| i.name.clone())).collect();
            client(world, |c| c.message = Some(format!("These items have expired: {}", names.join(", "))));
            let catalog = screen.catalog.clone();
            let fell = screen.loadout.expired(&catalog, &gone);
            let expired: Vec<_> = gone.iter().filter_map(|n| catalog.get(*n).cloned()).collect();
            let owned = screen.owned.clone();
            screen.loadout.auto_equip(&catalog, &owned, &expired, &fell);
        }
        client(world, |c| c.go = None);
        // The options screen over the lobby is modal: the lobby takes no
        // pointer or keys while it is up.
        let options_up = world.get_resource::<crate::game::options::CaOptions>().is_some_and(|o| o.open);
        if options_up {
            screen.typed.clear();
        } else {
            pointer(world, screen, &ui);
            act(world, screen);
            typing(world, screen);
        }
        sync(world, screen);
        let draws = (0..7)
            .filter(|&i| screen.desk.sheets[screen.sheets[i]].shown)
            .flat_map(|i| screen.layout(i).draw(&ui.ctx()))
            .collect::<Vec<_>>();
        if draws != screen.last_draw {
            screen.last_draw = draws;
            screen.changed = true;
        }
        let _ = t;
    });
}

/// The screens without a window.
pub struct CaLobbyStatePlugin;

impl Plugin for CaLobbyStatePlugin {
    fn build(&self, app: &mut App) {
        let mut ui = app.world().get_resource::<CaUi>().cloned().unwrap_or_default();
        add_stand_ins(&mut ui);
        let catalog = Catalog::over(crate::game::data::CaData::of(app).gear());
        app.insert_resource(LobbyScreen::new(&ui, catalog))
            .insert_resource(ui)
            .init_resource::<MenuPointer>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_systems(
                Update,
                update.in_set(super::menu::MenuSystems).after(crate::game::lobby::host::CaLobbySet),
            );
    }
}

/// The lobby's flat stand-in art (ours).
fn add_stand_ins(ui: &mut CaUi) {
    let mut add = |name: &str, size: (i32, i32), c: [u8; 4]| {
        ui.stand_ins.0.insert(super::install::texture_key(name), (IVec2::new(size.0, size.1), c));
    };
    add("lob_back", (1024, 768), [18, 22, 28, 255]);
    add("lob_panel", (236, 400), [28, 32, 38, 255]);
}

/// The screens with the window: the pointer, the keys, the drawing.
pub struct CaLobbyScreenPlugin;

impl Plugin for CaLobbyScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CaLobbyStatePlugin)
            .add_message::<KeyboardInput>()
            .add_systems(Startup, spawn)
            .add_systems(Update, window_size.before(super::menu::MenuSystems))
            .add_systems(Update, collect_keys.before(super::menu::MenuSystems))
            .add_systems(Update, draw_screen.after(super::menu::MenuSystems));
        crate::console::ConsoleAppExt::console_command(app, "combat_arms:lobby", "Open Combat Arms' lobby on this process.", |w, _| {
            w.resource_mut::<crate::console::Console>().submit("ca_lobby 1");
            Ok(None)
        });
    }
}

fn window_size(window: Query<&Window, With<PrimaryWindow>>, mut screen: ResMut<LobbyScreen>) {
    let Ok(w) = window.single() else { return };
    let size = IVec2::new(w.resolution.width() as i32, w.resolution.height() as i32);
    if size.x > 0 && size.y > 0 && screen.screen != size {
        screen.screen = size;
        screen.place();
        screen.changed = true;
    }
}

/// The keyboard into the screen while it is open; the game sees none.
fn collect_keys(options: Option<Res<crate::game::options::CaOptions>>, mut events: MessageReader<KeyboardInput>, mut screen: ResMut<LobbyScreen>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    if !screen.open {
        events.clear();
        return;
    }
    // The options screen over the lobby has the keys.
    if options.is_some_and(|o| o.open) {
        return;
    }
    for e in events.read() {
        if e.state != ButtonState::Pressed {
            continue;
        }
        let t = match (&e.logical_key, e.key_code) {
            (_, KeyCode::F5) => Typed::F5,
            (Key::Enter, _) => Typed::Enter,
            (Key::Escape, _) => Typed::Escape,
            (Key::Backspace, _) => Typed::Backspace,
            (Key::Space, _) => Typed::Text(" ".into()),
            (Key::Character(s), _) => Typed::Text(s.to_string()),
            _ => continue,
        };
        screen.typed.push(t);
    }
    // Escape here is the screen's; the in-game menu does not open over it.
    keys.reset_all();
}

#[derive(Component)]
struct ScreenRoot;

fn spawn(mut commands: Commands) {
    commands.spawn((
        ScreenRoot,
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0),
            height: percent(100.0),
            ..default()
        },
        // Over the HUD, the in-game menu and the options.
        GlobalZIndex(65),
        Visibility::Hidden,
    ));
}

#[allow(clippy::too_many_arguments)]
fn draw_screen(
    mut screen: ResMut<LobbyScreen>,
    ui: Res<CaUi>,
    fonts: Option<Res<UiFonts>>,
    faces: Option<Res<CaFaces>>,
    art: Option<Res<draw::CaArt>>,
    root: Single<(Entity, &mut Visibility), With<ScreenRoot>>,
    mut commands: Commands,
) {
    let redraw = screen.changed || faces.as_ref().is_some_and(|f| f.is_changed()) || art.as_ref().is_some_and(|a| a.is_changed());
    if !redraw {
        return;
    }
    let open = screen.open;
    screen.bypass_change_detection().changed = false;
    let (root, mut vis) = root.into_inner();
    *vis = if open { Visibility::Visible } else { Visibility::Hidden };
    commands.entity(root).despawn_related::<Children>();
    if !open {
        return;
    }
    let order = screen.desk.layers.order().to_vec();
    commands.entity(root).with_children(|p| {
        for sheet in order {
            let s = &screen.desk.sheets[sheet];
            if !s.shown {
                continue;
            }
            let draws = s.layout.draw(&ui.ctx());
            draw::spawn_draws(p, &draws, &ui.stand_ins, art.as_deref(), fonts.as_deref(), faces.as_deref());
        }
    });
}

/// Items are listed by `Kind` in the inventory: for the tests, whether a
/// kind is a weapon.
pub fn is_weapon_kind(k: Kind) -> bool {
    k.weapon()
}
