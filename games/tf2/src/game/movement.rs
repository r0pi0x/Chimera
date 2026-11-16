//! TF2's movement (`tf2:movement`, specs/tf2/movement.md): Source's player
//! movement (`source_engine::source_movement`) with TF2's config
//! (`config`: gravity 800, accelerate and airaccelerate 10, friction 4,
//! stopspeed 100, keys 450, the jump 289 added to the vertical speed (set
//! during the duck-down), the 48 x 48 x 82 box (62 ducked, eyes 45), the
//! 0.2 s duck, the whole 20-unit lift in the air, two duck releases per
//! air time (the Scout's air jump counts again), the 0.3 s re-duck delay
//! on the ground and no re-duck during an unduck, no duck in water, no
//! ground jump while fully ducked, ducked a third, the ground speed
//! clamped to the max speed, the backpedal clamp (on the ground, the
//! velocity's backward part at most 0.9 of it above 100 units/s; the input
//! untouched, so the Scout's air jump is 400 every way), a raised
//! acceleration for wishes under 40, the ground left above 250 units/s
//! up, the jump's cap at 1.2 x the player's top speed P, the 520 ceiling
//! in place of `sv_maxspeed`, fall damage as max health x speed / 6000
//! above 650 with a +-20 % spread: movement.md and its corrections from
//! the SDK)
//! and a class layer: each player is the class of the TF2 gun in hand
//! (`super::classes`, `select_class`, predicted: its rung's drawn class
//! when the gun can be it, `classes::RungClass`, else the gun's own),
//! whose reload times follow it (the Shotgun's start, the Pistol's
//! clip: `weapons::damage::Tf2Gun::for_class`), whose top speed is the
//! tick's speed (`SourceTraits::max_speed`; the Heavy's spun Minigun lowers it to 110,
//! its weapon's `max_speed`, the zoomed rifle to 80, the weapon layer's
//! `MaxSpeed`), whose eyes stand at 65, 68 or 75 and whose
//! air jumps (the Scout's one: the vertical speed set to 268.33, the
//! horizontal velocity to the keys' wish) are the class's
//! (`SourceTraits`). On the server a class's max health is the player's
//! (`class_health`: the health's share kept across a change of class; a
//! player that leaves TF2's movement gets back the max it had). The
//! match's movement (`slots::Loadout`); its cvars are `tf2:`'s
//! (`tf2:sv_gravity` ..., `tf2:tf_fall_damage_disablespread`).
//!
//! Known shortcuts:
//! - The teammates' push-away is Source's shared one; the Spy's disguise
//!   and the Medic's patient speed aren't modelled (no such classes' kits
//!   yet).
//! - The landing sound and view punch thresholds are the shared code's
//!   (10); a player holding another game's gun on TF2's movement (none
//!   in the party: its movement follows the gun) keeps its last class.

use bevy::prelude::*;
use source_engine::source_movement::{
    SourceMovementAppExt, SourceMovementConfig, SourceMovementConfigs, SourceMovementStep, SourceTraits,
};

use super::{
    classes::{Classes, RungClass, Tf2Class},
    weapons::damage::Tf2Gun,
};
use crate::{
    core::{Health, MaxSpeed},
    slots::{Loadout, MovementSlot},
    weapon::{Inventory, Magazine, ShellReload, Weapon},
};

/// The movement's id.
pub const ID: &str = "tf2:movement";

/// The Scout's air jump's vertical speed, √(2 · 800 · 45) units/s.
pub const AIR_JUMP_SPEED: f32 = 268.328_16;

/// The highest top speed any effect gives (movement.md, "Speed ceiling").
pub const SPEED_CEILING: f32 = 520.0;

