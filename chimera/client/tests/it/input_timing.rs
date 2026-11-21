//! When a local key press reaches a command (specs/source/input_timing.md):
//! the local input runs before the fixed loop
//! (`input::LocalInputSystems`), so a press acts in the frame it comes in
//! when that frame runs a tick, else in the next frame that does; a tap
//! shorter than a tick is on in exactly one command (`input::KeyLatch`);
//! keys a menu takes or a released cursor never act; move keys send their
//! share by the spec's table (`input::move_share`).

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use chimera_client::client::{binds::CapturedKeys, front_end::MenuStatus, input::LocalInputPlugin, view::FreeCam};
use chimera_core::{
    console::Console,
    core::{HalfFirstPress, Intent, LocalPlayer},
};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

/// Each tick's command as the simulation read it, with the frame it ran in.
#[derive(Resource, Default)]
struct Cmds(Vec<(u64, Intent)>);

#[derive(Resource, Default)]
struct Frame(u64);

fn count_frame(mut f: ResMut<Frame>) {
    f.0 += 1;
}

fn record(frame: Res<Frame>, me: Single<&Intent, With<LocalPlayer>>, mut cmds: ResMut<Cmds>) {
    cmds.0.push((frame.0, (*me).clone()));
}

/// The local player on the greybox, read from the keys by the client's
/// own input, ticking every `frames_per_tick` frames.
fn local(frames_per_tick: u32) -> Sim {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins(GreyboxMapPlugin)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseMotion>()
            .init_resource::<AccumulatedMouseScroll>()
            .init_resource::<FreeCam>()
            .init_resource::<Cmds>()
            .init_resource::<Frame>()
            .add_message::<bevy::window::WindowFocused>()
            .insert_resource(HalfFirstPress(vec![placeholder::ID]))
            .add_plugins(LocalInputPlugin)
            .add_systems(First, count_frame)
            .add_systems(FixedUpdate, record);
    });
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), placeholder::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.app.world_mut().spawn(CursorOptions {
        grab_mode: CursorGrabMode::Locked,
        visible: false,
        ..default()
    });
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("bind mouse1 +attack; bind w +forward; bind s +back; bind 3 slot3");
    let tick = 1.0 / sim.tick_hz();
    sim.app
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(
            tick / frames_per_tick as f64,
        )));
    sim.ticks(2);
    // Frames to the end of the tick: the next frame's tick is the first.
    sim.app.world_mut().resource_mut::<Cmds>().0.clear();
    sim
}

/// One frame; the keys' just-pressed and just-released go (as Bevy's
/// input does at the next frame's start).
fn frame(sim: &mut Sim) {
    sim.app.update();
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
    sim.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
}

fn frame_no(sim: &Sim) -> u64 {
    sim.app.world().resource::<Frame>().0
}

fn take(sim: &mut Sim) -> Vec<(u64, Intent)> {
    std::mem::take(&mut sim.app.world_mut().resource_mut::<Cmds>().0)
}

fn mouse(sim: &mut Sim) -> Mut<'_, ButtonInput<MouseButton>> {
    sim.app.world_mut().resource_mut::<ButtonInput<MouseButton>>()
}

fn keys(sim: &mut Sim) -> Mut<'_, ButtonInput<KeyCode>> {
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>()
}

/// Run frames until one ticks; that frame's commands.
fn to_tick(sim: &mut Sim) -> Vec<(u64, Intent)> {
    for _ in 0..10 {
        frame(sim);
        let c = take(sim);
        if !c.is_empty() {
            return c;
        }
    }
    panic!("no tick in 10 frames");
}

/// Spec test 1: at one tick per frame, `+attack` pressed in frame k is on
/// in frame k's command (before, the input ran after the fixed loop and
/// the press reached frame k+1's). Test 2: at three frames per tick, a
/// press in a frame without a tick is on in the next frame's that has
/// one, and nothing earlier.
#[test]
fn a_press_reaches_the_command_of_its_own_frame() {
    let mut sim = local(1);
    mouse(&mut sim).press(MouseButton::Left);
    frame(&mut sim);
    let k = frame_no(&sim);
    let cmds = take(&mut sim);
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].0, k, "the tick of the pressing frame");
    assert!(cmds[0].1.fire, "fire on in the pressing frame's command");

    let mut sim = local(3);
    // Find a frame without a tick (the one after a tick).
    to_tick(&mut sim);
    mouse(&mut sim).press(MouseButton::Left);
    frame(&mut sim);
    assert!(take(&mut sim).is_empty(), "this frame has no tick");
    let cmds = to_tick(&mut sim);
    assert_eq!(cmds.len(), 1);
    assert!(cmds[0].1.fire, "on in the next command");
}

