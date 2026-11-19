//! specs/combat_arms/missions.md's test cases (`case_01_` is test case 1),
//! on mission documents written here in the attribute syntax (the field
//! names are the data's; the values the spec's), the reader
//! (`game::missions`) and the modes on `CaSim`. Cases 41 and 42 read the
//! install: `heavy::missions`.

use std::{collections::HashMap, sync::Arc};

use bevy::prelude::*;
use chimera_world::harness::Sim;
use combat_arms::game::{
    missions::{self, CaMission, Mission, MissionIndex, ServerFile, VipData},
    rules::{CaMatch, CaRuleSettings, Phase, objectives::BOMB},
};
use lithtech_formats::attributes::Document;

use crate::mode_kit::*;

fn doc(text: &str) -> Document {
    Document::parse(text.as_bytes()).unwrap_or_else(|e| panic!("{e}"))
}

fn mission(text: &str) -> Mission {
    Mission::read("gms/test.gms", &doc(text)).unwrap()
}

/// A mission file of `mode` on `world` with an `[Object0]` of `object`.
fn file(world: &str, mode: &str, object: &str) -> String {
    format!(
        "[MapInfo]\nMap = \"{world}\"\nName = \"x\"\n[Mission0]\nMissionMode = \"{mode}\"\nStartObject = 0\n[Object0]\n{object}\n"
    )
}

/// The room data and the mission files as the install's archive reads
/// them (normalized paths).
fn install() -> HashMap<String, String> {
    let mut files = HashMap::new();
    files.insert(
        missions::ROOM_FILE.into(),
        "[Map0]\nKey = \"GhostTown\"\nGMS_TDM = \"GMS\\GhostTown_T.GMS\"\nGMS_TMM = \"GMS\\GhostTown_M.GMS\"\n\
         GMS_TMM_OFFICIAL = \"GMS\\GhostTown_M_OFFICIAL.GMS\"\n\
         [Map1]\nKey = \"Warehouse\"\nGMS_TSV = \"GMS\\Showdown_TSV.GMS\"\nGMS_FFA = \"GMS\\Showdown_F.GMS\"\n\
         [Map2]\nKey = \"HiredGuns2\"\nGMS_SQM = \"GMS\\HiredGuns2_HQM.GMS\"\n"
            .into(),
    );
    let w = "Worlds\\GhostTown.dat";
    files.insert("gms/ghosttown_t.gms".into(), file(w, "TDM", "JSFSpawn = 1"));
    files.insert(
        "gms/ghosttown_m.gms".into(),
        file(w, "TMM", "TimeLimit = 180\nTrainingTimeLimit = 10\nJSFWeapon = \"XK-2\"\nOffenceTeam = \"JSF\"\nDefenceTeam = \"Creed\""),
    );
    files.insert("gms/ghosttown_m_official.gms".into(), file(w, "TMM", "TimeLimit = 180"));
    files.insert(
        "gms/showdown_tsv.gms".into(),
        file("Worlds2\\Warehouse.dat", "TSV", "TimeLimit = 180"),
    );
    files.insert("gms/showdown_f.gms".into(), file("Worlds2\\Warehouse.dat", "FFA", ""));
    files.insert(
        "gms/hiredguns2_hqm.gms".into(),
        file("Worlds4\\HiredGuns2.dat", "SQM2", "TimeLimit = 1800"),
    );
    files
}

fn index() -> MissionIndex {
    let files = install();
    MissionIndex::build(&|p| files.get(p).map(|s| s.as_bytes().to_vec()))
}

#[test]
fn case_01_room_to_path() {
    let m = index().find("ghosttown", "TDM").unwrap();
    assert_eq!(
        (m.path.as_str(), m.mission.mode.as_str()),
        ("gms/ghosttown_t.gms", "TDM")
    );
    assert_eq!(m.map, "Worlds\\GhostTown.dat");
    assert_eq!(m.world_name(), "ghosttown");
}

