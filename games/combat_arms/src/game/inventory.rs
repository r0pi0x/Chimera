//! Combat Arms' inventory: what a player carries and how it switches,
//! from specs/combat_arms/inventory.md (section numbers below are its).
//!
//! - Classes (1): a weapon's class is its data `slot` + 1 (0 primary,
//!   1 secondary, 2 melee, 3 throw, 4 special, 5 objective; any other
//!   slot is class 0: never selected by key, never dropped). The list is
//!   `weapon::Inventory::weapons` (its index is the equip index, at most
//!   `MAX_CARRIED`); a class's order slots are its weapons in that order.
//!   The loadout is `weapon::StartingWeapons` in loadout order (primary,
//!   secondary, melee, throw, then backpack items); at spawn the lowest
//!   class is drawn. Per-weapon data the frame doesn't have is the
//!   `combat_arms:inventory` hook (`CaItem`: hide-when-empty, drop on
//!   death, infinite ammo, the post-sprint time, the select-rate bonus).
//! - Switching (3, 4, 6): `CaSelection` on each character keeps the class
//!   cursors, each class's last chosen weapon and the previous weapon (Q).
//!   Class keys (`Intent::select`, slot k = class k + 1) select or cycle
//!   with no re-draw; Q (`Intent::last_weapon`) follows section 4 to the
//!   letter (it overwrites "previous" before checking). Chimera's
//!   selection is told to leave keys to us (`weapon::OwnSelection`) and
//!   only applies `Inventory::wanted`: the holster is instant (its reload,
//!   burst and zoom abandoned) and the draw is `Weapon::draw_time`, the
//!   select animation's length over the played rate (`played_rate`: above
//!   `DRAW_RATE_CAP` it plays at 1). A magazine weapon drawn with an empty
//!   clip and spare rounds reloads at once instead of drawing (6.6).
//! - What interrupts what (7): `hands` runs before the weapon frame: R
//!   during a magazine weapon's draw starts the reload at once (Quirk 2);
//!   sprint (`MovementState::sprinting`) puts an idle weapon in the sprint
//!   pose, which holds fire until sprint ends and the weapon's post-sprint
//!   time (`CaItem::sprint_out`) passes, and one update more (the update
//!   that sees the pose end only returns to idle); a reload started or
//!   running under sprint completes; holding use holds fire. Sprint and
//!   use end the firing state, so the first round after them, a draw or a
//!   reload is the trigger's chain (`Trigger::latency`, Corrections 1).
//! - The knife (8): the `combat_arms:knife` hook (`Knife`), its own state
//!   machine per update: the first update after a press arms it, the next
//!   starts the swing (the melee class plays no pre-fire: Corrections
//!   (2026-10-30) 1; an attack's `prefire` is not played); hit keys that
//!   pass in the same update make one hit (special_weapons.md 5.1); a map's
//!   breakable the swing meets takes its damage as a melee hit, as any
//!   knife's on those brushes (a CS:S window, a vent);
//!   each hit key casts one ray from the eye with the gun's square spread
//!   at the knife's cone (`perturb`, n = 1 for the M9), range, first thing
//!   hit, no penetration (on a network server a remote player's swing meets
//!   the others where its client drew them: lag compensation's
//!   `weapon::lagcomp::LagComp`, their boxes and shapes then, as its rounds), dealing `B x u x b x k_zone` after armour (u
//!   uniform in 1 +- the damage spread, b = 1 + `BEHIND_BONUS` from the
//!   victim's rear sector by its body yaw, `behind`, `weapons::body_yaw`:
//!   Corrections 2; an AI's x `AIDamageFactor`, uncapped;
//!   the zone of the hit sphere the ray
//!   passes, `weapons::pick_sphere`; melee gear coverage: none in stock); a
//!   held button chains the next swing at once, fire beating alt-fire. No
//!   quick-knife (9).
//! - Auto-switch (10): a hide-when-empty weapon in hand that is spent (or
//!   gone with its last round) gives the last chosen primary, else
//!   secondary, else melee, and clears Q.
//! - Pickup (11): `pickups` aims the view ray at dropped items (`Dropped`,
//!   with a `map::loose::LooseItem` to draw) each tick
//!   (`CaSelection::pickup_target`, for the HUD's prompt); E on one calls
//!   `pick_up`: a same-id non-primary adds its ammo (clamped), fills the
//!   clip and is drawn; otherwise it swaps for the class's last chosen
//!   weapon in the same equip index (the old one dropped at the left foot
//!   with jitter and a random yaw) and draws it with the item's clip (the
//!   server's "better weapon" rule, `better_weapon`, always switches: the
//!   weapon in hand was put away for the request). Only a real ray hit
//!   within 100 units targets an item (the fallback, `body_fallback`, is
//!   dead bodies'). Ammo travels with the gun (`Dropped::total`, `clip`).
//!   Corrections 2026-10-22 (faithful quirks): a primary item whose five
//!   part slots (empty ones filled with the weapon's defaults) and sight
//!   style equal the last chosen primary's is no swap: the carried weapon
//!   stays (whatever weapon the item is: a bare AK-47 item "matches" a
//!   bare M16A3) and its total and clip are SET to the item's; with no
//!   last chosen weapon in the class (none switched to since spawning)
//!   the request does nothing; an item reserved for a looter
//!   (`CaReserved`, Opportunist) is refused to others (the owner alone
//!   for `OwnerTime`, then the owner's team only). The client's HUD after
//!   a matching-set pickup is not traced (Open question 18); we show the
//!   real count.
//!   No walk-over pickup and no drop key.
//! - A thrown weapon stays at total 0 when its last grenade goes (10):
//!   `grenade::KeepSpent` on every throwable; its key skips it, Q refuses
//!   it, a respawn restores it.
//! - Death drop (12): the last chosen primary drops at the left foot (no
//!   jitter) when `CaDeathDrops` allows and its data does; items go after
//!   `DROPPED_LIFETIME`. Respawn (13): `respawn_inventory`.
//!
//! Known shortcuts:
//! - Units: movement's `meters_per_unit` (`UNIT`, by body size: movement.md
//!   Open question 3). Health 100 = 1.0 (`HP`).
//! - Updates are the game's fixed tick (movement.md's 15 ms,
//!   `game::TICK_INTERVAL`), not rendered frames as inventory.md has the
//!   weapon logic run (Open question 12: once per frame, after the frame's
//!   movement ticks), so every "next update" is a tick, as if the game
//!   ran at 66.7 fps; the server here runs the same logic (the spec's
//!   server only applies messages). Test case 37 runs with the tick set to
//!   the frame's length.
//! - Fire-gating by sprint and use applies to magazine weapons and the
//!   knife; thrown weapons are `grenades`' (sprint holds their fire there,
//!   use doesn't), on Chimera's `grenade::Throwable` (a spent one kept at
//!   total 0: `grenade::KeepSpent`). The cooked-grenade release on death
//!   (12.1) is grenades.md's (`grenades`). A shell-by-shell weapon's
//!   reload during sprint or its draw goes to Chimera's frame (refused
//!   while held).
//! - Reloads started here (6.6, Quirk 2, under sprint) play no reload
//!   sounds (the frame's sound queue is private).
//! - The left foot is the `Left_foot` node of the character's posed
//!   skeleton (`characters::CaPose`: a body model's, else the spec's
//!   "stand" pose, characters.md test case 27), the hull's bottom centre
//!   only for a character without one; the swap's jitter +-0.5 x the
//!   hull's half-extents; the knife's zones are `weapons`' hit spheres,
//!   on a character wearing another game's body (its identity's) that
//!   body's own hitboxes by the same rule (`weapons::their_shapes`).
//! - Pickup: an item's box is its body's, else 10 units each way (the
//!   reference's is its world model's extents: Open question 13, to be
//!   measured); no dead-body interactions (the fallback's only use). The
//!   owner/team, human-model, infection, VIP and buff checks, map-placed
//!   items' respawn and the auto weapon switch option (always on) aren't
//!   there; command ids aren't kept per weapon (the rule always switches
//!   in play); the non-primary pickup fills the clip as the server does
//!   (Open question 5).
//! - Class keys: the objective class (key 6, the mission bomb) is the
//!   front end's `slot6`; the bomb rule (key 6 does nothing once the bomb
//!   is planted: the planter's bomb is gone then), "in use" weapons,
//!   forced-select and the menu/vote/weapons-disabled conditions aren't
//!   modelled (no such weapons or states yet). Q and a
//!   class's last chosen weapon are kept as entities (an equip index plus
//!   id in the game: the same outcome when a pickup replaces one).
//! - Death drop: every character counts as a human model; the backpack
//!   perk's extra drops (Open question 8) aren't there; dropped items
//!   fall only where a map gives them a body (`map::loose`).
//! - Respawn: `respawn_inventory` rebuilds the loadout; the game modes
//!   (game_modes.md) decide when.

