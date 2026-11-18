//! CS:S's rules over the network (docs/plans/active/multiplayer.md, slice
//! 5): the game's part of the network (`net::NetAppExt::net_part`,
//! registered by `CsSourceNetPlugin`), registered after the engine's own
//! protocol. Everything here is the server's, and a client shows it.
//!
//! - **The round** (`NetRound`, one replicated entity): phase, its times,
//!   the score, whether buying is open, the bomb's outcome and the
//!   hostages' tally. A client writes them into the same resources single
//!   player has (`rules::rounds::RoundState`, `weapon::economy::BuyWindow`,
//!   `objectives::bomb::BombState`, `HostageTally`), times moved onto its
//!   own clock (`net::game::ServerTime`), so the HUD, the scoreboard and
//!   the spectator bars read them as they always did. A round's end comes
//!   as `RoundOver` (the banner and the announcer).
//! - **Requests**: joining a team (`TeamRequest`) or a class
//!   (`ClassRequest`) is the server's to check (`mp_limitteams`); a
//!   refusal comes back as `net::Notice`, the game's hint.
//! - **Objectives**: the planted bomb (`NetBomb`: where, its timer, who
//!   defuses), hostages as characters (`NetHostage`), what happens as
//!   `ObjectiveNews` (the HUD's words); a player's automatic hints
//!   (`rules::hints`) to its client as `HintNews`.
//! - **Hooks** the network calls (`net::parts`): the join flow
//!   (`net::JoinRules`: `rules::JoinChoice`, `hold_out`,
//!   `make_unassigned`, `join_spectators`, `enter_game`), bots
//!   (`net::BotHooks`: `bot::kick_bot` with the quota one lower,
//!   `bot::sync_quota`), the holds a client predicts (`net::Holds`: the
//!   freeze's end, `rules::rounds::frozen_intent`; arming and defusing,
//!   `objectives::bomb::BombHold`, before `net::predict::RecordCommand`),
//!   and what goes when serving stops (`net::NetHooks`: the round's
//!   entity, `NetBomb`, `NetHostage`).
//! - **Voice** (`net::voice::VoiceRules::can_hear`): who hears whom,
//!   `hears_voice`: `sv_alltalk 1` everyone; 0 (CS:S's default) the
//!   team, a dead talker only by the dead (spectators a team of their
//!   own).
//!
//! Known shortcuts (docs/tech-debt.md): Voice's hearing rule
//! (`game::net::hears_voice`): `sv_alltalk`'s default and help are measured
//! (probe server), the dead-talk-to-the-dead rule is CS:S's documented
//! behaviour, unconfirmed with two clients; unassigned players are a team of
//! their own (ours).
//!
//! Scores, buying and the rules' sounds are every game's (`net::game`).

use bevy::prelude::*;
use crate::gameplay::GameplayAppExt as _;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    core::{NetRole, SimClock, Team},
    map::{
        BodyName, BodySkin,
        loose::{ShownItem, SitOnOrigin},
    },
    net::{
        BotHooks, Holds, JoinRules, NetAppExt, NetCharacter, NetHooks,
        game::{ServerTime, notify, tick_of},
        ordered::OrderedAppExt,
        predict::RecordCommand,
        server::Player,
    },
    objectives::{
        ObjectiveEvent, RoundOpen,
        bomb::{BombHold, BombOutcome, BombRules, BombState, Defuse, PlantedBomb},
        hostages::{Hostage, HostagePenalty, HostageRules, HostageTally},
    },
    rules::{
        JoinChoice,
        hints::AutoHint,
        rounds::{FunFact, Mvp, Phase, RoundEndReason, RoundEnded, RoundState, RoundSummary},
    },
    weapon::{
        Armor,
        economy::{BuyWindow, DefuseKit, Money, NightVision},
    },
};

/// CS:S's part of the network (`register`), named `cs_source`.
pub struct CsSourceNetPlugin;

impl Plugin for CsSourceNetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AllTalk>().net_part("cs_source", register);
        crate::console::resource_cvar::<AllTalk, u8>(
            app,
            "sv_alltalk",
            "Players can hear all other players, no team restrictions.",
            |a| &mut a.0,
        );
        super::votes::register(app);
    }
}

/// `sv_alltalk`: 1 lets everyone hear everyone's voice; 0 (CS:S's
/// default, measured on the probe server 2026-10-15) keeps it within the
/// team (`hears_voice`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AllTalk(pub u8);

/// Whether a listener hears a talker's voice, CS:S's way: with
/// `sv_alltalk` everyone; else a teammate's, and a dead talker's only
/// when dead too (a living talker is heard by the whole team, the dead
/// included). Teams as `voice_team` gives them.
pub fn hears_voice(
    alltalk: bool,
    listener_alive: bool,
    listener_team: Option<u8>,
    talker_alive: bool,
    talker_team: Option<u8>,
) -> bool {
    alltalk || (listener_team.is_some() && listener_team == talker_team && (talker_alive || !listener_alive))
}

/// A character's team for voice: its `Team`, spectators and players who
/// haven't picked one each a team of their own (as Source's spectator and
/// unassigned teams).
pub fn voice_team(world: &World, e: Entity) -> Option<u8> {
    if world.get::<crate::core::Spectating>(e).is_some() {
        Some(u8::MAX)
    } else if world.get::<crate::core::Unassigned>(e).is_some() {
        Some(u8::MAX - 1)
    } else {
        world.get::<Team>(e).map(|t| t.0)
    }
}

