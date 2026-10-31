//! A minigame over the network (docs/plans/active/multiplayer.md), as a
//! part of the network (`net::NetAppExt::net_part`, registered by
//! `MinigameNetPlugin`, named `minigame`; an app that plays minigames over
//! the network adds it beside `NetPlugin`): the
//! server plays it (`chimera_sim::minigame`, its runtime and rules module
//! on the server only) and a client shows it, so the HUD it draws
//! (`minigame::hud`: the rung and next gun, the leader, the match's
//! state and time, notices) reads what it always reads.
//!
//! - **The match** (`NetMinigame`, one replicated entity): the minigame's
//!   id, the match's phase (`phase::*`) and when the round ends (a server
//!   tick). A client starts the same minigame as its gameplay when it
//!   learns the id (`minigame::join_remote`: the slots, the rules module's
//!   parameters, no restart) and keeps its `Match` in step, times on its
//!   own clock (`game::ServerTime`).
//! - **The players**: `InMinigame` and the rules module's own state
//!   (`ladder::Rung`, `wins::MatchesWon`) replicate on the characters, so the HUD finds who
//!   plays and who leads; kills and deaths are `NetScore`'s. The ladder's
//!   rung extras (`ladder::RungExtras`, drawn by the server) go with the
//!   match (`NetMinigame::rung_extras`) into the client's ladder (its HUD's
//!   rung label), as do its rungs and melee count (`rungs`,
//!   `melee_rungs`: the host's draw of the melee finale, and of the guns
//!   of a ladder drawn when the match starts, `ladder = "auto"`); what they put on a player (a game's look, fitted parts,
//!   the melee weapon) replicates as that game's state does. The client
//!   then puts each player's `ladder::OnRung` on from them
//!   (`ladder::follow_rungs`; not replicated), so the games' observers of
//!   arriving on a rung and leaving it fire there too.
//! - **Spawn protection** (`NetProtected`): a protected character's two
//!   windows (`core::SpawnProtected`) as server ticks, which a client
//!   puts on its copy on its own clock (`apply_protection`), so its
//!   shots and others' traced again show no hit on that player for the
//!   same windows the server blocks (no blood without damage).
//! - **Events** (`MinigameNews`): rung ups and downs, the last rung, the
//!   lead, a match won, for the HUD's notices and callouts, as the index
//!   in `minigame::EVENT_NAMES`.
//!
//! - **The manifest** (`net::manifest`): the server's hook adds the
//!   minigame (its id, its definition's SHA-256 and text), the games it
//!   names (wanted; `needs`) and its weapon sets; a client's hook plays the
//!   server's definition (its own if the hash matches, else the text, read
//!   and checked: `minigame::add`) or leaves saying why. A client starts a
//!   minigame only once a manifest names it. Another minigame on the
//!   server is announced (`ChangeMatch`), as is the same id made again
//!   with another definition (an automatic gun game's new ladder), which a
//!   client then plays anew.
//! - **Changing match** (`change_match`, console `change_match <id>
//!   [map]`): the server starts another minigame, players kept and
//!   connected, scores anew; only one every connected player can play
//!   there. On another map (`MapChange`) it holds the match
//!   (`minigame::MatchHold`: nothing starts or respawns), changes level
//!   through the app's `changelevel` (a standalone game's `map`; clients
//!   get `ChangingLevel` and show the loading dialog), and starts the
//!   match only once the map is in here and every client reported it
//!   loaded (`server::ClientMap`), or `CLIENT_GRACE` s after the map came
//!   in; a load that fails (or none in `MAP_LOAD_TIMEOUT` s) starts it
//!   where the server is. The manifest names the coming minigame
//!   meanwhile. A server rotates through it by itself once a match has
//!   been over for `RESULTS_HOLD` s (`serve_minigame`,
//!   `MinigameDef::rotate`: the pool's next map, or a random one but the
//!   last played, `minigame::rotation::MapHistory`, which the host keeps
//!   for its lifetime; `NextMatchHooks` let the app act first) and puts
//!   the results up meanwhile (`NetMinigame::results`, each row's
//!   character mapped to the client's entity) with the match-end hold
//!   (`minigame::NextMatch`: the map picked for the next match as soon as
//!   the match is decided, and when the change comes; `NetMinigame::next`),
//!   which the shell's match-end screen shows.
//! - **The map vote** (a definition's `map_vote`, `minigame::vote`): with
//!   one, the hold's next map isn't picked at once: the rotation's pick
//!   and a few other maps go on a ballot (`minigame::vote::MapVote`,
//!   replicated as `NetMinigame::vote` with the counts, no ballots) and
//!   the hold waits for it (`NextMatch::at` moved to its close plus the
//!   reveal); each player votes (`vote_map`, console `vote_map <n>`: a
//!   client sends `MapVoteCast`, the server counts it for that client's
//!   character; a host casts for its own player), and at the close the
//!   most votes win (a tie at random, none: the rotation's pick), which
//!   becomes the hold's next map.
//! - **The playable set** (`playable`, `PartyOffer`, console
//!   `party_offer`): each registered minigame on its pool's first map
//!   against the players' mount doctor reports (their userinfo); the
//!   minigames every player can play go to the clients when the set or
//!   the players change (a lobby's list). `installed_by_all`: the games
//!   every player has (an automatic minigame's choice of games).
//!
//! Nothing is sent while no minigame plays.

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    core::{NetRole, SimClock, SpawnProtected},
    net::{
        GREYBOX, NetAppExt, NetHooks,
        game::{ServerTime, tick_of},
        manifest::{self, CANT_PLAY, GameNeed, MatchManifest, MinigameOffer, ServerManifest},
        maps::sha256,
        ordered::OrderedAppExt,
    },
    minigame::{self, ActiveMinigame, MatchResults, NextMatch, EVENT_NAMES, MinigameDef, MinigameEvent, Minigames, ladder::{Ladder, Rung, RungExtras}, rotation::MapHistory, runtime::InMinigame, vote::{self, MapVote, MapVoteSettings}, wins::MatchesWon},
    mode::{Match, Phase},
};
use chimera_core::game_registry::Games;