use avian3d::prelude::{ColliderAabb, ColliderOf, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    movement::CaInfected,
    weapons::{
        ArmourHit, HitSpheres, Location, MALE, MELEE as MELEE_DAMAGE, RIFLE, armour_and_zone, body_geometry,
        body_shapes, geometry_at, pick_sphere, square_dir,
    },
};
use crate::{
    core::{
        self, Damage, DamageKind, Died, FirstTimePredicted, Health, Hitgroup, Intent, MovementState, NetRole, Predict,
        PredictedAppExt, SimClock, SimSet,
    },
    map::{PhysicsProp, interp::NetDrawn, loose::LooseItem},
    weapon::{
        CarriedAppExt, Inventory, Magazine, NetPart, OwnSelection, SelectWeapons, ShellReload, SlotLastInv, StartingWeapons, Weapon,
        WeaponEvent, WeaponEventKind, WeaponFrame, WeaponState,
        data::{WeaponData, WeaponDataAppExt},
        grenade::{KeepSpent, Throwable},
        lagcomp::LagComp,
        random::{Rng, shared_random},
    },
};

/// Combat Arms' knife, the M9 (data/weapons/m9.toml).
pub const KNIFE: &str = "combat_arms:m9_knife";

/// What every Combat Arms player starts with, in loadout order (primary,
/// secondary, melee, throw, backpack; 1). PLACEHOLDER: the skeleton's
/// rifle until weapon_firing.md's, the M9 and the M67 (grenades.md).
pub const STARTING_WEAPONS: &[&str] = &[RIFLE, KNIFE, super::grenades::M67];

/// Metres per LithTech unit: movement's `meters_per_unit` (by body size;
/// see Known shortcuts).
pub const UNIT: f32 = 0.018;
/// Health per damage point (100 points = 1.0).
pub const HP: f32 = 0.01;

/// Weapons a player carries at most (max_carried).
pub const MAX_CARRIED: usize = 16;
/// Seconds the outgoing weapon takes to go away (holster_time): none.
pub const HOLSTER_TIME: f32 = 0.0;
/// An animation rate above this plays at 1.0 (draw_rate_cap).
pub const DRAW_RATE_CAP: f32 = 2.0;
/// Farthest a pickup's surface hit may be from the camera, m
/// (activation_distance, 100 units; exactly 100 counts).
pub const ACTIVATION_DISTANCE: f32 = 100.0 * UNIT;
/// Taken off the camera-to-centre distance for the fallback test of
/// dead bodies' hit boxes (activation_fallback_slack, 35 units); weapon
/// items never use it (Open question 13). No body interactions yet.
pub const ACTIVATION_FALLBACK_SLACK: f32 = 35.0 * UNIT;
/// Seconds a dropped item lasts (dropped_lifetime).
pub const DROPPED_LIFETIME: f64 = 20.0;
/// A hit from the victim's rear sector does x (1 + this) (behind_bonus).
pub const BEHIND_BONUS: f32 = 0.3;
/// The damage roll when a weapon's data gives none, and when its value
/// lies outside [0, 1] (damage_spread_default, damage_spread_fallback).
pub const DAMAGE_SPREAD_DEFAULT: f32 = 0.1;
pub const DAMAGE_SPREAD_FALLBACK: f32 = 0.2;
/// Total shown and used for infinite-ammo weapons (infinite_ammo).
pub const INFINITE_AMMO: u32 = 1000;

/// The classes (1), as `class_of` gives them; 0 is none.
pub const PRIMARY: u8 = 1;
pub const SECONDARY: u8 = 2;
pub const MELEE: u8 = 3;
pub const THROW: u8 = 4;
pub const SPECIAL: u8 = 5;
pub const OBJECTIVE: u8 = 6;
const CLASSES: usize = 7;

/// A dropped item's box when its body has none (ours: 10 units each way).
const ITEM_HALF: Vec3 = Vec3::splat(10.0 * UNIT);
/// Timers due on a tick fire on it (as the weapon frame's slack).
const SLACK: f64 = 1e-5;

/// The weapon data's hook for what this module reads of a weapon.
pub const ITEM_HOOK: &str = "combat_arms:inventory";
/// The weapon data's hook for a knife's attacks.
pub const KNIFE_HOOK: &str = "combat_arms:knife";

/// A weapon's class (1) from its data slot.
pub fn class_of(slot: u8) -> u8 {
    if slot < 6 { slot + 1 } else { 0 }
}

/// The rate an animation plays at for a summed rate `rate` (6.4: above
/// `DRAW_RATE_CAP` it plays at 1).
pub fn played_rate(rate: f32) -> f32 {
    if rate > DRAW_RATE_CAP { 1.0 } else { rate }
}

/// The damage roll's spread r for a weapon's data value (8.4).
pub fn damage_spread(value: Option<f32>) -> f32 {
    match value {
        None => DAMAGE_SPREAD_DEFAULT,
        Some(r) if (0.0..=1.0).contains(&r) => r,
        Some(_) => DAMAGE_SPREAD_FALLBACK,
    }
}

/// Whether a hit along `dir` (shooter towards victim) comes from the
/// rear sector of a victim facing `forward` with `right` its right
/// (8.4): theta = atan2(f.d, r.d), sector s = floor((pi + theta) / (pi/6)),
/// behind when 7 <= s <= 11 (a 150 degree sector, not symmetric).
pub fn behind(forward: Vec3, right: Vec3, dir: Vec3) -> bool {
    use std::f32::consts::PI;
    let theta = forward.dot(dir).atan2(right.dot(dir));
    let s = ((PI + theta) / (PI / 6.0)).floor() as i32;
    (7..=11).contains(&s)
}

// ---------------------------------------------------------------------------
// Parts

/// The server's "better weapon" rule after a pickup (11, step 6): a
/// switch to the picked weapon when no weapon is in hand, or when it
/// wasn't already carried and its command id (its place in the weapon
/// order lists, all classes in a row) is higher than the one in hand's.
/// Every pickup puts the weapon in hand away first, so in play it always
/// switches.
pub fn better_weapon(in_hand: Option<usize>, picked: usize, already_carried: bool) -> bool {
    match in_hand {
        None => true,
        Some(held) => !already_carried && picked > held,
    }
}

/// A thrown weapon that stays in the inventory at total 0 (10, "The
/// spent weapon stays"): Chimera's `grenade::KeepSpent` on every
/// throwable.
fn keep_spent(new: Query<Entity, (Added<Throwable>, Without<KeepSpent>)>, mut commands: Commands) {
    for e in &new {
        // A weapon given and taken back in one tick (a bot's kit before
        // the round's respawn) may be gone when this applies.
        commands.entity(e).try_insert(KeepSpent);
    }
}

/// The pickup fallback for a dead body's hit box the view ray passes
/// (11): its centre `distance` from the camera, less 35 units, within
/// reach. Weapon items never use it. (No body interactions yet.)
pub fn body_fallback(distance: f32) -> bool {
    distance - ACTIVATION_FALLBACK_SLACK <= ACTIVATION_DISTANCE + 1e-6
}

/// What this module reads of a weapon (`combat_arms:inventory` hook).
#[derive(Component, Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct CaItem {
    /// Unselectable at total 0, and auto-switched from when spent (1, 10).
    pub hide_when_empty: bool,
    /// The death drop takes it (12.4; a primary's data flag).
    pub drop_on_death: bool,
    /// Infinite ammo (knives): total `INFINITE_AMMO`.
    pub infinite: bool,
    /// The post-sprint animation, s (the stock table's sprint-out).
    pub sprint_out: f32,
    /// Summed select-rate bonuses (mods, model, sets; 6.4): the draw
    /// plays at `played_rate(1 + this)`.
    pub select_rate: f32,
    /// The weapon record's `FireAniRate`: added to the fire animation
    /// rate (view_models.md Corrections 6; weapon_firing.md r_fire).
    pub fire_rate: f32,
}

/// A weapon's draw at rate 1, s (its data's `draw`, a copy's own
/// `select` length): the draw time is this over the summed select rate
/// (`attachments::apply_draw`: the item's, the parts' and the gear's,
/// for every weapon, knives, grenades and specials included: inventory.md
/// Corrections (2026-10-30) 2).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct DrawStock(pub f32);

impl Default for CaItem {
    fn default() -> Self {
        Self {
            hide_when_empty: false,
            drop_on_death: true,
            infinite: false,
            sprint_out: 0.0,
            select_rate: 0.0,
            fire_rate: 0.0,
        }
    }
}

