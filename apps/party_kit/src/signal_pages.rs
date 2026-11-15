//! The main menu's Play and Join pages in the Signal look (feature
//! `present`; a theme with `[layout.pages]`, the Ultimate Gun Game's;
//! docs/plans/active/ugg-ui.md M5, main-menu.md M2): Settings' frame
//! (the lockup, the title and where it is, a window over the menu's
//! ambient, a bar with Back and the page's button, the keys at the foot)
//! with the kit's controls (`chimera_ui::controls`). Presentation and keys
//! only: the values, the console lines and the joins stay `main_menu`'s
//! (`PartyMenu`, `MainMenuAction`), so Start, Connect, the loading panel,
//! the failure and reconnect boxes and Disconnect are as before. Without
//! the theme (Lucker Party, the plain apps) `main_menu` draws its VGUI
//! frames.
//!
//! - **Play**: sub-tabs Server (data/main_menu.toml's `[[play]]` rows:
//!   the map a stepper through "Rotation" and the pool, ranges steppers,
//!   Off/On a toggle, up to five choices segmented, text a text field)
//!   and Game (host_settings.toml's next-match rows, `main_menu::
//!   game_settings`); a pane at the right sums the match up (minigame,
//!   map, bots, players, lobby, server, password). Enter starts.
//!   Its rows scroll in the kit's container (`chimera_ui::scroll`: the
//!   bar, the eased wheel, the right-button pan and fling); ↑↓ keep the
//!   focused row in view (`SignalPages::reveal`).
//! - **Join**: Direct (the address and the password, text fields), the
//!   recent addresses, and the **server browser** (main-menu.md M2; its
//!   rows and files are `server_list`'s) in three tabs: **LAN** (every
//!   server of ours `ServerQueries`' LAN scan found: name, minigame from
//!   its keywords, map, players/max, bots, ping, a password mark, sorted
//!   by ping; scanned when the page opens, Refresh (F5) again),
//!   **Favourites** (kept with F or its button, each asked by address for
//!   its info and ping) and **History** (the servers joined, newest first,
//!   with when). A list longer than `VISIBLE_ROWS` scrolls: the keys keep
//!   the focused row in view, the wheel moves the window, a bar at the
//!   right shows where it is. Enter on the address or password connects,
//!   on a recent address or a server joins it; a server with a password
//!   asks for it first in a **modal** (the kit's `modal` and `text_field`,
//!   the Direct password in it already; Enter joins, Esc cancels). A click
//!   on a focused row presses it.
//! - **Keys**: ↑↓ (and Tab) move the focus through the rows, ←→ change a
//!   value (on Join, not typing: the browser's tab), Space toggles, Q/E
//!   (PageUp/PageDown) the Play tabs (not while typing; on a server row
//!   PageUp/PageDown move a window), F2 F3 F4 the browser's tabs, F
//!   (not typing) favourites or drops the focused server, F5 refreshes,
//!   Enter the page's button, Esc back or closes the modal
//!   (`main_menu::typing`, which also types into the focused text field,
//!   its case kept). The pointer: a click focuses a row; the kit's hover
//!   ticks.
//! - **Motion and sound**: a page or tab change wipes the rows in on the
//!   12° slant (`Ui::wipe_cover`); the menu's cues (`ui_cues`: hover,
//!   select, back, denied, the steppers' slider).
//!
//! `ui_scene play [server|game]` and `ui_scene join [lan|favourites|
//! history|password]` stage them for screenshots (`join` with made-up
//! servers, none of them saved).
//!
//! Known shortcuts: the server browser keeps its own window of
//! `VISIBLE_ROWS` rows (PageUp/PageDown move by a window, a row index its
//! first) rather than the kit's scroll container; no Internet list (M2's rest, M3); a Recent address
//! can't be made a favourite (only a listed server); no selection shown
//! in a text field.

use std::net::SocketAddr;

use bevy::{input::mouse::{MouseScrollUnit, MouseWheel}, prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_client::client::ui_sound::UiCue;
use chimera_net::net::query::{LanSettings, QueryResult, QueryState, ServerInfo, ServerQueries};
use chimera_ui::{
    AmbientMaterial, IconImages, KitFonts, Role, Theme, Tone, UiClock, UiStyle, UiUnits,
    controls::{Kind, State, Ui},
};

use crate::{
    host::{Pick, Setting, same_value},
    main_menu::{Field, MainMenuAction, MenuData, MenuMinigame, MenuPage, MenuUiCamera, PartyMenu, PlayKind, Screen, ShownScreen},
};
pub use crate::server_list::{BrowserTab, LanServer, Status, VISIBLE_ROWS, lan_servers};
use crate::server_list::{self as sl, BrowserFiles, Visit};

/// The layout table.
pub const L: &str = "pages";

/// Play's rows' view in `chimera_ui::ScrollViews`.
pub const PLAY_SCROLL: &str = "pages.play";

/// The Signal look draws Play and Join (a theme with `[layout.pages]`).
pub fn themed(theme: Option<&Theme>) -> bool {
    theme.is_some_and(|t| t.has_layout(L))
}

/// The server browser's lists on Join (the LAN one gave it its name).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LanList {
    /// The LAN tab's rows, by ping.
    pub servers: Vec<LanServer>,
    /// The Favourites tab's rows (`fav_addrs`, answered first by ping).
    pub favourites: Vec<LanServer>,
    /// The History tab's rows (`visits`, newest first).
    pub history: Vec<LanServer>,
    /// The favourites kept, and the servers joined.
    pub fav_addrs: Vec<SocketAddr>,
    pub visits: Vec<Visit>,
    /// The tab shown, and the first row of it in the window.
    pub tab: BrowserTab,
    pub scroll: usize,
    /// A scan (or a query) still under way.
    pub scanning: bool,
    /// The queries' generation the list was made from.
    pub generation: u64,
    /// A staged scene's made-up servers (`staged_results`; not replaced by
    /// the queries, nothing written to the files).
    pub staged: bool,
    pub staged_results: Vec<QueryResult>,
    /// The rows are to be made again (a favourite or a visit changed).
    pub dirty: bool,
    /// The saved servers are to be asked once the LAN scan is over.
    pub want_saved: bool,
}

impl LanList {
    /// The rows of the tab shown.
    pub fn rows(&self) -> &[LanServer] {
        match self.tab {
            BrowserTab::Lan => &self.servers,
            BrowserTab::Favourites => &self.favourites,
            BrowserTab::History => &self.history,
        }
    }

    /// The three lists from the queries' results (a staged scene's own).
    pub fn rebuild(&mut self, results: &[QueryResult]) {
        let staged;
        let results = if self.staged {
            staged = self.staged_results.clone();
            &staged[..]
        } else {
            results
        };
        let fav = |a: &SocketAddr| self.fav_addrs.contains(a);
        let mut servers = sl::lan_servers(results);
        servers.iter_mut().for_each(|r| r.favourite = fav(&r.addr));
        let mut favourites = sl::listed(&self.fav_addrs.iter().map(|a| (*a, None)).collect::<Vec<_>>(), results);
        favourites.iter_mut().for_each(|r| r.favourite = true);
        sl::sort_favourites(&mut favourites);
        let mut history = sl::listed(&self.visits.iter().map(|v| (v.addr, Some(v.when))).collect::<Vec<_>>(), results);
        history.iter_mut().for_each(|r| r.favourite = fav(&r.addr));
        (self.servers, self.favourites, self.history) = (servers, favourites, history);
        self.scroll = sl::clamp_offset(self.scroll, self.rows().len(), VISIBLE_ROWS);
    }

    /// Every address kept (favourites, then the history's).
    pub fn saved_addrs(&self) -> Vec<SocketAddr> {
        let mut v = self.fav_addrs.clone();
        for x in &self.visits {
            if !v.contains(&x.addr) {
                v.push(x.addr);
            }
        }
        v
    }
}

