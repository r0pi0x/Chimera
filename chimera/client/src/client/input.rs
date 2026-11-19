//! Local intent source: keyboard and mouse write the local player's `Intent`.

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::core::{Intent, LocalPlayer};

/// Mouse look as CS:S does it: each count of raw mouse motion (no
/// acceleration) turns the view by `sensitivity * m_yaw` degrees
/// sideways and `sensitivity * m_pitch` degrees up or down (negative
/// `m_pitch` inverts).
#[derive(Resource, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Resource)]
pub struct MouseSettings {
    pub sensitivity: f32,
    pub m_yaw: f32,
    pub m_pitch: f32,
    /// Zoomed in, the turn is also scaled by this times the zoomed FOV over
    /// the normal one (`zoom_sensitivity_ratio`; CS:S's default 1.2 and the
    /// rule as the community documents it, not measured here).
    pub zoom_ratio: f32,
    /// `m_filter`: 1 averages each frame's motion with the last frame's
    /// (CS:S's "smooth out mouse movement over 2 frames").
    pub filter: u8,
    /// `m_customaccel`: non-zero accelerates the motion, each axis's
    /// counts raised to `m_customaccel_exponent` (the options' Mouse
    /// acceleration check box sets 3; the curve is ours, see
    /// docs/plans/active/ui-parity.md).
    pub accel: u8,
    pub accel_exponent: f32,
}

impl Default for MouseSettings {
    fn default() -> Self {
        // CS:S defaults.
        Self {
            sensitivity: 3.0,
            m_yaw: 0.022,
            m_pitch: 0.022,
            zoom_ratio: 1.2,
            filter: 0,
            accel: 0,
            accel_exponent: 1.05,
        }
    }
}

impl MouseSettings {
    /// The counts mouse look turns by, from this frame's raw motion:
    /// filtered with the last frame's (`m_filter`, `last` keeps it), then
    /// accelerated (`m_customaccel`).
    pub fn counts(&self, raw: Vec2, last: &mut Vec2) -> Vec2 {
        let mut c = if self.filter != 0 { (raw + *last) / 2.0 } else { raw };
        *last = raw;
        if self.accel != 0 {
            let e = self.accel_exponent.clamp(0.5, 3.0);
            c = Vec2::new(c.x.signum() * c.x.abs().powf(e), c.y.signum() * c.y.abs().powf(e));
        }
        c
    }

    /// Radians to turn (yaw left, pitch up) for a mouse delta in counts
    /// (x right, y down).
    pub fn look_delta(&self, counts: Vec2) -> Vec2 {
        Vec2::new(
            -counts.x * self.sensitivity * self.m_yaw,
            -counts.y * self.sensitivity * self.m_pitch,
        ) * std::f32::consts::PI
            / 180.0
    }

    /// The turn multiplier at a zoomed field of view (degrees; the normal
    /// one is 90).
    pub fn zoom_scale(&self, zoomed_fov: Option<f32>) -> f32 {
        zoomed_fov.map_or(1.0, |fov| self.zoom_ratio * fov / 90.0)
    }

    /// The turn multiplier looking through `zoomed`: the game's own
    /// (`Zoomed::sensitivity`, Combat Arms' by zoom level) or else the
    /// FOV rule (`zoom_scale`).
    pub fn zoomed_scale(&self, zoomed: Option<&crate::weapon::Zoomed>) -> f32 {
        match zoomed {
            Some(z) => z.sensitivity.unwrap_or_else(|| self.zoom_scale(Some(z.fov))),
            None => 1.0,
        }
    }
}

pub struct LocalInputPlugin;

impl Plugin for LocalInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MouseSettings>()
            .init_resource::<super::binds::PolledBinds>()
            .init_resource::<WheelJump>()
            .init_resource::<PinOnGrab>()
            .init_resource::<FreeLook>()
            .init_resource::<super::binds::CapturedKeys>()
            .register_type::<MouseSettings>()
            .add_systems(PreUpdate, super::binds::release_captured.after(bevy::input::InputSystems))
            .add_systems(Update, ((grab_cursor, write_local_intent, chimera_net::net::predict::hold_view).chain(), drop_key))
            .add_systems(FixedPreUpdate, apply_wheel_jump);
        super::binds::add_owned_keys(app);
        mouse_cvars(app);
    }
}

pub fn mouse_cvars(app: &mut App) {
    use crate::console::{Console, resource_cvar};
    resource_cvar::<MouseSettings, f32>(
        app,
        "sensitivity",
        "Mouse sensitivity: degrees per count are this times m_yaw / m_pitch.",
        |m| &mut m.sensitivity,
    );
    resource_cvar::<MouseSettings, f32>(
        app,
        "m_yaw",
        "Mouse yaw factor (degrees per count at sensitivity 1).",
        |m| &mut m.m_yaw,
    );
    resource_cvar::<MouseSettings, f32>(
        app,
        "m_pitch",
        "Mouse pitch factor (degrees per count at sensitivity 1; negative inverts).",
        |m| &mut m.m_pitch,
    );
    resource_cvar::<MouseSettings, f32>(
        app,
        "zoom_sensitivity_ratio",
        "Extra mouse scale while zoomed (times the zoomed FOV / 90).",
        |m| &mut m.zoom_ratio,
    );
    resource_cvar::<MouseSettings, u8>(
        app,
        "m_filter",
        "1: smooth mouse look, averaging each frame's motion with the last frame's.",
        |m| &mut m.filter,
    );
    resource_cvar::<MouseSettings, u8>(
        app,
        "m_customaccel",
        "Mouse acceleration: 0 off; else each frame's counts raised to m_customaccel_exponent.",
        |m| &mut m.accel,
    );
    resource_cvar::<MouseSettings, f32>(
        app,
        "m_customaccel_exponent",
        "Mouse acceleration amount: 1 none, higher accelerates more (with m_customaccel).",
        |m| &mut m.accel_exponent,
    );
    let mut console = app.world_mut().resource_mut::<Console>();
    for name in [
        "sensitivity",
        "m_yaw",
        "m_pitch",
        "zoom_sensitivity_ratio",
        "m_filter",
        "m_customaccel",
        "m_customaccel_exponent",
    ] {
        console.archive(name);
    }
}

