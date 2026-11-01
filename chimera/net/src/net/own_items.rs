//! A client's own projectiles, drawn from the fire frame (backlog, "Gun
//! game playtest, 2026-10-28"), an option: `cl_predict_projectiles 1`
//! (archived; default 0, the originals': Source and Combat Arms make no
//! client copy of a projectile, special_weapons.md "Projectile looks and
//! sounds in flight"). The server flies every projectile and clients
//! draw it from snapshots (`NetItem`, `weapons::draw_items`), in the past
//! by the interpolation delay; the shooter's own leaves the muzzle that
//! delay plus the round trip after the shot. With the option a game's
//! client, when its predicted weapon frame fires a projectile the first
//! time (`WeaponEvent::replay` false), spawns a `PredictedItem`: the
//! projectile's model, start and velocity, flown here on the same steps
//! the server's flight takes (`step`: a game's own frame; the move over
//! a step `v dt + gravity_term g dt²`), stopped by the map's solid
//! geometry, and drawn between its steps one step behind, as the server
//! draws its own (Combat Arms' objects, `special::sync_objects`).
//!
//! When the server's copy arrives with this client's id as its owner
//! (`NetItem::owner`), the oldest unclaimed prediction of the same model
//! claims it (`claim`): the copy is not shown (no `ShownItem`) while the
//! prediction is. The prediction hands over (`hand_over`: the copy gets
//! its `ShownItem`, the prediction goes) when the copy has stopped on the
//! server (its last two snapshots the same after it moved: a bolt stuck in
//! someone, a shell resting) or the prediction has stopped against the
//! world and the copy has caught up with it (within `CATCH_UP`), and goes
//! with the copy when the server removes it (a detonation: its effects
//! come from the server's `Detonation`). One never claimed in `UNCLAIMED`
//! (a shot the server didn't fire) goes. Bots' and other players'
//! projectiles are drawn from snapshots as before.
//!
//! Known shortcuts (docs/tech-debt.md): The prediction stops only at the
//! map (static bodies), not at players or props, and isn't corrected
//! toward the server's copy in flight: a projectile that hits a player
//! flies on through him for about the round trip until the server's
//! detonation removes it. Thrown grenades (both games) aren't predicted:
//! they bounce, and Source predicts the throw only (multiplayer.md,
//! "Predicted vs server-only"). Why it's OK for now: impact projectiles
//! and bolts fly straight or on a plain arc from a shared seed, so the
//! prediction matches the server's path; the overshoot is what Source
//! games with predicted projectiles show too.

use avian3d::prelude::*;
use bevy::{app::RunFixedMainLoopSystems, prelude::*};

use super::{
    NetItem,
    client::LocalClientId,
    interp::Snapshots,
};
use crate::{
    core::{NetRole, SOLID_LAYERS},
    map::{
        interp::{InterpSystems, NetDrawn},
        loose::ShownItem,
    },
};

/// A prediction no server copy claimed in this long (s) goes.
pub const UNCLAIMED: f32 = 1.0;
/// A stopped prediction hands over once the server's copy is drawn this
/// close to it (m).
pub const CATCH_UP: f32 = 0.5;
/// At most this long (s), whatever happens.
pub const LONGEST: f32 = 30.0;

/// A projectile this client fired, flown and drawn here until the
/// server's copy takes over (module doc).
#[derive(Component, Clone, Debug)]
pub struct PredictedItem {
    /// The server copy's `NetItem::model` (its `ShownItem` key).
    pub model: String,
    /// m/s, m/s² (signed: down is negative), and the weight of gravity's
    /// term in a step's move (½ for exact, a game's own number).
    pub velocity: Vec3,
    pub gravity: f32,
    pub gravity_term: f32,
    /// The flight's step (s): the server's.
    pub step: f32,
    /// Where it was at the step before and is now (m).
    pub from: Vec3,
    pub at: Vec3,
    /// Time into the current step (s), and since it was made.
    pub into: f32,
    pub age: f32,
    /// It met the world.
    pub stopped: bool,
    /// The server's copy it stands for.
    pub claimed: Option<Entity>,
    /// That copy has moved (its first snapshots wait at the start).
    pub copy_moved: bool,
    /// The shooter (its bodies aren't the world).
    pub shooter: Option<Entity>,
}

impl PredictedItem {
    /// A projectile of `model` from `at` (m) at `velocity` (m/s).
    pub fn new(model: impl Into<String>, at: Vec3, velocity: Vec3, step: f32) -> Self {
        Self {
            model: model.into(),
            velocity,
            gravity: 0.0,
            gravity_term: 0.5,
            step: step.max(1e-3),
            from: at,
            at,
            into: 0.0,
            age: 0.0,
            stopped: false,
            claimed: None,
            copy_moved: false,
            shooter: None,
        }
    }

    /// Under `gravity` (m/s², signed) weighted `term` in each step's move.
    pub fn with_gravity(mut self, gravity: f32, term: f32) -> Self {
        self.gravity = gravity;
        self.gravity_term = term;
        self
    }

    pub fn with_shooter(mut self, shooter: Entity) -> Self {
        self.shooter = Some(shooter);
        self
    }

    /// Where it is drawn: one step behind, between its last two places.
    pub fn drawn(&self) -> Vec3 {
        self.from.lerp(self.at, (self.into / self.step).clamp(0.0, 1.0))
    }

