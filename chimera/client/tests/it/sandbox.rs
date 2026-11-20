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

/// The variants of one gun are one family, shown as one tile: by a shared
/// first-person model, by the name's model, by a name that holds another's
/// whole; only in a game whose names split, but for the model file.
#[test]
fn variants_of_one_gun_are_one_tile() {
    use chimera_client::client::sandbox::model::{SandboxGun, Tile, set_families};
    use chimera_core::game_parts::GunClass;
    let gun = |id: &str, name: &str, class: GunClass| SandboxGun {
        id: id.into(),
        game: id.split_once(':').unwrap().0.into(),
        name: name.into(),
        class: Some(class),
        family: id.into(),
        ..default()
    };
    let list = [
        ("ca:m4a1_gold", "M4A1 GOLD", GunClass::Rifle, "pv_m4a1.ltb"),
        ("ca:m4a1", "M4A1", GunClass::Rifle, "pv_m4a1.ltb"),
        ("ca:midnight_m4a1_lv.4", "MIDNIGHT WARDEN M4A1 LV.4", GunClass::Rifle, "pv_midnight.ltb"),
        ("ca:ak-47", "AK 47", GunClass::Rifle, "pv_ak47.ltb"),
        ("ca:fiery_ak-47", "FIERY DRAGON AK 47", GunClass::Rifle, "pv_fiery.ltb"),
        ("ca:g3", "G3", GunClass::Rifle, "pv_g3.ltb"),
        ("ca:g36c", "G36C", GunClass::Rifle, "pv_g36c.ltb"),
        ("ca:m4a1_pistol", "M4A1 SHORTY", GunClass::Pistol, "pv_m4a1_pst.ltb"),
        ("halo:pistol", "Pistol", GunClass::Pistol, "pistol.gbx"),
        ("halo:plasma_pistol", "Plasma Pistol", GunClass::Pistol, "plasma.gbx"),
        ("halo:m4a1", "M4A1", GunClass::Rifle, "pv_m4a1.ltb"),
    ];
    let mut g: Vec<SandboxGun> = list.iter().enumerate().map(|(i, (id, n, c, _))| SandboxGun { rung: i, ..gun(id, n, *c) }).collect();
    let models: Vec<Option<String>> = list.iter().map(|l| Some(l.3.to_string())).collect();
    set_families(&mut g, &models, &chimera_sim::weapon::names::NameWords::shipped(), &|game| game == "ca");
    let family = |id: &str| g.iter().find(|x| x.id == id).unwrap().family.clone();
    assert_eq!(family("ca:m4a1_gold"), "ca:m4a1", "the same model file: the shortest name is the base");
    assert_eq!(family("ca:midnight_m4a1_lv.4"), "ca:m4a1", "the name's model");
    assert_eq!(family("ca:fiery_ak-47"), "ca:ak-47", "a name holding the base's whole");
    assert_eq!(family("ca:g36c"), "ca:g36c", "G36C is no G3");
    assert_eq!(family("ca:m4a1_pistol"), "ca:m4a1_pistol", "another class is another shelf");
    assert_eq!(family("halo:plasma_pistol"), "halo:plasma_pistol", "names don't join in a game that doesn't split");
    assert_eq!(family("halo:m4a1"), "halo:m4a1", "nor across games");
    // One tile a family, where its first gun stands, its face the base.
    let v = GunView::default();
    let tiles = v.tiles(&g, &|_| Files::Here);
    let rifles = &tiles.iter().find(|(n, _)| *n == "Rifles and MGs").unwrap().1;
    assert_eq!(rifles[0], Tile { face: 1, variants: vec![0, 1, 2] });
    assert_eq!(rifles.iter().map(|t| g[t.face].id.as_str()).collect::<Vec<_>>(), ["ca:m4a1", "ca:ak-47", "ca:g3", "ca:g36c", "halo:m4a1"]);
    assert_eq!(v.shown(&g, &|_| Files::Here).iter().map(|s| s.1.len()).sum::<usize>(), 8, "8 tiles for 11 guns");
    // A search that leaves one variant: its tile shows that one.
    let v = GunView { search: "midnight".into(), ..default() };
    assert_eq!(v.tiles(&g, &|_| Files::Here), vec![("Rifles and MGs", vec![Tile { face: 2, variants: vec![2] }])]);
}

