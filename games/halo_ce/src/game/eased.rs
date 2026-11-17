//! Halo's 30 Hz motion drawn at the frame rate (docs/backlog.md, "Halo:
//! view models and animations at frame rate"): the body's pose, the view
//! model's pose and the objects Halo moves each Halo tick (the shown
//! shots, the authority's grenades) are drawn between their last two Halo
//! ticks by the frame's time (`map::interp::GameTicks`, Halo's clock
//! `clock`: the engine's tick counted in Halo ticks, `ticks::ratio`), for
//! display only, the way the engine draws its own ticks
//! (`map::interp::Interpolated`). At Halo's own tick (its app: 30 Hz)
//! that is the blend of the last two ticks at the frame's overstep
//! fraction, one tick behind as the engine's; under a faster engine tick
//! (64 Hz in the party apps, where a Halo tick lands on about every other
//! engine tick) the frame draws 1.44 Halo ticks behind, so the next pose is
//! always in.
//!
//! - Laid: the body's pose by `body::animate` (`HaloBody::drawn`), the
//!   view's by `view::drive` (`HaloViewModel::drawn`), each when it is
//!   made; a `HaloEased` object's transform after every tick (`sample`):
//!   the shots shown in flight (`fx::shots::HaloShown`) and, off a
//!   server (whose replication reads the transform), the grenades
//!   (`fx::objects::show_grenades`).
//! - Drawn (`ease`, `RunFixedMainLoop` after the fixed loop, with the
//!   engine's `InterpSystems::Ease`): the drawn `Animator`'s and
//!   `ViewAnimator`'s game pose (`Animator::fixed`, a blend made when first
//!   posed) and the object's `Transform`. The server's posing (`SimAnimator`,
//!   the hit volumes) keeps the tick's.
//! - Put back (`restore`, `FixedFirst`): the latest of each, so the
//!   simulation never sees a blend.
//!
//! Only where the engine's interpolation is plugged in (the client's
//! `InterpPlugin`, `Interpolation`); `cl_interpolate 0` draws the latest
//! (stepping at 30 Hz, Halo's own look, for A/B).
//!
//! Known shortcuts: the corpses' death animations, a campaign body's own
//! first-person arms (`campaign::first_person`) and the vehicles keep their
//! tick's pose.

use bevy::prelude::*;
use chimera_core::later::Later;

use super::{
    body::HaloBody,
    ticks,
    view::HaloViewModel,
};
use crate::{
    core::SimClock,
    map::{
        ViewAnimator,
        anim::{Animator, BonePose},
        interp::{GameClock, GameTicks, InterpSystems, Interpolation},
    },
};

/// Halo's tick as the engine's counts it (`ticks::ratio`).
pub fn clock(sim: &SimClock) -> GameClock {
    GameClock { per_tick: ticks::ratio(sim.delta) }
}

/// The Halo tick of what this engine tick lays.
pub fn laid(sim: &SimClock) -> u64 {
    clock(sim).laid(sim.tick)
}

/// A pose laid each Halo tick, drawn between them (module doc).
pub type PoseTicks = GameTicks<Later<Vec<BonePose>>>;

/// An object Halo moves each Halo tick, drawn between them (module doc).
#[derive(Component, Clone, Debug, Default)]
pub struct HaloEased(pub GameTicks<Transform>);

pub(super) fn plugin(app: &mut App) {
    let drawing = |i: Option<Res<Interpolation>>| i.is_some();
    app.register_required_components::<super::fx::shots::HaloShown, HaloEased>()
        .add_systems(FixedFirst, restore.in_set(InterpSystems::Restore).run_if(drawing))
        .add_systems(FixedLast, sample.in_set(InterpSystems::Sample).run_if(drawing))
        .add_systems(RunFixedMainLoop, ease.in_set(InterpSystems::Ease).run_if(drawing));
}

/// The objects' transforms at this tick (module doc).
fn sample(clock: Res<SimClock>, mut moved: Query<(&Transform, &mut HaloEased)>) {
    let tick = laid(&clock);
    for (t, mut e) in &mut moved {
        e.0.lay(tick, *t);
    }
}

/// The game time a frame draws (module doc); infinite (the latest) with
/// the interpolation off.
pub fn drawn_time(sim: &SimClock, fixed: &Time<Fixed>, settings: &Interpolation) -> f64 {
    if settings.enabled == 0 {
        return f64::INFINITY;
    }
    clock(sim).drawn(sim.tick, fixed.overstep_fraction())
}

/// The drawn poses and transforms (module doc).
pub fn ease(
    sim: Res<SimClock>,
    fixed: Res<Time<Fixed>>,
    settings: Res<Interpolation>,
    mut bodies: Query<(&HaloBody, &mut Animator)>,
    mut views: Query<(&HaloViewModel, &mut ViewAnimator)>,
    mut moved: Query<(&HaloEased, &mut Transform)>,
) {
    let at = drawn_time(&sim, &fixed, &settings);
    for (body, mut a) in &mut bodies {
        if let Some(p) = body.drawn.at(at) {
            a.fixed = Some(p);
        }
    }
    for (view, mut v) in &mut views {
        if let (Some(p), Some(a)) = (view.drawn.at(at), v.animator.as_mut()) {
            a.fixed = Some(p);
        }
    }
    for (e, mut t) in &mut moved {
        if let Some(want) = e.0.at(at)
            && *t != want
        {
            *t = want;
        }
    }
}

/// The latest of each back for the simulation (module doc).
fn restore(
    mut bodies: Query<(&HaloBody, &mut Animator)>,
    mut views: Query<(&HaloViewModel, &mut ViewAnimator)>,
    mut moved: Query<(&HaloEased, &mut Transform)>,
) {
    for (body, mut a) in &mut bodies {
        if let Some(p) = body.drawn.latest() {
            a.fixed = Some(p.clone());
        }
    }
    for (view, mut v) in &mut views {
        if let (Some(p), Some(a)) = (view.drawn.latest(), v.animator.as_mut()) {
            a.fixed = Some(p.clone());
        }
    }
    for (e, mut t) in &mut moved {
        if let Some(want) = e.0.latest()
            && *t != *want
        {
            *t = *want;
        }
    }
}
