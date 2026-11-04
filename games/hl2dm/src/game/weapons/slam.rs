//! The S.L.A.M. (specs/hl2dm/projectiles.md 8), `hl2dm:slam` (`Hl2Slam`,
//! its charges in `launch::Hl2Rounds`: slam, 5 carried, 3 a pickup),
//! predicted:
//!
//! - **Modes**: trip-mine mode when a solid line 42 units along the view
//!   hits something that isn't a player, else throw mode; a change plays
//!   its transition (throw to mine 1.3333 s; mine to throw 1.6667 s, 1.0
//!   with the detonator armed), during which neither attack goes and the
//!   mode doesn't flip back.
//! - **Trip mine**: attack in mine mode, with a surface within 128 along
//!   the view: the attach (0.2333 s); on the first tick strictly after it
//!   (tick 16) the line is traced again and the mine (`Hl2Mine`) placed at
//!   the hit + 3 along the normal, 1 used; the next placement 0.6 + 0.8333
//!   s on. The mine (±4, ±4, ±2, frozen, through players, hit by bullets
//!   and blasts) checks 13 ticks after its creation and then every 7; it
//!   arms on the first check after 2 s (tick 139: `TripmineGrenade.
//!   Activate`) with the beam's length to where a line 2048 along its
//!   normal stops (through players); then 7 ticks on and every 3 it trips
//!   when the line meets any player or its length changes by more than
//!   2.048. Tripped or damaged (armed or not): 17 ticks on, 200 in 200 at
//!   its point + 8 along the normal, credited to its placer.
//! - **Satchel**: attack in throw mode: the throw (0.4667 s), the
//!   detonator armed at its start; at its end (tick 32) the satchel
//!   (`Hl2Satchel`: a 1 kg physics body through players) leaves from the
//!   owner's centre + 18 along the view + 24 up at the owner's velocity
//!   + 500 along the view, 1 used; the next throw 0.8 + 1.0 s on.
//! - **Detonator**: attack2 with it armed, not in mine mode and not in an
//!   action: every satchel of this player goes off 0.2 s later (14 ticks):
//!   150 in 200 at the satchel + 16 up; the detonator is disarmed. A
//!   satchel that takes damage is removed with a harmless pop
//!   (`SatchelPop`, drawn by `projectile_fx`; a `Scorch` within 24 under
//!   it, `BaseGrenade.Explode` heard by everyone); its thrower's death
//!   sets them all off 0.2 s later. A disconnect (8.5): in DM they stay
//!   live; in TDM the leaver is killed first, so they go off
//!   (`leaver_satchels`).
//! - With no slam left and no satchels out the S.L.A.M. leaves the
//!   inventory.
//!
//! Known shortcuts: the beam, the strobe and the animations' drawing are
//! M10's; the detonate animation's length (1.3333 s, 0.6667 s
//! detonator-only) holds both attacks; a satchel near a wall at the throw's
//! end is set down 5 units ahead at 1 unit/s (the spec's) only when the
//! 42-unit line still hits; a TDM leaver's satchels have no thrower left to
//! credit (its entity is gone).

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    Inventory, NetPart, Weapon, WeaponEvent, WeaponEventKind, WeaponState, grenade::BlastAt, projectile::ShownBody,
};

use super::{
    alt_fire::makes_projectiles,
    launch::{self, Hl2Rounds, MINE_MODEL, SATCHEL_MODEL, U},
};
use crate::{
    core::{
        Damage, Damageable, Died, FirstTimePredicted, Health, ITEM_LAYER, Intent, MovementState, NetRole, SOLID_LAYERS,
        SimClock, Velocity,
    },
    map::{PlaySound, loose::ShownItem},
};