/// A tile of several variants says so, the detail pane lists them, a press
/// (or Tab) picks one and Enter switches to it.
#[test]
fn a_tiles_variants_are_listed_and_one_is_picked() {
    use chimera_client::client::sandbox::change_gun::GunAction;
    let mut sim = sim("gun");
    // The staged AK-47 and M4A1 (CS:S) as one gun's variants.
    {
        let mut cat = sim.app.world_mut().resource_mut::<SandboxCatalogue>();
        let before = cat.guns.len();
        for g in cat.guns.iter_mut().filter(|g| g.id == "cs_source:weapon_m4a1") {
            g.family = "cs_source:weapon_ak47".into();
        }
        assert_eq!(cat.guns.len(), before);
    }
    frame(&mut sim);
    frame(&mut sim);
    let grid = sim.app.world().resource::<Grid>().clone();
    let cat = sim.app.world().resource::<SandboxCatalogue>().clone();
    let tiles: usize = grid.0.iter().map(|s| s.1.len()).sum();
    assert_eq!(tiles, cat.guns.len() - 1, "one tile for the two");
    let ak = cat.guns.iter().position(|g| g.id == "cs_source:weapon_ak47").unwrap();
    let m4 = cat.guns.iter().position(|g| g.id == "cs_source:weapon_m4a1").unwrap();
    assert_eq!(grid.variants_of(ak), [ak, m4]);
    assert_eq!(grid.cursor_of(m4), grid.cursor_of(ak), "a variant is on its gun's tile");
    let drawn = |sim: &mut Sim, id: &str| {
        let w = sim.app.world_mut();
        w.query::<&UiParityId>().iter(w).any(|p| p.0 == id)
    };
    assert!(drawn(&mut sim, "gun.tile.variants.cs_source:weapon_ak47"), "the tile says how many");
    assert!(!drawn(&mut sim, "gun.tile.cs_source:weapon_m4a1"), "no tile of its own");
    assert!(drawn(&mut sim, "gun.variants") && drawn(&mut sim, "gun.variant.cs_source:weapon_m4a1"), "the detail pane lists them");
    // A press on a variant picks it: the cursor stays, the pane is its.
    let cursor_before = cursor(&sim);
    sim.app.world_mut().write_message(GunAction::Variant("cs_source:weapon_m4a1".into()));
    frame(&mut sim);
    frame(&mut sim);
    assert_eq!(ui(&sim).view.selected.as_deref(), Some("cs_source:weapon_m4a1"));
    assert_eq!(cursor(&sim), cursor_before);
    // Tab goes on to the next (around to the base), Shift+Tab back.
    press(&mut sim, &[KeyCode::Tab]);
    frame(&mut sim);
    assert_eq!(ui(&sim).view.selected.as_deref(), Some("cs_source:weapon_ak47"));
    press(&mut sim, &[KeyCode::ShiftLeft, KeyCode::Tab]);
    frame(&mut sim);
    assert_eq!(ui(&sim).view.selected.as_deref(), Some("cs_source:weapon_m4a1"));
    // Enter switches to the variant picked.
    press(&mut sim, &[KeyCode::Enter]);
    frame(&mut sim);
    assert_eq!(ui(&sim).current.as_deref(), Some("cs_source:weapon_m4a1"));
}

