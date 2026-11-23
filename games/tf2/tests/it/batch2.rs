//! TF2's batch-2 unlocks (specs/tf2/batch2_mechanics.md, its test cases):
//! a made-up item schema in items_game's shape (the spec's attribute
//! values, nothing copied from the game) through the converter, the
//! items registered with the stock weapons, then played on the headless
//! harness: health on hit and kill (Black Box, Blutsauger and the Medic's
//! regeneration, Powerjack, Kunai, Candy Cane, Eyelander heads),
//! crit and mini-crit conditions (airborne after a blast, the Market
//! Gardener, the Back Scatter, burning victims, the Axtinguisher), bleed
//! (Boston Basher, Shiv), the flares (Flare Gun, Detonator, Scorch Shot)
//! and the speed buffs (the boost, Escape Plan, Equalizer, Disciplinary
//! Action, Eviction Notice, GRU, Atomizer); a crit's hit sound, Pyros
//! and flares' burns, and a network client predicting its own speed
//! boost and heads with no correction after their start (`NetSim`).

use bevy::prelude::*;
use chimera_core::core::{DamageKind, DamageTaken, Health, Team, Velocity};
use chimera_sim::{
    status::SpeedBoost,
    testkit,
    weapon::{Inventory, Weapon, give, projectile::{Flyer, LaunchRequest}},
};
use chimera_world::{greybox::{self, GreyboxMapPlugin}, harness::Sim};
use source_engine::source_movement::SourceTraits;
use tf2::{
    game::{
        TICK_INTERVAL,
        items::{ItemPack, convert, effects},
        weapons::{ROCKET_LAUNCHER, blast::{BlastJumping, BlastLaunched}, flame::Burning, script_numbers::GunNumbers},
    },
    harness::Tf2Sim,
};

use super::weapons::{health_points, hold_still, holding, no_spread, rockets as flyers, run};

const U: f32 = 0.0254;

/// (name, class, format) of each attribute the made-up schema defines.
const DEFS: &[(&str, &str, &str)] = &[
    ("damage penalty", "mult_dmg", "value_is_percentage"),
    ("dmg penalty vs players", "mult_dmg_vs_players", "value_is_percentage"),
    ("clip size penalty", "mult_clipsize", "value_is_inverted_percentage"),
    ("fire rate penalty", "mult_postfiredelay", "value_is_inverted_percentage"),
    ("spread penalty", "mult_spread_scale", "value_is_percentage"),
    ("maxammo secondary reduced", "mult_maxammo_secondary", "value_is_percentage"),
    ("health on radius damage", "add_health_on_radius_damage", "value_is_additive"),
    ("heal on hit for rapidfire", "add_onhit_addhealth", "value_is_additive"),
    ("health drain medic", "add_health_regen", "value_is_additive"),
    ("heal on kill", "heal_on_kill", "value_is_additive"),
    ("provide on active", "provide_on_active", "value_is_additive"),
    ("move speed bonus", "mult_player_movespeed", "value_is_percentage"),
    ("mult_player_movespeed_active", "mult_player_movespeed_active", "value_is_percentage"),
    ("dmg taken increased", "mult_dmgtaken", "value_is_percentage"),
    ("blast dmg taken", "mult_dmgtaken_from_explosions", "value_is_percentage"),
    ("max health additive penalty", "add_maxhealth", "value_is_additive"),
    ("sanguisuge", "sanguisuge", "value_is_additive"),
    ("drop health pack on kill", "drop_health_pack_on_kill", "value_is_additive"),
    ("decapitate type", "decapitate_type", "value_is_additive"),
    ("is_a_sword", "is_a_sword", "value_is_additive"),
    ("mod mini-crit airborne", "mini_crit_airborne", "value_is_additive"),
    ("mod crit while airborne", "crit_while_airborne", "value_is_additive"),
    ("closerange backattack minicrits", "closerange_backattack_minicrits", "value_is_additive"),
    ("minicrit vs burning player", "or_minicrit_vs_playercond_burning", "value_is_or"),
    ("attack_minicrits_and_consumes_burning", "attack_minicrits_and_consumes_burning", "value_is_additive"),
    ("bleeding duration", "bleeding_duration", "value_is_additive"),
    ("hit self on miss", "hit_self_on_miss", "value_is_additive"),
    ("speed_boost_on_hit", "speed_boost_on_hit", "value_is_additive"),
    ("speed buff ally", "speed_buff_ally", "value_is_additive"),
    ("set_weapon_mode", "set_weapon_mode", "value_is_additive"),
    ("self mark for death", "self_mark_for_death", "value_is_additive"),
    ("air dash count", "air_dash_count", "value_is_additive"),
    ("melee range multiplier", "melee_range_multiplier", "value_is_percentage"),
    ("melee bounds multiplier", "melee_bounds_multiplier", "value_is_percentage"),
    ("mod_maxhealth_drain_rate", "mod_maxhealth_drain_rate", "value_is_additive"),
    ("blast dmg to self increased", "blast_dmg_to_self", "value_is_percentage"),
    ("self dmg push force decreased", "mult_dmgself_push_force", "value_is_percentage"),
];

