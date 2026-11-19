//! specs/combat_arms/gear_perks.md's test cases, numbered as there
//! (`case_<n>_...`), on `CaSim`: the vests, helmets, goggles and backpacks
//! as the speed limit, the stamina cap and the armour step they come to
//! (1 to 26), the sets and Fighting Chance (27 to 31), the perks (32 to
//! 52), the VIP, offline and bot players (53 to 56), and a loadout over
//! the network.
//!
//! The gear is chosen as a player chooses it: a `Selection` (the look: body
//! record, head piece, goggle, backpack) and a `CaLoadout` (mods, items,
//! perks) on the character; the systems build the rest. Values are world
//! units and hit points (health 1.0 = 100 HP). Tolerances: 1e-3 for one
//! tick's speed limit, 1e-3 HP for damage, a tick's drain (0.5 sp) for the
//! gauge over seconds.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Dead, Died, Health, Hitgroup, Team};
use chimera_sim::weapon::{self, Inventory, Magazine, data::WeaponDataAppExt};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        characters::records::Selection,
        gear::{
            self, CaGearData, CaGearEffects, CaGearQuirks, CaLoadout, CaOwnGear, FC_AMMO,
            records::{GearData, SetEffect},
        },
        grenades::CaGrenade,
        inventory::UNIT,
        movement::{self, CaCharacter, CaMovement, HeldMovePenalty, VipMovement, movement_data},
        perks::{self, CaPerkBox, CaPerkEffects, CaPerkRng, CaPerkState, CaPerks, CaSupplyBox},
        rules::escort::CaVip,
        weapons::{
            self as ca, ArmourHit, BULLET, BURN, CaSpread, FLASH, FireMode, Location, MELEE, POISON, armour_and_zone,
            gear_torso,
        },
    },
    harness::{CaSim, CaSimWith},
};

use super::movement::{T, close};

/// Metres per world unit.
const U: f32 = UNIT;
const M4A1: &str = combat_arms::game::weapons::RIFLE;

// ---------------------------------------------------------------------------
// Setup

/// The greybox with a Combat Arms player at the origin wearing `look` and
/// `loadout`, settled: its gear built.
fn wearer(look: Selection, loadout: CaLoadout) -> (Sim, Entity) {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_tick_interval(TICK_INTERVAL);
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(p).insert((look, loadout));
    sim.seconds(0.5);
    (sim, p)
}

fn look(model: i32) -> Selection {
    Selection { model, ..default() }
}

/// A character's Combat Arms armour: Chimera's `Armor` and the gear's
/// pieces, with its points and the pieces at hand.
struct Worn {
    armor: chimera_sim::weapon::Armor,
    pieces: chimera_sim::weapon::ArmorPieces,
    points: f32,
    gear: Vec<chimera_sim::weapon::hits::ArmorPiece>,
}

fn armour_of(sim: &Sim, p: Entity) -> Worn {
    let w = sim.app.world();
    let armor = *w.get::<chimera_sim::weapon::Armor>(p).expect("armour");
    let pieces = w
        .get::<chimera_sim::weapon::ArmorPieces>(p)
        .cloned()
        .unwrap_or_default();
    Worn {
        armor,
        points: combat_arms::game::weapons::points(&armor),
        gear: pieces.pieces.clone(),
        pieces,
    }
}

/// A hit of `d` hit points of damage type `ty` on `loc` (zone factor
/// `zone`) on the armour as built; what health loses and the armour
/// points after.
fn hit(sim: &Sim, p: Entity, d: f32, ty: i32, loc: Location, zone: f32) -> (f32, f32) {
    let mut armour = armour_of(sim, p);
    let h = ArmourHit {
        damage_type: ty,
        location: loc,
        zone,
        infected: false,
    };
    let lost = armour_and_zone(d, h, Some(&mut armour.armor), Some(&armour.pieces), true);
    (lost, combat_arms::game::weapons::points(&armour.armor))
}

fn set_armour(sim: &mut Sim, p: Entity, points: f32) {
    sim.app
        .world_mut()
        .get_mut::<chimera_sim::weapon::Armor>(p)
        .expect("armour")
        .amount = points / 100.0;
}

#[track_caller]
fn near(got: f32, want: f32, what: &str) {
    close(got, want, 1e-3, what);
}

// ---------------------------------------------------------------------------
// Speed, sprint and stamina (1-9, 29, 53)

/// A player on the movement tests' floor wearing `look` and `loadout`,
/// holding a weapon of penalty `held`, its gauge full.
fn runner(look: Selection, loadout: CaLoadout, held: f32) -> T {
    let mut t = T::standing();
    // The tests' bare figure out; the gear's in.
    t.sim
        .app
        .world_mut()
        .entity_mut(t.p)
        .remove::<(CaCharacter, gear::CaGearKey)>();
    t.insert((
        look,
        loadout,
        HeldMovePenalty {
            penalty: held,
            mod_factor: 0.0,
        },
    ));
    t.ticks(5);
    let cap = t
        .sim
        .app
        .world()
        .get::<CaCharacter>(t.p)
        .expect("character")
        .stamina_max();
    t.mv_mut().stamina = cap;
    t
}

/// The speed limit with the keys `setup` holds, steady.
fn limit(t: &mut T, setup: impl Fn(&mut T)) -> f32 {
    t.release_all();
    t.ticks(30);
    setup(t);
    let cap = t.sim.app.world().get::<CaCharacter>(t.p).unwrap().stamina_max();
    t.mv_mut().stamina = cap;
    t.steady_forward();
    t.tick();
    t.mv().speed_limit
}

/// Run, sprint, walk and crouch limits.
fn limits(t: &mut T) -> [f32; 4] {
    [
        limit(t, |_| {}),
        limit(t, |t| t.keys().sprint = true),
        limit(t, |t| t.keys().walk = true),
        limit(t, |t| t.keys().crouch = true),
    ]
}

fn assert_limits(got: [f32; 4], want: [f32; 4], what: &str) {
    for (i, name) in ["run", "sprint", "walk", "crouch"].into_iter().enumerate() {
        if want[i] > 0.0 {
            close(got[i], want[i], 2e-3, &format!("{what}: {name}"));
        }
    }
}

fn stamina_cap(t: &T) -> f32 {
    t.sim.app.world().get::<CaCharacter>(t.p).unwrap().stamina_max()
}

/// 1. Normal vest, no helmet, goggle or backpack: P 0.20, 228, 319.2, 56,
/// 40; stamina capacity 100.
#[test]
fn case_01_normal_vest() {
    let mut t = runner(look(0), CaLoadout::default(), 0.1);
    assert_limits(limits(&mut t), [228.0, 319.2, 56.0, 40.0], "normal");
    assert_eq!(stamina_cap(&t), 100.0);
}

/// 2. Light vest: P 0.10; 256.5, 359.1.
#[test]
fn case_02_light_vest() {
    let mut t = runner(look(1), CaLoadout::default(), 0.1);
    assert_limits(limits(&mut t), [256.5, 359.1, 0.0, 0.0], "light");
}

/// 3. Heavy vest: P 0.26; 210.9, 295.26, 51.8.
#[test]
fn case_03_heavy_vest() {
    let mut t = runner(look(2), CaLoadout::default(), 0.1);
    assert_limits(limits(&mut t), [210.9, 295.26, 51.8, 0.0], "heavy");
}

/// 4. light_NX vest with a knife (no weapon penalty): P -0.05; 299.25,
/// 418.95.
#[test]
fn case_04_light_nx_vest() {
    let mut t = runner(look(30), CaLoadout::default(), 0.0);
    assert_limits(limits(&mut t), [299.25, 418.95, 0.0, 0.0], "light_NX");
}

/// 5. Normal vest and a Bandana_default (Hair10): P 0.17; run 236.55.
#[test]
fn case_05_bandana() {
    let mut t = runner(Selection { head: 10, ..look(0) }, CaLoadout::default(), 0.1);
    close(limit(&mut t, |_| {}), 236.55, 2e-3, "run");
}

/// 6. Normal vest, Anti-FlashGoggles (Goggle39) and normal_backpack_slot2
/// (Backpack2): 0.10 - 0.03 + 0.03 on the vest's; run 228.
#[test]
fn case_06_goggles_and_backpack() {
    let l = Selection {
        goggle: 39,
        backpack: 2,
        ..look(0)
    };
    let mut t = runner(l, CaLoadout::default(), 0.1);
    close(limit(&mut t, |_| {}), 228.0, 2e-3, "run");
}

/// 7. Specialist01_M (22) and Black_Beret (Hair24): the capacity is
/// floor(100 x 1.4) = 140; sprinting 4.000 s from full leaves 20.
#[test]
fn case_07_stamina_bonuses() {
    let mut t = runner(Selection { head: 24, ..look(22) }, CaLoadout::default(), 0.1);
    assert_eq!(stamina_cap(&t), 140.0);
    t.release_all();
    t.ticks(30);
    t.mv_mut().stamina = 140.0;
    t.keys().sprint = true;
    t.forward(true);
    // 4.000 s of sprint (the first tick starts it).
    t.ticks(267);
    close(t.mv().stamina, 20.0, 0.6, "the gauge after 4 s of sprint");
}

/// 8. Normal vest and a Red Nose (Goggle13): capacity 120.
#[test]
fn case_08_red_nose() {
    let t = runner(Selection { goggle: 13, ..look(0) }, CaLoadout::default(), 0.1);
    assert_eq!(stamina_cap(&t), 120.0);
}

/// 9. A female Heavy (9) against a male Heavy (2): the same run, the same
/// stamina, the same health.
#[test]
fn case_09_female_equals_male() {
    let mut male = runner(look(2), CaLoadout::default(), 0.1);
    let mut female = runner(look(9), CaLoadout::default(), 0.1);
    close(limit(&mut male, |_| {}), 210.9, 2e-3, "male run");
    close(limit(&mut female, |_| {}), 210.9, 2e-3, "female run");
    assert_eq!(stamina_cap(&male), stamina_cap(&female));
    let (m, f) = (armour_of(&male.sim, male.p), armour_of(&female.sim, female.p));
    assert_eq!(m.gear, f.gear);
    assert_eq!(
        male.sim.app.world().get::<Health>(male.p).unwrap().max,
        female.sim.app.world().get::<Health>(female.p).unwrap().max
    );
}

// ---------------------------------------------------------------------------
// Armour (10-26)

/// 10. Normal vest, torso bullet 35: B 5.25, HP -29.75, armour 94.75.
#[test]
fn case_10_normal_vest_torso() {
    let (sim, p) = wearer(look(0), default());
    let (lost, ap) = hit(&sim, p, 35.0, BULLET, Location::Torso, 1.0);
    near(lost, 29.75, "HP lost");
    near(ap, 94.75, "armour");
}

/// 11. Heavy vest, torso bullet 35: B 10.5, HP -24.5, armour 89.5.
#[test]
fn case_11_heavy_vest_torso() {
    let (sim, p) = wearer(look(2), default());
    let (lost, ap) = hit(&sim, p, 35.0, BULLET, Location::Torso, 1.0);
    near(lost, 24.5, "HP lost");
    near(ap, 89.5, "armour");
}

/// 12. Light vest, torso bullet 35: B 1.75, HP -33.25, armour 98.25.
#[test]
fn case_12_light_vest_torso() {
    let (sim, p) = wearer(look(1), default());
    let (lost, ap) = hit(&sim, p, 35.0, BULLET, Location::Torso, 1.0);
    near(lost, 33.25, "HP lost");
    near(ap, 98.25, "armour");
}

/// 13. Normal vest and a Combat_Helmet (Hair2), head bullet 35 (x6): B
/// 8.75, HP -157.5 (dead), armour 91.25.
#[test]
fn case_13_combat_helmet() {
    let (sim, p) = wearer(Selection { head: 2, ..look(0) }, default());
    let (lost, ap) = hit(&sim, p, 35.0, BULLET, Location::Head, 6.0);
    near(lost, 157.5, "HP lost");
    near(ap, 91.25, "armour");
}

/// 14. Normal vest and an EOD_helmet (Hair8), head bullet 35: B 10.5, D'
/// 147, armour 89.5.
#[test]
fn case_14_eod_helmet() {
    let (sim, p) = wearer(Selection { head: 8, ..look(0) }, default());
    let (lost, ap) = hit(&sim, p, 35.0, BULLET, Location::Head, 6.0);
    near(lost, 147.0, "damage");
    near(ap, 89.5, "armour");
}

/// 15. Normal vest and a Bonehead_Helmet (Hair19), armour 50, head bullet
/// 20: B 7, D' 78, armour 43.
#[test]
fn case_15_bonehead_helmet() {
    let (mut sim, p) = wearer(Selection { head: 19, ..look(0) }, default());
    set_armour(&mut sim, p, 50.0);
    let (lost, ap) = hit(&sim, p, 20.0, BULLET, Location::Head, 6.0);
    near(lost, 78.0, "damage");
    near(ap, 43.0, "armour");
}

