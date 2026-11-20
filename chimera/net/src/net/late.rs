//! Items drawn in the past on every machine that draws
//! (specs/tf2/effects.md 11, 11.1: TF2's client draws its rockets, pipes
//! and stickies, the shooter's own too, at the interpolated time now − L).
//! A game asks for it in its weapon data (`chimera:projectile`'s
//! `interp`, `map::loose::DrawnLate`); other games' projectiles stay as
//! they were (a network client draws every item from snapshots anyway,
//! `weapons::draw_items`, but from its first snapshot on and gone when
//! the server removes it).
//!
//! - **The delay** (`LateDelay`, each frame): a network client's
//!   interpolation delay (`InterpClock::delay`); elsewhere (a single
//!   player game, the sandbox, a listen server's own view) what its
//!   settings would give: `max(cl_interp, cl_interp_ratio /
//!   cl_updaterate)` at the default update rate, in whole ticks plus one
//!   (`interp::interp_amount`: 0.12 s at TF2's 0.015 s tick).
//! - **The copy** (`map::loose::LateCopy`): an item shown as a late key
//!   isn't drawn itself (`render::loose`, the games' trails skip it); a
//!   copy with its `ShownItem` is, at where the item was `LateDelay` ago:
//!   its places sampled each frame it moved (the simulated `Transform`
//!   here, the newest `NetItem` on a client), drawn between the two
//!   around now − delay. It arrives as the item's first snapshot
//!   carries it (effects.md 11.2: on a client when it comes; here at
//!   the first snapshot time, 20 a second, after the item's birth, so
//!   0 to 4 ticks of flight out), its first place in whole units when
//!   its data asks (`whole_units`); it is **drawn at that first place
//!   while the drawn time is before its arrival** (no place behind it:
//!   the captures, Open questions 16), then between its places. The
//!   copy keeps that first place (`LateCopy::first`, where TF2 makes its
//!   trail). The model is hidden for its key's
//!   first seconds from the arrival (`DrawnLate::hidden_for`: TF2's rocket
//!   0.2 s, pipes and stickies 0.1 s; their trails emit from the arrival);
//!   the copy goes when the item is gone and the drawn time has passed
//!   that moment (a blast held as long shows as it arrives: TF2's
//!   `explosions`).
//!
//! Known shortcuts: a dedicated server makes the copies too (nothing draws
//! them); a client keys the places by when they arrived, not by
//! the server tick they are from (jitter shows); a projectile this client
//! predicts (`cl_predict_projectiles 1`, `own_items`) gets no copy; the
//! newer places are joined by straight lines (Source: a curve through
//! the three newest); the hold at the first place is fitted to captures
//! at the default interpolation and at 66 updates a second (with
//! `cl_interp 0.5` TF2 held it ~0.3 s, then drew it far behind:
//! effects.md Open questions 16), and the sandbox's snapshot times are
//! on the server's tick count at the default update rate.

use std::collections::VecDeque;

use bevy::prelude::*;

use super::{NetItem, interp::InterpSettings, own_items::PredictedItem};
use crate::{
    core::NetRole,
    map::loose::{DrawnLate, LateCopy, LateDelay, ShownItem},
};

/// A copy's places by this machine's time (s), oldest first, and when it
/// appeared and its item went.
#[derive(Component, Debug, Default, Clone)]
pub struct LatePlaces {
    pub places: VecDeque<(f64, Vec3, Quat)>,
    pub born: f64,
    pub gone: Option<f64>,
}

impl LatePlaces {
    /// A place at `t` (none when it hasn't moved).
    pub fn add(&mut self, t: f64, at: Vec3, turn: Quat) {
        if self.places.back().is_some_and(|(_, p, r)| *p == at && *r == turn) {
            return;
        }
        self.places.push_back((t, at, turn));
    }

    /// Where it is drawn at time `t`: between the places around it, the
    /// first before them, the newest after.
    pub fn at(&self, t: f64) -> Option<(Vec3, Quat)> {
        let first = self.places.front()?;
        if t <= first.0 {
            return Some((first.1, first.2));
        }
        for w in self.places.iter().zip(self.places.iter().skip(1)) {
            let ((ta, pa, ra), (tb, pb, rb)) = w;
            if t <= *tb {
                let f = ((t - ta) / (tb - ta).max(1e-9)) as f32;
                return Some((pa.lerp(*pb, f), ra.slerp(*rb, f)));
            }
        }
        self.places.back().map(|(_, p, r)| (*p, *r))
    }

    /// Places older than `t` but the last before it go.
    fn prune(&mut self, t: f64) {
        while self.places.len() > 2 && self.places[1].0 < t {
            self.places.pop_front();
        }
    }
}

/// The delay items drawn late are drawn with this frame.
fn late_delay(
    role: Option<Res<NetRole>>,
    clock: Res<super::interp::InterpClock>,
    settings: Res<InterpSettings>,
    fixed: Res<Time<Fixed>>,
    mut delay: ResMut<LateDelay>,
) {
    let d = if role.as_deref() == Some(&NetRole::Client) && clock.running {
        clock.delay as f32
    } else {
        let lerp = settings.interp.max(settings.interp_ratio / super::server::DEFAULT_UPDATERATE);
        super::interp::interp_amount(lerp as f64, fixed.timestep().as_secs_f64()) as f32
    };
    if delay.0 != d {
        delay.0 = d;
    }
}

