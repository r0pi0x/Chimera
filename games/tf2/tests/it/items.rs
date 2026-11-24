//! TF2's unlocks (specs/tf2/item_attributes.md, its test cases): a
//! made-up item schema written here in items_game's shape (the spec's
//! attribute values, nothing copied from the game) through the converter
//! (`items::convert`), the pack read back, the item weapons registered
//! with the stock ones (`GunNumbers::with_pack`), then played on the
//! headless harness: clips and reserves, rocket speeds, the own blast at
//! the feet, the Iron Bomber's fuse, the Quickiebomb's charge, the Sticky
//! Jumper's limit, spin-ups and the Brass Beast's spun speed, the
//! Skullcutter's speeds, the Bread Bite's drain, the Winger's jump, the
//! melee damages, the Warrior's Spirit's damage taken and the switch
//! times after a holster (4.9).

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Health, Hitgroup};
use chimera_sim::{
    testkit,
    weapon::{Inventory, Magazine, data::WeaponCatalog, give, projectile::{Flyer, FlyerGone}},
};
use chimera_world::{greybox::{self, GreyboxMapPlugin}, harness::Sim};
use source_engine::source_movement::MoveTraits;
use tf2::{
    game::{
        TICK_INTERVAL,
        items::{ItemPack, convert},
        weapons::{ROCKET_LAUNCHER, script_numbers::GunNumbers},
    },
    harness::Tf2Sim,
};

use super::weapons::{health_points, hold_still, holding, rockets as flyers, run, target};

const U: f32 = 0.0254;

/// (name, format) of each attribute the schema defines.
const DEFS: &[(&str, &str, &str)] = &[
    ("damage bonus", "mult_dmg", "value_is_percentage"),
    ("damage penalty", "mult_dmg", "value_is_percentage"),
    ("clip size bonus", "mult_clipsize", "value_is_percentage"),
    ("clip size penalty", "mult_clipsize", "value_is_inverted_percentage"),
    ("fire rate bonus", "mult_postfiredelay", "value_is_inverted_percentage"),
    ("fire rate penalty", "mult_postfiredelay", "value_is_inverted_percentage"),
    ("Projectile speed increased", "mult_projectile_speed", "value_is_percentage"),
    ("Blast radius decreased", "mult_explosion_radius", "value_is_percentage"),
    ("rocket jump damage reduction", "rocket_jump_dmg_reduction", "value_is_percentage"),
    ("no self blast dmg", "no_self_blast_dmg", "value_is_additive"),
    ("maxammo primary increased", "mult_maxammo_primary", "value_is_percentage"),
    ("maxammo secondary increased", "mult_maxammo_secondary", "value_is_percentage"),
    ("fuse bonus", "fuse_mult", "value_is_percentage"),
    ("grenade no bounce", "grenade_no_bounce", "value_is_additive"),
    ("sticky arm time bonus", "sticky_arm_time", "value_is_additive"),
    ("stickybomb charge rate", "stickybomb_charge_rate", "value_is_percentage"),
    ("stickybomb_charge_damage_increase", "stickybomb_charge_damage_increase", "value_is_percentage"),
    ("max pipebombs decreased", "add_max_pipebombs", "value_is_additive"),
    ("minigun spinup time increased", "mult_minigun_spinup_time", "value_is_percentage"),
    ("minigun spinup time decreased", "mult_minigun_spinup_time", "value_is_percentage"),
    ("aiming movespeed decreased", "mult_player_aiming_movespeed", "value_is_percentage"),
    ("spunup_damage_resistance", "spunup_damage_resistance", "value_is_percentage"),
    ("move speed penalty", "mult_player_movespeed", "value_is_percentage"),
    ("mult_player_movespeed_active", "mult_player_movespeed_active", "value_is_percentage"),
    ("provide on active", "provide_on_active", "value_is_additive"),
    ("is_a_sword", "is_a_sword", "value_is_additive"),
    ("increased jump height from weapon", "mod_jump_height_from_weapon", "value_is_percentage"),
    ("dmg taken increased", "mult_dmgtaken", "value_is_percentage"),
    ("heal on kill", "heal_on_kill", "value_is_additive"),
    ("dmg penalty vs players", "mult_dmg_vs_players", "value_is_percentage"),
    ("Set DamageType Ignite", "set_dmgtype_ignite", "value_is_additive"),
    ("mod_maxhealth_drain_rate", "mod_maxhealth_drain_rate", "value_is_additive"),
    ("heal on hit for rapidfire", "add_onhit_addhealth", "value_is_additive"),
    ("single wep holster time increased", "mult_switch_from_wep_deploy_time", "value_is_percentage"),
];

