//! `+use` carry (specs/hl2dm/gravity_gun.md 8, "With `hl2mp_allow_pickup`
//! 1"; measured on the probe 2026-11-04, the spec's 12): with
//! `hl2dm:hl2mp_allow_pickup` 1 (`CarryRules`; 0 by default, when `+use`
//! at a prop only clicks: `physics::use_props`) a fresh `+use` press at a
//! prop that passes the carry test (at most 35 kg and 128 units a side,
//! movable or spawnflag 64, not 512, not what the player stands on) picks
//! it up (`pick_up`): `Carrying` on the player, the prop held on Chimera's
//! held bodies (`map::held`) as the gravity gun holds (24 + 22.627 + its
//! depth ahead along the view, pitch clamped to 75, 1 kg, damping 10, at
//! most 1000 in/s) but in the player's **yaw-only** frame (it keeps its
//! attitude when the player looks up or down), each of its axes within
//! 30 degrees of a frame axis snapped onto it at the grab; its influence
//! set (the thrower's credit, `physics::credit`). While carrying (`carry`,
//! each tick after the weapons): the weapon in hand stays but can't fire
//! (its timers held off), sprint is off (`movement::select_speed`);
//! secondary, a fresh `+use`, the error past 12 after the first second,
//! the object gone or pinned, standing on it or dying drop it (speed cut to
//! 285 in/s, spin to 720 deg/s; zeroed instead when the object touches
//! anything at that moment, as the spec says); primary throws it: the same cut, then
//! `player_throwforce` (1000) × k along the view at its centre, k = 0.5 +
//! 3.5 (clamp(m, 0.5, 15) − 0.5) / 14.5, and k × U(−10, 10) deg/s of spin
//! on each axis. Either way the weapon in hand is deployed again (its
//! draw time from now, measured: the SMG1's next attack 0.8333 s on at
//! the throw). Data: data/use_carry.toml.
//!
//! Prediction: the weapon side of the carry is `Carried` (a predicted
//! part, sent with the snapshot) and `carry_weapon` (`Predict::Weapons`,
//! after the weapon frame, on the server and on the predicting client): the
//! weapon's timers held off, and at the throw or a drop by secondary or a
//! fresh `+use` the deploy; `movement::select_speed` reads `Carried` too, so
//! the sprint stays off (190). The weapon is holstered in the spec's words,
//! so the view model (`ViewModelHidden`, kept in step with `Carried` by
//! `show_hands`) and the weapon-selection HUD (`ui`) are hidden while
//! carrying. What only the server knows (the object, the error past 12, the
//! ground, death) still ends a carry there, and the client learns it in the
//! next snapshot; so does the pick-up itself.
//!
//! Known shortcuts: the throw's spin reads the spec's random torque as
//! degrees per second (its units aren't given); a client doesn't predict the
//! pick-up (which prop is the server's lookup).

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{
    Inventory, ViewModelHidden, Weapon, WeaponEvent, WeaponEventKind, WeaponFrame, WeaponState, random::shared_random,
};

use super::weapons::{
    launch::{self, U},
    physcannon::{extent, smooth_error},
};
use crate::{
    console::resource_cvar,
    core::{Dead, FirstTimePredicted, Health, Intent, MovementState, Predict, PredictedAppExt, SOLID_LAYERS, SimClock, SimSet},
    map::{
        PhysicsProp, PropEntity,
        held::{self, Held, PhysicsInfluence},
    },
};

/// data/use_carry.toml.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CarryParams {
    pub max_mass: f32,
    pub max_size: f32,
    pub hold_base: f32,
    pub player_radius: f32,
    pub pitch_clamp: f32,
    pub hold_speed: f32,
    pub hold_angular: f32,
    pub held_mass: f32,
    pub held_rot_damping: f32,
    pub error_limit: f32,
    pub error_grace: f32,
    pub drop_speed: f32,
    pub drop_spin: f32,
    pub snap_degrees: f32,
    pub throw_mass: [f32; 2],
    pub throw_k: [f32; 2],
    pub throw_spin: f32,
}

