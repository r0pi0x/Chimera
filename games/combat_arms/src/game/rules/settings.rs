//! The modes' settings (specs/combat_arms/game_modes.md S2): the room's
//! choices (the mode, the goal G, friendly fire, join in progress), the
//! dedicated server's file (round setup, round-over and intermission
//! lengths), the shared constants (respawn waits, spawn protection, the
//! warm-up's revive period) and each mode's goal range and mission data
//! (its round `TimeLimit` and warm-up `TrainingTimeLimit`), as data
//! (data/modes.toml, `rule_module::parse_settings`) the `ca_*` cvars set
//! after.
//!
//! The per-map mission files are `game::missions`' (their `TimeLimit`,
//! `TrainingTimeLimit`, objective weapons and mode sections over the
//! numbers here, which stay the fallback: the stock values of
//! missions.md); with the install, the single-player host's server file
//! (`ATTRIBUTES\SERVER.CFG`) sets the phase lengths at start-up.
//!
//! Known shortcuts: `ca_goal` takes any value (the lobby's range and step
//! aren't enforced: 0 is the mode's default); the respawn waits and the
//! protection run on the game's clock, not the OS millisecond clocks.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::Deserialize;

use super::super::weapons::CaAiDamage;
use crate::{console::resource_cvar, rule_module};

/// data/modes.toml, built in.
pub const MODES_DATA: &str = include_str!("../../../data/modes.toml");

/// A mode's own numbers (S2's goal table, the mission files).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModeData {
    pub goal_min: u32,
    pub goal_max: u32,
    pub goal_step: u32,
    pub goal_default: u32,
    /// The mission's `TimeLimit` (s); None: the round has no timer.
    pub time_limit: Option<f32>,
    /// The mission's `TrainingTimeLimit` (s): the warm-up round's length,
    /// for the modes that have one.
    pub training_time_limit: Option<f32>,
    /// The mission's objective weapons (`JSFWeapon`, `CreedWeapon`: weapon
    /// `Name`s).
    #[serde(default)]
    pub jsf_weapon: Vec<String>,
    #[serde(default)]
    pub creed_weapon: Vec<String>,
}

/// The objectives' numbers (data/modes.toml `[objectives]`): the shared
/// constants of the bomb and the flags, and the mode sections' mission
/// data.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ObjectiveData {
    /// M4: a dropped flag goes home after this (s).
    pub flag_return: f32,
    /// M3: the plant gauge (s), the defuse and the fuse a site without its
    /// own `DefuseTime` and `DetonateTime` has (s), the plant margin (u).
    pub bomb_plant: f32,
    pub bomb_defuse: f32,
    pub bomb_fuse: f32,
    pub plant_margin: f32,
    /// M10: the triggers' use time (s), `FlagMoveTime` (s),
    /// `LastOccupyBonus` (points).
    pub seize_time: f32,
    pub flag_move_time: f32,
    pub last_occupy_bonus: i32,
    /// M8: host selection's delay (s), `SelectionRate`, `MinHostNum`; the
    /// infected's heal (hit points), the stillness before it and its
    /// period (s); the points.
    pub host_delay: f32,
    pub selection_rate: f32,
    pub min_hosts: u32,
    pub infected_heal: f32,
    pub infected_still: f32,
    pub infected_heal_period: f32,
    pub human_kills_infected: i32,
    pub infected_killed_by_human: i32,
    pub infected_infects: i32,
    pub humans_win: i32,
    pub infected_win: i32,
    /// M7: intel items a round.
    pub intel_count: u32,
    /// M5: the share of G (rounded) the kills must pass for a round ended
    /// by leavers to have a winner.
    pub leave_kill_share: f32,
    /// M12: the points a death is worth by the victim's VIP rank (the last
    /// for higher ranks).
    pub vip_points: Vec<i32>,
    /// M12 (missions.md F4.7): the VIP tables, (`TargetPoint`, VIPs for a
    /// team of 1..8).
    pub vip_tables: Vec<(i32, [i32; 8])>,
    /// M7: `Stage2PlayTime` and `AgentCameraHoldingTime`, s.
    pub stage2_time: f32,
    pub agent_camera_hold: f32,
    /// M14 (missions.md F4.8): the default ladder's `Percent` and its
    /// stock steps per goal; our weapon standing in for each stock weapon
    /// id without the install.
    pub ladder_percent: i32,
    pub ladders: Vec<LadderData>,
    pub stand_ins: BTreeMap<String, String>,
    /// S12: each box item's pick weights (lose, even, win; items 0..9,
    /// then the revenge buff's 10).
    pub mutation_weights: Vec<[u32; 3]>,
}