#[test]
fn case_02_official_room() {
    let m = index().find("ghosttown", "TMM_OFFICIAL").unwrap();
    assert_eq!(
        (m.path.as_str(), m.mission.mode.as_str()),
        ("gms/ghosttown_m_official.gms", "TMM")
    );
}

#[test]
fn case_03_04_world_differs_from_the_map() {
    let i = index();
    assert_eq!(i.find("warehouse", "TSV").unwrap().path, "gms/showdown_tsv.gms");
    let f = i.find("warehouse", "FFA").unwrap();
    assert_eq!(
        missions::round_length(&f.mission.mode, f.object().time_limit, 30),
        None,
        "no time limit"
    );
}

#[test]
fn case_05_mode_not_listed() {
    assert!(index().find("warehouse", "TMM").is_none());
    assert_eq!(index().keys_of("warehouse"), vec!["FFA", "TSV"]);
}

#[test]
fn case_06_the_file_names_the_mode() {
    let m = index().find("hiredguns2", "SQM").unwrap();
    assert_eq!(m.mission.mode, "SQM2", "Quirk 1: Squad Match 2");
    assert_eq!(missions::round_length("SQM2", m.object().time_limit, 12), Some(1800.0));
}

/// Case 7: TDM has no round timer and a 0 s setup whatever `ObjectStart`.
#[test]
fn case_07_tdm_timer_and_setup() {
    let mut sim = game("TDM", 2);
    settings(&mut sim).object_start = 5.0;
    join(&mut sim, JSF);
    join(&mut sim, CREED);
    until_live(&mut sim);
    assert!(now(&sim) < 0.2, "setup 0 s: live at {}", now(&sim));
    assert_eq!(m(&sim).round_ends, None);
}

/// Cases 8 and 9: S&D's warm-up of 10 s, then setups of `ObjectStart`
/// and 180 s rounds; bombs only from round 1; a time-out goes to Creed.
#[test]
fn case_08_09_search_and_destroy_clock() {
    let mut sim = game("TMM", 2);
    settings(&mut sim).object_start = 5.0;
    let a = join(&mut sim, JSF);
    join(&mut sim, CREED);
    until(&mut sim, 7.0, |p| p == Phase::Live);
    let m0 = m(&sim);
    assert!(m0.warmup);
    let ends = m0.round_ends.unwrap();
    assert!((ends - now(&sim) - 10.0).abs() < 0.1, "warm-up 10 s");
    assert!(!weapons(&sim, a).contains(&BOMB), "no bomb in the warm-up");
    until(&mut sim, 25.0, |p| matches!(p, Phase::Setup { .. }));
    let Phase::Setup { until: setup_end } = phase(&sim) else {
        unreachable!()
    };
    assert!((setup_end - now(&sim) - 5.0).abs() < 0.1, "5 s setup");
    until(&mut sim, 7.0, |p| p == Phase::Live);
    let ends = m(&sim).round_ends.unwrap();
    assert!((ends - now(&sim) - 180.0).abs() < 0.1, "180 s round");
    assert!(
        wait_for(&mut sim, 1.0, |s| weapons(s, a).contains(&BOMB)),
        "the bomb from round 1"
    );
    until(&mut sim, 190.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).winner, Some(CREED), "the defence on a time-out");
}

#[test]
fn case_10_11_capture_the_flag_clocks() {
    for (g, t) in [(3, 540.0), (5, 900.0), (7, 1260.0)] {
        assert_eq!(missions::round_length("CTF", 180, g), Some(t));
    }
    assert_eq!(missions::round_length("CCTF", 600, 5), Some(600.0));
    assert_eq!(missions::warmup_length("CCTF", 600, 10, 5), Some(10.0));
    assert!(!missions::has_warmup("CTF"));
}

#[test]
fn case_12_quarantine_minutes() {
    assert_eq!(missions::round_length("QRT", 3, 7), Some(180.0));
    assert_eq!(missions::warmup_length("QRT", 3, 10, 7), Some(10.0));
}

