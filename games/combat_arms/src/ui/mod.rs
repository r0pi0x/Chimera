//! Combat Arms' front end on the client shell (the `ui` feature), built
//! the game's way (specs/combat_arms/ui_system.md) on Chimera's reference
//! canvas (`client::canvas`):
//!
//! - `layout`: widget trees from the game's layout format (`CBALAYOUT`,
//!   `layout::xml`), placed and scaled by its rules (HUD panels stretched
//!   by x_r, y_r; menus and popups 1:1), fonts scaled with both factors,
//!   texts aligned in their parent's box, string tables (`layout::text`).
//! - `desk`: screens and popups as the pointer sees them (draw order,
//!   modal capture, hover and click sounds, buttons firing on press).
//! - `draw`: a laid-out tree as the shell's UI nodes (the install's font
//!   faces, stand-ins for the game's keyed art).
//! - `hud` (crosshair and basic HUD, hud_crosshair.md): its texts placed
//!   and sized by `layout`, with the game mode's score panel and round
//!   timer (`hud::score`); `scoreboard`: the scoreboard (Tab, and a
//!   round's result); `loading`: the loading screen while joining;
//!   `menu`: the in-game menu (Esc) in place of the
//!   shell's default; `hud::death`: the death screen on the install's
//!   kill-cam layout, and the death camera's input and camera on the shell;
//!   `music`: plays the client's music and the map's ambient sounds
//!   (game::music, music_ambience.md);
//!   `chat`: the match chat's queue (the model); `talk`: the chat panel,
//!   input bar and radio menu on it, and their keys;
//!   `hud::radar`, `hud::full_map` (with `hud::scene`): the radar and the
//!   full-screen map (radar_minimap.md);
//!   `visuals`: placeholder looks (box guns); `effects`: decals, tracers,
//!   casings, the first-person muzzle flash and effect lights
//!   (game::effects).
//!
//! `with_hud` puts Combat Arms' HUD among the shell's plugins (after its
//! fallback HUD, `client::hud::HudPlugin`) and the shell's weapon
//! selection (`client::weapon_menu`, as CS:S's: `slot1`..`slot5`,
//! `invnext` / `invprev`, here with no HUD row, so they switch at once;
//! see `game::inventory`), as CS:S's `with_hud` does; `CaUiPlugin` adds
//! the rest after the shell: the in-game menu, the scoreboard, the
//! visuals and `slot6` (key 6, the objective class: the mission bomb).
//!
//! `CaUi` is the UI's data: our font list (`data/ui/fonts.toml`), string
//! table (`data/ui/strings.txt`) and stand-in textures.
//!
//! With the V1 key the install's own layouts, string table, font list
//! and art (`install`) replace ours in the app (`CaUi::installed`): the
//! HUD's health and weapon panels are the game's HUD layout's named
//! controls, bound to the HUD's values (`hud::install`), as are its kill
//! feed, the mode's score panel, the pickup prompt and the action gauge
//! (`hud::bound`), the scoreboard and its result banner
//! (`scoreboard::BoundBoard`) and the loading screen (`loading`); the
//! in-game menu is the game's (`menu`); ours stay the fallback, and the
//! headless tests'.
//!
//! `lobby`: the lobby's screens (room list, creation popup, waiting room,
//! inventory; `ca_lobby 1`).
//!
//! Known shortcuts: no other screens (shop, result screen: the
//! game has them as menus at 1024 x 768; no vote panel yet, and the
//! death screen only with the install's layout); without the install the HUD's panels keep their places as code
//! (`hud::model`, from hud_crosshair.md); with it the crosshair and name
//! tags stay code-drawn (as the spec has them), the objectives' lines
//! ours.

pub mod attached_fx;
pub mod chat;
pub mod desk;
pub mod draw;
pub mod effects;
pub mod hud;
pub mod install;
pub mod layout;
pub mod loading;
pub mod lobby;
pub mod menu;
pub mod music;
pub mod options;
pub mod options_apply;
pub mod scope;
pub mod scoreboard;
pub mod talk;
pub mod textures;
pub mod visuals;

use bevy::{app::PluginGroupBuilder, prelude::*};

use draw::StandIns;
use layout::{Ctx, FontList, Strings};

