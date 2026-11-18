use avian3d::prelude::{
    AngularVelocity, Collider, ColliderAabb, ColliderDisabled, CollisionLayers, CollisionStart, ComputedMass,
    LayerMask, LinearVelocity, PhysicsSystems, Position, RigidBody, RigidBodyDisabled, Rotation,
};
use bevy::{ecs::message::MessageCursor, prelude::*};
use chimera_core::query_cache::{cached_query, first};

use crate::logic::classes::ServerLine;
use crate::logic::hud::{HudEvent, HudMessages, HudShow, ScreenFades};
use crate::logic::prop_damage::{Hit, Motion, PropExplosion};
use crate::logic::world::{Collision, Effect, EntId, LogicWorld, Player, SOLID_SKIN, SWEEP_EPS, Who};
use crate::console::{Console, ConsoleAppExt, Level};
use crate::core::{
    BaseVelocity, Damage, DamageKind, Damageable, EntityGravity, Explosion, Health, Hitgroup, Intent, LocalPlayer,
    MapBrush, MapBrushes, MapTerrain, MovementState, MovingSolid, RoundRestarts, SimSet, Team, Velocity,
};
use crate::map::breakables::{FallingPane, GibPiece, GlassImpact, GlassShatter, SpawnGibs};
use crate::map::entities::{engine_to_entity, entity_rotation, entity_to_engine, rotation_to_engine};
use crate::map::{LogicHidden, VisClusters};
use crate::map::{
    BreakProp, BrushPanes, EntityPart, LightStyles, MapBrushEntity, MapEntities, PlaySound, PropEntity, PropHome,
    PropIndex, PropLook, PropSequence, SoundControl, SoundKey, SoundLevel, SoundscapeSwitches, SoundscapeTouches,
    StartSound,
};

use super::*;