pub const PARAMS: &str = include_str!("../../data/use_carry.toml");

impl Default for CarryParams {
    fn default() -> Self {
        toml::from_str(PARAMS).expect("data/use_carry.toml")
    }
}

/// The server's cvars: `hl2mp_allow_pickup` (0: `+use` carries nothing)
/// and `player_throwforce`.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct CarryRules {
    pub allow_pickup: u8,
    pub throw_force: f32,
    pub params: CarryParams,
}

impl Default for CarryRules {
    fn default() -> Self {
        Self {
            allow_pickup: 0,
            throw_force: 1000.0,
            params: CarryParams::default(),
        }
    }
}

/// On a player carrying `object` by `+use`.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Carrying {
    pub object: Entity,
    /// Its rotation after the player's yaw-only frame (snapped at the grab).
    pub relative: Quat,
    /// Its box centre in its own frame, and its box.
    pub anchor: Vec3,
    pub bounds: (Vec3, Vec3),
    pub error: f32,
    pub error_clock: f64,
    pub last_target: Option<Vec3>,
    /// `+use` was down on the last tick (a fresh press drops).
    pub use_was: bool,
}

/// On a player carrying by `+use`, the part the client predicts (the
/// weapon held off, the deploy at the end, no sprint) and the server sends.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Carried {
    /// `+use` was down on the last tick (a fresh press drops).
    pub use_was: bool,
}

/// k of the throw (8 step 5) for an object of `mass` kg.
pub fn throw_k(p: &CarryParams, mass: f32) -> f32 {
    let [lo, hi] = p.throw_mass;
    let [k0, k1] = p.throw_k;
    k0 + (mass.clamp(lo, hi) - lo) * (k1 - k0) / (hi - lo)
}

/// The throw's speed (in/s) along the view on an object of `mass` kg.
pub fn throw_speed(rules: &CarryRules, mass: f32) -> f32 {
    rules.throw_force * throw_k(&rules.params, mass) / mass.max(1e-3)
}

/// `rot` with each of its axes within `degrees` of an axis of the frame
/// snapped onto it (8 step 2): two or three such axes make it the frame's
/// nearest axis-aligned rotation, one turns that axis onto its frame axis.
pub fn snap(rot: Quat, degrees: f32) -> Quat {
    let cos = degrees.to_radians().cos();
    let frame = [Vec3::X, Vec3::Y, Vec3::Z, Vec3::NEG_X, Vec3::NEG_Y, Vec3::NEG_Z];
    let axes = [rot * Vec3::X, rot * Vec3::Y, rot * Vec3::Z];
    let near: Vec<Option<Vec3>> = axes
        .iter()
        .map(|a| frame.iter().copied().find(|f| a.dot(*f) >= cos))
        .collect();
    let snapped: Vec<usize> = (0..3).filter(|i| near[*i].is_some()).collect();
    match snapped.len() {
        0 => rot,
        1 => {
            let i = snapped[0];
            (Quat::from_rotation_arc(axes[i], near[i].unwrap()) * rot).normalize()
        }
        _ => {
            let (i, j) = (snapped[0], snapped[1]);
            let (a, b) = (near[i].unwrap(), near[j].unwrap());
            let mut cols = [Vec3::ZERO; 3];
            cols[i] = a;
            cols[j] = b;
            // The third axis completes a right-handed frame.
            let k = 3 - i - j;
            cols[k] = cols[(k + 1) % 3].cross(cols[(k + 2) % 3]);
            Quat::from_mat3(&Mat3::from_cols(cols[0], cols[1], cols[2])).normalize()
        }
    }
}

/// The player's yaw-only frame.
fn yaw_frame(intent: &Intent) -> Quat {
    chimera_sim::weapon::aim_of(intent.yaw, 0.0)
}

