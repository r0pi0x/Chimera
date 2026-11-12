//! Combat Arms over the network (docs/plans/active/combat-arms.md, "Network
//! part"): what the game adds to Chimera's (`net`), so a listen server or
//! a client joining one plays Combat Arms as single player does.
//!
//! Most of it is Chimera's and needs nothing here: characters, health,
//! teams and deaths replicated; each client predicting its own player
//! (movement, the inventory and its weapons' parts) and corrected from the
//! server's state; others' shots, swings and held weapons sent to the
//! other clients; lag compensation's history; the default join flow
//! (`net::parts::enter_at_spawn`: on the smaller team, in at a free spawn
//! point once the map is loaded).
//!
//! What the game's parts register themselves, so prediction runs them on a
//! client (with or without this plugin):
//!
//! - `movement`: `CaMovement`, the whole movement state, as a networked
//!   predicted component;
//! - `weapons`: the spread meters (`CaSpread`), the zoom (`CaZoom`; the
//!   owner's `Zoomed` and sway on its `ViewPunch` are Chimera's predicted
//!   parts) and a minigun's spin (`CaSpin`) as predicted weapon parts
//!   (`weapon::NetPart`), beside Chimera's (timers, the firing chain,
//!   burst, clip, fire mode: `WeaponState`, `Magazine`, `AltModes`); the
//!   player's view kick (`CaKick`) as a networked predicted component; the
//!   held weapon's movement penalty in the predicted

//!   tick; the server's rounds and knife swings lag compensated
//!   (`weapon::lagcomp`, the shooter's view tick); a client deals no damage;
//! - `inventory`: the selection (`CaSelection`: class cursors, last
//!   chosen, Q, the sprint pose; weapons as inventory indices) and the
//!   knife's attack (`Knife`'s `KnifeState`);
//! - `grenades`: the throw is Chimera's predicted `Throwable` (pin,
//!   release, post-fire, the count) with the sprint gate in the predicted
//!   tick; the grenade, its blast, flash timers and fire are the server's,
//!   and Chimera's network carries them: the grenade in flight as a
//!   `NetItem`, its detonation as `Detonation`, a flash as the victim's
//!   `Senses` (`Blinded`), a smoke cloud as `NetSmoke`, damage as health.
//!
//! What this part adds (`CaNetPlugin`, registered as `combat_arms`):
//!
//! - **Armour** (Chimera's `weapon::Armor` and the gear as
//!   `weapon::ArmorPieces`): the server's, replicated in tick order to
//!   every client (the HUD's armour points; others' for spectating).
//! - **The pickup prompt** (`CaPickup`): the weapon id of the dropped item
//!   a player's view is on, the server's (`inventory::pickups` runs there),
//!   for its HUD.
//! - **The match** (`NetCaMatch`, one replicated entity): the mode, its
//!   goal, the phase, the team counters, round wins and number, the round
//!   timer's end (server ticks), the winner and result, the players per
//!   team; a client writes it into `rules::CaMatchView`, its times on its
//!   own clock (`net::game::ServerTime`), which the HUD reads as single
//!   player's. Each player's statistics (`rules::CaStats`: kills, deaths,
//!   assists, captures, mission points, ...) replicated, for the
//!   scoreboard.
//! - **The objectives** (`NetCaObjectives`, on the match's entity): the
//!   planted bomb and its fuse, the bomb on the ground, the flags, the
//!   pole, the intel (`rules::objectives::CaObjectives`, times in server
//!   ticks), which a client writes into its own `CaObjectives` on its
//!   clock (the HUD's lines, the flags' props); what each player carries
//!   (`CaCarry`) and the action gauge it fills (`CaAction`: plant, defuse,
//!   seize).
//! - **The radar** (radar_minimap.md N; `radar`): the round's map image
//!   and corners and the spawn markers (`NetCaRadar`, on the match's
//!   entity, written into a client's `CaRadarRound`), each holder's
//!   scanner (`CaScanner`: on, battery) and who a scan or a shared
//!   heartbeat list shows (`CaSeen`, on each player).
//! - **The modes of game_modes_2.md**: Hired Guns' bullion, banked bars
//!   and pockets travel in `NetCaObjectives` and `CaCarry`; each player's
//!   bank (`hired_guns::CaBank`) and squad order (`mercs::CaSquadOrder`),
//!   VIP Escort's VIP mark (`escort::CaVip`), and the custom and VIP move
//!   penalties (predicted with the movement) replicate; mercenaries
//!   (characters that aren't players, `core::Npc`) replicate as
//!   characters (`NetCharacter` with no owner, `NetBody`) with their
//!   `mercs::CaMerc` (whose hirer they follow); a client's hire and order
//!   requests (`CaHireRequest`, `CaOrderRequest`: the console's `ca_hire`
//!   and `ca_order`) go to the server, which checks them
//!   (`mercs::hire`, `mercs::set_order`).
//! - **Respawning** (game_modes.md S5): a client's player, dead for more
//!   than its wait (`respawn_wait_client` s, Relentless's shorter) by its
//!   own clock in a mode that respawns, asks once (`CaRespawnRequest`,
//!   with F7's fast flag): its death camera sends it (`death_cam`); the
//!   server checks it (`rules::spawns::request_respawn_with`).
//! - **The death camera's facts** (`death_cam::register`, its own part):
//!   each player's client id (`CaSlot`), the kill-me counts, the room's
//!   Kill Cam option on the match, fast respawns and Relentless, and
//!   the held joiners' marker (`CaHeld`).
//! - **Hooks**: the join flow (`net::JoinRules::enter`:
//!   `rules::flow::enter_game`, S1: in at once, or held until the next
//!   round), the setup's freeze for a client's prediction (`net::Holds`:
//!   `rules::flow::freeze_ends`, `frozen_intent`).
//!
//! Known shortcuts: armour, the pickup prompt and the
//! statistics go to every client, not only the player's own; a refused
//! respawn request isn't told to the client (it stays dead, as S5 has
//! it). A listen server is the app's (`combat_arms`), a dedicated one
//! apps/combat_arms_server.

