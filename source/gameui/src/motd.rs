//! The message of the day as a Source game shows it to a player coming
//! into a game (specs/hl2dm/hud.md 14, rules.md "Joining"): the server's
//! message (`net::ServerMotd`) in the game's text window (`map::hud::
//! GameMenus::motd`, HL2:DM's `resource/ui/textwindow.res`), over the game,
//! the mouse free and the game's keys idle while it is up (`VguiOpen::motd`);
//! its OK button (or Enter, the button being the panel's default) closes it.
//!
//! The message: we don't render HTML, so an HTML message shows its text
//! (`net::motd::plain_text`); with `cl_disablehtmlmotd 1` the server's
//! plain-text message shows instead (`motd_shown`). Which box gets it and
//! what the title says is `motd_page`'s (HL2:DM's `HTMLMessage` and
//! `MessageTitle`, CS:S's `Message` and `serverName`).
//!
//! `MotdWindowPlugin` is the window for a game whose players join without
//! menus (HL2:DM): it opens when a local player first appears in a game an
//! interactive run starts (`core::JoinChoice::Menus`; tool runs and tests
//! join straight in). CS:S's join flow (team and class menus after the
//! OK) is its own (`cs_source::ui::join_menus`) and shares the rest.
//!
//! What it shows (hud.md 14, answered 2026-11-09): the title is the
//! server's `hostname`; the `motd` entry by its first characters
//! (`motd_kind`): empty a blank area, `http(s)://` a page, `<` HTML, else
//! plain text; Enter or Space press OK (closing runs no command); the key
//! bound to `showscores` shows the scoreboard over it (the HUD's, drawn
//! above); it opens again on every map entered (`changelevel`). HL2:DM's
//! frame (`motd_frame`): its corner and solid pictures as black boxes at
//! `Frame.BgColor`'s alpha over a blank backdrop.
//!
//! Known shortcuts: a page (`http://`) shows its address as text and HTML
//! its text (no web view); the frame's `hl2mp_logo` isn't drawn (the
//! pictures are boxes, not the install's textures).

use bevy::{prelude::*, window::CursorOptions};
use chimera_client::client::{
    fonts::UiFonts,
    vgui::{self, Painter, Shown, VguiButton, VguiMenu, VguiOpen},
    widgets::Interaction,
};

use crate::{
    console::{Console, resource_cvar},
    core::{JoinChoice, LocalPlayer},
    map::hud::{ActiveHud, GameMenus, UiKind, UiLayout},
    net::ServerMotd,
};

/// `cl_disablehtmlmotd` (default 0).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct DisableHtmlMotd(pub u8);

/// `cl_disablehtmlmotd` (archived; Multiplayer > Advanced).
pub fn motd_cvar(app: &mut App) {
    if app.world().contains_resource::<DisableHtmlMotd>() {
        return;
    }
    app.init_resource::<DisableHtmlMotd>();
    resource_cvar::<DisableHtmlMotd, u8>(
        app,
        "cl_disablehtmlmotd",
        "1: the message of the day as the server's plain text (motd_text.txt) rather than its HTML one.",
        |s| &mut s.0,
    );
    app.world_mut().resource_mut::<Console>().archive("cl_disablehtmlmotd");
}

/// What the `motd` entry is, by its first characters (hud.md 14, "What
/// the client shows").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotdKind {
    /// Empty: the area stays blank.
    Blank,
    /// `http://` or `https://`: that page.
    Url,
    /// Starting `<`: the text as an HTML page.
    Html,
    /// Anything else: plain text.
    Plain,
}

/// The kind of a `motd` entry.
pub fn motd_kind(entry: &str) -> MotdKind {
    let e = entry.trim_start();
    let lower = e.get(..8).unwrap_or(e).to_ascii_lowercase();
    if e.is_empty() {
        MotdKind::Blank
    } else if lower.starts_with("http://") || lower.starts_with("https://") {
        MotdKind::Url
    } else if e.starts_with('<') {
        MotdKind::Html
    } else {
        MotdKind::Plain
    }
}

