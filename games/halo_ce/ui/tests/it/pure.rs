//! The front end's plain functions: the motion tracker's sweep at the
//! frame's own time (`hud::rules`; Halo's motion drawn at the frame
//! rate), an effect light's intensity by its fall-off (`fx_lights`), a hit's
//! screen flash as the frame's light (`victim_view`) and the damage
//! arrows' "always point down" flag (`hud::nodes::points_down`).

/// The tracker's sweep at the frame's time: between its ticks' sizes.
#[test]
fn the_trackers_sweep_moves_between_ticks() {
    use halo_ce_ui::hud::rules::{sweep, sweep_at};
    let (a, b) = (sweep(10), sweep(11));
    let m = sweep_at(10.5);
    assert!(m < a.max(b) && m > a.min(b), "{a} {m} {b}");
    assert_eq!(sweep_at(10.0), a);
}

/// effects.md "Effect lights": intensity = 1 - the fall-off function of the
/// elapsed fraction (the frag's linear, the plasma grenade's late, the
/// tank shell's early).
#[test]
fn an_effect_lights_intensity_follows_its_falloff_function() {
    use halo_ce_ui::fx_lights::intensity;
    assert!((intensity(0, 0.5) - 0.5).abs() < 1e-6);
    assert!((intensity(3, 0.5) - 0.75).abs() < 1e-6, "late: stays bright");
    assert!((intensity(1, 0.25) - 0.5).abs() < 1e-6, "early: drops fast");
    assert_eq!((intensity(0, 0.0), intensity(0, 1.0)), (1.0, 0.0));
}


/// A flash as the frame's light (scale, add): lighten adds, darken
/// multiplies toward the colour, tint mixes; nothing without one.
#[test]
fn a_flash_lights_the_frame_by_its_type() {
    use bevy::math::Vec3;
    use halo_ce::game::fx::victim::ShownFlash;
    use halo_ce_ui::victim_view::flash_light;
    let f = |kind| ShownFlash { kind, color: [1.0, 0.5, 0.0, 1.0], intensity: 0.5 };
    assert_eq!(flash_light(None), (Vec3::ONE, Vec3::ZERO));
    assert_eq!(flash_light(Some(&f(1))), (Vec3::ONE, Vec3::new(0.5, 0.25, 0.0)));
    assert_eq!(flash_light(Some(&f(2))), (Vec3::new(1.0, 0.75, 0.5), Vec3::ZERO));
    assert_eq!(flash_light(Some(&f(6))), (Vec3::splat(0.5), Vec3::new(0.5, 0.25, 0.0)));
}

/// hud.md 12: a damage effect with "indicators always point down" (flag
/// bit 8) sets only the bottom arrow; the disc's effects have none.
#[test]
fn an_always_down_effect_points_the_arrows_down() {
    use halo_ce::game::{
        fx::victim::HaloVictim,
        vitality::{DamageEffects, data::flags::DAMAGE_INDICATORS_ALWAYS_POINT_DOWN, ids},
    };
    use halo_ce_ui::hud::{nodes::points_down, rules::arrow_sides};
    let mut effects = DamageEffects::load();
    let i = effects.index_of(ids::FALLING.0).unwrap();
    let hit = HaloVictim { effect: i, scale: 1.0, dir: bevy::math::Vec3::NEG_X, directed: true };
    assert!(!points_down(&hit, &effects));
    effects.0.get_mut(ids::FALLING.0).unwrap().flags |= DAMAGE_INDICATORS_ALWAYS_POINT_DOWN;
    assert!(points_down(&hit, &effects));
    assert!(DamageEffects::load().0.values().all(|e| !e.has(DAMAGE_INDICATORS_ALWAYS_POINT_DOWN)));
    // From the left, pointing down: the bottom alone.
    assert_eq!(arrow_sides([-1.0, 0.0, 0.0], true), [false, false, true, false]);
}
