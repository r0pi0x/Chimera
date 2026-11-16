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
//! Also here, what a match on a slice does beyond the wall
//! (`chimera_world::map::play_area`): map pickups outside the play area
//! are not placed (`drop_pickups_outside`, `weapon::respawn::MapPickup`); a
//! loose item (a dropped weapon) found outside is put back at the nearest
//! point inside (`put_loose_back_inside`); and a slice whose map gives it
//! fewer spawn points than `[slice] points_want` (chimera/sim/data/
//! map_fit.toml) makes more once its spots are made (`arena_spawn_points`,
//! `slice_points`: `slice_safety::choose_points` seeded by the slice's
//! key, so every machine with the same map, slice and spots makes the
//! same; nothing more goes over the network).

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

/// A spawn point a slice made for itself (`arena_spawn_points`).
#[derive(Component, Clone, Copy, Debug)]
pub struct ArenaMadeSpawn;

/// The slice (its key) whose spawn points are made on the loaded map
/// (map-scoped).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct ArenaSpawnsMade(pub String);

/// A line-of-sight test against the world's solid layers, characters
/// left out (`slice_safety::Sees`).
pub fn sees_in<'a>(q: &'a avian3d::prelude::SpatialQuery, filter: &'a avian3d::prelude::SpatialQueryFilter) -> impl Fn(Vec3, Vec3) -> bool + 'a {
    move |a: Vec3, b: Vec3| {
        let d = b - a;
        let len = d.length();
        match Dir3::new(d) {
            Ok(dir) => q.cast_ray(a, dir, (len - 0.05).max(0.0), true, filter).is_none(),
            Err(_) => true,
        }
    }
}

/// The spawn points a slice adds to `existing` (its map's inside and its
/// seed's) from the floor `spots`, as `slice_safety::choose_points` makes
/// them with the shipped `[slice]` rules, seeded by the slice's key: the
/// same on every machine with the same map, slice and spots.
pub fn slice_points(
    area: &PlayArea,
    spots: &[Vec3],
    existing: &[Vec3],
    q: &avian3d::prelude::SpatialQuery,
    characters: &[Entity],
) -> Vec<Vec3> {
    let rules = super::map_fit::FitSettings::shipped().slice.point_rules();
    let filter = avian3d::prelude::SpatialQueryFilter::from_mask(crate::core::SOLID_LAYERS).with_excluded_entities(characters.iter().copied());
    let sees = sees_in(q, &filter);
    let seed = crate::weapon::random::crc32(area.slice.key().as_bytes());
    super::slice_safety::choose_points(spots, existing, &area.polygon, &sees, rules, seed)
}

/// Once the slice's spawn spots are made (`spawns::SpawnSpots`), more
/// spawn points where the map gives it fewer than `[slice] points_want`
/// (`slice_points`): `SpawnPoint`s inside (`ArenaMadeSpawn`), for the
/// modes that spawn on points, a character put back inside, and the
/// spots' and bots' floods of a later match. The server's (a client
/// spawns nobody); made once per slice on the loaded map.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn arena_spawn_points(
    area: Option<Res<PlayArea>>,
    spots: Option<Res<super::spawns::SpawnSpots>>,
    made: Option<Res<ArenaSpawnsMade>>,
    points: Query<&Transform, (With<crate::core::SpawnPoint>, Without<crate::core::Intent>, Without<ArenaMadeSpawn>)>,
    characters: Query<Entity, With<crate::core::Intent>>,
    q: avian3d::prelude::SpatialQuery,
    (role, playback): (Option<Res<crate::core::NetRole>>, Option<Res<crate::core::Playback>>),
    mut commands: Commands,
) {
    let (Some(area), Some(spots)) = (area, spots) else { return };
    if !spots.ready || !spots.from_nav || made.is_some_and(|m| m.0 == area.slice.key()) || !crate::core::authoritative(role, playback) {
        return;
    }
    let existing: Vec<Vec3> = points.iter().map(|t| t.translation - Vec3::Y * crate::map::SPAWN_LIFT).collect();
    let mut existing = existing;
    existing.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z)));
    let characters: Vec<Entity> = characters.iter().collect();
    let new = slice_points(&area, &spots.spots, &existing, &q, &characters);
    info!(
        "arena {}: {} spawn points inside, {} made (spots {})",
        area.slice.key(),
        existing.len(),
        new.len(),
        spots.spots.len()
    );
    for p in new {
        commands.spawn((
            Name::new("Arena spawn point"),
            ArenaMadeSpawn,
            crate::map::play_area::HeldSpawn,
            crate::map::MapPart,
            crate::core::SpawnPoint { team: None },
            Transform::from_translation(p + Vec3::Y * crate::map::SPAWN_LIFT),
        ));
    }
    commands.insert_resource(ArenaSpawnsMade(area.slice.key()));
}

/// How far inside (m) a loose item outside is put back.
pub const LOOSE_BACK_MARGIN: f32 = 1.0;

/// A loose item (a dropped weapon or item, `weapon::drop::Loose`) whose
/// x, z is outside the play area is put back at the nearest point inside
/// (`PlanPolygon::nearest_inside`, `LOOSE_BACK_MARGIN` from the wall) at
/// its height, its motion stopped: the wall stops a thrown one, so only
/// one that came out of a body against the wall or through a gap of the
/// wall's corners ends here. The server's (a client is shown its place).
#[allow(clippy::type_complexity)]
pub fn put_loose_back_inside(
    area: Option<Res<PlayArea>>,
    role: Option<Res<crate::core::NetRole>>,
    playback: Option<Res<crate::core::Playback>>,
    mut loose: Query<
        (
            &mut Transform,
            Option<&mut avian3d::prelude::Position>,
            Option<&mut avian3d::prelude::LinearVelocity>,
        ),
        With<crate::weapon::drop::Loose>,
    >,
) {
    let Some(area) = area else { return };
    if !crate::core::authoritative(role, playback) {
        return;
    }
    for (mut t, pos, vel) in &mut loose {
        let at = t.translation;
        if area.polygon.contains3(at) {
            continue;
        }
        let back = area.polygon.nearest_inside(Vec2::new(at.x, at.z), LOOSE_BACK_MARGIN);
        let to = Vec3::new(back.x, at.y, back.y);
        t.translation = to;
        if let Some(mut p) = pos {
            p.0 = to;
        }
        if let Some(mut v) = vel {
            v.0 = Vec3::ZERO;
        }
        debug!("arena: a loose item put back inside, from {at} to {to}");
    }
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