/// What the HUD shows of a client's own player, beside its predicted
/// state (`net::parts::OwnParts`): money, armour (amount, helmet), the
/// defusal kit and night vision goggles. Money is shown only to its owner,
/// as in CS:S.
fn own_parts(app: &mut App) {
    fn flag(has: bool) -> Option<Vec<u8>> {
        has.then(Vec::new)
    }
    fn set<C: Bundle>(world: &mut World, e: Entity, on: bool, value: C) {
        if let Ok(mut e) = world.get_entity_mut(e) {
            if on {
                e.insert(value);
            } else {
                e.remove::<C>();
            }
        }
    }
    app.own_part(
        "cs_source.money",
        |w, e| w.get::<Money>(e).map(|m| m.0.to_le_bytes().to_vec()),
        |w, e, b| match b.and_then(|b| Some(u32::from_le_bytes(b.try_into().ok()?))) {
            Some(m) => set(w, e, true, Money(m)),
            None => set(w, e, false, Money(0)),
        },
    )
    .own_part(
        "cs_source.armor",
        |w, e| w.get::<Armor>(e).map(|a| [a.amount.to_le_bytes().as_slice(), &[a.helmet as u8]].concat()),
        |w, e, b| match b.and_then(|b| {
            let (amount, helmet) = b.split_first_chunk::<4>().and_then(|(a, h)| Some((a, *h.first()?)))?;
            Some((f32::from_le_bytes(*amount), helmet != 0))
        }) {
            Some((amount, helmet)) => set(w, e, true, Armor { amount, helmet }),
            None => set(w, e, false, Armor::default()),
        },
    )
    .own_part(
        "cs_source.kit",
        |w, e| flag(w.get::<DefuseKit>(e).is_some()),
        |w, e, b| set(w, e, b.is_some(), DefuseKit),
    )
    .own_part(
        "cs_source.nightvision",
        |w, e| flag(w.get::<NightVision>(e).is_some()),
        |w, e, b| set(w, e, b.is_some(), NightVision),
    );
}

/// `net::voice::VoiceRules::can_hear`: `hears_voice` on the server's
/// characters.
fn can_hear(world: &World, listener: Entity, talker: Entity) -> bool {
    let alltalk = world.get_resource::<AllTalk>().is_some_and(|a| a.0 != 0);
    let alive = |e| crate::net::voice::alive(world, e);
    hears_voice(alltalk, alive(listener), voice_team(world, listener), alive(talker), voice_team(world, talker))
}

/// The protocol (in this order on every side), the systems and the hooks.
fn register(app: &mut App) {
    own_parts(app);
    app.replicate::<NetRound>()
        .ordered::<NetRound>()
        .replicate::<NetBomb>()
        .ordered::<NetBomb>()
        .replicate::<NetHostage>()
        .ordered::<NetHostage>()
        .add_client_message::<TeamRequest>(Channel::Ordered)
        .add_client_message::<ClassRequest>(Channel::Ordered)
        .add_mapped_server_message::<ObjectiveNews>(Channel::Ordered)
        .add_mapped_server_message::<RoundOver>(Channel::Ordered)
        .add_server_message::<HintNews>(Channel::Ordered)
        .make_message_independent::<HintNews>()
        .add_message::<crate::rules::TeamRequested>()
        .add_message::<crate::rules::ClassRequested>()
        .add_message::<RoundEnded>()
        .add_message::<ObjectiveEvent>()
        .add_message::<HostagePenalty>()
        // CS:S's hooks, used while it is the gameplay's (its bots' for
        // `BotHooks`).
        .game_hook(
            super::GAME,
            JoinRules {
                joined,
                enter: crate::rules::enter_game,
                choice: |w| w.get_resource::<JoinChoice>().copied().unwrap_or_default().key(),
            },
        )
        .game_hook(
            super::GAME,
            BotHooks {
                kick: |w, bot| {
                    crate::bot::kick_bot(w, bot);
                    if let Some(mut q) = w.get_resource_mut::<crate::bot::BotQuota>() {
                        q.quota = q.quota.saturating_sub(1);
                    }
                },
                sync_quota: crate::bot::sync_quota,
                quota: |w| w.get_resource::<crate::bot::BotQuota>().map_or(0, |q| q.quota),
            },
        )
        .game_hook(
            super::GAME,
            Holds {
                freeze_ends: |w| match w.get_resource::<RoundState>().map(|r| r.phase) {
                    Some(Phase::Freeze { until }) => Some(until),
                    _ => None,
                },
                frozen: crate::rules::rounds::frozen_intent,
            },
        )
        // Arming and defusing hold the player as predicted, before its
        // command is recorded.
        .configure_sets(FixedUpdate, BombHold.before(RecordCommand));
    {
        let mut hooks = app.world_mut().resource_mut::<NetHooks>();
        hooks.stopped_serving.push(stopped_serving);
    }
    app.world_mut().resource_mut::<crate::net::voice::VoiceRules>().can_hear = can_hear;
    let server = || resource_equals(NetRole::Server);
    let client = || resource_equals(NetRole::Client);
    // The match's state: while CS:S is the gameplay (`objectives::plays`).
    app.add_systems(
        FixedLast,
        (write_round, write_bombs, write_hostages).run_if(server()).run_if(crate::objectives::plays),
    )
    .add_systems(
        PreUpdate,
        (receive_teams, receive_classes)
            .after(ServerSystems::Receive)
            .run_if(in_state(ServerState::Running))
            .run_if(crate::objectives::plays),
    )
    .add_systems(
        PostUpdate,
        (replicate_hostages, send_round_over, send_objectives, send_hints)
            .before(ServerSystems::Send)
            .run_if(server())
            .run_if(crate::objectives::plays),
    )
    .add_systems(
        PreUpdate,
        (apply_round, apply_bombs, apply_hostages, receive_round_over, receive_objectives, receive_hints)
            .chain()
            .after(ClientSystems::Receive)
            .after(crate::net::predict::rebuild_own_states)
            .run_if(client())
            .run_if(crate::objectives::plays),
    )
    .add_systems(
        PostUpdate,
        (send_teams, send_classes)
            .before(ClientSystems::Send)
            .run_if(client())
            .run_if(crate::objectives::plays),
    );
}

