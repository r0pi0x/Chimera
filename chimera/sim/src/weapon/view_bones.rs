//! Bones a weapon's data turns in its holder's first-person view model on
//! top of the animation (any game; `[[anim.procedural]]`,
//! `data::ProceduralBoneData`, docs/weapon-data.md): each round fired (a
//! shown `WeaponEvent` `Fired`) turns the bone `per_shot` degrees about
//! its own z axis over `time` seconds, eased in and out (`ease`:
//! smoothstep, slow at both ends, fastest halfway), from the frame after
//! the shot. Turns add up: a round fired inside a turn finishes the last
//! at once and starts its own. Nothing else turns it (draws, idles,
//! reloads), and only the view model does (`map::ViewAnimator::
//! bone_rolls`): the held model others see never turns. The angle starts
//! at 0 each time the weapon is drawn. TF2's Grenade Launcher's drum
//! (specs/tf2/weapons_more.md 9.6: one chamber, 60 degrees, a shot).
//!
//! Run each frame after the view-model driver (`view_anim::drive`, which
//! shows the model and clears its rolls when another shows).

use bevy::prelude::*;

use super::{Inventory, Weapon, WeaponEvent, WeaponEventKind, data::WeaponCatalog};
use crate::map::{ViewAnimator, ViewModels};

/// A character's procedural turns: the weapon they belong to, its rounds
/// fired since it was drawn and when the last one was (`Time` elapsed, s).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ProceduralTurns {
    pub weapon: Option<Entity>,
    pub rounds: u32,
    pub last: f64,
}

/// Smoothstep: 0 at 0, 1 at 1, flat at both ends.
pub fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The angle (radians, within one turn) after `rounds` rounds, the last
/// `since` seconds ago, of `per_shot` degrees over `time` seconds each.
pub fn turned(per_shot: f32, time: f32, rounds: u32, since: f32) -> f32 {
    if rounds == 0 {
        return 0.0;
    }
    let part = if time <= 0.0 { 1.0 } else { ease(since / time) };
    ((rounds - 1) as f32 + part) * per_shot.to_radians() % std::f32::consts::TAU
}

/// Each holder's procedural bones from its active weapon's data (module
/// doc).
#[allow(clippy::type_complexity)]
pub fn turn(
    time: Res<Time>,
    models: Option<Res<ViewModels>>,
    catalog: Option<Res<WeaponCatalog>>,
    mut holders: Query<(Entity, &Inventory, &mut ViewAnimator, Option<&mut ProceduralTurns>)>,
    weapons: Query<&Weapon>,
    mut events: MessageReader<WeaponEvent>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    let fired: Vec<(Entity, Entity)> =
        events.read().filter(|e| e.shown() && matches!(e.kind, WeaponEventKind::Fired { .. })).map(|e| (e.owner, e.weapon)).collect();
    for (e, inv, mut view, turns) in &mut holders {
        let rules = inv
            .active
            .and_then(|w| Some((w, catalog.as_ref()?.get(weapons.get(w).ok()?.id)?)))
            .filter(|(_, d)| !d.anim.procedural.is_empty());
        let Some((w, data)) = rules else { continue };
        let mut t = turns.as_deref().copied().unwrap_or_default();
        if t.weapon != Some(w) {
            t = ProceduralTurns { weapon: Some(w), rounds: 0, last: now };
        }
        for _ in fired.iter().filter(|(o, g)| *o == e && *g == w) {
            t.rounds += 1;
            t.last = now;
        }
        match turns {
            Some(mut old) => {
                old.set_if_neq(t);
            }
            None => {
                commands.entity(e).insert(t);
            }
        }
        let Some(model) = view.key.as_deref().and_then(|k| models.as_ref()?.get(k)) else { continue };
        let mut rolls = view.bone_rolls.clone();
        for rule in &data.anim.procedural {
            let Some(bone) = model.bones.iter().position(|b| b.name.eq_ignore_ascii_case(&rule.bone)) else { continue };
            let angle = turned(rule.per_shot, rule.time, t.rounds, (now - t.last) as f32);
            match rolls.iter_mut().find(|(b, _)| *b == bone) {
                Some(r) => r.1 = angle,
                None => rolls.push((bone, angle)),
            }
        }
        if view.bone_rolls != rolls {
            view.bone_rolls = rolls;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_round_turns_one_step_smoothly() {
        assert_eq!(turned(60.0, 0.14, 0, 1.0), 0.0, "nothing before a shot");
        assert_eq!(turned(60.0, 0.14, 1, 0.0), 0.0, "not yet in the shot's frame");
        let half = turned(60.0, 0.14, 1, 0.07);
        assert!((half - 30f32.to_radians()).abs() < 1e-5, "halfway at half time: {half}");
        assert!(turned(60.0, 0.14, 1, 0.0167) < 3f32.to_radians(), "slow at first");
        assert!((turned(60.0, 0.14, 1, 0.2) - 60f32.to_radians()).abs() < 1e-5, "done");
        assert!((turned(60.0, 0.14, 2, 0.2) - 120f32.to_radians()).abs() < 1e-5, "two rounds, two steps");
    }
}
