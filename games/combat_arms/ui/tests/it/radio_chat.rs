//! The HUD's chat panel, input bar and radio menu
//! (specs/combat_arms/radio_chat.md P1, C2, R2; ui_system.md H) on the
//! front end's state (`ui::talk`, headless): the game's lines in the
//! spec's queue with their colours and lives, the input and its tabs
//! typed with real keyboard messages, the radio menu's window and its
//! number keys, the Esc menu staying shut while typing.

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use chimera_core::core::LocalPlayer;
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use combat_arms::{game::{
        chat::{
            self, CaChat, CaSayRequest, now_ms,
            client::Tab,
            radio,
            show::{PanelLine, colours},
        },
        movement,
    }, harness::CaSim};
use combat_arms_ui::{
        chat::PLAYER_LIFE,
        menu::{CaMenuStatePlugin, InGameMenu},
        CaUi,
        layout::Strings,
        talk::{CaTalk, CaTalkKeysPlugin, CaTalkStatePlugin},
    };

fn sim() -> Sim {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, CaTalkStatePlugin, CaTalkKeysPlugin, CaMenuStatePlugin));
    });
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.app.world_mut().entity_mut(p).insert((LocalPlayer, Name::new("A1")));
    sim.ticks(3);
    sim
}

fn talk(sim: &Sim) -> &CaTalk {
    sim.app.world().resource::<CaTalk>()
}

fn line(sim: &mut Sim, text: &str, colour: [u8; 4], life: f64) {
    let w = sim.app.world_mut();
    let now = now_ms(w);
    w.resource_mut::<CaChat>().log.push(PanelLine::new(text, colour, now, life));
}

