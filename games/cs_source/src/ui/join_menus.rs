//! CS:S's join flow: a player who comes into a game (`core::Unassigned`:
//! a network client let in, or the player of a game it starts with the
//! `map` command or `--map`) first reads the server's message of the day
//! (`net::ServerMotd`) in the game's info panel; its OK opens the team
//! menu (`team_menu`); picking the terrorists or counter-terrorists there
//! opens the class menu (`joinclass`: the player model, or auto-select),
//! and the player spawns as the rules allow (`rules::join_class`). Until
//! it picks a team it watches as a spectator does, with no body.
//!
//! With the game's layouts (`map::hud::GameMenus::motd`, `::class`) the
//! panels draw as the game draws them (`crate::client::vgui`); without, plain
//! lists. The class menu's picture of a class is CS:S's live view of its
//! model (`CSClassImagePanel`), which isn't drawn: its box stays empty. We don't render HTML: an HTML message shows its text
//! (`net::motd::plain_text`); with `cl_disablehtmlmotd 1` the server's
//! plain-text message shows instead, as CS:S does.
//!
//! Whether the flow runs is `rules::JoinChoice` (`client::Args::join_choice`;
//! tests and scripted runs join a team at once).
//!
//! CS:S's join flow: a player coming into a game on no team
//! (`core::Unassigned`: joining a server, or a game started with `map`/`--map`,
//! `start_game`) reads the message of the day (`net::ServerMotd` in the game's
//! `motd.res`, the server's name over it; HTML shown as its text,
//! `cl_disablehtmlmotd` the plain-text one), OK opens the team menu, a team
//! opens the class menu (`classmenu_ter.res`/`classmenu_ct.res`, the hovered
//! class's picture; `joinclass`), else plain lists; `motd` shows it again;
//! `JoinMenusStatePlugin` follows the player without drawing (tests). Whether
//! it runs: `rules::JoinChoice` (`Args::join_choice`, `--join`). The game
//! menu (Esc) draws over them and takes their clicks and keys while open;
//! they stay up under it and work again once it closes, as under CS:S's
//! GameUI.

use crate::client::widgets::Interaction;
use bevy::{prelude::*, window::CursorOptions};

use crate::client::fonts::UiFonts;
use super::team_menu::TeamMenu;
use crate::client::vgui::{Painter, Shown, VguiButton, VguiMenu, VguiOpen};
use crate::{
    console::{Console, ConsoleAppExt, resource_cvar},
    core::{ChoosingClass, LocalPlayer, NetRole, PlayerClass, Spectating, Team, Unassigned},
    map::hud::{ActiveHud, GameMenus, UiKind, UiLayout},
    net::ServerMotd,
    rules::JoinChoice,
};

pub struct JoinMenusPlugin;

impl Plugin for JoinMenusPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(JoinMenusStatePlugin)
            .add_systems(Update, (keys, pointer, draw, draw_vgui).chain().after(follow_player));
    }
}

/// The game-look panels' clicks and drawing, in `set` (tests of what
/// draws over them).
#[cfg(test)]
pub(super) fn add_panels(app: &mut App, set: impl SystemSet) {
    app.add_systems(Update, (pointer, draw_vgui).chain().in_set(set));
}

/// Which join menu is open, following the local player, without drawing
/// them (tests: `NetSim` clients).
pub struct JoinMenusStatePlugin;

impl Plugin for JoinMenusStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(crate::client::front_end::on_world::<crate::client::front_end::StartGame>(start_game));
        app.init_resource::<JoinMenus>()
            .init_resource::<TeamMenu>()
            .add_systems(Update, follow_player)
            .console_command("motd", "Show the server's message of the day.", |w, _| {
                let mut m = w.resource_mut::<JoinMenus>();
                m.motd = true;
                m.class = None;
                Ok(None)
            });
        motd_cvar(app);
    }
}

/// The message of the day's OK: on to the team menu.
pub fn motd_ok(w: &mut World) {
    w.resource_scope(|w, mut menus: Mut<JoinMenus>| {
        if menus.motd {
            motd_done(&mut menus, &mut w.resource_mut::<TeamMenu>());
        }
    });
}

/// `cl_disablehtmlmotd` (archived; Multiplayer > Advanced).
pub(crate) fn motd_cvar(app: &mut App) {
    app.init_resource::<DisableHtmlMotd>();
    resource_cvar::<DisableHtmlMotd, u8>(
        app,
        "cl_disablehtmlmotd",
        "1: the message of the day as the server's plain text (motd_text.txt) rather than its HTML one.",
        |d| &mut d.0,
    );
    app.world_mut().resource_mut::<Console>().archive("cl_disablehtmlmotd");
}

