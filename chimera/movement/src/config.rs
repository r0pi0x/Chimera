//! The movement's config (`QuakeConfig`: the cvars' values, the box, the
//! duck, the falls, each game's switches), the shared code's values every
//! game's starts from (`reference`; no game's are the defaults), the console
//! variables by name (`QUAKE_CVARS`, `set_cvar`, `get_cvar`), the configs by
//! movement id (`QuakeConfigs`), each character's fitted to the
//! loaded map's envelope (`MovementConfigs`, `config_of`).

use bevy::prelude::*;

use crate::{mover::LEAVE_GROUND_VZ, slots::MovementSlot, units::METERS_PER_UNIT};

/// Console variables and per-player values the movement reads. No game's
/// are its defaults: `reference` is the movement spec's shared-code values
/// (the SDK's, which the spec's test cases use), and each game states its
/// own over them (`cs_source::movement::config`, CS:S as measured;
/// HL2:DM's, TF2's, DoD:S's), registered by its movement's id
/// (`QuakeConfigs`).
#[derive(Clone, Debug)]
pub struct QuakeConfig {
    pub accelerate: f32,
    pub airaccelerate: f32,
    pub friction: f32,
    pub stopspeed: f32,
    pub gravity: f32,
    pub maxspeed: f32,
    pub stepsize: f32,
    pub maxvelocity: f32,
    pub bounce: f32,
    /// Jump speed, units/s.
    pub jump_impulse: f32,
    /// Max speed without a weapon's `MaxSpeed` (the knife's 250).
    pub player_maxspeed: f32,
    /// What a full move key sends (cl_forwardspeed etc.), rescaled to the max speed.
    pub key_speed: f32,
    /// Move input scale while ducked on the ground.
    pub duck_speed: f32,
    /// Collision box heights and eye heights above the feet, units.
    pub stand_height: f32,
    pub duck_height: f32,
    pub eye_stand: f32,
    pub eye_duck: f32,
    /// How much of the height difference the feet move when ducking or
    /// unducking in the air (1 = head stays put).
    pub air_duck_shift: f32,
    /// CS:S jump stamina, ms: set by each jump, counting down in real time.
    /// While it lasts, jump speed and ground speed are scaled down by
    /// 1 - coefficient x stamina. 0 turns it off (the shared code has none).
    pub jump_stamina: f32,
    pub stamina_jump_scale: f32,
    pub stamina_ground_scale: f32,
    /// sv_enablebunnyhopping: when false, a jump caps the player's speed
    /// (3D) at `bunnyhop_cap`.
    pub enable_bunnyhopping: bool,
    pub bunnyhop_cap: f32,
    /// sv_autobunnyhopping: holding jump jumps again on landing.
    pub auto_bunnyhopping: bool,
    /// Max speed scale while the walk key (+speed) is held: CS:S walks at
    /// 0.52 x the weapon speed (130 with the knife), on the ground, in the
    /// air and in water; the duck scale applies on top (measured with
    /// movecmp).
    pub walk_speed: f32,
    /// sv_ladder_dampen / sv_ladder_angle (CS:S): sideways ladder input is
    /// scaled by `ladder_dampen` when the push is mostly into the ladder
    /// (the angle test's dot below `ladder_angle`). Dampen 1 turns it off.
    pub ladder_dampen: f32,
    pub ladder_angle: f32,
    /// The player's own max speed property, used by swimming's upward
    /// push (holding jump, looking up). CS:S: 260, the base player speed,
    /// whatever the weapon (measured with movecmp fuzz).
    pub client_maxspeed: f32,
    /// CS:S: ladder speed gets the same duck scale as move input (see
    /// `duck_slows_everywhere`; measured; the shared code leaves climbing
    /// speed alone).
    pub duck_slows_ladder: bool,
    /// The slowest a step plays at, off ladders (the shared code's walk
    /// band starts at 90; CS:S's at 150, measured: 140 silent, 150 steps,
    /// so walking (130) and ducking (85) are silent by speed, not by key;
    /// specs/cs_source/sounds.md "Footsteps").
    pub step_min_speed: f32,
    /// CS:S: the ducked input scale applies while duck is held, or when the
    /// tick started ducked or mid-duck: from the tick duck is pressed until
    /// the tick after it's released, in the air and water too (measured
    /// with movecmp fuzz). The shared code scales only when already ducked
    /// on the ground.
    pub duck_slows_everywhere: bool,
    /// CS:S: releasing duck partway down leaves the duck timer running, so
    /// the unduck has as long behind it as the duck had (released 105 ms
    /// in, the eye drops to the unduck curve's 105 ms point, 56.1, and is
    /// up 7 ticks later; measured with movecmp `duck_release_7`). The
    /// shared code starts the unduck from the matching point instead.
    pub unduck_keeps_duck_timer: bool,
    /// CS:S: a jump pressed on the ground while standing up from a duck
    /// (refused, as in the shared code) still sets the jump stamina, drops
    /// the eye to the ducked height, and ends the unduck on the next tick
    /// (measured with movecmp `unduck_jump_5`/`_10`). The shared code just
    /// ignores the press.
    pub unduck_jump_ducks: bool,
    /// CS:S: a move stopped by a surface leaves this much velocity (1/32
    /// unit/s) away from it, so a player pressed along a wall settles 0.0317
    /// from it, not at the 1/32 back-off (measured with movecmp
    /// `wall_slide`: 0.0312 off the wall every tick). 0 in the shared code.
    pub clip_push_out: f32,
    /// Fall damage (specs/cs_source/fall_damage.md): landing faster than
    /// `fall_safe` (units/s) out of water deals (speed - safe) x
    /// `fall_damage_per_speed` health points, truncated to whole points.
    pub fall_safe: f32,
    pub fall_damage_per_speed: f32,
    /// sv_footsteps: false silences steps, jump and landing sounds
    /// (sounds.md 3; the timers still run).
    pub footsteps: bool,
    /// Landing effects' thresholds (units/s): none below `fall_punch`;
    /// above `fall_safe` full volume (and damage), above `fall_half_safe`
    /// 0.85, below `fall_bounce` silent, else 0.5 (CS:S: the shared code's
    /// 350, 580, 290, 200; HL2's set scaled for gravity 600: 303, 526.5,
    /// 263.25, 173).
    pub fall_punch: f32,
    pub fall_half_safe: f32,
    pub fall_bounce: f32,
    /// Full landing volume above this instead of `fall_safe` (TF2: the
    /// shared 580 while its damage starts at 650); None: `fall_safe`.
    pub fall_loud: Option<f32>,
    /// Landing sound and view punch only at full volume (TF2's Scout:
    /// above 580; usually per character, `MoveTraits`).
    pub loud_landings_only: bool,
    /// Taken off the fall speed of a landing out of water on a floating
    /// body (`map::buoyancy::Floating`; CS:S 200, HL2 173: movement.md
    /// "Falling", specs/hl2dm/movement.md "Falling and landing").
    pub floating_bonus: f32,
    /// A damaging landing costs exactly this (HL2:DM's `mp_falldamage` 0:
    /// 10); None: by speed (`fall_damage_per_speed`).
    pub fall_fixed: Option<f32>,
    /// HL2's damage by speed: the fraction is kept (the health's
    /// remainder takes it) and the landing roll uses the fall speed less
    /// `fall_safe` (specs/hl2dm/movement.md "Falling and landing"); CS:S
    /// truncates and rolls by the whole fall.
    pub fall_fraction: bool,
    /// Brushes with ladder contents are climbed (CS:S). HL2's games climb
    /// `func_useableladder`s instead (`useable_ladders`) and contents
    /// ladders are solid like any brush; a game with no ladders (TF2)
    /// climbs none on its own maps. On another game's map every movement
    /// climbs (`QuakeConfig::climbs_contents`).
    pub contents_ladders: bool,
    /// `func_useableladder`s are climbed (`useable_ladders`; HL2:DM), with
    /// `sv_ladder_useonly` (only `+use` mounts) and
    /// `sv_autoladderdismount` (the ends dismount by themselves).
    pub useable_ladders: bool,
    pub ladder_use_only: bool,
    pub ladder_auto_dismount: bool,
    /// Swimming's upward push is the player's max speed (HL2: 190, 150 or
    /// 320), not `client_maxspeed`.
    pub swim_lift_max_speed: bool,
    /// Footsteps are the character's model's (`StepSounds`: HL2:DM's run
    /// steps, whatever the surface) on the shared timing, silent while
    /// ducked; else the surface's (CS:S).
    pub model_footsteps: bool,
    /// CS:S's drowning (`drown`); a game with its own (HL2:DM's suit)
    /// turns it off.
    pub drowning: bool,
    /// Steps in waist-deep water (level 2) skip one attempt in four, on a
    /// counter every player shares (HL2's step timing: the step comes a
    /// tick later; specs/hl2dm/movement.md "Step sounds").
    pub waist_step_skip: bool,
    /// Half the box's width, units (16 in the shared code and CS:S; TF2's
    /// 24: specs/tf2/movement.md).
    pub half_width: f32,
    /// How long a duck on the ground takes, s (the shared code's 0.4; the
    /// unduck is 0.2 either way; TF2 0.2).
    pub duck_time: f32,
    /// A duck's or unduck's timer counts the tick of the press (or
    /// release) already: tick k of the transition is k ticks in (TF2,
    /// measured: the eye 64.68 on the press tick of a Scout's duck, the box
    /// switched on the 14th); the shared code's first tick is 0 in.
    pub duck_counts_first_tick: bool,
    /// The unduck's timer alone counts the tick of the release (DoD:S,
    /// measured: the eye 34.385 on the release tick, the standing box on
    /// the 14th, 0.21 s; its duck doesn't: specs/dods/movement.md
    /// "Getting up").
    pub unduck_counts_first_tick: bool,
    /// A jump caps the speed (3D) at this times the player's max speed P
    /// (TF2's 1.2: the cap follows the class, a weapon's lower speed too;
    /// not reduced by the surface or the ceiling); 0: `bunnyhop_cap` alone.
    pub bunnyhop_cap_scale: f32,
    /// TF2's backpedal clamp (specs/tf2/movement.md "Backwards"): on the
    /// ground after acceleration, a velocity above `back_min_speed` that
    /// points behind the view has its part along the flat view-forward
    /// capped at this x the max speed (the sideways part kept), then the
    /// whole clamped to the max speed; nothing in the air or in water,
    /// the input untouched. 1 (the shared code): no clamp.
    pub back_speed: f32,
    pub back_min_speed: f32,
    /// On every grounded walking tick the horizontal speed is clamped to
    /// the max speed after acceleration (TF2; the shared code has none).
    pub ground_clamp: bool,
    /// Turbo physics (`sv_turbophysics 1`, DoD:S's shipped config:
    /// specs/dods/physics_props.md): no physics shadow (`shadow`), so a
    /// `prop_physics` of any mass is a wall; a multiplayer prop is the
    /// push-away's soft prop (`pushaway`) only while in the push-away
    /// group (asleep since it spawned, or moving), a wall once settled in
    /// "solid" mode; debris and settled non-solid ones are walked through
    /// (`chimera_world::map::prop_physics::PropGroup::live`). Off (the
    /// shared code, CS:S, HL2:DM, TF2): the shadow, every multiplayer prop
    /// soft.
    pub turbo_physics: bool,
    /// A wish speed under 100 x friction x surface friction / accelerate
    /// gets a raised acceleration that beats friction (TF2: a ducked spun
    /// Heavy still moves).
    pub low_wish_accel: bool,
    /// Rising faster than this (units/s) leaves the ground without the
    /// ground probe (the shared code's 140; TF2's 250, so its 0.25 surface
    /// friction applies while rising up to 250).
    pub leave_ground_vz: f32,
    /// After duck is released the duck key counts as not held on the
    /// ground for this long, s (TF2's 0.3), and a duck press during an
    /// unduck transition is ignored (the unduck goes on); 0: neither.
    pub reduck_delay: f32,
    /// No duck while the feet are in water off the ground or the eyes are
    /// under water (TF2).
    pub no_water_duck: bool,
    /// Jumps a player may make in the air before landing again (TF2's
    /// Scout: 1; usually set per character, `MoveTraits`), each setting
    /// the vertical speed to `air_jump_speed` and the horizontal velocity
    /// to the keys' wish velocity.
    pub air_jumps: u8,
    pub air_jump_speed: f32,
    /// No ground jump while fully ducked (TF2: a crouch jump is a jump,
    /// then a duck in the air).
    pub no_ducked_jump: bool,
    /// The air duck limit (TF2's `tf_clamp_airducks`: 2): each release of
    /// duck in the air counts one, and once this many are counted the duck
    /// key counts as not held in the air (until landing or an air jump);
    /// 0: no limit.
    pub air_ducks_max: u8,
    /// Fall damage as a share of max health per unit/s of landing speed
    /// above `fall_safe` (TF2: max health x speed / 6000, rounded); 0: by
    /// `fall_damage_per_speed`. `fall_spread`: the damage x a random factor
    /// in 1 +- this (TF2's 0.2; `tf_fall_damage_disablespread` 1 sets 0).
    pub fall_by_max_health: f32,
    pub fall_spread: f32,
    /// The sound entries the movement plays itself: a swim stroke and a
    /// splash into or out of water, and getting on a ladder line (Source's
    /// `Player.Swim` and `Ladder.StepRight`).
    pub swim_sound: &'static str,
    pub ladder_mount_sound: &'static str,
    /// The surfaces whose step sounds stand for a ladder's rungs, wading
    /// knee-deep and feet in water (Source's surface properties `ladder`,
    /// `wade` and `water`).
    pub ladder_surface: &'static str,
    pub wade_surface: &'static str,
    pub water_surface: &'static str,
    /// The channel the movement's sounds play on (footsteps, swim: a new
    /// one replaces the last; Source's body channel, 4).
    pub sound_channel: u8,
    /// The map outputs a ladder line's entity fires when a player gets on
    /// and off it (Source's `func_useableladder`).
    pub ladder_on_output: &'static str,
    pub ladder_off_output: &'static str,
    /// How fast a hit's velocity modifier (`QuakeMovement::
    /// velocity_modifier`) climbs back to 1, per second (CS:S, weapons.md
    /// M18: +0.005 a 0.015 s tick).
    pub velocity_modifier_recovery: f32,
    /// The world is traced through the map's clip hulls where it has them
    /// (`trace::hulls`: GoldSrc maps), the standing box through its hull
    /// and the ducked box through its own; on every other map, and for a
    /// box no hull is made for, the box sweep. Off (Source's games): the
    /// box sweep everywhere.
    pub clip_hulls: bool,
    /// A tick is a command of whole milliseconds (GoldSrc: one step per
    /// client command, its length a whole number of milliseconds, at most
    /// 255): the tick's time is cut to whole milliseconds and what is left
    /// over is carried to the next tick (`QuakeMovement::msec_carry`,
    /// predicted). Nothing changes at a tick of whole milliseconds (0.01 s,
    /// 0.015 s); at 1/66 s the steps are 15 ms with a 16 ms one every 6 or
    /// 7. Off (Source's games): the tick's own time.
    pub command_msec: bool,
    /// Edge friction (GoldSrc's `edgefriction`: 2): the ground friction
    /// times this while the box's way ahead ends in a drop
    /// (`Mover::at_edge`: a trace of the box from 16 units ahead along the
    /// horizontal velocity, its centre at the feet's height, 34 units
    /// straight down, meets nothing; starting in solid counts as a hit).
    /// 1 (Source's games): none, and no probe.
    pub edge_friction: f32,
}

