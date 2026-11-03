//! The gravity gun's effects (specs/hl2dm/effects_sounds.md 11 and 15.3,
//! gravity_gun.md 10-11): what a punt or launch looks like
//! (`PhyscannonImpact`: a spark burst at the end point turned back toward
//! the gun, electric sparks, and for the punter a beam of
//! `sprites/orangelight1` from the gun to the point, 12 units wide to 4,
//! for 0.1 s, and the white fire fade (245, 245, 255, 32 over 0.1 s)), the
//! client-side hold loop (`Weapon_PhysCannon.HoldSound`: from volume 0 and
//! pitch 50 to 0.8 and 100 over 0.5 s while it holds, back to 0 and 50
//! over 1 s after) and a held combine ball's loop
//! (`NPC_CombineBall.HoldingInPhysCannon`, its pitch ramping to 150 over
//! the last second of its 8).
//!
//! Known shortcuts (docs/tech-debt.md): the claws' core glow, glows, end
//! caps and beams (`sprites/orangecore1`, `glow04_noz`, `orangeflare1`,
//! `orangelight1_noz` on the view and world models' `fork` attachments)
//! are not drawn: they need persistent sprites on a model's attachments,
//! which the render layer only has for one-frame muzzle flashes; the
//! punt's end point is the end of the eye's 250-unit trace (not the
//! object's centre) and its beam starts at an estimate of the view
//! model's attachment 1 (eye + 0.4 ahead, 0.1 right, 0.1 down), the
//! other players' sparks at their eye. Why it's OK for now: the launch
//! reads as the spec's burst, beam and fade; the claws need the render
//! layer's attachments.

use std::collections::BTreeMap;

use bevy::prelude::*;
use chimera_sim::weapon::{Inventory, Weapon, WeaponEvent, WeaponEventKind};
use source_engine::effects::impact_effects::{self, EffectMaterials};

use super::{
    weapons::{
        self,
        physcannon::{HeldBall, Hl2Physcannon, PUNT},
    },
};
use crate::{
    core::{Intent, LocalPlayer, MovementState},
    map::{
        ParticleRng, SoundControl, SoundKey, StartSound,
        logic_view::{ScreenFade, ScreenFades},
        particles::{
            Fade, Motion, Particle, ParticleGroup, ParticleMaterials, Particles, Ramp, Shape, WorldTrace, WorldTracer,
        },
    },
    weapon::aim_of,
};

const U: f32 = 0.0254;

/// The hold loop's key; a held ball's is `BALL_KEYS` + its entity index.
const HOLD_KEY: SoundKey = SoundKey(0x6E4C_0001);
const BALL_KEYS: u64 = 0x6E4D_0000_0000;

pub const HOLD_SOUND: &str = "Weapon_PhysCannon.HoldSound";
pub const BALL_HOLD_SOUND: &str = "NPC_CombineBall.HoldingInPhysCannon";

/// The hold loop's volume and pitch `t` seconds after the gun started
/// holding (0 to 0.8 and 50 to 100 over 0.5 s).
pub fn hold_ramp_up(t: f32) -> (f32, f32) {
    let k = (t / 0.5).clamp(0.0, 1.0);
    (0.8 * k, 50.0 + 50.0 * k)
}

/// After it stopped holding, `t` seconds on, from `(volume, pitch)` then:
/// to volume 0 and pitch 50 over 1 s.
pub fn hold_ramp_down(from: (f32, f32), t: f32) -> (f32, f32) {
    let k = (t / 1.0).clamp(0.0, 1.0);
    (from.0 * (1.0 - k), from.1 + (50.0 - from.1) * k)
}

/// A held ball's loop pitch `t` seconds after the catch: 100, rising to
/// 150 over the last second of its 8 (`life`).
pub fn ball_pitch(t: f32, life: f32) -> f32 {
    100.0 + 50.0 * ((t - (life - 1.0)) / 1.0).clamp(0.0, 1.0)
}

#[derive(Resource)]
struct FxRng(ParticleRng);

impl Default for FxRng {
    fn default() -> Self {
        Self(ParticleRng::new(0x6A_5C0))
    }
}

