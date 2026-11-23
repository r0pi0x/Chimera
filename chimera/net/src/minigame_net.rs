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
//! - **The match's effects** (`NetMinigame::effects`): the authority's
//!   `minigame::effects::MatchEffects` (a host's surprises), their times
//!   as server ticks, into the client's own (its clock), which the
//!   shell's effects screen draws (`chimera_client::client::match_effects`).
//! - **The slice** (`NetMinigame::arena`): the server's play area
//!   (`map::play_area::PlayArea`: a match on a part of its map, the party
//!   kit's `arena`), whole, into the client's `ArenaPlan` (`follow_arena`),
//!   so its wall, put-back and look are the server's.
//! - **Events** (`MinigameNews`): rung ups and downs, the last rung, the
//!   lead, a match won, for the HUD's notices and callouts, as the index
//!   in `minigame::EVENT_NAMES`.
//!
//! - **The manifest** (`net::manifest`): the server's hook adds the
//!   minigame (its id, its definition's SHA-256 and text), the games it
//!   names (wanted; `needs`), its weapon sets and the ladder it deals
//!   (`MinigameOffer::ladder`, the server's `match_ladder`: a client knows
//!   it before its map loads, `server_ladder`, and a game loads its models
//!   behind the loading screen); a client's hook plays the
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
//!   match only once the map is in here (a listen host's own load done:
//!   spawned and settled behind its loading screen, `map::LocalMapLoad`)
//!   and every client reported it loaded (`server::ClientMap`, sent once
//!   the client's own load is done), or `CLIENT_GRACE` s after the map came
//!   in (with the pre-match lobby on, `prematch`, as soon as the map is in
//!   here: the lobby waits for the clients); a load that fails (or none in `MAP_LOAD_TIMEOUT` s) starts it
//!   where the server is. A host's own `changelevel <map>` in a party
//!   app (this part's `net::HostChangeLevel`) while a minigame is
//!   the rules is that minigame's next match there, as a rotation's
//!   (`host_changelevel`, which the apps' `changelevel` commands ask
//!   first: the next-match hooks, then `change_match`), not only a new
//!   map under the running match. The manifest names the coming minigame
//!   meanwhile, and its asset offer the coming ladder's draw (its melee
//!   weapons, a game's rung bodies: `match_ladder`, `ladder::draw`). A server rotates through it by itself once a match has
//!   been over for `RESULTS_HOLD` s (`serve_minigame`,
//!   `MinigameDef::rotate`: the pool's next map, or a random one but the
//!   last played, `minigame::rotation::MapHistory`, which the host keeps
//!   for its lifetime; `NextMatchHooks` let the app act first) and puts
//!   the results up meanwhile (`NetMinigame::results`, each row's
//!   character mapped to the client's entity) with the match-end hold
//!   (`minigame::NextMatch`: the map picked for the next match as soon as
//!   the match is decided, and when the change comes; `NetMinigame::next`),
//!   which the shell's match-end screen shows.
//! - **A failed rotation** (`rotate`, `RotateFailed`): a move to the next
//!   match that fails (a ladder no game makes for these players, a map one
//!   lacks the game for) is told once and tried again only when the
//!   players or their mount reports change, not every frame.
//! - **The pool by count** (a definition's `map_fit`, `minigame::map_fit`):
//!   the rotation's pick and the vote's ballot are drawn by the players
//!   present at the change (humans and bots): the maps too big for them
//!   out, the rest by weight (`map_fit::draw_weights`); a host's
//!   `changelevel` without a map is the next match as a rotation's
//!   (`host_next_level`, `net::HostNextLevel`); console `map_pool
//!   [players]` lists the pool at a count.
//! - **Arena slices in the pool** (docs/plans/active/arena-slices.md,
//!   A2): in a party app (`arena::SlicesInPool`) the rotation's pool has
//!   each map's slices after it (`rotation_pool`, `arena::with_slices`),
//!   `game:map@id`, judged by `map_fit` (a slice only where its whole map
//!   is too big). The entry in play is `current_entry` (the map, or
//!   `map@id` while a slice plays); the history records entries and the
//!   hold-back and the ballot count a slice as its map. A match's change
//!   to `map@id` (a rotation's, a vote's, a host's `changelevel map@id`)
//!   plans the slice (`arena::plan`) and loads the map by its own name
//!   (nothing new on the wire: the slice reaches clients with the match);
//!   to a whole map it clears the plan; the same map again (another slice)
//!   loads it again and waits for the load (`MapChange::reload`).
//!   `mp_maps_off` names a slice as `map@id`; a map off takes its slices
//!   with it.
//! - **The map vote** (a definition's `map_vote`, `minigame::vote`): with
//!   one, the hold's next map isn't picked at once: the rotation's pick
//!   and a few other maps go on a ballot (`minigame::vote::MapVote`,
//!   replicated as `NetMinigame::vote` with the counts, no ballots) and
//!   the hold is set by it (`NextMatch::at` its close plus the reveal: one
//!   deadline); each player votes (`vote_map`, console `vote_map <n>`: a
//!   client sends `MapVoteCast`, the server counts it for that client's
//!   character; a host casts for its own player; a spectator only with
//!   `sv_vote_allow_spectators 1`), and at the close the
//!   most votes win (a tie: the pool's pick, the tied map earliest on the
//!   ballot; none: the rotation's pick), which
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
        manifest::{self, CANT_PLAY, GameNeed, LadderOffer, MatchManifest, MinigameOffer, ServerManifest},
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
    /// The match's effects on now (`minigame::effects::MatchEffects`:
    /// an air strike, moon gravity), their times as server ticks.
    pub effects: Vec<minigame::effects::Effect>,
    /// The slice of the map the match is played on (`map::play_area`:
    /// the server's `PlayArea`), its shape sent whole so a client of
    /// another build plays the server's; a client's `ArenaPlan`
    /// (`follow_arena`). None: the whole map.
    pub arena: Option<minigame::arena::ArenaSlice>,
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
    // A map change for the next match is part of what its asset sets are
    // made of (`server_assets::MatchInputs`).
    crate::net::server_assets::watch_input::<MapChange>(app);
    // The maps a host took out of the rotation (the party apps' Maps
    // picker): `mp_maps_off`.
    app.init_resource::<MapsOff>();
    crate::console::resource_cvar::<MapsOff, String>(
        app,
        "mp_maps_off",
        "Server: the maps out of the minigame's rotation and vote, comma-separated (`game:map`; the Host page's Maps picker sets it). Taking every map out takes none.",
        |m| &mut m.0,
    );
    // A host's `changelevel` is the minigame's next match there.
    app.insert_resource(crate::net::HostChangeLevel(host_changelevel));
    app.insert_resource(crate::net::HostNextLevel(host_next_level));
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
    // The pre-match lobby (`prematch`), after the tick's minigame (a
    // match started by a map change is held at once).
    crate::prematch::register(app);
    // Spectators (`spectators`): the cap, the switch, joining as one.
    crate::spectators::register(app);
    app.add_systems(
        FixedLast,
        (crate::prematch::begin, crate::prematch::serve)
            .chain()
            .after(serve_minigame)
            .run_if(resource_equals(NetRole::Server)),
    );
    {
        let mut hooks = app.world_mut().resource_mut::<NetHooks>();
        hooks.stopped_serving.push(stopped_serving);
        hooks.manifest.push(add_to_manifest);
        hooks.accept.push(accept_minigame);
    }
    // What a match's asset offer names of the minigame (`net::assets`).
    app.world_mut()
        .get_resource_or_init::<crate::net::assets::AssetHooks>()
        .0
        .push(|w| crate::net::assets::AssetNames {
            weapons: offered_weapons(w),
            ..Default::default()
        });
    use crate::console::ConsoleAppExt;
    app.console_command(
        "chimera:change_match",
        "change_match <minigame> [map]: play another minigame, the players kept and connected, its scores \
         anew; on [map], else on this map if it is in the minigame's pool, else on the pool's first.",
        |w, a| {
            let id = a.first().ok_or("change_match <minigame> [map]")?;
            change_match(w, id, a.get(1).map(String::as_str)).map(Some)
        },
    )
    .console_command(
        "chimera:vote_map",
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
        "chimera:map_pool",
        "map_pool [players]: the playing minigame's map pool at that many players (else the characters present): \
         each map's share of the next draw, or why it is out (its data/map_pool.toml, its measurements).",
        |w, a| {
            let n = a.first().map(|n| n.parse::<usize>().map_err(|_| format!("not a count: {n}"))).transpose()?;
            pool_text(w, n).map(Some)
        },
    )
    .console_command(
        "chimera:party_offer",
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
            (apply_minigame, follow_arena, crate::minigame::ladder::follow_rungs, apply_protection, receive_news, receive_offer)
                .chain()
                .after(ClientSystems::Receive)
                .run_if(client()),
        );
}

