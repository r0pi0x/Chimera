//! Night vision (CS:S's `nightvision`, N): the local player's goggles
//! (`weapon::economy::NightVision`, bought with `buy nvgs`) turned on and
//! off, with the game's sounds (`Prices::nightvision_on`/`_off`, heard by
//! the player only), and the view they give: the finished frame added to
//! itself through a green filter, brighter and green (the install's
//! `materials/effects/nightvision.vmt`: an additive full-screen copy of the
//! frame buffer, `_rt_FullFrameFB`). The pass runs on the last camera
//! drawing the view (the view model's when it draws, else the world's),
//! after color correction and before the HUD. Losing the goggles (death)
//! turns it off; spectating shows nobody's.
//!
//! `nightvision` (N) toggles the local player's goggles when it carries them
//! (`NightVisionOn`), playing their on/off sound to the player; the frame gets
//! the green additive pass (`night_vision.wesl`) on the last camera drawing the
//! view, after color correction; off without goggles (death).
//!
//! Known shortcuts (docs/tech-debt.md): Night vision: the overlay's colour (the
//! frame plus the frame times pure green, `NIGHT_VISION_TINT`) is ours (the
//! material says additive frame buffer; the colour the game draws it with isn't
//! in the install); no light of its own around the player; the toggle sound
//! plays to its owner only (whether CS:S plays it to others is unknown);
//! others never see your goggles' state. Why it's OK for now: Reads as CS:S's
//! green night vision; capture C24 compares.

use bevy::{
    asset::embedded_asset,
    core_pipeline::{
        FullscreenShader,
        schedule::{Core3d, Core3dSystems},
        tonemapping::tonemapping,
    },
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_resource::{
            binding_types::{sampler, texture_2d},
            *,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        view::ViewTarget,
    },
};

use super::FirstPersonCamera;
use crate::{
    console::ConsoleAppExt,
    core::{Health, LocalPlayer},
    map::{PlaySound, ViewModelCamera},
    weapon::economy::{NightVision, Prices},
};

/// The colour the frame is added to itself through (gamma space): the
/// frame's green doubles, its red and blue stay. Ours; see the module
/// doc. Mirrored in `night_vision.wesl`.
pub const NIGHT_VISION_TINT: [f32; 3] = [0.0, 1.0, 0.0];

/// Whether the local player's night vision is on.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NightVisionOn(pub bool);

/// On the camera that runs the pass this frame.
#[derive(Component, Clone, Copy, Debug, Default, ExtractComponent)]
#[extract_app(bevy::render::RenderApp)]
pub struct NightVisionPass;

pub struct NightVisionPlugin;

impl Plugin for NightVisionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NightVisionOn>()
            .add_systems(Update, lose_sight)
            // After the view-model cameras decide whether they draw this frame.
            .add_systems(
                PostUpdate,
                pass_camera.after(crate::render::view_model::ViewModelCameraSet),
            );
        app.console_command(
            "nightvision",
            "Turn your night vision goggles on or off (N; buy them with buy nvgs).",
            |w, _| {
                toggle(w);
                Ok(None)
            },
        );
        if app.get_sub_app(RenderApp).is_none() {
            return;
        }
        embedded_asset!(app, "night_vision.wesl");
        app.add_plugins(ExtractComponentPlugin::<NightVisionPass>::default());
        app.sub_app_mut(RenderApp)
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(
                Core3d,
                night_vision
                    .after(tonemapping)
                    .after(super::hdr::SourceTonemapSet)
                    .after(super::color_correction::ColorCorrectionSet)
                    .after(crate::render::screen_post::ScreenPostSet)
                    .in_set(Core3dSystems::PostProcess),
            );
    }
}