/// 16. Combat_Helmet, armour 0, head bullet 35: 210, armour 0.
#[test]
fn case_16_helmet_without_armour() {
    let (mut sim, p) = wearer(Selection { head: 2, ..look(0) }, default());
    set_armour(&mut sim, p, 0.0);
    let (lost, ap) = hit(&sim, p, 35.0, BULLET, Location::Head, 6.0);
    near(lost, 210.0, "damage");
    near(ap, 0.0, "armour");
}

/// 17. Normal vest and an AF-II Fire Mask (Goggle5), burn 10: P 0.85, B
/// 8.5, HP -1.5, armour 91.5.
#[test]
fn case_17_fire_mask() {
    let (sim, p) = wearer(Selection { goggle: 5, ..look(0) }, default());
    let (lost, ap) = hit(&sim, p, 10.0, BURN, Location::Torso, 1.0);
    near(lost, 1.5, "HP lost");
    near(ap, 91.5, "armour");
}

/// 18. Normal vest and a Mk1 gas mask (Goggle3), poison 10: B 2.8, HP
/// -7.2, the armour stays 100.
#[test]
fn case_18_gas_mask() {
    let (sim, p) = wearer(Selection { goggle: 3, ..look(0) }, default());
    let (lost, ap) = hit(&sim, p, 10.0, POISON, Location::Torso, 1.0);
    near(lost, 7.2, "HP lost");
    near(ap, 100.0, "armour");
}

/// 19. As 18 with armour 0: HP -10.
#[test]
fn case_19_gas_mask_without_armour() {
    let (mut sim, p) = wearer(Selection { goggle: 3, ..look(0) }, default());
    set_armour(&mut sim, p, 0.0);
    let (lost, ap) = hit(&sim, p, 10.0, POISON, Location::Torso, 1.0);
    near(lost, 10.0, "HP lost");
    near(ap, 0.0, "armour");
}

/// 20. Heavy vest, an M67 at the blast's centre (explode 60): HP -60, the
/// armour stays 100.
#[test]
fn case_20_blast_ignores_gear() {
    let (sim, p) = wearer(look(2), default());
    let (lost, ap) = hit(&sim, p, 60.0, ca::EXPLOSION, Location::Torso, 1.0);
    near(lost, 60.0, "HP lost");
    near(ap, 100.0, "armour");
}

/// 21. Combat_Helmet and a Normal vest, a knife's head hit 45 (melee): no
/// piece lists melee: 45 x 6 = 270, the armour stays 100.
#[test]
fn case_21_melee_ignores_helmet() {
    let (sim, p) = wearer(Selection { head: 2, ..look(0) }, default());
    let (lost, ap) = hit(&sim, p, 45.0, MELEE, Location::Head, 6.0);
    near(lost, 270.0, "damage");
    near(ap, 100.0, "armour");
}

/// The flash protection a wearer has (the torso flash entries summed, as
/// the grenades read them).
fn flash_protection(sim: &Sim, p: Entity) -> f32 {
    gear_torso(&armour_of(sim, p).gear, FLASH).clamp(0.0, 1.0)
}

/// 22. Anti-FlashGoggles, flash amount 25: 0, the armour unchanged.
#[test]
fn case_22_anti_flash() {
    let (sim, p) = wearer(Selection { goggle: 39, ..look(0) }, default());
    near(25.0 * (1.0 - flash_protection(&sim, p)), 0.0, "flash amount");
    near(armour_of(&sim, p).points, 100.0, "armour");
}

/// 23. Board_Goggle (0.3) and Metal_AntiFlash (0.7), flash amount 25:
/// 17.5 and 7.5.
#[test]
fn case_23_partial_flash_protection() {
    for (goggle, want) in [(2, 17.5), (44, 7.5)] {
        let (sim, p) = wearer(Selection { goggle, ..look(0) }, default());
        near(
            25.0 * (1.0 - flash_protection(&sim, p)),
            want,
            &format!("goggle {goggle}"),
        );
    }
}

/// 24. A Normal vest and Anti-FlashGoggles with armour 60 at a gear
/// rebuild while alive (a model change): the pieces are added again and
/// repair by 100, capped: 100.
#[test]
fn case_24_rebuild_repairs() {
    let (mut sim, p) = wearer(Selection { goggle: 39, ..look(0) }, default());
    set_armour(&mut sim, p, 60.0);
    sim.ticks(3);
    near(armour_of(&sim, p).points, 60.0, "left alone while nothing changes");
    sim.app
        .world_mut()
        .entity_mut(p)
        .insert(Selection { goggle: 39, ..look(1) });
    sim.ticks(5);
    near(armour_of(&sim, p).points, 100.0, "repaired and capped");
}

/// 25. Quirk 1 on, Normal vest, no goggle: the head piece 255 twice, head
/// protection 0.34: B 11.9, D' 138.6, armour 88.1.
#[test]
fn case_25_byte_ids_head() {
    let mut sim = quirk_sim();
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(p).insert(look(0));
    sim.seconds(0.5);
    let (lost, ap) = hit(&sim, p, 35.0, BULLET, Location::Head, 6.0);
    near(lost, 138.6, "damage");
    near(ap, 88.1, "armour");
}

/// 26. Quirk 1 on, no goggle, flash amount 25: P 0.60 gives 10.
#[test]
fn case_26_byte_ids_flash() {
    let mut sim = quirk_sim();
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(p).insert(look(0));
    sim.seconds(0.5);
    near(25.0 * (1.0 - flash_protection(&sim, p)), 10.0, "flash amount");
}

fn quirk_sim() -> Sim {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins(GreyboxMapPlugin);
        app.insert_resource(CaGearQuirks { byte_ids: true });
    });
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

// ---------------------------------------------------------------------------
// Set items and Fighting Chance (27-31)

/// A gear table with one set: the items `items`, its first tier at one
/// piece with `effects`; `weapon_items` map item numbers to weapons.
fn with_set(items: &[i32], effects: Vec<SetEffect>, weapon_items: &[(i32, &str)]) -> CaGearData {
    let mut data = GearData::built_in();
    data.sets = vec![gear::set(items, vec![(1, effects)])];
    for (no, w) in weapon_items {
        data.items.insert(*no, w.to_string());
    }
    CaGearData(std::sync::Arc::new(data))
}

fn effect(kind: i32, value: f32) -> SetEffect {
    SetEffect {
        kind,
        value,
        only: Vec::new(),
        locations: Vec::new(),
    }
}

fn wearer_with(data: CaGearData, look: Selection, loadout: CaLoadout) -> (Sim, Entity) {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app.insert_resource(data);
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(p).insert((look, loadout));
    sim.seconds(0.5);
    (sim, p)
}

fn items(list: &[i32]) -> CaLoadout {
    CaLoadout {
        items: list.to_vec(),
        ..default()
    }
}

/// 27. A set with torso protection +0.1 (effect 15) active, Normal vest,
/// torso bullet 35: P 0.25, B 8.75, HP -26.25, armour 91.25.
#[test]
fn case_27_set_protection() {
    let torso = SetEffect {
        locations: vec![2],
        ..effect(4, 0.1)
    };
    let (sim, p) = wearer_with(with_set(&[9001], vec![torso], &[]), look(0), items(&[9001]));
    let (lost, ap) = hit(&sim, p, 35.0, BULLET, Location::Torso, 1.0);
    near(lost, 26.25, "HP lost");
    near(ap, 91.25, "armour");
    // Not on the head.
    let (lost, _) = hit(&sim, p, 35.0, BULLET, Location::Head, 6.0);
    near(lost, 210.0, "the head");
}

/// 28. A set with extra magazines +1 for the M4A1: the maximum is 150, the
/// starting total stays 120 (Corrections 2026-10-22, 1).
#[test]
fn case_28_set_magazines() {
    let only_rifle = SetEffect {
        only: vec![9002],
        ..effect(9, 1.0)
    };
    let data = with_set(&[9002], vec![only_rifle], &[(9002, M4A1)]);
    let (mut sim, p) = wearer_with(data, look(0), items(&[9002]));
    let w = weapon::give(sim.app.world_mut(), p, M4A1).unwrap();
    sim.ticks(3);
    let m = sim.app.world().get::<Magazine>(w).unwrap().clone();
    assert_eq!(m.reserve_max + m.size, 150, "the maximum");
    // Corrections (2026-10-22) 1: the server starts at the spawned amount;
    // the magazines only raise the cap.
    assert_eq!(m.clip + m.reserve, 120, "the starting total");
    // Without the set, 120.
    let (mut sim, p) = wearer(look(0), default());
    let w = weapon::give(sim.app.world_mut(), p, M4A1).unwrap();
    sim.ticks(3);
    let m = sim.app.world().get::<Magazine>(w).unwrap().clone();
    assert_eq!(m.reserve_max + m.size, 120);
    assert_eq!(m.clip + m.reserve, 120);
}

/// 29. A set with speed +0.06, Normal vest: run 285 x (1 - 0.2 + 0.06) =
/// 245.1.
#[test]
fn case_29_set_speed() {
    let mut t = T::standing();
    t.sim
        .app
        .world_mut()
        .entity_mut(t.p)
        .remove::<(CaCharacter, gear::CaGearKey)>();
    t.sim.app.insert_resource(with_set(&[9003], vec![effect(6, 0.06)], &[]));
    t.insert((
        look(0),
        items(&[9003]),
        HeldMovePenalty {
            penalty: 0.1,
            mod_factor: 0.0,
        },
    ));
    t.ticks(5);
    close(limit(&mut t, |_| {}), 245.1, 2e-3, "run");
}

/// A Quarantine round with one human holding an infected-mercenary item:
/// the sim and the human.
fn quarantine_human(item: bool) -> (Sim, Entity) {
    use super::mode_kit::{CREED, game, join, past_warmup, unprotect_all};
    let mut sim = game("QRT", 12);
    let human = join(&mut sim, CREED);
    let _other = join(&mut sim, CREED);
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    sim.app.world_mut().entity_mut(human).insert(CaLoadout {
        infected_item: item,
        ..default()
    });
    sim.seconds(0.5);
    (sim, human)
}

/// 30. A Quarantine human with an infected-mercenary item, Normal vest:
/// ammunition maximum 150; a torso bullet of 35 to an unarmoured target
/// 36.05; run 234.84 (228 x 1.03); stamina 103.
#[test]
fn case_30_fighting_chance() {
    let (mut sim, p) = quarantine_human(true);
    assert!(sim.app.world().get::<CaGearEffects>(p).unwrap().fighting_chance);
    // Ammunition.
    let w = weapon::give(sim.app.world_mut(), p, M4A1).unwrap();
    sim.ticks(3);
    let m = sim.app.world().get::<Magazine>(w).unwrap().clone();
    assert_eq!(m.reserve_max + m.size, 150, "{FC_AMMO}x the maximum");
    // Speed and stamina.
    let c = sim.app.world().get::<CaCharacter>(p).unwrap().clone();
    near(c.stamina_max(), 103.0, "stamina");
    assert_eq!(c.speed_scale, 1.03);
    // Damage, on the data as the hit applies it.
    let data = sim.app.world().resource::<CaGearData>().0.clone();
    let fx = sim.app.world().get::<CaGearEffects>(p).unwrap().clone();
    near(fx.dealt(&data, M4A1, 35.0), 36.05, "damage");
    // Without the item, nothing.
    let (sim, p) = quarantine_human(false);
    assert!(!sim.app.world().get::<CaGearEffects>(p).unwrap().fighting_chance);
}

/// 31. As 30, a 10-damage pistol hit on no armour: 10 + max(1, 0.3) = 11.
#[test]
fn case_31_fighting_chance_minimum() {
    let (sim, p) = quarantine_human(true);
    let data = sim.app.world().resource::<CaGearData>().0.clone();
    let fx = sim.app.world().get::<CaGearEffects>(p).unwrap().clone();
    near(fx.dealt(&data, "combat_arms:beretta_m92fs", 10.0), 11.0, "damage");
    assert_eq!(gear::max_total(120, 30, true, 0.0), 150);
    assert_eq!(gear::max_total(10, 30, true, 0.0), 12, "at least one more");
}

// ---------------------------------------------------------------------------
// Perks (32-52)

/// A loadout of perk items.
fn perks_of(items: &[i32]) -> CaLoadout {
    CaLoadout {
        perks: items.to_vec(),
        ..default()
    }
}

/// The greybox with a perk player `p` at the origin holding an M4A1.
fn perk_sim(loadout: CaLoadout) -> (Sim, Entity, Entity) {
    let (mut sim, p) = wearer(look(0), loadout);
    let w = weapon::give(sim.app.world_mut(), p, M4A1).unwrap();
    sim.seconds(1.5);
    (sim, p, w)
}

