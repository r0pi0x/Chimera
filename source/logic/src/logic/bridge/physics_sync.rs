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

/// `chimera_logic_record 1`: the logic world records the outputs it fires
/// and the inputs it delivers (the debug UI's Logic tab lists them),
/// keeping the latest `RECORD_KEEP`.
#[derive(Resource, Default)]
pub(super) struct LogicRecord(pub(super) u8);

pub(super) const RECORD_KEEP: usize = 256;

pub(super) fn apply_record(rec: Res<LogicRecord>, logic: Option<ResMut<Logic>>, mut last: Local<u8>) {
    let Some(mut logic) = logic else { return };
    // Turned off: stop (tests that record by hand leave the cvar at 0).
    if rec.0 != *last {
        *last = rec.0;
        logic.world.record = rec.0 != 0;
    }
    if rec.0 == 0 {
        return;
    }
    if !logic.world.record {
        logic.world.record = true;
    }
    for len in [logic.world.fired.len(), logic.world.deliveries.len()] {
        if len > RECORD_KEEP * 2 {
            let w = &mut logic.world;
            let fired = w.fired.len().saturating_sub(RECORD_KEEP);
            w.fired.drain(..fired);
            let delivered = w.deliveries.len().saturating_sub(RECORD_KEEP);
            w.deliveries.drain(..delivered);
            break;
        }
    }
}

pub(super) fn pre(world: &mut World) {
    run_phase(world, |w, col| {
        w.break_orphan_joints();
        w.run_thinks();
        w.move_cameras();
        w.step_movers(col);
        w.player_uses(col);
        w.push_conveyors();
    });
}

pub(super) fn post(
    world: &mut World,
    mut awake: Local<MessageCursor<crate::map::prop_physics::PropAwakened>>,
    mut broken: Local<MessageCursor<crate::map::controllers::JointBroke>>,
    mut animated: Local<MessageCursor<crate::map::PropAnimationDone>>,
) {
    // Animated props whose sequence ended (map::props::animate_props).
    let ended: Vec<(Entity, u32)> = match world.get_resource::<Messages<crate::map::PropAnimationDone>>() {
        Some(m) => animated.read(m).map(|d| (d.node, d.serial)).collect(),
        None => Vec::new(),
    };
    let ended: Vec<(EntId, u32)> = match world.get_resource::<Logic>() {
        Some(l) => ended
            .into_iter()
            .filter_map(|(n, s)| l.props.iter().find(|(_, p)| *p == n).map(|(id, _)| (*id, s)))
            .collect(),
        None => Vec::new(),
    };
    // Props that started asleep and woke (map::prop_physics).
    let woke: Vec<Entity> = match world.get_resource::<Messages<crate::map::prop_physics::PropAwakened>>() {
        Some(m) => awake.read(m).map(|a| a.0).collect(),
        None => Vec::new(),
    };
    let woke: Vec<EntId> = match world.get_resource::<Logic>() {
        Some(l) => woke
            .into_iter()
            .filter_map(|n| l.props.iter().find(|(_, p)| *p == n).map(|(id, _)| *id))
            .collect(),
        None => Vec::new(),
    };
    // Constraints that passed their break limits (map::controllers).
    let broke: Vec<EntId> = match world.get_resource::<Messages<crate::map::controllers::JointBroke>>() {
        Some(m) => broken
            .read(m)
            .map(|b| EntId {
                index: (b.0 & 0xffff_ffff) as u32,
                generation: (b.0 >> 32) as u32,
            })
            .collect(),
        None => Vec::new(),
    };
    run_phase(world, |w, col| {
        for id in woke {
            w.prop_awakened(id);
        }
        for (id, serial) in ended {
            w.prop_animation_done(id, serial);
        }
        for id in broke {
            w.deliver(Who::Ent(id), "Break", crate::logic::Value::Void, None, Some(Who::Ent(id)));
        }
        w.touch_triggers(col);
        w.touch_breakables();
        w.pressure_props();
        w.untouch();
        w.service_queue();
        w.end_frame();
    });
}

/// Damage dealt to mover nodes (breakables) and prop nodes this tick, into
/// the logic (with the push that came with it, and its attacker: a
/// player, or a prop that hit it).
pub(super) fn damage(world: &mut World, mut cursor: Local<MessageCursor<Damage>>) {
    let hits: Vec<Damage> = match world.get_resource::<Messages<Damage>>() {
        Some(m) => cursor.read(m).cloned().collect(),
        None => return,
    };
    let Some(logic) = world.get_resource::<Logic>() else {
        return;
    };
    let scale = logic.scale;
    let node_id = |e: Entity| {
        logic
            .nodes
            .iter()
            .chain(&logic.props)
            .find(|(_, n)| *n == e)
            .map(|(id, _)| *id)
    };
    let targeted: Vec<(EntId, Damage, Option<Who>)> = hits
        .into_iter()
        .filter_map(|d| {
            let id = node_id(d.target)?;
            let by = d.attacker.and_then(node_id).map(Who::Ent);
            Some((id, d, by))
        })
        .collect();
    if targeted.is_empty() {
        return;
    }
    run_phase(world, |w, _| {
        for (id, d, by) in targeted {
            let attacker = d.attacker.filter(|a| w.player(*a).is_some()).map(Who::Player).or(by);
            let dir = Vec3::new(d.dir.x, -d.dir.z, d.dir.y);
            let point = engine_to_entity(d.point, scale);
            let hit = Hit {
                amount: d.amount * 100.0,
                kind: d.kind,
                attacker,
                point,
                dir,
                force: d.force.length() / scale,
                direct: false,
            };
            if !w.prop_hit(id, hit) {
                w.damage(id, hit.amount, d.kind, attacker, point, dir);
            }
        }
    });
}

