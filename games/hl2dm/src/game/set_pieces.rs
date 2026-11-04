//! The stock maps' set pieces (specs/hl2dm/maps_entities.md 8, 12, 16):
//! dm_runoff's headcrab canisters, `game_ragdoll_manager`, and what the
//! AI sound list hears for `env_microphone` (source_logic's
//! `sensors`).
//!
//! - **`env_headcrabcanister`** (`Canister`): made when the logic tells of
//!   one (`MapEntitySpawned`: at the map's load, after a restart, or a
//!   `point_template`'s copy, which dm_runoff's strike spawns), fired by
//!   `FireCanister` (once). It flies in the world along its angles'
//!   opposite for `FlightTime` at `FlightSpeed`, thinking every 13 ticks:
//!   the incoming sound within 1 s of the impact, a sweep that shoves and
//!   hurts what lies in its path, early detonation within 126 units of the
//!   impact; at the time it blasts (`DamageRadius`, `Damage`, the
//!   squared falloff, 750 units/s shove on physics objects, 250 to 1000 on
//!   players), shakes the view (amplitude 16, radius 1024) and lands,
//!   opening (`OnOpened`) unless it waits for `OpenCanister`. Outputs
//!   `OnLaunched`, `OnImpacted`, `OnOpened`. Server-side.
//! - **`game_ragdoll_manager`** (`RagdollLimit`): `MaxRagdollCount`
//!   (-1: 8) and the inputs `SetMaxRagdollCount`, `SetMaxRagdollCountDX8`.
//!   HL2:DM's player bodies aren't in the list it limits and no stock map
//!   has other ragdolls, so nothing reads the number yet.
//! - **AI sounds** (`ai_sounds`): each bullet impact (volume 200 units, 0.5
//!   s) and blast (the larger of 128 and 1.5 of its radius, 0.25 s) is told
//!   to the logic (`map::entities::AiSound`).
//!
//! Known shortcuts: the canister's model, its flight trail, smoke, the
//! explosion effect and the 3D skybox arc before it enters the world
//! aren't drawn (it flies unseen; the flight's timing, damage, sounds and
//! shake are the spec's); `StartingHeight`, `LaunchPositionName`,
//! "start impacted" and the skybox-only shots are not built (no stock
//! canister uses them); the sweep and the blast test the distance of a
//! body's centre, not its bounds, and the sweep is the spec's cube as a
//! sphere of its half-diagonal; the shake reaches players in the air too;
//! headcrabs are never released (`HeadcrabCount` 0 in the stock maps); the
//! early detonation still lets the canister fly on without effect.

use avian3d::prelude::{LinearVelocity, Position, RigidBody, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;

use crate::{
    core::{Damage, DamageKind, Damageable, Explosion, Health, Intent, MapDamage, MovementState, SimClock, Velocity},
    map::{
        PlaySound,
        entities::{
            AiSound, AiSoundKind, FireKeyedOutput, GameEntityClasses, MapEntityInput, MapEntityRemoved, MapEntitySpawned,
            entity_rotation, entity_to_engine,
        },
        logic_view::MapShake,
    },
    weapon::{WeaponEvent, WeaponEventKind},
};

const UNIT: f32 = 0.0254;

/// Classes the game handles itself (`map::entities::GameEntityClasses`).
pub const CLASSES: &[&str] = &[
    "item_suitcharger",
    "item_healthcharger",
    "env_headcrabcanister",
    "game_ragdoll_manager",
];

/// The canister's think period (ticks, 0.2 s at 0.015 s).
pub const THINK_TICKS: u64 = 13;
/// The canister's box as a sphere: the cube of half-size 73 has this
/// half-diagonal (units).
pub const SWEEP_RADIUS: f32 = 126.44;
/// Within this of the impact (1 s of flight) the incoming sound starts.
pub const INCOMING_DISTANCE: f32 = 3000.0;
/// The physics shove of a blast or sweep (units/s per unit of scale).
pub const PUSH: f32 = 750.0;
/// The shake an impact makes: amplitude (capped), radius (units).
pub const SHAKE_AMPLITUDE: f32 = 16.0;
pub const SHAKE_RADIUS: f32 = 1024.0;
/// Seconds until `OnOpened` after landing (the `open` sequence).
pub const OPEN_TIME: f64 = 35.0 / 30.0;
/// An impact with flag 262144 removes the canister after this (s).
pub const REMOVE_AFTER: f64 = 3.0;
/// The ray the flight's entry point is found with (units).
const ENTRY_RAY: f32 = 10_000.0;
const INCOMING: &str = "HeadcrabCanister.IncomingSound";

const SF_NO_IMPACT_SOUND: u32 = 1;
const SF_NO_LAUNCH_SOUND: u32 = 2;
const SF_AT_ORIGIN: u32 = 8192;
const SF_WAIT_OPEN: u32 = 16384;
const SF_NO_SHAKE: u32 = 131_072;
const SF_REMOVE_ON_IMPACT: u32 = 262_144;

/// A canister's keyvalues.
#[derive(Clone, Debug, PartialEq)]
pub struct CanisterParams {
    pub speed: f32,
    pub time: f32,
    pub damage: f32,
    pub radius: f32,
    pub flags: u32,
}

impl CanisterParams {
    pub fn read(kv: &[(String, String)]) -> Self {
        let f = |k: &str| {
            kv.iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(k))
                .and_then(|(_, v)| v.trim().parse::<f32>().ok())
                .unwrap_or(0.0)
        };
        Self {
            speed: f("FlightSpeed"),
            time: f("FlightTime"),
            damage: f("Damage"),
            radius: f("DamageRadius"),
            flags: f("spawnflags") as u32,
        }
    }

    fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }
}