/// The made-up items: (index, name, item class, slot, class, attributes).
#[allow(clippy::type_complexity)]
const ITEMS: &[(u32, &str, &str, &str, &str, &[(&str, &str)])] = &[
    (449, "The Winger", "tf_weapon_handgun_scout_secondary", "secondary", "scout", &[
        ("damage bonus", "1.15"), ("clip size penalty", "0.4"), ("increased jump height from weapon", "1.25"),
    ]),
    (773, "Pretty Boy's Pocket Pistol", "tf_weapon_handgun_scout_secondary", "secondary", "scout", &[
        ("provide on active", "1"), ("heal on hit for rapidfire", "3"), ("fire rate bonus", "0.85"), ("clip size penalty", "0.75"),
    ]),
    (414, "The Liberty Launcher", "tf_weapon_rocketlauncher", "primary", "soldier", &[
        ("Projectile speed increased", "1.4"), ("damage penalty", "0.75"), ("clip size bonus", "1.25"),
        ("rocket jump damage reduction", "0.75"),
    ]),
    (237, "Rocket Jumper", "tf_weapon_rocketlauncher", "primary", "soldier", &[
        ("damage penalty", "0"), ("no self blast dmg", "2"), ("maxammo primary increased", "3"),
    ]),
    (127, "The Direct Hit", "tf_weapon_rocketlauncher_directhit", "primary", "soldier", &[
        ("Blast radius decreased", "0.3"), ("Projectile speed increased", "1.8"), ("damage bonus", "1.25"),
    ]),
    (308, "The Loch-n-Load", "tf_weapon_grenadelauncher", "primary", "demoman", &[("clip size penalty", "0.75")]),
    (1151, "The Iron Bomber", "tf_weapon_grenadelauncher", "primary", "demoman", &[
        ("Blast radius decreased", "0.85"), ("grenade no bounce", "1"), ("fuse bonus", "0.7"),
    ]),
    (265, "Sticky Jumper", "tf_weapon_pipebomblauncher", "secondary", "demoman", &[
        ("damage penalty", "0"), ("no self blast dmg", "1"), ("maxammo secondary increased", "3"), ("max pipebombs decreased", "-6"),
    ]),
    (1150, "The Quickiebomb Launcher", "tf_weapon_pipebomblauncher", "secondary", "demoman", &[
        ("sticky arm time bonus", "-0.2"), ("stickybomb charge rate", "0.3"), ("stickybomb_charge_damage_increase", "1.35"),
        ("clip size penalty", "0.5"), ("damage penalty", "0.85"),
    ]),
    (172, "The Scotsman's Skullcutter", "tf_weapon_sword", "melee", "demoman", &[
        ("is_a_sword", "72"), ("provide on active", "1"), ("damage bonus", "1.2"), ("move speed penalty", "0.85"),
    ]),
    (312, "The Brass Beast", "tf_weapon_minigun", "primary", "heavy", &[
        ("damage bonus", "1.2"), ("minigun spinup time increased", "1.5"), ("aiming movespeed decreased", "0.4"),
        ("spunup_damage_resistance", "0.8"),
    ]),
    (424, "Tomislav", "tf_weapon_minigun", "primary", "heavy", &[("minigun spinup time decreased", "0.8"), ("fire rate penalty", "1.2")]),
    (425, "The Family Business", "tf_weapon_shotgun", "secondary", "heavy", &[
        ("damage penalty", "0.85"), ("clip size bonus", "1.33"), ("fire rate bonus", "0.85"),
    ]),
    (310, "Warrior's Spirit", "tf_weapon_fists", "melee", "heavy", &[
        ("provide on active", "1"), ("dmg taken increased", "1.3"), ("damage bonus", "1.3"), ("heal on kill", "50"),
    ]),
    (331, "Fists of Steel", "tf_weapon_fists", "melee", "heavy", &[("single wep holster time increased", "2.0")]),
    (1100, "Bread Bite", "tf_weapon_fists", "melee", "heavy", &[
        ("mult_player_movespeed_active", "1.3"), ("mod_maxhealth_drain_rate", "10"),
    ]),
    (153, "The Homewrecker", "tf_weapon_fireaxe", "melee", "pyro", &[("dmg penalty vs players", "0.75")]),
    (326, "The Back Scratcher", "tf_weapon_fireaxe", "melee", "pyro", &[("damage bonus", "1.25")]),
    (813, "The Neon Annihilator", "tf_weapon_breakable_sign", "melee", "pyro", &[("dmg penalty vs players", "0.8")]),
    (348, "Sharpened Volcano Fragment", "tf_weapon_fireaxe", "melee", "pyro", &[("Set DamageType Ignite", "1"), ("damage penalty", "0.8")]),
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

fn pack() -> ItemPack {
    let text = convert::convert(&schema(), &|_| None).pack.to_text();
    ItemPack::parse(&text).unwrap()
}

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

fn catalog(sim: &Sim) -> WeaponCatalog {
    sim.app.world().resource::<WeaponCatalog>().clone()
}

fn hook_num(c: &WeaponCatalog, id: &str, hook: &str, key: &str) -> f64 {
    let v = &c.get(id).unwrap_or_else(|| panic!("{id}")).hooks[hook][key];
    v.as_float().or_else(|| v.as_integer().map(|i| i as f64)).unwrap()
}

#[test]
fn every_item_registers_with_its_numbers_and_names() {
    let sim = sim();
    let numbers = sim.app.world().resource::<GunNumbers>();
    assert_eq!(numbers.items.len(), ITEMS.len(), "{:?}", numbers.sources.iter().filter(|s| s.gun == "items").collect::<Vec<_>>());
    let c = catalog(&sim);
    let clip = |id: &str| c.get(id).unwrap().magazine.as_ref().unwrap().clip;
    let reserve = |id: &str| c.get(id).unwrap().ammo.as_ref().unwrap().max;
    assert_eq!(clip("tf2:winger"), 5, "12 x 0.4 = 4.8 -> 5");
    assert_eq!(clip("tf2:pretty_boys_pocket_pistol"), 9);
    assert_eq!(clip("tf2:liberty_launcher"), 5);
    assert_eq!(clip("tf2:loch_n_load"), 3);
    assert_eq!(clip("tf2:quickiebomb_launcher"), 4);
    assert_eq!(clip("tf2:family_business"), 8, "6 x 1.33 = 7.98 -> 8");
    assert_eq!(reserve("tf2:rocket_jumper"), 60);
    assert_eq!(reserve("tf2:sticky_jumper"), 72);
    assert_eq!(reserve(ROCKET_LAUNCHER), 20, "the stock reserve untouched");
    assert_eq!(c.get("tf2:winger").unwrap().hud.name.as_deref(), Some("Winger"), "\"The \" left off");
    // Spin-ups, the Brass Beast's spun speed, the Direct Hit's radius.
    assert!((hook_num(&c, "tf2:brass_beast", "tf2:minigun", "wind_up") - 1.125).abs() < 1e-6);
    assert!((hook_num(&c, "tf2:tomislav", "tf2:minigun", "wind_up") - 0.6).abs() < 1e-6);
    assert!((hook_num(&c, "tf2:brass_beast", "tf2:minigun", "spun_speed") - 44.0).abs() < 1e-4);
    assert!((hook_num(&c, "tf2:direct_hit", "tf2:rocket", "radius") - 43.8).abs() < 1e-4);
    assert_eq!(hook_num(&c, "tf2:direct_hit", "tf2:rocket", "self_radius"), 121.0, "the own radius never changes");
    assert!((hook_num(&c, "tf2:quickiebomb_launcher", "tf2:sticky", "charge_time") - 1.2).abs() < 1e-6);
    assert_eq!(hook_num(&c, "tf2:sticky_jumper", "tf2:sticky", "max"), 2.0);
    // The ladder: the jumpers deal no damage, so no ladder draws them.
    assert!(c.get("tf2:rocket_jumper").unwrap().class.is_none());
    assert!(c.get("tf2:sticky_jumper").unwrap().class.is_none());
    assert!(c.get("tf2:liberty_launcher").unwrap().class.is_some());
    // The class table knows them.
    let classes = sim.app.world().resource::<tf2::game::classes::Classes>();
    assert_eq!(classes.of_gun("tf2:brass_beast").map(|c| c.name.as_str()), Some("heavy"));
}

/// The speed of the first flyer `id` fires (units/s).
fn launch_speed(id: &str) -> f32 {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], id);
    run(&mut sim, p, 3, |s, k| s.intent(p).fire = k == 0);
    let f = flyers(&mut sim, p);
    assert_eq!(f.len(), 1, "{id}");
    f[0].2.length() / U
}

