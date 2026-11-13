//! `points = "scored"`: spawning anywhere on the map's walkable floor at
//! the best of a scored sample of spots, out of the enemies' view
//! (Call of Duty's way in shape; `pick`, called by `runtime::spawn_point`).
//!
//! - Candidates: the map's spawn spots (`spawns::SpawnSpots`, the same as
//!   `random_hidden`'s), `samples` of them a spawn from a seeded random
//!   place in their (shuffled) order.
//! - Score (`base_score`, `sight_penalty`, `choose`; the weights are data,
//!   chimera/sim/data/spawn_scoring.toml, tuned by a definition's
//!   `[spawn.scoring]`, `ScoringSettings`): each living enemy whose eye
//!   has a clear line to the spawned player's head or centre takes `seen`
//!   off, all of it inside its field of view (`fov`, from its look), down
//!   to `behind` of it straight behind; the nearest enemy under `near` m
//!   costs `close` (twice at 0 m), past `far` up to `far_penalty`; each
//!   death and each player's last shot of the last `danger_memory` s
//!   costs up to `danger` within `danger_radius`, fading with age and
//!   distance (`Memory`, `remember_danger`); the player's own last spawn
//!   and its death place cost `repeat` within `repeat_radius`; a living
//!   friend near (a mode with teams; never free for all) adds up to
//!   `friend`; a seeded `jitter` breaks ties. Every candidate gets the
//!   cheap terms; then the lines of sight only for the best by them until
//!   no unchecked one can beat the best found (sight only takes off), the
//!   enemies' lines culled first by the map's visibility data where it has
//!   some (cluster to cluster). Never fails: the least bad spot wins.
//! - Facing: the definition's `[spawn] facing` (`spawns::face`, toward
//!   open space), each direction's openness weighed down by up to
//!   `face_away` the more it looks at the nearest enemy, and to almost
//!   nothing within half the field of view (and 10 degrees) of an enemy
//!   in sight of the spot: a spawn that had to be in sight looks away.
//! - A match's start (`begin_batch`, at the flow's mission start, then the
//!   first spawn of the batch, `plan`): every player placed one at a time
//!   in a seeded order, each scored against everyone placed before it
//!   (where it will stand and face) and the living characters outside the
//!   match, so the opening spawns spread out and don't see each other;
//!   bots and humans alike. The spawns of that tick then take their
//!   placements. Players joining (taken in this tick, `runtime::join`) are
//!   no one's enemies until placed.
//! - Determinism: the draws are `spawns::SpawnDraws`' seeded generator
//!   (the order, the sample's start, the jitter, the facing's turn);
//!   enemies are walked in entity order. The state lives on `SpawnDraws`
//!   (no resource of its own), which a match's restart removes.
//!
//! Known shortcuts: sight is two rays per enemy (no field of view's
//! aspect, fog, or smoke); an enemy's view is where it looks at the
//! spawn, not where it will turn; the danger of a shot is where it was
//! fired from, not where it went.

use avian3d::prelude::*;
use bevy::{
    ecs::{message::MessageCursor, system::SystemState},
    prelude::*,
};
use chimera_core::query_cache::cached_query;
use serde::Deserialize;

use super::{
    ActiveMinigame, MinigameDef, runtime,
    spawns::{self, EYE, SpawnDraws, SpawnSpots},
};
use crate::{
    core::{Connecting, Dead, Died, Health, Intent, Npc, SOLID_LAYERS, SimClock, Spectating, Team},
    map::{SPAWN_LIFT, vis::ActiveVisibility},
    weapon::{WeaponEvent, WeaponEventKind},
};

/// The built-in settings (chimera/sim/data/spawn_scoring.toml).
pub const DATA: &str = include_str!("../../data/spawn_scoring.toml");

/// A standing player's centre and head above its feet (m): the targets
/// of an enemy's sight and the point distances are measured from.
pub const CENTRE: f32 = 1.0;
pub const HEAD: f32 = 1.7;