/// One focusable line of a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    /// A Server tab row (`MenuData::play`'s index).
    Play(usize),
    /// A Game tab row (`main_menu::game_settings`' index).
    Game(usize),
    Address,
    Password,
    Recent(usize),
    Server(usize),
}

impl Line {
    /// The text field it is.
    pub fn field(self, data: &MenuData) -> Option<Field> {
        match self {
            Line::Play(i) if matches!(data.play.get(i).map(|r| &r.kind), Some(PlayKind::Text { .. })) => Some(Field::Play(i)),
            Line::Address => Some(Field::Address),
            Line::Password => Some(Field::Password),
            _ => None,
        }
    }
}

/// The lines of the page shown, in focus order.
pub fn lines(menu: &PartyMenu, data: &MenuData, lan: &LanList) -> Vec<Line> {
    match menu.page {
        MenuPage::Play if menu.play_tab == 0 => (0..data.play.len()).map(Line::Play).collect(),
        MenuPage::Play => (0..crate::main_menu::game_settings().len()).map(Line::Game).collect(),
        MenuPage::Join => [Line::Address, Line::Password]
            .into_iter()
            .chain((0..menu.recent.len()).map(Line::Recent))
            .chain((0..lan.rows().len()).map(Line::Server))
            .collect(),
        _ => Vec::new(),
    }
}

/// The pages' state.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct SignalPages {
    /// The page shown (None: neither).
    pub up: Option<MenuPage>,
    /// The Play tab it was drawn for.
    pub tab: usize,
    /// The focused line (an index into `lines`).
    pub focus: usize,
    /// The keys moved the focus: Play's focused row is kept in view (the
    /// kit's scroll container); the pointer moving it doesn't scroll.
    pub reveal: bool,
    /// When the page or tab last changed (the wipe), on the UI clock.
    pub since: f64,
    /// A line in error (an empty address) and the bar's notice.
    pub error: Option<usize>,
    pub notice: Option<String>,
    /// The password modal, over the server it asks for.
    pub ask: Option<PasswordAsk>,
    /// A staged scene's page (`ui_scene play|join`), opened once the menu
    /// is up, and its Play tab.
    pub stage: Option<(MenuPage, usize)>,
}

/// The password modal: a locked server (a row of the tab shown) joined
/// once its password is typed (`PartyMenu::password`, which Direct shares).
#[derive(Clone, Debug, PartialEq)]
pub struct PasswordAsk {
    pub index: usize,
    pub name: String,
}

/// A line's place (the pointer focusing it; a click on a focused line
/// presses it).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageLine(pub usize);

/// The pages' root.
#[derive(Component)]
pub struct SignalPagesRoot;

pub struct SignalPagesPlugin;

impl Plugin for SignalPagesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SignalPages>()
            .init_resource::<LanList>()
            .init_resource::<BrowserFiles>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<MouseWheel>()
            .add_systems(Startup, load_saved)
            .add_systems(
                Update,
                (follow, wheel, keys, line_clicks).chain().before(crate::main_menu::MainMenuSystems).before(crate::esc_menu::EscMenuSystems),
            )
            .add_systems(Update, crate::ui_cues::hover::<PageLine>.in_set(crate::main_menu::MainMenuSystems))
            .add_systems(Update, draw.after(crate::main_menu::MainMenuSystems));
        use chimera_client::client::ui_scene::UiSceneHooks;
        UiSceneHooks::add(app.world_mut(), "play", |w, a| stage(w, MenuPage::Play, a));
        UiSceneHooks::add(app.world_mut(), "join", |w, a| stage(w, MenuPage::Join, a));
    }
}

/// The favourites and the history from their files.
fn load_saved(files: Res<BrowserFiles>, mut lan: ResMut<LanList>) {
    lan.fav_addrs = files.read_favourites();
    lan.visits = files.read_history();
    lan.dirty = true;
}

/// `ui_scene play [server|game]`, `ui_scene join [lan|favourites|history|
/// password]`: the page over the menu (`join` with made-up servers; the
/// password modal over the first locked one of the LAN tab).
pub fn stage(w: &mut World, page: MenuPage, args: &[String]) -> Result<(), String> {
    if args.first().is_some_and(|a| a == "off") {
        w.resource_mut::<SignalPages>().stage = None;
        let files = w.get_resource::<BrowserFiles>().cloned().unwrap_or_default();
        let mut l = w.resource_mut::<LanList>();
        l.staged = false;
        l.staged_results.clear();
        l.fav_addrs = files.read_favourites();
        l.visits = files.read_history();
        l.tab = BrowserTab::Lan;
        l.dirty = true;
        return Ok(());
    }
    let arg = args.first().map(|a| a.to_ascii_lowercase());
    let tab = match (page, arg.as_deref()) {
        (MenuPage::Join, Some("favourites" | "favorites" | "favs" | "1")) => 1,
        (MenuPage::Join, Some("history" | "2")) => 2,
        (MenuPage::Join, Some("password" | "3")) => 3,
        (MenuPage::Play, Some("game" | "1")) => 1,
        _ => 0,
    };
    w.resource_mut::<SignalPages>().stage = Some((page, tab));
    if page == MenuPage::Join {
        let mut l = w.resource_mut::<LanList>();
        let (results, favs, visits) = staged_servers(sl::unix_now());
        l.staged_results = results;
        l.fav_addrs = favs;
        l.visits = visits;
        l.tab = if tab < 3 { BrowserTab::from_index(tab) } else { BrowserTab::Lan };
        l.scroll = 0;
        l.staged = true;
        l.scanning = false;
        l.dirty = true;
    }
    Ok(())
}

/// A made-up server's answer.
#[allow(clippy::too_many_arguments)]
fn made_up(ip: [u8; 4], port: u16, name: &str, map: &str, players: u8, max: u8, bots: u8, ping_ms: u32, locked: bool, lan: bool) -> QueryResult {
    use chimera_net::net::query::{GAME_FOLDER, GAME_NAME, PROTOCOL};
    QueryResult {
        addr: SocketAddr::from((ip, port)),
        state: QueryState::Answered {
            info: ServerInfo {
                protocol: PROTOCOL,
                name: name.into(),
                map: map.into(),
                folder: GAME_FOLDER.into(),
                game: GAME_NAME.into(),
                app_id: 0,
                players,
                max_players: max,
                bots,
                dedicated: false,
                os: 'l',
                password: locked,
                secure: false,
                version: chimera_net::net::NET_VERSION.into(),
                port: Some(port),
                instance: Some(port as u64 ^ ((ip[3] as u64) << 16)),
                keywords: Some("cs_source,minigame=ultimate_gun_game:auto".into()),
            },
            ping_ms,
        },
        lan,
    }
}

