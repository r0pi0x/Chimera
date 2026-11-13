//! The bots' brain (specs/combat_arms/bots.md 1, 2, 4-10): the senses
//! (one sense pass per server frame: the nearest enemy's sight test, else
//! the first heard stimulus; the notice and hearing rolls while relaxed),
//! the goals as states (patrol, investigate, chase, attack, dodge), and
//! what an attacking bot does: face the aim point, wait the reaction
//! delay, fire bursts by the clip, the hit/miss cycle per shot, reload,
//! switch, knife, throw a grenade after its count of shots, attack moves,
//! dodges and crouching. Everything ends in the bot's `Intent` (aim, keys,
//! trigger) and its `Inventory::wanted`; a throw is the grenade spawned
//! with the hang-time solution (`grenades::spawn_grenade`).
//!
//! The corrections of 2026-10-22 are in the module doc of `bots.rs`:
//! `investigate` (the alarm, look-only, half-distance), `grenade_danger`,
//! `dead` (a fresh brain), `take_start_node` and `patrol` (stops),
//! `throw_step`, `chase_fire`, `weapon_view` and `aim_and_fire` (also the
//! mercenaries' aim).
//!
//! `post_stimuli` (after the weapons and damage): gunfire from every
//! `Fired` round of a gun (radius: the fitted weapon's fire sound,
//! `attachments::CaMods`), footsteps, the shot's bookkeeping (a new aim
//! for the next shot, the grenade count, letting go of the trigger), and
//! damage (a bot that hurt a player resets its grenade count; a bot hit
//! moves and may dodge). `camping` keeps each human's camping time (5.5).
//!
//! Units: positions in metres; the spec's distances in world units
//! (`inventory::UNIT` metres each) where the code names them so.

use std::collections::{HashMap, HashSet};

use avian3d::prelude::*;
use bevy::{
    ecs::{query::QueryState, system::SystemState},
    prelude::*,
};

use super::{
    BotSettings, BotState, CaBot, MercMind, Pending, Throw,
    chat::Event as Chat,
    data::{BotData, GunAi},
    map_ai::MapAi,
    math::{self, Choice, MOVES, Move},
    nav,
};
use super::super::{
    attachments::CaMods,
    grenades::{CaGrenade, SERVER_FRAME},
    inventory::UNIT,
    rules::{CaMatch, Phase, spawns::CaHeld},
};
use crate::{
    core::{Damage, Dead, FreezeTime, Health, Intent, MovementState, SimClock, Spectating, Team},
    map::{SeeThrough, nav::NavMesh, nav_gen, vis::ActiveVisibility},
    weapon::{Inventory, Magazine, Weapon, WeaponEvent, WeaponEventKind, grenade::Throwable},
};

/// Sight range (u), `SeeEnemyDistance`.
pub const SIGHT_RANGE: f32 = 3000.0;
/// Inside this (u) the field of view is 180° even when relaxed.
pub const NO_FOV_RADIUS: f32 = 256.0;
/// Relaxed field of view: dot with the eye's forward, and with up.
pub const RELAXED_DOT: f32 = 0.342;
pub const RELAXED_UP_DOT: f32 = 0.5;
/// The sight cooldown while patrolling (s).
pub const SIGHT_COOLDOWN: f64 = 1.5;
/// Hearing: the template's distance for footsteps (u), the footstep radius
/// per unit of surface noise, vertical windows.
pub const HEARING: f32 = 1500.0;
pub const FOOTSTEP_RADIUS: f32 = 256.0;
pub const FOOTSTEP_WINDOW: f32 = 128.0;
pub const GUNFIRE_WINDOW: f32 = 512.0;
/// The ally call's radius (u).
pub const ALLY_CALL: f32 = 512.0;
/// Stimuli lifetimes (s).
pub const GUNFIRE_LIFE: f64 = 1.0;
pub const FOOTSTEP_LIFE: f64 = 0.5;
pub const CALL_LIFE: f64 = 1.0;
/// The reaction re-arm gap (s).
pub const REARM_GAP: f64 = 5.0;
/// The investigate goal's decay time (s).
pub const INVESTIGATE_TIME: f64 = 10.0;
/// The bot's knife reach (u, 8.4: it closes in until within it, its
/// swings reach that far, `inventory::KnifeReach`).
pub const KNIFE_RANGE: f32 = 130.0;
/// The chase path's rotating offset (u).
pub const CHASE_OFFSET: f32 = 96.0;
/// The camping radius (u) and onset (s).
pub const CAMP_RADIUS: f32 = 120.0;
/// Crouch on engaging: chance, hold (s).
pub const CROUCH_CHANCE: f32 = 0.01;
pub const CROUCH_HOLD: f64 = 10.0;
/// Dodging fire: the check interval, the delay after a dodge (s), the
/// roll's distance (u), "aiming at me" both ways.
pub const DODGE_CHECK: f64 = 0.25;
pub const DODGE_DELAY: f64 = 3.0;
pub const ROLL: f32 = 140.0;
pub const AIMING_DOT: f32 = 0.95;
/// A teammate closer than this (u) isn't asked to crouch.
pub const CROUCH_ASK: f32 = 48.0;
/// The bot's radius for its moves (u): the fallback hull's.
pub const RADIUS: f32 = 15.0;
/// The grenade dodge: point at this times the blast radius; notice range
/// beyond the radius (u); moves needed before a new notice (u).
pub const DODGE_FACTOR: f32 = 1.1;
pub const DANGER_MARGIN: f32 = 100.0;
pub const DANGER_MOVED: f32 = 5.0;
/// The throw: hang time (s), gravity (u/s²).
pub const HANG_TIME: f32 = 0.5;
pub const GRAVITY: f32 = -1600.0;
/// The sight points (5.2, Open question 18): the target's object position
/// (its hull centre) and its `Head` node, the eye the bot's own hull x/z
/// at its `Head` node's height; the node's height above the centre is the
/// spec's measured standing pose (`Head` at 29.03 u), m. The aim base
/// point is the `Torso` node (5.14 u above the centre standing, Correction
/// 11), the line of fire starts at the bot's right side (`side_origin`).
pub const TORSO: f32 = 5.14 * UNIT;
pub const HEAD: f32 = 29.03 * UNIT;
pub const EYE: f32 = HEAD;
/// The line of fire's origin: one radius out to the right, a quarter of
/// the hull's height above its centre (Correction 11), u.
pub const SIDE_UP: f32 = 25.0;
/// Heard sounds (5.3 Corrections 2, 3): a sound is only looked at within
/// this (u) and more than this long (s) after the last relaxed moment (the
/// spawn, or an investigation or chase ending).
pub const LOOK_ONLY_RANGE: f32 = 1000.0;
pub const LOOK_ONLY_GRACE: f64 = 30.0;
/// The grenade dodge ends at the latest this long after it began (s);
/// otherwise when the grenade is gone or the move ends (Correction 4).
pub const DODGE_MAX: f64 = 5.0;
/// Chasing, the torso turns toward the target's head this far (rad) from
/// the run heading, in yaw and pitch (ours, Open question 28).
pub const TORSO_YAW: f32 = std::f32::consts::FRAC_PI_2;
pub const TORSO_PITCH: f32 = 1.0;
/// Patrol stops (4): the look-around lasts 3 + U(0, 3) s.
pub const STOP_MIN: f32 = 3.0;
pub const STOP_SPREAD: f32 = 3.0;
/// The hand the grenade leaves (8.5): ours (the `RightHand` socket isn't
/// read), u right, forward and up of the hull centre.
pub const HAND_RIGHT: f32 = 20.0;
pub const HAND_FORWARD: f32 = 15.0;
pub const HAND_UP: f32 = 5.0;
/// A mercenary's aim point above its target's hull centre at first (u,
/// bots_objectives.md 3.3: half the half-height standing).
pub const MERC_HEIGHT: f32 = 25.0;
/// The hull's height standing and crouched (u).
pub const HULL_HEIGHT: f32 = 100.0;
pub const HULL_HEIGHT_CROUCHED: f32 = 80.0;

/// A kind of stimulus (5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Gunfire,
    AllyCall,
    Footstep,
}

/// A stimulus in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stimulus {
    pub id: u64,
    pub kind: Kind,
    /// Where (m).
    pub at: Vec3,
    /// Its radius (u).
    pub radius: f32,
    pub source: Entity,
    pub team: u8,
    pub until: f64,
    /// An ally call's caller's alarm total and facing (yaw).
    pub alarm: f32,
    pub facing: Option<f32>,
}

/// What a bot heard: the stimulus, its alarm level, and (a call's) the
/// caller's total and facing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heard {
    pub id: u64,
    pub kind: Kind,
    pub at: Vec3,
    pub level: f32,
    pub caller_alarm: f32,
    pub face: Option<f32>,
}

/// The global stimulus list.
#[derive(Resource, Clone, Debug, Default)]
pub struct Stimuli {
    pub list: Vec<Stimulus>,
    pub next: u64,
}

impl Stimuli {
    /// Post a stimulus (from now for its kind's lifetime).
    pub fn post(&mut self, kind: Kind, at: Vec3, radius: f32, source: Entity, team: u8, now: f64) {
        self.post_call(kind, at, radius, source, team, now, 0.0, None);
    }

    /// `post` with an ally call's caller's alarm total and facing.
    #[allow(clippy::too_many_arguments)]
    pub fn post_call(&mut self, kind: Kind, at: Vec3, radius: f32, source: Entity, team: u8, now: f64, alarm: f32, facing: Option<f32>) {
        self.next += 1;
        let life = match kind {
            Kind::Gunfire => GUNFIRE_LIFE,
            Kind::AllyCall => CALL_LIFE,
            Kind::Footstep => FOOTSTEP_LIFE,
        };
        self.list.push(Stimulus {
            id: self.next,
            kind,
            at,
            radius,
            source,
            team,
            until: now + life,
            alarm,
            facing,
        });
    }
}

/// When each patrol node (area) was last reached (ms), and the chase
/// offsets' shared cycle.
#[derive(Resource, Clone, Debug, Default)]
pub struct PatrolClock {
    pub reached: HashMap<usize, f64>,
    pub chase_cycle: u32,
    /// The spawns each team's bots have had (Alpha, Bravo): the next
    /// patrol start node is this mod 8 (4).
    pub starts: [u32; 2],
}

/// The brains' shared copy of the navigation mesh (`ensure_nav`).
#[derive(Resource, Clone)]
pub struct BotNav {
    pub id: usize,
    pub mesh: std::sync::Arc<NavMesh>,
    pub patrol: std::sync::Arc<nav::PatrolGraph>,
}

/// The bots' server frames (one sense pass and goal update each).
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct BotFrames(pub u64);

/// A human's camping time (5.5): within `CAMP_RADIUS` of the anchor.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct CaCamping {
    pub anchor: Vec3,
    pub time: f32,
}

/// A character's leg animation and its time at the last update (the
/// footstep keys passed since).
#[derive(Component, Clone, Debug, Default)]
pub struct StepClock {
    pub name: String,
    pub time: u32,
}

// ---------------------------------------------------------------------------
// Stimuli and camping