/// TF2's movement config (movement.md "Constants").
pub fn config() -> SourceMovementConfig {
    SourceMovementConfig {
        accelerate: 10.0,
        airaccelerate: 10.0,
        friction: 4.0,
        stopspeed: 100.0,
        gravity: 800.0,
        // Not `sv_maxspeed` 320 (the Scout's 400 passes it): the ceiling.
        maxspeed: SPEED_CEILING,
        stepsize: 18.0,
        jump_impulse: 289.0,
        player_maxspeed: 240.0,
        key_speed: 450.0,
        duck_speed: 1.0 / 3.0,
        walk_speed: 1.0,
        stand_height: 82.0,
        duck_height: 62.0,
        eye_stand: 68.0,
        eye_duck: 45.0,
        air_duck_shift: 1.0,
        half_width: 24.0,
        duck_time: 0.2,
        duck_counts_first_tick: true,
        jump_stamina: 0.0,
        enable_bunnyhopping: false,
        bunnyhop_cap: 0.0,
        bunnyhop_cap_scale: 1.2,
        back_speed: 0.9,
        back_min_speed: 100.0,
        ground_clamp: true,
        low_wish_accel: true,
        leave_ground_vz: 250.0,
        reduck_delay: 0.3,
        no_water_duck: true,
        air_jumps: 0,
        air_jump_speed: AIR_JUMP_SPEED,
        no_ducked_jump: true,
        air_ducks_max: 2,
        fall_safe: 650.0,
        fall_by_max_health: 1.0 / 6000.0,
        fall_spread: 0.2,
        // No ladders in TF2: its maps' ladder brushes are walls.
        contents_ladders: false,
        ..SourceMovementConfig::shared_code()
    }
}

/// TF2's own cvars (movement.md, "Console variables"), under `tf2:`.
pub const CVARS: &[(&str, &str)] = &[("tf_fall_damage_disablespread", "1: fall damage without its +-20 % spread")];

/// `tf2:movement`, and the match's movement.
pub struct Tf2MovementPlugin;

impl Plugin for Tf2MovementPlugin {
    fn build(&self, app: &mut App) {
        add_movement(app);
        app.insert_resource(Loadout { movement: ID });
    }
}

/// `tf2:movement` on a composed game's simulation (a party's) without
/// making it the match's: Source's movement with TF2's config, the class
/// layer, the cvars.
pub fn add_movement(app: &mut App) {
    let registered = app
        .world()
        .get_resource::<SourceMovementConfigs>()
        .is_some_and(|c| c.get(ID).is_some());
    if registered {
        return;
    }
    super::classes::add_rung_classes(app);
    app.init_resource::<Classes>()
        .source_movement(ID, Some(config()), Some(super::GAME))
        .add_systems(FixedUpdate, wear.before(crate::core::SimSet::Movement))
        .add_systems(crate::core::Predict::Movement, select_class.before(SourceMovementStep))
        .add_systems(
            FixedUpdate,
            class_health.after(crate::core::SimSet::Movement).run_if(crate::core::authoritative),
        );
    for (name, help) in CVARS {
        let name = *name;
        crate::console::ConsoleAppExt::console_cvar(
            app,
            &format!("{}:{name}", super::GAME),
            help,
            "0",
            move |w| {
                let cfg = w.get_resource::<SourceMovementConfigs>()?.get(ID)?;
                Some(if cfg.fall_spread > 0.0 { "0" } else { "1" }.to_string())
            },
            move |w, v| {
                let off = v.trim().trim_matches('"').parse::<f32>().map_err(|_| format!("{name}: not a number: {v}"))? != 0.0;
                config_mut(w).fall_spread = if off { 0.0 } else { 0.2 };
                Ok(())
            },
        );
    }
}

/// TF2's movement config as registered (tests and cvars change it here).
pub fn config_mut(world: &mut World) -> Mut<'_, SourceMovementConfig> {
    world
        .resource_mut::<SourceMovementConfigs>()
        .map_unchanged(|c| c.get_mut(ID).expect("tf2:movement registered"))
}

/// The max health a player had before TF2's class gave it its own
/// (`class_health`), given back when it leaves TF2's movement.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct HealthBefore(pub f32);

