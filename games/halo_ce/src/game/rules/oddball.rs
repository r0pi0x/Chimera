//! Oddball (specs/halo_ce/multiplayer_rules.md 6.5): the ball.
//!
//! - **Ball type 0 (normal)**: ball n (0 to the ball count - 1) appears
//!   450 x (n + 1) ticks after the start at the oddball flag (type 2) whose
//!   team index is n (random start: a random ball flag), a `HaloBall` every
//!   machine sees (its model through `NetItem`). A living player touching a
//!   ball not carried takes it; a carrier who dies drops it where it was;
//!   a ball not carried, away from its spot and not held for more than
//!   1200 ticks resets to its spot ("ball reset" if it had been touched).
//! - **Types 1 (reverse tag) and 2 (juggernaut)**: no object; the balls
//!   are a status ("it", `OddballState::holders`), all of them free from
//!   the start. A kill of the holder passes its balls to an enemy killer
//!   (else they go free); an enemy kill while a ball is free makes the
//!   killer "it" and scores 1.
//! - **Scoring**: types 0 and 1: each carrier 1 per tick per ball while the
//!   game can score, the team the sum (the score to win in minutes); type
//!   2: a kill of the holder or by the holder gives the killer +1 (the score
//!   to win in kills).
//! - **The carrier** (`carriers`): its speed x 0.75 slow, 1.0 normal, 1.25
//!   fast (the movement's `SpeedMultiplier`); the traits with and without
//!   the ball: invisible (camouflage that never lapses), extra damage (x 1.5
//!   dealt) and damage resistant (x 0.5 taken: `trait_factor`); a carrier
//!   without the invisible trait loses camouflage every tick; with the ball
//!   in hand it cannot fire, zoom, reload or throw (the server clears those
//!   inputs, as for the flag).
//!
//! Known shortcuts: the 60 s and 30 s warnings and the chimes are not
//! played; types 1 and 2 have no marker on the holder; the carrier's own
//! client predicts its refused shots.

use bevy::prelude::*;
use blam_formats::{
    game_type::{BallSpeed, BallType, GameTypeOptions, OddballTrait, Rules},
    scenario::NetgameFlagType,
};
use chimera_sim::{
    mode::{Match, side},
    weapon::projectile::ShownBody,
};

use super::{
    HaloGameType, HaloMatch, HaloPlayer,
    netgame::Netgame,
    scoring::{HaloAnnounce, Line, add_score},
    spawning::{centre_above_feet, feet},
};
use crate::{
    core::{Dead, Intent, Score, Team},
    map::loose::ShownItem,
};

/// The ball's tag (the globals' multiplayer information: 11.1).
pub const BALL_TAG: &str = "weapons\\ball\\ball";
/// Ticks between the balls' appearances (ball n at 450 x (n + 1)).
pub const BALL_STEP: i64 = 450;
/// Ticks a ball waits before it resets (more than this).
pub const RESET_TICKS: i64 = 1200;

/// The oddball options (6.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub random_start: bool,
    pub speed: BallSpeed,
    pub with_ball: OddballTrait,
    pub without_ball: OddballTrait,
    pub ball_type: BallType,
    pub balls: i32,
}

pub fn options(r: &Rules) -> Option<Options> {
    match r.options {
        GameTypeOptions::Oddball { random_start, speed_with_ball, trait_with_ball, trait_without_ball, ball_type, ball_spawn_count, .. } => {
            Some(Options { random_start, speed: speed_with_ball, with_ball: trait_with_ball, without_ball: trait_without_ball, ball_type, balls: ball_spawn_count })
        }
        _ => None,
    }
}

/// The carrier's speed factor.
pub fn speed_factor(s: BallSpeed) -> f32 {
    match s {
        BallSpeed::Slow => 0.75,
        BallSpeed::Normal => 1.0,
        BallSpeed::Fast => 1.25,
    }
}

/// A ball of type 0: its number, its spot (feet, metres), its carrier,
/// the tick it was last held, whether anyone touched it.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct HaloBall {
    pub number: i16,
    pub home: Vec3,
    pub carrier: Option<Entity>,
    pub last_held: i64,
    pub touched: bool,
}

/// The oddball's state: the next ball to appear (type 0), the holders of
/// the balls of types 1 and 2.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct OddballState {
    pub next_ball: i32,
    pub holders: Vec<Option<Entity>>,
}

