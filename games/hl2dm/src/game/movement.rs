//! HL2:DM's movement (`hl2dm:movement`, specs/hl2dm/movement.md, H1):
//! Source's player movement (`source_engine::source_movement`, lift S6)
//! with HL2:DM's numbers (`config`: gravity 600, a fixed 160 jump, hulls
//! 72/36 with eyes 64/28 and the whole 36-unit duck lift in the air, a
//! third of the input ducked, accelerate 10, stopspeed 100, keys 450, no
//! stamina and no bunny-hop cap), HL2's fall damage (thresholds 303,
//! 526.5, 922.5; `mp_falldamage` 0: a flat 10, 1: by speed with the
//! fraction kept), `func_useableladder`s instead of ladder brushes, the
//! swim lift of the player's own speed, footsteps of the model's own run
//! steps (none while ducked), and the speed selection (`select_speed`):
//! run 190, `+walk` 150, `+speed` sprint 320 on the suit's aux power
//! (`super::suit`), started only on the key's press. The match's movement
//! (`slots::Loadout`). Its cvars are `hl2dm:`'s (`hl2dm:sv_gravity` ...,
//! lift C8), backed by its own config, apart from CS:S's.
//!
//! Known shortcuts:
//! - `+speed` and `+walk`: Chimera's shared `+speed` sets both the walk
//!   and the sprint key (`core::Intent`), so walking here is the walk key
//!   without the sprint key: holding ALT and SHIFT with no aux power runs
//!   at 190 (HL2:DM walks at 150).
//! - Not modelled: the ground material's speed factor, landing on a
//!   floating object or a descending one (movement.md "Falling and
//!   landing"), `IgnoreFallDamage` inputs, the waist-water footstep
//!   counter shared by all players (a one-tick delay), `xc_uncrouch_on_jump`
//!   (toggle duck isn't modelled), the ground probe ignoring hostile
//!   combine balls (none in HL2:DM), `trigger_vphysics_motion` blocking
//!   jumps (no stock map has one).

use bevy::prelude::*;
use source_engine::source_movement::{
    SourceMovement, SourceMovementAppExt, SourceMovementConfig, SourceMovementConfigs, SourceMovementStep, SourceSpeed,
};

use crate::{
    core::{FirstTimePredicted, Intent, SimClock, Velocity},
    map::PlaySound,
    slots::{Loadout, MovementSlot},
};

/// The movement's id.
pub const ID: &str = "hl2dm:movement";

/// Walk, run and sprint speeds (`hl2_walkspeed`, `hl2_normspeed`,
/// `hl2_sprintspeed`), units/s.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Speeds {
    pub walk: f32,
    pub run: f32,
    pub sprint: f32,
}

impl Default for Speeds {
    fn default() -> Self {
        Self {
            walk: 150.0,
            run: 190.0,
            sprint: 320.0,
        }
    }
}

/// Aux power needed to start a sprint, and below which it stops
/// (movement.md "Speed selection").
pub const SPRINT_START: f32 = 10.0;
pub const SPRINT_STOP: f32 = 0.01;

/// HL2:DM's movement config (movement.md "Constants"; source_deltas.md
/// 1.1): the shared code's values with HL2:DM's.
pub fn config() -> SourceMovementConfig {
    SourceMovementConfig {
        accelerate: 10.0,
        airaccelerate: 10.0,
        friction: 4.0,
        stopspeed: 100.0,
        gravity: 600.0,
        maxspeed: 320.0,
        jump_impulse: 160.0,
        player_maxspeed: 190.0,
        key_speed: 450.0,
        duck_speed: 1.0 / 3.0,
        walk_speed: 1.0,
        stand_height: 72.0,
        duck_height: 36.0,
        eye_stand: 64.0,
        eye_duck: 28.0,
        air_duck_shift: 1.0,
        jump_stamina: 0.0,
        enable_bunnyhopping: true,
        bunnyhop_cap: 0.0,
        step_min_speed: 90.0,
        fall_safe: 526.5,
        fall_damage_per_speed: 100.0 / (922.5 - 526.5),
        fall_punch: 303.0,
        fall_half_safe: 263.25,
        fall_bounce: 173.0,
        fall_fixed: Some(FALL_FIXED),
        fall_fraction: true,
        contents_ladders: false,
        useable_ladders: true,
        ladder_use_only: false,
        ladder_auto_dismount: true,
        swim_lift_max_speed: true,
        model_footsteps: true,
        drowning: false,
        ..SourceMovementConfig::shared_code()
    }
}

