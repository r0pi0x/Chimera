//! Combat Arms' bots (specs/combat_arms/bots.md): its numbered test cases
//! 1-44 on `CaSim` with Team Death Match on (`harness::with_mode`), on the
//! spec's common setup: a flat open floor (one 400 m square, the
//! navigation areas generated over it), the player standing still, the
//! bot on its mid primary. Where the spec fixes a random draw, the case
//! checks the formula with that draw and the scenario with the draw's
//! range. Cases about things the game doesn't have yet (placed mines:
//! 32's mine, 36, 42) check the rule the bot would use.

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::{
    console::{Console, run_queue},
    core::{Damage, DamageFilter, DamageKind, Dead, Health, Hitgroup, SimClock, SpawnPoint, Team},
};
use chimera_sim::weapon::{Inventory, Magazine, OwnKit, Weapon, WeaponEvent, WeaponEventKind, grenade::Throwable};
use chimera_world::{
    harness::Sim,
    map::{
        MapEntities, MapEntity, SeeThrough,
        nav::{NavArea, NavMesh, Side, Via},
    },
};
use combat_arms::{
    game::{
        bots::{
            self, BotSettings, BotState, CaBot, Level, Template,
            brain::{self, CaCamping, Kind, Stimuli},
            data::{BotData, ItemSet, SlotList},
            map_ai::MapAi,
            math::{self, Choice, HitCycle, Move, Tracking},
            nav as bot_nav, roster,
        },
        grenades::{self, CaGrenade, M18, M67},
        inventory::{KNIFE, KnifeReach, UNIT},
        rules::{CaMatch, Phase, spawns::CaProtected},
        weapons::RIFLE,
    },
    harness::{CaSim, with_mode},
};

const ALPHA: u8 = 1;
const BRAVO: u8 = 2;
const PISTOL: &str = "combat_arms:beretta_m92fs";
const SNIPER: &str = "combat_arms:l96a1";

/// A flat open floor, 400 m square, top at y = 0, and four start points
/// a team.
struct OpenFloor;

impl Plugin for OpenFloor {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, |mut commands: Commands| {
            commands.spawn((
                Name::new("Floor"),
                RigidBody::Static,
                Collider::cuboid(400.0, 1.0, 400.0),
                Transform::from_xyz(0.0, -0.5, 0.0),
            ));
            for (k, team) in [ALPHA, BRAVO].into_iter().enumerate() {
                for i in 0..4 {
                    let z = if team == ALPHA { 100.0 } else { -100.0 };
                    commands.spawn((
                        Name::new(format!("Spawn {}", k * 4 + i)),
                        SpawnPoint { team: Some(Team(team)) },
                        Transform::from_xyz(-6.0 + 4.0 * i as f32, 0.0, z),
                    ));
                }
            }
        });
    }
}

/// The open floor playing Team Death Match.
fn world() -> Sim {
    let mut sim = Sim::new(CaSim, OpenFloor);
    with_mode(sim.app.world_mut(), "TDM");
    sim
}

fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

/// A wall (a static box) from `min` to `max` (m); the navigation areas
/// made again with it.
fn wall(sim: &mut Sim, min: Vec3, max: Vec3) {
    sim.app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(max.x - min.x, max.y - min.y, max.z - min.z),
        Transform::from_translation((min + max) * 0.5),
    ));
    sim.ticks(1);
    sim.app.world_mut().remove_resource::<NavMesh>();
    sim.ticks(2);
}

/// A human player on `team`, made tough (it should live through a case).
fn human(sim: &mut Sim, team: u8) -> Entity {
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.app.world_mut().entity_mut(p).insert(Team(team));
    p
}

/// Run until the match is live.
fn live(sim: &mut Sim) {
    for _ in 0..200 {
        if sim.app.world().resource::<CaMatch>().phase == Phase::Live {
            return;
        }
        sim.ticks(1);
    }
    panic!("not live");
}

