//! The UI material Halo's HUD pictures are drawn with (`HudMaterial`,
//! hud.wesl): a sprite "alpha-multiply add", a meter by the meter shader's
//! rule, or the motion tracker's sweep, blended premultiplied (the
//! output's colour added, the frame kept by one minus its alpha), which
//! is the game's blend function 7 for every HUD picture
//! (specs/halo_ce/hud.md, section 3). `HudRenderPlugin` registers it (the
//! shell's apps only: the headless tests have no renderer).

use bevy::{
    asset::embedded_asset,
    prelude::*,
    reflect::TypePath,
    render::render_resource::{AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor, ShaderType},
    shader::ShaderRef,
};

use super::draw::Fill;

/// The material's numbers (hud.wesl's `HudParams`).
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct HudParams {
    pub mode: Vec4,
    pub color: Vec4,
    pub uv: Vec4,
    pub lo: Vec4,
    pub hi: Vec4,
    pub flash: Vec4,
    pub empty: Vec4,
}

impl HudParams {
    /// The numbers of a quad's fill over the texture rectangle `uv`.
    pub fn of(fill: &Fill, uv: [f32; 4]) -> HudParams {
        let v3 = |c: [f32; 3], a: f32| Vec4::new(c[0], c[1], c[2], a);
        let uv = Vec4::from_array(uv);
        match fill {
            Fill::Sprite(c) => HudParams { mode: Vec4::ZERO, color: Vec4::from_array(*c), uv, ..default() },
            Fill::Meter(m) => HudParams {
                mode: Vec4::new(1.0, m.value, m.extension, m.tint),
                color: Vec4::ONE,
                uv,
                lo: v3(m.min, m.fill_keep),
                hi: v3(m.max, m.empty_keep),
                flash: v3(m.flash, 0.0),
                empty: v3(m.empty, 0.0),
            },
            Fill::Sweep { colour, half, .. } => HudParams {
                mode: Vec4::new(2.0, 0.0, 0.0, 1.0),
                color: v3(*colour, 1.0),
                uv,
                flash: Vec4::new(0.0, 0.0, 0.0, *half),
                ..default()
            },
        }
    }
}

/// One HUD picture's material.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct HudMaterial {
    #[uniform(0)]
    pub params: HudParams,
    #[texture(1)]
    #[sampler(2)]
    pub image: Handle<Image>,
    /// The sweep's mask (white elsewhere: the default image).
    #[texture(3)]
    #[sampler(4)]
    pub mask: Handle<Image>,
}

impl UiMaterial for HudMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://halo_ce/ui/hud/hud.wesl".into()
    }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, _key: UiMaterialKey<Self>) {
        let premultiplied = BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState { color: premultiplied, alpha: premultiplied });
            }
        }
    }
}

/// The material and its shader (a windowed app's).
pub struct HudRenderPlugin;

impl Plugin for HudRenderPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "hud.wesl");
        app.add_plugins(UiMaterialPlugin::<HudMaterial>::default());
    }
}