#[test]
fn rockets_fly_at_their_item_speeds() {
    assert!((launch_speed(ROCKET_LAUNCHER) - 1100.0).abs() < 1.0);
    assert!((launch_speed("tf2:liberty_launcher") - 1540.0).abs() < 1.0);
    assert!((launch_speed("tf2:direct_hit") - 1980.0).abs() < 1.0);
}

/// The damage a Soldier with `id` takes from a rocket into the floor at
/// his feet.
fn own_blast(id: &str) -> f32 {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, id);
    let feet = sim.position(p) - Vec3::Y * 0.9 - Vec3::Z * 0.3;
    testkit::aim_at(&mut sim, p, feet);
    let before = health_points(&sim, p);
    run(&mut sim, p, 40, |s, k| s.intent(p).fire = k == 0);
    (before - health_points(&sim, p)).round()
}

#[test]
fn the_own_blast_is_the_class_base_86_86_64_0() {
    let stock = own_blast(ROCKET_LAUNCHER);
    assert!(stock > 50.0, "the stock rocket at the feet: {stock}");
    assert_eq!(own_blast("tf2:direct_hit"), stock, "its damage and radius don't apply");
    assert_eq!(own_blast("tf2:liberty_launcher"), (stock * 0.75).round(), "x0.75 reduction only");
    assert_eq!(own_blast("tf2:rocket_jumper"), 0.0);
}

