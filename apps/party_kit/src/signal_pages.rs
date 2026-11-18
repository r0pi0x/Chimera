//! The main menu's Play and Join pages in the Signal look (feature
//! `present`; a theme with `[layout.pages]`, the Ultimate Gun Game's;
//! docs/plans/active/ugg-ui.md M5 and "Signal revision 8", main-menu.md
//! M2), Play shown as **Host**: the shell (`chimera_ui::shell`: the bar
//! with the breadcrumb "‹ Main menu" over the title, the window, its
//! action bar with the status sentence and the hero, the keys at the
//! foot; Back is the breadcrumb, Esc or a right click) with the kit's
//! controls (`chimera_ui::controls`). Presentation and keys
//! only: the values, the console lines and the joins stay `main_menu`'s
//! (`PartyMenu`, `MainMenuAction`), so Start, Connect, the loading panel,
//! the failure and reconnect boxes and Disconnect are as before. Without
//! the theme (Lucker Party, the plain apps) `main_menu` draws its VGUI
//! frames.
//!
//! - **Host** (`MenuPage::Play`): sub-tabs Server (data/main_menu.toml's
//!   `[[play]]` rows: Server name, Password and Port first; the Maps
//!   field opening the picker, `maps_picker`; ranges steppers, Off/On a
//!   toggle, up to five choices segmented, one choice its words, text a
//!   text field; UPnP's outcome a STATUS block, its words on hover) and
//!   Game (host_settings.toml's next-match rows, `main_menu::
//!   game_settings`), Q and E at the tab strip's ends; a pane at the right
//!   sums the match up. Enter starts (on the Maps row: opens the picker).
//!   Its rows scroll in the kit's container (`chimera_ui::scroll`: the
//!   bar, the eased wheel); ↑↓ keep the focused row in view
//!   (`SignalPages::reveal`).
//! - **Join**: Direct (the address and the password, text fields), the
//!   recent addresses, and the **server browser** (main-menu.md M2 and
//!   M3; its rows and files are `server_list`'s) in four tabs:
//!   **Internet** (docs/plans/active/hub.md 4.3: the servers the
//!   account's hub lists, `chimera_net::net::hub::Hub`, asked when the
//!   page opens and on Refresh; each is then asked directly for its info
//!   and ping, at most `server_list::MAX_QUERIES` at once, the hub's
//!   numbers filling its row until it answers; another build's server is
//!   greyed with "needs build ...", another app's left out unless All
//!   games is on; without an account the tab says how to sign in, with
//!   the hub not answering it shows the last list, marked; the tab the
//!   page opens on when signed in to a hub), **LAN** (every
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
//!   PageUp/PageDown move a window), F1 F2 F3 F4 the browser's tabs, F
//!   (not typing) favourites or drops the focused server, F5 refreshes,
//!   Q/E (not typing) the browser's tabs (their caps at the strip's ends),
//!   Enter the page's button, Esc back or closes the modal
//!   (`main_menu::typing`, which also types into the focused text field,
//!   its case kept). The pointer: a click focuses a row; the kit's hover
//!   ticks.
//! - **Motion and sound**: a page or tab change wipes the rows in on the
//!   12° slant (`Ui::wipe_cover`); the menu's cues (`ui_cues`: hover,
//!   select, back, denied, the steppers' slider).
//!
//! - **The hub's words**: a build older than the hub's servers take
//!   (`client.min_net`) shows a banner over the browser, "A newer build
//!   is needed for this hub's servers", with the build and the link (LAN,
//!   Favourites, History and Direct work as before); the main menu's
//!   news card (`signal_menu::MenuNews`) shows the same, else the hub's
//!   newest news until dismissed, else its MOTD (`hub_news`).
//!
//! `ui_scene play [server|game|maps]` (`maps`: the picker open) and `ui_scene join [internet|lan|favourites|
//! history|password|offline|update|nohub]` stage them for screenshots (`join` with made-up
//! servers, none of them saved; `offline`: the hub not answering, its
//! last list shown; `update`: the newer-build banner; `nohub`: no account).
//!
//! Known shortcuts: the server browser keeps its own window of
//! `VISIBLE_ROWS` rows (PageUp/PageDown move by a window, a row index its
//! first) rather than the kit's scroll container; a Recent address
//! can't be made a favourite (only a listed server); no selection shown
//! in a text field.

use std::net::SocketAddr;

use bevy::{input::mouse::{MouseScrollUnit, MouseWheel}, prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_client::client::ui_sound::UiCue;
use chimera_net::net::{
    hub::{Hub, HubGame, Listing},
    query::{LanSettings, QueryResult, QueryState, ServerInfo, ServerQueries},
};
use chimera_ui::{
    AmbientMaterial, IconImages, KitFonts, Role, Theme, Tone, UiClock, UiStyle, UiUnits,
    controls::{Kind, NOTE, SENTENCE, State, Ui},
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

/// What the Internet tab says of the hub besides its rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HubView {
    /// This machine has an account on a hub.
    pub signed_in: bool,
    /// The hub's name (its document's).
    pub name: String,
    /// The list is being asked.
    pub busy: bool,
    /// Why the last call failed (the hub offline, a refusal).
    pub offline: Option<String>,
    /// A list came at all (else nothing to show yet).
    pub has_list: bool,
    /// This build is too old for the hub's servers: the build to get and
    /// where.
    pub update: Option<(String, String)>,
}

/// The hub's list asked server by server (`server_list::HubQueue`; apart
/// from `LanList` so asking doesn't redraw the page).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct HubAsks(pub sl::HubQueue);

/// The server browser's lists on Join (the LAN one gave it its name).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LanList {
    /// The Internet tab's rows (`server_list::internet_servers`), the
    /// hub's listings they are made from, this app's game on the hub,
    /// the games the account may play (None: all) and whether other
    /// apps' servers are listed too.
    pub internet: Vec<LanServer>,
    pub hub_list: Vec<Listing>,
    pub hub_game: String,
    pub hub_games: Option<Vec<String>>,
    pub all_games: bool,
    pub hub: HubView,
    /// The hub's generation the listings were taken at, and when the
    /// list they are came.
    pub hub_generation: u64,
    pub hub_listed: Option<std::time::Instant>,
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
            BrowserTab::Internet => &self.internet,
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
        let games = self.hub_games.clone();
        let may_play = |g: &str| games.as_ref().is_none_or(|l| l.iter().any(|x| x == "*" || x == g));
        let mut internet = sl::internet_servers(&self.hub_list, results, &self.hub_game, self.all_games, &may_play);
        internet.iter_mut().for_each(|r| r.favourite = fav(&r.addr));
        self.internet = internet;
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
            .init_resource::<crate::maps_picker::MapsPicker>()
            .init_resource::<crate::maps_picker::MapsSource>()
            .init_resource::<crate::maps_picker::PickerRows>()
            .add_systems(
                PostUpdate,
                crate::maps_picker::bind_rows.after(chimera_ui::vlist::place_slots).before(bevy::ui::UiSystems::Prepare),
            )
            .add_message::<crate::maps_picker::PickerAction>()
            .init_resource::<LanList>()
            .init_resource::<HubAsks>()
            .init_resource::<BrowserFiles>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<MouseWheel>()
            .add_systems(Startup, load_saved)
            .add_systems(
                Update,
                (follow, wheel, picker_keys, picker_clicks, keys, line_clicks)
                    .chain()
                    .before(crate::main_menu::MainMenuSystems)
                    .before(crate::esc_menu::EscMenuSystems),
            )
            .add_systems(Update, crate::ui_cues::hover::<PageLine>.in_set(crate::main_menu::MainMenuSystems))
            .add_systems(Update, (field_pointer, draw).chain().after(crate::main_menu::MainMenuSystems))
            .add_systems(Update, hub_news.run_if(resource_exists::<crate::signal_menu::MenuNews>));
        use chimera_client::client::ui_scene::UiSceneHooks;
        UiSceneHooks::add(app.world_mut(), "play", |w, a| stage(w, MenuPage::Play, a));
        UiSceneHooks::add(app.world_mut(), "join", |w, a| stage(w, MenuPage::Join, a));
    }
}

