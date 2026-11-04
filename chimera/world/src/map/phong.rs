//! Source's Phong model shading (specs/cs_source/shaders_phong.md): which
//! VertexLitGeneric materials take it (`route`), what the loader reads of a
//! `$phong` material (`MapPhong`: exponent rule, fresnel ramp, boost, tint,
//! rim light, the warps, the envmap mask rules), and a CPU mirror of the
//! per-pixel maths prop.wesl runs on it (`shade` and its parts), so the
//! spec's test cases check the numbers without a GPU
//! (chimera/world/tests/it/phong.rs). Vectors here are in engine axes (Y
//! up): the spec's N_z, the up component, is `y`, and the ambient cube is
//! ordered +X -X +Y -Y +Z -Z as `LightProbe::cube`.
//!
//! Known shortcuts (docs/tech-debt.md): Phong shading's unsettled inputs
//! (shaders_phong.md open questions): a parameter missing from the VMT takes
//! the spec's stated defaults (fresnel ranges (0, 0.5, 1), boost 1, rim
//! exponent 4, rim boost 1), not "defined and zero" (question 1); the probe's
//! lights are directional (L constant, question 2); a material without a
//! normal map on the full path uses a flat normal with alpha 1 (question 4);
//! the lightwarp reads its first row (question 6); the phong warp's u is the
//! lobe's scalar part before the light's colour; the normal map is read
//! through a screen-space tangent frame (no per-vertex VVD tangents) in
//! prop.wesl; `$selfillumfresnel`, `$blendtintbybasealpha`, wrinkle maps
//! and the flashlight pass are not drawn; static props lit by vertex colours
//! get the diffuse only (no probe, so no specular or rim). Why it's OK for
//! now: every stock phong material sets fresnel ranges and boost and has a
//! normal map, none uses the skipped parameters; check rim boost and the
//! lights against the game by refcmp (question 9).

use bevy::math::{Vec3, Vec4};

/// Local lights evaluated per pixel.
pub const MAX_LIGHTS: usize = 4;
/// `$phongfresnelranges` when unset (min, mid, max).
pub const FRESNEL_RANGES_DEFAULT: [f32; 3] = [0.0, 0.5, 1.0];
/// `$rimlightexponent` when unset.
pub const RIM_EXPONENT_DEFAULT: f32 = 4.0;
/// A lightwarp texel is doubled.
pub const LIGHTWARP_SCALE: f32 = 2.0;

/// How a VertexLitGeneric material is drawn (shaders_phong.md 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhongRoute {
    /// The Phong shader, per pixel (sections 2-8, 10).
    Phong,
    /// Its reduced variant (section 9).
    Cheap,
    /// Plain VertexLitGeneric (shaders.md 4): `$phong*` and `$rim*` ignored.
    Plain,
}

/// What the renderer can do (section 1, "DX9 fallbacks"); all on for us.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhongSupport {
    /// Pixel shader 2.0b.
    pub ps20b: bool,
    /// The video config's Phong (not "disable phong").
    pub phong: bool,
    /// mat_bumpmap 1.
    pub bumpmap: bool,
}

impl Default for PhongSupport {
    fn default() -> Self {
        Self {
            ps20b: true,
            phong: true,
            bumpmap: true,
        }
    }
}

/// The parts of a VertexLitGeneric material that decide its route.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhongInputs {
    pub phong: bool,
    pub bumpmap: bool,
    pub lightwarp: bool,
    /// `$basemapalphaphongmask` exactly 1.
    pub base_alpha_mask: bool,
    pub exponent_texture: bool,
    pub phong_warp: bool,
    pub rimlight: bool,
    pub detail: bool,
    pub selfillum: bool,
    pub blend_tint_by_base_alpha: bool,
    /// The base texture has an alpha channel.
    pub base_has_alpha: bool,
}

/// Which shader draws a material (section 1).
pub fn route(m: PhongInputs, support: PhongSupport) -> PhongRoute {
    if !m.phong || !(m.lightwarp || m.base_alpha_mask || m.bumpmap) {
        return PhongRoute::Plain;
    }
    if !support.ps20b || !support.phong || !support.bumpmap {
        return PhongRoute::Plain;
    }
    let simple = !m.bumpmap
        && !m.exponent_texture
        && !m.phong_warp
        && !m.rimlight
        && !m.detail
        && !m.selfillum
        && !m.blend_tint_by_base_alpha;
    if simple && (m.base_alpha_mask || !m.base_has_alpha) {
        PhongRoute::Cheap
    } else {
        PhongRoute::Phong
    }
}

