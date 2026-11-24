//! TF2's batch-3 unlocks, part 1 (specs/tf2/batch3_mechanics.md, its test
//! cases): a made-up item schema in items_game's shape (the spec's
//! attribute values, nothing copied from the game) through the converter,
//! on the headless harness: the Demoman's shields and their charge (speed,
//! meter, recharge, turn cap, the crit boost by meter, the Tide Turner's
//! meter loss and kill refill, the boots, the Claidheamh Mòr, the
//! Skullcutter, the Eyelander's heads, the resistances, the impact's
//! stand-in) and the Soldier's banners (rage, the horn, the 450-unit
//! pulsed buff and each buff).

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Health, Hitgroup, SimClock, Team, Velocity};
use chimera_sim::{
    status::{Condition, Conditions},
    weapon::{Inventory, give},
};
use chimera_world::{greybox::{self, GreyboxMapPlugin}, harness::Sim};
use source_engine::source_movement::SourceTraits;
use tf2::{
    game::{
        TICK_INTERVAL,
        items::{ItemPack, convert, effects::Heads, meters::Tf2Meter, shields::Tf2Shield},
        weapons::{BOTTLE, SHOVEL, melee::Tf2Melee, script_numbers::GunNumbers},
    },
    harness::Tf2Sim,
};

use super::weapons::{hold_still, holding, run};

const U: f32 = 0.0254;

/// The attribute classes the made-up schema uses.
const DEFS: &[(&str, &str)] = &[
    ("mult_dmgtaken_from_explosions", "value_is_percentage"),
    ("mult_dmgtaken_from_fire", "value_is_percentage"),
    ("charge_recharge_rate", "value_is_percentage"),
    ("charge_impact_damage", "value_is_percentage"),
    ("charge_turn_control", "value_is_percentage"),
    ("kill_refills_meter", "value_is_additive"),
    ("lose_demo_charge_on_damage_when_charging", "value_is_additive"),
    ("mod_charge_time", "value_is_additive"),
    ("add_maxhealth", "value_is_additive"),
    ("mult_player_movespeed_shieldrequired", "value_is_percentage"),
    ("mult_player_movespeed", "value_is_percentage"),
    ("provide_on_active", "value_is_additive"),
    ("is_a_sword", "value_is_additive"),
    ("decapitate_type", "value_is_additive"),
    ("set_buff_type", "value_is_additive"),
    ("add_health_regen", "value_is_additive"),
];

/// (index, name, class, slot, used by, attributes).
#[allow(clippy::type_complexity)]
const ITEMS: &[(u32, &str, &str, &str, &str, &[(&str, &str)])] = &[
    (131, "The Chargin' Targe", "tf_wearable_demoshield", "secondary", "demoman", &[
        ("mult_dmgtaken_from_explosions", "0.7"), ("mult_dmgtaken_from_fire", "0.5"),
    ]),
    (406, "The Splendid Screen", "tf_wearable_demoshield", "secondary", "demoman", &[
        ("mult_dmgtaken_from_explosions", "0.8"), ("mult_dmgtaken_from_fire", "0.8"), ("charge_recharge_rate", "1.5"),
        ("charge_impact_damage", "1.7"),
    ]),
    (1099, "The Tide Turner", "tf_wearable_demoshield", "secondary", "demoman", &[
        ("mult_dmgtaken_from_explosions", "0.85"), ("mult_dmgtaken_from_fire", "0.85"), ("charge_turn_control", "50"),
        ("kill_refills_meter", "0.75"), ("lose_demo_charge_on_damage_when_charging", "1"),
    ]),
    (405, "Ali Baba's Wee Booties", "tf_wearable", "primary", "demoman", &[
        ("add_maxhealth", "25"), ("charge_turn_control", "3"), ("kill_refills_meter", "0.25"),
        ("mult_player_movespeed_shieldrequired", "1.1"),
    ]),
    (327, "The Claidheamohmor", "tf_weapon_sword", "melee", "demoman", &[
        ("is_a_sword", "72"), ("kill_refills_meter", "0.25"), ("mod_charge_time", "0.5"), ("provide_on_active", "1"),
    ]),
    (172, "The Scotsman's Skullcutter", "tf_weapon_sword", "melee", "demoman", &[
        ("is_a_sword", "72"), ("mult_player_movespeed", "0.85"), ("provide_on_active", "1"),
    ]),
    (132, "The Eyelander", "tf_weapon_sword", "melee", "demoman", &[("is_a_sword", "72"), ("decapitate_type", "1"), ("add_maxhealth", "-25")]),
    (129, "The Buff Banner", "tf_weapon_buff_item", "secondary", "soldier", &[("set_buff_type", "1")]),
    (226, "The Battalion's Backup", "tf_weapon_buff_item", "secondary", "soldier", &[("set_buff_type", "2"), ("add_maxhealth", "20")]),
    (354, "The Concheror", "tf_weapon_buff_item", "secondary", "soldier", &[("set_buff_type", "3"), ("add_health_regen", "4")]),
];