/// `points = "scored"`'s weights and thresholds (module docs; the data
/// file says each).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoringSettings {
    pub samples: usize,
    pub seen: f32,
    pub fov: f32,
    pub behind: f32,
    pub near: f32,
    pub close: f32,
    pub far: f32,
    pub far_penalty: f32,
    pub danger: f32,
    pub danger_radius: f32,
    #[serde(deserialize_with = "super::def::seconds")]
    pub danger_memory: f32,
    pub repeat: f32,
    pub repeat_radius: f32,
    pub friend: f32,
    pub friend_radius: f32,
    pub jitter: f32,
    pub face_away: f32,
}

impl Default for ScoringSettings {
    fn default() -> Self {
        Self::with(&toml::Table::new()).expect("the built-in spawn scoring reads")
    }
}

impl ScoringSettings {
    /// `DATA` with `t`'s keys in place.
    pub fn with(t: &toml::Table) -> Result<Self, String> {
        let mut base: toml::Table = toml::from_str(DATA).map_err(|e| e.to_string())?;
        for (k, v) in t {
            base.insert(k.clone(), v.clone());
        }
        let s: Self = toml::Value::Table(base)
            .try_into()
            .map_err(|e: toml::de::Error| e.to_string())?;
        if s.samples == 0 {
            return Err("samples = 0: at least 1".into());
        }
        if !(0.0..=360.0).contains(&s.fov) || !(0.0..=1.0).contains(&s.behind) {
            return Err(format!("fov {} (0..360) or behind {} (0..1)", s.fov, s.behind));
        }
        Ok(s)
    }

    /// `def`'s settings (its `[spawn.scoring]` over `DATA`).
    pub fn of(def: &MinigameDef) -> Result<Self, String> {
        Self::with(def.spawn.scoring.as_ref().unwrap_or(&toml::Table::new()))
    }
}

/// What the scoring remembers over a match (on `SpawnDraws`).
#[derive(Clone, Debug, Default)]
pub struct Memory {
    /// The active definition's settings, read once.
    settings: Option<ScoringSettings>,
    /// Deaths: where and when (`SimClock`).
    deaths: Vec<(Vec3, f64)>,
    /// Each shooter's last shot: from where and when.
    shots: Vec<(Entity, Vec3, f64)>,
    /// Each player's last spawn (feet).
    last: Vec<(Entity, Vec3)>,
    /// A batch of spawns begins (`begin_batch`): the next spawn plans it.
    batch: bool,
    /// The batch's placements and the time they were made for.
    plan: Vec<(Entity, Transform)>,
    plan_at: f64,
}

/// A living enemy as the scoring sees it: its centre and the unit
/// direction it looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewer {
    pub centre: Vec3,
    pub look: Vec3,
}

impl Viewer {
    pub fn eye(&self) -> Vec3 {
        self.centre + Vec3::Y * EYE
    }
}

/// What a spot is scored against.
#[derive(Clone, Copy, Debug, Default)]
pub struct Scene<'a> {
    pub enemies: &'a [Viewer],
    /// Living friends' centres (a mode with teams).
    pub friends: &'a [Vec3],
    /// Recent danger: where, and how old (s).
    pub dangers: &'a [(Vec3, f32)],
    /// The player's own last spawn and death place (feet).
    pub avoid: &'a [Vec3],
}

/// The direction a look of `yaw`, `pitch` (radians, `Intent`'s) points.
pub fn look_dir(yaw: f32, pitch: f32) -> Vec3 {
    Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0) * Vec3::NEG_Z
}

/// The share of `seen` an enemy looking along `look` from `eye` costs a
/// target at `to`: 1 inside the field of view, falling linearly with the
/// angle to `behind` straight behind.
pub fn view_weight(s: &ScoringSettings, look: Vec3, eye: Vec3, to: Vec3) -> f32 {
    let dir = (to - eye).normalize_or_zero();
    let angle = look.dot(dir).clamp(-1.0, 1.0).acos().to_degrees();
    let half = s.fov / 2.0;
    if angle <= half || half >= 180.0 {
        1.0
    } else {
        1.0 - (1.0 - s.behind) * ((angle - half) / (180.0 - half)).min(1.0)
    }
}