#[test]
fn sections_follow_the_class_order() {
    let g = guns();
    let names: Vec<&str> = GunView::default().shown(&g, &|_| Files::Here).into_iter().map(|(n, _)| n).collect();
    let mut want = sandbox::model::SECTIONS.to_vec();
    want.retain(|s| *s != "Other");
    assert_eq!(names, want, "melee its own class; nothing staged is other");
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

/// The Change gun grid's picker state (it owns the search field).
fn gp(sim: &Sim) -> chimera_ui::picker::PickerState {
    sim.app.world().resource::<sandbox::change_gun::GunPicker>().0.clone()
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
    type_text(&mut sim, "/awp");
    assert_eq!(gp(&sim).search, "awp");
    frame(&mut sim);
    let grid = sim.app.world().resource::<Grid>().clone();
    assert_eq!(grid.0.iter().map(|s| s.1.len()).sum::<usize>(), 1);
    // The selection follows the search when the cursor's gun went.
    let cat = sim.app.world().resource::<SandboxCatalogue>().clone();
    sim.app.world_mut().resource_mut::<SandboxUi>().view.selected = Some("cs_source:weapon_awp".into());
    frame(&mut sim);
    let _ = cat;
    press(&mut sim, &[KeyCode::Enter]);
    assert!(!gp(&sim).search_focus, "the first Enter lets go of the field");
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
fn letters_type_only_in_the_focused_search_and_b_toggles() {
    let mut sim = sim("gun");
    // Not focused: letters don't type; I is the model view.
    type_text(&mut sim, "ir");
    let u = ui(&sim);
    assert!(gp(&sim).search.is_empty(), "no letters into an unfocused search");
    assert!(u.view.detail, "I is the model view");
    // `/` focuses the field: then every letter types, B and I and R too.
    type_text(&mut sim, "/bir");
    let u = ui(&sim);
    assert!(gp(&sim).search_focus);
    assert_eq!(u.screen, SandboxScreen::ChangeGun, "B doesn't close the view while typing");
    assert_eq!(gp(&sim).search, "bir");
    assert!(u.view.detail, "I typed, the model view unchanged");
    // Esc leaves the field (the text stays), then clears it, then closes.
    press(&mut sim, &[KeyCode::Escape]);
    assert!(!gp(&sim).search_focus && gp(&sim).search == "bir");
    press(&mut sim, &[KeyCode::Escape]);
    assert!(gp(&sim).search.is_empty() && ui(&sim).screen == SandboxScreen::ChangeGun);
    // B closes it; the screen toggles back open with B from the range.
    real_key(&mut sim, KeyCode::KeyB, "b");
    frame(&mut sim);
    assert_eq!(ui(&sim).screen, SandboxScreen::None, "B closes Change gun");
    // The opening press as the platform gives it (its event too): the
    // view doesn't take it as its close (playtest 2026-11-18).
    real_key(&mut sim, KeyCode::KeyB, "b");
    frame(&mut sim);
    frame(&mut sim);
    assert_eq!(ui(&sim).screen, SandboxScreen::ChangeGun, "B opens it and it stays open");
    // A click on the field focuses it too; Enter lets go of it.
    sim.app.world_mut().resource_mut::<sandbox::change_gun::GunPicker>().0.focus_search();
    frame(&mut sim);
    assert!(gp(&sim).search_focus);
    type_text(&mut sim, "awp");
    press(&mut sim, &[KeyCode::Enter]);
    assert!(!gp(&sim).search_focus);
    assert_eq!(gp(&sim).search, "awp");
    assert_eq!(ui(&sim).screen, SandboxScreen::ChangeGun, "Enter in the field switched nothing");
}

/// A key as the platform gives it: its event (the text) and its press
/// (`ButtonInput`) in one frame, then let go.
fn real_key(sim: &mut Sim, code: KeyCode, ch: &str) {
    sim.app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: Key::Character(ch.into()),
        state: ButtonState::Pressed,
        text: Some(ch.into()),
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    press(sim, &[code]);
}

/// One key event with its physical key (`type_text` has none).
fn key_event(sim: &mut Sim, code: KeyCode, key: Key) {
    sim.app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: key,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    frame(sim);
}

/// The Change gun search is the kit's text field (a caret, a selection):
/// `PickerSearch` on a text field, the arrows move its caret and not the
/// grid's cursor, Backspace and Delete edit at the caret, Ctrl+A selects
/// all (typing then replaces), `/` and a click focus it, Enter and Esc
/// let go, and B toggles the menu only while it isn't focused.
#[test]
fn the_search_is_the_standard_text_field_with_a_caret_and_a_selection() {
    use chimera_ui::picker::PickerSearch;
    let mut sim = sim("gun");
    let w = sim.app.world_mut();
    assert_eq!(w.query::<&PickerSearch>().iter(w).count(), 1, "the picker's search field");
    type_text(&mut sim, "/awp");
    assert!(gp(&sim).search_focus);
    let at = cursor(&sim);
    key_event(&mut sim, KeyCode::ArrowLeft, Key::ArrowLeft);
    key_event(&mut sim, KeyCode::ArrowLeft, Key::ArrowLeft);
    assert_eq!(gp(&sim).caret.at, 1, "the caret moved");
    assert_eq!(cursor(&sim), at, "Left doesn't move the grid's cursor in the field");
    type_text(&mut sim, "r");
    assert_eq!(gp(&sim).search, "arwp", "typed at the caret");
    key_event(&mut sim, KeyCode::Delete, Key::Delete);
    assert_eq!(gp(&sim).search, "arp");
    key_event(&mut sim, KeyCode::Home, Key::Home);
    key_event(&mut sim, KeyCode::End, Key::End);
    assert_eq!(gp(&sim).caret.at, 3);
    key_event(&mut sim, KeyCode::Backspace, Key::Backspace);
    assert_eq!(gp(&sim).search, "ar");
    // Ctrl+A selects all; typing replaces it.
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ControlLeft);
    key_event(&mut sim, KeyCode::KeyA, Key::Character("a".into()));
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::ControlLeft);
    let u = ui(&sim);
    assert_eq!((gp(&sim).caret.at, gp(&sim).caret.anchor), (2, 0), "all selected");
    type_text(&mut sim, "m4");
    assert_eq!(gp(&sim).search, "m4");
    // B is a letter while the field has the keys.
    real_key(&mut sim, KeyCode::KeyB, "b");
    assert_eq!(gp(&sim).search, "m4b");
    assert_eq!(ui(&sim).screen, SandboxScreen::ChangeGun);
    // Enter lets go; B then closes; Esc after `/` lets go too.
    press(&mut sim, &[KeyCode::Enter]);
    assert!(!gp(&sim).search_focus);
    type_text(&mut sim, "/");
    assert_eq!(gp(&sim).caret.at, 3, "focused again with the caret at the end");
    press(&mut sim, &[KeyCode::Escape]);
    assert!(!gp(&sim).search_focus && gp(&sim).search == "m4b");
    real_key(&mut sim, KeyCode::KeyB, "b");
    frame(&mut sim);
    assert_eq!(ui(&sim).screen, SandboxScreen::None, "B closes it again once the field let go");
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

#[test]
fn the_tag_popovers_own_search_types_finds_and_enter_ticks() {
    let mut sim = sim("gun");
    sim.app.world_mut().write_message(sandbox::change_gun::GunAction::TagsButton);
    frame(&mut sim);
    assert!(ui(&sim).view.tags_open, "the popover open");
    type_text(&mut sim, "spl");
    let u = ui(&sim);
    assert_eq!(u.view.tag_search, "spl", "typing goes to Find a tag");
    assert!(gp(&sim).search.is_empty(), "not to the grid's search");
    assert_eq!(u.view.tags_found(), vec![(TagGroup::Behaviour, vec!["splash"])], "the tags holding it");
    // A group's label finds its tags too.
    let v = GunView { tag_search: "optics".into(), ..default() };
    assert_eq!(v.tags_found(), vec![(TagGroup::Optics, vec!["iron sights", "scope", "zoom"])]);
    // The field shows what is typed.
    frame(&mut sim);
    let w = sim.app.world_mut();
    assert!(w.query::<&UiParityId>().iter(w).any(|p| p.0 == "gun.popover.find"));
    let shown: Vec<String> = w.query::<&Text>().iter(w).map(|t| t.0.clone()).collect();
    // (The kit's field: one text a character.)
    assert!(["s", "p", "l"].iter().all(|c| shown.iter().any(|t| t == c)), "the typed words in the field");
    // Enter ticks the first one found and clears the field.
    press(&mut sim, &[KeyCode::Enter]);
    let u = ui(&sim);
    assert!(u.view.tags.contains(&(TagGroup::Behaviour, "splash")), "{:?}", u.view.tags);
    assert!(u.view.tag_search.is_empty());
    assert_eq!(u.screen, SandboxScreen::ChangeGun, "Enter switched nothing");
    // Esc clears what is typed, then closes the popover, then the view.
    type_text(&mut sim, "x");
    press(&mut sim, &[KeyCode::Escape]);
    assert!(ui(&sim).view.tag_search.is_empty() && ui(&sim).view.tags_open);
    press(&mut sim, &[KeyCode::Escape]);
    assert!(!ui(&sim).view.tags_open);
    assert_eq!(ui(&sim).screen, SandboxScreen::ChangeGun);
}

#[test]
fn the_bench_holds_list_opens_on_the_bodys_guns_and_holds_one() {
    use chimera_client::client::sandbox::{
        bench::{BenchAction, BenchUi, CharacterCatalogue, holds_of},
        link::BenchView,
    };
    let mut sim = sim("bench");
    let t = sim.app.world().resource::<BenchView>().targets[0].clone();
    assert_eq!(t.spec.character, "halo_ce:spartan");
    let chars = sim.app.world().resource::<CharacterCatalogue>().clone();
    let guns = holds_of(&chars.0, &t.spec.character).to_vec();
    assert!(guns.len() >= 3 && guns.iter().all(|g| g.starts_with("halo_ce:")), "its own game's guns: {guns:?}");
    assert!(holds_of(&chars.0, "nobody:here").is_empty());
    sim.app.world_mut().write_message(BenchAction::HoldsOpen);
    frame(&mut sim);
    assert!(sim.app.world().resource::<BenchUi>().holds_open);
    let w = sim.app.world_mut();
    let list = w.query::<(Entity, &UiParityId)>().iter(w).find(|(_, p)| p.0 == "bench.holds.list").map(|(e, _)| e).expect("the list drawn");
    let rows = w.get::<Children>(list).map_or(0, |c| c.len());
    assert_eq!(rows, guns.len() + 1, "its default and each gun");
    // A press holds it: the target is asked to change.
    sim.app.world_mut().write_message(BenchAction::Hold(t.id, Some(guns[1].clone())));
    frame(&mut sim);
    frame(&mut sim);
    let now = sim.app.world().resource::<BenchView>().targets[0].clone();
    assert_eq!(now.spec.gun.as_deref(), Some(guns[1].as_str()), "it holds the gun");
    assert!(!sim.app.world().resource::<BenchUi>().holds_open, "the list closed");
    // Esc closes an open list and leaves the bench open.
    sim.app.world_mut().write_message(BenchAction::HoldsOpen);
    frame(&mut sim);
    press(&mut sim, &[KeyCode::Escape]);
    frame(&mut sim);
    assert!(!sim.app.world().resource::<BenchUi>().holds_open);
    assert_eq!(ui(&sim).screen, SandboxScreen::Bench);
}

#[test]
fn the_bench_places_a_target_on_the_platform_where_it_stands_still() {
    use bevy::ui::DeprecatedInteraction as Interaction;
    use chimera_client::client::sandbox::{
        bench::BenchAction,
        link::{BenchView, TargetMotion, TargetPlace},
    };
    let mut sim = sim("bench");
    let act = |sim: &mut Sim, a: BenchAction| {
        sim.app.world_mut().write_message(a);
        frame(sim);
        frame(sim);
        sim.app.world().resource::<BenchView>().targets[0].clone()
    };
    let t = sim.app.world().resource::<BenchView>().targets[0].clone();
    assert_eq!(t.spec.place, TargetPlace::Line);
    // The options of a row that take a press (the greyed ones don't).
    let pressable = |sim: &mut Sim, row: &str| {
        let w = sim.app.world_mut();
        let r = w.query::<(Entity, &UiParityId)>().iter(w).find(|(_, p)| p.0 == row).map(|(e, _)| e).unwrap_or_else(|| panic!("{row} drawn"));
        let mut n = 0;
        let mut stack = vec![r];
        while let Some(e) = stack.pop() {
            n += usize::from(w.get::<BenchAction>(e).is_some() && w.get::<Interaction>(e).is_some());
            stack.extend(w.get::<Children>(e).map(|c| c.iter().collect::<Vec<_>>()).unwrap_or_default());
        }
        n
    };
    assert_eq!(pressable(&mut sim, "bench.place"), 2, "Line and Platform");
    assert_eq!(pressable(&mut sim, "bench.moves"), 3);
    // Strafing, then onto the platform: it stands still there.
    let t = act(&mut sim, BenchAction::Motion(t.id, TargetMotion::Strafes));
    assert_eq!(t.spec.motion, TargetMotion::Strafes);
    let t = act(&mut sim, BenchAction::Place(t.id, TargetPlace::Platform));
    assert_eq!((t.spec.place, t.spec.motion), (TargetPlace::Platform, TargetMotion::Still));
    assert_eq!(pressable(&mut sim, "bench.moves"), 1, "Strafes and Walks greyed out");
    let t = act(&mut sim, BenchAction::Motion(t.id, TargetMotion::Walks));
    assert_eq!(t.spec.motion, TargetMotion::Still, "no walking on the platform");
    // A distance is a line's: it goes back on one.
    let t = act(&mut sim, BenchAction::Distance(t.id, 40.0));
    assert_eq!((t.spec.place, t.spec.distance_m), (TargetPlace::Line, 40.0));
    assert_eq!(pressable(&mut sim, "bench.moves"), 3);
    let t = act(&mut sim, BenchAction::Place(t.id, TargetPlace::Platform));
    let t = act(&mut sim, BenchAction::Place(t.id, TargetPlace::Line));
    assert_eq!((t.spec.place, t.spec.distance_m), (TargetPlace::Line, 10.0), "back on the 10 m line");
}

#[test]
fn the_pickers_search_types_and_finds_and_t_does_not_close_the_bench() {
    use chimera_client::client::sandbox::bench::{BenchAction, BenchUi, CharacterCatalogue};
    let mut sim = sim("bench");
    sim.app.world_mut().write_message(BenchAction::Add);
    frame(&mut sim);
    assert!(sim.app.world().resource::<BenchUi>().picker.is_some(), "the picker open");
    type_text(&mut sim, "heavy");
    let b = sim.app.world().resource::<BenchUi>().clone();
    assert_eq!(b.picker_search, "heavy");
    let chars = sim.app.world().resource::<CharacterCatalogue>().clone();
    let found: Vec<&str> = b.found(&chars.0).iter().map(|c| c.id.as_str()).collect();
    assert_eq!(found, ["tf2:heavy"]);
    let all = BenchUi { picker_search: "fortress".into(), ..default() };
    assert_eq!(all.found(&chars.0).len(), 4, "a game's name finds its bodies");
    // T and B are letters while the picker is open.
    press(&mut sim, &[KeyCode::KeyT]);
    press(&mut sim, &[KeyCode::KeyB]);
    assert_eq!(ui(&sim).screen, SandboxScreen::Bench, "the bench stays");
    // Esc clears the search, then closes the picker.
    press(&mut sim, &[KeyCode::Escape]);
    frame(&mut sim);
    let b = sim.app.world().resource::<BenchUi>().clone();
    assert!(b.picker_search.is_empty() && b.picker.is_some());
    press(&mut sim, &[KeyCode::Escape]);
    frame(&mut sim);
    assert!(sim.app.world().resource::<BenchUi>().picker.is_none());
    assert_eq!(ui(&sim).screen, SandboxScreen::Bench);
}

/// 120 frames' instructions on this thread (None: no counter here).
fn frames_count(sim: &mut Sim, n: usize) -> Option<u64> {
    let mut counter = chimera_core::metrics::InstructionCounter::current();
    let a = counter.read()?;
    for _ in 0..n {
        frame(sim);
    }
    Some(counter.read()? - a)
}

/// The range's readouts (the look panel, a respawn ring, a blast's rings)
/// held still are rebound in place, never spawned again each frame: the
/// instructions they add over the same frames without them.
#[test]
fn the_range_readouts_rebind_in_place_instead_of_spawning_each_frame() {
    let mut range = sim("range");
    let mut rest = sim("rest");
    for _ in 0..10 {
        frame(&mut range);
        frame(&mut rest);
    }
    let entities = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        w.query::<Entity>().iter(w).count()
    };
    let before = entities(&mut range);
    let builds = sim_builds(&range);
    let counts = (frames_count(&mut range, 120), frames_count(&mut rest, 120));
    assert_eq!(entities(&mut range), before, "no entity spawned or dropped while nothing moves");
    assert_eq!(sim_builds(&range), builds, "spawned once, rebound since");
    if let (Some(r), Some(b)) = counts {
        eprintln!("120 frames: with the range's readouts {r} instructions, without {b}: the readouts {}", r.saturating_sub(b));
    } else {
        eprintln!("no hardware instruction counter here: the count is skipped");
    }
    // A move rebinds in place: the ring's number counts down only when
    // its second changes (a staged ring holds 2).
    let w = range.app.world_mut();
    let ids: Vec<String> = w.query::<&UiParityId>().iter(w).map(|p| p.0.clone()).collect();
    for id in ["range.look", "range.respawn.2", "range.blast"] {
        assert!(ids.iter().any(|i| i == id), "{id} drawn: {ids:?}");
    }
}

