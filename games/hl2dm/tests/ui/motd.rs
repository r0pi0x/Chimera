//! HL2:DM's message of the day (specs/hl2dm/hud.md 14, rules.md "Joining"):
//! a player coming into a game an interactive run starts sees it, a tool run
//! doesn't; Enter (the OK being the default button) closes it and the
//! mouse goes back to the game; the message goes into the window's message
//! box with the server's name as its title (`source_gameui::motd`).

use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use chimera_client::client::{fonts::UiFonts, vgui::VguiOpen};
use chimera_core::{
    console::ConsolePlugin,
    core::{JoinChoice, LocalPlayer},
};
use chimera_net::net::ServerMotd;
use chimera_world::map::hud::UiKind;
use source_gameui::motd::{MotdWindow, MotdWindowPlugin, motd_page, motd_shown};

fn app(choice: JoinChoice) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, ConsolePlugin, MotdWindowPlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<VguiOpen>()
        .init_resource::<UiFonts>()
        .insert_resource(choice);
    app.world_mut().spawn(CursorOptions { grab_mode: CursorGrabMode::Locked, ..default() });
    app.update();
    app
}

fn open(app: &App) -> bool {
    app.world().resource::<MotdWindow>().open
}

#[test]
fn a_joining_player_sees_it_and_enter_closes_it() {
    let mut a = app(JoinChoice::Menus);
    assert!(!open(&a), "no player yet");
    a.world_mut().spawn(LocalPlayer);
    a.update();
    assert!(open(&a), "opened on joining");
    // Enter: the OK.
    a.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Enter);
    a.update();
    assert!(!open(&a), "closed");
    assert!(!a.world().resource::<VguiOpen>().motd);

    // A tool run (or a test) joins straight in.
    let mut a = app(JoinChoice::Auto);
    a.world_mut().spawn(LocalPlayer);
    a.update();
    assert!(!open(&a));
}

/// The text window's boxes (our own file of `textwindow.res`'s shape): the
/// message in the HTML box as text, the plain-text box hidden, the server's
/// name as the title; `cl_disablehtmlmotd` picks the plain-text message.
#[test]
fn the_message_goes_into_its_box() {
    let res = r#""x"
    {
        "HTMLMessage" { "ControlName" "HTML" "fieldName" "HTMLMessage" "xpos" "76" "ypos" "116" "wide" "480" "tall" "240" }
        "TextMessage" { "ControlName" "TextEntry" "fieldName" "TextMessage" "xpos" "76" "ypos" "116" "wide" "480" "tall" "240" }
        "MessageTitle" { "ControlName" "Label" "fieldName" "MessageTitle" "labelText" "Title" }
        "ok" { "ControlName" "Button" "fieldName" "ok" "labelText" "OK" "command" "okay" }
    }"#;
    let root = source_formats::keyvalues::parse_pc(res);
    let root = root.items().first().map(|(_, v)| v.clone()).unwrap();
    let layout = source_engine::hud::vgui::layout(&root, &Default::default(), &Default::default(), &mut |_| None);
    let motd = ServerMotd {
        server: "Our server".into(),
        html: "<html><body><pre>Welcome\nto the game</pre></body></html>".into(),
        text: "Plain welcome".into(),
    };
    let page = motd_page(&layout, &motd, &motd_shown(&motd, false));
    let c = |n: &str| page.controls.iter().find(|c| c.name == n).unwrap().clone();
    assert_eq!((c("HTMLMessage").kind, c("HTMLMessage").text.as_str()), (UiKind::RichText, "Welcome\nto the game"));
    assert!(!c("TextMessage").visible);
    assert_eq!(c("MessageTitle").text, "Our server");
    assert_eq!(motd_shown(&motd, true), "Plain welcome");
}