pub(super) fn apply_effects(world: &mut World, effects: Vec<Effect>, scale: f32) {
    let local = first::<With<LocalPlayer>>(world);
    for effect in effects {
        match effect {
            Effect::Damage { target, amount, crush } => {
                let point = world.get::<Transform>(target).map_or(Vec3::ZERO, |t| t.translation);
                world.write_message(Damage {
                    force: bevy::math::Vec3::ZERO,
                    target,
                    attacker: None,
                    amount: amount / 100.0,
                    point,
                    dir: Vec3::NEG_Y,
                    hitgroup: Hitgroup::Generic,
                    kind: if crush { DamageKind::Crush } else { DamageKind::Generic },
                    weapon: None,
                });
            }
            Effect::Hurt { target, amount, armour, kind } => {
                let point = world.get::<Transform>(target).map_or(Vec3::ZERO, |t| t.translation);
                world.write_message(crate::core::MapDamage {
                    damage: Damage {
                        force: Vec3::ZERO,
                        target,
                        attacker: None,
                        amount: amount / 100.0,
                        point,
                        dir: Vec3::NEG_Y,
                        hitgroup: Hitgroup::Generic,
                        kind,
                        weapon: None,
                    },
                    armour,
                });
            }
            Effect::Burn { target, amount } => {
                // Armour takes its part, as of a trigger_hurt's burn type
                // (measured on env_fire 2026-10-17: 7 a second took 3
                // health and about 2 armour; fire.md question 2).
                let point = world.get::<Transform>(target).map_or(Vec3::ZERO, |t| t.translation);
                world.write_message(crate::core::MapDamage {
                    damage: Damage {
                        force: Vec3::ZERO,
                        target,
                        attacker: None,
                        amount: amount / 100.0,
                        point,
                        dir: Vec3::NEG_Y,
                        hitgroup: Hitgroup::Generic,
                        kind: DamageKind::Burn,
                        weapon: None,
                    },
                    armour: true,
                });
            }
            Effect::FlameStart { id, target } => {
                let follow = match target {
                    Who::Player(p) => Some(p),
                    Who::Ent(e) => prop_node(world, e),
                };
                let at = follow.and_then(|f| world.get::<Transform>(f)).map(|t| t.translation);
                world.write_message(SoundControl::Start(StartSound {
                    key: sound_key(id),
                    entry: crate::logic::fire::BURNING_SOUND.into(),
                    at,
                    follow,
                    volume: None,
                    pitch: None,
                    level: None,
                    ..Default::default()
                }));
            }
            Effect::Gibs {
                set,
                glass,
                pieces,
                bounce,
            } => {
                let pieces = pieces
                    .into_iter()
                    .map(|g| {
                        GibPiece::thrown(
                            entity_to_engine(g.position, scale),
                            entity_to_engine(g.velocity, scale),
                            entity_to_engine(g.spin * std::f32::consts::PI / 180.0, 1.0),
                            g.life,
                        )
                    })
                    .collect();
                world.write_message(SpawnGibs {
                    set,
                    glass,
                    prop: false,
                    pieces,
                    shatters: None,
                    bounce: bounce.map(str::to_string),
                });
            }
            Effect::PropBreak { id, sound, explode } => prop_broke(world, id, sound, explode, scale),
            Effect::PropMotion { id, motion } => {
                let Some(node) = prop_node(world, id) else { continue };
                match motion {
                    Motion::Enable => {
                        crate::map::prop_physics::enable_motion(world, node);
                    }
                    Motion::Disable => crate::map::prop_physics::disable_motion(world, node),
                    Motion::Wake => crate::map::prop_physics::wake(world, node),
                    Motion::Sleep => crate::map::prop_physics::sleep(world, node),
                }
            }
            Effect::PaneShatter {
                at,
                normal,
                size,
                velocity,
                tile,
                shard,
            } => {
                world.write_message(GlassShatter {
                    at: entity_to_engine(at, scale),
                    normal: entity_to_engine(normal, 1.0),
                    size: size * scale,
                    velocity: entity_to_engine(velocity, scale),
                    tile,
                    shard: shard * scale,
                    piece: false,
                });
            }
            Effect::PaneFall {
                at,
                axes,
                size,
                body,
                spin,
            } => {
                world.write_message(FallingPane {
                    at: entity_to_engine(at, scale),
                    axes: axes.map(|a| entity_to_engine(a, 1.0)),
                    size: size * scale,
                    body,
                    spin: entity_to_engine(spin * std::f32::consts::PI / 180.0, 1.0),
                    shard: crate::logic::breakables::SMALL_SHARD * scale,
                });
            }
            Effect::Explosion {
                at,
                damage,
                radius,
                attacker,
                inflictor,
                flags,
                kind,
            } => {
                let attacker = match attacker {
                    Some(Who::Player(e)) => Some(e),
                    Some(Who::Ent(e)) => entity_node(world, e),
                    None => None,
                };
                let inflictor = entity_node(world, inflictor);
                world.write_message(Explosion {
                    origin: entity_to_engine(at, scale),
                    damage: damage / 100.0,
                    radius: radius * scale,
                    attacker,
                    inflictor,
                    sound: None,
                    weapon: None,
                    round_scorch: false,
                    through_walls: false,
                    bell_sigma: None,
                    // No weapon's ratio: armour as from a trigger_hurt,
                    // health half (measured on env_explosion 2026-10-17,
                    // game_entities.md question 7; the HE's is 1.475).
                    armor_ratio: Some(1.0),
                    flags,
                    kind,
                });
            }
            Effect::GlassImpact { at, normal } => {
                world.write_message(GlassImpact {
                    at: entity_to_engine(at, scale),
                    normal: entity_to_engine(normal, 1.0),
                });
            }
            Effect::Heal { target, amount } => {
                if let Some(mut h) = world.get_mut::<Health>(target)
                    && h.current > 0.0
                {
                    h.current = (h.current + amount / 100.0).min(h.max);
                }
            }
            Effect::SetHealth { target, health } => {
                if let Some(mut h) = world.get_mut::<Health>(target) {
                    h.current = (health / 100.0).clamp(0.0, h.max.max(health / 100.0));
                }
            }
            Effect::Sound {
                entry,
                at,
                volume,
                pitch,
            } => {
                let mut sound = PlaySound::at(entry, entity_to_engine(at, scale));
                sound.volume = volume;
                sound.pitch = pitch;
                logic_sound(world, sound);
            }
            Effect::AmbientStart {
                id,
                entry,
                at,
                source,
                volume,
                pitch,
                level,
            } => {
                let follow = source.and_then(|s| entity_node(world, s));
                world.write_message(SoundControl::Start(StartSound {
                    key: sound_key(id),
                    entry,
                    at: Some(entity_to_engine(at, scale)),
                    follow,
                    volume,
                    pitch,
                    level: level.map(SoundLevel::Db),
                    ..Default::default()
                }));
            }
            Effect::AmbientChange { id, volume, pitch } => {
                world.write_message(SoundControl::Change {
                    key: sound_key(id),
                    volume,
                    pitch,
                });
            }
            Effect::AmbientStop { id } => {
                world.write_message(SoundControl::Stop(sound_key(id)));
            }
            Effect::GameText { to, message } => hud_event(world, to, HudShow::Text(message)),
            Effect::Hud { to, what } => hud_event(world, to, what),
            Effect::SpeedMod {
                target,
                key,
                scale,
                flags,
            } => with_controls(world, target, |c| {
                // specs/source/game_entities.md 1.
                let take = crate::logic::game::speedmod_buttons(flags);
                if scale != 1.0 {
                    if flags & 1 != 0 {
                        c.weapon_hidden |= 1;
                    }
                    c.disabled.retain(|(k, _)| *k != key);
                    if take != 0 {
                        c.disabled.push((key, take));
                    }
                    if flags & 2 != 0 {
                        c.hud_hidden = true;
                    }
                } else {
                    if flags & 1 != 0 {
                        c.weapon_hidden &= !1;
                    }
                    c.disabled.retain(|(k, _)| *k != key);
                    if flags & 2 != 0 {
                        c.hud_hidden = false;
                    }
                }
                c.time_scale = scale;
            }),
            Effect::GameUi {
                target,
                on,
                freeze,
                hide_weapon,
            } => with_controls(world, target, |c| {
                if freeze {
                    c.at_controls = on;
                }
                if hide_weapon {
                    if on {
                        c.weapon_hidden |= 2;
                    } else {
                        c.weapon_hidden &= !2;
                    }
                }
            }),
            Effect::ViewControl { target, camera, freeze } => {
                let look = world.get::<Intent>(target).map(|i| (i.yaw, i.pitch));
                with_controls(world, target, |c| {
                    match camera {
                        Some(_) => {
                            c.invulnerable = true;
                            c.weapon_hidden |= 4;
                            if freeze {
                                c.frozen = look;
                            }
                        }
                        None => {
                            // Disable always unfreezes (viewcontrol 1.3).
                            c.invulnerable = false;
                            c.weapon_hidden &= !4;
                            c.frozen = None;
                        }
                    }
                });
                if camera.is_none()
                    && let Ok(mut e) = world.get_entity_mut(target)
                {
                    e.remove::<crate::core::MapView>();
                }
            }
            Effect::Score {
                target,
                points,
                team,
                allow_negative,
            } => {
                world.write_message(crate::core::ScoreChange {
                    target,
                    points,
                    team,
                    allow_negative,
                });
            }
            Effect::Spawned {
                id,
                source,
                origin,
                angles,
            } => spawn_copy(world, id, source, origin, angles, scale),
            Effect::Shake {
                at,
                amplitude,
                frequency,
                duration,
                radius,
            } => {
                world.write_message(crate::logic::MapShake {
                    at: entity_to_engine(at, scale),
                    amplitude: amplitude * scale,
                    frequency,
                    duration,
                    radius: radius * scale,
                });
            }
            Effect::EntityInput {
                id,
                map_index,
                input,
                value,
                activator,
            } => {
                world.write_message(crate::map::entities::MapEntityInput {
                    key: entity_key(id),
                    map_index,
                    input,
                    value,
                    activator,
                });
            }
            Effect::EntitySpawned {
                id,
                map_index,
                class,
                keyvalues,
                origin,
                angles,
            } => {
                world.write_message(crate::map::entities::MapEntitySpawned {
                    key: entity_key(id),
                    map_index,
                    class,
                    keyvalues,
                    origin,
                    angles,
                });
            }
            Effect::EntityRemoved { id } => {
                world.write_message(crate::map::entities::MapEntityRemoved { key: entity_key(id) });
            }
            Effect::PropRespawn { id } => prop_respawn(world, id),
            Effect::Spark { at, dir, magnitude } => {
                world.write_message(crate::map::SparkBurst {
                    at: entity_to_engine(at, scale),
                    dir: entity_to_engine(dir, 1.0),
                    magnitude,
                });
            }
            Effect::Tesla { entity } => {
                world.write_message(crate::map::TeslaSpark { entity });
            }
            Effect::BeamStrike { entity, start, end, life } => {
                world.write_message(crate::map::BeamStrike {
                    entity,
                    start: entity_to_engine(start, scale),
                    end: entity_to_engine(end, scale),
                    life,
                });
            }
            Effect::MuzzleFlash { entity } => {
                world.write_message(crate::map::MuzzleFlashFire { entity });
            }
            Effect::ViewPunch { player, angles } => {
                // Source's pitch is down positive; ours up (radians).
                world.write_message(crate::map::ViewKick {
                    player,
                    angles: Vec3::new(-angles.x, angles.y, angles.z) * std::f32::consts::PI / 180.0,
                });
            }
            Effect::BodyVelocity { id, velocity } => {
                if let Some(node) = entity_node(world, id)
                    && let Some(mut v) = world.get_mut::<LinearVelocity>(node)
                {
                    v.0 += entity_to_engine(velocity, scale);
                    crate::map::prop_physics::wake(world, node);
                }
            }
            Effect::BodyTeleport { id, origin, angles } => {
                let now = world.get_resource::<crate::core::SimClock>().map_or(0.0, |c| c.now);
                let Some(node) = entity_node(world, id) else { continue };
                let Ok(mut e) = world.get_entity_mut(node) else {
                    continue;
                };
                // Without a landmark (angles given) its origin goes to the
                // destination (triggers.md, trigger_teleport 4: dm_lockdown's
                // plug seats so); with one, its centre goes to `origin`, and
                // its origin by the same amount.
                let centre = e
                    .get::<ColliderAabb>()
                    .filter(|_| angles.is_none())
                    .map(|a| (a.min + a.max) / 2.0)
                    .or_else(|| e.get::<Transform>().map(|t| t.translation))
                    .unwrap_or_default();
                let delta = entity_to_engine(origin, scale) - centre;
                let rotation = angles.map(|a| rotation_to_engine(entity_rotation(a)));
                if let Some(mut t) = e.get_mut::<Transform>() {
                    t.translation += delta;
                    if let Some(r) = rotation {
                        t.rotation = r;
                    }
                }
                if let Some(mut p) = e.get_mut::<Position>() {
                    p.0 += delta;
                }
                if let Some(r) = rotation
                    && let Some(mut rot) = e.get_mut::<Rotation>()
                {
                    *rot = Rotation::from(r);
                }
                if let Some(t) = e.get::<Transform>().copied() {
                    e.insert(crate::map::prop_physics::JustTeleported { position: t.translation, rotation: t.rotation, at: now });
                }
            }
            Effect::DamageFilter { target, filter } => {
                let Ok(mut e) = world.get_entity_mut(target) else { continue };
                match filter {
                    Some((bits, negated)) => {
                        // A damage passes when its Source type bits equal
                        // the filter's (entity_io.md, filter_damage_type).
                        let blocked = DAMAGE_KINDS
                            .iter()
                            .filter(|(_, b)| (*b == bits) == negated)
                            .map(|(k, _)| *k)
                            .collect();
                        e.insert(crate::core::DamageFilter { blocked });
                    }
                    None => {
                        e.remove::<crate::core::DamageFilter>();
                    }
                }
            }
            Effect::IgnoreFallDamage { target, seconds, reset } => {
                let now = world.get_resource::<crate::core::SimClock>().map_or(0.0, |c| c.now);
                if let Ok(mut e) = world.get_entity_mut(target) {
                    e.insert(crate::core::IgnoreFallDamage { until: now + f64::from(seconds), reset });
                }
            }
            Effect::Equip { target, items, strip } => {
                world.write_message(crate::core::Equip { target, items, strip });
            }
            Effect::ServerCommand(line) => server_command(world, &line),
            Effect::ClientCommand { player, command } => {
                if Some(player) == local
                    && let Some(mut c) = world.get_resource_mut::<Console>()
                {
                    c.submit(command);
                }
            }
        }
    }
}