#[test]
fn case_13_14_spy_hunt_minutes() {
    assert_eq!(missions::round_length("BNT", 10, 12), Some(720.0));
    assert_eq!(missions::round_length("BNT", 10, 0), Some(600.0));
    assert_eq!(missions::round_length("BNT", 0, 0), None);
}

#[test]
fn case_15_16_seize_and_secure() {
    assert_eq!(missions::round_length("HQM", 0, 8), Some(480.0));
    assert_eq!(missions::warmup_length("HQM", 0, 10, 8), Some(10.0));
    assert_eq!(missions::warmup_length("HQM", 0, 0, 8), Some(10.0), "case 16");
    let m = mission(
        "[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"HQM\"\n[Object0]\nTrainingTimeLimit = 10\n\
         ActivateTriggerName0 = \"ActivateTrigger_A\"\nActivateTriggerName1 = \"ActivateTrigger_B\"\n\
         ActivateTriggerTimeAmount0 = 5.0\nActivateTriggerTimeAmount1 = 5.0\n\
         [HeadQuatersMode]\nFlagMoveTime = 120.0\nLastOccupyBonus = 50\nAbuse_PlayTime_Plus = 10.0\n",
    );
    let o = m.object();
    assert_eq!(
        o.activate,
        vec![("ActivateTrigger_A".into(), 5.0), ("ActivateTrigger_B".into(), 5.0)]
    );
    let s = m.seize.unwrap();
    assert_eq!((s.flag_move_time, s.last_occupy_bonus), (120.0, 50));
    assert_eq!(s.flag_move_time + s.abuse_play_plus, 130.0, "the anti-abuse timer");
}

#[test]
fn case_17_warmup_falls_back_to_the_round() {
    assert_eq!(missions::warmup_length("TMM", 180, 0, 5), Some(180.0));
}

#[test]
fn case_18_hunted() {
    assert_eq!(missions::warmup_length("HG", 270, 20, 5), Some(20.0));
    assert_eq!(missions::round_length("HG", 270, 5), Some(270.0));
    assert!(!missions::setup_forced("HG"));
}

/// Case 19: Bombing Run's taker gets their own team's field (`XK-2`).
#[test]
fn case_19_bombing_run_weapons() {
    let sim = game("SBT", 1);
    let w = sim.app.world();
    assert_eq!(missions::objective_weapons(w, CREED), vec![BOMB]);
    assert_eq!(missions::objective_weapons(w, JSF), vec![BOMB]);
    assert_eq!(missions::round_length("SBT", 300, 5), Some(300.0));
}

#[test]
fn case_20_vip_escort() {
    let m = mission(
        "[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"ESC\"\nVIPSpawn = \"VipTeleportPoint\"\n[Object0]\nTimeLimit = 180\n\
         TrainingTimeLimit = 10\nJSFWeapon = \"XK-2\"\nEscapeZoneTrigger = \"EscapeZoneTrigger\"\nTimerTrigger = \"VipExitTimer\"\n\
         [VIPChangeCharInfo]\nModelIndex = 0\nWeaponIndex = 0\n[VIPChangeCharModel_0]\nModelID = 87\nHUDIndex = 0\n\
         [VIPChangeCharWeapon_0]\nPrimary0 = 91\nSecondary0 = 0\n",
    );
    assert_eq!(m.mission.vip_spawn, "VipTeleportPoint");
    let v = m.vip_escort.clone().unwrap();
    assert_eq!((v.model, v.weapons[0], v.hud_index), (87, 91, 0));
    assert_eq!(m.object().timer_trigger, "VipExitTimer");
}