/// `cl_drawhud 0` (captures against the real games) hides the range's
/// readouts with every other HUD; back at 1 they return.
#[test]
fn cl_drawhud_0_hides_the_range_readouts() {
    let mut range = sim("range");
    let drawn = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        w.query::<&UiParityId>().iter(w).any(|p| p.0 == "range.look")
    };
    assert!(drawn(&mut range));
    use chimera_client::client::hud::{DRAWHUD_COVER, HudCover};
    // What `cl_drawhud 0` puts up (`hud::HudCoverPlugin`).
    range.app.world_mut().get_resource_or_insert_with(HudCover::default).set(DRAWHUD_COVER, true);
    frame(&mut range);
    frame(&mut range);
    assert!(!drawn(&mut range), "hidden under cl_drawhud 0");
    range.app.world_mut().resource_mut::<HudCover>().set(DRAWHUD_COVER, false);
    frame(&mut range);
    frame(&mut range);
    assert!(drawn(&mut range), "back");
}

/// A blast's spheres (`range::spheres_shown`) stand round it in play and
/// are gone under `cl_drawhud 0` with the readouts: a capture against
/// real TF2 showed the outer one as a hard-edged orange disc over a
/// mid-air explosion.
#[test]
fn cl_drawhud_0_hides_the_blast_spheres() {
    use sandbox::range::spheres_shown;
    let play = SandboxUi { on: true, staged: false, ..Default::default() };
    assert!(spheres_shown(&play, false), "in play");
    assert!(!spheres_shown(&play, true), "none under cl_drawhud 0");
    assert!(!spheres_shown(&SandboxUi { staged: true, ..play.clone() }, false), "none on a staging");
    assert!(!spheres_shown(&SandboxUi { screen: SandboxScreen::ChangeGun, ..play.clone() }, false), "none with Change gun open");
}

