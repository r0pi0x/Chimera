//! Sensors that watch the game rather than the map (specs/hl2dm/
//! maps_entities.md 10 and 16): `point_angularvelocitysensor` (dm_lockdown's
//! fan) and `env_microphone` (dm_steamlab's gas trap).
//!
//! Covers: point_angularvelocitysensor: the target's angular speed (the
//! host reports it: `LogicWorld::set_prop_spin`) compared with `threshold`
//! each tick, firing `OnGreaterThan`, `OnLessThan`, `OnEqualTo` (and the "or
//! equal" outputs) `fireinterval` after the comparison changes, the
//! `AngularVelocity` output, `Test` and `TestWithInterval`; env_microphone:
//! every 0.1 s the AI sound list (`LogicWorld::add_ai_sound`: games write
//! `map::entities::AiSound` for bullet impacts and blasts) heard by mask,
//! sensitivity and range, `OnHeardSound`, the smoothed `SoundLevel`, `Enable`
//! and `Disable`.
//!
//! Known shortcuts (docs/tech-debt.md): the sensor reads a prop body's spin
//! only (a non-physics target's local angular velocity is not measured), and
//! doesn't tell a stuck object from a slow one; the microphone's speaker
//! routing (`SpeakerName`, `ListenFilter`, `SetSpeakerName`) is not built
//! (no stock map uses it) and every AI sound counts, whoever made it. Why
//! it's OK for now: the stock maps' fan and gas trap need neither.

use bevy::prelude::*;

use super::classes::Class;
use super::community::Extra;
use super::value::Value;
use super::world::{EntId, LogicWorld, Who};
use crate::map::entities::AiSoundKind;

/// `fireinterval`'s default (s).
pub const DEFAULT_FIRE_INTERVAL: f32 = 0.2;
/// The microphone thinks this often (s).
pub const MIC_THINK: f32 = 0.1;
/// The microphone's `Sensitivity` is capped at this.
pub const MAX_SENSITIVITY: f32 = 10.0;
/// A level this close to the loudest sound's is taken as exactly it.
pub const LEVEL_SNAP: f32 = 0.005;
/// Microphone spawnflags: sound kinds added to the mask.
pub const SF_MIC_COMBAT: u32 = 1;
pub const SF_MIC_WORLD: u32 = 2;
pub const SF_MIC_PLAYER: u32 = 4;
pub const SF_MIC_BULLET: u32 = 8;
pub const SF_MIC_EXPLOSION: u32 = 32;
/// Microphone: start disabled (keyvalue `StartDisabled`).

/// A sound of the AI sound list (entity space, units, seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AiSoundEntry {
    pub at: Vec3,
    pub volume: f32,
    pub kind: AiSoundKind,
    /// Heard until this time (s).
    pub until: f32,
}

/// The mask bit of a sound kind.
pub fn kind_bit(kind: AiSoundKind) -> u32 {
    match kind {
        AiSoundKind::Combat => SF_MIC_COMBAT,
        AiSoundKind::World => SF_MIC_WORLD,
        AiSoundKind::Player => SF_MIC_PLAYER,
        AiSoundKind::BulletImpact => SF_MIC_BULLET,
        AiSoundKind::Explosion => SF_MIC_EXPLOSION,
    }
}

/// point_angularvelocitysensor's state.
#[derive(Clone, Debug)]
pub struct AngularSensor {
    pub target: String,
    pub threshold: f32,
    pub interval: f32,
    /// Measure along the direction to `axis` (entity space) instead of
    /// the magnitude.
    pub axis: Option<Vec3>,
    /// The comparison last seen (-1 less, 0 equal, 1 greater).
    pub current: Option<i32>,
    /// The comparison last fired.
    pub fired: Option<i32>,
    /// When a pending fire is due (s).
    pub due: Option<f32>,
    /// The measured value last reported through `AngularVelocity`.
    pub value: f32,
}

/// env_microphone's state.
#[derive(Clone, Debug)]
pub struct Microphone {
    pub enabled: bool,
    pub mask: u32,
    pub sensitivity: f32,
    pub smooth: f32,
    pub max_range: f32,
    pub target: String,
    pub level: f32,
}