/// Made-up servers for the staged Join: their answers, the favourites'
/// and the history's addresses (`now`: unix seconds).
fn staged_servers(now: u64) -> (Vec<QueryResult>, Vec<SocketAddr>, Vec<Visit>) {
    let l = |ip: u8, port, name: &str, map: &str, p, max, bots, ping, locked| made_up([192, 168, 1, ip], port, name, map, p, max, bots, ping, locked, true);
    let w = |ip: u8, port, name: &str, map: &str, p, max, bots, ping, locked| made_up([198, 51, 100, ip], port, name, map, p, max, bots, ping, locked, false);
    let mut results = vec![
        l(20, 27015, "Friday ladder", "de_dust2", 7, 8, 4, 3, false),
        l(31, 27016, "Back room", "warehouse", 3, 12, 1, 9, true),
        l(44, 27015, "Knife party", "cs_office", 10, 16, 6, 21, false),
        l(52, 27015, "Attic", "de_inferno", 2, 8, 0, 24, false),
        l(53, 27016, "Kitchen table", "cs_italy", 5, 10, 3, 27, false),
        l(60, 27015, "Late shift", "de_nuke", 1, 8, 5, 31, true),
        l(61, 27015, "Spare room", "de_train", 0, 8, 8, 35, false),
        l(70, 27015, "Cousins", "cs_assault", 6, 12, 2, 41, false),
        l(71, 27016, "Basement", "de_dust2", 4, 8, 4, 48, false),
        l(80, 27015, "Garage", "cs_militia", 8, 16, 0, 55, true),
        l(81, 27015, "Rooftop", "de_aztec", 3, 8, 6, 63, false),
        l(90, 27015, "Dorm 4B", "de_cbble", 9, 12, 1, 77, false),
    ];
    results.extend([
        w(7, 27015, "Weekend regulars", "de_dust2", 11, 16, 5, 38, false),
        w(12, 27016, "Locked clubhouse", "warehouse", 4, 12, 2, 92, true),
    ]);
    results.push(QueryResult { addr: SocketAddr::from(([198, 51, 100, 99], 27015)), state: QueryState::NoResponse, lan: false });
    let addr = |r: &QueryResult| r.addr;
    let favs = vec![addr(&results[12]), addr(&results[13]), addr(&results[14]), addr(&results[1])];
    let visit = |a: SocketAddr, ago: u64| Visit { addr: a, when: now.saturating_sub(ago) };
    let visits = vec![
        visit(addr(&results[0]), 240),
        visit(addr(&results[12]), 5400),
        visit(addr(&results[13]), 3 * 86_400),
        visit(addr(&results[14]), 9 * 86_400),
    ];
    (results, favs, visits)
}

/// Scan the LAN again (Refresh), and ask the saved servers.
pub fn scan(world: &mut World) {
    let lan = world.get_resource::<LanSettings>().cloned().unwrap_or_default();
    if let Some(mut q) = world.get_resource_mut::<ServerQueries>() {
        q.scan_lan(lan.port_range(), &lan.broadcast_addresses());
    }
    if let Some(mut l) = world.get_resource_mut::<LanList>() {
        l.want_saved = true;
        l.staged = false;
        l.scanning = true;
    }
}

/// Ask the favourites and the history's servers by address, but those
/// the LAN scan found (they are answered).
fn query_saved(lan: &LanList, q: &mut ServerQueries) {
    for a in lan.saved_addrs() {
        if !q.result(a).is_some_and(|r| r.lan) {
            q.query(a);
        }
    }
}

/// Favourite the server of row `i` of the tab shown, or drop it.
pub fn toggle_favourite(world: &mut World, i: usize) {
    let files = world.get_resource::<BrowserFiles>().cloned().unwrap_or_default();
    let Some(mut lan) = world.get_resource_mut::<LanList>() else { return };
    let Some(addr) = lan.rows().get(i).map(|r| r.addr) else { return };
    let added = sl::toggle_favourite(&mut lan.fav_addrs, addr);
    lan.dirty = true;
    if !lan.staged {
        files.write_favourites(&lan.fav_addrs);
    }
    if added && !lan.staged {
        let known = world.get_resource::<ServerQueries>().is_some_and(|q| q.result(addr).is_some());
        if !known && let Some(mut q) = world.get_resource_mut::<ServerQueries>() {
            q.query(addr);
        }
    }
}

/// A join of `address` for the History (an IP address; a host name is
/// only Recent).
pub fn record_join(world: &mut World, address: &str) {
    let Some(addr) = sl::parse_addr(address) else { return };
    let files = world.get_resource::<BrowserFiles>().cloned().unwrap_or_default();
    let Some(mut lan) = world.get_resource_mut::<LanList>() else { return };
    sl::record_visit(&mut lan.visits, addr, sl::unix_now());
    lan.dirty = true;
    if !lan.staged {
        files.write_history(&lan.visits);
    }
}

/// The browser's tab `t` (a click, the keys): from its first row.
pub fn set_tab(world: &mut World, t: usize) {
    let base = world.resource::<PartyMenu>().recent.len() + 2;
    let tab = BrowserTab::from_index(t);
    let Some(mut lan) = world.get_resource_mut::<LanList>() else { return };
    if lan.tab == tab {
        return;
    }
    lan.tab = tab;
    lan.scroll = 0;
    let n = lan.rows().len();
    let mut p = world.resource_mut::<SignalPages>();
    p.ask = None;
    if p.focus >= base {
        p.focus = if n > 0 { base } else { base - 1 };
    }
}

/// Close the password modal (Esc, Cancel): whether it was open.
pub fn close_ask(world: &mut World) -> bool {
    let was = world.resource_mut::<SignalPages>().ask.take().is_some();
    if was {
        let mut m = world.resource_mut::<PartyMenu>();
        m.focus = None;
    }
    was
}

/// The modal's Join: the row asked for when a password is typed (the
/// modal closes), else None and why in its notice.
pub fn take_ask(world: &mut World) -> Option<usize> {
    let typed = !world.resource::<PartyMenu>().password.is_empty();
    let mut p = world.resource_mut::<SignalPages>();
    let ask = p.ask.clone()?;
    if !typed {
        p.notice = Some("Type the password".into());
        return None;
    }
    p.ask = None;
    world.resource_mut::<PartyMenu>().focus = None;
    Some(ask.index)
}

/// The modal over row `i`: the password field takes the keys.
fn open_ask(p: &mut SignalPages, menu: &mut PartyMenu, i: usize, name: &str) {
    p.ask = Some(PasswordAsk { index: i, name: name.to_string() });
    p.notice = None;
    menu.focus = Some(Field::Password);
    menu.caret = chimera_client::client::widgets::Caret::end_of(&menu.password);
}

fn now(clock: &Option<Res<UiClock>>) -> f64 {
    clock.as_ref().map_or(0.0, |c| c.now)
}

/// Focus line `i`: a text field takes the keys (`PartyMenu::focus`).
fn focus_line(p: &mut SignalPages, menu: &mut PartyMenu, data: &MenuData, lines: &[Line], i: usize) {
    p.focus = i.min(lines.len().saturating_sub(1));
    let field = lines.get(p.focus).and_then(|l| l.field(data));
    if menu.focus != field {
        menu.focus = field;
        let text = field.and_then(|f| match f {
            Field::Address => Some(menu.address.clone()),
            Field::Password => Some(menu.password.clone()),
            Field::Play(i) => menu.play.get(i).cloned(),
        });
        menu.caret = chimera_client::client::widgets::Caret::end_of(text.as_deref().unwrap_or(""));
    }
}

