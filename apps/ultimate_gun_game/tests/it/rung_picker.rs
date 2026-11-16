//! The sandbox's rung picker (`party_kit::rung_picker`): the dialog lists
//! every rung of the sandbox's ladder (every game's guns) by game and
//! class, Halo's nine among them; a row puts the player on its rung as
//! `rung` does and closes it; Esc changes nothing; the filter narrows
//! across games and Enter picks the first match; the Escape menu's Change
//! gun opens it; outside the sandbox it never opens (`ui_scene
//! rung_picker` stages it, a pick then changing nothing). On the party's
//! simulation, headless (the dialog's drawing is the screenshots').

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use chimera_core::{console::Console, core::LocalPlayer};
use chimera_sim::{
    minigame::{self, ladder::{Ladder, Rung}},
    testkit,
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{game::movement, harness::CaSim};
use party_kit::{
    PartyPartsPlugin,
    esc_menu::{self, Action, EscMenu, EscMenuPlugin},
    rung_picker::{PickRow, RungPicker, RungPickerAction as A, RungPickerPlugin},
};
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin, sandbox::SANDBOX};

const SNIPER: &str = "halo_ce:sniper_rifle";
const FRAG: &str = "halo_ce:frag_grenade";

fn sim() -> Sim {
    Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin, UltimateGunGamePlugin::default(), EscMenuPlugin, RungPickerPlugin))
}

/// The sandbox playing with the local player.
fn play() -> (Sim, Entity) {
    let mut sim = sim();
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(1.5);
    (sim, me)
}

fn submit(sim: &mut Sim, line: &str) {
    sim.app.world_mut().resource_mut::<Console>().submit(line.to_string());
    sim.ticks(3);
}

fn picker(sim: &Sim) -> RungPicker {
    sim.app.world().resource::<RungPicker>().clone()
}

fn tap(sim: &mut Sim, key: KeyCode) {
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
    sim.app.update();
    let mut keys = sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
    sim.app.update();
}

fn type_text(sim: &mut Sim, text: &str) {
    for ch in text.chars() {
        let logical_key = if ch == ' ' { Key::Space } else { Key::Character(ch.to_string().into()) };
        sim.app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyA,
            logical_key,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        sim.app.update();
    }
    sim.app.update();
}

fn act(sim: &mut Sim, a: A) {
    sim.app.world_mut().write_message(a);
    sim.ticks(3);
}

fn rung(sim: &Sim, e: Entity) -> usize {
    sim.app.world().get::<Rung>(e).map_or(usize::MAX, |r| r.0)
}

fn index_of(sim: &Sim, id: &str) -> usize {
    sim.app.world().resource::<Ladder>().rungs.iter().position(|r| *r == id).unwrap()
}

