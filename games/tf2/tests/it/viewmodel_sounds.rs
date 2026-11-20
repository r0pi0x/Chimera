//! specs/tf2/viewmodel_sounds.md's test cases 1, 3 (cut by a switch), 4
//! on the greybox: the
//! shooter hears his view model's animation sound events (event 5004) as
//! the animation passes them, nobody else hears those; everyone else
//! hears the gun's world reload sound (`[sounds] world_reload`) at the
//! reload's start, per shell for a one-at-a-time reload, and the shooter
//! never. The Scout's Pistol on a made-up view model whose `p_reload`
//! (37 frames at 30 fps, 1.2 s) carries the real one's events (cycles
//! 0.028 and 0.5; `heavy::guns` checks them against the install).

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::core::LocalPlayer;
use chimera_sim::weapon::{Inventory, Magazine, give};
use chimera_world::{
    greybox,
    harness::Sim,
    map::{
        MapBone, MapModel, MapViewModel, OthersSound, PlaySound, ViewModels,
        anim::{AnimEvent, AnimSet, Animation, Sequence},
    },
};
use tf2::game::weapons::{BAT, PISTOL_SCOUT, SHOTGUN};

use super::weapons::{holding, sim};

const RELOAD_SCOUT: &str = "tf2:Weapon_Pistol.ReloadScout";
const CLIP_IN: &str = "tf2:Weapon_Pistol.ClipIn";
const WORLD_RELOAD: &str = "tf2:Weapon_Pistol.WorldReload";

/// A one-bone view model of `key` whose sequences are `(name, activity,
/// frames at 30 fps, sound events (cycle, entry))`.
fn view_model(key: &str, sequences: &[(&str, &str, usize, &[(f32, &str)])]) -> MapViewModel {
    let mut set = AnimSet { defaults: vec![(Quat::IDENTITY, Vec3::ZERO)], parents: vec![None], ..default() };
    for (name, activity, frames, events) in sequences {
        set.animations.push(Animation { name: name.to_string(), fps: 30.0, frames: *frames, ..default() });
        set.sequences.push(Sequence {
            name: name.to_string(),
            activity: activity.to_string(),
            looping: name.contains("idle"),
            grid: (1, 1),
            anims: vec![set.animations.len() - 1],
            fade_out: 0.2,
            events: events
                .iter()
                .map(|(cycle, entry)| AnimEvent { cycle: *cycle, event: 5004, name: String::new(), options: entry.to_string() })
                .collect(),
            ..default()
        });
    }
    MapViewModel {
        key: key.into(),
        model: MapModel::default(),
        bones: vec![MapBone { name: "root".into(), parent: None, position: Vec3::ZERO, rotation: Quat::IDENTITY }],
        root: Transform::IDENTITY,
        animations: Some(Arc::new(set)),
        right_handed: true,
        allow_flipping: false,
        attachments: Vec::new(),
        light_origin: Vec3::ZERO,
    }
}

/// The Scout's Pistol's view model (viewmodel_sounds.md 3).
fn scout_pistol() -> MapViewModel {
    view_model(
        PISTOL_SCOUT,
        &[
            ("p_draw", "ACT_SECONDARY_VM_DRAW", 16, &[]),
            ("p_idle", "ACT_SECONDARY_VM_IDLE", 31, &[]),
            ("p_fire", "ACT_SECONDARY_VM_PRIMARYATTACK", 10, &[]),
            ("p_reload", "ACT_SECONDARY_VM_RELOAD", 37, &[(0.028, "Weapon_Pistol.ReloadScout"), (0.5, "Weapon_Pistol.ClipIn")]),
        ],
    )
}

/// Per tick from the reload's start: the sounds this machine played
/// (entry) and the world reload sounds written (`OthersSound`: by, entry).
struct Heard {
    played: Vec<(usize, String)>,
    others: Vec<(usize, Entity, String)>,
}

