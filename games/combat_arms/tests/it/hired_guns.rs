//! Hired Guns (specs/combat_arms/game_modes_2.md M11): its test cases
//! 1-19 on `CaSim` and the greybox with the common setup (`mode_kit`):
//! Blood Money's objects scaled into the greybox (84 `SquadMatch` drop
//! points, the two banks with the stock boxes, the two unit spawns), and
//! the mercenaries' cases of bots_objectives.md (8-24) that need a
//! Hired Guns match.

use bevy::prelude::*;
use chimera_core::core::{DamageKind, Intent, MovementState};
use chimera_world::harness::Sim;
use combat_arms::game::{
    bots::{BotSettings, CaBot},
    inventory::UNIT,
    movement::{CaMovement, CustomMovePenalty},
    rules::{
        CaMatch, Phase,
        hired_guns::CaBank,
        mercs::{self, CaMerc, CaSquadOrder, MercOrder},
        objectives::{Bullion, CaCarry, CaObjectives},
        objects::{BankTrigger, NamedPoint},
        scoreboard::{ScoreRow, sort_by},
    },
    weapons::CaAiDamage,
};

use crate::mode_kit::*;

/// Alpha's bank (`SaveZone_A`) and Bravo's, their centres (m) and the
/// stock box (128, 160, 128 u).
pub const BANK_A: Vec3 = Vec3::new(25.0, 1.0, 25.0);
pub const BANK_B: Vec3 = Vec3::new(-15.0, 1.0, 25.0);
/// The unit spawns.
pub const UNIT_A: Vec3 = Vec3::new(25.0, 1.0, 18.0);
pub const UNIT_B: Vec3 = Vec3::new(-15.0, 1.0, 18.0);

fn bank(team: u8, centre: Vec3) -> BankTrigger {
    BankTrigger {
        name: if team == JSF {
            "SaveZone_A".into()
        } else {
            "SaveZone_B".into()
        },
        team: Some(team),
        area: obj_box(centre, Vec3::new(128.0, 160.0, 128.0)),
    }
}

fn named(name: &str, at: Vec3) -> NamedPoint {
    NamedPoint {
        class: "BaseClass".into(),
        name: name.into(),
        at: Transform::from_translation(at),
    }
}

/// The greybox with Blood Money's objects, playing Hired Guns.
pub fn hg_game(goal: u32) -> Sim {
    let mut sim = game("SQM", 8);
    settings(&mut sim).goal = goal;
    let mut o = objects(&mut sim);
    o.banks = vec![bank(JSF, BANK_A), bank(CREED, BANK_B)];
    o.points = vec![named("UNITSPAWN_ALPHA", UNIT_A), named("UNITSPAWN_BRAVO", UNIT_B)];
    o.drops = (0..84)
        .map(|i| {
            drop_point(
                "SquadMatch",
                Vec3::new(-5.0 + (i % 12) as f32 * 1.5, 0.2, 16.0 + (i / 12) as f32 * 1.2),
            )
        })
        .collect();
    sim
}

/// Hired Guns past its warm-up with `alpha` and `bravo` players, all
/// unprotected.
pub fn hg(alpha: usize, bravo: usize) -> (Sim, Vec<Entity>, Vec<Entity>) {
    let mut sim = hg_game(3);
    let a: Vec<Entity> = (0..alpha).map(|_| join(&mut sim, JSF)).collect();
    let b: Vec<Entity> = (0..bravo).map(|_| join(&mut sim, CREED)).collect();
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    sim.ticks(2);
    (sim, a, b)
}

pub fn bank_of(sim: &Sim, p: Entity) -> i32 {
    sim.app.world().get::<CaBank>(p).map_or(0, |b| b.0)
}

fn pocket(sim: &Sim, p: Entity) -> (u32, i32) {
    let c = carry(sim, p);
    (c.bars, c.money)
}

/// Give `p` a pocket of `bars` bars (100 each).
pub fn give_pocket(sim: &mut Sim, p: Entity, bars: u32) {
    let mut c = carry(sim, p);
    c.bars = bars;
    c.money = bars as i32 * 100;
    sim.app.world_mut().entity_mut(p).insert(c);
}