/// The console variables every game of the Quake lineage knows, mapped
/// onto the config: (name, what it sets). A registration binds these and
/// its engine's own (`QuakeMovementAppExt::quake_movement`'s lists:
/// Source's are `source_movement::SOURCE_CVARS`); `set_cvar` and
/// `get_cvar` know every list's names.
pub const QUAKE_CVARS: &[(&str, &str)] = &[
    ("sv_accelerate", "ground acceleration"),
    ("sv_airaccelerate", "air acceleration (surf servers raise it, e.g. 150)"),
    ("sv_friction", "ground friction"),
    ("sv_stopspeed", "speed below which friction stops at a constant rate"),
    ("sv_gravity", "gravity, units/s^2"),
    ("sv_maxspeed", "upper limit on move speed"),
    ("sv_stepsize", "highest step climbed without jumping"),
    ("sv_maxvelocity", "per-axis velocity limit"),
    ("sv_bounce", "wall bounce (0 in CS:S)"),
];

impl QuakeConfig {
    /// Whether ladder contents (and ladder zones, and other games'
    /// ladder lines as posts) are climbed on this map: the game's own
    /// rule on its own maps; on another game's map (`foreign`) any
    /// movement that climbs no ladder lines climbs them, so a game
    /// without ladders (TF2) still climbs every other game's
    /// (docs/DESIGN.md "Ladders across games"), at the shared code's
    /// climb speed (200 units/s, specs/cs_source/movement.md).
    #[inline]
    pub fn climbs_contents(&self, foreign: bool) -> bool {
        self.contents_ladders || (foreign && !self.useable_ladders)
    }