/// The made-up items: (index, name, item class, slot, class, attributes).
#[allow(clippy::type_complexity)]
const ITEMS: &[(u32, &str, &str, &str, &str, &[(&str, &str)])] = &[
    (36, "The Blutsauger", "tf_weapon_syringegun_medic", "primary", "medic", &[("heal on hit for rapidfire", "3"), ("health drain medic", "-2")]),
    (38, "The Axtinguisher", "tf_weapon_fireaxe", "melee", "pyro", &[("attack_minicrits_and_consumes_burning", "1"), ("damage penalty", "0.67")]),
    (39, "The Flare Gun", "tf_weapon_flaregun", "secondary", "pyro", &[("maxammo secondary reduced", "0.5")]),
    (128, "The Equalizer", "tf_weapon_shovel", "melee", "soldier", &[("provide on active", "1"), ("set_weapon_mode", "1")]),
    (132, "The Eyelander", "tf_weapon_sword", "melee", "demoman", &[("max health additive penalty", "-25"), ("decapitate type", "1"), ("is_a_sword", "72")]),
    (171, "The Tribalman's Shiv", "tf_weapon_club", "melee", "sniper", &[("bleeding duration", "6"), ("damage penalty", "0.5")]),
    (214, "The Powerjack", "tf_weapon_fireaxe", "melee", "pyro", &[
        ("heal on kill", "25"), ("provide on active", "1"), ("move speed bonus", "1.15"), ("dmg taken increased", "1.2"),
    ]),
    (228, "The Black Box", "tf_weapon_rocketlauncher", "primary", "soldier", &[("health on radius damage", "20"), ("clip size penalty", "0.75")]),
    (239, "Gloves of Running Urgently", "tf_weapon_fists", "melee", "heavy", &[("mult_player_movespeed_active", "1.3"), ("mod_maxhealth_drain_rate", "10")]),
    (317, "The Candy Cane", "tf_weapon_bat", "melee", "scout", &[("drop health pack on kill", "1"), ("blast dmg taken", "1.25")]),
    (325, "The Boston Basher", "tf_weapon_bat", "melee", "scout", &[("bleeding duration", "5"), ("hit self on miss", "1")]),
    (351, "The Detonator", "tf_weapon_flaregun", "secondary", "pyro", &[
        ("damage penalty", "0.75"), ("minicrit vs burning player", "1"), ("set_weapon_mode", "1"), ("blast dmg to self increased", "1.5"),
    ]),
    (356, "Conniver's Kunai", "tf_weapon_knife", "melee", "spy", &[("max health additive penalty", "-55"), ("sanguisuge", "1")]),
    (415, "The Reserve Shooter", "tf_weapon_shotgun", "secondary", "soldier", &[("mod mini-crit airborne", "1"), ("clip size penalty", "0.66")]),
    (416, "The Market Gardener", "tf_weapon_shovel", "melee", "soldier", &[("mod crit while airborne", "1"), ("fire rate penalty", "1.2")]),
    (426, "The Eviction Notice", "tf_weapon_fists", "melee", "heavy", &[
        ("damage penalty", "0.4"), ("mult_player_movespeed_active", "1.15"), ("mod_maxhealth_drain_rate", "5"), ("speed_boost_on_hit", "3"),
    ]),
    (447, "The Disciplinary Action", "tf_weapon_shovel", "melee", "soldier", &[
        ("damage penalty", "0.75"), ("speed buff ally", "1"), ("melee range multiplier", "1.7"), ("melee bounds multiplier", "1.55"),
    ]),
    (450, "The Atomizer", "tf_weapon_bat", "melee", "scout", &[("air dash count", "1"), ("dmg penalty vs players", "0.85")]),
    (740, "The Scorch Shot", "tf_weapon_flaregun", "secondary", "pyro", &[
        ("damage penalty", "0.65"), ("minicrit vs burning player", "1"), ("set_weapon_mode", "3"), ("self dmg push force decreased", "0.65"),
    ]),
    (775, "The Escape Plan", "tf_weapon_shovel", "melee", "soldier", &[("provide on active", "1"), ("set_weapon_mode", "2"), ("self mark for death", "1")]),
    (1103, "The Back Scatter", "tf_weapon_scattergun", "primary", "scout", &[
        ("closerange backattack minicrits", "1"), ("clip size penalty", "0.66"), ("spread penalty", "1.2"),
    ]),
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
    for (i, (name, class, format)) in DEFS.iter().enumerate() {
        s += &format!("\"{i}\" {{ \"name\" \"{name}\" \"attribute_class\" \"{class}\" \"description_format\" \"{format}\" }}\n");
    }
    s + "}\n}\n"
}

