//! The damage indicator's side shape as a picture (`Draw::Trapezoid`): a
//! quadrilateral with a vertical edge at each side of its box, slanted top
//! and bottom, and a vertex alpha across it, as the coverage image of an
//! additive glyph material (white, the shape's alpha in its alpha
//! channel; the draw's colour tints it). Vertex alpha depends on x alone
//! here, so a linear gradient is exactly what the triangles'
//! interpolation gives. Quantized to a key so equal shapes share an image.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

/// A shape's image size, texels.
pub const WIDTH: usize = 64;
pub const HEIGHT: usize = 128;

/// A shape as bytes: the top and bottom at the left and right edges as
/// fractions (of 255) of the box's height, the alpha at each edge.
pub type ShapeKey = [u8; 6];

/// The key of a shape in `rect`: edge heights `top` and `bottom` (screen
/// y at the left and right edges), alphas `alpha`.
pub fn shape_key(rect: &Rect, alpha: &[f32; 2], top: &[f32; 2], bottom: &[f32; 2]) -> ShapeKey {
    let h = rect.height().max(1e-3);
    let y = |v: f32| (((v - rect.min.y) / h).clamp(0.0, 1.0) * 255.0).round() as u8;
    let a = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [y(top[0]), y(top[1]), y(bottom[0]), y(bottom[1]), a(alpha[0]), a(alpha[1])]
}

/// The shape's alpha at (`x`, `y`), fractions of its box: 0 outside, the
/// edge alphas blended across inside, the slanted edges one texel soft.
pub fn shape_alpha(key: &ShapeKey, x: f32, y: f32) -> f32 {
    let f = |i: usize| f32::from(key[i]) / 255.0;
    let top = f(0) + (f(1) - f(0)) * x;
    let bottom = f(2) + (f(3) - f(2)) * x;
    let soft = HEIGHT as f32;
    let inside = ((y - top) * soft + 0.5).clamp(0.0, 1.0) * ((bottom - y) * soft + 0.5).clamp(0.0, 1.0);
    inside * (f(4) + (f(5) - f(4)) * x)
}

/// The coverage image of `key`.
pub fn shape_image(key: ShapeKey) -> Image {
    let mut data = Vec::with_capacity(WIDTH * HEIGHT * 4);
    for j in 0..HEIGHT {
        for i in 0..WIDTH {
            let a = shape_alpha(&key, (i as f32 + 0.5) / WIDTH as f32, (j as f32 + 0.5) / HEIGHT as f32);
            data.extend_from_slice(&[255, 255, 255, (a * 255.0).round() as u8]);
        }
    }
    Image::new(
        Extent3d { width: WIDTH as u32, height: HEIGHT as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spec's left shape at 640 x 480: outer edge x 30 (y 100 to 340,
    /// alpha 0), inner edge x 66 (y 120 to 320, alpha 1).
    fn left() -> (Rect, ShapeKey) {
        let rect = Rect::new(30.0, 100.0, 66.0, 340.0);
        (rect, shape_key(&rect, &[0.0, 1.0], &[100.0, 120.0], &[340.0, 320.0]))
    }

    #[test]
    fn the_shape_is_a_trapezoid_with_alpha_rising_inward() {
        let (_, key) = left();
        // Middle row: 0 at the outer edge, 1 at the inner.
        assert!(shape_alpha(&key, 0.02, 0.5) < 0.05);
        assert!((shape_alpha(&key, 0.5, 0.5) - 0.5).abs() < 0.02);
        assert!(shape_alpha(&key, 0.98, 0.5) > 0.95);
        // The slanted end: the inner edge starts 20 of 240 down (1/12), the
        // outer at the box's top; so near the top row only the outer side
        // is inside (and its alpha is ~0).
        assert!(shape_alpha(&key, 0.98, 0.03) == 0.0, "above the inner edge's top");
        assert!(shape_alpha(&key, 0.98, 0.12) > 0.9);
        assert!(shape_alpha(&key, 0.98, 0.97) == 0.0, "below the inner edge's bottom");
    }

    #[test]
    fn equal_shapes_share_a_key_and_the_mirror_does_not() {
        let (rect, key) = left();
        assert_eq!(key, shape_key(&rect, &[0.0, 1.0], &[100.0, 120.0], &[340.0, 320.0]));
        assert_ne!(key, shape_key(&rect, &[1.0, 0.0], &[120.0, 100.0], &[320.0, 340.0]));
    }

    /// With `CHIMERA_CAPTURE_DIR` set: the two side shapes of a hit at full
    /// strength (255 44 0, alpha 200, added) on a grey ramp, 640 x 480, as
    /// raw RGB (`hl2_shapes.rgb`), by the material's rule.
    #[test]
    fn capture_the_shapes() {
        let Ok(dir) = std::env::var("CHIMERA_CAPTURE_DIR") else { return };
        let (w, h) = (640usize, 480usize);
        let mut px: Vec<[f32; 3]> = (0..w * h).map(|i| [0.3 + 0.3 * ((i % w) as f32 / w as f32); 3]).collect();
        let left = Rect::new(30.0, 100.0, 66.0, 340.0);
        let shapes = [
            (left, shape_key(&left, &[0.0, 1.0], &[100.0, 120.0], &[340.0, 320.0])),
            (Rect::new(574.0, 100.0, 610.0, 340.0), shape_key(&left, &[1.0, 0.0], &[120.0, 100.0], &[320.0, 340.0])),
        ];
        for (rect, key) in shapes {
            for y in rect.min.y as usize..rect.max.y as usize {
                for x in rect.min.x as usize..rect.max.x as usize {
                    let a = shape_alpha(&key, (x as f32 + 0.5 - rect.min.x) / rect.width(), (y as f32 + 0.5 - rect.min.y) / rect.height()) * 200.0 / 255.0;
                    let dst = &mut px[y * w + x];
                    for (c, v) in [1.0, 44.0 / 255.0, 0.0].into_iter().enumerate() {
                        dst[c] += v * a;
                    }
                }
            }
        }
        let bytes: Vec<u8> = px.iter().flat_map(|p| p.map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8)).collect();
        std::fs::write(std::path::Path::new(&dir).join("hl2_shapes.rgb"), bytes).unwrap();
    }
}