/// An enemy dummy: an entity with a team and a place, to be killed.
fn dummy(sim: &mut Sim, team: u8, at: Vec3) -> Entity {
    sim.app
        .world_mut()
        .spawn((Team(team), Transform::from_translation(at)))
        .id()
}

/// `killer` kills `victim` with its M4A1.
fn kill(sim: &mut Sim, killer: Entity, victim: Entity) {
    let at = sim
        .app
        .world()
        .get::<Transform>(victim)
        .map(|t| t.translation)
        .unwrap_or_default();
    let damage = Damage {
        target: victim,
        attacker: Some(killer),
        amount: 1.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    };
    sim.app.world_mut().write_message(Died {
        entity: victim,
        attacker: Some(killer),
        damage,
    });
    sim.ticks(1);
}

fn boxes(sim: &mut Sim) -> Vec<CaPerkBox> {
    let w = sim.app.world_mut();
    w.query::<&CaPerkBox>().iter(w).copied().collect()
}

fn set_health(sim: &mut Sim, p: Entity, hp: f32) {
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = hp / 100.0;
}

fn set_ammo(sim: &mut Sim, w: Entity, total: u32) {
    let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
    m.clip = total.min(m.size);
    m.reserve = total - m.clip;
}

/// Resourceful with the roll forced to pass.
fn resourceful_kill(killer_hp: f32, ammo: u32) -> (Sim, Vec<CaPerkBox>) {
    let (mut sim, p, w) = perk_sim(perks_of(&[16001]));
    // The killer far from where its victim falls.
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = Vec3::new(0.0, 1.0, 30.0);
    sim.ticks(2);
    set_health(&mut sim, p, killer_hp);
    set_ammo(&mut sim, w, ammo);
    sim.app.world_mut().resource_mut::<CaPerkRng>().forced = vec![0];
    let victim = dummy(&mut sim, 2, Vec3::new(0.0, 1.0, 0.0));
    kill(&mut sim, p, victim);
    let b = boxes(&mut sim);
    (sim, b)
}

/// 32. Resourceful basic, the roll passes, the killer's HP 30: a medikit
/// at the victim; only the killer can take it for 10 s; gone after 20 s.
#[test]
fn case_32_resourceful_medikit() {
    let (mut sim, b) = resourceful_kill(30.0, 60);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].kind, perks::BoxKind::Medikit);
    // A teammate standing on it can't take it for the first 10 s.
    let mate = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(mate).insert(Team(0));
    sim.seconds(5.0);
    assert_eq!(boxes(&mut sim).len(), 1, "kept for the killer");
    // After the hide time the killer's team may (the killer is on team 0).
    sim.seconds(5.5);
    assert!(boxes(&mut sim).is_empty() || boxes(&mut sim).len() == 1);
    // Untouched, it is gone at 20 s.
    let (mut sim, _) = resourceful_kill(30.0, 60);
    sim.seconds(19.5);
    assert_eq!(boxes(&mut sim).len(), 1, "still there at 19.5 s");
    sim.seconds(1.0);
    assert!(boxes(&mut sim).is_empty(), "gone after 20 s");
}

/// 33. Resourceful basic, the roll passes, killer HP 80, the weapon 50 of
/// 120: 41 % <= 50: an ammo box.
#[test]
fn case_33_resourceful_ammo() {
    let (_, b) = resourceful_kill(80.0, 50);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].kind, perks::BoxKind::Ammo);
}

/// 34. As 33 with 100 of 120: 83 %: nothing.
#[test]
fn case_34_resourceful_nothing() {
    let (_, b) = resourceful_kill(80.0, 100);
    assert!(b.is_empty());
}

/// Victory Lap's kills at the given times; the speed limit, gauge and
/// sprint flag at `at` s with forward and sprint held from `hold` s.
fn victory_lap(kills: &[f64], hold: f64, at: f64) -> (f32, f32, bool) {
    let mut t = T::standing();
    t.sim
        .app
        .world_mut()
        .entity_mut(t.p)
        .remove::<(CaCharacter, gear::CaGearKey)>();
    t.insert((
        look(0),
        perks_of(&[16004]),
        HeldMovePenalty {
            penalty: 0.1,
            mod_factor: 0.0,
        },
    ));
    t.ticks(10);
    let start = t.sim.app.world().resource::<chimera_core::core::SimClock>().now;
    let me = t.p;
    let mut forward = false;
    let mut kills = kills.to_vec();
    kills.reverse();
    loop {
        let now = t.sim.app.world().resource::<chimera_core::core::SimClock>().now - start;
        if kills.last().is_some_and(|&k| now + 1e-9 >= k) {
            kills.pop();
            let v = dummy(&mut t.sim, 2, Vec3::ZERO);
            kill(&mut t.sim, me, v);
            continue;
        }
        if now >= at {
            break;
        }
        if !forward && now >= hold {
            t.forward(true);
            t.keys().sprint = true;
            forward = true;
        }
        t.tick();
    }
    t.tick();
    let m = t.mv();
    (m.speed_limit, m.stamina, m.sprint)
}

/// 35. Victory Lap, Normal vest and M4A1: enemy kills at 0, 3 and 6 s;
/// from 6 s to 14 s the running limit is 319.2, the gauge unchanged and
/// the sprint key inert.
#[test]
fn case_35_victory_lap() {
    let (limit, stamina, sprint) = victory_lap(&[0.0, 3.0, 6.0], 6.0, 8.0);
    close(limit, 319.2, 2e-3, "limit while active");
    close(stamina, 100.0, 1e-3, "gauge");
    assert!(!sprint, "the sprint flag stays off");
    // Over at 14 s: the sprint key works again, at the same speed, with
    // the gauge draining.
    let (_, stamina, sprint) = victory_lap(&[0.0, 3.0, 6.0], 6.0, 15.0);
    assert!(sprint && stamina < 100.0, "ended: sprint {sprint}, gauge {stamina}");
}

/// 36. Victory Lap, kills at 0, 6 and 9 s: the counter is 1, 1, 2: never
/// active.
#[test]
fn case_36_victory_lap_never() {
    let (limit, stamina, sprint) = victory_lap(&[0.0, 6.0, 9.0], 9.5, 11.0);
    // The sprint key held: the plain sprint, the gauge draining.
    assert!(sprint, "an ordinary sprint");
    close(limit, 319.2, 2e-3, "sprint limit");
    assert!(stamina < 100.0, "the gauge drained: {stamina}");
}

/// The throw-ready scene of grenades' tests: a floor, a player at the
/// origin facing +Z holding an M67 with no spread.
const M67_EXACT: &str = r#"
id = "test:m67_exact"
base = "combat_arms:m67"
[hooks."combat_arms:grenade"]
perturb = [0, 0]
"#;

fn throwing(loadout: CaLoadout) -> (Sim, Entity) {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app.add_weapon_data(&[("m67_exact", M67_EXACT)]).unwrap();
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.intent(p).yaw = std::f32::consts::PI;
    sim.app.world_mut().entity_mut(p).insert((look(0), loadout));
    sim.seconds(0.5);
    let w = weapon::give(sim.app.world_mut(), p, "test:m67_exact").unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.0);
    (sim, p)
}

/// The speed (u/s, along the ground) of the grenade thrown after holding
/// fire `hold` s.
fn throw_speed(loadout: CaLoadout, hold: f64) -> f32 {
    let (mut sim, p) = throwing(loadout);
    sim.intent(p).fire = true;
    let mut held = 0.0;
    for _ in 0..400 {
        sim.ticks(1);
        held += TICK_INTERVAL;
        if held >= hold {
            sim.intent(p).fire = false;
        }
        let w = sim.app.world_mut();
        if let Some(g) = w.query::<&CaGrenade>().iter(w).next() {
            return Vec2::new(g.velocity.x, g.velocity.z).length();
        }
    }
    panic!("no throw");
}

/// 37. Pitcher, an M67 (Velocity 1550): fire held 0, 400 and 1000 ms:
/// force 0.8, 1.075 and 1.35; speed 1240, 1666.25 and 2092.5 u/s.
#[test]
fn case_37_pitcher() {
    for (hold, force) in [(0.0, 0.8), (0.4, 1.075), (1.0, 1.35)] {
        let speed = throw_speed(perks_of(&[16006]), hold);
        // A tick of hold at least; the bar is quantised to ticks.
        let got = speed / 1550.0;
        assert!((got - force).abs() < 0.02, "hold {hold}: force {got}, want {force}");
    }
}

/// 38. No Pitcher: the thrower's bar is read by nobody: 1550 u/s.
#[test]
fn case_38_no_pitcher() {
    let speed = throw_speed(default(), 1.0);
    close(speed, 1550.0, 0.05, "speed");
}

/// A player holding a scoped sniper rifle zoomed in, standing, with its
/// gauge at 100.
fn steady_scene(item: i32) -> (Sim, Entity) {
    let (mut sim, p) = wearer(look(0), perks_of(&[item]));
    let w = weapon::give(sim.app.world_mut(), p, "combat_arms:l96a1").unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(2.0);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.seconds(1.0);
    (sim, p)
}

/// 39. Steady Hand basic / Expert, a sniper zoomed in, standing, gauge
/// 100: after 2.000 s the gauge is 80 / 84.
#[test]
fn case_39_steady_hand() {
    for (item, want) in [(16005, 80.0), (16021, 84.0)] {
        let (mut sim, p) = steady_scene(item);
        let zoomed = sim
            .app
            .world()
            .get::<Inventory>(p)
            .unwrap()
            .active
            .and_then(|w| sim.app.world().get::<ca::zoom::CaZoom>(w).map(|z| z.zoomed()));
        assert_eq!(zoomed, Some(true), "zoomed in");
        sim.app.world_mut().get_mut::<CaMovement>(p).unwrap().stamina = 100.0;
        sim.seconds(2.0);
        let g = sim.app.world().get::<CaMovement>(p).unwrap().stamina;
        close(g, want, 0.7, &format!("item {item}"));
        assert!(sim.app.world().get::<CaPerkEffects>(p).unwrap().steady);
    }
}

/// 40. Awareness on at gauge 100: standing 4.000 s, then forward: gauge 80
/// at the key; the perk off; the gauge then recovers 25 sp/s.
#[test]
fn case_40_awareness() {
    let mut t = T::standing();
    t.sim
        .app
        .world_mut()
        .entity_mut(t.p)
        .remove::<(CaCharacter, gear::CaGearKey)>();
    t.insert((look(0), perks_of(&[16002])));
    t.ticks(5);
    t.mv_mut().stamina = 100.0;
    let msg = perks::use_perk(t.sim.app.world_mut(), t.p, 1).expect("starts");
    assert!(msg.contains("Awareness"), "{msg}");
    t.sim.seconds(4.0);
    close(t.mv().stamina, 80.0, 0.7, "gauge after 4 s");
    assert!(t.sim.app.world().get::<CaPerkEffects>(t.p).unwrap().awareness);
    t.forward(true);
    t.ticks(2);
    assert!(
        !t.sim.app.world().get::<CaPerkEffects>(t.p).unwrap().awareness,
        "off at the key"
    );
    let at_key = t.mv().stamina;
    close(at_key, 80.0, 1.0, "gauge when the key is pressed");
    t.forward(false);
    t.sim.seconds(0.4);
    close(t.mv().stamina, at_key + 10.0, 1.5, "recovers 25 sp/s");
}

/// Teammates `distances` (u) from the origin; the buffed ones' flags.
fn war_cry(item: i32, distances: &[f32]) -> Vec<bool> {
    let (mut sim, p) = wearer(look(0), perks_of(&[item]));
    sim.app.world_mut().entity_mut(p).insert(Team(1));
    let mates: Vec<Entity> = distances
        .iter()
        .map(|d| {
            let e = sim.spawn_character(Vec3::new(0.0, 1.0, -d * U), movement::ID);
            sim.app.world_mut().entity_mut(e).insert(Team(1));
            e
        })
        .collect();
    sim.ticks(3);
    perks::use_perk(sim.app.world_mut(), p, 1).expect("starts");
    sim.ticks(2);
    mates
        .iter()
        .map(|&e| sim.app.world().get::<CaPerkEffects>(e).is_some_and(|f| f.free_stamina))
        .collect()
}

/// 41. War Cry basic used, teammates at 1200 u and 1400 u: 1200 buffed,
/// 1400 not; Expert buffs both (1625).
#[test]
fn case_41_war_cry_range() {
    assert_eq!(war_cry(16012, &[1200.0, 1400.0]), [true, false]);
    assert_eq!(war_cry(16023, &[1200.0, 1400.0]), [true, true]);
}