/// The bullion on the ground replaced by `items`.
fn set_bullion(sim: &mut Sim, items: Vec<Bullion>) {
    sim.app.world_mut().resource_mut::<CaObjectives>().bullion = items;
}

fn bar(id: u32, at: Vec3) -> Bullion {
    Bullion {
        id,
        at,
        bars: 1,
        money: 100,
        bag: false,
    }
}

/// Aim `p` (standing at `from`) at `at`.
fn aim(sim: &mut Sim, p: Entity, from: Vec3, at: Vec3) {
    let eye = from
        + sim
            .app
            .world()
            .get::<MovementState>(p)
            .map_or(Vec3::ZERO, |s| s.eye_offset);
    let d = at - eye;
    let h = Vec2::new(d.x, d.z).length();
    let i = &mut *sim.intent(p);
    i.yaw = (-d.x).atan2(-d.z);
    i.pitch = d.y.atan2(h);
}

/// `p` at `from` presses use once while aiming at `at`.
fn use_at(sim: &mut Sim, p: Entity, from: Vec3, at: Vec3) {
    put(sim, p, from);
    aim(sim, p, from, at);
    sim.intent(p).use_key = false;
    hold_at(sim, p, from, 0.05);
    aim(sim, p, from, at);
    sim.intent(p).use_key = true;
    hold_at(sim, p, from, 0.05);
    sim.intent(p).use_key = false;
}

fn hire(sim: &mut Sim, p: Entity, unit: usize) -> Result<Entity, String> {
    mercs::hire(sim.app.world_mut(), p, unit)
}

/// 1. The real round's start: 80 bars on 80 distinct drop points of 84;
/// banked 0 - 0, nothing in bags, 80 unclaimed; every bank 1000, nobody
/// carries bars. In the warm-up before: no bars, banks 0, a hire refused.
#[test]
fn case_01_round_start() {
    let mut sim = hg_game(3);
    let a = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    until_live(&mut sim);
    assert!(m(&sim).warmup);
    assert!(objectives(&sim).bullion.is_empty(), "no bars in the warm-up");
    assert_eq!((bank_of(&sim, a), bank_of(&sim, b)), (0, 0));
    assert!(hire(&mut sim, a, 0).is_err(), "bank 0 < 200");
    until(&mut sim, 20.0, |p| matches!(p, Phase::Setup { .. }));
    until_live(&mut sim);
    let o = objectives(&sim);
    assert_eq!((o.placed, o.bullion.len()), (80, 80));
    let mut spots: Vec<[i32; 3]> = o
        .bullion
        .iter()
        .map(|b| (b.at * 100.0).round().as_ivec3().to_array())
        .collect();
    spots.sort();
    spots.dedup();
    assert_eq!(spots.len(), 80, "80 distinct points");
    assert_eq!(o.banked, [0, 0]);
    assert_eq!(o.bars_on_ground(), (80, 0));
    assert_eq!((bank_of(&sim, a), bank_of(&sim, b)), (1000, 1000));
    assert_eq!((pocket(&sim, a), pocket(&sim, b)), ((0, 0), (0, 0)));
}

/// 2. A level bar by use: 199 u away → pocket 1 bar, 100; 79 unclaimed of
/// the round's. 201 u → nothing.
#[test]
fn case_02_bar_by_use() {
    let (mut sim, a, _b) = hg(1, 1);
    let a = a[0];
    let from = Vec3::new(0.0, 1.0, 28.0);
    let near = from + Vec3::new(u(199.0), 0.0, 0.0);
    let far = from + Vec3::new(-u(201.0), 0.0, 0.0);
    set_bullion(&mut sim, vec![bar(9001, near), bar(9002, far)]);
    use_at(&mut sim, a, from, far);
    assert_eq!(pocket(&sim, a), (0, 0), "201 u: out of reach");
    use_at(&mut sim, a, from, near);
    assert_eq!(pocket(&sim, a), (1, 100));
    assert_eq!(objectives(&sim).bars_on_ground(), (1, 0));
    // Not by touch: standing on the far bar takes nothing.
    hold_at(&mut sim, a, far + Vec3::Y * 0.9, 0.2);
    assert_eq!(pocket(&sim, a), (1, 100));
}

