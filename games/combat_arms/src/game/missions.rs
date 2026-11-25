//! Combat Arms' per-map mission data (specs/combat_arms/missions.md; F1-F4
//! below are its field sections): the one small file a (map, mode) pair
//! has in the install's `GMS.rez` (`GMS\<MAP>_<SUFFIX>.GMS`, V1-keyed,
//! attribute syntax: `lithtech_formats::attributes`), found through the
//! room data (`ATTRIBUTES\GAMEROOM.TXT`, `[Map<n>]` `GMS_<key>`), with its
//! mode default file (`GMS\VWRM.GMS`'s Arms Race ladders) and the VIP table
//! (`GMS\VTDM.GMS`), and the server file (`ATTRIBUTES\SERVER.CFG`, the
//! single-player and bot host's copy: the phase lengths).
//!
//! - `Mission::read`: `[MapInfo]` (`Map`, the world), `[Mission0]`
//!   (`MissionMode`, controllers and triggers, `VIPSpawn`), every
//!   `[Object<n>]` (`TimeLimit`, `TrainingTimeLimit`, offence and defence,
//!   the objective weapons as name lists, spawn areas, next objectives,
//!   activate-trigger times, the minimap calibration, co-op lives and AI
//!   factors, Hunted's numbers) and the mode sections: Quarantine's
//!   infected and host settings, heal and points (F4.1), Spy Hunt's intel,
//!   spy and agent settings and stage-2 time (F4.2), Bombing Run's drop
//!   object (F4.3), Seize and Secure's (F4.4), Hired Guns' bullion
//!   (F4.5), VIP Escort's VIP (F4.6), the Arms Race ladders (F4.8), the
//!   forced loadout (F4.9) and the world object overrides (F4.10). Typed
//!   reads are strict: a value of the wrong type is the field's default
//!   (`TimeLimit = 180.0` is no time limit).
//! - `MissionIndex`: every `[Map<n>]` record's `GMS_<key>` paths, each
//!   file's world, so a map (its world file's name) and a room key find
//!   their mission; the official variant by `<key>_OFFICIAL`.
//! - The modes' clocks (Timers per mode): `round_length`,
//!   `warmup_length`, `setup_forced`.
//! - `CaMission`: the playing (map, mode)'s data the rules read, resolved
//!   when a match starts (`resolve`; at the first mission start too when
//!   it was found for another map or mode, `ensure_resolved`: a map the
//!   app started on with `--map` had no fresh match): the install's mission where it has
//!   one, else our data (data/modes.toml's `TimeLimit` and
//!   `TrainingTimeLimit` per mode, the objectives' numbers) as the
//!   fallback, field by field.
//! - The map messages (`map_messages`, `MapStep`): at mission start
//!   `[Mission0]`'s controllers `ON` and start triggers `TRIGGER`, at each
//!   round's setup the objective's, at the round's end its end triggers,
//!   at the mission's end `[Mission0]`'s (the stock maps'
//!   `LevelNNStartTrigger` / `LevelNNEndTrigger`); `rules::flow` sends
//!   them to the map's objects.
//!
//! Known shortcuts: the file decides the mode in the game (Quirk 1: the
//! US room data's Hired Guns on Hired Guns 2 runs Squad Match 2); here
//! `ca_mode` keeps deciding and a mission whose `MissionMode` differs is
//! logged (`CaMission::mode` says what the file names); a (map, mode)
//! the room data doesn't list plays our data (the game would refuse to
//! start); the map cycle of the server file isn't read (never enabled in
//! stock files); the co-operative sections (F4.12), Hunted's map items
//! (F4.11), rewards (F4.13) and the dedicated server's own `server.cfg`
//! aren't read (nothing plays them); minimap calibration is read but no
//! minimap exists yet.

use std::{
    collections::HashMap,
    sync::Arc,
};

use bevy::prelude::*;
use lithtech_formats::attributes::{Document, Section, Value};

use super::rules::CaRuleSettings;

/// The mission archive's folder (normalized paths: lower case, `/`).
pub const GMS_DIR: &str = "gms/";
/// The room data and the single-player host's server file in the
/// attributes archive.
pub const ROOM_FILE: &str = "attributes/gameroom.txt";
pub const SERVER_CFG: &str = "attributes/server.cfg";
/// The VIP Elimination table's fixed file (F4.7).
pub const VTDM_FILE: &str = "gms/vtdm.gms";

/// Team ids (game_modes.md S1).
pub const JSF: u8 = 1;
pub const CREED: u8 = 2;

/// A name list (Format): one string of names separated by `;`, nothing
/// trimmed; an empty last piece is skipped, an empty string is none.
pub fn name_list(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut parts: Vec<String> = text.split(';').map(str::to_string).collect();
    if parts.last().is_some_and(String::is_empty) {
        parts.pop();
    }
    parts
}

fn text(s: &Section, key: &str) -> Option<String> {
    s.string(key).map(|b| b.iter().map(|&c| c as char).collect())
}

fn names(s: &Section, key: &str) -> Vec<String> {
    text(s, key).map(|t| name_list(&t)).unwrap_or_default()
}

/// A team short name (`JSF`, `Creed`, `Observer`; case ignored), else
/// `fallback`.
fn team(s: &Section, key: &str, fallback: u8) -> u8 {
    match text(s, key).map(|t| t.to_ascii_lowercase()).as_deref() {
        Some("jsf") => JSF,
        Some("creed") => CREED,
        Some("observer") => 0,
        _ => fallback,
    }
}

/// An integer list stored as integers: a value of another type is -1
/// (F4.1's note), entries -1 are skipped.
fn int_ids(s: &Section, key: &str) -> Vec<i32> {
    (0..)
        .map_while(|i| s.get(&format!("{key}{i}")))
        .map(|v| if let Value::Int(n) = v { *n } else { -1 })
        .filter(|n| *n != -1)
        .collect()
}

/// A list of strings each converted to an integer (Spy Hunt's agent
/// weapons, F4.2).
fn string_ids(s: &Section, key: &str) -> Vec<i32> {
    s.string_list(key)
        .iter()
        .map(|b| lithtech_formats::attributes::c_atoi(b))
        .filter(|n| *n != -1)
        .collect()
}

/// The minimap calibration (F3): two reference points, pixel and world.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Minimap {
    pub image: String,
    pub width: i32,
    pub height: i32,
    pub px: [[i32; 2]; 2],
    pub world: [[f64; 3]; 2],
}

impl Minimap {
    /// The world rectangle the whole image covers: (LeftTop, RightBottom),
    /// X and Z (Y 0); None when the two points share an x or a y.
    pub fn rect(&self) -> Option<([f64; 3], [f64; 3])> {
        let [[x0, y0], [x1, y1]] = self.px;
        let [p0, p1] = self.world;
        if x0 == x1 || y0 == y1 {
            return None;
        }
        let sx = (p1[0] - p0[0]) / (x1 - x0) as f64;
        let sz = (p0[2] - p1[2]) / (y1 - y0) as f64;
        let lt = [p0[0] - x0 as f64 * sx, 0.0, p0[2] + y0 as f64 * sz];
        let rb = [p1[0] + (self.width - x1) as f64 * sx, 0.0, p1[2] - (self.height - y1) as f64 * sz];
        Some((lt, rb))
    }
}

