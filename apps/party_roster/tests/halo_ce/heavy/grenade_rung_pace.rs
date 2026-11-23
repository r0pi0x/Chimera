//! How fast each game's grenade rung lets its holder throw again (the
//! user's wish, 2026-11-22: level the games out): for every grenade the
//! party's games lend a ladder (`auto::report`), its holder on a ladder
//! with `refill_ammo` works the fire button for 30 s (pressed 0.15 s,
//! let go 0.05 s)
//! and the throws' times are read off its weapon events (a `Thrown` of
//! an engine grenade, the `ShotMade` of Halo's rung). The steady gap
//! between throws (the last ones: Halo's rung carries four) lies
//! in the grenade band's range, and each game's matches its
//! `power_curve.toml` `[grenade_supply]` seconds (the throw's own cycle
//! where that is longer). Halo's grenade button throws too.

use bevy::prelude::*;
use chimera_core::core::{ActionRegistry, God};
use chimera_sim::{
    minigame::{auto, power::PowerCurve},
    weapon::{WeaponEvent, WeaponEventKind},
};
use chimera_world::harness::Sim;
use halo_ce::game::weapons::{self, grenades::GRENADE_ACTION, rung::FRAG_RUNG};

use crate::{
    halo_party::party_with,
    play::{held, set_rung, soldier},
};

/// The grenade band's range of steady gaps (s).
const BAND: (f64, f64) = (2.0, 4.0);

fn party(grenade: &str) -> Sim {
    let ladder = ["cs_source:weapon_glock", grenade, weapons::ASSAULT_RIFLE, "combat_arms:mp5a4"];
    let text = crate::play::ladder_text(&ladder, false).replace(
        "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]",
        "weapons = [\"cs_source\", \"hl2dm\", \"halo_ce\", \"combat_arms\"]",
    );
    party_with(&text)
}

/// The grenades the party's games lend a ladder.
fn grenades() -> Vec<&'static str> {
    let sim = party(FRAG_RUNG);
    auto::report(sim.app.world()).iter().flat_map(|c| c.grenades.clone()).collect()
}

/// The throws' times (s) of `grenade`'s rung holder working `press`
/// (the fire button, or Halo's grenade action) for `secs`.
fn throws(grenade: &str, action: bool, secs: f64) -> Vec<f64> {
    let mut sim = party(grenade);
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    set_rung(&mut sim, a, 1);
    sim.app.world_mut().entity_mut(a).insert(God);
    sim.seconds(1.5);
    assert_eq!(held(&sim, a), Some(grenade), "on the grenade rung");
    let bit = sim.app.world().resource::<ActionRegistry>().bit(GRENADE_ACTION);
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let hz = sim.tick_hz();
    let mut out = Vec::new();
    for i in 0..(secs * hz) as u32 {
        // Pressed for 0.15 s, let go for 0.05 s.
        let down = (i as f64 / hz) % 0.2 < 0.15;
        {
            let mut it = sim.intent(a);
            it.pitch = -0.3;
            if action {
                it.actions = if down { bit } else { 0 };
                it.fire = false;
            } else {
                it.fire = down;
            }
        }
        sim.ticks(1);
        let t = i as f64 / hz;
        for m in cursor.read(sim.app.world().resource::<Messages<WeaponEvent>>()) {
            if m.owner == a && !m.replay && matches!(m.kind, WeaponEventKind::Thrown | WeaponEventKind::ShotMade { .. }) {
                out.push(t);
            }
        }
    }
    out
}

/// The steady gap: the mean of the last five gaps (Halo's rung has
/// thrown the four it carried by then).
fn steady(times: &[f64]) -> Option<f64> {
    let gaps: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    (gaps.len() >= 9).then(|| gaps[gaps.len() - 5..].iter().sum::<f64>() / 5.0)
}

#[test]
fn every_games_grenade_rung_throws_again_within_the_band() {
    let supply = PowerCurve::shipped().grenade_supply;
    let mut lines = Vec::new();
    let mut bad = Vec::new();
    for g in grenades() {
        let times = throws(g, false, 30.0);
        let gap = steady(&times);
        lines.push(format!("{g}: {} throws in 30 s, steady gap {gap:?}, supply {} s", times.len(), supply.seconds_for(g)));
        match gap {
            Some(s) if (BAND.0..=BAND.1).contains(&s) && s + 0.15 >= supply.seconds_for(g) as f64 => {}
            _ => bad.push(g),
        }
    }
    println!("{}", lines.join("\n"));
    assert!(bad.is_empty(), "out of the band {BAND:?}: {bad:?}\n{}", lines.join("\n"));
}

#[test]
fn halos_grenade_button_throws_on_the_rung_too() {
    let times = throws(FRAG_RUNG, true, 12.0);
    assert!(times.len() >= 5, "four carried and one back since: {times:?}");
}
