//! Thrown grenades for any game (specs/cs_source/grenades.md gives the
//! Counter-Strike rules this follows): a `Throwable` weapon part (a count
//! of grenades, pull the pin on attack, throw on release), the flying
//! `Projectile` (a small box under its own gravity that mirrors its
//! velocity on every contact and keeps part of its speed, stopping dead
//! when slow; a fuse checked every few ticks), and what it does when it
//! goes off (`GrenadeEffect`): a blast (radius damage stopped by the world
//! and static props, partly shielded by props and characters between
//! (`BlastShield`), or through everything (`Blast::through_walls`, CS:S's
//! bomb); a push on loose bodies, a scorch mark), a flash (blinds
//! everyone who sees it: `core::Blinded`) or a smoke cloud (`SmokeCloud`,
//! with a `core::SightBlocker`). Games fill in the numbers and draw the
//! effects from `Detonated`. A spawn-protected character in its bullets
//! window (`core::SpawnProtected::passes_attacks`) isn't there for them:
//! projectiles fly through it, blasts and flashes leave it out.
//!
//! Units: meters, seconds, damage normalized like `Health`. Pitch-based
//! throw rules use degrees with the Source convention (positive looks
//! down).
//!
//! Covers (specs/cs_source/grenades.md): `Throwable` (count and carry limit,
//! pin on press, throw on release after a delay, redraw or the spent weapon
//! goes; predicted with the weapon frame, the projectile launched only where
//! `core::authoritative`), `ThrowRule` (pitch bend and speed), the `Projectile`
//! (box sweep, gravity, mirror bounce with elasticity, stop speed, breakables,
//! water, fuse checked every few ticks), `GrenadeEffect` blast (radius damage
//! traced against the world only, armour, push on loose bodies, a `RagdollShot`
//! blast line for each ragdoll in reach (`RagdollBlast`), scorch decal), flash
//! (`core::Blinded` from a game's `FlashModel`) and smoke (`SmokeCloud` with a
//! `core::SightBlocker`), `Detonated` for games' effects; `core::Explosion`s
//! blast the same way with a game's `ExplosionRule` (the HE blast for CS:S) at
//! their magnitude and radius (a bell's σ scaled with the radius, never
//! linear by `LegacyBlastDamage`; only characters' damage is whole points); a throw says `core::Radio` "fireinhole" (unless
//! `sv_ignoregrenaderadio`); dying primed drops it live; cleared at a round
//! restart; buying stacks grenades up to their limit and keeps the hand.
//! Another game's throw (`ThrowOptions` on the weapon; Combat Arms'):
//! the release when the pin animation ends whatever the button does, no
//! live drop on death, and its own projectile (`ThrowOptions::
//! game_launch`: a `Launch` event on the weapon where the grenade leaves
//! the hand, the server's only, which the game's observer answers for its
//! own weapons with a `Projectile` marked `GameFlown` that it flies and
//! detonates; `Detonated` and the network's drawing still apply). Other
//! projectiles (`projectile`'s flyers, a game's own bodies) ask for a
//! blast with `BlastAt`; `Blast::radius_rule` (`SourceRadius`) is
//! Source's generic radius damage (HL2:DM's: characters' distance to
//! where the line to their eye enters their box, a body between shielding
//! mass/350, the force by each victim's own damage); every blast's
//! damage is told with where it came from (`weapon::Inflicted`).
//!
//! Known shortcuts (docs/tech-debt.md): Grenades (specs/cs_source/grenades.md)
//! follow the SDK template where CS:S is unmeasured: release + 0.1 s to the
//! throw (Q1), redraw of the same type (Q2; the probe's bot held its knife
//! after the throw, but its AI picks that knife itself: a human capture),
//! HE armour by H1 (Q4), the signed pitch bend (Q5), a bounce sound on every contact (Q7, measured
//! 2026-10-17: as CS:S), the 1.5 s
//! fuse (Q8, measured: matches), smoke pops once still after the fuse (Q15),
//! dying primed drops it live (Q17). Measured since (grenades.md "CS:S
//! values"): a switch with the pin out keeps the grenade (Q3); dying
//! primed drops it live, and the dead drop one unthrown grenade
//! (`drop::DeathDrop`, Q17); HE armour (H1), HE falloff (a bell, `blast_damage_bell`, or
//! linear with `sv_legacy_grenade_damage 1`), the flash's strength by
//! distance and facing (`grenades::flash_model`); its hold and fade split
//! is a FIT pending Q13; the blast's shield (`BlastShield`: three lines,
//! their weights) is a FIT to crates and players covering the victim
//! (Q10); a flash's line is stopped whole by the world, static props,
//! prop entities and other living characters (measured, Q12: a crate or a
//! player's hull; loose bodies too, unmeasured), not by glass or grates
//! (`map::SeeThrough`; measured on de_nuke's windows and window-contents
//! vents and a grate catwalk), and a cloud anywhere on the line scales the blindness by one
//! measured factor (`Flash::through_smoke`; CS:S's at one geometry); the
//! after-image is a screenshot
//! read-back of the window at the flash (it holds the HUD, arrives a frame or
//! two late; Q13 for its curve). Hearing (5.5, 6.4): the DSP presets are
//! approximated (`grenades::EXPLOSION_RING`, `SHOCK_MUFFLE`, `FLASH_MUFFLE`:
//! top of the mix range, a diffusor as a 2.5 kHz low-pass, the 3 kHz LFO as an
//! added tone, a fade time of -1 read as a 1 s exponential fade); which flash
//! muffle by blindness (60 %/30 % of the longest) is a hypothesis (Q14), HE
//! uses the base player's rule, not presets 137-139, and rings only players
//! the blast hurts (Q11, capture C31: the server's message isn't
//! observable on the probe). The HE view shake is
//! a guess (2 units, 700-unit radius, 1 s; Q9). Bots' sight through smoke:
//! blocked when more than the blocker's radius (0.75 × the cloud's) of the line
//! is inside (Q16 hypothesis). A breakable hit is assumed broken (×0.4, no
//! bounce; a second contact with it bounces); grenades resting on movers don't
//! ride them; the blast pushes loose bodies (speed capped at 2000 units/s) but
//! not players; HE trace sees static world colliders (not movers); the
//! explosion's glow size is ours; others see a thrown grenade at once (no 0.5 s
//! hiding); in flight it's drawn with the held world model (the `_thrown`
//! models share no bone with the player skeleton) and moves per tick (no
//! interpolation). Why it's OK for now: Each is a listed open question with its
//! probe check; the measured parts (prices, limits, view-model times) are
//! exact.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{CHAN_WEAPON, HitArmor, Inventory, Weapon, WeaponEvent, WeaponEventKind, WeaponState};
use crate::{
    core::{
        Blinded, Damage, DamageKind, Damageable, Deafened, Died, Explosion, FirstTimePredicted, Health, HearingEffect,
        Hitgroup, Intent, MapWater, MovementState, NetRole, Radio, RoundRestarts, SOLID_LAYERS, SightBlocker, SimClock,
        Velocity,
    },
    map::{
        DecalGroup, MapPropCollider, ParticleRng, PlaceDecal, PlaySound,
        interp::NetDrawn,
        loose::ShownItem,
    },
};

/// Source's voice channel (grenade bounces).
const CHAN_VOICE: u8 = 2;
/// Timers due on this tick fire on it (see `TIME_SLACK` in the frame).
const TIME_SLACK: f64 = 1e-5;
/// Gap kept between the box and what it touched, m (so a resting or
/// sliding box doesn't start its next sweep inside the surface).
const SKIN: f32 = 1e-3;

pub(super) fn plugin(app: &mut App) {
    use super::CarriedAppExt;
    app.predicted_part::<Throwable>()
        .add_message::<Detonated>()
        .add_message::<BlastAt>()
        .add_message::<super::Inflicted>()
        .add_message::<PlaceDecal>()
        .add_message::<Radio>()
        .init_resource::<GrenadeRng>()
        .init_resource::<GrenadeRadio>()
        // The throw is predicted with the weapon frame (`core::Predict`):
        // the pin, the release and the redraw; the projectile is the
        // server's.
        .add_systems(
            crate::core::Predict::Weapons,
            throw_frame.after(super::WeaponFrame),
        )
        .add_systems(
            FixedUpdate,
            (
                (fly, explosions, smoke_clouds)
                    .chain()
                    .after(super::WeaponFrame)
                    .in_set(crate::core::SimSet::Weapons),
                blast_requests
                    .after(super::projectile::FlyerSet::Touch)
                    .after(explosions)
                    // Only projectiles' blasts (none in CS:S or Combat
                    // Arms): unordered against their systems on purpose.
                    .ambiguous_with_all()
                    .in_set(crate::core::SimSet::Weapons),
                drop_primed
                    .after(crate::core::apply_damage)
                    .before(super::drop::drop_on_death),
                clear_on_restart
                    .after(crate::core::SimSet::Rules)
                    .before(crate::core::SimSet::Movement),
            )
                .run_if(crate::core::authoritative),
        );
}

#[derive(Resource)]
struct GrenadeRng(ParticleRng);

/// `sv_ignoregrenaderadio`: 1 stops the "Fire in the hole!" radio call
/// on a throw (spec 2).
#[derive(Resource, Default)]
pub struct GrenadeRadio(pub u8);

impl Default for GrenadeRng {
    fn default() -> Self {
        Self(ParticleRng::new(0x6E_4ADE))
    }
}

// ---------------------------------------------------------------------------
// Parts

/// How the throw direction and speed follow the view (spec 3): the pitch
/// is bent `p' = lift + p x scale` (degrees, positive down; `scale_up`
/// while looking up, `scale` looking down), the speed is
/// `min(max, per_deg x (90 - p'))`, the projectile starts `forward` ahead
/// of the eye and takes the thrower's velocity on top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThrowRule {
    pub pitch_lift: f32,
    pub pitch_scale: f32,
    /// The scale while looking up (`p` < 0).
    pub pitch_scale_up: f32,
    /// m/s per degree.
    pub speed_per_deg: f32,
    pub speed_max: f32,
    pub forward: f32,
}

impl ThrowRule {
    /// The bent pitch for a view pitch `p` (degrees, positive down).
    pub fn pitch(&self, p: f32) -> f32 {
        let scale = if p < 0.0 { self.pitch_scale_up } else { self.pitch_scale };
        self.pitch_lift + p * scale
    }

    /// Speed for a bent pitch, m/s.
    pub fn speed(&self, bent: f32) -> f32 {
        (self.speed_per_deg * (90.0 - bent)).min(self.speed_max)
    }

