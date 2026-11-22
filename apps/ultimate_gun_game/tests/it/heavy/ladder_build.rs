//! A new ladder costs no frame (backlog "Nothing blocks the main thread";
//! docs/performance.md, "A death's frame, a new ladder's frame"): the
//! automatic gun game
//! with every game's install (Combat Arms' thousands of guns in the
//! catalogue), headless on the greybox. What the command's frame of
//! `ugg_new` and a rotation run (the definition made again, `generate`;
//! the next match's ladder drawn for the asset offer, `ladder::draw`) and
//! what a match's start runs (`minigame::start`: the ladder drawn again
//! with its rungs' extras) are counted in instructions (`metrics::work`,
//! the same on every run) and held under `MADE`, `DRAWN` and `STARTED`,
//! but a session's first (what the data works out once: the catalogue's
//! weapons in one list, their powers, the character data).
//! Prints them (`--nocapture`). Passes with a note without the installs or
//! hardware counters.

use chimera_core::{metrics::work, mount::config::LocalConfig};
use chimera_sim::{
    minigame::{self, Minigames, ladder},
    weapon::data::WeaponCatalog,
};
use ultimate_gun_game::{MINIGAME, RUNGS, generate};

use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{game::data::CaData, harness::CaSimWith};
use party_roster::PartyPartsPlugin;
use ultimate_gun_game::UltimateGunGamePlugin;

/// `generate`: the definition from the games' parts (measured 2026-11-14:
/// 21 M, was 100 M; docs/performance.md).
const MADE: u64 = 45_000_000;
/// `ladder::draw`: the next ladder, nothing changed (15-16 M, one seed 33 M; was 69-73 M).
const DRAWN: u64 = 50_000_000;
/// `minigame::start`: the match's start (16 M, one seed 33 M; was 71-75 M).
const STARTED: u64 = 50_000_000;

fn combat_arms_installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config
        .game_path(combat_arms::game::GAME)
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
}

#[test]
fn a_new_ladder_costs_no_frame() {
    if !combat_arms_installed() {
        eprintln!("skipping: needs the Combat Arms install (its guns are most of the catalogue)");
        return;
    }
    // The install's data, as the apps' (`CaSim` is the built-in guns only).
    let mut sim = Sim::new(
        CaSimWith(CaData::install()),
        (GreyboxMapPlugin, PartyPartsPlugin, UltimateGunGamePlugin::default()),
    );
    sim.repeatable();
    sim.ticks(5);
    let weapons: usize = sim.app.world().resource::<WeaponCatalog>().sets.iter().map(|s| s.weapons.len()).sum();
    if work::now().is_none() {
        eprintln!("skipping: no hardware counters");
        return;
    }
    let count = |f: &mut dyn FnMut()| {
        let a = work::now().unwrap();
        f();
        work::now().unwrap() - a
    };
    let (mut made, mut drawn, mut started) = (Vec::new(), Vec::new(), Vec::new());
    for seed in 1..=5u64 {
        let w = sim.app.world_mut();
        made.push(count(&mut || {
            generate(w, seed, RUNGS).expect("a ladder");
        }));
        let def = w.resource::<Minigames>().get(MINIGAME).expect("the gun game");
        let mut rungs = 0;
        drawn.push(count(&mut || rungs = ladder::draw(w, def).expect("a ladder drawn").rungs.len()));
        assert!(rungs > 0, "seed {seed}: an empty ladder");
        started.push(count(&mut || {
            chimera_core::gameplay::new_game(w);
            minigame::start(w, MINIGAME).expect("the match starts");
        }));
        sim.ticks(20);
    }
    let m = |v: &[u64]| v.iter().map(|n| format!("{:.1}", *n as f64 / 1e6)).collect::<Vec<_>>().join(" / ");
    eprintln!(
        "{weapons} weapons in the catalogue; five ladders (the first the session's first), M instructions: the \
         definition made {}; the ladder drawn {}; the match started {}",
        m(&made),
        m(&drawn),
        m(&started)
    );
    for (what, counts, limit) in [("generate", &made, MADE), ("ladder::draw", &drawn, DRAWN), ("minigame::start", &started, STARTED)] {
        let worst = *counts[1..].iter().max().unwrap();
        assert!(worst < limit, "{what} costs {worst} instructions (limit {limit})");
    }
}