/// One stock Arms Race ladder: its goal and steps (kills, weapon id,
/// weapon type).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LadderData {
    pub goal: i32,
    pub steps: Vec<[i32; 3]>,
}

impl ObjectiveData {
    /// The stock ladders as the mission files' tables (one candidate of
    /// weight 100 per step, everyone's: `ItemNo` -1).
    pub fn ladders(&self) -> Vec<super::super::missions::StepTable> {
        use super::super::missions::{Step, StepTable};
        self.ladders
            .iter()
            .map(|l| StepTable {
                goal: l.goal,
                item_no: -1,
                percent: self.ladder_percent,
                steps: l
                    .steps
                    .iter()
                    .map(|[kill, id, kind]| Step {
                        kill: *kill,
                        weapon_type: *kind,
                        candidates: vec![(*id, 100)],
                    })
                    .collect(),
            })
            .collect()
    }

    /// Our weapon standing in for stock weapon id `id`.
    pub fn stand_in(&self, id: i32) -> Option<String> {
        self.stand_ins.get(&id.to_string()).cloned()
    }
}

/// Hired Guns' numbers (data/modes.toml `[hired_guns]`; game_modes_2.md
/// M11, missions.md F4.5).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HiredGunsData {
    /// The drop points' and spawn points' game type.
    pub family: String,
    pub drop_bullion_num: u32,
    pub bullion_amount: i32,
    pub base_bank: i32,
    pub round_win_bonus: i32,
    pub kill_bonus: i32,
    pub death_bonus: i32,
    /// Bars and bags are taken within this (u).
    pub pickup_reach: f32,
    /// The carry penalty's sections: first bar, last bar, rate per bar.
    pub penalty_sections: Vec<(u32, u32, f32)>,
    pub ai_damage_factor: f32,
    pub merc_cap: u32,
    pub unit_spawn_alpha: String,
    pub unit_spawn_bravo: String,
    pub body_lifetime: f32,
    /// The hire menu's units by unit id.
    pub units: Vec<MercUnit>,
}

impl HiredGunsData {
    /// The carry penalty for `n` bars (missions.md F4.5): the last section
    /// holding n gives P(b_{i-1}) + r_i (n − b_{i-1}) (the first: r_0 n);
    /// none: 0.
    pub fn penalty(&self, n: u32) -> f32 {
        let s = &self.penalty_sections;
        let Some(i) = s.iter().rposition(|(a, b, _)| (*a..=*b).contains(&n)) else { return 0.0 };
        let mut p = 0.0;
        for (k, (_, b, r)) in s.iter().enumerate().take(i + 1) {
            let from = if k == 0 { 0 } else { s[k - 1].1 };
            let to = if k == i { n } else { *b };
            p += r * to.saturating_sub(from) as f32;
        }
        p
    }
}

/// A hireable unit (the hire menu's row, its template's weapons).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MercUnit {
    pub template: String,
    pub name: String,
    pub cost: i32,
    pub weapon: String,
    pub grenade: String,
}

/// VIP Escort's numbers (data/modes.toml `[escort]`; game_modes_2.md M13,
/// missions.md F4.6).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EscortData {
    pub family: String,
    pub vip_spawn: String,
    pub escape_zone: String,
    pub exit_time: f32,
    pub exit_arrival: f32,
    pub end_hold: f32,
    pub swap_wait: f32,
    pub swap_notice: f32,
    pub vip_weapons: Vec<String>,
    pub vip_move_penalty: f32,
}

