//! Counter-Strike 1.6's movement (`cs16:movement`, specs/cs16/movement.md):
//! Chimera's shared movement of the Quake lineage (`chimera_movement`)
//! with CS 1.6's config (`config`, its numbers from data/movement.toml:
//! `numbers`) and GoldSrc's seams switched on:
//!
//! - **hull traces** on GoldSrc maps (`clip_hulls`: the standing box
//!   through hull 1, the ducked one through hull 3, `goldsrc_engine::
//!   hulls`), Chimera's box sweep on every other game's map and against
//!   characters, movers, ladder volumes and props (goldsrc.md §3);
//! - **commands of whole milliseconds** (`command_msec`: 10 ms at the
//!   game's 0.01 s tick; a party's other tick carries the remainder);
//! - **edge friction** (`edgefriction` 2) and the cvar defaults
//!   (accelerate 5, airaccelerate 10, friction 4, stopspeed 75, stepsize
//!   18, gravity 800, maxvelocity 2000), each a console variable under
//!   `cs16:`;
//! - **the jump**: 268.33 units/s set (not added), no auto-jump; the
//!   **jump penalty** (the landing slowdown): a 1 315.79 ms timer set by
//!   each jump, every ground tick while it runs scaling the horizontal
//!   speed by 1 - 0.00019 x timer, and a jump's speed too (per command,
//!   so it depends on the tick: kept, at 10 ms); the **bunny-hop cap**: a
//!   take-off above 1.2 x the max speed is cut to 0.96 x it;
//! - **the duck**: the 36-unit box, the keys a third (0.333) from the
//!   press, the box 0.4 s after it (40 commands), at once in the air with
//!   the feet lifted 18 (the duck jump), standing up at once where the
//!   standing box fits (a duck let go before it completed hops 18 units);
//! - **the ground**: tested every tick, the body put on its 2-unit probe's
//!   end (slopes, small drops), in the air above 180 units/s up; steps of
//!   18; slides' components under 0.1 stopped;
//! - **ladders** (`func_ladder` volumes, climbed at 200 units/s, a third
//!   ducked, 270 off with jump) and **water** (the shared swim, its water
//!   jump 225 up);
//! - **falls**: landing effects from 250 units/s (loud above 580, 0.85
//!   above 290), damage above 500 of (speed - 500) / 4.8 whole points;
//! - **being hit** (`hit_slow`, `on_hit`): a weapon's hit sets the speed
//!   modifier (0.5; 0.65 and a 170 units/s push for a large flinch: one of
//!   the data's twelve guns, not in the legs, the victim not ducked),
//!   which on each ground tick gains 0.01 and scales the whole velocity;
//! - **the punch roll** decaying by 10 degrees/s plus half its size a
//!   second.
//!
//! **Speed by what is held** (`speed`, data/movement.toml's table): the
//! weapon layer hands the movement the held gun's `max_speed`
//! (`core::MaxSpeed`: each gun's document states it, and its scope's
//! zoomed speed); without one it is the table's `default` (250). The
//! table is the spec's, and the movement's tests hold every gun's
//! `max_speed` to it.
//!
//! Known shortcuts:
//! - Ladder volumes are solid ladder brushes found by a 2-unit probe
//!   along the keys (the shared code's, as on every game's maps); the
//!   original's are not solid and hold a player whose box is in them.
//! - Swimming is the shared code's (jump held adds the player's speed to
//!   the wish; the spec's up key, slime and lava speeds, and currents are
//!   not modelled). GoldSrc maps hand over their liquids as water
//!   volumes (`goldsrc_engine::content::water`).
//! - The walk key follows the spec's letter (the keys' amounts x 0.52:
//!   208 units/s with a 250 gun), not checked in the game.
//! - Moving brush entities are box sweeps (`goldsrc_engine::hulls`). A
//!   predicting client riding one is corrected once when it stops, as
//!   when it starts (`heavy::net_logic`'s elevator ride: the rider is put
//!   on its ground probe's end each tick, where the client's lift is on
//!   the tick it arrives; not looked into further).
//! - The stuck rules (the server's one offset every 50 ms, the push out of
//!   another player) are the shared code's nudge table.
//! - Landing step sounds wait on the map's sounds (none converted yet);
//!   fall damage does not yet zero the pitch punch (the weapons', M1).

use bevy::prelude::*;
use chimera_movement::{QUAKE_CVARS, QuakeConfig, QuakeConfigs, QuakeMovement, QuakeMovementAppExt};
use serde::Deserialize;

use crate::{
    core::{Hitgroup, Velocity},
    slots::{Loadout, MovementSlot},
};

/// The movement's id.
pub const ID: &str = "cs16:movement";

