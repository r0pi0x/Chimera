//! HL2:DM's players (specs/hl2dm/rules.md "Joining", "Models",
//! "Changing team", "Disconnecting"): `Hl2Player` (its team number, model
//! and timers), joining, the models and their round-robin lists
//! (data/player_models.toml), `cl_playermodel` changes, `jointeam` and
//! `spectate`, leaving. Chimera's `core::Team` follows the HL2:DM team:
//! TDM's Combine and Rebels are the two sides (`mode::ALPHA`,
//! `mode::BRAVO`), every DM player a team of its own (`mode::FFA_FIRST`
//! on: nobody is anyone's teammate, `say_team` reaches only its sender),
//! a spectator `Team(0)` and `core::Spectating`.

use bevy::prelude::*;
use serde::Deserialize;

use super::{COMBINE, Hl2Match, Hl2Rules, MODEL_CHANGE_INTERVAL, REBELS, SPECTATOR, TEAM_CMD_RATE, UNASSIGNED, team_name};
use crate::{
    console::{ConsoleAppExt, command_player},
    core::{Connecting, Dead, Health, Intent, LocalPlayer, Npc, Score, SimClock, Spectating, Team, UserInfo},
};
use super::super::weapons::inventory::{DEFAULT_MODEL, Hl2Model, ModelKind};
use chimera_sim::mode::{ALPHA, BRAVO, FFA_FIRST, FFA_LAST};

/// A player in HL2:DM's game.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Hl2Player {
    /// HL2:DM's team number (`super::UNASSIGNED`...).
    pub team: u8,
    /// Its model (a valid one, lower case).
    pub model: String,
    /// A model change is refused until after this (s).
    pub model_free_at: f64,
    /// When `jointeam` and `spectate` were last accepted (s).
    pub last_jointeam: f64,
    pub last_spectate: f64,
    /// The last suicide (s).
    pub last_suicide: f64,
    /// The tick it died (None: alive or never).
    pub death_tick: Option<u64>,
    /// Dead, with every button let go once since.
    pub respawnable: bool,
    /// Said `mp_ready_signal` while the game waits for it.
    pub ready: bool,
    /// The last spawn (s): the blast grace counts from it.
    pub spawned_at: f64,
    /// The look the intermission froze.
    pub frozen_look: Option<(f32, f32)>,
    /// The order it came in.
    pub order: u64,
}

impl Hl2Player {
    fn new(order: u64) -> Self {
        Self {
            team: UNASSIGNED,
            model: String::new(),
            model_free_at: f64::NEG_INFINITY,
            last_jointeam: f64::NEG_INFINITY,
            last_spectate: f64::NEG_INFINITY,
            last_suicide: f64::NEG_INFINITY,
            death_tick: None,
            respawnable: false,
            ready: false,
            spawned_at: f64::NEG_INFINITY,
            frozen_look: None,
            order,
        }
    }
}

/// data/player_models.toml.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlayerModels {
    pub combine: Vec<String>,
    pub citizens: Vec<String>,
}

pub const MODELS: &str = include_str!("../../../data/player_models.toml");

impl PlayerModels {
    pub fn hl2() -> Self {
        toml::from_str(MODELS).unwrap_or_else(|e| panic!("data/player_models.toml: {e}"))
    }

    /// `name` is one of the 19 (ignoring case and slashes).
    pub fn valid(&self, name: &str) -> Option<String> {
        let n = name.trim().to_ascii_lowercase().replace('\\', "/");
        self.combine.iter().chain(&self.citizens).find(|m| **m == n).cloned()
    }
}

/// The round-robin counters (one per list, never reset, not even at a map
/// change: a process's).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModelCounters {
    pub combine: usize,
    pub citizens: usize,
}

/// The client's `cl_playermodel` (userinfo), the local player's.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct PlayerModelSetting(pub String);

impl Default for PlayerModelSetting {
    fn default() -> Self {
        Self("none".into())
    }
}

/// The userinfo key.
pub const PLAYERMODEL_KEY: &str = "cl_playermodel";

fn is_citizen(model: &str) -> bool {
    ModelKind::of(model) == Some(ModelKind::Citizen)
}

