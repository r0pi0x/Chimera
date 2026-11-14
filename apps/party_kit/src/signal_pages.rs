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
//! - **Join**: Direct (the address and the password, text fields), the
//!   recent addresses, and the **LAN servers** (main-menu.md M2): every
//!   server of ours `ServerQueries`' LAN scan found (`lan_servers`: name,
//!   minigame from its keywords, map, players/max, bots, ping, a password
//!   mark), sorted by ping; scanned when the page opens, Refresh (F5) again.
//!   Enter on the address or password connects, on a recent address or a
//!   server joins it (a server with a password and none typed: the
//!   password field takes the keys first); a click on a focused row
//!   presses it.
//! - **Keys**: ↑↓ (and Tab) move the focus through the rows, ←→ change a
//!   value, Space toggles, Q/E (PageUp/PageDown) the Play tabs (not while
//!   typing), F5 refreshes the LAN list, Enter the page's button, Esc
//!   back (`main_menu::typing`, which also types into the focused text
//!   field). The pointer: a click focuses a row; the kit's hover ticks.
//! - **Motion and sound**: a page or tab change wipes the rows in on the
//!   12° slant (`Ui::wipe_cover`); the menu's cues (`ui_cues`: hover,
//!   select, back, denied, the steppers' slider).
//!
//! `ui_scene play [server|game]` and `ui_scene join` stage them for
//! screenshots (`join` with made-up LAN servers).
//!
//! Known shortcuts: the LAN list shows the first `LAN_ROWS` servers (no
//! scroll); no Favourites or History tabs and no Internet list (M2's
//! rest, M3); a locked server's password is the Direct one (no dialog);
//! typed text shows in capitals (the kit's faces); no selection shown in
//! a text field.

use std::net::SocketAddr;