/// `hl2dm:slam`'s parameters (Source units, seconds, ticks, points).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SlamParams {
    #[serde(flatten)]
    pub rounds: launch::RoundsParams,
    pub mode_reach: f32,
    pub attach_reach: f32,
    pub to_mine: f32,
    pub to_throw: f32,
    pub to_throw_detonator: f32,
    pub attach: f32,
    pub attach_after: f32,
    pub throw_anim: f32,
    pub throw_after: f32,
    pub detonate: f32,
    pub detonate_only: f32,
    pub satchel_speed: f32,
    pub satchel_fuse: f32,
    pub satchel_damage: f32,
    pub satchel_radius: f32,
    pub mine_offset: f32,
    pub mine_first_check: u32,
    pub mine_check: u32,
    pub mine_arm: f32,
    pub beam: f32,
    pub beam_tolerance: f32,
    pub beam_first_check: u32,
    pub beam_check: u32,
    pub mine_delay: u32,
    pub mine_damage: f32,
    pub mine_radius: f32,
    pub place_sound: String,
    pub activate_sound: String,
    pub stop_sound: String,
    pub throw_sound: String,
    pub detonate_sound: String,
    pub explode: String,
}

/// Its actions (`Hl2Slam::action`).
pub const IDLE: u8 = 0;
pub const ATTACHING: u8 = 1;
pub const THROWING: u8 = 2;

/// The S.L.A.M.'s state.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Hl2Slam {
    pub params: SlamParams,
    /// Trip-mine mode (else throw mode).
    pub mine_mode: bool,
    /// No mode change before this (a transition plays).
    pub transition_until: f64,
    pub detonator: bool,
    pub action: u8,
    pub action_end: f64,
}

impl NetPart for Hl2Slam {
    type Net = (bool, f64, bool, u8, f64);
    fn net(&self) -> Self::Net {
        (
            self.mine_mode,
            self.transition_until,
            self.detonator,
            self.action,
            self.action_end,
        )
    }
    fn set_net(&mut self, (m, t, d, a, e): Self::Net) {
        self.mine_mode = m;
        self.transition_until = t;
        self.detonator = d;
        self.action = a;
        self.action_end = e;
    }
}

/// A satchel (the server's): its thrower, and when it goes off.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Hl2Satchel {
    pub thrower: Option<Entity>,
    pub explode_at: Option<f64>,
    pub params: SlamParams,
}

/// A trip mine (the server's).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Hl2Mine {
    pub placer: Option<Entity>,
    /// Its surface's normal (engine space).
    pub normal: Vec3,
    pub created_tick: u64,
    pub created: f64,
    /// Armed with this beam length (m), and the tick it armed.
    pub beam: Option<(f32, u64)>,
    /// Goes off on this tick.
    pub explode_tick: Option<u64>,
    pub params: SlamParams,
}

/// A request from the weapon frame to the server: place a mine, throw a
/// satchel, detonate.
#[derive(Message, Clone, Debug)]
pub enum SlamAction {
    Mine {
        placer: Entity,
        at: Vec3,
        normal: Vec3,
        params: SlamParams,
    },
    Satchel {
        thrower: Entity,
        at: Vec3,
        velocity: Vec3,
        params: SlamParams,
    },
    Detonate {
        thrower: Entity,
    },
}

/// What a solid line from `eye` along `dir` meets within `reach`: the
/// point, its normal and whether it is a player.
fn ahead(
    spatial: &SpatialQuery,
    colliders: &Query<&ColliderOf>,
    players: &Query<(), With<Intent>>,
    owner: Entity,
    eye: Vec3,
    dir: Vec3,
    reach: f32,
) -> Option<(Vec3, Vec3, bool)> {
    let d = Dir3::new(dir).ok()?;
    let filter =
        SpatialQueryFilter::from_excluded_entities([owner]).with_mask(LayerMask(SOLID_LAYERS.0 & !ITEM_LAYER.0));
    let h = spatial.cast_ray(eye, d, reach, true, &filter)?;
    let body = colliders.get(h.entity).map_or(h.entity, |c| c.body);
    Some((eye + dir * h.distance, h.normal, players.contains(body)))
}

