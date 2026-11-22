//! Combat Arms' bots on Halo's maps (docs/plans/active/halo-ce.md, M4):
//! their generated navigation is held to the drawn level
//! (`nav_gen::NavWithin`, from Halo's structure BSP's world bounds,
//! `halo_ce::game::maps::collision::nav_within`) and links Halo's
//! teleporters (`nav_gen::link_map`, `Via::Teleport`).
//!
//! - Blood Gulch: no area of the bots' mesh lies outside the drawn level's
//!   box (structure_bsp.md's world bounds for bloodgulch, numbers of the
//!   spec) nor above its highest drawn point, and the held flood, growing
//!   to the box's footprint (`NavGenParams::cell_limit`), no longer stops
//!   at its cell limit: it is every floor the walker reaches. (Measured:
//!   the bots' walker never left the drawn level there; the flood stopped
//!   at 75,000 of its 78,000 m2.) The mesh unheld and held (area, cells,
//!   whether capped, time) is printed with `--nocapture`.
//! - Chiron TL-34 (`putput`), whose rooms join only by teleporters: a bot
//!   spawned in one room patrols into another through a teleporter (a
//!   jump from a pad's source to its exit) within 90 s.
//! - Hang 'Em High: six bots still get about and fire in 40 s (as
//!   `bots_on_rungs` measures them on de_dust2).
//!
//! Each passes with a note without the disc image and the CS:S and
//! Combat Arms installs.

use std::{collections::HashMap, time::Instant};

use avian3d::prelude::SpatialQuery;
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::mount::config::LocalConfig;
use chimera_sim::weapon::{WeaponEvent, WeaponEventKind};
use chimera_world::{
    harness::Sim,
    map::{
        MapSimPlugin,
        nav::{NavMesh, Via},
        nav_gen::{self, NavWithin},
        teleport::MapTeleporters,
    },
};
use combat_arms::{game::bots::CaBot, harness::CaSim};
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin, spot_checks::spawn_feet};

/// The gun game on Halo map `id` with `bots` Combat Arms bots; None
/// without the disc image and both installs.
fn party_on(id: &str, bots: u32) -> Option<Sim> {
    if halo_ce::game::INFO.mount().is_none() {
        eprintln!("skipping: no Halo CE disc image ($CHIMERA_HALO_CE_ROOT or [games.halo_ce] path)");
        return None;
    }
    let config = LocalConfig::load().ok()?;
    let css = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config
        .game_path(combat_arms::game::GAME)
        .filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if css.is_none() || ca.is_none() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return None;
    }
    let map = party_roster::load_map(id, 0).unwrap_or_else(|e| panic!("{id}: {e}"));
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
    chimera_sim::minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    let mut s = sim
        .app
        .world_mut()
        .resource_mut::<combat_arms::game::bots::BotSettings>();
    s.quota = bots;
    s.quota_set = true;
    Some(sim)
}

fn m2(m: &NavMesh) -> f32 {
    m.areas.iter().map(|a| (a.max - a.min).x * (a.max - a.min).y).sum()
}

/// Blood Gulch's drawn level (structure_bsp.md, "World bounds", world
/// units) in engine space.
fn bloodgulch_drawn() -> (Vec3, Vec3) {
    use halo_ce::game::maps::space::point;
    let (a, b) = (point([5.907, -190.219, -0.349]), point([131.902, -45.057, 26.233]));
    (a.min(b), a.max(b))
}

