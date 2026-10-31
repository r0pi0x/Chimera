//! Effects that follow an entity (`effects::attached`; effects_sounds.md
//! "The effect file", placement modes; special_weapons.md Follow-up
//! (2026-10-30), cases 48-53; game_modes.md Corrections (2026-10-30) 4
//! and 7, case 114), on the harness with effect groups written here
//! (named ours) and Chimera's particle pool:
//!
//! - a projectile's fly sound starts looping with it, follows it and stops
//!   when it goes; `FlySound` 0 none (48);
//! - an attached group is made once, emits where its entity is, and is
//!   shut down with it: smooth, its particles live out; else they go at
//!   once (49);
//! - the smoke trail's segments and puffs; no light (50);
//! - the model faces its fire direction for the whole flight (51);
//! - launched ammo's surface group is the surface's impact group (53);
//! - the infected face's eyes: its `ClientFX#` groups on the eye sockets,
//!   looping, seen in third person only (114).

use std::{collections::HashMap, sync::Arc};

use bevy::prelude::*;
use chimera_core::core::LocalPlayer;
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::{
        ShowLocalBody,
        loose::ShownItem,
        particles::{MapParticles, ParticleBlend, ParticleMaterial, ParticleMaterials, ParticleSet, Particles, Shape, step_particles},
    },
};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        characters::{CaPose, CaRungLook, records::Selection},
        data::{CaData, attributes::MODELS_FILE, read_attributes},
        effects::{
            attached::{AttachedGroup, CaAttachedGroups, fly_sound_key, pose_of, worn_groups},
            data::CaFxTables,
            fx::{CaFx, SmokeTrail},
            launched_burst,
            library::{FxLibrary, material_name, normalize, set_library, smoke_trail},
        },
        inventory::UNIT,
        special::{CaObject, Kind, LOOK_SEPARATOR, PROJECTILE, ProjectileLook, ProjectileParams, facing},
        view_model::attached::CaLoopSound,
    },
    harness::{CaSim, CaSimWith},
};
use lithtech_formats::fxf::{FxFile, FxGroup, FxKey, FxProp, FxValue};

const TAIL_TEX: &str = "test_fx/tail.dtx";
const EYE_SPR: &str = "test_fx/eye.spr";
const EYE_TEX: &str = "test_fx/eye.dtx";
const TRAIL_TEX: &str = "test_fx/trail.dtx";
const SMOKE_TEX: &str = "test_fx/smoke.dtx";

fn combo(i: i32, choices: &[&str]) -> FxValue {
    FxValue::Combo {
        index: i,
        choices: choices.iter().map(|c| c.to_string()).collect(),
    }
}

fn path(p: &str) -> FxValue {
    FxValue::Path {
        ext: p.rsplit('.').next().unwrap_or("").into(),
        path: p.into(),
    }
}

fn key(kind: &str, end: f32, props: Vec<(&str, FxValue)>) -> FxKey {
    FxKey {
        kind: kind.into(),
        label: String::new(),
        id: 1,
        linked: false,
        link_id: 0,
        link_node: String::new(),
        start: 0.0,
        end,
        repeat: 1,
        props: props.into_iter().map(|(n, value)| FxProp { name: n.into(), value }).collect(),
    }
}

/// `Test_Tail` (1 s, a follow-mode particle system emitting a particle
/// every 0.05 s that lives 1 s) and `Test_Eyes` (3 s: an additive sprite
/// on `EYE_L`, a trail on `EYE_R`, both socket-attached for the whole
/// group: the stock red eyes' shape).
fn groups() -> Vec<FxGroup> {
    let socket = |s: &str| vec![("UpdatePos", combo(4, &["Fixed", "Follow", "PlayerView", "NodeAttach", "SocketAttach"])), ("AttachName", FxValue::String(s.into()))];
    vec![
        FxGroup {
            name: "Test_Tail".into(),
            phase_ms: 1000,
            keys: vec![key(
                "ParticleSystem",
                1.0,
                vec![
                    ("UpdatePos", combo(1, &["Fixed", "Follow"])),
                    ("Texture", path(TAIL_TEX)),
                    ("EmissionInterval", FxValue::Float(0.05)),
                    ("ParticlesPerEmission", FxValue::Int(1)),
                    ("MinParticleLifeSpan", FxValue::Float(1.0)),
                    ("MaxParticleLifeSpan", FxValue::Float(1.0)),
                    ("Type", combo(1, &["Sphere", "Point"])),
                ],
            )],
        },
        FxGroup {
            name: "Test_Eyes".into(),
            phase_ms: 3000,
            keys: vec![
                key(
                    "Sprite",
                    3.0,
                    [socket("EYE_L"), vec![("Sprite", path(EYE_SPR)), ("BlendMode", combo(1, &["None", "Additive"])), ("Sk", FxValue::Vector4([0.0, 0.05, 0.0, 0.0]))]].concat(),
                ),
                key(
                    "PolyTrail",
                    3.0,
                    [
                        socket("EYE_R"),
                        vec![
                            ("Texture", path(TRAIL_TEX)),
                            ("TrailWidth", FxValue::Float(2.0)),
                            ("TrailLen", FxValue::Int(50)),
                            ("SectionLifespan", FxValue::Float(0.2)),
                            ("SectionInterval", FxValue::Float(0.03)),
                            ("BlendMode", combo(1, &["None", "Add"])),
                        ],
                    ]
                    .concat(),
                ),
            ],
        },
    ]
}

