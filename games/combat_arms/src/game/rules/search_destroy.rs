//! Search & Destroy (`combat_arms:search_destroy`, room key `TMM`;
//! specs/combat_arms/game_modes.md M3): JSF (Alpha, `OffenceTeam`)
//! attacks with bombs, Creed (Bravo, `DefenceTeam`) defends two bomb
//! sites; rounds without respawn after a warm-up; G round wins take the
//! match.
//!
//! - Bombs: at each real round's setup every living team-1 player gets
//!   the mission's objective weapons (`[Object0]` `JSFWeapon` and
//!   `CreedWeapon`, both to team 1: missions.md Quirk 3; `XK-2`, the
//!   mission bomb `objectives::BOMB`, class 6), once its loadout is there
//!   and without drawing it (Open question 33); never dropped on death;
//!   leftover bombs go at the round's end. No bombs in the warm-up (S9).
//! - Sites: the map's `BombZoneTrigger`s of `GameType` `Doobic` (`bomb`:
//!   plant, defuse, fuse).
//! - Deaths (S7's S&D column): enemy kill, killer kills +1 and victim
//!   deaths +1; team kill, killer friendly kills +1 (no death, Quirk 3);
//!   own damage, victim deaths and suicides +1; no killer, victim deaths
//!   +1. Assists as S7.
//! - Offence and defence are the mission's `OffenceTeam` / `DefenceTeam`
//!   (`missions::sides`; JSF and Creed in every stock file).
//! - Round win by elimination: a death leaving Creed with nobody alive
//!   gives the round to JSF, even with the bomb planted; one leaving JSF
//!   with nobody alive gives it to Creed only if no bomb is ticking; one
//!   leaving both empty to the victim's enemy team. A defuse wins for
//!   Creed, the detonation for JSF.
//! - Time-out (`TimeLimit` 180 s, a plant adds the fuse): the defence
//!   wins. The detonation hurts nobody (`bomb`).
//! - Match end, after each real round: a team has G round wins, or a team
//!   is empty; the winner more round wins, else Bravo (Quirk 4, test case
//!   42).
//! - Leaving: a living leaver who was the last of its team alive gives the
//!   round to the other team, also in the warm-up (which it ends early,
//!   Corrections 2026-10-22, 2). Joiners are held while a bomb ticks, from
//!   the joining count (S1).
//! - The planter is credited the detonation also when the round is decided
//!   with the bomb ticking, and S&D keeps no plant count (`bomb`).
//! - Spawns: the `Doobic` points per team. Statistics: plants, detonations
//!   and defuses.
//!
//! Sides never swap (Open question 33: only the clan variants and VIP
//! Escort do).
//!
//! Known shortcuts: the bomb's hand-out waits for the loadout (the tick
//! after the setup's respawn) and comes after the freeze starts (the
//! mission's start triggers aren't run).

use bevy::prelude::*;

use super::{
    super::missions,
    CaMode, CaRuleSettings, Hooks, Leaver, NO_HOOKS, Panel, RoundTime, SEARCH_DESTROY, ScoreOrder, Spawns, Teams,
    bomb,
    flow::alive,
    objectives,
    scoring::round_mode,
    state::{ALPHA, BRAVO, CaMatch, Death, Phase, enemy},
};

/// The bomb sites' and points' game type.
pub const FAMILY: &str = "Doobic";
/// The attacking and defending teams (`OffenceTeam` JSF, `DefenceTeam`
/// Creed in every stock file).
pub const OFFENCE: u8 = ALPHA;
pub const DEFENCE: u8 = BRAVO;

/// The mode.
pub const MODE: CaMode = CaMode {
    rules: super::Mode {
        id: SEARCH_DESTROY,
        key: "TMM",
        shown_as: "Search & Destroy",
        description: "Search & Destroy: JSF plants a bomb at one of two sites, Creed defends; rounds without respawn after a \
                      warm-up, G round wins take the match",
        respawns: false,
        warmup: true,
        warmup_revives: true,
        forced_setup: None,
        teams: Teams::Two,
        on_death,
        time_out: |w| w.resource_mut::<CaMatch>().winner = Some(missions::sides(w).1),
        match_end,
        hooks: HOOKS,
    },
    ff_scaled: true,
    assists: true,
    timer_shown: true,
    spawns: Spawns {
        family: Some(FAMILY),
        one_list: false,
        shuffled: false,
        locked_rounds: false,
    },
    round_time: RoundTime::Mission,
    fire_ends_protection: false,
    death_drops: true,
    panel: Panel::RoundWins,
    order: ScoreOrder::ExplodeDefuse,
    leaving,
};