/// The Maps picker's keys while it is open (`maps_picker` module doc):
/// ↑↓ the row, Space on or off, ←→ fold or open, Enter Done, Esc Cancel
/// (a right click is Esc).
#[allow(clippy::too_many_arguments)]
fn picker_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut picker: ResMut<crate::maps_picker::MapsPicker>,
    source: Res<crate::maps_picker::MapsSource>,
    pages: Res<SignalPages>,
    (minigames, game_id, slices): (
        Option<Res<chimera_sim::minigame::Minigames>>,
        Res<MenuMinigame>,
        Option<Res<chimera_sim::minigame::arena::ArenaSlices>>,
    ),
    mut out: MessageWriter<crate::maps_picker::PickerAction>,
    mut cues: MessageWriter<UiCue>,
) {
    use crate::maps_picker::PickerAction as P;
    picker.took_keys = picker.open.is_some();
    if picker.open.is_none() {
        return;
    }
    if pages.up != Some(MenuPage::Play) {
        // Not on Host (yet: a staged scene opens it first): no keys.
        if pages.up.is_some() {
            picker.open = None;
        }
        picker.took_keys = false;
        return;
    }
    let pool = picker_pool(minigames.as_deref(), &game_id.0, slices.as_deref());
    let games = source.0.listing(&pool);
    let rows = picker.rows(&games);
    let k = |c: KeyCode| keys.just_pressed(c);
    if k(KeyCode::Escape) {
        out.write(P::Cancel);
    } else if k(KeyCode::Enter) || k(KeyCode::NumpadEnter) {
        out.write(P::Done);
    } else if k(KeyCode::Space) {
        out.write(P::Toggle(picker.cursor));
    } else if (k(KeyCode::ArrowUp) || k(KeyCode::ArrowDown)) && !rows.is_empty() {
        let next = if k(KeyCode::ArrowUp) { picker.cursor.saturating_sub(1) } else { (picker.cursor + 1).min(rows.len() - 1) };
        if next != picker.cursor {
            picker.cursor = next;
            cues.write(UiCue::new("ui_hover"));
        }
    } else if k(KeyCode::ArrowLeft) || k(KeyCode::ArrowRight) {
        let did = if k(KeyCode::ArrowLeft) { picker.left(&games) } else { picker.right(&games) };
        if did {
            cues.write(UiCue::new("ui_select"));
        }
    }
}

/// The minigame's pool, the picker's listing's source: each map's arena
/// slices after it (`ArenaSlices::after_maps`).
fn picker_pool(
    minigames: Option<&chimera_sim::minigame::Minigames>,
    id: &str,
    slices: Option<&chimera_sim::minigame::arena::ArenaSlices>,
) -> Vec<String> {
    let maps: Vec<String> = minigames
        .and_then(|m| m.get(id))
        .map(|d| d.maps.iter().map(|m| chimera_net::net::normalize_map(m)).collect())
        .unwrap_or_default();
    slices.map_or(maps.clone(), |s| s.after_maps(&maps))
}

/// The picker's clicks and what its actions do.
#[allow(clippy::too_many_arguments)]
fn picker_clicks(
    pressed: Query<(&Interaction, &crate::maps_picker::PickerAction), Changed<Interaction>>,
    mut actions: ResMut<Messages<crate::maps_picker::PickerAction>>,
    mut picker: ResMut<crate::maps_picker::MapsPicker>,
    source: Res<crate::maps_picker::MapsSource>,
    (minigames, game_id, slices): (
        Option<Res<chimera_sim::minigame::Minigames>>,
        Res<MenuMinigame>,
        Option<Res<chimera_sim::minigame::arena::ArenaSlices>>,
    ),
    mut cues: MessageWriter<UiCue>,
) {
    use crate::maps_picker::PickerAction as P;
    let mut todo: Vec<P> = actions.drain().collect();
    todo.extend(pressed.iter().filter(|(i, _)| **i == Interaction::Pressed).map(|(_, a)| *a));
    if todo.is_empty() || picker.open.is_none() {
        return;
    }
    let pool = picker_pool(minigames.as_deref(), &game_id.0, slices.as_deref());
    let games = source.0.listing(&pool);
    let rows = picker.rows(&games);
    for a in todo {
        match a {
            P::Toggle(i) => {
                if let Some(row) = rows.get(i) {
                    picker.cursor = i;
                    picker.toggle(&games, row);
                    cues.write(UiCue::new("ui_select"));
                }
            }
            P::Fold(i) => {
                if let Some(row) = rows.get(i) {
                    // A folded one opens, an open one folds.
                    let open = picker.is_folded(&games, row);
                    picker.cursor = i;
                    picker.fold(&games, row, open);
                    cues.write(UiCue::new("ui_select"));
                }
            }
            P::AllOff => {
                let all: Vec<String> = games.iter().flat_map(|g| g.groups.iter().flat_map(|k| k.maps.iter().cloned())).collect();
                picker.open = Some(all.into_iter().collect());
                cues.write(UiCue::new("ui_select"));
            }
            P::Done => {
                if let Some(off) = picker.open.take() {
                    picker.off = off;
                }
                cues.write(UiCue::new("ui_select"));
            }
            P::Cancel => {
                picker.open = None;
                cues.write(UiCue::new("ui_back"));
            }
        }
    }
}

