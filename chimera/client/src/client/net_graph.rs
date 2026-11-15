//! `net_graph`: Source's network readout, bottom right, in our Mono at
//! GameUI's `DefaultFixedOutline` size (tall 10, outlined), as the
//! Valve Developer Community's pages and the community's guides describe
//! it. `net_graph 1` is the text:
//!
//! ```text
//! fps:  299  ping: 31 ms  lerp 100.0 ms
//! in :  117  4.56 k/s  20.0/s
//! out:   67  4.39 k/s  64.0/s
//! loss:   0  choke:   0
//! sv: 64.0 fps +- 0.3 ms  var: 0.4 ms
//! tick: 64.0  up: 20.0/s  cmd: 64.0/s
//! ```
//!
//! fps over the last second; ping: renet's round trip; lerp: how far in
//! the past others are drawn (`net::interp`: `max(cl_interp, cl_interp_ratio
//! × update interval)`), orange when under two update intervals (a lost
//! update leaves nothing to draw towards), yellow when the server's frame
//! takes longer than it; `in`/`out`: the packets' mean size (bytes), k/s
//! (1000 bytes) and packets a second at the transport (`net::stats::
//! Traffic`); loss: renet's packet loss, %; choke: the share of updates
//! due that the server's `rate` held back for us, %; sv: the server's frame
//! rate and its frame time's deviation; var: our frame time's deviation;
//! tick: the server's tick rate; up: updates the server sent us a second;
//! cmd: command packets we sent a second (`net::stats::ServerStats`, once
//! a second). A listen server's host (and a game of one) shows its own
//! numbers as Source's does: ping 0, no traffic or loss, updates and
//! commands at the tick rate, its frames as `sv`, lerp one tick (the host
//! draws between its last two ticks, `map::interp`).
//!
//! `net_graph 2` adds the graph under the text, one column a frame,
//! newest on the right: the latency band at the bottom (green, yellow while
//! the server chokes us, red while renet reports loss) and the incoming
//! bytes of that frame above it (blue); `net_graph 3` stacks the outgoing
//! bytes on them (orange). `net_graphpos` (0 left, 1 right, 2 centre, more:
//! that many pixels from the left), `net_graphheight` (the graph's height,
//! pixels, the text sits above it at any level) and
//! `net_graphproportionalfont` (1: the font scales with the window as a
//! proportional scheme font does, by height / 480; 0: its 10 pixels).
//!
//! Known shortcuts (docs/tech-debt.md): the layout (the lines' order and
//! words, the colours' thresholds, the graph's bands, scales and colours,
//! `net_graphheight`'s meaning) is from public descriptions, not compared
//! with CS:S's own net graph; the graph's columns are frames, not packets
//! (Source draws one per packet, its loss and choke per packet; ours only
//! know the current shares); Source's `net_graph 3` per-message-type
//! packet bands, `net_graph 4`, `net_scale`, `net_graphmsecs`,
//! `net_graphtext` and `net_graphsolid` aren't there.

use std::collections::VecDeque;

use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::PrimaryWindow,
};

use super::fonts::UiFonts;
use crate::{
    console::resource_cvar,
    core::NetRole,
    net::{
        interp::InterpSettings,
        predict::{NetGraph, RateSettings},
        stats::{Recent, ServerFrames, Traffic},
    },
};

pub struct NetGraphPlugin;

impl Plugin for NetGraphPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetGraphView>()
            .init_resource::<Traffic>()
            .init_resource::<ServerFrames>()
            .add_systems(Startup, spawn)
            .add_systems(Update, (sample, draw).chain());
        cvars(app);
    }
}

/// Columns of the graph (frames kept).
pub const COLUMNS: usize = 256;
/// The latency band's full height, ms; bytes a pixel in the packet bands.
const LATENCY_FULL_MS: f32 = 250.0;
const BYTES_PER_PIXEL: f32 = 16.0;
const MAX_HEIGHT: f32 = 512.0;
/// The text's line, pixels at 480 lines (GameUI's DefaultFixedOutline:
/// tall 10).
const NET_GRAPH_LINE: f32 = 10.0;
/// Our Mono's line in ems (OS/2 Windows ascent 820 + descent 200).
const MONO_LINE_PER_EM: f32 = 1.02;