/// Spec test 3: `+attack` pressed and released between two commands is on
/// in the next one and off in the one after; a tap inside one frame, and
/// one spread over frames without a tick, alike. A slot key's tap selects.
/// Several ticks in one frame: only the first gets the tap.
#[test]
fn a_tap_shorter_than_a_tick_is_never_lost() {
    // Inside one frame (press and release before it runs).
    let mut sim = local(1);
    mouse(&mut sim).press(MouseButton::Left);
    mouse(&mut sim).release(MouseButton::Left);
    frame(&mut sim);
    let cmds = take(&mut sim);
    assert!(cmds[0].1.fire, "the tap fires");
    frame(&mut sim);
    assert!(!take(&mut sim)[0].1.fire, "once");

    // Over two frames without a tick.
    let mut sim = local(3);
    to_tick(&mut sim);
    mouse(&mut sim).press(MouseButton::Left);
    frame(&mut sim);
    mouse(&mut sim).release(MouseButton::Left);
    frame(&mut sim);
    let mut ticked = take(&mut sim);
    if ticked.is_empty() {
        ticked = to_tick(&mut sim);
    }
    assert!(ticked[0].1.fire, "the tap from earlier frames fires");
    assert!(!to_tick(&mut sim)[0].1.fire, "once");

    // A slot key's tap.
    keys(&mut sim).press(KeyCode::Digit3);
    keys(&mut sim).release(KeyCode::Digit3);
    frame(&mut sim);
    let mut ticked = take(&mut sim);
    if ticked.is_empty() {
        ticked = to_tick(&mut sim);
    }
    assert_eq!(ticked[0].1.select, Some(2));

    // Two ticks in one frame (a slow frame): the first has the tap.
    let mut sim = local(1);
    let tick = 1.0 / sim.tick_hz();
    sim.app
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(tick * 2.0)));
    mouse(&mut sim).press(MouseButton::Left);
    mouse(&mut sim).release(MouseButton::Left);
    frame(&mut sim);
    let fired: Vec<bool> = take(&mut sim).iter().map(|c| c.1.fire).collect();
    assert_eq!(fired, [true, false]);
}

/// Keys a menu takes (`CapturedKeys`, in `PreUpdate`) and the keys of a
/// player whose cursor a menu freed act on no command, a tap included;
/// back in the game, the key acts again.
#[test]
fn menus_still_swallow_keys() {
    let mut sim = local(1);
    fn take_attack(mut c: ResMut<CapturedKeys>, mouse: Res<ButtonInput<MouseButton>>, on: Res<MenuOn>) {
        if on.0 && mouse.just_pressed(MouseButton::Left) {
            c.capture_button(MouseButton::Left);
        }
    }
    #[derive(Resource)]
    struct MenuOn(bool);
    sim.app.insert_resource(MenuOn(true)).add_systems(
        PreUpdate,
        take_attack.after(chimera_client::client::binds::release_captured),
    );
    mouse(&mut sim).press(MouseButton::Left);
    frame(&mut sim);
    mouse(&mut sim).release(MouseButton::Left);
    frame(&mut sim);
    mouse(&mut sim).press(MouseButton::Left);
    mouse(&mut sim).release(MouseButton::Left);
    frame(&mut sim);
    assert!(take(&mut sim).iter().all(|c| !c.1.fire), "taken by the menu: no command fires");

    sim.app.insert_resource(MenuOn(false));
    // The game menu open (it frees the cursor): nothing, a tap neither.
    sim.app.insert_resource(MenuStatus {
        open: true,
        in_game: true,
        ..default()
    });
    frame(&mut sim);
    mouse(&mut sim).press(MouseButton::Left);
    mouse(&mut sim).release(MouseButton::Left);
    frame(&mut sim);
    frame(&mut sim);
    assert!(take(&mut sim).iter().all(|c| !c.1.fire), "the game menu open: no command fires");

    // Closed (the cursor grabbed again): the key acts.
    sim.app.world_mut().resource_mut::<MenuStatus>().open = false;
    frame(&mut sim);
    frame(&mut sim);
    take(&mut sim);
    let grabbed = sim.app.world_mut().query::<&CursorOptions>().single(sim.app.world()).unwrap().grab_mode;
    assert_ne!(grabbed, CursorGrabMode::None);
    mouse(&mut sim).press(MouseButton::Left);
    frame(&mut sim);
    assert!(take(&mut sim)[0].1.fire);
}

