//! The frag (specs/hl2dm/projectiles.md 9), `hl2dm:frag` (`Hl2Frag`, its
//! grenades in `launch::Hl2Rounds`: Grenade, 5 carried), predicted:
//!
//! - **Pull-back**: attack (throw) or attack2 (lob, or roll with duck held
//!   at the release) with grenades left: the pull-back (0.25 s) whose
//!   event ends it on tick 14; the first weapon frame after that with that
//!   button up starts the throw (0.55 s, released 4 ticks on), the lob or
//!   the roll (0.7 s, 7 ticks on). Holding keeps the pin pulled; the fuse
//!   hasn't started (no cooking). Put away pulled back: kept, unthrown.
//! - **Release** (the server makes the grenade): 1 used, next primary and
//!   secondary + 0.5; once the animation is over (seen a tick late, as the
//!   view model's) and the timers passed, the 1.2667 s draw (none with no
//!   grenades left: the next best weapon). A tapped throw: grenade on tick
//!   19, redraw 53, next press 138; a lob or roll: 22, 63, 148.
//! - **The grenade** (`Hl2Grenade`): a 1 kg physics body (avian, ±4 box,
//!   friction 0.9, elasticity 0.01, Source's weapon collision: through
//!   players, `core::ITEM_LAYER`); throw 1200·(forward + 0.1 up), lob 350
//!   forward + 50 up, roll 700 along the ground, each plus the thrower's
//!   velocity, from eye + 18 forward + 8 right (the lob 8 lower; the roll
//!   from the box's bottom + 4, 18 along the ground), pulled back to where
//!   a ±6 box from the eye (the roll: the box's centre) stops. It thinks
//!   on the tick after its creation and every 7 ticks: past 2.5 s it
//!   explodes (tick 169), blipping (`Grenade.Blip`) at creation, then each
//!   time past the next blip, every 1.0 s and every 0.3 s from 1.0 s:
//!   ticks 0, 71, 92, 113, 134, 155. A player in its way this tick
//!   (`player_taps`): it bounces off at 0.2 of its speed and taps him for
//!   0.1 crush from its thrower; starting inside one reverses it at 0.2.
//!   In flight it feels air drag (`air_drag`, measured on the real
//!   server): (0.07 + 0.00009 |v|) v per second, 0.176 of its speed a
//!   second at 1176 (a throw loses about 2 % of its speed in its first
//!   0.1 s), 0.10 at 350 (a lob).
//!   Explosion: a probe from 8 above to 24 below, the blast point 0.6 off
//!   the floor, 150 in 250.
//! - **Martyrdom** (`drop_live`): dying with the frag in hand and attack
//!   or attack2 held drops a live grenade (the throw's start, the dead
//!   body's velocity, a full fuse; one used if any) instead of the frag;
//!   one released but not yet out is lost.
//!
//! - **Spin** (9.2, `release_spin`): about the grenade's own axes, the
//!   throw's (600, U{-1200..1200}, 0) and the lob's (200, U{-600..600}, 0)
//!   degrees per second on an unturned grenade (the world's axes), the
//!   roll's (0, 0, 720) on one turned (0, body yaw, -90)
//!   (`roll_orientation`): a forward roll about the thrower's left.
//!
//! Known shortcuts: the throw and the lob are made unturned (the spec
//! gives only the roll's orientation); the martyrdom grenade's spin (600
//! about the right) is ours (the spec names none); the red glow and trail are
//! M10's; the gravity gun's catch (1.5 s fuse) is M6's; the roll's
//! projection onto a slope uses the floor under the box's centre.

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    Inventory, NetPart, WeaponEvent, WeaponEventKind, WeaponState,
    grenade::BlastAt,
    projectile::{ShownBody, reflect},
    random::shared_random,
};

use super::{
    alt_fire::makes_projectiles,
    launch::{self, Hl2Rounds, U},
};
use crate::{
    core::{
        Damage, DamageKind, Died, FirstTimePredicted, Health, Hitgroup, ITEM_LAYER, Intent, MovementState, NetRole,
        SOLID_LAYERS, SimClock, Velocity,
    },
    map::{PlaySound, held::Held, loose::ShownItem},
};