/// `[Mission0]` (F2).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MissionRecord {
    /// The mode key (`MissionMode`, matched exactly); empty: none.
    pub mode: String,
    pub controller: Vec<String>,
    pub start_trigger: Vec<String>,
    pub end_trigger: Vec<String>,
    pub vip_spawn: String,
    pub ai_default_team: i32,
}

/// An `[Object<n>]` (F3).
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectRecord {
    /// `TimeLimit` as stored (s; Quarantine and Spy Hunt: minutes); 0: no
    /// limit (also when stored as a real).
    pub time_limit: i32,
    pub training_time_limit: i32,
    /// `OffenceTeam` / `DefenceTeam` (built in: Creed attacks, JSF
    /// defends, Quirk 6).
    pub offence: u8,
    pub defence: u8,
    /// The objective weapons by their weapon `Name`s.
    pub jsf_weapon: Vec<String>,
    pub creed_weapon: Vec<String>,
    pub jsf_spawn: i32,
    pub creed_spawn: i32,
    pub jsf_win: i32,
    pub creed_win: i32,
    pub start_trigger: Vec<String>,
    pub end_trigger: Vec<String>,
    pub controller: Vec<String>,
    pub round_trigger: Vec<String>,
    pub escape_zone_trigger: String,
    pub timer_trigger: String,
    /// `ActivateTriggerName<i>` with `ActivateTriggerTimeAmount<i>` (s).
    pub activate: Vec<(String, f64)>,
    pub minimap: Option<Minimap>,
    /// Co-op: lives on entering, the cap, the AI damage factor.
    pub life_count: i32,
    pub max_life_count: i32,
    pub ai_damage_factor: f64,
    /// Hunted: `SystemScanPlayTime`, `SystemScanDelayTime` (s),
    /// `MaxMapItemSpawnCnt`, `DefaultActItemSpawnCnt`,
    /// `AddActItemSpawnCntPerPlayer`.
    pub scan_play_time: f64,
    pub scan_delay_time: f64,
    pub max_map_items: i32,
    pub default_items: i32,
    pub items_per_player: i32,
}

impl Default for ObjectRecord {
    fn default() -> Self {
        Self {
            time_limit: 0,
            training_time_limit: 0,
            offence: CREED,
            defence: JSF,
            jsf_weapon: Vec::new(),
            creed_weapon: Vec::new(),
            jsf_spawn: 0,
            creed_spawn: 0,
            jsf_win: -1,
            creed_win: -1,
            start_trigger: Vec::new(),
            end_trigger: Vec::new(),
            controller: Vec::new(),
            round_trigger: Vec::new(),
            escape_zone_trigger: String::new(),
            timer_trigger: String::new(),
            activate: Vec::new(),
            minimap: None,
            life_count: 0,
            max_life_count: 0,
            ai_damage_factor: 0.5,
            scan_play_time: 0.0,
            scan_delay_time: 0.0,
            max_map_items: 0,
            default_items: 0,
            items_per_player: 0,
        }
    }
}

impl ObjectRecord {
    pub fn read(s: &Section) -> Self {
        let d = Self::default();
        let activate = s
            .string_list("ActivateTriggerName")
            .into_iter()
            .zip(s.number_list("ActivateTriggerTimeAmount"))
            .map(|(n, t)| (n.iter().map(|&c| c as char).collect(), t))
            .collect();
        let minimap = text(s, "LargeMap").filter(|m| !m.is_empty()).map(|image| Minimap {
            image,
            width: s.int("LargeMapWidth", 0),
            height: s.int("LargeMapHeight", 0),
            px: [s.point("LargeMapPoint0_2D").unwrap_or_default(), s.point("LargeMapPoint1_2D").unwrap_or_default()],
            world: [s.vector("LargeMapPoint0_3D").unwrap_or_default(), s.vector("LargeMapPoint1_3D").unwrap_or_default()],
        });
        Self {
            time_limit: s.int("TimeLimit", 0),
            training_time_limit: s.int("TrainingTimeLimit", 0),
            offence: team(s, "OffenceTeam", d.offence),
            defence: team(s, "DefenceTeam", d.defence),
            jsf_weapon: names(s, "JSFWeapon"),
            creed_weapon: names(s, "CreedWeapon"),
            jsf_spawn: s.int("JSFSpawn", 0),
            creed_spawn: s.int("CreedSpawn", 0),
            jsf_win: s.int("JSFWin", -1),
            creed_win: s.int("CreedWin", -1),
            start_trigger: names(s, "StartTrigger"),
            end_trigger: names(s, "EndTrigger"),
            controller: names(s, "Controller"),
            round_trigger: names(s, "RoundTrigger"),
            escape_zone_trigger: text(s, "EscapeZoneTrigger").unwrap_or_default(),
            timer_trigger: text(s, "TimerTrigger").unwrap_or_default(),
            activate,
            minimap,
            life_count: s.int("LifeCount", 0),
            max_life_count: s.int("MaxLifeCount", 0),
            ai_damage_factor: s.number("AIDamageFactor", d.ai_damage_factor),
            scan_play_time: s.number("SystemScanPlayTime", 0.0),
            scan_delay_time: s.number("SystemScanDelayTime", 0.0),
            max_map_items: s.int("MaxMapItemSpawnCnt", 0),
            default_items: s.int("DefaultActItemSpawnCnt", 0),
            items_per_player: s.int("AddActItemSpawnCntPerPlayer", 0),
        }
    }

    /// The next objective after `winner`'s win (`JSFWin`, `CreedWin`;
    /// a draw: -1).
    pub fn next_after(&self, winner: Option<u8>) -> i32 {
        match winner {
            Some(JSF) => self.jsf_win,
            Some(CREED) => self.creed_win,
            _ => -1,
        }
    }
}

/// A model and loadout setting (F4.1, F4.2, F4.6).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CharSetting {
    pub model: i32,
    pub goggle: i32,
    pub backpack: i32,
    /// The loadout: slot 0 of primary, secondary, melee and throw, then
    /// the further entries in the same class order (-1 skipped).
    pub weapons: Vec<i32>,
}

impl CharSetting {
    fn read(s: &Section, as_strings: bool) -> Self {
        let lists: Vec<Vec<i32>> = ["PrimaryWeapon", "SecondaryWeapon", "MeleeWeapon", "ThrowWeapon"]
            .iter()
            .map(|k| if as_strings { string_ids(s, k) } else { int_ids(s, k) })
            .collect();
        Self {
            model: s.int("Model", 0),
            goggle: s.int("Goggle", -1),
            backpack: s.int("BackPack", -1),
            weapons: loadout(&lists),
        }
    }
}

/// F4.1's loadout order: each list's first entry in class order, then
/// every further entry the same way.
pub fn loadout(lists: &[Vec<i32>]) -> Vec<i32> {
    let mut out: Vec<i32> = lists.iter().filter_map(|l| l.first().copied()).collect();
    for l in lists {
        out.extend(l.iter().skip(1).copied());
    }
    out
}

