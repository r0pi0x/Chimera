//! Where a character's held weapon shoots from as everyone else sees it,
//! the same for every game and every body (`HeldMuzzles`): the
//! `view_model::HeldMuzzle` that `render::bodies::attach_held` puts on
//! every held model it draws, at the model's own muzzle (its attachment or
//! marker: a Source model's `muzzle`, Halo's `primary trigger`), else at
//! its bone (the hand) for a model with none; a body of another game's
//! skeleton, which has no bone to hold the model on, gets it at its right
//! hand (`right_hand`). Last frame's, posed. The effects that start at
//! the muzzle in third person (tracers, flashes, a projectile's drawn
//! start) take it from here; hits stay from the eye by each game's rules.
//! The eye is the last resort (`HeldMuzzles::at_or_eye`): nothing drawn
//! for the holder (a headless server, a model still loading, a body with
//! no right hand), logged once per held model.

use bevy::platform::collections::HashSet;

use bevy::{ecs::system::SystemParam, prelude::*};

use super::{Held, view_model::HeldMuzzle};

/// A held weapon's muzzle in the world: its point and the way it fires
/// (the muzzle's +X).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Muzzle {
    pub at: Vec3,
    pub forward: Vec3,
}

/// Each holder's held weapon's muzzle (module doc).
#[derive(SystemParam)]
pub struct HeldMuzzles<'w, 's> {
    muzzles: Query<'w, 's, (&'static HeldMuzzle, &'static GlobalTransform)>,
    held: Query<'w, 's, &'static Held>,
    logged: Local<'s, HashSet<String>>,
}

impl HeldMuzzles<'_, '_> {
    /// `owner`'s held weapon's muzzle, if one is drawn for it.
    pub fn of(&self, owner: Entity) -> Option<Muzzle> {
        self.muzzles.iter().find(|(m, _)| m.owner == owner).map(|(_, g)| Muzzle {
            at: g.translation(),
            forward: (g.rotation() * Vec3::X).normalize_or(Vec3::NEG_Z),
        })
    }

    /// `owner`'s muzzle point, else `eye` (the last resort: logged once per
    /// held model).
    pub fn at_or_eye(&mut self, owner: Entity, eye: Vec3) -> Vec3 {
        if let Some(m) = self.of(owner) {
            return m.at;
        }
        if let Ok(Held(Some(key))) = self.held.get(owner)
            && self.logged.insert(key.clone())
        {
            debug!("held model {key}: no muzzle drawn for its holder; its effects start at the eye");
        }
        eye
    }
}

/// The index of the right hand among a skeleton's bone `names`, by name
/// whatever the game's convention (`ValveBiped.Bip01_R_Hand`,
/// `bip_hand_R`, `bip01 r hand`, `frame r hand`, `RightHand`): a bone
/// whose name's words are "hand" and "r" or "right" (not a finger's).
pub fn right_hand<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<usize> {
    names.into_iter().position(|name| {
        let w = words(name);
        let has = |x: &str| w.iter().any(|y| y == x);
        has("hand") && (has("r") || has("right")) && !["finger", "thumb", "index", "middle", "ring", "pinky"].iter().any(|f| w.iter().any(|y| y.starts_with(f)))
    })
}

/// A bone name's words, lower case: split at anything not a letter or
/// digit and where a lower-case letter meets an upper-case one.
fn words(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        if !c.is_ascii_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
            continue;
        }
        if c.is_ascii_uppercase() && prev_lower && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        prev_lower = c.is_ascii_lowercase();
        cur.push(c.to_ascii_lowercase());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_right_hand_is_found_in_every_games_naming() {
        for (names, want) in [
            (&["ValveBiped.Bip01_Pelvis", "ValveBiped.Bip01_L_Hand", "ValveBiped.Bip01_R_Hand", "ValveBiped.Bip01_R_Finger0"][..], Some(2)),
            (&["bip_pelvis", "bip_hand_L", "bip_index_0_R", "bip_hand_R"][..], Some(3)),
            (&["bip01 pelvis", "bip01 l hand", "bip01 r finger0", "bip01 r hand"][..], Some(3)),
            (&["frame pelvis", "frame l hand", "frame r hand"][..], Some(2)),
            (&["Hips", "LeftHand", "RightHandThumb1", "RightHand"][..], Some(3)),
            (&["root", "spine", "head"][..], None),
            (&["weapon_bone", "r_handle"][..], None),
        ] {
            assert_eq!(right_hand(names.iter().copied()), want, "{names:?}");
        }
    }
}
