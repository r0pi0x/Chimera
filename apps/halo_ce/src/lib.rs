//! The Halo: Combat Evolved app's glue (apps/halo_ce, crate `halo_ce_app`):
//! what composes Chimera and Halo (games/halo_ce) into the game binary
//! (`halo_ce`, main.rs). M1 of docs/plans/active/halo-ce.md: the greybox
//! with Halo's movement (`halo_ce::game::movement`), and the disc's maps
//! (`--map halo_ce:<name>`).
//!
//! `SimPlugin`: the simulation as the binary runs it (its tests run it on
//! the headless harness, tests/it/).
//!
//! `APP`: the game binary's app on Chimera's client app (`chimera_client::
//! client::app`; main.rs is `run(&APP)`): `SimPlugin`, the client shell
//! with Halo's HUD (`halo_ce::ui::with_hud`, none of its own yet) and Halo's
//! front end (`front_end`: its weapons' melee and zoom buttons).
//!
//! `VERSION` and `BUILD`: this build's version and commit, stamped by
//! build.rs (the script every app shares, chimera/core/build_stamp.rs).
//!
//! Maps: `halo_ce:<name>` (or a bare name) from the install, a disc image
//! or a folder of its files (`$CHIMERA_HALO_CE_ROOT` or `[games.halo_ce]
//! path`; `halo_ce::game::maps`); the greybox when no map is named. Until
//! Halo's map loader is written (`halo_ce::game::maps`), a named map fails
//! with the loader's message.
//!
//! Known shortcuts: no install is read as assets (`installs`: the disc's
//! textures and sounds come with their specs); no map file is offered to
//! clients or loaded from a downloaded copy (maps are the disc's, by id).

use bevy::prelude::*;
use chimera_client::client::{
    Args,
    app::{self, AppMaps, GameApp, GameAppHooks},
};
use chimera_world::map::{MapData, MapLoadOptions};
use halo_ce::game::{self, GAME, MAP_LOADER};

/// This build's version (`version` command, bug reports); stamped by
/// build.rs.
pub const VERSION: &str = env!("CHIMERA_VERSION");

/// This build's commit (`git describe`: client hooks, bug reports);
/// stamped by build.rs.
pub const BUILD: &str = env!("CHIMERA_GIT");

/// The game binary's app (`halo_ce`, main.rs), on Chimera's client app
/// (`chimera_client::client::app`): `SimPlugin` on a map of the install or
/// the greybox, the shell with Halo's HUD.
pub static APP: GameApp = GameApp {
    bin: "halo_ce",
    info: &game::INFO,
    title: "Halo: Combat Evolved",
    about: || {
        "halo_ce: Halo: Combat Evolved on Chimera (Halo's movement on the greybox;\n\
         --map halo_ce:<name>, e.g. halo_ce:hangemhigh, from the disc image named by\n\
         $CHIMERA_HALO_CE_ROOT or [games.halo_ce] path, else the greybox).\n"
            .into()
    },
    version: VERSION,
    build: BUILD,
    maps: AppMaps::by_id(load_map_with),
    installs: app::no_installs,
    // The simulation at Halo's tick (the app's tests run it headless).
    sim: |app, _| {
        app.add_plugins(SimPlugin);
    },
    hud: halo_ce::ui::with_hud,
    front_end,
    hooks: GameAppHooks {
        server: Some("halo_ce_server"),
        ..GameAppHooks::NONE
    },
};

/// The map's name in `id` (`halo_ce:<name>` or a bare name), or why it
/// isn't Halo's.
pub fn map_name(id: &str) -> Result<&str, String> {
    match id.split_once(':') {
        Some((game, name)) if game == GAME => Ok(name),
        Some(_) => Err(format!("{id}: not a Halo map ({GAME}:<name>)")),
        None => Ok(id),
    }
}

/// Map `id` from the install with `options`.
pub fn load_map_with(id: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    (MAP_LOADER.load)(map_name(id)?, options)
}

/// The simulation as this app runs it (main.rs, and its tests on the
/// headless harness): Halo's simulation and parts (`halo_ce::SimPlugins`,
/// `game::HaloPlugins`), Halo the gameplay (its tick and binds:
/// `chimera_core::gameplay`).
pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        use chimera_core::gameplay::{Gameplay, GameplayAppExt};
        app.add_plugins((halo_ce::SimPlugins, game::HaloPlugins)).set_gameplay(Gameplay::of(&game::INFO));
    }
}

/// The client after the shell (`APP`'s front end): nothing of Halo's own
/// yet (its field of view, menus and HUD come with their specs).
pub fn front_end(app: &mut App, _: &Args) {
    // The weapons' melee and zoom buttons.
    halo_ce::ui::binds(app);
}

#[cfg(test)]
mod tests {
    use chimera_world::map::MapLoadOptions;

    #[test]
    fn the_build_is_stamped() {
        assert!(!super::BUILD.is_empty() && !super::VERSION.is_empty());
        assert_eq!(super::APP.app_hooks().build, super::BUILD);
    }

    #[test]
    fn map_ids_are_halos() {
        assert_eq!(super::map_name("halo_ce:hangemhigh"), Ok("hangemhigh"));
        assert_eq!(super::map_name("bloodgulch"), Ok("bloodgulch"));
        assert!(super::map_name("hl2dm:dm_lockdown").is_err());
        assert!(super::load_map_with("cs_source:de_dust2", &MapLoadOptions::default()).is_err());
    }
}