#[test]
fn the_rocket_jumper_still_jumps() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:rocket_jumper");
    let feet = sim.position(p) - Vec3::Y * 0.9 - Vec3::Z * 0.3;
    testkit::aim_at(&mut sim, p, feet);
    let mut up: f32 = 0.0;
    run(&mut sim, p, 30, |s, k| {
        s.intent(p).fire = k == 0;
        up = up.max(s.app.world().get::<chimera_core::core::Velocity>(p).unwrap().0.y);
    });
    assert!(up / U > 200.0, "pushed up: {}", up / U);
}

#[test]
fn the_iron_bomber_goes_off_near_1_4_s() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:iron_bomber");
    let up = sim.position(p) + Vec3::new(0.0, 3.0, -10.0);
    testkit::aim_at(&mut sim, p, up);
    let mut cursor = sim.app.world().resource::<Messages<FlyerGone>>().get_cursor_current();
    let mut gone_at = None;
    for k in 0..200u64 {
        run(&mut sim, p, 1, |s, _| s.intent(p).fire = k == 0);
        if cursor.read(sim.app.world().resource::<Messages<FlyerGone>>()).any(|g| g.owner == Some(p) && g.expired) {
            gone_at = Some(k as f64 * TICK_INTERVAL);
            break;
        }
    }
    let t = gone_at.expect("it fused");
    assert!((1.38..1.6).contains(&t), "{t}");
}

