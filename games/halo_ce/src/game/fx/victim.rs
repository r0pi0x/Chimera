//! What a hit shows its victim (specs/halo_ce/effects.md "Damage effect
//! presentation", low_health.md 5): the damage effect's (`jpt!`) screen
//! flash, temporary camera impulse, permanent camera impulse and camera
//! shake, per local player.
//!
//! - **The numbers** are the tags' own: every damage effect on the donor
//!   map is read at its conversion into a `DamageLook` (`look_of`, kept in
//!   `HaloFx::damage` by tag path); nothing of them is in the repo. Which
//!   tag a hit's effect is: `data/damage.toml`'s `tag`.
//! - **The hit reaches its victim's machine** as `HaloVictim` (the damage
//!   effect by its index in `DamageEffects`, the event's scale and
//!   direction, and whether someone else dealt it): written from the
//!   server's `damage::HaloHurt` for a local player there, and sent to the
//!   client whose character it hit (`HaloVictimNet`: one byte of effect,
//!   one of flags, the scale and the direction; only to that client).
//! - **Each frame** (`present`) the runs started by the hits are evaluated
//!   by the spec's rules (`FlashRun`, `ImpulseRun`, `ShakeRun`: intensity
//!   z + (1 - z) s, durations truncated to ticks, the replacement rules,
//!   the fade, falloff and wobble functions, the shake's two random
//!   directions once a tick from a local generator) into `VictimView`: the
//!   flash's kind, colour and intensity, and the view's rotation (a
//!   rotation vector) and offset (metres), both in the camera's own frame
//!   (x right, y up, -z forward). The presentation (`halo_ce_ui`) draws
//!   them; the permanent impulse turns the local player's aim at once
//!   (`Intent` yaw and pitch).
//!
//! The HUD's damage arrows read `HaloVictim` too, for the effect's
//! "indicators always point down" flag (`ui::hud`).
//!
//! Known shortcuts: the controller rumble is not played (no rumble
//! output); how the renderer blends each flash type with the frame is not
//! in the specs (`halo_ce_ui::victim_view` says what we do); the
//! impulse's direction is kept relative to the player's facing as a
//! horizontal vector in the yaw's frame, applied in the camera's (pitched)
//! frame.

use std::collections::BTreeMap;

use bevy::prelude::*;
use blam_formats::{
    damage_effect::{DamageEffect, FlashPriority, ScreenFlashType},
    object::TransitionFunction,
    shader::{PeriodicFunction, PeriodicTables},
};
use serde::{Deserialize, Serialize};

use super::{ActiveFx, super::METERS_PER_UNIT};
use crate::{
    core::{Intent, LocalPlayer, NetRole},
    map::ParticleRng,
};
use crate::game::{damage::HaloHurt, vitality::DamageEffects};

/// Halo ticks a second.
const TICKS: f32 = 30.0;

/// A screen flash's numbers (projectiles.md 0x024 to 0x05C).
#[derive(Clone, Debug, PartialEq)]
pub struct FlashLook {
    /// 1 lighten, 2 darken, 3 max, 4 min, 5 invert, 6 tint (0, none, is
    /// no flash: no `FlashLook`).
    pub kind: u8,
    /// 0 low, 1 medium, 2 high.
    pub priority: u8,
    /// Seconds.
    pub duration: f32,
    pub fade: TransitionFunction,
    pub maximum: f32,
    pub zero: f32,
    /// RGBA fractions (the tag's ARGB reordered).
    pub color: [f32; 4],
}

/// The temporary camera impulse's numbers (0x098 to 0x0B0).
#[derive(Clone, Debug, PartialEq)]
pub struct ImpulseLook {
    /// Seconds.
    pub duration: f32,
    pub fade: TransitionFunction,
    /// Radians.
    pub rotation: f32,
    /// WU.
    pub pushback: f32,
    /// WU, lower and upper.
    pub jitter: [f32; 2],
    pub zero: f32,
}

