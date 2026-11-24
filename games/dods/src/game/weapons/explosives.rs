//! DoD:S's explosives (M1b; specs/dods/weapons.md, "Grenades", "Rifle
//! grenades", "Rockets"; damage.md, "Explosions"), on Chimera's grenade
//! and projectile parts:
//!
//! - **Frag and stick grenades** (`dods:grenade`, `GrenadeParams`): a
//!   `Throwable` (one in hand, the pin on attack, the throw on release:
//!   left click, DoD:S's own attack key) whose projectile DoD:S makes
//!   (`ThrowOptions::game_launch`, `on_launch`). **The fuse runs from the
//!   press** (`DodsGrenade::pinned_at`, `pins`): the thrown grenade goes
//!   off `fuse` (5.1 s) after the pin was pulled, and held that long it
//!   goes off in the hand (at the eye). **The throw**: from the eye, the
//!   aim raised 10° and squeezed so straight up and down stay so (aiming A°
//!   up: 10 + A × 80/90; D° down: 10 − D × 100/90), at 500 units/s, and
//!   aiming more than 22.5° up (`speed_knee`) faster by 250 units/s over
//!   the last 67.5° (`speed_per_deg` 3.7037: about 750 straight up), at
//!   most 1400 (`dod_grenademaxspeed`), plus half the thrower's velocity
//!   along the aim (`carry`; none across it). **Timing** (`pins`): the
//!   press pulls the pin and blocks the weapon 0.667 s (`pin_time`); the
//!   release throws on its own tick, however soon; the next grenade is
//!   drawn once the 0.667 s are over and the grenade has left, and is
//!   ready 0.667 s later (`redraw_time`); after the last the player
//!   switches on the next tick. A box under 420 units/s² of gravity, its
//!   bounce low and scattered (`BOUNCE`: the spec's suggestion).
//! - **Rifle grenades** (`dods:rifle_grenade`, `RifleGrenadeParams`): a
//!   gun whose rounds (`Fired`, no bullet) become grenades (`rifle_shots`):
//!   from the eye along the aim at 2000 units/s, gravity 448 (the measured
//!   gravity and drag together, 440-456), bouncing, going off 3.63 s after
//!   the shot whatever they meet.
//! - **Rockets** (`dods:rocket`, `RocketParams`, on Chimera's flyers:
//!   `chimera:projectile`, 1300 units/s, 80 units/s² of gravity): they
//!   explode on touching a player or the world (measured). **Shouldering** (`Shoulder`, `shoulders`): the alt fire shoulders
//!   the launcher (the next shot `shoulder_time`, 0.79 s, later), and again
//!   (or held past that) lowers it (`unshoulder_time`, 0.99 s); lowered it
//!   doesn't fire; shouldered its holder moves at most 50 units/s
//!   (`Shoulder::MAX_SPEED`, `stance::step`). After a shot the launcher
//!   takes nothing, the reload key neither, for 0.84 s (bazooka) or 2.33 s
//!   (Panzerschreck: the data's cycle).
//! - **Reloads by the key only** (`key_reloads`): the launchers (3.0 s)
//!   and rifle grenades (2.91 s, 2.505 s) stay empty until the reload key
//!   (measured).
//! - **The blast** (`blast`): `damage` × (1 − r / `radius`) (150 in 375 for
//!   grenades and rifle grenades, 150 in 200 for rockets), r to a point 40
//!   units above the victim's feet standing (its body's middle: the share
//!   40/58 of its eye height), only along a clear line through the world
//!   (players don't shield it: measured), the thrower hurt too.
//! - **In the ladders**: the frags are the grenade band's, the rifle
//!   grenades and launchers the launcher band's (their data's `class`),
//!   each hurting by its blast (the harm and power readers: the blast's
//!   damage at its centre).
//! - **In flight** each is drawn as its projectile's own model (its data's
//!   `model`: `w_frag`, `w_stick`, `w_garand_rg_grenade`,
//!   `w_k98_rg_grenade`, `w_bazooka_rocket`, `w_panzerschreck_rocket`;
//!   the scripts name only the weapon's), by `projectile_key`, never the
//!   weapon's world model.
//! - **Their look** (`looks`): Source's explosion (`source_engine::effects::
//!   explosions`) where a DoD:S blast went off, and `BaseExplosionEffect.Sound`
//!   when the map's sounds have it; the particle materials load with DoD:S's
//!   maps and its guest dressing (`maps`).
//!
//! Known shortcuts (docs/tech-debt.md):
//! - The blast's stun event (two thirds of it, no health: the view shake
//!   of `dod_stun_*`) isn't made (how it shakes is unmeasured); why a
//!   grenade resting on the ground hurt nobody 50-100 away is open
//!   (damage.md, open question 8): ours hurts along a clear line.
//! - The thrower's velocity's share is "about half" (three throws:
//!   weapons.md, open question 6); dying with the pin out drops nothing.
//! - The bounce is the spec's suggested low, scattered one (`BOUNCE`), not
//!   DoD:S's tumbling physics object (no law found); the rifle grenade's
//!   drag is part of its gravity; no live-grenade pick-up, no smoke
//!   grenades.
//! - Shouldering isn't predicted with the weapon (`Shoulder` is the
//!   server's and the predicting client's own); the rifle grenades' lock
//!   after a shot is ours (0.5 s, unmeasured).
//! - Their holders' bodies play the shared model's own sets for them (their
//!   data's `dods:body_anim`: `GREN_FRAG`, `GREN_STICK`, `BAZOOKA`,
//!   `PSCHRECK`, the rifle's with the `RIFLEGRENADE` reload), but the
//!   shouldered launcher plays no zoom set and the throw no gesture of its
//!   own beyond the set's attack.
//! - The explosion's sound is Source's shared entry (no spec names
//!   DoD:S's); no scorch decal (unspecified).

