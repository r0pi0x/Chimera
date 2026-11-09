//! HL2:DM's movement (`hl2dm:movement`, specs/hl2dm/movement.md, H1):
//! Source's player movement (`source_engine::source_movement`, lift S6)
//! with HL2:DM's numbers (`config`: gravity 600, a fixed 160 jump, hulls
//! 72/36 with eyes 64/28 and the whole 36-unit duck lift in the air, a
//! third of the input ducked, accelerate 10, stopspeed 100, keys 450, no
//! stamina and no bunny-hop cap), HL2's fall damage (thresholds 303,
//! 526.5, 922.5; `mp_falldamage` 0: a flat 10, 1: by speed with the
//! fraction kept), `func_useableladder`s instead of ladder brushes, the
//! swim lift of the player's own speed, footsteps of the model's own run
//! steps (none while ducked; waist deep, one attempt in four on a counter
//! all players share is put off a tick), landings counting 173 less on a
//! floating body and less a descending ground's speed, and the speed
//! selection (`select_speed`): run 190, `+walk` 150 (its own action,
//! `WALK_ACTION`: Chimera's shared `Intent::walk` is `+speed`'s too, so
//! Alt and Shift with no aux power walk), `+speed` sprint 320 on the
//! suit's aux power (`super::suit`), started only on the key's press; the
//! ground material's max speed and jump factors are the shared
//! movement's. `toggle_duck` (`DuckToggle`, predicted) ducks until toggled
//! again or, with `xc_uncrouch_on_jump` 1, a jump press. The match's
//! movement (`slots::Loadout`). Its cvars are `hl2dm:`'s
//! (`hl2dm:sv_gravity` ..., lift C8), backed by its own config, apart from
//! CS:S's.
//!
//! Known shortcuts:
//! - What un-toggles `toggle_duck` is the jump key's press (the spec says
//!   "a jump"), and its duck holds the duck key in the intent; the waist
//!   counter is per process, so a client predicting its own steps counts
//!   only its own.
//! - Not modelled: the ground probe ignoring hostile combine balls (none
//!   in HL2:DM), `trigger_vphysics_motion` blocking jumps (no stock map
//!   has one).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use source_engine::source_movement::{
    SourceMovement, SourceMovementAppExt, SourceMovementConfig, SourceMovementConfigs, SourceMovementStep, SourceSpeed,
};

use crate::{
    core::{ActionRegistry, FirstTimePredicted, Intent, SimClock, Velocity},
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
        floating_bonus: 173.0,
        waist_step_skip: true,
        ..SourceMovementConfig::shared_code()
    }
}

/// The walk key (`+walk`, Alt), a named action apart from Chimera's
/// shared `Intent::walk`, which `+speed` sets too: HL2:DM's walk is the
/// key itself, so Alt and Shift with no aux power walk.
pub const WALK_ACTION: &str = "walk";

/// `xc_uncrouch_on_jump`: a jump un-toggles `toggle_duck`.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct UncrouchOnJump(pub u8);

impl Default for UncrouchOnJump {
    fn default() -> Self {
        Self(1)
    }
}

/// `toggle_duck`'s state (movement.md "Jumping": the toggle style of
/// ducking, the server's): while `on` the player ducks as if `+duck` were
/// held; with `xc_uncrouch_on_jump` 1 pressing jump turns it off.
/// Predicted, so a client ducks with it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DuckToggle {
    pub on: bool,
    /// Jump was held last tick.
    pub jump_held: bool,
    /// The duck key is held for it (let go when it turns off).
    pub forced: bool,
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
        use crate::core::{ActionAppExt, PredictedAppExt};
        app.source_movement(ID, Some(config()), Some(super::GAME))
            .init_resource::<Speeds>()
            .init_resource::<UncrouchOnJump>()
            .init_resource::<ActionRegistry>()
            .intent_action(WALK_ACTION)
            .predicted_net::<DuckToggle>()
            .add_systems(
                crate::core::Predict::Movement,
                (toggled_duck, select_speed)
                    .chain()
                    .after(super::suit::breath_devices)
                    .before(SourceMovementStep),
            );
        crate::console::resource_cvar::<UncrouchOnJump, u8>(
            app,
            &format!("{}:xc_uncrouch_on_jump", super::GAME),
            "1: a jump un-toggles toggle_duck",
            |u| &mut u.0,
        );
        crate::console::ConsoleAppExt::console_command(
            app,
            &format!("{}:toggle_duck", super::GAME),
            "Duck until toggled again (or a jump, with xc_uncrouch_on_jump 1).",
            |w, _| {
                let e = crate::console::command_player(w)?;
                let mut t = w.get::<DuckToggle>(e).copied().unwrap_or_default();
                t.on = !t.on;
                w.entity_mut(e).insert(t);
                Ok(None)
            },
        );
        app.world_mut()
            .resource_mut::<crate::console::Console>()
            .player_command(&format!("{}:toggle_duck", super::GAME));
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
/// stops when `+speed` is let go, the aux power runs out or the player
/// carries something by `+use` (`carry::Carried`, predicted); walking needs
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
            Has<super::carry::Carried>,
        ),
        Without<crate::core::Dead>,
    >,
    speeds: Option<Res<Speeds>>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    infinite: Option<Res<super::suit::InfiniteAux>>,
    registry: Option<Res<ActionRegistry>>,
    mut play: MessageWriter<PlaySound>,
) {
    let speeds = speeds.map_or_else(Speeds::default, |s| *s);
    let dt = clock.dt();
    let infinite = infinite.is_some_and(|i| i.0);
    for (entity, intent, me, vel, mut suit, mut speed, t, slot, carrying) in &mut q {
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
        // Carrying by `+use` turns the sprint off (gravity_gun.md 8).
        let allowed = suit.worn && !fully_ducked && me.water_level != 3 && !carrying;
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
        if suit.aux < SPRINT_STOP || carrying {
            suit.sprinting = false;
        }
        // `+walk` (its own action), or Chimera's walk without the sprint
        // key (a bot, a game binding `+walk` alone).
        let walk_key = registry.as_ref().is_some_and(|r| intent.action(r, WALK_ACTION)) || intent.walk && !intent.sprint;
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

/// `toggle_duck` held down (`DuckToggle`), first in each movement tick:
/// its duck as `+duck`; a jump press un-toggles it with
/// `xc_uncrouch_on_jump` 1. Predicted.
pub fn toggled_duck(mut q: Query<(&mut Intent, &mut DuckToggle)>, uncrouch: Option<Res<UncrouchOnJump>>) {
    let uncrouch = uncrouch.is_none_or(|u| u.0 != 0);
    for (mut intent, mut t) in &mut q {
        let pressed = intent.jump && !t.jump_held;
        if t.jump_held != intent.jump {
            t.jump_held = intent.jump;
        }
        if t.on && pressed && uncrouch {
            t.on = false;
        }
        if t.on && !intent.crouch {
            intent.crouch = true;
            t.forced = true;
        } else if !t.on && t.forced {
            intent.crouch = false;
            t.forced = false;
        }
    }
}
