//! The Combat Arms game binary (`combat_arms`): the app's `APP` (lib.rs:
//! Combat Arms' simulation, `combat_arms::SimPlugins` +
//! `CombatArmsPlugins`, on the greybox or a map of the install, `--map
//! combat_arms:<name>`; the presentation, the network and the client
//! shell with its HUD) on Chimera's client app (`chimera_client::client::
//! app::run`). Wave 0: walk and shoot the placeholder weapons. Options:
//! `--help`.
//!
//! Network play is the shell's (`connect`, `-port`, `maxplayers`, `map`):
//! host a listen server with `combat_arms -port 27016 +maxplayers 4 +map
//! greybox`, join it with `combat_arms +connect <ip>:27016` (Combat Arms'
//! part of it: `combat_arms::game::net`), or a dedicated server
//! (apps/combat_arms_server, `combat_arms_server`).

fn main() {
    chimera_client::client::app::run(&combat_arms_app::APP);
}