use std::sync::Arc;

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    Inventory, Weapon, WeaponEvent, WeaponEventKind, WeaponState,
    grenade::{
        Blast, BounceRule, Detonated, Flight, GrenadeEffect, GrenadeKind, Launch, Projectile, RagdollBlast, ThrowOptions, ThrowRule, Throwable,
    },
    projectile::Launcher,
};

use crate::core::{Intent, MovementState, NetRole, SimClock};
use crate::map::loose::ShownItem;

/// Source's unit, m; a damage point, health.
const U: f32 = 0.0254;
const HP: f32 = 0.01;

/// The hooks' names.
pub const GRENADE_HOOK: &str = "dods:grenade";
pub const RIFLE_GRENADE_HOOK: &str = "dods:rifle_grenade";
pub const ROCKET_HOOK: &str = "dods:rocket";

/// Where a blast aims at a character, units above its feet standing
/// (damage.md: the body's middle), and the standing eye.
pub const BODY_MIDDLE: f32 = 40.0;

/// The model key (`ShownItem`) of `weapon`'s projectile: its data's
/// `model` loads under it (`super::add_guns`), the rockets' `shown` names
/// it, and a grenade or rifle grenade is drawn by it, never by the
/// weapon's own world model. Starting with the weapon's id, it stays with
/// its weapon in a map's kit (`GameKit::stay`).
pub fn projectile_key(weapon: &str) -> String {
    format!("{weapon}/projectile")
}

/// The projectile's model `weapon`'s data names (its `dods:grenade`,
/// `dods:rifle_grenade` or `dods:rocket` hook's `model`).
pub fn projectile_model(weapon: &chimera_sim::weapon::data::WeaponData) -> Option<&str> {
    [GRENADE_HOOK, RIFLE_GRENADE_HOOK, ROCKET_HOOK]
        .iter()
        .find_map(|h| weapon.hooks.get(*h)?.get("model")?.as_str())
}

