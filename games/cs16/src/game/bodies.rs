//! Counter-Strike 1.6's player bodies (specs/goldsrc/mdl.md): the eight
//! classes' models `models/player/<class>/<class>.mdl` as GoldSrc
//! character bodies of this game (`goldsrc_engine::content::studio`),
//! with their hitboxes and sequences, and what they play.
//!
//! The classes are numbered as CS:S's (`core::PlayerClass`: 1-4 the
//! Terrorists' terror, leet, arctic, guerilla; 5-8 the
//! Counter-Terrorists' urban, gsg9, sas, gign); a team's first class is
//! its default body (terror for team 1, urban for anyone else), the
//! others tagged with their class, as CS:S's bodies are (`add_bodies`).
//! On another game's map the same bodies are added by the guest dressing
//! (`dress_guest_in`), so a CS 1.6 player wears its body there.
//!
//! What a body plays (`drive`, the placeholder movement's stand-in for
//! CS 1.6's player animation): a gait sequence by the character's ground
//! speed and stance (`idle1`, `walk`, `run`, `crouch_idle`,
//! `crouchrun`; `jump` in the air), its playback matched to the speed
//! over its authored speed, and the stand-in pistol's aim
//! (`ref_aim_onehanded`, `crouch_aim_onehanded`) layered over the upper
//! body.
//!
//! Known shortcuts:
//! - The gait and aim choice is a stand-in until specs/cs16 has the
//!   player animation (M1/M2): no shooting or reloading sequences, the aim
//!   blend at the middle (pitch 0), no turning of the legs.

use bevy::prelude::*;
use goldsrc_engine::content::{METERS_PER_UNIT, studio};

use super::GAME;
use crate::{
    core::{MovementState, Team, Velocity},
    map::{
        BodyModel, CharacterModels, MapData,
        anim::{Animator, Layer},
    },
    mount::Mount,
};

/// The classes' models, in class order (module doc).
pub const CLASS_MODELS: [&str; 8] = [
    "models/player/terror/terror.mdl",
    "models/player/leet/leet.mdl",
    "models/player/arctic/arctic.mdl",
    "models/player/guerilla/guerilla.mdl",
    "models/player/urban/urban.mdl",
    "models/player/gsg9/gsg9.mdl",
    "models/player/sas/sas.mdl",
    "models/player/gign/gign.mdl",
];

/// The eight bodies into `data`'s kit (their textures appended), each
/// with its team and class (module doc); a model that doesn't read is a
/// warning.
pub fn add_bodies(mount: &Mount, decode: bool, data: &mut MapData) {
    for (i, path) in CLASS_MODELS.iter().enumerate() {
        if !mount.contains(path) {
            data.warnings.push(format!("{path}: not in the install"));
            continue;
        }
        match studio::character(mount, GAME, path, decode) {
            Ok(mut body) => {
                let c = &mut body.character;
                match i {
                    0 => c.team = Some(Team(1)),
                    4 => c.team = None,
                    _ => {
                        c.team = Some(Team(1 + i as u8 / 4));
                        c.class = Some(i as u8 + 1);
                    }
                }
                studio::add_to(data, body);
            }
            Err(e) => data.warnings.push(e),
        }
    }
}

/// The game's files for its guest dressing: the install, mounted.
pub fn guest_mount() -> Result<std::sync::Arc<Mount>, String> {
    super::maps::installed_mount().map(std::sync::Arc::new)
}

/// CS 1.6's bodies on another game's map (`lean`: textures' sizes only).
pub fn dress_guest_in(mount: &Mount, map: &mut MapData, lean: bool) {
    add_bodies(mount, !lean, map);
    let names = super::weapons::skeleton_names(map);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let kit = crate::map::KitLoad::Players(super::BUNDLE.kept);
    super::weapons::add_guns(mount, &|id| kit.wants(id), &names, !lean, map);
}

/// The gait a body plays for `speed` (m/s), its stance and whether it is
/// on the ground (module doc).
pub fn gait(speed: f32, crouching: bool, on_ground: bool) -> &'static str {
    match (on_ground, crouching, speed > 0.1, speed > 3.0) {
        (false, _, _, _) => "jump",
        (true, true, false, _) => "crouch_idle",
        (true, true, true, _) => "crouchrun",
        (true, false, false, _) => "idle1",
        (true, false, true, false) => "walk",
        (true, false, true, true) => "run",
    }
}

/// The stand-in pistol's aim for a stance.
pub fn aim(crouching: bool) -> &'static str {
    if crouching { "crouch_aim_onehanded" } else { "ref_aim_onehanded" }
}

/// Bodies of this game play their gait and aim (module doc).
#[allow(clippy::type_complexity)]
pub fn drive(
    mut bodies: Query<(&mut Animator, &BodyModel, Option<&Velocity>, Option<&MovementState>)>,
    models: Option<Res<CharacterModels>>,
    time: Res<Time>,
) {
    let Some(models) = models else { return };
    let (dt, now) = (time.delta_secs(), time.elapsed_secs_f64());
    for (mut a, body, velocity, state) in &mut bodies {
        if !models.0.get(body.0).is_some_and(|m| m.game == Some(GAME)) {
            continue;
        }
        let speed = velocity.map_or(0.0, |v| Vec2::new(v.0.x, v.0.z).length());
        let (crouching, on_ground) = state.map_or((false, true), |s| (s.crouching, s.on_ground));
        let set = a.set.clone();
        let Some(s) = set.sequence(gait(speed, crouching, on_ground)).or_else(|| set.sequence("idle1")) else {
            continue;
        };
        a.play(s, now);
        // Feet at the ground's speed: playback by the authored speed.
        let authored = set.ground_speed(s, &a.params) * METERS_PER_UNIT;
        a.playback = if authored > 0.0 { (speed / authored).clamp(0.3, 2.5) } else { 1.0 };
        a.advance(dt, now);
        if let Some(l) = set.sequence(aim(crouching)) {
            if a.layers.first().copied().flatten().is_none_or(|x| x.sequence != l) {
                a.layers = vec![Some(Layer { sequence: l, cycle: 0.0, weight: 1.0 })];
            }
        }
    }
}

/// CS 1.6's bodies played.
pub struct Cs16BodiesPlugin;

impl Plugin for Cs16BodiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, drive.in_set(crate::map::DriveAnimation));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_classes_four_a_side() {
        assert_eq!(CLASS_MODELS.len(), 8);
        assert!(CLASS_MODELS.iter().all(|p| p.starts_with("models/player/")));
        assert_eq!(gait(0.0, false, true), "idle1");
        assert_eq!(gait(5.0, false, true), "run");
        assert_eq!(gait(1.0, false, true), "walk");
        assert_eq!(gait(1.0, true, true), "crouchrun");
        assert_eq!(gait(0.0, false, false), "jump");
    }
}
