//! The gravity gun (`weapon_physcannon`, specs/hl2dm/gravity_gun.md 1-7,
//! 10-12, H5), `hl2dm:physcannon` (`Hl2Physcannon`), on Chimera's held
//! bodies (`map::held`, lift C6). Its frame (before the shared weapon
//! frame, which does nothing for it) is predicted for its timers, sounds
//! and view-model events; what it does to objects is the server's
//! (`makes_projectiles`).
//!
//! - **Punt** (primary, not holding; every 0.5 s while held, a dry fire
//!   too): a ±8 box, then a line, of 250 from the eye. A physics object
//!   (not a ragdoll) gets 15000 at its centre plus 600 × min(M, 250) at
//!   the hit point along the view (a downward view's z × −0.65), then the
//!   2000 in/s cap; a 0-damage hit naming the punter; its influence (the
//!   punter, now); a motion-disabled prop with spawnflag 64 is set moving
//!   first (without it the impulse does nothing); the same object again
//!   within 0.5 s: nothing. A damageable non-physics thing (a
//!   `func_breakable`) takes 1 crush. A player, the world or nothing: a dry
//!   fire. The fire's punch (U(1, 2), U(−1, 1) then −6, U{−2..2}, as
//!   HL2's view punch on its spring), `Weapon_PhysCannon.Launch`, next
//!   primary and secondary + 0.5, the claws closing 0.5 s later.
//! - **Grab or pull** (secondary, server): a line of 1000 (a ±4 box when
//!   it misses or meets the world), the hit grabbed within 250 and pulled
//!   farther (4000 toward the eye, × (M + 0.5)/50 under 50 kg, every 0.1
//!   s); else the nearest object within 251 in the 0.97 cone with a clear
//!   line is grabbed. What can't be held (a player, the world, the
//!   holder's ground, a ragdoll, someone's held object, over 250 kg,
//!   spawnflag 512, motion-disabled without 64) plays
//!   `Weapon_PhysCannon.TooHeavy` once a press. A grab: `.Pickup`, next
//!   secondary + 0.5 with the button masked until released, the object 1
//!   kg with angular damping 10 (`map::held::hold`), owned by the holder
//!   when it had no owner, its influence set, its rotation kept relative
//!   to the view; the gun can't be put away (`HolsterLocked`).
//! - **Holding** (`hold`, the server's, each tick): the target 24 +
//!   22.627 + the object's depth toward the eye ahead along the view
//!   (pitch clamped to ±75), or half its radius when the world is nearer
//!   than half that line, pushed out to the radius horizontally; the
//!   controller (Chimera's) brings it there each tick at ≤ 1000 in/s. The
//!   smoothed error (0.985 E + 0.015 e, e halved above 15) past 12, after
//!   the first second, drops it; so does the holder standing on it, the
//!   body gone or pinned.
//! - **Drop** (a fresh secondary press 0.5 s after the grab): speed cut to
//!   285 in/s and spin to 720 deg/s, `.Drop`, next primary and
//!   secondary 0.5 s on. **Launch** (primary, within 250): velocity and spin zeroed,
//!   then 1500 along the view (1500 − 800 S((min(m, 1000) − 100)/500) over
//!   100 kg) and U(−600, 600) deg/s a axis, the fire effects; farther
//!   than 250: a dry fire, no timer. Forced drops: `phys_swap`, the
//!   holder's death.
//! - **Claws** (predicted): a ±4 box of 250 finding something that can be
//!   held opens them (`.OpenClaws`); 0.5 s without one closes them
//!   (`.CloseClaws`); a punt closes them 0.5 s later, a launch 0.1 s,
//!   both suspending the check 0.25 s.
//! - **Special objects**: a frag grabbed or punted becomes the gun user's,
//!   its fuse 1.5 s from then with a blip and fast blips (11); a combine
//!   ball is always held (no mass limit), becomes the catcher's, passes
//!   through everything while held and explodes 8 s after the catch;
//!   launched it leaves at 959.2 (750 kg's speed) with 1500 again after its
//!   first contact and 4 s of life; dropped, at 1500 within ±55 whole
//!   degrees of the view (12).
//! - `phys_swap`: with the gun in hand, a forced drop and the last weapon;
//!   otherwise the gun.
//!
//! Known shortcuts: several bodies per object aren't split (every object
//! is one body; the mass shares of 2.5 are 1); the "launch spin" of a
//! sawblade, `preferred_carryangles`, the 10× first-impact damage and
//! first-impact breaks of props launched from the hold (9) aren't done;
//! the claws' view-model pose, the hold's idle shake (`ACT_VM_RELOAD`),
//! the white fade, the beams and glow and the hold loop are M10's or
//! cosmetic; the cone search's line for a ball checks only the world; held
//! items' grown touch box (11, Open questions 6) isn't done; the client
//! doesn't predict the launch's distance check (it predicts a launch) nor
//! a grab (the server's); a free combine ball can't be punted (it has no
//! collider for the punt's traces: only the grab's cone finds it); a held
//! ball's 8 s end is its explosion sound and removal (its blast is H4's,
//! not built); satchels and loose items are held as plain bodies.

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    HolsterLocked, Inventory, NetPart, Weapon, WeaponEvent, WeaponEventKind, WeaponState,
    projectile::{Flyer, ShownBody},
    random::shared_random,
};

use super::{
    alt_fire::makes_projectiles,
    frag::Hl2Grenade,
    launch::{self, U},
    recoil::{Hl2Punch, IMPULSE_SCALE},
};
use crate::{
    core::{Damage, DamageKind, Died, FirstTimePredicted, Health, Hitgroup, Intent, MovementState, NetRole, SOLID_LAYERS, SimClock},
    map::{
        MapBrushEntity, MapEntities, PhysicsProp, PlaySound, PropEntity, RagdollBody,
        held::{self, Held, PhysicsInfluence},
        interp::NetDrawn,
        prop_physics::{FrozenBody, StartAsleep},
    },
};

/// `hl2dm:physcannon`'s parameters (Source units, kg, seconds, degrees).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PhysParams {
    pub trace_length: f32,
    pub search_length: f32,
    pub max_mass: f32,
    pub max_force: f32,
    pub min_force: f32,
    pub pull_force: f32,
    pub cone: f32,
    pub punt_base: f32,
    pub punt_mass: f32,
    pub down_reflect: f32,
    pub punt_hull: f32,
    pub grab_hull: f32,
    pub refire: f32,
    pub search_refire: f32,
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
    pub launch_spin: f32,
    pub punch_pitch: f32,
    pub punch_yaw: i32,
    pub recoil_pitch: [f32; 2],
    pub recoil_yaw: [f32; 2],
    pub claws_close_delay: f32,
    pub punt_claws: f32,
    pub launch_claws: f32,
    pub target_suppress: f32,
    pub frag_fuse: f32,
    pub ball_life: f32,
    pub ball_speed: f32,
    pub ball_mass: f32,
    pub ball_held_mass: f32,
    pub ball_flight_life: f32,
    pub ball_drop_spread: i32,
    pub launch_sound: String,
    pub dry_sound: String,
    pub pickup_sound: String,
    pub drop_sound: String,
    pub open_sound: String,
    pub close_sound: String,
    pub too_heavy_sound: String,
    pub ball_hold_sound: String,
    pub ball_launch_sound: String,
}

