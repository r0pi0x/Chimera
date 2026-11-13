//! `points = "scored"` (`chimera_sim::minigame::spawn_score`) on the
//! greybox: a match's start places everyone one at a time, so no one
//! starts in another's view or near one; the same seed places the same;
//! respawns keep out of the enemies' view; a spawn's cost stays under its
//! budget of instructions.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::core::{Damage, DamageKind, Dead, Hitgroup, Intent, SOLID_LAYERS};
use chimera_sim::{
    minigame::{self, InMinigame, MinigameAppExt, MinigamePlugin, spawns::SpawnSeed},
    mode::{Match, Phase},
    movement::placeholder,
    testkit,
};
use chimera_world::{greybox, harness::Sim};

const DEF: &str = r#"
id = "kit:scored"
name = "Kit scored"
players = { min = 2, max = 16 }
maps = ["chimera:greybox"]
teams = "free_for_all"
time_limit = "60m"

[spawn]
respawn = { delay = "1s", protection = "0.5s" }
points = "scored"

[rules]
module = "chimera:ladder"
ladder = ["kit:pistol", "kit:rifle", "kit:shotgun"]
demote_on = "melee_death"
"#;

/// The scoring's "in view": the field of view (degrees) and the near
/// distance (m), as chimera/sim/data/spawn_scoring.toml has them.
const FOV: f32 = 110.0;
const NEAR: f32 = 10.0;

fn started(seed: u32, players: usize) -> (Sim, Vec<Entity>) {
    let mut sim = Sim::with(chimera_sim::SimBasePlugins, |app| {
        app.add_plugins((greybox::GreyboxMapPlugin, MinigamePlugin));
        testkit::add_weapons(app);
        app.add_minigames(&[("kit_scored.toml", DEF)])
            .expect("the definition reads");
        app.insert_resource(SpawnSeed(seed));
    });
    sim.ticks(5);
    minigame::start(sim.app.world_mut(), "kit:scored").expect("it starts");
    let all: Vec<Entity> = (0..players)
        .map(|i| sim.spawn_character(greybox::SPAWNS[i % 2], placeholder::ID))
        .collect();
    sim.seconds(1.0);
    (sim, all)
}

/// The match from the start: everyone placed in one tick.
fn restart(sim: &mut Sim) {
    minigame::restart(sim.app.world_mut());
    for _ in 0..10 {
        sim.ticks(1);
        if sim.app.world().resource::<Match>().phase != Phase::Waiting {
            return;
        }
    }
    panic!("the match didn't start");
}

/// Of `who`, those in a living other's view (a clear line from its eye to
/// the head or centre, inside its field of view) and those near one.
fn in_view_and_near(sim: &mut Sim, who: &[Entity]) -> (Vec<Entity>, Vec<Entity>) {
    let w = sim.app.world_mut();
    let all: Vec<(Entity, Vec3, Vec3)> = w
        .query_filtered::<(Entity, &Transform, &Intent), (With<InMinigame>, Without<Dead>)>()
        .iter(w)
        .map(|(e, t, i)| {
            (
                e,
                t.translation,
                Quat::from_euler(EulerRot::YXZ, i.yaw, i.pitch, 0.0) * Vec3::NEG_Z,
            )
        })
        .collect();
    let characters: Vec<Entity> = all.iter().map(|a| a.0).collect();
    let mut state = SystemState::<SpatialQuery>::new(w);
    let q = state.get(w).unwrap();
    let filter = SpatialQueryFilter::from_mask(SOLID_LAYERS).with_excluded_entities(characters);
    let clear = |from: Vec3, to: Vec3| {
        let d = to - from;
        Dir3::new(d).map_or(true, |dir| {
            q.cast_ray(from, dir, (d.length() - 0.05).max(0.0), true, &filter)
                .is_none()
        })
    };
    let (mut seen, mut near) = (Vec::new(), Vec::new());
    for e in who {
        let Some(&(_, at, _)) = all.iter().find(|a| a.0 == *e) else {
            continue;
        };
        let others = all.iter().filter(|a| a.0 != *e);
        if others.clone().any(|&(_, c, look)| {
            let eye = c + Vec3::Y * 0.7;
            let angle = look
                .dot((at - eye).normalize_or_zero())
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees();
            angle <= FOV / 2.0 && (clear(eye, at + Vec3::Y * 0.7) || clear(eye, at))
        }) {
            seen.push(*e);
        }
        if others.clone().any(|a| a.1.distance(at) < NEAR) {
            near.push(*e);
        }
    }
    (seen, near)
}