/// A DoD:S blast of `damage` points falling linearly to nothing at
/// `radius` units (damage.md, "Explosions").
pub fn blast(damage: f32, radius: f32) -> Blast {
    let middle = BODY_MIDDLE / super::super::movement::EYE_STAND;
    Blast {
        damage: damage * HP,
        radius: radius * U,
        bell_sigma: None,
        legacy: false,
        armor_ratio: None,
        quantum: 0.0,
        // CS:S's ground probe and lift (Source's grenade template).
        probe_up: 8.0 * U,
        probe_down: 24.0 * U,
        pull_out: 0.6 * U,
        src_lift: U,
        body_target: (middle, middle),
        // `dod_explosionforcescale` 1: Source's push of loose bodies.
        force: (damage * 300.0).min(30000.0) * U,
        force_jitter: (0.85, 1.15),
        max_push_speed: 2000.0 * U,
        ragdolls: Some(RagdollBlast { length: damage * U, min: U, drop: 32.0 * U }),
        scorch: None,
        sound: None,
        hearing: None,
        shake: None,
        shield: None,
        through_walls: false,
        radius_rule: None,
    }
}

/// The grenades' bounce (weapons.md, "Bounce": the spec's suggestion for
/// the party, no law having been found): about 0.2 of the speed into the
/// surface comes back, half of the speed along it is kept, turned up to
/// 30° either way (the tumble's scatter); slow enough on a floor it rests
/// (`Flight::stop_speed`: strong rolling friction).
pub const BOUNCE: BounceRule = BounceRule { normal: 0.2, tangent: 0.5, scatter: 30.0 * std::f32::consts::PI / 180.0 };

/// A grenade's flight: a 4-unit box under `gravity` (units/s²), its fuse
/// checked every tick (its bounce `BOUNCE`; the elasticity here is CS:S's,
/// for what `BOUNCE` doesn't cover: other characters).
pub fn flight(gravity: f32) -> Flight {
    Flight {
        half: 2.0 * U,
        gravity: gravity * U,
        elasticity: 0.45,
        player_elasticity: 0.3,
        elasticity_cap: 0.9,
        stop_speed: 30.0 * U,
        floor_normal: 0.7,
        breakable_damage: 10.0 * HP,
        breakable_slowdown: 0.4,
        water_slowdown: 0.5,
        water_entry: 0.5,
        max_velocity: 3500.0 * U,
        check_interval: super::super::TICK_INTERVAL as f32,
        spin_roll: 600f32.to_radians(),
        spin_pitch: 1200f32.to_radians(),
    }
}

/// `dods:grenade`'s parameters (units, seconds, damage points).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GrenadeParams {
    /// The script it reads (`script_numbers`: its models and sounds).
    pub script: String,
    /// The projectile's model (a file of the pak; the scripts name only
    /// the weapon's), drawn by `projectile_key` (`super::add_guns`).
    #[serde(default)]
    pub model: Option<String>,
    pub damage: f32,
    pub radius: f32,
    /// From the pin's pull.
    pub fuse: f32,
    pub gravity: f32,
    /// The throw: the aim raised `lift` degrees, squeezed by `scale_up`
    /// aiming up and `scale_down` aiming down; speed `speed_min`, plus
    /// `speed_per_deg` for each degree the aim is above `speed_knee` (up),
    /// at most `speed_max`; plus `carry` of the thrower's velocity along
    /// the aim.
    pub lift: f32,
    pub scale_up: f32,
    pub scale_down: f32,
    pub speed_knee: f32,
    pub speed_per_deg: f32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub carry: f32,
    /// The pin's block (the windup), and the next grenade's redraw.
    pub pin_time: f32,
    pub redraw_time: f32,
}

impl GrenadeParams {
    /// The throw's direction as Chimera's rule (the speed is `velocity`'s).
    pub fn rule(&self) -> ThrowRule {
        ThrowRule {
            pitch_lift: -self.lift,
            pitch_scale: self.scale_down,
            pitch_scale_up: self.scale_up,
            speed_per_deg: self.speed_min * U / 90.0,
            speed_max: self.speed_max * U,
            forward: 0.0,
        }
    }

    /// The throw's speed (units/s) aiming `up` degrees up (negative down).
    pub fn speed(&self, up: f32) -> f32 {
        (self.speed_min + (up - self.speed_knee).max(0.0) * self.speed_per_deg).min(self.speed_max)
    }

