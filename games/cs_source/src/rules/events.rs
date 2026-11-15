//! CS:S's game events and server log lines (`game_log`), by the specs'
//! names: what happens to the bomb, hostages, rounds, weapons and players,
//! written by the server (and single player) for plugins and admins.
//!
//! - Objectives (objectives.md 2-8): `bomb_dropped`, `bomb_pickup`,
//!   `bomb_beginplant`, `bomb_abortplant`, `bomb_planted` (site, posx, posy),
//!   `bomb_beep`, `bomb_begindefuse` (haskit), `bomb_abortdefuse`,
//!   `bomb_defused`, `bomb_exploded`, `hostage_follows`,
//!   `hostage_stops_following`, `hostage_hurt`, `hostage_killed`,
//!   `hostage_rescued` (site), `hostage_rescued_all`; log lines `triggered`
//!   "Dropped_The_Bomb", "Got_The_Bomb", "Planted_The_Bomb",
//!   "Begin_Bomb_Defuse_With_Kit" / "Begin_Bomb_Defuse_Without_Kit",
//!   "Defused_The_Bomb", "Touched_A_Hostage", "Killed_A_Hostage",
//!   "Rescued_A_Hostage".
//! - Rounds (objectives.md 7, questions 6 and 15; parity-gaps.md C9):
//!   `round_start`, `round_end` (winner, message); log `Team "<team>"
//!   triggered "<reason>"` (`Terrorists_Win`, `CTs_Win`, `Target_Bombed`,
//!   `Bomb_Defused`, `Target_Saved`, `All_Hostages_Rescued`,
//!   `Hostages_Not_Rescued`) or `World triggered "Round_Draw"`, then `World
//!   triggered "Round_End"`; `World triggered "Intermission_Time_Limit"`
//!   (`game_end::begin`).
//! - Weapons and players (weapons.md's probe events, grenades.md 5-6):
//!   `weapon_fire` (weapon), `bullet_impact` (x, y, z: every surface a
//!   bullet enters), `weapon_reload`, `item_pickup` (item: a loose weapon
//!   taken), `player_hurt` (attacker, health, dmg_health, hitgroup, weapon),
//!   `player_death` (attacker, weapon, headshot, dominated, revenge: 0/1,
//!   dominations.md 3), `hegrenade_detonate` and
//!   `flashbang_detonate` (x, y, z); log `"<A>" killed "<B>" with
//!   "<weapon>"` and `"<B>" committed suicide with "<weapon>"` ("world"
//!   with none; measured, objectives.md Q17).
//!
//! Every event has the player it is about as `userid`; positions are in
//! Source units and axes. Team numbers in events are Source's (2 T, 3 CT).
//!
//! Known shortcuts (docs/tech-debt.md): `site` is the target's index among
//! the map's bomb targets / rescue zones (Source's: the entity index), and
//! only `bomb_planted` and `hostage_rescued` carry it (CS:S's
//! beginplant, abortplant, defused and exploded have it too, exploded a
//! `userid`); `player_hurt` has no `dmg_armor` or `armor`; `round_end` no `reason` number; a hostage is
//! named by its entity index; no `(headshot)` suffix on kill lines
//! (unmeasured). Not written (measured 2026-10-18, objectives.md Q17):
//! `attacked` lines (`mp_logdetail`), say, connect, enter, disconnect and
//! team-join lines, the `Restart_Round_(1_second)` and `Team "CT" scored
//! "n" with "m" players` lines; `weapon_zoom`, `player_blind`,
//! `grenade_bounce`, `smokegrenade_detonate`, `item_pickup` for bought or
//! given weapons. CS:S logs no purchases.
//! Why it's OK for now: the spec-defined events and lines are in; the rest
//! need a spec session on the log standard and the event list.

use bevy::prelude::*;