/// 42. A War Cry buff on, gauge 100: sprinting 5.000 s leaves 100.
#[test]
fn case_42_war_cry_stamina() {
    let mut t = T::standing();
    t.sim
        .app
        .world_mut()
        .entity_mut(t.p)
        .remove::<(CaCharacter, gear::CaGearKey)>();
    t.insert((look(0), perks_of(&[16012])));
    t.ticks(5);
    perks::use_perk(t.sim.app.world_mut(), t.p, 1).expect("starts");
    t.mv_mut().stamina = 100.0;
    t.keys().sprint = true;
    t.forward(true);
    t.sim.seconds(5.0);
    assert!(t.mv().sprint, "sprinting");
    close(t.mv().stamina, 100.0, 1e-3, "gauge");
}

/// 43. Eagle Eye on, an M4A1 with a cone of min 3 and max 100: the limits
/// are 3 and 90.
#[test]
fn case_43_eagle_eye() {
    let (mut sim, p, w) = perk_sim(perks_of(&[16013]));
    let mut full = sim.app.world().get::<CaSpread>(w).unwrap().clone();
    full.fire = 1.0;
    let plain = full.cone(FireMode::Normal);
    perks::use_perk(sim.app.world_mut(), p, 1).expect("starts");
    sim.ticks(3);
    let cut = sim.app.world().get::<CaSpread>(w).unwrap().clone();
    assert!(cut.perk_cut > 0.0, "the cut reaches the weapon");
    let mut cut = cut;
    cut.fire = 1.0;
    let g = (cut.params.dynamic_factor + cut.mods.dynamic) as f64;
    let (min, max) = (cut.params.cone.min as f64, cut.params.cone.max as f64);
    let (min, max) = (min - (min * 0.1).floor(), max - (max * 0.1).floor());
    assert_eq!(cut.cone(FireMode::Normal), (max * g).trunc() as f32);
    assert!(
        cut.cone(FireMode::Normal) < plain,
        "narrower: {} against {plain}",
        cut.cone(FireMode::Normal)
    );
    assert!(min <= max);
    let (pmin, pmax) = (cut.params.cone.min, cut.params.cone.max);
    assert_eq!((pmin, pmax), (3, 100), "the M4A1's cone");
}

/// 44. Inner Focus, HP 40, gauge 70: used: HP 100, gauge 0 and still 0
/// after 10 s; after death and respawn the perk is ready and the gauge
/// recovers.
#[test]
fn case_44_inner_focus() {
    let mut t = T::standing();
    t.sim
        .app
        .world_mut()
        .entity_mut(t.p)
        .remove::<(CaCharacter, gear::CaGearKey)>();
    t.insert((look(0), perks_of(&[16018])));
    t.ticks(5);
    set_health(&mut t.sim, t.p, 40.0);
    t.mv_mut().stamina = 70.0;
    perks::use_perk(t.sim.app.world_mut(), t.p, 1).expect("starts");
    t.ticks(2);
    near(t.sim.app.world().get::<Health>(t.p).unwrap().current, 1.0, "HP");
    close(t.mv().stamina, 0.0, 1e-3, "gauge");
    t.sim.seconds(10.0);
    close(t.mv().stamina, 0.0, 1e-3, "gauge after 10 s");
    // Used up for this life.
    set_health(&mut t.sim, t.p, 40.0);
    assert!(perks::use_perk(t.sim.app.world_mut(), t.p, 1).is_err());
    // Death, then back.
    let now = t.sim.app.world().resource::<chimera_core::core::SimClock>().now;
    t.sim.app.world_mut().entity_mut(t.p).insert(Dead { since: now });
    t.ticks(2);
    t.sim.app.world_mut().entity_mut(t.p).remove::<Dead>();
    set_health(&mut t.sim, t.p, 40.0);
    t.sim.seconds(1.0);
    assert!(t.mv().stamina > 10.0, "the gauge recovers again: {}", t.mv().stamina);
    perks::use_perk(t.sim.app.world_mut(), t.p, 1).expect("ready again");
}

/// 45. Inner Focus at HP 100: refused, nothing changes.
#[test]
fn case_45_inner_focus_full_hp() {
    let mut t = T::standing();
    t.sim
        .app
        .world_mut()
        .entity_mut(t.p)
        .remove::<(CaCharacter, gear::CaGearKey)>();
    t.insert((look(0), perks_of(&[16018])));
    t.ticks(5);
    t.mv_mut().stamina = 70.0;
    let why = perks::use_perk(t.sim.app.world_mut(), t.p, 1).unwrap_err();
    assert!(why.contains("full HP"), "{why}");
    close(t.mv().stamina, 70.0, 1e-3, "gauge");
}

/// 46. Spoils of War: 11 pickups in a match: 100 GP; the 11th does
/// nothing and the perk is disabled.
#[test]
fn case_46_spoils_of_war() {
    let (mut sim, p, w) = perk_sim(perks_of(&[16008]));
    let mut started = 0;
    for _ in 0..11 {
        let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
        sim.app.world_mut().spawn((
            Transform::from_translation(Vec3::new(0.0, 1.0, -50.0 * U)),
            combat_arms::game::inventory::Dropped {
                weapon: w,
                since: now,
                total: 0,
                clip: 0,
            },
        ));
        if perks::use_perk(sim.app.world_mut(), p, 1).is_ok() {
            started += 1;
            sim.seconds(2.0);
        }
    }
    assert_eq!(started, 10, "the 11th does nothing");
    let state = sim.app.world().get::<CaPerkState>(p).unwrap();
    assert_eq!((state.spoils, state.gp), (10, 100));
    assert!(perks::use_perk(sim.app.world_mut(), p, 1).is_err(), "disabled");
}

/// 48's counterpart here: the loadout carries the fast-respawn allowance
/// the death camera's `CaFastRespawn` takes, validated.
#[test]
fn fast_respawn_counts_are_checked() {
    let data = GearData::built_in();
    let l = CaLoadout {
        fast_left: 5,
        fast_max: 3,
        premium: true,
        ..default()
    };
    let v = gear::validate(&data, &l);
    assert_eq!((v.fast_left, v.fast_max, v.premium), (3, 3, true));
    let v = gear::validate(
        &data,
        &CaLoadout {
            fast_max: 1_000_000,
            fast_left: 7,
            ..default()
        },
    );
    assert_eq!(v.fast_max, gear::FAST_CAP);
}

/// 47. Relentless: the respawn wait the death camera takes is 5.6 s, in
/// the modes it works in only.
#[test]
fn relentless_modes_and_levels() {
    let data = GearData::built_in();
    let working = |mode: &str| perks::working(&data, &perks_of(&[16010]), mode, true, true);
    let list = working("TDM");
    let perks = CaPerks { list };
    assert_eq!(perks::respawn_wait(&data, &perks), Some(5.6));
    assert!(working("TSV").is_empty(), "Elimination Pro: no respawn to shorten");
    assert!(working("TMM").is_empty());
    for mode in perks::RELENTLESS_MODES {
        assert_eq!(working(mode).len(), 1, "{mode}");
    }
    // Off with the room option, and on an infected body.
    assert!(perks::working(&data, &perks_of(&[16010]), "TDM", false, true).is_empty());
    assert!(perks::working(&data, &perks_of(&[16010]), "TDM", true, false).is_empty());
}

/// 50. Warlord: the player holds an M4A1 with 40 of 120, touches the box,
/// stands still 3.0 s: the ammunition 120, the box's charges 9.
#[test]
fn case_50_warlord_refill() {
    let (mut sim, p, w) = warlord_scene();
    let b = supply_point(&mut sim);
    // Stand on it.
    stand_on(&mut sim, p, b);
    sim.ticks(2);
    let charges = sim.app.world().get::<CaSupplyBox>(b).unwrap().charges;
    assert_eq!(charges, 9, "a charge is reserved");
    sim.seconds(3.2);
    let m = sim.app.world().get::<Magazine>(w).unwrap();
    assert_eq!(m.clip + m.reserve, 120, "refilled");
    assert_eq!(sim.app.world().get::<CaSupplyBox>(b).unwrap().charges, 9);
}

/// 51. As 50, the player moves at 2.0 s: no refill, the charges back to 10.
#[test]
fn case_51_warlord_cancelled() {
    let (mut sim, p, w) = warlord_scene();
    let b = supply_point(&mut sim);
    stand_on(&mut sim, p, b);
    sim.seconds(2.0);
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation += Vec3::X * 0.5;
    sim.seconds(1.5);
    let m = sim.app.world().get::<Magazine>(w).unwrap();
    assert_eq!(m.clip + m.reserve, 40, "no refill");
    assert!(sim.app.world().get::<CaSupplyBox>(b).unwrap().charges >= 9);
}

/// Put the player on the supply point, standing still.
fn stand_on(sim: &mut Sim, p: Entity, b: Entity) {
    let at = sim.app.world().get::<Transform>(b).unwrap().translation;
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = at + Vec3::Y * 0.91;
    sim.app
        .world_mut()
        .get_mut::<chimera_core::core::Velocity>(p)
        .unwrap()
        .0 = Vec3::ZERO;
    // Charging needs the use key held (Corrections 2026-10-22, 5).
    sim.intent(p).use_key = true;
}

/// A Warlord player with an M4A1 at 40 of 120.
fn warlord_scene() -> (Sim, Entity, Entity) {
    let (mut sim, p, w) = perk_sim(perks_of(&[16011]));
    set_ammo(&mut sim, w, 40);
    (sim, p, w)
}

/// Start Warlord and wait for the box (2.5 s).
fn supply_point(sim: &mut Sim) -> Entity {
    let p = {
        let w = sim.app.world_mut();
        w.query_filtered::<Entity, With<CaPerks>>().iter(w).next().unwrap()
    };
    perks::use_perk(sim.app.world_mut(), p, 1).expect("starts");
    sim.seconds(2.7);
    let w = sim.app.world_mut();
    w.query_filtered::<Entity, With<CaSupplyBox>>()
        .iter(w)
        .next()
        .expect("a supply point")
}

/// 52. Three perk slots: War Cry (2) and Resourceful (1) fit, Relentless
/// (3) is refused; with the extender (4 slots) the first two still leave 1
/// and Relentless is still refused.
#[test]
fn case_52_perk_slots() {
    let data = GearData::built_in();
    assert!(gear::perks_fit(&data, &[16012, 16001], 3));
    assert!(!gear::perks_fit(&data, &[16012, 16001, 16010], 3));
    assert!(!gear::perks_fit(&data, &[16012, 16001, 16010], 4));
    let v = gear::validate(&data, &perks_of(&[16012, 16001, 16010]));
    assert_eq!(v.perks, [16012, 16001]);
    let v = gear::validate(
        &data,
        &CaLoadout {
            extender: true,
            ..perks_of(&[16012, 16001, 16010])
        },
    );
    assert_eq!(v.perks, [16012, 16001]);
    // Unknown items drop; the same ability twice is one entry, the Expert
    // item winning.
    let v = gear::validate(&data, &perks_of(&[99999, 16001, 16019]));
    assert_eq!(v.perks, [16019]);
}

// ---------------------------------------------------------------------------
// The VIP, the player without a lobby, bots (53-56)

/// 53. The VIP of VIP Escort wearing a Heavy vest, a Combat_Helmet and a
/// Red Nose, holding an M4A1: P 0.16 (the vest's alone), run 239.4, sprint
/// 285 x 1.4 x 0.84 = 335.16, capacity 100, perks disabled.
#[test]
fn case_53_vip() {
    let mut t = T::standing();
    t.sim
        .app
        .world_mut()
        .entity_mut(t.p)
        .remove::<(CaCharacter, gear::CaGearKey)>();
    t.insert((
        Selection {
            head: 2,
            goggle: 13,
            ..look(2)
        },
        perks_of(&[16010, 16001]),
        HeldMovePenalty {
            penalty: 0.1,
            mod_factor: 0.0,
        },
        CaVip,
        VipMovement { penalty: 0.16 },
    ));
    t.ticks(5);
    assert_limits(limits(&mut t), [239.4, 335.16, 0.0, 0.0], "vip");
    // The gauge's cap is the figure's alone: a full gauge of 200 settles
    // at 100.
    t.release_all();
    t.mv_mut().stamina = 200.0;
    t.ticks(5);
    close(t.mv().stamina, 100.0, 1e-3, "the VIP's capacity");
    assert!(
        t.sim.app.world().get::<CaPerks>(t.p).unwrap().list.is_empty(),
        "perks disabled"
    );
}

/// 54. A gear item that expires during the match keeps working until the
/// player leaves: the lobby's expiry is between matches, so nothing in
/// the match takes gear back.
#[test]
fn case_54_gear_lasts() {
    let (mut sim, p) = wearer(
        Selection {
            head: 2,
            goggle: 39,
            ..look(2)
        },
        perks_of(&[16001]),
    );
    let before = (
        armour_of(&sim, p).gear,
        sim.app.world().get::<CaPerks>(p).unwrap().clone(),
    );
    sim.seconds(20.0);
    let after = (
        armour_of(&sim, p).gear,
        sim.app.world().get::<CaPerks>(p).unwrap().clone(),
    );
    assert_eq!(before, after);
    assert_eq!(after.1.list.len(), 1);
}