/// `net_graph` and its settings, and the graph's frames.
#[derive(Resource)]
pub struct NetGraphView {
    pub level: u8,
    pub pos: f32,
    pub height: f32,
    pub proportional: u8,
    /// Frame times over the last second (s).
    frames: Recent,
    columns: VecDeque<Column>,
    /// The transport's totals at the last frame (bytes in, out).
    totals: (u64, u64),
}

impl Default for NetGraphView {
    fn default() -> Self {
        Self {
            level: 0,
            pos: 1.0,
            height: 64.0,
            proportional: 1,
            frames: Recent::default(),
            columns: VecDeque::new(),
            totals: (0, 0),
        }
    }
}

/// One frame of the graph.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Column {
    pub ping_ms: f32,
    pub in_bytes: u32,
    pub out_bytes: u32,
    pub loss: bool,
    pub choke: bool,
}

fn cvars(app: &mut App) {
    resource_cvar::<NetGraphView, u8>(
        app,
        "net_graph",
        "1: fps, ping, lerp, traffic in and out, loss, choke, the server's frame rate, tick, update and command \
         rates; 2: also the graph of latency and incoming bytes; 3: and outgoing bytes.",
        |v| &mut v.level,
    );
    resource_cvar::<NetGraphView, f32>(
        app,
        "net_graphpos",
        "Where net_graph sits: 0 left, 1 right, 2 centre, more: pixels from the left.",
        |v| &mut v.pos,
    );
    resource_cvar::<NetGraphView, f32>(
        app,
        "net_graphheight",
        "net_graph's graph height in pixels (its text sits above).",
        |v| &mut v.height,
    );
    resource_cvar::<NetGraphView, u8>(
        app,
        "net_graphproportionalfont",
        "1: net_graph's font scales with the window's height; 0: fixed.",
        |v| &mut v.proportional,
    );
    if let Some(mut console) = app.world_mut().get_resource_mut::<crate::console::Console>() {
        for name in ["net_graph", "net_graphpos", "net_graphheight", "net_graphproportionalfont"] {
            console.archive(name);
        }
    }
}

/// The numbers `net_graph 1` shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Readout {
    pub fps: f64,
    /// Our frame time's deviation, ms.
    pub var_ms: f64,
    pub ping_ms: f64,
    pub lerp_ms: f64,
    /// The update interval as served, ms (the lerp's colour).
    pub update_ms: f64,
    /// Packets' mean size (bytes), k/s, packets a second.
    pub in_size: usize,
    pub in_kbs: f64,
    pub in_rate: f64,
    pub out_size: usize,
    pub out_kbs: f64,
    pub out_rate: f64,
    /// Percent.
    pub loss: f64,
    pub choke: f64,
    /// The server's frame rate, its frame time's deviation (ms), tick
    /// rate; None until it says.
    pub sv: Option<(f64, f64, f64)>,
    pub up: f64,
    pub cmd: f64,
}

/// The lerp's colour: orange under two update intervals, yellow when the
/// server's frames take longer than it, else the text's.
pub fn lerp_color(r: &Readout) -> Option<Color> {
    let sv_frame_ms = r.sv.map_or(0.0, |(fps, ..)| if fps > 0.0 { 1000.0 / fps } else { 0.0 });
    if r.update_ms > 0.0 && r.lerp_ms + 1e-6 < 2.0 * r.update_ms {
        Some(Color::srgb(1.0, 0.6, 0.15))
    } else if sv_frame_ms > r.lerp_ms {
        Some(Color::srgb(1.0, 1.0, 0.2))
    } else {
        None
    }
}