    /// The throw's velocity (m/s) looking at `yaw` and `pitch` (radians,
    /// `Intent`'s), before the thrower's own.
    pub fn velocity(&self, yaw: f32, pitch: f32) -> Vec3 {
        let bent = self.rule().pitch(-pitch.to_degrees());
        let dir = Quat::from_euler(EulerRot::YXZ, yaw, -bent.to_radians(), 0.0) * Vec3::NEG_Z;
        dir * self.speed(pitch.to_degrees()) * U
    }

    /// What of the thrower's `velocity` (m/s) the grenade takes, looking at
    /// `yaw` and `pitch`: `carry` of its part along the aim.
    pub fn carried(&self, yaw: f32, pitch: f32, velocity: Vec3) -> Vec3 {
        let aim = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0) * Vec3::NEG_Z;
        aim * velocity.dot(aim) * self.carry
    }
}

/// A DoD:S grenade's own state: when its pin was pulled (the fuse's
/// start).
#[derive(Component, Clone, Debug)]
pub struct DodsGrenade {
    pub params: GrenadeParams,
    pub pinned_at: Option<f64>,
    was_pin: bool,
}

/// `dods:rifle_grenade`'s parameters.
#[derive(Component, Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RifleGrenadeParams {
    pub script: String,
    /// The projectile's model (a file of the pak; the scripts name only
    /// the weapon's), drawn by `projectile_key` (`super::add_guns`).
    #[serde(default)]
    pub model: Option<String>,
    pub damage: f32,
    pub radius: f32,
    pub speed: f32,
    pub gravity: f32,
    pub fuse: f32,
}

/// `dods:rocket`'s parameters.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RocketParams {
    pub script: String,
    /// The projectile's model (a file of the pak; the scripts name only
    /// the weapon's), drawn by `projectile_key` (`super::add_guns`).
    #[serde(default)]
    pub model: Option<String>,
    pub damage: f32,
    pub radius: f32,
    pub shoulder_time: f32,
    pub unshoulder_time: f32,
}

/// A launcher's shouldering (on the weapon).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Shoulder {
    pub shouldered: bool,
    /// When the alt fire may act again (game seconds).
    pub ready_at: f64,
    pub shoulder_time: f32,
    pub unshoulder_time: f32,
}

impl Shoulder {
    /// The most a holder moves with it shouldered, units/s (measured).
    pub const MAX_SPEED: f32 = 50.0;
}

/// The hooks, on an app.
pub fn register_hooks(app: &mut App) {
    use chimera_sim::weapon::data::WeaponDataAppExt;
    app.register_weapon_hook::<GrenadeParams>(GRENADE_HOOK, |e, _, p| {
        let throwable = Throwable {
            count: 1,
            max: 1,
            pin_time: p.pin_time,
            // The release throws on its own tick (the frame's "strictly
            // after" made a hair early), the redraw waits the windup out
            // (`pins`).
            throw_delay: -1e-3,
            throw_time: super::super::TICK_INTERVAL as f32,
            redraw_time: p.redraw_time,
            fuse: p.fuse,
            throw: p.rule(),
            flight: flight(p.gravity),
            effect: GrenadeEffect::Blast(blast(p.damage, p.radius)),
            pin_sounds: Vec::new(),
            bounce_sound: None,
            water_sounds: None,
            pin: false,
            throw_at: None,
            redraw_at: None,
        };
        e.insert((
            throwable,
            ThrowOptions { game_launch: true, drop_primed: false, ..default() },
            DodsGrenade { params: p.clone(), pinned_at: None, was_pin: false },
        ));
    })
    .register_weapon_hook::<RifleGrenadeParams>(RIFLE_GRENADE_HOOK, |e, _, p| {
        e.insert(p.clone());
    })
    .register_weapon_hook::<RocketParams>(ROCKET_HOOK, |e, _, p| {
        if let Some(mut l) = e.get_mut::<Launcher>() {
            l.template.blast = Some(Arc::new(blast(p.damage, p.radius)));
        }
        e.insert(Shoulder { shouldered: false, ready_at: f64::MIN, shoulder_time: p.shoulder_time, unshoulder_time: p.unshoulder_time });
    })
    .add_observer(on_launch);
    // A drawn ladder deals only weapons that hurt, ordered by what its
    // power curve reads: each blast's damage at its centre.
    use chimera_sim::{minigame::power::PowerAppExt, weapon::harm::HarmAppExt};
    let hurts = |p: &chimera_sim::weapon::harm::HookParams| chimera_sim::weapon::harm::num(p, "damage").is_some_and(|d| d > 0.0);
    for hook in [GRENADE_HOOK, RIFLE_GRENADE_HOOK, ROCKET_HOOK] {
        app.add_harm_reader(hook, hurts).add_power_reader(hook, power_blast);
    }
}

