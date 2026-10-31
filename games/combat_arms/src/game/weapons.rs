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
//!   the kick ends and neither kicks nor returns touch the punch.
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
//! `by_priority`), so the same rules hit another game's body through its
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
//!   `weapon::lagcomp`, their boxes and looks then); nothing the client
//!   found is sent (no client-reported victim). The fire delay compares
//!   the rounds' tick times (the client's timestamps). The server's clip
//!   isn't separate: a shot dropped in its reload window spends the
//!   client's round, and nothing refills at the window's end.
//! - The hit priorities (`Node#Priority`) are ours by location
//!   (data/characters.toml `hit_priority`: torso and groin 3, head 2,
//!   limbs 1; the stock values are unknown). A wall-bang's band is applied
//!   twice at the real distance (Q13's recommendation for the first).
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
//! - Not done: the back push (`BaseCamBackJitter`) and the view-model
//!   jiggle (presentation), continuous fire, perk and set bonuses (Q8:
//!   identity in a plain match), the fire-mode key (the toggle is attack2
//!   until the binds come), the out-of-ammo voice line. The pre-fire is the
//!   trigger's latency, not an animation (the minigun's spin is,
//!   `classes`); an alt-fire volley has none.

//!
//! Slots and switching are `inventory`'s, thrown weapons `grenades`',
//! attachments `attachments`', an install copy's own fire cycle, reload
//! and draw from its view model `timing`'s.

pub mod classes;
pub mod power;
pub mod timing;
pub mod zoom;

use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock},
};

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_sim::server_docs::DocsAppExt;
use serde::{Deserialize, Serialize};

use crate::{
    core::{
        Damage, DamageKind, DamageRules, Damageable, FirstTimePredicted, Health, Hitgroup, Intent, MovementState, NetRole,
        Predict, PredictedAppExt, PropSurface, SOLID_LAYERS, SimClock, Velocity,
    },
    map::{SurfaceNames, sound::SurfaceGrid},
    weapon::{
        AltModes, Burst, CarriedAppExt, Hitscan, ParamsAppExt, Inventory, NetPart, SpreadShape, ViewPunch, WeaponEvent,
        Armor, ArmorPieces, WeaponEventKind, WeaponFrame, Zoom, aim_of,
        hits,
        data::{WeaponData, WeaponDataAppExt},
        lagcomp::{self, HitHistory, LagCompSettings, LagCompStats, ViewTick},
        random::{Rng, shared_random},
    },
};

use super::characters;
use super::attachments::{CaNpc, CaSpecialRoll, DamageMods, RecoilMods, SpecialKind, SpreadMods, Victim};

pub use self::{
    classes::{ALT_FIRE, CaAltFire, CaSpin, CarryOver, PelletCarryOver, SPIN, SpinParams, SpinState, reported_hits},
    zoom::{CaZoom, Scope, Sway, ZOOM, ZoomParams},
};

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
/// from.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
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
        match inv.active.and_then(|w| handling.get(w).ok()) {
            Some(h) => {
                e.insert(super::movement::HeldMovePenalty {
                    penalty: h.move_penalty,
                    mod_factor: 0.0,
                });
            }
            None => {
                e.remove::<super::movement::HeldMovePenalty>();
            }
        }
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

// ---------------------------------------------------------------------------
// Hook parameters (data)

fn one() -> f32 {
    1.0
}

/// `combat_arms:damage`: damage per ray and the server's spacing.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageParams {
    /// `InstDamage` (ammo), hit points (the document's damage units).
    pub damage: f32,
    /// `InstDamageType` (ammo): 2 is bullet (the only one armour knows yet).
    #[serde(default = "bullet_type")]
    pub damage_type: i32,
    /// `DamageRFacter` (weapon): each hit x U(1 - roll, 1 + roll).
    #[serde(default = "default_roll")]
    pub roll: f32,
    /// `Effectrange0..2` (weapon): band edges, length units (truncated to
    /// whole units).
    #[serde(default)]
    pub effect_range: Vec<f32>,
    /// `Rangedamage0..2` (ammo): each band's multiplier.
    #[serde(default)]
    pub range_damage: Vec<f32>,
    /// `fireDelay` (weapon), ms.
    #[serde(default = "default_fire_delay")]
    pub fire_delay: f32,
    /// `DamageMovePenalty` (ammo): the knock-back strength K on an
    /// infected victim (movement.md, "Hit knock-back").
    #[serde(default)]
    pub move_penalty: f32,
    /// `DamageMovePenaltyDuration` (ammo), s; not above 0: none.
    #[serde(default = "no_duration")]
    pub move_penalty_duration: f32,
    /// The third-person reload's length at rate 1 (s; by stance in the
    /// game, ours one): the server's reload window (weapon_classes.md
    /// Corrections 2). None: the magazine's reload.
    #[serde(default)]
    pub reload_third_person: Option<f32>,
}

fn bullet_type() -> i32 {
    BULLET
}
fn no_duration() -> f32 {
    -1.0
}
fn default_roll() -> f32 {
    0.1
}
fn default_fire_delay() -> f32 {
    30.0
}

/// A cone's limits (spread units, thousandths) and the meter's speeds.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConeParams {
    /// `MinPerturb`, `MaxPerturb`.
    pub min: i32,
    pub max: i32,
    /// `PerturbIncSpeed`, `PerturbDecSpeed`.
    pub inc: f32,
    pub dec: f32,
    /// The weapon's zoomed rates are in use (`CaSpread::cone_params` in
    /// zoomed mode): the attachments' zoomed terms apply.
    #[serde(skip)]
    pub zoomed: bool,
}

/// The ammo's state values (`Fire*Perturb` caps, movement targets).
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct StateParams {
    pub fire: f32,
    pub fire_move: f32,
    pub fire_duck: f32,
    pub fire_move_duck: f32,
    pub walk: f32,
    pub run: f32,
    pub jump: f32,
}

/// `combat_arms:spread`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpreadParams {
    pub cone: ConeParams,
    /// Zoomed values (`Zoomed*`), used while zoomed where non-zero; the
    /// record defaults (1/500, 3.0/3.0) when the document leaves them out
    /// (attribute_data.md Corrections 2).
    #[serde(default = "zoomed_default")]
    pub zoomed_cone: Option<ConeParams>,
    /// Burst values (`TripleShot*Perturb*`), used in burst mode where
    /// non-zero, field by field; the record defaults (7/100, 1.0/1.0) when
    /// left out.
    #[serde(default = "burst_default")]
    pub burst_cone: Option<ConeParams>,
    /// `DynamicPerturbFactor`.
    #[serde(default = "one")]
    pub dynamic_factor: f32,
    pub state: StateParams,
    /// The ammo's `Zoomed*Perturb` values, used while zoomed where non-zero.
    #[serde(default)]
    pub zoomed_state: StateParams,
    /// `DecDelay` (ammo), ms.
    #[serde(default = "default_dec_delay")]
    pub dec_delay: f32,
}

fn default_dec_delay() -> f32 {
    150.0
}

/// The record defaults of `ZoomedMinPerturb`/`MaxPerturb` and the zoomed
/// speeds (attribute_data.md, Field table: weapons).
fn zoomed_default() -> Option<ConeParams> {
    Some(ConeParams {
        min: 1,
        max: 500,
        inc: 3.0,
        dec: 3.0,
        zoomed: false,
    })
}

/// The record defaults of the `TripleShot*Perturb*` fields.
fn burst_default() -> Option<ConeParams> {
    Some(ConeParams {
        min: 7,
        max: 100,
        inc: 1.0,
        dec: 1.0,
        zoomed: false,
    })
}

/// `combat_arms:recoil` (degrees but the two random thresholds, radians;
/// delays ms; rates radians/s). Defaults: attribute_data.md's.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct RecoilParams {
    /// `BaseCamRecoilPitch`.
    pub pitch: f32,
    /// `BaseCamRecoilAccelPitch`, per radian accumulated.
    pub accel_pitch: f32,
    /// `MaxCamRecoilPitch`.
    pub max_pitch: f32,
    /// `MaxRandomPitch`, radians.
    pub random_pitch: f32,
    /// `BaseCamRecoilWagR`, `BaseCamRecoilWagL`.
    pub wag_r: f32,
    pub wag_l: f32,
    /// `MaxCamRecoilWag`.
    pub max_wag: f32,
    /// `MaxRandomwag`, radians.
    pub random_wag: f32,
    /// `BaseCamBackJitter`, `BackJitterRecover` (not used yet).
    pub back_jitter: f32,
    pub back_jitter_recover: f32,
    /// `TripleShotBaseCamRecoilPitch`, `TripleShotMaxCamRecoilPitch`: the
    /// kick's in burst mode when both are non-zero, the return's cap when
    /// the cap is (record defaults 1.0 and 50.0, attribute_data.md
    /// Corrections 2).
    pub burst_pitch: f32,
    pub burst_max_pitch: f32,
    /// `JitterDecDelay` (ammo), ms.
    pub jitter_dec_delay: f32,
    /// `JitterWaitRecover`, `JitterRecover` (ammo).
    pub jitter_wait_recover: f32,
    pub jitter_recover: f32,
}

impl Default for RecoilParams {
    fn default() -> Self {
        Self {
            pitch: 1.2,
            accel_pitch: 0.0,
            max_pitch: 10.0,
            random_pitch: 0.032,
            wag_r: 1.0,
            wag_l: -1.0,
            max_wag: 30.0,
            random_wag: 0.015,
            back_jitter: 0.5,
            back_jitter_recover: 3.0,
            burst_pitch: 1.0,
            burst_max_pitch: 50.0,
            jitter_dec_delay: 600.0,
            jitter_wait_recover: 0.1,
            jitter_recover: 0.3,
        }
    }
}

// ---------------------------------------------------------------------------
// Parts

/// A weapon's damage (`combat_arms:damage`), in engine units.
#[derive(Component, Clone, Debug)]
pub struct CaDamage {
    /// Per ray, health (1.0 = 100 HP).
    pub damage: f32,
    pub damage_type: i32,
    /// The random roll's half-width (already 0.2 when the data's was
    /// outside [0, 1]).
    pub roll: f32,
    /// (upper edge in metres, multiplier), three bands; none: no banding.
    pub bands: Option<[(f32, f32); 3]>,
    /// The server's spacing, s.
    pub fire_delay: f64,
    /// Metres per the data's length unit.
    pub unit: f32,
    /// When the last round that passed the server's spacing arrived (for
    /// `fire_delay`).
    pub last_round: Option<f64>,
    /// The knock-back on an infected victim: K and its duration (s).
    pub move_penalty: f32,
    pub move_penalty_duration: f32,
    /// The fitted attachments' terms (`attachments`).
    pub mods: DamageMods,
    /// The third-person reload's length at rate 1, s.
    pub reload_third_person: f32,
    /// The server's reload window ends then (weapon_classes.md
    /// Corrections 2): shots before it are dropped.
    pub reload_mark: Option<f64>,
}

/// The server's reload window (weapon_classes.md 5.6, Corrections 2): the
/// third-person reload `length` (s) / (min(`rate`, 2) x (2 -
/// `ReloadCheckTime`)); the rate is 1 + the reload bonuses, capped at 2
/// (not the client's "above 2 plays at 1").
pub fn server_reload_mark(length: f64, rate: f64, check: f64) -> f64 {
    length / (rate.min(2.0) * (2.0 - check))
}

/// The server's settings firing reads (attribute_data.md Corrections 3):
/// `ReloadCheckTime` (the install's `[TimeWeight]`, else 0.8).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct CaServerSettings {
    pub reload_check_time: f64,
}

impl CaServerSettings {
    /// The settings `data` gives.
    pub fn of(data: &super::data::CaData) -> Self {
        Self {
            reload_check_time: data.reload_check_time(),
        }
    }
}

impl CaDamage {
    pub fn new(p: &DamageParams, unit: f32, health_per_point: f32) -> Self {
        let bands = (p.effect_range.len() >= 3 && p.range_damage.len() >= 3).then(|| {
            std::array::from_fn(|i| (p.effect_range[i].trunc() * unit, p.range_damage[i]))
        });
        Self {
            damage: p.damage * health_per_point,
            damage_type: p.damage_type,
            roll: if (0.0..=1.0).contains(&p.roll) { p.roll } else { DAMAGE_ROLL_INVALID },
            bands,
            fire_delay: p.fire_delay.max(0.0) as f64 / 1000.0,
            unit,
            last_round: None,
            move_penalty: p.move_penalty,
            move_penalty_duration: p.move_penalty_duration,
            mods: DamageMods::default(),
            reload_third_person: p.reload_third_person.unwrap_or(0.0),
            reload_mark: None,
        }
    }
}

/// Which values a weapon's spread and recoil use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireMode {
    Normal,
    Zoomed,
    Burst,
}

/// The mode of a weapon from its attack2 mode (`AltModes`): burst when its
/// `Burst` is on, zoomed in a scope's modes.
pub fn fire_mode(mode: u8, burst: Option<&Burst>, zoom: Option<&Zoom>) -> FireMode {
    match (burst, zoom) {
        (Some(b), _) if b.mode == mode && b.count > 0 => FireMode::Burst,
        (_, Some(_)) if mode > 0 => FireMode::Zoomed,
        _ => FireMode::Normal,
    }
}

/// `fire_mode`, zoomed whenever Combat Arms' own zoom is (`CaZoom::zoomed`:
/// a level or a transition, weapon_classes.md 7).
pub fn mode_of(mode: u8, burst: Option<&Burst>, zoom: Option<&Zoom>, ca_zoom: Option<&CaZoom>) -> FireMode {
    if ca_zoom.is_some_and(CaZoom::zoomed) {
        FireMode::Zoomed
    } else {
        fire_mode(mode, burst, zoom)
    }
}

/// A weapon's spread (`combat_arms:spread`): the data and the two meters.
#[derive(Component, Clone, Debug)]
pub struct CaSpread {
    pub params: SpreadParams,
    /// Metres per the data's length unit (for speeds).
    pub unit: f32,
    /// The firing meter F and the movement meter M.
    pub fire: f32,
    pub movement: f32,
    /// When the last shot stamped the firing meter.
    pub last_shot: Option<f64>,
    /// Time not yet stepped, s.
    pub clock: f64,
    /// The body's (pitch, yaw) at the last tick (pitch: 0, never measured).
    pub prev_view: Option<Vec2>,
    /// The body yaw from the mouse (without the side kick): it lags the
    /// view by the upper body's twist.
    pub body_yaw: Option<f32>,
    /// Airborne after a jump (until landing).
    pub jumped: bool,
    pub was_on_ground: bool,
    /// The fitted attachments' terms (`attachments`; zero: stock).
    pub mods: SpreadMods,
    /// The perks' and sets' cuts of the cone's limits (`perks`, `gear`):
    /// Eagle Eye's fraction of both, a set's fraction of the maximum.
    pub perk_cut: f32,
    pub set_max_cut: f32,
}

