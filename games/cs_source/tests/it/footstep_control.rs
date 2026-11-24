//! func_footstep_control (`MapSounds::step_remaps`, from the map by
//! `games::cs_source::sound::step_remaps`): a player whose feet are inside
//! a volume steps on its `source` surface with its `destination`'s
//! sounds. A small map in Source units: a concrete floor at z = 0 and a
//! ladder slab whose face (normal -X) is at x = 1000.

use bevy::prelude::*;
use crate::stack::{
    games::cs_source::movement::{self, QuakeMovement, SourceMovementPlugin, to_engine, to_source},
    harness::{CsSim, Sim},
    map::{MapBrush, MapBrushes, MapSounds, MapSurface, PlaySound, StepRemap, sound::SoundBank},
};

const LADDER_X: f32 = 1000.0;

fn brush(lo: Vec3, hi: Vec3) -> MapBrush {
    let (a, b) = (to_engine(lo), to_engine(hi));
    MapBrush::from_box(a.min(b), a.max(b))
}

/// An axis-aligned remap volume (Source units) as the loader gives it:
/// outward planes in engine space.
fn remap(lo: Vec3, hi: Vec3, from: &str, to: &str) -> StepRemap {
    let (a, b) = (to_engine(lo), to_engine(hi));
    let (lo, hi) = (a.min(b), a.max(b));
    StepRemap {
        hulls: vec![vec![
            (Vec3::X, hi.x),
            (Vec3::NEG_X, -lo.x),
            (Vec3::Y, hi.y),
            (Vec3::NEG_Y, -lo.y),
            (Vec3::Z, hi.z),
            (Vec3::NEG_Z, -lo.z),
        ]],
        from: from.into(),
        to: to.into(),
    }
}

fn test_map(mut commands: Commands) {
    commands.insert_resource(MapBrushes(vec![
        MapBrush {
            surface: Some("concrete".into()),
            ..brush(Vec3::new(-8192.0, -8192.0, -64.0), Vec3::new(8192.0, 8192.0, 0.0))
        },
        MapBrush {
            ladder: true,
            ..brush(Vec3::new(LADDER_X, -64.0, 0.0), Vec3::new(LADDER_X + 8.0, 64.0, 400.0))
        },
    ]));
    let mut sounds = MapSounds::default();
    for (name, step) in [("concrete", "Concrete"), ("wood", "Wood"), ("ladder", "Ladder"), ("dirt", "Dirt")] {
        sounds.surfaces.insert(
            name.into(),
            MapSurface {
                step_left: Some(format!("{step}.StepLeft")),
                step_right: Some(format!("{step}.StepRight")),
                game_material: 'C',
                ..default()
            },
        );
    }
    sounds.step_remaps = vec![
        // Concrete sounds like wood over x 0..200.
        remap(Vec3::new(0.0, -256.0, -16.0), Vec3::new(200.0, 256.0, 64.0), "concrete", "wood"),
        // Another surface's remap over the whole walk: no effect here.
        remap(Vec3::new(-1000.0, -256.0, -16.0), Vec3::new(800.0, 256.0, 64.0), "metal", "dirt"),
        // The ladder's upper part sounds like wood (cs_militia's deck).
        remap(
            Vec3::new(LADDER_X - 64.0, -64.0, 200.0),
            Vec3::new(LADDER_X, 64.0, 600.0),
            "ladder",
            "wood",
        ),
    ];
    commands.insert_resource(SoundBank(std::sync::Arc::new(sounds)));
}

struct TestMap;
impl Plugin for TestMap {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, test_map);
    }
}

/// Footsteps played: entry and where (Source units).
#[derive(Resource, Default)]
struct Heard(Vec<(String, Vec3)>);

fn hear(mut sounds: MessageReader<PlaySound>, mut heard: ResMut<Heard>) {
    heard
        .0
        .extend(sounds.read().filter_map(|s| Some((s.entry.clone(), to_source(s.at?)))));
}

