//! The weapon tag reader and its formulas on a synthetic weapon (one
//! magazine, one trigger) with the numbers of specs/halo_ce/weapons.md:
//! the layout, the rejections, the rate of fire rule, zoom and aim assist.

// The spec's numbers are written as it states them, rounded.
#![allow(clippy::approx_constant, clippy::excessive_precision)]

use blam_formats::cache::{MapFile, class};
use blam_formats::object::{ObjectTag, ObjectType};
use blam_formats::weapon::{
    AimAssist, Distribution, FiringNoise, MovementPenalized, OverchargedAction, SecondaryTriggerMode, TriggerFlags,
    Weapon, WeaponFlags, WeaponInput, WeaponType, per_tick_rate, seconds_to_ticks, zoomed_field_of_view,
};

use crate::map_fixture::{CacheFile, Struct, TagData, null_ref};

const P: usize = 0x308;

const SCNR: usize = 0;
const EQIP: usize = 1;
const PROJ: usize = 2;
const EFFE: usize = 3;
const JPT: usize = 4;
const WEAP: usize = 5;

fn tags() -> Vec<(&'static str, &'static str)> {
    vec![
        ("scnr", "scenarios\\syn"),
        ("eqip", "powerups\\test ammo\\test ammo"),
        ("proj", "weapons\\test\\bullet"),
        ("effe", "weapons\\test\\effects\\fire"),
        ("jpt!", "weapons\\test\\trigger"),
        ("weap", "weapons\\test\\gun"),
    ]
}

/// The trigger of the spec's assault rifle row, with a firing effect.
fn trigger(td: &mut TagData, magazine: i16) -> Vec<u8> {
    let effects = td.block(&[Struct::new(0x84)
        .i16(0, 1)
        .i16(2, 1)
        .put(0x24, &td.tag_ref(EFFE))
        .put(0x34, &null_ref("effe"))
        .put(0x44, &null_ref("snd!"))
        .put(0x54, &td.tag_ref(JPT))
        .put(0x64, &null_ref("jpt!"))
        .put(0x74, &null_ref("jpt!"))
        .into()]);
    Struct::new(0x114)
        .u32(0x00, 0x0408)
        .f32s(0x04, &[15.0, 15.0])
        .f32(0x0C, 1.0)
        .f32(0x10, 0.1)
        .f32(0x14, 0.5)
        .i16(0x20, magazine)
        .i16(0x22, 1)
        .i16(0x24, 2)
        .i16(0x26, 3)
        .i16(0x2E, 2)
        .f32s(0x30, &[0.0, 1.0])
        .f32(0x38, 0.6)
        .f32(0x3C, 1.0)
        .f32(0x48, 0.6)
        .f32(0x4C, 45.0)
        .i16(0x50, 1)
        .f32(0x54, 1.0)
        .f32(0x58, 0.5)
        .i16(0x6C, 1)
        .i16(0x6E, 15)
        .f32(0x70, 0.1)
        .f32(0x78, 0.0349066)
        .f32s(0x7C, &[0.0349066, 0.113446])
        .f32s(0x84, &[0.0, -0.1, 0.0])
        .put(0x94, &td.tag_ref(PROJ))
        .f32(0xA4, 0.15)
        .f32(0xA8, 0.1)
        .f32(0xB8, 0.08)
        .f32(0xBC, 0.005)
        .f32(0xC4, 1.0)
        .f32(0xF0, 1.0 / 3.0)
        .f32(0xF4, 0.222222)
        .f32(0xF8, 0.0333333)
        .f32(0xFC, 0.333333)
        .f32(0x100, 0.0555556)
        .f32(0x104, 0.0333333)
        .put(0x108, &effects)
        .into()
}