/// `who` picks up `body` by `+use` (8 steps 1-2).
pub fn pick_up(world: &mut World, who: Entity, body: Entity) {
    let Some(rules) = world.get_resource::<CarryRules>().cloned() else { return };
    let p = rules.params;
    if world.get::<Carrying>(who).is_some() || world.get::<Held>(body).is_some() {
        return;
    }
    let Some(intent) = world.get::<Intent>(who).cloned() else { return };
    crate::map::prop_physics::enable_motion(world, body);
    crate::map::prop_physics::wake(world, body);
    let now = world.resource::<SimClock>().now;
    world.entity_mut(body).insert(PhysicsInfluence { by: who, at: now });
    let rot = world.get::<Transform>(body).map_or(Quat::IDENTITY, |t| t.rotation);
    let bounds = world
        .get::<PhysicsProp>(body)
        .map(|x| x.bounds)
        .unwrap_or((Vec3::splat(-4.0 * U), Vec3::splat(4.0 * U)));
    let frame = yaw_frame(&intent);
    let relative = snap(frame.inverse() * rot, p.snap_degrees);
    let dt = world.resource::<SimClock>().dt();
    // The load weight: its mass before the hold.
    let load = world
        .get::<avian3d::prelude::ComputedMass>(body)
        .map(|m| m.value())
        .filter(|m| m.is_finite() && *m > 0.0)
        .or(world.get::<PhysicsProp>(body).map(|x| x.mass))
        .unwrap_or(1.0);
    held::hold(
        world,
        body,
        Held::new(who, dt, p.hold_speed * U, p.hold_angular.to_radians(), load),
        p.held_mass,
        p.held_rot_damping,
    );
    world.entity_mut(who).insert(Carried { use_was: true });
    world.entity_mut(who).insert(Carrying {
        object: body,
        relative,
        anchor: (bounds.0 + bounds.1) / 2.0,
        bounds,
        error: 0.0,
        error_clock: -(p.error_grace as f64),
        last_target: None,
        use_was: true,
    });
    hold_off_weapon(world, who);
}

/// How a carry ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Release {
    Drop,
    /// Along the view `dir` (engine space, unit), its spin's random
    /// stream from `seed`.
    Throw { dir: Vec3, seed: u32 },
}

/// The carry of `who` ends (8 steps 4-6).
pub fn release(world: &mut World, who: Entity, how: Release) {
    let Some(c) = world.get::<Carrying>(who).cloned() else { return };
    world.entity_mut(who).remove::<Carrying>();
    // The predicted side may have deployed the weapon already (`carry_weapon`).
    let deploy = world.entity_mut(who).take::<Carried>().is_some();
    let touching = touches(world, c.object);
    let rules = world.get_resource::<CarryRules>().cloned().unwrap_or_default();
    let p = &rules.params;
    if world.get_entity(c.object).is_ok() {
        held::release(world, c.object);
        let now = world.resource::<SimClock>().now;
        world.entity_mut(c.object).insert(PhysicsInfluence { by: who, at: now });
        let mut b = world.entity_mut(c.object);
        let mass = b.get::<Mass>().map_or(1.0, |m| m.0);
        let v = b.get::<LinearVelocity>().map_or(Vec3::ZERO, |v| v.0).clamp_length_max(p.drop_speed * U);
        let w = b.get::<AngularVelocity>().map_or(Vec3::ZERO, |w| w.0).clamp_length_max(p.drop_spin.to_radians());
        let (v, w) = match how {
            Release::Drop if touching => (Vec3::ZERO, Vec3::ZERO),
            Release::Drop => (v, w),
            Release::Throw { dir, seed } => {
                let k = throw_k(p, mass);
                let mut rng = shared_random(seed, "hl2dm:use_throw_spin", 0);
                let s = p.throw_spin;
                let spin = Vec3::new(rng.range(-s, s), rng.range(-s, s), rng.range(-s, s)) * k;
                (v + dir * throw_speed(&rules, mass) * U, w + spin * std::f32::consts::PI / 180.0)
            }
        };
        if let Some(mut x) = b.get_mut::<LinearVelocity>() {
            x.0 = v;
        }
        if let Some(mut x) = b.get_mut::<AngularVelocity>() {
            x.0 = w;
        }
    }
    if deploy {
        redeploy(world, who);
    }
}

