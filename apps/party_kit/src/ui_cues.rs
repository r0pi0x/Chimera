//! The party menus' UI cues (feature `present`;
//! docs/plans/active/ugg-ui.md, "UI sound"): what the main menu and the
//! Escape menu send to the shell's `ui` bus
//! (`chimera_client::client::ui_sound`), which plays them only in an app
//! whose theme turns UI sound on (Ultimate Gun Game's Signal); Lucker
//! Party hears nothing.
//!
//! - **Hover** (`hover::<A>` for each menu's action component, one
//!   `HoverCue` among them): `ui_hover` once per logical item (a row, a
//!   list line, a menu entry, a button; a `CueItem` on a row makes its
//!   parts one item), never again for moving among its parts or back onto
//!   it; at least `HOVER_GAP_SECS` (90 ms) apart, one too soon dropped;
//!   none while a mouse button is held (a slider drag, a pan); the keys'
//!   focus moves cue each step (the screens' own). Its level is 4 dB
//!   under select (`ui_sounds.toml`).
//! - **Menus**: a press (`ui_select`),
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

/// A logical item's hover key (module doc, "Hover"): put on a row or a
/// list line, it makes the pointer's moves among its parts (its label,
/// slider, value box, keys, background) one item, cued once. Without one
/// an entity's item is its action.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct CueItem(pub String);

/// Hover cues at least this far apart, seconds; one too soon is dropped.
pub const HOVER_GAP_SECS: f64 = 0.09;

/// An item not under the pointer this long is left (entering it again
/// cues); shorter covers a page drawn again under a still pointer.
pub const HOVER_FORGET_SECS: f64 = 0.25;

/// The menus' hover cue (module doc), shared by every `hover::<A>`.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct HoverCue {
    /// The item under the pointer, and when it last was.
    pub over: Option<String>,
    pub seen: f64,
    /// The last hover cue.
    pub last: Option<f64>,
}

/// The item an entity belongs to: its own or an ancestor's `CueItem`,
/// else its action.
fn item_of<A: std::fmt::Debug>(e: Entity, a: &A, parents: &Query<&ChildOf>, items: &Query<&CueItem>) -> String {
    let mut at = e;
    for _ in 0..32 {
        if let Ok(i) = items.get(at) {
            return i.0.clone();
        }
        match parents.get(at) {
            Ok(p) => at = p.parent(),
            Err(_) => break,
        }
    }
    format!("{}:{a:?}", std::any::type_name::<A>())
}

/// `ui_hover` when the pointer enters a new logical item carrying `A`
/// (module doc): once per item (`CueItem`), not for its parts, not on a
/// press, at least `HOVER_GAP_SECS` apart (one too soon dropped), none
/// while a mouse button is held (a drag, a pan). Keyed by value, so a
/// menu drawn again under a resting pointer stays quiet.
#[allow(clippy::too_many_arguments)]
pub fn hover<A: Component + std::fmt::Debug>(
    buttons: Query<(Entity, &A, &widgets::Interaction)>,
    parents: Query<&ChildOf>,
    items: Query<&CueItem>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    clock: Option<Res<chimera_ui::UiClock>>,
    time: Option<Res<Time<Real>>>,
    mut st: ResMut<HoverCue>,
    mut out: MessageWriter<UiCue>,
) {
    let now = clock.map(|c| c.now).or_else(|| time.map(|t| t.elapsed_secs_f64())).unwrap_or(0.0);
    let held = mouse.is_some_and(|m| m.get_pressed().next().is_some());
    let mut entered: Option<(String, bool)> = None;
    for (e, a, i) in &buttons {
        if *i == widgets::Interaction::None {
            continue;
        }
        let k = item_of(e, a, &parents, &items);
        if st.over.as_ref() == Some(&k) && now - st.seen <= HOVER_FORGET_SECS {
            // Still on the same item (a part of it, or it drawn again).
            st.seen = now;
            return;
        }
        entered = Some((k, *i == widgets::Interaction::Hovered));
    }
    let Some((k, hovered)) = entered else { return };
    st.over = Some(k);
    st.seen = now;
    if hovered && !held && st.last.is_none_or(|l| now - l >= HOVER_GAP_SECS) {
        st.last = Some(now);
        out.write(UiCue::new("ui_hover"));
    }
}

/// Registers `UiCue` where the shell didn't (tests of a menu alone), and
/// the hover cue's state.
pub fn ensure_message(app: &mut App) {
    if !app.world().contains_resource::<Messages<UiCue>>() {
        app.add_message::<UiCue>();
    }
    app.init_resource::<HoverCue>();
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