/// The players in the match, in the order they came in (on a team that
/// plays; not spectating or connecting).
pub fn players(world: &mut World) -> Vec<(Entity, u8)> {
    let mut all: Vec<(u64, Entity, u8)> = world
        .query_filtered::<(Entity, &Hl2Player, &Team), (Without<Spectating>, Without<Connecting>)>()
        .iter(world)
        .filter(|(_, p, _)| p.team != SPECTATOR)
        .map(|(e, p, t)| (p.order, e, t.0))
        .collect();
    all.sort();
    all.into_iter().map(|(_, e, t)| (e, t)).collect()
}

/// Every player's frags and deaths back to 0, the team scores too (a new
/// map, a restart).
pub fn reset_scores(world: &mut World) {
    let all: Vec<Entity> = world.query_filtered::<Entity, With<Hl2Player>>().iter(world).collect();
    for e in all {
        world.entity_mut(e).insert(Score::default());
    }
    world.resource_mut::<chimera_sim::mode::Match>().score = [0; 2];
}

/// A new map: DM puts everyone playing back on team 0; TDM keeps the
/// teams (a team 0 player joins one at its spawn).
pub fn new_map(world: &mut World) {
    let teamplay = world.resource::<Hl2Match>().teamplay;
    let all: Vec<(Entity, u8)> = world.query::<(Entity, &Hl2Player)>().iter(world).map(|(e, p)| (e, p.team)).collect();
    for (e, team) in all {
        let team = if !teamplay && team != SPECTATOR { UNASSIGNED } else { team };
        set_team(world, e, team);
    }
}

/// Characters not yet in HL2:DM's game come in (the local player, a
/// remote one whose client has the map, a test's).
#[allow(clippy::type_complexity)]
pub(super) fn join(
    new: Query<Entity, (With<Intent>, With<Health>, With<Team>, Without<Hl2Player>, Without<Connecting>, Without<Npc>)>,
    mut commands: Commands,
) {
    for e in &new {
        commands.queue(move |w: &mut World| join_one(w, e));
    }
}

/// `e` comes into the game: team 0 (DM) or the team with fewer players
/// (TDM; a random one on a tie), its model, and spawned at once.
pub fn join_one(world: &mut World, e: Entity) {
    if world.get::<Hl2Player>(e).is_some() || world.get_entity(e).is_err() {
        return;
    }
    let order = world.query::<&Hl2Player>().iter(world).map(|p| p.order + 1).max().unwrap_or(0);
    let mut player = Hl2Player::new(order);
    player.model = String::new();
    world.entity_mut(e).insert((player, Score::default()));
    world.entity_mut(e).remove::<Spectating>();
    // In at once (no team menu): TDM joins a team at the spawn, DM is on
    // team 0; the model resolved after the spawn's reset of its timer.
    if !world.resource::<Hl2Match>().teamplay {
        set_team(world, e, UNASSIGNED);
    }
    super::spawns::spawn(world, e);
    let teamplay = world.resource::<Hl2Match>().teamplay;
    if teamplay {
        let team = world.get::<Hl2Player>(e).map_or(UNASSIGNED, |p| p.team);
        say(world, e, format!("You are on team {}", team_name(team)));
    } else {
        resolve_model(world, e, true);
    }
}

/// The team with fewer players (counting the dead and bots), a random one
/// of Combine and Rebels on a tie.
pub fn smaller_team(world: &mut World, me: Entity) -> u8 {
    let mut count = [0usize; 2];
    for (e, p) in world.query::<(Entity, &Hl2Player)>().iter(world) {
        if e != me && (p.team == COMBINE || p.team == REBELS) {
            count[(p.team - COMBINE) as usize] += 1;
        }
    }
    match count[0].cmp(&count[1]) {
        std::cmp::Ordering::Less => COMBINE,
        std::cmp::Ordering::Greater => REBELS,
        std::cmp::Ordering::Equal => {
            if world.resource_mut::<Hl2Match>().roll(2) == 0 {
                COMBINE
            } else {
                REBELS
            }
        }
    }
}

