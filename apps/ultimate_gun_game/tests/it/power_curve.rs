//! The power curve on the composed games' real guns (backlog "A natural
//! power curve for the random ladder"; `chimera_sim::minigame::power`):
//! each gun's band and score from its data and its game's readers, the
//! `ladder_power` command, and ladders of 10, 20 and 30 rungs that climb
//! warm-up to melee, a pistol first, launchers only among the specials,
//! each band weakest first, deterministic per seed.

use chimera_core::{console::Console, game_parts::GunClass};
use chimera_sim::minigame::{
    auto,
    power::{Band, PowerCurve, Powers},
};

use crate::parts::{powers, ugg};

fn power(p: &Powers, id: &str) -> (Band, f32, f32) {
    let x = p.get(id).unwrap_or_else(|| panic!("{id} has a power"));
    (x.band, x.score, x.ttk)
}

#[test]
fn guns_score_and_band_from_their_data() {
    let sim = ugg();
    let caps = auto::report(sim.app.world());
    let p = powers(sim.app.world(), &caps);
    let band = |id: &str| power(&p, id).0;
    let score = |id: &str| power(&p, id).1;
    assert_eq!(band("cs_source:weapon_glock"), Band::WarmUp);
    assert_eq!(band("combat_arms:beretta_m92fs"), Band::WarmUp);
    assert_eq!(band("cs_source:weapon_mp5navy"), Band::Rising);
    assert_eq!(band("cs_source:weapon_m3"), Band::Rising);
    assert_eq!(band("cs_source:weapon_ak47"), Band::Peak);
    assert_eq!(band("combat_arms:m4a1"), Band::Peak);
    assert_eq!(band("cs_source:weapon_deagle"), Band::Heavy, "a heavy pistol");
    assert_eq!(band("cs_source:weapon_awp"), Band::Heavy);
    assert_eq!(band("combat_arms:m60"), Band::Heavy);
    assert_eq!(band("combat_arms:rpg-7"), Band::Specials);
    assert_eq!(band("combat_arms:flamethrower"), Band::Specials);
    assert_eq!(band("combat_arms:m67"), Band::Grenade);
    // Every gun scores (its data has damage, or the device it places has:
    // the auto turret's gun, `turret_scores_by_its_gun`).
    for c in &caps {
        for g in &c.guns {
            assert!(score(g.id) > 0.0, "{} scores: {:?}", g.id, p.get(g.id));
        }
    }
    // Some orderings any player would agree with.
    assert!(score("cs_source:weapon_ak47") > score("cs_source:weapon_glock"));
    assert!(score("combat_arms:m4a1") > score("combat_arms:beretta_m92fs"));
    assert!(score("cs_source:weapon_usp") > score("cs_source:weapon_glock"));
    assert!(score("cs_source:weapon_p90") > score("cs_source:weapon_tmp"));
    assert!(score("cs_source:weapon_awp") > score("cs_source:weapon_scout"));
    // The console prints the table, by band.
    let mut sim = sim;
    sim.app.world_mut().resource_mut::<Console>().submit("ladder_power");
    sim.ticks(1);
    let out: Vec<String> = sim.app.world().resource::<Console>().output.iter().map(|l| l.text.clone()).collect();
    let text = out.join("\n");
    for id in ["cs_source:weapon_glock", "combat_arms:rpg-7", "combat_arms:m67"] {
        assert!(text.contains(id), "{id} in {text}");
    }
    let warm = text.find("cs_source:weapon_glock").unwrap();
    let special = text.find("combat_arms:rpg-7").unwrap();
    assert!(warm < special, "by band");
}

