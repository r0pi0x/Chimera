use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};

use bevy::{
    app::AppExit,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
        mouse::{MouseScrollUnit, MouseWheel},
    },
    log::{BoxedLayer, tracing_subscriber::Layer},
    prelude::*,
    window::CursorOptions,
};

use chimera_ui::clipboard::Clipboard;
use crate::client::{FirstPersonCamera, fonts::UiFonts, widgets::Interaction};
use super::*;
use crate::{
    console::{Console, ConsoleAppExt, Level, cfg_dir, parse, resource_cvar},
    core::{LocalPlayer, MovementState, Velocity},
    game_registry::{GameInfo, Games},
    map::PlaySound,
};

// ------------------------------------------------------------- overlays

#[derive(Resource)]
pub struct Overlays {
    pub showpos: u8,
    pub showfps: u8,
    pub snd_show: u8,
    /// `developer`: 1 shows console output top left for a few seconds.
    pub developer: u8,
    /// Notify lines and seconds left, and console lines already taken.
    notify: VecDeque<(String, f32)>,
    notify_seen: u64,
    pub(super) watch_text: String,
    frames: VecDeque<f32>,
}

impl Default for Overlays {
    fn default() -> Self {
        Self {
            showpos: 0,
            showfps: 0,
            snd_show: 0,
            developer: 0,
            notify: VecDeque::new(),
            notify_seen: 0,
            watch_text: String::new(),
            frames: VecDeque::new(),
        }
    }
}

/// Source's con_notifytime and notify line count.
pub(super) const NOTIFY_SECONDS: f32 = 8.0;
pub(super) const NOTIFY_LINES: usize = 8;

#[derive(Component)]
pub(super) struct NotifyText;

/// `developer 1`: new console lines show top left for a few seconds.
pub(super) fn draw_notify(
    time: Res<Time>,
    console: Res<Console>,
    ui: Res<ConsoleUi>,
    mut o: ResMut<Overlays>,
    mut text: Single<(&mut Text, &mut Visibility), With<NotifyText>>,
) {
    let new = console.printed.saturating_sub(o.notify_seen) as usize;
    o.notify_seen = console.printed;
    if o.developer > 0 {
        let start = console.output.len().saturating_sub(new);
        for line in &console.output[start..] {
            if line.level != crate::console::Level::Input {
                o.notify.push_back((line.text.clone(), NOTIFY_SECONDS));
            }
        }
    }
    let dt = time.delta_secs();
    o.notify.retain_mut(|(_, left)| {
        *left -= dt;
        *left > 0.0
    });
    while o.notify.len() > NOTIFY_LINES {
        o.notify.pop_front();
    }
    let (t, vis) = &mut *text;
    // Hidden while the console itself is open, or when off.
    let show = o.developer > 0 && !ui.open && !o.notify.is_empty();
    **vis = if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if show {
        t.0 = o.notify.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>().join("\n");
    }
}

pub(super) fn overlay_cvars(app: &mut App) {
    resource_cvar::<Overlays, u8>(
        app,
        "developer",
        "1: show console output at the top left for a few seconds.",
        |o| &mut o.developer,
    );
    resource_cvar::<Overlays, u8>(
        app,
        "cl_showpos",
        "1: show map, position, angles and velocity (CS:S units on CS:S maps).",
        |o| &mut o.showpos,
    );
    resource_cvar::<Overlays, u8>(
        app,
        "cl_showfps",
        "1: frame rate; 2: also frame time min/avg/max over the last second.",
        |o| &mut o.showfps,
    );
    resource_cvar::<Overlays, u8>(
        app,
        "snd_show",
        "1: mark where sounds play, with their names, for a few seconds; show the soundscape and room DSP preset.",
        |o| &mut o.snd_show,
    );
    // The room (audio::room) may be added after the console.
    app.init_resource::<crate::audio::room::RoomDsp>();
    resource_cvar::<crate::audio::room::RoomDsp, u8>(app, "dsp_off", "1: no room reverb or echo.", |r| &mut r.off);
    resource_cvar::<crate::audio::room::RoomDsp, f32>(
        app,
        "dsp_volume",
        "Level of the room reverb and echo (1 normal).",
        |r| &mut r.volume,
    );
    app.console_cvar(
        "con_timestamps",
        "1: time stamps on console lines.",
        "0",
        |w| {
            w.get_resource::<ConsoleUi>()
                .map(|u| if u.timestamps { "1" } else { "0" }.to_string())
        },
        |w, v| {
            w.resource_mut::<ConsoleUi>().timestamps = v.trim() != "0";
            Ok(())
        },
    );
    // CS:S's default is 0 (the console key does nothing until the
    // options' Keyboard > Advanced enables it); ours is 1, so the console
    // stays at hand for playtests.
    app.console_cvar(
        "con_enable",
        "1: the console key opens the console (CS:S's default is 0; ours 1).",
        "1",
        |w| {
            w.get_resource::<ConsoleUi>()
                .map(|u| if u.disabled { "0" } else { "1" }.to_string())
        },
        |w, v| {
            w.resource_mut::<ConsoleUi>().disabled = v.trim() == "0";
            Ok(())
        },
    );
    let mut console = app.world_mut().resource_mut::<Console>();
    console.archive("con_timestamps");
    console.archive("con_enable");
    // Set from the game menu's options too.
    console.archive("cl_showfps");
}

