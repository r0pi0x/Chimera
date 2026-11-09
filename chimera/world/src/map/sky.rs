//! The sky: the 2D sky cubemap's image and the render layers that
//! separate the 3D skybox from the world (`render::sky` draws
//! them: the sky on the cameras, the 3D skybox's camera).

#[allow(unused_imports)]
use super::*;

/// Render layer for 3D skybox content.
pub const SKYBOX_LAYER: usize = 1;

/// Draws the 3D skybox; follows the main camera.
#[derive(Component)]
pub struct SkyboxCamera;

/// The playable world's sky faces as triangles (`MapData::sky_surfaces`),
/// for what ends on the sky rather than touching a wall.
#[derive(Resource, Clone, Debug, Default)]
pub struct MapSkyFaces(pub Vec<[Vec3; 3]>);

impl MapSkyFaces {
    pub fn of(points: &[[f32; 3]]) -> Self {
        Self(
            points
                .chunks_exact(3)
                .map(|t| [Vec3::from_array(t[0]), Vec3::from_array(t[1]), Vec3::from_array(t[2])])
                .collect(),
        )
    }

    /// How far along `dir` (unit) from `from`, within `len`, the first sky
    /// face is.
    pub fn along(&self, from: Vec3, dir: Vec3, len: f32) -> Option<f32> {
        self.0
            .iter()
            .filter_map(|[a, b, c]| {
                let (e1, e2) = (*b - *a, *c - *a);
                let p = dir.cross(e2);
                let det = e1.dot(p);
                if det.abs() < 1e-9 {
                    return None;
                }
                let s = from - *a;
                let u = s.dot(p) / det;
                let q = s.cross(e1);
                let v = dir.dot(q) / det;
                let t = e2.dot(q) / det;
                (u >= 0.0 && v >= 0.0 && u + v <= 1.0 && (0.0..=len).contains(&t)).then_some(t)
            })
            .min_by(f32::total_cmp)
    }
}

/// Holds nothing: the sky camera's layer when only the 2D sky shows.
pub const EMPTY_LAYER: usize = 31;

/// Six faces into one cube texture. Faces narrower than tall or shorter
/// than wide (Source sides are often half height) are stretched to square.
pub fn sky_image(sky: &MapSky, textures: &[MapTexture]) -> Image {
    if let Some(faces) = sky.hdr.as_deref()
        && chimera_core::env::var_os("CHIMERA_SKY_DEBUG").is_none()
    {
        return hdr_sky_image(sky, faces);
    }
    let size = sky
        .faces
        .iter()
        .map(|(t, _)| textures[*t].width.max(textures[*t].height))
        .max()
        .unwrap_or(1);
    let mut data = Vec::with_capacity((size * size * 4 * 6) as usize);
    // Calibration hook for `refcmp skyconv`: each pixel encodes its own
    // face (blue) and position (red = u, green = v).
    let debug = chimera_core::env::var_os("CHIMERA_SKY_DEBUG").is_some();
    for (face, (tex, turns)) in sky.faces.into_iter().enumerate() {
        if debug {
            for y in 0..size {
                for x in 0..size {
                    data.extend_from_slice(&[
                        (x * 255 / size) as u8,
                        (y * 255 / size) as u8,
                        face as u8 * 40 + 20,
                        255,
                    ]);
                }
            }
            continue;
        }
        let t = &textures[tex];
        for y in 0..size {
            for x in 0..size {
                let (sx, sy) = sky_texel(x, y, size, turns, &sky.transforms[face], t.width, t.height);
                let i = ((sy * t.width + sx) * 4) as usize;
                data.extend_from_slice(&t.rgba8[i..i + 4]);
            }
        }
    }
    sky_cube_image(size, data, TextureFormat::Rgba8UnormSrgb)
}

