use bevy::prelude::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::*;

/// A character's command buttons as Source numbers them (the `IN_*` bits
/// map entities read and take away: game_ui, player_speedmod).
pub mod buttons {
    pub const ATTACK: u32 = 1;
    pub const JUMP: u32 = 2;
    pub const DUCK: u32 = 4;
    pub const FORWARD: u32 = 8;
    pub const BACK: u32 = 16;
    pub const USE: u32 = 32;
    pub const MOVELEFT: u32 = 512;
    pub const MOVERIGHT: u32 = 1024;
    pub const ATTACK2: u32 = 2048;
    pub const RELOAD: u32 = 8192;
    pub const SPEED: u32 = 131072;
    pub const ZOOM: u32 = 524288;

    /// The buttons an intent holds.
    pub fn of(i: &super::Intent) -> u32 {
        let mut b = 0;
        for (on, bit) in [
            (i.fire, ATTACK),
            (i.jump, JUMP),
            (i.crouch, DUCK),
            (i.move_axis.y > 0.0, FORWARD),
            (i.move_axis.y < 0.0, BACK),
            (i.use_key, USE),
            (i.move_axis.x < 0.0, MOVELEFT),
            (i.move_axis.x > 0.0, MOVERIGHT),
            (i.secondary, ATTACK2),
            (i.reload, RELOAD),
            (i.walk || i.sprint, SPEED),
        ] {
            if on {
                b |= bit;
            }
        }
        b
    }
}

/// What map entities do to a character's controls (specs/source/
/// game_entities.md 1-2, viewcontrol_and_templates.md 1): player_speedmod's
/// movement clock and taken buttons, game_ui's "at controls", a camera's
/// freeze. Applied to its `Intent` each tick before movement
/// (`apply_map_controls`, on the server and on the client predicting it:
/// a predicted part). Absent: nothing changed.
#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapControls {
    /// Its movement runs this many ticks per tick (player_speedmod's
    /// lagged movement; 0 holds it still). Reset to 1 when it spawns.
    pub time_scale: f32,
    /// Buttons taken away (`buttons` bits), per entity that took them
    /// (a key the logic gives each).
    pub disabled: Vec<(u32, u32)>,
    /// Its movement inputs are zeroed (game_ui "Freeze Player").
    pub at_controls: bool,
    /// Frozen (point_viewcontrol "Freeze Player"): no movement, buttons
    /// or impulse; the view held at these angles (yaw, pitch, radians).
    pub frozen: Option<(f32, f32)>,
    /// Its weapons put away (bits: 1 player_speedmod, 2 game_ui, 4 a
    /// camera): no firing, no view model.
    pub weapon_hidden: u8,
    /// The HUD hidden (player_speedmod "Suppress HUD").
    pub hud_hidden: bool,
    /// Takes no damage (viewing through a point_viewcontrol).
    pub invulnerable: bool,
    /// The buttons of this tick's command after the above (what game_ui
    /// reads): `buttons` bits.
    pub buttons: u32,
    /// Alive last tick (a respawn resets the movement clock).
    pub alive: bool,
    /// Parented to a map entity (SetParent: a kart's seat): the map logic
    /// carries it, its own movement doesn't run, and it neither pushes
    /// nor collides with physics bodies (its physics shadow is off).
    pub parented: bool,
}

impl Default for MapControls {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            disabled: Vec::new(),
            at_controls: false,
            frozen: None,
            weapon_hidden: 0,
            hud_hidden: false,
            invulnerable: false,
            buttons: 0,
            alive: true,
            parented: false,
        }
    }
}

impl MapControls {
    /// All buttons taken away.
    pub fn disabled_buttons(&self) -> u32 {
        self.disabled.iter().fold(0, |a, (_, b)| a | b)
    }
}

