//! `rung`, the developer command that puts a player on a chosen rung of the
//! playing ladder (to try a gun without climbing to it), and `+rung ...` on
//! the command line, which waits for the match.
//!
//! - `rung <n>`: rung `n`, numbered from 1 as the HUD shows it (past the
//!   ends: the nearest). `rung <weapon id or part of it>` (`halo_ce:sniper_rifle`,
//!   `frag`): the first rung whose gun is that id, else the first of the one
//!   gun whose id holds the text; two guns that fit are refused, naming
//!   them. `rung next` / `rung prev` step from the player's rung (not past
//!   the ends). `rung list`: every rung (number, weapon id, game, band; the
//!   player's marked). A last argument names the player (their name, a
//!   part of it) or is `all`; none: the local player (on a dedicated
//!   server's console, which has none: name one or `all`).
//! - It changes a rung through `ladder::set_rung`, as a kill does (weapon,
//!   identity, body, HUD), on the server, so clients get it by replication;
//!   no message of its own.
//! - Who may: offline, a listen host and a dedicated server's console
//!   always; a remote client's `rung` is sent to the server like a game's
//!   player command (`Console::player_command`) and acts on that client's
//!   player alone, when the server's `sv_cheats` is 1 (the server checks it
//!   here: this is a cheat for a client, not for the host).
//! - `+rung <...>` runs at startup, before any match: with no ladder or
//!   player yet the line is kept (`PendingRung`) and applied by a system
//!   the first frame the local player has a rung (an error then goes to the
//!   console); a later `rung` replaces it.
//!
//! - **The sandbox's rule** (`HoldRungs`): an app that plays a sandbox
//!   minigame (Ultimate Gun Game's `sandbox`) names its id, and while it
//!   plays a player stays on the rung they were put on (`rung`, `ladder_rung`):
//!   a kill, a death or a melee demotion that moves them is undone by
//!   `hold` (`ladder.rs` has no option for it, and is not this file's to
//!   change). The ladder's last rung is then a sentinel (a copy of the one
//!   before it, so a kill on the last real rung climbs to it instead of
//!   winning the match): it is never shown, listed or chosen.
//!
//! Known shortcuts: the sandbox's kill still moves the player for the frame
//! before `hold` puts them back (the same gun when it is the last rung's, a
//! swap of the arming otherwise); a `+rung` on a network client is lost (the line is sent
//! to a server not yet joined); `rung list` marks only the target's rung.

use bevy::prelude::*;

use super::{
    ladder::{self, Ladder, Rung},
    runtime,
};
use crate::console::{Cheats, CommandPlayer, Console, ConsoleAppExt, Level, command_player};

/// The rung a player was put on while `HoldRungs` holds them there (0-based).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chosen(pub usize);

/// The sandbox's rule: while the minigame with this id plays, a player stays
/// on the rung they were put on, and the ladder's last rung is a sentinel
/// (module doc). Inserted by the app that has the sandbox.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HoldRungs(pub &'static str);

/// Whether the sandbox's rule is in force now.
fn holding(world: &World) -> bool {
    let (Some(h), Some(a)) = (world.get_resource::<HoldRungs>(), world.get_resource::<super::ActiveMinigame>()) else {
        return false;
    };
    a.def.id == h.0
}

/// The rungs a player may be put on: all, but the sandbox's sentinel.
fn usable(world: &World, ladder: &Ladder) -> usize {
    ladder.rungs.len().saturating_sub(usize::from(holding(world)))
}

/// `e` on `rung` of `ladder` (`ladder::set_rung`), staying there while the
/// sandbox's rule holds.
pub fn move_to(world: &mut World, e: Entity, rung: usize, ladder: &Ladder) {
    ladder::set_rung(world, e, rung, ladder);
    if holding(world) {
        world.entity_mut(e).insert(Chosen(rung));
    }
}

/// The command's name (full).
const NAME: &str = "chimera:rung";

/// A `rung` line that arrived before the match did (`+rung`): its arguments.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct PendingRung(pub Vec<String>);

pub(super) fn plugin(app: &mut App) {
    app.console_command(
        NAME,
        "rung <n | weapon id or part | next | prev | list> [player | all]: put a player (default: you) on that \
         rung of the ladder, from 1, as a kill would (a developer command; `+rung <id>` at launch waits for the \
         match; a client needs the server's sv_cheats 1).",
        |w, a| run(w, a, true),
    );
    app.world_mut().resource_mut::<Console>().player_command(NAME);
    app.add_systems(Update, (apply_pending.run_if(resource_exists::<PendingRung>), hold));
}

/// What a selector asks for.
#[derive(Debug, PartialEq, Eq)]
enum Pick {
    Number(usize),
    Weapon(String),
    Next,
    Prev,
}