/// 55. Offline (no lobby), the default selection: no perks, no fast
/// respawn; the Normal vest's numbers (cases 1 and 10).
#[test]
fn case_55_offline() {
    let (sim, p) = wearer(Selection::default(), default());
    assert!(sim.app.world().get::<CaPerks>(p).unwrap().list.is_empty());
    assert_eq!(
        *sim.app.world().get::<CaLoadout>(p).unwrap_or(&CaLoadout::default()),
        CaLoadout::default()
    );
    let (lost, ap) = hit(&sim, p, 35.0, BULLET, Location::Torso, 1.0);
    near(lost, 29.75, "HP lost");
    near(ap, 94.75, "armour");
    // The local player's own settings are empty: no perks, no fast respawns.
    let own = CaOwnGear::default().loadout();
    assert_eq!(own, CaLoadout::default());
    assert_eq!(own.fast_left, 0);
    // The reference character is the default selection's.
    let c = sim.app.world().get::<CaCharacter>(p).unwrap();
    let reference = &movement_data().character;
    assert_eq!(c.stamina_max(), reference.stamina_max());
    let sum = |c: &CaCharacter, f: fn(&movement::Gear) -> f32| c.gear.iter().map(f).sum::<f32>();
    close(
        sum(c, |g| g.move_penalty),
        sum(reference, |g| g.move_penalty),
        1e-6,
        "penalty",
    );
    close(
        sum(c, |g| g.dash_speed),
        sum(reference, |g| g.dash_speed),
        1e-6,
        "sprint bonus",
    );
}

/// 56. A bot with the template model Heavy (2): its gear penalty is 0.16
/// (bots.md's p_gear) and it runs at 285 x 0.84 without a weapon.
#[test]
fn case_56_bot_gear() {
    let mut t = T::standing();
    t.sim
        .app
        .world_mut()
        .entity_mut(t.p)
        .remove::<(CaCharacter, gear::CaGearKey)>();
    t.insert((chimera_core::core::Bot, look(2), HeldMovePenalty::default()));
    t.ticks(5);
    let c = t.sim.app.world().get::<CaCharacter>(t.p).unwrap();
    close(c.gear.iter().map(|g| g.move_penalty).sum(), 0.16, 1e-6, "p_gear");
    close(limit(&mut t, |_| {}), 285.0 * 0.84, 2e-3, "run");
    assert!(
        t.sim.app.world().get::<CaPerks>(t.p).unwrap().list.is_empty(),
        "bots have no perks"
    );
}

// ---------------------------------------------------------------------------
// The loadout reaches the server