impl CaSpread {
    pub fn new(params: SpreadParams, unit: f32) -> Self {
        Self {
            params,
            unit,
            fire: 0.0,
            movement: 0.0,
            last_shot: None,
            clock: 0.0,
            prev_view: None,
            body_yaw: None,
            jumped: false,
            was_on_ground: true,
            mods: SpreadMods::default(),
            perk_cut: 0.0,
            set_max_cut: 0.0,
        }
    }

    /// The cone limits and speeds in `mode` (zoomed ones where non-zero).
    pub fn cone_params(&self, mode: FireMode) -> ConeParams {
        let base = self.params.cone;
        match mode {
            FireMode::Normal => base,
            // Each burst field replaces its plain one when non-zero
            // (Corrections 12).
            FireMode::Burst => match self.params.burst_cone {
                Some(z) => ConeParams {
                    min: if z.min != 0 { z.min } else { base.min },
                    max: if z.max != 0 { z.max } else { base.max },
                    inc: if z.inc != 0.0 { z.inc } else { base.inc },
                    dec: if z.dec != 0.0 { z.dec } else { base.dec },
                    zoomed: false,
                },
                None => base,
            },
            FireMode::Zoomed => match self.params.zoomed_cone {
                Some(z) => ConeParams {
                    min: if z.min != 0 { z.min } else { base.min },
                    max: if z.max != 0 { z.max } else { base.max },
                    inc: if z.inc != 0.0 { z.inc } else { base.inc },
                    dec: if z.dec != 0.0 { z.dec } else { base.dec },
                    zoomed: true,
                },
                None => base,
            },
        }
    }

    /// The state values in `mode` (zoomed ones where non-zero, alone),
    /// with the attachments' terms (attachments.md, "Spread"): the walk
    /// and run targets and the still firing caps (standing, crouched) add
    /// theirs; jumping and the moving caps have none.
    pub fn states(&self, mode: FireMode) -> StateParams {
        let (b, z, m) = (self.params.state, self.params.zoomed_state, self.mods);
        let zoomed = mode == FireMode::Zoomed;
        let pick = |z: f32, b: f32, add: f32| if zoomed && z != 0.0 { z } else { b + add };
        StateParams {
            fire: pick(z.fire, b.fire, m.fire),
            fire_move: pick(z.fire_move, b.fire_move, 0.0),
            fire_duck: pick(z.fire_duck, b.fire_duck, m.fire_duck),
            fire_move_duck: pick(z.fire_move_duck, b.fire_move_duck, 0.0),
            walk: pick(z.walk, b.walk, m.walk),
            run: pick(z.run, b.run, m.run),
            jump: pick(z.jump, b.jump, 0.0),
        }
    }

    /// The spread fraction p = max(F, M) (0..1) a shot fired now uses: the
    /// HUD's r.
    pub fn fraction(&self) -> f32 {
        self.fire.max(self.movement)
    }

    /// The cone `n` (thousandths) a shot fired now gets in `mode`, from
    /// the meters as they stand: trunc(n_min + floor((n_max - n_min) p) g),
    /// each limit truncated after multiplying by g (weapon_classes.md,
    /// "Cone integers"; Q7: g the 32-bit value, the products in double
    /// precision, so 50 x 1.3 is 64).
    /// With attachments (attachments.md, "Cone"): g adds their
    /// `DynamicPerturbFactor`; zoomed, a non-zero zoomed limit adds their
    /// `ZoomedMinPerturb` / `ZoomedMaxPerturb`, truncated.
    pub fn cone(&self, mode: FireMode) -> f32 {
        let c = self.cone_params(mode);
        let m = self.mods;
        let (mut c_min, mut c_max) = (c.min as f64, c.max as f64);
        if mode == FireMode::Zoomed
            && let Some(z) = self.params.zoomed_cone
        {
            if z.min != 0 {
                c_min = (z.min as f64 + m.zoomed_min as f64).trunc();
            }
            if z.max != 0 {
                c_max = (z.max as f64 + m.zoomed_max as f64).trunc();
            }
        }
        // Eagle Eye (gear_perks.md 14): both limits lose a tenth, whole
        // units; a set's spread effect cuts the maximum (12).
        if self.perk_cut > 0.0 {
            let cut = self.perk_cut as f64;
            c_min -= (c_min * cut).floor();
            c_max -= (c_max * cut).floor();
        }
        if self.set_max_cut > 0.0 {
            c_max = (c_max * (1.0 - self.set_max_cut as f64)).trunc();
        }
        let g = (self.params.dynamic_factor + m.dynamic) as f64;
        let max = (c_max * g).trunc();
        let min = (c_min * g).trunc().min(max);
        if max <= 0.0 {
            return 0.0;
        }
        let p = self.fraction() as f64;
        (min + ((max - min) * p).floor() * g).trunc() as f32
    }

    /// One firing-meter sub-step at `now`: a shot grows it toward `cap`
    /// and stamps the time; otherwise it waits while less than `DecDelay`
    /// whole milliseconds have passed since the stamp, then recovers (the
    /// first recovery step clears the stamp). The attachments' rates
    /// (attachments.md, "Firing meter"): growth at I + [zoomed] Z_dec +
    /// sum inc (Q4: the zoomed decrease sum), recovery at R + [zoomed]
    /// Z_dec + sum dec.
    pub fn fire_substep(&mut self, scales: &SpreadScales, cone: ConeParams, fired: bool, cap: f32, now: f64) {
        let dt = SPREAD_SUBSTEP as f32;
        let m = self.mods;
        let z_dec = if cone.zoomed { m.zoomed_dec } else { 0.0 };
        if fired {
            self.fire = cap.min(self.fire + dt * scales.fire_inc * (cone.inc + z_dec + m.inc));
            self.last_shot = Some(now);
        } else if self
            .last_shot
            .is_some_and(|t| (((now - t) * 1000.0) + 1e-6).floor() < self.params.dec_delay as f64)
        {
        } else {
            self.last_shot = None;
            if self.fire > 0.0 {
                self.fire = (self.fire - dt * scales.fire_dec * (cone.dec + z_dec + m.dec)).max(0.0);
            }
        }
    }

    /// One movement-meter sub-step toward `target` (S), with `mu_s` (mu S)
    /// and the turn term `rho`; `jumping`: airborne after a jump.
    #[allow(clippy::too_many_arguments)]
    pub fn move_substep(
        &mut self,
        scales: &SpreadScales,
        cone: ConeParams,
        target: f32,
        mu_s: f32,
        rho: f32,
        jumping: bool,
        jump_cap: f32,
    ) {
        let dt = SPREAD_SUBSTEP as f32;
        let a = rho.max(mu_s);
        let m = self.movement;
        // The attachments' terms (attachments.md, "Movement meter"): rising,
        // the rate x (1 + sum inc) and, zoomed, + Z_inc a sub-step; falling,
        // R + [zoomed] Z_dec + sum dec; in a jump, I + [zoomed] Z_inc +
        // sum inc.
        let k = self.mods;
        let (z_inc, z_dec) = if cone.zoomed { (k.zoomed_inc, k.zoomed_dec) } else { (0.0, 0.0) };
        if a > m {
            let grown = m + (dt * scales.move_inc).min(a - m) * cone.inc * (1.0 + k.inc) + z_inc;
            self.movement = grown.min(target).min(mu_s);
        } else if a < m {
            let next = if jumping {
                // Q12: in a jump it rises instead, by the decrease step at
                // the increase speed.
                m + (dt * scales.move_dec).min(m - a) * (cone.inc + z_inc + k.inc)
            } else {
                m - (dt * scales.move_dec).min(m - a) * (cone.dec + z_dec + k.dec)
            };
            self.movement = next.min(jump_cap).max(0.0);
        }
    }

    /// The body yaw's change this tick (Q13): mouse yaw `mouse` turns the
    /// upper body first, up to `TWIST_MAX` either way, while no movement
    /// key is held (`moving` off); the body follows only beyond that (with
    /// a key held, at once). The recoil's side kick `wag` (view yaw) always
    /// turns it. Pitch never counts.
    pub fn body_turn(&mut self, mouse: f32, wag: f32, moving: bool) -> f32 {
        let body = self.body_yaw.get_or_insert(mouse);
        if moving {
            *body = mouse;
        } else {
            let twist = wrap_angle(mouse - *body);
            if twist > TWIST_MAX {
                *body = mouse - TWIST_MAX;
            } else if twist < -TWIST_MAX {
                *body = mouse + TWIST_MAX;
            }
        }
        let yaw = *body + wag;
        let turned = self.prev_view.map_or(0.0, |p| wrap_angle(yaw - p.y));
        self.prev_view = Some(Vec2::new(0.0, yaw));
        turned
    }

    /// The turn term rho for a view change of (pitch, yaw) radians.
    pub fn turn(scales: &SpreadScales, d_pitch: f32, d_yaw: f32) -> f32 {
        let dt = SPREAD_SUBSTEP as f32;
        (scales.turn_effect * (d_pitch.abs() + d_yaw.abs()) / (2.0 + scales.fast_turn_rate * dt)).min(1.0)
    }
}

/// A player's view kick (weapon_firing.md Corrections 1): the vertical
/// accumulator J and the side one J_y (radians, half the view offset each
/// caused, Q4), the side direction and the last kick's time. One per
/// player, never reset by a switch, a death or a respawn (only returned);
/// every kick and return reads the data of the weapon in hand then
/// (`CaRecoil`). Predicted with the player (`game::net`).
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CaKick {
    pub pitch: f32,
    pub yaw: f32,
    /// The side kick goes right (`wag_r`); None until the first one picks
    /// at random (the game draws it as the player enters the map).
    pub right: Option<bool>,
    pub last_kick: Option<f64>,
}

/// A weapon's view-kick data (`combat_arms:recoil`) and its fitted
/// attachments' terms; the state is its holder's (`CaKick`).
#[derive(Component, Clone, Debug)]
pub struct CaRecoil {
    pub params: RecoilParams,
    /// The fitted attachments' terms (`attachments`; zero: stock).
    pub mods: RecoilMods,
}

/// The recoil of a weapon with no `combat_arms:recoil` table: the record
/// defaults (a knife or a grenade in hand still returns the view).
pub static DEFAULT_RECOIL: LazyLock<CaRecoil> = LazyLock::new(|| CaRecoil::new(RecoilParams::default()));

impl CaRecoil {
    pub fn new(params: RecoilParams) -> Self {
        Self {
            params,
            mods: RecoilMods::default(),
        }
    }

    /// (base kick, cap) radians in this mode, the kick x (1 + p_b) and
    /// the cap x (1 + `cap`) (attachments.md, "Recoil").
    fn pitch_params(&self, burst: bool, cap: f32) -> (f32, f32) {
        let p = &self.params;
        let (b, max) = if burst && p.burst_pitch != 0.0 && p.burst_max_pitch != 0.0 {
            (p.burst_pitch, p.burst_max_pitch)
        } else {
            (p.pitch, p.max_pitch)
        };
        ((b * (1.0 + self.mods.pitch)).to_radians(), (max * (1.0 + cap)).to_radians())
    }

    /// The return's cap, radians: in burst mode the burst cap whenever it
    /// is non-zero (the kick needs both burst values, the return only
    /// this: Corrections 2), else the normal one; scaled by the muzzle
    /// part's `MaxCamRecoilPitchPer` alone.
    pub fn return_cap(&self, burst: bool) -> f32 {
        let p = &self.params;
        let max = if burst && p.burst_max_pitch != 0.0 { p.burst_max_pitch } else { p.max_pitch };
        (max * (1.0 + self.mods.return_max)).to_radians()
    }

    /// One shot's kick on `k` at `now`: returns how far the view goes up,
    /// radians (2h).
    pub fn kick(&self, k: &mut CaKick, burst: bool, now: f64) -> f32 {
        let (b, max) = self.pitch_params(burst, self.mods.max_pitch);
        if max <= 0.0 {
            return 0.0;
        }
        let j = k.pitch.clamp(0.0, max);
        let kick = b * (1.0 - j / max);
        let accel = self.params.accel_pitch + self.mods.accel;
        let kick = if kick > 0.0 { kick + accel * j } else { kick };
        let h = (kick / 2.0).min(max - j);
        k.pitch = j + h;
        k.last_kick = Some(now);
        2.0 * h
    }

    /// The side wag after a kick: returns the yaw change in the engine's
    /// sense (radians, + left), 0 without one. `roll` in [0, 1) picks the
    /// first direction and the flips.
    pub fn wag(&self, k: &mut CaKick, roll: f32) -> f32 {
        let p = self.params;
        // The threshold adds the attachments' `MaxRandomPitch` (a
        // silencer's -2 kicks from the first shot: Q11).
        if k.pitch.abs() <= p.random_pitch + self.mods.random_pitch {
            return 0.0;
        }
        let right = *k.right.get_or_insert(roll < 0.5);
        let max = p.max_wag.to_radians();
        let q = if max != 0.0 { 1.0 - k.yaw.abs() / max } else { 1.0 };
        let w = q * if right { p.wag_r } else { p.wag_l }.to_radians();
        k.yaw += w / 2.0;
        if k.yaw.abs() > p.random_wag && (roll * 2.0).fract() < 0.5 {
            k.right = Some(!right);
        }
        // Combat Arms turns yaw by -w (+ right); the engine's yaw is + left.
        w
    }