/// A client plays the slice the server's match is played on
/// (`NetMinigame::arena` into `map::play_area::ArenaPlan`; none without a
/// match), which the map's play area follows when that map is in.
fn follow_arena(world: &mut World) {
    use minigame::arena::ArenaPlan;
    let want = world.query::<&NetMinigame>().iter(world).next().and_then(|n| n.arena.clone());
    if world.get_resource::<ArenaPlan>().is_none_or(|p| p.0 != want) {
        world.insert_resource(ArenaPlan(want));
        // The wall now, in the same frame as the match that brought it (a
        // client that joins mid-match moves its player from the next
        // tick; the plan's own follower may already have run this frame).
        chimera_world::map::play_area::follow_plan(world);
    }
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
        effects: world
            .get_resource::<minigame::effects::MatchEffects>()
            .map(|m| {
                m.effects
                    .iter()
                    .map(|e| minigame::effects::Effect {
                        starts: tick_of(&clock, e.starts),
                        impact_at: e.impact_at.map(|t| tick_of(&clock, t)),
                        ends: tick_of(&clock, e.ends),
                        ..e.clone()
                    })
                    .collect()
            })
            .unwrap_or_default(),
        arena: world.get_resource::<minigame::arena::PlayArea>().map(|a| a.slice.clone()),
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
    if !crate::net::is_code_map(&map)
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
    change_match_on(world, id, map, false)
}