#[test]
fn the_picker_lists_every_rung_by_game_and_class_with_halos_nine() {
    let (mut sim, me) = play();
    submit(&mut sim, "rung_picker");
    let p = picker(&sim);
    assert!(p.open, "opens in the sandbox");
    let ladder = sim.app.world().resource::<Ladder>().clone();
    // Only what a drawn ladder may deal (`auto::drawable`), one row a gun.
    let mut caps = chimera_sim::minigame::auto::report(sim.app.world());
    chimera_sim::minigame::auto::drawable(sim.app.world(), &mut caps);
    let dealt: std::collections::BTreeSet<&str> =
        caps.iter().flat_map(|c| c.guns.iter().map(|g| g.id).chain(c.grenades.iter().copied())).collect();
    let mut listed: Vec<usize> = p.games.iter().flat_map(|g| g.classes.iter().flat_map(|k| k.rungs.iter().map(|r| r.index))).collect();
    let n = listed.len();
    listed.sort();
    listed.dedup();
    assert_eq!(listed.len(), n, "a rung once");
    assert!(listed.iter().all(|i| *i < ladder.rungs.len() - 1), "never the sentinel");
    for g in &p.games {
        assert!(g.count() < 100, "{}: {} rows, tens at most", g.id, g.count());
        eprintln!("{}: {} guns", g.id, g.count());
        for k in &g.classes {
            let mut names: Vec<String> = k.rungs.iter().map(|r| r.name.to_lowercase()).collect();
            names.sort();
            let all = names.len();
            names.dedup();
            assert_eq!(names.len(), all, "{} {}: no name twice", g.id, k.name);
            for r in &k.rungs {
                assert_eq!(r.id, ladder.rungs[r.index], "its gun");
                assert!(r.id.starts_with(&format!("{}:", g.id)), "{} under {}", r.id, g.id);
                assert!(!r.name.is_empty());
                assert!(dealt.contains(r.id.as_str()) || r.tag.is_some(), "{} is drawable or a special rung", r.id);
            }
        }
    }
    assert!(p.counts().0 < ladder.rungs.len() - 1 || ladder.rungs.len() < 100, "the install's variants collapsed");
    assert!(p.games.len() >= 3, "every game: {:?}", p.games.iter().map(|g| &g.id).collect::<Vec<_>>());
    let halo = p.games.iter().find(|g| g.id == "halo_ce").expect("Halo's guns");
    assert_eq!(halo.count(), 9, "Halo's eight guns and the frag grenade");
    let class_of = |id: &str| halo.classes.iter().find(|k| k.rungs.iter().any(|r| r.id == id)).map(|k| k.name.as_str());
    assert_eq!(class_of(SNIPER), Some("Snipers"));
    assert_eq!(class_of(FRAG), Some("Grenades"));
    let frag = halo.classes.iter().flat_map(|k| &k.rungs).find(|r| r.id == FRAG).unwrap();
    assert_eq!(frag.tag.as_deref(), Some("Grenade"), "the grenade rung's tag");
    // Classes in the bands' order: pistols before snipers before grenades.
    let names: Vec<&str> = halo.classes.iter().map(|k| k.name.as_str()).collect();
    let at = |n: &str| names.iter().position(|k| *k == n).unwrap();
    assert!(at("Pistols") < at("Snipers") && at("Snipers") < at("Grenades"), "{names:?}");
    // Opened on the current rung, its game open, the others folded.
    let now = rung(&sim, me);
    assert_eq!(p.current, Some(now));
    let rows = p.rows();
    assert_eq!(p.rung(rows[p.cursor]).map(|r| r.index), Some(now), "the cursor on the current rung");
    assert_eq!(rows.iter().filter(|r| matches!(r, PickRow::Game(_))).count(), p.games.len(), "every game's header");
    // Combat Arms' turret, when in the ladder, is tagged.
    if let Some(t) = p.games.iter().flat_map(|g| g.classes.iter().flat_map(|k| &k.rungs)).find(|r| r.id == "combat_arms:auto-turret") {
        assert_eq!(t.tag.as_deref(), Some("Turret"));
    }
}

#[test]
fn a_row_puts_the_player_on_its_rung_and_closes() {
    let (mut sim, me) = play();
    submit(&mut sim, "rung_picker");
    // Open Halo (its chevron), then click the sniper rifle's row.
    let rows = picker(&sim).rows();
    let p = picker(&sim);
    let halo = p.games.iter().position(|g| g.id == "halo_ce").unwrap();
    let header = rows.iter().position(|r| *r == PickRow::Game(halo)).unwrap();
    if p.is_folded(PickRow::Game(halo)) {
        act(&mut sim, A::Fold(header));
    }
    let p = picker(&sim);
    assert!(!p.is_folded(PickRow::Game(halo)), "the chevron opened it");
    let row = p.rows().iter().position(|r| p.rung(*r).is_some_and(|r| r.id == SNIPER)).expect("the sniper's row");
    act(&mut sim, A::Row(row));
    sim.seconds(1.0);
    assert!(!picker(&sim).open, "picking closes it");
    assert_eq!(rung(&sim, me), index_of(&sim, SNIPER));
    assert_eq!(testkit::active_id(sim.app.world(), me), Some(SNIPER), "its gun in hand");
    // Held there, as `rung` holds it.
    sim.seconds(1.0);
    assert_eq!(rung(&sim, me), index_of(&sim, SNIPER));
}

#[test]
fn esc_closes_it_and_changes_nothing() {
    let (mut sim, me) = play();
    let before = rung(&sim, me);
    submit(&mut sim, "rung_picker");
    tap(&mut sim, KeyCode::ArrowDown);
    tap(&mut sim, KeyCode::ArrowDown);
    assert!(sim.app.world().resource::<chimera_client::client::front_end::AppMenu>().open, "the game gets no input while open");
    tap(&mut sim, KeyCode::Escape);
    sim.seconds(0.5);
    assert!(!picker(&sim).open, "Esc closed it");
    assert!(!sim.app.world().resource::<EscMenu>().open, "the Escape menu left Esc to it");
    assert_eq!(rung(&sim, me), before, "nothing changed");
}

