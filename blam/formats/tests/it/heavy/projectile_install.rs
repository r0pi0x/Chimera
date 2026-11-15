//! The projectile and damage effect readers on the user's Halo disc image,
//! against "What is on the disc" of specs/halo_ce/projectiles.md: the flight
//! numbers of the 16 projectile tags, their references, flags, noises and
//! timers, the material responses the spec summarises, the damage effects
//! table (category, side effect, flags, bounds, radii, core, camouflage,
//! stun, acceleration, the modifiers of cyborg and cyborg shield), the full
//! per-material modifier lists, a sweep of every projectile and damage
//! effect of `bloodgulch` (references resolve to plausible classes,
//! enumerations in range, unused slots zero) and the same values on other
//! multiplayer maps. Each test passes with a note when there is no image.

// The spec's numbers are written as it states them, rounded.
#![allow(clippy::approx_constant, clippy::excessive_precision, clippy::type_complexity)]

use blam_formats::cache::{MapFile, TagRef, class};
use blam_formats::collision::MaterialType;
use blam_formats::damage_effect::{DamageCategory, DamageEffect, DamageFlags, SideEffect};
use blam_formats::projectile::{Projectile, ProjectileFlags, ResponseType, TimerStarts};

use super::maps::{MULTIPLAYER, load};
use super::object_install::{normalized, path_of};

const PROJECTILES: [&str; 16] = [
    "weapons\\assault rifle\\bullet",
    "weapons\\pistol\\bullet",
    "weapons\\shotgun\\pellet",
    "weapons\\sniper rifle\\sniper bullet",
    "weapons\\rocket launcher\\rocket",
    "weapons\\plasma pistol\\bolt",
    "weapons\\plasma rifle\\charged bolt",
    "weapons\\plasma rifle\\bolt",
    "weapons\\needler\\needle",
    "weapons\\flamethrower\\flame",
    "weapons\\frag grenade\\frag grenade",
    "weapons\\plasma grenade\\plasma grenade",
    "vehicles\\warthog\\bullet",
    "vehicles\\scorpion\\tank shell",
    "vehicles\\scorpion\\bullet",
    "vehicles\\ghost\\ghost bolt",
];

/// `a` is `b` to the digits the spec prints (relative 5e-4, or 1e-6 absolute
/// near 0).
fn close(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() <= 5e-4 * b.abs() + 1e-6, "{what}: {a} vs {b}");
}

fn proj(map: &MapFile, path: &str) -> Projectile {
    Projectile::read(
        map,
        map.find(class::PROJECTILE, path).unwrap_or_else(|| panic!("no {path}")),
    )
    .unwrap()
}

fn dmg(map: &MapFile, path: &str) -> DamageEffect {
    DamageEffect::read(
        map,
        map.find(class::DAMAGE_EFFECT, path)
            .unwrap_or_else(|| panic!("no {path}")),
    )
    .unwrap()
}

fn p(map: &MapFile, r: &TagRef) -> Option<String> {
    path_of(map, r)
}

fn some(s: &str) -> Option<String> {
    Some(s.to_string())
}

#[test]
fn sixteen_projectiles() {
    let Some(map) = load("bloodgulch") else { return };
    assert_eq!(map.of_class(class::PROJECTILE).count(), 16);
    for path in PROJECTILES {
        let t = proj(&map, path);
        assert_eq!(t.object.object_type.code(), 5, "{path}");
    }
}

/// The "Flight numbers" table.
#[test]
fn flight_numbers() {
    let Some(map) = load("bloodgulch") else { return };
    // initial, final, air range, a, max range, gravity air / water, collision
    // radius, timer (s; None: no timer), guided, minimum velocity.
    #[rustfmt::skip]
    let rows: [(&str, f32, f32, (f32, f32), f32, f32, (f32, f32), f32, Option<f32>, f32, f32); 16] = [
        ("weapons\\assault rifle\\bullet", 10.8, 10.8, (5.0, 15.0), 0.0, 40.0, (1.0, 0.4), 0.0, None, 0.0, 0.0),
        ("weapons\\pistol\\bullet", 10.0, 10.0, (0.0, 100.0), 0.0, 40.0, (1.0, 0.4), 0.0, None, 0.0, 0.0),
        ("weapons\\shotgun\\pellet", 4.6667, 3.3333, (1.5, 3.0), 3.5555552, 40.0, (1.0, 0.4), 0.0, None, 0.0, 0.0),
        ("weapons\\sniper rifle\\sniper bullet", 33.3333, 33.3333, (1000.0, 1000.0), 0.0, 1000.0, (0.0, 0.0), 0.0, None, 0.0, 0.0),
        ("weapons\\rocket launcher\\rocket", 0.4, 0.3333, (0.0, 100.0), 0.0002444, 128.0, (0.0, 0.4), 0.1, None, 0.0, 0.0),
        ("weapons\\plasma pistol\\bolt", 0.8333, 0.5, (20.0, 50.0), 0.0074074, 50.0, (0.1, 0.0), 0.0, None, 0.0, 0.0),
        ("weapons\\plasma rifle\\charged bolt", 0.5, 0.3, (5.0, 20.0), 0.0053333, 40.0, (0.0, 0.0), 0.1, None, 0.506145, 0.0),
        ("weapons\\plasma rifle\\bolt", 1.6667, 0.8333, (20.0, 50.0), 0.0347222, 0.0, (0.0, 0.0), 0.0, None, 0.0, 0.0),
        ("weapons\\needler\\needle", 0.1333, 0.1333, (0.0, 100.0), 0.0, 20.0, (0.0, 0.0), 0.0, Some(0.75), 1.5708, 0.0667),
        ("weapons\\flamethrower\\flame", 0.3333, 0.3333, (0.0, 0.0), 0.0, 2.5, (0.0, 0.0), 0.1, None, 0.0, 0.0),
        ("weapons\\frag grenade\\frag grenade", 0.0, 0.0, (0.0, 10.0), 0.0, 0.0, (1.0, 1.0), 0.0, Some(0.5), 0.0, 0.0),
        ("weapons\\plasma grenade\\plasma grenade", 0.0, 0.0, (0.0, 10.0), 0.0, 0.0, (1.0, 1.0), 0.0, Some(2.0), 0.0, 0.0),
        ("vehicles\\warthog\\bullet", 9.7667, 0.3333, (0.0, 100.0), 0.4763834, 0.0, (1.0, 0.4), 0.0, None, 0.0, 0.0),
        ("vehicles\\scorpion\\tank shell", 3.3333, 3.3333, (1000.0, 1000.0), 0.0, 1000.0, (0.0, 0.0), 0.0, None, 0.0, 0.0),
        ("vehicles\\scorpion\\bullet", 10.8, 10.8, (5.0, 15.0), 0.0, 40.0, (1.0, 0.4), 0.0, None, 0.0, 0.0),
        ("vehicles\\ghost\\ghost bolt", 1.6667, 0.8333, (20.0, 50.0), 0.0347222, 50.0, (0.0, 0.0), 0.0, None, 0.0, 0.0),
    ];
    for (path, v0, vf, air, a, range, (ga, gw), radius, timer, guided, minv) in rows {
        let o = proj(&map, path).own;
        close(o.initial_velocity, v0, path);
        close(o.final_velocity, vf, path);
        close(o.air_damage_range.lower, air.0, path);
        close(o.air_damage_range.upper, air.1, path);
        close(o.air_deceleration(), a, &format!("{path} deceleration"));
        close(o.maximum_range, range, path);
        close(o.air_gravity_scale, ga, path);
        close(o.water_gravity_scale, gw, path);
        close(o.collision_radius, radius, path);
        match timer {
            Some(t) => {
                assert!(o.has_timer(), "{path}");
                close(o.detonation_timer.upper, t, path);
            }
            None => assert!(!o.has_timer(), "{path}"),
        }
        close(o.guided_angular_velocity, guided, path);
        close(o.minimum_velocity, minv, path);
        // "Decel. from" 4 for the pellet (1.5 / 4.6667 timer steps: the lower
        // bound over the initial velocity), active at once (0) with no lower
        // bound.
        if air.0 == 0.0 {
            assert_eq!(o.deceleration_step(o.air_damage_range), 0.0, "{path}");
        }
    }
    // The scale of a hit: the start speed is 1, the final speed 0.
    let o = proj(&map, "vehicles\\warthog\\bullet").own;
    assert_eq!(o.direct_hit_scale(o.initial_velocity), 1.0);
    assert_eq!(o.direct_hit_scale(o.final_velocity), 0.0);
}