    /// Set a console variable by name, as a server config would.
    pub fn set_cvar(&mut self, name: &str, value: &str) -> Result<(), String> {
        let num = || {
            value
                .trim()
                .trim_matches('"')
                .parse::<f32>()
                .map_err(|_| format!("{name}: not a number: {value}"))
        };
        match name.to_ascii_lowercase().as_str() {
            "sv_accelerate" => self.accelerate = num()?,
            "sv_airaccelerate" => self.airaccelerate = num()?,
            "sv_friction" => self.friction = num()?,
            "sv_stopspeed" => self.stopspeed = num()?,
            "sv_gravity" => self.gravity = num()?,
            "sv_maxspeed" => self.maxspeed = num()?,
            "sv_stepsize" => self.stepsize = num()?,
            "sv_maxvelocity" => self.maxvelocity = num()?,
            "sv_bounce" => self.bounce = num()?,
            "edgefriction" => self.edge_friction = num()?,
            "sv_enablebunnyhopping" => self.enable_bunnyhopping = num()? != 0.0,
            "sv_autobunnyhopping" => self.auto_bunnyhopping = num()? != 0.0,
            "sv_footsteps" => self.footsteps = num()? != 0.0,
            "cl_forwardspeed" | "cl_sidespeed" => self.key_speed = num()?,
            "sv_ladder_dampen" => self.ladder_dampen = num()?,
            "sv_ladder_angle" => self.ladder_angle = num()?,
            other => return Err(format!("unknown console variable {other}")),
        }
        Ok(())
    }

