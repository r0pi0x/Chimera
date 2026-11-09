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
//! **The claws' sprites and beams** (spec's "Gravity gun sprites and
//! beams" table; `claws`, every frame, through Chimera's `map::attached`):
//! by the gun's effect state (`GunEffect`: none when it isn't in hand,
//! ready, holding while it holds or its secondary search runs; the
//! server's `Hl2GunEffect` on each player, replicated, for others on a
//! network client; computed from the gun for the rest), the core
//! (`sprites/orangecore1` at attachment 1) moving linearly to the state's
//! width and alpha, six `sprites/glow04_noz` glows (255, 128, 0) on the
//! `fork` attachments re-drawn every frame at width U(6.4, 9.6) and alpha
//! I(24, 32), and while holding the end caps (`sprites/orangeflare1`) and
//! the beams from the fork tips to attachment 1 (noise 16, 8 segments);
//! widths are full widths (the spec's "glow quads"); the view model's set
//! in first person (drawn over the claws, not depth tested against the
//! view model), the world model's (`fork3`
//! too) for everyone else. A punt or launch adds the blast sprite
//! (`sprites/orangecore2`, width 8 to 64 in 0.1 s, alpha 255 to 0 in
//! 0.2 s) for every machine drawing the gun; the punter's beam starts at
//! the view model's attachment 1 (re-projected), others' sparks at the
//! world gun's.
//!
//! The end point of a punt or launch is the punted or launched object's
//! centre (its box's, the server's `Hl2Physcannon::punted`), else the end
//! of the eye's 250-unit trace.
//!
//! Known shortcuts (docs/tech-debt.md): a network client's own punt (it
//! doesn't know what the server punted) ends at its eye trace's end; the
//! "closed" effect state is never entered (the spec names no change into
//! it, and it looks as the state before it with the end caps hidden);
//! the beams' texture scrolls at 15
//! lengths a second (a temporary beam's 0.1 × 150) and their frames don't
//! play; without a drawn model (headless) the punt beam starts at an
//! estimate of attachment 1 (eye + 0.4 ahead, 0.1 right, 0.1 down) and
//! others' sparks at their eye. Why it's OK for now: the sizes, colours,
//! states and timings are the spec's; the rest needs captures of the real
//! game.

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
    core::{Intent, LocalPlayer, MovementState, NetRole, SimClock},
    map::{
        PhysicsProp,
        prop_physics::FrozenBody,
        ShowLocalBody, ViewAnimator, ViewModelCamera, ViewModelOffset, ViewModelSettings, ViewModels,
        attached::{self, AttachPoint, AttachedFx, AttachedPiece, BeamEnd},
        view_model::HeldAttachment,
        ParticleRng, SoundControl, SoundKey, StartSound,
        logic_view::{ScreenFade, ScreenFades},
        particles::{
            Fade, Motion, Particle, ParticleGroup, ParticleMaterials, Particles, Ramp, Shape, WorldTrace,
            WorldTracer,
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

/// The materials the gun's sprites and beams use (loaded with the map's,
/// additive: `effects::ADDITIVE_SPRITES`).
pub const MATERIALS: &[&str] = &[
    CORE,
    BLAST,
    GLOW,
    END_CAP,
    VIEW_BEAM,
    WORLD_BEAM,
];

pub const CORE: &str = "sprites/orangecore1";
pub const BLAST: &str = "sprites/orangecore2";
pub const GLOW: &str = "sprites/glow04_noz";
pub const END_CAP: &str = "sprites/orangeflare1";
pub const VIEW_BEAM: &str = "sprites/orangelight1_noz";
pub const WORLD_BEAM: &str = "sprites/orangelight1";

/// The glows' attachments on the view model and on the world model.
pub const VIEW_GLOWS: [&str; 6] = ["fork1b", "fork1m", "fork1t", "fork2b", "fork2m", "fork2t"];
pub const WORLD_GLOWS: [&str; 6] = ["fork1m", "fork1t", "fork2m", "fork2t", "fork3m", "fork3t"];
/// The end caps' and the beams' (the fork tips).
pub const VIEW_TIPS: [&str; 2] = ["fork1t", "fork2t"];
pub const WORLD_TIPS: [&str; 3] = ["fork1t", "fork2t", "fork3t"];

/// The glows' colour.
pub const GLOW_COLOR: Vec3 = Vec3::new(1.0, 128.0 / 255.0, 0.0);

/// The gun's effect state (gravity_gun.md 10; effects_sounds.md 11).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, serde::Serialize, serde::Deserialize)]
pub enum GunEffect {
    /// Not in hand: everything hidden.
    #[default]
    None,
    /// The idle glow.
    Ready,
    /// Holding or searching: beams, end caps, the brighter core.
    Holding,
}

/// A player's gravity gun effect state, the server's (replicated): what
/// other players' guns show on a network client.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
pub struct Hl2GunEffect(pub GunEffect);

/// The state a gun's frame leaves: holding, or its secondary search
/// running (pressed and not masked), else ready.
pub fn effect_of(gun: &Hl2Physcannon) -> GunEffect {
    if gun.holding || (gun.secondary_was && !gun.masked) {
        GunEffect::Holding
    } else {
        GunEffect::Ready
    }
}

/// The core's width (units) and alpha (0-255) a state moves to, and over
/// how long (s), on the view model (`view`) or the world model.
pub fn core_target(state: GunEffect, view: bool) -> ((f32, f32), (f32, f32)) {
    match (state, view) {
        (GunEffect::Ready, true) => ((14.0, 0.2), (128.0, 0.2)),
        (GunEffect::Holding, true) => ((16.0, 0.2), (255.0, 0.1)),
        (GunEffect::Ready, false) => ((8.0, 0.2), (0.0, 0.2)),
        (GunEffect::Holding, false) => ((14.0, 0.2), (255.0, 0.1)),
        (GunEffect::None, _) => ((0.0, 0.0), (0.0, 0.0)),
    }
}

/// A value moving linearly from `from` to `to` over `over` seconds; `t`
/// seconds since it started.
pub fn approach(from: f32, to: f32, over: f32, t: f32) -> f32 {
    if over <= 0.0 {
        return to;
    }
    from + (to - from) * (t / over).clamp(0.0, 1.0)
}

/// The blast sprite `t` seconds after a punt or launch: width (units) 8
/// to 64 over 0.1 s, alpha (0-255) 255 to 0 over 0.2 s; None after.
pub fn blast_at(t: f32) -> Option<(f32, f32)> {
    (0.0..0.2).contains(&t).then(|| (approach(8.0, 64.0, 0.1, t), approach(255.0, 0.0, 0.2, t)))
}

/// One core's moving width and alpha (view or world model).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CoreLook {
    /// The values when the current state began, and when.
    pub from: (f32, f32),
    pub since: f64,
}

/// What a character's gun is drawing: its state, the two cores and the
/// last punt or launch (local to each machine).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct GunLook {
    pub state: GunEffect,
    pub view: CoreLook,
    pub world: CoreLook,
    pub blast: Option<f64>,
}

