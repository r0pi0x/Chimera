//! Meeting physics props, for any movement (docs/DESIGN.md "Props across
//! games"): a character follows the prop rule of the map it is on
//! (`prop_physics::PlayerPropRules::for_character`: the map's game's, on a
//! map of no game its movement's game's), whatever movement moves it.
//! Under `PlayerProps::Shadow` (CS:S's, HL2:DM's, TF2's) its physics shadow
//! (`shadow`) shoves a `prop_physics` up to 350 kg and the character follows
//! it; a heavier one is a wall; multiplayer props are walked through, shoved
//! and, "solid" ones, push back (`pushaway`). Under `PlayerProps::Turbo`
//! (DoD:S's) it has no shadow, every `prop_physics` is a wall, and only the
//! push-away group is soft.
//!
//! What a movement does to take part (Quake's does it in its step, with its
//! own measured touch; Combat Arms' and Halo's through the neutral parts):
//! - its casts leave out the props it moves through (`PassedProps`: the
//!   rule's, one list a tick);
//! - it runs before `core::AfterMovements`, where `observe` reads its
//!   body for every character on a movement with a hull
//!   (`slots::MovementImpl::footing`) but the shared movement: the box from
//!   its collider, the feet, the ground, the keys' direction, which
//!   pushable prop the box touches and which it stands on
//!   (`shadow::PhysicsTouch`, the shadow's input), and gives it the "solid"
//!   soft props' push-back as an acceleration (`push_back_velocity`).
//!
//! The service's systems (`PropsPlugin`) are the app's: the shared
//! movement's plugin adds them, so every app with a Source game (whose maps
//! have the physics props) has them, and another app adds them with `add`.
//! A body other than the shared movement's has a shadow only while the map
//! has a physics prop (Combat Arms' and Halo's own maps have none, and their
//! own apps leave the service out).
//!
//! Known shortcuts (docs/tech-debt.md): Props across movements: a movement
//! other than the shared one is observed after its move, not inside it: it
//! touched a pushable prop when its box grown by `TOUCH_REACH` overlaps one
//! (not a hit of its move), its wish is the keys' direction at
//! `WISH_SPEED` (Source's run speed, not its own), and the soft props'
//! push-back is Source's accelerate toward the push at `PUSHBACK_ACCELERATE`
//! (Source's `sv_accelerate`), not through its own input. Why it's OK for
//! now: a Combat Arms or Halo body shoves a light crate and is stopped by a
//! heavy one as a Source one is, and its game's specs say nothing of props
//! from other games (apps/party_roster/tests/it/props_across_games.rs).

use avian3d::prelude::*;
use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{
    core::{CHARACTER_FILTER, Intent, MapGame, MovementState, SimClock, Velocity},
    map::{
        PhysicsProp,
        prop_physics::{PlayerPropRules, PlayerProps, PropGroups},
    },
    pushaway, shadow,
    slots::{MovementRegistry, MovementSlot},
    state::QuakeMovement,
    units::{METERS_PER_UNIT, to_engine, to_units},
};

/// A body grown by this much each way (units) touches what it is against.
pub const TOUCH_REACH: f32 = 1.0;
/// What stands under the feet within this (units) is stood on.
pub const STAND_REACH: f32 = 2.0;
/// A body's wish speed toward the keys' direction (units/s): Source's run.
pub const WISH_SPEED: f32 = 250.0;
/// The soft props' push-back: Source's accelerate (`sv_accelerate` 5) to a
/// wish speed of at most `PUSHBACK_SPEED` (units/s) along the push.
pub const PUSHBACK_ACCELERATE: f32 = 5.0;
pub const PUSHBACK_SPEED: f32 = 250.0;

/// The service's systems, once per app: the shadow, the push-away and
/// `observe`. The shared movement's plugin adds it.
pub struct PropsPlugin;

impl Plugin for PropsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::config::QuakeConfigs>()
            .add_systems(
                crate::core::Predict::Movement,
                observe.in_set(crate::core::AfterMovements),
            )
            .add_plugins((pushaway::PushAwayPlugin, shadow::ShadowPlugin));
    }
}

