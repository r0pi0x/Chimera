//! Halo's first-person view (`game::view`, specs/halo_ce/animations.md
//! "First person", "Playback"; weapons.md 2, 11, 13, 16) on synthetic
//! animation sets: the slots' sequence names, the weapon events' cues, the
//! playback step, each state lasting its animation's frame count in ticks
//! (for each gun the document's draw, reload, shell and melee numbers),
//! the shotgun's enter / shell / exit chain, the states fire may enter
//! from, the push back on a shot, hiding while zoomed; and on `HaloSim`
//! with a stand-in view model: giving the assault rifle readies it, a shot
//! pushes the gun back without a fire state, and a reload plays the reload
//! for its full length, in step with the weapon's own timer.

use std::sync::Arc;

use bevy::prelude::*;
use blam_formats::animation::FirstPersonSlot;
use chimera_sim::weapon::{Magazine, ViewModelHidden, WeaponEventKind, give};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{
        MapViewModel, ViewModels,
        anim::{AnimEvent, AnimSet, Animation, Sequence, Track},
    },
};
use halo_ce::{
    game::{
        body::anims::{BIND, LOOP_EVENT},
        movement,
        view::{
            HaloViewModel, cue,
            machine::{
                Cue, HaloView, KEY_EVENT, OVERLAY, Step, Tick, ViewState, advance, numbers, sequence, sequence_name,
            },
        },
        weapons::{ASSAULT_RIFLE, GUNS, PISTOL, gun},
    },
    harness::HaloSim,
};

/// A two-bone set with `slots` (slot, frames, loop frame), each a base
/// animation turning bone 0 a little per frame; `moving` and `overlays`
/// as overlays moving bone 1.
fn set_of(slots: &[(FirstPersonSlot, usize, i32)]) -> AnimSet {
    let defaults = vec![(Quat::IDENTITY, Vec3::ZERO), (Quat::IDENTITY, Vec3::X)];
    let mut set = AnimSet {
        defaults: defaults.clone(),
        bases: vec![defaults],
        parents: vec![None, Some(0)],
        ..Default::default()
    };
    set.animations.push(Animation {
        name: BIND.into(),
        fps: 30.0,
        frames: 1,
        ..Default::default()
    });
    set.sequences.push(Sequence {
        name: BIND.into(),
        anims: vec![0],
        grid: (1, 1),
        ..Default::default()
    });
    set.names.insert(BIND.into(), 0);
    for &(slot, frames, looping) in slots {
        let overlay = matches!(slot, FirstPersonSlot::Moving | FirstPersonSlot::Overlays);
        let track = if overlay {
            Track {
                bone: 1,
                rotation: None,
                position: Some((0..frames).map(|f| Vec3::Y * (f + 1) as f32).collect()),
            }
        } else {
            Track {
                bone: 0,
                rotation: Some((0..frames).map(|f| Quat::from_rotation_z(f as f32 * 0.01)).collect()),
                position: None,
            }
        };
        set.animations.push(Animation {
            name: slot.label().into(),
            fps: 30.0,
            frames,
            delta: overlay,
            tracks: vec![track],
            ..Default::default()
        });
        let ev = |name: &str, n: i32| AnimEvent {
            cycle: 0.0,
            event: n,
            name: name.into(),
            options: String::new(),
        };
        set.sequences.push(Sequence {
            name: sequence_name(slot).into(),
            activity: if overlay { OVERLAY.into() } else { "base".into() },
            anims: vec![set.animations.len() - 1],
            grid: (1, 1),
            events: vec![ev(LOOP_EVENT, looping), ev(KEY_EVENT, 0)],
            ..Default::default()
        });
        set.names.insert(sequence_name(slot).into(), set.sequences.len() - 1);
    }
    set
}

use FirstPersonSlot as S;

/// The assault rifle's slots as the disc has them (animations.md
/// "First-person graphs"): no fire-1.
fn rifle() -> AnimSet {
    set_of(&[
        (S::Idle, 80, 0),
        (S::Posing, 118, 0),
        (S::Moving, 26, 0),
        (S::Overlays, 9, 0),
        (S::ReloadEmpty, 87, 0),
        (S::ReloadFull, 87, 0),
        (S::Ready, 29, 0),
        (S::PutAway, 11, 0),
        (S::Melee, 37, 0),
    ])
}