/// Where a canister is in its life.
#[derive(Clone, Debug, PartialEq)]
pub enum CanState {
    Idle,
    /// Launched at `launched` (s); in the world from `enter` (s since
    /// launch); `last` is the position the last think left it at.
    Flying {
        launched: f64,
        enter: f32,
        incoming: bool,
        detonated: bool,
        last: Option<Vec3>,
    },
    /// Landed; opening since `since` once `started` (at once, or at
    /// `OpenCanister`); `opened` once `OnOpened` fired.
    Landed { since: f64, started: bool, opened: bool },
    /// Gone at that time (flag 262144).
    Removed(f64),
}

/// A headcrab canister (`env_headcrabcanister`).
#[derive(Component, Clone, Debug)]
pub struct Canister {
    pub key: u64,
    pub params: CanisterParams,
    /// Engine space, meters: where it lands (its origin) and the unit
    /// vector it flies along (opposite its forward axis).
    pub impact: Vec3,
    pub dir: Vec3,
    pub state: CanState,
    pub fired: bool,
}

impl Canister {
    /// Its position `t` seconds after launch (m), on the straight flight
    /// to the impact.
    pub fn position(&self, t: f32) -> Vec3 {
        let t = t.min(self.params.time);
        self.impact - self.dir * (self.params.time - t) * self.params.speed * UNIT
    }
}

/// `game_ragdoll_manager`'s limit on the ragdoll retirement list.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RagdollLimit {
    pub max: i32,
    pub max_dx8: i32,
}

impl Default for RagdollLimit {
    fn default() -> Self {
        Self { max: DEFAULT_RAGDOLLS, max_dx8: -1 }
    }
}

/// `g_ragdoll_maxcount`.
pub const DEFAULT_RAGDOLLS: i32 = 8;

impl RagdollLimit {
    /// The count in force (-1: the default).
    pub fn current(&self) -> i32 {
        if self.max < 0 { DEFAULT_RAGDOLLS } else { self.max }
    }
}

pub struct SetPiecesPlugin;

impl Plugin for SetPiecesPlugin {
    fn build(&self, app: &mut App) {
        {
            let mut classes = app.world_mut().get_resource_or_init::<GameEntityClasses>();
            for c in CLASSES {
                if !classes.0.contains(c) {
                    classes.0.push(c);
                }
            }
        }
        app.add_message::<MapEntityInput>()
            .add_message::<MapEntitySpawned>()
            .add_message::<MapEntityRemoved>()
            .add_message::<FireKeyedOutput>()
            .add_message::<AiSound>()
            .add_message::<MapShake>()
            .add_message::<PlaySound>()
            .add_message::<Damage>()
            .add_message::<MapDamage>()
            .add_message::<Explosion>()
            .add_message::<WeaponEvent>()
            .init_resource::<RagdollLimit>()
            .add_systems(
                FixedUpdate,
                (track, ragdoll_manager, canister_inputs, canister_flight, ai_sounds)
                    .chain()
                    .run_if(crate::core::authoritative),
            );
    }
}