#[test]
fn case_21_22_hired_guns() {
    let m = mission(
        "[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"SQM\"\n[Object0]\nTimeLimit = 300\n[BullionManage]\n\
         DropBullionNum = 80\nBullionAmount = 100\nBaseBankAmount = 1000\nRoundWinBonusAmount = 500\nKillBonusAmount = 100\n\
         DeathBonusAmount = 0\nMovePenaltyIncPerBullionNumSection0 = [0, 10]\nMovePenaltyIncPerBullionNum0 = 0.0\n\
         MovePenaltyIncPerBullionNumSection1 = [11, 25]\nMovePenaltyIncPerBullionNum1 = 0.03\n\
         MovePenaltyIncPerBullionNumSection2 = [26, 80]\nMovePenaltyIncPerBullionNum2 = 0.0\n",
    );
    let b = m.bullion.unwrap();
    assert_eq!(
        (
            b.drop_num,
            b.amount,
            b.base_bank,
            b.round_win_bonus,
            b.kill_bonus,
            b.death_bonus
        ),
        (80, 100, 1000, 500, 100, 0)
    );
    assert_eq!(b.base_bank + b.round_win_bonus, 1500, "the last round's winners");
    for (n, p) in [(10.0, 0.0), (20.0, 0.30), (25.0, 0.45), (40.0, 0.45), (81.0, 0.0)] {
        assert!((b.carry_penalty(n) - p).abs() < 1e-9, "P({n}) = {}", b.carry_penalty(n));
    }
}

#[test]
fn case_23_24_25_vip_counts() {
    let v = VipData::stock();
    assert_eq!(v.vips(140, 6), 2);
    assert_eq!((v.weight(2), v.weight(1), v.weight(0)), (5, 3, 1));
    assert_eq!(v.vips(140, 8), 3);
    assert_eq!((v.weight(3), v.weight(2), v.weight(1)), (7, 5, 3));
    assert_eq!(v.vips(140, 1), 0);
    assert_eq!(v.vips(60, 8), 2, "case 25: table 0");
    let read = VipData::read(&doc(
        "[VIPElimination_Table0]\nTargetPoint = 60\nVIPNumber6 = 1\nVIPNumber7 = 1\nVIPNumber8 = 2\n\
         [VIPElimination_Table1]\nTargetPoint = 1000\nVIPNumber2 = 1\nVIPNumber3 = 1\nVIPNumber4 = 1\nVIPNumber5 = 2\n\
         VIPNumber6 = 2\nVIPNumber7 = 2\nVIPNumber8 = 3\n[Point_Weight]\nVIPPointWeight0 = 1\nVIPPointWeight1 = 3\n\
         VIPPointWeight2 = 5\nVIPPointWeight3 = 7\n",
    ));
    assert_eq!(read, v);
}

/// `VWRM.GMS`'s ladders as the stock data gives them (G 15's default,
/// the start of G 30's) and a function item's (9063) for G 30.
const LADDERS: &str = "[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"VWRM\"\n\
     [Weapon_Step_Table0]\nGoal = 15\nItemNo = -1\nPercent = 80\nKill0 = 0\nWeaponType0 = 0\nSet0_0 = 325\nSet_rate0_0 = 100\n\
     Kill1 = 4\nWeaponType1 = 1\nSet1_0 = 337\nSet_rate1_0 = 100\nKill2 = 7\nWeaponType2 = 3\nSet2_0 = 35\nSet_rate2_0 = 100\n\
     Kill3 = 10\nWeaponType3 = 26\nSet3_0 = 125\nSet_rate3_0 = 100\nKill4 = 12\nWeaponType4 = 2\nSet4_0 = 257\nSet_rate4_0 = 100\n\
     Kill5 = 13\nWeaponType5 = 4\nSet5_0 = 110\nSet_rate5_0 = 100\nKill6 = 14\nWeaponType6 = 6\nSet6_0 = 84\nSet_rate6_0 = 100\n\
     [Weapon_Step_Table1]\nGoal = 30\nItemNo = -1\nPercent = 80\nKill0 = 0\nWeaponType0 = 0\nSet0_0 = 325\nSet_rate0_0 = 100\n\
     Kill1 = 4\nWeaponType1 = 0\nSet1_0 = 255\nSet_rate1_0 = 100\n\
     [Weapon_Step_Table2]\nGoal = 30\nItemNo = 9063\nPercent = 50\nKill0 = 0\nWeaponType0 = 0\nSet0_0 = 1\nSet_rate0_0 = 1\n\
     Kill1 = 4\nWeaponType1 = 0\nSet1_0 = 723\nSet_rate1_0 = 25\nSet1_1 = 877\nSet_rate1_1 = 25\nSet1_2 = 876\nSet_rate1_2 = 25\n\
     Set1_3 = 847\nSet_rate1_3 = 25\n";