use bevy::prelude::*;
use crate::gameplay::GameplayAppExt as _;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    inventory::{CaSelection, Dropped},
    radar::{CaRadarRound, CaScanner, CaSeen},
    rules::{
        CaMatch, CaMatchView, CaRuleSettings, CaStats, CaTeamCounts, Phase, current_mode,
        escort::CaVip,
        flow::{self, enter_game},
        hired_guns::CaBank,
        mercs::{self, CaMerc, CaSquadOrder, MercOrder},
        objectives::{CaAction, CaCarry, CaObjectives},
        spawns,
    },
};
use crate::{
    core::{NetRole, SimClock},
    net::{
        Holds, JoinRules, NetAppExt, NetBody, NetCharacter, NetHooks,
        game::{ServerTime, tick_of},
        ordered::OrderedAppExt,
        server::Player,
    },
    weapon::{Armor, ArmorPieces, Weapon},
};

/// Combat Arms' part of the network (`register`), named `combat_arms`.
pub struct CaNetPlugin;

impl Plugin for CaNetPlugin {
    fn build(&self, app: &mut App) {
        app.net_part("combat_arms", register);
    }
}

/// The weapon id of the dropped item a player's view is on (the server's
/// `CaSelection::pickup_target`), for its HUD's pickup prompt.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CaPickup(pub Option<String>);

