//! What materials no entity draws cost the render world, offscreen (no
//! window or display needed, a GPU is): a camera per view draws `DRAWN`
//! cubes, each with its own `PropMaterial`, into an image, while the store
//! holds `n` more that nothing draws, in phases (`0 12000 0` by default:
//! none, then many, then none again). Prints each phase's instructions a
//! frame (the main thread, where the render world runs here, and every
//! thread) and how many GPU buffers wgpu's allocator holds: whether a
//! material held but not drawn costs every frame, and whether the cost
//! falls once it's gone (docs/performance.md, "A session's render thread").
//!
//! `cargo run -p chimera_present --features dev --example material_price -- [--views <k>] [--frames <f>] [<n>...]`
//! (Linux: instructions need hardware counters, `perf_event_paranoid` <= 2).

use bevy::{
    app::{PluginsState, ScheduleRunnerPlugin},
    camera::RenderTarget,
    prelude::*,
    render::{RenderPlugin, pipelined_rendering::PipelinedRenderingPlugin, render_resource::TextureFormat},
    window::ExitCondition,
    winit::WinitPlugin,
};
use chimera_core::metrics::counters::{AllThreads, ThreadCounters};
use chimera_present::render::prop_material::{PropMaterial, PropMaterialPlugin};

/// Cubes drawn, each with its own material (about the bodies and held
/// models a game draws).
const DRAWN: usize = 300;

#[derive(Resource, Default)]
struct Pad(Vec<Handle<PropMaterial>>);

fn main() {
    let mut views = 4;
    let mut frames = 300;
    let mut phases: Vec<usize> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--views" => views = args.next().and_then(|v| v.parse().ok()).expect("--views <k>"),
            "--frames" => frames = args.next().and_then(|v| v.parse().ok()).expect("--frames <f>"),
            n => phases.push(n.parse().expect("a phase: how many undrawn materials")),
        }
    }
    if phases.is_empty() {
        phases = vec![0, 12000, 0];
    }
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                close_when_requested: false,
                ..default()
            })
            .set(RenderPlugin::default())
            .disable::<WinitPlugin>()
            .disable::<PipelinedRenderingPlugin>(),
    )
    .add_plugins(ScheduleRunnerPlugin::default())
    .add_plugins(PropMaterialPlugin)
    .init_resource::<Pad>();
    while app.plugins_state() == PluginsState::Adding {
        bevy::tasks::tick_global_task_pools_on_main_thread();
    }
    app.finish();
    app.cleanup();
    scene(&mut app, views);
    // Shaders and pipelines compile in the first frames.
    for _ in 0..120 {
        app.update();
    }
    println!("views {views}, drawn {DRAWN}, frames {frames} a phase");
    let mut main = ThreadCounters::current();
    let mut all = AllThreads::default();
    for n in phases {
        pad(app.world_mut(), n);
        // Let it be prepared (and freed) first.
        for _ in 0..30 {
            app.update();
        }
        let (m0, a0) = (main.read(), all.read());
        for _ in 0..frames {
            app.update();
        }
        let (m1, a1) = (main.read(), all.read());
        let per = |a: chimera_core::metrics::counters::Counts, b: chimera_core::metrics::counters::Counts| {
            match (a.hw, b.hw) {
                (Some(a), Some(b)) => format!("{:.2}M", (b - a).instructions as f64 / frames as f64 / 1e6),
                _ => format!("{:.3} ms cpu", (b.cpu_ns - a.cpu_ns) as f64 / frames as f64 / 1e6),
            }
        };
        let store = app.world().resource::<Assets<PropMaterial>>().len();
        println!(
            "undrawn {n:>6}: store {store:>6}, main {} a frame, all threads {}",
            per(m0, m1),
            per(a0, a1)
        );
    }
}

fn pad(w: &mut World, n: usize) {
    let mut held = std::mem::take(&mut w.resource_mut::<Pad>().0);
    held.truncate(n);
    let mut store = w.resource_mut::<Assets<PropMaterial>>();
    while held.len() < n {
        held.push(store.add(PropMaterial::default()));
    }
    w.resource_mut::<Pad>().0 = held;
}

fn scene(app: &mut App, views: usize) {
    let w = app.world_mut();
    let mesh = w.resource_mut::<Assets<Mesh>>().add(Cuboid::new(0.2, 0.2, 0.2));
    for i in 0..DRAWN {
        let material = w.resource_mut::<Assets<PropMaterial>>().add(PropMaterial {
            params: chimera_present::render::prop_material::PropParams {
                base_color: Vec4::new((i % 7) as f32 / 7.0, 0.5, 0.5, 1.0),
                ..default()
            },
            ..default()
        });
        let (x, y) = ((i % 20) as f32 * 0.3 - 3.0, (i / 20) as f32 * 0.3 - 2.0);
        w.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material), Transform::from_xyz(x, y, 0.0)));
    }
    for v in 0..views {
        let image = Image::new_target_texture(1280, 720, TextureFormat::Rgba8UnormSrgb, None);
        let image = w.resource_mut::<Assets<Image>>().add(image);
        w.spawn((
            Camera3d::default(),
            Camera { order: v as isize, ..default() },
            RenderTarget::Image(image.into()),
            Transform::from_xyz(0.0, 0.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        ));
    }
}