    /// (start, velocity) of a throw from `eye` looking at `yaw` (radians,
    /// ours) and `pitch` (radians, ours: positive up), by a thrower moving
    /// at `carried`.
    pub fn throw(&self, eye: Vec3, yaw: f32, pitch: f32, carried: Vec3) -> (Vec3, Vec3) {
        let bent = self.pitch(-pitch.to_degrees());
        let dir = Quat::from_euler(EulerRot::YXZ, yaw, -bent.to_radians(), 0.0) * Vec3::NEG_Z;
        (eye + dir * self.forward, dir * self.speed(bent) + carried)
    }
}

/// How a projectile flies and bounces (spec 4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    /// Half the side of its collision box, m.
    pub half: f32,
    /// Downward acceleration, m/s².
    pub gravity: f32,
    /// Speed kept per contact; times `player_elasticity` against
    /// characters; the product is capped at `elasticity_cap`.
    pub elasticity: f32,
    pub player_elasticity: f32,
    pub elasticity_cap: f32,
    /// A contact leaving it slower than this stops it, m/s.
    pub stop_speed: f32,
    /// Contacts with a normal's up part above this are floors.
    pub floor_normal: f32,
    /// Damage dealt to breakables it hits; its velocity after breaking
    /// through (factor).
    pub breakable_damage: f32,
    pub breakable_slowdown: f32,
    /// Velocity factor per check while in water; vertical velocity factor
    /// on entering water.
    pub water_slowdown: f32,
    pub water_entry: f32,
    /// Each velocity component is clamped to this, m/s.
    pub max_velocity: f32,
    /// Seconds between fuse checks (rounded to whole ticks).
    pub check_interval: f32,
    /// Initial spin: roll rate, and the pitch rate's range (radians/s).
    pub spin_roll: f32,
    pub spin_pitch: f32,
}

/// Mirror `v` about the surface with normal `n` and keep part of its speed
/// (spec 4, contact resolution). `player`: the surface is a character.
/// Returns the new velocity and whether it comes to rest (a floor contact
/// slower than the stop speed). A wall contact slower than the stop speed
/// leaves it with no velocity, to fall from there.
pub fn bounce(v: Vec3, n: Vec3, player: bool, f: &Flight, snap: f32) -> (Vec3, bool) {
    let e = (f.elasticity * if player { f.player_elasticity } else { 1.0 }).clamp(0.0, f.elasticity_cap);
    let mut r = v - 2.0 * v.dot(n) * n;
    for c in [&mut r.x, &mut r.y, &mut r.z] {
        if c.abs() < snap {
            *c = 0.0;
        }
    }
    r *= e;
    let slow = r.length() < f.stop_speed;
    if n.y > f.floor_normal {
        if slow { (Vec3::ZERO, true) } else { (r, false) }
    } else if slow {
        (Vec3::ZERO, false)
    } else {
        (r, false)
    }
}

/// The parabola step of one tick (spec 4 step 4): the move and the new
/// velocity.
pub fn gravity_step(v: Vec3, gravity: f32, dt: f32) -> (Vec3, Vec3) {
    let vy = v.y - gravity * dt;
    (
        Vec3::new(v.x * dt, (v.y + vy) / 2.0 * dt, v.z * dt),
        Vec3::new(v.x, vy, v.z),
    )
}

/// Radius damage (spec 5.3): `damage` at the centre, falling linearly to
/// zero at `radius`.
pub fn blast_damage(damage: f32, radius: f32, distance: f32) -> f32 {
    (damage - distance * damage / radius).max(0.0)
}

/// Radius damage as a bell: `damage` × exp(−d² / 2σ²). No cut-off of its
/// own: what a blast reaches is its bounds test (bounds touching the
/// sphere of its radius), and a target there whose point is beyond the
/// radius still takes the bell's tail (CS:S, measured 2026-10-17: an
/// env_explosion of radius 100 does 0.555 at 107.4 units;
/// specs/source/prop_damage.md Q3).
pub fn blast_damage_bell(damage: f32, sigma: f32, distance: f32) -> f32 {
    damage * (-(distance * distance) / (2.0 * sigma * sigma)).exp()
}

/// Blasts with a bell falloff (`Blast::bell_sigma`) fall linearly instead
/// (CS:S's `sv_legacy_grenade_damage 1`, the original game's rule).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct LegacyBlastDamage(pub u8);

/// A blast (the HE grenade, spec 5).
#[derive(Clone, Debug)]
pub struct Blast {
    pub damage: f32,
    pub radius: f32,
    /// Falloff: a bell of this σ (`blast_damage_bell`; linear instead
    /// when `legacy` and `LegacyBlastDamage`); None: linear to `radius`.
    pub bell_sigma: Option<f32>,
    /// `LegacyBlastDamage` turns the bell linear (CS:S's grenades; map
    /// explosions and exploding props keep the bell, measured 2026-10-17,
    /// specs/source/prop_damage.md Q3).
    pub legacy: bool,
    /// As `DamageEffect::armor_ratio`; blasts hit no hitgroup, so armour
    /// without a helmet covers them.
    pub armor_ratio: Option<f32>,
    pub quantum: f32,
    /// The ground under the blast: probed from this far above to this far
    /// below the grenade, m; the blast point is pulled out of the surface
    /// by `pull_out`.
    pub probe_up: f32,
    pub probe_down: f32,
    pub pull_out: f32,
    /// Damage is traced from this far above the blast point.
    pub src_lift: f32,
    /// Characters are aimed at between these fractions of their eye
    /// height above the feet.
    pub body_target: (f32, f32),
    /// Push on loose bodies, kg·m/s, times a random factor in `jitter`.
    pub force: f32,
    pub force_jitter: (f32, f32),
    /// A pushed body gets at most this speed from it, m/s.
    pub max_push_speed: f32,
    /// The push on ragdolls (the client's explosion effect), if any.
    pub ragdolls: Option<RagdollBlast>,
    /// Decal group placed on the probed ground.
    pub scorch: Option<String>,
    pub sound: Option<String>,
    /// What it does to the hearing of characters it hurts.
    pub hearing: Option<BlastHearing>,
    /// How it shakes the view of those near it (clients draw it).
    pub shake: Option<Shake>,
    /// How much of it props and characters between shield a character.
    pub shield: Option<BlastShield>,
    /// Reaches everyone in range whatever is between (no traces: CS:S's
    /// bomb, measured 2026-10-18).
    pub through_walls: bool,
    /// Source's generic radius damage instead of the rules above for
    /// characters' distance, shielding and the push (HL2:DM's every
    /// explosion, specs/hl2dm/projectiles.md 2): `SourceRadius`.
    pub radius_rule: Option<SourceRadius>,
}

/// Source's generic radius damage (HL2's, specs/hl2dm/projectiles.md 2):
/// a character is aimed at its eye and its distance is to where that line
/// enters its box (0 from inside); the first other body on the line (not
/// the world, which blocks whole) shields a share and the distance is to
/// it: a physics body `mass / shield_mass` of the damage (all at or above
/// it; living characters count `character_mass`), anything else
/// `shield_other`; the push on loose bodies and each victim's force is
/// `per_damage` × its own decayed damage (normalized), at most `force_cap`
/// (kg·m/s). Linear falloff (`Blast::bell_sigma` None).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourceRadius {
    pub shield_mass: f32,
    pub shield_other: f32,
    pub character_mass: f32,
    pub per_damage: f32,
    pub force_cap: f32,
    /// Under water when the blast point and the point this far above it
    /// (m) are in water: it then reaches only those with their feet in
    /// water, and a blast not under water only those not wholly under. 0:
    /// no such rule.
    pub water_lift: f32,
    /// Map classes that block it whole (doors), wherever on the line.
    pub stoppers: &'static [&'static str],
}

/// A blast a game's projectile asks for (lift C1: `projectile`): `blast`
/// at `at` (its ground probe, its damage and push, its scorch, sound and
/// shake, `Detonated` for the effects), dealt by `thrower` with `weapon`
/// (kill notices), leaving out `inflictor`. The server's, applied in
/// `SimSet::Weapons` after the flyers move (`projectile::FlyerSet`).
#[derive(Message, Clone, Debug)]
pub struct BlastAt {
    pub at: Vec3,
    pub blast: std::sync::Arc<Blast>,
    pub thrower: Option<Entity>,
    pub weapon: &'static str,
    pub inflictor: Option<Entity>,
}

/// Props and characters between a blast and a character shield part of the
/// blast (CS:S, measured 2026-10-18, grenades.md Q10; the world and static
/// props block it whole). Lines from the blast go to three points of the
/// character: its eye (`head` of the weight) and its hull's two sides,
/// across the line, at half its eye height (the rest, split). Each line
/// crossing a prop entity or a moving body takes `1 − prop` of its weight
/// off the damage and the push, one crossing a character `1 − character`:
/// a character hidden behind a crate keeps `prop` of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlastShield {
    pub prop: f32,
    pub character: f32,
    pub head: f32,
}

/// A blast's push on ragdolls (specs/cs_source/grenades.md 5.3, the
/// explosion effect's rule): each ragdoll within the radius gets a blast
/// line (`map::RagdollShot`, pushed by 4000 × its length, ragdolls.md 6.2)
/// from `drop` below the blast towards its root body, of length
/// `length · (1 − d/radius)` for its distance `d` from the blast; none
/// when that is `min` or less. Meters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RagdollBlast {
    pub length: f32,
    pub min: f32,
    pub drop: f32,
}

impl RagdollBlast {
    /// The blast line's length at `distance` from a blast of `radius`.
    pub fn length_at(&self, radius: f32, distance: f32) -> Option<f32> {
        let l = self.length - self.length / radius * distance;
        (l > self.min).then_some(l)
    }
}

/// What a blast does to the hearing of a character it hurts (Source's
/// base player, specs/cs_source/grenades.md 5.5): `shock` when the damage
/// is at least `shock_damage`, else `ring` when the character's origin
/// is closer than `ring_distance` (m) to the blast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlastHearing {
    pub shock_damage: f32,
    pub shock: HearingEffect,
    pub ring_distance: f32,
    pub ring: HearingEffect,
}

impl BlastHearing {
    /// The effect on a character hurt by `damage` (before armour) with its
    /// origin `distance` (m) from the blast.
    pub fn effect(&self, damage: f32, distance: f32) -> Option<HearingEffect> {
        if damage >= self.shock_damage {
            Some(self.shock)
        } else if distance < self.ring_distance {
            Some(self.ring)
        } else {
            None
        }
    }
}

/// A screen shake around a point (Source's env_shake shape): the view
/// moves up to `amplitude` (m) at the centre, less further out and none
/// beyond `radius` (m), changing direction `frequency` times a second,
/// dying out linearly over `duration` seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shake {
    pub amplitude: f32,
    pub frequency: f32,
    pub duration: f32,
    pub radius: f32,
}

impl Shake {
    /// The amplitude `distance` (m) from the centre, `t` seconds in.
    pub fn amplitude_at(&self, distance: f32, t: f32) -> f32 {
        if distance >= self.radius || t < 0.0 || t >= self.duration {
            return 0.0;
        }
        self.amplitude * (1.0 - distance / self.radius) * (1.0 - t / self.duration)
    }
}