use super::rounds::{ATTACKERS, DEFENDERS, RoundEndReason, RoundEnded};
use crate::{
    core::{DamageTaken, Died, Health, RoundRestarts, Team, authoritative},
    game_log::{EventValue, GameEvent, LogLine, LogPart, ServerLog},
    objectives::{
        ObjectiveEvent, UNIT,
        hostages::{Hostage, HostageTally},
    },
    weapon::{
        Inventory, Weapon, WeaponEvent, WeaponEventKind,
        drop::PickedUp,
        grenade::{Detonated, GrenadeKind},
    },
};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ServerLog>();
    app.world_mut().resource_mut::<ServerLog>().team_names = vec![
        (ATTACKERS.0, "TERRORIST".into()),
        (DEFENDERS.0, "CT".into()),
    ];
    app.add_systems(
        PostUpdate,
        (objective_events, round_events, weapon_events, player_events)
            .after(chimera_sim::dominations::DominationSet)
            .run_if(authoritative)
            .run_if(crate::objectives::plays),
    );
}

/// A weapon id as CS:S's events name it: `cs_source:weapon_ak47` -> `ak47`.
pub fn short_weapon(id: &str) -> &str {
    let id = id.rsplit(':').next().unwrap_or(id);
    id.strip_prefix("weapon_").unwrap_or(id)
}

/// Source's team number for ours (T 2, CT 3; 0 otherwise).
fn source_team(team: Option<Team>) -> i64 {
    match team {
        Some(ATTACKERS) => 2,
        Some(DEFENDERS) => 3,
        _ => 0,
    }
}

/// Engine space (meters, Y up) to Source's units and axes (Z up).
fn units(at: Vec3) -> Vec3 {
    Vec3::new(at.x, -at.z, at.y) / UNIT
}

fn player(e: Entity) -> EventValue {
    EventValue::Player(e)
}

fn xyz(event: GameEvent, at: Vec3, z: bool) -> GameEvent {
    let p = units(at);
    let e = event
        .with("x", EventValue::Float(p.x))
        .with("y", EventValue::Float(p.y));
    if z { e.with("z", EventValue::Float(p.z)) } else { e }
}

fn objective_events(
    mut events: MessageReader<ObjectiveEvent>,
    tally: Option<Res<HostageTally>>,
    mut out: MessageWriter<GameEvent>,
    mut log: MessageWriter<LogLine>,
) {
    use ObjectiveEvent as E;
    let mut rescued = false;
    for e in events.read() {
        match *e {
            E::DroppedBomb { who } => {
                out.write(GameEvent::of("bomb_dropped", who));
                log.write(LogLine::triggered(who, "Dropped_The_Bomb"));
            }
            E::PickedUpBomb { who } => {
                out.write(GameEvent::of("bomb_pickup", who));
                log.write(LogLine::triggered(who, "Got_The_Bomb"));
            }
            E::BeginPlant { who } => {
                out.write(GameEvent::of("bomb_beginplant", who));
            }
            E::AbortPlant { who, .. } => {
                out.write(GameEvent::of("bomb_abortplant", who));
            }
            E::Planted { who, at, site } => {
                // `posx`, `posy` (CS:S's modevents.res).
                let p = units(at);
                out.write(
                    GameEvent::of("bomb_planted", who)
                        .with("site", EventValue::Int(site.map_or(-1, |s| s as i64)))
                        .with("posx", EventValue::Int(p.x.round() as i64))
                        .with("posy", EventValue::Int(p.y.round() as i64)),
                );
                log.write(LogLine::triggered(who, "Planted_The_Bomb"));
            }
            E::Beep { .. } => {
                out.write(GameEvent::new("bomb_beep"));
            }
            E::BeginDefuse { who, kit } => {
                out.write(GameEvent::of("bomb_begindefuse", who).with("haskit", EventValue::Bool(kit)));
                // Measured 2026-10-18 (objectives.md Q17).
                let what = if kit { "Begin_Bomb_Defuse_With_Kit" } else { "Begin_Bomb_Defuse_Without_Kit" };
                log.write(LogLine::triggered(who, what));
            }
            E::AbortDefuse { who } => {
                out.write(GameEvent::of("bomb_abortdefuse", who));
            }
            E::Defused { who } => {
                out.write(GameEvent::of("bomb_defused", who));
                log.write(LogLine::triggered(who, "Defused_The_Bomb"));
            }
            E::Exploded { .. } => {
                out.write(GameEvent::new("bomb_exploded"));
            }
            E::HostageFollows { hostage, leader, .. } => {
                out.write(GameEvent::of("hostage_follows", leader).with("hostage", hostage_id(hostage)));
                log.write(LogLine::triggered(leader, "Touched_A_Hostage"));
            }
            E::HostageStops { hostage, leader } => {
                out.write(GameEvent::of("hostage_stops_following", leader).with("hostage", hostage_id(hostage)));
            }
            E::HostageHurt { hostage, attacker, .. } => {
                if let Some(a) = attacker {
                    out.write(GameEvent::of("hostage_hurt", a).with("hostage", hostage_id(hostage)));
                }
            }
            E::HostageKilled { hostage, attacker } => {
                if let Some(a) = attacker {
                    out.write(GameEvent::of("hostage_killed", a).with("hostage", hostage_id(hostage)));
                    log.write(LogLine::triggered(a, "Killed_A_Hostage"));
                }
            }
            E::HostageRescued { hostage, leader } => {
                let event = match leader {
                    Some(l) => GameEvent::of("hostage_rescued", l),
                    None => GameEvent::new("hostage_rescued"),
                };
                out.write(event.with("hostage", hostage_id(hostage)));
                if let Some(l) = leader {
                    log.write(LogLine::triggered(l, "Rescued_A_Hostage"));
                }
                rescued = true;
            }
            _ => {}
        }
    }
    if rescued && tally.is_some_and(|t| t.all_rescued()) {
        out.write(GameEvent::new("hostage_rescued_all"));
    }
}

