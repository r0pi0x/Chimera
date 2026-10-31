//! Entities' render colour, alpha and mode (Source "rendercolor",
//! "renderamt", "rendermode"; the Color and Alpha inputs; AddOutput of
//! those keys): a brush entity node's meshes, and a character's body and
//! what it holds, draw with their material's colour multiplied by the
//! colour and, in a translucent render mode, faded or added by the
//! alpha. The logic layer writes `BrushTint` on mover nodes and
//! `RenderLook` on characters; a tinted node draws its own meshes, not
//! the merged chunks (`merge`).
//!
//! Entities' render colour, alpha and mode (`RenderLook`: Source rendercolor,
//! renderamt, rendermode; normal mode ignores the alpha, colour/texture modes
//! blend by it, glow and additive modes add, 6 and 10 hide): brush entity nodes
//! (`BrushTint`, written by the logic bridge) draw a looked copy of their
//! material, a looked node its own meshes, not the merged chunks (`merge`);
//! characters (`RenderLook` on the character, from the logic's player looks)
//! draw their body and held model faded, added or tinted
//! (`apply_character_looks`, on their `probe_lit` materials), faded further by
//! `BodyOpacity`: a spawn-protected character (`core::SpawnProtected`) draws
//! half see-through on every machine and fades back over 0.2 s when its
//! protection ends (`fade_protected`). Baked brush
//! entities' render modes and rendercolor are drawn from load
//! (`MapMesh::render`, `render_color`, `flat_color`). On brush entities
//! (`RenderLook::surface_blend`, `surface_factor`) mode 1 draws the flat
//! rendercolor, not the texture, and mode 4 cuts out at the texture's alpha
//! (`ALPHA_TEST_REFERENCE`, renderamt ignored) instead of blending; characters
//! still blend in both.
//!
//! Known shortcuts (docs/tech-debt.md): a see-through (spawn-protected) body's
//! blob shadow (`render::shadows`) stays as dark as an opaque one's (every blob
//! shares one material), and a blended body draws without sorting its own
//! triangles (its far side may show through its near side). Why it's OK for
//! now: the body reads as half there at a glance, which is what the look is for.

use bevy::prelude::*;

use super::{prop_material::PropMaterial, world_material::WorldMaterial};
use crate::map::{BrushTint, MapAlpha, RenderLook, ALPHA_TEST_REFERENCE};

/// A brush entity mesh's texture frames (its base texture's, when it has
/// several and something can pick one).
#[derive(Component, Clone, Debug)]
pub struct TextureFrames(pub Vec<Handle<Image>>);

/// A tinted mesh's own material, to go back to (white) or tint again.
#[derive(Component, Clone, Debug)]
pub struct UntintedWorld(Handle<WorldMaterial>);

#[derive(Component, Clone, Debug)]
pub struct UntintedStandard(Handle<StandardMaterial>);

/// A world material with a look applied.
fn looked_world(mut m: WorldMaterial, look: &RenderLook) -> WorldMaterial {
    m.params.base_color *= look.surface_factor();
    // Render mode 1: the flat rendercolor, not the texture.
    if look.flat_color() {
        m.base = None;
    }
    if let Some((blend, _)) = look.surface_blend() {
        m.params.translucent = blend.shader_mode();
        m.alpha_mode = blend.shader_alpha_mode();
        // Render mode 4: cut out at the texture's alpha.
        if let MapAlpha::Mask(cutoff) = blend {
            m.params.alpha_cutoff = cutoff;
        }
    }
    m
}

/// Nodes whose tint changed: their meshes get a tinted copy of their
/// material (or their own back).
#[allow(clippy::type_complexity)]
pub fn apply_brush_tints(
    nodes: Query<(&BrushTint, &Children), Changed<BrushTint>>,
    mut world_parts: Query<(
        &mut MeshMaterial3d<WorldMaterial>,
        Option<&UntintedWorld>,
        Option<&TextureFrames>,
    )>,
    mut standard_parts: Query<(&mut MeshMaterial3d<StandardMaterial>, Option<&UntintedStandard>)>,
    world_materials: Option<ResMut<Assets<WorldMaterial>>>,
    standard_materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut commands: Commands,
) {
    let (Some(mut wm), Some(mut sm)) = (world_materials, standard_materials) else {
        return;
    };
    for (tint, children) in &nodes {
        let look = tint.0;
        let f = look.surface_factor();
        for child in children.iter() {
            if let Ok((mut mat, own, frames)) = world_parts.get_mut(child) {
                let own = match own {
                    Some(o) => o.0.clone(),
                    None => {
                        commands.entity(child).insert(UntintedWorld(mat.0.clone()));
                        mat.0.clone()
                    }
                };
                if tint.is_white() {
                    mat.0 = own;
                } else if let Some(mut m) = wm.get(&own).cloned() {
                    if let Some(f) = frames.filter(|f| !f.0.is_empty()) {
                        m.base = Some(f.0[tint.1 as usize % f.0.len()].clone());
                    }
                    mat.0 = wm.add(looked_world(m, &look));
                }
            } else if let Ok((mut mat, own)) = standard_parts.get_mut(child) {
                let own = match own {
                    Some(o) => o.0.clone(),
                    None => {
                        commands.entity(child).insert(UntintedStandard(mat.0.clone()));
                        mat.0.clone()
                    }
                };
                if tint.is_white() {
                    mat.0 = own;
                } else if let Some(mut m) = sm.get(&own).cloned() {
                    let c = m.base_color.to_linear();
                    m.base_color = Color::linear_rgba(c.red * f.x, c.green * f.y, c.blue * f.z, c.alpha * f.w);
                    if look.flat_color() {
                        m.base_color_texture = None;
                    }
                    if let Some((blend, _)) = look.surface_blend() {
                        m.alpha_mode = match blend {
                            MapAlpha::Add => AlphaMode::Add,
                            MapAlpha::Mask(c) => AlphaMode::Mask(c),
                            _ => AlphaMode::Blend,
                        };
                    }
                    mat.0 = sm.add(m);
                }
            }
        }
    }
}

