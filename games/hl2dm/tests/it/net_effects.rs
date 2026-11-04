//! Others' shots on a network client (`NetSim` on `Hl2Sim`, the greybox;
//! specs/hl2dm/effects_sounds.md 1-3, H9's cases by number): the server
//! flags each round by its own tracer count (`shot_flags`), the watching
//! client draws a tracer on every bullet of a flagged round (an SMG1
//! round every 2nd bullet; every shotgun shot, 7 pellets each), hears the
//! surface's impact sound and none of the near-miss sounds, and traces
//! the round again through every player (the impact lands on the wall
//! behind the one hit); a double shot is drawn with its 12 pellets.
//! Lag compensation picks whom to rewind as HL2:DM does (weapons.md 12).

use std::{sync::Arc, time::Duration};

use avian3d::prelude::Position;
use bevy::{ecs::message::MessageCursor, prelude::*};
use chimera_core::core::{Intent, Velocity};
use chimera_net::{
    harness::NetSim,
    net::{FireBullets, memory::LinkConditions},
};
use chimera_sim::weapon::{Inventory, Magazine, Weapon, WeaponEvent, WeaponEventKind, give, remote::shot_flags};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    map::{
        PlaySound,
        sound::{MapSounds, MapSurface, SoundBank},
        tracer::Tracer,
    },
};
use hl2dm::{
    game::{
        effects::{AR2_TRACER, TRACER},
        weapons,
    },
    harness::Hl2Sim,
};