/// `e` on HL2:DM team `team`, and Chimera's team to match.
pub fn set_team(world: &mut World, e: Entity, team: u8) {
    if let Some(mut p) = world.get_mut::<Hl2Player>(e) {
        p.team = team;
    }
    let engine = match team {
        COMBINE => ALPHA,
        REBELS => BRAVO,
        SPECTATOR => 0,
        _ => {
            let taken: Vec<u8> = world
                .query::<(Entity, &Hl2Player, &Team)>()
                .iter(world)
                .filter(|(o, p, _)| *o != e && p.team == UNASSIGNED)
                .map(|(_, _, t)| t.0)
                .collect();
            let own = world.get::<Team>(e).map(|t| t.0).filter(|t| *t >= FFA_FIRST && !taken.contains(t));
            own.or_else(|| (FFA_FIRST..=FFA_LAST).find(|t| !taken.contains(t))).unwrap_or(FFA_FIRST)
        }
    };
    if world.get::<Team>(e).is_none_or(|t| t.0 != engine) {
        world.entity_mut(e).insert(Team(engine));
    }
}

/// A line to the player (its console; HL2:DM shows it in chat).
fn say(world: &mut World, e: Entity, text: String) {
    if world.get::<LocalPlayer>(e).is_some()
        && let Some(mut c) = world.get_resource_mut::<crate::console::Console>()
    {
        c.info(text);
    }
}

/// `e`'s `cl_playermodel`.
fn wanted_model(world: &World, e: Entity) -> String {
    world
        .get::<UserInfo>(e)
        .and_then(|u| u.0.get(PLAYERMODEL_KEY).cloned())
        .unwrap_or_else(|| "none".into())
}

/// Advance a list's counter and take its model (the first replacement
/// after a start is the list's second model).
fn next_of(world: &mut World, citizens: bool) -> String {
    let models = PlayerModels::hl2();
    let mut c = world.resource_mut::<ModelCounters>();
    if citizens {
        c.citizens = (c.citizens + 1) % models.citizens.len();
        models.citizens[c.citizens].clone()
    } else {
        c.combine = (c.combine + 1) % models.combine.len();
        models.combine[c.combine].clone()
    }
}

/// `e`'s model resolved by its team's rules ("Models in TDM", "Models in
/// DM"), the 5 s model timer started; the client told when its setting
/// isn't the model (`tell`).
pub fn resolve_model(world: &mut World, e: Entity, tell: bool) {
    let models = PlayerModels::hl2();
    let Some(p) = world.get::<Hl2Player>(e).cloned() else { return };
    let asked = wanted_model(world, e);
    let teamplay = world.resource::<Hl2Match>().teamplay;
    let mut model = match models.valid(&asked) {
        Some(m) => m,
        None if !teamplay => models.valid(&p.model).unwrap_or_else(|| DEFAULT_MODEL.into()),
        None => DEFAULT_MODEL.into(),
    };
    let fallback = models.valid(&asked).is_none();
    if teamplay {
        if p.team == COMBINE && is_citizen(&model) {
            model = next_of(world, false);
        } else if p.team == REBELS && !is_citizen(&model) {
            model = next_of(world, true);
        }
    }
    let now = world.resource::<SimClock>().now;
    if let Some(mut p) = world.get_mut::<Hl2Player>(e) {
        p.model = model.clone();
        p.model_free_at = now + MODEL_CHANGE_INTERVAL;
    }
    world.entity_mut(e).insert(Hl2Model(model.clone()));
    // The client's setting follows (an invalid one is rewritten to the
    // fallback, a team's replacement to the model used): the server's
    // copy, and the local player's own cvar.
    if tell || fallback || !asked.eq_ignore_ascii_case(&model) {
        let set = if fallback { DEFAULT_MODEL.to_string() } else { model };
        if world.get::<LocalPlayer>(e).is_some()
            && let Some(mut s) = world.get_resource_mut::<PlayerModelSetting>()
        {
            s.0 = set.clone();
        }
        match world.get_mut::<UserInfo>(e) {
            Some(mut u) => {
                u.0.insert(PLAYERMODEL_KEY.into(), set);
            }
            None => {
                let mut u = UserInfo::default();
                u.0.insert(PLAYERMODEL_KEY.into(), set);
                world.entity_mut(e).insert(u);
            }
        }
    }
}