#[test]
fn the_filter_narrows_across_games_and_enter_picks_the_first_match() {
    let (mut sim, me) = play();
    submit(&mut sim, "rung_picker");
    sim.ticks(2);
    type_text(&mut sim, "pistol");
    let p = picker(&sim);
    assert_eq!(p.filter, "pistol");
    let games: std::collections::BTreeSet<&str> =
        p.rows().iter().filter_map(|r| p.rung(*r)).map(|r| r.id.split(':').next().unwrap()).collect::<Vec<_>>().into_iter().collect();
    assert!(games.len() >= 2, "matches in several games, their headers open: {games:?}");
    assert!(p.rows().iter().filter_map(|r| p.rung(*r)).all(|r| !r.id.contains("sniper")), "only matches");
    // Narrower: one gun; Enter picks it.
    for _ in 0.."pistol".len() {
        sim.app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Backspace,
            logical_key: Key::Backspace,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        sim.app.update();
    }
    type_text(&mut sim, "halo sniper");
    let p = picker(&sim);
    assert_eq!(p.filter, "halo sniper");
    assert_eq!(p.counts().1, 1, "one match: {:?}", p.rows());
    assert_eq!(p.rung(p.rows()[p.cursor]).map(|r| r.id.as_str()), Some(SNIPER), "the cursor on the first match");
    tap(&mut sim, KeyCode::Enter);
    sim.seconds(1.0);
    assert!(!picker(&sim).open);
    assert_eq!(rung(&sim, me), index_of(&sim, SNIPER));
}

/// Enter is chat's while chat has the keys: the picker picks nothing.
#[test]
fn enter_while_chat_types_picks_nothing() {
    use chimera_client::client::chat::ChatInput;
    let (mut sim, me) = play();
    let before = rung(&sim, me);
    submit(&mut sim, "rung_picker");
    tap(&mut sim, KeyCode::ArrowDown);
    sim.app.world_mut().insert_resource(ChatInput { open: Some(false), ..default() });
    tap(&mut sim, KeyCode::Enter);
    sim.seconds(0.5);
    assert!(picker(&sim).open, "still open: the Enter was chat's");
    assert_eq!(rung(&sim, me), before);
    sim.app.world_mut().insert_resource(ChatInput::default());
    tap(&mut sim, KeyCode::Escape);
    assert!(!picker(&sim).open, "Esc is Back once chat is shut");
}

#[test]
fn the_escape_menus_change_gun_opens_it_in_the_sandbox_only() {
    let (mut sim, _) = play();
    let entries = esc_menu::shown_entries_in(true, true, false, true);
    assert_eq!(entries.get(1).and_then(|e| e.action), Some(Action::ChangeGun), "after Resume in the sandbox");
    assert!(!esc_menu::shown_entries_in(true, true, false, false).iter().any(|e| e.action == Some(Action::ChangeGun)));
    sim.app.world_mut().resource_mut::<EscMenu>().set_open(true);
    sim.app.world_mut().write_message(Action::ChangeGun);
    sim.ticks(3);
    assert!(!sim.app.world().resource::<EscMenu>().open, "the menu closed");
    assert!(picker(&sim).open, "the picker opened");
}

#[test]
fn outside_the_sandbox_it_never_opens() {
    let mut sim = sim();
    minigame::start(sim.app.world_mut(), MINIGAME).expect("the gun game starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(1.0);
    let before = sim.app.world().resource::<Console>().output.len();
    submit(&mut sim, "rung_picker");
    assert!(!picker(&sim).open, "not in a normal match");
    let said: Vec<String> = sim.app.world().resource::<Console>().output[before..].iter().map(|l| l.text.clone()).collect();
    assert!(said.iter().any(|l| l.contains("sandbox")), "says why: {said:?}");
    // The Escape menu's entry: off, and its action opens nothing.
    sim.app.world_mut().resource_mut::<EscMenu>().set_open(true);
    sim.app.world_mut().write_message(Action::ChangeGun);
    sim.ticks(3);
    assert!(!picker(&sim).open);
    assert!(party_kit::rung_picker::pick(sim.app.world_mut(), 0).is_err(), "no pick outside the sandbox");
    // `ui_scene rung_picker` stages it (screenshots); a pick there moves no one.
    let rung_before = rung(&sim, me);
    let hook = sim.app.world().resource::<chimera_client::client::ui_scene::UiSceneHooks>().0.iter().find(|(n, _)| *n == "rung_picker").unwrap().1;
    hook(sim.app.world_mut(), &[]).unwrap();
    sim.ticks(1);
    let p = picker(&sim);
    assert!(p.open && p.staged && !p.games.is_empty(), "staged");
    act(&mut sim, A::Pick);
    assert!(!picker(&sim).open);
    sim.seconds(0.5);
    assert_eq!(rung(&sim, me), rung_before, "a staged pick changes nothing");
}
