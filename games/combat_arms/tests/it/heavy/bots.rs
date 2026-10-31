//! Combat Arms' bots on real maps from the user's install: on WAREHOUSE
//! (no AI data) the navigation areas are generated from its collision, and
//! four bots a team play Team Death Match for two minutes: they find and
//! kill each other, the kills are scored, the dead respawn, nothing
//! panics. On costarecon_cp they walk the map's own 86 AI volumes and
//! patrol its 8 patrol nodes. Pass with a note without `[games.combat_arms]
//! path` in chimera.local.toml.

use std::collections::HashSet;

use bevy::prelude::*;
use chimera_core::{core::Dead, mount::config::LocalConfig};
use chimera_world::{
    harness::Sim,
    map::{MapSimPlugin, nav::NavMesh},
};
use combat_arms::{
    game::{
        GAME, INFO,
        bots::{self, CaBot, brain, map_ai::MapAi},
        rules::CaMatch,
        world,
    },
    harness::{CaSim, with_mode},
};

fn mount() -> Option<chimera_core::mount::Mount> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some((INFO.open)(&install).unwrap())
}

#[test]
fn bots_fight_on_warehouse() {
    let Some(mount) = mount() else { return };
    let map = world::load_from(&mount, "warehouse").unwrap();
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    with_mode(sim.app.world_mut(), "TDM");
    for team in [1, 2, 1, 2, 1, 2, 1, 2] {
        bots::add_bot(sim.app.world_mut(), Some(team)).unwrap();
    }
    let secs = 120.0;
    let mut deaths = 0;
    let mut dead_now: HashSet<Entity> = HashSet::new();
    let mut respawned = 0;
    for _ in 0..(secs / combat_arms::game::TICK_INTERVAL) as usize {
        sim.ticks(1);
        let w = sim.app.world_mut();
        let dead: HashSet<Entity> = w.query_filtered::<Entity, (With<CaBot>, With<Dead>)>().iter(w).collect();
        deaths += dead.difference(&dead_now).count();
        respawned += dead_now.difference(&dead).count();
        dead_now = dead;
    }
    let areas = sim.app.world().get_resource::<NavMesh>().map_or(0, |m| m.areas.len());
    let m = sim.app.world().resource::<CaMatch>().clone();
    eprintln!("warehouse: {areas} areas, {deaths} deaths, {respawned} respawns, score {:?}", m.score);
    assert!(areas > 50, "a navigation mesh: {areas} areas");
    assert!(deaths >= 2, "bots killed each other: {deaths}");
    assert!(m.score.iter().sum::<i32>() >= 2, "kills scored: {:?}", m.score);
    assert!(respawned >= 1, "the dead came back");
}

/// costarecon_cp's AI data (bots.md 3, 4): its 86 `AIVolume`s become the
/// navigation areas (not generated ones), linked where their boxes touch,
/// with floors found under them; its 8 `AINodePatrol`s the patrol graph.
/// Three bots of one team patrol them for 40 s (they reach the map's
/// patrol nodes), then three of the other come in and they fight; all
/// the while they stay over its volumes.
#[test]
fn bots_use_costarecon_ai_volumes_and_patrol_nodes() {
    let Some(mount) = mount() else { return };
    let map = world::load_from(&mount, "costarecon_cp").unwrap();
    let ai = MapAi::read(&map.entities, map.entity_scale);
    assert_eq!((ai.volumes.len(), ai.patrol.len()), (86, 8), "the map's AI objects");
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    with_mode(sim.app.world_mut(), "TDM");
    // One team first: with no enemy they patrol.
    for _ in 0..3 {
        bots::add_bot(sim.app.world_mut(), Some(1)).unwrap();
    }
    sim.seconds(1.0);
    let nav = sim.app.world().resource::<brain::BotNav>().clone();
    let links: usize = nav.mesh.areas.iter().map(|a| a.links.len()).sum();
    let floors = nav
        .mesh
        .areas
        .iter()
        .zip(&ai.volumes)
        .filter(|(a, v)| (a.heights[0] - v.min.y).abs() > 1e-4)
        .count();
    assert!(floors >= 80, "floors found under the boxes: {floors} of 86");
    eprintln!(
        "costarecon_cp: {} areas, {} links, {floors} floors found under the boxes, {} patrol nodes, {} patrol links",
        nav.mesh.areas.len(),
        links,
        nav.patrol.points.len(),
        nav.patrol.links.iter().map(Vec::len).sum::<usize>()
    );
    assert_eq!(nav.mesh.areas.len(), 86, "the map's volumes, not generated areas");
    assert!(links >= 2 * 80, "volumes linked: {links}");
    assert_eq!(nav.patrol.points.len(), 8, "the map's patrol nodes");
    assert!(nav.patrol.links.iter().all(|l| !l.is_empty()), "every node linked: {:?}", nav.patrol.links);
    // costarecon_cp is a co-op map (no Team Death Match start points:
    // everyone comes in at its first one), so the bots start spread over
    // its patrol nodes.
    let spread = |sim: &mut Sim, from: usize| {
        let w = sim.app.world_mut();
        let bots: Vec<Entity> = w.query_filtered::<Entity, With<CaBot>>().iter(w).collect();
        for (b, p) in bots.iter().zip(nav.patrol.points.iter().cycle().skip(from)) {
            w.get_mut::<Transform>(*b).unwrap().translation = *p + Vec3::Y * 1.0;
        }
    };
    spread(&mut sim, 0);
    let mut over = 0usize;
    let mut samples = 0usize;
    let mut sample = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        let feet: Vec<Vec3> = w
            .query_filtered::<&Transform, (With<CaBot>, Without<Dead>)>()
            .iter(w)
            .map(|t| t.translation - Vec3::Y * 0.9)
            .collect();
        for f in feet {
            samples += 1;
            over += nav.mesh.areas.iter().any(|a| a.overlaps(f.x, f.z)) as usize;
        }
    };
    for _ in 0..(40.0 / combat_arms::game::TICK_INTERVAL / 20.0) as usize {
        sim.ticks(20);
        sample(&mut sim);
    }
    let reached = sim.app.world().resource::<brain::PatrolClock>().reached.len();
    // Then the other team: they meet and fight.
    for _ in 0..3 {
        bots::add_bot(sim.app.world_mut(), Some(2)).unwrap();
    }
    sim.seconds(1.0);
    spread(&mut sim, 4);
    let mut deaths = 0;
    let mut dead_now: HashSet<Entity> = HashSet::new();
    for k in 0..(40.0 / combat_arms::game::TICK_INTERVAL) as usize {
        sim.ticks(1);
        let w = sim.app.world_mut();
        let dead: HashSet<Entity> = w.query_filtered::<Entity, (With<CaBot>, With<Dead>)>().iter(w).collect();
        deaths += dead.difference(&dead_now).count();
        dead_now = dead;
        if k % 20 == 0 {
            sample(&mut sim);
        }
    }
    eprintln!("costarecon_cp: {reached} patrol nodes reached, {over} of {samples} positions over a volume, {deaths} deaths");
    assert!(reached >= 3, "the map's patrol nodes reached: {reached}");
    assert!(over * 10 >= samples * 8, "bots over the volumes most of the time: {over} of {samples}");
    assert!(deaths >= 1, "they met and fought");
}
