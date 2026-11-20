//! Source's integer HDR on the client's cameras (specs/tf2/hdr_bloom.md;
//! maps whose `MapHdr::model` is `HdrModel::Integer`, TF2's): the frame is
//! the scene times one tone-map scale k, clamped and stored as 8-bit sRGB
//! (hdr_tonemap.wesl); a compute pass counts the centre region's luminance
//! histogram (source_meter.wesl), read back to the CPU where
//! `render::source_hdr::Exposure` keeps one bin per frame and turns them
//! into the goal and k; the bloom (source_bloom.wesl) is a quarter-size
//! soft-threshold downsample, a 13-tap blur across and down, each 8-bit,
//! added to the frame's bytes in gamma space and clamped, its amount
//! smoothed by 0.05 a frame from 1 at launch. Particles clamp their
//! fragments to 1 / k (particle.wesl), k riding in the views' colour
//! grading exposure. The exposure resets at map start, on the local
//! player's respawn and when the spectated target changes. Captures
//! (`--views`, `--screenshot`) settle k on its goal every frame and start
//! the bloom amount at the map's scale.
//!
//! `mat_disable_bloom` (1: no bloom) and `mat_force_tonemap_scale` (> 0:
//! k pinned) as in the game. `mat_hdr_level` 1 draws scale 1 with bloom
//! x 0.3; 2 meters and blooms in full.
//!
//! Known shortcuts (docs/tech-debt.md): Integer HDR: the frame is a float
//! target quantized to 8 bits by the tone-map pass, so only particles
//! clamp per fragment before blending (the world's opaque surfaces don't
//! need it; sprites and other translucents blend unclamped, then clamp).
//! The GPU counts every bin each frame and the CPU keeps one, the
//! readback's latency standing in for the game's 3-frame query delay.
//! Why it's OK for now: the only differences are overbright translucent
//! layers other than particles, and the metered counts are the same.

use bevy::{
    asset::{RenderAssetUsages, embedded_asset},
    camera::Hdr,
    core_pipeline::{
        FullscreenShader,
        schedule::{Core3d, Core3dSystems},
    },
    prelude::*,
    render::{
        Render, RenderApp,
        diagnostic::RecordDiagnostics, RenderStartup, RenderSystems,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        gpu_readback::{Readback, ReadbackComplete},
        render_asset::RenderAssets,
        render_resource::{
            binding_types::{sampler, storage_buffer_sized, texture_2d, uniform_buffer_sized},
            *,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        storage::{GpuShaderBuffer, ShaderBuffer},
        texture::{CachedTexture, TextureCache},
        view::{ExtractedView, ViewTarget},
    },
};

use super::hdr::{SourceTonemap, SourceTonemapSet};
use crate::{
    console::{ConsoleAppExt, resource_cvar},
    core::{Dead, LocalPlayer},
    map::{ActiveMapLook, HdrModel, TonemapInputs},
    render::source_hdr::{self, BloomAmount, COUNTS, Exposure},
};

/// `mat_disable_bloom` and `mat_force_tonemap_scale`.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct BloomCvars {
    pub disable_bloom: u8,
    pub force_tonemap_scale: f32,
}

/// The exposure and bloom state (one per client, as the game's).
#[derive(Resource, Debug, Default)]
pub struct IntegerHdr {
    pub exposure: Exposure,
    /// Per launch, not per map (Quirks 3).
    pub bloom_amount: BloomAmount,
    /// The bloom amount B drawn this frame.
    pub bloom: f32,
    pub frame: u64,
    /// The meter's readbacks not yet handed to the meter: (frame tag, counts).
    readings: Vec<(u32, [u32; COUNTS])>,
    /// The spectated target last seen (a change resets).
    spectated: Option<Entity>,
    /// Frames drawn on this map (0: the first; captures settle the bloom).
    map_frames: u64,
}