/// `cl_disablehtmlmotd` (CS:S's default 0).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct DisableHtmlMotd(pub u8);

/// Which join menu is open.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct JoinMenus {
    /// The message of the day.
    pub motd: bool,
    /// It opened as we came into the game: its OK opens the team menu
    /// (else, the `motd` command, it just closes).
    pub joining: bool,
    /// The class menu, for this team (`core::Team` number).
    pub class: Option<u8>,
    /// The team menu asked for this team: its class menu opens once we're
    /// on it.
    pub class_after_team: Option<u8>,
    /// The class menu's button the pointer is over (its picture shows).
    hover: Option<String>,
}

/// The message of the day as shown: the HTML one's text, or with
/// `cl_disablehtmlmotd` the plain-text one (its HTML one's text when the
/// server has none).
pub fn motd_shown(motd: &ServerMotd, disable_html: bool) -> String {
    if disable_html && !motd.text.trim().is_empty() {
        motd.text.replace('\r', "").trim().to_string()
    } else {
        crate::net::motd::plain_text(&motd.html)
    }
}

/// A game the local player starts (`map`, `--map`): it comes in as
/// `rules::JoinChoice` says: on no team with the message of the day open
/// (`Menus`), or straight onto a team (with an auto-selected class).
/// A network client's server decides for it (`net::server`).
pub fn start_game(w: &mut World) {
    if w.get_resource::<NetRole>() == Some(&NetRole::Client) {
        return;
    }
    let Some(player) = w.query_filtered::<Entity, With<LocalPlayer>>().iter(w).next() else {
        return;
    };
    let choice = w.get_resource::<JoinChoice>().copied().unwrap_or_default();
    let joined = match choice {
        JoinChoice::Menus => {
            crate::rules::make_unassigned(w, player);
            open_motd(w);
            return;
        }
        JoinChoice::Team(t) if t.0 == 0 => crate::rules::join_spectators(w, player),
        JoinChoice::Team(t) => crate::rules::join_team(w, player, t),
        JoinChoice::Auto => {
            if w.get::<Unassigned>(player).is_none() {
                return;
            }
            let team = crate::rules::auto_assign(w);
            crate::rules::join_team(w, player, team)
        }
    };
    if let Err(e) = joined {
        warn!("joining: {e}");
    }
    if w.get::<ChoosingClass>(player).is_some() {
        let _ = crate::rules::join_class(w, player, 0);
    }
}

fn open_motd(w: &mut World) {
    if let Some(mut m) = w.get_resource_mut::<JoinMenus>() {
        m.motd = true;
        m.joining = true;
        m.class = None;
        m.class_after_team = None;
    }
    if let Some(mut t) = w.get_resource_mut::<TeamMenu>() {
        t.0 = false;
    }
}

/// The menus follow the local player: coming into the game on no team
/// opens the message of the day; joining a team from the team menu (or
/// waiting for a class) opens the class menu.
#[allow(clippy::type_complexity)]
fn follow_player(
    player: Option<Single<(Entity, &Team, Has<Unassigned>, Has<ChoosingClass>, Has<Spectating>), With<LocalPlayer>>>,
    mut menus: ResMut<JoinMenus>,
    mut team_menu: ResMut<TeamMenu>,
    mut last: Local<Option<(Entity, bool, bool)>>,
) {
    let Some(p) = player else {
        *last = None;
        return;
    };
    let (e, team, unassigned, choosing, spectating) = *p;
    let (was_unassigned, was_choosing) = match *last {
        Some((le, u, c)) if le == e => (u, c),
        _ => (false, false),
    };
    *last = Some((e, unassigned, choosing));
    if unassigned && !was_unassigned {
        menus.motd = true;
        menus.joining = true;
        menus.class = None;
        menus.class_after_team = None;
        team_menu.0 = false;
    }
    if !unassigned && menus.motd && menus.joining {
        // On a team some other way (`jointeam` typed): the message goes.
        menus.motd = false;
    }
    if team_menu.is_changed() && team_menu.0 {
        // Menus close each other.
        menus.motd = false;
        menus.class = None;
    }
    let on_team = !spectating && (1..=2).contains(&team.0);
    if choosing && !was_choosing && on_team {
        menus.class = Some(team.0);
    }
    if let Some(t) = menus.class_after_team
        && on_team
        && team.0 == t
    {
        menus.class = Some(t);
        menus.class_after_team = None;
    }
    if menus.class.is_some_and(|c| !on_team || c != team.0) {
        menus.class = None;
    }
}