/// Ticks the view stays in its current state, ticking with no input.
fn lasts(view: &mut HaloView, set: &AnimSet) -> u32 {
    let state = view.state;
    // The tick of the cue showed frame 0.
    let mut n = 1;
    while n < 1000 {
        view.tick(set, &Tick::default());
        if view.state != state {
            return n;
        }
        n += 1;
    }
    n
}

#[test]
fn slots_are_named_by_their_labels() {
    assert_eq!(sequence_name(S::Fire1), "fire-1");
    assert_eq!(sequence_name(S::ReloadEmpty), "reload-empty");
    assert_eq!(sequence_name(S::OverheatedExit), "o-h-exit");
    let mut names: Vec<&str> = S::ALL.iter().map(|s| sequence_name(*s)).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), S::COUNT, "every slot its own sequence");
    assert!(!names.contains(&BIND));
}

#[test]
fn weapon_events_are_cues() {
    let a = WeaponEventKind::Action;
    assert_eq!(cue(&WeaponEventKind::Deployed, None), Some(Cue::Ready));
    assert_eq!(cue(&a("fire"), None), Some(Cue::Fire));
    assert_eq!(cue(&a("fire_last"), None), Some(Cue::Fire));
    assert_eq!(cue(&a("misfire"), None), Some(Cue::Misfire));
    assert_eq!(cue(&a("melee"), None), Some(Cue::Melee));
    assert_eq!(cue(&a("overheated"), None), Some(Cue::Overheat));
    assert_eq!(cue(&a("overheat_cleared"), None), Some(Cue::OverheatCleared));
    assert_eq!(cue(&WeaponEventKind::ShellInserting, None), Some(Cue::Shell));
    // A reload: the rounds in, and what is to load (the room, at most the
    // reserve).
    assert_eq!(
        cue(&WeaponEventKind::ReloadStarted, Some((0, 60, 180))),
        Some(Cue::Reload { loaded: 0, needed: 60 })
    );
    assert_eq!(
        cue(&WeaponEventKind::ReloadStarted, Some((10, 12, 1))),
        Some(Cue::Reload { loaded: 10, needed: 1 })
    );
    for k in [
        WeaponEventKind::DryFire,
        WeaponEventKind::Reloaded,
        a("chamber"),
        a("shell_in"),
        a("zoom_in"),
        a("zoom_out"),
    ] {
        assert_eq!(cue(&k, None), None, "{k:?}");
    }
}

#[test]
fn playback_steps_as_the_spec() {
    // Play once, 3 frames: 1 running, 2 ends next tick, then restarts.
    assert_eq!(advance(0, 3, 0), (1, Step::Running));
    assert_eq!(advance(1, 3, 0), (2, Step::EndsNext));
    assert_eq!(advance(2, 3, 0), (0, Step::Restarted));
    // A loop frame at or past the end holds the last frame (overheated 50/58).
    assert_eq!(advance(49, 50, 58), (49, Step::Looped));
    assert_eq!(advance(4, 5, 2), (2, Step::Looped));
}

#[test]
fn each_state_lasts_its_frame_count() {
    // For each gun: its document's draw, reload and melee (the weapons'
    // own timers) are the view's ready, reload and melee lengths.
    for id in GUNS {
        let a = gun(id).animations;
        let mut slots = vec![
            (S::Idle, 60, 0),
            (S::Ready, a.ready as usize, 0),
            (S::Melee, a.melee as usize, 0),
        ];
        if let Some(r) = a.reload_empty {
            slots.push((S::ReloadEmpty, r as usize, 0));
        }
        let set = set_of(&slots);
        let mut v = HaloView::new(1);
        v.cue(&set, Cue::Ready, false);
        assert_eq!(v.state, ViewState::Ready);
        assert_eq!(lasts(&mut v, &set), a.ready as u32, "{id}: the draw");
        assert_eq!(v.state, ViewState::Idle);
        v.cue(&set, Cue::Melee, false);
        assert_eq!(lasts(&mut v, &set), a.melee as u32, "{id}: the melee");
        if let (Some(r), None) = (a.reload_empty, a.enter) {
            v.cue(&set, Cue::Reload { loaded: 0, needed: 5 }, false);
            assert_eq!(v.state, ViewState::ReloadEmpty);
            assert_eq!(lasts(&mut v, &set), r as u32, "{id}: the reload");
        }
    }
    // The pistol's reload with rounds in plays reload-full (65 frames),
    // whose timer is still reload-empty's (67).
    let set = set_of(&[(S::Idle, 60, 0), (S::ReloadEmpty, 67, 0), (S::ReloadFull, 65, 0)]);
    let mut v = HaloView::new(1);
    v.cue(&set, Cue::Reload { loaded: 3, needed: 9 }, false);
    assert_eq!(v.state, ViewState::ReloadFull);
    assert_eq!(lasts(&mut v, &set), 65);
}