/// The camera shake's numbers (0x0CC to 0x0F4).
#[derive(Clone, Debug, PartialEq)]
pub struct ShakeLook {
    /// Seconds.
    pub duration: f32,
    pub falloff: TransitionFunction,
    /// WU.
    pub translation: f32,
    /// Radians.
    pub rotation: f32,
    pub wobble: PeriodicFunction,
    /// Seconds.
    pub wobble_period: f32,
    pub wobble_weight: f32,
    pub zero: f32,
}

/// What a damage effect shows its victim (module doc): each part None
/// when the tag has none.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DamageLook {
    pub flash: Option<FlashLook>,
    pub impulse: Option<ImpulseLook>,
    /// The permanent impulse's angle (radians) and zero scale factor.
    pub permanent: Option<(f32, f32)>,
    pub shake: Option<ShakeLook>,
}

/// A damage effect tag's victim-side presentation.
pub fn look_of(d: &DamageEffect) -> DamageLook {
    let c = d.screen_flash_color;
    let flash = (d.screen_flash_type != ScreenFlashType::None).then(|| FlashLook {
        kind: d.screen_flash_type as u8,
        priority: match d.screen_flash_priority {
            FlashPriority::Low => 0,
            FlashPriority::Medium => 1,
            FlashPriority::High => 2,
        },
        duration: d.screen_flash_duration,
        fade: d.screen_flash_fade_function,
        maximum: d.screen_flash_maximum_intensity,
        zero: d.screen_flash_zero_scale_factor,
        color: [c.r, c.g, c.b, c.a],
    });
    let impulse = (d.temporary_camera_impulse_duration > 0.0).then(|| ImpulseLook {
        duration: d.temporary_camera_impulse_duration,
        fade: d.temporary_camera_impulse_fade_function,
        rotation: d.temporary_camera_impulse_rotation,
        pushback: d.temporary_camera_impulse_pushback,
        jitter: [d.temporary_camera_impulse_jitter.lower, d.temporary_camera_impulse_jitter.upper],
        zero: d.temporary_camera_impulse_zero_scale_factor,
    });
    let permanent = (d.permanent_camera_impulse_angle != 0.0).then_some((d.permanent_camera_impulse_angle, d.permanent_camera_impulse_zero_scale_factor));
    let shake = (d.camera_shaking_duration > 0.0).then(|| ShakeLook {
        duration: d.camera_shaking_duration,
        falloff: d.camera_shaking_falloff_function,
        translation: d.camera_shaking_random_translation,
        rotation: d.camera_shaking_random_rotation,
        wobble: d.camera_shaking_wobble_function,
        wobble_period: d.camera_shaking_wobble_period,
        wobble_weight: d.camera_shaking_wobble_weight,
        zero: d.camera_shaking_zero_scale_factor,
    });
    DamageLook { flash, impulse, permanent, shake }
}

/// The spec's intensity of a part with zero scale factor `z` at event
/// scale `s`: z + (1 - z) s.
pub fn intensity(z: f32, s: f32) -> f32 {
    z + (1.0 - z) * s
}

/// Seconds to the countdown's ticks: (short)(30 x seconds), truncated.
pub fn ticks_of(seconds: f32) -> f32 {
    (TICKS * seconds).trunc().max(0.0)
}

/// A running screen flash.
#[derive(Clone, Debug, PartialEq)]
pub struct FlashRun {
    pub kind: u8,
    pub priority: u8,
    /// I, clamped to the maximum intensity.
    pub intensity: f32,
    /// Ticks (truncated) and ticks left.
    pub duration: f32,
    pub left: f32,
    pub fade: TransitionFunction,
    pub color: [f32; 4],
    started: bool,
}

impl FlashRun {
    pub fn new(l: &FlashLook, s: f32) -> Self {
        let duration = ticks_of(l.duration);
        FlashRun {
            kind: l.kind,
            priority: l.priority,
            intensity: intensity(l.zero, s).clamp(0.0, l.maximum.max(0.0)),
            duration,
            left: duration,
            fade: l.fade,
            color: l.color,
            started: false,
        }
    }