#[test]
fn case_26_to_29_arms_race() {
    let own = mission("[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"VWRM\"\n[Object0]\nTimeLimit = 0\n").ladders;
    assert!(own.is_empty(), "a per-map file has none");
    let defaults = mission(LADDERS).ladders;
    // Case 26: G 15 from the default file; its steps and the last stage.
    let g15 = missions::ladders_for(&own, &defaults, 15);
    let t = &g15[0];
    assert_eq!(
        t.steps.iter().map(|s| s.kill).collect::<Vec<_>>(),
        vec![0, 4, 7, 10, 12, 13, 14]
    );
    assert_eq!(t.step_at(14).unwrap().candidates, vec![(84, 100)]);
    assert_eq!(t.next_kill(14), None, "the last stage");
    // Case 27: G 30, no item: r 79 the table's M417 (255), r 80 your own.
    let g30 = missions::ladders_for(&own, &defaults, 30);
    assert_eq!(missions::ladder_pick(&g30, &[], 4, 79, 50, true), Some(255));
    assert_eq!(missions::ladder_pick(&g30, &[], 4, 80, 50, true), None);
    // Case 28: owning item 9063 with r 49: its table's step 1, by weight
    // (q 30: the second's running sum 50 is the first at or above it).
    assert_eq!(missions::ladder_pick(&g30, &[9063], 4, 49, 30, true), Some(877));
    // Case 29: a goal no table names: no ladder, your own weapons.
    let g20 = missions::ladders_for(&own, &defaults, 20);
    assert!(g20.is_empty());
    assert_eq!(missions::ladder_pick(&g20, &[], 4, 0, 0, true), None);
}

#[test]
fn case_30_coop_difficulty() {
    let m = mission(
        "[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"COOP\"\n[Object0]\nTimeLimit = 1200\nAIDamageFactor = 0.15\nLifeCount = 1\nMaxLifeCount = 1\n\
         [Object1]\nTimeLimit = 1080\nAIDamageFactor = 0.4\nLifeCount = 1\nMaxLifeCount = 1\n\
         [Object2]\nTimeLimit = 900\nAIDamageFactor = 0.8\nLifeCount = 1\nMaxLifeCount = 1\n",
    );
    for (g, (t, f)) in [(0, (1200, 0.15)), (1, (1080, 0.4)), (2, (900, 0.8)), (5, (900, 0.8))] {
        let o = m.coop_object(g);
        assert_eq!(
            (o.time_limit, o.ai_damage_factor, o.life_count, o.max_life_count),
            (t, f, 1, 1),
            "G {g}"
        );
    }
}

#[test]
fn case_31_32_forced_loadout_and_overrides() {
    let m = mission(
        "[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"SFM\"\n[WeaponList]\nWeapon0 = 100\nWeapon1 = 101\nWeapon2 = 102\nWeapon3 = 103\n\
         [WorldObjectData0]\nObjectName = \"FlagA\"\nPropName0 = \"Skin\"\nPropValue0 = \"red\"\n\
         [WorldObjectData1]\nObjectName = \"FlagB\"\nPropName0 = \"Skin\"\nPropValue0 = \"blue\"\n",
    );
    assert_eq!(m.weapon_list, vec![100, 101, 102, 103]);
    assert_eq!(missions::round_length("SFM", m.object().time_limit, 50), None);
    assert_eq!(m.overrides_of("FlagA"), vec![("Skin".into(), "red".into())]);
    assert_eq!(m.overrides_of("FlagB"), vec![("Skin".into(), "blue".into())]);
}

