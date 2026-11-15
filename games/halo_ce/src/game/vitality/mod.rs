//! A Halo player's vitality (specs/halo_ce/health_shields.md, M2 of
//! docs/plans/active/halo-ce.md): body and shield as the spec stores them,
//! fractions of the Spartan's 75 points each (`Vitality`), the shield's
//! 6 s delay and 4 s linear recharge, the overshield and active
//! camouflage as vitality state, the body stun movement reads (`Stun`),
//! and the soft and hard pings (`HaloPing`) for animation later. The
//! numbers are data (`data`: data/vitality.toml, data/damage.toml), the
//! spec's arithmetic pure functions (`apply`); damage reaches a Halo
//! character through the seam in `super::damage` (`HaloHit`).
//!
//! **The engine's view.** Every character on Halo's movement wears
//! Halo's shield as its one armour (docs/DESIGN.md "Hits across games"):
//! `weapon::Armor`'s amount is the shield fraction (1.0 a full shield, 3.0
//! an overshield), covered by `shield_armour` (every part and class, run
//! out by what it blocks) with its own ratio (`weapon::ArmorRatio(1)`), so
//! any other game's hit (a CS:S bullet, a Combat Arms grenade) empties the
//! shield first through the shared armour step and only the overflow
//! reaches health. `core::Health` is the body fraction (1.0 the Spartan's
//! 75 points: a Halo point is 1/75 of a standard player's health), with
//! one exception: a body of exactly 0 is alive in Halo (the spec's strict
//! below-zero death) and shows as `ALIVE_AT_ZERO`. `Vitality` holds the
//! spec's exact values; each tick (`update`, after `core::apply_damage`)
//! takes in what other games' hits did to `Armor` and `Health` (a drop
//! restarts the shield delay, as any hit does), runs the shield and
//! camouflage step once per 30 Hz tick the engine tick advances
//! (`ticks::halo_ticks`: at Halo's tick every tick, under a party's 66.7 Hz
//! on 30 of every 66.7, so the delay is 6 s and the recharge 4 s at any
//! tick rate; the stun's counter likewise), and writes the two back. Kills, `Died` and the kill's
//! credit go through the engine's `Damage` (`super::damage`), so a gun
//! game's ladder sees Halo's kills as any other.
//!
//! **Wear.** A character becoming Halo's (its movement `halo_ce:movement`,
//! a party app's identity following a Halo gun) gets `Vitality`, `Stun`,
//! the shield's pieces (over any the last game left, marked Halo's:
//! `weapon::PiecesOf`, so another game's armour step leaves them on, and
//! put back by `keep_shield` if one took them off in the same tick) and
//! ratio, and an `Armor` if it had none (a full shield; one it had stays:
//! the armour value is the character's and recharges from there); one
//! leaving takes them off, keeping `Armor`.
//! A respawn (health back above 0) starts a fresh `Vitality`, shield full.
//!
//! **Replication.** The server owns all of it: `Vitality` replicates to
//! every client (`halo_ce_vitality`: the owner's HUD, everyone's shield
//! glow and camouflage later); `Stun` is predicted (`predicted_net`) and
//! steps in the predicted tick (`UnitStep`, in `Predict::Movement`), as
//! movement reads it there.
//!
//! **For movement** (specs/halo_ce/movement.md): read `Stun` (its
//! `speed_scale` multiplies the wanted speed and the jump; stunned
//! animations above 0.2) in systems ordered `.after(UnitStep)`; write
//! falls as `HaloHit::fall` / `HaloHit::lethal_fall` (`super::damage`).
//! **For the HUD**: `Vitality::shield_points`, `body_points`,
//! `charging`, `recent_shield_damage`; the engine's fallback HUD shows
//! `Health` and has no second bar.
//!
//! **Powerups**: `GrantPowerup` (health pack, overshield, camouflage),
//! processed at the end of the tick's update (pickups come after the
//! objects' update, "Per-tick order"); `Vitality::fired` for a shot's
//! camouflage loss (the weapons' side).
//!
//! Known shortcuts:
//! - The game type's options (health, no shields, always invisible) and
//!   traits are not read: multiplayer defaults (game multiplier 1).
//! - Other games' hits on a Halo character restart the shield delay but
//!   neither stun nor ping it, nor count for its kill credit's attacker
//!   list; their own game decides how much (`hits::absorb` through the
//!   shield's coverage), with no Halo material multipliers.
//! - The pickups themselves (touching equipment) are a later step:
//!   `GrantPowerup` is how they will grant.
//! - Assists are not counted (no engine score for them yet).