use bevy::{prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_client::client::ui_sound::UiCue;
use chimera_net::net::query::{LanSettings, QueryResult, QueryState, ServerQueries};
use chimera_ui::{
    AmbientMaterial, IconImages, KitFonts, Role, Theme, Tone, UiClock, UiStyle, UiUnits,
    controls::{Kind, State, Ui},
};

use crate::{
    host::{Pick, Setting, same_value},
    main_menu::{Field, MainMenuAction, MenuData, MenuMinigame, MenuPage, MenuUiCamera, PartyMenu, PlayKind, Screen, ShownScreen},
};

/// The layout table.
pub const L: &str = "pages";

/// LAN servers shown at most.
pub const LAN_ROWS: usize = 8;

/// The Signal look draws Play and Join (a theme with `[layout.pages]`).
pub fn themed(theme: Option<&Theme>) -> bool {
    theme.is_some_and(|t| t.has_layout(L))
}

/// A server of the LAN list.
#[derive(Clone, Debug, PartialEq)]
pub struct LanServer {
    pub addr: SocketAddr,
    pub name: String,
    pub minigame: Option<String>,
    pub map: String,
    pub players: u8,
    pub max_players: u8,
    pub bots: u8,
    pub ping_ms: u32,
    pub locked: bool,
    /// Its network version is ours (else it can't be joined).
    pub same_version: bool,
}

/// The LAN list from the queries' results: the servers found by a LAN
/// scan that answered, ours only, by ping (then name).
pub fn lan_servers(results: &[QueryResult]) -> Vec<LanServer> {
    let mut out: Vec<LanServer> = results
        .iter()
        .filter(|r| r.lan)
        .filter_map(|r| match &r.state {
            QueryState::Answered { info, ping_ms } if info.is_ours() => Some(LanServer {
                addr: r.addr,
                name: if info.name.trim().is_empty() { r.addr.to_string() } else { info.name.clone() },
                minigame: info.minigame().map(str::to_string),
                map: info.map.clone(),
                players: info.players,
                max_players: info.max_players,
                bots: info.bots,
                ping_ms: *ping_ms,
                locked: info.password,
                same_version: info.version == chimera_net::net::NET_VERSION,
            }),
            _ => None,
        })
        .collect();
    out.sort_by(|a, b| a.ping_ms.cmp(&b.ping_ms).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

/// The LAN list shown on Join.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LanList {
    pub servers: Vec<LanServer>,
    /// A scan (or a query) still under way.
    pub scanning: bool,
    /// The queries' generation the list was made from.
    pub generation: u64,
    /// A staged scene's made-up servers (not replaced by the queries).
    pub staged: bool,
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
            .chain((0..lan.servers.len().min(LAN_ROWS)).map(Line::Server))
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
    /// When the page or tab last changed (the wipe), on the UI clock.
    pub since: f64,
    /// A line in error (an empty address) and the bar's notice.
    pub error: Option<usize>,
    pub notice: Option<String>,
    /// A staged scene's page (`ui_scene play|join`), opened once the menu
    /// is up, and its Play tab.
    pub stage: Option<(MenuPage, usize)>,
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
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(
                Update,
                (follow, keys, line_clicks).chain().before(crate::main_menu::MainMenuSystems).before(crate::esc_menu::EscMenuSystems),
            )
            .add_systems(Update, crate::ui_cues::hover::<PageLine>.in_set(crate::main_menu::MainMenuSystems))
            .add_systems(Update, draw.after(crate::main_menu::MainMenuSystems));
        use chimera_client::client::ui_scene::UiSceneHooks;
        UiSceneHooks::add(app.world_mut(), "play", |w, a| stage(w, MenuPage::Play, a));
        UiSceneHooks::add(app.world_mut(), "join", |w, a| stage(w, MenuPage::Join, a));
    }
}

/// `ui_scene play [server|game]`, `ui_scene join`: the page over the menu
/// (`join` with made-up LAN servers).
pub fn stage(w: &mut World, page: MenuPage, args: &[String]) -> Result<(), String> {
    if args.first().is_some_and(|a| a == "off") {
        w.resource_mut::<SignalPages>().stage = None;
        w.resource_mut::<LanList>().staged = false;
        return Ok(());
    }
    let tab = match args.first().map(|a| a.to_ascii_lowercase()) {
        Some(a) if a == "game" || a == "1" => 1,
        _ => 0,
    };
    w.resource_mut::<SignalPages>().stage = Some((page, tab));
    if page == MenuPage::Join {
        let mut l = w.resource_mut::<LanList>();
        l.servers = staged_servers();
        l.staged = true;
        l.scanning = false;
    }
    Ok(())
}

/// Made-up LAN servers for the staged Join.
fn staged_servers() -> Vec<LanServer> {
    let s = |ip: [u8; 4], port: u16, name: &str, map: &str, p: u8, max: u8, bots: u8, ping: u32, locked: bool| LanServer {
        addr: SocketAddr::from((ip, port)),
        name: name.into(),
        minigame: Some("ultimate_gun_game:auto".into()),
        map: map.into(),
        players: p,
        max_players: max,
        bots,
        ping_ms: ping,
        locked,
        same_version: true,
    };
    vec![
        s([192, 168, 1, 20], 27015, "Friday ladder", "de_dust2", 7, 8, 4, 3, false),
        s([192, 168, 1, 31], 27016, "Back room", "warehouse", 3, 12, 1, 9, true),
        s([192, 168, 1, 44], 27015, "Knife party", "cs_office", 10, 16, 6, 21, false),
    ]
}

/// Scan the LAN again (Refresh).
pub fn scan(world: &mut World) {
    let lan = world.get_resource::<LanSettings>().cloned().unwrap_or_default();
    if let Some(mut q) = world.get_resource_mut::<ServerQueries>() {
        q.scan_lan(lan.port_range(), &lan.broadcast_addresses());
    }
    if let Some(mut l) = world.get_resource_mut::<LanList>() {
        l.staged = false;
        l.scanning = true;
    }
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
) {
    if !themed(theme.as_deref()) {
        return;
    }
    if let Some((page, tab)) = p.stage
        && menu.shown()
        && menu.screen == Screen::MainMenu
    {
        menu.failure = None;
        menu.page = page;
        menu.play_tab = tab;
        menu.focus = (page == MenuPage::Join).then_some(Field::Address);
        p.stage = None;
    }
    if !lan.staged
        && let Some(q) = queries.as_deref()
        && q.generation != lan.generation
    {
        lan.generation = q.generation;
        lan.servers = lan_servers(&q.results);
        lan.scanning = q.busy();
    }
    if lan.scanning && queries.as_deref().is_some_and(|q| !q.busy()) && !lan.staged {
        lan.scanning = false;
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
        if up.is_none() {
            return;
        }
        let ls = lines(&menu, &data, &lan);
        focus_line(&mut p, &mut menu, &data, &ls, 0);
        if opened
            && up == Some(MenuPage::Join)
            && !lan.staged
            && let Some(q) = queries.as_deref_mut()
        {
            let s = lan_settings.as_deref().cloned().unwrap_or_default();
            q.scan_lan(s.port_range(), &s.broadcast_addresses());
            lan.scanning = true;
        }
        return;
    }
    if up.is_none() {
        return;
    }
    // The menu's text field moved (a click on a field, `mainmenu_type`):
    // the focus follows.
    let ls = lines(&menu, &data, &lan);
    if let Some(f) = menu.focus
        && ls.get(p.focus).and_then(|l| l.field(&data)) != Some(f)
        && let Some(i) = ls.iter().position(|l| l.field(&data) == Some(f))
    {
        p.focus = i;
    }
    if p.focus >= ls.len() && !ls.is_empty() {
        let last = ls.len() - 1;
        focus_line(&mut p, &mut menu, &data, &ls, last);
    }
}

/// What pressing line `i` does (Enter, a click on the focused line):
/// None when it is refused (and why in the notice).
fn press(line: Line, menu: &PartyMenu, lan: &LanList, p: &mut SignalPages) -> Option<MainMenuAction> {
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
            let s = lan.servers.get(i)?;
            if !s.same_version {
                p.notice = Some(format!("{} runs another version", s.name));
                return None;
            }
            if s.locked && menu.password.is_empty() {
                p.error = Some(1);
                p.notice = Some(format!("{} has a password: type it, then join again", s.name));
                return None;
            }
            Some(MainMenuAction::Server(i))
        }
    }
}