#[test]
fn case_33_minimap() {
    let m = mission(
        "[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"TDM\"\n[Object0]\nLargeMap = \"m\"\nLargeMapWidth = 1024\nLargeMapHeight = 1024\n\
         LargeMapPoint0_2D = (128, 128)\nLargeMapPoint0_3D = <-5120.0, 0.0, 4096.0>\nLargeMapPoint1_2D = (895, 895)\nLargeMapPoint1_3D = <5120.0, 0.0, -6144.0>\n",
    );
    let (lt, rb) = m.object().minimap.unwrap().rect().unwrap();
    let close = |a: f64, b: f64| (a - b).abs() < 1e-3;
    assert!(close(lt[0], -6828.892) && close(lt[2], 5804.892), "{lt:?}");
    assert!(close(rb[0], 6842.243) && close(rb[2], -7866.243), "{rb:?}");
}

/// Case 34: without `OffenceTeam` and `DefenceTeam` the built-in reverse
/// stands: an S&D time-out goes to JSF.
#[test]
fn case_34_built_in_sides() {
    let file = mission(
        "[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"TMM\"\n[Object0]\nTimeLimit = 3\nTrainingTimeLimit = 1\n",
    );
    assert_eq!((file.object().offence, file.object().defence), (CREED, JSF));
    let mut sim = game("TMM", 2);
    sim.app.world_mut().insert_resource(CaMission(Some(Arc::new(file))));
    join(&mut sim, JSF);
    join(&mut sim, CREED);
    until_live(&mut sim);
    until(&mut sim, 10.0, |p| matches!(p, Phase::Setup { .. }));
    until_live(&mut sim);
    until(&mut sim, 5.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).winner, Some(JSF));
}

/// Case 35: `TimeLimit = -1` in TDM is an expired timer.
#[test]
fn case_35_negative_time_limit() {
    let mut sim = game("TDM", 2);
    let file = mission("[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"TDM\"\n[Object0]\nTimeLimit = -1\n");
    sim.app.world_mut().insert_resource(CaMission(Some(Arc::new(file))));
    join(&mut sim, JSF);
    join(&mut sim, CREED);
    until_live(&mut sim);
    sim.ticks(5);
    assert!(matches!(phase(&sim), Phase::RoundOver { .. }), "{:?}", phase(&sim));
}

#[test]
fn case_36_real_time_limit_is_none() {
    let m = mission("[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"TMM\"\n[Object0]\nTimeLimit = 180.0\n");
    assert_eq!(m.object().time_limit, 0);
    assert_eq!(missions::round_length("TMM", m.object().time_limit, 5), None);
}

#[test]
fn case_37_38_server_file() {
    assert!(ServerFile::read(&doc("[Server]\nName = \"x\"\n")).is_err());
    let s = ServerFile::read(&doc("[Server]\n[CombatMode]\nRoundMax = 5\nMaxPlayer = 24\n")).unwrap();
    assert_eq!((s.object_start, s.object_end, s.mission_end), (0.0, 0.0, 0.0));
    let c = ServerFile::read(&doc(
        "[Server]\n[CombatMode]\nObjectStart = 0\nObjectEnd = 3\nMissionEnd = 3\nObjectPlay = 600\nMissionTimeLimit = 30\n",
    ))
    .unwrap();
    assert_eq!(
        (c.object_start, c.object_end, c.mission_end, c.mission_time_limit),
        (0.0, 3.0, 3.0, 30)
    );
}

