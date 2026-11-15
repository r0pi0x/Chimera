//! The weapon reader on the user's Halo disc image, against the "Multiplayer
//! weapon numbers" tables of specs/halo_ce/weapons.md: every row for the 15
//! `weap` tags of `bloodgulch` (weapon part, aim assist, heat, magazines,
//! triggers with the ticks between shots, error and projectiles, heat and
//! charge, firing effects, references), a sweep of the references and
//! enumerations, and the same values on other multiplayer maps. Each test
//! passes with a note when there is no image.

// The spec's numbers are written as it states them, rounded.
#![allow(clippy::approx_constant, clippy::excessive_precision, clippy::type_complexity)]

use blam_formats::cache::{MapFile, TagRef, class};
use blam_formats::weapon::{Distribution, SecondaryTriggerMode, Weapon, WeaponFlags, per_tick_rate, seconds_to_ticks};

use super::maps::{MULTIPLAYER, load};
use super::object_install::{near, normalized, path_of};

const ALL: [&str; 15] = [
    "weapons\\assault rifle\\assault rifle",
    "weapons\\pistol\\pistol",
    "weapons\\shotgun\\shotgun",
    "weapons\\plasma rifle\\plasma rifle",
    "weapons\\sniper rifle\\sniper rifle",
    "weapons\\plasma pistol\\plasma pistol",
    "weapons\\needler\\needler",
    "weapons\\rocket launcher\\rocket launcher",
    "weapons\\flamethrower\\flamethrower",
    "weapons\\gravity rifle\\gravity rifle",
    "weapons\\ball\\ball",
    "weapons\\flag\\flag",
    "vehicles\\warthog\\warthog gun",
    "vehicles\\ghost\\ghost gun",
    "vehicles\\scorpion\\scorpion cannon",
];

fn weapon(map: &MapFile, path: &str) -> Weapon {
    Weapon::read(map, map.find(class::WEAPON, path).unwrap_or_else(|| panic!("no {path}"))).unwrap()
}

fn p(map: &MapFile, r: &TagRef) -> Option<String> {
    path_of(map, r)
}

fn ps(s: &str) -> Option<String> {
    (s != "-").then(|| s.to_string())
}

#[test]
fn fifteen_weapons_in_order_of_the_table() {
    let Some(map) = load("bloodgulch") else { return };
    assert_eq!(map.of_class(class::WEAPON).count(), 15);
    for path in ALL {
        let w = weapon(&map, path);
        assert_eq!(w.object.object_type.code(), 2);
    }
}

/// Short names of the table rows.
fn full(short: &str) -> &'static str {
    ALL.iter()
        .copied()
        .find(|p| p.rsplit('\\').next().unwrap() == short || (short == "ghost gun" && p.ends_with("ghost gun")))
        .unwrap_or_else(|| panic!("{short}"))
}

#[test]
fn weapon_part_table() {
    let Some(map) = load("bloodgulch") else { return };
    // Flags, label, type, zoom levels, magnification, ready time.
    let rows: [(&str, u32, &str, i16, i16, (f32, f32), f32); 15] = [
        ("assault rifle", 0x0000, "ar", 0, 0, (0.0, 0.0), 1.3),
        ("pistol", 0x0000, "hp", 0, 1, (2.0, 2.0), 0.0),
        ("shotgun", 0x0000, "sg", 1, 0, (0.0, 0.0), 0.0),
        ("plasma rifle", 0x0800, "pr", 4, 0, (0.0, 0.0), 0.0),
        ("sniper rifle", 0x4020, "sr", 0, 2, (2.0, 8.0), 0.0),
        ("plasma pistol", 0x0800, "pp", 3, 0, (0.0, 0.0), 0.0),
        ("needler", 0x0000, "ne", 2, 0, (0.0, 0.0), 0.0),
        ("rocket launcher", 0x0000, "rl", 0, 1, (2.0, 2.0), 0.0),
        ("flamethrower", 0x0000, "ft", 0, 0, (0.0, 0.0), 0.0),
        ("gravity rifle", 0x0000, "ar", 0, 0, (0.0, 0.0), 0.0),
        ("ball", 0x00D8, "b", 0, 0, (0.0, 0.0), 0.0),
        ("flag", 0x00D8, "f", 0, 0, (0.0, 0.0), 0.0),
        ("warthog gun", 0x0000, "cg", 0, 0, (0.0, 0.0), 0.0),
        ("ghost gun", 0x0000, "gun", 0, 0, (0.0, 0.0), 0.0),
        ("scorpion cannon", 0x1000, "cannon", 0, 0, (0.0, 0.0), 0.0),
    ];
    for (name, flags, label, wtype, zoom, mag, ready) in rows {
        let w = weapon(&map, full(name)).own;
        assert_eq!(w.flags.bits(), flags, "{name} flags");
        assert_eq!(w.flags.unnamed_bits(), 0, "{name}");
        assert_eq!(w.label, label, "{name}");
        assert_eq!(w.weapon_type.code(), wtype, "{name}");
        assert_eq!(w.magnification_levels, zoom, "{name}");
        near(w.magnification_range.lower, mag.0, name);
        near(w.magnification_range.upper, mag.1, name);
        near(w.ready_time, ready, name);
        assert_eq!(w.secondary_trigger_mode, SecondaryTriggerMode::Normal, "{name}");
        assert_eq!(w.maximum_alternate_shots_loaded, 0, "{name}");
    }
    // Zoom levels: the pistol and rocket launcher 2x, the sniper 2x then 8x.
    for n in ["pistol", "rocket launcher"] {
        let w = weapon(&map, full(n)).own;
        assert_eq!(w.magnification(0), Some(2.0));
        assert_eq!(w.magnification(1), None);
    }
    let s = weapon(&map, full("sniper rifle")).own;
    assert_eq!(s.magnification(0), Some(2.0));
    near(s.magnification(1).unwrap(), 8.0, "8x");
    assert!(weapon(&map, full("sniper rifle")).own.flags.contains(WeaponFlags::AIM_ASSISTS_ONLY_WHEN_ZOOMED));
    assert!(weapon(&map, full("ball")).own.flags.contains(WeaponFlags::MUST_BE_READIED));

    // Function sources A to D.
    let sources: [(&str, [i16; 4]); 15] = [
        ("assault rifle", [14, 2, 12, 0]),
        ("pistol", [12, 0, 0, 0]),
        ("shotgun", [7, 0, 12, 14]),
        ("plasma rifle", [1, 12, 0, 13]),
        ("sniper rifle", [12, 6, 0, 0]),
        ("plasma pistol", [1, 10, 12, 13]),
        ("needler", [12, 2, 0, 0]),
        ("rocket launcher", [14, 0, 0, 0]),
        ("flamethrower", [15, 2, 1, 0]),
        ("gravity rifle", [12, 0, 0, 0]),
        ("warthog gun", [12, 0, 0, 4]),
        ("ghost gun", [4, 0, 0, 0]),
        ("scorpion cannon", [12, 0, 0, 4]),
        ("ball", [0; 4]),
        ("flag", [0; 4]),
    ];
    for (name, want) in sources {
        let w = weapon(&map, full(name)).own;
        assert_eq!([w.input_a.code(), w.input_b.code(), w.input_c.code(), w.input_d.code()], want, "{name}");
    }
}

