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
//! What a body plays (`drive`, CS 1.6's player animation as far as the
//! model's own sequences tell it): a gait sequence by the character's
//! ground speed and stance (`idle1`, `walk`, `run`, `crouch_idle`,
//! `crouchrun`; `jump` in the air), its playback matched to the speed
//! over its authored speed; over the upper body the held weapon's set
//! (`BodyAnim`, the weapon data's `[hooks."cs16:body_anim"] set`: the
//! models' sequences are named `ref_aim_<set>`, `crouch_shoot_<set>` ...):
//! its aim (`ref_aim_<set>`, `crouch_aim_<set>`) on the pitch blends
//! (`body_pitch`, the view's pitch down, `aim_param`), and over it a
//! gesture from its start to its end: a shot, swing or throw
//! `ref_shoot_<set>` (the dual pistols `ref_shoot_` and `ref_shoot2_` by
//! the clip's parity, as the view model's left and right), a reload
//! `ref_reload_<set>` (`crouch_` for each while ducked). A weapon with no
//! set (another game's) plays `onehanded`'s, as does a set the model
//! lacks. The body holds its active weapon's held model (`hold_weapons`:
//! `map::Held`, the `p_` model in the right hand).
//!
//! Known shortcuts:
//! - No spec says which set each weapon plays (view_and_hud.md has no
//!   player animation): the sets in the data are ours, by the sequences'
//!   names (`ak47`, `mp5`, `m249`, `knife`, `grenade`, `dualpistols` where
//!   the model names the weapon; pistols `onehanded`, shotguns `shotgun`,
//!   SMGs `carbine`, other rifles and snipers `rifle`).
//! - Which way the pitch blends run (mdl.md open question 2) is read
//!   from the model (the forearms point up at the blends' start, −90,
//!   down at their end); the original may snap between blends instead.
//! - No turning of the legs against the view, no swimming, death,
//!   flinch or shield sequences; a server's hitboxes stay in the drawn
//!   pose's place (no tick pose of their own).

use bevy::prelude::*;
use goldsrc_engine::content::{METERS_PER_UNIT, studio};

use super::GAME;
use chimera_sim::weapon::{Inventory, Magazine, Weapon, WeaponEvent, WeaponEventKind};
use serde::Deserialize;

use crate::{
    core::{Intent, MovementState, Team, Velocity},
    map::{
        BodyModel, CharacterModels, Held, MapData,
        anim::{AnimSet, Animator, Layer},
        interp::{RenderedView, eye_view},
    },
    mount::Mount,
};

/// The weapon data's hook naming the body's sequence set (module doc).
pub const BODY_ANIM_HOOK: &str = "cs16:body_anim";

/// The set a weapon without one plays.
pub const FALLBACK: &str = "onehanded";

/// A weapon's sequence set (`BODY_ANIM_HOOK`): `ak47` for
/// `ref_aim_ak47` and its kin.
#[derive(Component, Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BodyAnim {
    pub set: String,
}

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

/// A sequence of `set` for a stance: `ref_<what>_<set>` standing,
/// `crouch_<what>_<set>` ducked (`what`: `aim`, `shoot`, `shoot2`,
/// `reload`).
pub fn upper(what: &str, set: &str, crouching: bool) -> String {
    format!("{}_{what}_{set}", if crouching { "crouch" } else { "ref" })
}

/// The pitch blends' parameter for a view `pitch_up` degrees above level
/// (module doc): the blends run from −90 (up) to +90 (down), GoldSrc's
/// pitch (positive looking down).
pub fn aim_param(pitch_up: f32) -> f32 {
    (-pitch_up).clamp(-90.0, 90.0)
}

/// Sequence `what` of `set` (else of `FALLBACK`) for a stance in `anims`.
fn find(anims: &AnimSet, what: &str, set: &str, crouching: bool) -> Option<usize> {
    anims
        .sequence(&upper(what, set, crouching))
        .or_else(|| anims.sequence(&upper(what, FALLBACK, crouching)))
}

/// A body's gesture over its aim (module doc).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
// Comes and goes with the game's body on a character: a sparse set, or
// each combination with the rest makes a table (docs/DESIGN.md, "ECS style").
#[component(storage = "SparseSet")]
pub struct Gesture {
    /// The sequence and how far in (0-1); None: none playing.
    pub playing: Option<(usize, f32)>,
}

/// What a weapon did, by owner, this frame: the gesture to start
/// (`what`) and the weapon.
fn happened(events: &mut MessageReader<WeaponEvent>, magazines: &Query<&Magazine>) -> Vec<(Entity, &'static str)> {
    let mut out = Vec::new();
    for ev in events.read().filter(|e| e.shown()) {
        let what = match ev.kind {
            WeaponEventKind::Shot { .. } => {
                // The dual pistols: the left gun on an odd clip after the
                // shot (the view model's rule).
                let odd = magazines.get(ev.weapon).is_ok_and(|m| m.clip % 2 == 1);
                if odd { "shoot" } else { "shoot2" }
            }
            WeaponEventKind::Swing { .. } | WeaponEventKind::Thrown => "shoot",
            WeaponEventKind::ReloadStarted => "reload",
            _ => continue,
        };
        out.push((ev.owner, what));
    }
    out
}

