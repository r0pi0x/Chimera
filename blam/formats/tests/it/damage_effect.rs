//! The damage effect reader on synthetic `jpt!` tags with the numbers of
//! specs/halo_ce/projectiles.md ("Damage effects"): the layout, the
//! material modifier lookup, the rejections, and the formulas on the tag's
//! fields: test case (b)'s direct-hit amounts and test case (d)'s area
//! damage table.

// The spec's numbers are written as it states them, rounded.
#![allow(clippy::approx_constant, clippy::excessive_precision, clippy::type_complexity)]

use blam_formats::cache::{MapFile, TagEntry};
use blam_formats::collision::MaterialType;
use blam_formats::damage_effect::{
    DamageCategory, DamageEffect, DamageFlags, EffectFlags, FlashPriority, MATERIAL_SLOTS, ScreenFlashType, SideEffect,
};
use blam_formats::object::TransitionFunction;
use blam_formats::shader::PeriodicFunction;

use crate::map_fixture::{CacheFile, Struct, TagData};

const SCNR: usize = 0;
const SND: usize = 1;
const JPT: usize = 2;

fn tags() -> Vec<(&'static str, &'static str)> {
    vec![
        ("scnr", "scenarios\\syn"),
        ("snd!", "sound\\test\\hurt"),
        ("jpt!", "weapons\\test\\explosion"),
    ]
}

/// A struct with the damage part of one disc row: lower bound, upper bound
/// pair, radii, flags and the modifiers.
fn row(lower: f32, upper: (f32, f32), radii: (f32, f32), flags: u32, mods: &[(usize, f32)]) -> Struct {
    let mut s = Struct::new(0x2A0)
        .f32s(0x000, &[radii.0, radii.1])
        .u32(0x00C, flags)
        .f32(0x1D0, lower)
        .f32s(0x1D4, &[upper.0, upper.1]);
    for (slot, v) in mods {
        s = s.f32(0x200 + 4 * slot, *v);
    }
    s
}

fn map_of(s: Struct) -> MapFile {
    let mut td = TagData::new(&tags());
    td.set_struct(SCNR, &Struct::new(0x5B0).0);
    let mut s = s;
    s = s.put(0x114, &td.tag_ref(SND));
    td.set_struct(JPT, &s.0);
    MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap()
}

fn tag(map: &MapFile) -> &TagEntry {
    &map.tags()[JPT]
}

fn effect(s: Struct) -> DamageEffect {
    let map = map_of(s);
    DamageEffect::read(&map, tag(&map)).unwrap()
}