#[test]
fn the_shotgun_loads_shell_by_shell() {
    let a = gun(halo_ce::game::weapons::SHOTGUN).animations;
    let (enter, shell) = (a.enter.unwrap() as usize, a.reload_empty.unwrap() as usize);
    let set = set_of(&[
        (S::Idle, 62, 0),
        (S::Enter, enter, 0),
        (S::ReloadEmpty, shell, 0),
        (S::ExitEmpty, 42, 0),
        (S::ExitFull, 24, 0),
    ]);
    // Three shells into an empty tube: enter, idle a tick, two shells,
    // exit-empty.
    let mut v = HaloView::new(1);
    v.cue(&set, Cue::Reload { loaded: 0, needed: 3 }, false);
    assert_eq!(v.state, ViewState::Enter);
    assert_eq!(lasts(&mut v, &set), enter as u32);
    assert_eq!(v.state, ViewState::Idle);
    for _ in 0..2 {
        v.cue(&set, Cue::Shell, false);
        assert_eq!(v.state, ViewState::ReloadEmpty);
        assert_eq!(lasts(&mut v, &set), shell as u32);
    }
    assert_eq!(v.state, ViewState::ExitEmpty);
    assert_eq!(lasts(&mut v, &set), 42);
    assert_eq!(v.state, ViewState::Idle);
    // One shell with some in: enter, then exit-full.
    v.cue(&set, Cue::Reload { loaded: 7, needed: 1 }, false);
    lasts(&mut v, &set);
    assert_eq!(v.state, ViewState::ExitFull);
}

#[test]
fn fire_enters_only_from_the_listed_states() {
    let set = set_of(&[
        (S::Idle, 60, 0),
        (S::Fire1, 15, 0),
        (S::Ready, 35, 0),
        (S::Melee, 45, 0),
        (S::Overlays, 9, 0),
    ]);
    let mut v = HaloView::new(1);
    v.cue(&set, Cue::Ready, false);
    v.cue(&set, Cue::Fire, false);
    assert_eq!(v.state, ViewState::Ready, "not from ready");
    v.cue(&set, Cue::Melee, false);
    v.cue(&set, Cue::Fire, false);
    assert_eq!(v.state, ViewState::Melee, "not from melee");
    let mut v = HaloView::new(1);
    v.cue(&set, Cue::Fire, false);
    assert_eq!(v.state, ViewState::Fire, "from idle");
    // A shot during fire restarts it.
    for _ in 0..5 {
        v.tick(&set, &Tick::default());
    }
    v.cue(&set, Cue::Fire, false);
    assert_eq!((v.state, v.frame), (ViewState::Fire, 0));
    // The assault rifle has no fire-1: no state, only the push back.
    let set = rifle();
    let mut v = HaloView::new(1);
    let rest = v.tick(&set, &Tick::default());
    v.tick(
        &set,
        &Tick {
            cues: vec![Cue::Fire],
            ..Default::default()
        },
    );
    assert_eq!(v.state, ViewState::Idle);
    let pushed = v.tick(&set, &Tick::default());
    assert!(v.push.0 > 0.0, "pushed back: {:?}", v.push);
    assert!(pushed[1].1.y > rest[1].1.y, "the overlays' frame 8 moves the gun");
    for _ in 0..60 {
        v.tick(&set, &Tick::default());
    }
    assert_eq!(v.push, (0.0, 0.0), "and back");
}

#[test]
fn moving_runs_its_overlay() {
    let set = rifle();
    let mut v = HaloView::new(1);
    let walk = Tick {
        throttle: Vec2::new(1.0, 0.0),
        ..Default::default()
    };
    v.tick(&set, &walk);
    assert_eq!(v.moving, Some(0));
    for _ in 0..30 {
        v.tick(&set, &walk);
    }
    assert_eq!(v.moving, Some(30 % 26), "loops");
    v.tick(&set, &Tick { airborne: true, ..walk });
    assert_eq!(v.moving, None, "not in the air");
    let (frames, _, _) = numbers(&set, sequence(&set, S::Moving).unwrap());
    assert_eq!(frames, 26);
}

