//! Combat Arms' options (specs/combat_arms/options.md): the settings the
//! options screen edits and the profile file that keeps them, with no
//! drawing: the front end (`ui::options`) is the screen on top of this
//! and hands the results to the shell (mouse look, mixer, brightness).
//!
//! - `settings`: every option as data (`Setting`: console variable,
//!   profile section and key, type, code default, page, when it applies),
//!   the console variables registered from it and `CaSettings`, their
//!   values;
//! - `flow`: the profile's life (F: loading with the template and the
//!   version rule, K4's bindings, saving) and the screen's (P: opening,
//!   OK, Cancel, Default per tab, previews, key capture, the quality
//!   preset's apply);
//! - `bindings` (K, with `keys`): the 42 bindable actions, a binding's
//!   three slots, the missing-binding rules, rebinding by capture, the
//!   console's binds;
//! - `mouse` (M), `slider` (W), `video` (G: gamma, modes, presets, the
//!   Custom switches), `sound` (S2), `social` (O: the Other tab, the
//!   block list);
//! - `derive`: what the settings mean for the game (`CaMouseLook`,
//!   `CaSoundLevels`, `CaGamma`, `CaGameOptions`), the in-match sensitivity
//!   keys with their chat line, the HUD on/off key and the help key.
//!
//! The profile is `options.profile` (`chimera_core::profile`): the text of
//! `Profiles/player.txt` in the player's folder, created from the
//! template (`data/options/default_profile.prf`, ours) when missing and
//! kept unchanged where the options don't manage a line. The
//! options' console variables are the live values; the screen's edits wait
//! (`CaOptions::pending`) for OK unless the setting applies at once (a
//! preview that Cancel undoes).
//!
//! Known shortcuts (docs/tech-debt.md; the spec's most literal reading):
//! - The default profile is ours: the spec could not read the game's
//!   template (Open questions 1 and 2), so its values (volumes 70 and 50,
//!   sensitivity 5, gamma 1.22, High quality) are chosen, the stock keys
//!   are the spec's.
//! - The profile is in the user's data folder (beside the console's
//!   config), not the game's folder, and plain text; the Steam build's
//!   encryption (Open question 1) isn't there.
//! - The screen opens from the lobby's start menu (`ui::lobby`: its option
//!   entry runs the `options` console command, a modal over the current
//!   page that leaves the room state alone) and by that console command
//!   (refused in a match; `options force` opens it there too).
//! - Resolution, depth, anti-aliasing, window mode, vsync, quality
//!   presets and the Custom block are stored, derived (`Derived`) and
//!   kept; the renderer applies none of them, only brightness (a ramp)
//!   and the sound and mouse settings reach the shell. The music list's
//!   choices and the voice devices have no backing.
//! - Joystick bindings are kept and shown, never read; some actions'
//!   features aren't built (`bindings::COMMANDS_UNBUILT`).
//! - Binds are applied to the console's binds at load and OK; the
//!   console's own `bind` doesn't write the profile.
//! - Gamma maps to the shell's `mat_monitorgamma` (2.2 / gamma), which
//!   the shell clamps to 1.6..2.6, so only gamma 0.85 to 1.375 shows.

pub mod bindings;
pub mod derive;
pub mod flow;
pub mod keys;
pub mod mouse;
pub mod settings;
pub mod slider;
pub mod social;
pub mod sound;
pub mod video;

use std::{collections::BTreeMap, path::PathBuf};

use bevy::prelude::*;

pub use self::{
    bindings::{BindingTable, Capture, RawEvent},
    derive::{CaGameOptions, CaGamma, CaHudSwitch, CaMouseLook, CaNotices, HelpPopup},
    settings::{CaSettings, Setting, Tab},
    social::{CaAccount, CaSocial, CaVoice},
    sound::CaSoundLevels,
    video::{CaMachine, Custom, Derived, Mode},
};
use crate::profile::Profile;

/// The template profile (ours).
pub const DEFAULT_PROFILE: chimera_core::game_data::GameData = chimera_core::game_data!("data/options/default_profile.prf");