/// `hl2dm:frag`'s parameters (Source units, seconds, points, degrees/s).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FragParams {
    #[serde(flatten)]
    pub rounds: launch::RoundsParams,
    pub pull: f32,
    pub pull_event: f32,
    pub throw_anim: f32,
    pub lob_anim: f32,
    pub throw_release: f32,
    pub lob_release: f32,
    pub refire: f32,
    pub draw: f32,
    pub throw_speed: f32,
    pub throw_lift: f32,
    pub lob_speed: f32,
    pub lob_up: f32,
    pub roll_speed: f32,
    pub fuse: f32,
    pub warn: f32,
    pub blip_slow: f32,
    pub blip_fast: f32,
    pub think_ticks: u32,
    pub damage: f32,
    pub radius: f32,
    pub mass: f32,
    pub half: f32,
    pub friction: f32,
    pub elasticity: f32,
    pub player_bounce: f32,
    pub player_tap: f32,
    /// Air drag: each tick in flight the velocity v loses
    /// (`drag_linear` + `drag_quadratic` |v|) v dt (per second, per unit).
    pub drag_linear: f32,
    pub drag_quadratic: f32,
    pub blip: String,
    pub roll_sound: String,
    pub explode: String,
}

/// Where the frag is (`Hl2Frag::stage`).
pub const READY: u8 = 0;
pub const PULLING: u8 = 1;
pub const THROWING: u8 = 2;

/// How it leaves the hand.
pub const THROW: u8 = 0;
pub const LOB: u8 = 1;
pub const ROLL: u8 = 2;

/// The frag's state.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Hl2Frag {
    pub params: FragParams,
    pub stage: u8,
    /// Attack2 pulled it (lob or roll).
    pub secondary: bool,
    pub kind: u8,
    pub since: f64,
    pub released: bool,
}

impl NetPart for Hl2Frag {
    type Net = (u8, bool, u8, f64, bool);
    fn net(&self) -> Self::Net {
        (self.stage, self.secondary, self.kind, self.since, self.released)
    }
    fn set_net(&mut self, (stage, secondary, kind, since, released): Self::Net) {
        self.stage = stage;
        self.secondary = secondary;
        self.kind = kind;
        self.since = since;
        self.released = released;
    }
}

/// A live grenade (the server's): its thrower, its fuse and blips.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Hl2Grenade {
    pub thrower: Option<Entity>,
    pub created_tick: u64,
    pub explode_at: f64,
    pub warn_at: f64,
    pub next_blip: f64,
    pub warned: bool,
    /// The player it started inside (reversed once).
    pub inside: Option<Entity>,
    pub params: FragParams,
}

/// Where a grenade of `kind` starts and how fast it goes (engine space),
/// before the start's wall check: from the eye `eye` with the view basis,
/// the thrower's box bottom centre `feet` and the floor's normal under it,
/// and the thrower's velocity.
#[allow(clippy::too_many_arguments)]
pub fn release(
    p: &FragParams,
    kind: u8,
    eye: Vec3,
    fwd: Vec3,
    right: Vec3,
    feet: Vec3,
    floor: Option<Vec3>,
    carried: Vec3,
) -> (Vec3, Vec3) {
    match kind {
        THROW => (
            eye + (fwd * 18.0 + right * 8.0) * U,
            (fwd + Vec3::Y * p.throw_lift) * p.throw_speed * U + carried,
        ),
        LOB => (
            eye + (fwd * 18.0 + right * 8.0 - Vec3::Y * 8.0) * U,
            fwd * p.lob_speed * U + Vec3::Y * p.lob_up * U + carried,
        ),
        _ => {
            let p0 = feet + Vec3::Y * 4.0 * U;
            let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
            let f = match floor {
                Some(m) => m.cross(flat.cross(m)).normalize_or(flat),
                None => flat,
            };
            (p0 + f * 18.0 * U, f * p.roll_speed * U + carried)
        }
    }
}