/// The shell's plugins with Combat Arms' HUD in its place, and the
/// shell's weapon selection where CS:S has its own (after the views).
pub fn with_hud(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    shell
        // Combat Arms draws its own shot effects (`effects`).
        .disable::<crate::client::effects::ShotEffectsPlugin>()
        .add_after::<crate::client::hud::HudPlugin>(hud::CaHudPlugin)
        .add_after::<crate::client::view::ViewPlugin>(crate::client::weapon_menu::WeaponMenuPlugin)
}

/// Combat Arms' front end in a party app (beside its `game::BUNDLE`):
/// its HUD on the shell (`with_hud`).
pub static PARTY_CLIENT: chimera_world::game_bundle::GameClient = chimera_world::game_bundle::GameClient {
    game: crate::game::GAME,
    hud: Some(with_hud),
    app: None,
};

/// Combat Arms' front end but its HUD: the in-game menu and the
/// placeholder visuals.
pub struct CaUiPlugin;

impl Plugin for CaUiPlugin {
    fn build(&self, app: &mut App) {
        use crate::console::ConsoleAppExt;
        app.add_plugins((
            visuals::CaVisualsPlugin,
            effects::CaEffectsDrawPlugin,
            menu::CaMenuPlugin,
            options::CaOptionsPlugin,
            lobby::CaLobbyScreenPlugin,
            options_apply::CaOptionsApplyPlugin,
            scoreboard::CaScoreboardPlugin,
            loading::CaLoadingPlugin,
            hud::death::CaDeathScreenPlugin,
            talk::CaTalkPlugin,
            music::CaMusicPlayPlugin,
            textures::CaTexturesPlugin,
            scope::CaScopePlugin,
        ))
        // Key 6, the objective class (inventory.md 2, 3: the mission
        // bomb), through the shell's selection as slot 6.
        .console_command(
            "combat_arms:slot6",
            "The objective class (the mission bomb).",
            |w, _| {
                if let Some(mut m) = w.get_resource_mut::<crate::client::weapon_menu::WeaponMenu>() {
                    m.pressed.push(5);
                }
                Ok(None)
            },
        )
        // The flash fades on Combat Arms' cosine, not the shell's linear
        // fade (grenades.md Corrections 2026-10-22, 1).
        .insert_resource(crate::client::senses::FlashFade(crate::game::grenades::blinded_alpha))
        .add_message::<crate::game::bots::chat::CaBotChat>()
        .add_systems(Update, bot_chat);
    }
}

/// A bot's chat line in the chat box (bots.md 10): "nickname : line", the
/// line its string-table key resolved, for its team (our own text when
/// the table lacks the key: `bots::chat::shown_line`).
pub fn bot_chat(
    mut lines: MessageReader<crate::game::bots::chat::CaBotChat>,
    ui: Option<Res<CaUi>>,
    names: Query<(&Name, Option<&crate::core::Team>)>,
    mut out: MessageWriter<crate::client::chat::ChatLine>,
) {
    for l in lines.read() {
        let line = crate::game::bots::chat::shown_line(
            |k| ui.as_ref().map_or_else(|| k.to_string(), |ui| ui.strings.resolve(k)),
            &l.key,
        );
        let (name, team) = names.get(l.bot).map_or(("bot".to_string(), None), |(n, t)| (n.to_string(), t.copied()));
        let mut chat = crate::client::chat::ChatLine::plain(format!("{name} : {line}"));
        chat.1 = team;
        out.write(chat);
    }
}

/// Combat Arms' UI data: the font list, the string table and the
/// stand-in textures; with the install's (`installed`), its layouts,
/// strings and art (`install`).
#[derive(Resource, Clone, Debug)]
pub struct CaUi {
    pub fonts: FontList,
    pub strings: Strings,
    pub stand_ins: StandIns,
    pub install: Option<std::sync::Arc<install::InstallUi>>,
}