/// The options' state: the profile, the screen and what it has pending.
#[derive(Resource)]
pub struct CaOptions {
    /// The settings table (`settings::all`).
    pub table: Vec<Setting>,
    /// The quality presets' table (`video::presets`).
    pub presets: Vec<video::Row>,
    /// The template (`DEFAULT_PROFILE`).
    pub default_profile: Profile,
    /// The player profile in memory.
    pub profile: Profile,
    /// Where it lives (`Profiles/player.txt`); None: memory only.
    pub path: Option<PathBuf>,
    /// The profile was made from the template this start (F2).
    pub first_play: bool,
    /// The screen is open and on this tab.
    pub open: bool,
    pub tab: Tab,
    /// The screen's changes that haven't reached the engine, by variable
    /// name (lower case).
    pub pending: BTreeMap<String, String>,
    /// The bindings as the screen edits them (the profile's in-memory
    /// table); the console's binds follow on OK.
    pub bindings: BindingTable,
    /// A row waiting for input (K3).
    pub capture: Option<Capture>,
    /// The profile as saved when the screen opened (Cancel returns to
    /// it).
    pub saved: Option<Profile>,
    /// The Custom block's switches while the screen is open.
    pub custom: Custom,
    /// The key-name file (K2).
    pub key_names: keys::KeyNames,
    /// The display modes the renderer reports (G2); empty: unknown.
    pub modes: Vec<Mode>,
    /// The renderer's settings derived from the quality (G4).
    pub derived: Derived,
    /// The Other tab's edits that haven't been sent.
    pub social_pending: Option<CaSocial>,
    /// The account's block list (O).
    pub block_list: social::BlockList,
    /// How many times the profile was saved (tests).
    pub saves: u32,
}

impl Default for CaOptions {
    fn default() -> Self {
        let default_profile = Profile::parse(&DEFAULT_PROFILE);
        Self {
            table: settings::all(),
            presets: video::presets(),
            profile: default_profile.clone(),
            default_profile,
            path: None,
            first_play: false,
            open: false,
            tab: Tab::Graphic,
            pending: BTreeMap::new(),
            bindings: BindingTable::default(),
            capture: None,
            saved: None,
            custom: Custom::new(CaMachine::default()),
            key_names: keys::KeyNames::default(),
            modes: Vec::new(),
            derived: Derived::default(),
            social_pending: None,
            block_list: social::BlockList::default(),
            saves: 0,
        }
    }
}

impl CaOptions {
    /// A setting by variable name.
    pub fn setting(&self, cvar: &str) -> Option<&Setting> {
        self.table.iter().find(|x| x.cvar.eq_ignore_ascii_case(cvar))
    }
}

/// Where the user's profile lives: `profiles/combat_arms/player.txt`
/// beside the console's config folder.
pub fn user_profile_path() -> Option<PathBuf> {
    Some(crate::console::cfg_dir()?.parent()?.join("profiles").join("combat_arms").join("player.txt"))
}

/// Keep the profile on disk: load it at start and save it at exit (the
/// app's; headless tests load and save by hand). `path` None: the user's
/// folder.
pub fn use_user_profile(app: &mut App, path: Option<PathBuf>) {
    let path = path.or_else(user_profile_path);
    app.world_mut().resource_mut::<CaOptions>().path = path;
    // After the shell's own startup (its config and binds).
    app.add_systems(PostStartup, flow::load_at_start);
    app.add_systems(Last, flow::save_at_exit);
}

pub struct CaOptionsPlugin;

impl Plugin for CaOptionsPlugin {
    fn build(&self, app: &mut App) {
        let options = CaOptions::default();
        let table = options.table.clone();
        app.insert_resource(CaSettings::defaults(&table))
            .insert_resource(options)
            .init_resource::<CaMachine>()
            .init_resource::<CaAccount>()
            .init_resource::<CaSocial>()
            .init_resource::<CaVoice>()
            .init_resource::<CaSoundLevels>()
            .init_resource::<CaMouseLook>()
            .init_resource::<CaGamma>()
            .init_resource::<CaGameOptions>()
            .init_resource::<CaHudSwitch>()
            .init_resource::<HelpPopup>()
            .init_resource::<CaNotices>();
        settings::register(app, &table);
        derive::register(app);
        app.add_systems(Update, derive::update);
        flow::register(app);
    }
}