/// Before the weapon frame: the pull-back, the release (where projectiles
/// are made, the grenade) and the redraw.
#[allow(clippy::type_complexity)]
pub fn before_frame(
    mut owners: Query<
        (
            Entity,
            &Intent,
            &mut Inventory,
            &Transform,
            &MovementState,
            &Health,
            Option<&Velocity>,
        ),
        Without<crate::map::interp::NetDrawn>,
    >,
    mut frags: Query<(&mut Hl2Frag, &mut Hl2Rounds, &mut WeaponState)>,
    others: Query<(&super::inventory::Hl2Item, Option<&chimera_sim::weapon::Magazine>)>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    role: Option<Res<NetRole>>,
    spatial: SpatialQuery,
    mut events: MessageWriter<WeaponEvent>,
    mut play: MessageWriter<PlaySound>,
    mut commands: Commands,
) {
    let (now, dt) = (clock.now, clock.dt() as f64);
    let due = now + 1e-5;
    for (owner, intent, mut inv, at, state, health, velocity) in &mut owners {
        let Some(active) = inv.active else { continue };
        let Ok((mut frag, mut rounds, mut st)) = frags.get_mut(active) else {
            continue;
        };
        if health.current <= 0.0 {
            continue;
        }
        let p = frag.params.clone();
        let mut event = |kind| {
            events.write(WeaponEvent {
                owner,
                weapon: active,
                kind,
                replay: !first.0,
            })
        };
        match frag.stage {
            READY => {
                if due < inv.next_attack || rounds.count == 0 {
                    continue;
                }
                let secondary = if intent.fire && st.next_primary <= due {
                    false
                } else if intent.secondary && st.next_secondary <= due {
                    true
                } else {
                    continue;
                };
                frag.stage = PULLING;
                frag.secondary = secondary;
                frag.since = now;
                st.next_primary = f64::MAX;
                st.next_secondary = f64::MAX;
                event(WeaponEventKind::PinPulled);
            }
            PULLING => {
                // The pull-back's event, seen by the next weapon frame.
                let done = now - dt - frag.since > p.pull_event as f64 + 1e-6;
                let held = if frag.secondary { intent.secondary } else { intent.fire };
                if !done || held {
                    continue;
                }
                frag.kind = if !frag.secondary {
                    THROW
                } else if intent.crouch {
                    ROLL
                } else {
                    LOB
                };
                frag.stage = THROWING;
                frag.since = now;
                frag.released = false;
                event(WeaponEventKind::Thrown);
            }
            _ => {
                let (offset, anim) = match frag.kind {
                    THROW => (p.throw_release, p.throw_anim),
                    _ => (p.lob_release, p.lob_anim),
                };
                if !frag.released {
                    if now - frag.since <= offset as f64 + 1e-6 {
                        continue;
                    }
                    frag.released = true;
                    rounds.count = rounds.count.saturating_sub(1);
                    st.next_primary = now + p.refire as f64;
                    st.next_secondary = now + p.refire as f64;
                    let (eye, fwd, right, _) = launch::aim(at, state, intent, None, false);
                    if frag.kind == ROLL && first.0 {
                        play.write(PlaySound::at(p.roll_sound.clone(), eye));
                    }
                    if makes_projectiles(role.as_deref(), &first) {
                        let feet = at.translation + Vec3::Y * state.hull_min.y;
                        let filter = SpatialQueryFilter::from_excluded_entities([owner]).with_mask(SOLID_LAYERS);
                        let floor = spatial
                            .cast_ray(feet + Vec3::Y * 4.0 * U, Dir3::NEG_Y, 16.0 * U, true, &filter)
                            .map(|h| h.normal);
                        let carried = velocity.map_or(Vec3::ZERO, |v| v.0);
                        let (start, v) = release(&p, frag.kind, eye, fwd, right, feet, floor, carried);
                        let from = if frag.kind == ROLL {
                            at.translation + (state.hull_min + state.hull_max) / 2.0
                        } else {
                            eye
                        };
                        let start = clear_start(&spatial, &filter, from, start);
                        let mut rng = shared_random(intent.command, "hl2dm:frag", 0);
                        let (rotation, spin) = match frag.kind {
                            THROW => release_spin(Quat::IDENTITY, 600.0, (rng.next() * 2400.0 - 1200.0).round(), 0.0),
                            LOB => release_spin(Quat::IDENTITY, 200.0, (rng.next() * 1200.0 - 600.0).round(), 0.0),
                            _ => release_spin(roll_orientation(intent.yaw), 0.0, 0.0, 720.0),
                        };
                        let thrower = Some(owner);
                        commands.queue(move |world: &mut World| {
                            spawn_turned(world, &p, thrower, start, rotation, v, spin);
                        });
                    }
                    continue;
                }
                let over = now - dt - frag.since >= anim as f64 - 1e-6;
                if !over || st.next_primary > due || st.next_secondary > due {
                    continue;
                }
                frag.stage = READY;
                if rounds.count > 0 {
                    let ready = now + p.draw as f64;
                    inv.next_attack = ready;
                    st.next_primary = ready;
                    st.next_secondary = ready;
                    event(WeaponEventKind::Deployed);
                } else {
                    // The shared empty rule's next best weapon.
                    let pick = super::inventory::next_best(&inv, |e| others.get(e).ok().map(|(i, m)| (*i, m.cloned())));
                    inv.wanted = pick;
                }
            }
        }
    }
}