impl GunLook {
    /// The core's (width, alpha) now.
    pub fn core(&self, view: bool, now: f64) -> (f32, f32) {
        let c = if view { self.view } else { self.world };
        let ((w, tw), (a, ta)) = core_target(self.state, view);
        let t = (now - c.since) as f32;
        (approach(c.from.0, w, tw, t), approach(c.from.1, a, ta, t))
    }

    /// Change to `state` at `now`: the cores move on from where they are
    /// (hidden, back to nothing).
    pub fn set(&mut self, state: GunEffect, now: f64) {
        if state == self.state {
            return;
        }
        let (v, w) = if state == GunEffect::None {
            ((0.0, 0.0), (0.0, 0.0))
        } else {
            (self.core(true, now), self.core(false, now))
        };
        self.view = CoreLook { from: v, since: now };
        self.world = CoreLook { from: w, since: now };
        self.state = state;
    }
}

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
    mut looks: Query<&mut GunLook>,
    held: Query<(&HeldAttachment, &GlobalTransform)>,
    view: FirstPersonView,
    guns: Query<&Hl2Physcannon>,
    objects: Query<(&Transform, Option<&PhysicsProp>, Option<&FrozenBody>)>,
    clock: Res<SimClock>,
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
        // The punted or launched object's centre (spec 11; the server's,
        // set on this tick), else the end of the eye's trace.
        let end = guns
            .get(e.weapon)
            .ok()
            .and_then(|g| g.punted)
            .filter(|(_, until)| *until > clock.now)
            .and_then(|(o, _)| objects.get(o).ok())
            .map(|(t, prop, frozen)| {
                let b = prop.map(|p| p.bounds).or(frozen.map(|f| f.bounds));
                t.translation + b.map_or(Vec3::ZERO, |b| t.rotation * (b.0 + b.1) / 2.0)
            })
            .unwrap_or_else(|| world.trace(eye, far).map_or(far, |(f, _)| eye + (far - eye) * f));
        let mine = local.contains(e.owner);
        if let Ok(mut look) = looks.get_mut(e.owner) {
            look.blast = Some(time.elapsed_secs_f64());
        }
        // Attachment 1: the punter's view model's (re-projected), else the
        // world gun's; without a drawn model, an estimate.
        let core = AttachPoint::Index(0);
        let shown = view.first(e.owner, time.elapsed_secs_f64());
        let muzzle = match (mine, shown) {
            (true, Some(f)) => f.world_point(&core),
            (false, _) => attached::attachment_point(&held, e.owner, &core),
            _ => None,
        }
        .unwrap_or(if mine { eye + fwd * 0.4 + q * Vec3::X * 0.1 - q * Vec3::Y * 0.1 } else { eye });
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

