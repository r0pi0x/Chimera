//! A TF2 gun's impacts on another game's map take TF2's own looks
//! (`map::impact::own_name`): with CS:S's looks on the map under the same
//! names (every Source game's `Default.BulletImpact`, its 'C' decal group,
//! its `flesh` surface), as on de_dust2 with TF2's as a guest
//! (`map::kept::append`), a TF2 pistol's hit on the world plays TF2's
//! impact entry and asks for TF2's decal group, and its hit on a player
//! plays TF2's flesh impact (TF2 has no hurt sounds of its own).
//! The same hit asks for TF2's flesh decal on the body (effects.md 8b),
//! none in low violence.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_world::{
    greybox,
    harness::Sim,
    map::{
        DecalGroup, MapData, PlaceBodyDecal, PlaceDecal, PlaySound,
        decal::{DecalBlend, MapDecal},
        impact::{ActiveImpacts, MapImpacts},
        kept,
        sound::{Interval, MapSoundClip, MapSoundEntry, MapSurface, SoundBank, SoundLevel},
    },
};
use chimera_sim::testkit;
use chimera_sim::weapon::{WeaponEvent, WeaponEventKind};
use tf2::game::weapons::{PISTOL_SCOUT, SYRINGE_GUN};

use super::weapons::{holding, sim, target};

/// One game's impact looks under the names every Source game uses; `tag`
/// tells whose (the clip's rate, the decal's size).
fn looks(game: &str, tag: usize) -> MapData {
    let mut map = MapData::default();
    let sounds = Arc::make_mut(&mut map.sounds);
    for (surface, entry, letter) in [("default", "Default.BulletImpact", 'C'), ("flesh", "Flesh.BulletImpact", 'F')] {
        sounds.surfaces.insert(
            surface.into(),
            MapSurface {
                game_material: letter,
                bullet_impact: Some(entry.into()),
                ..default()
            },
        );
        sounds.entries.insert(
            entry.to_lowercase(),
            MapSoundEntry {
                waves: vec![sounds.clips.len()],
                volume: Interval::fixed(1.0),
                pitch: Interval::fixed(100.0),
                level: SoundLevel::Db(75.0),
                channel: 0,
                dry: false,
                hearing: None,
            },
        );
        sounds.clips.push(MapSoundClip {
            rate: tag as u32,
            channels: 1,
            samples: Arc::from(&[0i16][..]),
            loop_start: None,
        });
    }
    map.kit.decals.decals.push(MapDecal {
        texture: 0,
        uv_min: Vec2::ZERO,
        uv_max: Vec2::ONE,
        size: Vec2::splat(tag as f32),
        blend: DecalBlend::Modulate2x,
        fog_fade: chimera_world::map::decal::SOURCE_FOG_FADE,
    });
    map.kit.decals.groups.insert("impact.concrete".into(), vec![(1.0, 0)]);
    map.kit.decals.by_material.insert('C', "impact.concrete".into());
    map.impacts.add(game);
    map
}

/// A CS:S map with TF2's looks on it as a guest's.
fn cs_map_with_tf2s() -> MapData {
    let mut map = looks("cs_source", 1);
    map.impacts.game = Some("cs_source".into());
    kept::append(&mut map, &looks("tf2", 2));
    map
}

/// A TF2 pistol's one shot at `at` on `map`'s looks: the sounds played
/// and the decals asked for.
fn shoot(map: &MapData, at: impl FnOnce(&mut Sim, Vec3) -> Vec3) -> (Vec<String>, Vec<DecalGroup>) {
    shoot_with(PISTOL_SCOUT, 10, map, at).0
}

/// `gun`'s one shot at `at` on `map`'s looks over `ticks`: the sounds
/// played and decals asked for, and the projectiles' marks shown (the
/// weapon events' `Marked`).
fn shoot_with(
    gun: &str,
    ticks: u32,
    map: &MapData,
    at: impl FnOnce(&mut Sim, Vec3) -> Vec3,
) -> ((Vec<String>, Vec<DecalGroup>), Vec<WeaponEventKind>) {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, gun);
    let aim = at(&mut sim, from);
    sim.app.insert_resource(SoundBank(map.sounds.clone()));
    sim.app.insert_resource(ActiveImpacts(MapImpacts::clone(&map.impacts)));
    testkit::aim_at(&mut sim, p, aim);
    let mut sounds = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
    let mut decals = sim.app.world().resource::<Messages<PlaceDecal>>().get_cursor_current();
    let mut events = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let (mut played, mut placed, mut marks) = (Vec::new(), Vec::new(), Vec::new());
    for k in 0..ticks {
        sim.intent(p).fire = k == 0;
        sim.ticks(1);
        let w = sim.app.world();
        played.extend(sounds.read(w.resource::<Messages<PlaySound>>()).map(|s| s.entry.clone()));
        placed.extend(decals.read(w.resource::<Messages<PlaceDecal>>()).map(|d| d.group.clone()));
        marks.extend(
            events
                .read(w.resource::<Messages<WeaponEvent>>())
                .filter(|e| matches!(e.kind, WeaponEventKind::Marked { .. }))
                .map(|e| e.kind.clone()),
        );
    }
    ((played, placed), marks)
}