/// The logic's tell of canisters and managers: made, and removed.
fn track(
    mut spawned: MessageReader<MapEntitySpawned>,
    mut removed: MessageReader<MapEntityRemoved>,
    cans: Query<(Entity, &Canister)>,
    mut limit: ResMut<RagdollLimit>,
    mut commands: Commands,
) {
    for r in removed.read() {
        for (e, c) in &cans {
            if c.key == r.key {
                commands.entity(e).despawn();
            }
        }
    }
    for s in spawned.read() {
        match s.class.to_ascii_lowercase().as_str() {
            "env_headcrabcanister" => {
                for (e, c) in &cans {
                    if c.key == s.key {
                        commands.entity(e).despawn();
                    }
                }
                let params = CanisterParams::read(&s.keyvalues);
                // Its forward axis points back along the flight.
                let forward = entity_to_engine(entity_rotation(s.angles) * Vec3::X, 1.0).normalize_or_zero();
                commands.spawn((
                    Name::new("env_headcrabcanister"),
                    Canister {
                        key: s.key,
                        impact: entity_to_engine(s.origin, UNIT),
                        dir: -forward,
                        params,
                        state: CanState::Idle,
                        fired: false,
                    },
                ));
            }
            "game_ragdoll_manager" => {
                let n = |k: &str| {
                    s.keyvalues
                        .iter()
                        .find(|(n, _)| n.eq_ignore_ascii_case(k))
                        .and_then(|(_, v)| v.trim().parse::<i32>().ok())
                        .unwrap_or(-1)
                };
                *limit = RagdollLimit { max: n("MaxRagdollCount"), max_dx8: n("MaxRagdollCountDX8") };
            }
            _ => {}
        }
    }
}

fn ragdoll_manager(mut inputs: MessageReader<MapEntityInput>, mut limit: ResMut<RagdollLimit>) {
    for i in inputs.read() {
        let n = i.value.trim().parse::<f32>().unwrap_or(0.0) as i32;
        match i.input.as_str() {
            "setmaxragdollcount" => limit.max = n,
            "setmaxragdollcountdx8" => limit.max_dx8 = n,
            _ => {}
        }
    }
}

fn say(play: &mut MessageWriter<PlaySound>, entry: &str) {
    play.write(PlaySound::ui(entry));
}

fn canister_inputs(
    mut inputs: MessageReader<MapEntityInput>,
    mut cans: Query<&mut Canister>,
    clock: Res<SimClock>,
    mut play: MessageWriter<PlaySound>,
    mut outputs: MessageWriter<FireKeyedOutput>,
    spatial: SpatialQuery,
) {
    for i in inputs.read() {
        let Some(mut c) = cans.iter_mut().find(|c| c.key == i.key) else { continue };
        let c = &mut *c;
        match i.input.as_str() {
            "firecanister" if !c.fired => {
                c.fired = true;
                if !c.params.has(SF_NO_LAUNCH_SOUND) {
                    say(&mut play, "HeadcrabCanister.LaunchSound");
                }
                outputs.write(FireKeyedOutput { key: c.key, output: "OnLaunched".into(), activator: None });
                // Without flag 8192 it lands where a 10000-unit trace from
                // its origin along the flight hits the world.
                if !c.params.has(SF_AT_ORIGIN)
                    && let Ok(d) = Dir3::new(c.dir)
                {
                    let end = spatial
                        .cast_ray(c.impact, d, ENTRY_RAY * UNIT, true, &SpatialQueryFilter::default())
                        .map_or(ENTRY_RAY * UNIT, |h| h.distance);
                    let step = c.dir * end;
                    c.impact += step;
                }
                // The flight's entry into the world: the sky along its
                // forward axis (a ray against the map, 10000 units).
                let v = c.params.speed;
                let reach = if v > 0.0 {
                    let from = c.impact - c.dir * 100.0 * UNIT;
                    let hit = Dir3::new(-c.dir)
                        .ok()
                        .and_then(|d| spatial.cast_ray(from, d, ENTRY_RAY * UNIT, true, &SpatialQueryFilter::default()));
                    hit.map_or(ENTRY_RAY, |h| h.distance / UNIT + 100.0)
                } else {
                    0.0
                };
                let enter = if v > 0.0 { (c.params.time - reach / v).max(0.0) } else { 0.0 };
                c.state = CanState::Flying { launched: clock.now, enter, incoming: false, detonated: false, last: None };
            }
            "opencanister" => {
                if let CanState::Landed { started: false, opened, .. } = c.state {
                    say(&mut play, "HeadcrabCanister.Open");
                    c.state = CanState::Landed { since: clock.now, started: true, opened };
                }
            }
            _ => {}
        }
    }
}

/// The distance from `p` to the segment `a`-`b`.
pub fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

/// A blast's scale at `rho` units from the impact (maps_entities.md 8).
pub fn blast_scale(rho: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    (((rho - radius) / (0.75 * radius)).powi(2)).min(1.0)
}