#[test]
fn case_39_mode_default_file() {
    assert_eq!(
        missions::default_file_of("GMS\\SnowValley_M_OFFICIAL.GMS").as_deref(),
        Some("GMS\\OFFICIAL.GMS")
    );
    let mut files = install();
    files.insert(
        missions::ROOM_FILE.into(),
        "[Map0]\nGMS_TMM_OFFICIAL = \"GMS\\SnowValley_M_OFFICIAL.GMS\"\n".into(),
    );
    files.insert(
        "gms/snowvalley_m_official.gms".into(),
        file("Worlds\\SnowValley.dat", "TMM", "TimeLimit = 180"),
    );
    let i = MissionIndex::build(&|p| files.get(p).map(|s| s.as_bytes().to_vec()));
    assert!(
        i.find("snowvalley", "TMM_OFFICIAL").is_some(),
        "a missing default file fails nothing"
    );
}

#[test]
fn case_40_duplicate_world_object_data() {
    let m = mission(
        "[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"CTF\"\n[WorldObjectData0]\nObjectName = \"FlagA\"\nPropName0 = \"Skin\"\nPropValue0 = \"first\"\n\
         [WorldObjectData1]\nObjectName = \"FlagA\"\nPropName0 = \"Skin\"\nPropValue0 = \"second\"\n",
    );
    assert_eq!(m.overrides_of("FlagA"), vec![("Skin".into(), "first".into())]);
}

/// Our data without the install: every mode's clock as the stock mission
/// files' (Stock values).
#[test]
fn stock_values_as_fallback() {
    for (key, round, warm) in [
        ("TSV", Some(180.0), Some(10.0)),
        ("TMM", Some(180.0), Some(10.0)),
        ("CTF", Some(900.0), None),
        ("QRT", Some(180.0), Some(10.0)),
        ("SBT", Some(300.0), Some(10.0)),
        ("HQM", Some(720.0), Some(10.0)),
        ("SQM2", Some(1800.0), Some(10.0)),
        ("HG", Some(270.0), Some(20.0)),
        ("BNT", Some(720.0), Some(10.0)),
    ] {
        let mut sim = game(key, 1);
        let w = sim.app.world_mut();
        assert_eq!(missions::real_round(w), round, "{key}");
        if missions::has_warmup(key) {
            assert_eq!(missions::warmup(w), warm, "{key} warm-up");
        }
        let _ = w.resource::<CaMatch>();
        let _ = w.resource::<CaRuleSettings>();
    }
}

/// A mission without a `[MapInfo]` or a mode key is a load failure.
#[test]
fn missing_sections_fail() {
    assert!(Mission::read("x", &doc("[Mission0]\nMissionMode = \"TDM\"\n")).is_err());
    assert!(Mission::read("x", &doc("[MapInfo]\nMap = \"w\"\n")).is_err());
}

/// Quarantine's sections (F4.1): the loadout order and a string id read
/// as -1 (dropped).
#[test]
fn quarantine_sections() {
    let m = mission(
        "[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"QRT\"\n[Object0]\nTimeLimit = 3\n\
         [InfectedModelSetting]\nModel = 10\nMeleeWeapon0 = 191\nPrimaryWeapon0 = \"5\"\n\
         [InfectedHostModelSetting]\nModel = 10\nMeleeWeapon0 = 190\nMinHostNum = 2\nSelectionRate = 0.2\n\
         [Infected_NoMovementHeal]\nTickTime = 1.0\nHeal = 0.1\nWaitingTIme = 3.0\n",
    );
    let q = m.quarantine.unwrap();
    assert_eq!(q.infected.unwrap().weapons, vec![191]);
    assert_eq!(q.host.unwrap().weapons, vec![190]);
    assert_eq!((q.min_hosts, q.selection_rate, q.heal), (2, 0.2, Some((1.0, 0.1, 3.0))));
    assert_eq!(
        missions::loadout(&[vec![1, 4], vec![2], vec![3], vec![]]),
        vec![1, 2, 3, 4]
    );
}

