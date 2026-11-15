//! Full-screen passes a game asks for over the finished 3D frame, after
//! the world, the view model and anything drawn in their passes (a scope
//! card) and before the 2D HUD: a scene light (an additive tint and a
//! multiplied light scale) and a focus blur through a mask picture. Any
//! game sets `ScreenPost`; the client puts `ScreenPostPass` on the last
//! camera drawing the view (`client::screen_post`), whose post-process
//! runs these (`ScreenPostSet`, inside `Core3dSystems::PostProcess`).
//!
//! The scene light: per channel `clamp(c x scale + add, 0, 1)` on the
//! frame's gamma-encoded values (Combat Arms' damage tint and darkening,
//! specs/combat_arms/effects_sounds.md Follow-up (2026-11-14) 4). Skipped
//! when the add is 0 and the scale 1.
//!
//! The focus blur (specs/combat_arms/weapon_classes.md Follow-up
//! (2026-11-14) 3; `W x H` the frame, all sampling bilinear and clamped):
//! 1. the scene image S = the frame moved halfway to its grey, `S = c/2 +
//!    (0.3 r + 0.59 g + 0.11 b)/2` per channel (inferred in the spec: the
//!    shader's fixed 0.5; `FOCUS_SATURATION`);
//! 2. a quarter-size image D (`W/4 x H/4`, floored), each pixel the mean
//!    of S at its centre +-(1/W, 1/H) (one full-screen pixel diagonally);
//! 3. vertical blur: `B1 = D/2 + D(v + 8/H)/4 + D(v - 8/H)/4`;
//! 4. horizontal blur: `B = B1/2 + B1(u + 8/W)/4 + B1(u - 8/W)/4`;
//! 5. the merge: `a S + (1 - a) B` with `a` the mask's alpha, the mask
//!    stretched once over the frame.
//!
//! The CPU twins (`saturate`, `downsample`, `blur_1d`, `merge`) do the
//! same sums for tests (the shaders are `screen_post.wesl` and `screen_post_merge.wesl`).
//!
//! Known shortcuts: the passes run on the gamma-encoded values of the
//! tone-mapped frame (as the games' 8-bit frame buffers hold them); an
//! HDR frame is clamped to 0..1 by them. A mask not loaded yet skips the
//! blur for that frame.

use bevy::{
    asset::embedded_asset,
    core_pipeline::{
        FullscreenShader,
        schedule::{Core3d, Core3dSystems},
        tonemapping::tonemapping,
    },
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        camera::ExtractedCamera,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_asset::RenderAssets,
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer_sized},
            *,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        texture::{CachedTexture, GpuImage, TextureCache},
        view::ViewTarget,
    },
};

/// The grey mix of the focus blur's scene copy (step 1; the spec's
/// "fixed 0.5", inferred).
pub const FOCUS_SATURATION: f32 = 0.5;
/// The blur's tap distance, full-screen pixels.
pub const FOCUS_TAP: f32 = 8.0;

/// What a game asks of the frame this frame.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct ScreenPost {
    /// Added to the frame (gamma values, 0..1).
    pub light_add: Vec3,
    /// Multiplies the frame (1: none).
    pub light_scale: Vec3,
    /// The focus blur's mask (its alpha: 1 sharp, 0 blurred); None: no
    /// blur.
    pub focus: Option<Handle<Image>>,
}

impl Default for ScreenPost {
    fn default() -> Self {
        Self {
            light_add: Vec3::ZERO,
            light_scale: Vec3::ONE,
            focus: None,
        }
    }
}

impl ScreenPost {
    /// Whether any pass has work.
    pub fn is_on(&self) -> bool {
        self.light_on() || self.focus.is_some()
    }

    pub fn light_on(&self) -> bool {
        self.light_add != Vec3::ZERO || self.light_scale != Vec3::ONE
    }
}