fn weapon_map(trigger_magazine: i16, extra_magazines: usize) -> MapFile {
    let mut td = TagData::new(&tags());
    td.set_struct(SCNR, &Struct::new(0x5B0).0);
    let ammo = td.block(&[Struct::new(0x1C).i16(0, 240).put(0x0C, &td.tag_ref(EQIP)).into()]);
    let mag = |ammo: [u8; 12]| -> Vec<u8> {
        Struct::new(0x70)
            .u32(0, 1)
            .i16(4, 0)
            .i16(6, 240)
            .i16(8, 600)
            .i16(0xA, 60)
            .f32(0x14, 3.4)
            .i16(0x18, 60)
            .f32(0x1C, 0.3)
            .put(0x38, &null_ref("effe"))
            .put(0x64, &ammo)
            .into()
    };
    let mags: Vec<Vec<u8>> = (0..1 + extra_magazines).map(|_| mag(ammo)).collect();
    let magazines = td.block(&mags);
    let triggers = {
        let t = trigger(&mut td, trigger_magazine);
        td.block(&[t])
    };
    let resources = td.block(&[Struct::new(8).i16(0, 0).u32(4, 0xFFFF_FFFF).into()]);
    let main = Struct::new(0x508)
        .i16(0, 2)
        .u16(2, 4)
        .f32(4, 0.6)
        .u32(0x17C, 0)
        .i16(0x180, 4)
        .f32(0x184, 1.8)
        .u32(P, 0x4020)
        .str(P + 4, "ar")
        .i16(P + 0x24, 0)
        .i16(P + 0x28, 14)
        .i16(P + 0x2A, 2)
        .i16(P + 0x2C, 12)
        .f32(P + 0x30, 1.3)
        .put(P + 0x34, &null_ref("effe"))
        .f32(P + 0x44, 0.25)
        .f32(P + 0x48, 1.0)
        .f32(P + 0x4C, 1.0)
        .f32(P + 0x54, 0.3)
        .f32(P + 0x58, 1.0)
        .put(P + 0x8C, &td.tag_ref(JPT))
        .put(P + 0x9C, &null_ref("jpt!"))
        .f32(P + 0xC4, 4.0)
        .f32(P + 0xC8, 16.0)
        .f32(P + 0xCC, 80.0)
        .i16(P + 0xD2, 2)
        .f32s(P + 0xD4, &[2.0, 8.0])
        .f32(P + 0xDC, 0.0523599)
        .f32(P + 0xE0, 30.0)
        .f32(P + 0xE4, 0.10472)
        .f32(P + 0xE8, 30.0)
        .f32(P + 0xEC, 0.0)
        .i16(P + 0xF4, 2)
        .f32(P + 0xF8, 0.25)
        .f32(P + 0xFC, 0.25)
        .f32(P + 0x138, 0.2)
        .f32(P + 0x13C, 0.2)
        .f32(P + 0x140, 0.9)
        .f32(P + 0x144, 0.5)
        .f32(P + 0x1C4, 0.25)
        .f32(P + 0x1C8, 0.04)
        .i16(P + 0x1DA, 4)
        .put(P + 0x1DC, &resources)
        .put(P + 0x1E8, &magazines)
        .put(P + 0x1F4, &triggers);
    td.set_struct(WEAP, &main.0);
    MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap()
}

fn tag(map: &MapFile) -> &blam_formats::cache::TagEntry {
    &map.tags()[WEAP]
}

