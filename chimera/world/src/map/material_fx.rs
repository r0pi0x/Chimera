//! Material effects shared by the world and prop shaders
//! (specs/cs_source/shaders.md): texture coordinate transforms
//! (`$basetexturetransform` and a TextureScroll proxy on it), the detail
//! combine modes and self-illumination. The combine and self-illum
//! functions here mirror world.wesl and prop.wesl on the CPU, so the
//! spec's test cases check the maths the shaders run.
//!
//! What the world and prop shaders share from specs/cs_source/shaders.md:
//! `MapUvTransform` (`$basetexturetransform` parsed from its VMT string, or a
//! TextureScroll proxy's scroll, sent as two rows plus a scroll rate),
//! `DetailMode`, `MapSelfIllum`, and CPU mirrors of the shaders' detail combine
//! and self-illum for the spec's test cases (chimera/present/tests/it/shader_spec.rs).
//!
//! Known shortcuts (docs/tech-debt.md): Material effects past the spec
//! (`map::material_fx`, specs/cs_source/shaders.md open questions 16-19):
//! `$basetexturetransform` composed as uv' = c + R(S uv - c) + t from the
//! public wiki's wording; a TextureScroll proxy on it replaces it; the
//! TextureTransform proxy, `$bumptransform` (without `$detail`),
//! `$envmapmasktransform`, `$blendmasktransform`, `$detailtexturetransform`,
//! `$selfillum_envmapmask_alpha` and detail modes 10/11 aren't read;
//! `$selfillummask` masks per rgb channel; the depth prepass of alpha-tested
//! surfaces ignores the scroll. Why it's OK for now: Community maps' scrolling,
//! scaled and self-lit materials look right; check against the game when the
//! spec answers.

use bevy::math::{Vec2, Vec3, Vec4};

/// A texture coordinate transform: uv' = rows · (u, v, 1), plus a
/// translation scrolling at `scroll` (texture repeats per second, wrapped
/// to [0, 1) as Source's TextureScroll proxy does).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapUvTransform {
    pub rows: [[f32; 3]; 2],
    pub scroll: [f32; 2],
}

impl Default for MapUvTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl MapUvTransform {
    pub const IDENTITY: Self = Self {
        rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        scroll: [0.0, 0.0],
    };

    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    /// From the parts of a VMT transform ("center cx cy scale sx sy rotate
    /// deg translate tx ty"; the Valve Developer Wiki's
    /// `$basetexturetransform` page: scale fits the texture that many
    /// times, the centre is the point of rotation only, rotation is
    /// counter-clockwise in degrees, translate shifts by whole repeats):
    /// uv' = c + R(deg) · (S · uv − c) + t, with R = [[cos, −sin], [sin,
    /// cos]] in (u, v). The order and the sense of rotation in (u, v) are
    /// taken, not specified (shaders.md open question 16).
    pub fn from_parts(center: Vec2, scale: Vec2, rotate_degrees: f32, translate: Vec2) -> Self {
        let (s, c) = rotate_degrees.to_radians().sin_cos();
        let a = [c * scale.x, -s * scale.y];
        let b = [s * scale.x, c * scale.y];
        let rc = Vec2::new(c * center.x - s * center.y, s * center.x + c * center.y);
        let off = center - rc + translate;
        Self {
            rows: [[a[0], a[1], off.x], [b[0], b[1], off.y]],
            scroll: [0.0, 0.0],
        }
    }

    /// A VMT transform string. Keywords may come in any order and any may
    /// be missing (the default then: center .5 .5, scale 1 1, rotate 0,
    /// translate 0 0); a lone number after `scale` scales both axes. None
    /// when nothing in it is readable.
    pub fn parse(v: &str) -> Option<Self> {
        let t: Vec<&str> = v.split_whitespace().collect();
        let num = |i: usize| t.get(i).and_then(|w| w.parse::<f32>().ok());
        let (mut center, mut scale, mut rotate, mut translate) = (Vec2::splat(0.5), Vec2::ONE, 0.0, Vec2::ZERO);
        let mut any = false;
        for (i, w) in t.iter().enumerate() {
            let pair = || Some(Vec2::new(num(i + 1)?, num(i + 2).or(num(i + 1))?));
            match w.to_ascii_lowercase().as_str() {
                "center" => {
                    if let Some(p) = pair() {
                        center = p;
                        any = true;
                    }
                }
                "scale" => {
                    if let Some(p) = pair() {
                        scale = p;
                        any = true;
                    }
                }
                "rotate" => {
                    if let Some(r) = num(i + 1) {
                        rotate = r;
                        any = true;
                    }
                }
                "translate" => {
                    if let Some(p) = pair() {
                        translate = p;
                        any = true;
                    }
                }
                _ => {}
            }
        }
        any.then(|| Self::from_parts(center, scale, rotate, translate))
    }