/// A remote player's character has just spawned (`net::JoinRules::joined`):
/// out of the game until its client has the map, on the team (or the join
/// menus) its client asked for (`JoinChoice`, its userinfo).
fn joined(world: &mut World, e: Entity, choice: Option<&str>) {
    let choice = choice.and_then(JoinChoice::from_key).unwrap_or_default();
    crate::rules::hold_out(world, e);
    match choice {
        // No team until it picks one (its client's join menus).
        JoinChoice::Menus => crate::rules::make_unassigned(world, e),
        JoinChoice::Team(t) if t.0 == 0 => {
            if crate::rules::join_spectators(world, e).is_err() {
                crate::rules::make_unassigned(world, e);
            }
        }
        JoinChoice::Team(t) => {
            world.entity_mut(e).insert(t);
        }
        JoinChoice::Auto => {}
    }
}

/// Serving stopped: the round's entity goes, the bomb and the hostages
/// stop replicating (`net::NetHooks::stopped_serving`).
fn stopped_serving(world: &mut World) {
    let replicated: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<NetBomb>, With<NetHostage>)>>()
        .iter(world)
        .collect();
    for e in replicated {
        world.entity_mut(e).remove::<(NetBomb, NetHostage)>();
    }
    let rounds: Vec<Entity> = world.query_filtered::<Entity, With<NetRound>>().iter(world).collect();
    for e in rounds {
        world.despawn(e);
    }
}

// --- Protocol

/// The round and the objectives' totals, on one replicated entity the
/// server keeps (`write_round`). Times are server ticks
/// (fractional), which a client turns into its own clock
/// (`net::game::ServerTime`).
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetRound {
    /// `round_phase::*`.
    pub phase: u8,
    /// Live: when it went live. Freeze: when it ends. Live: when time is
    /// up. Over: when the next round starts.
    pub since: f64,
    pub until: f64,
    /// Over: the winner's team number (None: a draw).
    pub winner: Option<u8>,
    pub number: u32,
    /// Rounds won by the attackers and the defenders.
    pub wins: [u32; 2],
    /// Why buying is closed now (the rules' words); None: open.
    pub buy_closed: Option<String>,
    /// This round's bomb: `bomb_outcome::*`.
    pub bomb: u8,
    /// Hostages this round: total, rescued, killed.
    pub hostages: [u32; 3],
    /// The game on the map is over: the intermission (`RoundState::game_over`).
    pub game_over: bool,
    /// When the game on this map began (`RoundState::map_started`), for
    /// the time left with `mp_timelimit`.
    pub map_started: Option<f64>,
}

/// `NetRound::phase`.
pub mod round_phase {
    pub const OFF: u8 = 0;
    pub const FREEZE: u8 = 1;
    pub const LIVE: u8 = 2;
    pub const OVER: u8 = 3;
}

/// `NetRound::bomb`.
pub mod bomb_outcome {
    pub const NONE: u8 = 0;
    pub const PLANTED: u8 = 1;
    pub const EXPLODED: u8 = 2;
    pub const DEFUSED: u8 = 3;
}

/// The planted bomb (`objectives::bomb::PlantedBomb`), on its own
/// replicated entity: where, its model, and its clock in server ticks.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetBomb {
    pub origin: [f32; 3],
    pub rotation: [f32; 4],
    pub model: Option<String>,
    pub site: Option<u32>,
    pub planted: f64,
    pub explode: f64,
    pub timer: f32,
    /// Who defuses it, since and until when (ticks), with a kit.
    #[entities]
    pub defuser: Option<Entity>,
    pub defuse_since: f64,
    pub defuse_until: f64,
    pub defuse_kit: bool,
    pub defused: bool,
}

/// A hostage (`objectives::hostages::Hostage`): which one, its model and
/// whom it follows. With `NetCharacter` and `NetBody` like anyone.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetHostage {
    pub index: u32,
    pub model: Option<String>,
    /// Its skin family (`map::BodySkin`).
    pub skin: i32,
    #[entities]
    pub leader: Option<Entity>,
}

/// Client -> server: put me on this team (`jointeam`: our team number,
/// 1 terrorists, 2 counter-terrorists; 0 the spectators).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct TeamRequest {
    pub team: u8,
}