/// A model's own materials as they were before a look (colour, blend).
#[derive(Component, Clone, Debug)]
pub struct UnlookedModel {
    base: Vec<(Vec4, f32, AlphaMode)>,
    applied: RenderLook,
}

/// How opaque a character's body draws (1: as its look says), times its
/// `RenderLook`'s alpha (`apply_character_looks`); spawn protection's
/// see-through (`fade_protected`).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct BodyOpacity(pub f32);

/// A spawn-protected body's opacity (`core::SpawnProtected`, user
/// 2026-10-29: others see their shots won't land).
pub const PROTECTED_OPACITY: f32 = 0.5;
/// Seconds a body takes to come back to opaque when its protection ends.
pub const UNPROTECT_FADE_SECONDS: f32 = 0.2;

/// Spawn-protected characters (`core::SpawnProtected`, on every machine:
/// a network client's from `NetProtected`) draw see-through
/// (`PROTECTED_OPACITY`) while either window runs, at once when it
/// starts, fading back to opaque over `UNPROTECT_FADE_SECONDS` when it
/// ends (`BodyOpacity`).
#[allow(clippy::type_complexity)]
pub fn fade_protected(
    characters: Query<
        (Entity, Option<&crate::core::SpawnProtected>, Option<&mut BodyOpacity>),
        Or<(With<crate::core::SpawnProtected>, With<BodyOpacity>)>,
    >,
    clock: Option<Res<crate::core::SimClock>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let now = clock.map_or(0.0, |c| c.now);
    let step = time.delta_secs() * (1.0 - PROTECTED_OPACITY) / UNPROTECT_FADE_SECONDS;
    for (e, protected, opacity) in characters {
        let target = if protected.is_some_and(|p| !p.over(now)) { PROTECTED_OPACITY } else { 1.0 };
        match opacity {
            Some(mut o) => {
                let next = if target < o.0 { target } else { (o.0 + step).min(target) };
                if next != o.0 {
                    o.0 = next;
                }
            }
            None => {
                commands.entity(e).insert(BodyOpacity(target));
            }
        }
    }
}

/// A character's look as drawn: its `RenderLook` (none: plain) faded by
/// its `BodyOpacity` (a normal render mode becomes a blended one).
pub fn drawn_look(look: Option<&RenderLook>, opacity: Option<&BodyOpacity>) -> RenderLook {
    let mut l = look.copied().unwrap_or_default();
    if let Some(o) = opacity.filter(|o| o.0 < 1.0) {
        if l.mode == crate::map::RENDER_NORMAL {
            l.mode = crate::map::RENDER_TRANS_ALPHA;
            l.alpha = 255;
        }
        l.alpha = (l.alpha as f32 * o.0.clamp(0.0, 1.0)).round() as u8;
    }
    l
}