/// How strongly a flash blinds someone (a game's model): from the distance
/// to the grenade (m) and the facing `k` (the cosine between the view and
/// the direction to it): (peak whiteness 0-1, hold seconds, fade seconds),
/// or None when it doesn't blind.
pub type FlashModel = fn(distance: f32, facing: f32) -> Option<(f32, f32, f32)>;

#[derive(Clone, Debug)]
pub struct Flash {
    pub model: FlashModel,
    pub sound: Option<String>,
    /// What it does to the hearing of those it blinds.
    pub hearing: Option<FlashHearing>,
    /// The blindness's length is scaled by this when the line from the
    /// flash to the eye passes through a smoke cloud's `SightBlocker`.
    pub through_smoke: f32,
}

/// The hearing effect of a blindness (a game's model): from the peak
/// whiteness (0-1) and how long it lasts (seconds, hold + fade).
pub type FlashHearing = fn(alpha: f32, seconds: f32) -> Option<HearingEffect>;

/// A smoke cloud (spec 7): grows to `max_radius` over `expand_time`, holds,
/// fades between `fade_start` and `fade_end` (seconds after it starts).
#[derive(Clone, Debug)]
pub struct Smoke {
    pub max_radius: f32,
    pub expand_time: f32,
    pub fade_start: f32,
    pub fade_end: f32,
    /// It pops only once the grenade moves slower than this (m/s).
    pub pop_speed: f32,
    /// The sight blocker's radius as a fraction of the cloud's.
    pub sight_radius: f32,
    pub sound: Option<String>,
}

#[derive(Clone, Debug)]
pub enum GrenadeEffect {
    Blast(Blast),
    Flash(Flash),
    Smoke(Smoke),
}

