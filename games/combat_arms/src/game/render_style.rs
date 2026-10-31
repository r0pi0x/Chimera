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
//! styles' formulas with the game's lit colour taken as half our light
//! (its default 2 x T x L is our T x light).
//!
//! Known shortcuts: the files aren't read (their format is in
//! models_ltb.md now); the table is our summary of them, so a style
//! not in it draws opaque, as the default style; `pv_char_b`'s three
//! passes aren't drawn (opaque); the layered styles' materials (ambient,
//! diffuse, emissive, specular) aren't applied, only the formulas over our
//! light; a face's, head piece's or attached part's slot 1 isn't named
//! (only weapons' view and held models and body records' pieces).

use std::{collections::HashMap, sync::OnceLock};

use serde::Deserialize;

use bevy::prelude::default;

use crate::map::{LayerAddress, LayerCombine, LayerCoords, MapAlpha, MapLayer, MapMesh};

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
        };
        MapLayer {
            coords,
            address,
            combine,
            factor,
            light: GAME_LIGHT,
            ..default()
        }
    }
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

/// `apply`, a layered style's slot-1 texture being `skins[second]` (the
/// piece's second texture index; none: the layer draws nothing).
pub fn apply_with(mesh: &mut MapMesh, style: Style, skins: &[String], second: i32) {
    apply(mesh, style);
    if let Some(layer) = mesh.layer.as_mut().filter(|_| matches!(style, Style::Layered { .. })) {
        layer.material = usize::try_from(second).ok().and_then(|i| skins.get(i)).cloned().unwrap_or_default();
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
}