    /// A new flash replaces this one unless this has a higher priority
    /// and more ticks left than the new one's duration.
    pub fn replaced_by(&self, new: &FlashRun) -> bool {
        !(self.priority > new.priority && self.left > new.duration)
    }

    /// This frame's shown intensity, then the countdown by `dt` ticks;
    /// None once it is over.
    pub fn frame(&mut self, dt: f32) -> Option<f32> {
        if self.started && self.left <= 0.0 {
            return None;
        }
        self.started = true;
        let shown = if self.duration <= 0.0 { self.intensity } else { self.fade.apply((self.intensity * self.left / self.duration).clamp(0.0, 1.0)) };
        self.left -= dt;
        Some(shown)
    }
}

/// A running temporary camera impulse (directions in the yaw's frame:
/// x right, y up, -z forward).
#[derive(Clone, Debug, PartialEq)]
pub struct ImpulseRun {
    pub duration: f32,
    pub elapsed: f32,
    pub fade: TransitionFunction,
    pub intensity: f32,
    pub rotation: f32,
    /// Metres.
    pub pushback: f32,
    /// The horizontal direction, unit (zero: none).
    pub dir: Vec3,
    /// Metres.
    pub jitter: Vec3,
    started: bool,
}

impl ImpulseRun {
    pub fn new(l: &ImpulseLook, s: f32, dir: Vec3, rng: &mut ParticleRng) -> Self {
        let dir = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
        // Perpendicular to the direction, its length drawn from the
        // bounds, turned by a random angle about it.
        let side = if dir == Vec3::ZERO { Vec3::X } else { Vec3::Y.cross(dir).normalize_or_zero() };
        let length = rng.float(l.jitter[0], l.jitter[1]) * METERS_PER_UNIT;
        let about = if dir == Vec3::ZERO { Vec3::NEG_Z } else { dir };
        let jitter = Quat::from_axis_angle(about, rng.random() * std::f32::consts::TAU) * side * length;
        ImpulseRun {
            duration: ticks_of(l.duration),
            elapsed: 0.0,
            fade: l.fade,
            intensity: intensity(l.zero, s),
            rotation: l.rotation,
            pushback: l.pushback * METERS_PER_UNIT,
            dir,
            jitter,
            started: false,
        }
    }

    /// The new impulse replaces this one when this one already counts
    /// down, or the new one is stronger, or as strong with more time.
    pub fn replaced_by(&self, new: &ImpulseRun) -> bool {
        self.started
            || new.intensity > self.intensity
            || (new.intensity == self.intensity && new.duration > self.duration - self.elapsed)
    }

    /// This frame's k, then the clock by `dt` ticks; None once over.
    pub fn k(&mut self, dt: f32) -> Option<f32> {
        let k = if !self.started {
            1.0
        } else if self.elapsed < self.duration {
            self.fade.apply((1.0 - self.elapsed / self.duration).clamp(0.0, 1.0)) * self.intensity
        } else {
            return None;
        };
        self.elapsed += dt;
        self.started = true;
        Some(k)
    }

    /// The view's rotation (a rotation vector) and offset for `k`: k x
    /// rotation about up x direction, k x (pushback x direction + jitter).
    pub fn view(&self, k: f32) -> (Vec3, Vec3) {
        let axis = Vec3::Y.cross(self.dir).normalize_or_zero();
        (axis * (k * self.rotation), (self.dir * self.pushback + self.jitter) * k)
    }
}

/// A running camera shake.
#[derive(Clone, Debug, PartialEq)]
pub struct ShakeRun {
    pub duration: f32,
    pub elapsed: f32,
    pub falloff: TransitionFunction,
    pub intensity: f32,
    /// Metres.
    pub translation: f32,
    pub rotation: f32,
    pub wobble: PeriodicFunction,
    pub wobble_period: f32,
    pub wobble_weight: f32,
    /// The tick whose directions are drawn, and the two directions.
    drawn: Option<i64>,
    axis: Vec3,
    toward: Vec3,
    started: bool,
}