/// Add `PropsPlugin` once.
pub fn add(app: &mut App) {
    if !app.is_plugin_added::<PropsPlugin>() {
        app.add_plugins(PropsPlugin);
    }
}

/// A character's body as the props see it, Source units: written by its
/// movement each tick (`shadow::PhysicsTouch::body`). Height 0: none (a
/// movement without a hull, noclip's).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PropBody {
    /// The box's height and half its width.
    pub height: f32,
    pub half_width: f32,
    /// How far the character's origin (`Transform`) is above its feet.
    pub origin_above_feet: f32,
    pub on_ground: bool,
}

impl PropBody {
    /// The shared movement's body: its box for `cfg` (ducked or not), its
    /// origin `ORIGIN_ABOVE_FEET` over the feet.
    #[inline]
    pub fn quake(cfg: &crate::config::QuakeConfig, me: &QuakeMovement) -> Self {
        Self {
            height: if me.ducked { cfg.duck_height } else { cfg.stand_height },
            half_width: cfg.half_width,
            origin_above_feet: crate::units::ORIGIN_ABOVE_FEET,
            on_ground: me.on_ground,
        }
    }

    /// Whether a movement wrote it.
    #[inline]
    pub fn some(&self) -> bool {
        self.height > 0.0
    }

    /// The feet (bottom centre) under origin `t` (engine).
    #[inline]
    pub fn feet(&self, t: Vec3) -> Vec3 {
        to_units(t) - Vec3::Z * self.origin_above_feet
    }

    /// The box at `t` (Source units, min and max).
    #[inline]
    pub fn bounds(&self, t: Vec3) -> (Vec3, Vec3) {
        let feet = self.feet(t);
        (
            feet + Vec3::new(-self.half_width, -self.half_width, 0.0),
            feet + Vec3::new(self.half_width, self.half_width, self.height),
        )
    }
}

/// The rule a character meets, and the props its movement passes.
#[derive(SystemParam)]
pub struct PassedProps<'w, 's> {
    rules: Option<Res<'w, PlayerPropRules>>,
    map_game: Option<Res<'w, MapGame>>,
    props: Query<'w, 's, (Entity, &'static PhysicsProp), (Without<RigidBodyDisabled>, Without<ColliderDisabled>)>,
    groups: PropGroups<'w, 's>,
}

impl PassedProps<'_, '_> {
    /// The rule of a character on movement `movement` (an id).
    pub fn rule(&self, movement: Option<&str>) -> PlayerProps {
        PlayerPropRules::for_character(self.rules.as_deref(), self.map_game.as_deref(), movement)
    }

    /// The props a character's casts leave out under `rule`
    /// (`PropGroups::moved_through`).
    pub fn passed(&self, rule: PlayerProps) -> Vec<Entity> {
        self.props
            .iter()
            .filter(|(e, p)| self.groups.moved_through(*e, p, rule))
            .map(|(e, _)| e)
            .collect()
    }

