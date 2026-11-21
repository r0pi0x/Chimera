//! Movement envelopes: the body a map was built for, and the rule that fits
//! another game's character into it (docs/DESIGN.md has no older seam for
//! this: each game's movement keeps its own body).
//!
//! A `MovementEnvelope` is a game's own character as its movement collides
//! with the level, in metres: the radius (half the width), the standing
//! and crouched heights, and the step it walks up. Each game states its own
//! once (`MovementEnvelopeAppExt::add_movement_envelope`, from its movement
//! data), and a map's envelope is its game's (`core::MapGame`: the loader
//! records it; `MovementEnvelopes::for_map`). No map, no game recorded or
//! no envelope registered for that game: none, and nothing changes.
//!
//! The rule (`MovementEnvelope::within`): on any map a character's movement
//! body is no wider and no taller than the map's own character's (standing
//! and crouched) and steps at least as high. It is about the movement only:
//! the model, the hit boxes, jumps and gravity stay the character's own. On
//! a map of the character's own game the rule is the identity. Both the
//! server and every client derive it from the loaded map, so prediction
//! needs nothing new on the network.

use bevy::prelude::*;

use crate::core::MapGame;

/// How far (m) the eye stays under the top of a body the rule made lower
/// than the character's own (`eye_within`).
pub const EYE_MARGIN: f32 = 0.1;

/// The body a game's character moves with (metres; module doc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementEnvelope {
    /// Half the body's width.
    pub radius: f32,
    /// Standing height, feet to top.
    pub height: f32,
    /// Crouched height.
    pub crouch_height: f32,
    /// The highest edge it walks up without a jump.
    pub step: f32,
}

impl MovementEnvelope {
    /// This body on a map whose envelope is `map` (the rule, module doc):
    /// the narrower radius, the lower heights, the higher step. `None`:
    /// this body unchanged.
    pub fn within(self, map: Option<&MovementEnvelope>) -> MovementEnvelope {
        let Some(m) = map else { return self };
        MovementEnvelope {
            radius: self.radius.min(m.radius),
            height: self.height.min(m.height),
            crouch_height: self.crouch_height.min(m.crouch_height),
            step: self.step.max(m.step),
        }
    }

    /// A box's half widths (any unit, `scale` metres each) on this fitted
    /// body: each axis no wider than `radius` (the box fits per axis).
    pub fn fit_half_widths(&self, half: [f32; 2], scale: f32) -> [f32; 2] {
        half.map(|h| h.min(self.radius / scale))
    }

    /// An eye `eye` above the feet of a body `own_height` tall, on a body
    /// `height` tall (same unit, `scale` metres each): unchanged when the
    /// height is the own one, else kept `EYE_MARGIN` under the top.
    pub fn eye_within(eye: f32, own_height: f32, height: f32, scale: f32) -> f32 {
        if height >= own_height { eye } else { eye.min(height - EYE_MARGIN / scale) }
    }
}

/// Every game's envelope, by game id (`core::MapGame`'s).
#[derive(Resource, Clone, Debug, Default)]
pub struct MovementEnvelopes(pub Vec<(&'static str, MovementEnvelope)>);

impl MovementEnvelopes {
    /// The envelope registered for `game`.
    pub fn of(&self, game: &str) -> Option<MovementEnvelope> {
        self.0.iter().find(|(g, _)| g.eq_ignore_ascii_case(game)).map(|(_, e)| *e)
    }

    /// The loaded map's envelope: its game's (None without a game).
    pub fn for_map(&self, map: Option<&MapGame>) -> Option<MovementEnvelope> {
        self.of(&map?.0)
    }
}

/// The loaded map's envelope in a world (None: no effect).
pub fn map_envelope(world: &World) -> Option<MovementEnvelope> {
    world.get_resource::<MovementEnvelopes>()?.for_map(world.get_resource::<MapGame>())
}

/// Registering a game's envelope.
pub trait MovementEnvelopeAppExt {
    fn add_movement_envelope(&mut self, game: &'static str, envelope: MovementEnvelope) -> &mut Self;
}

impl MovementEnvelopeAppExt for App {
    fn add_movement_envelope(&mut self, game: &'static str, envelope: MovementEnvelope) -> &mut Self {
        let mut all = self.world_mut().get_resource_or_init::<MovementEnvelopes>();
        all.0.retain(|(g, _)| *g != game);
        all.0.push((game, envelope));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPARTAN: MovementEnvelope = MovementEnvelope {
        radius: 0.61,
        height: 2.13,
        crouch_height: 1.52,
        step: 0.375,
    };
    const SOURCE: MovementEnvelope = MovementEnvelope {
        radius: 0.41,
        height: 1.57,
        crouch_height: 1.14,
        step: 0.46,
    };

    #[test]
    fn the_rule_narrows_lowers_and_raises_the_step() {
        let b = SPARTAN.within(Some(&SOURCE));
        assert_eq!(b, MovementEnvelope { step: 0.46, ..SOURCE });
        // A smaller character keeps its own body and the higher step.
        let small = MovementEnvelope { radius: 0.3, height: 1.2, crouch_height: 0.9, step: 0.2 };
        assert_eq!(small.within(Some(&SPARTAN)), MovementEnvelope { step: 0.375, ..small });
        // No map envelope, or the character's own: unchanged.
        assert_eq!(SPARTAN.within(None), SPARTAN);
        assert_eq!(SPARTAN.within(Some(&SPARTAN)), SPARTAN);
    }

    #[test]
    fn a_box_fits_per_axis_and_the_eye_stays_under_a_lowered_top() {
        let b = SPARTAN.within(Some(&SOURCE));
        assert_eq!(b.fit_half_widths([1.0, 0.2], 1.0), [0.41, 0.2]);
        assert_eq!(MovementEnvelope::eye_within(1.9, 2.13, 2.13, 1.0), 1.9);
        assert!((MovementEnvelope::eye_within(1.9, 2.13, 1.57, 1.0) - 1.47).abs() < 1e-6);
        assert_eq!(MovementEnvelope::eye_within(1.2, 2.13, 1.57, 1.0), 1.2);
    }

    #[test]
    fn a_map_carries_its_games_envelope() {
        let mut app = App::new();
        app.add_movement_envelope("halo_ce", SPARTAN).add_movement_envelope("cs_source", SOURCE);
        assert_eq!(map_envelope(app.world()), None);
        app.world_mut().insert_resource(MapGame("cs_source".into()));
        assert_eq!(map_envelope(app.world()), Some(SOURCE));
        app.world_mut().insert_resource(MapGame("greybox_of_nobody".into()));
        assert_eq!(map_envelope(app.world()), None);
    }
}