fn key(sim: &mut Sim, code: KeyCode, logical: Key) {
    let w = sim.app.world_mut();
    w.write_message(KeyboardInput {
        key_code: code,
        logical_key: logical,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    w.resource_mut::<ButtonInput<KeyCode>>().press(code);
    sim.ticks(1);
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(code);
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
}

fn typed(sim: &mut Sim, s: &str) {
    for c in s.chars() {
        key(sim, KeyCode::KeyA, Key::Character(c.to_string().into()));
    }
}

/// P1: lines come into the queue with their colour and life, in order;
/// a string id is resolved with the table.
#[test]
fn lines_enter_the_queue() {
    let mut sim = sim();
    line(&mut sim, "A1 : hi", colours::ALL_TEAMMATE, PLAYER_LIFE);
    line(&mut sim, "$SameSentence", colours::SAME_SENTENCE, 5000.0);
    sim.ticks(2);
    let q = &talk(&sim).queue.messages;
    assert_eq!(q.len(), 2);
    assert_eq!((q[0].text.as_str(), q[0].colour, q[0].life), ("A1 : hi", colours::ALL_TEAMMATE, 10_000.0));
    assert_eq!(q[1].text, "Please do not repeat yourself.");
    assert_eq!(q[1].life, 5000.0);
    // Each line once, however many frames pass.
    sim.ticks(5);
    assert_eq!(talk(&sim).queue.messages.len(), 2);
}

/// H: lines past their life go, one oldest per expired line; the stack is
/// kept inside the window (180 ref px).
#[test]
fn queue_expiry_and_overflow() {
    let mut sim = sim();
    line(&mut sim, "system", colours::SYSTEM, 5000.0);
    sim.ticks(2);
    sim.seconds(5.5);
    assert!(talk(&sim).queue.messages.is_empty(), "past its 5000 ms life");
    for i in 0..40 {
        line(&mut sim, &format!("line {i}"), colours::TEAM, PLAYER_LIFE);
    }
    sim.ticks(2);
    let q = &talk(&sim).queue;
    assert!(q.total() <= 180, "{}", q.total());
    assert!(q.messages.len() < 40 && q.messages.back().unwrap().text == "line 39");
}

/// C2: Enter opens the input on the current tab; F3 switches the tab;
/// typed text appears; Up cycles whisper partners; Esc closes and clears.
#[test]
fn input_bar_keys() {
    let mut sim = sim();
    assert!(chat::open_enter(sim.app.world_mut()));
    sim.ticks(1);
    assert_eq!(talk(&sim).input.as_ref().map(|i| i.tab), Some(Tab::All));
    typed(&mut sim, "hi");
    assert_eq!(talk(&sim).input.as_ref().unwrap().text, "hi");
    key(&mut sim, KeyCode::F3, Key::F3);
    assert_eq!(talk(&sim).input.as_ref().unwrap().tab, Tab::Team);
    key(&mut sim, KeyCode::F4, Key::F4);
    assert_eq!(talk(&sim).input.as_ref().unwrap().tab, Tab::Clan);
    key(&mut sim, KeyCode::Backspace, Key::Backspace);
    assert_eq!(talk(&sim).input.as_ref().unwrap().text.chars().count(), 1);
    key(&mut sim, KeyCode::PageUp, Key::PageUp);
    key(&mut sim, KeyCode::Escape, Key::Escape);
    assert!(talk(&sim).input.is_none());
    // The tab persists: Enter opens on the Clan tab.
    sim.app.world_mut().resource_mut::<CaChat>().sent_this_frame = false;
    assert!(chat::open_enter(sim.app.world_mut()));
    sim.ticks(1);
    assert_eq!(talk(&sim).input.as_ref().unwrap().tab, Tab::Clan);
}

/// Enter sends the typed line as a request and closes the input; the
/// Enter that sent does not reopen it.
#[test]
fn enter_sends_and_closes() {
    let mut sim = sim();
    assert!(chat::open_enter(sim.app.world_mut()));
    sim.app.world_mut().resource_mut::<CaChat>().type_text("go go");
    key(&mut sim, KeyCode::Enter, Key::Enter);
    let sent: Vec<CaSayRequest> = sim.app.world_mut().resource_mut::<Messages<CaSayRequest>>().drain().collect();
    // A game of one serves the request in the same frame; either the
    // request is still there or its line came back.
    let came_back = sim.app.world().resource::<CaChat>().log.lines.iter().any(|l| l.text == "A1 : go go");
    assert!(sent.iter().any(|s| s.text == "go go") || came_back);
    assert!(!sim.app.world().resource::<CaChat>().is_open());
}

/// R2: the radio menu is a window with its title and entries; a number
/// key picks (the key never reaches the weapon selection); 0 does
/// nothing.
#[test]
fn radio_menu_window_and_keys() {
    let mut sim = sim();
    chat::radio_key(sim.app.world_mut(), radio::REPORT);
    sim.ticks(1);
    let menu = talk(&sim).radio.clone().expect("the window");
    assert_eq!(menu.title, "Report");
    assert!(!menu.lines.is_empty() && menu.lines.len() <= 9);
    // 0 leaves it open.
    key(&mut sim, KeyCode::Digit0, Key::Character("0".into()));
    assert!(talk(&sim).radio.is_some());
    // 1 picks entry 1: a request, the menu closes, the key was swallowed.
    let w = sim.app.world_mut();
    w.write_message(KeyboardInput {
        key_code: KeyCode::Digit1,
        logical_key: Key::Character("1".into()),
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    w.resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    sim.ticks(1);
    assert!(
        !sim.app.world().resource::<ButtonInput<KeyCode>>().just_pressed(KeyCode::Digit1),
        "the weapon key doesn't see the pick"
    );
    sim.ticks(2);
    assert!(talk(&sim).radio.is_none());
    let calls = sim.app.world().resource::<CaChat>().last_radio;
    assert!(calls.is_some(), "the cooldown is stamped");
}

/// Corrections 2026-10-22 (1): only the keys 1 to 6 pick a radio entry;
/// 7 to 9 do nothing and the menu stays open (the line is shown, though).
#[test]
fn radio_keys_seven_to_nine_pick_nothing() {
    let mut sim = sim();
    {
        let mut radio = sim.app.world_mut().resource_mut::<chat::CaRadio>();
        let class = &mut radio.0.classes[radio::REPORT as usize];
        let first = class.entries[0].clone();
        class.entries = vec![first; 9];
    }
    chat::radio_key(sim.app.world_mut(), radio::REPORT);
    sim.ticks(1);
    assert_eq!(talk(&sim).radio.as_ref().map(|m| m.lines.len()), Some(9));
    for (code, c) in [(KeyCode::Digit7, "7"), (KeyCode::Digit8, "8"), (KeyCode::Digit9, "9")] {
        key(&mut sim, code, Key::Character(c.into()));
        assert!(talk(&sim).radio.is_some(), "{c} leaves the menu open");
        assert!(sim.app.world().resource::<CaChat>().last_radio.is_none(), "{c} sent nothing");
    }
    let w = sim.app.world_mut();
    w.resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit6);
    sim.ticks(3);
    assert!(talk(&sim).radio.is_none(), "6 picks entry 6");
    assert!(sim.app.world().resource::<CaChat>().last_radio.is_some());
}

/// The install's radio data lists its entries by string id
/// (`$RadioMsg_Report_list_N`): the menu shows the table's text (the
/// number the text carries once), and a built-in line where the table has
/// no such id, never the raw id.
#[test]
fn radio_menu_resolves_string_ids() {
    let mut sim = sim();
    {
        let w = sim.app.world_mut();
        let mut radio = w.resource_mut::<chat::CaRadio>();
        let class = &mut radio.0.classes[radio::REPORT as usize];
        class.entries.truncate(3);
        class.entries[0].list_text = "$RadioMsg_Report_list_1".into();
        class.entries[1].list_text = "$RadioMsg_Report_list_2".into();
        class.entries[2].list_text = "$RadioMsg_Report_list_missing".into();
        class.entries[2].chat = vec!["Fallback line".into()];
        let mut ui = w.resource_mut::<CaUi>();
        ui.strings = Strings::parse("RadioMsg_Report_list_1 = \"1. Roger\"\nRadioMsg_Report_list_2 = \"Negative\"\n");
    }
    chat::radio_key(sim.app.world_mut(), radio::REPORT);
    sim.ticks(1);
    let menu = talk(&sim).radio.clone().expect("the window");
    assert_eq!(menu.lines, vec!["Roger", "Negative", "Fallback line"]);
}

#[test]
fn radio_text_helpers() {
    use combat_arms_ui::talk::{shown_id, without_number};
    let t = Strings::parse("a = \"x\"");
    assert_eq!(shown_id(&t, "$a", "f"), "x");
    assert_eq!(shown_id(&t, "$b", "f"), "f");
    assert_eq!(shown_id(&t, "plain", "f"), "plain");
    assert_eq!(without_number("12. Go"), "Go");
    assert_eq!(without_number("Go. now"), "Go. now");
}

/// Number keys are the game's when no menu is open.
#[test]
fn number_keys_pass_without_a_menu() {
    let mut sim = sim();
    let w = sim.app.world_mut();
    w.resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    sim.ticks(1);
    // The input plugin of a real app clears it a frame later; nobody
    // here swallowed it.
    assert!(sim.app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::Digit1));
}

/// I2: Esc closes the chat input and does not open the in-game menu; with
/// no input open it opens the menu.
#[test]
fn esc_closes_chat_before_the_menu() {
    let mut sim = sim();
    assert!(chat::open_enter(sim.app.world_mut()));
    key(&mut sim, KeyCode::Escape, Key::Escape);
    assert!(!sim.app.world().resource::<CaChat>().is_open());
    assert!(!sim.app.world().resource::<InGameMenu>().open, "the Esc was the input's");
    key(&mut sim, KeyCode::Escape, Key::Escape);
    assert!(sim.app.world().resource::<InGameMenu>().open);
}

/// The talk icon follows push-to-talk.
#[test]
fn talk_icon() {
    let mut sim = sim();
    chat::voice_key(sim.app.world_mut(), true);
    sim.ticks(1);
    assert!(talk(&sim).talking);
    chat::voice_key(sim.app.world_mut(), false);
    sim.ticks(1);
    assert!(!talk(&sim).talking);
}

/// The console reads the same keyboard messages as the chat input. With
/// no chat input open the talk keys must leave them for it (they used to
/// drain every frame, so the console could not be typed into), and with
/// the console open the chat input and radio menu get none.
#[test]
fn console_keeps_its_keys() {
    use chimera_client::client::console::ConsoleUi;
    #[derive(Resource, Default)]
    struct Seen(Vec<String>);
    fn console_reads(mut ev: MessageReader<KeyboardInput>, mut seen: ResMut<Seen>) {
        for e in ev.read() {
            if let Key::Character(s) = &e.logical_key {
                seen.0.push(s.to_string());
            }
        }
    }
    let mut sim = sim();
    sim.app.init_resource::<Seen>().init_resource::<ConsoleUi>().add_systems(Update, console_reads);
    // Console open, no chat: every typed key reaches the console.
    sim.app.world_mut().resource_mut::<ConsoleUi>().open = true;
    typed(&mut sim, "sv_gravity");
    assert_eq!(sim.app.world().resource::<Seen>().0.concat(), "sv_gravity");
    // No chat typing happened.
    assert!(!sim.app.world().resource::<CaChat>().is_open());
    // With a chat input open under an open console, the chat takes none.
    assert!(chat::open_enter(sim.app.world_mut()));
    sim.app.world_mut().resource_mut::<Seen>().0.clear();
    typed(&mut sim, "ab");
    assert_eq!(sim.app.world().resource::<Seen>().0.concat(), "ab");
    assert_eq!(sim.app.world().resource::<CaChat>().input.clone().unwrap_or_default(), "");
}

/// The number that picks a radio command doesn't also pick a weapon: the
/// slot bind doesn't see the key in the frame it picks, nor after the menu
/// has closed with the key still down; let go and pressed again, it does.
#[test]
fn the_pick_key_never_reaches_the_slot_binds() {
    use chimera_client::client::binds::{CapturedKeys, just_pressed_free, pressed_free, release_captured};
    use std::collections::BTreeMap;
    let mut sim = sim();
    sim.app.init_resource::<CapturedKeys>().add_systems(PreUpdate, release_captured.after(bevy::input::InputSystems));
    let binds: BTreeMap<String, String> = [("4".to_string(), "slot4".to_string())].into();
    let mouse = ButtonInput::<MouseButton>::default();
    let sees = |sim: &Sim, just: bool| {
        let w = sim.app.world();
        let (keys, cap) = (w.resource::<ButtonInput<KeyCode>>(), w.resource::<CapturedKeys>());
        if just {
            just_pressed_free(&binds, keys, &mouse, Some(cap), "slot4")
        } else {
            pressed_free(&binds, keys, &mouse, Some(cap), "slot4")
        }
    };
    chat::radio_key(sim.app.world_mut(), radio::REPORT);
    sim.ticks(1);
    // Press 4 (entry 4 of the built-in report menu, or the last one).
    let w = sim.app.world_mut();
    w.write_message(KeyboardInput {
        key_code: KeyCode::Digit4,
        logical_key: Key::Character("4".into()),
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    w.resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit4);
    sim.ticks(1);
    assert!(!sees(&sim, true) && !sees(&sim, false), "not in the frame it picks");
    // Held on, the menu closed (or open on a missing entry): still not.
    sim.ticks(3);
    assert!(sim.app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::Digit4));
    assert!(!sees(&sim, false), "not after the menu closed, the key still down");
    // Let go: forgotten; pressed again, the slot sees it.
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::Digit4);
    sim.ticks(1);
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit4);
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear_just_pressed(KeyCode::Digit4);
    assert!(sees(&sim, false), "after closing, the number switches weapons again");
}

