//! TF2's batch-3 unlocks, part 2 (specs/tf2/batch3b_mechanics.md, its test
//! cases): a made-up item schema in items_game's shape (the spec's
//! attribute values, nothing copied from the game) through the converter,
//! on the headless harness: the fire family (Phlogistinator, Backburner,
//! Degreaser, Dragon's Fury, Manmelter, Gas Passer), the movement kit
//! (B.A.S.E. Jumper, Mantreads, Thermal Thruster) and the Cow Mangler,
//! the Hitman's Heatmaker and the Razorback. The shield entity (§1) is
//! batch3.rs's.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Health, Hitgroup, SimClock, Team, Velocity};
use chimera_sim::{
    status::{Condition, Conditions},
    testkit,
    weapon::{Inventory, Magazine, Weapon, WeaponState, give},
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use tf2::{
    game::{
        TICK_INTERVAL,
        items::{
            ItemPack, convert,
            consumables::Tf2Consumable,
            fire::{GAS, Tf2Revenge},
            kit::{Tf2Chute, Tf2Thruster},
            meters::Tf2Meter,
            misc::Tf2Mangler,
        },
        weapons::{
            FLAMETHROWER, KNIFE, MINIGUN, ROCKET_LAUNCHER, SHOVEL,
            flame::{self, Burning, CRIT_BOOST},
            script_numbers::GunNumbers,
        },
    },
    harness::Tf2Sim,
};

use super::weapons::{fired, hold_still, holding, run};

const U: f32 = 0.0254;

/// The attribute classes the made-up schema uses.
const DEFS: &[(&str, &str)] = &[
    ("airblast_disabled", "value_is_additive"),
    ("burn_damage_earns_rage", "value_is_additive"),
    ("set_flamethrower_back_crit", "value_is_additive"),
    ("mult_airblast_cost", "value_is_percentage"),
    ("mult_single_wep_deploy_time", "value_is_percentage"),
    ("mult_switch_from_wep_deploy_time", "value_is_percentage"),
    ("mult_wpn_burndmg", "value_is_percentage"),
    ("extinguish_restores_health", "value_is_additive"),
    ("mult_maxammo_primary", "value_is_percentage"),
    ("energy_weapon_no_ammo", "value_is_additive"),
    ("extinguish_revenge", "value_is_additive"),
    ("mult_projectile_speed", "value_is_percentage"),
    ("item_meter_charge_rate", "value_is_additive"),
    ("item_meter_damage_for_full_charge", "value_is_additive"),
    ("parachute_attribute", "value_is_additive"),
    ("airblast_vulnerability_multiplier", "value_is_percentage"),
    ("boots_falling_stomp", "value_is_additive"),
    ("damage_force_reduction", "value_is_percentage"),
    ("thermal_thruster", "value_is_additive"),
    ("crits_become_minicrits", "value_is_additive"),
    ("energy_weapon_charged_shot", "value_is_additive"),
    ("rage_on_kill", "value_is_additive"),
    ("bodyshot_damage_modify", "value_is_percentage"),
    ("set_blockbackstab_once", "value_is_additive"),
];

/// (index, name, class, slot, used by, attributes).
#[allow(clippy::type_complexity)]
const ITEMS: &[(u32, &str, &str, &str, &str, &[(&str, &str)])] = &[
    (594, "The Phlogistinator", "tf_weapon_flamethrower", "primary", "pyro", &[("airblast_disabled", "1"), ("burn_damage_earns_rage", "1")]),
    (40, "The Backburner", "tf_weapon_flamethrower", "primary", "pyro", &[
        ("set_flamethrower_back_crit", "1"), ("mult_airblast_cost", "2.5"), ("extinguish_restores_health", "20"),
    ]),
    (215, "The Degreaser", "tf_weapon_flamethrower", "primary", "pyro", &[
        ("mult_airblast_cost", "1.25"), ("mult_single_wep_deploy_time", "0.4"), ("mult_switch_from_wep_deploy_time", "0.7"),
        ("mult_wpn_burndmg", "0.34"),
    ]),
    (1178, "The Dragon's Fury", "tf_weapon_rocketlauncher_fireball", "primary", "pyro", &[
        ("mult_airblast_cost", "0.25"), ("mult_maxammo_primary", "0.2"), ("extinguish_restores_health", "20"),
    ]),
    (595, "The Manmelter", "tf_weapon_flaregun_revenge", "secondary", "pyro", &[
        ("energy_weapon_no_ammo", "1"), ("extinguish_revenge", "1"), ("mult_projectile_speed", "1.5"),
    ]),
    (1180, "The Gas Passer", "tf_weapon_jar_gas", "secondary", "pyro", &[
        ("item_meter_charge_rate", "60"), ("item_meter_damage_for_full_charge", "750"),
    ]),
    (1101, "The B.A.S.E. Jumper", "tf_weapon_parachute", "secondary", "soldier", &[("parachute_attribute", "1")]),
    (444, "The Mantreads", "tf_wearable", "secondary", "soldier", &[
        ("airblast_vulnerability_multiplier", "0.25"), ("boots_falling_stomp", "1"), ("damage_force_reduction", "0.25"),
    ]),
    (1179, "The Thermal Thruster", "tf_weapon_rocketpack", "secondary", "pyro", &[("thermal_thruster", "1"), ("item_meter_charge_rate", "30")]),
    (441, "The Cow Mangler 5000", "tf_weapon_particle_cannon", "primary", "soldier", &[
        ("crits_become_minicrits", "1"), ("energy_weapon_charged_shot", "1"), ("energy_weapon_no_ammo", "1"),
    ]),
    (752, "The Hitman's Heatmaker", "tf_weapon_sniperrifle", "primary", "sniper", &[("rage_on_kill", "35"), ("bodyshot_damage_modify", "0.8")]),
    (57, "The Razorback", "tf_wearable_razorback", "secondary", "sniper", &[("set_blockbackstab_once", "1"), ("item_meter_charge_rate", "30")]),
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

fn hp(sim: &Sim, e: Entity) -> f32 {
    (sim.app.world().get::<Health>(e).unwrap().current * 100.0 * 100.0).round() / 100.0
}

fn set_hp(sim: &mut Sim, e: Entity, p: f32) {
    sim.app.world_mut().get_mut::<Health>(e).unwrap().current = p / 100.0;
}

fn has(sim: &Sim, e: Entity, c: Condition) -> bool {
    sim.app.world().resource::<Conditions>().has(e, c, now(sim))
}

fn carry(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    let w = give(sim.app.world_mut(), p, id).unwrap();
    sim.ticks(2);
    w
}

fn draw(sim: &mut Sim, p: Entity, w: Entity) {
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w));
}

