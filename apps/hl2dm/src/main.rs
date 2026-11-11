//! The Half-Life 2: Deathmatch game binary (`hl2dm`): the app's `APP`
//! (lib.rs: HL2:DM's simulation, `hl2dm::SimPlugins` + `Hl2dmPlugins`,
//! on a map of the install, `--map hl2dm:<name>`, e.g.
//! `hl2dm:dm_lockdown`, or the greybox; the presentation, the network
//! and the client shell with its HUD) on Chimera's client app
//! (`chimera_client::client::app::run`). M1: walk the stock maps with the
//! stand-in movement and shoot its bullet and melee weapons. Options:
//! `--help`.
//!
//! Network play is the shell's (`connect`, `-port`, `maxplayers`, `map`):
//! host a listen server with `hl2dm -port 27016 +maxplayers 4 +map
//! hl2dm:dm_lockdown`, join it with `hl2dm +connect <ip>:27016`, or a
//! dedicated server (apps/hl2dm_server, `hl2dm_server`).

fn main() {
    chimera_client::client::app::run(&hl2dm_app::APP);
}
