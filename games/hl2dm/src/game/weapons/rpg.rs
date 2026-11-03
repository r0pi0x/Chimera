//! The RPG (specs/hl2dm/projectiles.md 6), `hl2dm:rpg` (`Hl2Rpg`, its
//! rockets in `launch::Hl2Rounds`: RPG_Round, 3 carried):
//!
//! - **Firing**: attack with the next primary passed, no missile of its
//!   own alive and not reloading: 1 rocket, next primary + 0.5
//!   (`Weapon_RPG.Single`); with none, the empty click. The missile
//!   starts at eye + 12 forward + 6 right − 3 up with 300 forward + 128 up
//!   (units/s) under full gravity, a ±4 box; non-solid for 0.3 s when a
//!   128-unit line along the view hits nothing (the grace). While it
//!   lives the RPG can't be put away (`weapon::HolsterLocked`).
//! - **Ignition** (`missiles`, 20 ticks on): 1500 units/s along its launch
//!   angles, no gravity, a point, solid; then each tick a guidance step
//!   toward the dot (6.4): with a target entity the dot pushed 10 units
//!   into its surface, else the point 256 ahead on the line from its
//!   owner's centre (not the eye) to that, while it is nearer the owner
//!   than the dot or within 512 of it (riding the beam), else that point;
//!   the new direction normalize(0.125 t̂ + 0.875 v̂), the speed kept. No
//!   dot (its owner dead, the RPG put away or dropped): straight on.
//! - **The dot** (`Hl2LaserDot`, on the weapon, the server's): each tick
//!   its owner holds the RPG with rockets or a missile out, where a line
//!   from the eye along the view (no punch) stops, through windows; the
//!   surface normal, and what it stopped on when that is a player or
//!   something else that takes damage. It can't be turned off.
//! - **Explosion**: on touching anything solid (never its owner), 150 in
//!   300 at its origin (`launch::hl2_blast`), credited to its shooter
//!   even after his death. Nothing shoots it down (it takes no damage).
//! - **Reload**: when its missile is gone, with rockets left, the 1.6667 s
//!   reload animation; the RPG fires again only after a weapon frame with
//!   no attack held strictly after its end (holding attack through it
//!   blocks the next shot until let go): the fastest refire is 113 ticks
//!   after the explosion.
//!
//! Known shortcuts: the dot isn't drawn (H9: M10) nor replicated; the
//! dot appears with the draw (not after the 1.1 s draw animation) and
//! isn't lowered with no ammo; the missile's smoke trail, the owner's
//! fade at ignition and `Missile.Ignite`'s loop are M10's; the missile's
//! sky test (removed silently) is Chimera's flyers' (none yet).

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    HolsterLocked, Inventory, NetPart, Weapon, WeaponEvent, WeaponEventKind, WeaponState,
    projectile::{Flyer, FlyerGone, FlyerSet, LaunchRequest, Launched, Touch},
};

use super::{
    alt_fire::makes_projectiles,
    launch::{self, Hl2Rounds, MISSILE_MODEL, U},
};
use crate::{
    core::{Damageable, FirstTimePredicted, Health, Intent, MovementState, NetRole, SOLID_LAYERS, SimClock},
    map::{PlaySound, SeeThrough},
};

/// The `LaunchRequest` tag of a missile (`Launched` names it).
pub const MISSILE_TAG: u64 = 0x5250_4700;

/// `hl2dm:rpg`'s parameters (Source units, seconds, points).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RpgParams {
    #[serde(flatten)]
    pub rounds: launch::RoundsParams,
    pub refire: f32,
    pub reload: f32,
    pub muzzle: [f32; 3],
    pub launch_speed: f32,
    pub launch_up: f32,
    pub half: f32,
    pub grace_check: f32,
    pub ignite: f32,
    pub speed: f32,
    pub homing: f32,
    pub ride_lead: f32,
    pub ride_near: f32,
    pub dot_sink: f32,
    pub dot_range: f32,
    pub damage: f32,
    pub radius: f32,
    pub sound: String,
    pub empty: String,
    pub explode: String,
}

/// The RPG's state (predicted): its missile alive, the reload animation's
/// end while it plays.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Hl2Rpg {
    pub params: RpgParams,
    pub missile: bool,
    pub reload_until: Option<f64>,
    /// The server's: its missile.
    pub missile_entity: Option<Entity>,
}

impl NetPart for Hl2Rpg {
    type Net = (bool, Option<f64>);
    fn net(&self) -> Self::Net {
        (self.missile, self.reload_until)
    }
    fn set_net(&mut self, (missile, reload): Self::Net) {
        self.missile = missile;
        self.reload_until = reload;
    }
}