/// The damage factor of the traits on a hit (4.7): `by` whether the
/// attacker (a player) holds a ball, `target_holds` whether the target
/// does; extra damage x 1.5 dealt, damage resistant x 0.5 taken.
pub fn trait_factor(o: &Options, by: Option<bool>, target_holds: bool) -> f32 {
    let traits = |has: bool| if has { o.with_ball } else { o.without_ball };
    let mut s = 1.0;
    if by.is_some_and(|h| traits(h) == OddballTrait::ExtraDamage) {
        s *= 1.5;
    }
    if traits(target_holds) == OddballTrait::DamageResistant {
        s *= 0.5;
    }
    s
}

/// The balls at the game's start: types 1 and 2 all free; type 0 none yet.
pub fn setup(world: &mut World) {
    clear(world);
    let Some(o) = options(&world.resource::<HaloGameType>().0.rules) else { return };
    let holders = if o.ball_type == BallType::Normal { Vec::new() } else { vec![None; o.balls.max(0) as usize] };
    world.insert_resource(OddballState { next_ball: 0, holders });
}

/// The balls gone (another game begins).
pub fn clear(world: &mut World) {
    let old: Vec<Entity> = world.query_filtered::<Entity, With<HaloBall>>().iter(world).collect();
    for e in old {
        world.despawn(e);
    }
    world.remove_resource::<OddballState>();
}

/// A kill in oddball types 1 and 2 (module doc), from the kill handler.
pub fn kill(world: &mut World, credited: Option<Entity>, victim: Entity, friendly: bool) {
    let Some(o) = options(&world.resource::<HaloGameType>().0.rules) else { return };
    if o.ball_type == BallType::Normal {
        return;
    }
    let enemy = credited.filter(|k| !friendly && *k != victim);
    let Some(mut state) = world.get_resource::<OddballState>().cloned() else { return };
    let held: Vec<usize> = (0..state.holders.len()).filter(|i| state.holders[*i] == Some(victim)).collect();
    let killer_holds = enemy.is_some_and(|k| state.holders.contains(&Some(k)));
    for i in &held {
        state.holders[*i] = enemy;
    }
    let mut point = false;
    if let Some(k) = enemy {
        if !held.is_empty() || killer_holds {
            point = o.ball_type == BallType::Juggernaut;
        } else if let Some(i) = state.holders.iter().position(|h| h.is_none()) {
            state.holders[i] = Some(k);
            point = true;
        }
    }
    world.insert_resource(state);
    if point && let Some(k) = enemy {
        add_score(world, k, 1);
    }
}

/// One Halo tick of oddball (module doc).
pub fn tick(world: &mut World) {
    let Some(o) = options(&world.resource::<HaloGameType>().0.rules) else { return };
    let (now, over) = {
        let m = world.resource::<HaloMatch>();
        (m.time, m.over)
    };
    if o.ball_type == BallType::Normal {
        balls(world, &o, now);
    }
    if over || o.ball_type == BallType::Juggernaut {
        return;
    }
    // Time scores: 1 per tick per ball held.
    let mut holders: Vec<Entity> = world.query::<&HaloBall>().iter(world).filter_map(|b| b.carrier).collect();
    if let Some(s) = world.get_resource::<OddballState>() {
        holders.extend(s.holders.iter().flatten());
    }
    for e in holders {
        if world.get::<Dead>(e).is_some() {
            continue;
        }
        let team = world.get::<Team>(e).map_or(0, |t| t.0);
        if let Some(mut p) = world.get_mut::<HaloPlayer>(e) {
            p.score += 1;
        }
        if let Some(mut s) = world.get_mut::<Score>(e) {
            s.kills += 1;
        }
        if let Some(i) = side(team) {
            world.resource_mut::<Match>().score[i] += 1;
        }
    }
}

