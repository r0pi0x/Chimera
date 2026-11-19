//! Head and body damage of five stock Combat Arms guns (rifle, SMG,
//! sniper, shotgun, pistol) against the spec's numbers
//! (weapon_firing.md "Hit zones", Test case 4; weapon_classes.md 5.3,
//! case 22): base damage x range band x zone factor (head 6, torso 1),
//! no roll, no spread, an unarmoured dummy. A regression check that a
//! head is worth six bodies and no more, and that the neck and chest
//! spheres are torso hits.

use bevy::prelude::*;
use chimera_sim::weapon::data::WeaponDataAppExt;
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::harness::CaSim;

use super::weapon_firing::{close, place_at, range_lane, shoot_at};

/// (gun, base damage per pellet, pellets, band edges, band multipliers)
const GUNS: [(&str, f32, u32, [f32; 3], [f32; 3]); 5] = [
    ("m4a1", 35.0, 1, [2500.0, 3000.0, 3500.0], [1.0, 0.7, 0.35]),
    ("mp5a4", 29.0, 1, [500.0, 2000.0, 2500.0], [1.1, 0.7, 0.35]),
    ("l96a1", 160.0, 1, [4000.0, 4500.0, 5000.0], [1.0, 0.7, 0.35]),
    ("remington_870", 9.0, 8, [600.0, 800.0, 1000.0], [1.0, 0.7, 0.35]),
    ("beretta_m92fs", 26.0, 1, [1000.0, 2000.0, 2500.0], [1.0, 0.7, 0.35]),
];

/// `id` with no roll, no spread and no kick (zero cone), range lengthened
/// so a band edge's part is reached.
fn exact(id: &str) -> String {
    format!(
        r#"
id = "test:{id}_exact"
base = "combat_arms:{id}"
[hooks."combat_arms:damage"]
roll = 0.0
[hooks."combat_arms:spread"]
cone = {{ min = 0, max = 0, inc = 1.0, dec = 1.0 }}
[hooks."combat_arms:recoil"]
pitch = 0.0
max_pitch = 0.0
random_pitch = 0.0
wag_r = 0.0
wag_l = 0.0
max_wag = 0.0
"#
    )
}

/// The band multiplier at `d` (the data's integer edges).
fn band(edges: [f32; 3], mult: [f32; 3], d: f32) -> f32 {
    match edges.iter().position(|e| d <= *e) {
        Some(i) => mult[i],
        None => 1.0,
    }
}

#[test]
fn head_and_body_damage_by_gun_and_range() {
    let mut table = String::from("gun, part, range, measured, spec\n");
    for (gun, base, pellets, edges, mult) in GUNS {
        let doc = exact(gun);
        let id = format!("test:{gun}_exact");
        // Ranges well inside each band's middle (the part must be reached).
        for d in [300.0, (edges[0] + edges[1]) / 2.0] {
            // A fresh lane per range: a dummy left in the line would be
            // the round's first victim.
            let mut sim = Sim::new(CaSim, GreyboxMapPlugin);
            sim.set_tick_interval(combat_arms::game::TICK_INTERVAL);
            sim.app.add_weapon_data(&[(gun, doc.as_str())]).unwrap();
            super::weapon_corrections::steady_surfaces(&mut sim);
            let (shooter, target) = range_lane(&mut sim, &id, d);
            for (part, zone) in [
                ("Face_Eyelid1", 6.0),
                ("Torso", 1.0),
                ("Upper_torso", 1.0),
                ("Neck", 1.0),
            ] {
                // The slowest gun's cycle (bolt 1.4 s, pump) between rounds.
                sim.seconds(2.0);
                place_at(&mut sim, shooter, target, part, d);
                let got = shoot_at(&mut sim, shooter, target, part);
                // The shotgun's 7th pellet is dropped (weapon_classes.md 5.x,
                // case 22's correction: t22 in weapon_classes.rs).
                let landing = if pellets > 1 { pellets - 1 } else { 1 };
                let want = base * landing as f32 * band(edges, mult, d) * zone;
                table += &format!("{gun}, {part}, {d}, {got:.2}, {want:.2}\n");
                // Pellets summed in f32: a looser tolerance.
                close(got, want, 1e-2 * pellets as f32, &format!("{gun} {part} at {d}"));
            }
        }
    }
    println!("{table}");
}
