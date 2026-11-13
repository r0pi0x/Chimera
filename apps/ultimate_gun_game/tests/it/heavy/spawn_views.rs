//! Spawning by score (`points = "scored"`, `chimera_sim::minigame::
//! spawn_score`) on real maps: the automatic gun game with 4, 8 and 12
//! Combat Arms bots on de_dust2, cs_office, Combat Arms' warehouse and
//! HL2:DM's dm_lockdown, headless and repeatable. Measured at each spawn,
//! against every living enemy then:
//!
//! - "in view": a clear line from the enemy's eye to the spawned player's
//!   head or centre, inside its field of view (110 degrees, from where it
//!   looks);
//! - "seen": the same line, wherever the enemy looks;
//! - "near": an enemy under 10 m.
//!
//! Three moments: everyone joining in the first `JOIN` s (a server
//! starting), the match's start (`minigame::restart`, everyone placed in
//! one tick; `STARTS` times), and the respawns of `PLAY` s of bot play
//! (one bot also dies of the world each second, so there are enough).
//! Prints the rates per map and count (`--nocapture`) and fails when a
//! start's or a respawn's rate passes its bound (`bounds`).
//! `CHIMERA_SPAWN_BEFORE=1` also measures `random_hidden` (the gun game's
//! spawns before scoring) for comparison, checking nothing of it;
//! `CHIMERA_SPAWN_SLICED=1` makes the spots a slice a tick as a front end
//! does when the minigame starts on a map already loaded (the spots ready
//! some seconds in: the joining spawns at the map's spawn points until
//! then, which a party app's menu avoided only once its spots were made
//! behind the loading screen, `spawns::settle_spots`);
//! `CHIMERA_SPAWN_MAPS=<game:map>,...` and `CHIMERA_SPAWN_COUNTS=4,8`
//! narrow the runs. Passes with a note without the installs.
//!
//! `cargo test -p ultimate_gun_game --features dev --test it heavy::spawn_views -- --nocapture`

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::{
    core::{Damage, DamageKind, Dead, Health, Hitgroup, Intent, SOLID_LAYERS},
    mount::config::LocalConfig,
};
use chimera_sim::{
    minigame::{self, ActiveMinigame, InMinigame, def::Points, spawns::SpawnSpots},
    mode::{Match, Phase},
};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin};

const MAPS: [&str; 4] = [
    "cs_source:de_dust2",
    "cs_source:cs_office",
    "combat_arms:warehouse",
    "hl2dm:dm_lockdown",
];
const COUNTS: [u32; 3] = [4, 8, 12];
/// Match starts measured per map and count.
const STARTS: usize = 6;
/// Seconds after the minigame's start whose spawns (everyone joining) are
/// measured.
const JOIN: f64 = 5.0;
/// Seconds of bot play whose respawns are measured.
const PLAY: f64 = 45.0;
const FOV: f32 = 110.0;
/// Instructions a scored spawn may take among the living: 0.9 to 1.6 M
/// measured (2026-11-13), 4.1 M on the warehouse at 12 (its spots mostly
/// in someone's sight, so more of the sample has its lines checked).
const SPAWN_BUDGET: u64 = 10_000_000;
const NEAR: f32 = 10.0;
/// An enemy's eye above its centre, as the scoring has it.
const EYE: f32 = 0.7;

fn installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config
        .game_path(combat_arms::game::GAME)
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
}

#[derive(Default, Clone, Copy)]
struct Rates {
    spawns: u32,
    in_view: u32,
    seen: u32,
    near: u32,
}

impl Rates {
    fn pct(&self, n: u32) -> f32 {
        if self.spawns == 0 {
            0.0
        } else {
            100.0 * n as f32 / self.spawns as f32
        }
    }
    fn line(&self) -> String {
        format!(
            "{} spawns: in view {:.1}%, seen {:.1}%, near {:.1}%",
            self.spawns,
            self.pct(self.in_view),
            self.pct(self.seen),
            self.pct(self.near)
        )
    }
}

/// The living players in the match: entity, centre, look.
fn living(sim: &mut Sim) -> Vec<(Entity, Vec3, Vec3)> {
    let w = sim.app.world_mut();
    let mut out: Vec<(Entity, Vec3, Vec3)> = w
        .query_filtered::<(Entity, &Transform, &Health, &Intent), (With<InMinigame>, Without<Dead>)>()
        .iter(w)
        .filter(|(_, _, h, _)| h.current > 0.0)
        .map(|(e, t, _, i)| {
            (
                e,
                t.translation,
                Quat::from_euler(EulerRot::YXZ, i.yaw, i.pitch, 0.0) * Vec3::NEG_Z,
            )
        })
        .collect();
    out.sort_by_key(|o| o.0);
    out
}

