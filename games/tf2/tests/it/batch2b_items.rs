//! TF2's batch-2 unlocks, part 2 (specs/tf2/batch2b_mechanics.md, its test
//! cases): a made-up item schema in items_game's shape (the spec's
//! attribute values, nothing copied from the game) through the converter,
//! played on the headless harness: the consumables (the Sandvich and its
//! thrown kit, the Dalokohs Bar, the Buffalo Steak Sandvich, Bonk and
//! Crit-a-Cola, Jarate and Mad Milk), the meters (hype, boost, crikey,
//! heads), the sniper rifles (Machina, Sydney Sleeper, Classic, Bazaar
//! Bargain), the revolvers (Ambassador, Diamondback) and the crit
//! feedback (the mini-crit's sound and the texts' messages).

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Health, Hitgroup, SimClock, Team};
use chimera_sim::{
    status::{Condition, Conditions},
    testkit,
    weapon::{Inventory, Weapon, give, projectile::{Flyer, FlyerHit}},
};
use chimera_world::{greybox::{self, GreyboxMapPlugin}, harness::Sim};
use source_engine::source_movement::SourceTraits;
use tf2::{
    game::{
        TICK_INTERVAL,
        items::{ItemPack, consumables::{self, Tf2Consumable}, convert, meters::Tf2Meter},
        weapons::{SNIPER_RIFLE, crit_hits::CritHit, script_numbers::GunNumbers, sniper::Tf2Sniper},
    },
    harness::Tf2Sim,
};

use super::weapons::{fired, health_points, hold_still, holding, no_spread, run};

const U: f32 = 0.0254;

/// The attribute classes the made-up schema uses (named as their class).
const DEFS: &[(&str, &str)] = &[
    ("item_meter_charge_rate", "value_is_additive"),
    ("set_weapon_mode", "value_is_additive"),
    ("lunchbox_healing_scale", "value_is_percentage"),
    ("energy_buff_dmg_taken_multiplier", "value_is_percentage"),
    ("mod_mark_attacker_for_death", "value_is_additive"),
    ("extinguish_reduces_cooldown", "value_is_percentage"),
    ("hype_on_damage", "value_is_additive"),
    ("boost_on_damage", "value_is_additive"),
    ("hype_resets_on_jump", "value_is_additive"),
    ("lose_hype_on_take_damage", "value_is_additive"),
    ("mult_player_movespeed", "value_is_percentage"),
    ("minicrit_boost_charge_rate", "value_is_additive"),
    ("minicrit_boost_when_charged", "value_is_additive"),
    ("sniper_full_charge_damage_bonus", "value_is_percentage"),
    ("sniper_penetrate_players_when_charged", "value_is_additive"),
    ("sniper_only_fire_zoomed", "value_is_additive"),
    ("mult_sniper_charge_per_sec", "value_is_percentage"),
    ("jarate_duration", "value_is_additive"),
    ("bodyshot_damage_modify", "value_is_percentage"),
    ("mult_dmg", "value_is_percentage"),
    ("mult_postfiredelay", "value_is_inverted_percentage"),
    ("sapper_kills_collect_crits", "value_is_additive"),
];

