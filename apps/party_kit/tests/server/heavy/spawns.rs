//! The gun game's `points = "random_hidden"` on the real maps (needs both
//! installs in chimera.local.toml, else passes with a note): de_dust2 (its
//! own navigation mesh) and Combat Arms' warehouse (a mesh generated from
//! its collision). Fifty respawns of one soldier with another standing
//! somewhere: spread over many areas rather than the two team spawns, out
//! of the other's sight, at the least distance, on spots a soldier can
//! stand on and walk from. Prints the figures (`--nocapture`). And every
//! map the party apps rotate to (CS:S's de_ and cs_ maps, Combat Arms'
//! pool): every spot the picker may choose is inside the map, also with
//! the bots' generated mesh in place (de_aztec's spawn outside the map).

use std::collections::BTreeSet;

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::{
    core::{Damage, DamageKind, Dead, Hitgroup, Intent, SOLID_LAYERS, SpawnPoint},
    mount::config::LocalConfig,
};
use chimera_sim::minigame::spawns::SpawnSpots;
use chimera_world::harness::Sim;

use crate::support::*;

fn installs() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    let css = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config.game_path(combat_arms::game::GAME).filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    css.is_some() && ca.is_some()
}

fn die(sim: &mut Sim, p: Entity) {
    let at = sim.position(p);
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: None,
        amount: 5000.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(2);
    assert!(sim.app.world().get::<Dead>(p).is_some(), "dead");
}

fn blocked(sim: &mut Sim, from: Vec3, to: Vec3) -> bool {
    let w = sim.app.world_mut();
    let characters: Vec<Entity> = w.query_filtered::<Entity, With<Intent>>().iter(w).collect();
    let mut state = SystemState::<SpatialQuery>::new(w);
    let q = state.get(w).unwrap();
    let filter = SpatialQueryFilter::from_mask(SOLID_LAYERS).with_excluded_entities(characters);
    let d = to - from;
    q.cast_ray(from, Dir3::new(d).unwrap(), d.length() - 0.05, true, &filter).is_some()
}

fn cells(points: impl Iterator<Item = Vec3>) -> BTreeSet<(i32, i32)> {
    points.map(|p| ((p.x / 10.0).floor() as i32, (p.z / 10.0).floor() as i32)).collect()
}

