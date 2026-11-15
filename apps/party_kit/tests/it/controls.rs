//! The party's key binds in layers (shared binds, then the identity's
//! game's own) and their Controls page: Shift walks for a CS:S player and
//! sprints for a Combat Arms one (`GAME_BINDS`), a mouse wheel bind on jump
//! jumps in both identities, the page rebinds shared and per-game keys, and
//! the config saves the games' own binds.

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use chimera_client::client::{
    input::{FreeLook, MouseSettings, WheelJump, apply_wheel_jump, write_local_intent},
    view::FreeCam,
};
use chimera_core::{console::Console, core::LocalPlayer};
use chimera_world::harness::Sim;
use combat_arms::game::movement::CaMovement;
use party_kit::controls::{self, Action, ControlsPage, ControlsPlugin};

use crate::gun_game::{party, place, soldier};
use crate::switches::set_rung;

/// A party whose first soldier is the local player, read from the keyboard
/// as the client reads it (`write_local_intent`, the wheel's jumps).
pub(crate) fn local_party() -> (Sim, Entity) {
    local_party_with(party())
}

/// `local_party` on `sim` (the party with a test's plugins too).
pub(crate) fn local_party_with(mut sim: Sim) -> (Sim, Entity) {
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    let app = &mut sim.app;
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseMotion>()
        .init_resource::<AccumulatedMouseScroll>()
        .init_resource::<MouseSettings>()
        .init_resource::<WheelJump>()
        .init_resource::<FreeLook>()
        .init_resource::<FreeCam>()
        .add_systems(Update, write_local_intent)
        .add_systems(FixedPreUpdate, apply_wheel_jump);
    app.world_mut().spawn(CursorOptions {
        grab_mode: CursorGrabMode::Locked,
        ..default()
    });
    app.world_mut().entity_mut(a).insert(LocalPlayer);
    sim.seconds(1.5);
    (sim, a)
}

fn submit(sim: &mut Sim, line: &str) {
    sim.app.world_mut().resource_mut::<Console>().submit(line);
    sim.ticks(1);
}

fn press(sim: &mut Sim, key: KeyCode) {
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
}

fn release(sim: &mut Sim, key: KeyCode) {
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(key);
}

/// How far (meters) `e` runs forward in a second with `held` also down.
fn run_second(sim: &mut Sim, e: Entity, held: &[KeyCode]) -> f32 {
    let from = sim.position(e);
    press(sim, KeyCode::KeyW);
    for k in held {
        press(sim, *k);
    }
    sim.seconds(1.0);
    release(sim, KeyCode::KeyW);
    for k in held {
        release(sim, *k);
    }
    let went = (sim.position(e) - from).xz().length();
    place(sim, e, from);
    sim.seconds(0.5);
    went
}

/// Item 3 of the 2026-10-24 playtest: Shift is Combat Arms' sprint in the
/// shared defaults and CS:S's walk in a CS:S player's game.
#[test]
fn shift_walks_for_a_cs_source_player_and_sprints_for_a_combat_arms_one() {
    let (mut sim, a) = local_party();
    submit(&mut sim, "bind w +forward");
    // The Glock: CS:S's identity.
    let run = run_second(&mut sim, a, &[]);
    let walk = run_second(&mut sim, a, &[KeyCode::ShiftLeft]);
    assert!(run > 1.0, "ran {run} m");
    assert!(walk < run * 0.75, "Shift in CS:S: walked {walk} m against running {run} m");
    // A Combat Arms gun: Shift sprints.
    set_rung(&mut sim, a, 1);
    sim.seconds(0.5);
    let limit = |sim: &mut Sim, held: &[KeyCode]| {
        sim.app.world_mut().get_mut::<CaMovement>(a).unwrap().stamina = 100.0;
        press(sim, KeyCode::KeyW);
        for k in held {
            press(sim, *k);
        }
        sim.ticks(8);
        let limit = sim.app.world().get::<CaMovement>(a).unwrap().speed_limit;
        release(sim, KeyCode::KeyW);
        for k in held {
            release(sim, *k);
        }
        sim.seconds(0.5);
        limit
    };
    let run = limit(&mut sim, &[]);
    let sprint = limit(&mut sim, &[KeyCode::ShiftLeft]);
    assert!(sprint > run * 1.1, "Shift in Combat Arms: speed limit {sprint} against running's {run}");
}

/// `bind mwheeldown +jump` (the page's way: a shared bind) jumps in both
/// identities, a notch at a time.
#[test]
fn a_mouse_wheel_jump_bind_jumps_in_both_games() {
    let (mut sim, a) = local_party();
    let line = controls::rebind_line(&sim.app.world().resource::<Console>(), 0, "+jump", "mwheeldown");
    submit(&mut sim, &line);
    for (rung, game) in [(0, "cs_source"), (1, "combat_arms")] {
        set_rung(&mut sim, a, rung);
        sim.seconds(1.0);
        let slot = sim.app.world().get::<chimera_core::slots::MovementSlot>(a).unwrap().0;
        assert!(slot.starts_with(game), "{slot}");
        let ground = sim.position(a).y;
        sim.app.world_mut().resource_mut::<AccumulatedMouseScroll>().delta = Vec2::new(0.0, -1.0);
        sim.ticks(1);
        sim.app.world_mut().resource_mut::<AccumulatedMouseScroll>().delta = Vec2::ZERO;
        let mut top: f32 = 0.0;
        for _ in 0..40 {
            sim.ticks(1);
            top = top.max(sim.position(a).y - ground);
        }
        assert!(top > 0.3, "{game}: a wheel notch jumped {top} m");
    }
}