pub mod apply;
pub mod data;

use bevy::prelude::*;
use chimera_sim::weapon::{Armor, ArmorPieces, ArmorRatio, Coverage, Depletion, PiecesOf, hits::ArmorPiece};
use serde::{Deserialize, Serialize};

use crate::{
    core::{Health, Predict, PredictedAppExt, SimSet},
    slots::MovementSlot,
};

pub use apply::{Event, Outcome, melee_scale, unsuspecting};
pub use data::{DamageEffect, DamageEffects, EffectId, Spartan, ids};

/// The health a living Spartan whose body is exactly 0 shows (the
/// engine's health dies at 0; Halo's body only below it).
pub const ALIVE_AT_ZERO: f32 = 1e-6;

/// The body fraction as `core::Health`: itself, `ALIVE_AT_ZERO` at
/// exactly 0, 0 below.
pub fn health_of(body: f32) -> f32 {
    if body > 0.0 {
        body
    } else if body == 0.0 {
        ALIVE_AT_ZERO
    } else {
        0.0
    }
}

/// A Halo character's vitality (health_shields.md "How vitality is
/// represented"), the spec's values exactly. Owned by the server,
/// replicated to every client.
#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[component(storage = "SparseSet")]
pub struct Vitality {
    /// β: the body fraction of 75 points; below 0 dead.
    pub body: f32,
    /// σ: the shield fraction of 75 points, 0 to 3 (`Armor::amount`).
    pub shield: f32,
    /// d: ticks before the shield recharges.
    pub delay: u32,
    pub current_shield_damage: f32,
    pub recent_shield_damage: f32,
    pub current_body_damage: f32,
    pub recent_body_damage: f32,
    /// The fade timers (None: idle).
    pub shield_fade: Option<u32>,
    pub body_fade: Option<u32>,
    /// The shield is marked depleted (broken, not yet recharging).
    pub depleted: bool,
    /// The overshield is charging up (invulnerable).
    pub overcharging: bool,
    /// The shield grew this tick and isn't above 1 (the HUD's and sounds'
    /// "charging").
    pub charging: bool,
    pub dead: bool,
    /// κ: 0 visible, 1 fully camouflaged.
    pub camo: f32,
    /// The camouflaged state and its timer (ticks).
    pub camouflaged: bool,
    pub camo_ticks: u32,
    /// The regrowth rate after a shot, while it lasts.
    pub camo_regrowth: Option<f32>,
}

impl Default for Vitality {
    /// A fresh multiplayer Spartan: β = 1, σ = 1.
    fn default() -> Self {
        Self {
            body: 1.0,
            shield: 1.0,
            delay: 0,
            current_shield_damage: 0.0,
            recent_shield_damage: 0.0,
            current_body_damage: 0.0,
            recent_body_damage: 0.0,
            shield_fade: None,
            body_fade: None,
            depleted: false,
            overcharging: false,
            charging: false,
            dead: false,
            camo: 0.0,
            camouflaged: false,
            camo_ticks: 0,
            camo_regrowth: None,
        }
    }
}

impl Vitality {
    /// The shield in points (σ x 75).
    pub fn shield_points(&self, spartan: &Spartan) -> f32 {
        self.shield * spartan.model.maximum_shield
    }

    /// The body in points (β x 75).
    pub fn body_points(&self, spartan: &Spartan) -> f32 {
        self.body * spartan.model.maximum_body
    }
}

/// A Halo character's body stun (health_shields.md "Stun and pings"):
/// `value` (0 to 1) holds until `ticks` runs out, then drops to 0.
/// Predicted: movement reads it (`speed_scale`) after `UnitStep`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[component(storage = "SparseSet")]
pub struct Stun {
    pub value: f32,
    pub ticks: u32,
}

/// The unit update's steps in the predicted tick (`Stun::step`), before
/// the biped's movement: movement orders itself after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnitStep;

/// The direction a ping plays from ("Pings"): the hit's travel against
/// the unit's facing, flattened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PingDirection {
    Back,
    Front,
    Left,
    Right,
}

/// A flinch for animation (health_shields.md "Pings"): soft, hard or
/// both, from a hit that was not silent.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct HaloPing {
    pub target: Entity,
    pub soft: bool,
    pub hard: bool,
    /// The killing hit.
    pub kill: bool,
    pub direction: PingDirection,
}

