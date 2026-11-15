//! The per-game damage scale (`weapon::GameDamage`, the party apps'
//! table): a weapon's hit takes its game's scale before armour (here the
//! test kit's pistol, game `kit`), a game the table doesn't name and an
//! app without the table deal as before, the map's damage is not scaled,
//! and the power curve counts the scale.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Health, Hitgroup, MapDamage};
use chimera_sim::{
    minigame::power::{self, PowerCurve, PowerReaders},
    testkit::{self, PISTOL},
    weapon::{Armor, GameDamage, data::WeaponCatalog},
};
use chimera_world::greybox;

/// One shot of the kit's pistol (20 points) at the chest of a target 5 m
/// ahead with `table` (None: no table), the target wearing CS:S's kevlar
/// when `kevlar`: (health, armour) after.
fn shot(table: Option<&str>, kevlar: bool) -> (f32, f32) {
    let mut sim = testkit::kit_sim();
    if let Some(t) = table {
        sim.app.insert_resource(GameDamage::parse(t).unwrap());
    }
    let shooter = testkit::with_player(&mut sim, None, PISTOL);
    let t = sim.spawn_character(greybox::SPAWNS[0] - Vec3::Z * 5.0, chimera_sim::movement::placeholder::ID);
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
    for table in [None, Some(""), Some("other = 2.0")] {
        assert!(close(shot(table, false), (0.8, 0.0)), "{table:?}: {:?}", shot(table, false));
    }
    // The kit at 1.5: 30 points.
    let (health, _) = shot(Some("kit = 1.5"), false);
    assert!((health - 0.7).abs() < 1e-4, "{health}");
    // Before armour: kevlar (the pistol has no armour ratio: as it comes)
    // takes the scaled hit as it takes any.
    let plain = shot(None, true);
    let scaled = shot(Some("kit = 1.5"), true);
    assert!((1.0 - scaled.0 - 1.5 * (1.0 - plain.0)).abs() < 0.011, "{plain:?} {scaled:?}");
    assert!(GameDamage::parse("kit = -1").is_err() && GameDamage::parse("kit = 'x'").is_err());
}

/// A trigger's damage (`core::MapDamage`) is the map's, not a game's: as
/// it comes, the table or not.
#[test]
fn the_map_s_damage_is_not_scaled() {
    for table in [None, Some("kit = 2.0")] {
        let mut sim = testkit::kit_sim();
        if let Some(t) = table {
            sim.app.insert_resource(GameDamage::parse(t).unwrap());
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
    readers.game_damage = GameDamage::parse("kit = 1.5").unwrap();
    let scaled = power::stats(data, &readers, &curve, 10.0).damage;
    assert!(plain > 0.0 && (scaled - 1.5 * plain).abs() < 1e-6, "{plain} {scaled}");
}