/// 3. A bag of 12 bars taken by an enemy walking over it: +12 bars,
/// +1200; the bags' count 12 → 0.
#[test]
fn case_03_bag_by_touch() {
    let (mut sim, _a, b) = hg(1, 1);
    let b = b[0];
    let at = Vec3::new(u(100.0), 0.0, 20.0 + u(200.0));
    set_bullion(
        &mut sim,
        vec![Bullion {
            id: 7,
            at,
            bars: 12,
            money: 1200,
            bag: true,
        }],
    );
    assert_eq!(objectives(&sim).bars_on_ground().1, 12);
    hold_at(&mut sim, b, at + Vec3::Y * 0.9, 0.1);
    assert_eq!(pocket(&sim, b), (12, 1200));
    assert_eq!(objectives(&sim).bars_on_ground().1, 0);
}

/// 4. The weight: 10, 11, 20, 25, 30 bars → P_custom 0, 0.03, 0.30,
/// 0.45, 0.45; an M4A1 and the default vest (P 0.2) with 25 bars → 0.65:
/// 285 x 0.35 = 99.75 u/s.
#[test]
fn case_04_weight() {
    let (mut sim, a, _b) = hg(1, 1);
    let a = a[0];
    let from = Vec3::new(0.0, 1.0, 28.0);
    let mut last = 0;
    for (n, p) in [(10u32, 0.0f32), (11, 0.03), (20, 0.30), (25, 0.45), (30, 0.45)] {
        set_bullion(
            &mut sim,
            vec![Bullion {
                id: 100 + n,
                at: from + Vec3::new(0.0, -0.9, 0.0),
                bars: n - last,
                money: (n - last) as i32 * 100,
                bag: true,
            }],
        );
        hold_at(&mut sim, a, from, 0.1);
        last = n;
        assert_eq!(pocket(&sim, a).0, n);
        let got = sim.app.world().get::<CustomMovePenalty>(a).map_or(0.0, |c| c.0);
        assert!((got - p).abs() < 1e-5, "{n} bars: {got}");
    }
    // 30 bars (P_custom 0.45, as 25), the rifle in hand: the speed limit.
    sim.intent(a).move_axis = Vec2::Y;
    sim.ticks(10);
    let limit = sim.app.world().get::<CaMovement>(a).unwrap().speed_limit;
    assert!((limit - 99.75).abs() < 0.5, "run speed {limit} u/s");
}

/// 5. Banking: an Alpha carrier of 7 bars along z = the bank's centre:
/// at |dx| = 150 u nothing; at 140 u (< 143) banked on that frame: bank
/// +700, mission points +700, Alpha banked 7, pocket 0, P_custom 0. A
/// Bravo carrier inside Alpha's bank: nothing.
#[test]
fn case_05_banking() {
    let (mut sim, a, b) = hg(1, 1);
    let (a, b) = (a[0], b[0]);
    give_pocket(&mut sim, a, 7);
    let before = bank_of(&sim, a);
    hold_at(&mut sim, a, BANK_A - Vec3::new(u(150.0), 0.0, 0.0), 0.1);
    assert_eq!(pocket(&sim, a), (7, 700), "150 u: not yet");
    hold_at(&mut sim, a, BANK_A - Vec3::new(u(140.0), 0.0, 0.0), 0.05);
    assert_eq!(pocket(&sim, a), (0, 0));
    assert_eq!(bank_of(&sim, a), before + 700);
    assert_eq!(stats(&sim, a).mission_points, 700);
    assert_eq!(objectives(&sim).banked, [7, 0]);
    assert!(sim.app.world().get::<CustomMovePenalty>(a).is_none());
    give_pocket(&mut sim, b, 3);
    hold_at(&mut sim, b, BANK_A, 0.2);
    assert_eq!(pocket(&sim, b), (3, 300), "the enemy's bank takes nothing");
    assert_eq!(objectives(&sim).banked, [7, 0]);
}