/// `mp_falldamage` 0's flat damage, health points.
pub const FALL_FIXED: f32 = 10.0;

/// The HL2-only movement cvars (movement.md "Cvars and configs"), under
/// `hl2dm:`: (name, help).
pub const CVARS: &[(&str, &str)] = &[
    ("hl2_walkspeed", "+walk speed (150)"),
    ("hl2_normspeed", "run speed (190)"),
    ("hl2_sprintspeed", "+speed sprint speed (320)"),
    ("mp_falldamage", "1: fall damage by speed, else a flat 10"),
    ("sv_ladder_useonly", "1: only +use mounts a ladder"),
    ("sv_autoladderdismount", "1: a ladder's ends dismount by themselves"),
];

/// `hl2dm:movement`, and the match's movement.
pub struct Hl2MovementPlugin;

impl Plugin for Hl2MovementPlugin {
    fn build(&self, app: &mut App) {
        add_movement(app);
        app.insert_resource(Loadout { movement: ID });
    }
}

/// `hl2dm:movement` without making it the match's (a game of several
/// games: `super::party_sim`): Source's movement with HL2:DM's config, its
/// speed selection and its cvars. The suit (`super::suit::SuitPlugin`)
/// goes with it.
pub fn add_movement(app: &mut App) {
    {
        app.source_movement(ID, Some(config()), Some(super::GAME))
            .init_resource::<Speeds>()
            .add_systems(
                crate::core::Predict::Movement,
                select_speed.after(super::suit::breath_devices).before(SourceMovementStep),
            );
        for (name, help) in CVARS {
            let name = *name;
            crate::console::ConsoleAppExt::console_cvar(
                app,
                &format!("{}:{name}", super::GAME),
                help,
                &get_cvar(app.world(), name).unwrap_or_default(),
                move |w| get_cvar(w, name),
                move |w, v| set_cvar(w, name, v),
            );
        }
    }
}

fn get_cvar(w: &World, name: &str) -> Option<String> {
    let speeds = w.get_resource::<Speeds>().copied().unwrap_or_default();
    let cfg = w.get_resource::<SourceMovementConfigs>().and_then(|c| c.get(ID)).cloned()?;
    let b = |v: bool| if v { "1" } else { "0" }.to_string();
    Some(match name {
        "hl2_walkspeed" => speeds.walk.to_string(),
        "hl2_normspeed" => speeds.run.to_string(),
        "hl2_sprintspeed" => speeds.sprint.to_string(),
        "mp_falldamage" => b(cfg.fall_fixed.is_none()),
        "sv_ladder_useonly" => b(cfg.ladder_use_only),
        "sv_autoladderdismount" => b(cfg.ladder_auto_dismount),
        _ => return None,
    })
}

fn set_cvar(w: &mut World, name: &str, value: &str) -> Result<(), String> {
    let num = value
        .trim()
        .trim_matches('"')
        .parse::<f32>()
        .map_err(|_| format!("{name}: not a number: {value}"))?;
    if name.starts_with("hl2_") {
        let mut s = w.get_resource_or_insert_with(Speeds::default);
        match name {
            "hl2_walkspeed" => s.walk = num,
            "hl2_normspeed" => s.run = num,
            _ => s.sprint = num,
        }
        return Ok(());
    }
    let mut configs = w.get_resource_mut::<SourceMovementConfigs>().ok_or("no HL2:DM movement")?;
    let cfg = configs.get_mut(ID).ok_or("no HL2:DM movement")?;
    match name {
        "mp_falldamage" => cfg.fall_fixed = (num != 1.0).then_some(FALL_FIXED),
        "sv_ladder_useonly" => cfg.ladder_use_only = num != 0.0,
        _ => cfg.ladder_auto_dismount = num != 0.0,
    }
    Ok(())
}

