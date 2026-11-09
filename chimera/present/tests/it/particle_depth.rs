//! Particles (CS:S's tracers and impacts, Combat Arms' effect groups) are
//! depth tested: a wall between the eye and a tracer hides it
//! (specs/cs_source/tracers.md draws nothing over the world; only Combat
//! Arms' own tracer strips ignore depth, tested in that crate). Groups on
//! a first-person view model (`ParticleGroup::view_model`: the sprites and
//! beams on its attachments, `map::attached`) are drawn over the model,
//! without the depth test (specs/hl2dm/effects_sounds.md 11: the gravity
//! gun's glows over its claws). The pipeline needs a GPU, so this checks
//! the depth state the material specializes with and which meshes get it.

use bevy::{
    camera::visibility::RenderLayers,
    prelude::*,
    render::render_resource::{CompareFunction, DepthStencilState, TextureFormat},
};
use chimera_present::render::particles::{ParticleAssets, ParticleDrawMaterial, draw_particles};
use chimera_world::map::{
    particles::{MapParticles, Motion, Particle, ParticleBlend, ParticleGroup, ParticleMaterial, Particles},
    view_model::VIEW_MODEL_LAYER,
};

fn tested() -> DepthStencilState {
    DepthStencilState {
        format: TextureFormat::Depth32Float,
        depth_write_enabled: Some(true),
        depth_compare: Some(CompareFunction::GreaterEqual),
        stencil: Default::default(),
        bias: Default::default(),
    }
}

#[test]
fn particles_keep_the_depth_test() {
    let mut depth = tested();
    ParticleDrawMaterial::apply_depth(&mut depth, false);
    assert_eq!(depth.depth_compare, Some(CompareFunction::GreaterEqual));
    assert_eq!(depth.depth_write_enabled, Some(false));
}

#[test]
fn particles_over_the_view_model_always_pass() {
    let mut depth = tested();
    ParticleDrawMaterial::apply_depth(&mut depth, true);
    assert_eq!(depth.depth_compare, Some(CompareFunction::Always), "the model doesn't hide them");
    assert_eq!(depth.depth_write_enabled, Some(false));
}

/// Two additive materials, one with `$ignorez`, each drawn by a world group
/// and a view-model group: only the `$ignorez` one's view-model mesh is over
/// the model (the gravity gun's beams; its core and glows hide behind the
/// claws, specs/hl2dm/effects_sounds.md 11).
#[test]
fn only_the_view_model_layer_draws_over_the_model() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<ParticleDrawMaterial>()
        .insert_resource(ParticleAssets::new(
            MapParticles {
                materials: [false, true]
                    .map(|no_depth| ParticleMaterial {
                        name: format!("no_depth {no_depth}"),
                        blend: ParticleBlend::Additive,
                        vertex_color: true,
                        vertex_alpha: true,
                        no_depth,
                        ..default()
                    })
                    .to_vec(),
            },
            Vec::new(),
        ))
        .init_resource::<Particles>()
        .add_systems(Update, draw_particles);
    app.world_mut().spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection::default()),
        GlobalTransform::default(),
    ));
    for (view_model, material) in [(false, 0), (false, 1), (true, 0), (true, 1)] {
        let mut g = ParticleGroup::new(Motion::default());
        g.skip_first = false;
        g.view_model = view_model;
        g.particles.push(Particle::new(Vec3::NEG_Z, 1.0, material, 0.1));
        app.world_mut().resource_mut::<Particles>().add(g);
    }
    app.update();
    let mut q = app
        .world_mut()
        .query::<(&MeshMaterial3d<ParticleDrawMaterial>, Option<&RenderLayers>)>();
    let drawn: Vec<(bool, bool)> = q
        .iter(app.world())
        .map(|(m, layers)| {
            let over = app.world().resource::<Assets<ParticleDrawMaterial>>().get(&m.0).unwrap().over_model;
            (layers.is_some_and(|l| *l == RenderLayers::layer(VIEW_MODEL_LAYER)), over)
        })
        .collect();
    assert_eq!(drawn.len(), 4, "{drawn:?}");
    assert_eq!(drawn.iter().filter(|d| **d == (true, true)).count(), 1, "view model, `$ignorez`: over it: {drawn:?}");
    assert_eq!(drawn.iter().filter(|d| **d == (true, false)).count(), 1, "view model, without: depth tested: {drawn:?}");
    assert_eq!(drawn.iter().filter(|d| **d == (false, false)).count(), 2, "world, depth tested: {drawn:?}");
}