    /// The return of `k` over `dt` seconds at `now` (`burst`: the weapon
    /// in hand is in burst mode): (pitch down, yaw change in the engine's
    /// sense), radians.
    pub fn recover(&self, k: &mut CaKick, dt: f32, now: f64, burst: bool) -> (f32, f32) {
        let p = self.params;
        let waiting = k.last_kick.is_some_and(|t| now - t < p.jitter_dec_delay as f64 / 1000.0);
        let rate = if waiting { p.jitter_wait_recover } else { p.jitter_recover };
        let step = |acc: f32, max: f32| {
            let scale = if max > 0.0 { 1.0 + acc / max } else { 1.0 };
            acc.min(dt * rate * scale)
        };
        let mut down = 0.0;
        if k.pitch > 0.0 {
            down = step(k.pitch, self.return_cap(burst));
            k.pitch -= down / 2.0;
        }
        let mut side = 0.0;
        if k.yaw != 0.0 {
            let v = step(k.yaw.abs(), p.max_wag.to_radians());
            let sign = k.yaw.signum();
            k.yaw -= sign * v / 2.0;
            if k.yaw.signum() != sign {
                k.yaw = 0.0;
            }
            side = -sign * v;
        }
        (down, side)
    }
}

// ---------------------------------------------------------------------------
// Hit zones and armour

/// A hit sphere's location code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    Head,
    Torso,
    Arm,
    Leg,
    Groin,
}

/// One hit sphere of a skeleton (specs/combat_arms/characters.md 11):
/// its hit-table entry's node or socket name, radius, location and damage
/// factor, and `at`: its centre in the spec's measured standing pose (`LST`
/// at t = 0) in the character's frame (feet at the origin, +Y up, facing
/// -Z, +X its right), world units. Entries no stock body has, or with
/// radius 0, have radius 0 here (never hit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitSphere {
    pub name: &'static str,
    pub at: Vec3,
    pub radius: f32,
    pub location: Location,
    pub factor: f32,
    /// The skeleton (`characters::records` index: 0 male, 1 female).
    pub skeleton: usize,
    /// `Node#Priority`: the server's own walk picks the highest
    /// (`priority_sphere`; data/characters.toml, ours by location).
    pub priority: f32,
}

use Location::{Arm, Groin, Head, Leg, Torso};

impl HitSphere {
    /// Its body region (`core::BodyRegion`, docs/DESIGN.md "Hits across games"):
    /// by location, the side by where it sits (`at.x`, as hits report it),
    /// the limb's part by its node's name (`*hand*`; `*arml*` the forearm;
    /// `*foot*`, `*toe*`; `*legl*` the shin; the rest of a limb, the
    /// pelvis spheres too, upper arm or thigh); the torso spheres chest
    /// but the neck (its hitgroup is the chest either way).
    pub fn region(&self) -> crate::core::BodyRegion {
        use crate::core::BodyRegion as R;
        let n = self.name.to_ascii_lowercase();
        let side = |l, r| if self.at.x < 0.0 { l } else { r };
        match self.location {
            Head => R::Head,
            Groin => R::Groin,
            Torso if n.contains("neck") => R::Neck,
            Torso => R::Chest,
            Arm if n.contains("hand") => side(R::LeftHand, R::RightHand),
            Arm if n.contains("arml") => side(R::LeftForearm, R::RightForearm),
            Arm => side(R::LeftUpperArm, R::RightUpperArm),
            Leg if n.contains("foot") || n.contains("toe") => side(R::LeftFoot, R::RightFoot),
            Leg if n.contains("legl") => side(R::LeftShin, R::RightShin),
            Leg => side(R::LeftThigh, R::RightThigh),
        }
    }
}

/// Skeleton `index`'s hit table from data/characters.toml.
fn hit_table(index: usize) -> &'static [HitSphere] {
    let skel = characters::records::builtin().skeleton(index);
    let table: Vec<HitSphere> = skel
        .spheres
        .iter()
        .map(|s| HitSphere {
            name: s.name.clone().leak(),
            at: s.at.map_or(Vec3::ZERO, |a| characters::skeleton::to_body(Vec3::from(a), characters::skeleton::STAND_FEET)),
            radius: if s.absent || s.at.is_none() { 0.0 } else { s.radius },
            location: match s.location {
                1 => Head,
                3 => Arm,
                4 => Leg,
                5 => Groin,
                _ => Torso,
            },
            factor: s.factor,
            skeleton: index,
            priority: s.priority.unwrap_or_else(|| skel.hit_priority.of(s.location)),
        })
        .collect();
    table.leak()
}

/// The male skeleton's hit table (table 0, the default male body).
pub static MALE: LazyLock<&'static [HitSphere]> = LazyLock::new(|| hit_table(0));
/// The female skeleton's (table 1).
pub static FEMALE: LazyLock<&'static [HitSphere]> = LazyLock::new(|| hit_table(1));

/// A character's hit table (`MALE` when it has none; `characters` sets
/// it from the body's skeleton).
#[derive(Component, Clone, Copy, Debug)]
pub struct HitSpheres(pub &'static [HitSphere]);

/// A named sphere of the male skeleton.
pub fn male(name: &str) -> &'static HitSphere {
    MALE.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no sphere {name}"))
}

/// Which sphere a segment from `start` to `end` hits: of those it passes
/// (radius > 0), the one with the smallest |p|^2 - r^2 (p the centre from
/// `start`). Spheres as (centre, radius).
pub fn pick_sphere(spheres: &[(Vec3, f32)], start: Vec3, end: Vec3) -> Option<usize> {
    let seg = end - start;
    let len2 = seg.length_squared().max(1e-12);
    spheres
        .iter()
        .enumerate()
        .filter(|(_, (c, r))| {
            if *r <= 0.0 {
                return false;
            }
            let t = ((*c - start).dot(seg) / len2).clamp(0.0, 1.0);
            (start + seg * t).distance_squared(*c) <= r * r
        })
        .map(|(i, (c, r))| (i, (*c - start).length_squared() - r * r))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Which sphere a character found by the server's own walk is hit in
/// (weapon_firing.md Corrections 5): of the spheres (centre, radius) whose
/// centre is within its radius of the infinite line through `at` along
/// the unit `dir`, wherever along the line, the one with the highest
/// `priority`; on a tie the later one. None: the line passes them all.
pub fn priority_sphere(spheres: &[(Vec3, f32)], data: &[HitSphere], at: Vec3, dir: Vec3) -> Option<usize> {
    spheres
        .iter()
        .enumerate()
        .filter(|(_, (c, r))| {
            let p = *c - at;
            *r > 0.0 && (p - dir * p.dot(dir)).length_squared() <= r * r
        })
        .map(|(i, _)| (i, data.get(i).map_or(0.0, |s| s.priority)))
        .fold(None, |best: Option<(usize, f32)>, (i, pr)| match best {
            Some((_, b)) if b > pr => best,
            _ => Some((i, pr)),
        })
        .map(|(i, _)| i)
}

/// The direction sector of a bullet travelling along `dir` at a victim
/// facing `forward` with `right` (0..11, pi/6 each).
pub fn sector(forward: Vec3, right: Vec3, dir: Vec3) -> u32 {
    let theta = std::f32::consts::PI + forward.dot(dir).atan2(right.dot(dir));
    ((theta / (std::f32::consts::PI / 6.0)).floor() as i32).clamp(0, 11) as u32
}

/// From behind: sectors 7 to 11 (Q5: lopsided).
pub fn is_behind(forward: Vec3, right: Vec3, dir: Vec3) -> bool {
    (7..=11).contains(&sector(forward, right, dir))
}

/// One piece of gear's protection (Q15): per damage type
/// (`InstDamageType`), a fraction for the head, torso, arm and leg, and
/// whether it is used up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gear {
    pub protects: &'static [Protection],
}

/// Gear whose only entry is a flash protection of `$v` on the head and
/// torso (grenades.md section 10).
macro_rules! flash_gear {
    ($v:expr) => {
        Gear {
            protects: &[Protection {
                damage_type: FLASH,
                head: $v,
                torso: $v,
                arm: 0.0,
                leg: 0.0,
                consumable: false,
            }],
        }
    };
}

/// Every piece of gear there is, by name (`Gear` goes over the network as
/// its place here).
pub const GEARS: &[(&str, Gear)] = &[
    ("default body", DEFAULT_BODY_GEAR),
    ("light body", LIGHT_BODY_GEAR),
    ("heavy body", HEAVY_BODY_GEAR),
    ("default head", DEFAULT_HEAD_GEAR),
    ("gas mask", GAS_MASK),
    // Gear with a flash entry (grenades.md section 10, its torso value,
    // which is its head value too), by data name; a name with two records
    // takes its first. Only the flash entries: their other protection
    // isn't in a spec here.
    ("Anti-Flash", flash_gear!(1.0)),
    ("Skull_head", flash_gear!(1.0)),
    ("Codename_Raven", flash_gear!(1.0)),
    ("Prison_Recon_S_grade", flash_gear!(1.0)),
    ("Assassin_F", flash_gear!(1.0)),
    ("Assassin_M", flash_gear!(1.0)),
    ("Protect_flash_100", flash_gear!(1.0)),
    ("Ballistic_Mask", flash_gear!(0.8)),
    ("SnowMan_Head", flash_gear!(0.8)),
    ("Davil_Mask", flash_gear!(0.8)),
    ("Guy_Fawkes_mask", flash_gear!(0.8)),
    ("Protect_flash_75", flash_gear!(0.75)),
    ("Anti-Flash_70", flash_gear!(0.7)),
    ("Recruit_Survival_Bandana", flash_gear!(0.7)),
    ("Protect_flash_45", flash_gear!(0.55)),
    ("Protect_flash_55", flash_gear!(0.55)),
    ("Protect_fire_24", flash_gear!(0.55)),
    ("Anti-Flash_50", flash_gear!(0.5)),
    ("Codename_Viper", flash_gear!(0.5)),
    ("Powered_Suit_Helmet_Extreme", flash_gear!(0.5)),
    ("Stek_Headgear", flash_gear!(0.5)),
    ("RoboSkull_HauserRider_G", flash_gear!(0.5)),
    ("Protect_flash_25", flash_gear!(0.45)),
    ("Protect_flash_35", flash_gear!(0.45)),
    ("Protect_fire_22", flash_gear!(0.45)),
    ("RoboSkull_HauserRider_S", flash_gear!(0.4)),
    ("Protect_fire_20", flash_gear!(0.35)),
    ("Anti-Flash_30", flash_gear!(0.3)),
    ("Eyepatch_Skull", flash_gear!(0.3)),
    ("Powered_Suit_Helmet_Hard", flash_gear!(0.3)),
    ("Protect_gas_20", flash_gear!(0.25)),
    ("Eyepatch", flash_gear!(0.15)),
    ("Protect_flash_15", flash_gear!(0.15)),
    ("Protect_fire_16", flash_gear!(0.15)),
    ("Protect_gas_16", flash_gear!(0.15)),
    ("Protect_flash_12", flash_gear!(0.12)),
    ("Protect_flash_9", flash_gear!(0.09)),
    ("Infected", flash_gear!(1.0)),
    ("Host_Infected", flash_gear!(1.0)),
];

/// A piece of gear by its name in `GEARS`.
pub fn gear(name: &str) -> Option<Gear> {
    GEARS.iter().find(|(n, _)| *n == name).map(|(_, g)| *g)
}

/// The sum of `pieces`' torso entries for `damage_type` (flash protection
/// sums them, grenades.md section 10), before any clamp. Only entries
/// made for that type count (not an every-class kit such as CS:S's
/// kevlar, which is no `ArmorPieces`).
pub fn gear_torso(pieces: &[hits::ArmorPiece], damage_type: i32) -> f32 {
    let class = Some(class_of(damage_type));
    pieces.iter().flat_map(|p| p.entries.iter()).filter(|e| e.class == class).map(|e| e.torso).sum()
}

impl Gear {
    /// A piece with these entries, interned: the same entries are always
    /// the same slice (a piece goes over the network by its entries, and
    /// a client decoding it must not grow memory each time).
    pub fn new(entries: &[Protection]) -> Gear {
        static INTERNED: std::sync::Mutex<Vec<&'static [Protection]>> = std::sync::Mutex::new(Vec::new());
        let mut list = INTERNED.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(found) = list.iter().find(|s| s.iter().eq(entries.iter())) {
            return Gear { protects: found };
        }
        let leaked: &'static [Protection] = Box::leak(entries.to_vec().into_boxed_slice());
        list.push(leaked);
        Gear { protects: leaked }
    }
}

// A piece goes over the network by its entries: the install's pieces
// are not in `GEARS`.
impl Serialize for Gear {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.protects.serialize(s)
    }
}

impl<'de> Deserialize<'de> for Gear {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let entries = Vec::<Protection>::deserialize(d)?;
        Ok(Gear::new(&entries))
    }
}

/// What a piece of gear does against one damage type.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Protection {
    pub damage_type: i32,
    pub head: f32,
    pub torso: f32,
    pub arm: f32,
    pub leg: f32,
    pub consumable: bool,
}

impl Gear {
    /// The piece as Chimera's armour coverage (`weapon::hits`): its first
    /// entry per damage type, by class (`class_of`), the groin uncovered,
    /// worn by what it blocks when consumable.
    pub fn coverage(&self) -> impl Iterator<Item = hits::Coverage> + '_ {
        self.protects
            .iter()
            .enumerate()
            .filter(|(i, p)| self.protects[..*i].iter().all(|o| o.damage_type != p.damage_type))
            .map(|(_, p)| hits::Coverage {
                class: Some(class_of(p.damage_type)),
                head: p.head,
                torso: p.torso,
                arm: p.arm,
                leg: p.leg,
                groin: 0.0,
                wear: if p.consumable { 1.0 } else { 0.0 },
            })
    }
}

const fn bullet(head: f32, body: f32) -> Protection {
    Protection {
        damage_type: BULLET,
        head,
        torso: body,
        arm: body,
        leg: body,
        consumable: true,
    }
}

/// Body gear's burn protection: 0.5 everywhere, the head too.
const BODY_BURN: Protection = Protection {
    damage_type: BURN,
    head: 0.5,
    torso: 0.5,
    arm: 0.5,
    leg: 0.5,
    consumable: true,
};

/// The spec's common gear ("Armour"): example values until the
/// characters spec gives the table (Q7). Body gear covers bullets and
/// burn, head gear bullets, the gas mask poison (not used up); nothing
/// covers melee or explosions.
pub const DEFAULT_BODY_GEAR: Gear = Gear {
    protects: &[bullet(0.0, 0.15), BODY_BURN],
};
pub const LIGHT_BODY_GEAR: Gear = Gear {
    protects: &[bullet(0.0, 0.05), BODY_BURN],
};
pub const HEAVY_BODY_GEAR: Gear = Gear {
    protects: &[bullet(0.0, 0.30), BODY_BURN],
};
pub const DEFAULT_HEAD_GEAR: Gear = Gear {
    protects: &[bullet(0.25, 0.0)],
};
pub const GAS_MASK: Gear = Gear {
    protects: &[Protection {
        damage_type: POISON,
        head: 1.0,
        torso: 1.0,
        arm: 1.0,
        leg: 1.0,
        consumable: false,
    }],
};