/// (index, name, class, slot, used by, attributes).
#[allow(clippy::type_complexity)]
const ITEMS: &[(u32, &str, &str, &str, &str, &[(&str, &str)])] = &[
    (42, "The Sandvich", "tf_weapon_lunchbox", "secondary", "heavy", &[("item_meter_charge_rate", "30")]),
    (159, "The Dalokohs Bar", "tf_weapon_lunchbox", "secondary", "heavy", &[("item_meter_charge_rate", "10"), ("set_weapon_mode", "1")]),
    (311, "The Buffalo Steak Sandvich", "tf_weapon_lunchbox", "secondary", "heavy", &[
        ("item_meter_charge_rate", "30"), ("set_weapon_mode", "2"), ("energy_buff_dmg_taken_multiplier", "1.2"),
    ]),
    (1190, "The Second Banana", "tf_weapon_lunchbox", "secondary", "heavy", &[
        ("item_meter_charge_rate", "10"), ("set_weapon_mode", "6"), ("lunchbox_healing_scale", "0.67"),
    ]),
    (46, "Bonk! Atomic Punch", "tf_weapon_lunchbox_drink", "secondary", "scout", &[]),
    (163, "Crit-a-Cola", "tf_weapon_lunchbox_drink", "secondary", "scout", &[("set_weapon_mode", "2"), ("mod_mark_attacker_for_death", "5")]),
    (58, "Jarate", "tf_weapon_jar", "secondary", "sniper", &[("extinguish_reduces_cooldown", "0.8")]),
    (222, "Mad Milk", "tf_weapon_jar_milk", "secondary", "scout", &[("extinguish_reduces_cooldown", "0.8")]),
    (448, "The Soda Popper", "tf_weapon_soda_popper", "primary", "scout", &[("hype_on_damage", "1")]),
    (772, "Baby Face's Blaster", "tf_weapon_pep_brawler_blaster", "primary", "scout", &[
        ("boost_on_damage", "1"), ("hype_resets_on_jump", "75"), ("lose_hype_on_take_damage", "4"), ("mult_player_movespeed", "0.9"),
    ]),
    (751, "The Cleaner's Carbine", "tf_weapon_charged_smg", "secondary", "sniper", &[
        ("minicrit_boost_charge_rate", "1"), ("minicrit_boost_when_charged", "8"),
    ]),
    (402, "The Bazaar Bargain", "tf_weapon_sniperrifle_decap", "primary", "sniper", &[]),
    (526, "The Machina", "tf_weapon_sniperrifle", "primary", "sniper", &[
        ("set_weapon_mode", "2"), ("sniper_full_charge_damage_bonus", "1.15"), ("sniper_penetrate_players_when_charged", "1"),
        ("sniper_only_fire_zoomed", "1"),
    ]),
    (230, "The Sydney Sleeper", "tf_weapon_sniperrifle", "primary", "sniper", &[
        ("jarate_duration", "5"), ("mult_sniper_charge_per_sec", "1.25"), ("set_weapon_mode", "1"),
    ]),
    (1098, "The Classic", "tf_weapon_sniperrifle_classic", "primary", "sniper", &[("bodyshot_damage_modify", "0.9"), ("set_weapon_mode", "3")]),
    (61, "The Ambassador", "tf_weapon_revolver", "secondary", "spy", &[("mult_dmg", "0.85"), ("mult_postfiredelay", "1.2"), ("set_weapon_mode", "1")]),
    (525, "The Diamondback", "tf_weapon_revolver", "secondary", "spy", &[("mult_dmg", "0.85"), ("sapper_kills_collect_crits", "1")]),
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

/// The pack, the revolver class's script numbers set (the converter
/// reads them from the class script: 40 every 0.5 s).
fn pack() -> ItemPack {
    let mut p = convert::convert(&schema(), &|_| None).pack;
    for r in &mut p.items {
        if r.class == "tf_weapon_revolver" {
            r.base.insert("damage".into(), 40.0);
            r.base.insert("interval".into(), 0.5);
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
    ((h.current * 100.0 * 100.0).round() / 100.0, (h.max * 100.0).round())
}

fn set_health(sim: &mut Sim, e: Entity, points: f32) {
    sim.app.world_mut().get_mut::<Health>(e).unwrap().current = points / 100.0;
}

fn speed(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<SourceTraits>(p).unwrap().max_speed.unwrap()
}

fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

fn has(sim: &Sim, e: Entity, c: Condition) -> bool {
    sim.app.world().resource::<Conditions>().has(e, c, now(sim))
}

fn left(sim: &Sim, e: Entity, c: Condition) -> f32 {
    sim.app.world().resource::<Conditions>().left(e, c, now(sim))
}

fn give_cond(sim: &mut Sim, e: Entity, c: Condition, secs: f64) {
    let t = now(sim) + secs;
    sim.app.world_mut().resource_mut::<Conditions>().give(e, c, t, None);
}

/// `p` gets `id` too (not drawn).
fn carry(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    give(sim.app.world_mut(), p, id).unwrap()
}

/// A target `units` ahead (-Z) of `from` holding `gun`, with `hp` points.
fn victim(sim: &mut Sim, from: Vec3, units: f32, gun: &str, hp: f32) -> Entity {
    let (t, _) = holding(sim, from - Vec3::Z * units * U, gun);
    set_health(sim, t, hp);
    hold_still(sim, t);
    t
}

/// `e` writes damage `amount` (points) on `target` with `weapon`.
fn deal(sim: &mut Sim, attacker: Option<Entity>, target: Entity, amount: f32, weapon: Option<&'static str>) {
    sim.app.world_mut().resource_mut::<Messages<Damage>>().write(Damage {
        target,
        attacker,
        amount: amount / 100.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Bullet,
        weapon,
        force: Vec3::ZERO,
    });
    sim.ticks(1);
}

fn consumable(sim: &Sim, w: Entity) -> tf2::game::items::consumables::UseState {
    sim.app.world().get::<Tf2Consumable>(w).unwrap().state
}

fn meter(sim: &Sim, w: Entity) -> f32 {
    sim.app.world().get::<Tf2Meter>(w).unwrap().state.value
}

/// One press of attack (or attack2) by `p`.
fn press(sim: &mut Sim, p: Entity, alt: bool) {
    run(sim, p, 2, |s, k| {
        if alt {
            s.intent(p).secondary = k == 0;
        } else {
            s.intent(p).fire = k == 0;
        }
    });
}

// ---- 11. Lunchbox items ----

#[test]
fn the_sandvich_heals_75_at_each_of_four_seconds_and_refills_in_30() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], "tf2:sandvich");
    let primary = carry(&mut sim, p, tf2::game::weapons::MINIGUN);
    assert_eq!(points(&sim, p).1, 300.0, "a Heavy");
    set_health(&mut sim, p, 50.0);
    let start = now(&sim);
    run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
    let mut seen = Vec::new();
    for t in [1.05, 2.05, 3.05, 4.05] {
        let n = ((start + t - now(&sim)) / TICK_INTERVAL).round() as u64;
        run(&mut sim, p, n, |s, _| {
            s.intent(p).fire = false;
            // Trying to walk: the taunt holds him.
            s.intent(p).move_axis = Vec2::new(0.0, 1.0);
        });
        seen.push(points(&sim, p).0.round());
    }
    assert_eq!(seen, [125.0, 200.0, 275.0, 300.0]);
    assert!(sim.app.world().get::<chimera_core::core::Velocity>(p).unwrap().0.length() < 0.05, "no moving while eating");
    run(&mut sim, p, 30, |_, _| {});
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(primary), "the primary drawn after");
    assert!(consumable(&sim, w).meter < 0.2);
    sim.seconds(25.0);
    assert!(consumable(&sim, w).meter < 1.0, "not yet");
    sim.seconds(1.0);
    assert!(consumable(&sim, w).meter >= 1.0, "full 30 s after: {}", consumable(&sim, w).meter);
}