/// A radio call's line shows the message from the viewer's string table
/// (the ids travel as indices: each client resolves in its language), the
/// name's colour span moves with the text, and where the table has no
/// such id the line shows the id without its `$`, never the raw key.
#[test]
fn a_radio_line_resolves_its_string_ids() {
    use combat_arms::game::chat::CaRadioIn;
    let mut sim = sim();
    {
        let w = sim.app.world_mut();
        let mut radio = w.resource_mut::<chat::CaRadio>();
        let class = &mut radio.0.classes[radio::REPORT as usize];
        class.name = "$radio_class_report".into();
        class.entries.truncate(2);
        class.entries[0].chat = vec!["$radio_message_report_chat_1".into()];
        class.entries[1].chat = vec!["$radio_message_report_chat_missing".into()];
        let mut ui = w.resource_mut::<CaUi>();
        ui.strings = Strings::parse("radio_class_report = \"Report\"\nradio_message_report_chat_1 = \"Enemy spotted!\"\n");
    }
    let me = sim.app.world_mut().query_filtered::<Entity, With<LocalPlayer>>().single(sim.app.world()).unwrap();
    for entry in [0u8, 1] {
        sim.app.world_mut().write_message(CaRadioIn {
            sender: Some(me),
            name: "A1".into(),
            name_colour: 0x00ff00,
            class: radio::REPORT,
            entry,
            ..default()
        });
        sim.ticks(2);
    }
    let q = &talk(&sim).queue.messages;
    let texts: Vec<_> = q.iter().map(|m| m.text.clone()).collect();
    assert_eq!(
        texts,
        vec!["[Report] A1 : Enemy spotted!", "[Report] A1 : radio_message_report_chat_missing"]
    );
    let (a, b) = {
        let n = q[0].name.expect("the name's span");
        (n.0, n.1)
    };
    assert_eq!(&texts[0][a..b], "A1");
}
