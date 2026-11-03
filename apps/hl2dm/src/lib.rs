//! The Half-Life 2: Deathmatch app's glue (apps/hl2dm, crate `hl2dm_app`):
//! what composes Chimera and HL2:DM (games/hl2dm) into the game binary
//! (`hl2dm`, main.rs). M1 of docs/plans/active/hl2dm.md: the stock maps
//! (and any map of the install) or the greybox, with HL2:DM's stand-in
//! movement and its bullet and melee weapons (M4).
//!
//! `SimPlugin`: the simulation as the binary runs it (its tests run it on
//! the headless harness, tests/it/).
//!
//! `ClientPlugin`: the client shell (`chimera_client`) with this app's
//! hooks (`client_hooks`) and HL2:DM's field of view (75, specs/hl2dm/
//! source_deltas.md 1.8); the shell's own HUD (HL2:DM's is M9).
//!
//! `VERSION` and `BUILD`: this build's version and commit, stamped by
//! build.rs (the script every app shares, chimera/core/build_stamp.rs).
//!
//! Maps: `hl2dm:<name>` (or a bare name) from the install
//! (`hl2dm::game::maps`), or from a copy of the map's file (a server's
//! map downloaded); the greybox otherwise.
//!
//! Known shortcuts: no menus (straight into the map or the greybox) and
//! the shell's HUD; the shell keeps its config (binds, cvars) in the same
//! folder as the other apps' (`console::cfg_dir`).

use bevy::prelude::*;
use chimera_client::client::{AppHooks, Args, ClientPlugins};
use chimera_world::{
    greybox,
    map::{MapData, MapLoadOptions},
};
use hl2dm::game::{self, GAME, maps};

/// This build's version (`version` command, bug reports); stamped by
/// build.rs.
pub const VERSION: &str = env!("CHIMERA_VERSION");

/// This build's commit (`git describe`: client hooks, bug reports);
/// stamped by build.rs.
pub const BUILD: &str = env!("CHIMERA_GIT");

/// HL2:DM's field of view: `default_fov` 75, horizontal at 4:3
/// (specs/hl2dm/source_deltas.md 1.8).
pub const DEFAULT_FOV: f32 = 75.0;

/// What this app tells the client shell: its commit, HL2:DM's maps by id
/// (`load_map_with`) or from a downloaded file (`load_map_file`), the
/// install's map files for the network's offer, and the greybox's hooks.
pub fn client_hooks() -> AppHooks {
    AppHooks {
        build: BUILD,
        load_map: load_map_with,
        load_map_file,
        map_file_bytes,
        unload_greybox: greybox::unload,
        respawn_greybox: greybox::respawn,
    }
}

/// The map's name in `id` (`hl2dm:<name>` or a bare name), or why it
/// isn't HL2:DM's.
pub fn map_name(id: &str) -> Result<&str, String> {
    match id.split_once(':') {
        Some((game, name)) if game == GAME => Ok(name),
        Some(_) => Err(format!("{id}: not an HL2:DM map ({GAME}:<name>)")),
        None => Ok(id),
    }
}

/// Map `id` from the install at `hdr_level`.
pub fn load_map(id: &str, hdr_level: u8) -> Result<MapData, String> {
    load_map_with(id, &MapLoadOptions::hdr(hdr_level))
}

/// `load_map` with `options`.
pub fn load_map_with(id: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    maps::load_installed_map(map_name(id)?, options)
}

/// Map `id` from a copy of its file at `path`, its content from the
/// install.
pub fn load_map_file(id: &str, path: &std::path::Path, options: &MapLoadOptions) -> Result<MapData, String> {
    let name = map_name(id)?;
    let install = game::INFO.install().ok_or("no HL2:DM install found")?;
    let mount = game::mount::open(&install).map_err(|e| e.to_string())?;
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    maps::load_level_bytes_with(&mount, name, bytes, options)
}

/// The bytes of map `id`'s file in the install.
pub fn map_file_bytes(id: &str) -> Option<Vec<u8>> {
    let name = map_name(id).ok()?;
    let mount = game::mount::open(&game::INFO.install()?).ok()?;
    mount.read(&format!("maps/{name}.bsp")).ok()
}

/// The simulation as this app runs it (main.rs, and its tests on the
/// headless harness): HL2:DM's simulation and parts (`hl2dm::SimPlugins`,
/// `game::Hl2dmPlugins`), HL2:DM the gameplay (its rules, HUD, binds and
/// tick: `chimera_core::gameplay`).
pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        use chimera_core::gameplay::{Gameplay, GameplayAppExt};
        app.add_plugins((hl2dm::SimPlugins, game::Hl2dmPlugins))
            .set_gameplay(Gameplay::of(&game::INFO));
    }
}

/// The client as this app runs it: the shell's plugins with the app's
/// hooks, at HL2:DM's field of view.
pub struct ClientPlugin {
    pub args: Args,
}

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ClientPlugins {
            args: self.args.clone(),
            hooks: client_hooks(),
        })
        .insert_resource(chimera_client::client::video::PlayerFov(DEFAULT_FOV));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_build_is_stamped() {
        assert!(!super::BUILD.is_empty() && !super::VERSION.is_empty());
        assert_eq!(super::client_hooks().build, super::BUILD);
    }

    #[test]
    fn map_ids_are_hl2dms() {
        assert_eq!(super::map_name("hl2dm:dm_lockdown"), Ok("dm_lockdown"));
        assert_eq!(super::map_name("halls3"), Ok("halls3"));
        assert!(super::map_name("cs_source:de_dust2").is_err());
    }
}
