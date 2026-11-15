//! Halo's player body (docs/plans/active/halo-ce.md, M2: "the Spartan
//! body (third person) with basic animations"): the multiplayer Spartan
//! (the biped `characters\cyborg_mp\cyborg_mp`, its model, animation graph
//! and collision model) as the engine's character model, animated on the
//! simulation's side so the server poses its hit volumes.
//!
//! - `skeleton`: the model's nodes as bones, the stored quaternions as
//!   usual rotations, joints from the vertices' two nodes, the root from
//!   Halo's axes and units to the engine's.
//! - `anims`: the graph's unit tables as the engine's `AnimSet` (at load)
//!   and each tick's state, frame and pose (`HaloAnim`).
//! - `hits`: the collision model's nodes as boxes on the bones, the head
//!   material's node the head.
//! - `load`: the body from a map file (`spartan`; textures through the map
//!   loader's model-shader looks, the armour tinted per colour as skins),
//!   added to a Halo map's kit and, from the donor map (`DONOR_MAP`,
//!   bloodgulch: every multiplayer map holds the same biped), to any other
//!   game's map (`GUEST`, the bundle's dressing); the donor's body is made
//!   once per app (its `LoadContext`).
//!
//! The body is unnamed and teamless, of game `halo_ce`: a character whose
//! identity is Halo's (`slots::GameIdentity`: a Halo gun's holder in a
//! party app, every player of Halo's own app) wears it
//! (`CharacterModels::for_identity`), and gets the engine's `Animator` and
//! hitboxes for it (`map::attach_hitboxes`). Here, after each tick
//! (`FixedLast`, before `map::SimPose`): each body of Halo's is ticked
//! (`HaloAnim::tick`, one call per 30 Hz tick: under another tick the
//! ticks are counted out of the engine's, so the animation keeps Halo's
//! pace) from its movement (`MovementState`, `Velocity`), its look
//! (`Intent`) and its weapon's label (`WeaponLabel`, which a weapon's step
//! sets; without one: `unarmed` empty-handed, a rifle's stance holding
//! anything), and the pose is the game's own (`Animator::fixed`) on both
//! the drawn animator and the server's (`SimAnimator`, which the engine
//! poses the hitboxes from); players ignore root translation (no frame
//! info is read) and turn with the look. `wear_colours` shows each player's
//! armour colour (`BodySkin`: the skin of its colour, `COLOURS`).
//!
//! Death: the body has no ragdoll; the engine hides a dead body
//! (`MapCharacterModel::ragdoll` None).
//!
//! Known shortcuts:
//! - The drawn body takes the tick's pose (no easing between ticks; Halo
//!   draws at its tick rate, the engine's frames may be faster).
//! - Death and ping animations (the unit damage table), the third-person
//!   weapon in the hand (the weapon class's hand marker `right hand`) and
//!   the left hand's IK onto it are later steps.

pub mod anims;
pub mod hits;
pub mod load;
pub mod skeleton;

use bevy::prelude::*;

use self::anims::{ARMED_DEFAULT, HaloAnim, Inputs, Landing, SEATS, State, key};
use super::{GAME, METERS_PER_UNIT};
use crate::{
    core::{Health, Intent, MovementState, Team, Velocity},
    map::{BodyModel, BodySkin, CharacterModels, SimAnimator, SimPose, anim::Animator},
};

/// The weapon label a holder's body plays (animations.md: the weapon tag's
/// label, `ar`, `hp`, ...): set by a weapon's step; without it, `unarmed`
/// with nothing held and `ARMED_DEFAULT` with anything.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct WeaponLabel(pub String);

/// A Halo body's animation state, and the share of a 30 Hz tick the
/// engine's ticks have counted towards the next.
#[derive(Component, Clone, Debug, Default)]
pub struct HaloBody {
    pub anim: HaloAnim,
    pub carry: f32,
}

/// The armour colours (objects.md, "Spartan multiplayer colour"): the
/// free-for-all table's 18, then the red and blue teams'. A body's skin
/// is its colour's index here.
pub const COLOURS: [u32; 20] = [
    0xFFFFFF, 0x000000, 0xFE0000, 0x0201E3, 0x707E71, 0xFFFF01, 0x00FF01, 0xFF56B9, 0xAB10F4, 0x01FFFF, 0x6493ED,
    0xFF7F00, 0x1ECC91, 0x006401, 0x603814, 0xC69C6C, 0x9D0B0E, 0xF5999E, 0xFF0000, 0x0000FF,
];

/// The skin of the red team's colour, and the blue's.
pub const RED: usize = 18;
pub const BLUE: usize = 19;

/// A colour of `COLOURS` as linear RGB 0..1 (the bytes over 255).
pub fn colour(index: usize) -> [f32; 3] {
    let c = COLOURS[index.min(COLOURS.len() - 1)];
    [(c >> 16) as u8, (c >> 8) as u8, c as u8].map(|b| f32::from(b) / 255.0)
}

/// The armour colour of a player of `team` (multiplayer_rules.md "Teams";
/// the engine's sides are teams 1 and 2, a free-for-all player's team its
/// own id past them, which picks its profile colour: Chimera has no
/// profile colour): red for the first side, blue for the second, the
/// table by id in a free-for-all, white without a team.
pub fn skin_of(team: Option<Team>) -> usize {
    match team.map(|t| t.0) {
        Some(1) => RED,
        Some(2) => BLUE,
        Some(t) if t > 2 => (t as usize - 3) % 18,
        _ => 0,
    }
}