/// Quarantine (F4.1).
#[derive(Clone, Debug, PartialEq)]
pub struct QuarantineData {
    pub infected: Option<CharSetting>,
    pub host: Option<CharSetting>,
    pub min_hosts: i32,
    pub selection_rate: f64,
    /// `[Infected_NoMovementHeal]`: `TickTime` (s), `Heal` (a fraction of
    /// the maximum hit points), `WaitingTIme` (s); None without the
    /// section.
    pub heal: Option<(f64, f64, f64)>,
    pub to_infected_fx: String,
    pub to_host_fx: String,
    /// `QRT_BasicPoint`, `KillPoint`, `DeadPoint`, `InfectPoint`,
    /// `InfectedPoint`, `HumanWinPoint`, `InfectWinPoint`.
    pub points: Option<[i32; 7]>,
    pub destroy: Vec<String>,
}

/// Spy Hunt (F4.2).
#[derive(Clone, Debug, PartialEq)]
pub struct SpyHuntData {
    pub intel_total: i32,
    pub spy: Option<CharSetting>,
    pub agent: Option<CharSetting>,
    pub agent_trigger: String,
    pub agent_camera_hold: i32,
    /// `Stage2PlayTime`, s (180 without it).
    pub stage2_time: i32,
}

/// Seize and Secure (F4.4).
#[derive(Clone, Debug, PartialEq)]
pub struct SeizeData {
    pub flag_move_time: f64,
    pub last_occupy_bonus: i32,
    pub abuse_play_plus: f64,
    pub abuse_flag_plus: f64,
}

impl Default for SeizeData {
    fn default() -> Self {
        Self {
            flag_move_time: 120.0,
            last_occupy_bonus: 50,
            abuse_play_plus: 30.0,
            abuse_flag_plus: 5.0,
        }
    }
}

/// Hired Guns' bullion (F4.5).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BullionData {
    pub drop_num: i32,
    pub amount: i32,
    pub base_bank: i32,
    pub round_win_bonus: i32,
    pub kill_bonus: i32,
    pub death_bonus: i32,
    pub spawn_cost_ai: Vec<i32>,
    /// The carry penalty's sections (inclusive bar ranges) and rates.
    pub penalty: Vec<([f64; 2], f64)>,
}

impl BullionData {
    /// The carry penalty for `n` bars (F4.5's formula: the last section
    /// containing n, built on the previous section's end).
    pub fn carry_penalty(&self, n: f64) -> f64 {
        carry_penalty(&self.penalty, n)
    }
}

/// F4.5's P(n).
pub fn carry_penalty(sections: &[([f64; 2], f64)], n: f64) -> f64 {
    fn at(sections: &[([f64; 2], f64)], i: usize, n: f64) -> f64 {
        let ([_, _], r) = sections[i];
        if i == 0 {
            r * n
        } else {
            let prev = sections[i - 1].0[1];
            at(sections, i - 1, prev) + r * (n - prev)
        }
    }
    match sections.iter().rposition(|([a, b], _)| n >= *a && n <= *b) {
        Some(i) => at(sections, i, n),
        None => 0.0,
    }
}

/// VIP Escort's VIP (F4.6).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VipEscortData {
    pub model: i32,
    pub camo: i32,
    pub goggle: i32,
    pub backpack: i32,
    pub hud_index: i32,
    pub weapons: Vec<i32>,
}

/// One Arms Race ladder (`Weapon_Step_Table<t>`, F4.8).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepTable {
    pub goal: i32,
    /// -1: the default ladder; else the function item that switches to it.
    pub item_no: i32,
    pub percent: i32,
    pub steps: Vec<Step>,
}

/// A ladder step: the kills it starts at, the weapon class shown, its
/// weighted candidates (weapon id, weight > 0).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Step {
    pub kill: i32,
    pub weapon_type: i32,
    pub candidates: Vec<(i32, i32)>,
}

impl StepTable {
    fn read(s: &Section) -> Self {
        let mut steps = Vec::new();
        for i in 0.. {
            let (kill, kind) = (s.int(&format!("Kill{i}"), -1), s.int(&format!("WeaponType{i}"), -1));
            if kill < 0 || kind < 0 {
                break;
            }
            let mut candidates = Vec::new();
            for j in 0.. {
                let (id, w) = (s.int(&format!("Set{i}_{j}"), -1), s.int(&format!("Set_rate{i}_{j}"), -1));
                if id == -1 || w == -1 {
                    break;
                }
                if w != 0 {
                    candidates.push((id, w));
                }
            }
            steps.push(Step {
                kill,
                weapon_type: kind,
                candidates,
            });
        }
        Self {
            goal: s.int("Goal", 0),
            item_no: s.int("ItemNo", -1),
            percent: s.int("Percent", 0),
            steps,
        }
    }

    /// The step that starts at exactly `kills`.
    pub fn step_at(&self, kills: i32) -> Option<&Step> {
        self.steps.iter().find(|s| s.kill == kills)
    }

    /// The step in force at `kills` (the last whose `Kill` ≤ kills).
    pub fn step_for(&self, kills: i32) -> Option<&Step> {
        self.steps.iter().rev().find(|s| s.kill <= kills)
    }

    /// The next step's kill count (None: the last stage).
    pub fn next_kill(&self, kills: i32) -> Option<i32> {
        self.steps.iter().find(|s| s.kill > kills).map(|s| s.kill)
    }
}

impl Step {
    /// The weighted pick for a draw q uniform in 0..=W: the first
    /// candidate whose running sum reaches q.
    pub fn pick(&self, q: i32) -> Option<i32> {
        let mut sum = 0;
        for (id, w) in &self.candidates {
            sum += w;
            if sum >= q {
                return Some(*id);
            }
        }
        self.candidates.last().map(|c| c.0)
    }

    /// W, the sum of the weights.
    pub fn total(&self) -> i32 {
        self.candidates.iter().map(|c| c.1).sum()
    }
}

/// The ladders G uses (F4.8, Loading): `tables` with `Goal` = G, else the
/// mode default file's; the first table kept per `ItemNo`.
pub fn ladders_for(own: &[StepTable], defaults: &[StepTable], goal: i32) -> Vec<StepTable> {
    let pick = |ts: &[StepTable]| -> Vec<StepTable> {
        let mut out: Vec<StepTable> = Vec::new();
        for t in ts.iter().filter(|t| t.goal == goal) {
            if !out.iter().any(|o| o.item_no == t.item_no) {
                out.push(t.clone());
            }
        }
        out
    };
    let mine = pick(own);
    if mine.is_empty() { pick(defaults) } else { mine }
}

/// The weapon a player gets at an Arms Race step (F4.8, Choosing a
/// weapon): roll r in 0..=100; below the default ladder's `Percent` its
/// pick (by q in 0..=W), and below a function ladder's when the player
/// owns its item; else None (the player's own weapon of that class).
pub fn ladder_pick(ladders: &[StepTable], owned: &[i32], kills: i32, r: i32, q: i32, exact: bool) -> Option<i32> {
    let default = ladders.iter().find(|t| t.item_no == -1)?;
    if r >= default.percent {
        return None;
    }
    let step_of = |t: &StepTable| if exact { t.step_at(kills).cloned() } else { t.step_for(kills).cloned() };
    let special = ladders.iter().find(|t| t.item_no != -1 && owned.contains(&t.item_no) && r < t.percent);
    let step = special.and_then(step_of).or_else(|| step_of(default))?;
    step.pick(q)
}

