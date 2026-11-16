//! Halo's vitality and damage tables (`game::vitality::data`:
//! data/vitality.toml, data/damage.toml) parse, hold every id, and give
//! the spec's derived numbers bit for bit (health_shields.md
//! "Constants"; projectiles.md "Damage effects").

use halo_ce::game::vitality::{
    DamageEffects, Spartan,
    data::{Category, SideEffect, flags, ticks},
    ids,
};

#[test]
fn the_tables_parse_and_hold_every_id() {
    let s = Spartan::load();
    let fx = DamageEffects::load();
    assert_eq!(fx.0.len(), ids::ALL.len() + ids::CAMPAIGN.len());
    for id in ids::ALL.into_iter().chain(ids::CAMPAIGN) {
        assert!(fx.get(id).is_some(), "{id:?}");
    }
    assert_eq!(s.model.materials.len(), 4);
    assert_eq!(s.model.materials[s.model.indirect_material].name, "body");
    assert!(s.model.materials[0].head && s.model.materials[2].body == 0.8);
}

#[test]
fn the_spec_s_derived_numbers() {
    let s = Spartan::load();
    assert_eq!(s.model.recharge_velocity.to_bits(), 0x3C088889, "RECHARGE_STEP");
    assert_eq!((1.0f32 / s.model.maximum_shield).to_bits(), 0.013333334f32.to_bits());
    assert_eq!(s.shield_delay(), 180);
    assert_eq!(ticks(0.15), 4);
    assert_eq!(ticks(s.globals.maximum_stun_time), 37);
    assert_eq!(ticks(s.unit.soft_ping_interrupt_time), 9);
    assert_eq!(ticks(s.powerups.active_camouflage.time), 1350);
    assert_eq!(s.constants.overshield_drain, 1.0 / 1350.0);
    assert_eq!(s.constants.overcharge_step, 1.0 / 30.0);
    assert_eq!(s.constants.damage_fade, 1.0 / 60.0);
    assert_eq!(s.constants.camo_step, 1.0 / 120.0);
}

#[test]
fn damage_effects_as_the_spec_lists_them() {
    let fx = DamageEffects::load();
    let pistol = fx.get(ids::PISTOL_BULLET).unwrap();
    assert_eq!((pistol.modifier(21), pistol.modifier(22), pistol.modifier(9)), (1.5, 1.0, 4.0));
    assert!(pistol.has(flags::CAN_CAUSE_HEADSHOTS));
    let bolt = fx.get(ids::PLASMA_RIFLE_BOLT).unwrap();
    assert_eq!(bolt.flags, 0x804);
    assert_eq!((bolt.modifier(21), bolt.modifier(22), bolt.stun), (0.5, 2.0, [1.0, 1.0, 0.15]));
    let melee = fx.get(ids::NEEDLER_MELEE).unwrap();
    assert_eq!(melee, &{
        let mut m = fx.get(ids::ASSAULT_RIFLE_MELEE).unwrap().clone();
        m.tag = r"weapons\needler\melee".into();
        m
    });
    assert_eq!((melee.side_effect, melee.category, melee.no_distance_scaling), (SideEffect::LethalToTheUnsuspecting, Category::Melee, true));
    assert_eq!((melee.amount_with(0.0, 50.0), melee.amount(1.0, 1.0), melee.modifier(22)), (40.0, 60.0, 1.4));
    let falling = fx.get(ids::FALLING).unwrap();
    assert_eq!((falling.modifier(30), falling.modifier(31)), (1.0, 0.0));
    let shell = fx.get(ids::FRAG_SHOCK_WAVE).unwrap();
    assert_eq!((shell.modifier(9), shell.modifier(21)), (1.0, 0.0));
}