/// The protocol (in this order on every side) and the systems.
fn register(app: &mut App) {
    let server = || resource_equals(NetRole::Server);
    let client = || resource_equals(NetRole::Client);
    app.replicate::<Armor>()
        .ordered::<Armor>()
        .replicate::<ArmorPieces>()
        .ordered::<ArmorPieces>()
        .replicate::<CaPickup>()
        .ordered::<CaPickup>()
        .replicate::<CaStats>()
        .ordered::<CaStats>()
        .replicate::<NetCaMatch>()
        .ordered::<NetCaMatch>()
        .replicate::<NetCaObjectives>()
        .ordered::<NetCaObjectives>()
        .replicate::<NetCaRadar>()
        .ordered::<NetCaRadar>()
        .replicate::<CaScanner>()
        .ordered::<CaScanner>()
        .replicate::<CaSeen>()
        .ordered::<CaSeen>()
        .replicate::<CaCarry>()
        .ordered::<CaCarry>()
        .replicate::<CaAction>()
        .ordered::<CaAction>()
        .replicate::<CaBank>()
        .ordered::<CaBank>()
        .replicate::<CaSquadOrder>()
        .ordered::<CaSquadOrder>()
        .replicate::<CaMerc>()
        .ordered::<CaMerc>()
        .replicate::<CaVip>()
        .ordered::<CaVip>()
        .add_client_message::<CaRespawnRequest>(Channel::Ordered)
        .add_client_message::<CaHireRequest>(Channel::Ordered)
        .add_client_message::<CaOrderRequest>(Channel::Ordered)
        // Combat Arms' hooks, used while it is the gameplay's.
        .game_hook(
            super::GAME,
            JoinRules {
                enter: enter_game,
                ..default()
            },
        )
        .game_hook(
            super::GAME,
            Holds {
                freeze_ends: flow::freeze_ends,
                frozen: flow::frozen_intent,
            },
        )
        // The match's state: while Combat Arms is the gameplay
        // (`gameplay::gameplay_is`).
        .add_systems(
            FixedLast,
            (write_pickups, write_match, replicate_mercs).run_if(server()).run_if(crate::gameplay::gameplay_is(super::GAME)),
        )
        .add_systems(
            PreUpdate,
            (receive_respawns, receive_squad_requests)
                .after(ServerSystems::Receive)
                .run_if(in_state(ServerState::Running))
                .run_if(crate::gameplay::gameplay_is(super::GAME)),
        )
        .add_systems(
            PreUpdate,
            apply_match
                .after(ClientSystems::Receive)
                .after(crate::net::predict::rebuild_own_states)
                .run_if(client())
                .run_if(crate::gameplay::gameplay_is(super::GAME)),
        )
        ;
    app.world_mut()
        .resource_mut::<NetHooks>()
        .stopped_serving
        .push(stopped_serving);
}

/// The match, on one replicated entity the server keeps (`write_match`).
/// Times are server ticks (fractional).
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetCaMatch {
    /// The room key; empty: no mode.
    pub mode: String,
    pub goal: u32,
    /// `match_phase::*`.
    pub phase: u8,
    /// The phase's end (setup, round over, intermission), server ticks.
    pub until: f64,
    pub warmup: bool,
    pub round: u32,
    pub score: [i32; 2],
    pub round_wins: [u32; 2],
    /// Live: when the round timer reaches 0, server ticks.
    pub round_ends: Option<f64>,
    pub winner: Option<u8>,
    pub result: Option<Option<u8>>,
    pub team_counts: [u32; 2],
}

/// The objectives (`rules::objectives::CaObjectives`), times in server
/// ticks (whole), on the match's entity.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetCaObjectives(pub CaObjectives);

/// The radar's round data (`radar::CaRadarRound`: the map image, its
/// corners, the spawn markers; radar_minimap.md N), on the match's entity.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetCaRadar(pub CaRadarRound);

/// `NetCaMatch::phase`.
pub mod match_phase {
    pub const WAITING: u8 = 0;
    pub const STARTING: u8 = 1;
    pub const SETUP: u8 = 2;
    pub const LIVE: u8 = 3;
    pub const ROUND_OVER: u8 = 4;
    pub const INTERMISSION: u8 = 5;
    pub const MATCH_OVER: u8 = 6;
}

