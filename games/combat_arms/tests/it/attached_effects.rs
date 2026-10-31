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
//! - a projectile's activate, finish and create sounds (61-63);
//! - trail keys: width by style, cadence, texture u, colour over each
//!   section's life, ending (77-81); smoke puffs' size, growth and fade
//!   (82-83);
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
            attached::{AttachedGroup, CaAttachedGroups, CaFlightLook, CaFlightSounds, FlightSound, fly_sound_key, flight_sounds, pose_of, worn_groups},
            data::CaFxTables,
            fx::{CaFx, SmokeSegment, SmokeTrail},
            launched_burst,
            library::{FxLibrary, UNTEXTURED_PUFF, material_name, normalize, set_library, smoke_trail},
            trail::{Trail, TrailMode, TrailParams, WidthStyle},
        },
        inventory::UNIT,
        special::{CaObject, Kind, LOOK_SEPARATOR, PROJECTILE, ProjectileLook, ProjectileParams, facing},
        view_model::attached::CaLoopSound,
    },
    harness::{CaSim, CaSimWith},
};
use lithtech_formats::fxf::{FxFile, FxGroup, FxKey, FxProp, FxValue};

pub(super) const TAIL_TEX: &str = "test_fx/tail.dtx";
const EYE_SPR: &str = "test_fx/eye.spr";
const EYE_TEX: &str = "test_fx/eye.dtx";
const TRAIL_TEX: &str = "test_fx/trail.dtx";

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
        // The M136's FireFX group, `test` in the install: a smoke puff 4 u
        // ahead and a light, both on the launcher's `Flash` socket.
        FxGroup {
            name: "Test_Rocket_Fire".into(),
            phase_ms: 200,
            keys: vec![
                key(
                    "Sprite",
                    0.2,
                    [
                        socket("Flash"),
                        vec![
                            ("Sprite", path(EYE_SPR)),
                            ("BlendMode", combo(1, &["None", "Additive"])),
                            ("Offset", FxValue::Vector([0.0, 0.0, 4.0])),
                            ("Sk", FxValue::Vector4([0.0, 0.05, 0.0, 0.0])),
                        ],
                    ]
                    .concat(),
                ),
                key("DynaLight", 0.2, [socket("Flash"), vec![("Offset", FxValue::Vector([0.0, 0.0, 0.0]))]].concat()),
            ],
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
    let textures: HashMap<String, UVec2> = [EYE_TEX, TRAIL_TEX, TAIL_TEX].iter().map(|t| (t.to_string(), UVec2::splat(32))).collect();
    // The smoke trail's sprites are not in the install: its puffs are
    // untextured squares.
    let sprites: HashMap<String, String> = [(EYE_SPR.to_string(), EYE_TEX.to_string())].into_iter().collect();
    Arc::new(FxLibrary { file, textures, sprites })
}

