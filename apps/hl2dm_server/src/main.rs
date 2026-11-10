//! The HL2:DM dedicated server's binary (`hl2dm_server`): the library's
//! `SERVER` on Chimera's dedicated server (`chimera_net::dedicated::run`:
//! the headless run loop, the console on stdin (`status`, `changelevel`,
//! `quit`, any cvar), its output on stdout). Options: `--help` (lib.rs's
//! `USAGE`).
//!
//! Released built alone: `cargo build --release -p hl2dm_server` (no
//! presentation links).

fn main() {
    chimera_net::dedicated::run(&hl2dm_server::SERVER);
}
