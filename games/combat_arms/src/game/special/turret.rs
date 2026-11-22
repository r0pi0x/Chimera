//! The auto turret as it is drawn (specs/combat_arms/special_weapons.md
//! 6.7 and its Follow-up (2026-11-21), T2-T3): three models from one
//! state, `CaTurretShots` (the server's, replicated).
//!
//! - In flight and on the floor until its unfold starts: the object's own
//!   drawn model (`DeviceParams::shown_model`), the base in its first
//!   pose (`Lower_throw`, the closed case), with its flare.
//! - Unfolding (`TurretPhase::Unfolding`): the base's `Lower_set` as a
//!   flip-book of still models (`unfold_keys`: one every `UNFOLD_STEP`,
//!   posed by `characters::models::turret_models` with the pieces its
//!   `PIECE_VISIBLE 0` keys hid by then: the case at 1766 ms), played on
//!   this machine's clock from the frame it first sees the unfold (a
//!   machine that joins later starts it then; Open question 17); the
//!   object's own meshes are hidden.
//! - Ready: the base's last frame with the case and the animated head and
//!   gun hidden, and the separate head (`TurretParams::head_model`, at
//!   `head_at`, turned about the vertical to `CaTurretShots::head`) and gun
//!   (`gun_model` at `gun_at`, turned to `gun`), each with the base's
//!   frame; the gun's `fire_anim` (`Upper_fire`) as a flip-book restarted
//!   by each new shot (`gun_keys`).
//!
//! Its zone effect (`zone`) is a looping group on it from its unfold,
//! drawn for its owner's team only.
//!
//! Each part is a child of the object with a `ShownItem` of its key; a
//! shown item is drawn centred on its bounds (`map::loose::LooseAsset`),
//! so each part is moved back by its own centre to put its model's origin
//! where the base's is. Without the models (no install, or a machine that
//! draws nothing: no `LooseAssets`) nothing here is made and the object's
//! own model stays.
//!
//! Known shortcuts: the unfold is a flip-book at `UNFOLD_STEP` (20 frames a
//! second), not an interpolated pose; the head and gun are placed at the
//! data's points (the spec's, `head_at`, `gun_at`), not read from the
//! base's nodes.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{DEVICE, DeviceParams, PROJECTILE_KEY, TurretParams};
use super::super::effects::attached::{AttachedGroup, CaAttachedGroups};
use crate::map::loose::{LooseAssets, ShownItem};

/// The time between the unfold's still frames, s.
pub const UNFOLD_STEP: f32 = 0.05;
/// The time between the gun's fire frames, s.
pub const FIRE_STEP: f32 = 1.0 / 30.0;

/// Where a turret is in its life (T3), as the server says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurretPhase {
    /// Thrown, or landed with its unfold not started yet.
    #[default]
    Closed,
    /// `Lower_set` plays (T3 4): no aim, no fire.
    Unfolding,
    /// Head and gun shown; it may fire (T3 5).
    Ready,
}

/// A turret model's drawn keys (`characters::models::turret_models` makes
/// them, `draw` shows them): one per unfold frame (the last: the ready
/// base), the head, one per gun frame (the first: idle).
#[derive(Clone, Debug, PartialEq)]
pub struct TurretKeys {
    pub unfold: Vec<String>,
    pub ready: String,
    pub head: String,
    pub gun: Vec<String>,
}

impl TurretKeys {
    /// The keys for device `d` (None: it has no base, head or gun model).
    pub fn of(d: &DeviceParams) -> Option<Self> {
        let t = d.turret.as_ref()?;
        let base = d.model.as_ref()?;
        t.head_model.as_ref()?;
        t.gun_model.as_ref()?;
        let stem = format!("{PROJECTILE_KEY}{base}|{:?}|{}#turret", d.model_scale, d.skins.join(","));
        Some(Self {
            unfold: (0..unfold_frames(t)).map(|i| format!("{stem}:unfold{i}")).collect(),
            ready: format!("{stem}:ready"),
            head: format!("{stem}:head"),
            gun: (0..gun_frames(t)).map(|i| format!("{stem}:gun{i}")).collect(),
        })
    }
}

