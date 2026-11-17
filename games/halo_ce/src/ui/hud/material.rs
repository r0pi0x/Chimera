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
    /// x: the scaling (`HudFilter::code`).
    pub style: Vec4,
}

/// How the HUD's pictures are scaled up to the screen (`halo_ce:hud_filter`,
/// an opt-in setting; the pictures are authored for 640 by 480).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HudFilter {
    /// Linear, as the game's pictures were always drawn here.
    #[default]
    Original,
    /// Nearest at the whole-number scale inside each texel, linear only over
    /// the one-pixel seam between texels (`sharp_coord`): crisp edges at any
    /// window size.
    Sharp,
    /// Linear with a light sharpening (an unsharp mask over the four
    /// neighbours a screen pixel away).
    SmoothSharpen,
}

impl HudFilter {
    pub fn from_code(code: u8) -> HudFilter {
        match code {
            1 => HudFilter::Sharp,
            2 => HudFilter::SmoothSharpen,
            _ => HudFilter::Original,
        }
    }

    pub fn code(self) -> u8 {
        self as u8
    }
}

/// The setting `halo_ce:hud_filter`: 0 original, 1 sharp, 2 smooth and
/// sharpened.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HudFilterSetting(pub u8);

/// Where a `Sharp` picture samples along one axis (hud.wesl's `sharp`,
/// mirrored here to test it): `p` the position in texels, `scale` the
/// screen pixels per texel. Inside a texel the position snaps to its
/// centre (nearest) except for the last pixel's width at each edge, where
/// it runs linearly to the neighbour's centre, so the texel boundaries
/// land on whole pixels' seams.
pub fn sharp_coord(p: f32, scale: f32) -> f32 {
    if scale <= 1.0 {
        return p;
    }
    let base = p.floor();
    let centre = p - base - 0.5;
    let flat = 0.5 - 0.5 / scale;
    base + (centre - centre.clamp(-flat, flat)) * scale + 0.5
}

impl HudParams {
    /// The numbers of a quad's fill over the texture rectangle `uv`.
    pub fn of(fill: &Fill, uv: [f32; 4]) -> HudParams {
        let v3 = |c: [f32; 3], a: f32| Vec4::new(c[0], c[1], c[2], a);
        let uv = Vec4::from_array(uv);
        match fill {
            Fill::Sprite(c) => HudParams { mode: Vec4::ZERO, color: Vec4::from_array(*c), uv, ..default() },
            Fill::Reticle(c) => HudParams { mode: Vec4::new(0.0, 1.0, 0.0, 0.0), color: Vec4::from_array(*c), uv, ..default() },
            Fill::Meter(m) => HudParams {
                mode: Vec4::new(1.0, m.value, m.extension, m.tint),
                color: Vec4::ONE,
                uv,
                lo: v3(m.min, m.fill_keep),
                hi: v3(m.max, m.empty_keep),
                flash: v3(m.flash, 0.0),
                empty: v3(m.empty, 0.0),
                style: Vec4::ZERO,
            },
            Fill::Darken => HudParams { mode: Vec4::new(3.0, 0.0, 0.0, 0.0), color: Vec4::ONE, uv, ..default() },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sharp_sampling_is_flat_inside_a_texel_and_runs_over_one_pixel_at_its_edge() {
        // Scale 2.25 pixels per texel: flat for the middle 1 - 1/2.25 of it.
        let flat = 0.5 - 0.5 / 2.25;
        assert_eq!(sharp_coord(3.5, 2.25), 3.5);
        assert!((sharp_coord(3.5 + flat * 0.99, 2.25) - 3.5).abs() < 0.02);
        // The seam: from the texel's centre at the flat edge to the next
        // texel's centre across one pixel (1/2.25 texel).
        assert!((sharp_coord(4.0, 2.25) - 4.0).abs() < 1e-6);
        assert!((sharp_coord(4.0 + 0.5 / 2.25, 2.25) - 4.5).abs() < 1e-5);
        assert!((sharp_coord(4.0 - 0.5 / 2.25, 2.25) - 3.5).abs() < 1e-5);
        // Never leaves the texel pair it is between, and never goes back.
        let mut last = 0.0;
        for i in 0..400 {
            let c = sharp_coord(i as f32 * 0.01, 2.25);
            assert!(c >= last - 1e-6 && (c - i as f32 * 0.01).abs() <= 0.5 + 1e-5);
            last = c;
        }
        // At or under one pixel per texel nothing changes.
        assert_eq!(sharp_coord(3.3, 1.0), 3.3);
        assert_eq!(sharp_coord(3.3, 0.5), 3.3);
        // Scale 2: flat over the middle half, a pixel's seam at the edge.
        assert!((sharp_coord(3.9, 2.0) - 3.8).abs() < 1e-5);
        assert_eq!(HudFilter::from_code(1), HudFilter::Sharp);
        assert_eq!(HudFilter::from_code(9), HudFilter::Original);
    }
}
