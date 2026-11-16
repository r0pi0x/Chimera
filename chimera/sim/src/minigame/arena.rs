//! Arena slices as data (docs/plans/active/arena-slices.md, section 3): a
//! game's `data/arena_slices.toml`, registered beside its pool rules
//! (`map_fit::MapPoolAppExt::add_arena_slices`) into one `ArenaSlices`
//! for every game, each slice a sub-map of its map (`halo_ce:bloodgulch`
//! is the parent, `halo_ce:bloodgulch@west_base` the slice). Game-neutral:
//! any game states slices of its maps; what playing one does is
//! `chimera_world::map::play_area`'s, which plays the slice the app puts
//! in `ArenaPlan` (`plan`: the party kit's `arena` command).
//!
//! The file: `[[slice]]` entries with `map` (the map's name without the
//! game), `id` (unique per map), `name` (the slice's own, "West Base"),
//! optional `map_name` (the map's display name, "Blood Gulch"; else
//! `hud::map_display_name`'s), `points` (the polygon in plan view, engine x
//! and z in metres), `seed` (engine x, y, z on a floor inside), optional
//! `players = [lo, hi]` and `note`, and the measurements a tool writes
//! (`area_m2`, `sight_m`, ...: kept, not read). The shown name is made:
//! "<map's display name>: <name>" (the owner's decision 6).
//!
//! Also here: map pickups outside the play area are not placed
//! (`drop_pickups_outside`, `weapon::respawn::MapPickup`).

use bevy::prelude::*;
use serde::Deserialize;

// Re-exported for the layers above `minigame` (`minigame_net`).
pub use crate::map::play_area::{ArenaPlan, ArenaSlice, PlayArea};

/// One file's entry (module doc).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    map: String,
    id: String,
    name: String,
    #[serde(default)]
    map_name: Option<String>,
    points: Vec<[f32; 2]>,
    seed: [f32; 3],
    #[serde(default)]
    players: Option<[u32; 2]>,
    #[serde(default)]
    note: Option<String>,
    // The tool's measurements (read by people and the pool later).
    #[serde(default)]
    area_m2: Option<f64>,
    #[serde(default)]
    sight_m: Option<f64>,
    #[serde(default)]
    spots: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    slice: Vec<Entry>,
}

/// A registered slice: the shape and how many players it is drawn for.
#[derive(Clone, Debug, PartialEq)]
pub struct SliceDef {
    pub slice: ArenaSlice,
    pub players: Option<[u32; 2]>,
}

/// Every game's slices (module doc), in registration order.
#[derive(Resource, Clone, Debug, Default)]
pub struct ArenaSlices(pub Vec<SliceDef>);

impl ArenaSlices {
    /// The slice named `name`: `game:map@id`, `map@id`, or `id` alone when
    /// only one map has it (case ignored).
    pub fn find(&self, name: &str) -> Result<&SliceDef, String> {
        let (map, id) = match name.rsplit_once('@') {
            Some((m, i)) => (Some(m), i),
            None => (None, name),
        };
        let hits: Vec<&SliceDef> = self
            .0
            .iter()
            .filter(|d| d.slice.id.eq_ignore_ascii_case(id))
            .filter(|d| {
                map.is_none_or(|m| {
                    d.slice.map.eq_ignore_ascii_case(m) || d.slice.map.split_once(':').is_some_and(|(_, bare)| bare.eq_ignore_ascii_case(m))
                })
            })
            .collect();
        match hits[..] {
            [one] => Ok(one),
            [] => Err(format!("no arena slice `{name}` (arena list names them)")),
            _ => Err(format!("`{name}` names several slices: say the map too (map@id)")),
        }
    }
}