/// How many unfold frames: one every `UNFOLD_STEP` from 0 while short of
/// the animation's end (the ready frame is the end).
pub fn unfold_frames(t: &TurretParams) -> usize {
    ((t.setup / UNFOLD_STEP - 1e-4).ceil() as usize).max(1)
}

/// The unfold frame's time, ms.
pub fn unfold_time(i: usize) -> f32 {
    i as f32 * UNFOLD_STEP * 1000.0
}

/// How many gun frames: idle, then the fire animation one every
/// `FIRE_STEP` up to its end.
pub fn gun_frames(t: &TurretParams) -> usize {
    1 + ((t.fire_length / FIRE_STEP - 1e-4).ceil() as usize).max(1)
}

/// The gun frame's time in its fire animation, ms (frame 0: idle).
pub fn gun_time(i: usize, t: &TurretParams) -> Option<f32> {
    (i > 0).then(|| ((i - 1) as f32 * FIRE_STEP * 1000.0).min(t.fire_length * 1000.0))
}

/// The unfold frame shown `elapsed` s into the unfold.
pub fn unfold_frame(elapsed: f64, frames: usize) -> usize {
    ((elapsed.max(0.0) / UNFOLD_STEP as f64) as usize).min(frames.saturating_sub(1))
}

/// The gun frame shown `since` s after the last shot (None: no shot yet).
pub fn gun_frame(since: Option<f64>, frames: usize) -> usize {
    match since {
        Some(s) if s >= 0.0 => {
            let i = 1 + (s / FIRE_STEP as f64) as usize;
            if i >= frames { 0 } else { i }
        }
        _ => 0,
    }
}

/// A point of the data in LithTech axes (x right, y up, z ahead), u, in
/// the drawn frame of a model facing −Z, metres.
pub fn local(p: [f32; 3], unit: f32) -> Vec3 {
    Vec3::new(p[0], p[1], -p[2]) * unit
}

/// The world point `p` (LithTech axes, u) of a model at `at` (u) facing
/// `forward` (horizontal or not, unit) with up as square to it as world
/// up allows: right, up and forward from it.
pub fn place(at: Vec3, forward: Vec3, p: [f32; 3]) -> Vec3 {
    let f = forward.normalize_or(Vec3::NEG_Z);
    let right = f.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = right.cross(f).normalize_or(Vec3::Y);
    at + right * p[0] + up * p[1] + f * p[2]
}

/// Whether ray `serial` of the server's turrets carries the tracer of a
/// `TracerFX` whose `Frequency` is `frequency` (0 and 1: every ray; T3 7:
/// one count for every turret).
pub fn tracer(serial: u32, frequency: u32) -> bool {
    frequency <= 1 || serial % frequency == 0
}

/// The parts made for a turret here (`draw`), and its clocks.
#[derive(Component, Debug)]
pub struct TurretParts {
    pub keys: TurretKeys,
    pub unfold: Vec<Entity>,
    pub ready: Entity,
    pub head: Entity,
    pub gun: Vec<Entity>,
    /// This machine's time when it saw the unfold start, and the last shot.
    pub unfold_seen: Option<f64>,
    pub shots: u32,
    pub shot_seen: Option<f64>,
    /// The parts' own centres (`LooseAsset`'s), by entity.
    pub centres: Vec<(Entity, Vec3)>,
}

/// A part of a turret (`TurretParts`).
#[derive(Component, Clone, Copy, Debug)]
pub struct TurretPart;