/// The tone-map scale k on a camera of an integer-HDR view (the render
/// world puts log2 k into its view's colour-grading exposure).
#[derive(Component, Clone, Copy, Debug, PartialEq, ExtractComponent)]
#[extract_app(bevy::render::RenderApp)]
pub struct ExposureScale(pub f32);

/// The meter's GPU counts (16 bins, every pixel, the frame's tag).
#[derive(Resource, Clone, ExtractResource)]
#[extract_app(bevy::render::RenderApp)]
struct MeterBuffer(Handle<ShaderBuffer>);

/// The entity reading the meter back.
#[derive(Component)]
struct MeterReadback;

pub struct IntegerHdrPlugin;

impl Plugin for IntegerHdrPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BloomCvars>().init_resource::<IntegerHdr>();
        resource_cvar::<BloomCvars, u8>(app, "mat_disable_bloom", "1: no bloom.", |s| &mut s.disable_bloom);
        resource_cvar::<BloomCvars, f32>(
            app,
            "mat_force_tonemap_scale",
            "> 0: pins the HDR tone-map scale (integer HDR maps).",
            |s| &mut s.force_tonemap_scale,
        );
        app.console_command(
            "mat_hdr_tonemapscale",
            "Integer HDR: the tone-map scale drawn with, its goal, the bloom amount and how many histogram bins have counts.",
            |w, _| {
                let s = w.resource::<IntegerHdr>();
                let bins = s.exposure.meter.counts.iter().filter(|c| c.is_some()).count();
                Ok(Some(format!(
                    "scale {:.4} goal {:.4} bloom {:.4} (amount {:.4}) bins {bins}/{COUNTS}",
                    s.exposure.scale, s.exposure.goal, s.bloom, s.bloom_amount.0
                )))
            },
        );
        app.add_systems(PostUpdate, integer_exposure.after(super::hdr::HdrCamerasSet));
        if app.get_sub_app(RenderApp).is_none() {
            return;
        }
        let buffer = app
            .world_mut()
            .resource_mut::<Assets<ShaderBuffer>>()
            .add(ShaderBuffer::new(vec![0u32; COUNTS + 1], RenderAssetUsages::RENDER_WORLD));
        app.insert_resource(MeterBuffer(buffer));
        embedded_asset!(app, "source_meter.wesl");
        embedded_asset!(app, "source_bloom.wesl");
        app.add_plugins((
            ExtractComponentPlugin::<ExposureScale>::default(),
            ExtractResourcePlugin::<MeterBuffer>::default(),
        ));
        app.sub_app_mut(RenderApp)
            .add_systems(RenderStartup, init_pipelines)
            .add_systems(
                Render,
                (
                    view_exposure.in_set(RenderSystems::PrepareViews),
                    prepare_textures.in_set(RenderSystems::PrepareResources),
                ),
            )
            .add_systems(
                Core3d,
                integer_post
                    .after(super::hdr::source_tonemap)
                    .in_set(SourceTonemapSet)
                    .in_set(Core3dSystems::PostProcess),
            );
    }
}