/// The class menu's console line for a button's command (None: close).
fn class_line(command: &str) -> Option<String> {
    let command = command.trim();
    (!command.eq_ignore_ascii_case("vguicancel")).then(|| command.to_string())
}

/// The OK of the message of the day: on to the team menu.
fn motd_done(menus: &mut JoinMenus, team_menu: &mut TeamMenu) {
    menus.motd = false;
    if std::mem::take(&mut menus.joining) {
        team_menu.0 = true;
    }
}

#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Single<&CursorOptions>,
    (open, ui, chat, game_menu): (
        Res<VguiOpen>,
        Option<Res<crate::client::console::ConsoleUi>>,
        Option<Res<crate::client::chat::ChatInput>>,
        Option<Res<super::game_menu::GameMenu>>,
    ),
    mut menus: ResMut<JoinMenus>,
    mut team_menu: ResMut<TeamMenu>,
    mut console: ResMut<Console>,
    hud: Option<Res<ActiveHud>>,
) {
    if !crate::client::vgui::keys_live(&cursor, &open, ui.as_deref(), chat.as_deref(), game_menu.is_some_and(|m| m.open)) {
        return;
    }
    let game = hud.as_ref().and_then(|h| h.0.menus.as_ref());
    if menus.motd {
        let ok = game.and_then(motd_layout).and_then(|l| {
            let key = crate::client::vgui::digit(&keys)?;
            crate::client::vgui::hotkey_button(l, key, &mut Shown::of)
        });
        if ok.is_some() || keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) || keys.just_pressed(KeyCode::Space) {
            motd_done(&mut menus, &mut team_menu);
        }
        return;
    }
    let Some(team) = menus.class else { return };
    let Some(key) = crate::client::vgui::digit(&keys) else { return };
    if let Some(page) = game.and_then(|m| class_layout(m, team)) {
        if let Some(command) = crate::client::vgui::hotkey_button(page, key, &mut Shown::of).and_then(|c| c.command.clone()) {
            if let Some(l) = class_line(&command) {
                console.submit(l);
            }
            menus.class = None;
        }
        return;
    }
    // The plain list: 1-4 the team's classes, 6 auto-select (as the game numbers it), 0 closes.
    let per = PlayerClass::PER_TEAM;
    match key.to_digit(10) {
        Some(n @ 1..=4) => {
            console.submit(format!("joinclass {}", (team - 1) * per + n as u8));
            menus.class = None;
        }
        Some(6) => {
            console.submit("joinclass 0");
            menus.class = None;
        }
        Some(0) => menus.class = None,
        _ => {}
    }
}

/// Clicks on the game-look panels' buttons; the pointer over a class
/// shows its picture.
fn pointer(
    buttons: Query<(&Interaction, &VguiButton), Changed<Interaction>>,
    mut menus: ResMut<JoinMenus>,
    mut team_menu: ResMut<TeamMenu>,
    mut console: ResMut<Console>,
) {
    for (interaction, b) in &buttons {
        match (b.menu, interaction) {
            (VguiMenu::Motd, Interaction::Pressed) if menus.motd && b.enabled => {
                motd_done(&mut menus, &mut team_menu);
                return;
            }
            (VguiMenu::Class, Interaction::Hovered) if menus.class.is_some() => {
                if menus.hover.as_deref() != Some(b.name.as_str()) {
                    menus.hover = Some(b.name.clone());
                }
            }
            (VguiMenu::Class, Interaction::Pressed) if menus.class.is_some() && b.enabled => {
                if let Some(command) = &b.command {
                    if let Some(l) = class_line(command) {
                        console.submit(l);
                    }
                    menus.class = None;
                    menus.hover = None;
                    return;
                }
            }
            _ => {}
        }
    }
}

/// The game's message of the day panel.
fn motd_layout(menus: &GameMenus) -> Option<&UiLayout> {
    menus.layouts.get(menus.motd.as_ref()?)
}

/// The game's class menu for a team.
fn class_layout(menus: &GameMenus, team: u8) -> Option<&UiLayout> {
    menus.layouts.get(menus.class.get(&team)?)
}