/// Bodies of this game play their gait, aim and gestures (module doc).
#[allow(clippy::type_complexity)]
pub fn drive(
    mut bodies: Query<(
        Entity,
        &mut Animator,
        &BodyModel,
        Option<&Velocity>,
        Option<&MovementState>,
        Option<&Intent>,
        Option<&RenderedView>,
        Option<&Inventory>,
        Option<&mut Gesture>,
    )>,
    sets: Query<&BodyAnim>,
    magazines: Query<&Magazine>,
    mut events: MessageReader<WeaponEvent>,
    models: Option<Res<CharacterModels>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let did = happened(&mut events, &magazines);
    let Some(models) = models else { return };
    let (dt, now) = (time.delta_secs(), time.elapsed_secs_f64());
    for (e, mut a, body, velocity, state, intent, view, inventory, gesture) in &mut bodies {
        if !models.0.get(body.0).is_some_and(|m| m.game == Some(GAME)) {
            continue;
        }
        let Some(mut gesture) = gesture else {
            commands.entity(e).insert(Gesture::default());
            continue;
        };
        let speed = velocity.map_or(0.0, |v| Vec2::new(v.0.x, v.0.z).length());
        let (crouching, on_ground) = state.map_or((false, true), |s| (s.crouching, s.on_ground));
        let anims = a.set.clone();
        let Some(s) = anims.sequence(gait(speed, crouching, on_ground)).or_else(|| anims.sequence("idle1")) else {
            continue;
        };
        a.play(s, now);
        // Feet at the ground's speed: playback by the authored speed.
        let authored = anims.ground_speed(s, &a.params) * METERS_PER_UNIT;
        a.playback = if authored > 0.0 { (speed / authored).clamp(0.3, 2.5) } else { 1.0 };
        // The aim's blends by the view's pitch.
        if let (Some(i), Some(intent), Some(state)) = (anims.param(studio::PITCH_PARAM), intent, state) {
            let look = eye_view(view, intent, state);
            a.set_param(i, aim_param(look.pitch.to_degrees()));
        }
        a.advance(dt, now);
        let set = inventory
            .and_then(|i| i.active)
            .and_then(|w| sets.get(w).ok())
            .map_or(FALLBACK, |b| b.set.as_str());
        // A gesture this frame starts from its first frame.
        if let Some((_, what)) = did.iter().rev().find(|(o, _)| *o == e) {
            let seq = find(&anims, what, set, crouching).or_else(|| find(&anims, "shoot", set, crouching).filter(|_| *what == "shoot2"));
            gesture.playing = seq.map(|q| (q, 0.0));
        } else if let Some((q, cycle)) = gesture.playing {
            let c = cycle + dt * anims.cycle_rate(q, &a.params);
            gesture.playing = (c < 1.0).then_some((q, c));
        }
        let mut layers = Vec::with_capacity(2);
        if let Some(l) = find(&anims, "aim", set, crouching) {
            let cycle = a.layers.first().copied().flatten().filter(|x| x.sequence == l).map_or(0.0, |x| x.cycle);
            let cycle = (cycle + dt * anims.cycle_rate(l, &a.params)).fract();
            layers.push(Some(Layer { sequence: l, cycle, weight: 1.0 }));
        }
        if let Some((q, cycle)) = gesture.playing {
            layers.push(Some(Layer { sequence: q, cycle, weight: 1.0 }));
        }
        a.layers = layers;
    }
}

/// Characters in this game's bodies hold their active weapon's held
/// model (`map::Held`: its `p_` model in the right hand).
#[allow(clippy::type_complexity)]
pub fn hold_weapons(
    characters: Query<(Entity, Option<&Inventory>, Option<&Held>, &BodyModel), With<Animator>>,
    weapons: Query<&Weapon>,
    models: Option<Res<CharacterModels>>,
    mut commands: Commands,
) {
    let Some(models) = models else { return };
    for (e, inventory, held, body) in &characters {
        if !models.0.get(body.0).is_some_and(|m| m.game == Some(GAME)) {
            continue;
        }
        let id = inventory.and_then(|i| i.active).and_then(|w| weapons.get(w).ok()).map(|w| w.id.to_string());
        if held.is_none_or(|h| h.0 != id) {
            commands.entity(e).insert(Held(id));
        }
    }
}

/// CS 1.6's bodies played.
pub struct Cs16BodiesPlugin;

impl Plugin for Cs16BodiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (hold_weapons, drive.in_set(crate::map::DriveAnimation)));
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
        assert_eq!(upper("shoot", "ak47", false), "ref_shoot_ak47");
        assert_eq!(upper("aim", "rifle", true), "crouch_aim_rifle");
        assert_eq!(aim_param(120.0), -90.0);
    }
}
