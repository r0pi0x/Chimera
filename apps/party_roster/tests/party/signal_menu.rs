//! The party menu's first page in the Signal look (`party_roster::
//! signal_menu`, M5 of docs/plans/active/ugg-ui.md), headless on a manual
//! UI clock: drawn only for a theme with `[layout.menu]` (the VGUI list
//! otherwise), four entries from the data, the highlight following the
//! keys and the pointer and Enter acting (hover selects, the slab shows
//! the hovered entry inverted, keys go on from it and take the lead from a
//! still pointer until it moves), the entrance on the clock, the
//! backdrops' rotation (the Ladder its whole climb, the others 16 s; a
//! tab resets the dwell; none with reduced motion), `ui_scene menu`.

use bevy::prelude::*;
use chimera_ui::{ChimeraUiPlugin, Prop, Theme, Timeline, UiClock};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::harness::CaSim;
use party_roster::{
    PartyPartsPlugin,
    main_menu::{MenuData, MenuPage, PartyMenu},
    menu_backdrops::LADDER_PERIOD,
    signal_menu::{SignalHighlight, SignalInverse, SignalItem, SignalMenu, SignalMenuRoot, SignalTab},
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
fn menu_has_five_entries_from_data() {
    let mut sim = sim_with(MENU_THEME);
    assert_eq!(count::<SignalMenuRoot>(&mut sim), 1);
    let entries = sim.app.world().resource::<MenuData>().entries.len();
    assert_eq!(entries, 5, "Play, Join, Sandbox, Options, Quit");
    let mut items: Vec<usize> = sim
        .app
        .world_mut()
        .query::<&SignalItem>()
        .iter(sim.app.world())
        .map(|i| i.0)
        .collect();
    items.sort();
    assert_eq!(items, vec![0, 1, 2, 3, 4]);
    // Sandbox's NEW chip, in the entry and in the highlight's copy.
    let tags = sim.app.world_mut().query::<&chimera_ui::UiParityId>().iter(sim.app.world()).filter(|p| p.0.starts_with("menu.item.3.tag")).count();
    assert_eq!(tags, 2, "the NEW chip on Sandbox");
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
    assert_eq!(state(&sim).sel, 4, "wraps to the last");
    press(&mut sim, KeyCode::ArrowDown);
    press(&mut sim, KeyCode::ArrowDown);
    // The highlight glides there on the clock.
    step(&mut sim, 1.0);
    let y = sim
        .app
        .world_mut()
        .query_filtered::<&Timeline, With<party_roster::signal_menu::SignalHighlight>>()
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
    party_roster::signal_menu::stage(sim.app.world_mut(), &["ident".into(), "3.2".into()]).unwrap();
    sim.ticks(2);
    let s = state(&sim);
    assert_eq!((s.backdrop, s.freeze, s.auto), (2, Some(3.2), false));
    step(&mut sim, 30.0);
    assert_eq!(state(&sim).backdrop, 2, "held");
}

#[test]
fn the_ground_is_the_ambient_calm_behind_the_menu() {
    use chimera_ui::AmbientMaterial;
    use party_roster::signal_menu::MenuAmbient;
    let mut sim = sim_with(MENU_THEME);
    let ambient = |sim: &mut Sim| -> AmbientMaterial {
        let h = sim
            .app
            .world_mut()
            .query_filtered::<&MaterialNode<AmbientMaterial>, With<MenuAmbient>>()
            .single(sim.app.world())
            .expect("one ambient under the page")
            .0
            .clone();
        sim.app
            .world()
            .resource::<Assets<AmbientMaterial>>()
            .get(&h)
            .unwrap()
            .clone()
    };
    let a = ambient(&mut sim);
    let (column, bloom) = AmbientMaterial::MENU;
    assert_eq!(a.place.x, column, "calm left of the menu column");
    assert_eq!(
        (a.place.y, a.place.z),
        (bloom.x, bloom.y),
        "the bloom behind the backdrops"
    );
    assert!(a.clock.y < 0.0, "live");
    // Staged (and with reduced motion): one still frame.
    party_roster::signal_menu::stage(sim.app.world_mut(), &["ladder".into()]).unwrap();
    sim.ticks(2);
    assert!(ambient(&mut sim).clock.y >= 0.0, "frozen");
}

/// The pointer over entry `i` (Bevy UI's focus system sets this each
/// frame in the app; headless the test does).
fn hover(sim: &mut Sim, i: usize) {
    let items: Vec<(Entity, usize)> = sim
        .app
        .world_mut()
        .query::<(Entity, &SignalItem)>()
        .iter(sim.app.world())
        .map(|(e, s)| (e, s.0))
        .collect();
    for (e, k) in items {
        let want = if k == i { Interaction::Hovered } else { Interaction::None };
        let mut w = sim.app.world_mut().entity_mut(e);
        if w.get::<Interaction>() != Some(&want) {
            w.insert(want);
        }
    }
    sim.ticks(1);
}

/// A real pointer move: the window's cursor somewhere new.
fn move_pointer(sim: &mut Sim, to: Vec2) {
    let w = sim.app.world_mut();
    let mut q = w.query::<&mut Window>();
    let mut win = match q.iter_mut(w).next() {
        Some(win) => win,
        None => {
            w.spawn(Window::default());
            q = w.query::<&mut Window>();
            q.iter_mut(w).next().unwrap()
        }
    };
    win.set_cursor_position(Some(to));
    sim.ticks(1);
}

fn y_of<C: Component>(sim: &mut Sim) -> f32 {
    sim.app
        .world_mut()
        .query_filtered::<&Timeline, With<C>>()
        .single(sim.app.world())
        .unwrap()
        .value(Prop::Y)
        .unwrap()
}

/// Every text's colour under `C`'s node with `text`.
fn colors_under<C: Component>(sim: &mut Sim, text: &str) -> Vec<Color> {
    let roots: Vec<Entity> =
        sim.app.world_mut().query_filtered::<Entity, With<C>>().iter(sim.app.world()).collect();
    let w = sim.app.world_mut();
    let mut kids = w.query::<&Children>();
    let mut texts = w.query::<(&Text, &TextColor)>();
    let mut out = Vec::new();
    for r in roots {
        let mut stack = vec![r];
        while let Some(e) = stack.pop() {
            if let Ok((t, c)) = texts.get(w, e)
                && t.0.eq_ignore_ascii_case(text)
            {
                out.push(c.0);
            }
            if let Ok(ch) = kids.get(w, e) {
                stack.extend(ch.iter());
            }
        }
    }
    out
}

#[test]
fn hover_selects_and_the_slab_shows_the_inverted_entry() {
    let mut sim = sim_with(MENU_THEME);
    step(&mut sim, 3.0);
    hover(&mut sim, 1);
    assert_eq!(state(&sim).sel, 1, "the pointer over JOIN selects it");
    // Through the glide the inverted copy moves against the slab, so
    // entry 2's inverted label stays inside it at every point.
    for _ in 0..8 {
        step(&mut sim, 0.05);
        let (hl, inv) = (y_of::<SignalHighlight>(&mut sim), y_of::<SignalInverse>(&mut sim));
        assert!((hl + inv).abs() < 1e-4, "the copy moves the other way: {hl} {inv}");
    }
    step(&mut sim, 1.0);
    let (hl, inv) = (y_of::<SignalHighlight>(&mut sim), y_of::<SignalInverse>(&mut sim));
    assert!((hl - 3.5).abs() < 1e-3, "the slab on JOIN: {hl}");
    assert!((inv + 3.5).abs() < 1e-3, "JOIN's inverted row at the slab: {inv}");
    let ink = sim.app.world().resource::<Theme>().tone(chimera_ui::Tone::Ink);
    let fg = sim.app.world().resource::<Theme>().tone(chimera_ui::Tone::Fg);
    let inverted = colors_under::<SignalInverse>(&mut sim, "Join");
    assert_eq!(inverted.len(), 1, "one inverted JOIN");
    assert!(inverted[0].alpha() > 0.99, "the inverted JOIN is drawn: {:?}", inverted[0]);
    assert_eq!(inverted[0].with_alpha(1.0), ink.with_alpha(1.0), "in ink on the slab");
    let play = colors_under::<SignalItem>(&mut sim, "Host");
    assert_eq!(play.len(), 1);
    assert!(play[0].alpha() > 0.99 && play[0].with_alpha(1.0) == fg.with_alpha(1.0), "HOST in its normal white: {:?}", play[0]);
}

#[test]
fn keys_after_hover_continue_from_the_hovered_entry() {
    let mut sim = sim_with(MENU_THEME);
    step(&mut sim, 3.0);
    hover(&mut sim, 1);
    assert_eq!(state(&sim).sel, 1);
    press(&mut sim, KeyCode::ArrowDown);
    assert_eq!(state(&sim).sel, 2, "on from JOIN");
}

#[test]
fn keys_take_the_lead_from_a_still_pointer_until_it_moves() {
    let mut sim = sim_with(MENU_THEME);
    step(&mut sim, 3.0);
    move_pointer(&mut sim, Vec2::new(100.0, 100.0));
    hover(&mut sim, 0);
    assert_eq!(state(&sim).sel, 0);
    press(&mut sim, KeyCode::ArrowDown);
    press(&mut sim, KeyCode::ArrowDown);
    assert_eq!(state(&sim).sel, 2, "the keys move off the hovered entry");
    // The still pointer's hover (even one that changes under it, a row
    // redrawn) doesn't pull the selection back.
    hover(&mut sim, 1);
    hover(&mut sim, 0);
    sim.ticks(3);
    assert_eq!(state(&sim).sel, 2, "a still pointer doesn't pin it");
    // A real move gives the selection back to the entry under it.
    move_pointer(&mut sim, Vec2::new(100.0, 130.0));
    assert_eq!(state(&sim).sel, 0, "the moved pointer selects the entry under it");
}