/// 6. A carrier of 12 bars killed by a Bravo rifle: a bag of 12 50 u
/// above the body, dropped to the floor; Alpha's pockets −12; the killer's
/// bank and mission points +100, Bravo's round kills +1, Alpha's round
/// deaths +1.
#[test]
fn case_06_death_drop() {
    let (mut sim, a, b) = hg(1, 1);
    let (a, b) = (a[0], b[0]);
    let at = Vec3::new(u(100.0) * 0.5, 1.0, 20.0 + u(200.0) * 0.5);
    put(&mut sim, a, at);
    give_pocket(&mut sim, a, 12);
    sim.ticks(1);
    assert_eq!(objectives(&sim).in_pockets, [12, 0]);
    let body = sim.position(a);
    let bank_b = bank_of(&sim, b);
    kill_now(&mut sim, a, b);
    let o = objectives(&sim);
    let bag = o.bullion.iter().find(|x| x.bag).expect("a bag");
    assert_eq!((bag.bars, bag.money), (12, 1200));
    assert!((bag.at.xz() - body.xz()).length() < 1e-3);
    assert!(bag.at.y.abs() < 0.05, "on the floor: {}", bag.at.y);
    assert_eq!(o.in_pockets, [0, 0]);
    assert_eq!(bank_of(&sim, b), bank_b + 100);
    assert_eq!(stats(&sim, b).mission_points, 100);
    let mm = m(&sim);
    assert_eq!((mm.round_kills, mm.round_deaths), ([0, 1], [1, 0]));
}

/// 7. The same death 1 s after the round was decided: no bag, no counts.
#[test]
fn case_07_death_in_round_over() {
    let (mut sim, a, b) = hg(1, 1);
    let (a, b) = (a[0], b[0]);
    give_pocket(&mut sim, a, 12);
    let t = now(&sim);
    sim.app.world_mut().resource_mut::<CaMatch>().round_ends = Some(t + 0.05);
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    sim.ticks(66);
    let bank_b = bank_of(&sim, b);
    kill_now(&mut sim, a, b);
    assert!(dead(&sim, a));
    assert!(objectives(&sim).bullion.iter().all(|x| !x.bag), "no bag");
    assert_eq!(bank_of(&sim, b), bank_b);
    assert_eq!(m(&sim).round_kills, [0, 0]);
}

/// 8. A's mercenary (`Squad_7`, unit 6) kills Bravo player B: A kills +1,
/// bank +100, mission points +100, Alpha's round kills +1; B deaths +1,
/// Bravo's round deaths +1.
#[test]
fn case_08_kill_by_mercenary() {
    let (mut sim, a, b) = hg(1, 1);
    let merc = hire(&mut sim, a[0], 6).unwrap();
    let bank = bank_of(&sim, a[0]);
    kill_now(&mut sim, b[0], merc);
    let s = stats(&sim, a[0]);
    assert_eq!((s.kills, s.mission_points), (1, 100));
    assert_eq!(bank_of(&sim, a[0]), bank + 100);
    assert_eq!(stats(&sim, b[0]).deaths, 1);
    let mm = m(&sim);
    assert_eq!((mm.round_kills, mm.round_deaths), ([1, 0], [0, 1]));
}

/// 9. A Bravo player kills A's mercenary: kills +1, bank +100, Bravo's
/// round kills +1; nobody's deaths change; A may hire again.
#[test]
fn case_09_mercenary_killed() {
    let (mut sim, a, b) = hg(1, 1);
    let m1 = hire(&mut sim, a[0], 0).unwrap();
    let _m2 = hire(&mut sim, a[0], 1).unwrap();
    assert!(hire(&mut sim, a[0], 2).is_err(), "two alive");
    let bank = bank_of(&sim, b[0]);
    kill_now(&mut sim, m1, b[0]);
    assert_eq!(stats(&sim, b[0]).kills, 1);
    assert_eq!(bank_of(&sim, b[0]), bank + 100);
    assert_eq!(m(&sim).round_kills, [0, 1]);
    assert_eq!((stats(&sim, a[0]).deaths, stats(&sim, b[0]).deaths), (0, 0));
    assert_eq!(m(&sim).round_deaths, [0, 0]);
    assert!(hire(&mut sim, a[0], 2).is_ok(), "the slot is free");
    // The body goes 2 s after the death.
    assert!(sim.app.world().get_entity(m1).is_ok());
    sim.ticks(140);
    assert!(sim.app.world().get_entity(m1).is_err(), "the body is gone");
}

