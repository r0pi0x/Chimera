//! The Halo dedicated server's binary (`halo_ce_server`): the library's
//! `SERVER` on Chimera's dedicated server (`chimera_net::dedicated::run`:
//! the headless run loop, the console on stdin (`status`, `changelevel`,
//! `quit`, any cvar), its output on stdout). Options: `--help` (lib.rs's
//! `USAGE`).
//!
//! Released built alone: `cargo build --release -p halo_ce_server` (no
//! presentation links).

fn main() {
    chimera_net::dedicated::run(&halo_ce_server::SERVER);
}