/// The ping direction of a hit travelling along `dir` on a unit facing
/// `facing` (engine space, Y up), by the spec's cones.
pub fn ping_direction(spartan: &Spartan, dir: Vec3, facing: Vec3) -> PingDirection {
    let (d, f) = (Vec2::new(dir.x, -dir.z), Vec2::new(facing.x, -facing.z));
    let angle = f.angle_to(d);
    let c = &spartan.constants;
    if angle.abs() < c.ping_rear_cone {
        PingDirection::Back
    } else if angle.abs() > c.ping_front_cone {
        PingDirection::Front
    } else if angle > 0.0 {
        PingDirection::Left
    } else {
        PingDirection::Right
    }
}

/// A powerup for a Halo character (the pickups' step, later; tests).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Powerup {
    HealthPack,
    Overshield,
    ActiveCamouflage,
}

/// Grant `powerup` to `target` at the end of this tick's update
/// (`GrantPowerup`); whether it was taken is `PowerupTaken`.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct GrantPowerup {
    pub target: Entity,
    pub powerup: Powerup,
}

/// A powerup a Halo character took (refused ones aren't told).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct PowerupTaken {
    pub target: Entity,
    pub powerup: Powerup,
}

/// Halo's shield as the engine's armour coverage: every part, every
/// damage class, one shield point per point it blocks, run out by what it
/// blocks (so the overflow reaches health); with `ArmorRatio(1.0)`.
pub fn shield_armour() -> ArmorPieces {
    ArmorPieces {
        pieces: vec![ArmorPiece {
            entries: vec![Coverage {
                class: None,
                head: 1.0,
                torso: 1.0,
                arm: 1.0,
                leg: 1.0,
                groin: 1.0,
                wear: 1.0,
                except: 0,
            }],
        }],
        depletion: Depletion::Blocked,
        armour_quantum: None,
    }
}

/// What the server last wrote to a Halo character's `Health` and
/// `Armor`, to tell other games' hits apart (server-side).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(storage = "SparseSet")]
pub struct Mirrored {
    pub health: f32,
    pub shield: f32,
}

pub struct VitalityPlugin;

impl Plugin for VitalityPlugin {
    fn build(&self, app: &mut App) {
        use chimera_net::net::parts::NetAppExt;
        app.insert_resource(Spartan::load())
            .insert_resource(DamageEffects::load())
            .add_message::<HaloPing>()
            .add_message::<GrantPowerup>()
            .add_message::<PowerupTaken>()
            .predicted_net::<Stun>()
            .net_part("halo_ce_vitality", |app| {
                use bevy_replicon::prelude::AppRuleExt;
                app.replicate::<Vitality>();
            })
            .add_systems(FixedUpdate, (wear, keep_shield).chain().before(SimSet::Movement))
            .add_systems(Predict::Movement, stun_step.in_set(UnitStep))
            .add_systems(
                FixedUpdate,
                update.after(crate::core::apply_damage).run_if(crate::core::authoritative),
            );
    }
}

/// Characters on Halo's movement wear its vitality; leaving it takes it
/// off (`Armor` stays).
fn wear(mut commands: Commands, on: Query<(Entity, &MovementSlot, Has<Vitality>), Changed<MovementSlot>>) {
    for (e, slot, halo) in &on {
        let mut ent = commands.entity(e);
        if slot.0 == super::movement::ID {
            // The shield's pieces over any the last game left on.
            ent.insert((shield_armour(), PiecesOf(super::GAME), ArmorRatio(1.0)));
            ent.insert_if_new((
                Vitality::default(),
                Stun::default(),
                Armor { amount: 1.0, helmet: false },
                super::damage::Attackers::default(),
            ));
        } else if halo {
            ent.remove::<(Vitality, Stun, ArmorPieces, PiecesOf, ArmorRatio, Mirrored, super::damage::Attackers)>();
        }
    }
}

/// A Halo character's armour is the shield's pieces: put back if another
/// game's step took them off or put its own on in the same tick as the
/// wear (each game's armour step runs on its own; only a mismatch writes).
fn keep_shield(on: Query<(Entity, Option<&ArmorPieces>, Option<&PiecesOf>), With<Vitality>>, mut commands: Commands) {
    let shield = shield_armour();
    for (e, pieces, owner) in &on {
        if pieces != Some(&shield) || owner.is_none_or(|o| o.0 != super::GAME) {
            commands.entity(e).insert((shield.clone(), PiecesOf(super::GAME), ArmorRatio(1.0)));
        }
    }
}