/// What a character's first-person view model is, as `map::attached`
/// draws it.
#[derive(bevy::ecs::system::SystemParam)]
pub struct FirstPersonView<'w, 's> {
    owners: Query<
        'w,
        's,
        (Option<&'static ViewAnimator>, Option<&'static ViewModelOffset>, Option<&'static crate::core::MapControls>),
    >,
    anchors: attached::AnchorQuery<'w, 's>,
    vm_cameras: Query<'w, 's, &'static Projection, With<ViewModelCamera>>,
    models: Option<Res<'w, ViewModels>>,
    settings: Option<Res<'w, ViewModelSettings>>,
    third_person: Option<Res<'w, ShowLocalBody>>,
}

impl FirstPersonView<'_, '_> {
    /// `owner`'s view model if it is drawn.
    pub fn first(&self, owner: Entity, now: f64) -> Option<attached::FirstPerson<'_>> {
        let (view, offset, controls) = self.owners.get(owner).ok()?;
        attached::first_person_shown(
            owner,
            view,
            offset,
            controls,
            &self.anchors,
            &self.vm_cameras,
            self.models.as_deref(),
            self.settings.as_deref().copied().unwrap_or_default(),
            self.third_person.as_ref().is_some_and(|t| t.0),
            now,
        )
    }
}

/// The server's: each player's gun effect state, for clients drawing
/// others' guns.
fn server_effects(
    players: Query<(Entity, &Inventory, Option<&Hl2GunEffect>)>,
    guns: Query<&Hl2Physcannon>,
    mut commands: Commands,
) {
    for (e, inv, had) in &players {
        let state = inv.active.and_then(|a| guns.get(a).ok()).map_or(GunEffect::None, effect_of);
        if had.map(|h| h.0) != Some(state) && (had.is_some() || state != GunEffect::None) {
            commands.entity(e).insert(Hl2GunEffect(state));
        }
    }
}

/// The material indices of the claws' look.
struct ClawMaterials {
    core: usize,
    blast: Option<usize>,
    glow: Option<usize>,
    cap: Option<usize>,
    view_beam: Option<usize>,
    world_beam: Option<usize>,
}

impl ClawMaterials {
    fn new(m: &crate::map::particles::MapParticles) -> Option<Self> {
        Some(Self {
            core: m.find(CORE)?,
            blast: m.find(BLAST),
            glow: m.find(GLOW),
            cap: m.find(END_CAP),
            view_beam: m.find(VIEW_BEAM),
            world_beam: m.find(WORLD_BEAM),
        })
    }
}

