//! Capture the flag (specs/halo_ce/multiplayer_rules.md 6.4): two flags at
//! the scenario's capture the flag spots (netgame flags of type 0, by team
//! index: 0 red, 1 blue), each owned by its team (`HaloFlag`).
//!
//! Each Halo tick (`tick`, after the players' spawns):
//! - **Drop**: a carrier with no unit (dead, gone) drops the flag where it
//!   was; the flag's "last held" is that tick.
//! - **Grab**: a living enemy of the flag's team touching it (within the
//!   biped's bounding radius of the unit's centre, as the pickups touch)
//!   carries it: "has the flag" for the carrier's team (not in assault),
//!   again every 20 s while it is carried.
//! - **Return by touch**: a living player of the owning team touching the
//!   flag away from home returns it at once ("flag returned", the team's
//!   returns +1), unless the variant has "flag must reset" (then the
//!   failure sound, at most every 4 s); the flag at home is left alone.
//! - **Timeout**: a flag not carried, away from home and not held for more
//!   than 510 ticks returns.
//! - **Capture**: a carrier whose unit (its feet, the origin) is less than
//!   1.0 WU (3D) from its own team's flag's home scores: the team +1, the
//!   player's score +1 (its flag scores); the flag returns. With "flag at
//!   home to score" only while the carrier's own flag is at home (else the
//!   failure sound). Nothing scores once the game is over.
//! - **Assault**: team t's flag starts at the other team's spot: a team
//!   takes the enemy's flag from its own base to the enemy's.
//! - **Single flag**: one flag, a random team's; each time the single flag
//!   time elapses with it not carried, the other team owns it and it goes
//!   to that team's spot.
//!
//! What the carrier cannot do (6.4, weapons.md: the flag is an objective
//! weapon that must be readied, held in place of its guns): fire, zoom,
//! reload or throw (the server clears those inputs), and it loses active
//! camouflage every tick.
//!
//! Every machine sees a flag (`ShownItem` of the flag's model and
//! `ShownBody`, so the network's `NetItem` carries it): at its spot, on the
//! carrier's back, or where it lies.
//!
//! Known shortcuts: the carrier's own client predicts its shots and then
//! sees the server refuse them; the flag is carried on the back, not in
//! hand (no first-person flag); the announcer's lines are `HaloAnnounce`s
//! (text); with single flag the period restarts every time it elapses,
//! carried or not (6.4 does not say).

use bevy::prelude::*;
use blam_formats::game_type::GameTypeOptions;
use chimera_sim::{mode::{Match, side}, weapon::projectile::ShownBody};

use super::{
    BLUE, HaloGameType, HaloMatch, HaloPlayer, RED,
    netgame::{Netgame, WU},
    scoring::{HaloAnnounce, Line, add_score},
    spawning::{centre_above_feet, feet},
};
use crate::{
    core::{Dead, Intent, Team},
    map::loose::ShownItem,
};

/// The flag's tag (the globals' multiplayer information: 11.1).
pub const FLAG_TAG: &str = "weapons\\flag\\flag";
/// Ticks a dropped flag waits before it returns (more than this).
pub const RETURN_TICKS: i64 = 510;
/// The capture's distance (WU, strict).
pub const CAPTURE_RADIUS: f32 = 1.0;
/// Ticks between "has the flag" lines while carried.
pub const WARNING_TICKS: i64 = 600;
/// Ticks between failure sounds.
pub const FAILURE_TICKS: i64 = 120;

/// A flag: its owning team, its home (feet, metres), whether it was handled
/// (away from home), its carrier, the tick it was last held.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct HaloFlag {
    pub team: u8,
    pub home: Vec3,
    pub away: bool,
    pub carrier: Option<Entity>,
    pub last_held: i64,
    /// The next "has the flag" line (game time).
    pub next_warning: i64,
}

/// Capture the flag's state beyond the flags.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CtfState {
    /// The last failure sound (game time).
    pub last_failure: Option<i64>,
    /// Single flag: the next swap (game time).
    pub next_swap: Option<i64>,
}