#[test]
fn the_second_banana_heals_50_a_step() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:second_banana");
    set_health(&mut sim, p, 50.0);
    run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
    sim.seconds(4.2);
    assert_eq!(points(&sim, p).0.round(), 250.0);
}

#[test]
fn the_dalokohs_bar_heals_25_a_step_and_adds_50_max_health() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:dalokohs_bar");
    set_health(&mut sim, p, 50.0);
    run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
    sim.seconds(4.2);
    assert_eq!((points(&sim, p).0.round(), points(&sim, p).1), (150.0, 350.0));
}

#[test]
fn the_thrown_sandvich_is_a_kit_a_teammate_takes_for_half_his_max() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:sandvich");
    let mate = victim(&mut sim, from, 80.0, tf2::game::weapons::SYRINGE_GUN, 30.0);
    for e in [p, mate] {
        sim.app.world_mut().entity_mut(e).insert(Team(2));
    }
    assert_eq!(points(&sim, mate).1, 150.0, "a Medic");
    press(&mut sim, p, true);
    let kit = {
        let mut q = sim.app.world_mut().query::<(&Flyer, &Transform)>();
        q.iter(sim.app.world()).find(|(f, _)| f.weapon == consumables::LUNCH_KIT).map(|(_, t)| t.translation)
    };
    let kit = kit.expect("a kit thrown");
    set_health(&mut sim, mate, 30.0);
    sim.app.world_mut().get_mut::<Transform>(mate).unwrap().translation = kit;
    sim.ticks(3);
    assert_eq!(points(&sim, mate).0.round(), 105.0);
}

#[test]
fn the_steak_gives_the_buff_speed_and_the_melee_for_19_s() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:buffalo_steak_sandvich");
    let fists = carry(&mut sim, p, tf2::game::weapons::FISTS);
    carry(&mut sim, p, tf2::game::weapons::MINIGUN);
    run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
    run(&mut sim, p, 70, |s, _| s.intent(p).fire = false);
    assert!(has(&sim, p, Condition::MiniCritBuff), "from 1 s");
    assert!((speed(&sim, p) - 299.0).abs() < 0.01, "{}", speed(&sim, p));
    sim.seconds(4.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(fists), "melee only");
    // Taken x1.2.
    set_health(&mut sim, p, 300.0);
    deal(&mut sim, None, p, 50.0, None);
    assert_eq!(points(&sim, p).0.round(), 240.0);
    sim.seconds(14.6);
    assert!(has(&sim, p, Condition::MiniCritBuff), "until 20 s");
    sim.seconds(0.6);
    assert!(!has(&sim, p, Condition::MiniCritBuff), "gone after 20 s");
}

// ---- 12. Drinks ----

#[test]
fn bonk_phases_from_0_9_to_8_9_s() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:bonk_atomic_punch");
    run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
    run(&mut sim, p, 55, |s, _| s.intent(p).fire = false);
    assert!(!has(&sim, p, Condition::Phased), "not before 0.9 s");
    run(&mut sim, p, 10, |_, _| {});
    assert!(has(&sim, p, Condition::Phased));
    let before = points(&sim, p).0;
    deal(&mut sim, None, p, 50.0, None);
    assert_eq!(points(&sim, p).0, before, "no damage reaches him");
    sim.seconds(7.8);
    assert!(has(&sim, p, Condition::Phased));
    sim.seconds(0.3);
    assert!(!has(&sim, p, Condition::Phased), "gone at 8.9 s");
}

