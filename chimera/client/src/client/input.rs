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
            .init_resource::<FreeLook>()
            .init_resource::<super::binds::CapturedKeys>()
            .register_type::<MouseSettings>()
            .add_systems(PreUpdate, super::binds::release_captured.after(bevy::input::InputSystems))
            .add_systems(Update, ((grab_cursor, write_local_intent, chimera_net::net::predict::hold_view).chain(), drop_key))
            .add_systems(FixedPreUpdate, apply_wheel_jump);
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

#[allow(clippy::type_complexity)]
pub(super) fn grab_cursor(
    mut cursor: Single<&mut CursorOptions>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut focus: MessageReader<bevy::window::WindowFocused>,
    menu: Option<Res<super::front_end::MenuStatus>>,
    vgui: Option<Res<super::vgui::VguiOpen>>,
    (console, debug_ui, egui, chat): (
        Option<Res<super::console::ConsoleUi>>,
        Option<Res<super::debug_ui::DebugUi>>,
        Option<Res<bevy_inspector_egui::bevy_egui::input::EguiWantsInput>>,
        Option<Res<super::chat::ChatInput>>,
    ),
) {
    // The game menu and the game-look buy and team menus own the mouse
    // while open (Esc opens and closes the game menu), and the click that
    // closes one isn't a click to grab. So do the console (clicks there
    // select and scroll its text), the debug UI, and any egui window
    // under the pointer (the inspector).
    let in_menu = menu.as_ref().is_some_and(|m| m.open || m.is_changed())
        || vgui.as_ref().is_some_and(|v| v.any() || v.is_changed())
        || console.is_some_and(|c| c.open)
        || debug_ui.is_some_and(|d| d.open || d.is_changed())
        || egui.is_some_and(|e| e.wants_any_pointer_input());
    // Losing focus (alt-tab) ends the grab: Windows drops the cursor clip
    // then, and a grab we still believed in would keep turning the view
    // while the real cursor wanders off and clicks other windows. Coming
    // back (alt-tab) grabs again at once, as CS:S, when playing with
    // nothing open that owns the mouse or keys (menu, console, chat,
    // dialogs, debug UI); else a click grabs.
    let (mut lost, mut gained) = (false, false);
    for f in focus.read() {
        (lost, gained) = (!f.focused, f.focused);
    }
    let playing = menu.as_ref().is_none_or(|m| m.in_game && !m.loading);
    let typing = chat.is_some_and(|c| c.open.is_some());
    if lost {
        release_cursor(&mut cursor);
    } else if gained && playing && !in_menu && !typing {
        capture_cursor(&mut cursor);
    } else if keys.just_pressed(KeyCode::Escape) && menu.is_none() {
        release_cursor(&mut cursor);
    } else if mouse.just_pressed(MouseButton::Left) && !cursor_grabbed(&cursor) && !in_menu {
        capture_cursor(&mut cursor);
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