/// The view-model actions (`WeaponEventKind::Action`): a punt or launch
/// (`ACT_VM_SECONDARYATTACK`), a grab or drop (`ACT_VM_PRIMARYATTACK`).
pub const PUNT: &str = "physcannon_punt";
pub const PICKUP: &str = "physcannon_pickup";

/// The gun's state: what the holder's client predicts (`NetPart`) and the
/// server's hold.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Hl2Physcannon {
    pub params: PhysParams,
    pub holding: bool,
    /// Secondary is masked from a grab until let go.
    pub masked: bool,
    /// TooHeavy played this press.
    pub denied: bool,
    pub secondary_was: bool,
    pub claws: bool,
    pub claws_close_at: Option<f64>,
    /// The claws have had no target since.
    pub lost_since: Option<f64>,
    pub suppress_until: f64,
    /// The server's: what it holds, the object last punted or launched
    /// and until when it can't be again.
    pub object: Option<Entity>,
    pub punted: Option<(Entity, f64)>,
    /// The held object's rotation after the view's at the grab, its box
    /// centre in its own frame, its depth from its origin along each
    /// local axis.
    pub relative: Quat,
    pub anchor: Vec3,
    pub bounds: (Vec3, Vec3),
    pub error: f32,
    pub error_clock: f64,
    pub last_target: Option<Vec3>,
    /// The gun made the holder its owner.
    pub owned: bool,
}

impl Hl2Physcannon {
    pub fn new(params: PhysParams) -> Self {
        Self {
            params,
            holding: false,
            masked: false,
            denied: false,
            secondary_was: false,
            claws: false,
            claws_close_at: None,
            lost_since: None,
            suppress_until: 0.0,
            object: None,
            punted: None,
            relative: Quat::IDENTITY,
            anchor: Vec3::ZERO,
            bounds: (Vec3::ZERO, Vec3::ZERO),
            error: 0.0,
            error_clock: -1.0,
            last_target: None,
            owned: false,
        }
    }
}

impl NetPart for Hl2Physcannon {
    type Net = (bool, bool, bool, bool, bool, Option<f64>, Option<f64>, f64);
    fn net(&self) -> Self::Net {
        (
            self.holding,
            self.masked,
            self.denied,
            self.secondary_was,
            self.claws,
            self.claws_close_at,
            self.lost_since,
            self.suppress_until,
        )
    }
    fn set_net(&mut self, n: Self::Net) {
        (
            self.holding,
            self.masked,
            self.denied,
            self.secondary_was,
            self.claws,
            self.claws_close_at,
            self.lost_since,
            self.suppress_until,
        ) = n;
    }
}

/// A combine ball in a gun: who holds it and when it goes off.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct HeldBall {
    pub holder: Entity,
    pub explode_at: f64,
}

/// Δv (in/s) of a punt on a single body of `mass` kg hit through its
/// centre (2): (15000 + 600 min(m, 250)) / m, before the speed cap.
pub fn punt_speed(p: &PhysParams, mass: f32) -> f32 {
    (p.punt_base + p.punt_mass * mass.min(p.max_mass)) / mass.max(1e-3)
}

/// A punt's direction (2 step 3): the view's, a downward z × −0.65
/// (engine: y up), not renormalised.
pub fn punt_dir(p: &PhysParams, fwd: Vec3) -> Vec3 {
    let mut d = fwd;
    if d.y < 0.0 {
        d.y *= p.down_reflect;
    }
    d
}

/// The pull impulse's size on an object of `mass` kg (3).
pub fn pull_impulse(p: &PhysParams, mass: f32) -> f32 {
    p.pull_force * if mass < 50.0 { (mass + 0.5) / 50.0 } else { 1.0 }
}

/// A launch's speed (7) for an object of `mass` kg (the unclamped spline).
pub fn launch_speed(p: &PhysParams, mass: f32) -> f32 {
    if mass <= 100.0 {
        return p.max_force;
    }
    let x = (mass.min(1000.0) - 100.0) / 500.0;
    p.max_force - (p.max_force - p.min_force) * (3.0 * x * x - 2.0 * x * x * x)
}

/// The smoothed error's update (5): `clock` seconds since the last,
/// `now` the error now (units).
pub fn smooth_error(error: f32, clock: f64, now: f32) -> f32 {
    if clock <= 0.0 {
        return error;
    }
    let t = clock.min(1.0) as f32;
    let e = if now / t > 1000.0 { now / 2.0 } else { now };
    (1.0 - t) * error + t * e
}

/// Where a held object's box centre goes (5, steps 1-5), Source units:
/// from the eye along the clamped view `fwd` (unit), the object's extent
/// `e` toward the eye, the world's distance along that line (None: clear
/// within its length) and the player's vertical axis `axis` (x, y).
pub fn hold_point(p: &PhysParams, eye: Vec3, fwd: Vec3, e: f32, wall: Option<f32>, axis: Vec2) -> Vec3 {
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

/// The box `(lo, hi)` (its own frame) turned by `rot`: how far its
/// farthest corner reaches from its origin along `dir`.
pub fn extent(bounds: (Vec3, Vec3), rot: Quat, dir: Vec3) -> f32 {
    let (lo, hi) = bounds;
    (0..8)
        .map(|k| {
            let c = Vec3::new([lo.x, hi.x][k & 1], [lo.y, hi.y][(k >> 1) & 1], [lo.z, hi.z][(k >> 2) & 1]);
            (rot * c).dot(dir)
        })
        .fold(0.0, f32::max)
}

/// What a trace or search found.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Found {
    /// Something that moves by physics (or a pinned prop): a body.
    Body(Entity),
    Ball(Entity),
    /// Damageable but not physics (a breakable brush, a trip mine).
    Damageable(Entity),
    Player(Entity),
    World,
}


/// A prop's spawnflags the gun reads (physics_props.md: 64 "enable motion
/// on gravity gun", 512 "no player pick-up").
pub const SF_ENABLE_ON_GUN: u32 = 64;
pub const SF_NO_PICKUP: u32 = 512;