/// The pack, with the flare gun class's script numbers the converter reads
/// from the class script (here the spec's: 30, every 2 s).
fn pack() -> ItemPack {
    let mut p = convert::convert(&schema(), &|_| None).pack;
    for r in &mut p.items {
        if r.class == "tf_weapon_flaregun" {
            r.base.insert("damage".into(), 30.0);
            r.base.insert("interval".into(), 2.0);
        }
    }
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

fn sim() -> Sim {
    let mut sim = Sim::new(WithItems, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

fn points(sim: &Sim, e: Entity) -> (f32, f32) {
    let h = sim.app.world().get::<Health>(e).unwrap();
    (h.current * 100.0, h.max * 100.0)
}

fn set_health(sim: &mut Sim, e: Entity, points: f32) {
    sim.app.world_mut().get_mut::<Health>(e).unwrap().current = points / 100.0;
}

fn speed(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<SourceTraits>(p).unwrap().max_speed.unwrap()
}

/// A target `units` ahead (-Z) of `from` holding `gun` (so of its class),
/// with `hp` points.
fn victim(sim: &mut Sim, from: Vec3, units: f32, gun: &str, hp: f32) -> Entity {
    let (t, _) = holding(sim, from - Vec3::Z * units * U, gun);
    set_health(sim, t, hp);
    hold_still(sim, t);
    t
}

/// One swing of `p` at `t` (aimed at its centre): the damage it took.
fn swing_at(sim: &mut Sim, p: Entity, t: Entity) -> f32 {
    let at = sim.position(t);
    testkit::aim_at(sim, p, at);
    let before = health_points(sim, t);
    run(sim, p, 20, |s, k| s.intent(p).fire = k == 0);
    (before - health_points(sim, t)).round()
}

/// Every damage `e` took of `kind` while `f` runs: (time, points).
fn taken_while(sim: &mut Sim, e: Entity, kind: DamageKind, ticks: u64, mut f: impl FnMut(&mut Sim, u64)) -> Vec<(f64, f32)> {
    let mut cursor = sim.app.world().resource::<Messages<DamageTaken>>().get_cursor_current();
    let mut out = Vec::new();
    for k in 0..ticks {
        f(sim, k);
        sim.ticks(1);
        let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
        for t in cursor.read(sim.app.world().resource::<Messages<DamageTaken>>()) {
            if t.damage.target == e && t.damage.kind == kind {
                out.push((now, (t.damage.amount * 100.0).round()));
            }
        }
    }
    out
}

fn burn(sim: &mut Sim, t: Entity, taken: u32) {
    let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    sim.app.world_mut().entity_mut(t).insert(Burning {
        left: 7.5 - 0.5 * taken as f32,
        next: now + 0.5,
        attacker: None,
        weapon: "tf2:flare_gun",
        damage: 0.0,
        interval: 0.5,
        harmless: false,
        from_tick: true,
    });
}

// ---- 1. Health on hit and kill ----

#[test]
fn the_black_box_heals_once_per_rocket_by_the_damage_dealt() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:black_box");
    assert_eq!(sim.app.world().get::<chimera_sim::weapon::Magazine>(w).unwrap().clip, 3, "4 x 0.75");
    let t = victim(&mut sim, from, 300.0, "tf2:tf_weapon_minigun", 1000.0);
    set_health(&mut sim, p, 100.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    run(&mut sim, p, 40, |s, k| s.intent(p).fire = k == 0);
    let dealt = 1000.0 - health_points(&sim, t);
    let gained = (points(&sim, p).0 - 100.0).round();
    assert!(dealt > 30.0, "{dealt}");
    assert_eq!(gained, (20.0 * (dealt / 90.0).min(1.0) + 1e-4).floor(), "dealt {dealt}");
    // At 195 of 200: to 200 only.
    set_health(&mut sim, p, 195.0);
    sim.seconds(1.0);
    hold_still(&mut sim, t);
    run(&mut sim, p, 40, |s, k| s.intent(p).fire = k == 0);
    assert_eq!(points(&sim, p).0.round(), 200.0);
}

#[test]
fn the_blutsauger_heals_3_a_needle_and_regenerates_2_5_then_4() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:blutsauger");
    let t = victim(&mut sim, from, 100.0, "tf2:tf_weapon_minigun", 1000.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    set_health(&mut sim, p, 50.0);
    let before = points(&sim, p).0;
    run(&mut sim, p, 20, |s, k| s.intent(p).fire = k == 0);
    let gain = (points(&sim, p).0 - before).round();
    assert!((3.0..=6.0).contains(&gain), "a needle's 3 (and maybe a regeneration tick): {gain}");
    // Hurt now; 0-5 s: 2.5 a second; after 10 s: 4.
    hurt(&mut sim, p, 20.0);
    let h0 = points(&sim, p).0;
    sim.seconds(4.0);
    let early = points(&sim, p).0 - h0;
    assert!((9.0..=11.0).contains(&early), "4 s at 2.5: {early}");
    sim.seconds(8.0);
    let h1 = points(&sim, p).0;
    sim.seconds(3.0);
    assert!(((points(&sim, p).0 - h1) - 12.0).abs() <= 1.0, "3 s at 4: {}", points(&sim, p).0 - h1);
}

fn hurt(sim: &mut Sim, p: Entity, amount: f32) {
    sim.app.world_mut().resource_mut::<Messages<chimera_core::core::Damage>>().write(chimera_core::core::Damage {
        target: p,
        attacker: None,
        amount: amount / 100.0,
        point: Vec3::ZERO,
        dir: Vec3::Y,
        hitgroup: chimera_core::core::Hitgroup::Generic,
        kind: DamageKind::Generic,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(1);
}

#[test]
fn the_powerjacks_kill_heals_25_to_max_and_it_moves_at_345() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:powerjack");
    assert!((speed(&sim, p) - 345.0).abs() < 0.01, "{}", speed(&sim, p));
    let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_minigun", 5.0);
    set_health(&mut sim, p, 160.0);
    swing_at(&mut sim, p, t);
    assert_eq!(points(&sim, p).0.round(), 175.0);
}

#[test]
fn the_kunais_backstab_heals_the_victims_health_to_3x_and_drains() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:connivers_kunai");
    assert_eq!(points(&sim, p).1.round(), 70.0, "125 - 55");
    let stab = |sim: &mut Sim, mine: f32, theirs: f32| {
        let t = victim(sim, from, 40.0, "tf2:tf_weapon_minigun", theirs);
        set_health(sim, p, mine);
        let at = sim.position(t);
        testkit::aim_at(sim, p, at);
        let yaw = sim.intent(p).yaw;
        sim.intent(t).yaw = yaw;
        run(sim, p, 20, |s, k| {
            s.intent(t).yaw = yaw;
            s.intent(p).fire = k == 0
        });
        assert!(health_points(sim, t) <= 0.0, "a backstab kill");
        sim.app.world_mut().despawn(t);
        points(sim, p).0
    };
    assert!((stab(&mut sim, 20.0, 90.0) - 110.0).abs() <= 0.5);
    let after = stab(&mut sim, 123.0, 300.0);
    assert!((after - 210.0).abs() <= 0.5, "{after}");
    sim.seconds(4.3);
    assert!((points(&sim, p).0 - 200.0).abs() <= 0.6, "{}", points(&sim, p).0);
}

#[test]
fn the_candy_canes_kill_drops_a_kit_gone_33_s_later() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:candy_cane");
    let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_minigun", 5.0);
    swing_at(&mut sim, p, t);
    let kits = |sim: &mut Sim| {
        let mut q = sim.app.world_mut().query::<&Flyer>();
        q.iter(sim.app.world()).filter(|f| f.weapon == effects::KIT).count()
    };
    assert_eq!(kits(&mut sim), 1, "a small kit at the victim");
    // Hurt, the killer walks onto it: ceil(125 x 0.2) = 25.
    set_health(&mut sim, p, 50.0);
    let kit = {
        let mut q = sim.app.world_mut().query::<(&Flyer, &Transform)>();
        q.iter(sim.app.world()).find(|(f, _)| f.weapon == effects::KIT).unwrap().1.translation
    };
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = kit.with_y(sim.position(p).y);
    sim.ticks(3);
    assert_eq!(points(&sim, p).0.round(), 75.0);
    assert_eq!(kits(&mut sim), 0, "taken");
}

#[test]
fn the_candy_canes_untaken_kit_is_gone_after_its_life() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:candy_cane");
    let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_minigun", 5.0);
    swing_at(&mut sim, p, t);
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation += Vec3::X * 300.0 * U;
    let kits = |sim: &mut Sim| {
        let mut q = sim.app.world_mut().query::<&Flyer>();
        q.iter(sim.app.world()).filter(|f| f.weapon == effects::KIT).count()
    };
    sim.seconds(30.0);
    assert_eq!(kits(&mut sim), 1, "still there at 30 s");
    sim.seconds(3.0);
    assert_eq!(kits(&mut sim), 0, "gone by 33 s");
}

#[test]
fn eyelander_heads_add_health_and_speed_up_to_four() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:eyelander");
    assert_eq!(points(&sim, p).1.round(), 150.0);
    set_health(&mut sim, p, 50.0);
    let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_minigun", 5.0);
    swing_at(&mut sim, p, t);
    assert_eq!((points(&sim, p).0.round(), points(&sim, p).1.round()), (65.0, 165.0));
    sim.ticks(2);
    assert!((speed(&sim, p) - 302.4).abs() < 0.01, "{}", speed(&sim, p));
    // Four heads, then the fifth.
    sim.app.world_mut().entity_mut(p).insert(effects::Heads(4));
    sim.ticks(2);
    set_health(&mut sim, p, 100.0);
    sim.app.world_mut().despawn(t);
    let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_minigun", 5.0);
    swing_at(&mut sim, p, t);
    sim.ticks(2);
    assert_eq!(points(&sim, p).1.round(), 210.0);
    assert!((speed(&sim, p) - 369.6).abs() < 0.01, "{}", speed(&sim, p));
    assert_eq!(sim.app.world().get::<effects::Heads>(p), Some(&effects::Heads(5)));
}