    /// A console variable's value, as `set_cvar` reads it.
    pub fn get_cvar(&self, name: &str) -> Option<String> {
        let b = |v: bool| if v { "1" } else { "0" }.to_string();
        let f = |v: f32| {
            let s = format!("{v}");
            if s.contains('.') {
                s.trim_end_matches('0').trim_end_matches('.').to_string()
            } else {
                s
            }
        };
        Some(match name.to_ascii_lowercase().as_str() {
            "sv_accelerate" => f(self.accelerate),
            "sv_airaccelerate" => f(self.airaccelerate),
            "sv_friction" => f(self.friction),
            "sv_stopspeed" => f(self.stopspeed),
            "sv_gravity" => f(self.gravity),
            "sv_maxspeed" => f(self.maxspeed),
            "sv_stepsize" => f(self.stepsize),
            "sv_maxvelocity" => f(self.maxvelocity),
            "sv_bounce" => f(self.bounce),
            "edgefriction" => f(self.edge_friction),
            "sv_enablebunnyhopping" => b(self.enable_bunnyhopping),
            "sv_autobunnyhopping" => b(self.auto_bunnyhopping),
            "sv_footsteps" => b(self.footsteps),
            "cl_forwardspeed" | "cl_sidespeed" => f(self.key_speed),
            "sv_ladder_dampen" => f(self.ladder_dampen),
            "sv_ladder_angle" => f(self.ladder_angle),
            _ => return None,
        })
    }