/// `p` reloads its gun `w` from `clip` rounds (reload pressed on tick
/// 0) for `ticks`; `fire(sim, t)` holds attack on tick t (and may do
/// more).
fn reload(sim: &mut Sim, p: Entity, w: Entity, clip: u32, ticks: usize, mut fire: impl FnMut(&mut Sim, usize) -> bool) -> Heard {
    let world = sim.app.world();
    let mut play = world.resource::<Messages<PlaySound>>().get_cursor_current();
    let mut others = world.resource::<Messages<OthersSound>>().get_cursor_current();
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = clip;
    let mut heard = Heard { played: Vec::new(), others: Vec::new() };
    for t in 0..ticks {
        let held = fire(sim, t);
        sim.intent(p).fire = held;
        sim.intent(p).reload = t == 0;
        sim.ticks(1);
        let world = sim.app.world();
        heard.played.extend(play.read(world.resource::<Messages<PlaySound>>()).map(|s| (t, s.entry.clone())));
        heard.others.extend(others.read(world.resource::<Messages<OthersSound>>()).map(|s| (t, s.by, s.sound.entry.clone())));
    }
    heard
}

fn ticks_of(list: &[(usize, String)], entry: &str) -> Vec<usize> {
    list.iter().filter(|(_, e)| e == entry).map(|(t, _)| *t).collect()
}

/// A Scout with the Pistol drawn on its made-up view model, this
/// machine's player when `local`.
fn scout(local: bool) -> (Sim, Entity, Entity) {
    let mut sim = sim();
    sim.app.world_mut().insert_resource(ViewModels::of([scout_pistol()]));
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], PISTOL_SCOUT);
    if local {
        sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    }
    sim.ticks(5);
    (sim, p, w)
}

/// 1: the owner hears `ReloadScout` at 0.033 s (tick 2-3) and `ClipIn` at
/// 0.600 s (tick 39-40, within a frame), never the world reload; the
/// world reload goes to everyone else at the reload's start (tick 0).
#[test]
fn the_scout_hears_his_view_models_reload_others_the_world_reload_1() {
    let (mut sim, p, w) = scout(true);
    let h = reload(&mut sim, p, w, 3, 80, |_, _| false);
    let scout = ticks_of(&h.played, RELOAD_SCOUT);
    assert!(scout.len() == 1 && (2..=3).contains(&scout[0]), "ReloadScout at {scout:?}");
    let clip = ticks_of(&h.played, CLIP_IN);
    assert!(clip.len() == 1 && (39..=40).contains(&clip[0]), "ClipIn at {clip:?}");
    assert!(ticks_of(&h.played, WORLD_RELOAD).is_empty(), "the shooter never hears the world reload");
    assert_eq!(h.others, vec![(0, p, WORLD_RELOAD.to_string())], "the world reload at the start, for the others");
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, 12, "full");
}

/// 3 (cut by a switch: our whole-clip reload isn't cut by attack yet,
/// weapons_more.md Open questions 6): the reload's sequence replaced at
/// tick 20, the owner heard `ReloadScout` only.
#[test]
fn a_reload_cut_short_skips_the_later_events_3() {
    let (mut sim, p, w) = scout(true);
    let bat = give(sim.app.world_mut(), p, BAT).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w), "the Pistol in hand");
    let h = reload(&mut sim, p, w, 3, 80, |s, t| {
        if t == 20 {
            s.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(bat);
        }
        false
    });
    assert_eq!(ticks_of(&h.played, RELOAD_SCOUT).len(), 1);
    assert!(ticks_of(&h.played, CLIP_IN).is_empty(), "{:?}", h.played);
}

/// Another player's view-model events are nobody's here; his world
/// reload is played here (he isn't this machine's player).
#[test]
fn a_bots_reload_is_heard_as_its_world_reload() {
    let (mut sim, p, w) = scout(false);
    let h = reload(&mut sim, p, w, 3, 80, |_, _| false);
    assert_eq!(ticks_of(&h.played, WORLD_RELOAD), [0]);
    assert!(ticks_of(&h.played, RELOAD_SCOUT).is_empty() && ticks_of(&h.played, CLIP_IN).is_empty(), "{:?}", h.played);
}

/// 4: a one-at-a-time reload's world reload once per shell, at each
/// shell's insert start (the Shotgun, 4 of 6: two).
#[test]
fn a_shell_reloads_world_reload_per_shell_4() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], SHOTGUN);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    let h = reload(&mut sim, p, w, 4, 150, |_, _| false);
    let world = "tf2:Weapon_Shotgun.WorldReload";
    let shells: Vec<usize> = h.others.iter().filter(|(_, by, e)| *by == p && e == world).map(|(t, ..)| *t).collect();
    assert_eq!(shells.len(), 2, "one per shell: {:?}", h.others);
    assert!(shells[0] > 0, "at the first shell's insert, not the start sequence: {shells:?}");
    assert!(ticks_of(&h.played, world).is_empty(), "never for the shooter");
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, 6);
}

