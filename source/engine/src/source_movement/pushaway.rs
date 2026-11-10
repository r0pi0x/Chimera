//! Players and multiplayer physics props (specs/cs_source/physics_props.md
//! 4.2): players walk through them; a player shoves the props around them
//! (an impulse at the player's centre, so they spin too; one asleep since
//! it spawned wakes, with OnAwakened) at most every 0.1 s (`PUSH_INTERVAL`,
//! measured), and "solid"-mode props push the player back through the move
//! input.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{MovementConfigs, SourceMovement, SourceMovementConfig, to_source};
use crate::{
    core::{SimSet, Velocity},
    map::{
        PhysicsProp, PushAway,
        prop_physics::{PropAwakened, StartAsleep},
    },
};

/// The query box is the player's box grown by this much, units.
const QUERY_EXPAND: f32 = 3.0;
/// "solid"-mode props aren't shoved by players slower than this (units/s).
const MIN_PLAYER_SPEED: f32 = 75.0;
const PUSH_FORCE: f32 = 30_000.0;
const PUSH_MAX: f32 = 1_000.0;
/// A player shoves props at most this often, seconds: CS:S kicked a
/// 30 kg crate on ice overlapping a standing bot by 1000 kg·units/s every
/// 7 ticks (0.105 s), not every tick (measured 2026-10-18,
/// physics_props.md Q12).
pub const PUSH_INTERVAL: f64 = 0.1;
const PUSHBACK_FORCE: f32 = 200_000.0;
const PUSHBACK_MAX: f32 = 10_000.0;
/// Applied to every client push-back (a quirk the game keeps).
const PUSHBACK_SCALE: f32 = 0.25;
const PUSHBACK_GAP: f32 = 5.0;
const HALF_WIDTH: f32 = 16.0;
/// The movement's transform sits this far above the feet (units).
const ORIGIN_ABOVE_FEET: f32 = 36.0;

pub struct PushAwayPlugin;

impl Plugin for PushAwayPlugin {
    fn build(&self, app: &mut App) {
        use chimera_core::character::CharacterPartsExt;
        app.add_message::<PropAwakened>()
            .on_every_character::<NextPush>()
            .add_systems(FixedUpdate, push_props.after(SimSet::Movement));
    }
}

/// A prop's world box in Source units (its rotated bounds' extent).
pub fn prop_box(transform: &Transform, bounds: (Vec3, Vec3)) -> (Vec3, Vec3) {
    let (lo, hi) = bounds;
    let corners = (0..8).map(|i| {
        let c = Vec3::new(
            if i & 1 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 4 == 0 { lo.z } else { hi.z },
        );
        to_source(transform.transform_point(c))
    });
    corners.fold((Vec3::MAX, Vec3::MIN), |(a, b), c| (a.min(c), b.max(c)))
}

/// Whether a prop's world box (`prop_box`) surely misses `query` (Source
/// units): the cube around its bounds' centre that holds the box at any
/// turn (half a side: the bounds' half diagonal, with slack for rounding)
/// does. Tells nothing else: a prop that isn't surely apart gets the exact
/// test. Every player asks about every prop each tick, and the exact box
/// is eight transformed corners (docs/performance.md, "Performance pass").
pub fn surely_apart(transform: &Transform, bounds: (Vec3, Vec3), query: (Vec3, Vec3)) -> bool {
    let (lo, hi) = bounds;
    let centre = to_source(transform.transform_point((lo + hi) * 0.5));
    let radius = (hi - lo).length() * 0.5 * transform.scale.abs().max_element() / super::METERS_PER_UNIT;
    let reach = Vec3::splat(radius * 1.01 + 1.0);
    // Written so a NaN anywhere says "not surely apart".
    (centre - reach).cmpgt(query.1).any() || (centre + reach).cmplt(query.0).any()
}

/// The player's box (Source units) from the movement transform.
pub fn player_box(transform: &Transform, me: &SourceMovement, cfg: &SourceMovementConfig) -> (Vec3, Vec3) {
    let feet = to_source(transform.translation) - Vec3::Z * ORIGIN_ABOVE_FEET;
    let height = if me.ducked { cfg.duck_height } else { cfg.stand_height };
    (
        feet + Vec3::new(-HALF_WIDTH, -HALF_WIDTH, 0.0),
        feet + Vec3::new(HALF_WIDTH, HALF_WIDTH, height),
    )
}

fn overlap(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
    a.0.cmple(b.1).all() && b.0.cmple(a.1).all()
}

