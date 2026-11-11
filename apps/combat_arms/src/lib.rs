//! The Combat Arms app's glue (apps/combat_arms, crate `combat_arms_app`):
//! what composes Chimera and Combat Arms (games/combat_arms) into the game
//! binary (`combat_arms`, main.rs). Wave 0: the skeleton, on the greybox.
//!
//! `SimPlugin`: the simulation as the binary runs it (its tests run it on
//! the headless harness, tests/it/).
//!
//! `APP`: the game binary's app on Chimera's client app (`chimera_client::
//! client::app`; main.rs is `run(&APP)`): `SimPlugin`, the client shell
//! with this app's hooks (`client_hooks`) and Combat Arms' HUD in its
//! place (`combat_arms::ui::with_hud`), its field of view, its looks and
//! the player's profile.
//!
//! `VERSION` and `BUILD`: this build's version and commit, stamped by
//! build.rs (the script every app shares, chimera/core/build_stamp.rs);
//! `APP` carries both (`core::BuildVersion`, the hooks' commit).
//!
//! Maps: `combat_arms:<name>` from the install (`load_map`, through
//! `combat_arms::game::world`), the greybox otherwise.
//!
//! Known shortcuts: maps load only by id (no downloaded map files, no
//! map offer to network clients); the shell keeps its config (binds, cvars) in the same
//! folder as the CS:S app's (`console::cfg_dir`).

use bevy::prelude::*;
use chimera_client::client::{
    AppHooks,
    app::{AppMaps, GameApp, GameAppHooks},
};
use chimera_world::map::{MapData, MapLoadOptions};

/// This build's version (`version` command, bug reports): see
/// `version::format_version`; stamped by build.rs.
pub const VERSION: &str = env!("CHIMERA_VERSION");

/// This build's commit (`git describe`: client hooks, bug reports);
/// stamped by build.rs.
pub const BUILD: &str = env!("CHIMERA_GIT");

/// What this app tells the client shell: its commit, Combat Arms' maps
/// by id (`load_map`), and the greybox's own hooks (`APP`'s).
pub fn client_hooks() -> AppHooks {
    APP.app_hooks()
}

/// The game binary's app (`combat_arms`, main.rs), on Chimera's client
/// app (`chimera_client::client::app`): `SimPlugin` on a map of the
/// install, else straight into the greybox (no menus yet); the shell with
/// Combat Arms' HUD (`combat_arms::ui::with_hud`), then its field of
/// view, its other looks (`CaUiPlugin`) and the player's profile
/// (options: loaded at start, saved at exit).
pub static APP: GameApp = GameApp {
    bin: "combat_arms",
    info: &combat_arms::game::INFO,
    title: "Combat Arms",
    about: || {
        "combat_arms: Combat Arms on Chimera (its movement, the M4A1 and the M9 on the greybox,\n\
         or a map of the install: --map combat_arms:<name>, e.g. combat_arms:warehouse).\n"
            .into()
    },
    version: VERSION,
    build: BUILD,
    maps: AppMaps {
        load: load_map_with,
        load_file: |id, _, _| Err(no_maps(id)),
        file_bytes: |_| None,
    },
    // The install as `combat_arms://` assets.
    installs: || {
        vec![chimera_present::assets::InstallSource::shared(
            combat_arms::game::INFO.id,
            combat_arms::game::world::mount,
        )]
    },
    // The simulation at Combat Arms' tick (the app's tests run it
    // headless).
    sim: |app, _| {
        app.add_plugins(SimPlugin);
    },
    hud: combat_arms::ui::with_hud,
    front_end: |app, _| {
        app.insert_resource(chimera_client::client::video::PlayerFov(combat_arms::game::view_model::DEFAULT_FOV))
            .add_plugins(combat_arms::ui::CaUiPlugin);
        combat_arms::game::options::use_user_profile(app, None);
    },
    hooks: GameAppHooks {
        // No menus yet: straight into the greybox.
        args: Some(|mut args| {
            args.map.get_or_insert_with(|| "chimera:greybox".into());
            args
        }),
        server: Some("combat_arms_server"),
        ..GameAppHooks::NONE
    },
};

fn no_maps(id: &str) -> String {
    format!("{id}: Combat Arms maps load by id only (combat_arms:<name>), not from a file")
}

/// Map `id` (`combat_arms:<name>`, or a bare name) from the install; the
/// HDR level doesn't apply.
pub fn load_map(id: &str, _hdr_level: u8) -> Result<MapData, String> {
    load_map_with(id, &MapLoadOptions::default())
}