/// A uniform random unit vector.
fn random_dir(rng: &mut ParticleRng) -> Vec3 {
    let z = rng.float(-1.0, 1.0);
    let a = rng.random() * std::f32::consts::TAU;
    let r = (1.0 - z * z).max(0.0).sqrt();
    Vec3::new(r * a.cos(), r * a.sin(), z)
}

impl ShakeRun {
    pub fn new(l: &ShakeLook, s: f32) -> Self {
        ShakeRun {
            duration: ticks_of(l.duration),
            elapsed: 0.0,
            falloff: l.falloff,
            intensity: intensity(l.zero, s),
            translation: l.translation * METERS_PER_UNIT,
            rotation: l.rotation,
            wobble: l.wobble,
            wobble_period: l.wobble_period,
            wobble_weight: l.wobble_weight,
            drawn: None,
            axis: Vec3::ZERO,
            toward: Vec3::ZERO,
            started: false,
        }
    }

    /// The new shake replaces this one when it is longer than the time
    /// left, or stronger.
    pub fn replaced_by(&self, new: &ShakeRun) -> bool {
        new.duration > self.duration - self.elapsed || new.intensity > self.intensity
    }

    /// This frame's magnitude, then the clock by `dt` ticks; None once
    /// over.
    pub fn magnitude(&mut self, dt: f32, tables: &PeriodicTables) -> Option<f32> {
        let k = if !self.started {
            1.0
        } else if self.elapsed < self.duration {
            self.falloff.apply((1.0 - self.elapsed / self.duration).clamp(0.0, 1.0)) * self.intensity
        } else {
            return None;
        };
        let period = if self.wobble_period > 0.0 { self.wobble_period } else { 1.0 };
        let wobble = tables.value(self.wobble, self.elapsed / (TICKS * period));
        let m = k * (wobble * self.wobble_weight + 1.0 - self.wobble_weight);
        self.elapsed += dt;
        self.started = true;
        Some(m)
    }

    /// The view's rotation and offset at magnitude `m`, the directions
    /// drawn once per tick.
    pub fn view(&mut self, m: f32, rng: &mut ParticleRng) -> (Vec3, Vec3) {
        let tick = self.elapsed.floor() as i64;
        if self.drawn != Some(tick) {
            self.drawn = Some(tick);
            self.axis = random_dir(rng);
            self.toward = random_dir(rng);
        }
        (self.axis * (m * self.rotation), self.toward * (m * self.translation))
    }
}

/// The flash as the screen shows it this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShownFlash {
    /// `FlashLook::kind`.
    pub kind: u8,
    /// RGBA fractions.
    pub color: [f32; 4],
    pub intensity: f32,
}

/// What the local player's view shows of its hits this frame (module
/// doc), in the camera's frame.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct VictimView {
    pub flash: Option<ShownFlash>,
    /// A rotation vector (axis times radians).
    pub rotation: Vec3,
    /// Metres.
    pub offset: Vec3,
}

/// The local player's running flash, impulse and shake.
#[derive(Resource)]
pub struct VictimFx {
    pub flash: Option<FlashRun>,
    pub impulse: Option<ImpulseRun>,
    pub shake: Option<ShakeRun>,
    pub rng: ParticleRng,
    pub tables: PeriodicTables,
}

impl Default for VictimFx {
    fn default() -> Self {
        Self { flash: None, impulse: None, shake: None, rng: ParticleRng::new(0x7EC7_1A), tables: PeriodicTables::new() }
    }
}

impl VictimFx {
    /// A hit with `look` at scale `s` from direction `dir` (in the yaw's
    /// frame): each part starts or not by its replacement rule.
    pub fn hit(&mut self, look: &DamageLook, s: f32, dir: Vec3) {
        if let Some(l) = &look.flash {
            let new = FlashRun::new(l, s);
            if self.flash.as_ref().is_none_or(|c| c.left <= 0.0 || c.replaced_by(&new)) {
                self.flash = Some(new);
            }
        }
        if let Some(l) = &look.impulse {
            let new = ImpulseRun::new(l, s, dir, &mut self.rng);
            if self.impulse.as_ref().is_none_or(|c| c.replaced_by(&new)) {
                self.impulse = Some(new);
            }
        }
        if let Some(l) = &look.shake {
            let new = ShakeRun::new(l, s);
            if self.shake.as_ref().is_none_or(|c| c.replaced_by(&new)) {
                self.shake = Some(new);
            }
        }
    }