pub(super) fn draw_overlays(
    time: Res<Time>,
    mut o: ResMut<Overlays>,
    mut text: Single<&mut Text, With<OverlayText>>,
    player: Option<Single<(&Transform, &Velocity, &crate::core::Intent), With<LocalPlayer>>>,
    map: Option<Res<crate::map::ActiveMapLook>>,
    console: Res<Console>,
    sound: (
        Option<Res<crate::audio::soundscape::ScapeState>>,
        Option<Res<crate::audio::room::RoomDsp>>,
    ),
) {
    let _ = map;
    let dt = time.delta_secs();
    o.frames.push_back(dt);
    while o.frames.iter().sum::<f32>() > 1.0 && o.frames.len() > 1 {
        o.frames.pop_front();
    }
    let mut lines = Vec::new();
    if o.showfps > 0 {
        let n = o.frames.len().max(1) as f32;
        let avg = o.frames.iter().sum::<f32>() / n;
        lines.push(format!("{:.0} fps", 1.0 / avg.max(1e-6)));
        if o.showfps > 1 {
            let min = o.frames.iter().cloned().fold(f32::MAX, f32::min);
            let max = o.frames.iter().cloned().fold(0.0f32, f32::max);
            lines.push(format!(
                "frame {:.1} / {:.1} / {:.1} ms",
                min * 1e3,
                avg * 1e3,
                max * 1e3
            ));
        }
    }
    if o.showpos > 0
        && let Some(p) = &player
    {
        let (t, v, i) = **p;
        let units = |x: Vec3| Vec3::new(x.x, -x.z, x.y) / 0.0254;
        let pos = units(t.translation);
        let vel = units(v.0);
        lines.push(format!("pos: {:.2} {:.2} {:.2}", pos.x, pos.y, pos.z));
        lines.push(format!(
            "ang: {:.2} {:.2} 0.00",
            -i.pitch.to_degrees(),
            (i.yaw.to_degrees() + 90.0).rem_euclid(360.0)
        ));
        lines.push(format!("vel: {:.2} (xy {:.2})", vel.length(), vel.truncate().length()));
    }
    if o.snd_show > 0
        && let (Some(scape), Some(room)) = &sound
    {
        lines.push(crate::audio::soundscape::readout(scape, room));
    }
    if !o.watch_text.is_empty() {
        lines.push(o.watch_text.clone());
    }
    let _ = console;
    // Only a change marks the text for layout again (`ui_write`).
    crate::client::ui_write::set_text(&mut text, &lines.join("\n"));
}

/// Sounds marked on screen for `snd_show`.
#[derive(Resource, Default)]
pub(super) struct SoundMarks(Vec<(String, Vec3, f32, Entity)>);

#[derive(Component)]
pub(super) struct SoundMark;

pub(super) const MARK_SECONDS: f32 = 3.0;

