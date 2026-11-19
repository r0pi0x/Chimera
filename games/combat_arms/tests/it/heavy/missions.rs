//! The mission data of the user's Combat Arms install
//! (specs/combat_arms/missions.md, cases 41 and 42 and the reader on every
//! file), through our own `.rez` reader with the V1 key. Passes with a note
//! without an install; without the key only the archive's directory is
//! checked. Nothing read here is written anywhere.

use chimera_core::mount::{FileSource, config::LocalConfig};
use combat_arms::game::{
    GAME,
    missions::{self, Mission},
    mount, world,
};
use lithtech_formats::{attributes::Document, rez::RezArchive};

fn gms_rez() -> Option<std::path::PathBuf> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    let path = install.join("Game").join("GMS.rez");
    path.is_file().then_some(path)
}

/// Case 41: the archive's directory (plain), then (case 42) every file
/// parses, and the reader takes every mission file.
#[test]
fn install_mission_files() {
    let Some(path) = gms_rez() else { return };
    let keys = mount::keys().unwrap();
    let archive = RezArchive::open(&path, &keys).unwrap();
    let entries = archive.entries();
    let size = |p: &str| entries.iter().find(|e| e.path == p).map(|e| e.size);
    assert_eq!(entries.len(), 820);
    assert!(entries.iter().all(|e| e.path.starts_with("gms/")));
    assert_eq!(size("gms/ghosttown_t.gms"), Some(1404));
    assert_eq!(size("gms/vtdm.gms"), Some(448));
    assert_eq!(size("gms/showdown_t.gms"), Some(1390));
    if archive.locked() {
        eprintln!("no V1 key: only the directory was checked");
        return;
    }
    let mut read = 0;
    let mut modes = std::collections::BTreeMap::<String, usize>::new();
    let mut failed = Vec::new();
    for e in &entries {
        let bytes = archive.read(&e.path).unwrap().unwrap();
        let doc = match Document::parse(&bytes) {
            Ok(d) => d,
            Err(err) => {
                failed.push(format!("{}: {err}", e.path));
                continue;
            }
        };
        if doc.section("MapInfo").is_none() {
            continue;
        }
        match Mission::read(&e.path, &doc) {
            Ok(m) => {
                read += 1;
                *modes.entry(m.mission.mode.clone()).or_default() += 1;
            }
            Err(err) => failed.push(err),
        }
    }
    eprintln!("{read} mission files; by mode: {modes:?}");
    // One Steam file doesn't parse (a value missing at its line 886); the
    // room data names no mode with it.
    assert_eq!(failed.len(), 1, "{failed:#?}");
    assert!(failed[0].starts_with("gms/snowstorm_boss.gms"), "{failed:#?}");
    assert!(read > 700, "{read}");
}