/// On the camera that runs the passes this frame, with what they do.
#[derive(Component, Clone, Debug, PartialEq, ExtractComponent)]
#[extract_app(bevy::render::RenderApp)]
pub struct ScreenPostPass {
    pub light_add: Vec3,
    pub light_scale: Vec3,
    pub focus: Option<AssetId<Image>>,
}

impl ScreenPostPass {
    pub fn of(post: &ScreenPost) -> Self {
        Self {
            light_add: post.light_add,
            light_scale: post.light_scale,
            focus: post.focus.as_ref().map(Handle::id),
        }
    }
}

/// The passes' place among the post-processes (the client orders it
/// after its colour correction and before its night vision).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ScreenPostSet;

// ---------------------------------------------------------------------------
// The CPU twins (an image: rows of RGB, `w` wide)

/// Step 1: a colour moved halfway to its grey.
pub fn saturate(c: Vec3) -> Vec3 {
    let grey = 0.3 * c.x + 0.59 * c.y + 0.11 * c.z;
    c * (1.0 - FOCUS_SATURATION) + Vec3::splat(grey) * FOCUS_SATURATION
}

/// Bilinear, clamped, at texture coordinates `uv` of an image `w x h`.
pub fn sample(img: &[Vec3], w: usize, h: usize, uv: Vec2) -> Vec3 {
    let x = (uv.x * w as f32 - 0.5).clamp(0.0, (w - 1) as f32);
    let y = (uv.y * h as f32 - 0.5).clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let at = |x: usize, y: usize| img[y * w + x];
    let top = at(x0, y0).lerp(at(x1, y0), fx);
    let bottom = at(x0, y1).lerp(at(x1, y1), fx);
    top.lerp(bottom, fy)
}

/// Step 2: the quarter-size image of `s` (`w x h`).
pub fn downsample(s: &[Vec3], w: usize, h: usize) -> (Vec<Vec3>, usize, usize) {
    let (qw, qh) = ((w / 4).max(1), (h / 4).max(1));
    let px = Vec2::new(1.0 / w as f32, 1.0 / h as f32);
    let mut out = Vec::with_capacity(qw * qh);
    for y in 0..qh {
        for x in 0..qw {
            let uv = Vec2::new((x as f32 + 0.5) / qw as f32, (y as f32 + 0.5) / qh as f32);
            let sum: Vec3 = [Vec2::new(1.0, 1.0), Vec2::new(-1.0, 1.0), Vec2::new(1.0, -1.0), Vec2::new(-1.0, -1.0)]
                .iter()
                .map(|d| sample(s, w, h, uv + *d * px))
                .sum();
            out.push(sum / 4.0);
        }
    }
    (out, qw, qh)
}

/// Steps 3 and 4: one blur pass over a quarter image (`w x h`) of a
/// `screen` (`W x H`) frame, vertical or horizontal.
pub fn blur_1d(img: &[Vec3], w: usize, h: usize, screen: UVec2, vertical: bool) -> Vec<Vec3> {
    let step = if vertical {
        Vec2::new(0.0, FOCUS_TAP / screen.y as f32)
    } else {
        Vec2::new(FOCUS_TAP / screen.x as f32, 0.0)
    };
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let uv = Vec2::new((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
            out.push(sample(img, w, h, uv) * 0.5 + sample(img, w, h, uv + step) * 0.25 + sample(img, w, h, uv - step) * 0.25);
        }
    }
    out
}

/// Step 5: one output pixel from S, B and the mask's alpha.
pub fn merge(s: Vec3, b: Vec3, alpha: f32) -> Vec3 {
    s * alpha + b * (1.0 - alpha)
}