/// S&D's hooks (Go4CA's too).
pub const HOOKS: Hooks = Hooks {
    setup,
    round_over: bomb::credit_planter,
    round_end: bomb::remove_all,
    holds_joiner: bomb::ticking,
    warmup_end: match_end,
    ..NO_HOOKS
};

/// A player waiting for the bomb (its loadout comes first).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CaBombDue;

/// A real round's setup: every living team-1 player is due the
/// objective weapons (Quirk 14: whichever team the mission names).
fn setup(world: &mut World) {
    world.resource_mut::<bomb::CaPlanter>().0 = None;
    if world.resource::<CaMatch>().warmup {
        return;
    }
    for p in objectives::takers(world) {
        if p.team == ALPHA {
            world.entity_mut(p.e).insert(CaBombDue);
        }
    }
}

/// The objective weapons of the mission's `[Object0]`: `JSFWeapon`'s,
/// then `CreedWeapon`'s (missions.md Quirk 3: both go to team 1); our
/// bomb when the mission names none we have.
pub fn handed_out(world: &World) -> Vec<&'static str> {
    let mut ids = missions::objective_weapons(world, missions::JSF);
    ids.extend(missions::objective_weapons(world, missions::CREED));
    ids.dedup();
    ids
}

/// The modes playing S&D's per-tick systems (`hand_out`, `bombs`):
/// S&D, Go4CA's and the clans'.
pub const RUNS_IN: &[&str] = &[
    super::SEARCH_DESTROY,
    super::GO4CA_CUP,
    super::CLAN_SEARCH_DESTROY,
];

/// The bombs handed out once a due player's loadout is there (not
/// drawn: the player keeps the primary in hand); building weapons is
/// world work, queued (`rules::mode_updates`, before `bombs`).
pub(super) fn hand_out(due: Query<(Entity, Option<&crate::weapon::Inventory>), With<CaBombDue>>, mut commands: Commands) {
    for (e, inv) in &due {
        if inv.is_some_and(|i| !i.weapons.is_empty()) {
            commands.queue(move |world: &mut World| {
                for id in handed_out(world) {
                    objectives::give_weapon(world, e, id);
                }
                world.entity_mut(e).remove::<CaBombDue>();
            });
        }
    }
}

/// Plants, defuses and the fuse on S&D's sites.
pub(super) fn bombs(mut bombs: bomb::Bombs) {
    bombs.tick(FAMILY);
}

fn on_death(world: &mut World, d: &Death) {
    round_mode(world, d);
    if world.resource::<CaMatch>().winner.is_some() {
        return;
    }
    let (offence, defence) = missions::sides(world);
    let (a, b) = (alive(world, offence), alive(world, defence));
    let winner = match (a, b) {
        (0, 0) => Some(enemy(d.victim_team)),
        (_, 0) => Some(offence),
        (0, _) if !bomb::ticking(world) => Some(defence),
        _ => None,
    };
    if let Some(w) = winner {
        world.resource_mut::<CaMatch>().win(w);
    }
}

/// A living leaver, the last of its team alive, gives the round to the
/// other team (Corrections 2026-10-22, 2: also in the warm-up, which it
/// ends early).
pub fn leaving(m: &mut CaMatch, l: &Leaver) {
    if m.phase == Phase::Live && m.winner.is_none() && l.alive && l.team_alive <= 1 {
        m.win(enemy(l.team));
    }
}

/// G round wins or an empty team; more round wins, else Bravo.
pub fn match_end(world: &World) -> Option<Option<u8>> {
    let goal = world.resource::<CaRuleSettings>().goal();
    let m = world.resource::<CaMatch>();
    let counts = world.resource::<super::CaTeamCounts>().0;
    if !counts.contains(&0) && m.round_wins.iter().all(|w| *w < goal) {
        return None;
    }
    Some(Some(if m.round_wins[0] > m.round_wins[1] { ALPHA } else { BRAVO }))
}