#[test]
fn crit_a_cola_marks_its_drinker_for_5_s_from_each_shot() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:crit_a_cola");
    carry(&mut sim, p, tf2::game::weapons::SCATTERGUN);
    run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
    sim.seconds(2.5);
    assert!(has(&sim, p, Condition::MiniCritBuff));
    assert!(!has(&sim, p, Condition::MarkedForDeath), "silent until he attacks");
    run(&mut sim, p, 3, |s, k| s.intent(p).fire = k == 0);
    let l = left(&sim, p, Condition::MarkedForDeath);
    assert!((4.9..=5.0).contains(&l), "{l}");
}

// ---- 13. Jars ----

/// A Jarate jar breaking `units` from the victim's centre (its own touch).
fn jar_at(units: f32) -> bool {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:jarate");
    let t = victim(&mut sim, from, 400.0, tf2::game::weapons::MINIGUN, 300.0);
    let (min, max) = {
        let w = sim.app.world();
        tf2::game::weapons::blast::boxed(w.get::<Transform>(t).unwrap(), None, w.get::<chimera_core::core::MovementState>(t))
    };
    let centre = (min + max) / 2.0;
    let flyer = sim.app.world_mut().spawn_empty().id();
    sim.app.world_mut().resource_mut::<Messages<FlyerHit>>().write(FlyerHit {
        flyer,
        weapon: "tf2:jarate",
        owner: Some(p),
        other: None,
        point: centre + Vec3::X * units * U,
        normal: Vec3::Y,
        velocity: Vec3::ZERO,
        character: false,
        world: true,
        gone: false,
        hitgroup: Hitgroup::Generic,
    });
    sim.ticks(2);
    let on = has(&sim, t, Condition::Game(consumables::JARATE));
    if on {
        let l = left(&sim, t, Condition::Game(consumables::JARATE));
        assert!((9.9..=10.0).contains(&l), "{l}");
    }
    on
}

#[test]
fn a_jar_soaks_an_enemy_within_230_for_10_s() {
    assert!(jar_at(225.0), "225: in");
    assert!(!jar_at(236.0), "236: out");
}

#[test]
fn a_thrown_jar_flies_switches_back_and_recharges_in_20_s() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:jarate");
    let rifle = carry(&mut sim, p, SNIPER_RIFLE);
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.0);
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().last = Some(rifle);
    press(&mut sim, p, false);
    assert_eq!(consumable(&sim, w).meter, 0.0, "thrown: the meter empty");
    let jars = |sim: &mut Sim| {
        let mut q = sim.app.world_mut().query::<&Flyer>();
        q.iter(sim.app.world()).filter(|f| f.weapon == "tf2:jarate").count()
    };
    assert_eq!(jars(&mut sim), 1, "thrown");
    sim.seconds(1.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(rifle), "the weapon before");
    sim.seconds(18.5);
    assert!(consumable(&sim, w).meter < 1.0);
    sim.seconds(1.0);
    assert!(consumable(&sim, w).meter >= 1.0);
}

#[test]
fn jarate_makes_a_rifle_body_hit_a_mini_crit_with_its_sound() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, SNIPER_RIFLE);
    let t = victim(&mut sim, from, 400.0, tf2::game::weapons::MINIGUN, 1000.0);
    give_cond(&mut sim, t, Condition::Game(consumables::JARATE), 10.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let mut crits = sim.app.world().resource::<Messages<CritHit>>().get_cursor_current();
    let mut sounds = sim.app.world().resource::<Messages<chimera_world::map::GameSound>>().get_cursor_current();
    let mut got = Vec::new();
    let mut names = Vec::new();
    run(&mut sim, p, 3, |s, k| {
        s.intent(p).fire = k == 0;
        got.extend(crits.read(s.app.world().resource::<Messages<CritHit>>()).copied());
        names.extend(sounds.read(s.app.world().resource::<Messages<chimera_world::map::GameSound>>()).map(|g| g.0.entry.to_string()));
    });
    sim.ticks(1);
    assert_eq!(1000.0 - health_points(&sim, t), 68.0, "50 x 1.35");
    assert!(got.iter().any(|c| c.mini && c.victim == t && c.attacker == p), "{got:?}");
    assert!(names.iter().any(|n| n == tf2::game::weapons::crit_hits::CRIT_HIT_MINI), "{names:?}");
}