/// The predicted unit update: the stun counter.
fn stun_step(mut q: Query<&mut Stun>, clock: Res<crate::core::SimClock>) {
    let n = super::ticks::halo_ticks(&clock);
    for mut stun in &mut q {
        for _ in 0..n {
            if stun.ticks > 0 {
                stun.step();
            }
        }
    }
}

/// Two health values this close are one (the engine's subtraction of a
/// Halo hit's share leaves float noise).
pub const SAME_HEALTH: f32 = 1e-5;

/// Take in what other games' hits did to `armor` and `health` since the
/// server last wrote them (`mirrored`): a drop restarts the shield delay
/// (any hit does) and a broken shield is marked; then the spec's values
/// are theirs.
pub fn take_foreign(spartan: &Spartan, v: &mut Vitality, health: &Health, armor: Option<&Armor>, mirrored: &Mirrored) {
    if let Some(armor) = armor
        && armor.amount.to_bits() != mirrored.shield.to_bits()
    {
        if armor.amount < mirrored.shield {
            v.delay = spartan.shield_delay();
            if armor.amount <= 0.0 && !v.depleted {
                v.depleted = true;
                v.current_shield_damage = 0.0;
            }
        }
        v.shield = armor.amount.clamp(0.0, spartan.constants.overshield_max);
    }
    if (health.current - mirrored.health).abs() > SAME_HEALTH && !v.dead {
        if health.current < mirrored.health {
            v.delay = spartan.shield_delay();
        }
        v.body = health.current;
        if health.current <= 0.0 {
            v.die();
        }
    }
}

/// Write the spec's values to the engine's `Health` and `Armor` (only
/// what differs, so change detection means a change).
pub fn mirror(v: &Vitality, health: &mut Health, armor: Option<&mut Armor>, mirrored: &mut Mirrored) {
    if !v.dead {
        let h = health_of(v.body);
        if health.current.to_bits() != h.to_bits() {
            health.current = h;
        }
    }
    mirrored.health = health.current;
    if let Some(armor) = armor {
        if armor.amount.to_bits() != v.shield.to_bits() {
            armor.amount = v.shield;
        }
        mirrored.shield = armor.amount;
    }
}

/// The tick's vitality update for every Halo character, after this tick's
/// hits: other games' hits taken in, a respawn's fresh start, the
/// spec's tick (`Vitality::tick`), the values written back, then the
/// tick's pickups.
#[allow(clippy::type_complexity)]
fn update(
    mut q: Query<(Entity, &mut Vitality, &mut Health, Option<&mut Armor>, Option<&mut Stun>, Option<&mut Mirrored>)>,
    spartan: Res<Spartan>,
    clock: Res<crate::core::SimClock>,
    mut grants: MessageReader<GrantPowerup>,
    mut taken: MessageWriter<PowerupTaken>,
    mut commands: Commands,
) {
    let grants: Vec<GrantPowerup> = grants.read().copied().collect();
    for (e, mut v, mut health, mut armor, stun, mirrored) in &mut q {
        let Some(mut mirrored) = mirrored else {
            // First tick: the engine's values are where Halo starts (an
            // armour value from another game stays, and recharges).
            if let Some(a) = armor.as_deref() {
                v.shield = a.amount.clamp(0.0, spartan.constants.overshield_max);
            }
            if health.current > 0.0 && health.current < 1.0 {
                v.body = health.current;
            }
            let mut m = Mirrored::default();
            mirror(&v, &mut health, armor.as_deref_mut(), &mut m);
            commands.entity(e).insert(m);
            continue;
        };
        if v.dead && health.current > 0.0 {
            // Respawned: a new unit, shield and body full.
            *v = Vitality::default();
            if let Some(a) = armor.as_deref_mut() {
                a.amount = 1.0;
            }
            if let Some(mut s) = stun {
                *s = Stun::default();
            }
        } else {
            take_foreign(&spartan, &mut v, &health, armor.as_deref(), &mirrored);
            if v.dead {
                v.die();
            }
        }
        for _ in 0..super::ticks::halo_ticks(&clock) {
            v.tick(&spartan);
        }
        for g in grants.iter().filter(|g| g.target == e) {
            let ok = match g.powerup {
                Powerup::HealthPack => v.heal(),
                Powerup::Overshield => v.overshield(&spartan),
                Powerup::ActiveCamouflage => v.camouflage(&spartan),
            };
            if ok {
                taken.write(PowerupTaken {
                    target: e,
                    powerup: g.powerup,
                });
            }
        }
        mirror(&v, &mut health, armor.as_deref_mut(), &mut mirrored);
    }
}