/// A client's player asks to respawn (game_modes.md S5); `fast` is F7's
/// fast-respawn flag (death_killcam.md 6, 9). The client sends it itself:
/// its death camera's countdown (`death_cam`).
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct CaRespawnRequest {
    pub fast: bool,
}

/// A client asks to hire unit `unit` (game_modes_2.md M11.7).
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct CaHireRequest {
    pub unit: u8,
}

/// A client gives its squad an order.
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct CaOrderRequest {
    pub order: MercOrder,
}

/// Mercenaries replicate as characters with no owner.
fn replicate_mercs(
    q: Query<(Entity, &CaMerc, Option<&Name>), Without<Replicated>>,
    mut commands: Commands,
) {
    for (e, m, name) in &q {
        commands.entity(e).insert((
            Replicated,
            NetCharacter {
                owner: None,
                name: name.map_or_else(|| format!("Mercenary {}", m.unit), |n| n.as_str().to_string()),
            },
            NetBody::default(),
        ));
    }
}

/// Clients' hire and order requests, checked by the server.
fn receive_squad_requests(
    mut hires: MessageReader<FromClient<CaHireRequest>>,
    mut orders: MessageReader<FromClient<CaOrderRequest>>,
    players: Query<&Player>,
    mut commands: Commands,
) {
    for r in hires.read() {
        let Some(client) = r.client_id.entity() else { continue };
        let Ok(p) = players.get(client) else { continue };
        let (character, unit) = (p.character, r.message.unit as usize);
        commands.queue(move |w: &mut World| {
            if let Err(why) = mercs::hire(w, character, unit) {
                debug!("hire refused: {why}");
            }
        });
    }
    for r in orders.read() {
        let Some(client) = r.client_id.entity() else { continue };
        let Ok(p) = players.get(client) else { continue };
        let (character, order) = (p.character, r.message.order);
        commands.queue(move |w: &mut World| mercs::set_order(w, character, order));
    }
}

/// The server's match into `NetCaMatch`.
fn write_match(world: &mut World) {
    let clock = *world.resource::<SimClock>();
    let t = |s: f64| tick_of(&clock, s);
    let m = world.resource::<CaMatch>().clone();
    let (phase, until) = match m.phase {
        Phase::Waiting => (match_phase::WAITING, 0.0),
        Phase::Starting => (match_phase::STARTING, 0.0),
        Phase::Setup { until } => (match_phase::SETUP, t(until)),
        Phase::Live => (match_phase::LIVE, 0.0),
        Phase::RoundOver { until } => (match_phase::ROUND_OVER, t(until)),
        Phase::Intermission { until } => (match_phase::INTERMISSION, t(until)),
        Phase::MatchOver => (match_phase::MATCH_OVER, 0.0),
    };
    let now = NetCaMatch {
        mode: current_mode(world).map(|m| m.key.to_string()).unwrap_or_default(),
        goal: world.resource::<CaRuleSettings>().goal(),
        phase,
        // Whole ticks, so they don't change with the clock's rounding.
        until: until.round(),
        warmup: m.warmup,
        round: m.round,
        score: m.score,
        round_wins: m.round_wins,
        round_ends: m.round_ends.map(|e| t(e).round()),
        winner: m.winner,
        result: m.result,
        team_counts: world.resource::<CaTeamCounts>().0,
    };
    let mut objectives = world.resource::<CaObjectives>().clone();
    objectives.map_times(|s| t(s).round());
    let objectives = NetCaObjectives(objectives);
    let radar = NetCaRadar(world.resource::<CaRadarRound>().clone());
    let have = world.query::<(Entity, &NetCaMatch)>().iter(world).next().map(|(e, r)| (e, r.clone()));
    match have {
        Some((e, r)) => {
            if r != now {
                world.entity_mut(e).insert(now);
            }
            if world.get::<NetCaObjectives>(e) != Some(&objectives) {
                world.entity_mut(e).insert(objectives);
            }
            if world.get::<NetCaRadar>(e) != Some(&radar) {
                world.entity_mut(e).insert(radar);
            }
        }
        None => {
            world.spawn((Name::new("Match"), Replicated, now, objectives, radar));
        }
    }
}

