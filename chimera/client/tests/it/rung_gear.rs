//! A rung's gear on the Signal HUD (`client::rung_gear`; the user's pick
//! of the mockup, option A): the chip's tail shows the local player's
//! rung's passives (icon, name, the key bound now and the verb) under the
//! race chip and folds to icons after 5 s; none without gear, off the
//! Signal look or while the board is open. The Tab board gains a Gear
//! column (each row its rung's items, a dash for none) whose icons carry
//! the kit's item card once the cursor is free. A passive's kill shows
//! the item's icon in the kill feed in the accent text tone.

use bevy::prelude::*;
use chimera_client::client::{
    console::HeldActions,
    kill_feed::{FeedName, KillEntry, KillFeed, KillFeedPlugin, SharedKillFeed},
    minigame_scoreboard::{GunNames, MinigameScoreboardPlugin},
    race_chip::{BoardGear, BoardPick},
    rung_gear::{GearTailRoot, GearTailView, GearTile, GearTileText},
    signal::icons::SignalIcon,
};
use chimera_core::core::{DamageKind, LocalPlayer, Score};
use chimera_sim::{
    SimBasePlugins,
    minigame::{
        hud::MinigameHud,
        ladder::{Ladder, OnRung, Rung, RungExtras},
        passives::{PassiveTable, PassiveTables},
        runtime::InMinigame,
    },
    weapon::data::WeaponDataAppExt,
};
use chimera_ui::{Theme, Tone, UiClock};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

const SIGNAL: &str = "name = \"test\"\n[flags.hud]\nsignal = true\n[icons]\nprefer_game = true\n";

/// The kit game's items: boots (worn, no key) and a pack (carried, its
/// slot's key).
const TABLE: &str = r#"
[[passive]]
item = "kit:boots"
glyph = "boots"
summary = "Stomp by landing on a head"
[[passive]]
item = "kit:pack"
kind = "carried"
glyph = "jetpack"
key = "slot2"
verb = "Launch"
summary = "Switch to it and fire to fly"
"#;

fn gear(ids: &[&str]) -> Option<RungExtras> {
    Some(RungExtras { game: "kit".into(), passives: ids.iter().map(|s| s.to_string()).collect(), ..default() })
}

fn sim(signal: bool) -> Sim {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins((GreyboxMapPlugin, MinigameScoreboardPlugin, KillFeedPlugin));
        app.add_weapon_data(&[
            ("p.toml", "id = \"kit:weapon_pistol\"\nslot = 1\ndraw = 0.5\nclass = \"pistol\"\n"),
            ("r.toml", "id = \"kit:weapon_rifle\"\nslot = 0\ndraw = 0.5\nclass = \"carbine\"\n"),
            ("b.toml", "id = \"kit:boots\"\nslot = 4\ndraw = 0.5\n[hud]\nname = \"Boots of Testing\"\n"),
            ("k.toml", "id = \"kit:pack\"\nslot = 1\ndraw = 0.5\n[hud]\nname = \"Test Pack\"\n"),
        ])
        .expect("the data reads");
    });
    if signal {
        sim.app.insert_resource(Theme::from_toml(SIGNAL).expect("theme"));
    }
    sim.app.insert_resource(UiClock::manual());
    sim.app.init_resource::<HeldActions>();
    sim.app.insert_resource(PassiveTables([("kit".to_string(), PassiveTable::parse(TABLE).unwrap())].into()));
    sim.app.insert_resource(Ladder {
        rungs: vec!["kit:weapon_pistol", "kit:weapon_rifle", "kit:weapon_pistol"],
        extras: vec![None, gear(&["kit:boots", "kit:pack"]), gear(&["kit:boots"])],
        ..default()
    });
    sim.app.world_mut().insert_resource(GunNames::default());
    sim.app.world_mut().insert_resource(MinigameHud { title: "Gun game".into(), ..default() });
    sim
}

fn player(sim: &mut Sim, name: &'static str, rung: usize, local: bool) -> Entity {
    let extras = sim.app.world().resource::<Ladder>().extras(rung).cloned();
    let mut e = sim.app.world_mut().spawn((InMinigame, Name::new(name), Rung(rung), Score { kills: 1, deaths: 1 }, OnRung { index: rung, extras, from: None }));
    if local {
        e.insert(LocalPlayer);
    }
    e.id()
}

fn frame(sim: &mut Sim, dt: f32) {
    sim.app.world_mut().resource_mut::<UiClock>().step(dt);
    sim.app.update();
}

fn shown(sim: &mut Sim) -> bool {
    let w = sim.app.world_mut();
    w.query_filtered::<&Visibility, With<GearTailRoot>>().iter(w).any(|v| *v != Visibility::Hidden)
}

fn count<C: Component>(sim: &mut Sim) -> usize {
    let w = sim.app.world_mut();
    w.query_filtered::<(), With<C>>().iter(w).count()
}

fn texts_under<C: Component>(sim: &mut Sim) -> Vec<String> {
    let w = sim.app.world_mut();
    let mut stack: Vec<Entity> = w.query_filtered::<Entity, With<C>>().iter(w).collect();
    let mut out = Vec::new();
    while let Some(e) = stack.pop() {
        if let Some(t) = w.get::<Text>(e) {
            out.push(t.0.clone());
        }
        if let Some(c) = w.get::<Children>(e) {
            stack.extend(c.iter());
        }
    }
    out
}

fn text_shown(sim: &mut Sim) -> bool {
    let w = sim.app.world_mut();
    w.query_filtered::<&Node, With<GearTileText>>().iter(w).any(|n| n.display != Display::None)
}

