//! Slayer's own per-tick work (specs/halo_ce/multiplayer_rules.md 6.2):
//! the speed multiplier's drift back to 1.0 when an option is off, and kill
//! in order's targets (a random living enemy, chosen when a player has a
//! unit and no target or its target is out). The kill handler is
//! `scoring::slayer_kill`; the end test is the rules' (`super::decide`).
//!
//! Known shortcuts: the target's nav point ("target blue") is not drawn.

use bevy::prelude::*;

use super::{HaloGameType, HaloMatch, HaloPlayer, out_of_lives, scoring};
use crate::core::{Dead, Team};

/// A random living enemy of `hunter` other than `not` (6.2), from the
/// match's generator.
pub fn pick_target(world: &mut World, hunter: Entity, not: Option<Entity>) -> Option<Entity> {
    let r = world.resource::<HaloGameType>().0.rules;
    let my = world.get::<Team>(hunter).map_or(0, |t| t.0);
    let mut list: Vec<(u8, Entity)> = world
        .query_filtered::<(Entity, &HaloPlayer, &Team), Without<Dead>>()
        .iter(world)
        .filter(|(e, p, t)| {
            *e != hunter && Some(*e) != not && !scoring::teammates(r.team_play, t.0, my) && !out_of_lives(&r, p)
        })
        .map(|(e, p, _)| (p.slot, e))
        .collect();
    list.sort();
    if list.is_empty() {
        return None;
    }
    let mut m = world.resource_mut::<HaloMatch>();
    let i = m.roll(list.len() as u32) as usize;
    Some(list[i].1)
}

/// One tick of slayer's per-player update (exclusive).
pub fn tick(world: &mut World) {
    let r = world.resource::<HaloGameType>().0.rules;
    let Some((penalty_on, bonus_on, in_order)) = scoring::slayer_options(&r) else { return };
    let players: Vec<(Entity, bool, Option<Entity>)> = world
        .query::<(Entity, &HaloPlayer, Has<Dead>)>()
        .iter(world)
        .map(|(e, p, d)| (e, !d, p.target))
        .collect();
    for (e, alive, target) in players {
        if let Some(mut m) = world.get_mut::<super::super::movement::SpeedMultiplier>(e) {
            let to = scoring::speed_tick(m.0, penalty_on, bonus_on);
            if to != m.0 {
                m.0 = to;
            }
        }
        if !in_order || !alive {
            continue;
        }
        let out = target.is_none_or(|t| {
            world.get_entity(t).is_err() || world.get::<HaloPlayer>(t).is_none_or(|p| out_of_lives(&r, p))
        });
        if out {
            let next = pick_target(world, e, target);
            if let Some(mut p) = world.get_mut::<HaloPlayer>(e) {
                p.target = next;
            }
        }
    }
}