fn deal(sim: &mut Sim, attacker: Option<Entity>, target: Entity, amount: f32, kind: DamageKind, weapon: Option<&'static str>) {
    sim.app.world_mut().resource_mut::<Messages<Damage>>().write(Damage {
        target,
        attacker,
        amount: amount / 100.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Generic,
        kind,
        weapon,
        force: Vec3::ZERO,
    });
    sim.ticks(1);
}

/// A target `units` ahead of `from` holding `gun` with `points`, still.
fn victim(sim: &mut Sim, from: Vec3, units: f32, gun: &str, points: f32) -> Entity {
    let (t, _) = holding(sim, from - Vec3::Z * units * U, gun);
    set_hp(sim, t, points);
    hold_still(sim, t);
    t
}

fn teams(sim: &mut Sim, a: Entity, ta: u8, b: Entity, tb: u8) {
    sim.app.world_mut().entity_mut(a).insert(Team(ta));
    sim.app.world_mut().entity_mut(b).insert(Team(tb));
}

fn meter(sim: &Sim, w: Entity) -> f32 {
    sim.app.world().get::<Tf2Meter>(w).unwrap().state.value
}

fn horizontal(v: Vec3) -> f32 {
    Vec2::new(v.x, v.z).length() / U
}

// ---- 2. Phlogistinator ----

