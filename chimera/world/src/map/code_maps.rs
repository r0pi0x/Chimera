//! The maps built in code (`CODE_MAPS`: the greybox, `chimera_world::
//! greybox`, and the range, `chimera_world::range`): their ids, how a map
//! name reads as one (`code_map`), which one the greybox's slot spawns
//! (`CodeMap`) and the id of a code-built map's kit (`kit_id`,
//! `sources::CODE_MAP_KIT`). Here, below the code-built maps, so the map's
//! loading and the network can read them.

use bevy::prelude::*;

/// The range's map id (`chimera_world::range`).
pub const RANGE: &str = "range";

/// The greybox's map id.
pub const GREYBOX: &str = "greybox";

/// The maps built in code, by id: the greybox and the range.
pub const CODE_MAPS: [&str; 2] = [GREYBOX, RANGE];

/// Which code-built map `GreyboxMapPlugin` and `respawn` spawn (one of
/// `CODE_MAPS`; the greybox by default).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeMap(pub &'static str);

impl Default for CodeMap {
    fn default() -> Self {
        Self(GREYBOX)
    }
}

/// The code-built map a map name stands for: the greybox for none,
/// `greybox`, `chimera:greybox` and the old `mashup:greybox`; the range
/// for `range` and `chimera:range` (any case). None: another map.
pub fn code_map(name: &str) -> Option<&'static str> {
    let bare = match name.split_once(':') {
        Some((game, map)) if game.eq_ignore_ascii_case("chimera") || game.eq_ignore_ascii_case("mashup") => map,
        Some(_) => return None,
        None => name,
    };
    if bare.is_empty() {
        return Some(GREYBOX);
    }
    CODE_MAPS.into_iter().find(|m| m.eq_ignore_ascii_case(bare))
}

/// The map id of code-built map `id`'s kit (`sources::CODE_MAP_KIT`:
/// the bodies and guns an app's maps are dressed with), `chimera:<id>`.
pub fn kit_id(id: &str) -> String {
    format!("{}:{id}", super::sources::CODE_MAP_GAME)
}


/// A map whose players spawn at its spawn points only (the range's firing
/// line): spawning on spots (`points = "scored"`, `"random_hidden"`,
/// chimera_sim's `minigame::spawns`) takes the map's spawn points as its
/// spots instead of the floor's, and with `facing` every spawn faces that
/// way (flat; the range's: down range). Inserted with the map, removed
/// with it.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct SpawnOnPoints {
    pub facing: Option<Vec3>,
}
