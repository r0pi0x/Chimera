//! Dominations and revenge (specs/cs_source/dominations.md; measured in
//! CS:S): per ordered pair of players, the killer's kills of the victim
//! since the victim last killed the killer ("unanswered kills"). The
//! kill that makes it exactly 4 starts a domination (the killer dominates
//! the victim, the victim's nemesis is the killer); the dominated player's
//! kill of their dominator is a revenge and ends it.
//!
//! - **Counting** (`count_kills`, `Dominations::kill`, spec Behavior 1): the
//!   flags first (domination when the count reaches exactly 4 between
//!   enemies; else revenge when the victim dominated the killer, between
//!   enemies), then the counters (the killer's count on the victim up, the
//!   victim's on the killer to 0). Team kills update the counters but never
//!   flag; suicides and deaths with no credited player change nothing.
//!   Players are enemies when their `Team`s differ or either has none (a
//!   free-for-all).
//! - **What it tells** (`KillMark`, per kill with a flag, beside `Died`):
//!   `player_death`'s `dominated` and `revenge` (0/1) in a game's events;
//!   a network server sends it with the kill (`net::Killed`), a client
//!   writes it again. Each player's `Relationships` (whom it dominates,
//!   who dominates it) on the server, sent only to that player's own
//!   client (the network's own-state part `chimera.relationships`).
//! - **Lifetime** (spec Behavior 2): kept over rounds and game restarts;
//!   a game clears a player's (`Dominations::clear`) on a team change that
//!   kills them, and everything on a new map (`Dominations::reset`); a
//!   player who leaves (despawned) takes theirs along (`prune`).
//!
//! The counting runs where a game adds `count_kills` (its rules' run
//! conditions; `DominationSet`, before the network sends kills and a
//! game's events read the marks); `DominationsPlugin` (in
//! `SimBasePlugins`) only registers the parts and keeps `Relationships`
//! in step.
//!
//! Known shortcuts (docs/tech-debt.md): kills by a player who has since
//! left are not counted (their entity is gone); CS:S's team balance moves
//! players by its team change, so it clears theirs (which kind of switch
//! the game's balance makes is spec Q3, open); no kill feed icon on a
//! dominating or revenge kill (spec Q4 open: `hud/freeze_dominated` is a
//! guess, not drawn). Why it's OK for now: the measured rules are in; the
//! rest is what the spec leaves open.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;

use crate::core::{Died, Team};

/// Unanswered kills that start a domination (measured, spec Constants).
pub const DOMINATION_KILLS: u32 = 4;

/// The types, the ordering set and `Relationships` kept in step.
pub struct DominationsPlugin;

impl Plugin for DominationsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dominations>()
            .add_message::<Died>()
            .add_message::<KillMark>()
            .configure_sets(PostUpdate, DominationSet.before(chimera_core::announcer::AnnounceSet))
            .add_systems(
                PostUpdate,
                (prune, show_relationships)
                    .chain()
                    .after(DominationSet)
                    .run_if(crate::core::authoritative),
            );
    }
}

/// Where a game's `count_kills` runs (PostUpdate, before the network
/// sends the kills and a game's events read `KillMark`s).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DominationSet;

/// A kill's domination flags (spec Behavior 3: `player_death`'s
/// `dominated` and `revenge`), written only for a kill with one; never
/// both.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct KillMark {
    pub victim: Entity,
    pub killer: Entity,
    /// The kill started a domination.
    pub dominated: bool,
    /// The kill was a revenge (ended the victim's domination of the killer).
    pub revenge: bool,
}

/// A player's relationships (spec Behavior 3's two flag arrays): whom it
/// dominates and who dominates it (its nemeses). On the server for every
/// player with one; a network client has only its own player's.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct Relationships {
    pub dominating: Vec<Entity>,
    pub nemeses: Vec<Entity>,
}

/// The server's counters and relationships.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Dominations {
    /// (victim, killer): the killer's kills of the victim since the victim
    /// last killed the killer.
    unanswered: BTreeMap<(Entity, Entity), u32>,
    /// (dominator, dominated).
    dominating: BTreeSet<(Entity, Entity)>,
}

impl Dominations {
    /// The killer's unanswered kills of the victim.
    pub fn unanswered(&self, victim: Entity, killer: Entity) -> u32 {
        self.unanswered.get(&(victim, killer)).copied().unwrap_or(0)
    }

    /// Whether `a` dominates `b`.
    pub fn dominates(&self, a: Entity, b: Entity) -> bool {
        self.dominating.contains(&(a, b))
    }

    /// `killer` killed `victim` (not itself), `enemies` when on different
    /// teams: the flags (dominated, revenge), the relationships and the
    /// counters as spec Behavior 1 orders them.
    pub fn kill(&mut self, killer: Entity, victim: Entity, enemies: bool) -> (bool, bool) {
        if killer == victim {
            return (false, false);
        }
        let n = self.unanswered(victim, killer) + 1;
        let mut flags = (false, false);
        if n == DOMINATION_KILLS && enemies {
            self.dominating.insert((killer, victim));
            flags.0 = true;
        } else if enemies && self.dominating.remove(&(victim, killer)) {
            flags.1 = true;
        }
        self.unanswered.insert((victim, killer), n);
        self.unanswered.remove(&(killer, victim));
        flags
    }

    /// Everything of `player`'s, both ways (a team change that kills them,
    /// leaving).
    pub fn clear(&mut self, player: Entity) {
        self.unanswered.retain(|(a, b), _| *a != player && *b != player);
        self.dominating.retain(|(a, b)| *a != player && *b != player);
    }