fn positions(sim: &Sim, who: &[Entity]) -> Vec<Vec3> {
    who.iter().map(|e| sim.position(*e)).collect()
}

#[test]
fn a_match_start_places_everyone_out_of_each_others_view() {
    let (mut sim, all) = started(1, 6);
    let mut total = (0, 0);
    for k in 0..5 {
        // Another seed each time (a restart begins the draws again).
        sim.app.world_mut().insert_resource(SpawnSeed(10 + k));
        restart(&mut sim);
        let (seen, near) = in_view_and_near(&mut sim, &all);
        total = (total.0 + seen.len(), total.1 + near.len());
        sim.seconds(2.0);
    }
    // The greybox is one open yard with a few crates: six players can't
    // always all hide from each other.
    println!("five starts of six players: {} in view, {} near", total.0, total.1);
    assert!(
        total.0 <= 3 && total.1 == 0,
        "(in view, near) of 30 placements: {total:?}"
    );
}

#[test]
fn the_same_seed_places_the_same() {
    let run = |seed: u32| {
        let (mut sim, all) = started(seed, 5);
        restart(&mut sim);
        positions(&sim, &all)
    };
    let (one, again, other) = (run(3), run(3), run(4));
    assert_eq!(one, again, "same seed");
    assert_ne!(one, other, "another seed");
}

#[test]
fn respawns_keep_out_of_the_enemies_view() {
    let (mut sim, all) = started(2, 4);
    let (mut bad, mut repeats) = (0, 0);
    for k in 0..30 {
        let p = all[k % all.len()];
        let at = sim.position(p);
        sim.app.world_mut().write_message(Damage {
            target: p,
            attacker: None,
            amount: 500.0,
            point: at,
            dir: Vec3::NEG_Z,
            hitgroup: Hitgroup::Chest,
            kind: DamageKind::Bullet,
            weapon: None,
            force: Vec3::ZERO,
        });
        sim.ticks(2);
        assert!(sim.app.world().get::<Dead>(p).is_some(), "dead");
        while sim.app.world().get::<Dead>(p).is_some() {
            sim.ticks(1);
        }
        let (seen, near) = in_view_and_near(&mut sim, &[p]);
        bad += seen.len() + near.len();
        repeats += (sim.position(p).distance(at) < 3.0) as u32;
    }
    println!("30 respawns among four: {bad} in view or near, {repeats} where they died");
    assert!(bad <= 3, "{bad} of 30 spawns in view or near");
    assert!(repeats <= 4, "{repeats} of 30 back where they died");
}

#[test]
fn a_scored_spawn_stays_in_its_budget() {
    let (mut sim, all) = started(5, 8);
    restart(&mut sim);
    let n = 40u64;
    let Some(before) = chimera_core::metrics::work::now() else {
        eprintln!(
            "skipping: no hardware counters ({})",
            chimera_core::metrics::hw_status()
        );
        return;
    };
    for k in 0..n as usize {
        let e = all[k % all.len()];
        let w = sim.app.world_mut();
        let f = minigame::def::Facing::default();
        assert!(minigame::spawn_score::pick(w, e, f).is_some());
    }
    let each = chimera_core::metrics::work::now()
        .unwrap_or(before)
        .saturating_sub(before)
        / n;
    println!("spawn_score::pick: {each} instructions per spawn among 8 players");
    assert!(each < 5_000_000, "{each} instructions per spawn");
}