#[test]
fn reads_the_weapon_part_with_one_magazine_and_one_trigger() {
    let map = weapon_map(0, 0);
    let w = Weapon::read(&map, tag(&map)).unwrap();
    assert_eq!(w.object.object_type, ObjectType::Weapon);
    assert_eq!(w.object.bounding_radius, 0.6);
    assert_eq!((w.item.message_index, w.item.scale), (4, 1.8));
    let p = &w.own;
    assert!(p.flags.contains(WeaponFlags::AIM_ASSISTS_ONLY_WHEN_ZOOMED));
    assert!(p.flags.contains(WeaponFlags::ENABLES_INTEGRATED_NIGHT_VISION));
    assert_eq!(p.flags.bits(), 0x4020);
    assert_eq!(p.flags.unnamed_bits(), 0);
    assert_eq!(p.label, "ar");
    assert_eq!(p.secondary_trigger_mode, SecondaryTriggerMode::Normal);
    assert_eq!(
        [p.input_a, p.input_b, p.input_c, p.input_d],
        [WeaponInput::IntegratedLight, WeaponInput::PrimaryAmmunition, WeaponInput::Illumination, WeaponInput::None]
    );
    assert_eq!(p.ready_time, 1.3);
    assert_eq!((p.heat_recovery_threshold, p.overheated_threshold), (0.25, 1.0));
    assert_eq!((p.heat_detonation_threshold, p.heat_loss_per_second, p.heat_illumination), (1.0, 0.3, 1.0));
    assert_eq!(map.tag(p.melee_damage.id).unwrap().class, class::DAMAGE_EFFECT);
    assert!(p.melee_response.is_null());
    assert_eq!((p.near_reticle_range, p.far_reticle_range, p.intersection_reticle_range), (4.0, 16.0, 80.0));
    assert_eq!(p.magnification_levels, 2);
    assert_eq!((p.magnification_range.lower, p.magnification_range.upper), (2.0, 8.0));
    assert_eq!((p.autoaim_angle, p.autoaim_range), (0.0523599, 30.0));
    assert_eq!((p.magnetism_angle, p.magnetism_range, p.deviation_angle), (0.10472, 30.0, 0.0));
    assert_eq!(p.movement_penalized, MovementPenalized::WhenZoomedOrReloading);
    assert_eq!((p.forward_movement_penalty, p.sideways_movement_penalty), (0.25, 0.25));
    assert_eq!((p.age_heat_recovery_penalty, p.age_rate_of_fire_penalty), (0.2, 0.2));
    assert_eq!((p.age_misfire_start, p.age_misfire_chance), (0.9, 0.5));
    assert_eq!((p.active_camo_ding, p.active_camo_regrowth_rate), (0.25, 0.04));
    assert_eq!(p.weapon_type, WeaponType::PlasmaRifle);
    assert_eq!(p.predicted_resources.len(), 1);

    assert_eq!(p.magazines.len(), 1);
    let m = &p.magazines[0];
    assert!(m.flags.contains(blam_formats::weapon::MagazineFlags::WASTES_ROUNDS_WHEN_RELOADED));
    assert_eq!((m.rounds_total_initial, m.rounds_total_maximum, m.rounds_loaded_maximum), (240, 600, 60));
    assert_eq!((m.reload_time, m.rounds_reloaded, m.chamber_time), (3.4, 60, 0.3));
    assert_eq!(m.magazines.len(), 1);
    assert_eq!(m.magazines[0].rounds, 240);
    assert_eq!(map.tag(m.magazines[0].equipment.id).unwrap().class, class::EQUIPMENT);

    assert_eq!(p.triggers.len(), 1);
    let t = &p.triggers[0];
    assert!(t.flags.contains(TriggerFlags::DOES_NOT_REPEAT_AUTOMATICALLY));
    assert!(t.flags.contains(TriggerFlags::USE_ERROR_WHEN_UNZOOMED));
    assert_eq!((t.rounds_per_second.lower, t.rounds_per_second.upper), (15.0, 15.0));
    assert_eq!((t.acceleration_time, t.deceleration_time, t.blurred_rate_of_fire), (1.0, 0.1, 0.5));
    assert_eq!(t.magazine_index(), Some(0));
    assert_eq!((t.rounds_per_shot, t.minimum_rounds_loaded, t.rounds_between_tracers), (1, 2, 3));
    assert_eq!(t.firing_noise, FiringNoise::Loud);
    assert_eq!((t.error_acceleration_time, t.error_deceleration_time), (0.6, 1.0));
    assert_eq!((t.charging_time, t.charged_time), (0.6, 45.0));
    assert_eq!(t.overcharged_action, OverchargedAction::Explodes);
    assert_eq!((t.charged_illumination, t.spew_time), (1.0, 0.5));
    assert_eq!(t.distribution_function, Distribution::HorizontalFan);
    assert_eq!((t.projectiles_per_shot, t.distribution_angle), (15, 0.1));
    assert_eq!(t.minimum_error, 0.0349066);
    assert_eq!((t.error_angle.lower, t.error_angle.upper), (0.0349066, 0.113446));
    assert_eq!(t.first_person_offset, [0.0, -0.1, 0.0]);
    assert_eq!(map.tag(t.projectile.id).unwrap().path, "weapons\\test\\bullet");
    assert_eq!((t.ejection_port_recovery_time, t.illumination_recovery_time), (0.15, 0.1));
    assert_eq!((t.heat_generated_per_round, t.age_generated_per_round, t.overload_time), (0.08, 0.005, 1.0));
    assert_eq!(t.runtime_rate_of_fire_acceleration, 0.0333333);
    assert_eq!(t.firing_effects.len(), 1);
    let e = &t.firing_effects[0];
    assert_eq!((e.shot_count.lower, e.shot_count.upper), (1, 1));
    assert_eq!(map.tag(e.firing_effect.id).unwrap().class, class::EFFECT);
    assert!(e.misfire_effect.is_null() && e.empty_effect.is_null() && e.misfire_damage.is_null());
    assert_eq!(map.tag(e.firing_damage.id).unwrap().path, "weapons\\test\\trigger");

    // The trigger's magazine resolves.
    assert_eq!(w.magazine_of(0).unwrap().rounds_loaded_maximum, 60);
    assert!(w.magazine_of(1).is_none());
}