#[test]
fn the_tail_shows_the_rungs_gear_with_its_key_and_folds_after_5_s() {
    let mut sim = sim(true);
    player(&mut sim, "Alpha", 1, true);
    player(&mut sim, "Botty", 2, false);
    for _ in 0..3 {
        frame(&mut sim, 1.0 / 60.0);
    }
    assert!(shown(&mut sim), "the tail is up under the chip");
    assert_eq!(count::<GearTile>(&mut sim), 2, "a tile an item");
    let texts = texts_under::<GearTailRoot>(&mut sim);
    for want in ["BOOTS OF TESTING", "TEST PACK", "LAUNCH"] {
        assert!(texts.iter().any(|t| t.contains(want)), "{want} in {texts:?}");
    }
    let view = sim.app.world().resource::<GearTailView>().clone();
    assert_eq!(view.tiles[0].key, None, "a worn item with no key has no cap");
    assert!(view.tiles[1].key.is_some(), "the carried item's key bound now: {:?}", view.tiles[1].key);
    assert!(text_shown(&mut sim), "names shown on arriving");

    // 5 s on: the names, keys and verbs fold away; the icons stay.
    for _ in 0..50 {
        frame(&mut sim, 0.1);
    }
    frame(&mut sim, 0.3);
    frame(&mut sim, 0.0);
    assert!(sim.app.world().resource::<GearTailView>().folded(), "folded after 5 s");
    assert!(!text_shown(&mut sim), "only the icons");
    assert!(shown(&mut sim) && count::<GearTile>(&mut sim) == 2, "the tiles stay");

    // Tab: the board covers it.
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    for _ in 0..3 {
        frame(&mut sim, 0.1);
    }
    assert!(!shown(&mut sim), "none while the board is open");
}

#[test]
fn no_tail_without_gear_or_off_the_signal_look() {
    let mut sim = sim(true);
    player(&mut sim, "Alpha", 0, true);
    for _ in 0..3 {
        frame(&mut sim, 1.0 / 60.0);
    }
    assert!(!shown(&mut sim), "the first rung carries nothing");

    let mut plain = sim_plain();
    player(&mut plain, "Alpha", 1, true);
    for _ in 0..3 {
        frame(&mut plain, 1.0 / 60.0);
    }
    assert!(!shown(&mut plain), "not off the Signal look");
}

fn sim_plain() -> Sim {
    sim(false)
}

#[test]
fn the_board_has_a_gear_column_with_the_items_cards() {
    let mut sim = sim(true);
    player(&mut sim, "Alpha", 1, true);
    player(&mut sim, "Botty", 2, false);
    player(&mut sim, "Cee", 0, false);
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    for _ in 0..4 {
        frame(&mut sim, 0.1);
    }
    let w = sim.app.world_mut();
    let mut ids: Vec<String> = w.query::<&BoardGear>().iter(w).map(|g| g.0.clone()).collect();
    ids.sort();
    assert_eq!(ids, ["kit:boots", "kit:boots", "kit:pack"], "each row its rung's items");
    let board = texts_under::<chimera_client::client::race_chip::BoardContent>(&mut sim);
    assert!(board.iter().any(|t| t == "GEAR"), "the column's title: {board:?}");
    assert!(board.iter().any(|t| t == "\u{2014}"), "a dash for the rung without gear");

    // The cursor free: each icon a kit item with the item's card.
    sim.app.insert_resource(BoardPick(true));
    for _ in 0..3 {
        frame(&mut sim, 0.05);
    }
    let w = sim.app.world_mut();
    let cards: Vec<chimera_ui::shell::ItemCard> = w
        .query_filtered::<&chimera_ui::shell::Tip, With<BoardGear>>()
        .iter(w)
        .filter_map(|t| t.item.as_deref().cloned())
        .collect();
    assert_eq!(cards.len(), 3, "a card an icon");
    let pack = cards.iter().find(|c| c.title == "Test Pack").expect("the pack's card");
    assert_eq!(pack.text.as_deref(), Some("Switch to it and fire to fly"));
    assert_eq!(pack.key.as_ref().map(|k| k.1.as_str()), Some("Launch"));
    assert!(pack.sub.contains("carried") && pack.foot.contains("Rung 02"), "{pack:?}");
    let controls = w.query_filtered::<(), (With<BoardGear>, With<chimera_ui::controls::KitControl>)>().iter(w).count();
    assert_eq!(controls, 3, "the kit's pressable items");
}

#[test]
fn a_passives_kill_shows_the_items_icon_in_the_accent_text_tone() {
    let mut sim = sim(true);
    sim.app.insert_resource(SharedKillFeed);
    player(&mut sim, "Alpha", 1, true);
    frame(&mut sim, 0.0);
    let now = sim.app.world().resource::<Time>().elapsed_secs();
    let side = |name: &str| FeedName { entity: Entity::PLACEHOLDER, name: name.into(), team: None, local: false, bot: false };
    sim.app.world_mut().resource_mut::<KillFeed>().entries.push(KillEntry {
        id: 7,
        killer: Some(side("Larry")),
        victim: side("Rook"),
        weapon: Some("kit:boots"),
        kind: DamageKind::Generic,
        headshot: false,
        suicide: false,
        team_kill: false,
        at: now,
    });
    frame(&mut sim, 0.05);
    frame(&mut sim, 0.05);
    let acc = Theme::from_toml(SIGNAL).unwrap().tone(Tone::AccText);
    let w = sim.app.world_mut();
    let icons: Vec<(String, Color)> = w.query::<&SignalIcon>().iter(w).map(|i| (i.name.clone(), i.color)).collect();
    assert!(icons.iter().any(|(n, c)| n == "boots" && *c == acc), "the boots' glyph in the accent text tone: {icons:?}");
}