// ---- 2. Crits and mini-crits ----

#[test]
fn a_blast_push_into_the_air_sets_the_launched_state() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, ROCKET_LAUNCHER);
    let t = victim(&mut sim, from, 250.0, "tf2:tf_weapon_minigun", 1000.0);
    let feet = sim.position(t) - Vec3::Y * 0.9 + Vec3::Z * 0.6;
    testkit::aim_at(&mut sim, p, feet);
    let mut seen = false;
    run(&mut sim, p, 40, |s, k| {
        s.intent(p).fire = k == 0;
        seen |= s.app.world().get::<BlastLaunched>(t).is_some() && !s.state(t).on_ground;
    });
    assert!(seen, "launched by another's blast");
}

/// The Reserve Shooter's shot at a target in the air: launched or not.
fn reserve_shot(launched: bool) -> f32 {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:reserve_shooter");
    no_spread(&mut sim, w);
    let t = victim(&mut sim, from, 200.0, "tf2:tf_weapon_minigun", 1000.0);
    sim.app.world_mut().get_mut::<Transform>(t).unwrap().translation += Vec3::Y * 80.0 * U;
    sim.app.world_mut().get_mut::<Velocity>(t).unwrap().0 = Vec3::Y * 300.0 * U;
    sim.ticks(2);
    if launched {
        sim.app.world_mut().entity_mut(t).insert(BlastLaunched);
    }
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    assert!(!sim.state(t).on_ground);
    let before = health_points(&sim, t);
    run(&mut sim, p, 2, |s, k| s.intent(p).fire = k == 0);
    before - health_points(&sim, t)
}

#[test]
fn the_reserve_shooter_mini_crits_a_launched_victim_only() {
    let plain = reserve_shot(false);
    let mini = reserve_shot(true);
    assert!(plain > 10.0, "{plain}");
    assert!((mini - (plain * 1.35).round()).abs() <= 1.0, "{plain} -> {mini}");
}

/// The Market Gardener's swing from the air at a floating target.
fn gardener(jumping: bool) -> f32 {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:market_gardener");
    let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_minigun", 1000.0);
    for e in [p, t] {
        sim.app.world_mut().get_mut::<Transform>(e).unwrap().translation += Vec3::Y * 200.0 * U;
        sim.app.world_mut().get_mut::<Velocity>(e).unwrap().0 = Vec3::ZERO;
    }
    if jumping {
        sim.app.world_mut().entity_mut(p).insert(BlastJumping);
    }
    sim.ticks(1);
    swing_at(&mut sim, p, t)
}

#[test]
fn the_market_gardener_crits_while_blast_jumping_195_65() {
    assert_eq!(gardener(true), 195.0);
    assert_eq!(gardener(false), 65.0);
}