/// The node of a logic prop.
pub(super) fn prop_node(world: &World, id: EntId) -> Option<Entity> {
    let logic = world.get_resource::<Logic>()?;
    logic.props.iter().find(|(p, _)| *p == id).map(|(_, n)| *n)
}

/// Where a prop node is (its body's pose when it has one) and the centre
/// of its bounds.
pub(super) fn prop_pose(world: &World, node: Entity) -> (Vec3, Quat, Vec3) {
    let (pos, rot) = match (world.get::<Position>(node), world.get::<Rotation>(node)) {
        (Some(p), Some(r)) => (p.0, r.0),
        _ => world
            .get::<GlobalTransform>(node)
            .map_or((Vec3::ZERO, Quat::IDENTITY), |g| {
                let (_, r, t) = g.to_scale_rotation_translation();
                (t, r)
            }),
    };
    let centre = std::iter::once(node)
        .chain(world.get::<Children>(node).map(|c| c.to_vec()).unwrap_or_default())
        .find_map(|e| world.get::<ColliderAabb>(e))
        .map_or(pos, |a| a.center());
    (pos, rot, centre)
}

/// A prop broke: its break sound where it is, its explosion from its
/// centre, its pieces from its pose and velocity.
pub(super) fn prop_broke(world: &mut World, id: EntId, sound: Option<String>, explode: Option<PropExplosion>, scale: f32) {
    let Some(node) = prop_node(world, id) else { return };
    let (pos, rot, centre) = prop_pose(world, node);
    if let Some(s) = sound {
        logic_sound(world, PlaySound::at(s, pos));
    }
    if let Some(x) = explode {
        let attacker = match x.attacker {
            Some(Who::Player(e)) => Some(e),
            Some(Who::Ent(e)) => prop_node(world, e),
            None => None,
        };
        // No player hurt it last: its influencer, while the influence is
        // fresh (the game's window).
        let attacker = match (x.attacker, world.get_resource::<BlastInfluenceWindow>().copied()) {
            (Some(Who::Player(_)), _) | (_, None) => attacker,
            (_, Some(window)) => {
                let now = world.get_resource::<crate::core::SimClock>().map_or(0.0, |c| c.now);
                world
                    .get::<crate::map::held::PhysicsInfluence>(node)
                    .filter(|i| now - i.at <= window.0 + 1e-6)
                    .map(|i| i.by)
                    .or(attacker)
            }
        };
        world.write_message(Explosion {
            origin: centre,
            damage: x.damage / 100.0,
            radius: x.radius * scale,
            attacker,
            inflictor: Some(node),
            sound: x.sound,
            weapon: world.get_resource::<PropBlastWeapon>().map(|w| w.0),
            round_scorch: false,
            through_walls: false,
            bell_sigma: None,
            armor_ratio: None,
            // Made by code: never sparks; both of a prop's explosion paths
            // ask for "surface only", its 1-damage fallback doesn't
            // (prop_damage.md 7.5, hl2dm/rules.md spawn grace).
            flags: crate::core::BlastFlags(
                crate::core::BlastFlags::NO_SPARKS | if x.silent { crate::core::BlastFlags::NO_SOUND } else { 0 },
            ),
            kind: if x.surface { crate::core::DamageKind::BlastSurface } else { crate::core::DamageKind::Blast },
        });
    }
    if let Some(index) = world.get::<PropIndex>(node).map(|p| p.0) {
        let velocity = world.get::<LinearVelocity>(node).map_or(Vec3::ZERO, |v| v.0);
        let spin = world.get::<AngularVelocity>(node).map_or(Vec3::ZERO, |v| v.0);
        let skin = world.get::<PropLook>(node).map(|l| l.skin);
        world.write_message(BreakProp {
            prop: index,
            transform: Transform::from_translation(pos).with_rotation(rot),
            velocity,
            spin,
            skin,
        });
    }
    // A respawnable prop's body waits at the spot it started at.
    let respawns = world
        .get_resource::<Logic>()
        .is_some_and(|l| l.world.prop(id).is_some_and(|p| p.respawn.is_some()));
    if respawns && let Some(home) = world.get::<PropHome>(node).map(|h| h.0) {
        let mut e = world.entity_mut(node);
        if let Some(mut t) = e.get_mut::<Transform>() {
            *t = home;
        }
        if let Some(mut p) = e.get_mut::<Position>() {
            p.0 = home.translation;
        }
        if let Some(mut r) = e.get_mut::<Rotation>() {
            *r = Rotation::from(home.rotation);
        }
        if let Some(mut v) = e.get_mut::<LinearVelocity>() {
            v.0 = Vec3::ZERO;
        }
        if let Some(mut v) = e.get_mut::<AngularVelocity>() {
            v.0 = Vec3::ZERO;
        }
    }
}