/// One knife attack as the data gives it (seconds, damage points).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttackData {
    prefire: f32,
    length: f32,
    hits: Vec<f32>,
    damage: f32,
    postfire: f32,
}

/// The `combat_arms:knife` hook's parameters (lengths in the document's
/// units, damage in its damage points).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct KnifeData {
    range: f32,
    #[serde(default)]
    damage_spread: Option<f32>,
    /// The weapon's fire-rate bonus (f_weapon, 8.2).
    #[serde(default)]
    fire_rate: f32,
    /// The attack ray's cone n (perturb units, thousandths): the knife's
    /// spread meters stay 0, so its `MinPerturb` (8.3).
    #[serde(default)]
    perturb: f32,
    primary: AttackData,
    #[serde(default)]
    secondary: Option<AttackData>,
}

/// One knife attack (8.2): seconds of animation time, damage as health.
#[derive(Clone, Debug, PartialEq)]
pub struct Attack {
    /// The model's pre-fire: kept (the timing hook carries it), never
    /// played (the melee class has none: Corrections (2026-10-30) 1).
    pub prefire: f32,
    pub length: f32,
    /// Hit events, seconds into the swing.
    pub hits: Vec<f32>,
    pub damage: f32,
    pub postfire: f32,
}

/// Where a knife attack is (8.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum KnifePhase {
    #[default]
    Idle,
    /// "Firing" set; the swing starts on the next update (no pre-fire).
    Armed,
    Swing,
    PostFire,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KnifeState {
    pub phase: KnifePhase,
    /// 0 primary, 1 secondary.
    pub attack: usize,
    /// When the phase began.
    pub since: f64,
    /// Hit events of this swing already made.
    pub hits: usize,
}

/// A character's own knife reach (m), over the knife's `range`: a bot's
/// 130 u (bots.md 8.4; the player's knife reaches 105 u).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[component(storage = "SparseSet")]
pub struct KnifeReach(pub f32);

/// A knife (`combat_arms:knife`): two attacks with timed hit events.
#[derive(Component, Clone, Debug)]
pub struct Knife {
    /// Metres.
    pub range: f32,
    /// The damage roll's r.
    pub spread: f32,
    /// The played fire rate (rho_fire after the cap).
    pub rate: f32,
    /// The attack ray's square spread, half-width (n / 1000).
    pub perturb: f32,
    /// Primary, then the secondary if any.
    pub attacks: Vec<Attack>,
    pub state: KnifeState,
}

impl Knife {
    /// Whether an attack animation is playing (not idle).
    pub fn busy(&self) -> bool {
        self.state.phase != KnifePhase::Idle
    }
}

/// A character's selection state (1, 3, 4) and its hands' (7).
#[derive(Component, Clone, Debug, Default)]
pub struct CaSelection {
    /// Per class: the order slot last used.
    pub cursor: [usize; CLASSES],
    /// Per class: the weapon last chosen (the auto-switch and the death
    /// drop use it).
    pub last_chosen: [Option<Entity>; CLASSES],
    /// The previous weapon (Q).
    pub previous: Option<Entity>,
    /// The weapon in hand as last seen, and whether it hides when empty.
    pub in_hand: Option<Entity>,
    in_hand_hides: bool,
    /// In the sprint pose (7), and when the post-sprint animation ends.
    pub sprint_pose: bool,
    pub sprint_out_until: f64,
    /// The dropped item the view is on (the HUD's pickup prompt; 11).
    pub pickup_target: Option<Entity>,
    prev_select: Option<u8>,
    prev_last: bool,
    prev_use: bool,
}

/// A weapon lying in the world (11, 12): its weapon entity (mods and the
/// rest stay on it), when it was dropped, and the ammo it carries.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Dropped {
    pub weapon: Entity,
    pub since: f64,
    pub total: u32,
    pub clip: u32,
}

/// A dropped weapon reserved for a looter (Opportunist): `owner` alone
/// until `until` (server time, s), then `owner`'s team only (gear_perks.md
/// Corrections 2026-10-22, 7).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CaReserved {
    pub owner: Entity,
    pub team: Option<u8>,
    pub until: f64,
}

/// Whether the dead drop their primary (12.3): off in snowball mode and
/// the VIP weapon race (the rules set it).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaDeathDrops(pub bool);

impl Default for CaDeathDrops {
    fn default() -> Self {
        Self(true)
    }
}

/// A weapon's total (the ammo model: the clip included).
pub fn total_of(item: Option<&CaItem>, mag: Option<&Magazine>, throw: Option<&Throwable>) -> u32 {
    if item.is_some_and(|i| i.infinite) {
        return INFINITE_AMMO;
    }
    match (mag, throw) {
        (Some(m), _) => m.clip + m.reserve,
        (None, Some(t)) => t.count,
        (None, None) => INFINITE_AMMO,
    }
}

// ---------------------------------------------------------------------------
// The plugin

/// Combat Arms' inventory: its hooks (registered before the data loads:
/// `CombatArmsPlugins` adds this before `weapons`), the starting weapons
/// unless the app set some first (a test's), and its systems.
pub struct CaInventoryPlugin;

impl Plugin for CaInventoryPlugin {
    fn build(&self, app: &mut App) {
        let set = app.world().get_resource::<StartingWeapons>().is_some_and(|s| !s.is_empty());
        if !set {
            app.insert_resource(StartingWeapons {
                team: Vec::new(),
                all: STARTING_WEAPONS.to_vec(),
            });
        }
        app.register_weapon_hook::<CaItem>(ITEM_HOOK, |e, d, p| {
            let mut item = p.clone();
            item.sprint_out = item.sprint_out.max(0.0);
            if let Some(mut w) = e.get_mut::<Weapon>() {
                w.draw_time = d.draw / played_rate(1.0 + item.select_rate);
            }
            e.insert((item, DrawStock(d.draw)));
            if !e.contains::<super::gear::CaGearRates>() {
                e.insert(super::gear::CaGearRates::default());
            }
        })
        .register_weapon_hook::<KnifeData>(KNIFE_HOOK, |e, d, p| {
            e.insert(knife(d, p));
        })
        .insert_resource(OwnSelection)
        .init_resource::<CaDeathDrops>()
        .add_systems(
            Predict::Select,
            (select.before(SelectWeapons), after_select.after(SelectWeapons)),
        )
        // The knife first: the hands see it idle on the update it ends.
        .add_systems(Predict::Weapons, (swing_knives, hands).chain().before(WeaponFrame))
        .add_systems(FixedUpdate, keep_spent.before(SimSet::Weapons))
        // Predicted with the player (a network client's own): the knife's
        // swing with the weapon (`weapon::sync`), the selection as its
        // own part with weapons as indices in the inventory
        // (`selection_codec`).
        .predicted_part::<Knife>()
        .predicted::<CaSelection>()
        .predicted_codec(std::any::type_name::<CaSelection>(), encode_selection, decode_selection)
        .add_systems(
            FixedUpdate,
            (
                pickups.before(SelectWeapons).before(SimSet::Movement),
                death_drops.after(core::apply_damage),
                expire_items.after(SimSet::Weapons),
            )
                .run_if(core::authoritative),
        );
    }
}

impl NetPart for Knife {
    /// Where its attack is (the attacks are the data's).
    type Net = KnifeState;
    fn net(&self) -> Self::Net {
        self.state
    }
    fn set_net(&mut self, net: Self::Net) {
        self.state = net;
    }
}

/// `CaSelection` over the wire: weapons as their index in the owner's
/// `Inventory::weapons` (entity ids differ between server and client).
/// The pickup target and the use key's edge are the server's (`pickups`
/// runs there only) and stay out.
#[derive(Serialize, Deserialize)]
struct NetSelection {
    cursor: [u8; CLASSES],
    last_chosen: [Option<u8>; CLASSES],
    previous: Option<u8>,
    in_hand: Option<u8>,
    in_hand_hides: bool,
    sprint_pose: bool,
    sprint_out_until: f64,
    prev_select: Option<u8>,
    prev_last: bool,
}

/// The selection's network form (`PredictedAppExt::predicted_codec`).
/// Decoded after the inventory (its name sorts after
/// `chimera_sim::weapon::Inventory`), so the indices meet the weapons the
/// client has rebuilt. A weapon no longer carried (the auto-switch's
/// `in_hand` after a spent grenade) goes as `GONE` and comes back as
/// `Entity::PLACEHOLDER`: still some weapon, not one carried.
/// `NetSelection`'s index for a weapon no longer carried.
const GONE: u8 = u8::MAX;