fn sim_builds(sim: &Sim) -> u64 {
    sim.app.world().resource::<sandbox::range::RangeBuilds>().0
}

// ---------------------------------------------------------------------
// Change gun laid out (Bevy's UI layout on a 1920x1080 target).

/// `sim("gun")` with the UI laid out, `edit` applied to the staging before
/// the first frame.
fn laid_out(edit: impl FnOnce(&mut World)) -> Sim {
    laid_out_as("gun", edit)
}

/// `laid_out` for another staging.
fn laid_out_as(what: &str, edit: impl FnOnce(&mut World)) -> Sim {
    use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.init_resource::<Assets<Image>>()
            .init_resource::<Assets<bevy::image::TextureAtlasLayout>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Touches>()
            .init_resource::<bevy::picking::hover::HoverMap>()
            .init_resource::<bevy::picking::pointer::PointerMap>()
            .init_resource::<bevy::picking::events::PointerState>()
            .add_message::<bevy::picking::pointer::PointerInput>()
            .add_message::<bevy::picking::backend::PointerHits>()
            .init_asset::<Font>();
        app.add_plugins((bevy::text::TextPlugin, bevy::ui::UiPlugin));
        app.add_plugins((GreyboxMapPlugin, SandboxUiPlugin));
    });
    sim.app.insert_resource(Theme::from_toml(SIGNAL).expect("theme"));
    sim.app.insert_resource(UiClock::manual());
    let size = UVec2::new(1920, 1080);
    sim.app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: ComputedCameraValues { target_info: Some(RenderTargetInfo { physical_size: size, scale_factor: 1.0 }), ..default() },
            viewport: Some(Viewport { physical_size: size, ..default() }),
            ..default()
        },
    ));
    sim.app.insert_resource(chimera_ui::UiUnits { height: 1080.0, scale_factor: 1.0, ..default() });
    sandbox::stage(sim.app.world_mut(), &[what.to_string()]).expect("staged");
    edit(sim.app.world_mut());
    for _ in 0..8 {
        frame(&mut sim);
    }
    sim
}