/// The Back Scatter's shot from `units` away at `off` degrees off the
/// victim's back.
fn back_scatter(units: f32, off: f32) -> f32 {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:back_scatter");
    no_spread(&mut sim, w);
    let t = victim(&mut sim, from, units, "tf2:tf_weapon_minigun", 1000.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    // The victim faces away from the shooter, turned `off` degrees.
    let yaw = sim.intent(p).yaw + off.to_radians();
    let before = health_points(&sim, t);
    run(&mut sim, p, 2, |s, k| {
        s.intent(t).yaw = yaw;
        s.intent(p).fire = k == 0
    });
    before - health_points(&sim, t)
}

#[test]
fn the_back_scatter_mini_crits_from_behind_within_512() {
    let face = back_scatter(300.0, 180.0);
    assert!((back_scatter(300.0, 74.0) - (face * 1.35).round()).abs() <= 1.0, "74 degrees: mini-crit");
    assert_eq!(back_scatter(300.0, 76.0), face, "76 degrees: normal");
    let near = back_scatter(511.0, 180.0);
    assert!((back_scatter(511.0, 0.0) - (near * 1.35).round()).abs() <= 1.0, "511: mini-crit");
    assert_eq!(back_scatter(514.0, 0.0), back_scatter(514.0, 180.0), "514: normal");
}

#[test]
fn the_axtinguisher_grows_with_the_burn_left_and_puts_it_out() {
    let hit = |taken: Option<u32>| {
        let mut sim = sim();
        let from = greybox::SPAWNS[0];
        let (p, _) = holding(&mut sim, from, "tf2:axtinguisher");
        let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_minigun", 1000.0);
        if let Some(k) = taken {
            burn(&mut sim, t, k);
        }
        let d = swing_at(&mut sim, p, t);
        (d, sim.app.world().get::<Burning>(t).is_some())
    };
    assert_eq!(hit(None), (44.0, false));
    assert_eq!(hit(Some(2)), (137.0, false), "burn removed");
    assert_eq!(hit(Some(14)).0, 86.0);
    // A kill on a burning player: the speed boost for 3 s (Pyro 405).
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:axtinguisher");
    let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_minigun", 5.0);
    burn(&mut sim, t, 2);
    swing_at(&mut sim, p, t);
    sim.ticks(2);
    assert!((speed(&sim, p) - 405.0).abs() < 0.01, "{}", speed(&sim, p));
    sim.seconds(3.0);
    assert!((speed(&sim, p) - 300.0).abs() < 0.01);
}

// ---- 3. Bleed ----

#[test]
fn the_boston_basher_bleeds_10_ticks_of_4_and_a_second_hit_extends_it() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:boston_basher");
    let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_minigun", 1000.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let hit = taken_while(&mut sim, t, DamageKind::Melee, 20, |s, k| s.intent(p).fire = k == 0);
    assert_eq!(hit.len(), 1);
    assert_eq!(hit[0].1, 35.0);
    let ticks = taken_while(&mut sim, t, DamageKind::Bleed, 500, |_, _| {});
    assert_eq!(ticks.len() + 1, 10, "the first with the hit's next tick: {ticks:?}");
    assert!(ticks.iter().all(|t| t.1 == 4.0));
    for w in ticks.windows(2) {
        assert!((w[1].0 - w[0].0 - 0.51).abs() < 1e-6, "{w:?}");
    }

    // Hit, then again 1.29 s later: 13 ticks, the last 6.12 s after the first.
    let mut sim = self::sim();
    let (p, _) = holding(&mut sim, from, "tf2:boston_basher");
    let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_minigun", 1000.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let second = (1.29 / TICK_INTERVAL as f64).round() as u64;
    let ticks = taken_while(&mut sim, t, DamageKind::Bleed, 600, |s, k| {
        hold_still(s, t);
        s.intent(p).fire = k == 0 || k == second;
    });
    assert_eq!(ticks.len(), 13, "{ticks:?}");
    let span = ticks[12].0 - ticks[0].0;
    assert!((span - 6.12).abs() < 1e-6, "{span}");
}

#[test]
fn the_boston_basher_at_nothing_hits_its_scout_for_18_and_bleeds_him() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:boston_basher");
    let before = points(&sim, p).0;
    let ticks = taken_while(&mut sim, p, DamageKind::Bleed, 20, |s, k| s.intent(p).fire = k == 0);
    assert!(!ticks.is_empty(), "bleeding");
    assert_eq!((before - points(&sim, p).0).round(), 18.0 + 4.0 * ticks.len() as f32);
}

#[test]
fn the_shiv_hits_33_and_bleeds_12_ticks() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:tribalmans_shiv");
    let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_minigun", 1000.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let hit = taken_while(&mut sim, t, DamageKind::Melee, 20, |s, k| s.intent(p).fire = k == 0);
    assert_eq!(hit[0].1, 33.0);
    let ticks = taken_while(&mut sim, t, DamageKind::Bleed, 600, |_, _| {});
    assert_eq!(ticks.len() + 1, 12);
}

// ---- 4. Flares ----

/// `id`'s flare at a victim `units` ahead, `lit` burning: what it took.
fn flare_hit(id: &str, units: f32, lit: bool) -> (Sim, Entity, Entity, f32) {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, id);
    let t = victim(&mut sim, from, units, "tf2:tf_weapon_minigun", 1000.0);
    if lit {
        burn(&mut sim, t, 0);
    }
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let before = health_points(&sim, t);
    let ticks = (units as f64 / 2000.0 / TICK_INTERVAL) as u64 + 4;
    run(&mut sim, p, ticks, |s, k| s.intent(p).fire = k == 0);
    let d = (before - health_points(&sim, t)).round();
    (sim, p, t, d)
}