/// Make and show each turret's parts (the module doc).
#[allow(clippy::type_complexity)]
pub fn draw(
    time: Res<Time>,
    assets: Option<Res<LooseAssets>>,
    catalog: Option<Res<crate::weapon::data::WeaponCatalog>>,
    mut turrets: Query<(Entity, &super::CaTurretShots, &Transform, Option<&mut TurretParts>, Option<&Children>)>,
    mut parts: Query<(&mut Visibility, &mut Transform), (With<TurretPart>, Without<super::CaTurretShots>)>,
    mut hidden: Query<&mut Visibility, (Without<TurretPart>, Without<super::CaTurretShots>)>,
    mut commands: Commands,
) {
    let Some(assets) = assets else { return };
    let now = time.elapsed_secs_f64();
    for (e, s, at, have, children) in &mut turrets {
        let Some(mut p) = have else {
            if s.phase == TurretPhase::Closed {
                continue;
            }
            let Some(d) = catalog.as_ref().and_then(|c| c.get(&s.weapon)).and_then(|w| w.hook::<DeviceParams>(DEVICE)) else {
                continue;
            };
            let Some(keys) = TurretKeys::of(&d) else { continue };
            let all: Vec<&String> = keys.unfold.iter().chain([&keys.ready, &keys.head]).chain(&keys.gun).collect();
            if !all.iter().all(|k| assets.0.contains_key(*k)) {
                continue;
            }
            let unit = catalog.as_ref().and_then(|c| c.get(&s.weapon)).map_or(1.0, |w| w.units.length);
            let t = d.turret.as_ref().expect("a turret's keys");
            let mut centres = Vec::new();
            let mut part = |key: &String, offset: Vec3, commands: &mut Commands| {
                // A holder whose visibility is ours: the drawn item under
                // it gets its own when its meshes are added.
                let c = -assets.0[key].frame.translation;
                let id = commands
                    .spawn((
                        Name::new("turret part"),
                        TurretPart,
                        Transform::from_translation(offset + c),
                        Visibility::Hidden,
                        ChildOf(e),
                    ))
                    .id();
                commands.spawn((ShownItem(key.clone()), Transform::IDENTITY, ChildOf(id)));
                centres.push((id, c));
                id
            };
            let unfold: Vec<Entity> = keys.unfold.iter().map(|k| part(k, Vec3::ZERO, &mut commands)).collect();
            let ready = part(&keys.ready, Vec3::ZERO, &mut commands);
            let head = part(&keys.head, local(t.head_at, unit), &mut commands);
            let gun: Vec<Entity> = keys.gun.iter().map(|k| part(k, local(t.gun_at, unit), &mut commands)).collect();
            commands.entity(e).insert(TurretParts {
                keys,
                unfold,
                ready,
                head,
                gun,
                unfold_seen: (s.phase == TurretPhase::Unfolding).then_some(now),
                shots: s.shots,
                shot_seen: None,
                centres,
            });
            continue;
        };
        // The object's own model (the closed case: its drawn item's
        // children, all but the parts) goes when its parts take over.
        for c in children.into_iter().flatten() {
            if let Ok(mut v) = hidden.get_mut(*c) {
                v.set_if_neq(Visibility::Hidden);
            }
        }
        if s.phase == TurretPhase::Unfolding && p.unfold_seen.is_none() {
            p.unfold_seen = Some(now);
        }
        if s.shots != p.shots {
            p.shots = s.shots;
            p.shot_seen = Some(now);
        }
        let ready = s.phase == TurretPhase::Ready;
        let shown_unfold = (!ready).then(|| unfold_frame(now - p.unfold_seen.unwrap_or(now), p.unfold.len()));
        let shown_gun = gun_frame(p.shot_seen.map(|t| now - t), p.gun.len());
        let turn = at.rotation.inverse();
        let head = turn * super::facing(Vec3::from(s.head).normalize_or(Vec3::NEG_Z));
        let gun = turn * super::facing(Vec3::from(s.gun).normalize_or(Vec3::NEG_Z));
        let show = |mut v: Mut<Visibility>, on: bool| {
            v.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
        };
        for (i, u) in p.unfold.iter().enumerate() {
            if let Ok((v, _)) = parts.get_mut(*u) {
                show(v, shown_unfold == Some(i));
            }
        }
        if let Ok((v, _)) = parts.get_mut(p.ready) {
            show(v, ready);
        }
        let centre = |id: Entity| p.centres.iter().find(|(c, _)| *c == id).map_or(Vec3::ZERO, |(_, c)| *c);
        if let Ok((v, mut t)) = parts.get_mut(p.head) {
            show(v, ready);
            let pivot = t.translation - t.rotation * centre(p.head);
            set_turn(&mut t, pivot, head, centre(p.head));
        }
        for (i, g) in p.gun.iter().enumerate() {
            if let Ok((v, mut t)) = parts.get_mut(*g) {
                show(v, ready && i == shown_gun);
                let pivot = t.translation - t.rotation * centre(*g);
                set_turn(&mut t, pivot, gun, centre(*g));
            }
        }
    }
}

