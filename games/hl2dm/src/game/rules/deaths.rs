//! Deaths and respawns (specs/hl2dm/rules.md "Death", "Respawn",
//! "Scoring", "Suicide commands", "Game events", "Server log lines"):
//! the dead marked, frags and deaths (not during the intermission), the
//! team-point rule (the attacker's team +1, the victim's own team -1 for
//! a suicide, a team kill still +1 to the killer's team), `player_death`
//! with the death notice's weapon name, the log lines; the respawn checks
//! (every button let go once, then a press, or `mp_forcerespawn`'s 5 s,
//! more than 3 s after the death); the 0.5 s blast grace after a spawn
//! and the 5 s between suicides (`refuse`).

use avian3d::prelude::ColliderDisabled;
use bevy::prelude::*;
use chimera_core::game_log::{EventValue, GameEvent};

use super::{
    COMBINE, FORCE_RESPAWN_AFTER, Hl2Match, Hl2Player, Hl2Rules, REBELS, RESPAWN_MIN_DEAD, SPAWN_BLAST_GRACE, SPECTATOR,
    SUICIDE_INTERVAL, team_name,
};
use crate::{
    console::{ConsoleAppExt, command_player},
    core::{Damage, DamageKind, Dead, Died, Health, Hitgroup, Intent, Score, SimClock, Spectating},
};
use chimera_sim::{
    mode::{self, Match},
    weapon::{Inventory, Weapon},
};

/// Refused before it lands: pure blast within 0.5 s of the victim's
/// spawn; a `kill` within 5 s of the last suicide (else the suicide is
/// noted).
pub(super) fn refuse(
    mut damage: MessageMutator<Damage>,
    mut players: Query<(&mut Hl2Player, &Health), Without<Dead>>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    for d in damage.read() {
        let Ok((mut p, health)) = players.get_mut(d.target) else { continue };
        if d.kind == DamageKind::Blast && now < p.spawned_at + SPAWN_BLAST_GRACE {
            d.amount = 0.0;
        } else if d.is_suicide() && health.current > 0.0 {
            if now - p.last_suicide < SUICIDE_INTERVAL {
                d.amount = 0.0;
            } else {
                p.last_suicide = now;
            }
        }
    }
}

/// The tick's deaths (after the damage), each scored in order.
pub(super) fn deaths(mut died: MessageReader<Died>, mut commands: Commands) {
    for d in died.read().cloned() {
        commands.queue(move |w: &mut World| died_now(w, &d));
    }
}

fn died_now(world: &mut World, d: &Died) {
    let e = d.entity;
    if world.get::<Hl2Player>(e).is_none() || world.get::<Dead>(e).is_some() {
        return;
    }
    // A fall or drowning is the world's.
    let attacker = d
        .attacker
        .filter(|a| !(*a == e && matches!(d.damage.kind, DamageKind::Fall | DamageKind::Drown)));
    score(world, e, attacker, &d.damage);
}

/// `e` dies now as a suicide (a team change's, `explode`): unless
/// `ignore_limit`, refused within 5 s of its last one. True when it died.
pub fn suicide(world: &mut World, e: Entity, ignore_limit: bool, kind: Option<DamageKind>) -> bool {
    let now = world.resource::<SimClock>().now;
    let Some(p) = world.get::<Hl2Player>(e).cloned() else { return false };
    let alive = world.get::<Dead>(e).is_none() && world.get::<Health>(e).is_some_and(|h| h.current > 0.0);
    if !alive || (!ignore_limit && now - p.last_suicide < SUICIDE_INTERVAL) {
        return false;
    }
    world.get_mut::<Hl2Player>(e).unwrap().last_suicide = now;
    if let Some(mut h) = world.get_mut::<Health>(e) {
        h.current = 0.0;
    }
    let at = world.get::<Transform>(e).map_or(Vec3::ZERO, |t| t.translation);
    let damage = Damage {
        target: e,
        attacker: Some(e),
        amount: 0.0,
        point: at,
        dir: Vec3::NEG_Y,
        hitgroup: Hitgroup::Generic,
        kind: kind.unwrap_or(DamageKind::Generic),
        weapon: None,
        force: Vec3::ZERO,
    };
    world.write_message(Died {
        entity: e,
        attacker: Some(e),
        damage: damage.clone(),
    });
    score(world, e, Some(e), &damage);
    true
}

/// How a death counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Kill,
    TeamKill,
    Suicide,
    World,
}