/// Each moving body's velocity before this physics step (impact damage
/// needs the speed a collision took away).
#[derive(Resource, Default)]
pub(super) struct PreStep(std::collections::HashMap<Entity, Vec3>);

pub(super) fn record_velocities(mut pre: ResMut<PreStep>, bodies: Query<(Entity, &RigidBody, &LinearVelocity)>) {
    pre.0.clear();
    pre.0.extend(
        bodies
            .iter()
            .filter(|(_, rb, _)| rb.is_dynamic())
            .map(|(e, _, v)| (e, v.0)),
    );
}

/// A player's mass for impacts (kg; Source's player shadow).
pub(super) const PLAYER_MASS: f32 = 85.0;
/// A prop landing on a player meets it as two contacts in Source's physics (CS:S
/// measured 2026-10-17, prop_damage.md 6 and Q13,
/// `tools/css_probe/impacts.py`; HL2:DM 2026-11-04, gravity_gun.md 9,
/// `tools/hl2dm_probe/cases.py drops`); each counts as the prop losing
/// this share of its speed into the player, by the prop's mass (kg):
/// (mass, [first, second]). Fitted to the landings, the speed being the
/// prop's at the head: CS:S's 30 kg crate at 576, 848 and 1233 in/s hurt
/// the player 5 + 5, 5 + 10 and 10 + 20 (shares 0.31-0.35, then
/// 0.43-0.52); its 500 kg cupboard at 259 to 1522 in/s 5 + 50 up to
/// 500 + 500 (0.262-0.274, then 0.62-0.73; 0.276 since props feel air
/// drag, which slows the cupboard about 0.6 in/s before it lands at
/// 100 in/s and took its first share under the 10-point band). HL2:DM's
/// (energy scale 4) 30 kg crate at 284, 389, 524 and 663: 5 + 10, 5 + 10,
/// 10 + 20, 10 + 50 (second 0.51-0.61, so 30 kg's second is the narrow
/// overlap with CS:S's, 0.512); 10 kg at 420: one 10 (first under 0.36,
/// second over 0.6); 100 kg at 572 and 780: 20 + 100, 100 + 500 (first
/// 0.29-0.31, second 0.41-0.56); 250 kg at 582: 100 + 500.
/// Between the points by the logarithm of the mass, clamped at the ends.
pub(super) const PLAYER_CONTACT_SHARES: [(f32, &[f32]); 4] = [
    (10.0, &[0.335, 0.65]),
    (30.0, &[0.335, 0.512]),
    (100.0, &[0.31, 0.512]),
    (500.0, &[0.276, 0.67]),
];

/// A prop running into a player from the side (its contact normal within
/// `SIDE_CONTACT` of level) meets it as three contacts: HL2:DM measured
/// 2026-11-04 (gravity_gun.md 12, `cases.py crate_hits`): a 40-unit crate
/// punted level into a standing bot, 10 kg at 1500 in/s: 20 + 10 (shares
/// 0.43-0.52, 0.24-0.43, the third under 0.14); 30 kg at 1000: 20 + 10 + 5
/// (0.38-0.47, 0.21-0.38, 0.13-0.21), and at 1100 from 50 ahead of the
/// bot: 50 + 10 + 5 (0.42-0.54, 0.19-0.34, 0.11-0.19); 100 kg at 730: 50 + 10 + 10
/// (0.35-0.45, 0.16-0.29 twice); 250 kg at 650: 100 + 5 + 10 (0.32-0.46,
/// 0.07-0.11, 0.11-0.20). The values are mid-band fits, not vphysics' law.
pub(super) const PLAYER_SIDE_SHARES: [(f32, &[f32]); 4] = [
    (10.0, &[0.47, 0.33, 0.07]),
    (30.0, &[0.44, 0.30, 0.17]),
    (100.0, &[0.40, 0.22, 0.22]),
    (250.0, &[0.39, 0.09, 0.16]),
];

/// A contact whose normal's vertical part is under this (the sine of its
/// slope) is a side hit (`PLAYER_SIDE_SHARES`), else a landing.
pub(super) const SIDE_CONTACT: f32 = 0.7;