/// A blast's damage at its centre, one a shot (the power curve's reader).
fn power_blast(p: &chimera_sim::weapon::harm::HookParams, at: &chimera_sim::minigame::power::ReadAt, s: &mut chimera_sim::minigame::power::GunStats) {
    s.damage = chimera_sim::weapon::harm::num(p, "damage").unwrap_or(0.0) * at.data.units.damage;
    s.pellets = 1;
}

/// Whether this machine makes projectiles (the server, single player).
fn makes(role: Option<&NetRole>) -> bool {
    role.is_none_or(|r| *r != NetRole::Client)
}

/// A grenade of `weapon` (id) by `thrower` at `start` (m) leaving at
/// `velocity`, going off `fuse` seconds on.
fn spawn_grenade(commands: &mut Commands, weapon: &'static str, thrower: Entity, start: Vec3, velocity: Vec3, flight: Flight, fuse: f32, effect: GrenadeEffect) {
    let spin = Vec3::new(flight.spin_pitch * 0.5, 0.0, flight.spin_roll);
    commands.spawn((
        Name::new(format!("Grenade {weapon}")),
        Transform::from_translation(start),
        Projectile::new(weapon, Some(thrower), velocity, flight, fuse.max(0.0), effect, spin),
        ShownItem(projectile_key(weapon)),
        BOUNCE,
    ));
}

/// After the throw frame: when each grenade's pin was pulled, the next
/// one's redraw held until the windup (`pin_time` from the press) is over,
/// and a grenade held past its fuse going off in the hand (at the eye).
#[allow(clippy::type_complexity)]
pub fn pins(
    mut grenades: Query<(&Weapon, &mut DodsGrenade, &mut Throwable, &mut WeaponState)>,
    owners: Query<(&Transform, &MovementState)>,
    clock: Res<SimClock>,
    role: Option<Res<NetRole>>,
    mut commands: Commands,
) {
    let now = clock.now;
    for (weapon, mut g, mut t, mut st) in &mut grenades {
        if t.pin && !g.was_pin {
            g.pinned_at = Some(now);
        }
        g.was_pin = t.pin;
        let Some(at) = g.pinned_at else { continue };
        // Thrown: the next is drawn once the windup is over too (the last
        // one's switch isn't held).
        let windup = at + g.params.pin_time as f64;
        if !t.pin && t.count > 0 && t.redraw_at.is_some_and(|r| r < windup) {
            t.redraw_at = Some(windup);
            st.next_primary = st.next_primary.max(windup);
        }
        if !t.pin || now + 1e-6 < at + g.params.fuse as f64 {
            continue;
        }
        // Held past the fuse: it goes off in the hand.
        t.pin = false;
        g.was_pin = false;
        t.count = t.count.saturating_sub(1);
        t.redraw_at = Some(now + t.throw_time as f64);
        st.next_primary = now + t.throw_time as f64;
        let Some(owner) = weapon.owner else { continue };
        if makes(role.as_deref())
            && let Ok((tr, state)) = owners.get(owner)
        {
            spawn_grenade(&mut commands, weapon.id, owner, tr.translation + state.eye_offset, Vec3::ZERO, t.flight, 0.0, t.effect.clone());
        }
    }
}