fn encode_selection(world: &World, owner: Entity) -> Option<Vec<u8>> {
    let sel = world.get::<CaSelection>(owner)?;
    let weapons = world.get::<Inventory>(owner).map(|i| i.weapons.as_slice()).unwrap_or_default();
    let index = |w: Option<Entity>| w.map(|w| weapons.iter().position(|x| *x == w).map_or(GONE, |i| i as u8));
    postcard::to_allocvec(&NetSelection {
        cursor: sel.cursor.map(|c| c.min(u8::MAX as usize) as u8),
        last_chosen: sel.last_chosen.map(index),
        previous: index(sel.previous),
        in_hand: index(sel.in_hand),
        in_hand_hides: sel.in_hand_hides,
        sprint_pose: sel.sprint_pose,
        sprint_out_until: sel.sprint_out_until,
        prev_select: sel.prev_select,
        prev_last: sel.prev_last,
    })
    .ok()
}

fn decode_selection(world: &mut World, owner: Entity, bytes: Option<&[u8]>) {
    let Some(net) = bytes.and_then(|b| postcard::from_bytes::<NetSelection>(b).ok()) else {
        world.entity_mut(owner).remove::<CaSelection>();
        return;
    };
    let weapons = world.get::<Inventory>(owner).map(|i| i.weapons.clone()).unwrap_or_default();
    let at = |i: Option<u8>| i.map(|i| weapons.get(i as usize).copied().unwrap_or(Entity::PLACEHOLDER));
    let kept = world.get::<CaSelection>(owner);
    let sel = CaSelection {
        cursor: net.cursor.map(|c| c as usize),
        last_chosen: net.last_chosen.map(at),
        previous: at(net.previous),
        in_hand: at(net.in_hand),
        in_hand_hides: net.in_hand_hides,
        sprint_pose: net.sprint_pose,
        sprint_out_until: net.sprint_out_until,
        pickup_target: kept.and_then(|k| k.pickup_target),
        prev_select: net.prev_select,
        prev_last: net.prev_last,
        prev_use: kept.is_some_and(|k| k.prev_use),
    };
    world.entity_mut(owner).insert(sel);
}

fn knife(d: &WeaponData, p: &KnifeData) -> Knife {
    let dmg = d.units.damage;
    let attack = |a: &AttackData| Attack {
        prefire: a.prefire,
        length: a.length,
        hits: a.hits.clone(),
        damage: a.damage * dmg,
        postfire: a.postfire,
    };
    Knife {
        range: p.range * d.units.length,
        spread: damage_spread(p.damage_spread),
        rate: played_rate(1.0 + p.fire_rate),
        perturb: p.perturb / 1000.0,
        attacks: std::iter::once(attack(&p.primary)).chain(p.secondary.iter().map(attack)).collect(),
        state: KnifeState::default(),
    }
}

type WeaponInfo<'a> = (
    &'a Weapon,
    Option<&'a CaItem>,
    Option<&'a Magazine>,
    Option<&'a Throwable>,
);

fn total(i: &WeaponInfo) -> u32 {
    total_of(i.1, i.2, i.3)
}

fn hides(i: &WeaponInfo) -> bool {
    i.1.is_some_and(|c| c.hide_when_empty)
}

/// A class's weapons in order (its order slots).
fn order(inv: &Inventory, class: u8, weapons: &Query<WeaponInfo>) -> Vec<Entity> {
    inv.weapons
        .iter()
        .copied()
        .filter(|w| weapons.get(*w).is_ok_and(|i| class_of(i.0.slot) == class))
        .collect()
}

// ---------------------------------------------------------------------------
// Selection (3, 4, 10)

/// Class keys, Q and the auto-switch, before Chimera's selection applies
/// `Inventory::wanted`; a new character draws its lowest class (1).
fn select(
    mut owners: Query<
        (Entity, &Intent, &mut Inventory, Option<&mut CaSelection>, Option<&Health>, Has<SlotLastInv>),
        Without<NetDrawn>,
    >,
    weapons: Query<WeaponInfo>,
    mut commands: Commands,
) {
    for (owner, intent, mut inv, sel, health, by_slot) in &mut owners {
        let Some(mut sel) = sel else {
            let lowest = inv
                .weapons
                .iter()
                .copied()
                .filter_map(|w| weapons.get(w).ok().map(|i| (w, class_of(i.0.slot))))
                .filter(|(_, c)| *c != 0)
                .min_by_key(|(_, c)| *c)
                .map(|(w, _)| w);
            if lowest.is_some() {
                inv.wanted = lowest;
            }
            commands.entity(owner).insert(CaSelection {
                prev_select: intent.select,
                prev_last: intent.last_weapon,
                prev_use: intent.use_key,
                ..default()
            });
            continue;
        };
        let alive = health.is_none_or(|h| h.current > 0.0);

        // 10. A spent hide-when-empty weapon in hand (or one gone with its
        // last round) gives the last chosen primary, secondary or melee.
        let gone = sel.in_hand.is_some_and(|w| !inv.weapons.contains(&w)) && sel.in_hand_hides;
        let spent = inv.active.and_then(|a| weapons.get(a).ok()).is_some_and(|i| {
            hides(&i) && total(&i) == 0 && !i.3.is_some_and(|t| t.primed() || t.redraw_at.is_some())
        });
        if alive && (gone || spent) {
            sel.previous = None;
            sel.in_hand_hides = false;
            let fallback = [PRIMARY, SECONDARY, MELEE].into_iter().find_map(|c| {
                sel.last_chosen[c as usize].filter(|w| inv.weapons.contains(w) && inv.active != Some(*w))
            });
            if let Some(w) = fallback {
                inv.wanted = Some(w);
            }
        }

        // 3. Class keys, on the press.
        let pressed = intent.select != sel.prev_select;
        sel.prev_select = intent.select;
        if alive
            && pressed
            && let Some(select) = intent.select
        {
            let (slot, nth) = Intent::picked(select);
            if let Some(next) = class_key(&sel, &inv, class_of(slot), nth, &weapons)
                && inv.active != Some(next)
            {
                sel.previous = inv.active;
                inv.wanted = Some(next);
            }
        }

        // 4. Q, on the press.
        let q = intent.last_weapon && !sel.prev_last;
        sel.prev_last = intent.last_weapon;
        if alive
            && q
            && let Some(prev) = if by_slot {
                // A character that follows its weapon (the gun game): the
                // most recently drawn other slot, whatever was swapped.
                inv.slot_target(|w| weapons.get(w).ok().map(|i| i.0.slot))
            } else {
                sel.previous
            }
            && inv.active != Some(prev)
        {
            let target = weapons.get(prev).ok().filter(|_| inv.weapons.contains(&prev));
            sel.previous = inv.active;
            if let Some(i) = target
                && class_of(i.0.slot) != 0
                && !(total(&i) == 0 && hides(&i))
            {
                inv.wanted = Some(prev);
            }
        }
    }
}

/// The weapon class key `class` picks (3): the next order slot when one
/// of its weapons is in hand (wrapping), else the cursor's; classes above
/// melee skip a hide-when-empty weapon at total 0 and search on for one
/// with rounds. `nth`: a pick of that order slot (the HUD's selection).
fn class_key(sel: &CaSelection, inv: &Inventory, class: u8, nth: Option<u8>, weapons: &Query<WeaponInfo>) -> Option<Entity> {
    if class == 0 || class as usize >= CLASSES {
        return None;
    }
    let list = order(inv, class, weapons);
    if list.is_empty() {
        return None;
    }
    let mut i = sel.cursor[class as usize];
    if class >= THROW && i >= list.len() {
        // Nothing left at the cursor (2).
        return None;
    }
    if let Some(n) = nth {
        i = (n as usize).min(list.len() - 1);
    } else if inv.active.is_some_and(|a| list.contains(&a)) {
        i += 1;
        if i >= list.len() {
            i = 0;
        }
    } else if i >= list.len() {
        i = 0;
    }
    if class <= MELEE {
        return Some(list[i]);
    }
    let info = |w: Entity| weapons.get(w).ok();
    for k in 0..list.len() {
        let w = list[(i + k) % list.len()];
        let Some(x) = info(w) else { continue };
        let usable = if k == 0 {
            !(total(&x) == 0 && hides(&x))
        } else {
            total(&x) > 0
        };
        if usable {
            return Some(w);
        }
    }
    None
}