/// Before the weapon frame: modes, placing, throwing, detonating.
#[allow(clippy::type_complexity)]
pub fn before_frame(
    owners: Query<
        (
            Entity,
            &Intent,
            &Inventory,
            &Transform,
            &MovementState,
            &Health,
            Option<&Velocity>,
        ),
        Without<crate::map::interp::NetDrawn>,
    >,
    mut slams: Query<(&mut Hl2Slam, &mut Hl2Rounds, &mut WeaponState)>,
    spatial: SpatialQuery,
    colliders: Query<&ColliderOf>,
    players: Query<(), With<Intent>>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    role: Option<Res<NetRole>>,
    mut events: MessageWriter<WeaponEvent>,
    mut actions: MessageWriter<SlamAction>,
    mut play: MessageWriter<PlaySound>,
) {
    let now = clock.now;
    let due = now + 1e-5;
    let makes = makes_projectiles(role.as_deref(), &first);
    for (owner, intent, inv, at, state, health, velocity) in &owners {
        if health.current <= 0.0 {
            continue;
        }
        let Some(active) = inv.active else { continue };
        let Ok((mut slam, mut rounds, mut st)) = slams.get_mut(active) else {
            continue;
        };
        let p = slam.params.clone();
        let (eye, fwd, ..) = launch::aim(at, state, intent, None, false);
        let mut event = |kind| {
            events.write(WeaponEvent {
                owner,
                weapon: active,
                kind,
                replay: !first.0,
            })
        };
        // An action's end: the mine goes on, the satchel leaves.
        if slam.action != IDLE && now > slam.action_end + 1e-6 {
            let action = std::mem::replace(&mut slam.action, IDLE);
            if action == ATTACHING {
                if let Some((hit, n, false)) =
                    ahead(&spatial, &colliders, &players, owner, eye, fwd, p.attach_reach * U)
                {
                    rounds.count = rounds.count.saturating_sub(1);
                    if first.0 {
                        play.write(PlaySound::at(p.place_sound.clone(), hit));
                    }
                    if makes {
                        actions.write(SlamAction::Mine {
                            placer: owner,
                            at: hit + n * p.mine_offset * U,
                            normal: n,
                            params: p.clone(),
                        });
                    }
                }
                let next = now + (p.attach_after) as f64;
                st.next_primary = next;
                st.next_secondary = next;
            } else {
                rounds.count = rounds.count.saturating_sub(1);
                let centre = at.translation + (state.hull_min + state.hull_max) / 2.0;
                let carried = velocity.map_or(Vec3::ZERO, |v| v.0);
                let (start, v) = match ahead(&spatial, &colliders, &players, owner, eye, fwd, p.mode_reach * U) {
                    Some(_) => (centre + fwd * 5.0 * U, fwd * U),
                    None => (
                        centre + fwd * 18.0 * U + Vec3::Y * 24.0 * U,
                        carried + fwd * p.satchel_speed * U,
                    ),
                };
                if first.0 {
                    play.write(PlaySound::at(p.throw_sound.clone(), eye));
                }
                if makes {
                    actions.write(SlamAction::Satchel {
                        thrower: owner,
                        at: start,
                        velocity: v,
                        params: p.clone(),
                    });
                }
                let next = now + p.throw_after as f64;
                st.next_primary = next;
                st.next_secondary = next;
            }
            continue;
        }
        if slam.action != IDLE || due < inv.next_attack {
            continue;
        }
        // The mode follows what is ahead, once a transition is over.
        if now + 1e-6 >= slam.transition_until {
            let wall = ahead(&spatial, &colliders, &players, owner, eye, fwd, p.mode_reach * U)
                .is_some_and(|(.., player)| !player);
            if wall != slam.mine_mode {
                slam.mine_mode = wall;
                let t = if wall {
                    p.to_mine
                } else if slam.detonator {
                    p.to_throw_detonator
                } else {
                    p.to_throw
                };
                slam.transition_until = now + t as f64;
                st.next_primary = slam.transition_until;
                st.next_secondary = slam.transition_until;
                event(WeaponEventKind::Action(if wall { "to_mine" } else { "to_throw" }));
                continue;
            }
        }
        if now + 1e-6 < slam.transition_until {
            continue;
        }
        if intent.secondary && st.next_secondary <= due && slam.detonator && !slam.mine_mode {
            slam.detonator = false;
            let t = if rounds.count > 0 { p.detonate } else { p.detonate_only };
            st.next_primary = now + t as f64;
            st.next_secondary = now + t as f64;
            if first.0 {
                play.write(PlaySound::at(p.detonate_sound.clone(), eye));
            }
            if makes {
                actions.write(SlamAction::Detonate { thrower: owner });
            }
            event(WeaponEventKind::Action("detonate"));
            continue;
        }
        if !intent.fire || st.next_primary > due || rounds.count == 0 {
            continue;
        }
        if slam.mine_mode {
            if ahead(&spatial, &colliders, &players, owner, eye, fwd, p.attach_reach * U)
                .is_some_and(|(.., player)| !player)
            {
                slam.action = ATTACHING;
                slam.action_end = now + p.attach as f64;
                st.next_primary = f64::MAX;
                event(WeaponEventKind::Action("attach"));
            }
        } else {
            slam.action = THROWING;
            slam.action_end = now + p.throw_anim as f64;
            slam.detonator = true;
            st.next_primary = f64::MAX;
            st.next_secondary = f64::MAX;
            event(WeaponEventKind::Action("throw"));
        }
    }
}