fn hostage_id(e: Entity) -> EventValue {
    EventValue::Int(e.index().index() as i64)
}

/// The log's trigger for a round's end.
pub fn round_trigger(reason: RoundEndReason, winner: Option<Team>) -> &'static str {
    match reason {
        RoundEndReason::Draw => "Round_Draw",
        RoundEndReason::TargetBombed => "Target_Bombed",
        RoundEndReason::BombDefused => "Bomb_Defused",
        RoundEndReason::TargetSaved => "Target_Saved",
        RoundEndReason::HostagesRescued => "All_Hostages_Rescued",
        RoundEndReason::HostagesNotRescued => "Hostages_Not_Rescued",
        RoundEndReason::Eliminated | RoundEndReason::TimeRanOut => match winner {
            Some(ATTACKERS) => "Terrorists_Win",
            Some(DEFENDERS) => "CTs_Win",
            _ => "Round_Draw",
        },
    }
}

fn round_events(
    mut ended: MessageReader<RoundEnded>,
    restarts: Option<Res<RoundRestarts>>,
    mut out: MessageWriter<GameEvent>,
    mut log: MessageWriter<LogLine>,
) {
    if restarts.as_ref().is_some_and(|r| r.is_changed() && !r.is_added()) {
        out.write(GameEvent::new("round_start"));
    }
    for end in ended.read() {
        let trigger = round_trigger(end.reason, end.winner);
        out.write(
            GameEvent::new("round_end")
                .with("winner", EventValue::Int(source_team(end.winner)))
                .with("message", EventValue::Str(format!("#{trigger}"))),
        );
        log.write(match end.winner {
            Some(team) if trigger != "Round_Draw" => LogLine::team(team, trigger),
            _ => LogLine::world(trigger),
        });
        log.write(LogLine::world("Round_End"));
    }
}

fn weapon_events(
    mut events: MessageReader<WeaponEvent>,
    mut picked: MessageReader<PickedUp>,
    mut detonated: MessageReader<Detonated>,
    weapons: Query<&Weapon>,
    mut out: MessageWriter<GameEvent>,
) {
    let name = |w: Entity| weapons.get(w).map_or("", |w| short_weapon(w.id)).to_string();
    for e in events.read().filter(|e| !e.replay) {
        match &e.kind {
            WeaponEventKind::Fired { .. } => {
                out.write(GameEvent::of("weapon_fire", e.owner).with("weapon", EventValue::Str(name(e.weapon))));
            }
            WeaponEventKind::Shot { to, normal: Some(_), .. } => {
                out.write(xyz(GameEvent::of("bullet_impact", e.owner), *to, true));
            }
            WeaponEventKind::ReloadStarted => {
                out.write(GameEvent::of("weapon_reload", e.owner));
            }
            _ => {}
        }
    }
    for p in picked.read() {
        out.write(GameEvent::of("item_pickup", p.who).with("item", EventValue::Str(name(p.weapon))));
    }
    for d in detonated.read() {
        let event = match d.kind {
            GrenadeKind::Blast => "hegrenade_detonate",
            GrenadeKind::Flash => "flashbang_detonate",
            GrenadeKind::Smoke => continue,
        };
        let event = match d.thrower {
            Some(t) => GameEvent::of(event, t),
            None => GameEvent::new(event),
        };
        out.write(xyz(event, d.at, true));
    }
}

