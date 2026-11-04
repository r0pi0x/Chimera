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
//!   `OnLaunched`, `OnImpacted`, `OnOpened`. Server-side; it is drawn on
//!   every machine as shown items, their models placed as props are by
//!   the entity's angles (`model_turn`; its long axis, the model's +X,
//!   along its forward axis, so dm_runoff's stand 71 deg steep):
//!   `SKY_CANISTER_MODEL` in the 3D skybox until it enters the world (the
//!   flight model shrunk by the sky's scale, on the image of its line
//!   raised by `StartingHeight` at the launch: `Canister::sky_position`;
//!   without a sky camera it flies all its time in the world),
//!   `CANISTER_MODEL` in the world along its
//!   line, spinning 90 deg/s about its long axis, then `landed_key`: the
//!   landed model at `OPEN_FRAMES` frames of its `open` sequence through
//!   the opening, smoking until `SmokeLifetime` or `StopSmoke`;
//!   `canister_models` loads them on maps with a canister), its impact's
//!   effects told as `CanisterBlast` (unless flag 524288); the trail,
//!   smoke and explosion are `set_piece_fx`'s.
//! - **`game_ragdoll_manager`** (`RagdollLimit`): `MaxRagdollCount`
//!   (-1: 8) and the inputs `SetMaxRagdollCount`, `SetMaxRagdollCountDX8`.
//!   HL2:DM's player bodies aren't in the list it limits and no stock map
//!   has other ragdolls, so nothing reads the number yet.
//! - **AI sounds** (`ai_sounds`): each bullet impact (volume 200 units, 0.5
//!   s) and blast (the larger of 128 and 1.5 of its radius, 0.25 s) is told
//!   to the logic (`map::entities::AiSound`).
//!
//! Known shortcuts: the spec doesn't say how large the model is drawn in
//! the skybox: it is shrunk by the sky's scale so its image is its own
//! size, as in the world; the opening is four still
//! frames, not the sequence played; the landed model isn't solid;
//! `LaunchPositionName`,
//! "start impacted" and the skybox-only shots are not built (no stock
//! canister uses them); the sweep and the blast test the distance of a
//! body's centre, not its bounds, and the sweep is the spec's cube as a
//! sphere of its half-diagonal; the shake reaches players in the air too;
//! headcrabs are never released (`HeadcrabCount` 0 in the stock maps); the
//! early detonation still lets the canister fly on without effect.

use avian3d::prelude::{LinearVelocity, Position, RigidBody, SpatialQuery, SpatialQueryFilter};
use chimera_sim::weapon::projectile::ShownBody;
use bevy::prelude::*;

use crate::{
    core::{Damage, DamageKind, Damageable, Explosion, Health, Intent, MapDamage, MovementState, SimClock, Velocity},
    map::{
        PlaySound,
        entities::{
            AiSound, AiSoundKind, FireKeyedOutput, GameEntityClasses, MapEntities, MapEntityInput, MapEntityRemoved, MapEntitySpawned,
            entity_rotation, entity_to_engine, rotation_to_engine,
        },
        logic_view::MapShake,
        loose::{LooseAssets, ShownItem},
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

/// The canister's models as shown items (`ShownItem` keys, drawn on every
/// machine: the network's `NetItem`): in flight `headcrabcannister01a`,
/// landed `headcrabcannister01b` posed at `OPEN_FRAMES` frames of its
/// `open` sequence, smoking (`open`) or not (`spent`).
pub const CANISTER_MODEL: &str = "hl2dm:headcrabcanister";
/// The flight model shrunk by the map's 3D skybox scale, for its flight in
/// the skybox before it enters the world (its image there is its own size).
pub const SKY_CANISTER_MODEL: &str = "hl2dm:headcrabcanister_sky";
pub const FLIGHT_MODEL_FILE: &str = "models/props_combine/headcrabcannister01a.mdl";
pub const LANDED_MODEL_FILE: &str = "models/props_combine/headcrabcannister01b.mdl";
/// Frames of the `open` sequence the landed model is shown at (cycle
/// k / (n − 1)).
pub const OPEN_FRAMES: usize = 4;
/// The landed canister's key at frame `k`, smoking or not.
pub fn landed_key(k: usize, smoking: bool) -> String {
    format!("hl2dm:headcrabcanister_{}{k}", if smoking { "open" } else { "spent" })
}
/// The spin about its long axis in flight (deg/s).
pub const SPIN: f32 = 90.0;
/// Its smoke after landing, until `SmokeLifetime` (s) runs out or
/// `StopSmoke`.
const SF_NO_SMOKE: u32 = 65_536;
const SF_NO_IMPACT_EFFECTS: u32 = 524_288;

/// A canister's impact effects (explosion, unless flag 524288), for
/// every machine to draw (`set_piece_fx`).
#[derive(Message, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CanisterBlast {
    pub at: Vec3,
}

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
    /// `SmokeLifetime` (s).
    pub smoke: f32,
    /// `StartingHeight` (units): how far above its line the skybox arc
    /// starts.
    pub height: f32,
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
            smoke: f("SmokeLifetime"),
            height: f("StartingHeight"),
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
    /// Its orientation (entity space, its angles).
    pub turn: Quat,
    /// The shown model (`ShownItem` entity) and its key.
    pub shown: Option<(Entity, String)>,
    /// Smoke until then (s).
    pub smoke_until: f64,
}