/// The contact shares for a prop of `mass` kg meeting a player, the
/// contact normal's vertical part `up` (None: unknown, a landing).
pub(super) fn player_contact_shares(mass: f32, up: Option<f32>) -> Vec<f32> {
    let points: &[(f32, &[f32])] = if up.is_some_and(|u| u.abs() < SIDE_CONTACT) {
        &PLAYER_SIDE_SHARES
    } else {
        &PLAYER_CONTACT_SHARES
    };
    let m = mass.max(1e-3);
    let k = points.iter().position(|(at, _)| *at > m).unwrap_or(points.len()).clamp(1, points.len() - 1);
    let ((m0, a), (m1, b)) = (points[k - 1], points[k]);
    let t = ((m.ln() - m0.ln()) / (m1.ln() - m0.ln())).clamp(0.0, 1.0);
    a.iter().zip(b.iter()).map(|(a, b)| a + (b - a) * t).collect()
}
/// Damage (health points) that breaks any breakable: an impact on one
/// that breaks on the first physics impact (flag 512).
pub(super) const BREAK_NOW: f32 = 1.0e6;
/// What a game's rule makes of a body's first new contact after it was
/// launched from a hold (`map::held::LaunchedFromHold`): its own impact
/// damage this many times over (HL2:DM's 10, specs/hl2dm/gravity_gun.md 9),
/// and a prop with the `firstimpact_break` interaction breaks whatever the
/// speed. Without it the mark does nothing.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct FirstImpactScale(pub f32);

/// A prop's side hit on a player is dealt on the step it begins rather than
/// when measured (its damage needs only the speed it came with and the
/// contact's normal): a game whose players are knocked on that tick (HL2:DM,
/// specs/hl2dm/gravity_gun.md Open questions 14).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct PromptSideHits;

/// A prop that breaks and explodes with no player as its attacker
/// credits its influencer (`map::held::PhysicsInfluence`) when the
/// influence is at most this old (s): HL2:DM's 2 (gravity_gun.md 9).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct BlastInfluenceWindow(pub f64);

/// What a game's explosive props' kills count as (the death notice); none:
/// a map explosion's (`weapon::grenade::EXPLOSION_WEAPON`, CS:S's).
#[derive(Resource, Clone, Copy, Debug)]
pub struct PropBlastWeapon(pub &'static str);

/// An impact is measured over this many physics steps from its first
/// contact: avian's contacts are speculative and soft, so a body is
/// stopped over a few steps where Source's stops it in one.
pub(super) const IMPACT_STEPS: u32 = 4;

/// Who an impact damages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Receiver {
    /// A logic prop: its table, energy scale and mass.
    Prop(crate::logic::prop_damage::ImpactTable, f32, f32),
    /// A breakable brush (static): its table, energy scale and mass, and
    /// whether the first impact breaks it.
    Breakable(crate::logic::prop_damage::ImpactTable, f32, f32, bool),
    Player,
}

/// An impact being measured: both bodies' velocities when it began and
/// the slowest each has been since (engine space).
#[derive(Clone, Debug)]
pub(super) struct Impact {
    me: Entity,
    other: Entity,
    receiver: Receiver,
    before: [Vec3; 2],
    slowest: [Vec3; 2],
    steps: u32,
    /// The contact's normal when it began, from `me` toward `other`
    /// (engine space; None: no contact points yet).
    normal: Option<Vec3>,
    /// `me`'s first contact since its launch from a hold.
    first_after_launch: bool,
    /// Dealt on its first step (a side hit in a game with `PromptSideHits`),
    /// measured on only to keep stress off it.
    prompt: bool,
    dealt: bool,
}

#[derive(Resource, Default)]
pub(super) struct Impacts(Vec<Impact>);

/// A game whose props broken other than by lethal damage (the Break
/// input, a health input, pressure) stay in the world, non-solid and
/// undamageable (`LogicWorld::break_keeps_props`): HL2:DM's
/// (specs/hl2dm/gravity_gun.md Corrections 2, measured P15). Read at map
/// load.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct KeepPropsOnBreak(pub bool);

/// A game's energy scale for physics impacts on its players and for their
/// stress (prop_damage.md 6: CS:S's player 1; HL2:DM's 4,
/// specs/hl2dm/gravity_gun.md 9).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct PlayerImpactScale(pub f32);