/// The terms of a spot's score that need no line of sight: distance to
/// the nearest enemy, danger, repeats, friends.
pub fn base_score(s: &ScoringSettings, feet: Vec3, scene: &Scene) -> f32 {
    let at = feet + Vec3::Y * CENTRE;
    let mut score = 0.0;
    let d = scene
        .enemies
        .iter()
        .map(|v| v.centre.distance(at))
        .fold(f32::INFINITY, f32::min);
    if d < s.near {
        score -= s.close * (1.0 + (s.near - d) / s.near.max(1e-3));
    } else if d.is_finite() && d > s.far {
        score -= s.far_penalty * ((d - s.far) / s.far.max(1e-3)).min(1.0);
    }
    if s.danger_memory > 0.0 && s.danger_radius > 0.0 {
        for (p, age) in scene.dangers {
            let dd = p.distance(at);
            if *age < s.danger_memory && dd < s.danger_radius {
                score -= s.danger * (1.0 - age / s.danger_memory) * (1.0 - dd / s.danger_radius);
            }
        }
    }
    if scene.avoid.iter().any(|a| a.distance(feet) < s.repeat_radius) {
        score -= s.repeat;
    }
    let f = scene
        .friends
        .iter()
        .map(|c| c.distance(at))
        .fold(f32::INFINITY, f32::min);
    if f < s.friend_radius {
        score += s.friend * (1.0 - f / s.friend_radius);
    }
    score
}

/// What the enemies' sight takes off a spot at `feet`: `seen` times each
/// seeing enemy's `view_weight`. `sees(i, target)`: whether enemy `i`'s
/// eye has a clear line to `target`. Stops once it reaches `enough`.
pub fn sight_penalty(
    s: &ScoringSettings,
    feet: Vec3,
    enemies: &[Viewer],
    enough: f32,
    sees: &mut impl FnMut(usize, Vec3) -> bool,
) -> f32 {
    let (head, centre) = (feet + Vec3::Y * HEAD, feet + Vec3::Y * CENTRE);
    let mut pen = 0.0;
    for (i, v) in enemies.iter().enumerate() {
        if pen >= enough {
            break;
        }
        if sees(i, head) || sees(i, centre) {
            pen += s.seen * view_weight(s, v.look, v.eye(), centre);
        }
    }
    pen
}

/// The best of `spots` for `scene` (its index and score; None without
/// spots): every spot's `base_score` plus `jitter` (a draw in 0..1 each,
/// in order) times the setting, then `sight_penalty` for the best of
/// them, best first, until none left could beat the best found.
pub fn choose(
    s: &ScoringSettings,
    spots: &[Vec3],
    scene: &Scene,
    jitter: &mut impl FnMut() -> f32,
    sees: &mut impl FnMut(usize, Vec3) -> bool,
) -> Option<(usize, f32)> {
    let mut rated: Vec<(f32, usize)> = spots
        .iter()
        .enumerate()
        .map(|(i, p)| (base_score(s, *p, scene) + s.jitter * jitter(), i))
        .collect();
    rated.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut best: Option<(f32, usize)> = None;
    for (b, i) in rated {
        if best.is_some_and(|(top, _)| b <= top) {
            break;
        }
        let enough = best.map_or(f32::INFINITY, |(top, _)| b - top);
        let score = b - sight_penalty(s, spots[i], scene.enemies, enough, sees);
        if best.is_none_or(|(top, _)| score > top) {
            best = Some((score, i));
        }
    }
    best.map(|(score, i)| (i, score))
}

/// The scoring's settings, read once a match from the active definition.
fn settings(world: &World, draws: &mut SpawnDraws) -> ScoringSettings {
    if let Some(s) = draws.memory.settings {
        return s;
    }
    let s = world
        .get_resource::<ActiveMinigame>()
        .and_then(|a| ScoringSettings::of(a.def).ok())
        .unwrap_or_default();
    draws.memory.settings = Some(s);
    s
}