#[test]
fn reads_every_field() {
    let s = row(
        80.0,
        (300.0, 330.0),
        (0.5, 2.0),
        0x1,
        &[(21, 1.4), (22, 1.5), (32, 0.25)],
    )
    .f32(0x008, 0.3)
    .i16(0x024, 6)
    .i16(0x026, 2)
    .f32(0x034, 0.4)
    .i16(0x038, 5)
    .f32(0x044, 0.9)
    .f32(0x048, 0.1)
    .f32s(0x04C, &[1.0, 0.8, 0.6, 0.4])
    .f32(0x05C, 0.5)
    .f32(0x060, 0.3)
    .i16(0x064, 1)
    .f32(0x070, 0.6)
    .f32(0x074, 0.2)
    .i16(0x078, 2)
    .f32(0x084, 0.25)
    .f32(0x098, 0.5)
    .i16(0x09C, 3)
    .f32(0x0A0, 0.05)
    .f32(0x0A4, 0.1)
    .f32s(0x0A8, &[-0.01, 0.02])
    .f32(0x0B0, 0.7)
    .f32(0x0B8, 0.12)
    .f32(0x0BC, 0.3)
    .f32(0x0CC, 1.5)
    .i16(0x0D0, 4)
    .f32(0x0D4, 0.03)
    .f32(0x0D8, 0.04)
    .i16(0x0E8, 2)
    .f32(0x0EC, 30.0)
    .f32(0x0F0, 0.8)
    .f32(0x0F4, 0.9)
    .f32(0x194, 4.0)
    .f32(0x198, 5.0)
    .f32(0x19C, 6.0)
    .f32(0x1AC, 7.0)
    .f32(0x1B0, 8.0)
    .f32(0x1B4, 9.0)
    .i16(0x1C4, 3)
    .i16(0x1C6, 4)
    .u32(0x1C8, 0x20)
    .f32(0x1CC, 0.6)
    .f32(0x1DC, 0.5)
    .f32(0x1E0, 1.0)
    .f32(0x1E4, 1.0)
    .f32(0x1E8, 0.9)
    .f32(0x1EC, 0.15)
    .f32(0x1F4, 6.0);
    let map = map_of(s);
    let d = DamageEffect::read(&map, tag(&map)).unwrap();
    assert_eq!((d.radius.lower, d.radius.upper), (0.5, 2.0));
    assert_eq!(d.cutoff_scale, 0.3);
    assert!(d.flags.contains(EffectFlags::DO_NOT_SCALE_DAMAGE_BY_DISTANCE));
    assert_eq!(d.screen_flash_type, ScreenFlashType::Tint);
    assert_eq!(d.screen_flash_priority, FlashPriority::High);
    assert_eq!(d.screen_flash_duration, 0.4);
    assert_eq!(d.screen_flash_fade_function, TransitionFunction::Cosine);
    assert_eq!(
        (d.screen_flash_maximum_intensity, d.screen_flash_zero_scale_factor),
        (0.9, 0.1)
    );
    assert_eq!(
        (
            d.screen_flash_color.a,
            d.screen_flash_color.r,
            d.screen_flash_color.g,
            d.screen_flash_color.b
        ),
        (1.0, 0.8, 0.6, 0.4)
    );
    assert_eq!(
        (d.low_frequency_vibrate_frequency, d.low_frequency_vibrate_duration),
        (0.5, 0.3)
    );
    assert_eq!(d.low_frequency_vibrate_fade_function, TransitionFunction::Early);
    assert_eq!(
        (d.high_frequency_vibrate_frequency, d.high_frequency_vibrate_duration),
        (0.6, 0.2)
    );
    assert_eq!(d.high_frequency_vibrate_fade_function, TransitionFunction::VeryEarly);
    assert_eq!(d.vibrate_zero_scale_factor, 0.25);
    assert_eq!(d.temporary_camera_impulse_duration, 0.5);
    assert_eq!(d.temporary_camera_impulse_fade_function, TransitionFunction::Late);
    assert_eq!(
        (d.temporary_camera_impulse_rotation, d.temporary_camera_impulse_pushback),
        (0.05, 0.1)
    );
    assert_eq!(
        (
            d.temporary_camera_impulse_jitter.lower,
            d.temporary_camera_impulse_jitter.upper
        ),
        (-0.01, 0.02)
    );
    assert_eq!(d.temporary_camera_impulse_zero_scale_factor, 0.7);
    assert_eq!(
        (
            d.permanent_camera_impulse_angle,
            d.permanent_camera_impulse_zero_scale_factor
        ),
        (0.12, 0.3)
    );
    assert_eq!(d.camera_shaking_duration, 1.5);
    assert_eq!(d.camera_shaking_falloff_function, TransitionFunction::VeryLate);
    assert_eq!(
        (d.camera_shaking_random_translation, d.camera_shaking_random_rotation),
        (0.03, 0.04)
    );
    assert_eq!(d.camera_shaking_wobble_function, PeriodicFunction::Cosine);
    assert_eq!(
        (
            d.camera_shaking_wobble_period,
            d.camera_shaking_wobble_weight,
            d.camera_shaking_zero_scale_factor
        ),
        (30.0, 0.8, 0.9)
    );
    assert_eq!(map.tag(d.sound.id).unwrap().path, "sound\\test\\hurt");
    assert_eq!(
        (
            d.breaking_effect_forward_velocity,
            d.breaking_effect_forward_radius,
            d.breaking_effect_forward_exponent
        ),
        (4.0, 5.0, 6.0)
    );
    assert_eq!(
        (
            d.breaking_effect_outward_velocity,
            d.breaking_effect_outward_radius,
            d.breaking_effect_outward_exponent
        ),
        (7.0, 8.0, 9.0)
    );
    assert_eq!(d.side_effect, SideEffect::Emp);
    assert_eq!(d.category, DamageCategory::HighExplosive);
    assert!(d.damage_flags.contains(DamageFlags::DETONATES_EXPLOSIVES));
    assert_eq!(d.damage_flags.unnamed_bits(), 0);
    assert_eq!(d.area_of_effect_core_radius, 0.6);
    assert_eq!(d.damage_lower_bound, 80.0);
    assert_eq!((d.damage_upper_bound.lower, d.damage_upper_bound.upper), (300.0, 330.0));
    assert_eq!((d.vehicle_passthrough_penalty, d.active_camouflage_damage), (0.5, 1.0));
    assert_eq!((d.stun, d.maximum_stun, d.stun_time), (1.0, 0.9, 0.15));
    assert_eq!(d.instantaneous_acceleration, 6.0);
    assert_eq!(d.material_modifiers.0.len(), MATERIAL_SLOTS);
}

