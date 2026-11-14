//! `cs_source`'s integration tests of its front end (feature `ui`), as one
//! test binary (each file a module): menus and HUD logic on the client
//! shell, checks that need the presentation, and the scenario tests that
//! use the shell's or the front end's state (on the headless harness with
//! CS:S's base, `harness::CsSim`, as tests/it/'s). `cargo test -p
//! cs_source --features dev,ui --test ui`. Where tests go:
//! docs/OBSERVABILITY.md, section 1.
//!
//! `stack` (tests/common/stack.rs) re-exports what the scenario tests run
//! on at the CS:S app's paths, `client` among them.
//!
//! `heavy` holds the slow ones (real maps from the install): the fast
//! tier skips them with `-- --skip heavy::`.

#[path = "../common/mod.rs"]
mod common;
#[path = "../common/stack.rs"]
mod stack;
#[allow(unused_imports)]
use common::{load_map, load_map_level};

mod announcer;
mod console_commands;
mod cs_guns;
mod debug_ui;
mod debug_overlays;
mod demo_chat;
mod hints;
mod join_flow;
mod menus;
mod net_players;
mod net_query;
mod net_rules;
mod night_vision;
mod objectives;
mod spectate;
mod target_id;
mod voice;
mod weapon_select;

/// Slow tests: the fast tier (`-- --skip heavy::`) leaves them out.
mod heavy {
    mod bot_radio;
    mod hud_layout;
    mod install_assets;
    mod kill_icons;
    mod lightmap_probe;
}