/// Each of `spawned` measured against every other living player.
fn measure(sim: &mut Sim, spawned: &[Entity], rates: &mut Rates) {
    let all = living(sim);
    let w = sim.app.world_mut();
    let characters: Vec<Entity> = w.query_filtered::<Entity, With<Intent>>().iter(w).collect();
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
    for s in spawned {
        let Some(&(_, at, _)) = all.iter().find(|o| o.0 == *s) else {
            continue;
        };
        rates.spawns += 1;
        let (mut in_view, mut seen, mut near) = (false, false, false);
        for &(o, c, look) in &all {
            if o == *s {
                continue;
            }
            let eye = c + Vec3::Y * EYE;
            near |= c.distance(at) < NEAR;
            if clear(eye, at + Vec3::Y * EYE) || clear(eye, at) {
                seen = true;
                let angle = look
                    .dot((at - eye).normalize_or_zero())
                    .clamp(-1.0, 1.0)
                    .acos()
                    .to_degrees();
                in_view |= angle <= FOV / 2.0;
            }
        }
        rates.in_view += in_view as u32;
        rates.seen += seen as u32;
        rates.near += near as u32;
    }
}

/// One map and count: the rates of everyone joining, of the match
/// starts and of the respawns.
fn run(map: &str, bots: u32, points: Points) -> Option<[Rates; 3]> {
    let data = party_kit::load_map(map, 0).ok()?;
    let mut sim = Sim::new(
        CaSim,
        (
            MapSimPlugin::new(data),
            PartyPartsPlugin,
            UltimateGunGamePlugin {
                seed: 36336,
                rungs: 13,
                ..Default::default()
            },
        ),
    );
    sim.set_tick_interval(TICK_INTERVAL);
    sim.repeatable();
    {
        let mut s = sim
            .app
            .world_mut()
            .resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = bots;
        s.quota_set = true;
    }
    let sliced = std::env::var("CHIMERA_SPAWN_SLICED").is_ok_and(|v| v == "1");
    if sliced {
        sim.app.insert_resource(chimera_world::map::nav_gen::SliceFloods);
    }
    minigame::start(sim.app.world_mut(), MINIGAME).expect("the automatic gun game starts");
    {
        let w = sim.app.world_mut();
        let active = *w.resource::<ActiveMinigame>();
        let mut def = active.def.clone();
        def.spawn.points = points;
        w.insert_resource(ActiveMinigame {
            def: Box::leak(Box::new(def)),
            rules: active.rules,
        });
    }
    // The first seconds: everyone joining as the server starts.
    let mut joins = Rates::default();
    let mut known: Vec<Entity> = Vec::new();
    let mut ready_at = None;
    for t in 0..(JOIN * sim.tick_hz()) as u64 {
        sim.ticks(1);
        if ready_at.is_none() && sim.app.world().get_resource::<SpawnSpots>().is_some_and(|s| s.ready) {
            ready_at = Some(t);
        }
        let now: Vec<Entity> = living(&mut sim)
            .into_iter()
            .map(|o| o.0)
            .filter(|e| !known.contains(e))
            .collect();
        if !now.is_empty() {
            measure(&mut sim, &now, &mut joins);
            known.extend(now);
        }
    }
    if sliced {
        eprintln!("{map} x{bots}: the spots ready at tick {ready_at:?}");
    } else {
        assert!(ready_at.is_some(), "{map}: the spots are made at the first spawn");
    }
    let mut starts = Rates::default();
    for _ in 0..STARTS {
        minigame::restart(sim.app.world_mut());
        for _ in 0..20 {
            sim.ticks(1);
            if sim.app.world().resource::<Match>().phase != Phase::Waiting {
                break;
            }
        }
        let everyone: Vec<Entity> = living(&mut sim).into_iter().map(|o| o.0).collect();
        assert_eq!(everyone.len() as u32, bots, "{map}: everyone in");
        measure(&mut sim, &everyone, &mut starts);
        sim.seconds(3.0);
    }
    // A spawn's cost: picks for the living (not taken), counted.
    if points == Points::Scored
        && let Some(before) = chimera_core::metrics::work::now()
    {
        let alive: Vec<Entity> = living(&mut sim).into_iter().map(|o| o.0).collect();
        let n = 40u64;
        for k in 0..n as usize {
            let w = sim.app.world_mut();
            minigame::spawn_score::pick(w, alive[k % alive.len()], minigame::def::Facing::default());
        }
        let each = chimera_core::metrics::work::now()
            .unwrap_or(before)
            .saturating_sub(before)
            / n;
        eprintln!("{map} x{bots}: {:.2} M instructions a scored spawn", each as f64 / 1e6);
        assert!(each < SPAWN_BUDGET, "{map} x{bots}: {each} instructions a spawn");
    }
    let mut respawns = Rates::default();
    let ticks = (PLAY * sim.tick_hz()) as u64;
    let dead = |sim: &mut Sim| -> Vec<Entity> {
        let w = sim.app.world_mut();
        w.query_filtered::<Entity, (With<InMinigame>, With<Dead>)>()
            .iter(w)
            .collect()
    };
    let mut was_dead = dead(&mut sim);
    let every = sim.tick_hz() as u64;
    for t in 0..ticks {
        // Besides the bots' own kills, one of them dies of the world each
        // second (a fixed turn), so there are respawns to measure.
        if t % every == 0 {
            let alive = living(&mut sim);
            if !alive.is_empty() {
                let (victim, at, _) = alive[(t / every * 7) as usize % alive.len()];
                sim.app.world_mut().write_message(Damage {
                    target: victim,
                    attacker: None,
                    amount: 5000.0,
                    point: at,
                    dir: Vec3::NEG_Z,
                    hitgroup: Hitgroup::Chest,
                    kind: DamageKind::Bullet,
                    weapon: None,
                    force: Vec3::ZERO,
                });
            }
        }
        sim.ticks(1);
        let now_dead = dead(&mut sim);
        let back: Vec<Entity> = was_dead.iter().copied().filter(|e| !now_dead.contains(e)).collect();
        if !back.is_empty() {
            measure(&mut sim, &back, &mut respawns);
        }
        was_dead = now_dead;
    }
    Some([joins, starts, respawns])
}