/// Opening and closing, a staged page, the focus following the menu's
/// text field, the LAN list from the queries (a scan as Join opens).
#[allow(clippy::too_many_arguments)]
fn follow(
    theme: Option<Res<Theme>>,
    clock: Option<Res<UiClock>>,
    shown: Res<ShownScreen>,
    data: Res<MenuData>,
    mut menu: ResMut<PartyMenu>,
    mut p: ResMut<SignalPages>,
    mut lan: ResMut<LanList>,
    mut queries: Option<ResMut<ServerQueries>>,
    lan_settings: Option<Res<LanSettings>>,
    mut views: Option<ResMut<chimera_ui::ScrollViews>>,
) {
    if !themed(theme.as_deref()) {
        return;
    }
    let mut stage_ask = false;
    if let Some((page, tab)) = p.stage
        && menu.shown()
        && menu.screen == Screen::MainMenu
    {
        menu.failure = None;
        menu.page = page;
        if page == MenuPage::Play {
            menu.play_tab = tab;
        }
        stage_ask = page == MenuPage::Join && tab == 3;
        menu.focus = (page == MenuPage::Join).then_some(Field::Address);
        p.stage = None;
    }
    if let Some(q) = queries.as_deref() {
        if !lan.staged && (q.generation != lan.generation || lan.dirty) {
            lan.generation = q.generation;
            lan.dirty = false;
            lan.rebuild(&q.results);
            lan.scanning = q.busy();
        }
        if lan.scanning && !q.busy() && !lan.staged {
            lan.scanning = false;
        }
    } else if lan.dirty && !lan.staged {
        lan.dirty = false;
        lan.rebuild(&[]);
    }
    if lan.staged && lan.dirty {
        lan.dirty = false;
        lan.rebuild(&[]);
    }
    if lan.want_saved
        && !lan.staged
        && let Some(q) = queries.as_deref_mut()
        && !q.scanning()
    {
        lan.want_saved = false;
        query_saved(&lan, q);
    }
    let up = (shown.screen == Screen::MainMenu
        && menu.screen == Screen::MainMenu
        && menu.failure.is_none()
        && menu.reconnecting.is_none()
        && matches!(menu.page, MenuPage::Play | MenuPage::Join))
    .then_some(menu.page);
    let t = now(&clock);
    if up != p.up || (up == Some(MenuPage::Play) && p.tab != menu.play_tab) {
        let opened = up != p.up;
        p.up = up;
        p.tab = menu.play_tab;
        p.since = t;
        p.error = None;
        p.notice = None;
        p.ask = None;
        p.reveal = false;
        // A page or tab shown again: from the top.
        if let Some(v) = views.as_deref_mut() {
            v.forget(PLAY_SCROLL);
        }
        if up.is_none() {
            if opened && !lan.staged {
                lan.tab = BrowserTab::Lan;
                lan.scroll = 0;
            }
            return;
        }
        let ls = lines(&menu, &data, &lan);
        focus_line(&mut p, &mut menu, &data, &ls, 0);
        if opened && up == Some(MenuPage::Join) {
            if stage_ask
                && let Some(i) = lan.servers.iter().position(|s| s.locked)
            {
                let base = menu.recent.len() + 2;
                let name = lan.servers[i].name.clone();
                p.focus = base + i;
                open_ask(&mut p, &mut menu, i, &name);
            }
            if !lan.staged && let Some(q) = queries.as_deref_mut() {
                let s = lan_settings.as_deref().cloned().unwrap_or_default();
                q.scan_lan(s.port_range(), &s.broadcast_addresses());
                lan.scanning = true;
                lan.want_saved = true;
            }
        }
        return;
    }
    if up.is_none() {
        return;
    }
    // The menu's text field moved (a click on a field, `mainmenu_type`):
    // the focus follows (not under the modal, which owns the field).
    let ls = lines(&menu, &data, &lan);
    if p.ask.is_none()
        && let Some(f) = menu.focus
        && ls.get(p.focus).and_then(|l| l.field(&data)) != Some(f)
        && let Some(i) = ls.iter().position(|l| l.field(&data) == Some(f))
    {
        p.focus = i;
    }
    if p.focus >= ls.len() && !ls.is_empty() {
        let last = ls.len() - 1;
        focus_line(&mut p, &mut menu, &data, &ls, last);
    }
    if p.ask.as_ref().is_some_and(|a| lan.rows().get(a.index).is_none()) {
        p.ask = None;
    }
}

/// What pressing line `i` does (Enter, a click on the focused line):
/// None when it is refused (and why in the notice) or the password modal
/// opens instead.
fn press(line: Line, menu: &mut PartyMenu, lan: &LanList, p: &mut SignalPages) -> Option<MainMenuAction> {
    match line {
        Line::Play(_) | Line::Game(_) => Some(MainMenuAction::Start),
        Line::Address | Line::Password => {
            if menu.address.trim().is_empty() {
                p.error = Some(0);
                p.notice = Some("Type the server's address (host or host:port)".into());
                return None;
            }
            Some(MainMenuAction::Connect)
        }
        Line::Recent(i) => Some(MainMenuAction::Recent(i)),
        Line::Server(i) => {
            let s = lan.rows().get(i)?;
            if !s.same_version {
                p.notice = Some(format!("{} runs another version", s.name));
                return None;
            }
            if s.locked {
                open_ask(p, menu, i, &s.name);
                return None;
            }
            Some(MainMenuAction::Server(i))
        }
    }
}

/// The window follows the focused line (a server row kept in view).
fn reveal(lan: &mut LanList, line: Option<Line>) {
    if let Some(Line::Server(i)) = line {
        let o = sl::keep_in_view(lan.scroll, i, lan.rows().len(), VISIBLE_ROWS);
        if o != lan.scroll {
            lan.scroll = o;
        }
    }
}

/// The wheel moves the list's window.
fn wheel(mut wheel: MessageReader<MouseWheel>, p: Res<SignalPages>, mut lan: ResMut<LanList>) {
    let mut d = 0.0;
    for e in wheel.read() {
        d += match e.unit {
            MouseScrollUnit::Line => e.y,
            MouseScrollUnit::Pixel => e.y / 40.0,
        };
    }
    if d == 0.0 || p.up != Some(MenuPage::Join) || p.ask.is_some() {
        return;
    }
    let notches = if d.abs() < 1.0 { d.signum() } else { d.round() };
    let o = sl::scroll_by(lan.scroll, -notches as i32, lan.rows().len(), VISIBLE_ROWS);
    if o != lan.scroll {
        lan.scroll = o;
    }
}

/// The keys (module doc).
#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    console: Option<Res<chimera_client::client::console::ConsoleUi>>,
    data: Res<MenuData>,
    mut lan: ResMut<LanList>,
    mut menu: ResMut<PartyMenu>,
    mut p: ResMut<SignalPages>,
    mut out: MessageWriter<MainMenuAction>,
    mut cues: MessageWriter<UiCue>,
) {
    let (Some(page), Some(keys)) = (p.up, keys) else { return };
    if console.is_some_and(|c| c.open) {
        return;
    }
    let k = |c: KeyCode| keys.just_pressed(c);
    if p.ask.is_some() {
        // The password modal: its field has the keys (`main_menu::typing`).
        if k(KeyCode::Enter) || k(KeyCode::NumpadEnter) {
            out.write(MainMenuAction::PasswordJoin);
        }
        return;
    }
    let ls = lines(&menu, &data, &lan);
    let line = ls.get(p.focus).copied();
    let typing = line.and_then(|l| l.field(&data)).is_some();
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let on_server = matches!(line, Some(Line::Server(_)));
    let jump = on_server && (k(KeyCode::PageUp) || k(KeyCode::PageDown));
    let up = k(KeyCode::ArrowUp) || (k(KeyCode::Tab) && shift) || (jump && k(KeyCode::PageUp));
    let down = k(KeyCode::ArrowDown) || (k(KeyCode::Tab) && !shift) || (jump && k(KeyCode::PageDown));
    if (up || down) && !ls.is_empty() {
        let by = if jump { VISIBLE_ROWS } else { 1 };
        let next = if up { p.focus.saturating_sub(by) } else { (p.focus + by).min(ls.len() - 1) };
        if next != p.focus {
            focus_line(&mut p, &mut menu, &data, &ls, next);
            p.reveal = true;
            reveal(&mut lan, ls.get(next).copied());
            cues.write(UiCue::new("ui_hover"));
        }
        return;
    }
    if k(KeyCode::Enter) || k(KeyCode::NumpadEnter) {
        let pressed = match (page, line) {
            (MenuPage::Play, _) => Some(MainMenuAction::Start),
            (_, Some(l)) => press(l, &mut menu, &lan, &mut p),
            _ => None,
        };
        match pressed {
            Some(a) => {
                out.write(a);
            }
            None if p.ask.is_none() => {
                cues.write(UiCue::new("ui_denied"));
            }
            None => {}
        }
        return;
    }
    if k(KeyCode::F5) && page == MenuPage::Join {
        out.write(MainMenuAction::Refresh);
        return;
    }
    if page == MenuPage::Join {
        // The browser's tabs: F2 F3 F4 anywhere, ←→ off a text field.
        for (key, t) in [(KeyCode::F2, 0), (KeyCode::F3, 1), (KeyCode::F4, 2)] {
            if k(key) {
                out.write(MainMenuAction::BrowserTab(t));
                return;
            }
        }
    }
    if typing {
        return;
    }
    if page == MenuPage::Join {
        if k(KeyCode::KeyF)
            && let Some(Line::Server(i)) = line
        {
            out.write(MainMenuAction::Favourite(i));
            return;
        }
        let dir = if k(KeyCode::ArrowLeft) { -1 } else if k(KeyCode::ArrowRight) { 1 } else { 0 };
        if dir != 0 {
            let next = lan.tab.step(dir);
            if next != lan.tab {
                out.write(MainMenuAction::BrowserTab(next.index()));
            }
            return;
        }
    }
    let step = |dir: i8| match line {
        Some(Line::Play(row)) => Some(MainMenuAction::PlayStep { row, dir }),
        Some(Line::Game(index)) => Some(MainMenuAction::GameStep { index, dir }),
        _ => None,
    };
    if k(KeyCode::ArrowLeft) || k(KeyCode::ArrowRight) || k(KeyCode::Space) {
        if let Some(a) = step(if k(KeyCode::ArrowLeft) { -1 } else { 1 }) {
            out.write(a);
        }
        return;
    }
    if page == MenuPage::Play && (k(KeyCode::KeyQ) || k(KeyCode::KeyE) || k(KeyCode::PageUp) || k(KeyCode::PageDown)) {
        out.write(MainMenuAction::PlayTab(1 - menu.play_tab.min(1)));
    }
}

