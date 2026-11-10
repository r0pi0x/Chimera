//! Lucker Party's dedicated server's binary (`lucker_party_server`): the
//! party kit's `SERVER` (`party_kit::server`) on Chimera's dedicated
//! server (`chimera_net::dedicated::run`: its `ServerPlugin` on a headless
//! run loop, the console on stdin: `status`, `minigames`,
//! `minigame_status`, `minigame_restart`, `changelevel`, `quit`, any
//! cvar; its output on stdout). Options: `--help` (`party_kit::server::
//! USAGE`).
//!
//! Released built alone: `cargo build --release -p lucker_party_server`
//! (no presentation links).

fn main() {
    chimera_net::dedicated::run(&party_kit::server::SERVER);
}
