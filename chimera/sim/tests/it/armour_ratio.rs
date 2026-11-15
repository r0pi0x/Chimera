//! Armour with its own ratio (`weapon::ArmorRatio`, Halo's shield): a
//! character wearing pieces that cover every part and class, run out by
//! what they block, and ratio 1 takes the test kit's rifle (armour ratio
//! 0.5) and pistol (none) on the armour first, whatever the gun's ratio;
//! without `ArmorRatio` the pistol's hit goes past the armour to health.

use bevy::prelude::*;
use chimera_core::core::Health;
use chimera_sim::{
    testkit::{self, PISTOL, RIFLE},
    weapon::{Armor, ArmorPieces, ArmorRatio, Coverage, Depletion, hits::ArmorPiece},
};
use chimera_world::greybox;

/// Every part, every class, one point of armour per point blocked.
const ALL: Coverage = Coverage {
    class: None,
    head: 1.0,
    torso: 1.0,
    arm: 1.0,
    leg: 1.0,
    groin: 1.0,
    wear: 1.0,
    except: 0,
};

/// One shot of `gun` at the chest of a target 5 m ahead wearing a full
/// cover of `armour` (with `ArmorRatio(1)` when `own_ratio`): (health,
/// armour) after.
fn shot(gun: &str, armour: f32, own_ratio: bool) -> (f32, f32) {
    let mut sim = testkit::kit_sim();
    let shooter = testkit::with_player(&mut sim, None, gun);
    let t = sim.spawn_character(greybox::SPAWNS[0] - Vec3::Z * 5.0, chimera_sim::movement::placeholder::ID);
    let mut e = sim.app.world_mut().entity_mut(t);
    e.insert((
        Armor { amount: armour, helmet: false },
        ArmorPieces {
            pieces: vec![ArmorPiece { entries: vec![ALL] }],
            depletion: Depletion::Blocked,
            armour_quantum: None,
        },
    ));
    if own_ratio {
        e.insert(ArmorRatio(1.0));
    }
    sim.ticks(2);
    let point = sim.position(t) + Vec3::Y * (1.8 * 0.72 - 0.9);
    testkit::aim_at(&mut sim, shooter, point);
    sim.intent(shooter).fire = true;
    sim.ticks(1);
    sim.intent(shooter).fire = false;
    sim.ticks(1);
    let w = sim.app.world();
    (w.get::<Health>(t).unwrap().current, w.get::<Armor>(t).unwrap().amount)
}

#[test]
fn armour_with_its_own_ratio_takes_every_gun_s_hit_first() {
    for gun in [RIFLE, PISTOL] {
        let (health, armour) = shot(gun, 1.0, true);
        assert_eq!(health, 1.0, "{gun}: the armour took the whole hit");
        assert!(armour < 1.0 && armour > 0.5, "{gun}: armour {armour}");
    }
    // The overflow of a hit bigger than what is left reaches health.
    let (health, armour) = shot(PISTOL, 0.05, true);
    assert_eq!(armour, 0.0);
    assert!(health < 1.0 && health > 0.8, "overflow {health}");
}

#[test]
fn without_it_the_gun_s_ratio_decides() {
    // The kit's pistol has no armour ratio: armour doesn't apply.
    let (health, armour) = shot(PISTOL, 1.0, false);
    assert_eq!(armour, 1.0);
    assert!(health < 1.0, "{health}");
}