fn on_readback(ev: On<ReadbackComplete>, mut state: ResMut<IntegerHdr>) {
    let data: Vec<u32> = ev.to_shader_type();
    if data.len() > COUNTS {
        let counts: [u32; COUNTS] = std::array::from_fn(|i| data[i]);
        state.readings.push((data[COUNTS], counts));
        let n = state.readings.len();
        if n > 16 {
            state.readings.drain(..n - 16);
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn integer_exposure(
    mut state: ResMut<IntegerHdr>,
    cvars: Res<BloomCvars>,
    look: Option<Res<ActiveMapLook>>,
    inputs: Option<Res<TonemapInputs>>,
    time: Res<Time>,
    args: Option<Res<super::ClientArgs>>,
    meter: Option<Res<MeterBuffer>>,
    spectator: Option<Res<super::spectate::Spectator>>,
    locals: Query<(), With<LocalPlayer>>,
    mut revived: RemovedComponents<Dead>,
    readbacks: Query<Entity, With<MeterReadback>>,
    mut post: Query<&mut SourceTonemap>,
    cameras: Query<(Entity, Option<&ExposureScale>), (With<Hdr>, Without<super::debug::UiCamera>)>,
    mut commands: Commands,
) {
    let hdr = look
        .as_ref()
        .and_then(|l| l.0.hdr.clone())
        .filter(|h| h.model == HdrModel::Integer)
        .map(|h| inputs.as_ref().map_or(h.clone(), |i| i.apply(&h)));
    let Some(hdr) = hdr else {
        for (e, scale) in &cameras {
            if scale.is_some() {
                commands.entity(e).remove::<ExposureScale>();
            }
        }
        for e in &readbacks {
            commands.entity(e).despawn();
        }
        return;
    };
    let state = &mut *state;
    // Level start, respawn, a new spectated target: back to scale 1.
    let spectated = spectator.as_ref().and_then(|s| s.target);
    let respawned = revived.read().any(|e| locals.contains(e));
    if look.as_ref().is_some_and(|l| l.is_changed()) {
        state.exposure.reset();
        state.map_frames = 0;
    } else if respawned || spectated != state.spectated {
        state.exposure.reset();
    }
    state.spectated = spectated;
    let capture = args.is_some_and(|a| a.0.views.is_some() || a.0.screenshot.is_some());
    state.frame += 1;
    let frame = state.frame;

    // Auto exposure at mat_hdr_level 2; scale 1 at 1.
    let metering = hdr.exposure.is_some() && meter.is_some();
    if let (Some(m), true) = (meter.as_ref(), metering) {
        if readbacks.is_empty() {
            commands
                .spawn((Name::new("HDR meter readback"), MeterReadback, Readback::buffer(m.0.clone())))
                .observe(on_readback);
        }
        state.exposure.meter.issue(frame);
        for (tag, counts) in std::mem::take(&mut state.readings) {
            let issued: Vec<u64> = state
                .exposure
                .meter
                .pending
                .iter()
                .map(|p| p.1)
                .filter(|f| *f as u32 == tag)
                .collect();
            for f in issued {
                state.exposure.meter.deliver(f, &counts);
            }
        }
    }
    match hdr.exposure {
        Some((lo, hi)) => {
            let range = source_hdr::resolve_range(Some(lo), Some(hi), false);
            state.exposure.step(frame, range, time.delta_secs(), hdr.rate, capture);
        }
        None => state.exposure.reset(),
    }
    if cvars.force_tonemap_scale > 0.0 {
        state.exposure.scale = cvars.force_tonemap_scale;
    }
    let k = state.exposure.scale;

    // The bloom amount, smoothed while bloom is on.
    state.bloom = if cvars.disable_bloom != 0 {
        0.0
    } else {
        if capture && state.map_frames == 0 {
            state.bloom_amount = BloomAmount(hdr.bloom_scale);
        }
        let a = state.bloom_amount.step(hdr.bloom_scale.max(0.0));
        source_hdr::bloom_amount(a, hdr.exposure.is_some())
    };
    state.map_frames += 1;

    for mut t in &mut post {
        t.integer = 1.0;
        t.scale = k;
        t.bloom = state.bloom;
        t.auto_exposure = if metering { 1.0 } else { 0.0 };
        t.frame = frame as u32;
    }
    for (e, scale) in &cameras {
        if scale.map(|s| s.0) != Some(k) {
            commands.entity(e).insert(ExposureScale(k));
        }
    }
}

/// k into the colour-grading exposure of the views that draw with it (no
/// tone map reads it there: Bevy's is off on these cameras).
fn view_exposure(mut views: Query<(&mut ExtractedView, &ExposureScale)>) {
    for (mut view, k) in &mut views {
        view.color_grading.global.exposure = k.0.max(1e-4).log2();
    }
}

/// The bloom's quarter-size 8-bit work images.
#[derive(Component)]
struct BloomTextures {
    a: CachedTexture,
    b: CachedTexture,
    size: UVec2,
}

const QUARTER: TextureFormat = TextureFormat::Rgba8Unorm;

fn prepare_textures(
    mut commands: Commands,
    mut cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    views: Query<(Entity, &ViewTarget, &SourceTonemap)>,
) {
    for (e, target, t) in &views {
        let s = target.main_texture().size();
        let quarter = UVec2::new(s.width / 4, s.height / 4).max(UVec2::ONE);
        if t.integer < 0.5 || t.bloom <= 0.0 {
            commands.entity(e).remove::<BloomTextures>();
            continue;
        }
        let texture = |label: &'static str| TextureDescriptor {
            label: Some(label),
            size: Extent3d {
                width: quarter.x,
                height: quarter.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: QUARTER,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        commands.entity(e).insert(BloomTextures {
            a: cache.get(&render_device, texture("source_bloom_a")),
            b: cache.get(&render_device, texture("source_bloom_b")),
            size: quarter,
        });
    }
}

#[derive(Resource)]
struct Pipelines {
    meter_layout: BindGroupLayoutDescriptor,
    meter: CachedComputePipelineId,
    bloom_layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    down: CachedRenderPipelineId,
    hblur: CachedRenderPipelineId,
    vblur: CachedRenderPipelineId,
    add: CachedRenderPipelineId,
}

fn init_pipelines(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen_shader: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    let meter_layout = BindGroupLayoutDescriptor::new(
        "source_meter_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                storage_buffer_sized(false, None),
                uniform_buffer_sized(false, None),
            ),
        ),
    );
    let meter = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("source_meter".into()),
        layout: vec![meter_layout.clone()],
        shader: asset_server.load("embedded://chimera_client/client/source_meter.wesl"),
        entry_point: Some("meter".into()),
        zero_initialize_workgroup_memory: true,
        ..default()
    });
    let bloom_layout = BindGroupLayoutDescriptor::new(
        "source_bloom_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
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
    let shader: Handle<Shader> = asset_server.load("embedded://chimera_client/client/source_bloom.wesl");
    let pipeline = |entry: &'static str, format: TextureFormat| {
        pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some(format!("source_bloom_{entry}").into()),
            layout: vec![bloom_layout.clone()],
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
    commands.insert_resource(Pipelines {
        down: pipeline("down", QUARTER),
        hblur: pipeline("hblur", QUARTER),
        vblur: pipeline("vblur", QUARTER),
        add: pipeline("add", TextureFormat::Rgba16Float),
        meter_layout,
        meter,
        bloom_layout,
        sampler,
    });
}

fn uniform(device: &RenderDevice, label: &'static str, words: &[u32]) -> Buffer {
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some(label),
        contents: &bytes,
        usage: BufferUsages::UNIFORM,
    })
}

