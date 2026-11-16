//! The sandbox's screens (`client::sandbox`): the Change gun view's
//! filters (games, a game's facets, the global tags, the search, files),
//! the pins and the bar's fade, Tab's reveal, the download states through
//! the fake fetcher; then the staged screens on the headless app: the
//! grid's keys (tiles, a header's fold), Ctrl+1–6 pinning, typing to
//! find, a double press switching, the HUD's pinned bar fading.

use bevy::{
    input::{ButtonState, keyboard::{Key, KeyboardInput, NativeKey}},
    prelude::*,
};
use chimera_client::client::sandbox::{
    self, SandboxCatalogue, SandboxScreen, SandboxUi, SandboxUiPlugin,
    change_gun::{Grid, LIST},
    hud,
    link::{KillLog, SandboxOptions},
    model::{Files, GunView, PIN_IDLE, PinBar, Pins, TagGroup},
    units::{FakeFetcher, UnitFetcher, UnitProgress, UnitState},
};
use chimera_sim::SimBasePlugins;
use chimera_ui::{Theme, UiClock, UiParityId, VCursor, VirtualLists};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

const SIGNAL: &str = "name = \"test\"\n[flags.hud]\nsignal = true\n";

fn guns() -> Vec<sandbox::model::SandboxGun> {
    let sim = Sim::new(SimBasePlugins, GreyboxMapPlugin);
    sandbox::staged_catalogue(sim.app.world())
}

fn ids(v: &GunView, g: &[sandbox::model::SandboxGun]) -> Vec<String> {
    v.shown(g, &|_| Files::Here).into_iter().flat_map(|(_, i)| i).map(|i| g[i].id.clone()).collect()
}

#[test]
fn games_facets_tags_and_search_narrow_the_grid() {
    let g = guns();
    let mut v = GunView::default();
    assert_eq!(ids(&v, &g).len(), g.len(), "nothing ticked: every gun");
    v.tick_game(Some("tf2"));
    assert!(ids(&v, &g).iter().all(|id| id.starts_with("tf2:")));
    v.tick_facet("tf2", "Soldier");
    let soldier = ids(&v, &g);
    assert_eq!(soldier, vec!["tf2:tf_weapon_shotgun_primary", "tf2:tf_weapon_rocketlauncher"], "the classes that carry it");
    v.tick_facet("tf2", "Soldier");
    v.tick_game(None);
    // Tags AND inside; the files are a tag group too.
    v.tick_tag((TagGroup::Type, "launcher"));
    let launchers = ids(&v, &g);
    assert!(launchers.len() >= 4 && launchers.iter().all(|id| g.iter().find(|x| x.id == *id).unwrap().tags.contains(&(TagGroup::Type, "launcher"))));
    v.tick_tag((TagGroup::Files, "here"));
    let here: Vec<String> = v
        .shown(&g, &|x| if x.unit.is_some() { Files::NotHere } else { Files::Here })
        .into_iter()
        .flat_map(|(_, i)| i)
        .map(|i| g[i].id.clone())
        .collect();
    assert_eq!(here, vec!["combat_arms:m32"], "the one launcher here");
    let mut v = GunView { search: "rocket tf2".into(), ..default() };
    assert_eq!(ids(&v, &g), vec!["tf2:tf_weapon_rocketlauncher"], "every word in the name, game or tags");
    v.search = "halo sniper".into();
    assert_eq!(ids(&v, &g), vec!["halo_ce:sniper_rifle"]);
}

#[test]
fn sections_follow_the_class_order() {
    let g = guns();
    let names: Vec<&str> = GunView::default().shown(&g, &|_| Files::Here).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, sandbox::model::SECTIONS.to_vec());
}