#[test]
fn the_quickiebomb_forces_its_launch_at_1_2_s() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:quickiebomb_launcher");
    let mut first = None;
    for k in 0..200u64 {
        run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
        if !flyers(&mut sim, p).is_empty() {
            first = Some(k as f64 * TICK_INTERVAL);
            break;
        }
    }
    let t = first.expect("forced out");
    assert!((1.15..1.3).contains(&t), "{t}");
}

#[test]
fn the_sticky_jumper_keeps_two() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], "tf2:sticky_jumper");
    for _ in 0..3 {
        sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 8;
        run(&mut sim, p, 60, |s, k| s.intent(p).fire = k < 2);
    }
    let live = {
        let mut q = sim.app.world_mut().query::<&Flyer>();
        q.iter(sim.app.world()).filter(|f| f.owner == Some(p)).count()
    };
    assert_eq!(live, 2);
}

fn traits(sim: &Sim, p: Entity) -> MoveTraits {
    *sim.app.world().get::<MoveTraits>(p).unwrap()
}

#[test]
fn the_brass_beast_spun_moves_at_44() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:brass_beast");
    run(&mut sim, p, 100, |s, _| s.intent(p).secondary = true);
    let speed = traits(&sim, p).max_speed.unwrap();
    assert!((speed - 44.0).abs() < 0.5, "{speed}");
}

#[test]
fn the_skullcutter_slows_only_while_out_238_280() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:scotsmans_skullcutter");
    let out = traits(&sim, p).max_speed.unwrap();
    assert!((out - 238.0).abs() < 0.01, "{out}");
    let other = give(sim.app.world_mut(), p, "tf2:tf_weapon_grenadelauncher").unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(other);
    sim.seconds(2.0);
    let held = traits(&sim, p).max_speed.unwrap();
    assert!((held - 280.0).abs() < 0.01, "{held}");
}

/// Switch `p` to a fresh `to` and the deploy it got: from the tick of the
/// switch to its next allowed attack, s.
fn deploy_to(sim: &mut Sim, p: Entity, to: &str) -> f64 {
    let other = give(sim.app.world_mut(), p, to).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(other);
    sim.ticks(1);
    let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    sim.app.world().get::<Inventory>(p).unwrap().next_attack - now
}

#[test]
fn switching_from_the_fists_of_steel_takes_1_0_and_from_a_sword_0_875() {
    let dt = TICK_INTERVAL as f64;
    // Out 1 s (`holding`): x2.0 on the next weapon's 0.5 s.
    let mut s = sim();
    let (p, fists) = holding(&mut s, greybox::SPAWNS[0], "tf2:fists_of_steel");
    let d = deploy_to(&mut s, p, "tf2:tf_weapon_minigun");
    assert!((d - 1.0).abs() <= dt + 1e-6, "{d}");
    // Out only 0.3 s: not counted.
    s.seconds(1.2);
    s.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(fists);
    s.seconds(0.3);
    let d = deploy_to(&mut s, p, "tf2:tf_weapon_shotgun_primary");
    assert!((d - 0.5).abs() <= dt + 1e-6, "{d}");
    // A sword either side: x1.75.
    let mut s = sim();
    let (p, _) = holding(&mut s, greybox::SPAWNS[0], "tf2:scotsmans_skullcutter");
    let d = deploy_to(&mut s, p, "tf2:tf_weapon_grenadelauncher");
    assert!((d - 0.875).abs() <= dt + 1e-6, "{d}");
    // The stock Fists: the plain 0.5.
    let mut s = sim();
    let (p, _) = holding(&mut s, greybox::SPAWNS[0], "tf2:tf_weapon_fists");
    let d = deploy_to(&mut s, p, "tf2:tf_weapon_minigun");
    assert!((d - 0.5).abs() <= dt + 1e-6, "{d}");
}