fn schema() -> String {
    let mut s = String::from("\"items_game\"\n{\n\"items\"\n{\n");
    for (index, name, class, slot, used, attrs) in ITEMS {
        s += &format!("\"{index}\" {{ \"name\" \"{name}\" \"item_class\" \"{class}\" \"item_slot\" \"{slot}\" \"used_by_classes\" {{ \"{used}\" \"1\" }} \"attributes\" {{\n");
        for (a, v) in *attrs {
            s += &format!("\"{a}\" {{ \"value\" \"{v}\" }}\n");
        }
        s += "} }\n";
    }
    s += "}\n\"attributes\"\n{\n";
    for (i, (class, format)) in DEFS.iter().enumerate() {
        s += &format!("\"{i}\" {{ \"name\" \"{class}\" \"attribute_class\" \"{class}\" \"description_format\" \"{format}\" }}\n");
    }
    s + "}\n}\n"
}

fn pack() -> ItemPack {
    let p = convert::convert(&schema(), &|_| None).pack;
    ItemPack::parse(&p.to_text()).unwrap()
}

#[derive(Clone)]
struct WithItems;

impl Plugin for WithItems {
    fn build(&self, app: &mut App) {
        app.insert_resource(GunNumbers::builtin().with_pack(&pack()));
        Plugin::build(&Tf2Sim, app);
    }
}