/// `cl_playermodel` changed during play ("Changing `cl_playermodel`").
#[allow(clippy::type_complexity)]
pub(super) fn model_changes(
    changed: Query<(Entity, &UserInfo, &Hl2Player), Changed<UserInfo>>,
    mut commands: Commands,
) {
    for (e, info, p) in &changed {
        let Some(asked) = info.0.get(PLAYERMODEL_KEY).cloned() else { continue };
        if asked.eq_ignore_ascii_case(&p.model) || p.model.is_empty() {
            continue;
        }
        commands.queue(move |w: &mut World| change_model(w, e, &asked));
    }
}

/// `e` asks for model `asked`.
pub fn change_model(world: &mut World, e: Entity, asked: &str) {
    let Some(p) = world.get::<Hl2Player>(e).cloned() else { return };
    if asked.eq_ignore_ascii_case(&p.model) {
        return;
    }
    let now = world.resource::<SimClock>().now;
    if p.model_free_at >= now {
        // Back to the model it has, and a word.
        let left = (p.model_free_at - now).floor() as i64;
        if let Some(mut u) = world.get_mut::<UserInfo>(e) {
            u.0.insert(PLAYERMODEL_KEY.into(), p.model.clone());
        }
        if world.get::<LocalPlayer>(e).is_some()
            && let Some(mut s) = world.get_resource_mut::<PlayerModelSetting>()
        {
            s.0 = p.model.clone();
        }
        say(world, e, format!("Please wait {left} more seconds before trying to switch."));
        return;
    }
    if world.resource::<Hl2Match>().teamplay {
        let Some(valid) = PlayerModels::hl2().valid(asked) else {
            resolve_model(world, e, true);
            return;
        };
        let team = if is_citizen(&valid) { REBELS } else { COMBINE };
        if team != p.team && p.team != SPECTATOR {
            change_team(world, e, team, true);
        } else {
            resolve_model(world, e, false);
        }
        return;
    }
    resolve_model(world, e, false);
    let model = world.get::<Hl2Player>(e).map(|p| p.model.clone()).unwrap_or_default();
    say(world, e, format!("Your player model is: {model}"));
}

/// `e` moves to `team` (2 or 3, or 0 in DM): `player_team`, a TDM
/// suicide after the move when it came from a team and lives (the 5 s
/// suicide limit applies), the model re-resolved.
pub fn change_team(world: &mut World, e: Entity, team: u8, suicide: bool) {
    let Some(old) = world.get::<Hl2Player>(e).map(|p| p.team) else { return };
    let teamplay = world.resource::<Hl2Match>().teamplay;
    let team = if teamplay { team } else { UNASSIGNED };
    if team != old {
        super::deaths::team_event(world, e, team, old, false);
        set_team(world, e, team);
        world.entity_mut(e).remove::<Spectating>();
        if teamplay && suicide && old != UNASSIGNED {
            super::deaths::suicide(world, e, false, None);
        }
    }
    resolve_model(world, e, false);
}

/// `jointeam <n>` for `e` (the command's rate already checked).
pub fn jointeam(world: &mut World, e: Entity, n: u8) -> Result<Option<String>, String> {
    match n {
        SPECTATOR => spectate(world, e),
        COMBINE | REBELS => {
            // Out of any observer mode; still dead if it was.
            let was_spectator = world.get::<Hl2Player>(e).is_some_and(|p| p.team == SPECTATOR);
            if was_spectator {
                world.entity_mut(e).remove::<Spectating>();
            }
            change_team(world, e, n, true);
            Ok(None)
        }
        _ => Err(format!("jointeam {n}: not a team you can join")),
    }
}