#[test]
fn water_ranges_timers_noises_radii_and_flags() {
    let Some(map) = load("bloodgulch") else { return };
    // Water ranges.
    let water = [
        ("weapons\\assault rifle\\bullet", (0.0, 5.0)),
        ("vehicles\\scorpion\\bullet", (0.0, 5.0)),
        ("weapons\\pistol\\bullet", (0.0, 10.0)),
        ("weapons\\rocket launcher\\rocket", (0.0, 10.0)),
        ("weapons\\needler\\needle", (0.0, 10.0)),
        ("vehicles\\warthog\\bullet", (0.0, 10.0)),
        ("weapons\\sniper rifle\\sniper bullet", (0.0, 10.0)),
        ("vehicles\\scorpion\\tank shell", (0.0, 10.0)),
        ("weapons\\shotgun\\pellet", (0.0, 1.0)),
        ("weapons\\plasma rifle\\bolt", (20.0, 50.0)),
        ("vehicles\\ghost\\ghost bolt", (20.0, 50.0)),
        ("weapons\\plasma pistol\\bolt", (0.0, 0.0)),
        ("weapons\\plasma rifle\\charged bolt", (0.0, 0.0)),
        ("weapons\\flamethrower\\flame", (0.0, 0.0)),
        ("weapons\\frag grenade\\frag grenade", (0.0, 10.0)),
        ("weapons\\plasma grenade\\plasma grenade", (0.0, 10.0)),
    ];
    for (path, (lo, hi)) in water {
        let o = proj(&map, path).own;
        assert_eq!(
            (o.water_damage_range.lower, o.water_damage_range.upper),
            (lo, hi),
            "{path}"
        );
    }
    // Timer-starts values: 1 for the AR, pistol, pellet, rocket, sniper and
    // Scorpion bullets and the frag; 2 for the needle and plasma grenade; 0
    // for the rest.
    for path in PROJECTILES {
        let want = match path {
            "weapons\\assault rifle\\bullet"
            | "weapons\\pistol\\bullet"
            | "weapons\\shotgun\\pellet"
            | "weapons\\rocket launcher\\rocket"
            | "weapons\\sniper rifle\\sniper bullet"
            | "vehicles\\scorpion\\bullet"
            | "weapons\\frag grenade\\frag grenade" => TimerStarts::AfterFirstBounce,
            "weapons\\needler\\needle" | "weapons\\plasma grenade\\plasma grenade" => TimerStarts::WhenAtRest,
            _ => TimerStarts::Immediately,
        };
        let o = proj(&map, path).own;
        assert_eq!(o.detonation_timer_starts, want, "{path}");
        // Noises (impact, detonation): pistol 1, 0; pellet 1, 0; sniper 1, 0;
        // rocket 0, 2; tank shell 0, 2; frag 4, 2; plasma grenade 1, 2;
        // needle 4, 1; others 0, 0.
        let noise = match path {
            "weapons\\pistol\\bullet" | "weapons\\shotgun\\pellet" | "weapons\\sniper rifle\\sniper bullet" => (1, 0),
            "weapons\\rocket launcher\\rocket" | "vehicles\\scorpion\\tank shell" => (0, 2),
            "weapons\\frag grenade\\frag grenade" => (4, 2),
            "weapons\\plasma grenade\\plasma grenade" => (1, 2),
            "weapons\\needler\\needle" => (4, 1),
            _ => (0, 0),
        };
        assert_eq!((o.impact_noise.code(), o.detonation_noise.code()), noise, "{path}");
        // AI radii: 0 except the grenades (perception 8, danger 2.25); arming
        // 0 except the frag's 1.5.
        let grenade = path.contains("grenade\\") && path.rsplit('\\').next().unwrap().ends_with("grenade");
        assert_eq!(o.ai_perception_radius, if grenade { 8.0 } else { 0.0 }, "{path}");
        assert_eq!(o.danger_radius, if grenade { 2.25 } else { 0.0 }, "{path}");
        assert_eq!(o.arming_time, if path.contains("frag") { 1.5 } else { 0.0 }, "{path}");
        // Flags: 2 frag, 6 plasma grenade, 11 needle, 1 charged bolt, else 0.
        let flags = match path {
            "weapons\\frag grenade\\frag grenade" => 2,
            "weapons\\plasma grenade\\plasma grenade" => 6,
            "weapons\\needler\\needle" => 11,
            "weapons\\plasma rifle\\charged bolt" => 1,
            _ => 0,
        };
        assert_eq!(o.flags.bits(), flags, "{path}");
        assert_eq!(o.flags.unnamed_bits(), 0);
    }
    let needle = proj(&map, "weapons\\needler\\needle").own;
    assert!(needle.flags.contains(ProjectileFlags::HAS_SUPER_COMBINING_EXPLOSION));
    let plasma = proj(&map, "weapons\\plasma grenade\\plasma grenade").own;
    assert!(plasma.flags.contains(ProjectileFlags::DETONATION_MAX_TIME_IF_ATTACHED));
    // The timer steps of the frag and the plasma grenade: 0.5 s and 2.0 s.
    assert_eq!(
        proj(&map, "weapons\\frag grenade\\frag grenade")
            .own
            .timer_ticks()
            .upper,
        15
    );
    assert_eq!(plasma.timer_ticks().upper, 60);
}

