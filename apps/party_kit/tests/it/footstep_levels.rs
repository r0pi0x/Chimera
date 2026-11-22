//! How far each game's identity is heard stepping (docs/backlog.md, "Loudness
//! and hearing range differ by game"), as Chimera's mixer computes it: the
//! party's soldier `b` walks, runs, sprints and jumps holding CS:S's Glock,
//! then Combat Arms' M4, while the local player `a` listens; each step's
//! volume and falloff (CS:S's `PlaySound`s by the bank entry's level, as
//! CS:S's scripts give every footstep: CompatibilityAttenuation 1.0,
//! specs/cs_source/sounds.md; Combat Arms' `CaSound`s by `CaSound::
//! sound_level`) give its gain at 5, 15 and 30 m (`distance_gain`) and the
//! distance it is silent from. `party_even_footsteps 0` (the default) keeps
//! each game's own; 1 gives both one profile, CS:S's. The table prints with
//! `--nocapture`.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::core::{Carried, LocalPlayer, PropSurface};
use chimera_world::{
    greybox::GreyboxPart,
    harness::Sim,
    map::{
        MapSounds, MapSurface, PlaySound, SoundLevel,
        sound::{EvenFootsteps, Interval, MapSoundEntry, SoundBank, distance_gain, silent_from},
    },
};
use combat_arms::game::{
    effects::{
        rules::{inner_radius, FOOTSTEP_RADIUS},
        sounds::CaSound,
    },
    inventory::UNIT,
};
use cs_source::game::weapons as cs_weapons;

use crate::gun_game::{held, party, soldier};

const M4: &str = "combat_arms:m4a1";
const FLOOR: &str = "concrete";
const STEP: &str = "Concrete.Step";
const DISTANCES: [f32; 3] = [5.0, 15.0, 30.0];

/// One step heard: its volume (0..1) and falloff.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Step {
    volume: f32,
    level: SoundLevel,
}

impl Step {
    fn gain(&self, metres: f32) -> f32 {
        distance_gain(self.level, metres) * self.volume
    }
}

#[derive(Resource, Default)]
struct Steps(Vec<Step>, Option<Entity>);

fn hear(
    mut play: MessageReader<PlaySound>,
    mut ca: MessageReader<CaSound>,
    bank: Option<Res<SoundBank>>,
    mut steps: ResMut<Steps>,
) {
    let who = steps.1;
    for s in play.read() {
        if s.source.is_some() && s.source == who && s.entry.starts_with(STEP) {
            let level = bank.as_ref().and_then(|b| b.0.entry(&s.entry)).map(|e| e.level).expect("a step entry");
            steps.0.push(Step {
                volume: s.volume.unwrap_or(1.0),
                level,
            });
        }
    }
    for s in ca.read() {
        if s.at.is_some() && s.file.contains("footsteps") {
            steps.0.push(Step {
                volume: s.volume / 100.0,
                level: s.sound_level(),
            });
        }
    }
}

/// The party: `a` listening (local), `b` walking; the floor CS:S's
/// concrete with its two step entries.
fn sim(even: bool) -> (Sim, Entity) {
    let mut sim = party();
    if even {
        sim.app.world_mut().resource_mut::<EvenFootsteps>().on = 1;
    }
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.app.world_mut().entity_mut(a).insert(LocalPlayer);
    sim.app.insert_resource(Steps(Vec::new(), Some(b))).add_systems(Last, hear);
    sim.seconds(1.5);
    let w = sim.app.world_mut();
    let parts: Vec<Entity> = w.query_filtered::<Entity, With<GreyboxPart>>().iter(w).collect();
    for e in parts {
        w.entity_mut(e).insert(PropSurface(FLOOR.into()));
    }
    let mut sounds = MapSounds::default();
    sounds.surfaces.insert(
        FLOOR.into(),
        MapSurface {
            step_left: Some(format!("{STEP}Left")),
            step_right: Some(format!("{STEP}Right")),
            game_material: 'C',
            ..default()
        },
    );
    for side in ["left", "right"] {
        sounds.entries.insert(
            format!("{STEP}{side}").to_lowercase(),
            MapSoundEntry {
                waves: Vec::new(),
                volume: Interval::fixed(1.0),
                pitch: Interval::fixed(100.0),
                level: SoundLevel::Attenuation(1.0),
                channel: 4,
                dry: false,
                hearing: None,
            },
        );
    }
    sim.app.insert_resource(SoundBank(Arc::new(sounds)));
    assert_eq!(held(&sim, b), Some(cs_weapons::GLOCK));
    (sim, b)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Walk,
    Run,
    Sprint,
    Jump,
}