#[test]
fn a_pin_moves_and_the_bar_fades() {
    let mut p = Pins::default();
    p.pin(0, "a");
    p.pin(2, "b");
    p.pin(5, "a");
    assert_eq!(p.0[0], None, "pinned again: it moves");
    assert_eq!(p.slot_of("a"), Some(5));
    p.unpin(2);
    assert_eq!(p.slot_of("b"), None);
    let mut bar = PinBar::default();
    assert_eq!(bar.alpha(0.0, false, 0.5), 0.0, "hidden until woken");
    bar.wake(10.0);
    assert_eq!(bar.alpha(10.0 + PIN_IDLE - 0.1, false, 0.5), 1.0);
    let mid = bar.alpha(10.0 + PIN_IDLE + 0.25, false, 0.5);
    assert!(mid > 0.4 && mid < 0.6, "half faded: {mid}");
    assert_eq!(bar.alpha(10.0 + PIN_IDLE + 1.0, false, 0.5), 0.0);
    assert_eq!(bar.alpha(10.0 + PIN_IDLE + 1.0, true, 0.5), 1.0, "Tab or a drag holds it");
}

#[test]
fn tab_reveals_the_kills_and_take_no_damage() {
    let g = guns();
    let cat = SandboxCatalogue::new(g);
    let mut ui = SandboxUi { on: true, current: Some("cs_source:weapon_ak47".into()), ..default() };
    let mut kills = KillLog::default();
    kills.push(sandbox::link::KillCard {
        gun: "cs_source:weapon_ak47".into(),
        gun_name: "AK-47".into(),
        gun_game_short: "CS:S".into(),
        victim: "Spartan".into(),
        protection: "Shield and health".into(),
        distance_m: 10.0,
        motion: Default::default(),
        shots: 5,
        hits: vec![],
        first_hit_to_kill: 0.3,
        at: 1.0,
    });
    let opts = SandboxOptions::default();
    let rest = hud::shown(&ui, &cat, &UnitProgress::default(), &kills, &opts, 13.0);
    assert_eq!(rest.tags, vec!["Endless ammo"], "take no damage only with Tab");
    assert!(rest.kills.is_empty());
    assert_eq!(rest.gun.as_ref().map(|g| g.name.as_str()), Some("AK-47"));
    ui.tab = true;
    let tab = hud::shown(&ui, &cat, &UnitProgress::default(), &kills, &opts, 13.0);
    assert_eq!(tab.tags, vec!["Endless ammo", "Take no damage"]);
    assert_eq!(tab.kills.len(), 1);
    assert_eq!(tab.kill_age, Some(12));
    ui.screen = SandboxScreen::ChangeGun;
    assert!(!hud::shown(&ui, &cat, &UnitProgress::default(), &kills, &opts, 13.0).on, "Change gun hides the HUD");
}

#[test]
fn a_download_goes_queued_downloading_here_and_a_failure_retries() {
    let mut f = FakeFetcher::new(&[("tf2", "a", 3_000_000), ("tf2", "b", 1_000_000)], 1_000_000.0);
    f.fail.push(("tf2".into(), "b".into()));
    let mut p = UnitProgress::default();
    f.poll(0.0, &mut p);
    assert_eq!(p.state("tf2", "a"), UnitState::NotHere { bytes: 3_000_000 });
    assert_eq!(p.state("cs_source", "x"), UnitState::Here, "a unit not listed is here");
    f.request_unit("tf2", "a");
    f.request_unit("tf2", "b");
    f.poll(0.0, &mut p);
    assert_eq!(p.state("tf2", "b"), UnitState::Queued { bytes: 1_000_000 }, "one at a time");
    f.poll(1.0, &mut p);
    assert_eq!(p.state("tf2", "a"), UnitState::Downloading { done: 1_000_000, total: 3_000_000 });
    assert!((p.state("tf2", "a").share() - 1.0 / 3.0).abs() < 1e-4);
    f.poll(3.0, &mut p);
    assert_eq!(p.state("tf2", "a"), UnitState::Here);
    f.poll(4.0, &mut p);
    assert!(matches!(p.state("tf2", "b"), UnitState::Failed { .. }), "{:?}", p.state("tf2", "b"));
    f.retry("tf2", "b");
    f.poll(4.0, &mut p);
    assert_eq!(p.state("tf2", "b"), UnitState::Downloading { done: 0, total: 4_000_000 }, "asked again, at the queue's head");
    f.poll(9.0, &mut p);
    assert_eq!(p.state("tf2", "b"), UnitState::Here, "a retry doesn't fail again");
}

// ---------------------------------------------------------------------
// The staged screens.