/// The bounds (percent) on the scored spawns' rates at the start and on
/// respawns, (in view, near), per map and count. Measured (2026-11-13):
/// no start in view on any map at any count, none near but Combat Arms'
/// warehouse at 12 (16.7%: a small map, everyone placed at once); on
/// respawns at most 2.2% in view (de_dust2 at 12) and 2.3% near
/// (warehouse at 8 and 12). `random_hidden` before: starts in view on
/// warehouse 20.8% (8) and 51.4% (12), dm_lockdown 6.9% (12); respawns
/// in view on warehouse 24.4% (12).
fn bounds(map: &str, bots: u32) -> ((f32, f32), (f32, f32)) {
    let small = map == "combat_arms:warehouse";
    match bots {
        0..=8 => ((0.0, 0.0), (5.0, 5.0)),
        _ => ((0.0, if small { 20.0 } else { 5.0 }), (5.0, 5.0)),
    }
}

#[test]
fn scored_spawns_keep_out_of_the_enemies_view() {
    if !installed() {
        eprintln!("skipping: needs the CS:S, Combat Arms and HL2:DM installs");
        return;
    }
    let maps: Vec<String> = std::env::var("CHIMERA_SPAWN_MAPS")
        .map(|s| s.split(',').map(str::to_string).collect())
        .unwrap_or_else(|_| MAPS.iter().map(|s| s.to_string()).collect());
    let counts: Vec<u32> = std::env::var("CHIMERA_SPAWN_COUNTS")
        .map(|s| s.split(',').filter_map(|n| n.parse().ok()).collect())
        .unwrap_or_else(|_| COUNTS.to_vec());
    let before = std::env::var("CHIMERA_SPAWN_BEFORE").is_ok_and(|v| v == "1");
    let mut failures = Vec::new();
    for map in &maps {
        for &n in &counts {
            let line =
                |[j, s, r]: [Rates; 3]| format!("joining {}\n  start {}\n  respawns {}", j.line(), s.line(), r.line());
            if before {
                match run(map, n, Points::RandomHidden) {
                    Some(rates) => eprintln!("{map} x{n} random_hidden:\n  {}", line(rates)),
                    None => eprintln!("{map}: not loaded, skipped"),
                }
            }
            let Some(rates) = run(map, n, Points::Scored) else {
                eprintln!("{map}: not loaded, skipped");
                continue;
            };
            eprintln!("{map} x{n} scored:\n  {}", line(rates));
            let [_, s, r] = rates;
            let ((sv, sn), (rv, rn)) = bounds(map, n);
            if s.pct(s.in_view) > sv || s.pct(s.near) > sn || r.pct(r.in_view) > rv || r.pct(r.near) > rn {
                failures.push(format!("{map} x{n}: {}", line(rates)));
            }
        }
    }
    assert!(failures.is_empty(), "over the bounds:\n{}", failures.join("\n"));
}