const ACTIONS: [Action; 4] = [Action::Walk, Action::Run, Action::Sprint, Action::Jump];

/// The steps `b` makes doing `action` (moving forward and back, or one
/// jump and its landing).
fn steps(sim: &mut Sim, b: Entity, action: Action) -> Vec<Step> {
    sim.seconds(1.0);
    sim.app.world_mut().resource_mut::<Steps>().0.clear();
    if action == Action::Jump {
        sim.intent(b).jump = true;
        sim.ticks(2);
        sim.intent(b).jump = false;
        sim.seconds(1.2);
    } else {
        for axis in [Vec2::Y, Vec2::NEG_Y] {
            {
                let mut i = sim.intent(b);
                i.move_axis = axis;
                i.walk = action == Action::Walk;
                i.sprint = action == Action::Sprint;
            }
            sim.seconds(1.0);
        }
        let mut i = sim.intent(b);
        i.move_axis = Vec2::ZERO;
        i.walk = false;
        i.sprint = false;
    }
    std::mem::take(&mut sim.app.world_mut().resource_mut::<Steps>().0)
}

/// Each action's loudest step for `b` holding CS:S's Glock, then Combat
/// Arms' M4.
fn measure(even: bool) -> Vec<(&'static str, Action, Option<Step>)> {
    let (mut sim, b) = sim(even);
    let mut out = Vec::new();
    for (game, gun) in [("cs_source", None), ("combat_arms", Some(M4))] {
        if let Some(gun) = gun {
            chimera_sim::weapon::give(sim.app.world_mut(), b, gun);
            sim.seconds(1.0);
            assert_eq!(held(&sim, b), Some(gun));
        }
        for action in ACTIONS {
            let loudest = steps(&mut sim, b, action).into_iter().max_by(|x, y| x.gain(5.0).total_cmp(&y.gain(5.0)));
            out.push((game, action, loudest));
        }
    }
    print(even, &out);
    out
}

fn print(even: bool, rows: &[(&str, Action, Option<Step>)]) {
    println!("party_even_footsteps {}:", u8::from(even));
    println!("  {:<12} {:<7} {:>6} {:>8} {:>8} {:>8} {:>10}  level", "identity", "action", "volume", "5 m", "15 m", "30 m", "silent at");
    for (game, action, step) in rows {
        match step {
            None => println!("  {game:<12} {:<7} silent", format!("{action:?}")),
            Some(s) => println!(
                "  {game:<12} {:<7} {:>6.3} {:>8.3} {:>8.3} {:>8.3} {:>9.1}m  {:?}",
                format!("{action:?}"),
                s.volume,
                s.gain(DISTANCES[0]),
                s.gain(DISTANCES[1]),
                s.gain(DISTANCES[2]),
                silent_from(s.level).unwrap_or(f32::INFINITY),
                s.level
            ),
        }
    }
}

fn row(rows: &[(&str, Action, Option<Step>)], game: &str, action: Action) -> Option<Step> {
    rows.iter().find(|r| r.0 == game && r.1 == action).and_then(|r| r.2)
}

#[test]
fn off_each_game_keeps_its_own_falloff() {
    let rows = measure(false);
    let cs = row(&rows, "cs_source", Action::Run).expect("CS:S's running steps");
    let ca = row(&rows, "combat_arms", Action::Run).expect("Combat Arms' running steps");
    assert_eq!(cs.level, SoundLevel::Attenuation(1.0), "CS:S's step level");
    assert!((cs.volume - 0.5).abs() < 1e-4, "CS:S's running step volume: {}", cs.volume);
    let SoundLevel::Radius { inner, outer, .. } = ca.level else {
        panic!("Combat Arms' steps fall off by radius: {:?}", ca.level)
    };
    // FootStepSoundRadius, the inner radius 7 % of it, in the game's unit.
    let want = FOOTSTEP_RADIUS * UNIT;
    assert!((outer - want).abs() < 1e-3 && (inner - inner_radius(want)).abs() < 1e-3, "radii {inner} {outer}");
    assert!((ca.volume - 1.0).abs() < 1e-4, "Combat Arms' step at full volume: {}", ca.volume);
    // The gap the setting closes: Combat Arms' steps are louder close by.
    assert!(ca.gain(5.0) > cs.gain(5.0) * 2.0, "{} vs {}", ca.gain(5.0), cs.gain(5.0));
}