impl Default for PlayerImpactScale {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Whether the logic prop on `node` breaks on its first impact after a
/// launch (the model's `onfirstimpact` "break").
pub(super) fn breaks_on_first_impact(logic: &Logic, node: Entity) -> bool {
    logic
        .props
        .iter()
        .find(|(_, n)| *n == node)
        .and_then(|(id, _)| logic.world.prop(*id))
        .is_some_and(|p| p.interactions.iter().any(|x| x == "firstimpact_break"))
}

/// Physics impacts (prop_damage.md 5, 6): a new contact of a logic prop's
/// body damages it by the speed the collision took away (its table and
/// energy scale), and a moving prop hitting a player damages the player
/// (the player table, attacker = the prop), as crush damage applied next
/// tick. A prop and a player meet as two contacts landing
/// (`PLAYER_CONTACT_SHARES`), three from the side (`PLAYER_SIDE_SHARES`),
/// each hurting both: the prop by the player (attacker = the player) too.
#[allow(clippy::too_many_arguments)]
pub(super) fn impacts(
    mut started: MessageReader<CollisionStart>,
    logic: Option<Res<Logic>>,
    pre: Res<PreStep>,
    mut pending: ResMut<Impacts>,
    bodies: Query<(
        &RigidBody,
        &LinearVelocity,
        Option<&ComputedMass>,
        Has<crate::map::PhysicsProp>,
    )>,
    characters: Query<&MovementState, With<Intent>>,
    shadows: Query<&crate::map::prop_physics::PhysicsShadow>,
    positions: Query<&Position>,
    collisions: avian3d::prelude::Collisions,
    (held, player_scale): (Query<(), With<crate::map::held::Held>>, Res<PlayerImpactScale>),
    (launched, sensors, first_scale, knock): (
        Query<(), With<crate::map::held::LaunchedFromHold>>,
        Query<(), With<avian3d::prelude::Sensor>>,
        Option<Res<FirstImpactScale>>,
        Option<Res<PromptSideHits>>,
    ),
    mut damage: MessageWriter<Damage>,
    mut commands: Commands,
) {
    let Some(logic) = logic else {
        started.clear();
        pending.0.clear();
        return;
    };
    let scale = logic.scale;
    let velocity = |e: Entity| match bodies.get(e) {
        Ok((rb, v, ..)) if rb.is_dynamic() => v.0,
        _ => Vec3::ZERO,
    };
    // A player's physics shadow touches props for its player.
    let player = |e: Entity| shadows.get(e).map_or(e, |s| s.owner);
    // New contacts start being measured.
    for s in started.read() {
        let (b1, b2) = (
            player(s.body1.unwrap_or(s.collider1)),
            player(s.body2.unwrap_or(s.collider2)),
        );
        // A held body deals and takes no impact damage (gravity_gun.md 5).
        if b1 == b2 || held.contains(b1) || held.contains(b2) {
            continue;
        }
        let sensor = sensors.contains(s.collider1) || sensors.contains(s.collider2);
        for (me, other) in [(b1, b2), (b2, b1)] {
            // The mark goes with the first solid contact, whatever else.
            let first_after_launch = !sensor && first_scale.is_some() && launched.contains(me);
            if first_after_launch {
                commands.entity(me).remove::<crate::map::held::LaunchedFromHold>();
            }
            let receiver = if let Some((id, _)) = logic.props.iter().find(|(_, n)| *n == me)
                && let Some((table, energy, mass)) = logic.world.prop_impact(*id)
            {
                // A player's contact counts too (prop_damage.md Q13; only
                // prop_physics reach players: multiplayer props never touch
                // them, `map::prop_physics::ShadowContacts`).
                Receiver::Prop(table, energy, mass.max(0.1))
            } else if let Some((id, _)) = logic.nodes.iter().find(|(_, n)| *n == me)
                && let Some((table, energy, mass, instant)) = logic.world.breakable_impact(*id)
                // Only physics objects (players break them by touch).
                && bodies.get(other).is_ok_and(|(rb, ..)| rb.is_dynamic())
                && !characters.contains(other)
            {
                Receiver::Breakable(table, energy, mass, instant)
            } else if characters.get(me).is_ok_and(|s| s.ground != Some(other))
                && bodies.get(other).is_ok_and(|(rb, _, _, prop)| rb.is_dynamic() && prop)
            {
                Receiver::Player
            } else {
                continue;
            };
            if pending.0.iter().any(|i| i.me == me && i.other == other) {
                continue;
            }
            let before = [me, other].map(|e| pre.0.get(&e).copied().unwrap_or_else(|| velocity(e)));
            // The manifolds' normals point from collider1 to collider2.
            let normal = collisions.get(s.collider1, s.collider2).and_then(|pair| {
                let n: Vec3 = pair.manifolds.iter().map(|m| m.normal * m.points.len() as f32).sum();
                let from_first = player(pair.body1.unwrap_or(pair.collider1)) == me;
                n.try_normalize().map(|n| if from_first { n } else { -n })
            });
            pending.0.push(Impact {
                me,
                other,
                receiver,
                before,
                slowest: before,
                steps: 0,
                normal,
                first_after_launch,
                // Its damage reads the speed it came with and the normal
                // alone, and with the knock the player must feel it now.
                prompt: knock.is_some()
                    && (characters.contains(me) || characters.contains(other))
                    && normal.is_some_and(|n| n.y.abs() < SIDE_CONTACT),
                dealt: false,
            });
        }
    }
    // Measure; done ones deal their damage.
    let mut done = Vec::new();
    pending.0.retain_mut(|i| {
        for (k, e) in [i.me, i.other].into_iter().enumerate() {
            let v = velocity(e);
            if v.length_squared() < i.slowest[k].length_squared() {
                i.slowest[k] = v;
            }
        }
        i.steps += 1;
        if !i.dealt && (i.prompt || i.steps >= IMPACT_STEPS) {
            i.dealt = true;
            done.push(i.clone());
        }
        i.steps < IMPACT_STEPS
    });
    for i in done {
        let fixed = |e: Entity| !bodies.get(e).is_ok_and(|(rb, ..)| rb.is_dynamic());
        let mass = |e: Entity| {
            bodies
                .get(e)
                .ok()
                .and_then(|(_, _, m, _)| m.map(|m| m.value()))
                .unwrap_or(1.0)
        };
        let side = |k: usize, e: Entity, m: f32| crate::logic::prop_damage::Impactor {
            mass: m,
            before: engine_to_entity(i.before[k], scale),
            after: engine_to_entity(i.slowest[k], scale),
            fixed: fixed(e),
        };
        // A player (through its shadow): an 85 kg body whose own speed
        // change isn't measured.
        let by_player = characters.contains(i.other);
        let theirs = if by_player {
            crate::logic::prop_damage::Impactor {
                mass: PLAYER_MASS,
                ..default()
            }
        } else {
            side(1, i.other, mass(i.other))
        };
        // A prop meeting a player counts as two contacts landing on it, three
        // from the side (measured), each the prop losing a share of its
        // speed (by its mass).
        let player_contact = by_player || i.receiver == Receiver::Player;
        let shares: Vec<f32> = if player_contact {
            let prop_mass = match i.receiver {
                Receiver::Prop(_, _, m) => m,
                _ => theirs.mass,
            };
            player_contact_shares(prop_mass, i.normal.map(|n| n.y))
        } else {
            vec![1.0]
        };
        let point = positions.get(i.me).map_or(Vec3::ZERO, |p| p.0);
        // Where the prop loses speed into the player: along the contact's
        // normal (entity space), when it has one.
        let toward = |n: Option<Vec3>| n.map(|n| engine_to_entity(n, 1.0).normalize_or_zero());
        for &share in &shares {
            let (amount, force) = match i.receiver {
                Receiver::Prop(table, energy, m) => {
                    let mine = crate::logic::prop_damage::Impactor {
                        // A frozen prop still counts its own mass.
                        fixed: false,
                        ..side(0, i.me, m)
                    };
                    let mine = if by_player {
                        mine.lost_toward(share, toward(i.normal))
                    } else {
                        mine.share(share)
                    };
                    let amount = crate::logic::prop_damage::impact_damage(&table, energy, mine, theirs, false);
                    let amount = match first_scale.as_deref().filter(|_| i.first_after_launch) {
                        Some(_) if breaks_on_first_impact(&logic, i.me) => amount.max(BREAK_NOW),
                        Some(first) => amount * first.0,
                        None => amount,
                    };
                    (amount, velocity(i.me) * m)
                }
                Receiver::Breakable(table, energy, m, instant) => {
                    // A static brush: its speed never changes.
                    let mine = crate::logic::prop_damage::Impactor { mass: m, ..default() };
                    let amount = crate::logic::prop_damage::impact_damage(&table, energy, mine, theirs, false);
                    // "Break immediately on physics": any impact is lethal.
                    let amount = if instant { amount.max(BREAK_NOW) } else { amount };
                    (amount, i.slowest[1] * theirs.mass)
                }
                Receiver::Player => {
                    let player = crate::logic::prop_damage::Impactor {
                        mass: PLAYER_MASS,
                        ..default()
                    };
                    let amount = crate::logic::prop_damage::impact_damage(
                        &crate::logic::prop_damage::PLAYER_TABLE,
                        player_scale.0,
                        player,
                        // The prop moves toward the player against the
                        // normal (from the player to it).
                        theirs.lost_toward(share, toward(i.normal.map(|n| -n))),
                        true,
                    );
                    (amount, Vec3::ZERO)
                }
            };
            if amount > 0.0 && bodies.contains(i.me) | characters.contains(i.me) {
                damage.write(Damage {
                    target: i.me,
                    attacker: (by_player || !theirs.fixed).then_some(i.other),
                    amount: amount / 100.0,
                    point,
                    dir: i.before[1].normalize_or(Vec3::NEG_Y),
                    hitgroup: Hitgroup::Generic,
                    kind: DamageKind::Crush,
                    weapon: None,
                    force,
                });
            }
        }
    }
}

/// Physics stress on players (prop_damage.md 6, "Stress"): what a
/// player's received stress is scaled by (`player_stress_scale`, over its
/// 85 kg), the stress past which it is crushed, the crush damage and the
/// mass of a "large object" one of its contacts must be.
pub(super) const PLAYER_STRESS_SCALE: f32 = 0.125;
pub(super) const STRESS_LIMIT: f32 = 5.0;
pub(super) const STRESS_DAMAGE: f32 = 200.0;
pub(super) const STRESS_LARGE_MASS: f32 = 500.0;
/// Avian 0.7's contact `normal_impulse` adds each substep's impulse after
/// both its solve and its relax pass (`normal_part.rs`, `total_impulse`):
/// twice what the step applied (a 4000 kg box resting on a player read
/// 7900 kg).
pub(super) const AVIAN_IMPULSE_PASSES: f32 = 2.0;

/// Player stress (prop_damage.md 6): each physics step, the force (kg
/// of weight) moving props press on a player with, through its own
/// collider or its physics shadow, not counting what it stands on; past
/// `STRESS_LIMIT` (scaled by `PLAYER_STRESS_SCALE` over its 85 kg) with a
/// `STRESS_LARGE_MASS` prop among them: `STRESS_DAMAGE` crush from the
/// world, pushing straight down.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn stress(
    logic: Option<Res<Logic>>,
    collisions: avian3d::prelude::Collisions,
    time: Res<Time>,
    gravity: Option<Res<avian3d::prelude::Gravity>>,
    characters: Query<(Entity, &MovementState, &Health, Option<&crate::map::prop_physics::Shadowed>), With<Intent>>,
    props: Query<(&RigidBody, &crate::map::PhysicsProp)>,
    of: Query<&avian3d::prelude::ColliderOf>,
    impacts: Res<Impacts>,
    player_scale: Res<PlayerImpactScale>,
    mut damage: MessageWriter<Damage>,
) {
    if logic.is_none() {
        return;
    }
    let dt = time.delta_secs();
    let g = gravity.map_or(0.0, |g| g.0.length());
    if dt <= 0.0 || g <= 0.0 {
        return;
    }
    let body = |e: Entity| of.get(e).map_or(e, |o| o.body);
    for (player, state, health, shadow) in &characters {
        if health.current <= 0.0 {
            continue;
        }
        let mut received = 0.0;
        let mut large = false;
        for me in std::iter::once(player).chain(shadow.and_then(|s| s.0)) {
            for pair in collisions.collisions_with(me) {
                if !pair.flags.contains(avian3d::prelude::ContactPairFlags::TOUCHING) {
                    continue;
                }
                let other = body(if body(pair.collider1) == me {
                    pair.collider2
                } else {
                    pair.collider1
                });
                // What it stands on doesn't count, nor a contact still
                // being measured as an impact (its first steps: the blow
                // of a landing is impact damage; CS:S's landings of the
                // 500 kg cupboard didn't crush, measured).
                if state.ground == Some(other) || impacts.0.iter().any(|i| i.me == player && i.other == other) {
                    continue;
                }
                let Ok((rb, prop)) = props.get(other) else { continue };
                if !rb.is_dynamic() {
                    continue;
                }
                received += pair.total_normal_impulse_magnitude() / AVIAN_IMPULSE_PASSES / dt / g;
                large |= prop.mass >= STRESS_LARGE_MASS;
            }
        }
        if large && received * player_scale.0 * PLAYER_STRESS_SCALE / PLAYER_MASS > STRESS_LIMIT {
            damage.write(Damage {
                target: player,
                attacker: None,
                amount: STRESS_DAMAGE / 100.0,
                point: Vec3::ZERO,
                dir: Vec3::NEG_Y,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Crush,
                weapon: None,
                force: Vec3::NEG_Y * received * g * dt,
            });
        }
    }
}

