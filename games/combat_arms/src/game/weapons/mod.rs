//! Combat Arms' firing (specs/combat_arms/weapon_firing.md): how its guns
//! shoot, on Chimera's weapon frame (`weapon`) with the rest as
//! `combat_arms:*` weapon hooks and the systems around the frame in
//! `core::Predict::Weapons`, as CS:S's accuracy and recoil are.
//!
//! What the frame does from the data (data/weapons/*.toml, docs/weapon-data.md):
//! the trigger (`Trigger`: the fire cycle is the fire animation's length,
//! 105 ms on the M4A1, with `accumulate` timing for the spec's exact-time
//! mode and `set` for the frame-quantised one; `automatic = false` is
//! `RapidFireCount` 1, `per_press` a higher count), bursts (`[alt] kind =
//! "burst"`: three rounds a cycle apart that finish on their own, one burst
//! per pull, the lockout as the next pull's `refire`, the fire-mode toggle's
//! lockout as `step_blocks_primary`), the magazine (clip, the reserve as
//! total ammo minus the clip, partial reloads that lose nothing, refusals,
//! switching away abandoning a reload, `auto_reload`: an empty clip reloads
//! once the last fire cycle ends, trigger held or not) and the round's
//! `Fired` event (`Hitscan` without `DamageEffect`: the frame traces
//! nothing, this module delivers).
//!
//! What this module adds, by hook:
//!
//! - `combat_arms:damage` (`CaDamage`): delivery and damage. Each pellet
//!   draws its own uniform square offset (`square_dir`) of the cone the
//!   spread meters give before this tick's sub-steps. The shooter's test
//!   (`client_test`): characters by their hit spheres (`HitSpheres`: the
//!   box widened 2x on X and Z, hit only when the whole segment passes a
//!   sphere; the sphere with the smallest |p|^2 - r^2 wins,
//!   `pick_sphere`), never through anything; the plain box decides what
//!   is reported (the widened box's entry, or, on a near miss, the thing
//!   behind whose plane the victim stands: Corrections 8). The server's
//!   own walk (`server_walk`, Corrections 4, 5, 9): from the fire position,
//!   at most 4 intersections (surfaces, objects, the reported victim's
//!   box, which it skips), walls passed by their surface (`CaSurfaces`,
//!   data/surfaces.toml: the back-cast from the limit + 1 unit, the restart
//!   at the exit or the entry by the surface, the range left from the fire
//!   position, the damage dampened, a whole-thousandth re-perturb on a
//!   look rotation with world up, `reperturb`), live grenades by their
//!   boxes (the round stops there; its banded damage comes off the
//!   grenade's hit points, grenades.md 13), and ends at the first character
//!   it damages: hit on its plain box, in the sphere of the highest
//!   `Node#Priority` within its radius of the line (`priority_sphere`), or
//!   passed 5 units on. Damage (`base_damage`, `armour_and_zone`):
//!   `InstDamage` x a uniform roll of +-`DamageRFacter` x 1.3 from behind
//!   (`is_behind` on the victim's body yaw, `body_yaw`: Corrections 6; the
//!   reported victim only) x the stepped range band at the reported point
//!   (`band_scale`; beyond the last band: 1) x penetration dampening, an
//!   AI's x `AIDamageFactor` (`CaAiDamage`, `AiHits`), then armour
//!   (Chimera's `weapon::Armor`, its points, with the gear as
//!   `weapon::ArmorPieces`: protection by damage type and location as
//!   coverage, `Gear::coverage`, through the shared armour step
//!   `weapon::hits::absorb`; the groin
//!   unprotected but on an infected victim, armour points) and the
//!   sphere's zone factor, an AI's hit on a player capped (50; 990 at
//!   1000 or more; not melee: Corrections 7); the victim dies at 1 HP or
//!   less. A victim the server's walk finds takes the reported hit's damage
//!   banded again at the reported distance (a second victim), or base x
//!   roll x the band twice (a wall-bang), with no behind bonus. The server
//!   spacing `fireDelay` (`spacing`, Corrections 3) checks a shot's last
//!   ray only, every earlier ray stamping the clock: a multi-ray shot loses
//!   its last ray when it is above 0 (spent all the same). The rays: the
//!   client sends n + floor(n v) by the magazine part's
//!   `VectorsPerRoundPer`, the server expects the muzzle part's; different
//!   counts and the whole shot does nothing (attachments.md Corrections 1).
//!   A shot inside the server's reload window (`server_reload_mark`: the
//!   third-person reload / (min(rate, 2) x (2 - `ReloadCheckTime`)),
//!   `CaServerSettings`) is dropped. An infected victim
//!   (`movement::CaInfected`) of the other team gets the ammo's knock-back
//!   (`movement::HitKnockback`). A hit on a spawn-protected character
//!   (Chimera's `core::SpawnProtected`, a party match's) shows nothing:
//!   the round's events go through `weapon::impact::hide_protected`; in
//!   its bullets window the character isn't among the round's bodies at
//!   all (`Delivery::bodies`), so the round hits what is behind.
//! - `combat_arms:spread` (`CaSpread`): the firing and movement meters on
//!   fixed 1/80 s sub-steps (an exact accumulator keeping the remainder,
//!   Q2's recommendation; `SpreadOptions::quantised` discards it as the
//!   game does), the firing cap by stance, recovery after less than
//!   `DecDelay` whole ms, the movement target by movement.md's run flag,
//!   "in a jump" and ladder, mu against its speed limit (penalties
//!   included), the turn term on the body yaw (`body_turn`: mouse yaw into
//!   the upper body's 45 degree twist first with no movement key held, the
//!   side kick always, pitch never; all on a tick's first sub-step), the
//!   airborne rise (Q12), the cone `n` in thousandths (`cone`), zoomed and
//!   burst values.
//! - `combat_arms:recoil` (`CaRecoil`): the view kick on the owner's
//!   `ViewPunch` (its pitch twice the accumulator, Q4), the side wag with
//!   its 50 % flips, and the return each tick after the frame (so a shot
//!   leaves before this tick's return: Corrections 11), at the waiting rate
//!   for `JitterDecDelay` after a kick, against the burst cap in burst
//!   mode. The state is the player's (`CaKick`: Corrections 1), read with
//!   the weapon in hand's data (the record defaults when it has none).
//!   With another game's weapon in hand (`weapon::impact::RecoilGame`)
//!   the kick ends and neither kicks nor returns touch the punch. The
//!   back push: `BaseCamBackJitter` added to a backward offset per shot,
//!   returned at 5 x `BackJitterRecover` a second, the camera 10 x it
//!   units back along its view (Chimera's `ViewBackPush`: predicted with
//!   the player, not sent).
//! - Pushes: a round that meets a physics object gives it
//!   `IMPULSE_PER_HIT_POINT` N·s a hit point of its damage there
//!   (`weapon::impact::BulletImpulse`, also the damage's force).
//! - `combat_arms:handling` (`CaHandling`): the held weapon's
//!   `MovePenalty`, set on its carrier as the movement's `HeldMovePenalty`
//!   when it is drawn (in the predicted tick, `core::Predict::Select`).
//! - `combat_arms:zoom` (`zoom`, `CaZoom`): zoom and scopes
//!   (weapon_classes.md 7); while zoomed (`CaZoom::zoomed`) the spread uses
//!   its zoomed values (`mode_of`).
//! - `combat_arms:spin`, `combat_arms:alt_fire` and the class rules
//!   (`classes`): what differs between the classes (weapon_classes.md:
//!   shell reloads stopped by fire, minigun spin, alt-fire volleys, the
//!   pellet hit carry-over in the delivery here).
//!
//! Fitted attachments (`attachments`) change these through each part's
//! `mods` (`CaDamage::mods`: the base damage, the special functions on the
//! victim, the shooter's and the server's ray lengths; `CaSpread::mods`;
//! `CaRecoil::mods`), zero for a stock weapon.
//!
//! The cone's arithmetic is weapon_classes.md's "Cone integers": the
//! limits truncated after multiplying by the dynamic factor (a 32-bit
//! value, the products in double precision), the cone truncated again.
//!
//! A network client predicts `CaSpread`, `CaZoom` and `CaSpin` with the
//! weapon (`weapon::NetPart`: the server's state goes with its player's) and
//! `CaKick` with the player (`game::net`).
//!
//! The data's `combat_arms:inventory` table (sprint-out time) is
//! `inventory`'s hook.
//!
//! Per tick (the spec's "Per-tick order"): before the frame, the cone is
//! set from the meters; the frame fires (its first round of a pull
//! `Trigger::latency` updates after the one that entered the firing
//! state, the data's 3, 2 on a primary shotgun: Corrections 10); after
//! it, each round is delivered with that cone, the recoil returns, the
//! spread sub-steps run (the first one a firing one when a round left),
//! then each round kicks the view. Every Combat Arms character gets the
//! default armour (`default_armour`: 100 points, the default body gear)
//! when it first has health, its gear follows its game identity
//! (`armour_follows_identity`: on Combat Arms' characters only), and is hit through `HitSpheres`: its skeleton's hit table
//! (characters.md 11: `MALE`, `FEMALE` from data/characters.toml, set by
//! `characters` from the body's skeleton; the male one when it has none),
//! each sphere on its node or socket of the posed skeleton: a body
//! model's own pose (`characters::CaPose`, `posed_geometry`), else the
//! spec's measured skeleton crouched by the box's height and pitched by
//! the look (`geometry_at`). Those spheres are Chimera's hit shapes
//! (`sphere_shapes`: `core::Hitbox` spheres with region, priority and zone
//! factor) and the tests above are the shapes' own (`Hitboxes::by_power`,
//! `by_priority`: a box answers as the box, not its bounding sphere), so
//! the same rules hit another game's body through its
//! `core::Hitboxes` (a character of another identity that has them,
//! rewound with lag compensation: `body_shapes`, `their_shapes`; a CS:S
//! body's boxes, the character's box widened to hold them, `box_holding`,
//! since a taller model's head pokes out of the hull, where the box tests
//! would never reach it), its
//! zone factor by region then (`zone_factor`, the male skeleton's) and its
//! armour location by part (`location_of`;
//! docs/DESIGN.md "Hits across games").
//!
//! Units: the data's (world units, hit points; `units` in each document).
//!
//! Known shortcuts (the spec's most literal reading where it is unclear,
//! listed for its open questions):
//! - The scale is movement's `meters_per_unit` (the data's `units`; by
//!   body size: movement.md Q3).
//! - The level's polygons are open meshes here, not the original's solid
//!   BSP: a round passing a wall of the level's own surface (Unknown
//!   included, effects_sounds.md Follow-up (2026-10-31b) 3) also needs the
//!   wall's far side within the limit + 1 unit ahead, so a single polygon
//!   or a thick wall of the level stops it; a collider with a surface of
//!   its own (a glass set, a prop, a world model) passes by the back-cast
//!   alone.
//! - Without a body model (no V2 key) the spheres sit on the spec's
//!   skeleton as measured at `LST` t = 0 (characters.md 11), squashed
//!   towards the feet when crouched and pitched by the look, the whole
//!   body turned to the view (no upper-body twist); they don't follow
//!   the legs' or the weapon's animation (`characters` Known shortcuts).
//!   The groin sphere (`Nuts`) has location 5, as this spec and the BR
//!   data have it (the US data: 2; characters.md Q1b).
//! - The spec's client and server halves aren't split by process: the
//!   "client" test and the "server" walk both run where the round is
//!   delivered. In a network game the shooter's client predicts the round
//!   (its meters, kick and events; no damage) and the server runs both
//!   against where the shooter saw the others (lag compensation,
//!   `weapon::lagcomp`, their boxes, looks and posed spheres then); nothing the client
//!   found is sent (no client-reported victim). The fire delay compares
//!   the rounds' tick times (the client's timestamps). The server's clip
//!   isn't separate: a shot dropped in its reload window spends the
//!   client's round, and nothing refills at the window's end.
//! - A wall-bang's band is applied twice at the real distance (Q13's
//!   recommendation for the first).
//! - The side direction is drawn at the first side kick (the game draws it
//!   as the player enters the map), from the command's shared random.
//! - The spread runs per tick (the spec's per frame), its sub-steps on an
//!   exact 80 Hz clock (`SpreadOptions::quantised` drops the remainder
//!   as the game does); the body's twist is ours to keep (`body_yaw`).
//! - What a round passes is the hit collider's own surface
//!   (`effects::owned_surface`: a prop's `PropSurface`, a glass set's on a
//!   Combat Arms map, a brush entity's `map::BrushSurface` such as a CS:S
//!   window's glass or a world model's type number), else the
//!   world's surface type where it hit (the drawn triangles' types,
//!   `effects::world_surface`) by name in the table; on another game's map
//!   (a mashup) its surface names are Combat Arms' through the app's table
//!   (`map::SurfaceNames`). A surface's "flags" for the restart point are
//!   its name. A round that hits something with `core::Damageable` that
//!   isn't a character (a map's breakable) deals it the round's damage
//!   (banded and dampened, no zone) and, on the server's walk, goes on as
//!   the surface lets it.
//! - Armour: the gear (which pieces a character wears, Q7) is the default
//!   body gear for everyone, with the spec's stock coverage (bullets and
//!   burn on body gear, bullets on head gear, poison on the gas mask);
//!   pieces go over the network by name (`GEARS`). Figure modifiers are 0
//!   (both figure types). Nothing marks a character infected yet (no game
//!   modes).
//! - The back push moves the camera only: shots still leave from the eye
//!   (the spec's first-person ray starts at the pushed camera), and it
//!   steps per tick, not per frame.
//! - Not done: continuous fire, perk and set bonuses (Q8:
//!   identity in a plain match), the fire-mode key (the toggle is attack2
//!   until the binds come), the out-of-ammo voice line. The pre-fire is the
//!   trigger's latency, not an animation (the minigun's spin is,
//!   `classes`); an alt-fire volley has none.