/// The auto turret deals its damage through the turret it places: its
/// power is the turret's gun's (Combat Arms' reader of `combat_arms:
/// device`), slowed by the placing, the setup and the curve's
/// `device_uptime`; it can hurt a player, so a draw deals it, in the
/// specials band.
#[test]
fn turret_scores_by_its_gun_and_is_drawn() {
    const TURRET: &str = "combat_arms:auto-turret";
    let sim = ugg();
    let mut caps = auto::report(sim.app.world());
    let skipped = auto::drawable(sim.app.world(), &mut caps);
    assert!(skipped.iter().all(|s| s.id != TURRET), "not skipped: {skipped:?}");
    assert!(caps.iter().flat_map(|c| &c.guns).any(|g| g.id == TURRET), "drawable");
    let p = powers(sim.app.world(), &caps);
    let (band, score, ttk) = power(&p, TURRET);
    assert_eq!(band, Band::Specials);
    assert!(score > 0.0 && ttk.is_finite(), "{score} {ttk}");
    // Placing (3.1 s) and setting up (2.8 s) come before its first shot.
    assert!(ttk > 5.9, "{ttk}");
    // Not stronger than a real launcher.
    assert!(score < power(&p, "combat_arms:rpg-7").1, "{score}");
    // A drawn ladder deals it, in the specials band.
    let refs: Vec<&auto::Capabilities> = caps.iter().collect();
    let curve = &PowerCurve::shipped();
    let dealt = (0..400).find_map(|seed| {
        let d = auto::ladder(&refs, &p, curve, 30, seed);
        d.rungs.iter().position(|id| *id == TURRET).map(|k| d.bands[k])
    });
    assert_eq!(dealt, Some(Band::Specials), "a draw deals the turret");
}

#[test]
fn real_ladders_climb_the_bands() {
    let sim = ugg();
    let caps = auto::report(sim.app.world());
    let refs: Vec<&auto::Capabilities> = caps.iter().collect();
    let p = powers(sim.app.world(), &caps);
    let curve = &PowerCurve::shipped();
    let class = |id: &str| caps.iter().flat_map(|c| &c.guns).find(|g| g.id == id).map(|g| g.class);
    for n in [10usize, 20, 30] {
        for seed in 0..40 {
            let d = auto::ladder(&refs, &p, curve, n, seed);
            assert_eq!(d, auto::ladder(&refs, &p, curve, n, seed), "the seed's");
            assert_eq!(d.len(), n, "{d:?}");
            assert!(d.melee >= 1, "melee last");
            assert!(d.bands.windows(2).all(|w| w[0] <= w[1]), "{:?}", d.bands);
            assert_eq!(class(d.rungs[0]), Some(GunClass::Pistol), "a pistol first: {:?}", d.rungs);
            for (k, id) in d.rungs.iter().enumerate() {
                if matches!(class(id), Some(GunClass::Launcher | GunClass::Flamethrower)) {
                    assert_eq!(d.bands[k], Band::Specials, "{id}");
                }
                if k > 0 && d.bands[k - 1] == d.bands[k] {
                    assert!(power(&p, d.rungs[k - 1]).1 <= power(&p, id).1, "weakest first: {:?}", d.rungs);
                }
            }
        }
        assert_ne!(auto::ladder(&refs, &p, curve, n, 1).rungs, auto::ladder(&refs, &p, curve, n, 2).rungs);
    }
}

/// Prints the sample ladders of the report (`cargo test ... -- --nocapture
/// sample_ladders`).
#[test]
fn sample_ladders() {
    let sim = ugg();
    let caps = auto::report(sim.app.world());
    let refs: Vec<&auto::Capabilities> = caps.iter().collect();
    let p = powers(sim.app.world(), &caps);
    for n in [10usize, 20, 30] {
        let d = auto::ladder(&refs, &p, &PowerCurve::shipped(), n, 7);
        println!("{n} rungs (seed 7):");
        for (id, band) in d.rungs.iter().zip(&d.bands) {
            let (_, score, ttk) = power(&p, id);
            println!("  {:<9} {score:>6.3}  ttk {ttk:>5.2} s  {id}", band.name());
        }
        println!("  melee x{}", d.melee);
    }
}