/// The effect library every test of this binary shares (the attached
/// view-model effects' groups too: `attached_fx`), so tests running at
/// once set the same one.
pub(super) fn test_library() -> Arc<FxLibrary> {
    let mut file = FxFile::parse(super::attached_fx::FX).unwrap();
    file.groups.extend(groups());
    let textures: HashMap<String, UVec2> = [EYE_TEX, TRAIL_TEX, TAIL_TEX, SMOKE_TEX].iter().map(|t| (t.to_string(), UVec2::splat(32))).collect();
    let sprites: HashMap<String, String> = [
        (EYE_SPR.to_string(), EYE_TEX.to_string()),
        (normalize(smoke_trail(2).sprite), SMOKE_TEX.to_string()),
    ]
    .into_iter()
    .collect();
    Arc::new(FxLibrary { file, textures, sprites })
}

/// The materials the groups draw with, in the pool's material list.
fn materials(sim: &mut Sim) {
    let mut list = sim.app.world().get_resource::<ParticleMaterials>().map_or_else(MapParticles::default, |m| m.0.clone());
    for (tex, add) in [(TAIL_TEX, false), (EYE_TEX, true), (TRAIL_TEX, true), (SMOKE_TEX, false)] {
        let name = material_name(tex, add);
        if list.find(&name).is_none() {
            list.materials.push(ParticleMaterial {
                name,
                blend: if add { ParticleBlend::Additive } else { ParticleBlend::Alpha },
                ..default()
            });
        }
    }
    sim.app.insert_resource(ParticleMaterials(list));
}

#[derive(Resource, Default)]
struct Loops(Vec<CaLoopSound>);

fn sim_with(data: Option<CaData>) -> Sim {
    set_library(Some(test_library()));
    let setup = |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
        // The pool as a drawing app steps it (the headless harness has
        // none): after the effects, before drawing.
        app.init_resource::<Loops>()
            .init_resource::<Particles>()
            .add_systems(Update, step_particles.in_set(ParticleSet::Step))
            .add_systems(Last, |mut r: MessageReader<CaLoopSound>, mut l: ResMut<Loops>| l.0.extend(r.read().cloned()));
    };
    let mut sim = match data {
        Some(d) => Sim::with(CaSimWith(d), setup),
        None => Sim::with(CaSim, setup),
    };
    sim.set_tick_interval(TICK_INTERVAL);
    sim.ticks(2);
    materials(&mut sim);
    sim
}

fn material(sim: &Sim, tex: &str, add: bool) -> usize {
    sim.app.world().resource::<ParticleMaterials>().0.find(&material_name(tex, add)).unwrap()
}

/// Every live particle of the pool with its group's tag.
fn particles(sim: &Sim) -> Vec<(Option<u64>, chimera_world::map::particles::Particle)> {
    sim.app
        .world()
        .resource::<Particles>()
        .groups
        .iter()
        .flat_map(|g| g.particles.iter().map(move |p| (g.tag, p.clone())))
        .collect()
}

/// A projectile drawn with `look` (no model).
fn projectile(sim: &mut Sim, look: &ProjectileLook, at: Vec3) -> Entity {
    let key = format!("{}{LOOK_SEPARATOR}{}", ProjectileParams::NO_MODEL_KEY, look.key());
    sim.app.world_mut().spawn((ShownItem(key), Transform::from_translation(at))).id()
}