/// A shooter whose rounds go exactly where it looks: no spread cone, one
/// ray per shot (no extra pellets) and no recoil kick. Combat Arms' bots
/// (bots.md 7.3, Quirk 7), whose brain picks each shot's aim point.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExactAim;

/// Combat Arms' armour: Chimera's `weapon::Armor` (its amount: the
/// points, AP, over `HIT_POINTS`) with the gear worn as `ArmorPieces`
/// (`pieces_of`). The server's; replicated to clients (`game::net`).
pub fn gear_armour(points: f32, gear: &[Gear]) -> (Armor, ArmorPieces) {
    (
        Armor {
            amount: points / HIT_POINTS,
            helmet: false,
        },
        pieces_of(gear),
    )
}

/// The gear worn as Chimera's armour pieces (`Gear::coverage`), running
/// out by what they block.
pub fn pieces_of(gear: &[Gear]) -> ArmorPieces {
    ArmorPieces {
        pieces: gear.iter().map(|g| hits::ArmorPiece { entries: g.coverage().collect() }).collect(),
        depletion: hits::Depletion::Blocked,
    }
}

/// A model's `MaxArmor` 100 with the default body gear.
pub fn default_armour() -> (Armor, ArmorPieces) {
    gear_armour(100.0, &[DEFAULT_BODY_GEAR])
}

/// Armour points (AP) of `armor`.
pub fn points(armor: &Armor) -> f32 {
    armor.amount * HIT_POINTS
}

/// What the armour step needs of a hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArmourHit {
    /// `InstDamageType`.
    pub damage_type: i32,
    /// The hit's location (torso for types without zones).
    pub location: Location,
    /// The zone factor (1 for types without zones).
    pub zone: f32,
    /// An infected victim: its groin counts as torso.
    pub infected: bool,
}

/// Damage `d` (hit points) of `hit` after armour: returns what health
/// loses, and takes the armour's share. With `lands` false (a refused hit)
/// the armour is left whole. The shared armour step
/// (`weapon::hits::absorb`, ratio 1, fractions kept, `Depletion::Blocked`)
/// with the gear as coverage (`Gear::coverage`), then the zone factor.
pub fn armour_and_zone(d: f32, hit: ArmourHit, armor: Option<&mut Armor>, pieces: Option<&ArmorPieces>, lands: bool) -> f32 {
    let zone = hit.zone;
    let Some(armor) = armor else { return (d * zone).max(0.0) };
    let location = if hit.infected && hit.location == Groin { Torso } else { hit.location };
    let ap = points(armor);
    let (entries, depletion) = crate::weapon::coverage(armor, pieces);
    let a = hits::absorb(d, part_of(location), class_of(hit.damage_type), Some(1.0), 0.0, ap, entries, depletion);
    if a.covered && lands {
        armor.amount = (ap - a.taken).max(0.0) / HIT_POINTS;
    }
    (a.to_health * zone).max(0.0)
}

/// Chimera's damage class of a damage type (`InstDamageType`): bullet,
/// burn, explosion (blast), poison, flash and melee as Chimera's, any
/// other as a class of Combat Arms' own (`DamageClass::OWN` + the type).
pub fn class_of(damage_type: i32) -> hits::DamageClass {
    match damage_type {
        BULLET => hits::DamageClass::BULLET,
        BURN => hits::DamageClass::BURN,
        EXPLOSION => hits::DamageClass::BLAST,
        POISON => hits::DamageClass::POISON,
        FLASH => hits::DamageClass::FLASH,
        MELEE => hits::DamageClass::MELEE,
        t => hits::DamageClass(hits::DamageClass::OWN.saturating_add(t.clamp(0, i32::from(u16::MAX)) as u16)),
    }
}

/// An armour location as Chimera's body part.
pub fn part_of(location: Location) -> crate::core::BodyPart {
    match location {
        Head => crate::core::BodyPart::Head,
        Torso => crate::core::BodyPart::Torso,
        Arm => crate::core::BodyPart::Arm,
        Leg => crate::core::BodyPart::Leg,
        Groin => crate::core::BodyPart::Groin,
    }
}

/// Steps 1 to 4 of "Damage per ray" before penetration and the victim:
/// base x the roll (`u` in [0, 1] picks within it) x `behind` x the band
/// at `distance` metres. Health units.
pub fn base_damage(d: &CaDamage, u: f32, behind: bool, distance: f32) -> f32 {
    let roll = 1.0 - d.roll + 2.0 * d.roll * u;
    let back = if behind { BEHIND_BONUS } else { 1.0 };
    d.damage * roll * back * band_scale(d, distance)
}

/// The range band's multiplier at `distance` metres (edges inclusive at
/// the top; beyond the last, or without bands, 1: Q8).
pub fn band_scale(d: &CaDamage, distance: f32) -> f32 {
    let Some(bands) = d.bands else { return 1.0 };
    // Edges are whole units: compare in units, a hair of float slack.
    let units = distance / d.unit;
    bands
        .iter()
        .find(|(edge, _)| units <= edge / d.unit + 1e-3)
        .map_or(1.0, |(_, m)| *m)
}

/// A uniform square spread: the aim's forward bent by (2 u1 - 1) `half`
/// right and (2 u2 - 1) `half` up, normalised (`half` = n / 1000).
pub fn square_dir(aim: Quat, half: f32, u1: f32, u2: f32) -> Vec3 {
    let f = aim * Vec3::NEG_Z;
    let r = aim * Vec3::X;
    let up = aim * Vec3::Y;
    (f + r * ((2.0 * u1 - 1.0) * half) + up * ((2.0 * u2 - 1.0) * half)).normalize()
}

// ---------------------------------------------------------------------------
// Surfaces

/// One shoot-through surface (data/surfaces.toml), metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfacePass {
    /// Most material passed (0: always).
    pub thickness: f32,
    /// Exit direction offset, thousandths per axis.
    pub perturb: f32,
    pub range: f32,
    pub damage: f32,
}

/// Shoot-through surfaces by name (lower case).
#[derive(Resource, Clone, Debug, Default)]
pub struct CaSurfaces(pub Vec<(String, SurfacePass)>);

impl CaSurfaces {
    pub fn get(&self, name: &str) -> Option<SurfacePass> {
        let name = name.to_lowercase();
        self.0.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
    }

    /// Parse a surfaces document (data/surfaces.toml's format).
    pub fn parse(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Units {
            length: f32,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Row {
            names: Vec<String>,
            thickness: f32,
            perturb: f32,
            range: f32,
            damage: f32,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Doc {
            units: Units,
            surface: Vec<Row>,
        }
        let doc: Doc = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for row in doc.surface {
            for n in row.names {
                out.push((
                    n.to_lowercase(),
                    SurfacePass {
                        thickness: row.thickness * doc.units.length,
                        perturb: row.perturb,
                        range: row.range,
                        damage: row.damage,
                    },
                ));
            }
        }
        Ok(Self(out))
    }

    /// The built-in table (data/surfaces.toml).
    pub fn built_in() -> &'static Self {
        static SET: OnceLock<CaSurfaces> = OnceLock::new();
        SET.get_or_init(|| {
            Self::parse(include_str!("../../data/surfaces.toml"))
                .unwrap_or_else(|e| panic!("Combat Arms surfaces.toml: {e}"))
        })
    }
}

// ---------------------------------------------------------------------------
// Plugin

/// Combat Arms' weapons: its hooks, the systems around the weapon frame,
/// and the weapons registered from its data (`data::weapon_set`).
pub struct CaWeaponsPlugin;

impl Plugin for CaWeaponsPlugin {
    fn build(&self, app: &mut App) {
        power::plugin(app);
        app.register_weapon_hook::<DamageParams>(DAMAGE, |e, d: &'static WeaponData, p| {
            let mut dmg = CaDamage::new(p, d.units.length, d.units.damage);
            if p.reload_third_person.is_none() {
                dmg.reload_third_person = e.get::<crate::weapon::Magazine>().map_or(0.0, |m| m.reload_time);
            }
            e.insert(dmg);
        })
        .register_weapon_hook::<SpreadParams>(SPREAD, |e, d, p| {
            e.insert(CaSpread::new(p.clone(), d.units.length));
        })
        .register_weapon_hook::<RecoilParams>(RECOIL, |e, _, p| {
            e.insert(CaRecoil::new(*p));
        })
        .register_weapon_hook::<CaHandling>(HANDLING, |e, _, p| {
            e.insert(*p);
        })
        .register_weapon_hook::<ZoomParams>(ZOOM, |e, _, p| {
            e.insert((CaZoom::new(p.clone()), crate::weapon::OwnZoom));
        })
        .register_weapon_hook::<SpinParams>(SPIN, |e, _, p| {
            e.insert(CaSpin {
                params: *p,
                state: SpinState::Idle,
            });
        })
        .register_weapon_hook::<CaAltFire>(ALT_FIRE, |e, _, p| {
            e.insert((*p, p.burst()));
        })
        .register_weapon_hook::<timing::TimingParams>(timing::TIMING, timing::apply)
        // An infected's weapon: a mark for the gun game's rungs only.
        .register_weapon_hook::<toml::Table>(super::rungs::INFECTED, |_, _, _| {})
        // The server's timing goes to its clients (`weapon::params`).
        .weapon_param(timing::TIMING, timing::encode, timing::push);
        let data = super::data::CaData::of(app);
        if let Err(e) = app.add_weapon_set(data.weapon_set()) {
            panic!("Combat Arms weapon data: {e}");
        }
        // A network client whose documents differ takes the server's.
        app.server_weapon_docs(super::data::served::WEAPONS, data.weapon_set(), data.weapon_docs().to_vec());
        if !app.world().contains_resource::<CaServerSettings>() {
            app.insert_resource(CaServerSettings::of(&data));
        }
        app.init_resource::<SpreadScales>()
            .init_resource::<timing::ViewTimes>()
            .init_resource::<SpreadOptions>()
            .init_resource::<CaAiDamage>()
            .insert_resource(CaSurfaces::built_in().clone())
            .add_message::<WeaponEvent>()
            .add_message::<Damage>()
            .init_resource::<PelletCarryOver>()
            .add_systems(
                Predict::Weapons,
                (
                    // The class rules and the zoom step after the hands
                    // (`inventory`), then the cone the frame shows.
                    (classes::class_rules, zoom::zoom_step, before_shots)
                        .chain()
                        .after(super::inventory::hands)
                        .before(WeaponFrame),
                    (after_shots, zoom::bolt_zoom_out)
                        .chain()
                        .after(WeaponFrame)
                        .before(crate::weapon::impact::ApplyImpulses),
                ),
            )
            // Zoom presses before movement (the scoped speed limit from
            // the press's tick), after selection (a switch resets).
            .add_systems(
                Predict::Select,
                zoom::zoom_input.after(crate::weapon::SelectWeapons).after(apply_handling),
            )
            .add_systems(FixedUpdate, armour_follows_identity.run_if(crate::core::authoritative))
            // In the predicted tick: a client replaying a switch moves
            // with the penalty of the weapon it held then.
            .add_systems(Predict::Select, apply_handling.after(crate::weapon::SelectWeapons))
            // The meters are predicted with the weapon and go to the
            // client predicting it (`weapon::sync`); the kick with the
            // player.
            .predicted_part::<CaSpread>()
            .predicted_part::<CaZoom>()
            .predicted_part::<CaSpin>()
            .predicted_net::<CaKick>();
    }
}

impl NetPart for CaSpread {
    /// The meters, the last shot, the sub-step clock, the last body yaw
    /// and its twist, the jump flags (the parameters are the data's).
    type Net = (f32, f32, Option<f64>, f64, Option<Vec2>, Option<f32>, bool, bool);
    fn net(&self) -> Self::Net {
        (
            self.fire,
            self.movement,
            self.last_shot,
            self.clock,
            self.prev_view,
            self.body_yaw,
            self.jumped,
            self.was_on_ground,
        )
    }
    fn set_net(&mut self, (fire, movement, last_shot, clock, prev_view, body_yaw, jumped, was_on_ground): Self::Net) {
        self.fire = fire;
        self.movement = movement;
        self.last_shot = last_shot;
        self.clock = clock;
        self.prev_view = prev_view;
        self.body_yaw = body_yaw;
        self.jumped = jumped;
        self.was_on_ground = was_on_ground;
    }
}

/// Combat Arms' armour on its own characters only (`slots::
/// GameIdentity::allows`; where identity follows the weapon, once it has
/// one): one that first has health gets the default
/// armour; one that becomes Combat Arms' (another game's armour, or none)
/// puts on the default gear over the armour it has (its amount kept);
/// one that becomes another game's takes the gear off (what covers it is
/// then its armour's default kit: CS:S's kevlar). docs/DESIGN.md "Hits
/// across games".
#[allow(clippy::type_complexity)]
fn armour_follows_identity(
    characters: Query<
        (Entity, Has<Armor>, Has<ArmorPieces>, Option<&crate::slots::GameIdentity>, Has<crate::slots::FollowsWeapon>),
        (With<Health>, With<Intent>),
    >,
    minigame: Option<Res<chimera_sim::minigame::ActiveMinigame>>,
    mut commands: Commands,
) {
    // A minigame whose players' identity follows their weapon: a player
    // without one yet (before it is taken in, or holding nothing) isn't
    // Combat Arms'.
    let following = minigame.is_some_and(|m| m.def.slots.identity == Some(chimera_sim::minigame::def::IdentityDef::Weapon));
    for (e, armor, pieces, identity, follows) in &characters {
        if (follows || following) && identity.is_none() {
            continue;
        }
        let ours = crate::slots::GameIdentity::allows(identity, super::GAME);
        match (ours, armor, pieces) {
            (true, false, _) => {
                commands.entity(e).insert(default_armour());
            }
            (true, true, false) => {
                commands.entity(e).insert(pieces_of(&[DEFAULT_BODY_GEAR]));
            }
            (false, _, true) => {
                commands.entity(e).remove::<ArmorPieces>();
            }
            _ => {}
        }
    }
}

