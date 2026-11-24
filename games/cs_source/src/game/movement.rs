//! CS:S's movement (`cs_source:movement`): Source's player movement
//! (`source_engine::source_movement`, lift S6 of docs/plans/active/hl2dm.md,
//! re-exported here at its old paths, with its drowning, physics shadow
//! and push-away) with CS:S's hurt sounds (`super::pain`), and the
//! movement the characters CS:S spawns get (`match_movement`).

pub use source_engine::source_movement::*;

/// The movement's id.
pub const ID: &str = "cs_source:movement";

use bevy::prelude::*;

/// The movement a character the game spawns gets (bots, hostages): the
/// match's (`Loadout`) unless that is noclip, else CS:S's when registered,
/// else one some character uses that isn't noclip (headless tests with
/// another movement). Never the local player's current slot: a player
/// flying with noclip (or a `--movement chimera:noclip` run) gave every bot
/// and hostage noclip, so they hung in the air and flew through walls.
pub fn match_movement(world: &mut World) -> Option<&'static str> {
    use crate::slots::{Loadout, MovementRegistry, MovementSlot};
    let noclip = chimera_sim::movement::noclip::ID;
    if let Some(l) = world.get_resource::<Loadout>()
        && l.movement != noclip
    {
        return Some(l.movement);
    }
    if world.get_resource::<MovementRegistry>().is_some_and(|r| r.get(ID).is_some()) {
        return Some(ID);
    }
    world
        .query::<&MovementSlot>()
        .iter(world)
        .map(|m| m.0)
        .find(|m| *m != noclip)
}

/// CS:S's movement config, as measured on the reference install (cvars
/// read over RCON, 2026-10-05; the spec's "CS:S values") over the shared
/// code's (`QuakeConfig::reference`): jump speed sqrt(2 * 800 *
/// 57), which reproduces the measured standing-jump apex (54.75 units at
/// the server's 66.67 tick).
pub fn config() -> QuakeConfig {
    QuakeConfig {
        stopspeed: 75.0,
        jump_impulse: (2.0f32 * 800.0 * 57.0).sqrt(),
        key_speed: 400.0,
        // Measured with movecmp: ducked acceleration implies a wish speed
        // of 85 at max speed 250.
        duck_speed: 0.34,
        // Measured with the probe (the player's box and view offset):
        // CS:S stands 62 tall (eye 64) and ducks to 45 (eye 47); in the
        // air the feet move half the difference, 8.5 units.
        stand_height: 62.0,
        duck_height: 45.0,
        eye_stand: 64.0,
        eye_duck: 47.0,
        air_duck_shift: 0.5,
        // Measured with the probe (run/jump/land traces): stamina
        // 1315.79 ms per jump, 15 ms off per tick; jump speed x (1 -
        // 0.00019 T); ground speed x (1 - 0.000199 T) each tick between
        // friction and acceleration. The landing slowdown and the quick
        // stop after a jump.
        jump_stamina: 1315.789_4,
        stamina_jump_scale: 0.000_19,
        stamina_ground_scale: 0.000_199,
        // Measured: with sv_enablebunnyhopping 0, a jump scales the
        // velocity (3D, including this tick's gravity) down to 286 when
        // faster, whatever the weapon (1.1 x 260, the fastest weapon).
        enable_bunnyhopping: false,
        bunnyhop_cap: 286.0,
        ladder_dampen: 0.2,
        client_maxspeed: 260.0,
        step_min_speed: 150.0,
        duck_slows_ladder: true,
        duck_slows_everywhere: true,
        unduck_keeps_duck_timer: true,
        unduck_jump_ducks: true,
        clip_push_out: 1.0 / 32.0,
        // Measured on the probe server: 100 damage at 996, not 1024.
        fall_damage_per_speed: 100.0 / 416.0,
        ..QuakeConfig::reference()
    }
}

/// CS:S's movement config as registered (tests, tools and cvars change it
/// here).
pub fn config_mut(world: &mut World) -> Mut<'_, QuakeConfig> {
    world
        .resource_mut::<QuakeConfigs>()
        .map_unchanged(|c| c.get_mut(ID).expect("cs_source:movement registered"))
}

/// CS:S's movement config set to `config`, before or after the movement
/// is registered (the app's `--exec` and `--cvar` settings, a test's).
pub fn set_config(world: &mut World, config: QuakeConfig) {
    world.get_resource_or_init::<QuakeConfigs>().set(ID, config);
}

/// Source's movement as CS:S's (`ID`): its config is CS:S's (`config`,
/// registered by its id like every Source game's), its cvars the bare
/// Source names (CS:S's by the console's rule); with its pain sounds (a
/// hard landing's `Player.FallDamage`).
pub struct SourceMovementPlugin;

impl Plugin for SourceMovementPlugin {
    fn build(&self, app: &mut App) {
        app.source_movement(ID, Some(config()), None);
        // Spawn spots where players get to: up ledges a jump takes (a
        // standing jump clears about 45 units; custom maps' steps, lego
        // bricks, are jumped), bounded to the rooms a walk reaches.
        {
            use crate::map::SpawnClimbAppExt;
            app.add_spawn_climb(super::GAME, 0.8);
        }
        if !app.is_plugin_added::<super::pain::PainSoundsPlugin>() {
            app.add_plugins(super::pain::PainSoundsPlugin);
        }
    }
}