/// Shove props away from players (spec 4.2.2). A prop asleep since it
/// spawned wakes when a player shoves it (CS:S, measured 2026-10-17: a
/// player walking into cs_office's trash can fires its OnAwakened;
/// specs/source/prop_damage.md Q14).
#[allow(clippy::type_complexity)]
fn push_props(
    players: Query<(
        Entity,
        &Transform,
        &Velocity,
        &SourceMovement,
        Option<&crate::core::MapControls>,
        Option<&NextPush>,
        Option<&crate::slots::MovementSlot>,
    )>,
    clock: Res<crate::core::SimClock>,
    mut props: Query<
        (Entity, &Transform, &PhysicsProp, Forces, Has<StartAsleep>),
        (Without<SourceMovement>, Without<RigidBodyDisabled>, Without<ColliderDisabled>),
    >,
    cfg: MovementConfigs,
    mut commands: Commands,
    mut awakened: MessageWriter<PropAwakened>,
) {
    for (who, pt, pv, me, controls, next, slot) in &players {
        if controls.is_some_and(|c| c.parented) {
            continue;
        }
        // Ticks land a hair off 0.1 s steps: compared with a little slack.
        if next.is_some_and(|n| clock.now + 1e-6 < n.0) {
            continue;
        }
        let mut pushed = false;
        let mut player = player_box(pt, me, cfg.of(slot));
        let centre = (player.0 + player.1) / 2.0;
        player.0 -= Vec3::splat(QUERY_EXPAND);
        player.1 += Vec3::splat(QUERY_EXPAND);
        let speed = to_source(pv.0).truncate().length();
        for (e, t, p, mut forces, asleep) in &mut props {
            if matches!(p.push, PushAway::Collide | PushAway::Ignore) {
                continue;
            }
            if p.push == PushAway::Solid && speed < MIN_PLAYER_SPEED {
                continue;
            }
            if surely_apart(t, p.bounds, player) {
                continue;
            }
            let b = prop_box(t, p.bounds);
            if !overlap(player, b) {
                continue;
            }
            if asleep {
                commands.entity(e).remove::<StartAsleep>().insert(RigidBody::Dynamic);
                awakened.write(PropAwakened(e));
                continue;
            }
            // kg.in/s to kg.m/s (to_engine scales units to meters), at the
            // player's centre, so the prop spins too.
            let impulse = super::to_engine(push_impulse(centre, (b.0 + b.1) / 2.0));
            let at = super::to_engine(centre);
            forces.apply_linear_impulse_at_point(impulse, at);
            pushed = true;
        }
        if pushed {
            commands.entity(who).insert(NextPush(clock.now + PUSH_INTERVAL));
        }
    }
}

/// When a player may next shove props (`push_props`).
#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct NextPush(pub f64);

/// The impulse a player at `player` (centre, units) gives a prop centred at
/// `prop`: horizontal, away from the player, min(30000 / dist, 1000)
/// kg.units/s (dist at least 1; none when exactly above or below).
pub fn push_impulse(player: Vec3, prop: Vec3) -> Vec3 {
    let mut d = prop - player;
    d.z = 0.0;
    let dist = d.length().max(1.0);
    d.normalize_or_zero() * (PUSH_FORCE / dist).min(PUSH_MAX)
}