/// An angle in (-pi, pi].
fn wrap_angle(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let a = a.rem_euclid(tau);
    if a > std::f32::consts::PI { a - tau } else { a }
}

/// Before the frame: each active weapon's cone is what the HUD reads
/// (`Hitscan::spread`, a disc of n / 1000). The recoil returns after the
/// frame (`after_shots`): the shot leaves along the view as it was before
/// this tick's return (Corrections 11).
#[allow(clippy::type_complexity)]
fn before_shots(
    owners: Query<&Inventory>,
    mut weapons: Query<(
        Option<&CaSpread>,
        Option<&mut Hitscan>,
        Option<&AltModes>,
        Option<&Burst>,
        Option<&Zoom>,
        Option<&CaZoom>,
    )>,
) {
    for inv in &owners {
        let Some(w) = inv.active else { continue };
        let Ok((Some(s), Some(mut scan), modes, burst, zoom, ca_zoom)) = weapons.get_mut(w) else {
            continue;
        };
        let mode = mode_of(modes.map_or(0, |m| m.current), burst, zoom, ca_zoom);
        let n = s.cone(mode) / 1000.0;
        let want = SpreadShape::Disc {
            inaccuracy: n,
            spread: 0.0,
        };
        if scan.spread != want {
            scan.spread = want;
        }
    }
}

/// One round to deliver (a `Fired` event of a Combat Arms weapon).
#[derive(Clone, Copy, Debug)]
struct Round {
    owner: Entity,
    weapon: Entity,
    origin: Vec3,
    yaw: f32,
    pitch: f32,
    seed: u32,
    mode: u8,
    replay: bool,
}

/// A character as the ray sees it.
struct Body {
    entity: Entity,
    /// Its origin (the near-miss test's plane side).
    origin: Vec3,
    /// World box (min, max).
    min: Vec3,
    max: Vec3,
    /// Its body's forward and right (the legs': Corrections 6).
    forward: Vec3,
    right: Vec3,
    /// Its hit shapes in the world (`body_shapes`).
    shapes: crate::core::Hitboxes,
}