/// `NetMinigame::phase`.
pub mod phase {
    /// Waiting for players, or about to start.
    pub const WAITING: u8 = 0;
    /// Live: kills count, the round's timer runs.
    pub const LIVE: u8 = 1;
    /// A round or the match is over.
    pub const OVER: u8 = 2;
}

/// The playing minigame and its match, on one replicated entity the server
/// keeps (`write_minigame`). Times are server ticks (fractional), which a
/// client turns into its own clock (`game::ServerTime`).
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetMinigame {
    /// The minigame's id (`originals:gun_game_dust2`).
    pub id: String,
    /// `phase::*`.
    pub phase: u8,
    /// Live: when the round's timer reaches 0.
    pub ends: Option<f64>,
    /// The finished match's results (`minigame::MatchResults`), names as
    /// the players are known on the network; none while the match plays.
    #[entities]
    pub results: Option<MatchResults>,
    /// The match-end hold (`minigame::NextMatch`): the next match's map
    /// once picked, and when it comes (a server tick).
    pub next: Option<(Option<String>, f64)>,
    /// The ladder's rung extras (`ladder::Ladder::extras`: what each rung
    /// gives everyone on it), which a client's ladder takes.
    pub rung_extras: Vec<Option<RungExtras>>,
    /// The ladder's rungs' weapons (`ladder::Ladder::rungs`): the melee
    /// rungs' are the host's draw (a drawn ladder's guns too), which a
    /// client's ladder takes.
    pub rungs: Vec<String>,
    /// How many of `rungs` are the melee finale's (`Ladder::melee_rungs`).
    pub melee_rungs: usize,
    /// The map vote on the match-end screen (`minigame::vote::MapVote`),
    /// while there is one.
    pub vote: Option<NetMapVote>,
}

/// `minigame::vote::MapVote` on the network: the ballot, the counts, when
/// it closes (a server tick) and, once closed, the winner.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetMapVote {
    pub candidates: Vec<String>,
    pub counts: Vec<u32>,
    pub closes: f64,
    pub winner: Option<usize>,
}

/// Client -> server: the client's player votes for the vote's candidate
/// `choice` (from 0).
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct MapVoteCast {
    pub choice: u8,
}

/// A character's spawn protection (`core::SpawnProtected`), its windows'
/// ends as server ticks (fractional); gone when it ends.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct NetProtected {
    pub damage_until: f64,
    pub bullets_until: f64,
}

/// Server -> clients: something happened in the match
/// (`minigame::MinigameEvent`): its name's place in `EVENT_NAMES`, who it
/// concerns (a placeholder for no one) and its number.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct MinigameNews {
    pub name: u8,
    #[entities]
    pub player: Option<Entity>,
    pub value: i32,
}

/// The minigame's part of the network (`register`).
pub struct MinigameNetPlugin;

impl Plugin for MinigameNetPlugin {
    fn build(&self, app: &mut App) {
        app.net_part("minigame", register);
    }
}

/// The protocol (in this order on every side) and the systems.
fn register(app: &mut App) {
    // The events exist in an app without a minigame runtime too.
    app.add_message::<MinigameEvent>();
    // While a minigame is the rules, a predicting client is held as the
    // match's end holds everyone (`runtime::end_frozen`: the look only),
    // and a joiner comes in at a free spawn point (`JoinRules`'
    // default).
    chimera_core::gameplay::GameplayAppExt::game_hook(
        app,
        minigame::MINIGAME,
        crate::net::Holds {
            freeze_ends: |_| None,
            frozen: minigame::runtime::end_frozen,
        },
    );
    app.replicate::<NetMinigame>()
        .ordered::<NetMinigame>()
        .replicate::<InMinigame>()
        .replicate::<Rung>()
        .replicate::<MatchesWon>()
        .replicate::<NetProtected>()
        .ordered::<Rung>()
        .add_mapped_server_message::<MinigameNews>(Channel::Ordered)
        .add_server_message::<PartyOffer>(Channel::Ordered)
        .make_message_independent::<PartyOffer>()
        .add_client_message::<MapVoteCast>(Channel::Ordered);
    {
        let mut hooks = app.world_mut().resource_mut::<NetHooks>();
        hooks.stopped_serving.push(stopped_serving);
        hooks.manifest.push(add_to_manifest);
        hooks.accept.push(accept_minigame);
    }
    use crate::console::ConsoleAppExt;
    app.console_command(
        "change_match",
        "change_match <minigame> [map]: play another minigame, the players kept and connected, its scores \
         anew; on [map], else on this map if it is in the minigame's pool, else on the pool's first.",
        |w, a| {
            let id = a.first().ok_or("change_match <minigame> [map]")?;
            change_match(w, id, a.get(1).map(String::as_str)).map(Some)
        },
    )
    .console_command(
        "vote_map",
        "vote_map <n>: vote for the end screen's map n (from 1) while the map vote is open; alone, the vote as it \
         stands.",
        |w, a| match a.first() {
            None => Ok(Some(vote_text(w))),
            Some(n) => {
                let n: usize = n.parse().ok().filter(|n| *n >= 1).ok_or_else(|| format!("not a choice: {n}"))?;
                vote_map(w, n - 1).map(|_| None)
            }
        },
    )
    .console_command(
        "party_offer",
        "The minigames every connected player can play (the server's mount doctor reports; a client: what \
         its server offers), and on the server who lacks what for the others.",
        |w, _| Ok(Some(offer_text(w))),
    );
    let server = || resource_equals(NetRole::Server);
    let client = || resource_equals(NetRole::Client);
    app.add_systems(PreUpdate, receive_map_votes.after(ServerSystems::Receive).run_if(server()))
        .add_systems(FixedLast, serve_minigame.run_if(not(client())))
        .add_systems(
            PostUpdate,
            (write_protection, send_news, send_offer).chain().before(ServerSystems::Send).run_if(server()),
        )
        .add_systems(
            PreUpdate,
            (apply_minigame, crate::minigame::ladder::follow_rungs, apply_protection, receive_news, receive_offer)
                .chain()
                .after(ClientSystems::Receive)
                .run_if(client()),
        );
}