/// A copy for each new item drawn late, as its first snapshot carries
/// it (effects.md 11.2): on a network client at once; elsewhere (a
/// single player game, the sandbox, a listen server's own view) once the
/// server's tick count reaches the next snapshot after the item's birth
/// (`DEFAULT_UPDATERATE` a second: 0 to 4 ticks of flight at TF2's 66.67
/// ticks a second). Its first place is rounded to whole units when its key
/// asks (`DrawnLate::received`).
#[allow(clippy::type_complexity)]
fn copy_late(
    time: Res<Time>,
    late: Res<DrawnLate>,
    added: Query<Entity, (Added<ShownItem>, Without<LateCopy>, Without<PredictedItem>)>,
    items: Query<(&ShownItem, &Transform, Option<&NetItem>, Option<&crate::weapon::projectile::Flyer>)>,
    (tick, fixed, role): (Option<Res<crate::core::SimTick>>, Res<Time<Fixed>>, Option<Res<NetRole>>),
    mut waiting: Local<Vec<Entity>>,
    mut commands: Commands,
) {
    for e in &added {
        if items.get(e).is_ok_and(|(shown, ..)| late.keys.contains(&shown.0)) {
            waiting.push(e);
        }
    }
    if waiting.is_empty() {
        return;
    }
    let now = time.elapsed_secs_f64();
    let tick = tick.map_or(0, |t| t.0);
    let dt = fixed.timestep().as_secs_f64();
    let client = role.as_deref() == Some(&NetRole::Client);
    let snapshot = |t: u64| (t as f64 * dt * super::server::DEFAULT_UPDATERATE as f64 + 1e-6).floor() as u64;
    waiting.retain(|&e| {
        let Ok((shown, t, net, flyer)) = items.get(e) else { return false };
        // Sent in a snapshot yet (a client's item came in one).
        if !client
            && let Some(f) = flyer
            && snapshot(tick) <= snapshot(f.born_tick)
        {
            return true;
        }
        let (at, turn) = place(t, net);
        let at = late.received(&shown.0, at);
        let mut places = LatePlaces { born: now, ..default() };
        places.add(now, at, turn);
        commands.spawn((
            Name::new(format!("Drawn late ({})", shown.0)),
            ShownItem(shown.0.clone()),
            LateCopy { of: e, owner: flyer.and_then(|f| f.owner), first: at },
            places,
            Transform::from_translation(at).with_rotation(turn),
            Visibility::Hidden,
        ));
        false
    });
}

/// Where an item is now: the simulated place, or on a client the newest
/// the server sent.
fn place(t: &Transform, net: Option<&NetItem>) -> (Vec3, Quat) {
    match net {
        Some(n) => (Vec3::from_array(n.origin), Quat::from_array(n.rotation).normalize()),
        None => (t.translation, t.rotation),
    }
}

/// Each copy where its item was `LateDelay` ago; gone after its item once
/// the drawn time passes the moment it went.
fn move_late(
    time: Res<Time>,
    delay: Res<LateDelay>,
    items: Query<(&Transform, Option<&NetItem>), Without<LateCopy>>,
    mut copies: Query<(Entity, &LateCopy, &mut LatePlaces, &mut Transform)>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    let drawn = now - delay.0 as f64;
    for (e, copy, mut places, mut t) in &mut copies {
        match items.get(copy.of) {
            Ok((it, net)) => {
                let (at, turn) = place(it, net);
                places.add(now, at, turn);
            }
            Err(_) => {
                let gone = *places.gone.get_or_insert(now);
                if drawn >= gone {
                    commands.entity(e).despawn();
                    continue;
                }
            }
        }
        places.prune(drawn);
        if let Some((at, turn)) = places.at(drawn)
            && (t.translation != at || t.rotation != turn)
        {
            t.translation = at;
            t.rotation = turn;
        }
    }
}

/// A copy's model shows after its key's hidden time
/// (`DrawnLate::hidden_for`).
fn show_late(time: Res<Time>, late: Res<DrawnLate>, mut copies: Query<(&LatePlaces, &ShownItem, &mut Visibility), With<LateCopy>>) {
    let now = time.elapsed_secs_f64();
    for (places, shown, mut v) in &mut copies {
        let want = if now - places.born >= late.hidden_for(&shown.0) as f64 { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
    }
}

/// Where the copies are placed each frame (before the particles that
/// follow them: `map::psys::StartedSystemsRun`).
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LatePlaced;

pub(super) fn plugin(app: &mut App) {
    app.world_mut().get_resource_or_init::<DrawnLate>().running = true;
    app.init_resource::<LateDelay>()
        .add_systems(
            Update,
            (late_delay, copy_late, move_late)
                .chain()
                .in_set(LatePlaced)
                .before(crate::map::psys::StartedSystemsRun),
        )
        .add_systems(
            PostUpdate,
            show_late.before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// effects.md 11.2: a rocket first received at p0 = (100, 0, 0) at
    /// 10.0 is drawn there while the drawn time is before 10.0 (no place
    /// behind it), then between its places.
    #[test]
    fn a_copy_stays_at_its_first_place_until_the_drawn_time_reaches_it() {
        let mut p = LatePlaces::default();
        p.add(10.0, Vec3::new(100.0, 0.0, 0.0), Quat::IDENTITY);
        let x = |p: &LatePlaces, t: f64| p.at(t).unwrap().0.x;
        assert_eq!(x(&p, 9.9), 100.0);
        p.add(10.05, Vec3::new(155.0, 0.0, 0.0), Quat::IDENTITY);
        assert_eq!(x(&p, 9.95), 100.0);
        assert!((x(&p, 10.025) - 127.5).abs() < 1e-2);
        // Held still: no new place, and the newest is drawn past it.
        p.add(10.5, Vec3::new(155.0, 0.0, 0.0), Quat::IDENTITY);
        assert_eq!(p.places.len(), 2);
        assert_eq!(x(&p, 11.0), 155.0);
    }
}