/// The objects the gun can meet, and what they are.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Objects<'w, 's> {
    #[allow(clippy::type_complexity)]
    bodies: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            Option<&'static RigidBody>,
            Option<&'static ComputedMass>,
            Option<&'static PhysicsProp>,
            Option<&'static FrozenBody>,
            Option<&'static PropEntity>,
            Has<Held>,
            Has<RagdollBody>,
            Has<StartAsleep>,
        ),
        Without<Intent>,
    >,
    characters: Query<'w, 's, (), With<Intent>>,
    healthy: Query<'w, 's, (), (With<Health>, Without<Intent>)>,
    brushes: Query<'w, 's, (), With<MapBrushEntity>>,
    of: Query<'w, 's, &'static ColliderOf>,
    flyers: Query<'w, 's, (Entity, &'static Flyer, &'static Transform, Has<HeldBall>)>,
    entities: Option<Res<'w, MapEntities>>,
}

impl Objects<'_, '_> {
    fn body(&self, collider: Entity) -> Entity {
        self.of.get(collider).map_or(collider, |o| o.body)
    }

    /// A map prop's spawnflags (0 for anything else).
    pub fn flags(&self, e: Entity) -> u32 {
        let Ok((.., Some(index), _, _, _)) = self.bodies.get(e) else { return 0 };
        self.entities
            .as_ref()
            .and_then(|m| m.entities.get(index.0))
            .and_then(|e| e.get("spawnflags"))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    }

    fn classify(&self, collider: Entity) -> Found {
        let e = self.body(collider);
        if self.characters.contains(e) {
            return Found::Player(e);
        }
        if let Ok((_, _, rb, _, prop, frozen, ..)) = self.bodies.get(e)
            && (rb.is_some_and(|r| r.is_dynamic()) || prop.is_some() || frozen.is_some())
        {
            return Found::Body(e);
        }
        if self.brushes.contains(e) || self.healthy.contains(e) {
            return Found::Damageable(e);
        }
        Found::World
    }

    /// A body's mass (kg) now.
    pub fn mass(&self, e: Entity) -> f32 {
        match self.bodies.get(e) {
            Ok((_, _, _, m, prop, frozen, ..)) => m
                .map(|m| m.value())
                .filter(|m| m.is_finite() && *m > 0.0)
                .or(prop.map(|p| p.mass))
                .or(frozen.map(|f| f.physics.mass))
                .unwrap_or(1.0),
            Err(_) => 1.0,
        }
    }

    /// A body's box centre (engine space).
    fn centre(&self, e: Entity) -> Option<Vec3> {
        let (_, t, _, _, prop, frozen, ..) = self.bodies.get(e).ok()?;
        let b = prop.map(|p| p.bounds).or(frozen.map(|f| f.bounds));
        Some(t.translation + b.map_or(Vec3::ZERO, |b| t.rotation * (b.0 + b.1) / 2.0))
    }

    fn ragdoll(&self, e: Entity) -> bool {
        self.bodies.get(e).is_ok_and(|b| b.8)
    }

    /// Whether the gun may hold what was found, for a player standing on
    /// `ground` (3 step 4); a ball always, unless held.
    fn can_hold(&self, p: &PhysParams, f: Found, ground: Option<Entity>) -> bool {
        match f {
            Found::Ball(b) => self.flyers.get(b).is_ok_and(|x| !x.3),
            Found::Body(e) => {
                let Ok((_, _, rb, _, prop, frozen, _, held, ragdoll, asleep)) = self.bodies.get(e) else { return false };
                if held || ragdoll || ground == Some(e) {
                    return false;
                }
                let flags = self.flags(e);
                if flags & SF_NO_PICKUP != 0 {
                    return false;
                }
                // A prop that started asleep is woken by the grab.
                let pinned = frozen.is_some() || (rb.is_none_or(|r| !r.is_dynamic()) && !asleep);
                if pinned && ((prop.is_none() && frozen.is_none()) || flags & SF_ENABLE_ON_GUN == 0) {
                    return false;
                }
                self.mass(e) <= p.max_mass + 1e-3
            }
            _ => false,
        }
    }
}

/// The gun's traces see debris props too (interactive in HL2:DM, 11), on
/// the ragdoll layer, but not ragdolls.
const GUN_MASK: LayerMask = LayerMask(SOLID_LAYERS.0 | crate::core::RAGDOLL_LAYER.0);

/// The first thing a ray (`half` 0) or box from `eye` along `dir` meets
/// within `len` meters, leaving out `owner` and ragdolls: (collider,
/// distance, point).
fn trace(o: &Objects, spatial: &SpatialQuery, eye: Vec3, dir: Dir3, len: f32, half: f32, owner: Entity) -> Option<(Entity, f32, Vec3)> {
    let filter = SpatialQueryFilter::from_excluded_entities([owner]).with_mask(GUN_MASK);
    let keep = |c: Entity| !o.ragdoll(o.body(c));
    if half <= 0.0 {
        return spatial
            .cast_ray_predicate(eye, dir, len, true, &filter, &keep)
            .map(|h| (h.entity, h.distance, eye + *dir * h.distance));
    }
    let shape = Collider::cuboid(half * 2.0, half * 2.0, half * 2.0);
    let config = ShapeCastConfig {
        max_distance: len,
        ..ShapeCastConfig::DEFAULT
    };
    spatial
        .cast_shape_predicate(&shape, eye, Quat::IDENTITY, dir, &config, &filter, &keep)
        .map(|h| (h.entity, h.distance, h.point1))
}

/// The punt's target (2 step 2): the box's hit if it is something, else
/// the line's.
fn punt_target(spatial: &SpatialQuery, o: &Objects, p: &PhysParams, eye: Vec3, dir: Dir3, owner: Entity) -> Option<(Found, Vec3)> {
    for half in [p.punt_hull * U, 0.0] {
        if let Some((c, _, point)) = trace(o, spatial, eye, dir, p.trace_length * U, half, owner) {
            let f = o.classify(c);
            if f != Found::World {
                return Some((f, point));
            }
        }
    }
    None
}