/// Client -> server: my class (`joinclass`: CS:S's number, 0
/// auto-select; `rules::join_class`).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ClassRequest {
    pub class: u8,
}


/// Server -> a client: an automatic hint for its player
/// (`rules::hints::AutoHint`: the game's localization token, which the
/// client reads from its own install).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct HintNews {
    pub token: String,
}

/// Server -> clients: an objective event (`objectives::ObjectiveEvent`,
/// `hostages::HostagePenalty`) for the HUD's words and sounds: its kind
/// (`encode_objective` numbers them), who, the other party, where, and the rest.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct ObjectiveNews {
    pub kind: u8,
    #[entities]
    pub who: Option<Entity>,
    #[entities]
    pub other: Option<Entity>,
    pub at: [f32; 3],
    pub flag: bool,
    pub site: Option<u32>,
    pub amount: f32,
    pub reason: u8,
}

/// Server -> clients: the round ended (`rules::rounds::RoundEnded`):
/// the winner's team number and why (`REASONS` numbers them), and the
/// win panel's summary (`rules::rounds::RoundSummary`): the MVP and why
/// (`Mvp::ALL`'s index), the fun fact (its string token, the player it
/// names, its values) and the round's live seconds.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct RoundOver {
    pub winner: Option<u8>,
    pub reason: u8,
    #[entities]
    pub mvp: Option<Entity>,
    pub mvp_reason: u8,
    pub fun_fact: Option<String>,
    #[entities]
    pub fun_fact_player: Option<Entity>,
    pub fun_fact_values: Vec<String>,
    pub seconds: f32,
}


// --- Server

/// The round, the buy window, the bomb and the hostages into `NetRound`.
fn write_round(world: &mut World) {
    let clock = *world.resource::<SimClock>();
    let t = |s: f64| tick_of(&clock, s);
    let state = world.get_resource::<RoundState>().cloned().unwrap_or_default();
    let (phase, since, until, winner) = match state.phase {
        Phase::Off => (round_phase::OFF, 0.0, 0.0, None),
        Phase::Freeze { until } => (round_phase::FREEZE, 0.0, t(until), None),
        Phase::Live { since, ends } => (round_phase::LIVE, t(since), t(ends), None),
        Phase::Over { until, winner } => (round_phase::OVER, 0.0, t(until), winner.map(|w| w.0)),
    };
    let bomb = world.get_resource::<BombState>().cloned().unwrap_or_default();
    let tally = world.get_resource::<HostageTally>().copied().unwrap_or_default();
    let now = NetRound {
        phase,
        since,
        until,
        winner,
        number: state.number,
        wins: state.wins,
        buy_closed: world.get_resource::<BuyWindow>().and_then(|b| b.0.clone().err()),
        bomb: match (bomb.outcome, bomb.planted) {
            (Some(BombOutcome::Exploded), _) => bomb_outcome::EXPLODED,
            (Some(BombOutcome::Defused), _) => bomb_outcome::DEFUSED,
            (None, Some(_)) => bomb_outcome::PLANTED,
            (None, None) => bomb_outcome::NONE,
        },
        hostages: [tally.total, tally.rescued, tally.killed],
        game_over: state.game_over.is_some(),
        // Whole ticks, so it doesn't change with the clock's rounding.
        map_started: state.map_started.map(|s| t(s).round()),
    };
    let have = world.query::<(Entity, &NetRound)>().iter(world).next().map(|(e, r)| (e, r.clone()));
    match have {
        Some((e, r)) => {
            if r != now {
                world.entity_mut(e).insert(now);
            }
        }
        None => {
            world.spawn((Name::new("Round"), Replicated, now));
        }
    }
}

/// The planted bomb into `NetBomb` (replicated from the plant on).
#[allow(clippy::type_complexity)]
fn write_bombs(
    mut q: Query<(Entity, &PlantedBomb, &Transform, Option<&ShownItem>, Option<&mut NetBomb>)>,
    replicated: Query<(), With<Replicated>>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let t = |s: f64| tick_of(&clock, s);
    for (e, b, tf, model, have) in &mut q {
        let defuse = b.defuse.filter(|d| replicated.contains(d.who));
        let now = NetBomb {
            origin: tf.translation.to_array(),
            rotation: tf.rotation.to_array(),
            model: model.map(|m| m.0.clone()),
            site: b.site.map(|s| s as u32),
            planted: t(b.planted_at),
            explode: t(b.explode_at),
            timer: b.timer,
            defuser: defuse.map(|d| d.who),
            defuse_since: defuse.map_or(0.0, |d| t(d.started)),
            defuse_until: defuse.map_or(0.0, |d| t(d.ends)),
            defuse_kit: defuse.is_some_and(|d| d.kit),
            defused: b.defused,
        };
        match have {
            Some(mut h) => {
                h.set_if_neq(now);
            }
            None => {
                commands.entity(e).insert((Replicated, now));
            }
        }
    }
}

/// Whom each hostage follows.
fn write_hostages(mut q: Query<(&Hostage, &mut NetHostage)>, replicated: Query<(), With<Replicated>>) {
    for (h, mut n) in &mut q {
        let leader = h.leader.filter(|l| replicated.contains(*l));
        if n.leader != leader {
            n.leader = leader;
        }
    }
}