/// A node's left and right edges and its top and bottom (physical px).
fn edges(sim: &mut Sim, id: &str) -> (Vec2, Vec2) {
    use bevy::ui::{ComputedNode, UiGlobalTransform};
    let w = sim.app.world_mut();
    let (_, n, t) = w
        .query::<(&UiParityId, &ComputedNode, &UiGlobalTransform)>()
        .iter(w)
        .find(|(p, _, _)| p.0 == id)
        .unwrap_or_else(|| panic!("no node {id}"));
    (t.translation - n.size * 0.5, t.translation + n.size * 0.5)
}

#[test]
fn change_gun_opens_scrolled_to_the_gun_in_hand() {
    // A long catalogue, the gun in hand in its last class: the grid opens
    // with that tile in view, not at the top.
    const HELD: &str = "tf2:tf_weapon_bat";
    let mut sim = laid_out(|w| {
        let mut cat = w.resource_mut::<SandboxCatalogue>();
        let like = cat.guns.iter().find(|g| g.id == "cs_source:weapon_glock").expect("a pistol").clone();
        for n in 0..120 {
            let id = format!("kit:extra_{n}");
            let rung = cat.guns.len();
            cat.guns.push(sandbox::model::SandboxGun { id: id.clone(), family: id, rung, name: format!("Extra {n}"), unit: None, ..like.clone() });
        }
        let mut ui = w.resource_mut::<SandboxUi>();
        ui.current = Some(HELD.into());
        ui.view.selected = Some(HELD.into());
    });
    let grid = sim.app.world().resource::<Grid>().clone();
    let cat = sim.app.world().resource::<SandboxCatalogue>().clone();
    let held = cat.guns.iter().position(|g| g.id == HELD).unwrap();
    assert_eq!(cursor(&sim), grid.cursor_of(held), "the cursor on the gun in hand");
    let st = sim.app.world().resource::<VirtualLists>().get(LIST).unwrap().clone();
    let line = st.layout.line_of(st.cursor.unwrap()).unwrap();
    let (top, bottom) = (st.layout.tops[line], st.layout.tops[line] + st.layout.heights[line]);
    let view = sim.app.world().resource::<chimera_ui::ScrollViews>().get(LIST).expect("the grid's view").clone();
    assert!(view.view > 0.0 && view.max > 10.0, "laid out and taller than its view: {view:?}");
    assert!(top > view.view, "the tile starts below the first view: {top}");
    assert!(view.offset <= top + 0.01 && bottom <= view.offset + view.view + 0.01, "its row {top}..{bottom} in view at {} (+{})", view.offset, view.view);
    // And its tile is drawn.
    let (a, b) = edges(&mut sim, &format!("gun.tile.{HELD}"));
    let (va, vb) = edges(&mut sim, "gun.detail");
    assert!(a.y >= va.y - 1.0 && b.y <= vb.y + 1.0, "the tile {a}..{b} inside the body {va}..{vb}");
}

