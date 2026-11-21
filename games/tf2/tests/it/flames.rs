//! The Flame Thrower's flame points and the stream drawn on them
//! (specs/tf2/flames.md; `weapons::flame`, `weapons::looks::flame_stream`)
//! on the greybox (`harness::Tf2Sim`):
//!
//! - the drawn copy (`FlameShown`) is the server's damage points
//!   (`FlameStream`) tick for tick, on the same machine, on the firing
//!   client (predicted) and on another client (from the server's rounds);
//! - the stream's control points are those points, the newest active,
//!   one `new_flame` per burst, stopped on release; the crit and
//!   under-water names;
//! - damage only where the points are (no cone): a target beside the
//!   stream takes nothing, every hit on one in it has a point's box on him;
//! - a wall: the points slide on it and climb, never through it;
//! - the burst's sounds and the pilot light (effects.md 14.2): when each
//!   starts, stops and fades, on the Pyro's machine and another's.

use std::collections::HashMap;

use bevy::{ecs::message::MessageCursor, prelude::*};
use chimera_core::core::{Health, Intent};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_sim::weapon::{self, Inventory, Weapon};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{
        FadeSound, PlaySound, StopSound,
        psys::{ParticleControlPoints, StartParticleSystem, StopParticleSystems},
    },
};
use tf2::{
    game::{
        TICK_INTERVAL,
        weapons::{
            FLAMETHROWER,
            flame::{FlamePoint, FlameShown, FlameStream, Tf2Flame, segment_meets_box},
            looks::{GunLooks, particle_systems},
        },
    },
    harness::Tf2Sim,
};

use super::weapons::{hold_still, holding, sim, target};

const U: f32 = 0.0254;

/// Look level along -Z.
fn level(sim: &mut Sim, p: Entity) {
    let mut i = sim.intent(p);
    i.yaw = 0.0;
    i.pitch = 0.0;
}

fn points(world: &World, w: Entity) -> (Vec<FlamePoint>, Vec<FlamePoint>) {
    let damage = world.get::<FlameStream>(w).map(|s| s.points.points.clone()).unwrap_or_default();
    let shown = world.get::<FlameShown>(w).map(|s| s.points.points.clone()).unwrap_or_default();
    (damage, shown)
}

/// On one machine, the drawn points are the damage points tick for tick
/// (the same rounds, seeds and motion; frames are ticks here), and the
/// stream's control points are the drawn points (slot i at i + 1, the
/// newest active).
#[test]
fn the_drawn_points_are_the_damage_points_and_the_streams_control_points() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[1], FLAMETHROWER);
    level(&mut sim, p);
    let mut compared = 0;
    for k in 0..60 {
        sim.intent(p).fire = k < 40;
        sim.ticks(1);
        let world = sim.app.world();
        let (damage, shown) = points(world, w);
        assert_eq!(damage.len(), shown.len(), "tick {k}");
        for (a, b) in damage.iter().zip(&shown) {
            assert_eq!(a.slot, b.slot);
            assert!((a.at - b.at).length() < 1e-4, "tick {k}: {:?} vs {:?}", a.at, b.at);
            assert!((a.before - b.before).length() < 1e-4);
            compared += 1;
        }
        let cps = world.get::<ParticleControlPoints>(w).expect("the stream's control points");
        assert_eq!(cps.points.len(), shown.len());
        for (p, (n, c)) in shown.iter().zip(&cps.points) {
            assert_eq!(*n, p.slot as usize + 1);
            assert_eq!(c.position, p.at);
            assert!((c.radius / U - 12.0).abs() < 1e-4 && c.duration == p.life);
        }
        assert_eq!(cps.active, shown.last().map(|p| p.slot as usize + 1));
    }
    assert!(compared > 300, "{compared}");
}

