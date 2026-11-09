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
//! Known shortcuts: HL2:DM's own frame behind the window (its rounded
//! corners, the solid and translucent backgrounds, the logo) isn't drawn:
//! hud.md 14 names the pictures and sizes but not which part each fills,
//! so the window draws on the scheme's `Frame.BgColor`; its title shows the
//! server's name (the spec doesn't say what the server sends as the
//! title).

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
        app.init_resource::<MotdWindow>().add_systems(Update, open_on_join);
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
    // The OK button is the panel's default: Enter presses it.
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
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
            BackgroundColor(painter.color("Frame.BgColor", [0, 0, 0, 196])),
            // A click on it stops here; the game menu opens over it.
            bevy::ui::FocusPolicy::Block,
            GlobalZIndex(50),
        ))
        .id();
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
        assert_eq!(motd_shown(&html_only, true), "Hello\nthere", "no plain one: the HTML one's text");
    }
}
