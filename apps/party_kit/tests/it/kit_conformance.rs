//! Every staged screen's controls are the kit's (docs/DESIGN.md, "UI:
//! always the kit"; the user's reports, 2026-11-21: Host drawn with the
//! old controls, the match end's Leave with no hover). Under the
//! Ultimate Gun Game's theme each `ui_scene` an app registers
//! (`UiSceneHooks`: the menu, Host's tabs and dialogs, Join, Settings, the
//! Escape menu, the report dialog, the party panel, the loading screen) is
//! staged headless and its world checked:
//!
//! - every node a player can press (an `Interaction`) carries
//!   `KitControl`: it came from a kit builder, which gives it the kit's
//!   hover, focus and selected states (a node made pressable by hand has
//!   none of them); a panel that only blocks the pointer carries
//!   `PointerCatch` (`controls::catch_pointer`), one whose states its
//!   screen draws `OwnStates` (each on ui_kit.rs's allowlist);
//! - a form control (a toggle, a segmented choice, a stepper, a dropdown)
//!   inside one of the kit's rows sits in a setting row's control column
//!   (`Ui::setting_row`: one column width, one marker a row).
//!
//! The text scan beside it, `checks/tests/it/ui_kit.rs`
//! (`controls_are_the_kits`), catches the same by the code; this one by
//! what is drawn. The first run's screens (`ui_scene onboarding`) wait
//! for their form components (docs/plans/active/onboarding.md).

use bevy::{prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_client::client::ui_scene::UiSceneHooks;
use chimera_ui::controls::{
    Chevron, KitControl, KitRow, OwnStates, PointerCatch, SegmentedTrack, SettingRowMark, StepperBorder, ToggleSlab,
};
use chimera_world::harness::Sim;

use super::main_menu::menu_bits;

/// The party menu under `theme` with every staged screen's plugin (the
/// menu's, Settings, the report dialog, the party panel).
fn sim_with(theme: &str) -> Sim {
    let theme = chimera_ui::Theme::from_toml(theme).unwrap();
    let mut sim = Sim::with(combat_arms::harness::CaSim, |app| {
        app.add_plugins((chimera_world::greybox::GreyboxMapPlugin, party_kit::PartyPartsPlugin, chimera_net::net::NetPlugin));
        app.insert_resource(theme).add_plugins(chimera_ui::ChimeraUiPlugin).insert_resource(chimera_ui::UiClock::manual());
        if !app.world().contains_resource::<Assets<Image>>() {
            app.init_asset::<Image>();
        }
        menu_bits(app, true);
        app.add_plugins((
            party_kit::settings::SettingsPlugin,
            party_kit::feedback_signal::FeedbackSignalPlugin,
            party_kit::party_signal::PartySignalPlugin,
        ));
    });
    sim.ticks(3);
    sim
}

const SIGNAL: &str = include_str!("../../../ultimate_gun_game/data/theme.toml");

/// The scenes staged, with their arguments.
const SCENES: &[(&str, &[&str])] = &[
    ("menu", &[]),
    ("play", &["server"]),
    ("play", &["game"]),
    ("play", &["guns"]),
    ("play", &["maps"]),
    ("join", &["lan"]),
    ("join", &["password"]),
    ("settings", &[]),
    ("esc", &[]),
    ("report", &[]),
    ("party", &["lead"]),
    ("load", &["ask"]),
];

/// The scenes that must be there (the rest, where the app has them).
const NEEDED: &[&str] = &["menu", "play", "join", "settings", "esc"];

fn stage(sim: &mut Sim, name: &str, args: &[&str]) -> Result<(), String> {
    let hooks = sim.app.world().resource::<UiSceneHooks>().0.clone();
    for (_, hook) in &hooks {
        let _ = hook(sim.app.world_mut(), &["off".to_string()]);
    }
    sim.ticks(2);
    let hook = hooks
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, h)| *h)
        .ok_or_else(|| format!("no scene {name}"))?;
    hook(
        sim.app.world_mut(),
        &args.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
    )?;
    sim.ticks(6);
    Ok(())
}

