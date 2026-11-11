//! The textures of Combat Arms models kept past their map (docs/
//! performance.md, "A client's memory"): a body loaded on demand
//! (`models::load_body`), a view model (`view_model::on_demand::
//! load_weapon`, with the hands) and a held model (`models::
//! load_held_weapon`) keep each texture whose file holds whole DXT blocks
//! as those blocks alone (`world::BlocksOnly`), no RGBA8 texels or mip
//! levels: a client kept 1.9 GB of decoded texels for them after three maps.
//! Bytes, not times. Passes with a note without the install (or the V2
//! key).

use chimera_world::map::MapTexture;
use combat_arms::game::{
    GAME, INFO,
    characters::models,
    data::CaData,
    view_model::on_demand,
};

fn mount() -> Option<chimera_core::mount::Mount> {
    let install = chimera_core::mount::config::LocalConfig::load().ok().and_then(|c| c.game_path(GAME));
    let Some(install) = install else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    (INFO.open)(&install).ok()
}

/// Decoded bytes (texels and mips) of the textures with whole blocks, the
/// blocks' bytes, how many are blocks only, how many textures.
fn bytes(textures: &[MapTexture]) -> (usize, usize, usize, usize) {
    let decoded = |t: &MapTexture| t.rgba8.len() + t.mips.iter().map(Vec::len).sum::<usize>();
    let fitting = textures.iter().filter(|t| t.bc.as_ref().is_some_and(|b| b.fits(t.width, t.height)));
    let blocks = |t: &MapTexture| t.bc.as_ref().map_or(0, |b| b.levels.iter().map(Vec::len).sum());
    (
        fitting.clone().map(decoded).sum(),
        fitting.clone().map(blocks).sum(),
        textures.iter().filter(|t| t.blocks_only()).count(),
        textures.len(),
    )
}

#[test]
fn models_kept_for_later_maps_keep_their_blocks_only() {
    const M4: &str = "combat_arms:m4a1";
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let ca = CaData::install();
    let data = ca.characters();
    let Some(id) = data.rung_bodies().into_iter().find(|id| !models::LOADED.contains(id)) else {
        eprintln!("skipping: no rung bodies (the V1 key)");
        return;
    };
    let body = match models::load_body(&ca, id, &file) {
        Ok(b) => b.textures,
        Err(e) => {
            eprintln!("skipping: body {id} doesn't load ({e}; the V2 key)");
            return;
        }
    };
    let view = on_demand::load_weapon(&ca, M4, &file).expect("the M4A1's view model").textures;
    let held = models::load_held_weapon(&ca, M4, &file).expect("the M4A1's held model").textures;
    for (what, textures) in [("body", &body[..]), ("view model", &view[..]), ("held model", &held[..])] {
        let (decoded, blocks, lean, all) = bytes(textures);
        // What decoding them cost: RGBA8 and a third more for the mips.
        let texels: usize = textures.iter().filter(|t| t.blocks_only()).map(|t| (t.width * t.height * 4) as usize * 4 / 3).sum();
        eprintln!(
            "{what}: {all} textures, {lean} blocks only ({blocks} bytes of blocks, {texels} decoded), {decoded} decoded \
             bytes beside blocks"
        );
        // The held model's one texture is BGRA (no blocks) on the dev box's install.
        assert!(lean > 0 || what == "held model", "{what}: some textures are kept as blocks");
        assert_eq!(decoded, 0, "{what}: a texture with whole blocks keeps no decoded texels");
        for t in textures.iter().filter(|t| t.blocks_only()) {
            // The drawing's fallback (no BC on the GPU) decodes level 0.
            assert_eq!(t.decoded().rgba8.len(), (t.width * t.height * 4) as usize, "{what}: {}", t.name);
        }
    }
}