/// 48. The fly sound starts with the projectile, looping, 3D with R 1200
/// and r 84, following it; it stops when the projectile goes. `FlySound`
/// 0 (empty): no sound.
#[test]
fn t48_fly_sound_follows() {
    let mut sim = sim_with(None);
    let look = ProjectileLook {
        fly_sound: "test_snd/rocket/fly.wav".into(),
        sound_radius: 1200.0,
        ..default()
    };
    let e = projectile(&mut sim, &look, Vec3::ZERO);
    sim.ticks(1);
    let loops = std::mem::take(&mut sim.app.world_mut().resource_mut::<Loops>().0);
    let start: Vec<_> = loops.iter().filter(|l| l.key == fly_sound_key(e)).collect();
    assert_eq!(start.len(), 1, "{loops:?}");
    let s = start[0].sound.as_ref().expect("a start");
    assert_eq!((s.file.as_str(), s.radii, s.volume), ("test_snd/rocket/fly.wav", (84.0, 1200.0), 100.0));
    assert_eq!(start[0].follow, Some(e), "it follows the projectile");
    sim.ticks(5);
    assert!(sim.app.world().resource::<Loops>().0.is_empty(), "started once");
    sim.app.world_mut().entity_mut(e).despawn();
    sim.ticks(1);
    let loops = &sim.app.world().resource::<Loops>().0;
    assert!(loops.iter().any(|l| l.key == fly_sound_key(e) && l.sound.is_none()), "stopped when it goes: {loops:?}");
    // No fly sound: nothing starts.
    sim.app.world_mut().resource_mut::<Loops>().0.clear();
    let quiet = ProjectileLook { fx: "Test_Tail".into(), ..default() };
    projectile(&mut sim, &quiet, Vec3::ZERO);
    sim.ticks(2);
    assert!(sim.app.world().resource::<Loops>().0.is_empty());
}

/// 49. An attached group starts once and moves with its projectile (each
/// emission where the projectile is then); at detonation, with smooth
/// shut-down, the emitter stops and live particles stay; without, every
/// particle goes in that frame. Not looping: it plays its length once.
#[test]
fn t49_attached_group() {
    for smooth in [true, false] {
        let mut sim = sim_with(None);
        let look = ProjectileLook {
            fx: "Test_Tail".into(),
            fx_smooth: smooth,
            ..default()
        };
        let e = projectile(&mut sim, &look, Vec3::ZERO);
        let tex = material(&sim, TAIL_TEX, false);
        for i in 1..=30 {
            sim.app.world_mut().get_mut::<Transform>(e).unwrap().translation = Vec3::X * i as f32 * 0.1;
            sim.ticks(1);
        }
        assert_eq!(sim.app.world().resource::<CaFx>().attached(e).count(), 1, "made once");
        let ours: Vec<_> = particles(&sim).into_iter().filter(|(t, p)| t.is_some() && p.material == tex).collect();
        assert!(ours.len() >= 8, "{} particles", ours.len());
        let xs: Vec<f32> = ours.iter().map(|(_, p)| p.position.x).collect();
        let (lo, hi) = xs.iter().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
        assert!(lo < 0.5 && hi > 2.5, "emitted along the way: {lo}..{hi}");
        sim.app.world_mut().entity_mut(e).despawn();
        sim.ticks(1);
        assert_eq!(sim.app.world().resource::<CaFx>().attached(e).count(), 0, "shut down");
        let left = particles(&sim).into_iter().filter(|(t, p)| t.is_some() && p.material == tex).count();
        if smooth {
            assert!(left > 0, "smooth: its particles live out");
        } else {
            assert_eq!(left, 0, "every key gone at once");
        }
    }
    // FXLoop 0: one length (1 s) of emissions, then none while it flies on.
    let mut sim = sim_with(None);
    let look = ProjectileLook { fx: "Test_Tail".into(), ..default() };
    let e = projectile(&mut sim, &look, Vec3::ZERO);
    sim.seconds(2.5);
    let tex = material(&sim, TAIL_TEX, false);
    assert_eq!(particles(&sim).iter().filter(|(_, p)| p.material == tex).count(), 0, "spent after its length");
    assert_eq!(sim.app.world().resource::<CaFx>().attached(e).count(), 1, "and not made again");
}

