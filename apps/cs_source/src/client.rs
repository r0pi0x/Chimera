//! The client shell (chimera/client, crate `chimera_client`) at its old
//! path, with the app's own: `movement_config` (`--exec` and `--cvar` on
//! CS:S's movement, the app's `APP`), and CS:S's HUD modules where the
//! shell had them (`game_hud`, `radar`, ...: the front end's), the
//! spectator's HUD beside the shell's `spectate`.
//!
//! App glue, not the shell: the architecture test counts this file as
//! the crate root's (`lib`), which may name the games.

pub use chimera_client::client::*;

pub use crate::games::cs_source::ui::{game_hud, hud_sprites, overview, radar, weapon_select, win_panel};

/// Spectating: the shell's state and camera with CS:S's spectator HUD.
pub mod spectate {
    pub use crate::games::cs_source::ui::spectator_hud::*;
    pub use chimera_client::client::spectate::*;
}

/// Source movement settings: CS:S's, then `--exec` files, then `--cvar`s
/// (a bad file or cvar ends the process, 2).
pub fn movement_config(args: &Args) -> crate::games::cs_source::movement::SourceMovementConfig {
    let mut config = crate::games::cs_source::movement::SourceMovementConfig::default();
    for file in &args.exec {
        match std::fs::read_to_string(file) {
            Ok(text) => {
                for problem in config.exec(&text) {
                    eprintln!("warning: {}: {problem}", file.display());
                }
            }
            Err(e) => {
                eprintln!("error: --exec {}: {e}", file.display());
                std::process::exit(2);
            }
        }
    }
    for (name, value) in &args.cvars {
        if let Err(e) = config.set_cvar(name, value) {
            eprintln!("error: --cvar {name}={value}: {e}");
            std::process::exit(2);
        }
    }
    config
}