    /// This frame's view after `dt` ticks (each part's frame rule).
    pub fn frame(&mut self, dt: f32) -> VictimView {
        let mut view = VictimView::default();
        if let Some(f) = &mut self.flash {
            match f.frame(dt) {
                Some(i) => view.flash = Some(ShownFlash { kind: f.kind, color: f.color, intensity: i }),
                None => self.flash = None,
            }
        }
        if let Some(i) = &mut self.impulse {
            match i.k(dt) {
                Some(k) => {
                    let (r, o) = i.view(k);
                    view.rotation += r;
                    view.offset += o;
                }
                None => self.impulse = None,
            }
        }
        if let Some(s) = &mut self.shake {
            match s.magnitude(dt, &self.tables) {
                Some(m) => {
                    let (r, o) = s.view(m, &mut self.rng);
                    view.rotation += r;
                    view.offset += o;
                }
                None => self.shake = None,
            }
        }
        view
    }

    pub fn clear(&mut self) {
        self.flash = None;
        self.impulse = None;
        self.shake = None;
    }
}

/// The permanent impulse's turn of the aim (radians: yaw, pitch) for
/// event direction `d` (engine space) on a player looking by `yaw` and
/// `pitch`: yaw by (left . d) angle i, pitch by (forward . d) angle i.
pub fn permanent_turn(angle: f32, zero: f32, s: f32, d: Vec3, yaw: f32, pitch: f32) -> (f32, f32) {
    let aim = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
    let i = intensity(zero, s);
    let left = aim * Vec3::NEG_X;
    let forward = aim * Vec3::NEG_Z;
    (left.dot(d) * angle * i, forward.dot(d) * angle * i)
}

/// A hit's damage effect reached the local player (module doc).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct HaloVictim {
    /// The effect's index in `DamageEffects` (`DamageEffects::index_of`).
    pub effect: usize,
    /// The event's scale.
    pub scale: f32,
    /// The direction the damage travelled (engine space, unit or zero).
    pub dir: Vec3,
    /// Someone other than the victim dealt it.
    pub directed: bool,
}

/// Server -> the client whose character was hit: `HaloVictim`.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HaloVictimNet {
    pub effect: u8,
    /// `DIRECTED`.
    pub flags: u8,
    pub scale: f32,
    pub dir: [f32; 3],
}

/// `HaloVictimNet::flags`: someone else dealt it.
pub const DIRECTED: u8 = 1;

/// The victim's side of a server's hurt.
fn victim_of(h: &HaloHurt, effects: &DamageEffects) -> Option<HaloVictim> {
    Some(HaloVictim {
        effect: effects.index_of(h.effect.0)?,
        scale: h.scale,
        dir: h.dir.normalize_or_zero(),
        directed: h.instigator.is_some_and(|i| i != h.target),
    })
}

/// The authority's hurts of its own local player (a listen server, a
/// game without a network).
fn local_hurts(mut hurts: MessageReader<HaloHurt>, effects: Res<DamageEffects>, locals: Query<(), With<LocalPlayer>>, role: Option<Res<NetRole>>, mut out: MessageWriter<HaloVictim>) {
    if role.as_deref() == Some(&NetRole::Client) {
        hurts.clear();
        return;
    }
    for h in hurts.read() {
        if locals.contains(h.target)
            && let Some(v) = victim_of(h, &effects)
        {
            out.write(v);
        }
    }
}