/// 50. `SmokeTrail` 1, type 2: segments 0.1 s apart, puffs living 1.5 s,
/// alpha 0.9 → 0, scale 1 → 15 (a square of side 4 s), the long white
/// trail's sprite; `Light` 1 makes no light.
#[test]
fn t50_smoke_trail_and_no_light() {
    let mut sim = sim_with(None);
    let look = ProjectileLook {
        smoke_trail: Some(2),
        ..default()
    };
    let e = projectile(&mut sim, &look, Vec3::ZERO);
    for i in 1..=34 {
        sim.app.world_mut().get_mut::<Transform>(e).unwrap().translation = Vec3::Z * i as f32 * 0.2;
        sim.ticks(1);
    }
    let trail = sim.app.world().get::<SmokeTrail>(e).expect("a trail").clone();
    assert!(trail.segments.len() == 2, "type 2 emits 0.2 s, a segment each 0.1 s: {trail:?}");
    assert!(trail.segments.iter().all(|s| ((s / 0.1).round() * 0.1 - s).abs() < 1e-3));
    let tex = material(&sim, SMOKE_TEX, false);
    let puffs: Vec<_> = particles(&sim).into_iter().filter(|(_, p)| p.material == tex).map(|(_, p)| p).collect();
    assert!(!puffs.is_empty());
    for p in &puffs {
        assert_eq!(p.life, smoke_trail(2).life);
        assert!((p.alpha - 0.9).abs() < 1e-6);
        assert!((p.size.at(0.0) - 2.0 * UNIT).abs() < 1e-6 && (p.size.at(1.0) - 30.0 * UNIT).abs() < 1e-5);
        let up = p.velocity.y / UNIT;
        assert!((5.0..=6.0).contains(&up) && p.velocity.x == 0.0, "{}", p.velocity / UNIT);
    }
    assert!(sim.app.world().resource::<CaFx>().lights.is_empty(), "no light");
}

/// 51. A rocket fired 30° up faces that way for its whole flight, even as
/// gravity bends its path; one fired straight up takes its up axis from
/// world +X.
#[test]
fn t51_orientation() {
    let set = CaData::builtin_static().weapon_set();
    let p: ProjectileParams = set.get("combat_arms:law").unwrap().hook(PROJECTILE).unwrap();
    let dir = Vec3::new(0.0, 0.5, -(3.0f32.sqrt()) / 2.0);
    let mut o = CaObject::new("combat_arms:law", None, UNIT, 1.0, Vec3::ZERO, dir * 1300.0, Kind::Impact(Arc::new(p)));
    assert!(o.drawn_dir().distance(dir) < 1e-6);
    o.velocity = Vec3::new(0.0, -300.0, -1100.0);
    assert!(o.drawn_dir().distance(dir) < 1e-6, "not turned along the arc");
    let r = facing(dir);
    assert!((r * Vec3::NEG_Z).distance(dir) < 1e-5);
    let up = facing(Vec3::Y);
    assert!((up * Vec3::NEG_Z).distance(Vec3::Y) < 1e-5);
    assert!((up * Vec3::Y).abs().distance(Vec3::X) < 1e-5, "up from world +X");
}

/// 53. A launched round's surface group on Wood is the surface's impact
/// group (`Impact_Wood`), a grenade's its grenade group (`Grenade_Dirt`).
#[test]
fn t53_surface_group() {
    let t = CaFxTables::built_in();
    let wood = t.surface(t.surface_id("Wood").unwrap());
    assert_eq!(launched_burst(wood, false).as_deref(), Some("Impact_Wood"));
    assert_eq!(wood.grenade_fx, "Grenade_Dirt");
}

/// The models file of case 114, shaped as the stock one: body 0's
/// override 0 wears face 20, which lists the eye group; face 21 lists `1`,
/// a group the effect file lacks.
fn infected_data() -> CaData {
    let mut text = String::from("[Model0] Name=\"Normal\" FigureType=0 InfectedOveridable=0\n[Overidable_Model0] Name=\"Infected_Man_Light\" FaceIndex=20\n");
    for i in 0..22 {
        let fx = match i {
            20 => " ClientFX0=\"Test_Eyes\"",
            21 => " ClientFX0=\"1\"",
            _ => "",
        };
        text.push_str(&format!("[Face{i}] Name=\"Face{i}\"{fx}\n"));
    }
    let (attrs, problems) = read_attributes(|name| match name {
        MODELS_FILE => Ok(text.as_bytes().to_vec()),
        _ => Err(std::io::Error::new(std::io::ErrorKind::NotFound, name.to_string())),
    });
    assert!(attrs.models.is_some(), "{problems:?}");
    CaData::with_attributes(attrs)
}