/// The grab search (3): what was found and whether it is near enough to
/// grab (else it is pulled).
fn search(spatial: &SpatialQuery, o: &Objects, p: &PhysParams, eye: Vec3, dir: Dir3, owner: Entity) -> Option<(Found, bool)> {
    let len = p.search_length * U;
    let line = trace(o, spatial, eye, dir, len, 0.0, owner).map(|(c, d, _)| (o.classify(c), d));
    let hit = match line {
        Some((Found::World, _)) | None => trace(o, spatial, eye, dir, len, p.grab_hull * U, owner).map(|(c, d, _)| (o.classify(c), d)),
        some => some,
    };
    if let Some((f, d)) = hit
        && f != Found::World
    {
        return Some((f, d <= p.trace_length * U + 1e-4));
    }
    // The cone: the nearest within 251 whose centre is in it, in sight.
    let reach = (p.trace_length + 1.0) * U;
    let mut best: Option<(Found, f32)> = None;
    let mut consider = |f: Found, c: Vec3, clear: &dyn Fn() -> bool| {
        let to = c - eye;
        let d = to.length();
        if d >= reach || d < 1e-4 || to.dot(*dir) / d <= p.cone || best.is_some_and(|(_, b)| d >= b) || !clear() {
            return;
        }
        best = Some((f, d));
    };
    for (e, _, rb, _, prop, frozen, ..) in &o.bodies {
        if !(rb.is_some_and(|r| r.is_dynamic()) || prop.is_some() || frozen.is_some()) {
            continue;
        }
        let Some(c) = o.centre(e) else { continue };
        consider(Found::Body(e), c, &|| {
            Dir3::new(c - eye).is_ok_and(|to| {
                trace(o, spatial, eye, to, (c - eye).length() + 1e-3, 0.0, owner).is_none_or(|(h, ..)| o.body(h) == e)
            })
        });
    }
    for (b, f, t, _) in &o.flyers {
        if f.weapon != super::AR2 {
            continue;
        }
        let c = t.translation;
        consider(Found::Ball(b), c, &|| {
            Dir3::new(c - eye).is_ok_and(|to| {
                trace(o, spatial, eye, to, (c - eye).length(), 0.0, owner).is_none_or(|(h, ..)| o.classify(h) != Found::World)
            })
        });
    }
    best.map(|(f, _)| (f, true))
}

/// How a hold ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LetGo {
    /// A secondary press (6) or a forced drop: speed and spin capped.
    Drop,
    /// Primary (7): zeroed, then the launch along `dir` with spin from
    /// `seed`.
    Launch { dir: Vec3, seed: u32 },
}

fn sound(play: &mut MessageWriter<PlaySound>, first: &FirstTimePredicted, entry: &str, at: Vec3, source: Entity) {
    if first.0 {
        play.write(PlaySound {
            pitch: None,
            entry: entry.to_string(),
            at: Some(at),
            volume: None,
            source: Some(source),
            channel: None,
        });
    }
}

fn world_sound(world: &mut World, entry: &str, at: Vec3, source: Entity) {
    world.write_message(PlaySound {
        pitch: None,
        entry: entry.to_string(),
        at: Some(at),
        volume: None,
        source: Some(source),
        channel: None,
    });
}