pub fn cursor_grabbed(cursor: &CursorOptions) -> bool {
    cursor.grab_mode != CursorGrabMode::None
}

pub fn release_cursor(cursor: &mut CursorOptions) {
    cursor.visible = true;
    cursor.grab_mode = CursorGrabMode::None;
}

/// Lock and hide the cursor so the mouse turns the view.
pub fn capture_cursor(cursor: &mut CursorOptions) {
    cursor.visible = false;
    cursor.grab_mode = CursorGrabMode::Locked;
}

/// Frames after a focus gain in which the grab is (re)made: the first
/// one the player is free to play in grabs; while grabbed the grab is
/// sent again every `REASSERT_EVERY` frames, since Windows can drop the
/// cursor clip after the focus came (a taskbar click ends after the
/// window took focus, a minimized window is restored after), while our
/// `CursorOptions` still says grabbed, so nothing would send it again.
const REGRAB_FRAMES: u32 = 30;
const REASSERT_EVERY: u32 = 10;

/// Whether a grab first puts the pointer inside the window when the
/// window doesn't know it to be there (`grab_cursor`). Windows locks the
/// cursor where it is (winit clips it to that one pixel): focus coming
/// from a taskbar click leaves the pointer on the taskbar, outside the
/// window, with no `CursorEntered`, so the locked pointer kept sending
/// the clicks and the wheel to the taskbar while raw motion still turned
/// the view. Moved to the window's middle first (bevy_winit applies the
/// position before the grab, in one frame), the lock holds it there.
/// On by default on Windows and macOS (which also locks in place); off
/// on Linux, where X11 confines (and so warps the pointer in) and Wayland
/// refuses to place an unlocked pointer.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PinOnGrab(pub bool);

impl Default for PinOnGrab {
    fn default() -> Self {
        Self(cfg!(any(windows, target_os = "macos")))
    }
}

/// Put the pointer at the window's middle unless the window knows it is
/// over it (`PinOnGrab`).
fn pin_inside(win: Option<Mut<Window>>) {
    if let Some(mut w) = win
        && w.cursor_position().is_none()
    {
        let middle = w.size() / 2.0;
        w.set_cursor_position(Some(middle));
    }
}

/// `grab_cursor`'s memory: frames of a focus gain's regrab left, and
/// frames in which a click is still the focusing one.
#[derive(Default)]
pub(super) struct Regrab {
    left: u32,
    focus_click: u32,
}

/// The grab (module doc of `grab_cursor`'s comments): lost with focus;
/// on every focus gain (alt-tab, a taskbar click, a click into the
/// window, a restore) grabbed and hidden again while playing with nothing
/// open; else a click grabs. Each grab, and each resend of it, first puts
/// a pointer the window doesn't know to be over it at its middle
/// (`PinOnGrab`). The click that grabs, or that came with the focus,
/// doesn't fire (`CapturedKeys::capture_button`, until let go).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn grab_cursor(
    mut window: Single<(&mut CursorOptions, Option<&mut Window>)>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut focus: MessageReader<bevy::window::WindowFocused>,
    (menu, app_menu): (Option<Res<super::front_end::MenuStatus>>, Option<Res<super::front_end::AppMenu>>),
    vgui: Option<Res<super::vgui::VguiOpen>>,
    (console, debug_ui, egui, chat): (
        Option<Res<super::console::ConsoleUi>>,
        Option<Res<super::debug_ui::DebugUi>>,
        Option<Res<bevy_inspector_egui::bevy_egui::input::EguiWantsInput>>,
        Option<Res<super::chat::ChatInput>>,
    ),
    mut captured: Option<ResMut<super::binds::CapturedKeys>>,
    pin: Option<Res<PinOnGrab>>,
    mut regrab: Local<Regrab>,
) {
    let (cursor, win) = &mut *window;
    let pin = pin.map_or_else(|| PinOnGrab::default().0, |p| p.0);
    // The game menu and the game-look buy and team menus own the mouse
    // while open (Esc opens and closes the game menu), and the click that
    // closes one isn't a click to grab. So do the console (clicks there
    // select and scroll its text), the debug UI, an app's own menu (the
    // party apps' Escape menu, the sandbox's screens) and any egui window
    // under the pointer (the inspector).
    let in_menu = menu.as_ref().is_some_and(|m| m.open || m.is_changed())
        || app_menu.as_ref().is_some_and(|m| m.open || m.is_changed())
        || vgui.as_ref().is_some_and(|v| v.any() || v.is_changed())
        || console.is_some_and(|c| c.open)
        || debug_ui.is_some_and(|d| d.open || d.is_changed())
        || egui.is_some_and(|e| e.wants_any_pointer_input());
    // Losing focus (alt-tab) ends the grab: Windows drops the cursor clip
    // then, and a grab we still believed in would keep turning the view
    // while the real cursor wanders off and clicks other windows. Coming
    // back, however it came (alt-tab, the taskbar, a click), grabs again
    // as CS:S, when playing with nothing open that owns the mouse or keys
    // (menu, console, chat, dialogs, debug UI): within `REGRAB_FRAMES`,
    // so a menu flag changing in the frame focus came doesn't lose it;
    // else a click grabs.
    let (mut lost, mut gained) = (false, false);
    for f in focus.read() {
        (lost, gained) = (!f.focused, f.focused);
    }
    let playing = menu.as_ref().is_none_or(|m| m.in_game && !m.loading);
    let typing = chat.is_some_and(|c| c.open.is_some());
    let mut grabbed_now = false;
    if lost {
        release_cursor(cursor);
        *regrab = Regrab::default();
        return;
    }
    if gained {
        *regrab = Regrab { left: REGRAB_FRAMES, focus_click: 2 };
    }
    // A window that says it lost focus since (no message read here) ends
    // the regrab.
    if win.as_ref().is_some_and(|w| !w.focused) && !gained {
        regrab.left = 0;
    }
    if keys.just_pressed(KeyCode::Escape) && menu.is_none() {
        release_cursor(cursor);
        regrab.left = 0;
    } else if regrab.left > 0 {
        regrab.left -= 1;
        if playing && !in_menu && !typing {
            if !cursor_grabbed(cursor) || cursor.visible {
                capture_cursor(cursor);
                grabbed_now = true;
            } else if regrab.left % REASSERT_EVERY == 0 {
                // Send the grab again (bevy_winit sends on a change).
                cursor.set_changed();
                if pin {
                    pin_inside(win.as_mut().map(|w| w.reborrow()));
                }
            }
        }
    } else if mouse.just_pressed(MouseButton::Left) && !cursor_grabbed(cursor) && !in_menu {
        capture_cursor(cursor);
        grabbed_now = true;
    }
    if grabbed_now && pin {
        pin_inside(win.as_mut().map(|w| w.reborrow()));
    }
    // The click that grabbed, or that focused the window, is not a shot.
    let focusing = regrab.focus_click > 0;
    regrab.focus_click = regrab.focus_click.saturating_sub(1);
    if (grabbed_now || focusing)
        && let Some(c) = captured.as_mut()
    {
        let held: Vec<MouseButton> = if grabbed_now { mouse.get_pressed().copied().collect() } else { mouse.get_just_pressed().copied().collect() };
        for b in held {
            if !c.holds_button(b) {
                c.capture_button(b);
            }
        }
    }
}

