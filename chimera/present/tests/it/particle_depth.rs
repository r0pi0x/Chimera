//! Particles (CS:S's tracers and impacts, Combat Arms' effect groups) are
//! depth tested: a wall between the eye and a tracer hides it
//! (specs/cs_source/tracers.md draws nothing over the world; only Combat
//! Arms' own tracer strips ignore depth, tested in that crate). Groups on
//! a first-person view model (`ParticleGroup::view_model`: the sprites and
//! beams on its attachments, `map::attached`) are drawn over the model,
//! without the depth test (specs/hl2dm/effects_sounds.md 11: the gravity
//! gun's glows over its claws). Started `.pcf` systems are the world's,
//! first-person ones too (specs/tf2/particles.md 6). The pipeline needs a GPU, so this checks
//! the depth state the material specializes with and which meshes get it.

use bevy::{
    camera::visibility::RenderLayers,
    core_pipeline::prepass::DepthPrepass,
    prelude::*,
    render::render_resource::{CompareFunction, DepthStencilState, TextureFormat},
};
use chimera_present::render::particles::{ParticleAssets, ParticleDrawMaterial, draw_particles, soft_particle_prepass};
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

/// A material that says `$depthblend` draws soft and gives the main view
/// the depth prepass it fades against (spec impact_effects.md: "softens the
/// intersection with geometry"); one that doesn't stays cut at the world.
#[test]
fn depth_blend_materials_fade_against_a_prepass() {
    fn run(flags: &[bool]) -> (Vec<(bool, String)>, bool) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<ParticleDrawMaterial>()
            .insert_resource(ParticleAssets::new(
                MapParticles {
                    materials: flags
                        .iter()
                        .map(|&depth_blend| ParticleMaterial {
                            name: format!("depth_blend {depth_blend}"),
                            vertex_alpha: true,
                            depth_blend,
                            ..default()
                        })
                        .collect(),
                },
                Vec::new(),
            ))
            .init_resource::<Particles>()
            .add_systems(Update, (draw_particles, soft_particle_prepass));
        let camera = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                Projection::Perspective(PerspectiveProjection::default()),
                GlobalTransform::default(),
            ))
            .id();
        for material in 0..flags.len() {
            let mut g = ParticleGroup::new(Motion::default());
            g.skip_first = false;
            g.particles.push(Particle::new(Vec3::NEG_Z, 1.0, material, 0.1));
            app.world_mut().resource_mut::<Particles>().add(g);
        }
        app.update();
        app.update();
        let prepass = app.world().get::<DepthPrepass>(camera).is_some();
        let mut q = app.world_mut().query::<(&Name, &MeshMaterial3d<ParticleDrawMaterial>)>();
        let drawn = q
            .iter(app.world())
            .map(|(n, m)| {
                (app.world().resource::<Assets<ParticleDrawMaterial>>().get(&m.0).unwrap().soft, n.to_string())
            })
            .collect();
        (drawn, prepass)
    }
    let (mut drawn, prepass) = run(&[false, true]);
    drawn.sort();
    assert!(prepass, "a `$depthblend` material needs the prepass");
    assert!(!drawn[0].0 && drawn[0].1.ends_with("false"), "{drawn:?}");
    assert!(drawn[1].0 && drawn[1].1.ends_with("true"), "{drawn:?}");
    let (drawn, prepass) = run(&[false]);
    assert!(!prepass, "no material asks: no prepass for it");
    assert!(!drawn[0].0, "{drawn:?}");
}

