//! CS:S guns' bullet holes drawn in the party (the user's installs, headless
//! with the render assets decals need): a CS:S gun's shot at a de_dust2
//! wall leaves a drawn decal (`render::decal::RuntimeDecal`), for a
//! holder of either game's identity, after hundreds of decals (the oldest
//! recycled past `mp_decals`), and after the map changes away and back.
//! Before, the party's CS:S maps (loaded for Combat Arms' gameplay) had
//! their kit's impact decals stripped, while their impacts were still
//! CS:S's to draw: no bullet holes from CS:S guns on CS:S maps. Passes
//! with a note without both installs in chimera.local.toml.

use std::collections::HashSet;

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::{
    core::{SOLID_LAYERS, SpawnPoint},
    mount::config::LocalConfig,
};
use chimera_present::{
    MapPlugin,
    render::{
        decal::{DecalMaterial, RuntimeDecal},
        prop_material::PropMaterial,
        rope_material::RopeMaterial,
        sprite_material::SpriteMaterial,
        world_material::WorldMaterial,
    },
};
use chimera_sim::testkit;
use chimera_world::{
    harness::Sim,
    map::{self, DecalGroup, PlaceDecal},
};
use combat_arms::harness::CaSim;
use party_kit::PartyPartsPlugin;

/// The render assets the map spawns its meshes and decals with.
fn render_assets(app: &mut App) {
    app.init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .init_asset::<WorldMaterial>()
        .init_asset::<PropMaterial>()
        .init_asset::<SpriteMaterial>()
        .init_asset::<RopeMaterial>()
        .init_asset::<DecalMaterial>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
}

fn installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    let css = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config.game_path(combat_arms::game::GAME).is_some_and(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if !(css && ca) {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
    }
    css && ca
}

/// The party, drawn, on de_dust2.
fn party() -> Option<Sim> {
    if !installed() {
        return None;
    }
    let map = party_kit::load_map("cs_source:de_dust2", 0).unwrap();
    let mut sim = Sim::new(CaSim, (render_assets, MapPlugin::new(map), PartyPartsPlugin));
    sim.ticks(2);
    Some(sim)
}

/// A player of `movement`'s game at the map's first spawn holding `gun`,
/// and a wall point 2-25 m from its eye.
fn shooter(sim: &mut Sim, movement: &'static str, gun: &str) -> (Entity, Vec3) {
    let at = {
        let w = sim.app.world_mut();
        let mut q = w.query::<(&Transform, &SpawnPoint)>();
        q.iter(w).next().map(|(t, _)| t.translation).expect("a spawn point")
    };
    let p = sim.spawn_character(at, movement);
    sim.seconds(1.0);
    chimera_sim::weapon::give(sim.app.world_mut(), p, gun);
    sim.seconds(1.5);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(gun));
    (p, wall(sim, p))
}

fn wall(sim: &mut Sim, p: Entity) -> Vec3 {
    let eye = sim.position(p) + sim.state(p).eye_offset;
    let mut state: SystemState<SpatialQuery> = SystemState::new(sim.app.world_mut());
    let spatial = state.get(sim.app.world()).unwrap();
    let filter = SpatialQueryFilter::from_excluded_entities([p]).with_mask(SOLID_LAYERS);
    (0..8)
        .filter_map(|i| {
            let a = i as f32 * std::f32::consts::FRAC_PI_4;
            let dir = Dir3::new(Vec3::new(a.cos(), 0.0, a.sin())).ok()?;
            let hit = spatial.cast_ray(eye, dir, 25.0, true, &filter)?;
            (hit.distance > 2.0).then(|| eye + *dir * hit.distance)
        })
        .next()
        .expect("a wall within 25 m")
}

fn decals(sim: &mut Sim) -> HashSet<Entity> {
    let w = sim.app.world_mut();
    let mut q = w.query_filtered::<Entity, With<RuntimeDecal>>();
    q.iter(w).collect()
}

/// One round from `p` at `at`: the decals drawn that weren't before.
fn new_decals(sim: &mut Sim, p: Entity, at: Vec3) -> usize {
    testkit::aim_at(sim, p, at);
    sim.seconds(0.5);
    let before = decals(sim);
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    sim.seconds(0.5);
    decals(sim).difference(&before).count()
}

#[test]
fn cs_source_guns_leave_drawn_bullet_holes_in_the_party() {
    for (movement, gun) in [
        (combat_arms::game::movement::ID, "cs_source:weapon_mp5navy"),
        (combat_arms::game::movement::ID, "cs_source:weapon_elite"),
        (cs_source::game::movement::ID, "cs_source:weapon_p90"),
    ] {
        let Some(mut sim) = party() else { return };
        let (p, at) = shooter(&mut sim, movement, gun);
        assert!(new_decals(&mut sim, p, at) > 0, "{gun} held by a {movement} player: a bullet hole");
    }
}

#[test]
fn bullet_holes_still_come_after_hundreds_of_decals() {
    let Some(mut sim) = party() else { return };
    let (p, at) = shooter(&mut sim, cs_source::game::movement::ID, "cs_source:weapon_mp5navy");
    let normal = (sim.position(p) - at).with_y(0.0).normalize();
    for i in 0..600 {
        sim.app.world_mut().write_message(PlaceDecal {
            target: None,
            group: DecalGroup::Material('C'),
            point: at + Vec3::Y * ((i % 20) as f32 * 0.05 - 0.5),
            normal,
            dir: -normal,
            spin: false,
            round: false,
        });
        if i % 50 == 0 {
            sim.ticks(1);
        }
    }
    sim.ticks(2);
    assert!(new_decals(&mut sim, p, at) > 0, "a bullet hole after 600 decals");
}

#[test]
fn bullet_holes_still_come_after_the_map_changes() {
    let Some(mut sim) = party() else { return };
    let (p, at) = shooter(&mut sim, cs_source::game::movement::ID, "cs_source:weapon_mp5navy");
    assert!(new_decals(&mut sim, p, at) > 0, "before the change");
    for id in ["combat_arms:warehouse", "cs_source:de_dust2"] {
        let data = party_kit::load_map(id, 0).unwrap();
        map::change_map(sim.app.world_mut(), data, map::MapDebugView::Normal);
        sim.ticks(4);
    }
    let (p, at) = shooter(&mut sim, cs_source::game::movement::ID, "cs_source:weapon_mp5navy");
    assert!(new_decals(&mut sim, p, at) > 0, "a bullet hole after warehouse and back to de_dust2");
    // The same map again (a restart by changelevel).
    let data = party_kit::load_map("cs_source:de_dust2", 0).unwrap();
    map::change_map(sim.app.world_mut(), data, map::MapDebugView::Normal);
    sim.ticks(4);
    let (p, at) = shooter(&mut sim, combat_arms::game::movement::ID, "cs_source:weapon_p90");
    assert!(new_decals(&mut sim, p, at) > 0, "a bullet hole after a restart of de_dust2");
}

/// The party's CS:S maps carry CS:S's impact decals (its impact looks),
/// as its Combat Arms maps do as a guest's.
#[test]
fn the_partys_maps_carry_cs_sources_impact_decals() {
    if !installed() {
        return;
    }
    for id in ["cs_source:de_dust2", "combat_arms:warehouse"] {
        let m = party_kit::load_map(id, 0).unwrap();
        assert!(m.impacts.has(cs_source::game::GAME), "{id}");
        let decals = &m.kit.decals;
        assert!(!decals.groups.is_empty() && decals.by_material.contains_key(&'C'), "{id}: CS:S's decal groups");
    }
}
