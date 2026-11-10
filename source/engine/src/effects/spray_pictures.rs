//! Imported sprays in the world (`weapon::spray::SprayPictureSource`, set by
//! the Source games): a `cl_logofile` naming a file in Chimera's user logos
//! folder (where `hud::spray_import` writes Options > Multiplayer > Import
//! Spray's VTFs) becomes the spray's picture, reduced to at most 64 x 64,
//! which then travels with the spray to the server and the other players
//! (docs/plans/active/hl2dm.md, 2026-11-09 follow-up). The install's sprays
//! have no picture here: they go by name.

/// Where imported sprays are written: Chimera's own user data, beside the
/// configs.
pub fn user_logos_dir() -> Option<std::path::PathBuf> {
    crate::console::cfg_dir().and_then(|d| d.parent().map(|p| p.join("logos")))
}

/// The picture of the spray `file` (a `cl_logofile` value) names, if it is
/// one imported into `logos_dir`; None for the install's sprays.
pub fn picture_in(logos_dir: &std::path::Path, file: &str) -> Option<crate::map::decal::SprayPicture> {
    let stem = crate::weapon::spray::logo_name(file)?;
    // The folder's file by case-insensitive name, as the picker lists it.
    let path = std::fs::read_dir(logos_dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_lowercase() == format!("{stem}.vtf"))
            .unwrap_or(false)
    })?;
    let image = vtf::from_bytes(&std::fs::read(path).ok()?)
        .ok()
        .and_then(|v| v.highres_image.decode(0).ok())?
        .to_rgba8();
    crate::map::decal::SprayPicture::reduced(image.width(), image.height(), image.as_raw())
}

/// `picture_in` the user's folder (`user_logos_dir`).
pub fn picture_of(file: &str) -> Option<crate::map::decal::SprayPicture> {
    picture_in(&user_logos_dir()?, file)
}