/// The movement's numbers (data/movement.toml).
pub const DATA: chimera_core::game_data::GameData = chimera_core::game_data!("data/movement.toml");

/// data/movement.toml (the spec's names on each line there).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Numbers {
    pub cvars: std::collections::BTreeMap<String, f32>,
    pub keys: Keys,
    pub body: Body,
    pub jump: Jump,
    pub water: Water,
    pub fall: Fall,
    pub hit: Hit,
    pub punch: Punch,
    pub speed: std::collections::BTreeMap<String, f32>,
    pub speed_zoomed: std::collections::BTreeMap<String, f32>,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct Keys {
    pub speed: f32,
    pub walk: f32,
    pub duck: f32,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub half_width: f32,
    pub stand_height: f32,
    pub duck_height: f32,
    pub eye_stand: f32,
    pub eye_duck: f32,
    pub duck_time: f32,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct Jump {
    pub speed: f32,
    pub penalty_ms: f32,
    pub penalty_slope: f32,
    pub cap: f32,
    pub cut: f32,
    pub leave_ground: f32,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct Water {
    pub jump_up: f32,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct Fall {
    pub punch_min: f32,
    pub safe: f32,
    pub fatal: f32,
    pub damage_factor: f32,
    pub sound_full: f32,
    pub sound_soft: f32,
    pub sound_none: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Hit {
    pub slow_small: f32,
    pub slow_large: f32,
    /// The guns whose hits are large flinches, by id after `cs16:weapon_`.
    pub large: Vec<String>,
    pub knockback: f32,
    pub knockback_below: f32,
    pub recover: f32,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct Punch {
    pub decay_const: f32,
    pub decay_prop: f32,
}

/// The numbers of data/movement.toml.
pub fn numbers() -> Numbers {
    toml::from_str(DATA.text()).unwrap_or_else(|e| panic!("{}: {e}", DATA.path()))
}

/// The max speed (units/s) holding the gun `id` (`cs16:weapon_ak47` or
/// `ak47`; None: empty hands), zoomed or not: the spec's table
/// (data/movement.toml), `default` for a gun it doesn't list.
pub fn speed(n: &Numbers, id: Option<&str>, zoomed: bool) -> f32 {
    let Some(id) = id else { return n.speed["none"] };
    let key = id.strip_prefix("cs16:weapon_").unwrap_or(id);
    let held = n.speed.get(key).copied().unwrap_or(n.speed["default"]);
    if zoomed { n.speed_zoomed.get(key).copied().unwrap_or(held) } else { held }
}

/// CS 1.6's own console variables beside the shared ones (`QUAKE_CVARS`).
pub const CVARS: &[(&str, &str)] = &[("edgefriction", "ground friction multiplier near a drop")];

/// CS 1.6's movement config (module doc).
pub fn config() -> QuakeConfig {
    let n = numbers();
    let mut c = QuakeConfig {
        // The keys.
        key_speed: n.keys.speed,
        walk_speed: n.keys.walk,
        walk_scales_keys: true,
        duck_speed: n.keys.duck,
        duck_slows_everywhere: true,
        duck_slows_ladder: true,
        player_maxspeed: n.speed["default"],
        client_maxspeed: n.speed["default"],
        // The box about its centre: 72 and 36 high, the eyes 53 and 30
        // over the feet; ducking in the air lifts the feet by half the
        // difference (the centre stays).
        half_width: n.body.half_width,
        stand_height: n.body.stand_height,
        duck_height: n.body.duck_height,
        eye_stand: n.body.eye_stand,
        eye_duck: n.body.eye_duck,
        air_duck_shift: 0.5,
        duck_time: n.body.duck_time,
        duck_ends_on_time: true,
        unduck_at_once: true,
        // The jump, its penalty (the shared stamina timer) and its cap.
        jump_impulse: n.jump.speed,
        jump_sets_speed: true,
        jump_stamina: n.jump.penalty_ms,
        stamina_jump_scale: n.jump.penalty_slope,
        stamina_ground_scale: n.jump.penalty_slope,
        enable_bunnyhopping: false,
        bunnyhop_cap: 0.0,
        bunnyhop_cap_scale: n.jump.cap,
        bunnyhop_cut: n.jump.cut,
        auto_bunnyhopping: false,
        // The ground and the slides.
        leave_ground_vz: n.jump.leave_ground,
        ground_snap: true,
        clip_stop_epsilon: 0.1,
        rising_friction: 1.0,
        // GoldSrc's seams.
        clip_hulls: true,
        command_msec: true,
        // Ladders and water.
        contents_ladders: true,
        useable_ladders: false,
        water_jump_up: n.water.jump_up,
        drowning: false,
        // Falls: effects from `punch_min`, loud above `sound_full`, 0.85
        // above `sound_soft`, silent below `sound_none`; damage above
        // `safe`, 100 points at `fatal` before the factor.
        fall_punch: n.fall.punch_min,
        fall_safe: n.fall.safe,
        fall_damage_per_speed: 100.0 / (n.fall.fatal - n.fall.safe) * n.fall.damage_factor,
        fall_loud: Some(n.fall.sound_full),
        fall_half_safe: n.fall.sound_soft,
        fall_bounce: n.fall.sound_none,
        floating_bonus: 0.0,
        // Being hit, the punch.
        modifier_per_tick: n.hit.recover,
        punch_decay: Some((n.punch.decay_const, n.punch.decay_prop)),
        ..QuakeConfig::reference()
    };
    for (name, value) in &n.cvars {
        c.set_cvar(name, &value.to_string()).unwrap_or_else(|e| panic!("{}: {e}", DATA.path()));
    }
    c
}

/// A hit's slowdown on a CS 1.6 mover (specs/cs16/movement.md, "Being
/// hit"): the speed modifier 0.5, or for a large flinch (`large`: a gun
/// of `Hit::large`, not in the legs, the victim not ducked) 0.65 and,
/// when the victim is slower than 300 units/s, a push of 170 units/s away
/// from the shooter (`away`: flat, engine space). The movement then
/// recovers it per ground tick (`QuakeConfig::modifier_per_tick`).
pub fn hit_slow(n: &Hit, me: &mut QuakeMovement, velocity: &mut Velocity, large: bool, away: Vec3) {
    if !large {
        me.velocity_modifier = n.slow_small;
        return;
    }
    let unit = super::METERS_PER_UNIT;
    if velocity.0.length() < n.knockback_below * unit {
        velocity.0 += away.normalize_or_zero() * n.knockback * unit;
    }
    me.velocity_modifier = n.slow_large;
}

/// The numbers `on_hit` reads, parsed once.
#[derive(Resource, Clone, Debug)]
pub struct HitNumbers(pub Hit);

/// Each hit a weapon deals a living character on `cs16:movement` slows it
/// (`hit_slow`). The server's: a predicting victim learns it with its
/// movement state's next correction.
pub fn on_hit(
    mut events: MessageReader<chimera_sim::weapon::WeaponEvent>,
    numbers: Res<HitNumbers>,
    weapons: Query<&chimera_sim::weapon::Weapon>,
    places: Query<&Transform>,
    mut victims: Query<(&MovementSlot, &mut QuakeMovement, &mut Velocity, &Transform, &crate::core::Health)>,
) {
    for e in events.read() {
        let chimera_sim::weapon::WeaponEventKind::Hit { target, hitgroup, .. } = e.kind else { continue };
        let Ok((slot, mut me, mut velocity, at, health)) = victims.get_mut(target) else { continue };
        if slot.0 != ID || health.current <= 0.0 {
            continue;
        }
        let legs = matches!(hitgroup, Hitgroup::LeftLeg | Hitgroup::RightLeg);
        let gun = weapons.get(e.weapon).ok().and_then(|w| w.id.strip_prefix("cs16:weapon_"));
        let is_large = gun.is_some_and(|g| numbers.0.large.iter().any(|l| l == g)) && !legs && !me.ducked;
        let away = places.get(e.owner).map_or(Vec3::ZERO, |from| (at.translation - from.translation).with_y(0.0));
        hit_slow(&numbers.0, &mut me, &mut velocity, is_large, away);
    }
}

/// `cs16:movement`, and the match's movement.
pub struct Cs16MovementPlugin;

impl Plugin for Cs16MovementPlugin {
    fn build(&self, app: &mut App) {
        add_movement(app);
        app.insert_resource(Loadout { movement: ID });
    }
}

/// `cs16:movement` on a composed game's simulation (a party's) without
/// making it the match's; once.
pub fn add_movement(app: &mut App) {
    if app.world().get_resource::<QuakeConfigs>().is_some_and(|c| c.get(ID).is_some())
        && app.world().contains_resource::<HitNumbers>()
    {
        return;
    }
    app.quake_movement(ID, config(), &[QUAKE_CVARS, CVARS], Some(super::GAME))
        .add_plugins(goldsrc_engine::hulls::GoldSrcHullsPlugin)
        .insert_resource(HitNumbers(numbers().hit))
        .add_systems(
            FixedUpdate,
            on_hit
                .after(crate::core::SimSet::Weapons)
                .after(crate::core::apply_damage)
                .run_if(crate::core::authoritative),
        );
}