pub(super) fn record_sounds(
    mut sounds: MessageReader<PlaySound>,
    o: Res<Overlays>,
    mut marks: ResMut<SoundMarks>,
    fonts: Res<UiFonts>,
    mut commands: Commands,
) {
    for s in sounds.read() {
        if o.snd_show == 0 {
            continue;
        }
        let Some(at) = s.at else { continue };
        let e = commands
            .spawn((
                SoundMark,
                Text::new(s.entry.clone()),
                overlay_font(&fonts, 13.0),
                TextColor(Color::srgb(0.5, 1.0, 0.6)),
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
            ))
            .id();
        marks.0.push((s.entry.clone(), at, MARK_SECONDS, e));
    }
}

pub(super) fn draw_sound_marks(
    time: Res<Time>,
    mut marks: ResMut<SoundMarks>,
    camera: Query<(&Camera, &GlobalTransform), With<FirstPersonCamera>>,
    mut nodes: Query<(&mut Node, &mut TextColor, &mut Visibility), With<SoundMark>>,
    mut commands: Commands,
) {
    let cam = camera.iter().next();
    marks.0.retain_mut(|(_, at, left, e)| {
        *left -= time.delta_secs();
        if *left <= 0.0 {
            commands.entity(*e).despawn();
            return false;
        }
        if let (Some((c, t)), Ok((mut node, mut color, mut vis))) = (cam, nodes.get_mut(*e)) {
            match c.world_to_viewport(t, *at) {
                Ok(p) => {
                    node.left = px(p.x);
                    node.top = px(p.y);
                    *vis = Visibility::Visible;
                    color.0 = color.0.with_alpha(*left / MARK_SECONDS);
                }
                Err(_) => *vis = Visibility::Hidden,
            }
        }
        true
    });
}

// ------------------------------------------------------------- log

pub(super) static LOG: OnceLock<Arc<Mutex<Vec<(Level, String)>>>> = OnceLock::new();

/// A tracing layer that copies warnings and errors into the console.
pub fn log_layer(_: &mut App) -> Option<BoxedLayer> {
    struct ToConsole(Arc<Mutex<Vec<(Level, String)>>>);
    impl<S: bevy::log::tracing::Subscriber> Layer<S> for ToConsole {
        fn on_event(
            &self,
            event: &bevy::log::tracing::Event<'_>,
            _: bevy::log::tracing_subscriber::layer::Context<'_, S>,
        ) {
            let level = match *event.metadata().level() {
                bevy::log::tracing::Level::ERROR => Level::Error,
                bevy::log::tracing::Level::WARN => Level::Warn,
                bevy::log::tracing::Level::INFO if event.metadata().target().starts_with("mashup") => Level::Info,
                _ => return,
            };
            struct Msg(String);
            impl bevy::log::tracing::field::Visit for Msg {
                fn record_debug(&mut self, f: &bevy::log::tracing::field::Field, v: &dyn std::fmt::Debug) {
                    if f.name() == "message" {
                        self.0 = format!("{v:?}");
                    }
                }
            }
            let mut m = Msg(String::new());
            event.record(&mut m);
            if let Ok(mut q) = self.0.lock() {
                q.push((level, m.0));
            }
        }
    }
    let queue = LOG.get_or_init(Default::default).clone();
    // The session's log too (`diagnostics`: INFO and above, every target).
    let layer = ToConsole(queue).and_then(chimera_core::diagnostics::SessionLayer);
    // Each frame's log lines, for the frame inspector's spike records.
    let layer = layer.and_then(chimera_core::metrics::frame_events::LogEvents);
    // A profile build also times every span for the hitch log
    // (`frame_metrics::span_times`).
    #[cfg(feature = "profile")]
    let layer = layer.and_then(crate::client::frame_metrics::span_times::SpanTimes);
    // A `trace` build records short windows of spans (`trace_capture`).
    #[cfg(feature = "trace")]
    let layer = layer.and_then(crate::client::trace_capture::CaptureLayer);
    Some(Box::new(layer))
}

pub(super) fn drain_log(mut console: ResMut<Console>) {
    let Some(q) = LOG.get() else { return };
    let lines: Vec<_> = q.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
    for (level, text) in lines {
        console.print(level, text);
    }
}