fn play(map: &str, min_cells: usize, max_near_points: usize) {
    let Some(mut sim) = server(&["+minigame", "originals:gun_game_dust2", "+map", map, "+maxplayers", "4"]) else {
        return;
    };
    let points: Vec<Vec3> = {
        let w = sim.app.world_mut();
        w.query_filtered::<&Transform, With<SpawnPoint>>().iter(w).map(|t| t.translation).collect()
    };
    let a = soldier(&mut sim, points[0]);
    let b = soldier(&mut sim, points[1]);
    sim.seconds(2.0);
    let mut at = Vec::new();
    let (mut hidden, mut far, mut moved) = (0, 0, 0);
    for i in 0..50 {
        // The enemy moves about: another of the map's spawn points every five.
        if i % 5 == 0 {
            place(&mut sim, a, points[(i / 5 * 7) % points.len()]);
            sim.seconds(0.3);
        }
        die(&mut sim, b);
        sim.seconds(4.0);
        assert!(sim.app.world().get::<Dead>(b).is_none(), "back");
        let p = sim.position(b);
        let (ea, pa) = (sim.position(a) + Vec3::Y * 0.7, sim.position(a));
        if blocked(&mut sim, ea, p + Vec3::Y * 0.7) && blocked(&mut sim, ea, p - Vec3::Y * 0.8) {
            hidden += 1;
        }
        if p.distance(pa) >= 20.0 {
            far += 1;
        }
        // Stands where it came, and walks away (some direction a metre).
        let still = sim.position(b);
        let mut best = 0.0f32;
        for yaw in [0.0f32, 1.57, 3.14, 4.71] {
            place(&mut sim, b, still);
            sim.intent(b).yaw = yaw;
            sim.intent(b).move_axis = Vec2::new(0.0, 1.0);
            sim.seconds(0.5);
            best = best.max(sim.position(b).xz().distance(still.xz()));
        }
        sim.intent(b).move_axis = Vec2::ZERO;
        if best > 1.0 {
            moved += 1;
        }
        place(&mut sim, b, still);
        at.push(p);
    }
    let spots = sim.app.world().resource::<SpawnSpots>().clone();
    let spawn_cells = cells(points.iter().copied());
    let got = cells(at.iter().copied());
    let near_a_point = at.iter().filter(|p| points.iter().any(|s| s.xz().distance(p.xz()) < 3.0)).count();
    let (lo, hi) = spots.spots.iter().fold((Vec3::MAX, Vec3::MIN), |(l, h), s| (l.min(*s), h.max(*s)));
    let (plo, phi) = points.iter().fold((Vec3::MAX, Vec3::MIN), |(l, h), s| (l.min(*s), h.max(*s)));
    let nav_areas = sim.app.world().get_resource::<chimera_world::map::nav::NavMesh>().map_or(0, |n| n.areas.len());
    eprintln!("spots span {lo} .. {hi}; spawn points span {plo} .. {phi}; spots in {} cells; map nav areas {nav_areas}", cells(spots.spots.iter().copied()).len());
    eprintln!(
        "{map}: {} spots (nav {}), map spawn points {} in {} cells; 50 respawns in {} cells, {} near a map spawn \
         point; hidden {hidden}/50, 20 m away {far}/50, walked {moved}/50",
        spots.spots.len(),
        spots.from_nav,
        points.len(),
        spawn_cells.len(),
        got.len(),
        near_a_point
    );
    assert!(spots.from_nav && spots.spots.len() > 80);
    assert!(got.len() >= min_cells, "spread over {} cells", got.len());
    assert!(near_a_point <= max_near_points, "{near_a_point} at the team spawns");
    assert!(hidden >= 40, "{hidden}/50 hidden");
    assert!(moved >= 48, "{moved}/50 could walk");
}

#[test]
fn the_gun_game_spawns_spread_over_dust2() {
    if !installs() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    play("cs_source:de_dust2", 15, 10);
}

#[test]
fn the_gun_game_spawns_spread_over_the_warehouse() {
    if !installs() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    play("combat_arms:warehouse", 8, 50);
}

/// Every candidate spot of `map`, as `random_hidden` makes them, checked:
/// inside the map (`spawns::inside_map`) and, where the mesh is generated,
/// walked to (on the walking mesh from the spawn points), also with the
/// Combat Arms bots' mesh (jump-ups) loaded as a party with bots has it.
/// One line per map: (outside, off the walk) counts, and the old picker's.
fn sweep(map: &str) -> Option<String> {
    use chimera_sim::minigame::spawns;
    use chimera_world::map::{nav::NavMesh, nav_gen, vis::ActiveVisibility};
    let mut sim = server(&["+minigame", "originals:gun_game_dust2", "+map", map, "+maxplayers", "4"])?;
    sim.seconds(1.0);
    let w = sim.app.world_mut();
    let feet: Vec<Vec3> = w
        .query_filtered::<&Transform, With<SpawnPoint>>()
        .iter(w)
        .map(|t| t.translation - Vec3::Y * chimera_world::map::SPAWN_LIFT)
        .collect();
    let vis = w.get_resource::<ActiveVisibility>().map(|v| v.0.clone());
    let own = w.get_resource::<NavMesh>().filter(|n| !n.generated).is_some();
    let (walk, bots) = {
        let mut state = SystemState::<SpatialQuery>::new(w);
        let q = state.get(w).unwrap();
        (
            nav_gen::generate(&q, &|_| false, &feet, &spawns::walker()),
            nav_gen::generate(&q, &|_| false, &feet, &combat_arms::game::bots::brain::nav_params()),
        )
    };
    let walked = |p: &Vec3| walk.areas.iter().any(|a| a.overlaps(p.x, p.z) && (a.height_at(p.x, p.z) - p.y).abs() < 0.6);
    let count = |spots: &[Vec3]| {
        (
            spots.iter().filter(|s| !spawns::inside_map(vis.as_deref(), **s)).count(),
            spots.iter().filter(|s| !walked(s)).count(),
        )
    };
    let now = spawns::candidates(w);
    let (outside, off) = count(&now);
    let mut line = format!(
        "{map}: {} spots ({} mesh), {outside} outside the map, {off} off the walk",
        now.len(),
        if own { "own" } else { "walked" }
    );
    assert_eq!(outside, 0, "{line}");
    if !own {
        assert_eq!(off, 0, "{line}");
        // A party with bots: their mesh is loaded, and isn't spawned on.
        let old = {
            w.insert_resource(NavMesh { generated: false, ..bots.clone() });
            spawns::candidates(w)
        };
        w.insert_resource(bots);
        let with_bots = spawns::candidates(w);
        let (old_outside, old_off) = count(&old);
        let (b_outside, b_off) = count(&with_bots);
        line += &format!(
            "; on the bots' mesh {} spots, {old_outside} outside, {old_off} off the walk; with it loaded {b_outside}, {b_off}",
            old.len()
        );
        assert_eq!((b_outside, b_off), (0, 0), "{line}");
        w.remove_resource::<NavMesh>();
    }
    Some(line)
}

