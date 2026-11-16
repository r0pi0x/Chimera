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
//! - `flinch`: the soft ping over the body, from a hit each machine reads
//!   off the replicated vitality.
//! - `death`: a dead player's body (`HaloCorpse`): the death chosen, played
//!   and removed.
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
//! ticks are counted out of the engine's tick index, `ticks::halo_ticks`,
//! so the animation keeps Halo's pace exactly) from its movement (`MovementState`, `Velocity`), its look
//! (`Intent`) and its weapon's label (`WeaponLabel`, which a weapon's step
//! sets; without one: `unarmed` empty-handed, a rifle's stance holding
//! anything), and the pose is the game's own (`Animator::fixed`) on both
//! the drawn animator and the server's (`SimAnimator`, which the engine
//! poses the hitboxes from); players ignore root translation (no frame
//! info is read) and turn with the look. `wear_colours` shows each player's
//! armour colour (`BodySkin`: the skin of its colour, `COLOURS`); a body
//! with a colour of its own (a campaign body, tinted all over) wears that
//! for a player given no colour and in place of the table's black, which
//! would leave it a silhouette (ours, not the game's: no campaign
//! character is a multiplayer body there).
//!
//! A campaign body (`campaign::BodyKind`: the Elite in a party app) is the
//! same code on its own model (named `halo_ce:elite`, worn through the
//! engine's `BodyName`): its graph's tables, its hit boxes, its landing
//! and run numbers (`campaign::movement_of`), its move animations at the
//! ground speed (`anims::Inputs::rate`), its corpse (`HaloCorpse::body`).
//!
//! Death (`death`): the body has no ragdoll; a dead player is hidden (the
//! engine's `Dead`) and the server leaves a body of its own in its place
//! (`death::HaloCorpse`, replicated: net part `halo_ce_corpses`), which
//! every machine plays (the death the killing hit picked from the unit
//! damage table or the airborne death, its sound, its fall) and draws as
//! the engine's `map::Corpse`.
//!
//! Known shortcuts:
//! - The drawn body takes the tick's pose (no easing between ticks; Halo
//!   draws at its tick rate, the engine's frames may be faster).
//! - The left hand's IK onto the held weapon is a later step (the weapon
//!   in the hand is `view`'s held model).

pub mod anims;
pub mod death;
pub mod flinch;
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
/// label, `ar`, `hp`, ...): set by a weapon's step; without it (or empty),
/// `unarmed` with nothing held and `ARMED_DEFAULT` with anything.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct WeaponLabel(pub String);

/// A Halo body's animation state, stepped once a 30 Hz tick
/// (`ticks::halo_ticks`).
#[derive(Component, Clone, Debug, Default)]
pub struct HaloBody {
    pub anim: HaloAnim,
    /// Its vitality as last seen here (`flinch::hit_between`).
    pub seen: Option<flinch::Seen>,
    /// The model it animates (`BodyModel`): another (a campaign body put
    /// on, or loaded) starts the animation afresh, its sequences being
    /// another set's.
    pub model: Option<usize>,
}

/// The armour colours (objects.md, "Spartan multiplayer colour"): the
/// free-for-all table's 18, then the red and blue teams'. A body's skin
/// is its colour's index here.
pub const COLOURS: [u32; 20] = [
    0xFFFFFF, 0x000000, 0xFE0000, 0x0201E3, 0x707E71, 0xFFFF01, 0x00FF01, 0xFF56B9, 0xAB10F4, 0x01FFFF, 0x6493ED,
    0xFF7F00, 0x1ECC91, 0x006401, 0x603814, 0xC69C6C, 0x9D0B0E, 0xF5999E, 0xFF0000, 0x0000FF,
];

/// The skin of the table's black (a campaign body wears its own colour
/// instead: `wear_colours`).
pub const BLACK: usize = 1;

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

/// The landing numbers of a biped (movement.md; data/movement.toml, a
/// campaign body's own: `campaign::movement_of`).
fn landing(m: &super::movement::MovementData) -> Landing {
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
    inputs_for(&super::movement::MovementData::builtin(), intent, velocity, state)
}

/// `inputs` for a body of movement table `m` (its own run speed).
pub fn inputs_for(m: &super::movement::MovementData, intent: &Intent, velocity: Vec3, state: &MovementState) -> Inputs {
    let run = m.run_forward * METERS_PER_UNIT;
    let turn = Quat::from_rotation_y(intent.yaw);
    let (forward, left) = (turn * Vec3::NEG_Z, turn * Vec3::NEG_X);
    Inputs {
        grounded: state.on_ground,
        crouching: state.crouching,
        throttle: Vec2::new(velocity.dot(forward), velocity.dot(left)) / run.max(1e-6),
        vertical: velocity.y / METERS_PER_UNIT,
        pitch: intent.pitch,
        throw: None,
        hit: None,
        own_speeds: None,
        speed: 0.0,
    }
}