/// Thrusters and keep-uprights that are on, for the physics
/// (`map::controllers::BodyControllers`, engine space, SI units).
pub(super) fn sync_controls(world: &mut World, logic: &Logic) {
    use crate::logic::physics::{
        ControlKind as L, SF_THRUST_FORCE, SF_THRUST_IGNORE_MASS, SF_THRUST_IGNORE_POS, SF_THRUST_LOCAL,
        SF_THRUST_TORQUE,
    };
    use crate::map::controllers::{BodyController, BodyControllers, ControlKind};
    let s = logic.scale;
    let list: Vec<BodyController> = logic
        .world
        .controls()
        .into_iter()
        .filter_map(|c| {
            let body = logic
                .props
                .iter()
                .chain(&logic.nodes)
                .find(|(id, _)| *id == c.body)
                .map(|(_, n)| *n)?;
            let kind = match c.kind {
                L::Thrust {
                    force,
                    force_local,
                    offset,
                    flags,
                    scale,
                    serial,
                } => ControlKind::Thrust {
                    force: entity_to_engine(force, s),
                    force_local: entity_to_engine(force_local, s),
                    offset: entity_to_engine(offset, s),
                    local: flags & SF_THRUST_LOCAL != 0,
                    linear: flags & SF_THRUST_FORCE != 0,
                    angular: flags & SF_THRUST_TORQUE != 0,
                    ignore_mass: flags & SF_THRUST_IGNORE_MASS != 0,
                    ignore_pos: flags & SF_THRUST_IGNORE_POS != 0,
                    scale,
                    serial,
                },
                L::Upright { goal, limit } => ControlKind::Upright {
                    goal: entity_to_engine(goal, 1.0).normalize_or_zero(),
                    limit: limit.to_radians(),
                },
                L::Motor {
                    axis,
                    speed,
                    spinup,
                    inertia_factor,
                    add_accel,
                } => ControlKind::Motor {
                    axis: entity_to_engine(axis, 1.0).normalize_or_zero(),
                    speed: speed.to_radians(),
                    spinup,
                    inertia_factor,
                    add_accel: add_accel.to_radians(),
                },
                L::Spin { axis, speed } => ControlKind::Spin {
                    axis: entity_to_engine(axis, 1.0).normalize_or_zero(),
                    speed: speed.to_radians(),
                },
                L::Slide { axis, speed } => ControlKind::Slide {
                    axis: entity_to_engine(axis, 1.0).normalize_or_zero(),
                    speed: speed * s,
                },
            };
            Some(BodyController {
                key: (c.id.generation as u64) << 32 | c.id.index as u64,
                body,
                kind,
            })
        })
        .collect();
    if world.get_resource::<BodyControllers>().is_some_and(|b| b.0 != list) {
        world.insert_resource(BodyControllers(list));
    }
    // Players parented to an entity ride it (`core::MapControls::parented`).
    let players: Vec<(Entity, bool)> =
        cached_query::<(Entity, Option<&crate::core::MapControls>), With<crate::core::Intent>, _>(world, |q, world| {
            q.iter(world)
                .filter_map(|(e, c)| {
                    let want = logic.world.player_parent(e).is_some();
                    (c.map_or(false, |c| c.parented) != want).then_some((e, want))
                })
                .collect()
        });
    for (e, want) in players {
        with_controls(world, e, |c| c.parented = want);
    }
    // Constraints and motors' hinges.
    use crate::logic::physics::JointKind as J;
    use crate::map::controllers::{BodyJoint, BodyJoints, JointKind};
    let node = |id: EntId| {
        logic
            .props
            .iter()
            .chain(&logic.nodes)
            .find(|(p, _)| *p == id)
            .map(|(_, n)| *n)
    };
    // The map's gravity (m/s²) turns break limits in pounds into forces.
    let gravity = world
        .get_resource::<avian3d::prelude::Gravity>()
        .map_or(9.81, |g| g.0.length());
    let joints: Vec<BodyJoint> = logic
        .world
        .joints()
        .into_iter()
        .filter_map(|j| {
            let body2 = node(j.body2)?;
            let body1 = match j.body1 {
                Some(b) => Some(node(b)?),
                None => None,
            };
            let dir = |v: Vec3| entity_to_engine(v, 1.0).normalize_or_zero();
            Some(BodyJoint {
                key: (j.id.generation as u64) << 32 | j.id.index as u64,
                kind: match j.kind {
                    J::Fixed => JointKind::Fixed,
                    J::Ball => JointKind::Ball,
                    J::Hinge { axis } => JointKind::Hinge { axis: dir(axis) },
                    J::Slide { axis } => JointKind::Slide { axis: dir(axis) },
                    J::Length { min, max, other } => JointKind::Length {
                        min: min * s,
                        max: max * s,
                        other: entity_to_engine(other, s),
                    },
                },
                body1,
                body2,
                anchor: entity_to_engine(j.anchor, s),
                on: j.on,
                no_collide: j.no_collide,
                // Pounds as a weight in the map's gravity (kg = lb / 2.2),
                // a hanging body's weight breaking it past 1/6.3 of that
                // (CS:S, measured 2026-10-17: a ball socket holding 15, 30
                // and 60 kg crates at rest broke below a forcelimit of about
                // 13.8 lb a kg; physics_constraints.md question 1; a weld
                // the same: 400 broke a 30 kg crate's, 420 held, whether at
                // its centre or 40 units off, 2026-10-18). The torque limit
                // breaks nothing: a weld 40 units off a 30 kg crate's centre
                // held it at torquelimit 1 (measured 2026-10-18; moving
                // loads unmeasured).
                force_limit: j.force_limit / 2.2 * gravity / HANGING_LOAD,
                torque_limit: 0.0,
            })
        })
        .collect();
    if world.get_resource::<BodyJoints>().is_some_and(|b| b.0 != joints) {
        world.insert_resource(BodyJoints(joints));
    }
}

