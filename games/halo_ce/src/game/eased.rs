//! Halo's 30 Hz motion drawn at the frame rate (docs/backlog.md, "Halo:
//! view models and animations at frame rate"), for display only: the
//! simulation keeps Halo's tick, its poses and positions.
//!
//! **Animations, no trail** (`map::interp::PoseStream`): a playing
//! animation's keyframes are data, so each pose is drawn at the frame's
//! own time on Halo's clock (`clock`: the engine's tick counted in Halo
//! ticks, `ticks::ratio`; `GameClock::now`, not behind), between the
//! tick's pose and the poses the same animation makes on its next ticks
//! with the same inputs (predicted when first drawn: `GameClock::ahead`
//! of them, by a clone of the animation's state ticked on). A tick other
//! than predicted (input that drifted, a state changed into with the
//! game's own blend) cross-fades from the old prediction over one Halo
//! tick; an event (a shot, a melee, a hit, a throw, a weapon's cue) shows
//! at once. Laid by what makes each pose: the body's (`body::animate`,
//! `HaloBody::drawn`), the first-person view's (`view::drive`,
//! `HaloViewModel::drawn`), a campaign body's own first-person arm
//! (`campaign::first_person::drive`, `Playing::drawn`) and a corpse's
//! (`body::death::play_corpses`, `CorpseState::drawn`).
//!
//! **Positions, eased** (`HaloEased`, `map::interp::GameTicks`): what the
//! simulation decides from input or physics is drawn between its last two
//! Halo ticks, `GameClock::lag` behind (one tick at Halo's own 30 Hz, the
//! engine's own lag; 1.44 Halo ticks under a 64 Hz engine tick, so the
//! next is always in): the shots shown in flight (`fx::shots::HaloShown`),
//! the vehicles (`vehicles::drawn_on_halos_clock`),
//! off a server (whose replication reads the transform) the grenades
//! (`fx::objects::show_grenades`), and a corpse's fall. A vehicle's wheel
//! spin, steering and suspension (`vehicles::look::VehicleLook::drawn`)
//! are eased the same way where the prop is posed (`look::pose`).
//!
//! - Drawn (`ease`, `RunFixedMainLoop` after the fixed loop, with the
//!   engine's `InterpSystems::Ease`): the drawn `Animator`'s and
//!   `ViewAnimator`'s game pose (`Animator::fixed`) and the object's
//!   `Transform`. The server's posing (`SimAnimator`, the hit volumes)
//!   keeps the tick's.
//! - Put back (`restore`, `FixedFirst`): the latest of each, so the
//!   simulation never sees a blend.
//!
//! Only where the engine's interpolation is plugged in (the client's
//! `InterpPlugin`, `Interpolation`); the predictions are only made there
//! (`drawing`), and only when first drawn. `cl_interpolate 0` draws the
//! latest (stepping at 30 Hz, Halo's own look, for A/B).
//!
//! A vehicle where it is simulated is one of these objects too (its
//! `HaloEased` with the engine's `DrawnByGame`, so the engine's
//! `Interpolated` neither eases nor restores it): smooth under a party
//! app's 64 Hz engine tick, where easing engine ticks stepped on those
//! without a Halo tick. Its riders are drawn with it by the engine
//! (`core::Carried`, `map::interp`'s `ease_carried`), and the local
//! rider's third-person seat camera is made each frame from the vehicle
//! as drawn (`vehicles::ride_drawn`).

use bevy::prelude::*;
use chimera_core::later::Later;