fn parse_pick(text: &str) -> Pick {
    match text.to_lowercase().as_str() {
        "next" => Pick::Next,
        "prev" | "previous" => Pick::Prev,
        t => t.parse::<usize>().map_or_else(|_| Pick::Weapon(t.to_string()), Pick::Number),
    }
}

/// The 0-based rung a number or weapon text names (`n` from 1).
fn resolve(ladder: &Ladder, usable: usize, pick: &Pick) -> Result<usize, String> {
    let last = usable.checked_sub(1).ok_or("the ladder has no rungs")?;
    match pick {
        Pick::Number(0) => Err("rungs are numbered from 1 (`rung list`)".into()),
        Pick::Number(n) => Ok((n - 1).min(last)),
        Pick::Weapon(t) => {
            if let Some(i) = ladder.rungs[..usable].iter().position(|r| r.eq_ignore_ascii_case(t)) {
                return Ok(i);
            }
            let mut found: Vec<usize> = Vec::new();
            for (i, r) in ladder.rungs[..usable].iter().enumerate() {
                if r.to_lowercase().contains(t) && !found.iter().any(|f| ladder.rungs[*f] == *r) {
                    found.push(i);
                }
            }
            match found.as_slice() {
                [] => Err(format!("no rung's gun matches {t:?} (`rung list`)")),
                [i] => Ok(*i),
                many => Err(format!(
                    "{t:?} fits several guns: {} (name one fully)",
                    many.iter().map(|i| ladder.rungs[*i]).collect::<Vec<_>>().join(", ")
                )),
            }
        }
        Pick::Next | Pick::Prev => Err("next and prev step from a player's rung".into()),
    }
}

/// `rung list`'s text; `mark` is the 0-based rung to mark.
fn list(ladder: &Ladder, usable: usize, mark: Option<usize>) -> String {
    let guns = ladder.rungs.len().saturating_sub(ladder.melee_rungs);
    let mut out = format!("{usable} rungs:");
    for (i, id) in ladder.rungs[..usable].iter().enumerate() {
        let game = id.split(':').next().unwrap_or("?");
        let band = if i >= guns { "melee" } else { "gun" };
        let here = if mark == Some(i) { "  <- you" } else { "" };
        out.push_str(&format!("\n{:>3}  {id}  {game}  {band}{here}", i + 1));
    }
    out
}

/// Whether the command may run now with these arguments; the entities it
/// acts on, or an error.
fn targets(world: &mut World, a: &[String], remote: bool) -> Result<Vec<Entity>, String> {
    match a.get(1).map(|t| t.to_lowercase()) {
        None => command_player(world).map(|e| vec![e]).map_err(|_| {
            "no local player here: name a player or `all` (`rung <n> all`)".to_string()
        }),
        Some(_) if remote => Err("a client's rung command acts on its own player alone".into()),
        Some(t) if t == "all" => {
            let all: Vec<Entity> = runtime::players(world).into_iter().map(|(e, _)| e).collect();
            if all.is_empty() { Err("no players in the match".into()) } else { Ok(all) }
        }
        Some(t) => {
            let mut found = Vec::new();
            for (e, _) in runtime::players(world) {
                let name = world.get::<Name>(e).map(|n| n.as_str().to_lowercase()).unwrap_or_default();
                if name == t {
                    return Ok(vec![e]);
                }
                if name.contains(&t) {
                    found.push(e);
                }
            }
            match found.as_slice() {
                [e] => Ok(vec![*e]),
                [] => Err(format!("no player named {t:?}")),
                _ => Err(format!("several players match {t:?}")),
            }
        }
    }
}

/// Run `rung a...`; `may_wait`: a command line (not the pending one) may
/// keep itself for the match.
fn run(world: &mut World, a: &[String], may_wait: bool) -> Result<Option<String>, String> {
    let first = a.first().ok_or("rung <n | weapon id | next | prev | list> [player | all]")?;
    let pick = if first.eq_ignore_ascii_case("list") { None } else { Some(parse_pick(first)) };
    let remote = world.contains_resource::<CommandPlayer>();
    // Not ready: the match (or the player) isn't there yet.
    let ready = world.contains_resource::<Ladder>()
        && (a.len() > 1 || command_player(world).is_ok_and(|e| world.get::<Rung>(e).is_some()));
    if !ready && may_wait && !remote && pick.is_some() && a.len() == 1 {
        world.insert_resource(PendingRung(a.to_vec()));
        return Ok(Some("rung: waiting for the match and your player".into()));
    }
    let ladder = world.get_resource::<Ladder>().cloned().ok_or("no ladder plays")?;
    let Some(pick) = pick else {
        let mark = command_player(world).ok().and_then(|e| world.get::<Rung>(e)).map(|r| r.0);
        return Ok(Some(list(&ladder, usable(world, &ladder), mark)));
    };
    if remote && world.get_resource::<Cheats>().is_none_or(|c| c.0 == 0) {
        return Err("Can't use cheat command rung in multiplayer, unless the server has sv_cheats set to 1.".into());
    }
    let who = targets(world, a, remote)?;
    let n = usable(world, &ladder);
    let last = n.saturating_sub(1);
    let mut said = Vec::new();
    // Resolve every target first, so a refusal changes nothing.
    let mut moves = Vec::new();
    for e in who {
        let now = world.get::<Rung>(e).map_or(0, |r| r.0);
        let to = match pick {
            Pick::Next => (now + 1).min(last),
            Pick::Prev => now.saturating_sub(1),
            _ => resolve(&ladder, n, &pick)?,
        };
        moves.push((e, to));
    }
    for (e, to) in moves {
        move_to(world, e, to, &ladder);
        let name = world.get::<Name>(e).map_or_else(|| e.to_string(), |n| n.as_str().to_string());
        said.push(format!("{name}: rung {} of {}: {}", to + 1, last + 1, ladder.rungs[to]));
    }
    Ok(Some(said.join("\n")))
}

