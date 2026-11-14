//! The combine camera (`npc_combine_camera`, specs/hl2dm/maps_entities.md
//! 14; dm_underpass's ceiling camera): searching, it sweeps. Every think
//! (0.05 s) its aim goal is its own yaw + 45° · sin(2 · time) at pitch 15°,
//! and its head's pose parameters turn toward it by at most 6°
//! (`CameraSweep::think`; drawn on its prop, held at its first
//! sequence's first frame, `map::PropCycle`); `NPC_CombineCamera.Move` plays at
//! most once a second while it turns, `NPC_CombineCamera.Ping` once a
//! second; its eye glows green (`sprites/glow1`). Each machine runs the
//! sweep from its own clock (a network client too: it is a look and its
//! sounds, nothing anyone else needs), so nothing goes over the network.
//! Flag 128 (start inactive) leaves it still and silent.
//!
//! Known shortcuts: no lock-on (an enemy within the outer radius, the
//! amber eye, `Active`/`Angry`, the white flashes and clicks): whether
//! the camera ever takes a player for an enemy is open (Open questions 2,
//! probe P8; the expectation is never). No `Enable`/`Disable`/`Toggle`
//! inputs, no damage (the stock camera is unnamed and indestructible).
//! The glow's size and brightness aren't in the spec (ours: `GLOW_HALF`,
//! full alpha), and it sits at the `eyes` attachment's place in the
//! reference pose turned with the head's yaw and pitch about the model's
//! origin, not at the posed bone's. "time" is the machine's simulation
//! clock (`SimClock`), not the map's start. The spec names neither the
//! model's pose parameters nor the sequence they blend: ours are its
//! first parameter whose name holds "yaw" and the first that holds
//! "pitch", on its first sequence (the model wasn't read to check).

use bevy::prelude::*;

use crate::{
    core::SimClock,
    map::{
        MapEntities, PlaySound,
        entities::entity_to_engine,
        particles::{Motion, Particle, ParticleGroup, ParticleMaterials, ParticleSet, Particles},
        PropCycle, PropEntity, PropRig,
    },
};

const U: f32 = 0.0254;
/// A sprite drawn for one frame.
const ONE_FRAME: f32 = 0.0005;

/// The camera's class.
pub const CLASS: &str = "npc_combine_camera";
/// Its model (`source_engine`'s props place it).
pub const MODEL: &str = "models/combine_camera/combine_camera.mdl";
/// Seconds between thinks (3 ticks).
pub const THINK: f64 = 0.05;
/// The sweep: its own yaw + `SWEEP` · sin(`SWEEP_RATE` · time), degrees.
pub const SWEEP: f32 = 45.0;
pub const SWEEP_RATE: f64 = 2.0;
/// The searching aim's pitch, degrees.
pub const SEARCH_PITCH: f32 = 15.0;
/// Most a pose parameter turns in a think, degrees.
pub const TURN: f32 = 6.0;
pub const MOVE_SOUND: &str = "NPC_CombineCamera.Move";
pub const PING_SOUND: &str = "NPC_CombineCamera.Ping";
/// Seconds between pings, and at least between move sounds.
pub const SOUND_GAP: f64 = 1.0;
/// The eye's glow: its sprite and colour.
pub const GLOW: &str = "sprites/glow1";
/// The materials it draws (additive: `effects::ADDITIVE_SPRITES`).
pub const MATERIALS: &[&str] = &[GLOW];
pub const GLOW_COLOR: Vec3 = Vec3::new(0.0, 1.0, 0.0);
/// The glow's half size, units (ours: the spec gives none).
pub const GLOW_HALF: f32 = 4.0;
/// Its pose parameters: the first whose names hold these.
pub const YAW_PARAM: &str = "yaw";
pub const PITCH_PARAM: &str = "pitch";
/// Flag 128: start inactive.
const SF_INACTIVE: u32 = 128;
/// The `eyes` attachment's keyvalue (`source_engine`'s `ATTACHMENT_KEY`).
const EYES_KEY: &str = "$attachment eyes";

/// The aim goal's yaw of a camera of own yaw `own` at `time`, degrees.
pub fn goal_yaw(own: f32, time: f64) -> f32 {
    own + SWEEP * (SWEEP_RATE * time).sin() as f32
}

/// `from` turned toward `to` by at most `step`.
fn approach(from: f32, to: f32, step: f32) -> f32 {
    from + (to - from).clamp(-step, step)
}

/// One camera's sweep.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraSweep {
    /// Its map entity.
    pub entity: usize,
    /// Its own yaw, degrees.
    pub own_yaw: f32,
    /// Where it is (engine space), and its eye in the model's reference
    /// pose (entity space, units, from the origin; unturned).
    pub at: Vec3,
    pub eye: Option<Vec3>,
    /// Its head's pose parameters, degrees (yaw from its own).
    pub yaw: f32,
    pub pitch: f32,
    pub active: bool,
    pub next_think: Option<f64>,
    pub last_move: f64,
    pub last_ping: f64,
}