#[test]
fn the_phlogistinators_rage_fills_from_300_fire_damage_only() {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:phlogistinator");
    let t = victim(&mut sim, from, 300.0, MINIGUN, 5000.0);
    assert!(sim.app.world().get::<flame::Tf2Flame>(w).unwrap().params.airblast.is_none(), "no airblast");
    deal(&mut sim, Some(p), t, 60.0, DamageKind::Bullet, Some("tf2:phlogistinator"));
    assert_eq!(meter(&sim, w), 0.0, "fire damage only");
    for _ in 0..30 {
        deal(&mut sim, Some(p), t, 10.0, DamageKind::Burn, Some("tf2:phlogistinator"));
    }
    assert!((meter(&sim, w) - 100.0).abs() < 1e-3, "{}", meter(&sim, w));
}

#[test]
fn the_phlogistinators_taunt_is_invulnerable_then_crits_until_11_2_s() {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:phlogistinator");
    // Rage 60: attack2 does nothing.
    sim.app.world_mut().get_mut::<Tf2Meter>(w).unwrap().state.value = 60.0;
    run(&mut sim, p, 3, |s, k| s.intent(p).secondary = k == 0);
    assert!(!has(&sim, p, Condition::Phased) && !has(&sim, p, Condition::Game(CRIT_BOOST)));
    sim.app.world_mut().get_mut::<Tf2Meter>(w).unwrap().state.value = 100.0;
    run(&mut sim, p, 2, |s, k| s.intent(p).secondary = k == 0);
    assert!(has(&sim, p, Condition::Phased), "invulnerable from the start");
    assert!(has(&sim, p, Condition::Game(CRIT_BOOST)));
    let before = hp(&sim, p);
    deal(&mut sim, None, p, 40.0, DamageKind::Bullet, None);
    assert_eq!(hp(&sim, p), before, "no damage in the taunt");
    sim.seconds(2.7);
    assert!(!has(&sim, p, Condition::Phased), "2.61 s");
    assert!(has(&sim, p, Condition::Game(CRIT_BOOST)));
    sim.seconds(7.9);
    assert!(has(&sim, p, Condition::Game(CRIT_BOOST)), "lingers after the rage");
    sim.seconds(0.8);
    assert!(!has(&sim, p, Condition::Game(CRIT_BOOST)), "over at 11.2 s");
}

// ---- 3. Backburner ----

/// The flame damage dealt in 0.6 s on a target 80 units ahead whose
/// facing is `yaw`.
fn flame_on(id: &str, target_yaw: f32) -> f32 {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, id);
    let t = victim(&mut sim, from, 80.0, MINIGUN, 5000.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let before = hp(&sim, t);
    run(&mut sim, p, 40, |s, _| {
        s.intent(p).fire = true;
        s.intent(t).yaw = target_yaw;
    });
    before - hp(&sim, t)
}

#[test]
fn the_backburner_crits_from_behind_only() {
    // The Pyro looks along -Z: a target facing -Z shows him its back.
    let back = flame_on("tf2:backburner", 0.0);
    let front = flame_on("tf2:backburner", std::f32::consts::PI);
    assert!(back > 2.5 * front, "behind {back}, in front {front}");
    // 40 degrees off the back: no crit.
    let off = flame_on("tf2:backburner", 40f32.to_radians());
    assert!(off < 1.3 * front, "40 off {off}, front {front}");
}

#[test]
fn the_airblast_costs_50_on_the_backburner_and_25_on_the_degreaser() {
    for (id, left) in [("tf2:backburner", 150), ("tf2:degreaser", 175)] {
        let mut sim = new_sim();
        let (p, w) = holding(&mut sim, greybox::SPAWNS[0], id);
        run(&mut sim, p, 2, |s, k| s.intent(p).secondary = k == 0);
        assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, left, "{id}");
    }
}

#[test]
fn the_degreaser_deploys_in_0_2_s_and_burns_for_1_a_tick() {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:degreaser");
    assert!((sim.app.world().get::<Weapon>(w).unwrap().draw_time - 0.2).abs() < 1e-4);
    let f = sim.app.world().get::<flame::Tf2Flame>(w).unwrap().params.clone();
    assert!((f.burn_damage - 1.36).abs() < 1e-4);
    let _ = p;
    // A tick of 1.36 reads 1.
    let t = victim(&mut sim, from, 300.0, MINIGUN, 500.0);
    let before = hp(&sim, t);
    let lit = Burning { left: 0.5, damage: 1.36, ..flame::ignited(now(&sim), p, "tf2:degreaser", &f) };
    sim.app.world_mut().entity_mut(t).insert(lit);
    sim.seconds(0.6);
    assert_eq!(before - hp(&sim, t), 1.0);
}