/// The pieces a gun in `look`'s state shows on one model (`view`: the
/// view model's set) at `now`.
fn claw_pieces(look: &GunLook, view: bool, m: &ClawMaterials, rng: &mut ParticleRng, now: f64) -> Vec<AttachedPiece> {
    let mut out = Vec::new();
    let sprite = |at: AttachPoint, material: usize, width: f32, color: Vec3, alpha: f32| AttachedPiece::Sprite {
        at,
        material,
        width: width * U,
        color,
        alpha: alpha / 255.0,
    };
    if let Some((w, a)) = look.blast.and_then(|t| blast_at((now - t) as f32))
        && let Some(b) = m.blast
    {
        out.push(sprite(AttachPoint::Index(0), b, w, Vec3::ONE, a));
    }
    if look.state == GunEffect::None {
        return out;
    }
    let (w, a) = look.core(view, now);
    if a > 0.0 && w > 0.0 {
        out.push(sprite(AttachPoint::Index(0), m.core, w, Vec3::ONE, a));
    }
    if let Some(g) = m.glow {
        for name in if view { &VIEW_GLOWS[..] } else { &WORLD_GLOWS[..] } {
            let (w, a) = (rng.float(6.4, 9.6), rng.int(24, 32) as f32);
            out.push(sprite(AttachPoint::named(name), g, w, GLOW_COLOR, a));
        }
    }
    if look.state != GunEffect::Holding {
        return out;
    }
    let tips = if view { &VIEW_TIPS[..] } else { &WORLD_TIPS[..] };
    if let Some(c) = m.cap {
        for name in tips {
            let (w, a) = (rng.float(3.0, 5.0), rng.int(200, 255) as f32);
            out.push(sprite(AttachPoint::named(name), c, w, Vec3::ONE, a));
        }
    }
    if let Some(b) = if view { m.view_beam } else { m.world_beam } {
        let width = if view { [0.0, 4.0 * U] } else { [0.5 * U, 2.0 * U] };
        for name in tips {
            out.push(AttachedPiece::Beam {
                from: AttachPoint::named(name),
                to: BeamEnd::Attachment(AttachPoint::Index(0)),
                material: b,
                width,
                color: Vec3::ONE,
                alpha: 1.0,
                noise: 16.0,
                segments: 8,
                scroll: 15.0,
            });
        }
    }
    out
}