/// The text, as three runs: before the lerp, the lerp (coloured by
/// `lerp_color`), after it.
pub fn text_runs(r: &Readout) -> [String; 3] {
    let first = format!("fps: {:4.0}  ping: {:.0} ms  ", r.fps, r.ping_ms);
    let lerp = format!("lerp {:.1} ms", r.lerp_ms);
    let sv = match r.sv {
        Some((fps, dev, _)) => format!("sv: {fps:.1} fps +- {dev:.1} ms  var: {:.1} ms", r.var_ms),
        None => format!("sv: -  var: {:.1} ms", r.var_ms),
    };
    let tick = r.sv.map_or("-".to_string(), |(.., hz)| format!("{hz:.1}"));
    let rest = format!(
        "\nin : {:4}  {:.2} k/s  {:.1}/s\nout: {:4}  {:.2} k/s  {:.1}/s\nloss: {:3.0}  choke: {:3.0}\n{sv}\ntick: {tick}  up: {:.1}/s  cmd: {:.1}/s",
        r.in_size, r.in_kbs, r.in_rate, r.out_size, r.out_kbs, r.out_rate, r.loss, r.choke, r.up, r.cmd
    );
    [first, lerp, rest]
}

#[derive(Component)]
struct GraphRoot;

#[derive(Component)]
struct GraphText;

/// The text's runs (0 before the lerp, 1 the lerp, 2 after).
#[derive(Component)]
struct GraphRun(usize);

#[derive(Component)]
struct GraphImage;

fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_fill(
        Extent3d {
            width: COLUMNS as u32,
            height: 64,
            ..default()
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    let image = images.add(image);
    commands
        .spawn((
            Name::new("net_graph"),
            GraphRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(0.0),
                left: px(0.0),
                width: percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            GlobalZIndex(50),
            Visibility::Hidden,
        ))
        .with_children(|c| {
            c.spawn((
                GraphText,
                Text::default(),
                TextColor(Color::srgb(0.95, 0.95, 0.95)),
                TextShadow {
                    offset: Vec2::splat(1.0),
                    color: Color::BLACK,
                },
                Node {
                    padding: UiRect::horizontal(px(8.0)),
                    ..default()
                },
            ))
            .with_children(|t| {
                for i in 0..3 {
                    t.spawn((GraphRun(i), TextSpan::default()));
                }
            });
            c.spawn((
                GraphImage,
                ImageNode::new(image),
                Node {
                    width: px(COLUMNS as f32),
                    height: px(64.0),
                    margin: UiRect::horizontal(px(8.0)),
                    ..default()
                },
            ));
        });
}

/// This frame's column and frame time.
fn sample(
    time: Res<Time<Real>>,
    mut view: ResMut<NetGraphView>,
    traffic: Res<Traffic>,
    graph: Option<Res<NetGraph>>,
    role: Option<Res<NetRole>>,
) {
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs_f64();
    if dt > 0.0 {
        view.frames.push(now, dt);
    }
    let client = role.is_some_and(|r| *r == NetRole::Client);
    let totals = (traffic.incoming.bytes, traffic.outgoing.bytes);
    let (was_in, was_out) = std::mem::replace(&mut view.totals, totals);
    let column = match graph.filter(|_| client) {
        Some(g) => Column {
            ping_ms: g.ping_ms as f32,
            in_bytes: totals.0.saturating_sub(was_in) as u32,
            out_bytes: totals.1.saturating_sub(was_out) as u32,
            loss: g.loss > 0.0,
            choke: g.server.is_some_and(|s| s.choke > 0.0),
        },
        None => Column::default(),
    };
    view.columns.push_back(column);
    while view.columns.len() > COLUMNS {
        view.columns.pop_front();
    }
}

/// The readout from this process's numbers: a network client's from its
/// connection, else (a listen server's host, a game of one) local ones.
#[allow(clippy::too_many_arguments)]
pub fn readout(
    view: &mut NetGraphView,
    now: f64,
    role: NetRole,
    graph: Option<&NetGraph>,
    server_frames: &mut ServerFrames,
    tick_hz: f64,
    interp: Option<&InterpSettings>,
    rates: Option<&RateSettings>,
) -> Readout {
    let (fps, _) = view.frames.per_second(now);
    let (_, dev) = view.frames.mean_dev();
    let mut r = Readout {
        fps,
        var_ms: dev * 1e3,
        ..default()
    };
    match graph.filter(|_| role == NetRole::Client) {
        Some(g) => {
            r.ping_ms = g.ping_ms;
            r.update_ms = if g.update_ms > 0.0 {
                g.update_ms
            } else {
                rates.map_or(0.0, |s| 1000.0 / s.updaterate.max(1.0) as f64)
            };
            r.lerp_ms = if g.interp_ms > 0.0 {
                g.interp_ms
            } else {
                interp.map_or(0.0, |i| (i.interp as f64 * 1e3).max(i.interp_ratio as f64 * r.update_ms))
            };
            r.in_size = g.incoming.mean.round() as usize;
            r.in_kbs = g.incoming.bytes_per_sec / 1000.0;
            r.in_rate = g.incoming.packets_per_sec;
            r.out_size = g.outgoing.mean.round() as usize;
            r.out_kbs = g.outgoing.bytes_per_sec / 1000.0;
            r.out_rate = g.outgoing.packets_per_sec;
            r.loss = g.loss * 100.0;
            r.cmd = g.cmd_rate;
            if let Some(s) = g.server {
                r.choke = s.choke as f64 * 100.0;
                r.sv = Some((s.fps as f64, s.frame_dev_ms as f64, s.tick_hz as f64));
                r.up = s.updates as f64;
            }
        }
        None => {
            let (sv_fps, sv_dev) = server_frames.rate_and_dev(now);
            r.sv = Some((sv_fps as f64, sv_dev as f64, tick_hz));
            r.lerp_ms = 1000.0 / tick_hz.max(1.0);
            r.up = tick_hz;
            r.cmd = tick_hz;
        }
    }
    r
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    time: Res<Time<Real>>,
    fixed: Res<Time<Fixed>>,
    mut view: ResMut<NetGraphView>,
    net: (
        Option<Res<NetGraph>>,
        Option<Res<NetRole>>,
        ResMut<ServerFrames>,
        Option<Res<InterpSettings>>,
        Option<Res<RateSettings>>,
    ),
    fonts: Res<UiFonts>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    mut root: Single<(&mut Node, &mut Visibility), (With<GraphRoot>, Without<GraphImage>)>,
    mut text_font: Single<&mut TextFont, (With<GraphText>, Without<GraphRun>)>,
    mut runs: Query<(&GraphRun, &mut TextSpan, &mut TextColor, &mut TextFont), Without<GraphText>>,
    mut image: Single<(&ImageNode, &mut Node, &mut Visibility), (With<GraphImage>, Without<GraphRoot>)>,
    mut images: ResMut<Assets<Image>>,
) {
    let (graph, role, mut server_frames, interp, rates) = net;
    let (root_node, root_vis) = &mut *root;
    let show = view.level > 0;
    root_vis.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    if !show {
        return;
    }
    let now = time.elapsed_secs_f64();
    let role = role.map_or(NetRole::Standalone, |r| *r);
    let r = readout(
        &mut view,
        now,
        role,
        graph.as_deref(),
        &mut server_frames,
        1.0 / fixed.timestep().as_secs_f64().max(1e-6),
        interp.as_deref(),
        rates.as_deref(),
    );
    // Where: left, right, centre or pixels from the left.
    let align = match view.pos.round() as i32 {
        0 => AlignItems::FlexStart,
        2 => AlignItems::Center,
        p if p >= 3 => AlignItems::FlexStart,
        _ => AlignItems::FlexEnd,
    };
    root_node.align_items = align;
    root_node.left = px(if view.pos >= 3.0 { view.pos } else { 0.0 });
    // The font: our Mono at DefaultFixedOutline's size (tall 10), scaled
    // by the window's height over 480 when proportional.
    let height = window.as_ref().map_or(480.0, |w| w.height());
    let scale = if view.proportional != 0 { (height / 480.0).max(1.0) } else { 1.0 };
    let font = fonts.mono(NET_GRAPH_LINE * scale / MONO_LINE_PER_EM);
    if **text_font != font {
        **text_font = font.clone();
    }
    let parts = text_runs(&r);
    let lerp = lerp_color(&r);
    for (run, mut span, mut color, mut f) in &mut runs {
        let s = &parts[run.0.min(2)];
        if span.0 != *s {
            span.0.clone_from(s);
        }
        let want = if run.0 == 1 { lerp } else { None }.unwrap_or(Color::srgb(0.95, 0.95, 0.95));
        color.set_if_neq(TextColor(want));
        if *f != font {
            *f = font.clone();
        }
    }
    // The graph (net_graph 2 and up), always taking its height.
    let h = view.height.clamp(0.0, MAX_HEIGHT).round() as u32;
    let (node, node_image, vis) = &mut *image;
    let (image_node, image_layout) = (node, node_image);
    image_layout.height = px(h as f32);
    vis.set_if_neq(if view.level >= 2 && h > 0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    if view.level < 2 || h == 0 {
        return;
    }
    if let Some(mut img) = images.get_mut(&image_node.image) {
        let pixels = draw_columns(&view.columns, h as usize, view.level >= 3);
        if img.texture_descriptor.size.height != h {
            img.resize(Extent3d {
                width: COLUMNS as u32,
                height: h,
                ..default()
            });
        }
        img.data = Some(pixels);
    }
}

const LATENCY: [u8; 4] = [60, 220, 60, 255];
const LATENCY_CHOKE: [u8; 4] = [240, 220, 40, 255];
const LATENCY_LOSS: [u8; 4] = [230, 40, 40, 255];
const IN_BYTES: [u8; 4] = [90, 150, 255, 220];
const OUT_BYTES: [u8; 4] = [255, 150, 40, 220];

/// The graph's pixels (RGBA, `COLUMNS` wide, `height` tall, top row first):
/// per column the latency from the bottom (to half the height), then the
/// incoming bytes above it and, with `outgoing`, the outgoing bytes on top.
pub fn draw_columns(columns: &VecDeque<Column>, height: usize, outgoing: bool) -> Vec<u8> {
    let mut px = vec![0u8; COLUMNS * height * 4];
    let half = height as f32 / 2.0;
    let start = COLUMNS.saturating_sub(columns.len());
    for (i, c) in columns.iter().enumerate() {
        let x = start + i;
        let mut y = 0usize;
        let mut fill = |n: usize, color: [u8; 4], y: &mut usize| {
            for _ in 0..n {
                if *y >= height {
                    return;
                }
                let row = height - 1 - *y;
                let at = (row * COLUMNS + x) * 4;
                px[at..at + 4].copy_from_slice(&color);
                *y += 1;
            }
        };
        let latency = ((c.ping_ms / LATENCY_FULL_MS).min(1.0) * half).round() as usize;
        let color = if c.loss {
            LATENCY_LOSS
        } else if c.choke {
            LATENCY_CHOKE
        } else {
            LATENCY
        };
        // At least a pixel: a frame of a connection shows.
        fill(latency.max(usize::from(c.ping_ms > 0.0)), color, &mut y);
        fill((c.in_bytes as f32 / BYTES_PER_PIXEL).ceil() as usize, IN_BYTES, &mut y);
        if outgoing {
            fill((c.out_bytes as f32 / BYTES_PER_PIXEL).ceil() as usize, OUT_BYTES, &mut y);
        }
    }
    px
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Readout {
        Readout {
            fps: 299.4,
            var_ms: 0.42,
            ping_ms: 31.2,
            lerp_ms: 100.0,
            update_ms: 50.0,
            in_size: 117,
            in_kbs: 4.561,
            in_rate: 20.0,
            out_size: 67,
            out_kbs: 4.39,
            out_rate: 64.0,
            loss: 0.0,
            choke: 0.0,
            sv: Some((64.0, 0.31, 64.0)),
            up: 20.0,
            cmd: 64.0,
        }
    }

    #[test]
    fn text_lines() {
        let [a, b, c] = text_runs(&sample());
        let all = format!("{a}{b}{c}");
        assert_eq!(
            all,
            "fps:  299  ping: 31 ms  lerp 100.0 ms\n\
             in :  117  4.56 k/s  20.0/s\n\
             out:   67  4.39 k/s  64.0/s\n\
             loss:   0  choke:   0\n\
             sv: 64.0 fps +- 0.3 ms  var: 0.4 ms\n\
             tick: 64.0  up: 20.0/s  cmd: 64.0/s"
        );
        // Before the server has said anything.
        let r = Readout { sv: None, ..sample() };
        assert!(text_runs(&r)[2].contains("sv: -  var: 0.4 ms\ntick: -"));
    }

    #[test]
    fn lerp_colours() {
        assert_eq!(lerp_color(&sample()), None);
        // cl_interp_ratio 1: one lost update and nothing to draw towards.
        let short = Readout { lerp_ms: 50.0, ..sample() };
        assert_eq!(lerp_color(&short), Some(Color::srgb(1.0, 0.6, 0.15)));
        // The server's frames are slower than the lerp.
        let slow = Readout {
            sv: Some((5.0, 1.0, 64.0)),
            ..sample()
        };
        assert_eq!(lerp_color(&slow), Some(Color::srgb(1.0, 1.0, 0.2)));
    }

    #[test]
    fn a_hosts_readout_is_local() {
        let mut view = NetGraphView::default();
        let mut frames = ServerFrames::default();
        for i in 0..64 {
            view.frames.push(i as f64 / 64.0, 1.0 / 64.0);
            frames.0.push(i as f64 / 64.0, 1.0 / 64.0);
        }
        // A client's numbers on hand don't count on a server.
        let mut g = NetGraph::default();
        g.ping_ms = 80.0;
        let r = readout(&mut view, 63.0 / 64.0, NetRole::Server, Some(&g), &mut frames, 64.0, None, None);
        assert_eq!(r.ping_ms, 0.0);
        assert_eq!((r.in_size, r.out_size, r.loss, r.choke), (0, 0, 0.0, 0.0));
        assert_eq!((r.up, r.cmd), (64.0, 64.0));
        let (fps, dev, hz) = r.sv.unwrap();
        assert!((fps - 64.0).abs() < 1.5 && dev < 1e-6 && hz == 64.0, "{:?}", r.sv);
        assert!((r.lerp_ms - 15.625).abs() < 1e-9);
        assert!((r.fps - 64.0).abs() < 1.5);
    }

    #[test]
    fn a_clients_readout_is_its_connections() {
        let mut view = NetGraphView::default();
        let mut frames = ServerFrames::default();
        let mut g = NetGraph::default();
        g.ping_ms = 80.0;
        g.loss = 0.05;
        g.cmd_rate = 64.0;
        g.incoming.mean = 300.0;
        g.incoming.bytes_per_sec = 6000.0;
        g.server = Some(crate::net::stats::ServerStats {
            fps: 64.0,
            frame_dev_ms: 0.5,
            tick_hz: 64.0,
            updates: 20.0,
            choke: 0.1,
        });
        let interp = InterpSettings::default();
        let rates = RateSettings::default();
        let r = readout(&mut view, 1.0, NetRole::Client, Some(&g), &mut frames, 64.0, Some(&interp), Some(&rates));
        assert_eq!((r.ping_ms, r.in_size, r.in_kbs, r.up, r.cmd), (80.0, 300, 6.0, 20.0, 64.0));
        assert!((r.loss - 5.0).abs() < 1e-9 && (r.choke - 10.0).abs() < 1e-4);
        // Not drawing others yet: the lerp from the settings (0.1 s, 2
        // intervals of cl_updaterate 20).
        assert!((r.lerp_ms - 100.0).abs() < 1e-3, "{}", r.lerp_ms);
    }

    #[test]
    fn graph_columns() {
        let mut cols = VecDeque::new();
        cols.push_back(Column {
            ping_ms: 125.0,
            in_bytes: 32,
            out_bytes: 16,
            loss: false,
            choke: false,
        });
        let h = 64;
        let px = draw_columns(&cols, h, true);
        let at = |x: usize, y: usize| {
            let row = h - 1 - y;
            let i = (row * COLUMNS + x) * 4;
            [px[i], px[i + 1], px[i + 2], px[i + 3]]
        };
        let x = COLUMNS - 1;
        // 125 of 250 ms: a quarter of the height (half of the lower half).
        assert_eq!(at(x, 0), LATENCY);
        assert_eq!(at(x, 15), LATENCY);
        assert_eq!(at(x, 16), IN_BYTES);
        assert_eq!(at(x, 17), IN_BYTES);
        assert_eq!(at(x, 18), OUT_BYTES);
        assert_eq!(at(x, 19), [0; 4]);
        assert_eq!(at(x - 1, 0), [0; 4]);
        // Without the outgoing band.
        let px = draw_columns(&cols, h, false);
        assert_eq!(px[((h - 1 - 18) * COLUMNS + x) * 4 + 3], 0);
    }
}