/// 10. Hiring: bank 1000, unit 4 → 500 and a unit at `UNITSPAWN_ALPHA` on
/// Alpha; unit 0 → 300; a third → refused (2 alive), bank 300. Bank 150,
/// unit 0 → refused. (bots_objectives.md 8-10.)
#[test]
fn case_10_hiring() {
    let (mut sim, a, _b) = hg(1, 1);
    assert_eq!(bank_of(&sim, a[0]), 1000);
    let u4 = hire(&mut sim, a[0], 4).unwrap();
    assert_eq!(bank_of(&sim, a[0]), 500);
    assert!((sim.position(u4) - UNIT_A).length() < 1e-3, "at the unit spawn");
    assert_eq!(team(&sim, u4), JSF);
    let merc = *sim.app.world().get::<CaMerc>(u4).unwrap();
    assert_eq!((merc.leader, merc.unit), (a[0], 4));
    hire(&mut sim, a[0], 0).unwrap();
    assert_eq!(bank_of(&sim, a[0]), 300);
    assert!(hire(&mut sim, a[0], 9).is_err());
    assert_eq!(bank_of(&sim, a[0]), 300);
    sim.app.world_mut().entity_mut(a[0]).insert(CaBank(150));
    mercs::remove_of(sim.app.world_mut(), a[0]);
    assert!(hire(&mut sim, a[0], 0).is_err());
    assert_eq!(bank_of(&sim, a[0]), 150);
    // Quirk 4: no unit spawn object → the money is spent, no unit.
    sim.app.world_mut().entity_mut(a[0]).insert(CaBank(1000));
    objects(&mut sim).points.clear();
    assert!(hire(&mut sim, a[0], 0).is_err());
    assert_eq!(bank_of(&sim, a[0]), 800);
    assert!(mercs::living_units(sim.app.world_mut(), a[0]).is_empty());
}

/// 11. A mercenary's rifle hit of 80 on a player takes 50 at most (the
/// cap is every AI's: `CaAiDamage`); a teammate's mercenary does nothing,
/// friendly fire on or off (Quirk 12).
#[test]
fn case_11_mercenary_damage() {
    let (mut sim, a, b) = hg(2, 1);
    settings(&mut sim).friendly_fire = 1;
    let merc = hire(&mut sim, a[0], 0).unwrap();
    // The factor (the objective's, 1.0 on Blood Money) and the cap are
    // every AI's, applied where a hit's damage is worked out
    // (`weapons::AiHits`; weapon_firing.md Corrections 7): an 80 hit is
    // 50, a melee one isn't capped.
    let ai = *sim.app.world().resource::<CaAiDamage>();
    assert_eq!(ai.factor, 1.0, "Hired Guns' AIDamageFactor");
    assert_eq!((ai.capped(80.0, false), ai.capped(45.0, true)), (50.0, 45.0));
    hit(
        &mut sim,
        b[0],
        Some(merc),
        0.5,
        DamageKind::Bullet,
        Some(combat_arms::game::weapons::RIFLE),
    );
    sim.ticks(1);
    assert!((health(&sim, b[0]) - 0.5).abs() < 1e-4, "{}", health(&sim, b[0]));
    hit(
        &mut sim,
        a[1],
        Some(merc),
        0.8,
        DamageKind::Bullet,
        Some(combat_arms::game::weapons::RIFLE),
    );
    hit(
        &mut sim,
        merc,
        Some(a[1]),
        0.8,
        DamageKind::Bullet,
        Some(combat_arms::game::weapons::RIFLE),
    );
    sim.ticks(1);
    assert_eq!(health(&sim, a[1]), 1.0);
    assert_eq!(health(&sim, merc), 1.0);
}

/// 12. "Follow me", the hirer standing 250 u away from a holding unit:
/// the unit moves about 0.25 s later and stops within 200 u.
#[test]
fn case_12_follow() {
    let (mut sim, a, _b) = hg(1, 1);
    let merc = hire(&mut sim, a[0], 0).unwrap();
    mercs::set_order(sim.app.world_mut(), a[0], MercOrder::FollowMe);
    let near = UNIT_A + Vec3::new(u(150.0), 0.0, 0.0);
    hold_at(&mut sim, a[0], near, 1.0);
    let start = sim.position(merc);
    let away = start + Vec3::new(u(250.0), 0.0, 0.0);
    put(&mut sim, a[0], away);
    hold_at(&mut sim, a[0], away, 0.15);
    assert!(
        sim.position(merc).distance(start) < 0.05,
        "not yet: {}",
        sim.position(merc).distance(start)
    );
    hold_at(&mut sim, a[0], away, 2.5);
    let d = sim.position(merc).distance(sim.position(a[0])) / UNIT;
    assert!(sim.position(merc).distance(start) > 0.2, "it moved");
    assert!(d < 205.0, "within 200 u: {d}");
}