/// A player leaving (its entity removed) in TDM while alive is killed on
/// the way (rules.md; `rules::players`' `left`), so its satchels go off
/// 0.2 s later as at a death (8.5); in DM they stay live.
pub fn leaver_satchels(
    remove: On<Remove<crate::game::rules::Hl2Player>>,
    dead: Query<(), With<crate::core::Dead>>,
    game: Option<Res<crate::game::rules::Hl2Match>>,
    clock: Res<SimClock>,
    mut satchels: Query<&mut Hl2Satchel>,
) {
    if !game.is_some_and(|m| m.on && m.teamplay) || dead.contains(remove.entity) {
        return;
    }
    for mut s in &mut satchels {
        if s.thrower == Some(remove.entity) && s.explode_at.is_none() {
            s.explode_at = Some(clock.now + s.params.satchel_fuse as f64);
        }
    }
}

/// The S.L.A.M.'s requests on the server: mines placed, satchels thrown,
/// detonations; a dead thrower's satchels go off.
pub fn actions(
    mut requests: MessageReader<SlamAction>,
    mut died: MessageReader<Died>,
    mut satchels: Query<&mut Hl2Satchel>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let set_off = |thrower: Entity, satchels: &mut Query<&mut Hl2Satchel>| {
        for mut s in satchels.iter_mut() {
            if s.thrower == Some(thrower) && s.explode_at.is_none() {
                s.explode_at = Some(clock.now + s.params.satchel_fuse as f64);
            }
        }
    };
    for d in died.read() {
        set_off(d.entity, &mut satchels);
    }
    for r in requests.read() {
        match r.clone() {
            SlamAction::Detonate { thrower } => set_off(thrower, &mut satchels),
            SlamAction::Mine {
                placer,
                at,
                normal,
                params,
            } => {
                commands.spawn((
                    Name::new("npc_tripmine"),
                    Transform::from_translation(at).with_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, normal)),
                    RigidBody::Static,
                    Collider::cuboid(8.0 * U, 8.0 * U, 4.0 * U),
                    CollisionLayers::new(ITEM_LAYER, LayerMask::ALL),
                    Damageable,
                    ShownItem(MINE_MODEL.into()),
                    ShownBody { owner: Some(placer) },
                    Hl2Mine {
                        placer: Some(placer),
                        normal,
                        created_tick: clock.tick,
                        created: clock.now,
                        beam: None,
                        explode_tick: None,
                        params,
                    },
                ));
            }
            SlamAction::Satchel {
                thrower,
                at,
                velocity,
                params,
            } => {
                commands.spawn((
                    Name::new("npc_satchel"),
                    Transform::from_translation(at),
                    RigidBody::Dynamic,
                    Collider::cuboid(12.0 * U, 4.0 * U, 12.0 * U),
                    CollisionLayers::new(ITEM_LAYER, LayerMask::ALL),
                    Mass(1.0),
                    LinearVelocity(velocity),
                    AngularVelocity(Vec3::Y * 400f32.to_radians()),
                    SweptCcd::default(),
                    Damageable,
                    ShownItem(SATCHEL_MODEL.into()),
                    ShownBody { owner: Some(thrower) },
                    Hl2Satchel {
                        thrower: Some(thrower),
                        explode_at: None,
                        params,
                    },
                ));
            }
        }
    }
}

