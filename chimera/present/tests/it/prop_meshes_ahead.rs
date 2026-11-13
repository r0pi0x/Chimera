//! A prop lit by its baked light draws its own mesh copies with the light
//! per vertex (its probe on each normal, or the light baked per vertex of
//! the model), and so do its lower levels of detail. A load makes those
//! meshes ahead, off the main thread (`render::prepare_images`, a
//! `map::prepare` job: a probe on every vertex of a step's props was the
//! worst frame of a map's swap, docs/performance.md, "A swap's prop meshes
//! and shadows"), and the kit's bodies' meshes with their per-joint bounds
//! too: a spawn with them draws the same meshes as one that makes them in
//! place. Headless.

use bevy::prelude::*;
use chimera_present::{
    MapPlugin,
    render::{
        bodies::PlayModels, lod::LodMeshes, prop_material::PropMaterial, props::PropMeshSlot, rope_material::RopeMaterial,
        shadows::ShadowMaterial, sprite_material::SpriteMaterial, world_material::WorldMaterial,
    },
};
use chimera_world::{
    harness::Sim,
    map::{LightProbe, MapData, MapMesh, MapModel, MapProp, PropIndex, PropSolid, lod::MapLod},
};

/// A quad of `n` x `n` vertices' worth of normals leaning every way.
fn sheet(material: &str, n: usize, skinned: bool) -> MapMesh {
    let mut m = MapMesh {
        material: material.into(),
        color: [128, 128, 128],
        ..default()
    };
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x as f32 / (n - 1) as f32, y as f32 / (n - 1) as f32);
            m.positions.push([u - 0.5, v, 0.0]);
            m.normals.push(Vec3::new(u - 0.5, v - 0.5, 0.4).normalize().to_array());
            m.uvs.push([u, v]);
            m.source_vertices.push((y * n + x) as u32);
        }
    }
    for y in 0..n - 1 {
        for x in 0..n - 1 {
            let i = (y * n + x) as u32;
            m.indices.extend([i, i + 1, i + n as u32 + 1, i, i + n as u32 + 1, i + n as u32]);
        }
    }
    if skinned {
        m.joints = m.positions.iter().map(|p| [(p[1] > 0.5) as u16, 0, 0, 0]).collect();
        m.joint_weights = m.positions.iter().map(|_| [1.0, 0.0, 0.0, 0.0]).collect();
    }
    m
}

fn probe(k: f32) -> LightProbe {
    LightProbe {
        cube: [
            Vec3::new(0.9, 0.1, 0.1) * k,
            Vec3::new(0.1, 0.8, 0.1) * k,
            Vec3::new(0.1, 0.1, 0.7) * k,
            Vec3::new(0.6, 0.6, 0.1) * k,
            Vec3::new(0.1, 0.5, 0.5) * k,
            Vec3::new(0.4, 0.1, 0.4) * k,
        ],
        lights: vec![(Vec3::new(0.3, 0.8, 0.5).normalize(), Vec3::new(1.5, 1.2, 0.9) * k)],
    }
}

/// Props lit every way a baked prop is: by its probe (turned), by light
/// baked per vertex, skinned, in the 3D skybox (no levels), and one
/// without baked light (its model's shared meshes); two meshes a model,
/// one lower level each, the second mesh's missing from the level.
fn map() -> MapData {
    let mut data = MapData {
        name: "test:prop_meshes_ahead".into(),
        entity_scale: 0.0254,
        ..default()
    };
    data.look.light_scale = 1.7;
    let bone = |name: &str, parent| chimera_world::map::MapBone {
        name: name.into(),
        parent,
        position: Vec3::ZERO,
        rotation: Quat::IDENTITY,
    };
    for skinned in [false, true] {
        data.models.push(MapModel {
            meshes: vec![sheet("test/pot", 5, skinned), sheet("test/lid", 3, skinned)],
            bounds: (Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.0, 0.5)),
            lods: vec![MapLod {
                switch: 20.0,
                meshes: vec![Some(sheet("test/pot", 3, skinned)), None],
            }],
            rig: skinned.then(|| {
                std::sync::Arc::new(chimera_world::map::MapRig {
                    bones: vec![bone("root", None), bone("top", Some(0))],
                    root: Transform::IDENTITY,
                    animations: default(),
                })
            }),
            ..default()
        });
    }
    let prop = |model: usize, x: f32| MapProp {
        pose: None,
        ragdoll: None,
        model,
        translation: Vec3::new(x, 0.0, 0.0),
        rotation: Quat::from_euler(EulerRot::YXZ, 0.7 * x, 0.2, 0.1 * x),
        solid: PropSolid::None,
        skybox: false,
        lighting: Some(probe(1.0 + x)),
        vertex_light: None,
        casts_shadow: false,
        physics: None,
        fade: None,
        parent: None,
        entity: None,
        skin: 0,
        body: 0,
        detail: false,
    };
    data.props.push(prop(0, 0.0));
    data.props.push(prop(0, 2.0));
    data.props.push(MapProp {
        vertex_light: Some(std::sync::Arc::new((0..25).map(|i| [i as f32 * 0.01, 0.5, 1.0 - i as f32 * 0.02]).collect())),
        ..prop(0, 4.0)
    });
    data.props.push(prop(1, 6.0));
    data.props.push(MapProp {
        skybox: true,
        ..prop(0, 8.0)
    });
    data.props.push(MapProp {
        lighting: None,
        ..prop(0, 10.0)
    });
    // A body of the skinned model, with its lower level.
    data.kit.characters.push(chimera_world::map::MapCharacterModel {
        game: None,
        team: None,
        name: Some("tester".into()),
        class: None,
        model: data.models[1].clone(),
        hitboxes: Vec::new(),
        bones: vec![bone("root", None), bone("top", Some(0))],
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    });
    data
}