/// The (map, mode) index from the room data, and the values the modes
/// read on the Steam build (25595979). Steam's Ghost Town S&D round is
/// 120 s (the older client's 180 s, missions.md Stock values and Open
/// question 1).
#[test]
fn install_missions_for_the_modes() {
    if gms_rez().is_none() {
        return;
    }
    let Some(mount) = world::mount() else { return };
    let i = missions::read_install(&|p| mount.read(p).ok());
    let ca = combat_arms::game::data::CaData::install();
    if i.index.paths.is_empty() {
        eprintln!("no V1 key: no mission data");
        return;
    }
    eprintln!("{} (map, mode) pairs", i.index.paths.len());
    let tdm = i.index.find("warehouse", "TDM").expect("Showdown TDM");
    assert_eq!(tdm.path, "gms/showdown_t.gms");
    assert_eq!((tdm.mission.mode.as_str(), tdm.object().time_limit), ("TDM", 0));
    let sd = i.index.find("ghosttown", "TMM").expect("Ghost Town S&D");
    let o = sd.object();
    assert_eq!((o.time_limit, o.training_time_limit), (120, 10));
    assert_eq!(o.jsf_weapon, vec!["XK-2"]);
    assert_eq!((o.offence, o.defence), (missions::JSF, missions::CREED));
    assert_eq!(missions::round_length("TMM", o.time_limit, 5), Some(120.0));
    let (lt, rb) = o.minimap.as_ref().unwrap().rect().unwrap();
    assert!(
        (lt[0] + 6828.892).abs() < 1e-3 && (rb[2] + 7866.243).abs() < 1e-3,
        "case 33 on the install"
    );
    let q = i.index.find("ghosttown", "QRT").expect("Ghost Town Quarantine");
    let qd = q.quarantine.clone().unwrap();
    assert_eq!(q.object().time_limit, 3);
    assert_eq!(qd.min_hosts, 2);
    assert!((qd.selection_rate - 0.2).abs() < 1e-6);
    let (tick, heal, wait) = qd.heal.unwrap();
    assert!((tick - 1.0).abs() < 1e-6 && (heal - 0.1).abs() < 1e-6 && (wait - 3.0).abs() < 1e-6);
    assert_eq!(qd.infected.unwrap().weapons, vec![191]);
    assert_eq!(qd.host.unwrap().weapons, vec![190]);
    eprintln!("Quarantine points on Steam: {:?}", qd.points);
    assert_eq!(i.vip.clone().unwrap(), missions::VipData::stock(), "VTDM.GMS");
    let server = i.server.clone().expect("SERVER.CFG");
    assert_eq!(
        (server.object_start, server.object_end, server.mission_end),
        (0.0, 3.0, 3.0)
    );
    if let Some(ar) = i.index.find("ghosttown", "VWRM") {
        let g15 = missions::ladders_for(&ar.ladders, &ar.default_ladders, 15);
        let steps: Vec<(i32, Vec<(i32, i32)>)> = g15
            .iter()
            .find(|t| t.item_no == -1)
            .map(|t| t.steps.iter().map(|s| (s.kill, s.candidates.clone())).collect())
            .unwrap_or_default();
        eprintln!("Arms Race G 15 on Steam: {steps:?}");
        if let Some(w) = ca.attributes().and_then(|a| a.weapons.as_ref()) {
            let name = |i: i32| w.weapons.get(i as usize).map(|r| r.name.clone());
            let names: Vec<_> = steps
                .iter()
                .map(|(_, c)| (c[0].0, name(c[0].0), ca.weapon_for_index(c[0].0)))
                .collect();
            eprintln!("  as weapon records: {names:?}");
            // The ids are weapon record indices (missions.md Open question
            // 6): every step names a weapon our set has with the install.
            assert!(
                names.iter().all(|(_, n, ours)| n.is_some() && ours.is_some()),
                "{names:?}"
            );
            assert_eq!(names[0].1.as_deref(), Some("ACR_Sandfire"));
            let ours = |i| ca.weapon_for_index(i);
            assert_eq!(ours(191), Some("combat_arms:quarantine_virus_hand"), "the infected's");
            assert_eq!(ours(190), Some("combat_arms:quarantine_host_hand"), "the host's");
            assert_eq!(ours(934), Some("combat_arms:fist"), "Hunted's start");
        }
        assert!(!g15.is_empty(), "the default file's ladders");
    }
    // Case 6 (Quirk 1) on Steam: Hired Guns 2's Hired Guns room plays
    // Squad Match 2.
    let sq = i.index.find("hiredguns2", "SQM").expect("Hired Guns 2's SQM");
    assert_eq!(
        (sq.path.as_str(), sq.mission.mode.as_str()),
        ("gms/hiredguns2_hqm.gms", "SQM2")
    );
    // The map messages the rules send (S15): GRAYHAMMER's S&D and CTF
    // start triggers at the setup, their end triggers at round over.
    for (key, level) in [("TMM", "Level02"), ("CTF", "Level05")] {
        let m = i.index.find("grayhammer", key).expect("GRAYHAMMER's mission");
        let o = m.object();
        let names = |step| {
            m.map_messages(step, &o)
                .into_iter()
                .map(|(n, msg)| format!("{n} {msg}"))
                .collect::<Vec<_>>()
        };
        eprintln!(
            "grayhammer {key}: mission start {:?}",
            names(missions::MapStep::MissionStart)
        );
        assert!(
            names(missions::MapStep::RoundSetup).contains(&format!("{level}StartTrigger TRIGGER")),
            "{key}"
        );
        assert!(
            names(missions::MapStep::RoundOver).contains(&format!("{level}EndTrigger TRIGGER")),
            "{key}"
        );
    }
    for (key, map) in [
        ("HQM", "ghosttown"),
        ("BNT", "ghosttown"),
        ("SBT", "ghosttown"),
        ("CTF", "ghosttown"),
        ("LMS", "ghosttown"),
    ] {
        if let Some(m) = i.index.find(map, key) {
            let o = m.object();
            eprintln!(
                "{map} {key}: TimeLimit {} TrainingTimeLimit {} weapons {:?}/{:?}",
                o.time_limit, o.training_time_limit, o.jsf_weapon, o.creed_weapon
            );
        }
    }
}

/// The Steam mission of Ghost Town's S&D played on the greybox: its
/// 120 s rounds after its 10 s warm-up, the defence (Creed) winning a
/// time-out.
#[test]
fn install_mission_plays() {
    use crate::mode_kit::*;
    use combat_arms::game::rules::Phase;
    if gms_rez().is_none() {
        return;
    }
    let Some(mount) = world::mount() else { return };
    let i = missions::read_install(&|p| mount.read(p).ok());
    let Some(sd) = i.index.find("ghosttown", "TMM") else {
        eprintln!("no V1 key: no mission data");
        return;
    };
    let mut sim = game("TMM", 2);
    sim.app.world_mut().insert_resource(missions::CaMission(Some(sd)));
    join(&mut sim, JSF);
    join(&mut sim, CREED);
    until_live(&mut sim);
    assert!(m(&sim).warmup);
    assert!((m(&sim).round_ends.unwrap() - now(&sim) - 10.0).abs() < 0.1);
    until(&mut sim, 20.0, |p| matches!(p, Phase::Setup { .. }));
    until_live(&mut sim);
    assert!(
        (m(&sim).round_ends.unwrap() - now(&sim) - 120.0).abs() < 0.1,
        "Steam's 120 s"
    );
    until(&mut sim, 125.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).winner, Some(CREED));
}
