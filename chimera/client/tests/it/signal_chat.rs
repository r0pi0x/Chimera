//! The one chat in the Signal look (`client::signal_chat`, plan ugg-ui
//! M10): only under a theme that turns the Signal HUD on (else the game's
//! own chat lines); notices drawn apart from players' lines (a slanted
//! mark, no name); a line held 8.5 s, then faded over 1 s and gone; the
//! field takes the keys while it is open (the game sees none), Enter
//! sends and Esc closes; a mute or `cl_chat` hides lines, past ones too;
//! docked in the lobby, the history and the field show with no line said,
//! and Enter opens it there.

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use chimera_client::client::{
    chat::{ChatEntry, ChatFilter, ChatInput, ChatLine, ChatMutes, LineKind, SignalChatActive},
    prematch_lobby::{LobbyRoot, LobbyView},
    signal::UiOpacity,
    signal_chat::{ChatDockRoot, ChatPlayRoot, FeedLineId, SignalChatLog, SignalChatPlugin},
};
use chimera_core::core::LocalPlayer;
use chimera_net::net::NetCharacter;
use chimera_sim::SimBasePlugins;
use chimera_ui::{
    Theme, UiClock, UiCue,
    chat::{ChatField, ChatLineNode, ChatName, NoticeMark},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

const SIGNAL: &str = "name = \"test\"\n[flags.hud]\nsignal = true\n";

fn sim(signal: bool) -> Sim {
    let mut sim = Sim::new(SimBasePlugins, (GreyboxMapPlugin, SignalChatPlugin));
    if signal {
        sim.app.insert_resource(Theme::from_toml(SIGNAL).expect("theme"));
    }
    sim.app.insert_resource(UiClock::manual());
    frame(&mut sim, 0.0);
    sim
}

fn frame(sim: &mut Sim, dt: f32) {
    sim.app.world_mut().resource_mut::<UiClock>().step(dt);
    sim.app.update();
}

fn line(kind: LineKind, sender: Option<Entity>, name: Option<&str>, text: &str) -> ChatEntry {
    ChatEntry { kind, sender, name: name.map(str::to_string), team: None, text: text.into(), classic: Some(ChatLine::plain(text)), sound: None }
}

fn lines(sim: &mut Sim) -> Vec<(LineKind, bool, bool)> {
    let w = sim.app.world_mut();
    let rows: Vec<(Entity, LineKind)> = w.query::<(Entity, &ChatLineNode)>().iter(w).map(|(e, k)| (e, k.0)).collect();
    let mut out = Vec::new();
    for (e, kind) in rows {
        let kids: Vec<Entity> = w.get::<Children>(e).map(|c| c.iter().collect()).unwrap_or_default();
        let mark = kids.iter().any(|k| w.get::<NoticeMark>(*k).is_some());
        let name = kids.iter().any(|k| w.get::<ChatName>(*k).is_some());
        out.push((kind, mark, name));
    }
    out
}

fn opacities(sim: &mut Sim) -> Vec<f32> {
    let w = sim.app.world_mut();
    w.query_filtered::<&UiOpacity, With<FeedLineId>>().iter(w).map(|o| o.0).collect()
}

fn count<C: Component>(sim: &mut Sim) -> usize {
    let w = sim.app.world_mut();
    w.query_filtered::<(), With<C>>().iter(w).count()
}

fn key(sim: &mut Sim, code: KeyCode, logical: Key) {
    let window = Entity::PLACEHOLDER;
    sim.app.world_mut().write_message(KeyboardInput { key_code: code, logical_key: logical, state: ButtonState::Pressed, text: None, repeat: false, window });
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(code);
}

#[test]
fn without_the_signal_look_the_games_chat_shows_the_lines() {
    let mut sim = sim(false);
    let mut seen = sim.app.world().resource::<Messages<ChatLine>>().get_cursor_current();
    sim.app.world_mut().write_message(line(LineKind::All, None, Some("Ann"), "hello"));
    frame(&mut sim, 0.1);
    assert!(!sim.app.world().resource::<SignalChatActive>().0);
    let classic: Vec<ChatLine> = seen.read(sim.app.world().resource::<Messages<ChatLine>>()).cloned().collect();
    assert_eq!(classic, [ChatLine::plain("hello")], "the game's own line");
    assert!(sim.app.world().resource::<SignalChatLog>().lines.is_empty());
    assert_eq!(count::<ChatPlayRoot>(&mut sim), 0);
}

#[test]
fn notices_are_drawn_apart_from_players_lines() {
    let mut sim = sim(true);
    let mut seen = sim.app.world().resource::<Messages<ChatLine>>().get_cursor_current();
    let ann = sim.app.world_mut().spawn(NetCharacter { owner: Some(3), name: "Ann".into() }).id();
    sim.app.world_mut().write_message(ChatEntry::notice("Ann is on the last level!", None));
    sim.app.world_mut().write_message(line(LineKind::All, Some(ann), Some("Ann"), "gg"));
    sim.app.world_mut().write_message(line(LineKind::System, None, None, "You are flooding the server: wait a moment."));
    frame(&mut sim, 0.1);
    frame(&mut sim, 0.0);
    assert!(sim.app.world().resource::<SignalChatActive>().0);
    let mut got = lines(&mut sim);
    got.sort_by_key(|(k, ..)| format!("{k:?}"));
    assert_eq!(
        got,
        [(LineKind::All, false, true), (LineKind::Notice, true, false), (LineKind::System, false, false)],
        "a notice has the mark and no name; a player's line its name"
    );
    let classic = seen.read(sim.app.world().resource::<Messages<ChatLine>>()).count();
    assert_eq!(classic, 0, "the game's own chat shows none of them");
}

#[test]
fn a_line_holds_then_fades_and_goes() {
    let mut sim = sim(true);
    let ann = sim.app.world_mut().spawn(NetCharacter { owner: Some(3), name: "Ann".into() }).id();
    sim.app.world_mut().write_message(line(LineKind::All, Some(ann), Some("Ann"), "hi"));
    frame(&mut sim, 0.0);
    frame(&mut sim, 0.0);
    assert_eq!(count::<FeedLineId>(&mut sim), 1);
    frame(&mut sim, 8.4);
    assert_eq!(opacities(&mut sim), [1.0], "held 8.5 s");
    frame(&mut sim, 0.6);
    let o = opacities(&mut sim);
    assert!(o.len() == 1 && (o[0] - 0.5).abs() < 0.02, "half faded 0.5 s into the fade: {o:?}");
    frame(&mut sim, 0.6);
    frame(&mut sim, 0.0);
    assert_eq!(count::<FeedLineId>(&mut sim), 0, "gone after 9.5 s");
    // Still in the history: opening it shows the line again, unfaded.
    sim.app.world_mut().resource_mut::<ChatInput>().open = Some(false);
    frame(&mut sim, 0.0);
    assert_eq!(count::<FeedLineId>(&mut sim), 1);
    assert_eq!(count::<ChatField>(&mut sim), 1);
}

#[test]
fn the_open_field_takes_the_keys_and_enter_sends() {
    let mut sim = sim(true);
    sim.app.world_mut().spawn((LocalPlayer, Name::new("Me")));
    sim.app.world_mut().resource_mut::<chimera_core::console::Console>().submit("messagemode");
    frame(&mut sim, 0.0);
    assert_eq!(sim.app.world().resource::<ChatInput>().open, Some(false), "messagemode opens it");
    key(&mut sim, KeyCode::KeyW, Key::Character("W".into()));
    key(&mut sim, KeyCode::KeyA, Key::Character("a".into()));
    frame(&mut sim, 0.0);
    assert_eq!(sim.app.world().resource::<ChatInput>().text, "Wa", "typed, case kept");
    let keys = sim.app.world().resource::<ButtonInput<KeyCode>>();
    assert!(!keys.pressed(KeyCode::KeyW) && !keys.just_pressed(KeyCode::KeyA), "the game sees no key");
    // Tab: to the team; Enter sends.
    key(&mut sim, KeyCode::Tab, Key::Tab);
    frame(&mut sim, 0.0);
    assert_eq!(sim.app.world().resource::<ChatInput>().open, Some(true));
    key(&mut sim, KeyCode::Enter, Key::Enter);
    frame(&mut sim, 0.0);
    frame(&mut sim, 0.0);
    assert_eq!(sim.app.world().resource::<ChatInput>().open, None, "closed");
    let log = sim.app.world().resource::<SignalChatLog>();
    let sent: Vec<(LineKind, &str, bool)> = log.lines.iter().map(|l| (l.kind, l.text.as_str(), l.own)).collect();
    assert_eq!(sent, [(LineKind::Team, "Wa", true)]);
    assert!(!sim.app.world().resource::<ButtonInput<KeyCode>>().just_pressed(KeyCode::Enter), "Enter not seen by the binds");
    // Esc drops a typed line.
    sim.app.world_mut().resource_mut::<chimera_core::console::Console>().submit("messagemode2");
    frame(&mut sim, 0.0);
    key(&mut sim, KeyCode::KeyX, Key::Character("x".into()));
    key(&mut sim, KeyCode::Escape, Key::Escape);
    frame(&mut sim, 0.0);
    assert_eq!(*sim.app.world().resource::<ChatInput>(), ChatInput::default());
    assert_eq!(sim.app.world().resource::<SignalChatLog>().lines.len(), 1, "nothing sent");
}

#[test]
fn mutes_and_cl_chat_hide_lines_and_their_cue() {
    let mut sim = sim(true);
    let mut cues = sim.app.world().resource::<Messages<UiCue>>().get_cursor_current();
    let ann = sim.app.world_mut().spawn(NetCharacter { owner: Some(3), name: "Ann".into() }).id();
    let bot = sim.app.world_mut().spawn((NetCharacter { owner: None, name: "Rook".into() }, chimera_core::core::Bot)).id();
    sim.app.world_mut().write_message(line(LineKind::All, Some(ann), Some("Ann"), "one"));
    sim.app.world_mut().write_message(line(LineKind::All, Some(bot), Some("Rook"), "beep"));
    frame(&mut sim, 0.1);
    let heard = |sim: &Sim, c: &mut bevy::ecs::message::MessageCursor<UiCue>| c.read(sim.app.world().resource::<Messages<UiCue>>()).filter(|c| c.event == "chat_in").count();
    assert_eq!(heard(&sim, &mut cues), 1, "one cue for two lines at once (rate-limited)");
    let shown = |sim: &Sim| -> Vec<String> {
        let w = sim.app.world();
        w.resource::<SignalChatLog>().shown(w.resource::<ChatMutes>(), *w.resource::<ChatFilter>()).map(|l| l.text.clone()).collect()
    };
    assert_eq!(shown(&sim), ["one", "beep"]);
    let tags: Vec<_> = sim.app.world().resource::<SignalChatLog>().lines.iter().map(|l| l.tag).collect();
    assert_eq!(tags, [None, Some(chimera_ui::chat::NameTag::Bot)], "the bot's tag");
    sim.app.world_mut().resource_mut::<chimera_core::console::Console>().submit("ignore rook");
    frame(&mut sim, 0.0);
    assert_eq!(shown(&sim), ["one"], "the bot's line hidden, by name");
    sim.app.world_mut().resource_mut::<ChatMutes>().set(Some(3), "Ann", true);
    frame(&mut sim, 2.0);
    sim.app.world_mut().write_message(line(LineKind::All, Some(ann), Some("Ann"), "two"));
    frame(&mut sim, 0.1);
    assert!(shown(&sim).is_empty(), "Ann muted by user id");
    assert_eq!(heard(&sim, &mut cues), 0, "no cue for a muted line");
    sim.app.world_mut().resource_mut::<chimera_core::console::Console>().submit("unignore ann");
    sim.app.world_mut().resource_mut::<chimera_core::console::Console>().submit("cl_chat 0");
    frame(&mut sim, 0.0);
    sim.app.world_mut().write_message(ChatEntry::notice("Bo joined the game", None));
    frame(&mut sim, 0.0);
    assert_eq!(shown(&sim), ["Bo joined the game"], "cl_chat 0: notices only");
}

#[test]
fn docked_in_the_lobby_the_history_and_field_show_and_enter_opens_it() {
    let mut sim = sim(true);
    let lobby = sim.app.world_mut().spawn((LobbyRoot, Node::default())).id();
    sim.app.world_mut().insert_resource(LobbyView { shown: true, ..default() });
    frame(&mut sim, 0.0);
    assert_eq!(count::<ChatDockRoot>(&mut sim), 1, "docked with nothing said yet");
    assert_eq!(count::<ChatPlayRoot>(&mut sim), 0);
    let dock = {
        let w = sim.app.world_mut();
        w.query_filtered::<(Entity, &ChildOf), With<ChatDockRoot>>().single(w).unwrap().1.parent()
    };
    assert_eq!(dock, lobby, "under the lobby's root");
    assert_eq!(count::<ChatField>(&mut sim), 1, "the field waits");
    sim.app.world_mut().write_message(ChatEntry::notice("Ann joined the game", None));
    frame(&mut sim, 0.0);
    frame(&mut sim, 30.0);
    assert_eq!(count::<FeedLineId>(&mut sim), 1, "the lobby keeps the history, unfaded");
    key(&mut sim, KeyCode::Enter, Key::Enter);
    frame(&mut sim, 0.0);
    assert_eq!(sim.app.world().resource::<ChatInput>().open, Some(false), "Enter opens it in the lobby");
}

/// One z-order rule for the chat against the games' HUDs: a game's whole
/// HUD is one layer (`GAME_HUD_Z`; HL2:DM's and TF2's pooled nodes order
/// themselves under it by a local `ZIndex`), the chat is above it and
/// below the scoreboards and menus.
#[test]
fn the_chat_is_above_every_games_hud_and_below_the_menus() {
    use chimera_client::client::{
        hud::GAME_HUD_Z,
        signal_chat::CHAT_Z,
        widgets::GAME_UI_Z,
    };
    assert!(GAME_HUD_Z < CHAT_Z && CHAT_Z < GAME_UI_Z, "{GAME_HUD_Z} < {CHAT_Z} < {GAME_UI_Z}");
    let mut sim = sim(true);
    sim.app.world_mut().write_message(ChatEntry::notice("hello", None));
    frame(&mut sim, 0.1);
    frame(&mut sim, 0.0);
    let w = sim.app.world_mut();
    let layers: Vec<i32> = w.query_filtered::<&GlobalZIndex, With<ChatPlayRoot>>().iter(w).map(|z| z.0).collect();
    assert_eq!(layers, [CHAT_Z], "the chat's one layer");
}

/// Report 0cd30d86: on the match end (the chat docked) with the report
/// dialog open, Y (`messagemode`) and Enter type into the report and never
/// open the chat behind it (the dialog's keys run first and are reset;
/// the chat waits while it shows).
#[test]
fn the_report_dialog_keeps_y_and_enter_from_the_docked_chat() {
    use chimera_client::client::{
        feedback::{AssetsOutbox, Feedback, FeedbackOutbox, FeedbackPlugin, Phase},
        match_end::MatchEndView,
    };
    let folder = std::env::temp_dir().join(format!("chimera-chat-report-{}", std::process::id()));
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.insert_resource(FeedbackOutbox(std::sync::Arc::new(AssetsOutbox::at(
            folder.clone(),
            chimera_core::mount::assets::feedback::Targets::default(),
        ))));
        app.add_plugins((GreyboxMapPlugin, SignalChatPlugin, FeedbackPlugin));
    });
    sim.app.insert_resource(Theme::from_toml(SIGNAL).expect("theme"));
    sim.app.insert_resource(UiClock::manual());
    sim.app.world_mut().resource_mut::<chimera_core::console::Console>().binds.insert("y".into(), "messagemode".into());
    sim.app.world_mut().insert_resource(MatchEndView { shown: true, ..default() });
    frame(&mut sim, 0.0);
    sim.app.world_mut().resource_mut::<Feedback>().phase = Phase::Open;
    for (code, logical) in [(KeyCode::KeyY, Key::Character("y".into())), (KeyCode::Enter, Key::Enter)] {
        key(&mut sim, code, logical);
        frame(&mut sim, 0.0);
        assert_eq!(sim.app.world().resource::<ChatInput>().open, None, "{code:?} doesn't open the chat");
        assert!(!sim.app.world().resource::<ButtonInput<KeyCode>>().just_pressed(code), "{code:?} reset for the rest");
        let mut keys = sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(code);
        keys.clear();
    }
    assert_eq!(sim.app.world().resource::<Feedback>().text, "y\n", "both typed into the report");
    // Closed, Enter opens the docked chat again.
    sim.app.world_mut().resource_mut::<Feedback>().phase = Phase::Closed;
    key(&mut sim, KeyCode::Enter, Key::Enter);
    frame(&mut sim, 0.0);
    assert_eq!(sim.app.world().resource::<ChatInput>().open, Some(false), "Enter opens the docked chat");
    let _ = std::fs::remove_dir_all(&folder);
}