/// The weapon a hit or kill is credited to: the damage's own (a thrown
/// grenade) or what the attacker holds.
fn credited(
    weapon: Option<&'static str>,
    attacker: Option<Entity>,
    held: &Query<&Inventory>,
    weapons: &Query<&Weapon>,
) -> String {
    weapon
        .or_else(|| {
            let inv = held.get(attacker?).ok()?;
            Some(weapons.get(inv.active?).ok()?.id)
        })
        .map_or_else(|| "world".into(), |w| short_weapon(w).to_string())
}

fn player_events(
    mut hurt: MessageReader<DamageTaken>,
    mut died: MessageReader<Died>,
    mut marks: MessageReader<chimera_sim::dominations::KillMark>,
    players: Query<&Health, (With<Team>, With<Inventory>, Without<Hostage>)>,
    held: Query<&Inventory>,
    weapons: Query<&Weapon>,
    mut out: MessageWriter<GameEvent>,
    mut log: MessageWriter<LogLine>,
) {
    for t in hurt.read() {
        let d = &t.damage;
        let Ok(health) = players.get(d.target) else { continue };
        let mut event = GameEvent::of("player_hurt", d.target);
        if let Some(a) = d.attacker {
            event = event.with("attacker", player(a));
        }
        out.write(
            event
                .with("health", EventValue::Int((health.current.max(0.0) * 100.0).round() as i64))
                .with("dmg_health", EventValue::Int((d.amount * 100.0).round() as i64))
                .with("hitgroup", EventValue::Int(d.hitgroup as i64))
                .with("weapon", EventValue::Str(credited(d.weapon, d.attacker, &held, &weapons))),
        );
    }
    let marks: Vec<_> = marks.read().copied().collect();
    for d in died.read() {
        if !players.contains(d.entity) {
            continue;
        }
        // Dominations (dominations.md Behavior 3): 0/1, never both.
        let mark = marks.iter().find(|m| m.victim == d.entity && Some(m.killer) == d.attacker);
        let mut event = GameEvent::of("player_death", d.entity);
        if let Some(a) = d.attacker {
            event = event.with("attacker", player(a));
        }
        let weapon = credited(d.damage.weapon, d.attacker, &held, &weapons);
        out.write(
            event
                .with("weapon", EventValue::Str(weapon.clone()))
                .with("headshot", EventValue::Bool(d.damage.hitgroup == crate::core::Hitgroup::Head))
                .with("dominated", EventValue::Int(mark.is_some_and(|m| m.dominated) as i64))
                .with("revenge", EventValue::Int(mark.is_some_and(|m| m.revenge) as i64)),
        );
        // `"A" killed "B" with "usp"`, `"B" committed suicide with "world"`
        // (measured 2026-10-18, objectives.md Q17).
        let line = match d.attacker.filter(|a| *a != d.entity && players.contains(*a)) {
            Some(a) => LogLine(vec![
                LogPart::Player(a),
                LogPart::Text(" killed ".into()),
                LogPart::Player(d.entity),
                LogPart::Text(format!(" with \"{weapon}\"")),
            ]),
            None => LogLine(vec![
                LogPart::Player(d.entity),
                LogPart::Text(format!(" committed suicide with \"{weapon}\"")),
            ]),
        };
        log.write(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_triggers() {
        assert_eq!(short_weapon("cs_source:weapon_ak47"), "ak47");
        assert_eq!(short_weapon("weapon_c4"), "c4");
        assert_eq!(short_weapon("knife"), "knife");
        assert_eq!(round_trigger(RoundEndReason::Eliminated, Some(ATTACKERS)), "Terrorists_Win");
        assert_eq!(round_trigger(RoundEndReason::TimeRanOut, Some(DEFENDERS)), "CTs_Win");
        assert_eq!(round_trigger(RoundEndReason::TargetSaved, Some(DEFENDERS)), "Target_Saved");
        assert_eq!(round_trigger(RoundEndReason::Draw, None), "Round_Draw");
    }
}