#[test]
fn references() {
    let Some(map) = load("bloodgulch") else { return };
    for path in PROJECTILES {
        let o = proj(&map, path).own;
        // Impact damage: the damage effect of the same path, the needle's own,
        // none for the rocket, flame, tank shell and grenades.
        let impact = match path {
            "weapons\\rocket launcher\\rocket"
            | "weapons\\flamethrower\\flame"
            | "vehicles\\scorpion\\tank shell"
            | "weapons\\frag grenade\\frag grenade"
            | "weapons\\plasma grenade\\plasma grenade" => None,
            "weapons\\needler\\needle" => some("weapons\\needler\\impact damage"),
            _ => some(path),
        };
        assert_eq!(p(&map, &o.impact_damage), impact, "{path} impact damage");
        let attached = match path {
            "weapons\\needler\\needle" => some("weapons\\needler\\detonation damage"),
            "weapons\\plasma grenade\\plasma grenade" => some("weapons\\plasma grenade\\attached"),
            _ => None,
        };
        assert_eq!(p(&map, &o.attached_detonation_damage), attached, "{path} attached");
        let effect = match path {
            "weapons\\rocket launcher\\rocket" => some("weapons\\rocket launcher\\effects\\rocket explosion"),
            "weapons\\flamethrower\\flame" => some("weapons\\flamethrower\\effects\\flame detonation"),
            "vehicles\\scorpion\\tank shell" => some("vehicles\\scorpion\\shell explosion"),
            "weapons\\frag grenade\\frag grenade" => some("weapons\\frag grenade\\effects\\explosion"),
            "weapons\\plasma grenade\\plasma grenade" => some("weapons\\plasma grenade\\effects\\explosion"),
            "weapons\\needler\\needle" => some("weapons\\needler\\effects\\needle detonate"),
            "weapons\\plasma rifle\\charged bolt" => some("weapons\\plasma rifle\\effects\\detonate"),
            _ => None,
        };
        assert_eq!(p(&map, &o.effect), effect, "{path} effect");
        let sup = (path == "weapons\\needler\\needle").then(|| "weapons\\needler\\effects\\explosion".to_string());
        assert_eq!(p(&map, &o.super_detonation), sup, "{path} super detonation");
        let started = (path == "weapons\\plasma grenade\\plasma grenade")
            .then(|| "weapons\\plasma grenade\\effects\\detonation start".to_string());
        assert_eq!(p(&map, &o.detonation_started), started, "{path} detonation started");
        let flyby = match path {
            "weapons\\assault rifle\\bullet"
            | "weapons\\pistol\\bullet"
            | "weapons\\sniper rifle\\sniper bullet"
            | "vehicles\\scorpion\\bullet" => some("sound\\sfx\\impulse\\impacts\\bullets_projectile"),
            "weapons\\plasma rifle\\bolt" | "weapons\\plasma pistol\\bolt" | "weapons\\plasma rifle\\charged bolt" => {
                some("sound\\sfx\\impulse\\impacts\\plasrif_projectile")
            }
            "weapons\\needler\\needle" => some("sound\\sfx\\impulse\\impacts\\needler_projectile"),
            "vehicles\\scorpion\\tank shell" => some("sound\\sfx\\impulse\\impacts\\scorpion_projectile"),
            _ => None,
        };
        assert_eq!(p(&map, &o.flyby_sound), flyby, "{path} flyby");
    }
}

/// The 33 material types as the damage and response tables name them.
fn ty(code: usize) -> MaterialType {
    MaterialType::ALL[code]
}

fn resp(map: &MapFile, path: &str, material: usize) -> blam_formats::projectile::MaterialResponse {
    proj(map, path).own.response(ty(material)).unwrap().clone()
}