/// The laser dot of an RPG (the server's): where it is, the surface's
/// normal, what it is on (a player or something else that takes damage).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Hl2LaserDot {
    pub at: Vec3,
    pub normal: Vec3,
    pub target: Option<Entity>,
}

/// A missile: its RPG, its launch direction and when it ignites.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Hl2Missile {
    pub rpg: Entity,
    pub dir: Vec3,
    pub ignite_tick: u64,
    pub ignited: bool,
}

/// Before the weapon frame: fire, and the reload's end (after the fire
/// test: a press on the tick the reload ends waits for the next).
#[allow(clippy::type_complexity)]
pub fn before_frame(
    owners: Query<
        (Entity, &Intent, &Inventory, &Transform, &MovementState, &Health),
        Without<crate::map::interp::NetDrawn>,
    >,
    mut rpgs: Query<(&mut Hl2Rpg, &mut Hl2Rounds, &mut WeaponState)>,
    spatial: SpatialQuery,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    role: Option<Res<NetRole>>,
    mut events: MessageWriter<WeaponEvent>,
    mut play: MessageWriter<PlaySound>,
    mut launch: MessageWriter<LaunchRequest>,
) {
    let now = clock.now;
    let due = now + 1e-5;
    for (owner, intent, inv, at, state, health) in &owners {
        if health.current <= 0.0 {
            continue;
        }
        let Some(active) = inv.active else { continue };
        let Ok((mut rpg, mut rounds, mut st)) = rpgs.get_mut(active) else {
            continue;
        };
        let p = rpg.params.clone();
        let (eye, fwd, right, up) = launch::aim(at, state, intent, None, false);
        if intent.fire && due >= inv.next_attack && st.next_primary <= due && !rpg.missile && rpg.reload_until.is_none()
        {
            if rounds.count == 0 {
                if first.0 {
                    play.write(PlaySound::at(p.empty.clone(), eye));
                }
                st.next_primary = now + 0.5;
                events.write(WeaponEvent {
                    owner,
                    weapon: active,
                    kind: WeaponEventKind::DryFire,
                    replay: !first.0,
                });
            } else {
                rounds.count -= 1;
                rpg.missile = true;
                st.next_primary = now + p.refire as f64;
                if first.0 {
                    play.write(PlaySound {
                        pitch: None,
                        entry: p.sound.clone(),
                        at: Some(eye),
                        volume: None,
                        source: Some(owner),
                        channel: Some(chimera_sim::weapon::CHAN_WEAPON),
                    });
                }
                events.write(WeaponEvent {
                    owner,
                    weapon: active,
                    kind: WeaponEventKind::Action("fire"),
                    replay: !first.0,
                });
                if makes_projectiles(role.as_deref(), &first) {
                    let start = eye + (fwd * p.muzzle[0] + right * p.muzzle[1] + up * p.muzzle[2]) * U;
                    let mut f = Flyer::new(
                        super::RPG,
                        Some(owner),
                        fwd * p.launch_speed * U + Vec3::Y * p.launch_up * U,
                        &clock,
                    );
                    f.gravity = 600.0 * U;
                    f.half = Vec3::splat(p.half * U);
                    f.touch = Touch::Explode;
                    f.blast = Some(launch::blast(p.damage, p.radius, None, Some(&p.explode)));
                    // The grace: nothing within 128 along the view.
                    let filter = SpatialQueryFilter::from_excluded_entities([owner]).with_mask(SOLID_LAYERS);
                    let clear = Dir3::new(fwd)
                        .is_ok_and(|d| spatial.cast_ray(eye, d, p.grace_check * U, true, &filter).is_none());
                    f.solid_after = if clear { now + p.ignite as f64 } else { now };
                    launch.write(LaunchRequest {
                        flyer: f,
                        start,
                        shown: Some(MISSILE_MODEL.into()),
                        tag: Some(MISSILE_TAG),
                    });
                }
            }
        }
        if let Some(end) = rpg.reload_until
            && now > end + 1e-6
            && !intent.fire
        {
            rpg.reload_until = None;
        }
    }
}

/// An RPG with its missile out can't be put away.
pub fn lock_rpg(
    mut rpgs: Query<(Entity, &Hl2Rpg, Has<HolsterLocked>), Without<super::alt_fire::Hl2Ball>>,
    mut commands: Commands,
) {
    for (e, rpg, locked) in &mut rpgs {
        if rpg.missile && !locked {
            commands.entity(e).insert(HolsterLocked);
        } else if !rpg.missile && locked {
            commands.entity(e).remove::<HolsterLocked>();
        }
    }
}