// ---- 5. Dragon's Fury ----

#[test]
fn the_dragons_fury_hits_28_at_200_ignites_and_refills_faster() {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:dragons_fury");
    let t = victim(&mut sim, from, 200.0, MINIGUN, 1000.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let before = hp(&sim, t);
    let ev = run(&mut sim, p, 6, |s, k| s.intent(p).fire = k == 0);
    assert_eq!(fired(&ev).len(), 1);
    assert_eq!(before - hp(&sim, t), 28.0);
    assert!(sim.app.world().get::<Burning>(t).is_some(), "ignited");
    // The refill from the hit on: under 0.6 s, not 0.8.
    let next = sim.app.world().get::<WeaponState>(w).unwrap().next_primary;
    let shot_at = now(&sim) - 5.0 * TICK_INTERVAL;
    assert!(next - shot_at < 0.6 && next - shot_at > 0.5, "{}", next - shot_at);
    // A burning target: × 3.
    sim.seconds(1.0);
    let before = hp(&sim, t);
    run(&mut sim, p, 8, |s, k| s.intent(p).fire = k == 0);
    // 28 × 3 (an afterburn tick of 4 may land meanwhile).
    let dealt = before - hp(&sim, t);
    assert!((83.0..=90.0).contains(&dealt), "{dealt}");
}

#[test]
fn the_dragons_fury_airblast_costs_5_and_refills_in_1_6_s() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], "tf2:dragons_fury");
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, 40);
    run(&mut sim, p, 1, |s, _| s.intent(p).secondary = true);
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, 35);
    let left = sim.app.world().get::<WeaponState>(w).unwrap().next_primary - now(&sim);
    assert!((left - 1.6).abs() < 0.03, "{left}");
}

// ---- 6. Manmelter ----

#[test]
fn the_manmelter_puts_out_a_teammate_for_a_crit_and_20_health() {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:manmelter");
    let mate = victim(&mut sim, from, 100.0, MINIGUN, 200.0);
    teams(&mut sim, p, 2, mate, 2);
    set_hp(&mut sim, p, 100.0);
    let lit = flame::ignited(now(&sim), p, FLAMETHROWER, &flame::stock_params());
    sim.app.world_mut().entity_mut(mate).insert(lit);
    let at = sim.position(mate);
    testkit::aim_at(&mut sim, p, at);
    run(&mut sim, p, 2, |s, k| s.intent(p).secondary = k == 0);
    assert!(sim.app.world().get::<Burning>(mate).is_none(), "put out");
    assert_eq!(sim.app.world().get::<Tf2Revenge>(w).unwrap().stored, 1);
    assert_eq!(hp(&sim, p), 120.0);
    // The next shot crits: 90.
    let enemy = victim(&mut sim, from + Vec3::X * 200.0 * U, 300.0, MINIGUN, 1000.0);
    sim.app.world_mut().entity_mut(enemy).insert(Team(3));
    let at = sim.position(enemy);
    testkit::aim_at(&mut sim, p, at);
    let before = hp(&sim, enemy);
    run(&mut sim, p, 20, |s, k| s.intent(p).fire = k == 0);
    assert_eq!(before - hp(&sim, enemy), 90.0);
    assert_eq!(sim.app.world().get::<Tf2Revenge>(w).unwrap().stored, 0);
}

// ---- 7. Gas Passer ----