/// The AI damage cap (game_modes_2.md Constants (shared)).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AiDamageData {
    pub cap: f32,
    pub cap_big: f32,
    pub big: f32,
}

impl AiDamageData {
    /// One AI hit's health damage `hp` (HP) on a player, capped (a melee
    /// hit isn't).
    pub fn capped(&self, hp: f32, melee: bool) -> f32 {
        if melee {
            hp
        } else if hp >= self.big {
            self.cap_big
        } else {
            hp.min(self.cap)
        }
    }
}

/// The settings the rules read (data/modes.toml, then the cvars).
#[derive(Resource, Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CaRuleSettings {
    /// The room's mode by its room key (`TDM`, `TSV`), or `none`.
    pub mode: String,
    /// The goal G; 0: the mode's default.
    pub goal: u32,
    /// Friendly fire (S8): 1 on.
    pub friendly_fire: u8,
    /// Join in progress (S1): 1 on.
    pub join_in_progress: u8,
    /// `ObjectStart`, `ObjectEnd`, `MissionEnd` (S3), s.
    pub object_start: f32,
    pub object_end: f32,
    pub mission_end: f32,
    /// S5: the client's wait before it asks, the server's least wait.
    pub respawn_wait_client: f32,
    pub respawn_wait_server: f32,
    /// S6: shots pass through, all damage blocked (s).
    pub protect_bullets: f32,
    pub protect_all: f32,
    /// S9: the warm-up revives the dead this often (s).
    pub warmup_revive: f32,
    /// S1: round modes hold a joiner with at least this many players in.
    pub late_join_wait: u32,
    /// Each mode's numbers, by room key.
    pub modes: BTreeMap<String, ModeData>,
    /// The objectives' numbers.
    pub objectives: ObjectiveData,
    /// Hired Guns'.
    pub hired_guns: HiredGunsData,
    /// VIP Escort's.
    pub escort: EscortData,
    /// The AI damage cap.
    pub ai_damage: AiDamageData,
}

impl Default for CaRuleSettings {
    fn default() -> Self {
        rule_module::parse_settings("data/modes.toml", MODES_DATA).unwrap_or_else(|e| panic!("{e}"))
    }
}

impl CaRuleSettings {
    /// The running mode's numbers (the defaults for a mode without any).
    pub fn mode_data(&self) -> ModeData {
        self.modes.get(&self.mode).cloned().unwrap_or_default()
    }

    /// G: the room's goal, else the mode's default.
    pub fn goal(&self) -> u32 {
        if self.goal > 0 { self.goal } else { self.mode_data().goal_default }
    }
}