/// The dots (the server's, `FlyerSet::Think`): placed for every owner
/// holding an RPG with rockets or a missile out, gone otherwise.
#[allow(clippy::type_complexity)]
pub fn place_dots(
    owners: Query<(Entity, &Intent, &Inventory, &Transform, &MovementState, &Health)>,
    rpgs: Query<(Entity, &Hl2Rpg, &Hl2Rounds, &Weapon, Has<Hl2LaserDot>)>,
    spatial: SpatialQuery,
    colliders: Query<&ColliderOf>,
    see_through: Query<(), With<SeeThrough>>,
    takes: Query<(), Or<(With<Health>, With<Damageable>)>>,
    mut commands: Commands,
) {
    for (e, rpg, rounds, w, has) in &rpgs {
        let holder = w
            .owner
            .and_then(|o| owners.get(o).ok())
            .filter(|(_, _, inv, .., h)| inv.active == Some(e) && h.current > 0.0);
        let Some((owner, intent, _, at, state, _)) = holder.filter(|_| rounds.count > 0 || rpg.missile) else {
            if has {
                commands.entity(e).remove::<Hl2LaserDot>();
            }
            continue;
        };
        let (eye, fwd, ..) = launch::aim(at, state, intent, None, false);
        let Ok(dir) = Dir3::new(fwd) else { continue };
        let filter = SpatialQueryFilter::from_excluded_entities([owner]).with_mask(SOLID_LAYERS);
        let range = rpg.params.dot_range * U;
        let hit = spatial.cast_ray_predicate(eye, dir, range, true, &filter, &|c| {
            let b = colliders.get(c).map_or(c, |o| o.body);
            !see_through.contains(c) && !see_through.contains(b)
        });
        let dot = match hit {
            Some(h) => {
                let body = colliders.get(h.entity).map_or(h.entity, |o| o.body);
                Hl2LaserDot {
                    at: eye + fwd * h.distance,
                    normal: h.normal,
                    target: takes.contains(body).then_some(body),
                }
            }
            None => Hl2LaserDot {
                at: eye + fwd * range,
                normal: -fwd,
                target: None,
            },
        };
        commands.entity(e).insert(dot);
    }
}

/// New missiles get their state (`FlyerSet::Think`, first).
pub fn new_missiles(
    mut launched: MessageReader<Launched>,
    owners: Query<&Inventory>,
    mut rpgs: Query<&mut Hl2Rpg>,
    mut commands: Commands,
) {
    for l in launched.read() {
        if l.tag != Some(MISSILE_TAG) {
            continue;
        }
        let Some(rpg) = l.owner.and_then(|o| owners.get(o).ok()).and_then(|inv| inv.active) else { continue };
        let Ok(mut r) = rpgs.get_mut(rpg) else { continue };
        r.missile_entity = Some(l.flyer);
        let ticks = (r.params.ignite / 0.015).round() as u64;
        let flat = Vec3::new(l.velocity.x, 0.0, l.velocity.z).normalize_or(Vec3::NEG_Z);
        // The launch angles' direction: the velocity less its lift.
        let dir = (l.velocity - Vec3::Y * r.params.launch_up * U).normalize_or(flat);
        commands.entity(l.flyer).insert(Hl2Missile {
            rpg,
            dir,
            ignite_tick: l.tick + ticks,
            ignited: false,
        });
    }
}

/// The aim point of a guidance step (6.4 step 2), engine space: the dot
/// `dot` with normal `n` and target or not, the owner's centre `c`, the
/// missile at `m`, `p`'s distances.
pub fn aim_point(p: &RpgParams, dot: Vec3, n: Vec3, target: bool, c: Vec3, m: Vec3) -> Vec3 {
    let l = dot - n * p.dot_sink * U;
    if target {
        return l;
    }
    let ell = (l - c).length();
    let e = (l - c) / ell.max(1e-6);
    if (m - c).length() < ell || (m - l).length() < p.ride_near * U {
        let along = (m - c).dot(e).clamp(0.0, ell);
        c + e * along + e * p.ride_lead * U
    } else {
        l
    }
}

/// One guidance step: the new direction from `v` toward `t`.
pub fn steer(v: Vec3, t: Vec3, homing: f32) -> Vec3 {
    let n = homing * t + (1.0 - homing) * v;
    n.try_normalize().unwrap_or(t)
}