/// One `new_flame` per burst, tagged with the gun (its control points'
/// owner), stopped when the burst ends; another on the next burst.
#[test]
fn each_burst_starts_one_new_flame_and_stops_it() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[1], FLAMETHROWER);
    level(&mut sim, p);
    let mut starts = sim.app.world().resource::<Messages<StartParticleSystem>>().get_cursor_current();
    let mut stops = sim.app.world().resource::<Messages<StopParticleSystems>>().get_cursor_current();
    let (mut started, mut stopped) = (Vec::new(), Vec::new());
    for k in 0..120 {
        sim.intent(p).fire = k < 30 || (60..80).contains(&k);
        sim.ticks(1);
        let world = sim.app.world();
        started.extend(starts.read(world.resource::<Messages<StartParticleSystem>>()).map(|s| (k, s.clone())));
        stopped.extend(stops.read(world.resource::<Messages<StopParticleSystems>>()).map(|s| (k, *s)));
    }
    let names: Vec<&str> = started.iter().map(|(_, s)| s.name.as_str()).collect();
    assert_eq!(names, ["new_flame", "new_flame"], "{started:?}");
    assert!(started.iter().all(|(_, s)| s.tag == Some(w) && s.emit_for.is_none()));
    assert_eq!(stopped.len(), 2, "{stopped:?}");
    assert!(stopped.iter().all(|(_, s)| s.tag == w));
    // Stopped a little after each release (`burst_gap` 0.1 s), before the
    // next burst.
    assert!(stopped[0].0 > 30 && stopped[0].0 < 60 && stopped[1].0 > 80, "{stopped:?}");
}

/// flames.md 3.1: the crit stream by team, under water's.
#[test]
fn the_stream_names_crit_and_under_water() {
    let looks = GunLooks::default();
    let s = looks.stream_of(FLAMETHROWER).expect("the Flame Thrower's stream");
    assert_eq!(s.system_for(false, false, false), "new_flame");
    assert_eq!(s.system_for(false, true, false), "new_flame", "the stream itself has no team");
    assert_eq!(s.system_for(true, false, false), "new_flame_crit_red");
    assert_eq!(s.system_for(true, true, false), "new_flame_crit_blue");
    assert_eq!(s.system_for(true, true, true), "flamethrower_underwater");
    for n in ["new_flame", "new_flame_crit_red", "new_flame_crit_blue", "flamethrower_underwater"] {
        assert!(particle_systems().contains(&n), "{n} loads");
    }
    assert!(!particle_systems().contains(&"flamethrower"), "the old system isn't used");
}

/// Damage only where the points are (flames.md 2: no cone): a target
/// 60 units beside the stream 100 ahead takes nothing; one in it is hit,
/// and at every hit a point's swept box is on him.
#[test]
fn damage_lands_only_where_the_points_are() {
    // Beside the stream.
    let mut sim = sim();
    let from = greybox::SPAWNS[1];
    let (p, _) = holding(&mut sim, from, FLAMETHROWER);
    let beside = sim.spawn_character(from + Vec3::new(60.0, 0.0, -100.0) * U, chimera_sim::movement::placeholder::ID);
    sim.seconds(1.0);
    level(&mut sim, p);
    let before = sim.app.world().get::<Health>(beside).unwrap().current;
    for _ in 0..90 {
        sim.intent(p).fire = true;
        hold_still(&mut sim, beside);
        sim.ticks(1);
    }
    assert_eq!(sim.app.world().get::<Health>(beside).unwrap().current, before, "no cone");

    // In it.
    let mut sim = super::weapons::sim();
    let (p, w) = holding(&mut sim, from, FLAMETHROWER);
    let near = target(&mut sim, from, 150.0);
    sim.app.world_mut().get_mut::<Health>(near).unwrap().current = 1000.0;
    level(&mut sim, p);
    let mut hits = 0;
    let mut last = sim.app.world().get::<Health>(near).unwrap().current;
    for _ in 0..90 {
        sim.intent(p).fire = true;
        hold_still(&mut sim, near);
        sim.ticks(1);
        let world = sim.app.world();
        let h = world.get::<Health>(near).unwrap().current;
        if h < last && world.get::<tf2::game::weapons::flame::Burning>(near).is_some_and(|b| b.left > 0.0) && last - h > 0.045 {
            // A flame hit (not the 0.04 of afterburn): a point's box on him.
            let (min, max) = victim_box(world, near);
            let half = Vec3::splat(12.0 * U);
            let (damage, _) = points(world, w);
            assert!(
                damage.iter().any(|pt| segment_meets_box(pt.before, pt.at, min - half, max + half)),
                "a hit with no point on him"
            );
            hits += 1;
        }
        last = h;
    }
    assert!(hits >= 5, "{hits}");
}