/// The server's hurts to the client whose character it hit.
fn send(
    mut hurts: MessageReader<HaloHurt>,
    effects: Res<DamageEffects>,
    players: Query<(Entity, &chimera_net::net::server::Player)>,
    mut out: MessageWriter<bevy_replicon::prelude::ToClients<HaloVictimNet>>,
) {
    use bevy_replicon::prelude::{ClientId, SendTargets, ToClients};
    for h in hurts.read() {
        let Some(v) = victim_of(h, &effects) else { continue };
        let Ok(effect) = u8::try_from(v.effect) else { continue };
        for (client, _) in players.iter().filter(|(_, p)| p.character == h.target) {
            out.write(ToClients {
                targets: SendTargets::Single(ClientId::Client(client)),
                message: HaloVictimNet { effect, flags: if v.directed { DIRECTED } else { 0 }, scale: v.scale, dir: v.dir.to_array() },
            });
        }
    }
}

/// A client's own hits from its server.
fn receive(mut hits: MessageReader<HaloVictimNet>, mut out: MessageWriter<HaloVictim>) {
    for h in hits.read() {
        out.write(HaloVictim { effect: h.effect as usize, scale: h.scale, dir: Vec3::from_array(h.dir), directed: h.flags & DIRECTED != 0 });
    }
}

/// The looks by tag path of the library now loaded (lower case).
fn look_for<'a>(fx: &'a ActiveFx, effects: &DamageEffects, index: usize) -> Option<&'a DamageLook> {
    let (_, e) = effects.at(index)?;
    fx.0.as_ref()?.damage.get(&e.tag.to_ascii_lowercase())
}

/// The local player's hits started and evaluated (module doc).
#[allow(clippy::too_many_arguments)]
pub fn present(
    mut hits: MessageReader<HaloVictim>,
    effects: Res<DamageEffects>,
    fx: Res<ActiveFx>,
    time: Res<Time>,
    mut me: Query<&mut Intent, With<LocalPlayer>>,
    mut state: ResMut<VictimFx>,
    mut view: ResMut<VictimView>,
) {
    let Ok(mut intent) = me.single_mut() else {
        hits.clear();
        state.clear();
        view.set_if_neq(VictimView::default());
        return;
    };
    for h in hits.read() {
        let Some(look) = look_for(&fx, &effects, h.effect) else { continue };
        // The direction in the yaw's frame.
        let local = Quat::from_rotation_y(-intent.yaw) * h.dir;
        state.hit(look, h.scale, local);
        if let Some((angle, zero)) = look.permanent {
            let (dy, dp) = permanent_turn(angle, zero, h.scale, h.dir, intent.yaw, intent.pitch);
            intent.yaw = (intent.yaw + dy).rem_euclid(std::f32::consts::TAU);
            intent.pitch = (intent.pitch + dp).clamp(-std::f32::consts::FRAC_PI_2 + 0.01, std::f32::consts::FRAC_PI_2 - 0.01);
        }
    }
    let now = state.frame(time.delta_secs() * TICKS);
    view.set_if_neq(now);
}

/// Where the hits reach the local player: before what reads `HaloVictim`
/// this frame (the presentation, the HUD's arrows).
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VictimSystems;

/// The looks of every damage effect tag of a map file, by lower-case
/// path.
pub fn compile(file: &blam_formats::cache::MapFile) -> BTreeMap<String, DamageLook> {
    file.of_class(blam_formats::cache::class::DAMAGE_EFFECT)
        .filter_map(|t| DamageEffect::read(file, t).ok().map(|d| (t.path.to_ascii_lowercase(), look_of(&d))))
        .collect()
}

fn register_net(app: &mut App) {
    use bevy_replicon::prelude::*;
    app.add_server_message::<HaloVictimNet>(Channel::Unordered)
        .add_systems(PostUpdate, send.before(ServerSystems::Send).run_if(resource_equals(NetRole::Server)))
        .add_systems(PreUpdate, receive.after(ClientSystems::Receive).run_if(resource_equals(NetRole::Client)));
}

pub(super) fn plugin(app: &mut App) {
    use chimera_net::net::parts::NetAppExt;
    app.add_message::<HaloVictim>()
        .add_message::<HaloHurt>()
        .init_resource::<VictimFx>()
        .init_resource::<VictimView>()
        .add_systems(Update, (local_hurts, present).chain().in_set(VictimSystems))
        .net_part("halo_ce_victim", register_net);
}