/// How many times its weight a body hanging at rest asks of a constraint's
/// force limit (forcelimit / 2.2 kg): CS:S's sockets broke below about 6.3
/// (measured on 15, 30 and 60 kg crates, physics_constraints.md question 1).
pub(super) const HANGING_LOAD: f32 = 6.3;

/// Players viewing through a point_viewcontrol see from it
/// (`core::MapView`: a camera's pose, looking down -Z).
pub(super) fn sync_views(world: &mut World, logic: &Logic) {
    // What monitors show (`map::monitor`).
    let monitor = crate::map::MonitorCamera(logic.world.monitor_camera().map(|(origin, angles, fov)| {
        (
            entity_to_engine(origin, logic.scale),
            rotation_to_engine(entity_rotation(angles)) * Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
            fov,
        )
    }), logic.world.monitor_fog().map(|f| crate::map::MonitorFog {
        start: f.start * logic.scale,
        end: f.end * logic.scale,
        ..f
    }));
    if world.get_resource::<crate::map::MonitorCamera>() != Some(&monitor) {
        world.insert_resource(monitor);
    }
    for (p, origin, angles) in logic.world.camera_views() {
        let view = crate::core::MapView {
            origin: entity_to_engine(origin, logic.scale),
            rotation: rotation_to_engine(entity_rotation(angles)) * Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
        };
        if let Ok(mut e) = world.get_entity_mut(p)
            && e.get::<crate::core::MapView>() != Some(&view)
        {
            e.insert(view);
        }
    }
}

