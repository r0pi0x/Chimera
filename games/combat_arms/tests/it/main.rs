//! `combat_arms`' integration tests, as one test binary (each file a
//! module): scenario tests of Combat Arms' simulation on the engine's
//! headless harness (`chimera_world::harness::Sim`) with its base
//! (`combat_arms::harness::CaSim`) and the greybox, with nothing above
//! the game. `cargo test -p combat_arms --features dev --test it`; where
//! tests go: docs/OBSERVABILITY.md, section 1.
//!
//! Wave 0 checks the skeleton: a player spawns, walks, fires the
//! placeholder rifle and switches to the placeholder knife. Spec test
//! cases (specs/combat_arms/) get their own files here as each subsystem
//! is implemented.

#[path = "../common/mod.rs"]
mod common;

mod aim_points;
mod attached_effects;
mod attached_fx;
mod attachments;
mod attribute_data;
mod bot_objectives;
mod bots;
mod bots_followups;
mod capture_the_flag;
mod characters;
mod clan_modes;
mod data_isolation;
mod death_killcam;
mod effects_sounds;
mod free_for_all;
mod gear_perks;
mod game_modes;
mod game_modes_followups;
mod grenades;
mod hired_guns;
mod install_launchers;
mod launcher_aim;
mod inventory;
mod lobby;
mod lobby_loadout;
mod lobby_net;
mod missions;
mod mode_corrections;
mod mode_kit;
mod model;
mod movement;
mod movement_corrections;
mod movement_volumes;
mod music_ambience;
mod net;
mod net_modes;
mod net_modes_2;
mod net_projectiles;
mod options;
mod own_sounds;
mod radio_chat;
mod radio_chat_net;
mod other_modes;
mod radar;
mod schedules;
mod rung_character;
mod rung_parts;
mod search_destroy;
mod server_docs;
mod skeleton;
mod special_devices;
mod special_weapons;
mod view_model;
mod vip_escort;
mod weapon_classes;
mod weapon_corrections;
mod weapon_firing;
mod weapon_timings;
mod world;
mod volume_corrections;
mod object_carry;
mod object_sounds;
mod world_corrections;
mod world_objects;
mod world_objects_play;
mod world_events;
mod world_markers;
mod debris;
mod world_teleports;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod attached_fx;
    mod attributes;
    mod body_skins;
    mod rung_attachments;
    mod bots;
    mod characters;
    mod effects_sounds;
    mod flight_looks;
    mod infected;
    mod junk_flea2;
    mod launchers_install;
    mod map_load;
    mod missions;
    mod modes_part_two;
    mod music_ambience;
    mod model;
    mod movement_volumes;
    mod own_sounds;
    mod timings;
    mod mount;
    mod options;
    mod props;
    mod scopes;
    mod served;
    mod radio;
    mod radar;
    mod render_styles;
    mod view_model;
    mod weapon_skins;
    mod world;
    mod world_objects;
    mod world_objects_2;
}