#[test]
fn the_winger_jumps_higher_while_out() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:winger");
    assert_eq!(traits(&sim, p).jump_scale, Some(1.25));
    run(&mut sim, p, 1, |s, _| s.intent(p).jump = true);
    let vy = sim.app.world().get::<chimera_core::core::Velocity>(p).unwrap().0.y / U;
    // 289 x 1.25 = 361.25 at the jump, less the tick's gravity as the
    // stock jump's 289 is stored 271 (movement.md test 5).
    assert!((vy - (361.25 - 18.0)).abs() < 0.5, "{vy}");
}

#[test]
fn the_bread_bite_drains_max_health_270_after_1_s() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:bread_bite");
    // `holding` waited 1 s with it out: 20 at the deploy and 10 a second.
    let max = sim.app.world().get::<Health>(p).unwrap().max * 100.0;
    assert!((max - 270.0).abs() < 1.5, "{max}");
    sim.seconds(4.0);
    let later = sim.app.world().get::<Health>(p).unwrap().max * 100.0;
    assert!((max - later - 40.0).abs() < 1.5, "{later}");
    sim.seconds(30.0);
    assert!((sim.app.world().get::<Health>(p).unwrap().max * 100.0 - 100.0).abs() < 1e-3, "floor 100");
}

/// One swing of `id` at a Soldier 50 units ahead: what it took.
fn swing(id: &str) -> f32 {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, id);
    let t = target(&mut sim, from, 50.0);
    hold_still(&mut sim, t);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let before = health_points(&sim, t);
    run(&mut sim, p, 20, |s, k| s.intent(p).fire = k == 0);
    (before - health_points(&sim, t)).round()
}

#[test]
fn melee_items_hit_for_49_81_52_52_85() {
    for (id, want) in [
        ("tf2:homewrecker", 49.0),
        ("tf2:back_scratcher", 81.0),
        ("tf2:neon_annihilator", 52.0),
        ("tf2:sharpened_volcano_fragment", 52.0),
        ("tf2:warriors_spirit", 85.0),
    ] {
        assert_eq!(swing(id), want, "{id}");
    }
}

/// `amount` points of generic damage to `p`: what it took.
fn hurt(sim: &mut Sim, p: Entity, amount: f32) -> f32 {
    let before = health_points(sim, p);
    sim.app.world_mut().resource_mut::<Messages<Damage>>().write(Damage {
        target: p,
        attacker: None,
        amount: amount / 100.0,
        point: Vec3::ZERO,
        dir: Vec3::Y,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Generic,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(2);
    (before - health_points(sim, p)).round()
}

#[test]
fn the_warriors_spirit_takes_65_out_and_50_holstered() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:warriors_spirit");
    assert_eq!(hurt(&mut sim, p, 50.0), 65.0);
    let other = give(sim.app.world_mut(), p, "tf2:tf_weapon_minigun").unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(other);
    sim.seconds(2.0);
    assert_eq!(hurt(&mut sim, p, 50.0), 50.0);
}

#[test]
fn the_volcano_fragment_sets_its_victim_alight_and_pretty_boys_heals_3() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:sharpened_volcano_fragment");
    let t = target(&mut sim, from, 50.0);
    hold_still(&mut sim, t);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    run(&mut sim, p, 20, |s, k| s.intent(p).fire = k == 0);
    assert!(sim.app.world().get::<tf2::game::weapons::flame::Burning>(t).is_some(), "alight");

    let mut sim = self::sim();
    let (p, _) = holding(&mut sim, from, "tf2:pretty_boys_pocket_pistol");
    let t = target(&mut sim, from, 300.0);
    hold_still(&mut sim, t);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 0.5;
    run(&mut sim, p, 3, |s, k| s.intent(p).fire = k == 0);
    assert_eq!((health_points(&sim, p)).round(), 53.0, "50 + 3");
}
