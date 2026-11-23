//! A runtime decal over a map's own decal (gg_bk_warehouse_v1's blood
//! splat infodecals, feedback 04321aa6; the map from the user's content
//! cache, headless with the render assets decals need): the impact is one
//! blended item ordered after every map decal (`decal::IMPACT_ORDER`), with
//! nothing under it writing depth, so it multiplies over the map's decal
//! instead of cutting a hole in it. Skipped without the install or the
//! cached map.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use crate::stack::{
    games::{
        self, cs_source,
        cs_source::{movement::SourceMovementPlugin, weapons::CsWeaponsPlugin},
    },
    harness::{CsSim, Sim},
    map::{DecalGroup, MapData, MapPlugin, PlaceDecal},
    mount::config::LocalConfig,
    render::{
        blend_order::DecalOrder,
        decal::{DecalMaterial, IMPACT_ORDER, RuntimeDecal},
        prop_material::PropMaterial, rope_material::RopeMaterial, sprite_material::SpriteMaterial,
        world_material::WorldMaterial,
    },
};

const MAP: &str = "gg_bk_warehouse_v1";

fn load() -> Option<MapData> {
    let installed = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref()))
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let cached = ::cs_source::game::INFO.mount().is_some_and(|m| m.contains(&format!("maps/{MAP}.bsp")));
    if !installed || !cached {
        eprintln!("skipping: {MAP} not in the content cache (or no install)");
        return None;
    }
    Some(games::load_map(&format!("cs_source:{MAP}")).expect("load the map"))
}

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

/// Where the reported view looked (Source units: `setpos -405 -855 0`, yaw
/// 288): the wall with the map's blood splat.
fn reported_spot() -> Vec3 {
    cs_source::movement::to_engine(Vec3::new(-405.0, -855.0, 40.0))
}

#[test]
fn an_impact_on_a_map_decal_multiplies_over_it() {
    let Some(map) = load() else { return };
    let mut sim = Sim::new(CsSim, (render_assets, MapPlugin::new(map), SourceMovementPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    sim.seconds(1.0);
    // The map's decals drawn near the reported view, by their meshes'
    // centres.
    let world = sim.app.world_mut();
    // Each drawn map decal triangle: (entity, order, centre, normal).
    // Map decals of one material share a mesh (a chunk of many).
    let mut map_decals: Vec<(Entity, u32, Vec3, Vec3)> = Vec::new();
    {
        let mut q = world.query::<(Entity, &DecalOrder, &Mesh3d, &GlobalTransform)>();
        let meshes = world.resource::<Assets<Mesh>>();
        for (e, order, mesh, at) in q.iter(world) {
            let Some(m) = meshes.get(&mesh.0) else { continue };
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) = m.attribute(Mesh::ATTRIBUTE_POSITION) else {
                continue;
            };
            let p: Vec<Vec3> = p.iter().map(|v| at.transform_point(Vec3::from(*v))).collect();
            let Some(indices) = m.indices() else { continue };
            let idx: Vec<usize> = indices.iter().collect();
            for t in idx.chunks_exact(3) {
                let (a, b, c) = (p[t[0]], p[t[1]], p[t[2]]);
                let normal = (b - a).cross(c - a).normalize_or_zero();
                map_decals.push((e, order.0, (a + b + c) / 3.0, normal));
            }
        }
    }
    assert!(!map_decals.is_empty(), "the map's decals are drawn");
    assert!(map_decals.iter().all(|d| d.1 < IMPACT_ORDER), "map decals order before impacts");
    // The nearest to the reported spot.
    let spot = reported_spot();
    let (decal, _, centre, normal) = *map_decals
        .iter()
        .filter(|d| d.3 != Vec3::ZERO)
        .min_by(|a, b| a.2.distance_squared(spot).total_cmp(&b.2.distance_squared(spot)))
        .unwrap();
    assert!(centre.distance(spot) < 10.0, "a map decal near the report: {centre} vs {spot}");
    // A ray into the wall under it (from the side it faces), as a shot.
    let hit = {
        let mut state = SystemState::<SpatialQuery>::new(world);
        let q = state.get(world).unwrap();
        [normal, -normal].into_iter().find_map(|n| {
            let from = centre + n * 0.5;
            q.cast_ray(from, Dir3::new(-n).ok()?, 1.0, true, &SpatialQueryFilter::default())
                .map(|h| (h.entity, from - n * h.distance, h.normal))
        })
    };
    let (target, point, normal) = hit.unwrap_or_else(|| panic!("the surface under the map decal at {centre}"));
    world.write_message(PlaceDecal {
        target: Some(target),
        group: DecalGroup::Material('C'),
        point,
        normal,
        dir: -normal,
        spin: false,
        round: false,
    });
    sim.ticks(2);
    let world = sim.app.world_mut();
    let impacts: Vec<(Entity, u32, Handle<DecalMaterial>)> = world
        .query_filtered::<(Entity, &DecalOrder, &MeshMaterial3d<DecalMaterial>), With<RuntimeDecal>>()
        .iter(world)
        .map(|(e, o, m)| (e, o.0, m.0.clone()))
        .collect();
    assert_eq!(impacts.len(), 1, "one impact on the map decal");
    let (impact, order, material) = &impacts[0];
    assert_eq!(*order, IMPACT_ORDER, "drawn after the map's decals");
    assert!(world.get::<Children>(*impact).is_none(), "no depth-writing copy under it");
    let m = world.resource::<Assets<DecalMaterial>>().get(material).unwrap();
    assert!(matches!(m.alpha_mode(), AlphaMode::Blend), "the map decals' (blended) phase");
    assert!(
        world.get::<Visibility>(decal).is_none_or(|v| *v != Visibility::Hidden),
        "the map decal stays drawn"
    );
}