    /// Source's TextureScroll proxy writing this transform: a translation
    /// of `rate` (repeats per second) along `angle_degrees`, after scaling
    /// by `scale` (the proxy's `texturescale`, 1 when unset). It replaces
    /// the material's own value (specs/cs_source/water.md, TextureScroll).
    pub fn scrolling(rate: f32, angle_degrees: f32, scale: f32) -> Self {
        let (s, c) = angle_degrees.to_radians().sin_cos();
        Self {
            rows: [[scale, 0.0, 0.0], [0.0, scale, 0.0]],
            scroll: [rate * c, rate * s],
        }
    }

    /// The transformed coordinates at time `t` (seconds).
    pub fn apply(&self, uv: Vec2, t: f32) -> Vec2 {
        let [a, b] = self.rows;
        let scroll = (Vec2::from_array(self.scroll) * t).fract_gl();
        Vec2::new(a[0] * uv.x + a[1] * uv.y + a[2], b[0] * uv.x + b[1] * uv.y + b[2]) + scroll
    }

    /// The two rows for a shader: (m00, m01, m02, scroll u) and (m10, m11,
    /// m12, scroll v).
    pub fn shader_rows(&self) -> [Vec4; 2] {
        let [a, b] = self.rows;
        [
            Vec4::new(a[0], a[1], a[2], self.scroll[0]),
            Vec4::new(b[0], b[1], b[2], self.scroll[1]),
        ]
    }
}

/// `$detailblendmode` and the two-texture blend's own detail modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DetailMode {
    /// Source's `$detailblendmode` 0-9 (shaders.md 2, "$detail").
    Source(u8),
    /// WorldTwoTextureBlend: detail over base
    /// (specs/cs_source/shaders_two_texture_blend.md).
    TwoTextureOver,
    /// WorldTwoTextureBlend's 2x grime mask.
    TwoTextureMask,
    /// LithTech's detail texture (specs/combat_arms/textures_dtx.md,
    /// "Detail textures", `DetailTextureAdd` 1): clamp(base + detail -
    /// 0.5) in gamma space, alpha from the base (`signed_add`).
    SignedAdd,
}

impl DetailMode {
    /// The shader's `detail` parameter: 0 none, 1 + the Source mode, 20
    /// and 21 the two-texture modes, 30 the signed add (see world.wesl).
    pub fn shader_value(self) -> f32 {
        match self {
            Self::Source(m) => m as f32 + 1.0,
            Self::TwoTextureOver => 20.0,
            Self::TwoTextureMask => 21.0,
            Self::SignedAdd => 30.0,
        }
    }
}

/// Source `$selfillum`: the surface glows with its own colour where the
/// mask says (base alpha, or `$selfillummask` on models).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapSelfIllum {
    /// `$selfillumtint`, linear as the shader uses it.
    pub tint: [f32; 3],
    /// `$selfillummask` (index into `MapData::textures`, linear); None:
    /// the base texture's alpha.
    pub mask: Option<usize>,
}

/// The pre-lighting detail combine (shaders.md 2, "$detail"; modes 5 and 6
/// add after lighting, see `detail_post_lighting`): albedo `a` (linear
/// rgba), detail texel `d` (as sampled, times `$detailtint`), blend factor
/// `f`. Mirrors world.wesl and prop.wesl.
pub fn detail_combine(mode: u8, a: Vec4, d: Vec4, f: f32) -> Vec4 {
    let rgb = a.truncate();
    let drgb = d.truncate();
    match mode {
        0 => (rgb * Vec3::ONE.lerp(2.0 * drgb, f)).extend(a.w),
        1 => (rgb + f * drgb).extend(a.w),
        2 => rgb.lerp(drgb, f * d.w).extend(a.w),
        3 => a.lerp(d, f),
        4 => rgb.lerp(drgb, f * (1.0 - a.w)).extend(d.w),
        7 => {
            let c = d.x + (d.w - d.x) * a.w;
            (rgb * (1.0 + (2.0 * c - 1.0) * f)).extend(a.w)
        }
        8 => a.lerp(a * d, f),
        9 => rgb.extend(a.w + (a.w * d.w - a.w) * f),
        _ => a,
    }
}

/// What the model shaders add after lighting for detail modes 5 and 6
/// (shaders.md 4); zero for the other modes.
pub fn detail_post_lighting(mode: u8, d: Vec3, f: f32) -> Vec3 {
    match mode {
        5 => f * d,
        6 => {
            let (m, a) = if f >= 0.5 {
                (1.0 / f, 1.0 - 1.0 / f)
            } else {
                (4.0 * f, -2.0 * f)
            };
            (m * d + Vec3::splat(a)).clamp(Vec3::ZERO, Vec3::ONE)
        }
        _ => Vec3::ZERO,
    }
}

/// LithTech's signed add of a second stage (detail texture or environment
/// map, specs/combat_arms/world_dat.md "Second textures"): clamp(base +
/// layer - 0.5, 0, 1) per channel, colours in gamma space. Mirrors
/// world.wesl.
pub fn signed_add(base: Vec3, layer: Vec3) -> Vec3 {
    (base + layer - Vec3::splat(0.5)).clamp(Vec3::ZERO, Vec3::ONE)
}