/// The whole blur on a frame (`w x h`), the mask's alpha by texture
/// coordinate.
pub fn focus_blur(frame: &[Vec3], w: usize, h: usize, alpha: impl Fn(Vec2) -> f32) -> Vec<Vec3> {
    let s: Vec<Vec3> = frame.iter().map(|c| saturate(*c)).collect();
    let (d, qw, qh) = downsample(&s, w, h);
    let screen = UVec2::new(w as u32, h as u32);
    let b1 = blur_1d(&d, qw, qh, screen, true);
    let b = blur_1d(&b1, qw, qh, screen, false);
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let uv = Vec2::new((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
            out.push(merge(s[y * w + x], sample(&b, qw, qh, uv), alpha(uv)));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The GPU passes

pub struct ScreenPostPlugin;

impl Plugin for ScreenPostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenPost>();
        if app.get_sub_app(RenderApp).is_none() {
            return;
        }
        embedded_asset!(app, "screen_post.wesl");
        embedded_asset!(app, "screen_post_merge.wesl");
        app.add_plugins(ExtractComponentPlugin::<ScreenPostPass>::default());
        app.sub_app_mut(RenderApp)
            .add_systems(RenderStartup, init_pipelines)
            .add_systems(Render, prepare_textures.in_set(RenderSystems::PrepareResources))
            .add_systems(
                Core3d,
                screen_post
                    .after(tonemapping)
                    .in_set(ScreenPostSet)
                    .in_set(Core3dSystems::PostProcess),
            );
    }
}

/// The quarter images' format.
const QUARTER: TextureFormat = TextureFormat::Rgba16Float;

#[derive(Resource)]
struct Pipelines {
    one: BindGroupLayoutDescriptor,
    three: BindGroupLayoutDescriptor,
    sampler: Sampler,
    /// By the view's format: (HDR, LDR).
    light: [CachedRenderPipelineId; 2],
    saturate: CachedRenderPipelineId,
    down: CachedRenderPipelineId,
    vblur: CachedRenderPipelineId,
    hblur: CachedRenderPipelineId,
    merge: [CachedRenderPipelineId; 2],
}

fn init_pipelines(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen_shader: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    let one = BindGroupLayoutDescriptor::new(
        "screen_post_one",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer_sized(false, None),
            ),
        ),
    );
    let three = BindGroupLayoutDescriptor::new(
        "screen_post_three",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                texture_2d(TextureSampleType::Float { filterable: true }),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer_sized(false, None),
            ),
        ),
    );
    let sampler = render_device.create_sampler(&SamplerDescriptor {
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        ..default()
    });
    let shader = asset_server.load("embedded://chimera_present/render/screen_post.wesl");
    let merge_shader: Handle<Shader> = asset_server.load("embedded://chimera_present/render/screen_post_merge.wesl");
    let pipeline = |entry: &'static str, layout: &BindGroupLayoutDescriptor, format: TextureFormat| {
        let shader = if entry == "merge" { merge_shader.clone() } else { shader.clone() };
        pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some(format!("screen_post_{entry}").into()),
            layout: vec![layout.clone()],
            vertex: fullscreen_shader.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: shader.clone(),
                entry_point: Some(entry.into()),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        })
    };
    let (hdr, ldr) = (TextureFormat::Rgba16Float, TextureFormat::Rgba8UnormSrgb);
    commands.insert_resource(Pipelines {
        light: [pipeline("light", &one, hdr), pipeline("light", &one, ldr)],
        saturate: pipeline("saturate", &one, QUARTER),
        down: pipeline("down", &one, QUARTER),
        vblur: pipeline("vblur", &one, QUARTER),
        hblur: pipeline("hblur", &one, QUARTER),
        merge: [pipeline("merge", &three, hdr), pipeline("merge", &three, ldr)],
        one,
        three,
        sampler,
    });
}

/// The blur's work images on a view that asks for it.
#[derive(Component)]
struct FocusTextures {
    scene: CachedTexture,
    quarter_a: CachedTexture,
    quarter_b: CachedTexture,
}

