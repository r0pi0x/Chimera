//! The Spartan's footsteps, jumps and landings (specs/halo_ce/effects.md
//! "Material effects": kinds 0 walk, 1 run, 4 jump, 5 jump land;
//! animations.md "Footsteps"; sounds.md "A play request"), on every
//! machine from what it already simulates: each Halo body's animation
//! (`body::anims::HaloAnim`, ticked on every machine from the replicated
//! movement) says which foot stepped this tick (`HaloAnim::feet`: a move
//! state, the throttle above 0.5, the frame one of the animation's foot
//! frames), and its state the jump and the landing.
//!
//! A step plays the biped's material effects sound (`AudioLibrary::
//! footsteps`, `globals\cyborg`) of its kind (walk crouching, run standing;
//! jump on leaving the ground upward, jump land on entering a landing,
//! soft or hard) for the material under the feet: a probe from the body's
//! centre straight down to 0.15 WU below its feet (effects.md's 0.3 WU
//! below a point 0.15 WU above the foot) against the level, the hit's
//! material type from the level's table (`weapons::materials::
//! LevelMaterials`; without a table, stone, as the weapons' traces count
//! it); no hit, no step. It plays at the feet with scale 0 (sounds.md's
//! admission drops a sound whose gain at scale 0 is 0) and only within
//! 10 WU of the local player (effects.md's `MATERIAL_EFFECT_RANGE`), whose
//! own steps it includes. A carried body (a vehicle's rider: the engine's
//! `Carried`) makes none (`core::Afoot`). Only bodies of Halo's (its `HaloBody`: a
//! character of Halo's identity, `GameIdentity`) step here: another game's
//! character keeps its own game's footsteps, and a Halo holder's are not
//! another game's (each game's footsteps skip identities not its own). A
//! campaign body (`campaign::BodyKind`) steps with its own biped's
//! material effects (`campaign::voice::CampaignSounds::steps`, from its
//! unit: the Elite's `footsteps\elite\...`), the Spartan's without them.
//!
//! Known shortcuts: one probe at the body's centre for both feet (not
//! each foot's contact marker); the landing plays as the landing state
//! starts (not 2 ticks into the recovery); water (material 28 when the
//! probe's point is under water) and the material effects' effects (the
//! water splash) are not made; sliding and shuffling (kinds 2, 3) are not
//! played (the body has no turn-in-place or slip states).

use std::collections::BTreeMap;

use avian3d::prelude::{Collider, Position, Rotation, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;

use super::{library::ActiveAudio, looping::one_shot};
use crate::{
    core::{Intent, LocalPlayer, MovementState, SOLID_LAYERS, Velocity},
    map::PlaySound,
};
use super::super::{
    METERS_PER_UNIT,
    body::{HaloBody, anims::State},
    movement::MovementData,
    weapons::materials::{LevelMaterials, NO_TABLE},
};

/// The material effects kinds (effects.md).
pub const WALK: usize = 0;
pub const RUN: usize = 1;
pub const JUMP: usize = 4;
pub const JUMP_LAND: usize = 5;
/// Material effects play within this of a local player, WU.
pub const RANGE: f32 = 10.0;
/// The probe's reach below the feet, WU.
pub const BELOW_FEET: f32 = 0.15;

/// What a body's animation did since it was last seen.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seen {
    pub sequence: Option<usize>,
    pub frame: usize,
    pub state: State,
    pub air_ticks: u32,
}

/// The step kinds a body makes going from `was` to `now` with `feet` the
/// tick's feet and `rising` its upward speed (module doc).
pub fn kinds(was: &Seen, now: &Seen, feet: u8, crouching: bool, rising: bool) -> Vec<usize> {
    let mut out = Vec::new();
    let new_frame = (now.sequence, now.frame) != (was.sequence, was.frame);
    if feet != 0 && new_frame {
        let k = if crouching { WALK } else { RUN };
        out.extend(std::iter::repeat_n(k, feet.count_ones() as usize));
    }
    if was.air_ticks == 0 && now.air_ticks > 0 && rising {
        out.push(JUMP);
    }
    let landing = |s: State| matches!(s, State::LandSoft | State::LandHard);
    if landing(now.state) && !landing(was.state) {
        out.push(JUMP_LAND);
    }
    out
}

/// The bodies' steps this tick (module doc).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn footsteps(
    active: Res<ActiveAudio>,
    bodies: Query<
        (Entity, &HaloBody, &Transform, Option<&MovementState>, Option<&Velocity>, Option<&crate::game::campaign::BodyKind>),
        crate::core::Afoot,
    >,
    campaign: Option<Res<crate::game::campaign::voice::CampaignSounds>>,
    locals: Query<&Transform, With<LocalPlayer>>,
    characters: Query<Entity, With<Intent>>,
    spatial: SpatialQuery,
    shapes: Query<(&Collider, Option<&Position>, Option<&Rotation>, Option<&Name>)>,
    materials: Option<Res<LevelMaterials>>,
    data: Option<Res<MovementData>>,
    mut seen: Local<BTreeMap<Entity, Seen>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let Some(lib) = active.0.as_deref() else { return };
    if lib.footsteps.is_empty() {
        return;
    }
    let listeners: Vec<Vec3> = locals.iter().map(|t| t.translation).collect();
    seen.retain(|e, _| bodies.contains(*e));
    for (e, body, t, state, velocity, kind) in &bodies {
        // A campaign body steps with its own biped's sounds.
        let lib = campaign.as_deref().and_then(|c| c.steps(kind)).unwrap_or(lib);
        let a = &body.anim;
        let now = Seen { sequence: a.sequence, frame: a.frame, state: a.state, air_ticks: a.air_ticks };
        let was = seen.insert(e, now).unwrap_or(now);
        let crouching = state.is_some_and(|s| s.crouching);
        let rising = velocity.is_some_and(|v| v.0.y > 0.0);
        let made = kinds(&was, &now, a.feet, crouching, rising);
        if made.is_empty() {
            continue;
        }
        let at = t.translation;
        let near = listeners.is_empty() || listeners.iter().any(|l| l.distance(at) <= RANGE * METERS_PER_UNIT);
        if !near {
            continue;
        }
        // The probe: from the centre to 0.15 WU below the feet.
        let centre = data.as_deref().map_or(0.5, |d| d.centre_above_origin(if crouching { 1.0 } else { 0.0 }));
        let reach = (centre + BELOW_FEET) * METERS_PER_UNIT;
        let filter = SpatialQueryFilter::from_excluded_entities(characters.iter()).with_mask(SOLID_LAYERS);
        let Some(hit) = spatial.cast_ray(at, Dir3::NEG_Y, reach, true, &filter) else { continue };
        let material = match &materials {
            Some(m) => m.at(shapes.get(hit.entity).ok(), at, Vec3::NEG_Y, hit.distance),
            None => Some(NO_TABLE),
        };
        let Some(material) = material else { continue };
        let feet = at - Vec3::Y * hit.distance;
        for k in made {
            let Some(sound) = lib.footsteps.get(k).and_then(|row| row.get(usize::from(material))).filter(|s| !s.is_empty()) else { continue };
            if let Some(s) = one_shot(lib, sound, 1.0, 0.0, Some(feet), 0.5) {
                sounds.write(s);
            }
        }
    }
}

/// The steps after the bodies' tick (`body::animate`).
pub(super) fn plugin(app: &mut App) {
    app.add_systems(FixedLast, footsteps.after(super::super::body::animate).before(crate::map::SimPose));
}
