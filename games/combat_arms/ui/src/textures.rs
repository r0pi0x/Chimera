//! Combat Arms' textures as assets (architecture review L4): `.dtx` files
//! of the install loaded by the `AssetServer` through the install's asset
//! source (`combat_arms://<path>`, `chimera_present::assets`), decoded on
//! its threads and freed when the last handle drops.
//!
//! - By default (`DtxSettings::default`) as the maps draw them (X9): the
//!   file's DXT1/3/5 blocks with its own mip levels where the device takes
//!   BC (`assets::texture_bc`, `game::world::dtx_bc`), else decoded RGBA8
//!   with a box-filtered mip chain; repeating, linear.
//! - `DtxSettings { plain: true }`: the top level decoded to RGBA8, no mip
//!   levels, the default sampler: what the effects drew from their own
//!   decode before (`effects::FxImages`), kept so they look the same.
//!
//! Known shortcuts: no `DetailLevel` (the map loader's level skipping):
//! the loaded level is the file's first.

use bevy::{
    asset::{AssetLoader, LoadContext, RenderAssetUsages, io::Reader},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use lithtech_formats::dtx::Dtx;
use serde::{Deserialize, Serialize};

/// How a `.dtx` becomes an image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DtxSettings {
    /// The top level only, decoded (the effects' look).
    pub plain: bool,
}

/// Loads `.dtx` files as images.
#[derive(TypePath)]
pub struct DtxLoader {
    /// Keep block-compressed texels (the device takes BC).
    pub bc: bool,
}

impl AssetLoader for DtxLoader {
    type Asset = Image;
    type Settings = DtxSettings;
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &DtxSettings,
        _: &mut LoadContext<'_>,
    ) -> Result<Image, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let dtx = Dtx::parse(bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(dtx_image(&dtx, *settings, self.bc))
    }

    fn extensions(&self) -> &[&str] {
        &["dtx"]
    }
}

/// A parsed `.dtx` as an image (`DtxSettings`).
pub fn dtx_image(dtx: &Dtx, settings: DtxSettings, bc: bool) -> Image {
    if settings.plain {
        return Image::new(
            Extent3d {
                width: dtx.width,
                height: dtx.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            dtx.rgba(0),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
    }
    let (width, height) = dtx.level_dims(0);
    let mut texture = crate::map::MapTexture {
        name: String::new(),
        srgb: true,
        mips: Vec::new(),
        width,
        height,
        rgba8: dtx.rgba(0),
        bc: if bc { crate::game::world::dtx_bc(dtx, 0) } else { None },
    };
    if texture.bc.is_none() {
        texture.mips = texture.box_mips();
    }
    crate::render::to_image(&texture, &crate::map::MapLook::default(), bc)
}

/// Registers `DtxLoader` once the renderer has said what the device takes
/// (`finish`), where there is an asset server.
pub struct CaTexturesPlugin;

impl Plugin for CaTexturesPlugin {
    fn build(&self, _: &mut App) {}

    fn finish(&self, app: &mut App) {
        if app.world().contains_resource::<AssetServer>() {
            let bc = crate::assets::texture_bc(app);
            app.register_asset_loader(DtxLoader { bc });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4 x 4 BGRA texture: plain is its decoded top level; the default
    /// adds the mip chain (no blocks to keep).
    #[test]
    fn plain_and_mapped_images() {
        // The header (textures_dtx.md "Header"): a texture, 4 x 4,
        // version -5, one level, BGRA (code 3); then its 64 bytes.
        let mut bytes = vec![0u8; lithtech_formats::dtx::HEADER_LEN];
        bytes[4] = 4;
        bytes[6] = 4;
        bytes[8..12].copy_from_slice(&lithtech_formats::dtx::VERSION.to_le_bytes());
        bytes[0x0C] = 1;
        bytes[0x1A] = 3;
        bytes.extend((0..64u8).map(|i| i.wrapping_mul(37)));
        let dtx = Dtx::parse(bytes).unwrap();
        let plain = dtx_image(&dtx, DtxSettings { plain: true }, true);
        assert_eq!(plain.texture_descriptor.mip_level_count, 1);
        assert_eq!(plain.texture_descriptor.format, TextureFormat::Rgba8UnormSrgb);
        assert_eq!(plain.data.as_deref(), Some(&dtx.rgba(0)[..]));
        let mapped = dtx_image(&dtx, DtxSettings::default(), true);
        assert_eq!(mapped.texture_descriptor.mip_level_count, 3);
        assert_eq!(mapped.texture_descriptor.format, TextureFormat::Rgba8UnormSrgb);
    }
}