/// Whether `body` touches anything that isn't a sensor (8 step 4).
fn touches(world: &World, body: Entity) -> bool {
    let Some(graph) = world.get_resource::<ContactGraph>() else { return false };
    graph.iter_active_touching().chain(graph.iter_sleeping_touching()).any(|pair| {
        let other = if pair.body1 == Some(body) {
            pair.collider2
        } else if pair.body2 == Some(body) {
            pair.collider1
        } else {
            return false;
        };
        world.get::<Sensor>(other).is_none()
    })
}

/// While carrying, the weapon in hand can't fire: its timers held a tick
/// ahead.
fn hold_off_weapon(world: &mut World, who: Entity) {
    let now = world.resource::<SimClock>().now;
    let ahead = now + 2.0 * world.resource::<SimClock>().dt() as f64;
    let Some(mut inv) = world.get_mut::<Inventory>(who) else { return };
    inv.next_attack = inv.next_attack.max(ahead);
    let Some(active) = inv.active else { return };
    if let Some(mut st) = world.get_mut::<WeaponState>(active) {
        st.next_primary = st.next_primary.max(ahead);
        st.next_secondary = st.next_secondary.max(ahead);
    }
}

/// The weapon in hand deployed again (8 step 6): usable its draw time
/// from now.
fn redeploy(world: &mut World, who: Entity) {
    let now = world.resource::<SimClock>().now;
    let Some(active) = world.get::<Inventory>(who).and_then(|i| i.active) else { return };
    let Some(draw) = world.get::<Weapon>(active).map(|w| w.draw_time) else { return };
    let ready = now + draw as f64;
    if let Some(mut inv) = world.get_mut::<Inventory>(who) {
        inv.next_attack = ready;
    }
    if let Some(mut st) = world.get_mut::<WeaponState>(active) {
        st.next_primary = ready;
        st.next_secondary = ready;
        st.fired_on_empty = false;
    }
    world.write_message(WeaponEvent {
        owner: who,
        weapon: active,
        kind: WeaponEventKind::Deployed,
        replay: false,
    });
}

/// Each tick, after the weapon frame, the server's and the predicting
/// client's (`Predict::Weapons`): the weapon of a player carrying by `+use`
/// held off, or deployed again when secondary, a fresh `+use` or primary
/// ends the carry (8 steps 3 and 6; the object itself is `carry`'s).
#[allow(clippy::type_complexity)]
pub fn carry_weapon(
    mut carriers: Query<(Entity, &mut Carried, &Intent, &mut Inventory)>,
    mut weapons: Query<(&Weapon, &mut WeaponState)>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    mut events: MessageWriter<WeaponEvent>,
    mut commands: Commands,
) {
    let now = clock.now;
    let ahead = now + 2.0 * clock.dt() as f64;
    for (who, mut c, intent, mut inv) in &mut carriers {
        let fresh_use = intent.use_key && !c.use_was;
        if c.use_was != intent.use_key {
            c.use_was = intent.use_key;
        }
        let ends = intent.secondary || fresh_use || intent.fire;
        let active = inv.active;
        let draw = active.and_then(|a| weapons.get(a).ok()).map(|(w, _)| w.draw_time as f64);
        if ends {
            commands.entity(who).remove::<Carried>();
            let Some((active, draw)) = active.zip(draw) else { continue };
            inv.next_attack = now + draw;
            if let Ok((_, mut st)) = weapons.get_mut(active) {
                st.next_primary = now + draw;
                st.next_secondary = now + draw;
                st.fired_on_empty = false;
            }
            if first.0 {
                events.write(WeaponEvent { owner: who, weapon: active, kind: WeaponEventKind::Deployed, replay: false });
            }
        } else {
            inv.next_attack = inv.next_attack.max(ahead);
            if let Some((_, mut st)) = active.and_then(|a| weapons.get_mut(a).ok()) {
                st.next_primary = st.next_primary.max(ahead);
                st.next_secondary = st.next_secondary.max(ahead);
            }
        }
    }
}