#[test]
fn looks_modifiers_up_by_material_type() {
    let d = effect(row(
        10.0,
        (10.0, 10.0),
        (0.0, 0.0),
        0,
        &[(9, 4.0), (21, 1.5), (22, 1.0), (32, 0.25), (39, 7.0)],
    ));
    assert_eq!(d.material_modifier(MaterialType::Glass), 4.0);
    assert_eq!(d.material_modifier(MaterialType::Cyborg), 1.5);
    assert_eq!(d.material_modifier(MaterialType::CyborgEnergyShield), 1.0);
    assert_eq!(d.material_modifier(MaterialType::HunterShield), 0.25);
    assert_eq!(d.material_modifier(MaterialType::Dirt), 0.0);
    // The "none" type multiplies by 1; slots 33 to 39 are kept as stored.
    assert_eq!(d.material_modifier_or_one(None), 1.0);
    assert_eq!(d.material_modifier_or_one(Some(MaterialType::Cyborg)), 1.5);
    assert_eq!(d.material_modifiers.0[39], 7.0);
}

#[test]
fn rejects_out_of_range_enumerations_and_other_classes() {
    let bad = Struct::new(0x2A0).i16(0x1C6, 13);
    let map = map_of(bad);
    let err = DamageEffect::read(&map, tag(&map)).unwrap_err();
    assert!(err.to_string().contains("DamageCategory"), "{err}");
    assert!(err.to_string().contains("weapons\\test\\explosion"), "{err}");
    let err = DamageEffect::read(&map, &map.tags()[SND]).unwrap_err();
    assert!(err.to_string().contains("not a damage effect"), "{err}");
}

/// Test case (b): the direct hit's amount is `(1 - s) lower + s U(upper)`.
#[test]
fn direct_hit_amounts() {
    // Pistol bullet: 25, 25 - 25: every draw gives 25 at scale 1.
    let pistol = effect(row(25.0, (25.0, 25.0), (0.0, 0.0), 0, &[]));
    for u in [0.0, 0.5, 1.0] {
        assert_eq!(pistol.amount(1.0, u), 25.0);
    }
    // Warthog bullet: 16 + s (U(16, 24) - 16), 16.0 to 23.788 at s 0.9735.
    let w = effect(row(16.0, (16.0, 24.0), (0.0, 0.0), 0, &[]));
    assert!((w.amount(0.9735, 0.0) - 16.0).abs() < 1e-4);
    assert!((w.amount(0.9735, 1.0) - 23.788).abs() < 1e-3);
    assert!((w.amount(0.8308, 1.0) - 22.646).abs() < 1e-3);
    assert!((w.amount(0.6196, 1.0) - 20.956).abs() < 1e-3);
    // Plasma rifle bolt: lower 10, upper 12 - 14: 11.742 to 13.483 at 0.8708,
    // 10.828 to 11.657 at 0.4142.
    let pr = effect(row(10.0, (12.0, 14.0), (0.0, 0.0), 0, &[]));
    assert!((pr.amount(0.8708, 0.0) - 11.742).abs() < 1e-3);
    assert!((pr.amount(0.8708, 1.0) - 13.483).abs() < 1e-3);
    assert!((pr.amount(0.4142, 0.0) - 10.828).abs() < 1e-3);
    assert!((pr.amount(0.4142, 1.0) - 11.657).abs() < 1e-3);
    // Shotgun pellet: lower 8, upper 18 - 25: scale 1 gives 18 to 25, scale
    // 0 gives 8 whatever the draw.
    let sg = effect(row(8.0, (18.0, 25.0), (0.0, 0.0), 0, &[]));
    assert_eq!((sg.amount(1.0, 0.0), sg.amount(1.0, 1.0)), (18.0, 25.0));
    assert_eq!((sg.amount(0.0, 0.0), sg.amount(0.0, 1.0)), (8.0, 8.0));
}

