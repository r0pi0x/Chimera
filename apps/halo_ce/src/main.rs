//! The Halo: Combat Evolved game binary (`halo_ce`): the app's `APP`
//! (lib.rs: Halo's simulation, `halo_ce::SimPlugins` + `HaloPlugins`, on a
//! map of the disc, `--map halo_ce:<name>`, e.g. `halo_ce:hangemhigh`, or
//! the greybox; the presentation, the network and the client shell) on
//! Chimera's client app (`chimera_client::client::app::run`): stand, run and
//! jump with Halo's movement. Options: `--help`.
//!
//! Network play is the shell's (`connect`, `-port`, `maxplayers`, `map`), or
//! a dedicated server (apps/halo_ce_server, `halo_ce_server`).

fn main() {
    chimera_client::client::app::run(&halo_ce_app::APP);
}
