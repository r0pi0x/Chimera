//! Halo's vitality and damage tables (data/vitality.toml, data/damage.toml)
//! against the user's disc image: every number they hold equals what
//! `blam_formats` reads from `bloodgulch.map` (the Spartan's collision
//! model, the globals' player information, the three powerups, every
//! damage effect). Passes with a note without an install.

use blam_formats::{
    cache::{MapFile, class},
    damage_effect::DamageEffect,
    globals::Globals,
    object::Equipment,
    vitality::Vitality,
};
use halo_ce::game::vitality::{
    DamageEffects, Spartan,
    data::ids,
};

fn bloodgulch() -> Option<MapFile> {
    let mount = super::maps::mount()?;
    let bytes = mount.read("maps/bloodgulch.map").unwrap();
    Some(MapFile::load(&bytes).unwrap())
}

#[test]
fn the_spartan_s_numbers_are_the_disc_s() {
    let Some(map) = bloodgulch() else { return };
    let s = Spartan::load();
    let m = &s.model;
    let v = Vitality::read(&map, map.find(class::MODEL_COLLISION_GEOMETRY, &m.tag).unwrap()).unwrap();
    assert_eq!(v.flags.bits(), m.flags);
    assert_eq!(v.indirect_damage_material as usize, m.indirect_material);
    assert_eq!(v.maximum_body_vitality, m.maximum_body);
    assert_eq!(v.maximum_shield_vitality, m.maximum_shield);
    assert_eq!(v.friendly_damage_resistance, m.friendly_damage_resistance);
    assert_eq!(v.area_damage_effect_threshold, m.area_damage_effect_threshold);
    assert_eq!(v.body_damaged_threshold, m.body_damaged_threshold);
    assert_eq!(v.body_destroyed_threshold, m.body_destroyed_threshold);
    assert_eq!(v.shield_material_type_code as usize, m.shield_material_type);
    assert_eq!(v.shield_failure_threshold, m.shield_failure_threshold);
    assert_eq!(v.failing_shield_leak_fraction, m.failing_shield_leak);
    assert_eq!(v.minimum_stun_damage, m.minimum_stun_damage);
    assert_eq!((v.stun_time, v.recharge_time), (m.stun_time, m.recharge_time));
    assert_eq!(v.runtime_shield_recharge_velocity.to_bits(), m.recharge_velocity.to_bits());
    assert_eq!(v.shield_damaged_threshold, m.shield_damaged_threshold);
    assert_eq!(v.materials.len(), m.materials.len());
    for (disc, ours) in v.materials.iter().zip(&m.materials) {
        assert_eq!(disc.name, ours.name);
        assert_eq!(disc.is_head(), ours.head, "{}", ours.name);
        assert_eq!(disc.material_type_code as usize, ours.kind, "{}", ours.name);
        assert_eq!(
            (disc.shield_leak_fraction, disc.shield_damage_multiplier, disc.body_damage_multiplier),
            (ours.leak, ours.shield, ours.body),
            "{}",
            ours.name
        );
    }
    assert_eq!(v.regions.len(), m.regions.len());
    for (disc, ours) in v.regions.iter().zip(&m.regions) {
        assert_eq!((disc.name.as_str(), disc.flags.bits(), disc.damage_threshold), (ours.name.as_str(), ours.flags, ours.threshold));
    }
    let g = Globals::read(&map, map.find(class::GLOBALS, r"globals\globals").unwrap()).unwrap();
    let p = &g.player;
    let ours = &s.globals;
    assert_eq!(
        (p.minimum_stun_time, p.maximum_stun_time, p.run_forward_speed),
        (ours.minimum_stun_time, ours.maximum_stun_time, ours.run_forward_speed)
    );
    assert_eq!(
        (p.stun_movement_penalty, p.stun_turning_penalty, p.stun_jumping_penalty),
        (ours.stun_movement_penalty, ours.stun_turning_penalty, ours.stun_jumping_penalty)
    );
    for ours in [&s.powerups.overshield, &s.powerups.active_camouflage, &s.powerups.health_pack] {
        let eq = Equipment::read(&map, map.find(class::EQUIPMENT, &ours.tag).unwrap()).unwrap();
        assert_eq!((eq.own.powerup_type.code() as u16, eq.own.powerup_time), (ours.kind, ours.time), "{}", ours.tag);
    }
}

#[test]
fn every_damage_effect_is_the_disc_s() {
    let Some(map) = bloodgulch() else { return };
    let fx = DamageEffects::load();
    for id in ids::ALL {
        let ours = fx.get(id).unwrap();
        let tag = map.find(class::DAMAGE_EFFECT, &ours.tag).unwrap_or_else(|| panic!("no {}", ours.tag));
        let d = DamageEffect::read(&map, tag).unwrap();
        let what = &ours.tag;
        assert_eq!(d.category.code() as u16, ours.category.code(), "{what}");
        assert_eq!(d.side_effect.code() as u16, ours.side_effect.code(), "{what}");
        assert_eq!(d.damage_flags.bits(), ours.flags, "{what}");
        assert_eq!(d.flags.bits() & 1 != 0, ours.no_distance_scaling, "{what}");
        assert_eq!(d.damage_lower_bound, ours.lower, "{what}");
        assert_eq!([d.damage_upper_bound.lower, d.damage_upper_bound.upper], ours.upper, "{what}");
        assert_eq!([d.radius.lower, d.radius.upper], ours.radius, "{what}");
        assert_eq!(d.area_of_effect_core_radius, ours.core, "{what}");
        if ours.passthrough != 0.0 {
            assert_eq!(d.vehicle_passthrough_penalty, ours.passthrough, "{what}");
        }
        assert_eq!(d.active_camouflage_damage, ours.camo, "{what}");
        assert_eq!([d.stun, d.maximum_stun, d.stun_time], ours.stun, "{what}");
        assert_eq!(d.instantaneous_acceleration, ours.accel, "{what}");
        for t in 0..33 {
            assert_eq!(d.material_modifiers.0[t], ours.modifiers[t], "{what}: material type {t}");
        }
    }
}
