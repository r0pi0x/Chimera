//! Items drawn in the past on every machine that draws
//! (specs/tf2/effects.md 11: TF2's client draws its rockets, the shooter's
//! own too, at the interpolated time now − lerp, 0.1 s with its
//! defaults). A game asks for it in its weapon data (`chimera:projectile`'s
//! `interp`, `map::loose::DrawnLate`); other games' projectiles stay as
//! they were (a network client draws every item from snapshots anyway,
//! `weapons::draw_items`, but from its first snapshot on and gone when
//! the server removes it).
//!
//! - **The delay** (`LateDelay`, each frame): a network client's
//!   interpolation delay (`InterpClock::delay`); elsewhere (a single
//!   player game, the sandbox, a listen server's own view) what its
//!   settings would give, `max(cl_interp, cl_interp_ratio /
//!   cl_updaterate)` at the default update rate (0.1 s).
//! - **The copy** (`map::loose::LateCopy`): an item shown as a late key
//!   isn't drawn itself (`render::loose`, the games' trails skip it); a
//!   copy with its `ShownItem` is, at where the item was `LateDelay` ago:
//!   its places sampled each frame it moved (the simulated `Transform`
//!   here, the newest `NetItem` on a client), drawn between the two
//!   around now − delay; before its first place's time it is held at
//!   that place (measured on real TF2, effects.md "Measured": a rocket's
//!   trail glows at the launcher from the frame after the shot, its
//!   smoke reaching back to it; it doesn't start behind the eye, as a
//!   history point one second back along the velocity would draw: ours
//!   did, and its trail streaked from behind the shooter). The model is
//!   hidden for its key's first seconds (`DrawnLate::hidden_for`: TF2's
//!   rocket 0.2 s; its trail isn't); the copy goes when the
//!   item is gone and the drawn time has passed that moment (a blast held
//!   as long shows as it arrives: TF2's `explosions`).
//!
//! Known shortcuts: a dedicated server makes the copies too (nothing draws
//! them); a client keys the places by when they arrived, not by
//! the server tick they are from (jitter shows); a projectile this client
//! predicts (`cl_predict_projectiles 1`, `own_items`) gets no copy.

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
    mut delay: ResMut<LateDelay>,
) {
    let d = if role.as_deref() == Some(&NetRole::Client) && clock.running {
        clock.delay as f32
    } else {
        settings.interp.max(settings.interp_ratio / super::server::DEFAULT_UPDATERATE)
    };
    if delay.0 != d {
        delay.0 = d;
    }
}

/// A copy for each new item drawn late.
#[allow(clippy::type_complexity)]
fn copy_late(
    time: Res<Time>,
    late: Res<DrawnLate>,
    added: Query<
        (Entity, &ShownItem, &Transform, Option<&NetItem>, Option<&crate::weapon::projectile::Flyer>),
        (Added<ShownItem>, Without<LateCopy>, Without<PredictedItem>),
    >,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    for (e, shown, t, net, flyer) in &added {
        if !late.keys.contains(&shown.0) {
            continue;
        }
        let (at, turn) = place(t, net);
        let mut places = LatePlaces { born: now, ..default() };
        places.add(now, at, turn);
        commands.spawn((
            Name::new(format!("Drawn late ({})", shown.0)),
            ShownItem(shown.0.clone()),
            LateCopy { of: e, owner: flyer.and_then(|f| f.owner) },
            places,
            Transform::from_translation(at).with_rotation(turn),
            Visibility::Hidden,
        ));
    }
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

    /// A copy is held at its launch point until the drawn time reaches
    /// it (real TF2: the rocket's trail glows at the launcher from the
    /// frame after the shot), then follows its places a delay late.
    #[test]
    fn a_copy_holds_at_its_launch_then_follows_late() {
        let mut p = LatePlaces::default();
        let v = Vec3::new(1100.0, 0.0, 0.0);
        p.add(10.0, Vec3::ZERO, Quat::IDENTITY);
        p.add(10.0 + 1.0 / 60.0, v / 60.0, Quat::IDENTITY);
        assert_eq!(p.at(10.0 - 0.1).unwrap().0, Vec3::ZERO, "held at the launch, not behind it");
        let (at, _) = p.at(10.0 + 1.0 / 120.0).unwrap();
        assert!((at.x - 1100.0 / 120.0).abs() < 1e-2, "{at}");
        // Held still: no new place, and the newest is drawn past it.
        p.add(10.5, v / 60.0, Quat::IDENTITY);
        assert_eq!(p.places.len(), 2);
        assert_eq!(p.at(11.0).unwrap().0, v / 60.0);
    }
}