/// Where a ±6 box swept from `from` toward `to` stops (the start's wall
/// check, 9.2).
fn clear_start(spatial: &SpatialQuery, filter: &SpatialQueryFilter, from: Vec3, to: Vec3) -> Vec3 {
    let d = to - from;
    let Ok(dir) = Dir3::new(d) else { return to };
    let shape = Collider::cuboid(12.0 * U, 12.0 * U, 12.0 * U);
    let config = ShapeCastConfig {
        max_distance: d.length(),
        ..ShapeCastConfig::DEFAULT
    };
    match spatial.cast_shape(&shape, from, Quat::IDENTITY, dir, &config, filter) {
        Some(h) => from + *dir * (h.distance - 1e-3).max(0.0),
        None => to,
    }
}

/// The roll's grenade's orientation (9.2: Source angles (0, body yaw,
/// -90)) for a thrower looking along `yaw` (Chimera's: forward is -Z
/// turned by it): its nose (the model's +X) along the facing, its own z
/// (up) to the thrower's left.
pub fn roll_orientation(yaw: f32) -> Quat {
    Quat::from_rotation_y(yaw + std::f32::consts::FRAC_PI_2) * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
}

/// A grenade's orientation `rotation` and its spin (rad/s, world) of
/// (`x`, `y`, `z`) degrees per second about its own Source axes (9.2;
/// Source's x, y, z are Chimera's x, -z, y). The throw and the lob are
/// made unturned (the table gives the roll alone an orientation): their
/// local axes are the world's.
pub fn release_spin(rotation: Quat, x: f32, y: f32, z: f32) -> (Quat, Vec3) {
    let local = Vec3::new(x, z, -y) * std::f32::consts::PI / 180.0;
    (rotation, rotation * local)
}

/// A live grenade of `p` at `start` with `velocity` (m/s) and `spin`
/// (rad/s), thrown by `thrower`: the physics body, its fuse, its first
/// blip.
pub fn spawn_grenade(
    world: &mut World,
    p: &FragParams,
    thrower: Option<Entity>,
    start: Vec3,
    velocity: Vec3,
    spin: Vec3,
) -> Entity {
    spawn_turned(world, p, thrower, start, Quat::IDENTITY, velocity, spin)
}

/// `spawn_grenade` turned by `rotation`.
pub fn spawn_turned(
    world: &mut World,
    p: &FragParams,
    thrower: Option<Entity>,
    start: Vec3,
    rotation: Quat,
    velocity: Vec3,
    spin: Vec3,
) -> Entity {
    let clock = *world.resource::<SimClock>();
    let half = p.half * U;
    let e = world
        .spawn((
            Name::new("npc_grenade_frag"),
            Transform::from_translation(start).with_rotation(rotation),
            RigidBody::Dynamic,
            Collider::cuboid(half * 2.0, half * 2.0, half * 2.0),
            CollisionLayers::new(ITEM_LAYER, LayerMask::ALL),
            Mass(p.mass),
            Friction::new(p.friction),
            Restitution::new(p.elasticity),
            LinearVelocity(velocity),
            AngularVelocity(spin),
            SweptCcd::default(),
            ShownItem(super::FRAG.to_string()),
            ShownBody { owner: thrower },
            Hl2Grenade {
                thrower,
                created_tick: clock.tick,
                explode_at: clock.now + p.fuse as f64,
                warn_at: clock.now + p.warn as f64,
                next_blip: clock.now + p.blip_slow as f64,
                warned: false,
                inside: None,
                params: p.clone(),
            },
        ))
        .id();
    world.write_message(PlaySound {
        pitch: None,
        entry: p.blip.clone(),
        at: Some(start),
        volume: None,
        source: Some(e),
        channel: None,
    });
    e
}

/// Before each physics step: grenades in flight (not held by a gravity
/// gun, which switches drag off) slow down by their air drag (9.4).
pub fn air_drag(time: Res<Time>, mut grenades: Query<(&Hl2Grenade, &mut LinearVelocity), Without<Held>>) {
    let dt = time.delta_secs();
    for (g, mut v) in &mut grenades {
        let speed = v.0.length() / U;
        let slow = ((g.params.drag_linear + g.params.drag_quadratic * speed) * dt).min(1.0);
        v.0 *= 1.0 - slow;
    }
}

