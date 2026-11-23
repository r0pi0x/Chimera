//! `party_roster`'s tests of Halo: CE in the party, one test binary: its
//! guns, grenades, balance, maps and vehicles on the party's simulation. A
//! change to Halo selects this binary and the cross-game one (`it`), not
//! the other games' (checks/src/affected.rs, "tests by game").
//! `cargo test -p party_roster --features dev --test halo_ce`.

// Shared helpers (tests/common/): each binary includes the ones it uses.
#[path = "../common/halo_party.rs"]
mod halo_party;
#[path = "../common/lobby.rs"]
mod lobby;
#[path = "../common/net_play.rs"]
mod net_play;
#[path = "../common/play.rs"]
mod play;

mod arena_net;
mod balance_table;
mod halo_balance;
mod halo_grenade_rung;
mod halo_grenades;
mod halo_ladder;
mod halo_melee_demotes;
mod halo_vehicle_looks;
mod persist;

// Slow ones (the user's installs): the fast tier skips `heavy::`.
mod heavy {
    mod arena_bots;
    mod grenade_rung_pace;
    mod halo_bots;
    mod halo_envelope;
    mod halo_gun_bots;
    mod halo_gun_sounds;
    mod halo_ladders;
    mod halo_maps;
    mod halo_net_rung;
    mod halo_vehicles;
    mod net_smoothness;
}