/// The view model is hidden while carrying (the weapon is holstered).
pub fn show_hands(
    carrying: Query<Entity, (With<Carried>, Without<ViewModelHidden>)>,
    done: Query<Entity, (With<ViewModelHidden>, Without<Carried>)>,
    mut commands: Commands,
) {
    for e in &carrying {
        commands.entity(e).insert(ViewModelHidden);
    }
    for e in &done {
        commands.entity(e).remove::<ViewModelHidden>();
    }
}

/// Each tick, the server's, after the weapons (8 step 3): a carry's
/// drops and throw, else its target.
#[allow(clippy::type_complexity)]
pub fn carry(
    mut carriers: Query<(Entity, &mut Carrying, &Transform, &MovementState, &Intent, &Health, Has<Dead>)>,
    mut bodies: Query<(&mut Held, &Transform, Option<&RigidBody>, Has<ColliderDisabled>)>,
    statics: Query<(Option<&RigidBody>, Has<PropEntity>, Has<Intent>)>,
    of: Query<&ColliderOf>,
    rules: Option<Res<CarryRules>>,
    spatial: SpatialQuery,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let Some(rules) = rules else { return };
    let p = &rules.params;
    let dt = clock.dt();
    for (who, mut c, at, state, intent, health, dead) in &mut carriers {
        let fresh_use = intent.use_key && !c.use_was;
        c.use_was = intent.use_key;
        let view = chimera_sim::weapon::aim_of(intent.yaw, intent.pitch);
        let ok = bodies.get(c.object).ok().map(|(_, _, rb, gone)| (rb.is_some_and(|r| r.is_dynamic()), gone));
        if let Some(e_now) = c.last_target.zip(bodies.get(c.object).ok()).map(|(l, (_, t, ..))| l.distance(t.translation) / U)
            && c.error_clock > 0.0
        {
            c.error = smooth_error(c.error, c.error_clock, e_now);
            c.error_clock = 0.0;
        }
        let lost = dead || health.current <= 0.0 || ok.is_none_or(|(moving, gone)| !moving || gone);
        if lost || intent.secondary || fresh_use || c.error > p.error_limit || state.ground == Some(c.object) {
            commands.queue(move |w: &mut World| release(w, who, Release::Drop));
            continue;
        }
        if intent.fire {
            let dir = view * Vec3::NEG_Z;
            let seed = intent.command;
            commands.queue(move |w: &mut World| release(w, who, Release::Throw { dir, seed }));
            continue;
        }
        let Ok((mut held, t, ..)) = bodies.get_mut(c.object) else { continue };
        let eye = at.translation + state.eye_offset;
        let clamp = p.pitch_clamp.to_radians();
        let f = chimera_sim::weapon::aim_of(intent.yaw, intent.pitch.clamp(-clamp, clamp)) * Vec3::NEG_Z;
        let axis = launch::to_source(at.translation + (state.hull_min + state.hull_max) / 2.0);
        let axis = Vec2::new(axis.x, axis.y);
        let rot = (yaw_frame(intent) * c.relative).normalize();
        let e = extent(c.bounds, rot, -f) / U;
        let r = p.player_radius + e;
        let d = p.hold_base + 2.0 * r;
        let body = |x: Entity| of.get(x).map_or(x, |o| o.body);
        let world_only = |x: Entity| {
            statics
                .get(body(x))
                .is_ok_and(|(rb, prop, ch)| !prop && !ch && rb.is_none_or(|r| r.is_static()))
        };
        let filter = SpatialQueryFilter::from_mask(SOLID_LAYERS);
        let wall = Dir3::new(f)
            .ok()
            .and_then(|dir| spatial.cast_ray_predicate(eye, dir, d * U, true, &filter, &world_only))
            .map(|h| h.distance / U);
        let point = hold_point(p, launch::to_source(eye), launch::to_source(f).normalize_or_zero(), e, wall, axis);
        let origin = launch::to_engine(point) - rot * c.anchor;
        held.target = Some((origin, rot));
        held.arrive = dt;
        held.view_offset = Some((view.inverse() * (origin - eye), (view.inverse() * rot).normalize()));
        let _ = t;
        c.last_target = Some(origin);
        c.error_clock += dt as f64;
    }
}

