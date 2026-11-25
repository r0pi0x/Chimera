//! Helmets (specs/dods/bodygroups.md): every DoD:S body (the twelve
//! class models) has one skin and a helmet body part with two choices,
//! the helmet (body number 0, the class scripts' `helmetgroup`) and
//! nothing (1, `hairgroup`); the team is in the model, so no skin is ever
//! chosen. A bullet that hits a character's head (`Hitgroup::Head` from a
//! weapon that shoots, `Hitscan`) knocks its helmet off on the same tick,
//! lethal or not (`DodsHelmet::off`, the server's, replicated to every
//! client); it stays off for the rest of that life, so a head shot's
//! ragdoll is bare-headed, and comes back with the next (`Dead` taken
//! off). Every machine draws a character on a DoD:S body with its helmet
//! part's choice (`show`: `map::BodyNumber`); other games' bodies are left
//! alone, and a body number set here is put back to 0 (every part's first
//! choice, as with none) when the helmet comes back or the body stops
//! being DoD:S's. `DodsHelmet` and `HelmetShown` are on every character
//! from its spawn and only change value; `BodyNumber` is put on at the
//! first helmet knocked off and then kept (docs/DESIGN.md, "ECS style").
//!
//! Known shortcuts (docs/tech-debt.md):
//! - Only bullets knock the helmet off; melee and blasts to the head are
//!   unmeasured (bodygroups.md, open question 2).
//! - The helmet doesn't fly off (the game's client throws the class's
//!   `drophelmet` model; unmeasured, open question 1).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{Hitscan, WeaponEvent, WeaponEventKind};

use super::GAME;
use crate::{
    core::{Dead, Hitgroup},
    map::{BodyModel, BodyNumber, CharacterModels},
};

/// The helmet part's choices as body numbers (the part's base is 1:
/// bodygroups.md, "The models").
pub const HELMET_ON: i32 = 0;
pub const HELMET_OFF: i32 = 1;

/// On every character: its helmet was knocked off this life (the
/// server's; sent to every client).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DodsHelmet {
    pub off: bool,
}

/// On every character, each machine's own: whether `show` set its body
/// number last, so it can put it back when the body stops being DoD:S's.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HelmetShown(pub bool);

/// The body number a DoD:S body shows: the helmet part's choice.
pub fn body_number(off: bool) -> i32 {
    if off { HELMET_OFF } else { HELMET_ON }
}

/// A bullet's hit on a head knocks its helmet off (the server's).
pub fn on_hit(mut events: MessageReader<WeaponEvent>, bullets: Query<(), With<Hitscan>>, mut helmets: Query<&mut DodsHelmet>) {
    for e in events.read() {
        let WeaponEventKind::Hit { target, hitgroup: Hitgroup::Head, .. } = e.kind else { continue };
        if bullets.contains(e.weapon)
            && let Ok(mut h) = helmets.get_mut(target)
            && !h.off
        {
            h.off = true;
        }
    }
}

/// A new life puts the helmet back (the server's).
pub fn on_respawn(mut lives: RemovedComponents<Dead>, mut helmets: Query<&mut DodsHelmet>) {
    for e in lives.read() {
        if let Ok(mut h) = helmets.get_mut(e)
            && h.off
        {
            h.off = false;
        }
    }
}

/// Whether `body` is one of DoD:S's own models (not any game's).
fn is_dods(models: Option<&CharacterModels>, body: Option<&BodyModel>) -> bool {
    let model = body.zip(models).and_then(|(b, m)| m.0.get(b.0));
    model.is_some_and(|m| m.game.is_some_and(|g| g.eq_ignore_ascii_case(GAME)))
}

/// What `show` wants of a character's body number: None, leave it; else
/// that value. A DoD:S body shows its helmet part's choice (none put on
/// for the helmet on: the first choice is the default); another body gets
/// back 0 where `show` had set one.
pub fn wanted(dods: bool, off: bool, number: Option<i32>, shown: bool) -> Option<i32> {
    if dods {
        let want = body_number(off);
        (number.unwrap_or(HELMET_ON) != want).then_some(want)
    } else {
        (shown && number.is_some_and(|n| n != HELMET_ON)).then_some(HELMET_ON)
    }
}

/// Every machine: a character on a DoD:S body shows its helmet part's
/// choice; a body number set here goes back to 0 when the body is another
/// game's (the component stays: no archetype per change).
pub fn show(
    mut characters: Query<(Entity, Option<&BodyModel>, &DodsHelmet, Option<&mut BodyNumber>, &mut HelmetShown)>,
    models: Option<Res<CharacterModels>>,
    mut commands: Commands,
) {
    for (e, body, helmet, number, mut shown) in &mut characters {
        let dods = is_dods(models.as_deref(), body);
        if let Some(want) = wanted(dods, helmet.off, number.as_deref().map(|n| n.0), shown.0) {
            match number {
                Some(mut n) => n.0 = want,
                None => {
                    commands.entity(e).insert(BodyNumber(want));
                }
            }
        }
        if shown.0 != dods {
            shown.0 = dods;
        }
    }
}

/// DoD:S's part of the network (`dods`): the helmets.
fn register(app: &mut App) {
    use bevy_replicon::prelude::*;
    app.replicate::<DodsHelmet>();
}

/// The helmets: knocked off and put back on the server, drawn everywhere.
pub fn add(app: &mut App) {
    use chimera_core::character::CharacterPartsExt;
    use chimera_net::net::parts::NetAppExt;
    app.on_every_character::<DodsHelmet>()
        .on_every_character::<HelmetShown>()
        .net_part("dods", register)
        .add_systems(
            FixedUpdate,
            (on_hit.after(crate::core::apply_damage), on_respawn)
                .after(crate::core::SimSet::Weapons)
                .run_if(crate::core::authoritative),
        )
        .add_systems(Update, show.before(crate::map::DriveAnimation));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_helmet_part_is_the_first_choice_and_off_the_second() {
        assert_eq!(body_number(false), 0);
        assert_eq!(body_number(true), 1);
        // A DoD:S body: nothing put on for the helmet on, 1 when off.
        assert_eq!(wanted(true, false, None, false), None);
        assert_eq!(wanted(true, true, None, true), Some(1));
        assert_eq!(wanted(true, false, Some(1), true), Some(0));
        // Another game's body: its own number left alone, ours put back.
        assert_eq!(wanted(false, false, Some(3), false), None);
        assert_eq!(wanted(false, true, Some(1), true), Some(0));
    }
}