impl CameraSweep {
    /// A think at `now`: the pose turned toward the goal; the sounds it
    /// plays.
    pub fn think(&mut self, now: f64) -> Vec<&'static str> {
        let mut out = Vec::new();
        let goal = goal_yaw(self.own_yaw, now) - self.own_yaw;
        let (yaw, pitch) = (approach(self.yaw, goal, TURN), approach(self.pitch, SEARCH_PITCH, TURN));
        let turning = yaw != self.yaw || pitch != self.pitch;
        (self.yaw, self.pitch) = (yaw, pitch);
        if turning && now - self.last_move >= SOUND_GAP - 1e-6 {
            self.last_move = now;
            out.push(MOVE_SOUND);
        }
        if now - self.last_ping >= SOUND_GAP - 1e-6 {
            self.last_ping = now;
            out.push(PING_SOUND);
        }
        out
    }

    /// Where its eye's glow is (engine space).
    pub fn glow_at(&self, scale: f32) -> Vec3 {
        let Some(eye) = self.eye else { return self.at };
        // Entity space: z up; the head turns about z by its yaw and pitches
        // about its own right axis.
        let turn = Quat::from_rotation_z((self.own_yaw + self.yaw).to_radians())
            * Quat::from_rotation_y(self.pitch.to_radians());
        self.at + entity_to_engine(turn * eye, scale)
    }
}

/// The map's cameras.
#[derive(Resource, Default, Debug)]
pub struct CombineCameras(pub Vec<CameraSweep>);

/// The map's cameras found when its entities change.
fn find(entities: Option<Res<MapEntities>>, mut cameras: ResMut<CombineCameras>) {
    let Some(entities) = entities else {
        if !cameras.0.is_empty() {
            cameras.0.clear();
        }
        return;
    };
    if !entities.is_changed() {
        return;
    }
    cameras.0 = entities
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.classname().eq_ignore_ascii_case(CLASS))
        .map(|(entity, e)| {
            let flags: u32 = e.get("spawnflags").and_then(|v| v.trim().parse().ok()).unwrap_or(0);
            CameraSweep {
                entity,
                own_yaw: e.angles().y,
                at: entity_to_engine(e.origin(), entities.scale),
                eye: e.get(EYES_KEY).map(crate::map::entities::parse_vector),
                yaw: 0.0,
                pitch: 0.0,
                active: flags & SF_INACTIVE == 0,
                next_think: None,
                last_move: f64::NEG_INFINITY,
                last_ping: f64::NEG_INFINITY,
            }
        })
        .collect();
}

/// Each camera's thinks due by now, and their sounds.
fn sweep(clock: Res<SimClock>, mut cameras: ResMut<CombineCameras>, mut play: MessageWriter<PlaySound>) {
    let now = clock.now;
    if cameras.0.is_empty() {
        return;
    }
    for c in cameras.0.iter_mut().filter(|c| c.active) {
        let mut t = *c.next_think.get_or_insert(now);
        while t <= now + 1e-6 {
            for s in c.think(t) {
                play.write(PlaySound::at(s, c.at));
            }
            t += THINK;
        }
        c.next_think = Some(t);
    }
}

/// The cameras' heads posed by their pose parameters (where the props are
/// drawn).
fn pose(
    cameras: Res<CombineCameras>,
    mut props: Query<(Entity, &PropEntity, &mut PropRig, Has<PropCycle>)>,
    mut commands: Commands,
) {
    if cameras.0.is_empty() {
        return;
    }
    for (node, e, mut rig, held) in &mut props {
        let Some(c) = cameras.0.iter().find(|c| c.entity == e.0) else { continue };
        if !held {
            commands.entity(node).insert(PropCycle(0.0));
        }
        let rig = &mut *rig;
        for (part, value) in [(YAW_PARAM, c.yaw), (PITCH_PARAM, c.pitch)] {
            let set = &rig.animator.set;
            let Some(i) = set.params.iter().position(|p| p.name.to_ascii_lowercase().contains(part)) else { continue };
            let v = set.params[i].encode(value);
            if let Some(slot) = rig.animator.params.get_mut(i) {
                *slot = v;
            }
        }
    }
}

/// The eyes' glows, one frame each.
fn glow(
    cameras: Res<CombineCameras>,
    entities: Option<Res<MapEntities>>,
    materials: Option<Res<ParticleMaterials>>,
    mut particles: ResMut<Particles>,
) {
    let (Some(entities), Some(m)) = (entities, materials) else { return };
    let Some(mat) = m.0.find(GLOW) else { return };
    let mut g = ParticleGroup::new(Motion::default());
    g.skip_first = false;
    g.capped = false;
    for c in cameras.0.iter().filter(|c| c.active) {
        g.particles.push(Particle {
            color: GLOW_COLOR,
            ..Particle::new(c.glow_at(entities.scale), ONE_FRAME, mat, GLOW_HALF * U)
        });
    }
    if !g.particles.is_empty() {
        particles.add(g);
    }
}

pub struct CombineCameraPlugin;

impl Plugin for CombineCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CombineCameras>()
            .init_resource::<Particles>()
            .add_message::<PlaySound>()
            .add_systems(FixedUpdate, (find, sweep).chain())
            .add_systems(
                Update,
                (
                    pose.before(crate::map::animate_props),
                    glow.after(ParticleSet::Step).before(ParticleSet::Draw),
                ),
            );
    }
}
