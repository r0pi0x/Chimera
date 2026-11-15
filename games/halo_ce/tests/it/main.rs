//! `halo_ce`'s integration tests, as one test binary (each file a module):
//! what the game tells the shell (`info`), the install's build and a
//! Halo map's asset offer (`assets`), the skeleton on the headless
//! harness (`skeleton`: `harness::HaloSim` and the greybox), the map
//! converter's pure pieces (`maps`), the movement (`movement`; over the
//! network: `movement_net`; with the vitality: `movement_vitality`), a
//! player's vitality through the damage seam (`vitality`, its tables
//! `vitality_data`, across games `cross_game`, over the network
//! `net_vitality`), the four weapons' rules (`weapons_rules`) and their
//! timelines and kills in the game (`weapons`), and, in `heavy`, the user's
//! disc image (its mount, every multiplayer map converted, a player standing
//! on Hang 'Em High, the vitality and damage tables and the weapon documents
//! against the disc, Halo from the asset store alone: `served`) (found
//! through `$CHIMERA_HALO_CE_ROOT` or
//! `[games.halo_ce] path`, each test passing with a note without one).
//! `cargo test -p halo_ce --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1.

mod arsenal;
mod arsenal_fx;
mod arsenal_rules;
mod assets;
mod body;
mod cross_game;
mod fx;
mod fx_place;
mod hud;
mod info;
mod ladders;
mod map_objects;
mod maps;
mod movement;
mod movement_net;
mod movement_vitality;
mod net_vitality;
mod skeleton;
mod ticks;
mod view;
mod vitality;
mod vitality_data;
mod weapons;
mod weapons_rules;

// Slow ones (the user's disc image): the fast tier skips `heavy::`.
mod heavy {
    mod body;
    mod fx;
    mod hud;
    mod ladder;
    mod map_objects;
    mod nav_within;
    mod maps;
    mod mount;
    mod served;
    mod stand;
    mod view;
    mod vitality_data;
    mod weapon_data;
    mod walk;
}