/// `change_match` that loads the map even when its name is the one in
/// play: for a start from a front end that shows no world (a main menu,
/// where the map "in play" is only a name), so the load brings the world
/// in and takes the player into the game as any map change does.
pub fn change_match_loading(world: &mut World, id: &str, map: Option<&str>) -> Result<String, String> {
    change_match_on(world, id, map, true)
}

/// The pool entry in play: the loaded map (`net::current_map`), as
/// `game:map@id` while an arena slice of it plays (`PlayArea`).
pub fn current_entry(world: &World) -> String {
    let map = crate::net::current_map(world);
    match world.get_resource::<minigame::arena::PlayArea>() {
        Some(a) if a.slice.map.eq_ignore_ascii_case(&map) => format!("{map}@{}", a.slice.id),
        _ => map,
    }
}

/// A pool entry (`game:map` or an arena slice's `game:map@id`) split: the
/// map, and the slice's key with the slice planned (`arena::plan`) or the
/// plan cleared for a whole map. Err: no such slice.
fn plan_entry(world: &mut World, entry: &str) -> Result<String, String> {
    match entry.split_once('@') {
        Some((map, _)) => {
            minigame::arena::plan(world, Some(entry))?;
            Ok(map.to_string())
        }
        None => {
            if world.get_resource::<minigame::arena::ArenaPlan>().is_some_and(|p| p.0.is_some()) {
                minigame::arena::plan(world, None)?;
            }
            Ok(entry.to_string())
        }
    }
}