/// A click on a line focuses it; on the focused one presses it.
fn line_clicks(
    clicked: Query<(&Interaction, &PageLine), Changed<Interaction>>,
    data: Res<MenuData>,
    mut lan: ResMut<LanList>,
    mut menu: ResMut<PartyMenu>,
    mut p: ResMut<SignalPages>,
    mut out: MessageWriter<MainMenuAction>,
    mut cues: MessageWriter<UiCue>,
) {
    if p.up.is_none() || p.ask.is_some() {
        return;
    }
    for (i, l) in &clicked {
        if *i != Interaction::Pressed {
            continue;
        }
        let ls = lines(&menu, &data, &lan);
        let Some(line) = ls.get(l.0).copied() else { continue };
        let presses = p.focus == l.0 && matches!(line, Line::Recent(_) | Line::Server(_));
        if presses {
            match press(line, &mut menu, &lan, &mut p) {
                Some(a) => {
                    out.write(a);
                }
                None if p.ask.is_none() => {
                    cues.write(UiCue::new("ui_denied"));
                }
                None => {}
            }
        } else {
            focus_line(&mut p, &mut menu, &data, &ls, l.0);
            p.reveal = false;
            reveal(&mut lan, Some(line));
            cues.write(UiCue::new("ui_select"));
        }
    }
}

// ---------------------------------------------------------------------------
// Drawing.

/// What the page was last drawn for.
#[derive(Default, PartialEq)]
struct DrawKey {
    menu: Option<PartyMenu>,
    pages: SignalPages,
    lan: LanList,
    units: (u32, u32),
    theme: String,
    camera: Option<Entity>,
    pool: Vec<String>,
}

/// What the drawing reads besides the state.
struct Ctx<'a> {
    data: &'a MenuData,
    lan: &'a LanList,
    pool: &'a [String],
    minigame: String,
    now: f64,
    /// Unix seconds (History's "5 min ago"), at the draw.
    unix: u64,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    (theme, fonts, units, clock): (Option<Res<Theme>>, Option<Res<KitFonts>>, Option<Res<UiUnits>>, Option<Res<UiClock>>),
    (menu, p, lan, data): (Res<PartyMenu>, Res<SignalPages>, Res<LanList>, Res<MenuData>),
    (minigames, game_id): (Option<Res<chimera_sim::minigame::Minigames>>, Res<MenuMinigame>),
    (mut images, mut icons, mut ambients): (Option<ResMut<Assets<Image>>>, Option<ResMut<IconImages>>, Option<ResMut<Assets<AmbientMaterial>>>),
    camera: Query<Entity, With<MenuUiCamera>>,
    roots: Query<Entity, With<SignalPagesRoot>>,
    mut drawn: Local<DrawKey>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme, units) else { return };
    let def = minigames.as_ref().and_then(|m| m.get(&game_id.0));
    let pool: Vec<String> = def.map(|d| d.maps.iter().map(|m| chimera_net::net::normalize_map(m)).collect()).unwrap_or_default();
    let cam = camera.iter().next();
    let key = DrawKey {
        menu: p.up.map(|_| menu.clone()),
        pages: SignalPages { stage: None, ..p.clone() },
        lan: if p.up == Some(MenuPage::Join) { lan.clone() } else { LanList::default() },
        units: ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32),
        theme: theme.name.clone(),
        camera: cam,
        pool: pool.clone(),
    };
    if *drawn == key {
        return;
    }
    *drawn = key;
    for r in &roots {
        commands.entity(r).despawn();
    }
    let Some(page) = p.up else { return };
    let ui = Ui::new(&theme, fonts.as_deref(), *units);
    let cx = Ctx {
        data: &data,
        lan: &lan,
        pool: &pool,
        minigame: def.map_or_else(|| game_id.0.clone(), |d| d.name.clone()),
        now: now(&clock),
        unix: sl::unix_now(),
    };
    let root = spawn(&mut commands, &ui, page, &menu, &p, &cx, images.as_deref_mut(), icons.as_deref_mut(), ambients.as_deref_mut());
    if let Some(c) = cam {
        commands.entity(root).insert(UiTargetCamera(c));
    }
}

fn lay(t: &Theme, k: &str, or: f32) -> f32 {
    t.layout_opt(L, k).unwrap_or(or)
}