fn victim_box(world: &World, e: Entity) -> (Vec3, Vec3) {
    let aabb = world.get::<avian3d::prelude::ColliderAabb>(e).expect("a box");
    (aabb.min, aabb.max)
}

/// flames.md 1: points meeting a wall slide on it and climb (the drift
/// kept each update), never through it.
#[test]
fn points_slide_up_a_wall_never_through_it() {
    let mut sim = sim();
    // 150 units from the north wall's face.
    let from = Vec3::new(25.0, 1.0, greybox::NORTH_WALL_Z + 150.0 * U);
    let (p, w) = holding(&mut sim, from, FLAMETHROWER);
    level(&mut sim, p);
    let face = greybox::NORTH_WALL_Z + 12.0 * U;
    let (mut slid, mut climbing) = (0, 0);
    // The damage points' last upward speed and walls met, by birth.
    let mut last: HashMap<(u8, u64), (f32, u16)> = HashMap::new();
    for _ in 0..60 {
        sim.intent(p).fire = true;
        sim.ticks(1);
        let (damage, shown) = points(sim.app.world(), w);
        for pt in &damage {
            // Meeting the wall, its velocity keeps the drift: it gains
            // upward speed (TF2's slide quirk) where a free point loses
            // some to the drag.
            let key = (pt.slot, pt.born.to_bits());
            if let Some((vy, hits)) = last.get(&key)
                && pt.wall_hits > *hits
            {
                assert!(pt.velocity.y > *vy, "{} -> {}", vy / U, pt.velocity.y / U);
                climbing += 1;
            }
            last.insert(key, (pt.velocity.y, pt.wall_hits));
        }
        for pt in damage.iter().chain(&shown) {
            assert!(pt.at.z >= face - 1e-3, "through the wall: {}", (pt.at.z - greybox::NORTH_WALL_Z) / U);
            if pt.wall_hits > 0 {
                slid += 1;
                assert_eq!(pt.carried, Vec3::ZERO);
                assert!(pt.velocity.z > -1e-3, "nothing into the wall: {:?}", pt.velocity / U);
            }
        }
    }
    assert!(slid > 50 && climbing > 5, "{slid} {climbing}");
}