/// Ignition and guidance (the server's, `FlyerSet::Think`).
#[allow(clippy::type_complexity)]
pub fn missiles(
    mut missiles: Query<(&mut Flyer, &mut Hl2Missile, &Transform)>,
    rpgs: Query<(&Hl2Rpg, &Weapon, Option<&Hl2LaserDot>)>,
    owners: Query<(&Transform, &MovementState)>,
    clock: Res<SimClock>,
) {
    for (mut f, mut m, t) in &mut missiles {
        let Ok((rpg, w, dot)) = rpgs.get(m.rpg) else { continue };
        let p = &rpg.params;
        if !m.ignited {
            if clock.tick < m.ignite_tick {
                continue;
            }
            m.ignited = true;
            f.velocity = m.dir * p.speed * U;
            f.gravity = 0.0;
            f.half = Vec3::ZERO;
            f.solid_after = clock.now;
            continue;
        }
        // Its owner's dot only.
        let Some(dot) = dot.filter(|_| w.owner.is_some() && w.owner == f.owner) else {
            continue;
        };
        let Some((ot, os)) = f.owner.and_then(|o| owners.get(o).ok()) else {
            continue;
        };
        let c = ot.translation + (os.hull_min + os.hull_max) / 2.0;
        let a = aim_point(p, dot.at, dot.normal, dot.target.is_some(), c, t.translation);
        let speed = f.velocity.length();
        let v = f.velocity / speed.max(1e-6);
        let target = (a - t.translation).normalize_or(v);
        f.velocity = steer(v, target, p.homing) * speed;
    }
}

/// A missile gone: its RPG reloads (rockets left) and may be put away
/// (`FlyerSet::Touch`, the server's).
pub fn missile_gone(mut gone: MessageReader<FlyerGone>, mut rpgs: Query<(&mut Hl2Rpg, &Hl2Rounds)>, clock: Res<SimClock>) {
    for g in gone.read() {
        for (mut rpg, rounds) in &mut rpgs {
            if rpg.missile_entity != Some(g.flyer) {
                continue;
            }
            rpg.missile_entity = None;
            rpg.missile = false;
            if rounds.count > 0 {
                rpg.reload_until = Some(clock.now + rpg.params.reload as f64);
            }
        }
    }
}

/// The RPG's systems (`Hl2GunsPlugin`).
pub fn plugin(app: &mut App) {
    use chimera_sim::weapon::{CarriedAppExt, WeaponFrame};
    app.add_systems(crate::core::Predict::Weapons, before_frame.before(WeaponFrame))
        .add_systems(
            crate::core::Predict::Select,
            lock_rpg.before(chimera_sim::weapon::SelectWeapons),
        )
        .add_systems(
            FixedUpdate,
            (
                (new_missiles, place_dots, missiles).chain().in_set(FlyerSet::Think),
                missile_gone.in_set(FlyerSet::Touch),
            )
                .run_if(crate::core::authoritative),
        )
        .predicted_part::<Hl2Rpg>();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> RpgParams {
        launch::hook_params(super::super::RPG, super::super::RPG_HOOK)
    }

    /// projectiles.md test 29: one step at 90 degrees.
    #[test]
    fn turns_eight_degrees_a_tick_at_ninety() {
        let d = steer(Vec3::X, Vec3::Z, 0.125);
        assert!((d - Vec3::new(0.98995, 0.0, 0.14142)).length() < 1e-4, "{d}");
    }

    /// Test 30 (Source axes as engine x, z): riding the beam.
    #[test]
    fn rides_the_beam_from_the_owners_centre() {
        let p = params();
        let s = |x: f32, y: f32, z: f32| launch::to_engine(Vec3::new(x, y, z));
        let a = aim_point(
            &p,
            s(2000.0, 0.0, 36.0),
            Vec3::NEG_X,
            false,
            s(0.0, 0.0, 36.0),
            s(500.0, 100.0, 36.0),
        );
        assert!(
            (launch::to_source(a) - Vec3::new(756.0, 0.0, 36.0)).length() < 1e-2,
            "{}",
            launch::to_source(a)
        );
        let t = launch::to_source((a - s(500.0, 100.0, 36.0)).normalize() * U);
        assert!((t.normalize() - Vec3::new(0.93146, -0.36385, 0.0)).length() < 1e-4);
        // Test 31: a dot on a player: 10 into it.
        let b = aim_point(
            &p,
            s(2000.0, 0.0, 36.0),
            Vec3::NEG_X,
            true,
            s(0.0, 0.0, 36.0),
            s(500.0, 100.0, 36.0),
        );
        assert!((launch::to_source(b) - Vec3::new(2010.0, 0.0, 36.0)).length() < 1e-2);
    }
}