#[test]
fn every_pool_map_s_spawn_spots_are_inside_it() {
    if !installs() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    let Some(css) = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref()))
    else {
        return;
    };
    let mut maps: Vec<String> = std::fs::read_dir(css.join("cstrike/maps"))
        .map(|d| {
            d.filter_map(|e| e.ok()?.file_name().into_string().ok())
                .filter_map(|n| n.strip_suffix(".bsp").map(str::to_string))
                .filter(|n| n.starts_with("de_") || n.starts_with("cs_"))
                .map(|n| format!("cs_source:{n}"))
                .collect()
        })
        .unwrap_or_default();
    maps.sort();
    maps.extend(
        ["warehouse", "junk_flea", "darkhills", "bloodmoney"].map(|m| format!("combat_arms:{m}")),
    );
    let only = std::env::var("CHIMERA_SPAWN_SWEEP").ok();
    for map in maps.iter().filter(|m| only.as_ref().is_none_or(|o| m.contains(o.as_str()))) {
        match sweep(map) {
            Some(line) => eprintln!("{line}"),
            None => eprintln!("{map}: not loaded"),
        }
    }
}

/// de_dust2's spots off its 64-unit grid lines, where faces meet (the
/// 2026-10-30 "seam": a party spawn's eye 0.85 units from the edge at
/// y 2304 between a bump-lit and a flat-lit displacement, looking along
/// it, saw the edge as a vertical line through the frame's middle; 35
/// spots, area-centred, sat on that one line). Each spot coordinate within
/// a unit of a 64-unit line no more often than chance (1 in 32) allows,
/// with room, and no row of spots along y 2304.
#[test]
fn dust2s_spawn_eyes_keep_off_its_grid_lines() {
    use chimera_sim::minigame::spawns;
    if !installs() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    let Some(mut sim) = server(&["+minigame", "originals:gun_game_dust2", "+map", "cs_source:de_dust2", "+maxplayers", "4"])
    else {
        return;
    };
    sim.seconds(1.0);
    let spots = spawns::candidates(sim.app.world_mut());
    // Source units: x, and y = -z.
    let coords: Vec<f32> = spots.iter().flat_map(|s| [s.x / 0.0254, -s.z / 0.0254]).collect();
    let on = coords.iter().filter(|c| (**c - (**c / 64.0).round() * 64.0).abs() < 1.0).count();
    let chance = coords.len() as f32 / 32.0;
    let row = spots.iter().filter(|s| (-s.z / 0.0254 - 2304.0).abs() < 1.0).count();
    eprintln!("{} spots: {on} coordinates within a unit of a 64-unit line (chance {chance:.0}); {row} within a unit of y 2304", spots.len());
    assert!(spots.len() > 1000, "{} spots", spots.len());
    assert!((on as f32) < chance * 1.4, "{on} coordinates on 64-unit lines, chance {chance:.0}");
    assert!(row < 8, "{row} spots along y 2304");
}