/// After a switch: the destination's class cursor and last chosen (1),
/// and a magazine weapon drawn empty with spare rounds reloads at once
/// instead of drawing (6.6).
#[allow(clippy::type_complexity)]
fn after_select(
    mut owners: Query<(Entity, &mut Inventory, &mut CaSelection), Without<NetDrawn>>,
    slots: Query<&Weapon>,
    mut weapons: Query<(&Weapon, Option<&CaItem>, Option<&Magazine>, &mut WeaponState, Has<ShellReload>)>,
    mut events: MessageWriter<WeaponEvent>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
) {
    for (owner, mut inv, mut sel) in &mut owners {
        if inv.active == sel.in_hand {
            continue;
        }
        sel.in_hand = inv.active;
        sel.in_hand_hides = false;
        sel.sprint_pose = false;
        let Some(active) = inv.active else { continue };
        let Ok(slot) = slots.get(active).map(|w| w.slot) else { continue };
        let class = class_of(slot);
        let index = inv
            .weapons
            .iter()
            .filter(|w| slots.get(**w).is_ok_and(|x| class_of(x.slot) == class))
            .position(|w| *w == active);
        if class != 0 {
            sel.cursor[class as usize] = index.unwrap_or(0);
            sel.last_chosen[class as usize] = Some(active);
        }
        let Ok((_, item, mag, mut st, shells)) = weapons.get_mut(active) else { continue };
        sel.in_hand_hides = item.is_some_and(|i| i.hide_when_empty);
        if let Some(mag) = mag
            && mag.clip == 0
            && !shells
            && start_reload(&mut inv, &mut st, mag, clock.now)
        {
            events.write(WeaponEvent {
                owner,
                weapon: active,
                kind: WeaponEventKind::ReloadStarted,
                replay: !first.0,
            });
        }
    }
}

/// Start a magazine reload now (the frame's rule: refused when no round
/// would move); its end holds every attack.
pub(super) fn start_reload(inv: &mut Inventory, st: &mut WeaponState, mag: &Magazine, now: f64) -> bool {
    if (mag.size - mag.clip).min(mag.reserve) == 0 {
        return false;
    }
    let end = now + mag.reload_time as f64;
    inv.next_attack = end;
    st.next_primary = end;
    st.next_secondary = end;
    st.reload_end = Some(end);
    st.fired_on_empty = false;
    st.burst_left = 0;
    true
}

// ---------------------------------------------------------------------------
// The hands: draw, reload, sprint and use (7)

/// Before the weapon frame: R during a magazine weapon's draw reloads at
/// once (Quirk 2); the sprint pose and the post-sprint animation hold
/// fire, as holding use does; reloads start and complete under them.
#[allow(clippy::type_complexity)]
pub(super) fn hands(
    mut owners: Query<
        (Entity, &Intent, &mut Inventory, &mut CaSelection, &MovementState, Option<&Health>),
        Without<NetDrawn>,
    >,
    mut weapons: Query<(
        Option<&CaItem>,
        Option<&mut Magazine>,
        &mut WeaponState,
        Has<ShellReload>,
        Option<&Knife>,
    )>,
    mut events: MessageWriter<WeaponEvent>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
) {
    let now = clock.now;
    let due = now + SLACK;
    let dt = clock.dt() as f64;
    for (owner, intent, mut inv, mut sel, state, health) in &mut owners {
        if health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let Some(active) = inv.active else { continue };
        let Ok((item, mut mag, mut st, shells, knife)) = weapons.get_mut(active) else {
            continue;
        };
        let mut event = |kind| {
            events.write(WeaponEvent {
                owner,
                weapon: active,
                kind,
                replay: !first.0,
            })
        };
        // A reload due under sprint completes here (it plays and
        // completes under sprint; then the sprint pose).
        if state.sprinting
            && let Some(end) = st.reload_end
            && due >= end
            && let Some(m) = mag.as_mut()
        {
            let n = (m.size - m.clip).min(m.reserve);
            m.clip += n;
            m.reserve -= n;
            st.reload_end = None;
            st.next_primary = now;
            st.next_secondary = now;
            event(WeaponEventKind::Reloaded);
        }
        let reloading = st.reloading();
        let drawing = due < inv.next_attack && !reloading;
        let firing = match knife {
            Some(k) => k.busy(),
            None => due < st.next_primary,
        };
        if state.sprinting && !sel.sprint_pose && !drawing && !reloading && !firing {
            sel.sprint_pose = true;
        }
        if !state.sprinting && sel.sprint_pose {
            sel.sprint_pose = false;
            sel.sprint_out_until = now + item.map_or(0.0, |i| i.sprint_out) as f64;
        }
        let Some(m) = mag else { continue };
        // The update that sees the post-sprint pose end only returns to
        // idle (Corrections 1): fire waits one update more, then the
        // weapon's own chain (`Trigger::latency`).
        let blocked_until = if sel.sprint_pose || intent.use_key {
            now + dt
        } else if sel.sprint_out_until > 0.0 {
            sel.sprint_out_until + dt
        } else {
            0.0
        };
        let held = blocked_until > due;
        if intent.reload && !reloading && !shells && (drawing || held) && start_reload(&mut inv, &mut st, &m, now) {
            event(WeaponEventKind::ReloadStarted);
        }
        if held && !st.reloading() {
            st.next_primary = st.next_primary.max(blocked_until);
            st.next_secondary = st.next_secondary.max(blocked_until);
        }
        // Sprint and use end the firing state (a held trigger starts the
        // chain again after them).
        if held && (st.firing || st.armed_round || st.postfire_end.is_some()) {
            st.firing = false;
            st.arming = 0;
            st.armed_round = false;
            st.postfire_end = None;
        }

    }
}

// ---------------------------------------------------------------------------
// The knife (8)

