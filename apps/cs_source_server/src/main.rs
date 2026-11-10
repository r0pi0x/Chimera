//! The CS:S dedicated server's binary (`cs_source_server`): the
//! library's `SERVER` on Chimera's dedicated server (`chimera_net::
//! dedicated::run`: the headless run loop, the console on stdin
//! (`status`, `bot_add`, `bot_quota`, `changelevel`, `quit`, any cvar),
//! its output on stdout). Options: `--help` (lib.rs's `USAGE`).
//!
//! Released built alone: `cargo build --release -p cs_source_server` (no
//! presentation links).

fn main() {
    chimera_net::dedicated::run(&cs_source_server::SERVER);
}