/// Hostages replicate as characters (`NetCharacter`, `NetBody`) with
/// `NetHostage`.
fn replicate_hostages(
    q: Query<(Entity, &Hostage, Option<&Name>, Option<&BodyName>, Option<&BodySkin>), Without<Replicated>>,
    mut commands: Commands,
) {
    for (e, h, name, body, skin) in &q {
        commands.entity(e).insert((
            Replicated,
            NetCharacter {
                owner: None,
                name: name.map_or_else(|| format!("Hostage {}", h.index + 1), |n| n.to_string()),
            },
            crate::net::NetBody::default(),
            NetHostage {
                index: h.index as u32,
                model: body.map(|b| b.0.clone()),
                skin: skin.map_or(0, |s| s.0),
                leader: None,
            },
        ));
    }
}

/// A client asked for a team (`rules::join_team`), or the spectators
/// (team 0: `rules::join_spectators`).
fn receive_teams(mut requests: MessageReader<FromClient<TeamRequest>>, players: Query<&Player>, mut commands: Commands) {
    for r in requests.read() {
        let Some(client) = r.client_id.entity() else { continue };
        let Ok(p) = players.get(client) else { continue };
        let (character, team) = (p.character, r.message.team);
        commands.queue(move |w: &mut World| {
            if team > 2 || w.get_entity(character).is_err() {
                return;
            }
            let joined = if team == 0 {
                crate::rules::join_spectators(w, character)
            } else {
                crate::rules::join_team(w, character, Team(team))
            };
            if let Err(text) = joined {
                notify(w, client, text);
            }
        });
    }
}

/// A client chose its class (`rules::join_class`).
fn receive_classes(mut requests: MessageReader<FromClient<ClassRequest>>, players: Query<&Player>, mut commands: Commands) {
    for r in requests.read() {
        let Some(client) = r.client_id.entity() else { continue };
        let Ok(p) = players.get(client) else { continue };
        let (character, class) = (p.character, r.message.class);
        commands.queue(move |w: &mut World| {
            if w.get_entity(character).is_err() {
                return;
            }
            if let Err(text) = crate::rules::join_class(w, character, class) {
                notify(w, client, text);
            }
        });
    }
}

/// Round ends, to every client.
fn send_round_over(
    mut ended: MessageReader<RoundEnded>,
    replicated: Query<(), With<Replicated>>,
    mut out: MessageWriter<ToClients<RoundOver>>,
) {
    for e in ended.read() {
        let s = &e.summary;
        let known = |p: Option<Entity>| p.filter(|p| replicated.contains(*p));
        let mvp = s.mvp.filter(|(p, _)| replicated.contains(*p));
        out.write(ToClients {
            targets: SendTargets::CLIENTS_ONLY,
            message: RoundOver {
                winner: e.winner.map(|t| t.0),
                reason: REASONS.iter().position(|r| *r == e.reason).unwrap_or(0) as u8,
                mvp: mvp.map(|(p, _)| p),
                mvp_reason: mvp.map_or(0, |(_, why)| Mvp::ALL.iter().position(|m| *m == why).unwrap_or(0) as u8),
                fun_fact: s.fun_fact.as_ref().map(|f| f.token.clone()),
                fun_fact_player: s.fun_fact.as_ref().and_then(|f| known(f.player)),
                fun_fact_values: s.fun_fact.as_ref().map_or_else(Vec::new, |f| f.values.clone()),
                seconds: s.seconds,
            },
        });
    }
}

/// Objective events and hostage penalties, to every client (each client
/// shows what concerns it, as single player does for the local player).
fn send_objectives(
    mut events: MessageReader<ObjectiveEvent>,
    mut penalties: MessageReader<HostagePenalty>,
    replicated: Query<(), With<Replicated>>,
    mut out: MessageWriter<ToClients<ObjectiveNews>>,
) {
    let known = |e: Entity| replicated.contains(e).then_some(e);
    let news = events
        .read()
        .filter_map(|e| encode_objective(e, &known))
        .chain(penalties.read().filter_map(|p| {
            let (kind, who) = match p {
                HostagePenalty::Warning { who } => (PENALTY_WARNING, who),
                HostagePenalty::Removed { who } => (PENALTY_REMOVED, who),
            };
            Some(ObjectiveNews {
                kind,
                who: Some(known(*who)?),
                ..default()
            })
        }))
        .collect::<Vec<_>>();
    for message in news {
        out.write(ToClients {
            targets: SendTargets::CLIENTS_ONLY,
            message,
        });
    }
}

// --- Encoding

const REASONS: [RoundEndReason; 8] = [
    RoundEndReason::Eliminated,
    RoundEndReason::TimeRanOut,
    RoundEndReason::Draw,
    RoundEndReason::TargetBombed,
    RoundEndReason::BombDefused,
    RoundEndReason::TargetSaved,
    RoundEndReason::HostagesRescued,
    RoundEndReason::HostagesNotRescued,
];

/// Why planting or defusing was refused: the game's words
/// (`objectives::bomb`), by number.
const REFUSALS: [&str; 4] = [
    "C4 must be planted at a bomb site.",
    "You must be standing on the ground to plant the C4.",
    "The bomb is already being defused.",
    "You must be on the ground to defuse the bomb.",
];