/// `victim` died of `damage`, dealt by `attacker`: marked dead, scored.
fn score(world: &mut World, victim: Entity, attacker: Option<Entity>, damage: &Damage) {
    let (now, tick) = {
        let c = world.resource::<SimClock>();
        (c.now, c.tick)
    };
    let over = world.resource::<Hl2Match>().over;
    let teamplay = world.resource::<Hl2Match>().teamplay;
    let v_team = world.get::<Hl2Player>(victim).map_or(0, |p| p.team);
    let scorer = attacker.filter(|a| world.get::<Hl2Player>(*a).is_some());
    let kind = match scorer {
        None => Kind::World,
        Some(k) if k == victim => Kind::Suicide,
        Some(k) if teamplay && world.get::<Hl2Player>(k).is_some_and(|p| p.team == v_team) => Kind::TeamKill,
        Some(_) => Kind::Kill,
    };
    // The weapon's name before anything changes what is held.
    let weapon = notice_weapon(world, scorer, damage);
    if !over {
        if let Some(mut s) = world.get_mut::<Score>(victim) {
            s.deaths += 1;
            if matches!(kind, Kind::Suicide | Kind::World) {
                s.kills -= 1;
            }
        }
        if let Some(k) = scorer.filter(|k| *k != victim)
            && let Some(mut s) = world.get_mut::<Score>(k)
        {
            s.kills += if kind == Kind::TeamKill { -1 } else { 1 };
        }
    }
    // The team point: the attacker entity's team +1, the victim's own -1.
    let point = match scorer {
        Some(k) if k == victim => Some((v_team, -1)),
        Some(k) => world.get::<Hl2Player>(k).map(|p| (p.team, 1)),
        None => None,
    };
    if let Some((team, n)) = point
        && (team == COMBINE || team == REBELS)
    {
        world.resource_mut::<Match>().score[(team - COMBINE) as usize] += n;
    }
    world.entity_mut(victim).insert((Dead { since: now }, ColliderDisabled));
    if let Some(mut p) = world.get_mut::<Hl2Player>(victim) {
        p.death_tick = Some(tick);
        p.respawnable = false;
    }
    if !over {
        let attacker_id = match scorer {
            Some(k) => EventValue::Player(k),
            None => EventValue::Int(0),
        };
        crate::rules::event(
            world,
            GameEvent::of("player_death", victim)
                .with("attacker", attacker_id)
                .with("weapon", EventValue::Str(weapon.clone())),
        );
        let v = subject(world, victim);
        let line = match (kind, scorer) {
            (Kind::Kill | Kind::TeamKill, Some(k)) => format!("{} killed {v} with \"{weapon}\"", subject(world, k)),
            (Kind::Suicide, _) => format!("{v} committed suicide with \"{weapon}\""),
            _ => format!("{v} committed suicide with \"world\""),
        };
        crate::rules::log_text(world, line);
    }
    if let Some(id) = mode::current_mode(world).map(|m| m.id) {
        let died = Died {
            entity: victim,
            attacker,
            damage: damage.clone(),
        };
        mode::mode_death(world, id, &died);
    }
}

/// A player in the log, with its HL2:DM team.
pub(super) fn subject(world: &World, e: Entity) -> String {
    let team = world.get::<Hl2Player>(e).map_or(0, |p| p.team);
    crate::rules::subject(world, e, team_name(team))
}

/// The death notice's weapon name (rules.md "Death-notice weapon name").
pub fn notice_weapon(world: &World, scorer: Option<Entity>, damage: &Damage) -> String {
    let class = match (damage.weapon, scorer) {
        (Some(w), _) => w.to_string(),
        (None, Some(k)) => held(world, k).map_or_else(|| "world".to_string(), str::to_string),
        (None, None) => match damage.kind {
            DamageKind::Fall | DamageKind::Drown => "worldspawn".into(),
            DamageKind::Crush => "func_door".into(),
            _ => "trigger_hurt".into(),
        },
    };
    notice_name(&class)
}

/// What `k` holds (its weapon's id).
fn held(world: &World, k: Entity) -> Option<&'static str> {
    let w = world.get::<Inventory>(k)?.active?;
    world.get::<Weapon>(w).map(|w| w.id)
}

/// A class name (or a weapon id, `game:weapon_x`) as the death notice
/// names it: one leading `weapon_`, `npc_` or `func_` dropped, else
/// anything with `physics` is `physics`; then HL2:DM's renames.
pub fn notice_name(class: &str) -> String {
    let c = class.rsplit(':').next().unwrap_or(class).to_ascii_lowercase();
    let c = match ["weapon_", "npc_", "func_"].iter().find_map(|p| c.strip_prefix(p)) {
        Some(rest) => rest.to_string(),
        None if c.contains("physics") => "physics".into(),
        None => c,
    };
    match c.as_str() {
        "prop_combine_ball" => "combine_ball".into(),
        "grenade_ar2" => "smg1_grenade".into(),
        "satchel" | "tripmine" => "slam".into(),
        _ => c,
    }
}