/// Outputs game rules ask a map entity to fire (a bomb target's
/// `BombExplode`), with the player as activator.
pub(super) fn fire_outputs(mut asks: MessageReader<crate::map::entities::FireEntityOutput>, logic: Option<ResMut<Logic>>) {
    let Some(mut logic) = logic else {
        asks.clear();
        return;
    };
    for a in asks.read() {
        let id = logic
            .world
            .ids()
            .into_iter()
            .find(|id| logic.world.get(*id).is_some_and(|e| e.map_index == Some(a.map_index)));
        if let Some(id) = id {
            let who = a.activator.map(crate::logic::world::Who::Player);
            logic.world.fire_output(id, &a.output, who, crate::logic::Value::Void);
            // A bomb target's legacy `target` is used when it explodes
            // (spec objectives.md 6.3, *hyp.*).
            let target = logic.world.get(id).and_then(|e| e.kv("target")).map(str::to_string);
            if a.output.eq_ignore_ascii_case("BombExplode")
                && let Some(t) = target.filter(|t| !t.is_empty())
            {
                logic.world.queue_input(&t, "Use", crate::logic::Value::Void, 0.0, who);
            }
        }
    }
}

/// A logic entity's key in messages to games.
pub(super) fn entity_key(id: EntId) -> u64 {
    (id.generation as u64) << 32 | id.index as u64
}