/// Serving stopped: the match's entity goes, and the players stop
/// replicating the minigame's marks.
fn stopped_serving(world: &mut World) {
    let entities: Vec<Entity> = world.query_filtered::<Entity, With<NetMinigame>>().iter(world).collect();
    for e in entities {
        world.despawn(e);
    }
}

/// The definition `write_minigame` last put in `NetMinigame`.
#[derive(Resource)]
struct Announced(&'static MinigameDef);

/// The minigame and the match into `NetMinigame` (gone with the minigame).
fn write_minigame(world: &mut World) {
    let have = world.query::<(Entity, &NetMinigame)>().iter(world).next().map(|(e, n)| (e, n.clone()));
    let Some(active) = world.get_resource::<ActiveMinigame>().copied() else {
        if let Some((e, _)) = have {
            world.despawn(e);
        }
        return;
    };
    let clock = *world.resource::<SimClock>();
    let m = world.get_resource::<Match>().cloned().unwrap_or_default();
    let now = NetMinigame {
        id: active.def.id.clone(),
        phase: match m.phase {
            Phase::Live => phase::LIVE,
            Phase::RoundOver { .. } | Phase::Intermission { .. } | Phase::MatchOver => phase::OVER,
            Phase::Waiting | Phase::Starting | Phase::Setup { .. } => phase::WAITING,
        },
        ends: m.round_ends.filter(|_| m.phase == Phase::Live).map(|s| tick_of(&clock, s)),
        results: world.get_resource::<MatchResults>().cloned().map(|mut r| {
            for row in &mut r.rows {
                if let Some(c) = row.player.and_then(|p| world.get::<crate::net::NetCharacter>(p)) {
                    row.name = c.name.clone();
                }
            }
            r
        }),
        next: world.get_resource::<NextMatch>().map(|n| (n.map.clone(), tick_of(&clock, n.at))),
        rung_extras: world.get_resource::<Ladder>().map(|l| l.extras.clone()).unwrap_or_default(),
        rungs: world.get_resource::<Ladder>().map(|l| l.rungs.iter().map(|r| r.to_string()).collect()).unwrap_or_default(),
        melee_rungs: world.get_resource::<Ladder>().map_or(0, |l| l.melee_rungs),
        vote: world.get_resource::<MapVote>().map(|v| NetMapVote {
            candidates: v.candidates.clone(),
            counts: v.counts.clone(),
            closes: tick_of(&clock, v.closes),
            winner: v.winner,
        }),
    };
    // Another minigame, or its definition made again (same id, new text).
    let announced = world.get_resource::<Announced>().map(|a| a.0);
    let changed = have.as_ref().is_none_or(|(_, n)| n.id != now.id)
        || announced.is_some_and(|d| !std::ptr::eq(d, active.def));
    world.insert_resource(Announced(active.def));
    match have {
        Some((e, n)) => {
            if n != now {
                world.entity_mut(e).insert(now);
            }
        }
        None => {
            world.spawn((Name::new("Minigame"), Replicated, now));
        }
    }
    // Clients hear the match's new manifest.
    if changed {
        manifest::announce(world);
    }
}

/// Server -> clients: the minigames every connected player can play
/// (`playable`), by id in the registry's order; sent when it changes or
/// players come and go. On a client, the newest heard (a lobby's list).
#[derive(Resource, Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct PartyOffer {
    pub minigames: Vec<String>,
}

/// Whether every connected player can play a minigame on a map
/// (`playable`).
#[derive(Clone, Debug, PartialEq)]
pub struct Playable {
    pub id: String,
    /// The map it would play on (`change_match`'s pick; the pool's first
    /// for the offer).
    pub map: String,
    /// Each player lacking a game it requires: the player's name and the
    /// game's id.
    pub lacking: Vec<(String, String)>,
}

impl Playable {
    pub fn ok(&self) -> bool {
        self.lacking.is_empty()
    }
}

/// The remote players' mount doctor reports (name, report), from their
/// userinfo (`manifest::MOUNTS_KEY`); a player that sent none isn't
/// listed (taken as having everything), nor are bots.
fn reports(world: &mut World) -> Vec<(String, manifest::MountReport)> {
    let mut q = world.query::<(&crate::net::NetCharacter, &chimera_core::core::UserInfo)>();
    q.iter(world)
        .filter(|(c, _)| c.owner.is_some())
        .filter_map(|(c, info)| {
            let r = manifest::from_userinfo(info.0.iter().map(|(k, v)| (k.as_str(), v.as_str())))?;
            Some((c.name.clone(), r))
        })
        .collect()
}