/// Type 0's balls: appearing, dropped, taken, reset.
fn balls(world: &mut World, o: &Options, now: i64) {
    let n = world.get_resource::<Netgame>().cloned().unwrap_or_default();
    let spots: Vec<(i16, Vec3)> = n.flags_of(NetgameFlagType::OddballBallSpawn).map(|f| (f.team, f.at)).collect();
    // Appearing.
    let next = world.get_resource::<OddballState>().map_or(0, |s| s.next_ball);
    if next < o.balls && now >= BALL_STEP * (next as i64 + 1) {
        world.get_resource_or_init::<OddballState>().next_ball = next + 1;
        let spot = if o.random_start && !spots.is_empty() {
            let i = world.resource_mut::<HaloMatch>().roll(spots.len() as u32) as usize;
            Some(spots[i].1)
        } else {
            spots.iter().find(|(t, _)| *t == next as i16).map(|(_, p)| *p)
        };
        if let Some(home) = spot {
            world.spawn((
                Name::new(format!("halo_ce ball {next}")),
                HaloBall { number: next as i16, home, carrier: None, last_held: now, touched: false },
                Transform::from_translation(home),
                ShownItem(super::super::view::load::item_key(BALL_TAG)),
                ShownBody { owner: None },
            ));
            world.write_message(HaloAnnounce { line: Line::PlayBall, player: None });
        }
    }
    let reach = world
        .get_resource::<super::super::weapons::grenades::GrenadeData>()
        .map_or(0.3, |g| g.explosions.biped_bounding_radius * super::netgame::WU);
    let units: Vec<(Entity, Vec3)> = world
        .query_filtered::<(Entity, &Transform), (With<HaloPlayer>, Without<Dead>)>()
        .iter(world)
        .map(|(e, t)| (e, t.translation))
        .collect();
    let all: Vec<(Entity, HaloBall)> = world.query::<(Entity, &HaloBall)>().iter(world).map(|(e, b)| (e, b.clone())).collect();
    for (b, ball) in all {
        if let Some(c) = ball.carrier {
            match units.iter().find(|(e, _)| *e == c) {
                Some((_, centre)) => {
                    let yaw = world.get::<Intent>(c).map_or(0.0, |i| i.yaw);
                    if let Some(mut t) = world.get_mut::<Transform>(b) {
                        t.translation = *centre + Quat::from_rotation_y(yaw) * Vec3::new(0.25, -0.1, -0.35);
                    }
                    world.get_mut::<HaloBall>(b).unwrap().last_held = now;
                }
                None => {
                    let at = world.get::<Transform>(c).map(feet);
                    let mut x = world.get_mut::<HaloBall>(b).unwrap();
                    x.carrier = None;
                    x.last_held = now;
                    if let (Some(at), Some(mut t)) = (at, world.get_mut::<Transform>(b)) {
                        t.translation = at;
                    }
                }
            }
            continue;
        }
        let at = world.get::<Transform>(b).map_or(ball.home, |t| t.translation);
        if at.distance(ball.home) > 0.01 && now - ball.last_held > RESET_TICKS {
            if let Some(mut t) = world.get_mut::<Transform>(b) {
                t.translation = ball.home;
            }
            if ball.touched {
                world.write_message(HaloAnnounce { line: Line::BallReset, player: None });
            }
            continue;
        }
        if let Some((e, _)) = units.iter().find(|(_, c)| c.distance(at + Vec3::Y * centre_above_feet()) <= reach) {
            let mut x = world.get_mut::<HaloBall>(b).unwrap();
            x.carrier = Some(*e);
            x.touched = true;
            x.last_held = now;
        }
    }
}

/// The carriers' and the others' speed and traits each tick, and the inputs
/// a ball in hand refuses (module doc; server-side, before the weapons).
#[allow(clippy::type_complexity)]
pub fn carriers(
    game: Res<HaloGameType>,
    balls: Query<&HaloBall>,
    state: Option<Res<OddballState>>,
    mut players: Query<
        (Entity, &mut Intent, Option<&mut super::super::movement::SpeedMultiplier>, Option<&mut super::super::vitality::Vitality>),
        (With<HaloPlayer>, Without<Dead>),
    >,
    actions: Option<Res<crate::core::ActionRegistry>>,
) {
    let Some(o) = options(&game.0.rules) else { return };
    let refused = actions.map_or(0, |a| a.bit(super::super::weapons::grenades::GRENADE_ACTION) | a.bit(super::super::weapons::systems::ZOOM_ACTION));
    let holding = |e: Entity| balls.iter().any(|b| b.carrier == Some(e)) || state.as_ref().is_some_and(|s| s.holders.contains(&Some(e)));
    for (e, mut i, speed, v) in &mut players {
        let has = holding(e);
        let want = if has { speed_factor(o.speed) } else { 1.0 };
        if let Some(mut m) = speed
            && m.0 != want
        {
            m.0 = want;
        }
        let traits = if has { o.with_ball } else { o.without_ball };
        if let Some(mut v) = v {
            if traits == OddballTrait::Invisible {
                v.camouflaged = true;
                v.camo_ticks = v.camo_ticks.max(15);
            } else if has && v.camouflaged {
                v.camouflaged = false;
                v.camo_ticks = 0;
            }
        }
        if has && o.ball_type == BallType::Normal && (i.fire || i.secondary || i.reload || i.actions & refused != 0) {
            i.fire = false;
            i.secondary = false;
            i.reload = false;
            i.actions &= !refused;
        }
    }
}
