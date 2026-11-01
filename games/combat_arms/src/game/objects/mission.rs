//! What the mission's event objects have told the clients so far, kept
//! as state (`CaMission`): the running timers, the caption and message
//! on show, the objective list. **Not from the original**: the original
//! client's handlers for these notices aren't in the reference
//! (world_objects.md Open question 15); the state is the notices applied
//! in order, ours, and `ui::hud::mission` draws it.
//!
//! The server applies every notice as it is made (`apply_effects`); the state
//! replicates on an entity of its own (`NetCaMission`, times as whole
//! server ticks) so a late joiner gets the running timers, the caption
//! and message still showing and the current objectives, not only the
//! notices from now on; a client writes it into its own `CaMission`
//! (`read`) on its clock. Single player and the server read their own.
//!
//! - A timer (`Notice::Timer`): on at T runs T s from the notice (the
//!   object's own run-out is T + 1 s; the display counts T), off removes
//!   it. One per object.
//! - A caption or message (`Notice::Caption`, `Message`): replaces the
//!   one showing, for `Showtime` s; `team` 0 is everyone's, else that
//!   team's. `HideCaption` clears the caption.
//! - Objectives (`Notice::Objective`, `ObjectiveRemoved`): added at the
//!   end, or changed in place by id.
//!
//! Known shortcuts: a caption's or message's text is its id until the
//! string table has it (the co-op mission file's isn't read); the state
//! isn't reset at a round start, only when a map loads; the sub-mission
//! time the original also sets from a timer isn't there.

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    commands::ObjId,
    events::{Notice, Objective},
};
use crate::{
    core::SimClock,
    net::game::{ServerTime, tick_of},
};

/// A running timer: the object, and when it reaches 0 (s, the local
/// clock).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MissionTimer {
    pub object: ObjId,
    pub name: String,
    pub ends: f64,
}

/// A caption or message on show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shown {
    /// The text's id.
    pub text: String,
    /// When it goes (s, the local clock).
    pub until: f64,
    /// 0: everyone's.
    pub team: u8,
}

/// The mission's timers, caption, message and objectives, as told.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CaMission {
    pub timers: Vec<MissionTimer>,
    pub caption: Option<Shown>,
    pub message: Option<Shown>,
    pub objectives: Vec<Objective>,
}

impl CaMission {
    /// A notice from object `object` (named `name`) at `now`.
    pub fn apply(&mut self, object: ObjId, name: &str, notice: &Notice, now: f64) {
        match notice {
            Notice::Timer { on: true, seconds } => {
                let ends = now + *seconds as f64;
                match self.timers.iter_mut().find(|t| t.object == object) {
                    Some(t) => t.ends = ends,
                    None => self.timers.push(MissionTimer {
                        object,
                        name: name.to_string(),
                        ends,
                    }),
                }
            }
            Notice::Timer { on: false, .. } => self.timers.retain(|t| t.object != object),
            Notice::Caption { id, showtime, team } => {
                self.caption = Some(Shown {
                    text: id.clone(),
                    until: now + *showtime as f64,
                    team: *team,
                });
            }
            Notice::HideCaption => self.caption = None,
            Notice::Message { id, showtime, team } => {
                self.message = Some(Shown {
                    text: id.clone(),
                    until: now + *showtime as f64,
                    team: *team,
                });
            }
            Notice::Objective(o) => match self.objectives.iter_mut().find(|x| x.id == o.id) {
                Some(x) => *x = o.clone(),
                None => self.objectives.push(o.clone()),
            },
            Notice::ObjectiveRemoved { id } => self.objectives.retain(|o| &o.id != id),
            Notice::UseLocked { .. } | Notice::Use { .. } | Notice::Fov { .. } => {}
        }
    }

    /// Drops what has run out by `now`: a caption or message past its
    /// time, a timer more than `GRACE` s past 0 (its stop notice lost).
    pub fn expire(&mut self, now: f64) {
        if self.caption.as_ref().is_some_and(|c| c.until <= now) {
            self.caption = None;
        }
        if self.message.as_ref().is_some_and(|c| c.until <= now) {
            self.message = None;
        }
        self.timers.retain(|t| t.ends + GRACE > now);
    }

    /// Nothing to show or tell.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    fn map_times(&mut self, f: impl Fn(f64) -> f64) {
        for t in &mut self.timers {
            t.ends = f(t.ends);
        }
        for s in self.caption.iter_mut().chain(self.message.iter_mut()) {
            s.until = f(s.until);
        }
    }
}

/// A timer is dropped this long after it ran out (s).
pub const GRACE: f64 = 5.0;

/// The mission state on one replicated entity (times in whole server
/// ticks).
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetCaMission(pub CaMission);

/// The server's state into `NetCaMission` (called from `net::write`,
/// after each tick), dropping what ran out.
pub(super) fn write(world: &mut World) {
    let clock = *world.resource::<SimClock>();
    world.resource_mut::<CaMission>().expire(clock.now);
    let mut state = world.resource::<CaMission>().clone();
    state.map_times(|s| tick_of(&clock, s).round());
    let state = NetCaMission(state);
    let have = world.query::<(Entity, &NetCaMission)>().iter(world).next().map(|(e, n)| (e, n == &state));
    match have {
        Some((e, false)) => {
            world.entity_mut(e).insert(state);
        }
        Some(_) => {}
        None if !state.0.is_empty() => {
            world.spawn((Name::new("Mission"), Replicated, state));
        }
        None => {}
    }
}

/// A client: the server's state into `CaMission` on this clock (called
/// from `net::read`).
pub(super) fn read(world: &mut World) {
    let Some(NetCaMission(mut state)) = world.query::<&NetCaMission>().iter(world).next().cloned() else {
        if !world.resource::<CaMission>().is_empty() {
            *world.resource_mut::<CaMission>() = CaMission::default();
        }
        return;
    };
    let Some(time) = ServerTime::of(world) else { return };
    state.map_times(|tick| time.local(tick));
    if *world.resource::<CaMission>() != state {
        world.insert_resource(state);
    }
}

/// Serving stopped: the state's entity goes.
pub(super) fn stopped_serving(world: &mut World) {
    let gone: Vec<Entity> = world.query_filtered::<Entity, With<NetCaMission>>().iter(world).collect();
    for e in gone {
        world.entity_mut(e).despawn();
    }
}