/// The label to play: the holder's, else by what it holds; one the set
/// has no table for plays `unarmed`, or, for a body without an unarmed
/// table (a campaign character's graph: the Elite's), the first label it
/// has (animations.md: an unselectable weapon keeps a previous set; a
/// body never seen with one takes its first).
fn label_of(set: &crate::map::anim::AnimSet, own: Option<&WeaponLabel>, armed: bool) -> String {
    let want = own.filter(|l| !l.0.is_empty()).map_or(if armed { ARMED_DEFAULT } else { "unarmed" }, |l| l.0.as_str());
    let has = |label: &str| set.names.contains_key(&key(SEATS[0], label, State::Idle.name()));
    if has(want) {
        want.to_ascii_lowercase()
    } else if has("unarmed") {
        "unarmed".into()
    } else {
        anims::LABELS.iter().find(|l| has(l)).map_or("unarmed", |l| l).into()
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
        Option<&super::weapons::HaloHands>,
        Option<&super::vitality::Vitality>,
        Option<&super::campaign::BodyKind>,
    )>,
    (tables, data): (Option<Res<super::campaign::BodyTables>>, Option<Res<super::movement::MovementData>>),
    mut commands: Commands,
) {
    let Some(models) = models else { return };
    let builtin;
    let spartan = match data.as_deref() {
        Some(d) => d,
        None => {
            builtin = super::movement::MovementData::builtin();
            &builtin
        }
    };
    for (e, body, intent, velocity, state, health, label, inventory, halo, animator, sim, hands, vitality, kind) in &mut bodies {
        // The body's own landing and run numbers (`campaign::movement_of`).
        let table = super::campaign::movement_of(spartan, tables.as_deref(), kind);
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
        if halo.model != Some(body.0) {
            halo.anim = HaloAnim::default();
            halo.model = Some(body.0);
        }
        let mut ticks = super::ticks::halo_ticks(&clock).min(4);
        if first {
            ticks = ticks.max(1);
        }
        if ticks == 0 {
            continue;
        }
        let armed = inventory.is_some_and(|i| i.active.is_some());
        let label = label_of(&set, label, armed);
        let mut i = inputs_for(table, intent, velocity.0, state);
        // A campaign body's legs keep up with its ground speed (`Inputs::rate`).
        i.own_speeds = kind.and_then(|k| tables.as_deref()?.animation.get(k)).map(|a| a.array());
        i.speed = Vec2::new(velocity.0.x, velocity.0.z).length() / METERS_PER_UNIT;
        i.throw = hands.and_then(|h| h.throw.frame).map(|f| f.max(0) as usize);
        // A hit since the last tick seen here flinches it (`flinch`).
        if let Some(v) = vitality {
            let now = flinch::Seen::of(v);
            let hit = halo.seen.is_some_and(|old| flinch::hit_between(&old, &now));
            i.hit = hit.then(|| flinch::draw(&now));
            halo.seen = Some(now);
        }
        let land = landing(table);
        let mut pose = Vec::new();
        for _ in 0..ticks {
            pose = halo.anim.tick(&set, &label, &i, &land);
            i.hit = None;
        }
        let fixed = Some(chimera_core::later::Later::ready(pose));
        animator.fixed = fixed.clone();
        animator.main = Some(0);
        animator.yaw = None;
        // What the body gains goes on in one move (each part alone is an
        // archetype for every combination of the others: docs/DESIGN.md,
        // "ECS style").
        let made = match sim {
            Some(mut s) => {
                s.0.fixed = fixed;
                s.0.main = Some(0);
                None
            }
            None => {
                let mut s = animator.clone();
                s.fixed = fixed;
                Some(SimAnimator(s))
            }
        };
        match (made, first) {
            (Some(s), true) => {
                commands.entity(e).insert((s, fresh));
            }
            (Some(s), false) => {
                commands.entity(e).insert(s);
            }
            (None, true) => {
                commands.entity(e).insert(fresh);
            }
            (None, false) => {}
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
        // A body with a colour of its own for a player given none (the
        // Elite's minor blue: `load::Tint::uncoloured`) wears it then.
        // Such a body's colour tints its armour all over (the Elite's):
        // the table's black would leave it a silhouette, so it wears its
        // own colour then too.
        let own = m.model.skins.len() > COLOURS.len();
        let index = match (team, skin_of(team.copied())) {
            (None, _) if own => COLOURS.len(),
            (_, BLACK) if own => COLOURS.len(),
            (_, i) => i,
        };
        let want = BodySkin(index as i32);
        if skin != Some(&want) {
            commands.entity(e).insert(want);
        }
    }
}

/// Halo's bodies animated and coloured (module doc).
pub struct HaloBodyPlugin;

impl Plugin for HaloBodyPlugin {
    fn build(&self, app: &mut App) {
        use chimera_net::net::parts::NetAppExt;
        app.add_systems(FixedLast, animate.before(SimPose))
            .add_systems(Update, wear_colours)
            // Deaths (`death`): the server leaves the bodies, every machine
            // plays them.
            .add_systems(
                FixedUpdate,
                death::leave_corpses.after(crate::core::apply_damage).run_if(crate::core::authoritative),
            )
            .add_systems(
                FixedLast,
                (death::play_corpses, death::remove_corpses.run_if(crate::core::authoritative)).chain().before(SimPose),
            )
            .net_part("halo_ce_corpses", death::register);
        #[cfg(feature = "dev")]
        death::dev_commands(app);
    }
}