fn sim_at(feet: Vec3) -> (Sim, Entity) {
    let mut sim = Sim::new(CsSim, (TestMap, SourceMovementPlugin));
    sim.set_tick_interval(crate::stack::games::cs_source::TICK_INTERVAL);
    movement::set_config(sim.app.world_mut(), movement::config());
    sim.app.init_resource::<Heard>().add_systems(Last, hear);
    let p = sim.spawn_character(to_engine(feet + Vec3::Z * 36.0), movement::ID);
    (sim, p)
}

fn kind(entry: &str) -> &str {
    entry.split('.').next().unwrap_or("")
}

/// Running +x across the volume: concrete steps before and after it,
/// wood steps inside; the other surface's remap changes nothing.
#[test]
fn walking_through_a_footstep_volume() {
    let (mut sim, p) = sim_at(Vec3::new(-400.0, 0.0, 0.5));
    sim.ticks(4);
    sim.intent(p).yaw = -std::f32::consts::FRAC_PI_2;
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(3.2);
    let heard = sim.app.world().resource::<Heard>().0.clone();
    let x_end = to_source(sim.position(p)).x;
    assert!(x_end > 300.0, "walked through: x {x_end}");
    let (mut before, mut inside, mut after) = (0, 0, 0);
    for (entry, at) in &heard {
        let want = if (0.0..=200.0).contains(&at.x) { "Wood" } else { "Concrete" };
        assert_eq!(kind(entry), want, "step at x {}: {heard:?}", at.x);
        match at.x {
            x if x < 0.0 => before += 1,
            x if x <= 200.0 => inside += 1,
            _ => after += 1,
        }
    }
    assert!(before >= 2 && inside >= 2 && after >= 1, "steps {before}/{inside}/{after}: {heard:?}");
}

/// Climbing a ladder into a volume that remaps "ladder" (as cs_militia's
/// two do): ladder steps below it, the destination's steps once the feet
/// are inside.
#[test]
fn climbing_into_a_ladder_remap() {
    let (mut sim, p) = sim_at(Vec3::new(LADDER_X - 17.0, 0.0, 100.0));
    sim.app.world_mut().get_mut::<QuakeMovement>(p).unwrap().ladder = Some(Vec3::NEG_X);
    sim.intent(p).yaw = -std::f32::consts::FRAC_PI_2;
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.4);
    let heard = sim.app.world().resource::<Heard>().0.clone();
    let (mut below, mut inside) = (0, 0);
    for (entry, at) in &heard {
        if at.z >= 200.0 {
            assert_eq!(kind(entry), "Wood", "step at z {}: {heard:?}", at.z);
            inside += 1;
        } else {
            assert_eq!(kind(entry), "Ladder", "step at z {}: {heard:?}", at.z);
            below += 1;
        }
    }
    assert!(below >= 1 && inside >= 1, "steps {below}/{inside}: {heard:?}");
}

/// `sv_footsteps 0` silences steps (sounds.md 3); the cvar reads back.
#[test]
fn sv_footsteps_zero_silences_steps() {
    let run = |value: Option<&str>| {
        let (mut sim, p) = sim_at(Vec3::new(-400.0, 0.0, 0.5));
        if let Some(v) = value {
            movement::config_mut(sim.app.world_mut())
                .set_cvar("sv_footsteps", v)
                .unwrap();
        }
        sim.ticks(4);
        sim.intent(p).yaw = -std::f32::consts::FRAC_PI_2;
        sim.intent(p).move_axis = Vec2::Y;
        sim.seconds(1.5);
        sim.app.world().resource::<Heard>().0.len()
    };
    assert!(run(None) >= 2, "steps by default");
    assert_eq!(run(Some("0")), 0, "none with sv_footsteps 0");
    assert!(run(Some("1")) >= 2);
    let mut c = movement::config();
    assert_eq!(c.get_cvar("sv_footsteps").as_deref(), Some("1"));
    c.set_cvar("sv_footsteps", "0").unwrap();
    assert_eq!(c.get_cvar("sv_footsteps").as_deref(), Some("0"));
}