#[test]
fn aim_assist_and_movement_table() {
    let Some(map) = load("bloodgulch") else { return };
    // Autoaim angle and range, magnetism angle and range, deviation,
    // movement penalized, forward, sideways.
    let rows: [(&str, f32, f32, f32, f32, f32, i16, f32, f32); 15] = [
        ("assault rifle", 0.10472, 25.0, 0.20944, 25.0, 0.0, 0, 0.0, 0.0),
        ("pistol", 0.0523599, 30.0, 0.10472, 30.0, 0.0, 0, 0.0, 0.0),
        ("shotgun", 0.10472, 15.0, 0.20944, 15.0, 0.0, 0, 0.0, 0.0),
        ("plasma rifle", 0.0872665, 25.0, 0.20944, 25.0, 0.0, 0, 0.0, 0.0),
        ("sniper rifle", 0.0174533, 35.0, 0.0349066, 35.0, 0.0, 0, 0.0, 0.0),
        ("plasma pistol", 0.0872665, 20.0, 0.20944, 20.0, 0.0, 0, 0.0, 0.0),
        ("needler", 0.10472, 25.0, 0.20944, 25.0, 0.0, 0, 0.0, 0.0),
        ("rocket launcher", 0.0, 35.0, 0.20944, 35.0, 0.0, 2, 0.25, 0.25),
        ("flamethrower", 0.0, 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0.0),
        ("gravity rifle", 0.0, 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0.0),
        ("ball", 0.0, 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0.0),
        ("flag", 0.0, 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0.0),
        ("warthog gun", 0.10472, 25.0, 0.20944, 25.0, 0.174533, 0, 0.0, 0.0),
        ("ghost gun", 0.0872665, 25.0, 0.20944, 25.0, 0.785398, 0, 0.0, 0.0),
        ("scorpion cannon", 0.10472, 35.0, 0.20944, 35.0, 0.349066, 0, 0.0, 0.0),
    ];
    for (name, aa, ar, ma, mr, dev, pen, fwd, side) in rows {
        let w = weapon(&map, full(name)).own;
        near(w.autoaim_angle, aa, name);
        near(w.autoaim_range, ar, name);
        near(w.magnetism_angle, ma, name);
        near(w.magnetism_range, mr, name);
        near(w.deviation_angle, dev, name);
        assert_eq!(w.movement_penalized.code(), pen, "{name}");
        near(w.forward_movement_penalty, fwd, name);
        near(w.sideways_movement_penalty, side, name);
    }
    // The pistol zoomed 2x: autoaim 1.5 degrees, range 60, deviation 1.5.
    let a = weapon(&map, full("pistol")).own.aim_assist(Some(2.0)).unwrap();
    near(a.autoaim_angle, 1.5f32.to_radians(), "pistol zoomed autoaim");
    near(a.autoaim_range, 60.0, "pistol zoomed range");
    near(a.deviation, 1.5f32.to_radians(), "pistol zoomed deviation");
    // Unzoomed pistol: deviation 3 degrees; the rocket launcher: 0.
    near(weapon(&map, full("pistol")).own.aim_assist(None).unwrap().deviation, 3.0f32.to_radians(), "pistol");
    assert_eq!(weapon(&map, full("rocket launcher")).own.aim_assist(None).unwrap().deviation, 0.0);
    // The sniper rifle only assists zoomed: 0.5 degrees at 2x, 0.125 at 8x
    // for the autoaim cone's deviation... (the spec's 0.5 and 0.125 are the
    // 1 degree cone over the magnification).
    let s = weapon(&map, full("sniper rifle")).own;
    assert!(s.aim_assist(None).is_none());
    near(s.aim_assist(s.magnification(0)).unwrap().deviation, 0.5f32.to_radians(), "sniper 2x");
    near(s.aim_assist(s.magnification(1)).unwrap().deviation, 0.125f32.to_radians(), "sniper 8x");
}