/// What a blast of scale `s` does to a player's velocity (units/s).
pub fn player_push(s: f32) -> f32 {
    250.0 + 750.0 * (3.0 * s * s - 2.0 * s * s * s)
}

type Bodies<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Position, &'static mut LinearVelocity, Has<Damageable>),
    (With<RigidBody>, Without<Intent>),
>;
type Players<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform, &'static mut Velocity, &'static Health, Option<&'static mut MovementState>), With<Intent>>;

#[allow(clippy::too_many_arguments)]
fn canister_flight(
    mut cans: Query<(Entity, &mut Canister)>,
    mut bodies: Bodies,
    mut players: Players,
    clock: Res<SimClock>,
    mut play: MessageWriter<PlaySound>,
    mut outputs: MessageWriter<FireKeyedOutput>,
    mut damage: MessageWriter<Damage>,
    mut hurt: MessageWriter<MapDamage>,
    mut shakes: MessageWriter<MapShake>,
    mut commands: Commands,
) {
    for (entity, mut c) in &mut cans {
        let c = &mut *c;
        match c.state.clone() {
            CanState::Flying { launched, enter, mut incoming, mut detonated, last } => {
                if clock.tick % THINK_TICKS != 0 {
                    continue;
                }
                let t = (clock.now - launched) as f32;
                if t < enter {
                    continue;
                }
                let total = c.params.time;
                let here = c.position(t);
                let dist = here.distance(c.impact) / UNIT;
                if !incoming && dist <= INCOMING_DISTANCE && !c.params.has(SF_NO_IMPACT_SOUND) {
                    incoming = true;
                    say(&mut play, INCOMING);
                }
                if let Some(from) = last {
                    sweep(c, from, here, &mut bodies, &mut players, &mut damage, &mut hurt);
                }
                let landing = t >= total;
                if (landing || dist <= SWEEP_RADIUS) && !detonated {
                    detonated = true;
                    detonate(c, &mut bodies, &mut players, &mut play, &mut outputs, &mut damage, &mut hurt, &mut shakes);
                }
                if landing {
                    if c.params.has(SF_REMOVE_ON_IMPACT) {
                        c.state = CanState::Removed(clock.now + REMOVE_AFTER);
                    } else {
                        say(&mut play, "HeadcrabCanister.AfterLanding");
                        let started = !c.params.has(SF_WAIT_OPEN);
                        c.state = CanState::Landed { since: clock.now, started, opened: false };
                        if started {
                            say(&mut play, "HeadcrabCanister.Open");
                        }
                    }
                } else {
                    c.state = CanState::Flying { launched, enter, incoming, detonated, last: Some(here) };
                }
            }
            CanState::Landed { since, started: true, opened: false } if clock.now - since >= OPEN_TIME => {
                outputs.write(FireKeyedOutput { key: c.key, output: "OnOpened".into(), activator: None });
                c.state = CanState::Landed { since, started: true, opened: true };
            }
            CanState::Removed(at) if clock.now >= at => {
                commands.entity(entity).despawn();
            }
            _ => {}
        }
    }
}

/// What the canister's path from `from` to `to` hits: a shove along its
/// flight and the full damage.
fn sweep(c: &Canister, from: Vec3, to: Vec3, bodies: &mut Bodies, players: &mut Players, damage: &mut MessageWriter<Damage>, hurt: &mut MessageWriter<MapDamage>) {
    let reach = SWEEP_RADIUS * UNIT;
    for (e, pos, mut v, damageable) in bodies.iter_mut() {
        if segment_distance(pos.0, from, to) > reach {
            continue;
        }
        v.0 += c.dir * PUSH * UNIT;
        if damageable && c.params.damage != 0.0 {
            damage.write(blast(e, c.params.damage, pos.0, c.dir));
        }
    }
    for (e, at, _, health, _) in players.iter() {
        let centre = at.translation + Vec3::Y * 0.9;
        if health.current > 0.0 && c.params.damage != 0.0 && segment_distance(centre, from, to) <= reach {
            hurt.write(MapDamage { damage: blast(e, c.params.damage, centre, c.dir), armour: true });
        }
    }
}

fn blast(target: Entity, amount: f32, point: Vec3, dir: Vec3) -> Damage {
    Damage {
        target,
        attacker: None,
        amount: amount / 100.0,
        point,
        dir,
        hitgroup: crate::core::Hitgroup::default(),
        kind: DamageKind::Blast,
        weapon: None,
        force: Vec3::ZERO,
    }
}