/// Players on TF2's movement get its class layer (`Tf2Class`, the traits
/// `select_class` sets: speed, eyes, air jumps); one that leaves it loses
/// them, and its max health is the one it had before (the share kept).
#[allow(clippy::type_complexity)]
fn wear(
    mut commands: Commands,
    mut on: Query<(Entity, &MovementSlot, Has<Tf2Class>, Option<&HealthBefore>, Option<&mut Health>), Changed<MovementSlot>>,
) {
    for (e, slot, worn, before, health) in &mut on {
        if slot.0 == ID {
            commands.entity(e).insert_if_new((Tf2Class::default(), SourceTraits::default()));
        } else if worn {
            commands.entity(e).remove::<(Tf2Class, SourceTraits, HealthBefore)>();
            if let (Some(before), Some(mut h)) = (before, health) {
                set_max(&mut h, before.0);
            }
        }
    }
}

/// `h`'s max set to `max`, its share of it kept.
fn set_max(h: &mut Health, max: f32) {
    if (h.max - max).abs() < 1e-6 || max <= 0.0 {
        return;
    }
    let share = if h.max > 0.0 { h.current / h.max } else { 1.0 };
    h.max = max;
    if h.current > 0.0 {
        h.current = (share * max).max(f32::MIN_POSITIVE);
    }
}

/// Each TF2 player's class from the gun in hand (its rung's drawn class
/// when the gun can be it, else the gun's own: `Classes::held`; kept when
/// it holds none of TF2's), the gun's reload times for it, its speed (the
/// class's top speed, or the weapon's own `max_speed` when lower: the
/// spun Minigun's 110) and its traits (eyes, air jumps), first in each
/// movement tick, predicted.
#[allow(clippy::type_complexity)]
pub fn select_class(
    mut q: Query<(&MovementSlot, &Inventory, &mut Tf2Class, &mut SourceTraits, Option<&RungClass>, Option<&MaxSpeed>)>,
    mut weapons: Query<(&Weapon, Option<&Tf2Gun>, Option<&mut ShellReload>, Option<&mut Magazine>)>,
    table: Res<Classes>,
) {
    for (slot, inv, mut class, mut traits, drawn, zoomed) in &mut q {
        if slot.0 != ID {
            continue;
        }
        let mut held = inv.active.and_then(|w| weapons.get_mut(w).ok());
        if let Some((w, gun, shells, magazine)) = held.as_mut()
            && let Some(c) = table.held(&w.id, drawn.map(|d| d.0.as_str()))
        {
            if class.0 != c.index {
                class.0 = c.index;
            }
            if let Some(gun) = gun {
                gun.for_class(&c.name, shells.as_deref_mut(), magazine.as_deref_mut());
            }
        }
        let c = class.class(&table);
        // The weapon's own (the spun Minigun's 110) and the scope's (the
        // weapon layer's `MaxSpeed`: the zoomed rifle's 80).
        let weapon_speed = held.as_ref().and_then(|(w, ..)| w.max_speed).map(|s| s / super::METERS_PER_UNIT);
        let scope_speed = zoomed.and_then(|m| m.0).map(|s| s / super::METERS_PER_UNIT);
        let p = [weapon_speed, scope_speed].into_iter().flatten().fold(c.speed, f32::min);
        let want = SourceTraits { max_speed: Some(p), eye_stand: Some(c.eye), air_jumps: Some(c.air_jumps) };
        if *traits != want {
            *traits = want;
        }
    }
}

/// Each TF2 player's max health is its class's (classes.md; the server's,
/// replicated), its share of health kept across a change; the max it had
/// before TF2's is kept for when it leaves.
pub fn class_health(
    mut q: Query<(Entity, &Tf2Class, &mut Health, Option<&HealthBefore>)>,
    table: Res<Classes>,
    mut commands: Commands,
) {
    for (e, class, mut h, before) in &mut q {
        if before.is_none() {
            commands.entity(e).insert(HealthBefore(h.max));
        }
        let max = class.class(&table).health / 100.0;
        if (h.max - max).abs() > 1e-6 {
            set_max(&mut h, max);
        }
    }
}
