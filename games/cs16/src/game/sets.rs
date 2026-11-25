//! What each of Counter-Strike 1.6's pieces reads, for the asset
//! service's sets (`chimera_pack sets cs16`; docs/assets-for-developers.md):
//! the packer loads each piece through these on a tapped mount of the
//! install and enters the paths read as the piece's set, so a client with
//! the asset store alone fetches what loading it reads.
//!
//! - `players`: the eight class bodies (`load_players`: each model with
//!   its external texture file) and the HUD's sprites (`hud::read_all`:
//!   `sprites/hud.txt`, the `weapon_*.txt` lists, the atlases their 640
//!   entries name, the pain arcs);
//! - `weapon/<id>` (`load_weapon`): a weapon's models as its data names
//!   them (view, held, and the dropped `w_` beside the held, with their
//!   `<name>t.mdl` texture and `<name>NN.mdl` sequence-group files) and
//!   its sounds (`weapons::data_sounds`: each entry's waves under
//!   `sound/`, and the view model's sound events), the muzzle flash
//!   sprites its view model's events name and the sprites a grenade's
//!   look draws (`weapons::looks`);
//! - `map/<name>` (`load_map`): the map converted as another game's
//!   gameplay loads it (its world, the WADs its `wad` key names, its sky,
//!   `sound/materials.txt`; the bodies it reads are `players`').
//!
//! GoldSrc keeps its files loose, so the same bytes under two names
//! (`cstrike/` and `valve/` both ship `decals.wad`, maps share WADs) are
//! one record in the store (records are by content).

use goldsrc_engine::content::maps as gmaps;

use crate::{
    map::{KitLoad, MapData, MapLoadOptions},
    mount::Mount,
};

/// Read the eight class bodies (`bodies::add_bodies`, textures decoded)
/// and the HUD's sprites (`hud::read_all`).
pub fn load_players(mount: &Mount) {
    super::bodies::add_bodies(mount, true, &mut MapData::default());
    super::hud::read_all(mount);
}

/// Read weapon `id`'s files (module doc); false if the game has no such
/// weapon.
pub fn load_weapon(mount: &Mount, id: &str) -> bool {
    let Some(w) = super::weapons::data(id) else {
        return false;
    };
    let dropped = super::weapons::dropped_model(&w);
    for model in [w.models.view.as_ref(), w.models.world.as_ref(), w.models.world_alt.as_ref(), dropped.as_ref()].into_iter().flatten() {
        read_model(mount, model);
    }
    // The data's sounds and the view model's sound events, as the kit
    // reads them (`weapons::add_guns`).
    let mut entries = super::weapons::data_sounds(&w);
    if let Some(view) = &w.models.view
        && let Ok((v, _, waves)) = goldsrc_engine::content::weapon_models::view_model(mount, view, id, false)
    {
        entries.extend(waves);
        // The muzzle flashes its events name (`weapons::add_guns`).
        let codes = v.animations.as_deref().map(goldsrc_engine::content::weapon_models::flash_codes).unwrap_or_default();
        for sprite in codes.iter().filter_map(|c| goldsrc_engine::content::weapon_models::flash_sprite(c)) {
            let _ = mount.read(&sprite);
        }
    }
    // The sprites its look draws (`weapons::looks`).
    for sprite in super::weapons::looks::sprites_of(&w.id) {
        let _ = mount.read(sprite);
    }
    let mut table = Default::default();
    for entry in entries {
        goldsrc_engine::content::sounds::add_entry(mount, &entry, &mut table);
    }
    true
}

/// A studio model and the files beside it that it may need: its texture
/// file (`<name>t.mdl`) and sequence groups (`<name>01.mdl`...).
fn read_model(mount: &Mount, path: &str) {
    if mount.read(path).is_err() {
        return;
    }
    let stem = path.strip_suffix(".mdl").unwrap_or(path);
    let _ = mount.read(&format!("{stem}t.mdl"));
    for group in 1..100 {
        if mount.read(&format!("{stem}{group:02}.mdl")).is_err() {
            break;
        }
    }
}

/// Map `name` converted with the players' kit, never from the
/// converted-map cache (a hit reads none of the files).
pub fn load_map(mount: &Mount, name: &str) -> Result<MapData, String> {
    let options = MapLoadOptions { kit: KitLoad::Players(&[]), ..Default::default() };
    gmaps::load_in(&super::maps::CS16, mount, name, &options, None).map(|(m, _)| m)
}