    /// Apply a Source-style config (one `name value` per line, `//`
    /// comments); unknown variables are reported, not fatal.
    pub fn exec(&mut self, text: &str) -> Vec<String> {
        let mut problems = Vec::new();
        for line in text.lines() {
            let line = line.split("//").next().unwrap_or("").trim();
            let mut parts = line.splitn(2, char::is_whitespace);
            let (Some(name), Some(value)) = (parts.next(), parts.next()) else {
                continue;
            };
            if let Err(e) = self.set_cvar(name, value) {
                problems.push(e);
            }
        }
        problems
    }

    /// Feet movement for a duck or unduck in the air.
    #[inline]
    pub(crate) fn air_duck_lift(&self) -> f32 {
        (self.stand_height - self.duck_height) * self.air_duck_shift
    }

    /// This config's character as a movement envelope
    /// (`chimera_core::envelope`, metres): the box's half width, heights,
    /// step and eyes (CS:S's eye, 64 u, is over its 62 u box).
    pub fn envelope(&self) -> chimera_core::envelope::MovementEnvelope {
        let u = METERS_PER_UNIT;
        chimera_core::envelope::MovementEnvelope {
            radius: self.half_width * u,
            height: self.stand_height * u,
            crouch_height: self.duck_height * u,
            step: self.stepsize * u,
            eye: self.eye_stand * u,
            crouch_eye: self.eye_duck * u,
        }
    }