/// A bot of `team` at `level` (archetype `arch`), its kit `kit` (slots
/// mid, long, secondary, melee, grenade, mine), on the floor.
fn bot(sim: &mut Sim, team: u8, level: Level, arch: u8, kit: [Option<&'static str>; 6]) -> Entity {
    let w = sim.app.world_mut();
    let b = bots::add_bot(w, Some(team)).unwrap();
    set_kit(sim, b, level, arch, kit);
    b
}

/// Give bot `b` a template and kit, its inventory rebuilt next tick.
fn set_kit(sim: &mut Sim, b: Entity, level: Level, arch: u8, kit: [Option<&'static str>; 6]) {
    let w = sim.app.world_mut();
    let data = w.resource::<BotData>().clone();
    let weapons = w.get::<Inventory>(b).map(|i| i.weapons.clone()).unwrap_or_default();
    for x in weapons {
        w.despawn(x);
    }
    w.entity_mut(b).remove::<Inventory>();
    w.entity_mut(b).insert(OwnKit(kit.iter().flatten().copied().collect()));
    let mut bot = w.get_mut::<CaBot>(b).unwrap();
    bot.template = Template::new(level, arch);
    bot.kit = kit;
    bot.armed = false;
    bot.reach_fixed = false;
    bot.grenades = data.template.grenade_shots.at(bot.template) as u32;
}

const RIFLE_KIT: [Option<&str>; 6] = [Some(RIFLE), None, None, Some(KNIFE), None, None];

/// Put character `e` with its feet at `feet`, facing `yaw`.
fn place(sim: &mut Sim, e: Entity, feet: Vec3, yaw: f32) {
    let w = sim.app.world_mut();
    w.get_mut::<Transform>(e).unwrap().translation = feet + Vec3::Y * 0.9;
    w.get_mut::<chimera_core::core::Velocity>(e).unwrap().0 = Vec3::ZERO;
    let mut i = w.get_mut::<chimera_core::core::Intent>(e).unwrap();
    i.yaw = yaw;
    i.pitch = 0.0;
}

fn b(sim: &Sim, e: Entity) -> CaBot {
    sim.app.world().get::<CaBot>(e).unwrap().clone()
}

fn b_mut(sim: &mut Sim, e: Entity) -> Mut<'_, CaBot> {
    sim.app.world_mut().get_mut::<CaBot>(e).unwrap()
}

fn console(sim: &mut Sim, line: &str) {
    let w = sim.app.world_mut();
    w.resource_mut::<Console>().submit(line);
    run_queue(w);
}

/// Yaw facing +Z (yaw 0 faces −Z).
const FACE_PLUS_Z: f32 = std::f32::consts::PI;

fn u(units: f32) -> f32 {
    units * UNIT
}

/// The bots stand still (they still sense, aim and fire).
fn stop(sim: &mut Sim) {
    sim.app.world_mut().resource_mut::<BotSettings>().stop = 1;
}

/// A setup for a duel: the open floor live, a tough human on Alpha at the
/// origin, a bot on Bravo at `d` u along +Z facing it, the bots standing
/// still. Past the first 5 s of server time (the re-arm edge case).
fn duel(level: Level, arch: u8, d: f32, kit: [Option<&'static str>; 6]) -> (Sim, Entity, Entity) {
    let mut sim = world();
    let p = human(&mut sim, ALPHA);
    live(&mut sim);
    let bt = bot(&mut sim, BRAVO, level, arch, kit);
    stop(&mut sim);
    sim.seconds(5.5);
    tough(&mut sim, p);
    place(&mut sim, p, Vec3::ZERO, FACE_PLUS_Z);
    place(&mut sim, bt, Vec3::new(0.0, 0.0, u(d)), 0.0);
    (sim, p, bt)
}

fn tough(sim: &mut Sim, e: Entity) {
    *sim.app.world_mut().get_mut::<Health>(e).unwrap() = Health {
        current: 1000.0,
        max: 1000.0,
    };
}

/// Run until `f` holds (at most `secs`), returning the time it took.
fn until(sim: &mut Sim, secs: f64, mut f: impl FnMut(&mut Sim) -> bool) -> Option<f64> {
    let start = now(sim);
    while now(sim) - start < secs {
        if f(sim) {
            return Some(now(sim) - start);
        }
        sim.ticks(1);
    }
    None
}

// ---------------------------------------------------------------------------
// Roster

/// Case 1: player count 6, the human on Alpha: 6 bots, all Bravo, six
/// distinct profiles of the level.
#[test]
fn case_01_roster_size_and_team() {
    let mut sim = world();
    let _p = human(&mut sim, ALPHA);
    live(&mut sim);
    console(&mut sim, "BotLevel 0");
    console(&mut sim, "MaxPlayer 6");
    sim.ticks(10);
    let w = sim.app.world_mut();
    let all: Vec<(u8, usize, Template)> =
        w.query::<(&Team, &CaBot)>().iter(w).map(|(t, b)| (t.0, b.profile, b.template)).collect();
    assert_eq!(all.len(), 6);
    assert!(all.iter().all(|(t, _, _)| *t == BRAVO));
    let mut profiles: Vec<usize> = all.iter().map(|a| a.1).collect();
    profiles.sort();
    profiles.dedup();
    assert_eq!(profiles.len(), 6);
    assert!(all.iter().all(|a| a.2.level == Level::Easy && a.1 < 22));
}

/// Case 2: Easy, ρ = 45, profile 1 used: 45 mod 22 = 1 taken, 2 used.
#[test]
fn case_02_profile_probing() {
    let sim = world();
    let count = sim.app.world().resource::<BotData>().profiles.count(Level::Easy);
    assert_eq!(count, 22);
    let mut used = vec![1];
    assert_eq!(math::pick_profile(45, count, &mut used), 2);
    assert_eq!(used, vec![1, 2]);
}

/// Case 3: the first partial slot (grenade, 45 %) with σ = 1234: 34 < 45,
/// given; σ becomes 123.
#[test]
fn case_03_partial_slot_digits() {
    let _sim = world();
    let slot = SlotList {
        percent: 45.0,
        items: vec![M67.into()],
    };
    let (item, sigma) = math::fill_slot(&slot, 1234, 1234);
    assert_eq!(item.as_deref(), Some(M67));
    assert_eq!(sigma, 123);
    // Through a whole set: full slots don't use a digit.
    let set = ItemSet {
        level: "easy".into(),
        mid: SlotList {
            percent: 100.0,
            items: vec![RIFLE.into()],
        },
        grenade: slot.clone(),
        mine: SlotList {
            percent: 50.0,
            items: vec!["mine".into()],
        },
        ..Default::default()
    };
    let got = math::fill_set(&set, 1234);
    assert_eq!(got[4].as_deref(), Some(RIFLE));
    assert_eq!(got[8].as_deref(), Some(M67));
    // σ = 123 for the mine: 23 < 50, given.
    assert_eq!(got[9].as_deref(), Some("mine"));
}

/// Case 4: mid and secondary lists of 7 each: the second gets 8 (its first
/// entry repeated).
#[test]
fn case_04_list_padding() {
    let _sim = world();
    let list = |n: usize, p: &str| SlotList {
        percent: 100.0,
        items: (0..n).map(|i| format!("{p}{i}")).collect(),
    };
    let mut set = ItemSet {
        level: "easy".into(),
        mid: list(7, "m"),
        secondary: list(7, "s"),
        ..Default::default()
    };
    set.pad();
    assert_eq!(set.mid.items.len(), 7);
    assert_eq!(set.secondary.items.len(), 8);
    assert_eq!(set.secondary.items[7], "s0");
}

// ---------------------------------------------------------------------------
// Reaction

/// The reaction delay a bot rolled when its attack began: (fire_at −
/// activation).
fn reaction(sim: &mut Sim, bt: Entity) -> f64 {
    until(sim, 3.0, |s| matches!(b(s, bt).state, BotState::Attack { .. })).expect("attacks");
    let bot = b(sim, bt);
    let BotState::Attack { since } = bot.state else { unreachable!() };
    bot.fire_at - since
}

/// Case 5: NN, target in view at 800 u, the goal never active before:
/// with draw 0.5 the first shot waits 0.75 s; any draw: 0.6-0.9 s.
#[test]
fn case_05_reaction_easy() {
    assert!((math::reaction_delay(0.6, 0.9, 0.0, 0.5) - 0.75).abs() < 1e-6);
    let (mut sim, _p, bt) = duel(Level::Easy, 0, 800.0, RIFLE_KIT);
    let r = reaction(&mut sim, bt);
    assert!((0.6..=0.9 + 1e-6).contains(&r), "{r}");
}

/// Case 6: XE: draw 0 → 0.1 s, draw 1 → 0.3 s.
#[test]
fn case_06_reaction_veteran() {
    assert!((math::reaction_delay(0.1, 0.3, 0.0, 0.0) - 0.1).abs() < 1e-6);
    assert!((math::reaction_delay(0.1, 0.3, 0.0, 1.0) - 0.3).abs() < 1e-6);
    let (mut sim, _p, bt) = duel(Level::Veteran, 2, 800.0, RIFLE_KIT);
    let r = reaction(&mut sim, bt);
    assert!((0.1..=0.3 + 1e-6).contains(&r), "{r}");
}

/// Case 7: NN, the player camping 20 s: t_max' = 0.5 < 0.6, exactly 0.5 s.
#[test]
fn case_07_reaction_with_camping() {
    assert!((math::reaction_delay(0.6, 0.9, 20.0, 0.3) - 0.5).abs() < 1e-6);
    let (mut sim, p, bt) = duel(Level::Easy, 0, 800.0, RIFLE_KIT);
    sim.app.world_mut().entity_mut(p).insert(CaCamping {
        anchor: Vec3::new(0.0, 0.9, 0.0),
        time: 20.0,
    });
    let r = reaction(&mut sim, bt);
    assert!((r - 0.5).abs() < 0.03, "{r}");
}

/// Case 8: XE camping 20 s: t_max' = 0, no delay.
#[test]
fn case_08_reaction_floor() {
    assert_eq!(math::reaction_delay(0.1, 0.3, 20.0, 0.7), 0.0);
    let (mut sim, p, bt) = duel(Level::Veteran, 2, 800.0, RIFLE_KIT);
    sim.app.world_mut().entity_mut(p).insert(CaCamping {
        anchor: Vec3::new(0.0, 0.9, 0.0),
        time: 20.0,
    });
    let r = reaction(&mut sim, bt);
    assert!(r.abs() < 0.03, "{r}");
}

/// Case 9: the goal ended 3 s ago: no new delay; 5.1 s ago: a new one.
#[test]
fn case_09_no_rearm() {
    for (gap, rolled) in [(3.0, false), (5.1, true)] {
        let (mut sim, _p, bt) = duel(Level::Easy, 0, 800.0, RIFLE_KIT);
        let t = now(&sim);
        {
            let mut bot = b_mut(&mut sim, bt);
            bot.attack_off = t - gap;
            bot.fire_at = t - gap - 1.0;
        }
        let r = reaction(&mut sim, bt);
        if rolled {
            assert!((0.6..=0.9 + 1e-6).contains(&r), "{gap}: {r}");
        } else {
            assert!(r < 0.0, "{gap}: fires at once ({r})");
        }
    }
}

// ---------------------------------------------------------------------------
// Hit cycle

/// Count aimed shots per cycle from a reset over many cycles.
fn aimed_per_cycle(a: f32, seed: u64) -> Vec<u32> {
    let (h, m) = math::cycle(a);
    let mut c = HitCycle::default();
    let mut dice = math::Dice::new(seed);
    (0..50)
        .map(|_| (0..h + m).filter(|_| c.shot(a, dice.r100())).count() as u32)
        .collect()
}

/// Case 10: a = 0.17: m = 5, h = 1: one aimed shot in every 6.
#[test]
fn case_10_hit_cycle_easy_rifle() {
    let sim = world();
    let a = sim.app.world().resource::<BotData>().gun(RIFLE, Template::new(Level::Easy, 0)).unwrap().a;
    assert_eq!(a, 0.17);
    assert_eq!(math::cycle(a), (1, 5));
    assert!(aimed_per_cycle(a, 7).iter().all(|&n| n == 1));
    // And from the bot's shots in play: the misses land beside the target.
    let (mut sim, p, bt) = duel(Level::Easy, 0, 800.0, RIFLE_KIT);
    let mut offsets = Vec::new();
    until(&mut sim, 8.0, |s| {
        let bot = b(s, bt);
        if let Some(o) = bot.aim_offset
            && offsets.last() != Some(&o)
        {
            offsets.push(o);
        }
        offsets.len() >= 13
    });
    let _ = p;
    let wide = offsets.iter().filter(|o| o.xz().length() > 90.0).count();
    let aimed = offsets.iter().filter(|o| o.length() < 5.0).count();
    assert!(offsets.len() >= 12, "{offsets:?}");
    assert!(wide >= aimed * 3, "aimed {aimed}, wide {wide}: {offsets:?}");
}

/// Case 11: a = 0.46: h = m = 1, one aimed shot per pair.
#[test]
fn case_11_hit_cycle_veteran_rifle() {
    let sim = world();
    let a = sim.app.world().resource::<BotData>().gun(RIFLE, Template::new(Level::Veteran, 2)).unwrap().a;
    assert_eq!(math::cycle(a), (1, 1));
    assert!(aimed_per_cycle(a, 3).iter().all(|&n| n == 1));
}

/// Case 12: a = 0.17, counters 0/0, draws 10 then 90: hit, then miss.
#[test]
fn case_12_hit_cycle_with_draws() {
    let _sim = world();
    let mut c = HitCycle::default();
    assert!(c.shot(0.17, 10));
    assert_eq!(c.hits, 1);
    assert!(!c.shot(0.17, 90));
}

/// Case 13: the pistol, a = 0.8: 4 aimed of 5.
#[test]
fn case_13_pistol() {
    let sim = world();
    let a = sim.app.world().resource::<BotData>().gun(PISTOL, Template::new(Level::Easy, 0)).unwrap().a;
    assert_eq!(math::cycle(a), (4, 1));
    assert!(aimed_per_cycle(a, 11).iter().all(|&n| n == 4));
}

/// Case 14: XE rifle, the player's centre hidden by low cover, the head
/// clear: head only, a = 0.23 → 1 of 4, aimed at the head.
#[test]
fn case_14_head_only() {
    let (mut sim, p, bt) = duel(Level::Veteran, 2, 800.0, RIFLE_KIT);
    // A wall 1 m in front of the player, 1.2 m high: over the centre
    // (0.9 m), under the head (1.5 m).
    wall(&mut sim, Vec3::new(-3.0, 0.0, 0.8), Vec3::new(3.0, 1.2, 1.1));
    sim.ticks(2);
    assert_eq!(brain::sees(sim.app.world_mut(), bt, p), Some(true));
    let a = math::accuracy(0.46, 0.0, true);
    assert!((a - 0.23).abs() < 1e-6);
    assert_eq!(math::cycle(a), (1, 3));
    until(&mut sim, 3.0, |s| matches!(b(s, bt).state, BotState::Attack { .. })).unwrap();
    sim.seconds(0.2);
    let bot = b(&sim, bt);
    assert!(bot.head_only);
}

/// Case 15: NN rifle, the player camping 30 s: a = 0.47 → 1 of 2.
#[test]
fn case_15_camping_accuracy() {
    let _sim = world();
    let a = math::accuracy(0.17, 30.0, false);
    assert!((a - 0.47).abs() < 1e-6);
    assert_eq!(math::cycle(a), (1, 1));
}

/// Case 16: a miss from (0, 0, 0) at a torso at (0, 50, 1000) with the
/// vertical draw 15: (∓99.9, 65, 1000), the same sideways at 100 or 3000 u.
#[test]
fn case_16_miss_geometry() {
    let _sim = world();
    for z in [100.0f32, 1000.0, 3000.0] {
        let base = Vec3::new(0.0, 50.0 * z / 1000.0, z);
        for right in [true, false] {
            let s = math::miss_point(Vec3::ZERO, base, 100.0, right, 15.0);
            let side = (s - base).xz().length();
            assert!((side - 99.875).abs() < 0.05, "{z}: {side}");
            assert!((s.y - base.y - 15.0).abs() < 1e-3);
        }
    }
    let s = math::miss_point(Vec3::ZERO, Vec3::new(0.0, 50.0, 1000.0), 100.0, true, 15.0);
    assert!((s.x.abs() - 99.875).abs() < 0.01 && (s.y - 65.0).abs() < 1e-3 && (s.z - 1000.0).abs() < 1e-3);
}

/// Case 17: the rifle at 9 u: exact; at 11 u: the cycle applies.
#[test]
fn case_17_full_accuracy_zone() {
    let sim = world();
    let v = sim.app.world().resource::<BotData>().gun(RIFLE, Template::new(Level::Easy, 0)).unwrap().full_accuracy;
    assert!(math::full_accuracy(9.0, v));
    assert!(!math::full_accuracy(11.0, v));
}

/// Case 18: XE sniper (c_max 0.48, k 0.03), a straight line at constant
/// speed, 0.05 s updates: −0.0015 each, 0 after 16 s; a reversal snaps it
/// back to 0.48.
#[test]
fn case_18_movement_inaccuracy() {
    let sim = world();
    let g = sim.app.world().resource::<BotData>().gun(SNIPER, Template::new(Level::Veteran, 2)).unwrap();
    assert_eq!((g.c_max, g.decay), (0.48, 0.03));
    let mut t = Tracking::default();
    t.select(g.c_max);
    let v = Vec3::new(150.0, 0.0, 0.0);
    let mut p = Vec3::ZERO;
    t.update(p, 0.05, g.c_max, g.decay);
    p += v * 0.05;
    t.update(p, 0.05, g.c_max, g.decay);
    let before = t.c;
    p += v * 0.05;
    t.update(p, 0.05, g.c_max, g.decay);
    assert!((before - t.c - 0.0015).abs() < 1e-5, "{before} {}", t.c);
    for _ in 0..330 {
        p += v * 0.05;
        t.update(p, 0.05, g.c_max, g.decay);
    }
    assert!(t.c < 1e-4, "{}", t.c);
    p -= v * 0.05;
    t.update(p, 0.05, g.c_max, g.decay);
    assert_eq!(t.c, 0.48);
    // Aimed offsets at most ±48 u horizontally, ±24 u vertically.
    let hi = math::hit_point(Vec3::ZERO, t.c, Vec3::ONE);
    assert!((hi - Vec3::new(48.0, 24.0, 48.0)).length() < 1e-3);
}

// ---------------------------------------------------------------------------
// Noticing

/// Case 19: NN patrolling, the player at 1550 u in view: P = 55 (56 %),
/// the next roll no sooner than 1.5 s later.
#[test]
fn case_19_notice_roll_easy() {
    let p = math::notice_percent(1550.0, 700.0, 2400.0, 10.0, 0.0);
    assert!((p - 55.0).abs() < 1e-4);
    assert!(math::roll(p, 55) && !math::roll(p, 56));
    let (mut sim, _p, bt) = duel(Level::Easy, 0, 1550.0, RIFLE_KIT);
    // The first pass rolls (or sees): the cooldown runs 1.5 s.
    sim.seconds(0.06);
    let bot = b(&sim, bt);
    if bot.state.relaxed() {
        assert!((bot.sight_cooldown - now(&sim)).abs() <= 1.5 + 1e-6 && bot.sight_cooldown > now(&sim));
    }
}

/// Case 20: XE at 3000 u: P ≈ 52.4 → 53 %.
#[test]
fn case_20_notice_roll_veteran() {
    let _sim = world();
    let p = math::notice_percent(3000.0, 700.0, 3600.0, 40.0, 0.0);
    assert!((p - 52.41).abs() < 0.01);
    assert_eq!((0..100).filter(|&r| math::roll(p, r)).count(), 53);
}

/// Case 21: at 650 u: always seen (the first pass); at 3001 u: never.
#[test]
fn case_21_notice_roll_near() {
    let (mut sim, _p, bt) = duel(Level::Easy, 0, 650.0, RIFLE_KIT);
    sim.seconds(0.1);
    assert!(b(&sim, bt).target.is_some());
    assert!((0..100).all(|r| math::roll(math::notice_percent(650.0, 700.0, 2400.0, 10.0, 0.0), r)));
    let (mut sim, p, bt) = duel(Level::Easy, 0, 3001.0, RIFLE_KIT);
    assert_eq!(brain::sees(sim.app.world_mut(), bt, p), None);
    sim.seconds(4.0);
    assert!(b(&sim, bt).target.is_none());
}

/// Case 22: a relaxed bot facing +Z: 75° off at 1000 u not seen, at 200 u
/// seen; alert: 100° not, 89° seen.
#[test]
fn case_22_fov() {
    let (mut sim, p, bt) = duel(Level::Easy, 0, 0.0, RIFLE_KIT);
    let at = |deg: f32, d: f32| {
        let a = deg.to_radians();
        Vec3::new(a.sin() * u(d), 0.0, a.cos() * u(d))
    };
    place(&mut sim, bt, Vec3::ZERO, FACE_PLUS_Z);
    for (deg, d, alert, seen) in [
        (75.0, 1000.0, false, false),
        (0.0, 1000.0, false, true),
        (75.0, 200.0, false, true),
        (100.0, 1000.0, true, false),
        (89.0, 1000.0, true, true),
    ] {
        place(&mut sim, p, at(deg, d), 0.0);
        b_mut(&mut sim, bt).state = if alert {
            BotState::Chase { until: 1e9, breath: 0.0 }
        } else {
            BotState::Patrol
        };
        sim.app.world_mut().get_mut::<Transform>(bt).unwrap().translation = Vec3::Y * 0.9;
        assert_eq!(brain::sees(sim.app.world_mut(), bt, p).is_some(), seen, "{deg}° at {d} u, alert {alert}");
    }
}

/// Case 23: a smoke cloud between bot and player at 800 u: seen and
/// engaged as without it.
#[test]
fn case_23_smoke() {
    let (mut sim, p, bt) = duel(Level::Easy, 0, 800.0, RIFLE_KIT);
    sim.app.world_mut().resource_mut::<BotSettings>().dont_shoot = 1;
    // An M18 thrown by a third party, landing between them.
    let thrower = human(&mut sim, BRAVO);
    place(&mut sim, thrower, Vec3::new(5.0, 0.0, u(400.0)), 0.0);
    chimera_sim::weapon::give(sim.app.world_mut(), thrower, M18);
    sim.ticks(2);
    let w = sim.app.world().get::<Inventory>(thrower).unwrap().weapons.clone();
    let smoke = w.into_iter().find(|&x| sim.app.world().get::<Weapon>(x).unwrap().id == M18).unwrap();
    grenades::spawn_grenade(
        sim.app.world_mut(),
        smoke,
        Some(thrower),
        Vec3::new(0.0, 0.3, u(400.0)),
        Vec3::ZERO,
    );
    sim.seconds(3.5);
    assert!(brain::sees(sim.app.world_mut(), bt, p).is_some());
    until(&mut sim, 3.0, |s| matches!(b(s, bt).state, BotState::Attack { .. }) && b(s, bt).target == Some(p))
        .expect("engaged through the smoke");
}

/// Case 24: a teammate standing in the line of fire: no shot; the
/// teammate, more than 48 u away, is asked to crouch.
#[test]
fn case_24_teammate_in_line() {
    let (mut sim, p, b1) = duel(Level::Easy, 0, 800.0, RIFLE_KIT);
    let b2 = bot(&mut sim, BRAVO, Level::Easy, 0, RIFLE_KIT);
    sim.app.world_mut().get_mut::<CaBot>(b2).unwrap().state = BotState::Patrol;
    sim.ticks(3);
    place(&mut sim, b2, Vec3::new(0.0, 0.0, u(400.0)), 0.0);
    let mut fired = false;
    let start = sim.app.world().get::<Inventory>(b1).and_then(|i| i.active);
    until(&mut sim, 3.0, |s| {
        // Keep the teammate standing on the line.
        place(s, b2, Vec3::new(0.0, 0.0, u(400.0)), 0.0);
        let w = s.app.world_mut();
        let events: Vec<WeaponEvent> = w.resource_mut::<Messages<WeaponEvent>>().drain().collect();
        fired |= events.iter().any(|e| e.owner == b1 && matches!(e.kind, WeaponEventKind::Fired { .. }));
        false
    });
    let _ = (p, start);
    assert!(!fired, "fired through a teammate");
    assert!(b(&sim, b2).crouch_until > now(&sim), "the teammate was asked to crouch");
}

// ---------------------------------------------------------------------------
// Hearing

/// Case 25: a patrolling bot 900 u from a player firing an M4A1 (radius
/// 2000): the investigate goal fires 10-30 passes after the shot is heard
/// and the bot runs toward the shot.
#[test]
fn case_25_hearing_a_shot() {
    let (mut sim, p, bt) = duel(Level::Easy, 0, 900.0, RIFLE_KIT);
    // Facing away: it can't see the player.
    place(&mut sim, bt, Vec3::new(0.0, 0.0, u(900.0)), FACE_PLUS_Z);
    sim.ticks(2);
    let start = now(&sim);
    let heard = until(&mut sim, 6.0, |s| {
        s.intent(p).fire = true;
        s.intent(p).yaw = std::f32::consts::FRAC_PI_2;
        matches!(b(s, bt).state, BotState::Investigate { .. })
    });
    let t = heard.expect("investigates");
    assert!(t >= 0.5 && now(&sim) - start >= 0.5, "after the reaction delay: {t}");
    let BotState::Investigate { spot, .. } = b(&sim, bt).state else { unreachable!() };
    assert!(spot.xz().distance(Vec2::ZERO) < 1.5, "toward the shot: {spot}");
    // It turns to the shot and goes (or, seeing the player on the way,
    // fights).
    let before = sim.position(bt);
    sim.intent(p).fire = false;
    sim.app.world_mut().resource_mut::<BotSettings>().stop = 0;
    sim.seconds(1.0);
    let bot = b(&sim, bt);
    assert!(
        sim.position(bt).distance(Vec3::ZERO) < before.distance(Vec3::ZERO) - 1.0
            || (matches!(bot.state, BotState::Attack { .. }) && bot.target == Some(p)),
        "runs toward it: {:?}",
        bot.state
    );
}

/// Case 26: a relaxed bot, steps every 0.4 s at 300 u: two don't fire the
/// sense, the third heard one does.
#[test]
fn case_26_footsteps() {
    let (mut sim, p, bt) = duel(Level::Easy, 0, 300.0, RIFLE_KIT);
    place(&mut sim, bt, Vec3::new(0.0, 0.0, u(300.0)), FACE_PLUS_Z);
    // A step, then 0.4 s watching: whether the sense fired.
    let step = |sim: &mut Sim| {
        let t = now(sim);
        sim.app.world_mut().resource_mut::<Stimuli>().post(Kind::Footstep, Vec3::Y * 0.9, 256.0, p, ALPHA, t);
        until(sim, 0.4, |s| !b(s, bt).state.relaxed() || matches!(b(s, bt).state, BotState::Investigate { .. }))
            .is_some()
    };
    // Both heard (the roll at 300 u is 89 %), or one missed: either way no
    // investigation yet.
    assert!(!step(&mut sim) && !step(&mut sim), "fired before the third step");
    let heard = b(&sim, bt).footsteps.level;
    assert!(heard > 0.0 && heard < 1.0, "{heard}");
    for _ in 0..6 {
        if step(&mut sim) {
            return;
        }
    }
    panic!("never investigated");
}

// ---------------------------------------------------------------------------
// Memory

/// Case 27: lost from sight, the player moving: the chase's path ends at
/// the player's position ± 96 u, re-planned as they move; 12 s after the
/// pursuit began, still unseen, it gives up and patrols.
#[test]
fn case_27_chase_knows() {
    let (mut sim, p, bt) = duel(Level::Easy, 0, 600.0, RIFLE_KIT);
    sim.app.world_mut().resource_mut::<BotSettings>().stop = 0;
    // The player shut in a box (no path in, no sight).
    let c = Vec3::new(20.0, 0.0, 0.0);
    wall(&mut sim, c + Vec3::new(-3.0, 0.0, -3.0), c + Vec3::new(3.0, 3.0, -2.5));
    wall(&mut sim, c + Vec3::new(-3.0, 0.0, 2.5), c + Vec3::new(3.0, 3.0, 3.0));
    wall(&mut sim, c + Vec3::new(-3.0, 0.0, -3.0), c + Vec3::new(-2.5, 3.0, 3.0));
    wall(&mut sim, c + Vec3::new(2.5, 0.0, -3.0), c + Vec3::new(3.0, 3.0, 3.0));
    wall(&mut sim, c + Vec3::new(-3.0, 3.0, -3.0), c + Vec3::new(3.0, 3.5, 3.0));
    place(&mut sim, p, c, 0.0);
    sim.ticks(2);
    let t0 = now(&sim);
    {
        let mut bot = b_mut(&mut sim, bt);
        bot.target = Some(p);
        bot.visible = false;
        bot.route.clear();
        bot.route_goal = None;
        bot.state = BotState::Chase {
            until: t0 + 12.0,
            breath: 0.0,
        };
    }
    sim.seconds(0.5);
    let goal = b(&sim, bt).route_goal.expect("a chase path");
    let off = goal.xz() - c.xz();
    assert!((off.length() - u(96.0)).abs() < 0.05, "{off}");
    // The player moves: the next plan (at the path's end, the box's wall)
    // goes to where they are now.
    let c2 = c + Vec3::new(1.5, 0.0, 1.0);
    place(&mut sim, p, c2, 0.0);
    let first = goal;
    until(&mut sim, 11.0, |s| b(s, bt).route_goal.is_some_and(|g| g.distance(first) > 0.01)).expect("planned again");
    let goal = b(&sim, bt).route_goal.unwrap();
    assert!((goal.xz().distance(c2.xz()) - u(96.0)).abs() < 0.05, "{goal}");
    assert!(now(&sim) - t0 < 12.0);
    // It gives up at 12 s.
    let gave_up = until(&mut sim, 13.0, |s| b(s, bt).state == BotState::Patrol).expect("gives up");
    assert!(((now(&sim) - t0) - 12.0).abs() < 0.1, "after {gave_up}");
}

/// Case 28: the player out of view in an attack: draw −10.5 ends it at
/// once, draw 5 after 15.5 s.
#[test]
fn case_28_attack_chase_timer() {
    let _sim = world();
    assert_eq!(math::attack_chase_timer(0.0), 0.0);
    assert!((math::attack_chase_timer(1.0) - 15.5).abs() < 1e-5);
    let (mut sim, p, bt) = duel(Level::Easy, 0, 800.0, RIFLE_KIT);
    until(&mut sim, 3.0, |s| matches!(b(s, bt).state, BotState::Attack { .. })).unwrap();
    // The player behind a wall.
    wall(&mut sim, Vec3::new(-4.0, 0.0, 1.0), Vec3::new(4.0, 3.0, 1.5));
    sim.seconds(0.2);
    let bot = b(&sim, bt);
    let end = bot.chase_timer.expect("the chase timer runs");
    assert!(end - now(&sim) <= 15.5 && end - now(&sim) >= -0.1);
    let _ = p;
}

// ---------------------------------------------------------------------------
// Weapons

/// Case 29: AR (1000) and sniper: at 850 u the AR, at 900 u (> 894.4) the
/// sniper.
#[test]
fn case_29_weapon_choice() {
    let _sim = world();
    let ar = Some(Choice {
        has_ammo: true,
        range_max: 1000.0,
    });
    let sr = Some(Choice {
        has_ammo: true,
        range_max: 5000.0,
    });
    assert_eq!(math::choose_weapon(ar, sr, None, 850.0), 0);
    assert_eq!(math::choose_weapon(ar, sr, None, 900.0), 1);
    // In play: the bot holds the sniper at 900 u and keeps it.
    let (mut sim, _p, bt) = duel(Level::Easy, 0, 900.0, [Some(RIFLE), Some(SNIPER), None, Some(KNIFE), None, None]);
    until(&mut sim, 3.0, |s| matches!(b(s, bt).state, BotState::Attack { .. })).unwrap();
    sim.seconds(1.0);
    let held = b(&sim, bt).held.unwrap();
    assert_eq!(sim.app.world().get::<Weapon>(held).unwrap().id, SNIPER);
}

/// Case 30: an Easy bot with an AR (1000) and a pistol; the AR runs dry:
/// the pistol, reach still 1000: at 800 u it stays in the attack, its
/// moves aimed at 400 u from the target.
#[test]
fn case_30_reach_fixed_at_spawn() {
    let (mut sim, p, bt) = duel(Level::Easy, 0, 800.0, [Some(RIFLE), None, Some(PISTOL), Some(KNIFE), None, None]);
    sim.ticks(3);
    assert_eq!(b(&sim, bt).ranged_reach, 1000.0);
    let rifle = {
        let w = sim.app.world();
        let inv = w.get::<Inventory>(bt).unwrap();
        inv.weapons.iter().copied().find(|&x| w.get::<Weapon>(x).unwrap().id == RIFLE).unwrap()
    };
    {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(rifle).unwrap();
        m.clip = 0;
        m.reserve = 0;
    }
    sim.app.world_mut().resource_mut::<BotSettings>().stop = 0;
    until(&mut sim, 3.0, |s| matches!(b(s, bt).state, BotState::Attack { .. })).unwrap();
    let mut seen_move = None;
    until(&mut sim, 4.0, |s| {
        if let Some(to) = b(s, bt).move_to {
            seen_move = Some(to);
        }
        seen_move.is_some()
    });
    let bot = b(&sim, bt);
    assert_eq!(bot.ranged_reach, 1000.0);
    assert!(matches!(bot.state, BotState::Attack { .. }), "{:?}", bot.state);
    assert_eq!(sim.app.world().get::<Weapon>(bot.held.unwrap()).unwrap().id, PISTOL);
    let to = seen_move.expect("an attack move");
    let d = to.xz().distance(sim.position(p).xz()) / UNIT;
    assert!((d - 400.0).abs() < 30.0, "{d}");
}

/// The bot's grenade weapon.
fn grenade_of(sim: &Sim, bt: Entity) -> Entity {
    let w = sim.app.world();
    let inv = w.get::<Inventory>(bt).unwrap();
    inv.weapons.iter().copied().find(|&x| w.get::<Weapon>(x).unwrap().id == M67).unwrap()
}

/// Case 31: HN (G = 15), an M67, the player at 1000 u: each shot counts
/// down, a shot that hurts the player resets the count to 15, at 0 the
/// next action is a throw.
#[test]
fn case_31_grenade_after_misses() {
    let (mut sim, p, bt) = duel(Level::Normal, 0, 1000.0, [Some(RIFLE), None, None, Some(KNIFE), Some(M67), None]);
    // Shots that hit don't hurt (a blocked damage kind): "without damage".
    sim.app.world_mut().entity_mut(p).insert(DamageFilter {
        blocked: vec![DamageKind::Bullet, DamageKind::Blast],
    });
    sim.ticks(3);
    assert_eq!(b(&sim, bt).grenades, 15);
    // Shots count down (the player can't be hurt: health 1000).
    let shots = std::cell::Cell::new(0);
    until(&mut sim, 6.0, |s| {
        let w = s.app.world_mut();
        let events: Vec<WeaponEvent> = w.resource_mut::<Messages<WeaponEvent>>().drain().collect();
        shots.set(shots.get() + events.iter().filter(|e| e.owner == bt && matches!(e.kind, WeaponEventKind::Fired { .. })).count());
        shots.get() >= 3
    })
    .expect("fires");
    let g = b(&sim, bt).grenades;
    assert!(g <= 12, "{g} after {} shots", shots.get());
    // A hit that hurts the player resets it.
    sim.app.world_mut().entity_mut(p).remove::<DamageFilter>();
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: Some(bt),
        amount: 0.1,
        point: Vec3::ZERO,
        dir: Vec3::Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(1);
    assert!(b(&sim, bt).grenades >= 14, "{}", b(&sim, bt).grenades);
    // At 0: a throw, the count back to 15.
    sim.app.world_mut().entity_mut(p).insert(DamageFilter {
        blocked: vec![DamageKind::Bullet, DamageKind::Blast],
    });
    let left = sim.app.world().get::<Throwable>(grenade_of(&sim, bt)).unwrap().count;
    b_mut(&mut sim, bt).grenades = 0;
    let thrown = until(&mut sim, 1.0, |s| {
        let w = s.app.world_mut();
        w.query::<&CaGrenade>().iter(w).any(|g| g.thrower == Some(bt))
    });
    assert!(thrown.is_some(), "threw");
    assert_eq!(sim.app.world().get::<Throwable>(grenade_of(&sim, bt)).unwrap().count, left - 1);
    assert!(b(&sim, bt).grenades >= 13);
    // Its flight: v = 2 (T − F) + (0, 400, 0) u/s (case 33).
    let w = sim.app.world_mut();
    let g = w.query::<&CaGrenade>().iter(w).find(|g| g.thrower == Some(bt)).unwrap().clone();
    assert!(g.velocity.length() > 1000.0, "{}", g.velocity);
}

/// Case 32: XN with a mine and a grenade: thrown reach 1500 (the player
/// at 1400 u is in it); without the mine 1200 (not).
#[test]
fn case_32_grenade_reach() {
    let sim = world();
    let data = sim.app.world().resource::<BotData>();
    let xn = Template::new(Level::Veteran, 0);
    let with = roster::thrown_reach(data, xn, &["ThrowWeapon", "Mine"]);
    let without = roster::thrown_reach(data, xn, &["ThrowWeapon"]);
    assert_eq!((with, without), (1500.0, 1200.0));
    assert!(1400.0 <= with && 1400.0 > without);
    // A bot with the M67 alone: 1200.
    let (mut sim, _p, bt) = duel(Level::Veteran, 0, 1400.0, [Some(RIFLE), None, None, Some(KNIFE), Some(M67), None]);
    sim.ticks(3);
    assert_eq!(b(&sim, bt).thrown_reach, 1200.0);
}

/// Case 33: F = (0, 60, 0), T = (0, 60, 800), angle 0, power 1:
/// (0, 400, 1600), 1649.2 u/s.
#[test]
fn case_33_throw_solution() {
    let _sim = world();
    let v = math::throw_velocity(Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 60.0, 800.0), 0.5, -1600.0, 1.0, 0.0);
    assert!((v - Vec3::new(0.0, 400.0, 1600.0)).length() < 1e-3);
    assert!((v.length() - 1649.24).abs() < 0.01);
}

/// A player's M67 resting at `d` u from the bot (on +X).
fn player_grenade(sim: &mut Sim, p: Entity, bt: Entity, d: f32) {
    let w = sim.app.world_mut();
    let g = chimera_sim::weapon::give(w, p, M67).unwrap();
    let at = w.get::<Transform>(bt).unwrap().translation;
    let at = Vec3::new(at.x + u(d), 0.15, at.z);
    grenades::spawn_grenade(w, g, Some(p), at, Vec3::ZERO);
}

/// Case 34: a player's M67 rests 100 u from an HN bot, line clear: after
/// the delay (0.1-0.7 s) the bot heads for the point 1.1 r from it,
/// straight away.
#[test]
fn case_34_grenade_dodge() {
    let (mut sim, p, bt) = duel(Level::Normal, 0, 2500.0, RIFLE_KIT);
    sim.app.world_mut().resource_mut::<BotSettings>().stop = 0;
    let r = 450.0;
    player_grenade(&mut sim, p, bt, 100.0);
    let start = now(&sim);
    let t = until(&mut sim, 2.0, |s| matches!(b(s, bt).state, BotState::Dodge { .. })).expect("dodges");
    assert!((0.05..=0.85).contains(&t), "{t}");
    let BotState::Dodge { to, .. } = b(&sim, bt).state else { unreachable!() };
    let w = sim.app.world_mut();
    let g = w.query::<&CaGrenade>().iter(w).next().unwrap().clone();
    let gp = g.pos * g.unit;
    let d = to.xz().distance(gp.xz()) / UNIT;
    assert!((d - 1.1 * r).abs() < 20.0, "{d}");
    // Straight away: on the far side of the bot from the grenade.
    let bot_at = sim.position(bt);
    assert!((to.x - gp.x) * (bot_at.x - gp.x) > 0.0 || (to.x - gp.x).abs() > 0.1);
    let _ = start;
}

/// Case 35: the grenade rests r + 101 u away: no reaction.
#[test]
fn case_35_grenade_too_far() {
    let (mut sim, p, bt) = duel(Level::Normal, 0, 2500.0, RIFLE_KIT);
    player_grenade(&mut sim, p, bt, 450.0 + 101.0);
    assert!(until(&mut sim, 2.0, |s| b(s, bt).danger.is_some() || matches!(b(s, bt).state, BotState::Dodge { .. })).is_none());
}

/// Case 36: XE (safe 0.5), a mine of blast radius 300: at 140 u not seen,
/// at 160 u seen.
#[test]
fn case_36_mine_as_target() {
    let sim = world();
    let safe = sim.app.world().resource::<BotData>().template.safe_device.at(Template::new(Level::Veteran, 2));
    assert_eq!(safe, 0.5);
    assert!(!math::device_seen(140.0, 300.0, safe));
    assert!(math::device_seen(160.0, 300.0, safe));
}

/// Case 37: HN rifle: bursts of 3-7 rounds by the clip, pauses of
/// 0.3-0.4 s between them.
#[test]
fn case_37_burst_rhythm() {
    let (mut sim, _p, bt) = duel(Level::Normal, 0, 600.0, RIFLE_KIT);
    let mut shots: Vec<f64> = Vec::new();
    until(&mut sim, 8.0, |s| {
        let t = now(s);
        let w = s.app.world_mut();
        let events: Vec<WeaponEvent> = w.resource_mut::<Messages<WeaponEvent>>().drain().collect();
        for e in events {
            if e.owner == bt && matches!(e.kind, WeaponEventKind::Fired { .. }) {
                shots.push(t);
            }
        }
        false
    });
    assert!(shots.len() > 10, "{shots:?}");
    // Bursts between pauses; a reload (over 1.5 s) cuts the burst before
    // it, which isn't counted.
    let mut bursts: Vec<(u32, bool)> = vec![(1, false)];
    let mut pauses = Vec::new();
    for w in shots.windows(2) {
        let gap = w[1] - w[0];
        if gap > 1.5 {
            bursts.last_mut().unwrap().1 = true;
            bursts.push((1, true));
        } else if gap > 0.2 {
            pauses.push(gap);
            bursts.push((1, false));
        } else {
            bursts.last_mut().unwrap().0 += 1;
        }
    }
    // The first and last bursts may be cut by the test.
    let inner: Vec<u32> = bursts[1..bursts.len() - 1].iter().filter(|b| !b.1).map(|b| b.0).collect();
    assert!(!inner.is_empty(), "{bursts:?}");
    assert!(inner.iter().all(|&n| (3..=7).contains(&n)), "{bursts:?}");
    // The pause plus the rifle's fire cycle (the next shot waits for it).
    assert!(pauses.iter().all(|&g| (0.29..=0.4 + 0.12).contains(&g)), "{pauses:?}");
}

/// Case 38: the clip empty, reserve 30: the reload starts and an attack
/// move with it.
#[test]
fn case_38_reload() {
    let (mut sim, _p, bt) = duel(Level::Easy, 0, 600.0, RIFLE_KIT);
    until(&mut sim, 3.0, |s| matches!(b(s, bt).state, BotState::Attack { .. })).unwrap();
    sim.seconds(1.0);
    let held = b(&sim, bt).held.unwrap();
    {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(held).unwrap();
        m.clip = 0;
        m.reserve = 30;
    }
    b_mut(&mut sim, bt).next_move = now(&sim) + 100.0;
    sim.ticks(1);
    assert!(sim.intent(bt).reload, "reloads");
    assert!(b(&sim, bt).next_move <= now(&sim), "an attack move at once");
    sim.seconds(3.0);
    let m = sim.app.world().get::<Magazine>(held).unwrap();
    assert!(m.clip > 0, "refilled");
}

/// Case 39: radius 16, NN (shuffle 30), facing +Z: shuffle right 46 u to
/// its right, back up 66 u behind.
#[test]
fn case_39_attack_move_geometry() {
    let _sim = world();
    let f = Vec3::Z;
    let r = math::move_target(Move::ShuffleRight, Vec3::ZERO, f, 30.0, 16.0, false, 0.0);
    assert!((r.length() - 46.0).abs() < 1e-4 && r.z.abs() < 1e-4);
    // Its right when facing +Z (right = forward × up).
    assert!((r - f.cross(Vec3::Y) * 46.0).length() < 1e-4);
    let back = math::move_target(Move::BackUp, Vec3::ZERO, f, 30.0, 16.0, false, 0.0);
    assert!((back - Vec3::new(0.0, 0.0, -66.0)).length() < 1e-4);
}

/// Case 40: killed at t = 0: back at 7 s (the mode's respawn) at a Bravo
/// point, protected.
#[test]
fn case_40_respawn() {
    let (mut sim, p, bt) = duel(Level::Easy, 0, 800.0, RIFLE_KIT);
    combat_arms::game::rules::spawns::unprotect(sim.app.world_mut(), bt);
    sim.app.world_mut().write_message(Damage {
        target: bt,
        attacker: Some(p),
        amount: 10.0,
        point: Vec3::ZERO,
        dir: Vec3::Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Generic,
        weapon: None,
        force: Vec3::ZERO,
    });
    until(&mut sim, 1.0, |s| s.app.world().get::<Dead>(bt).is_some()).expect("dies");
    let t0 = now(&sim);
    let back = until(&mut sim, 9.0, |s| s.app.world().get::<Dead>(bt).is_none()).expect("respawns");
    assert!((back - 7.0).abs() < 0.1, "{back}");
    let at = sim.position(bt);
    assert!((at.z + 100.0).abs() < 1.0, "at a Bravo point: {at}");
    assert!(sim.app.world().get::<CaProtected>(bt).is_some());
    let _ = t0;
    // Its kit again, its brain fresh.
    sim.ticks(3);
    assert!(sim.app.world().get::<Inventory>(bt).is_some_and(|i| !i.weapons.is_empty()));
    assert!(b(&sim, bt).armed);
}

/// Case 41: A never visited, B visited 5 s ago dead ahead: A. A at 100000
/// ms sideways, B at 104000 ms ahead: B (94000 < 100000).
#[test]
fn case_41_patrol_link_choice() {
    let _sim = world();
    let f = Vec3::Z;
    let now_ms = 50_000.0;
    assert_eq!(math::patrol_choice(&[(0.0, Vec3::X), (now_ms - 5000.0, f)], f), Some(0));
    assert_eq!(math::patrol_choice(&[(100_000.0, Vec3::X), (104_000.0, f)], f), Some(1));
    assert_eq!(math::patrol_score(104_000.0, f, f), 94_000.0);
}

/// Case 42: NE with a mine enters patrol at 0, draw 0.5: the mine at 7.5 s.
#[test]
fn case_42_mine_drop_timer() {
    let sim = world();
    let t = &sim.app.world().resource::<BotData>().template;
    let ne = Template::new(Level::Easy, 2);
    assert_eq!(math::mine_timer(t.mine_min.at(ne), t.mine_max.at(ne), 0.5), 7.5);
}

/// Case 43: areas A → B → C, the start 100 u from the A–B link, the B–C
/// link 300 u from where the path entered B: 100² + 300² = 100000.
#[test]
fn case_43_path_cost() {
    let _sim = world();
    let s = UNIT;
    let area = |id: u32, x0: f32, x1: f32| NavArea {
        id,
        flags: 0,
        min: Vec2::new(x0, 0.0),
        max: Vec2::new(x1, 100.0 * s),
        heights: [0.0; 4],
        center: Vec3::new((x0 + x1) / 2.0, 0.0, 50.0 * s),
        links: Vec::new(),
        place: None,
        hiding: Vec::new(),
        visible: Vec::new(),
        encounters: Vec::new(),
    };
    let mut m = NavMesh {
        areas: vec![area(0, 0.0, 200.0 * s), area(1, 200.0 * s, 500.0 * s), area(2, 500.0 * s, 700.0 * s)],
        ladders: Vec::new(),
        places: Vec::new(),
        generated: false,
        ..Default::default()
    };
    m.areas[0].links.push((1, Via::Walk(Side::MaxX)));
    m.areas[1].links.push((0, Via::Walk(Side::MinX)));
    m.areas[1].links.push((2, Via::Walk(Side::MaxX)));
    m.areas[2].links.push((1, Via::Walk(Side::MinX)));
    let start = Vec3::new(100.0 * s, 0.0, 50.0 * s);
    let path = bot_nav::find(&m, 0, start, 2, s, bot_nav::MIN_LINK, |_| 1.0).unwrap();
    assert_eq!(path.areas.iter().map(|a| a.0).collect::<Vec<_>>(), vec![0, 1, 2]);
    assert!((path.cost - 100_000.0).abs() < 5.0, "{}", path.cost);
}

/// Case 44: the round reaches the kill goal: every bot stops moving and
/// firing on the next update.
#[test]
fn case_44_freeze() {
    let (mut sim, _p, bt) = duel(Level::Easy, 0, 600.0, RIFLE_KIT);
    sim.app.world_mut().resource_mut::<BotSettings>().stop = 0;
    until(&mut sim, 3.0, |s| matches!(b(s, bt).state, BotState::Attack { .. })).unwrap();
    sim.seconds(1.5);
    // The goal reached: the round is over.
    let t = now(&sim);
    sim.app.world_mut().resource_mut::<CaMatch>().phase = Phase::RoundOver { until: t + 3.0 };
    sim.ticks(1);
    let i = sim.intent(bt).clone();
    assert!(!i.fire && i.move_axis == Vec2::ZERO, "{i:?}");
}

// ---------------------------------------------------------------------------
// Bots playing

/// The greybox with four start points a team (its own two, which have no
/// team, removed), playing Team Death Match.
fn greybox() -> Sim {
    let mut sim = Sim::new(CaSim, chimera_world::greybox::GreyboxMapPlugin);
    let w = sim.app.world_mut();
    let old: Vec<Entity> = w.query_filtered::<Entity, With<SpawnPoint>>().iter(w).collect();
    for e in old {
        w.despawn(e);
    }
    for (k, team) in [ALPHA, BRAVO].into_iter().enumerate() {
        for i in 0..4 {
            let z = if team == ALPHA { 30.0 } else { -30.0 };
            w.spawn((
                Name::new(format!("Spawn {}", k * 4 + i)),
                SpawnPoint { team: Some(Team(team)) },
                Transform::from_xyz(-6.0 + 4.0 * i as f32, 1.0, z),
            ));
        }
    }
    with_mode(w, "TDM");
    sim
}

/// The quota fills while the match starts (bots given their kit in the
/// tick the mission start respawns everyone): all in, armed.
#[test]
fn bots_join_while_the_match_starts() {
    let mut sim = world();
    let _p = human(&mut sim, BRAVO);
    console(&mut sim, "MaxPlayer 6");
    sim.seconds(3.0);
    let w = sim.app.world_mut();
    let bots: Vec<(u8, bool)> = w.query::<(&Team, &CaBot)>().iter(w).map(|(t, b)| (t.0, b.armed)).collect();
    assert_eq!(bots.len(), 6);
    assert!(bots.iter().all(|&(t, armed)| t == ALPHA && armed), "{bots:?}");
}

/// Bots fight each other in TDM on the greybox: they find, shoot and kill
/// each other, and the dead come back.
#[test]
fn bots_fight_on_the_greybox() {
    let mut sim = greybox();
    for team in [ALPHA, BRAVO, ALPHA, BRAVO] {
        bots::add_bot(sim.app.world_mut(), Some(team)).unwrap();
    }
    let mut deaths = 0;
    let mut was_dead = std::collections::HashSet::new();
    for _ in 0..(60.0 / 0.015) as usize {
        sim.ticks(1);
        let w = sim.app.world_mut();
        let dead: Vec<Entity> = w.query_filtered::<Entity, (With<CaBot>, With<Dead>)>().iter(w).collect();
        for d in &dead {
            if was_dead.insert(*d) {
                deaths += 1;
            }
        }
        was_dead.retain(|e| dead.contains(e));
    }
    let m = sim.app.world().resource::<CaMatch>().clone();
    assert!(deaths >= 1, "nobody died: {m:?}");
    assert!(m.score.iter().sum::<i32>() >= 1, "kills scored: {:?}", m.score);
}

// ---------------------------------------------------------------------------
// The map's AI data, glass, the knife's reach

/// An AI volume as `game::world` passes it through: class, name, `origin`
/// and `Dims` in entity axes (the file's x, z, y), units.
fn ai_volume(name: &str, centre: [f32; 3], dims: [f32; 3]) -> MapEntity {
    let kv = |k: &str, v: String| (k.to_string(), v);
    MapEntity {
        keyvalues: vec![
            kv("classname", "AIVolume".into()),
            kv("targetname", name.into()),
            kv("origin", format!("{} {} {}", centre[0], centre[2], centre[1])),
            kv("Dims", format!("{} {} {}", dims[0], dims[2], dims[1])),
        ],
        ..default()
    }
}

/// A patrol node at `pos` (file axes, u) linking to `next`.
fn patrol_node(name: &str, pos: [f32; 3], next: &str) -> MapEntity {
    let kv = |k: &str, v: String| (k.to_string(), v);
    MapEntity {
        keyvalues: vec![
            kv("classname", "AINodePatrol".into()),
            kv("targetname", name.into()),
            kv("origin", format!("{} {} {}", pos[0], pos[2], pos[1])),
            kv("Next", next.into()),
            kv("StartDisabled", "0".into()),
        ],
        ..default()
    }
}

/// bots.md 3: volumes link where their boxes overlap or touch on all
/// three axes, across the shared rectangle of their tops, and not when
/// it is shorter than 48 u; patrol nodes link by `Next` both ways.
#[test]
fn map_volumes_link_by_their_boxes() {
    let s = UNIT;
    let entities = vec![
        // A 400 x 400 u floor box, its bottom at y 0 (file axes).
        ai_volume("A", [0.0, 32.0, 0.0], [200.0, 32.0, 200.0]),
        // Touching A's +x face, 400 u of shared edge.
        ai_volume("B", [400.0, 32.0, 0.0], [200.0, 32.0, 200.0]),
        // Touching B's +x face over only 40 u of z: too short.
        ai_volume("C", [800.0, 32.0, 360.0], [200.0, 32.0, 200.0]),
        // Overlapping A on its -z side, 300 u of shared edge.
        ai_volume("D", [0.0, 32.0, -350.0], [150.0, 32.0, 200.0]),
        // Above A, apart from it: not linked.
        ai_volume("E", [0.0, 300.0, 0.0], [200.0, 32.0, 200.0]),
        patrol_node("N0", [0.0, 16.0, 0.0], "N1"),
        patrol_node("N1", [400.0, 16.0, 0.0], ""),
        patrol_node("N2", [0.0, 16.0, -350.0], "N0"),
    ];
    let ai = MapAi::read(&entities, s);
    assert_eq!((ai.volumes.len(), ai.patrol.len()), (5, 3));
    let a = &ai.volumes[0];
    assert!(a.min.abs_diff_eq(Vec3::new(-200.0, 0.0, -200.0) * s, 1e-5), "{a:?}");
    assert!(a.max.abs_diff_eq(Vec3::new(200.0, 64.0, 200.0) * s, 1e-5), "{a:?}");
    // File z is mirrored: D at z -350 is engine z +350.
    assert!((ai.volumes[3].min.z - 150.0 * s).abs() < 1e-5, "{:?}", ai.volumes[3]);
    let mesh = ai.nav_mesh(s, |_, _| None).unwrap();
    let links = |i: usize| -> Vec<(usize, Via)> { mesh.areas[i].links.clone() };
    assert_eq!(links(0), vec![(1, Via::Walk(Side::MaxX)), (3, Via::Walk(Side::MaxZ))]);
    assert_eq!(links(1), vec![(0, Via::Walk(Side::MinX))], "B-C's 40 u edge dropped");
    assert!(links(2).is_empty() && links(4).is_empty());
    assert_eq!(mesh.areas[0].heights, [0.0; 4], "floors at the boxes' bottoms without a floor found");
    let graph = ai.patrol_graph(&mesh).unwrap();
    assert_eq!(graph.links, vec![vec![1, 2], vec![0], vec![0]]);
    assert_eq!(graph.nodes, vec![0, 1, 3]);
    assert!(graph.points[1].abs_diff_eq(Vec3::new(400.0, 0.0, 0.0) * s, 1e-5), "on the floor: {:?}", graph.points);
    // Over the limit: none.
    let many: Vec<MapEntity> = (0..1025).map(|i| ai_volume("V", [i as f32 * 500.0, 32.0, 0.0], [100.0; 3])).collect();
    assert!(MapAi::read(&many, s).nav_mesh(s, |_, _| None).is_none());
}

/// A map with AI volumes and patrol nodes: the bots walk its volumes (not
/// generated areas) and patrol its nodes.
#[test]
fn bots_use_the_maps_volumes_and_patrol_nodes() {
    let mut sim = world();
    // Two floor boxes end to end along z (file axes), 6000 u long each,
    // over the start points (engine z ±100 m), and a patrol chain along
    // them.
    let entities = vec![
        ai_volume("A", [0.0, 32.0, 3000.0], [600.0, 32.0, 3000.0]),
        ai_volume("B", [0.0, 32.0, -3000.0], [600.0, 32.0, 3000.0]),
        patrol_node("P0", [0.0, 16.0, 5000.0], "P1"),
        patrol_node("P1", [0.0, 16.0, 0.0], "P2"),
        patrol_node("P2", [0.0, 16.0, -5000.0], ""),
    ];
    sim.app.world_mut().insert_resource(MapEntities {
        entities: std::sync::Arc::new(entities),
        scale: UNIT,
    });
    let bt = bots::add_bot(sim.app.world_mut(), Some(ALPHA)).unwrap();
    sim.seconds(1.0);
    let nav = sim.app.world().resource::<brain::BotNav>().clone();
    assert_eq!(nav.mesh.areas.len(), 2, "the map's two volumes, not generated areas");
    // A (the file's +z) is engine -z: B lies across its +z side.
    assert_eq!(nav.mesh.areas[0].links, vec![(1, Via::Walk(Side::MaxZ))]);
    assert_eq!(nav.patrol.points.len(), 3, "the map's patrol nodes");
    assert!(nav.mesh.areas[0].heights[0].abs() < 0.05, "the floor found inside the box: {:?}", nav.mesh.areas[0].heights);
    // The bot patrols node to node along the volumes.
    let reached = until(&mut sim, 80.0, |s| s.app.world().resource::<brain::PatrolClock>().reached.len() >= 2);
    assert!(reached.is_some(), "patrol nodes reached: {:?}", sim.app.world().resource::<brain::PatrolClock>().reached);
    let feet = sim.app.world().get::<Transform>(bt).unwrap().translation;
    assert!(feet.x.abs() < 600.0 * UNIT, "stayed over the volumes: {feet}");
}

/// bots.md 5.2 step 6: see-through surfaces (glass, Chimera's
/// `SeeThrough`) don't block sight; a wall does.
#[test]
fn bots_see_through_glass() {
    let (mut sim, p, bt) = duel(Level::Veteran, 0, 800.0, RIFLE_KIT);
    let pane = sim
        .app
        .world_mut()
        .spawn((
            RigidBody::Static,
            Collider::cuboid(6.0, 3.0, 0.05),
            Transform::from_xyz(0.0, 1.5, u(400.0)),
            SeeThrough,
        ))
        .id();
    // The navigation areas made again around the pane (no straight walk
    // through it: the ray decides, 5.2 step 5).
    sim.ticks(1);
    sim.app.world_mut().remove_resource::<NavMesh>();
    sim.ticks(2);
    assert_eq!(brain::sees(sim.app.world_mut(), bt, p), Some(false), "seen through the glass");
    sim.app.world_mut().entity_mut(pane).remove::<SeeThrough>();
    sim.ticks(2);
    assert_eq!(brain::sees(sim.app.world_mut(), bt, p), None, "the same pane as a wall hides the player");
}

/// bots.md 8.4: the bot's knife reaches 130 u (the player's 105): a bot
/// with only its knife, kept from moving, cuts a player 120 u away, not
/// one 140 u away.
#[test]
fn the_bot_knife_reaches_130_units() {
    let knife_only = [None, None, None, Some(KNIFE), None, None];
    let cut = |d: f32| {
        let (mut sim, p, bt) = duel(Level::Veteran, 0, d, knife_only);
        assert_eq!(sim.app.world().get::<KnifeReach>(bt), Some(&KnifeReach(130.0 * UNIT)));
        let hurt = until(&mut sim, 4.0, |s| s.app.world().get::<Health>(p).unwrap().current < 1000.0);
        let held = sim.app.world().get::<Inventory>(bt).and_then(|i| i.active);
        let id = held.and_then(|w| sim.app.world().get::<Weapon>(w)).map(|w| w.id);
        (hurt.is_some(), id)
    };
    assert_eq!(cut(120.0), (true, Some(KNIFE)), "cut at 120 u");
    assert!(!cut(140.0).0, "out of reach at 140 u");
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-22): cases 58-69 (bots_objectives.md 25-28 below)

use combat_arms::game::bots::{
    MercMind,
    chat::{CaBotChat, CaBotEvent, CaChat, Event as ChatEvent},
};

/// Post a footstep or a gunshot of `src` at `at` (m) every tick until the
/// bot's state is `Investigate`; that state (None: it never was in 4 s).
fn hear(sim: &mut Sim, bt: Entity, src: Entity, kind: Kind, at: Vec3) -> Option<BotState> {
    let radius = if kind == Kind::Gunfire { 2000.0 } else { 256.0 };
    // A footstep every 0.4 s; a shot is reacted to 0.5-1.5 s later (and a
    // second shot would set that back), so one every 2 s.
    let gap = if kind == Kind::Gunfire { 160 } else { 32 };
    for _ in 0..12 {
        let t = now(sim);
        sim.app.world_mut().resource_mut::<Stimuli>().post(kind, at, radius, src, ALPHA, t);
        for _ in 0..gap {
            sim.ticks(1);
            let st = b(sim, bt).state;
            if matches!(st, BotState::Investigate { .. }) {
                return Some(st);
            }
        }
    }
    None
}

/// A set-up for the senses: the bot at the origin facing +Z, the player far
/// behind it (out of sight range), the bots free to move.
fn listener() -> (Sim, Entity, Entity) {
    let (mut sim, p, bt) = duel(Level::Easy, 0, 5000.0, RIFLE_KIT);
    place(&mut sim, p, Vec3::new(0.0, 0.0, -u(4000.0)), FACE_PLUS_Z);
    place(&mut sim, bt, Vec3::ZERO, FACE_PLUS_Z);
    sim.ticks(2);
    (sim, p, bt)
}

/// Case 58: a gunshot 1000 u away: the bot runs toward it and is back on
/// patrol on the first update within 500 u of the shot; at 300 u, 150 u.
#[test]
fn case_58_half_distance_investigation() {
    for (d, half) in [(1000.0, 500.0), (300.0, 150.0)] {
        let (mut sim, p, bt) = listener();
        let at = Vec3::new(u(d), 0.9, 0.0);
        let Some(BotState::Investigate { radius, look_only, .. }) = hear(&mut sim, bt, p, Kind::Gunfire, at) else {
            panic!("never investigated")
        };
        assert!(!look_only, "a gunshot is always run to");
        sim.app.world_mut().resource_mut::<BotSettings>().stop = 0;
        let start = sim.position(bt).distance(at) / UNIT;
        assert!((radius / UNIT - start / 2.0).abs() < 2.0, "half of {start}: {}", radius / UNIT);
        until(&mut sim, 10.0, |s| b(s, bt).state == BotState::Patrol).expect("back on patrol");
        let end = sim.position(bt).distance(at) / UNIT;
        assert!(end <= half + 1.0 && end >= half - 10.0, "gave up at {end} u from a shot {d} u away");
        assert!(end >= start / 2.0 - 10.0, "never closer than half the starting distance");
    }
}

/// Case 59: look only: 40 s after spawning, a footstep in plain view at
/// 400 u is turned to; at 20 s after spawning, at 1200 u, or after a
/// gunshot heard this life, it runs.
#[test]
fn case_59_look_only() {
    let run = |relaxed_ago: f64, alarm: f32, d: f32| {
        let (mut sim, p, bt) = listener();
        let t = now(&sim);
        {
            let mut bot = b_mut(&mut sim, bt);
            bot.last_relaxed = Some(t - relaxed_ago);
            bot.alarm = alarm;
        }
        let st = hear(&mut sim, bt, p, Kind::Footstep, Vec3::new(0.0, 0.9, u(d))).expect("investigates");
        let before = sim.position(bt);
        sim.app.world_mut().resource_mut::<BotSettings>().stop = 0;
        sim.seconds(1.0);
        let moved = sim.position(bt).distance(before) / UNIT;
        let BotState::Investigate { look_only, .. } = st else { unreachable!() };
        (look_only, moved)
    };
    let (look, moved) = run(40.0, 0.0, 400.0);
    assert!(look && moved < 5.0, "turns only: {look} {moved}");
    let (look, moved) = run(20.0, 0.0, 400.0);
    assert!(!look && moved > 50.0, "20 s after spawning it runs: {look} {moved}");
    let (look, _) = run(40.0, 0.0, 1200.0);
    assert!(!look, "1200 u: runs");
    let (look, _) = run(40.0, 75.0, 400.0);
    assert!(!look, "after a gunshot heard this life: runs");
}

/// Case 60: an investigation of a gunshot ignores an ally's call; a
/// footstep investigation is replaced by a gunshot.
#[test]
fn case_60_calls_while_investigating() {
    let (mut sim, p, bt) = listener();
    let shot = Vec3::new(u(900.0), 0.9, 0.0);
    let Some(BotState::Investigate { at, id, .. }) = hear(&mut sim, bt, p, Kind::Gunfire, shot) else { panic!() };
    for _ in 0..40 {
        let t = now(&sim);
        sim.app.world_mut().resource_mut::<Stimuli>().post(Kind::AllyCall, Vec3::new(-u(300.0), 0.9, 0.0), 512.0, p, BRAVO, t);
        sim.ticks(1);
    }
    let BotState::Investigate { at: now_at, id: now_id, .. } = b(&sim, bt).state else {
        panic!("{:?}", b(&sim, bt).state)
    };
    assert_eq!((at, id), (now_at, now_id), "the call was ignored");

    let (mut sim, p, bt) = listener();
    let t = now(&sim);
    b_mut(&mut sim, bt).last_relaxed = Some(t - 40.0);
    let Some(BotState::Investigate { level, .. }) = hear(&mut sim, bt, p, Kind::Footstep, Vec3::new(0.0, 0.9, u(400.0))) else { panic!() };
    assert!(level < 50.0);
    let mut n = 0;
    until(&mut sim, 10.0, |s| {
        if n % 160 == 0 {
            let t = now(s);
            s.app.world_mut().resource_mut::<Stimuli>().post(Kind::Gunfire, Vec3::new(u(300.0), 0.9, 0.0), 2000.0, p, ALPHA, t);
        }
        n += 1;
        matches!(b(s, bt).state, BotState::Investigate { level, .. } if level > 50.0)
    })
    .expect("switched to the gunshot");
}

/// Case 61: a grenade at rest r + 50 u from an HN bot: it moves onto the
/// grenade's position (and keeps dodging until it is gone); at r - 50 u,
/// to the point 1.1 r away.
#[test]
fn case_61_grenade_just_outside_the_radius() {
    let r = 450.0;
    let (mut sim, p, bt) = duel(Level::Normal, 0, 2500.0, RIFLE_KIT);
    sim.app.world_mut().resource_mut::<BotSettings>().stop = 0;
    player_grenade(&mut sim, p, bt, r + 50.0);
    until(&mut sim, 2.0, |s| matches!(b(s, bt).state, BotState::Dodge { .. })).expect("dodges");
    let BotState::Dodge { to, grenade, .. } = b(&sim, bt).state else { unreachable!() };
    let w = sim.app.world_mut();
    let g = w.get::<CaGrenade>(grenade).unwrap().clone();
    let gp = g.pos * g.unit;
    assert!(to.xz().distance(gp.xz()) < 0.05, "onto the grenade: {to} {gp}");

    // It lasts until the grenade is gone: held in place it still dodges 2 s
    // later, and ends right after the grenade is removed.
    let (mut sim, p, bt) = duel(Level::Normal, 0, 2500.0, RIFLE_KIT);
    player_grenade(&mut sim, p, bt, r + 50.0);
    until(&mut sim, 2.0, |s| matches!(b(s, bt).state, BotState::Dodge { .. })).expect("dodges");
    let BotState::Dodge { grenade, .. } = b(&sim, bt).state else { unreachable!() };
    sim.seconds(2.0);
    assert!(matches!(b(&sim, bt).state, BotState::Dodge { .. }), "still dodging while the grenade is there");
    sim.app.world_mut().despawn(grenade);
    sim.ticks(6);
    assert!(!matches!(b(&sim, bt).state, BotState::Dodge { .. }), "over with the grenade");

    let (mut sim, p, bt) = duel(Level::Normal, 0, 2500.0, RIFLE_KIT);
    sim.app.world_mut().resource_mut::<BotSettings>().stop = 0;
    player_grenade(&mut sim, p, bt, r - 50.0);
    until(&mut sim, 2.0, |s| matches!(b(s, bt).state, BotState::Dodge { .. })).expect("dodges");
    let BotState::Dodge { to, .. } = b(&sim, bt).state else { unreachable!() };
    let w = sim.app.world_mut();
    let g = w.query::<&CaGrenade>().iter(w).next().unwrap().clone();
    let gp = g.pos * g.unit;
    let d = to.xz().distance(gp.xz()) / UNIT;
    assert!((d - 1.1 * r).abs() < 20.0, "{d}");
}

fn drain_chat(sim: &mut Sim) -> Vec<(Entity, String)> {
    let w = sim.app.world_mut();
    w.resource_mut::<Messages<CaBotChat>>().drain().map(|c| (c.bot, c.key)).collect()
}

fn died(sim: &mut Sim, victim: Entity, killer: Entity) {
    sim.app.world_mut().write_message(chimera_core::core::Died {
        entity: victim,
        attacker: Some(killer),
        damage: Damage {
            target: victim,
            attacker: Some(killer),
            amount: 1.0,
            point: Vec3::ZERO,
            dir: Vec3::Z,
            hitgroup: Hitgroup::Chest,
            kind: DamageKind::Bullet,
            weapon: None,
            force: Vec3::ZERO,
        },
    });
}

/// Case 62: a bot's kill of another bot says nothing; of a player, with
/// the cooldown over, a Kill line (the roll passes on some tick).
#[test]
fn case_62_kill_chat() {
    let (mut sim, p, bt) = duel(Level::Easy, 0, 5000.0, RIFLE_KIT);
    let other = bot(&mut sim, BRAVO, Level::Easy, 0, RIFLE_KIT);
    sim.ticks(3);
    drain_chat(&mut sim);
    let mut lines = Vec::new();
    for _ in 0..24 {
        sim.app.world_mut().get_mut::<CaChat>(bt).unwrap().until = 0.0;
        died(&mut sim, other, bt);
        sim.ticks(1);
        lines.extend(drain_chat(&mut sim).into_iter().filter(|(who, k)| *who == bt && k.contains("_Kill_")));
    }
    assert!(lines.is_empty(), "no Kill line for a bot: {lines:?}");
    for _ in 0..24 {
        sim.app.world_mut().get_mut::<CaChat>(bt).unwrap().until = 0.0;
        died(&mut sim, p, bt);
        sim.ticks(1);
        lines.extend(drain_chat(&mut sim).into_iter().filter(|(who, k)| *who == bt && k.contains("_Kill_")));
    }
    assert!(!lines.is_empty(), "a Kill line for a player");
}

/// Case 63: the brain raises Battle when the attack starts from another
/// state, Search when the patrol goal comes back, Grenade for a dodge, and
/// no Battle for an attack move or a dodge inside the attack. (The cooldown
/// and percent roll are `CaChat::event`, case 64.)
#[test]
fn case_63_battle_and_search_events() {
    let (mut sim, p, bt) = duel(Level::Normal, 0, 800.0, RIFLE_KIT);
    sim.app.world_mut().resource_mut::<BotSettings>().stop = 0;
    let mut events: Vec<ChatEvent> = Vec::new();
    let tick = |sim: &mut Sim, events: &mut Vec<ChatEvent>| {
        sim.ticks(1);
        let w = sim.app.world_mut();
        events.extend(w.resource_mut::<Messages<CaBotEvent>>().drain().filter(|e| e.bot == bt).map(|e| e.event));
    };
    place(&mut sim, bt, Vec3::new(0.0, 0.0, u(5000.0)), 0.0);
    tick(&mut sim, &mut events);
    events.clear();
    place(&mut sim, bt, Vec3::new(0.0, 0.0, u(800.0)), 0.0);
    for _ in 0..800 {
        tick(&mut sim, &mut events);
        if matches!(b(&sim, bt).state, BotState::Attack { .. }) {
            break;
        }
    }
    tick(&mut sim, &mut events);
    let count = |events: &[ChatEvent], ev| events.iter().filter(|e| **e == ev).count();
    assert_eq!(count(&events, ChatEvent::Battle), 1, "{events:?}");
    // Attack moves for 3 s: still one.
    for _ in 0..240 {
        tick(&mut sim, &mut events);
    }
    assert!(matches!(b(&sim, bt).state, BotState::Attack { .. }), "{:?}", b(&sim, bt).state);
    assert_eq!(count(&events, ChatEvent::Battle), 1, "attack moves say nothing: {events:?}");
    // A dodge inside the attack: a Grenade line, and no second Battle.
    player_grenade(&mut sim, p, bt, 100.0);
    for _ in 0..200 {
        tick(&mut sim, &mut events);
        if !matches!(b(&sim, bt).state, BotState::Attack { .. } | BotState::Dodge { .. }) {
            break;
        }
    }
    assert!(events.contains(&ChatEvent::Grenade), "{events:?}");
    assert_eq!(count(&events, ChatEvent::Battle), 1, "{events:?}");
    // It loses its target: the patrol goal says Search.
    events.clear();
    place(&mut sim, p, Vec3::new(0.0, 0.0, -u(4000.0)), FACE_PLUS_Z);
    {
        let mut bot = b_mut(&mut sim, bt);
        bot.state = BotState::Chase { until: 1e9, breath: 0.0 };
        bot.target = None;
    }
    for _ in 0..8 {
        tick(&mut sim, &mut events);
    }
    assert!(events.contains(&ChatEvent::Search), "{events:?}");
}

/// Case 64: the cooldown is the profile's: 25 s on a profile that has it,
/// 10 s on one that has none; two lines are that far apart.
#[test]
fn case_64_chat_cooldown() {
    let (mut sim, _p, bt) = duel(Level::Easy, 0, 5000.0, RIFLE_KIT);
    let profile = b(&sim, bt).profile;
    sim.app.world_mut().resource_mut::<BotData>().chat.profiles.push(bots::data::ProfileChat {
        level: "easy".into(),
        profile,
        cooldown: 25.0,
    });
    let data = sim.app.world().resource::<BotData>().clone();
    assert_eq!(bots::chat::cooldown(&data, &b(&sim, bt)), 25.0);
    let mut other = b(&sim, bt);
    other.profile = profile + 1;
    assert_eq!(bots::chat::cooldown(&data, &other), 10.0);
    let mut chat = CaChat::default();
    assert!(chat.event(ChatEvent::Kill, 100.0, 100_000, 25.0, 0).is_some());
    assert!(chat.event(ChatEvent::Kill, 124.9, 124_900, 25.0, 0).is_none());
    assert!(chat.event(ChatEvent::Kill, 125.0, 125_000, 25.0, 0).is_some());
    // A new bot's first line comes one cooldown after the roster is built.
    sim.app.world_mut().entity_mut(bt).remove::<CaChat>();
    sim.ticks(2);
    let c = *sim.app.world().get::<CaChat>(bt).unwrap();
    assert!((c.until - now(&sim) - 25.0).abs() < 0.2, "{}", c.until - now(&sim));
}

/// Case 65: a fresh brain each life: the hit/miss counters, the alarm
/// total and the attack's last time are gone after a respawn, so the
/// first attack rolls a reaction delay.
#[test]
fn case_65_fresh_brain() {
    let (mut sim, p, bt) = duel(Level::Easy, 0, 5000.0, RIFLE_KIT);
    place(&mut sim, p, Vec3::new(0.0, 0.0, -u(4000.0)), FACE_PLUS_Z);
    let t = now(&sim);
    {
        let mut bot = b_mut(&mut sim, bt);
        bot.cycle = HitCycle { hits: 1, misses: 2 };
        bot.attack_off = t - 1.0;
        bot.alarm = 75.0;
    }
    sim.app.world_mut().write_message(Damage {
        target: bt,
        attacker: Some(p),
        amount: 10.0,
        point: Vec3::ZERO,
        dir: Vec3::Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(2);
    assert!(sim.app.world().get::<Dead>(bt).is_some(), "dead");
    let respawned = until(&mut sim, 15.0, |s| {
        let w = s.app.world();
        w.get::<Dead>(bt).is_none() && w.get::<Health>(bt).unwrap().current > 0.0 && w.get::<CaBot>(bt).unwrap().last_relaxed.is_some()
    });
    assert!(respawned.is_some(), "respawned");
    let bot = b(&sim, bt);
    assert_eq!((bot.cycle, bot.alarm, bot.attack_off), (HitCycle::default(), 0.0, 0.0));
    assert!(bot.last_relaxed.is_some_and(|r| (now(&sim) - r).abs() < 1.0), "spawn counts as relaxed");
    // Its first attack rolls a delay.
    place(&mut sim, bt, Vec3::new(0.0, 0.0, u(800.0)), 0.0);
    place(&mut sim, p, Vec3::ZERO, FACE_PLUS_Z);
    tough(&mut sim, p);
    sim.app.world_mut().resource_mut::<BotSettings>().stop = 1;
    let r = reaction(&mut sim, bt);
    assert!((0.6..=0.9 + 1e-6).contains(&r), "{r}");
}

/// Case 66: at a node a 0..99 draw of 20 stops the bot (3 + U(0, 3) s), 30
/// runs on; about 26 in 100 stop; the k-th Bravo spawn starts at
/// `AI_Node_B_<k mod 8 + 1>` (`PatrolNode_Bravo<k mod 8>`).
#[test]
fn case_66_patrol_stops_and_start_nodes() {
    assert!(math::patrol_stops(20, 25.0) && math::patrol_stops(25, 25.0));
    assert!(!math::patrol_stops(30, 25.0) && !math::patrol_stops(26, 25.0));
    assert_eq!((0..100).filter(|&d| math::patrol_stops(d, 25.0)).count(), 26);

    let mut sim = world();
    let _p = human(&mut sim, ALPHA);
    let mut entities = vec![
        ai_volume("A", [0.0, 32.0, 3000.0], [600.0, 32.0, 3000.0]),
        ai_volume("B", [0.0, 32.0, -3000.0], [600.0, 32.0, 3000.0]),
    ];
    for k in 0..8 {
        let next = if k < 7 { format!("AI_Node_B_{:02}", k + 2) } else { String::new() };
        entities.push(patrol_node(&format!("AI_Node_B_{:02}", k + 1), [0.0, 16.0, 5500.0 - 1400.0 * k as f32], &next));
    }
    sim.app.world_mut().insert_resource(MapEntities {
        entities: std::sync::Arc::new(entities),
        scale: UNIT,
    });
    live(&mut sim);
    let bots_: Vec<Entity> = (0..9).map(|_| bots::add_bot(sim.app.world_mut(), Some(BRAVO)).unwrap()).collect();
    let mut first: Vec<Option<usize>> = vec![None; 9];
    for _ in 0..400 {
        sim.ticks(1);
        for (k, &e) in bots_.iter().enumerate() {
            let bot = b(&sim, e);
            if first[k].is_none() && !bot.needs_start && bot.patrol_to.is_some() {
                first[k] = bot.patrol_to;
            }
        }
        if first.iter().all(Option::is_some) {
            break;
        }
    }
    let nav = sim.app.world().resource::<brain::BotNav>().clone();
    let mut got: Vec<usize> = first.iter().map(|f| f.expect("a start node")).collect();
    let expected = |k: usize| nav.patrol.by_name(&format!("AI_Node_B_{:02}", k % 8 + 1)).unwrap();
    // The bots take their nodes in an order of their own: every node is
    // used, the first twice (the ninth spawn wraps round).
    got.sort();
    let mut want: Vec<usize> = (0..9).map(expected).collect();
    want.sort();
    assert_eq!(got, want);
    assert_eq!(sim.app.world().resource::<brain::PatrolClock>().starts, [0, 9]);

    // The stop itself: a bot arriving at a node with a draw of 20.
    let bt = bots_[0];
    let seed = (0u64..).find(|&s| math::Dice::new(s).r100() == 20).unwrap();
    let pt = nav.patrol.points[0];
    place(&mut sim, bt, pt, 0.0);
    {
        let mut bot = b_mut(&mut sim, bt);
        bot.state = BotState::Patrol;
        bot.patrol_to = Some(0);
        bot.dice = math::Dice::new(seed);
        bot.patrol_wait = 0.0;
    }
    sim.ticks(2);
    let t = now(&sim);
    let wait = b(&sim, bt).patrol_wait - t;
    assert!((2.9..=6.1).contains(&wait), "stops for {wait} s");
}

/// Case 67: the grenade throw is an action: from the decision to the
/// throw's end no round is fired, the grenade leaves the hand (not the
/// eye) at the throw key, and the gun is fired again after the switch
/// back.
#[test]
fn case_67_throw() {
    let me = brain::Char {
        e: Entity::PLACEHOLDER,
        team: 2,
        pos: Vec3::new(0.0, 0.9, 0.0),
        yaw: 0.0,
        pitch: 0.0,
        alive: true,
        bot: true,
        crouched: false,
        crouching_attack: None,
    };
    assert!(me.hand().distance(me.eye()) > 0.3, "the hand is not the eye");
    assert!(me.hand().y < me.eye().y);

    let (mut sim, _p, bt) = duel(Level::Normal, 0, 800.0, [Some(RIFLE), None, None, Some(KNIFE), Some(M67), None]);
    until(&mut sim, 3.0, |s| matches!(b(s, bt).state, BotState::Attack { .. }) && b(s, bt).visible).expect("attacks");
    b_mut(&mut sim, bt).grenades = 0;
    let (mut begun, mut released, mut over) = (None, None, None);
    let mut fired: Vec<f64> = Vec::new();
    let mut thrown = false;
    for _ in 0..600 {
        sim.ticks(1);
        let t = now(&sim);
        let w = sim.app.world_mut();
        let events: Vec<WeaponEvent> = w.resource_mut::<Messages<WeaponEvent>>().drain().collect();
        fired.extend(events.iter().filter(|e| e.owner == bt && matches!(e.kind, WeaponEventKind::Fired { .. })).map(|_| t));
        thrown |= w.query::<&CaGrenade>().iter(w).next().is_some();
        if let Some(th) = b(&sim, bt).throw {
            begun.get_or_insert(t);
            assert!(th.near, "800 u: the near animation");
            if th.released {
                released.get_or_insert(t);
            }
        } else if begun.is_some() && over.is_none() {
            over = Some(t);
        }
        if over.is_some_and(|o| fired.iter().filter(|&&f| f > o).count() >= 2) {
            break;
        }
    }
    let (begun, released, over) = (begun.expect("throws"), released.expect("released"), over.expect("over"));
    assert!(thrown, "a grenade flew");
    assert!(released > begun && over >= released, "{begun} {released} {over}");
    assert!(!fired.iter().any(|&f| f >= begun && f <= over), "no round fired during the throw: {fired:?} in {begun}..{over}");
    assert!(fired.iter().any(|&f| f > over), "fires again after switching back");
}

/// Case 68: chasing with the target visible from the weapon, each round
/// leaves along the torso facing (toward the target's head), never 100 u
/// sideways.
#[test]
fn case_68_blind_chase_fire() {
    let (mut sim, p, bt) = duel(Level::Easy, 0, 600.0, RIFLE_KIT);
    // A nearer enemy behind a wall, so the sight pass does not hand the
    // bot to the attack goal.
    let p2 = human(&mut sim, ALPHA);
    tough(&mut sim, p2);
    wall(&mut sim, Vec3::new(u(140.0), 0.0, u(400.0)), Vec3::new(u(160.0), 3.0, u(700.0)));
    place(&mut sim, p, Vec3::ZERO, FACE_PLUS_Z);
    place(&mut sim, p2, Vec3::new(u(300.0), 0.0, u(500.0)), FACE_PLUS_Z);
    place(&mut sim, bt, Vec3::new(0.0, 0.0, u(600.0)), 0.0);
    sim.app.world_mut().resource_mut::<BotSettings>().stop = 0;
    sim.ticks(2);
    {
        let mut bot = b_mut(&mut sim, bt);
        bot.state = BotState::Chase { until: 1e9, breath: 0.0 };
        bot.target = Some(p);
        bot.visible = true;
        bot.cycle = HitCycle::default();
    }
    let mut shots = 0;
    // Until just before the target comes nearer than the walled-off enemy
    // (the bot walks 0.067 m a tick toward it; the two are level about 78
    // ticks in, and the sight pass then hands the bot to the attack goal).
    for _ in 0..72 {
        sim.ticks(1);
        let head = sim.position(p) + Vec3::Y * brain::HEAD;
        let w = sim.app.world_mut();
        let events: Vec<WeaponEvent> = w.resource_mut::<Messages<WeaponEvent>>().drain().collect();
        for e in events.iter().filter(|e| e.owner == bt) {
            if let WeaponEventKind::Fired { origin, yaw, pitch, .. } = e.kind {
                let (ey, ep) = brain::look_at(origin, head);
                let dy = (yaw - ey + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                assert!(dy.abs() < 0.03 && (pitch - ep).abs() < 0.03, "along the torso: {dy} {}", pitch - ep);
                shots += 1;
            }
        }
        let bot = b(&sim, bt);
        assert!(matches!(bot.state, BotState::Chase { .. }), "{:?}", bot.state);
        assert_eq!(bot.cycle, HitCycle::default(), "no hit/miss cycle");
    }
    assert!(shots >= 4, "fired while chasing: {shots}");
}

/// Case 69: a wall 20 u in front of the target, its top 52 u: the target's
/// `Torso` node is visible from the weapon (not head-only) though the eye
/// only sees the head; at 60 u, head-only.
#[test]
fn case_69_base_point() {
    assert!((brain::TORSO - 5.14 * UNIT).abs() < 1e-6);
    for (top, head_only) in [(52.0f32, false), (60.0, true)] {
        let (mut sim, p, bt) = duel(Level::Easy, 0, 500.0, RIFLE_KIT);
        wall(&mut sim, Vec3::new(-3.0, 0.0, u(19.0)), Vec3::new(3.0, u(top), u(21.0)));
        assert_eq!(brain::sees(sim.app.world_mut(), bt, p), Some(true), "the eye sees only the head");
        until(&mut sim, 3.0, |s| matches!(b(s, bt).state, BotState::Attack { .. })).expect("attacks");
        sim.seconds(0.3);
        assert_eq!(b(&sim, bt).head_only, head_only, "wall top {top} u");
    }
}

// bots_objectives.md 25-28: mercenaries aim like PvE AI.

/// A duel whose bot is a mercenary (its hirer the player), the gun's
/// accuracy `a`.
fn merc_duel(a: f32, d: f32) -> (Sim, Entity, Entity) {
    let (mut sim, p, bt) = duel(Level::Normal, 0, d, RIFLE_KIT);
    {
        let w = sim.app.world_mut();
        let ty = w.resource::<BotData>().gun_type(RIFLE).unwrap().to_string();
        w.resource_mut::<BotData>().guns.get_mut(&ty).unwrap().a = bots::data::PerTemplate::One(a);
        w.entity_mut(bt).insert(combat_arms::game::rules::mercs::CaMerc {
            leader: p,
            unit: 0,
            died: None,
        });
    }
    {
        let mut bot = b_mut(&mut sim, bt);
        bot.merc = Some(MercMind::new(p));
        bot.cycle = HitCycle::default();
    }
    (sim, p, bt)
}

/// The aim offsets of the merc's next `n` shots, `c` held each tick.
fn merc_offsets(sim: &mut Sim, bt: Entity, c: f32, n: usize) -> Vec<Vec3> {
    let mut out = Vec::new();
    let mut had = false;
    // Past the target's selection (which sets c to its cap).
    for _ in 0..40 {
        b_mut(sim, bt).tracking.c = c;
        sim.ticks(1);
    }
    {
        let mut bot = b_mut(sim, bt);
        bot.aim_offset = None;
        bot.cycle = HitCycle::default();
    }
    for _ in 0..3000 {
        b_mut(sim, bt).tracking.c = c;
        sim.ticks(1);
        let o = b(sim, bt).aim_offset;
        if let Some(o) = o
            && !had
        {
            out.push(o);
        }
        had = o.is_some();
        if out.len() >= n {
            break;
        }
    }
    out
}

/// Bots_objectives 25: h = 1, m = 3: shots 1, 5, 9 are aimed (on the base
/// point once c is 0), the rest miss 100 u to the side.
#[test]
fn merc_25_hit_order() {
    let (mut sim, _p, bt) = merc_duel(0.25, 800.0);
    let o = merc_offsets(&mut sim, bt, 0.0, 9);
    assert_eq!(o.len(), 9);
    for (i, off) in o.iter().enumerate() {
        if i % 4 == 0 {
            assert!(off.length() < 1e-3, "shot {} aimed: {off}", i + 1);
        } else {
            assert!(off.xz().length() > 90.0, "shot {} a miss: {off}", i + 1);
        }
    }
}

/// 26: with the target moving (c above 0) every aimed shot is exactly
/// 100 u from the base point.
#[test]
fn merc_26_aim_under_movement() {
    let (mut sim, _p, bt) = merc_duel(0.25, 800.0);
    let o = merc_offsets(&mut sim, bt, 0.3, 9);
    for i in [0, 4, 8] {
        assert!((o[i].length() - 100.0).abs() < 0.01, "shot {}: {}", i + 1, o[i].length());
    }
}

/// 27: the aim point starts 25 u above the hull centre; each update the
/// weapon's ray to it fails it steps down by an eighth of the hull.
#[test]
fn merc_27_base_point() {
    let (mut sim, p, bt) = merc_duel(0.25, 800.0);
    // Open: the first aimed shot is at 25 u above the centre.
    let mut seen = false;
    for _ in 0..40 {
        b_mut(&mut sim, bt).tracking.c = 0.0;
        sim.ticks(1);
    }
    {
        let mut bot = b_mut(&mut sim, bt);
        bot.aim_offset = None;
        bot.cycle = HitCycle::default();
    }
    for _ in 0..600 {
        b_mut(&mut sim, bt).tracking.c = 0.0;
        sim.ticks(1);
        let bot = b(&sim, bt);
        if let Some(off) = bot.aim_offset {
            let eye = sim.position(bt) + Vec3::Y * brain::HEAD;
            let want = brain::look_at(eye, sim.position(p) + Vec3::Y * 25.0 * UNIT);
            let intent = sim.app.world().get::<chimera_core::core::Intent>(bt).unwrap();
            assert!(off.length() < 1e-3);
            assert!((intent.pitch - want.1).abs() < 1e-3, "{} {}", intent.pitch, want.1);
            seen = true;
            break;
        }
    }
    assert!(seen);
    assert_eq!(b(&sim, bt).merc_height, 25.0);
    // Behind a wall 76 u high at 20 u in front: the ray to 75 u fails and
    // the height steps down 12.5 u a frame, wrapping from the bottom.
    let (mut sim, _p, bt) = merc_duel(0.25, 500.0);
    wall(&mut sim, Vec3::new(-3.0, 0.0, u(19.0)), Vec3::new(3.0, u(76.0), u(21.0)));
    let mut heights = vec![25.0f32];
    for _ in 0..200 {
        sim.ticks(1);
        let h = b(&sim, bt).merc_height;
        if heights.last() != Some(&h) {
            heights.push(h);
        }
    }
    assert_eq!(heights, vec![25.0, 12.5, 0.0, -12.5, -25.0, -37.5, -50.0, 50.0]);
}

/// 28: a mercenary's round does body damage wherever it lands: its arm
/// hits (zone factor 0.9) do 1/0.9 of what a bot's do (the head's factor 6
/// is the same rule; its sphere is too small to aim at reliably).
#[test]
fn merc_28_damage() {
    assert_eq!(combat_arms::game::weapons::male("Right_arml").factor, 0.9);
    let mean_arm_hit = |merc: bool| {
        let (mut sim, _p, bt) = merc_duel(1.0, 500.0);
        if !merc {
            sim.app.world_mut().entity_mut(bt).remove::<combat_arms::game::rules::mercs::CaMerc>();
            b_mut(&mut sim, bt).merc = None;
        }
        let mut arm = Vec::new();
        for _ in 0..6000 {
            b_mut(&mut sim, bt).tracking.c = 0.0;
            sim.ticks(1);
            let w = sim.app.world_mut();
            for e in w.resource_mut::<Messages<WeaponEvent>>().drain() {
                if let WeaponEventKind::Hit { amount, hitgroup, .. } = e.kind
                    && e.owner == bt
                    && matches!(hitgroup, Hitgroup::LeftArm | Hitgroup::RightArm)
                {
                    arm.push(amount);
                }
            }
            if arm.len() >= 60 {
                break;
            }
        }
        assert!(arm.len() >= 40, "merc {merc}: {} arm hits", arm.len());
        arm.iter().sum::<f32>() / arm.len() as f32
    };
    let m = mean_arm_hit(true);
    let bot = mean_arm_hit(false);
    assert!((m / bot - 1.0 / 0.9).abs() < 0.06, "mercenary {m}, bot {bot}: {}", m / bot);
}