#[test]
fn material_response_blocks_and_summary() {
    let Some(map) = load("bloodgulch") else { return };
    // Every block has 33 elements except the flame's, which has 31.
    for path in PROJECTILES {
        let o = proj(&map, path).own;
        let want = if path == "weapons\\flamethrower\\flame" { 31 } else { 33 };
        assert_eq!(o.material_responses.len(), want, "{path}");
        for r in &o.material_responses {
            assert_eq!(r.flags.bits(), 0, "{path}");
        }
    }
    let flame = proj(&map, "weapons\\flamethrower\\flame").own;
    for m in [MaterialType::Ice, MaterialType::HunterShield] {
        assert!(flame.response(m).is_none());
        assert_eq!(
            flame.response_or_built_in(Some(m)).default_response,
            ResponseType::Disappear
        );
    }
    use ResponseType::{Attach, Detonate, Disappear, Overpenetrate, Reflect};
    let dflt = |path: &str, m: usize| resp(&map, path, m).default_response;

    // AR bullet and Scorpion bullet.
    for path in ["weapons\\assault rifle\\bullet", "vehicles\\scorpion\\bullet"] {
        for m in [0, 2, 5, 6, 7, 22] {
            assert_eq!(dflt(path, m), Disappear, "{path} {m}");
        }
        // The cyborg body and the organic bodies pass at 30 % (initial
        // friction 0.7); flood combat, water and leaves at full speed.
        for m in [21, 11, 13, 14, 15, 17, 23, 24] {
            let r = resp(&map, path, m);
            assert_eq!(r.default_response, Overpenetrate, "{path} {m}");
            close(r.initial_friction, 0.7, path);
        }
        for m in [19, 28, 29] {
            let r = resp(&map, path, m);
            assert_eq!(r.default_response, Overpenetrate, "{path} {m}");
            assert_eq!(r.initial_friction, 0.0, "{path} {m}");
        }
        for m in [16, 32] {
            assert_eq!(dflt(path, m), Reflect, "{path} {m}");
        }
    }
    assert_eq!(dflt("weapons\\assault rifle\\bullet", 9), Overpenetrate);
    assert_eq!(resp(&map, "weapons\\assault rifle\\bullet", 9).initial_friction, 0.0);

    // Pistol bullet: everything disappears but glass and leaves, and the
    // hunter shield reflects.
    let pistol = "weapons\\pistol\\bullet";
    for m in [0, 2, 5, 6, 7, 21, 22] {
        assert_eq!(dflt(pistol, m), Disappear, "{m}");
    }
    assert_eq!(
        (dflt(pistol, 9), dflt(pistol, 29), dflt(pistol, 32)),
        (Overpenetrate, Overpenetrate, Reflect)
    );

    // Shotgun pellet: glass, water, leaves pass; jackal and hunter shields
    // reflect.
    let pellet = "weapons\\shotgun\\pellet";
    for m in [0, 2, 5, 6, 7, 21, 22] {
        assert_eq!(dflt(pellet, m), Disappear, "{m}");
    }
    for m in [9, 28, 29] {
        assert_eq!(dflt(pellet, m), Overpenetrate, "{m}");
    }
    assert_eq!((dflt(pellet, 16), dflt(pellet, 32)), (Reflect, Reflect));

    // Sniper bullet: bodies, the elite shield, glass, water and leaves pass at
    // full speed.
    let sniper = "weapons\\sniper rifle\\sniper bullet";
    for m in [0, 2, 5, 6, 7, 22] {
        assert_eq!(dflt(sniper, m), Disappear, "{m}");
    }
    for m in [21, 11, 13, 14, 15, 17, 23, 24, 30, 9, 28, 29] {
        let r = resp(&map, sniper, m);
        assert_eq!(r.default_response, Overpenetrate, "{m}");
        assert_eq!(r.initial_friction, 0.0, "{m}");
    }
    assert_eq!((dflt(sniper, 16), dflt(sniper, 32)), (Reflect, Reflect));

    // Rocket and tank shell: everything detonates; leaves pass with a 10 %
    // detonate (skip 0.9).
    for path in ["weapons\\rocket launcher\\rocket", "vehicles\\scorpion\\tank shell"] {
        for m in [0, 2, 5, 6, 7, 21, 22] {
            assert_eq!(dflt(path, m), Detonate, "{path} {m}");
        }
    }
    let leaves = resp(&map, "weapons\\rocket launcher\\rocket", 29);
    assert_eq!(
        (leaves.default_response, leaves.potential_response),
        (Overpenetrate, Detonate)
    );
    close(leaves.skip_fraction, 0.9, "rocket leaves");

    // Plasma bolts disappear, the hunter shield reflects; leaves pass for the
    // rifle and ghost bolts.
    for path in [
        "weapons\\plasma pistol\\bolt",
        "weapons\\plasma rifle\\bolt",
        "vehicles\\ghost\\ghost bolt",
    ] {
        for m in [0, 2, 5, 6, 7, 21, 22] {
            assert_eq!(dflt(path, m), Disappear, "{path} {m}");
        }
        assert_eq!(dflt(path, 32), Reflect, "{path}");
    }
    for path in ["weapons\\plasma rifle\\bolt", "vehicles\\ghost\\ghost bolt"] {
        assert_eq!(dflt(path, 29), Overpenetrate, "{path}");
    }

    // Charged bolt: on the disc every element's default response is
    // disappear with an unconditional potential detonate (no angle or
    // velocity condition, nothing skipped), which is the table's "detonate";
    // leaves pass (a potential overpenetrate).
    let charged = "weapons\\plasma rifle\\charged bolt";
    for m in [0, 2, 5, 6, 7, 21, 22] {
        let r = resp(&map, charged, m);
        assert_eq!((r.default_response, r.potential_response), (Disappear, Detonate), "{m}");
        assert_eq!(
            (r.between_angle.upper, r.and_velocity.upper, r.skip_fraction),
            (0.0, 0.0, 0.0),
            "{m}"
        );
        assert!(r.potential_applies(0.0, 0.0, false), "{m}");
    }
    assert_eq!(resp(&map, charged, 29).potential_response, Overpenetrate);

    // Needle: attach everywhere that matters; stone reflects at a grazing
    // angle (0.4363 with noise 0.2618, frictions 0.3/0.3); the metals up to
    // 0.6109 (noise 0.0873); glass up to 15 degrees.
    let needle = "weapons\\needler\\needle";
    for m in [0, 2, 5, 6, 7, 21, 22] {
        assert_eq!(dflt(needle, m), Attach, "{m}");
    }
    let stone = resp(&map, needle, 2);
    assert_eq!(stone.potential_response, Reflect);
    close(stone.between_angle.upper, 0.4363, "needle stone angle");
    close(stone.angular_noise, 0.2618, "needle stone noise");
    close(stone.parallel_friction, 0.3, "needle stone friction");
    close(stone.perpendicular_friction, 0.3, "needle stone friction");
    for m in [5, 6, 7] {
        let r = resp(&map, needle, m);
        assert_eq!(r.potential_response, Reflect, "{m}");
        close(r.between_angle.upper, 0.6109, "needle metal angle");
        close(r.angular_noise, 0.0873, "needle metal noise");
        close(r.parallel_friction, 0.3, "needle metal friction");
    }
    let glass = resp(&map, needle, 9);
    assert!(glass.has_potential() || glass.default_response == Reflect);
    for m in [10, 16, 18, 32] {
        assert_eq!(dflt(needle, m), Reflect, "{m}");
    }
    for m in [28, 29] {
        assert_eq!(dflt(needle, m), Overpenetrate, "{m}");
    }

    // Frag: reflect with the frictions (parallel/perpendicular): dirt
    // 0.8/0.9, stone 0.7/0.8, cyborg 0.8/0.9, shield 0.8/0.9, metals 0.7/0.8,
    // snow 0.9/0.95, force field 0.6/0.7, jackal shield 0.8/0.8, ice 0.55/0.65.
    let frag = "weapons\\frag grenade\\frag grenade";
    for (m, par, per) in [
        (0, 0.8, 0.9),
        (2, 0.7, 0.8),
        (21, 0.8, 0.9),
        (22, 0.8, 0.9),
        (5, 0.7, 0.8),
        (6, 0.7, 0.8),
        (7, 0.7, 0.8),
        (3, 0.9, 0.95),
        (10, 0.6, 0.7),
        (16, 0.8, 0.8),
        (31, 0.55, 0.65),
    ] {
        let r = resp(&map, frag, m);
        assert_eq!(r.default_response, Reflect, "{m}");
        close(r.parallel_friction, par, &format!("frag {m}"));
        close(r.perpendicular_friction, per, &format!("frag {m}"));
    }
    // Water: pass, a potential skimming reflect 0.7/0.7 (angle <= 0.5236,
    // 0.0333 <= u <= 33.3).
    let water = resp(&map, frag, 28);
    assert_eq!(
        (water.default_response, water.potential_response),
        (Overpenetrate, Reflect)
    );
    close(water.between_angle.upper, 0.5236, "frag water angle");
    close(water.and_velocity.lower, 0.033333, "frag water velocity");
    close(water.and_velocity.upper, 33.333333, "frag water velocity");
    close(water.parallel_friction, 0.7, "frag water");
    // Leaves: reflect, a potential pass (skip 0.2).
    let leaves = resp(&map, frag, 29);
    assert_eq!(
        (leaves.default_response, leaves.potential_response),
        (Reflect, Overpenetrate)
    );
    close(leaves.skip_fraction, 0.2, "frag leaves");

    // Plasma grenade: reflect 0.9/0.95 on dirt and stone with a potential
    // attach against units only; the cyborg, its shield and the metals
    // reflect with a potential attach; glass and force field 0.7/0.8 with no
    // stick.
    let plasma = "weapons\\plasma grenade\\plasma grenade";
    for m in [0, 2] {
        let r = resp(&map, plasma, m);
        assert_eq!(r.default_response, Reflect, "{m}");
        close(r.parallel_friction, 0.9, "plasma");
        close(r.perpendicular_friction, 0.95, "plasma");
        assert_eq!(r.potential_response, Attach, "{m}");
    }
    for m in [21, 22, 5, 6, 7] {
        let r = resp(&map, plasma, m);
        assert_eq!((r.default_response, r.potential_response), (Reflect, Attach), "{m}");
        assert!(
            r.potential_flags
                .contains(blam_formats::projectile::PotentialFlags::ONLY_AGAINST_UNITS),
            "{m}"
        );
    }
    for m in [9, 10] {
        let r = resp(&map, plasma, m);
        assert_eq!((r.default_response, r.potential_response), (Reflect, Disappear), "{m}");
        close(r.parallel_friction, 0.7, "plasma glass");
        close(r.perpendicular_friction, 0.8, "plasma glass");
    }

    // Flame: disappear on every element.
    for m in 0..31 {
        assert_eq!(
            resp(&map, "weapons\\flamethrower\\flame", m).default_response,
            Disappear,
            "{m}"
        );
    }

    // Warthog bullet: dirt disappear; the cyborg, hollow and thin metals and
    // wood pass at 30 %; the shield and thick metal disappear; the jackal
    // shield reflects.
    let hog = "vehicles\\warthog\\bullet";
    for m in [0, 22, 7] {
        assert_eq!(dflt(hog, m), Disappear, "{m}");
    }
    for m in [21, 5, 6, 4] {
        let r = resp(&map, hog, m);
        assert_eq!(r.default_response, Overpenetrate, "{m}");
        close(r.initial_friction, 0.7, "warthog");
    }
    assert_eq!(dflt(hog, 16), Reflect);

    // The impact effect scale and the reported angle noise exist on every
    // element; the response lookup by the "none" material is the built-in one.
    let o = proj(&map, hog).own;
    assert_eq!(o.response_or_built_in(None).default_response, Disappear);
}