/// Apply each character's `MapControls` to its intent: taken buttons
/// released, movement zeroed at controls, everything but the held view
/// zeroed when frozen, no firing with weapons put away; the buttons left
/// recorded for the logic. A respawn resets the movement clock and brings
/// the HUD back; the taken buttons stay (CS:S, measured 2026-10-17 with a
/// bot: lagged movement 1 and the hide-all HUD bit cleared after a death
/// and respawn, jumping still taken until the speedmod gave it back;
/// game_entities.md question 3); a camera's view and freeze go too (our
/// choice, viewcontrol_and_templates.md open question 2).
pub fn apply_map_controls(
    mut q: Query<(Entity, &mut Intent, &mut MapControls, Option<&Health>, Has<MapView>)>,
    mut commands: Commands,
) {
    for (e, mut intent, mut c, health, viewing) in &mut q {
        let alive = health.is_none_or(|h| h.current > 0.0);
        if alive && !c.alive {
            c.time_scale = 1.0;
            c.hud_hidden = false;
            c.frozen = None;
            c.invulnerable = false;
            c.weapon_hidden &= !4;
            if viewing {
                commands.entity(e).remove::<MapView>();
            }
        }
        if c.alive != alive {
            c.alive = alive;
        }
        let off = c.disabled_buttons();
        let i = &mut *intent;
        if off & buttons::JUMP != 0 {
            i.jump = false;
        }
        if off & buttons::DUCK != 0 {
            i.crouch = false;
        }
        if off & buttons::USE != 0 {
            i.use_key = false;
        }
        if off & buttons::SPEED != 0 {
            i.walk = false;
            i.sprint = false;
        }
        if off & buttons::ATTACK != 0 {
            i.fire = false;
        }
        if off & buttons::ATTACK2 != 0 {
            i.secondary = false;
        }
        if let Some((yaw, pitch)) = c.frozen {
            let (command, select) = (i.command, i.select);
            *i = Intent {
                yaw,
                pitch,
                command,
                select,
                ..default()
            };
        }
        let pressed = buttons::of(i);
        if c.at_controls {
            i.move_axis = Vec2::ZERO;
        }
        if c.weapon_hidden != 0 {
            i.fire = false;
            i.secondary = false;
            i.reload = false;
        }
        if c.buttons != pressed {
            c.buttons = pressed;
        }
    }
}

/// Where a character views the world from instead of its eye (a
/// point_viewcontrol camera; viewcontrol_and_templates.md 1.5): engine
/// space. The server's logic moves it; the client predicting the
/// character hears it with its own state.
#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapView {
    pub origin: Vec3,
    pub rotation: Quat,
}

/// A character carried by another entity, the one named (a vehicle's
/// seat): its movement is suspended whatever game's it is (every movement
/// implementation skips it: Source's, Combat Arms', Halo's, noclip and the
/// stand-in) and its carrier puts it where it rides. A network client
/// predicting it takes the server's state of it as it comes (no prediction
/// error, no eased view: `net::predict`), since the carrier is the
/// server's. The game that seats it puts it on and takes it off. It makes
/// no movement sounds (steps, jump and landing steps), whatever its keys
/// say (every game's footsteps filter by `Afoot`), and it is drawn with
/// its carrier (`map::interp`, `ease_carried`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carried(pub Entity);

/// The characters that may step: not carried (`Carried`). Every game's
/// footsteps, jump and landing sounds query with it (Source's movement,
/// which makes them, skips `Carried` already).
pub type Afoot = Without<Carried>;

/// A map entity changed a character's score (game_score): kills, or with
/// `team` its team's score; `allow_negative` as the entity's flag 1
/// (specs/source/game_entities.md 4). The rules apply it.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct ScoreChange {
    pub target: Entity,
    pub points: i32,
    pub team: bool,
    pub allow_negative: bool,
}

/// Takes `Damage` without having `Health`: something else (the logic
/// layer's breakables) reads the messages aimed at it. Weapons hit it as
/// a plain object (no hitgroups, no flesh sounds).
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Damageable;