/// A client asked to respawn its player.
fn receive_respawns(mut requests: MessageReader<FromClient<CaRespawnRequest>>, players: Query<&Player>, mut commands: Commands) {
    for r in requests.read() {
        let Some(client) = r.client_id.entity() else { continue };
        let Ok(p) = players.get(client) else { continue };
        let (character, fast) = (p.character, r.message.fast);
        commands.queue(move |w: &mut World| {
            if w.get_entity(character).is_ok()
                && let Err(why) = spawns::request_respawn_with(w, character, fast)
            {
                debug!("respawn request refused: {why}");
            }
        });
    }
}

/// The server's match into `CaMatchView`, on this client's clock.
fn apply_match(world: &mut World) {
    let Some((r, objectives, radar)) = world
        .query::<(&NetCaMatch, Option<&NetCaObjectives>, Option<&NetCaRadar>)>()
        .iter(world)
        .next()
        .map(|(m, o, rd)| (m.clone(), o.cloned(), rd.cloned()))
    else {
        return;
    };
    if let Some(NetCaRadar(round)) = radar
        && *world.resource::<CaRadarRound>() != round
    {
        world.insert_resource(round);
    }
    let Some(time) = ServerTime::of(world) else { return };
    if let Some(NetCaObjectives(mut o)) = objectives {
        o.map_times(|tick| time.local(tick));
        if *world.resource::<CaObjectives>() != o {
            world.insert_resource(o);
        }
    }
    let t = |tick: f64| time.local(tick);
    let phase = match r.phase {
        match_phase::STARTING => Phase::Starting,
        match_phase::SETUP => Phase::Setup { until: t(r.until) },
        match_phase::LIVE => Phase::Live,
        match_phase::ROUND_OVER => Phase::RoundOver { until: t(r.until) },
        match_phase::INTERMISSION => Phase::Intermission { until: t(r.until) },
        match_phase::MATCH_OVER => Phase::MatchOver,
        _ => Phase::Waiting,
    };
    let view = CaMatchView {
        mode: r.mode,
        goal: r.goal,
        phase,
        warmup: r.warmup,
        round: r.round,
        score: r.score,
        round_wins: r.round_wins,
        round_ends: r.round_ends.map(t),
        winner: r.winner,
        result: r.result,
        team_counts: r.team_counts,
    };
    if *world.resource::<CaMatchView>() != view {
        world.insert_resource(view);
    }
}

/// Each player's pickup target as the weapon it is.
fn write_pickups(
    players: Query<(Entity, &CaSelection, Option<&CaPickup>)>,
    dropped: Query<&Dropped>,
    weapons: Query<&Weapon>,
    mut commands: Commands,
) {
    for (e, sel, have) in &players {
        let id = sel
            .pickup_target
            .and_then(|i| dropped.get(i).ok())
            .and_then(|d| weapons.get(d.weapon).ok())
            .map(|w| w.id.to_string());
        if have.is_none_or(|h| h.0 != id) {
            commands.entity(e).insert(CaPickup(id));
        }
    }
}

/// Serving stopped: the prompts and the match's entity go (armour and the
/// statistics stay: single player uses them).
fn stopped_serving(world: &mut World) {
    let matches: Vec<Entity> = world.query_filtered::<Entity, With<NetCaMatch>>().iter(world).collect();
    for e in matches {
        world.despawn(e);
    }
    let with: Vec<Entity> = world.query_filtered::<Entity, With<CaPickup>>().iter(world).collect();
    for e in with {
        world.entity_mut(e).remove::<CaPickup>();
    }
}
