//! TF2's maps in the party (docs/plans/active/tf2.md, M1): `tf2:ctf_2fort`
//! loads through the party's map sources (`party_roster::GUESTS`) as the
//! dedicated server loads it, with the party's parts and the gun game
//! started; its spawn points are RED's and BLU's `info_player_teamspawn`s,
//! none of them a spot a standing player is stuck in; and a Combat Arms
//! player dropped at one lands on the floor and walks. Every map of TF2's
//! pool (M4: data/map_pool.toml) lets a player out of each spawn room
//! (`a_player_walks_out_of_every_spawn_room_of_the_tf2_pool`;
//! `CHIMERA_TF2_POOL=<text>`: only the pool's maps whose id has it).
//! A free for all spawns outside the spawn rooms on ctf_2fort and
//! cp_badlands (`a_free_for_all_spawns_outside_the_spawn_rooms`, TF2's
//! install alone: `$CHIMERA_TF2_ROOT` will do). Passes with a note without the TF2 and Combat Arms installs.

use bevy::prelude::*;
use chimera_core::mount::config::LocalConfig;
use chimera_world::{
    harness::Sim,
    map::{KitLoad, LoadContext, MapLoadOptions, MapSimPlugin, nav::NavMesh},
};
use combat_arms::harness::CaSim;
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin, spot_checks};

fn installed() -> bool {
    let ca = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path(combat_arms::game::GAME))
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    let tf2 = tf2::game::INFO.install().is_some_and(|p| chimera_core::mount::served::usable("tf2", p, "tf"));
    ca && tf2
}

#[test]
fn a_player_lands_and_walks_on_ctf_2fort() {
    if !installed() {
        eprintln!("skipping: needs the TF2 and Combat Arms installs");
        return;
    }
    let context = LoadContext::default();
    let options = MapLoadOptions { kit: KitLoad::Only(&[]), ..MapLoadOptions::lean().in_context(&context) };
    let data = party_roster::load_map_with("tf2:ctf_2fort", &options).expect("tf2:ctf_2fort loads");
    assert_eq!(data.name, "tf2:ctf_2fort");
    assert_eq!(data.spawns.len(), 30, "RED's and BLU's 15 each");
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(data), PartyPartsPlugin));
    chimera_sim::minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    sim.ticks(3);
    let w = sim.app.world_mut();
    let feet = spot_checks::spawn_feet(w);
    assert_eq!(feet.len(), 30, "every spawn point placed");
    let bots = spot_checks::bots_mesh(w, &feet);
    let r = spot_checks::report(w, &feet, &bots);
    eprintln!("tf2:ctf_2fort: {} spots, stuck {}", r.spots.len(), r.stuck.len());
    assert!(r.stuck.is_empty(), "spots a standing player is stuck in: {:?}", r.stuck);
    // A Combat Arms player at the first spawn: lands, then walks.
    let at = feet[0];
    let p = spawn_at(&mut sim, at + Vec3::Y * 0.3);
    sim.seconds(1.5);
    let low = sim.position(p).y + sim.state(p).hull_min.y;
    eprintln!("spawn {at}: feet at {low:.3}");
    assert!(low > at.y - 1.0 && low < at.y + 0.3, "spawn {at}: feet at {low}, not on the floor");
    let start = sim.position(p);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(0.5);
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.seconds(0.5);
    let moved = sim.position(p).xz().distance(start.xz());
    eprintln!("walked {moved:.2} m");
    assert!(moved > 0.5, "walked only {moved} m");
    assert!(sim.position(p).y > start.y - 1.0, "fell walking");
}

/// `CHIMERA_TF2_POOL_TRACE`: each walk's place every second.
fn trace() -> bool {
    std::env::var_os("CHIMERA_TF2_POOL_TRACE").is_some()
}

/// A Combat Arms player with its feet at `feet`: put back there once the
/// gun game has spawned it (at a spawn spot anywhere on the map, holding
/// its first rung's gun and with it that gun's game's movement), at rest.
fn spawn_at(sim: &mut Sim, feet: Vec3) -> Entity {
    let p = sim.spawn_character(feet, combat_arms::game::movement::ID);
    sim.ticks(2);
    let lift = -sim.state(p).hull_min.y + 0.02;
    let w = sim.app.world_mut();
    w.get_mut::<Transform>(p).expect("a character").translation = feet + Vec3::Y * lift;
    if let Some(mut v) = w.get_mut::<chimera_core::core::Velocity>(p) {
        v.0 = Vec3::ZERO;
    }
    p
}

