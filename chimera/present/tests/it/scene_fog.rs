//! The scene's fog (`render::fog::SceneFog`) on the materials that take
//! it (ropes, runtime decals, particles): `FogShaderPlugin`, which every
//! map material's plugin adds, keeps them on it. A map's fog reaches the
//! materials made before it arrived; the water's fog (the eye under the
//! surface) reaches them, and leaves them as the eye comes out; the fog
//! setting's switch (`fog_enable 0`) survives a change of fog.

use bevy::prelude::*;
use chimera_present::render::{
    decal::DecalMaterial,
    fog::{FogSetting, FogShaderPlugin, FogUniform, SceneFog, WATER_FOG_RANGE, world_fog},
    particles::ParticleDrawMaterial,
    rope_material::RopeMaterial,
};
use chimera_world::{
    harness::Sim,
    map::{MapData, MapDebugView, MapFog, change_map},
};

use crate::prop_shadows::{base, map};

fn sim() -> Sim {
    let mut sim = Sim::with(base(), |app| {
        app.init_asset::<Image>()
            .init_asset::<Shader>()
            .init_asset::<StandardMaterial>()
            .init_asset::<chimera_present::render::world_material::WorldMaterial>()
            .init_asset::<chimera_present::render::prop_material::PropMaterial>()
            .init_asset::<chimera_present::render::sprite_material::SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<chimera_present::render::shadows::ShadowMaterial>()
            .init_asset::<chimera_present::render::water::WaterMaterial>()
            .init_asset::<chimera_present::render::beams::BeamMaterial>()
            .init_asset::<chimera_present::render::detail::DetailMaterial>()
            .init_asset::<DecalMaterial>()
            .init_asset::<ParticleDrawMaterial>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .add_plugins((chimera_present::MapPlugin::empty(), FogShaderPlugin));
    });
    sim.repeatable();
    sim
}

/// A map with Combat Arms-style linear grey fog, 1 to 40 m.
fn fogged() -> MapData {
    let mut data = map();
    data.name = "test:scene_fog".into();
    data.fog = Some(MapFog {
        color: [0.5, 0.5, 0.5],
        start: 1.0,
        end: 40.0,
        max_density: 1.0,
        linear: true,
    });
    data
}

/// A decal and a particle material made with no fog (before the map).
fn materials(sim: &mut Sim) -> (Handle<DecalMaterial>, Handle<ParticleDrawMaterial>) {
    let w = sim.app.world_mut();
    let decal = w.resource_mut::<Assets<DecalMaterial>>().add(DecalMaterial {
        params: Vec4::ZERO,
        texture: Handle::default(),
        fog: FogUniform::default(),
        light: Vec4::ZERO,
        tint: Vec4::ONE,
    });
    let particle = w.resource_mut::<Assets<ParticleDrawMaterial>>().add(ParticleDrawMaterial {
        texture: None,
        fog: FogUniform::default(),
        add_self: Vec4::ZERO,
        blend: chimera_world::map::particles::ParticleBlend::Additive,
        over_model: false,
        soft: false,
        before_water: false,
    });
    (decal, particle)
}

fn fogs(sim: &Sim, decal: &Handle<DecalMaterial>, particle: &Handle<ParticleDrawMaterial>) -> (FogUniform, FogUniform) {
    let w = sim.app.world();
    (
        w.resource::<Assets<DecalMaterial>>().get(decal).unwrap().fog,
        w.resource::<Assets<ParticleDrawMaterial>>().get(particle).unwrap().fog,
    )
}

#[test]
fn the_maps_fog_and_the_waters_reach_existing_materials() {
    let mut sim = sim();
    let (decal, particle) = materials(&mut sim);
    change_map(sim.app.world_mut(), fogged(), MapDebugView::Normal);
    sim.run_until_idle(600).unwrap();
    sim.app.update();
    let want = world_fog(fogged().fog.as_ref());
    assert!(want.color.w > 0.5, "the map's fog is on");
    assert_eq!(sim.app.world().resource::<SceneFog>().0, want);
    assert_eq!(fogs(&sim, &decal, &particle), (want, want), "the map's fog on materials made before it");

    // The eye goes under water (`water::underwater_fog` sets the water part).
    let under = FogUniform {
        water_color: Vec4::new(0.1, 0.2, 0.3, WATER_FOG_RANGE),
        water_range: Vec4::new(0.0, 5.0, 1.0, 0.0),
        ..want
    };
    sim.app.world_mut().resource_mut::<SceneFog>().0 = under;
    sim.app.update();
    assert_eq!(fogs(&sim, &decal, &particle), (under, under), "the water's fog under water");

    // `fog_enable 0`, then back out of the water: the world fog's switch
    // stays off.
    sim.app.world_mut().resource_mut::<FogSetting>().enabled = 0;
    sim.app.update();
    assert_eq!(fogs(&sim, &decal, &particle).0.color.w, -1.0, "fog_enable 0 switches it off");
    sim.app.world_mut().resource_mut::<SceneFog>().0 = want;
    sim.app.update();
    let (d, p) = fogs(&sim, &decal, &particle);
    let off = FogUniform {
        color: want.color.with_w(-1.0),
        ..want
    };
    assert_eq!((d, p), (off, off), "out of the water, the world fog still switched off");
}