pub(super) fn spawn(w: &LogicWorld, id: EntId, class: &str) -> Option<Class> {
    let e = w.get(id)?;
    let x = match class {
        "point_angularvelocitysensor" => {
            let helper = e.kv_i("usehelper") != 0;
            Extra::Sensor(AngularSensor {
                target: e.kv("target").unwrap_or("").trim().to_string(),
                threshold: e.kv_f("threshold").abs(),
                interval: e.kv("fireinterval").map_or(DEFAULT_FIRE_INTERVAL, super::value::atof),
                axis: helper.then(|| e.vector_kv("axis")),
                current: None,
                fired: None,
                due: None,
                value: f32::NAN,
            })
        }
        "env_microphone" => {
            let s = e.kv("Sensitivity").map_or(1.0, super::value::atof);
            Extra::Mic(Microphone {
                enabled: e.kv_i("StartDisabled") == 0,
                mask: (e.kv_i("SoundMask") as u32)
                    | (e.spawnflags & (SF_MIC_COMBAT | SF_MIC_WORLD | SF_MIC_PLAYER | SF_MIC_BULLET | SF_MIC_EXPLOSION)),
                sensitivity: s.clamp(0.0, MAX_SENSITIVITY),
                smooth: e.kv_f("SmoothFactor").clamp(0.0, 0.9),
                max_range: e.kv_f("MaxRange").max(0.0),
                target: e.kv("target").unwrap_or("").trim().to_string(),
                level: 0.0,
            })
        }
        _ => return None,
    };
    Some(Class::Extra(Box::new(x)))
}

fn x_of(w: &LogicWorld, id: EntId) -> Option<Extra> {
    match w.get(id).map(|e| &e.class) {
        Some(Class::Extra(x)) => Some((**x).clone()),
        _ => None,
    }
}

fn put(w: &mut LogicWorld, id: EntId, x: Extra) {
    if let Some(Class::Extra(slot)) = w.get_mut(id).map(|e| &mut e.class) {
        **slot = x;
    }
}

pub(super) fn activate(w: &mut LogicWorld, id: EntId) {
    match x_of(w, id) {
        Some(Extra::Sensor(s)) if w.find(&s.target).is_some() => w.think_in(id, w.dt),
        Some(Extra::Mic(m)) if m.enabled => w.think_in(id, MIC_THINK),
        _ => {}
    }
}

/// Seconds since the map's first tick.
fn now(w: &LogicWorld) -> f32 {
    w.tick as f32 * w.dt
}

impl LogicWorld {
    /// A sound enters the AI sound list (entity space, units), heard for
    /// `life` seconds.
    pub fn add_ai_sound(&mut self, at: Vec3, volume: f32, kind: AiSoundKind, life: f32) {
        let until = now(self) + life;
        self.ai_sounds.push(AiSoundEntry { at, volume, kind, until });
    }

    /// Drop the sounds that ended.
    pub(super) fn expire_ai_sounds(&mut self) {
        let t = now(self);
        self.ai_sounds.retain(|s| s.until >= t);
    }
}

/// What the sensor measures on its target now.
fn measure(w: &LogicWorld, id: EntId, s: &AngularSensor) -> Option<f32> {
    let target = w.find(&s.target)?;
    let spin = w.prop(target)?.spin;
    let origin = w.get(id)?.origin;
    Some(match s.axis {
        Some(axis) => {
            let d = (axis - origin).normalize_or_zero();
            spin.dot(d).abs()
        }
        None => spin.length(),
    })
}

fn compare(value: f32, threshold: f32) -> i32 {
    if value > threshold {
        1
    } else if value < threshold {
        -1
    } else {
        0
    }
}

fn fire_cmp(w: &mut LogicWorld, id: EntId, cmp: i32, activator: Option<Who>) {
    let me = Some(Who::Ent(id));
    let _ = activator;
    match cmp {
        -1 => {
            w.fire_output(id, "OnLessThan", me, Value::Void);
            w.fire_output(id, "OnLessThanOrEqualTo", me, Value::Void);
        }
        1 => {
            w.fire_output(id, "OnGreaterThan", me, Value::Void);
            w.fire_output(id, "OnGreaterThanOrEqualTo", me, Value::Void);
        }
        _ => {
            w.fire_output(id, "OnEqualTo", me, Value::Void);
            w.fire_output(id, "OnLessThanOrEqualTo", me, Value::Void);
            w.fire_output(id, "OnGreaterThanOrEqualTo", me, Value::Void);
        }
    }
}