fn map_label(id: &str) -> String {
    id.rsplit(':').next().unwrap_or(id).to_string()
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    c: &mut Commands,
    ui: &Ui,
    page: MenuPage,
    menu: &PartyMenu,
    p: &SignalPages,
    cx: &Ctx,
    images: Option<&mut Assets<Image>>,
    icons: Option<&mut IconImages>,
    ambients: Option<&mut Assets<AmbientMaterial>>,
) -> Entity {
    let t = ui.theme;
    let m = lay(t, "margin", 4.6);
    let root = c
        .spawn((
            SignalPagesRoot,
            Name::new("signal pages"),
            GlobalZIndex(crate::settings::SETTINGS_Z),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(t.tone_a(Tone::Ink, 1.0)),
            bevy::ui::FocusPolicy::Block,
        ))
        .id();
    if let Some(amb) = ambients {
        let (col, bloom) = AmbientMaterial::MENU;
        c.spawn((
            MaterialNode(amb.add(AmbientMaterial::new(t).column(Some(col)).bloom(bloom))),
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
    let frame = ui.boxed(c, root, UiStyle { w: Some(100.0), h: Some(56.25), shrink: Some(0.0), ..UiStyle::row() });
    // Header: the lockup, the title, where it is.
    let head = ui.boxed(c, frame, UiStyle { gap: Some(2.4), ..UiStyle::row() }.abs([Some(m), Some(2.8), Some(m), None]));
    if let (Some(images), Some(icons)) = (images, icons) {
        ui.lockup(c, head, images, icons, 1.05);
    }
    let (title, crumb) = match page {
        MenuPage::Play => ("Play", "Main menu ▸ Play • host a game others can join"),
        _ => ("Join", "Main menu ▸ Join • a server by address or on your network"),
    };
    ui.text(c, head, Role::DisplayBold, 1.2, Tone::Fg, title);
    ui.text(c, head, Role::Mono, 0.5, Tone::Fg3, crumb);
    let rule = ui.boxed(c, frame, UiStyle::row().abs([Some(m), Some(7.4), Some(m), None]));
    c.spawn((Node { width: percent(100.0), height: Val::Px(ui.units.hairline()), ..default() }, BackgroundColor(ui.color(Tone::Fg4, 1.0)), ChildOf(rule)));

    // The window.
    let win = c
        .spawn((
            UiStyle::column()
                .abs([Some(m), Some(lay(t, "win_top", 9.4)), Some(m), Some(lay(t, "win_bottom", 7.6))])
                .bg_a(Tone::Ink, 0.78)
                .outline(Tone::Fg5, 1.0),
            Node::default(),
            ChildOf(frame),
        ))
        .id();
    let top = if page == MenuPage::Play {
        let tabbar = ui.boxed(c, win, UiStyle::column().abs([Some(2.0), Some(1.6), Some(2.0), None]));
        for (i, e) in ui.tabs(c, tabbar, &["Server", "Game"], menu.play_tab, false).into_iter().enumerate() {
            c.entity(e).insert(MainMenuAction::PlayTab(i));
        }
        4.6
    } else {
        1.0
    };
    let cols = ui.boxed(
        c,
        win,
        UiStyle { gap: Some(2.4), align: Some(AlignItems::Stretch), ..UiStyle::row() }.abs([Some(2.0), Some(top), Some(2.0), Some(4.4)]),
    );
    let ls = lines(menu, cx.data, cx.lan);
    let row_h = lay(t, "row_h", 2.9);
    match page {
        MenuPage::Play => {
            // The kit's scroll container: its bar, the wheel, a
            // right-button pan; ↑↓ keep the focused row in view.
            let sc = ui.scroll(c, cols, PLAY_SCROLL, UiStyle { grow: Some(1.0), shrink: Some(1.0), ..UiStyle::column() }, UiStyle::column());
            if p.ask.is_some() {
                c.entity(sc.view).insert(chimera_ui::scroll::ScrollLocked);
            }
            for (i, l) in ls.iter().enumerate() {
                play_line(c, ui, sc.content, i, *l, menu, p, cx, row_h);
            }
            ui.wipe_cover(c, sc.frame, Tone::Ink, p.since, cx.now, lay(t, "wipe", 0.26));
            summary(c, ui, cols, menu, cx);
        }
        _ => {
            let left = ui.boxed(c, cols, UiStyle { w: Some(lay(t, "join_w", 30.0)), shrink: Some(0.0), ..UiStyle::column() });
            let right = ui.boxed(c, cols, UiStyle { grow: Some(1.0), shrink: Some(1.0), ..UiStyle::column() });
            ui.section(c, left, "Direct", None);
            for (i, l) in ls.iter().enumerate() {
                if *l == Line::Recent(0) {
                    ui.section(c, left, "Recent", None);
                }
                if matches!(l, Line::Server(_)) {
                    continue;
                }
                join_line(c, ui, left, i, *l, menu, p, row_h);
            }
            if menu.recent.is_empty() {
                ui.section(c, left, "Recent", None);
                let e = ui.boxed(c, left, UiStyle { pad: Some([0.4, 0.0, 0.0, 1.1].into()), ..UiStyle::row() });
                ui.text(c, e, Role::Mono, 0.56, Tone::Fg3, "None yet");
            }
            lan_list(c, ui, right, &ls, p, cx, row_h);
            ui.wipe_cover(c, cols, Tone::Ink, p.since, cx.now, lay(t, "wipe", 0.26));
        }
    }

    // The bar.
    let bar = c
        .spawn((
            UiStyle { h: Some(lay(t, "bar_h", 3.4)), gap: Some(1.2), pad: Some([0.0, 1.2, 0.0, 2.0].into()), ..UiStyle::row() }
                .abs([Some(0.0), None, Some(0.0), Some(0.0)])
                .bg_a(Tone::Ink, 0.6),
            Node::default(),
            ChildOf(win),
        ))
        .id();
    c.spawn((
        Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, height: Val::Px(ui.units.hairline()), ..default() },
        BackgroundColor(ui.color(Tone::Fg4, 1.0)),
        ChildOf(bar),
    ));
    let st = ui.boxed(c, bar, UiStyle { gap: Some(0.6), grow: Some(1.0), ..UiStyle::row() });
    let (mark, line) = match (&p.notice, page) {
        (Some(n), _) => (Tone::Threat, n.clone()),
        (None, MenuPage::Play) => {
            let max = value_of(cx.data, menu, "maxplayers");
            (Tone::Acc, format!("A listen server for {max} • bots fill the empty places"))
        }
        (None, _) => (Tone::Fg4, "Enter joins the focused line • the port is 27015 unless given".to_string()),
    };
    ui.mark(c, st, mark, 0.42, 0.42);
    ui.text(c, st, Role::Mono, 0.6, if p.notice.is_some() { Tone::Fg } else { Tone::Fg3 }, &line);
    let back = ui.button(c, bar, Kind::Ghost, "Back", Some("Esc"), 2.0, State::NONE);
    c.entity(back).insert(MainMenuAction::Back);
    let (label, action) = match (page, ls.get(p.focus)) {
        (MenuPage::Play, _) => ("Start", MainMenuAction::Start),
        (_, Some(Line::Server(i))) => ("Join server", MainMenuAction::Server(*i)),
        (_, Some(Line::Recent(i))) => ("Join", MainMenuAction::Recent(*i)),
        _ => ("Connect", MainMenuAction::Connect),
    };
    let go = ui.button(c, bar, Kind::Primary, label, Some("Enter"), 2.0, State::NONE);
    c.entity(go).insert(action);

    // The keys at the foot.
    let foot = ui.boxed(c, frame, UiStyle { gap: Some(2.0), ..UiStyle::row() }.abs([Some(m), None, None, Some(3.0)]));
    let keys: &[(&str, &str)] = match page {
        MenuPage::Play => &[("↑↓", "Row"), ("←→", "Change"), ("Q E", "Tab"), ("Enter", "Start"), ("Esc", "Back")],
        _ => &[("↑↓", "Row"), ("←→", "Tab"), ("F", "Favourite"), ("Enter", "Join"), ("F5", "Refresh"), ("Esc", "Back")],
    };
    for (k, what) in keys {
        let g = ui.boxed(c, foot, UiStyle { gap: Some(0.5), ..UiStyle::row() });
        ui.kbd(c, g, k, 0.6, Tone::Fg);
        ui.text(c, g, Role::Mono, 0.6, Tone::Fg2, what);
    }
    if let Some(ask) = &p.ask {
        password_modal(c, ui, root, menu, p, ask);
    }
    root
}

/// The password modal over the page: the locked server's name, the
/// password field (the Direct password in it already), Join and Cancel.
fn password_modal(c: &mut Commands, ui: &Ui, root: Entity, menu: &PartyMenu, p: &SignalPages, ask: &PasswordAsk) {
    let shade = c
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(ui.color(Tone::Ink, 0.72)),
            bevy::ui::FocusPolicy::Block,
            ZIndex(30),
            ChildOf(root),
        ))
        .id();
    let m = ui.modal(c, shade, 30.0, false);
    ui.text(c, m, Role::DisplayBold, 1.35, Tone::Fg, "Password");
    let tx = ui.boxed(c, m, UiStyle { w_pct: Some(100.0), margin: Some([0.8, 0.0, 1.0, 0.0].into()), ..UiStyle::column() });
    ui.paragraph(c, tx, Role::Mono, 0.62, Tone::Fg2, &format!("{} is locked. Type its password to join.", ask.name));
    let st = State { focus: true, error: p.notice.is_some(), ..State::NONE };
    let f = ui.text_field(c, m, &menu.password, "Password", Some(menu.caret.cursor(&menu.password)), true, 26.0, st);
    c.entity(f).insert(MainMenuAction::Focus(Field::Password));
    if let Some(n) = &p.notice {
        let row = ui.boxed(c, m, UiStyle { gap: Some(0.6), margin: Some([0.7, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
        ui.mark(c, row, Tone::Threat, 0.42, 0.42);
        ui.text(c, row, Role::Mono, 0.58, Tone::Fg, n);
    }
    let acts = ui.boxed(c, m, UiStyle { gap: Some(0.5), margin: Some([1.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() }.justify(JustifyContent::FlexEnd));
    let join = ui.button(c, acts, Kind::Primary, "Join", Some("Enter"), 2.4, State::NONE);
    c.entity(join).insert(MainMenuAction::PasswordJoin);
    let cancel = ui.button(c, acts, Kind::Secondary, "Cancel", Some("Esc"), 2.4, State::NONE);
    c.entity(cancel).insert(MainMenuAction::PasswordCancel);
}

/// A Play row's value by its cvar, as shown.
fn value_of(data: &MenuData, menu: &PartyMenu, cvar: &str) -> String {
    data.play
        .iter()
        .position(|r| r.cvar == cvar)
        .map(|i| {
            let v = menu.play.get(i).cloned().unwrap_or_default();
            match &data.play[i].kind {
                PlayKind::Pick(p) => p.shown(&v),
                _ => v,
            }
        })
        .unwrap_or_default()
}

/// How a pick is changed on the page: a toggle, segmented, or a stepper.
fn pick_widget(pick: &Pick) -> u8 {
    match pick {
        Pick::Choices(ch) if ch.iter().map(|(_, l)| l.to_ascii_lowercase()).collect::<Vec<_>>() == ["off", "on"] => 0,
        Pick::Choices(ch) if ch.len() <= 5 => 1,
        _ => 2,
    }
}

#[allow(clippy::too_many_arguments)]
fn play_line(c: &mut Commands, ui: &Ui, list: Entity, i: usize, line: Line, menu: &PartyMenu, p: &SignalPages, cx: &Ctx, row_h: f32) {
    let focus = p.focus == i;
    let vst = State::focused(focus);
    match line {
        Line::Play(r) => {
            let Some(row) = cx.data.play.get(r) else { return };
            let v = menu.play.get(r).cloned().unwrap_or_default();
            let (e, vl) = ui.row(c, list, &row.label, None, row_h, State { focus, error: p.error == Some(i), ..State::NONE });
            c.entity(e).insert(PageLine(i));
            if focus && p.reveal {
                chimera_ui::scroll_into_view(c, e);
            }
            match &row.kind {
                PlayKind::Map => {
                    let shown = if v.is_empty() { format!("Rotation • {} maps", cx.pool.len()) } else { map_label(&v) };
                    let (minus, plus) = ui.stepper(c, vl, &shown, 13.0, vst);
                    c.entity(minus).insert(MainMenuAction::PlayStep { row: r, dir: -1 });
                    c.entity(plus).insert(MainMenuAction::PlayStep { row: r, dir: 1 });
                }
                PlayKind::Pick(pick) => pick_value(c, ui, vl, pick, &v, vst, |k| match k {
                    PickAct::Step(dir) => MainMenuAction::PlayStep { row: r, dir },
                    PickAct::Option(option) => MainMenuAction::PlayPick { row: r, option },
                }),
                PlayKind::Text { masked, .. } => {
                    let caret = (menu.focus == Some(Field::Play(r))).then(|| menu.caret.cursor(&v));
                    let placeholder = if row.cvar == "hostport" { "27015" } else { "None" };
                    let f = ui.text_field(c, vl, &v, placeholder, caret, *masked, 13.0, vst);
                    c.entity(f).insert(MainMenuAction::Focus(Field::Play(r)));
                }
            }
        }
        Line::Game(g) => {
            let Some(s) = crate::main_menu::game_settings().get(g).cloned() else { return };
            let v = menu.values.get(&s.cvar).cloned().unwrap_or_default();
            let (e, vl) = ui.row(c, list, &s.label, Some("next match"), row_h, State::focused(focus));
            c.entity(e).insert(PageLine(i));
            if focus && p.reveal {
                chimera_ui::scroll_into_view(c, e);
            }
            game_value(c, ui, vl, &s, &v, vst, g);
        }
        _ => {}
    }
}

enum PickAct {
    Step(i8),
    Option(usize),
}

#[allow(clippy::too_many_arguments)]
fn pick_value(c: &mut Commands, ui: &Ui, vl: Entity, pick: &Pick, v: &str, st: State, act: impl Fn(PickAct) -> MainMenuAction) {
    match (pick_widget(pick), pick) {
        (0, Pick::Choices(ch)) => {
            let on = ch.get(1).is_some_and(|(cv, _)| same_value(cv, v));
            let tg = ui.toggle(c, vl, on, st);
            c.entity(tg).insert(act(PickAct::Step(1)));
        }
        (1, Pick::Choices(ch)) => {
            let labels: Vec<&str> = ch.iter().map(|(_, l)| l.as_str()).collect();
            let sel = ch.iter().position(|(cv, _)| same_value(cv, v));
            for (k, e) in ui.segmented(c, vl, &labels, sel, st).into_iter().enumerate() {
                c.entity(e).insert(act(PickAct::Option(k)));
            }
        }
        _ => {
            let (minus, plus) = ui.stepper(c, vl, &pick.shown(v), 7.0, st);
            c.entity(minus).insert(act(PickAct::Step(-1)));
            c.entity(plus).insert(act(PickAct::Step(1)));
        }
    }
}

fn game_value(c: &mut Commands, ui: &Ui, vl: Entity, s: &Setting, v: &str, st: State, index: usize) {
    pick_value(c, ui, vl, &s.pick, v, st, |k| match k {
        PickAct::Step(dir) => MainMenuAction::GameStep { index, dir },
        PickAct::Option(option) => MainMenuAction::GamePick { index, option },
    });
}

/// The pane at the right of Play: the match as it will start.
fn summary(c: &mut Commands, ui: &Ui, cols: Entity, menu: &PartyMenu, cx: &Ctx) {
    let pane = ui.boxed(c, cols, UiStyle { w: Some(lay(ui.theme, "summary_w", 27.0)), gap: Some(1.0), shrink: Some(0.0), ..UiStyle::column() });
    let tagrow = ui.boxed(c, pane, UiStyle::row());
    let (node, label) = chimera_ui::widgets::tag(ui.theme, ui.kit, &ui.units, chimera_ui::widgets::TagKind::Accent, "This match", 0.5);
    let tag = c.spawn((node, ChildOf(tagrow))).id();
    c.spawn((label, ChildOf(tag)));
    ui.text(c, pane, Role::DisplayBold, 1.3, Tone::Fg, &cx.minigame);
    let map_row = cx.data.play.iter().position(|r| r.kind == PlayKind::Map);
    let map = map_row.and_then(|i| menu.play.get(i)).cloned().unwrap_or_default();
    let map = if map.is_empty() {
        format!("Rotation, from {}", cx.pool.first().map_or("the pool".into(), |m| map_label(m)))
    } else {
        map_label(&map)
    };
    let password = if value_of(cx.data, menu, "sv_password").is_empty() { "None".to_string() } else { "Set".to_string() };
    let port = value_of(cx.data, menu, "hostport");
    let kv: Vec<(&str, String)> = vec![
        ("Map", map),
        ("Bots", value_of(cx.data, menu, "MaxPlayer")),
        ("Players", value_of(cx.data, menu, "maxplayers")),
        ("Lobby", value_of(cx.data, menu, "mp_lobby")),
        ("Server", value_of(cx.data, menu, "hostname")),
        ("Password", password),
        ("Port", if port.trim().is_empty() { "27015".into() } else { port }),
    ];
    ui.kv(c, pane, &kv, 6.0);
}

#[allow(clippy::too_many_arguments)]
fn join_line(c: &mut Commands, ui: &Ui, list: Entity, i: usize, line: Line, menu: &PartyMenu, p: &SignalPages, row_h: f32) {
    let focus = p.focus == i;
    let st = State { focus, error: p.error == Some(i), ..State::NONE };
    match line {
        Line::Address | Line::Password => {
            let (label, field, value, masked, ph) = if line == Line::Address {
                ("Address", Field::Address, &menu.address, false, "host or host:port")
            } else {
                ("Password", Field::Password, &menu.password, true, "None")
            };
            let (e, vl) = ui.row(c, list, label, None, row_h, st);
            c.entity(e).insert(PageLine(i));
            let caret = (menu.focus == Some(field) && p.ask.is_none()).then(|| menu.caret.cursor(value));
            let f = ui.text_field(c, vl, value, ph, caret, masked, 15.0, st);
            c.entity(f).insert(MainMenuAction::Focus(field));
        }
        Line::Recent(r) => {
            let Some(a) = menu.recent.get(r) else { return };
            let (e, vl) = ui.row(c, list, a, None, row_h, State::focused(focus));
            c.entity(e).insert(PageLine(i));
            if focus {
                ui.kbd(c, vl, "Enter", 0.56, Tone::Fg2);
            }
        }
        _ => {}
    }
}

/// The LAN servers' cells' widths, cq: map, players, bots, ping, lock.
const CELLS: [f32; 5] = [7.0, 4.2, 3.0, 3.6, 1.8];

fn cells(c: &mut Commands, ui: &Ui, vl: Entity, texts: [&str; 5], tone: Tone, size: f32) {
    for (w, tx) in CELLS.iter().zip(texts) {
        let cell = ui.boxed(c, vl, UiStyle { w: Some(*w), shrink: Some(0.0), ..UiStyle::row() });
        ui.text(c, cell, Role::Mono, size, tone, tx);
    }
}

/// The browser at the right of Join: the tabs, the head (a note, Favourite
/// and Refresh), the columns' heads and the window of rows.
#[allow(clippy::too_many_arguments)]
fn lan_list(c: &mut Commands, ui: &Ui, right: Entity, ls: &[Line], p: &SignalPages, cx: &Ctx, row_h: f32) {
    let rows = cx.lan.rows();
    let n = rows.len();
    let tabs = ui.boxed(c, right, UiStyle::column());
    let names: Vec<&str> = BrowserTab::ALL.iter().map(|t| t.name()).collect();
    for (i, e) in ui.tabs(c, tabs, &names, cx.lan.tab.index(), false).into_iter().enumerate() {
        c.entity(e).insert(MainMenuAction::BrowserTab(i));
    }
    let first = cx.lan.scroll.min(n.saturating_sub(VISIBLE_ROWS));
    let last = (first + VISIBLE_ROWS).min(n);
    let asking = cx.lan.scanning && cx.lan.tab == BrowserTab::Lan;
    let note = if asking {
        "Looking...".to_string()
    } else if n == 0 {
        "None".to_string()
    } else if n > VISIBLE_ROWS {
        format!("{}-{} of {n}", first + 1, last)
    } else if cx.lan.tab == BrowserTab::Lan {
        format!("{n} found, by ping")
    } else {
        format!("{n}")
    };
    let head = ui.boxed(c, right, UiStyle { gap: Some(1.0), ..UiStyle::row() });
    let sec = ui.boxed(c, head, UiStyle { grow: Some(1.0), ..UiStyle::column() });
    ui.section(c, sec, cx.lan.tab.name(), Some(&note));
    let focused = match ls.get(p.focus) {
        Some(Line::Server(i)) => rows.get(*i).map(|r| (*i, r.favourite)),
        _ => None,
    };
    let (label, i) = match focused {
        Some((i, true)) => ("Unfavourite", i),
        Some((i, false)) => ("Favourite", i),
        None => ("Favourite", 0),
    };
    let fav = ui.button(c, head, Kind::Secondary, label, Some("F"), 1.9, State { disabled: focused.is_none(), ..State::NONE });
    c.entity(fav).insert(MainMenuAction::Favourite(i));
    let refresh = ui.button(c, head, Kind::Secondary, "Refresh", Some("F5"), 1.9, State { disabled: cx.lan.scanning, ..State::NONE });
    c.entity(refresh).insert(MainMenuAction::Refresh);
    // The columns' heads.
    let heads = ui.boxed(
        c,
        right,
        UiStyle { pad: Some([0.3, 1.3, 0.3, 1.1].into()), gap: Some(1.2), ..UiStyle::row() }.justify(JustifyContent::SpaceBetween),
    );
    ui.text(c, heads, Role::Mono, 0.5, Tone::Fg3, "Server • minigame");
    let hv = ui.boxed(c, heads, UiStyle { gap: Some(0.6), shrink: Some(0.0), ..UiStyle::row() });
    cells(c, ui, hv, ["Map", "Players", "Bots", "Ping", ""], Tone::Fg3, 0.5);
    if n == 0 {
        let e = ui.boxed(c, right, UiStyle { pad: Some([0.6, 0.0, 0.0, 1.1].into()), ..UiStyle::row() });
        let none = match cx.lan.tab {
            BrowserTab::Lan if asking => "Looking for servers on your network",
            BrowserTab::Lan => "No server answered on your network",
            BrowserTab::Favourites => "No favourites yet: focus a server and press F",
            BrowserTab::History => "No server joined yet",
        };
        ui.text(c, e, Role::Mono, 0.56, Tone::Fg3, none);
        return;
    }
    let list = ui.boxed(c, right, UiStyle::column());
    for (i, l) in ls.iter().enumerate() {
        let Line::Server(s) = l else { continue };
        if !(first..last).contains(s) {
            continue;
        }
        let Some(sv) = rows.get(*s) else { continue };
        let focus = p.focus == i;
        let mut small: Vec<String> = Vec::new();
        match sv.status {
            Status::Up => small.extend(sv.minigame.as_deref().map(|m| m.rsplit(':').next().unwrap_or(m).replace('_', " "))),
            Status::Asking => small.push("asking".into()),
            Status::Down => small.push("no answer".into()),
        }
        if let Some(w) = sv.when {
            small.insert(0, sl::ago(cx.unix, w));
        }
        if sv.favourite && cx.lan.tab != BrowserTab::Favourites {
            small.push("favourite".into());
        }
        let small = small.join(" • ");
        let (e, vl) = ui.row(c, list, &sv.name, Some(&small), row_h, State { focus, disabled: !sv.same_version || sv.status == Status::Down, ..State::NONE });
        c.entity(e).insert(PageLine(i));
        if sv.status == Status::Up {
            let players = format!("{}/{}", sv.players, sv.max_players);
            let bots = sv.bots.to_string();
            let ping = format!("{} ms", sv.ping_ms);
            cells(c, ui, vl, [&map_label(&sv.map), &players, &bots, &ping, if sv.locked { "PW" } else { "" }], Tone::Fg, 0.6);
        } else {
            cells(c, ui, vl, ["", "", "", if sv.status == Status::Asking { "..." } else { "-" }, ""], Tone::Fg3, 0.6);
        }
    }
    if n > VISIBLE_ROWS {
        // Where the window is: a thin bar at the right edge of the rows.
        let track = c
            .spawn((
                Node { position_type: PositionType::Absolute, right: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, width: ui.units.val(0.3), ..default() },
                BackgroundColor(ui.color(Tone::Fg, 0.08)),
                Pickable::IGNORE,
                ChildOf(list),
            ))
            .id();
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::ZERO,
                right: Val::ZERO,
                top: percent(first as f32 / n as f32 * 100.0),
                height: percent(VISIBLE_ROWS as f32 / n as f32 * 100.0),
                ..default()
            },
            BackgroundColor(ui.color(Tone::Acc, 1.0)),
            Pickable::IGNORE,
            ChildOf(track),
        ));
    }
}