fn change_match_on(world: &mut World, id: &str, map: Option<&str>, load: bool) -> Result<String, String> {
    if world.get_resource::<NetRole>() == Some(&NetRole::Client) {
        return Err("the server picks the minigame; you are a client of it".into());
    }
    let def = world
        .get_resource::<Minigames>()
        .and_then(|m| m.get(id))
        .ok_or_else(|| format!("no minigame `{id}` (minigames lists them)"))?;
    // The entry in play: an arena slice is `map@id` (`current_entry`).
    let here = current_entry(world);
    let pool: Vec<String> = def.maps.iter().map(|m| crate::net::normalize_map(m)).collect();
    let target = match map {
        Some(m) => crate::net::normalize_map(m),
        None if pool.contains(&here) => here.clone(),
        None => pool.first().cloned().unwrap_or_else(|| here.clone()),
    };
    // An arena slice by its full key (`game:map@id`).
    let target = if target.contains('@') {
        world
            .get_resource::<minigame::arena::ArenaSlices>()
            .ok_or("no arena slices registered")?
            .find(&target)?
            .slice
            .key()
    } else {
        target
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
    // Another minigame: the match left is over for good, its end screen
    // and its countdown to a next match of its own (`NextMatch`, the
    // rotation's) gone at once, not shown through the change.
    if world.get_resource::<ActiveMinigame>().is_some_and(|a| a.def.id != def.id) {
        world.remove_resource::<MatchResults>();
        world.remove_resource::<NextMatch>();
        world.remove_resource::<MapVote>();
    }
    // A map dressed for fewer games than this match deals loads again,
    // dressed for it.
    let load = load || (target == here && !dressed_for(world, def));
    if load || target != here {
        minigame::check(world, def)?;
        // An arena slice: planned for its map (the map comes in with its
        // wall; clients get it with the match, `NetMinigame::arena`); a
        // whole map: no slice.
        let map = plan_entry(world, &target)?;
        // The entry left was played (a rotation's random pick skips it,
        // and a slice's map with it).
        world.get_resource_or_init::<MapHistory>().record(&here);
        // Nothing starts on the map being left: the match is held until
        // the new one is in everywhere (`follow_map_change`). The same map
        // again (another slice of it, or it whole) loads again: the change
        // waits for the load to replace the map in.
        let began = world.get_resource::<SimClock>().map_or(0.0, |c| c.now);
        let reload = (map == crate::net::current_map(world))
            .then(|| crate::net::maps::map_file_tick(world))
            .flatten();
        world.insert_resource(minigame::MatchHold);
        world.insert_resource(MapChange {
            def,
            map: map.clone(),
            began,
            loading: false,
            arrived: None,
            reload,
        });
        // With the lobby on, the lobby between the maps from now: every
        // player's row loading the map coming in (`prematch::begin_between`);
        // none for a match entered at once (`lobby_rule`).
        lobby_rule(world);
        crate::prematch::begin_between(world, &map, &def.name);
        // A server moves its players with it; a standalone game (no
        // server) just loads the map.
        let command = if world.get_resource::<NetRole>() == Some(&NetRole::Server) { "changelevel" } else { "map" };
        world
            .resource_mut::<crate::console::Console>()
            .submit(format!("{command} {map}"));
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
    /// The same map loaded again (another arena slice of it, or it whole
    /// after a slice): its map file's change tick when the change began (`net::maps::map_file_tick`);
    /// the map is in only once a load replaced it.
    pub reload: Option<bevy::ecs::change_detection::Tick>,
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
            // In here, and a listen host's own load done (spawned and
            // settled behind its loading screen: `map::LocalMapLoad`), so
            // its player isn't started on while it still loads. A match
            // entered at once (`MinigameDef::lobby` false: the sandbox)
            // doesn't wait for the server's own game files besides
            // (`net::server_assets`): they come behind it, its guns on
            // demand.
            let reloaded = c.reload.is_none_or(|before| {
                crate::net::maps::map_file_tick(world).is_some_and(|t| t != before)
            });
            let loading = if c.def.lobby {
                crate::net::maps::local_load_under_way(world)
            } else {
                crate::net::maps::own_load_under_way(world)
            };
            let arrived = crate::net::maps::loaded_map(world).is_some_and(|(id, _)| id == c.map)
                && reloaded
                && !loading;
            if arrived {
                c.arrived = Some(now);
                if crate::prematch::lobby_on(world) {
                    // The pre-match lobby waits for them, showing who.
                    info!("{} is in: the next match's lobby waits for the players", c.map);
                    true
                } else {
                    info!("{} is in: the next match waits for the players to load it", c.map);
                    clients_have(world, &c.map)
                }
            } else if here == c.map && !(c.reload.is_some() && now > c.began + MAP_LOAD_TIMEOUT) {
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
    if !crate::net::is_code_map(&first)
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

/// The minigame of the match (the coming one while the map changes for
/// it), if any.
fn match_def(world: &World) -> Option<&'static MinigameDef> {
    let coming = world.get_resource::<MapChange>().map(|c| c.def);
    coming.or_else(|| world.get_resource::<ActiveMinigame>().map(|a| a.def))
}

/// The weapons the match can hand out (`game:weapon`): its ladder's (a
/// `chimera:ladder` module's `ladder`), its kit's, and the match's drawn
/// ladder's own (`match_ladder`: a drawn ladder's rungs, its melee rungs,
/// and each rung's extras' melee weapon and character weapon, which the
/// host draws), and the app's starting weapons (`weapon::StartingWeapons`,
/// every team's: a joining player holds them from its spawn until the
/// minigame arms it; the 2026-11-12 rehearsal's one on-demand fetch at a
/// join was Combat Arms' starting M4A1's reload sound). The asset offer's
/// `weapon/<id>` sets (`net::assets`).
pub fn match_weapons(world: &World) -> Vec<String> {
    let Some(def) = match_def(world) else { return Vec::new() };
    let mut out: Vec<String> = def
        .rules
        .params
        .get("ladder")
        .and_then(|l| l.as_array())
        .map(|l| l.iter().filter_map(|w| w.as_str().map(String::from)).collect())
        .unwrap_or_default();
    out.extend(def.spawn.kit.all().into_iter().map(String::from));
    if let Some(ladder) = match_ladder(world) {
        out.extend(ladder.rungs.iter().map(|w| w.to_string()));
        for extras in ladder.extras.iter().flatten() {
            out.extend(extras.melee.iter().cloned());
            out.extend(extras.character.iter().map(|c| c.weapon.clone()));
        }
    }
    if let Some(start) = world.get_resource::<chimera_sim::weapon::StartingWeapons>() {
        let teams = start.team.iter().flat_map(|(_, ids)| ids.iter());
        out.extend(teams.chain(start.all.iter()).map(|w| w.to_string()));
    }
    out.retain(|w| w.contains(':'));
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|w| seen.insert(w.clone()));
    out
}

/// The weapons a match's offer names (`match_weapons`), but in a sandbox
/// (`rung_command::HoldRungs` names the match's minigame): its ladder is
/// every gun of every game, so only the first rung's gun (where a player
/// starts) and the kit's come with the join; the others are fetched when
/// picked (`net::units`).
pub fn offered_weapons(world: &World) -> Vec<String> {
    let mut out = match_weapons(world);
    let Some(def) = match_def(world).filter(|d| sandbox(world, d)) else { return out };
    let mut rungs: Vec<String> = match_ladder(world).map(|l| l.rungs.iter().map(|r| r.to_string()).collect()).unwrap_or_default();
    rungs.extend(
        def.rules.params.get("ladder").and_then(|l| l.as_array()).into_iter().flatten().filter_map(|w| w.as_str().map(String::from)),
    );
    let first = rungs.first().cloned();
    let kit: Vec<String> = def.spawn.kit.all().into_iter().map(String::from).collect();
    out.retain(|w| Some(w) == first.as_ref() || kit.contains(w) || !rungs.contains(w));
    out
}

/// Whether `def` is played as a sandbox here (`rung_command::HoldRungs`
/// names it): its ladder is every gun of every game.
fn sandbox(world: &World, def: &MinigameDef) -> bool {
    world.get_resource::<chimera_sim::minigame::rung_command::HoldRungs>().is_some_and(|h| h.0 == def.id)
}

/// On a network client: the ladder of the match the server's manifest
/// names (`MinigameOffer::ladder`: the one playing, or the coming match's
/// while the map changes for it) and its minigame, as this client's ladder
/// will be once the match is in (`ladder::draw`, then the server's rungs
/// and extras, as `apply_minigame` takes them): known before the map
/// loads, so a game loads its models behind the loading screen (Combat
/// Arms' `preload::plan_coming_ladder`). None without one.
pub fn server_ladder(world: &World) -> Option<(&'static MinigameDef, Ladder)> {
    let offer = world.get_resource::<ServerManifest>()?.0.minigame.as_ref()?;
    let wire = offer.ladder.as_ref()?;
    let def = world.get_resource::<Minigames>()?.get(&offer.id)?;
    let mut ladder = minigame::ladder::draw(world, def)?;
    let catalog = world.get_resource::<chimera_sim::weapon::data::WeaponCatalog>()?;
    ladder.take_rungs(catalog, &wire.rungs, wire.melee_rungs);
    ladder.extras = wire.extras.clone();
    Some((def, ladder))
}

/// The playing ladder (`Ladder`), while no map change for another match
/// is under way (then the ladder is the old match's; `match_ladder` has
/// the coming one's).
pub fn playing_ladder(world: &World) -> Option<&Ladder> {
    world.get_resource::<Ladder>().filter(|_| !world.contains_resource::<MapChange>())
}

/// The match's ladder: the playing one (`playing_ladder`), and while the
/// map changes for the next match (`MapChange`, a rotation) the ladder
/// that match will deal there (`ladder::draw`: the host's next draw, the
/// same its start makes). What the asset offer names of a ladder's draw
/// (`match_weapons`, a game's `net::assets::AssetHooks` entry for its
/// rung extras: Combat Arms' bodies), so a client fetches the next
/// match's melee weapons and bodies with the level change.
pub fn match_ladder(world: &World) -> Option<std::borrow::Cow<'_, Ladder>> {
    match world.get_resource::<MapChange>() {
        Some(c) => minigame::ladder::draw(world, c.def).map(std::borrow::Cow::Owned),
        None => world.get_resource::<Ladder>().map(std::borrow::Cow::Borrowed),
    }
}

/// The games whose parts the match this world's map is loading for can
/// deal (`map::MapLoadOptions::dealt`: only their guests dress the map):
/// with a map change for a ladder's match under way (`MapChange`), its
/// ladder's guns', its rungs' extras' (melee weapons, characters), its
/// definition's kit's and slots' games. None, any game: no such change
/// (a sandbox, a plain `map`, a client: the coming match isn't known
/// here), or a definition without a ladder.
pub fn dealt_games(world: &World) -> Option<std::collections::BTreeSet<String>> {
    let def = world.get_resource::<MapChange>()?.def;
    let ladder = minigame::ladder::draw(world, def)?;
    Some(games_dealt(def, &ladder))
}

/// The games `ladder` of `def` deals (`dealt_games`).
fn games_dealt(def: &MinigameDef, ladder: &Ladder) -> std::collections::BTreeSet<String> {
    let prefix = |id: &str| id.split_once(':').map(|(g, _)| g.to_string());
    let mut games: std::collections::BTreeSet<String> = ladder.rungs.iter().filter_map(|r| prefix(r)).collect();
    for e in ladder.extras.iter().flatten() {
        games.extend(e.melee.as_deref().and_then(prefix));
        games.extend(e.character.as_ref().and_then(|c| prefix(&c.id)));
    }
    let s = &def.slots;
    games.extend([&s.hud, &s.binds, &s.tick, &s.bots].into_iter().flatten().cloned());
    games.extend([&s.movement, &s.character].into_iter().flatten().filter_map(|id| prefix(id)));
    games.extend(def.spawn.kit.all().into_iter().filter_map(|id| prefix(id)));
    games
}

/// Whether the loaded map was dressed for every game `def`'s next ladder
/// deals (`map::sources::MapDealt`, the load's `dealt`): a match started
/// on the map as it is (no load) must find the parts of each game it
/// deals there.
fn dressed_for(world: &World, def: &MinigameDef) -> bool {
    let Some(dressed) = world.get_resource::<chimera_world::map::sources::MapDealt>().and_then(|d| d.0.clone()) else {
        return true;
    };
    let Some(ladder) = minigame::ladder::draw(world, def) else { return true };
    games_dealt(def, &ladder).iter().all(|g| dressed.iter().any(|d| d.eq_ignore_ascii_case(g)))
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
    let text = world.get_resource::<Minigames>().and_then(|ms| ms.text(&def.id)).unwrap_or_default().to_string();
    // The ladder it deals (a client loads its models behind its loading
    // screen, `server_ladder`); not a sandbox's (every gun there is).
    let ladder = (!sandbox(world, def)).then(|| match_ladder(world)).flatten().map(|l| LadderOffer {
        rungs: l.rungs.iter().map(|r| r.to_string()).collect(),
        melee_rungs: l.melee_rungs,
        extras: l.extras.clone(),
    });
    m.minigame = Some(MinigameOffer {
        id: def.id.clone(),
        hash: sha256(text.as_bytes()),
        text,
        ladder,
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
    let coming = world.get_resource::<ServerManifest>().and_then(|m| m.0.minigame.as_ref()).map(|o| o.id.clone());
    if coming.as_ref() != Some(&n.id) {
        // Another minigame coming (the server changes match: its level
        // change's manifest names it): the match playing is over for
        // good, its end screen gone, as on the server (`change_match`).
        if coming.is_some() {
            world.remove_resource::<MatchResults>();
            world.remove_resource::<NextMatch>();
            world.remove_resource::<MapVote>();
        }
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
    // The match's effects, their times on this clock (rewritten only when
    // one comes, goes or moves by more than the clocks' jitter).
    if let Some(f) = local.as_ref() {
        let effects: Vec<minigame::effects::Effect> = n
            .effects
            .iter()
            .map(|e| minigame::effects::Effect { starts: f(e.starts), impact_at: e.impact_at.map(f), ends: f(e.ends), ..e.clone() })
            .collect();
        let near = |a: f64, b: f64| (a - b).abs() < 0.1;
        let same = world.get_resource::<minigame::effects::MatchEffects>().is_some_and(|have| {
            have.effects.len() == effects.len()
                && have.effects.iter().zip(&effects).all(|(h, e)| {
                    h.id == e.id
                        && h.stopped == e.stopped
                        && near(h.starts, e.starts)
                        && near(h.ends, e.ends)
                        && match (h.impact_at, e.impact_at) {
                            (Some(a), Some(b)) => near(a, b),
                            (a, b) => a.is_none() && b.is_none(),
                        }
                })
        }) || (effects.is_empty() && world.get_resource::<minigame::effects::MatchEffects>().is_none());
        if !same {
            let last_id = effects.iter().map(|e| e.id).max().unwrap_or(0);
            world.insert_resource(minigame::effects::MatchEffects { effects, last_id });
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
    lobby_rule(world);
    follow_map_change(world);
    rotate(world);
    lobby_rule(world);
    if world.get_resource::<NetRole>() == Some(&NetRole::Server) {
        write_minigame(world);
    }
}

/// The match coming or playing (`match_def`) entered at once
/// (`MinigameDef::lobby` false: the sandbox): no pre-match lobby for it
/// (`prematch::NoLobby`, which `prematch::lobby_on` reads).
fn lobby_rule(world: &mut World) {
    let none = match_def(world).is_some_and(|d| !d.lobby);
    if none != world.contains_resource::<crate::prematch::NoLobby>() {
        if none {
            world.insert_resource(crate::prematch::NoLobby);
        } else {
            world.remove_resource::<crate::prematch::NoLobby>();
        }
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
                    shorten_vote(world, now);
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
    // A move that failed (a new ladder no game can make, a map a player
    // lacks the game for) isn't tried again until the players or their
    // mount reports change: once, not every frame.
    let key = rotation_inputs(world);
    if world.get_resource::<RotateFailed>().is_some_and(|f| f.0 == key) {
        return;
    }
    let target = if playable_on(world, def, Some(&target)).ok() { target } else { next_map(world, def) };
    if go_to(world, def, &target) {
        world.remove_resource::<RotateFailed>();
    } else {
        world.insert_resource(RotateFailed(key));
    }
}

/// The inputs of a rotation's move (`rotate`): the connected players and
/// their mount doctor reports, as a hash.
fn rotation_inputs(world: &mut World) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut names: Vec<String> = world
        .query::<&crate::net::NetCharacter>()
        .iter(world)
        .filter(|c| c.owner.is_some())
        .map(|c| c.name.clone())
        .collect();
    names.sort();
    let mut reports: Vec<String> = reports(world).into_iter().map(|r| format!("{r:?}")).collect();
    reports.sort();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (names, reports).hash(&mut h);
    h.finish()
}

/// The inputs (`rotation_inputs`) of the rotation's move that failed;
/// `rotate` waits for them to change before it tries again.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
struct RotateFailed(u64);

/// The map vote opened when `def` has one (`minigame::vote`): the
/// rotation's pick `first` and other playable maps of the pool on the
/// ballot, closing `time` s from now, the hold (`NextMatch::at`) set to
/// its close plus the reveal (one deadline). False without a vote, or with one candidate
/// only (the rotation's pick stands).
fn open_vote(world: &mut World, def: &'static MinigameDef, first: &str) -> bool {
    let Ok(Some(settings)) = MapVoteSettings::of(def) else { return false };
    let here = current_entry(world);
    let pool = rotation_pool(world, def);
    let ok: Vec<String> = pool.iter().filter(|m| playable_on(world, def, Some(m)).ok()).cloned().collect();
    let here_ok = playable_on(world, def, Some(&here)).ok();
    // The rotation's own draw: the maps too big for the players present
    // out, the rest by weight (`minigame::map_fit`).
    let weights = minigame::map_fit::draw_weights(world, def, &pool);
    let weight = |m: &str| weight_of(&pool, weights.as_deref(), m);
    let mut history = world.remove_resource::<MapHistory>().unwrap_or_default();
    let recent = history.recent(def.avoid_recent).to_vec();
    let playable = |m: &str| ok.iter().any(|o| o == m) || (m == here && here_ok);
    let ballot =
        vote::candidates_weighted(first, &pool, &here, &recent, settings.candidates, &playable, &weight, &mut history.rng);
    world.insert_resource(history);
    if ballot.len() < 2 {
        return false;
    }
    let now = world.resource::<SimClock>().now;
    let closes = now + settings.time as f64;
    info!("{}: map vote on {}", def.id, ballot.join(", "));
    world.insert_resource(MapVote::new(ballot, closes));
    // One deadline: the vote's close is the next match's countdown, the
    // change the reveal after it (the results hold gives way to it).
    let mut next = world.resource_mut::<NextMatch>();
    next.at = closes + settings.reveal as f64;
    true
}

/// Every human in the match has voted: the vote closes `mp_vote_all_voted`
/// s from now (if it had longer), the change that long plus the reveal
/// after it. Bots don't count; with no human nothing changes.
fn shorten_vote(world: &mut World, now: f64) {
    let left = crate::prematch::vote_all_voted(world) as f64;
    let Some(closes_at) = world.get_resource::<MapVote>().map(|v| v.closes) else { return };
    if closes_at - now <= left {
        return;
    }
    let humans: Vec<Entity> = {
        let mut q = world.query_filtered::<(Entity, &crate::net::NetCharacter, Has<chimera_core::core::LocalPlayer>), With<InMinigame>>();
        q.iter(world).filter(|(_, c, local)| c.owner.is_some() || *local).map(|(e, _, _)| e).collect()
    };
    let vote = world.resource::<MapVote>();
    if humans.is_empty() || !humans.iter().all(|h| vote.ballots.iter().any(|(v, _)| v == h)) {
        return;
    }
    let reveal = world.resource::<ActiveMinigame>().def;
    let reveal = MapVoteSettings::of(reveal).ok().flatten().map_or(0.0, |s| s.reveal as f64);
    let closes = now + left;
    info!("map vote: everyone has voted; it closes in {left} s");
    world.resource_mut::<MapVote>().closes = closes;
    world.resource_mut::<NextMatch>().at = closes + reveal;
}

/// The open vote closed: the winner (`MapVote::tally`), said on the
/// console.
fn close_vote(world: &mut World) -> String {
    let mut vote = world.resource_mut::<MapVote>();
    let map = vote.close();
    let counts = vote.counts.clone();
    info!("map vote: {map} ({counts:?})");
    map
}

/// This machine's player's vote in the open map vote (set by `vote_map`;
/// the match-end screen marks it and clears it when the vote goes).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct MapVoteChoice(pub Option<usize>);

/// The local player votes for candidate `choice` (from 0) of the open map
/// vote: a client sends it to its server, a host casts it for its own
/// player; either way it is this machine's `MapVoteChoice`.
pub fn vote_map(world: &mut World, choice: usize) -> Result<(), String> {
    cast_local(world, choice)?;
    world.insert_resource(MapVoteChoice(Some(choice)));
    Ok(())
}

fn cast_local(world: &mut World, choice: usize) -> Result<(), String> {
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
    // A spectator votes only where the host lets spectators vote
    // (`sv_vote_allow_spectators`, `crate::spectators`).
    let watcher_may = world.get::<chimera_core::core::Spectating>(voter).is_some()
        && world.get_resource::<crate::net::vote::VoteSettings>().is_some_and(|s| s.allow_spectators != 0);
    if world.get::<InMinigame>(voter).is_none() && !watcher_may {
        return Err("only players in the match vote".into());
    }
    let mut vote = world.get_resource_mut::<MapVote>().ok_or("no map vote now")?;
    if vote.cast(voter, choice) { Ok(()) } else { Err("the vote is closed, or no such map".into()) }
}

/// The vote as it stands, for the console.
fn vote_text(world: &World) -> String {
    let Some(v) = world.get_resource::<MapVote>() else { return "no map vote now".into() };
    let slices = world.get_resource::<minigame::arena::ArenaSlices>();
    let mut lines: Vec<String> = v
        .candidates
        .iter()
        .zip(&v.counts)
        .enumerate()
        .map(|(i, (m, c))| format!("{}. {} ({c})", i + 1, vote::map_label(slices, m)))
        .collect();
    if let Some(m) = v.winning_map() {
        lines.push(format!("won: {}", vote::map_label(slices, m)));
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
fn go_to(world: &mut World, def: &'static MinigameDef, target: &str) -> bool {
    info!("{} is over: next, {} on {target}", def.id, def.name);
    let hooks = world.get_resource::<NextMatchHooks>().map(|h| h.0.clone()).unwrap_or_default();
    for hook in hooks {
        hook(world, &def.id, target);
    }
    match change_match(world, &def.id, Some(target)) {
        Ok(_) => true,
        Err(e) => {
            warn!("rotating {}: {e}", def.id);
            false
        }
    }
}

/// A host's `changelevel` to `map` (the app's map id) in a party app,
/// while a minigame is the rules: the minigame's next match there, as a
/// rotation moves it on (`go_to`: the app's `NextMatchHooks`, a new
/// ladder; `change_match`: the match held, the pre-match lobby on the new
/// map). None: a client, no minigame, the map being played, or a match's
/// map change under way (`MapChange`: this is its own `changelevel`): the
/// plain level change. This part's `net::HostChangeLevel`, which the apps'
/// `changelevel` commands ask first (`net::host_changelevel`).
pub fn host_changelevel(world: &mut World, map: &str) -> Option<Result<Option<String>, String>> {
    if world.get_resource::<NetRole>() == Some(&NetRole::Client)
        || world.contains_resource::<MapChange>()
    {
        return None;
    }
    let def = world.get_resource::<ActiveMinigame>()?.def;
    let target = crate::net::normalize_map(map);
    // The map being played (an arena slice: `map@id`, `current_entry`):
    // loaded again, as before (the match restarts).
    if target.eq_ignore_ascii_case(&current_entry(world)) {
        return None;
    }
    let hooks = world.get_resource::<NextMatchHooks>().map(|h| h.0.clone()).unwrap_or_default();
    // Checked before the hooks act (a new ladder for a match that can't
    // start there would be lost).
    let p = playable_on(world, def, Some(&target));
    if !p.ok() {
        let games = world.get_resource::<Games>().cloned();
        return Some(Err(format!(
            "not every player can play {} on {target}: {}",
            def.id,
            lacking_text(games.as_ref(), &p.lacking)
        )));
    }
    info!("changelevel {target}: a new match of {}", def.id);
    for hook in hooks {
        hook(world, &def.id, &target);
    }
    Some(change_match(world, &def.id, Some(&target)).map(Some))
}

/// `mp_maps_off`: the maps out of the rotation (comma-separated).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MapsOff(pub String);

impl MapsOff {
    /// Whether `map` (any form `normalize_map` takes; an arena slice
    /// `game:map@id`) is out: named, or a slice whose map is named, unless
    /// the slice is marked on (`+game:map@id`: kept on under a map that is
    /// off; a list without marks reads as before).
    pub fn has(&self, map: &str) -> bool {
        let map = crate::net::normalize_map(map);
        let parent = minigame::map_fit::entry_map(&map);
        let items: Vec<&str> = self.0.split(',').map(str::trim).filter(|m| !m.is_empty()).collect();
        if items.iter().any(|m| m.strip_prefix('+').is_some_and(|s| s.eq_ignore_ascii_case(&map))) {
            return false;
        }
        items
            .iter()
            .filter(|m| !m.starts_with('+'))
            .map(|m| crate::net::normalize_map(m))
            .any(|m| m.eq_ignore_ascii_case(&map) || m.eq_ignore_ascii_case(parent))
    }
}

/// `def`'s rotation pool without the maps a host took out
/// (`mp_maps_off`); all of it when that would leave none. In a party app
/// (`arena::SlicesInPool`) each map's arena slices follow it
/// (`arena::with_slices`).
pub fn rotation_pool(world: &World, def: &MinigameDef) -> Vec<String> {
    let maps: Vec<String> = def.maps.iter().map(|m| crate::net::normalize_map(m)).collect();
    let pool = minigame::arena::with_slices(world, &maps);
    let Some(off) = world.get_resource::<MapsOff>().filter(|o| !o.0.trim().is_empty()) else { return pool };
    let kept: Vec<String> = pool.iter().filter(|m| !off.has(m)).cloned().collect();
    if kept.is_empty() { pool } else { kept }
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
    let here = current_entry(world);
    if !def.rotate {
        return here;
    }
    let pool = rotation_pool(world, def);
    let ok: Vec<String> = pool.iter().filter(|m| playable_on(world, def, Some(m)).ok()).cloned().collect();
    // By the players present (`minigame::map_fit`): the maps too big for
    // them out, the rest drawn by weight.
    let weights = minigame::map_fit::draw_weights(world, def, &pool);
    let mut history = world.remove_resource::<MapHistory>().unwrap_or_default();
    // The host's rotation settings (`minigame::overrides`) count from now.
    let rule = minigame::overrides::effective(world, def);
    let chosen = history.next_weighted(&rule, &pool, &here, &|m| ok.iter().any(|o| o == m), &|m| {
        weight_of(&pool, weights.as_deref(), m)
    });
    world.insert_resource(history);
    chosen.unwrap_or(here)
}

/// Map `m`'s weight in `pool`'s draw (`weights`, None: every map 1).
fn weight_of(pool: &[String], weights: Option<&[f64]>, m: &str) -> f64 {
    match weights {
        None => 1.0,
        Some(w) => pool.iter().position(|p| p == m).map_or(0.0, |i| w[i]),
    }
}

/// A host's `changelevel` with no map while a minigame plays: its next
/// match as a rotation's (`next_match`: the pool drawn for the players
/// present, then the app's hooks and the hold). None: a client, no
/// minigame, or a match's map change under way. This part's
/// `net::HostNextLevel`.
pub fn host_next_level(world: &mut World) -> Option<Result<Option<String>, String>> {
    if world.get_resource::<NetRole>() == Some(&NetRole::Client) || world.contains_resource::<MapChange>() {
        return None;
    }
    world.get_resource::<ActiveMinigame>()?;
    let map = next_match(world)?;
    Some(Ok(Some(format!("changelevel: the next match on {map}"))))
}

/// The playing minigame's pool at `players` (else the characters
/// present), a line per map (`map_fit::MapPool::lines`), for the console.
fn pool_text(world: &mut World, players: Option<usize>) -> Result<String, String> {
    let def = world.get_resource::<ActiveMinigame>().ok_or("no minigame is playing")?.def;
    let settings = minigame::map_fit::FitSettings::of(def)?
        .ok_or_else(|| format!("{} draws its maps without the count (map_fit off)", def.id))?;
    let n = players.unwrap_or_else(|| minigame::map_fit::present(world));
    let maps: Vec<String> = def.maps.iter().map(|m| crate::net::normalize_map(m)).collect();
    let pool = minigame::arena::with_slices(world, &maps);
    let maps = world.get_resource::<minigame::map_fit::MapPool>().cloned().unwrap_or_default();
    Ok(maps.lines(&pool, n, &settings))
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