/// A template's copy of a brush entity or prop: the map entity's node
/// copied to where the copy stands, paired with the new logic entity.
pub(super) fn spawn_copy(world: &mut World, id: EntId, source: usize, origin: Vec3, angles: Vec3, scale: f32) {
    use crate::map::copies::{MapCopy, copy_node};
    let pose = Transform::from_translation(entity_to_engine(origin, scale))
        .with_rotation(rotation_to_engine(entity_rotation(angles)));
    let brush = world
        .query_filtered::<(Entity, &MapBrushEntity), Without<MapCopy>>()
        .iter(world)
        .find(|(_, b)| b.0 == source)
        .map(|(e, _)| e);
    if let Some(node) = brush {
        if let Some(copy) = copy_node(world, node, pose) {
            // A physics brush (func_physbox) is a prop too, as the map's
            // own are (`attach_nodes`, `attach_props`): its copy of a
            // template member, which was taken out of the map (hidden, its
            // body off), is shown with its body on.
            let physics = world.get::<PropEntity>(copy).is_some();
            if physics {
                set_prop_shown(world, copy, true, true, true);
            }
            if let Some(mut logic) = world.get_resource_mut::<Logic>() {
                logic.nodes.push((id, copy));
                if physics {
                    logic.props.push((id, copy));
                }
            }
            copy_takes_damage(world, id, copy);
        }
        return;
    }
    let prop = world
        .query_filtered::<(Entity, &PropEntity), Without<MapCopy>>()
        .iter(world)
        .find(|(_, p)| p.0 == source)
        .map(|(e, _)| e);
    if let Some(node) = prop
        && let Some(copy) = copy_node(world, node, pose)
    {
        set_prop_shown(world, copy, true, true, true);
        if let Some(mut logic) = world.get_resource_mut::<Logic>() {
            logic.props.push((id, copy));
        }
        copy_takes_damage(world, id, copy);
    }
}