pub(super) fn think(w: &mut LogicWorld, id: EntId) -> bool {
    match x_of(w, id) {
        Some(Extra::Sensor(s)) => {
            sensor_think(w, id, s);
            true
        }
        Some(Extra::Mic(m)) if m.enabled => {
            mic_think(w, id, m);
            true
        }
        _ => false,
    }
}

fn sensor_think(w: &mut LogicWorld, id: EntId, mut s: AngularSensor) {
    w.think_in(id, w.dt);
    let Some(value) = measure(w, id, &s) else { return };
    let t = now(w);
    if s.value.is_nan() || (value - s.value).abs() > 1e-4 {
        w.fire_output(id, "AngularVelocity", Some(Who::Ent(id)), Value::Float(value));
        s.value = value;
    }
    let cmp = compare(value, s.threshold);
    match s.current {
        Some(c) if c == cmp => {
            if let Some(due) = s.due
                && t >= due
            {
                s.due = None;
                s.fired = Some(cmp);
                fire_cmp(w, id, cmp, None);
            }
        }
        previous => {
            if s.fired == Some(cmp) {
                s.due = None;
            } else if previous.is_some() {
                s.due = Some(t + s.interval);
            }
            s.current = Some(cmp);
        }
    }
    put(w, id, Extra::Sensor(s));
}

fn mic_think(w: &mut LogicWorld, id: EntId, mut m: Microphone) {
    w.think_in(id, MIC_THINK);
    let t = now(w);
    let Some(e) = w.get(id) else { return };
    let from = if m.target.is_empty() {
        e.origin
    } else {
        w.find(&m.target).and_then(|t| w.get(t)).map_or(e.origin, |t| t.origin)
    };
    let mut loudest: Option<f32> = None;
    for s in &w.ai_sounds {
        if s.until < t || s.kind_mask() & m.mask == 0 {
            continue;
        }
        let d = s.at.distance(from);
        let reach = s.volume * m.sensitivity;
        let level = if d == 0.0 {
            1.0
        } else if (m.max_range == 0.0 || d <= m.max_range) && d <= reach {
            1.0 - d / reach
        } else {
            continue;
        };
        loudest = Some(loudest.map_or(level, |l: f32| l.max(level)));
    }
    let max = loudest.unwrap_or(0.0);
    let before = m.level;
    let new = max * (1.0 - m.smooth) + m.level * m.smooth;
    m.level = if (new - max).abs() < LEVEL_SNAP { max } else { new };
    let level = m.level;
    put(w, id, Extra::Mic(m));
    if loudest.is_some() {
        w.fire_output(id, "OnHeardSound", Some(Who::Ent(id)), Value::Void);
    }
    if level != before {
        w.fire_output(id, "SoundLevel", Some(Who::Ent(id)), Value::Float(level));
    }
}

impl AiSoundEntry {
    fn kind_mask(&self) -> u32 {
        kind_bit(self.kind)
    }
}

pub(super) fn input(w: &mut LogicWorld, id: EntId, input: &str, value: &Value, activator: Option<Who>) -> bool {
    let Some(x) = x_of(w, id) else { return false };
    match (x, input) {
        (Extra::Sensor(s), "test") => {
            if let Some(v) = measure(w, id, &s) {
                fire_cmp(w, id, compare(v, s.threshold), activator);
            }
        }
        (Extra::Sensor(mut s), "testwithinterval") => {
            if let Some(v) = measure(w, id, &s) {
                s.current = Some(compare(v, s.threshold));
                s.fired = None;
                s.due = Some(now(w) + s.interval);
                put(w, id, Extra::Sensor(s));
            }
        }
        (Extra::Mic(mut m), "enable" | "disable") => {
            let on = input == "enable";
            m.enabled = on;
            put(w, id, Extra::Mic(m));
            if on {
                w.think_in(id, MIC_THINK);
            } else if let Some(e) = w.get_mut(id) {
                e.next_think = None;
            }
        }
        (Extra::Mic(_), "setspeakername") => {
            let _ = w.need_str(value, input);
        }
        _ => return false,
    }
    true
}