#[test]
fn the_flare_flies_at_2000_falling_at_240() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:flare_gun");
    let ahead = sim.position(p) + Vec3::new(0.0, 0.5, -20.0);
    testkit::aim_at(&mut sim, p, ahead);
    run(&mut sim, p, 2, |s, k| s.intent(p).fire = k == 0);
    let f = flyers(&mut sim, p);
    assert_eq!(f.len(), 1);
    let v0 = f[0].2 / U;
    assert!((v0.length() - 2000.0).abs() < 5.0, "{}", v0.length());
    sim.ticks(1);
    let v1 = flyers(&mut sim, p)[0].2 / U;
    assert!(((v0.y - v1.y) - 3.6).abs() < 0.05, "{}", v0.y - v1.y);
}

#[test]
fn the_flare_gun_hits_30_burns_15_ticks_of_4_and_crits_a_burning_player_90() {
    for units in [30.0, 600.0] {
        let (mut sim, _, t, d) = flare_hit("tf2:flare_gun", units, false);
        assert_eq!(d, 30.0, "{units}");
        let ticks = taken_while(&mut sim, t, DamageKind::Burn, 600, |s, _| hold_still(s, t));
        assert_eq!(ticks.len(), 15);
        assert!(ticks.iter().all(|t| t.1 == 4.0));
        for w in ticks.windows(2) {
            assert!((w[1].0 - w[0].0 - 0.51).abs() < 1e-6, "{w:?}");
        }
    }
    assert_eq!(flare_hit("tf2:flare_gun", 300.0, true).3, 90.0);
}

#[test]
fn the_detonator_hits_23_and_30_on_a_burning_player() {
    assert_eq!(flare_hit("tf2:detonator", 300.0, false).3, 23.0);
    assert_eq!(flare_hit("tf2:detonator", 300.0, true).3, 30.0);
}

/// The Detonator's flare held still `r` units from a victim's centre,
/// then attack2: what he took.
fn detonated(r: f32) -> f32 {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:detonator");
    let t = victim(&mut sim, from, 400.0, "tf2:tf_weapon_minigun", 1000.0);
    let id = sim.app.world().get::<Weapon>(w).unwrap().id;
    let centre = sim.position(t);
    let clock = sim.app.world().resource::<chimera_core::core::SimClock>().clone();
    let f = Flyer::new(id, Some(p), Vec3::ZERO, &clock);
    sim.app.world_mut().write_message(LaunchRequest { flyer: f, start: centre + Vec3::Z * r * U, shown: None, tag: None });
    sim.ticks(1);
    let before = health_points(&sim, t);
    run(&mut sim, p, 3, |s, k| s.intent(p).secondary = k == 0);
    (before - health_points(&sim, t)).round()
}

#[test]
fn the_detonators_explosion_falls_to_half_at_110() {
    assert_eq!(detonated(70.0), 15.0);
    assert_eq!(detonated(97.0), 13.0);
    assert_eq!(detonated(140.9), 0.0, "his box 114 away");
}

#[test]
fn the_scorch_shot_hits_20_pushes_and_bounces_to_explode_on_the_floor() {
    let (mut sim, p, t, d) = flare_hit("tf2:scorch_shot", 200.0, false);
    assert_eq!(d, 20.0);
    let v = sim.app.world().get::<Velocity>(t).unwrap().0 / U;
    // Lifted off the ground (at least 268.3 up at the hit, `scorch_push`;
    // read some ticks of gravity later).
    assert!(v.y >= 200.0 && !sim.state(t).on_ground, "a grounded victim lifted: {v:?}");
    // The flare bounced: 139 forward, 100 up.
    let f = flyers(&mut sim, p);
    assert_eq!(f.len(), 1, "it flies on");
    let fv = f[0].2 / U;
    assert!((fv.with_y(0.0).length() - 139.0).abs() < 1.0 && fv.y < 100.0 && fv.y > 50.0, "{fv:?}");
    // On the floor it explodes, the player it hit taking it whole, now
    // burning: 19.5 x 1.35 = 26.
    let blast = taken_while(&mut sim, t, DamageKind::Blast, 120, |_, _| {});
    assert_eq!(blast.iter().map(|b| b.1).collect::<Vec<_>>(), [26.0]);
    assert!(flyers(&mut sim, p).is_empty());
    assert_eq!(flare_hit("tf2:scorch_shot", 200.0, true).3, 26.0);
}

// ---- 5. Speed buffs ----

#[test]
fn the_speed_boost_is_x1_4_capped_at_105_505_336_322() {
    for (gun, want) in [("tf2:tf_weapon_scattergun", 505.0), (ROCKET_LAUNCHER, 336.0), ("tf2:tf_weapon_minigun", 322.0)] {
        let mut sim = sim();
        let (p, _) = holding(&mut sim, greybox::SPAWNS[0], gun);
        sim.app.world_mut().entity_mut(p).insert(SpeedBoost { until: 100.0, mult: effects::BOOST_MULT, cap: effects::BOOST_CAP });
        sim.ticks(2);
        assert!((speed(&sim, p) - want).abs() < 0.01, "{gun}: {}", speed(&sim, p));
    }
}

#[test]
fn the_escape_plan_speeds_up_as_health_falls_264_288_336_384() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:escape_plan");
    for (hp, want) in [(160.0, 240.0), (159.0, 264.0), (119.0, 288.0), (79.0, 336.0), (39.0, 384.0)] {
        set_health(&mut sim, p, hp);
        sim.ticks(2);
        assert!((speed(&sim, p) - want).abs() < 0.01, "{hp}: {}", speed(&sim, p));
    }
}

#[test]
fn the_equalizer_hits_70_at_100_and_107_at_1() {
    for (hp, want) in [(100.0, 70.0), (1.0, 107.0)] {
        let mut sim = sim();
        let from = greybox::SPAWNS[0];
        let (p, _) = holding(&mut sim, from, "tf2:equalizer");
        let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_minigun", 1000.0);
        set_health(&mut sim, p, hp);
        assert_eq!(swing_at(&mut sim, p, t), want, "{hp}");
    }
}