#[derive(bevy::ecs::system::SystemParam)]
struct Delivery<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    characters: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Intent,
            &'static MovementState,
            &'static Health,
            Option<&'static ColliderAabb>,
            Option<&'static HitSpheres>,
            (
                Option<&'static characters::CaPose>,
                Option<&'static crate::core::Hitboxes>,
                Option<&'static crate::slots::GameIdentity>,
            ),
        ),
    >,
    armor: Query<'w, 's, (&'static mut Armor, Option<&'static ArmorPieces>)>,
    infected: Query<'w, 's, (), With<super::movement::CaInfected>>,
    /// The bots, whose shots the server traces on its own pose.
    bots: Query<'w, 's, (), With<crate::core::Bot>>,
    /// Mercenaries, whose rounds deal body damage wherever they land
    /// (bots_objectives.md 3.3 Correction 1: PvE AI has no zone factor).
    mercs: Query<'w, 's, (), With<super::rules::mercs::CaMerc>>,
    teams: Query<'w, 's, &'static crate::core::Team>,
    figures: Query<'w, 's, Option<&'static super::movement::CaCharacter>>,
    knockback: Query<'w, 's, &'static super::movement::HitKnockback>,
    now: Res<'w, SimClock>,
    commands: Commands<'w, 's>,
    /// Colliders' own surfaces: props' and Combat Arms' glass sets'
    /// (`PropSurface`), brush entities' (`map::BrushSurface`).
    surfaces: (Query<'w, 's, &'static PropSurface>, Query<'w, 's, &'static crate::map::BrushSurface>),
    colliders: Query<'w, 's, &'static ColliderOf>,
    bodies: Query<'w, 's, (&'static RigidBody, Option<&'static ColliderAabb>)>,
    table: Res<'w, CaSurfaces>,
    /// The world's surfaces (walls without a `PropSurface`), another
    /// game's surface names as Combat Arms' (a mashup's map), and what
    /// takes damage that isn't a character (map breakables, props).
    ground: (
        Option<Res<'w, SurfaceGrid>>,
        Option<Res<'w, SurfaceNames>>,
        Query<'w, 's, (), (With<Damageable>, Without<Intent>)>,
    ),
    /// Live grenades, which rounds hit (grenades.md 13), and special
    /// weapons' objects (rockets, mines: special_weapons.md 1.4).
    grenades: Query<'w, 's, (Entity, &'static super::grenades::CaGrenade)>,
    objects: Query<'w, 's, (Entity, &'static super::special::CaTarget)>,
    damage: MessageWriter<'w, Damage>,
    /// The hit's full region beside its damage (`core::RegionHit`).
    regions: Option<ResMut<'w, Messages<crate::core::RegionHit>>>,
    /// Rounds' pushes on physics objects (`weapon::impact`).
    impulses: MessageWriter<'w, crate::weapon::impact::BulletImpulse>,
    rules: DamageRules<'w, 's>,
    /// Spawn-protected characters (`core::SpawnProtected`, a party's): a
    /// hit on them shows nothing (`weapon::impact::hide_protected`).
    spawn_protected: Query<'w, 's, &'static crate::core::SpawnProtected>,
    /// The mode and friendly fire (game_modes.md S8: a teammate's round is
    /// scaled before armour and the zone factor).
    settings: Option<Res<'w, super::rules::CaRuleSettings>>,
    first: Res<'w, FirstTimePredicted>,
    role: Option<Res<'w, NetRole>>,
    playback: Option<Res<'w, crate::core::Playback>>,
    /// The attachments' target groups (an AI victim) and a forced draw for
    /// their probabilities.
    attach: (Query<'w, 's, &'static CaNpc>, Res<'w, CaSpecialRoll>),
    /// AI damage (Corrections 7).
    ai: AiHits<'w, 's>,
    /// Damage the shooter's gear adds (Fighting Chance, set items;
    /// gear_perks.md 11, 12, 15) and the weapons' ids.
    dealt: Query<'w, 's, &'static super::gear::CaGearEffects>,
    gear_data: Option<Res<'w, super::gear::CaGearData>>,
    ids: Query<'w, 's, &'static crate::weapon::Weapon>,
    /// Lag compensation (a network server): each shooter's view tick,
    /// the characters' kept hit volumes, `sv_unlag`.
    lag: (
        Query<'w, 's, &'static ViewTick>,
        Query<'w, 's, (Entity, &'static HitHistory, &'static Health)>,
        Res<'w, LagCompSettings>,
        ResMut<'w, LagCompStats>,
        Res<'w, SimClock>,
    ),
}

impl Delivery<'_, '_> {
    fn authoritative(&self) -> bool {
        self.role.as_deref().is_none_or(|r| *r != NetRole::Client) && self.playback.is_none()
    }

    /// Where the other characters were when `owner` fired, as its client
    /// drew them (lag compensation, `weapon::lagcomp`): a network server's
    /// remote player's shot with `sv_unlag` on; else none (the present).
    fn rewind(&mut self, owner: Entity, server: bool) -> Vec<lagcomp::Rewound> {
        let (views, histories, settings, stats, clock) = &mut self.lag;
        let Some(view) = views.get(owner).ok().and_then(|v| v.0) else {
            return Vec::new();
        };
        if !server || settings.unlag == 0 {
            return Vec::new();
        }
        lagcomp::rewind(
            owner,
            view,
            clock.tick,
            clock.delta.as_secs_f64(),
            settings,
            histories.iter().filter(|(.., h)| h.current > 0.0).map(|(e, history, _)| (e, history)),
            stats,
        )
    }

    /// Every living character but `owner`, as the ray sees it: where it
    /// is, or where `rewound` puts it (its box and look then); `yaws`:
    /// each character's body yaw (`body_yaw`; none: its view's).
    fn bodies(&self, owner: Entity, rewound: &[lagcomp::Rewound], yaws: &HashMap<Entity, f32>) -> Vec<Body> {
        // A bot's shot is traced on the server's pose (characters.md 8,
        // Open question 5: no `Torso` pitch there).
        let server = self.bots.contains(owner);
        self.characters
            .iter()
            .filter(|(e, .., h, _, _, _)| *e != owner && h.current > 0.0)
            // The spawn-protected in their bullets window aren't there
            // (Chimera's `core::SpawnProtected::passes_attacks`, a party's):
            // the round goes on to what is behind.
            .filter(|(e, ..)| !self.spawn_protected.get(*e).is_ok_and(|p| p.passes_attacks(self.now.now)))
            .filter_map(|(entity, t, intent, state, _, aabb, spheres, (pose, boxes, identity))| {
                let data = spheres.map_or(*MALE, |s| s.0);
                let rewound = rewound.iter().find(|r| r.entity == entity);
                // Another game's body with its own hitboxes (then, when
                // rewound): those, as its game made them.
                let theirs = match rewound {
                    Some(r) if !crate::slots::GameIdentity::allows(identity, super::GAME) => {
                        r.hitboxes.as_ref().map(|h| {
                            let (feet, _) = r.frame();
                            body_shapes(&h[..], feet, Quat::from_rotation_y(r.yaw))
                        })
                    }
                    Some(_) => None,
                    None => their_shapes(t, intent, aabb, identity, boxes),
                };
                // The body's twist from its view (the legs lag the mouse).
                let twist = yaws.get(&entity).map_or(0.0, |b| wrap_angle(b - intent.yaw));
                // A body model's own pose; else the spec's skeleton.
                let modelled = pose.filter(|p| p.modelled && p.skeleton == data.first().map_or(0, |s| s.skeleton));
                let (origin, min, max, spheres, body) = match rewound {
                    Some(r) => {
                        let b = r.aabb();
                        let turn = Quat::from_rotation_y(r.yaw);
                        let (min, max, mut spheres) = geometry_at_by(r.origin, turn, b.min, b.max, data, -intent.pitch, server);
                        if let Some(p) = modelled {
                            spheres = posed_geometry_by(r.origin, min, r.yaw + (p.body_yaw - intent.yaw), p, data, server);
                        }
                        (r.origin, min, max, spheres, r.yaw + twist)
                    }
                    None => {
                        let (min, max, mut spheres) = body_geometry_by(t, intent, state, aabb, data, server)?;
                        if let Some(p) = modelled {
                            spheres = posed_geometry_by(t.translation, min, p.body_yaw, p, data, server);
                        }
                        (t.translation, min, max, spheres, intent.yaw + twist)
                    }
                };
                let turn = Quat::from_rotation_y(body);
                // Another game's shapes may poke out of the hull (a taller
                // model's head): the box holds them, so they're reached.
                let (min, max) = theirs.as_ref().map_or((min, max), |s| box_holding(min, max, s));
                Some(Body {
                    entity,
                    origin,
                    min,
                    max,
                    forward: turn * Vec3::NEG_Z,
                    right: turn * Vec3::X,
                    shapes: theirs.unwrap_or_else(|| sphere_shapes(data, &spheres)),
                })
            })
            .collect()
    }

    /// S8's factor on a round from `a` to `b` (Open question 26: before
    /// armour and the zone factor): `FF_SCALE` between teammates of a
    /// team mode that scales friendly fire, with it on; else 1.
    fn friendly_scale(&self, a: Entity, b: Entity) -> f32 {
        let Some(s) = self.settings.as_deref() else { return 1.0 };
        let scaled = s.friendly_fire != 0 && super::rules::mode_by_key(&s.mode).is_some_and(|m| m.ff_scaled);
        let mates = a != b
            && matches!((self.teams.get(a), self.teams.get(b)), (Ok(x), Ok(y)) if x == y && matches!(x.0, 1 | 2));
        if scaled && mates { super::rules::scoring::FF_SCALE } else { 1.0 }
    }

    /// Both on one team (neither without one).
    fn same_team(&self, a: Entity, b: Entity) -> bool {
        matches!((self.teams.get(a), self.teams.get(b)), (Ok(x), Ok(y)) if x == y)
    }

    fn body(&self, collider: Entity) -> Entity {
        self.colliders.get(collider).map_or(collider, |c| c.body)
    }

    /// The name of the surface a round hit (`CaSurfaces`' key): the
    /// collider's own (`effects::owned_surface`: a prop's, a glass set's, a
    /// brush entity's such as a CS:S window), else the world's surface
    /// type at `point`; another game's names (a mashup's map) as Combat
    /// Arms' (`map::SurfaceNames`).
    fn surface_name(&self, collider: Entity, point: Vec3) -> Option<String> {
        let names = self.ground.1.as_deref();
        let tables = super::effects::data::CaFxTables::built_in();
        let (props, brushes) = &self.surfaces;
        if let Some(p) = super::effects::owned_surface(collider, props, brushes, &self.colliders) {
            let p = names.map_or(p, |n| n.resolve(p));
            // Combat Arms' own world models name their type's number
            // (`library::tag_surfaces`).
            return match p.parse::<u16>() {
                Ok(ty) => Some(tables.surface.iter().find(|s| s.id == ty)?.name.clone()),
                Err(_) => Some(p.to_string()),
            };
        }
        let ty = super::effects::world_surface_named(self.ground.0.as_deref(), names, point);
        Some(tables.surface.iter().find(|s| s.id == ty)?.name.clone())
    }

    /// How a round passes what it hit (`surface_name` in `CaSurfaces`).
    /// None: it stops there.
    fn surface_at(&self, collider: Entity, point: Vec3) -> Option<SurfacePass> {
        self.table.get(&self.surface_name(collider, point)?)
    }
}

/// A character's body yaw (weapon_firing.md Corrections 6, inventory.md
/// Corrections 2): its view yaw `view` minus the upper body's twist, as
/// its weapon's spread keeps it (`CaSpread::body_yaw`; none: the view's).
pub fn body_yaw(view: f32, kept: Option<f32>) -> f32 {
    kept.unwrap_or(view)
}

/// A Combat Arms body's hit spheres (`data`, at `spheres`: world centres
/// and radii) as Chimera's hit shapes (`core::Hitbox`): each sphere's
/// region (`HitSphere::region`), `Node#Priority` and zone factor (the
/// body's own scale).
pub fn sphere_shapes(data: &[HitSphere], spheres: &[(Vec3, f32)]) -> crate::core::Hitboxes {
    crate::core::Hitboxes(
        data.iter()
            .zip(spheres)
            .map(|(s, (c, r))| crate::core::Hitbox {
                priority: s.priority,
                scale: Some(s.factor),
                ..crate::core::Hitbox::sphere(*c, *r, s.region())
            })
            .collect(),
    )
}

/// Another game's body's hitboxes (character space: `local`) in the
/// world, its feet at `feet` and turned by `turn`.
pub fn body_shapes(local: &[crate::core::Hitbox], feet: Vec3, turn: Quat) -> crate::core::Hitboxes {
    crate::core::Hitboxes(
        local
            .iter()
            .map(|h| crate::core::Hitbox {
                center: feet + turn * h.center,
                rotation: turn * h.rotation,
                ..*h
            })
            .collect(),
    )
}

/// A character's hit shapes in the world when its body is another game's
/// (its identity isn't Combat Arms' and that game's body gave it
/// `core::Hitboxes`): those, its feet at its box's bottom, turned by its
/// look's yaw (as every game places them). None: a body of ours, whose
/// shapes are our skeleton's spheres.
pub fn their_shapes(
    t: &Transform,
    intent: &Intent,
    aabb: Option<&ColliderAabb>,
    identity: Option<&crate::slots::GameIdentity>,
    hitboxes: Option<&crate::core::Hitboxes>,
) -> Option<crate::core::Hitboxes> {
    if crate::slots::GameIdentity::allows(identity, super::GAME) {
        return None;
    }
    let feet = aabb.map_or(t.translation, |b| t.translation.with_y(b.min.y));
    hitboxes.map(|h| body_shapes(&h.0, feet, intent.yaw_rotation()))
}

/// A body's box (world min, max) widened to hold its hit shapes (world),
/// so a shape poking out of the hull (another game's taller model's head)
/// is still reached through the box tests.
pub fn box_holding(min: Vec3, max: Vec3, shapes: &crate::core::Hitboxes) -> (Vec3, Vec3) {
    shapes.bounds().map_or((min, max), |(a, b)| (min.min(a), max.max(b)))
}

/// Combat Arms' zone factor for a region of a body that gives none of its
/// own (another game's: its hitboxes have no scale): the male skeleton's
/// first sphere of that region (data/characters.toml), else of its
/// hitgroup's region, else 1.
pub fn zone_factor(region: crate::core::BodyRegion) -> f32 {
    let of = |r: crate::core::BodyRegion| MALE.iter().find(|s| s.radius > 0.0 && s.region() == r).map(|s| s.factor);
    of(region)
        .or_else(|| of(crate::core::BodyRegion::from_hitgroup(region.hitgroup())))
        .unwrap_or(1.0)
}

/// A region's armour location (`BodyPart`'s, Combat Arms' five).
pub fn location_of(region: crate::core::BodyRegion) -> Location {
    match region.part() {
        crate::core::BodyPart::Head => Head,
        crate::core::BodyPart::Torso => Torso,
        crate::core::BodyPart::Arm => Arm,
        crate::core::BodyPart::Leg => Leg,
        crate::core::BodyPart::Groin => Groin,
    }
}

/// A character's box (world min, max: its movement's hull, else its
/// collider's bounds) and its spheres `data` in the world (centre,
/// radius): the spec's skeleton (`characters::skeleton::fallback`)
/// crouched by the box's height and pitched by the look.
pub fn body_geometry(
    t: &Transform,
    intent: &Intent,
    state: &MovementState,
    aabb: Option<&ColliderAabb>,
    data: &[HitSphere],
) -> Option<(Vec3, Vec3, Vec<(Vec3, f32)>)> {
    let (min, max) = if state.hull_max != state.hull_min {
        (t.translation + state.hull_min, t.translation + state.hull_max)
    } else {
        let b = aabb?;
        (b.min, b.max)
    };
    Some(geometry_at(t.translation, intent.yaw_rotation(), min, max, data, -intent.pitch))
}

/// `body_geometry` with the server's aim shares (`server`: no `Torso`
/// pitch, characters.md 8) for the hits the server traces itself.
pub fn body_geometry_by(
    t: &Transform,
    intent: &Intent,
    state: &MovementState,
    aabb: Option<&ColliderAabb>,
    data: &[HitSphere],
    server: bool,
) -> Option<(Vec3, Vec3, Vec<(Vec3, f32)>)> {
    let (min, max) = if state.hull_max != state.hull_min {
        (t.translation + state.hull_min, t.translation + state.hull_max)
    } else {
        let b = aabb?;
        (b.min, b.max)
    };
    Some(geometry_at_by(t.translation, intent.yaw_rotation(), min, max, data, -intent.pitch, server))
}

/// A character's spheres `data` in the world for its box `min..max`
/// (world) around `origin`, turned by `turn`, with the third-person aim
/// pitch `pitch` (radians, positive down): (min, max, spheres). The
/// skeleton is squashed by the box's height over the standing one.
pub fn geometry_at(origin: Vec3, turn: Quat, min: Vec3, max: Vec3, data: &[HitSphere], pitch: f32) -> (Vec3, Vec3, Vec<(Vec3, f32)>) {
    geometry_at_by(origin, turn, min, max, data, pitch, false)
}

/// `geometry_at` with the server's aim shares when `server`.
pub fn geometry_at_by(
    origin: Vec3,
    turn: Quat,
    min: Vec3,
    max: Vec3,
    data: &[HitSphere],
    pitch: f32,
    server: bool,
) -> (Vec3, Vec3, Vec<(Vec3, f32)>) {
    let unit = super::movement::movement_data().meters_per_unit;
    let squash = ((max.y - min.y) / (characters::STAND_HEIGHT * unit)).max(0.0);
    let skel = characters::records::builtin().skeleton(data.first().map_or(0, |s| s.skeleton));
    let mut posed = characters::skeleton::fallback(skel, squash);
    posed.aim(&skel.aim, pitch, 0.0, server);
    let feet = origin.with_y(min.y);
    let spheres = data
        .iter()
        .map(|s| {
            let p = posed.sphere(s.name).map_or(Vec3::ZERO, |p| characters::skeleton::to_body(p, posed.feet));
            (feet + turn * (p * unit), s.radius * unit)
        })
        .collect();
    (min, max, spheres)
}

/// A character's spheres from its posed body model (`characters::CaPose`)
/// for its box `min..max` around `origin`, its legs turned to `yaw`.
pub fn posed_geometry(origin: Vec3, min: Vec3, yaw: f32, pose: &characters::CaPose, data: &[HitSphere]) -> Vec<(Vec3, f32)> {
    posed_geometry_by(origin, min, yaw, pose, data, false)
}

/// `posed_geometry` on the server's pose (`CaPose::server_spheres`) when
/// `server`.
pub fn posed_geometry_by(
    origin: Vec3,
    min: Vec3,
    yaw: f32,
    pose: &characters::CaPose,
    data: &[HitSphere],
    server: bool,
) -> Vec<(Vec3, f32)> {
    let unit = super::movement::movement_data().meters_per_unit;
    let feet = origin.with_y(min.y);
    let turn = Quat::from_rotation_y(yaw);
    let centres = if server && !pose.server_spheres.is_empty() { &pose.server_spheres } else { &pose.spheres };
    data.iter()
        .enumerate()
        .map(|(i, s)| match centres.get(i).copied().flatten() {
            Some(p) => (feet + turn * (p * unit), s.radius * unit),
            None => (feet, 0.0),
        })
        .collect()
}

/// Where a ray enters an axis-aligned box (distance along the unit `dir`,
/// clamped at 0), if within `limit`.
pub fn box_entry(min: Vec3, max: Vec3, from: Vec3, dir: Vec3, limit: f32) -> Option<f32> {
    let (mut enter, mut leave) = (f32::MIN, f32::MAX);
    for i in 0..3 {
        if dir[i].abs() < 1e-9 {
            if from[i] < min[i] || from[i] > max[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((min[i] - from[i]) / dir[i], (max[i] - from[i]) / dir[i]);
        enter = enter.max(a.min(b));
        leave = leave.min(a.max(b));
    }
    (enter <= leave && leave >= 0.0 && enter.max(0.0) <= limit).then_some(enter.max(0.0))
}

/// The direction after a shoot-through surface (Corrections 9): `dir`
/// plus `k_r` / 1000 and `k_u` / 1000 on the right and up axes of a look
/// rotation along it with world up, normalised.
pub fn reperturb(dir: Vec3, k_r: i32, k_u: i32) -> Vec3 {
    let right = Vec3::Y.cross(dir).try_normalize().unwrap_or(Vec3::X);
    let up = dir.cross(right);
    (dir + right * (k_r as f32 / 1000.0) + up * (k_u as f32 / 1000.0)).normalize()
}

/// A whole number drawn uniformly from {-p..p} with `u` in [0, 1).
pub fn whole_draw(u: f32, p: i32) -> i32 {
    let p = p.max(0);
    ((u * (2 * p + 1) as f32) as i32).min(2 * p) - p
}

/// Whether a ray of the server's spacing check (Corrections 3) passes:
/// a shot of `rays` rays at `now` (s) whose last ray only is checked
/// against `last` (the last ray that passed or wasn't checked) and
/// `delay` (s), strictly in whole ms; every other ray stamps the clock.
/// Returns, per ray, whether it lands, and updates `last`.
pub fn spacing(rays: u32, now: f64, delay: f64, last: &mut Option<f64>) -> Vec<bool> {
    (0..rays)
        .map(|i| {
            if i + 1 == rays {
                let early = last.is_some_and(|t| ((now - t) * 1000.0 + 1e-6).floor() < delay * 1000.0 - 1e-6);
                if early {
                    return false;
                }
            }
            *last = Some(now);
            true
        })
        .collect()
}

/// After the frame: deliver each round, return the recoil, step the
/// spread, kick the view.
#[allow(clippy::type_complexity)]
fn after_shots(
    mut events: ParamSet<(MessageReader<WeaponEvent>, MessageWriter<WeaponEvent>)>,
    mut owners: Query<
        (
            Entity,
            &Inventory,
            &Intent,
            &MovementState,
            &Velocity,
            Option<&mut ViewPunch>,
            Option<&super::movement::CaMovement>,
            Option<&mut CaKick>,
            Option<&crate::weapon::impact::RecoilGame>,
        ),
    >,
    mut weapons: Query<(
        Option<&mut CaDamage>,
        Option<&mut CaSpread>,
        Option<&CaRecoil>,
        Option<&Hitscan>,
        Option<&AltModes>,
        Option<&Burst>,
        Option<&Zoom>,
        Option<&CaZoom>,
    )>,
    mut delivery: Delivery,
    exact: Query<(), With<ExactAim>>,
    clock: Res<SimClock>,
    scales: Res<SpreadScales>,
    (options, carry_over, server_settings): (Res<SpreadOptions>, Res<PelletCarryOver>, Res<CaServerSettings>),
    mut commands: Commands,
) {
    let now = clock.now;
    let all: Vec<WeaponEvent> = events.p0().read().cloned().collect();
    // The server's reload windows (weapon_classes.md Corrections 2).
    if delivery.authoritative() && delivery.first.0 {
        let started: Vec<Entity> = all
            .iter()
            .filter(|e| !e.replay && matches!(e.kind, WeaponEventKind::ReloadStarted))
            .map(|e| e.weapon)
            .collect();
        for w in started {
            if let Ok((Some(mut dmg), ..)) = weapons.get_mut(w) {
                let rate = 1.0 + dmg.mods.reload_rate as f64;
                let mark = server_reload_mark(dmg.reload_third_person as f64, rate, server_settings.reload_check_time);
                dmg.reload_mark = Some(now + mark);
            }
        }
    }
    let rounds: Vec<Round> = all
        .iter()
        .filter_map(|e| match e.kind {

            WeaponEventKind::Fired {
                origin,
                yaw,
                pitch,
                seed,
                mode,
                ..
            } => Some(Round {
                owner: e.owner,
                weapon: e.weapon,
                origin,
                yaw,
                pitch,
                seed,
                mode,
                replay: e.replay,
            }),
            _ => None,
        })
        .collect();

    // Each character's body yaw, for the behind test.
    let yaws: HashMap<Entity, f32> = if rounds.is_empty() {
        HashMap::new()
    } else {
        owners
            .iter()
            .map(|(e, inv, intent, ..)| {
                let kept = inv.active.and_then(|w| weapons.get(w).ok()).and_then(|w| w.1.and_then(|s| s.body_yaw));
                (e, body_yaw(intent.yaw, kept))
            })
            .collect()
    };

    // Deliver, with the cone the meters give before this tick's steps.
    let mut out: Vec<WeaponEvent> = Vec::new();
    for r in &rounds {
        let Ok((Some(mut dmg), spread, _, scan, _, burst, zoom, ca_zoom)) = weapons.get_mut(r.weapon) else {
            continue;
        };
        let mode = mode_of(r.mode, burst, zoom, ca_zoom);
        let exact_aim = exact.contains(r.owner);
        let half = if exact_aim {
            0.0
        } else {
            spread.as_ref().map_or(0.0, |s| s.cone(mode)) / 1000.0
        };
        let range = scan.map_or(0.0, |s| s.range);
        let server = delivery.authoritative() && delivery.first.0 && !r.replay;
        // The rays (attachments.md Corrections 1): the client sends n +
        // floor(n v) with the magazine part's `VectorsPerRoundPer`, the
        // server expects the muzzle part's; when they differ it ignores
        // the whole shot.
        let n = if exact_aim { 1 } else { scan.map_or(1, |s| s.pellets.max(1)) };
        let (sent, expected) = if exact_aim {
            (1, 1)
        } else {
            (dmg.mods.rays(n, dmg.mods.pellets_client), dmg.mods.rays(n, dmg.mods.pellets_server))
        };
        // The server's spacing (Corrections 3): checked on the last ray
        // only; every earlier ray stamps the clock with the shot's time.
        // A shot during the server's reload window is dropped (no server
        // ammo spent, Corrections 2).
        let reloading = dmg.reload_mark.is_some_and(|m| now + 1e-9 < m);
        if !reloading && dmg.reload_mark.is_some() && server {
            dmg.reload_mark = None;
        }
        let lands = if server && sent == expected && !reloading {
            spacing(sent, now, dmg.fire_delay, &mut dmg.last_round)
        } else {
            vec![false; sent as usize]
        };

        // The attachments (attachments.md, "Damage" and "Ray length"): the
        // base damage x (1 + sum Instdamage); the shooter's ray (the
        // client test) Range + trunc(Range x sum RangePer), the server's
        // own walk Range x (1 + the sight's).
        let mut eff = dmg.clone();
        eff.damage *= 1.0 + eff.mods.damage;
        let units = (range / eff.unit).round() as f64;
        let client_range = if eff.mods.range_per != 0.0 {
            eff.mods.client_range(units) as f32 * eff.unit
        } else {
            range
        };
        let server_range = if eff.mods.sight_range_per != 0.0 {
            eff.mods.server_range(units) as f32 * eff.unit
        } else {
            range
        };
        let dmg = &eff;
        let aim = aim_of(r.yaw, r.pitch);
        let rewound = delivery.rewind(r.owner, server);
        let bodies = delivery.bodies(r.owner, &rewound, &yaws);
        // Weapon_classes.md 5.7: a pellet that hits no one reports the
        // shot's most recent real hit.
        let mut carry = CarryOver::new(carry_over.0);
        for (pellet, land) in lands.into_iter().enumerate() {
            let mut rng = Rng::new(r.seed.wrapping_add(1 + pellet as u32));
            let (u1, u2) = (rng.next(), rng.next());
            let dir = square_dir(aim, half, u1, u2);
            let real = client_test(&mut delivery, r, dir, client_range, &bodies, &mut out);
            let report = carry.pellet(real).or(real);
            // The reported hit: the roll, from behind (the body's), the
            // band at the reported point; then the server's own walk.
            let u = rng.next();
            let mut first = None;
            if let Some(c) = report {
                let b = &bodies[c.body];
                let behind = is_behind(b.forward, b.right, c.dir);
                let d = base_damage(dmg, u, behind, c.distance);
                if land {
                    hurt(&mut delivery, dmg, r, b, c.sphere, d, c.point, c.dir, &mut out);
                }
                first = Some((d, c.distance));
            }
            let walk = Walk {
                dir,
                range: server_range,
                lands: land,
                skip_first: report.is_some(),
                first,
            };
            server_walk(&mut delivery, dmg, r, walk, &bodies, &mut rng, &mut out);
        }
    }

    // The recoil returns, the spread steps for each owner's active
    // weapon, then the kicks.
    let dt = clock.dt();
    for (owner, inv, intent, state, vel, mut punch, ca_move, kick, recoil_game) in &mut owners {
        let active = inv.active.and_then(|a| weapons.get_mut(a).ok().map(|w| (a, w)));
        let (recoil, mode, burst_mode) = match &active {
            Some((_, (_, _, recoil, _, modes, burst, zoom, ca_zoom))) => {
                let current = modes.map_or(0, |m| m.current);
                (
                    recoil.cloned().unwrap_or_else(|| DEFAULT_RECOIL.clone()),
                    mode_of(current, *burst, *zoom, *ca_zoom),
                    fire_mode(current, *burst, *zoom) == FireMode::Burst,
                )
            }
            None => (DEFAULT_RECOIL.clone(), FireMode::Normal, false),
        };
        let (burst_part, zoom_part) = match &active {
            Some((_, w)) => (w.5.cloned(), w.6.cloned()),
            None => (None, None),
        };
        let mut k = kick.as_deref().cloned().unwrap_or_default();
        // Holding another game's weapon (the gun game's swap): that game's
        // recoil owns the view punch (`weapon::impact::RecoilGame`, which
        // zeroed it at the switch); this game's kick ends, nothing returns.
        // (A weapon with Combat Arms' recoil data is its own whatever its
        // id: tests' `test:` guns.)
        let ours = crate::weapon::impact::RecoilGame::is(recoil_game, crate::game::GAME)
            || active.as_ref().is_some_and(|(_, w)| w.2.is_some());
        if !ours {
            k.pitch = 0.0;
            k.yaw = 0.0;
        }
        // The return, with the data of the weapon in hand (Corrections 1).
        if ours && (k.pitch > 0.0 || k.yaw != 0.0) {
            let (down, side) = recoil.recover(&mut k, dt, now, burst_mode);
            add_punch(&mut punch, &mut commands, owner, Vec3::new(-down, side, 0.0));
        }
        let mine: Vec<&Round> = active
            .as_ref()
            .map(|(a, _)| rounds.iter().filter(|r| r.owner == owner && r.weapon == *a).collect())
            .unwrap_or_default();
        if let Some((_, (_, Some(mut s), ..))) = active {
            let tick = SpreadTick {
                intent,
                state,
                movement: ca_move,
                velocity: vel.0,
                punch: punch.as_deref(),
                mode,
                fired: !mine.is_empty(),
                now,
                dt,
            };
            step_spread(&mut s, &scales, &options, &tick);
        }
        if ours && !exact.contains(owner) {
            for (i, r) in mine.iter().enumerate() {
                let burst = fire_mode(r.mode, burst_part.as_ref(), zoom_part.as_ref()) == FireMode::Burst;
                let up = recoil.kick(&mut k, burst, now);

                let roll = shared_random(intent.command, "combat_arms:wag", i as i32).next();
                let side = recoil.wag(&mut k, roll);
                add_punch(&mut punch, &mut commands, owner, Vec3::new(up, side, 0.0));
            }
        }
        match kick {
            Some(mut have) => {
                if *have != k {
                    *have = k;
                }
            }
            None => {
                if k != CaKick::default() {
                    commands.entity(owner).insert(k);
                }
            }
        }
    }
    // A spawn-protected character's hits show nothing (Chimera's
    // `SpawnProtected`; plain Combat Arms' own protection is `CaProtected`,
    // whose blood the effects leave out).
    let at = delivery.now.now;
    let hides = |t: Entity| delivery.spawn_protected.get(t).is_ok_and(|p| p.hides_hits(at));
    let out: Vec<WeaponEvent> = out
        .into_iter()
        .filter_map(|e| {
            let kind = crate::weapon::impact::hide_protected(e.kind, hides)?;
            Some(WeaponEvent { kind, ..e })
        })
        .collect();
    let mut writer = events.p1();
    for e in out {
        writer.write(e);
    }
}

/// Add `delta` to `owner`'s view punch (giving it one).
fn add_punch(punch: &mut Option<Mut<ViewPunch>>, commands: &mut Commands, owner: Entity, delta: Vec3) {
    match punch.as_mut() {
        Some(p) => p.0 += delta,
        None => {
            commands.entity(owner).insert(ViewPunch(delta));
        }
    }
}

/// The movement fraction mu: the speed over the tick's speed limit, at
/// most 1 (no limit: 1).
pub fn movement_fraction(speed: f32, limit: f32) -> f32 {
    if limit > 0.0 { (speed / limit).min(1.0) } else { 1.0 }
}

/// The turn term of each of a tick's `n` sub-steps for a body turn of
/// `turned` radians (Q13): all of it on the first, 0 on the rest.
pub fn substep_turns(scales: &SpreadScales, turned: f32, n: u32) -> Vec<f32> {
    (0..n).map(|i| if i == 0 { CaSpread::turn(scales, 0.0, turned) } else { 0.0 }).collect()
}

/// What one tick of the spread meters reads of the owner.
struct SpreadTick<'a> {
    intent: &'a Intent,
    state: &'a MovementState,
    /// Combat Arms' movement, when the owner has it: its run flag, "in a
    /// jump" and speed limit.
    movement: Option<&'a super::movement::CaMovement>,
    velocity: Vec3,
    punch: Option<&'a ViewPunch>,
    mode: FireMode,
    fired: bool,
    now: f64,
    dt: f32,
}

/// The movement target S in a jump (Corrections 12): the zoomed
/// `ZoomedJumpPerturb` only zoomed with the run flag off and non-zero,
/// else the plain `JumpPerturb`.
pub fn jump_target(params: &SpreadParams, mode: FireMode, run: bool) -> f32 {
    let z = params.zoomed_state.jump;
    if mode == FireMode::Zoomed && !run && z != 0.0 { z } else { params.state.jump }
}

/// One tick of a weapon's spread meters (the spec's step 8).
fn step_spread(s: &mut CaSpread, scales: &SpreadScales, options: &SpreadOptions, k: &SpreadTick) {
    let (intent, state) = (k.intent, k.state);
    // Airborne from a jump (Q12): movement.md's "in a jump"; without that
    // movement, left the ground with the jump key held, until landing.
    if state.on_ground {
        s.jumped = false;
    } else if s.was_on_ground {
        s.jumped = intent.jump;
    }
    s.was_on_ground = state.on_ground;
    let jumping = k.movement.map_or(s.jumped && !state.on_ground, |m| m.in_jump);
    let cone = s.cone_params(k.mode);
    let st = s.states(k.mode);
    // "No spread" holds both meters at 0: the plain `MaxPerturb` 0
    // (Corrections 12), whatever the mode's.
    if s.params.cone.max == 0 {
        s.fire = 0.0;
        s.movement = 0.0;
        return;
    }
    let keys = intent.move_axis != Vec2::ZERO || intent.opposed.any();
    let cap = match (keys, state.crouching) {
        (true, true) => st.fire_move_duck,
        (true, false) => st.fire_move,
        (false, true) => st.fire_duck,
        (false, false) => st.fire,
    };
    // The run flag (Q6): not crouched and not walking, at rest too.
    let run = k.movement.map_or(!intent.walk && !state.crouching, |m| m.run);
    let target = if state.on_ladder {
        st.run
    } else if jumping {
        jump_target(&s.params, k.mode, run)
    } else if run {
        st.run
    } else {
        st.walk
    };
    // mu: against the tick's speed limit, penalties included (Q6).
    let vmax = match k.movement {
        _ if state.on_ladder => LADDER_SPEED,
        Some(m) => m.speed_limit,
        None if state.crouching => CROUCH_SPEED,
        None if run => RUN_SPEED,
        None => WALK_SPEED,
    };
    let mu = movement_fraction(k.velocity.length() / s.unit, vmax);
    let mu_s = if state.on_ladder { target } else { mu * target };
    s.clock += k.dt as f64;
    let mut n = ((s.clock / SPREAD_SUBSTEP + 1e-9).floor() as u32).min(SPREAD_SUBSTEP_MAX);
    if n > 0 {
        s.clock = if options.quantised { 0.0 } else { (s.clock - n as f64 * SPREAD_SUBSTEP).max(0.0) };
    } else if k.fired {
        n = 1;
    }
    // The turn (Q13): the body yaw's change, all on the first sub-step.
    let wag = k.punch.map_or(0.0, |p| p.0.y);
    let turned = s.body_turn(intent.yaw, wag, keys);
    // The falling branch's clamp is the plain `JumpPerturb` (Corrections
    // 12).
    let jump_cap = s.params.state.jump;
    for (i, rho) in substep_turns(scales, turned, n).into_iter().enumerate() {
        s.move_substep(scales, cone, target, mu_s, rho, jumping, jump_cap);
        s.fire_substep(scales, cone, k.fired && i == 0, cap, k.now);
    }
}

/// A pellet's real hit on a character in the shooter's own test, the hit
/// it reports (weapon_classes.md 5.7's carry-over reports one too).
#[derive(Clone, Copy, Debug)]
struct PelletHit {
    /// Index in the round's bodies.
    body: usize,
    sphere: usize,
    /// The reported point: the widened box's entry (Corrections 8).
    point: Vec3,
    dir: Vec3,
    /// From the round's origin, metres.
    distance: f32,
}

/// The shooter's own test of one ray (the spec's "Client test"): the
/// widened boxes whose spheres the whole segment passes, the nearest
/// before any wall; then the plain test decides what is reported
/// (Corrections 8): the same character at the widened box's entry, or
/// something else whose plane has the victim's origin behind it (no
/// player hit), else still the character. A live grenade in front stops
/// the round. Its `Shot` event; the character hit, if any. Never through
/// anything.
fn client_test(w: &mut Delivery, r: &Round, dir: Vec3, range: f32, bodies: &[Body], out: &mut Vec<WeaponEvent>) -> Option<PelletHit> {
    let excluded: Vec<Entity> = std::iter::once(r.owner).chain(w.characters.iter().map(|(e, ..)| e)).collect();
    let filter = SpatialQueryFilter::from_excluded_entities(excluded).with_mask(SOLID_LAYERS);
    let from = r.origin;
    let event = |to: Vec3, hit: Option<Entity>, normal: Option<Vec3>| WeaponEvent {
        owner: r.owner,
        weapon: r.weapon,
        kind: WeaponEventKind::Shot { from, to, hit, normal },
        replay: r.replay,
    };
    let Ok(d3) = Dir3::new(dir) else { return None };
    let world = w.spatial.cast_ray(from, d3, range, true, &filter);
    let wall = world.as_ref().map_or(range, |h| h.distance);
    let end = from + dir * range;
    // The nearest character whose widened box the ray enters before the
    // wall and whose spheres the whole segment passes.
    let mut best: Option<(f32, usize, usize)> = None;
    for (bi, b) in bodies.iter().enumerate() {
        let c = (b.min + b.max) / 2.0;
        let half = (b.max - b.min) / 2.0 * Vec3::new(HITBOX_WIDEN, 1.0, HITBOX_WIDEN);
        let Some(t) = box_entry(c - half, c + half, from, dir, wall) else {
            continue;
        };
        if best.is_some_and(|(bt, ..)| bt <= t) {
            continue;
        }
        let Some(si) = b.shapes.by_power(from, end) else {
            continue;
        };
        best = Some((t, bi, si));
    }
    // A live grenade's box before them (grenades.md 13): the round stops
    // on its metal (the server's walk deals its damage).
    let grenade = w
        .grenades
        .iter()
        .filter_map(|(e, g)| {
            let (c, half) = (g.pos * g.unit, Vec3::from(g.params.box_half) * g.unit);
            box_entry(c - half, c + half, from, dir, wall).map(|t| (t, e))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((t, hit)) = grenade
        && best.is_none_or(|(bt, ..)| t < bt)
    {
        out.push(event(from + dir * t, Some(hit), Some(-dir)));
        return None;
    }
    let world_hit = world.map(|h| (from + dir * h.distance, h.normal, w.body(h.entity)));
    let Some((t_wide, bi, si)) = best else {
        out.push(match world_hit {
            Some((point, normal, body)) => event(point, Some(body), Some(normal)),
            None => event(end, None, None),
        });
        return None;
    };
    let b = &bodies[bi];
    // The plain test: what it meets first.
    let plain = |o: &Body| box_entry(o.min, o.max, from, dir, wall).filter(|_| o.shapes.by_power(from, end).is_some());
    let own = plain(b);
    let other = bodies
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != bi)
        .filter_map(|(i, o)| plain(o).map(|t| (t, i)))
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let own_first = own.is_some_and(|t| other.is_none_or(|(o, _)| t <= o));
    if !own_first {
        let something = match other {
            Some((t, i)) => Some((from + dir * t, -dir, bodies[i].entity)),
            None => world_hit,
        };
        if let Some((point, normal, thing)) = something
            && (b.origin - point).dot(normal) < 0.0
        {
            out.push(event(point, Some(thing), Some(normal)));
            return None;
        }
    }
    let point = from + dir * t_wide;
    out.push(event(point, Some(b.entity), Some(-dir)));
    Some(PelletHit {
        body: bi,
        sphere: si,
        point,
        dir,
        distance: t_wide,
    })
}

/// How the server walks one ray.
struct Walk {
    dir: Vec3,
    /// The server's ray length (m).
    range: f32,
    /// Its damage lands (the server, the shot accepted, the ray not
    /// dropped by the spacing).
    lands: bool,
    /// A player hit was reported: the walk skips the first character box
    /// it meets.
    skip_first: bool,
    /// The reported hit's damage and distance (a second victim's start).
    first: Option<(f32, f32)>,
}

/// The server's own walk of one ray (the spec's "Server" and
/// "Penetration"): from the fire position, at most `SERVER_INTERSECTIONS`
/// intersections (every surface, object or skipped character box counts
/// one); shoot-through surfaces pass (the back-cast from the limit + 1
/// unit, the restart point, the range left from the fire position, the
/// damage dampened, the whole-thousandth re-perturb); a live grenade
/// stops it with its damage; the first character box after the skipped
/// one is hit in the sphere of the highest priority on the line
/// (`priority_sphere`), on its unwidened box, and the walk ends there
/// (none on the line: it goes on 5 units past the box's entry). That
/// character's damage: a reported hit's running value banded again at
/// the reported distance, else base x roll x the band (twice, at the real
/// distance: Q13's recommendation), dampened; no behind bonus.
/// `ShotContinued` events past the first intersection (the shooter's test
/// drew that one).
fn server_walk(w: &mut Delivery, dmg: &CaDamage, r: &Round, walk: Walk, bodies: &[Body], rng: &mut Rng, out: &mut Vec<WeaponEvent>) {
    let excluded: Vec<Entity> = std::iter::once(r.owner).chain(w.characters.iter().map(|(e, ..)| e)).collect();
    let filter = SpatialQueryFilter::from_excluded_entities(excluded).with_mask(SOLID_LAYERS);
    let unit = dmg.unit;
    let mut from = r.origin;
    let mut dir = walk.dir;
    let mut left = walk.range;
    let mut dampen = 1.0;
    let mut skip: Vec<Entity> = Vec::new();
    let mut to_skip = walk.skip_first;
    for i in 0..SERVER_INTERSECTIONS {
        if left < unit {
            break;
        }
        let Ok(d3) = Dir3::new(dir) else { break };
        let world = w.spatial.cast_ray(from, d3, left, true, &filter);
        let wall = world.as_ref().map_or(left, |h| h.distance);
        let show = i > 0;
        let event = |from: Vec3, to: Vec3, hit: Option<Entity>, normal: Option<Vec3>| WeaponEvent {
            owner: r.owner,
            weapon: r.weapon,
            kind: WeaponEventKind::ShotContinued { from, to, hit, normal },
            replay: r.replay,
        };
        let body = bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| !skip.contains(&b.entity))
            .filter_map(|(bi, b)| box_entry(b.min, b.max, from, dir, wall).map(|t| (t, bi)))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let grenade = w
            .grenades
            .iter()
            .filter_map(|(e, g)| {
                let (c, half) = (g.pos * g.unit, Vec3::from(g.params.box_half) * g.unit);
                box_entry(c - half, c + half, from, dir, wall).map(|t| (t, e))
            })
            .chain(w.objects.iter().filter_map(|(e, t)| {
                box_entry(t.centre - t.half, t.centre + t.half, from, dir, wall).map(|t| (t, e))
            }))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        // A live grenade's box (grenades.md 13): the round stops on its
        // metal, its damage (with the range band, no zone) off the
        // grenade's hit points.
        if let Some((t, hit)) = grenade
            && body.is_none_or(|(bt, _)| t < bt)
        {
            let point = from + dir * t;
            if show {
                out.push(event(from, point, Some(hit), Some(-dir)));
            }
            let u = rng.next();
            if walk.lands {
                let d = base_damage(dmg, u, false, point.distance(r.origin)) * dampen;
                w.damage.write(Damage {
                    target: hit,
                    attacker: Some(r.owner),
                    amount: d,
                    point,
                    dir,
                    hitgroup: Hitgroup::Generic,
                    kind: DamageKind::Bullet,
                    weapon: None,
                    force: Vec3::ZERO,
                });
            }
            return;
        }
        if let Some((t, bi)) = body {
            let b = &bodies[bi];
            let at = from + dir * t;
            if to_skip {
                // The reported victim's box (or the first met).
                to_skip = false;
                skip.push(b.entity);
                left -= t;
                from = at;
                continue;
            }
            let Some(si) = b.shapes.by_priority(at, dir) else {
                // The line passes it: on, 5 units past its entry.
                skip.push(b.entity);
                let step = SERVER_SKIP * unit;
                left -= t + step;
                from = at + dir * step;
                continue;
            };
            if show {
                out.push(event(from, at, Some(b.entity), Some(-dir)));
            }
            let u = rng.next();
            let d = match walk.first {
                // One behind the reported victim (Q5): its damage banded
                // again at the reported distance.
                Some((prev, distance)) => prev * band_scale(dmg, distance),
                None => {
                    let distance = at.distance(r.origin);
                    base_damage(dmg, u, false, distance) * band_scale(dmg, distance)
                }
            };
            if walk.lands {
                hurt(w, dmg, r, b, si, d * dampen, at, dir, out);
            }
            return;
        }
        let Some(hit) = world else {
            if show {
                out.push(event(from, from + dir * left, None, None));
            }
            return;
        };
        let point = from + dir * hit.distance;
        let body = w.body(hit.entity);
        if show {
            out.push(event(from, point, Some(body), Some(hit.normal)));
        }
        // Something that takes damage (a map's breakable): the round's
        // damage, banded and dampened as a character's, no zone.
        let u = rng.next();
        let d = base_damage(dmg, u, false, point.distance(r.origin)) * dampen;
        // Its push (`IMPULSE_PER_HIT_POINT`): on a physics object, and as
        // the damage's force (what wakes a map's props).
        let push = dir * d * HIT_POINTS * IMPULSE_PER_HIT_POINT;
        if walk.lands && w.bodies.get(body).is_ok_and(|(rb, _)| rb.is_dynamic()) {
            w.impulses.write(crate::weapon::impact::BulletImpulse {
                body,
                point,
                impulse: push,
            });
        }
        if walk.lands && (w.ground.2.contains(hit.entity) || w.ground.2.contains(body)) {
            w.damage.write(Damage {
                target: if w.ground.2.contains(body) { body } else { hit.entity },
                attacker: Some(r.owner),
                amount: d,
                point,
                dir,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Bullet,
                weapon: None,
                force: push,
            });
        }
        // Through it? A collider's surface, else the world's there.
        let name = w.surface_name(hit.entity, point);
        let Some(pass) = w.surface_at(hit.entity, point) else { return };
        let restart = if pass.thickness <= 0.0 {
            point
        } else {
            // Cast back from the limit + 1 unit beyond the entry to 1 unit
            // before it: anything it meets passes (the thickness itself is
            // never compared).
            let reach = pass.thickness + unit;
            let far = point + dir * reach;
            let back = Dir3::new(-dir).unwrap_or(Dir3::NEG_Z);
            let Some(h) = w.spatial.cast_ray(far, back, reach + unit, true, &filter) else { return };
            if h.distance <= 0.0 {
                // Still inside it at the limit: too thick.
                return;
            }
            // An object (not the world) must be thinner than the limit on
            // some axis too.
            if let Ok((rb, aabb)) = w.bodies.get(body)
                && !rb.is_static()
                && aabb.is_some_and(|b| (b.max - b.min).min_element() >= pass.thickness)
            {
                return;
            }
            // The exit when its surface is the entry's, else the entry.
            let exit = far - dir * h.distance;
            if w.surface_name(h.entity, exit) == name { exit } else { point }
        };
        left = (walk.range - restart.distance(r.origin)) * pass.range;
        dampen *= pass.damage;
        let p = pass.perturb.round() as i32;
        let (k_r, k_u) = (whole_draw(rng.next(), p), whole_draw(rng.next(), p));
        dir = reperturb(dir, k_r, k_u);
        from = restart + dir * 1e-3;
    }
}

/// Damage `d` (health units, before the victim) to the sphere `si` of
/// `b`: the attachments' victim factor, the AI factor, armour and zone,
/// the AI cap, the death threshold.
#[allow(clippy::too_many_arguments)]
fn hurt(
    w: &mut Delivery,
    dmg: &CaDamage,
    r: &Round,
    b: &Body,
    si: usize,
    d: f32,
    point: Vec3,
    dir: Vec3,
    out: &mut Vec<WeaponEvent>,
) {
    // The shape hit: its region (its hitgroup, armour location) and zone
    // factor (the body's own, Combat Arms' spheres'; else by region).
    let shape = b.shapes.0[si];
    let location = location_of(shape.region);
    let factor = shape.scale.unwrap_or_else(|| zone_factor(shape.region));
    // Hit points from here on (armour points are).
    let hp = 0.01;
    let hitgroup = shape.region.hitgroup();
    let mut damage = Damage {
        target: b.entity,
        attacker: Some(r.owner),
        amount: 0.0,
        point,
        dir,
        hitgroup,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    };
    let lands = w.rules.lands(&damage);
    let infected = w.infected.contains(b.entity);
    // The attachments' special functions (attachments.md, "Bullet slot"):
    // the victim-side factor before armour, by target group and
    // probability (each function's own draw).
    let npc = w.attach.0.get(b.entity).ok().copied();
    let victim = Victim {
        npc: npc.is_some(),
        zombie: infected || npc.is_some_and(|n| n.zombie),
    };
    let forced = w.attach.1.0;
    let mut draws = Rng::new(r.seed.wrapping_mul(0x9e37_79b9).wrapping_add(si as u32));
    let mut draw = |_: usize| forced.unwrap_or_else(|| draws.next());
    // The shooter's gear first (Fighting Chance, then the sets: hit
    // points, attacker side before armour).
    let d = match (w.dealt.get(r.owner), w.ids.get(r.weapon), &w.gear_data) {
        (Ok(fx), Ok(weapon), Some(data)) => fx.dealt(&data.0, weapon.id, d / hp) * hp,
        _ => d,
    };
    // Then an AI's damage x `AIDamageFactor` (Corrections 7).
    let d = d * dmg.mods.victim_factor(victim, &mut draw) * w.ai.factor(Some(r.owner));
    let enemy = !w.same_team(r.owner, b.entity);
    // Stun: the shooter's passing Stun functions slow the victim (movement's
    // P_stun) for their duration.
    if lands
        && enemy
        && let Some((penalty, duration)) = dmg.mods.stun(SpecialKind::Stun, victim, &mut draw)
    {
        w.commands.entity(b.entity).insert(super::movement::StunPenalty {
            penalty,
            until: w.now.now + duration as f64,
        });
    }
    let knock = dmg.mods.stun(SpecialKind::Knockback, victim, &mut draw).unwrap_or_default();
    let ff = w.friendly_scale(r.owner, b.entity);
    let hit = ArmourHit {
        damage_type: dmg.damage_type,
        location,
        zone: if w.mercs.contains(r.owner) { 1.0 } else { factor },
        infected,
    };
    let after = match w.armor.get_mut(b.entity) {
        Ok((a, pieces)) => armour_and_zone(d * ff / hp, hit, Some(a.into_inner()), pieces, lands),
        Err(_) => armour_and_zone(d * ff / hp, hit, None, None, lands),
    };
    // An AI's hit on a player is capped (not melee).
    let points = w.ai.cap(Some(r.owner), b.entity, after, dmg.damage_type == MELEE) * hp;
    // An infected victim hit by the other team is knocked back (movement.md,
    // "Hit knock-back").
    if infected && lands && enemy {
        let stiff = w.figures.get(b.entity).ok().flatten().map_or(super::movement::movement_data().character.figure.stiff_duration, |c| c.figure.stiff_duration);
        let current = w.knockback.get(b.entity).ok();
        if let Some(k) =
            super::movement::HitKnockback::hit(current, dmg.move_penalty + knock.0, dmg.move_penalty_duration + knock.1, stiff, dir, w.now.now)
        {
            w.commands.entity(b.entity).insert(k);
        }
    }
    let health = w.characters.get(b.entity).map_or(0.0, |(.., h, _, _, _)| h.current);
    // Dead at 1 HP or less.
    damage.amount = if points > 0.0 && health - points <= DEATH_THRESHOLD * hp + 1e-6 {
        points.max(health)
    } else {
        points
    };
    out.push(WeaponEvent {
        owner: r.owner,
        weapon: r.weapon,
        kind: WeaponEventKind::Hit {
            target: b.entity,
            amount: damage.amount,
            hitgroup,
        },
        replay: r.replay,
    });
    // The rules' friendly-fire step scales the message again (S8): it
    // carries the amount before that step.
    damage.amount /= ff;
    if let Some(regions) = w.regions.as_mut() {
        regions.write(crate::core::RegionHit {
            target: b.entity,
            region: shape.region,
        });
    }
    w.damage.write(damage);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_surface_table_parses() {
        let t = CaSurfaces::built_in();
        assert_eq!(t.get("Wood").map(|s| s.damage), Some(0.5));
        assert_eq!(t.get("plywood").map(|s| s.damage), Some(0.6));
        assert_eq!(t.get("water").map(|s| s.thickness), Some(0.0));
        assert!(t.get("concrete").is_none());
    }

    #[test]
    fn spheres_are_named_once() {
        for set in [*MALE, *FEMALE] {
            for (i, s) in set.iter().enumerate() {
                assert!(set[..i].iter().all(|o| o.name != s.name), "{}", s.name);
            }
        }
    }
}