//!
//! Slots and switching are `inventory`'s, thrown weapons `grenades`',
//! attachments `attachments`', an install copy's own fire cycle, reload
//! and draw from its view model `timing`'s.

pub mod classes;
pub mod harm;
pub mod power;
pub mod timing;
pub mod zoom;

use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock},
};

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::{character::CharacterPartsExt, later::Later};
use chimera_sim::server_docs::DocsAppExt;
use serde::{Deserialize, Serialize};

use crate::{
    core::{
        Damage, DamageKind, DamageRules, Damageable, FirstTimePredicted, Health, Hitgroup, Intent, MovementState, NetRole,
        Predict, PredictedAppExt, PropSurface, SOLID_LAYERS, SimClock, Velocity,
    },
    map::{SurfaceNames, sound::SurfaceGrid},
    weapon::{
        AltModes, Burst, CarriedAppExt, Hitscan, ParamsAppExt, Inventory, NetPart, SpreadShape, ViewBackPush, ViewPunch,
        WeaponEvent,
        Armor, ArmorPieces, WeaponEventKind, WeaponFrame, Zoom, aim_of,
        hits,
        data::{WeaponData, WeaponDataAppExt},
        lagcomp,
        random::{Rng, shared_random},
    },
};

use super::characters;
use super::attachments::{CaNpc, CaSpecialRoll, DamageMods, RecoilMods, SpecialKind, SpreadMods, Victim};