/// The claws' sprites and beams on every gun (see the module docs): each
/// character's state, then its `AttachedFx`.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn claws(
    mut characters: Query<(
        Entity,
        Option<&Inventory>,
        Option<&Hl2GunEffect>,
        Option<&mut GunLook>,
        Option<&mut AttachedFx>,
        Has<LocalPlayer>,
    )>,
    guns: Query<&Hl2Physcannon>,
    role: Option<Res<NetRole>>,
    materials: Option<Res<ParticleMaterials>>,
    time: Res<Time>,
    mut rng: ResMut<FxRng>,
    mut commands: Commands,
) {
    let Some(mats) = materials.as_ref().and_then(|m| ClawMaterials::new(&m.0)) else { return };
    let client = role.is_some_and(|r| *r == NetRole::Client);
    let now = time.elapsed_secs_f64();
    for (e, inv, net, look, fx, local) in &mut characters {
        let state = if client && !local {
            net.map_or(GunEffect::None, |n| n.0)
        } else {
            inv.and_then(|i| i.active).and_then(|a| guns.get(a).ok()).map_or(GunEffect::None, effect_of)
        };
        let Some(mut look) = look else {
            if state != GunEffect::None {
                let mut l = GunLook::default();
                l.set(state, now);
                commands.entity(e).insert(l);
            }
            continue;
        };
        look.set(state, now);
        let view = claw_pieces(&look, true, &mats, &mut rng.0, now);
        let world = claw_pieces(&look, false, &mats, &mut rng.0, now);
        match fx {
            // Emptied for the frame (`effects::clear_attached`).
            Some(mut fx) => {
                fx.view.extend(view);
                fx.world.extend(world);
            }
            None => {
                commands.entity(e).insert(AttachedFx { view, world });
            }
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
        use chimera_net::net::parts::NetAppExt;
        app.init_resource::<FxRng>()
            .register_type::<Hl2GunEffect>()
            .add_message::<SoundControl>()
            .add_systems(
                FixedUpdate,
                (
                    launches.after(crate::core::SimSet::Weapons),
                    server_effects.after(crate::core::SimSet::Weapons).run_if(crate::core::authoritative),
                ),
            )
            .add_systems(
                Update,
                (
                    hold_sound,
                    ball_sound,
                    claws.in_set(super::effects::AttachedPieces),
                ),
            )
            .net_part("hl2dm_physcannon_fx", register);
    }
}

/// The protocol: each player's gun effect state, from the server.
fn register(app: &mut App) {
    use bevy_replicon::prelude::AppRuleExt;
    use chimera_net::net::ordered::OrderedAppExt;
    app.replicate::<Hl2GunEffect>().ordered::<Hl2GunEffect>();
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

    /// Spec test 48: holding from ready (core 14, alpha 128), 0.1 s on:
    /// width 15, alpha 255; test 49: the blast at 0.05 s and 0.2 s.
    #[test]
    fn the_core_and_the_blast_move_as_the_spec_says() {
        let mut look = GunLook::default();
        look.set(GunEffect::Ready, 0.0);
        assert_eq!(look.core(true, 1.0), (14.0, 128.0));
        assert_eq!(look.core(false, 1.0), (8.0, 0.0));
        look.set(GunEffect::Holding, 1.0);
        let (w, a) = look.core(true, 1.1);
        assert!((w - 15.0).abs() < 1e-3 && (a - 255.0).abs() < 1e-3, "{w} {a}");
        assert_eq!(look.core(true, 2.0), (16.0, 255.0));
        look.set(GunEffect::None, 2.0);
        assert_eq!(look.core(true, 2.5), (0.0, 0.0));
        let (w, a) = blast_at(0.05).unwrap();
        assert!((w - 36.0).abs() < 1e-3 && (a - 191.25).abs() < 1e-3);
        assert_eq!(blast_at(0.2), None);
    }

    #[test]
    fn holding_or_searching_shows_the_beams() {
        let mut gun = Hl2Physcannon::new(weapons::launch::hook_params(weapons::PHYSCANNON, "hl2dm:physcannon"));
        assert_eq!(effect_of(&gun), GunEffect::Ready);
        gun.secondary_was = true;
        assert_eq!(effect_of(&gun), GunEffect::Holding);
        gun.masked = true;
        assert_eq!(effect_of(&gun), GunEffect::Ready);
        gun.holding = true;
        assert_eq!(effect_of(&gun), GunEffect::Holding);
    }

    #[test]
    fn the_pieces_by_state() {
        let m = ClawMaterials { core: 0, blast: Some(1), glow: Some(2), cap: Some(3), view_beam: Some(4), world_beam: Some(5) };
        let mut rng = ParticleRng::new(1);
        let mut look = GunLook::default();
        look.set(GunEffect::Ready, 0.0);
        let count = |p: &[AttachedPiece], mat: usize| {
            p.iter()
                .filter(|x| match x {
                    AttachedPiece::Sprite { material, .. } | AttachedPiece::Beam { material, .. } => *material == mat,
                })
                .count()
        };
        let ready = claw_pieces(&look, true, &m, &mut rng, 1.0);
        assert_eq!((count(&ready, 0), count(&ready, 2), count(&ready, 3), count(&ready, 4)), (1, 6, 0, 0));
        // The world model's ready core is alpha 0: not drawn.
        let ready = claw_pieces(&look, false, &m, &mut rng, 1.0);
        assert_eq!((count(&ready, 0), count(&ready, 2)), (0, 6));
        look.set(GunEffect::Holding, 1.0);
        let held = claw_pieces(&look, true, &m, &mut rng, 1.5);
        assert_eq!((count(&held, 0), count(&held, 3), count(&held, 4)), (1, 2, 2));
        let held = claw_pieces(&look, false, &m, &mut rng, 1.5);
        assert_eq!((count(&held, 3), count(&held, 5)), (3, 3));
        for p in &held {
            if let AttachedPiece::Sprite { material: 2, width, alpha, color, .. } = p {
                assert!((6.4 * U..=9.6 * U).contains(width) && (24.0 / 255.0..=32.0 / 255.0).contains(alpha));
                assert_eq!(*color, GLOW_COLOR);
            }
        }
        look.blast = Some(1.45);
        let punt = claw_pieces(&look, true, &m, &mut rng, 1.5);
        assert_eq!(count(&punt, 1), 1);
        look.set(GunEffect::None, 2.0);
        assert!(claw_pieces(&look, true, &m, &mut rng, 2.0).is_empty());
    }

    #[test]
    fn the_balls_pitch_climbs_in_the_last_second() {
        assert_eq!(ball_pitch(3.0, 8.0), 100.0);
        assert_eq!(ball_pitch(7.0, 8.0), 100.0);
        assert!((ball_pitch(7.5, 8.0) - 125.0).abs() < 1e-4);
        assert_eq!(ball_pitch(8.0, 8.0), 150.0);
    }
}