#[test]
fn the_gas_passer_coats_and_the_next_damage_ignites() {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:gas_passer");
    assert!(sim.app.world().get::<Tf2Consumable>(w).unwrap().state.meter < 0.02, "starts empty (1 s of its 60)");
    sim.app.world_mut().get_mut::<Tf2Consumable>(w).unwrap().state.meter = 1.0;
    let t = victim(&mut sim, from, 250.0, MINIGUN, 1000.0);
    let late = victim(&mut sim, from + Vec3::X * 600.0 * U, 250.0, MINIGUN, 1000.0);
    teams(&mut sim, p, 2, t, 3);
    sim.app.world_mut().entity_mut(late).insert(Team(3));
    // Its jar breaks 100 units from him (the cloud 30 over it).
    let at = sim.position(t) + Vec3::X * 100.0 * U;
    let flyer = sim.app.world_mut().spawn_empty().id();
    sim.app.world_mut().resource_mut::<Messages<chimera_sim::weapon::projectile::FlyerHit>>().write(chimera_sim::weapon::projectile::FlyerHit {
        flyer,
        weapon: "tf2:gas_passer",
        owner: Some(p),
        other: None,
        point: at,
        normal: Vec3::Y,
        velocity: Vec3::ZERO,
        character: false,
        world: true,
        gone: false,
        hitgroup: Hitgroup::Generic,
    });
    sim.ticks(2);
    assert!(!has(&sim, late, Condition::Game(GAS)), "out of reach");
    // One walking into the cloud while it stands is coated then.
    sim.app.world_mut().get_mut::<Transform>(late).unwrap().translation = at + Vec3::X * 50.0 * U;
    sim.seconds(0.5);
    assert!(has(&sim, late, Condition::Game(GAS)), "entered the cloud");
    assert!(has(&sim, t, Condition::Game(GAS)), "coated");
    assert!(sim.app.world().get::<Burning>(t).is_none());
    deal(&mut sim, None, t, 10.0, DamageKind::Bullet, None);
    sim.ticks(1);
    assert!(!has(&sim, t, Condition::Game(GAS)), "the gas is gone");
    let b = *sim.app.world().get::<Burning>(t).expect("burning");
    assert!((b.left - 6.0).abs() < 0.6 && b.attacker == Some(p), "{b:?}");
    // The meter fills from damage dealt: 750 to full.
    sim.app.world_mut().get_mut::<Tf2Consumable>(w).unwrap().state.meter = 0.0;
    deal(&mut sim, Some(p), t, 375.0, DamageKind::Bullet, None);
    let m = sim.app.world().get::<Tf2Consumable>(w).unwrap().state.meter;
    assert!((0.5..0.52).contains(&m), "{m}");
}

// ---- 8. B.A.S.E. Jumper ----

#[test]
fn the_chute_holds_the_fall_to_106_and_75_across_once_an_air_time() {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, SHOVEL);
    let c = carry(&mut sim, p, "tf2:base_jumper");
    // High up, falling.
    let high = sim.position(p) + Vec3::Y * 600.0 * U;
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = high;
    sim.ticks(10);
    run(&mut sim, p, 2, |s, k| s.intent(p).jump = k == 0);
    assert!(sim.app.world().get::<Tf2Chute>(c).unwrap().state.open);
    sim.ticks(5);
    let y0 = sim.position(p).y;
    sim.seconds(1.0);
    let fell = (y0 - sim.position(p).y) / U;
    assert!((fell - 106.0).abs() < 3.0, "{fell}");
    run(&mut sim, p, 14, |s, _| s.intent(p).move_axis = Vec2::new(0.0, 1.0));
    let v = sim.app.world().get::<Velocity>(p).unwrap().0;
    assert!((horizontal(v) - 75.0).abs() < 1.5, "{}", horizontal(v));
    run(&mut sim, p, 1, |_, _| {});
    // Closed, then a press keeps it closed.
    run(&mut sim, p, 2, |s, k| s.intent(p).jump = k == 0);
    assert!(!sim.app.world().get::<Tf2Chute>(c).unwrap().state.open);
    run(&mut sim, p, 2, |s, k| s.intent(p).jump = k == 0);
    assert!(!sim.app.world().get::<Tf2Chute>(c).unwrap().state.open, "once an air time");
}

// ---- 9. Mantreads and the Thermal Thruster ----

