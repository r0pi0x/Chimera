//! "Use my installed games" (`net::assets::INSTALL_CVAR`, the party
//! apps' Settings ▸ Network): off by default, saved in config.cfg when
//! turned on, and read back from it at the next start before anything
//! mounts a game (`install_default_here`): on, the install first; off, the
//! service first (a client's waiting token counting as a service).

use bevy::prelude::*;
use chimera_assets::config::InstallDefault;
use chimera_core::console::{Console, ConsolePlugin, config_text};
use chimera_net::net::assets::{INSTALL_CVAR, InstallSetting, install_chosen_in, install_setting};

/// A console with the setting; `config`'s lines run as a start runs
/// config.cfg.
fn app(config: Option<&str>) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, ConsolePlugin));
    install_setting(&mut app);
    if let Some(text) = config {
        let mut console = app.world_mut().resource_mut::<Console>();
        for line in text.lines() {
            console.submit(line.to_string());
        }
    }
    app.update();
    app.update();
    app
}

#[test]
fn use_my_installed_games_is_off_by_default_and_survives_a_restart() {
    let mut first = app(None);
    assert_eq!(first.world().resource::<InstallSetting>().use_install, 0, "off by default");
    let fresh = config_text(first.world_mut());
    assert!(!fresh.contains("asset_use_install"), "the default isn't written: {fresh}");
    assert_eq!(install_chosen_in([fresh.as_str()]), None, "nothing chosen: the service first");

    first.world_mut().resource_mut::<Console>().submit(format!("{INSTALL_CVAR} 1"));
    first.update();
    assert_eq!(first.world().resource::<InstallSetting>().use_install, 1);
    let saved = config_text(first.world_mut());
    assert_eq!(install_chosen_in([saved.as_str()]), Some(true), "saved on: {saved}");

    // The next start: the console reads it back, and so does the early
    // scan that sets the process's default before the mounts.
    let second = app(Some(&saved));
    assert_eq!(second.world().resource::<InstallSetting>().use_install, 1, "kept");
    // The command line's `+chimera:asset_use_install 0` (its `+` taken off) goes after the config: the last word wins.
    assert_eq!(install_chosen_in([saved.as_str(), "chimera:asset_use_install 0"]), Some(false));
    assert_eq!(install_chosen_in(["asset_use_install \"1\"; echo hi"]), Some(true), "the bare name, quoted");
}

#[test]
fn a_dedicated_server_reads_only_its_command_line() {
    use chimera_net::net::assets::install_default_here;
    assert_eq!(install_default_here(&["chimera:asset_use_install 1".into()], false), InstallDefault::Install);
    assert_eq!(install_default_here(&[], false), InstallDefault::ServiceFirst);
}