/// TF2's maps in the party's pool, as the apps find them (`auto::report`
/// over the installed maps its data/map_pool.toml allows).
fn tf2_pool() -> Vec<String> {
    let mut sim = Sim::new(CaSim, PartyPartsPlugin);
    let w = sim.app.world_mut();
    chimera_sim::minigame::auto::find_installed_maps(w);
    chimera_sim::minigame::auto::report(w).into_iter().find(|c| c.game == tf2::game::GAME).map(|c| c.maps).unwrap_or_default()
}

/// Spawn points closer than this (m, level, within a storey) are one
/// spawn room's.
const ROOM_GAP: f32 = 12.0;
/// This far from every spawn point of its room (m, level), a player is out
/// of the room.
const OUT_M: f32 = 20.0;

/// The spawn points in groups, one per spawn room (single linkage by
/// `ROOM_GAP`).
fn rooms(feet: &[Vec3]) -> Vec<Vec<Vec3>> {
    let mut group: Vec<usize> = (0..feet.len()).collect();
    let root = |g: &Vec<usize>, mut i: usize| {
        while g[i] != i {
            i = g[i];
        }
        i
    };
    for i in 0..feet.len() {
        for j in i + 1..feet.len() {
            let (a, b) = (feet[i], feet[j]);
            if a.xz().distance(b.xz()) < ROOM_GAP && (a.y - b.y).abs() < 3.0 {
                let (ri, rj) = (root(&group, i), root(&group, j));
                group[ri.max(rj)] = ri.min(rj);
            }
        }
    }
    let mut out: std::collections::BTreeMap<usize, Vec<Vec3>> = Default::default();
    for (i, f) in feet.iter().enumerate() {
        out.entry(root(&group, i)).or_default().push(*f);
    }
    out.into_values().collect()
}