use super::{
    body::{HaloBody, death::CorpseState},
    campaign::first_person::BodyViewPlay,
    ticks,
    view::HaloViewModel,
};
use crate::{
    core::SimClock,
    map::{
        ViewAnimator,
        anim::Animator,
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

/// How many Halo ticks a pose's prediction runs on (`GameClock::ahead`).
pub fn ahead(sim: &SimClock) -> usize {
    clock(sim).ahead()
}

/// Whether poses are drawn here (the engine's interpolation is plugged
/// in): only then are the predictions made.
pub fn drawing(i: &Option<Res<Interpolation>>) -> bool {
    i.is_some()
}

/// A pose made each Halo tick, drawn at the frame's time (module doc).
pub type PoseStream = crate::map::interp::PoseStream;

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

/// The Halo time a frame draws eased values at (`GameClock::drawn`, behind
/// by the lag); infinite (the latest) with the interpolation off.
pub fn drawn_time(sim: &SimClock, fixed: &Time<Fixed>, settings: &Interpolation) -> f64 {
    if settings.enabled == 0 {
        return f64::INFINITY;
    }
    clock(sim).drawn(sim.tick, fixed.overstep_fraction())
}

/// The Halo time a frame samples animations at (`GameClock::now`, its own
/// time); infinite (the latest) with the interpolation off.
pub fn pose_time(sim: &SimClock, fixed: &Time<Fixed>, settings: &Interpolation) -> f64 {
    if settings.enabled == 0 {
        return f64::INFINITY;
    }
    clock(sim).now(sim.tick, fixed.overstep_fraction())
}

/// The drawn poses and transforms (module doc).
#[allow(clippy::type_complexity)]
pub fn ease(
    sim: Res<SimClock>,
    fixed: Res<Time<Fixed>>,
    settings: Res<Interpolation>,
    mut bodies: Query<(&mut HaloBody, &mut Animator), Without<CorpseState>>,
    mut corpses: Query<(&mut CorpseState, &mut Animator), Without<HaloBody>>,
    mut views: Query<(Entity, &mut HaloViewModel, &mut ViewAnimator)>,
    mut arms: Option<ResMut<BodyViewPlay>>,
    mut moved: Query<(&HaloEased, &mut Transform)>,
) {
    let now = pose_time(&sim, &fixed, &settings);
    for (mut body, mut a) in &mut bodies {
        if let Some(p) = body.drawn.at(now) {
            a.fixed = Some(p);
        }
    }
    for (mut corpse, mut a) in &mut corpses {
        if let Some(p) = corpse.drawn.at(now) {
            a.fixed = Some(p);
        }
    }
    for (e, mut view, mut v) in &mut views {
        // A body's own arm (`campaign::first_person`) is posed by its play.
        let stream = match arms.as_deref_mut().and_then(|a| a.0.get_mut(&e)) {
            Some(arm) => &mut arm.drawn,
            None => &mut view.drawn,
        };
        if let (Some(p), Some(a)) = (stream.at(now), v.animator.as_mut()) {
            a.fixed = Some(p);
        }
    }
    let at = drawn_time(&sim, &fixed, &settings);
    for (e, mut t) in &mut moved {
        if let Some(want) = e.0.at(at)
            && *t != want
        {
            *t = want;
        }
    }
}

/// The latest of each back for the simulation (module doc).
#[allow(clippy::type_complexity)]
fn restore(
    mut bodies: Query<(&HaloBody, &mut Animator), Without<CorpseState>>,
    mut corpses: Query<(&CorpseState, &mut Animator), Without<HaloBody>>,
    mut views: Query<(Entity, &HaloViewModel, &mut ViewAnimator)>,
    arms: Option<Res<BodyViewPlay>>,
    mut moved: Query<(&HaloEased, &mut Transform)>,
) {
    let back = |a: &mut Animator, p: Option<&Later<Vec<crate::map::anim::BonePose>>>| {
        if let Some(p) = p {
            a.fixed = Some(p.clone());
        }
    };
    for (body, mut a) in &mut bodies {
        back(&mut a, body.drawn.latest());
    }
    for (corpse, mut a) in &mut corpses {
        back(&mut a, corpse.drawn.latest());
    }
    for (e, view, mut v) in &mut views {
        let stream = arms.as_deref().and_then(|a| a.0.get(&e)).map_or(&view.drawn, |arm| &arm.drawn);
        if let Some(a) = v.animator.as_mut() {
            back(a, stream.latest());
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
