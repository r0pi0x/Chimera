//! Source's ladder entities read into the movement's ladder lines
//! (specs/hl2dm/movement.md "The ladder entity"): `func_useableladder`s
//! with their `info_ladder_dismount` nodes (`ladders_of`, Source units),
//! kept in `EntityLadders` as the map's entities change (`read_ladders`,
//! before `useable_ladders::build_ladders`, which takes them as the map's
//! own lines). Source's part of the ladder lines: climbing them is
//! `useable_ladders`, any engine's.

use bevy::prelude::*;

use crate::{
    map::{MapEntities, MapEntity},
    source_movement::useable_ladders::{self, EntityLadders, LadderFrom, UseableLadder},
};

/// The dismount list: nodes this near the ends or the points sampled along
/// the ladder, at this spacing.
const DISMOUNT_LIST: f32 = 100.0;
const DISMOUNT_SAMPLE: f32 = 40.0;

/// The reader as a system, once per app (`SourceMovementAppExt::
/// source_movement` adds it).
pub struct SourceLaddersPlugin;

impl Plugin for SourceLaddersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EntityLadders>()
            .add_systems(First, read_ladders.before(useable_ladders::build_ladders));
    }
}

/// Keep `EntityLadders` the map's entities' (`map::MapEntities`): read
/// again when they change, emptied when they go.
pub fn read_ladders(entities: Option<Res<MapEntities>>, mut read: ResMut<EntityLadders>) {
    match &entities {
        Some(e) if e.is_changed() => read.0 = ladders_of(&e.entities),
        None if !read.0.is_empty() => read.0.clear(),
        _ => {}
    }
}

/// The ladders among `entities` (Source units), with their dismount lists
/// (movement.md "The ladder entity").
pub fn ladders_of(entities: &[MapEntity]) -> Vec<UseableLadder> {
    let nodes: Vec<(Vec3, Option<String>)> = entities
        .iter()
        .filter(|e| e.classname().eq_ignore_ascii_case("info_ladder_dismount"))
        .map(|e| (e.origin(), e.get("target").map(str::trim).filter(|t| !t.is_empty()).map(str::to_lowercase)))
        .collect();
    let named = |name: &str| {
        entities
            .iter()
            .position(|e| e.get("targetname").is_some_and(|t| t.eq_ignore_ascii_case(name)))
    };
    entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.classname().eq_ignore_ascii_case("func_useableladder"))
        .map(|(i, e)| {
            let (a, b) = (e.vector("point0"), e.vector("point1"));
            let (top, bottom) = if a.z >= b.z { (a, b) } else { (b, a) };
            let mut samples = vec![top, bottom];
            let up = (top - bottom).normalize_or_zero();
            let len = (top - bottom).length();
            let mut s = DISMOUNT_SAMPLE;
            while s < len {
                samples.push(bottom + up * s);
                s += DISMOUNT_SAMPLE;
            }
            let dismounts = nodes
                .iter()
                .filter(|(p, target)| {
                    // A node naming another entity (the first of that name)
                    // belongs to that one.
                    target.as_deref().is_none_or(|t| named(t) == Some(i))
                        && samples.iter().any(|s| s.distance(*p) <= DISMOUNT_LIST)
                })
                .map(|(p, _)| *p)
                .collect();
            let flags: i64 = e.get("spawnflags").and_then(|v| v.trim().parse().ok()).unwrap_or(0);
            let disabled = e.get("StartDisabled").and_then(|v| v.trim().parse::<i64>().ok()).unwrap_or(0) != 0;
            UseableLadder {
                map_index: i,
                top,
                bottom,
                enabled: !disabled,
                dont_get_on: flags & 1 != 0,
                dismounts,
                fixed: false,
                from: LadderFrom::Map,
                made: Vec::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(pairs: &[(&str, &str)]) -> MapEntity {
        MapEntity {
            keyvalues: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            ..Default::default()
        }
    }

    /// The ladder's ends (the higher point is the top), flags, and its
    /// dismount list: nodes within 100 of the ends or of the points every
    /// 40 up, not those naming another entity.
    #[test]
    fn ladders_and_their_dismount_points() {
        let ents = vec![
            entity(&[("classname", "func_useableladder"), ("targetname", "a"), ("point0", "0 0 8"), ("point1", "0 0 264")]),
            entity(&[("classname", "func_useableladder"), ("targetname", "b"), ("point0", "1000 0 0"), ("point1", "1000 0 100"), ("spawnflags", "1"), ("StartDisabled", "1")]),
            entity(&[("classname", "info_ladder_dismount"), ("origin", "48 0 264")]),
            // Beside the middle of a: within 100 of the point 120 up.
            entity(&[("classname", "info_ladder_dismount"), ("origin", "90 0 130")]),
            // Too far from anything of a.
            entity(&[("classname", "info_ladder_dismount"), ("origin", "300 0 130")]),
            // Near a, but naming b.
            entity(&[("classname", "info_ladder_dismount"), ("origin", "40 0 8"), ("target", "b")]),
        ];
        let l = ladders_of(&ents);
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].top, l[0].bottom), (Vec3::new(0.0, 0.0, 264.0), Vec3::new(0.0, 0.0, 8.0)));
        assert_eq!(l[0].dismounts, [Vec3::new(48.0, 0.0, 264.0), Vec3::new(90.0, 0.0, 130.0)]);
        assert!(l[0].enabled && !l[0].dont_get_on);
        assert!(!l[1].enabled && l[1].dont_get_on);
        assert!(l[1].dismounts.is_empty());
        // The line: closest points and the parameter from the top.
        assert_eq!(l[0].closest(Vec3::new(-20.0, 0.0, 0.0)), Vec3::new(0.0, 0.0, 8.0));
        assert!((l[0].param(Vec3::new(5.0, 0.0, 136.0)) - 0.5).abs() < 1e-6);
        assert!((l[0].line_distance(Vec3::new(3.0, 4.0, 50.0)) - 5.0).abs() < 1e-6);
    }
}