/// A DoD:S grenade leaving the hand (Chimera's `Launch`, the server's):
/// its projectile from the eye, with what is left of its fuse.
fn on_launch(l: On<Launch>, grenades: Query<(&Weapon, &DodsGrenade, &Throwable)>, clock: Res<SimClock>, mut commands: Commands) {
    let Ok((w, g, t)) = grenades.get(l.weapon) else { return };
    let p = &g.params;
    let left = p.fuse - g.pinned_at.map_or(0.0, |at| (clock.now - at) as f32);
    let velocity = p.velocity(l.yaw, l.pitch) + p.carried(l.yaw, l.pitch, l.velocity);
    spawn_grenade(&mut commands, w.id, l.thrower, l.eye, velocity, t.flight, left, t.effect.clone());
}

/// Each rifle grenade fired (the server's): a grenade from the eye along
/// the aim.
pub fn rifle_shots(
    mut events: MessageReader<WeaponEvent>,
    guns: Query<(&Weapon, &RifleGrenadeParams)>,
    mut commands: Commands,
) {
    for e in events.read() {
        if e.replay {
            continue;
        }
        let WeaponEventKind::Fired { origin, yaw, pitch, .. } = e.kind else { continue };
        let Ok((w, p)) = guns.get(e.weapon) else { continue };
        let dir = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0) * Vec3::NEG_Z;
        let effect = GrenadeEffect::Blast(blast(p.damage, p.radius));
        spawn_grenade(&mut commands, w.id, e.owner, origin, dir * p.speed * U, flight(p.gravity), p.fuse, effect);
    }
}

/// Before the weapon frame: the alt fire shoulders and lowers a launcher
/// in hand; lowered (or not in hand) it doesn't fire.
pub fn shoulders(
    owners: Query<(&Intent, &Inventory), Without<crate::map::interp::NetDrawn>>,
    mut launchers: Query<(Entity, &Weapon, &mut Shoulder, &mut WeaponState)>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    let dt = clock.dt() as f64;
    for (entity, w, mut sh, mut st) in &mut launchers {
        let held = w.owner.and_then(|o| owners.get(o).ok()).filter(|(_, inv)| inv.active == Some(entity));
        let mut s = *sh;
        match held {
            None => s.shouldered = false,
            Some((intent, _)) => {
                if intent.secondary && now + 1e-6 >= s.ready_at {
                    s.shouldered = !s.shouldered;
                    let wait = if s.shouldered { s.shoulder_time } else { s.unshoulder_time } as f64;
                    s.ready_at = now + wait;
                    if st.next_primary < s.ready_at {
                        st.next_primary = s.ready_at;
                    }
                }
                if !s.shouldered && st.next_primary < now + dt {
                    st.next_primary = now + dt;
                }
            }
        }
        if *sh != s {
            *sh = s;
        }
    }
}

/// Before the weapon frame: a launcher or rifle grenade reloads only by
/// the reload key (measured: the clip stays empty until it), never by
/// itself (`WeaponState::reload_held` unless the key is held).
#[allow(clippy::type_complexity)]
pub fn key_reloads(
    owners: Query<&Intent, Without<crate::map::interp::NetDrawn>>,
    mut weapons: Query<(&Weapon, &mut WeaponState), Or<(With<Shoulder>, With<RifleGrenadeParams>)>>,
) {
    for (w, mut st) in &mut weapons {
        let held = !w.owner.and_then(|o| owners.get(o).ok()).is_some_and(|i| i.reload);
        if st.reload_held != held {
            st.reload_held = held;
        }
    }
}