/// The visible points match the damage points tick for tick on the
/// server and the clients (flames.md 1): the firing client's own
/// (predicted) and another client's (from the server's rounds) are the
/// server's, point by point and update by update.
#[test]
fn every_client_draws_the_servers_points() {
    let link = LinkConditions { latency: std::time::Duration::from_millis(40), ..LinkConditions::default() };
    let mut sim = NetSim::new(Tf2Sim, link, 5, 0, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.add_client(|_| {});
    sim.add_client(|_| {});
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(900);
    sim.until_looks_taken(600);
    let theirs = sim.character_of(0).unwrap();
    let sw = weapon::give(sim.server.app.world_mut(), theirs, FLAMETHROWER).unwrap();
    sim.server.app.world_mut().get_mut::<Inventory>(theirs).unwrap().wanted = Some(sw);
    sim.ticks(150);
    let me = sim.local_player(0).unwrap();
    let mine = sim.clients[0].app.world().get::<Inventory>(me).unwrap().active.expect("drawn");
    assert_eq!(sim.clients[0].app.world().get::<Weapon>(mine).unwrap().id, FLAMETHROWER);
    // Each machine's points by birth: (its slot, its birth) -> places.
    type Tracks = (HashMap<(u8, u64), usize>, Vec<Vec<Vec3>>);
    let mut tracks: [Tracks; 3] = Default::default();
    let record = |t: &mut Tracks, pts: &[FlamePoint]| {
        for pt in pts {
            let key = (pt.slot, pt.born.to_bits());
            let n = t.1.len();
            let i = *t.0.entry(key).or_insert(n);
            if i == t.1.len() {
                t.1.push(Vec::new());
            }
            t.1[i].push(pt.at);
        }
    };
    for k in 0..90 {
        {
            let me = sim.local_player(0).unwrap();
            let mut i = sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap();
            i.fire = k < 40;
            i.yaw = 0.3;
            i.pitch = -0.05;
        }
        sim.step();
        let (server, _) = points(sim.server.app.world(), sw);
        record(&mut tracks[0], &server);
        let (_, own) = points(sim.clients[0].app.world(), mine);
        record(&mut tracks[1], &own);
        // Client 1's copy of the shooter and the gun in his hands.
        let world = sim.clients[1].app.world_mut();
        let guns: Vec<Entity> = world.query_filtered::<Entity, With<Tf2Flame>>().iter(world).collect();
        let shown: Vec<FlamePoint> = guns.iter().flat_map(|g| points(world, *g).1).collect();
        record(&mut tracks[2], &shown);
    }
    let server = &tracks[0].1;
    assert!(server.len() >= 15, "{} points", server.len());
    for (who, t) in [("the firing client", &tracks[1].1), ("another client", &tracks[2].1)] {
        assert_eq!(t.len(), server.len(), "{who}: every point");
        for (i, (a, b)) in server.iter().zip(t).enumerate() {
            let n = a.len().min(b.len());
            assert!(n + 2 >= a.len(), "{who}: point {i} lived {} updates of the server's {}", b.len(), a.len());
            for k in 0..n {
                assert!((a[k] - b[k]).length() < 0.01 * U, "{who}: point {i} update {k}: {:?} vs {:?}", a[k] / U, b[k] / U);
            }
        }
    }
}

// --- The burst's sounds and the pilot light (specs/tf2/effects.md 14.2)

/// What one tick sent of the Flame Thrower's sounds: played (entry,
/// volume, channel), stopped, faded (entry, to, over).
#[derive(Default, Debug, Clone)]
struct Heard {
    played: Vec<(String, Option<f32>, Option<u8>)>,
    stopped: Vec<String>,
    faded: Vec<(String, f32, f32)>,
}

impl Heard {
    fn played(&self, end: &str) -> Option<&(String, Option<f32>, Option<u8>)> {
        self.played.iter().find(|p| p.0.ends_with(end))
    }
    fn stopped(&self, end: &str) -> bool {
        self.stopped.iter().any(|s| s.ends_with(end))
    }
    fn faded(&self, end: &str) -> Option<(f32, f32)> {
        self.faded.iter().find(|f| f.0.ends_with(end)).map(|f| (f.1, f.2))
    }
    fn is_empty(&self) -> bool {
        self.played.is_empty() && self.stopped.is_empty() && self.faded.is_empty()
    }
}

/// Cursors on one app's sound messages.
struct Ears(MessageCursor<PlaySound>, MessageCursor<StopSound>, MessageCursor<FadeSound>);

impl Ears {
    fn new(app: &App) -> Self {
        let w = app.world();
        Self(
            w.resource::<Messages<PlaySound>>().get_cursor_current(),
            w.resource::<Messages<StopSound>>().get_cursor_current(),
            w.resource::<Messages<FadeSound>>().get_cursor_current(),
        )
    }
    /// The Flame Thrower's sounds since the last read.
    fn read(&mut self, app: &App) -> Heard {
        let w = app.world();
        let ours = |e: &str| e.contains("FlameThrower.") && !e.ends_with(".FireHit") && !e.ends_with(".Draw");
        Heard {
            played: self.0.read(w.resource::<Messages<PlaySound>>()).filter(|s| ours(&s.entry)).map(|s| (s.entry.clone(), s.volume, s.channel)).collect(),
            stopped: self.1.read(w.resource::<Messages<StopSound>>()).filter(|s| ours(&s.entry)).map(|s| s.entry.clone()).collect(),
            faded: self.2.read(w.resource::<Messages<FadeSound>>()).filter(|s| ours(&s.entry)).map(|s| (s.entry.clone(), s.to, s.over)).collect(),
        }
    }
}

/// `n` ticks, `fire` deciding the attack button by tick: what each sent.
fn listen(sim: &mut Sim, p: Entity, n: usize, fire: impl Fn(usize) -> bool) -> Vec<Heard> {
    let mut ears = Ears::new(&sim.app);
    (0..n)
        .map(|t| {
            sim.intent(p).fire = fire(t);
            sim.ticks(1);
            ears.read(&sim.app)
        })
        .collect()
}

/// The ticks that sent anything.
fn busy(h: &[Heard]) -> Vec<usize> {
    h.iter().enumerate().filter(|(_, x)| !x.is_empty()).map(|(i, _)| i).collect()
}

const PILOT: &str = ".PilotLoop";
const START: &str = ".FireStart";
const LOOP: &str = ".FireLoop";
const END: &str = ".WindDown";

/// Test cases (effects.md 14.2): the draw starts the pilot light once,
/// nothing restarts it in 15 s idle; a 15-tick tap: in its first tick the
/// pilot light stops, the start (volume 1, no channel) and the loop
/// (volume 0, the weapon channel) start with their 3.5 s fades to 0 and
/// 1, nothing more while held; in the release's tick both are cut, the
/// end (`.WindDown`, no channel) plays and the pilot light again; a burst
/// 10 ticks later leaves the end alone and starts both again; switching
/// away stops the pilot light.
#[test]
fn a_burst_fades_start_and_loop_and_a_release_cuts_both_14_2() {
    let mut sim = sim();
    let p = sim.spawn_character(greybox::SPAWNS[1], tf2::game::movement::ID);
    sim.seconds(1.0);
    let w = weapon::give(sim.app.world_mut(), p, FLAMETHROWER).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    let drawn = listen(&mut sim, p, 1000, |_| false);
    let pilots: Vec<usize> = drawn.iter().enumerate().filter(|(_, h)| h.played(PILOT).is_some()).map(|(i, _)| i).collect();
    assert_eq!(pilots.len(), 1, "the pilot light once, from the draw: {:?}", busy(&drawn));
    assert!(pilots[0] <= 2, "in the draw's tick: {pilots:?}");
    assert_eq!(drawn[pilots[0]].played(PILOT).unwrap().1, None, "at its entry's volume (0.25)");
    assert_eq!(busy(&drawn), pilots, "nothing else, nothing restarts it");

    // A tap of 15 ticks, then 10 ticks off, then 20 more.
    let h = listen(&mut sim, p, 60, |t| t < 15 || (25..45).contains(&t));
    assert_eq!(busy(&h), [0, 15, 25, 45], "{h:#?}");
    let first = &h[0];
    assert!(first.stopped(PILOT), "the pilot light out: {first:?}");
    let start = first.played(START).expect("the start");
    assert_eq!((start.1, start.2), (None, None), "volume 1, never replaced");
    let lp = first.played(LOOP).expect("the loop");
    assert_eq!((lp.1, lp.2), (Some(0.0), Some(weapon::CHAN_WEAPON)), "from 0, the weapon channel");
    assert_eq!(first.faded(START), Some((0.0, 3.5)));
    assert_eq!(first.faded(LOOP), Some((1.0, 3.5)));
    let release = &h[15];
    assert!(release.stopped(START) && release.stopped(LOOP), "both cut in the release's tick: {release:?}");
    let end = release.played(END).expect("the end");
    assert_eq!((end.1, end.2), (None, None));
    assert!(release.played(PILOT).is_some(), "the pilot light again: {release:?}");
    assert!(release.played(".FireEnd").is_none());
    // Fired again within the end: the end left alone, the pilot out.
    let again = &h[25];
    assert!(!again.stopped(END), "{again:?}");
    assert!(again.stopped(PILOT) && again.played(START).is_some() && again.played(LOOP).is_some(), "{again:?}");

    // Switching to the Shotgun stops the pilot light.
    let shotgun = weapon::give(sim.app.world_mut(), p, tf2::game::weapons::SHOTGUN).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(shotgun);
    let away = listen(&mut sim, p, 5, |_| false);
    assert!(away.iter().any(|x| x.stopped(PILOT)), "{away:#?}");
    assert!(away.iter().all(|x| x.played(PILOT).is_none()));
}

/// Test case: fired until the ammo runs out, the burst ends as a release
/// (start and loop cut, the end) but the pilot light doesn't restart.
#[test]
fn running_dry_ends_the_burst_without_the_pilot_light_14_2() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[1], FLAMETHROWER);
    sim.app.world_mut().get_mut::<weapon::Magazine>(w).unwrap().clip = 3;
    let h = listen(&mut sim, p, 120, |_| true);
    let ends: Vec<usize> = h.iter().enumerate().filter(|(_, x)| x.played(END).is_some()).map(|(i, _)| i).collect();
    assert_eq!(ends.len(), 1, "{h:#?}");
    let dry = &h[ends[0]];
    assert!(dry.stopped(START) && dry.stopped(LOOP), "{dry:?}");
    assert!(h.iter().all(|x| x.played(PILOT).is_none()), "no pilot light on an empty gun");
}