/// The message of the day as shown (hud.md 14): blank for an empty entry;
/// a page's address as text (we draw no web pages); an HTML message's
/// text, or with `cl_disablehtmlmotd` the `motd_text` entry as plain text
/// (blank when that is empty); plain text as it is.
pub fn motd_shown(motd: &ServerMotd, disable_html: bool) -> String {
    let plain = |t: &str| t.replace('\r', "").trim().to_string();
    match motd_kind(&motd.html) {
        MotdKind::Blank => String::new(),
        MotdKind::Url | MotdKind::Html if disable_html => plain(&motd.text),
        MotdKind::Url => plain(&motd.html),
        MotdKind::Html => crate::net::motd::plain_text(&motd.html),
        MotdKind::Plain => plain(&motd.html),
    }
}

/// The game's message of the day panel.
pub fn motd_layout(menus: &GameMenus) -> Option<&UiLayout> {
    menus.layouts.get(menus.motd.as_ref()?)
}

/// A text window's layout made ready to draw: its message box (CS:S's
/// `Message`, HL2:DM's `HTMLMessage`) as wrapped text, a second plain-text
/// box (`TextMessage`, the same place) hidden, the server's name in the
/// title (`serverName`, `MessageTitle`).
pub fn motd_page(page: &UiLayout, motd: &ServerMotd, message: &str) -> UiLayout {
    let mut page = page.clone();
    for c in &mut page.controls {
        let name = c.name.to_ascii_lowercase();
        match name.as_str() {
            "message" | "htmlmessage" => {
                c.kind = UiKind::RichText;
                c.text = message.to_string();
            }
            "textmessage" => c.visible = false,
            "servername" | "messagetitle" => c.text = motd.server.clone(),
            _ => {}
        }
    }
    page
}

/// Whether the message of the day is up.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MotdWindow {
    pub open: bool,
}

/// The window for a game whose players join without menus (HL2:DM).
pub struct MotdWindowPlugin;

impl Plugin for MotdWindowPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MotdStatePlugin)
            .add_systems(Update, (keys, pointer, draw).chain().after(open_on_join));
    }
}

/// When it opens and closes, without drawing (tests).
pub struct MotdStatePlugin;

impl Plugin for MotdStatePlugin {
    fn build(&self, app: &mut App) {
        motd_cvar(app);
        app.init_resource::<MotdWindow>()
            .add_systems(Update, open_on_join)
            // Again after every map change (`changelevel`): each new map
            // re-activates the client and the server sends it again.
            .add_observer(chimera_client::client::front_end::on_world::<chimera_client::client::front_end::EnteredGame>(reopen));
    }
}

/// A map entered in an interactive run: the window opens again.
fn reopen(w: &mut World) {
    let interactive = w.get_resource::<JoinChoice>().is_some_and(|c| *c == JoinChoice::Menus);
    let playing = w.query_filtered::<(), With<LocalPlayer>>().iter(w).next().is_some();
    if interactive
        && playing
        && let Some(mut m) = w.get_resource_mut::<MotdWindow>()
    {
        m.open = true;
    }
}

/// A local player new in a game opens it, in an interactive run.
fn open_on_join(
    joined: Query<(), Added<LocalPlayer>>,
    choice: Option<Res<JoinChoice>>,
    mut window: ResMut<MotdWindow>,
) {
    if !joined.is_empty() && choice.is_some_and(|c| *c == JoinChoice::Menus) && !window.open {
        window.open = true;
    }
}

/// The OK: closes it.
pub fn motd_ok(w: &mut World) {
    if let Some(mut m) = w.get_resource_mut::<MotdWindow>() {
        m.open = false;
    }
}

fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Single<&CursorOptions>,
    (open, ui, chat, game_menu): (
        Res<VguiOpen>,
        Option<Res<chimera_client::client::console::ConsoleUi>>,
        Option<Res<chimera_client::client::chat::ChatInput>>,
        Option<Res<crate::game_menu::GameMenu>>,
    ),
    mut window: ResMut<MotdWindow>,
) {
    if !window.open || !vgui::keys_live(&cursor, &open, ui.as_deref(), chat.as_deref(), game_menu.is_some_and(|m| m.open)) {
        return;
    }
    // The OK button holds the focus: Enter or Space press it (closing runs
    // no command). Escape isn't the window's (hud.md Open questions 12).
    if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space]) {
        window.open = false;
    }
}