#[test]
fn dispatches_as_a_weapon() {
    let map = weapon_map(0, 0);
    let t = ObjectTag::read(&map, tag(&map)).unwrap();
    assert_eq!(t.object().bounding_radius, 0.6);
    let ObjectTag::Weapon(w) = t else { panic!("not a weapon") };
    assert_eq!(w.own.label, "ar");
}

#[test]
fn trigger_without_a_magazine_and_bad_indices() {
    let map = weapon_map(-1, 0);
    let w = Weapon::read(&map, tag(&map)).unwrap();
    assert_eq!(w.own.triggers[0].magazine_index(), None);
    assert!(w.magazine_of(0).is_none());
    // A magazine index past the block is damage; so is -2.
    for bad in [1, -2] {
        let map = weapon_map(bad, 0);
        let err = Weapon::read(&map, tag(&map)).unwrap_err();
        assert!(err.to_string().contains("trigger 0 draws from magazine"), "{err}");
    }
    // Three magazines are cut to two; index 1 is then fine, 2 is not.
    let map = weapon_map(1, 2);
    let w = Weapon::read(&map, tag(&map)).unwrap();
    assert_eq!(w.own.magazines.len(), 2);
    assert!(Weapon::read(&weapon_map(2, 2), &weapon_map(2, 2).tags()[WEAP]).is_err());
}

#[test]
fn enumerations_out_of_range_are_refused() {
    let patch = |off: usize, v: i16| {
        let mut td = TagData::new(&tags());
        td.set_struct(SCNR, &Struct::new(0x5B0).0);
        let at = td.set_struct(WEAP, &Struct::new(0x508).i16(0, 2).i16(off, v).0);
        let _ = at;
        MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap()
    };
    let map = patch(P + 0x1DA, 5);
    assert!(Weapon::read(&map, tag(&map)).is_err(), "weapon type 5");
    let map = patch(P + 0x28, 17);
    assert!(Weapon::read(&map, tag(&map)).is_err(), "input 17");
    let map = patch(P + 0x24, 5);
    assert!(Weapon::read(&map, tag(&map)).is_err(), "secondary mode 5");
    let map = patch(P + 0x1DA, 4);
    assert!(Weapon::read(&map, tag(&map)).is_ok());
    // A biped's tag is not a weapon.
    let mut td = TagData::new(&[("scnr", "s"), ("bipd", "b")]);
    td.set_struct(0, &Struct::new(0x5B0).0);
    td.set_struct(1, &Struct::new(0x4F4).0);
    let map = MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap();
    assert!(Weapon::read(&map, &map.tags()[1]).is_err());
}