/// Test case (d): area damage before the target's material modifiers.
#[test]
fn area_damage_table() {
    // (explosion, lower, upper, r0, r1, [amounts at d = 0, 0.5, 1, 1.5, 2]
    // with u = 1 for the upper end; a negative entry means none).
    let rows: [(&str, f32, (f32, f32), (f32, f32), [f32; 5]); 4] = [
        (
            "frag",
            80.0,
            (120.0, 120.0),
            (1.5, 2.5),
            [120.0, 120.0, 120.0, 120.0, 100.0],
        ),
        (
            "plasma grenade",
            80.0,
            (120.0, 120.0),
            (0.5, 2.0),
            [120.0, 120.0, 106.667, 93.333, -1.0],
        ),
        (
            "tank shell",
            80.0,
            (351.0, 351.0),
            (1.5, 3.25),
            [351.0, 351.0, 351.0, 351.0, 273.571],
        ),
        (
            "needler super",
            40.0,
            (60.0, 60.0),
            (0.5, 1.0),
            [60.0, 60.0, -1.0, -1.0, -1.0],
        ),
    ];
    for (name, lower, upper, radii, amounts) in rows {
        let d = effect(row(lower, upper, radii, 0, &[]));
        for (d_wu, want) in [0.0, 0.5, 1.0, 1.5, 2.0].into_iter().zip(amounts) {
            let got = d.area_amount(d_wu, 1.0);
            if want < 0.0 {
                assert_eq!(got, 0.0, "{name} at {d_wu}");
            } else {
                assert!((got - want).abs() < 2e-3, "{name} at {d_wu}: {got} vs {want}");
            }
        }
    }
    // Scales: plasma grenade 0.66667 at 1.0, 0.33333 at 1.5, 0 at 2.0; frag
    // 0.5 at 2.0.
    let pg = effect(row(80.0, (120.0, 120.0), (0.5, 2.0), 0, &[]));
    assert!((pg.area_scale(1.0) - 0.66667).abs() < 1e-5);
    assert!((pg.area_scale(1.5) - 0.33333).abs() < 1e-5);
    assert_eq!(pg.area_scale(2.0), 0.0);
    assert_eq!(pg.area_scale(0.0), 1.0);
    let frag = effect(row(80.0, (120.0, 120.0), (1.5, 2.5), 0, &[]));
    assert!((frag.area_scale(2.0) - 0.5).abs() < 1e-6);
    assert_eq!(frag.area_amount(2.5, 1.0), 0.0);
    assert_eq!(frag.area_amount(9.0, 1.0), 0.0);
    // Rocket (0.5 - 2): 300 to 330 at the centre, 226.667 to 246.667 at
    // 1.0, 153.333 to 163.333 at 1.5, none at 2.
    let rk = effect(row(80.0, (300.0, 330.0), (0.5, 2.0), 0, &[]));
    assert_eq!((rk.area_amount(0.0, 0.0), rk.area_amount(0.0, 1.0)), (300.0, 330.0));
    assert!((rk.area_amount(1.0, 0.0) - 226.667).abs() < 1e-2);
    assert!((rk.area_amount(1.0, 1.0) - 246.667).abs() < 1e-2);
    assert!((rk.area_amount(1.5, 0.0) - 153.333).abs() < 1e-2);
    assert!((rk.area_amount(1.5, 1.0) - 163.333).abs() < 1e-2);
    assert_eq!(rk.area_amount(2.0, 1.0), 0.0);
}

#[test]
fn melee_effects_do_not_scale_by_distance() {
    let melee = effect(row(40.0, (50.0, 60.0), (0.5, 0.5), 0x1, &[]));
    assert_eq!(melee.area_event_scale(0.4, 1.5), 1.5);
    assert_eq!(melee.area_event_scale(0.4, 0.0), 0.0);
    let frag = effect(row(80.0, (120.0, 120.0), (1.5, 2.5), 0, &[]));
    assert!((frag.area_event_scale(2.0, 0.0) - 0.5).abs() < 1e-6);
    // Equal radii (or a lower bound over the upper): scale 1 inside the
    // cutoff, none at it.
    assert_eq!(melee.area_scale(0.2), 1.0);
    assert_eq!(melee.area_amount(0.5, 1.0), 0.0);
    assert_eq!(melee.area_amount(0.2, 0.0), 50.0);
}