    /// This character's movement body on a map whose envelope is `map`
    /// (the envelope rule, `MovementEnvelope::within`, shrink only): the
    /// box narrower and lower (standing and ducked), a lowered body's eyes
    /// `EYE_MARGIN` under its top or at the map's character's eyes,
    /// whichever is higher (never above its own), the step raised to the map's
    /// character's. The model and hit boxes are not here and stay the
    /// character's own. Itself, borrowed, where nothing changes (its own
    /// game's maps, no map envelope).
    pub fn fitted(&self, map: Option<&chimera_core::envelope::MovementEnvelope>) -> std::borrow::Cow<'_, Self> {
        let own = self.envelope();
        let b = own.within(map);
        if b == own {
            return std::borrow::Cow::Borrowed(self);
        }
        let u = METERS_PER_UNIT;
        let mut c = self.clone();
        c.half_width = b.fit_half_widths([self.half_width; 2], u)[0];
        c.stand_height = self.stand_height.min(b.height / u);
        c.duck_height = self.duck_height.min(b.crouch_height / u).min(c.stand_height);
        c.stepsize = self.stepsize.max(b.step / u);
        if c.stand_height < self.stand_height {
            c.eye_stand = self.eye_stand.min(b.eye / u);
        }
        let eye_duck = if c.duck_height < self.duck_height { self.eye_duck.min(b.crouch_eye / u) } else { self.eye_duck };
        c.eye_duck = eye_duck.min(c.eye_stand);
        std::borrow::Cow::Owned(c)
    }
}

impl QuakeConfig {
    /// The SDK 2013 shared movement code's values: the reference every
    /// game's config starts from.
    pub fn reference() -> Self {
        Self {
            accelerate: 5.0,
            airaccelerate: 10.0,
            friction: 4.0,
            stopspeed: 100.0,
            gravity: 800.0,
            maxspeed: 320.0,
            stepsize: 18.0,
            maxvelocity: 3500.0,
            bounce: 0.0,
            jump_impulse: 268.328_16,
            player_maxspeed: 250.0,
            key_speed: 450.0,
            duck_speed: 1.0 / 3.0,
            walk_speed: 0.52,
            stand_height: 72.0,
            duck_height: 36.0,
            eye_stand: 64.0,
            eye_duck: 28.0,
            air_duck_shift: 1.0,
            jump_stamina: 0.0,
            stamina_jump_scale: 0.0,
            stamina_ground_scale: 0.0,
            enable_bunnyhopping: true,
            bunnyhop_cap: 0.0,
            auto_bunnyhopping: false,
            // The dampening is compiled only for CS:S; it's off here.
            ladder_dampen: 1.0,
            ladder_angle: -0.707,
            client_maxspeed: 250.0,
            step_min_speed: 90.0,
            duck_slows_ladder: false,
            duck_slows_everywhere: false,
            unduck_keeps_duck_timer: false,
            unduck_jump_ducks: false,
            clip_push_out: 0.0,
            fall_safe: 580.0,
            fall_damage_per_speed: 100.0 / (1024.0 - 580.0),
            footsteps: true,
            fall_punch: 350.0,
            fall_half_safe: 290.0,
            fall_bounce: 200.0,
            fall_loud: None,
            loud_landings_only: false,
            floating_bonus: 200.0,
            fall_fixed: None,
            fall_fraction: false,
            contents_ladders: true,
            useable_ladders: false,
            ladder_use_only: false,
            ladder_auto_dismount: true,
            swim_lift_max_speed: false,
            model_footsteps: false,
            drowning: true,
            waist_step_skip: false,
            half_width: 16.0,
            duck_time: 0.4,
            duck_counts_first_tick: false,
            unduck_counts_first_tick: false,
            bunnyhop_cap_scale: 0.0,
            back_speed: 1.0,
            back_min_speed: 100.0,
            ground_clamp: false,
            turbo_physics: false,
            low_wish_accel: false,
            leave_ground_vz: LEAVE_GROUND_VZ,
            reduck_delay: 0.0,
            no_water_duck: false,
            air_jumps: 0,
            air_jump_speed: 0.0,
            no_ducked_jump: false,
            air_ducks_max: 0,
            fall_by_max_health: 0.0,
            fall_spread: 0.0,
            swim_sound: "Player.Swim",
            ladder_mount_sound: "Ladder.StepRight",
            ladder_surface: "ladder",
            wade_surface: "wade",
            water_surface: "water",
            sound_channel: 4,
            ladder_on_output: "OnPlayerGotOnLadder",
            ladder_off_output: "OnPlayerGotOffLadder",
            velocity_modifier_recovery: 1.0 / 3.0,
            clip_hulls: false,
            command_msec: false,
            edge_friction: 1.0,
        }
    }
}