/// 13. Bank money over rounds: A spends 200, kills 3 (+300), banks 5 bars
/// (+500): 1600 at the round over, Alpha wins; round 2 → 3100; a Bravo
/// player who did nothing → 2000; A's mission points 1300.
#[test]
fn case_13_bank_over_rounds() {
    let (mut sim, a, b) = hg(1, 3);
    sim.app.world_mut().resource_mut::<BotSettings>().dont_shoot = 1;
    hire(&mut sim, a[0], 0).unwrap();
    for v in &b {
        kill_now(&mut sim, *v, a[0]);
    }
    give_pocket(&mut sim, a[0], 5);
    hold_at(&mut sim, a[0], BANK_A, 0.05);
    assert_eq!(bank_of(&sim, a[0]), 1600);
    let t = now(&sim);
    sim.app.world_mut().resource_mut::<CaMatch>().round_ends = Some(t + 0.05);
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).winner, Some(JSF));
    assert_eq!(bank_of(&sim, a[0]), 1600);
    assert_eq!(stats(&sim, a[0]).mission_points, 1300);
    assert!(
        mercs::all(sim.app.world_mut()).is_empty(),
        "AI removed at the round over"
    );
    until(&mut sim, 10.0, |p| matches!(p, Phase::Setup { .. }));
    sim.ticks(1);
    assert_eq!(bank_of(&sim, a[0]), 3100);
    assert_eq!(bank_of(&sim, b[0]), 2000);
}

/// 14. Time-out winners: banked 30 - 30, kills 12 - 10 → Alpha; banked
/// 30 - 30, kills 10 - 10, deaths 9 - 11 → Alpha; all equal → no winner,
/// no round win, no bonus.
#[test]
fn case_14_time_out() {
    for (kills, deaths, want) in [
        ([12, 10], [0, 0], Some(JSF)),
        ([10, 10], [9, 11], Some(JSF)),
        ([5, 5], [5, 5], None),
    ] {
        let (mut sim, a, _b) = hg(1, 1);
        {
            let w = sim.app.world_mut();
            w.resource_mut::<CaObjectives>().banked = [30, 30];
            let t = w.resource::<chimera_core::core::SimClock>().now;
            let mut m = w.resource_mut::<CaMatch>();
            m.round_kills = kills;
            m.round_deaths = deaths;
            m.round_ends = Some(t + 0.05);
        }
        until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
        let mm = m(&sim);
        assert_eq!(mm.winner, want);
        let wins: u32 = mm.round_wins.iter().sum();
        assert_eq!(wins, want.is_some() as u32);
        assert_eq!(
            stats(&sim, a[0]).mission_points,
            if want == Some(JSF) { 500 } else { 0 }
        );
    }
}

/// 15. The early end: Alpha banks the last carried bars so that banked is
/// 50 + 30 = 80 = placed with nothing on the ground: round over at the
/// next frame, Alpha the winner.
#[test]
fn case_15_early_end() {
    let (mut sim, a, _b) = hg(1, 1);
    set_bullion(&mut sim, Vec::new());
    sim.app.world_mut().resource_mut::<CaObjectives>().banked = [45, 30];
    give_pocket(&mut sim, a[0], 5);
    hold_at(&mut sim, a[0], BANK_A, 0.02);
    assert_eq!(objectives(&sim).banked, [50, 30]);
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).winner, Some(JSF));
}

/// 16. The match end: G 3, round wins 1 - 1 after 3 rounds (one draw) →
/// Bravo. A team emptied after round 1: the match ends after its
/// intermission, won by the other team.
#[test]
fn case_16_match_end() {
    let (mut sim, _a, _b) = hg(1, 1);
    for banked in [[2, 1], [1, 2], [1, 1]] {
        let w = sim.app.world_mut();
        w.resource_mut::<CaObjectives>().banked = banked;
        let t = w.resource::<chimera_core::core::SimClock>().now;
        w.resource_mut::<CaMatch>().round_ends = Some(t + 0.05);
        until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
        until(&mut sim, 10.0, |p| matches!(p, Phase::Live | Phase::MatchOver));
    }
    let mm = m(&sim);
    assert_eq!(mm.round_wins, [1, 1]);
    assert_eq!(mm.result, Some(Some(CREED)));

    let (mut sim, _a, b) = hg(1, 1);
    sim.app.world_mut().despawn(b[0]);
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    until(&mut sim, 10.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).result, Some(Some(JSF)));
}