fn trig(lower: f32, upper: f32) -> blam_formats::weapon::Trigger {
    let map = weapon_map(0, 0);
    let mut t = Weapon::read(&map, tag(&map)).unwrap().own.triggers.remove(0);
    t.rounds_per_second.lower = lower;
    t.rounds_per_second.upper = upper;
    t
}

#[test]
fn ticks_between_shots() {
    // The spec's rows: rate bounds, the ticks at rate of fire 0 and 1 with
    // no battery penalty.
    for (lo, hi, at0, at1) in [
        (15.0, 15.0, 2, 2),
        (3.5, 3.5, 9, 9),
        (1.0, 1.0, 30, 30),
        (7.0, 10.0, 5, 3),
        (2.0, 2.0, 15, 15),
        (3.0, 10.0, 10, 3),
        (0.5, 0.5, 60, 60),
        (30.0, 30.0, 1, 1),
        (8.0, 15.0, 4, 2),
        (5.0, 6.0, 6, 5),
    ] {
        let t = trig(lo, hi);
        assert_eq!(t.ticks_between_shots(0.0, 0.0, 0.0), at0, "{lo} - {hi} at 0");
        assert_eq!(t.ticks_between_shots(1.0, 0.0, 0.0), at1, "{lo} - {hi} at 1");
    }
    // No limit: 1. (Rate 0 reads required 0.)
    let t = trig(0.0, 0.0);
    assert_eq!(t.required_ticks(0.0, 0.0, 0.0), 0.0);
    assert_eq!(t.ticks_between_shots(0.0, 0.0, 0.0), 1);
    // The plasma rifle and its battery: age 0.005 at full rate of fire is
    // 3.003 ticks (interval 4); age 0 is 3.0 (3); age 0.995 at rate of fire
    // 0 is 5.1386 (6).
    let t = trig(7.0, 10.0);
    assert!((t.required_ticks(1.0, 0.005, 0.2) - 3.003).abs() < 1e-4);
    assert_eq!(t.ticks_between_shots(1.0, 0.005, 0.2), 4);
    assert_eq!(t.required_ticks(1.0, 0.0, 0.2), 3.0);
    assert_eq!(t.ticks_between_shots(1.0, 0.0, 0.2), 3);
    assert!((t.required_ticks(0.0, 0.995, 0.2) - 5.1386).abs() < 1e-3);
    assert_eq!(t.ticks_between_shots(0.0, 0.995, 0.2), 6);
    // A penalty of 0 ignores the age.
    assert_eq!(t.ticks_between_shots(1.0, 1.0, 0.0), 3);
    // On the weapon: the age penalty is the weapon's own.
    let map = weapon_map(0, 0);
    let w = Weapon::read(&map, tag(&map)).unwrap();
    assert_eq!(w.ticks_between_shots(0, 1.0, 0.0), Some(2));
    assert_eq!(w.ticks_between_shots(0, 1.0, 1.0), Some(3), "15/s: 2 x 1.2 = 2.4");
    assert_eq!(w.ticks_between_shots(1, 1.0, 0.0), None);
}

#[test]
fn seconds_and_per_tick_rates() {
    // Times x 30 truncated: charging 0.6 s gives 18, charged 45 s 1350,
    // chamber 0 gives 0.
    assert_eq!(seconds_to_ticks(0.6), 18);
    assert_eq!(seconds_to_ticks(45.0), 1350);
    assert_eq!(seconds_to_ticks(0.0), 0);
    assert_eq!(seconds_to_ticks(0.3), 9);
    // The runtime rates: 1 / (30 t), 1 for t = 0.
    assert!((per_tick_rate(1.0) - 0.0333333).abs() < 1e-6);
    assert!((per_tick_rate(0.6) - 0.0555556).abs() < 1e-6);
    assert_eq!(per_tick_rate(0.0), 1.0);
    assert!((per_tick_rate(0.1) - 0.333333).abs() < 1e-5);
    // Against the synthetic trigger's stored fields.
    let t = trig(15.0, 15.0);
    assert!((per_tick_rate(t.acceleration_time) - t.runtime_rate_of_fire_acceleration).abs() < 1e-6);
    assert!((per_tick_rate(t.deceleration_time) - t.runtime_rate_of_fire_deceleration).abs() < 1e-5);
    assert!((per_tick_rate(t.error_acceleration_time) - t.runtime_error_acceleration).abs() < 1e-6);
}