/// The gun's frame (before the shared one): claws, secondary, primary.
#[allow(clippy::type_complexity)]
pub fn frame(
    mut owners: Query<(Entity, &Intent, &Inventory, &Transform, &MovementState, &Health, Option<&mut Hl2Punch>), Without<NetDrawn>>,
    mut guns: Query<(&mut Hl2Physcannon, &mut WeaponState)>,
    objects: Objects,
    spatial: SpatialQuery,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    role: Option<Res<NetRole>>,
    mut events: MessageWriter<WeaponEvent>,
    mut play: MessageWriter<PlaySound>,
    mut damage: MessageWriter<Damage>,
    mut commands: Commands,
) {
    let now = clock.now;
    let due = now + 1e-5;
    let server = makes_projectiles(role.as_deref(), &first);
    let authoritative = role.as_deref().is_none_or(|r| *r != NetRole::Client);
    for (owner, intent, inv, at, state, health, mut punch) in &mut owners {
        let Some(active) = inv.active else { continue };
        let Ok((mut gun, mut st)) = guns.get_mut(active) else { continue };
        if health.current <= 0.0 {
            continue;
        }
        let gun = &mut *gun;
        let p = gun.params.clone();
        let (eye, fwd, ..) = launch::aim(at, state, intent, None, false);
        let Ok(dir) = Dir3::new(fwd) else { continue };
        let replay = !first.0;
        let mut event = |kind| {
            events.write(WeaponEvent {
                owner,
                weapon: active,
                kind,
                replay,
            })
        };
        let close = |gun: &mut Hl2Physcannon, play: &mut MessageWriter<PlaySound>| {
            gun.claws_close_at = None;
            if gun.claws {
                gun.claws = false;
                sound(play, &first, &p.close_sound, eye, owner);
            }
        };
        let open = |gun: &mut Hl2Physcannon, play: &mut MessageWriter<PlaySound>| {
            gun.claws_close_at = None;
            gun.lost_since = None;
            if !gun.claws {
                gun.claws = true;
                sound(play, &first, &p.open_sound, eye, owner);
            }
        };
        // (a) The claws.
        if !gun.holding {
            if gun.claws_close_at.is_some_and(|t| due >= t) {
                close(gun, &mut play);
            }
            if due >= gun.suppress_until {
                let found = trace(&objects, &spatial, eye, dir, p.trace_length * U, p.grab_hull * U, owner)
                    .map(|(c, ..)| objects.classify(c))
                    .is_some_and(|f| objects.can_hold(&p, f, state.ground));
                if found {
                    open(gun, &mut play);
                } else {
                    let since = *gun.lost_since.get_or_insert(now);
                    if gun.claws && now - since >= p.claws_close_delay as f64 - 1e-6 {
                        close(gun, &mut play);
                    }
                }
            }
        }
        // (b) Secondary.
        let fresh = intent.secondary && !gun.secondary_was;
        gun.secondary_was = intent.secondary;
        if intent.secondary && !gun.masked {
            if gun.holding {
                if fresh && st.next_secondary <= due {
                    st.next_primary = now + p.refire as f64;
                    st.next_secondary = now + p.refire as f64;
                    gun.holding = false;
                    sound(&mut play, &first, &p.drop_sound, eye, owner);
                    event(WeaponEventKind::Action(PICKUP));
                    if server {
                        commands.queue(move |w: &mut World| let_go(w, active, LetGo::Drop));
                    }
                } else if st.next_secondary <= due {
                    // The search runs on the held object and fails.
                    if !gun.denied {
                        gun.denied = true;
                        sound(&mut play, &first, &p.too_heavy_sound, eye, owner);
                    }
                    st.next_secondary = now + p.search_refire as f64;
                    close(gun, &mut play);
                }
            } else if st.next_secondary <= due && authoritative {
                let found = search(&spatial, &objects, &p, eye, dir, owner).filter(|(f, _)| objects.can_hold(&p, *f, state.ground));
                match found {
                    Some((f, true)) => {
                        gun.holding = true;
                        gun.masked = true;
                        st.next_secondary = now + p.refire as f64;
                        open(gun, &mut play);
                        sound(&mut play, &first, &p.pickup_sound, eye, owner);
                        event(WeaponEventKind::Action(PICKUP));
                        let view = launch_view(intent);
                        commands.queue(move |w: &mut World| grab(w, active, owner, f, view));
                    }
                    Some((Found::Body(e), false)) => {
                        st.next_secondary = now + p.search_refire as f64;
                        close(gun, &mut play);
                        if let Some(c) = objects.centre(e) {
                            let m = objects.mass(e);
                            let dv = (eye - c).normalize_or_zero() * pull_impulse(&p, m) / m.max(1e-3) * U;
                            commands.queue(move |w: &mut World| push(w, e, dv, Vec3::ZERO));
                        }
                    }
                    _ => {
                        if !gun.denied {
                            gun.denied = true;
                            sound(&mut play, &first, &p.too_heavy_sound, eye, owner);
                        }
                        st.next_secondary = now + p.search_refire as f64;
                        close(gun, &mut play);
                    }
                }
            }
        } else if !intent.secondary {
            gun.denied = false;
            gun.masked = false;
        }
        // (d) Primary.
        if !intent.fire {
            continue;
        }
        let mut fire = |gun: &mut Hl2Physcannon, recoil: bool, punch: &mut Option<Mut<Hl2Punch>>, commands: &mut Commands| {
            let mut rng = shared_random(intent.command, "hl2dm:physcannon", 0);
            let mut kick = Vec3::new(p.punch_pitch, rng.range(-(p.punch_yaw as f32), p.punch_yaw as f32 + 0.999).floor(), 0.0);
            if recoil {
                kick += Vec3::new(rng.range(p.recoil_pitch[0], p.recoil_pitch[1]), rng.range(p.recoil_yaw[0], p.recoil_yaw[1]), 0.0);
            }
            match punch.as_deref_mut() {
                Some(v) => v.velocity += kick * IMPULSE_SCALE,
                None => {
                    commands.entity(owner).insert((
                        Hl2Punch {
                            velocity: kick * IMPULSE_SCALE,
                        },
                        chimera_sim::weapon::ViewPunch::default(),
                    ));
                }
            }
            gun.suppress_until = now + p.target_suppress as f64;
            sound(&mut play, &first, &p.launch_sound, eye, owner);
        };
        if gun.holding {
            if st.next_primary > due {
                continue;
            }
            let object = gun.object;
            let far = server
                && object.is_some_and(|o| {
                    let centre = objects
                        .centre(o)
                        .or(objects.flyers.get(o).ok().map(|f| f.2.translation));
                    let mine = at.translation + (state.hull_min + state.hull_max) / 2.0;
                    centre.is_some_and(|c| c.distance(mine) > p.trace_length * U)
                });
            let again = server && gun.punted.is_some_and(|(o, until)| Some(o) == object && due < until);
            if far {
                sound(&mut play, &first, &p.dry_sound, eye, owner);
                event(WeaponEventKind::DryFire);
                continue;
            }
            if again {
                continue;
            }
            gun.holding = false;
            st.next_primary = now + p.refire as f64;
            st.next_secondary = now + p.refire as f64;
            gun.claws_close_at = Some(now + p.launch_claws as f64);
            if let Some(o) = object {
                gun.punted = Some((o, now + p.refire as f64));
            }
            fire(gun, false, &mut punch, &mut commands);
            event(WeaponEventKind::Action(PUNT));
            if server {
                let seed = intent.command;
                commands.queue(move |w: &mut World| let_go(w, active, LetGo::Launch { dir: fwd, seed }));
            }
            continue;
        }
        if st.next_primary > due {
            continue;
        }
        st.next_primary = now + p.refire as f64;
        let target = punt_target(&spatial, &objects, &p, eye, dir, owner);
        let dry = match target {
            None | Some((Found::World | Found::Player(_), _)) => true,
            Some((Found::Body(e), _)) => objects.ragdoll(e),
            _ => false,
        };
        if dry {
            sound(&mut play, &first, &p.dry_sound, eye, owner);
            event(WeaponEventKind::DryFire);
            continue;
        }
        let Some((found, point)) = target else { continue };
        let thing = match found {
            Found::Body(e) | Found::Ball(e) | Found::Damageable(e) | Found::Player(e) => e,
            Found::World => continue,
        };
        // The same object within 0.5 s: nothing at all.
        if server && gun.punted.is_some_and(|(o, until)| o == thing && due < until) {
            continue;
        }
        if server {
            gun.punted = Some((thing, now + p.refire as f64));
        }
        gun.claws_close_at = Some(now + p.punt_claws as f64);
        match found {
            Found::Damageable(e) => {
                if server {
                    damage.write(Damage {
                        target: e,
                        attacker: Some(owner),
                        amount: launch::HP,
                        point,
                        dir: fwd,
                        hitgroup: Hitgroup::Generic,
                        kind: DamageKind::Crush,
                        weapon: Some(super::PHYSCANNON),
                        force: Vec3::ZERO,
                    });
                }
                fire(gun, false, &mut punch, &mut commands);
            }
            _ => {
                if server {
                    // A 0-damage hit makes the punter its last attacker.
                    damage.write(Damage {
                        target: thing,
                        attacker: Some(owner),
                        amount: 0.0,
                        point,
                        dir: fwd,
                        hitgroup: Hitgroup::Generic,
                        kind: DamageKind::Generic,
                        weapon: Some(super::PHYSCANNON),
                        force: Vec3::ZERO,
                    });
                    let d = punt_dir(&p, fwd);
                    let p2 = p.clone();
                    commands.queue(move |w: &mut World| punt(w, &p2, owner, thing, d, point));
                }
                st.next_secondary = now + p.refire as f64;
                fire(gun, true, &mut punch, &mut commands);
            }
        }
        event(WeaponEventKind::Action(PUNT));
    }
}

/// The full view rotation of `intent`.
fn launch_view(intent: &Intent) -> Quat {
    chimera_sim::weapon::aim_of(intent.yaw, intent.pitch)
}

/// Add `dv` (m/s) and `dw` (rad/s) to a body, waking it.
fn push(world: &mut World, e: Entity, dv: Vec3, dw: Vec3) {
    crate::map::prop_physics::wake(world, e);
    let Ok(mut b) = world.get_entity_mut(e) else { return };
    if !b.get::<RigidBody>().is_some_and(|r| r.is_dynamic()) {
        return;
    }
    if let Some(mut v) = b.get_mut::<LinearVelocity>() {
        v.0 += dv;
    }
    if let Some(mut w) = b.get_mut::<AngularVelocity>() {
        w.0 += dw;
    }
    b.remove::<Sleeping>();
}

