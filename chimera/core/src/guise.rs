//! What others see of a character that hides or poses as someone else,
//! for any game: TF2's Spy cloaked (invisible) or disguised (another
//! player's name, class and team), a game's own invisibility or disguise
//! later. A game's rules write `Guise` on the character (the server's,
//! sent to every client); every machine that draws reads it:
//!
//! - **Hidden** (`Guise::hidden`, 0 seen as usual to 255 not seen): the
//!   body and what it holds draw at `opacity` (the presenter's
//!   `BodyOpacity`, `chimera_present::render::tint`), its blob shadow
//!   fading with it; the character's own client keeps at least
//!   `SELF_OPACITY` of himself (a third-person view).
//! - **Posed** (`Guise::pose`, `Pose`): to the characters that are his
//!   enemies (`Guise::shown_to`) he is someone else: a name, a team, a
//!   game's class, the weapon it holds and its health. Each game draws
//!   that (its bodies, held models, HUD); his teammates and his own client
//!   see him as he is.
//! - **A fake death** (`core::Feigned`): a death that isn't one: every
//!   machine's kill feed shows it and draws a ragdoll of the body, but
//!   nothing counts it (no score, no rung, no respawn) and the character
//!   lives on.
//!
//! A character gets `Guise` the first time it hides or poses and keeps it
//! (at rest: nothing hidden, no pose), so its parts don't come and go.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::core::Team;

/// The least of himself a hidden character's own client draws (its
/// third-person view keeps him in sight).
pub const SELF_OPACITY: f32 = 0.25;

/// What others see of a character (module doc). Sent to every client.
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Guise {
    /// 0: seen as usual; 255: not seen at all.
    pub hidden: u8,
    /// Who he poses as, to his enemies.
    pub pose: Option<Pose>,
}

/// Someone a character poses as (module doc).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    /// The name shown (a real player's of `team`, when there is one).
    pub name: String,
    pub team: u8,
    /// The game's own class name (TF2's `heavy`).
    pub class: String,
    /// The weapon shown in his hands (a game's weapon id).
    pub weapon: Option<String>,
    /// Health shown, and its max (normalized as `core::Health`).
    pub health: f32,
    pub max: f32,
}

impl Guise {
    /// `hidden` for a share `f` (0 seen, 1 not seen).
    pub fn level(f: f32) -> u8 {
        (f.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    /// How opaque others draw him (1 seen as usual, 0 not at all).
    pub fn opacity(&self) -> f32 {
        1.0 - self.hidden as f32 / 255.0
    }

    /// Not seen at all.
    pub fn invisible(&self) -> bool {
        self.hidden == u8::MAX
    }

    /// The pose a viewer on `viewer` sees on a character of team `own`
    /// (`enemies`): none to his teammates.
    pub fn shown_to(&self, own: Option<&Team>, viewer: Option<&Team>) -> Option<&Pose> {
        self.pose.as_ref().filter(|_| enemies(own, viewer))
    }
}

/// Characters are enemies when their teams differ or either has none
/// (no `Team`, or team 0: a free-for-all).
pub fn enemies(a: Option<&Team>, b: Option<&Team>) -> bool {
    match (a.filter(|t| t.0 != 0), b.filter(|t| t.0 != 0)) {
        (Some(a), Some(b)) => a != b,
        _ => true,
    }
}

/// How opaque a viewer draws a character with `guise` (none: 1): `own`
/// when it is the viewer's own character (`SELF_OPACITY` at least).
pub fn drawn_opacity(guise: Option<&Guise>, own: bool) -> f32 {
    let o = guise.map_or(1.0, Guise::opacity);
    if own { o.max(SELF_OPACITY) } else { o }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pose_shows_to_enemies_only_and_hiding_fades() {
        let g = Guise { hidden: Guise::level(1.0), pose: Some(Pose { name: "Bob".into(), team: 2, ..default() }) };
        assert!(g.invisible());
        assert_eq!(drawn_opacity(Some(&g), false), 0.0);
        assert_eq!(drawn_opacity(Some(&g), true), SELF_OPACITY);
        assert!(g.shown_to(Some(&Team(1)), Some(&Team(2))).is_some());
        assert!(g.shown_to(Some(&Team(1)), Some(&Team(1))).is_none());
        assert!(g.shown_to(None, None).is_some(), "a free-for-all: everyone else");
        assert!(g.shown_to(Some(&Team(0)), Some(&Team(0))).is_some(), "team 0 is none");
        assert_eq!(Guise::level(0.5), 128);
    }
}