/// The length of the way from `at` through `route` (m).
fn way(at: Vec3, route: &[Vec3]) -> f32 {
    std::iter::once(at).chain(route.iter().copied()).collect::<Vec<_>>().windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// `nav`'s route from `from` to area `goal`'s centre, starting in the area
/// under `from` (as `spot_checks::region` finds it), else the nearest.
fn way_from(nav: &NavMesh, from: Vec3, goal: usize) -> Option<Vec<Vec3>> {
    let start = nav.area_at(from + Vec3::Y * 0.1).or_else(|| nav.nearest_area(from))?;
    let to = nav.areas[goal].center;
    let (areas, _) = nav.find_path(start, goal)?;
    Some(nav.points_along(&areas, from, to, |_, _, _| 0.0).into_iter().map(|p| p.0).collect())
}

/// A player from `from` walking the bots' mesh `nav` to `to`: steering at
/// the route's next point; where it stalls (a pillar's corner between two
/// points) it routes again from where it stands and steps aside, and
/// jumps. How far along it got (m: the way at the start less the way
/// left), the farthest it got from every point of `room` (m, level) and
/// whether it got there.
fn walk(sim: &mut Sim, nav: &NavMesh, from: Vec3, goal: usize, room: &[Vec3]) -> (f32, f32, bool) {
    let p = spawn_at(sim, from);
    sim.seconds(0.5);
    let Some(mut route) = way_from(nav, from, goal) else { return (0.0, 0.0, false) };
    let mut away = 0.0f32;
    let total = way(from, &route);
    let limit = (total / 3.0 + 15.0).min(120.0);
    let hz = sim.tick_hz();
    let (mut next, mut best, mut stalled, mut aside, mut side) = (0usize, f32::INFINITY, 0u32, 0u32, 1.0f32);
    let mut there = false;
    for t in 0..(limit as f64 * hz) as u32 {
        let at = sim.position(p);
        away = away.max(room.iter().map(|r| r.xz().distance(at.xz())).fold(f32::INFINITY, f32::min));
        // The point reached: beside it, within a storey (a drop's foot is
        // under the ledge it leaves).
        let feet = at.y + sim.state(p).hull_min.y;
        while next < route.len() && at.xz().distance(route[next].xz()) < 0.8 && (route[next].y - feet).abs() < 1.2 {
            next += 1;
            best = f32::INFINITY;
        }
        if next >= route.len() {
            there = true;
            break;
        }
        let gap = at.xz().distance(route[next].xz());
        if gap < best - 0.05 {
            best = gap;
            stalled = 0;
        } else {
            stalled += 1;
        }
        if stalled > hz as u32 {
            if let Some(r) = way_from(nav, at, goal) {
                route = r;
                next = 0;
            }
            best = f32::INFINITY;
            stalled = 0;
            aside = (0.4 * hz) as u32;
            side = -side;
        }
        if trace() && t % (hz as u32) == 0 {
            eprintln!("    t {:.0} s at {at:.2} next {:.2} ground {}", t as f64 / hz, route[next], sim.state(p).on_ground);
        }
        // Steer at the first point not straight over or under (a spiral
        // stair's crossings stack).
        let aim = route[next..].iter().find(|q| at.xz().distance(q.xz()) > 0.8).copied().unwrap_or(route[next]);
        let d = (aim - at).with_y(0.0);
        let mut i = sim.intent(p);
        i.yaw = (-d.x).atan2(-d.z);
        i.move_axis = if aside > 0 { Vec2::new(side, 0.5) } else { Vec2::Y };
        i.jump = stalled > (0.3 * hz) as u32 && t % 8 == 0;
        aside = aside.saturating_sub(1);
        sim.ticks(1);
    }
    let at = sim.position(p);
    let left = if there { 0.0 } else { way_from(nav, at, goal).map_or(total, |r| way(at, &r)) };
    sim.app.world_mut().despawn(p);
    sim.seconds(0.2);
    ((total - left).max(0.0), away, there)
}

/// Every TF2 map of the party's pool, loaded as the dedicated server loads
/// it with the party's parts and the gun game started: a free-for-all's
/// spawn rooms let everyone out. From each spawn room (the spawn points in
/// groups) a player walks the bots' generated mesh (TF2's `.nav` files
/// don't parse, M1) toward the map's middle (the region's area nearest the
/// spawn points' centre) and gets there or at least `OUT_M` from every
/// spawn point of its room: through the spawn room's doors (a trigger opens them
/// for anyone, `maps::free_for_all_doors`) and past its door brushes
/// (`func_respawnroomvisualizer`, no wall); and its room and the middle
/// are one region of the bots' mesh (walked to and back). The spots spread past the
/// rooms too (`stuck` none, at least 2000), and every spawn point is on
/// the main region.
#[test]
fn a_player_walks_out_of_every_spawn_room_of_the_tf2_pool() {
    if !installed() {
        eprintln!("skipping: needs the TF2 and Combat Arms installs");
        return;
    }
    let only = std::env::var("CHIMERA_TF2_POOL").ok();
    let pool: Vec<String> = tf2_pool().into_iter().filter(|m| only.as_ref().is_none_or(|o| m.contains(o.as_str()))).collect();
    eprintln!("TF2's pool: {pool:?}");
    assert!(only.is_some() || pool.iter().any(|m| m == "tf2:ctf_2fort"), "ctf_2fort in the pool: {pool:?}");
    let context = LoadContext::default();
    let mut problems = Vec::new();
    for id in &pool {
        let options = MapLoadOptions { kit: KitLoad::Only(&[]), ..MapLoadOptions::lean().in_context(&context) };
        let data = party_roster::load_map_with(id, &options).unwrap_or_else(|e| panic!("{id}: {e}"));
        let mut sim = Sim::new(CaSim, (MapSimPlugin::new(data), PartyPartsPlugin));
        chimera_sim::minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
        sim.ticks(3);
        let w = sim.app.world_mut();
        let feet = spot_checks::spawn_feet(w);
        let bots = spot_checks::bots_mesh(w, &feet);
        let r = spot_checks::report(w, &feet, &bots);
        let (inside, starts) = spot_checks::region(&bots, &feet);
        let off: Vec<Vec3> = feet.iter().zip(&starts).filter(|(_, s)| s.is_none_or(|s| !inside[s])).map(|(f, _)| *f).collect();
        eprintln!("{id}: {} spawn points, {} spots, {} stuck, {} points off the region", feet.len(), r.spots.len(), r.stuck.len(), off.len());
        if r.spots.len() < 2000 || !r.stuck.is_empty() || !off.is_empty() {
            problems.push(format!("{id}: {} spots, stuck {:?}, points off the region {off:?}", r.spots.len(), r.stuck.iter().map(|s| s.0).collect::<Vec<_>>()));
        }
        let centre = feet.iter().copied().sum::<Vec3>() / feet.len().max(1) as f32;
        let goal = bots
            .areas
            .iter()
            .enumerate()
            .filter(|(i, _)| inside[*i])
            .min_by(|a, b| a.1.center.xz().distance(centre.xz()).total_cmp(&b.1.center.xz().distance(centre.xz())))
            .map(|(i, _)| i)
            .expect("a main region");
        for room in rooms(&feet) {
            let from = room[0];
            let middle = bots.areas[goal].center;
            // The room's own region (walked to from its first point and
            // back) holds the middle: not a map in halves that never meet.
            let (alone, _) = spot_checks::region(&bots, &[from]);
            if !alone[goal] {
                problems.push(format!("{id}: the spawn room at {from:.1} and the middle {middle:.1} are apart on the bots' mesh"));
                continue;
            }
            if way_from(&bots, from, goal).is_none() {
                problems.push(format!("{id}: no way from the spawn room at {from} to the middle {middle}"));
                continue;
            }
            let (got, away, there) = walk(&mut sim, &bots, from, goal, &room);
            eprintln!(
                "  from {from:.1} ({} points): {got:.1} m along, {away:.1} m from the room's points, {}",
                room.len(),
                if there { "at the middle" } else { "not at the middle" }
            );
            if !there && away < OUT_M {
                problems.push(format!("{id}: from the spawn room at {from:.1} only {away:.1} m from its points (toward the middle {middle:.1})"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The gun game (free for all) on ctf_2fort and cp_badlands: no spawn spot
/// is in a spawn room (`func_respawnroom`, `chimera_world::map::SpawnRooms`),
/// the spots still spread over the map, and none is a spot a standing
/// player is stuck in.
#[test]
fn a_free_for_all_spawns_outside_the_spawn_rooms() {
    if !tf2::game::INFO.install().is_some_and(|p| chimera_core::mount::served::usable("tf2", p, "tf")) {
        eprintln!("skipping: needs the TF2 install");
        return;
    }
    for map in ["tf2:ctf_2fort", "tf2:cp_badlands"] {
        let context = LoadContext::default();
        let options = MapLoadOptions { kit: KitLoad::Only(&[]), ..MapLoadOptions::lean().in_context(&context) };
        let data = party_roster::load_map_with(map, &options).expect("the map loads");
        let mut sim = Sim::new(CaSim, (MapSimPlugin::new(data), PartyPartsPlugin));
        chimera_sim::minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
        sim.ticks(3);
        let w = sim.app.world_mut();
        let rooms = chimera_world::map::spawn_room_zones(w);
        let feet = spot_checks::spawn_feet(w);
        let in_room = |p: Vec3| [0.1, 1.0, 1.7].iter().any(|h| rooms.iter().any(|b| b.contains(p + Vec3::Y * *h)));
        let roomed = feet.iter().filter(|p| in_room(**p)).count();
        let spots = chimera_sim::minigame::spawns::candidates(w);
        let inside: Vec<Vec3> = spots.iter().copied().filter(|p| in_room(*p)).collect();
        let cells: std::collections::BTreeSet<(i32, i32)> =
            spots.iter().map(|p| ((p.x / 10.0).floor() as i32, (p.z / 10.0).floor() as i32)).collect();
        let stuck = spot_checks::stuck(w, &spots);
        eprintln!(
            "{map}: {} spawn-room volumes, {roomed} of {} spawn points in them; {} spots in {} 10 m cells, {} in a room, {} stuck",
            rooms.len(),
            feet.len(),
            spots.len(),
            cells.len(),
            inside.len(),
            stuck.len()
        );
        eprintln!("{map}: the first spots: {:?}", &spots[..spots.len().min(4)]);
        assert!(!rooms.is_empty(), "{map}: its spawn rooms' volumes");
        assert!(roomed * 2 > feet.len(), "{map}: most team spawns are in the rooms ({roomed} of {})", feet.len());
        assert!(inside.is_empty(), "{map}: spots in a spawn room: {inside:?}");
        assert!(spots.len() > 1000 && cells.len() > 60, "{map}: spread ({} spots, {} cells)", spots.len(), cells.len());
        assert!(stuck.is_empty(), "{map}: spots a standing player is stuck in: {stuck:?}");
    }
}
