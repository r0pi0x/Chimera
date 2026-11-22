//! The front end's plain functions: the motion tracker's sweep at the
//! frame's own time (`hud::rules`; Halo's motion drawn at the frame rate)
//! and an effect light's intensity by its fall-off (`fx_lights`).

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