/// A started particle system (`StartParticleSystem`, a `.pcf` effect) is
/// drawn by the world camera, a first-person one following the eye too
/// (a muzzle flash at the muzzle re-projected into the world view:
/// specs/tf2/particles.md 6, never the view-model camera, which drew it
/// tan(world/2) / tan(54/2) too big), depth tested against the world.
#[test]
fn started_systems_draw_in_the_world_pass() {
    use chimera_present::render::psys::{ParticleSystemDefs, run_started_systems};
    use chimera_world::map::psys::{Emitter, Follow, MapParticleSystems, Render, StartParticleSystem, StopParticleSystems, SystemDef};
    let def = |name: &str| SystemDef {
        name: name.into(),
        material: Some(0),
        initial_particles: 0,
        emitters: vec![Emitter::Instantaneously { start: 0.0, count: 4 }],
        renderer: Render::Sprites { rate: 1.0, as_fps: false, fit_life: false, orientation: 0 },
        ..default()
    };
    let run = |first_person: bool| {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<ParticleDrawMaterial>()
            .insert_resource(ParticleAssets::new(
                MapParticles { materials: vec![ParticleMaterial { name: "flash".into(), vertex_alpha: true, ..default() }] },
                Vec::new(),
            ))
            .insert_resource(ParticleSystemDefs(std::sync::Arc::new(MapParticleSystems {
                defs: vec![def("flash")],
                placed: Vec::new(),
                scale: 0.0254,
            })))
            .init_resource::<Particles>()
            .add_message::<StartParticleSystem>()
            .add_message::<StopParticleSystems>()
            .add_systems(Update, (run_started_systems, draw_particles).chain());
        let eye = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                Projection::Perspective(PerspectiveProjection::default()),
                Transform::default(),
                GlobalTransform::default(),
            ))
            .id();
        let at = Vec3::new(0.2, -0.2, -0.5);
        app.world_mut().write_message(StartParticleSystem {
            follow: first_person.then_some(Follow { entity: eye, local: at, forward: Vec3::NEG_Z, carry: true, up: None }),
            ..StartParticleSystem::new("flash", at, Vec3::NEG_Z)
        });
        app.update();
        app.update();
        let mut q = app.world_mut().query_filtered::<(Option<&RenderLayers>, &MeshMaterial3d<ParticleDrawMaterial>), ()>();
        let materials = app.world().resource::<Assets<ParticleDrawMaterial>>();
        q.iter(app.world())
            .map(|(l, m)| {
                let tested = materials.get(&m.0).is_some_and(|m| !m.over_model);
                (l.is_some_and(|l| *l == RenderLayers::layer(VIEW_MODEL_LAYER)), tested)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(run(true), vec![(false, true)], "a first-person flash: the world's layer, depth tested");
    assert_eq!(run(false), vec![(false, true)], "in the world");
}

/// Particles in water seen from above it are drawn before the water
/// surfaces (`ParticleDrawMaterial::before_water`), so the surface's
/// refraction shows them (specs/tf2/effects.md 9: TF2's blast on a river
/// bed seen from just above the surface, an orange glow through it); in
/// the transparent pass the surface's depth hid them. With the eye in the
/// water every particle draws as usual.
#[test]
fn particles_in_water_seen_from_above_draw_before_the_surface() {
    fn run(eye_under: bool) -> Vec<(f32, bool)> {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<ParticleDrawMaterial>()
            .insert_resource(ParticleAssets::new(
                MapParticles {
                    materials: vec![ParticleMaterial { name: "glow".into(), blend: ParticleBlend::Additive, vertex_alpha: true, ..default() }],
                },
                Vec::new(),
            ))
            .insert_resource(chimera_core::core::MapWater(vec![chimera_core::core::MapWaterVolume {
                brush: chimera_core::core::MapBrush::from_box(Vec3::new(-10.0, -5.0, -10.0), Vec3::new(10.0, 0.0, 10.0)),
                slime: false, lava: false, current: None,
            }]))
            .insert_resource(chimera_world::map::water::WaterView { under: eye_under.then_some(0), ..default() })
            .init_resource::<Particles>()
            .add_systems(Update, draw_particles);
        app.world_mut().spawn((
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection::default()),
            GlobalTransform::from_translation(Vec3::Y),
        ));
        for y in [-1.0, 0.5] {
            let mut g = ParticleGroup::new(Motion::default());
            g.skip_first = false;
            g.particles.push(Particle::new(Vec3::new(0.0, y, -3.0), 1.0, 0, 0.1));
            app.world_mut().resource_mut::<Particles>().add(g);
        }
        app.update();
        let mut q = app.world_mut().query::<(&MeshMaterial3d<ParticleDrawMaterial>, &Transform)>();
        let mut drawn: Vec<(f32, bool)> = q
            .iter(app.world())
            .map(|(m, t)| (t.translation.y, app.world().resource::<Assets<ParticleDrawMaterial>>().get(&m.0).unwrap().before_water))
            .collect();
        drawn.sort_by(|a, b| a.0.total_cmp(&b.0));
        drawn
    }
    // The mesh's centre is its particles'; the mesh is placed in the
    // frame after it is spawned, so compare by material only.
    let above = run(false);
    assert_eq!(above.len(), 2, "{above:?}");
    assert_eq!(above.iter().filter(|d| d.1).count(), 1, "the one in the water before the surface: {above:?}");
    let under = run(true);
    assert!(under.iter().all(|d| !d.1), "eye in the water: as usual: {under:?}");
}