/// Batch2b 18: each mini-crit bullet starts one `minicrit_text` over its
/// victim, replacing the victim's earlier one (tagged by him) rather than
/// piling up (a buffed Cleaner's Carbine's stream once stacked a smeared
/// heap of them).
#[test]
fn each_mini_crit_starts_one_text_that_replaces_its_victims_last() {
    use chimera_world::map::psys::StartParticleSystem;
    use tf2::game::weapons::crit_hits::MINICRIT_TEXT;
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:cleaners_carbine");
    sim.app.world_mut().entity_mut(p).insert(chimera_core::core::LocalPlayer);
    let t = victim(&mut sim, from, 200.0, tf2::game::weapons::MINIGUN, 1000.0);
    give_cond(&mut sim, p, Condition::MiniCritBuff, 10.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let mut starts = sim.app.world().resource::<Messages<StartParticleSystem>>().get_cursor_current();
    let mut texts = Vec::new();
    let mut shots = 0;
    for _ in 0..2 {
        let before = 1000.0 - health_points(&sim, t);
        run(&mut sim, p, 30, |s, k| {
            s.intent(p).fire = k == 0;
            texts.extend(starts.read(s.app.world().resource::<Messages<StartParticleSystem>>()).filter(|s| s.name == MINICRIT_TEXT).cloned());
        });
        if 1000.0 - health_points(&sim, t) > before {
            shots += 1;
        }
    }
    assert_eq!(shots, 2, "both bullets hit");
    assert_eq!(texts.len(), 2, "one text a hit: {texts:?}");
    assert!(texts.iter().all(|s| s.tag == Some(t) && s.replace), "{texts:?}");
}

#[test]
fn mad_milk_heals_the_attacker_60_per_cent_rounded_down() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, tf2::game::weapons::MINIGUN);
    let t = victim(&mut sim, from, 400.0, tf2::game::weapons::MINIGUN, 1000.0);
    give_cond(&mut sim, t, Condition::Game(consumables::MILK), 10.0);
    set_health(&mut sim, p, 100.0);
    deal(&mut sim, Some(p), t, 121.0, None);
    assert_eq!(points(&sim, p).0.round(), 172.0);
}

// ---- 14. Meters ----

#[test]
fn the_soda_poppers_damage_fills_hype_and_attack2_drains_it() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:soda_popper");
    let t = victim(&mut sim, from, 400.0, tf2::game::weapons::MINIGUN, 1000.0);
    deal(&mut sim, Some(p), t, 60.0, Some("tf2:soda_popper"));
    assert!((meter(&sim, w) - 15.4).abs() < 0.05, "{}", meter(&sim, w));
    sim.app.world_mut().get_mut::<Tf2Meter>(w).unwrap().state.value = 100.0;
    press(&mut sim, p, true);
    assert!(sim.app.world().get::<SourceTraits>(p).unwrap().air_jumps == Some(5), "hype's air jumps");
    sim.seconds(10.4);
    assert!(meter(&sim, w) > 0.0);
    sim.seconds(0.6);
    assert_eq!(meter(&sim, w), 0.0, "about 10.7 s");
}

#[test]
fn baby_faces_boost_adds_speed_and_hurt_takes_4_a_point() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], "tf2:baby_faces_blaster");
    assert!((speed(&sim, p) - 360.0).abs() < 0.01, "{}", speed(&sim, p));
    sim.app.world_mut().get_mut::<Tf2Meter>(w).unwrap().state.value = 60.0;
    sim.ticks(2);
    assert!((speed(&sim, p) - 457.2).abs() < 0.01, "{}", speed(&sim, p));
    deal(&mut sim, None, p, 10.0, None);
    assert!((meter(&sim, w) - 20.0).abs() < 0.01, "{}", meter(&sim, w));
}

#[test]
fn the_carbines_crikey_fills_by_the_damage_and_gives_8_s_of_mini_crits() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:cleaners_carbine");
    let t = victim(&mut sim, from, 400.0, tf2::game::weapons::MINIGUN, 1000.0);
    // The spec's 10.2 counts as dealt, whole (a Known shortcut).
    deal(&mut sim, Some(p), t, 10.2, Some("tf2:cleaners_carbine"));
    assert!((meter(&sim, w) - 10.0).abs() < 0.01, "{}", meter(&sim, w));
    sim.app.world_mut().get_mut::<Tf2Meter>(w).unwrap().state.value = 100.0;
    press(&mut sim, p, true);
    sim.ticks(1);
    let l = left(&sim, p, Condition::MiniCritBuff);
    assert!((7.9..=8.0).contains(&l), "{l}");
}

/// Seconds scoped until the rifle `id` (with `heads`) is fully charged.
fn time_to_full(id: &str, heads: Option<f32>) -> f64 {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], id);
    if let Some(h) = heads {
        sim.app.world_mut().get_mut::<Tf2Meter>(w).unwrap().state.value = h;
    }
    press(&mut sim, p, true);
    let start = now(&sim) - 2.0 * TICK_INTERVAL;
    for _ in 0..600 {
        sim.ticks(1);
        if sim.app.world().get::<Tf2Sniper>(w).unwrap().full() {
            return now(&sim) - start;
        }
    }
    panic!("never full");
}