/// A copy of a breakable or a prop takes weapons' damage as the map's own
/// do (`attach_nodes`, `attach_props`): the template member it was cloned
/// from was taken out of the map and so took none (shots passed through
/// spawned items and balls, and didn't wake them).
pub(super) fn copy_takes_damage(world: &mut World, id: EntId, copy: Entity) {
    let damageable = world.get_resource::<Logic>().is_some_and(|l| match l.world.get(id).map(|e| &e.class) {
        Some(crate::logic::classes::Class::Breakable(_)) => true,
        Some(crate::logic::classes::Class::Prop(_)) => l.world.prop_states().iter().any(|s| s.id == id && s.damageable),
        _ => false,
    });
    if damageable && let Ok(mut e) = world.get_entity_mut(copy) {
        e.insert(Damageable);
    }
}

/// Something for players' HUDs: shown here when it is for the local
/// player (or everyone), and handed on as a `HudEvent` (a network server
/// sends it to the client it is for).
pub(super) fn hud_event(world: &mut World, to: Option<Entity>, what: HudShow) {
    crate::logic::hud::send_hud(world, to, what);
}

/// Change a character's `core::MapControls` (added when missing).
pub(super) fn with_controls(world: &mut World, target: Entity, f: impl FnOnce(&mut crate::core::MapControls)) {
    let Ok(mut e) = world.get_entity_mut(target) else { return };
    let mut c = e.get::<crate::core::MapControls>().cloned().unwrap_or_default();
    f(&mut c);
    if e.get::<crate::core::MapControls>() != Some(&c) {
        e.insert(c);
    }
}

/// Our damage kinds as Source damage-type bits (DMG_GENERIC 0, CRUSH 1,
/// BULLET 2, SLASH 4, BURN 8, FALL 32, BLAST 64, DROWN 16384; the public SDK's
/// names). Melee as slash and bullets without their extra flag bits are
/// our reading; fall, the one maps filter, is exact.
pub(super) const DAMAGE_KINDS: [(DamageKind, u32); 11] = [
    (DamageKind::Generic, 0),
    (DamageKind::Crush, 1),
    (DamageKind::Bullet, 2),
    (DamageKind::Melee, 4),
    (DamageKind::Burn, 8),
    (DamageKind::Fall, 32),
    (DamageKind::Blast, 64),
    (DamageKind::BlastSurface, 64 | 134_217_728),
    (DamageKind::Drown, 16384),
    (DamageKind::Poison, 131072),
    (DamageKind::Radiation, 262144),
];

/// The long-lived sound of a logic entity.
pub(super) fn sound_key(id: EntId) -> SoundKey {
    SoundKey(1 << 63 | (id.generation as u64) << 32 | id.index as u64)
}

/// The ECS node standing for a logic entity, if it has one: a mover's
/// node or a prop the entity placed.
pub(super) fn entity_node(world: &mut World, id: EntId) -> Option<Entity> {
    let logic = world.get_resource::<Logic>()?;
    if let Some((_, node)) = logic.nodes.iter().chain(&logic.props).find(|(m, _)| *m == id) {
        return Some(*node);
    }
    let index = logic.world.get(id)?.map_index?;
    world
        .query::<(Entity, &PropEntity)>()
        .iter(world)
        .find(|(_, p)| p.0 == index)
        .map(|(e, _)| e)
}

/// Game commands a map may run as if they were settings: cvars in Source,
/// console commands here. They change nothing to put back.
pub(super) const SETTING_COMMANDS: &[&str] = &["mp_restartgame"];