#[test]
fn the_mantreads_stomp_for_99_at_900_and_take_no_fall_damage() {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let t = victim(&mut sim, from, 0.0, MINIGUN, 1000.0);
    let (p, _) = holding(&mut sim, from + Vec3::X * 300.0 * U, ROCKET_LAUNCHER);
    carry(&mut sim, p, "tf2:mantreads");
    teams(&mut sim, p, 2, t, 3);
    sim.seconds(0.5);
    let top = sim.position(t) + Vec3::Y * 90.0 * U;
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = top;
    sim.app.world_mut().get_mut::<chimera_core::core::MovementState>(p).unwrap().on_ground = false;
    let (vb, sb) = (hp(&sim, t), hp(&sim, p));
    // Falling at 900 at each tick's end (888 at its start), from 40
    // units over his head.
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = top + Vec3::Y * 40.0 * U;
    for _ in 0..12 {
        let falling = !sim.app.world().get::<chimera_core::core::MovementState>(p).unwrap().on_ground;
        if falling {
            sim.app.world_mut().get_mut::<Velocity>(p).unwrap().0 = Vec3::NEG_Y * 888.0 * U;
        }
        sim.ticks(1);
    }
    assert_eq!(vb - hp(&sim, t), 99.0, "10 + 3 x 200 x 894 / 6000");
    assert_eq!(hp(&sim, p), sb, "no fall damage");
}

#[test]
fn the_mantreads_take_a_quarter_of_the_airblast() {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (pyro, _) = holding(&mut sim, from, FLAMETHROWER);
    let s = victim(&mut sim, from, 100.0, ROCKET_LAUNCHER, 200.0);
    carry(&mut sim, s, "tf2:mantreads");
    teams(&mut sim, pyro, 2, s, 3);
    hold_still(&mut sim, s);
    sim.intent(pyro).yaw = 0.0;
    sim.intent(pyro).pitch = 0.0;
    run(&mut sim, pyro, 1, |sim, _| sim.intent(pyro).secondary = true);
    let v = sim.app.world().get::<Velocity>(s).unwrap().0;
    assert!((horizontal(v) - 111.8).abs() < 8.0 && (v.y / U - 100.0).abs() < 8.0, "{} {}", horizontal(v), v.y / U);
}

#[test]
fn the_thruster_hops_then_launches_810_and_315_up_and_lands_pushing() {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, FLAMETHROWER);
    let w = carry(&mut sim, p, "tf2:thermal_thruster");
    draw(&mut sim, p, w);
    sim.intent(p).yaw = 0.0;
    sim.intent(p).pitch = 0.0;
    run(&mut sim, p, 2, |s, k| s.intent(p).fire = k == 0);
    assert!((sim.app.world().get::<Tf2Thruster>(w).unwrap().state.meter - 50.0).abs() < 0.2);
    assert!(sim.app.world().get::<Velocity>(p).unwrap().0.y > 0.0, "the hop");
    let mut launched = None;
    run(&mut sim, p, 50, |s, k| {
        let v = s.app.world().get::<Velocity>(p).unwrap().0;
        if launched.is_none() && horizontal(v) > 700.0 {
            launched = Some((k, v));
        }
        s.intent(p).fire = k == 30;
    });
    let (k, v) = launched.expect("launched");
    assert!((41..=44).contains(&k), "0.65 s: tick {k}");
    assert!((horizontal(v) - 810.0).abs() < 5.0 && v.y / U > 250.0, "{} {}", horizontal(v), v.y / U);
    let m = sim.app.world().get::<Tf2Thruster>(w).unwrap().state.meter;
    assert!((50.0..55.0).contains(&m), "no second launch in flight: {m}");
    assert!(sim.app.world().get::<Tf2Thruster>(w).unwrap().state.flying);
}

#[test]
fn the_thruster_landing_pushes_enemies_within_220_and_cuts_to_300() {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, FLAMETHROWER);
    let w = carry(&mut sim, p, "tf2:thermal_thruster");
    let e = victim(&mut sim, from, 120.0, MINIGUN, 500.0);
    teams(&mut sim, p, 2, e, 3);
    // In flight just above the ground, coming down fast across.
    let at = sim.position(p) + Vec3::Y * 10.0 * U;
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = at;
    sim.app.world_mut().get_mut::<Tf2Thruster>(w).unwrap().state.flying = true;
    sim.app.world_mut().get_mut::<Velocity>(p).unwrap().0 = Vec3::new(810.0, -300.0, 0.0) * U;
    let before = hp(&sim, e);
    sim.ticks(4);
    assert!(!sim.app.world().get::<Tf2Thruster>(w).unwrap().state.flying, "landed");
    let v = sim.app.world().get::<Velocity>(e).unwrap().0;
    assert!(v.y > 100.0 * U || horizontal(v) > 100.0, "pushed: {v}");
    assert_eq!(hp(&sim, e), before, "no damage");
    let mine = sim.app.world().get::<Velocity>(p).unwrap().0;
    assert!(horizontal(mine) <= 300.5, "{}", horizontal(mine));
}