/// Outputs with a number a game asks a map entity to fire.
pub(super) fn fire_output_values(
    mut asks: MessageReader<crate::map::entities::FireEntityOutputValue>,
    logic: Option<ResMut<Logic>>,
) {
    let Some(mut logic) = logic else {
        asks.clear();
        return;
    };
    for a in asks.read() {
        let id = logic
            .world
            .ids()
            .into_iter()
            .find(|id| logic.world.get(*id).is_some_and(|e| e.map_index == Some(a.map_index)));
        if let Some(id) = id {
            let who = a.activator.map(crate::logic::world::Who::Player);
            logic.world.fire_output(id, &a.output, who, crate::logic::Value::Float(a.value));
        }
    }
}

/// Outputs a game asks a logic entity to fire, by key.
pub(super) fn fire_keyed_outputs(mut asks: MessageReader<crate::map::entities::FireKeyedOutput>, logic: Option<ResMut<Logic>>) {
    let Some(mut logic) = logic else {
        asks.clear();
        return;
    };
    for a in asks.read() {
        if let Some(id) = logic.world.ids().into_iter().find(|id| entity_key(*id) == a.key) {
            let who = a.activator.map(crate::logic::world::Who::Player).or(Some(crate::logic::world::Who::Ent(id)));
            logic.world.fire_output(id, &a.output, who, crate::logic::Value::Void);
        }
    }
}