    /// Only what is between players `keep` says are still there.
    pub fn retain(&mut self, keep: impl Fn(Entity) -> bool) {
        self.unanswered.retain(|(a, b), _| keep(*a) && keep(*b));
        self.dominating.retain(|(a, b)| keep(*a) && keep(*b));
    }

    /// All of it (a new map).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// `player`'s relationships.
    pub fn of(&self, player: Entity) -> Relationships {
        let mut r = Relationships::default();
        for &(a, b) in &self.dominating {
            if a == player {
                r.dominating.push(b);
            }
            if b == player {
                r.nemeses.push(a);
            }
        }
        r
    }

    fn players(&self) -> BTreeSet<Entity> {
        self.dominating.iter().flat_map(|&(a, b)| [a, b]).collect()
    }
}

/// Players are enemies when their teams differ or either has none.
pub fn enemies(a: Option<&Team>, b: Option<&Team>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a != b,
        _ => true,
    }
}

/// Each death by another player's hand: the counters, and a `KillMark`
/// for a kill with a flag. A game adds it (in `DominationSet`) under its
/// rules' run conditions.
pub fn count_kills(
    mut died: MessageReader<Died>,
    teams: Query<Option<&Team>, With<crate::weapon::Inventory>>,
    mut table: ResMut<Dominations>,
    mut out: MessageWriter<KillMark>,
) {
    for d in died.read() {
        let Some(killer) = d.attacker.filter(|a| *a != d.entity) else { continue };
        // Only players (characters that carry weapons; not a hostage).
        let (Ok(kt), Ok(vt)) = (teams.get(killer), teams.get(d.entity)) else {
            continue;
        };
        let (dominated, revenge) = table.kill(killer, d.entity, enemies(kt, vt));
        if dominated || revenge {
            out.write(KillMark {
                victim: d.entity,
                killer,
                dominated,
                revenge,
            });
        }
    }
}

/// Players who left take their counters along (spec Behavior 2: a
/// disconnect clears both directions; a slot's next player starts clean).
fn prune(mut table: ResMut<Dominations>, alive: Query<()>) {
    if table.unanswered.keys().any(|(a, b)| !alive.contains(*a) || !alive.contains(*b))
        || table.dominating.iter().any(|(a, b)| !alive.contains(*a) || !alive.contains(*b))
    {
        table.retain(|e| alive.contains(e));
    }
}

/// `Relationships` on each player as the table has them (none: off).
fn show_relationships(
    table: Res<Dominations>,
    shown: Query<(Entity, &Relationships)>,
    mut commands: Commands,
) {
    if !table.is_changed() {
        return;
    }
    let players = table.players();
    for (e, r) in &shown {
        if !players.contains(&e) {
            commands.entity(e).remove::<Relationships>();
        } else if *r != table.of(e) {
            commands.entity(e).insert(table.of(e));
        }
    }
    for e in players {
        if !shown.contains(e)
            && let Ok(mut ent) = commands.get_entity(e)
        {
            ent.insert(table.of(e));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> (Entity, Entity) {
        (Entity::from_raw_u32(1).unwrap(), Entity::from_raw_u32(2).unwrap())
    }

    #[test]
    fn the_fourth_unanswered_kill_dominates_and_only_the_fourth() {
        let (a, b) = pair();
        let mut t = Dominations::default();
        for _ in 0..3 {
            assert_eq!(t.kill(a, b, true), (false, false));
        }
        assert_eq!(t.kill(a, b, true), (true, false));
        assert!(t.dominates(a, b));
        assert_eq!(t.kill(a, b, true), (false, false), "the 5th");
        assert_eq!(t.kill(a, b, true), (false, false), "the 6th");
        // Revenge ends it; counts restart.
        assert_eq!(t.kill(b, a, true), (false, true));
        assert!(!t.dominates(a, b));
        assert_eq!((t.unanswered(a, b), t.unanswered(b, a)), (1, 0));
        // Dominated again after 4 more.
        for _ in 0..3 {
            assert_eq!(t.kill(a, b, true), (false, false));
        }
        assert_eq!(t.kill(a, b, true), (true, false));
    }

    #[test]
    fn an_answer_restarts_the_count() {
        let (a, b) = pair();
        let mut t = Dominations::default();
        for _ in 0..3 {
            t.kill(a, b, true);
        }
        t.kill(b, a, true);
        for _ in 0..3 {
            assert_eq!(t.kill(a, b, true), (false, false));
        }
        assert!(!t.dominates(a, b));
    }

    #[test]
    fn team_kills_count_but_never_flag() {
        let (a, b) = pair();
        let mut t = Dominations::default();
        for _ in 0..4 {
            assert_eq!(t.kill(a, b, false), (false, false));
        }
        assert!(!t.dominates(a, b));
        // The 4th was spent: as enemies the 5th doesn't dominate.
        assert_eq!(t.kill(a, b, true), (false, false));
        let mut t = Dominations::default();
        for _ in 0..3 {
            t.kill(a, b, false);
        }
        assert_eq!(t.kill(a, b, true), (true, false), "3 team kills + 1");
        // A dominated teammate's kill is no revenge and ends nothing.
        assert_eq!(t.kill(b, a, false), (false, false));
        assert!(t.dominates(a, b));
    }

    #[test]
    fn clearing_one_player_clears_both_ways() {
        let (a, b) = pair();
        let c = Entity::from_raw_u32(3).unwrap();
        let mut t = Dominations::default();
        for _ in 0..4 {
            t.kill(a, b, true);
        }
        for _ in 0..2 {
            t.kill(c, a, true);
        }
        t.clear(b);
        assert!(!t.dominates(a, b));
        assert_eq!(t.unanswered(b, a), 0);
        assert_eq!(t.unanswered(a, c), 2, "others' counts stay");
        assert_eq!(t.of(a), Relationships::default());
    }
}