/// Push-back from "solid"-mode props near the player (spec 4.2.3),
/// as additions to (forward, side) move input, in key units.
pub fn push_back<'a>(
    player: (Vec3, Vec3),
    forward: Vec3,
    right: Vec3,
    props: impl Iterator<Item = (&'a Transform, &'a PhysicsProp)>,
) -> Vec2 {
    let centre = (player.0 + player.1) / 2.0;
    let mut out = Vec2::ZERO;
    // Props within the gap count (the spec's cases: 2 units pushes, 6
    // doesn't).
    let reach = (
        player.0 - Vec3::splat(PUSHBACK_GAP),
        player.1 + Vec3::splat(PUSHBACK_GAP),
    );
    for (t, p) in props {
        if p.push != PushAway::Solid {
            continue;
        }
        if surely_apart(t, p.bounds, reach) {
            continue;
        }
        let b = prop_box(t, p.bounds);
        if !overlap(reach, b) {
            continue;
        }
        let w = p.mass.clamp(10.0, 30.0) / 30.0;
        let a = centre.clamp(b.0, b.1);
        let near = a.clamp(player.0, player.1);
        let mut v = near - a;
        let mut dist = v.length();
        let inside = a.cmpge(player.0).all() && a.cmple(player.1).all();
        if dist > PUSHBACK_GAP && !inside {
            continue;
        }
        if v == Vec3::ZERO {
            v = centre - a;
            if v == Vec3::ZERO {
                v = centre - (b.0 + b.1) / 2.0;
            }
            dist = v.length();
        }
        let n = v.normalize_or_zero();
        let dist = dist.max(1.0);
        let f = (PUSHBACK_FORCE * w / dist).min(PUSHBACK_MAX) * PUSHBACK_SCALE;
        out += Vec2::new(f * n.dot(forward), f * n.dot(right));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_movement::to_engine;

    #[test]
    fn server_push() {
        let player = Vec3::new(0.0, 0.0, 36.0);
        // 10 kg prop centred at (20, 0, 10): J = 1000 along +X (dv 100 in/s).
        assert!((push_impulse(player, Vec3::new(20.0, 0.0, 10.0)) - Vec3::new(1000.0, 0.0, 0.0)).length() < 1e-3);
        // At 40 units: 750.
        assert!((push_impulse(player, Vec3::new(40.0, 0.0, 10.0)).x - 750.0).abs() < 1e-3);
        // Closer than 1: the distance counts as 1, capped at 1000.
        assert!((push_impulse(player, Vec3::new(0.5, 0.0, 10.0)).x - 1000.0).abs() < 1e-3);
        // Straight above: no direction, no push.
        assert_eq!(push_impulse(player, Vec3::new(0.0, 0.0, 80.0)), Vec3::ZERO);
    }

    /// A 20 kg prop whose box face is at x = 18, in front of a player at the
    /// origin looking along +X.
    fn pushback(face: f32, mass: f32, push: PushAway) -> Vec2 {
        let player = (Vec3::new(-16.0, -16.0, 0.0), Vec3::new(16.0, 16.0, 72.0));
        // A prop box from x = face to face + 20, y -10..10, z 0..40, placed
        // so its world box is exactly that (unit model space, identity).
        let lo = to_engine(Vec3::new(face, -10.0, 0.0));
        let hi = to_engine(Vec3::new(face + 20.0, 10.0, 40.0));
        let prop = PhysicsProp {
            push,
            mass,
            bounds: (lo.min(hi), lo.max(hi)),
        };
        let t = Transform::IDENTITY;
        push_back(player, Vec3::X, Vec3::NEG_Y, [(&t, &prop)].into_iter())
    }

    /// `surely_apart` never rejects a prop whose exact box overlaps the
    /// query: boxes at many places, turns and scales around a player's.
    #[test]
    fn the_far_test_never_drops_an_overlap() {
        let query = (Vec3::new(-19.0, -19.0, -3.0), Vec3::new(19.0, 19.0, 75.0));
        let bounds = (Vec3::new(-0.3, -0.1, -0.5), Vec3::new(0.4, 0.9, 0.2));
        let (mut near, mut far) = (0, 0);
        for i in 0..40_000u32 {
            // A cheap fixed sequence of places and turns.
            let f = |k: u32| ((i.wrapping_mul(2654435761).rotate_left(k) % 2001) as f32 / 1000.0) - 1.0;
            let at = to_engine(Vec3::new(f(3) * 90.0, f(7) * 90.0, 36.0 + f(11) * 110.0));
            let t = Transform::from_translation(at)
                .with_rotation(Quat::from_euler(EulerRot::XYZ, f(13) * 3.1, f(17) * 3.1, f(19) * 3.1))
                .with_scale(Vec3::new(1.0 + f(23) * 0.5, 1.0 + f(29) * 0.5, 1.0 + f(5) * 0.5));
            let overlaps = overlap(query, prop_box(&t, bounds));
            let apart = surely_apart(&t, bounds, query);
            assert!(!(overlaps && apart), "dropped an overlapping prop at {at}");
            near += overlaps as u32;
            far += apart as u32;
        }
        // Both cases happened, and most misses were cut.
        assert!(near > 1_000 && far > 10_000, "near {near}, far {far}");
    }

    #[test]
    fn client_push_back() {
        // dist 2, w = 20/30: F = min(66667, 10000) x 0.25 = 2500, backwards.
        let f = pushback(18.0, 20.0, PushAway::Solid);
        assert!((f.x + 2500.0).abs() < 1e-2 && f.y.abs() < 1e-3, "{f}");
        // 50 kg: the same (cap).
        assert!((pushback(18.0, 50.0, PushAway::Solid).x + 2500.0).abs() < 1e-2);
        // A 6-unit gap: too far.
        assert_eq!(pushback(22.0, 20.0, PushAway::Solid), Vec2::ZERO);
        // Non-solid props don't push back.
        assert_eq!(pushback(18.0, 20.0, PushAway::NonSolid), Vec2::ZERO);
    }
}