pub use self::{
    classes::{ALT_FIRE, CaAltFire, CaAltVolley, CaSpin, CarryOver, PelletCarryOver, SPIN, SpinParams, SpinState, reported_hits},
    zoom::{CaFocusBlur, CaZoom, CaZoomSound, Scope, Sway, ZOOM, ZoomParams},
};


mod armour;
mod hitspheres;
mod params;
mod spread_recoil;
mod systems;
#[cfg(test)]
mod tests;

pub use armour::*;
pub use hitspheres::*;
pub use params::*;
pub use spread_recoil::*;
pub use systems::*;

/// The M4A1 (data/weapons/m4a1.toml): the rifle players start with.
pub const RIFLE: &str = "combat_arms:m4a1";

/// Hook names (docs/weapon-data.md, "Hooks").
pub const DAMAGE: &str = "combat_arms:damage";
pub const SPREAD: &str = "combat_arms:spread";
pub const RECOIL: &str = "combat_arms:recoil";
pub const HANDLING: &str = "combat_arms:handling";

/// `combat_arms:handling`: what holding the weapon does to its carrier.
#[derive(Component, Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct CaHandling {
    /// `MovePenalty`: the fraction of speed lost while held (the
    /// movement's `HeldMovePenalty`).
    pub move_penalty: f32,
}