/// The grenades' thinks (the server's): every 7 ticks from the one after
/// their creation, the fuse, the warning, the blips.
pub fn think(
    mut grenades: Query<(Entity, &mut Hl2Grenade, &Transform)>,
    clock: Res<SimClock>,
    mut play: MessageWriter<PlaySound>,
    mut blasts: MessageWriter<BlastAt>,
    mut commands: Commands,
) {
    let now = clock.now;
    for (e, mut g, t) in &mut grenades {
        let age = clock.tick.saturating_sub(g.created_tick);
        if age == 0 || (age - 1) % g.params.think_ticks.max(1) as u64 != 0 {
            continue;
        }
        if now > g.explode_at + 1e-6 {
            blasts.write(BlastAt {
                at: t.translation,
                blast: launch::blast(
                    g.params.damage,
                    g.params.radius,
                    Some((8.0, 24.0, 0.6)),
                    Some(&g.params.explode),
                ),
                thrower: g.thrower,
                weapon: super::FRAG,
                inflictor: Some(e),
            });
            commands.entity(e).despawn();
            continue;
        }
        if now + 1e-6 >= g.warn_at {
            g.warned = true;
        }
        if now > g.next_blip + 1e-6 {
            play.write(PlaySound {
                pitch: None,
                entry: g.params.blip.clone(),
                at: Some(t.translation),
                volume: None,
                source: Some(e),
                channel: None,
            });
            g.next_blip = now
                + if g.warned {
                    g.params.blip_fast
                } else {
                    g.params.blip_slow
                } as f64;
        }
    }
}

/// Players in a grenade's way (9.4): it passes through them, but one it
/// meets this tick turns it back at 0.2 and takes 0.1 crush from its
/// thrower; one it starts inside reverses it at 0.2, once.
#[allow(clippy::type_complexity)]
pub fn player_taps(
    mut grenades: Query<(&mut Hl2Grenade, &Transform, &mut LinearVelocity, &mut AngularVelocity)>,
    players: Query<(Entity, &Health), With<Intent>>,
    spatial: SpatialQuery,
    colliders: Query<&ColliderOf>,
    clock: Res<SimClock>,
    mut damage: MessageWriter<Damage>,
) {
    let dt = clock.dt();
    for (mut g, t, mut v, mut spin) in &mut grenades {
        let alive = |b: Entity| players.get(b).is_ok_and(|(_, h)| h.current > 0.0);
        let body = |c: Entity| colliders.get(c).map_or(c, |o| o.body);
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        let inside = spatial
            .point_intersections(t.translation, &filter)
            .into_iter()
            .map(body)
            .find(|b| alive(*b) && Some(*b) != g.thrower);
        match (inside, g.inside) {
            (Some(p), None) => {
                v.0 *= -g.params.player_bounce;
                g.inside = Some(p);
                continue;
            }
            (Some(_), Some(_)) => continue,
            (None, Some(_)) => g.inside = None,
            (None, None) => {}
        }
        let step = v.0 * dt;
        let Ok(dir) = Dir3::new(step) else { continue };
        let thrower = g.thrower;
        let hit = spatial.cast_ray_predicate(t.translation, dir, step.length(), true, &filter, &|c| {
            let b = body(c);
            alive(b) && Some(b) != thrower
        });
        let Some(h) = hit else { continue };
        let victim = body(h.entity);
        let n = h.normal.normalize_or(-*dir);
        v.0 = reflect(v.0, n) * g.params.player_bounce;
        spin.0 *= -0.5;
        damage.write(Damage {
            target: victim,
            attacker: g.thrower,
            amount: g.params.player_tap * launch::HP,
            point: t.translation + *dir * h.distance,
            dir: *dir,
            hitgroup: Hitgroup::Generic,
            kind: DamageKind::Crush,
            weapon: Some(super::FRAG),
            force: Vec3::ZERO,
        });
    }
}