/// A CS:S gun's view model in the party bobs and sways with CS:S's rules
/// (`cl_bobcycle`, `cl_wpn_sway_*`: `view_motion`) while the player moves;
/// with Combat Arms' driver holding the character (`OwnViewAnim`) it is
/// left to that driver (Combat Arms' own sway).
#[test]
fn a_cs_source_guns_view_model_bobs_while_moving_and_stands_still_otherwise() {
    use chimera_world::map::ViewModelOffset;
    let (mut sim, a) = local_party();
    submit(&mut sim, "bind w +forward");
    let offset = |sim: &Sim| sim.app.world().get::<ViewModelOffset>(a).copied();
    sim.seconds(0.5);
    let still = offset(&sim).expect("CS:S's motion made the view model's offset");
    press(&mut sim, KeyCode::KeyW);
    let mut seen = vec![];
    for _ in 0..40 {
        sim.ticks(3);
        seen.push(offset(&sim).unwrap().translation);
    }
    let spread = |axis: fn(&Vec3) -> f32| {
        let (lo, hi) = seen.iter().map(axis).fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(v), h.max(v)));
        hi - lo
    };
    assert!(
        spread(|v| v.y) > 0.002 || spread(|v| v.x) > 0.002 || spread(|v| v.z) > 0.002,
        "the view model bobbed while running (still: {still:?}, running: {:?})",
        &seen[seen.len() - 3..]
    );
    // Combat Arms' driver owns the character: CS:S's motion lets go.
    // (Standing still first: with a CS:S gun in hand Combat Arms' driver
    // lets go of the character again at once, `OwnViewAnim` removed, so
    // only a player at rest keeps the offset still; running on, as where
    // the spawn point leaves room to run, it bobs on.)
    release(&mut sim, KeyCode::KeyW);
    sim.seconds(1.0);
    sim.app.world_mut().entity_mut(a).insert(chimera_sim::weapon::view_anim::OwnViewAnim);
    sim.ticks(3);
    let frozen = offset(&sim).unwrap();
    sim.seconds(0.5);
    assert_eq!(offset(&sim).unwrap(), frozen, "Combat Arms' driver's view model isn't moved by CS:S's bob");
}

/// The page over the console's binds.
fn page_app() -> App {
    let mut app = App::new();
    app.add_plugins(chimera_core::console::ConsolePlugin)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseScroll>()
        .add_plugins(ControlsPlugin);
    app.world_mut().spawn(CursorOptions {
        grab_mode: CursorGrabMode::Locked,
        ..default()
    });
    for (k, v) in party_kit::GAME_BINDS {
        app.world_mut().resource_mut::<Console>().ship_layer(k, v);
    }
    app.world_mut().resource_mut::<Console>().submit("bind space +jump; bind shift +sprint");
    app
}

fn actions(app: &mut App) -> Vec<(Entity, Action)> {
    app.world_mut().query::<(Entity, &Action)>().iter(app.world()).map(|(e, a)| (e, *a)).collect()
}

fn click(app: &mut App, action: Action) {
    let (e, _) = actions(app).into_iter().find(|(_, a)| *a == action).expect("a button for it");
    app.world_mut().entity_mut(e).insert(Interaction::Pressed);
    app.update();
    if let Ok(mut button) = app.world_mut().get_entity_mut(e) {
        button.insert(Interaction::None);
    }
    app.update();
}

fn tap(app: &mut App, key: KeyCode) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
    app.update();
}

fn keys(app: &App, tab: usize, command: &str) -> Vec<String> {
    controls::keys_on(app.world().resource::<Console>(), tab, command)
}