/// The gravity gun's hold point (`physcannon::hold_point`) with this
/// carry's numbers.
fn hold_point(p: &CarryParams, eye: Vec3, fwd: Vec3, e: f32, wall: Option<f32>, axis: Vec2) -> Vec3 {
    let r = p.player_radius + e;
    let d = p.hold_base + 2.0 * r;
    let mut at = match wall {
        Some(w) if w / d < 0.5 => eye + fwd * r / 2.0,
        _ => eye + fwd * (d - r),
    };
    let flat = Vec2::new(at.x, at.y) - axis;
    if flat.length() < r {
        let out = flat.try_normalize().unwrap_or(Vec2::new(fwd.x, fwd.y).normalize_or(Vec2::X)) * r;
        at.x = axis.x + out.x;
        at.y = axis.y + out.y;
    }
    at
}

/// The carry's cvars and systems (`physics::Hl2PhysicsPlugin`).
pub fn plugin(app: &mut App) {
    app.init_resource::<CarryRules>();
    resource_cvar::<CarryRules, u8>(app, "hl2dm:hl2mp_allow_pickup", "1: +use picks up and carries small props.", |r| {
        &mut r.allow_pickup
    });
    resource_cvar::<CarryRules, f32>(app, "hl2dm:player_throwforce", "+use throw impulse base (kg in/s).", |r| {
        &mut r.throw_force
    });
    app.predicted_net::<Carried>()
        .add_systems(Predict::Weapons, carry_weapon.after(WeaponFrame))
        .add_systems(FixedUpdate, show_hands)
        .add_systems(
            FixedUpdate,
            carry.after(SimSet::Weapons).run_if(crate::core::authoritative),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gravity_gun.md 8 step 5: Δv of the throw by mass.
    #[test]
    fn throw_speeds() {
        let r = CarryRules::default();
        for (m, v) in [(1.0, 620.7), (5.0, 317.2), (10.0, 279.3), (15.0, 266.7), (20.0, 200.0), (35.0, 114.3)] {
            assert!((throw_speed(&r, m) - v).abs() < 0.1, "{m} kg: {}", throw_speed(&r, m));
        }
    }

    /// Step 2: axes within 30 degrees snap; a 40-degree turn stays.
    #[test]
    fn axes_snap_within_30_degrees() {
        let q = snap(Quat::from_rotation_z(20f32.to_radians()), 30.0);
        assert!(q.angle_between(Quat::IDENTITY) < 1e-3, "{q}");
        let tilted = Quat::from_rotation_z(40f32.to_radians());
        // 40 about z: x and y are 40 off; z stays on its axis (one snap: none to do).
        assert!(snap(tilted, 30.0).angle_between(tilted) < 1e-3);
        let q = snap(Quat::from_rotation_z(100f32.to_radians()), 30.0);
        assert!(q.angle_between(Quat::from_rotation_z(90f32.to_radians())) < 1e-3, "{q}");
    }
}