#[test]
fn a_long_name_keeps_the_details_icon_in_the_pane() {
    const GUN: &str = "cs_source:weapon_ak47";
    const LONG: &str = "Bushmaster Carbon 15 Type 97S Anniversary Gold Edition Mk II";
    let mut sim = laid_out(|w| {
        let mut cat = w.resource_mut::<SandboxCatalogue>();
        cat.guns.iter_mut().find(|g| g.id == GUN).unwrap().name = LONG.into();
    });
    let (pane_a, pane_b) = edges(&mut sim, "gun.detail");
    let (icon_a, icon_b) = edges(&mut sim, "gun.detail.icon");
    let (name_a, name_b) = edges(&mut sim, "gun.detail.name");
    assert!(icon_b.x - icon_a.x > 4.0, "the icon has its width: {icon_a}..{icon_b}");
    assert!(icon_b.x <= pane_b.x + 0.5, "the icon ends inside the pane: {} > {}", icon_b.x, pane_b.x);
    assert!(name_a.x >= pane_a.x - 0.5 && name_b.x <= icon_a.x + 0.5, "the name {name_a}..{name_b} ends before the icon at {}", icon_a.x);
    let w = sim.app.world_mut();
    let shown = w.query::<(&UiParityId, &Text)>().iter(w).find(|(p, _)| p.0 == "gun.detail.name").map(|(_, t)| t.0.clone()).unwrap();
    assert!(shown.ends_with("...") || shown.ends_with('\u{2026}'), "cut with the kit's ellipsis: {shown}");
    assert!(LONG.to_uppercase().starts_with(shown.trim_end_matches(['.', '\u{2026}']).trim_end()), "its own start: {shown}");
}

/// The target picker is a fixed dialog: its characters are the kit's list
/// (tiles bound as slots, scrolling inside), whatever the number of
/// options, and its search is the standard text field (a caret, a
/// selection: `PickerSearch` on a `TextField`).
#[test]
fn the_target_picker_lists_characters_in_a_scrolling_list_inside_the_dialog() {
    use chimera_client::client::sandbox::bench::{BenchAction, PICKER_LIST, PickerSearch};
    let mut sim = laid_out_as("bench-pick", |_| {});
    let w = sim.app.world_mut();
    let tiles = w.query::<&BenchAction>().iter(w).filter(|a| matches!(a, BenchAction::PickerChoose(_))).count();
    assert!(tiles >= 4, "character tiles are bound in the list ({tiles})");
    let boxes: Vec<(Vec2, Vec2)> = w
        .query::<(&BenchAction, &bevy::ui::ComputedNode, &bevy::ui::UiGlobalTransform)>()
        .iter(w)
        .filter(|(a, ..)| matches!(a, BenchAction::PickerChoose(_)))
        .map(|(_, n, t)| (t.translation, n.size))
        .collect();
    assert!(boxes.iter().all(|(_, s)| s.x > 10.0 && s.y > 10.0), "tiles have a size: {boxes:?}");
    assert_eq!(w.query::<&PickerSearch>().iter(w).count(), 1, "the standard search field");
    assert!(w.resource::<VirtualLists>().get(PICKER_LIST).is_some());
    // The buttons stay inside the plate.
    let (plate_lo, plate_hi) = edges(&mut sim, "bench.picker");
    let w = sim.app.world_mut();
    let uses = w.query::<(&BenchAction, &bevy::ui::ComputedNode, &bevy::ui::UiGlobalTransform)>().iter(w).find(|(a, ..)| **a == BenchAction::PickerUse).map(|(_, n, t)| (t.translation - n.size * 0.5, t.translation + n.size * 0.5)).unwrap();
    assert!(uses.1.y <= plate_hi.y + 1.0 && uses.0.y >= plate_lo.y, "Use is inside the dialog");
}

/// Change gun's grid is the kit's shared picker (docs/plans/active/
/// shared-picker.md, step 3): single pick, cards, the detail its side
/// pane; a card has its gun's icon and foot (the picker's slots, filled
/// by the screen) and selects through the list; a class header's click
/// folds it in the picker's state and its list at once, the folded header
/// showing its preview.
#[test]
fn change_gun_is_the_shared_picker() {
    use chimera_ui::picker::{PickerAction, PickerCard, PickerLook, PickerMode, PickerPart, PickerSlot, Pickers};
    let mut sim = laid_out(|_| {});
    let w = sim.app.world_mut();
    let view = w.resource::<Pickers>().0.get(LIST).expect("the grid's picker").clone();
    assert_eq!((view.frame.look, view.frame.mode), (PickerLook::Cards, PickerMode::Single));
    assert!(view.frame.side.is_some() && view.frame.foot, "the detail's side pane, the cards' feet");
    let cards = w.query::<&PickerCard>().iter(w).count();
    assert!(cards > 0, "cards drawn");
    assert_eq!(w.query_filtered::<(), (With<PickerCard>, With<chimera_ui::VHit>)>().iter(w).count(), cards, "a card selects through the list");
    let parts: Vec<PickerPart> = w.query::<&PickerSlot>().iter(w).filter(|s| s.list == LIST).map(|s| s.part).collect();
    assert_eq!(parts.iter().filter(|p| **p == PickerPart::Art).count(), cards, "every card's gun icon is the screen's");
    assert_eq!(parts.iter().filter(|p| **p == PickerPart::Foot).count(), cards);
    // Every header folds by a click (`PickerAction::Fold`); the keys: ↑
    // to the gun's class header, ← folds it (state and list).
    assert!(w.query::<&PickerAction>().iter(w).any(|a| matches!(a, PickerAction::Fold(_))), "headers fold on a click");
    while !matches!(cursor(&sim), Some(VCursor::Header(_))) {
        press(&mut sim, &[KeyCode::ArrowUp]);
    }
    let Some(VCursor::Header(sec)) = cursor(&sim) else { unreachable!() };
    press(&mut sim, &[KeyCode::ArrowLeft]);
    frame(&mut sim);
    let st = sim.app.world().resource::<VirtualLists>().get(LIST).unwrap().clone();
    assert!(st.is_folded(sec), "the list folds it");
    let gp = sim.app.world().resource::<sandbox::change_gun::GunPicker>().clone();
    assert_eq!(gp.0.folded.len(), 1, "and the picker's state");
    let w = sim.app.world_mut();
    assert!(w.query::<&PickerSlot>().iter(w).any(|s| s.list == LIST && s.part == PickerPart::Head), "the folded header's preview");
}