/// The drop key (`drop`, G) drops the held weapon while playing; the bug
/// report key (`bug`: F4 as in CS:S, and F9) saves a bug report any time.
fn drop_key(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions>,
    console: Option<ResMut<crate::console::Console>>,
    ui: Option<Res<super::console::ConsoleUi>>,
    chat: Option<Res<super::chat::ChatInput>>,
    menu: Option<Res<super::front_end::MenuStatus>>,
) {
    let Some(mut console) = console else { return };
    // Typing, or the game menu reading keys (rebinding them).
    if ui.is_some_and(|u| u.open) || chat.is_some_and(|c| c.open.is_some()) || menu.is_some_and(|m| m.open) {
        return;
    }
    let pressed = |c: &str| super::binds::just_pressed(&console.binds, &keys, &mouse, c);
    let (drop, report) = (pressed("drop"), pressed("bug") || pressed("bugreport"));
    if drop && cursor_grabbed(&cursor) {
        console.submit("drop");
    }
    // A bug report (screenshot, position, build, console) to send.
    if report {
        console.submit("bugreport");
    }
}

/// Jumping on the mouse wheel, as CS:S players bind it (`bind mwheeldown
/// +jump`; the default wheel binds are CS:S's `invnext`/`invprev`): each
/// notch bound to `+jump` is one press of jump. Presses are queued and played out one
/// fixed tick held, one tick released, so a jump that needs a fresh press
/// (Source) sees every notch, and spinning the wheel near a landing lands
/// a press on the right tick.
#[derive(Resource, Default)]
pub struct WheelJump {
    pending: u32,
    /// Jump held on the keyboard.
    key_held: bool,
    pressed_last_tick: bool,
}

/// Notches queued at most, so a long spin doesn't keep jumping after it.
const MAX_WHEEL_JUMPS: u32 = 4;

pub fn apply_wheel_jump(mut wheel: ResMut<WheelJump>, mut intent: Single<&mut Intent, With<LocalPlayer>>) {
    let pulse = wheel.pending > 0 && !wheel.pressed_last_tick;
    if pulse {
        wheel.pending -= 1;
    }
    wheel.pressed_last_tick = pulse;
    intent.jump = wheel.key_held || pulse;
}

/// Free look: while Left Alt (or `+freelook`) is held, the mouse turns
/// only the camera, by these offsets (radians) from the aim; movement and
/// aim keep their direction. Released, the view snaps back to the aim.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct FreeLook {
    pub yaw: f32,
    pub pitch: f32,
}