/// A turret's zone effect (`DeviceParams::zone_fx`, `Ind_Mine_SH`): a
/// looping group on it from its unfold, drawn for its owner's team only,
/// following him if he changes team (6.0, T3 3; team 0, none: nobody's).
pub fn zone(
    turrets: Query<(Entity, &super::CaTurretShots, Option<&CaAttachedGroups>)>,
    me: Query<&crate::core::Team, With<crate::core::LocalPlayer>>,
    catalog: Option<Res<crate::weapon::data::WeaponCatalog>>,
    mut commands: Commands,
) {
    let mine = me.iter().next().map_or(0, |t| t.0);
    for (e, s, groups) in &turrets {
        let Some(fx) = catalog
            .as_ref()
            .and_then(|c| c.get(&s.weapon))
            .and_then(|w| w.hook::<DeviceParams>(DEVICE))
            .map(|d| d.zone_fx)
            .filter(|f| !f.is_empty())
        else {
            continue;
        };
        let want = mine != 0 && s.team == mine && s.phase != TurretPhase::Closed;
        let has = groups.is_some_and(|g| g.0.iter().any(|a| a.group == fx));
        if want == has {
            continue;
        }
        let mut list = groups.map(|g| g.0.clone()).unwrap_or_default();
        if want {
            list.push(AttachedGroup {
                group: fx,
                looping: true,
                smooth: false,
                third_person: false,
            });
        } else {
            list.retain(|a| a.group != fx);
        }
        commands.entity(e).insert(CaAttachedGroups(list));
    }
}

/// A part turned by `turn` about its model's origin at `pivot` (its own
/// centre `c` moved back after the turn).
fn set_turn(t: &mut Transform, pivot: Vec3, turn: Quat, c: Vec3) {
    let new = Transform::from_translation(pivot + turn * c).with_rotation(turn);
    if *t != new {
        *t = new;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flip_books_run_on_their_steps() {
        assert_eq!(unfold_frame(0.0, 57), 0);
        assert_eq!(unfold_frame(1.766, 57), 35);
        assert_eq!(unfold_frame(9.0, 57), 56);
        assert_eq!(gun_frame(None, 6), 0);
        assert_eq!(gun_frame(Some(0.0), 6), 1);
        assert_eq!(gun_frame(Some(0.14), 6), 5);
        assert_eq!(gun_frame(Some(0.2), 6), 0, "idle after the fire animation");
    }

    #[test]
    fn a_point_on_a_turned_model() {
        // Facing +X: its right is -Z... no: facing +X, right is +X x Y = +Z.
        let p = place(Vec3::ZERO, Vec3::X, [1.0, 2.0, 3.0]);
        assert!((p - Vec3::new(3.0, 2.0, 1.0)).length() < 1e-5, "{p}");
        let p = place(Vec3::ZERO, Vec3::NEG_Z, [1.0, 2.0, 3.0]);
        assert!((p - Vec3::new(1.0, 2.0, -3.0)).length() < 1e-5, "{p}");
        assert_eq!(local([1.0, 2.0, 3.0], 1.0), Vec3::new(1.0, 2.0, -3.0));
    }
}