#[test]
fn the_disciplinary_action_boosts_a_teammate_2_s_and_itself_3_6_s() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:disciplinary_action");
    let t = victim(&mut sim, from, 75.0, "tf2:tf_weapon_rocketlauncher", 150.0);
    sim.app.world_mut().entity_mut(p).insert(Team(1));
    sim.app.world_mut().entity_mut(t).insert(Team(1));
    assert_eq!(swing_at(&mut sim, p, t), 0.0, "no damage");
    let until = |sim: &Sim, e: Entity| sim.app.world().get::<SpeedBoost>(e).map(|b| b.until);
    let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    let (mate, me) = (until(&sim, t).expect("teammate boosted"), until(&sim, p).expect("self boosted"));
    assert!((mate - me + 1.6).abs() < 1e-6, "2.0 vs 3.6 s");
    assert!(me > now && me - now < 3.6);
    assert!((speed(&sim, t) - 336.0).abs() < 0.01);
    // An enemy: 49, no boost.
    let mut sim = self::sim();
    let (p, _) = holding(&mut sim, from, "tf2:disciplinary_action");
    let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_minigun", 1000.0);
    assert_eq!(swing_at(&mut sim, p, t), 49.0);
    assert!(sim.app.world().get::<SpeedBoost>(p).is_none());
}

#[test]
fn the_eviction_notice_drains_and_boosts_on_hit_264_5_370_3() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:eviction_notice");
    assert!((speed(&sim, p) - 264.5).abs() < 0.01);
    let max = points(&sim, p).1;
    assert!((max - 275.0).abs() < 1.5, "300 - 20 - 5 a second for 1 s: {max}");
    let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_minigun", 1000.0);
    assert_eq!(swing_at(&mut sim, p, t), 26.0);
    sim.ticks(1);
    assert!((speed(&sim, p) - 370.3).abs() < 0.01, "{}", speed(&sim, p));
    sim.seconds(3.0);
    assert!((speed(&sim, p) - 264.5).abs() < 0.01);
}

#[test]
fn the_gru_moves_at_299_and_drains_10() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:gloves_of_running_urgently");
    assert!((speed(&sim, p) - 299.0).abs() < 0.01);
    let max = points(&sim, p).1;
    assert!((max - 270.0).abs() < 1.5, "{max}");
}

#[test]
fn the_atomizer_adds_an_air_jump_while_out() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:atomizer");
    assert_eq!(sim.app.world().get::<SourceTraits>(p).unwrap().air_jumps, Some(2));
    let other = give(sim.app.world_mut(), p, "tf2:tf_weapon_scattergun").unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(other);
    sim.seconds(1.5);
    assert_eq!(sim.app.world().get::<SourceTraits>(p).unwrap().air_jumps, Some(1));
}

#[test]
fn every_batch_two_weapon_that_deals_damage_is_a_ladder_gun() {
    let sim = sim();
    let c = sim.app.world().resource::<chimera_sim::weapon::data::WeaponCatalog>();
    for id in ["tf2:flare_gun", "tf2:detonator", "tf2:scorch_shot", "tf2:black_box", "tf2:back_scatter", "tf2:reserve_shooter", "tf2:blutsauger"] {
        assert!(c.get(id).unwrap_or_else(|| panic!("{id}")).class.is_some(), "{id}");
    }
}

// ---- Polish: the crit's sound, Pyros and flares, predicted boosts ----

/// `id`'s flare from a player at a `victim_gun` holder 300 units ahead
/// (`lit` burning): the points he took and every TF2 sound entry played
/// meanwhile.
fn flare_heard(id: &str, victim_gun: &str, lit: bool) -> (f32, Vec<String>) {
    use chimera_world::map::GameSound;
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, id);
    let t = victim(&mut sim, from, 300.0, victim_gun, 1000.0);
    if lit {
        burn(&mut sim, t, 0);
    }
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let before = health_points(&sim, t);
    let mut cursor = sim.app.world().resource::<Messages<GameSound>>().get_cursor_current();
    let mut heard = Vec::new();
    for k in 0..12 {
        sim.intent(p).fire = k == 0;
        sim.ticks(1);
        heard.extend(cursor.read(sim.app.world().resource::<Messages<GameSound>>()).map(|s| s.0.entry.clone()));
    }
    ((before - health_points(&sim, t)).round(), heard)
}

#[test]
fn a_crit_plays_the_crit_hit_sound_and_a_plain_hit_does_not() {
    let crit_hit = tf2::game::weapons::melee::CRIT_HIT;
    let (d, heard) = flare_heard("tf2:flare_gun", "tf2:tf_weapon_minigun", true);
    assert_eq!(d, 90.0);
    assert_eq!(heard.iter().filter(|e| *e == crit_hit).count(), 1, "{heard:?}");
    let (d, heard) = flare_heard("tf2:flare_gun", "tf2:tf_weapon_minigun", false);
    assert_eq!(d, 30.0);
    assert!(!heard.iter().any(|e| e == crit_hit), "{heard:?}");
    // A mini-crit has no sound of its own (none specified).
    let (d, heard) = flare_heard("tf2:detonator", "tf2:tf_weapon_minigun", true);
    assert_eq!(d, 30.0);
    assert!(!heard.iter().any(|e| e == crit_hit), "{heard:?}");
}