/// A node's components, for the failure message.
fn names(sim: &Sim, e: Entity) -> String {
    let w = sim.app.world();
    let mut n: Vec<String> = w
        .inspect_entity(e)
        .map(|it| it.map(|(_, c)| c.name().shortname().to_string()).collect())
        .unwrap_or_default();
    n.retain(|c| {
        !matches!(
            c.as_str(),
            "Node"
                | "ComputedNode"
                | "ComputedUiTargetCamera"
                | "ComputedUiRenderTargetInfo"
                | "UiTransform"
                | "UiGlobalTransform"
                | "Transform"
                | "GlobalTransform"
                | "Visibility"
                | "InheritedVisibility"
                | "ViewVisibility"
                | "ChildOf"
                | "Children"
                | "ZIndex"
        )
    });
    n.join(", ")
}

fn ancestors(w: &World, e: Entity) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut at = e;
    while let Some(p) = w.get::<ChildOf>(at) {
        at = p.parent();
        out.push(at);
    }
    out
}

fn problems(sim: &mut Sim) -> Vec<String> {
    let mut out = Vec::new();
    let w = sim.app.world_mut();
    let pressable: Vec<(Entity, bool)> = w
        .query::<(Entity, Has<KitControl>, Has<PointerCatch>, Has<OwnStates>)>()
        .iter(w)
        .filter(|(e, ..)| w.get::<Interaction>(*e).is_some())
        .map(|(e, k, p, o)| (e, k || p || o))
        .collect();
    for (e, kit) in pressable {
        if !kit {
            out.push(format!(
                "a pressable node not from the kit (no `KitControl`): {e} [{}]",
                names(sim, e)
            ));
        }
    }
    let w = sim.app.world_mut();
    let mut forms: Vec<(Entity, &str)> = Vec::new();
    forms.extend(
        w.query_filtered::<Entity, With<SegmentedTrack>>()
            .iter(w)
            .map(|e| (e, "segmented")),
    );
    forms.extend(
        w.query_filtered::<Entity, With<ToggleSlab>>()
            .iter(w)
            .map(|e| (e, "toggle")),
    );
    forms.extend(
        w.query_filtered::<Entity, With<StepperBorder>>()
            .iter(w)
            .map(|e| (e, "stepper")),
    );
    forms.extend(
        w.query_filtered::<Entity, With<Chevron>>()
            .iter(w)
            .map(|e| (e, "dropdown")),
    );
    let w = sim.app.world();
    for (e, what) in forms {
        let up = ancestors(w, e);
        let Some(row) = up.iter().find(|a| w.get::<KitRow>(**a).is_some()) else {
            continue;
        };
        if w.get::<SettingRowMark>(*row).is_none() {
            out.push(format!(
                "a {what} in a plain `Ui::row` ({row}): a setting's control goes in `Ui::setting_row`'s column"
            ));
        }
    }
    out
}

#[test]
fn every_staged_screen_uses_the_kits_controls() {
    let mut sim = sim_with(SIGNAL);
    let have: Vec<&str> = sim
        .app
        .world()
        .resource::<UiSceneHooks>()
        .0
        .iter()
        .map(|(n, _)| *n)
        .collect();
    for n in NEEDED {
        assert!(have.contains(n), "`ui_scene {n}` is registered (have: {have:?})");
    }
    let mut bad = Vec::new();
    let mut seen = 0;
    for (name, args) in SCENES {
        if !have.contains(name) {
            continue;
        }
        if let Err(e) = stage(&mut sim, name, args) {
            bad.push(format!("ui_scene {name} {}: {e}", args.join(" ")));
            continue;
        }
        seen += 1;
        for p in problems(&mut sim) {
            bad.push(format!("ui_scene {name} {}: {p}", args.join(" ")));
        }
    }
    assert!(seen >= NEEDED.len());
    bad.dedup();
    assert!(
        bad.is_empty(),
        "screens with controls not from the kit (docs/DESIGN.md, \"UI: always the kit\"; build them with the kit's \
         builders: `Ui::button`, `Ui::item`, `Ui::setting_row` and its column's `big_toggle`/`choice`/`stepper`, ...; \
         checks/tests/it/ui_kit.rs's message lists them):\n{}",
        bad.join("\n")
    );
}