impl Canister {
    /// Its position `t` seconds after launch (m), on the straight flight
    /// to the impact.
    pub fn position(&self, t: f32) -> Vec3 {
        let t = t.min(self.params.time);
        self.impact - self.dir * (self.params.time - t) * self.params.speed * UNIT
    }

    /// Its place in the 3D skybox `t` seconds after launch, before it
    /// enters the world at `enter` (m, engine space): the image of its line
    /// back from the entry point, raised by `StartingHeight` at the launch
    /// and coming down to the line at the entry with the line's slope
    /// (horizontal speed and descent as in the world, at the sky's
    /// `scale`), placed by the sky camera at `sky`.
    pub fn sky_position(&self, t: f32, enter: f32, sky: Vec3, scale: f32) -> Vec3 {
        let rise = if enter > 0.0 { (1.0 - t / enter).max(0.0).powi(2) } else { 0.0 };
        let image = self.position(t) + Vec3::Y * self.params.height * UNIT * rise;
        sky + image / scale
    }
}

/// The map's 3D skybox camera: its origin (engine space, m) and scale;
/// None without one.
pub fn sky_camera(entities: Option<&MapEntities>) -> Option<(Vec3, f32)> {
    let m = entities?;
    let e = m.entities.iter().find(|e| e.classname().eq_ignore_ascii_case("sky_camera"))?;
    let scale = e.get("scale").and_then(|s| s.trim().parse::<f32>().ok()).filter(|s| *s > 0.0).unwrap_or(16.0);
    Some((entity_to_engine(e.origin(), m.scale), scale))
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
            .add_message::<CanisterBlast>()
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
                despawn_with_shown(&mut commands, e, c);
            }
        }
    }
    for s in spawned.read() {
        match s.class.to_ascii_lowercase().as_str() {
            "env_headcrabcanister" => {
                for (e, c) in &cans {
                    if c.key == s.key {
                        despawn_with_shown(&mut commands, e, c);
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
                        turn: entity_rotation(s.angles),
                        shown: None,
                        smoke_until: 0.0,
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

/// The canister's shown models (`CANISTER_MODEL`, `landed_key`): the
/// flight model, and the landed one posed at each of `OPEN_FRAMES` frames
/// of `open` (twice: smoking and not).
pub fn canister_models(
    materials: &mut source_engine::content::material::MaterialLoader,
    sky_scale: Option<f32>,
    warnings: &mut Vec<String>,
) -> Vec<crate::map::MapHeldModel> {
    use source_engine::content::props;
    let mut out = Vec::new();
    match props::load_held(materials, FLIGHT_MODEL_FILE, CANISTER_MODEL, &[]) {
        Ok(h) => {
            // With a 3D skybox, the same shrunk by its scale.
            if let Some(scale) = sky_scale {
                let mut sky = h.clone();
                sky.key = SKY_CANISTER_MODEL.into();
                for m in &mut sky.model.meshes {
                    for p in &mut m.positions {
                        *p = (Vec3::from(*p) / scale).to_array();
                    }
                }
                out.push(sky);
            }
            out.push(h);
        }
        Err(e) => warnings.push(e),
    }
    for k in 0..OPEN_FRAMES {
        let cycle = k as f32 / (OPEN_FRAMES - 1) as f32;
        match props::load_held_posed(materials, LANDED_MODEL_FILE, &landed_key(k, true), &[], Some(("open", cycle))) {
            Ok(h) => {
                let mut spent = h.clone();
                spent.key = landed_key(k, false);
                out.extend([h, spent]);
            }
            Err(e) => warnings.push(e),
        }
    }
    out
}

fn despawn_with_shown(commands: &mut Commands, e: Entity, c: &Canister) {
    if let Some((shown, _)) = &c.shown {
        commands.entity(*shown).try_despawn();
    }
    commands.entity(e).try_despawn();
}

/// Show canister `c` as `key` at `pose` (its model's origin there, its
/// model's axes turned as a prop's by its angles; a shown item is drawn
/// centred on its bounds in the loose frame, `map::loose::LooseAssets`):
/// the shown entity moved, or made again when the key changes (a shown
/// item's model is fixed).
fn show(commands: &mut Commands, c: &mut Canister, key: &str, mut pose: Transform, assets: Option<&LooseAssets>) {
    if let Some(a) = assets.and_then(|a| a.0.get(key)) {
        pose.rotation *= model_turn(a.frame);
        pose.translation -= pose.rotation * a.frame.translation;
    }
    match &c.shown {
        Some((e, k)) if k == key => {
            commands.entity(*e).try_insert(pose);
        }
        _ => {
            if let Some((e, _)) = c.shown.take() {
                commands.entity(e).try_despawn();
            }
            let e = commands
                .spawn((
                    Name::new("env_headcrabcanister model"),
                    ShownItem(key.into()),
                    ShownBody::default(),
                    crate::map::MapPart,
                    pose,
                ))
                .id();
            c.shown = Some((e, key.into()));
        }
    }
}

/// What a shown item's pose is turned by so its model is placed as a
/// prop's (Source axes straight to ours, x z −y, then the entity's
/// rotation): the loose frame (`frame`, `map::loose::asset`) carries the
/// player skeleton's turn too, a quarter turn about up that would lay the
/// canister's long axis (its +X) along the entity's left, where its pitch
/// doesn't tilt it.
pub fn model_turn(frame: Transform) -> Quat {
    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2) * frame.rotation.inverse()
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
    entities: Option<Res<MapEntities>>,
) {
    let sky = sky_camera(entities.as_deref()).is_some();
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
                // Without a 3D skybox it flies all of its time in the world.
                let enter = if v > 0.0 && sky { (c.params.time - reach / v).max(0.0) } else { 0.0 };
                c.state = CanState::Flying { launched: clock.now, enter, incoming: false, detonated: false, last: None };
            }
            "stopsmoke" => c.smoke_until = c.smoke_until.min(clock.now),
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

/// The frame of the `open` sequence (of `OPEN_FRAMES`) `t` seconds into
/// the opening.
pub fn open_frame(t: f64) -> usize {
    ((t / OPEN_TIME).clamp(0.0, 1.0) * (OPEN_FRAMES - 1) as f64).round() as usize
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
    mut blasts: MessageWriter<CanisterBlast>,
    assets: Option<Res<LooseAssets>>,
    entities: Option<Res<MapEntities>>,
    mut commands: Commands,
) {
    let sky = sky_camera(entities.as_deref());
    for (entity, mut c) in &mut cans {
        let c = &mut *c;
        match c.state.clone() {
            CanState::Flying { launched, enter, mut incoming, mut detonated, last } => {
                let t = (clock.now - launched) as f32;
                // Drawn every tick in the world: along its line, spinning
                // about its long axis.
                if t >= enter && t < c.params.time {
                    let spin = Quat::from_rotation_x((SPIN * (t - enter)).to_radians());
                    let pose = Transform::from_translation(c.position(t)).with_rotation(rotation_to_engine(c.turn * spin));
                    show(&mut commands, c, CANISTER_MODEL, pose, assets.as_deref());
                } else if t < enter
                    && let Some((origin, scale)) = sky
                {
                    // Before that, in the 3D skybox (not spinning yet).
                    let pose = Transform::from_translation(c.sky_position(t, enter, origin, scale))
                        .with_rotation(rotation_to_engine(c.turn));
                    show(&mut commands, c, SKY_CANISTER_MODEL, pose, assets.as_deref());
                }
                if clock.tick % THINK_TICKS != 0 {
                    continue;
                }
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
                    if !c.params.has(SF_REMOVE_ON_IMPACT) && !c.params.has(SF_NO_IMPACT_EFFECTS) {
                        blasts.write(CanisterBlast { at: c.impact });
                    }
                }
                if landing {
                    if c.params.has(SF_REMOVE_ON_IMPACT) {
                        c.state = CanState::Removed(clock.now + REMOVE_AFTER);
                        if let Some((e, _)) = c.shown.take() {
                            commands.entity(e).try_despawn();
                        }
                    } else {
                        c.smoke_until = if c.params.has(SF_NO_SMOKE) { clock.now } else { clock.now + c.params.smoke as f64 };
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
            CanState::Landed { since, started, opened } => {
                if started && !opened && clock.now - since >= OPEN_TIME {
                    outputs.write(FireKeyedOutput { key: c.key, output: "OnOpened".into(), activator: None });
                    c.state = CanState::Landed { since, started: true, opened: true };
                }
                // The landed model at its `open` frame, smoking or not.
                let k = if started { open_frame(clock.now - since) } else { 0 };
                let smoking = clock.now < c.smoke_until;
                let pose = Transform::from_translation(c.impact).with_rotation(rotation_to_engine(c.turn));
                show(&mut commands, c, &landed_key(k, smoking), pose, assets.as_deref());
            }
            CanState::Removed(at) if clock.now >= at => {
                despawn_with_shown(&mut commands, entity, c);
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
            params: CanisterParams { speed: 3000.0, time: 4.0, damage: 150.0, radius: 350.0, flags: 0, smoke: 0.0, height: 0.0 },
            impact: Vec3::new(1.0, 2.0, 3.0),
            dir: Vec3::new(0.0, -1.0, 0.0),
            state: CanState::Idle,
            fired: false,
            turn: Quat::IDENTITY,
            shown: None,
            smoke_until: 0.0,
        };
        assert!(c.position(4.0).distance(c.impact) < 1e-5);
        assert!((c.position(3.0).y - (2.0 + 3000.0 * UNIT)).abs() < 1e-4, "1 s out: {:?}", c.position(3.0));
        assert!(c.position(9.0).distance(c.impact) < 1e-5, "clamped");
    }

    /// In the skybox: the image of its line at the sky's scale, raised by
    /// `StartingHeight` at the launch and on the line at the entry.
    #[test]
    fn the_skybox_arc_ends_on_the_entry_point() {
        let mut c = Canister {
            key: 1,
            params: CanisterParams { speed: 3000.0, time: 4.0, damage: 0.0, radius: 0.0, flags: 0, smoke: 0.0, height: 1600.0 },
            impact: Vec3::ZERO,
            dir: Vec3::new(1.0, -1.0, 0.0).normalize(),
            state: CanState::Idle,
            fired: false,
            turn: Quat::IDENTITY,
            shown: None,
            smoke_until: 0.0,
        };
        let sky = Vec3::new(10.0, -50.0, 3.0);
        let at_entry = c.sky_position(3.0, 3.0, sky, 16.0);
        assert!(at_entry.distance(sky + c.position(3.0) / 16.0) < 1e-5);
        let launch = c.sky_position(0.0, 3.0, sky, 16.0);
        let raised = launch - (sky + c.position(0.0) / 16.0);
        assert!((raised - Vec3::Y * 100.0 * UNIT).length() < 1e-5, "{raised}");
        c.params.height = 0.0;
        let mid = c.sky_position(1.5, 3.0, sky, 16.0);
        assert!(mid.distance(sky + c.position(1.5) / 16.0) < 1e-5);
    }

    /// The loose frame's player-skeleton turn is undone: the model's +X
    /// (Source forward) ends up along the entity's forward.
    #[test]
    fn a_shown_canister_is_turned_as_a_prop() {
        let face = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let axes = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        let frame = Transform::from_rotation(face * axes);
        let angles = Vec3::new(-71.0, 30.0, 0.0);
        let entity = rotation_to_engine(entity_rotation(angles));
        let drawn = entity * model_turn(frame) * frame.rotation * Vec3::X;
        let forward = entity_to_engine(entity_rotation(angles) * Vec3::X, 1.0);
        assert!(drawn.distance(forward) < 1e-5, "{drawn} against {forward}");
        assert!(drawn.y > 0.94, "71 deg up: {drawn}");
    }
}