/// A batch of spawns begins (every player respawned at a match's or a
/// round's start): the next spawn places them all (module docs).
pub fn begin_batch(world: &mut World) {
    let mut draws = world
        .remove_resource::<SpawnDraws>()
        .unwrap_or_else(|| SpawnDraws::new(world));
    draws.memory.batch = true;
    draws.memory.plan.clear();
    world.insert_resource(draws);
}

/// The characters the scoring may count: living, in the game.
type Living = (With<Intent>, Without<Dead>, Without<Connecting>, Without<Spectating>);

/// A living character in the match (or an NPC): its centre, look, team.
/// One the minigame is taking in this tick isn't (`living`).
struct Other {
    e: Entity,
    centre: Vec3,
    look: Vec3,
    team: Option<Team>,
}

fn living(world: &mut World) -> Vec<Other> {
    let mut out = cached_query::<
        (
            Entity,
            &Transform,
            &Health,
            &Intent,
            Option<&Team>,
            Has<runtime::InMinigame>,
            Has<Npc>,
        ),
        Living,
        _,
    >(world, |q, world| {
        let mut out = Vec::new();
        for (e, t, h, i, team, playing, npc) in q.iter(world) {
            // A character the minigame is about to take in stands
            // where it will not be.
            if h.current > 0.0 && (playing || npc) {
                out.push(Other {
                    e,
                    centre: t.translation,
                    look: look_dir(i.yaw, i.pitch),
                    team: team.copied(),
                });
            }
        }
        out
    });
    out.sort_by_key(|o| o.e);
    out
}

/// `e`'s spawn by score (module docs); None when the map has no spawn
/// point at all.
pub fn pick(world: &mut World, e: Entity, facing: super::def::Facing) -> Option<Transform> {
    let points = spawns::spawn_feet(world);
    if points.is_empty() {
        return None;
    }
    spawns::ensure_spots(world, &points);
    let now = world.get_resource::<SimClock>().map_or(0.0, |c| c.now);
    let mut draws = world
        .remove_resource::<SpawnDraws>()
        .unwrap_or_else(|| SpawnDraws::new(world));
    let s = settings(world, &mut draws);
    if draws.memory.batch {
        draws.memory.batch = false;
        let players: Vec<Entity> = runtime::players(world).into_iter().map(|(p, _)| p).collect();
        draws.memory.plan = plan(world, &mut draws, &s, &players, &facing, now);
        draws.memory.plan_at = now;
    }
    let planned = (draws.memory.plan_at == now)
        .then(|| draws.memory.plan.iter().position(|(p, _)| *p == e))
        .flatten()
        .map(|k| draws.memory.plan.swap_remove(k).1);
    let at = match planned {
        Some(t) => Some(t),
        None => {
            let others = living(world);
            let team = world.get::<Team>(e).copied();
            let mut avoid: Vec<Vec3> = Vec::new();
            if world.get::<Dead>(e).is_some()
                && let Some(t) = world.get::<Transform>(e)
            {
                avoid.push(t.translation - Vec3::Y * SPAWN_LIFT);
            }
            let views: Vec<(Option<Team>, Viewer)> = others
                .iter()
                .filter(|o| o.e != e)
                .map(|o| {
                    (
                        o.team,
                        Viewer {
                            centre: o.centre,
                            look: o.look,
                        },
                    )
                })
                .collect();
            place(world, &mut draws, &s, e, team, &views, avoid, &facing, now)
        }
    };
    if let Some(t) = at {
        let feet = t.translation - Vec3::Y * SPAWN_LIFT;
        match draws.memory.last.iter_mut().find(|(p, _)| *p == e) {
            Some(l) => l.1 = feet,
            None => draws.memory.last.push((e, feet)),
        }
    }
    world.insert_resource(draws);
    at
}