const PENALTY_WARNING: u8 = 100;
const PENALTY_REMOVED: u8 = 101;

fn reason_number(r: &str) -> u8 {
    REFUSALS.iter().position(|x| *x == r).unwrap_or(0) as u8
}

/// An objective event as news, with only entities clients know.
fn encode_objective(e: &ObjectiveEvent, known: &dyn Fn(Entity) -> Option<Entity>) -> Option<ObjectiveNews> {
    use ObjectiveEvent as E;
    let n = |kind: u8, who: Entity| -> Option<ObjectiveNews> {
        Some(ObjectiveNews {
            kind,
            who: Some(known(who)?),
            ..default()
        })
    };
    Some(match e {
        E::GotBomb { who } => n(0, *who)?,
        E::DroppedBomb { who } => n(1, *who)?,
        E::PickedUpBomb { who } => n(2, *who)?,
        E::BeginPlant { who } => n(3, *who)?,
        E::AbortPlant { who, left_zone } => ObjectiveNews {
            flag: *left_zone,
            ..n(4, *who)?
        },
        E::PlantRefused { who, reason } => ObjectiveNews {
            reason: reason_number(reason),
            ..n(5, *who)?
        },
        E::Planted { who, at, site } => ObjectiveNews {
            kind: 6,
            who: known(*who),
            at: at.to_array(),
            site: site.map(|s| s as u32),
            ..default()
        },
        E::BeginDefuse { who, kit } => ObjectiveNews { flag: *kit, ..n(7, *who)? },
        E::AbortDefuse { who } => n(8, *who)?,
        E::DefuseRefused { who, reason } => ObjectiveNews {
            reason: reason_number(reason),
            ..n(9, *who)?
        },
        E::Defused { who } => ObjectiveNews {
            kind: 10,
            who: known(*who),
            ..default()
        },
        E::Exploded { at } => ObjectiveNews {
            kind: 11,
            at: at.to_array(),
            ..default()
        },
        E::Beep { at } => ObjectiveNews {
            kind: 12,
            at: at.to_array(),
            ..default()
        },
        E::PickedUpKit { who } => n(13, *who)?,
        E::HostageFollows { hostage, leader, first } => ObjectiveNews {
            other: Some(known(*leader)?),
            flag: *first,
            ..n(14, *hostage)?
        },
        E::HostageStops { hostage, leader } => ObjectiveNews {
            other: Some(known(*leader)?),
            ..n(15, *hostage)?
        },
        E::HostageRefused { who } => n(16, *who)?,
        E::HostageRescued { hostage, leader } => ObjectiveNews {
            other: leader.and_then(known),
            ..n(17, *hostage)?
        },
        E::HostageHurt {
            hostage,
            attacker,
            amount,
        } => ObjectiveNews {
            other: attacker.and_then(known),
            amount: *amount,
            ..n(18, *hostage)?
        },
        E::HostageKilled { hostage, attacker } => ObjectiveNews {
            other: attacker.and_then(known),
            ..n(19, *hostage)?
        },
    })
}

/// News back into the event single player has (an entity a client
/// couldn't map is `Entity::PLACEHOLDER`, never anyone's).
enum Decoded {
    Event(ObjectiveEvent),
    Penalty(HostagePenalty),
}

fn decode_objective(n: &ObjectiveNews) -> Option<Decoded> {
    use ObjectiveEvent as E;
    let who = n.who.unwrap_or(Entity::PLACEHOLDER);
    let at = Vec3::from_array(n.at);
    let reason = REFUSALS[(n.reason as usize).min(REFUSALS.len() - 1)];
    Some(Decoded::Event(match n.kind {
        0 => E::GotBomb { who },
        1 => E::DroppedBomb { who },
        2 => E::PickedUpBomb { who },
        3 => E::BeginPlant { who },
        4 => E::AbortPlant {
            who,
            left_zone: n.flag,
        },
        5 => E::PlantRefused { who, reason },
        6 => E::Planted {
            who,
            at,
            site: n.site.map(|s| s as usize),
        },
        7 => E::BeginDefuse { who, kit: n.flag },
        8 => E::AbortDefuse { who },
        9 => E::DefuseRefused { who, reason },
        10 => E::Defused { who },
        11 => E::Exploded { at },
        12 => E::Beep { at },
        13 => E::PickedUpKit { who },
        14 => E::HostageFollows {
            hostage: who,
            leader: n.other?,
            first: n.flag,
        },
        15 => E::HostageStops {
            hostage: who,
            leader: n.other?,
        },
        16 => E::HostageRefused { who },
        17 => E::HostageRescued {
            hostage: who,
            leader: n.other,
        },
        18 => E::HostageHurt {
            hostage: who,
            attacker: n.other,
            amount: n.amount,
        },
        19 => E::HostageKilled {
            hostage: who,
            attacker: n.other,
        },
        PENALTY_WARNING => return Some(Decoded::Penalty(HostagePenalty::Warning { who })),
        PENALTY_REMOVED => return Some(Decoded::Penalty(HostagePenalty::Removed { who })),
        _ => return None,
    }))
}

// --- Client

