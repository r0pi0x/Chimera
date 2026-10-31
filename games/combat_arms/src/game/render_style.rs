//! Render styles (specs/combat_arms/models_ltb.md "Textures and render
//! styles", characters.md 2, view_models.md 2): a model piece names a
//! render-style index, which picks a render-style file (`RS\*.LTB`) from
//! the list its object was given (a body record's `RenderStyle#`, a
//! weapon's `PVRenderStyle#` / `HHRenderStyle#`, a head piece's or
//! face's); a missing entry is the engine's default style. How each file
//! draws is our data (data/render_styles.toml, by file name: opaque,
//! blended by the texture's alpha, added, glowing, alpha-tested cut out,
//! or layered, as models_ltb.md "Render styles and sprite skins" reads
//! the files), applied to the piece's mesh (`apply`). A layered style
//! (`Layer`: `env_alpha_mask`, `itemedge`, `edgeamp01`, `envmap`,
//! `pv_char`) draws the piece's second texture slot (`apply_with` names
//! it; `world::model_textures` resolves it) as an environment or rim map
//! on generated coordinates (Chimera's `map::MapLayer`), the stock
//! styles' formulas over the lit colour L of the style file's own
//! lighting material (`Layer::lighting`: M_e + M_a x A + M_d x the lights'
//! sum, on a probe-lit model; half our light where there is no probe),
//! with the material's specular added after the texture stages;
//! `pv_char_b`'s three passes are one combine (`pv_char_b`). A second slot
//! a piece names without a skin reads white (`WHITE_SLOT`); a worn piece's
//! slots follow its kind's rules (`piece_skin`: faces, head pieces,
//! goggles, backpacks, flags).
//!
//! Known shortcuts: the files aren't read at run time (the format is
//! `lithtech_formats::render_style`; our table is a summary, checked
//! against the install in `heavy::render_styles`), so a style not in it
//! draws opaque, as the default style; `pv_glass` draws blended, not as
//! its own lit pass; the materials' specular sums only the probe's four
//! directional lights, not the scene's point lights; head pieces' clan
//! slots (7, 8) and the figure-type skin choice aren't modelled.

use std::{collections::HashMap, sync::OnceLock};

use serde::Deserialize;

use bevy::prelude::{Vec3, default};

use crate::map::{LayerAddress, LayerCombine, LayerCoords, LayerLighting, MapAlpha, MapLayer, MapMesh};

/// How a render style draws a piece.
#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// The engine's default: opaque (alpha-tested where the texture says).
    #[default]
    Opaque,
    /// Blended by the texture's alpha (glass, the transparent mark).
    Blend,
    /// Added to what's behind (light effects).
    Add,
    /// At its texture's own brightness, ignoring the lighting.
    Glow,
    /// Alpha-tested, not blended (models_ltb.md "Render styles and sprite
    /// skins"): drawn opaque where the texture's alpha passes the test
    /// against `reference`/255 (≥, or > when `strict`), discarded
    /// elsewhere; `unlit`: the texture's colour alone; `two_sided`: no
    /// culling.
    Cutout {
        reference: u8,
        #[serde(default)]
        strict: bool,
        #[serde(default)]
        unlit: bool,
        #[serde(default)]
        two_sided: bool,
    },
    /// The piece's second texture slot as an environment or rim map on
    /// generated coordinates (models_ltb.md "Stock styles"; `Layer`),
    /// alpha-tested (kept where alpha > `cutout`/255) when set.
    Layered {
        layer: Layer,
        #[serde(default)]
        cutout: Option<u8>,
    },
}

/// The stock styles that draw slot 1 (models_ltb.md "Stock styles"), T
/// the colour map, S slot 1's sample, L the lit colour.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    /// `env_alpha_mask`: T + T.a x S x L, S at the camera-space
    /// reflection (clamped); T unlit.
    EnvAlphaMask,
    /// `itemedge`: T x L + 3 S, S at the camera-space normal (mirrored).
    ItemEdge,
    /// `edgeamp01`: as `itemedge`, S added once.
    EdgeAmp,
    /// `envmap`: 2 L x lerp(T, S, T.a), S at the camera-space reflection
    /// as a sphere map.
    Envmap,
    /// `pv_char`: (T x L + T.a x S) x (1 + T.a^2), S at the world-space
    /// reflection (clamped).
    PvChar,
    /// `pv_char_b`: three passes in one (`pv_char_b`), S at the
    /// world-space normal (clamped).
    PvCharB,
}