fn sim(what: &str) -> Sim {
    let mut sim = Sim::new(SimBasePlugins, (GreyboxMapPlugin, SandboxUiPlugin));
    sim.app.insert_resource(Theme::from_toml(SIGNAL).expect("theme"));
    sim.app.insert_resource(UiClock::manual());
    sandbox::stage(sim.app.world_mut(), &[what.to_string()]).expect("staged");
    frame(&mut sim);
    frame(&mut sim);
    sim
}

fn frame(sim: &mut Sim) {
    sim.app.world_mut().resource_mut::<UiClock>().step(1.0 / 60.0);
    sim.app.update();
    let w = sim.app.world_mut();
    let mut k = w.resource_mut::<ButtonInput<KeyCode>>();
    k.clear();
}

fn press(sim: &mut Sim, keys: &[KeyCode]) {
    let mut k = sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    for key in keys {
        k.press(*key);
    }
    frame(sim);
    let mut k = sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    for key in keys {
        k.release(*key);
    }
}

fn type_text(sim: &mut Sim, text: &str) {
    let window = Entity::PLACEHOLDER;
    for ch in text.chars() {
        sim.app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyA,
            logical_key: Key::Character(ch.to_string().into()),
            state: ButtonState::Pressed,
            text: Some(ch.to_string().into()),
            repeat: false,
            window,
        });
    }
    let _ = NativeKey::Unidentified;
    frame(sim);
}

fn ui(sim: &Sim) -> SandboxUi {
    sim.app.world().resource::<SandboxUi>().clone()
}

fn cursor(sim: &Sim) -> Option<VCursor> {
    sim.app.world().resource::<VirtualLists>().get(LIST).and_then(|s| s.cursor)
}

#[test]
fn the_grid_opens_on_the_gun_in_hand_and_the_arrows_move_the_selection() {
    let mut sim = sim("gun");
    assert_eq!(ui(&sim).view.selected.as_deref(), Some("cs_source:weapon_ak47"));
    let grid = sim.app.world().resource::<Grid>().clone();
    let cat = sim.app.world().resource::<SandboxCatalogue>().clone();
    let at = |c: Option<VCursor>| c.and_then(|c| grid.gun_at(c)).map(|i| cat.guns[i].id.clone());
    assert_eq!(at(cursor(&sim)).as_deref(), Some("cs_source:weapon_ak47"), "the cursor on it");
    press(&mut sim, &[KeyCode::ArrowRight]);
    assert_eq!(ui(&sim).view.selected.as_deref(), Some("cs_source:weapon_m4a1"));
    // Ctrl+3 pins it.
    press(&mut sim, &[KeyCode::ControlLeft, KeyCode::Digit3]);
    assert_eq!(ui(&sim).pins.slot_of("cs_source:weapon_m4a1"), Some(2));
    // Up twice from the section's first row: its header.
    press(&mut sim, &[KeyCode::ArrowUp]);
    assert!(matches!(cursor(&sim), Some(VCursor::Header(_))), "{:?}", cursor(&sim));
    let Some(VCursor::Header(s)) = cursor(&sim) else { unreachable!() };
    press(&mut sim, &[KeyCode::ArrowLeft]);
    assert!(sim.app.world().resource::<VirtualLists>().get(LIST).unwrap().is_folded(s), "← on a header folds");
    press(&mut sim, &[KeyCode::Enter]);
    assert!(!sim.app.world().resource::<VirtualLists>().get(LIST).unwrap().is_folded(s), "Enter flips it back");
}

#[test]
fn typing_finds_and_enter_switches() {
    let mut sim = sim("gun");
    type_text(&mut sim, "awp");
    assert_eq!(ui(&sim).view.search, "awp");
    frame(&mut sim);
    let grid = sim.app.world().resource::<Grid>().clone();
    assert_eq!(grid.0.iter().map(|s| s.1.len()).sum::<usize>(), 1);
    // The selection follows the search when the cursor's gun went.
    let cat = sim.app.world().resource::<SandboxCatalogue>().clone();
    sim.app.world_mut().resource_mut::<SandboxUi>().view.selected = Some("cs_source:weapon_awp".into());
    frame(&mut sim);
    let _ = cat;
    press(&mut sim, &[KeyCode::Enter]);
    frame(&mut sim);
    let u = ui(&sim);
    assert_eq!(u.screen, SandboxScreen::None, "switching closes the view");
    assert_eq!(u.current.as_deref(), Some("cs_source:weapon_awp"), "a staged switch takes it in hand");
}