#[test]
fn blood_gulchs_navigation_stays_in_the_drawn_level() {
    let Some(mut sim) = party_on("halo_ce:bloodgulch", 0) else {
        return;
    };
    sim.ticks(2);
    let w = sim.app.world_mut();
    let (lo, hi) = bloodgulch_drawn();
    let within = NavWithin::of(w).expect("Halo's map says where it is played");
    assert!(
        within.0.distance(lo) < 0.2 && within.1.distance(hi) < 0.2,
        "{within:?} against the spec's {lo} {hi}"
    );
    let feet = spawn_feet(w);
    let p = combat_arms::game::bots::brain::nav_params();
    let mut state = SystemState::<SpatialQuery>::new(w);
    let q = state.get(w).unwrap();
    let run = |within: Option<(Vec3, Vec3)>, max_cells: usize| {
        let t = Instant::now();
        let mesh = nav_gen::generate(&q, &|_| false, &feet, &nav_gen::NavGenParams { within, max_cells, ..p });
        (mesh, t.elapsed().as_secs_f32())
    };
    // Before (unheld: stopped at its cell limit) and after (held: grown
    // to the box's footprint, `NavGenParams::cell_limit`), and unheld
    // without a limit (where the walker would go).
    let (free, free_s) = run(None, p.max_cells);
    let (whole, whole_s) = run(None, 4_000_000);
    let (held, held_s) = run(Some(within), p.max_cells);
    let cells = |m: &NavMesh| (m2(m) / (p.cell * p.cell)).round() as usize;
    let capped = |m: &NavMesh, held: bool| {
        let limit = nav_gen::NavGenParams {
            within: held.then_some(within),
            ..p
        }
        .cell_limit();
        cells(m) as f32 >= 0.95 * limit as f32
    };
    let above = |m: &NavMesh| {
        m.areas
            .iter()
            .filter(|a| a.heights.iter().any(|y| *y > hi.y + 0.1))
            .count()
    };
    println!("bloodgulch, the bots' mesh: areas, m2, cells, capped, areas above the drawn top, seconds");
    println!(
        "  unheld {} {:.0} {} {} {} {free_s:.1}",
        free.areas.len(),
        m2(&free),
        cells(&free),
        capped(&free, false),
        above(&free)
    );
    println!(
        "  unheld, no limit {} {:.0} {} - {} {whole_s:.1}",
        whole.areas.len(),
        m2(&whole),
        cells(&whole),
        above(&whole)
    );
    println!(
        "  held   {} {:.0} {} {} {} {held_s:.1}",
        held.areas.len(),
        m2(&held),
        cells(&held),
        capped(&held, true),
        above(&held)
    );
    assert!(!capped(&held, true), "the held flood ends before its cell limit");
    assert!(
        (m2(&held) - m2(&whole)).abs() < 1.0,
        "held, every floor the walker reaches in the drawn level"
    );
    let half = p.cell * 0.5 + 0.06;
    let outside: Vec<_> = held
        .areas
        .iter()
        .filter(|a| {
            a.min.x < lo.x - half
                || a.min.y < lo.z - half
                || a.max.x > hi.x + half
                || a.max.y > hi.z + half
                || a.heights.iter().any(|y| *y > hi.y + 0.1 || *y < lo.y - 0.1)
        })
        .map(|a| (a.min, a.max, a.heights))
        .collect();
    assert!(
        outside.is_empty(),
        "{} areas outside the drawn level: {:?}",
        outside.len(),
        &outside[..outside.len().min(5)]
    );
    // Every spawn is still on it.
    assert!(
        feet.iter().all(|f| held.area_at(*f + Vec3::Y * 0.1).is_some()),
        "every spawn on the held mesh"
    );
}

/// The areas joined by walks (either way), as one label per area.
fn walk_parts(m: &NavMesh) -> Vec<usize> {
    let mut part: Vec<usize> = (0..m.areas.len()).collect();
    fn root(part: &mut [usize], mut i: usize) -> usize {
        while part[i] != i {
            part[i] = part[part[i]];
            i = part[i];
        }
        i
    }
    for (i, a) in m.areas.iter().enumerate() {
        for (j, via) in &a.links {
            if matches!(via, Via::Walk(_)) {
                let (ri, rj) = (root(&mut part, i), root(&mut part, *j));
                part[ri] = rj;
            }
        }
    }
    (0..m.areas.len()).map(|i| root(&mut part, i)).collect()
}

