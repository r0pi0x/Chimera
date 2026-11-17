//! The cross-game balance table (`weapon::balance::Balance`, the party
//! apps' table) where hits take it (`weapon::HitArmor`): a weapon's hit
//! on another game's body takes its game's outgoing scale before armour
//! (here the test kit's pistol, game `kit`), a game the table doesn't
//! name, an app without the table and the switch off deal as before, the
//! same game on both sides is 1 unless a pair says otherwise, the map's
//! damage is not scaled, and the power curve counts the outgoing column.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Health, Hitgroup, MapDamage};
use chimera_sim::{
    minigame::power::{self, PowerCurve, PowerReaders},
    testkit::{self, PISTOL},
    weapon::{Armor, GameDamage, balance::Balance, data::WeaponCatalog},
};
use chimera_world::greybox;

/// One shot of the kit's pistol (20 points) at the chest of a target 5 m
/// ahead with `table` (None: no table), the target wearing CS:S's kevlar
/// when `kevlar`: (health, armour) after.
fn shot(table: Option<&str>, kevlar: bool) -> (f32, f32) {
    shot_on(table.map(|t| Balance::parse(t).unwrap()), kevlar, None)
}

/// `shot` with a table as given, the target of game `game` (None: none
/// known).
fn shot_on(table: Option<Balance>, kevlar: bool, game: Option<&str>) -> (f32, f32) {
    let mut sim = testkit::kit_sim();
    if let Some(t) = table {
        sim.app.insert_resource(t);
    }
    let shooter = testkit::with_player(&mut sim, None, PISTOL);
    let t = sim.spawn_character(greybox::SPAWNS[0] - Vec3::Z * 5.0, chimera_sim::movement::placeholder::ID);
    if let Some(g) = game {
        sim.app.world_mut().entity_mut(t).insert(chimera_sim::weapon::balance::Victim { game: Some(g.into()), body: None });
    }
    if kevlar {
        sim.app.world_mut().entity_mut(t).insert(Armor { amount: 1.0, helmet: false });
    }
    sim.ticks(2);
    let point = sim.position(t) + Vec3::Y * (1.8 * 0.72 - 0.9);
    testkit::aim_at(&mut sim, shooter, point);
    sim.intent(shooter).fire = true;
    sim.ticks(1);
    sim.intent(shooter).fire = false;
    sim.ticks(1);
    let w = sim.app.world();
    (w.get::<Health>(t).unwrap().current, w.get::<Armor>(t).map_or(0.0, |a| a.amount))
}

fn close(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4
}

#[test]
fn a_weapon_s_hit_takes_its_game_s_scale_before_armour() {
    // No table, or a table without the kit: 20 points.
    for table in [None, Some(""), Some("[games.other]
outgoing = 2.0")] {
        assert!(close(shot(table, false), (0.8, 0.0)), "{table:?}: {:?}", shot(table, false));
    }
    // The kit at 1.5: 30 points.
    let (health, _) = shot(Some(KIT_15), false);
    assert!((health - 0.7).abs() < 1e-4, "{health}");
    // Before armour: kevlar (the pistol has no armour ratio: as it comes)
    // takes the scaled hit as it takes any.
    let plain = shot(None, true);
    let scaled = shot(Some(KIT_15), true);
    assert!((1.0 - scaled.0 - 1.5 * (1.0 - plain.0)).abs() < 0.011, "{plain:?} {scaled:?}");
    for bad in ["[games.kit]
outgoing = -1", "[games.kit]
outgoing = 'x'", "[games.kit]
incoming = 0", "kit = 1.5"] {
        assert!(Balance::parse(bad).is_err(), "{bad}");
    }
}

/// The kit's guns at 1.5 on other games' bodies.
const KIT_15: &str = "[games.kit]
outgoing = 1.5";

/// The switch off (a raw match): the kit's 20 points. The same game on
/// both sides counts the table too (outgoing times its own incoming). A
/// victim of another game: the outgoing times its game's incoming.
#[test]
fn the_switch_and_the_same_game() {
    let raw = Balance::parse(KIT_15).unwrap().raw();
    assert!(close(shot_on(Some(raw), false, None), (0.8, 0.0)), "raw");
    let t = |text: &str| Some(Balance::parse(text).unwrap());
    assert!(close(shot_on(t(KIT_15), false, Some("kit")), (0.7, 0.0)), "kit on kit: 1.5");
    let both = "[games.kit]
outgoing = 1.5
incoming = 2.0";
    assert!(close(shot_on(t(both), false, Some("kit")), (0.4, 0.0)), "kit on kit: 1.5 x 2");
    let other = "[games.kit]
outgoing = 1.5
[games.other]
incoming = 2.0";
    assert!(close(shot_on(t(other), false, Some("other")), (0.4, 0.0)), "1.5 x 2");
}

/// A trigger's damage (`core::MapDamage`) is the map's, not a game's: as
/// it comes, the table or not.
#[test]
fn the_map_s_damage_is_not_scaled() {
    for table in [None, Some("[games.kit]
outgoing = 2.0")] {
        let mut sim = testkit::kit_sim();
        if let Some(t) = table {
            sim.app.insert_resource(Balance::parse(t).unwrap());
        }
        let shooter = testkit::with_player(&mut sim, None, PISTOL);
        let t = sim.spawn_character(greybox::SPAWNS[0] - Vec3::Z * 5.0, chimera_sim::movement::placeholder::ID);
        sim.ticks(2);
        sim.app.world_mut().write_message(MapDamage {
            damage: Damage {
                target: t,
                attacker: Some(shooter),
                amount: 0.2,
                point: Vec3::ZERO,
                dir: Vec3::NEG_Y,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Generic,
                weapon: None,
                force: Vec3::ZERO,
            },
            armour: true,
        });
        sim.ticks(2);
        let h = sim.app.world().get::<Health>(t).unwrap().current;
        assert!((h - 0.8).abs() < 1e-4, "{table:?}: {h}");
    }
}

/// The power curve counts a gun's damage scaled by its game's.
#[test]
fn the_power_curve_counts_the_scale() {
    let sim = testkit::kit_sim();
    let w = sim.app.world();
    let data = w.resource::<WeaponCatalog>().get(PISTOL).unwrap();
    let mut readers = w.get_resource::<PowerReaders>().cloned().unwrap_or_default();
    let curve = PowerCurve::shipped();
    let plain = power::stats(data, &readers, &curve, 10.0).damage;
    readers.game_damage = Balance::parse(KIT_15).unwrap().power_table();
    assert_eq!(readers.game_damage, GameDamage::parse("kit = 1.5").unwrap());
    let scaled = power::stats(data, &readers, &curve, 10.0).damage;
    assert!(plain > 0.0 && (scaled - 1.5 * plain).abs() < 1e-6, "{plain} {scaled}");
}