fn prepare_textures(
    mut commands: Commands,
    mut cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    views: Query<(Entity, &ExtractedCamera, &ScreenPostPass)>,
) {
    for (e, camera, pass) in &views {
        let Some(size) = camera.physical_target_size.or(camera.physical_viewport_size) else {
            continue;
        };
        if pass.focus.is_none() || size.x == 0 || size.y == 0 {
            commands.entity(e).remove::<FocusTextures>();
            continue;
        }
        let texture = |label: &'static str, s: UVec2| TextureDescriptor {
            label: Some(label),
            size: Extent3d {
                width: s.x.max(1),
                height: s.y.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: QUARTER,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let quarter = size / 4;
        commands.entity(e).insert(FocusTextures {
            scene: cache.get(&render_device, texture("screen_post_scene", size)),
            quarter_a: cache.get(&render_device, texture("screen_post_quarter_a", quarter)),
            quarter_b: cache.get(&render_device, texture("screen_post_quarter_b", quarter)),
        });
    }
}

/// The parameters: the frame's size and its inverse, the add, the scale.
fn params(device: &RenderDevice, size: UVec2, pass: &ScreenPostPass) -> Buffer {
    let w = size.x.max(1) as f32;
    let h = size.y.max(1) as f32;
    let floats: [f32; 12] = [
        w,
        h,
        1.0 / w,
        1.0 / h,
        pass.light_add.x,
        pass.light_add.y,
        pass.light_add.z,
        0.0,
        pass.light_scale.x,
        pass.light_scale.y,
        pass.light_scale.z,
        1.0,
    ];
    let bytes: Vec<u8> = floats.iter().flat_map(|f| f.to_le_bytes()).collect();
    device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("screen_post_params"),
        contents: &bytes,
        usage: BufferUsages::UNIFORM,
    })
}