/// Gunfire, footsteps, the shots' and hits' bookkeeping (see the module
/// doc).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn post_stimuli(
    mut events: MessageReader<WeaponEvent>,
    mut damage: MessageReader<Damage>,
    weapons: Query<(&Weapon, Option<&CaMods>, Has<Magazine>, Option<&crate::weapon::Trigger>)>,
    mut bots: Query<&mut CaBot>,
    chars: Query<
        (Entity, &Team, &Transform, &MovementState, Option<&super::super::characters::CaAnim>, Option<&StepClock>),
        (Without<Dead>, With<Intent>),
    >,
    intents: Query<(), With<Intent>>,
    filters: Query<&crate::core::DamageFilter>,
    teams: Query<&Team>,
    mut stimuli: ResMut<Stimuli>,
    clock: Res<SimClock>,
    data: Res<BotData>,
    ca: Res<super::super::data::CaData>,
    mut commands: Commands,
) {
    let now = clock.now;
    stimuli.list.retain(|s| s.until > now);
    for e in events.read() {
        if e.replay {
            continue;
        }
        let WeaponEventKind::Fired { origin, .. } = e.kind else { continue };
        let Ok((w, fitted, gun, trigger)) = weapons.get(e.weapon) else { continue };
        if gun {
            let radius = fitted.map_or(2000, |f| f.fire_sound().radius) as f32;
            let team = teams.get(e.owner).map_or(0, |t| t.0);
            stimuli.post(Kind::Gunfire, origin, radius, e.owner, team, now);
        }
        if let Ok(mut b) = bots.get_mut(e.owner) {
            // The next round no sooner than the fire animation allows
            // (8.3); another game's gun (no record here, so no animation:
            // the default's 1 s would fire an SMG once a second) as fast as
            // its own cycle.
            b.next_shot = now
                + match ca.characters().weapons.get(w.id) {
                    Some(look) => math::fire_interval_ms(look.ani_type) as f64 / 1000.0,
                    None => trigger.map_or(0.1, |t| t.cycle as f64),
                };
            b.aim_offset = None;
            b.release = true;
            if gun && data.gun_type(w.id) != Some("Knife") {
                b.grenades = b.grenades.saturating_sub(1);
            }
        }
    }
    for d in damage.read() {
        if d.amount <= 0.0 {
            continue;
        }
        if let Some(a) = d.attacker
            && a != d.target
            && intents.contains(d.target)
            && teams.get(a).ok() != teams.get(d.target).ok()
            && !bots.contains(d.target)
            && !filters.get(d.target).is_ok_and(|f| f.blocked.contains(&d.kind))
            && let Ok(mut b) = bots.get_mut(a)
        {
            b.grenades = data.template.grenade_shots.at(b.template) as u32;
        }
        if let Ok(mut b) = bots.get_mut(d.target) {
            // When hit: an attack move at once, and a dodge if allowed.
            b.next_move = b.next_move.min(now);
            b.jump = false;
            b.passes = b.passes.max(1);
            b.dodge_hit = true;
        }
    }
    // Footsteps: at the keys of each character's (server-side) leg
    // animation (5.3), on the ground or a ladder.
    for (e, team, t, state, anim, step) in &chars {
        let Some(anim) = anim else { continue };
        let d = &anim.driver;
        let name = d.name_of(&d.lower).to_string();
        let time = d.lower.time;
        let length = d.list.entries.get(d.lower.current).map_or(0, |e| e.length());
        let last = step.map(|s| (s.name.clone(), s.time));
        commands.entity(e).insert(StepClock { name: name.clone(), time });
        let Some((last_name, before)) = last.filter(|(n, _)| *n == name) else { continue };
        let _ = last_name;
        if !(state.on_ground || state.on_ladder) {
            continue;
        }
        if math::footstep_keys(&name).iter().any(|k| math::passed_key(before, time, length, *k)) {
            stimuli.post(Kind::Footstep, t.translation, FOOTSTEP_RADIUS, e, team.0, now);
        }
    }
}