/// The dead's respawn checks, each command (before their intent is
/// cleared): respawnable once no button is down; then a button, or the
/// force after 5 s, respawns more than 3 s after the death.
#[allow(clippy::type_complexity)]
pub(super) fn respawns(
    mut dead: Query<(Entity, &mut Hl2Player, &Intent), (With<Dead>, Without<Spectating>)>,
    rules: Res<Hl2Rules>,
    m: Res<Hl2Match>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    if m.over {
        return;
    }
    let dt = clock.delta.as_secs_f64();
    for (e, mut p, i) in &mut dead {
        if p.team == SPECTATOR {
            continue;
        }
        let Some(died) = p.death_tick else { continue };
        let down = buttons(i);
        if !p.respawnable {
            if !down {
                p.respawnable = true;
            }
            continue;
        }
        let dead_for = (clock.tick - died) as f64 * dt;
        let forced = rules.forcerespawn > 0 && dead_for > FORCE_RESPAWN_AFTER + 1e-9;
        if (down || forced) && dead_for > RESPAWN_MIN_DEAD + 1e-9 {
            commands.queue(move |w: &mut World| super::spawns::spawn(w, e));
        }
    }
}

/// Any button down (the score key isn't one).
pub fn buttons(i: &Intent) -> bool {
    i.move_axis != Vec2::ZERO
        || i.jump
        || i.crouch
        || i.sprint
        || i.walk
        || i.fire
        || i.secondary
        || i.reload
        || i.use_key
        || i.actions != 0
}

/// `player_spawn`.
pub fn spawn_event(world: &mut World, e: Entity) {
    crate::rules::event(world, GameEvent::of("player_spawn", e));
}

/// `player_team` and the log line "joined team".
pub fn team_event(world: &mut World, e: Entity, team: u8, old: u8, disconnect: bool) {
    let name = world.get::<Name>(e).map_or_else(String::new, |n| n.as_str().to_string());
    crate::rules::event(
        world,
        GameEvent::of("player_team", e)
            .with("team", EventValue::Int(team as i64))
            .with("oldteam", EventValue::Int(old as i64))
            .with("disconnect", EventValue::Bool(disconnect))
            .with("autoteam", EventValue::Bool(false))
            .with("silent", EventValue::Bool(false))
            .with("name", EventValue::Str(name)),
    );
    if !disconnect {
        let who = crate::rules::subject(world, e, team_name(old));
        crate::rules::log_text(world, format!("{who} joined team \"{}\"", team_name(team)));
    }
}

/// A player left: `player_team` with `disconnect`, and in TDM a living
/// player's death as a suicide (the team point to team 0: none shown).
pub fn left_event(world: &mut World, e: Entity, team: u8, alive: bool) {
    crate::rules::event(
        world,
        GameEvent::new("player_team")
            .with("userid", EventValue::Int(e.index().index() as i64))
            .with("team", EventValue::Int(0))
            .with("oldteam", EventValue::Int(team as i64))
            .with("disconnect", EventValue::Bool(true)),
    );
    if alive && world.resource::<Hl2Match>().teamplay && team != SPECTATOR {
        crate::rules::event(
            world,
            GameEvent::new("player_death")
                .with("userid", EventValue::Int(e.index().index() as i64))
                .with("attacker", EventValue::Int(e.index().index() as i64))
                .with("weapon", EventValue::Str("world".into())),
        );
    }
}

/// `explode`.
pub(super) fn commands(app: &mut App) {
    app.console_command("hl2dm:explode", "Kill yourself in a blast (once every 5 s).", |w, _| {
        let e = command_player(w)?;
        suicide(w, e, false, Some(DamageKind::Blast));
        Ok(None)
    });
    app.world_mut().resource_mut::<crate::console::Console>().player_command("hl2dm:explode");
}

#[cfg(test)]
mod tests {
    #[test]
    fn notice_names() {
        use super::notice_name as n;
        for (class, name) in [
            ("hl2dm:weapon_smg1", "smg1"),
            ("weapon_357", "357"),
            ("weapon_physcannon", "physcannon"),
            ("prop_combine_ball", "combine_ball"),
            ("grenade_ar2", "smg1_grenade"),
            ("npc_satchel", "slam"),
            ("npc_tripmine", "slam"),
            ("prop_physics_respawnable", "physics"),
            ("func_physbox", "physbox"),
            ("func_door", "door"),
            ("crossbow_bolt", "crossbow_bolt"),
            ("rpg_missile", "rpg_missile"),
            ("npc_grenade_frag", "grenade_frag"),
            ("trigger_hurt", "trigger_hurt"),
            ("worldspawn", "worldspawn"),
        ] {
            assert_eq!(n(class), name, "{class}");
        }
    }
}