/// The weapon whose handling a character's `HeldMovePenalty` was last set
/// from. Both on every character from the spawn (a part that comes and
/// goes is a new archetype for every combination of the rest).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(storage = "SparseSet")]
struct HandlingOf(Option<Entity>);

/// On drawing a weapon, its `MovePenalty` becomes the carrier's
/// `HeldMovePenalty` (none without `CaHandling`); set once per draw, so a
/// test or a mod may change it while the weapon is held.
fn apply_handling(
    owners: Query<(Entity, &Inventory, Option<&HandlingOf>)>,
    handling: Query<&CaHandling>,
    mut commands: Commands,
) {
    for (owner, inv, last) in &owners {
        if last.map(|l| l.0) == Some(inv.active) {
            continue;
        }
        let mut e = commands.entity(owner);
        e.insert(HandlingOf(inv.active));
        // None: no penalty (its default).
        let penalty = inv.active.and_then(|w| handling.get(w).ok()).map_or(0.0, |h| h.move_penalty);
        e.insert(super::movement::HeldMovePenalty {
            penalty,
            mod_factor: 0.0,
        });
    }
}

// ---------------------------------------------------------------------------
// Constants (the spec's "Constants": fixed in code)

/// Spread meters advance in steps of this, s.
pub const SPREAD_SUBSTEP: f64 = 1.0 / 80.0;
/// Most sub-steps in one tick.
pub const SPREAD_SUBSTEP_MAX: u32 = 100;
/// Damage multiplier from behind (client-reported hits only).
pub const BEHIND_BONUS: f32 = 1.3;
/// A character dies at this many hit points or fewer after a hit.
pub const DEATH_THRESHOLD: f32 = 1.0;
/// Horizontal widening of a character's box in the first ray test.
pub const HITBOX_WIDEN: f32 = 2.0;
/// Most intersections the server's own walk makes (every surface, object
/// or skipped character box counts one; Corrections 4).
pub const SERVER_INTERSECTIONS: u32 = 4;
/// How far past a character box the server's walk restarts when its line
/// is within none of the spheres (units; Corrections 5).
pub const SERVER_SKIP: f32 = 5.0;