/// Martyrdom (9.5): a holder dying with the frag out and a button held
/// drops a live grenade instead of the frag.
#[allow(clippy::type_complexity)]
pub fn drop_live(
    mut died: MessageReader<Died>,
    mut owners: Query<(&mut Inventory, &Transform, &MovementState, &Intent, Option<&Velocity>)>,
    mut frags: Query<(&mut Hl2Frag, &mut Hl2Rounds)>,
    mut commands: Commands,
) {
    for d in died.read() {
        let Ok((mut inv, at, state, intent, vel)) = owners.get_mut(d.entity) else {
            continue;
        };
        let Some(active) = inv.active else { continue };
        let Ok((mut frag, mut rounds)) = frags.get_mut(active) else {
            continue;
        };
        let lost = frag.stage == THROWING && !frag.released;
        frag.stage = READY;
        if lost || !(intent.fire || intent.secondary) {
            continue;
        }
        rounds.count = rounds.count.saturating_sub(1);
        let (eye, fwd, right, _) = launch::aim(at, state, intent, None, false);
        let p = frag.params.clone();
        let start = eye + (fwd * 18.0 + right * 8.0) * U;
        let velocity = vel.map_or(Vec3::ZERO, |v| v.0);
        let spin = right * 600.0f32.to_radians();
        let thrower = Some(d.entity);
        // The frag goes with its holder, not to the floor.
        inv.weapons.retain(|w| *w != active);
        inv.active = None;
        commands.entity(active).despawn();
        commands.queue(move |world: &mut World| {
            spawn_grenade(world, &p, thrower, start, velocity, spin);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 9.2: the spins are about the grenade's own axes. The throw's 600
    /// about Source's x (Chimera's x), its random part about Source's y
    /// (Chimera's -z); the roll turned (0, body yaw, -90), spinning 720
    /// about its own z: the thrower's left, a forward roll.
    #[test]
    fn spins_about_the_grenades_own_axes() {
        let d = std::f32::consts::PI / 180.0;
        let (r, w) = release_spin(Quat::IDENTITY, 600.0, 300.0, 0.0);
        assert_eq!(r, Quat::IDENTITY);
        assert!((w - Vec3::new(600.0, 0.0, -300.0) * d).length() < 1e-4, "{w}");
        for yaw in [0.0f32, 1.0, -2.5] {
            let (r, w) = release_spin(roll_orientation(yaw), 0.0, 0.0, 720.0);
            let forward = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
            let left = Quat::from_rotation_y(yaw) * Vec3::NEG_X;
            assert!((r * Vec3::X - forward).length() < 1e-5, "nose along the facing");
            assert!((w - left * 720.0 * d).length() < 1e-3, "{w}");
            // Rolling forward: the bottom moves back, the top forward.
            assert!(w.cross(Vec3::Y).dot(forward) > 0.0);
        }
    }

    pub(crate) fn params() -> FragParams {
        launch::hook_params(super::super::FRAG, super::super::FRAG_HOOK)
    }

    /// projectiles.md tests 44-48: the start points and speeds, level and
    /// looking up 30 degrees, moving, the lob and the roll.
    #[test]
    fn throw_lob_and_roll_speeds() {
        let p = params();
        let s = launch::to_source;
        // Looking along Source's +X: right is Source's -Y, engine +Z.
        let (eye, fwd, right) = (Vec3::Y * 64.0 * U, Vec3::X, Vec3::Z);
        let (start, v) = release(&p, THROW, eye, fwd, right, Vec3::ZERO, None, Vec3::ZERO);
        assert!((s(v) - Vec3::new(1200.0, 0.0, 120.0)).length() < 1e-2, "{}", s(v));
        assert!(
            (s(start - eye) - Vec3::new(18.0, -8.0, 0.0)).length() < 1e-3,
            "{}",
            s(start - eye)
        );
        let up = Vec3::new(30f32.to_radians().cos(), 30f32.to_radians().sin(), 0.0);
        let (_, v) = release(&p, THROW, eye, up, right, Vec3::ZERO, None, Vec3::ZERO);
        assert!((s(v) - Vec3::new(1039.23, 0.0, 720.0)).length() < 1e-1, "{}", s(v));
        let carried = launch::to_engine(Vec3::new(100.0, 50.0, 0.0));
        let (_, v) = release(&p, THROW, eye, fwd, right, Vec3::ZERO, None, carried);
        assert!((s(v) - Vec3::new(1300.0, 50.0, 120.0)).length() < 1e-2);
        let (start, v) = release(&p, LOB, eye, fwd, right, Vec3::ZERO, None, Vec3::ZERO);
        assert!((s(v) - Vec3::new(350.0, 0.0, 50.0)).length() < 1e-2);
        assert!((s(start - eye) - Vec3::new(18.0, -8.0, -8.0)).length() < 1e-3);
        let (start, v) = release(&p, ROLL, eye, fwd, right, Vec3::ZERO, Some(Vec3::Y), Vec3::ZERO);
        assert!((s(start) - Vec3::new(18.0, 0.0, 4.0)).length() < 1e-3, "{}", s(start));
        assert!((s(v) - Vec3::new(700.0, 0.0, 0.0)).length() < 1e-2);
    }
}