/// How badly one cube layer of the (LDR) sky joins its four neighbours:
/// the mean colour difference (0-255) across its edges, each side sampled
/// just inside its own layer and just inside the neighbour, as a GPU
/// samples the cube. Each layer's orientation (`MapSky::faces`) decides
/// how the textures meet, so a misturned face shows as a large error.
pub fn sky_seam_error(sky: &MapSky, textures: &[MapTexture], layer: usize) -> f32 {
    let mut sky = sky.clone();
    sky.hdr = None;
    let image = sky_image(&sky, textures);
    let size = image.width() as usize;
    let Some(data) = image.data.as_ref() else { return 0.0 };
    // A direction's layer and texel, by the GPU cube convention.
    let sample = |d: Vec3| -> [f32; 3] {
        let a = d.abs();
        let (l, u, v, m) = if a.x >= a.y && a.x >= a.z {
            if d.x > 0.0 {
                (0, -d.z, -d.y, a.x)
            } else {
                (1, d.z, -d.y, a.x)
            }
        } else if a.y >= a.z {
            if d.y > 0.0 {
                (2, d.x, d.z, a.y)
            } else {
                (3, d.x, -d.z, a.y)
            }
        } else if d.z > 0.0 {
            (4, d.x, -d.y, a.z)
        } else {
            (5, -d.x, -d.y, a.z)
        };
        let px = |c: f32| (((c / m + 1.0) / 2.0 * size as f32) as usize).min(size - 1);
        let i = ((l * size + px(v)) * size + px(u)) * 4;
        [data[i] as f32, data[i + 1] as f32, data[i + 2] as f32]
    };
    let axis = layer / 2;
    let sign = if layer % 2 == 0 { 1.0 } else { -1.0 };
    let eps = 1.5 / size as f32;
    let (mut total, mut n) = (0.0, 0);
    for other in (0..3).filter(|a| *a != axis) {
        let third = 3 - axis - other;
        for edge in [-1.0f32, 1.0] {
            for k in 0..64 {
                let t = (k as f32 + 0.5) / 64.0 * 2.0 - 1.0;
                let point = |main: f32, side: f32| {
                    let mut p = Vec3::ZERO;
                    p[axis] = sign * main;
                    p[other] = edge * side;
                    p[third] = t * (1.0 - eps);
                    p
                };
                let inside = sample(point(1.0, 1.0 - eps));
                let outside = sample(point(1.0 - eps, 1.0));
                total += (0..3).map(|c| (inside[c] - outside[c]).abs()).sum::<f32>() / 3.0;
                n += 1;
            }
        }
    }
    total / n as f32
}

/// Which texel of a `w` x `h` sky face lands at (x, y) of a `size` cube
/// face, the face's coordinates through its material's `transform`
/// (clamped to the texture, as the sky samples it).
pub fn sky_texel(
    x: u32,
    y: u32,
    size: u32,
    turns: u8,
    transform: &MapUvTransform,
    w: u32,
    h: u32,
) -> (u32, u32) {
    // Rotate clockwise by `turns` quarter turns: sample the source
    // pixel that lands at (x, y).
    let (mut u, mut v) = (x, y);
    for _ in 0..turns % 4 {
        (u, v) = (v, size - 1 - u);
    }
    if turns >= 4 {
        u = size - 1 - u;
    }
    // Skip each face's outermost texel row and column: sky
    // textures leave them as borders (dust2's side faces end in
    // a black row) that the game never shows, while a cube map
    // blends them in at every seam.
    let inner = |p: u32, n: u32| 1 + (p as u64 * n.saturating_sub(2) as u64 / size as u64) as u32;
    if transform.is_identity() {
        return (inner(u, w).min(w - 1), inner(v, h).min(h - 1));
    }
    let face_uv = Vec2::new((u as f32 + 0.5) / size as f32, (v as f32 + 0.5) / size as f32);
    let t = transform.apply(face_uv, 0.0).clamp(Vec2::ZERO, Vec2::ONE);
    let texel = |c: f32, n: u32| ((c * n.saturating_sub(2) as f32) as u32 + 1).min(n.saturating_sub(2).max(1));
    (texel(t.x, w).min(w - 1), texel(t.y, h).min(h - 1))
}

/// The HDR sky (`MapSky::hdr`) as a linear half-float cube map.
pub fn hdr_sky_image(sky: &MapSky, faces: &[MapHdrImage; 6]) -> Image {
    let size = faces.iter().map(|f| f.width.max(f.height)).max().unwrap_or(1).max(1);
    let mut data = Vec::with_capacity((size * size * 8 * 6) as usize);
    for ((face, (_, turns)), transform) in faces.iter().zip(sky.faces).zip(&sky.transforms) {
        for y in 0..size {
            for x in 0..size {
                let [r, g, b] = if face.width == 0 || face.height == 0 {
                    [0.0; 3]
                } else {
                    let (sx, sy) = sky_texel(x, y, size, turns, transform, face.width, face.height);
                    face.rgb[(sy * face.width + sx) as usize]
                };
                for c in [r, g, b, 1.0] {
                    data.extend_from_slice(&half::f16::from_f32(c).to_le_bytes());
                }
            }
        }
    }
    sky_cube_image(size, data, TextureFormat::Rgba16Float)
}

pub fn sky_cube_image(size: u32, data: Vec<u8>, format: TextureFormat) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_view_descriptor = Some(wgpu_types::TextureViewDescriptor {
        dimension: Some(wgpu_types::TextureViewDimension::Cube),
        ..default()
    });
    image.sampler = ImageSampler::linear();
    image
}

/// World brushes are also drawn by water reflections (props and characters
/// only when the reflection shows entities).
pub fn world_layer_of(skybox: bool) -> bevy::camera::visibility::RenderLayers {
    if skybox { layer_of(true) } else { water::world_layers() }
}

pub fn layer_of(skybox: bool) -> bevy::camera::visibility::RenderLayers {
    bevy::camera::visibility::RenderLayers::layer(if skybox { SKYBOX_LAYER } else { 0 })
}