#[test]
fn heat_and_battery_table() {
    let Some(map) = load("bloodgulch") else { return };
    // Recovery, overheat, detonation threshold, fraction, loss per second,
    // illumination, age heat penalty, age rate of fire penalty, misfire
    // start, chance.
    let rows: [(&str, [f32; 10]); 3] = [
        ("plasma rifle", [0.25, 1.0, 1.0, 0.0, 0.3, 1.0, 0.2, 0.2, 0.9, 0.5]),
        ("plasma pistol", [0.25, 1.0, 1.0, 0.0, 0.65, 1.0, 0.0, 0.0, 0.9, 0.5]),
        ("flamethrower", [0.9, 1.0, 0.0, 0.0, 0.475, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ];
    let get = |w: &blam_formats::weapon::WeaponPart| {
        [
            w.heat_recovery_threshold,
            w.overheated_threshold,
            w.heat_detonation_threshold,
            w.heat_detonation_fraction,
            w.heat_loss_per_second,
            w.heat_illumination,
            w.age_heat_recovery_penalty,
            w.age_rate_of_fire_penalty,
            w.age_misfire_start,
            w.age_misfire_chance,
        ]
    };
    for (name, want) in rows {
        let got = get(&weapon(&map, full(name)).own);
        for (g, w) in got.iter().zip(want) {
            near(*g, w, name);
        }
    }
    for path in ALL {
        if rows.iter().any(|r| full(r.0) == path) {
            continue;
        }
        assert_eq!(get(&weapon(&map, path).own), [0.0; 10], "{path}");
    }
}

#[test]
fn magazines_table() {
    let Some(map) = load("bloodgulch") else { return };
    // Flags, recharge, initial, max, loaded, reload time, reloaded, chamber
    // time, ammunition objects (rounds, equipment).
    let rows: [(&str, u32, i16, i16, i16, i16, f32, i16, f32, Option<(i16, &str)>); 13] = [
        ("assault rifle", 0, 0, 240, 600, 60, 3.4, 60, 0.0, Some((240, "powerups\\assault rifle ammo\\assault rifle ammo"))),
        ("pistol", 0, 0, 60, 120, 12, 2.17, 12, 0.0, Some((120, "powerups\\pistol ammo\\pistol ammo"))),
        ("shotgun", 2, 0, 24, 60, 12, 0.4, 1, 0.0, Some((60, "powerups\\shotgun ammo\\shotgun ammo"))),
        ("plasma rifle", 0, 0, 0, 0, 0, 0.0, 0, 0.0, None),
        ("sniper rifle", 0, 0, 12, 24, 4, 0.0, 4, 0.0, Some((16, "powerups\\sniper rifle ammo\\sniper rifle ammo"))),
        ("plasma pistol", 0, 0, 0, 0, 0, 0.0, 0, 0.0, None),
        ("needler", 0, 0, 80, 80, 20, 1.0, 20, 0.0, Some((80, "powerups\\needler ammo\\needler ammo"))),
        ("rocket launcher", 0, 0, 4, 8, 2, 5.0, 2, 0.0, Some((2, "powerups\\rocket launcher ammo\\rocket launcher ammo"))),
        ("flamethrower", 1, 0, 600, 1200, 100, 3.4, 100, 0.0, Some((100, "powerups\\flamethrower ammo\\flamethrower ammo"))),
        ("gravity rifle", 0, 0, 200, 400, 200, 3.4, 200, 0.0, None),
        ("warthog gun", 0, 0, 0, 0, 0, 0.0, 0, 0.0, None),
        ("ghost gun", 0, 0, 300, 300, 50, 2.0, 50, 0.3, None),
        ("scorpion cannon", 2, 0, 9999, 9999, 1, 0.0, 1, 4.0, None),
    ];
    for (name, flags, recharge, initial, max, loaded, reload, reloaded, chamber, ammo) in rows {
        let w = weapon(&map, full(name));
        assert_eq!(w.own.magazines.len(), 1, "{name}");
        let m = &w.own.magazines[0];
        assert_eq!(m.flags.bits(), flags, "{name}");
        assert_eq!(m.rounds_recharged, recharge, "{name}");
        assert_eq!(m.rounds_total_initial, initial, "{name}");
        assert_eq!(m.rounds_total_maximum, max, "{name}");
        assert_eq!(m.rounds_loaded_maximum, loaded, "{name}");
        near(m.reload_time, reload, name);
        assert_eq!(m.rounds_reloaded, reloaded, "{name}");
        near(m.chamber_time, chamber, name);
        match ammo {
            None => assert!(m.magazines.is_empty(), "{name}"),
            Some((rounds, eq)) => {
                assert_eq!(m.magazines.len(), 1, "{name}");
                assert_eq!(m.magazines[0].rounds, rounds, "{name}");
                assert_eq!(p(&map, &m.magazines[0].equipment).as_deref(), Some(eq), "{name}");
                assert_eq!(m.magazines[0].equipment.class, class::EQUIPMENT);
            }
        }
    }
    // A new weapon holds the loaded rounds and the rest in reserve.
    for (name, loaded, reserve) in [
        ("assault rifle", 60, 180),
        ("pistol", 12, 48),
        ("shotgun", 12, 12),
        ("sniper rifle", 4, 8),
        ("needler", 20, 60),
        ("rocket launcher", 2, 2),
        ("flamethrower", 100, 500),
    ] {
        let m = &weapon(&map, full(name)).own.magazines[0];
        assert_eq!(m.rounds_loaded_maximum, loaded, "{name}");
        assert_eq!(m.rounds_total_initial - m.rounds_loaded_maximum, reserve, "{name}");
    }
    // Ball and flag have no magazines or triggers.
    for n in ["ball", "flag"] {
        let w = weapon(&map, full(n));
        assert!(w.own.magazines.is_empty() && w.own.triggers.is_empty(), "{n}");
    }
    // The only reloading and chambering effect on the disc is the pistol's
    // reload.
    for path in ALL {
        let w = weapon(&map, path);
        for m in &w.own.magazines {
            let want = (path == full("pistol")).then(|| "weapons\\pistol\\effects\\reload".to_string());
            assert_eq!(p(&map, &m.reloading_effect), want, "{path}");
            assert_eq!(p(&map, &m.chambering_effect), None, "{path}");
        }
    }
}

/// One trigger row of the three trigger tables.
struct T {
    weapon: &'static str,
    trigger: usize,
    flags: u32,
    rate: (f32, f32),
    /// Ticks between shots at rate of fire 0 and 1 (None: no limit stated).
    ticks: Option<(u32, u32)>,
    times: (f32, f32),
    runtime: (f32, f32),
    blur: f32,
    magazine: i16,
    rounds_per_shot: i16,
    min_loaded: i16,
    tracer: i16,
    noise: i16,
    error: (f32, f32),
    min_error: f32,
    angles: (f32, f32),
    error_times: (f32, f32),
    runtime_error: (f32, f32),
    projectiles: i16,
    projectile: &'static str,
    heat: f32,
    age: f32,
    charging: (f32, f32),
    overcharged: i16,
    charged_illum: f32,
    spew_overload: (f32, f32),
    offset: [f32; 3],
    ejection: (f32, f32),
    runtime_ejection: (f32, f32),
}

// The spec's table puts "1 explodes" on the plasma pistol's first trigger;
// on the disc it is the second (the charged bolt's) that explodes.
#[rustfmt::skip]
fn trigger_rows() -> Vec<T> {
    let t = |weapon, trigger, flags, rate, ticks, times, runtime, blur, magazine, rps, minl, tracer, noise,
             error, min_error, angles, error_times, runtime_error, projectiles, projectile,
             heat, age, charging, overcharged, charged_illum, spew_overload, offset, ejection, runtime_ejection| T {
        weapon, trigger, flags, rate, ticks, times, runtime, blur, magazine, rounds_per_shot: rps, min_loaded: minl,
        tracer, noise, error, min_error, angles, error_times, runtime_error, projectiles, projectile, heat, age,
        charging, overcharged, charged_illum, spew_overload, offset, ejection, runtime_ejection,
    };
    let z = [0.0; 3];
    vec![
        t("assault rifle", 0, 0, (15.0, 15.0), Some((2, 2)), (1.0, 0.1), (0.0333333, 0.333333), 0.0, 0, 1, 0, 3, 2,
          (0.0, 1.0), 0.0, (0.0349066, 0.113446), (0.6, 1.0), (0.0555556, 0.0333333), 1, "weapons\\assault rifle\\bullet",
          0.0, 0.0, (0.0, 0.0), 0, 0.0, (0.0, 0.0), z, (0.15, 0.1), (0.222222, 0.333333)),
        t("pistol", 0, 0, (3.5, 3.5), Some((9, 9)), (0.0, 0.0), (1.0, 1.0), 0.0, 0, 1, 0, 0, 2,
          (0.0, 2.0), 0.0, (0.00349066, 0.0349066), (0.5, 0.2), (0.0666667, 0.166667), 1, "weapons\\pistol\\bullet",
          0.0, 0.0, (0.0, 0.0), 0, 0.0, (0.0, 0.0), z, (0.0, 0.1), (1.0, 0.333333)),
        t("shotgun", 0, 0, (1.0, 1.0), Some((30, 30)), (0.0, 0.0), (1.0, 1.0), 0.0, 0, 1, 0, 0, 2,
          (0.0, 0.0), 0.0, (0.174533, 0.174533), (0.0, 0.0), (1.0, 1.0), 15, "weapons\\shotgun\\pellet",
          0.0, 0.0, (0.0, 0.0), 0, 0.0, (0.0, 0.0), z, (0.15, 0.1), (0.222222, 0.333333)),
        t("plasma rifle", 0, 0, (7.0, 10.0), Some((5, 3)), (1.8, 0.25), (0.0185185, 0.133333), 0.0, 0, 0, 0, 3, 2,
          (0.0, 1.0), 0.0, (0.00872665, 0.0872665), (2.5, 0.0), (0.0133333, 1.0), 1, "weapons\\plasma rifle\\bolt",
          0.08, 0.005, (0.0, 0.0), 0, 0.0, (0.0, 0.0), z, (0.0, 0.15), (1.0, 0.222222)),
        t("sniper rifle", 0, 0x0408, (2.0, 2.0), Some((15, 15)), (0.0, 0.0), (1.0, 1.0), 0.0, 0, 1, 0, 0, 1,
          (1.0, 1.0), 0.0, (0.00872665, 0.00872665), (0.0, 0.0), (1.0, 1.0), 1, "weapons\\sniper rifle\\sniper bullet",
          0.0, 0.0, (0.0, 0.0), 0, 0.0, (0.0, 0.0), z, (0.15, 0.1), (0.222222, 0.333333)),
        t("plasma pistol", 0, 0, (0.0, 0.0), None, (0.0, 0.0), (1.0, 1.0), 0.0, 0, 0, 0, 0, 2,
          (0.0, 1.0), 0.0, (0.0, 0.0), (0.0, 0.0), (1.0, 1.0), 1, "weapons\\plasma pistol\\bolt",
          0.16, 0.002, (0.6, 45.0), 0, 1.0, (0.0, 0.0), z, (0.0, 0.15), (1.0, 0.222222)),
        t("plasma pistol", 1, 0x0004, (0.0, 0.0), None, (0.0, 0.0), (1.0, 1.0), 0.0, 0, 0, 0, 0, 2,
          (0.0, 0.0), 0.0, (0.0, 0.0), (0.0, 0.0), (1.0, 1.0), 1, "weapons\\plasma rifle\\charged bolt",
          1.0, 0.11, (0.0, 0.0), 1, 0.0, (0.0, 0.0), z, (0.0, 1.0), (1.0, 0.0333333)),
        t("needler", 0, 0, (3.0, 10.0), Some((10, 3)), (0.5, 0.15), (0.0666667, 0.222222), 0.0, 0, 1, 0, 0, 2,
          (0.0, 1.0), 0.0349066, (0.0698132, 0.0698132), (0.5, 0.0), (0.0666667, 1.0), 1, "weapons\\needler\\needle",
          0.0, 0.0, (0.0, 0.0), 0, 0.0, (0.0, 0.0), z, (0.0, 0.2), (1.0, 0.166667)),
        t("rocket launcher", 0, 0x0008, (0.5, 0.5), Some((60, 60)), (0.0, 0.0), (1.0, 1.0), 0.0, 0, 1, 0, 0, 2,
          (0.0, 0.0), 0.0, (0.0, 0.0), (0.0, 0.0), (1.0, 1.0), 1, "weapons\\rocket launcher\\rocket",
          0.0, 0.0, (0.0, 0.0), 0, 0.0, (0.0, 0.0), [0.0, -0.1, 0.0], (0.0, 0.3), (1.0, 0.111111)),
        t("flamethrower", 0, 0x0800, (15.0, 15.0), Some((2, 2)), (0.2, 0.0), (0.166667, 1.0), 0.0, 0, 1, 0, 0, 2,
          (0.0, 0.0), 0.0, (0.0349066, 0.0698132), (0.0, 0.0), (1.0, 1.0), 1, "weapons\\flamethrower\\flame",
          0.04125, 0.0, (0.0, 0.0), 0, 0.0, (0.0, 0.0), z, (0.0, 1.0), (1.0, 0.0333333)),
        t("gravity rifle", 0, 0, (30.0, 30.0), Some((1, 1)), (0.0, 0.0), (1.0, 1.0), 0.0, 0, 1, 0, 0, 2,
          (0.0, 0.0), 0.0, (0.0698132, 0.20944), (0.0, 0.0), (1.0, 1.0), 1, "weapons\\flamethrower\\flame",
          0.0, 0.0, (0.0, 0.0), 0, 0.0, (0.0, 0.0), z, (0.0, 1.0), (1.0, 0.0333333)),
        t("warthog gun", 0, 0, (8.0, 15.0), Some((4, 2)), (1.0, 4.0), (0.0333333, 0.00833333), 0.5, 0, 0, 0, 3, 2,
          (0.0, 1.0), 0.0, (0.0174533, 0.0349066), (3.0, 1.0), (0.0111111, 0.0333333), 1, "vehicles\\warthog\\bullet",
          0.0, 0.0, (0.0, 0.0), 0, 0.0, (0.0, 0.0), z, (0.15, 0.1), (0.222222, 0.333333)),
        t("ghost gun", 0, 0x1020, (5.0, 6.0), Some((6, 5)), (1.0, 0.25), (0.0333333, 0.133333), 0.0, 0, 1, 0, 2, 2,
          (0.0, 1.0), 0.00872665, (0.00872665, 0.0523599), (2.0, 0.5), (0.0166667, 0.0666667), 1, "vehicles\\ghost\\ghost bolt",
          0.0, 0.0, (0.0, 0.0), 0, 0.0, (0.0, 0.0), z, (0.0, 0.0), (1.0, 1.0)),
        t("scorpion cannon", 0, 0x0028, (0.0, 0.0), None, (0.0, 0.0), (1.0, 1.0), 0.0, 0, 1, 0, 0, 2,
          (0.0, 0.0), 0.0, (0.0174533, 0.0349066), (0.0, 0.0), (1.0, 1.0), 1, "vehicles\\scorpion\\tank shell",
          0.0, 0.0, (0.0, 0.0), 0, 0.0, (0.0, 0.0), z, (0.15, 0.1), (0.222222, 0.333333)),
        t("scorpion cannon", 1, 0x0020, (15.0, 15.0), Some((2, 2)), (0.0, 0.0), (1.0, 1.0), 0.0, -1, 1, 0, 3, 2,
          (0.0, 1.0), 0.0, (0.0349066, 0.113446), (0.6, 1.0), (0.0555556, 0.0333333), 1, "vehicles\\scorpion\\bullet",
          0.0, 0.0, (0.0, 0.0), 0, 0.0, (0.0, 0.0), z, (0.15, 0.1), (0.222222, 0.333333)),
    ]
}

#[test]
fn trigger_tables() {
    let Some(map) = load("bloodgulch") else { return };
    for row in trigger_rows() {
        let name = format!("{} ({})", row.weapon, row.trigger);
        let w = weapon(&map, full(row.weapon));
        let t = &w.own.triggers[row.trigger];
        assert_eq!(t.flags.bits(), row.flags, "{name} flags");
        assert_eq!(t.flags.unnamed_bits(), 0, "{name}");
        near(t.rounds_per_second.lower, row.rate.0, &name);
        near(t.rounds_per_second.upper, row.rate.1, &name);
        if let Some((at0, at1)) = row.ticks {
            assert_eq!(w.ticks_between_shots(row.trigger, 0.0, 0.0), Some(at0), "{name} ticks at 0");
            assert_eq!(w.ticks_between_shots(row.trigger, 1.0, 0.0), Some(at1), "{name} ticks at 1");
        }
        near(t.acceleration_time, row.times.0, &name);
        near(t.deceleration_time, row.times.1, &name);
        near(t.runtime_rate_of_fire_acceleration, row.runtime.0, &name);
        near(t.runtime_rate_of_fire_deceleration, row.runtime.1, &name);
        near(t.blurred_rate_of_fire, row.blur, &name);
        assert_eq!(t.magazine, row.magazine, "{name} magazine");
        assert_eq!(t.rounds_per_shot, row.rounds_per_shot, "{name}");
        assert_eq!(t.minimum_rounds_loaded, row.min_loaded, "{name}");
        assert_eq!(t.rounds_between_tracers, row.tracer, "{name}");
        assert_eq!(t.firing_noise.code(), row.noise, "{name}");
        near(t.error.lower, row.error.0, &name);
        near(t.error.upper, row.error.1, &name);
        near(t.minimum_error, row.min_error, &name);
        near(t.error_angle.lower, row.angles.0, &name);
        near(t.error_angle.upper, row.angles.1, &name);
        near(t.error_acceleration_time, row.error_times.0, &name);
        near(t.error_deceleration_time, row.error_times.1, &name);
        near(t.runtime_error_acceleration, row.runtime_error.0, &name);
        near(t.runtime_error_deceleration, row.runtime_error.1, &name);
        assert_eq!(t.projectiles_per_shot, row.projectiles, "{name}");
        assert_eq!(t.distribution_function, Distribution::Point, "{name}");
        assert_eq!(t.distribution_angle, 0.0, "{name}");
        assert_eq!(p(&map, &t.projectile).as_deref(), Some(row.projectile), "{name}");
        assert_eq!(t.projectile.class, class::PROJECTILE);
        near(t.heat_generated_per_round, row.heat, &name);
        near(t.age_generated_per_round, row.age, &name);
        near(t.charging_time, row.charging.0, &name);
        near(t.charged_time, row.charging.1, &name);
        assert_eq!(t.overcharged_action.code(), row.overcharged, "{name}");
        near(t.charged_illumination, row.charged_illum, &name);
        near(t.spew_time, row.spew_overload.0, &name);
        near(t.overload_time, row.spew_overload.1, &name);
        for k in 0..3 {
            near(t.first_person_offset[k], row.offset[k], &name);
        }
        near(t.ejection_port_recovery_time, row.ejection.0, &name);
        near(t.illumination_recovery_time, row.ejection.1, &name);
        near(t.runtime_ejection_port_recovery, row.runtime_ejection.0, &name);
        near(t.runtime_illumination_recovery, row.runtime_ejection.1, &name);
        assert!(t.charging_effect.is_null(), "{name}: no trigger has a charging effect");
        // The stored per-tick rates are the formulas on the times.
        near(per_tick_rate(t.acceleration_time), t.runtime_rate_of_fire_acceleration, &name);
        near(per_tick_rate(t.deceleration_time), t.runtime_rate_of_fire_deceleration, &name);
        near(per_tick_rate(t.error_acceleration_time), t.runtime_error_acceleration, &name);
        near(per_tick_rate(t.error_deceleration_time), t.runtime_error_deceleration, &name);
        near(per_tick_rate(t.ejection_port_recovery_time), t.runtime_ejection_port_recovery, &name);
        near(per_tick_rate(t.illumination_recovery_time), t.runtime_illumination_recovery, &name);
    }
    // The number of triggers: two on the plasma pistol, scorpion cannon;
    // none on ball and flag; one on the others.
    for path in ALL {
        let n = weapon(&map, path).own.triggers.len();
        let want = match path {
            "weapons\\ball\\ball" | "weapons\\flag\\flag" => 0,
            "weapons\\plasma pistol\\plasma pistol" | "vehicles\\scorpion\\scorpion cannon" => 2,
            _ => 1,
        };
        assert_eq!(n, want, "{path}");
    }
}

#[test]
fn the_plasma_rifle_and_the_battery() {
    let Some(map) = load("bloodgulch") else { return };
    let w = weapon(&map, full("plasma rifle"));
    let t = &w.own.triggers[0];
    // Quirk 1: the battery penalty keeps the interval at 4 ticks.
    assert_eq!(w.ticks_between_shots(0, 1.0, 0.005), Some(4));
    near(t.required_ticks(1.0, 0.005, w.own.age_rate_of_fire_penalty), 3.003, "required at age 0.005");
    assert_eq!(w.ticks_between_shots(0, 1.0, 0.0), Some(3));
    near(t.required_ticks(0.0, 0.995, w.own.age_rate_of_fire_penalty), 5.1386, "required at age 0.995");
    assert_eq!(w.ticks_between_shots(0, 0.0, 0.995), Some(6));
    // The charge times of the plasma pistol: 18 and 1350 ticks.
    let pp = weapon(&map, full("plasma pistol"));
    assert_eq!(seconds_to_ticks(pp.own.triggers[0].charging_time), 18);
    assert_eq!(seconds_to_ticks(pp.own.triggers[0].charged_time), 1350);
    // The assault rifle and the pistol.
    assert_eq!(weapon(&map, full("assault rifle")).ticks_between_shots(0, 1.0, 0.0), Some(2));
    assert_eq!(weapon(&map, full("pistol")).ticks_between_shots(0, 1.0, 0.0), Some(9));
    assert_eq!(weapon(&map, full("shotgun")).ticks_between_shots(0, 1.0, 0.0), Some(30));
}

/// Weapon, trigger, entry, firing, misfire, empty, firing damage, misfire
/// damage ("-" none).
const EFFECTS: [(&str, usize, usize, &str, &str, &str, &str, &str); 17] = [
    ("assault rifle", 0, 0, "weapons\\assault rifle\\effects\\fire bullet", "-", "weapons\\assault rifle\\effects\\empty", "weapons\\assault rifle\\trigger", "-"),
    ("pistol", 0, 0, "weapons\\pistol\\effects\\fire bullet", "-", "weapons\\pistol\\effects\\empty", "weapons\\pistol\\trigger", "-"),
    ("shotgun", 0, 0, "weapons\\shotgun\\effects\\shotgun firing", "weapons\\shotgun\\effects\\shotgun empty", "sound\\sfx\\weapons\\assault rifle\\dryfire", "weapons\\shotgun\\trigger", "-"),
    ("plasma rifle", 0, 0, "weapons\\plasma rifle\\effects\\plasma rifle upper fire", "-", "sound\\sfx\\weapons\\plasma rifle\\overheat", "weapons\\plasma rifle\\trigger", "weapons\\plasma rifle\\misfire"),
    ("plasma rifle", 0, 1, "weapons\\plasma rifle\\effects\\plasma rifle lower fire", "weapons\\plasma rifle\\effects\\misfire", "-", "weapons\\plasma rifle\\trigger", "weapons\\plasma rifle\\misfire"),
    ("sniper rifle", 0, 0, "weapons\\sniper rifle\\effects\\fire bullet", "-", "sound\\sfx\\weapons\\assault rifle\\dryfire", "weapons\\sniper rifle\\trigger", "-"),
    ("plasma pistol", 0, 0, "weapons\\plasma pistol\\effects\\fire bolt", "weapons\\plasma pistol\\effects\\misfire", "sound\\sfx\\weapons\\plasma rifle\\overheat", "weapons\\plasma pistol\\trigger", "weapons\\plasma pistol\\misfire"),
    ("plasma pistol", 1, 0, "weapons\\plasma rifle\\effects\\charged bolt fire", "-", "-", "weapons\\plasma pistol\\trigger overcharge", "-"),
    ("needler", 0, 0, "weapons\\needler\\effects\\fire needle", "-", "weapons\\needler\\effects\\empty", "weapons\\needler\\trigger", "-"),
    ("rocket launcher", 0, 0, "weapons\\rocket launcher\\effects\\fire rocket", "-", "sound\\sfx\\weapons\\assault rifle\\dryfire", "weapons\\rocket launcher\\trigger", "-"),
    ("flamethrower", 0, 0, "weapons\\flamethrower\\effects\\flame thrower jet", "-", "-", "-", "-"),
    ("warthog gun", 0, 0, "vehicles\\warthog\\effects\\warthog gun fire", "-", "-", "vehicles\\warthog\\trigger", "-"),
    ("ghost gun", 0, 0, "vehicles\\ghost\\ghost gun fire beam", "-", "-", "vehicles\\ghost\\trigger", "-"),
    ("scorpion cannon", 0, 0, "vehicles\\scorpion\\scorpion cannon fire", "-", "-", "vehicles\\scorpion\\shell shock wave", "-"),
    ("scorpion cannon", 1, 0, "vehicles\\scorpion\\secondary fire bullet", "-", "-", "vehicles\\scorpion\\secondary trigger", "-"),
    ("gravity rifle", 0, 0, "", "", "", "", ""),
    ("ball", 0, 0, "", "", "", "", ""),
];

#[test]
fn firing_effects_table() {
    let Some(map) = load("bloodgulch") else { return };
    for (name, trig, entry, fire, misfire, empty, damage, misdamage) in EFFECTS {
        let w = weapon(&map, full(name));
        if fire.is_empty() {
            // No firing effects (gravity rifle), or no trigger at all.
            assert!(w.own.triggers.first().is_none_or(|t| t.firing_effects.is_empty()), "{name}");
            continue;
        }
        let effects = &w.own.triggers[trig].firing_effects;
        let e = &effects[entry];
        let n = format!("{name} ({trig}) entry {entry}");
        assert_eq!((e.shot_count.lower, e.shot_count.upper), (1, 1), "{n}");
        assert_eq!(p(&map, &e.firing_effect), ps(fire), "{n} firing");
        assert_eq!(p(&map, &e.misfire_effect), ps(misfire), "{n} misfire");
        assert_eq!(p(&map, &e.empty_effect), ps(empty), "{n} empty");
        assert_eq!(p(&map, &e.firing_damage), ps(damage), "{n} firing damage");
        assert_eq!(p(&map, &e.misfire_damage), ps(misdamage), "{n} misfire damage");
        assert!(e.empty_damage.is_null(), "{n}: no entry has an empty damage");
        // The sounds are `snd!` tags, the rest effects.
        for (r, path) in [(&e.firing_effect, fire), (&e.misfire_effect, misfire), (&e.empty_effect, empty)] {
            if path.starts_with("sound\\") {
                assert_eq!(r.class, class::SOUND, "{n}");
            } else if path != "-" {
                assert_eq!(r.class, class::EFFECT, "{n}");
            }
        }
    }
    // The plasma rifle has two entries, the others one.
    assert_eq!(weapon(&map, full("plasma rifle")).own.triggers[0].firing_effects.len(), 2);
    assert_eq!(weapon(&map, full("assault rifle")).own.triggers[0].firing_effects.len(), 1);
}

#[test]
fn references_table() {
    let Some(map) = load("bloodgulch") else { return };
    for short in [
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
    ] {
        let w = weapon(&map, full(short));
        let dir = full(short).rsplit_once('\\').unwrap().0;
        let n = short;
        assert_eq!(p(&map, &w.own.melee_damage), Some(format!("{dir}\\melee")), "{n}");
        assert_eq!(p(&map, &w.own.melee_response), Some(format!("{dir}\\melee_response")), "{n}");
        assert_eq!(w.own.melee_damage.class, class::DAMAGE_EFFECT);
        assert_eq!(p(&map, &w.own.first_person_model), Some(format!("{dir}\\fp\\fp")), "{n}");
        assert_eq!(p(&map, &w.own.first_person_animations), Some(format!("{dir}\\fp\\fp")), "{n}");
        assert_eq!(w.own.first_person_model.class, class::MODEL);
        assert_eq!(w.own.first_person_animations.class, class::MODEL_ANIMATIONS);
        let hud = match short {
            "ball" | "flag" => None,
            "rocket launcher" => Some(format!("{dir}\\rocket_launcher")),
            "flamethrower" => Some(format!("{dir}\\flame thrower")),
            _ => Some(format!("{dir}\\{short}")),
        };
        assert_eq!(p(&map, &w.own.hud_interface), hud, "{n} hud");
        assert_eq!(w.own.hud_interface.class, class::WEAPON_HUD_INTERFACE);
    }
    // The turrets: no melee, no first person; a HUD.
    for (short, hud) in [
        ("warthog gun", "vehicles\\warthog\\warthog gun"),
        ("scorpion cannon", "vehicles\\scorpion\\scorpion cannon"),
        ("ghost gun", "vehicles\\ghost\\ghost gun"),
    ] {
        let w = weapon(&map, full(short));
        assert!(w.own.melee_damage.is_null() && w.own.melee_response.is_null(), "{short}");
        assert!(w.own.first_person_model.is_null() && w.own.first_person_animations.is_null(), "{short}");
        assert_eq!(p(&map, &w.own.hud_interface).as_deref(), Some(hud), "{short}");
    }
    // Pickup sounds.
    for (short, snd) in [
        ("assault rifle", Some("ar_ammo")),
        ("pistol", Some("pistol_ammo")),
        ("shotgun", Some("shotgun_ammo")),
        ("sniper rifle", Some("sniper_ammo")),
        ("needler", Some("needle_ammo")),
        ("rocket launcher", Some("rocket_ammo")),
        ("plasma rifle", None),
        ("plasma pistol", None),
        ("flamethrower", None),
    ] {
        let want = snd.map(|s| format!("sound\\sfx\\weapons\\weapon_pickup_ammo\\{s}"));
        assert_eq!(p(&map, &weapon(&map, full(short)).own.pickup_sound), want, "{short}");
    }
    // Zoom sounds of the pistol, sniper rifle and rocket launcher.
    let zoom: Vec<_> = ["pistol", "sniper rifle", "rocket launcher"]
        .iter()
        .map(|s| {
            let w = weapon(&map, full(s)).own;
            (p(&map, &w.zoom_in_sound), p(&map, &w.zoom_out_sound))
        })
        .collect();
    for (a, b) in &zoom {
        assert_eq!(a.as_deref(), Some("sound\\sfx\\weapons\\sniper rifle\\sniper_2x_zoom"));
        assert_eq!(b.as_deref(), Some("sound\\sfx\\weapons\\sniper rifle\\sniper_10x_zoom"));
    }
    // Overheated effects, light effects and times, actor parameters, ready
    // effects.
    assert_eq!(
        p(&map, &weapon(&map, full("plasma rifle")).own.overheated).as_deref(),
        Some("weapons\\plasma rifle\\effects\\overheated")
    );
    assert_eq!(
        p(&map, &weapon(&map, full("plasma pistol")).own.overheated).as_deref(),
        Some("weapons\\plasma pistol\\effects\\overheated")
    );
    let ar = weapon(&map, full("assault rifle")).own;
    assert_eq!(p(&map, &ar.light_power_on_effect).as_deref(), Some("weapons\\assault rifle\\effects\\flashlight"));
    assert_eq!(p(&map, &ar.light_power_off_effect).as_deref(), Some("weapons\\assault rifle\\effects\\flashlight"));
    near(ar.light_power_on_time, 0.1, "light on");
    near(ar.light_power_off_time, 0.8, "light off");
    for (short, actor) in [
        ("warthog gun", "vehicles\\warthog\\warthog gunner"),
        ("ghost gun", "vehicles\\ghost\\ghost pilot"),
        ("scorpion cannon", "vehicles\\scorpion\\scorpion pilot"),
    ] {
        assert_eq!(p(&map, &weapon(&map, full(short)).own.actor_firing_parameters).as_deref(), Some(actor));
    }
    for path in ALL {
        let w = weapon(&map, path).own;
        let ready = match path {
            "weapons\\pistol\\pistol" => Some("weapons\\pistol\\effects\\ready"),
            "weapons\\needler\\needler" => Some("weapons\\needler\\effects\\ready"),
            _ => None,
        };
        assert_eq!(p(&map, &w.ready_effect).as_deref(), ready, "{path}");
        assert!(w.detonation.is_null(), "{path}: no detonation effects");
        if !path.starts_with("vehicles") {
            assert!(w.actor_firing_parameters.is_null(), "{path}");
        }
    }
}

#[test]
fn other_fields() {
    let Some(map) = load("bloodgulch") else { return };
    // Active camo ding and regrowth.
    for (short, ding, regrowth) in [
        ("assault rifle", 0.25, 0.04),
        ("pistol", 0.65, 0.01),
        ("shotgun", 1.0, 0.036),
        ("plasma rifle", 0.3, 0.05),
        ("sniper rifle", 0.55, 0.0),
        ("plasma pistol", 1.0, 0.04),
        ("needler", 0.16, 0.015),
        ("rocket launcher", 1.0, 0.0),
        ("flamethrower", 0.0, 0.0),
        ("ball", 0.0, 0.0),
        ("warthog gun", 0.0, 0.0),
    ] {
        let w = weapon(&map, full(short)).own;
        near(w.active_camo_ding, ding, short);
        near(w.active_camo_regrowth_rate, regrowth, short);
    }
    // Reticle ranges and AI fields.
    for (short, near_r, far_r, inter, min_target, look) in [
        ("warthog gun", 4.0, 16.0, 80.0, 2.0, 3.0),
        ("scorpion cannon", 4.0, 0.0, 0.0, 2.0, 3.0),
        ("ghost gun", 0.0, 0.0, 0.0, 2.0, 0.0),
        ("assault rifle", 0.0, 0.0, 0.0, 0.0, 0.0),
        ("pistol", 0.0, 0.0, 0.0, 0.0, 0.0),
    ] {
        let w = weapon(&map, full(short)).own;
        near(w.near_reticle_range, near_r, short);
        near(w.far_reticle_range, far_r, short);
        near(w.intersection_reticle_range, inter, short);
        near(w.minimum_target_range, min_target, short);
        near(w.looking_time_modifier, look, short);
    }
}

fn is_plausible(map: &MapFile, r: &TagRef, want: &[blam_formats::cache::ClassCode]) {
    if r.is_null() {
        return;
    }
    let t = map.tag(r.id).unwrap_or_else(|| panic!("unresolved {r:?}"));
    assert!(want.iter().any(|c| t.is_a(*c)), "{} is a {}", t.path, t.class);
    assert_eq!(t.class, r.class);
}

#[test]
fn every_weapon_reads_with_plausible_references_on_every_multiplayer_map() {
    let eff = [class::EFFECT, class::SOUND];
    for name in MULTIPLAYER {
        let Some(map) = load(name) else { return };
        assert_eq!(map.of_class(class::WEAPON).count(), 15, "{name}");
        for tag in map.of_class(class::WEAPON) {
            let w = Weapon::read(&map, tag).unwrap_or_else(|e| panic!("{name}: {e}"));
            let o = &w.own;
            is_plausible(&map, &o.ready_effect, &eff);
            is_plausible(&map, &o.overheated, &eff);
            is_plausible(&map, &o.detonation, &eff);
            is_plausible(&map, &o.melee_damage, &[class::DAMAGE_EFFECT]);
            is_plausible(&map, &o.melee_response, &[class::DAMAGE_EFFECT]);
            is_plausible(&map, &o.actor_firing_parameters, &[class::ACTOR_VARIANT]);
            is_plausible(&map, &o.light_power_on_effect, &eff);
            is_plausible(&map, &o.light_power_off_effect, &eff);
            is_plausible(&map, &o.first_person_model, &[class::MODEL]);
            is_plausible(&map, &o.first_person_animations, &[class::MODEL_ANIMATIONS]);
            is_plausible(&map, &o.hud_interface, &[class::WEAPON_HUD_INTERFACE]);
            is_plausible(&map, &o.pickup_sound, &[class::SOUND]);
            is_plausible(&map, &o.zoom_in_sound, &[class::SOUND]);
            is_plausible(&map, &o.zoom_out_sound, &[class::SOUND]);
            assert_eq!(o.flags.unnamed_bits(), 0);
            assert!(o.magazines.len() <= 2 && o.triggers.len() <= 2);
            for m in &o.magazines {
                assert!(m.flags.unnamed_bits() == 0);
                is_plausible(&map, &m.reloading_effect, &eff);
                is_plausible(&map, &m.chambering_effect, &eff);
                for a in &m.magazines {
                    is_plausible(&map, &a.equipment, &[class::EQUIPMENT]);
                    assert!(!a.equipment.is_null());
                }
            }
            for t in &o.triggers {
                assert!(t.flags.unnamed_bits() == 0);
                is_plausible(&map, &t.projectile, &[class::PROJECTILE]);
                assert!(!t.projectile.is_null());
                is_plausible(&map, &t.charging_effect, &eff);
                if let Some(i) = t.magazine_index() {
                    assert!(i < o.magazines.len());
                }
                for e in &t.firing_effects {
                    for r in [&e.firing_effect, &e.misfire_effect, &e.empty_effect] {
                        is_plausible(&map, r, &eff);
                    }
                    for r in [&e.firing_damage, &e.misfire_damage, &e.empty_damage] {
                        is_plausible(&map, r, &[class::DAMAGE_EFFECT]);
                    }
                }
            }
        }
    }
}

#[test]
fn the_weapons_match_on_other_maps() {
    let Some(base) = load("bloodgulch") else { return };
    let want: Vec<(String, String)> = ALL
        .iter()
        .map(|p| (p.to_string(), normalized(&base, &weapon(&base, p))))
        .collect();
    for other in ["hangemhigh", "sidewinder", "chillout"] {
        let Some(map) = load(other) else { return };
        for (path, w) in &want {
            assert_eq!(&normalized(&map, &weapon(&map, path)), w, "{other}: {path}");
        }
    }
}