/// Over the network: a client's console settings reach the server
/// validated, its player wears the gear (armour, perks, the character's
/// numbers) and every client sees it; a perk used by request changes
/// play on both sides.
#[test]
fn net_loadout_reaches_the_server() {
    use chimera_net::{
        harness::NetSim,
        net::{NetCharacter, memory::LinkConditions},
    };
    use combat_arms::game::characters::CaOwnLook;
    let link = LinkConditions {
        latency: std::time::Duration::from_millis(30),
        jitter: std::time::Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 11, 2, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    // Client 0: a Heavy vest, a Combat_Helmet, goggles, Resourceful, War
    // Cry, and (too many) Relentless; client 1: nothing.
    sim.clients[0].app.world_mut().insert_resource(CaOwnLook(Selection {
        head: 2,
        goggle: 39,
        ..look(2)
    }));
    sim.clients[0].app.world_mut().insert_resource(CaOwnGear {
        perks: "16012 16001 16010".into(),
        vest_mods: "10 11 99".into(),
        fast_respawns: 5,
        fast_respawns_max: 3,
        ..default()
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(120);
    let a = sim.character_of(0).expect("client 0's player");
    let b = sim.character_of(1).expect("client 1's player");
    // The server checked the loadout.
    let server = sim.server.app.world();
    let l = server.get::<CaLoadout>(a).expect("the server's copy").clone();
    assert_eq!(l.perks, [16012, 16001], "Relentless doesn't fit");
    assert_eq!(l.vest_mods, [10, 11], "the unknown mod is dropped");
    assert_eq!((l.fast_left, l.fast_max), (3, 3));
    assert!(server.get::<CaLoadout>(b).is_none_or(|l| *l == CaLoadout::default()));
    // The server's player wears the gear.
    let armour = server.get::<chimera_sim::weapon::ArmorPieces>(a).unwrap();
    let plain = server.get::<chimera_sim::weapon::ArmorPieces>(b).unwrap();
    assert_ne!(armour, plain, "Heavy, helmet and goggles against the Normal vest");
    assert_eq!(server.get::<CaPerks>(a).unwrap().list.len(), 2);
    // The speed limit the server and the predicting client compute.
    let server_char = server.get::<CaCharacter>(a).unwrap().clone();
    let mine = sim.local_player(0).unwrap();
    let client_char = sim.clients[0].app.world().get::<CaCharacter>(mine).unwrap().clone();
    assert_eq!(server_char.gear.len(), client_char.gear.len());
    close(
        server_char.gear.iter().map(|g| g.move_penalty).sum(),
        client_char.gear.iter().map(|g| g.move_penalty).sum(),
        1e-6,
        "the client's penalty is the server's",
    );
    // Everyone sees the loadout.
    let other = sim.clients[1].app.world_mut();
    let mut q = other.query::<(&NetCharacter, &CaLoadout)>();
    assert!(q.iter(other).any(|(_, l)| l.perks == [16012, 16001]));
    // The client asks for War Cry (position 1): the server starts it and
    // the effect reaches the client.
    sim.clients[0]
        .app
        .world_mut()
        .write_message(perks::CaPerkRequest { slot: 1 });
    sim.ticks(40);
    assert!(
        sim.server.app.world().get::<CaPerkEffects>(a).unwrap().free_stamina,
        "the server's"
    );
    let me = sim.local_player(0).unwrap();
    assert!(
        sim.clients[0]
            .app
            .world()
            .get::<CaPerkEffects>(me)
            .unwrap()
            .free_stamina,
        "the client's"
    );
    // And armour: the Heavy vest's piece reached the client.
    let seen = sim.clients[0]
        .app
        .world()
        .get::<chimera_sim::weapon::ArmorPieces>(me)
        .unwrap();
    assert_eq!(
        seen,
        sim.server
            .app
            .world()
            .get::<chimera_sim::weapon::ArmorPieces>(a)
            .unwrap()
    );
}

// ---------------------------------------------------------------------------
// Gear in play and in the data

/// The flash time (s) an XM84 going off 200 u ahead gives a wearer of
/// `goggle` (None: not flashed).
fn flash_time(goggle: i32) -> Option<f32> {
    let (mut sim, victim) = wearer(Selection { goggle, ..look(0) }, default());
    let thrower = sim.spawn_character(Vec3::new(0.0, 1.0, 60.0), movement::ID);
    sim.app.world_mut().entity_mut(thrower).insert(Team(1));
    sim.seconds(0.5);
    let w = weapon::spawn_weapon(
        sim.app.world_mut(),
        combat_arms::game::grenades::XM84,
        Entity::PLACEHOLDER,
    )
    .unwrap();
    let at = Vec3::new(0.0, 1.0, -200.0 * U);
    let g = combat_arms::game::grenades::spawn_grenade(sim.app.world_mut(), w, Some(thrower), at, Vec3::ZERO).unwrap();
    sim.app.world_mut().get_mut::<CaGrenade>(g).unwrap().params.fuse = 0.0;
    sim.seconds(1.5);
    sim.app
        .world()
        .get::<combat_arms::game::grenades::CaFlashed>(victim)
        .map(|f| f.time)
}

/// 22 and 23 in play: a real flash grenade against the goggles' pieces.
#[test]
fn flash_grenades_meet_the_goggles() {
    close(flash_time(-1).expect("bare eyes"), 25.0, 1e-3, "no goggles");
    assert_eq!(flash_time(39), None, "Anti-FlashGoggles");
    close(flash_time(2).expect("flashed"), 17.5, 1e-3, "Board_Goggle");
    close(flash_time(44).expect("flashed"), 7.5, 1e-3, "Metal_AntiFlash");
}

/// The M4A1 with no roll, spread or kick: every round goes where it is
/// aimed and deals 35.
const EXACT: &str = r#"
id = "test:m4a1_exact"
base = "combat_arms:m4a1"
[hooks."combat_arms:damage"]
roll = 0.0
[hooks."combat_arms:spread"]
cone = { min = 0, max = 0, inc = 1.3, dec = 0.3 }
[hooks."combat_arms:recoil"]
pitch = 0.0
"#;

/// A rifle round at `part` of a player wearing `look`, 100 u away: the
/// hit points it cost and the armour points left.
fn shot_at(look: Selection, part: &str) -> (f32, f32) {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app.add_weapon_data(&[("exact", EXACT)]).unwrap();
    super::weapon_corrections::steady_surfaces(&mut sim);
    let shooter = sim.spawn_character(Vec3::new(30.0, 1.0, 37.0), movement::ID);
    let victim = sim.spawn_character(Vec3::new(30.0, 1.0, 37.0 - 100.0 * U), movement::ID);
    sim.intent(victim).yaw = std::f32::consts::PI;
    sim.app.world_mut().entity_mut(victim).insert(look);
    sim.app.world_mut().entity_mut(victim).insert(Team(1));
    sim.app.world_mut().entity_mut(shooter).insert(Team(2));
    sim.seconds(0.5);
    // Plenty of health, to see the damage.
    sim.app.world_mut().entity_mut(victim).insert(Health {
        current: 10.0,
        max: 10.0,
    });
    weapon::give(sim.app.world_mut(), shooter, "test:m4a1_exact").unwrap();
    sim.seconds(1.5);
    let at = super::aim_points::point(sim.app.world(), shooter, victim, part);
    let eye = sim.position(shooter) + sim.state(shooter).eye_offset;
    let d = at - eye;
    {
        let mut i = sim.intent(shooter);
        i.yaw = (-d.x).atan2(-d.z);
        i.pitch = d.y.atan2(d.xz().length());
        i.fire = true;
    }
    sim.ticks(1);
    sim.intent(shooter).fire = false;
    sim.ticks(5);
    let lost = 1000.0 - sim.app.world().get::<Health>(victim).unwrap().current * 100.0;
    (lost, armour_of(&sim, victim).points)
}

/// 11 and 13 in play: a rifle round meets the Heavy vest (torso) and a
/// Combat_Helmet (head) as the armour step has it.
#[test]
fn shots_meet_the_gear() {
    let (lost, ap) = shot_at(look(2), "Torso");
    near(lost, 24.5, "Heavy vest, torso");
    near(ap, 89.5, "armour");
    let (lost, ap) = shot_at(Selection { head: 2, ..look(0) }, "Head");
    near(lost, 157.5, "Combat_Helmet, head");
    near(ap, 91.25, "armour");
}

/// The install's records replace ours by id, and the perk and set records
/// map as the spec names them.
#[test]
fn the_installs_records_replace_ours() {
    use combat_arms::game::gear::records::GearInstall;
    use lithtech_formats::attributes::Document;
    let doc = |t: &str| Document::parse(t.as_bytes()).unwrap();
    let weapons = doc(
        r#"[Gear0] Name="X" Armor=50 ProtectionType0=2 ProtectionType1=3 NodeHeadProtection0=0 NodeTorsoProtection0=0.5
        NodeArmProtection0=0.5 NodelegProtection0=0.5 Consumable0=0
        NodeHeadProtection1=1.5 NodeTorsoProtection1=0.25 NodeArmProtection1=0 NodelegProtection1=0 Consumable1=1"#,
    );
    let models = doc(
        r#"[Model0] Name="N" GearId=0 MaxArmor=80 MaxHitPoints=120 MovePenalty=0.2 DashSpeed=0.5 MaxStamina=0.1
        [Hair0] Name="H" GearId=-1 MovePenalty=0.01"#,
    );
    let attachments =
        doc(r#"[Goggle0] Name="G" GearId=0 MovePenalty=-0.1 Maxstamina=0.2 [Backpack0] Name="B" MovePenalty=0.05"#);
    let mods = doc(r#"[Mod0] Name="a" Kinds=0 [Mod1] Name="m" Kinds=7 GearID=0 MovePenalty=-0.01"#);
    let perks = doc(
        r#"[PerkSkill_0] Type=10 ItemNo_0=16010 RespawnTime=4 [PerkSkill_1] Type=3 ItemNo_0=16003 ItemNo_1=16020 FootSoundVolume_0=0.5 FootSoundVolume_1=0.25 DashSoundVolume=0.9"#,
    );
    let sets = doc(r#"[Effect0] EffectType=4 [Effect1] EffectType=6
        [SetItem0] ItemNo0=5 ItemNo1=6 ItemCount0=1 ItemCount0_Effect0_ID=0 ItemCount0_Effect0_Value=0.2
        ItemCount1=2 ItemCount1_Effect0_ID=1 ItemCount1_Effect0_Value=0.05"#);
    let install = GearInstall::read(
        Some(&weapons),
        Some(&models),
        Some(&attachments),
        Some(&mods),
        Some(&perks),
        Some(&sets),
    );
    let data = GearData::built_in().with_install(&install);
    // The piece: fractions clamped, the first entry of a type, consumable.
    let piece = &data.pieces[&0];
    assert_eq!(piece.armor, 50.0);
    let entries = piece.gear.protects;
    assert_eq!(entries.len(), 2);
    assert!(!entries[0].consumable && entries[1].consumable);
    assert_eq!(entries[1].head, 1.0, "clamped to 1");
    // The vest, the head piece, the goggle and the backpack.
    let v = &data.vests[&0];
    assert_eq!(
        (v.max_armor, v.max_hp, v.move_penalty, v.dash_speed, v.max_stamina),
        (80.0, 120.0, 0.2, 0.5, 0.1)
    );
    assert_eq!(data.heads[&0].move_penalty, 0.01);
    assert_eq!(data.heads[&0].gear, -1, "a missing GearId is no piece");
    assert_eq!(
        (
            data.goggles[&0].gear,
            data.goggles[&0].move_penalty,
            data.goggles[&0].max_stamina
        ),
        (0, -0.1, 0.2)
    );
    assert_eq!(data.backpacks[&0].move_penalty, 0.05);
    // The gear mod (Kinds 7: the vest's), and the others skipped.
    assert_eq!(data.mods[&1].gear, 0);
    assert_eq!(data.mods[&1].move_penalty, -0.01);
    // Perks: the unsuffixed value is both levels', the suffixed ones the
    // level's; ours stay where the install has nothing.
    let relentless = data.perks.iter().find(|p| p.ty == 10).unwrap();
    assert_eq!(
        (relentless.value("RespawnTime", 0), relentless.value("RespawnTime", 1)),
        (4.0, 4.0)
    );
    let specter = data.perks.iter().find(|p| p.ty == 3).unwrap();
    assert_eq!(
        (specter.value("FootSoundVolume", 0), specter.value("FootSoundVolume", 1)),
        (0.5, 0.25)
    );
    assert_eq!(
        (specter.value("DashSoundVolume", 0), specter.value("DashSoundVolume", 1)),
        (0.9, 0.9)
    );
    // Sets: the tiers and their effects.
    assert_eq!(data.sets.len(), 1);
    assert_eq!(data.sets[0].tier.iter().map(|t| t.count).collect::<Vec<_>>(), [1, 2]);
    let bonus = gear::resolve_sets(&data, &[5]);
    assert_eq!(bonus.protection, [(0.2, vec![])]);
    let bonus = gear::resolve_sets(&data, &[5, 6]);
    assert_eq!(
        (bonus.speed, bonus.protection.len()),
        (0.05, 0),
        "the highest tier alone"
    );
}

/// Parting Gift (14): the owner's death leaves a live grenade at the
/// body, credited to the dead, going straight up, with the perk's area
/// damage (20 basic, 24 Expert).
#[test]
fn parting_gift_leaves_a_grenade() {
    for (item, area) in [(16007, 20.0), (16022, 24.0)] {
        let (mut sim, p) = wearer(look(0), perks_of(&[item]));
        weapon::give(sim.app.world_mut(), p, combat_arms::game::grenades::M67).unwrap();
        sim.seconds(1.5);
        let killer = dummy(&mut sim, 2, Vec3::new(0.0, 1.0, -30.0));
        kill(&mut sim, killer, p);
        sim.ticks(2);
        let world = sim.app.world_mut();
        let grenades: Vec<CaGrenade> = world.query::<&CaGrenade>().iter(world).cloned().collect();
        assert_eq!(grenades.len(), 1, "item {item}");
        assert_eq!(grenades[0].thrower, Some(p));
        assert_eq!(grenades[0].params.area_damage, area);
        assert!(grenades[0].velocity.xz().length() < 1e-3, "straight up");
    }
    // Without the perk, nothing.
    let (mut sim, p) = wearer(look(0), default());
    weapon::give(sim.app.world_mut(), p, combat_arms::game::grenades::M67).unwrap();
    sim.seconds(1.5);
    let killer = dummy(&mut sim, 2, Vec3::new(0.0, 1.0, -30.0));
    kill(&mut sim, killer, p);
    sim.ticks(2);
    let world = sim.app.world_mut();
    assert_eq!(world.query::<&CaGrenade>().iter(world).count(), 0);
}

/// A respawn builds the gear again (9): the pool at the vest's `MaxArmor`
/// and the pieces of the look and mods worn, whatever the armour was.
#[test]
fn a_respawn_refills_the_pool_and_the_pieces() {
    let (mut sim, p) = wearer(
        Selection {
            head: 2,
            goggle: 3,
            ..look(2)
        },
        CaLoadout {
            vest_mods: vec![10],
            ..default()
        },
    );
    let worn = armour_of(&sim, p).gear;
    assert_eq!(worn.len(), 4, "vest, helmet, goggle, mod");
    sim.app
        .world_mut()
        .entity_mut(p)
        .insert(combat_arms::game::weapons::gear_armour(12.0, &Vec::new()));
    combat_arms::game::rules::spawns::respawn(sim.app.world_mut(), p);
    sim.ticks(2);
    let a = armour_of(&sim, p);
    assert_eq!(a.points, 100.0);
    assert_eq!(a.gear, worn);
}

/// The look and the loadout choose the gear as the console sets them: the
/// local player's `ca_head`, `ca_perks` and the rest reach its armour,
/// perks and loadout at once.
#[test]
fn the_consoles_settings_are_the_local_players_gear() {
    use combat_arms::game::characters::CaOwnLook;
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_tick_interval(TICK_INTERVAL);
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app
        .world_mut()
        .entity_mut(p)
        .insert(chimera_core::core::LocalPlayer);
    sim.seconds(0.5);
    let before = armour_of(&sim, p);
    sim.app
        .world_mut()
        .insert_resource(CaOwnLook(Selection { head: 8, ..look(2) }));
    sim.app.world_mut().insert_resource(CaOwnGear {
        perks: "16012 16001".into(),
        fast_respawns: 2,
        fast_respawns_max: 2,
        ..default()
    });
    sim.seconds(0.5);
    let after = armour_of(&sim, p);
    assert_ne!(before.gear, after.gear, "Heavy and the EOD helmet");
    assert_eq!(after.gear.len(), 2);
    let l = sim.app.world().get::<CaLoadout>(p).unwrap();
    assert_eq!((l.perks.clone(), l.fast_left), (vec![16012, 16001], 2));
    assert_eq!(sim.app.world().get::<CaPerks>(p).unwrap().list.len(), 2);
}

// ---------------------------------------------------------------------------
// Relentless and fast respawns reach the death camera (47-49)

mod respawn {
    use bevy::prelude::*;
    use chimera_core::core::{Dead, LocalPlayer};
    use chimera_world::harness::Sim;
    use combat_arms::game::{
        death_cam::{CaDeathInput, CaFastRespawn, CaKillCamRoom, CaRelentless, CaSlot, machine::Input},
        gear::CaOwnGear,
        rules::spawns,
    };

    use crate::mode_kit::*;

    /// The local player `me` (Alpha) with `own` as its console loadout in
    /// `mode`, live, and an enemy `b`.
    fn rig(mode: &str, own: CaOwnGear) -> (Sim, Entity, Entity) {
        let mut sim = game(mode, 12);
        let me = join(&mut sim, JSF);
        let c = join(&mut sim, JSF);
        let b = join(&mut sim, CREED);
        let d = join(&mut sim, JSF);
        let w = sim.app.world_mut();
        for (e, slot, name) in [(me, 2, "Me"), (c, 3, "Charlie"), (b, 4, "Bravo"), (d, 5, "Delta")] {
            w.entity_mut(e).insert((CaSlot(slot), Name::new(name)));
        }
        w.entity_mut(me).insert(LocalPlayer);
        w.insert_resource(CaKillCamRoom(-1));
        w.insert_resource(own);
        until_live(&mut sim);
        if m(&sim).warmup {
            past_warmup(&mut sim);
        }
        unprotect_all(&mut sim);
        sim.ticks(5);
        (sim, me, b)
    }

    fn since(sim: &Sim, me: Entity) -> f64 {
        now(sim) - sim.app.world().get::<Dead>(me).expect("dead").since
    }

    /// 47. Relentless, Elimination: die, wait: the request goes at 5.6 s
    /// and is accepted (the server's limit is 5.0 s).
    #[test]
    fn case_47_relentless() {
        let own = CaOwnGear {
            perks: "16010".into(),
            ..Default::default()
        };
        let (mut sim, me, b) = rig("TDM", own);
        assert_eq!(sim.app.world().get::<CaRelentless>(me).map(|r| r.0), Some(5.6));
        kill_now(&mut sim, me, b);
        let died = sim.app.world().get::<Dead>(me).unwrap().since;
        let mut at = None;
        for _ in 0..800 {
            sim.ticks(1);
            if !dead(&sim, me) {
                at = Some(now(&sim) - died);
                break;
            }
        }
        let t = at.expect("respawned");
        assert!(t > 5.6 && t < 5.7, "asked at {t}");
        // Without the perk, 7 s.
        let (mut sim, me, b) = rig("TDM", CaOwnGear::default());
        assert!(sim.app.world().get::<CaRelentless>(me).is_none());
        kill_now(&mut sim, me, b);
        sim.seconds(6.0);
        assert!(dead(&sim, me), "still waiting at 6 s");
    }

    /// 48. Elimination, 2 fast respawns left, no pass: F7 at 1 s respawns
    /// at once; the counter is 1.
    #[test]
    fn case_48_fast_respawn() {
        let own = CaOwnGear {
            fast_respawns: 2,
            fast_respawns_max: 3,
            ..Default::default()
        };
        let (mut sim, me, b) = rig("TDM", own);
        let f = sim
            .app
            .world()
            .get::<CaFastRespawn>(me)
            .copied()
            .expect("the allowance");
        assert_eq!((f.left, f.max, f.premium), (2, 3, false));
        kill_now(&mut sim, me, b);
        sim.seconds(1.0);
        assert!(dead(&sim, me));
        sim.app.world_mut().insert_resource(CaDeathInput(Input {
            fast: true,
            ..Default::default()
        }));
        sim.ticks(1);
        sim.app.world_mut().insert_resource(CaDeathInput::default());
        sim.ticks(3);
        assert!(!dead(&sim, me), "respawned at once");
        assert_eq!(sim.app.world().get::<CaFastRespawn>(me).unwrap().left, 1);
    }

    /// 49. Elimination, no fast respawns left, no pass: F7 does nothing; a
    /// forged fast request is refused and the player stays dead.
    #[test]
    fn case_49_no_fast_respawn_left() {
        let (mut sim, me, b) = rig("TDM", CaOwnGear::default());
        kill_now(&mut sim, me, b);
        sim.seconds(1.0);
        sim.app.world_mut().insert_resource(CaDeathInput(Input {
            fast: true,
            ..Default::default()
        }));
        sim.ticks(1);
        sim.app.world_mut().insert_resource(CaDeathInput::default());
        sim.ticks(3);
        assert!(dead(&sim, me), "F7 does nothing");
        let refused = spawns::request_respawn_with(sim.app.world_mut(), me, true);
        assert!(refused.is_err(), "{refused:?}");
        assert!(dead(&sim, me), "stays dead");
        assert!(since(&sim, me) < 7.0);
    }

    /// The pass is unlimited: fast respawns spend nothing.
    #[test]
    fn the_pass_spends_nothing() {
        let own = CaOwnGear {
            fast_pass: 1,
            ..Default::default()
        };
        let (mut sim, me, b) = rig("TDM", own);
        kill_now(&mut sim, me, b);
        sim.seconds(0.5);
        assert!(spawns::request_respawn_with(sim.app.world_mut(), me, true).is_ok());
        assert!(!dead(&sim, me));
        assert!(sim.app.world().get::<CaFastRespawn>(me).unwrap().premium);
    }
}

/// Opportunist (14): the nearest dead body within the distance gives up
/// one of the primaries it carried (after the wait; a body is looted
/// once, an empty one fails).
#[test]
fn opportunist_loots_a_body() {
    let (mut sim, p, _w) = perk_sim(perks_of(&[16009]));
    // A dead enemy two metres away carrying two primaries.
    let body = sim.spawn_character(Vec3::new(0.0, 1.0, -2.0), movement::ID);
    sim.app.world_mut().entity_mut(body).insert(Team(2));
    sim.seconds(0.5);
    for id in ["combat_arms:mp5a4", "combat_arms:l96a1"] {
        weapon::give(sim.app.world_mut(), body, id).unwrap();
    }
    sim.seconds(0.5);
    let drops = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        w.query::<&combat_arms::game::inventory::Dropped>().iter(w).count()
    };
    let before = drops(&mut sim);
    // Alive bodies aren't looted.
    assert!(
        perks::use_perk(sim.app.world_mut(), p, 1).is_err(),
        "nobody dead in reach"
    );
    let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    sim.app.world_mut().entity_mut(body).insert(Dead { since: now });
    perks::use_perk(sim.app.world_mut(), p, 1).expect("starts");
    sim.seconds(1.0);
    assert_eq!(drops(&mut sim), before, "not before the wait (1.5 s)");
    sim.seconds(1.0);
    assert_eq!(drops(&mut sim), before + 1, "DropWeaponMax 1");
    // Looted once.
    assert!(perks::use_perk(sim.app.world_mut(), p, 1).is_err(), "the body is empty");
}

/// Quirk 1 needs more than 255 pieces in the data (the US data's 258):
/// with fewer, byte-wide ids change nothing.
#[test]
fn quirk_one_needs_more_than_255_pieces() {
    let quirks = CaGearQuirks { byte_ids: true };
    let effects = CaGearEffects::default();
    let ours = GearData::built_in();
    assert!(ours.piece_count > 255);
    let wrapped = gear::resolve(
        &ours,
        &movement_data().character.figure,
        quirks,
        &look(0),
        &CaLoadout::default(),
        &effects,
        false,
    );
    let plain = gear::resolve(
        &ours,
        &movement_data().character.figure,
        CaGearQuirks::default(),
        &look(0),
        &CaLoadout::default(),
        &effects,
        false,
    );
    assert_eq!(
        wrapped.ids,
        [0, 255, 255],
        "the vest's piece and the missing face and goggle as 255"
    );
    assert_eq!(plain.ids, [0]);
    // An install with 200 pieces: no wrap.
    let mut small = ours.clone();
    small.piece_count = 200;
    let r = gear::resolve(
        &small,
        &movement_data().character.figure,
        quirks,
        &look(0),
        &CaLoadout::default(),
        &effects,
        false,
    );
    assert_eq!(r.ids, [0]);
}

// ---------------------------------------------------------------------------
// Perks limited to a weapon class, on the install's records

/// Install data whose weapons file holds one record per `(name, Guntype)`
/// (all with one ammo type): data an install would differ from ours by.
fn install_with(records: &[(&str, i32)]) -> combat_arms::game::data::CaData {
    use combat_arms::game::data::{CaData, InstallAttributes, attributes::WeaponsFile};
    let mut text = String::from("[Ammo0] Name=\"5.56mm\"\n");
    for (i, (name, ty)) in records.iter().enumerate() {
        // The weapons file's record names the sight the weapon is fitted
        // with (`DefaultScopeAttach`, attachments.md "Defaults in the
        // weapons file"): a record without it has none, so the L96A1's
        // carries the "Sniper Scope" as the install's does (the sight's own
        // record is ours).
        let scope = if name.eq_ignore_ascii_case("L96A1") {
            " DefaultScopeAttach=\"Sniper Scope\""
        } else {
            ""
        };
        text += &format!("[Weapon{i}] Name=\"{name}\" AmmoName0=\"5.56mm\" Guntype={ty}{scope}\n");
    }
    let doc = lithtech_formats::attributes::Document::parse(text.as_bytes()).unwrap();
    let weapons = WeaponsFile::read(&doc).unwrap();
    CaData::with_attributes(InstallAttributes {
        weapons: Some(weapons),
        ..default()
    })
}

/// 57. A weapon's class is the install's `Guntype` when it has a record
/// (looked up by the namespaced id), else ours: a bug looked the record up
/// by the short id and always fell back to our table.
#[test]
fn case_57_gun_type_reads_the_install_record() {
    let install = install_with(&[("M4A1", 6), ("L96A1", 4), ("Test Rifle", 3)]);
    assert_eq!(perks::gun_type(&install, "combat_arms:m4a1"), Some(6));
    assert_eq!(perks::gun_type(&install, "combat_arms:l96a1"), Some(4));
    assert_eq!(
        perks::gun_type(&install, "combat_arms:test_rifle"),
        Some(3),
        "a weapon only the install has"
    );
    assert_eq!(perks::gun_type(&install, "combat_arms:m67"), Some(7), "no record: ours");
    assert_eq!(perks::gun_type(&install, "combat_arms:nothing"), None);
    let ours = combat_arms::game::data::CaData::builtin();
    assert_eq!(perks::gun_type(&ours, "combat_arms:m4a1"), Some(4));
    assert_eq!(perks::gun_type(&ours, "combat_arms:l96a1"), Some(perks::SNIPER));
    assert_eq!(perks::gun_type(&ours, "combat_arms:m67"), Some(perks::THROWN));
}

/// The greybox on `data` with a perk player at the origin holding `weapon`.
fn class_sim(data: combat_arms::game::data::CaData, item: i32, weapon: &'static str) -> (Sim, Entity, Entity) {
    let mut sim = Sim::with(CaSimWith(data), |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_tick_interval(TICK_INTERVAL);
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(p).insert((look(0), perks_of(&[item])));
    sim.seconds(0.5);
    let w = weapon::give(sim.app.world_mut(), p, weapon).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(2.0);
    (sim, p, w)
}

/// Whether Steady Hand is on with `weapon` zoomed in on `data`.
fn steady_on(data: combat_arms::game::data::CaData, weapon: &'static str) -> bool {
    let (mut sim, p, _) = class_sim(data, 16005, weapon);
    let w = sim
        .app
        .world()
        .get::<Inventory>(p)
        .unwrap()
        .active
        .expect("a weapon in hand");
    // The scope comes from the record's default sight, not set by hand.
    let scope = sim.app.world().get::<ca::zoom::CaZoom>(w).and_then(|z| z.scope.clone());
    assert_eq!(scope.map(|s| s.fov), Some(vec![20.0]), "the L96A1's Sniper Scope");
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.seconds(1.0);
    assert_eq!(
        sim.app.world().get::<ca::zoom::CaZoom>(w).map(|z| z.zoomed()),
        Some(true),
        "zoomed in (builtin only: {})",
        sim.app
            .world()
            .resource::<combat_arms::game::data::CaData>()
            .builtin_only()
    );
    sim.app.world().get::<CaPerkEffects>(p).unwrap().steady
}

/// 58. Steady Hand follows the install's class: a record that makes the
/// L96A1 a rifle switches it off, and a sniper record keeps it on.
#[test]
fn case_58_steady_hand_follows_the_install_class() {
    assert!(
        steady_on(combat_arms::game::data::CaData::builtin(), "combat_arms:l96a1"),
        "ours"
    );
    assert!(steady_on(
        install_with(&[("L96A1", perks::SNIPER)]),
        "combat_arms:l96a1"
    ));
    assert!(
        !steady_on(install_with(&[("L96A1", 4)]), "combat_arms:l96a1"),
        "a rifle in the install's data"
    );
}

/// Whether a kill with the M4A1 on `data` (the roll forced to pass, the
/// killer's HP 30) drops a box.
fn resourceful_drops(data: combat_arms::game::data::CaData) -> bool {
    let (mut sim, p, _) = class_sim(data, 16001, M4A1);
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = Vec3::new(0.0, 1.0, 30.0);
    sim.ticks(2);
    set_health(&mut sim, p, 30.0);
    sim.app.world_mut().resource_mut::<CaPerkRng>().forced = vec![0];
    let victim = dummy(&mut sim, 2, Vec3::new(0.0, 1.0, 0.0));
    kill(&mut sim, p, victim);
    !boxes(&mut sim).is_empty()
}

/// 59. Resourceful drops for the classes it lists and not for a special
/// weapon, by the install's record.
#[test]
fn case_59_resourceful_follows_the_install_class() {
    assert!(
        resourceful_drops(combat_arms::game::data::CaData::builtin()),
        "ours: a rifle"
    );
    assert!(resourceful_drops(install_with(&[("M4A1", 6)])), "a sniper");
    assert!(resourceful_drops(install_with(&[("M4A1", 7)])), "a thrown weapon");
    assert!(!resourceful_drops(install_with(&[("M4A1", 8)])), "a special weapon");
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-22): 1-10

/// The perk player (the one with perks) of `sim`.
fn perk_player(sim: &mut Sim) -> Entity {
    let w = sim.app.world_mut();
    w.query_filtered::<Entity, With<CaPerks>>().iter(w).next().unwrap()
}

/// Put `p` at `at`, standing still.
fn place(sim: &mut Sim, p: Entity, at: Vec3) {
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = at;
    sim.app
        .world_mut()
        .get_mut::<chimera_core::core::Velocity>(p)
        .unwrap()
        .0 = Vec3::ZERO;
}

fn total(sim: &Sim, w: Entity) -> u32 {
    let m = sim.app.world().get::<Magazine>(w).unwrap();
    m.clip + m.reserve
}

/// 2. Resourceful's medikit heals to full and repairs the armour to full;
/// at full health and armour it cannot be taken and the box stays.
#[test]
fn c2_medikit_heals_and_repairs_to_full() {
    let (mut sim, b) = resourceful_kill(30.0, 60);
    assert_eq!(b[0].kind, perks::BoxKind::Medikit);
    let p = perk_player(&mut sim);
    sim.app
        .world_mut()
        .get_mut::<chimera_sim::weapon::Armor>(p)
        .unwrap()
        .amount = 20.0 / 100.0;
    place(&mut sim, p, Vec3::new(0.0, 1.0, 0.0));
    sim.ticks(3);
    near(sim.app.world().get::<Health>(p).unwrap().current, 1.0, "health");
    near(armour_of(&sim, p).points, 100.0, "armour");
    assert!(boxes(&mut sim).is_empty(), "used up");
    // Full health and armour: the box stays.
    let (mut sim, _) = resourceful_kill(30.0, 60);
    let p = perk_player(&mut sim);
    set_health(&mut sim, p, 100.0);
    place(&mut sim, p, Vec3::new(0.0, 1.0, 0.0));
    sim.ticks(3);
    assert_eq!(boxes(&mut sim).len(), 1, "nothing to take, the box stays");
}

/// 3. Resourceful's ammo box refills the last rifle or SMG held, not the
/// weapon in hand; a last primary that is a machine gun cannot take it
/// (the box stays for a teammate with a rifle).
#[test]
fn c3_ammo_box_refills_the_last_rifle() {
    let (mut sim, p, rifle) = perk_sim(perks_of(&[16001]));
    set_ammo(&mut sim, rifle, 10);
    let pistol = weapon::give(sim.app.world_mut(), p, "combat_arms:beretta_m92fs").unwrap();
    sim.seconds(1.5);
    set_ammo(&mut sim, pistol, 5);
    set_health(&mut sim, p, 80.0);
    place(&mut sim, p, Vec3::new(0.0, 1.0, 30.0));
    sim.app.world_mut().resource_mut::<CaPerkRng>().forced = vec![0];
    let victim = dummy(&mut sim, 2, Vec3::new(0.0, 1.0, 0.0));
    kill(&mut sim, p, victim);
    assert_eq!(boxes(&mut sim)[0].kind, perks::BoxKind::Ammo);
    place(&mut sim, p, Vec3::new(0.0, 1.0, 0.0));
    sim.ticks(3);
    assert_eq!(total(&sim, rifle), 120, "the last rifle is refilled");
    assert_eq!(total(&sim, pistol), 5, "the pistol in hand is not");
    assert!(boxes(&mut sim).is_empty());

    // A machine gun as the last primary: the box stays.
    let (mut sim, p, _rifle) = perk_sim(perks_of(&[16001]));
    let mg = weapon::give(sim.app.world_mut(), p, "combat_arms:m60").unwrap();
    sim.seconds(1.5);
    let pistol = weapon::give(sim.app.world_mut(), p, "combat_arms:beretta_m92fs").unwrap();
    sim.seconds(1.5);
    set_ammo(&mut sim, pistol, 5);
    set_ammo(&mut sim, mg, 10);
    set_health(&mut sim, p, 80.0);
    place(&mut sim, p, Vec3::new(0.0, 1.0, 30.0));
    sim.app.world_mut().resource_mut::<CaPerkRng>().forced = vec![0];
    let victim = dummy(&mut sim, 2, Vec3::new(0.0, 1.0, 0.0));
    kill(&mut sim, p, victim);
    assert_eq!(boxes(&mut sim).len(), 1);
    place(&mut sim, p, Vec3::new(0.0, 1.0, 0.0));
    sim.ticks(3);
    assert_eq!(boxes(&mut sim).len(), 1, "an M60 cannot take it");
    assert_eq!(total(&sim, mg), 10);
    // A teammate with a rifle can, after the hide time.
    let mate = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    let mate_rifle = weapon::give(sim.app.world_mut(), mate, M4A1).unwrap();
    sim.seconds(1.5);
    set_ammo(&mut sim, mate_rifle, 5);
    sim.seconds(10.0);
    assert_eq!(total(&sim, mate_rifle), 120);
    assert!(boxes(&mut sim).is_empty());
}

/// 4. The full-ammo box refills every weapon, thrown ones included (a
/// spent grenade comes back and is selectable again).
#[test]
fn c4_full_ammo_box_refills_grenades_too() {
    let (mut sim, p, rifle) = perk_sim(perks_of(&[16001]));
    let g = weapon::give(sim.app.world_mut(), p, combat_arms::game::grenades::M67).unwrap();
    sim.seconds(1.5);
    sim.app
        .world_mut()
        .get_mut::<chimera_sim::weapon::grenade::Throwable>(g)
        .unwrap()
        .count = 0;
    sim.seconds(0.5);
    set_ammo(&mut sim, rifle, 30);
    set_health(&mut sim, p, 80.0);
    place(&mut sim, p, Vec3::new(0.0, 1.0, 30.0));
    sim.app.world_mut().resource_mut::<CaPerkRng>().forced = vec![0];
    let victim = dummy(&mut sim, 2, Vec3::new(0.0, 1.0, 0.0));
    kill(&mut sim, p, victim);
    assert_eq!(boxes(&mut sim)[0].kind, perks::BoxKind::FullAmmo);
    place(&mut sim, p, Vec3::new(0.0, 1.0, 0.0));
    sim.ticks(3);
    let t = sim
        .app
        .world()
        .get::<chimera_sim::weapon::grenade::Throwable>(g)
        .unwrap();
    assert_eq!(t.count, t.max, "the grenade is back");
    assert!(t.count > 0);
    assert_eq!(total(&sim, rifle), 120);
    assert!(boxes(&mut sim).is_empty());
}

/// 5. Warlord: an enemy on the box gets nothing; the owner without use
/// gets nothing; holding use for 3.0 s refills (charges 9); releasing use
/// at 2.0 s cancels (charges back to 10); a switch to another rifle keeps
/// the charge and that rifle is refilled.
#[test]
fn c5_warlord_use_team_and_gun_type() {
    // An enemy holding use.
    let (mut sim, p, w) = warlord_scene();
    let b = supply_point(&mut sim);
    let at = sim.app.world().get::<Transform>(b).unwrap().translation;
    let enemy = sim.spawn_character(at + Vec3::Y * 0.91, movement::ID);
    sim.app.world_mut().entity_mut(enemy).insert(Team(2));
    let e_rifle = weapon::give(sim.app.world_mut(), enemy, M4A1).unwrap();
    sim.seconds(1.0);
    set_ammo(&mut sim, e_rifle, 40);
    sim.intent(enemy).use_key = true;
    sim.seconds(3.5);
    assert_eq!(total(&sim, e_rifle), 40, "an enemy gets nothing");
    assert_eq!(sim.app.world().get::<CaSupplyBox>(b).unwrap().charges, 10);
    sim.app.world_mut().despawn(enemy);
    // The owner without use.
    stand_on(&mut sim, p, b);
    sim.intent(p).use_key = false;
    sim.seconds(3.5);
    assert_eq!(
        sim.app.world().get::<CaSupplyBox>(b).unwrap().charges,
        10,
        "no key, no charge"
    );
    // Holding use.
    sim.intent(p).use_key = true;
    sim.ticks(2);
    assert_eq!(sim.app.world().get::<CaSupplyBox>(b).unwrap().charges, 9);
    sim.seconds(3.2);
    assert_eq!(total(&sim, w), 120);

    // Released at 2.0 s: cancelled.
    let (mut sim, p, w) = warlord_scene();
    let b = supply_point(&mut sim);
    stand_on(&mut sim, p, b);
    sim.seconds(2.0);
    sim.intent(p).use_key = false;
    sim.seconds(1.5);
    assert_eq!(total(&sim, w), 40, "no refill");
    assert_eq!(
        sim.app.world().get::<CaSupplyBox>(b).unwrap().charges,
        10,
        "the charge is back"
    );

    // Another rifle at 2.0 s still gets the refill.
    let (mut sim, p, w) = warlord_scene();
    let second = weapon::give(sim.app.world_mut(), p, M4A1).unwrap();
    sim.seconds(1.5);
    set_ammo(&mut sim, second, 40);
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.0);
    let b = supply_point(&mut sim);
    stand_on(&mut sim, p, b);
    sim.seconds(2.0);
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(second);
    sim.seconds(1.6);
    assert_eq!(total(&sim, second), 120, "the rifle in hand at the end");
    assert_eq!(sim.app.world().get::<CaSupplyBox>(b).unwrap().charges, 9);
}

/// 6. Warlord's box is removed as soon as the round is decided.
#[test]
fn c6_warlord_box_goes_with_the_round() {
    let (mut sim, _p, _w) = warlord_scene();
    let b = supply_point(&mut sim);
    assert!(sim.app.world().get_entity(b).is_ok());
    sim.app
        .world_mut()
        .resource_mut::<combat_arms::game::rules::CaMatch>()
        .phase = combat_arms::game::rules::Phase::RoundOver { until: f64::MAX };
    sim.ticks(3);
    assert!(sim.app.world().get_entity(b).is_err(), "gone in the end-of-round phase");
}

/// 7. Opportunist: the weapons lie within 20 u of the torso, reserved for
/// the looter (10 s) then the looter's team; the body is marked looted; an
/// empty body fails and stays.
#[test]
fn c7_opportunist_reserves_and_removes() {
    let (mut sim, p, _w) = perk_sim(perks_of(&[16009]));
    sim.app.world_mut().entity_mut(p).insert(Team(1));
    let body = sim.spawn_character(Vec3::new(0.0, 1.0, -2.0), movement::ID);
    sim.app.world_mut().entity_mut(body).insert(Team(2));
    sim.seconds(0.5);
    weapon::give(sim.app.world_mut(), body, "combat_arms:mp5a4").unwrap();
    weapon::give(sim.app.world_mut(), body, "combat_arms:l96a1").unwrap();
    sim.seconds(0.5);
    let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    sim.app.world_mut().entity_mut(body).insert(Dead { since: now });
    let torso = sim.app.world().get::<Transform>(body).unwrap().translation;
    perks::use_perk(sim.app.world_mut(), p, 1).expect("starts");
    sim.seconds(2.0);
    let w = sim.app.world_mut();
    let found: Vec<(Entity, Vec3, combat_arms::game::inventory::CaReserved)> = w
        .query::<(Entity, &Transform, &combat_arms::game::inventory::CaReserved)>()
        .iter(w)
        .map(|(e, t, r)| (e, t.translation, *r))
        .collect();
    assert_eq!(found.len(), 1);
    let (item, at, reserved) = found[0];
    assert!(
        (at.x - torso.x).abs() <= 20.0 * U + 1e-4 && (at.z - torso.z).abs() <= 20.0 * U + 1e-4,
        "{at} vs {torso}"
    );
    assert_eq!((reserved.owner, reserved.team), (p, Some(1)));
    assert!(
        sim.app.world().get::<perks::CaLooted>(body).is_some(),
        "the body is looted"
    );
    // Another team's player cannot take it, a teammate only after 10 s.
    let enemy = sim.spawn_character(Vec3::new(0.0, 1.0, 5.0), movement::ID);
    sim.app.world_mut().entity_mut(enemy).insert(Team(2));
    let mate = sim.spawn_character(Vec3::new(1.0, 1.0, 5.0), movement::ID);
    sim.app.world_mut().entity_mut(mate).insert(Team(1));
    for who in [enemy, mate] {
        weapon::give(sim.app.world_mut(), who, M4A1).unwrap();
    }
    sim.seconds(1.5);
    let w = sim.app.world_mut();
    assert!(!combat_arms::game::inventory::pick_up(w, enemy, item), "not an enemy");
    assert!(
        !combat_arms::game::inventory::pick_up(w, mate, item),
        "not a teammate yet"
    );
    sim.seconds(10.0);
    let w = sim.app.world_mut();
    assert!(!combat_arms::game::inventory::pick_up(w, enemy, item), "never an enemy");
    assert!(
        combat_arms::game::inventory::pick_up(w, mate, item),
        "a teammate after 10 s"
    );

    // An empty body fails and stays.
    let (mut sim, p, _w) = perk_sim(perks_of(&[16009]));
    let body = sim.spawn_character(Vec3::new(0.0, 1.0, -2.0), movement::ID);
    sim.seconds(0.5);
    sim.app.world_mut().get_mut::<Inventory>(body).unwrap().weapons.clear();
    let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    sim.app.world_mut().entity_mut(body).insert(Dead { since: now });
    perks::use_perk(sim.app.world_mut(), p, 1).expect("starts");
    sim.seconds(2.0);
    assert!(sim.app.world().get::<perks::CaLooted>(body).is_none(), "the body stays");
    assert!(
        sim.app.world().get::<CaPerkState>(p).unwrap().cooldown.is_empty(),
        "a failed perk cools nothing"
    );
}

/// 8. Parting Gift is always the perk's own frag on a 2.5 s fuse, whatever
/// the dead carried (here a smoke grenade, and nothing at all).
#[test]
fn c8_parting_gift_is_the_perks_own_frag() {
    for carried in [Some("combat_arms:m18"), None] {
        let (mut sim, p) = wearer(look(0), perks_of(&[16007]));
        if let Some(id) = carried {
            weapon::give(sim.app.world_mut(), p, id).unwrap();
        }
        sim.seconds(1.5);
        let killer = dummy(&mut sim, 2, Vec3::new(0.0, 1.0, -30.0));
        kill(&mut sim, killer, p);
        sim.ticks(2);
        let world = sim.app.world_mut();
        let grenades: Vec<CaGrenade> = world.query::<&CaGrenade>().iter(world).cloned().collect();
        assert_eq!(grenades.len(), 1, "{carried:?}");
        assert_eq!(
            grenades[0].weapon,
            combat_arms::game::grenades::M67,
            "the frag, not {carried:?}"
        );
        assert_eq!(grenades[0].params.fuse, 2.5);
        assert_eq!(grenades[0].params.area_damage, 20.0);
    }
}

/// 9. War Cry reaches a dead teammate in range too.
#[test]
fn c9_war_cry_reaches_the_dead() {
    let (mut sim, p) = wearer(look(0), perks_of(&[16012]));
    sim.app.world_mut().entity_mut(p).insert(Team(1));
    let mate = sim.spawn_character(Vec3::new(0.0, 1.0, -500.0 * U), movement::ID);
    sim.app.world_mut().entity_mut(mate).insert(Team(1));
    sim.ticks(3);
    let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    sim.app.world_mut().entity_mut(mate).insert(Dead { since: now });
    sim.ticks(2);
    perks::use_perk(sim.app.world_mut(), p, 1).expect("starts");
    sim.ticks(2);
    let buff = sim.app.world().get::<CaPerkState>(mate).and_then(|s| s.war_cry);
    assert!(buff.is_some(), "the dead teammate is on the buff list");
}

/// 10. The multi-kill window is strictly under 5 s: kills at 0, 5.000 and
/// 6 s count 1, 1, 2.
#[test]
fn c10_multi_kill_window_is_strict() {
    let (mut sim, p, _) = perk_sim(perks_of(&[16004]));
    sim.set_tick_interval(0.01);
    let mut counts = Vec::new();
    for gap in [0, 499, 99] {
        sim.ticks(gap);
        let v = dummy(&mut sim, 2, Vec3::ZERO);
        kill(&mut sim, p, v);
        counts.push(sim.app.world().get::<CaPerkState>(p).unwrap().kills);
    }
    assert_eq!(counts, [1, 1, 2]);
}

/// 1 (spawn totals). An M-23F: one real grenade, the maximum two (the
/// real count is shown; the original's client shows the maximum).
#[test]
fn c1_m23f_starts_with_one() {
    let (mut sim, p) = wearer(look(0), default());
    let g = weapon::give(sim.app.world_mut(), p, "combat_arms:m23f").unwrap();
    sim.seconds(1.0);
    let t = sim
        .app
        .world()
        .get::<chimera_sim::weapon::grenade::Throwable>(g)
        .unwrap();
    assert_eq!((t.count, t.max), (1, 2));
}