/// 114 (data): override 0's face lists the eye group, worn looping and
/// in third person only; a face listing `1` lists a group the effect file
/// doesn't have, which plays nothing.
#[test]
fn t114_eye_groups_by_face() {
    let data = infected_data();
    let look = data.characters().infected_look(0);
    assert_eq!(look.face, 20);
    assert_eq!(
        worn_groups(&data, &look),
        vec![AttachedGroup {
            group: "Test_Eyes".into(),
            looping: true,
            smooth: false,
            third_person: true,
        }]
    );
    let other = Selection { face: 21, ..default() };
    assert_eq!(worn_groups(&data, &other)[0].group, "1");
    assert!(test_library().group("1").is_none(), "no such group: nothing plays");
    assert!(worn_groups(&data, &Selection::default()).is_empty(), "a human face has none");
}

/// 114 (sockets): a character's groups sit at its pose's eye sockets.
#[test]
fn t114_eye_sockets() {
    let pose = CaPose {
        eyes: [Some(Vec3::new(-2.0, 64.0, -5.0)), Some(Vec3::new(2.0, 64.0, -5.0))],
        ..default()
    };
    let state = chimera_core::core::MovementState {
        hull_min: Vec3::new(-0.3, -0.9, -0.3),
        hull_max: Vec3::new(0.3, 0.9, 0.3),
        ..default()
    };
    let t = Transform::from_xyz(1.0, 0.9, 2.0);
    let p = pose_of(&t, Some((&pose, &state)));
    let left = p.sockets.iter().find(|s| s.0 == "EYE_L").unwrap().1;
    assert!(left.distance(pose.world(&t, &state, UNIT, Vec3::new(-2.0, 64.0, -5.0))) < 1e-6);
    assert!((left.y - 64.0 * UNIT).abs() < 1e-5, "at the eyes' height above the feet");
    assert!(p.sockets.iter().any(|s| s.0 == "EYE_R"));
}

/// 114 (play): an infected character's eyes are drawn looping where it
/// is (the sprite each frame, the trail's sections behind it as it moves),
/// for everyone else; never for the player himself in first person, and
/// shown when his body is (third person).
#[test]
fn t114_eyes_third_person_only() {
    let mut sim = sim_with(Some(infected_data()));
    sim.app.insert_resource(ShowLocalBody(false));
    let infected = Selection { face: 20, ..default() };
    let other = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.app.world_mut().entity_mut(other).insert(CaRungLook(infected));
    sim.ticks(2);
    assert!(sim.app.world().get::<CaAttachedGroups>(other).is_some_and(|g| g.0[0].group == "Test_Eyes"));
    let eye = material(&sim, EYE_TEX, true);
    let trail = material(&sim, TRAIL_TEX, true);
    let near = |sim: &Sim, e: Entity, m: usize| {
        let at = sim.app.world().get::<Transform>(e).unwrap().translation;
        particles(sim).into_iter().filter(|(_, p)| p.material == m && p.position.distance(at) < 2.0).count()
    };
    for i in 0..40 {
        sim.app.world_mut().get_mut::<Transform>(other).unwrap().translation.x = i as f32 * 0.02;
        sim.ticks(1);
    }
    assert_eq!(near(&sim, other, eye), 1, "one eye sprite each frame");
    let ribbons: Vec<_> = particles(&sim).into_iter().filter(|(_, p)| p.material == trail).map(|(_, p)| p).collect();
    assert!(!ribbons.is_empty(), "the trail's sections");
    assert!(ribbons.iter().all(|p| matches!(p.shape, Shape::Ribbon { width, .. } if (width[0] - 2.0 * UNIT).abs() < 1e-6)));
    sim.seconds(4.0);
    assert_eq!(near(&sim, other, eye), 1, "looping past its 3 s");
    // The same look on the local player: hidden in first person.
    let me = sim.spawn_character(Vec3::new(10.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.app.world_mut().entity_mut(me).insert((LocalPlayer, CaRungLook(infected)));
    sim.ticks(3);
    assert!(sim.app.world().get::<CaAttachedGroups>(me).is_some());
    assert_eq!(near(&sim, me, eye), 0, "you never see your own eyes");
    sim.app.insert_resource(ShowLocalBody(true));
    sim.ticks(2);
    assert_eq!(near(&sim, me, eye), 1, "seen in third person");
    // Changed back (another face): the groups go.
    sim.app.world_mut().entity_mut(other).insert(CaRungLook(Selection::default()));
    sim.ticks(3);
    assert!(sim.app.world().get::<CaAttachedGroups>(other).is_none());
    assert_eq!(sim.app.world().resource::<CaFx>().attached(other).count(), 0);
}
