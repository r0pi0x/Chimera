//! Recorded input played again (`games::cs_source::resim`) against the
//! real game: the CS:S probe server's logs of its own runs (never
//! committed: the server's `cstrike/mashup/<name>.in` and `.out`, from
//! movecmp's and weapcmp's runs; `[games.cs_source_server]` in
//! chimera.local.toml) on de_dust2 from the install. A pistol fired 120
//! times while strafing (`spread`) and five recoil taps (`recoil`, the
//! bot's USP silenced; it goes to its knife once the clip is empty) match
//! tick for tick: place, clip, accuracy penalty, the shots' commands; so
//! does a walk and stop down de_dust2's terrain (`walk_stop`; its ground
//! snap once missed a strip edge, 0.35 units for a tick). Skipped without
//! the logs or the install.

use std::sync::Arc;

use crate::stack::{
    games::{
        self,
        cs_source::{
            self,
            movement::SourceMovementPlugin,
            resim::{self, Options, Recording},
            weapons::CsWeaponsPlugin,
        },
    },
    harness::{CsSim, Sim},
    map::{MapDebugView, MapPlugin},
    mount::config::LocalConfig,
};

fn probe(name: &str) -> Option<Recording> {
    let config = LocalConfig::load().ok()?;
    let installed = config
        .game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref())
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let logs = config
        .game_path("cs_source_server")
        .map(|p| p.join("cstrike").join("mashup"));
    let read = |ext: &str| std::fs::read_to_string(logs.as_ref()?.join(format!("{name}.{ext}"))).ok();
    match (installed, read("in"), read("out")) {
        (true, Some(input), Some(output)) => Some(Recording::from_probe(name, "de_dust2", &input, &output).unwrap()),
        _ => {
            eprintln!("skipping: no CS:S install or no probe logs for {name}");
            None
        }
    }
}

fn run(rec: &Recording) -> resim::Report {
    let map = Arc::new(games::load_map_lean("cs_source:de_dust2").expect("de_dust2"));
    let make = || {
        let map = MapPlugin {
            data: Some(map.clone()).into(),
            view: MapDebugView::Normal,
        };
        Sim::new(CsSim, (map, SourceMovementPlugin, CsWeaponsPlugin))
    };
    let r = resim::resimulate(&make, rec, Options::default()).unwrap();
    println!("{r}");
    r
}

#[test]
fn the_probes_pistol_runs_replay_tick_for_tick() {
    for name in ["spread", "recoil"] {
        let Some(rec) = probe(name) else { return };
        let r = run(&rec);
        assert_eq!(r.compared, rec.commands.len(), "{name}");
        assert!(r.first.is_none(), "{name}: {:?}", r.first);
        assert!(r.max_inaccuracy < 1e-4, "{name}: {}", r.max_inaccuracy);
        assert!(r.shots.recorded > 0, "{name}");
        assert_eq!(
            r.shots.same_tick, r.shots.recorded,
            "{name}: {:?}",
            r.shots.first_mismatch
        );
        assert_eq!(r.shots.ours, r.shots.recorded, "{name}");
    }
}

#[test]
fn the_probes_walk_replays_tick_for_tick() {
    let Some(rec) = probe("walk_stop") else { return };
    let r = run(&rec);
    assert_eq!(r.compared, rec.commands.len());
    assert!(r.first.is_none(), "{:?}", r.first);
    assert!(r.max_position < 0.01, "{}", r.max_position);
    assert_eq!((r.ground, r.ducked), (0, 0));
}