/// Where the reflection's mask comes from on the Phong path (section 8).
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PhongEnvMask {
    /// The base alpha, not inverted (the default).
    #[default]
    BaseAlpha,
    /// The specular mask (`$normalmapalphaenvmapmask`).
    SpecularMask,
    /// `$selfillumfresnel`: `$invertphongmask`'s 0/1 value.
    Constant(f32),
}

/// `$rimlight` and its parameters (section 7).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapRimLight {
    /// k_r: `$rimlightexponent`, at least 1, 4 when unset.
    pub exponent: f32,
    /// `$rimlightboost` (1 when unset).
    pub boost: f32,
    /// `$rimmask` with an exponent texture: its alpha masks the rim.
    pub mask: bool,
}

/// A VertexLitGeneric `$phong` material as the Phong shader reads it
/// (`MapMesh::phong`; textures are indices into `MapData::textures`, raw).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapPhong {
    /// The reduced variant (section 9).
    pub cheap: bool,
    /// The exponent k = `exponent.0` + `exponent.1` × exp.r (section 5).
    pub exponent: (f32, f32),
    /// `$phongexponenttexture` (r exponent, g albedo tint, a rim mask).
    pub exponent_texture: Option<usize>,
    /// `$phongfresnelranges` (min, mid, max).
    pub fresnel_ranges: [f32; 3],
    pub boost: f32,
    /// S_t (section 6).
    pub tint: PhongTint,
    pub rim: Option<MapRimLight>,
    /// `$lightwarptexture`.
    pub lightwarp: Option<usize>,
    /// `$phongwarptexture`.
    pub phong_warp: Option<usize>,
    /// `$basemapalphaphongmask`: base alpha is the mask, the normal map's
    /// shading is dropped.
    pub base_alpha_mask: bool,
    pub env_mask: PhongEnvMask,
    /// `$invertphongmask`: inverts the envmap mask (only).
    pub invert_env_mask: bool,
    /// `$envmapfresnel`.
    pub envmap_fresnel: f32,
}

impl Default for MapPhong {
    fn default() -> Self {
        Self {
            cheap: false,
            exponent: (1.0, 149.0),
            exponent_texture: None,
            fresnel_ranges: FRESNEL_RANGES_DEFAULT,
            boost: 1.0,
            tint: PhongTint::White,
            rim: None,
            lightwarp: None,
            phong_warp: None,
            base_alpha_mask: false,
            env_mask: PhongEnvMask::BaseAlpha,
            invert_env_mask: false,
            envmap_fresnel: 0.0,
        }
    }
}

/// The specular tint S_t (section 6).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PhongTint {
    White,
    /// `$phongtint`, raw.
    Color([f32; 3]),
    /// lerp(1, base.rgb, exp.g): `$phongalbedotint` with an exponent texture.
    Albedo,
}

/// The exponent rule (section 5; section 9 for the cheap variant) as
/// k = a + b × exp.r. `factor`: `$phongexponentfactor` (0 unset);
/// `exponent`: `$phongexponent` (None or ≤ 0: unset).
pub fn exponent_rule(factor: f32, exponent: Option<f32>, cheap: bool) -> (f32, f32) {
    let constant = exponent.filter(|e| *e > 0.0);
    if cheap {
        let c = if factor != 0.0 { factor } else { constant.unwrap_or(-1.0) };
        return (c.max(0.0), 0.0);
    }
    if factor != 0.0 {
        (1.0, factor)
    } else if let Some(k) = constant {
        (k, 0.0)
    } else {
        (1.0, 149.0)
    }
}

/// k_r from `$rimlightexponent` (section 7, 3).
pub fn rim_exponent(set: Option<f32>) -> f32 {
    set.map_or(RIM_EXPONENT_DEFAULT, |k| k.max(1.0))
}

/// S_t from what the material set (section 6): `$phongtint` (all zero:
/// unset), `$phongalbedotint` and whether an exponent texture exists.
pub fn tint_rule(phong_tint: Option<[f32; 3]>, albedo_tint: bool, exponent_texture: bool) -> PhongTint {
    match phong_tint {
        Some(t) if t != [0.0; 3] => PhongTint::Color(t),
        _ if albedo_tint && exponent_texture => PhongTint::Albedo,
        _ => PhongTint::White,
    }
}

/// S_t for a pixel: `base` the linear base colour after detail, before
/// modulation; `exp_g` the exponent texture's green.
pub fn tint(rule: PhongTint, base: Vec3, exp_g: f32) -> Vec3 {
    match rule {
        PhongTint::White => Vec3::ONE,
        PhongTint::Color(c) => Vec3::from_array(c),
        PhongTint::Albedo => Vec3::ONE.lerp(base, exp_g),
    }
}