/// VIP Elimination's table (F4.7).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VipData {
    /// (`TargetPoint`, VIPs for a team of 1..8 players), in file order.
    pub tables: Vec<(i32, [i32; 8])>,
    /// Points a death is worth by the victim's rank (0 none .. 3 gold).
    pub weights: Vec<i32>,
}

impl VipData {
    pub fn read(doc: &Document) -> Self {
        let mut tables: Vec<(i32, [i32; 8])> = doc
            .records("VIPElimination_Table")
            .into_iter()
            .map(|s| {
                let mut n = [0; 8];
                for (i, v) in n.iter_mut().enumerate() {
                    *v = s.int(&format!("VIPNumber{}", i + 1), 0);
                }
                (s.int("TargetPoint", 0), n)
            })
            .filter(|(t, _)| *t != 0)
            .collect();
        tables.sort_by_key(|t| t.0);
        let weights = doc
            .section("Point_Weight")
            .map(|s| {
                (0..)
                    .map_while(|i| match s.get(&format!("VIPPointWeight{i}")) {
                        Some(Value::Int(n)) => Some(*n),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self { tables, weights }
    }

    /// The stock numbers (missions.md F4.7, for a play without the
    /// install).
    pub fn stock() -> Self {
        Self {
            tables: vec![(60, [0, 0, 0, 0, 0, 1, 1, 2]), (1000, [0, 1, 1, 1, 2, 2, 2, 3])],
            weights: vec![1, 3, 5, 7],
        }
    }

    /// VIPs for a team of `players` at goal G: the first table (by
    /// `TargetPoint`) with `TargetPoint` ≥ G; none: 0; capped at the team.
    pub fn vips(&self, goal: i32, players: usize) -> usize {
        let Some((_, n)) = self.tables.iter().find(|(t, _)| *t >= goal) else { return 0 };
        if players == 0 {
            return 0;
        }
        (n[players.min(8) - 1].max(0) as usize).min(players)
    }

    /// The points a victim of `rank` is worth (a missing weight: 1).
    pub fn weight(&self, rank: usize) -> i32 {
        self.weights.get(rank).copied().unwrap_or(1)
    }
}

/// A world object override (F4.10): properties replaced before the
/// object named `object` reads them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldOverride {
    pub object: String,
    pub props: Vec<(String, String)>,
}

/// A mission file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mission {
    /// Its path in the archive (normalized), or empty for ours.
    pub path: String,
    /// `[MapInfo]` `Map`: the world file to load.
    pub map: String,
    pub mission: MissionRecord,
    pub objects: Vec<ObjectRecord>,
    pub quarantine: Option<QuarantineData>,
    pub spy_hunt: Option<SpyHuntData>,
    /// `[DropObject]` (or `[IntelObject]`, sharing it) `TotalNum`.
    pub drop_total: Option<i32>,
    pub seize: Option<SeizeData>,
    pub bullion: Option<BullionData>,
    pub vip_escort: Option<VipEscortData>,
    pub ladders: Vec<StepTable>,
    /// The mode default file's ladders (`GMS\VWRM.GMS`), used when this
    /// file has none for G.
    pub default_ladders: Vec<StepTable>,
    /// `[WeaponList]` `Weapon<i>` (F4.9): the forced loadout.
    pub weapon_list: Vec<i32>,
    pub overrides: Vec<WorldOverride>,
}

impl Mission {
    /// A mission file from its document; `Err` when it has no `[MapInfo]`
    /// or no `[Mission0]` mode key (Edge cases).
    pub fn read(path: &str, doc: &Document) -> Result<Self, String> {
        let info = doc.section("MapInfo").ok_or_else(|| format!("{path}: no [MapInfo]"))?;
        let m = doc.section("Mission0").ok_or_else(|| format!("{path}: no [Mission0]"))?;
        let mission = MissionRecord {
            mode: text(m, "MissionMode").unwrap_or_default(),
            controller: names(m, "Controller"),
            start_trigger: names(m, "StartTrigger"),
            end_trigger: names(m, "EndTrigger"),
            vip_spawn: text(m, "VIPSpawn").unwrap_or_default(),
            ai_default_team: m.int("AIDefaultTeam", 0),
        };
        if mission.mode.is_empty() {
            return Err(format!("{path}: no MissionMode"));
        }
        let objects = doc.records("Object").into_iter().map(ObjectRecord::read).collect();
        let quarantine = (doc.section("InfectedModelSetting").is_some()
            || doc.section("InfectedHostModelSetting").is_some()
            || doc.section("Infected_NoMovementHeal").is_some())
        .then(|| {
            let host = doc.section("InfectedHostModelSetting");
            QuarantineData {
                infected: doc.section("InfectedModelSetting").map(|s| CharSetting::read(s, false)),
                host: host.map(|s| CharSetting::read(s, false)),
                min_hosts: host.map_or(2, |s| s.int("MinHostNum", 2)),
                selection_rate: host.map_or(0.2, |s| s.number("SelectionRate", 0.2)),
                heal: doc.section("Infected_NoMovementHeal").map(|s| {
                    (s.number("TickTime", 0.0), s.number("Heal", 0.0), s.number("WaitingTIme", 0.0))
                }),
                to_infected_fx: doc.section("ChangingFXSetting").and_then(|s| text(s, "PlayerToInfected")).unwrap_or_default(),
                to_host_fx: doc.section("ChangingFXSetting").and_then(|s| text(s, "PlayerToHost")).unwrap_or_default(),
                points: doc.section("ExpAndGPSetting").filter(|s| s.get("QRT_BasicPoint").is_some()).map(|s| {
                    [
                        "QRT_BasicPoint",
                        "QRT_KillPoint",
                        "QRT_DeadPoint",
                        "QRT_InfectPoint",
                        "QRT_InfectedPoint",
                        "QRT_HumanWinPoint",
                        "QRT_InfectWinPoint",
                    ]
                    .map(|k| s.int(k, 0))
                }),
                destroy: doc
                    .section("DestroyProjectiles")
                    .map(|s| s.string_list("Projectile").iter().map(|b| b.iter().map(|&c| c as char).collect()).collect())
                    .unwrap_or_default(),
            }
        });
        let agent = doc.section("AgentModelSetting");
        let spy_hunt = (doc.section("SpyModelSetting").is_some() || agent.is_some() || doc.section("BNT_Stage2_Comment").is_some())
            .then(|| SpyHuntData {
                intel_total: doc.section("IntelObject").map_or(0, |s| s.int("TotalNum", 0)),
                spy: doc.section("SpyModelSetting").map(|s| CharSetting::read(s, false)),
                agent: agent.map(|s| CharSetting::read(s, true)),
                agent_trigger: agent.and_then(|s| text(s, "AgentTrigger")).unwrap_or_default(),
                agent_camera_hold: agent.map_or(0, |s| s.int("AgentCameraHoldingTime", 0)),
                stage2_time: doc.section("BNT_Stage2_Comment").map_or(180, |s| s.int("Stage2PlayTime", 180)),
            });
        // The drop object shares its storage with the intel object: the
        // one read last wins.
        let drop_total = doc
            .section("DropObject")
            .or(doc.section("IntelObject"))
            .map(|s| s.int("TotalNum", 0));
        let seize = doc.section("HeadQuatersMode").map(|s| {
            let d = SeizeData::default();
            SeizeData {
                flag_move_time: s.number("FlagMoveTime", d.flag_move_time),
                last_occupy_bonus: s.int("LastOccupyBonus", d.last_occupy_bonus),
                abuse_play_plus: s.number("Abuse_PlayTime_Plus", d.abuse_play_plus),
                abuse_flag_plus: s.number("Abuse_OccupyFlagTime_Plus", d.abuse_flag_plus),
            }
        });
        let bullion = doc.section("BullionManage").map(|s| BullionData {
            drop_num: s.int("DropBullionNum", 0),
            amount: s.int("BullionAmount", 0),
            base_bank: s.int("BaseBankAmount", 0),
            round_win_bonus: s.int("RoundWinBonusAmount", 0),
            kill_bonus: s.int("KillBonusAmount", 0),
            death_bonus: s.int("DeathBonusAmount", 0),
            spawn_cost_ai: s.int_list("SpawnCostAI"),
            penalty: (0..)
                .map_while(|i| {
                    let range = s.range(&format!("MovePenaltyIncPerBullionNumSection{i}"))?;
                    let rate = s.number_opt(&format!("MovePenaltyIncPerBullionNum{i}"))?;
                    Some((range, rate))
                })
                .collect(),
        });
        let vip_escort = doc.section("VIPChangeCharInfo").map(|info| {
            let model = doc.section(&format!("VIPChangeCharModel_{}", info.int("ModelIndex", 0)));
            let weapon = doc.section(&format!("VIPChangeCharWeapon_{}", info.int("WeaponIndex", 0)));
            VipEscortData {
                model: model.map_or(0, |s| s.int("ModelID", 0)),
                camo: model.map_or(0, |s| s.int("CamoIndex", 0)),
                goggle: model.map_or(-1, |s| s.int("GoggleID", -1)),
                backpack: model.map_or(-1, |s| s.int("BackpackID", -1)),
                hud_index: model.map_or(-1, |s| s.int("HUDIndex", -1)),
                weapons: weapon
                    .map(|s| loadout(&["Primary", "Secondary", "Melee", "Throw"].map(|k| int_ids(s, k))))
                    .unwrap_or_default(),
            }
        });
        let ladders = doc.records("Weapon_Step_Table").into_iter().map(StepTable::read).collect();
        let weapon_list = doc.section("WeaponList").map(|s| int_ids(s, "Weapon")).unwrap_or_default();
        let overrides = world_overrides(doc);
        Ok(Self {
            path: path.to_string(),
            map: text(info, "Map").unwrap_or_default(),
            mission,
            objects,
            quarantine,
            spy_hunt,
            drop_total,
            seize,
            bullion,
            vip_escort,
            ladders,
            default_ladders: Vec::new(),
            weapon_list,
            overrides,
        })
    }

    /// `[Object0]` (every versus round plays it: `StartObject` is
    /// ignored, Quirk 5); the built-in record without one.
    pub fn object(&self) -> ObjectRecord {
        self.objects.first().cloned().unwrap_or_default()
    }

    /// The co-op objective for the difficulty G (clamp(G, 0, 2)).
    pub fn coop_object(&self, goal: i32) -> ObjectRecord {
        self.objects.get(goal.clamp(0, 2) as usize).cloned().unwrap_or_else(|| self.object())
    }

    /// The world file's name, lower case without folder or extension
    /// (`Worlds2\Warehouse.dat` is `warehouse`).
    pub fn world_name(&self) -> String {
        world_stem(&self.map)
    }

    /// The overrides of world object `name` (exact, case-sensitive).
    pub fn overrides_of(&self, name: &str) -> Vec<(String, String)> {
        self.overrides.iter().filter(|o| o.object == name).flat_map(|o| o.props.clone()).collect()
    }
}

/// `[WorldObjectData<n>]` (F4.10): a record whose `ObjectName` was seen
/// already is skipped with a warning (the game's reader would loop
/// forever, Edge cases).
fn world_overrides(doc: &Document) -> Vec<WorldOverride> {
    let mut out: Vec<WorldOverride> = Vec::new();
    for s in doc.records("WorldObjectData") {
        let Some(object) = text(s, "ObjectName") else { continue };
        if out.iter().any(|o| o.object == object) {
            warn!("mission data: a second [WorldObjectData] for {object}: kept the first");
            continue;
        }
        let names = s.string_list("PropName");
        let values = s.string_list("PropValue");
        let props = names
            .iter()
            .zip(values.iter())
            .map(|(n, v)| (n.iter().map(|&c| c as char).collect(), v.iter().map(|&c| c as char).collect()))
            .collect();
        out.push(WorldOverride { object, props });
    }
    out
}

/// A world path's stem, lower case (`Worlds2\Warehouse.dat` → `warehouse`).
pub fn world_stem(path: &str) -> String {
    let file = path.rsplit(['\\', '/']).next().unwrap_or(path);
    file.rsplit_once('.').map_or(file, |(s, _)| s).to_ascii_lowercase()
}

/// A mission path as the archive lists it (lower case, `/`).
pub fn normalized(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

/// The mode default file a mission path brings along (Format): with a
/// `\` and a `_`, the text after the last `_`, in the same folder
/// (`GMS\GHOSTTOWN_VWRM.GMS` → `GMS\VWRM.GMS`).
pub fn default_file_of(path: &str) -> Option<String> {
    let p = path.replace('/', "\\");
    let (dir, file) = p.rsplit_once('\\')?;
    let (_, tail) = file.rsplit_once('_')?;
    Some(format!("{dir}\\{tail}"))
}

/// The server file's `[CombatMode]` (The server file).
#[derive(Clone, Debug, PartialEq)]
pub struct ServerFile {
    pub object_start: f64,
    pub object_end: f64,
    pub mission_end: f64,
    pub object_play: f64,
    pub mission_time_limit: i32,
    pub round_max: i32,
    pub max_player: i32,
    /// `[MapCycle]` when `Enable` is true: the mission paths.
    pub map_cycle: Option<Vec<String>>,
}

impl ServerFile {
    /// `Err` without `[Server]` or `[CombatMode]` (the match doesn't
    /// start).
    pub fn read(doc: &Document) -> Result<Self, String> {
        doc.section("Server").ok_or("server file: no [Server]")?;
        let c = doc.section("CombatMode").ok_or("server file: no [CombatMode]")?;
        let map_cycle = doc
            .section("MapCycle")
            .filter(|s| s.bool("Enable", false))
            .map(|s| s.string_list("Map").iter().map(|b| b.iter().map(|&c| c as char).collect()).collect());
        Ok(Self {
            object_start: c.number("ObjectStart", 0.0),
            object_end: c.number("ObjectEnd", 0.0),
            mission_end: c.number("MissionEnd", 0.0),
            object_play: c.number("ObjectPlay", 0.0),
            mission_time_limit: c.int("MissionTimeLimit", 0),
            round_max: c.int("RoundMax", 0),
            max_player: c.int("MaxPlayer", 0),
            map_cycle,
        })
    }
}

// ---------------------------------------------------------------------
// The modes' clocks (Timers per mode).

/// The round setup is forced to 0 s in these modes (else the server
/// file's `ObjectStart`).
pub fn setup_forced(key: &str) -> bool {
    matches!(
        key,
        "TDM" | "VTDM" | "CDM" | "CTF" | "CCTF" | "FFA" | "VWRM" | "QRT" | "BNT" | "SFM" | "COOP" | "IFS" | "COOP_CF"
    )
}

/// Whether the mode has a warm-up round.
pub fn has_warmup(key: &str) -> bool {
    !matches!(key, "TDM" | "VTDM" | "CTF" | "FFA" | "VWRM" | "SFM" | "BDM" | "COOP" | "IFS" | "COOP_CF")
}

/// A real round's length (s) by mode from `TimeLimit` tl and the goal G;
/// None: no timer. A negative length is an already expired timer (the
/// "≠ 0" modes with a negative `TimeLimit`, Edge cases).
pub fn round_length(key: &str, tl: i32, goal: u32) -> Option<f32> {
    let g = goal as f32;
    let t = tl as f32;
    match key {
        // "≠ 0" tests.
        "TDM" | "VTDM" | "FFA" | "VWRM" | "CCTF" => (tl != 0).then_some(t),
        "CTF" => (tl != 0).then_some(t * g),
        "QRT" => (tl > 0).then_some(t * 60.0),
        "BNT" => {
            if goal > 0 {
                Some(g * 60.0)
            } else {
                (tl > 0).then_some(t * 60.0)
            }
        }
        "HQM" => Some(g * 60.0),
        "SQM2" => Some(t.max(1.0)),
        "SFM" => None,
        _ => (tl > 0).then_some(t),
    }
}

/// The warm-up's length (s): `TrainingTimeLimit` when above 0, else 10 s
/// in Seize and Secure and Squad Match 2, else a real round's length.
pub fn warmup_length(key: &str, tl: i32, ttl: i32, goal: u32) -> Option<f32> {
    if ttl > 0 {
        Some(ttl as f32)
    } else if matches!(key, "HQM" | "SQM2") {
        Some(10.0)
    } else {
        round_length(key, tl, goal)
    }
}

// ---------------------------------------------------------------------
// Finding a map's mission in the install.

/// The room data's mission paths by world and room key, and the mission
/// files read so far.
#[derive(Default)]
pub struct MissionIndex {
    /// (world stem, room key upper case) → mission path (normalized).
    pub paths: HashMap<(String, String), String>,
    files: HashMap<String, Option<Arc<Mission>>>,
}

impl MissionIndex {
    /// Build from a reader of the install's files (`read(path)`, the
    /// normalized path): every `[Map<n>]` record's `GMS_<key>` fields, each
    /// file read once for its world.
    pub fn build(read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Self {
        let mut index = Self::default();
        let Some(room) = read(ROOM_FILE).and_then(|b| Document::parse(&b).ok()) else {
            return index;
        };
        for map in room.records("Map") {
            let mut fields: Vec<(String, String)> = map
                .iter()
                .filter_map(|(k, v)| {
                    let key = k.strip_prefix("gms_")?;
                    let Value::Str(b) = v else { return None };
                    Some((key.to_ascii_uppercase(), b.iter().map(|&c| c as char).collect()))
                })
                .collect();
            fields.sort();
            for (key, path) in fields {
                let path = normalized(&path);
                let Some(m) = index.load(read, &path) else { continue };
                index.paths.entry((m.world_name(), key)).or_insert(path);
            }
        }
        index
    }

    fn load(&mut self, read: &dyn Fn(&str) -> Option<Vec<u8>>, path: &str) -> Option<Arc<Mission>> {
        if let Some(m) = self.files.get(path) {
            return m.clone();
        }
        let m = read(path).and_then(|b| match Document::parse(&b) {
            Ok(doc) => Mission::read(path, &doc).map_err(|e| warn!("mission data: {e}")).ok().map(|mut m| {
                // The mode default file's ladders when the file has none.
                if let Some(d) = default_file_of(path)
                    && let Some(doc) = read(&normalized(&d)).and_then(|b| Document::parse(&b).ok())
                {
                    let defaults: Vec<StepTable> =
                        doc.records("Weapon_Step_Table").into_iter().map(StepTable::read).collect();
                    m.default_ladders = defaults;
                }
                Arc::new(m)
            }),
            Err(e) => {
                warn!("mission data: {path}: {e}");
                None
            }
        });
        self.files.insert(path.to_string(), m.clone());
        m
    }

    /// The mission of world `map` (a stem: `warehouse`) for room key
    /// `key` (`TDM`; `TMM_OFFICIAL` for an official room).
    pub fn find(&self, map: &str, key: &str) -> Option<Arc<Mission>> {
        let path = self.paths.get(&(map.to_ascii_lowercase(), key.to_ascii_uppercase()))?;
        self.files.get(path).cloned().flatten()
    }

    /// Every room key a map is listed for.
    pub fn keys_of(&self, map: &str) -> Vec<String> {
        let map = map.to_ascii_lowercase();
        let mut keys: Vec<String> = self.paths.keys().filter(|(m, _)| *m == map).map(|(_, k)| k.clone()).collect();
        keys.sort();
        keys
    }
}

/// The install's data, read once per session (None without the install
/// or the V1 key: the mission archive is keyed).
pub struct InstallMissions {
    pub index: MissionIndex,
    pub vip: Option<VipData>,
    pub server: Option<ServerFile>,
}

/// The single-player host's server file alone, through `read` (the
/// rules' settings read it at start: `CaData::server_file`).
pub fn read_server_file(read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Option<ServerFile> {
    read(SERVER_CFG)
        .and_then(|b| Document::parse(&b).ok())
        .and_then(|d| ServerFile::read(&d).map_err(|e| warn!("{e}")).ok())
}

/// Read the install's missions through `read`.
pub fn read_install(read: &dyn Fn(&str) -> Option<Vec<u8>>) -> InstallMissions {
    let index = MissionIndex::build(read);
    let vip = read(VTDM_FILE).and_then(|b| Document::parse(&b).ok()).map(|d| VipData::read(&d));
    let server = read_server_file(read);
    if index.paths.is_empty() {
        info!("Combat Arms: no mission data read (GMS.rez needs the V1 key): our modes' numbers stand");
    } else {
        info!("Combat Arms: mission data for {} (map, mode) pairs", index.paths.len());
    }
    InstallMissions { index, vip, server }
}

// ---------------------------------------------------------------------
// What the rules read: the playing (map, mode)'s mission, else our data.

/// The install's mission of the (map, mode) playing, found when a match
/// starts afresh (`resolve`; None: none in the install, or no install,
/// so our data stands). A test may put its own in after the match was
/// set up.
#[derive(Resource, Clone, Debug, Default)]
pub struct CaMission(pub Option<Arc<Mission>>);

/// The (map, mode) `resolve` last looked the mission up for.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaMissionFor(pub Option<(String, String)>);

/// The playing (map, mode): the loaded map's world name and the room key.
fn playing(world: &World) -> (String, String) {
    let key = world.get_resource::<CaRuleSettings>().map(|s| s.mode.clone()).unwrap_or_default();
    let map = world
        .get_resource::<crate::map::MapFile>()
        .map(|m| super::world::map_name(&m.name))
        .unwrap_or_default();
    (map, key)
}

/// Find the playing (map, mode)'s mission (`flow::restart`).
pub fn resolve(world: &mut World) {
    let (map, key) = playing(world);
    world.insert_resource(CaMissionFor(Some((map.clone(), key.clone()))));
    let data = world.get_resource_or_insert_with(super::data::CaData::default).clone();
    let found = data.missions().and_then(|i| i.index.find(&map, &key));
    if let Some(m) = &found {
        if m.mission.mode != key {
            warn!("{}: the mission file names mode {} (the room's is {key}): playing {key}", m.path, m.mission.mode);
        }
        info!("mission data: {} for {map} {key}", m.path);
    }
    world.insert_resource(CaMission(found));
}

/// Find the mission unless it was found for the playing (map, mode)
/// already (`flow`'s mission start): a map that started without a fresh
/// game (`gameplay::NewGame`), as the app's `--map` does, still plays its
/// mission and sends its map messages.
pub fn ensure_resolved(world: &mut World) {
    let now = Some(playing(world));
    if world.get_resource::<CaMissionFor>().is_none_or(|f| f.0 != now) {
        resolve(world);
    }
}

fn installed(world: &World) -> Option<Arc<Mission>> {
    world.get_resource::<CaMission>().and_then(|m| m.0.clone())
}

/// The mode key the clocks follow: the mission file's `MissionMode`, else
/// the room's.
pub fn mode_key(world: &World) -> String {
    installed(world)
        .map(|m| m.mission.mode.clone())
        .unwrap_or_else(|| world.resource::<CaRuleSettings>().mode.clone())
}

/// `[Object0]`'s `TimeLimit` and `TrainingTimeLimit` as stored (minutes
/// in Quarantine and Spy Hunt): the install's, else our data's (its
/// seconds, as minutes there).
pub fn time_limits(world: &World) -> (i32, i32) {
    if let Some(m) = installed(world) {
        let o = m.object();
        return (o.time_limit, o.training_time_limit);
    }
    let settings = world.resource::<CaRuleSettings>();
    let data = settings.mode_data();
    let minutes = matches!(settings.mode.as_str(), "QRT" | "BNT");
    let raw = |t: Option<f32>, m: bool| t.map_or(0, |t| if m { (t / 60.0).round() as i32 } else { t.round() as i32 });
    (raw(data.time_limit, minutes), raw(data.training_time_limit, false))
}

/// A real round's length (s), None: no timer.
pub fn real_round(world: &World) -> Option<f32> {
    let (tl, _) = time_limits(world);
    round_length(&mode_key(world), tl, world.resource::<CaRuleSettings>().goal())
}

/// The warm-up's length (s), None: no timer.
pub fn warmup(world: &World) -> Option<f32> {
    let (tl, ttl) = time_limits(world);
    warmup_length(&mode_key(world), tl, ttl, world.resource::<CaRuleSettings>().goal())
}

/// The round setup's length (s): 0 in the modes that force it, else the
/// server file's `ObjectStart`.
pub fn setup_length(world: &World) -> f32 {
    if setup_forced(&mode_key(world)) { 0.0 } else { world.resource::<CaRuleSettings>().object_start }
}

/// (offence, defence): the mission's, else JSF and Creed (every stock
/// file).
pub fn sides(world: &World) -> (u8, u8) {
    match installed(world) {
        Some(m) => {
            let o = m.object();
            (o.offence, o.defence)
        }
        None => (JSF, CREED),
    }
}

/// The objective weapons of `[Object0]`'s `JSFWeapon` (`team` 1) or
/// `CreedWeapon` (2), as our weapon documents (an unknown name skipped
/// and logged: the game would crash, Edge cases).
pub fn objective_weapons(world: &World, team: u8) -> Vec<&'static str> {
    let names = match installed(world) {
        Some(m) => {
            let o = m.object();
            if team == JSF { o.jsf_weapon } else { o.creed_weapon }
        }
        None => {
            let d = world.resource::<CaRuleSettings>().mode_data();
            if team == JSF { d.jsf_weapon } else { d.creed_weapon }
        }
    };
    names
        .iter()
        .filter_map(|n| {
            let w = world.resource::<super::data::CaData>().weapon_for_name(n);
            if w.is_none() {
                warn!("mission data: no weapon named {n}");
            }
            w
        })
        .collect()
}

/// A loadout of install weapon ids as our documents (the unknown ones
/// skipped).
pub fn weapons_of(data: &super::data::CaDataInner, ids: &[i32]) -> Vec<&'static str> {
    ids.iter().filter_map(|i| data.weapon_for_index(*i)).collect()
}

/// Quarantine's numbers (F4.1): the mission's, else our data's.
#[derive(Clone, Debug, PartialEq)]
pub struct QuarantineNumbers {
    pub min_hosts: u32,
    pub selection_rate: f32,
    /// The heal: period (s), fraction of the maximum hit points, the wait
    /// standing still (s).
    pub heal_period: f32,
    pub heal: f32,
    pub heal_wait: f32,
    pub kill_point: i32,
    pub dead_point: i32,
    pub infect_point: i32,
    pub infected_point: i32,
    pub human_win_point: i32,
    pub infect_win_point: i32,
    /// The infected's and the hosts' weapons (our documents; empty: our
    /// knife).
    pub infected_weapons: Vec<&'static str>,
    pub host_weapons: Vec<&'static str>,
}

pub fn quarantine(world: &World) -> QuarantineNumbers {
    quarantine_in(
        world.get_resource::<CaMission>(),
        world.resource::<CaRuleSettings>(),
        world.resource::<super::data::CaData>(),
    )
}

/// `quarantine` from the resources (a system's).
pub fn quarantine_in(mission: Option<&CaMission>, settings: &CaRuleSettings, data: &super::data::CaData) -> QuarantineNumbers {
    let o = settings.objectives.clone();
    let mut n = QuarantineNumbers {
        min_hosts: o.min_hosts,
        selection_rate: o.selection_rate,
        heal_period: o.infected_heal_period,
        heal: o.infected_heal,
        heal_wait: o.infected_still,
        kill_point: o.human_kills_infected,
        dead_point: o.infected_killed_by_human,
        infect_point: o.infected_infects,
        infected_point: 0,
        human_win_point: o.humans_win,
        infect_win_point: o.infected_win,
        infected_weapons: Vec::new(),
        host_weapons: Vec::new(),
    };
    if let Some(q) = mission.and_then(|m| m.0.clone()).and_then(|m| m.quarantine.clone()) {
        n.min_hosts = q.min_hosts.max(0) as u32;
        n.selection_rate = q.selection_rate as f32;
        if let Some((tick, heal, wait)) = q.heal {
            (n.heal_period, n.heal, n.heal_wait) = (tick as f32, heal as f32, wait as f32);
        }
        if let Some(p) = q.points {
            n.kill_point = p[1];
            n.dead_point = p[2];
            n.infect_point = p[3];
            n.infected_point = p[4];
            n.human_win_point = p[5];
            n.infect_win_point = p[6];
        }
        n.infected_weapons = q.infected.as_ref().map(|c| weapons_of(data, &c.weapons)).unwrap_or_default();
        n.host_weapons = q.host.as_ref().map(|c| weapons_of(data, &c.weapons)).unwrap_or_default();
    }
    n
}

/// Spy Hunt's numbers (F4.2): intel items, the stage-2 time (s), the
/// camera hold (s).
pub fn spy_hunt(world: &World) -> (u32, f32, f32) {
    spy_hunt_in(world.get_resource::<CaMission>(), world.resource::<CaRuleSettings>())
}

/// `spy_hunt` from the resources (a system's).
pub fn spy_hunt_in(mission: Option<&CaMission>, settings: &CaRuleSettings) -> (u32, f32, f32) {
    let o = &settings.objectives;
    match mission.and_then(|m| m.0.clone()).and_then(|m| m.spy_hunt.clone()) {
        Some(s) => (s.intel_total.max(0) as u32, s.stage2_time as f32, s.agent_camera_hold as f32),
        None => (o.intel_count, o.stage2_time, o.agent_camera_hold),
    }
}

/// Seize and Secure's numbers (F4.4): `FlagMoveTime` (s) and
/// `LastOccupyBonus` (a mission without the section: the built-in 120 s
/// and 50).
pub fn seize(world: &World) -> (f32, i32) {
    seize_in(world.get_resource::<CaMission>(), world.resource::<CaRuleSettings>())
}

/// `seize` from the resources (a system's).
pub fn seize_in(mission: Option<&CaMission>, settings: &CaRuleSettings) -> (f32, i32) {
    let o = &settings.objectives;
    match mission.and_then(|m| m.0.clone()).map(|m| m.seize.clone().unwrap_or_default()) {
        Some(s) => (s.flag_move_time as f32, s.last_occupy_bonus),
        None => (o.flag_move_time, o.last_occupy_bonus),
    }
}

/// The use time the mission sets on activate trigger `name`, else our
/// `seize_time`.
pub fn activate_time(world: &World, name: &str) -> f32 {
    activate_time_in(world.get_resource::<CaMission>(), world.resource::<CaRuleSettings>(), name)
}

/// `activate_time` from the resources (a system's).
pub fn activate_time_in(mission: Option<&CaMission>, settings: &CaRuleSettings, name: &str) -> f32 {
    let fallback = settings.objectives.seize_time;
    mission
        .and_then(|m| m.0.clone())
        .and_then(|m| m.object().activate.iter().find(|(n, _)| n == name).map(|(_, t)| *t as f32))
        .unwrap_or(fallback)
}

/// The Arms Race ladders G uses (F4.8): the mission's own or its mode
/// default file's, else our data's stock ladders.
pub fn ladders(world: &World) -> Vec<StepTable> {
    let goal = world.resource::<CaRuleSettings>().goal() as i32;
    match installed(world) {
        Some(m) => ladders_for(&m.ladders, &m.default_ladders, goal),
        None => ladders_for(&world.resource::<CaRuleSettings>().objectives.ladders(), &[], goal),
    }
}

/// A ladder's weapon id as our document: the install's record (V1),
/// else our stand-in for that stock id (data/modes.toml).
pub fn ladder_weapon(world: &World, id: i32) -> Option<&'static str> {
    let data = world.resource::<super::data::CaData>();
    data.weapon_for_index(id).or_else(|| {
        let ours = world.resource::<CaRuleSettings>().objectives.stand_in(id)?;
        data.weapon_set().get(&ours).map(|w| w.id.as_str())
    })
}

/// VIP Elimination's table (F4.7): the install's `VTDM.GMS` when the
/// playing map's mission is the install's, else our data's stock one.
pub fn vip(world: &World) -> VipData {
    if installed(world).is_some()
        && let Some(v) = world.resource::<super::data::CaData>().missions().and_then(|i| i.vip.clone())
    {
        return v;
    }
    let o = &world.resource::<CaRuleSettings>().objectives;
    VipData {
        tables: o.vip_tables.clone(),
        weights: o.vip_points.clone(),
    }
}

/// `[Object<G>]`'s co-op lives (`LifeCount`, `MaxLifeCount`) for the
/// difficulty G (no co-operative mode plays them yet).
pub fn lives(world: &World) -> (i32, i32) {
    let goal = world.resource::<CaRuleSettings>().goal() as i32;
    installed(world).map_or((0, 0), |m| {
        let o = m.coop_object(goal);
        (o.life_count, o.max_life_count)
    })
}

/// A step of the match where the mission names map objects to message
/// (game_modes.md S3, S15; missions.md F2, F3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapStep {
    /// S3 step 2: `[Mission0]`'s `Controller` `ON`, its `StartTrigger`
    /// `TRIGGER`.
    MissionStart,
    /// S3 step 3: the objective's `Controller` `ON`, its `StartTrigger`
    /// `TRIGGER`.
    RoundSetup,
    /// S3 step 5: the objective's `EndTrigger` `TRIGGER`.
    RoundOver,
    /// S3 step 6 (every round ends its mission): `[Mission0]`'s
    /// `EndTrigger` `TRIGGER`.
    MissionEnd,
}

impl Mission {
    /// The (object name, message) pairs of `step`, in order: controllers
    /// before triggers; `object` the objective playing.
    pub fn map_messages(&self, step: MapStep, object: &ObjectRecord) -> Vec<(String, &'static str)> {
        let each = |names: &[String], message: &'static str| {
            names.iter().filter(|n| !n.is_empty()).map(|n| (n.clone(), message)).collect::<Vec<_>>()
        };
        match step {
            MapStep::MissionStart => {
                [each(&self.mission.controller, "ON"), each(&self.mission.start_trigger, "TRIGGER")].concat()
            }
            MapStep::RoundSetup => [each(&object.controller, "ON"), each(&object.start_trigger, "TRIGGER")].concat(),
            MapStep::RoundOver => each(&object.end_trigger, "TRIGGER"),
            MapStep::MissionEnd => each(&self.mission.end_trigger, "TRIGGER"),
        }
    }
}

/// The playing mission's map messages for `step` (none without the
/// install's mission: our data names no map objects).
pub fn map_messages(world: &World, step: MapStep) -> Vec<(String, &'static str)> {
    installed(world).map_or_else(Vec::new, |m| m.map_messages(step, &m.object()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_lists() {
        assert_eq!(name_list("XK-2"), vec!["XK-2"]);
        assert_eq!(name_list("a;b;"), vec!["a", "b"]);
        assert_eq!(name_list("a;;b"), vec!["a", "", "b"]);
        assert!(name_list("").is_empty());
    }

    #[test]
    fn default_files() {
        assert_eq!(default_file_of("GMS\\GHOSTTOWN_VWRM.GMS").as_deref(), Some("GMS\\VWRM.GMS"));
        assert_eq!(default_file_of("GMS\\SNOWVALLEY_M_OFFICIAL.GMS").as_deref(), Some("GMS\\OFFICIAL.GMS"));
        assert_eq!(default_file_of("GMS\\BLOODMONEY.GMS"), None);
    }
}