/// The settings and their cvars.
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<CaRuleSettings>();
    // The single-player host's server file (missions.md, The server file):
    // the phase lengths, over ours (the same values in the stock copy).
    if let Some(server) = super::super::data::CaData::of(app).missions().and_then(|i| i.server.clone()) {
        let mut s = app.world_mut().resource_mut::<CaRuleSettings>();
        s.object_start = server.object_start as f32;
        s.object_end = server.object_end as f32;
        s.mission_end = server.mission_end as f32;
    }
    // The AI cap from the data; the factor is the console variable's.
    let cap = app.world().resource::<CaRuleSettings>().ai_damage.clone();
    let factor = app.world().get_resource::<CaAiDamage>().map_or(CaAiDamage::default().factor, |a| a.factor);
    app.insert_resource(CaAiDamage {
        factor,
        cap: cap.cap,
        cap_big: cap.cap_big,
        big: cap.big,
    });
    resource_cvar::<CaAiDamage, f32>(
        app,
        "ca_ai_damage_factor",
        "AIDamageFactor: every AI's (bot's, mercenary's) damage x this before armour (0.8; a Hired Guns round sets \
         its objective's).",
        |s| &mut s.factor,
    );

    resource_cvar::<CaRuleSettings, String>(
        app,
        "ca_mode",
        "The game mode by its room key: TDM (Elimination), TSV (Elimination Pro), TMM (Search & Destroy), CTF \
         (Capture the Flag), FFA (One Man Army), LMS (Last Man Standing), BNT (Spy Hunt), QRT (Quarantine Regen), \
         SBT (Bombing Run), HQM (Seize and Secure), SQM2 (Squad Match 2), VTDM (VIP Elimination), VWRM (Arms \
         Race), HG (Hunted), ESL (S&D for the Go4CA Cup), SQM (Hired Guns), ESC (VIP Escort), the clan modes \
         CDM, CMM, CCTF, CTSV, CESC; none. Takes effect at the next map load or ca_restart.",
        |s| &mut s.mode,
    );
    resource_cvar::<CaRuleSettings, u32>(app, "ca_goal", "The goal G (kills, round wins, captures, rounds or minutes, by mode); 0: the mode's default.", |s| {
        &mut s.goal
    });
    resource_cvar::<CaRuleSettings, u8>(
        app,
        "ca_friendlyfire",
        "1: teammates hurt each other (NetFriendlyFire; bullets and melee at a quarter in the listed modes).",
        |s| &mut s.friendly_fire,
    );
    resource_cvar::<CaRuleSettings, u8>(
        app,
        "ca_join_in_progress",
        "1: players may join a running match.",
        |s| &mut s.join_in_progress,
    );
    resource_cvar::<CaRuleSettings, f32>(app, "ca_object_start", "Round setup length, s (ObjectStart).", |s| {
        &mut s.object_start
    });
    resource_cvar::<CaRuleSettings, f32>(app, "ca_object_end", "Round-over length, s (ObjectEnd).", |s| {
        &mut s.object_end
    });
    resource_cvar::<CaRuleSettings, f32>(app, "ca_mission_end", "Intermission length, s (MissionEnd).", |s| {
        &mut s.mission_end
    });
    resource_cvar::<CaRuleSettings, f32>(
        app,
        "ca_respawn_wait",
        "The server refuses a respawn sooner than this after a death, s.",
        |s| &mut s.respawn_wait_server,
    );
    resource_cvar::<CaRuleSettings, f32>(
        app,
        "ca_respawn_wait_client",
        "A dead player asks to respawn after this, s.",
        |s| &mut s.respawn_wait_client,
    );
    resource_cvar::<CaRuleSettings, f32>(
        app,
        "ca_respawn_invulnerability",
        "After a respawn, no damage at all for this long, s (RespawnInvulnerabilityTime).",
        |s| &mut s.protect_all,
    );
    resource_cvar::<CaRuleSettings, f32>(
        app,
        "ca_respawn_bullet_protection",
        "After a respawn, instant-hit shots pass through for this long, s.",
        |s| &mut s.protect_bullets,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_data_reads() {
        let s = CaRuleSettings::default();
        assert_eq!(s.mode, "TDM");
        assert_eq!(s.goal(), 100);
        assert_eq!(s.mode_data().time_limit, None);
        let tsv = &s.modes["TSV"];
        assert_eq!((tsv.goal_default, tsv.time_limit, tsv.training_time_limit), (7, Some(180.0), Some(10.0)));
        assert_eq!((s.object_start, s.object_end, s.mission_end), (0.0, 3.0, 3.0));
    }

    /// game_modes_2.md test case 4 (the stock table, missions.md F4.5).
    #[test]
    fn the_carry_penalty() {
        let h = CaRuleSettings::default().hired_guns;
        let p = [10, 11, 20, 25, 30, 40, 80, 81].map(|n| h.penalty(n));
        let want = [0.0, 0.03, 0.30, 0.45, 0.45, 0.45, 0.45, 0.0];
        for (got, want) in p.iter().zip(want) {
            assert!((got - want).abs() < 1e-5, "{p:?}");
        }
        assert_eq!(h.units.len(), 10);
        assert_eq!(h.units.iter().map(|u| u.cost).collect::<Vec<_>>(), [200, 200, 200, 200, 500, 500, 500, 500, 500, 500]);
    }

    /// Test case 53: the AI cap.
    #[test]
    fn the_ai_cap() {
        let a = CaRuleSettings::default().ai_damage;
        assert_eq!((a.capped(120.0, false), a.capped(1500.0, false), a.capped(120.0, true)), (50.0, 990.0, 120.0));
    }
}