/// The batch's placements: `players` one at a time in a seeded order,
/// each against those placed before it and the living characters outside
/// the batch.
fn plan(
    world: &mut World,
    draws: &mut SpawnDraws,
    s: &ScoringSettings,
    players: &[Entity],
    facing: &super::def::Facing,
    now: f64,
) -> Vec<(Entity, Transform)> {
    let mut order = players.to_vec();
    for i in (1..order.len()).rev() {
        let j = ((draws.rng.next() * (i + 1) as f32) as usize).min(i);
        order.swap(i, j);
    }
    let mut views: Vec<(Option<Team>, Viewer)> = living(world)
        .into_iter()
        .filter(|o| !players.contains(&o.e))
        .map(|o| {
            (
                o.team,
                Viewer {
                    centre: o.centre,
                    look: o.look,
                },
            )
        })
        .collect();
    let mut out = Vec::new();
    for p in order {
        let team = world.get::<Team>(p).copied();
        let avoid: Vec<Vec3> = draws
            .memory
            .last
            .iter()
            .filter(|(q, _)| *q == p)
            .map(|(_, f)| *f)
            .collect();
        let Some(t) = place(world, draws, s, p, team, &views, avoid, facing, now) else {
            continue;
        };
        let (yaw, pitch, _) = t.rotation.to_euler(EulerRot::YXZ);
        views.push((
            team,
            Viewer {
                centre: t.translation,
                look: look_dir(yaw, pitch),
            },
        ));
        out.push((p, t));
    }
    out
}

/// One spawn for player `e` (on `team`) among `views` (every other living
/// character, by team): the best of a sample of the spots, faced.
#[allow(clippy::too_many_arguments)]
fn place(
    world: &mut World,
    draws: &mut SpawnDraws,
    s: &ScoringSettings,
    e: Entity,
    team: Option<Team>,
    views: &[(Option<Team>, Viewer)],
    mut avoid: Vec<Vec3>,
    facing: &super::def::Facing,
    now: f64,
) -> Option<Transform> {
    avoid.extend(draws.memory.last.iter().filter(|(p, _)| *p == e).map(|(_, f)| *f));
    let enemies: Vec<Viewer> = views
        .iter()
        .filter(|(t, _)| team.is_none() || *t != team)
        .map(|(_, v)| *v)
        .collect();
    let friends: Vec<Vec3> = views
        .iter()
        .filter(|(t, _)| team.is_some() && *t == team)
        .map(|(_, v)| v.centre)
        .collect();
    let memory = s.danger_memory as f64;
    draws.memory.deaths.retain(|(_, t)| now - t < memory);
    draws.memory.shots.retain(|(_, _, t)| now - t < memory);
    let dangers: Vec<(Vec3, f32)> = draws
        .memory
        .deaths
        .iter()
        .map(|(p, t)| (*p, (now - t) as f32))
        .chain(draws.memory.shots.iter().map(|(_, p, t)| (*p, (now - t) as f32)))
        .collect();
    let spots = world.get_resource::<SpawnSpots>()?;
    let n = spots.spots.len();
    if n == 0 {
        return None;
    }
    let k = s.samples.min(n);
    let start = ((draws.rng.next() * n as f32) as usize).min(n - 1);
    let sample: Vec<Vec3> = (0..k).map(|i| spots.spots[(start + i) % n]).collect();
    let centroid = spots.spots.iter().sum::<Vec3>() / n as f32;
    let vis = world
        .get_resource::<ActiveVisibility>()
        .map(|v| v.0.clone())
        .filter(|v| !v.nodes.is_empty());
    let characters: Vec<Entity> = cached_query::<Entity, With<Intent>, _>(world, |q, world| q.iter(world).collect());
    let mut state = SystemState::<SpatialQuery>::new(world);
    let q = state.get(world).ok()?;
    let sight = SpatialQueryFilter::from_mask(SOLID_LAYERS).with_excluded_entities(characters);
    let eyes: Vec<(Vec3, Option<u32>)> = enemies
        .iter()
        .map(|v| (v.eye(), vis.as_ref().and_then(|m| m.cluster_at(v.eye()))))
        .collect();
    let mut sees = |i: usize, target: Vec3| {
        let (eye, cluster) = eyes[i];
        if let (Some(m), Some(c)) = (vis.as_ref(), cluster)
            && let Some(t) = m.cluster_at(target)
            && !m.sees(c, t)
        {
            return false;
        }
        let d = target - eye;
        let len = d.length();
        match Dir3::new(d) {
            Ok(dir) => q.cast_ray(eye, dir, (len - 0.05).max(0.0), true, &sight).is_none(),
            Err(_) => true,
        }
    };
    let scene = Scene {
        enemies: &enemies,
        friends: &friends,
        dangers: &dangers,
        avoid: &avoid,
    };
    let rng = &mut draws.rng;
    let (i, _) = choose(s, &sample, &scene, &mut || rng.next(), &mut sees)?;
    let feet = sample[i];
    let at = feet + Vec3::Y * CENTRE;
    let nearest = enemies
        .iter()
        .map(|v| v.centre)
        .min_by(|a, b| a.distance(at).total_cmp(&b.distance(at)));
    // The enemies in sight of the spot: not to be looked at.
    let eye = feet + Vec3::Y * HEAD;
    let seen: Vec<Vec3> = (0..enemies.len())
        .filter(|i| sees(*i, eye) || sees(*i, at))
        .map(|i| enemies[i].centre)
        .collect();
    let away = spawns::Away {
        nearest: nearest.filter(|_| s.face_away > 0.0),
        share: s.face_away,
        seen,
        cone: s.fov / 2.0 + 10.0,
    };
    let (yaw, pitch) = spawns::face(&q, &sight, feet, facing, Some(centroid), Some(&away), &mut draws.rng);
    Some(
        Transform::from_translation(feet + Vec3::Y * SPAWN_LIFT).with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            yaw,
            pitch,
            0.0,
        )),
    )
}