/// A server command that passed `classes::check_server_command`: `say`
/// goes to chat (`MapSay`) and prints; a setting our console has as a cvar remembers its value from
/// before the map first changed it (put back when the map unloads,
/// `restore_settings`) and runs through the console; anything else is
/// logged and ignored. Each outcome prints a console line.
pub(super) fn server_command(world: &mut World, line: &ServerLine) {
    if let ServerLine::Say(text) = line {
        info!("point_servercommand: say {text}");
        if let Some(mut console) = world.get_resource_mut::<Console>() {
            console.info(format!("point_servercommand: say {text}"));
        }
        world.write_message(crate::logic::MapSay(text.clone()));
        return;
    }
    let Some(mut console) = world.get_resource_mut::<Console>() else {
        return;
    };
    let (name, value) = match line {
        ServerLine::Say(_) => return,
        ServerLine::Set { name, value } => (name.clone(), value.clone()),
    };
    let run = format!("{name} {}", crate::console::quote(&value));
    let Some(cvar) = console.cvar(&name).cloned() else {
        if SETTING_COMMANDS.contains(&name.as_str()) && console.command(&name).is_some() {
            console.info(format!("point_servercommand: {name} {value}"));
            console.submit(run);
        } else {
            let what = if console.command(&name).is_some() { "a command, not a setting" } else { "no such setting" };
            console.print(Level::Warn, format!("point_servercommand: ignored '{name} {value}' ({what})"));
            warn!("point_servercommand: ignored '{name} {value}' ({what})");
        }
        return;
    };
    console.submit(run);
    // The line runs from the queue later, so this is still the value
    // before the map's.
    let old = (cvar.get)(world).unwrap_or_default();
    let mut first = false;
    if let Some(mut l) = world.get_resource_mut::<Logic>()
        && !l.restore.iter().any(|(n, _)| *n == name)
    {
        l.restore.push((name.clone(), old.clone()));
        first = true;
    }
    let was = if first { format!(" (was {old})") } else { String::new() };
    info!("point_servercommand: {name} {value}{was}");
    world
        .resource_mut::<Console>()
        .info(format!("point_servercommand: {name} {value}{was}"));
}

/// Run one logic phase with the players and the static world.
pub(super) fn run_phase(world: &mut World, phase: impl FnOnce(&mut LogicWorld, &dyn Collision)) {
    let Some(mut logic) = world.remove_resource::<Logic>() else {
        return;
    };
    // The tick can change after the map loads (a game sets its own).
    logic.world.dt = world.resource::<Time<Fixed>>().timestep().as_secs_f32();
    if logic.world.collision.is_none() {
        logic.world.collision = static_collision(world, logic.scale);
    }
    prop_bounds(world, &mut logic);
    let players = snapshot(world, &logic);
    logic.world.players = players.clone();
    // After the players are in: entities parented to a player follow it.
    follow_anchors(world, &mut logic);
    {
        let col = WorldCollision {
            brushes: world.get_resource::<MapBrushes>(),
            terrain: world.get_resource::<MapTerrain>(),
            scale: logic.scale,
        };
        let _span = info_span!("logic: phase").entered();
        phase(&mut logic.world, &col);
    }
    // Spans for traces (docs/performance.md): this exclusive system's parts.
    let span = info_span!("logic: sync to the ECS").entered();
    sync_fires(world, &logic);
    let after = std::mem::take(&mut logic.world.players);
    write_back(world, logic.scale, &players, &after);
    info_span!("logic: sync_movers").in_scope(|| sync_movers(world, &mut logic));
    sync_props(world, &mut logic);
    sync_visuals(world, &logic);
    sync_soundscapes(world, &logic);
    sync_views(world, &logic);
    sync_controls(world, &logic);
    sync_removed_weapons(world, &logic);
    sync_physics_clips(world, &logic);
    drop(span);
    let effects = std::mem::take(&mut logic.world.effects);
    for line in logic.world.log.drain(..) {
        if line.contains("refused") {
            warn!("{line}");
            // What a map's server and client commands were refused shows
            // in the console too.
            if line.starts_with("point_")
                && let Some(mut c) = world.get_resource_mut::<Console>()
            {
                c.print(Level::Warn, line);
            }
        } else {
            debug!("{line}");
        }
    }
    let scale = logic.scale;
    world.insert_resource(logic);
    apply_effects(world, effects, scale);
}

/// The anchor entities (placed weapons) where the ECS has them
/// (`map::entities::EntityAnchors`), and their children with them.
pub(super) fn follow_anchors(world: &World, logic: &mut Logic) {
    if logic.world.follows.is_empty() && logic.world.player_follows.is_empty() {
        return;
    }
    set_anchor_poses(world, logic);
    logic.world.follow_anchors();
}