/// The configs of the registered Source movements
/// (`QuakeMovementAppExt::quake_movement`), by movement id; and the
/// reference values, for a character where none is registered
/// (`fallback`).
#[derive(Resource, Clone, Debug)]
pub struct QuakeConfigs(pub Vec<(&'static str, QuakeConfig)>, QuakeConfig);

impl Default for QuakeConfigs {
    fn default() -> Self {
        Self(Vec::new(), QuakeConfig::reference())
    }
}

impl QuakeConfigs {
    pub fn get(&self, id: &str) -> Option<&QuakeConfig> {
        self.0.iter().find(|(i, _)| *i == id).map(|(_, c)| c)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut QuakeConfig> {
        self.0.iter_mut().find(|(i, _)| *i == id).map(|(_, c)| c)
    }

    /// The config of a character on no registered movement: the first
    /// registered one's (the app's own game's), else the reference values.
    pub fn fallback(&self) -> &QuakeConfig {
        self.0.first().map_or(&self.1, |(_, c)| c)
    }

    /// Movement `id`'s config set to `config`, registered or not yet (a
    /// registration after it keeps it: an app's settings read before its
    /// plugins are built).
    pub fn set(&mut self, id: &'static str, config: QuakeConfig) {
        match self.get_mut(id) {
            Some(c) => *c = config,
            None => self.0.push((id, config)),
        }
    }
}

/// Each Source character's config: its movement's
/// (`QuakeConfigs`, by its `MovementSlot`); for a character on no
/// registered Source movement the first registered one's (the app's own
/// game's), else the reference values.
/// Fitted to the loaded map's movement envelope (`QuakeConfig::fitted`).
#[derive(bevy::ecs::system::SystemParam)]
pub struct MovementConfigs<'w> {
    own: Res<'w, QuakeConfigs>,
    envelopes: Option<Res<'w, chimera_core::envelope::MovementEnvelopes>>,
    map_game: Option<Res<'w, crate::core::MapGame>>,
}

impl MovementConfigs<'_> {
    /// The config of a character on movement `slot`, its body fitted to
    /// the loaded map (`map`).
    pub fn of(&self, slot: Option<&MovementSlot>) -> std::borrow::Cow<'_, QuakeConfig> {
        self.own(slot).fitted(self.map().as_ref())
    }

    /// The config of movement `slot` as its game states it (not fitted).
    pub fn own(&self, slot: Option<&MovementSlot>) -> &QuakeConfig {
        slot.and_then(|s| self.own.get(s.0)).unwrap_or_else(|| self.own.fallback())
    }

    /// The loaded map's movement envelope (None: no fitting).
    pub fn map(&self) -> Option<chimera_core::envelope::MovementEnvelope> {
        self.envelopes.as_deref()?.for_map(self.map_game.as_deref())
    }
}

/// `MovementConfigs::of` from the world (fitted to the loaded map).
pub fn config_of(world: &World, e: Entity) -> QuakeConfig {
    let slot = world.get::<MovementSlot>(e);
    let reference;
    let own = match world.get_resource::<QuakeConfigs>() {
        Some(all) => slot.and_then(|s| all.get(s.0)).unwrap_or_else(|| all.fallback()),
        None => {
            reference = QuakeConfig::reference();
            &reference
        }
    };
    own.fitted(chimera_core::envelope::map_envelope(world).as_ref()).into_owned()
}

pub(crate) const HALF_WIDTH: f32 = 16.0;