/// `load_map` with `options`.
pub fn load_map_with(id: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    match id.split_once(':') {
        Some((game, _)) if format!("{game}:") != combat_arms::game::world::MAP_PREFIX => {
            Err(format!("{id}: not a Combat Arms map (combat_arms:<name>)"))
        }
        _ => combat_arms::game::world::load_with(id, options),
    }
}

/// The simulation as this app runs it (main.rs, and its tests on the
/// headless harness): Chimera's and Combat Arms' simulation plugins
/// (`combat_arms::SimPlugins`, `CombatArmsPlugins`), Combat Arms the
/// gameplay (its rules, HUD, binds and tick: `chimera_core::gameplay`),
/// with the install's data and the room's default mode (where the game's
/// own scenario tests' `combat_arms::harness::CaSim` plays the built-in
/// data and no mode).
pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        use chimera_core::gameplay::{Gameplay, GameplayAppExt};
        app.add_plugins((combat_arms::SimPlugins, combat_arms::game::CombatArmsPlugins))
            .set_gameplay(Gameplay::of(&combat_arms::game::INFO));
    }
}

#[cfg(test)]
mod tests {
    /// The world is 75 degrees vertical (view_models.md 5.2), the view
    /// model 50 (5.1): the app's unzoomed field of view is the spec's
    /// 91.31 at 4:3, and the view model's follows it unshrunk.
    #[test]
    fn the_world_is_75_degrees_vertical() {
        use chimera_client::client::{FirstPersonCamera, spectate::SpecView, video::PlayerFov, zoom_camera};
        use chimera_world::map::view_model::{vertical_fov, view_model_fov};
        use combat_arms::game::view_model::DEFAULT_FOV;
        let mut app = bevy::prelude::App::new();
        app.init_resource::<SpecView>()
            .insert_resource(PlayerFov(DEFAULT_FOV))
            .add_systems(bevy::prelude::Update, zoom_camera);
        let camera = app
            .world_mut()
            .spawn((FirstPersonCamera, Projection::Perspective(PerspectiveProjection::default())))
            .id();
        app.update();
        let Some(Projection::Perspective(p)) = app.world().get::<Projection>(camera) else {
            panic!("no projection");
        };
        assert!((p.fov.to_degrees() - 75.0).abs() < 0.01, "{}", p.fov.to_degrees());
        let vm = combat_arms::game::view_model::settings();
        assert!((vertical_fov(view_model_fov(&vm, DEFAULT_FOV)) - 50.0).abs() < 0.01);
    }

    use bevy::prelude::*;

    #[test]
    fn the_build_is_stamped() {
        // build.rs ran: a commit and a version (or "unknown" without git),
        // which the shell's hooks carry.
        assert!(!super::BUILD.is_empty() && !super::VERSION.is_empty());
        assert_eq!(super::client_hooks().build, super::BUILD);
    }

    /// The wheel's `invnext` / `invprev` switch weapons through the
    /// shell's selection, which Combat Arms' front end uses (no HUD row:
    /// each step switches at once).
    #[test]
    fn the_wheel_steps_through_the_weapons() {
        use bevy::prelude::*;
        use chimera_client::client::weapon_menu::{WeaponMenu, WeaponMenuPlugin};
        use chimera_core::{console::Console, core::LocalPlayer};
        use chimera_sim::weapon::{Inventory, Weapon};
        use chimera_world::{
            greybox::{self, GreyboxMapPlugin},
            harness::Sim,
        };
        use combat_arms::{
            game::{grenades, inventory, movement, weapons},
            harness::CaSim,
        };

        let mut sim = Sim::with(CaSim, |app| {
            app.init_resource::<ButtonInput<KeyCode>>()
                .init_resource::<ButtonInput<MouseButton>>()
                .add_plugins((GreyboxMapPlugin, WeaponMenuPlugin));
        });
        let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
        sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
        sim.seconds(0.5);
        let active = |sim: &Sim| {
            let w = sim.app.world();
            w.get::<Weapon>(w.get::<Inventory>(p).unwrap().active.unwrap()).unwrap().id
        };
        assert_eq!(active(&sim), weapons::RIFLE);
        // A command, then the selection's pick passed to the intent (as
        // the local input does) until the switch is done.
        let command = |sim: &mut Sim, line: &str| {
            sim.app.world_mut().resource_mut::<Console>().submit(line);
            for _ in 0..90 {
                sim.ticks(1);
                let pick = sim.app.world().resource::<WeaponMenu>().selected_slot();
                sim.intent(p).select = pick;
            }
        };
        command(&mut sim, "invnext");
        assert_eq!(active(&sim), inventory::KNIFE);
        command(&mut sim, "invprev");
        assert_eq!(active(&sim), weapons::RIFLE);
        command(&mut sim, "invprev");
        assert_eq!(active(&sim), grenades::M67, "wraps to the loadout's last");
    }
}