/// The game's lit colour L as a fraction of ours: its stock style draws
/// 2 x T x L where we draw T x our light.
const GAME_LIGHT: f32 = 0.5;

impl Layer {
    /// How the layer draws (its texture still to be named and resolved).
    pub fn map_layer(self) -> MapLayer {
        let (coords, address, combine, factor) = match self {
            Layer::EnvAlphaMask => (LayerCoords::CameraReflection, LayerAddress::Clamp, LayerCombine::MaskedAdd, 1.0),
            Layer::ItemEdge => (LayerCoords::CameraNormal, LayerAddress::Mirror, LayerCombine::Add, 3.0),
            Layer::EdgeAmp => (LayerCoords::CameraNormal, LayerAddress::Mirror, LayerCombine::Add, 1.0),
            Layer::Envmap => (LayerCoords::SphereMap, LayerAddress::Clamp, LayerCombine::AlphaLerp, 2.0),
            Layer::PvChar => (LayerCoords::WorldReflection, LayerAddress::Clamp, LayerCombine::Gloss, 1.0),
            Layer::PvCharB => (LayerCoords::WorldNormal, LayerAddress::Clamp, LayerCombine::PvCharB, 1.0),
        };
        MapLayer {
            coords,
            address,
            combine,
            factor,
            light: GAME_LIGHT,
            lighting: Some(self.lighting()),
            ..default()
        }
    }

    /// The style file's lighting material (models_ltb.md Follow-up
    /// (2026-10-31) 3), for the pass that is lit (`envmap`'s and
    /// `pv_char_b`'s first).
    pub fn lighting(self) -> LayerLighting {
        match self {
            Layer::EnvAlphaMask => lighting(0.408, 0.400, 0.369, 1.0, [0.0, 0.0, 0.251], [0.278, 0.275, 0.255], 20.0),
            Layer::ItemEdge | Layer::EdgeAmp => lighting(0.408, 0.400, 0.369, 1.0, [0.863, 0.859, 0.847], [0.278, 0.275, 0.255], 20.0),
            Layer::Envmap => lighting(0.0, 0.0, 0.0, 0.537, [1.0; 3], [0.0; 3], 20.0),
            Layer::PvChar => lighting(0.416, 0.416, 0.416, 1.0, [0.8; 3], [0.353, 0.349, 0.271], 20.0),
            Layer::PvCharB => lighting(0.416, 0.416, 0.416, 0.333, [0.875; 3], [0.443; 3], 50.0),
        }
    }
}

/// A material from the table of the spec: emissive (r, g, b), ambient (one
/// value), diffuse, specular and its power.
fn lighting(er: f32, eg: f32, eb: f32, ambient: f32, diffuse: [f32; 3], specular: [f32; 3], power: f32) -> LayerLighting {
    LayerLighting {
        emissive: Vec3::new(er, eg, eb),
        ambient: Vec3::splat(ambient),
        diffuse: Vec3::from(diffuse),
        specular: Vec3::from(specular),
        power,
    }
}

/// `default`'s material (models_ltb.md R2/R8): the style the sprite-skinned
/// guns and most pieces name; drawn lit by our light, so only the tests
/// of the formula read it.
pub fn default_lighting() -> LayerLighting {
    lighting(0.275, 0.275, 0.275, 0.294, [0.761; 3], [0.392; 3], 10.0)
}