/// Hit points in one unit of `Health` (1.0 = 100 HP).
pub const HIT_POINTS: f32 = 100.0;

/// A round's push on a physics object, N·s per hit point of the damage
/// it deals there (banded and dampened, before the victim): Combat Arms
/// has no physics objects, so no spec gives one; ours is CS:S's 9 mm
/// ratio (2000 kg·in/s for the Glock's 25 HP, 50.8 N·s: about 2 N·s per
/// HP; its .50 AE and rifle rounds give 1.6-1.7), so a Combat Arms gun
/// pushes a CS:S prop about as a CS:S gun of its damage does
/// (`weapon::impact::BulletImpulse`).
pub const IMPULSE_PER_HIT_POINT: f32 = 2.0;
/// `DamageRFacter` used when the data's is outside [0, 1].
pub const DAMAGE_ROLL_INVALID: f32 = 0.2;
/// Damage types (`InstDamageType`) the armour step knows.
pub const BULLET: i32 = 2;
pub const BURN: i32 = 3;
pub const EXPLOSION: i32 = 7;
pub const POISON: i32 = 9;
/// A flash grenade's damage type (protection only: it does no damage).
pub const FLASH: i32 = 34;
pub const MELEE: i32 = 13;
/// How far mouse yaw turns the upper body before the body yaw follows,
/// with no movement key held (half of `PlayerTwistYawMax` 90 degrees).
pub const TWIST_MAX: f32 = std::f32::consts::FRAC_PI_4;
/// Stance speeds for the movement fraction mu without Combat Arms'
/// movement (movement.md, u/s).
pub const RUN_SPEED: f32 = 285.0;
pub const WALK_SPEED: f32 = 70.0;
pub const CROUCH_SPEED: f32 = 50.0;
pub const LADDER_SPEED: f32 = 200.0;