/// The three-point fresnel ramp F of (1 − N·V)², not clamped (section 7).
pub fn fresnel_ramp(ranges: [f32; 3], n_dot_v: f32) -> f32 {
    let [m0, m1, m2] = ranges;
    let f = (1.0 - n_dot_v).clamp(0.0, 1.0).powi(2);
    if f < 0.5 {
        m0 + (m1 - m0) * 2.0 * f
    } else {
        m1 + (m2 - m1) * (2.0 * f - 1.0)
    }
}

/// The half-Lambert diffuse term, always (section 4).
pub fn half_lambert(n_dot_l: f32) -> f32 {
    (0.5 * n_dot_l + 0.5).clamp(0.0, 1.0).powi(2)
}

/// The lightwarp's coordinate u for N·L (not squared).
pub fn lightwarp_u(n_dot_l: f32) -> f32 {
    (0.5 * n_dot_l + 0.5).clamp(0.0, 1.0)
}

/// One light's lobe without its colour: clamp(R·L)^k × clamp(N·L); 0
/// behind the light or with R·L ≤ 0 (0⁰ counts as 0, Edge cases).
pub fn lobe(r_dot_l: f32, n_dot_l: f32, k: f32) -> f32 {
    if r_dot_l <= 0.0 || n_dot_l <= 0.0 {
        return 0.0;
    }
    r_dot_l.min(1.0).powf(k) * n_dot_l.min(1.0)
}

/// R: V reflected about N.
pub fn reflect_view(n: Vec3, v: Vec3) -> Vec3 {
    2.0 * n * n.dot(v) - v
}

/// The ambient cube at direction `n` (shaders.md 4): the six colours
/// (+X -X +Y -Y +Z -Z) weighted by the squared components.
pub fn ambient(cube: &[Vec3; 6], n: Vec3) -> Vec3 {
    let pick = |v: f32, pos: Vec3, neg: Vec3| if v >= 0.0 { pos } else { neg };
    pick(n.x, cube[0], cube[1]) * n.x * n.x + pick(n.y, cube[2], cube[3]) * n.y * n.y + pick(n.z, cube[4], cube[5]) * n.z * n.z
}

/// The shading normal (section 3): N = normalize(T x + B y + N_v z), with
/// n_t = 2 rgb − 1 (not renormalised), or the vertex normal under
/// `$basemapalphaphongmask`.
pub fn shading_normal(t: Vec3, b: Vec3, n_v: Vec3, normal_texel: Vec3, base_alpha_mask: bool) -> Vec3 {
    let n_t = if base_alpha_mask {
        Vec3::Z
    } else {
        normal_texel * 2.0 - Vec3::ONE
    };
    (t * n_t.x + b * n_t.y + n_v * n_t.z).normalize()
}

/// The rim's ambient part: ambient(V) × boost × clamp(ρ × N_up).
pub fn rim_ambient(cube: &[Vec3; 6], v: Vec3, n: Vec3, rho: f32, boost: f32) -> Vec3 {
    ambient(cube, v) * boost * (rho * n.y).clamp(0.0, 1.0)
}

/// One pixel's inputs to `shade`.
pub struct PhongPixel<'a> {
    /// The shading normal (`shading_normal`).
    pub n: Vec3,
    /// Toward the eye.
    pub v: Vec3,
    /// Base texture (linear, after detail).
    pub base: Vec4,
    /// The normal map texel's alpha (the default specular mask).
    pub normal_alpha: f32,
    /// The exponent texture texel (white without one).
    pub exp: Vec4,
    /// The material colour (linear).
    pub modulation: Vec3,
    pub cube: [Vec3; 6],
    /// Each light's direction (toward it) and colour × attenuation.
    pub lights: &'a [(Vec3, Vec3)],
    /// The envmap sample in R (linear), with the tint and scale.
    pub env: Option<(Vec3, Vec3)>,
    /// `$selfillum`: tint and mask.
    pub selfillum: Option<(Vec3, f32)>,
    /// `$lightwarptexture` and `$phongwarptexture` lookups (u, v) → rgb.
    pub lightwarp: Option<&'a dyn Fn(f32) -> Vec3>,
    pub phong_warp: Option<&'a dyn Fn(f32, f32) -> Vec3>,
}

/// `shade`'s result, linear.
#[derive(Clone, Copy, Debug)]
pub struct PhongShaded {
    pub diffuse: Vec3,
    pub specular: Vec3,
    pub env: Vec3,
    /// diffuse + specular + env (before light scale, fog and the 8-bit write).
    pub rgb: Vec3,
}