/// Apply a waiting `+rung` once the local player has a rung.
fn apply_pending(world: &mut World) {
    let ready = world.contains_resource::<Ladder>()
        && command_player(world).is_ok_and(|e| world.get::<Rung>(e).is_some());
    if !ready {
        return;
    }
    let Some(PendingRung(args)) = world.remove_resource::<PendingRung>() else { return };
    let (level, text) = match run(world, &args, false) {
        Ok(text) => (Level::Info, text.unwrap_or_default()),
        Err(e) => (Level::Error, format!("rung {}: {e}", args.join(" "))),
    };
    world.resource_mut::<Console>().print(level, text);
}

/// The sandbox's rule: every player back on the rung they were put on.
fn hold(world: &mut World) {
    if !holding(world) {
        return;
    }
    let Some(ladder) = world.get_resource::<Ladder>().cloned() else { return };
    // A new ladder (a new match) starts everyone over.
    if world.resource_ref::<Ladder>().is_changed() {
        let all: Vec<Entity> = world.query_filtered::<Entity, With<Chosen>>().iter(world).collect();
        for e in all {
            world.entity_mut(e).remove::<Chosen>();
        }
    }
    let rows: Vec<(Entity, usize, Option<usize>, bool)> = world
        .query::<(Entity, &Rung, Option<&Chosen>, Has<crate::core::Dead>)>()
        .iter(world)
        .map(|(e, r, c, d)| (e, r.0, c.map(|c| c.0), d))
        .collect();
    for (e, now, chosen, dead) in rows {
        match chosen {
            None => {
                world.entity_mut(e).insert(Chosen(now.min(ladder.rungs.len().saturating_sub(2))));
            }
            Some(c) if c != now && dead => {
                // The weapon is the next life's: only the number.
                world.entity_mut(e).insert(Rung(c));
            }
            Some(c) if c != now => ladder::set_rung(world, e, c, &ladder),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ladder(ids: &[&'static str]) -> Ladder {
        Ladder { rungs: ids.to_vec(), melee_rungs: 1, ..default() }
    }

    #[test]
    fn numbers_ids_and_fragments_resolve() {
        let l = ladder(&["a:glock", "h:frag_grenade", "h:sniper_rifle", "a:knife"]);
        assert_eq!(resolve(&l, l.rungs.len(), &Pick::Number(1)), Ok(0));
        assert_eq!(resolve(&l, l.rungs.len(), &Pick::Number(99)), Ok(3), "past the end: the last");
        assert!(resolve(&l, l.rungs.len(), &Pick::Number(0)).is_err());
        assert_eq!(resolve(&l, l.rungs.len(), &parse_pick("h:sniper_rifle")), Ok(2));
        assert_eq!(resolve(&l, l.rungs.len(), &parse_pick("FRAG")), Ok(1));
        assert!(resolve(&l, l.rungs.len(), &parse_pick("rifle_x")).is_err());
        let two = ladder(&["a:glock", "h:rifle", "h:rifle_2", "a:knife"]);
        let e = resolve(&two, two.rungs.len(), &parse_pick("rifle")).unwrap_err();
        assert!(e.contains("h:rifle") && e.contains("h:rifle_2"), "{e}");
        assert_eq!(resolve(&two, two.rungs.len(), &parse_pick("h:rifle")), Ok(1), "an exact id wins over a fragment");
    }

    #[test]
    fn the_list_numbers_from_one_and_names_the_band() {
        let text = list(&ladder(&["a:glock", "a:knife"]), 2, Some(0));
        assert!(text.contains("  1  a:glock  a  gun  <- you"), "{text}");
        assert!(text.contains("  2  a:knife  a  melee"), "{text}");
    }
}
