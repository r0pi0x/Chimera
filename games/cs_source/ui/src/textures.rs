//! CS:S's textures as assets (architecture review L4): `.vtf` files of
//! the install loaded by the `AssetServer` through the install's asset
//! source (`cs_source://materials/...`, `assets`), decoded on its
//! threads as the maps decode them (`material::decode_bytes`: the full
//! image, the file's own mip levels, its DXT blocks) and uploaded as the
//! maps upload them (X9, `render::to_image`: the blocks where the device
//! takes BC, else RGBA8 with mips); freed when the last handle drops.
//!
//! Known shortcuts: the first frame only (animated textures are the
//! map loader's); colour data (sRGB) unless `VtfSettings::linear`.

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
use serde::{Deserialize, Serialize};

/// How a `.vtf` becomes an image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VtfSettings {
    /// Data such as a normal map (linear), not colour (sRGB).
    pub linear: bool,
}

/// Loads `.vtf` files as images.
#[derive(TypePath)]
pub struct VtfLoader {
    /// Keep block-compressed texels (the device takes BC).
    pub bc: bool,
}

impl AssetLoader for VtfLoader {
    type Asset = Image;
    type Settings = VtfSettings;
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &VtfSettings,
        _: &mut LoadContext<'_>,
    ) -> Result<Image, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        vtf_image(&bytes, *settings, self.bc).map_err(std::io::Error::other)
    }

    fn extensions(&self) -> &[&str] {
        &["vtf"]
    }
}

/// A `.vtf` file as an image (`VtfSettings`).
pub fn vtf_image(bytes: &[u8], settings: VtfSettings, bc: bool) -> Result<Image, String> {
    let mut texture = crate::game::material::decode_bytes(bytes)?;
    texture.srgb = !settings.linear;
    Ok(crate::render::to_image(&texture, &crate::map::MapLook::default(), bc))
}

/// Registers `VtfLoader` once the renderer has said what the device takes
/// (`finish`), where there is an asset server.
pub struct CsTexturesPlugin;

impl Plugin for CsTexturesPlugin {
    fn build(&self, _: &mut App) {}

    fn finish(&self, app: &mut App) {
        if app.world().contains_resource::<AssetServer>() {
            let bc = crate::assets::texture_bc(app);
            app.register_asset_loader(VtfLoader { bc });
        }
    }
}