/// Source's explosion where a DoD:S blast went off (every machine that
/// draws), and its sound.
#[allow(clippy::too_many_arguments)]
pub fn looks(
    mut detonated: MessageReader<Detonated>,
    materials: Option<Res<crate::map::particles::ParticleMaterials>>,
    light: Option<Res<crate::map::LightField>>,
    mut particles: Option<ResMut<crate::map::particles::Particles>>,
    mut rng: Local<Option<crate::map::ParticleRng>>,
    mut play: MessageWriter<crate::map::PlaySound>,
    world: crate::map::particles::WorldTracer,
) {
    use source_engine::effects::explosions::{self, FxMaterials};
    let (Some(materials), Some(particles)) = (materials, particles.as_deref_mut()) else {
        detonated.clear();
        return;
    };
    let rng = rng.get_or_insert_with(|| crate::map::ParticleRng::new(0xD0D5_B1A5));
    let mats = FxMaterials::new(&materials.0);
    for d in detonated.read() {
        if d.kind != GrenadeKind::Blast || chimera_sim::weapon::impact::weapon_game(d.weapon) != Some(super::super::GAME) {
            continue;
        }
        let a = explosions::blast_direction(d.at, &world);
        let l = explosions::light_at(light.as_deref(), d.at + a * 32.0 * U);
        for g in explosions::explosion(rng, &mats, d.at, a, l, &world, d.flags) {
            particles.add(g);
        }
        play.write(crate::map::PlaySound::at("BaseExplosionEffect.Sound", d.at));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frag() -> GrenadeParams {
        GrenadeParams {
            script: "weapon_frag_us".into(),
            model: None,
            damage: 150.0,
            radius: 375.0,
            fuse: 5.1,
            gravity: 420.0,
            lift: 10.0,
            scale_up: 80.0 / 90.0,
            scale_down: 100.0 / 90.0,
            speed_knee: 22.5,
            speed_per_deg: 250.0 / 67.5,
            speed_min: 500.0,
            speed_max: 1400.0,
            carry: 0.5,
            pin_time: 0.667,
            redraw_time: 0.667,
        }
    }

    /// weapons.md, "Grenades": the measured throws by aim.
    #[test]
    fn the_throw_follows_the_measured_table() {
        let p = frag();
        // (aim, + down; speed; elevation, + up)
        for (aim, speed, elev) in [
            (-89.0f32, 750.0, 89.1),
            (-80.0, 716.0, 81.1),
            (-70.0, 677.0, 72.2),
            (-60.0, 640.0, 63.3),
            (-50.0, 600.0, 54.4),
            (-45.0, 592.0, 50.0),
            (-40.0, 566.0, 45.6),
            (-30.0, 530.0, 36.7),
            (-25.0, 509.0, 32.2),
            (-15.0, 500.0, 23.3),
            (0.0, 500.0, 10.0),
            (15.0, 500.0, -6.7),
            (30.0, 500.0, -23.3),
            (89.0, 500.0, -88.9),
        ] {
            let v = p.velocity(0.0, (-aim).to_radians()) / U;
            let e = (v.y / v.length()).asin().to_degrees();
            assert!((v.length() - speed).abs() < 10.0, "aim {aim}: {} against {speed}", v.length());
            assert!((e - elev).abs() < 1.0, "aim {aim}: {e} against {elev}");
        }
    }

    /// damage.md, "Explosions": 150 × (1 − r / 375).
    #[test]
    fn the_blast_is_linear() {
        let b = blast(150.0, 375.0);
        let at = |r: f32| chimera_sim::weapon::grenade::blast_damage(b.damage, b.radius, r * U) / HP;
        assert!((at(200.0) - 70.0).abs() < 0.01);
        assert_eq!(at(400.0), 0.0);
    }

    /// weapons.md, "Grenades": half the thrower's velocity along the aim,
    /// none across it (running at 220: ≈608 forward, ≈367 backward, ≈492
    /// sideways, along the aim).
    #[test]
    fn the_thrower_carries_half_along_the_aim() {
        let p = frag();
        let along = |run: Vec3| ((p.velocity(0.0, 0.0) + p.carried(0.0, 0.0, run * 220.0 * U)) / U).with_y(0.0);
        assert!((along(Vec3::NEG_Z).length() - 602.4).abs() < 1.0, "{}", along(Vec3::NEG_Z).length());
        assert!((along(Vec3::Z).length() - 382.4).abs() < 1.0, "{}", along(Vec3::Z).length());
        assert!(along(Vec3::X).x.abs() < 1e-3, "nothing across");
    }
}