#[test]
fn an_enemy_pyro_hit_by_a_flare_burns_in_look_only() {
    let (mut sim, _, t, _) = {
        let mut sim = sim();
        let from = greybox::SPAWNS[0];
        let (p, _) = holding(&mut sim, from, "tf2:flare_gun");
        let t = victim(&mut sim, from, 300.0, "tf2:tf_weapon_flamethrower", 1000.0);
        let at = sim.position(t);
        testkit::aim_at(&mut sim, p, at);
        run(&mut sim, p, 12, |s, k| s.intent(p).fire = k == 0);
        (sim, p, t, ())
    };
    let b = *sim.app.world().get::<Burning>(t).expect("lit");
    assert!(b.harmless && b.left <= 0.0, "{b:?}");
    let ticks = taken_while(&mut sim, t, DamageKind::Burn, 120, |s, _| hold_still(s, t));
    assert!(ticks.is_empty(), "{ticks:?}");
    assert!(sim.app.world().get::<Burning>(t).is_none(), "out after 0.25 s");
}

#[test]
fn a_pyro_in_his_own_detonator_blast_takes_one_afterburn_tick() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:detonator");
    set_health(&mut sim, p, 1000.0);
    let id = sim.app.world().get::<Weapon>(w).unwrap().id;
    let clock = sim.app.world().resource::<chimera_core::core::SimClock>().clone();
    let f = Flyer::new(id, Some(p), Vec3::ZERO, &clock);
    let start = sim.position(p) + Vec3::Z * 50.0 * tf2::game::weapons::damage::U;
    sim.app.world_mut().write_message(LaunchRequest { flyer: f, start, shown: None, tag: None });
    sim.ticks(1);
    run(&mut sim, p, 3, |s, k| s.intent(p).secondary = k == 0);
    let ticks = taken_while(&mut sim, p, DamageKind::Burn, 200, |s, _| hold_still(s, p));
    assert_eq!(ticks.iter().map(|t| t.1).collect::<Vec<_>>(), [4.0], "{ticks:?}");
}

/// A server and one client on TF2's movement holding the Scattergun
/// (400 units/s), the client running forward: the server's and the
/// client's player.
fn running_client() -> (chimera_net::harness::NetSim, Entity, Entity) {
    use chimera_net::{harness::NetSim, net::memory::LinkConditions};
    let conditions = LinkConditions { latency: std::time::Duration::from_millis(30), jitter: std::time::Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(WithItems, conditions, 5, 0, |app| {
        app.add_plugins(GreyboxMapPlugin).insert_resource(chimera_core::slots::Loadout { movement: tf2::game::movement::ID });
    });
    sim.add_client(|_| {});
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(900);
    let theirs = sim.character_of(0).unwrap();
    let w = give(sim.server.app.world_mut(), theirs, "tf2:tf_weapon_scattergun").unwrap();
    sim.server.app.world_mut().get_mut::<Inventory>(theirs).unwrap().wanted = Some(w);
    sim.ticks(150);
    let me = sim.local_player(0).unwrap();
    (sim, theirs, me)
}

/// Both machines' top speed for the player and the client's prediction
/// errors, `n` ticks of running forward.
fn run_both(sim: &mut chimera_net::harness::NetSim, theirs: Entity, me: Entity, n: usize) -> Vec<(f32, f32)> {
    let top = |w: &World, e: Entity| w.get::<SourceTraits>(e).and_then(|t| t.max_speed).unwrap_or(0.0);
    let mut out = Vec::new();
    for _ in 0..n {
        sim.clients[0].app.world_mut().get_mut::<chimera_core::core::Intent>(me).unwrap().move_axis = Vec2::Y;
        sim.step();
        out.push((top(sim.server.app.world(), theirs), top(sim.clients[0].app.world(), me)));
    }
    out
}

fn errors(sim: &chimera_net::harness::NetSim) -> u64 {
    sim.clients[0].app.world().resource::<chimera_net::net::predict::NetGraph>().errors
}

#[test]
fn a_clients_own_speed_boost_and_heads_are_predicted_without_corrections() {
    let (mut sim, theirs, me) = running_client();
    run_both(&mut sim, theirs, me, 60);
    let quiet = errors(&sim);
    // A 2-s boost from the server (as a hit gives it): the client takes
    // it once, then runs at 505 as the server does, and ends it on the
    // same tick.
    let now = sim.server.app.world().resource::<chimera_core::core::SimClock>().now;
    sim.server.app.world_mut().entity_mut(theirs).insert(SpeedBoost { until: now + 2.0, mult: effects::BOOST_MULT, cap: effects::BOOST_CAP });
    let speeds = run_both(&mut sim, theirs, me, 60);
    assert!(speeds.iter().any(|s| (s.1 - 505.0).abs() < 0.01), "the client runs boosted: {speeds:?}");
    let taken = errors(&sim);
    assert!(taken - quiet <= 1, "the boost's start, once: {}", taken - quiet);
    let speeds = run_both(&mut sim, theirs, me, 180);
    assert_eq!(errors(&sim), taken, "no correction while boosted or at its end");
    assert!(speeds.iter().any(|s| (s.1 - 505.0).abs() < 0.01) && speeds.last().is_some_and(|s| (s.1 - 400.0).abs() < 0.01), "{speeds:?}");
    assert!(sim.clients[0].app.world().get::<SpeedBoost>(me).is_none(), "ended here too");
    // Two heads (§1.6): 400 + 2 x 8 % of 400 on both.
    sim.server.app.world_mut().entity_mut(theirs).insert(effects::Heads(2));
    let speeds = run_both(&mut sim, theirs, me, 60);
    let taken = errors(&sim);
    let speeds = [speeds, run_both(&mut sim, theirs, me, 120)].concat();
    assert_eq!(errors(&sim), taken, "no correction after the heads came");
    assert!(speeds.last().is_some_and(|s| (s.0 - 464.0).abs() < 0.01 && (s.1 - 464.0).abs() < 0.01), "{:?}", speeds.last());
}