/// One damage effect row of the "Damage effects" table.
struct Row {
    path: &'static str,
    category: i16,
    side: i16,
    flags: u32,
    lower: f32,
    upper: (f32, f32),
    radii: (f32, f32),
    core: f32,
    camo: f32,
    stun: (f32, f32, f32),
    accel: f32,
    mods: (f32, f32),
}

#[rustfmt::skip]
fn rows() -> Vec<Row> {
    let r = |path, category, side, flags, lower, upper, radii, core, camo, stun, accel, mods| Row {
        path, category, side, flags, lower, upper, radii, core, camo, stun, accel, mods,
    };
    const Z: (f32, f32, f32) = (0.0, 0.0, 0.0);
    let melee = |path| r(path, 6, 2, 0x5, 40.0, (50.0, 60.0), (0.5, 0.5), 0.0, 0.0, Z, 0.0, (1.4, 1.4));
    vec![
        r("weapons\\assault rifle\\bullet", 2, 0, 0, 10.0, (10.0, 10.0), (0.0, 0.0), 0.0, 0.25, Z, 0.0, (1.0, 1.0)),
        r("weapons\\pistol\\bullet", 2, 0, 0x2, 25.0, (25.0, 25.0), (0.0, 0.0), 0.0, 0.45, Z, 0.0, (1.5, 1.0)),
        r("weapons\\shotgun\\pellet", 12, 0, 0, 8.0, (18.0, 25.0), (0.0, 0.0), 0.0, 0.18, Z, 0.0, (1.0, 0.5)),
        r("weapons\\sniper rifle\\sniper bullet", 5, 0, 0x2, 101.0, (101.0, 101.0), (0.0, 0.0), 0.0, 1.0, Z, 0.0, (1.0, 1.0)),
        r("weapons\\plasma rifle\\bolt", 10, 0, 0x804, 10.0, (12.0, 14.0), (0.0, 0.0), 0.0, 0.12, (1.0, 1.0, 0.15), 0.0, (0.5, 2.0)),
        r("weapons\\plasma pistol\\bolt", 10, 0, 0x4, 10.0, (16.0, 20.0), (0.0, 0.0), 0.0, 0.12, (1.0, 1.0, 0.15), 0.0, (0.6, 0.6)),
        r("weapons\\plasma rifle\\charged bolt", 10, 3, 0x4, 70.0, (70.0, 70.0), (0.0, 0.0), 0.0, 1.0, Z, 0.0, (0.6, 1.0)),
        r("weapons\\needler\\impact damage", 2, 0, 0, 0.0, (0.0, 0.0), (0.0, 0.0), 0.0, 0.15, Z, 0.0, (0.0, 0.0)),
        r("weapons\\needler\\detonation damage", 2, 0, 0, 10.0, (10.0, 10.0), (0.0, 0.0), 0.0, 0.1, Z, 0.0, (1.0, 1.0)),
        r("weapons\\needler\\explosion", 3, 3, 0x20, 40.0, (60.0, 60.0), (0.5, 1.0), 0.0, 1.0, Z, 2.0, (1.0, 1.0)),
        r("weapons\\needler\\shock wave", 0, 0, 0, 0.0, (0.0, 0.0), (2.0, 4.0), 0.0, 0.0, Z, 0.0, (0.0, 0.0)),
        r("weapons\\frag grenade\\explosion", 3, 0, 0x20, 80.0, (120.0, 120.0), (1.5, 2.5), 0.4, 0.6, Z, 4.0, (1.0, 1.0)),
        r("weapons\\frag grenade\\shock wave", 0, 0, 0, 0.0, (0.0, 0.0), (3.5, 8.0), 0.0, 0.0, Z, 0.0, (0.0, 0.0)),
        r("weapons\\plasma grenade\\explosion", 3, 3, 0x20, 80.0, (120.0, 120.0), (0.5, 2.0), 0.4, 1.0, Z, 4.0, (1.0, 1.0)),
        r("weapons\\plasma grenade\\attached", 3, 3, 0, 80.0, (120.0, 120.0), (0.0, 0.0), 0.0, 0.0, Z, 4.0, (1.0, 1.0)),
        r("weapons\\plasma grenade\\shock wave", 0, 0, 0, 0.0, (0.0, 0.0), (3.5, 8.0), 0.0, 0.0, Z, 0.0, (0.0, 0.0)),
        r("weapons\\rocket launcher\\explosion", 4, 0, 0x20, 80.0, (300.0, 330.0), (0.5, 2.0), 0.6, 1.0, Z, 6.0, (1.0, 1.0)),
        r("vehicles\\scorpion\\shell explosion", 8, 0, 0x20, 80.0, (351.0, 351.0), (1.5, 3.25), 0.0, 0.0, Z, 6.0, (1.0, 1.0)),
        r("weapons\\flamethrower\\explosion", 7, 0, 0x80, 10.0, (10.0, 10.0), (0.4, 0.4), 0.0, 1.0, Z, 0.0, (1.0, 1.0)),
        r("vehicles\\warthog\\bullet", 8, 0, 0, 16.0, (16.0, 24.0), (0.0, 0.0), 0.0, 0.25, Z, 0.0, (1.0, 1.0)),
        r("vehicles\\scorpion\\bullet", 8, 0, 0, 10.0, (10.0, 10.0), (0.0, 0.0), 0.0, 1.0, Z, 0.0, (1.0, 1.0)),
        r("vehicles\\ghost\\ghost bolt", 8, 0, 0x4, 10.0, (10.0, 14.0), (0.0, 0.0), 0.0, 0.25, (1.0, 1.0, 0.15), 0.0, (0.5, 2.0)),
        melee("weapons\\assault rifle\\melee"),
        melee("weapons\\pistol\\melee"),
        melee("weapons\\shotgun\\melee"),
        melee("weapons\\plasma rifle\\melee"),
        melee("weapons\\sniper rifle\\melee"),
        melee("weapons\\plasma pistol\\melee"),
        melee("weapons\\needler\\melee"),
        melee("weapons\\rocket launcher\\melee"),
        r("weapons\\flamethrower\\melee", 6, 2, 0x5, 60.0, (70.0, 80.0), (0.5, 0.5), 0.0, 0.0, Z, 0.0, (1.0, 1.0)),
        r("weapons\\ball\\melee", 6, 2, 0x5, 80.0, (80.0, 80.0), (0.5, 0.5), 0.0, 0.0, Z, 0.0, (1.0, 1.0)),
        r("weapons\\flag\\melee", 6, 2, 0x5, 80.0, (80.0, 80.0), (0.5, 0.5), 0.0, 0.0, Z, 0.0, (1.0, 1.0)),
        r("characters\\cyborg\\melee", 6, 0, 0x1, 1000.0, (1000.0, 1000.0), (0.2, 0.2), 0.0, 0.0, Z, 1.5, (1.0, 1.0)),
        r("globals\\falling", 1, 0, 0, 10.0, (155.0, 155.0), (0.0, 0.0), 0.0, 0.0, Z, 0.0, (1.0, 1.0)),
        r("globals\\distance", 1, 0, 0, 105.0, (105.0, 105.0), (0.0, 0.0), 0.0, 0.0, Z, 0.0, (1.0, 1.0)),
        r("globals\\vehicle_collision", 9, 0, 0, 1000.0, (1000.0, 1000.0), (0.0, 0.0), 0.0, 0.0, Z, 0.0, (1.0, 1.0)),
        r("globals\\flaming_death", 7, 0, 0, 5000.0, (5000.0, 5000.0), (0.0, 0.0), 0.0, 0.0, Z, 0.0, (1.0, 1.0)),
        r("globals\\vehicle_hit_environment", 0, 0, 0, 0.0, (0.0, 0.0), (0.0, 0.0), 0.0, 0.0, Z, 0.0, (1.0, 1.0)),
        r("globals\\vehicle_killed_unit", 0, 0, 0, 0.0, (0.0, 0.0), (0.0, 0.0), 0.0, 0.0, Z, 0.0, (1.0, 1.0)),
    ]
}