    /// A filter of what a character's body meets (`mask`) but `me` and the
    /// props in `passed`.
    pub fn filter(me: Entity, passed: &[Entity], mask: LayerMask) -> SpatialQueryFilter {
        SpatialQueryFilter::from_excluded_entities(std::iter::once(me).chain(passed.iter().copied())).with_mask(mask)
    }
}

/// The soft props' push-back on a body moving at `v` (units/s): Source's
/// accelerate toward `push` (the push-away's push-back, units/s of wish,
/// horizontal) for `dt` seconds.
pub fn push_back_velocity(v: Vec3, push: Vec3, dt: f32) -> Vec3 {
    let len = push.length();
    if len <= 0.0 {
        return v;
    }
    let dir = push / len;
    let speed = len.min(PUSHBACK_SPEED);
    let add = speed - v.dot(dir);
    if add <= 0.0 {
        return v;
    }
    v + dir * add.min(PUSHBACK_ACCELERATE * speed * dt)
}

/// Every character on a movement with a hull, but the shared movement's
/// (which writes its own): its body, what it touches and stands on, and
/// the soft props' push-back.
#[allow(clippy::type_complexity)]
fn observe(
    mut q: Query<
        (
            Entity,
            &Transform,
            &mut Velocity,
            &MovementState,
            &Intent,
            &Collider,
            &MovementSlot,
            &mut shadow::PhysicsTouch,
        ),
        (Without<QuakeMovement>, Without<crate::core::Carried>),
    >,
    registry: Res<MovementRegistry>,
    passed: PassedProps,
    soft: Query<
        (Entity, &Transform, &PhysicsProp),
        (Without<Intent>, Without<RigidBodyDisabled>, Without<ColliderDisabled>),
    >,
    shadow_props: shadow::ShadowProps,
    spatial: SpatialQuery,
    clock: Res<SimClock>,
) {
    let dt = clock.dt();
    for (e, t, mut vel, state, intent, collider, slot, mut touch) in &mut q {
        if !registry.get(slot.0).is_some_and(|m| m.footing.is_some()) {
            if touch.body.some() {
                *touch = shadow::PhysicsTouch::default();
            }
            continue;
        }
        let aabb = collider.aabb(t.translation, Quat::IDENTITY, 0.0);
        let (lo, hi) = (aabb.min, aabb.max);
        let size = (hi - lo) / METERS_PER_UNIT;
        let body = PropBody {
            height: size.y,
            half_width: 0.5 * size.x.max(size.z),
            origin_above_feet: (t.translation.y - lo.y) / METERS_PER_UNIT,
            on_ground: state.on_ground,
        };
        let feet = body.feet(t.translation);
        let v = to_units(vel.0);
        let rule = passed.rule(Some(slot.0));
        // The keys' direction (yaw 0 looks down -Z).
        let rot = Quat::from_rotation_y(intent.yaw);
        let keys = rot * Vec3::new(intent.move_axis.x, 0.0, -intent.move_axis.y);
        let wish = to_units(keys).normalize_or_zero() * WISH_SPEED;
        let mut new = shadow::PhysicsTouch {
            prev_feet: feet - v * dt,
            feet,
            wish,
            dt,
            body,
            ..default()
        };
        if rule == PlayerProps::Shadow {
            let filter = SpatialQueryFilter::from_excluded_entities([e]).with_mask(CHARACTER_FILTER);
            let half = Vec3::new(
                body.half_width + TOUCH_REACH,
                0.5 * body.height,
                body.half_width + TOUCH_REACH,
            );
            // From a step over the floor up, so the floor isn't touched.
            let low = STAND_REACH.min(0.5 * body.height);
            let side = Collider::cuboid(
                2.0 * half.x * METERS_PER_UNIT,
                (body.height - low) * METERS_PER_UNIT,
                2.0 * half.z * METERS_PER_UNIT,
            );
            let side_at = Vec3::new(
                0.5 * (lo.x + hi.x),
                lo.y + 0.5 * (body.height + low) * METERS_PER_UNIT,
                0.5 * (lo.z + hi.z),
            );
            let touched = spatial.shape_intersections(&side, side_at, Quat::IDENTITY, &filter);
            let ground = if state.on_ground {
                // Narrower than the body, so what it is against isn't
                // taken for what it stands on.
                let w = 2.0 * (body.half_width - TOUCH_REACH).max(TOUCH_REACH) * METERS_PER_UNIT;
                let under = Collider::cuboid(w, STAND_REACH * METERS_PER_UNIT, w);
                let under_at = Vec3::new(side_at.x, lo.y, side_at.z);
                spatial
                    .shape_intersections(&under, under_at, Quat::IDENTITY, &filter)
                    .into_iter()
                    .next()
            } else {
                None
            };
            new = shadow::PhysicsTouch {
                body,
                ..shadow_props.touch(&touched, ground, new.prev_feet, feet, wish, dt)
            };
        }
        if *touch != new {
            *touch = new;
        }
        // The "solid" soft props push it back (the rule's soft ones).
        let push = pushaway::push_back(
            body.bounds(t.translation),
            Vec3::X,
            Vec3::Y,
            soft.iter()
                .filter(|(p, ..)| rule != PlayerProps::Turbo || passed.groups.soft(*p))
                .map(|(_, t, p)| (t, p)),
        );
        if push != Vec2::ZERO {
            let out = push_back_velocity(v, push.extend(0.0), dt);
            vel.0 = to_engine(out);
        }
    }
}