/// The spark burst of a launch at `end`, back toward `gun` (spec 11):
/// 16 to 32 `effects/spark` trails, width U(1, 4), length U(0.01, 0.1),
/// life U(0.25, 1), speed U(128, 512) around the direction ± 0.6, gravity
/// 800.
fn burst(rng: &mut ParticleRng, spark: usize, end: Vec3, gun: Vec3) -> ParticleGroup {
    let back = (gun - end).normalize_or(Vec3::Y);
    let mut g = ParticleGroup::new(Motion { gravity: 800.0 * U, ..default() });
    for _ in 0..rng.int(16, 32) {
        let dir = (back + rng.vec3(-0.6, 0.6)).normalize_or(back);
        g.particles.push(Particle {
            velocity: dir * rng.float(128.0, 512.0) * U,
            fade: Fade::Ramp(Ramp::linear(1.0, 0.0)),
            shape: Shape::Trail { width: rng.float(1.0, 4.0) * U, length: rng.float(0.01, 0.1) },
            ..Particle::new(end, rng.float(0.25, 1.0), spark, 0.0)
        });
    }
    g
}

/// The punter's beam from the gun to the point: white, 12 units wide at
/// the gun to 4 at the point, 0.1 s.
fn beam(material: usize, from: Vec3, to: Vec3) -> ParticleGroup {
    let mut g = ParticleGroup::new(Motion::default());
    g.particles.push(Particle {
        fade: Fade::Ramp(Ramp::linear(1.0, 0.0)),
        shape: Shape::Ribbon {
            to,
            width: [12.0 * U, 4.0 * U],
            u: (0.0, 1.0),
            end_color: None,
            prev: None,
            next: None,
        },
        ..Particle::new(from, 0.1, material, 1.0)
    });
    g
}

/// A punt or launch of a gravity gun: the burst, and for the local
/// player's own the beam and the fade.
#[allow(clippy::too_many_arguments)]
fn launches(
    mut events: MessageReader<WeaponEvent>,
    weapons_q: Query<&Weapon>,
    owners: Query<(&Transform, &MovementState, &Intent)>,
    local: Query<(), With<LocalPlayer>>,
    materials: Option<Res<ParticleMaterials>>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<FxRng>,
    mut fades: Option<ResMut<ScreenFades>>,
    time: Res<Time>,
    world: WorldTracer,
) {
    for e in events.read().filter(|e| e.shown()) {
        let WeaponEventKind::Action(name) = e.kind else { continue };
        if name != PUNT || weapons_q.get(e.weapon).map_or(true, |w| w.id != weapons::PHYSCANNON) {
            continue;
        }
        let Ok((at, state, intent)) = owners.get(e.owner) else { continue };
        let eye = at.translation + state.eye_offset;
        let q = aim_of(intent.yaw, intent.pitch);
        let fwd = q * Vec3::NEG_Z;
        let far = eye + fwd * 250.0 * U;
        let end = world.trace(eye, far).map_or(far, |(f, _)| eye + (far - eye) * f);
        let mine = local.contains(e.owner);
        let muzzle = if mine { eye + fwd * 0.4 + q * Vec3::X * 0.1 - q * Vec3::Y * 0.1 } else { eye };
        if let Some(m) = materials.as_ref() {
            if let Some(spark) = m.0.find("effects/spark") {
                particles.add(burst(&mut rng.0, spark, end, muzzle));
            }
            let mats = EffectMaterials::new(&m.0);
            let n = (muzzle - end).normalize_or(Vec3::Y);
            for g in impact_effects::electric_sparks(&mut rng.0, &mats, end, n, &world) {
                particles.add(g);
            }
            if mine && let Some(b) = m.0.find("sprites/orangelight1") {
                particles.add(beam(b, muzzle, end));
            }
        }
        if mine && let Some(f) = fades.as_mut() {
            f.show(
                ScreenFade {
                    duration: ScreenFade::quantize(0.1),
                    hold: 0.0,
                    color: [245, 245, 255],
                    alpha: 32,
                    fade_in: true,
                    modulate: false,
                    stay_out: false,
                    purge: false,
                },
                time.elapsed_secs_f64(),
            );
        }
    }
}

/// The local player's hold loop and what it was when it stopped holding.
#[derive(Default)]
struct HoldLoop {
    since: Option<f64>,
    stopped: Option<(f64, (f32, f32))>,
}