/// The game type's options: (assault, flag must reset, flag at home to
/// score, single flag time).
pub fn options(r: &blam_formats::game_type::Rules) -> Option<(bool, bool, bool, i32)> {
    match r.options {
        GameTypeOptions::CaptureTheFlag { assault, flag_must_reset, flag_at_home_to_score, single_flag_time, .. } => {
            Some((assault, flag_must_reset, flag_at_home_to_score, single_flag_time))
        }
        _ => None,
    }
}

/// Team index (0 red, 1 blue) of an engine team.
fn index(team: u8) -> i16 {
    if team == BLUE { 1 } else { 0 }
}

/// Where team `team`'s flag lives in this game (its spot; assault: the
/// other team's).
pub fn home_of(n: &Netgame, team: u8, assault: bool) -> Option<Vec3> {
    let i = if assault { 1 - index(team) } else { index(team) };
    n.flag_spot(i).map(|f| f.at)
}

fn shown() -> ShownItem {
    ShownItem(super::super::view::load::item_key(FLAG_TAG))
}

/// The flags at the game's start (the runtime's round setup).
pub fn setup(world: &mut World) {
    let old: Vec<Entity> = world.query_filtered::<Entity, With<HaloFlag>>().iter(world).collect();
    for e in old {
        world.despawn(e);
    }
    let r = world.resource::<HaloGameType>().0.rules;
    let Some((assault, _, _, single)) = options(&r) else { return };
    let Some(n) = world.get_resource::<Netgame>().cloned() else { return };
    let teams: Vec<u8> = if single > 0 {
        let t = if world.resource_mut::<HaloMatch>().roll(2) == 0 { RED } else { BLUE };
        vec![t]
    } else {
        vec![RED, BLUE]
    };
    let now = world.resource::<HaloMatch>().time;
    for team in teams {
        let Some(home) = home_of(&n, team, assault) else { continue };
        world.spawn((
            Name::new(format!("halo_ce flag {team}")),
            HaloFlag { team, home, away: false, carrier: None, last_held: now, next_warning: 0 },
            Transform::from_translation(home),
            shown(),
            ShownBody { owner: None },
        ));
    }
    world.insert_resource(CtfState {
        last_failure: None,
        next_swap: (single > 0).then_some(now + single as i64),
    });
}

/// The flags gone (another game type begins).
pub fn clear(world: &mut World) {
    let old: Vec<Entity> = world.query_filtered::<Entity, With<HaloFlag>>().iter(world).collect();
    for e in old {
        world.despawn(e);
    }
}

fn failure(world: &mut World) {
    let now = world.resource::<HaloMatch>().time;
    let mut s = world.get_resource_or_init::<CtfState>();
    if s.last_failure.is_none_or(|t| now - t >= FAILURE_TICKS) {
        s.last_failure = Some(now);
        world.write_message(HaloAnnounce { line: Line::FlagFailure, player: None });
    }
}

fn return_flag(world: &mut World, f: Entity, announce: bool) {
    let Some(mut flag) = world.get_mut::<HaloFlag>(f) else { return };
    flag.away = false;
    flag.carrier = None;
    let (team, home) = (flag.team, flag.home);
    if let Some(mut t) = world.get_mut::<Transform>(f) {
        t.translation = home;
    }
    if announce {
        world.write_message(HaloAnnounce { line: Line::FlagReturned(team), player: None });
    }
}