/// The pointer in a text field (Play's, Join's, the password's;
/// `chimera_ui::controls::TextPress`): a press places the caret between
/// the characters it is nearest (`TextHits`, from the layout; after the
/// menu's `Focus`, which put it at the end), a drag from it selects, a
/// double press selects all.
#[allow(clippy::type_complexity)]
fn field_pointer(
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    fields: Query<(Entity, &Interaction, &MainMenuAction), With<chimera_ui::controls::EditText>>,
    hits: chimera_ui::controls::TextHits,
    clock: Option<Res<UiClock>>,
    mut st: Local<(chimera_ui::controls::TextPress, Option<Field>, bool)>,
    mut menu: ResMut<PartyMenu>,
) {
    use chimera_client::client::widgets::Caret;
    let Some(mouse) = mouse else { return };
    let now = clock.map_or(0.0, |c| c.now);
    let (press, held, dragged) = &mut *st;
    let field_of = |a: &MainMenuAction| match a {
        MainMenuAction::Focus(f) => Some(*f),
        _ => None,
    };
    if mouse.just_pressed(MouseButton::Left) {
        *dragged = false;
        *held = None;
        let hit = fields.iter().find_map(|(e, i, a)| (*i == Interaction::Pressed).then(|| field_of(a).map(|f| (e, f))).flatten());
        let Some((e, f)) = hit else {
            press.release();
            return;
        };
        let (Some(at), Some(len)) = (hits.char_at(e), hits.len(e)) else { return };
        let (c, a) = press.press(now, at, len);
        *held = Some(f);
        if menu.focus == Some(f) {
            menu.caret = Caret { at: c, anchor: a };
        }
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        press.release();
        *held = None;
        return;
    }
    let Some(f) = *held else { return };
    if menu.focus != Some(f) {
        return;
    }
    let Some(e) = fields.iter().find_map(|(e, _, a)| (field_of(a) == Some(f)).then_some(e)) else { return };
    let Some(at) = hits.char_at(e) else { return };
    if let Some((c, a)) = press.drag(at)
        && (c != a || *dragged)
    {
        *dragged = true;
        let next = Caret { at: c, anchor: a };
        if menu.caret != next {
            menu.caret = next;
        }
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
        // The hub's list again from the `Hub` (none: no hub).
        l.hub_list.clear();
        l.hub = HubView::default();
        l.all_games = false;
        l.hub_generation = u64::MAX;
        l.dirty = true;
        return Ok(());
    }
    let arg = args.first().map(|a| a.to_ascii_lowercase());
    // Join: the browser's tab, whether the password modal is up, and how
    // the hub stands.
    let (join_tab, hub_mode) = match arg.as_deref() {
        Some("internet" | "hub") => (BrowserTab::Internet, "on"),
        Some("offline") => (BrowserTab::Internet, "offline"),
        Some("update") => (BrowserTab::Internet, "update"),
        Some("nohub") => (BrowserTab::Internet, "nohub"),
        Some("favourites" | "favorites" | "favs" | "1") => (BrowserTab::Favourites, "on"),
        Some("history" | "2") => (BrowserTab::History, "on"),
        _ => (BrowserTab::Lan, "on"),
    };
    let tab = match (page, arg.as_deref()) {
        (MenuPage::Join, Some("password" | "3")) => STAGE_ASK,
        (MenuPage::Play, Some("game" | "1")) => 1,
        _ => 0,
    };
    w.resource_mut::<SignalPages>().stage = Some((page, tab));
    // `ui_scene play maps`: the Maps picker open, a game folded and a few
    // maps off.
    if page == MenuPage::Play && arg.as_deref() == Some("maps") {
        let mut pk = w.resource_mut::<crate::maps_picker::MapsPicker>();
        pk.open = Some(Default::default());
        pk.cursor = 6;
    }
    // `ui_scene play maps slices`: the games without arena slices folded,
    // the cursor on the first map with slices, that map off and its first
    // slice on.
    if page == MenuPage::Play && arg.as_deref() == Some("maps") && args.get(1).is_some_and(|a| a == "slices") {
        let pool = picker_pool(
            w.get_resource::<chimera_sim::minigame::Minigames>(),
            &w.resource::<MenuMinigame>().0,
            w.get_resource::<chimera_sim::minigame::arena::ArenaSlices>(),
        );
        let games = w.resource::<crate::maps_picker::MapsSource>().0.listing(&pool);
        let mut pk = w.resource_mut::<crate::maps_picker::MapsPicker>();
        let sliced = |g: &crate::maps_picker::CatalogueGame| g.groups.iter().any(|k| k.maps.iter().any(|m| m.contains('@')));
        pk.folded = games.iter().filter(|g| !sliced(g)).map(|g| g.id.clone()).collect();
        let rows = pk.rows(&games);
        let first_slice = rows.iter().position(|r| {
            matches!(r, crate::maps_picker::TreeRow::Map(..))
                && crate::maps_picker::MapsPicker::maps_of(&games, r).first().is_some_and(|m| m.contains('@'))
        });
        if let Some(i) = first_slice {
            pk.cursor = i - 1;
            // The map off, its first slice kept on (the stored form:
            // `game:map,+game:map@id`).
            if let Some(m) = crate::maps_picker::MapsPicker::maps_of(&games, &rows[i]).first()
                && let Some(map) = crate::maps_picker::slice_parent(m)
            {
                pk.open = Some([map.to_string(), format!("{}{m}", crate::maps_picker::ON_MARK)].into_iter().collect());
            }
        }
    }
    if page == MenuPage::Join {
        let game = w.get_resource::<HubGame>().map(|g| g.0.clone()).unwrap_or_default();
        let mut l = w.resource_mut::<LanList>();
        let (mut results, favs, visits) = staged_servers(sl::unix_now());
        let (listings, answers) = staged_hub(&game);
        results.extend(answers);
        l.staged_results = results;
        l.fav_addrs = favs;
        l.visits = visits;
        l.tab = join_tab;
        l.hub_list = if hub_mode == "nohub" { Vec::new() } else { listings };
        l.hub_game = game;
        l.hub_games = None;
        l.all_games = args.get(1).is_some_and(|a| a == "all");
        l.hub = HubView {
            signed_in: hub_mode != "nohub",
            name: if hub_mode == "nohub" { String::new() } else { "Friday hub".into() },
            busy: false,
            offline: (hub_mode == "offline").then(|| "the hub didn't answer".to_string()),
            has_list: hub_mode != "nohub",
            update: (hub_mode == "update").then(|| ("0.1.1".to_string(), "example.invalid/releases".to_string())),
        };
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

/// A staged scene's password modal (`SignalPages::stage`'s tab on Join).
const STAGE_ASK: usize = 3;

/// A made-up hub's list for the staged Join (`game`: this app's), and
/// the answers of the servers of it that answered: most of this app's
/// and answering, one of another build, one still asked, one silent, one
/// of another app.
fn staged_hub(game: &str) -> (Vec<Listing>, Vec<QueryResult>) {
    use chimera_net::net::hub::Reach;
    let game = if game.is_empty() { "ultimate_gun_game" } else { game };
    let mut listings = Vec::new();
    let mut answers = Vec::new();
    let mut add = |ip: u8, name: &str, map: &str, players: u8, max: u8, bots: u8, ping: Option<u32>, locked: bool, app: &str, net: &str| {
        let port = 27015;
        let addr = SocketAddr::from(([203, 0, 113, ip], port));
        let instance = port as u64 ^ ((ip as u64) << 16);
        listings.push(Listing {
            addr: addr.to_string(),
            lan: Vec::new(),
            instance: format!("{instance:016x}"),
            name: name.into(),
            game: app.into(),
            minigame: Some("ultimate_gun_game:auto".into()),
            map: map.into(),
            players,
            max,
            bots,
            password: locked,
            dedicated: true,
            net: net.into(),
            reachable: Reach::Yes,
            since: 0,
            mine: false,
            hint: None,
        });
        match ping {
            Some(ms) if ms > 0 => {
                let mut r = made_up([203, 0, 113, ip], port, name, map, players, max, bots, ms, locked, false);
                if let QueryState::Answered { info, .. } = &mut r.state {
                    info.version = net.into();
                }
                answers.push(r);
            }
            // Silent.
            Some(_) => answers.push(QueryResult { addr, state: QueryState::NoResponse, lan: false }),
            // Still asked.
            None => answers.push(QueryResult { addr, state: QueryState::Pending, lan: false }),
        }
    };
    let ours = chimera_net::net::NET_VERSION;
    add(11, "Saturday playtest", "de_dust2", 9, 12, 3, Some(24), false, game, ours);
    add(12, "Halo night", "hangemhigh", 6, 8, 2, Some(31), false, game, ours);
    add(13, "The long ladder", "warehouse", 12, 16, 4, Some(46), false, game, ours);
    add(14, "Clubhouse", "cs_office", 3, 8, 5, Some(58), true, game, ours);
    add(15, "Late and loud", "de_inferno", 2, 12, 8, Some(87), false, game, ours);
    add(16, "Overseas", "de_nuke", 5, 12, 0, Some(164), false, game, ours);
    add(17, "Next week's build", "de_train", 4, 12, 2, Some(39), false, game, "0.1.1/net99");
    add(18, "Just started", "cs_italy", 0, 8, 0, None, false, game, ours);
    add(19, "Gone quiet", "de_aztec", 1, 8, 0, Some(0), false, game, ours);
    add(20, "Party mix", "de_dust2", 7, 16, 0, Some(52), false, "lucker_party", ours);
    (listings, answers)
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
    // The hub's list again too (the Internet tab).
    if let Some(mut h) = world.get_resource_mut::<Hub>()
        && h.signed_in()
    {
        h.refresh_list();
    }
}

/// The Internet tab lists other apps' servers too, or this app's only.
pub fn toggle_all_games(world: &mut World) {
    if let Some(mut l) = world.get_resource_mut::<LanList>() {
        l.all_games = !l.all_games;
        l.scroll = 0;
        l.dirty = true;
    }
}

/// What the main menu's news card says for the hub (module doc, "The
/// hub's words"): the newer-build notice, else its newest news not
/// dismissed (with the MOTD beside the hub's name), else its MOTD.
pub fn hub_card(hub: &Hub) -> Option<crate::signal_menu::News> {
    use crate::signal_menu::News;
    let d = hub.doc.as_ref()?;
    let name = hub.name();
    if let Some((build, link)) = hub.needs_newer_build() {
        return Some(News { title: UPDATE_TITLE.into(), body: format!("{} LAN games and your own installs still work.", update_words(&build, &link)), source: name, dismiss: None });
    }
    let motd = d.motd.trim();
    if let Some(n) = d.newest_news(&hub.dismissed) {
        let source = [name.as_str(), n.date.trim(), motd].into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" \u{2022} ");
        return Some(News { title: n.title.clone(), body: n.text.clone(), source, dismiss: Some(n.id.clone()) });
    }
    (!motd.is_empty()).then(|| News { title: motd.to_string(), body: String::new(), source: name, dismiss: None })
}

/// The newer-build notice's title and its second sentence.
pub const UPDATE_TITLE: &str = "A newer build is needed for this hub's servers";

fn update_words(build: &str, link: &str) -> String {
    match (build.trim().is_empty(), link.trim().is_empty()) {
        (false, false) => format!("Get build {build}: {link}."),
        (true, false) => format!("Get it at {link}."),
        (false, true) => format!("Ask the host for build {build}."),
        (true, true) => "Ask the host for the new build.".to_string(),
    }
}

/// Keeps the main menu's news card as the hub's (`hub_card`); a card set
/// by hand (`menu_news`) stays until the hub says something else.
fn hub_news(hub: Option<Res<Hub>>, mut news: ResMut<crate::signal_menu::MenuNews>, mut mine: Local<Option<crate::signal_menu::News>>) {
    let Some(hub) = hub else { return };
    if !hub.is_changed() {
        return;
    }
    let next = hub_card(&hub);
    if next != *mine {
        if news.0 == *mine || next.is_some() {
            news.0 = next.clone();
        }
        *mine = next;
    }
}

/// Dismiss the news the card shows (its Dismiss button): kept in the
/// hub's cache, the card then shows the next one or the MOTD.
pub fn dismiss_news(world: &mut World) {
    let Some(id) = world.get_resource::<crate::signal_menu::MenuNews>().and_then(|n| n.0.as_ref()?.dismiss.clone()) else { return };
    if let Some(mut h) = world.get_resource_mut::<Hub>() {
        h.dismiss(&id);
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

/// Focus line `i`. A text field on it takes the keys (`PartyMenu::focus`)
/// only when `edit` (a click, an explicit Enter); otherwise the line is
/// just the cursor and no field has the keys. Switching a tab or moving
/// down the list never captures the keyboard on Host (Join's address
/// still opens focused).
fn focus_line(p: &mut SignalPages, menu: &mut PartyMenu, data: &MenuData, lines: &[Line], i: usize, edit: bool) {
    p.focus = i.min(lines.len().saturating_sub(1));
    let field = lines.get(p.focus).and_then(|l| l.field(data)).filter(|_| edit);
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
    (mut hub, hub_game, mut asks): (Option<ResMut<Hub>>, Option<Res<HubGame>>, ResMut<HubAsks>),
) {
    if !themed(theme.as_deref()) {
        return;
    }
    // The hub's list and state (the Internet tab), when they changed.
    if !lan.staged {
        match hub.as_deref() {
            Some(h) => {
                let view = HubView {
                    signed_in: h.signed_in(),
                    name: h.name(),
                    busy: h.busy,
                    offline: h.error.clone(),
                    has_list: h.list.is_some(),
                    update: h.needs_newer_build(),
                };
                if lan.hub != view {
                    lan.hub = view;
                }
                let game = hub_game.as_ref().map(|g| g.0.clone()).unwrap_or_default();
                if lan.hub_generation != h.generation || lan.hub_game != game {
                    lan.hub_generation = h.generation;
                    lan.hub_game = game;
                    lan.hub_list = h.list.as_ref().map(|l| l.servers.clone()).unwrap_or_default();
                    lan.hub_games = h.games.clone();
                    // A list that just came: each of its servers asked
                    // (again); another answer of the hub's asks none.
                    if lan.hub_listed != h.listed_at {
                        lan.hub_listed = h.listed_at;
                        asks.0.fill(&lan.hub_list);
                    }
                    lan.dirty = true;
                }
            }
            None => {
                if lan.hub != HubView::default() || !lan.hub_list.is_empty() {
                    lan.hub = HubView::default();
                    lan.hub_list.clear();
                    lan.dirty = true;
                }
            }
        }
        // Each listed server asked itself, a few at once.
        if asks.0.busy()
            && let Some(q) = queries.as_deref_mut()
        {
            let pending = |a: SocketAddr| q.result(a).is_some_and(|r| matches!(r.state, QueryState::Pending));
            let now = asks.bypass_change_detection().0.next(pending);
            for a in now {
                q.query(a);
            }
        }
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
        stage_ask = page == MenuPage::Join && tab == STAGE_ASK;
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
        // Join opens on the Internet tab when this machine is signed in to
        // a hub, and asks its list again.
        if opened
            && up == Some(MenuPage::Join)
            && !lan.staged
            && let Some(h) = hub.as_deref_mut().filter(|h| h.signed_in())
        {
            h.refresh_list();
            lan.tab = BrowserTab::Internet;
            lan.scroll = 0;
        }
        let ls = lines(&menu, &data, &lan);
        focus_line(&mut p, &mut menu, &data, &ls, 0, up == Some(MenuPage::Join));
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
        let edit = menu.page != MenuPage::Play;
        focus_line(&mut p, &mut menu, &data, &ls, last, edit);
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
            if s.barred {
                p.notice = Some(match &s.note {
                    Some(n) => format!("{}: {n}", s.name),
                    None => format!("{} can't be joined", s.name),
                });
                return None;
            }
            if !s.same_version {
                p.notice = Some(match &s.note {
                    Some(n) => format!("{} {n}", s.name),
                    None => format!("{} runs another version", s.name),
                });
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
    (picker, party): (Res<crate::maps_picker::MapsPicker>, Option<Res<crate::party_signal::PartyPanel>>),
) {
    let (Some(page), Some(keys)) = (p.up, keys) else { return };
    if console.is_some_and(|c| c.open) || picker.took_keys || party.is_some_and(|p| p.open) {
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
    let typing = line.and_then(|l| l.field(&data)).is_some_and(|f| menu.focus == Some(f));
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let on_server = matches!(line, Some(Line::Server(_)));
    let jump = on_server && (k(KeyCode::PageUp) || k(KeyCode::PageDown));
    let up = k(KeyCode::ArrowUp) || (k(KeyCode::Tab) && shift) || (jump && k(KeyCode::PageUp));
    let down = k(KeyCode::ArrowDown) || (k(KeyCode::Tab) && !shift) || (jump && k(KeyCode::PageDown));
    if (up || down) && !ls.is_empty() {
        let by = if jump { VISIBLE_ROWS } else { 1 };
        let next = if up { p.focus.saturating_sub(by) } else { (p.focus + by).min(ls.len() - 1) };
        if next != p.focus {
            focus_line(&mut p, &mut menu, &data, &ls, next, page != MenuPage::Play);
            p.reveal = true;
            reveal(&mut lan, ls.get(next).copied());
            cues.write(UiCue::new("ui_hover"));
        }
        return;
    }
    let on_maps = matches!(line, Some(Line::Play(r)) if data.play.get(r).is_some_and(|row| row.kind == PlayKind::Map));
    if on_maps && (k(KeyCode::Enter) || k(KeyCode::NumpadEnter) || k(KeyCode::Space)) {
        out.write(MainMenuAction::OpenMaps);
        return;
    }
    if page == MenuPage::Play
        && (k(KeyCode::Enter) || k(KeyCode::NumpadEnter))
        && let Some(f) = line.and_then(|l| l.field(&data))
        && menu.focus != Some(f)
    {
        // An explicit Enter on a text row: it takes the keys (the next
        // Enter starts the game, as with any row).
        let at = p.focus;
        focus_line(&mut p, &mut menu, &data, &ls, at, true);
        cues.write(UiCue::new("ui_select"));
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
        // The browser's tabs: F1 F2 F3 F4 anywhere, ←→ off a text field.
        for (key, t) in [(KeyCode::F1, 0), (KeyCode::F2, 1), (KeyCode::F3, 2), (KeyCode::F4, 3)] {
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
        let dir = if k(KeyCode::ArrowLeft) || k(KeyCode::KeyQ) {
            -1
        } else if k(KeyCode::ArrowRight) || k(KeyCode::KeyE) {
            1
        } else {
            0
        };
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
            focus_line(&mut p, &mut menu, &data, &ls, l.0, true);
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
    party: Option<chimera_ui::shell::Party>,
    upnp: String,
    picker: crate::maps_picker::MapsPicker,
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
    party: &'a chimera_ui::shell::Party,
    /// UPnP's outcome: its light and the plain words (the status block's
    /// hover).
    upnp: (chimera_ui::controls::StatusLight, String),
    /// The Maps picker and its listing.
    picker: &'a crate::maps_picker::MapsPicker,
    games: Vec<crate::maps_picker::CatalogueGame>,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    (theme, fonts, units, clock): (Option<Res<Theme>>, Option<Res<KitFonts>>, Option<Res<UiUnits>>, Option<Res<UiClock>>),
    (menu, p, lan, data): (Res<PartyMenu>, Res<SignalPages>, Res<LanList>, Res<MenuData>),
    (minigames, game_id): (Option<Res<chimera_sim::minigame::Minigames>>, Res<MenuMinigame>),
    (mut images, mut icons, mut ambients): (Option<ResMut<Assets<Image>>>, Option<ResMut<IconImages>>, Option<ResMut<Assets<AmbientMaterial>>>),
    camera: Query<Entity, With<MenuUiCamera>>,
    roots: Query<Entity, With<SignalPagesRoot>>,
    (party, upnp, picker, maps_source, slices): (
        Res<chimera_ui::shell::Party>,
        Option<Res<chimera_net::net::upnp::Upnp>>,
        Res<crate::maps_picker::MapsPicker>,
        Option<Res<crate::maps_picker::MapsSource>>,
        Option<Res<chimera_sim::minigame::arena::ArenaSlices>>,
    ),
    mut drawn: Local<DrawKey>,
    mut lists: ResMut<chimera_ui::VirtualLists>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme, units) else { return };
    let def = minigames.as_ref().and_then(|m| m.get(&game_id.0));
    let pool = picker_pool(minigames.as_deref(), &game_id.0, slices.as_deref());
    let cam = camera.iter().next();
    let key = DrawKey {
        menu: p.up.map(|_| menu.clone()),
        pages: SignalPages { stage: None, ..p.clone() },
        lan: if p.up == Some(MenuPage::Join) { lan.clone() } else { LanList::default() },
        units: ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32),
        theme: theme.name.clone(),
        camera: cam,
        pool: pool.clone(),
        party: Some(party.clone()),
        upnp: upnp.as_ref().map(|u| format!("{:?}", u.state)).unwrap_or_default(),
        picker: picker.clone(),
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
        party: &party,
        upnp: upnp_status(upnp.as_deref(), &menu, &data),
        picker: &picker,
        games: {
            let mut games = maps_source.as_ref().map(|m| m.0.listing(&pool)).unwrap_or_default();
            crate::maps_picker::name_slices(&mut games, slices.as_deref());
            games
        },
    };
    let root = spawn(&mut commands, &ui, page, &menu, &p, &cx, images.as_deref_mut(), icons.as_deref_mut(), ambients.as_deref_mut(), &mut lists);
    if let Some(c) = cam {
        commands.entity(root).insert(UiTargetCamera(c));
    }
}

/// UPnP's light and its words, plainly: forwarded, asking, failed with
/// the reason, or off (and what the toggle will do at Start).
fn upnp_status(u: Option<&chimera_net::net::upnp::Upnp>, menu: &PartyMenu, data: &MenuData) -> (chimera_ui::controls::StatusLight, String) {
    use chimera_net::net::upnp::UpnpState;
    use chimera_ui::controls::StatusLight as L;
    let line = u.and_then(|u| u.state.line()).map(|l| {
        let mut ch = l.chars();
        ch.next().map(|f| f.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or_default()
    });
    match (u.map(|u| &u.state), line) {
        (Some(UpnpState::Mapped(..)), Some(l)) => (L::Ok, l),
        (Some(UpnpState::Failed(..)), Some(l)) => (L::Failed, l),
        (Some(UpnpState::Asking(..)), Some(l)) => (L::Busy, l),
        _ => {
            let on = data.play.iter().position(|r| r.cvar == "net_upnp").and_then(|i| menu.play.get(i)).is_some_and(|v| v.trim() == "1");
            let words = if on {
                "Off until you start: then it asks your router to forward the port, and says here whether it did."
            } else {
                "Off: friends outside your network need the port forwarded by hand."
            };
            (L::Off, words.to_string())
        }
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
    lists: &mut chimera_ui::VirtualLists,
) -> Entity {
    let t = ui.theme;
    let sh = chimera_ui::shell::ShellLayout::of(t);
    let root = c
        .spawn((
            SignalPagesRoot,
            chimera_ui::shell::ShellScreen,
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
    // The shell's bar: the breadcrumb back to the main menu over the
    // title, the party and the system tiles.
    let title = if page == MenuPage::Play { "Host" } else { "Join" };
    let (mut images, mut icons) = (images, icons);
    if let (Some(images), Some(icons)) = (images.as_deref_mut(), icons.as_deref_mut()) {
        let bar = chimera_ui::shell::Bar {
            left: chimera_ui::shell::Left::Crumb { parent: "Main menu", title },
            party: &cx.party.members,
            party_max: cx.party.max, invite: cx.party.invite,
            open: None,
        };
        let parts = chimera_ui::shell::bar(ui, c, frame, images, icons, &bar);
        if let Some(e) = parts.tile(chimera_ui::shell::ShellTile::Settings) {
            c.entity(e).insert(MainMenuAction::OpenSettings);
        }
    }

    // The window.
    let win = chimera_ui::shell::window(ui, c, frame);
    let top = if page == MenuPage::Play {
        let tabbar = ui.boxed(c, win, UiStyle::column().abs([Some(sh.rows_in), Some(1.6), Some(sh.side_w + 2.0 * sh.rows_in), None]));
        for (i, e) in ui.tabs_ex(c, tabbar, &["Server", "Game"], menu.play_tab, false, Some(("Q", "E"))).into_iter().enumerate() {
            c.entity(e).insert(MainMenuAction::PlayTab(i));
        }
        4.6
    } else {
        1.6
    };
    let cols = ui.boxed(
        c,
        win,
        UiStyle { gap: Some(2.4), align: Some(AlignItems::Stretch), ..UiStyle::row() }.abs([Some(sh.rows_in), Some(top), Some(sh.rows_in), Some(sh.act_h + 1.4)]),
    );
    let ls = lines(menu, cx.data, cx.lan);
    let row_h = lay(t, "row_h", 2.9);
    match page {
        MenuPage::Play => {
            // The kit's scroll container: its bar, the wheel, a
            // the thumb; ↑↓ keep the focused row in view.
            let sc = ui.scroll(c, cols, PLAY_SCROLL, UiStyle { grow: Some(1.0), shrink: Some(1.0), ..UiStyle::column() }, UiStyle::column());
            if p.ask.is_some() {
                c.entity(sc.view).insert(chimera_ui::scroll::ScrollLocked);
            }
            for (i, l) in ls.iter().enumerate() {
                let res = match (images.as_deref_mut(), icons.as_deref_mut()) {
                    (Some(a), Some(b)) => Some((a, b)),
                    _ => None,
                };
                play_line(c, ui, sc.content, i, *l, menu, p, cx, row_h, res);
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
                ui.sentence(c, e, chimera_ui::controls::SENTENCE, Tone::Fg2, "None yet.");
            }
            lan_list(c, ui, right, &ls, p, cx, row_h);
            ui.wipe_cover(c, cols, Tone::Ink, p.since, cx.now, lay(t, "wipe", 0.26));
        }
    }

    // The action bar: the status sentence at the left, the hero last.
    let bar = chimera_ui::shell::action_bar(ui, c, win);
    let st = ui.boxed(c, bar, UiStyle { gap: Some(0.6), grow: Some(1.0), ..UiStyle::row() });
    let (notice, line) = match (&p.notice, page) {
        (Some(n), _) => (true, n.clone()),
        (None, MenuPage::Play) => {
            let max = value_of(cx.data, menu, "maxplayers");
            (false, format!("A listen server for {max}. Bots fill the empty places."))
        }
        (None, _) => (false, "Enter joins the focused line. The port is 27015 unless given.".to_string()),
    };
    if notice {
        ui.mark(c, st, Tone::Threat, 0.42, 0.42);
    }
    ui.sentence(c, st, chimera_ui::controls::SENTENCE, if notice { Tone::Fg } else { Tone::Fg2 }, &line);
    let (label, action) = match (page, ls.get(p.focus)) {
        (MenuPage::Play, _) => ("Start", MainMenuAction::Start),
        (_, Some(Line::Server(i))) => ("Join server", MainMenuAction::Server(*i)),
        (_, Some(Line::Recent(i))) => ("Join", MainMenuAction::Recent(*i)),
        _ => ("Connect", MainMenuAction::Connect),
    };
    let go = ui.button(c, bar, Kind::Primary, label, Some("Enter"), chimera_ui::controls::HERO_H, State::NONE);
    c.entity(go).insert(action);

    // The keys at the foot (Back is the breadcrumb; the tabs' Q and E sit
    // at the strips' ends).
    let keys: &[(&str, &str)] = match page {
        MenuPage::Play => &[("↑↓", "Row"), ("←→", "Change"), ("Enter", "Start")],
        _ => &[("↑↓", "Row"), ("F", "Favourite"), ("Enter", "Join"), ("F5", "Refresh")],
    };
    chimera_ui::shell::keys(ui, c, frame, keys);
    if page == MenuPage::Play && cx.picker.open.is_some() {
        crate::maps_picker::spawn(c, ui, root, cx.picker, &cx.games, lists);
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
    ui.paragraph(c, tx, Role::Text, SENTENCE, Tone::Fg2, &format!("{} is locked. Type its password to join.", ask.name));
    let st = State { focus: true, error: p.notice.is_some(), ..State::NONE };
    let f = ui.text_field(c, m, &menu.password, "Password", Some(menu.caret.cursor(&menu.password)), true, 26.0, st);
    c.entity(f).insert(MainMenuAction::Focus(Field::Password));
    if let Some(n) = &p.notice {
        let row = ui.boxed(c, m, UiStyle { gap: Some(0.6), margin: Some([0.7, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
        ui.mark(c, row, Tone::Threat, 0.42, 0.42);
        ui.sentence(c, row, NOTE, Tone::Fg, n);
    }
    let acts = ui.boxed(c, m, UiStyle { gap: Some(0.5), margin: Some([1.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() }.justify(JustifyContent::FlexEnd));
    let join = ui.button(c, acts, Kind::Primary, "Join", Some("Enter"), chimera_ui::controls::HERO_H, State::NONE);
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
#[allow(clippy::too_many_arguments)]
fn play_line(
    c: &mut Commands,
    ui: &Ui,
    list: Entity,
    i: usize,
    line: Line,
    menu: &PartyMenu,
    p: &SignalPages,
    cx: &Ctx,
    row_h: f32,
    res: Option<(&mut Assets<Image>, &mut IconImages)>,
) {
    let focus = p.focus == i;
    let vst = State::focused(focus);
    match line {
        Line::Play(r) => {
            let Some(row) = cx.data.play.get(r) else { return };
            let v = menu.play.get(r).cloned().unwrap_or_default();
            let (e, vl) = ui.row(c, list, &row.label, None, row_h, State { focus, error: p.error == Some(i), ..State::NONE });
            c.entity(e).insert((PageLine(i), crate::ui_cues::CueItem(format!("page_line:{i}"))));
            if focus && p.reveal {
                chimera_ui::scroll_into_view(c, e);
            }
            match &row.kind {
                PlayKind::Map => {
                    // The Maps field: opens the picker (the list-with-
                    // chevron icon: a dialog, not a dropdown or a link).
                    let shown = if v.is_empty() { cx.picker.summary(&cx.games) } else { map_label(&v) };
                    if let Some((images, icons)) = res {
                        let f = ui.dialog_field(c, vl, images, icons, &shown, 14.0, vst);
                        c.entity(f).insert(MainMenuAction::OpenMaps);
                    }
                }
                PlayKind::Pick(pick) if row.cvar == "net_upnp" => {
                    // The outcome as a status block (its words on hover),
                    // then the toggle.
                    let b = ui.status_block(c, vl, cx.upnp.0);
                    c.entity(b).insert(chimera_ui::shell::Tip::new("UPnP").note(&cx.upnp.1));
                    pick_value(c, ui, vl, pick, &v, vst, |k| match k {
                        PickAct::Step(dir) => MainMenuAction::PlayStep { row: r, dir },
                        PickAct::Option(option) => MainMenuAction::PlayPick { row: r, option },
                    })
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
            c.entity(e).insert((PageLine(i), crate::ui_cues::CueItem(format!("page_line:{i}"))));
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
        // One choice: its words, nothing to pick.
        (_, Pick::Choices(ch)) if ch.len() == 1 => {
            ui.sentence(c, vl, chimera_ui::controls::TYPED, Tone::Fg2, &ch[0].1);
        }
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
            let (minus, plus) = ui.stepper(c, vl, &pick.shown(v), pick.stepper_range(v), st);
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
        cx.picker.summary(&cx.games)
    } else {
        map_label(&map)
    };
    let password = if value_of(cx.data, menu, "sv_password").is_empty() { "None".to_string() } else { "Set".to_string() };
    let port = value_of(cx.data, menu, "hostport");
    // Values in capitals, the server's name as its host typed it.
    let kv: Vec<(&str, String)> = vec![
        ("Map", map.to_uppercase()),
        ("Bots", value_of(cx.data, menu, "MaxPlayer").to_uppercase()),
        ("Players", value_of(cx.data, menu, "maxplayers").to_uppercase()),
        ("Lobby", value_of(cx.data, menu, "mp_lobby").to_uppercase()),
        ("Server", value_of(cx.data, menu, "hostname")),
        ("Password", password.to_uppercase()),
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
            c.entity(e).insert((PageLine(i), crate::ui_cues::CueItem(format!("page_line:{i}"))));
            let caret = (menu.focus == Some(field) && p.ask.is_none()).then(|| menu.caret.cursor(value));
            let f = ui.text_field(c, vl, value, ph, caret, masked, 15.0, st);
            c.entity(f).insert(MainMenuAction::Focus(field));
        }
        Line::Recent(r) => {
            let Some(a) = menu.recent.get(r) else { return };
            let (e, vl) = ui.row(c, list, a, None, row_h, State::focused(focus));
            c.entity(e).insert((PageLine(i), crate::ui_cues::CueItem(format!("page_line:{i}"))));
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
    let hub = &cx.lan.hub;
    let internet = cx.lan.tab == BrowserTab::Internet;
    // A build older than the hub's servers take: said over the browser
    // (the other tabs and Direct work as before).
    if let Some((build, link)) = &hub.update {
        let banner = c
            .spawn((
                UiStyle { gap: Some(0.25), pad: Some([0.7, 1.1, 0.7, 1.3].into()), margin: Some([0.0, 0.0, 0.9, 0.0].into()), ..UiStyle::column() }
                    .bg_a(Tone::Ink, 0.7)
                    .outline(Tone::Fg5, 1.0),
                Node::default(),
                chimera_ui::UiParityId::new("join.update"),
                ChildOf(right),
            ))
            .id();
        ui.focus_bar(c, banner, 0.22);
        ui.text(c, banner, Role::DisplayBold, 0.8, Tone::Fg, UPDATE_TITLE);
        ui.sentence(c, banner, NOTE, Tone::Fg2, &update_words(build, link));
    }
    // One row: the tabs (Q and E at their ends), then the list's tools.
    let top = ui.boxed(c, right, UiStyle { gap: Some(1.0), align: Some(AlignItems::FlexEnd), margin: Some([0.0, 0.0, 0.8, 0.0].into()), ..UiStyle::row() });
    let tabs = ui.boxed(c, top, UiStyle { grow: Some(1.0), ..UiStyle::column() });
    let names: Vec<&str> = BrowserTab::ALL.iter().map(|t| t.name()).collect();
    for (i, e) in ui.tabs_ex(c, tabs, &names, cx.lan.tab.index(), false, Some(("Q", "E"))).into_iter().enumerate() {
        c.entity(e).insert(MainMenuAction::BrowserTab(i));
    }
    let first = cx.lan.scroll.min(n.saturating_sub(VISIBLE_ROWS));
    let last = (first + VISIBLE_ROWS).min(n);
    let asking = (cx.lan.scanning && cx.lan.tab == BrowserTab::Lan) || (internet && hub.busy);
    let focused = match ls.get(p.focus) {
        Some(Line::Server(i)) => rows.get(*i).map(|r| (*i, r.favourite)),
        _ => None,
    };
    let (label, i) = match focused {
        Some((i, true)) => ("Unfavourite", i),
        Some((i, false)) => ("Favourite", i),
        None => ("Favourite", 0),
    };
    let tools = ui.boxed(c, top, UiStyle { gap: Some(0.6), margin: Some([0.0, 0.0, 0.5, 0.0].into()), ..UiStyle::row() });
    let fav = ui.button(c, tools, Kind::Secondary, label, Some("F"), 1.9, State { disabled: focused.is_none(), ..State::NONE });
    c.entity(fav).insert(MainMenuAction::Favourite(i));
    if internet && hub.signed_in {
        let all = ui.button(c, tools, Kind::Secondary, if cx.lan.all_games { "This game" } else { "All games" }, None, 1.9, State::NONE);
        c.entity(all).insert(MainMenuAction::BrowserAllGames);
    }
    let refresh = ui.button(c, tools, Kind::Secondary, "Refresh", Some("F5"), 1.9, State { disabled: cx.lan.scanning, ..State::NONE });
    c.entity(refresh).insert(MainMenuAction::Refresh);
    // Where the list is (a long one's window, a scan under way, the hub
    // not answering).
    let note = if asking && internet {
        Some(format!("Asking {}...", hub.name))
    } else if asking {
        Some("Looking...".to_string())
    } else if internet && hub.offline.is_some() && hub.has_list {
        Some(format!("{} isn't answering: its last list", hub.name))
    } else if n > VISIBLE_ROWS {
        Some(format!("{}-{} of {n}", first + 1, last))
    } else {
        None
    };
    if let Some(note) = note {
        let e = ui.boxed(c, right, UiStyle { pad: Some([0.0, 0.0, 0.4, 0.0].into()), ..UiStyle::row() });
        ui.text(c, e, Role::Mono, 0.5, Tone::Fg3, &note);
    }
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
            BrowserTab::Internet if !hub.signed_in => {
                "Not signed in to a hub: start the game once with +chimera:hub <your invite link>. LAN and Direct work without one.".to_string()
            }
            BrowserTab::Internet if asking => format!("Asking {} for its servers", hub.name),
            BrowserTab::Internet if !hub.has_list => match &hub.offline {
                Some(why) => format!("{} isn't answering ({why}). LAN, Favourites and Direct still work.", hub.name),
                None => format!("No list from {} yet: press F5", hub.name),
            },
            BrowserTab::Internet if !cx.lan.hub_list.is_empty() => format!("No server of this game is listed on {}", hub.name),
            BrowserTab::Internet => format!("No server is listed on {}", hub.name),
            BrowserTab::Lan if asking => "Looking for servers on your network".to_string(),
            BrowserTab::Lan => "No server answered on your network".to_string(),
            BrowserTab::Favourites => "No favourites yet: focus a server and press F".to_string(),
            BrowserTab::History => "No server joined yet".to_string(),
        };
        ui.sentence(c, e, NOTE, Tone::Fg3, &none);
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
        if sv.status == Status::Up || sv.listed {
            small.extend(sv.minigame.as_deref().map(|m| m.rsplit(':').next().unwrap_or(m).replace('_', " ")));
        }
        match sv.status {
            Status::Up => {}
            Status::Asking => small.push("asking".into()),
            Status::Down => small.push("no answer".into()),
        }
        small.extend(sv.note.clone());
        if let Some(w) = sv.when {
            small.insert(0, sl::ago(cx.unix, w));
        }
        if sv.favourite && cx.lan.tab != BrowserTab::Favourites {
            small.push("favourite".into());
        }
        let small = small.join(" • ");
        let (e, vl) = ui.row(c, list, &sv.name, Some(&small), row_h, State { focus, disabled: !sv.same_version || sv.barred || sv.status == Status::Down, ..State::NONE });
        c.entity(e).insert((PageLine(i), crate::ui_cues::CueItem(format!("page_line:{i}"))));
        if sv.status == Status::Up || sv.listed {
            // (A listed server's numbers are the hub's until it answers.)
            let players = format!("{}/{}", sv.players, sv.max_players);
            let bots = sv.bots.to_string();
            let ping = match sv.status {
                Status::Up => format!("{} ms", sv.ping_ms),
                Status::Asking => "...".to_string(),
                Status::Down => "-".to_string(),
            };
            let tone = if sv.status == Status::Up { Tone::Fg } else { Tone::Fg3 };
            cells(c, ui, vl, [&map_label(&sv.map), &players, &bots, &ping, if sv.locked { "PW" } else { "" }], tone, 0.6);
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