#[test]
fn damage_effects_table() {
    let Some(map) = load("bloodgulch") else { return };
    assert_eq!(map.of_class(class::DAMAGE_EFFECT).count(), 66);
    for row in rows() {
        let d = dmg(&map, row.path);
        let w = row.path;
        assert_eq!(d.category.code(), row.category, "{w}");
        assert_eq!(d.side_effect.code(), row.side, "{w}");
        assert_eq!(d.damage_flags.bits(), row.flags, "{w} flags");
        assert_eq!(d.damage_flags.unnamed_bits(), 0, "{w}");
        close(d.damage_lower_bound, row.lower, w);
        close(d.damage_upper_bound.lower, row.upper.0, w);
        close(d.damage_upper_bound.upper, row.upper.1, w);
        close(d.radius.lower, row.radii.0, w);
        close(d.radius.upper, row.radii.1, w);
        close(d.area_of_effect_core_radius, row.core, w);
        close(d.active_camouflage_damage, row.camo, w);
        close(d.stun, row.stun.0, w);
        close(d.maximum_stun, row.stun.1, w);
        close(d.stun_time, row.stun.2, w);
        close(d.instantaneous_acceleration, row.accel, w);
        close(d.material_modifier(MaterialType::Cyborg), row.mods.0, w);
        close(d.material_modifier(MaterialType::CyborgEnergyShield), row.mods.1, w);
        // Cutoff scale is 0 on every disc tag; the pad ranges are not read.
        assert_eq!(d.cutoff_scale, 0.0, "{w}");
    }
    // The pistol and sniper bullets can cause headshots, the plasma rifle
    // bolt multiplayer headshots; the sniper bullet passes half to riders.
    assert!(
        dmg(&map, "weapons\\pistol\\bullet")
            .damage_flags
            .contains(DamageFlags::CAN_CAUSE_HEADSHOTS)
    );
    assert!(
        dmg(&map, "weapons\\plasma rifle\\bolt")
            .damage_flags
            .contains(DamageFlags::CAN_CAUSE_MULTIPLAYER_HEADSHOTS)
    );
    assert!(
        dmg(&map, "weapons\\plasma rifle\\bolt")
            .damage_flags
            .contains(DamageFlags::PINGS_RESISTANT_UNITS)
    );
    close(
        dmg(&map, "weapons\\sniper rifle\\sniper bullet").vehicle_passthrough_penalty,
        0.5,
        "passthrough",
    );
    assert_eq!(
        dmg(&map, "weapons\\frag grenade\\explosion").category,
        DamageCategory::Grenade
    );
    assert_eq!(
        dmg(&map, "weapons\\plasma grenade\\explosion").side_effect,
        SideEffect::Emp
    );
    // The melee effects have the tag flag "do not scale damage by distance"
    // (11 weapon melees: 8 + flamethrower, ball, flag).
    let melees = [
        "assault rifle",
        "pistol",
        "shotgun",
        "plasma rifle",
        "sniper rifle",
        "plasma pistol",
        "needler",
        "rocket launcher",
        "flamethrower",
        "ball",
        "flag",
    ];
    for w in melees {
        let d = dmg(&map, &format!("weapons\\{w}\\melee"));
        assert!(d.flags.bits() & 1 == 1, "{w}");
    }
    let n = map
        .of_class(class::DAMAGE_EFFECT)
        .filter(|t| DamageEffect::read(&map, t).unwrap().flags.bits() & 1 == 1)
        .count();
    assert_eq!(n, 11);
    // Frag explosion at the spec's scale 0.5 (d = 2.0): 100, with the table's
    // bounds.
    let frag = dmg(&map, "weapons\\frag grenade\\explosion");
    close(frag.area_amount(2.0, 1.0), 100.0, "frag at 2.0");
    assert_eq!(frag.area_amount(2.5, 1.0), 0.0);
    close(
        dmg(&map, "weapons\\plasma grenade\\explosion").area_amount(1.0, 0.5),
        106.667,
        "plasma at 1.0",
    );
    close(
        dmg(&map, "vehicles\\scorpion\\shell explosion").area_amount(2.0, 0.5),
        273.571,
        "shell at 2.0",
    );
    // Warthog bullet at the speed scale of test case (b).
    let hog = dmg(&map, "vehicles\\warthog\\bullet");
    close(hog.amount(0.9735, 1.0), 23.788, "warthog");
}