/// Each knife in hand steps its attack (8.2) and makes its hit events
/// (8.3, 8.4). A knife put away loses what was left of its swing.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn swing_knives(
    owners: Query<
        (&Intent, &Inventory, &CaSelection, &Transform, &MovementState, Option<&Health>, Option<&KnifeReach>),
        Without<NetDrawn>,
    >,
    mut knives: Query<(Entity, &Weapon, &mut Knife)>,
    targets: Query<(Option<&Intent>, Has<Health>, Option<&Inventory>)>,
    breakables: Query<(), (With<core::Damageable>, Without<Health>)>,
    spreads: Query<&super::weapons::CaSpread>,
    ai: super::weapons::AiHits,
    bodies: Query<(
        &Transform,
        &Intent,
        &MovementState,
        Option<&ColliderAabb>,
        Option<&HitSpheres>,
        (Option<&crate::slots::GameIdentity>, Option<&core::Hitboxes>),
    )>,
    mut armour: Query<(&mut crate::weapon::Armor, Option<&crate::weapon::ArmorPieces>)>,
    infected: Query<(), With<CaInfected>>,
    colliders: Query<&ColliderOf>,
    (spatial, mut lag): (SpatialQuery, LagComp),
    mut damage: MessageWriter<Damage>,
    mut events: MessageWriter<WeaponEvent>,
    mut regions: Option<ResMut<Messages<core::RegionHit>>>,
    (clock, first, role): (Res<SimClock>, Res<FirstTimePredicted>, Option<Res<NetRole>>),
) {
    let now = clock.now;
    let due = now + SLACK;
    let server = role.as_deref().is_none_or(|r| *r != NetRole::Client);
    for (entity, weapon, mut knife) in &mut knives {
        let Some(owner) = weapon.owner else { continue };
        let Ok((intent, inv, sel, at, state, health, reach)) = owners.get(owner) else {
            continue;
        };
        let range = reach.map_or(knife.range, |r| r.0);
        if inv.active != Some(entity) || health.is_some_and(|h| h.current <= 0.0) {
            knife.state = KnifeState::default();
            continue;
        }
        let ready = due >= inv.next_attack && !sel.sprint_pose && due >= sel.sprint_out_until && !state.sprinting;
        // 4.1: fire (not with use held) beats alt-fire.
        let request = if !ready {
            None
        } else if intent.fire && !intent.use_key {
            Some(0)
        } else if intent.secondary && knife.attacks.len() > 1 {
            Some(1)
        } else {
            None
        };
        // Animation time is exact, observed at updates: each animation
        // starts where the last one ended (Units and conventions).
        let rate = knife.rate.max(1e-3) as f64;
        let elapsed = |since: f64| (now - since) * rate + SLACK;
        let end_of = |since: f64, length: f32| since + length as f64 / rate;
        let st = knife.state;
        match st.phase {
            KnifePhase::Idle => {
                if let Some(a) = request {
                    knife.state = KnifeState {
                        phase: KnifePhase::Armed,
                        attack: a,
                        since: now,
                        hits: 0,
                    };
                }
            }
            KnifePhase::Armed => {
                // The melee class plays no pre-fire: the update after the
                // one that set "firing" starts the swing (inventory.md
                // Corrections (2026-10-30) 1).
                knife.state.phase = KnifePhase::Swing;
                knife.state.since = now;
                knife.state.hits = 0;
            }
            KnifePhase::Swing => {
                let attack = knife.attacks[st.attack].clone();
                let t = elapsed(st.since);
                while knife.state.hits < attack.hits.len() && attack.hits[knife.state.hits] as f64 <= t {
                    let n = knife.state.hits;
                    knife.state.hits += 1;
                    // Keys passed in the same advance raise one "fire now":
                    // one hit (special_weapons.md 5.1, its case 46).
                    while knife.state.hits < attack.hits.len() && attack.hits[knife.state.hits] as f64 <= t {
                        knife.state.hits += 1;
                    }
                    let eye = at.translation + state.eye_offset;
                    // The gun's square spread at the knife's cone (8.3).
                    let mut spread = shared_random(intent.command, "combat_arms:knife_spread", n as i32);
                    let (u1, u2) = (spread.next(), spread.next());
                    let dir = square_dir(crate::weapon::aim_of(intent.yaw, intent.pitch), knife.perturb, u1, u2);
                    let Ok(ray) = Dir3::new(dir) else { continue };
                    // A network server traces a remote player's swing
                    // against the others where its client drew them (lag
                    // compensation, as its rounds: `weapon::lagcomp`; 8.3,
                    // "the server re-checks the target"): their boxes then,
                    // the world and the rest as they are.
                    let rewound = if server && first.0 { lag.rewind(owner, true, None) } else { Vec::new() };
                    let filter = SpatialQueryFilter::from_excluded_entities(
                        std::iter::once(owner).chain(rewound.iter().map(|r| r.entity)),
                    )
                    .with_mask(core::SOLID_LAYERS.0 & !core::ITEM_LAYER.0);
                    // (distance, normal, collider, body)
                    let mut hit = spatial
                        .cast_ray(eye, ray, range, true, &filter)
                        .map(|h| (h.distance, h.normal, h.entity, colliders.get(h.entity).map_or(h.entity, |c| c.body)));
                    for r in &rewound {
                        if let Some(t) = r.box_entry(eye, dir, hit.map_or(range, |h| h.0), 0.0) {
                            hit = Some((t, -dir, r.entity, r.entity));
                        }
                    }
                    let end = hit.map_or(eye + dir * range, |h| eye + dir * h.0);
                    let body = hit.map(|h| h.3);
                    let back = rewound.iter().find(|r| body == Some(r.entity));
                    events.write(WeaponEvent {
                        owner,
                        weapon: entity,
                        kind: WeaponEventKind::Swing {
                            hit: hit.is_some(),
                            secondary: st.attack == 1,
                            at: hit.map(|h| (end, h.1, h.3)),
                            line: (eye, end),
                        },
                        replay: !first.0,
                    });
                    let Some(target) = body else { continue };
                    let mut roll = shared_random(intent.command, "combat_arms:knife", n as i32);
                    let u = roll.range(1.0 - knife.spread, 1.0 + knife.spread);
                    // A map's breakable (a window, a vent): the swing's
                    // damage at the hit, as any knife's on those brushes.
                    let hit_entity = hit.map_or(target, |h| h.2);
                    let thing = [target, hit_entity].into_iter().find(|e| breakables.contains(*e));
                    if let Some(thing) = thing {
                        if server && first.0 {
                            damage.write(Damage {
                                target: thing,
                                attacker: Some(owner),
                                amount: attack.damage * u,
                                point: end,
                                dir,
                                hitgroup: Hitgroup::Generic,
                                kind: DamageKind::Melee,
                                weapon: None,
                                force: Vec3::ZERO,
                            });
                        }
                        continue;
                    }
                    let Ok((t_intent, alive, t_inv)) = targets.get(target) else { continue };
                    if !alive || !server || !first.0 {
                        continue;
                    }
                    // The rear sector from the victim's body, not its view
                    // (Corrections 2).
                    let rear = t_intent.is_some_and(|i| {
                        let kept = t_inv.and_then(|v| v.active).and_then(|w| spreads.get(w).ok()).and_then(|s| s.body_yaw);
                        // Rewound: its look then, twisted as now.
                        let body = super::weapons::body_yaw(i.yaw, kept);
                        let yaw = Quat::from_rotation_y(back.map_or(body, |r| r.yaw + (body - i.yaw)));
                        behind(yaw * Vec3::NEG_Z, yaw * Vec3::X, dir)
                    });
                    let b = if rear { 1.0 + BEHIND_BONUS } else { 1.0 };
                    // The body part the ray passes on through the target
                    // (8.4: melee uses the zones), then the armour step
                    // (no stock gear covers melee).
                    // Another game's body: its own shapes, by the same
                    // rule (docs/DESIGN.md "Hits across games"), not our
                    // skeleton's spheres hidden inside it.
                    // A rewound body: its shapes where it was then.
                    let through = end + dir * range;
                    let theirs = bodies.get(target).ok().and_then(|(t, i, _, aabb, _, (identity, boxes))| {
                        let shapes = match back {
                            Some(r) if !crate::slots::GameIdentity::allows(identity, super::GAME) => {
                                r.hitboxes.as_ref().map(|h| body_shapes(&h.get().0, r.frame().0, Quat::from_rotation_y(r.yaw)))
                            }
                            Some(_) => None,
                            None => super::weapons::their_shapes(t, i, aabb, identity, boxes),
                        }?;
                        Some(shapes.by_power(eye, through).map(|k| shapes.0[k]))
                    });
                    let sphere = match theirs {
                        Some(_) => None,
                        None => bodies.get(target).ok().and_then(|(t, i, s, aabb, spheres, _)| {
                            let data = spheres.map_or(*MALE, |h| h.0);
                            let world = match back {
                                Some(r) => {
                                    let b = r.aabb();
                                    geometry_at(r.origin, Quat::from_rotation_y(r.yaw), b.min, b.max, data, -i.pitch).2
                                }
                                None => body_geometry(t, i, s, aabb, data)?.2,
                            };
                            pick_sphere(&world, eye, through).map(|k| data[k])
                        }),
                    };
                    let shape = theirs.flatten();
                    let (location, zone) = match (shape, sphere) {
                        (Some(h), _) => (
                            super::weapons::location_of(h.region),
                            h.scale.unwrap_or_else(|| super::weapons::zone_factor(h.region)),
                        ),
                        (None, Some(s)) => (s.location, s.factor),
                        (None, None) => (Location::Torso, 1.0),
                    };
                    let hit = ArmourHit {
                        damage_type: MELEE_DAMAGE,
                        location,
                        zone,
                        infected: infected.contains(target),
                    };
                    // An AI's knife x `AIDamageFactor` (weapon_firing.md
                    // Corrections 7; melee: no cap).
                    let points = attack.damage * u * b / HP * ai.factor(Some(owner));
                    let amount = match armour.get_mut(target) {
                        Ok((a, pieces)) => armour_and_zone(points, hit, Some(a.into_inner()), pieces, true),
                        Err(_) => armour_and_zone(points, hit, None, None, true),
                    } * HP;

                    let side = |l, r| if sphere.is_some_and(|s| s.at.x < 0.0) { l } else { r };
                    let hitgroup = match sphere.map(|s| s.location) {
                        _ if shape.is_some() => shape.map_or(Hitgroup::Generic, |h| h.region.hitgroup()),
                        None => Hitgroup::Generic,
                        Some(Location::Head) => Hitgroup::Head,
                        Some(Location::Torso) => Hitgroup::Chest,
                        Some(Location::Groin) => Hitgroup::Stomach,
                        Some(Location::Arm) => side(Hitgroup::LeftArm, Hitgroup::RightArm),
                        Some(Location::Leg) => side(Hitgroup::LeftLeg, Hitgroup::RightLeg),
                    };
                    damage.write(Damage {
                        target,
                        attacker: Some(owner),
                        amount,
                        point: end,
                        dir,
                        hitgroup,
                        kind: DamageKind::Melee,
                        weapon: None,
                        force: Vec3::ZERO,
                    });
                    // The full region beside the damage (a groin reads as
                    // the stomach there): the announcer's nutshot.
                    if let Some(regions) = regions.as_mut() {
                        let left = sphere.is_some_and(|s| s.at.x < 0.0);
                        regions.write(core::RegionHit {
                            target,
                            region: match sphere.map(|s| s.location) {
                                _ if shape.is_some() => shape.map_or(core::BodyRegion::Generic, |h| h.region),
                                None => core::BodyRegion::Generic,
                                Some(Location::Head) => core::BodyRegion::Head,
                                Some(Location::Torso) => core::BodyRegion::Chest,
                                Some(Location::Groin) => core::BodyRegion::Groin,
                                Some(Location::Arm) => if left { core::BodyRegion::LeftUpperArm } else { core::BodyRegion::RightUpperArm },
                                Some(Location::Leg) => if left { core::BodyRegion::LeftThigh } else { core::BodyRegion::RightThigh },
                            },
                        });
                    }
                }
                if t >= attack.length as f64 {
                    knife.state = match request {
                        // Held: the next swing at once, no pre-fire (8.1).
                        Some(a) => KnifeState {
                            phase: KnifePhase::Swing,
                            attack: a,
                            since: end_of(st.since, attack.length),
                            hits: 0,
                        },
                        None => KnifeState {
                            phase: KnifePhase::PostFire,
                            attack: st.attack,
                            since: end_of(st.since, attack.length),
                            hits: 0,
                        },
                    };
                }
            }
            KnifePhase::PostFire => {
                if elapsed(st.since) >= knife.attacks[st.attack].postfire as f64 {
                    knife.state = KnifeState::default();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pickup (11), drop (12) and respawn (13)

/// Where a ray from `origin` along `dir` enters the box `lo..hi`, if it
/// does (0 when it starts inside).
pub fn ray_box(origin: Vec3, dir: Vec3, lo: Vec3, hi: Vec3) -> Option<f32> {
    let inv = dir.recip();
    let a = (lo - origin) * inv;
    let b = (hi - origin) * inv;
    let near = a.min(b).max_element().max(0.0);
    let far = a.max(b).min_element();
    (far >= near).then_some(near)
}

/// The pickup target of every living character (11: the view ray within
/// `ACTIVATION_DISTANCE`, or the fallback), and E on it takes it.
#[allow(clippy::type_complexity)]
fn pickups(
    mut users: Query<(Entity, &Transform, &Intent, &MovementState, Option<&Health>, &mut CaSelection), Without<NetDrawn>>,
    items: Query<(Entity, &Transform, Option<&PhysicsProp>), With<Dropped>>,
    spatial: SpatialQuery,
    off: Option<Res<chimera_sim::weapon::drop::NoWeaponPickups>>,
    mut commands: Commands,
) {
    for (owner, at, intent, state, health, mut sel) in &mut users {
        let pressed = intent.use_key && !sel.prev_use;
        sel.prev_use = intent.use_key;
        if off.is_some() || health.is_some_and(|h| h.current <= 0.0) {
            sel.pickup_target = None;
            continue;
        }
        let eye = at.translation + state.eye_offset;
        let dir = crate::weapon::aim_of(intent.yaw, intent.pitch) * Vec3::NEG_Z;
        let best = items
            .iter()
            .filter_map(|(e, t, body)| {
                let half = body.map_or(ITEM_HALF, |b| (b.bounds.1 - b.bounds.0) / 2.0);
                let hit = ray_box(eye, dir, t.translation - half, t.translation + half)?;
                // Only a real hit within reach (the fallback is for dead
                // bodies, not items: Open question 13).
                (hit <= ACTIVATION_DISTANCE + 1e-6).then_some((hit, e))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        // Something solid in front hides it.
        let target = best.and_then(|(hit, e)| {
            let filter = SpatialQueryFilter::from_excluded_entities([owner])
                .with_mask(core::SOLID_LAYERS.0 & !core::ITEM_LAYER.0);
            let ray = Dir3::new(dir).ok()?;
            match spatial.cast_ray(eye, ray, hit, true, &filter) {
                Some(h) if h.distance < hit => None,
                _ => Some(e),
            }
        });
        if sel.pickup_target != target {
            sel.pickup_target = target;
        }
        if pressed && let Some(item) = target {
            commands.queue(move |w: &mut World| {
                pick_up(w, owner, item);
            });
        }
    }
}

/// A character that can't take weapons from the ground (VIP Escort's VIP,
/// game_modes_2.md M13.4).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoPickups;

/// `owner` takes the dropped `item` (11). Returns whether it did.
pub fn pick_up(world: &mut World, owner: Entity, item: Entity) -> bool {
    if world.get::<NoPickups>(owner).is_some() || world.contains_resource::<chimera_sim::weapon::drop::NoWeaponPickups>() {
        return false;
    }
    let Some(d) = world.get::<Dropped>(item).copied() else { return false };
    if world.get::<Health>(owner).is_none_or(|h| h.current <= 0.0) {
        return false;
    }
    if let Some(r) = world.get::<CaReserved>(item).copied() {
        let now = world.get_resource::<SimClock>().map_or(0.0, |c| c.now);
        let mine = world.get::<core::Team>(owner).map(|t| t.0);
        let allowed = owner == r.owner || (now >= r.until && r.team.is_some() && mine == r.team);
        if !allowed {
            return false;
        }
    }
    let Some((id, class)) = world.get::<Weapon>(d.weapon).map(|w| (w.id, class_of(w.slot))) else {
        return false;
    };
    let Some(inv) = world.get::<Inventory>(owner).cloned() else { return false };
    let Some(sel) = world.get::<CaSelection>(owner).cloned() else { return false };
    let in_class: Vec<Entity> = inv
        .weapons
        .iter()
        .copied()
        .filter(|w| world.get::<Weapon>(*w).is_some_and(|x| class_of(x.slot) == class))
        .collect();
    // A non-primary already carried: its ammo, and the clip refilled.
    if class != PRIMARY
        && let Some(mine) = in_class.iter().copied().find(|w| world.get::<Weapon>(*w).is_some_and(|x| x.id == id))
    {
        if let Some(mut m) = world.get_mut::<Magazine>(mine) {
            let total = (m.clip + m.reserve + d.total).min(m.size + m.reserve_max);
            m.clip = total.min(m.size);
            m.reserve = total - m.clip;
        } else if let Some(mut t) = world.get_mut::<Throwable>(mine) {
            t.count = (t.count + d.total).min(t.max);
        }
        despawn(world, item);
        despawn(world, d.weapon);
        // The server put the weapon in hand away for the request, so the
        // "better weapon" rule switches to this one (Open question 6).
        if better_weapon(None, 0, true)
            && let Some(mut inv) = world.get_mut::<Inventory>(owner)
            && inv.active != Some(mine)
        {
            inv.wanted = Some(mine);
        }
        return true;
    }
    // A swap within the class: the last chosen one goes.
    // Without one (none switched to since spawning) the request does
    // nothing (Corrections 2026-10-22, 2); class 0 keeps the old fallbacks.
    let replaced = if class != 0 {
        let Some(last) = sel.last_chosen[class as usize].filter(|w| in_class.contains(w)) else {
            return false;
        };
        Some(last)
    } else {
        in_class.get(sel.cursor[0]).copied().or_else(|| in_class.first().copied())
    };
    // A primary whose fitted set and sight style equal the carried
    // weapon's is no swap: the carried weapon stays and takes the item's
    // ammunition (Corrections 2026-10-22, 1).
    if class == PRIMARY
        && let Some(mine) = replaced
        && same_fitted_set(world, mine, d.weapon)
    {
        if let Some(mut m) = world.get_mut::<Magazine>(mine) {
            let total = d.total.min(m.size + m.reserve_max);
            m.clip = if total == 0 { 0 } else { d.clip.min(total).min(m.size) };
            m.reserve = total - m.clip;
        } else if let Some(mut t) = world.get_mut::<Throwable>(mine) {
            t.count = d.total.min(t.max);
        }
        despawn(world, item);
        despawn(world, d.weapon);
        world.write_message(crate::weapon::drop::PickedUp { who: owner, weapon: mine });
        return true;
    }
    if replaced.is_none() && inv.weapons.len() >= MAX_CARRIED {
        return false;
    }
    let index = replaced.and_then(|r| drop_item(world, owner, r, true));
    despawn(world, item);
    // Its ammo: the item's, clamped; the clip with it, none at total 0.
    if let Some(mut m) = world.get_mut::<Magazine>(d.weapon) {
        let total = d.total.min(m.size + m.reserve_max);
        m.clip = if total == 0 { 0 } else { d.clip.min(total).min(m.size) };
        m.reserve = total - m.clip;
    } else if let Some(mut t) = world.get_mut::<Throwable>(d.weapon) {
        t.count = d.total.min(t.max);
    }
    if let Some(mut w) = world.get_mut::<Weapon>(d.weapon) {
        w.owner = Some(owner);
    }
    if let Some(mut st) = world.get_mut::<WeaponState>(d.weapon) {
        *st = WeaponState::default();
    }
    let Some(mut inv) = world.get_mut::<Inventory>(owner) else { return false };
    match index {
        Some(i) => {
            let i = i.min(inv.weapons.len());
            inv.weapons.insert(i, d.weapon);
        }
        None => inv.weapons.push(d.weapon),
    }
    // The "better weapon" rule (no weapon in hand on the server), drawn at
    // once (the auto weapon switch option's default).
    if better_weapon(None, 0, false) {
        inv.wanted = Some(d.weapon);
    }
    world.write_message(crate::weapon::drop::PickedUp {
        who: owner,
        weapon: d.weapon,
    });
    true
}

/// The five part slots of `weapon` with an empty slot filled by its
/// weapon's default part (Corrections 2026-10-22, 1).
fn effective_parts(world: &World, weapon: Entity) -> [Option<u16>; super::attachments::SLOTS] {
    let mut out = world.get::<super::attachments::CaFitted>(weapon).map_or([None; super::attachments::SLOTS], |f| f.slots);
    if let (Some(slots), Some(table)) =
        (world.get::<super::attachments::CaAttachSlots>(weapon), world.get_resource::<super::attachments::CaModTable>())
    {
        for (s, part) in out.iter_mut().enumerate() {
            if part.is_none()
                && let Some(d) = slots.params.defaults.get(s)
            {
                *part = table.find(d);
            }
        }
    }
    out
}

/// Whether two weapons carry the same part in every slot and the same
/// sight style; the weapons themselves are not compared (a faithful quirk:
/// a bare AK-47 item "matches" a bare M16A3).
fn same_fitted_set(world: &World, a: Entity, b: Entity) -> bool {
    let (pa, pb) = (effective_parts(world, a), effective_parts(world, b));
    if pa != pb {
        return false;
    }
    let style = |w: Entity| {
        let table = world.get_resource::<super::attachments::CaModTable>()?;
        let id = pa_sight(world, w)?;
        table.get(id).map(|r| r.sight_style)
    };
    style(a) == style(b)
}

fn pa_sight(world: &World, w: Entity) -> Option<u16> {
    effective_parts(world, w)[super::attachments::SIGHT]
}

fn despawn(world: &mut World, e: Entity) {
    if let Ok(e) = world.get_entity_mut(e) {
        e.despawn();
    }
}

/// Take `weapon` out of `owner`'s list and lay it at the left foot with a
/// random yaw (0 to 1 rad), jittered by u x the hull's half-extents on X
/// and Z (u in [-0.5, 0.5]) for a swap (11, 12). Without a skeleton the
/// left foot is the hull's bottom centre (Open question 17). Returns its
/// equip index.
pub fn drop_item(world: &mut World, owner: Entity, weapon: Entity, jitter: bool) -> Option<usize> {
    let now = world.get_resource::<SimClock>().map_or(0.0, |c| c.now);
    let at = *world.get::<Transform>(owner)?;
    let intent = world.get::<Intent>(owner).cloned().unwrap_or_default();
    let (lo, hi) = world
        .get::<MovementState>(owner)
        .filter(|s| s.hull_max != s.hull_min)
        .map_or((Vec3::new(-0.4, -0.9, -0.4), Vec3::new(0.4, 0.9, 0.4)), |s| (s.hull_min, s.hull_max));
    let index = {
        let mut inv = world.get_mut::<Inventory>(owner)?;
        let i = inv.weapons.iter().position(|w| *w == weapon)?;
        inv.weapons.remove(i);
        if inv.active == Some(weapon) {
            inv.active = None;
        }
        if inv.wanted == Some(weapon) {
            inv.wanted = None;
        }
        i
    };
    // Put away: whatever it was doing is lost.
    if let Some(mut st) = world.get_mut::<WeaponState>(weapon) {
        *st = WeaponState::default();
    }
    let id = {
        let mut w = world.get_mut::<Weapon>(weapon)?;
        w.owner = None;
        w.id
    };
    let (total, clip) = match (world.get::<Magazine>(weapon), world.get::<Throwable>(weapon)) {
        (Some(m), _) => (m.clip + m.reserve, m.clip),
        (None, Some(t)) => (t.count, 0),
        _ => (INFINITE_AMMO, 0),
    };
    let mut rng: Rng = shared_random(intent.command, "combat_arms:drop", index as i32);
    let half = (hi - lo) / 2.0;
    // The left foot's node (characters.md 3), else the hull's bottom
    // centre.
    let foot = world.get::<super::characters::CaPose>(owner).and_then(|p| {
        let state = world.get::<MovementState>(owner)?;
        Some(p.world(&at, state, UNIT, p.left_foot?))
    });
    let mut spot = foot.unwrap_or(at.translation + Vec3::Y * lo.y);
    if jitter {
        spot.x += rng.range(-0.5, 0.5) * half.x;
        spot.z += rng.range(-0.5, 0.5) * half.z;
    }
    let yaw = rng.range(0.0, 1.0);
    world.spawn((
        Name::new(format!("Dropped {id}")),
        LooseItem(id.to_string()),
        Dropped {
            weapon,
            since: now,
            total,
            clip,
        },
        Transform::from_translation(spot).with_rotation(Quat::from_rotation_y(yaw)),
    ));
    Some(index)
}

/// A weapon item lying at `at` (a map's, a test's): a new `id` weapon
/// with `total` rounds, `clip` of them in the clip.
pub fn spawn_item(world: &mut World, id: &str, at: Vec3, total: u32, clip: u32) -> Option<Entity> {
    let weapon = crate::weapon::spawn_weapon(world, id, Entity::PLACEHOLDER)?;
    let id = world.get::<Weapon>(weapon)?.id;
    if let Some(mut w) = world.get_mut::<Weapon>(weapon) {
        w.owner = None;
    }
    let now = world.get_resource::<SimClock>().map_or(0.0, |c| c.now);
    Some(
        world
            .spawn((
                Name::new(format!("Dropped {id}")),
                LooseItem(id.to_string()),
                Dropped {
                    weapon,
                    since: now,
                    total,
                    clip,
                },
                Transform::from_translation(at),
            ))
            .id(),
    )
}

/// The dead drop their last chosen primary where they fell (12).
fn death_drops(
    mut died: MessageReader<Died>,
    allow: Res<CaDeathDrops>,
    none: Option<Res<chimera_sim::weapon::drop::NoDeathDrops>>,
    mut commands: Commands,
) {
    for d in died.read() {
        let owner = d.entity;
        let allow = allow.0 && none.is_none();
        commands.queue(move |w: &mut World| death_drop(w, owner, allow));
    }
}

fn death_drop(world: &mut World, owner: Entity, allow: bool) {
    let Some(sel) = world.get::<CaSelection>(owner).cloned() else { return };
    // The weapon in hand is put away.
    let held = world.get_mut::<Inventory>(owner).and_then(|mut inv| {
        inv.wanted = None;
        inv.active.take()
    });
    if let Some(h) = held
        && let Some(mut st) = world.get_mut::<WeaponState>(h)
    {
        *st = WeaponState::default();
    }
    // A character that isn't a player (a Hired Guns mercenary) drops
    // nothing (bots_objectives.md 2.7).
    if !allow || world.get::<core::Npc>(owner).is_some() {
        return;
    }
    let carried = world.get::<Inventory>(owner).map(|i| i.weapons.clone()).unwrap_or_default();
    let Some(primary) = sel.last_chosen[PRIMARY as usize].filter(|w| carried.contains(w)) else {
        return;
    };
    let droppable = world.get::<Weapon>(primary).is_some_and(|w| class_of(w.slot) == PRIMARY)
        && world.get::<CaItem>(primary).is_none_or(|i| i.drop_on_death);
    if droppable {
        drop_item(world, owner, primary, false);
    }
}

/// Dropped items go after `DROPPED_LIFETIME` (12).
fn expire_items(items: Query<(Entity, &Dropped)>, weapons: Query<&Weapon>, clock: Res<SimClock>, mut commands: Commands) {
    for (e, d) in &items {
        if clock.now - d.since + SLACK >= DROPPED_LIFETIME {
            commands.entity(e).despawn();
            if weapons.get(d.weapon).is_ok_and(|w| w.owner.is_none()) {
                commands.entity(d.weapon).despawn();
            }
        }
    }
}

/// Respawn (13): clear what `player` carries and its selection; the
/// loadout is given again on the next tick (`StartingWeapons`), with full
/// clips, cursors at 0, no previous weapon and the primary drawn.
pub fn respawn_inventory(world: &mut World, player: Entity) {
    let weapons = world.get::<Inventory>(player).map(|i| i.weapons.clone()).unwrap_or_default();
    for w in weapons {
        despawn(world, w);
    }
    if let Ok(mut e) = world.get_entity_mut(player) {
        e.remove::<(Inventory, CaSelection)>();
    }
}