/// HL2:DM's movement config as registered (tests change it here).
pub fn config_mut(world: &mut World) -> Mut<'_, SourceMovementConfig> {
    world
        .resource_mut::<SourceMovementConfigs>()
        .map_unchanged(|c| c.get_mut(ID).expect("hl2dm:movement registered"))
}

/// Walk, run or sprint, and the sprint's aux power (movement.md "Speed
/// selection", "Sprint and aux power, per tick"; suit.md "Sprint"), first
/// in each movement tick, predicted: the sprint starts only on a tick
/// `+speed` is pressed, with the suit, not fully ducked, eyes out of the
/// water and at least 10 aux power (else `HL2Player.SprintNoPower`), and
/// stops when `+speed` is let go or the aux power runs out; walking needs
/// `+walk`, no sprint and no duck. The sprint device drains 25/s while
/// the tick started moving; switching it off costs 2.5. A dead player's
/// suit is left as it was (its devices stay on until the respawn: suit.md
/// Quirks, measured).
#[allow(clippy::type_complexity)]
pub fn select_speed(
    mut q: Query<
        (
            Entity,
            &Intent,
            &SourceMovement,
            &Velocity,
            &mut super::suit::Suit,
            &mut SourceSpeed,
            &Transform,
            &MovementSlot,
        ),
        Without<crate::core::Dead>,
    >,
    speeds: Option<Res<Speeds>>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    infinite: Option<Res<super::suit::InfiniteAux>>,
    mut play: MessageWriter<PlaySound>,
) {
    let speeds = speeds.map_or_else(Speeds::default, |s| *s);
    let dt = clock.dt();
    let infinite = infinite.is_some_and(|i| i.0);
    for (entity, intent, me, vel, mut suit, mut speed, t, slot) in &mut q {
        if slot.0 != ID {
            continue;
        }
        let mut sound = |entry: &str| {
            if first.0 {
                play.write(PlaySound {
                    entry: entry.into(),
                    at: Some(t.translation),
                    volume: None,
                    pitch: None,
                    source: Some(entity),
                    channel: None,
                });
            }
        };
        let changed = intent.sprint != suit.speed_held;
        let fully_ducked = me.ducked && !me.ducking;
        let allowed = suit.worn && !fully_ducked && me.water_level != 3;
        let wanted = intent.sprint && allowed;
        let mut started = false;
        if changed && wanted != suit.sprinting {
            if wanted {
                if suit.aux < SPRINT_START {
                    sound("HL2Player.SprintNoPower");
                } else {
                    suit.sprinting = true;
                    started = true;
                }
            } else {
                suit.sprinting = false;
            }
        }
        if suit.aux < SPRINT_STOP {
            suit.sprinting = false;
        }
        let walk_key = intent.walk && !intent.sprint;
        let walking = walk_key && !suit.sprinting && !intent.crouch || !suit.worn;
        let p = if suit.sprinting {
            if started {
                sound("HL2Player.SprintStart");
            }
            speeds.sprint
        } else if walking {
            speeds.walk
        } else {
            speeds.run
        };
        if speed.0 != p {
            speed.0 = p;
        }
        // The sprint device and the tick's aux power (tick-start velocity,
        // Source units).
        let now = clock.tick;
        if p == speeds.sprint {
            suit.attach(super::suit::SPRINT);
        } else {
            suit.detach(super::suit::SPRINT, now, infinite);
        }
        let v = vel.0 / 0.0254;
        // Engine x and z are Source x and y.
        let moving = v.x.abs() > super::suit::STILL || v.z.abs() > super::suit::STILL;
        suit.update_aux(moving, now, dt, infinite);
        suit.speed_held = intent.sprint;
    }
}