/// A server and two clients, joined: client 0 shoots, client 1 watches
/// (with a sound bank whose surfaces have impact sounds).
fn joined() -> NetSim {
    let link = LinkConditions {
        latency: Duration::from_millis(40),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(Hl2Sim, link, 52, 2, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.until_joined(600);
    sim.ticks(200);
    let mut sounds = MapSounds::default();
    for (name, impact) in [("default", "Default.BulletImpact"), ("flesh", "Flesh.BulletImpact")] {
        sounds.surfaces.insert(name.into(), MapSurface { bullet_impact: Some(impact.into()), ..default() });
    }
    sim.clients[1].app.insert_resource(SoundBank(Arc::new(sounds)));
    sim
}

fn intent(sim: &mut NetSim, i: usize) -> Mut<'_, Intent> {
    let me = sim.local_player(i).unwrap();
    sim.clients[i].app.world_mut().get_mut::<Intent>(me).unwrap()
}

/// Put client `i`'s player at `at` looking along `yaw`, still.
fn place(sim: &mut NetSim, i: usize, at: Vec3, yaw: f32) {
    let e = sim.character_of(i).unwrap();
    let w = sim.server.app.world_mut();
    w.get_mut::<Transform>(e).unwrap().translation = at;
    if let Some(mut p) = w.get_mut::<Position>(e) {
        p.0 = at;
    }
    if let Some(mut v) = w.get_mut::<Velocity>(e) {
        v.0 = Vec3::ZERO;
    }
    *intent(sim, i) = Intent { yaw, ..default() };
}

/// Client 0's player holding `id` with a full magazine.
fn arm(sim: &mut NetSim, id: &str) {
    let theirs = sim.character_of(0).unwrap();
    let w = sim.server.app.world_mut();
    let have = w.get::<Inventory>(theirs).unwrap().weapons.iter().copied().find(|e| w.get::<Weapon>(*e).is_some_and(|x| x.id == id));
    let gun = have.unwrap_or_else(|| give(w, theirs, id).unwrap());
    if let Some(mut m) = w.get_mut::<Magazine>(gun) {
        m.clip = m.size;
        m.reserve = 200;
    }
    w.get_mut::<Inventory>(theirs).unwrap().wanted = Some(gun);
    sim.ticks(150);
}

/// What one app wrote of `M`, gathered after every step.
struct Log<M: Message> {
    /// 0: the server; i + 1: client i.
    app: usize,
    cursor: MessageCursor<M>,
    all: Vec<M>,
}

fn world_of(sim: &NetSim, app: usize) -> &World {
    if app == 0 { sim.server.app.world() } else { sim.clients[app - 1].app.world() }
}

impl<M: Message + Clone> Log<M> {
    fn new(sim: &NetSim, app: usize) -> Self {
        Self { app, cursor: world_of(sim, app).resource::<Messages<M>>().get_cursor_current(), all: Vec::new() }
    }

    fn poll(&mut self, sim: &NetSim) {
        let m = world_of(sim, self.app).resource::<Messages<M>>();
        self.all.extend(self.cursor.read(m).cloned());
    }
}

/// What the watching client saw of a burst.
struct Seen {
    /// The rounds the server fired.
    rounds: usize,
    received: Vec<FireBullets>,
    tracers: Vec<Tracer>,
    sounds: Vec<PlaySound>,
    events: Vec<WeaponEvent>,
}

/// Client 0 presses fire `presses` times (each held `hold` steps, then let
/// go for `gap`), client 1 watching.
fn burst(sim: &mut NetSim, presses: usize, hold: usize, gap: usize) -> Seen {
    burst_with(sim, presses, hold, gap, false)
}

/// `burst` with attack2 instead (`secondary`).
fn burst_with(sim: &mut NetSim, presses: usize, hold: usize, gap: usize, secondary: bool) -> Seen {
    let shooter = sim.character_of(0).unwrap();
    let mut fired = Log::<WeaponEvent>::new(sim, 0);
    let mut received = Log::<FireBullets>::new(sim, 2);
    let mut tracers = Log::<Tracer>::new(sim, 2);
    let mut sounds = Log::<PlaySound>::new(sim, 2);
    let mut events = Log::<WeaponEvent>::new(sim, 2);
    for _ in 0..presses {
        for k in 0..hold + gap {
            if secondary {
                intent(sim, 0).secondary = k < hold;
            } else {
                intent(sim, 0).fire = k < hold;
            }
            sim.step();
            fired.poll(sim);
            received.poll(sim);
            tracers.poll(sim);
            sounds.poll(sim);
            events.poll(sim);
        }
    }
    for _ in 0..30 {
        sim.step();
        received.poll(sim);
        tracers.poll(sim);
        sounds.poll(sim);
        events.poll(sim);
    }
    let watcher = sim.local_player(1);
    Seen {
        rounds: fired.all.iter().filter(|e| e.owner == shooter && e.shown() && matches!(e.kind, WeaponEventKind::Fired { .. })).count(),
        received: received.all,
        tracers: tracers.all.into_iter().filter(|t| Some(t.owner) != watcher).collect(),
        sounds: sounds.all,
        events: events.all.into_iter().filter(|e| Some(e.owner) != watcher).collect(),
    }
}

/// Spec 1-2, cases 18 and 26 on the watching client: the server's count
/// flags every 2nd SMG1 round, each flagged round draws its tracer, the
/// others none; no near-miss sound; the wall's impact sound plays.
#[test]
fn a_remote_smg1_draws_the_servers_tracers_and_its_impact_sounds() {
    let mut sim = joined();
    arm(&mut sim, weapons::SMG1);
    place(&mut sim, 0, Vec3::new(0.0, 1.0, 12.0), 0.0);
    place(&mut sim, 1, Vec3::new(6.0, 1.0, 12.0), 0.0);
    sim.ticks(40);
    let seen = burst(&mut sim, 1, 60, 10);
    println!("{} rounds, {} received, {} tracers", seen.rounds, seen.received.len(), seen.tracers.len());
    assert!(seen.rounds >= 6, "the SMG1 fired: {}", seen.rounds);
    assert_eq!(seen.received.len(), seen.rounds, "every round reached the watcher");
    assert!(seen.received.iter().all(|f| f.flags & shot_flags::FLAGGED != 0), "the server flags HL2:DM's rounds");
    let flagged = seen.received.iter().filter(|f| f.flags & shot_flags::TRACERS != 0).count();
    assert!(
        flagged.abs_diff(seen.rounds / 2) <= 1,
        "every 2nd round has a tracer by the server's count: {flagged} of {}",
        seen.rounds
    );
    assert_eq!(seen.tracers.len(), flagged, "a tracer for each flagged round's bullet");
    assert!(seen.tracers.iter().all(|t| t.look == TRACER));
    assert!(
        seen.events.iter().filter(|e| matches!(e.kind, WeaponEventKind::Redrawn { .. })).count() == seen.rounds,
        "each round traced again as flagged"
    );
    assert!(seen.sounds.iter().all(|s| !s.entry.contains("Nearmiss")), "remote tracers never whizz");
    let impacts = seen.sounds.iter().filter(|s| s.entry == "Default.BulletImpact").count();
    assert!(impacts >= seen.rounds - 1, "the wall's impact sound for each round: {impacts}");
}

/// Case 20: whatever the count, a shotgun shot's 7 pellets hold a
/// multiple of 4, so every shot is flagged and draws 7 tracers.
#[test]
fn every_remote_shotgun_shot_draws_seven_tracers_20() {
    let mut sim = joined();
    arm(&mut sim, weapons::SHOTGUN);
    place(&mut sim, 0, Vec3::new(0.0, 1.0, 12.0), 0.0);
    place(&mut sim, 1, Vec3::new(6.0, 1.0, 12.0), 0.0);
    sim.ticks(40);
    let seen = burst(&mut sim, 3, 3, 80);
    assert!(seen.rounds >= 2, "the shotgun fired: {}", seen.rounds);
    assert!(seen.received.iter().all(|f| f.flags & shot_flags::TRACERS != 0), "every shot flagged");
    assert_eq!(seen.tracers.len(), 7 * seen.received.len(), "7 tracers a shot");
}

/// The AR2's remote rounds draw its own streak.
#[test]
fn a_remote_ar2_draws_its_own_streak() {
    let mut sim = joined();
    arm(&mut sim, weapons::AR2);
    place(&mut sim, 0, Vec3::new(0.0, 1.0, 12.0), 0.0);
    place(&mut sim, 1, Vec3::new(6.0, 1.0, 12.0), 0.0);
    sim.ticks(40);
    let seen = burst(&mut sim, 1, 30, 10);
    assert!(!seen.tracers.is_empty());
    assert!(seen.tracers.iter().all(|t| t.look == AR2_TRACER));
}

/// Spec 1 and Quirks ("Remote impacts ignore players"): shot at, the
/// watcher's copy of the round passes through it and its own body and
/// ends on what is behind, which sounds as itself.
#[test]
fn a_remote_round_is_traced_again_through_players() {
    let mut sim = joined();
    arm(&mut sim, weapons::PISTOL);
    // The shooter at the south spawn looking north, the watcher in
    // front of it in the line of fire.
    place(&mut sim, 0, Vec3::new(0.0, 1.0, 12.0), 0.0);
    place(&mut sim, 1, Vec3::new(0.0, 1.0, 4.0), std::f32::consts::PI);
    sim.ticks(40);
    let seen = burst(&mut sim, 2, 2, 40);
    let shots: Vec<(Vec3, Option<Entity>)> = seen
        .events
        .iter()
        .filter_map(|e| match e.kind {
            WeaponEventKind::Shot { to, hit, .. } => Some((to, hit)),
            _ => None,
        })
        .collect();
    assert!(!shots.is_empty(), "the watcher traced the rounds again");
    let w = sim.clients[1].app.world_mut();
    let mut characters = w.query_filtered::<Entity, With<Intent>>();
    let characters: Vec<Entity> = characters.iter(w).collect();
    for (to, hit) in &shots {
        assert!(hit.is_none_or(|h| !characters.contains(&h)), "no player hit on the watcher's copy");
        assert!(to.z < 3.0, "the round ends behind the watcher (z 4): {to}");
    }
    assert!(seen.sounds.iter().any(|s| s.entry == "Default.BulletImpact"), "the wall sounds");
}

/// Weapons.md 7.4 and effects_sounds.md 1: the watcher draws a double
/// shot's 12 pellets (each with its tracer), not a single shot's 7.
#[test]
fn a_remote_double_shot_draws_twelve_pellets() {
    let mut sim = joined();
    arm(&mut sim, weapons::SHOTGUN);
    place(&mut sim, 0, Vec3::new(0.0, 1.0, 12.0), 0.0);
    place(&mut sim, 1, Vec3::new(6.0, 1.0, 12.0), 0.0);
    sim.ticks(40);
    let seen = burst_with(&mut sim, 2, 3, 90, true);
    assert!(seen.rounds >= 1, "the shotgun fired: {}", seen.rounds);
    assert!(seen.received.iter().all(|f| f.flags & shot_flags::TRACERS != 0), "every double shot flagged");
    assert_eq!(seen.tracers.len(), 12 * seen.received.len(), "12 tracers a double shot");
}

/// Weapons.md 12: a shot rewinds only the players within 1.5 x their max
/// speed x `sv_maxunlag` of the shooter or within 45 degrees of its view;
/// a shot with attack up (attack2's double shot, no shot in the last 5
/// commands) rewinds nobody.
#[test]
fn lag_compensation_picks_whom_to_rewind_by_distance_and_view() {
    use chimera_sim::weapon::lagcomp::LagCompStats;
    let mut sim = joined();
    arm(&mut sim, weapons::PISTOL);
    let moved = |sim: &mut NetSim, at: Vec3| {
        place(sim, 0, Vec3::new(0.0, 1.0, 12.0), 0.0);
        place(sim, 1, at, 0.0);
        sim.ticks(60);
        let before = sim.server.app.world().resource::<LagCompStats>().rewinds;
        burst(sim, 1, 2, 40);
        let stats = sim.server.app.world().resource::<LagCompStats>();
        assert!(stats.rewinds > before, "a rewind for the shot at {at}");
        stats.last.as_ref().unwrap().moved
    };
    // 15 m behind (out of view, farther than 1.5 x 190 units/s x 1 s).
    assert_eq!(moved(&mut sim, Vec3::new(0.0, 1.0, 27.0)), 0);
    // 5 m behind: near enough.
    assert_eq!(moved(&mut sim, Vec3::new(0.0, 1.0, 17.0)), 1);
    // 15 m ahead: in view.
    assert_eq!(moved(&mut sim, Vec3::new(2.0, 1.0, -3.0)), 1);
    // Attack2's double shot alone: no rewind at all.
    arm(&mut sim, weapons::SHOTGUN);
    place(&mut sim, 0, Vec3::new(0.0, 1.0, 12.0), 0.0);
    place(&mut sim, 1, Vec3::new(2.0, 1.0, -3.0), 0.0);
    sim.ticks(60);
    let before = sim.server.app.world().resource::<LagCompStats>().rewinds;
    let seen = burst_with(&mut sim, 1, 3, 40, true);
    assert!(seen.rounds >= 1, "the double shot fired");
    assert_eq!(sim.server.app.world().resource::<LagCompStats>().rewinds, before, "not rewound");
}