#[test]
fn a_gun_not_here_waits_as_next_until_it_lands() {
    let mut sim = sim("rest");
    sim.app.world_mut().write_message(sandbox::SandboxAction::Switch("tf2:tf_weapon_rocketlauncher".into()));
    frame(&mut sim);
    let u = ui(&sim);
    assert_eq!(u.pending.as_deref(), Some("tf2:tf_weapon_rocketlauncher"), "picked, downloading");
    assert_eq!(u.current.as_deref(), Some("cs_source:weapon_ak47"), "still the old gun in hand");
    let st = sim.app.world().resource::<UnitProgress>().state("tf2", "tf2:tf_weapon_rocketlauncher");
    assert!(matches!(st, UnitState::Queued { .. } | UnitState::Downloading { .. }), "{st:?}");
}

#[test]
fn esc_closes_change_gun_and_the_escape_menu_is_told() {
    let mut sim = sim("gun");
    press(&mut sim, &[KeyCode::Escape]);
    frame(&mut sim);
    let u = ui(&sim);
    assert_eq!(u.screen, SandboxScreen::None);
}

#[test]
fn the_hud_shows_the_pinned_bar_then_lets_it_go() {
    let mut sim = sim("pins");
    let has_bar = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        w.query::<&UiParityId>().iter(w).any(|p| p.0 == "sb.pins")
    };
    assert!(has_bar(&mut sim), "a number key showed it");
    for _ in 0..((PIN_IDLE + 1.0) * 60.0) as usize {
        frame(&mut sim);
    }
    assert!(!has_bar(&mut sim), "gone after {PIN_IDLE} s and the fade");
    // A number key brings it back and switches.
    press(&mut sim, &[KeyCode::Digit2]);
    frame(&mut sim);
    assert!(has_bar(&mut sim));
    assert_eq!(ui(&sim).current.as_deref(), Some("combat_arms:m4a1"));
}

#[test]
fn the_grid_draws_its_tiles() {
    let mut sim = sim("gun");
    frame(&mut sim);
    let w = sim.app.world_mut();
    let tiles = w.query::<&UiParityId>().iter(w).filter(|p| p.0.starts_with("gun.tile.")).count();
    let slots: Vec<(Option<usize>, Display, PositionType)> = w
        .query::<(&chimera_ui::VSlot, &Node)>()
        .iter(w)
        .map(|(s, n)| (s.line, n.display, n.position_type))
        .collect();
    assert!(slots.iter().all(|s| s.2 == PositionType::Absolute), "slots stay absolute: {slots:?}");
    assert!(tiles >= 10, "the rows in view have tiles: {tiles}; slots {slots:?}");
}

#[test]
fn every_letter_goes_to_the_search_even_b_i_and_r() {
    let mut sim = sim("gun");
    type_text(&mut sim, "bir");
    let u = ui(&sim);
    assert_eq!(u.screen, SandboxScreen::ChangeGun, "B doesn't close the view while typing");
    assert_eq!(u.view.search, "bir");
    assert!(!u.view.detail, "I typed, not the model view");
    // Ctrl+I is the model view.
    press(&mut sim, &[KeyCode::ControlLeft, KeyCode::KeyI]);
    assert!(ui(&sim).view.detail);
}

#[test]
fn backslash_folds_the_class_and_opens_it_back_on_the_same_tile() {
    let mut sim = sim("gun");
    press(&mut sim, &[KeyCode::ArrowRight]);
    let at = cursor(&sim);
    let Some(VCursor::Item(sec, _)) = at else { panic!("on a tile: {at:?}") };
    press(&mut sim, &[KeyCode::Backslash]);
    assert_eq!(cursor(&sim), Some(VCursor::Header(sec)), "parked on its header");
    assert!(sim.app.world().resource::<VirtualLists>().get(LIST).unwrap().is_folded(sec));
    // Typed letters never take it: a search runs, and \\ still folds.
    press(&mut sim, &[KeyCode::Enter]);
    assert_eq!(cursor(&sim), at, "Enter on the header opens it and goes back to the tile");
    press(&mut sim, &[KeyCode::IntlBackslash]);
    assert_eq!(cursor(&sim), Some(VCursor::Header(sec)), "the ISO key folds too");
    press(&mut sim, &[KeyCode::Backslash]);
    assert_eq!(cursor(&sim), at, "\\ opens it and goes back to the tile");
    assert_eq!(ui(&sim).screen, SandboxScreen::ChangeGun, "no switch on the way");
}