/// Deaths and shots into the scoring's memory (while a minigame spawns
/// by score and its draws are made).
pub fn remember_danger(
    mut shots: Local<MessageCursor<WeaponEvent>>,
    mut died: Local<MessageCursor<Died>>,
    weapon_events: Option<Res<Messages<WeaponEvent>>>,
    deaths: Option<Res<Messages<Died>>>,
    at: Query<&Transform>,
    clock: Res<SimClock>,
    draws: Option<ResMut<SpawnDraws>>,
) {
    let (Some(mut draws), now) = (draws, clock.now) else {
        if let Some(m) = weapon_events.as_deref() {
            shots.clear(m);
        }
        if let Some(m) = deaths.as_deref() {
            died.clear(m);
        }
        return;
    };
    let memory = &mut draws.memory;
    if let Some(m) = weapon_events.as_deref() {
        for ev in shots.read(m) {
            if let WeaponEventKind::Fired { origin, .. } = ev.kind {
                match memory.shots.iter_mut().find(|(o, ..)| *o == ev.owner) {
                    Some(s) => (s.1, s.2) = (origin, now),
                    None => memory.shots.push((ev.owner, origin, now)),
                }
            }
        }
    }
    if let Some(m) = deaths.as_deref() {
        for d in died.read(m) {
            if let Ok(t) = at.get(d.entity) {
                memory.deaths.push((t.translation, now));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewer(centre: Vec3, look: Vec3) -> Viewer {
        Viewer {
            centre,
            look: look.normalize(),
        }
    }

    #[test]
    fn the_built_in_settings_read_and_a_definition_tunes_them() {
        let s = ScoringSettings::default();
        assert_eq!((s.samples, s.near, s.danger_memory), (96, 10.0, 5.0));
        let t: toml::Table = toml::from_str("near = 12\ndanger_memory = \"3s\"").unwrap();
        let tuned = ScoringSettings::with(&t).unwrap();
        assert_eq!((tuned.near, tuned.danger_memory, tuned.seen), (12.0, 3.0, s.seen));
        let bad: toml::Table = toml::from_str("nearr = 12").unwrap();
        assert!(ScoringSettings::with(&bad).is_err());
    }

    #[test]
    fn being_seen_costs_more_in_front_than_behind() {
        let s = ScoringSettings::default();
        let eye = Vec3::ZERO;
        let ahead = Vec3::new(0.0, 0.0, -20.0);
        assert_eq!(view_weight(&s, Vec3::NEG_Z, eye, ahead), 1.0);
        let behind = view_weight(&s, Vec3::Z, eye, ahead);
        assert!((behind - s.behind).abs() < 1e-4, "{behind}");
        let side = view_weight(&s, Vec3::X, eye, ahead);
        assert!(side > behind && side < 1.0, "{side}");
    }

    #[test]
    fn the_terms_rank_spots_as_meant() {
        let s = ScoringSettings::default();
        let enemy = [viewer(Vec3::new(0.0, 1.0, 0.0), Vec3::NEG_Z)];
        let scene = Scene {
            enemies: &enemy,
            ..default()
        };
        // Too near costs, more the nearer; the band costs nothing; very far a little.
        let at = |x: f32| base_score(&s, Vec3::new(x, 0.0, 0.0), &scene);
        assert!(at(3.0) < at(8.0) && at(8.0) < at(20.0));
        assert_eq!(at(20.0), 0.0);
        assert!(at(80.0) < 0.0 && at(80.0) > -s.far_penalty - 1e-3);
        // Danger fades with distance and age.
        let fresh = [(Vec3::new(20.0, 1.0, 0.0), 0.0)];
        let old = [(Vec3::new(20.0, 1.0, 0.0), 4.0)];
        let d = |dangers: &[(Vec3, f32)], x: f32| {
            base_score(
                &s,
                Vec3::new(x, 0.0, 0.0),
                &Scene {
                    enemies: &enemy,
                    dangers,
                    ..default()
                },
            )
        };
        assert!(d(&fresh, 20.0) < d(&old, 20.0) && d(&old, 20.0) < 0.0);
        assert!(d(&fresh, 20.0) < d(&fresh, 25.0));
        // One's own last spawn and death place cost; a friend near helps.
        let avoid = [Vec3::new(20.0, 0.0, 0.0)];
        let repeat = Scene {
            enemies: &enemy,
            avoid: &avoid,
            ..default()
        };
        assert_eq!(base_score(&s, Vec3::new(21.0, 0.0, 0.0), &repeat), -s.repeat);
        let friends = [Vec3::new(20.0, 1.0, 5.0)];
        let team = Scene {
            enemies: &enemy,
            friends: &friends,
            ..default()
        };
        assert!(base_score(&s, Vec3::new(20.0, 0.0, 0.0), &team) > 0.0);
    }

    #[test]
    fn choose_takes_the_hidden_spot_and_checks_sight_only_for_the_best() {
        let s = ScoringSettings {
            jitter: 0.0,
            ..default()
        };
        // An enemy at the origin looking down -z; spots 20 m off in four
        // directions; it sees everything but the spot behind a wall at +x.
        let enemy = [viewer(Vec3::new(0.0, 1.0, 0.0), Vec3::NEG_Z)];
        let spots = [
            Vec3::new(0.0, 0.0, -20.0),
            Vec3::new(0.0, 0.0, 20.0),
            Vec3::new(20.0, 0.0, 0.0),
            Vec3::new(-20.0, 0.0, 0.0),
            // Hidden but too near.
            Vec3::new(4.0, 0.0, 0.0),
        ];
        let scene = Scene {
            enemies: &enemy,
            ..default()
        };
        let mut checks = 0;
        let mut sees = |_: usize, t: Vec3| {
            checks += 1;
            t.x <= 1.0
        };
        let (i, score) = choose(&s, &spots, &scene, &mut || 0.0, &mut sees).unwrap();
        assert_eq!(i, 2, "the hidden spot in the band");
        assert_eq!(score, 0.0);
        // Everything seen: the least bad, behind the enemy.
        let mut all = |_: usize, _: Vec3| true;
        let (i, _) = choose(&s, &spots[..4], &scene, &mut || 0.0, &mut all).unwrap();
        assert_eq!(i, 1, "behind the enemy");
        // The near hidden spot was never checked: its cheap score can't win.
        let mut asked = Vec::new();
        let mut log = |_: usize, t: Vec3| {
            asked.push(t);
            t.x <= 1.0
        };
        choose(&s, &spots, &scene, &mut || 0.0, &mut log);
        assert!(asked.iter().all(|t| (t.x - 4.0).abs() > 0.1), "{asked:?}");
    }
}