/// One Halo tick of capture the flag (module doc).
pub fn tick(world: &mut World) {
    let r = world.resource::<HaloGameType>().0.rules;
    let Some((assault, must_reset, at_home_to_score, single)) = options(&r) else { return };
    let (now, over) = {
        let m = world.resource::<HaloMatch>();
        (m.time, m.over)
    };
    let reach = world
        .get_resource::<super::super::weapons::grenades::GrenadeData>()
        .map_or(0.7 * WU * 0.3, |g| g.explosions.biped_bounding_radius * WU);
    let units: Vec<(Entity, Vec3, u8)> = world
        .query_filtered::<(Entity, &Transform, &Team), (With<HaloPlayer>, Without<Dead>)>()
        .iter(world)
        .map(|(e, t, team)| (e, t.translation, team.0))
        .collect();
    let flags: Vec<(Entity, HaloFlag)> = world.query::<(Entity, &HaloFlag)>().iter(world).map(|(e, f)| (e, f.clone())).collect();
    for (f, flag) in &flags {
        // Drop.
        if let Some(c) = flag.carrier {
            match units.iter().find(|(e, ..)| *e == c) {
                Some((_, centre, _)) => {
                    // Carried: on the back.
                    let yaw = world.get::<Intent>(c).map_or(0.0, |i| i.yaw);
                    let back = Quat::from_rotation_y(yaw) * Vec3::Z * 0.25 + Vec3::Y * 0.3;
                    if let Some(mut t) = world.get_mut::<Transform>(*f) {
                        t.translation = *centre + back;
                        t.rotation = Quat::from_rotation_y(yaw);
                    }
                    if let Some(mut fl) = world.get_mut::<HaloFlag>(*f) {
                        fl.last_held = now;
                    }
                    if !assault && now >= flag.next_warning {
                        let team = world.get::<Team>(c).map_or(0, |t| t.0);
                        world.write_message(HaloAnnounce { line: Line::HasTheFlag(team), player: None });
                        if let Some(mut fl) = world.get_mut::<HaloFlag>(*f) {
                            fl.next_warning = now + WARNING_TICKS;
                        }
                    }
                    continue;
                }
                None => {
                    let at = world.get::<Transform>(c).map(feet);
                    if let Some(mut fl) = world.get_mut::<HaloFlag>(*f) {
                        fl.carrier = None;
                        fl.last_held = now;
                    }
                    if let (Some(at), Some(mut t)) = (at, world.get_mut::<Transform>(*f)) {
                        t.translation = at;
                    }
                    continue;
                }
            }
        }
        let at = world.get::<Transform>(*f).map_or(flag.home, |t| t.translation);
        // Timeout.
        if flag.away && now - flag.last_held > RETURN_TICKS {
            return_flag(world, *f, true);
            continue;
        }
        // Touches, in slot order of the units listed.
        let touching: Vec<(Entity, u8)> =
            units.iter().filter(|(_, c, _)| c.distance(at + Vec3::Y * centre_above_feet()) <= reach).map(|(e, _, t)| (*e, *t)).collect();
        for (e, team) in touching {
            if team != flag.team {
                if over {
                    break;
                }
                if let Some(mut fl) = world.get_mut::<HaloFlag>(*f) {
                    fl.carrier = Some(e);
                    fl.away = true;
                    fl.last_held = now;
                    fl.next_warning = now;
                }
                break;
            }
            if flag.away {
                if must_reset {
                    failure(world);
                } else {
                    return_flag(world, *f, true);
                    if let Some(i) = side(team) {
                        world.resource_mut::<Match>().returns[i] += 1;
                    }
                }
                break;
            }
        }
    }
    // Captures (after the moves).
    if !over {
        let flags: Vec<(Entity, HaloFlag)> = world.query::<(Entity, &HaloFlag)>().iter(world).map(|(e, f)| (e, f.clone())).collect();
        let n = world.get_resource::<Netgame>().cloned().unwrap_or_default();
        for (f, flag) in &flags {
            let Some(c) = flag.carrier else { continue };
            let Some(t) = world.get::<Transform>(c) else { continue };
            let origin = feet(t);
            let team = world.get::<Team>(c).map_or(0, |t| t.0);
            let own = flags.iter().find(|(_, o)| o.team == team);
            let goal = own.map(|(_, o)| o.home).or_else(|| home_of(&n, team, assault));
            let Some(goal) = goal else { continue };
            if origin.distance(goal) / WU >= CAPTURE_RADIUS {
                continue;
            }
            if at_home_to_score && own.is_some_and(|(_, o)| o.away) {
                failure(world);
                continue;
            }
            add_score(world, c, 1);
            world.write_message(HaloAnnounce { line: Line::Score(team), player: None });
            return_flag(world, *f, false);
        }
    }
    // Single flag's swap.
    if single > 0 {
        let due = world.get_resource::<CtfState>().and_then(|s| s.next_swap);
        if due.is_some_and(|d| now >= d) {
            world.get_resource_or_init::<CtfState>().next_swap = Some(now + single as i64);
            let n = world.get_resource::<Netgame>().cloned().unwrap_or_default();
            for (f, flag) in world.query::<(Entity, &HaloFlag)>().iter(world).map(|(e, f)| (e, f.clone())).collect::<Vec<_>>() {
                if flag.carrier.is_some() {
                    continue;
                }
                let team = if flag.team == RED { BLUE } else { RED };
                let Some(home) = home_of(&n, team, assault) else { continue };
                if let Some(mut fl) = world.get_mut::<HaloFlag>(f) {
                    fl.team = team;
                    fl.home = home;
                }
                return_flag(world, f, false);
            }
        }
    }
}

