//! `combat_arms`' integration tests of its front end (feature `ui`), as
//! one test binary (each file a module): the HUD's behaviour and the
//! scenario tests that read its state (`ui::hud::CaHud`, headless, on
//! `combat_arms::harness::CaSim` and the greybox, as tests/it/'s); the UI
//! system's layout, text, input and in-game menu (`ui_system`); the kill
//! feed, score panel, action panel, scoreboard and loading screen bound
//! by name to layouts written in the test (`bound`); the
//! first-person camera's rise and jitter (`camera`); that screens which
//! are down cost nothing per string (`frame_cost`); in `heavy` the
//! install's own layouts and string tables (`install_ui`, V1 key) and
//! its M4A1 and knife view models drawn on the greybox (`view_model`,
//! V2 key).
//! `cargo test -p combat_arms --features dev,ui --test ui`; where tests
//! go: docs/OBSERVABILITY.md, section 1.

mod attachments;
mod bot_chat;
mod bound;
mod camera;
mod death_screen;
mod fire_voices;
mod frame_cost;
mod hud;
mod hud_scale;
mod lobby;
mod music;
mod net_hud;
mod options;
mod radio_chat;
mod radar;
mod score;
mod ui_system;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod bot_chat_install;
    mod death_screen;
    mod install_ui;
    mod kill_icons;
    mod options;
    mod reticles;
    mod strings;
    mod view_model;
}