impl GrenadeEffect {
    pub fn kind(&self) -> GrenadeKind {
        match self {
            GrenadeEffect::Blast(_) => GrenadeKind::Blast,
            GrenadeEffect::Flash(_) => GrenadeKind::Flash,
            GrenadeEffect::Smoke(_) => GrenadeKind::Smoke,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrenadeKind {
    Blast,
    Flash,
    Smoke,
}

/// Weapon part: grenades thrown one at a time (spec 2). Primary pulls the
/// pin; releasing it throws `throw_delay` later; the weapon is drawn again
/// after the throw, or goes when the last one is thrown.
#[derive(Component, Clone, Debug)]
pub struct Throwable {
    /// Grenades held, and the most that can be carried.
    pub count: u32,
    pub max: u32,
    /// Seconds: the pin-pull animation (no throw before... it may be cut
    /// short by a release), release to the projectile, the throw
    /// animation, the redraw.
    pub pin_time: f32,
    pub throw_delay: f32,
    pub throw_time: f32,
    pub redraw_time: f32,
    /// Fuse, seconds from the projectile's start.
    pub fuse: f32,
    pub throw: ThrowRule,
    pub flight: Flight,
    pub effect: GrenadeEffect,
    /// (seconds after the pin pull, entry): the view model's sounds.
    pub pin_sounds: Vec<(f32, String)>,
    pub bounce_sound: Option<String>,
    /// (entering, leaving): the entries played where it crosses the water's
    /// surface (spec 4 step 8).
    pub water_sounds: Option<(String, String)>,
    /// The pin is out, waiting for the release.
    pub pin: bool,
    /// When the released grenade leaves the hand.
    pub throw_at: Option<f64>,
    /// When the throw animation is over (redraw, or the weapon goes).
    pub redraw_at: Option<f64>,
}

impl Throwable {
    /// In the hand with the pin out, or released and not yet thrown.
    pub fn primed(&self) -> bool {
        self.pin || self.throw_at.is_some()
    }
}

/// On a throwable weapon: the last grenade thrown, it stays in the
/// inventory at count 0 (Combat Arms) instead of going; the game's own
/// selection puts it away.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct KeepSpent;

/// Weapon part beside `Throwable`: how a game's throw differs from the
/// default (CS:S's, which a weapon without this part keeps).
#[derive(Component, Clone, Copy, Debug)]
pub struct ThrowOptions {
    /// The release comes when the pin animation ends, whether the button
    /// is still held or not (a release before then waits for it); off:
    /// releasing the button releases the grenade.
    pub release_at_pin_end: bool,
    /// With `release_at_pin_end`: after the pin animation a throw
    /// animation of this length (s) plays, and plays again from its
    /// start for as long as the button is held when it ends; the release
    /// comes at the end of the play running when the button is up
    /// (Combat Arms: the grenade stays in hand while fire is held).
    pub hold_loop: Option<f32>,
    /// Dying with the pin out drops the grenade live (`drop_primed`).
    pub drop_primed: bool,
    /// The game makes the projectile itself when the grenade leaves the
    /// hand (where the default would `launch` one by `ThrowRule`): a
    /// `Launch` event on the weapon, which the game observes.
    pub game_launch: bool,
}

impl Default for ThrowOptions {
    /// The default throw's options.
    fn default() -> Self {
        Self {
            release_at_pin_end: false,
            hold_loop: None,
            drop_primed: true,
            game_launch: false,
        }
    }
}

/// A grenade leaving the hand of a weapon with `ThrowOptions::
/// game_launch`, an event on the weapon: its thrower, the thrower's eye
/// (m), view (radians, ours: `Intent`'s yaw and pitch), velocity and
/// command number (a seed). Triggered once per throw where projectiles are
/// made (the server, single player; never in a client's prediction), from
/// the throw's commands, so the observers run where a command would (in
/// order with the throw's other commands). A game's observer makes the
/// projectile (`spawn_projectile`, then its own parts) for weapons with
/// its own part and leaves the others alone.
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct Launch {
    #[event_target]
    pub weapon: Entity,
    pub thrower: Entity,
    pub eye: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub velocity: Vec3,
    pub command: u32,
}

/// On a `Projectile` a game flies, bounces and detonates itself (its own
/// rules): the default flight leaves it alone. The rest (drawing, the
/// network's `NetItem`, a round restart clearing it) still applies.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct GameFlown;

/// A grenade in flight (or lying, waiting to go off).
#[derive(Component, Clone, Debug)]
pub struct Projectile {
    pub velocity: Vec3,
    pub thrower: Option<Entity>,
    /// The weapon ID it came from (kill notices, drawing).
    pub weapon: &'static str,
    pub flight: Flight,
    pub fuse: f32,
    pub effect: GrenadeEffect,
    pub bounce_sound: Option<String>,
    /// As `Throwable::water_sounds`.
    pub water_sounds: Option<(String, String)>,
    /// Ticks simulated so far (the spawn tick is 0).
    pub ticks: u32,
    /// At rest on a floor.
    pub resting: bool,
    /// Radians/s about its own axes.
    pub spin: Vec3,
    /// The breakable it broke into last (a second contact bounces).
    pub breakable: Option<Entity>,
    in_water: bool,
}

/// A smoke cloud: where, when it started, and its rule. Carries a
/// `core::SightBlocker` that grows with it.
#[derive(Component, Clone, Debug)]
pub struct SmokeCloud {
    pub centre: Vec3,
    pub started: f64,
    pub smoke: Smoke,
    /// The spent grenade lying in it (removed with the cloud).
    pub grenade: Option<Entity>,
    /// The grenade's weapon id (a network client builds its cloud from
    /// it: the spent grenade is no longer a `Projectile`).
    pub weapon: Option<&'static str>,
}

impl SmokeCloud {
    /// Radius at `t` seconds (spec 7.2: `E(t)`).
    pub fn radius(&self, t: f32) -> f32 {
        smoke_radius(self.smoke.max_radius, self.smoke.expand_time, t)
    }

    /// 1 while holding, falling to 0 by the end (spec 7.2).
    pub fn fade(&self, t: f32) -> f32 {
        smoke_fade(self.smoke.fade_start, self.smoke.fade_end, t)
    }

    /// The cloud's overall alpha: fade × growth.
    pub fn alpha(&self, t: f32) -> f32 {
        self.fade(t) * self.radius(t) / self.smoke.max_radius.max(1e-6)
    }
}

/// `E(t) = max · sin(π/2 · min(t / expand, 1))`.
pub fn smoke_radius(max: f32, expand: f32, t: f32) -> f32 {
    max * (std::f32::consts::FRAC_PI_2 * (t / expand.max(1e-6)).clamp(0.0, 1.0)).sin()
}

/// Cosine fade between `start` and `end`.
pub fn smoke_fade(start: f32, end: f32, t: f32) -> f32 {
    if t < start {
        1.0
    } else if t >= end {
        0.0
    } else {
        0.5 + 0.5 * (std::f32::consts::PI * (t - start) / (end - start)).cos()
    }
}

/// The grey a camera inside a cloud sees (spec 7.4): what one cloud of
/// radius `e` and alpha `alpha` adds at distance `d` from its centre.
pub fn smoke_fog(d: f32, e: f32, alpha: f32) -> f32 {
    if e <= 0.0 {
        0.0
    } else if d < 0.3 * e {
        alpha
    } else if d < e {
        (1.0 - (d - 0.3 * e) / (0.7 * e)) * alpha
    } else {
        0.0
    }
}

/// The grey overlay's alpha at `camera` from every cloud (clamped to 1).
pub fn smoke_fog_at<'a>(camera: Vec3, now: f64, clouds: impl Iterator<Item = &'a SmokeCloud>) -> f32 {
    clouds
        .map(|c| {
            let t = (now - c.started) as f32;
            smoke_fog(camera.distance(c.centre), c.radius(t), c.alpha(t))
        })
        .sum::<f32>()
        .clamp(0.0, 1.0)
}

/// A grenade went off: games draw it.
#[derive(Message, Clone, Debug)]
pub struct Detonated {
    pub kind: GrenadeKind,
    pub weapon: &'static str,
    pub thrower: Option<Entity>,
    /// The blast point.
    pub at: Vec3,
    /// What the ground probe hit under it: point, normal, body.
    pub ground: Option<(Vec3, Vec3, Entity)>,
    /// The projectile (still alive for smoke: it lies in the cloud).
    pub projectile: Entity,
    /// The view shake around `at`, if any.
    pub shake: Option<Shake>,
    /// How it is drawn and heard.
    pub flags: crate::core::BlastFlags,
}

// ---------------------------------------------------------------------------
// Throwing

/// Start a projectile from `weapon`'s rules.
fn launch(
    commands: &mut Commands,
    rng: &mut ParticleRng,
    weapon: &Weapon,
    t: &Throwable,
    thrower: Option<Entity>,
    start: Vec3,
    velocity: Vec3,
) -> Entity {
    let spin = Vec3::new(
        rng.float(-t.flight.spin_pitch, t.flight.spin_pitch),
        0.0,
        t.flight.spin_roll,
    );
    commands
        .spawn((
            Name::new(format!("Grenade {}", weapon.id)),
            Transform::from_translation(start),
            Projectile {
                velocity,
                thrower,
                weapon: weapon.id,
                flight: t.flight,
                fuse: t.fuse,
                effect: t.effect.clone(),
                bounce_sound: t.bounce_sound.clone(),
                water_sounds: t.water_sounds.clone(),
                ticks: 0,
                resting: false,
                spin,
                breakable: None,
                in_water: false,
            },
            ShownItem(weapon.id.to_string()),
        ))
        .id()
}

/// Start a projectile of `weapon`'s kind (a `Throwable`) at `start` with
/// `velocity`, thrown by its owner, without using up a grenade (tests,
/// tools).
pub fn spawn_projectile(world: &mut World, weapon: Entity, start: Vec3, velocity: Vec3) -> Option<Entity> {
    let w = world.get::<Weapon>(weapon)?.clone();
    let t = world.get::<Throwable>(weapon)?.clone();
    let mut rng = ParticleRng::new(weapon.to_bits());
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let e = {
        let mut commands = Commands::new(&mut queue, world);
        launch(&mut commands, &mut rng, &w, &t, w.owner, start, velocity)
    };
    queue.apply(world);
    Some(e)
}

#[allow(clippy::type_complexity)]
fn throw_frame(
    mut owners: Query<(
        Entity,
        &Intent,
        &mut Inventory,
        &Transform,
        &MovementState,
        Option<&Velocity>,
        Option<&Health>,
    ), Without<NetDrawn>>,
    mut weapons: Query<(Entity, &Weapon, &mut WeaponState, &mut Throwable, Option<&ThrowOptions>)>,
    mut events: MessageWriter<WeaponEvent>,
    mut rng: ResMut<GrenadeRng>,
    (mut radio, radio_off): (MessageWriter<Radio>, Res<GrenadeRadio>),
    mut commands: Commands,
    (clock, first, role): (Res<SimClock>, Res<FirstTimePredicted>, Option<Res<NetRole>>),
    spatial: SpatialQuery,
) {
    // The command's time (as the weapon frame); the projectile and the
    // radio call are the server's (a client predicts only the hand).
    let now = clock.now;
    let due = now + TIME_SLACK;
    let replay = !first.0;
    let server = role.as_deref().is_none_or(|r| *r != NetRole::Client);
    for (entity, weapon, mut st, mut t, options) in &mut weapons {
        let Some(owner) = weapon.owner else { continue };
        let Ok((_, intent, mut inv, transform, state, vel, health)) = owners.get_mut(owner) else {
            continue;
        };
        let options = options.copied().unwrap_or_default();
        let active = inv.active == Some(entity);
        let alive = health.is_none_or(|h| h.current > 0.0);
        if !active && t.pin {
            // Switched away with the pin out: the throw is cancelled and
            // the grenade kept (spec 2, Q3).
            t.pin = false;
            st.pending.clear();
        }
        // 2. Released: throw (a release is checked first, so a tap throws
        // at once; or, with `release_at_pin_end`, when the pin's
        // animation ends, held or not).
        let released = if options.release_at_pin_end {
            let play_over = due >= st.next_primary;
            match options.hold_loop {
                // Held when a play ends: it plays again.
                Some(play) if play_over && intent.fire && active && alive && t.pin => {
                    st.next_primary = now + play as f64;
                    false
                }
                _ => play_over,
            }
        } else {
            !intent.fire
        };
        if active && alive && t.pin && released {
            t.pin = false;
            t.throw_at = Some(now + t.throw_delay as f64);
            t.count = t.count.saturating_sub(1);
            t.redraw_at = Some(now + t.throw_time as f64);
            st.next_primary = now + t.throw_time as f64;
            st.next_secondary = st.next_primary;
            // The throw animation cuts the pin's.
            st.pending.clear();
            events.write(WeaponEvent {
                owner,
                weapon: entity,
                kind: WeaponEventKind::Thrown,
                replay,
            });
            // The thrower's team hears "Fire in the hole!" (spec 2).
            if radio_off.0 == 0 && server {
                radio.write(Radio {
                    sender: owner,
                    command: "fireinhole".into(),
                });
            }
        }
        // 3. The grenade leaves the hand (strictly after its time).
        if let Some(at) = t.throw_at
            && now > at + 1e-9
        {
            t.throw_at = None;
            let eye = transform.translation + state.eye_offset;
            let carried = vel.map_or(Vec3::ZERO, |v| v.0);
            if options.game_launch {
                if server {
                    let launch = Launch {
                        weapon: entity,
                        thrower: owner,
                        eye,
                        yaw: intent.yaw,
                        pitch: intent.pitch,
                        velocity: carried,
                        command: intent.command,
                    };
                    commands.trigger(launch);
                }
            } else {
                let (mut start, velocity) = t.throw.throw(eye, intent.yaw, intent.pitch, carried);
                // Thrown against a wall, it starts where its box meets the
                // wall on the way out from the eye, never inside or beyond
                // (CS:S, measured 2026-10-18, grenades.md Q6: 4 units off a
                // panel at 45°, a 2-unit box's corner touching).
                if let Ok(dir) = Dir3::new(start - eye) {
                    let half = t.flight.half;
                    let config = ShapeCastConfig {
                        max_distance: (start - eye).length(),
                        ..ShapeCastConfig::DEFAULT
                    };
                    let filter = SpatialQueryFilter::from_excluded_entities([owner]).with_mask(SOLID_LAYERS);
                    if let Some(hit) = spatial.cast_shape(
                        &Collider::cuboid(half * 2.0, half * 2.0, half * 2.0),
                        eye,
                        Quat::IDENTITY,
                        dir,
                        &config,
                        &filter,
                    ) {
                        start = eye + *dir * hit.distance;
                    }
                }
                if server {
                    launch(&mut commands, &mut rng.0, weapon, &t, Some(owner), start, velocity);
                }
            }
        }
        // 4. The throw animation is over: draw the next one, or the weapon
        // goes with its last grenade.
        if let Some(at) = t.redraw_at
            && t.throw_at.is_none()
            && (due >= at || !active)
        {
            t.redraw_at = None;
            if t.count == 0 {
                commands.queue(move |w: &mut World| exhaust(w, owner, entity));
                continue;
            }
            if active {
                let ready = now + t.redraw_time as f64;
                inv.next_attack = ready;
                st.next_primary = ready;
                st.next_secondary = ready;
                events.write(WeaponEvent {
                    owner,
                    weapon: entity,
                    kind: WeaponEventKind::Deployed,
                    replay,
                });
            }
        }
        // 1. Pressed: pull the pin.
        if active
            && alive
            && intent.fire
            && !t.primed()
            && t.redraw_at.is_none()
            && t.count > 0
            && due >= inv.next_attack
            && due >= st.next_primary
        {
            t.pin = true;
            st.next_primary = now + (t.pin_time + options.hold_loop.unwrap_or(0.0)) as f64;
            st.pending = t.pin_sounds.iter().map(|(s, e)| (now + *s as f64, e.clone())).collect();
            events.write(WeaponEvent {
                owner,
                weapon: entity,
                kind: WeaponEventKind::PinPulled,
                replay,
            });
        }
    }
}

/// The last grenade is gone: the weapon leaves the inventory, and its
/// owner draws the best of the rest (the lowest slot); with `KeepSpent`
/// it stays where it is.
fn exhaust(world: &mut World, owner: Entity, weapon: Entity) {
    if world.get::<KeepSpent>(weapon).is_some() {
        return;
    }
    let next = world.get::<Inventory>(owner).and_then(|inv| {
        inv.weapons
            .iter()
            .copied()
            .filter(|w| *w != weapon)
            .min_by_key(|e| world.get::<Weapon>(*e).map_or(u8::MAX, |w| w.slot))
    });
    if let Some(mut inv) = world.get_mut::<Inventory>(owner) {
        inv.weapons.retain(|w| *w != weapon);
        if inv.last == Some(weapon) {
            inv.last = None;
        }
        if inv.active == Some(weapon) {
            inv.active = None;
            if inv.wanted.is_none_or(|w| w == weapon) {
                inv.wanted = next;
            }
        }
    }
    if let Ok(e) = world.get_entity_mut(weapon) {
        e.despawn();
    }
}

/// Dying with the pin out drops the grenade live where the eye was, with
/// only the body's velocity (spec 9; measured standing still, grenades.md
/// Q17: 16 units ahead of the eye, no velocity); one released but not
/// yet thrown still goes (its throw is pending).
fn drop_primed(
    mut died: MessageReader<Died>,
    owners: Query<(&Inventory, &Transform, &MovementState, &Intent, Option<&Velocity>)>,
    mut weapons: Query<(&Weapon, &mut Throwable, Option<&ThrowOptions>)>,
    mut rng: ResMut<GrenadeRng>,
    mut commands: Commands,
) {
    for d in died.read() {
        let Ok((inv, transform, state, intent, vel)) = owners.get(d.entity) else {
            continue;
        };
        let Some(active) = inv.active else { continue };
        let Ok((weapon, mut t, options)) = weapons.get_mut(active) else {
            continue;
        };
        let options = options.copied().unwrap_or_default();
        if !t.pin || !options.drop_primed {
            continue;
        }
        t.pin = false;
        t.count = t.count.saturating_sub(1);
        let eye = transform.translation + state.eye_offset;
        let start = eye + intent.look_rotation() * Vec3::NEG_Z * t.throw.forward;
        let velocity = vel.map_or(Vec3::ZERO, |v| v.0);
        launch(&mut commands, &mut rng.0, weapon, &t, Some(d.entity), start, velocity);
        if t.count == 0 {
            let owner = d.entity;
            commands.queue(move |w: &mut World| exhaust(w, owner, active));
        }
    }
}

// ---------------------------------------------------------------------------
// Flight

#[derive(bevy::ecs::query::QueryData)]
struct Body {
    entity: Entity,
    transform: &'static Transform,
    aabb: Option<&'static ColliderAabb>,
    health: Option<&'static Health>,
    damageable: Option<&'static Damageable>,
    intent: Option<&'static Intent>,
    state: Option<&'static MovementState>,
}

/// The world access grenades need.
#[derive(bevy::ecs::system::SystemParam)]
struct GrenadeWorld<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    legacy: Option<Res<'w, LegacyBlastDamage>>,
    bodies: Query<'w, 's, Body, Without<Projectile>>,
    colliders: Query<'w, 's, &'static ColliderOf>,
    rigid: Query<'w, 's, (&'static RigidBody, Option<&'static MapPropCollider>)>,
    prop_entities: Query<'w, 's, (), With<crate::map::PropEntity>>,
    see_through: Query<'w, 's, (), With<crate::map::SeeThrough>>,
    dynamic: Query<
        'w,
        's,
        (
            Entity,
            &'static RigidBody,
            &'static Position,
            Option<&'static ComputedMass>,
            Forces,
        ),
        // Players' physics shadows follow their players, not blasts;
        // ragdolls get their own push (`RagdollBlast`).
        (
            Without<crate::map::prop_physics::PhysicsShadow>,
            Without<crate::map::RagdollBody>,
        ),
    >,
    ragdolls: Query<'w, 's, (Entity, &'static crate::map::Ragdoll)>,
    ragdoll_bodies: Query<'w, 's, &'static Position, With<crate::map::RagdollBody>>,
    ragdoll_shots: MessageWriter<'w, crate::map::RagdollShot>,
    armor: HitArmor<'w, 's>,
    blinded: Query<'w, 's, &'static Blinded>,
    blockers: Query<'w, 's, &'static SightBlocker>,
    water: Option<Res<'w, MapWater>>,
    map_nodes: Query<'w, 's, (Option<&'static crate::map::MapBrushEntity>, Option<&'static crate::map::PropEntity>)>,
    map_entities: Option<Res<'w, crate::map::MapEntities>>,
    damage: MessageWriter<'w, Damage>,
    play: MessageWriter<'w, PlaySound>,
    decals: MessageWriter<'w, PlaceDecal>,
    detonated: MessageWriter<'w, Detonated>,
    deafened: MessageWriter<'w, Deafened>,
    inflicted: MessageWriter<'w, super::Inflicted>,
    push_scale: Option<Res<'w, super::PushScale>>,
    /// Spawn-protected characters (`core::SpawnProtected`): projectiles
    /// fly through them, blasts leave them out.
    protected: Query<'w, 's, (Entity, &'static crate::core::SpawnProtected)>,
    clock: Option<Res<'w, SimClock>>,
}

impl GrenadeWorld<'_, '_> {
    fn body(&self, collider: Entity) -> Entity {
        self.colliders.get(collider).map_or(collider, |c| c.body)
    }

    fn is_character(&self, e: Entity) -> bool {
        self.bodies.get(e).is_ok_and(|b| b.intent.is_some())
    }

    /// Characters that are dead (their bodies are ragdolls or hidden).
    fn dead(&self) -> Vec<Entity> {
        self.bodies
            .iter()
            .filter(|b| b.intent.is_some() && b.health.is_some_and(|h| h.current <= 0.0))
            .map(|b| b.entity)
            .collect()
    }

    /// Whether `e` is spawn-protected in its bullets window: not there
    /// for projectiles and blasts (`core::SpawnProtected::passes_attacks`).
    fn passing(&self, e: Entity) -> bool {
        let now = self.clock.as_ref().map_or(0.0, |c| c.now);
        self.protected.get(e).is_ok_and(|(_, p)| p.passes_attacks(now))
    }

    /// Every character `passing` now.
    fn all_passing(&self) -> Vec<Entity> {
        let now = self.clock.as_ref().map_or(0.0, |c| c.now);
        self.protected.iter().filter(|(_, p)| p.passes_attacks(now)).map(|(e, _)| e).collect()
    }

    /// A world-only line trace (static bodies that aren't props: the map's
    /// brushes and surfaces), also stopping at `target`: the fraction and
    /// the body hit.
    fn world_trace(&self, from: Vec3, to: Vec3, target: Option<Entity>) -> Option<(f32, Vec3, Entity)> {
        let d = to - from;
        let len = d.length();
        let dir = Dir3::new(d).ok()?;
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        self.spatial
            .cast_ray_predicate(from, dir, len, true, &filter, &|e| {
                let body = self.body(e);
                Some(body) == target || self.is_world(e, body)
            })
            .map(|h| (h.distance / len, h.normal, self.body(h.entity)))
    }

    /// The world: static bodies that aren't prop entities (the map's
    /// brushes, brush entities, static props: Source's static props are
    /// part of the world; measured 2026-10-18, a tall one on de_dust2
    /// stopped an explosion whole).
    fn is_world(&self, collider: Entity, body: Entity) -> bool {
        self.rigid
            .get(collider)
            .or_else(|_| self.rigid.get(body))
            .is_ok_and(|(rb, prop)| rb.is_static() && (prop.is_none() || !self.prop_entities.contains(body)))
    }

    /// What shields a blast (`BlastShield`): the first prop entity, moving
    /// body or living character other than `target` on the line, and its
    /// damage factor.
    fn shield(&self, from: Vec3, to: Vec3, target: Entity, s: &BlastShield) -> f32 {
        let d = to - from;
        let Ok(dir) = Dir3::new(d) else { return 1.0 };
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        let hit = self.spatial.cast_ray_predicate(from, dir, d.length(), true, &filter, &|e| {
            let body = self.body(e);
            body != target && (self.living_character(body) || !self.is_world(e, body) && !self.is_character(body))
        });
        match hit.map(|h| self.body(h.entity)) {
            None => 1.0,
            Some(b) if self.is_character(b) => s.character,
            Some(_) => s.prop,
        }
    }

    /// Whether anything stops a flash's line to `victim`'s eye: the
    /// world, a prop entity, a loose body or another living character,
    /// not glass or grates (`SeeThrough`) (measured 2026-10-18,
    /// grenades.md Q12: a crate or a player's hull on the line stops it
    /// whole; de_nuke's windows and vents let it through).
    fn flash_blocked(&self, from: Vec3, to: Vec3, victim: Entity) -> bool {
        let d = to - from;
        let Ok(dir) = Dir3::new(d) else { return false };
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        self.spatial
            .cast_ray_predicate(from, dir, d.length(), true, &filter, &|e| {
                let body = self.body(e);
                body != victim
                    && !self.see_through.contains(e)
                    && !self.see_through.contains(body)
                    && (self.is_world(e, body) || self.living_character(body) || !self.is_character(body))
            })
            .is_some()
    }

    /// The first body other than `target` (and `skip`) on the line that
    /// isn't the world: a living character, a prop, a loose body
    /// (`SourceRadius`'s shield).
    fn blocker(&self, from: Vec3, to: Vec3, target: Entity, skip: Option<Entity>) -> Option<Entity> {
        let d = to - from;
        let dir = Dir3::new(d).ok()?;
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        self.spatial
            .cast_ray_predicate(from, dir, d.length(), true, &filter, &|e| {
                let body = self.body(e);
                body != target
                    && Some(body) != skip
                    && !self.is_world(e, body)
                    && (self.living_character(body) || !self.is_character(body))
            })
            .map(|h| self.body(h.entity))
    }

    fn living_character(&self, e: Entity) -> bool {
        self.bodies
            .get(e)
            .is_ok_and(|b| b.intent.is_some() && b.health.is_some_and(|h| h.current > 0.0))
            && !self.passing(e)
    }

    /// Whether something of the classes `stoppers` is on the line (other
    /// than `target`).
    fn stopper_on_line(&self, from: Vec3, to: Vec3, target: Entity, stoppers: &[&str]) -> bool {
        let d = to - from;
        if stoppers.is_empty() {
            return false;
        }
        let Ok(dir) = Dir3::new(d) else { return false };
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        self.spatial
            .cast_ray_predicate(from, dir, d.length(), true, &filter, &|e| {
                let body = self.body(e);
                body != target && self.is_of_class(body, stoppers)
            })
            .is_some()
    }

    /// A map entity's class is one of `classes`.
    fn is_of_class(&self, body: Entity, classes: &[&str]) -> bool {
        let Ok((brush, prop)) = self.map_nodes.get(body) else { return false };
        let Some(index) = brush.map(|b| b.0).or(prop.map(|p| p.0)) else { return false };
        self.map_entities
            .as_ref()
            .and_then(|m| m.entities.get(index))
            .is_some_and(|e| classes.iter().any(|c| e.classname().eq_ignore_ascii_case(c)))
    }

    fn in_water(&self, p: Vec3) -> bool {
        self.water.as_ref().is_some_and(|w| {
            w.0.iter()
                .any(|v| v.brush.planes.iter().all(|(n, d)| n.dot(p) - d <= 0.0))
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn fly(
    mut projectiles: Query<(Entity, &mut Projectile, &mut Transform), Without<GameFlown>>,
    mut world: GrenadeWorld,
    mut rng: ResMut<GrenadeRng>,
    mut commands: Commands,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let now = time.elapsed_secs_f64();
    // The dead and the spawn-protected (`passing`) aren't in its way.
    let mut dead = world.dead();
    dead.extend(world.all_passing());
    for (entity, mut p, mut transform) in &mut projectiles {
        let f = p.flight;
        // 1. The periodic check: fuse, water.
        let check = ((f.check_interval / dt).round() as u32).max(1);
        if p.ticks.is_multiple_of(check) {
            let elapsed = p.ticks as f32 * dt;
            let pop = match &p.effect {
                GrenadeEffect::Smoke(s) => p.velocity.length() < s.pop_speed,
                _ => true,
            };
            if elapsed > p.fuse + 1e-4 && pop {
                detonate(
                    entity,
                    &p,
                    transform.translation,
                    &mut world,
                    &mut rng.0,
                    &mut commands,
                    now,
                );
                continue;
            }
            if world.in_water(transform.translation) {
                p.velocity *= f.water_slowdown;
            }
        }
        p.ticks += 1;
        // 2. At rest on the ground: nothing moves.
        if p.resting && p.velocity == Vec3::ZERO {
            continue;
        }
        p.resting = false;
        // 3-5. Clamp, gravity, spin.
        p.velocity = p
            .velocity
            .clamp(Vec3::splat(-f.max_velocity), Vec3::splat(f.max_velocity));
        let (mut step, v) = gravity_step(p.velocity, f.gravity, dt);
        p.velocity = v;
        transform.rotation *= Quat::from_scaled_axis(p.spin * dt);
        // 6-7. Sweep and contacts (a floor contact keeps the rest of the
        // move; walls lose it).
        let shape = Collider::cuboid(f.half * 2.0, f.half * 2.0, f.half * 2.0);
        let excluded: Vec<Entity> = p.thrower.into_iter().chain(dead.iter().copied()).collect();
        let filter = SpatialQueryFilter::from_excluded_entities(excluded).with_mask(SOLID_LAYERS);
        for _ in 0..2 {
            let len = step.length();
            let Ok(dir) = Dir3::new(step) else { break };
            let config = ShapeCastConfig {
                max_distance: len,
                ignore_origin_penetration: true,
                ..ShapeCastConfig::DEFAULT
            };
            let Some(hit) =
                world
                    .spatial
                    .cast_shape(&shape, transform.translation, Quat::IDENTITY, dir, &config, &filter)
            else {
                transform.translation += step;
                break;
            };
            let fraction = hit.distance / len;
            transform.translation += *dir * (hit.distance - SKIN).max(0.0);
            let body = world.body(hit.entity);
            let n = hit.normal1.normalize_or_zero();
            if let Some(s) = &p.bounce_sound {
                world.play.write(PlaySound {
                    pitch: None,
                    entry: s.clone(),
                    at: Some(transform.translation),
                    volume: None,
                    source: Some(entity),
                    channel: Some(CHAN_VOICE),
                });
            }
            // Glass and other breakables: damage it and fly on slower
            // (when it breaks this tick; a second contact with the same
            // one bounces).
            let breakable = world.bodies.get(body).is_ok_and(|b| b.damageable.is_some());
            if breakable && p.breakable != Some(body) {
                p.breakable = Some(body);
                world.damage.write(Damage {
                    force: bevy::math::Vec3::ZERO,
                    target: body,
                    attacker: p.thrower,
                    amount: f.breakable_damage,
                    point: hit.point1,
                    dir: *dir,
                    hitgroup: Hitgroup::Generic,
                    kind: DamageKind::Melee,
                    weapon: Some(p.weapon),
                });
                p.velocity *= f.breakable_slowdown;
                break;
            }
            let player = world.is_character(body);
            let (v, rest) = bounce(p.velocity, n, player, &f, 0.1 * 0.0254);
            p.velocity = v;
            if rest {
                p.resting = true;
                p.spin = Vec3::ZERO;
                let yaw = rng.0.float(0.0, std::f32::consts::TAU);
                transform.rotation = Quat::from_rotation_arc(Vec3::Y, n) * Quat::from_rotation_y(yaw);
                break;
            }
            if n.y <= f.floor_normal || v == Vec3::ZERO {
                break;
            }
            step = v * (1.0 - fraction) * dt;
        }
        // 8. Into water.
        let wet = world.in_water(transform.translation);
        if wet && !p.in_water {
            p.velocity.y *= f.water_entry;
        }
        if wet != p.in_water
            && let Some((enter, leave)) = &p.water_sounds
        {
            world.play.write(PlaySound::at(
                if wet { enter.clone() } else { leave.clone() },
                transform.translation,
            ));
        }
        p.in_water = wet;
    }
}

/// The grenade goes off.
fn detonate(
    entity: Entity,
    p: &Projectile,
    at: Vec3,
    world: &mut GrenadeWorld,
    rng: &mut ParticleRng,
    commands: &mut Commands,
    now: f64,
) {
    let mut message = Detonated {
        kind: p.effect.kind(),
        weapon: p.weapon,
        thrower: p.thrower,
        at,
        ground: None,
        projectile: entity,
        shake: match &p.effect {
            GrenadeEffect::Blast(b) => b.shake,
            _ => None,
        },
        flags: Default::default(),
    };
    match &p.effect {
        GrenadeEffect::Blast(b) => {
            let (origin, probe) = explode_at(at, b, p.thrower, p.weapon, Some(entity), world, rng, false, DamageKind::Blast);
            message.at = origin;
            message.ground = probe;
            if let Some(s) = &b.sound {
                world.play.write(PlaySound {
                    pitch: None,
                    entry: s.clone(),
                    at: Some(origin),
                    volume: None,
                    source: Some(entity),
                    channel: Some(CHAN_WEAPON),
                });
            }
            commands.entity(entity).despawn();
        }
        GrenadeEffect::Flash(fl) => {
            flash(at, fl, world, commands, now);
            if let Some(s) = &fl.sound {
                world.play.write(PlaySound {
                    pitch: None,
                    entry: s.clone(),
                    at: Some(at),
                    volume: None,
                    source: None,
                    channel: None,
                });
            }
            commands.entity(entity).despawn();
        }
        GrenadeEffect::Smoke(s) => {
            if let Some(sound) = &s.sound {
                world.play.write(PlaySound::at(sound.clone(), at));
            }
            commands.spawn((
                Name::new("Smoke cloud"),
                SmokeCloud {
                    centre: at,
                    started: now,
                    smoke: s.clone(),
                    grenade: Some(entity),
                    weapon: Some(p.weapon),
                },
                SightBlocker {
                    centre: at,
                    radius: 0.0,
                },
            ));
            // The spent grenade stays where it lies.
            commands.entity(entity).remove::<Projectile>();
        }
    }
    world.detonated.write(message);
}

/// A blast at `at`: the ground probe under it (spec 5.2), radius damage
/// and pushes from just above it, the scorch mark. Returns the blast
/// point and what the probe hit. A `round` scorch goes at the next round
/// restart (`PlaceDecal::round`).
#[allow(clippy::too_many_arguments)]
fn explode_at(
    at: Vec3,
    b: &Blast,
    thrower: Option<Entity>,
    weapon: &'static str,
    inflictor: Option<Entity>,
    world: &mut GrenadeWorld,
    rng: &mut ParticleRng,
    round: bool,
    kind: DamageKind,
) -> (Vec3, Option<(Vec3, Vec3, Entity)>) {
    let (from, to) = (at + Vec3::Y * b.probe_up, at - Vec3::Y * b.probe_down);
    let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
    let probe = Dir3::new(to - from).ok().and_then(|dir| {
        world
            .spatial
            .cast_ray_predicate(from, dir, from.distance(to), true, &filter, &|e| {
                let body = world.body(e);
                !world.is_character(body) && Some(body) != inflictor
            })
            .map(|h| (from + *dir * h.distance, h.normal, world.body(h.entity)))
    });
    let origin = probe.map_or(at, |(point, n, _)| point + n * b.pull_out);
    blast(origin, b, thrower, weapon, inflictor, world, rng, kind);
    if let (Some(group), Some((point, n, body))) = (&b.scorch, probe) {
        world.decals.write(PlaceDecal {
            target: Some(body),
            group: DecalGroup::Named(group.clone()),
            point,
            normal: n,
            dir: -n,
            spin: true,
            round,
        });
    }
    (origin, probe)
}

/// The blast rules explosions from map logic (`core::Explosion`) follow:
/// a game's grenade blast, with the explosion's damage and radius (the
/// push scaled with the damage, a bell's σ with the radius). Without one,
/// explosions do nothing.
#[derive(Resource, Clone, Debug)]
pub struct ExplosionRule(pub Blast);

/// Apply `core::Explosion`s: a blast like a grenade's, then `Detonated`
/// so games draw it.
fn explosions(
    mut events: MessageReader<Explosion>,
    rule: Option<Res<ExplosionRule>>,
    mut world: GrenadeWorld,
    mut rng: ResMut<GrenadeRng>,
    mut sounds: MessageWriter<crate::map::GameSound>,
) {
    let Some(rule) = rule else {
        events.clear();
        return;
    };
    for e in events.read() {
        let mut b = rule.0.clone();
        let scale = if b.damage > 0.0 { e.damage / b.damage } else { 1.0 };
        b.force *= scale;
        if let Some(r) = &mut b.ragdolls {
            r.length *= scale;
        }
        // The rule's bell keeps its shape: σ in proportion to the radius
        // (CS:S: σ = R/3 for every explosion, measured 2026-10-17 on
        // env_explosions of radius 100, 150, 250 and a gas can;
        // specs/source/prop_damage.md Q3), whatever
        // `LegacyBlastDamage` says.
        let sigma = rule.0.bell_sigma.filter(|_| rule.0.radius > 0.0).map(|s| s * e.radius / rule.0.radius);
        b.damage = e.damage;
        b.radius = e.radius;
        b.bell_sigma = e.bell_sigma.or(sigma);
        b.legacy = false;
        if e.armor_ratio.is_some() {
            b.armor_ratio = e.armor_ratio;
        }
        b.through_walls |= e.through_walls;
        if e.flags.has(crate::core::BlastFlags::NO_DECAL) {
            b.scorch = None;
        }
        let (origin, probe) = explode_at(
            e.origin,
            &b,
            e.attacker,
            e.weapon.unwrap_or(EXPLOSION_WEAPON),
            e.inflictor,
            &mut world,
            &mut rng.0,
            e.round_scorch,
            e.kind,
        );
        // The rules' (map logic's, the bomb's): clients hear it too.
        if let Some(s) = &e.sound {
            sounds.write(crate::map::GameSound(PlaySound {
                pitch: None,
                entry: s.clone(),
                at: Some(origin),
                volume: None,
                source: e.inflictor,
                channel: None,
            }));
        }
        world.detonated.write(Detonated {
            kind: GrenadeKind::Blast,
            weapon: e.weapon.unwrap_or(EXPLOSION_WEAPON),
            thrower: e.attacker,
            at: origin,
            ground: probe,
            projectile: Entity::PLACEHOLDER,
            shake: b.shake,
            flags: e.flags,
        });
    }
}

/// Apply the blasts projectiles asked for (`BlastAt`): as a grenade's,
/// then `Detonated` so games draw it.
fn blast_requests(mut events: MessageReader<BlastAt>, mut world: GrenadeWorld, mut rng: ResMut<GrenadeRng>) {
    for e in events.read() {
        let b = &e.blast;
        let (origin, probe) =
            explode_at(e.at, b, e.thrower, e.weapon, e.inflictor, &mut world, &mut rng.0, false, DamageKind::Blast);
        if let Some(s) = &b.sound {
            world.play.write(PlaySound {
                pitch: None,
                entry: s.clone(),
                at: Some(origin),
                volume: None,
                source: None,
                channel: Some(CHAN_WEAPON),
            });
        }
        world.detonated.write(Detonated {
            kind: GrenadeKind::Blast,
            weapon: e.weapon,
            thrower: e.thrower,
            at: origin,
            ground: probe,
            projectile: e.inflictor.unwrap_or(Entity::PLACEHOLDER),
            shake: b.shake,
            // Made by code: never sparks under water.
            flags: crate::core::BlastFlags(crate::core::BlastFlags::NO_SPARKS),
        });
    }
}

const Y_UP: Vec3 = Vec3::Y;

/// The weapon name explosions from map logic count as (kill notices).
pub const EXPLOSION_WEAPON: &str = "env_explosion";

/// Radius damage from `origin` (spec 5.3) and the push on loose bodies;
/// `skip` (what exploded) takes none.
fn blast(
    origin: Vec3,
    b: &Blast,
    thrower: Option<Entity>,
    weapon: &'static str,
    skip: Option<Entity>,
    world: &mut GrenadeWorld,
    rng: &mut ParticleRng,
    kind: DamageKind,
) {
    let src = origin + Vec3::Y * b.src_lift;
    // Source's water rule: None without one, else whether it is under.
    let under = b
        .radius_rule
        .filter(|r| r.water_lift > 0.0)
        .map(|r| world.in_water(origin) && world.in_water(origin + Y_UP * r.water_lift));
    let stoppers: &[&str] = b.radius_rule.map_or(&[], |r| r.stoppers);
    let mut hits: Vec<(Entity, Vec3, f32)> = Vec::new();
    // Characters' shield sample points (`BlastShield`): eye, and the two
    // sides at half the eye height.
    let mut samples: Vec<(Entity, [Vec3; 3])> = Vec::new();
    // Characters' origins (the feet, as Source's), for their hearing.
    let mut origins: Vec<(Entity, Vec3)> = Vec::new();
    // Each target's bounds (`SourceRadius`'s box entry).
    let mut boxes: Vec<(Entity, Vec3, Vec3)> = Vec::new();
    for t in world.bodies.iter() {
        // The spawn-protected aren't there for it (`passing`).
        if Some(t.entity) == skip || world.passing(t.entity) {
            continue;
        }
        let alive = t.health.is_some_and(|h| h.current > 0.0);
        if !alive && t.damageable.is_none() {
            continue;
        }
        // Bounds touching the sphere.
        let (lo, hi) = t
            .aabb
            .map_or((t.transform.translation, t.transform.translation), |a| (a.min, a.max));
        if src.clamp(lo, hi).distance(src) > b.radius {
            continue;
        }
        if let Some(under) = under {
            let (bottom, top) = match (t.intent, t.state) {
                (Some(_), Some(s)) => (t.transform.translation + s.hull_min.with_x(0.0).with_z(0.0), t.transform.translation + s.eye_offset),
                _ => {
                    let c = (lo + hi) / 2.0;
                    (c.with_y(lo.y), c.with_y(hi.y))
                }
            };
            if (under && !world.in_water(bottom)) || (!under && world.in_water(top)) {
                continue;
            }
        }
        let target = match (t.intent, t.state) {
            (Some(_), Some(state)) => {
                let eye = t.transform.translation + state.eye_offset;
                let feet = if state.hull_min != Vec3::ZERO {
                    t.transform.translation.y + state.hull_min.y
                } else {
                    lo.y
                };
                let h = rng.float(b.body_target.0, b.body_target.1);
                origins.push((t.entity, t.transform.translation.with_y(feet)));
                if b.shield.is_some() {
                    let across = Vec3::Y.cross(eye - src).normalize_or(Vec3::X);
                    let half = ((hi - lo).x.min((hi - lo).z)) / 2.0;
                    let mid = eye.with_y(feet + (eye.y - feet) / 2.0);
                    samples.push((t.entity, [eye, mid + across * half, mid - across * half]));
                }
                eye.with_y(feet + (eye.y - feet) * h)
            }
            _ => (lo + hi) / 2.0,
        };
        hits.push((t.entity, target, 0.0));
        boxes.push((t.entity, lo, hi));
    }
    for (target, point, amount) in &mut hits {
        if world.stopper_on_line(src, *point, *target, stoppers) {
            continue;
        }
        // Characters aren't in the trace (it sees world brushes only);
        // breakable brushes are, and a hit on the target itself counts.
        let own = (!world.is_character(*target)).then_some(*target);
        let trace = if b.through_walls {
            None
        } else {
            world.world_trace(src, *point, own)
        };
        let mut end = match trace {
            None => *point,
            Some((fraction, _, body)) if body == *target => src.lerp(*point, fraction),
            Some(_) => continue,
        };
        // Source's rule (`SourceRadius`): a body on the line shields a
        // share (the distance to the eye), else the distance is to where
        // the line enters the character's box.
        let mut share = 1.0;
        if let Some(rule) = b.radius_rule
            && world.is_character(*target)
        {
            match world.blocker(src, *point, *target, skip) {
                Some(blocker) => {
                    let mass = if world.is_character(blocker) {
                        rule.character_mass
                    } else {
                        world.dynamic.get(blocker).ok().and_then(|(_, rb, _, m, _)| rb.is_dynamic().then(|| m.map_or(1.0, |m| m.value()))).unwrap_or(f32::NAN)
                    };
                    let absorbed = if mass.is_nan() { rule.shield_other } else { mass / rule.shield_mass };
                    if absorbed >= 1.0 {
                        continue;
                    }
                    share = 1.0 - absorbed;
                }
                None => {
                    if let Some(&(_, lo, hi)) = boxes.iter().find(|(e, ..)| e == target) {
                        let d = *point - src;
                        let len = d.length();
                        if len > 0.0 {
                            let t = super::projectile::ray_box(lo, hi, src, d / len, len).unwrap_or(len);
                            end = src + d / len * t;
                        }
                    }
                }
            }
        }
        *point = end;
        let d = end.distance(src);
        let legacy = b.legacy && world.legacy.as_ref().is_some_and(|l| l.0 != 0);
        *amount = share
            * match b.bell_sigma {
                Some(sigma) if !legacy => blast_damage_bell(b.damage, sigma, d),
                _ => blast_damage(b.damage, b.radius, d),
            };
    }
    for (target, end, mut raw) in hits {
        if raw <= 0.0 {
            continue;
        }
        let mut kept = 1.0;
        if let (Some(s), Some((_, points)), false) =
            (&b.shield, samples.iter().find(|(e, _)| *e == target), b.through_walls)
        {
            let side = (1.0 - s.head) / 2.0;
            kept = 1.0
                - [s.head, side, side]
                    .iter()
                    .zip(points)
                    .map(|(w, p)| w * (1.0 - world.shield(src, *p, target, s)))
                    .sum::<f32>();
            raw *= kept;
        }
        // Whole points for characters only; objects truncate after their
        // multipliers (as `deliver`'s hits).
        let character = world.bodies.get(target).is_ok_and(|t| t.health.is_some());
        let dir = (end - src).normalize_or_zero();
        let force = match b.radius_rule {
            Some(rule) => (rule.per_damage * raw).min(rule.force_cap) * rng.float(b.force_jitter.0, b.force_jitter.1),
            None => b.force * kept,
        };
        let mut damage = Damage {
            force: dir * force,
            target,
            attacker: thrower,
            amount: if character { super::quantize(raw, b.quantum) } else { raw },
            point: end,
            dir,
            hitgroup: Hitgroup::Generic,
            kind,
            weapon: Some(weapon),
        };
        // Armour takes its share only if the blast lands (`HitArmor`);
        // so does the hearing effect (spec 5.5: a player that takes blast
        // damage; not a teammate's with friendly fire off, a noclipping or
        // `God` player). The thrower's own blast always lands.
        world.armor.apply(&mut damage, raw, b.armor_ratio, b.quantum);
        if let Some(hearing) = &b.hearing
            && world.armor.lands(&damage)
            && let Some(&(_, at)) = origins.iter().find(|(e, _)| *e == target)
            && let Some(effect) = hearing.effect(raw, at.distance(origin))
        {
            world.deafened.write(Deafened { target, effect });
        }
        world.inflicted.write(super::Inflicted {
            target,
            amount: damage.amount,
            from: origin,
        });
        world.damage.write(damage);
    }
    // Loose bodies in reach and in the open get the push (physics_props.md
    // 5.3: the undecayed force), capped to a sane speed.
    let pushes: Vec<(Entity, Vec3, f32)> = world
        .dynamic
        .iter()
        .filter(|(e, rb, ..)| rb.is_dynamic() && Some(*e) != skip)
        .filter(|(_, _, pos, ..)| pos.0.distance(src) < b.radius)
        .filter(|(_, _, pos, ..)| under.is_none_or(|u| u == world.in_water(pos.0)))
        .map(|(e, _, pos, mass, _)| (e, pos.0, mass.map_or(1.0, |m| m.value())))
        .collect();
    for (e, centre, mass) in pushes {
        if world.stopper_on_line(src, centre, e, stoppers) {
            continue;
        }
        if !b.through_walls
            && world
            .world_trace(src, centre, Some(e))
            .is_some_and(|(_, _, body)| body != e)
        {
            continue;
        }
        let base = match b.radius_rule {
            // Its own decayed damage's force (projectiles.md 2.3).
            Some(rule) => (rule.per_damage * blast_damage(b.damage, b.radius, centre.distance(src))).min(rule.force_cap),
            None => b.force,
        };
        let j = (base * rng.float(b.force_jitter.0, b.force_jitter.1)).min(mass * b.max_push_speed);
        let dir = (centre - src).normalize_or(Vec3::Y);
        let scale = world.push_scale.as_ref().map_or(1.0, |s| s.0);
        if let Ok((.., mut forces)) = world.dynamic.get_mut(e) {
            forces.apply_linear_impulse(dir * j * scale);
        }
    }
    // Ragdolls in the radius: a blast line each, walls or not (the
    // client effect's rule has no trace).
    if let Some(r) = b.ragdolls {
        let from = origin - Vec3::Y * r.drop;
        let lines: Vec<(Entity, Vec3)> = world
            .ragdolls
            .iter()
            .filter_map(|(e, rd)| Some((e, world.ragdoll_bodies.get(*rd.bodies.first()?).ok()?.0)))
            .collect();
        for (ragdoll, root) in lines {
            let Some(length) = r.length_at(b.radius, root.distance(origin)) else {
                continue;
            };
            let dir = (root - from).normalize_or(Vec3::Y);
            world.ragdoll_shots.write(crate::map::RagdollShot {
                from,
                to: from + dir * length,
                blast: true,
                ragdoll: Some(ragdoll),
            });
        }
    }
}

/// Blind everyone who sees the flash (spec 6.2): teammates and the
/// thrower too; a prop or another player on the line hides it.
fn flash(at: Vec3, fl: &Flash, world: &mut GrenadeWorld, commands: &mut Commands, now: f64) {
    let mut blinded = Vec::new();
    for b in world.bodies.iter() {
        let (Some(intent), Some(state)) = (b.intent, b.state) else {
            continue;
        };
        if b.health.is_none_or(|h| h.current <= 0.0) {
            continue;
        }
        let eye = b.transform.translation + state.eye_offset;
        if world.flash_blocked(at, eye, b.entity) {
            continue;
        }
        let to = at - eye;
        let facing = (intent.look_rotation() * Vec3::NEG_Z).dot(to.normalize_or_zero());
        if let Some((alpha, mut hold, mut fade)) = (fl.model)(to.length(), facing) {
            if world.blockers.iter().any(|s| s.chord(at, eye) > 0.0) {
                hold *= fl.through_smoke;
                fade *= fl.through_smoke;
            }
            blinded.push((b.entity, alpha, hold, fade));
        }
    }
    for (e, alpha, hold, fade) in blinded {
        if let Some(effect) = fl.hearing.and_then(|h| h(alpha, hold + fade)) {
            world.deafened.write(Deafened { target: e, effect });
        }
        let new = Blinded {
            alpha,
            fade_start: now + hold as f64,
            end: now + (hold + fade) as f64,
        };
        let next = match world.blinded.get(e) {
            Ok(old) => old.raised(new, now),
            Err(_) => new,
        };
        commands.entity(e).insert(next);
    }
}

/// Clouds grow (their sight blocker with them) and go when faded, with
/// the grenade lying in them.
fn smoke_clouds(mut clouds: Query<(Entity, &SmokeCloud, &mut SightBlocker)>, mut commands: Commands, time: Res<Time>) {
    let now = time.elapsed_secs_f64();
    for (e, cloud, mut blocker) in &mut clouds {
        let t = (now - cloud.started) as f32;
        if t >= cloud.smoke.fade_end {
            commands.entity(e).despawn();
            if let Some(g) = cloud.grenade {
                commands.entity(g).try_despawn();
            }
            continue;
        }
        // A thinning cloud stops hiding things halfway through its fade.
        let radius = if cloud.fade(t) > 0.5 {
            cloud.radius(t) * cloud.smoke.sight_radius
        } else {
            0.0
        };
        if blocker.radius != radius {
            blocker.radius = radius;
        }
    }
}

/// On a projectile or smoke cloud: a new round leaves it be (Combat Arms'
/// grenades carry over into the next round, its grenades.md section 15).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SurvivesRestart;

/// A new round clears grenades in flight, spent ones, clouds and
/// blindness (but what `SurvivesRestart` marks).
#[allow(clippy::type_complexity)]
fn clear_on_restart(
    restarts: Option<Res<RoundRestarts>>,
    mut seen: Local<u32>,
    gone: Query<Entity, (Or<(With<Projectile>, With<SmokeCloud>)>, Without<SurvivesRestart>)>,
    clouds: Query<&SmokeCloud, Without<SurvivesRestart>>,
    blinded: Query<Entity, With<Blinded>>,
    mut commands: Commands,
) {
    let Some(restarts) = restarts else { return };
    if restarts.0 == *seen {
        return;
    }
    *seen = restarts.0;
    for c in &clouds {
        if let Some(g) = c.grenade {
            commands.entity(g).try_despawn();
        }
    }
    for e in &gone {
        commands.entity(e).try_despawn();
    }
    for e in &blinded {
        commands.entity(e).remove::<Blinded>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const U: f32 = 0.0254;

    fn css_throw() -> ThrowRule {
        ThrowRule {
            pitch_lift: -10.0,
            pitch_scale: 100.0 / 90.0,
            pitch_scale_up: 80.0 / 90.0,
            speed_per_deg: 6.0 * U,
            speed_max: 750.0 * U,
            forward: 16.0 * U,
        }
    }

    fn css_flight() -> Flight {
        Flight {
            half: 2.0 * U,
            gravity: 320.0 * U,
            elasticity: 0.45,
            player_elasticity: 0.3,
            elasticity_cap: 0.9,
            stop_speed: 30.0 * U,
            floor_normal: 0.7,
            breakable_damage: 0.1,
            breakable_slowdown: 0.4,
            water_slowdown: 0.5,
            water_entry: 0.5,
            max_velocity: 3500.0 * U,
            check_interval: 0.2,
            spin_roll: 600f32.to_radians(),
            spin_pitch: 1200f32.to_radians(),
        }
    }

    /// Source (x forward, y left, z up) inches from ours at yaw 0.
    fn src(v: Vec3) -> Vec3 {
        Vec3::new(-v.z, -v.x, v.y) / U
    }

    fn close(a: Vec3, b: Vec3, eps: f32) -> bool {
        (a - b).length() < eps
    }

    #[test]
    fn throws_g1_to_g7() {
        let r = css_throw();
        let eye = Vec3::new(0.0, 64.0 * U, 0.0);
        // (view pitch, Source degrees positive down; spawn offset; velocity)
        let cases = [
            (0.0, Some(Vec3::new(15.757, 0.0, 2.778)), Vec3::new(590.88, 0.0, 104.19)),
            // Looking up bends by 80/90, not the template's 100/90
            // (measured on CS:S, grenades.md Q5).
            (-22.5, Some(Vec3::new(13.856, 0.0, 8.0)), Vec3::new(623.54, 0.0, 360.0)),
            (-45.0, None, Vec3::new(482.09, 0.0, 574.53)),
            (-89.0, Some(Vec3::new(0.248, 0.0, 15.998)), Vec3::new(11.64, 0.0, 749.91)),
            (45.0, None, Vec3::new(229.81, 0.0, -192.84)),
            (89.0, Some(Vec3::new(0.31, 0.0, -15.997)), Vec3::new(0.13, 0.0, -6.67)),
        ];
        for (p, offset, vel) in cases {
            let (start, v) = r.throw(eye, 0.0, (-p as f32).to_radians(), Vec3::ZERO);
            assert!(close(src(v), vel, 0.1), "pitch {p}: {}", src(v));
            if let Some(o) = offset {
                assert!(close(src(start - eye), o, 0.01), "pitch {p}: {}", src(start - eye));
            }
        }
        assert!((r.pitch(-22.5) + 30.0).abs() < 1e-4 && (r.speed(-30.0) / U - 720.0).abs() < 1e-3);
        // The probe's throws (grenades.md "CS:S values", Q5).
        for (view, bent, speed) in [(46.65, 41.84, 289.0), (17.58, 9.53, 482.8), (-23.06, -30.50, 723.0), (-69.35, -71.64, 750.0)] {
            assert!((r.pitch(view) - bent).abs() < 0.02, "{view}: {}", r.pitch(view));
            assert!((r.speed(r.pitch(view)) / U - speed).abs() < 0.2, "{view}");
        }
        // G7: running forward at 250 adds it all.
        let run = Vec3::new(0.0, 0.0, -250.0 * U);
        let (_, v) = r.throw(eye, 0.0, 0.0, run);
        assert!(close(src(v), Vec3::new(840.88, 0.0, 104.19), 0.1));
    }

    #[test]
    fn flight_g8_g9() {
        // G9: 20 ticks from rest with (600, 0, 300).
        let mut v = Vec3::new(0.0, 300.0, -600.0) * U;
        let mut p = Vec3::ZERO;
        for _ in 0..20 {
            let (step, nv) = gravity_step(v, 320.0 * U, 0.015);
            p += step;
            v = nv;
        }
        assert!(close(p / U, Vec3::new(0.0, 75.6, -180.0), 1e-2), "{}", p / U);
        assert!((v.y / U - 204.0).abs() < 1e-3);
        // G8: apex height 104.19² / 640.
        assert!((104.19f32.powi(2) / (2.0 * 320.0) - 16.96).abs() < 0.01);
    }

    #[test]
    fn bounces_g10_to_g14() {
        let f = css_flight();
        let snap = 0.1 * U;
        let up = Vec3::Y;
        // G10: floor, keeps moving.
        let (v, rest) = bounce(Vec3::new(590.88, -200.0, 0.0) * U, up, false, &f, snap);
        assert!(!rest && close(v / U, Vec3::new(265.896, 90.0, 0.0), 1e-2), "{}", v / U);
        // G11 wall, G12 player.
        let wall = Vec3::NEG_X;
        let (v, _) = bounce(Vec3::X * 500.0 * U, wall, false, &f, snap);
        assert!(close(v / U, Vec3::X * -225.0, 1e-3));
        let (v, _) = bounce(Vec3::X * 500.0 * U, wall, true, &f, snap);
        assert!(close(v / U, Vec3::X * -67.5, 1e-3));
        // G13: slow on a floor: rests.
        let (v, rest) = bounce(Vec3::new(40.0, -50.0, 0.0) * U, up, false, &f, snap);
        assert!(rest && v == Vec3::ZERO);
        // G14: slow on a wall: stops, no rest (falls next tick).
        let (v, rest) = bounce(Vec3::X * -50.0 * U, Vec3::X, false, &f, snap);
        assert!(!rest && v == Vec3::ZERO);
    }

    #[test]
    fn blast_falls_off_linearly() {
        assert_eq!(blast_damage(100.0, 350.0, 0.0), 100.0);
        assert!((blast_damage(100.0, 350.0, 175.0) - 50.0).abs() < 1e-4);
        assert_eq!(blast_damage(100.0, 350.0, 350.0), 0.0);
        assert_eq!(blast_damage(100.0, 350.0, 400.0), 0.0);
        // G18's range: d in [204.61, 209.51].
        assert!((blast_damage(100.0, 350.0, 204.61) - 41.54).abs() < 0.01);
        assert!((blast_damage(100.0, 350.0, 209.51) - 40.14).abs() < 0.01);
    }

    #[test]
    fn smoke_g25_to_g29() {
        for (t, e) in [(0.0, 0.0), (0.25, 91.84), (0.5, 169.71), (1.0, 240.0), (5.0, 240.0)] {
            assert!((smoke_radius(240.0, 1.0, t) - e).abs() < 0.01, "{t}");
        }
        for (t, f) in [(17.0, 1.0), (18.0, 0.9045), (19.5, 0.5), (21.0, 0.0955), (22.0, 0.0)] {
            assert!((smoke_fade(17.0, 22.0, t) - f).abs() < 1e-3, "{t}");
        }
        for (d, a) in [(50.0, 1.0), (120.0, 0.714), (200.0, 0.238), (260.0, 0.0)] {
            assert!((smoke_fog(d, 240.0, 1.0) - a).abs() < 1e-3, "{d}");
        }
        let cloud = |at: Vec3| SmokeCloud {
            centre: at,
            started: 0.0,
            smoke: Smoke {
                max_radius: 240.0,
                expand_time: 1.0,
                fade_start: 17.0,
                fade_end: 22.0,
                pop_speed: 0.1,
                sight_radius: 1.0,
                sound: None,
            },
            grenade: None,
            weapon: None,
        };
        let both = [cloud(Vec3::ZERO), cloud(Vec3::X)];
        assert_eq!(smoke_fog_at(Vec3::ZERO, 5.0, both.iter()), 1.0);
    }

    #[test]
    fn blindness_holds_fades_and_only_rises() {
        let b = Blinded {
            alpha: 1.0,
            fade_start: 2.0,
            end: 4.0,
        };
        assert_eq!(b.alpha_at(1.0), 1.0);
        assert!((b.alpha_at(3.0) - 0.5).abs() < 1e-6);
        assert_eq!(b.alpha_at(4.0), 0.0);
        let weak = Blinded {
            alpha: 0.3,
            fade_start: 1.5,
            end: 2.5,
        };
        assert_eq!(b.raised(weak, 1.0), b);
        assert_eq!(weak.raised(b, 1.0), b);
    }

    #[test]
    fn sight_blocker_chords() {
        let s = SightBlocker {
            centre: Vec3::ZERO,
            radius: 2.0,
        };
        assert!((s.chord(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0)) - 4.0).abs() < 1e-5);
        assert!(s.blocks(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0)));
        // Grazing: a short chord doesn't block.
        assert!(!s.blocks(Vec3::new(-5.0, 1.9, 0.0), Vec3::new(5.0, 1.9, 0.0)));
        // Stopping inside.
        assert!((s.chord(Vec3::new(-5.0, 0.0, 0.0), Vec3::ZERO) - 2.0).abs() < 1e-5);
    }
}
