//! The party menus' UI cues (feature `present`;
//! docs/plans/active/ugg-ui.md, "UI sound"): what the main menu and the
//! Escape menu send to the shell's `ui` bus
//! (`chimera_client::client::ui_sound`), which plays them only in an app
//! whose theme turns UI sound on (Ultimate Gun Game's Signal); Lucker
//! Party hears nothing.
//!
//! - **Menus**: the pointer landing on a button (`ui_hover`, one voice:
//!   `hover::<A>` for each menu's action component), a press (`ui_select`),
//!   back, close or cancel and Esc (`ui_back`), a press the menu refuses
//!   (`ui_denied`: while loading, a failure box or a reconnect shows), a
//!   range setting's step (`ui_slider` at the value's place in its range,
//!   `slider_value`). The backdrop swap's sound stays off.
//! - **The surprises**: their cues come from the match's effects
//!   (`chimera_client::client::ui_cues`); under UI sound Combat Arms' air
//!   strike siren stays silent on this client (`mute_siren`), our
//!   `air_alert` standing in for it.

use bevy::prelude::*;
use chimera_client::client::{ui_sound::UiCue, widgets};

use crate::host::Pick;

/// Send `cue` (a world without the message registered skips it).
pub fn cue(world: &mut World, cue: UiCue) {
    if world.contains_resource::<Messages<UiCue>>() {
        world.write_message(cue);
    }
}

/// A range setting's value as its place in the range, 0 to 1 (the
/// slider's rate); None for a choice.
pub fn slider_value(pick: &Pick, value: &str) -> Option<f32> {
    match pick {
        Pick::Range { min, max, .. } if max > min => {
            let v = value.trim().parse::<f32>().ok()?;
            Some(((v - min) / (max - min)).clamp(0.0, 1.0))
        }
        _ => None,
    }
}

/// The cue of a setting stepped to `next`: the slider, or a select.
pub fn step_cue(pick: &Pick, next: &str) -> UiCue {
    match slider_value(pick, next) {
        Some(v) => UiCue::at("ui_slider", v),
        None => UiCue::new("ui_select"),
    }
}

/// `ui_hover` when the pointer lands on a button carrying `A` (not again
/// for the same button, nor on a press's release; a button keyed by its
/// action, so a menu rebuilt under a resting pointer stays quiet).
pub fn hover<A: Component + PartialEq + Clone>(
    buttons: Query<(&A, &widgets::Interaction), Changed<widgets::Interaction>>,
    mut over: Local<Option<A>>,
    mut out: MessageWriter<UiCue>,
) {
    for (a, i) in &buttons {
        let e = a.clone();
        match i {
            widgets::Interaction::Hovered | widgets::Interaction::Pressed => {
                if over.as_ref() != Some(&e) {
                    if *i == widgets::Interaction::Hovered {
                        out.write(UiCue::new("ui_hover"));
                    }
                    *over = Some(e);
                }
            }
            widgets::Interaction::None if over.as_ref() == Some(&e) => *over = None,
            widgets::Interaction::None => {}
        }
    }
}

/// Registers `UiCue` where the shell didn't (tests of a menu alone).
pub fn ensure_message(app: &mut App) {
    if !app.world().contains_resource::<Messages<UiCue>>() {
        app.add_message::<UiCue>();
    }
}

/// Under UI sound the air strike's own alert (`air_alert`, from the
/// match's effects: `chimera_client::client::ui_cues`) stands in for
/// Combat Arms' siren (`surprise::ALARM`), which this client then doesn't
/// play (`MutedSounds`); without UI sound the siren plays as before.
pub fn mute_siren(
    theme: Option<Res<chimera_client::client::ui_sound::Theme>>,
    sounds: Option<Res<chimera_client::client::ui_sound::UiSounds>>,
    muted: Option<Res<chimera_present::audio::live_sound::MutedSounds>>,
    mut commands: Commands,
) {
    let on = chimera_client::client::ui_sound::active(theme.as_deref(), sounds.as_deref());
    let siren = crate::surprise::alarm_entry();
    let has = muted.as_ref().is_some_and(|m| m.0.contains(&siren));
    if on != has {
        let mut list = muted.map(|m| m.0.clone()).unwrap_or_default();
        list.retain(|e| *e != siren);
        if on {
            list.push(siren);
        }
        commands.insert_resource(chimera_present::audio::live_sound::MutedSounds(list));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slider_steps_at_its_place_in_the_range() {
        let r = Pick::Range { min: 0.0, max: 1.0, step: 0.05 };
        assert_eq!(slider_value(&r, "0.25"), Some(0.25));
        assert_eq!(step_cue(&r, "1").value, Some(1.0));
        let c = Pick::Choices(vec![("0".into(), "Off".into())]);
        assert_eq!(step_cue(&c, "0").event, "ui_select");
    }
}