/// Each human's camping time (5.5): grows within `CAMP_RADIUS` of the
/// anchor, starts again (a new anchor) beyond it or at death.
#[allow(clippy::type_complexity)]
pub fn camping(
    mut humans: Query<(Entity, &Transform, Has<Dead>, Option<&mut CaCamping>), (With<Intent>, With<Team>, Without<CaBot>)>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let dt = clock.dt();
    for (e, t, dead, camp) in &mut humans {
        let p = t.translation;
        match camp {
            None => {
                commands.entity(e).insert(CaCamping { anchor: p, time: 0.0 });
            }
            Some(mut c) => {
                if dead || c.anchor.distance(p) / UNIT > CAMP_RADIUS {
                    *c = CaCamping { anchor: p, time: 0.0 };
                } else {
                    c.time += dt;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Navigation mesh

/// The navigation mesh on its way (`ensure_nav`, a front end's): flooded on a
/// task (`nav_gen::NavGenTask`). Goes with the map (and the task with it).
#[derive(Resource)]
pub struct NavJob(nav_gen::NavGenTask);

/// The map's navigation mesh: generated from the collision
/// (`map::nav_gen`) once bots play on a map that has none, from its
/// spawn points and characters, bounded to the clusters a walking player
/// reaches where the map has visibility data (`nav_gen::Bounds`). A front end
/// floods it on a task (`nav_gen::NavGenTask`, the main thread only polling),
/// started behind the loading screen (`settle_nav`) or, on a map nothing
/// loaded behind one (the command line's `--map`), from the third tick, bots
/// standing until it is done (a second or two on de_dust2); a dedicated
/// server floods at once.
pub fn ensure_nav(world: &mut World) {
    if world.contains_resource::<NavMesh>() {
        // The brains' shared copy, kept with the map's mesh.
        let stale = world.get_resource::<BotNav>().is_none_or(|n| n.id != world.resource::<NavMesh>().areas.len())
            || world.is_resource_changed::<NavMesh>();
        if stale {
            let mesh = world.resource::<NavMesh>().clone();
            // The map's patrol nodes, else ours.
            let patrol = MapAi::of(world)
                .and_then(|ai| ai.patrol_graph(&mesh))
                .unwrap_or_else(|| nav::PatrolGraph::build(&mesh));
            world.insert_resource(BotNav {
                id: mesh.areas.len(),
                patrol: std::sync::Arc::new(patrol),
                mesh: std::sync::Arc::new(mesh),
            });
        }
        return;
    }
    world.remove_resource::<BotNav>();
    if world.resource::<SimClock>().tick < 3 {
        return;
    }
    if world.query_filtered::<(), With<CaBot>>().iter(world).next().is_none() {
        return;
    }
    if world.contains_resource::<nav_gen::SliceFloods>() {
        nav_on_task(world);
    } else {
        nav_whole(world);
    }
}

/// Behind the loading screen (`MapHooks::settle`): the navigation mesh of a
/// map bots are going to play, so they walk from the first frame shown.
pub fn settle_nav(world: &mut World) -> bool {
    let settings = world.resource::<BotSettings>();
    let client = world.get_resource::<crate::core::NetRole>().is_some_and(|r| *r == crate::core::NetRole::Client);
    if settings.quota == 0 || client || world.contains_resource::<NavMesh>() {
        return true;
    }
    nav_on_task(world);
    world.contains_resource::<NavMesh>()
}

/// What a flood of the map starts from: its seeds (the spawn points, the
/// living characters' feet), the moving bodies it looks through, the walker
/// and the map's visibility (the bound, `nav_gen::Bounds`).
struct NavStart {
    seeds: Vec<Vec3>,
    ignore: HashSet<Entity>,
    params: nav_gen::NavGenParams,
    vis: Option<std::sync::Arc<crate::map::vis::MapVisibility>>,
}

impl NavStart {
    fn of(world: &mut World) -> Self {
        let mut seeds: Vec<Vec3> = world
            .query_filtered::<&Transform, With<crate::core::SpawnPoint>>()
            .iter(world)
            .map(|t| t.translation)
            .collect();
        seeds.extend(
            world
                .query_filtered::<&Transform, (With<Intent>, Without<Dead>)>()
                .iter(world)
                .map(|t| t.translation - Vec3::Y * 0.8),
        );
        let ignore = SnapQueries::new(world).moving_bodies(world);
        // Bounded to the rooms a walking player reaches (`nav_gen::Bounds`):
        // the jump-ups climb out of de_aztec otherwise.
        let vis = world.get_resource::<ActiveVisibility>().map(|v| v.0.clone());
        let mut params = nav_params();
        if let Some(radius) = world.get_resource::<BotSettings>().and_then(|s| s.walker_radius) {
            params.radius = radius;
        }
        Self { seeds, ignore, params, vis }
    }

    fn job(&self) -> nav_gen::NavGenJob {
        nav_gen::NavGenJob::new(&self.seeds, &self.params, self.vis.is_some().then(nav_gen::walking_player))
    }

    /// The map's AI volumes as the mesh, where it has them (`map_ai`).
    fn map_ai(&self, world: &mut World) -> Option<NavMesh> {
        let ignore = &self.ignore;
        MapAi::of(world).and_then(|ai| {
            let mut state = SystemState::<SpatialQuery>::new(world);
            let q = state.get(world).unwrap();
            ai.nav_mesh(UNIT, |min, max| volume_floor(&q, &|e| ignore.contains(&e), min, max))
        })
    }
}

/// The mesh in, said in the log.
fn insert_mesh(world: &mut World, mesh: NavMesh, ai: bool) {
    if ai {
        info!("bots: the map's {} AI volumes as navigation areas", mesh.areas.len());
    } else {
        info!("bots: generated a navigation mesh of {} areas", mesh.areas.len());
    }
    world.insert_resource(mesh);
}

/// The whole mesh on this thread (a dedicated server, a test).
fn nav_whole(world: &mut World) {
    let start = NavStart::of(world);
    if let Some(mesh) = start.map_ai(world) {
        return insert_mesh(world, mesh, true);
    }
    let mesh = {
        let mut state = SystemState::<SpatialQuery>::new(world);
        let q = state.get(world).unwrap();
        // A bounded flood is two steps (the walk that finds the bound's
        // clusters, then the mesh): both here, on one job. One step of a
        // new job every tick never got past the first, and flooded the
        // whole map again each tick (2 G instructions on de_dust2) with
        // no mesh ever made (docs/performance.md, "Performance pass").
        let mut job = start.job();
        let mut step = || job.step(&q, &|e| start.ignore.contains(&e), start.vis.as_deref(), usize::MAX);
        step().or_else(&mut step)
    };
    if let Some(mesh) = mesh {
        insert_mesh(world, mesh, false);
    }
}

/// The mesh's task polled, or started (on a copy of the world's collision):
/// inserted when it is done.
fn nav_on_task(world: &mut World) {
    if let Some(mut job) = world.remove_resource::<NavJob>() {
        match job.0.poll(chimera_core::tasks::waits(world)) {
            Some(mesh) => insert_mesh(world, mesh, false),
            None => world.insert_resource(job),
        }
        return;
    }
    let start = NavStart::of(world);
    if let Some(mesh) = start.map_ai(world) {
        return insert_mesh(world, mesh, true);
    }
    let job = start.job();
    let snapshot = nav_gen::CollisionSnapshot::of(world);
    world.insert_resource(NavJob(nav_gen::NavGenTask::spawn(job, snapshot, start.ignore, start.vis)));
}

/// How far under an AI volume's box its floor may be (m, ours).
pub const FLOOR_BELOW: f32 = 1.5;

/// The floor inside an AI volume's box (`min`, `max`, m): the first
/// surface a ray down from the top of the box meets at its centre, within
/// the box or under it within `FLOOR_BELOW` (costarecon_cp's boxes sit
/// 0.2-0.8 m above their floors); None where there is none.
fn volume_floor(q: &SpatialQuery, ignore: &dyn Fn(Entity) -> bool, min: Vec3, max: Vec3) -> Option<f32> {
    let c = (min + max) * 0.5;
    let top = Vec3::new(c.x, max.y, c.z);
    let reach = max.y - min.y + FLOOR_BELOW;
    q.cast_ray_predicate(top, Dir3::NEG_Y, reach, true, &sight_filter(), &|e| !ignore(e))
        .map(|h| top.y - h.distance)
}

/// Ours: the highest rise between navigation cells (m), and a rise above
/// this one (m) ahead within `JUMP_AHEAD` (m) makes a walking bot jump.
pub const JUMP_UP: f32 = 0.8;
pub const JUMP_RISE: f32 = 0.35;
pub const JUMP_AHEAD: f32 = 1.2;

/// What sight, the line of fire and the grenade's line meet: the gameplay
/// layers (`core::SOLID_LAYERS`: not players-only clips or ragdolls;
/// traces-only detail does block).
fn sight_filter() -> SpatialQueryFilter {
    SpatialQueryFilter::from_mask(crate::core::SOLID_LAYERS)
}

/// The generator's walker: Chimera's 1.8 m hull, the fallback hull's half
/// width, rises up to `JUMP_UP`.
pub fn nav_params() -> crate::map::nav_gen::NavGenParams {
    let m = super::super::movement::movement_data();
    crate::map::nav_gen::NavGenParams {
        // A stair step or a jump up (the walker jumps up to the next point,
        // `walk`): the maps have ledges a little above the stair step.
        step: JUMP_UP,
        radius: super::super::rules::spawns::HULL.x * m.meters_per_unit,
        max_drop: 3.0,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// The brain

/// A character as the brains see it this tick.
#[derive(Clone, Copy, Debug)]
pub struct Char {
    pub e: Entity,
    pub team: u8,
    /// Hull centre (m).
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub alive: bool,
    pub bot: bool,
    pub crouched: bool,
    pub crouching_attack: Option<Entity>,
}

impl Char {
    pub fn eye(&self) -> Vec3 {
        self.pos + Vec3::Y * EYE
    }
    pub fn feet(&self) -> Vec3 {
        self.pos - Vec3::Y * 0.9
    }
    pub fn torso(&self) -> Vec3 {
        self.pos + Vec3::Y * TORSO
    }
    pub fn head(&self) -> Vec3 {
        self.pos + Vec3::Y * HEAD
    }
    /// The hull's height (u).
    pub fn hull_height(&self) -> f32 {
        if self.crouched { HULL_HEIGHT_CROUCHED } else { HULL_HEIGHT }
    }
    /// The line of fire's origin: the bot's right side (Correction 11).
    pub fn side_origin(&self) -> Vec3 {
        let right = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
        self.pos + right * (RADIUS * UNIT) + Vec3::Y * (SIDE_UP * UNIT)
    }
    /// The hand a grenade leaves (8.5).
    pub fn hand(&self) -> Vec3 {
        let right = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
        self.pos + (right * HAND_RIGHT + self.forward() * HAND_FORWARD + Vec3::Y * HAND_UP) * UNIT
    }
    pub fn forward(&self) -> Vec3 {
        Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos())
    }
    pub fn look(&self) -> Vec3 {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0) * Vec3::NEG_Z
    }
}

/// One of a bot's weapons now.
#[derive(Clone, Copy, Debug)]
struct Gun {
    e: Entity,
    id: &'static str,
    clip: u32,
    reserve: u32,
    count: Option<u32>,
    /// Its data slot (`Weapon::slot`: 0 primary, 1 secondary, 2 melee, 3
    /// thrown).
    data_slot: u8,
    /// A grenade's throw key and animation (s).
    throw_delay: f32,
    throw_time: f32,
    /// An auto turret to place (special_weapons.md 6.7).
    turret: bool,
}

impl Gun {
    fn has_ammo(&self) -> bool {
        self.count.map_or(self.clip + self.reserve > 0, |c| c > 0)
    }
}

/// A live grenade the bots may dodge.
#[derive(Clone, Copy, Debug)]
struct Danger {
    e: Entity,
    pos: Vec3,
    radius: f32,
    player_thrown: bool,
    team: u8,
    touched: bool,
}

/// What the world gives a brain for a tick.
struct Ctx<'a> {
    now: f64,
    frame: bool,
    q: &'a SpatialQuery<'a, 'a>,
    chars: &'a [Char],
    /// Bodies sight rays pass (characters and moving bodies).
    see_through: &'a HashSet<Entity>,
    /// Moving bodies that aren't characters (the weapon ray passes them).
    loose: &'a HashSet<Entity>,
    mesh: Option<&'a NavMesh>,
    graph: Option<&'a nav::PatrolGraph>,
    data: &'a BotData,
    stimuli: &'a Stimuli,
    patrol: &'a PatrolClock,
    camping: &'a HashMap<Entity, f32>,
    dangers: &'a [Danger],
    settings: &'a BotSettings,
}

/// What a brain does this tick.
#[derive(Default)]
struct Out {
    intent: Option<IntentOut>,
    wanted: Option<Entity>,
    throw: Option<(Entity, Vec3, Vec3)>,
    ask_crouch: Option<Entity>,
    /// An ally call: where, the caller's yaw and alarm total.
    ally_call: Option<(Vec3, f32, f32)>,
    /// A weapon whose clip is filled at once (a chasing bot's reload).
    refill: Option<Entity>,
    reached: Option<usize>,
    chase_step: bool,
}

#[derive(Clone, Copy, Default)]
struct IntentOut {
    yaw: f32,
    pitch: f32,
    axis: Vec2,
    fire: bool,
    reload: bool,
    crouch: bool,
    jump: bool,
}

/// Every bot's update this tick (1, "Per-tick order"): senses and goals on
/// the server's frames, aim, keys and the trigger every tick.
pub fn think(world: &mut World, mut queries: Local<Option<SnapQueries>>, mut spatial: Local<Option<SystemState<SpatialQuery<'static, 'static>>>>) {
    let now = world.resource::<SimClock>().now;
    let frame = {
        let due = ((now + 1e-9) / SERVER_FRAME).floor() as u64;
        let mut f = world.resource_mut::<BotFrames>();
        let run = f.0 < due;
        f.0 = f.0.max(due);
        run
    };
    let queries = queries.get_or_insert_with(|| SnapQueries::new(world));
    let bots: Vec<Entity> = queries.bots.iter(world).collect();
    if bots.is_empty() {
        return;
    }
    let settings = world.resource::<BotSettings>().clone();
    let frozen = world.resource::<FreezeTime>().0
        || world.get_resource::<CaMatch>().is_some_and(|m| {
            matches!(m.phase, Phase::RoundOver { .. } | Phase::Intermission { .. } | Phase::MatchOver)
        });

    let turrets = turrets_of(world);
    let snap = Snap::take(world, queries, now);
    let Snap { chars, see_through, loose, camping, dangers, stimuli, patrol, mesh, graph } = &snap;

    // The brains' decisions (the world read only).
    let mut outs: Vec<(Entity, CaBot, Out)> = Vec::new();
    let mut starts = patrol.starts;
    {
        let state = spatial.get_or_insert_with(|| SystemState::new(world));
        let q = state.get(world).unwrap();
        // Borrowed: a clone a tick cost more than the brains' decisions.
        let data = world.resource::<BotData>();
        let ctx = Ctx {
            now,
            frame,
            q: &q,
            chars,
            see_through,
            loose,
            mesh: mesh.as_deref(),
            graph: graph.as_deref(),
            data,
            stimuli,
            patrol,
            camping,
            dangers,
            settings: &settings,
        };
        for &e in &bots {
            let Some(me) = chars.iter().find(|c| c.e == e).copied() else { continue };
            let mut bot = world.get::<CaBot>(e).unwrap().clone();
            let guns = guns_of(world, e);
            let active = world.get::<Inventory>(e).and_then(|i| i.active);
            if me.alive
                && bot.needs_start
                && let Some(graph) = graph.as_deref().filter(|g| !g.nodes.is_empty())
            {
                take_start_node(&mut bot, &me, graph, &mut starts);
            }
            let out = if !me.alive {
                dead(&mut bot, data);
                Out::default()
            } else if frozen {
                Out {
                    intent: Some(IntentOut {
                        yaw: me.yaw,
                        pitch: me.pitch,
                        ..default()
                    }),
                    ..default()
                }
            } else {
                let mut out = decide(&ctx, &mut bot, &me, &guns, active);
                if ctx.settings.dont_shoot == 0 {
                    turret_step(&ctx, &bot, &me, &guns, active, &turrets, &mut out);
                }
                out
            };
            outs.push((e, bot, out));
        }
    }

    // Apply.
    for (e, bot, out) in outs {
        if let Some(i) = out.intent {
            let mut intent = world.get_mut::<Intent>(e).unwrap();
            intent.yaw = i.yaw;
            intent.pitch = i.pitch.clamp(-1.55, 1.55);
            intent.move_axis = i.axis;
            intent.fire = i.fire;
            intent.reload = i.reload;
            intent.crouch = i.crouch;
            intent.jump = i.jump;
            intent.sprint = false;
            intent.walk = false;
            intent.secondary = false;
            intent.select = None;
            intent.last_weapon = false;
        }
        if let Some(w) = out.wanted
            && let Some(mut inv) = world.get_mut::<Inventory>(e)
            && inv.active != Some(w)
            && inv.wanted != Some(w)
        {
            inv.wanted = Some(w);
        }
        if let Some((w, from, v)) = out.throw {
            let left = world.get::<Throwable>(w).map_or(0, |t| t.count);
            if left > 0 {
                super::super::grenades::spawn_grenade(world, w, Some(e), from, v);
                if let Some(mut t) = world.get_mut::<Throwable>(w) {
                    t.count -= 1;
                }
            }
        }
        if let Some(mate) = out.ask_crouch
            && let Some(mut m) = world.get_mut::<CaBot>(mate)
        {
            m.crouch_until = m.crouch_until.max(now + CROUCH_HOLD);
        }
        if let Some((at, yaw, alarm)) = out.ally_call {
            let team = world.get::<Team>(e).map_or(0, |t| t.0);
            world.resource_mut::<Stimuli>().post_call(Kind::AllyCall, at, ALLY_CALL, e, team, now, alarm, Some(yaw));
        }
        if let Some(w) = out.refill
            && let Some(mut m) = world.get_mut::<Magazine>(w)
        {
            let take = m.size.saturating_sub(m.clip).min(m.reserve);
            m.clip += take;
            m.reserve -= take;
        }
        if let Some(a) = out.reached {
            world.resource_mut::<PatrolClock>().reached.insert(a, now * 1000.0);
        }
        if out.chase_step {
            world.resource_mut::<PatrolClock>().chase_cycle += 1;
        }
        *world.get_mut::<CaBot>(e).unwrap() = bot;
    }
    world.resource_mut::<PatrolClock>().starts = starts;
}

/// A new life's first patrol node (4): its team's next start node of the
/// bot game-mode file, `PatrolNode_<Team><k mod 8>` = the map's
/// `AI_Node_<A|B>_<k mod 8 + 1>`, round robin over the team's spawns; a
/// map without such a node leaves the bot to the nearest one. Mercenaries
/// have none.
fn take_start_node(bot: &mut CaBot, me: &Char, graph: &nav::PatrolGraph, starts: &mut [u32; 2]) {
    bot.needs_start = false;
    if bot.merc.is_some() || !matches!(me.team, 1 | 2) {
        return;
    }
    let k = &mut starts[(me.team - 1) as usize];
    let n = *k % 8 + 1;
    *k += 1;
    let name = format!("AI_Node_{}_{n:02}", if me.team == 1 { 'A' } else { 'B' });
    bot.patrol_to = graph.by_name(&name);
    bot.patrol_from = None;
    bot.patrol_came = None;
    bot.route.clear();
    bot.route_goal = None;
    say(bot, Chat::Search);
}

/// Raise a chat event (10); a mercenary says nothing.
fn say(bot: &mut CaBot, event: Chat) {
    if bot.merc_aim || bot.merc.is_some() || bot.chat.len() >= 16 {
        return;
    }
    bot.chat.push(event);
}

/// What the brains read of the world this tick.
struct Snap {
    chars: Vec<Char>,
    see_through: HashSet<Entity>,
    loose: HashSet<Entity>,
    camping: HashMap<Entity, f32>,
    dangers: Vec<Danger>,
    stimuli: Stimuli,
    patrol: PatrolClock,
    mesh: Option<std::sync::Arc<NavMesh>>,
    graph: Option<std::sync::Arc<nav::PatrolGraph>>,
}

/// The queries the brains read the world through, kept from tick to tick
/// (`think`'s): made anew each tick, they cost more than the brains'
/// decisions (docs/performance.md, "Lucker Party").
#[allow(clippy::type_complexity)]
pub struct SnapQueries {
    bots: QueryState<Entity, With<CaBot>>,
    chars: QueryState<
        (Entity, &'static Team, &'static Transform, &'static Intent, &'static Health, Has<Dead>, Has<CaHeld>, Option<&'static CaBot>, Option<&'static MovementState>),
        Without<Spectating>,
    >,
    intents: QueryState<Entity, With<Intent>>,
    bodies: QueryState<(Entity, &'static RigidBody)>,
    see_through: QueryState<Entity, With<SeeThrough>>,
    camping: QueryState<(Entity, &'static CaCamping)>,
    grenades: QueryState<(Entity, &'static CaGrenade)>,
}

impl SnapQueries {
    fn new(world: &mut World) -> Self {
        Self {
            bots: world.query_filtered(),
            chars: world.query_filtered(),
            intents: world.query_filtered(),
            bodies: world.query(),
            see_through: world.query_filtered(),
            camping: world.query(),
            grenades: world.query(),
        }
    }

    /// Characters and bodies that aren't static (sight and the floor
    /// probes pass them).
    fn moving_bodies(&mut self, world: &World) -> HashSet<Entity> {
        let mut out: HashSet<Entity> = self.intents.iter(world).collect();
        out.extend(self.bodies.iter(world).filter(|(_, b)| !matches!(b, RigidBody::Static)).map(|(e, _)| e));
        out
    }
}

impl Snap {
    fn take(world: &mut World, queries: &mut SnapQueries, now: f64) -> Snap {
        let chars: Vec<Char> = queries
            .chars
            .iter(world)
            .map(|(e, t, tr, i, h, dead, held, b, ms)| Char {
                e,
                team: t.0,
                pos: tr.translation,
                yaw: i.yaw,
                pitch: i.pitch,
                alive: !dead && !held && h.current > 0.0,
                bot: b.is_some(),
                crouched: ms.is_some_and(|m| m.crouching),
                crouching_attack: b.and_then(|b| {
                    (b.crouch_until > now && matches!(b.state, BotState::Attack { .. })).then_some(b.target).flatten()
                }),
            })
            .collect();
        let all = queries.moving_bodies(world);
        let loose: HashSet<Entity> = {
            let chars: HashSet<Entity> = chars.iter().map(|c| c.e).collect();
            all.difference(&chars).copied().collect()
        };
        // Sight passes characters, moving bodies and see-through
        // surfaces: the map's glass (5.2 step 6; `world::
        // GLASS_SURFACE_TYPES`).
        let mut see_through = all;
        see_through.extend(queries.see_through.iter(world));
        let camping: HashMap<Entity, f32> = queries.camping.iter(world).map(|(e, c)| (e, c.time)).collect();
        let dangers: Vec<Danger> = queries
            .grenades
            .iter(world)
            .map(|(e, g)| {
                let thrower = g.thrower;
                Danger {
                    e,
                    pos: g.pos * g.unit,
                    radius: g.params.area_radius.max(g.params.over_time.map_or(0.0, |o| o.radius)),
                    player_thrown: thrower.is_some_and(|t| !chars.iter().any(|c| c.e == t && c.bot)),
                    team: thrower.and_then(|t| chars.iter().find(|c| c.e == t)).map_or(0, |c| c.team),
                    touched: g.touched || g.resting,
                }
            })
            .collect();
        let stimuli = world.resource::<Stimuli>().clone();
        let patrol = world.resource::<PatrolClock>().clone();
        let mesh = world.get_resource::<BotNav>().map(|n| n.mesh.clone());
        let graph = world.get_resource::<BotNav>().map(|n| n.patrol.clone());
        Snap {
            chars,
            see_through,
            loose,
            camping,
            dangers,
            stimuli,
            patrol,
            mesh,
            graph,
        }
    }
}

/// Whether bot `bot` sees `target` now by the sight test (5.2; its state's
/// field of view, no notice roll): None when not, Some(true) when only the
/// head is visible.
pub fn sees(world: &mut World, bot: Entity, target: Entity) -> Option<bool> {
    let now = world.resource::<SimClock>().now;
    let alert = !world.get::<CaBot>(bot)?.state.relaxed();
    let settings = world.resource::<BotSettings>().clone();
    let mut queries = SnapQueries::new(world);
    let snap = Snap::take(world, &mut queries, now);
    let mut state = SystemState::<SpatialQuery>::new(world);
    let q = state.get(world).unwrap();
    let ctx = Ctx {
        now,
        frame: false,
        q: &q,
        chars: &snap.chars,
        see_through: &snap.see_through,
        loose: &snap.loose,
        mesh: snap.mesh.as_deref(),
        graph: snap.graph.as_deref(),
        data: world.resource::<BotData>(),
        stimuli: &snap.stimuli,
        patrol: &snap.patrol,
        camping: &snap.camping,
        dangers: &snap.dangers,
        settings: &settings,
    };
    let me = snap.chars.iter().find(|c| c.e == bot)?;
    let t = snap.chars.iter().find(|c| c.e == target)?;
    sight(&ctx, me, t, alert)
}

fn guns_of(world: &World, e: Entity) -> Vec<Gun> {
    let Some(inv) = world.get::<Inventory>(e) else { return Vec::new() };
    inv.weapons
        .iter()
        .filter_map(|&w| {
            let id = world.get::<Weapon>(w)?.id;
            let m = world.get::<Magazine>(w);
            Some(Gun {
                e: w,
                id,
                data_slot: world.get::<Weapon>(w)?.slot,
                clip: m.map_or(0, |m| m.clip),
                reserve: m.map_or(0, |m| m.reserve),
                count: world.get::<Throwable>(w).map(|t| t.count),
                throw_delay: world.get::<Throwable>(w).map_or(0.0, |t| t.throw_delay),
                throw_time: world.get::<Throwable>(w).map_or(0.0, |t| t.throw_time),
                turret: world
                    .get::<super::super::special::CaDeviceData>(w)
                    .is_some_and(|d| d.params.kind == super::super::special::DeviceKind::Turret),
            })
        })
        .collect()
}

/// A landed auto turret as the bots see it (ours: the spec's bots have
/// no rule for turrets, bots.md 5.1 only says devices are seen).
#[derive(Clone, Copy, Debug)]
struct TurretSeen {
    e: Entity,
    /// Its box centre (m).
    at: Vec3,
    owner: Option<Entity>,
    team: u8,
    target: Option<Entity>,
}

fn turrets_of(world: &mut World) -> Vec<TurretSeen> {
    use super::super::special::{CaObject, DeviceKind, Kind};
    let mut q = world.query::<(Entity, &CaObject)>();
    let found: Vec<(Entity, Option<Entity>, Vec3, Option<Entity>)> = q
        .iter(world)
        .filter(|(_, o)| matches!(&o.kind, Kind::Device(p) if p.kind == DeviceKind::Turret) && o.s.landed && !o.destroyed)
        .map(|(e, o)| (e, o.owner, o.at(), o.s.target))
        .collect();
    found
        .into_iter()
        .map(|(e, owner, at, target)| TurretSeen {
            e,
            at,
            owner,
            team: owner.and_then(|o| world.get::<Team>(o)).map_or(0, |t| t.0),
            target,
        })
        .collect()
}

/// How far a bot looks for an enemy turret to shoot (u; ours: the
/// turret's own 800 u search and a margin).
pub const TURRET_SHOOT_RANGE: f32 = 1000.0;

/// The bots and auto turrets (ours, simple; the spec has none): a bot
/// carrying a turret with none of its own standing places one where it
/// looks (a little down, the throw's own speed), then fights on with its
/// other weapons; a bot holding a loaded gun shoots back at an enemy
/// turret in sight within `TURRET_SHOOT_RANGE` that is shooting at it (a
/// threat; a turret on someone else, or still setting up, is left alone,
/// or bots broke every turret before it fired).
fn turret_step(ctx: &Ctx, bot: &CaBot, me: &Char, guns: &[Gun], active: Option<Entity>, turrets: &[TurretSeen], out: &mut Out) {
    let Some(io) = out.intent.as_mut() else { return };
    if bot.throw.is_some() {
        return;
    }
    let mine = turrets.iter().any(|t| t.owner == Some(me.e));
    if !mine && let Some(g) = guns.iter().find(|g| g.turret && g.count.unwrap_or(0) > 0) {
        out.wanted = Some(g.e);
        io.pitch = -0.3;
        io.fire = active == Some(g.e);
        return;
    }
    let Some(held) = active.and_then(|a| guns.iter().find(|g| g.e == a)) else { return };
    if held.count.is_some() || held.clip == 0 {
        return;
    }
    let eye = me.eye();
    let enemy = |t: &&TurretSeen| t.owner != Some(me.e) && (t.team == 0 || t.team != me.team);
    let Some(t) = turrets
        .iter()
        .filter(enemy)
        .filter(|t| t.target == Some(me.e) && u(eye.distance(t.at)) <= TURRET_SHOOT_RANGE)
        .filter(|t| {
            let d = t.at - eye;
            let Ok(dir) = Dir3::new(d) else { return false };
            ctx.q
                .cast_ray_predicate(eye, dir, d.length(), true, &sight_filter(), &|e| e != me.e && !ctx.loose.contains(&e))
                .is_none_or(|h| h.entity == t.e || h.distance >= d.length() - 0.3)
        })
        .min_by(|a, b| eye.distance(a.at).total_cmp(&eye.distance(b.at)))
    else {
        return;
    };
    let (yaw, pitch) = look_at(eye, t.at);
    io.yaw = yaw;
    io.pitch = pitch;
    // Pressed every other server frame: a semi-automatic fires too.
    io.fire = ((ctx.now / SERVER_FRAME) as u64).is_multiple_of(2);
    out.wanted = Some(held.e);
}

/// A dead bot's brain is gone (Correction 7): every respawn is a new AI
/// character from the same roster entry (profile, template, kit, dice, the
/// reaches fixed at the first life, a mercenary's mind); its kit is set up
/// again and it takes a new patrol start node.
fn dead(bot: &mut CaBot, data: &BotData) {
    let mut fresh = CaBot::new(bot.profile, bot.template, 0, bot.kit, data);
    fresh.dice = bot.dice;
    fresh.reach_fixed = bot.reach_fixed;
    fresh.ranged_reach = bot.ranged_reach;
    fresh.thrown_reach = bot.thrown_reach;
    fresh.merc = bot.merc;
    *bot = fresh;
}

fn u(m: f32) -> f32 {
    m / UNIT
}

/// The yaw and pitch looking from `from` at `to` (yaw 0 looks down −Z,
/// positive toward −X).
pub fn look_at(from: Vec3, to: Vec3) -> (f32, f32) {
    let d = to - from;
    let h = Vec2::new(d.x, d.z).length();
    ((-d.x).atan2(-d.z), d.y.atan2(h))
}

/// Movement keys for walking toward `dir` (horizontal) while facing `yaw`.
fn axis_toward(dir: Vec3, yaw: f32) -> Vec2 {
    let d = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
    let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let a = Vec2::new(d.dot(right), d.dot(forward));
    if a.length() > 1e-3 { a / a.x.abs().max(a.y.abs()) } else { Vec2::ZERO }
}

/// The sight test (5.2) from `me` on `t`: None when not seen, else
/// whether only the head was.
fn sight(ctx: &Ctx, me: &Char, t: &Char, alert: bool) -> Option<bool> {
    if !t.alive {
        return None;
    }
    let eye = me.eye();
    let d = u(eye.distance(t.pos));
    if d >= SIGHT_RANGE {
        return None;
    }
    if d > NO_FOV_RADIUS {
        let dir = (t.pos - eye).normalize_or_zero();
        let fwd = me.forward();
        if alert {
            if Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero().dot(fwd) < 0.0 {
                return None;
            }
        } else if dir.dot(fwd) < RELAXED_DOT || dir.y > RELAXED_UP_DOT {
            return None;
        }
    }
    if let Some(mesh) = ctx.mesh
        && nav::straight(mesh, me.feet(), t.feet(), 0.4)
    {
        return Some(false);
    }
    let clear = |to: Vec3| {
        let d = to - eye;
        let len = d.length();
        let Ok(dir) = Dir3::new(d) else { return true };
        ctx.q
            .cast_ray_predicate(eye, dir, len, true, &sight_filter(), &|e| !ctx.see_through.contains(&e))
            .is_none()
    };
    if clear(t.pos) {
        return Some(false);
    }
    clear(t.head()).then_some(true)
}

/// The line of fire from `from` (the bot's right side, `Char::side_origin`)
/// to `to`: None when clear, Some(Some(e)) when character `e` stands in it,
/// Some(None) for the world.
fn fire_line(ctx: &Ctx, me: &Char, target: Entity, from: Vec3, to: Vec3) -> Option<Option<Entity>> {
    let d = to - from;
    let dir = Dir3::new(d).ok()?;
    let hit = ctx.q.cast_ray_predicate(from, dir, d.length(), true, &sight_filter(), &|e| {
        e != me.e && e != target && !ctx.loose.contains(&e)
    })?;
    Some(ctx.chars.iter().any(|c| c.e == hit.entity).then_some(hit.entity))
}

/// Whether the target is visible from the weapon (Correction 11): a ray
/// from the bot's right side to its `Torso` node, then its `Head` node.
/// Some(false) when the torso is, Some(true) when only the head is (the
/// head-only flag of 7.4), None when neither.
fn weapon_view(ctx: &Ctx, me: &Char, t: &Char) -> Option<bool> {
    let from = me.side_origin();
    let blocked = |to: Vec3| matches!(fire_line(ctx, me, t.e, from, to), Some(None));
    if !blocked(t.torso()) {
        Some(false)
    } else if !blocked(t.head()) {
        Some(true)
    } else {
        None
    }
}

/// The bot's kit weapon in slot `k` (0 mid primary, 1 long, 2 secondary,
/// 3 melee, 4 grenade, 5 mine), as carried now; when it doesn't carry
/// it, a weapon it carries of that class (ours: Lucker Party's ladder
/// strips the kit and gives its rung's gun and the gun's game's knife,
/// another game's weapons the bot fights with).
fn slot<'g>(bot: &CaBot, guns: &'g [Gun], k: usize) -> Option<&'g Gun> {
    if let Some(g) = bot.kit[k].and_then(|id| guns.iter().find(|g| g.id == id)) {
        return Some(g);
    }
    let class = match k {
        0 => 0,
        2 => 1,
        3 => 2,
        _ => return None,
    };
    guns.iter().find(|g| g.data_slot == class && bot.kit.iter().all(|k| *k != Some(g.id)))
}

/// Whether `id` is the bot's melee weapon (`slot` 3).
fn is_knife(bot: &CaBot, guns: &[Gun], id: &str) -> bool {
    slot(bot, guns, 3).is_some_and(|g| g.id == id)
}

/// A weapon's AI row (None: no row).
fn gun_ai(ctx: &Ctx, bot: &CaBot, id: &str) -> Option<GunAi> {
    ctx.data.gun(id, bot.template)
}

fn decide(ctx: &Ctx, bot: &mut CaBot, me: &Char, guns: &[Gun], active: Option<Entity>) -> Out {
    // A new life's relaxed moment is its spawn (Correction 2b).
    bot.last_relaxed.get_or_insert(ctx.now);
    bot.merc_aim = bot.merc.is_some();
    if let Some(mut mind) = bot.merc.take() {
        let out = decide_merc(ctx, bot, &mut mind, me, guns, active);
        bot.merc = Some(mind);
        return out;
    }
    let mut out = Out::default();
    let mut io = IntentOut {
        yaw: me.yaw,
        pitch: me.pitch,
        ..default()
    };
    let t = &ctx.data.template;
    let tp = bot.template;
    // A target that died or left is dropped at once.
    if let Some(target) = bot.target
        && !ctx.chars.iter().any(|c| c.e == target && c.alive)
    {
        bot.target = None;
        if matches!(bot.state, BotState::Attack { .. } | BotState::Chase { .. }) {
            leave_attack(ctx, bot);
            enter_patrol(ctx, bot);
        }
    }

    if ctx.frame {
        sense_pass(ctx, bot, me, guns, &mut out);
        goals(ctx, bot, me, guns);
        grenade_danger(ctx, bot, me);
    }

    let target = bot.target.and_then(|e| ctx.chars.iter().find(|c| c.e == e)).copied();
    match bot.state {
        BotState::Attack { .. } => {
            if let Some(target) = target {
                attack(ctx, bot, me, &target, guns, active, &mut io, &mut out);
            }
        }
        BotState::Chase { breath, .. } => {
            if let Some(target) = target {
                if ctx.now < breath {
                    io.axis = Vec2::ZERO;
                } else {
                    let offset = chase_offset(ctx.patrol.chase_cycle);
                    let goal = target.feet() + offset * UNIT;
                    // Re-planned at the path's end, toward where it is now.
                    if bot.route.is_empty() {
                        bot.route_goal = None;
                        out.chase_step = true;
                    }
                    let diverge = !bot.visible;
                    walk(ctx, bot, me, goal, diverge, &mut io);
                }
                // Fire on the way when in reach and visible: blind, along the
                // torso (Correction 10).
                if bot.visible && u(me.pos.distance(target.pos)) <= bot.ranged_reach {
                    chase_fire(ctx, bot, me, &target, guns, active, &mut io, &mut out);
                }
            }
        }
        BotState::Investigate { spot, look_only, at, radius, face, .. } => {
            if look_only {
                let (yaw, _) = look_at(me.eye(), spot);
                io.yaw = yaw;
                io.pitch = 0.0;
            } else if me.pos.distance(at) <= radius {
                // Over on the first update inside half the distance it
                // started from (Correction 1); a call's answer faces
                // where the caller faced (3).
                if let Some(yaw) = face {
                    io.yaw = yaw;
                    io.pitch = 0.0;
                }
                enter_patrol(ctx, bot);
            } else {
                walk(ctx, bot, me, spot, false, &mut io);
                if bot.route.is_empty() {
                    // The path ended.
                    enter_patrol(ctx, bot);
                }
            }
        }
        BotState::Dodge { to, .. } => {
            let d = to - me.feet();
            if d.xz().length() > 0.2 {
                if let Some(target) = target.filter(|_| bot.visible) {
                    aim_and_fire(ctx, bot, me, &target, guns, active, &mut io, &mut out);
                    io.axis = axis_toward(d, io.yaw);
                } else {
                    let (yaw, _) = look_at(me.feet(), to);
                    io.yaw = yaw;
                    io.pitch = 0.0;
                    io.axis = Vec2::Y;
                }
            }
        }
        BotState::Patrol => patrol(ctx, bot, me, &mut io, &mut out),
    }
    let _ = t;
    let _ = tp;
    io.crouch = ctx.now < bot.crouch_until;
    io.jump = std::mem::take(&mut bot.jump);
    if ctx.settings.dont_shoot != 0 {
        io.fire = false;
    }
    if ctx.settings.stop != 0 {
        // `ca_bot_stop`: no movement keys (senses, aim and the trigger go
        // on); a patrolling bot keeps its facing.
        io.axis = Vec2::ZERO;
        io.jump = false;
        if bot.state == BotState::Patrol {
            io.yaw = me.yaw;
            io.pitch = me.pitch;
        }
    }
    out.intent = Some(io);
    out
}

fn chase_offset(cycle: u32) -> Vec3 {
    match cycle % 4 {
        0 => Vec3::new(-CHASE_OFFSET, 0.0, 0.0),
        1 => Vec3::new(CHASE_OFFSET, 0.0, 0.0),
        2 => Vec3::new(0.0, 0.0, -CHASE_OFFSET),
        _ => Vec3::new(0.0, 0.0, CHASE_OFFSET),
    }
}

/// One sense pass (5.1).
fn sense_pass(ctx: &Ctx, bot: &mut CaBot, me: &Char, guns: &[Gun], out: &mut Out) {
    bot.passes += 1;
    let t = &ctx.data.template;
    let tp = bot.template;
    let relaxed = bot.state.relaxed();
    let alert = !relaxed;
    // The current target's visibility and the movement inaccuracy (the
    // target update, each pass).
    if let Some(target) = bot.target.and_then(|e| ctx.chars.iter().find(|c| c.e == e)) {
        let seen = sight(ctx, me, target, true);
        bot.visible = seen.is_some();
        // The head-only flag is the weapon's ray's (Correction 11).
        bot.head_only = seen.is_some() && weapon_view(ctx, me, target) == Some(true);
        let gun = held_gun(ctx, bot, guns, me);
        bot.tracking
            .update(target.pos / UNIT, SERVER_FRAME as f32, gun.map_or(0.0, |g| g.c_max), gun.map_or(0.0, |g| g.decay));
    }
    // 1: the nearest enemy that passes the coarse test.
    let mut enemies: Vec<(&Char, f32)> = ctx
        .chars
        .iter()
        .filter(|c| c.alive && c.team != me.team && crate::game::rules::state::plays(c.team))
        .map(|c| (c, u(me.pos.distance(c.pos))))
        .collect();
    enemies.sort_by(|a, b| a.1.total_cmp(&b.1));
    let cooldown_min = t.sight_cooldown_min.at(tp);
    let coarse = enemies
        .iter()
        .find(|(_, d)| !(relaxed && *d > cooldown_min && ctx.now < bot.sight_cooldown) && *d < SIGHT_RANGE + 5000.0);
    if let Some(&(target, d)) = coarse
        && let Some(head_only) = sight(ctx, me, target, alert)
    {
        let seen = if relaxed {
            bot.sight_cooldown = ctx.now + SIGHT_COOLDOWN;
            let camp = ctx.camping.get(&target.e).copied().unwrap_or(0.0);
            let p = math::notice_percent(
                d,
                t.notice_near.at(tp),
                t.notice_far.at(tp),
                t.notice_far_percent.at(tp),
                math::camping_bonus(camp, t.camp_sight_step.at(tp)),
            );
            math::roll(p, bot.dice.r100())
        } else {
            true
        };
        if seen {
            sight_fired(ctx, bot, me, target, d, head_only, guns);
            return;
        }
    }
    // 2: what it hears, in alarm order.
    let my_team = me.team;
    let near = |s: &Stimulus, window: f32| {
        let dy = u((s.at.y - me.pos.y).abs());
        (dy <= window, u(s.at.distance(me.pos)))
    };
    let mut order: Vec<&Stimulus> = ctx.stimuli.list.iter().collect();
    order.sort_by_key(|s| match s.kind {
        Kind::Gunfire => 0,
        Kind::AllyCall => 1,
        Kind::Footstep => 2,
    });
    for s in order {
        if s.source == me.e {
            continue;
        }
        match s.kind {
            Kind::Gunfire => {
                if s.id <= bot.last_gunfire || s.team == my_team {
                    continue;
                }
                let (inside, d) = near(s, GUNFIRE_WINDOW);
                if !inside || d >= s.radius {
                    continue;
                }
                bot.last_gunfire = s.id;
                if relaxed && !math::roll(math::hearing_percent(d, 1000.0, 50.0), bot.dice.r100()) {
                    continue;
                }
                let passes = (bot.dice.range(0.1, 0.3) / 0.01).round() as u64;
                bot.pending = Some(Pending {
                    at_pass: bot.passes + passes,
                    heard: Heard {
                        id: s.id,
                        kind: Kind::Gunfire,
                        at: s.at,
                        level: ctx.data.alarm.gunfire,
                        caller_alarm: 0.0,
                        face: None,
                    },
                });
                break;
            }
            Kind::AllyCall => {
                if s.id <= bot.last_call || s.team != my_team {
                    continue;
                }
                let (_, d) = near(s, f32::INFINITY);
                if d >= s.radius {
                    continue;
                }
                bot.last_call = s.id;
                let passes = (bot.dice.range(0.3, 1.0) / 0.01).round() as u64;
                if bot.pending.is_none() {
                    bot.pending = Some(Pending {
                        at_pass: bot.passes + passes,
                        heard: Heard {
                            id: s.id,
                            kind: Kind::AllyCall,
                            at: s.at,
                            level: ctx.data.alarm.ally_call,
                            caller_alarm: s.alarm,
                            face: s.facing,
                        },
                    });
                }
                break;
            }
            Kind::Footstep => {
                if s.id <= bot.last_step || s.team == my_team {
                    continue;
                }
                let (inside, d) = near(s, FOOTSTEP_WINDOW);
                if !inside || d >= HEARING + s.radius {
                    continue;
                }
                bot.last_step = s.id;
                if relaxed && !math::roll(math::hearing_percent(d, 800.0, 60.0), bot.dice.r100()) {
                    continue;
                }
                bot.last_step_at = Some(s.at);
                if bot.footsteps.add(0.5) {
                    investigate(ctx, bot, me, step_heard(ctx, s.id, s.at), out);
                }
                break;
            }
        }
    }
    // 3: the records' decay and timers.
    if bot.footsteps.decay(if alert { 0.00125 } else { 0.0025 })
        && let Some(at) = bot.last_step_at
    {
        // A third false stimulation: fully stimulated.
        investigate(ctx, bot, me, step_heard(ctx, bot.last_step, at), out);
    }
    if let Some(p) = bot.pending
        && bot.passes >= p.at_pass
    {
        bot.pending = None;
        investigate(ctx, bot, me, p.heard, out);
    }
}

fn step_heard(ctx: &Ctx, id: u64, at: Vec3) -> Heard {
    Heard {
        id,
        kind: Kind::Footstep,
        at,
        level: ctx.data.alarm.footstep,
        caller_alarm: 0.0,
        face: None,
    }
}

/// A heard sound fires the investigate goal (5.3): only over patrol and
/// investigate; the bot calls its allies when it starts. While
/// investigating it ignores allies' calls, and a new enemy sound replaces
/// the investigation only if its alarm level is not lower than the current
/// one's and it is not the stimulus already followed (Correction 3). Every
/// sound taken adds its level to the life's running total (Correction 2a).
fn investigate(ctx: &Ctx, bot: &mut CaBot, me: &Char, heard: Heard, out: &mut Out) {
    if !bot.state.relaxed() {
        return;
    }
    if let BotState::Investigate { level, id, .. } = bot.state
        && (heard.kind == Kind::AllyCall || heard.level < level || heard.id == id)
    {
        return;
    }
    let starting = bot.state == BotState::Patrol;
    bot.alarm += heard.level;
    if heard.kind == Kind::AllyCall {
        bot.alarm = bot.alarm.max(heard.caller_alarm);
    }
    let spot_feet = ctx
        .mesh
        .and_then(|m| m.nearest_area(heard.at).map(|a| m.areas[a].closest_point(heard.at)))
        .unwrap_or(heard.at);
    // Only looked at (Correction 2): not majorly alarmed, more than 30 s
    // since the last relaxed moment, within 1000 u, and in plain view
    // (inside the relaxed field of view, a straight walk or a clear eye
    // ray) or a straight walk away. Otherwise it runs.
    let distance = u(me.pos.distance(heard.at));
    let look_only = bot.alarm < ctx.data.alarm.major
        && ctx.now - bot.last_relaxed.unwrap_or(ctx.now) > LOOK_ONLY_GRACE
        && distance <= LOOK_ONLY_RANGE
        && (in_plain_view(ctx, me, heard.at) || ctx.mesh.is_some_and(|m| nav::straight(m, me.feet(), spot_feet, 0.4)));
    bot.state = BotState::Investigate {
        spot: spot_feet,
        until: ctx.now + INVESTIGATE_TIME,
        look_only,
        at: heard.at,
        radius: 0.5 * me.pos.distance(heard.at),
        level: heard.level,
        id: heard.id,
        face: heard.face,
    };
    bot.route.clear();
    bot.route_goal = None;
    if starting {
        out.ally_call = Some((me.pos, me.yaw, bot.alarm));
        reset_grenades(ctx, bot);
    }
}

/// Whether `spot` is inside the bot's relaxed field of view (5.2) and
/// visible by a clear eye ray.
fn in_plain_view(ctx: &Ctx, me: &Char, spot: Vec3) -> bool {
    let eye = me.eye();
    let d = u(eye.distance(spot));
    if d > NO_FOV_RADIUS {
        let dir = (spot - eye).normalize_or_zero();
        if dir.dot(me.forward()) < RELAXED_DOT || dir.y > RELAXED_UP_DOT {
            return false;
        }
    }
    let to = spot + Vec3::Y * 0.5 - eye;
    Dir3::new(to).ok().is_some_and(|dir| {
        ctx.q
            .cast_ray_predicate(eye, dir, to.length(), true, &sight_filter(), &|e| !ctx.see_through.contains(&e))
            .is_none()
    })
}

fn reset_grenades(ctx: &Ctx, bot: &mut CaBot) {
    bot.grenades = ctx.data.template.grenade_shots.at(bot.template) as u32;
}

/// The patrol goal activates (4): the bot is relaxed again (Correction 2b)
/// and says a Search line (Correction 5).
fn enter_patrol(ctx: &Ctx, bot: &mut CaBot) {
    bot.state = BotState::Patrol;
    bot.target = None;
    bot.route.clear();
    bot.route_goal = None;
    bot.patrol_to = None;
    bot.patrol_wait = 0.0;
    bot.throw = None;
    bot.last_relaxed = Some(ctx.now);
    reset_grenades(ctx, bot);
    say(bot, Chat::Search);
}

fn leave_attack(ctx: &Ctx, bot: &mut CaBot) {
    if matches!(bot.state, BotState::Attack { .. }) {
        bot.attack_off = ctx.now;
    }
    bot.move_to = None;
    bot.chase_timer = None;
}

/// Whether a ranged weapon of the kit can be used (has ammunition).
fn ranged_usable(bot: &CaBot, guns: &[Gun]) -> bool {
    (0..3).any(|k| slot(bot, guns, k).is_some_and(Gun::has_ammo))
}

/// Sight fired on `target` at `d` u (6: the last sight that fired is the
/// target).
fn sight_fired(ctx: &Ctx, bot: &mut CaBot, me: &Char, target: &Char, d: f32, _eye_head_only: bool, guns: &[Gun]) {
    if bot.target != Some(target.e) {
        bot.target = Some(target.e);
        let c_max = held_gun(ctx, bot, guns, me).map_or(0.0, |g| g.c_max);
        bot.tracking.select(c_max);
        bot.tracking.prev = Some(target.pos / UNIT);
        reset_grenades(ctx, bot);
        bot.aim_offset = None;
    }
    bot.visible = true;
    bot.head_only = weapon_view(ctx, me, target) == Some(true);
    let melee = !ranged_usable(bot, guns) && slot(bot, guns, 3).is_some();
    let in_reach = d <= bot.ranged_reach || (melee && d <= KNIFE_RANGE);
    match bot.state {
        BotState::Attack { .. } => bot.chase_timer = None,
        BotState::Dodge { .. } => {}
        _ if in_reach => enter_attack(ctx, bot, target, true),
        BotState::Chase { .. } => {}
        _ => enter_chase(ctx, bot),
    }
}

/// The attack goal activates (7.2's reaction delay, 9.3's crouch); from
/// another state it says a Battle line (`announce`; not when it returns
/// from a grenade dodge, Correction 5).
fn enter_attack(ctx: &Ctx, bot: &mut CaBot, target: &Char, announce: bool) {
    if announce {
        say(bot, Chat::Battle);
    }
    let t = &ctx.data.template;
    let tp = bot.template;
    if ctx.now - bot.attack_off > REARM_GAP {
        let camp = ctx.camping.get(&target.e).copied().unwrap_or(0.0);
        let delay = math::reaction_delay(t.reaction_min.at(tp), t.reaction_max.at(tp), camp, bot.dice.unit());
        bot.fire_at = ctx.now + delay as f64;
    }
    bot.state = BotState::Attack { since: ctx.now };
    bot.chase_timer = None;
    bot.route.clear();
    bot.route_goal = None;
    bot.next_move = ctx.now + bot.dice.range(t.attack_move_min.at(tp), t.attack_move_max.at(tp)) as f64;
    bot.move_to = None;
    bot.burst_left = 0;
    bot.pause_until = ctx.now;
    bot.aim_offset = None;
    let taken = ctx.chars.iter().any(|c| c.bot && c.crouching_attack == Some(target.e));
    if !taken && bot.dice.unit() < CROUCH_CHANCE {
        bot.crouch_until = ctx.now + CROUCH_HOLD;
    }
}

fn enter_chase(ctx: &Ctx, bot: &mut CaBot) {
    leave_attack(ctx, bot);
    let endurance = ctx.data.template.chase_endurance.at(bot.template) as f64;
    bot.state = BotState::Chase {
        until: ctx.now + endurance,
        breath: 0.0,
    };
    bot.route.clear();
    bot.route_goal = None;
}

/// Goal and state bookkeeping each frame (5.6, 2's decays).
fn goals(ctx: &Ctx, bot: &mut CaBot, me: &Char, guns: &[Gun]) {
    let now = ctx.now;
    let target = bot.target.and_then(|e| ctx.chars.iter().find(|c| c.e == e)).copied();
    match bot.state {
        BotState::Attack { since } => {
            let Some(target) = target else {
                leave_attack(ctx, bot);
                enter_patrol(ctx, bot);
                return;
            };
            if bot.visible {
                bot.chase_timer = None;
            } else {
                let end = *bot
                    .chase_timer
                    .get_or_insert_with(|| now + math::attack_chase_timer(bot.dice.unit()) as f64);
                if now >= end {
                    enter_chase(ctx, bot);
                    return;
                }
            }
            let d = u(me.pos.distance(target.pos));
            let melee = !ranged_usable(bot, guns);
            let reach = if melee { KNIFE_RANGE.max(bot.ranged_reach) } else { bot.ranged_reach };
            if d > reach && now - since >= 1.0 {
                enter_chase(ctx, bot);
            }
        }
        BotState::Chase { until, .. } => {
            if target.is_none() {
                enter_patrol(ctx, bot);
                return;
            }
            if now >= until {
                if bot.visible {
                    let endurance = ctx.data.template.chase_endurance.at(bot.template) as f64;
                    bot.state = BotState::Chase {
                        until: now + 1.0 + endurance,
                        breath: now + 1.0,
                    };
                } else {
                    enter_patrol(ctx, bot);
                }
            }
        }
        BotState::Investigate { until, .. } => {
            if now >= until {
                enter_patrol(ctx, bot);
            }
        }
        BotState::Dodge { until, to, grenade, chase } => {
            // Until the grenade is gone or the move ends (Correction 4);
            // then back to the chase it was in, to the attack when it has
            // a target, else idle.
            let gone = !ctx.dangers.iter().any(|d| d.e == grenade);
            if gone || now >= until || me.feet().xz().distance(to.xz()) < 0.25 {
                match target {
                    Some(_) if chase => enter_chase(ctx, bot),
                    Some(t) => enter_attack(ctx, bot, &t, false),
                    None => enter_patrol(ctx, bot),
                }
            }
        }
        BotState::Patrol => {}
    }
}

/// Grenade danger (5.4).
fn grenade_danger(ctx: &Ctx, bot: &mut CaBot, me: &Char) {
    let t = &ctx.data.template;
    let tp = bot.template;
    if let Some((g, _, _)) = bot.danger
        && !ctx.dangers.iter().any(|d| d.e == g)
    {
        bot.danger = None;
    }
    for g in ctx.dangers {
        if !g.player_thrown || !g.touched || g.team == me.team {
            continue;
        }
        match bot.danger {
            Some((other, _, _)) if other != g.e => continue,
            Some((_, last, _)) if u(last.distance(g.pos)) < DANGER_MOVED => continue,
            _ => {}
        }
        let d = u(g.pos.distance(me.pos));
        if d > g.radius + DANGER_MARGIN {
            continue;
        }
        let to = me.pos - g.pos;
        if let Ok(dir) = Dir3::new(to) {
            let blocked = ctx
                .q
                .cast_ray_predicate(g.pos, dir, to.length(), true, &sight_filter(), &|e| {
                    !ctx.see_through.contains(&e)
                })
                .is_some();
            if blocked {
                continue;
            }
        }
        let delay = bot.dice.range(t.dodge_delay_min.at(tp), t.dodge_delay_max.at(tp)) as f64;
        bot.danger = Some((g.e, g.pos, ctx.now + delay));
    }
    let Some((g, _, when)) = bot.danger else { return };
    if ctx.now < when || matches!(bot.state, BotState::Dodge { .. }) {
        return;
    }
    let Some(danger) = ctx.dangers.iter().find(|d| d.e == g) else { return };
    // The dodge is decided once per notice.
    bot.danger = bot.danger.map(|(g, at, _)| (g, at, f64::INFINITY));
    let from = danger.pos;
    let chase = matches!(bot.state, BotState::Chase { .. });
    if u(me.pos.distance(from)) > danger.radius {
        // Just outside the blast radius: onto the grenade itself, as the
        // reference does (Correction 4, kept for parity); only a bot
        // standing on that point stays.
        let mut to = from;
        to.y = me.feet().y;
        if me.feet().xz().distance(to.xz()) >= 0.25 && move_possible(ctx, me, to) {
            start_dodge(ctx, bot, to, g, chase);
        }
        return;
    }
    let away = Vec3::new(me.pos.x - from.x, 0.0, me.pos.z - from.z).normalize_or(Vec3::X);
    let r = danger.radius * DODGE_FACTOR * UNIT;
    for k in math::DODGE_STEPS {
        let dir = Quat::from_rotation_y(k as f32 * std::f32::consts::FRAC_PI_4) * away;
        let mut to = from + dir * r;
        to.y = me.feet().y;
        if move_possible(ctx, me, to) {
            start_dodge(ctx, bot, to, g, chase);
            return;
        }
    }
}

/// The grenade dodge goal activates with a point to go to (it says a
/// Grenade line).
fn start_dodge(ctx: &Ctx, bot: &mut CaBot, to: Vec3, grenade: Entity, chase: bool) {
    leave_attack(ctx, bot);
    bot.state = BotState::Dodge {
        to,
        until: ctx.now + DODGE_MAX,
        grenade,
        chase,
    };
    say(bot, Chat::Grenade);
}

/// A straight walk through the areas from `me` to `to` (feet level) with
/// no other bot on the line.
fn move_possible(ctx: &Ctx, me: &Char, to: Vec3) -> bool {
    let from = me.feet();
    if let Some(mesh) = ctx.mesh
        && !nav::straight(mesh, from, to, 0.25)
    {
        return false;
    }
    let seg = to - from;
    let len2 = seg.length_squared().max(1e-6);
    !ctx.chars.iter().any(|c| {
        if c.e == me.e || !c.bot || !c.alive {
            return false;
        }
        let p = c.feet();
        let k = ((p - from).dot(seg) / len2).clamp(0.0, 1.0);
        u((from + seg * k).distance(p)) < 2.0 * RADIUS
    })
}

/// The held weapon's AI row, else the chosen one's.
fn held_gun(ctx: &Ctx, bot: &CaBot, guns: &[Gun], _me: &Char) -> Option<GunAi> {
    bot.held
        .and_then(|h| guns.iter().find(|g| g.e == h))
        .or_else(|| slot(bot, guns, 0))
        .and_then(|g| gun_ai(ctx, bot, g.id))
}

/// The attack state (7-9).
#[allow(clippy::too_many_arguments)]
fn attack(ctx: &Ctx, bot: &mut CaBot, me: &Char, target: &Char, guns: &[Gun], active: Option<Entity>, io: &mut IntentOut, out: &mut Out) {
    let t = &ctx.data.template;
    let tp = bot.template;
    let d = u(me.pos.distance(target.pos));
    aim_and_fire(ctx, bot, me, target, guns, active, io, out);
    let gun = held_gun(ctx, bot, guns, me);
    let melee = bot.held.is_some_and(|h| guns.iter().any(|g| g.e == h && is_knife(bot, guns, g.id)));

    if ctx.frame {
        // Inside reach but beyond the held weapon: move in at once.
        if let Some(g) = gun
            && d > g.range.1
            && !melee
        {
            bot.next_move = bot.next_move.min(ctx.now);
        }
        // 9.1: attack moves.
        if ctx.now >= bot.next_move && !melee {
            let fwd = Vec3::new(target.pos.x - me.pos.x, 0.0, target.pos.z - me.pos.z).normalize_or(me.forward());
            let pick = bot.dice.int(0, 6) as usize;
            let shuffle = t.shuffle.at(tp);
            let flank_back = bot.dice.unit() < 0.5;
            let (rmin, rmax) = gun.map_or((0.0, f32::MAX), |g| g.range);
            let flank_forward = if rmin > 0.0 { (d - rmin).clamp(0.0, 50.0) } else { 50.0 };
            let here = me.feet() / UNIT;
            let tgt = target.feet() / UNIT;
            bot.move_to = None;
            for k in 0..7 {
                let m = MOVES[(pick + k) % 7];
                let mut dest = math::move_target(m, here, fwd, shuffle, RADIUS, flank_back, flank_forward);
                if d > rmax {
                    let dir = (dest - tgt).normalize_or_zero();
                    dest = tgt + dir * 0.8 * rmax;
                } else if matches!(m, Move::FlankLeft | Move::FlankRight) && rmin > 0.0 && dest.distance(tgt) < rmin {
                    dest = tgt + (dest - tgt).normalize_or_zero() * rmin;
                }
                let to = Vec3::new(dest.x * UNIT, me.feet().y, dest.z * UNIT);
                if move_possible(ctx, me, to) {
                    bot.move_to = Some(to);
                    bot.move_until = ctx.now + 2.0;
                    break;
                }
            }
            bot.next_move = ctx.now + bot.dice.range(t.attack_move_min.at(tp), t.attack_move_max.at(tp)) as f64;
        }
        // 9.2: dodging fire.
        let aiming = {
            let to_me = (me.pos - target.pos).normalize_or_zero();
            target.look().dot(to_me) > AIMING_DOT && me.look().dot(-to_me) > AIMING_DOT
        };
        let check = std::mem::take(&mut bot.dodge_hit) || (aiming && bot.passes % 5 == 0);
        if check && ctx.now >= bot.dodge_next && !melee {
            dodge(ctx, bot, me, target);
        }
    }
    // Moving: the attack move, or in to the knife's reach.
    if melee && d > KNIFE_RANGE {
        io.axis = axis_toward(target.pos - me.pos, io.yaw);
    } else if let Some(to) = bot.move_to {
        let d = to - me.feet();
        if d.xz().length() < 0.1 || ctx.now > bot.move_until {
            bot.move_to = None;
        } else {
            io.axis = axis_toward(d, io.yaw);
        }
    }
}

/// 9.2: 70 % shuffle, 30 % roll, a random side, then the other side,
/// then the other action.
fn dodge(ctx: &Ctx, bot: &mut CaBot, me: &Char, target: &Char) {
    let shuffle = ctx.data.template.shuffle.at(bot.template) + RADIUS;
    let fwd = Vec3::new(target.pos.x - me.pos.x, 0.0, target.pos.z - me.pos.z).normalize_or(me.forward());
    let right = fwd.cross(Vec3::Y).normalize_or_zero();
    let first_roll = bot.dice.unit() < 0.3;
    let side = if bot.dice.unit() < 0.5 { 1.0 } else { -1.0 };
    for (roll, s) in [(first_roll, side), (first_roll, -side), (!first_roll, side), (!first_roll, -side)] {
        let dist = if roll { ROLL } else { shuffle };
        let to = me.feet() + right * s * dist * UNIT;
        if move_possible(ctx, me, to) {
            bot.move_to = Some(to);
            bot.move_until = ctx.now + 1.0;
            bot.dodge_next = ctx.now + DODGE_DELAY;
            return;
        }
    }
}

/// 8.2: the weapon for a target at `d` u, kept in `held` until it needs
/// choosing again.
fn pick_weapon(ctx: &Ctx, bot: &mut CaBot, guns: &[Gun], d: f32) -> Option<Gun> {
    let now = ctx.now;
    let choice = |k: usize| {
        slot(bot, guns, k).map(|g| Choice {
            has_ammo: g.has_ammo(),
            range_max: gun_ai(ctx, bot, g.id).map_or(0.0, |a| a.range.1),
        })
    };
    let pick = math::choose_weapon(choice(0), choice(1), choice(2), d);
    let want = slot(bot, guns, pick).or_else(|| slot(bot, guns, 3));
    let current = bot.held.and_then(|h| guns.iter().find(|g| g.e == h));
    let rechoose = match current {
        None => true,
        Some(c) => {
            let ai = gun_ai(ctx, bot, c.id);
            !c.has_ammo() || ai.is_some_and(|a| d < a.range.0 || d > a.range.1) || c.count.is_some()
        }
    };
    if rechoose && let Some(w) = want {
        if bot.held != Some(w.e) {
            bot.burst_left = 0;
            bot.pause_until = now;
            bot.last_clip = None;
        }
        bot.held = Some(w.e);
    }
    bot.held.and_then(|h| guns.iter().find(|g| g.e == h)).copied()
}

/// Weapon choice, aim and trigger (7, 8).
#[allow(clippy::too_many_arguments)]
fn aim_and_fire(ctx: &Ctx, bot: &mut CaBot, me: &Char, target: &Char, guns: &[Gun], active: Option<Entity>, io: &mut IntentOut, out: &mut Out) {
    let now = ctx.now;
    let d = u(me.pos.distance(target.pos));
    if let Some(th) = bot.throw {
        throw_step(ctx, bot, me, target, guns, active, th, io, out);
        return;
    }
    let Some(held) = pick_weapon(ctx, bot, guns, d) else { return };
    out.wanted = Some(held.e);
    let gun = gun_ai(ctx, bot, held.id).unwrap_or_default();

    // 7.3: the aim point for this shot. A mercenary aims like PvE AI
    // (bots_objectives.md 3.3 Correction 1): at its height above the hull
    // centre, the cycle in fixed order, an aimed shot exactly 100 u off
    // while the target moves, no camping bonus, no head-only halving.
    let merc = bot.merc_aim;
    let base = if bot.head_only {
        target.head()
    } else if merc {
        target.pos + Vec3::Y * bot.merc_height * UNIT
    } else {
        target.torso()
    };
    if bot.aim_offset.is_none() {
        let camp = if merc { 0.0 } else { ctx.camping.get(&target.e).copied().unwrap_or(0.0) };
        let base_u = base / UNIT;
        let offset = if math::full_accuracy(d, gun.full_accuracy) {
            Vec3::ZERO
        } else {
            let a = math::accuracy(gun.a, camp, bot.head_only && !merc);
            let aimed = if merc {
                bot.cycle.ordered_shot(a)
            } else {
                let r = bot.dice.r100();
                bot.cycle.shot(a, r)
            };
            if aimed {
                let draws = Vec3::new(bot.dice.unit(), bot.dice.unit(), bot.dice.unit());
                let point = if merc {
                    math::merc_hit_point(base_u, bot.tracking.c, draws)
                } else {
                    math::hit_point(base_u, bot.tracking.c, draws)
                };
                point - base_u
            } else {
                let right = bot.dice.unit() < 0.5;
                let up = bot.dice.range(-5.0, 15.0);
                math::miss_point(me.pos / UNIT, base_u, gun.miss_offset, right, up) - base_u
            }
        };
        bot.aim_offset = Some(offset);
    }
    let aim = base + bot.aim_offset.unwrap_or_default() * UNIT;
    let (yaw, pitch) = look_at(me.eye(), aim);
    io.yaw = yaw;
    io.pitch = pitch;

    // 8.5: a grenade after its count of shots: the bot switches to it and
    // throws (`throw_step`); nothing is fired meanwhile.
    if ctx.frame
        && bot.grenades == 0
        && bot.visible
        && !bot.head_only
        && d <= bot.thrown_reach
        && let Some(g) = slot(bot, guns, 4).filter(|g| g.count.unwrap_or(0) > 0)
    {
        bot.throw = Some(Throw {
            weapon: g.e,
            begun: now,
            started: None,
            near: d < NEAR_THROW,
            released: false,
        });
        out.wanted = Some(g.e);
        return;
    }

    // 8.3: bursts by the clip, reloads.
    let drawn = active == Some(held.e);
    if let Some(prev) = bot.last_clip
        && held.clip < prev
    {
        let spent = prev - held.clip;
        bot.burst_left = bot.burst_left.saturating_sub(spent);
        if bot.burst_left == 0 {
            bot.pause_until = now + bot.dice.range(gun.pause.0, gun.pause.1) as f64;
        }
    }
    bot.last_clip = Some(held.clip);
    if held.count.is_none() && held.clip == 0 && held.reserve > 0 && gun.burst.1 > 0 {
        io.reload = true;
        bot.burst_left = 0;
        bot.pause_until = now;
        bot.next_move = bot.next_move.min(now);
    }
    if bot.burst_left == 0 && now >= bot.pause_until {
        bot.burst_left = bot.dice.int(gun.burst.0.max(1), gun.burst.1.max(1));
    }
    let knife = is_knife(bot, guns, held.id);
    let in_reach = !knife || d <= KNIFE_RANGE;
    let line = fire_line(ctx, me, target.e, me.side_origin(), base);
    if let Some(Some(mate)) = line
        && ctx.chars.iter().any(|c| c.e == mate && c.team == me.team)
    {
        // 9.3: a teammate in the line of fire.
        if u(me.pos.distance(ctx.chars.iter().find(|c| c.e == mate).unwrap().pos)) > CROUCH_ASK {
            out.ask_crouch = Some(mate);
        } else if ctx.frame && now >= bot.dodge_next {
            dodge(ctx, bot, me, target);
        }
    }
    let fire = drawn
        && in_reach
        && now >= bot.fire_at
        && now >= bot.next_shot
        && bot.visible
        && line.is_none()
        && (knife || held.clip > 0)
        && (knife || bot.burst_left > 0)
        && now >= bot.pause_until;
    io.fire = fire && !std::mem::take(&mut bot.release);
}

/// The target is nearer than this (u): the close throw animation (8.5).
pub const NEAR_THROW: f32 = 1000.0;

/// A grenade throw in progress (8.5, Correction 9): the bot has the
/// grenade drawn (it switched to it), the throw animation plays and the
/// grenade leaves the hand at the throw key; the animation over, the gun is
/// chosen again and switched back to. Nothing is fired meanwhile. The
/// animation's length and key are the grenade's (`Throwable`: the throw
/// key and the throw time), the same for both animations (Open question
/// 27).
#[allow(clippy::too_many_arguments)]
fn throw_step(ctx: &Ctx, bot: &mut CaBot, me: &Char, target: &Char, guns: &[Gun], active: Option<Entity>, mut th: Throw, io: &mut IntentOut, out: &mut Out) {
    let now = ctx.now;
    out.wanted = Some(th.weapon);
    io.fire = false;
    let (yaw, pitch) = look_at(me.eye(), target.torso());
    io.yaw = yaw;
    io.pitch = pitch;
    if th.started.is_none() && active == Some(th.weapon) {
        th.started = Some(now);
    }
    let Some(start) = th.started else {
        // Never drew it: give up.
        if now - th.begun > THROW_PATIENCE {
            bot.throw = None;
            bot.held = None;
            reset_grenades(ctx, bot);
        } else {
            bot.throw = Some(th);
        }
        return;
    };
    let grenade = guns.iter().find(|g| g.e == th.weapon);
    let key = grenade.map_or(THROW_KEY, |g| g.throw_delay) as f64;
    let total = grenade.map_or(THROW_TIME, |g| g.throw_time.max(g.throw_delay)) as f64;
    if !th.released && now + 1e-9 >= start + key {
        let from = me.hand();
        let v = math::throw_velocity(from / UNIT, target.pos / UNIT, HANG_TIME, GRAVITY, 1.0, 0.0);
        out.throw = Some((th.weapon, from, v * UNIT));
        th.released = true;
        reset_grenades(ctx, bot);
    }
    if th.released && now + 1e-9 >= start + total {
        // Over: choose the gun again.
        bot.throw = None;
        bot.held = None;
        bot.burst_left = 0;
        bot.pause_until = now;
        bot.last_clip = None;
        return;
    }
    bot.throw = Some(th);
}

/// A throw whose grenade is never drawn is given up after this (s); the
/// throw key and animation when the grenade has none (s).
pub const THROW_PATIENCE: f64 = 3.0;
pub const THROW_KEY: f32 = 0.4;
pub const THROW_TIME: f32 = 0.9;

/// Wrap an angle into -pi..pi.
fn wrap_pi(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// Firing while chasing (Correction 10, 5.6, 8.2): whenever the target is
/// visible from the weapon and in reach, with no reaction delay and no
/// burst pause: the whole clip, then an instant reload. The shots are
/// blind: each goes along the bot's torso facing, which turns toward the
/// target's head within `TORSO_YAW`/`TORSO_PITCH` of the run heading (Open
/// question 28), not to the aim point of 7.3 (no hit/miss cycle).
#[allow(clippy::too_many_arguments)]
fn chase_fire(ctx: &Ctx, bot: &mut CaBot, me: &Char, target: &Char, guns: &[Gun], active: Option<Entity>, io: &mut IntentOut, out: &mut Out) {
    let now = ctx.now;
    let d = u(me.pos.distance(target.pos));
    let Some(held) = pick_weapon(ctx, bot, guns, d) else { return };
    out.wanted = Some(held.e);
    // The torso turns toward the head while the legs run the path.
    let run = io.yaw;
    let (to_yaw, to_pitch) = look_at(me.eye(), target.head());
    let yaw = run + wrap_pi(to_yaw - run).clamp(-TORSO_YAW, TORSO_YAW);
    if io.axis != Vec2::ZERO {
        io.axis = axis_toward(Vec3::new(-run.sin(), 0.0, -run.cos()), yaw);
    }
    io.yaw = yaw;
    io.pitch = to_pitch.clamp(-TORSO_PITCH, TORSO_PITCH);
    let knife = is_knife(bot, guns, held.id);
    if !knife && held.count.is_none() && held.clip == 0 && held.reserve > 0 {
        out.refill = Some(held.e);
    }
    let view = weapon_view(ctx, me, target);
    let base = if view == Some(true) { target.head() } else { target.torso() };
    let line = fire_line(ctx, me, target.e, me.side_origin(), base);
    let fire = active == Some(held.e)
        && view.is_some()
        && line.is_none()
        && (!knife || d <= KNIFE_RANGE)
        && now >= bot.next_shot
        && (knife || held.clip > 0);
    io.fire = fire && !std::mem::take(&mut bot.release);
}

/// Walk toward `goal` (feet, m) along a planned path (3).
fn walk(ctx: &Ctx, bot: &mut CaBot, me: &Char, goal: Vec3, diverge: bool, io: &mut IntentOut) {
    let feet = me.feet();
    let plan = bot.route.is_empty() || bot.route_goal.is_none_or(|g| g.distance(goal) > 1.0 && bot.route.len() <= 1);
    if plan {
        bot.route_goal = Some(goal);
        bot.route = match ctx.mesh {
            Some(mesh) => {
                let (Some(a), Some(b)) = (mesh.nearest_area(feet + Vec3::Y * 0.3), mesh.nearest_area(goal + Vec3::Y * 0.3)) else {
                    return;
                };
                let reserved: HashSet<usize> = if diverge {
                    ctx.chars
                        .iter()
                        .filter(|c| c.bot && c.e != me.e && c.alive)
                        .filter_map(|c| mesh.area_at(c.feet() + Vec3::Y * 0.3))
                        .collect()
                } else {
                    HashSet::new()
                };
                match nav::find(mesh, a, feet, b, UNIT, 0.0, |i| if reserved.contains(&i) { 1.0 + nav::RESERVED } else { 1.0 }) {
                    Some(p) => nav::points(mesh, &p, feet, goal),
                    None => Vec::new(),
                }
            }
            None => vec![goal],
        };
        bot.stuck_check = (ctx.now, feet);
    }
    while let Some(&p) = bot.route.first() {
        if p.xz().distance(feet.xz()) < 0.35 && (p.y - feet.y).abs() < 1.2 {
            bot.route.remove(0);
        } else {
            break;
        }
    }
    let Some(&next) = bot.route.first() else { return };
    let (yaw, _) = look_at(feet, next);
    io.yaw = yaw;
    io.pitch = 0.0;
    io.axis = Vec2::Y;
    // A ledge above a stair step just ahead: jump up it.
    if next.y - feet.y > JUMP_RISE && next.xz().distance(feet.xz()) < JUMP_AHEAD {
        bot.jump = true;
    }
    // Ours: no progress for 1.5 s → plan again and jump.
    if ctx.now - bot.stuck_check.0 > 1.5 {
        if bot.stuck_check.1.distance(feet) < 0.3 {
            bot.route.clear();
            bot.jump = true;
            bot.patrol_to = None;
        }
        bot.stuck_check = (ctx.now, feet);
    }
}

/// The patrol (4): node to node of the patrol graph by the link choice,
/// along planned paths.
fn patrol(ctx: &Ctx, bot: &mut CaBot, me: &Char, io: &mut IntentOut, out: &mut Out) {
    let (Some(_), Some(graph)) = (ctx.mesh, ctx.graph) else { return };
    if graph.nodes.is_empty() {
        return;
    }
    // Standing at a node to look around (4).
    if ctx.now < bot.patrol_wait {
        return;
    }
    let feet = me.feet();
    if bot.patrol_to.is_none() {
        let here = bot.patrol_from.filter(|&n| n < graph.nodes.len()).or_else(|| graph.nearest(feet));
        let Some(here) = here else { return };
        let from = bot.patrol_came;
        let mut links: Vec<usize> = graph.links[here].iter().copied().filter(|&n| Some(n) != from).collect();
        if links.is_empty() {
            links = graph.links[here].clone();
        }
        if links.is_empty() {
            // A node with no links: head for the nearest other one.
            links = (0..graph.nodes.len()).filter(|&n| n != here).collect();
        }
        let at = graph.points[here];
        let options: Vec<(f64, Vec3)> = links
            .iter()
            .map(|&n| {
                (
                    ctx.patrol.reached.get(&n).copied().unwrap_or(0.0),
                    graph.points[n] - at,
                )
            })
            .collect();
        bot.patrol_to = math::patrol_choice(&options, me.forward()).map(|i| links[i]);
        bot.patrol_came = Some(here);
        bot.route.clear();
        bot.route_goal = None;
        bot.stuck_check = (ctx.now, feet);
    }
    let Some(to) = bot.patrol_to.filter(|&n| n < graph.nodes.len()) else {
        bot.patrol_to = None;
        return;
    };
    let goal = graph.points[to];
    if goal.xz().distance(feet.xz()) < 0.8 && (goal.y - feet.y).abs() < 1.5 {
        out.reached = Some(to);
        bot.patrol_from = Some(to);
        bot.patrol_to = None;
        // Every node is a look-around node: it stops with the template's
        // chance for 3 + U(0, 3) s, else runs on.
        let percent = ctx.data.template.stop_look_around_percent.at(bot.template);
        if math::patrol_stops(bot.dice.r100(), percent) {
            bot.patrol_wait = ctx.now + (STOP_MIN + bot.dice.range(0.0, STOP_SPREAD)) as f64;
        }
        return;
    }
    walk(ctx, bot, me, goal, false, io);
    if bot.route.is_empty() {
        // No path there: another node next time.
        bot.patrol_from = Some(to);
        bot.patrol_to = None;
    }
}

// ---------------------------------------------------------------------------
// Hired Guns mercenaries (bots_objectives.md 3)

/// The follow state's arrive radius (u, 3D).
pub const FOLLOW_ARRIVE: f32 = 200.0;
/// The leave radius: √(200² + 1000) u.
pub fn follow_leave() -> f32 {
    (FOLLOW_ARRIVE * FOLLOW_ARRIVE + 1000.0).sqrt()
}
/// The leave delay (s).
pub const FOLLOW_LEAVE_DELAY: f64 = 0.25;
/// The path to the follow target is planned again at least this often (s).
pub const FOLLOW_REPLAN: f64 = 1.0;
/// The ranged attack goal's decay after the last sight (s).
pub const MERC_ATTACK_DECAY: f64 = 2.0;

/// A mercenary's tick (3.3): the `Squad` goal set's attack (ranged,
/// dynamic: the bot brain's attack, 7-9) over its follow goal (3.4);
/// under "follow me" the follow goal only. No patrol, chase, investigate
/// or grenade dodge; sight is never rolled (Open question 5: no relaxed
/// state, so the alert field of view).
fn decide_merc(ctx: &Ctx, bot: &mut CaBot, mind: &mut MercMind, me: &Char, guns: &[Gun], active: Option<Entity>) -> Out {
    let mut out = Out::default();
    let mut io = IntentOut {
        yaw: me.yaw,
        pitch: me.pitch,
        ..default()
    };
    if let Some(target) = bot.target
        && !ctx.chars.iter().any(|c| c.e == target && c.alive)
    {
        bot.target = None;
        if matches!(bot.state, BotState::Attack { .. }) {
            leave_attack(ctx, bot);
        }
        bot.state = BotState::Patrol;
    }
    if ctx.frame {
        bot.passes += 1;
        // The current target's visibility (the target update).
        if let Some(target) = bot.target.and_then(|e| ctx.chars.iter().find(|c| c.e == e)) {
            let seen = sight(ctx, me, target, true);
            bot.visible = seen.is_some();
            bot.head_only = seen == Some(true);
            let gun = held_gun(ctx, bot, guns, me);
            bot.tracking
                .update(target.pos / UNIT, SERVER_FRAME as f32, gun.map_or(0.0, |g| g.c_max), gun.map_or(0.0, |g| g.decay));
            if bot.visible {
                mind.seen_at = ctx.now;
                // The aim point steps down the hull each update the
                // weapon's ray to it fails (bots_objectives.md 3.3).
                let aim = target.pos + Vec3::Y * bot.merc_height * UNIT;
                if matches!(fire_line(ctx, me, target.e, me.side_origin(), aim), Some(None)) {
                    let h = target.hull_height();
                    bot.merc_height -= h / 8.0;
                    if bot.merc_height < -h / 2.0 - 1e-3 {
                        bot.merc_height = h / 2.0;
                    }
                }
            }
        }
        if mind.follow_only {
            if matches!(bot.state, BotState::Attack { .. }) {
                leave_attack(ctx, bot);
            }
            bot.state = BotState::Patrol;
            bot.target = None;
        } else {
            // The nearest enemy in sight (players and mercenaries alike).
            let mut enemies: Vec<(&Char, f32)> = ctx
                .chars
                .iter()
                .filter(|c| c.alive && c.team != me.team && crate::game::rules::state::plays(c.team))
                .map(|c| (c, u(me.pos.distance(c.pos))))
                .filter(|(_, d)| *d < SIGHT_RANGE)
                .collect();
            enemies.sort_by(|a, b| a.1.total_cmp(&b.1));
            let seen = enemies
                .into_iter()
                .find_map(|(c, _)| sight(ctx, me, c, true).map(|head| (*c, head)));
            if let Some((target, head_only)) = seen {
                mind.seen_at = ctx.now;
                // A new target only when the current one is out of sight.
                if bot.target != Some(target.e) && !(bot.visible && bot.target.is_some()) {
                    bot.target = Some(target.e);
                    let c_max = held_gun(ctx, bot, guns, me).map_or(0.0, |g| g.c_max);
                    bot.tracking.select(c_max);
                    bot.tracking.prev = Some(target.pos / UNIT);
                    reset_grenades(ctx, bot);
                    bot.aim_offset = None;
                    bot.visible = true;
                    bot.head_only = head_only;
                    bot.merc_height = MERC_HEIGHT;
                }
                if !matches!(bot.state, BotState::Attack { .. }) {
                    enter_attack(ctx, bot, &target, true);
                }
            } else if matches!(bot.state, BotState::Attack { .. }) && ctx.now - mind.seen_at >= MERC_ATTACK_DECAY - 1e-9 {
                leave_attack(ctx, bot);
                bot.state = BotState::Patrol;
                bot.target = None;
                mind.holding = false;
            }
        }
    }
    let target = bot.target.and_then(|e| ctx.chars.iter().find(|c| c.e == e)).copied();
    match (bot.state, target) {
        (BotState::Attack { .. }, Some(target)) if !mind.follow_only => {
            attack(ctx, bot, me, &target, guns, active, &mut io, &mut out);
        }
        _ => follow(ctx, bot, mind, me, &mut io),
    }
    io.jump = std::mem::take(&mut bot.jump);
    if ctx.settings.dont_shoot != 0 || mind.follow_only {
        io.fire = false;
    }
    if ctx.settings.stop != 0 {
        io.axis = Vec2::ZERO;
        io.jump = false;
    }
    out.intent = Some(io);
    out
}

/// The follow goal (3.4): to the follow point, else the leader's current
/// position; stop within the arrive radius and hold (facing the leader)
/// until the goal is beyond the leave radius for the leave delay; the
/// path planned again at least every second.
fn follow(ctx: &Ctx, bot: &mut CaBot, mind: &mut MercMind, me: &Char, io: &mut IntentOut) {
    let leader = ctx.chars.iter().find(|c| c.e == mind.leader).copied();
    let point = mind.follow_point.filter(|p| *p != Vec3::ZERO);
    let goal = match (point, leader) {
        (Some(p), _) => p,
        (None, Some(l)) => l.feet(),
        (None, None) => {
            io.axis = Vec2::ZERO;
            return;
        }
    };
    let d = u(me.feet().distance(goal));
    if ctx.frame {
        if mind.holding {
            if d <= follow_leave() {
                mind.leave_at = ctx.now + FOLLOW_LEAVE_DELAY;
            } else if ctx.now + 1e-9 >= mind.leave_at {
                mind.holding = false;
                bot.route.clear();
                bot.route_goal = None;
                mind.replan_at = ctx.now + FOLLOW_REPLAN;
            }
        } else if d < FOLLOW_ARRIVE {
            mind.holding = true;
            mind.leave_at = ctx.now + FOLLOW_LEAVE_DELAY;
            bot.route.clear();
            bot.route_goal = None;
        } else if ctx.now + 1e-9 >= mind.replan_at {
            bot.route.clear();
            bot.route_goal = None;
            mind.replan_at = ctx.now + FOLLOW_REPLAN;
        }
    }
    if mind.holding {
        io.axis = Vec2::ZERO;
        if let Some(l) = leader.filter(|_| point.is_none()) {
            let (yaw, _) = look_at(me.eye(), l.eye());
            io.yaw = yaw;
            io.pitch = 0.0;
        }
        return;
    }
    walk(ctx, bot, me, goal, false, io);
}