#[test]
fn the_bazaar_with_3_heads_and_the_sydney_charge_in_2_4_s() {
    // 0.3 s before charging, then 150 at 62.5/s.
    let bazaar = time_to_full("tf2:bazaar_bargain", Some(3.0));
    assert!((bazaar - 2.7).abs() < 0.05, "{bazaar}");
    let sydney = time_to_full("tf2:sydney_sleeper", None);
    assert!((sydney - 2.7).abs() < 0.05, "{sydney}");
    let none = time_to_full("tf2:bazaar_bargain", Some(0.0));
    assert!((none - 6.3).abs() < 0.05, "0 heads: 25/s, {none}");
}

// ---- 15. Sniper rifle variants ----

/// `id` scoped `secs`, firing at a target `units` ahead aimed `up` m
/// above its centre; a second target `behind` units further on.
fn rifle_shot(id: &str, secs: f64, up: f32, behind: Option<f32>) -> (f32, Option<f32>, Sim, Entity) {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, id);
    let t = victim(&mut sim, from, 300.0, tf2::game::weapons::MINIGUN, 1000.0);
    let t2 = behind.map(|b| victim(&mut sim, from, 300.0 + b, tf2::game::weapons::MINIGUN, 1000.0));
    let at = sim.position(t) + Vec3::Y * up;
    testkit::aim_at(&mut sim, p, at);
    press(&mut sim, p, true);
    let n = (secs / TICK_INTERVAL).round() as u64;
    run(&mut sim, p, n, |s, _| s.intent(p).secondary = false);
    run(&mut sim, p, 3, |s, k| s.intent(p).fire = k == 0);
    sim.ticks(1);
    let a = 1000.0 - health_points(&sim, t);
    let b = t2.map(|t2| 1000.0 - health_points(&sim, t2));
    (a, b, sim, t)
}

#[test]
fn the_machinas_full_charge_passes_through_two() {
    let (a, b, ..) = rifle_shot("tf2:machina", 3.5, 0.0, Some(150.0));
    assert_eq!((a.round(), b.map(f32::round)), (173.0, Some(173.0)));
    let (a, b, ..) = rifle_shot("tf2:machina", 1.6, 0.0, Some(150.0));
    assert!(a < 100.0 && b == Some(0.0), "a part charge stops at the first: {a} {b:?}");
}

#[test]
fn the_sydney_sleepers_full_body_hit_jarates_5_s_and_its_head_mini_crits() {
    let (a, _, sim, t) = rifle_shot("tf2:sydney_sleeper", 3.5, 0.0, None);
    assert_eq!(a.round(), 150.0);
    let l = left(&sim, t, Condition::Game(consumables::JARATE));
    assert!((4.8..=5.0).contains(&l), "{l}");
    let (head, ..) = rifle_shot("tf2:sydney_sleeper", 3.5, 0.85, None);
    assert_eq!(head.round(), 203.0, "150 x 1.35");
}

#[test]
fn the_classic_charges_held_and_fires_on_release() {
    let shot = |up: f32, until: f32| {
        let mut sim = sim();
        let from = greybox::SPAWNS[0];
        let (p, w) = holding(&mut sim, from, "tf2:classic");
        let t = victim(&mut sim, from, 300.0, tf2::game::weapons::MINIGUN, 1000.0);
        let at = sim.position(t) + Vec3::Y * up;
        testkit::aim_at(&mut sim, p, at);
        for _ in 0..400 {
            run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
            if sim.app.world().get::<Tf2Sniper>(w).unwrap().state.charge >= until {
                break;
            }
        }
        assert_eq!(health_points(&sim, t), 1000.0, "no shot while held");
        run(&mut sim, p, 3, |s, _| s.intent(p).fire = false);
        1000.0 - health_points(&sim, t)
    };
    assert_eq!(shot(0.85, 150.0), 450.0, "a full charge's head crit");
    assert_eq!(shot(0.0, 51.0), 46.0, "51 x 0.9");
}

/// Input set once a frame: the Classic keeps charging while it stays down.
#[test]
fn the_classic_holds_its_charge_on_input_set_once() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:classic");
    let ev = run(&mut sim, p, 60, |s, k| match k {
        0 => s.intent(p).fire = true,
        40 => s.intent(p).fire = false,
        _ => {}
    });
    assert_eq!(fired(&ev), [40], "fired on the release only");
}

// ---- 16. Revolvers ----

#[test]
fn the_ambassadors_headshot_crits_with_the_far_falloff_after_1_s() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:ambassador");
    no_spread(&mut sim, w);
    let t = victim(&mut sim, from, 850.0, tf2::game::weapons::MINIGUN, 1000.0);
    let at = sim.position(t) + Vec3::Y * 0.85;
    testkit::aim_at(&mut sim, p, at);
    let shot = |sim: &mut Sim| {
        let before = health_points(sim, t);
        run(sim, p, 3, |s, k| s.intent(p).fire = k == 0);
        before - health_points(sim, t)
    };
    assert_eq!(shot(&mut sim), 59.0, "102 x the far half at 850");
    sim.seconds(0.8);
    let body = shot(&mut sim);
    assert!(body < 30.0, "0.85 s after: no crit, {body}");
}

