//! Halo's asset sets: what a client without the disc fetches for a join
//! (docs/plans/active/asset-service.md, "sets"; docs/plans/active/
//! halo-ce.md, section 2). `chimera_pack sets halo_ce` records each set by
//! running the loads here on the install's mount with a tap that notes
//! every path read, as it does for the other games:
//!
//! - `base`: the donor map (`DONOR_MAP`, `maps/bloodgulch.map`), read and
//!   checked (`load_base`). Every multiplayer map holds the same weapons,
//!   projectiles, equipment and bipeds (specs/halo_ce/cache_file.md), so
//!   one map is where Halo's guns and the Spartan are read from on any
//!   game's map once they are rungs; today nothing else of Halo is read
//!   at a start or a match, so `base` is that one record.
//! - `players` and `weapon/<id>` (`load_players`, `load_weapon`): the same
//!   donor map, nothing more than `base` (every Halo weapon and body lives
//!   in it; per-weapon sets would all be that one record). Halo has no
//!   guns among its parts yet (`parts::PARTS`), so no `weapon/<id>` set is
//!   recorded.
//! - `map/<name>` per multiplayer map (`load_map`): the map converted as
//!   another game's gameplay loads it (lean, no kit), which reads exactly
//!   its own file (`maps/<name>.map`): Halo's maps are self-contained.
//!   The packer drops what `base` holds from a map's set unless that is
//!   all of it, so Blood Gulch's set is its file too, the same record as
//!   `base` (fetched once).
//!
//! The party's asset offer needs nothing of Halo's own: a match on
//! `halo_ce:<name>` requires the game, and the offer names each game in
//! play's `base`, `players` and `common` and the map's `map/<name>`
//! (`chimera_net::net::assets::offer`).

use blam_formats::cache::MapFile;

use super::maps::{load_in, map_path};
use crate::{
    map::{KitLoad, MapLoadOptions},
    mount::Mount,
};

/// The map Halo's guns and body are read from on any map: Blood Gulch
/// (docs/plans/active/halo-ce.md, "What the specs settled").
pub const DONOR_MAP: &str = "bloodgulch";

/// `base`: the donor map read and checked as a map file.
pub fn load_base(mount: &Mount) -> Result<(), String> {
    let path = map_path(DONOR_MAP);
    let bytes = mount.read(&path).map_err(|e| format!("{path}: {e}"))?;
    MapFile::load(&bytes).map(|_| ()).map_err(|e| format!("{path}: {e}"))
}

/// `players`: what Halo's players need on any map, the donor map
/// (`base`).
pub fn load_players(mount: &Mount) -> Result<(), String> {
    load_base(mount)
}

/// `weapon/<id>`: what weapon `id` needs on any map, the donor map
/// (`base`).
pub fn load_weapon(mount: &Mount, _id: &str) -> Result<(), String> {
    load_base(mount)
}

/// `map/<name>`: map `name` converted as another game's gameplay loads it
/// (no kit, textures not decoded: the same file is read either way),
/// never from the converted-map cache.
pub fn load_map(mount: &Mount, name: &str) -> Result<(), String> {
    let options = MapLoadOptions { textures: false, kit: KitLoad::Players(&[]), ..Default::default() };
    load_in(mount, name, &options, None).map(|_| ())
}