/// The carriers' limits (module doc): inputs a flag in hand refuses, and
/// no camouflage (before the weapons, server-side).
pub fn carriers(
    flags: Query<&HaloFlag>,
    mut units: Query<(&mut Intent, Option<&mut super::super::vitality::Vitality>)>,
    actions: Option<Res<crate::core::ActionRegistry>>,
) {
    let refused = actions.map_or(0, |a| a.bit(super::super::weapons::grenades::GRENADE_ACTION) | a.bit(super::super::weapons::systems::ZOOM_ACTION));
    for c in flags.iter().filter_map(|f| f.carrier) {
        let Ok((mut i, v)) = units.get_mut(c) else { continue };
        if i.fire || i.secondary || i.reload || i.actions & refused != 0 {
            i.fire = false;
            i.secondary = false;
            i.reload = false;
            i.actions &= !refused;
        }
        if let Some(mut v) = v
            && v.camouflaged
        {
            v.camouflaged = false;
            v.camo_ticks = 0;
        }
    }
}

/// A look at the flags for screenshots and play tests (the
/// `halo_ce:ctf_go` cvar, once, then cleared): `red` or `blue` puts the
/// local player 3 m from that team's flag spot towards the other's,
/// facing it; `carry_red` or
/// `carry_blue` puts it on the spot (an enemy then carries the flag).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CtfGo(pub String);

#[allow(clippy::type_complexity)]
pub fn ctf_go(
    mut go: ResMut<CtfGo>,
    flags: Query<&HaloFlag>,
    local: Query<Entity, (With<crate::core::LocalPlayer>, With<HaloPlayer>, Without<Dead>)>,
    mut commands: Commands,
) {
    if go.0.is_empty() {
        return;
    }
    let (team, carry) = match go.0.as_str() {
        "red" => (RED, false),
        "blue" => (BLUE, false),
        "carry_red" => (RED, true),
        "carry_blue" => (BLUE, true),
        _ => {
            go.0.clear();
            return;
        }
    };
    let (Some(flag), Ok(e)) = (flags.iter().find(|f| f.team == team), local.single()) else { return };
    let other = flags.iter().find(|f| f.team != team).map_or(flag.home + Vec3::Z, |f| f.home);
    let towards = ((other - flag.home) * Vec3::new(1.0, 0.0, 1.0)).normalize_or(Vec3::Z);
    let off = if carry { Vec3::ZERO } else { towards * 3.0 };
    let d = -towards;
    let look = Quat::from_euler(EulerRot::YXZ, (-d.x).atan2(-d.z), -0.3, 0.0);
    let at = Transform::from_translation(flag.home + off + Vec3::Y * centre_above_feet()).with_rotation(look);
    commands.queue(move |w: &mut World| {
        chimera_sim::spawning::revive(w, e, Some(at));
    });
    go.0.clear();
}