/// Every prop mesh the spawn drew: (prop, mesh of its model, its levels'
/// meshes written out); the kit's body follows as prop `usize::MAX`.
fn drawn(prepared: bool) -> Vec<(usize, usize, Vec<String>)> {
    let mut sim = Sim::with(
        (chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin),
        |app| {
            app.init_asset::<Image>()
                .init_asset::<StandardMaterial>()
                .init_asset::<WorldMaterial>()
                .init_asset::<PropMaterial>()
                .init_asset::<SpriteMaterial>()
                .init_asset::<RopeMaterial>()
                .init_asset::<ShadowMaterial>()
                .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
                .add_plugins(MapPlugin::new(map()));
            // As a background load does (`map::prepare`).
            if prepared {
                let parts = chimera_world::map::prepare::prepare_pending(app.world_mut());
                assert!(parts > 0, "the drawer prepares parts of its spawn");
            }
        },
    );
    sim.ticks(2);
    let world = sim.app.world_mut();
    let mut found: Vec<(usize, usize, Vec<Handle<Mesh>>)> = world
        .query::<(&ChildOf, &PropMeshSlot, &Mesh3d, Option<&LodMeshes>)>()
        .iter(world)
        .filter_map(|(parent, slot, mesh, levels)| {
            let prop = world.get::<PropIndex>(parent.parent())?.0;
            Some((prop, slot.mesh, levels.map_or_else(|| vec![mesh.0.clone()], |l| l.0.clone())))
        })
        .collect();
    found.sort_by_key(|f| (f.0, f.1));
    let bodies = &world.resource::<PlayModels>().bodies;
    assert_eq!(bodies.len(), 1, "the kit's body is built");
    found.extend(bodies[0].levels.iter().enumerate().map(|(part, levels)| (usize::MAX, part, levels.clone())));
    let meshes = world.resource::<Assets<Mesh>>();
    found
        .into_iter()
        .map(|(prop, slot, levels)| {
            let written = levels
                .iter()
                .map(|h| format!("{:?}", meshes.get(h).expect("a prop's mesh")))
                .collect();
            (prop, slot, written)
        })
        .collect()
}

#[test]
fn meshes_lit_ahead_are_the_ones_lit_in_place() {
    let (here, ahead) = (drawn(false), drawn(true));
    // Six props of two meshes; the baked ones with their own lit copies.
    assert_eq!(here.len(), 14, "every prop's meshes are drawn, and the body's");
    let lit = |d: &[(usize, usize, Vec<String>)], prop: usize| d.iter().find(|m| m.0 == prop && m.1 == 0).unwrap().2.clone();
    assert!(lit(&here, 0)[0].contains("Vertex_Color"), "a baked prop's mesh carries its light");
    assert_ne!(lit(&here, 0), lit(&here, 1), "each by its own probe and turn");
    assert_eq!(lit(&here, 0).len(), 2, "with its lower level");
    assert_eq!(lit(&here, 4).len(), 1, "none in the 3D skybox");
    assert!(lit(&here, 3)[0].contains("Vertex_JointIndex"), "a skinned one with its joints");
    assert_eq!(lit(&here, usize::MAX).len(), 2, "the body's part with its lower level");
    assert!(lit(&here, usize::MAX)[0].contains("Vertex_JointIndex"), "skinned");
    for (a, b) in here.iter().zip(&ahead) {
        assert_eq!((a.0, a.1), (b.0, b.1));
        assert_eq!(a.2, b.2, "prop {} mesh {}: lit ahead as lit in place", a.0, a.1);
    }
    assert_eq!(here.len(), ahead.len());
}