#[test]
fn on_both_identities_are_heard_alike() {
    let rows = measure(true);
    for action in [Action::Run, Action::Sprint, Action::Jump] {
        let cs = row(&rows, "cs_source", action).unwrap_or_else(|| panic!("CS:S {action:?} steps"));
        let ca = row(&rows, "combat_arms", action).unwrap_or_else(|| panic!("Combat Arms {action:?} steps"));
        assert_eq!(ca.level, cs.level, "{action:?}: one falloff");
        assert!((ca.volume - cs.volume).abs() < 1e-4, "{action:?}: one volume, {} vs {}", ca.volume, cs.volume);
        for d in DISTANCES {
            assert!((ca.gain(d) - cs.gain(d)).abs() < 1e-4, "{action:?} at {d} m");
        }
        assert_eq!(silent_from(ca.level), silent_from(cs.level), "{action:?}: one audible range");
    }
    // CS:S's own steps don't change.
    let cs = row(&rows, "cs_source", Action::Run).unwrap();
    assert_eq!(cs.level, SoundLevel::Attenuation(1.0));
}

/// Every step heard, anyone's (the local player's own Combat Arms steps
/// play unplaced).
#[derive(Resource, Default)]
struct AnySteps(usize);

fn hear_any(mut play: MessageReader<PlaySound>, mut ca: MessageReader<CaSound>, mut n: ResMut<AnySteps>) {
    n.0 += play.read().filter(|s| s.entry.starts_with(STEP)).count();
    n.0 += ca.read().filter(|s| s.file.contains("footsteps")).count();
}

/// Carried along (a vehicle's seat moving them each tick).
fn ride_along(mut q: Query<&mut Transform, With<Carried>>) {
    for mut t in &mut q {
        t.translation.x += 0.1;
    }
}

/// The steps the local player `a` and `b` make holding the movement keys
/// forward and back for two seconds.
fn walk_both(sim: &mut Sim, a: Entity, b: Entity) -> usize {
    sim.app.world_mut().resource_mut::<AnySteps>().0 = 0;
    for axis in [Vec2::Y, Vec2::NEG_Y] {
        sim.intent(a).move_axis = axis;
        sim.intent(b).move_axis = axis;
        sim.seconds(1.0);
    }
    sim.intent(a).move_axis = Vec2::ZERO;
    sim.intent(b).move_axis = Vec2::ZERO;
    sim.seconds(0.5);
    std::mem::take(&mut sim.app.world_mut().resource_mut::<AnySteps>().0)
}

/// A carried character (the engine's `Carried`: a vehicle's seat) makes no
/// steps, the local player's own or another's, holding the movement keys
/// and carried along, as CS:S's identity and as Combat Arms'; on its feet
/// again it steps.
#[test]
fn a_carried_character_makes_no_steps() {
    let (mut sim, b) = sim(false);
    let w = sim.app.world_mut();
    let a = w.query_filtered::<Entity, With<LocalPlayer>>().single(w).unwrap();
    sim.app.init_resource::<AnySteps>().add_systems(Last, hear_any).add_systems(FixedUpdate, ride_along);
    let seat = sim.app.world_mut().spawn(Transform::default()).id();
    for gun in [None, Some(M4)] {
        if let Some(gun) = gun {
            for p in [a, b] {
                chimera_sim::weapon::give(sim.app.world_mut(), p, gun);
            }
            sim.seconds(1.0);
        }
        let game = gun.map_or("CS:S", |_| "Combat Arms");
        assert!(walk_both(&mut sim, a, b) > 0, "{game}: walking steps");
        for p in [a, b] {
            sim.app.world_mut().entity_mut(p).insert(Carried(seat));
        }
        assert_eq!(walk_both(&mut sim, a, b), 0, "{game}: carried, no steps");
        for p in [a, b] {
            sim.app.world_mut().entity_mut(p).remove::<Carried>();
        }
        sim.seconds(1.0);
    }
}