/// Shared tab first, then a tab a game; the wheel can be a bind; a game's
/// own key replaces the shared one for that game only.
#[test]
fn the_controls_page_rebinds_shared_and_per_game_keys() {
    let mut app = page_app();
    app.update();
    assert!(actions(&mut app).is_empty(), "closed: no page");
    app.world_mut().resource_mut::<Console>().submit("controls");
    app.update();
    let page = actions(&mut app);
    let tabs: Vec<_> = page.iter().filter(|(_, a)| matches!(a, Action::Tab(_))).collect();
    assert_eq!(tabs.len(), 4, "Shared, Combat Arms, CS:S, HL2:DM");
    assert!(page.iter().any(|(_, a)| *a == Action::Rebind("+jump")), "Shared first: Jump");
    assert!(!page.iter().any(|(_, a)| *a == Action::Rebind("+speed")), "CS:S's walk isn't on the shared page");
    let grabbed = app.world_mut().query::<&CursorOptions>().iter(app.world()).any(|c| c.grab_mode != CursorGrabMode::None);
    assert!(!grabbed, "the page frees the mouse");

    // Jump on the wheel, from the page.
    click(&mut app, Action::Rebind("+jump"));
    assert_eq!(app.world().resource::<ControlsPage>().capture, Some("+jump"));
    app.world_mut().resource_mut::<AccumulatedMouseScroll>().delta = Vec2::new(0.0, -1.0);
    app.update();
    app.world_mut().resource_mut::<AccumulatedMouseScroll>().delta = Vec2::ZERO;
    app.update();
    assert_eq!(keys(&app, 0, "+jump"), ["mwheeldown"], "the wheel replaced space");
    assert_eq!(app.world().resource::<ControlsPage>().capture, None);

    // CS:S's tab: Walk is Shift; set it to Z.
    click(&mut app, Action::Tab(2));
    assert_eq!(keys(&app, 2, "+speed"), ["shift"]);
    click(&mut app, Action::Rebind("+speed"));
    tap(&mut app, KeyCode::KeyZ);
    app.update();
    assert_eq!(keys(&app, 2, "+speed"), ["z"]);
    let console = app.world().resource::<Console>();
    let cs = console.effective_binds(Some("cs_source"));
    assert_eq!((cs.get("shift").map(String::as_str), cs.get("z").map(String::as_str)), (Some("+sprint"), Some("+speed")), "Shift is the shared sprint again, which CS:S ignores");
    // Combat Arms and the shared binds are untouched.
    assert_eq!(console.effective_binds(Some("combat_arms")).get("shift").map(String::as_str), Some("+sprint"));
    assert_eq!(console.binds.get("shift").map(String::as_str), Some("+sprint"));
    assert_eq!(keys(&app, 0, "+jump"), ["mwheeldown"]);

    // Esc cancels a capture; Clear and Defaults.
    click(&mut app, Action::Rebind("+duck"));
    tap(&mut app, KeyCode::Escape);
    app.update();
    assert_eq!(app.world().resource::<ControlsPage>().capture, None);
    click(&mut app, Action::Clear("+speed"));
    app.update();
    assert!(keys(&app, 2, "+speed").is_empty());
    click(&mut app, Action::Defaults);
    app.update();
    assert_eq!(keys(&app, 2, "+speed"), ["shift"], "CS:S's own binds as shipped");

    // The config keeps the games' own binds.
    app.world_mut().resource_mut::<Console>().submit("bindgame cs_source x +speed");
    app.update();
    let text = chimera_core::console::config_text(app.world_mut());
    assert!(text.contains("unbindgameall") && text.contains("bindgame cs_source x +speed"), "{text}");
}

/// HL2:DM's tab (playtest 7's settings audit): its sprint, walk, duck and
/// suit zoom, and its own gun view FOV, saved in the config.
#[test]
fn hl2dm_has_its_own_controls_tab() {
    use party_kit::controls::{GAMES, tab_game, tab_named, tab_rows};
    let tab = tab_named("hl2dm").expect("an HL2:DM tab");
    assert_eq!(tab_game(tab), Some(hl2dm::game::GAME));
    let commands: Vec<_> = tab_rows(tab).iter().map(|r| r.command).collect();
    assert_eq!(commands, ["+speed", "+walk", "+duck", "+zoom"]);
    let fov = &GAMES.iter().find(|g| g.game == hl2dm::game::GAME).unwrap().settings[0];
    assert_eq!(fov.cvar, "hl2dm_viewmodel_fov");
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, chimera_core::console::ConsolePlugin))
        .insert_resource(combat_arms::game::view_model::settings());
    party_kit::add_view_model_fovs(&mut app);
    let console = app.world().resource::<Console>();
    let cvar = console.cvar("hl2dm_viewmodel_fov").expect("the cvar");
    assert!(cvar.archive);
    assert_eq!(cvar.default, "54");
}

/// A step lands on the step's grid: Combat Arms' gun view FOV starts at
/// 63.74 and goes 64, 65, ... (it showed 64.74, 65.74), and back down to
/// its least.
#[test]
fn steps_land_on_whole_values() {
    use party_kit::controls::{GAMES, nudged};
    let ca = &GAMES.iter().find(|g| g.game == combat_arms::game::GAME).unwrap().settings[0];
    assert_eq!(nudged(ca, 63.74, 1), 64.0);
    assert_eq!(nudged(ca, 64.0, 1), 65.0);
    assert_eq!(nudged(ca, 65.0, -1), 64.0);
    assert_eq!(nudged(ca, 64.0, -1), 63.74);
    assert_eq!(nudged(ca, 110.0, 1), 110.0);
    assert_eq!(party_kit::host::step_from(0.5, 0.05, 1, 0.0, 2.0), 0.55);
    assert_eq!(party_kit::host::step_from(0.55, 0.05, -1, 0.0, 2.0), 0.5);
}