#[test]
fn zoom_and_aim_assist() {
    let map = weapon_map(0, 0);
    let mut p = Weapon::read(&map, tag(&map)).unwrap().own;
    // The sniper rifle: 2x then 8x. The pistol and the rocket launcher: one
    // level of 2x.
    assert_eq!(p.magnification(0), Some(2.0));
    assert!((p.magnification(1).unwrap() - 8.0).abs() < 1e-5);
    assert_eq!(p.magnification(2), None);
    p.magnification_levels = 1;
    p.magnification_range.upper = 2.0;
    assert_eq!(p.magnification(0), Some(2.0));
    // Three levels from 2 to 8: the middle one is 4.
    p.magnification_levels = 3;
    p.magnification_range.upper = 8.0;
    assert!((p.magnification(1).unwrap() - 4.0).abs() < 1e-5);
    // A bound not above 0 counts as 1.
    p.magnification_levels = 2;
    p.magnification_range.lower = 0.0;
    assert_eq!(p.magnification(0), Some(1.0));
    p.magnification_levels = 0;
    assert_eq!(p.magnification(0), None);

    // The pistol: 70 degrees base, 2x: 35 degrees.
    let base = 70.0f32.to_radians();
    let fov = zoomed_field_of_view(base, 2.0).unwrap();
    assert!((fov - 35.0f32.to_radians()).abs() < 1e-6);
    assert_eq!(zoomed_field_of_view(base, 1000.0), None, "below pi/100");
    assert_eq!(zoomed_field_of_view(base, 0.5), Some(base / 0.5));
    assert_eq!(zoomed_field_of_view(3.12, 1.0), None, "above 99 pi/100");

    // Aim assist: the pistol's 3 degrees, 30 WU; zoomed 2x: 1.5 degrees,
    // 60 WU, deviation 1.5 degrees.
    let mut pistol = p.clone();
    pistol.flags = WeaponFlags::default();
    pistol.autoaim_angle = 3.0f32.to_radians();
    pistol.autoaim_range = 30.0;
    pistol.magnetism_angle = 6.0f32.to_radians();
    pistol.magnetism_range = 30.0;
    pistol.deviation_angle = 0.0;
    let a = pistol.aim_assist(Some(2.0)).unwrap();
    assert!((a.autoaim_angle - 1.5f32.to_radians()).abs() < 1e-6);
    assert_eq!((a.autoaim_range, a.magnetism_range), (60.0, 60.0));
    assert!((a.magnetism_angle - 3.0f32.to_radians()).abs() < 1e-6);
    assert!((a.deviation - 1.5f32.to_radians()).abs() < 1e-6);
    let a = pistol.aim_assist(None).unwrap();
    assert!((a.deviation - 3.0f32.to_radians()).abs() < 1e-6, "pistol unzoomed: 3 degrees");
    // "Aim assists only when zoomed": none unzoomed.
    pistol.flags = WeaponFlags::AIM_ASSISTS_ONLY_WHEN_ZOOMED;
    assert!(pistol.aim_assist(None).is_none());
    assert!(pistol.aim_assist(Some(2.0)).is_some());
    // The rocket launcher: autoaim 0, deviation 0.
    pistol.flags = WeaponFlags::default();
    pistol.autoaim_angle = 0.0;
    assert_eq!(pistol.aim_assist(None).unwrap().deviation, 0.0);
    // The falloff g.
    assert_eq!(AimAssist::falloff(5.0, 10.0), 1.0);
    assert!((AimAssist::falloff(7.5, 10.0) - 0.5).abs() < 1e-6);
    assert_eq!(AimAssist::falloff(10.0, 10.0), 0.0);
    assert_eq!(AimAssist::falloff(20.0, 10.0), 0.0);
}