/// The fades' volumes are the measured ones (effects.md 14.2 rule 4, the
/// channel list's 0-255 scale; tick 0 the burst's first) and the burst
/// table's.
#[test]
fn the_fades_are_straight_lines_over_3_5_s_14_2() {
    use chimera_world::map::sound::ramp_volume;
    let start = |tick: f32| ramp_volume(1.0, 0.0, 3.5, tick * TICK_INTERVAL as f32) * 255.0;
    let lp = |tick: f32| ramp_volume(0.0, 1.0, 3.5, tick * TICK_INTERVAL as f32) * 255.0;
    for (tick, s, l) in [(3.0, 252.0, 2.0), (12.0, 242.0, 12.0), (60.0, 190.0, 64.0), (86.0, 162.0, 92.0), (218.0, 17.0, 237.0)] {
        // Within 2 ticks (about 2.2 on this scale): the pictures were
        // taken a tick or two off the tick they name.
        assert!((start(tick) - s).abs() <= 2.5, "start at tick {tick}: {} vs {s}", start(tick));
        assert!((lp(tick) - l).abs() <= 2.5, "loop at tick {tick}: {} vs {l}", lp(tick));
    }
    assert_eq!(lp(234.0), 255.0, "the loop holds 1 from 3.5 s");
    assert_eq!(start(240.0), 0.0);
    // The table: a tap (0.225 s) 0.94 / 0.06, a 1 s burst 0.71 / 0.29.
    assert!((ramp_volume(1.0, 0.0, 3.5, 0.225) - 0.94).abs() < 0.005);
    assert!((ramp_volume(0.0, 1.0, 3.5, 1.0) - 0.29).abs() < 0.005);
}