/// `Weapon_PhysCannon.HoldSound` (spec 11): started when the local
/// player's gun holds something, ramped, faded out when it lets go.
fn hold_sound(
    local: Query<&Inventory, With<LocalPlayer>>,
    guns: Query<&Hl2Physcannon>,
    time: Res<Time>,
    mut state: Local<HoldLoop>,
    mut control: MessageWriter<SoundControl>,
) {
    let now = time.elapsed_secs_f64();
    let holding = local
        .iter()
        .filter_map(|inv| inv.active)
        .filter_map(|a| guns.get(a).ok())
        .any(|g| g.holding);
    if holding {
        let t0 = *state.since.get_or_insert_with(|| {
            control.write(SoundControl::Start(StartSound {
                key: HOLD_KEY,
                entry: HOLD_SOUND.into(),
                volume: Some(0.0),
                pitch: Some(50.0),
                looping: true,
                ..default()
            }));
            now
        });
        state.stopped = None;
        let (volume, pitch) = hold_ramp_up((now - t0) as f32);
        control.write(SoundControl::Change { key: HOLD_KEY, volume: Some(volume), pitch: Some(pitch) });
    } else if let Some(t0) = state.since.take() {
        state.stopped = Some((now, hold_ramp_up((now - t0) as f32)));
    }
    if let Some((t, from)) = state.stopped {
        let (volume, pitch) = hold_ramp_down(from, (now - t) as f32);
        control.write(SoundControl::Change { key: HOLD_KEY, volume: Some(volume), pitch: Some(pitch) });
        if now - t >= 1.0 {
            control.write(SoundControl::Stop(HOLD_KEY));
            state.stopped = None;
        }
    }
}

/// A held combine ball's loop: from the catch, its pitch to 150 over the
/// last second before it goes off.
fn ball_sound(
    balls: Query<(Entity, &HeldBall)>,
    time: Res<Time>,
    mut playing: Local<BTreeMap<Entity, f64>>,
    mut control: MessageWriter<SoundControl>,
) {
    let now = time.elapsed_secs_f64();
    for (e, b) in &balls {
        let key = SoundKey(BALL_KEYS + (e.to_bits() & 0xFFFF_FFFF));
        // The catch is 8 s before it goes off.
        let start = *playing.entry(e).or_insert_with(|| {
            control.write(SoundControl::Start(StartSound {
                key,
                entry: BALL_HOLD_SOUND.into(),
                follow: Some(e),
                looping: true,
                ..default()
            }));
            now
        });
        let life = (b.explode_at - start).max(1.0) as f32;
        control.write(SoundControl::Change { key, volume: None, pitch: Some(ball_pitch((now - start) as f32, life)) });
    }
    playing.retain(|e, _| {
        let held = balls.contains(*e);
        if !held {
            control.write(SoundControl::Stop(SoundKey(BALL_KEYS + (e.to_bits() & 0xFFFF_FFFF))));
        }
        held
    });
}

pub struct Hl2PhyscannonFxPlugin;

impl Plugin for Hl2PhyscannonFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FxRng>()
            .add_message::<SoundControl>()
            .add_systems(FixedUpdate, launches.after(crate::core::SimSet::Weapons))
            .add_systems(Update, (hold_sound, ball_sound));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hold_ramps() {
        assert_eq!(hold_ramp_up(0.0), (0.0, 50.0));
        let (v, p) = hold_ramp_up(0.25);
        assert!((v - 0.4).abs() < 1e-6 && (p - 75.0).abs() < 1e-4);
        assert_eq!(hold_ramp_up(2.0), (0.8, 100.0));
        let (v, p) = hold_ramp_down((0.8, 100.0), 0.5);
        assert!((v - 0.4).abs() < 1e-6 && (p - 75.0).abs() < 1e-4);
        assert_eq!(hold_ramp_down((0.8, 100.0), 3.0), (0.0, 50.0));
    }

    #[test]
    fn the_balls_pitch_climbs_in_the_last_second() {
        assert_eq!(ball_pitch(3.0, 8.0), 100.0);
        assert_eq!(ball_pitch(7.0, 8.0), 100.0);
        assert!((ball_pitch(7.5, 8.0) - 125.0).abs() < 1e-4);
        assert_eq!(ball_pitch(8.0, 8.0), 150.0);
    }
}