/// `nightvision`: on or off, if the living local player has goggles.
pub fn toggle(world: &mut World) {
    let mut q = world.query_filtered::<(Entity, &Transform, Option<&Health>, Has<NightVision>), With<LocalPlayer>>();
    let Some((me, t, health, has)) = q.iter(world).next().map(|(e, t, h, n)| (e, *t, h.copied(), n)) else {
        return;
    };
    if !has || health.is_some_and(|h| h.current <= 0.0) {
        return;
    }
    let on = !world.resource::<NightVisionOn>().0;
    world.insert_resource(NightVisionOn(on));
    let prices = world.resource::<Prices>();
    let entry = if on {
        prices.nightvision_on.clone()
    } else {
        prices.nightvision_off.clone()
    };
    if let Some(entry) = entry
        && world.contains_resource::<Messages<PlaySound>>()
    {
        world.write_message(PlaySound {
            source: Some(me),
            ..PlaySound::at(entry, t.translation)
        });
    }
}

/// Without goggles (dead, or never bought) there is no night vision.
fn lose_sight(mut on: ResMut<NightVisionOn>, me: Query<(Has<NightVision>, Option<&Health>), With<LocalPlayer>>) {
    if !on.0 {
        return;
    }
    let keep = me
        .iter()
        .next()
        .is_some_and(|(has, h)| has && h.is_none_or(|h| h.current > 0.0));
    if !keep {
        on.0 = false;
    }
}

/// The pass on the last camera drawing the view while it's on.
fn pass_camera(
    on: Res<NightVisionOn>,
    first_person: Query<(Entity, &Camera, Option<&Children>), With<FirstPersonCamera>>,
    view_models: Query<&Camera, With<ViewModelCamera>>,
    marked: Query<Entity, With<NightVisionPass>>,
    mut commands: Commands,
) {
    let post = on.0.then(|| {
        first_person
            .iter()
            .find(|(_, c, _)| c.is_active)
            .map(|(e, _, children)| {
                children
                    .into_iter()
                    .flatten()
                    .find(|c| view_models.get(**c).is_ok_and(|c| c.is_active))
                    .copied()
                    .unwrap_or(e)
            })
    });
    let post = post.flatten();
    for e in &marked {
        if Some(e) != post {
            commands.entity(e).remove::<NightVisionPass>();
        }
    }
    if let Some(e) = post
        && !marked.contains(e)
    {
        commands.entity(e).insert(NightVisionPass);
    }
}

#[derive(Resource)]
struct NightVisionPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    /// For HDR (Rgba16Float) and LDR (sRGB 8-bit) view targets.
    hdr: CachedRenderPipelineId,
    ldr: CachedRenderPipelineId,
}

fn init_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen_shader: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "night_vision_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );
    let sampler = render_device.create_sampler(&SamplerDescriptor::default());
    let shader = asset_server.load("embedded://chimera_client/client/night_vision.wesl");
    let pipeline = |format: TextureFormat| {
        pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("night_vision_pipeline".into()),
            layout: vec![layout.clone()],
            vertex: fullscreen_shader.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: shader.clone(),
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
    let hdr = pipeline(TextureFormat::Rgba16Float);
    let ldr = pipeline(TextureFormat::Rgba8UnormSrgb);
    commands.insert_resource(NightVisionPipeline {
        layout,
        sampler,
        hdr,
        ldr,
    });
}

fn night_vision(
    view: ViewQuery<(&ViewTarget, &NightVisionPass)>,
    pipeline: Option<Res<NightVisionPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let Some(pipeline) = pipeline else { return };
    let (target, _) = view.into_inner();
    let id = if target.main_texture_format() == TextureFormat::Rgba16Float {
        pipeline.hdr
    } else if target.main_texture_format() == TextureFormat::Rgba8UnormSrgb {
        pipeline.ldr
    } else {
        return;
    };
    let Some(render_pipeline) = pipeline_cache.get_render_pipeline(id) else {
        return;
    };
    let post = target.post_process_write();
    let bind_group = ctx.render_device().create_bind_group(
        "night_vision_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((post.source, &pipeline.sampler)),
    );
    let mut pass = ctx.command_encoder().begin_render_pass(&RenderPassDescriptor {
        label: Some("night_vision_pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: post.destination,
            depth_slice: None,
            resolve_target: None,
            ops: Operations::default(),
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(render_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}