/// effects.md, "Needles on a wall": a needle leaves no needle but a
/// bullet's hole mark and debris where it touched (`marks`), as TF2's
/// impact (its decal group, its surface's sound), once.
#[test]
fn a_needle_marks_the_floor_it_touches_as_a_bullet_would() {
    let map = cs_map_with_tf2s();
    let ((played, placed), marks) = shoot_with(SYRINGE_GUN, 40, &map, |_, from| from + Vec3::new(0.0, -3.0, -2.0));
    assert_eq!(marks.len(), 1, "one mark: {marks:?}");
    let WeaponEventKind::Marked { id, normal, .. } = &marks[0] else { unreachable!() };
    assert_eq!(*id, SYRINGE_GUN);
    assert!(normal.y > 0.9, "on the floor: {normal}");
    assert!(played.iter().any(|s| s == "tf2:default.bulletimpact"), "TF2's impact sound: {played:?}");
    assert_eq!(
        placed.iter().filter(|g| matches!(g, DecalGroup::GameMaterial(game, 'C') if game == "tf2")).count(),
        1,
        "TF2's hole decal: {placed:?}"
    );
}

#[test]
fn a_tf2_guns_hit_on_another_games_map_is_tf2s_own() {
    let map = cs_map_with_tf2s();
    // At the floor ahead.
    let (played, placed) = shoot(&map, |_, from| from + Vec3::new(0.0, -3.0, -2.0));
    assert!(played.iter().any(|s| s == "tf2:default.bulletimpact"), "TF2's impact: {played:?}");
    assert!(!played.iter().any(|s| s.eq_ignore_ascii_case("Default.BulletImpact")), "not CS:S's: {played:?}");
    let group = placed.iter().find_map(|g| match g {
        DecalGroup::GameMaterial(game, letter) if game == "tf2" => Some(*letter),
        _ => None,
    });
    let letter = group.unwrap_or_else(|| panic!("TF2's decal group: {placed:?}"));
    // Resolved as the renderer does: TF2's bullet hole, not CS:S's.
    let decals = &map.kit.decals;
    let name = decals.material_group(Some("tf2"), letter).unwrap();
    assert_eq!(decals.decals[decals.groups[name][0].1].size.x, 2.0, "{name}");
    // The entry is TF2's clip.
    let entry = map.sounds.entry("tf2:default.bulletimpact").unwrap();
    assert_eq!(map.sounds.clips[entry.waves[0]].rate, 2);
}

#[test]
fn a_tf2_guns_hit_on_a_player_sounds_tf2s_flesh_impact() {
    let map = cs_map_with_tf2s();
    let (played, _) = shoot(&map, |sim, from| {
        let t = target(sim, from, 100.0);
        sim.position(t)
    });
    assert!(played.iter().any(|s| s == "tf2:flesh.bulletimpact"), "TF2's flesh impact: {played:?}");
    assert!(!played.iter().any(|s| s.eq_ignore_ascii_case("Flesh.BulletImpact")), "not CS:S's: {played:?}");
}

/// A TF2 pistol's shot at a player standing ahead: the body decals asked
/// for (target, group), with `violence_hblood` at `hblood`.
fn body_decals(hblood: i32) -> (Entity, Vec<(Entity, DecalGroup)>) {
    let map = cs_map_with_tf2s();
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, PISTOL_SCOUT);
    let t = target(&mut sim, from, 100.0);
    let aim = sim.position(t);
    sim.app.insert_resource(SoundBank(map.sounds.clone()));
    sim.app.insert_resource(ActiveImpacts(MapImpacts::clone(&map.impacts)));
    sim.app.insert_resource(source_engine::effects::impact_effects::ImpactEffectSettings {
        violence_hblood: hblood,
        ..default()
    });
    testkit::aim_at(&mut sim, p, aim);
    let mut asks = sim.app.world().resource::<Messages<PlaceBodyDecal>>().get_cursor_current();
    let mut placed = Vec::new();
    for k in 0..10 {
        sim.intent(p).fire = k == 0;
        sim.ticks(1);
        let w = sim.app.world();
        placed.extend(asks.read(w.resource::<Messages<PlaceBodyDecal>>()).map(|d| (d.target, d.group.clone())));
    }
    (t, placed)
}

/// effects.md 8b: each bullet that hits an enemy puts TF2's flesh decal
/// (`Impact.Flesh`: the F letter's group) on his body; none in low
/// violence.
#[test]
fn a_tf2_bullet_decals_the_body_but_not_in_low_violence() {
    let (t, placed) = body_decals(1);
    assert_eq!(placed.len(), 1, "{placed:?}");
    assert_eq!(placed[0].0, t);
    assert!(matches!(&placed[0].1, DecalGroup::GameMaterial(g, 'F') if g == "tf2"), "{placed:?}");
    let (_, placed) = body_decals(0);
    assert!(placed.is_empty(), "violence_hblood 0: {placed:?}");
}