/// Change gun's search is the picker's own bar (no field in the head), and
/// it still matches as the screen always did: a gun's game (long and
/// short), section, tags and its game's tags, not only name and id. The
/// cards carry that text (`PickerItem::search_text`), so the picker's
/// matching and the tiles' agree.
#[test]
fn change_gun_search_is_the_pickers_bar_with_the_screens_wider_matching() {
    use chimera_ui::picker::{PickerSearch, Pickers};
    let mut sim = laid_out(|_| {});
    let w = sim.app.world_mut();
    assert_eq!(w.query::<&PickerSearch>().iter(w).filter(|s| s.0 == LIST).count(), 1, "the picker's search field, the only one");
    assert!(w.resource::<Pickers>().0[LIST].frame.search, "the frame asks for the bar");
    let cat = w.resource::<SandboxCatalogue>().clone();
    let cards = w.resource::<sandbox::change_gun::GunCards>().0.clone();
    assert!(cards.iter().flat_map(|g| &g.groups).flat_map(|g| &g.items).all(|i| i.search_text.is_some()), "every card carries what the search matches");
    // A word only the wide text has (the gun's game, long and short): the
    // picker alone, without the screen's tiles, lets its cards through.
    let g = cat.guns.iter().find(|g| !g.game_short.is_empty()).expect("a gun with a game");
    for word in [g.game_short.to_lowercase(), g.game_name.to_lowercase()] {
        let st = chimera_ui::picker::PickerState { search: word.clone(), ..default() };
        let shown = |games: &[chimera_ui::picker::PickerGame]| st.rows(games).iter().filter(|r| chimera_ui::picker::PickerState::item(games, r).is_some_and(|i| i.id == g.id)).count();
        assert_eq!(shown(&cards), 1, "`{word}` finds {}", g.id);
    }
    // Typing goes through the picker's state, and the tiles follow.
    type_text(&mut sim, "/awp");
    assert_eq!(gp(&sim).search, "awp");
    frame(&mut sim);
    assert_eq!(ui(&sim).view.search, "awp", "the tiles' filter follows the picker's search");
}

/// The sandbox's map picker (M; docs/plans/completed/shared-picker.md, step
/// 4): the kit's picker, single pick, rows by game with the sandbox's own
/// first and the map in play the current one; `/` and typing find a map,
/// Esc lets go of the field and then clears it; Enter on a map submits
/// `sandbox <map>` (the main menu's Sandbox entry's path) and closes the
/// view; M opens it again.
#[test]
fn the_map_picker_finds_and_changes_map_by_the_sandbox_command() {
    use chimera_client::client::sandbox::change_map::{self, MapCards, MapPicker};
    use chimera_core::console::ConsoleAppExt;
    #[derive(Resource, Default)]
    struct Asked(Vec<String>);
    let mut sim = sim("map");
    sim.app.init_resource::<Asked>().console_command("test:sandbox", "sandbox <map> (recorded)", |w, a| {
        w.resource_mut::<Asked>().0.push(a.join(" "));
        Ok(None)
    });
    let w = sim.app.world_mut();
    assert_eq!(w.query_filtered::<(), With<change_map::ChangeMapView>>().iter(w).count(), 1, "the view is up");
    let view = w.resource::<chimera_ui::picker::Pickers>().0.get(change_map::LIST).expect("its picker").clone();
    assert_eq!((view.frame.look, view.frame.mode), (chimera_ui::picker::PickerLook::List, chimera_ui::picker::PickerMode::Single));
    assert_eq!(view.games[0].id, change_map::OWN, "the sandbox's own maps first");
    assert_eq!(w.resource::<MapPicker>().0.current.as_deref(), Some("cs_source:de_dust2"));
    // Typing finds a map.
    type_text(&mut sim, "/lock");
    frame(&mut sim);
    let (p, cards) = (sim.app.world().resource::<MapPicker>().0.clone(), sim.app.world().resource::<MapCards>().0.clone());
    assert_eq!(p.search, "lock");
    let items: Vec<String> = p.rows(&cards).iter().filter_map(|r| chimera_ui::picker::PickerState::item(&cards, r)).map(|i| i.id.clone()).collect();
    assert_eq!(items, ["hl2dm:dm_lockdown"]);
    press(&mut sim, &[KeyCode::Escape]);
    assert!(!sim.app.world().resource::<MapPicker>().0.search_focus, "Esc lets go of the field");
    press(&mut sim, &[KeyCode::Escape]);
    assert!(sim.app.world().resource::<MapPicker>().0.search.is_empty(), "then clears it");
    assert_eq!(ui(&sim).screen, SandboxScreen::ChangeMap, "still open");
    // Enter on a map: `sandbox <map>`, the view closed.
    press(&mut sim, &[KeyCode::ArrowDown]);
    press(&mut sim, &[KeyCode::Enter]);
    frame(&mut sim);
    frame(&mut sim);
    assert_eq!(ui(&sim).screen, SandboxScreen::None, "picked: closed");
    let said: Vec<String> = sim.app.world().resource::<chimera_core::console::Console>().output.iter().map(|l| l.text.clone()).collect();
    assert_eq!(sim.app.world().resource::<Asked>().0, ["range"], "the first row under the header: the range ({said:?})");
    // M opens it again.
    press(&mut sim, &[KeyCode::KeyM]);
    assert_eq!(ui(&sim).screen, SandboxScreen::ChangeMap, "M opens Change map");
}
