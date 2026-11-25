//! A texture with a block-compressed copy (`MapTexture::bc`) goes to the
//! GPU as BC where the device takes it (`render::to_image`), and as RGBA8
//! otherwise or when the blocks do not fit.

use bevy::render::render_resource::TextureFormat;
use chimera_present::render::to_image;
use chimera_world::map::{MapBc, MapBcFormat, MapLook, MapTexture};

/// A 16 x 8 DXT1 texture with its chain down to 1 x 1.
fn texture(srgb: bool) -> MapTexture {
    let level = |w: u32, h: u32| vec![7u8; MapBc::level_bytes(MapBcFormat::Bc1, w, h)];
    MapTexture {
        name: "t".into(),
        srgb,
        mips: Vec::new(),
        width: 16,
        height: 8,
        rgba8: vec![255; 16 * 8 * 4],
        bc: Some(MapBc {
            format: MapBcFormat::Bc1,
            levels: vec![level(16, 8), level(8, 4), level(4, 2), level(2, 1), level(1, 1)].into(),
        }),
    }
}

#[test]
fn compressed_where_the_device_takes_it() {
    let look = MapLook::default();
    let image = to_image(&texture(true), &look, true);
    assert_eq!(image.texture_descriptor.format, TextureFormat::Bc1RgbaUnormSrgb);
    assert_eq!(image.texture_descriptor.mip_level_count, 5);
    assert_eq!(image.data.as_ref().unwrap().len(), 64 + 16 + 8 + 8 + 8);
    let linear = to_image(&texture(false), &look, true);
    assert_eq!(linear.texture_descriptor.format, TextureFormat::Bc1RgbaUnorm);
}

#[test]
fn decoded_without_support_or_when_the_blocks_do_not_fit() {
    let look = MapLook::default();
    let plain = to_image(&texture(true), &look, false);
    assert_eq!(plain.texture_descriptor.format, TextureFormat::Rgba8UnormSrgb);
    assert_eq!(
        plain.texture_descriptor.mip_level_count, 5,
        "16 x 8: five levels, box-filtered"
    );
    let mut t = texture(true);
    std::sync::Arc::make_mut(&mut t.bc.as_mut().unwrap().levels)[1].pop();
    assert_eq!(
        to_image(&t, &look, true).texture_descriptor.format,
        TextureFormat::Rgba8UnormSrgb
    );
    let mut t = texture(true);
    (t.width, t.height) = (15, 8);
    t.rgba8 = vec![0; 15 * 8 * 4];
    assert_eq!(
        to_image(&t, &look, true).texture_descriptor.format,
        TextureFormat::Rgba8UnormSrgb
    );
}