/// The server's round into the resources the HUD reads.
fn apply_round(world: &mut World) {
    let Some(r) = world.query::<&NetRound>().iter(world).next().cloned() else {
        return;
    };
    let Some(time) = ServerTime::of(world) else { return };
    let t = |tick: f64| time.local(tick);
    let phase = match r.phase {
        round_phase::FREEZE => Phase::Freeze { until: t(r.until) },
        round_phase::LIVE => Phase::Live {
            since: t(r.since),
            ends: t(r.until),
        },
        round_phase::OVER => Phase::Over {
            until: t(r.until),
            winner: r.winner.map(Team),
        },
        _ => Phase::Off,
    };
    {
        let mut state = world.get_resource_or_init::<RoundState>();
        state.phase = phase;
        if state.number != r.number {
            state.number = r.number;
        }
        if state.wins != r.wins {
            state.wins = r.wins;
        }
        let over = r.game_over.then_some(f64::INFINITY);
        if state.game_over != over {
            state.game_over = over;
        }
        // The map's clock (`timeleft`, the scoreboard's time left).
        state.map_started = r.map_started.map(t);
    }
    let window = BuyWindow(r.buy_closed.clone().map_or(Ok(()), Err));
    if world.get_resource::<BuyWindow>().is_none_or(|w| w.0 != window.0) {
        world.insert_resource(window);
    }
    let frozen = r.phase == round_phase::FREEZE || r.game_over;
    if world.get_resource::<crate::core::FreezeTime>().is_none_or(|f| f.0 != frozen) {
        world.insert_resource(crate::core::FreezeTime(frozen));
    }
    let open = r.phase == round_phase::LIVE || r.phase == round_phase::OFF;
    if world.get_resource::<RoundOpen>().is_none_or(|o| o.0 != open) {
        world.insert_resource(RoundOpen(open));
    }
    let planted = world.query_filtered::<Entity, With<NetBomb>>().iter(world).next();
    let bomb = BombState {
        planted: planted.filter(|_| r.bomb == bomb_outcome::PLANTED),
        planter: None,
        outcome: match r.bomb {
            bomb_outcome::EXPLODED => Some(BombOutcome::Exploded),
            bomb_outcome::DEFUSED => Some(BombOutcome::Defused),
            _ => None,
        },
    };
    if world.get_resource::<BombState>() != Some(&bomb) {
        world.insert_resource(bomb);
    }
    let tally = HostageTally {
        total: r.hostages[0],
        rescued: r.hostages[1],
        killed: r.hostages[2],
    };
    if world.get_resource::<HostageTally>() != Some(&tally) {
        world.insert_resource(tally);
    }
}

/// The planted bomb: drawn with its model where the server put it, its
/// clock on ours.
fn apply_bombs(world: &mut World) {
    let Some(time) = ServerTime::of(world) else { return };
    let turn = world.get_resource::<BombRules>().map_or(Quat::IDENTITY, |r| r.planted_turn);
    let bombs: Vec<(Entity, NetBomb, bool)> = world
        .query::<(Entity, &NetBomb, Has<PlantedBomb>)>()
        .iter(world)
        .map(|(e, b, p)| (e, b.clone(), p))
        .collect();
    for (e, b, had) in bombs {
        let t = |tick: f64| time.local(tick);
        let defuse = b
            .defuser
            .filter(|d| *d != Entity::PLACEHOLDER && world.get_entity(*d).is_ok())
            .map(|who| Defuse {
                who,
                started: t(b.defuse_since),
                ends: t(b.defuse_until),
                kit: b.defuse_kit,
            });
        let planted = PlantedBomb {
            planter: None,
            site: b.site.map(|s| s as usize),
            planted_at: t(b.planted),
            explode_at: t(b.explode),
            timer: b.timer,
            next_beep: 0.0,
            // The server's: it thinks.
            next_think: f64::INFINITY,
            defuse,
            defused: b.defused,
        };
        let pose =
            Transform::from_translation(Vec3::from_array(b.origin)).with_rotation(Quat::from_array(b.rotation).normalize());
        let mut ent = world.entity_mut(e);
        ent.insert(planted);
        if !had {
            ent.insert((pose, crate::map::interp::NetDrawn));
            if let Some(m) = &b.model {
                ent.insert((ShownItem(m.clone()), SitOnOrigin(turn)));
            }
        } else if ent.get::<Transform>().is_some_and(|t| t.translation != pose.translation) {
            ent.insert(pose);
        }
    }
}

/// Hostages: characters without a team, with their model and leader (and
/// how bullets treat them, for the shots this client draws).
#[allow(clippy::type_complexity)]
fn apply_hostages(
    teams: Query<Entity, (With<NetHostage>, With<Team>)>,
    mut q: Query<(Entity, &NetHostage, Option<&mut Hostage>)>,
    rules: Option<Res<HostageRules>>,
    mut commands: Commands,
) {
    for e in &teams {
        commands.entity(e).remove::<Team>();
    }
    for (e, n, h) in &mut q {
        let leader = n.leader.filter(|l| *l != Entity::PLACEHOLDER);
        match h {
            Some(mut h) => {
                if h.leader != leader {
                    h.leader = leader;
                }
            }
            None => {
                let mut ent = commands.entity(e);
                ent.insert(Hostage::shown(n.index as usize, leader));
                if let Some(r) = &rules {
                    ent.insert(r.bullets);
                }
                if let Some(m) = &n.model {
                    ent.insert(BodyName(m.clone()));
                }
                if n.skin != 0 {
                    ent.insert(BodySkin(n.skin));
                }
            }
        }
    }
}

