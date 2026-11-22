//! The Halo HUD material drawn offscreen (a GPU, no display, no Halo data)
//! with a made-up picture in each scaling (`HudFilter`): proves hud.wesl
//! compiles and draws, and saves a PNG to look at.
//!
//! `cargo run -p halo_ce_ui --features dev --example hud_filters -- <out.png>`

use bevy::{
    app::{PluginsState, ScheduleRunnerPlugin},
    camera::RenderTarget,
    image::ImageSampler,
    prelude::*,
    render::{RenderPlugin, pipelined_rendering::PipelinedRenderingPlugin, render_resource::{Extent3d, TextureDimension, TextureFormat}, view::screenshot::{Screenshot, save_to_disk}},
    window::ExitCondition,
    winit::WinitPlugin,
};
use halo_ce_ui::hud::material::{HudMaterial, HudParams, HudRenderPlugin};

const W: u32 = 1280;
const H: u32 = 720;

/// A 40 by 30 picture: a bright frame, a diagonal, a cross, a gradient bar.
fn picture() -> Image {
    let (w, h) = (40u32, 30u32);
    let mut data = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let edge = x == 0 || y == 0 || x == w - 1 || y == h - 1;
            let diag = x == y || x + 1 == y;
            let cross = x == 20 || y == 15;
            let bar = y > 22 && y < 27 && x > 4 && x < 36;
            let px = if edge {
                [255, 255, 255, 255]
            } else if diag {
                [255, 80, 80, 255]
            } else if cross {
                [80, 255, 120, 255]
            } else if bar {
                let v = ((x - 4) * 255 / 32) as u8;
                [v, v, 255 - v, 255]
            } else if (x / 4 + y / 4) % 2 == 0 {
                [40, 60, 90, 255]
            } else {
                [20, 30, 50, 255]
            };
            data.extend_from_slice(&px);
        }
    }
    let mut image = Image::new(Extent3d { width: w, height: h, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8Unorm, default());
    image.sampler = ImageSampler::linear();
    image
}

fn main() {
    let out = std::env::args().nth(1).expect("<out.png>");
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin { primary_window: None, exit_condition: ExitCondition::DontExit, close_when_requested: false, ..default() })
            .set(RenderPlugin::default())
            .disable::<WinitPlugin>()
            .disable::<PipelinedRenderingPlugin>(),
    )
    .add_plugins(ScheduleRunnerPlugin::default())
    .add_plugins(HudRenderPlugin);
    while app.plugins_state() == PluginsState::Adding {
        bevy::tasks::tick_global_task_pools_on_main_thread();
    }
    app.finish();
    app.cleanup();
    let w = app.world_mut();
    let target = w.resource_mut::<Assets<Image>>().add(Image::new_target_texture(W, H, TextureFormat::Rgba8UnormSrgb, None));
    let image = w.resource_mut::<Assets<Image>>().add(picture());
    let camera = w
        .spawn((Camera2d, Camera { clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)), ..default() }, RenderTarget::Image(target.clone().into())))
        .id();
    // Three scalings by two sizes (about 3.0 and 7.5 pixels a texel) and
    // a meter and a sweep.
    for (row, size) in [(0, Vec2::new(120.0, 90.0)), (1, Vec2::new(300.0, 225.0))] {
        for filter in 0..3u32 {
            let mut params = HudParams::of(&halo_ce_ui::hud::draw::Fill::Sprite([1.0; 4]), [0.0, 0.0, 1.0, 1.0]);
            params.style.x = filter as f32;
            let m = w.resource_mut::<Assets<HudMaterial>>().add(HudMaterial { params, image: image.clone(), mask: image.clone() });
            let (x, y) = (20.0 + filter as f32 * 420.0, 20.0 + row as f32 * 130.0 + if row == 1 { 0.0 } else { 0.0 });
            w.spawn((
                Node { position_type: PositionType::Absolute, left: px(x), top: px(y), width: px(size.x), height: px(size.y), ..default() },
                MaterialNode(m),
                UiTargetCamera(camera),
            ));
        }
    }
    for _ in 0..90 {
        app.update();
    }
    app.world_mut().spawn(Screenshot::image(target)).observe(save_to_disk(out));
    for _ in 0..30 {
        app.update();
    }
}