/// Characters' looks (`RenderLook` on the character, faded by its
/// `BodyOpacity`: `drawn_look`): the body and the model it holds (each
/// `probe_lit::ProbeLit` among its descendants, with its own material
/// instances) take the colour and fade or add by the alpha; restored when
/// plain again.
#[allow(clippy::type_complexity)]
pub fn apply_character_looks(
    characters: Query<(Entity, Option<&RenderLook>, Option<&BodyOpacity>), Or<(With<RenderLook>, With<BodyOpacity>)>>,
    children: Query<&Children>,
    models: Query<(&crate::render::probe_lit::ProbeLit, Option<&UnlookedModel>)>,
    materials: Option<ResMut<Assets<PropMaterial>>>,
    mut commands: Commands,
) {
    let Some(mut materials) = materials else { return };
    for (character, look, opacity) in &characters {
        let look = &drawn_look(look, opacity);
        for model in children.iter_descendants(character) {
            let Ok((lit, unlooked)) = models.get(model) else {
                continue;
            };
            match unlooked {
                Some(u) if u.applied == *look => continue,
                None if look.is_plain() => continue,
                _ => {}
            }
            let base: Vec<(Vec4, f32, AlphaMode)> = match unlooked {
                Some(u) => u.base.clone(),
                None => lit
                    .materials
                    .iter()
                    .map(|h| {
                        materials.get(h).map_or((Vec4::ONE, 0.0, AlphaMode::Opaque), |m| {
                            (m.params.base_color, m.params.translucent, m.alpha_mode)
                        })
                    })
                    .collect(),
            };
            let f = look.factor();
            for (h, (color, translucent, mode)) in lit.materials.iter().zip(&base) {
                let Some(mut m) = materials.get_mut(h) else { continue };
                m.params.base_color = *color * f;
                match look.blend() {
                    Some((blend, _)) => {
                        m.params.translucent = blend.shader_mode();
                        m.alpha_mode = blend.shader_alpha_mode();
                    }
                    None => {
                        m.params.translucent = *translucent;
                        m.alpha_mode = *mode;
                    }
                }
            }
            commands.entity(model).insert(UnlookedModel { base, applied: *look });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_modes() {
        let look = |mode, alpha| RenderLook {
            color: [255; 3],
            alpha,
            mode,
        };
        // Normal mode ignores renderamt.
        assert!(look(0, 0).is_plain());
        assert_eq!(look(1, 100).blend(), Some((MapAlpha::Blend, 100.0 / 255.0)));
        assert!(look(1, 255).is_plain());
        assert_eq!(look(5, 255).blend(), Some((MapAlpha::Add, 1.0)));
        assert!(look(10, 255).invisible());
        assert!(look(1, 0).invisible());
        // Added: the opacity scales the colour.
        let f = look(5, 51).factor();
        assert!((f.x - 0.2).abs() < 1e-5 && f.w == 1.0);
        let red = RenderLook {
            color: [255, 0, 0],
            alpha: 255,
            mode: 0,
        };
        assert!(!red.is_plain());
        assert_eq!(red.factor(), Vec4::new(1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn brush_surfaces_in_modes_1_and_4() {
        let look = |mode, alpha| RenderLook {
            color: [255, 0, 0],
            alpha,
            mode,
        };
        // Mode 1: flat colour, with the alpha blended, even at full alpha.
        assert!(look(1, 255).flat_color() && look(1, 255).changes_surface());
        assert!(!look(0, 255).flat_color());
        assert_eq!(look(1, 255).surface_blend(), None);
        assert_eq!(look(1, 51).surface_blend(), Some((MapAlpha::Blend, 0.2)));
        // Mode 4: alpha test, renderamt ignored, not hidden unless 0.
        let cut = look(4, 100);
        assert_eq!(cut.surface_blend(), Some((MapAlpha::Mask(ALPHA_TEST_REFERENCE), 1.0)));
        assert_eq!(cut.surface_factor().w, 1.0);
        assert!(cut.changes_surface());
        assert!(look(4, 0).invisible());
        // Characters keep blending in mode 4.
        assert_eq!(cut.blend(), Some((MapAlpha::Blend, 100.0 / 255.0)));
        assert!(!crate::map::BrushTint(look(1, 255), 0).is_white());
        assert!(crate::map::BrushTint(RenderLook::default(), 0).is_white());
    }

    #[test]
    fn looked_world_flattens_and_cuts_out() {
        let textured = WorldMaterial {
            params: crate::render::world_material::WorldParams {
                base_color: Vec4::ONE,
                ..default()
            },
            base: Some(Handle::default()),
            normal: None,
            lightmap: None,
            lightmap_b0: None,
            lightmap_b1: None,
            lightmap_b2: None,
            base2: None,
            normal2: None,
            blend_mask: None,
            detail: None,
            envmap: None,
            envmap_mask: None,
            flat_envmap: None,
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
            decal: false,
            modulate: false,
            skin_frames: Vec::new(),
            skin_frame_ms: 0,
        };
        let flat = looked_world(
            textured.clone(),
            &RenderLook {
                color: [255, 0, 0],
                alpha: 255,
                mode: 1,
            },
        );
        assert!(flat.base.is_none());
        assert_eq!(flat.params.base_color.y, 0.0);
        let cut = looked_world(
            textured,
            &RenderLook {
                color: [255; 3],
                alpha: 128,
                mode: 4,
            },
        );
        assert!(cut.base.is_some());
        assert_eq!(cut.params.alpha_cutoff, ALPHA_TEST_REFERENCE);
        assert_eq!(cut.params.translucent, 0.0);
        assert_eq!(cut.params.base_color.w, 1.0);
    }
}