#[test]
fn enter_switches_on_a_tile_and_folds_on_a_header() {
    let mut sim = sim("gun");
    press(&mut sim, &[KeyCode::ArrowUp]);
    let Some(VCursor::Header(sec)) = cursor(&sim) else { panic!("on the header: {:?}", cursor(&sim)) };
    press(&mut sim, &[KeyCode::Enter]);
    assert!(sim.app.world().resource::<VirtualLists>().get(LIST).unwrap().is_folded(sec), "Enter folds a header");
    assert_eq!(ui(&sim).screen, SandboxScreen::ChangeGun, "and switches nothing");
    press(&mut sim, &[KeyCode::Enter]);
    assert!(!sim.app.world().resource::<VirtualLists>().get(LIST).unwrap().is_folded(sec), "and opens it again");
    press(&mut sim, &[KeyCode::ArrowDown]);
    let gun = ui(&sim).view.selected.clone();
    press(&mut sim, &[KeyCode::Enter]);
    frame(&mut sim);
    let u = ui(&sim);
    assert_eq!(u.screen, SandboxScreen::None, "Enter on a tile switches");
    assert_eq!(u.current, gun);
}

#[test]
fn right_wraps_rows_and_classes_and_stops_on_a_folded_header() {
    let mut sim = sim("gun");
    let grid = sim.app.world().resource::<Grid>().clone();
    // The AK-47 is a rifle; walk right past the class's last tile.
    let Some(VCursor::Item(sec, _)) = cursor(&sim) else { panic!() };
    let n = grid.0[sec].1.len();
    sim.app.world_mut().resource_mut::<VirtualLists>().state(LIST).select(VCursor::Item(sec, n - 1));
    press(&mut sim, &[KeyCode::ArrowRight]);
    assert_eq!(cursor(&sim), Some(VCursor::Item(sec + 1, 0)), "into the next class's first tile");
    press(&mut sim, &[KeyCode::ArrowLeft]);
    assert_eq!(cursor(&sim), Some(VCursor::Item(sec, n - 1)), "and back");
    sim.app.world_mut().resource_mut::<VirtualLists>().state(LIST).fold(sec + 1, true);
    press(&mut sim, &[KeyCode::ArrowRight]);
    assert_eq!(cursor(&sim), Some(VCursor::Header(sec + 1)), "a folded class's header in its place");
}

#[test]
fn the_drop_hint_names_the_gun_the_slot_and_what_it_replaces() {
    use chimera_client::client::sandbox::model::drop_hint;
    assert_eq!(drop_hint("Rocket Launcher", 5, None), "Release: Rocket Launcher to 6");
    assert_eq!(drop_hint("Rocket Launcher", 1, Some("M4A1")), "Release: Rocket Launcher to 2, replacing M4A1");
    assert_eq!(drop_hint("M4A1", 1, Some("M4A1")), "Release: M4A1 to 2", "onto itself: nothing replaced");
    // The staged drag over slot 2 shows it beside the bar, the ghost in
    // the slot and the old name struck through.
    let mut sim = sim("drag");
    frame(&mut sim);
    let w = sim.app.world_mut();
    let ids: Vec<String> = w.query::<&UiParityId>().iter(w).map(|p| p.0.clone()).collect();
    assert!(ids.iter().any(|i| i == "gun.drop.hint"), "the hint");
    assert!(ids.iter().any(|i| i == "sb.pin.drop"), "the ghost's frame");
    assert!(ids.iter().any(|i| i == "sb.pin.was"), "the old gun struck through");
    let hint = w
        .query::<(&UiParityId, &Text)>()
        .iter(w)
        .find(|(p, _)| p.0 == "gun.drop.hint")
        .map(|(_, t)| t.0.clone())
        .unwrap_or_default();
    assert_eq!(hint.to_lowercase(), "release: rocket launcher to 2, replacing m4a1");
}
