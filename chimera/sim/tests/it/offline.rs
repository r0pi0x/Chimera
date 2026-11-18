//! A `Sim` reaches no install and no asset store (`mount::offline`): an
//! install a config names is not found, so scenario tests give the same
//! result on every machine. docs/OBSERVABILITY.md, section 1.

use chimera_core::mount::{config::LocalConfig, offline};
use chimera_sim::SimBasePlugins;
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

#[test]
fn a_sim_finds_no_install_even_one_the_config_names() {
    let dir = std::env::temp_dir();
    let config: LocalConfig =
        toml::from_str(&format!("[games.cs_source]\npath = {:?}\n", dir.display().to_string())).unwrap();
    let _sim = Sim::new(SimBasePlugins, GreyboxMapPlugin);
    assert!(offline::installs_forbidden(), "building a Sim turns the switch on");
    assert_eq!(config.game_path("cs_source"), None, "the named folder is not offered");
}