#[test]
fn the_diamondbacks_backstab_kill_stores_a_crit_for_its_next_shot() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, knife) = holding(&mut sim, from, tf2::game::weapons::KNIFE);
    let gun = carry(&mut sim, p, "tf2:diamondback");
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(knife);
    sim.seconds(1.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(knife));
    no_spread(&mut sim, gun);
    let t = victim(&mut sim, from, 40.0, tf2::game::weapons::MINIGUN, 90.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let yaw = sim.intent(p).yaw;
    run(&mut sim, p, 20, |s, k| {
        s.intent(t).yaw = yaw;
        s.intent(p).fire = k == 0
    });
    assert!(health_points(&sim, t) <= 0.0, "a backstab kill");
    sim.app.world_mut().despawn(t);
    let stored = |sim: &Sim| sim.app.world().resource::<tf2::game::items::effects::Revolvers>().stored.get(&gun).copied().unwrap_or(0);
    assert_eq!(stored(&sim), 1);
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(gun);
    sim.seconds(1.0);
    let t = victim(&mut sim, from, 300.0, tf2::game::weapons::MINIGUN, 1000.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    run(&mut sim, p, 3, |s, k| s.intent(p).fire = k == 0);
    assert_eq!((1000.0 - health_points(&sim, t)).round(), 102.0);
    assert_eq!(stored(&sim), 0);
}

#[test]
fn the_utility_items_never_join_a_ladder() {
    let set = GunNumbers::builtin().with_pack(&pack()).parse().unwrap();
    for id in ["tf2:sandvich", "tf2:bonk_atomic_punch", "tf2:jarate", "tf2:mad_milk"] {
        assert!(set.get(id).unwrap().class.is_none(), "{id}");
    }
    for id in ["tf2:soda_popper", "tf2:machina", "tf2:ambassador"] {
        assert!(set.get(id).unwrap().class.is_some(), "{id}");
    }
    let _: Option<&Weapon> = None;
}

// ---- Prediction ----

/// A server and one client on TF2's movement, the client's player holding
/// `id` (given on the server): the server's and the client's player.
fn client_holding(id: &str) -> (chimera_net::harness::NetSim, Entity, Entity) {
    use chimera_net::{harness::NetSim, net::memory::LinkConditions};
    let conditions = LinkConditions { latency: std::time::Duration::from_millis(30), jitter: std::time::Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(WithItems, conditions, 5, 0, |app| {
        app.add_plugins(GreyboxMapPlugin).insert_resource(chimera_core::slots::Loadout { movement: tf2::game::movement::ID });
    });
    sim.add_client(|_| {});
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(900);
    let theirs = sim.character_of(0).unwrap();
    let w = give(sim.server.app.world_mut(), theirs, id).unwrap();
    sim.server.app.world_mut().get_mut::<Inventory>(theirs).unwrap().wanted = Some(w);
    sim.ticks(150);
    let me = sim.local_player(0).unwrap();
    (sim, theirs, me)
}

/// `n` ticks of the client running forward (`fire` on its first tick):
/// both machines' top speed.
fn steps(sim: &mut chimera_net::harness::NetSim, theirs: Entity, me: Entity, n: usize, fire: bool) -> Vec<(f32, f32)> {
    let top = |w: &World, e: Entity| w.get::<SourceTraits>(e).and_then(|t| t.max_speed).unwrap_or(0.0);
    let mut out = Vec::new();
    for k in 0..n {
        {
            let mut i = sim.clients[0].app.world_mut().get_mut::<chimera_core::core::Intent>(me).unwrap();
            i.move_axis = Vec2::Y;
            i.fire = fire && k < 2;
        }
        sim.step();
        out.push((top(sim.server.app.world(), theirs), top(sim.clients[0].app.world(), me)));
    }
    out
}

fn errors(sim: &chimera_net::harness::NetSim) -> u64 {
    sim.clients[0].app.world().resource::<chimera_net::net::predict::NetGraph>().errors
}

#[test]
fn a_clients_own_boost_and_steak_are_predicted_without_corrections() {
    let (mut sim, theirs, me) = client_holding("tf2:baby_faces_blaster");
    steps(&mut sim, theirs, me, 60, false);
    let quiet = errors(&sim);
    // The boost the server's hits gave: taken once, then 457.2 on both.
    let w = sim.server.app.world().get::<Inventory>(theirs).unwrap().active.unwrap();
    sim.server.app.world_mut().get_mut::<Tf2Meter>(w).unwrap().state.value = 60.0;
    let speeds = steps(&mut sim, theirs, me, 120, false);
    assert!(speeds.last().is_some_and(|s| (s.0 - 457.2).abs() < 0.01 && (s.1 - 457.2).abs() < 0.01), "{:?}", speeds.last());
    assert!(errors(&sim) - quiet <= 2, "the boost's start: {}", errors(&sim) - quiet);

    // The Steak eaten by the client's own press: the taunt and the speed
    // after it predicted.
    let (mut sim, theirs, me) = client_holding("tf2:buffalo_steak_sandvich");
    let steak = sim.server.app.world().get::<Inventory>(theirs).unwrap().active;
    give(sim.server.app.world_mut(), theirs, tf2::game::weapons::FISTS).unwrap();
    sim.server.app.world_mut().get_mut::<Inventory>(theirs).unwrap().wanted = steak;
    steps(&mut sim, theirs, me, 120, false);
    let quiet = errors(&sim);
    let speeds = [steps(&mut sim, theirs, me, 2, true), steps(&mut sim, theirs, me, 300, false)].concat();
    assert!(speeds.last().is_some_and(|s| (s.0 - 299.0).abs() < 0.01 && (s.1 - 299.0).abs() < 0.01), "{:?}", speeds.last());
    assert!(errors(&sim) - quiet <= 2, "eating predicted: {} corrections", errors(&sim) - quiet);
}

// ---- Batch 2's leftovers, measured (specs/tf2/batch3_mechanics.md 6) ----

#[test]
fn the_dalokohs_bonus_ends_34_1_s_after_the_eat() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:dalokohs_bar");
    run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
    run(&mut sim, p, 1, |s, _| s.intent(p).fire = false);
    sim.seconds(33.9);
    assert_eq!(points(&sim, p).1, 350.0, "still on at 34 s");
    sim.seconds(0.3);
    assert_eq!(points(&sim, p).1, 300.0, "gone at 34.1 s");
}

#[test]
fn bonk_takes_no_damage_his_own_blast_included() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:bonk_atomic_punch");
    run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
    run(&mut sim, p, 70, |s, _| s.intent(p).fire = false);
    assert!(has(&sim, p, Condition::Phased));
    let before = points(&sim, p).0;
    for (attacker, kind) in [(None, DamageKind::Blast), (Some(p), DamageKind::Blast), (None, DamageKind::Bullet)] {
        sim.app.world_mut().resource_mut::<Messages<Damage>>().write(Damage {
            target: p,
            attacker,
            amount: 0.5,
            point: Vec3::ZERO,
            dir: Vec3::NEG_Z,
            hitgroup: Hitgroup::Generic,
            kind,
            weapon: None,
            force: Vec3::ZERO,
        });
        sim.ticks(1);
        assert_eq!(points(&sim, p).0, before, "{kind:?}");
    }
}

