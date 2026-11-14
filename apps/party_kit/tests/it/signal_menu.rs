//! The party menu's first page in the Signal look (`party_kit::
//! signal_menu`, M5 of docs/plans/active/ugg-ui.md), headless on a manual
//! UI clock: drawn only for a theme with `[layout.menu]` (the VGUI list
//! otherwise), four entries from the data, the highlight following the
//! keys and the pointer and Enter acting, the entrance on the clock, the
//! backdrops' rotation (the Ladder its whole climb, the others 16 s; a
//! tab resets the dwell; none with reduced motion), `ui_scene menu`.

use bevy::prelude::*;
use chimera_ui::{ChimeraUiPlugin, Prop, Theme, Timeline, UiClock};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::harness::CaSim;
use party_kit::{
    PartyPartsPlugin,
    main_menu::{MenuData, MenuPage, PartyMenu},
    menu_backdrops::LADDER_PERIOD,
    signal_menu::{SignalItem, SignalMenu, SignalMenuRoot, SignalTab},
};

use super::main_menu::menu_bits;

/// A theme that lays the menu out (the numbers default to the look
/// book's).
pub(crate) const MENU_THEME: &str = r#"
name = "test"
[shape]
slant_deg = 12.0
[layout.menu]
items_y = 23.4
[motion.dur.menu]
backdrop_dwell = 16.0
backdrop_swap = 0.52
"#;

pub(crate) fn sim_with(theme: &str) -> Sim {
    let theme = Theme::from_toml(theme).unwrap();
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin, chimera_net::net::NetPlugin));
        app.insert_resource(theme)
            .add_plugins(ChimeraUiPlugin)
            .insert_resource(UiClock::manual());
        if !app.world().contains_resource::<Assets<Image>>() {
            app.init_asset::<Image>();
        }
        menu_bits(app, true);
    });
    sim.ticks(3);
    sim
}

pub(crate) fn step(sim: &mut Sim, secs: f32) {
    sim.app.world_mut().resource_mut::<UiClock>().step(secs);
    sim.ticks(1);
}

fn count<C: Component>(sim: &mut Sim) -> usize {
    sim.app
        .world_mut()
        .query_filtered::<(), With<C>>()
        .iter(sim.app.world())
        .count()
}

fn state(sim: &Sim) -> &SignalMenu {
    sim.app.world().resource::<SignalMenu>()
}

fn press(sim: &mut Sim, key: KeyCode) {
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
    sim.ticks(1);
    let mut k = sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    k.release(key);
    k.clear();
    sim.ticks(1);
}

#[test]
fn menu_has_four_entries_from_data() {
    let mut sim = sim_with(MENU_THEME);
    assert_eq!(count::<SignalMenuRoot>(&mut sim), 1);
    let entries = sim.app.world().resource::<MenuData>().entries.len();
    assert_eq!(entries, 4, "Play, Join, Options, Quit");
    let mut items: Vec<usize> = sim
        .app
        .world_mut()
        .query::<&SignalItem>()
        .iter(sim.app.world())
        .map(|i| i.0)
        .collect();
    items.sort();
    assert_eq!(items, vec![0, 1, 2, 3]);
    assert_eq!(count::<SignalTab>(&mut sim), 3, "A, B, C");
}

#[test]
fn without_the_menu_layout_the_vgui_list_draws() {
    let mut sim = sim_with("name = \"plain\"\n");
    assert_eq!(count::<SignalMenuRoot>(&mut sim), 0);
}

#[test]
fn keys_move_the_highlight_and_enter_acts() {
    let mut sim = sim_with(MENU_THEME);
    assert_eq!(state(&sim).sel, 0);
    press(&mut sim, KeyCode::ArrowDown);
    assert_eq!(state(&sim).sel, 1);
    press(&mut sim, KeyCode::ArrowUp);
    press(&mut sim, KeyCode::ArrowUp);
    assert_eq!(state(&sim).sel, 3, "wraps to the last");
    press(&mut sim, KeyCode::ArrowDown);
    press(&mut sim, KeyCode::ArrowDown);
    // The highlight glides there on the clock.
    step(&mut sim, 1.0);
    let y = sim
        .app
        .world_mut()
        .query_filtered::<&Timeline, With<party_kit::signal_menu::SignalHighlight>>()
        .single(sim.app.world())
        .unwrap()
        .value(Prop::Y)
        .unwrap();
    assert!((y - 3.5).abs() < 1e-3, "the bar on the second entry: {y}");
    press(&mut sim, KeyCode::Enter);
    sim.ticks(2);
    assert_eq!(
        sim.app.world().resource::<PartyMenu>().page,
        MenuPage::Join,
        "Enter opens the entry's page"
    );
}