/// `spectate` (and `jointeam 1`).
pub fn spectate(world: &mut World, e: Entity) -> Result<Option<String>, String> {
    if world.resource::<Hl2Rules>().allowspectators == 0 {
        return Err("This server does not allow spectating".into());
    }
    let Some(p) = world.get::<Hl2Player>(e).cloned() else { return Ok(None) };
    let alive = world.get::<Dead>(e).is_none() && world.get::<Health>(e).is_some_and(|h| h.current > 0.0);
    if p.team != UNASSIGNED && p.team != SPECTATOR && alive {
        // A suicide that ignores the limit, one frag given back.
        super::deaths::suicide(world, e, true, None);
        if let Some(mut s) = world.get_mut::<Score>(e) {
            s.kills += 1;
        }
    }
    super::deaths::team_event(world, e, SPECTATOR, p.team, false);
    set_team(world, e, SPECTATOR);
    chimera_sim::weapon::equip::strip(world, e);
    let now = world.resource::<SimClock>().now;
    world.entity_mut(e).insert((Spectating, Dead { since: now }, avian3d::prelude::ColliderDisabled));
    if let Some(mut h) = world.get_mut::<Health>(e) {
        h.current = 0.0;
    }
    Ok(None)
}

/// A player left (its entity removed): taken off its team; in TDM a
/// living one dies as a suicide on the way (a `player_death`, the team
/// point to team 0).
pub(super) fn left(remove: On<Remove<Hl2Player>>, players: Query<(&Hl2Player, Option<&Dead>)>, mut commands: Commands) {
    let Ok((p, dead)) = players.get(remove.entity) else { return };
    let (e, team, alive) = (remove.entity, p.team, dead.is_none());
    commands.queue(move |w: &mut World| {
        if !w.get_resource::<Hl2Match>().is_some_and(|m| m.on) {
            return;
        }
        super::deaths::left_event(w, e, team, alive);
    });
}

/// The players' commands and the client's `cl_playermodel`.
pub(super) fn commands(app: &mut App) {
    app.init_resource::<PlayerModelSetting>();
    crate::console::resource_cvar::<PlayerModelSetting, String>(
        app,
        "hl2dm:cl_playermodel",
        "Your player model (one of HL2:DM's 19; sent to the server as userinfo).",
        |s| &mut s.0,
    );
    {
        let mut console = app.world_mut().resource_mut::<crate::console::Console>();
        console.archive("hl2dm:cl_playermodel");
        console.userinfo("hl2dm:cl_playermodel");
    }
    app.console_command(
        "hl2dm:jointeam",
        "jointeam <1|2|3>: spectate, join the Combine or the Rebels.",
        |w, a| {
            let e = command_player(w)?;
            let n: u8 = a.first().and_then(|n| n.trim().parse().ok()).unwrap_or(0);
            if !accept(w, e, false) {
                return Ok(None);
            }
            if n == UNASSIGNED || n > REBELS {
                return Err(format!("jointeam {n}: no such team"));
            }
            jointeam(w, e, n)
        },
    )
    .console_command("hl2dm:spectate", "Watch the game from the spectators.", |w, _| {
        let e = command_player(w)?;
        if !accept(w, e, true) {
            return Ok(None);
        }
        spectate(w, e)
    })
    .console_command("hl2dm:joingame", "Accepted; does nothing.", |_, _| Ok(None));
    {
        let mut console = app.world_mut().resource_mut::<crate::console::Console>();
        for name in ["hl2dm:jointeam", "hl2dm:spectate", "hl2dm:joingame"] {
            console.player_command(name);
        }
    }
}

/// The 0.3 s command rate: whether this `jointeam` (or `spectate`) is
/// taken (and if so, noted).
fn accept(world: &mut World, e: Entity, spectate: bool) -> bool {
    let now = world.resource::<SimClock>().now;
    let Some(mut p) = world.get_mut::<Hl2Player>(e) else { return false };
    let last = if spectate { &mut p.last_spectate } else { &mut p.last_jointeam };
    if now - *last < TEAM_CMD_RATE {
        return false;
    }
    *last = now;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rung characters' groups are these lists, in this order.
    #[test]
    fn the_models_are_the_rungs() {
        let m = PlayerModels::hl2();
        assert_eq!((m.combine.len(), m.citizens.len()), (4, 15));
        let h = super::super::super::rungs::holders();
        let all = super::super::super::rungs::models_for(&h, "hl2dm:weapon_pistol");
        let mine: Vec<String> = m.combine.iter().chain(&m.citizens).cloned().collect();
        assert_eq!(all, mine);
        assert_eq!(m.valid("Models/Combine_Soldier.mdl").as_deref(), Some("models/combine_soldier.mdl"));
        assert_eq!(m.valid("none"), None);
    }
}
