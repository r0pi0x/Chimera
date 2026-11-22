//! Bots' chat lines (bots.md 10) and the weapons' display names
//! (hud_crosshair.md "Weapon name", attribute_data.md `$` strings) in the
//! front end: a bot's string-table key shows as the table's text, or as
//! our own readable line when the table lacks it, never as a raw `$` id;
//! the item file's weapon names reach the HUD's `ItemNames`.

use bevy::prelude::*;
use chimera_client::client::chat::ChatLine;
use combat_arms::game::bots::chat::{self, CaBotChat};
use combat_arms_ui::{CaUi, hud, install, layout::Strings};
use lithtech_formats::attributes::Document;

/// The text of every line `bot_chat` wrote for `keys`, with `strings` as
/// the game's table.
fn said(strings: Strings, keys: &[&str]) -> Vec<String> {
    let mut app = App::new();
    app.add_message::<CaBotChat>()
        .add_message::<ChatLine>()
        .insert_resource(CaUi { strings, ..default() })
        .add_systems(Update, combat_arms_ui::bot_chat);
    let bot = app.world_mut().spawn(Name::new("AI_Alpha_3")).id();
    for k in keys {
        app.world_mut().write_message(CaBotChat { bot, key: (*k).into() });
    }
    app.update();
    let mut lines = app.world_mut().resource_mut::<Messages<ChatLine>>();
    lines.drain().map(|l| l.0.iter().map(|(_, t)| t.as_str()).collect::<String>()).collect()
}

/// A line whose key is in the table shows the table's text.
#[test]
fn a_table_entry_resolves() {
    let strings = Strings::parse("Message1_Kill_4 = \"Gotcha, friend.\"\n");
    assert_eq!(said(strings, &["$Message1_Kill_4"]), ["AI_Alpha_3 : Gotcha, friend."]);
}

/// A key the table lacks falls back to the same line of another set, and
/// with none to our own text: readable, never `$...`, for every event.
#[test]
fn a_missing_entry_falls_back_to_readable_text() {
    let other_set = Strings::parse("Message0_Death_9 = \"Damn.\"\n");
    assert_eq!(said(other_set, &["$Message2_Death_9"]), ["AI_Alpha_3 : Damn."]);
    let keys: Vec<String> = ["Kill", "Death", "Respawn", "Battle", "Search", "Grenade", "Mine", "Conceal", "Escape"]
        .iter()
        .flat_map(|e| (0..3).flat_map(move |s| (0..10).map(move |n| format!("$Message{s}_{e}_{n}"))))
        .collect();
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    let lines = said(Strings::default(), &refs);
    assert_eq!(lines.len(), keys.len());
    for line in lines {
        let text = line.strip_prefix("AI_Alpha_3 : ").expect("the bot's name leads");
        assert!(!text.is_empty() && !text.contains('$'), "{line}");
    }
}

/// The item file's `WeaponItem` records name weapons by record index
/// through the string table.
#[test]
fn the_item_file_names_weapons() {
    let weapons = Document::parse(b"[Weapon0]\r\nName=\"M9\"\r\n[Weapon1]\r\nName=\"Beretta_M92FS\"\r\n[Weapon2]\r\nName=\"M60\"\r\n").unwrap();
    let items = Document::parse(
        b"[WeaponItem0]\r\nItemID=1\r\nItemName=\"$Weapon_name_1\"\r\n[WeaponItem1]\r\nItemID=2\r\nItemName=\"$Weapon_name_2\"\r\n[WeaponItem2]\r\nItemID=7\r\nItemName=\"$Weapon_name_7\"\r\n",
    )
    .unwrap();
    // Weapon 2 has no string; weapon 7 no record.
    let strings = Strings::parse("Weapon_name_1 = \"M92FS\"\n");
    assert_eq!(
        install::weapon_names_from(&weapons, &items, &strings),
        [("combat_arms:beretta_m92fs".to_string(), "M92FS".to_string())]
    );
}

/// Without a name from the install a gun still reads as words.
#[test]
fn an_unnamed_weapon_reads_as_words() {
    assert_eq!(hud::plain_name("combat_arms:beretta_m92fs"), "BERETTA M92FS");
    assert_eq!(hud::plain_name("combat_arms:m11-tactical_knife"), "M11 TACTICAL KNIFE");
    assert!(chat::built_in_line("$Message2_Death_9").is_some());
}