/// The round's end: the banner and the announcer (`RoundEnded`).
fn receive_round_over(mut over: MessageReader<RoundOver>, mut ended: MessageWriter<RoundEnded>) {
    for o in over.read() {
        let real = |p: Option<Entity>| p.filter(|p| *p != Entity::PLACEHOLDER);
        ended.write(RoundEnded {
            winner: o.winner.map(Team),
            reason: REASONS[(o.reason as usize).min(REASONS.len() - 1)],
            summary: RoundSummary {
                mvp: real(o.mvp).map(|p| (p, Mvp::ALL[(o.mvp_reason as usize).min(Mvp::ALL.len() - 1)])),
                fun_fact: o.fun_fact.clone().map(|token| FunFact {
                    token,
                    player: real(o.fun_fact_player),
                    values: o.fun_fact_values.clone(),
                }),
                seconds: o.seconds,
            },
        });
    }
}

/// Objective news back into events (the HUD's words, the bomb's LED).
fn receive_objectives(
    mut news: MessageReader<ObjectiveNews>,
    mut events: MessageWriter<ObjectiveEvent>,
    mut penalties: MessageWriter<HostagePenalty>,
) {
    for n in news.read() {
        match decode_objective(n) {
            Some(Decoded::Event(e)) => {
                events.write(e);
            }
            Some(Decoded::Penalty(p)) => {
                penalties.write(p);
            }
            None => {}
        }
    }
}

/// A hint for a remote player, to its client (the local player's shows
/// here).
fn send_hints(
    mut hints: MessageReader<AutoHint>,
    players: Query<(Entity, &Player)>,
    mut out: MessageWriter<ToClients<HintNews>>,
) {
    for h in hints.read() {
        if let Some((client, _)) = players.iter().find(|(_, p)| p.character == h.to) {
            out.write(ToClients {
                targets: SendTargets::Single(ClientId::Client(client)),
                message: HintNews { token: h.token.clone() },
            });
        }
    }
}

/// The server's hints for our player.
fn receive_hints(
    mut news: MessageReader<HintNews>,
    local: Query<Entity, With<crate::core::LocalPlayer>>,
    mut out: MessageWriter<AutoHint>,
) {
    for n in news.read() {
        if let Some(to) = local.iter().next() {
            out.write(AutoHint {
                to,
                token: n.token.clone(),
            });
        }
    }
}

/// Our class, to the server.
fn send_classes(mut asked: MessageReader<crate::rules::ClassRequested>, mut out: MessageWriter<ClassRequest>) {
    for a in asked.read() {
        out.write(ClassRequest { class: a.0 });
    }
}

/// Our team changes, to the server.
fn send_teams(mut asked: MessageReader<crate::rules::TeamRequested>, mut out: MessageWriter<TeamRequest>) {
    for a in asked.read() {
        out.write(TeamRequest { team: a.0.0 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CS:S's voice rule: the team hears a living talker, only the dead
    /// hear a dead one, the enemy nobody; alltalk lets everyone hear.
    #[test]
    fn who_hears_whose_voice() {
        let (t, ct, spec) = (Some(1), Some(2), Some(u8::MAX));
        // (listener alive, team), (talker alive, team), heard
        let cases = [
            ((true, t), (true, t), true),
            ((false, t), (true, t), true),
            ((true, t), (false, t), false),
            ((false, t), (false, t), true),
            ((true, ct), (true, t), false),
            ((false, ct), (false, t), false),
            ((false, spec), (false, spec), true),
            ((false, spec), (true, t), false),
            ((true, None), (true, None), false),
        ];
        for ((la, lt), (ta, tt), heard) in cases {
            assert_eq!(hears_voice(false, la, lt, ta, tt), heard, "{la} {lt:?} hearing {ta} {tt:?}");
            assert!(hears_voice(true, la, lt, ta, tt));
        }
    }

    #[test]
    fn objective_news_round_trips() {
        let a = Entity::from_raw_u32(5).unwrap();
        let b = Entity::from_raw_u32(9).unwrap();
        let known = |e: Entity| Some(e);
        let events = [
            ObjectiveEvent::PlantRefused {
                who: a,
                reason: REFUSALS[1],
            },
            ObjectiveEvent::Planted {
                who: a,
                at: Vec3::new(1.0, 2.0, 3.0),
                site: Some(1),
            },
            ObjectiveEvent::BeginDefuse { who: b, kit: true },
            ObjectiveEvent::HostageFollows {
                hostage: a,
                leader: b,
                first: true,
            },
            ObjectiveEvent::HostageKilled {
                hostage: a,
                attacker: None,
            },
            ObjectiveEvent::Beep { at: Vec3::X },
        ];
        for e in events {
            let n = encode_objective(&e, &known).unwrap();
            match decode_objective(&n) {
                Some(Decoded::Event(back)) => assert_eq!(back, e),
                _ => panic!("{e:?} didn't come back"),
            }
        }
        // An entity clients don't know: the event isn't sent.
        let none = |_: Entity| None;
        assert!(encode_objective(&ObjectiveEvent::GotBomb { who: a }, &none).is_none());
    }
}