/// The influence of `by` on `e` from now.
fn influence(world: &mut World, e: Entity, by: Entity) {
    let at = world.resource::<SimClock>().now;
    if let Ok(mut b) = world.get_entity_mut(e) {
        b.insert(PhysicsInfluence { by, at });
    }
}

/// A frag touched by the gun (11): the gun's user throws it, 1.5 s left,
/// a blip now and fast blips after.
fn take_frag(world: &mut World, e: Entity, by: Entity, fuse: f32) {
    let now = world.resource::<SimClock>().now;
    let Some(mut g) = world.get_mut::<Hl2Grenade>(e) else { return };
    g.thrower = Some(by);
    g.explode_at = now + fuse as f64;
    g.warned = true;
    g.next_blip = now + g.params.blip_fast as f64;
    let blip = g.params.blip.clone();
    if let Some(mut s) = world.get_mut::<ShownBody>(e) {
        s.owner = Some(by);
    }
    let at = world.get::<Transform>(e).map_or(Vec3::ZERO, |t| t.translation);
    world_sound(world, &blip, at, e);
}

/// A map prop's spawnflags (0 for anything else).
fn prop_flags(world: &World, e: Entity) -> u32 {
    let Some(index) = world.get::<PropEntity>(e) else { return 0 };
    world
        .get_resource::<MapEntities>()
        .and_then(|m| m.entities.get(index.0))
        .and_then(|e| e.get("spawnflags"))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

/// A physics punt (2) of `e` by `by` along `d` from the hit `point`.
fn punt(world: &mut World, p: &PhysParams, by: Entity, e: Entity, d: Vec3, point: Vec3) {
    let flags = prop_flags(world, e);
    if world.get::<FrozenBody>(e).is_some() && flags & SF_ENABLE_ON_GUN != 0 {
        crate::map::prop_physics::enable_motion(world, e);
    }
    influence(world, e, by);
    take_frag(world, e, by, p.frag_fuse);
    let Ok(b) = world.get_entity(e) else { return };
    let mass = b
        .get::<ComputedMass>()
        .map(|m| m.value())
        .filter(|m| m.is_finite() && *m > 0.0)
        .or(b.get::<PhysicsProp>().map(|p| p.mass))
        .unwrap_or(1.0);
    let centre_j = p.punt_base * d;
    let point_j = p.punt_mass * mass.min(p.max_mass) * d;
    let dv = (centre_j + point_j) / mass * U;
    // The point impulse's turn about the mass centre (kg m/s at m).
    let pos = b.get::<Position>().map_or(Vec3::ZERO, |x| x.0);
    let rot = b.get::<Rotation>().map_or(Quat::IDENTITY, |r| r.0);
    let com = pos + rot * b.get::<ComputedCenterOfMass>().map_or(Vec3::ZERO, |c| c.0);
    let dw = b
        .get::<ComputedAngularInertia>()
        .map(|i| i.rotated(rot).inverse().mul_vec3((point - com).cross(point_j * U)))
        .filter(|w| w.is_finite())
        .unwrap_or(Vec3::ZERO);
    push(world, e, dv, dw);
}

/// The gun `gun` of `by` grabs what was found (4), the view `view`.
fn grab(world: &mut World, gun: Entity, by: Entity, found: Found, view: Quat) {
    let Some(p) = world.get::<Hl2Physcannon>(gun).map(|g| g.params.clone()) else { return };
    let now = world.resource::<SimClock>().now;
    match found {
        Found::Ball(b) => {
            if let Some(mut f) = world.get_mut::<Flyer>(b) {
                f.owner = Some(by);
                f.solid_after = f64::MAX;
                f.expires = None;
                f.keep_speed = None;
                f.velocity = Vec3::ZERO;
            }
            if let Some(mut s) = world.get_mut::<ShownBody>(b) {
                s.owner = Some(by);
            }
            world.entity_mut(b).insert(HeldBall {
                holder: by,
                explode_at: now + p.ball_life as f64,
            });
            let at = world.get::<Transform>(b).map_or(Vec3::ZERO, |t| t.translation);
            world_sound(world, &p.ball_hold_sound, at, b);
            if let Some(mut g) = world.get_mut::<Hl2Physcannon>(gun) {
                g.object = Some(b);
                g.relative = Quat::IDENTITY;
                g.anchor = Vec3::ZERO;
                g.bounds = (Vec3::splat(-10.0 * U), Vec3::splat(10.0 * U));
                g.error = 0.0;
                g.error_clock = -(p.error_grace as f64);
                g.last_target = None;
            }
        }
        Found::Body(e) => {
            let flags = prop_flags(world, e);
            if world.get::<FrozenBody>(e).is_some() && flags & SF_ENABLE_ON_GUN != 0 {
                crate::map::prop_physics::enable_motion(world, e);
            }
            crate::map::prop_physics::wake(world, e);
            influence(world, e, by);
            take_frag(world, e, by, p.frag_fuse);
            let rot = world.get::<Transform>(e).map_or(Quat::IDENTITY, |t| t.rotation);
            let bounds = world
                .get::<PhysicsProp>(e)
                .map(|x| x.bounds)
                .or(world.get::<FrozenBody>(e).map(|f| f.bounds))
                .unwrap_or((Vec3::splat(-4.0 * U), Vec3::splat(4.0 * U)));
            let dt = world.resource::<SimClock>().dt();
            held::hold(
                world,
                e,
                Held {
                    holder: by,
                    target: None,
                    view_offset: None,
                    arrive: dt,
                    max_speed: p.hold_speed * U,
                    max_angular: p.hold_angular.to_radians(),
                },
                p.held_mass,
                p.held_rot_damping,
            );
            if let Some(mut g) = world.get_mut::<Hl2Physcannon>(gun) {
                g.object = Some(e);
                g.relative = view.inverse() * rot;
                g.anchor = (bounds.0 + bounds.1) / 2.0;
                g.bounds = bounds;
                g.error = 0.0;
                g.error_clock = -(p.error_grace as f64);
                g.last_target = None;
            }
        }
        _ => {}
    }
}

/// The hold of `gun` ends (6, 7).
pub fn let_go(world: &mut World, gun: Entity, how: LetGo) {
    let Some(mut g) = world.get_mut::<Hl2Physcannon>(gun) else { return };
    g.holding = false;
    let Some(object) = g.object.take() else { return };
    let p = g.params.clone();
    let by = world.get::<Weapon>(gun).and_then(|w| w.owner);
    let now = world.resource::<SimClock>().now;
    if let Some(ball) = world.get::<HeldBall>(object).copied() {
        world.entity_mut(object).remove::<HeldBall>();
        let speed = p.ball_speed * U;
        let v = match how {
            LetGo::Launch { dir, .. } => dir * launch_speed(&p, p.ball_mass) * U,
            LetGo::Drop => {
                let (yaw, pitch) = world
                    .get::<Intent>(ball.holder)
                    .map_or((0.0, 0.0), |i| (i.yaw, i.pitch));
                let command = world.get::<Intent>(ball.holder).map_or(0, |i| i.command);
                let mut rng = shared_random(command, "hl2dm:physcannon_ball", 0);
                let s = p.ball_drop_spread as f32;
                let dp = rng.range(-s, s + 0.999).floor().to_radians();
                let dy = rng.range(-s, s + 0.999).floor().to_radians();
                chimera_sim::weapon::aim_of(yaw + dy, pitch + dp) * Vec3::NEG_Z * speed
            }
        };
        if let Some(mut f) = world.get_mut::<Flyer>(object) {
            f.velocity = v;
            f.keep_speed = Some(speed);
            f.solid_after = now;
            f.expires = Some(now + p.ball_flight_life as f64);
        }
        let at = world.get::<Transform>(object).map_or(Vec3::ZERO, |t| t.translation);
        world_sound(world, &p.ball_launch_sound, at, object);
        return;
    }
    held::release(world, object);
    if let Some(by) = by {
        influence(world, object, by);
    }
    let Ok(mut b) = world.get_entity_mut(object) else { return };
    let mass = b.get::<Mass>().map_or(1.0, |m| m.0);
    let (v, w) = match how {
        LetGo::Drop => {
            let v = b.get::<LinearVelocity>().map_or(Vec3::ZERO, |v| v.0);
            let w = b.get::<AngularVelocity>().map_or(Vec3::ZERO, |w| w.0);
            (v.clamp_length_max(p.drop_speed * U), w.clamp_length_max(p.drop_spin.to_radians()))
        }
        LetGo::Launch { dir, seed } => {
            let mut rng = shared_random(seed, "hl2dm:physcannon_spin", 0);
            let s = p.launch_spin;
            let spin = Vec3::new(rng.range(-s, s), rng.range(-s, s), rng.range(-s, s));
            (dir * launch_speed(&p, mass) * U, spin * std::f32::consts::PI / 180.0)
        }
    };
    if let Some(mut x) = b.get_mut::<LinearVelocity>() {
        x.0 = v;
    }
    if let Some(mut x) = b.get_mut::<AngularVelocity>() {
        x.0 = w;
    }
}

/// A forced drop (6): the holder died, `phys_swap`, the hold failed.
pub fn force_drop(world: &mut World, gun: Entity) {
    let held = world.get::<Hl2Physcannon>(gun).is_some_and(|g| g.object.is_some());
    if held {
        let object = world.get::<Hl2Physcannon>(gun).and_then(|g| g.object);
        let entry = world.get::<Hl2Physcannon>(gun).map(|g| g.params.drop_sound.clone());
        if let (Some(o), Some(entry)) = (object, entry) {
            let at = world.get::<Transform>(o).map_or(Vec3::ZERO, |t| t.translation);
            world_sound(world, &entry, at, o);
        }
        let_go(world, gun, LetGo::Drop);
        if let Some(mut g) = world.get_mut::<Hl2Physcannon>(gun) {
            g.claws = false;
            g.claws_close_at = None;
        }
    }
}

/// Each tick, the server's: every hold's target (5), its error, and the
/// drops it forces; a held ball goes off after 8 s.
#[allow(clippy::type_complexity)]
pub fn hold(
    mut guns: Query<(Entity, &mut Hl2Physcannon, &Weapon)>,
    owners: Query<(&Transform, &MovementState, &Intent, &Health)>,
    mut bodies: Query<(&mut Held, &Transform, Option<&RigidBody>, Has<ColliderDisabled>)>,
    mut balls: Query<(&HeldBall, &mut Flyer, &Transform), Without<Held>>,
    statics: Query<(Option<&RigidBody>, Has<PropEntity>, Has<Intent>)>,
    of: Query<&ColliderOf>,
    ball_params: Query<&super::alt_fire::Hl2Ball>,
    spatial: SpatialQuery,
    clock: Res<SimClock>,
    mut play: MessageWriter<PlaySound>,
    mut commands: Commands,
) {
    let (now, dt) = (clock.now, clock.dt());
    for (gun, mut g, weapon) in &mut guns {
        let Some(object) = g.object else { continue };
        let Some((at, state, intent, health)) = weapon.owner.and_then(|o| owners.get(o).ok()) else {
            commands.queue(move |w: &mut World| force_drop(w, gun));
            continue;
        };
        if health.current <= 0.0 {
            commands.queue(move |w: &mut World| force_drop(w, gun));
            continue;
        }
        let p = g.params.clone();
        let eye = at.translation + state.eye_offset;
        let clamp = p.pitch_clamp.to_radians();
        let view = chimera_sim::weapon::aim_of(intent.yaw, intent.pitch);
        let flat_view = chimera_sim::weapon::aim_of(intent.yaw, intent.pitch.clamp(-clamp, clamp));
        let f = flat_view * Vec3::NEG_Z;
        let axis = launch::to_source(at.translation + (state.hull_min + state.hull_max) / 2.0);
        let axis = Vec2::new(axis.x, axis.y);
        // A ball: carried by its flight, no body.
        if let Ok((b, mut fl, t)) = balls.get_mut(object) {
            if now + 1e-6 >= b.explode_at {
                let entry = ball_params.iter().next().map(|x| x.params.explode_sound.clone());
                if let Some(entry) = entry {
                    play.write(PlaySound::at(entry, t.translation));
                }
                g.object = None;
                g.holding = false;
                commands.entity(object).despawn();
                continue;
            }
            let e = 10.0;
            let point = hold_point(&p, launch::to_source(eye), launch::to_source(f).normalize_or_zero(), e, None, axis);
            let to = launch::to_engine(point);
            fl.velocity = ((to - t.translation) / dt.max(1e-4)).clamp_length_max(p.hold_speed * U);
            continue;
        }
        let Ok((mut held, t, rb, gone)) = bodies.get_mut(object) else {
            commands.queue(move |w: &mut World| force_drop(w, gun));
            continue;
        };
        // The error since the last target (5).
        if let Some(last) = g.last_target {
            let e_now = last.distance(t.translation) / U;
            if g.error_clock > 0.0 {
                g.error = smooth_error(g.error, g.error_clock, e_now);
                g.error_clock = 0.0;
            }
        }
        let pinned = rb.is_none_or(|r| !r.is_dynamic());
        if g.error > p.error_limit || state.ground == Some(object) || gone || pinned {
            commands.queue(move |w: &mut World| force_drop(w, gun));
            continue;
        }
        let e = extent(g.bounds, t.rotation, -f) / U;
        let r = p.player_radius + e;
        let d = p.hold_base + 2.0 * r;
        let body = |c: Entity| of.get(c).map_or(c, |o| o.body);
        let world_only = |c: Entity| {
            let b = body(c);
            statics.get(b).is_ok_and(|(rb, prop, ch)| !prop && !ch && rb.is_none_or(|r| r.is_static()))
        };
        let filter = SpatialQueryFilter::from_mask(SOLID_LAYERS);
        let wall = Dir3::new(f)
            .ok()
            .and_then(|dir| spatial.cast_ray_predicate(eye, dir, d * U, true, &filter, &world_only))
            .map(|h| h.distance / U);
        let point = hold_point(&p, launch::to_source(eye), launch::to_source(f).normalize_or_zero(), e, wall, axis);
        let rot = (view * g.relative).normalize();
        let origin = launch::to_engine(point) - rot * g.anchor;
        held.target = Some((origin, rot));
        held.arrive = dt;
        held.view_offset = Some((view.inverse() * (origin - eye), (view.inverse() * rot).normalize()));
        g.last_target = Some(origin);
        g.error_clock += dt as f64;
    }
}

/// The holder of a gun holding something dies: a forced drop (before
/// the gun is taken away).
pub fn drop_on_death(
    mut died: MessageReader<Died>,
    owners: Query<&Inventory>,
    guns: Query<(), With<Hl2Physcannon>>,
    mut commands: Commands,
) {
    for d in died.read() {
        let Ok(inv) = owners.get(d.entity) else { continue };
        for w in inv.weapons.iter().copied().filter(|w| guns.contains(*w)) {
            commands.queue(move |world: &mut World| force_drop(world, w));
        }
    }
}

/// A gun holding something can't be put away (1).
pub fn lock_holding(guns: Query<(Entity, &Hl2Physcannon, Has<HolsterLocked>)>, mut commands: Commands) {
    for (e, g, locked) in &guns {
        if g.holding && !locked {
            commands.entity(e).insert(HolsterLocked);
        } else if !g.holding && locked {
            commands.entity(e).remove::<HolsterLocked>();
        }
    }
}

/// `phys_swap` (weapons.md 1): with the gun in hand, a forced drop and
/// the last weapon; otherwise the gun.
pub fn phys_swap(world: &mut World, player: Entity) {
    let Some(inv) = world.get::<Inventory>(player).cloned() else { return };
    let Some(active) = inv.active else { return };
    if world.get::<Hl2Physcannon>(active).is_some() {
        force_drop(world, active);
        world.entity_mut(active).remove::<HolsterLocked>();
        if let Some(mut i) = world.get_mut::<Inventory>(player) {
            i.wanted = inv.last;
        }
        return;
    }
    let gun = inv.weapons.iter().copied().find(|w| world.get::<Hl2Physcannon>(*w).is_some());
    if let Some(gun) = gun
        && let Some(mut i) = world.get_mut::<Inventory>(player)
    {
        i.wanted = Some(gun);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn params() -> PhysParams {
        launch::hook_params(super::super::PHYSCANNON, super::super::PHYSCANNON_HOOK)
    }

    /// Test cases 1, 2, 12: the punt's speeds and the pull's.
    #[test]
    fn punt_and_pull_speeds() {
        let p = params();
        assert!((punt_speed(&p, 30.0) - 1100.0).abs() < 1e-3);
        assert!((punt_speed(&p, 10.0) - 2100.0).abs() < 1e-3);
        assert!((punt_speed(&p, 250.0) - 660.0).abs() < 1e-3);
        assert!((punt_speed(&p, 500.0) - 330.0).abs() < 1e-3);
        assert!((punt_speed(&p, 1000.0) - 165.0).abs() < 1e-3);
        for (m, dv) in [(10.0, 84.0), (30.0, 81.333), (50.0, 80.0), (250.0, 16.0)] {
            assert!((pull_impulse(&p, m) / m - dv).abs() < 1e-2, "{m}");
        }
    }

    /// Test case 3: a downward punt goes up, flattened.
    #[test]
    fn a_downward_punt_goes_up() {
        let p = params();
        let f = Vec3::new(0.7071, -0.7071, 0.0);
        let d = punt_dir(&p, f) * punt_speed(&p, 30.0);
        assert!((d - Vec3::new(777.8, 505.6, 0.0)).length() < 0.2, "{d}");
    }

    /// Test case 23 and section 7's list: the launch speeds.
    #[test]
    fn launch_speeds() {
        let p = params();
        for (m, v) in [(30.0, 1500.0), (150.0, 1477.6), (200.0, 1416.8), (250.0, 1327.2), (600.0, 700.0), (750.0, 959.2), (1000.0, 3055.2)] {
            assert!((launch_speed(&p, m) - v).abs() < 0.1, "{m}: {}", launch_speed(&p, m));
        }
    }

    /// Test cases 18-20: the hold point.
    #[test]
    fn the_hold_point() {
        let p = params();
        let eye = Vec3::new(0.0, 0.0, 64.0);
        let at = hold_point(&p, eye, Vec3::X, 10.0, None, Vec2::ZERO);
        assert!((at - Vec3::new(56.627, 0.0, 64.0)).length() < 1e-3, "{at}");
        let at = hold_point(&p, eye, Vec3::X, 10.0, Some(30.0), Vec2::ZERO);
        assert!((at - Vec3::new(32.627, 0.0, 64.0)).length() < 1e-3, "{at}");
        let down = Vec3::new(75f32.to_radians().cos(), 0.0, -75f32.to_radians().sin());
        let at = hold_point(&p, eye, down, 10.0, None, Vec2::ZERO);
        assert!((at - Vec3::new(32.627, 0.0, 9.30)).length() < 0.01, "{at}");
    }

    /// Test cases 27, 28: the smoothed error against a wall gap of 48
    /// drops 46 ticks after the grace; 20 never.
    #[test]
    fn the_error_dead_zone() {
        let p = params();
        let ticks = |gap: f32| {
            let mut e = 0.0;
            for n in 1..2000 {
                e = smooth_error(e, 0.015, gap);
                if e > p.error_limit {
                    return Some(n);
                }
            }
            None
        };
        assert_eq!(ticks(48.0), Some(46));
        assert_eq!(ticks(20.0), None);
        assert_eq!(ticks(14.0), Some(129));
    }
}