/// `text` (a game's `arena_slices.toml`) as `game`'s slices, each checked
/// (`ArenaSlice::polygon`: 3 to 64 corners, simple, the seed inside; ids
/// unique per map).
pub fn parse(game: &str, text: &str) -> Result<Vec<SliceDef>, String> {
    let file: File = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut out: Vec<SliceDef> = Vec::new();
    for e in file.slice {
        let _ = (&e.note, e.area_m2, e.sight_m, e.spots);
        let map = if e.map.contains(':') { e.map.clone() } else { format!("{game}:{}", e.map) };
        let title = e.map_name.clone().unwrap_or_else(|| super::hud::map_display_name(&map));
        let slice = ArenaSlice {
            name: format!("{title}: {}", e.name),
            map,
            id: e.id,
            points: e.points.iter().map(|p| Vec2::new(p[0], p[1])).collect(),
            seed: Vec3::from_array(e.seed),
        };
        slice.polygon()?;
        if out.iter().any(|d| d.slice.map == slice.map && d.slice.id == slice.id) {
            return Err(format!("{}: twice", slice.key()));
        }
        if let Some([lo, hi]) = e.players
            && (lo < 2 || hi < lo)
        {
            return Err(format!("{}: players [{lo}, {hi}]", slice.key()));
        }
        out.push(SliceDef { slice, players: e.players });
    }
    Ok(out)
}

/// Registers `game`'s slices (panics on a bad file: shipped data).
pub fn add(world: &mut World, game: &str, name: &str, text: &str) {
    let defs = parse(game, text).unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut all = world.get_resource_or_init::<ArenaSlices>();
    for d in defs {
        if !all.0.iter().any(|x| x.slice.key() == d.slice.key()) {
            all.0.push(d);
        }
    }
}

/// Plays slice `name` (`ArenaSlices::find`) on its map from the next time
/// that map is in the world (or now, if it is and nothing was built on it
/// yet: `play_area::follow_plan`); None stops playing slices. Returns the
/// slice.
pub fn plan(world: &mut World, name: Option<&str>) -> Result<Option<ArenaSlice>, String> {
    let slice = match name {
        Some(n) => Some(world.get_resource::<ArenaSlices>().ok_or("no arena slices registered")?.find(n)?.slice.clone()),
        None => None,
    };
    world.insert_resource(ArenaPlan(slice.clone()));
    Ok(slice)
}

/// Map pickups outside the play area are not placed: gone as they appear.
pub fn drop_pickups_outside(
    area: Option<Res<PlayArea>>,
    pickups: Query<(Entity, &GlobalTransform), With<crate::weapon::respawn::MapPickup>>,
    mut commands: Commands,
) {
    let Some(area) = area else { return };
    for (e, t) in &pickups {
        if !area.polygon.contains3(t.translation()) {
            commands.entity(e).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = r#"
[[slice]]
map = "bloodgulch"
id = "west_base"
name = "West Base"
map_name = "Blood Gulch"
points = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]
seed = [5.0, 1.0, 5.0]
players = [2, 6]
"#;

    #[test]
    fn a_slice_is_a_sub_map_with_a_made_name() {
        let defs = parse("halo_ce", TEXT).unwrap();
        let s = &defs[0].slice;
        assert_eq!(s.key(), "halo_ce:bloodgulch@west_base");
        assert_eq!(s.name, "Blood Gulch: West Base");
        let all = ArenaSlices(defs.clone());
        for n in ["halo_ce:bloodgulch@west_base", "bloodgulch@west_base", "WEST_BASE"] {
            assert_eq!(all.find(n).unwrap().slice.id, "west_base", "{n}");
        }
        assert!(all.find("sidewinder@west_base").is_err());
    }

    #[test]
    fn bad_slices_are_refused() {
        assert!(parse("g", &TEXT.replace("[5.0, 1.0, 5.0]", "[50.0, 1.0, 5.0]")).is_err(), "seed outside");
        assert!(parse("g", &TEXT.replace("[2, 6]", "[6, 2]")).is_err(), "players");
        assert!(parse("g", &format!("{TEXT}{TEXT}")).is_err(), "twice");
        assert!(parse("g", &TEXT.replace("name =", "nmae =")).is_err(), "unknown key");
    }
}