/// The AI sound list's new sounds (bullet impacts, blasts) into the logic.
pub(super) fn take_ai_sounds(mut sounds: MessageReader<crate::map::entities::AiSound>, logic: Option<ResMut<Logic>>) {
    let Some(mut logic) = logic else {
        sounds.clear();
        return;
    };
    let scale = logic.scale;
    for s in sounds.read() {
        logic
            .world
            .add_ai_sound(engine_to_entity(s.at, scale), s.volume / scale, s.kind, s.life);
    }
}

/// A broken respawnable prop's time has come: if its spot is free its body
/// goes back there as at the map's start and the logic revives it, else
/// the logic asks again in a second (hl2dm/gravity_gun.md 13).
pub(super) fn prop_respawn(world: &mut World, id: EntId) {
    let Some(node) = prop_node(world, id) else { return };
    let home = world.get::<PropHome>(node).map(|h| h.0);
    let size = std::iter::once(node)
        .chain(world.get::<Children>(node).map(|c| c.to_vec()).unwrap_or_default())
        .find_map(|e| world.get::<ColliderAabb>(e).map(|a| a.size()))
        .unwrap_or(Vec3::splat(0.3));
    let blocked = home.is_some_and(|h| {
        use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
        use bevy::ecs::system::RunSystemOnce;
        let shape = Collider::cuboid(size.x * 0.9, size.y * 0.9, size.z * 0.9);
        world
            .run_system_once(move |q: SpatialQuery| {
                let filter = SpatialQueryFilter::from_excluded_entities([node]);
                let mut hit = false;
                q.shape_intersections_callback(&shape, h.translation, h.rotation, &filter, |_| {
                    hit = true;
                    false
                });
                hit
            })
            .unwrap_or(false)
    });
    if blocked {
        if let Some(mut logic) = world.get_resource_mut::<Logic>() {
            logic.world.prop_respawn_blocked(id);
        }
        return;
    }
    if let Some(home) = home
        && let Ok(mut e) = world.get_entity_mut(node)
        && e.contains::<RigidBody>()
    {
        if let Some(mut t) = e.get_mut::<Transform>() {
            *t = home;
        }
        if let Some(mut p) = e.get_mut::<Position>() {
            p.0 = home.translation;
        }
        if let Some(mut r) = e.get_mut::<Rotation>() {
            *r = Rotation::from(home.rotation);
        }
        if let Some(mut v) = e.get_mut::<LinearVelocity>() {
            v.0 = Vec3::ZERO;
        }
        if let Some(mut v) = e.get_mut::<AngularVelocity>() {
            v.0 = Vec3::ZERO;
        }
    }
    let asleep = world
        .get_resource::<Logic>()
        .and_then(|l| l.world.prop_states().into_iter().find(|s| s.id == id))
        .is_some_and(|s| s.asleep);
    if let Some(mut logic) = world.get_resource_mut::<Logic>() {
        logic.world.prop_revive(id);
    }
    crate::map::prop_physics::reset(world, node, asleep);
    if let Ok(mut e) = world.get_entity_mut(node) {
        e.insert(Damageable);
    }
}

/// The logic's entities for tools (`map::logic_view::LogicInspect`).
pub(super) fn inspect_entities(world: &World) -> Option<Vec<crate::map::logic_view::LogicEntity>> {
    let logic = world.get_resource::<Logic>()?;
    Some(
        logic
            .world
            .ids()
            .into_iter()
            .filter_map(|id| logic.world.get(id))
            .map(|e| crate::map::logic_view::LogicEntity {
                name: e.targetname.clone(),
                class: e.classname.clone(),
                origin: e.origin,
                keys: e.keyvalues.clone(),
                outputs: e
                    .outputs
                    .iter()
                    .flat_map(|(o, cs)| {
                        cs.iter().map(move |c| {
                            format!(
                                "{o} -> {} {}{} after {} s{}",
                                c.target,
                                c.input,
                                c.param.as_ref().map_or(String::new(), |p| format!(" ({p})")),
                                c.delay,
                                if c.times > 0 { format!(", {} more times", c.times) } else { String::new() }
                            )
                        })
                    })
                    .collect(),
            })
            .collect(),
    )
}

/// The latest `n` outputs fired, newest first, for tools.
pub(super) fn inspect_fired(world: &World, n: usize) -> Vec<String> {
    let Some(logic) = world.get_resource::<Logic>() else {
        return Vec::new();
    };
    logic
        .world
        .fired
        .iter()
        .rev()
        .take(n)
        .map(|(tick, id, out)| {
            let who = logic.world.get(*id).map_or("?".to_string(), |e| {
                if e.targetname.is_empty() { e.classname.clone() } else { e.targetname.clone() }
            });
            format!("tick {tick}: {who} {out}")
        })
        .collect()
}