/// Expand a list as the spec writes it ("9, 11, 14, 15, 21 to 25") to its
/// material types.
fn types(list: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for part in list.split(',') {
        let part = part.trim();
        if let Some((a, b)) = part.split_once(" to ") {
            out.extend(a.trim().parse::<usize>().unwrap()..=b.trim().parse::<usize>().unwrap());
        } else {
            out.push(part.parse().unwrap());
        }
    }
    out
}

/// The 33 modifiers of a "value: types" description.
fn mods(groups: &[(f32, &str)]) -> [f32; 33] {
    let mut m = [0.0; 33];
    for (v, list) in groups {
        for t in types(list) {
            m[t] = *v;
        }
    }
    m
}

#[test]
fn material_modifier_lists() {
    let Some(map) = load("bloodgulch") else { return };
    let ar: &[(f32, &str)] = &[
        (1.0, "9, 11, 14, 15, 21 to 25"),
        (0.25, "5 to 8"),
        (0.5, "12, 13"),
        (0.6, "17"),
        (0.65, "19, 20"),
        (0.7, "30"),
    ];
    let scorpion: &[(f32, &str)] = &[
        (1.0, "9, 11, 14, 15, 21 to 25"),
        (0.25, "5 to 8"),
        (0.5, "12, 13"),
        (0.6, "17"),
        (0.65, "19, 20"),
        (0.7, "16, 30"),
    ];
    let plasma_expl: &[(f32, &str)] = &[
        (1.0, "0 to 11, 14, 15, 17, 21 to 24, 27, 28, 29, 31"),
        (4.0, "20, 25, 30"),
        (2.0, "19"),
        (1.5, "16"),
        (0.5, "13"),
        (0.25, "12"),
    ];
    let plasma_expl_needler: &[(f32, &str)] = &[
        (1.0, "0 to 11, 14, 15, 17, 21 to 24, 27, 28, 29, 31"),
        (4.0, "20, 25, 30"),
        (2.0, "19"),
        (1.5, "16"),
        (0.5, "13"),
        (0.2, "12"),
    ];
    let rocket: &[(f32, &str)] = &[
        (1.0, "0 to 8, 11 to 17, 21 to 24, 27 to 31"),
        (4.0, "9, 20"),
        (2.0, "19"),
        (0.4, "25"),
        (0.2, "10"),
    ];
    let melee8: &[(f32, &str)] = &[
        (1.0, "9, 11 to 18, 23, 24, 30"),
        (1.4, "21, 22"),
        (0.2, "19, 20, 25, 26"),
    ];
    let melee3: &[(f32, &str)] = &[(1.0, "9, 11 to 18, 21 to 24, 30"), (0.2, "19, 20, 25, 26")];
    let falling: &[(f32, &str)] = &[(1.0, "0 to 30")];
    let table: Vec<(&str, &[(f32, &str)])> = vec![
        ("weapons\\assault rifle\\bullet", ar),
        ("vehicles\\scorpion\\bullet", scorpion),
        (
            "weapons\\pistol\\bullet",
            &[
                (1.0, "11, 13, 14, 15, 22, 23, 24, 30"),
                (1.5, "19, 20, 21"),
                (4.0, "9"),
                (2.0, "17"),
                (0.25, "5 to 8"),
                (0.2, "12, 25"),
            ],
        ),
        (
            "weapons\\shotgun\\pellet",
            &[
                (1.0, "9, 11, 13, 14, 15, 17, 21, 23, 24, 30"),
                (0.5, "22, 25"),
                (0.2, "12"),
                (1.5, "19"),
                (1.75, "20"),
                (0.25, "5 to 8"),
            ],
        ),
        (
            "weapons\\sniper rifle\\sniper bullet",
            &[
                (1.0, "9, 11, 14, 15, 21 to 24"),
                (2.0, "30"),
                (0.5, "12, 13"),
                (0.2, "17, 25"),
                (0.05, "19, 20"),
            ],
        ),
        (
            "weapons\\plasma rifle\\bolt",
            &[
                (1.0, "9, 11, 14 to 17, 19, 20, 23, 24"),
                (2.0, "10, 22, 25, 30"),
                (0.5, "12, 13, 21"),
                (0.25, "5 to 8"),
            ],
        ),
        (
            "vehicles\\ghost\\ghost bolt",
            &[
                (1.0, "11, 14, 15, 17, 19, 20, 23, 24"),
                (2.0, "10, 16, 22, 25, 30"),
                (0.5, "12, 13, 21"),
                (0.25, "5 to 8"),
            ],
        ),
        (
            "weapons\\plasma pistol\\bolt",
            &[
                (1.0, "9, 11, 14 to 17, 19, 20, 23, 24"),
                (2.0, "10, 25, 30"),
                (0.6, "21, 22"),
                (0.5, "12, 13"),
                (0.25, "5 to 8"),
            ],
        ),
        (
            "weapons\\plasma rifle\\charged bolt",
            &[
                (1.0, "9, 11, 14 to 17, 19, 20, 22"),
                (4.0, "10, 25"),
                (1.5, "30"),
                (0.6, "21"),
                (0.5, "12, 13"),
                (0.2, "23, 24"),
                (0.25, "5 to 8"),
            ],
        ),
        (
            "weapons\\needler\\detonation damage",
            &[
                (1.0, "9, 11, 14, 15, 16, 21 to 24, 30"),
                (1.2, "19, 20"),
                (0.6, "17"),
                (0.5, "12, 13"),
                (0.25, "5 to 8"),
            ],
        ),
        ("weapons\\needler\\impact damage", &[]),
        (
            "vehicles\\warthog\\bullet",
            &[
                (1.0, "9, 11, 14, 15, 17, 19 to 25, 30"),
                (0.5, "12, 13"),
                (0.25, "5 to 8"),
            ],
        ),
        (
            "weapons\\frag grenade\\explosion",
            &[
                (1.0, "0 to 8, 11, 14 to 17, 21 to 24, 27 to 31"),
                (4.0, "9, 20"),
                (2.0, "19"),
                (0.5, "13"),
                (0.4, "25"),
                (0.25, "12"),
                (0.2, "10"),
            ],
        ),
        ("weapons\\rocket launcher\\explosion", rocket),
        ("vehicles\\scorpion\\shell explosion", rocket),
        ("weapons\\plasma grenade\\explosion", plasma_expl),
        ("weapons\\needler\\explosion", plasma_expl_needler),
        (
            "weapons\\plasma grenade\\attached",
            &[(1.0, "9, 10, 11, 13 to 17, 19 to 24"), (4.0, "25, 30"), (0.2, "12")],
        ),
        (
            "weapons\\flamethrower\\explosion",
            &[
                (1.0, "11, 13, 15, 16, 17, 21 to 24, 30"),
                (4.0, "9, 20"),
                (2.0, "19"),
                (0.4, "14, 25"),
                (0.2, "10, 12"),
            ],
        ),
        ("weapons\\flamethrower\\melee", melee3),
        ("weapons\\ball\\melee", melee3),
        ("weapons\\flag\\melee", melee3),
        (
            "characters\\cyborg\\melee",
            &[(1.0, "11, 14 to 25, 30"), (0.2, "12, 13")],
        ),
        ("globals\\falling", falling),
        ("globals\\distance", falling),
        ("globals\\vehicle_collision", falling),
        ("globals\\flaming_death", &[(1.0, "0 to 31")]),
        ("weapons\\plasma grenade\\shock wave", &[]),
        ("weapons\\needler\\shock wave", &[]),
        ("weapons\\frag grenade\\shock wave", &[(1.0, "9")]),
    ];
    let mut tables = table;
    for w in [
        "assault rifle",
        "pistol",
        "shotgun",
        "plasma rifle",
        "sniper rifle",
        "plasma pistol",
        "needler",
        "rocket launcher",
    ] {
        tables.push((Box::leak(format!("weapons\\{w}\\melee").into_boxed_str()), melee8));
    }
    for (path, groups) in tables {
        let d = dmg(&map, path);
        let want = mods(groups);
        for (t, w) in want.iter().enumerate() {
            close(d.material_modifier(ty(t)), *w, &format!("{path} type {t}"));
        }
        // Slots 33 to 39 are zero.
        assert!(d.material_modifiers.0[33..].iter().all(|&x| x == 0.0), "{path}");
    }
    // The explosions have 0 for 18, 26 and 32.
    for path in [
        "weapons\\frag grenade\\explosion",
        "weapons\\rocket launcher\\explosion",
        "vehicles\\scorpion\\shell explosion",
        "weapons\\plasma grenade\\explosion",
        "weapons\\needler\\explosion",
    ] {
        let d = dmg(&map, path);
        for t in [18, 26, 32] {
            assert_eq!(d.material_modifier(ty(t)), 0.0, "{path} {t}");
        }
    }
    // Firing damage effects do no damage (all modifiers 0).
    for path in ["weapons\\pistol\\trigger", "weapons\\assault rifle\\trigger"] {
        if let Some(t) = map.find(class::DAMAGE_EFFECT, path) {
            let d = DamageEffect::read(&map, t).unwrap();
            assert!(d.material_modifiers.0.iter().all(|&x| x == 0.0), "{path}");
        }
    }
}