#[test]
fn a_hand_made_pressable_node_is_caught() {
    let mut sim = sim_with(SIGNAL);
    stage(&mut sim, "menu", &[]).unwrap();
    let before = problems(&mut sim);
    assert!(before.is_empty(), "{before:?}");
    // What the match end's Leave was: a node made pressable by hand.
    sim.app.world_mut().spawn((Node::default(), Interaction::default()));
    let p = problems(&mut sim);
    assert_eq!(p.len(), 1, "{p:?}");
    assert!(p[0].contains("not from the kit"), "{p:?}");
}

/// The kit's rule for a choice set (docs/DESIGN.md, "UI: always the kit";
/// the user's review, 2026-11-21): one that fits the control column is a
/// segmented control (Off/On a toggle), else a dropdown; a stepper only
/// for a number, and the setting's data type says which (`Pick::Range`
/// is a number, `Pick::Choices` never is, even with digits for labels).
#[test]
fn a_choice_set_that_does_not_fit_is_a_dropdown_and_only_a_range_is_a_stepper() {
    use party_kit::{game_options, host::{self, Pick}, settings::{Widget, game_rows, widget_of, Row, View}};
    let mut all = host::settings();
    for game in 0..game_options::games().len() {
        for page in 1..3 {
            all.extend(game_rows(View { game, page }).into_iter().filter_map(|r| match r {
                Row::Setting { s, .. } => Some(s),
                _ => None,
            }));
        }
    }
    let theme = chimera_ui::Theme::from_toml(SIGNAL).unwrap();
    let sizes = chimera_ui::form::FormSizes::of(&theme);
    let (mut dropdowns, mut ranges) = (0, 0);
    for s in &all {
        match (&s.pick, widget_of(&s.pick, s.widget.as_deref())) {
            (Pick::Range { .. }, Widget::Slider { .. }) => ranges += 1,
            (Pick::Range { .. }, w) => panic!("{}: a number is a slider or stepper, not {w:?}", s.label),
            (Pick::Choices(_), Widget::Stepper | Widget::Slider { .. }) => {
                panic!("{}: a choice set is never a stepper or slider", s.label)
            }
            (Pick::Choices(c), w) => {
                let words: Vec<&str> = c.iter().map(|(_, l)| l.as_str()).collect();
                let fits = chimera_ui::form::fits_segmented(&theme, &sizes, &words, sizes.control_w);
                if w == Widget::Dropdown {
                    dropdowns += 1;
                } else if w == Widget::Segmented {
                    assert!(fits, "{}: segmented but {words:?} don't fit", s.label);
                }
                if !fits && c.len() > 1 && w != Widget::Toggle {
                    assert_eq!(w, Widget::Dropdown, "{}: {words:?} don't fit: a dropdown", s.label);
                }
            }
        }
    }
    assert!(dropdowns >= 4 && ranges >= 1, "the data has both ({dropdowns} dropdowns, {ranges} ranges)");
}

/// The same, drawn: Host's Game tab (choice sets only, four of them too
/// long for segments) has dropdowns and no stepper; Host's Server tab (a
/// number or two) has steppers.
#[test]
fn hosts_game_tab_draws_dropdowns_not_steppers() {
    let mut sim = sim_with(SIGNAL);
    let count = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        let steppers = w.query::<&StepperBorder>().iter(w).count();
        let chevrons = w.query::<&Chevron>().iter(w).count();
        (steppers, chevrons)
    };
    stage(&mut sim, "play", &["game"]).unwrap();
    let (steppers, chevrons) = count(&mut sim);
    assert_eq!(steppers, 0, "a choice set is never a stepper");
    assert!(chevrons >= 4, "Infected odds, Melee rungs, Avoid recent maps and Spawn protection are dropdowns ({chevrons})");
    stage(&mut sim, "play", &["server"]).unwrap();
    let (steppers, _) = count(&mut sim);
    assert!(steppers >= 1, "a number is a stepper ({steppers})");
}