    /// The bundle a game spawns: the prediction, its model and pose.
    pub fn bundle(self) -> impl Bundle {
        let t = Transform::from_translation(self.at).looking_to(self.velocity.normalize_or(Vec3::NEG_Z), Vec3::Y);
        (ShownItem(self.model.clone()), t, NetDrawn, self)
    }
}

/// `cl_predict_projectiles`: 1, a client draws its own projectiles from
/// the shot (games read it before spawning a `PredictedItem`).
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct OwnItemSettings {
    pub predict: u8,
}

impl OwnItemSettings {
    pub fn on(&self) -> bool {
        self.predict != 0
    }
}

/// A server copy shown by a `PredictedItem` instead (no `ShownItem`
/// meanwhile).
#[derive(Component, Clone, Copy, Debug)]
pub struct ShownByPrediction(pub Entity);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<OwnItemSettings>();
    if app.world().get_resource::<crate::console::Console>().is_some() {
        crate::console::resource_cvar::<OwnItemSettings, u8>(
            app,
            "chimera:predict_projectiles",
            "1: your own rockets, launcher shells and bolts leave the muzzle on the shot, drawn by your client \
             until the server's copy takes over; 0: drawn from the server's copy, as the original games do \
             (default).",
            |s| &mut s.predict,
        );
        app.world_mut().resource_mut::<crate::console::Console>().archive("cl_predict_projectiles");
    }
    app.add_systems(
        RunFixedMainLoop,
        (fly, hand_over)
            .chain()
            .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop)
            .after(InterpSystems::Ease)
            .after(super::weapons::DrawItems)
            .run_if(resource_equals(NetRole::Client)),
    );
}

/// A server copy `item` arriving as entity `e`: claimed by the oldest
/// unclaimed prediction of its model when it is this client's own
/// (true: don't show it).
pub(super) fn claim(
    e: Entity,
    item: &NetItem,
    me: Option<&LocalClientId>,
    predictions: &mut Query<(Entity, &mut PredictedItem)>,
    commands: &mut Commands,
) -> bool {
    let (Some(me), Some(owner)) = (me, item.owner) else { return false };
    if me.0 != owner {
        return false;
    }
    let Some((p, mut predicted)) = predictions
        .iter_mut()
        .filter(|(_, p)| p.claimed.is_none() && p.model == item.model)
        .max_by(|a, b| a.1.age.total_cmp(&b.1.age))
    else {
        return false;
    };
    predicted.claimed = Some(e);
    commands.entity(e).insert(ShownByPrediction(p));
    true
}

/// Each prediction's flight this frame: its steps due, against the map,
/// and its drawn place.
fn fly(
    time: Res<Time>,
    spatial: SpatialQuery,
    bodies: Query<&ColliderOf>,
    rigid: Query<&RigidBody>,
    mut q: Query<(&mut PredictedItem, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (mut p, mut t) in &mut q {
        p.age += dt;
        p.into += dt;
        while p.into >= p.step {
            p.into -= p.step;
            p.from = p.at;
            if p.stopped {
                continue;
            }
            let (step, g) = (p.step, Vec3::new(0.0, p.gravity, 0.0));
            let delta = p.velocity * step + p.gravity_term * g * step * step;
            p.velocity += g * step;
            let Ok(dir) = Dir3::new(delta) else { continue };
            let len = delta.length();
            let shooter = p.shooter;
            let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
            let hit = spatial.cast_ray_predicate(p.at, dir, len, true, &filter, &|c| {
                let body = bodies.get(c).map_or(c, |b| b.body);
                Some(body) != shooter && rigid.get(body).is_ok_and(|r| *r == RigidBody::Static)
            });
            match hit {
                Some(h) => {
                    p.at += dir * h.distance;
                    p.stopped = true;
                }
                None => p.at += delta,
            }
        }
        let drawn = p.drawn();
        let facing = (p.at - p.from).normalize_or(p.velocity.normalize_or(Vec3::NEG_Z));
        let pose = Transform::from_translation(drawn).looking_to(facing, Vec3::Y).with_scale(t.scale);
        if *t != pose {
            *t = pose;
        }
    }
}

/// Predictions give way to the server's copies (module doc).
fn hand_over(
    mut predictions: Query<(Entity, &mut PredictedItem)>,
    copies: Query<(&Transform, &NetItem, Option<&Snapshots<NetItem>>), With<ShownByPrediction>>,
    mut commands: Commands,
) {
    for (e, mut p) in &mut predictions {
        let Some(copy) = p.claimed else {
            if p.age > UNCLAIMED {
                commands.entity(e).despawn();
            }
            continue;
        };
        let Ok((t, item, snaps)) = copies.get(copy) else {
            // The server removed it.
            commands.entity(e).despawn();
            continue;
        };
        let samples = snaps.map(|s| s.samples());
        let still = samples.is_some_and(|s| s.len() >= 2 && s[s.len() - 1].1.origin == s[s.len() - 2].1.origin);
        if samples.is_some_and(|s| s.iter().any(|(_, v)| v.origin != s[0].1.origin)) && !still {
            p.copy_moved = true;
        }
        // At rest on the server, and drawn there (the render time caught up).
        let rested = p.copy_moved
            && still
            && samples
                .and_then(|s| s.back())
                .is_some_and(|(_, v)| t.translation.distance(Vec3::from_array(v.origin)) < 1e-3);
        let caught_up = p.stopped && t.translation.distance(p.drawn()) <= CATCH_UP;
        if rested || caught_up || p.age > LONGEST {
            commands.entity(copy).remove::<ShownByPrediction>().insert(ShownItem(item.model.clone()));
            commands.entity(e).despawn();
        }
    }
}