/// The map messages per step (F2, F3; game_modes.md S3, S15): the
/// mission's controllers and start triggers at mission start, the
/// objective's at each round's setup, its end triggers at the round's
/// end, the mission's at the mission's end; name lists split on `;`.
#[test]
fn map_messages_per_step() {
    use missions::MapStep::*;
    let m = mission(
        "[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"TMM\"\nController = \"GameControl_SD\"\n\
         StartTrigger = \"MissionStart\"\nEndTrigger = \"MissionEnd;\"\n[Object0]\nTimeLimit = 180\n\
         Controller = \"GameControl_SD\"\nStartTrigger = \"Level02StartTrigger;Extra\"\n\
         EndTrigger = \"Level02EndTrigger\"\n",
    );
    let o = m.object();
    let pairs = |step| {
        m.map_messages(step, &o)
            .into_iter()
            .map(|(n, msg)| format!("{n} {msg}"))
            .collect::<Vec<_>>()
    };
    assert_eq!(pairs(MissionStart), ["GameControl_SD ON", "MissionStart TRIGGER"]);
    assert_eq!(
        pairs(RoundSetup),
        ["GameControl_SD ON", "Level02StartTrigger TRIGGER", "Extra TRIGGER"]
    );
    assert_eq!(pairs(RoundOver), ["Level02EndTrigger TRIGGER"]);
    assert_eq!(pairs(MissionEnd), ["MissionEnd TRIGGER"]);
    // No names: nothing.
    let bare = mission(&file("w", "TDM", "TimeLimit = 0"));
    assert!(bare.map_messages(RoundSetup, &bare.object()).is_empty());
}

/// The rules send the mission's map messages as the match runs (S3, S15):
/// controllers and start triggers at each mission start and setup, end
/// triggers at round over, the mission's at round end; again for the next
/// round (every round ends its mission).
#[test]
fn map_messages_follow_the_match() {
    use combat_arms::game::objects::CaObjectMessage;
    let file = mission(
        "[MapInfo]\nMap = \"w\"\n[Mission0]\nMissionMode = \"TMM\"\nController = \"Control\"\n\
         EndTrigger = \"MissionEndTrigger\"\n[Object0]\nTimeLimit = 2\nTrainingTimeLimit = 1\n\
         StartTrigger = \"Level01StartTrigger\"\nEndTrigger = \"Level01EndTrigger\"\n",
    );
    let mut sim = game("TMM", 2);
    sim.app.world_mut().insert_resource(CaMission(Some(Arc::new(file))));
    let mut cursor = sim.app.world().resource::<Messages<CaObjectMessage>>().get_cursor();
    let mut seen: Vec<(String, Phase)> = Vec::new();
    let mut step = |sim: &mut Sim, seen: &mut Vec<(String, Phase)>| {
        sim.ticks(1);
        let w = sim.app.world();
        let p = w.resource::<CaMatch>().phase;
        for msg in cursor.read(w.resource::<Messages<CaObjectMessage>>()) {
            seen.push((format!("{} {}", msg.target, msg.message), p));
        }
    };
    join(&mut sim, JSF);
    join(&mut sim, CREED);
    // The first round and the next one's setup.
    while seen.len() < 6 && now(&sim) < 20.0 {
        step(&mut sim, &mut seen);
    }
    let names: Vec<&str> = seen.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "Control ON",
            "Level01StartTrigger TRIGGER",
            "Level01EndTrigger TRIGGER",
            "MissionEndTrigger TRIGGER",
            "Control ON",
            "Level01StartTrigger TRIGGER"
        ],
        "{seen:?}"
    );
    assert!(matches!(seen[1].1, Phase::Setup { .. }), "at the setup: {:?}", seen[1]);
    assert!(
        matches!(seen[2].1, Phase::RoundOver { .. }),
        "at round over: {:?}",
        seen[2]
    );
    assert!(
        matches!(seen[3].1, Phase::Intermission { .. }),
        "at round end: {:?}",
        seen[3]
    );
    assert!(
        matches!(seen[5].1, Phase::Setup { .. }),
        "the next setup: {:?}",
        seen[5]
    );
}