/// AI damage (weapon_firing.md Corrections 7, attribute_data.md
/// Corrections 3): the server console variable `AIDamageFactor` (built-in
/// 0.8; a Hired Guns match or a mission sets its objective's) on every hit
/// an AI (a bot, a mercenary) deals, before armour; then, on a player
/// (not an AI) and not melee, one hit's health damage capped: `cap` (50),
/// or `cap_big` (990) when it was `big` (1000) or more.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct CaAiDamage {
    pub factor: f32,
    pub cap: f32,
    pub cap_big: f32,
    pub big: f32,
}

impl Default for CaAiDamage {
    fn default() -> Self {
        Self {
            factor: 0.8,
            cap: 50.0,
            cap_big: 990.0,
            big: 1000.0,
        }
    }
}

impl CaAiDamage {
    /// One AI hit's health damage `hp` (HP) on a player after armour,
    /// capped (a melee hit isn't).
    pub fn capped(&self, hp: f32, melee: bool) -> f32 {
        if melee {
            hp
        } else if hp >= self.big {
            self.cap_big
        } else {
            hp.min(self.cap)
        }
    }
}

/// Who a hit is from and on, for the AI damage rule (`CaAiDamage`): AIs
/// are bots and mercenaries (`core::Bot`, which `bots::CaBot` brings); a
/// player is a character that
/// is neither an AI nor a non-player (`core::Npc`).
#[derive(bevy::ecs::system::SystemParam)]
pub struct AiHits<'w, 's> {
    ai: Query<'w, 's, (), With<crate::core::Bot>>,
    npcs: Query<'w, 's, (), With<crate::core::Npc>>,
    settings: Res<'w, CaAiDamage>,
}

impl AiHits<'_, '_> {
    /// The factor on `attacker`'s damage before armour (1: not an AI).
    pub fn factor(&self, attacker: Option<Entity>) -> f32 {
        if attacker.is_some_and(|a| self.ai.contains(a)) { self.settings.factor } else { 1.0 }
    }

    /// `points` (HP, after armour and zone) of `attacker` on `victim`,
    /// with the AI cap on a player.
    pub fn cap(&self, attacker: Option<Entity>, victim: Entity, points: f32, melee: bool) -> f32 {
        let ai = attacker.is_some_and(|a| a != victim && self.ai.contains(a));
        let player = !self.ai.contains(victim) && !self.npcs.contains(victim);
        if ai && player { self.settings.capped(points, melee) } else { points }
    }
}

/// The global spread scales (the weapons file's `[Perturb]` section, as the
/// client uses them after swapping firing and movement, Q1) and the turn
/// constants.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct SpreadScales {
    pub fire_inc: f32,
    pub fire_dec: f32,
    pub move_inc: f32,
    pub move_dec: f32,
    pub turn_effect: f32,
    pub fast_turn_rate: f32,
}

impl Default for SpreadScales {
    fn default() -> Self {
        Self {
            fire_inc: 3.0,
            fire_dec: 3.0,
            move_inc: 3.0,
            move_dec: 9.0,
            turn_effect: 3.0,
            fast_turn_rate: 2.3,
        }
    }
}

/// How the spread sub-steps keep time.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct SpreadOptions {
    /// Discard the remainder each tick, as the game does at its frame rate
    /// (Q2); off: an exact 80 Hz clock.
    pub quantised: bool,
}