/// 17. A leaver with 4 bars and 2 mercenaries: a bag of 4 where it was,
/// both units gone. The last Alpha player leaving: the round asks to end
/// and Bravo wins the match. (bots_objectives.md 24.)
#[test]
fn case_17_leaving_carrier() {
    let (mut sim, a, _b) = hg(2, 1);
    hire(&mut sim, a[0], 0).unwrap();
    hire(&mut sim, a[0], 1).unwrap();
    give_pocket(&mut sim, a[0], 4);
    let at = sim.position(a[0]);
    sim.app.world_mut().despawn(a[0]);
    sim.ticks(1);
    let o = objectives(&sim);
    let bag = o.bullion.iter().find(|x| x.bag).expect("a bag");
    assert_eq!(bag.bars, 4);
    assert!((bag.at.xz() - at.xz()).length() < 1e-3);
    assert!(mercs::all(sim.app.world_mut()).is_empty(), "its mercenaries went");
    sim.app.world_mut().despawn(a[1]);
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    until(&mut sim, 10.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).result, Some(Some(CREED)));
}

/// 18. A carrier of 5 steps into its bank on the frame an enemy rifle
/// kills it: the bag drops in the bank, nothing is banked.
#[test]
fn case_18_bank_and_death_same_frame() {
    let (mut sim, a, b) = hg(1, 1);
    give_pocket(&mut sim, a[0], 5);
    put(&mut sim, a[0], BANK_A);
    kill_now(&mut sim, a[0], b[0]);
    let o = objectives(&sim);
    assert_eq!(o.banked, [0, 0], "nothing banked");
    let bag = o.bullion.iter().find(|x| x.bag).expect("a bag");
    assert_eq!(bag.bars, 5);
    assert!(
        obj_box(BANK_A, Vec3::new(128.0, 160.0, 128.0)).contains(bag.at, 0.0),
        "in the bank"
    );
}

/// 19. The scoreboard: mission points 1300 / 1300 / 900 with kills 3 / 5
/// / 9 → the 5-kill player, the 3-kill player, the 900-point player.
#[test]
fn case_19_scoreboard_sort() {
    let row = |n: u64, mp: i32, kills: u32| ScoreRow {
        player: Entity::from_bits(n + (1 << 32)),
        name: n.to_string(),
        team: 1,
        kills,
        deaths: 0,
        assists: 0,
        captures: 0,
        returns: 0,
        detonations: 0,
        defuses: 0,
        mission_points: mp,
        hunted_ms: 0,
        ping: None,
    };
    let mut rows = vec![row(1, 1300, 3), row(2, 1300, 5), row(3, 900, 9)];
    let order = combat_arms::game::rules::mode_by_key("SQM").unwrap().order;
    sort_by(&mut rows, order);
    assert_eq!(rows.iter().map(|r| r.kills).collect::<Vec<_>>(), [5, 3, 9]);
}

/// bots_objectives.md 11: a mercenary walking over bars leaves them.
#[test]
fn mercenaries_ignore_bullion() {
    let (mut sim, a, _b) = hg(1, 1);
    let merc = hire(&mut sim, a[0], 0).unwrap();
    let at = sim.position(merc);
    set_bullion(
        &mut sim,
        vec![
            Bullion {
                id: 1,
                at: at - Vec3::Y * 0.9,
                bars: 3,
                money: 300,
                bag: true,
            },
            bar(2, at),
        ],
    );
    let bank = bank_of(&sim, a[0]);
    sim.ticks(30);
    assert_eq!(objectives(&sim).bullion.len(), 2, "the bars stay");
    assert_eq!(bank_of(&sim, a[0]), bank);
    assert!(sim.app.world().get::<CaCarry>(merc).is_none());
}