/// Test case: another player hears the same sounds from the Pyro's
/// burst (the server's `PredictedSound`, `StopSound` and `FadeSound`),
/// 0-2 ticks after the Pyro's own.
#[test]
fn others_hear_the_burst_and_its_fades_14_2() {
    let link = LinkConditions { latency: std::time::Duration::from_millis(40), ..LinkConditions::default() };
    let mut sim = NetSim::new(Tf2Sim, link, 5, 0, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.add_client(|_| {});
    sim.add_client(|_| {});
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(900);
    sim.until_looks_taken(600);
    let theirs = sim.character_of(0).unwrap();
    let sw = weapon::give(sim.server.app.world_mut(), theirs, FLAMETHROWER).unwrap();
    sim.server.app.world_mut().get_mut::<Inventory>(theirs).unwrap().wanted = Some(sw);
    sim.ticks(150);
    let mut ears = Ears::new(&sim.clients[1].app);
    let mut heard = Heard::default();
    for k in 0..60 {
        let me = sim.local_player(0).unwrap();
        sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = k < 15;
        sim.step();
        let h = ears.read(&sim.clients[1].app);
        heard.played.extend(h.played);
        heard.stopped.extend(h.stopped);
        heard.faded.extend(h.faded);
    }
    assert!(heard.played(START).is_some() && heard.played(LOOP).is_some_and(|l| l.1 == Some(0.0)), "{heard:#?}");
    assert_eq!(heard.faded(START), Some((0.0, 3.5)), "{heard:#?}");
    assert_eq!(heard.faded(LOOP), Some((1.0, 3.5)), "{heard:#?}");
    assert!(heard.stopped(START) && heard.stopped(LOOP) && heard.stopped(PILOT), "{heard:#?}");
    assert!(heard.played(END).is_some() && heard.played(PILOT).is_some(), "{heard:#?}");
}