fn new_sim() -> Sim {
    let mut sim = Sim::new(WithItems, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

fn points(sim: &Sim, e: Entity) -> (f32, f32) {
    let h = sim.app.world().get::<Health>(e).unwrap();
    ((h.current * 100.0 * 100.0).round() / 100.0, (h.max * 100.0).round())
}

fn set_health(sim: &mut Sim, e: Entity, p: f32) {
    sim.app.world_mut().get_mut::<Health>(e).unwrap().current = p / 100.0;
}

fn speed(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<SourceTraits>(p).unwrap().max_speed.unwrap()
}

fn has(sim: &Sim, e: Entity, c: Condition) -> bool {
    sim.app.world().resource::<Conditions>().has(e, c, now(sim))
}

fn carry(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    let w = give(sim.app.world_mut(), p, id).unwrap();
    sim.ticks(2);
    w
}

fn shield(sim: &Sim, w: Entity) -> tf2::game::items::shields::ChargeState {
    sim.app.world().get::<Tf2Shield>(w).unwrap().state
}

fn set_meter(sim: &mut Sim, w: Entity, m: f32) {
    sim.app.world_mut().get_mut::<Tf2Shield>(w).unwrap().state.meter = m;
}

fn rage(sim: &Sim, w: Entity) -> f32 {
    sim.app.world().get::<Tf2Meter>(w).unwrap().state.value
}

fn deal(sim: &mut Sim, attacker: Option<Entity>, target: Entity, amount: f32, kind: DamageKind) {
    sim.app.world_mut().resource_mut::<Messages<Damage>>().write(Damage {
        target,
        attacker,
        amount: amount / 100.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Generic,
        kind,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(1);
}

/// A Demoman holding `melee` with `shield` carried, settled.
fn demo(sim: &mut Sim, melee: &str, shield: &str) -> (Entity, Entity, Entity) {
    let (p, m) = holding(sim, greybox::SPAWNS[0], melee);
    let s = carry(sim, p, shield);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(m), "the shield is never drawn");
    hold_still(sim, p);
    (p, m, s)
}

/// Charge for `n` ticks, forward held (attack2 on the first).
fn charge(sim: &mut Sim, p: Entity, n: u64) {
    run(sim, p, n, |s, k| {
        s.intent(p).secondary = k == 0;
        s.intent(p).move_axis = Vec2::new(0.0, 1.0);
    });
}

// ---- 1. Shield charge ----

#[test]
fn a_full_charge_runs_1_5_s_at_750_and_covers_about_1104_units() {
    let mut sim = new_sim();
    let (p, _, w) = demo(&mut sim, BOTTLE, "tf2:chargin_targe");
    let start = sim.position(p);
    charge(&mut sim, p, 50);
    assert!(shield(&sim, w).charging);
    assert_eq!(speed(&sim, p), 750.0);
    charge(&mut sim, p, 49);
    assert!(shield(&sim, w).charging, "99 ticks in");
    charge(&mut sim, p, 1);
    assert!(!shield(&sim, w).charging, "ended at 1.5 s");
    assert_eq!(shield(&sim, w).meter, 0.0);
    let d = (sim.position(p) - start).with_y(0.0).length() / U;
    assert!((d - 1104.0).abs() < 40.0, "{d}");
    sim.ticks(2);
    let v = sim.app.world().get::<Velocity>(p).unwrap().0.with_y(0.0).length() / U;
    assert!(v <= 280.5, "back to walking at once: {v}");
}

#[test]
fn the_crit_boost_comes_under_75_and_lingers_0_3_s() {
    let mut sim = new_sim();
    let (p, m, w) = demo(&mut sim, BOTTLE, "tf2:chargin_targe");
    charge(&mut sim, p, 25);
    assert_eq!(sim.app.world().get::<Tf2Melee>(m).unwrap().boost, 0, "meter 75");
    charge(&mut sim, p, 1);
    assert!(shield(&sim, w).meter < 75.0);
    assert_eq!(sim.app.world().get::<Tf2Melee>(m).unwrap().boost, 2, "a crit");
    charge(&mut sim, p, 74);
    assert!(!shield(&sim, w).charging);
    run(&mut sim, p, 15, |_, _| {});
    assert_eq!(sim.app.world().get::<Tf2Melee>(m).unwrap().boost, 2, "0.22 s after");
    run(&mut sim, p, 6, |_, _| {});
    assert_eq!(sim.app.world().get::<Tf2Melee>(m).unwrap().boost, 0, "gone 0.3 s after");
}

#[test]
fn the_meter_refills_in_12_s_and_the_splendid_screen_in_8() {
    for (shield_id, secs) in [("tf2:chargin_targe", 12.0), ("tf2:splendid_screen", 8.0)] {
        let mut sim = new_sim();
        let (p, _, w) = demo(&mut sim, BOTTLE, shield_id);
        charge(&mut sim, p, 100);
        assert_eq!(shield(&sim, w).meter, 0.0);
        sim.seconds(secs - 0.1);
        assert!(shield(&sim, w).meter < 100.0, "{shield_id}");
        sim.seconds(0.15);
        assert!(shield(&sim, w).meter >= 100.0 - 1e-3, "{shield_id}: {}", shield(&sim, w).meter);
    }
}

#[test]
fn the_claidheamh_mor_charges_2_s_and_the_skullcutter_at_637_5() {
    let mut sim = new_sim();
    let (p, _, w) = demo(&mut sim, "tf2:claidheamohmor", "tf2:chargin_targe");
    charge(&mut sim, p, 67);
    let m = shield(&sim, w).meter;
    assert!((m - 50.0).abs() < 1.0, "2.0 s: half gone in 1 s, {m}");

    let mut sim = new_sim();
    let (p, _, _) = demo(&mut sim, "tf2:scotsmans_skullcutter", "tf2:chargin_targe");
    charge(&mut sim, p, 10);
    assert!((speed(&sim, p) - 637.5).abs() < 0.01, "{}", speed(&sim, p));
}

#[test]
fn eyelander_heads_leave_the_charge_at_750() {
    let mut sim = new_sim();
    let (p, _, _) = demo(&mut sim, "tf2:eyelander", "tf2:chargin_targe");
    sim.app.world_mut().entity_mut(p).insert(Heads(4));
    sim.ticks(2);
    assert!(speed(&sim, p) > 360.0, "the heads' walking speed: {}", speed(&sim, p));
    charge(&mut sim, p, 10);
    assert_eq!(speed(&sim, p), 750.0);
}

/// The heading turned in 1 s of charging with the view turning at 3000
/// degrees/s.
fn turned(shield_id: &str, boots: bool) -> f32 {
    let mut sim = new_sim();
    let (p, _, w) = demo(&mut sim, BOTTLE, shield_id);
    if boots {
        carry(&mut sim, p, "tf2:ali_babas_wee_booties");
    }
    let yaw0 = sim.intent(p).yaw;
    let mut yaw = yaw0;
    let step = 3000f32.to_radians() * TICK_INTERVAL as f32;
    run(&mut sim, p, 67, |s, k| {
        s.intent(p).secondary = k == 0;
        s.intent(p).move_axis = Vec2::new(0.0, 1.0);
        if k > 0 {
            yaw = (yaw - step).min(yaw0 + 20.0);
            yaw = yaw.max(yaw0 - 170f32.to_radians());
        }
        s.intent(p).yaw = yaw;
    });
    assert!(shield(&sim, w).charging);
    (shield(&sim, w).heading - yaw0).abs().to_degrees()
}

#[test]
fn turning_is_capped_at_48_three_times_that_with_the_boots_and_free_with_the_tide_turner() {
    let targe = turned("tf2:chargin_targe", false);
    assert!((targe - 48.0).abs() < 2.0, "{targe}");
    let boots = turned("tf2:chargin_targe", true);
    assert!((boots - 144.0).abs() < 4.0, "{boots}");
    let tide = turned("tf2:tide_turner", false);
    assert!(tide > 169.0, "uncapped: {tide}");
}

#[test]
fn the_tide_turner_loses_meter_to_damage_and_a_melee_kill_refills_75() {
    let mut sim = new_sim();
    let (p, _, w) = demo(&mut sim, BOTTLE, "tf2:tide_turner");
    charge(&mut sim, p, 20);
    let before = shield(&sim, w).meter;
    assert!((before - 80.0).abs() < 0.5);
    deal(&mut sim, None, p, 55.0, DamageKind::Bullet);
    let after = shield(&sim, w).meter;
    assert!((before - 1.0 - 55.0 - after).abs() < 1.0, "79 - 55 - a tick's drain: {before} -> {after}");

    let mut sim = new_sim();
    let (p, _, w) = demo(&mut sim, BOTTLE, "tf2:tide_turner");
    let (t, _) = holding(&mut sim, greybox::SPAWNS[1], SHOVEL);
    set_health(&mut sim, t, 10.0);
    set_meter(&mut sim, w, 15.0);
    deal(&mut sim, Some(p), t, 50.0, DamageKind::Melee);
    assert!(points(&sim, t).0 <= 0.0);
    let m = shield(&sim, w).meter;
    assert!((m - 90.0).abs() < 0.5, "{m}");
}

#[test]
fn the_targe_cuts_enemy_blast_and_fire_but_not_his_own_blast() {
    let mut sim = new_sim();
    let (p, _, _) = demo(&mut sim, BOTTLE, "tf2:chargin_targe");
    let (t, _) = holding(&mut sim, greybox::SPAWNS[1], SHOVEL);
    set_health(&mut sim, p, 175.0);
    deal(&mut sim, Some(t), p, 100.0, DamageKind::Blast);
    assert_eq!(points(&sim, p).0, 105.0, "enemy blast × 0.7");
    set_health(&mut sim, p, 175.0);
    deal(&mut sim, Some(p), p, 100.0, DamageKind::Blast);
    assert_eq!(points(&sim, p).0, 75.0, "his own: whole");
}

#[test]
fn the_boots_add_25_health_and_a_tenth_of_walking_speed_with_a_shield() {
    let mut sim = new_sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], BOTTLE);
    let base = speed(&sim, p);
    carry(&mut sim, p, "tf2:ali_babas_wee_booties");
    assert_eq!(speed(&sim, p), base, "no shield: no speed");
    carry(&mut sim, p, "tf2:chargin_targe");
    sim.ticks(2);
    assert!((speed(&sim, p) - 308.0).abs() < 0.01, "{}", speed(&sim, p));
    assert_eq!(points(&sim, p).1, 200.0);
}

#[test]
fn a_charge_into_an_enemy_hits_him_and_ends() {
    let mut sim = new_sim();
    let (p, _, w) = demo(&mut sim, BOTTLE, "tf2:splendid_screen");
    let ahead = sim.position(p) - Vec3::Z * 400.0 * U;
    let t = sim.spawn_character(ahead, tf2::game::movement::ID);
    sim.seconds(1.0);
    hold_still(&mut sim, p);
    set_health(&mut sim, t, 300.0);
    charge(&mut sim, p, 80);
    assert!(!shield(&sim, w).charging, "the impact ended it");
    let taken = 300.0 - points(&sim, t).0;
    assert_eq!(taken, (tf2::game::items::shields::IMPACT_DAMAGE * 1.7).round(), "the Screen's × 1.7");
    assert_eq!(shield(&sim, w).boost, tf2::game::items::shields::Boost::Crit, "its crit after an impact");
}

// ---- 2. Banners ----

/// A Soldier with a shovel out and `banner` carried.
fn soldier(sim: &mut Sim, at: Vec3, banner: &str) -> (Entity, Entity) {
    let (p, _) = holding(sim, at, SHOVEL);
    let b = carry(sim, p, banner);
    (p, b)
}

#[test]
fn rage_fills_a_sixth_a_point_the_concheror_480_and_not_from_damage_taken() {
    for (id, gain) in [("tf2:buff_banner", 103.0 / 6.0), ("tf2:concheror", 103.0 / 4.8)] {
        let mut sim = new_sim();
        let (p, b) = soldier(&mut sim, greybox::SPAWNS[0], id);
        let (t, _) = holding(&mut sim, greybox::SPAWNS[1], SHOVEL);
        set_health(&mut sim, t, 1000.0);
        deal(&mut sim, Some(p), t, 103.0, DamageKind::Blast);
        assert!((rage(&sim, b) - gain).abs() < 0.01, "{id}: {}", rage(&sim, b));
    }
    let mut sim = new_sim();
    let (p, b) = soldier(&mut sim, greybox::SPAWNS[0], "tf2:battalions_backup");
    let (t, _) = holding(&mut sim, greybox::SPAWNS[1], SHOVEL);
    deal(&mut sim, Some(t), p, 72.0, DamageKind::Blast);
    assert_eq!(rage(&sim, b), 0.0);
}

/// A Soldier with full rage and `banner` out, a teammate at `mate` units
/// and another at 455; the horn blown.
fn blown(banner: &str, mate: f32) -> (Sim, Entity, Entity, Entity, Entity) {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (p, b) = soldier(&mut sim, from, banner);
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(b);
    sim.seconds(1.0);
    let (near, _) = holding(&mut sim, from + Vec3::X * mate * U, SHOVEL);
    let (far, _) = holding(&mut sim, from - Vec3::X * 455.0 * U, SHOVEL);
    for e in [p, near, far] {
        sim.app.world_mut().entity_mut(e).insert(Team(2));
        hold_still(&mut sim, e);
    }
    sim.app.world_mut().get_mut::<Tf2Meter>(b).unwrap().state.value = 100.0;
    run(&mut sim, p, 2, |s, k| s.intent(p).fire = k == 0);
    (sim, p, b, near, far)
}

#[test]
fn the_horn_buffs_the_soldier_and_teammates_within_450_from_2_7_s_for_10() {
    let (mut sim, p, b, near, far) = blown("tf2:buff_banner", 445.0);
    let hold = |sim: &mut Sim, n: f64| {
        let ticks = (n / TICK_INTERVAL).round() as u64;
        for _ in 0..ticks {
            for e in [p, near, far] {
                hold_still(sim, e);
            }
            sim.ticks(1);
        }
    };
    hold(&mut sim, 2.6);
    assert!(!has(&sim, p, Condition::MiniCritBuff), "the horn first");
    assert!((rage(&sim, b) - 100.0).abs() < 1e-3);
    hold(&mut sim, 0.2);
    assert!(has(&sim, p, Condition::MiniCritBuff));
    assert!(has(&sim, near, Condition::MiniCritBuff), "445: in");
    assert!(!has(&sim, far, Condition::MiniCritBuff), "455: out");
    hold(&mut sim, 9.95);
    assert!(rage(&sim, b) < 0.5, "drained in 10 s: {}", rage(&sim, b));
    assert!(has(&sim, p, Condition::MiniCritBuff), "lingers");
    hold(&mut sim, 1.4);
    assert!(!has(&sim, p, Condition::MiniCritBuff), "gone about 1.2 s after");
}

#[test]
fn the_buff_banners_buff_mini_crits() {
    let (mut sim, p, ..) = blown("tf2:buff_banner", 100.0);
    sim.seconds(3.0);
    let (t, _) = holding(&mut sim, greybox::SPAWNS[1], SHOVEL);
    set_health(&mut sim, t, 1000.0);
    deal(&mut sim, Some(p), t, 100.0, DamageKind::Bullet);
    assert_eq!(points(&sim, t).0, 865.0, "× 1.35");
}

#[test]
fn the_battalions_backup_cuts_damage_to_0_65_and_crits_to_normal() {
    let (mut sim, p, _, near, _) = blown("tf2:battalions_backup", 100.0);
    sim.seconds(3.0);
    assert!(has(&sim, near, Condition::DefenseBuff));
    assert_eq!(points(&sim, p).1, 220.0, "max health + 20");
    let (t, m) = holding(&mut sim, greybox::SPAWNS[1], SHOVEL);
    set_health(&mut sim, p, 200.0);
    deal(&mut sim, Some(t), p, 100.0, DamageKind::Bullet);
    assert_eq!(points(&sim, p).0, 135.0);
    set_health(&mut sim, p, 200.0);
    deal(&mut sim, Some(p), p, 100.0, DamageKind::Blast);
    assert_eq!(points(&sim, p).0, 135.0, "his own blast too");
    // A crit swing on him: a normal hit, cut (the Shovel's 65: 42).
    sim.app.world_mut().get_mut::<Tf2Melee>(m).unwrap().force_crit = Some(true);
    set_health(&mut sim, near, 200.0);
    let at = sim.position(near);
    sim.app.world_mut().get_mut::<Transform>(t).unwrap().translation = at + Vec3::Z * 50.0 * U;
    hold_still(&mut sim, t);
    chimera_sim::testkit::aim_at(&mut sim, t, at);
    for _ in 0..30 {
        hold_still(&mut sim, near);
        hold_still(&mut sim, t);
        sim.intent(t).fire = true;
        sim.ticks(1);
        sim.intent(t).fire = false;
        if points(&sim, near).0 < 200.0 {
            break;
        }
    }
    assert_eq!(points(&sim, near).0, 158.0, "65 × 0.65, not 195");
}

#[test]
fn the_concheror_speeds_to_336_and_heals_35_per_cent_of_a_hit() {
    let (mut sim, p, ..) = blown("tf2:concheror", 100.0);
    sim.seconds(3.0);
    assert!((speed(&sim, p) - 336.0).abs() < 0.01, "{}", speed(&sim, p));
    let (t, _) = holding(&mut sim, greybox::SPAWNS[1], SHOVEL);
    set_health(&mut sim, t, 1000.0);
    set_health(&mut sim, p, 100.0);
    deal(&mut sim, Some(p), t, 103.0, DamageKind::Blast);
    let h = points(&sim, p).0;
    assert!((136.0..138.0).contains(&h), "+ 36 (and a regeneration step at most): {h}");
}

#[test]
fn shields_banners_and_boots_never_join_a_ladder() {
    let set = GunNumbers::builtin().with_pack(&pack()).parse().unwrap();
    for id in ["tf2:chargin_targe", "tf2:tide_turner", "tf2:ali_babas_wee_booties", "tf2:buff_banner", "tf2:concheror"] {
        assert!(set.get(id).unwrap().class.is_none(), "{id}");
    }
}