/// After the tone map: meter the frame, then bloom it.
#[allow(clippy::too_many_arguments)]
fn integer_post(
    view: ViewQuery<(&ViewTarget, &SourceTonemap, Option<&BloomTextures>)>,
    pipelines: Option<Res<Pipelines>>,
    pipeline_cache: Res<PipelineCache>,
    meter: Option<Res<MeterBuffer>>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    mut ctx: RenderContext,
) {
    let Some(p) = pipelines else { return };
    let (target, t, textures) = view.into_inner();
    if t.integer < 0.5 || target.main_texture_format() != TextureFormat::Rgba16Float {
        return;
    }
    // Timed as one pass in `chimera_perf` ("source_hdr": meter and bloom).
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let span = diagnostics.time_span(ctx.command_encoder(), "source_hdr");
    integer_passes(&mut ctx, &p, &pipeline_cache, meter, &buffers, target, t, textures);
    span.end(ctx.command_encoder());
}

#[allow(clippy::too_many_arguments)]
fn integer_passes(
    ctx: &mut RenderContext,
    p: &Pipelines,
    pipeline_cache: &PipelineCache,
    meter: Option<Res<MeterBuffer>>,
    buffers: &RenderAssets<GpuShaderBuffer>,
    target: &ViewTarget,
    t: &SourceTonemap,
    textures: Option<&BloomTextures>,
) {
    let device = ctx.render_device().clone();
    let size = {
        let s = target.main_texture().size();
        UVec2::new(s.width, s.height)
    };
    // The meter: every bin of the centre region, tagged with the frame.
    if t.auto_exposure > 0.5
        && let Some(m) = meter
        && let Some(gpu) = buffers.get(&m.0)
        && let Some(pipeline) = pipeline_cache.get_compute_pipeline(p.meter)
    {
        let (x0, y0, x1, y1) = source_hdr::meter_region(size.x, size.y);
        let params = uniform(&device, "source_meter_params", &[x0, y0, x1, y1, t.frame, 0, 0, 0]);
        let bind = device.create_bind_group(
            "source_meter",
            &pipeline_cache.get_bind_group_layout(&p.meter_layout),
            &BindGroupEntries::sequential((
                target.main_texture_view(),
                gpu.buffer.as_entire_binding(),
                params.as_entire_binding(),
            )),
        );
        let encoder = ctx.command_encoder();
        encoder.clear_buffer(&gpu.buffer, 0, None);
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("source_meter"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups((x1 - x0).div_ceil(16), (y1 - y0).div_ceil(16), 1);
    }
    // The bloom.
    let Some(tex) = textures.filter(|_| t.bloom > 0.0) else {
        return;
    };
    let ids = [p.down, p.hblur, p.vblur, p.add];
    let Some(ready) = ids
        .iter()
        .map(|id| pipeline_cache.get_render_pipeline(*id))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let q = tex.size.as_vec2();
    let words: Vec<u32> = [q.x, q.y, 1.0 / q.x, 1.0 / q.y, t.bloom, 0.0, 0.0, 0.0]
        .iter()
        .map(|f| f.to_bits())
        .collect();
    let params = uniform(&device, "source_bloom_params", &words);
    let layout = pipeline_cache.get_bind_group_layout(&p.bloom_layout);
    let draw = |ctx: &mut RenderContext, i: usize, from: &TextureView, bloom: &TextureView, to: &TextureView| {
        let bind = device.create_bind_group(
            "source_bloom",
            &layout,
            &BindGroupEntries::sequential((from, bloom, &p.sampler, params.as_entire_binding())),
        );
        let mut pass = ctx.command_encoder().begin_render_pass(&RenderPassDescriptor {
            label: Some("source_bloom"),
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
        pass.set_pipeline(ready[i]);
        pass.set_bind_group(0, &bind, &[]);
        pass.draw(0..3, 0..1);
    };
    let (a, b) = (&tex.a.default_view, &tex.b.default_view);
    let frame = target.main_texture_view();
    draw(ctx, 0, frame, frame, a);
    draw(ctx, 1, a, a, b);
    draw(ctx, 2, b, b, a);
    let post = target.post_process_write();
    draw(ctx, 3, post.source, a, post.destination);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{MapHdr, MapLook};

    fn app(exposure: Option<(f32, f32)>) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, crate::console::ConsolePlugin, IntegerHdrPlugin));
        app.insert_resource(ActiveMapLook(MapLook {
            hdr: Some(MapHdr {
                exposure,
                bloom_scale: 0.5,
                rate: 1.0,
                model: HdrModel::Integer,
            }),
            ..default()
        }));
        // The camera that runs the post-processing (`hdr::hdr_cameras`
        // gives it these).
        let post = app
            .world_mut()
            .spawn((
                Hdr,
                SourceTonemap {
                    integer: 1.0,
                    scale: 1.0,
                    ..default()
                },
            ))
            .id();
        (app, post)
    }

    fn cvar(app: &mut App, name: &str, value: &str) {
        crate::console::execute(app.world_mut(), &[name.to_string(), value.to_string()], 0);
    }

    fn tonemap(app: &App, e: Entity) -> SourceTonemap {
        *app.world().get::<SourceTonemap>(e).unwrap()
    }

    /// Spec 5.1 on the cameras: the bloom amount from 1, 0.05 a frame
    /// toward 2fort's 0.5; off under mat_disable_bloom (and not smoothed
    /// meanwhile).
    #[test]
    fn the_bloom_amount_follows_the_map_and_the_cvar() {
        let (mut app, post) = app(Some((0.8, 1.1)));
        app.update();
        let t = tonemap(&app, post);
        assert!((t.bloom - 0.975).abs() < 1e-4, "{t:?}");
        // Headless there is no GPU meter (auto_exposure 0: no pass).
        assert_eq!((t.integer, t.auto_exposure), (1.0, 0.0));
        for _ in 0..13 {
            app.update();
        }
        assert!((tonemap(&app, post).bloom - 0.744).abs() < 1e-3);
        cvar(&mut app, "mat_disable_bloom", "1");
        app.update();
        app.update();
        assert_eq!(tonemap(&app, post).bloom, 0.0);
        let a = app.world().resource::<IntegerHdr>().bloom_amount.0;
        assert!((a - 0.744).abs() < 1e-3, "not smoothed while off: {a}");
        // No meter readings headless: the goal holds at 1 and k with it,
        // which the cameras carry for particles.
        assert_eq!(t.scale, 1.0);
        assert_eq!(app.world().get::<ExposureScale>(post), Some(&ExposureScale(1.0)));
    }

    /// mat_hdr_level 1 (no exposure): scale 1, bloom x 0.3.
    #[test]
    fn level_one_blooms_less_at_scale_one() {
        let (mut app, post) = app(None);
        app.world_mut().resource_mut::<IntegerHdr>().bloom_amount = BloomAmount(0.5);
        app.update();
        let t = tonemap(&app, post);
        assert!((t.bloom - 0.15).abs() < 1e-6, "{t:?}");
        assert_eq!((t.scale, t.auto_exposure), (1.0, 0.0));
    }

    /// mat_force_tonemap_scale pins k; the local player's respawn resets
    /// the exposure to 1.
    #[test]
    fn forced_scale_and_the_respawn_reset() {
        let (mut app, post) = app(Some((0.5, 2.0)));
        cvar(&mut app, "mat_force_tonemap_scale", "1.5");
        app.update();
        assert_eq!(tonemap(&app, post).scale, 1.5);
        cvar(&mut app, "mat_force_tonemap_scale", "0");
        {
            let mut s = app.world_mut().resource_mut::<IntegerHdr>();
            (s.exposure.scale, s.exposure.goal) = (1.7, 1.7);
        }
        let me = app.world_mut().spawn((LocalPlayer, Dead { since: 0.0 })).id();
        app.update();
        assert!((tonemap(&app, post).scale - 1.7).abs() < 1e-6, "the goal holds without data");
        app.world_mut().entity_mut(me).remove::<Dead>();
        app.update();
        assert_eq!(tonemap(&app, post).scale, 1.0);
    }

    /// Another model's map: nothing of this on the cameras.
    #[test]
    fn stand_in_maps_are_left_alone() {
        let (mut app, post) = app(Some((0.5, 2.0)));
        app.update();
        assert!(app.world().get::<ExposureScale>(post).is_some());
        app.world_mut().resource_mut::<ActiveMapLook>().0.hdr.as_mut().unwrap().model = HdrModel::StandIn;
        app.update();
        assert!(app.world().get::<ExposureScale>(post).is_none());
    }
}