fn screen_post(
    view: ViewQuery<(&ViewTarget, &ScreenPostPass, Option<&FocusTextures>)>,
    pipelines: Option<Res<Pipelines>>,
    pipeline_cache: Res<PipelineCache>,
    images: Res<RenderAssets<GpuImage>>,
    mut ctx: RenderContext,
) {
    let Some(p) = pipelines else { return };
    let (target, pass, textures) = view.into_inner();
    let slot = if target.main_texture_format() == TextureFormat::Rgba16Float {
        0
    } else if target.main_texture_format() == TextureFormat::Rgba8UnormSrgb {
        1
    } else {
        return;
    };
    let size = {
        let s = target.main_texture().size();
        UVec2::new(s.width, s.height)
    };
    let device = ctx.render_device().clone();
    let uniform = params(&device, size, pass);
    let draw = |ctx: &mut RenderContext, id: CachedRenderPipelineId, bind: &BindGroup, to: &TextureView| -> bool {
        let Some(pipeline) = pipeline_cache.get_render_pipeline(id) else {
            return false;
        };
        let mut rp = ctx.command_encoder().begin_render_pass(&RenderPassDescriptor {
            label: Some("screen_post_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: to,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rp.set_pipeline(pipeline);
        rp.set_bind_group(0, bind, &[]);
        rp.draw(0..3, 0..1);
        true
    };
    let one = pipeline_cache.get_bind_group_layout(&p.one);
    let bind_one = |from: &TextureView| {
        device.create_bind_group(
            "screen_post_one",
            &one,
            &BindGroupEntries::sequential((from, &p.sampler, uniform.as_entire_binding())),
        )
    };
    // The scene light.
    if (pass.light_add != Vec3::ZERO || pass.light_scale != Vec3::ONE)
        && pipeline_cache.get_render_pipeline(p.light[slot]).is_some()
    {
        let post = target.post_process_write();
        let bind = bind_one(post.source);
        draw(&mut ctx, p.light[slot], &bind, post.destination);
    }
    // The focus blur (every pipeline ready, or the frame as it is).
    let (Some(mask), Some(t)) = (pass.focus.and_then(|m| images.get(m)), textures) else {
        return;
    };
    let ready = [p.saturate, p.down, p.vblur, p.hblur, p.merge[slot]]
        .iter()
        .all(|id| pipeline_cache.get_render_pipeline(*id).is_some());
    if !ready {
        return;
    }
    let post = target.post_process_write();
    draw(&mut ctx, p.saturate, &bind_one(post.source), &t.scene.default_view);
    draw(&mut ctx, p.down, &bind_one(&t.scene.default_view), &t.quarter_a.default_view);
    draw(&mut ctx, p.vblur, &bind_one(&t.quarter_a.default_view), &t.quarter_b.default_view);
    draw(&mut ctx, p.hblur, &bind_one(&t.quarter_b.default_view), &t.quarter_a.default_view);
    let three = pipeline_cache.get_bind_group_layout(&p.three);
    let bind = device.create_bind_group(
        "screen_post_three",
        &three,
        &BindGroupEntries::sequential((
            &t.scene.default_view,
            &t.quarter_a.default_view,
            &mask.texture_view,
            &p.sampler,
            uniform.as_entire_binding(),
        )),
    );
    draw(&mut ctx, p.merge[slot], &bind, post.destination);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// weapon_classes.md 50 (a): a uniform red frame → S = (0.65, 0.15,
    /// 0.15) everywhere, B = S, so that whatever the mask.
    #[test]
    fn case_50a_uniform_red() {
        let (w, h) = (64, 48);
        let frame = vec![Vec3::new(1.0, 0.0, 0.0); w * h];
        for a in [0.0, 0.5, 1.0] {
            let out = focus_blur(&frame, w, h, |_| a);
            for c in out {
                assert!((c - Vec3::new(0.65, 0.15, 0.15)).abs().max_element() < 1e-5, "{c}");
            }
        }
    }

    /// 50 (b): the 1-D kernel on one bright quarter pixel: 0.5 there, 0.25
    /// two quarter pixels each side (8 screen pixels), 0 at one.
    #[test]
    fn case_50b_kernel() {
        let (qw, qh) = (256, 192);
        let screen = UVec2::new(1024, 768);
        let mut img = vec![Vec3::ZERO; qw * qh];
        let (x, y) = (100, 90);
        img[y * qw + x] = Vec3::ONE;
        let h = blur_1d(&img, qw, qh, screen, false);
        let at = |dx: i32| h[y * qw + (x as i32 + dx) as usize].x;
        assert!((at(0) - 0.5).abs() < 1e-6);
        assert!((at(2) - 0.25).abs() < 1e-6 && (at(-2) - 0.25).abs() < 1e-6);
        assert!(at(1).abs() < 1e-6 && at(-1).abs() < 1e-6);
        let v = blur_1d(&img, qw, qh, screen, true);
        assert!((v[(y + 2) * qw + x].x - 0.25).abs() < 1e-6, "vertical: 8/768 is two quarter rows");
        // 1024 x 768 → 256 x 192 work images.
        let (_, dw, dh) = downsample(&vec![Vec3::ZERO; 1024 * 768], 1024, 768);
        assert_eq!((dw, dh), (256, 192));
    }

    /// 50 (c): alpha 1 → S; alpha 0.5 → the mean of S and B.
    #[test]
    fn case_50c_mask() {
        let (s, b) = (Vec3::new(0.2, 0.4, 0.6), Vec3::new(0.6, 0.2, 0.0));
        assert_eq!(merge(s, b, 1.0), s);
        assert!((merge(s, b, 0.5) - (s + b) / 2.0).abs().max_element() < 1e-6);
        // On a frame: a sharp edge stays sharp where the alpha is 1.
        let (w, h) = (32, 32);
        let frame: Vec<Vec3> = (0..w * h).map(|i| if i % w < w / 2 { Vec3::ONE } else { Vec3::ZERO }).collect();
        let sharp = focus_blur(&frame, w, h, |_| 1.0);
        for (o, f) in sharp.iter().zip(&frame) {
            assert!((*o - saturate(*f)).abs().max_element() < 1e-6);
        }
        let soft = focus_blur(&frame, w, h, |_| 0.0);
        let edge = soft[5 * w + w / 2];
        assert!(edge.x > 0.0 && edge.x < 1.0, "blurred across the edge: {edge}");
    }

    #[test]
    fn screen_post_is_off_by_default() {
        assert!(!ScreenPost::default().is_on());
    }
}