/// The class names of the plain list, by class number - 1.
const CLASS_NAMES: [&str; 8] = [
    "Phoenix Connexion",
    "Elite Crew",
    "Arctic Avengers",
    "Guerilla Warfare",
    "SEAL Team 6",
    "GSG-9",
    "SAS",
    "GIGN",
];

#[derive(Component)]
struct PlainText;

/// The plain panels, without the game's layouts.
#[allow(clippy::too_many_arguments)]
fn draw(
    menus: Res<JoinMenus>,
    motd: Option<Res<ServerMotd>>,
    disable: Res<DisableHtmlMotd>,
    shown: Query<Entity, With<PlainText>>,
    windows: Query<&Window>,
    hud: Option<Res<ActiveHud>>,
    fonts: Res<UiFonts>,
    mut commands: Commands,
) {
    if !menus.is_changed() && !motd.as_ref().is_some_and(|m| m.is_changed()) && !disable.is_changed() {
        return;
    }
    for e in &shown {
        commands.entity(e).despawn();
    }
    let game = hud.as_ref().and_then(|h| h.0.menus.as_ref());
    let text = if menus.motd && game.and_then(motd_layout).is_none() {
        let message = motd.as_ref().map(|m| motd_shown(m, disable.0 != 0)).unwrap_or_default();
        let server = motd.as_ref().map(|m| m.server.clone()).unwrap_or_default();
        format!("{server}\n\n{message}\n\nEnter  OK")
    } else if let Some(team) = menus.class.filter(|t| game.and_then(|m| class_layout(m, *t)).is_none()) {
        let first = (team as usize - 1) * PlayerClass::PER_TEAM as usize;
        let mut t = String::from("SELECT A CLASS\n\n");
        for (i, name) in CLASS_NAMES[first..first + 4].iter().enumerate() {
            t.push_str(&format!("{}  {name}\n", i + 1));
        }
        t.push_str("\n6  Auto-Select\n\n0  Close");
        t
    } else {
        return;
    };
    let h = windows.iter().next().map_or(480.0, |w| w.height());
    let scale = h / 480.0;
    commands.spawn((
        PlainText,
        Text::new(text),
        fonts.client("Default", h, 12.0),
        TextColor(Color::srgb_u8(255, 176, 0)),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(4.0),
            top: Val::Percent(30.0),
            max_width: Val::Percent(60.0),
            padding: UiRect::all(Val::Px(10.0 * scale)),
            ..default()
        },
        GlobalZIndex(50),
    ));
}

/// Everything a game-look panel spawned.
#[derive(Component)]
struct VguiRoot;

/// What the game-look panels show, to redraw on change.
#[derive(Default, PartialEq)]
struct Drawn {
    key: String,
    /// The class menu's picture panel and its size.
    info: Option<(Entity, Vec2)>,
    info_shown: Option<String>,
}

/// The message of the day's layout made ready to draw: its message box
/// (the `Message` HTML control) as wrapped text, the server's name in
/// `serverName`.
fn motd_page(page: &UiLayout, motd: &ServerMotd, message: &str) -> UiLayout {
    let mut page = page.clone();
    for c in &mut page.controls {
        if c.name.eq_ignore_ascii_case("message") {
            c.kind = UiKind::RichText;
            c.text = message.to_string();
        } else if c.name.eq_ignore_ascii_case("servername") {
            c.text = motd.server.clone();
        }
    }
    page
}