/// The diffuse light D (section 4).
pub fn diffuse_light(m: &MapPhong, cube: &[Vec3; 6], n: Vec3, lights: &[(Vec3, Vec3)], lightwarp: Option<&dyn Fn(f32) -> Vec3>) -> Vec3 {
    let mut d = ambient(cube, n);
    for (l, c) in lights.iter().take(MAX_LIGHTS) {
        let ndl = n.dot(*l);
        let w = match (m.lightwarp, lightwarp) {
            (Some(_), Some(warp)) => LIGHTWARP_SCALE * warp(lightwarp_u(ndl)),
            _ => Vec3::splat(half_lambert(ndl)),
        };
        d += *c * w;
    }
    d
}

/// The specular mask (section 3): the normal alpha, or base alpha under
/// `$basemapalphaphongmask` (and on the cheap variant).
pub fn specular_mask(m: &MapPhong, normal_alpha: f32, base_alpha: f32) -> f32 {
    if m.base_alpha_mask || m.cheap { base_alpha } else { normal_alpha }
}

/// The envmap mask m_e (section 8).
pub fn env_mask(m: &MapPhong, base_alpha: f32, specular_mask: f32) -> f32 {
    let e = match m.env_mask {
        PhongEnvMask::BaseAlpha => base_alpha,
        PhongEnvMask::SpecularMask => specular_mask,
        PhongEnvMask::Constant(c) => c,
    };
    if m.invert_env_mask { 1.0 - e } else { e }
}

/// The envmap term: `cube` the sample in R, `tint` `$envmaptint` (raw) ×
/// envmap_scale.
pub fn env_term(m: &MapPhong, cube: Vec3, tint: Vec3, fresnel: f32, mask: f32) -> Vec3 {
    let f = 1.0 + (fresnel - 1.0) * m.envmap_fresnel;
    cube * tint * f * mask
}

/// The whole pixel (Per-pixel order, steps 4-12).
pub fn shade(m: &MapPhong, p: &PhongPixel) -> PhongShaded {
    let (n, v) = (p.n, p.v);
    let ndv = n.dot(v);
    let fresnel = fresnel_ramp(m.fresnel_ranges, ndv);
    let rim_fresnel = (1.0 - ndv).clamp(0.0, 1.0).powi(4);
    let r = reflect_view(n, v);
    let d = diffuse_light(m, &p.cube, n, p.lights, p.lightwarp);
    let mask = specular_mask(m, p.normal_alpha, p.base.w);
    let env = p.env.map_or(Vec3::ZERO, |(cube, tint)| {
        env_term(m, cube, tint, fresnel, env_mask(m, p.base.w, mask))
    });
    let k = m.exponent.0 + m.exponent.1 * p.exp.x;
    let rim = m.rim.filter(|_| !m.cheap);
    let (mut s, mut rm) = (Vec3::ZERO, Vec3::ZERO);
    for (l, c) in p.lights.iter().take(MAX_LIGHTS) {
        let (rdl, ndl) = (r.dot(*l), n.dot(*l));
        let lobe_s = lobe(rdl, ndl, k);
        let mut si = *c * lobe_s;
        if let (Some(_), Some(warp)) = (m.phong_warp, p.phong_warp) {
            si *= warp(lobe_s, fresnel);
        }
        s += si;
        if let Some(rim) = rim {
            rm += *c * lobe(rdl, ndl, rim.exponent);
        }
    }
    let masked = if m.phong_warp.is_some() { mask } else { mask * fresnel };
    s *= masked * m.boost;
    // Albedo and diffuse, self-illum.
    let base = p.base.truncate();
    let a = base * p.modulation;
    let mut diffuse = a * d;
    if let Some((tint, ms)) = p.selfillum {
        diffuse = diffuse.lerp(tint * a, ms).max(Vec3::ZERO);
    }
    if let Some(rim) = rim {
        let rim_mask = if rim.mask && m.exponent_texture.is_some() { p.exp.w } else { 1.0 };
        let rho = rim_mask * rim_fresnel;
        rm *= rho;
        s = s.max(rm);
        s += rim_ambient(&p.cube, v, n, rho, rim.boost);
    }
    let st = if m.cheap { Vec3::ONE } else { tint(m.tint, base, p.exp.y) };
    let specular = s * st;
    PhongShaded {
        diffuse,
        specular,
        env,
        rgb: specular + env + diffuse,
    }
}

/// The output alpha (section 10): `$alpha` × base.a unless self-illum,
/// blend-tint or the base-alpha phong mask use the base alpha.
pub fn alpha(m: &MapPhong, material_alpha: f32, base_alpha: f32, selfillum: bool) -> f32 {
    if selfillum || m.base_alpha_mask {
        material_alpha
    } else {
        material_alpha * base_alpha
    }
}
