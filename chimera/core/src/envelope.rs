//! Movement envelopes: the body a map was built for, and the rule that fits
//! another game's character into it (docs/DESIGN.md has no older seam for
//! this: each game's movement keeps its own body).
//!
//! A `MovementEnvelope` is a game's own character as its movement collides
//! with the level, in metres: the radius (half the width), the standing
//! and crouched heights, the step it walks up, and where its eye sits
//! standing and crouched (a game's eye may be above its own box: CS:S's,
//! 64 u over a 62 u box, specs/cs_source/movement.md, "Hulls, eyes and
//! ducking"). Each game states its own
//! once (`MovementEnvelopeAppExt::add_movement_envelope`, from its movement
//! data), and a map's envelope is its game's (`core::MapGame`: the loader
//! records it; `MovementEnvelopes::for_map`). No map, no game recorded or
//! no envelope registered for that game: none, and nothing changes.
//!
//! The rule (`MovementEnvelope::within`): on any map a character's movement
//! body is no wider and no taller than the map's own character's (standing
//! and crouched) and steps at least as high. The heights are the boxes the
//! map's game moves with (the clearance its players need), not its eye:
//! a character the rule lowers keeps its eye `EYE_MARGIN` under its new
//! top or at the map's own character's eye, whichever is higher, and never
//! above its own (so it sees no lower than the map's own players do, unless
//! its own eye is lower). It is about the movement only:
//! the model, the hit boxes, jumps and gravity stay the character's own. On
//! a map of the character's own game the rule is the identity. Both the
//! server and every client derive it from the loaded map, so prediction
//! needs nothing new on the network.

use bevy::prelude::*;

use crate::core::MapGame;

/// How far (m) the eye stays under the top of a body the rule made lower
/// than the character's own, unless the map's own character's eye is
/// higher (`MovementEnvelope::within`).
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
    /// The eye above the feet, standing (may be above `height`: CS:S's).
    pub eye: f32,
    /// The eye above the feet, crouched.
    pub crouch_eye: f32,
}

impl MovementEnvelope {
    /// This body on a map whose envelope is `map` (the rule, module doc):
    /// the narrower radius, the lower heights, the higher step, and the eyes
    /// of a lowered body (`fitted_eye`). `None`: this body unchanged.
    pub fn within(self, map: Option<&MovementEnvelope>) -> MovementEnvelope {
        let Some(m) = map else { return self };
        let height = self.height.min(m.height);
        let crouch_height = self.crouch_height.min(m.crouch_height);
        let eye = Self::fitted_eye(self.eye, self.height, height, m.eye);
        let crouch_eye = Self::fitted_eye(self.crouch_eye, self.crouch_height, crouch_height, m.crouch_eye);
        MovementEnvelope {
            radius: self.radius.min(m.radius),
            height,
            crouch_height,
            step: self.step.max(m.step),
            eye,
            crouch_eye: if crouch_height < self.crouch_height { crouch_eye.min(eye) } else { crouch_eye },
        }
    }

    /// An eye `eye` above the feet of a body `own_height` tall, on a body
    /// `height` tall whose map's own character has its eye at `map_eye`
    /// (metres): unchanged when the height is the own one, else `EYE_MARGIN`
    /// under the top or the map's eye, whichever is higher, never above
    /// its own.
    pub fn fitted_eye(eye: f32, own_height: f32, height: f32, map_eye: f32) -> f32 {
        if height >= own_height { eye } else { eye.min((height - EYE_MARGIN).max(map_eye)) }
    }

    /// A box's half widths (any unit, `scale` metres each) on this fitted
    /// body: each axis no wider than `radius` (the box fits per axis).
    pub fn fit_half_widths(&self, half: [f32; 2], scale: f32) -> [f32; 2] {
        half.map(|h| h.min(self.radius / scale))
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

    const U: f32 = 0.0254;
    const SPARTAN: MovementEnvelope = MovementEnvelope {
        radius: 0.61,
        height: 2.13,
        crouch_height: 1.52,
        step: 0.375,
        eye: 1.89,
        crouch_eye: 1.07,
    };
    /// CS:S's own (specs/cs_source/movement.md, "Hulls, eyes and ducking"):
    /// a 62 u box with its eye at 64, 45 ducked with the eye at 47.
    const CSS: MovementEnvelope = MovementEnvelope {
        radius: 16.0 * U,
        height: 62.0 * U,
        crouch_height: 45.0 * U,
        step: 18.0 * U,
        eye: 64.0 * U,
        crouch_eye: 47.0 * U,
    };

    #[test]
    fn the_rule_narrows_lowers_and_raises_the_step() {
        let b = SPARTAN.within(Some(&CSS));
        assert_eq!(b, MovementEnvelope { eye: CSS.eye, crouch_eye: SPARTAN.crouch_eye, ..CSS });
        // A smaller character keeps its own body and the higher step.
        let small = MovementEnvelope { radius: 0.3, height: 1.2, crouch_height: 0.9, step: 0.2, eye: 1.1, crouch_eye: 0.8 };
        assert_eq!(small.within(Some(&SPARTAN)), MovementEnvelope { step: 0.375, ..small });
        // No map envelope, or the character's own: unchanged.
        assert_eq!(SPARTAN.within(None), SPARTAN);
        assert_eq!(SPARTAN.within(Some(&SPARTAN)), SPARTAN);
        assert_eq!(CSS.within(Some(&CSS)), CSS);
    }

    #[test]
    fn a_box_fits_per_axis() {
        let b = SPARTAN.within(Some(&CSS));
        assert_eq!(b.fit_half_widths([1.0, 0.2], 1.0), [CSS.radius, 0.2]);
    }

    /// A lowered body's eye: at the map's own eye when that is above
    /// `EYE_MARGIN` under the new top (CS:S's eye is over its box), else
    /// under the top; never above the character's own.
    #[test]
    fn a_lowered_eye_is_never_below_the_maps_own() {
        // TF2's box (82 u, eye 68) on CS:S's maps: the box 62, the eye 64,
        // CS:S's own, not 58 (EYE_MARGIN under 62).
        let tf2 = MovementEnvelope { radius: 24.0 * U, height: 82.0 * U, crouch_height: 62.0 * U, step: 18.0 * U, eye: 68.0 * U, crouch_eye: 45.0 * U };
        let b = tf2.within(Some(&CSS));
        assert_eq!((b.height, b.eye), (CSS.height, CSS.eye));
        assert_eq!((b.crouch_height, b.crouch_eye), (CSS.crouch_height, tf2.crouch_eye));
        // A map whose own eye is low: EYE_MARGIN under the lowered top.
        let low = MovementEnvelope { height: 1.57, eye: 1.2, ..SPARTAN };
        assert!((SPARTAN.within(Some(&low)).eye - 1.47).abs() < 1e-6);
        // An eye already low stays its own.
        assert_eq!(MovementEnvelope::fitted_eye(1.2, 2.13, 1.57, 1.6), 1.2);
        assert_eq!(MovementEnvelope::fitted_eye(1.9, 2.13, 2.13, 0.0), 1.9);
    }

    #[test]
    fn a_map_carries_its_games_envelope() {
        let mut app = App::new();
        app.add_movement_envelope("halo_ce", SPARTAN).add_movement_envelope("cs_source", CSS);
        assert_eq!(map_envelope(app.world()), None);
        app.world_mut().insert_resource(MapGame("cs_source".into()));
        assert_eq!(map_envelope(app.world()), Some(CSS));
        app.world_mut().insert_resource(MapGame("greybox_of_nobody".into()));
        assert_eq!(map_envelope(app.world()), None);
    }
}