#[allow(clippy::too_many_arguments)]
fn draw_vgui(
    menus: Res<JoinMenus>,
    motd: Option<Res<ServerMotd>>,
    disable: Res<DisableHtmlMotd>,
    hud: Option<Res<ActiveHud>>,
    fonts: Res<UiFonts>,
    windows: Query<&Window>,
    roots: Query<Entity, With<VguiRoot>>,
    mut open: ResMut<VguiOpen>,
    mut drawn: Local<Drawn>,
    mut commands: Commands,
) {
    let game = hud.as_ref().and_then(|h| h.0.menus.as_ref());
    let motd_page_src = game.filter(|_| menus.motd).and_then(motd_layout);
    let class_page = menus.class.and_then(|t| Some((t, class_layout(game?, t)?)));
    let window = windows.iter().next();
    let (Some(hud), Some(game), Some(window)) = (hud.as_ref(), game, window) else {
        clear(&roots, &mut open, &mut drawn, &mut commands);
        return;
    };
    let motd = motd.map(|m| m.clone()).unwrap_or_default();
    let message = motd_shown(&motd, disable.0 != 0);
    let (page, menu) = match (motd_page_src, class_page) {
        (Some(p), _) => (motd_page(p, &motd, &message), VguiMenu::Motd),
        (None, Some((_, p))) => (p.clone(), VguiMenu::Class),
        (None, None) => {
            clear(&roots, &mut open, &mut drawn, &mut commands);
            return;
        }
    };
    let want = VguiOpen {
        motd: menu == VguiMenu::Motd,
        class: menu == VguiMenu::Class,
        ..*open
    };
    open.set_if_neq(want);
    let size = Vec2::new(window.width(), window.height());
    let key = format!("{menu:?}|{:?}|{}|{message}|{size}", menus.class, motd.server);
    let painter = Painter::new(hud, &fonts, size.y);
    if drawn.key != key || roots.is_empty() || hud.is_changed() {
        for e in &roots {
            commands.entity(e).despawn();
        }
        let root = commands
            .spawn((
                VguiRoot,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(painter.color("Frame.BgColor", [0, 0, 0, 196])),
                // A click on it stops here. The game menu opens over it
                // (`game_menu::MENU_Z`), its backdrop taking the clicks
                // until it closes.
                bevy::ui::FocusPolicy::Block,
                GlobalZIndex(50),
            ))
            .id();
        let a = crate::client::vgui::area(size.x, size.y);
        let area = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(a.min.x),
                    top: px(a.min.y),
                    width: px(a.width()),
                    height: px(a.height()),
                    ..default()
                },
                ChildOf(root),
            ))
            .id();
        // The class's picture panel is shown as the menu fills it.
        let parts = painter.spawn(&mut commands, area, a.size(), &page, menu, &mut |c| {
            let mut s = Shown::of(c);
            if c.name.eq_ignore_ascii_case("classinfo") {
                s.visible = true;
            }
            s
        });
        drawn.key = key;
        drawn.info = parts.get("classinfo").copied();
        drawn.info_shown = None;
    }
    if menu != VguiMenu::Class {
        return;
    }
    // The class's picture: the hovered button's, else the first's.
    let item = menus
        .hover
        .as_deref()
        .and_then(|name| page.controls.iter().find(|c| c.name == name && c.info.is_some()))
        .or_else(|| page.controls.iter().find(|c| c.kind == UiKind::Button && c.info.is_some() && c.visible));
    let info = item.and_then(|c| c.info.clone());
    if drawn.info_shown != info {
        drawn.info_shown = info.clone();
        if let Some((panel, size)) = drawn.info {
            commands.entity(panel).despawn_related::<Children>();
            if let Some(layout) = info.and_then(|i| game.layouts.get(&i)) {
                painter.spawn(&mut commands, panel, size, layout, VguiMenu::Class, &mut Shown::of);
            }
        }
    }
}

/// `open` stays a `ResMut` so an unchanged value isn't marked changed:
/// `grab_cursor` reads a change as a menu just closed and won't grab.
fn clear(roots: &Query<Entity, With<VguiRoot>>, open: &mut ResMut<VguiOpen>, drawn: &mut Drawn, commands: &mut Commands) {
    for e in roots {
        commands.entity(e).despawn();
    }
    *drawn = Drawn::default();
    let closed = VguiOpen {
        motd: false,
        class: false,
        ..**open
    };
    open.set_if_neq(closed);
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
        assert_eq!(motd_shown(&html_only, true), "Hello\nthere", "no plain one: the HTML one's text");
    }

    /// A frame with no join menu must not mark `VguiOpen` changed:
    /// `grab_cursor` reads a change as a menu just closed and keeps the
    /// cursor free (alt-tab and clicks stopped grabbing).
    #[test]
    fn nothing_open_leaves_vgui_open_unchanged() {
        #[derive(Resource, Default)]
        struct Changed(u32);
        let mut app = App::new();
        app.init_resource::<JoinMenus>()
            .init_resource::<DisableHtmlMotd>()
            .init_resource::<crate::client::fonts::UiFonts>()
            .init_resource::<VguiOpen>()
            .init_resource::<Changed>()
            .add_systems(
                Update,
                (draw_vgui, |open: Res<VguiOpen>, mut n: ResMut<Changed>| n.0 += open.is_changed() as u32).chain(),
            );
        app.update();
        app.world_mut().resource_mut::<Changed>().0 = 0;
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<Changed>().0, 0);
    }

    #[test]
    fn class_buttons_become_console_lines() {
        assert_eq!(class_line("joinclass 3").as_deref(), Some("joinclass 3"));
        assert_eq!(class_line(" vguicancel "), None);
    }
}