/// A shot satchel's harmless pop (8.6) at `at` (where the generic
/// grenade explosion puts it): every machine draws it (`projectile_fx`).
#[derive(Message, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SatchelPop {
    pub at: Vec3,
}

/// Where a pop goes off and what it scorches (the generic grenade
/// explosion, as the frag's 9.6): a line from 8 units above `at` to 32
/// below that; on a hit, the point 0.6 off it and the hit (point,
/// normal), else `at`.
pub fn pop_point(at: Vec3, world: &impl crate::map::particles::WorldTrace) -> (Vec3, Option<(Vec3, Vec3)>) {
    let from = at + Vec3::Y * 8.0 * U;
    let to = from - Vec3::Y * 32.0 * U;
    match world.trace(from, to) {
        Some((f, n)) => {
            let hit = from + (to - from) * f;
            (hit + n * 0.6 * U, Some((hit, n)))
        }
        None => (at, None),
    }
}

/// The scorch decal group of HL2:DM's explosions.
pub const SCORCH: &str = "Scorch";

/// Satchels: shot ones pop harmlessly (`SatchelPop`, a `Scorch` under it,
/// `BaseGrenade.Explode` heard by everyone), set-off ones explode (the
/// server's).
#[allow(clippy::too_many_arguments)]
pub fn satchels(
    satchels: Query<(Entity, &Hl2Satchel, &Transform)>,
    mut damage: MessageReader<Damage>,
    clock: Res<SimClock>,
    mut blasts: MessageWriter<BlastAt>,
    mut sounds: MessageWriter<crate::map::GameSound>,
    mut pops: MessageWriter<SatchelPop>,
    mut decals: MessageWriter<crate::map::PlaceDecal>,
    world: crate::map::particles::WorldTracer,
    mut commands: Commands,
) {
    let shot: Vec<Entity> = damage
        .read()
        .map(|d| d.target)
        .filter(|t| satchels.contains(*t))
        .collect();
    for (e, s, t) in &satchels {
        if s.explode_at.is_some_and(|x| clock.now + 1e-6 >= x) {
            let p = &s.params;
            blasts.write(BlastAt {
                at: t.translation + Vec3::Y * 16.0 * U,
                blast: launch::blast(p.satchel_damage, p.satchel_radius, None, Some(&p.explode)),
                thrower: s.thrower,
                weapon: super::SLAM,
                inflictor: Some(e),
            });
            commands.entity(e).despawn();
        } else if shot.contains(&e) {
            let (at, hit) = pop_point(t.translation, &world);
            sounds.write(crate::map::GameSound(PlaySound::at("BaseGrenade.Explode", at)));
            pops.write(SatchelPop { at });
            if let Some((point, normal)) = hit {
                decals.write(crate::map::PlaceDecal {
                    target: None,
                    group: crate::map::DecalGroup::Named(SCORCH.into()),
                    point,
                    normal,
                    dir: -normal,
                    spin: true,
                    round: false,
                });
            }
            commands.entity(e).despawn();
        }
    }
}