/// LithTech's dual-texture combine (world_dat.md "Second textures"): the
/// first texture `t0` (rgba) and the second `t1` by the vertex alpha
/// `vertex_alpha`; a = vertex alpha x t0's alpha, colour a x t0 + (1 - a)
/// x t1, or t0 + a x t1 (clamped) when t1 is fullbright (`add`); alpha a.
/// Gamma-space colours. Mirrors world.wesl (`BlendMode::BaseAlpha`,
/// `BaseAlphaAdd`; the shader's weight is 1 - vertex alpha).
pub fn dual_texture(t0: Vec4, t1: Vec3, vertex_alpha: f32, add: bool) -> Vec4 {
    let a = vertex_alpha * t0.w;
    let rgb = if add {
        (t0.truncate() + a * t1).min(Vec3::ONE)
    } else {
        a * t0.truncate() + (1.0 - a) * t1
    };
    rgb.extend(a)
}

/// LithTech's detail coordinates (textures_dtx.md "Detail textures"):
/// σ R(θ) (u, v), with σ the scale and θ the rotation (radians,
/// counter-clockwise). Mirrors world.wesl (`MapDetail::rotation`).
pub fn detail_uv(uv: Vec2, scale: f32, rotation: f32) -> Vec2 {
    let (s, c) = rotation.sin_cos();
    scale * Vec2::new(c * uv.x - s * uv.y, s * uv.x + c * uv.y)
}

/// A flat environment map's coordinates from the view-space reflection
/// vector `r` (unit; x right, y up): (r.x, r.y) x (-0.5 / scale) + 0.5
/// (world_dat.md "Second textures", the world environment-map matrix
/// taken as the identity). Mirrors world.wesl (`MapFlatEnvmap`).
pub fn flat_envmap_uv(r: Vec3, scale: f32) -> Vec2 {
    Vec2::new(r.x, r.y) * (-0.5 / scale) + Vec2::splat(0.5)
}

/// Self-illumination (shaders.md 2 and 4): the lit colour toward
/// `tint` × albedo by `mask`.
pub fn self_illum(diffuse: Vec3, albedo: Vec3, tint: Vec3, mask: f32) -> Vec3 {
    diffuse.lerp(tint * albedo, mask)
}
/// Source's parameter gamma-to-linear table (specs/cs_source/shaders.md
/// Quirks): rounded to 1/255; 0.95 and up become 1; above 1 unchanged.
pub fn parameter_gamma_to_linear(v: f32) -> f32 {
    if v > 1.0 {
        return v;
    }
    let v = (v * 255.0).round() / 255.0;
    if v >= 0.95 { 1.0 } else { v.powf(2.2) }
}


/// A texture whose frames show in turn (Source's AnimatedTexture proxy
/// on a material's `$basetexture` or `$bumpmap`): its frames (indices into
/// `MapData::textures`; a material drawing it binds `frames[0]` and
/// `render::texture_anim` swaps the frames in) and the proxy's
/// `animatedtextureframerate`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapTextureAnim {
    pub frames: Vec<usize>,
    /// Frames per second.
    pub rate: f32,
}

impl MapTextureAnim {
    /// The frame shown `t` seconds into the map (an index into `frames`):
    /// floor(rate · t) mod the frame count, a hard switch, no blending
    /// (specs/cs_source/water.md 4, AnimatedTexture). A rate of zero or
    /// less holds the first frame.
    pub fn frame(&self, t: f32) -> usize {
        let n = self.frames.len().max(1);
        if self.rate <= 0.0 || t <= 0.0 {
            return 0;
        }
        ((self.rate as f64 * t as f64).floor() as u64 % n as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// floor(rate · t) mod count, a hard switch; no rate holds frame 0.
    #[test]
    fn texture_anim_frames() {
        let anim = MapTextureAnim { frames: vec![10, 11, 12], rate: 21.0 };
        assert_eq!(anim.frame(0.0), 0);
        assert_eq!(anim.frame(0.047), 0);
        assert_eq!(anim.frame(0.048), 1);
        assert_eq!(anim.frame(2.0 / 21.0 + 1e-4), 2);
        assert_eq!(anim.frame(3.0 / 21.0 + 1e-4), 0);
        // cs_office's slide: a new slide every 10 s.
        let slide = MapTextureAnim { frames: vec![0; 5], rate: 0.1 };
        assert_eq!(slide.frame(9.9), 0);
        assert_eq!(slide.frame(10.1), 1);
        assert_eq!(slide.frame(49.0), 4);
        assert_eq!(slide.frame(51.0), 0);
        assert_eq!(MapTextureAnim { frames: vec![1, 2], rate: 0.0 }.frame(5.0), 0);
        // Hours into a map stays exact enough.
        assert_eq!(anim.frame(3600.0 + 1.0 / 42.0), (3600 * 21) % 3);
    }
}