/// Spec tests 4-7: `+forward` held sends 0.5, 1, 1; a tap between two
/// commands 0.25; released and pressed again between two 0.75; released 0.
/// Above the tick rate, a press in a frame without a tick loses its half
/// step (the frame's extra sample reads it): 1 at once.
#[test]
fn move_keys_send_their_share() {
    let mut sim = local(1);
    let forward = |c: &[(u64, Intent)]| c[0].1.move_axis.y;
    keys(&mut sim).press(KeyCode::KeyW);
    let mut held = Vec::new();
    for _ in 0..3 {
        frame(&mut sim);
        held.push(forward(&take(&mut sim)));
    }
    assert_eq!(held, [0.5, 1.0, 1.0], "held");
    // Released and pressed again between two commands.
    keys(&mut sim).release(KeyCode::KeyW);
    keys(&mut sim).press(KeyCode::KeyW);
    frame(&mut sim);
    assert_eq!(forward(&take(&mut sim)), 0.75, "re-pressed");
    frame(&mut sim);
    assert_eq!(forward(&take(&mut sim)), 1.0);
    keys(&mut sim).release(KeyCode::KeyW);
    frame(&mut sim);
    assert_eq!(forward(&take(&mut sim)), 0.0, "released");
    // A tap between two commands.
    keys(&mut sim).press(KeyCode::KeyW);
    keys(&mut sim).release(KeyCode::KeyW);
    frame(&mut sim);
    assert_eq!(forward(&take(&mut sim)), 0.25, "tapped");
    frame(&mut sim);
    assert_eq!(forward(&take(&mut sim)), 0.0);
    // Back pressed while forward is held: each its own share.
    keys(&mut sim).press(KeyCode::KeyW);
    frame(&mut sim);
    frame(&mut sim);
    take(&mut sim);
    keys(&mut sim).press(KeyCode::KeyS);
    frame(&mut sim);
    assert_eq!(forward(&take(&mut sim)), 0.5, "1 - 0.5");
    keys(&mut sim).release(KeyCode::KeyW);
    keys(&mut sim).release(KeyCode::KeyS);
    frame(&mut sim);
    take(&mut sim);

    // Three frames per tick: pressed in a frame without a tick.
    let mut sim = local(3);
    to_tick(&mut sim);
    keys(&mut sim).press(KeyCode::KeyW);
    frame(&mut sim);
    assert!(take(&mut sim).is_empty());
    assert_eq!(forward(&to_tick(&mut sim)), 1.0, "the half step was read in the frame without a tick");
}

/// A menu (it frees the cursor) that closes on a press: the second press
/// of a double-click (`double`), else the first. The cursor is grabbed
/// again at once while the button is still down.
#[derive(Resource)]
struct ClosesOnPress {
    double: bool,
    presses: u32,
}

fn close_on_press(mouse: Res<ButtonInput<MouseButton>>, mut c: ResMut<ClosesOnPress>, mut menu: ResMut<MenuStatus>) {
    if menu.open && mouse.just_pressed(MouseButton::Left) {
        c.presses += 1;
        if c.presses >= if c.double { 2 } else { 1 } {
            menu.open = false;
        }
    }
}

fn menu_closing_on_press(double: bool) -> Sim {
    let mut sim = local(1);
    sim.app
        .insert_resource(ClosesOnPress { double, presses: 0 })
        .insert_resource(MenuStatus { open: true, in_game: true, ..default() })
        .add_systems(Update, close_on_press);
    frame(&mut sim);
    frame(&mut sim);
    take(&mut sim);
    sim
}

/// The cursor is grabbed (the menu closed and the game took it back).
fn grabbed(sim: &mut Sim) -> bool {
    sim.app.world_mut().query::<&CursorOptions>().single(sim.app.world()).unwrap().grab_mode != CursorGrabMode::None
}

/// A double-click on a menu item that closes the menu (the sandbox gun
/// picker's switch): neither press fires, nor the button held after the
/// menu has gone and the cursor is grabbed; once let go, a new press fires.
#[test]
fn a_double_click_that_closes_a_menu_fires_nothing() {
    let mut sim = menu_closing_on_press(true);
    assert!(!grabbed(&mut sim), "the menu freed the cursor");
    // Press, release, press: the second press closes the menu.
    mouse(&mut sim).press(MouseButton::Left);
    frame(&mut sim);
    mouse(&mut sim).release(MouseButton::Left);
    frame(&mut sim);
    mouse(&mut sim).press(MouseButton::Left);
    frame(&mut sim);
    assert!(!sim.app.world().resource::<MenuStatus>().open, "the double-click closed the menu");
    // Held on into the game.
    for _ in 0..5 {
        frame(&mut sim);
    }
    assert!(grabbed(&mut sim), "back in the game");
    let fired: Vec<u64> = take(&mut sim).iter().filter(|c| c.1.fire).map(|c| c.0).collect();
    assert!(fired.is_empty(), "the clicks meant for the menu fired in frames {fired:?}");
    mouse(&mut sim).release(MouseButton::Left);
    frame(&mut sim);
    take(&mut sim);
    mouse(&mut sim).press(MouseButton::Left);
    frame(&mut sim);
    assert!(take(&mut sim)[0].1.fire, "a new press fires");
}

/// A single click that closes a menu (Resume, a buy, a dialog's button):
/// no shot, held or tapped.
#[test]
fn a_click_that_closes_a_menu_fires_nothing() {
    for held in [true, false] {
        let mut sim = menu_closing_on_press(false);
        mouse(&mut sim).press(MouseButton::Left);
        if !held {
            mouse(&mut sim).release(MouseButton::Left);
        }
        frame(&mut sim);
        assert!(!sim.app.world().resource::<MenuStatus>().open, "the click closed the menu");
        for _ in 0..5 {
            frame(&mut sim);
        }
        assert!(grabbed(&mut sim), "back in the game");
        assert!(take(&mut sim).iter().all(|c| !c.1.fire), "held {held}: the menu's click fired");
    }
}