#[test]
fn a_bot_on_putput_leaves_its_room_through_a_teleporter() {
    let Some(mut sim) = party_on("halo_ce:putput", 1) else {
        return;
    };
    // The bot in, its navigation made.
    sim.seconds(4.0);
    let w = sim.app.world_mut();
    let mesh = w.resource::<NavMesh>().clone();
    let pads = w.resource::<MapTeleporters>().clone();
    let parts = walk_parts(&mesh);
    let mut count: HashMap<usize, f32> = HashMap::new();
    for (i, a) in mesh.areas.iter().enumerate() {
        *count.entry(parts[i]).or_default() += (a.max - a.min).x * (a.max - a.min).y;
    }
    println!(
        "putput: {} areas in {} walk parts (m2: {:?}), {} teleporters, {} teleporter links",
        mesh.areas.len(),
        count.len(),
        {
            let mut v: Vec<_> = count.values().map(|v| v.round() as i32).collect();
            v.sort();
            v.reverse();
            v
        },
        pads.pads.len(),
        mesh.teleports.len()
    );
    assert!(count.len() > 1, "putput's rooms join only through teleporters");
    assert!(!mesh.teleports.is_empty(), "the bots' mesh links the teleporters");
    let bot = w.query_filtered::<Entity, With<CaBot>>().single(w).expect("one bot");
    let part_of = |p: Vec3| {
        mesh.area_at(p + Vec3::Y * 0.1)
            .or_else(|| mesh.nearest_area(p))
            .map(|a| parts[a])
    };
    let start = sim.position(bot);
    let home = part_of(start - Vec3::Y * 0.8);
    println!("the bot starts at {start} in walk part {home:?}");
    let mut last = start;
    let mut through = Vec::new();
    let mut left_at = None;
    for k in 0..(90 * 10) {
        sim.seconds(0.1);
        let at = sim.position(bot);
        if at.distance(last) > 4.0 {
            // A jump: a teleporter's if it was by a pad and is now by its exit.
            let pad = pads
                .pads
                .iter()
                .find(|p| p.source.xz().distance(last.xz()) < 2.5 && p.dest.xz().distance(at.xz()) < 2.5);
            println!(
                "  {:.1} s: jumped from {last} to {at}{}",
                k as f32 / 10.0,
                if pad.is_some() { " (a teleporter)" } else { "" }
            );
            if pad.is_some() {
                through.push(k);
            }
        }
        last = at;
        if !through.is_empty() && part_of(at - Vec3::Y * 0.8) != home {
            left_at = Some(k as f32 / 10.0);
            break;
        }
    }
    let left_at = left_at.unwrap_or_else(|| {
        panic!(
            "the bot never left its room through a teleporter (at {last}, teleported {} times)",
            through.len()
        )
    });
    println!("left its room through a teleporter after {left_at:.1} s, now at {last}");
}

/// Rounds fired, by shooter.
#[derive(Resource, Default)]
struct Rounds(HashMap<Entity, u32>);

fn count_rounds(mut events: MessageReader<WeaponEvent>, mut rounds: ResMut<Rounds>) {
    for e in events.read() {
        if matches!(e.kind, WeaponEventKind::Fired { .. }) {
            *rounds.0.entry(e.owner).or_default() += 1;
        }
    }
}

#[test]
fn bots_on_hang_em_high_get_about_and_fight() {
    let Some(mut sim) = party_on("halo_ce:hangemhigh", 6) else {
        return;
    };
    sim.app.init_resource::<Rounds>().add_systems(FixedUpdate, count_rounds);
    sim.seconds(6.0);
    let mut walked: HashMap<Entity, (Vec3, f32)> = HashMap::new();
    for _ in 0..(40 * 4) {
        sim.seconds(0.25);
        let w = sim.app.world_mut();
        for (e, t) in w.query_filtered::<(Entity, &Transform), With<CaBot>>().iter(w) {
            let row = walked.entry(e).or_insert((t.translation, 0.0));
            let step = t.translation.distance(row.0);
            if step < 6.0 {
                row.1 += step;
            }
            row.0 = t.translation;
        }
    }
    let rounds = sim.app.world().resource::<Rounds>();
    let bots: Vec<(f32, u32)> = walked
        .iter()
        .map(|(e, (_, d))| (*d, rounds.0.get(e).copied().unwrap_or(0)))
        .collect();
    let mesh = sim.app.world().resource::<NavMesh>();
    println!(
        "hangemhigh: {} nav areas ({:.0} m2); each bot's (m covered, rounds fired) in 40 s: {bots:?}",
        mesh.areas.len(),
        m2(mesh)
    );
    assert_eq!(bots.len(), 6);
    assert!(bots.iter().filter(|b| b.1 > 0).count() >= 3, "most bots fire: {bots:?}");
    assert!(
        bots.iter().filter(|b| b.0 > 20.0).count() >= 5,
        "most bots get about: {bots:?}"
    );
}
