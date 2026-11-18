//! TF2's living players' faces (specs/tf2/faces.md; clients only, never
//! networked): each TF2 body plays its class's idle expression
//! (data/faces.toml, spec 5), a scene of one expression event that pulls
//! the preset's controllers from 0 to its weights at the event's
//! intensity (4: v = w × clamp(I)), I a Catmull-Rom curve through the
//! event's samples between edge points of 0 at its start and end, the
//! samples' neighbours slid in time, the edges' not (4.1: a plateau of
//! two equal samples bulges to 1.125 × theirs mid-scene),
//! looping at the scene's length from the body's spawn; then the face's
//! rules and the mesh flexes' weights (`map::FaceWeights`, which the drawn
//! face follows). The Engineer's `defaultFace` 0.98 is what closes his
//! mouth: at rest (every controller 0) it hangs open (spec "Quirks").
//! The Pyro has no idle face: his stays at rest. No blinking (8).
//!
//! A dead player's scene plays on to its end, then the face is at rest
//! (10); a respawn starts the scene again.
//!
//! Known shortcuts: only the normal idle (spec 5): no hurt, round-lost,
//! winner, scoped or Minigun faces (6), firing faces (7) or eye aim (9:
//! the eyes look ahead); the loop restarts at once (Open question 1); a
//! ragdoll keeps the face its body showed at death (spec 10: it follows
//! the owner's scene to its end).

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::Deserialize;

use super::player_anim::Tf2PlayerAnim;
use crate::{core::Health, map};

/// The idle faces, as written.
pub const DATA: &str = include_str!("../../data/faces.toml");

/// One class's idle expression scene (spec 5).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct IdleFace {
    /// The class's player model.
    pub model: String,
    /// The scene's length (s): it starts again after it.
    pub length: f32,
    /// The event's span in the scene (s).
    pub from: f32,
    pub to: f32,
    /// Intensity samples (event time s, intensity); none: 1 throughout.
    #[serde(default)]
    pub ramp: Vec<[f32; 2]>,
    /// The preset's controllers and weights (each controller's range).
    pub preset: BTreeMap<String, f32>,
}

#[derive(Deserialize)]
struct Doc {
    classes: BTreeMap<String, IdleFace>,
}

/// The idle faces by class.
#[derive(Resource, Clone, Debug)]
pub struct IdleFaces(pub BTreeMap<String, IdleFace>);

impl IdleFaces {
    pub fn parse(text: &str) -> Result<IdleFaces, String> {
        let doc: Doc = toml::from_str(text).map_err(|e| format!("faces.toml: {e}"))?;
        for (class, f) in &doc.classes {
            if !(f.length > 0.0 && 0.0 <= f.from && f.from < f.to && f.to <= f.length + 1e-3) {
                return Err(format!("faces.toml: {class}'s span {}..{} in {}", f.from, f.to, f.length));
            }
        }
        Ok(IdleFaces(doc.classes))
    }

    /// The built-in table.
    pub fn builtin() -> IdleFaces {
        IdleFaces::parse(DATA).expect("data/faces.toml")
    }

    /// The idle face of the class whose model is `model`.
    pub fn of_model(&self, model: &str) -> Option<&IdleFace> {
        self.0.values().find(|f| f.model.eq_ignore_ascii_case(model))
    }
}

impl IdleFace {
    /// The event's intensity at scene time `t` (spec 4.1).
    pub fn intensity(&self, t: f32) -> f32 {
        let len = self.to - self.from;
        let tau = t - self.from;
        if !(0.0..=len).contains(&tau) {
            return 0.0;
        }
        if self.ramp.is_empty() {
            return 1.0;
        }
        let mut p: Vec<[f32; 2]> = Vec::with_capacity(self.ramp.len() + 2);
        p.push([0.0, 0.0]);
        p.extend(self.ramp.iter().copied());
        p.push([len, 0.0]);
        let i = p.windows(2).position(|w| tau <= w[1][0]).unwrap_or(p.len() - 2);
        let (p1, p2) = (p[i], p[i + 1]);
        let d = p2[0] - p1[0];
        if d <= 0.0 {
            return p2[1].clamp(0.0, 1.0);
        }
        // A neighbour that is a sample slides along its line to the span's
        // time distance; one beyond an edge is the edge's 0, not slid.
        let last = p.len() - 1;
        let y0 = if i >= 2 {
            let p0 = p[i - 1];
            p1[1] + (p0[1] - p1[1]) * d / (p1[0] - p0[0]).max(1e-9)
        } else {
            0.0
        };
        let y3 = if i + 2 < last {
            let p3 = p[i + 2];
            p2[1] + (p3[1] - p2[1]) * d / (p3[0] - p2[0]).max(1e-9)
        } else {
            0.0
        };
        let (y1, y2) = (p1[1], p2[1]);
        let u = (tau - p1[0]) / d;
        let v = 0.5
            * (2.0 * y1
                + (y2 - y0) * u
                + (2.0 * y0 - 5.0 * y1 + 4.0 * y2 - y3) * u * u
                + (-y0 + 3.0 * y1 - 3.0 * y2 + y3) * u * u * u);
        v.clamp(0.0, 1.0)
    }

    /// Each of `face`'s controllers at scene time `t` (spec 4 step 2 from
    /// 0): the preset's weight times the intensity, by name (any case);
    /// the rest 0.
    pub fn controllers(&self, face: &map::MapFace, t: f32) -> Vec<f32> {
        let s = self.intensity(t).clamp(0.0, 1.0);
        face.controllers
            .iter()
            .map(|c| {
                self.preset
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(&c.name))
                    .map_or(0.0, |(_, w)| w * s)
            })
            .collect()
    }
}