/// The games of `games` every player has installed: each remote player's
/// mount doctor report (one without a report has everything) and this
/// machine's own (`LocalMounts`, a listen host's or a standalone game's;
/// none: everything); a game not in `Games` (a stand-in) always. What
/// an automatic minigame is made of
/// (`chimera_sim::minigame::auto`).
pub fn installed_by_all(world: &mut World, games: &[&'static str]) -> Vec<&'static str> {
    let mut all = reports(world).into_iter().map(|(_, r)| r).collect::<Vec<_>>();
    if let Some(local) = world.get_resource::<manifest::LocalMounts>() {
        all.push(local.0.clone());
    }
    // A report lists the installed games only; a game that isn't a
    // registered one (no install to examine: a stand-in) counts as there.
    let registered = world.get_resource::<Games>().cloned();
    games
        .iter()
        .copied()
        .filter(|g| registered.as_ref().is_some_and(|r| r.get(g).is_none()) || all.iter().all(|r| r.has(g)))
        .collect()
}

/// Whether every connected player can play `def` on `map` (None: its
/// pool's first): each player's report against the games it requires
/// there (the map's game; `needs`).
pub fn playable_on(world: &mut World, def: &MinigameDef, map: Option<&str>) -> Playable {
    let map = map
        .map(crate::net::normalize_map)
        .or_else(|| def.maps.first().map(|m| crate::net::normalize_map(m)))
        .unwrap_or_else(|| GREYBOX.into());
    let mut needs = MatchManifest::default();
    if map != GREYBOX
        && let Some((game, _)) = map.split_once(':')
    {
        needs.need(game, true);
    }
    let mut lacking = Vec::new();
    for (name, report) in reports(world) {
        for n in needs.games.iter().filter(|n| n.required && !report.has(&n.game)) {
            lacking.push((name.clone(), n.game.clone()));
        }
    }
    Playable {
        id: def.id.clone(),
        map,
        lacking,
    }
}

/// Every registered minigame, on its pool's first map, against the
/// connected players (the lobby's playable set: `PartyOffer`).
pub fn playable(world: &mut World) -> Vec<Playable> {
    let defs: Vec<&'static MinigameDef> = world.get_resource::<Minigames>().map(|m| m.all().collect()).unwrap_or_default();
    defs.into_iter().map(|d| playable_on(world, d, None)).collect()
}

/// `party_offer`'s text.
fn offer_text(world: &mut World) -> String {
    if world.get_resource::<NetRole>() == Some(&NetRole::Client) {
        let offer = world.get_resource::<PartyOffer>().cloned().unwrap_or_default();
        return if offer.minigames.is_empty() {
            "the server offers no minigame".into()
        } else {
            offer.minigames.join("\n")
        };
    }
    let games = world.get_resource::<Games>().cloned();
    let lines: Vec<String> = playable(world)
        .iter()
        .map(|p| {
            if p.ok() {
                format!("{} on {}", p.id, p.map)
            } else {
                format!("({} on {}: {})", p.id, p.map, lacking_text(games.as_ref(), &p.lacking))
            }
        })
        .collect();
    if lines.is_empty() { "no minigames".into() } else { lines.join("\n") }
}

/// "Alice lacks CS:S (cs_source)", each player once.
fn lacking_text(games: Option<&Games>, lacking: &[(String, String)]) -> String {
    lacking
        .iter()
        .map(|(who, game)| format!("{who} lacks {}", manifest::game_name(games, game)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The server: the playable set to every client when it or the players
/// change.
fn send_offer(world: &mut World, mut last: Local<Option<(Vec<String>, usize)>>) {
    let players = world.query::<&crate::net::server::Player>().iter(world).count();
    let offer: Vec<String> = playable(world).into_iter().filter(|p| p.ok()).map(|p| p.id).collect();
    let now = (offer, players);
    if last.as_ref() == Some(&now) {
        return;
    }
    *last = Some(now.clone());
    if players == 0 {
        return;
    }
    world.write_message(ToClients {
        targets: SendTargets::CLIENTS_ONLY,
        message: PartyOffer { minigames: now.0 },
    });
}

/// A client: the server's offer, kept.
fn receive_offer(mut offers: MessageReader<PartyOffer>, mut commands: Commands) {
    if let Some(o) = offers.read().last() {
        commands.insert_resource(o.clone());
    }
}

/// `change_match <id> [map]` (M1): the server plays minigame `id` from
/// now on, its players kept and connected: a fresh match (`minigame::
/// start`: scores anew, everyone joins again), clients told by
/// `ChangeMatch` (`manifest::announce`, from `write_minigame`). The map is
/// `map`, else this one if it is in the minigame's pool, else the pool's
/// first; another map is the app's `changelevel` (which takes every client
/// along with the new match's manifest). A client can't change match.
pub fn change_match(world: &mut World, id: &str, map: Option<&str>) -> Result<String, String> {
    if world.get_resource::<NetRole>() == Some(&NetRole::Client) {
        return Err("the server picks the minigame; you are a client of it".into());
    }
    let def = world
        .get_resource::<Minigames>()
        .and_then(|m| m.get(id))
        .ok_or_else(|| format!("no minigame `{id}` (minigames lists them)"))?;
    let here = crate::net::current_map(world);
    let pool: Vec<String> = def.maps.iter().map(|m| crate::net::normalize_map(m)).collect();
    let target = match map {
        Some(m) => crate::net::normalize_map(m),
        None if pool.contains(&here) => here.clone(),
        None => pool.first().cloned().unwrap_or_else(|| here.clone()),
    };
    // Only what every player can play (the playable set, M3).
    let p = playable_on(world, def, Some(&target));
    if !p.ok() {
        let games = world.get_resource::<Games>().cloned();
        return Err(format!(
            "not every player can play {id} on {target}: {}",
            lacking_text(games.as_ref(), &p.lacking)
        ));
    }
    if target != here {
        minigame::check(world, def)?;
        // The map left was played (a rotation's random pick skips it).
        world.get_resource_or_init::<MapHistory>().record(&here);
        // Nothing starts on the map being left: the match is held until
        // the new one is in everywhere (`follow_map_change`).
        let began = world.get_resource::<SimClock>().map_or(0.0, |c| c.now);
        world.insert_resource(minigame::MatchHold);
        world.insert_resource(MapChange {
            def,
            map: target.clone(),
            began,
            loading: false,
            arrived: None,
        });
        // A server moves its players with it; a standalone game (no
        // server) just loads the map.
        let command = if world.get_resource::<NetRole>() == Some(&NetRole::Server) { "changelevel" } else { "map" };
        world
            .resource_mut::<crate::console::Console>()
            .submit(format!("{command} {target}"));
        return Ok(format!(
            "playing {id}: {}; changing level to {target}, the match starts once everyone has it",
            def.name
        ));
    }
    world.remove_resource::<MapChange>();
    minigame::start(world, id)?;
    Ok(format!("playing {id}: {} on {target}", def.name))
}

/// How long (s) a match's map change waits for the map to come in here
/// before it gives up and starts the match where the server is.
pub const MAP_LOAD_TIMEOUT: f64 = 60.0;
/// How long (s) after the new map came in the match waits for clients
/// still loading it.
pub const CLIENT_GRACE: f64 = 20.0;

/// A server (or a standalone game) changing map for its next match
/// (`change_match` to another map): the match is held
/// (`minigame::MatchHold`) until the map is in and every client has it.
#[derive(Resource, Clone, Debug)]
pub struct MapChange {
    /// The minigame to start there.
    pub def: &'static MinigameDef,
    /// The map (`net::normalize_map`'s form).
    pub map: String,
    /// When the change began (`SimClock`).
    pub began: f64,
    /// The load was seen under way (the current map became `map`).
    pub loading: bool,
    /// When the map was in here.
    pub arrived: Option<f64>,
}

/// Whether every client reported `map` loaded (`server::ClientMap`);
/// true without clients.
fn clients_have(world: &mut World, map: &str) -> bool {
    world
        .query::<(&crate::net::server::Player, Option<&crate::net::server::ClientMap>)>()
        .iter(world)
        .all(|(_, m)| m.is_some_and(|m| m.0.eq_ignore_ascii_case(map)))
}

/// A match's map change (`MapChange`) on: once the map is in here and
/// every client has it (or `CLIENT_GRACE` s after it came in), the
/// match starts there; a load that failed, or none in
/// `MAP_LOAD_TIMEOUT` s, starts it where the server is.
fn follow_map_change(world: &mut World) {
    let Some(mut c) = world.get_resource::<MapChange>().cloned() else {
        return;
    };
    let now = world.resource::<SimClock>().now;
    let here = crate::net::current_map(world);
    let ready = match c.arrived {
        None => {
            let arrived = crate::net::maps::loaded_map(world).is_some_and(|(id, _)| id == c.map);
            if arrived {
                info!("{} is in: the next match waits for the players to load it", c.map);
                c.arrived = Some(now);
                clients_have(world, &c.map)
            } else if here == c.map {
                c.loading = true;
                false
            } else if c.loading || now > c.began + MAP_LOAD_TIMEOUT {
                warn!("{} didn't load: the next match starts on {here}", c.map);
                true
            } else {
                false
            }
        }
        Some(at) => {
            let all = clients_have(world, &c.map);
            if !all && now >= at + CLIENT_GRACE {
                warn!("not every player has {} after {CLIENT_GRACE} s: the match starts", c.map);
            }
            all || now >= at + CLIENT_GRACE
        }
    };
    if !ready {
        world.insert_resource(c);
        return;
    }
    world.remove_resource::<MapChange>();
    let id = c.def.id.clone();
    match minigame::start(world, &id) {
        Ok(def) => info!("{} starts on {here}", def.id),
        Err(e) => {
            world.remove_resource::<minigame::MatchHold>();
            warn!("starting {id} after the map change: {e}");
        }
    }
}

/// The games minigame `def` reads from a player's install: its first
/// map's game required (none for the greybox), and wanted, every other
/// registered game it names (its slots, movement, character, weapon sets
/// and kit). Ids that aren't registered games (`chimera`) are left out;
/// without `Games` every id counts.
pub fn needs(def: &MinigameDef, games: Option<&Games>) -> Vec<GameNeed> {
    let mut m = MatchManifest::default();
    let first = def.maps.first().map(|m| crate::net::normalize_map(m)).unwrap_or_default();
    if first != GREYBOX
        && let Some((game, _)) = first.split_once(':')
    {
        m.need(game, true);
    }
    for g in wanted(def) {
        m.need(&g, false);
    }
    m.games.retain(|n| games.is_none_or(|gs| gs.get(&n.game).is_some()));
    m.games
}

/// The game ids a definition names besides its maps.
fn wanted(def: &MinigameDef) -> Vec<String> {
    let s = &def.slots;
    let prefix = |id: &str| id.split_once(':').map(|(g, _)| g.to_string());
    let mut out: Vec<String> = [&s.hud, &s.binds, &s.tick, &s.bots].into_iter().flatten().cloned().collect();
    out.extend([&s.movement, &s.character].into_iter().flatten().filter_map(|id| prefix(id)));
    out.extend(s.weapons.iter().cloned());
    out.extend(def.spawn.kit.all().into_iter().filter_map(prefix));
    out
}

/// The server's manifest hook: the playing minigame (the coming one
/// during a map change for it; its id, its definition's hash and text),
/// the games it names (wanted), its weapon sets and the slots it names
/// over the gameplay's.
fn add_to_manifest(world: &mut World, m: &mut MatchManifest) {
    // The match to come while the map changes for it (`MapChange`).
    let coming = world.get_resource::<MapChange>().map(|c| c.def);
    let Some(def) = coming.or_else(|| world.get_resource::<ActiveMinigame>().map(|a| a.def)) else {
        return;
    };
    let games = world.get_resource::<Games>();
    // Its pool's first map's game too, wanted: the map played is the
    // served one (`manifest::current` requires its game).
    for n in needs(def, games) {
        m.need(&n.game, false);
    }
    m.weapons = def.slots.weapons.clone();
    for (slot, game) in [("hud", &def.slots.hud), ("binds", &def.slots.binds), ("bots", &def.slots.bots)] {
        if let Some(g) = game
            && let Some(s) = m.slots.iter_mut().find(|(s, _)| s == slot)
        {
            s.1 = g.clone();
        }
    }
    let text = world.get_resource::<Minigames>().and_then(|ms| ms.text(&def.id)).unwrap_or_default();
    m.minigame = Some(MinigameOffer {
        id: def.id.clone(),
        hash: sha256(text.as_bytes()),
        text: text.to_string(),
    });
}

/// A client's manifest hook: the server's minigame is one this build can
/// play: its own definition with the same hash, else the server's text,
/// read and checked against what this build registered (`minigame::add`).
fn accept_minigame(world: &mut World, m: &MatchManifest) -> Result<(), String> {
    let Some(offer) = &m.minigame else {
        return Ok(());
    };
    let have = world.get_resource::<Minigames>().and_then(|ms| ms.text(&offer.id));
    if have.is_some_and(|t| sha256(t.as_bytes()) == offer.hash) {
        return Ok(());
    }
    if sha256(offer.text.as_bytes()) != offer.hash {
        return Err(format!("{CANT_PLAY}{} came with a definition that doesn't match its hash.", offer.id));
    }
    let def = minigame::add(world, &offer.id, &offer.text)
        .map_err(|e| format!("{CANT_PLAY}{} can't be played by this build: {e}", offer.id))?;
    if def.id != offer.id {
        return Err(format!("{CANT_PLAY}{}'s definition names itself {}.", offer.id, def.id));
    }
    info!("playing the server's definition of {}", offer.id);
    Ok(())
}

/// The match's events, to every client.
fn send_news(
    mut events: MessageReader<MinigameEvent>,
    replicated: Query<(), With<Replicated>>,
    mut out: MessageWriter<ToClients<MinigameNews>>,
) {
    for e in events.read() {
        let Some(name) = EVENT_NAMES.iter().position(|n| *n == e.name) else {
            warn!("minigame event `{}` isn't in minigame::EVENT_NAMES: not sent", e.name);
            continue;
        };
        out.write(ToClients {
            targets: SendTargets::CLIENTS_ONLY,
            message: MinigameNews {
                name: name as u8,
                player: e.player.filter(|p| replicated.contains(*p)),
                value: e.value,
            },
        });
    }
}

/// The server's minigame and match into what the HUD reads: the minigame
/// started here when it is a new one, the match's phase and end.
fn apply_minigame(world: &mut World, mut refused: Local<Option<String>>) {
    let Some(n) = world.query::<&NetMinigame>().iter(world).next().cloned() else {
        return;
    };
    // The definition comes with the match's manifest (`accept_minigame`):
    // wait for the manifest that names it.
    let offered = world
        .get_resource::<ServerManifest>()
        .and_then(|m| m.0.minigame.as_ref())
        .is_some_and(|o| o.id == n.id);
    if !offered {
        return;
    }
    // A new minigame, or the same id with a new definition (the server
    // made it again: an automatic gun game's new ladder) that the
    // manifest put in place of the one playing.
    let current = world.get_resource::<Minigames>().and_then(|m| m.get(&n.id));
    if world
        .get_resource::<ActiveMinigame>()
        .is_none_or(|a| a.def.id != n.id || current.is_some_and(|c| !std::ptr::eq(c, a.def)))
    {
        match crate::minigame::join_remote(world, &n.id) {
            Ok(def) => {
                info!("the server plays {}: {}", def.id, def.name);
                *refused = None;
            }
            Err(e) => {
                if refused.as_deref() != Some(&n.id) {
                    error!("the server plays minigame {}, which this build can't start: {e}", n.id);
                    *refused = Some(n.id.clone());
                }
                return;
            }
        }
    }
    let local = ServerTime::of(world).map(|t| {
        let now = world.resource::<SimClock>().now;
        move |tick: f64| now + (tick - t.tick) * t.step
    });
    let (phase, ends) = match n.phase {
        phase::LIVE => (Phase::Live, n.ends.zip(local.as_ref()).map(|(e, f)| f(e))),
        phase::OVER => (Phase::MatchOver, None),
        _ => (Phase::Waiting, None),
    };
    // The server's rungs (the melee rungs' draw is the host's) and rung
    // extras (the client's ladder draws none).
    minigame::ladder::take_rungs(world, &n.rungs, n.melee_rungs);
    if let Some(mut ladder) = world.get_resource_mut::<Ladder>()
        && ladder.extras != n.rung_extras
    {
        ladder.extras = n.rung_extras.clone();
    }
    let mut m = world.get_resource_or_init::<Match>();
    if m.phase != phase {
        m.phase = phase;
    }
    if m.round_ends != ends {
        m.round_ends = ends;
    }
    if world.get_resource::<MatchResults>() != n.results.as_ref() {
        match n.results {
            Some(r) => world.insert_resource(r),
            None => {
                world.remove_resource::<MatchResults>();
            }
        }
    }
    // The map vote, its close on this clock (ballots stay the server's).
    let vote = n.vote.and_then(|v| {
        let closes = local.as_ref().map(|f| f(v.closes))?;
        let mut m = MapVote::new(v.candidates, closes);
        m.counts = v.counts;
        m.winner = v.winner;
        Some(m)
    });
    let have = world.get_resource::<MapVote>();
    let same = match (have, &vote) {
        (Some(h), Some(v)) => h.candidates == v.candidates && h.counts == v.counts && h.winner == v.winner && (h.closes - v.closes).abs() < 0.1,
        (None, None) => true,
        _ => false,
    };
    if !same {
        match vote {
            Some(v) => world.insert_resource(v),
            None => {
                world.remove_resource::<MapVote>();
            }
        }
    }
    // The match-end hold, its time on this clock (rewritten only when it
    // moves by more than the clocks' jitter).
    let next = n.next.and_then(|(map, at)| Some(NextMatch { map, at: local.as_ref().map(|f| f(at))? }));
    let have = world.get_resource::<NextMatch>();
    let same = match (have, &next) {
        (Some(h), Some(n)) => h.map == n.map && (h.at - n.at).abs() < 0.1,
        (None, None) => true,
        _ => false,
    };
    if !same {
        match next {
            Some(n) => world.insert_resource(n),
            None => {
                world.remove_resource::<NextMatch>();
            }
        }
    }
}

/// The minigame's end of the tick on a server or a standalone game: a
/// map change for the next match followed (`follow_map_change`), the
/// next map once the match has been over a while (`rotate`), and (a
/// server) the match into `NetMinigame`.
fn serve_minigame(world: &mut World) {
    follow_map_change(world);
    rotate(world);
    if world.get_resource::<NetRole>() == Some(&NetRole::Server) {
        write_minigame(world);
    }
}

/// A server (or a standalone game): the match is decided and its results
/// up (`minigame::NextMatch`, the hold `results::record` began): the next
/// map (`next_map`: in turn, or at random but the maps played lately) is
/// picked at once for the match-end screen, and once the match has been
/// over `RESULTS_HOLD` s the server moves there (`go_to`: the app's
/// `NextMatchHooks`, then `change_match`; the map picked again if a player
/// who joined meanwhile can't play it).
fn rotate(world: &mut World) {
    if world.contains_resource::<MapChange>() {
        return;
    }
    let Some(active) = world.get_resource::<ActiveMinigame>().copied() else { return };
    let Some(next) = world.get_resource::<NextMatch>().cloned() else { return };
    let def = active.def;
    let now = world.resource::<SimClock>().now;
    let target = match next.map.clone() {
        Some(m) => m,
        None => match world.get_resource::<MapVote>().cloned() {
            // The vote is open: its close picks the map.
            Some(vote) if vote.open() => {
                if now < vote.closes {
                    return;
                }
                let m = close_vote(world);
                world.resource_mut::<NextMatch>().map = Some(m.clone());
                m
            }
            _ => {
                let m = next_map(world, def);
                if open_vote(world, def, &m) {
                    return;
                }
                world.resource_mut::<NextMatch>().map = Some(m.clone());
                m
            }
        },
    };
    let over = world.get_resource::<Match>().is_some_and(|m| m.phase == Phase::MatchOver);
    if !over || now < next.at {
        return;
    }
    let target = if playable_on(world, def, Some(&target)).ok() { target } else { next_map(world, def) };
    go_to(world, def, &target);
}

/// The map vote opened when `def` has one (`minigame::vote`): the
/// rotation's pick `first` and other playable maps of the pool on the
/// ballot, closing `time` s from now, the hold (`NextMatch::at`) moved to
/// its close plus the reveal. False without a vote, or with one candidate
/// only (the rotation's pick stands).
fn open_vote(world: &mut World, def: &'static MinigameDef, first: &str) -> bool {
    let Ok(Some(settings)) = MapVoteSettings::of(def) else { return false };
    let here = crate::net::current_map(world);
    let pool: Vec<String> = def.maps.iter().map(|m| crate::net::normalize_map(m)).collect();
    let ok: Vec<String> = pool.iter().filter(|m| playable_on(world, def, Some(m)).ok()).cloned().collect();
    let here_ok = playable_on(world, def, Some(&here)).ok();
    let mut history = world.remove_resource::<MapHistory>().unwrap_or_default();
    let recent = history.recent(def.avoid_recent).to_vec();
    let playable = |m: &str| ok.iter().any(|o| o == m) || (m == here && here_ok);
    let ballot = vote::candidates(first, &pool, &here, &recent, settings.candidates, &playable, &mut history.rng);
    world.insert_resource(history);
    if ballot.len() < 2 {
        return false;
    }
    let now = world.resource::<SimClock>().now;
    let closes = now + settings.time as f64;
    info!("{}: map vote on {}", def.id, ballot.join(", "));
    world.insert_resource(MapVote::new(ballot, closes));
    let mut next = world.resource_mut::<NextMatch>();
    next.at = next.at.max(closes + settings.reveal as f64);
    true
}

/// The open vote closed: the winner (`MapVote::tally` on the host's
/// generator), said on the console.
fn close_vote(world: &mut World) -> String {
    let mut history = world.remove_resource::<MapHistory>().unwrap_or_default();
    let mut vote = world.resource_mut::<MapVote>();
    let map = vote.close(&mut history.rng);
    let counts = vote.counts.clone();
    world.insert_resource(history);
    info!("map vote: {map} ({counts:?})");
    map
}

/// The local player votes for candidate `choice` (from 0) of the open map
/// vote: a client sends it to its server, a host casts it for its own
/// player.
pub fn vote_map(world: &mut World, choice: usize) -> Result<(), String> {
    let vote = world.get_resource::<MapVote>().ok_or("no map vote now")?;
    if !vote.open() {
        return Err("the map vote is closed".into());
    }
    if choice >= vote.candidates.len() {
        return Err(format!("the vote has {} maps", vote.candidates.len()));
    }
    if world.get_resource::<NetRole>() == Some(&NetRole::Client) {
        world.write_message(MapVoteCast { choice: choice as u8 });
        return Ok(());
    }
    let me = world
        .query_filtered::<Entity, With<chimera_core::core::LocalPlayer>>()
        .iter(world)
        .next()
        .ok_or("no player here to vote")?;
    cast_vote(world, me, choice)
}

/// `voter` (a character in the match) votes for `choice` (the host's).
pub fn cast_vote(world: &mut World, voter: Entity, choice: usize) -> Result<(), String> {
    if world.get::<InMinigame>(voter).is_none() {
        return Err("only players in the match vote".into());
    }
    let mut vote = world.get_resource_mut::<MapVote>().ok_or("no map vote now")?;
    if vote.cast(voter, choice) { Ok(()) } else { Err("the vote is closed, or no such map".into()) }
}

/// The vote as it stands, for the console.
fn vote_text(world: &World) -> String {
    let Some(v) = world.get_resource::<MapVote>() else { return "no map vote now".into() };
    let mut lines: Vec<String> = v
        .candidates
        .iter()
        .zip(&v.counts)
        .enumerate()
        .map(|(i, (m, c))| format!("{}. {} ({c})", i + 1, vote::map_name(m)))
        .collect();
    if let Some(m) = v.winning_map() {
        lines.push(format!("won: {}", vote::map_name(m)));
    }
    lines.join("\n")
}

/// The server: each client's vote, counted for its character (a listen
/// server host's own through replicon's local resend).
fn receive_map_votes(mut casts: MessageReader<FromClient<MapVoteCast>>, mut commands: Commands) {
    for c in casts.read() {
        let (client, choice) = (c.client_id, c.message.choice as usize);
        commands.queue(move |w: &mut World| {
            let voter = match client.entity() {
                Some(e) => w.get::<crate::net::server::Player>(e).map(|p| p.character),
                None => w.query_filtered::<Entity, With<chimera_core::core::LocalPlayer>>().iter(w).next(),
            };
            let Some(voter) = voter else { return };
            if let Err(e) = cast_vote(w, voter, choice) {
                debug!("a map vote refused: {e}");
            }
        });
    }
}

/// The playing minigame's next match now (a host), without the hold: the
/// map `next_map` picks, then `go_to` it. Returns the map.
pub fn next_match(world: &mut World) -> Option<String> {
    let def = world.get_resource::<ActiveMinigame>()?.def;
    let target = next_map(world, def);
    go_to(world, def, &target);
    Some(target)
}

/// The next match of `def` on `target`: the app's `NextMatchHooks` told
/// (a new automatic ladder), then `change_match`.
fn go_to(world: &mut World, def: &'static MinigameDef, target: &str) {
    info!("{} is over: next, {} on {target}", def.id, def.name);
    let hooks = world.get_resource::<NextMatchHooks>().map(|h| h.0.clone()).unwrap_or_default();
    for hook in hooks {
        hook(world, &def.id, target);
    }
    if let Err(e) = change_match(world, &def.id, Some(target)) {
        warn!("rotating {}: {e}", def.id);
    }
}

/// What an app does as a rotation moves a minigame on (called with its id
/// and the next map, before the match changes): Ultimate Gun Game makes a
/// new ladder. Host only.
#[derive(Resource, Clone, Default)]
pub struct NextMatchHooks(pub Vec<fn(&mut World, &str, &str)>);

/// The map `def`'s next match plays on, among those every player can
/// play (`minigame::rotation::MapHistory::next`: the pool's next in turn,
/// or with `rotation = "random"` a random one but the last played, the
/// host's history kept; the host's `rotation` and `avoid_recent`
/// overrides, `minigame::overrides`, over the definition's); with
/// `rotate = false`, or none playable, this one.
fn next_map(world: &mut World, def: &'static MinigameDef) -> String {
    let here = crate::net::current_map(world);
    if !def.rotate {
        return here;
    }
    let pool: Vec<String> = def.maps.iter().map(|m| crate::net::normalize_map(m)).collect();
    let ok: Vec<String> = pool.iter().filter(|m| playable_on(world, def, Some(m)).ok()).cloned().collect();
    let mut history = world.remove_resource::<MapHistory>().unwrap_or_default();
    // The host's rotation settings (`minigame::overrides`) count from now.
    let rule = minigame::overrides::effective(world, def);
    let chosen = history.next(&rule, &pool, &here, &|m| ok.iter().any(|o| o == m));
    world.insert_resource(history);
    chosen.unwrap_or(here)
}

/// The server's events back into `MinigameEvent`s (the HUD's notices).
/// The server: each spawn-protected character's windows into
/// `NetProtected` (written when they change), gone with them.
fn write_protection(
    clock: Res<SimClock>,
    protected: Query<(Entity, &SpawnProtected, Option<&NetProtected>), With<Replicated>>,
    stale: Query<Entity, (With<NetProtected>, Without<SpawnProtected>)>,
    mut commands: Commands,
) {
    for (e, p, have) in &protected {
        let now = NetProtected {
            damage_until: tick_of(&clock, p.damage_until),
            bullets_until: tick_of(&clock, p.bullets_until),
        };
        // The ticks of the same times move by float noise only.
        let same = have.is_some_and(|h| {
            (h.damage_until - now.damage_until).abs() < 1e-3 && (h.bullets_until - now.bullets_until).abs() < 1e-3
        });
        if !same {
            commands.entity(e).insert(now);
        }
    }
    for e in &stale {
        commands.entity(e).remove::<NetProtected>();
    }
}

/// A client: `NetProtected` as `SpawnProtected` on its own clock (gone
/// with it).
fn apply_protection(world: &mut World) {
    let Some(t) = ServerTime::of(world) else { return };
    let now = world.resource::<SimClock>().now;
    let local = |tick: f64| now + (tick - t.tick) * t.step;
    let mut q = world.query::<(Entity, &NetProtected, Option<&SpawnProtected>)>();
    let set: Vec<(Entity, SpawnProtected)> = q
        .iter(world)
        .filter_map(|(e, n, have)| {
            let p = SpawnProtected {
                damage_until: local(n.damage_until),
                bullets_until: local(n.bullets_until),
            };
            // Rewritten only when it moves by more than the clocks' jitter.
            let same = have.is_some_and(|h| {
                (h.damage_until - p.damage_until).abs() < 0.05 && (h.bullets_until - p.bullets_until).abs() < 0.05
            });
            (!same).then_some((e, p))
        })
        .collect();
    for (e, p) in set {
        world.entity_mut(e).insert(p);
    }
    let gone: Vec<Entity> =
        world.query_filtered::<Entity, (With<SpawnProtected>, Without<NetProtected>)>().iter(world).collect();
    for e in gone {
        world.entity_mut(e).remove::<SpawnProtected>();
    }
}

fn receive_news(mut news: MessageReader<MinigameNews>, mut events: MessageWriter<MinigameEvent>) {
    for n in news.read() {
        let Some(name) = EVENT_NAMES.get(n.name as usize) else { continue };
        events.write(MinigameEvent {
            name,
            player: n.player.filter(|p| *p != Entity::PLACEHOLDER),
            value: n.value,
        });
    }
}