#[test]
fn the_diamondback_stores_35_crits_at_most() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, knife) = holding(&mut sim, from, tf2::game::weapons::KNIFE);
    let gun = carry(&mut sim, p, "tf2:diamondback");
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(knife);
    sim.seconds(1.0);
    sim.app.world_mut().resource_mut::<tf2::game::items::effects::Revolvers>().stored.insert(gun, 35);
    let t = victim(&mut sim, from, 40.0, tf2::game::weapons::MINIGUN, 90.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let yaw = sim.intent(p).yaw;
    run(&mut sim, p, 20, |s, k| {
        s.intent(t).yaw = yaw;
        s.intent(p).fire = k == 0
    });
    assert!(health_points(&sim, t) <= 0.0, "a backstab kill");
    let stored = sim.app.world().resource::<tf2::game::items::effects::Revolvers>().stored.get(&gun).copied();
    assert_eq!(stored, Some(35));
}

#[test]
fn bazaar_heads_come_from_headshot_kills_only_and_count_past_6() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:bazaar_bargain");
    sim.app.world_mut().get_mut::<Tf2Meter>(w).unwrap().state.value = 6.0;
    let kill = |sim: &mut Sim, hitgroup: Hitgroup, hp: f32| {
        let t = victim(sim, from, 300.0, tf2::game::weapons::MINIGUN, hp);
        sim.app.world_mut().resource_mut::<Messages<Damage>>().write(Damage {
            target: t,
            attacker: Some(p),
            amount: 1.5,
            point: Vec3::ZERO,
            dir: Vec3::NEG_Z,
            hitgroup,
            kind: DamageKind::Bullet,
            weapon: Some("tf2:bazaar_bargain"),
            force: Vec3::ZERO,
        });
        sim.ticks(2);
        sim.app.world_mut().despawn(t);
    };
    kill(&mut sim, Hitgroup::Head, 100.0);
    assert_eq!(meter(&sim, w), 7.0, "6 -> 7");
    kill(&mut sim, Hitgroup::Chest, 100.0);
    assert_eq!(meter(&sim, w), 7.0, "a body-shot kill: none");
    kill(&mut sim, Hitgroup::Chest, 1000.0);
    assert_eq!(meter(&sim, w), 7.0, "a body hit: none");
}