fn pointer(buttons: Query<(&Interaction, &VguiButton), Changed<Interaction>>, mut window: ResMut<MotdWindow>) {
    for (interaction, b) in &buttons {
        if b.menu == VguiMenu::Motd && *interaction == Interaction::Pressed && b.enabled && window.open {
            window.open = false;
        }
    }
}

/// Everything the window spawned.
#[derive(Component)]
struct MotdRoot;

/// Which corner of a frame piece is rounded (the corner pictures').
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounded {
    None,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// HL2:DM's frame behind the text window on a `w` by `h` screen (hud.md
/// 14, "The frame pictures"): the four `round_corner_*` pictures and the
/// `solid_background` strips (top, title band, bottom, main area), in
/// pixels. All black at `BgColor`'s alpha; the `trans_background`
/// margins are practically clear and the viewport's backdrop is blank, so
/// the game shows around and through them.
pub fn motd_frame(w: f32, h: f32) -> Vec<(Rect, Rounded)> {
    let s = (w / 640.0).min(h / 480.0);
    let (ox, oy) = ((w - 640.0 * s) / 2.0, (h - 480.0 * s) / 2.0);
    let r = |x: f32, y: f32, wide: f32, tall: f32| Rect::new(x, y, x + wide, y + tall);
    let c = 10.0 * s;
    vec![
        (r(ox + 20.0 * s, oy + 20.0 * s, c, c), Rounded::TopLeft),
        (r(w - ox - 30.0 * s, oy + 20.0 * s, c, c), Rounded::TopRight),
        (r(ox + 20.0 * s, h - oy - 30.0 * s, c, c), Rounded::BottomLeft),
        (r(w - ox - 30.0 * s, h - oy - 30.0 * s, c, c), Rounded::BottomRight),
        (r(ox + 30.0 * s, oy + 20.0 * s, w - 2.0 * ox - 60.0 * s, c), Rounded::None),
        (r(ox + 20.0 * s, oy + 30.0 * s, w - 2.0 * ox - 40.0 * s, 42.0 * s), Rounded::None),
        (r(ox + 30.0 * s, h - oy - 30.0 * s, w - 2.0 * ox - 60.0 * s, c), Rounded::None),
        (r(ox + 20.0 * s, h - oy - 30.0 * s - 376.0 * s, w - 2.0 * ox - 40.0 * s, 376.0 * s), Rounded::None),
    ]
}

#[allow(clippy::too_many_arguments)]
fn draw(
    window: Res<MotdWindow>,
    motd: Option<Res<ServerMotd>>,
    disable: Res<DisableHtmlMotd>,
    hud: Option<Res<ActiveHud>>,
    fonts: Res<UiFonts>,
    windows: Query<&Window>,
    roots: Query<Entity, With<MotdRoot>>,
    mut open: ResMut<VguiOpen>,
    mut drawn: Local<String>,
    mut commands: Commands,
) {
    let page = hud.as_ref().and_then(|h| h.0.menus.as_ref()).and_then(motd_layout).filter(|_| window.open);
    let (Some(hud), Some(page), Some(win)) = (hud.as_ref(), page, windows.iter().next()) else {
        for e in &roots {
            commands.entity(e).despawn();
        }
        drawn.clear();
        // An unchanged value isn't marked changed (`grab_cursor` reads a
        // change as a menu just closed).
        open.set_if_neq(VguiOpen { motd: false, ..*open });
        return;
    };
    open.set_if_neq(VguiOpen { motd: true, ..*open });
    let motd = motd.map(|m| m.clone()).unwrap_or_default();
    let message = motd_shown(&motd, disable.0 != 0);
    let size = Vec2::new(win.width(), win.height());
    let key = format!("{}|{message}|{size}", motd.server);
    if *drawn == key && !roots.is_empty() && !hud.is_changed() {
        return;
    }
    for e in &roots {
        commands.entity(e).despawn();
    }
    *drawn = key;
    let painter = Painter::new(hud, &fonts, size.y);
    let root = commands
        .spawn((
            MotdRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            // The viewport's backdrop (`ViewportBG`) is blank: the game
            // shows around the frame.
            BackgroundColor(Color::NONE),
            // A click on it stops here; the game menu opens over it.
            bevy::ui::FocusPolicy::Block,
            GlobalZIndex(50),
        ))
        .id();
    // The frame: black pictures at `BgColor`'s alpha.
    let alpha = painter.color("Frame.BgColor", [0, 0, 0, 196]).alpha();
    for (r, round) in motd_frame(size.x, size.y) {
        let k = r.height();
        let radius = match round {
            Rounded::None => BorderRadius::ZERO,
            Rounded::TopLeft => BorderRadius::top_left(bevy::ui::CornerRadius::circular(Val::Px(k))),
            Rounded::TopRight => BorderRadius::top_right(bevy::ui::CornerRadius::circular(Val::Px(k))),
            Rounded::BottomLeft => BorderRadius::bottom_left(bevy::ui::CornerRadius::circular(Val::Px(k))),
            Rounded::BottomRight => BorderRadius::bottom_right(bevy::ui::CornerRadius::circular(Val::Px(k))),
        };
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(r.min.x),
                top: Val::Px(r.min.y),
                width: Val::Px(r.width()),
                height: Val::Px(r.height()),
                border_radius: radius,
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(alpha)),
            ChildOf(root),
        ));
    }
    let a = vgui::area(size.x, size.y);
    let area = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(a.min.x),
                top: Val::Px(a.min.y),
                width: Val::Px(a.width()),
                height: Val::Px(a.height()),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    painter.spawn(&mut commands, area, a.size(), &motd_page(page, &motd, &message), VguiMenu::Motd, &mut Shown::of);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_shown_follows_cl_disablehtmlmotd() {
        let m = ServerMotd {
            server: String::new(),
            html: "<html><head><title>x</title></head><body><pre>Hello\nthere</pre></body></html>".into(),
            text: "Plain hello\r\n".into(),
        };
        assert_eq!(motd_shown(&m, false), "Hello\nthere");
        assert_eq!(motd_shown(&m, true), "Plain hello");
        let html_only = ServerMotd {
            server: String::new(),
            html: m.html.clone(),
            text: String::new(),
        };
        assert_eq!(motd_shown(&html_only, true), "", "no plain one: blank (hud.md 14)");
    }

    /// hud.md 14 (answered 2026-11-09): the entry's first characters decide.
    #[test]
    fn the_entry_decides_what_shows() {
        let m = |html: &str, text: &str| ServerMotd { server: String::new(), html: html.into(), text: text.into() };
        assert_eq!(motd_kind(""), MotdKind::Blank);
        assert_eq!(motd_kind("HTTPS://example.com/motd"), MotdKind::Url);
        assert_eq!(motd_kind("<html>"), MotdKind::Html);
        assert_eq!(motd_kind("Welcome, a < b"), MotdKind::Plain);
        assert_eq!(motd_shown(&m("", "x"), false), "");
        assert_eq!(motd_shown(&m("Welcome, a < b <x>", "x"), true), "Welcome, a < b <x>", "plain text as it is");
        assert_eq!(motd_shown(&m("http://example.com", "text"), true), "text");
    }

    /// hud.md 14, "The frame pictures": at 4:3 the title band ends at 72s
    /// and the main area starts at 74s (a 2-unit line between); the
    /// window is centred on a wide screen.
    #[test]
    fn the_frame_follows_the_spec() {
        let f = motd_frame(640.0, 480.0);
        assert_eq!(f[0], (Rect::new(20.0, 20.0, 30.0, 30.0), Rounded::TopLeft));
        assert_eq!(f[3].0, Rect::new(610.0, 450.0, 620.0, 460.0));
        assert_eq!(f[5].0.max.y, 72.0);
        assert_eq!(f[7].0.min.y, 74.0);
        let wide = motd_frame(1920.0, 1080.0);
        let s = 1080.0 / 480.0;
        let ox = (1920.0 - 640.0 * s) / 2.0;
        assert!((wide[0].0.min.x - (ox + 20.0 * s)).abs() < 1e-3);
        assert!((wide[7].0.width() - (1920.0 - 2.0 * ox - 40.0 * s)).abs() < 1e-3);
    }
}