/// Brushes and entities riding physics bodies, again after the physics
/// step (they'd trail their body by a tick): their logic place, then
/// their nodes (drawn meshes, collider, `MovingSolid`).
pub(super) fn follow_bodies(world: &mut World) {
    if !world.get_resource::<Logic>().is_some_and(|l| l.world.has_body_riders()) {
        return;
    }
    let mut logic = world.remove_resource::<Logic>().unwrap();
    set_anchor_poses(world, &mut logic);
    logic.world.follow_bodies();
    sync_movers(world, &mut logic);
    world.insert_resource(logic);
}

/// The anchors' and physics bodies' poses into the logic (`set_anchor`).
pub(super) fn set_anchor_poses(world: &World, logic: &mut Logic) {
    let scale = logic.scale;
    let entity_pose = |t: &Transform| {
        let rotation = Quat::from_xyzw(t.rotation.x, -t.rotation.z, t.rotation.y, t.rotation.w);
        (engine_to_entity(t.translation, scale), crate::logic::anchors::angles_of(rotation))
    };
    let mut poses: Vec<(EntId, Vec3, Vec3)> = Vec::new();
    if let Some(anchors) = world.get_resource::<crate::map::entities::EntityAnchors>() {
        for (index, t) in &anchors.0 {
            if let Some(id) = logic.world.by_map_index(*index) {
                let (o, a) = entity_pose(t);
                poses.push((id, o, a));
            }
        }
    }
    // Physics brushes and props: where their bodies are (a moving body:
    // maps rename them, AddOutput classname, so the class can't tell).
    for (id, node) in &logic.props {
        let body = world.get::<RigidBody>(*node).is_some_and(|b| !b.is_static());
        if (body
            || logic
                .world
                .get(*id)
                .is_some_and(|e| crate::logic::anchors::is_physics_body(&e.classname)))
            && let Some(t) = world.get::<Transform>(*node)
        {
            let (o, a) = entity_pose(t);
            poses.push((*id, o, a));
        }
    }
    for (id, o, a) in poses {
        logic.world.set_anchor(id, o, a);
    }
}

/// Where each logic prop's body is now (its collider's world box), for
/// fire (flame size and place, damage boxes).
pub(super) fn prop_bounds(world: &mut World, logic: &mut Logic) {
    let scale = logic.scale;
    let props = std::mem::take(&mut logic.props);
    for &(id, node) in &props {
        let aabb = std::iter::once(node)
            .chain(world.get::<Children>(node).map(|c| c.to_vec()).unwrap_or_default())
            .find_map(|e| world.get::<ColliderAabb>(e).copied());
        if let Some(a) = aabb {
            let (lo, hi) = (engine_to_entity(a.min, scale), engine_to_entity(a.max, scale));
            logic.world.set_prop_bounds(id, (lo.min(hi), lo.max(hi)));
        }
        if let Some(v) = world.get::<AngularVelocity>(node) {
            // rad/s, engine space -> deg/s, entity space.
            logic.world.set_prop_spin(id, engine_to_entity(v.0, 1.0) * 180.0 / std::f32::consts::PI);
        }
    }
    logic.props = props;
}

/// Lit fires and burning entities for the game to draw
/// (`map::fire::MapFires`).
pub(super) fn sync_fires(world: &mut World, logic: &Logic) {
    use crate::map::fire::{FireLook, FlameLook, MapFires};
    let scale = logic.scale;
    let key = |id: EntId, serial: u32| (serial as u64) << 48 | (id.generation as u64 & 0xffff) << 32 | id.index as u64;
    let fires: Vec<FireLook> = logic
        .world
        .fire_looks()
        .into_iter()
        .filter(|f| !f.plasma)
        .map(|f| FireLook {
            key: key(f.id, f.serial),
            at: entity_to_engine(f.at, scale),
            size: f.size,
            smokeless: f.smokeless,
        })
        .collect();
    let flames: Vec<FlameLook> = logic
        .world
        .flame_looks()
        .into_iter()
        .filter_map(|f| {
            let (follow, lo, hi) = match f.target {
                Who::Player(p) => {
                    let pl = logic.world.player(p)?;
                    (Some(p), pl.origin + pl.mins, pl.origin + pl.maxs)
                }
                Who::Ent(e) => {
                    let node = logic.props.iter().find(|(id, _)| *id == e).map(|(_, n)| *n);
                    let (lo, hi) = logic.world.prop(e)?.bounds.unwrap_or_else(|| {
                        let o = logic.world.get(e).map_or(Vec3::ZERO, |x| x.origin);
                        (o - Vec3::splat(8.0), o + Vec3::splat(8.0))
                    });
                    (node, lo, hi)
                }
            };
            let (a, b) = (entity_to_engine(lo, scale), entity_to_engine(hi, scale));
            Some(FlameLook {
                key: key(f.id, 0),
                follow,
                min: a.min(b),
                max: a.max(b),
            })
        })
        .collect();
    let want = MapFires { fires, flames };
    if world.get_resource::<MapFires>() != Some(&want) {
        world.insert_resource(want);
    }
}

