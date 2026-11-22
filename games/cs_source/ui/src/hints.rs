//! The automatic hints on screen (`cl_autohelp`, `rules::hints`): each
//! `AutoHint` for the local player as the HUD's hint text
//! (`client::chat::Hint`, the `HudHintDisplay` panel of the install's
//! `scripts/HudLayout.res`) in the install's words (its localization
//! token, from `resource/cstrike_english.txt` through the map's
//! `GameMenus::string`; without the install, the token's own words), with
//! the hints' sound (`HintRules::sound`, CS:S's `Hud.Hint`). Whether and
//! when one shows is the rules' (single player's or the server's).
//!
//! Known shortcuts (docs/tech-debt.md): the hint replaces any hint shown
//! and stays as long as one (`client::chat::HINT_SECONDS`); CS:S's
//! `HudHintKeyDisplay` (hints naming a bound key) isn't drawn: none of
//! the automatic hints' texts names one.

use bevy::prelude::*;

use crate::{
    client::chat::Hint,
    core::LocalPlayer,
    map::{PlaySound, hud::ActiveHud},
    rules::hints::{AutoHint, HintRules},
};

pub struct HintsHudPlugin;

impl Plugin for HintsHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Hint>()
            .add_message::<PlaySound>()
            .add_message::<AutoHint>()
            .init_resource::<HintRules>()
            .add_systems(Update, show);
    }
}

/// A token's words without the install: `Cstrike_TitlesTXT_Hint_out_of_ammo`
/// is "Out of ammo".
pub fn fallback_text(token: &str) -> String {
    let words = token.trim_start_matches('#');
    let words = words.rsplit_once("_Hint_").map_or(words, |(_, w)| w).replace('_', " ");
    let mut c = words.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// The local player's hints: their text, their sound.
fn show(
    mut hints: MessageReader<AutoHint>,
    local: Query<(), With<LocalPlayer>>,
    rules: Res<HintRules>,
    hud: Option<Res<ActiveHud>>,
    mut out: MessageWriter<Hint>,
    mut sounds: MessageWriter<PlaySound>,
) {
    for h in hints.read() {
        if !local.contains(h.to) {
            continue;
        }
        let fallback = fallback_text(&h.token);
        let text = hud
            .as_ref()
            .and_then(|hud| hud.0.menus.as_ref())
            .map_or(fallback.clone(), |m| m.string(&h.token, &fallback).trim().to_string());
        out.write(Hint(text));
        if let Some(sound) = &rules.sound {
            sounds.write(PlaySound::ui(sound.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_without_the_install_reads_as_its_words() {
        assert_eq!(fallback_text("Cstrike_TitlesTXT_Hint_out_of_ammo"), "Out of ammo");
        assert_eq!(fallback_text("#Cstrike_TitlesTXT_Hint_spotted_an_enemy"), "Spotted an enemy");
        assert_eq!(fallback_text("Something"), "Something");
    }
}
