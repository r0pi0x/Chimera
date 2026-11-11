//! Images built from map data: textures, cubemaps and lightmaps (Source's
//! LDR lightmap encoding). A texture with a block-compressed copy
//! (`MapTexture::bc`: VTF and DTX DXT1/3/5) goes up as BC1/2/3 when the
//! device takes BC (`bc_wanted`; `CHIMERA_BC=0` forces the decoded copy for
//! comparisons), else decoded.

use bevy::{
    asset::RenderAssetUsages,
    image::{
        CompressedImageFormatSupport, CompressedImageFormats, ImageAddressMode, ImageFilterMode, ImageSampler,
        ImageSamplerDescriptor,
    },
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

#[allow(unused_imports)]
use crate::map::*;

/// Source LDR lightmaps (specs/cs_source/shaders.md): each linear value L is
/// rounded to 1/1024, clamped to 4095/1024, stored as the 8-bit sRGB texel
/// 0.5 L^(1/2.2), and multiplied back by 2^2.2 after the sampler's decode.
pub const SOURCE_LIGHTMAP_SCALE: f32 = 4.594_793;

pub fn source_ldr_texel(l: f32) -> u8 {
    SOURCE_LDR.texel[source_ldr_level(l)]
}

/// The 1/1024 step a linear lightmap value is stored at (0-4095).
pub fn source_ldr_level(l: f32) -> usize {
    (l * 1024.0).round().clamp(0.0, 4095.0) as usize
}

/// Every level's encoding, worked out once: a `powf` per channel was most
/// of the cost of relighting blocks of animated light styles.
pub struct SourceLdrTable {
    /// The stored 8-bit value of each level.
    pub texel: [u8; 4096],
    /// Each level's 0.5 L^(1/2.2), before the 8-bit rounding.
    pub gamma: [f32; 4096],
}

pub static SOURCE_LDR: std::sync::LazyLock<SourceLdrTable> = std::sync::LazyLock::new(|| {
    let mut t = SourceLdrTable {
        texel: [0; 4096],
        gamma: [0.0; 4096],
    };
    for i in 0..4096 {
        let l = i as f32 / 1024.0;
        t.texel[i] = (255.0 * 0.5 * l.powf(1.0 / 2.2)).round() as u8;
        t.gamma[i] = 0.5 * l.powf(1.0 / 2.2);
    }
    t
});

/// Source LDR bump pages (specs/cs_source/shaders.md, "Bump page encoding
/// at upload"): the three directional values of a luxel (linear) are
/// scaled so their mean is the flat value's encoded level, overflow above 1
/// is shared out to the other pages, and the result is stored linearly
/// (the sampler's sRGB decode x 2^2.2 still applies when reading).
pub fn source_ldr_bump_texels(flat: [f32; 3], pages: [[f32; 3]; 3]) -> [[u8; 3]; 3] {
    let mut q = [[0.0f32; 3]; 3];
    for c in 0..3 {
        let goal = SOURCE_LDR.gamma[source_ldr_level(flat[c])];
        let mean = (pages[0][c] + pages[1][c] + pages[2][c]) / 3.0;
        let s = if mean > 0.0 { goal / mean } else { 0.0 };
        for k in 0..3 {
            q[k][c] = pages[k][c] * s;
        }
    }
    let max = |v: [f32; 3]| v[0].max(v[1]).max(v[2]);
    // Order once by largest channel, descending; ties go to the last
    // matching permutation in this list.
    const ORDERS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    let m = q.map(max);
    let order = ORDERS
        .iter()
        .rev()
        .find(|o| m[o[0]] >= m[o[1]] && m[o[1]] >= m[o[2]])
        .copied()
        .unwrap_or([0, 1, 2]);
    for &k in &order {
        let top = max(q[k]);
        if top > 1.0 {
            let excess = q[k].map(|v| v * (top - 1.0) / top);
            for c in 0..3 {
                q[k][c] -= excess[c];
                for other in (0..3).filter(|&o| o != k) {
                    q[other][c] += excess[c] / 2.0;
                }
            }
        }
    }
    q.map(|v| {
        let top = max(v);
        let v = if top > 1.0 { v.map(|x| x / top) } else { v };
        v.map(|x| (255.0 * x.max(0.0)).round().clamp(0.0, 255.0) as u8)
    })
}

/// Whether textures go to the GPU block compressed (`MapTexture::bc`):
/// the device takes BC (`CompressedImageFormatSupport`, which the render
/// plugin fills in from the adapter), unless `CHIMERA_BC=0` asks for the
/// decoded copies (to compare, or to rule BC out).
pub fn bc_wanted(support: Option<&CompressedImageFormatSupport>) -> bool {
    support.is_some_and(|s| s.0.contains(CompressedImageFormats::BC))
        && !chimera_core::env::var("CHIMERA_BC").is_ok_and(|v| v == "0")
}

/// The GPU format of a texture's blocks.
fn bc_format(format: MapBcFormat, srgb: bool) -> TextureFormat {
    match (format, srgb) {
        (MapBcFormat::Bc1, true) => TextureFormat::Bc1RgbaUnormSrgb,
        (MapBcFormat::Bc1, false) => TextureFormat::Bc1RgbaUnorm,
        (MapBcFormat::Bc2, true) => TextureFormat::Bc2RgbaUnormSrgb,
        (MapBcFormat::Bc2, false) => TextureFormat::Bc2RgbaUnorm,
        (MapBcFormat::Bc3, true) => TextureFormat::Bc3RgbaUnormSrgb,
        (MapBcFormat::Bc3, false) => TextureFormat::Bc3RgbaUnorm,
    }
}

/// Whether `b`'s blocks are a valid texture of `t`'s size (`MapBc::fits`:
/// the device rejects anything else).
fn bc_fits(t: &MapTexture, b: &MapBc) -> bool {
    b.fits(t.width, t.height)
}

/// A repeating, mipmapped GPU image: the texture's own mip levels when it
/// has a complete chain (`MapTexture::mips`: shipped, or generated by its
/// loader off the main thread), else box-filtered here
/// (`MapTexture::box_mips`). With `bc` (`bc_wanted`) and a compressed copy
/// (`MapTexture::bc`) whose levels fit the format, the blocks as stored,
/// with the file's mip levels (the chain may stop above 1 x 1). A texture
/// kept as its blocks only (`MapTexture::keep_blocks_only`) is decoded here
/// when the blocks don't go up as they are.
pub fn to_image(t: &MapTexture, look: &MapLook, bc: bool) -> Image {
    let decoded;
    let t = if t.blocks_only() && !bc {
        decoded = t.decoded();
        &*decoded
    } else {
        t
    };
    let compressed = t.bc.as_ref().filter(|b| bc && bc_fits(t, b));
    let mut data;
    let mut levels = 1;
    let mut format = if t.srgb {
        TextureFormat::Rgba8UnormSrgb
    } else {
        TextureFormat::Rgba8Unorm
    };
    if let Some(b) = compressed {
        data = b.levels.concat();
        levels = b.levels.len() as u32;
        format = bc_format(b.format, t.srgb);
    } else {
        data = t.rgba8.clone();
        let (mut w, mut h) = (t.width as usize, t.height as usize);
        // Prefer the texture's own mip levels (the original tools' filtering).
        let complete = (t.width.max(t.height) as f32).log2() as usize;
        if !t.mips.is_empty() && t.mips.len() == complete {
            for m in &t.mips {
                data.extend_from_slice(m);
            }
            levels += t.mips.len() as u32;
            w = 1;
            h = 1;
        }
        if w > 1 || h > 1 {
            let generated = t.box_mips();
            levels += generated.len() as u32;
            for m in &generated {
                data.extend_from_slice(m);
            }
        }
    }
    let mut image = Image::new_uninit(
        Extent3d {
            width: t.width,
            height: t.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: if look.trilinear {
            ImageFilterMode::Linear
        } else {
            ImageFilterMode::Nearest
        },
        anisotropy_clamp: look.anisotropy.max(1),
        ..default()
    });
    image
}

/// Sample `image` clamped to its edges (pictures drawn once, not tiled).
pub fn clamp_edges(image: &mut Image) {
    if let ImageSampler::Descriptor(d) = &mut image.sampler {
        d.address_mode_u = ImageAddressMode::ClampToEdge;
        d.address_mode_v = ImageAddressMode::ClampToEdge;
    }
}

/// A baked cubemap as a cube texture (six sRGB layers with their mips,
/// clamped, linear within a mip, the nearest mip: the reference client's
/// bilinear filtering). The mips widen a bumped surface's highlight where
/// its normal varies faster than the screen's pixels, as the game's do.
pub fn cube_image(c: &MapCubemap) -> Image {
    // Layer by layer, each layer's mips largest first (wgpu's default
    // layer-major order).
    let mut image = Image::new_uninit(
        Extent3d {
            width: c.size,
            height: c.size,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    // Comparison hook: CHIMERA_CUBEMAP_TOP_MIP=1 keeps the largest mip only.
    if chimera_core::env::var("CHIMERA_CUBEMAP_TOP_MIP").is_ok_and(|v| v == "1") {
        let top = (c.size * c.size * 4) as usize;
        image.data = Some(
            c.faces
                .iter()
                .flat_map(|f| f[..top.min(f.len())].iter().copied())
                .collect(),
        );
    } else {
        image.data = Some(c.faces.concat());
        image.texture_descriptor.mip_level_count = c.mips.max(1);
    }
    image.texture_view_descriptor = Some(bevy::render::render_resource::TextureViewDescriptor {
        dimension: Some(bevy::render::render_resource::TextureViewDimension::Cube),
        ..default()
    });
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        address_mode_w: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// A lighting atlas layer as a filterable half-float texture, clamped.
/// A lighting atlas layer in Source's LDR encoding (see
/// `SOURCE_LIGHTMAP_SCALE`): 8-bit sRGB, decoded by the sampler.
pub fn source_lightmap_image(rgb: &[[f32; 3]], width: u32, height: u32) -> Image {
    srgb8_image(source_ldr_texels(rgb), width, height)
}

/// RGBA8 sRGB texels as a linearly filtered lightmap layer.
pub fn srgb8_image(data: Vec<u8>, width: u32, height: u32) -> Image {
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image
}

/// The lightmap atlas as images: the plain copy (Bevy's `Lightmap` on
/// StandardMaterial parts), the world material's copy (in the game's own
/// encoding when `source_ldr`; None: the plain one, the same texels) and
/// its directional pages.
pub struct LightmapLayers {
    pub plain: Image,
    pub world: Option<Image>,
    pub bumped: Option<[Image; 3]>,
}

impl LightmapLayers {
    pub fn build(
        rgb: &[[f32; 3]],
        bumped: Option<&[Vec<[f32; 3]>; 3]>,
        width: u32,
        height: u32,
        source_ldr: bool,
    ) -> Self {
        let world_layer = |rgb: &[[f32; 3]]| {
            if source_ldr {
                source_lightmap_image(rgb, width, height)
            } else {
                lightmap_image(rgb, width, height)
            }
        };
        let pages = bumped.map(|b| {
            if source_ldr {
                // The game encodes the three pages together, against the flat one.
                source_ldr_page_texels(rgb, b).map(|t| srgb8_image(t, width, height))
            } else {
                std::array::from_fn(|i| world_layer(&b[i]))
            }
        });
        Self {
            plain: lightmap_image(rgb, width, height),
            world: source_ldr.then(|| world_layer(rgb)),
            bumped: pages,
        }
    }
}

pub fn lightmap_image(rgb: &[[f32; 3]], width: u32, height: u32) -> Image {
    let data = f16_texels(rgb);
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image
}

/// Texel bytes of the plain lightmap layer (`lightmap_image`'s format).
/// Big atlases (Combat Arms' TERRORDOME: 8192 x 8192) convert on
/// several threads: the map spawns on the main thread.
pub fn f16_texels(rgb: &[[f32; 3]]) -> Vec<u8> {
    let convert = |rgb: &[[f32; 3]], data: &mut [u8]| {
        let one = half::f16::from_f32(1.0).to_le_bytes();
        for ([r, g, b], out) in rgb.iter().zip(data.chunks_exact_mut(8)) {
            let [r, g, b] = [*r, *g, *b].map(|c| half::f16::from_f32(c).to_le_bytes());
            out.copy_from_slice(&[r[0], r[1], g[0], g[1], b[0], b[1], one[0], one[1]]);
        }
    };
    let mut data = vec![0u8; rgb.len() * 8];
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(8);
    if rgb.len() < 1 << 20 || threads < 2 {
        convert(rgb, &mut data);
        return data;
    }
    let chunk = rgb.len().div_ceil(threads);
    std::thread::scope(|s| {
        for (texels, out) in rgb.chunks(chunk).zip(data.chunks_mut(chunk * 8)) {
            s.spawn(move || convert(texels, out));
        }
    });
    data
}

/// Texel bytes of the world layer in Source's LDR encoding
/// (`source_lightmap_image`'s format).
pub fn source_ldr_texels(rgb: &[[f32; 3]]) -> Vec<u8> {
    let mut data = Vec::with_capacity(rgb.len() * 4);
    for [r, g, b] in rgb {
        data.extend([source_ldr_texel(*r), source_ldr_texel(*g), source_ldr_texel(*b), 255]);
    }
    data
}

/// Texel bytes of the three directional pages in Source's LDR encoding.
pub fn source_ldr_page_texels(flat: &[[f32; 3]], pages: &[Vec<[f32; 3]>; 3]) -> [Vec<u8>; 3] {
    let mut texels: [Vec<u8>; 3] = std::array::from_fn(|_| Vec::with_capacity(flat.len() * 4));
    for (i, f) in flat.iter().enumerate() {
        let enc = source_ldr_bump_texels(*f, [pages[0][i], pages[1][i], pages[2][i]]);
        for (t, [r, g, b]) in texels.iter_mut().zip(enc) {
            t.extend([r, g, b, 255]);
        }
    }
    texels
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A texture kept as its blocks only (`MapTexture::keep_blocks_only`)
    /// goes up as its blocks where the GPU takes BC, else decoded here
    /// with its whole mip chain filtered.
    #[test]
    fn a_blocks_only_texture_is_decoded_where_bc_does_not_go_up() {
        let white = [0xFF, 0xFF, 0, 0, 0, 0, 0, 0];
        let mut t = MapTexture {
            name: "t".into(),
            srgb: true,
            mips: Vec::new(),
            width: 8,
            height: 8,
            rgba8: vec![255; 8 * 8 * 4],
            bc: Some(MapBc {
                format: MapBcFormat::Bc1,
                levels: vec![white.repeat(4), white.to_vec()],
            }),
        };
        assert!(t.keep_blocks_only() && t.rgba8.is_empty() && t.blocks_only());
        let look = MapLook::default();
        let up = to_image(&t, &look, true);
        assert_eq!(up.texture_descriptor.format, TextureFormat::Bc1RgbaUnormSrgb);
        assert_eq!((up.texture_descriptor.mip_level_count, up.data.as_ref().map(Vec::len)), (2, Some(40)));
        let decoded = to_image(&t, &look, false);
        assert_eq!(decoded.texture_descriptor.format, TextureFormat::Rgba8UnormSrgb);
        assert_eq!(decoded.texture_descriptor.mip_level_count, 4);
        let data = decoded.data.unwrap();
        assert_eq!(data.len(), 256 + 64 + 16 + 4);
        assert!(data.iter().all(|b| *b == 255), "white, every level");
        // Blocks that aren't a whole texture keep the texels.
        let mut odd = MapTexture { width: 6, rgba8: vec![255; 6 * 8 * 4], ..t.clone() };
        assert!(!odd.keep_blocks_only() && !odd.rgba8.is_empty());
    }
}