/// bots_objectives.md 17, 20: under "follow me" a unit never fires, even
/// shot at in plain view; a unit hired later takes the order.
#[test]
fn follow_me_never_fires() {
    let (mut sim, a, b) = hg(1, 1);
    let m1 = hire(&mut sim, a[0], 0).unwrap();
    mercs::set_order(sim.app.world_mut(), a[0], MercOrder::FollowMe);
    let m2 = hire(&mut sim, a[0], 1).unwrap();
    let mind = |sim: &Sim, e| sim.app.world().get::<CaBot>(e).unwrap().merc.unwrap();
    assert!(mind(&sim, m1).follow_only && mind(&sim, m2).follow_only);
    // An enemy in plain view, 500 u off.
    let at = sim.position(m1) + Vec3::new(0.0, 0.0, u(500.0));
    let mut fired = false;
    for _ in 0..200 {
        put(&mut sim, b[0], at);
        sim.ticks(1);
        fired |= sim.app.world().get::<Intent>(m1).unwrap().fire || sim.app.world().get::<Intent>(m2).unwrap().fire;
    }
    assert!(!fired, "follow me: no shot");
    assert_eq!(
        sim.app.world().get::<CaSquadOrder>(a[0]).copied().unwrap_or_default().0,
        MercOrder::FollowMe
    );
}

/// bots_objectives.md 15: with the default order a unit fights an enemy
/// it sees (the attack goal over the follow goal).
#[test]
fn default_order_fights() {
    let (mut sim, a, b) = hg(1, 1);
    let merc = hire(&mut sim, a[0], 0).unwrap();
    // In front of it (it faces −Z).
    let at = sim.position(merc) - Vec3::new(0.0, 0.0, u(400.0));
    let mut fired = false;
    for _ in 0..400 {
        put(&mut sim, b[0], at);
        sim.ticks(1);
        fired |= sim.app.world().get::<Intent>(merc).unwrap().fire;
        if fired {
            break;
        }
    }
    assert!(fired, "it attacks the enemy in view");
}

/// bots_objectives.md 18, 19: hold position keeps a unit at its spot; a
/// spot at the world origin means the leader (Quirk 4).
#[test]
fn hold_position() {
    // No enemy in reach: nothing to fight (the other team keeps a player, an
    // empty team ending the match, but far away).
    let (mut sim, a, b) = hg(1, 1);
    put(&mut sim, b[0], Vec3::new(900.0, 1.0, 900.0));
    let merc = hire(&mut sim, a[0], 0).unwrap();
    mercs::set_order(sim.app.world_mut(), a[0], MercOrder::Hold);
    let spot = sim.position(merc);
    hold_at(&mut sim, a[0], spot + Vec3::new(-u(2000.0) * 0.5, 0.0, 0.0), 3.0);
    let d = sim.position(merc).distance(spot) / UNIT;
    assert!(d < 200.0, "it stays at its spot: {d} u");
    let mind = sim.app.world().get::<CaBot>(merc).unwrap().merc.unwrap();
    assert!(mind.follow_point.is_some());
    // The world origin as the spot: the leader.
    let mut bot = sim.app.world_mut().get_mut::<CaBot>(merc).unwrap();
    bot.merc.as_mut().unwrap().follow_point = Some(Vec3::ZERO);
    let leader = Vec3::new(5.0, 1.0, 26.0);
    hold_at(&mut sim, a[0], leader, 8.0);
    let d = sim.position(merc).distance(leader) / UNIT;
    assert!(d < 260.0, "follows the leader: {d} u");
}

/// bots_objectives.md 23: a round over removes the units; the next round
/// the squad is empty and the order the default.
#[test]
fn round_over_removes_units() {
    let (mut sim, a, _b) = hg(1, 1);
    hire(&mut sim, a[0], 0).unwrap();
    hire(&mut sim, a[0], 1).unwrap();
    mercs::set_order(sim.app.world_mut(), a[0], MercOrder::FollowMe);
    let t = now(&sim);
    sim.app.world_mut().resource_mut::<CaMatch>().round_ends = Some(t + 0.05);
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert!(mercs::all(sim.app.world_mut()).is_empty());
    assert_eq!(
        sim.app.world().get::<CaSquadOrder>(a[0]).copied().unwrap_or_default().0,
        MercOrder::Default
    );
}