#[allow(clippy::too_many_arguments)]
fn detonate(
    c: &Canister,
    bodies: &mut Bodies,
    players: &mut Players,
    play: &mut MessageWriter<PlaySound>,
    outputs: &mut MessageWriter<FireKeyedOutput>,
    damage: &mut MessageWriter<Damage>,
    hurt: &mut MessageWriter<MapDamage>,
    shakes: &mut MessageWriter<MapShake>,
) {
    outputs.write(FireKeyedOutput { key: c.key, output: "OnImpacted".into(), activator: None });
    if !c.params.has(SF_NO_IMPACT_SOUND) {
        say(play, "HeadcrabCanister.Explosion");
    }
    if c.params.has(SF_REMOVE_ON_IMPACT) {
        return;
    }
    let radius = c.params.radius;
    let at = c.impact;
    for (e, pos, mut v, damageable) in bodies.iter_mut() {
        let rho = pos.0.distance(at) / UNIT;
        if rho > radius {
            continue;
        }
        let s = blast_scale(rho, radius);
        let away = (pos.0 - at).normalize_or_zero();
        v.0 += away * PUSH * s * UNIT;
        if damageable && c.params.damage != 0.0 {
            damage.write(blast(e, c.params.damage * s, pos.0, away));
        }
    }
    for (e, tr, mut vel, health, state) in players.iter_mut() {
        let centre = tr.translation + Vec3::Y * 0.9;
        let rho = centre.distance(at) / UNIT;
        if health.current <= 0.0 || rho > radius {
            continue;
        }
        let s = blast_scale(rho, radius);
        let mut away = (centre - at).normalize_or_zero();
        // Raised to at least 0.1 up, then renormalized.
        away.y = away.y.max(0.1);
        let away = away.normalize_or_zero();
        if c.params.damage != 0.0 {
            hurt.write(MapDamage { damage: blast(e, c.params.damage * s, centre, away), armour: true });
        }
        vel.0 += away * player_push(s) * UNIT;
        if let Some(mut st) = state {
            st.on_ground = false;
        }
    }
    if !c.params.has(SF_NO_SHAKE) {
        shakes.write(MapShake {
            at,
            amplitude: SHAKE_AMPLITUDE * UNIT,
            frequency: 150.0,
            duration: 1.0,
            radius: SHAKE_RADIUS * UNIT,
        });
    }
}

/// Bullet impacts and blasts into the AI sound list (maps_entities.md 16).
fn ai_sounds(mut shots: MessageReader<WeaponEvent>, mut blasts: MessageReader<Explosion>, mut out: MessageWriter<AiSound>) {
    for e in shots.read() {
        if e.replay {
            continue;
        }
        if let WeaponEventKind::Shot { to, hit, normal, .. } = &e.kind
            && (hit.is_some() || normal.is_some())
        {
            out.write(AiSound { at: *to, volume: 200.0 * UNIT, kind: AiSoundKind::BulletImpact, life: 0.5 });
        }
    }
    for b in blasts.read() {
        let units = b.radius / UNIT;
        out.write(AiSound {
            at: b.origin,
            volume: (1.5 * units).max(128.0) * UNIT,
            kind: AiSoundKind::Explosion,
            life: 0.25,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_blast_is_full_inside_a_quarter_and_gone_at_the_radius() {
        assert_eq!(blast_scale(0.0, 350.0), 1.0);
        assert_eq!(blast_scale(87.5, 350.0), 1.0);
        assert_eq!(blast_scale(350.0, 350.0), 0.0);
        let half = blast_scale(218.75, 350.0);
        assert!((half - 0.25).abs() < 1e-5, "{half}");
    }

    #[test]
    fn the_player_push_runs_from_250_to_1000() {
        assert_eq!(player_push(0.0), 250.0);
        assert_eq!(player_push(1.0), 1000.0);
    }

    #[test]
    fn the_path_ends_at_the_impact_after_the_flight_time() {
        let c = Canister {
            key: 1,
            params: CanisterParams { speed: 3000.0, time: 4.0, damage: 150.0, radius: 350.0, flags: 0 },
            impact: Vec3::new(1.0, 2.0, 3.0),
            dir: Vec3::new(0.0, -1.0, 0.0),
            state: CanState::Idle,
            fired: false,
        };
        assert!(c.position(4.0).distance(c.impact) < 1e-5);
        assert!((c.position(3.0).y - (2.0 + 3000.0 * UNIT)).abs() < 1e-4, "1 s out: {:?}", c.position(3.0));
        assert!(c.position(9.0).distance(c.impact) < 1e-5, "clamped");
    }
}
