//! The mashup game binary (`mashup_demo`): the crate's `APP` (lib.rs: a
//! Counter-Strike: Source map, de_dust2 without `--map`, played as Combat
//! Arms: its soldier, movement, weapons, HUD, bots and Team Death Match)
//! on Chimera's client app (`chimera_client::client::app::run`). Options:
//! `--help`; `--map cs_source:<name>` or `combat_arms:<name>` picks the
//! map, `+MaxPlayer <n>` the bots.

fn main() {
    chimera_client::client::app::run(&mashup_demo::APP);
}