#[test]
fn sweep_every_projectile_and_damage_effect() {
    let Some(map) = load("bloodgulch") else { return };
    let mut n = (0, 0);
    for t in map.of_class(class::PROJECTILE) {
        let proj = Projectile::read(&map, t).unwrap();
        let o = &proj.own;
        // References resolve to the right classes (the reader refuses
        // out-of-range enumerations).
        for r in [&o.super_detonation, &o.effect, &o.detonation_started] {
            if let Some(e) = map.tag(r.id) {
                assert_eq!(e.class, class::EFFECT, "{}", t.path);
            }
        }
        for (r, c) in [
            (&o.flyby_sound, class::SOUND),
            (&o.attached_detonation_damage, class::DAMAGE_EFFECT),
            (&o.impact_damage, class::DAMAGE_EFFECT),
        ] {
            if let Some(e) = map.tag(r.id) {
                assert_eq!(e.class, c, "{}", t.path);
            }
        }
        for r in &o.material_responses {
            for e in [&r.default_effect, &r.potential_effect, &r.detonation_effect] {
                if let Some(e) = map.tag(e.id) {
                    assert_eq!(e.class, class::EFFECT, "{}", t.path);
                }
            }
            // The unused fields are plausible: fractions in 0..=1.
            for f in [
                r.skip_fraction,
                r.initial_friction,
                r.parallel_friction,
                r.perpendicular_friction,
            ] {
                assert!((0.0..=1.0).contains(&f), "{}: {f}", t.path);
            }
        }
        assert!(o.detonation_timer.lower <= o.detonation_timer.upper, "{}", t.path);
        n.0 += 1;
    }
    for t in map.of_class(class::DAMAGE_EFFECT) {
        let d = DamageEffect::read(&map, t).unwrap();
        if let Some(e) = map.tag(d.sound.id) {
            assert_eq!(e.class, class::SOUND, "{}", t.path);
        }
        assert!(d.radius.lower <= d.radius.upper, "{}", t.path);
        assert!(d.damage_upper_bound.lower <= d.damage_upper_bound.upper, "{}", t.path);
        assert!(d.material_modifiers.0[33..].iter().all(|&x| x == 0.0), "{}", t.path);
        assert!(
            d.material_modifiers.0.iter().all(|&x| (0.0..=4.0).contains(&x)),
            "{}",
            t.path
        );
        assert_eq!(d.damage_flags.unnamed_bits() & !0x1FFF, 0, "{}", t.path);
        n.1 += 1;
    }
    assert_eq!(n, (16, 66));
}

#[test]
fn identical_on_other_maps() {
    let Some(base) = load("bloodgulch") else { return };
    for name in ["hangemhigh", "sidewinder", "chillout"] {
        assert!(MULTIPLAYER.contains(&name));
        let Some(other) = load(name) else { return };
        let mut n = 0;
        for t in base.of_class(class::PROJECTILE) {
            let o = other
                .find(class::PROJECTILE, &t.path)
                .unwrap_or_else(|| panic!("{name}: {}", t.path));
            assert_eq!(
                normalized(&base, &Projectile::read(&base, t).unwrap()),
                normalized(&other, &Projectile::read(&other, o).unwrap()),
                "{name}: {}",
                t.path
            );
            n += 1;
        }
        for t in base.of_class(class::DAMAGE_EFFECT) {
            let o = other
                .find(class::DAMAGE_EFFECT, &t.path)
                .unwrap_or_else(|| panic!("{name}: {}", t.path));
            assert_eq!(
                normalized(&base, &DamageEffect::read(&base, t).unwrap()),
                normalized(&other, &DamageEffect::read(&other, o).unwrap()),
                "{name}: {}",
                t.path
            );
            n += 1;
        }
        assert_eq!(n, 82, "{name}");
    }
}