/// `pv_char_b`'s three passes evaluated for one texel (models_ltb.md
/// Follow-up (2026-10-31) 4; the prop shader's `PvCharB` combine mirrors
/// it): `t` the colour map T1 with alpha `ta`, `s` the second slot read at
/// the world-space normal, `l` the lit colour L, `spec` the specular
/// added after pass 1. Every stage is clamped to 0..1.
pub fn pv_char_b(t: Vec3, ta: f32, s: Vec3, l: Vec3, spec: Vec3) -> Vec3 {
    const F: f32 = 0.502;
    let c01 = |v: Vec3| v.clamp(Vec3::ZERO, Vec3::ONE);
    // Pass 1: c = T1 F.a + F (1 - F.a), alpha T1.a^2; blended toward L by
    // that alpha; a dot3 of the result with itself (a grey).
    let c = c01(t * F + Vec3::splat(F * (1.0 - F)));
    let a = (ta * ta).clamp(0.0, 1.0);
    let m = c01(c * a + l * (1.0 - a));
    let d = (4.0 * (m - Vec3::splat(0.5)).length_squared()).clamp(0.0, 1.0);
    // Pass 2, added: (F T1)^2 + T1.a F.
    let p2 = c01((t * F) * (t * F) + Vec3::splat(F * ta));
    // Pass 3, multiplied: s0 = 2 T1; s1 = s0 + (1 - T1.a) s0; a1 = 4 T1.a
    // - 1; s2 = s1 + (1 - a1) S.
    let s0 = c01(t * 2.0);
    let s1 = c01(s0 + s0 * (1.0 - ta));
    let a1 = (4.0 * ta - 1.0).clamp(0.0, 1.0);
    let s2 = c01(s1 + s * (1.0 - a1));
    c01(Vec3::splat(d) + spec + p2) * s2
}

impl Style {
    /// Whether a texel of alpha `alpha` (0..255) is drawn.
    pub fn keeps(self, alpha: u8) -> bool {
        match self {
            Style::Cutout { reference, strict: false, .. } => alpha >= reference,
            Style::Cutout { reference, strict: true, .. } => alpha > reference,
            Style::Layered { cutout: Some(r), .. } => alpha > r,
            _ => alpha > 0,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    styles: HashMap<String, Style>,
}

fn key(name: &str) -> String {
    name.trim().replace('\\', "/").to_lowercase()
}

fn table() -> &'static HashMap<String, Style> {
    static T: OnceLock<HashMap<String, Style>> = OnceLock::new();
    T.get_or_init(|| {
        let t: Table = toml::from_str(include_str!("../../data/render_styles.toml")).expect("data/render_styles.toml");
        t.styles.into_iter().map(|(k, v)| (key(&k), v)).collect()
    })
}

/// How render-style file `name` draws (an unknown or empty name: the
/// default, opaque).
pub fn style(name: &str) -> Style {
    table().get(&key(name)).copied().unwrap_or_default()
}

/// The style of index `index` in an object's render-style list (past
/// the list or negative: the default).
pub fn of_list(list: &[String], index: i32) -> Style {
    usize::try_from(index)
        .ok()
        .and_then(|i| list.get(i))
        .map_or(Style::Opaque, |n| style(n))
}

/// Draw `mesh` in `style`.
pub fn apply(mesh: &mut MapMesh, style: Style) {
    match style {
        Style::Opaque => {}
        Style::Blend => mesh.alpha = MapAlpha::Blend,
        Style::Add => {
            mesh.alpha = MapAlpha::Add;
            mesh.unlit = true;
        }
        Style::Glow => mesh.unlit = true,
        Style::Cutout { reference, strict, unlit, two_sided } => {
            // The cut between the passing and failing texel values.
            let cut = if strict { f32::from(reference) + 0.5 } else { f32::from(reference) - 0.5 };
            mesh.alpha = MapAlpha::Mask(cut / 255.0);
            mesh.unlit |= unlit;
            mesh.double_sided |= two_sided;
        }
        Style::Layered { layer, cutout } => {
            let material = mesh.layer.take().map(|l| l.material).unwrap_or_default();
            mesh.layer = Some(MapLayer {
                material,
                ..layer.map_layer()
            });
            if let Some(r) = cutout {
                mesh.alpha = MapAlpha::Mask((f32::from(r) + 0.5) / 255.0);
            }
        }
    }
}

/// The name of a second slot that the piece binds but the record gives no
/// skin: the stage reads opaque white (models_ltb.md Follow-up (2026-10-31)
/// 6), `world::model_textures` supplies the white texture.
pub const WHITE_SLOT: &str = "<white>";

/// `apply`, a layered style's slot-1 texture being `skins[second]` (the
/// piece's second texture index; none, -1: the layer draws nothing; a slot
/// it names without a skin: white).
pub fn apply_with(mesh: &mut MapMesh, style: Style, skins: &[String], second: i32) {
    apply(mesh, style);
    if let Some(layer) = mesh.layer.as_mut().filter(|_| matches!(style, Style::Layered { .. })) {
        layer.material = match usize::try_from(second) {
            Ok(i) => skins.get(i).filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| WHITE_SLOT.to_string()),
            Err(_) => String::new(),
        };
    }
}