/// The keys (module doc).
#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    console: Option<Res<chimera_client::client::console::ConsoleUi>>,
    data: Res<MenuData>,
    lan: Res<LanList>,
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
    let ls = lines(&menu, &data, &lan);
    let line = ls.get(p.focus).copied();
    let typing = line.and_then(|l| l.field(&data)).is_some();
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let up = k(KeyCode::ArrowUp) || (k(KeyCode::Tab) && shift);
    let down = k(KeyCode::ArrowDown) || (k(KeyCode::Tab) && !shift);
    if (up || down) && !ls.is_empty() {
        let next = if up { p.focus.saturating_sub(1) } else { (p.focus + 1).min(ls.len() - 1) };
        if next != p.focus {
            focus_line(&mut p, &mut menu, &data, &ls, next);
            cues.write(UiCue::new("ui_hover"));
        }
        return;
    }
    if k(KeyCode::Enter) || k(KeyCode::NumpadEnter) {
        let pressed = match (page, line) {
            (MenuPage::Play, _) => Some(MainMenuAction::Start),
            (_, Some(l)) => press(l, &menu, &lan, &mut p),
            _ => None,
        };
        match pressed {
            Some(a) => {
                out.write(a);
            }
            None => {
                cues.write(UiCue::new("ui_denied"));
                if p.error == Some(1)
                    && let Some(i) = ls.iter().position(|l| *l == Line::Password)
                {
                    focus_line(&mut p, &mut menu, &data, &ls, i);
                }
            }
        }
        return;
    }
    if k(KeyCode::F5) && page == MenuPage::Join {
        out.write(MainMenuAction::Refresh);
        return;
    }
    if typing {
        return;
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
    lan: Res<LanList>,
    mut menu: ResMut<PartyMenu>,
    mut p: ResMut<SignalPages>,
    mut out: MessageWriter<MainMenuAction>,
    mut cues: MessageWriter<UiCue>,
) {
    if p.up.is_none() {
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
            match press(line, &menu, &lan, &mut p) {
                Some(a) => {
                    out.write(a);
                }
                None => {
                    cues.write(UiCue::new("ui_denied"));
                }
            }
        } else {
            focus_line(&mut p, &mut menu, &data, &ls, l.0);
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
            let list = ui.boxed(c, cols, UiStyle { grow: Some(1.0), shrink: Some(1.0), ..UiStyle::column() });
            for (i, l) in ls.iter().enumerate() {
                play_line(c, ui, list, i, *l, menu, p, cx, row_h);
            }
            ui.wipe_cover(c, list, Tone::Ink, p.since, cx.now, lay(t, "wipe", 0.26));
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
            lan_list(c, ui, right, &ls, menu, p, cx, row_h);
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
        _ => &[("↑↓", "Row"), ("Enter", "Join"), ("F5", "Refresh"), ("Esc", "Back")],
    };
    for (k, what) in keys {
        let g = ui.boxed(c, foot, UiStyle { gap: Some(0.5), ..UiStyle::row() });
        ui.kbd(c, g, k, 0.6, Tone::Fg);
        ui.text(c, g, Role::Mono, 0.6, Tone::Fg2, what);
    }
    root
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
                    let caret = (menu.focus == Some(Field::Play(r))).then_some(menu.caret.at);
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
            let caret = (menu.focus == Some(field)).then_some(menu.caret.at);
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

#[allow(clippy::too_many_arguments)]
fn lan_list(c: &mut Commands, ui: &Ui, right: Entity, ls: &[Line], menu: &PartyMenu, p: &SignalPages, cx: &Ctx, row_h: f32) {
    let _ = menu;
    let n = cx.lan.servers.len();
    let note = if cx.lan.scanning {
        "Looking...".to_string()
    } else {
        match n {
            0 => "None found".to_string(),
            1 => "1 found".to_string(),
            n => format!("{n} found, by ping"),
        }
    };
    let head = ui.boxed(c, right, UiStyle { gap: Some(1.0), ..UiStyle::row() });
    let sec = ui.boxed(c, head, UiStyle { grow: Some(1.0), ..UiStyle::column() });
    ui.section(c, sec, "LAN servers", Some(&note));
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
    for (i, l) in ls.iter().enumerate() {
        let Line::Server(s) = l else { continue };
        let Some(sv) = cx.lan.servers.get(*s) else { continue };
        let focus = p.focus == i;
        let small = sv.minigame.as_deref().map(|m| m.rsplit(':').next().unwrap_or(m).replace('_', " "));
        let (e, vl) = ui.row(c, right, &sv.name, small.as_deref(), row_h, State { focus, disabled: !sv.same_version, ..State::NONE });
        c.entity(e).insert(PageLine(i));
        let players = format!("{}/{}", sv.players, sv.max_players);
        let bots = sv.bots.to_string();
        let ping = format!("{} ms", sv.ping_ms);
        cells(c, ui, vl, [&map_label(&sv.map), &players, &bots, &ping, if sv.locked { "PW" } else { "" }], Tone::Fg, 0.6);
    }
    if n > LAN_ROWS {
        let more = ui.boxed(c, right, UiStyle { pad: Some([0.5, 0.0, 0.0, 1.1].into()), ..UiStyle::row() });
        ui.text(c, more, Role::Mono, 0.52, Tone::Fg3, &format!("And {} more", n - LAN_ROWS));
    }
}