impl FreeLook {
    /// Turn by a look delta, keeping the camera's total pitch in range.
    fn turn(&mut self, delta: Vec2, aim_pitch: f32) {
        self.yaw = (self.yaw + delta.x + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        self.pitch = (aim_pitch + self.pitch + delta.y).clamp(-PITCH_LIMIT, PITCH_LIMIT) - aim_pitch;
    }
}

const PITCH_LIMIT: f32 = 89f32.to_radians();

#[allow(clippy::too_many_arguments)]
pub fn write_local_intent(
    mut intent: Single<&mut Intent, With<LocalPlayer>>,
    cursor: Single<&CursorOptions>,
    (keys, mouse, motion, scroll): (
        Res<ButtonInput<KeyCode>>,
        Res<ButtonInput<MouseButton>>,
        Res<AccumulatedMouseMotion>,
        Res<AccumulatedMouseScroll>,
    ),
    console: Option<Res<crate::console::Console>>,
    mut wheel: ResMut<WheelJump>,
    mouse_settings: Res<MouseSettings>,
    held: Option<Res<super::console::HeldActions>>,
    mut free: ResMut<FreeLook>,
    mut freecam: ResMut<super::view::FreeCam>,
    time: Res<Time>,
    panels: Option<Res<super::front_end::Panels>>,
    zoomed: Query<&crate::weapon::Zoomed, With<LocalPlayer>>,
    spectator: Option<Res<super::spectate::Spectator>>,
    weapon_menu: Option<Res<super::weapon_menu::WeaponMenu>>,
    (actions, identity, captured): (
        Option<Res<crate::core::ActionRegistry>>,
        Query<&crate::slots::GameIdentity, With<LocalPlayer>>,
        Option<Res<super::binds::CapturedKeys>>,
    ),
    mut last_motion: Local<Vec2>,
) {
    // The motion mouse look turns by (`m_filter`, `m_customaccel`).
    let motion_counts = mouse_settings.counts(motion.delta, &mut last_motion);
    // Every game key is a bind (`binds`): what the keys bound to an action
    // hold.
    let empty = std::collections::BTreeMap::new();
    // The shared binds under the identity's game's own (`Console::layers`).
    let identity = identity.iter().next().map(|i| i.0.as_str());
    let binds = console.as_ref().map_or(std::borrow::Cow::Borrowed(&empty), |c| c.effective_binds(identity));
    let binds = &*binds;
    let bound = |command: &str| super::binds::pressed_free(binds, &keys, &mouse, captured.as_deref(), command);
    let freelook = bound("+freelook") || held.as_ref().is_some_and(|h| h.freelook);
    if !freelook {
        free.set_if_neq(FreeLook::default());
    }
    if !cursor_grabbed(&cursor) {
        *wheel = WheelJump::default();
        // Not playing (menu, inspector): stop moving, keep looking where we
        // were. Console-held actions (`+attack` from a bind or script)
        // still apply, as in Source.
        let (yaw, pitch) = (intent.yaw, intent.pitch);
        **intent = Intent {
            yaw,
            pitch,
            // Slot commands (`slot2` from a script) still switch.
            select: weapon_menu.as_ref().and_then(|m| m.selected_slot()),
            ..default()
        };
        if let Some(h) = held {
            apply_held(&mut intent, &mut wheel, &h, actions.as_deref());
        }
        return;
    }

    // Spectating (dead): the keys and mouse drive the spectator camera.
    if spectator.is_some_and(|s| s.active()) {
        *wheel = WheelJump::default();
        let (yaw, pitch) = (intent.yaw, intent.pitch);
        **intent = Intent {
            yaw,
            pitch,
            ..default()
        };
        return;
    }
    let axis = |pos: &str, neg: &str| bound(pos) as i8 as f32 - bound(neg) as i8 as f32;
    // Flying the detached camera: the player stands still.
    if freecam.mode == 1 {
        let input = Vec3::new(
            axis("+moveright", "+moveleft"),
            axis("+forward", "+back"),
            axis("+jump", "+duck"),
        );
        let speed = if bound("+speed") { 12.0 } else { 4.0 };
        freecam.fly(input, mouse_settings.look_delta(motion_counts), speed, time.delta_secs());
        let (yaw, pitch) = (intent.yaw, intent.pitch);
        **intent = Intent {
            yaw,
            pitch,
            ..default()
        };
        return;
    }
    intent.move_axis = Vec2::new(axis("+moveright", "+moveleft"), axis("+forward", "+back"));
    intent.opposed = BVec2::new(
        bound("+moveright") && bound("+moveleft"),
        bound("+forward") && bound("+back"),
    );

    let zoom = mouse_settings.zoomed_scale(zoomed.iter().next());
    let turn = mouse_settings.look_delta(motion_counts) * zoom;
    if freelook {
        free.turn(turn, intent.pitch);
    } else {
        intent.yaw = (intent.yaw + turn.x).rem_euclid(std::f32::consts::TAU);
        intent.pitch = (intent.pitch + turn.y).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    intent.jump = bound("+jump");
    wheel.key_held = intent.jump;
    let notches = super::binds::wheel_notches(binds, &scroll, "+jump");
    wheel.pending = (wheel.pending + notches).min(MAX_WHEEL_JUMPS);
    intent.crouch = bound("+duck");
    // `+speed` is Source's walk key (CS:S runs, Shift walks): it sets
    // both, and each movement reads the one it has; `+walk` and `+sprint`
    // set one each (Combat Arms' C and Shift).
    intent.sprint = bound("+speed") || bound("+sprint");
    intent.walk = bound("+speed") || bound("+walk");
    // Attack that just picked from the weapon selection doesn't fire.
    intent.fire = bound("+attack") && !weapon_menu.as_ref().is_some_and(|m| m.swallow_fire);
    intent.secondary = bound("+attack2");
    intent.reload = bound("+reload");
    intent.last_weapon = bound("lastinv");
    intent.use_key = bound("+use");
    // The game's named actions: `+name` held.
    intent.actions = 0;
    if let Some(registry) = actions.as_deref() {
        for name in registry.names() {
            if bound(&format!("+{name}")) {
                intent.actions |= registry.bit(name);
            }
        }
    }
    // Number keys pick from the buy, team or radio menu while one is open;
    // with the weapon selection (`hud_fastswitch 0`) they work through it.
    intent.select = if let Some(m) = weapon_menu.as_ref().filter(|m| m.takes_slots || m.select.is_some()) {
        m.selected_slot()
    } else if panels.is_some_and(|p| p.number_keys_taken()) {
        None
    } else {
        (0..5u8).find(|i| bound(&format!("slot{}", i + 1)))
    };
    // Actions held from the console (`+duck` typed, `++attack`) add to the
    // keys.
    if let Some(h) = held {
        apply_held(&mut intent, &mut wheel, &h, actions.as_deref());
    }
}

fn apply_held(
    intent: &mut Intent,
    wheel: &mut WheelJump,
    h: &super::console::HeldActions,
    actions: Option<&crate::core::ActionRegistry>,
) {
    let add = |a: bool, b: bool| a as i8 as f32 - b as i8 as f32;
    intent.move_axis += Vec2::new(add(h.moveright, h.moveleft), add(h.forward, h.back));
    intent.move_axis = intent.move_axis.clamp(Vec2::NEG_ONE, Vec2::ONE);
    intent.opposed |= BVec2::new(h.moveright && h.moveleft, h.forward && h.back);
    intent.jump |= h.jump;
    wheel.key_held |= h.jump;
    intent.crouch |= h.duck;
    intent.walk |= h.speed || h.walk;
    intent.sprint |= h.speed || h.sprint;
    intent.fire |= h.attack;
    intent.secondary |= h.attack2;
    intent.reload |= h.reload;
    intent.use_key |= h.use_key;
    if let Some(registry) = actions {
        for name in &h.named {
            intent.actions |= registry.bit(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoomed_turns_scale_by_the_ratio_and_fov() {
        let m = MouseSettings::default();
        assert_eq!(m.zoom_scale(None), 1.0);
        // AWP first zoom (40 degrees): 1.2 * 40 / 90.
        assert!((m.zoom_scale(Some(40.0)) - 1.2 * 40.0 / 90.0).abs() < 1e-6);
        // A game's own scale wins over the FOV rule (Combat Arms: 1/4 at
        // its first zoom level).
        let mut z = crate::weapon::Zoomed {
            fov: 20.0,
            scope: true,
            sensitivity: None,
        };
        assert!((m.zoomed_scale(Some(&z)) - 1.2 * 20.0 / 90.0).abs() < 1e-6);
        z.sensitivity = Some(0.25);
        assert_eq!(m.zoomed_scale(Some(&z)), 0.25);
        assert_eq!(m.zoomed_scale(None), 1.0);
    }

    #[test]
    fn mouse_filter_and_acceleration_shape_the_motion() {
        let mut last = Vec2::ZERO;
        let plain = MouseSettings::default();
        assert_eq!(plain.counts(Vec2::new(10.0, -4.0), &mut last), Vec2::new(10.0, -4.0), "off: raw");
        // m_filter: the average of this frame's and the last.
        let filtered = MouseSettings { filter: 1, ..plain };
        let mut last = Vec2::ZERO;
        assert_eq!(filtered.counts(Vec2::new(10.0, 0.0), &mut last), Vec2::new(5.0, 0.0));
        assert_eq!(filtered.counts(Vec2::new(20.0, 0.0), &mut last), Vec2::new(15.0, 0.0));
        assert_eq!(filtered.counts(Vec2::ZERO, &mut last), Vec2::new(10.0, 0.0), "eases out");
        // m_customaccel: fast motion turns more than in proportion, signs kept.
        let accel = MouseSettings {
            accel: 3,
            accel_exponent: 1.2,
            ..plain
        };
        let mut last = Vec2::ZERO;
        let slow = accel.counts(Vec2::new(2.0, 0.0), &mut last).x;
        let fast = accel.counts(Vec2::new(-20.0, 0.0), &mut last).x;
        assert!(fast < 0.0 && -fast / slow > 10.0, "{slow} {fast}");
        let unaccelerated = MouseSettings {
            accel: 3,
            accel_exponent: 1.0,
            ..plain
        };
        assert_eq!(unaccelerated.counts(Vec2::new(7.0, 3.0), &mut last), Vec2::new(7.0, 3.0), "1.0: none");
    }

    #[test]
    fn free_look_turns_within_the_pitch_limit() {
        let mut f = FreeLook::default();
        f.turn(Vec2::new(0.5, 2.0), 0.5);
        assert_eq!(f.yaw, 0.5);
        // Aim pitch 0.5 plus the offset stays at the limit.
        assert!((0.5 + f.pitch - PITCH_LIMIT).abs() < 1e-6);
        // Yaw wraps to (-pi, pi].
        f.turn(Vec2::new(3.0, 0.0), 0.5);
        assert!(f.yaw < 0.0 && f.yaw > -std::f32::consts::PI);
    }

    #[test]
    fn mouse_look_matches_css() {
        let mut app = App::new();
        app.add_plugins(crate::console::ConsolePlugin)
            .init_resource::<MouseSettings>();
        mouse_cvars(&mut app);
        // CS:S defaults: 3 * 0.022 = 0.066 degrees per count.
        let d = app
            .world()
            .resource::<MouseSettings>()
            .look_delta(Vec2::new(1000.0, -1000.0));
        assert!((d.x.to_degrees() + 66.0).abs() < 1e-3, "{d}");
        assert!((d.y.to_degrees() - 66.0).abs() < 1e-3, "{d}");
        app.world_mut()
            .resource_mut::<crate::console::Console>()
            .submit("sensitivity 1.5; m_pitch -0.022");
        app.update();
        let m = *app.world().resource::<MouseSettings>();
        assert_eq!(m.sensitivity, 1.5);
        let d = m.look_delta(Vec2::new(100.0, 100.0));
        assert!((d.x.to_degrees() + 3.3).abs() < 1e-4, "{d}");
        assert!((d.y.to_degrees() - 3.3).abs() < 1e-4, "inverted: {d}");
        let console = app.world().resource::<crate::console::Console>();
        assert!(
            ["sensitivity", "m_yaw", "m_pitch"]
                .iter()
                .all(|n| console.cvar(n).unwrap().archive)
        );
    }

    #[test]
    fn losing_focus_releases_the_cursor() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<bevy::window::WindowFocused>()
            .add_systems(Update, grab_cursor);
        let window = app
            .world_mut()
            .spawn(CursorOptions {
                visible: false,
                grab_mode: CursorGrabMode::Locked,
                ..default()
            })
            .id();
        app.update();
        assert!(cursor_grabbed(app.world().get::<CursorOptions>(window).unwrap()));
        app.world_mut()
            .write_message(bevy::window::WindowFocused { window, focused: false });
        app.update();
        let cursor = app.world().get::<CursorOptions>(window).unwrap();
        assert!(!cursor_grabbed(cursor) && cursor.visible);
        // Alt-tab back in: grabbed and hidden again at once, no click.
        app.world_mut()
            .write_message(bevy::window::WindowFocused { window, focused: true });
        app.update();
        let cursor = app.world().get::<CursorOptions>(window).unwrap();
        assert!(cursor_grabbed(cursor) && !cursor.visible);
    }

    /// However focus comes back (a taskbar click, playtest 2026-11-18:
    /// the cursor stayed free and the player couldn't fire), the cursor
    /// is grabbed and hidden again while playing: also when a menu flag
    /// changed in the frame focus came, and again when the platform
    /// dropped the grab after it (bevy_winit puts `grab_mode` back when
    /// the grab fails); the grab is sent again a few frames on (Windows
    /// can drop the clip once the taskbar click ends). The click that
    /// came with the focus doesn't fire; the next one does.
    #[test]
    fn every_focus_gain_grabs_again_and_the_focusing_click_does_not_fire() {
        use bevy::ecs::{change_detection::DetectChanges, world::Ref};
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<super::super::binds::CapturedKeys>()
            .insert_resource(super::super::front_end::MenuStatus { in_game: true, ..default() })
            .add_message::<bevy::window::WindowFocused>()
            .add_systems(PreUpdate, super::super::binds::release_captured)
            .add_systems(Update, grab_cursor);
        let window = app.world_mut().spawn(CursorOptions { visible: false, grab_mode: CursorGrabMode::Locked, ..default() }).id();
        app.update();
        app.update();
        let focus = |app: &mut App, focused: bool| app.world_mut().write_message(bevy::window::WindowFocused { window, focused });
        let grabbed = |app: &App| {
            let c = app.world().get::<CursorOptions>(window).unwrap();
            cursor_grabbed(c) && !c.visible
        };
        let attack = |app: &App| {
            let binds = std::collections::BTreeMap::from([("mouse1".to_string(), "+attack".to_string())]);
            let w = app.world();
            super::super::binds::pressed_free(&binds, w.resource(), w.resource(), Some(w.resource()), "+attack")
        };
        focus(&mut app, false);
        app.update();
        assert!(!grabbed(&app), "focus lost: free");
        // Back by the taskbar, a menu flag touched in that frame.
        focus(&mut app, true);
        app.world_mut().resource_mut::<super::super::front_end::MenuStatus>().set_changed();
        app.update();
        app.update();
        assert!(grabbed(&app), "grabbed a frame later");
        // The platform dropped the grab: grabbed again.
        app.world_mut().get_mut::<CursorOptions>(window).unwrap().bypass_change_detection().grab_mode = CursorGrabMode::None;
        app.update();
        assert!(grabbed(&app), "grabbed again after the platform let go");
        // The grab is sent again within the regrab's frames.
        let changed = |app: &App| {
            let c: Ref<CursorOptions> = app.world().entity(window).get_ref::<CursorOptions>().unwrap();
            c.last_changed()
        };
        let before = changed(&app);
        for _ in 0..REGRAB_FRAMES {
            app.update();
        }
        assert!(changed(&app) != before, "the grab sent again");
        // A click into the window that focuses it: grabbed, no shot.
        focus(&mut app, false);
        app.update();
        focus(&mut app, true);
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        app.update();
        assert!(grabbed(&app));
        assert!(!attack(&app), "the focusing click doesn't fire");
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
        app.update();
        assert!(!attack(&app), "nor while held");
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Left);
        app.update();
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
        for _ in 0..3 {
            app.update();
        }
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        app.update();
        assert!(attack(&app), "the next click fires");
    }

    /// Focus back from a taskbar click on Windows (owner's report
    /// 2026-11-18: the view turned but clicks and the wheel did nothing):
    /// the window loses focus and the pointer leaves it; focus comes back
    /// with the pointer still on the taskbar, so no `CursorEntered` or
    /// `CursorMoved`. The grab puts the pointer at the window's middle
    /// before locking it there (`PinOnGrab`), so the OS sends the clicks
    /// and the wheel to the window: fire, alt-fire and the wheel's weapon
    /// switch all reach the game. `os_sends` stands in for the OS, which
    /// gives a button or wheel event to the window under the pointer.
    #[test]
    fn a_taskbar_refocus_pins_the_pointer_inside_so_clicks_and_the_wheel_count() {
        use bevy::input::{
            ButtonState,
            mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel},
        };
        use bevy::window::{CursorEntered, CursorLeft, WindowFocused};
        #[derive(Resource, Default)]
        struct Switched(i32);
        let mut app = App::new();
        app.add_plugins((bevy::input::InputPlugin, crate::console::ConsolePlugin))
            .init_resource::<Time>()
            .init_resource::<MouseSettings>()
            .init_resource::<WheelJump>()
            .init_resource::<FreeLook>()
            .init_resource::<super::super::view::FreeCam>()
            .init_resource::<super::super::binds::CapturedKeys>()
            .init_resource::<Switched>()
            .insert_resource(PinOnGrab(true))
            .insert_resource(super::super::front_end::MenuStatus { in_game: true, ..default() })
            .add_message::<WindowFocused>()
            .add_message::<CursorLeft>()
            .add_message::<CursorEntered>()
            .add_systems(PreUpdate, super::super::binds::release_captured.after(bevy::input::InputSystems))
            .add_systems(Update, ((grab_cursor, write_local_intent).chain(), super::super::console::run_binds));
        use crate::console::ConsoleAppExt;
        app.console_command("invnext", "test", |w, _| {
            w.resource_mut::<Switched>().0 += 1;
            Ok(None)
        });
        app.world_mut()
            .resource_mut::<crate::console::Console>()
            .submit("bind mouse1 +attack; bind mouse2 +attack2; bind mwheeldown invnext");
        let me = app.world_mut().spawn((Intent::default(), LocalPlayer)).id();
        let mut w = Window::default();
        let middle = w.size() / 2.0;
        w.set_cursor_position(Some(middle));
        let window = app
            .world_mut()
            .spawn((w, CursorOptions { visible: false, grab_mode: CursorGrabMode::Locked, ..default() }))
            .id();
        for _ in 0..3 {
            app.update();
        }
        // The OS: a button or wheel event goes to the window the pointer is
        // over (Windows routes them by position; a locked pointer stays put).
        let os_sends = |app: &mut App, button: Option<(MouseButton, ButtonState)>, wheel: f32| {
            if app.world().get::<Window>(window).unwrap().cursor_position().is_none() {
                return;
            }
            if let Some((button, state)) = button {
                app.world_mut().write_message(MouseButtonInput { button, state, window });
            }
            if wheel != 0.0 {
                app.world_mut().write_message(MouseWheel { unit: MouseScrollUnit::Line, x: 0.0, y: wheel, window, phase: bevy::input::touch::TouchPhase::Moved });
            }
        };
        let intent = |app: &App| app.world().get::<Intent>(me).unwrap().clone();
        let grabbed = |app: &App| {
            let c = app.world().get::<CursorOptions>(window).unwrap();
            cursor_grabbed(c) && !c.visible
        };
        // Away (alt-tab, the Windows key): focus lost, the pointer leaves
        // (bevy_winit forgets its position on `CursorLeft`).
        app.world_mut().write_message(WindowFocused { window, focused: false });
        app.update();
        app.world_mut().get_mut::<Window>(window).unwrap().set_physical_cursor_position(None);
        app.world_mut().write_message(CursorLeft { window });
        app.update();
        assert!(!grabbed(&app), "away: free");
        // The taskbar button: focus, the pointer still on the taskbar.
        app.world_mut().write_message(WindowFocused { window, focused: true });
        app.update();
        assert!(grabbed(&app), "grabbed again");
        assert_eq!(
            app.world().get::<Window>(window).unwrap().cursor_position(),
            Some(middle),
            "the pointer is put inside before the lock"
        );
        for _ in 0..3 {
            app.update();
        }
        let mut click = |app: &mut App, b: MouseButton| {
            os_sends(app, Some((b, ButtonState::Pressed)), 0.0);
            app.update();
            let i = intent(app);
            os_sends(app, Some((b, ButtonState::Released)), 0.0);
            app.update();
            i
        };
        assert!(click(&mut app, MouseButton::Left).fire, "fire");
        assert!(click(&mut app, MouseButton::Right).secondary, "alt-fire");
        os_sends(&mut app, None, -1.0);
        // The bind's command runs from the console's queue a frame on.
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Switched>().0, 1, "the wheel switches weapon");
        // Still inside on the grab's resends.
        for _ in 0..REGRAB_FRAMES {
            app.update();
        }
        assert!(click(&mut app, MouseButton::Left).fire, "fire after the regrab's frames");
    }

    #[test]
    fn regaining_focus_leaves_an_open_menu_or_chat_alone() {
        for (menu_open, chat_open) in [(true, false), (false, true)] {
            let mut app = App::new();
            let menu = super::super::front_end::MenuStatus {
                open: menu_open,
                in_game: true,
                ..default()
            };
            app.init_resource::<ButtonInput<MouseButton>>()
                .init_resource::<ButtonInput<KeyCode>>()
                .insert_resource(menu)
                .insert_resource(super::super::chat::ChatInput {
                    open: chat_open.then_some(false),
                    ..default()
                })
                .add_message::<bevy::window::WindowFocused>()
                .add_systems(Update, grab_cursor);
            let window = app.world_mut().spawn(CursorOptions::default()).id();
            app.update();
            app.update();
            app.world_mut()
                .write_message(bevy::window::WindowFocused { window, focused: true });
            app.update();
            let cursor = app.world().get::<CursorOptions>(window).unwrap();
            assert!(!cursor_grabbed(cursor), "menu {menu_open}, chat {chat_open}");
        }
    }

    /// A game's named action: `bind x +throw` while `x` is held sets its
    /// bit in the local intent, and the bit survives the network command.
    #[test]
    fn a_named_action_is_held_with_its_key() {
        use super::super::binds::BindsAppExt;
        let mut app = App::new();
        app.add_plugins(crate::console::ConsolePlugin)
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<AccumulatedMouseMotion>()
            .init_resource::<AccumulatedMouseScroll>()
            .init_resource::<Time>()
            .init_resource::<MouseSettings>()
            .init_resource::<WheelJump>()
            .init_resource::<FreeLook>()
            .init_resource::<super::super::view::FreeCam>()
            .add_systems(Update, write_local_intent);
        app.bound_action("throw").bound_action("ability");
        app.world_mut().spawn((Intent::default(), LocalPlayer));
        app.world_mut().spawn(CursorOptions {
            grab_mode: CursorGrabMode::Locked,
            ..default()
        });
        app.world_mut()
            .resource_mut::<crate::console::Console>()
            .submit("bind g +throw; bind h +jump");
        app.update();
        let intent = |app: &mut App| app.world_mut().query::<&Intent>().single(app.world()).unwrap().clone();
        assert_eq!(intent(&mut app).actions, 0);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyG);
        app.update();
        let i = intent(&mut app);
        let registry = app.world().resource::<crate::core::ActionRegistry>();
        // Sorted by name: ability is bit 0, throw bit 1.
        assert_eq!(i.actions, 0b10);
        assert!(i.action(registry, "throw") && !i.action(registry, "ability"));
        let mut over_the_wire = Intent::default();
        crate::net::NetCmd::of(1, &i).apply(&mut over_the_wire);
        assert!(over_the_wire.action(registry, "throw"));
        // Chimera's own binds are untouched.
        assert!(!i.jump);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::KeyG);
        app.update();
        assert_eq!(intent(&mut app).actions, 0);
    }

    /// A key a menu has taken (`CapturedKeys`) selects no weapon, in the
    /// frame it goes down nor while it is held after the menu closed; let
    /// go and pressed again, it selects.
    #[test]
    fn a_captured_number_key_selects_no_weapon() {
        let mut app = App::new();
        app.add_plugins(crate::console::ConsolePlugin)
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<AccumulatedMouseMotion>()
            .init_resource::<AccumulatedMouseScroll>()
            .init_resource::<Time>()
            .init_resource::<MouseSettings>()
            .init_resource::<WheelJump>()
            .init_resource::<FreeLook>()
            .init_resource::<super::super::view::FreeCam>()
            .init_resource::<super::super::binds::CapturedKeys>()
            .add_systems(PreUpdate, super::super::binds::release_captured)
            .add_systems(Update, write_local_intent);
        let me = app.world_mut().spawn((Intent::default(), LocalPlayer)).id();
        app.world_mut().spawn(CursorOptions {
            grab_mode: CursorGrabMode::Locked,
            ..default()
        });
        app.world_mut().resource_mut::<crate::console::Console>().submit("bind 4 slot4");
        let select = |app: &mut App| {
            app.update();
            app.world().get::<Intent>(me).unwrap().select
        };
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit4);
        app.world_mut().resource_mut::<super::super::binds::CapturedKeys>().capture(KeyCode::Digit4);
        assert_eq!(select(&mut app), None, "taken by a menu");
        assert_eq!(select(&mut app), None, "still held, the menu closed");
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::Digit4);
        assert_eq!(select(&mut app), None);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit4);
        assert_eq!(select(&mut app), Some(3), "pressed again: slot 4");
    }

    /// Shift is `+sprint` in the shared binds (Combat Arms'); a player of
    /// a game whose own bind says `+speed` walks on it.
    #[test]
    fn a_games_own_bind_beats_the_shared_one() {
        let mut app = App::new();
        app.add_plugins(crate::console::ConsolePlugin)
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<AccumulatedMouseMotion>()
            .init_resource::<AccumulatedMouseScroll>()
            .init_resource::<Time>()
            .init_resource::<MouseSettings>()
            .init_resource::<WheelJump>()
            .init_resource::<FreeLook>()
            .init_resource::<super::super::view::FreeCam>()
            .add_systems(Update, write_local_intent);
        let me = app.world_mut().spawn((Intent::default(), LocalPlayer)).id();
        app.world_mut().spawn(CursorOptions {
            grab_mode: CursorGrabMode::Locked,
            ..default()
        });
        let mut console = app.world_mut().resource_mut::<crate::console::Console>();
        console.submit("bind shift +sprint");
        console.submit("bindgame alpha shift +speed");
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftLeft);
        let walks = |app: &mut App| {
            app.update();
            let i = app.world().get::<Intent>(me).unwrap();
            (i.sprint, i.walk)
        };
        assert_eq!(walks(&mut app), (true, false), "no identity: Shift is the shared sprint key");
        app.world_mut().entity_mut(me).insert(crate::slots::GameIdentity("beta".into()));
        assert_eq!(walks(&mut app), (true, false), "a game without its own bind: the same");
        app.world_mut().entity_mut(me).insert(crate::slots::GameIdentity("alpha".into()));
        assert_eq!(walks(&mut app), (true, true), "alpha's own Shift is +speed, which walks");
        app.world_mut().resource_mut::<crate::console::Console>().submit("bindgame alpha shift");
        app.world_mut().resource_mut::<crate::console::Console>().submit("bindgame alpha shift \"\"");
        assert_eq!(walks(&mut app), (false, false), "an empty own bind hides the shared one");
    }
}