/// The trip mines' checks (the server's): arming, the beam, damage, the
/// explosion.
#[allow(clippy::type_complexity)]
pub fn mines(
    mut mines: Query<(Entity, &mut Hl2Mine, &Transform)>,
    players: Query<(), With<Intent>>,
    spatial: SpatialQuery,
    colliders: Query<&ColliderOf>,
    mut damage: MessageReader<Damage>,
    clock: Res<SimClock>,
    mut blasts: MessageWriter<BlastAt>,
    mut play: MessageWriter<PlaySound>,
    mut commands: Commands,
) {
    let hurt: Vec<Entity> = damage.read().map(|d| d.target).collect();
    let tick = clock.tick;
    for (e, mut m, t) in &mut mines {
        let p = m.params.clone();
        if let Some(x) = m.explode_tick {
            if tick >= x {
                blasts.write(BlastAt {
                    at: t.translation + m.normal * 8.0 * U,
                    blast: launch::blast(p.mine_damage, p.mine_radius, None, Some(&p.explode)),
                    thrower: m.placer,
                    weapon: super::SLAM,
                    inflictor: Some(e),
                });
                commands.entity(e).despawn();
            }
            continue;
        }
        let trip = |m: &mut Hl2Mine, play: &mut MessageWriter<PlaySound>| {
            m.explode_tick = Some(tick + p.mine_delay as u64);
            play.write(PlaySound::at(p.stop_sound.clone(), t.translation));
        };
        if hurt.contains(&e) {
            trip(&mut m, &mut play);
            continue;
        }
        let Ok(dir) = Dir3::new(m.normal) else { continue };
        let reach = p.beam * U;
        let from = t.translation;
        let body = |c: Entity| colliders.get(c).map_or(c, |o| o.body);
        let age = tick.saturating_sub(m.created_tick);
        match m.beam {
            None => {
                let first = p.mine_first_check as u64;
                if age < first || (age - first) % p.mine_check.max(1) as u64 != 0 {
                    continue;
                }
                if clock.now - m.created <= p.mine_arm as f64 + 1e-6 {
                    continue;
                }
                // The beam's length, through players.
                let filter = SpatialQueryFilter::from_excluded_entities([e]).with_mask(SOLID_LAYERS);
                let len = spatial
                    .cast_ray_predicate(from, dir, reach, true, &filter, &|c| !players.contains(body(c)))
                    .map_or(reach, |h| h.distance);
                m.beam = Some((len, tick));
                play.write(PlaySound::at(p.activate_sound.clone(), t.translation));
            }
            Some((len, armed)) => {
                let since = tick - armed;
                let first = p.beam_first_check as u64;
                if since < first || (since - first) % p.beam_check.max(1) as u64 != 0 {
                    continue;
                }
                let filter = SpatialQueryFilter::from_excluded_entities([e])
                    .with_mask(LayerMask(SOLID_LAYERS.0 & !ITEM_LAYER.0));
                let hit = spatial.cast_ray(from, dir, reach, true, &filter);
                let player = hit.is_some_and(|h| players.contains(body(h.entity)));
                let now_len = hit.map_or(reach, |h| h.distance);
                if player || (now_len - len).abs() > p.beam_tolerance * U {
                    trip(&mut m, &mut play);
                }
            }
        }
    }
}

/// With no charge left and no satchel of its owner out, the S.L.A.M.
/// leaves the inventory.
pub fn spent(
    mut owners: Query<&mut Inventory>,
    slams: Query<(Entity, &Hl2Rounds, &Weapon, &Hl2Slam)>,
    satchels: Query<&Hl2Satchel>,
    mut commands: Commands,
) {
    for (e, rounds, w, slam) in &slams {
        if rounds.count > 0 || slam.action != IDLE {
            continue;
        }
        let Some(owner) = w.owner else { continue };
        if satchels
            .iter()
            .any(|s| s.thrower == Some(owner) && s.explode_at.is_none())
        {
            continue;
        }
        let Ok(mut inv) = owners.get_mut(owner) else { continue };
        inv.weapons.retain(|x| *x != e);
        if inv.active == Some(e) {
            inv.active = None;
            inv.wanted = inv.last.filter(|l| *l != e).or(inv.weapons.first().copied());
        }
        commands.entity(e).despawn();
    }
}
