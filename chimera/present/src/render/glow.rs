//! Glow sprites' visibility (`map::GlowSprite`: spotlight halos, light
//! glows, glow-mode sprites): each frame, faded by how much of its
//! occlusion proxy the eye sees (`glow_visibility`: line tests standing in
//! for the game's occlusion queries).

use avian3d::prelude::*;
use bevy::prelude::*;

use super::sprite_material::SpriteMaterial;
use crate::map::{GlowSprite, LogicHidden, SkyboxCamera, ViewModelCamera, VisClusters, monitor, water};

/// Glow visibility (specs/cs_source/sprites_dust.md, 5a): the game measures
/// the visible fraction of a view-facing square (half-diagonal = proxy
/// size) pulled the proxy size toward the eye, with occlusion queries. We
/// approximate it with lines to the square's centre and corners, and fade
/// the glow by the fraction that get through. Static props don't occlude.
#[allow(clippy::type_complexity)]
pub fn glow_visibility(
    query: SpatialQuery,
    cameras: Query<
        (&GlobalTransform, &Camera),
        (
            With<Camera3d>,
            Without<monitor::ScreenCamera>,
            Without<SkyboxCamera>,
            Without<ViewModelCamera>,
            Without<water::WaterReflectionCamera>,
        ),
    >,
    ignored: Query<Entity, With<crate::core::Intent>>,
    mut glows: Query<
        (
            &GlobalTransform,
            &GlowSprite,
            &MeshMaterial3d<SpriteMaterial>,
            &mut Visibility,
            Option<&VisClusters>,
        ),
        Without<LogicHidden>,
    >,
    materials: Option<ResMut<Assets<SpriteMaterial>>>,
) {
    let (Some((eye, camera)), Some(mut materials)) = (cameras.iter().next(), materials) else {
        return;
    };
    let filter = SpatialQueryFilter::from_excluded_entities(ignored.iter()).with_mask(crate::core::NOT_SHADOW);
    let from = eye.translation();
    let (right, up) = (eye.right().as_vec3(), eye.up().as_vec3());
    for (glow, sprite, material, mut visibility, clusters) in &mut glows {
        // Not potentially visible: hidden, no occlusion tests.
        if clusters.is_some_and(|c| !c.potentially_visible) {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }
        let p = glow.translation();
        let centre = p + (from - p).normalize_or_zero() * sprite.proxy;
        let r = sprite.proxy / std::f32::consts::SQRT_2;
        let points = [
            centre,
            centre + (right + up) * r,
            centre + (right - up) * r,
            centre - (right + up) * r,
            centre - (right - up) * r,
        ];
        // The fraction of the lines that get through, times the fraction
        // of the proxy's screen rectangle inside the viewport (0 if any
        // corner is behind the eye), as the game's occlusion queries are
        // clipped to the screen. Off the screen nothing is seen whatever
        // the lines say: they aren't cast (most of a map's glows in the
        // potentially visible set are behind or beside the eye).
        let seen = seen_fraction(clip_fraction(camera, eye, &points[1..]), || {
            points
                .iter()
                .filter(|to| {
                    Dir3::new(**to - from).map_or(true, |dir| {
                        query.cast_ray(from, dir, from.distance(**to), true, &filter).is_none()
                    })
                })
                .count() as f32
                / points.len() as f32
        });
        visibility.set_if_neq(if seen > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        // Only touch the material when the colour changes: a mutable
        // borrow alone re-prepares it for the GPU.
        let color = (sprite.color.truncate() * seen).extend(sprite.color.w);
        if materials.get(&material.0).is_some_and(|m| m.params.color != color)
            && let Some(mut m) = materials.get_mut(&material.0)
        {
            m.params.color = color;
        }
    }
}

/// `lines() * clip`: the lines' fraction asked only where the proxy is on
/// the screen at all (`clip` not 0: the product is 0 then anyway).
fn seen_fraction(clip: f32, lines: impl FnOnce() -> f32) -> f32 {
    if clip == 0.0 { 0.0 } else { lines() * clip }
}

/// How much of the screen rectangle around `corners` lies in the viewport.
fn clip_fraction(camera: &Camera, eye: &GlobalTransform, corners: &[Vec3]) -> f32 {
    let mut lo = Vec2::splat(f32::MAX);
    let mut hi = Vec2::splat(f32::MIN);
    for c in corners {
        let Some(ndc) = camera.world_to_ndc(eye, *c) else {
            return 0.0;
        };
        if ndc.z < 0.0 || ndc.z > 1.0 {
            return 0.0;
        }
        lo = lo.min(ndc.truncate());
        hi = hi.max(ndc.truncate());
    }
    let area = (hi - lo).max(Vec2::splat(1e-6));
    let inside = (hi.min(Vec2::ONE) - lo.max(Vec2::NEG_ONE)).max(Vec2::ZERO);
    (inside.x * inside.y / (area.x * area.y)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Off the screen the lines aren't cast, and the result is what the
    /// product gives everywhere.
    #[test]
    fn the_lines_are_cast_only_on_screen() {
        for clip in [0.0f32, -0.0, 1e-6, 0.25, 1.0, f32::NAN] {
            for lines in [0.0f32, 0.2, 0.6, 1.0] {
                let mut cast = false;
                let got = seen_fraction(clip, || {
                    cast = true;
                    lines
                });
                let want = lines * clip;
                assert!(got == want || (got.is_nan() && want.is_nan()), "{lines} x {clip}: {got}");
                assert_eq!(got > 0.0, want > 0.0);
                assert_eq!(cast, clip != 0.0, "clip {clip}");
            }
        }
    }
}