/// Which kind of worn piece a record's skin list belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    Face,
    Head,
    Goggle,
    Backpack,
    Flag,
}

/// The skin in slot `slot` of a piece whose record lists `skins`, at
/// camouflage index `c` (models_ltb.md Follow-up (2026-10-31) 5): a face's
/// whole list fills the slots in order; a head piece fills slot 0 only,
/// `Skin{c}` (out of range: `Skin0`), never slot 1; goggles copy the list
/// from `c` on (slot k = `Skin{c+k}`, `c` past the list: 0); backpacks and
/// flags fill slot k with `Skin{k}`. None: the slot has no skin.
pub fn piece_skin(piece: Piece, skins: &[String], slot: i32, c: usize) -> Option<&str> {
    let slot = usize::try_from(slot).ok()?;
    let from = |i: usize| skins.get(i).map(String::as_str).filter(|s| !s.is_empty());
    match piece {
        Piece::Face | Piece::Backpack | Piece::Flag => from(slot),
        Piece::Head => (slot == 0).then(|| from(if c < skins.len() { c } else { 0 })).flatten(),
        Piece::Goggle => from(if c < skins.len() { c } else { 0 } + slot),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_body_styles() {
        assert_eq!(style("RS\\default.ltb"), Style::Opaque);
        let mark = Style::Cutout { reference: 160, strict: false, unlit: true, two_sided: false };
        assert_eq!(style("RS\\MARKTRANSPARENT.LTB"), mark);
        assert_eq!(style("rs/marktransparent.ltb"), mark);
        assert_eq!(style("RS\\nothing_like_it.ltb"), Style::Opaque);
        let list = ["RS\\default.ltb".to_string(), "RS\\GLOW.LTB".into(), "RS\\MARKTRANSPARENT.LTB".into()];
        assert_eq!(of_list(&list, 2), mark);
        assert_eq!(of_list(&list, 3), Style::Opaque);
        assert_eq!(of_list(&list, -1), Style::Opaque);
        let mut m = MapMesh::default();
        apply(&mut m, Style::Blend);
        assert_eq!(m.alpha, MapAlpha::Blend);
        // models_ltb.md R1: alpha 159 discarded, 160 drawn, unlit.
        assert!(!mark.keeps(159) && mark.keeps(160));
        let mut m = MapMesh::default();
        apply(&mut m, mark);
        assert!(matches!(m.alpha, MapAlpha::Mask(c) if 159.0 / 255.0 < c && c < 160.0 / 255.0) && m.unlit);
    }

    /// models_ltb.md "Stock styles" and R3: `itemedge` adds slot 1 at the
    /// camera-space normal (mirrored) 3 times, alpha-tested > 160.
    #[test]
    fn layered_styles_draw_slot_one() {
        let edge = style("RS\\ItemEdge.ltb");
        assert_eq!(edge, Style::Layered { layer: Layer::ItemEdge, cutout: Some(160) });
        assert!(!edge.keeps(160) && edge.keeps(161));
        let skins = ["T\\GUN.DTX".to_string(), "TEXTURES\\128XPV_SPECIALSPECR.DTX".into()];
        let mut m = MapMesh::default();
        apply_with(&mut m, edge, &skins, 1);
        let l = m.layer.clone().unwrap();
        assert_eq!(l.material, skins[1]);
        assert_eq!((l.coords, l.address, l.combine, l.factor), (LayerCoords::CameraNormal, LayerAddress::Mirror, LayerCombine::Add, 3.0));
        assert!(matches!(m.alpha, MapAlpha::Mask(c) if 160.0 / 255.0 < c && c < 161.0 / 255.0));
        // R4: envmap's second pass: a sphere map, lerped by T.a, x 2L.
        let env = style("RS\\envmap.ltb").clone();
        let mut m = MapMesh::default();
        apply_with(&mut m, env, &skins, 1);
        let l = m.layer.unwrap();
        assert_eq!((l.coords, l.combine, l.factor, l.light), (LayerCoords::SphereMap, LayerCombine::AlphaLerp, 2.0, 0.5));
        for (name, layer) in [("RS\\ENV_ALPHA_MASK.ltb", Layer::EnvAlphaMask), ("RS\\PV_Char.ltb", Layer::PvChar), ("RS\\EdgeAmp01.ltb", Layer::EdgeAmp)] {
            assert!(matches!(style(name), Style::Layered { layer: l, .. } if l == layer), "{name}");
        }
        // A single-texture style draws no second slot, named or not.
        let mut m = MapMesh::default();
        apply_with(&mut m, style("RS\\default.ltb"), &skins, 1);
        assert!(m.layer.is_none());
        // A layered piece with one slot names nothing (the layer stays undrawn).
        let mut m = MapMesh::default();
        apply_with(&mut m, edge, &skins[..1], -1);
        assert!(m.layer.unwrap().material.is_empty());
    }

    /// models_ltb.md R7: pv_char_b's three passes. T1 = (0.6, 0.4, 0.2),
    /// T1.a 0.8, L 0.5, T2 0.5, no specular: pass 1's grey d = 0.045, pass
    /// 2 adds (0.492, 0.442, 0.412), pass 3 multiplies by (1, 0.96, 0.48):
    /// (0.537, 0.467, 0.219). With T1.a 0.3: d 0.001, pass 2 (0.241, 0.191,
    /// 0.161), factor (1, 1, 1): (0.242, 0.192, 0.162).
    #[test]
    fn r7_pv_char_b_passes() {
        let t = Vec3::new(0.6, 0.4, 0.2);
        let out = pv_char_b(t, 0.8, Vec3::splat(0.5), Vec3::splat(0.5), Vec3::ZERO);
        let want = Vec3::new(0.537, 0.467, 0.219);
        assert!(out.distance(want) < 2e-3, "{out} vs {want}");
        let out = pv_char_b(t, 0.3, Vec3::splat(0.5), Vec3::splat(0.5), Vec3::ZERO);
        let want = Vec3::new(0.242, 0.192, 0.162);
        assert!(out.distance(want) < 2e-3, "{out} vs {want}");
        // The specular goes in before pass 3's multiply: after the stages
        // of pass 1, not on top of the result.
        let lit = pv_char_b(t, 0.8, Vec3::splat(0.5), Vec3::splat(0.5), Vec3::splat(0.1));
        assert!((lit.x - 0.637).abs() < 2e-3 && (lit.y - 0.563).abs() < 2e-3, "{lit}");
    }

    /// R8: `default` with A = 0.2 and one white light at N.D = 0.5:
    /// L = 0.275 + 0.059 + 0.381 = 0.714; `env_alpha_mask` with the same:
    /// (0.608, 0.600, 0.694); the specular of a lit material is added after
    /// the stages and is black for a black specular colour.
    #[test]
    fn r8_lighting_materials() {
        let (a, sigma) = (Vec3::splat(0.2), Vec3::splat(0.5));
        let l = default_lighting().lit(a, sigma);
        assert!(l.distance(Vec3::splat(0.714)) < 1e-3, "{l}");
        let l = Layer::EnvAlphaMask.lighting().lit(a, sigma);
        assert!(l.distance(Vec3::new(0.608, 0.600, 0.694)) < 1e-3, "{l}");
        // pv_char: 0.416 + A + 0.8 sigma.
        let l = Layer::PvChar.lighting().lit(Vec3::splat(0.1), Vec3::splat(0.2));
        assert!(l.distance(Vec3::splat(0.416 + 0.1 + 0.16)) < 1e-3, "{l}");
        assert_eq!(Layer::PvChar.lighting().lit(a, sigma), Vec3::ONE, "0.416 + 0.2 + 0.4 clamps");
        // Clamped.
        assert_eq!(Layer::ItemEdge.lighting().lit(Vec3::ONE, Vec3::ONE), Vec3::ONE);
        // Specular: M_s x C x (N.H)^p; the half vector between a light and
        // the eye at the same direction is that direction.
        let n = Vec3::Y;
        let lights = [(Vec3::ONE, Vec3::Y)];
        let spec = Layer::PvChar.lighting().specular_of(n, Vec3::Y, &lights);
        assert!(spec.distance(Vec3::new(0.353, 0.349, 0.271)) < 1e-4, "{spec}");
        assert_eq!(Layer::Envmap.lighting().specular_of(n, Vec3::Y, &lights), Vec3::ZERO, "black specular: none");
        // Lit materials and our styles: every layered stock style has one.
        for name in ["RS\\ENV_ALPHA_MASK.ltb", "RS\\ItemEdge.ltb", "RS\\EdgeAmp01.ltb", "RS\\envmap.ltb", "RS\\PV_Char.ltb", "RS\\PV_Char_B.ltb"] {
            let Style::Layered { layer, .. } = style(name) else { panic!("{name} is layered") };
            assert!(layer.map_layer().lighting.is_some(), "{name}");
        }
    }

    /// R9 (logic): a goggle with `Skin0..2` = a, b, c at camouflage 1 fills
    /// slots 0, 1 with b, c; at camouflage 5 a, b, c. A head piece fills
    /// slot 0 with `Skin{c}` and never slot 1 (`Hair331`, camouflage 1: its
    /// `Skin1`, slot 1 empty). A face's list fills the slots in order,
    /// backpacks and flags too.
    #[test]
    fn r9_piece_skins() {
        let list: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        let g = |slot, c| piece_skin(Piece::Goggle, &list, slot, c);
        assert_eq!((g(0, 1), g(1, 1), g(2, 1)), (Some("b"), Some("c"), None));
        assert_eq!((g(0, 5), g(1, 5), g(2, 5)), (Some("a"), Some("b"), Some("c")));
        let h = |slot, c| piece_skin(Piece::Head, &list, slot, c);
        assert_eq!((h(0, 1), h(1, 1), h(2, 1)), (Some("b"), None, None));
        assert_eq!(h(0, 9), Some("a"), "out of range: Skin0");
        for piece in [Piece::Face, Piece::Backpack, Piece::Flag] {
            let at = |slot| piece_skin(piece, &list, slot, 2);
            assert_eq!((at(0), at(1), at(2), at(3), at(-1)), (Some("a"), Some("b"), Some("c"), None, None), "{piece:?}: not by camouflage");
        }
    }

    /// R9/item 6: a second slot a piece names without a skin reads white;
    /// no second slot at all stays undrawn.
    #[test]
    fn an_empty_second_slot_is_white() {
        let skins = ["T\\GUN.DTX".to_string()];
        let pv = style("RS\\PV_Char.ltb");
        let mut m = MapMesh::default();
        apply_with(&mut m, pv, &skins, 1);
        assert_eq!(m.layer.unwrap().material, WHITE_SLOT);
        let mut m = MapMesh::default();
        apply_with(&mut m, pv, &skins, -1);
        assert!(m.layer.unwrap().material.is_empty());
        let mut m = MapMesh::default();
        apply_with(&mut m, pv, &["T\\GUN.DTX".to_string(), String::new()], 1);
        assert_eq!(m.layer.unwrap().material, WHITE_SLOT, "a listed but empty skin");
    }
}