/// The materials the groups draw with, in the pool's material list.
pub(super) fn materials(sim: &mut Sim) {
    let mut list = sim.app.world().get_resource::<ParticleMaterials>().map_or_else(MapParticles::default, |m| m.0.clone());
    if list.find(UNTEXTURED_PUFF).is_none() {
        list.materials.push(ParticleMaterial {
            name: UNTEXTURED_PUFF.into(),
            blend: ParticleBlend::Alpha,
            vertex_color: true,
            vertex_alpha: true,
            ..default()
        });
    }
    for (tex, add) in [(TAIL_TEX, false), (EYE_TEX, true), (TRAIL_TEX, true)] {
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

pub(super) fn material(sim: &Sim, tex: &str, add: bool) -> usize {
    sim.app.world().resource::<ParticleMaterials>().0.find(&material_name(tex, add)).unwrap()
}

/// Every live particle of the pool with its group's tag.
pub(super) fn particles(sim: &Sim) -> Vec<(Option<u64>, chimera_world::map::particles::Particle)> {
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

/// 50. `SmokeTrail` 1, type 2: a segment every 0.1 s, each emitting for
/// 0.2 s and living on 1.5 s after (puffs: 82-83); `Light` 1 makes no
/// light.
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
    let born: Vec<f32> = trail.segments.iter().map(|s| s.born).collect();
    assert!(born.len() >= 5 && born.iter().all(|s| ((s / 0.1).round() * 0.1 - s).abs() < 1e-3), "a segment each 0.1 s: {born:?}");
    assert!(sim.app.world().resource::<CaFx>().lights.is_empty(), "no light");
}

/// The puffs the trail of `e` has made: (segment tag, puff).
fn puffs(sim: &Sim) -> Vec<(u64, chimera_world::map::particles::Particle)> {
    let tex = sim.app.world().resource::<ParticleMaterials>().0.find(UNTEXTURED_PUFF).unwrap();
    particles(sim).into_iter().filter(|(t, p)| t.is_some() && p.material == tex).map(|(t, p)| (t.unwrap(), p)).collect()
}

/// 82. A puff's square: 4 · (2000 / screen width) · k u, k = 1 + 14 f of
/// its own life (10 s); its alpha 0.9 · (1 - f).
#[test]
fn t82_puff_size() {
    for (width, side0, side1) in [(1024.0, 7.8125, 18.75), (1920.0, 4.1666665, 10.0)] {
        let mut sim = sim_with(None);
        sim.app.world_mut().resource_mut::<CaFx>().screen_width = width;
        let look = ProjectileLook { smoke_trail: Some(2), ..default() };
        let e = projectile(&mut sim, &look, Vec3::ZERO);
        for i in 1..=10 {
            sim.app.world_mut().get_mut::<Transform>(e).unwrap().translation = Vec3::Z * i as f32 * 0.2;
            sim.ticks(1);
        }
        let mut p = puffs(&sim).first().expect("a puff").1.clone();
        assert!(matches!(p.shape, Shape::Sprite));
        assert!((p.life - 10.0).abs() < 0.05, "10 s plus a little: {}", p.life);
        assert!((p.size.at(0.0) * 2.0 / UNIT - side0).abs() < 1e-3, "{width}: side {}", p.size.at(0.0) * 2.0 / UNIT);
        // 1 s old of 10: f = 0.1, k = 2.4.
        p.age = 0.1 * p.life;
        assert!((p.current_size() * 2.0 / UNIT - side1).abs() < 1e-3);
        let own = p.alpha * p.fade.at(p.age, p.life);
        assert!((own - 0.81).abs() < 1e-3, "{own}");
    }
}

/// 83. A long white segment (life 1.5 s, emitting 0.2 s) is at full alpha
/// while emitting, 0.5 at 0.75 s after its last emission, gone at 1.5 s
/// with all its puffs; a puff shows its own alpha times the segment's;
/// every puff an untextured grey square.
#[test]
fn t83_puff_fade_and_look() {
    let seg = SmokeSegment { born: 0.0, last_emit: 0.2, tag: 1 };
    let (emitting, life) = (0.2, 1.5);
    assert_eq!(seg.alpha(0.1, emitting, life), 1.0);
    assert!((seg.alpha(0.2 + 0.75, emitting, life) - 0.5).abs() < 1e-6);
    assert_eq!(seg.alpha(0.2 + 1.5, emitting, life), 0.0);
    // A puff at f = 0.075 under a segment at 0.5: 0.9 · 0.925 · 0.5.
    assert!((0.9f32 * (1.0 - 0.075) * 0.5 - 0.416).abs() < 1e-3);

    let mut sim = sim_with(None);
    let look = ProjectileLook { smoke_trail: Some(2), ..default() };
    let e = projectile(&mut sim, &look, Vec3::ZERO);
    // One segment's worth of flight, then the projectile is gone.
    for i in 1..=10 {
        sim.app.world_mut().get_mut::<Transform>(e).unwrap().translation = Vec3::Z * i as f32 * 0.2;
        sim.ticks(1);
    }
    let all = puffs(&sim);
    assert!(!all.is_empty());
    for (_, p) in &all {
        assert!(p.color.x == p.color.y && p.color.y == p.color.z, "channels equal");
        assert!((150.0 / 255.0..=230.0 / 255.0).contains(&p.color.x), "{}", p.color.x);
    }
    // Keep it still and flying on: the first segment (made at 0) stops
    // emitting at 0.2 s and is gone 1.5 s after its last emission.
    let first_tag = all.iter().map(|(t, _)| *t).min().unwrap();
    let trail_age = |sim: &Sim| sim.app.world().get::<SmokeTrail>(e).unwrap().age;
    while trail_age(&sim) < 0.2 + 0.75 {
        sim.ticks(1);
    }
    let mid: Vec<_> = puffs(&sim).into_iter().filter(|(t, _)| *t == first_tag).collect();
    assert!(!mid.is_empty());
    for (_, p) in &mid {
        let a = p.alpha / 0.9;
        assert!((0.4..0.55).contains(&a), "about half: {a}");
    }
    while trail_age(&sim) < 0.2 + 1.5 + 0.05 {
        sim.ticks(1);
    }
    assert!(puffs(&sim).iter().all(|(t, _)| *t != first_tag), "segment and puffs gone");
    assert!(sim.app.world().get::<SmokeTrail>(e).unwrap().segments.iter().all(|s| s.tag != first_tag));
}

// 77-81: trail keys, on the trail's points without a scene.

fn eye_params(u_add: f32, unit: f32) -> TrailParams {
    TrailParams {
        width: 2.0,
        style: WidthStyle::BigToSmall,
        len: 50,
        lifespan: 0.2,
        interval: 0.03,
        u_add,
        colors: vec![
            (0.0, [1.0, 0.0, 0.0, 1.0]),
            (0.0347, [1.0, 0.502, 0.0, 1.0]),
            (0.1025, [1.0, 0.0, 0.0, 1.0]),
            (0.6661, [1.0, 0.0, 0.0, 1.0]),
            (1.0, [0.0, 0.0, 0.0, 1.0]),
        ],
        unit,
    }
}

/// 77. The half-width by style and point count (W 2, N 50).
#[test]
fn t77_trail_width() {
    let w = |style: WidthStyle, n: usize| style.half_width(2.0, 50, n);
    use WidthStyle::*;
    let row = |n: usize, want: [f32; 4]| {
        for (style, want) in [Constant, SmallToBig, SmallToSmall, BigToSmall].into_iter().zip(want) {
            assert!((w(style, n) - want).abs() < 1e-5, "{style:?} n={n}: {} vs {want}", w(style, n));
        }
    };
    row(2, [2.0, 0.0, 0.0, 2.0]);
    row(8, [2.0, 0.24, 0.24, 1.76]);
    row(30, [2.0, 1.12, 0.88, 0.88]);
}

/// 78. Points every second update after the third at 64 Hz (I 0.03,
/// L 0.25, N 50); the oldest goes at update 17, when 9 points were held;
/// then 8 and 7 alternate.
#[test]
fn t78_trail_cadence() {
    let p = TrailParams { lifespan: 0.25, ..eye_params(0.0, 1.0) };
    let mut t = Trail::default();
    let dt = 1.0 / 64.0;
    let mut made = Vec::new();
    let mut counted = Vec::new();
    let mut before = 0;
    for update in 1..=24 {
        t.update(&p, dt, Vec3::ZERO, TrailMode::Active);
        if t.counted() > before {
            made.push(update);
        }
        before = t.counted();
        counted.push(t.counted());
        if update == 17 {
            assert_eq!(t.points.len(), 8, "the first point removed");
        }
    }
    assert_eq!(&made[..5], &[2, 3, 5, 7, 9]);
    assert_eq!(&counted[1..17], &[1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9]);
    assert!(counted[17..].iter().all(|n| *n == 7 || *n == 8), "{counted:?}");
    // BigToSmall at n = 8: 1.76.
    assert!((p.style.half_width(2.0, 50, 8) - 1.76).abs() < 1e-5);
}

/// 79. Texture u: once per W u of path; `UAdd` scrolls toward the oldest
/// point.
#[test]
fn t79_trail_texture() {
    let p = TrailParams {
        width: 20.0,
        lifespan: 100.0,
        interval: 0.001,
        ..eye_params(1.0, 1.0)
    };
    let mut t = Trail::default();
    for x in [0.0, 10.0, 30.0] {
        t.update(&p, 0.05, Vec3::X * x, TrailMode::Active);
    }
    let us: Vec<f32> = t.points.iter().map(|q| q.u).collect();
    assert_eq!(us.len(), 3, "{us:?}");
    assert!((us[0]).abs() < 1e-6 && (us[1] - 0.5).abs() < 1e-5 && (us[2] - 1.5).abs() < 1e-5, "{us:?}");
    // UAdd 1 per second: after 0.15 s every drawn u is 0.15 larger.
    assert!((t.drawn_u(0) - us[0] - 0.15).abs() < 1e-5);
    let mut t = Trail::default();
    t.update(&p, 0.05, Vec3::ZERO, TrailMode::Active);
    for _ in 0..5 {
        t.update(&p, 0.05, Vec3::ZERO, TrailMode::ShuttingDown);
    }
    // The first update counts too: 6 x 0.05 s.
    assert!((t.drawn_u(0) - t.points[0].u - 0.30).abs() < 1e-5, "after 0.3 s every u is 0.3 larger");
    // A feature at u0 sits where u + O = u0: 0.25 · W = 5 u nearer the oldest.
    assert!((0.25 * p.width - 5.0).abs() < 1e-5);
}

/// 80. Colour by each section's age over the lifespan.
#[test]
fn t80_trail_colour() {
    let p = eye_params(0.0, 1.0);
    let c = p.color_at(0.1);
    assert!((c.x - 1.0).abs() < 1e-5 && c.y == 0.0 && c.z == 0.0 && c.w == 1.0, "{c}");
    let c = p.color_at(0.17);
    assert!((c.x - 0.449).abs() < 1e-3 && c.y == 0.0 && c.z == 0.0, "{c}");
    let c = p.color_at(0.00347);
    assert!((c.x - 1.0).abs() < 1e-5 && (c.y - 0.251).abs() < 1e-3 && c.z == 0.0, "{c}");
    // The newest section is near the curve start.
    let mut t = Trail::default();
    t.update(&p, 0.04, Vec3::ZERO, TrailMode::Active);
    t.update(&p, 0.04, Vec3::X, TrailMode::Active);
    let newest = t.points.last().unwrap();
    assert!(newest.age <= 0.04 + 1e-6);
    assert_eq!(newest.color, p.color_at(newest.age), "its own age's colour");
}

/// 81. A trail whose key shuts down adds no point and the newest stays;
/// the rest go as they reach L; with one left nothing is drawn. Hiding
/// clears; shown again it starts empty.
#[test]
fn t81_trail_ending() {
    let p = eye_params(0.0, 1.0);
    let mut t = Trail::default();
    for i in 0..10 {
        t.update(&p, 0.04, Vec3::X * i as f32, TrailMode::Active);
    }
    let n = t.points.len();
    assert!(n >= 4 && t.drawn());
    let newest = t.points.last().unwrap().at;
    for _ in 0..3 {
        t.update(&p, 0.04, Vec3::X * 99.0, TrailMode::ShuttingDown);
        assert_eq!(t.points.last().unwrap().at, newest, "the newest stays");
    }
    assert!(t.points.len() < n, "older ones vanish");
    for _ in 0..10 {
        t.update(&p, 0.04, Vec3::X * 99.0, TrailMode::ShuttingDown);
    }
    assert_eq!(t.points.len(), 1);
    assert!(!t.drawn());
    // Hidden: everything goes; the first point comes after I.
    let mut t = Trail::default();
    for i in 0..10 {
        t.update(&p, 0.04, Vec3::X * i as f32, TrailMode::Active);
    }
    t.clear();
    assert!(t.points.is_empty());
    t.update(&p, 0.02, Vec3::ZERO, TrailMode::Active);
    assert!(t.points.is_empty(), "before I");
    t.update(&p, 0.02, Vec3::ZERO, TrailMode::Active);
    assert_eq!(t.points.len(), 1);
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
    assert!(ribbons.iter().all(|p| matches!(p.shape, Shape::Ribbon { width, end_color: Some(_), .. } if (width[0] - 4.0 * UNIT).abs() < 1e-6)));
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

// 61-63: the projectile effect's activate, finish and create sounds.

fn mine_look() -> combat_arms::game::special::ProjectileLook {
    combat_arms::game::special::ProjectileLook {
        fly_sound: "guns_pt/snd/bomb_beep.wav".into(),
        activate_sound: "guns_snd_etc/nerve_confuse_mine/activeloop.wav".into(),
        finish_sound: "guns_snd_etc/nerve_confuse_mine/autofinish.wav".into(),
        finish_time: 2.4,
        lifetime: 60.0,
        sound_radius: 600.0,
        ..default()
    }
}

/// 61. `NerveConfuseMine`: the beep loops from launch and follows it; at
/// landing (velocity 0, 1.2 s) the beep stops and `ActiveLoop` loops at the
/// landing point; at 57.6 s `AutoFinish` plays once, replacing it, and the
/// object removed at 60 s cuts it. Radius 600 throughout.
#[test]
fn t61_nerve_mine_sounds() {
    // The rules, step by step.
    let look = mine_look();
    let mut now = FlightSound::None;
    assert_eq!(flight_sounds(&mut now, &look, 0.0, false), vec![FlightSound::Fly]);
    assert_eq!(flight_sounds(&mut now, &look, 0.5, false), vec![], "once");
    assert_eq!(flight_sounds(&mut now, &look, 1.2, true), vec![FlightSound::Activate]);
    assert_eq!(flight_sounds(&mut now, &look, 30.0, true), vec![], "looping on its own");
    assert_eq!(flight_sounds(&mut now, &look, 57.59, true), vec![]);
    assert_eq!(flight_sounds(&mut now, &look, 57.61, true), vec![FlightSound::Finish]);
    assert_eq!(flight_sounds(&mut now, &look, 58.0, false), vec![], "nothing more changes");
    // On the harness: the messages the sounds make.
    let mut sim = sim_with(None);
    let e = sim
        .app
        .world_mut()
        .spawn((CaFlightLook(look.clone()), CaFlightSounds::default(), Transform::from_xyz(0.0, 1.0, 0.0)))
        .id();
    sim.app.world_mut().resource_mut::<Loops>().0.clear();
    let key = fly_sound_key(e);
    let started = |sim: &mut Sim| {
        std::mem::take(&mut sim.app.world_mut().resource_mut::<Loops>().0).into_iter().filter(|l| l.key == key).collect::<Vec<_>>()
    };
    // Flying: moving every tick for 1.2 s.
    for i in 1..=80 {
        sim.app.world_mut().get_mut::<Transform>(e).unwrap().translation.x = i as f32 * 0.1;
        sim.ticks(1);
    }
    let l = started(&mut sim);
    assert_eq!(l.len(), 1, "the beep, once: {l:?}");
    let s = l[0].sound.as_ref().unwrap();
    assert_eq!((s.file.as_str(), s.radii.1, s.volume), ("guns_pt/snd/bomb_beep.wav", 600.0, 100.0));
    assert!(!l[0].once && l[0].follow == Some(e));
    // Landed: still from now on.
    let landing = sim.app.world().get::<Transform>(e).unwrap().translation;
    sim.ticks(3);
    let l = started(&mut sim);
    assert_eq!(l.len(), 1, "{l:?}");
    let s = l[0].sound.as_ref().unwrap();
    assert_eq!(s.file, "guns_snd_etc/nerve_confuse_mine/activeloop.wav");
    assert!(s.at.unwrap().distance(landing) < 1e-4, "at the landing point");
    assert!(!l[0].once && l[0].follow.is_none(), "looping, not following");
    // 57.6 s after the effect was made: the finish, once.
    sim.seconds((57.7 - sim.app.world().get::<CaFlightSounds>(e).unwrap().age) as f64);
    let l = started(&mut sim);
    assert_eq!(l.len(), 1, "{l:?}");
    assert_eq!(l[0].sound.as_ref().unwrap().file, "guns_snd_etc/nerve_confuse_mine/autofinish.wav");
    assert!(l[0].once);
    // Removed: cut.
    sim.app.world_mut().entity_mut(e).despawn();
    sim.ticks(1);
    let l = started(&mut sim);
    assert!(l.len() == 1 && l[0].sound.is_none(), "stopped: {l:?}");
}

/// 62. `AssassinBomb_Projectile` (finish 5 s before its 30 s) still in
/// flight at 25 s: the finish sound starts once, where it is then, replacing
/// the fly sound; not moved afterwards.
#[test]
fn t62_assassin_bomb_finish() {
    let look = combat_arms::game::special::ProjectileLook {
        fly_sound: "guns_pt/snd/grenade.wav".into(),
        activate_sound: "guns_pt/snd/snow_fly.wav".into(),
        finish_sound: "guns_pt/snd/grenade.wav".into(),
        finish_time: 5.0,
        lifetime: 30.0,
        sound_radius: 1000.0,
        ..default()
    };
    let mut now = FlightSound::None;
    assert_eq!(flight_sounds(&mut now, &look, 0.0, false), vec![FlightSound::Fly]);
    assert_eq!(flight_sounds(&mut now, &look, 24.99, false), vec![]);
    assert_eq!(flight_sounds(&mut now, &look, 25.01, false), vec![FlightSound::Finish], "once, replacing the fly sound");
    assert_eq!(flight_sounds(&mut now, &look, 26.0, false), vec![]);
    // A record with no finish sound (or no lifetime) never makes one.
    let mut none = FlightSound::None;
    let quiet = combat_arms::game::special::ProjectileLook { finish_sound: String::new(), ..look.clone() };
    assert!(!flight_sounds(&mut none, &quiet, 29.0, false).contains(&FlightSound::Finish));
    // Finish time default 0: at the lifetime's end.
    let mut none = FlightSound::None;
    let at_end = combat_arms::game::special::ProjectileLook { finish_time: 0.0, ..look.clone() };
    assert!(!flight_sounds(&mut none, &at_end, 29.9, false).contains(&FlightSound::Finish));
    assert!(flight_sounds(&mut none, &at_end, 30.01, false).contains(&FlightSound::Finish));
    // Before launch (zero velocity, FireDelayTime): the activate sound.
    let mut none = FlightSound::None;
    assert_eq!(flight_sounds(&mut none, &look, 0.0, true), vec![FlightSound::Activate]);
    assert_eq!(flight_sounds(&mut none, &look, 0.1, false), vec![FlightSound::Fly], "replaced at launch");
}

/// 63. A player throws `Caltrops` (create sound `fire.wav`, radius 300,
/// weapon `PlayAutoThrowSound` 0): one server sound at his fire position and
/// no call-out; a weapon whose call-out is on makes the call-out only; no
/// radius or no file: nothing.
#[test]
fn t63_create_sound() {
    use combat_arms::game::effects::sounds::create_sound;
    let at = Vec3::new(1.0, 2.0, 3.0);
    let s = create_sound("guns_snd_throwing/caltrops/fire.wav", 300.0, false, at).expect("a sound");
    assert_eq!((s.file.as_str(), s.radii.1, s.at, s.volume, s.server), ("guns_snd_throwing/caltrops/fire.wav", 300.0, Some(at), 100.0, true));
    assert!(create_sound("guns_snd_throwing/caltrops/fire.wav", 300.0, true, at).is_none(), "the call-out stands in");
    assert!(create_sound("", 300.0, false, at).is_none());
    assert!(create_sound("x.wav", 0.0, false, at).is_none());
    // Our caltrops document carries them.
    let set = CaData::builtin_static().weapon_set();
    let w = set.get("combat_arms:caltrops").expect("caltrops");
    let d: combat_arms::game::special::DeviceParams = w.hook(combat_arms::game::special::DEVICE).unwrap();
    assert_eq!((d.create_sound.as_str(), d.create_radius, d.auto_throw), ("guns_snd_throwing/caltrops/fire.wav", 300.0, false));
    // ... and the nerve mine its sounds.
    let w = set.get("combat_arms:nerve_confuse_mine").expect("nerve mine");
    let d: combat_arms::game::special::DeviceParams = w.hook(combat_arms::game::special::DEVICE).unwrap();
    assert_eq!(d.look, mine_look());
}

/// 58 (placement): the shooter's own launcher group is made on him, at his
/// point, each key placing itself by its mode: the puff at the held
/// launcher's `Flash` socket 4 u along the character's forward, the light at
/// the socket.
#[test]
fn t58_own_launcher_group_on_the_shooter() {
    use combat_arms::game::effects::{CaFxRequest, FxKind};
    let mut sim = sim_with(None);
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.app.world_mut().entity_mut(me).insert(CaPose { right_hand: Some(Vec3::new(8.0, 40.0, -12.0)), ..default() });
    sim.ticks(2);
    let (t, state) = {
        let w = sim.app.world();
        (*w.get::<Transform>(me).unwrap(), w.get::<chimera_core::core::MovementState>(me).unwrap().clone())
    };
    let pose = sim.app.world().get::<CaPose>(me).unwrap().clone();
    let flash = pose.world(&t, &state, UNIT, pose.right_hand.expect("kept"));
    sim.app.world_mut().write_message(CaFxRequest {
        group: "Test_Rocket_Fire".into(),
        at: Vec3::new(9.0, 9.0, 9.0),
        normal: Vec3::Z,
        kind: FxKind::OwnParented(me),
    });
    sim.ticks(1);
    assert_eq!(sim.app.world().resource::<CaFx>().attached(me).count(), 1, "parented to the shooter");
    // His body faces -Z (yaw 0): 4 u along it from the socket.
    let want = flash + Vec3::NEG_Z * 4.0 * UNIT;
    let eye = material(&sim, EYE_TEX, true);
    let puffs: Vec<_> = particles(&sim).into_iter().filter(|(_, p)| p.material == eye).collect();
    assert_eq!(puffs.len(), 1, "one puff");
    assert!(puffs[0].1.position.distance(want) < 1e-4, "{} vs {want}", puffs[0].1.position);
    let lights = &sim.app.world().resource::<CaFx>().lights;
    assert_eq!(lights.len(), 1);
    assert!(lights[0].at.distance(flash) < 1e-4, "the light at the socket");
    // It is spent after its length, and nothing keeps it.
    sim.seconds(0.5);
    assert_eq!(sim.app.world().resource::<CaFx>().attached(me).count(), 0);
}