/// A TF2 body's idle scene: when it started (s) and whether its player
/// was alive last frame.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct IdleScene {
    pub start: f64,
    pub alive: bool,
}

/// Each drawn TF2 body with a face: its idle expression's weights.
#[allow(clippy::type_complexity)]
pub fn faces(
    time: Res<Time>,
    table: Option<Res<IdleFaces>>,
    models: Option<Res<map::CharacterModels>>,
    mut bodies: Query<
        (Entity, &map::BodyModel, Option<&mut IdleScene>, Option<&mut map::FaceWeights>, Option<&Health>),
        With<Tf2PlayerAnim>,
    >,
    mut commands: Commands,
) {
    let (Some(table), Some(models)) = (table, models) else { return };
    let now = time.elapsed_secs_f64();
    for (e, body, scene, weights, health) in &mut bodies {
        let Some(model) = models.0.get(body.0) else { continue };
        let Some(face) = model.model.face.as_ref() else { continue };
        let alive = health.is_none_or(|h| h.current > 0.0);
        let idle = model.name.as_deref().and_then(|m| table.of_model(m));
        let Some(mut scene) = scene else {
            commands.entity(e).insert(IdleScene { start: now, alive });
            continue;
        };
        // A respawn starts the scene again; a living player's loops.
        if alive && !scene.alive {
            scene.start = now;
        }
        scene.alive = alive;
        let mut t = (now - scene.start) as f32;
        if let Some(f) = idle.filter(|_| alive) {
            if t >= f.length {
                let loops = (t / f.length).floor();
                scene.start += (loops * f.length) as f64;
                t -= loops * f.length;
            }
        }
        let values = match idle {
            Some(f) => f.controllers(face, t),
            None => vec![0.0; face.controllers.len()],
        };
        let w = source_engine::content::face::weights(face, &values);
        match weights {
            Some(mut x) => {
                if x.0 != w {
                    x.0 = w;
                }
            }
            None => {
                commands.entity(e).insert(map::FaceWeights(w));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engineer() -> IdleFace {
        IdleFaces::builtin().0["engineer"].clone()
    }

    /// Test cases 1 and 1b: the Engineer's `defaultFace` 0.98 at its
    /// samples, bulging to 1 (clamped) between them, 0 at the scene's
    /// ends; a two-sample plateau is v (1 + u (1 - u) / 2).
    #[test]
    fn the_engineers_idle_face_holds_default_face() {
        let f = engineer();
        assert!(f.intensity(0.0).abs() < 1e-6);
        assert!((f.intensity(0.194) - 0.98).abs() < 1e-4, "{}", f.intensity(0.194));
        assert_eq!(f.intensity(15.0), 1.0);
        assert!((f.intensity(29.68) - 0.98).abs() < 1e-4);
        assert!(f.intensity(29.9) < 0.98);
        assert!(f.intensity(30.0).abs() < 1e-6);
        let sniper = IdleFaces::builtin().0["sniper"].clone();
        let (a, b, v) = (0.338, 14.556, 0.514);
        for u in [0.25f32, 0.5] {
            let i = sniper.intensity(a + u * (b - a));
            assert!((i - v * (1.0 + 0.5 * u * (1.0 - u))).abs() < 1e-4, "u {u}: {i}");
        }
    }

    fn face(names: &[&str]) -> map::MapFace {
        map::MapFace {
            controllers: names
                .iter()
                .map(|n| map::face::MapFlexController { name: n.to_string(), min: 0.0, max: 1.0 })
                .collect(),
            ..default()
        }
    }

    /// Test cases 2-4: the Spy's, Soldier's and Heavy's controllers
    /// mid-scene (7.5 s, the Soldier's plateau at its middle: 0.825);
    /// every other controller 0.
    #[test]
    fn mid_scene_controllers_are_the_presets_at_their_intensity() {
        let t = IdleFaces::builtin();
        let names = ["defaultFace", "idleface", "right_CloseLid", "left_CloseLid", "multi_CloseLid", "AH"];
        let f = face(&names);
        let near = |a: &[f32], b: &[f32]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 2e-3);
        let spy = t.0["spy"].controllers(&f, 7.5);
        assert!(near(&spy, &[1.0, 0.0, -0.962, -0.962, -0.656, 0.0]), "{spy:?}");
        let soldier = t.0["soldier"].controllers(&f, 7.512);
        assert!(near(&soldier, &[0.825, 0.0, -0.323, -0.323, -0.541, 0.0]), "{soldier:?}");
        let heavy = t.0["heavy"].controllers(&f, 7.5);
        assert!(near(&heavy, &[0.0, 1.0, -0.58, -0.58, 0.62, 0.0]), "{heavy:?}");
        // The Pyro has none.
        assert!(t.0.get("pyro").is_none());
    }

    /// The table covers every class's model but the Pyro's, and its spans
    /// fit their scenes.
    #[test]
    fn every_class_but_the_pyro_has_an_idle_face() {
        let t = IdleFaces::builtin();
        let classes = super::super::classes::Classes::parse(super::super::classes::DATA).unwrap();
        for c in &classes.classes {
            assert_eq!(t.of_model(&c.model).is_some(), c.name != "pyro", "{}", c.name);
        }
    }
}