impl Default for CaUi {
    fn default() -> Self {
        let mut stand_ins = StandIns::default();
        let mut add = |name: &str, size: (i32, i32), c: [u8; 4]| {
            stand_ins.0.insert(install::texture_key(name), (IVec2::new(size.0, size.1), c));
        };
        // PLACEHOLDERS for the in-game menu's art: flat boxes.
        add("igm_back", (220, 145), [22, 26, 30, 235]);
        add("igm_button_A", (196, 24), [52, 58, 64, 255]);
        add("igm_button_B", (196, 24), [84, 92, 100, 255]);
        add("igm_button_C", (196, 24), [120, 96, 30, 255]);
        add("igm_button_D", (196, 24), [38, 40, 44, 200]);
        // PLACEHOLDERS for the options screen's art (`options`): flat
        // boxes, a state to a texture (A normal, B hover, C pressed or
        // checked, D disabled).
        add("opt_back", (700, 520), [22, 26, 30, 245]);
        add("opt_list", (660, 100), [14, 16, 20, 255]);
        for (prefix, size, states) in [
            ("opt_tab_", (100, 24), [[52, 58, 64, 255], [84, 92, 100, 255], [120, 96, 30, 255], [38, 40, 44, 200]]),
            ("opt_btn_", (90, 26), [[52, 58, 64, 255], [84, 92, 100, 255], [120, 96, 30, 255], [38, 40, 44, 200]]),
            ("opt_chk_", (14, 14), [[60, 66, 74, 255], [90, 98, 108, 255], [255, 198, 0, 255], [40, 42, 46, 255]]),
            ("opt_drop_", (150, 20), [[40, 44, 50, 255], [70, 78, 86, 255], [120, 96, 30, 255], [30, 32, 36, 255]]),
            ("opt_row_", (330, 15), [[34, 38, 44, 255], [60, 68, 76, 255], [120, 96, 30, 255], [30, 32, 36, 255]]),
        ] {
            for (state, c) in ['A', 'B', 'C', 'D'].into_iter().zip(states) {
                add(&format!("{prefix}{state}"), size, c);
            }
        }
        // PLACEHOLDER for a round mode's bar divider marks
        // (hud_crosshair.md D6: their look isn't given).
        add(hud::bound::DIVIDER, (1, 1), [20, 20, 20, 220]);
        let mut fonts = FontList::ours();
        range_font(&mut fonts, "bold 20");
        Self {
            fonts,
            strings: Strings::ours(),
            stand_ins,
            install: None,
        }
    }
}

/// The range readout's font (hud_crosshair.md A9, Corrections 15): the
/// face of the font list's `from` (the install's `SysFont13`; ours a
/// stand-in) asked for at 20 px, as `hud::model::RANGE_FONT`.
fn range_font(fonts: &mut FontList, from: &str) {
    let Some(f) = fonts.get(from).cloned() else { return };
    let height = 20;
    fonts.add(layout::Font {
        name: hud::model::RANGE_FONT.to_string(),
        height,
        average: ((f.average * height) as f32 / f.height.max(1) as f32).round().max(1.0) as i32,
        face: f.face,
    });
}

impl CaUi {
    /// Ours with the install's UI data over it (`install::installed`):
    /// its string table (ours for the ids it lacks), the fonts its
    /// layouts name, its art's sizes (drawn white: the images are
    /// `draw::CaArt`'s). Ours alone without it.
    pub fn installed(data: &crate::game::data::CaDataInner) -> Self {
        install::installed(data).map_or_else(Self::default, Self::with_install)
    }

    /// Ours with `inst` over it (`installed`).
    pub fn with_install(inst: std::sync::Arc<install::InstallUi>) -> Self {
        let mut ui = Self::default();
        if let Some(t) = &inst.strings {
            let mut strings = t.clone();
            for (k, v) in &ui.strings.values {
                strings.values.entry(k.clone()).or_insert_with(|| v.clone());
            }
            ui.strings = strings;
        }
        install::apply_font_list(&inst.fonts, &mut ui.fonts);
        if let Some(f) = inst.fonts.iter().find(|f| f.number == hud::model::RANGE_SYSFONT) {
            range_font(&mut ui.fonts, &f.name.clone());
        }
        for l in [&inst.hud, &inst.menu, &inst.indicator, &inst.result, &inst.team_list, &inst.no_team_list, &inst.loading, &inst.kill_cam]
            .into_iter()
            .flatten()
            .chain(inst.options.iter().flatten())
        {
            install::add_fonts(l, &mut ui.fonts);
        }
        for (k, a) in &inst.art {
            ui.stand_ins.0.insert(k.clone(), (a.size, [255; 4]));
        }
        ui.install = Some(inst);
        ui
    }

    pub fn ctx(&self) -> Ctx<'_, StandIns> {
        Ctx {
            fonts: &self.fonts,
            strings: &self.strings,
            textures: &self.stand_ins,
        }
    }
}
