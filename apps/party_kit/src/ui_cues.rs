//! The party menus' and surprises' UI cues (feature `present`;
//! docs/plans/active/ugg-ui.md, "UI sound"): what the main menu, the
//! Escape menu and the host's surprises send to the shell's `ui` bus
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
//! - **The surprises** (`surprises`): the air strike's warning (its HUD
//!   banner, `surprise::warn_air_strike`'s text) plays `air_alert`, a
//!   pulse each second of the warning (`air_pulse`), the banner folding
//!   into its chip at 2 s (`hud_chip`) and `air_impact` when the first
//!   shells land (`surprise::WARNING`); moon gravity's banner `hud_chip`.
//!
//! Known shortcuts: the surprises are read from their HUD text until M3
//! makes them structured (`MatchEffects`), which should send these cues
//! from its effects instead.

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
/// for the same button, nor on a press's release).
pub fn hover<A: Component>(
    buttons: Query<(Entity, &widgets::Interaction), (Changed<widgets::Interaction>, With<A>)>,
    mut over: Local<Option<Entity>>,
    mut out: MessageWriter<UiCue>,
) {
    for (e, i) in &buttons {
        match i {
            widgets::Interaction::Hovered | widgets::Interaction::Pressed => {
                if *over != Some(e) {
                    if *i == widgets::Interaction::Hovered {
                        out.write(UiCue::new("ui_hover"));
                    }
                    *over = Some(e);
                }
            }
            widgets::Interaction::None if *over == Some(e) => *over = None,
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

/// The surprises' cues (module doc): seconds after the warning showed.
#[derive(Default)]
pub struct StrikeSeen {
    /// When the strike's banner and the moon's showed (`HudMessages`' times).
    strike: Option<f64>,
    moon: Option<f64>,
    /// The strike's cues still to send: (seconds after the banner, event).
    due: Vec<(f64, &'static str)>,
}

/// The air strike's cues after its banner: seconds in, event.
pub fn strike_schedule() -> Vec<(f64, &'static str)> {
    let w = crate::surprise::WARNING;
    let mut out = vec![(0.0, "air_alert")];
    let mut t = 1.0;
    while t < w - 0.05 {
        out.push((t, "air_pulse"));
        t += 1.0;
    }
    out.push((2.0_f64.min(w), "hud_chip"));
    out.push((w, "air_impact"));
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// The surprises' banners as they show on this client (module doc).
pub fn surprises(
    hud: Option<Res<chimera_world::map::logic_view::HudMessages>>,
    time: Res<Time>,
    mut seen: Local<StrikeSeen>,
    mut out: MessageWriter<UiCue>,
) {
    let now = time.elapsed_secs_f64();
    if let Some(hud) = hud.as_deref() {
        let shown = |pred: &dyn Fn(&str) -> bool| {
            hud.channels.iter().flatten().filter(|(m, _)| pred(&m.text)).map(|(_, at)| *at).fold(None, |a: Option<f64>, t| {
                Some(a.map_or(t, |a| a.max(t)))
            })
        };
        let strike = shown(&|t| t.starts_with("AIR STRIKE"));
        if strike.is_some() && strike != seen.strike {
            let at = strike.unwrap_or(now);
            seen.due = strike_schedule().into_iter().map(|(dt, e)| (at + dt, e)).collect();
        }
        seen.strike = strike.or(seen.strike);
        let moon = shown(&|t| t.starts_with("Moon gravity"));
        if moon.is_some() && moon != seen.moon {
            out.write(UiCue::new("hud_chip"));
        }
        seen.moon = moon.or(seen.moon);
    }
    while seen.due.first().is_some_and(|(t, _)| *t <= now) {
        let (_, e) = seen.due.remove(0);
        out.write(UiCue::new(e));
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

    #[test]
    fn the_strike_alerts_pulses_folds_and_lands() {
        let s = strike_schedule();
        assert_eq!(s.first(), Some(&(0.0, "air_alert")));
        assert_eq!(s.last().map(|e| e.1), Some("air_impact"));
        assert!(s.iter().any(|e| e.1 == "air_pulse") && s.iter().any(|e| e.1 == "hud_chip"));
    }
}