#[test]
fn entries_step_in_on_the_clock() {
    let mut sim = sim_with(MENU_THEME);
    let opacities = |sim: &mut Sim| -> Vec<f32> {
        let mut v: Vec<(usize, f32)> = sim
            .app
            .world_mut()
            .query::<(&SignalItem, &Timeline)>()
            .iter(sim.app.world())
            .map(|(i, t)| (i.0, t.value(Prop::Opacity).unwrap_or(1.0)))
            .collect();
        v.sort_by_key(|x| x.0);
        v.into_iter().map(|x| x.1).collect()
    };
    step(&mut sim, 0.0);
    assert!(opacities(&mut sim).iter().all(|o| *o == 0.0), "hidden at first");
    step(&mut sim, 0.3);
    let o = opacities(&mut sim);
    assert!(o[0] > 0.0 && o[0] > o[3], "one after another: {o:?}");
    step(&mut sim, 1.5);
    assert!(opacities(&mut sim).iter().all(|o| (*o - 1.0).abs() < 1e-3));
}

#[test]
fn backdrops_rotate_every_16_s() {
    let mut sim = sim_with(MENU_THEME);
    assert_eq!(state(&sim).backdrop, 0);
    // The Ladder dwells its whole climb.
    step(&mut sim, LADDER_PERIOD - 0.1);
    assert_eq!(state(&sim).backdrop, 0);
    step(&mut sim, 0.2);
    assert_eq!(state(&sim).backdrop, 1, "Assembly after the climb");
    assert_eq!(state(&sim).previous, Some(0), "the wipe from the Ladder");
    step(&mut sim, 0.6);
    assert_eq!(state(&sim).previous, None, "the wipe is over");
    step(&mut sim, 15.0);
    assert_eq!(state(&sim).backdrop, 1);
    step(&mut sim, 1.0);
    assert_eq!(state(&sim).backdrop, 2, "the Ident after 16 s");
    step(&mut sim, 16.0);
    assert_eq!(state(&sim).backdrop, 0, "and round");
}

#[test]
fn backdrop_tab_click_resets_the_dwell() {
    let mut sim = sim_with(MENU_THEME);
    step(&mut sim, 5.0);
    let tab = sim
        .app
        .world_mut()
        .query::<(Entity, &SignalTab)>()
        .iter(sim.app.world())
        .find(|(_, t)| t.0 == 2)
        .map(|(e, _)| e)
        .unwrap();
    sim.app.world_mut().entity_mut(tab).insert(Interaction::Pressed);
    sim.ticks(1);
    assert_eq!(state(&sim).backdrop, 2);
    step(&mut sim, 15.0);
    assert_eq!(state(&sim).backdrop, 2, "a full dwell from the click");
    step(&mut sim, 1.1);
    assert_eq!(state(&sim).backdrop, 0);
}

#[test]
fn reduced_motion_holds_the_backdrop() {
    let mut sim = sim_with(MENU_THEME);
    sim.app
        .world_mut()
        .insert_resource(chimera_client::client::logo_pass::ReduceMotion { on: 1 });
    // Built again for reduced motion: no auto swap.
    step(&mut sim, 0.0);
    assert!(!state(&sim).auto);
    step(&mut sim, 60.0);
    assert_eq!(state(&sim).backdrop, 0);
}

#[test]
fn ui_scene_menu_stages_a_backdrop_frozen() {
    let mut sim = sim_with(MENU_THEME);
    // What `ui_scene menu ident 3.2` calls.
    party_kit::signal_menu::stage(sim.app.world_mut(), &["ident".into(), "3.2".into()]).unwrap();
    sim.ticks(2);
    let s = state(&sim);
    assert_eq!((s.backdrop, s.freeze, s.auto), (2, Some(3.2), false));
    step(&mut sim, 30.0);
    assert_eq!(state(&sim).backdrop, 2, "held");
}