// ---- 11. Cow Mangler ----

#[test]
fn the_cow_mangler_fires_4_shots_and_a_charged_one_refused_below_full() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], "tf2:cow_mangler");
    let ev = run(&mut sim, p, 4 * 54, |s, _| s.intent(p).fire = true);
    let f = fired(&ev);
    assert!(f.len() >= 4, "{f:?}");
    assert!(f.windows(2).take(3).all(|x| x[1] - x[0] >= 53), "{f:?}");
    // Energy 15 (3 of 4): attack2 refused.
    sim.seconds(5.0);
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 3;
    run(&mut sim, p, 2, |s, k| s.intent(p).secondary = k == 0);
    assert!(sim.app.world().get::<Tf2Mangler>(w).unwrap().state.since < 0.0, "refused");
}

#[test]
fn the_cow_manglers_charged_shot_fires_after_2_s_mini_crits_and_burns() {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:cow_mangler");
    let t = victim(&mut sim, from, 300.0, MINIGUN, 1000.0);
    teams(&mut sim, p, 2, t, 3);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let before = hp(&sim, t);
    let ev = run(&mut sim, p, 160, |s, k| s.intent(p).secondary = k == 0);
    let f = fired(&ev);
    assert_eq!(f.len(), 1, "{f:?}");
    assert!((132..=135).contains(&f[0]), "2.0 s: {f:?}");
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, 0);
    let dealt = before - hp(&sim, t);
    assert!(dealt > 100.0, "a mini-crit: {dealt}");
    assert!(sim.app.world().get::<Burning>(t).is_some(), "burning");
}

// ---- 12. Hitman's Heatmaker ----

#[test]
fn the_heatmakers_focus_fills_35_a_kill_and_lasts_10_s() {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:hitmans_heatmaker");
    let t = victim(&mut sim, from, 300.0, MINIGUN, 30.0);
    deal(&mut sim, Some(p), t, 40.0, DamageKind::Bullet, Some("tf2:hitmans_heatmaker"));
    assert_eq!(meter(&sim, w), 35.0);
    sim.app.world_mut().get_mut::<Tf2Meter>(w).unwrap().state.value = 100.0;
    run(&mut sim, p, 2, |s, k| s.intent(p).reload = k == 0);
    assert!(sim.app.world().get::<Tf2Meter>(w).unwrap().in_force(now(&sim)));
    sim.seconds(9.0);
    assert!(sim.app.world().get::<Tf2Meter>(w).unwrap().in_force(now(&sim)));
    sim.seconds(1.5);
    assert!(!sim.app.world().get::<Tf2Meter>(w).unwrap().in_force(now(&sim)), "over");
}

// ---- 10. Razorback ----

#[test]
fn the_razorback_blocks_one_backstab_and_stalls_the_spy() {
    let mut sim = new_sim();
    let from = greybox::SPAWNS[0];
    let (spy, knife) = holding(&mut sim, from, KNIFE);
    let t = victim(&mut sim, from, 50.0, "tf2:tf_weapon_smg", 125.0);
    carry(&mut sim, t, "tf2:razorback");
    teams(&mut sim, spy, 2, t, 3);
    sim.intent(spy).yaw = 0.0;
    sim.intent(spy).pitch = 0.0;
    let stab = |sim: &mut Sim| {
        run(sim, spy, 20, |s, k| {
            s.intent(t).yaw = 0.0;
            s.intent(spy).yaw = 0.0;
            s.intent(spy).fire = k == 0;
        });
    };
    stab(&mut sim);
    assert_eq!(hp(&sim, t), 125.0, "blocked");
    let next = sim.app.world().get::<WeaponState>(knife).unwrap().next_primary;
    assert!(next - now(&sim) > 1.5, "the Spy waits: {}", next - now(&sim));
    sim.seconds(2.0);
    stab(&mut sim);
    assert!(hp(&sim, t) <= 0.0, "broken: the next one kills");
}