/// The landing numbers of Halo's biped (movement.md, data/movement.toml).
fn landing() -> Landing {
    let m = super::movement::MovementData::builtin();
    Landing {
        soft_min: m.soft_land_min,
        hard_min: m.hard_land_min,
        soft_ticks: (m.soft_land_time * 30.0).round() as u32,
        hard_ticks: (m.hard_land_time * 30.0).round() as u32,
    }
}

/// What the body reads from a character (`Inputs`): the horizontal
/// velocity over the run speed in the frame of its look's yaw.
pub fn inputs(intent: &Intent, velocity: Vec3, state: &MovementState) -> Inputs {
    let run = super::movement::MovementData::builtin().run_forward * METERS_PER_UNIT;
    let turn = Quat::from_rotation_y(intent.yaw);
    let (forward, left) = (turn * Vec3::NEG_Z, turn * Vec3::NEG_X);
    Inputs {
        grounded: state.on_ground,
        crouching: state.crouching,
        throttle: Vec2::new(velocity.dot(forward), velocity.dot(left)) / run.max(1e-6),
        vertical: velocity.y / METERS_PER_UNIT,
        pitch: intent.pitch,
    }
}

/// The label to play: the holder's, else by what it holds; one the set
/// has no table for plays `unarmed`.
fn label_of(set: &crate::map::anim::AnimSet, own: Option<&WeaponLabel>, armed: bool) -> String {
    let want = own.map_or(if armed { ARMED_DEFAULT } else { "unarmed" }, |l| l.0.as_str());
    let known = set.names.contains_key(&key(SEATS[0], want, State::Idle.name()));
    if known {
        want.to_ascii_lowercase()
    } else {
        "unarmed".into()
    }
}

/// Bodies of Halo's on this tick's values (module doc).
#[allow(clippy::type_complexity)]
pub fn animate(
    models: Option<Res<CharacterModels>>,
    clock: Res<crate::core::SimClock>,
    mut bodies: Query<(
        Entity,
        &BodyModel,
        &Intent,
        &Velocity,
        &MovementState,
        Option<&Health>,
        Option<&WeaponLabel>,
        Option<&chimera_sim::weapon::Inventory>,
        Option<&mut HaloBody>,
        Option<&mut Animator>,
        Option<&mut SimAnimator>,
    )>,
    mut commands: Commands,
) {
    let Some(models) = models else { return };
    for (e, body, intent, velocity, state, health, label, inventory, halo, animator, sim) in &mut bodies {
        let Some(m) = models.0.get(body.0).filter(|m| m.game == Some(GAME)) else {
            if halo.is_some() {
                commands.entity(e).remove::<(HaloBody, SimAnimator)>();
            }
            continue;
        };
        let (Some(set), Some(mut animator)) = (m.animations.clone(), animator) else {
            continue;
        };
        if health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let mut fresh = HaloBody::default();
        let first = halo.is_none();
        let halo = match halo {
            Some(h) => h.into_inner(),
            None => &mut fresh,
        };
        halo.carry += clock.dt() * 30.0;
        let mut ticks = halo.carry.floor().min(4.0) as u32;
        halo.carry -= halo.carry.floor();
        if first {
            ticks = ticks.max(1);
        }
        if ticks == 0 {
            continue;
        }
        let armed = inventory.is_some_and(|i| i.active.is_some());
        let label = label_of(&set, label, armed);
        let i = inputs(intent, velocity.0, state);
        let land = landing();
        let mut pose = Vec::new();
        for _ in 0..ticks {
            pose = halo.anim.tick(&set, &label, &i, &land);
        }
        let fixed = Some(chimera_core::later::Later::ready(pose));
        animator.fixed = fixed.clone();
        animator.main = Some(0);
        animator.yaw = None;
        match sim {
            Some(mut s) => {
                s.0.fixed = fixed;
                s.0.main = Some(0);
            }
            None => {
                let mut s = animator.clone();
                s.fixed = fixed;
                commands.entity(e).insert(SimAnimator(s));
            }
        }
        if first {
            commands.entity(e).insert(fresh);
        }
    }
}

/// Each Halo body shows its player's armour colour (`skin_of`).
#[allow(clippy::type_complexity)]
pub fn wear_colours(
    models: Option<Res<CharacterModels>>,
    bodies: Query<(Entity, &BodyModel, Option<&Team>, Option<&BodySkin>)>,
    mut commands: Commands,
) {
    let Some(models) = models else { return };
    for (e, body, team, skin) in &bodies {
        let Some(m) = models.0.get(body.0).filter(|m| m.game == Some(GAME)) else {
            continue;
        };
        if m.model.skins.is_empty() {
            continue;
        }
        let want = BodySkin(skin_of(team.copied()) as i32);
        if skin != Some(&want) {
            commands.entity(e).insert(want);
        }
    }
}

/// Halo's bodies animated and coloured (module doc).
pub struct HaloBodyPlugin;

impl Plugin for HaloBodyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedLast, animate.before(SimPose))
            .add_systems(Update, wear_colours);
    }
}