/// A stand-in view model for the assault rifle (`rifle`).
fn stand_in(sim: &mut Sim) {
    let model = MapViewModel {
        key: ASSAULT_RIFLE.into(),
        model: Default::default(),
        bones: vec![
            chimera_world::map::MapBone {
                name: "a".into(),
                parent: None,
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
            },
            chimera_world::map::MapBone {
                name: "b".into(),
                parent: Some(0),
                position: Vec3::X,
                rotation: Quat::IDENTITY,
            },
        ],
        root: Transform::IDENTITY,
        animations: Some(Arc::new(rifle())),
        right_handed: true,
        allow_flipping: false,
        attachments: Vec::new(),
        light_origin: Vec3::ZERO,
    };
    let pistol = MapViewModel {
        key: PISTOL.into(),
        ..model.clone()
    };
    sim.app.world_mut().insert_resource(ViewModels::of([model, pistol]));
}

fn state(sim: &Sim, p: Entity) -> ViewState {
    sim.app.world().get::<HaloViewModel>(p).unwrap().view.state
}

#[test]
fn the_assault_rifle_in_hand() {
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    stand_in(&mut sim);
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.ticks(2);
    let g = give(sim.app.world_mut(), p, ASSAULT_RIFLE).unwrap();
    sim.ticks(1);
    // The draw: ready for its 29 ticks (the one just run included).
    assert_eq!(state(&sim, p), ViewState::Ready);
    let view = sim.app.world().get::<chimera_world::map::ViewAnimator>(p).unwrap();
    assert_eq!(view.key.as_deref(), Some(ASSAULT_RIFLE));
    assert!(
        view.animator
            .as_ref()
            .is_some_and(|a| a.fixed.is_some() && a.main == Some(0)),
        "posed by Halo's view"
    );
    sim.ticks(28);
    assert_eq!(state(&sim, p), ViewState::Ready, "its last frame on its 29th tick");
    sim.ticks(1);
    assert_eq!(state(&sim, p), ViewState::Idle);
    // A shot: no fire state (no fire-1), the push back.
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    let v = &sim.app.world().get::<HaloViewModel>(p).unwrap().view;
    assert_eq!(v.state, ViewState::Idle);
    assert!(v.push.1 > 0.0 || v.push.0 > 0.0, "pushed back: {:?}", v.push);
    // A reload with rounds in: reload-full for its 87 ticks.
    sim.ticks(10);
    sim.intent(p).reload = true;
    sim.ticks(1);
    sim.intent(p).reload = false;
    assert_eq!(state(&sim, p), ViewState::ReloadFull);
    // Every frame of the 87 shown, one a Halo tick; the rounds go in on
    // its last ticks (the weapon's timer, 87 from the same tick), the view
    // idles as the weapon can fire again.
    let mut frames = vec![0];
    let mut rounds_in = None;
    for t in 1..100 {
        sim.ticks(1);
        let v = &sim.app.world().get::<HaloViewModel>(p).unwrap().view;
        if v.state != ViewState::ReloadFull {
            break;
        }
        frames.push(v.frame);
        if rounds_in.is_none() && sim.app.world().get::<Magazine>(g).unwrap().clip == 60 {
            rounds_in = Some(t);
        }
    }
    frames.dedup();
    assert_eq!(frames, (0..87).collect::<Vec<_>>(), "the reload's 87 frames");
    let rounds_in = rounds_in.expect("the rounds went in during the reload");
    assert!(
        rounds_in + 3 >= frames.len(),
        "the rounds in on its last ticks ({rounds_in})"
    );
    assert_eq!(state(&sim, p), ViewState::Idle, "over");
    assert_eq!(sim.app.world().get::<Magazine>(g).unwrap().clip, 60);
}

#[test]
fn the_zoomed_pistol_hides_its_view() {
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    stand_in(&mut sim);
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.ticks(2);
    give(sim.app.world_mut(), p, PISTOL).unwrap();
    sim.ticks(60);
    assert!(sim.app.world().get::<HaloViewModel>(p).is_some());
    assert!(sim.app.world().get::<ViewModelHidden>(p).is_none());
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.ticks(1);
    assert!(
        sim.app.world().get::<chimera_sim::weapon::Zoomed>(p).is_some(),
        "zoomed"
    );
    assert!(
        sim.app.world().get::<ViewModelHidden>(p).is_some(),
        "hidden while zoomed"
    );
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.ticks(1);
    assert!(sim.app.world().get::<ViewModelHidden>(p).is_none(), "shown again");
}
