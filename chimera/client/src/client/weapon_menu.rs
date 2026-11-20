//! Weapon selection for any game (`WeaponMenuPlugin`): the state of a HUD's
//! selection row (`WeaponMenu`) and the logic that drives it, without its
//! look. The shell's input reads `WeaponMenu` for the intent's pick and to
//! hold fire after a pick; a game's HUD draws the row from it (CS:S's:
//! its front end's `weapon_select`) and says whether slot keys go through
//! the row (`WeaponMenu::takes_slots`, set each frame before `menu_input`;
//! CS:S: `hud_fastswitch 0` and its HUD has the panel). Combat Arms uses
//! it with no row.
//!
//! The commands `slot1`..`slot5` (`SLOTS`), `invnext` / `invprev` (the
//! mouse wheel's) and `cancelselect`. With the row taking slots, a slot key
//! opens it on that slot (again: the next weapon in it, `next_in_slot`),
//! `invnext` / `invprev` move the highlight through every weapon held,
//! slot by slot (`step_weapon`), attack takes the highlighted weapon (the
//! intent's pick, `core::Intent::pick`: slot and place in it, so the
//! second grenade can be taken; `pick_of`) and keeps that attack from
//! firing, attack2 closes it; it closes itself after a while
//! (`WeaponMenuTimes`, the game's). Else slot keys switch directly (the
//! shell's input polls their binds) and `invnext` / `invprev` switch from
//! the weapon in hand. The selection plays the map's selection sounds
//! (`map::RoundSounds`): a move of the highlight, a pick, a slot key on an
//! empty slot.

use bevy::{prelude::*, window::CursorOptions};

use crate::{
    console::{Console, ConsoleAppExt},
    core::LocalPlayer,
    weapon::{Inventory, Weapon},
};

/// Weapon slot keys (`slot1`..`slot5`).
pub const SLOTS: u8 = 5;

/// The selection's state and commands, and `menu_input` before the local
/// input reads it. The game's HUD draws the row and sets `takes_slots`.
pub struct WeaponMenuPlugin;

impl Plugin for WeaponMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponMenu>()
            .init_resource::<WeaponMenuTimes>()
            .add_systems(RunFixedMainLoop, menu_input.in_set(crate::client::input::LocalInputSystems::Menus));
        crate::client::input::configure_local_input(app);
        for slot in 0..SLOTS {
            app.console_command(
                &format!("slot{}", slot + 1),
                "Weapon slot (with hud_fastswitch 0: open the weapon selection on it).",
                move |w, _| {
                    w.resource_mut::<WeaponMenu>().pressed.push(slot);
                    Ok(None)
                },
            );
        }
        for (name, step, help) in [
            ("invnext", 1, "The next weapon held (with hud_fastswitch 0: in the weapon selection)."),
            ("invprev", -1, "The previous weapon held (with hud_fastswitch 0: in the weapon selection)."),
        ] {
            app.console_command(name, help, move |w, _| {
                w.resource_mut::<WeaponMenu>().steps.push(step);
                Ok(None)
            });
        }
        app.console_command("cancelselect", "Close the weapon selection without picking.", |w, _| {
            w.resource_mut::<WeaponMenu>().cancel();
            Ok(None)
        });
    }
}

/// The selection's state.
#[derive(Resource, Default)]
pub struct WeaponMenu {
    /// The highlighted weapon while the row shows.
    pub highlighted: Option<Entity>,
    /// Seconds the row has shown; None when hidden.
    pub shown_for: Option<f32>,
    /// Open for a pick (fast switch off), not just showing a switch.
    pub choosing: bool,
    /// The slot or pick (`core::Intent::select`) to select and how long
    /// it is still held in the intent.
    pub select: Option<(u8, f32)>,
    /// Attack has picked a weapon and is still held: no firing until it
    /// is let go.
    pub swallow_fire: bool,
    /// Slot keys work through the selection (fast switch off and the game's
    /// HUD has one); else they switch directly. The game sets it.
    pub takes_slots: bool,
    /// Slot commands run since last frame (console, binds of `slotN`).
    pub pressed: Vec<u8>,
    /// `invnext` (+1) and `invprev` (-1) run since last frame.
    pub steps: Vec<i8>,
}

impl WeaponMenu {
    /// The slot the input should select this frame.
    pub fn selected_slot(&self) -> Option<u8> {
        self.select.map(|s| s.0)
    }

    /// Close the selection row, picking nothing (`cancelselect`).
    pub fn cancel(&mut self) {
        self.shown_for = None;
        self.choosing = false;
        self.highlighted = None;
    }

    /// The row's opacity now (1 while showing, fading at the end), None
    /// when hidden.
    pub fn alpha(&self, times: &WeaponMenuTimes) -> Option<f32> {
        let t = self.shown_for?;
        let total = if self.choosing { times.menu } else { times.show };
        let left = total - t;
        (left > 0.0).then(|| (left / times.fade).min(1.0))
    }
}

/// How long the selection shows, the game's (CS:S inserts its own).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct WeaponMenuTimes {
    /// Seconds the row stays open waiting for a pick.
    pub menu: f32,
    /// Seconds it shows a fast switch.
    pub show: f32,
    /// The last part of either it fades over.
    pub fade: f32,
    /// Seconds a picked weapon's slot is held in the intent, so a fixed
    /// tick sees it.
    pub select_hold: f32,
}

impl Default for WeaponMenuTimes {
    fn default() -> Self {
        Self {
            menu: 3.0,
            show: 1.5,
            fade: 0.5,
            select_hold: 0.15,
        }
    }
}

/// What a slot key does with the selection open on `current` (or closed):
/// the first weapon in `slot`, or the next one in it after `current`;
/// `held` lists (weapon, slot) in inventory order.
pub fn next_in_slot(held: &[(Entity, u8)], slot: u8, current: Option<Entity>) -> Option<Entity> {
    let in_slot: Vec<Entity> = held.iter().filter(|(_, s)| *s == slot).map(|(e, _)| *e).collect();
    match current.and_then(|c| in_slot.iter().position(|e| *e == c)) {
        Some(i) => Some(in_slot[(i + 1) % in_slot.len()]),
        None => in_slot.first().copied(),
    }
}

/// What `invnext` (`step` 1) or `invprev` (-1) moves to from `current`:
/// the weapons held ordered by slot, then inventory order, wrapping; from
/// nothing, the first (or last).
pub fn step_weapon(held: &[(Entity, u8)], current: Option<Entity>, step: i8) -> Option<Entity> {
    let mut order: Vec<(Entity, u8)> = held.to_vec();
    order.sort_by_key(|(_, s)| *s);
    let n = order.len() as i32;
    if n == 0 {
        return None;
    }
    let i = match current.and_then(|c| order.iter().position(|(e, _)| *e == c)) {
        Some(i) => (i as i32 + step as i32).rem_euclid(n),
        None if step < 0 => n - 1,
        None => 0,
    };
    Some(order[i as usize].0)
}

/// The intent's pick of `weapon` (its slot and place in it), if held.
pub fn pick_of(held: &[(Entity, u8)], weapon: Entity) -> Option<u8> {
    let slot = held.iter().find(|(e, _)| *e == weapon)?.1;
    let nth = held.iter().filter(|(_, s)| *s == slot).position(|(e, _)| *e == weapon)?;
    Some(crate::core::Intent::pick(slot, nth as u8))
}

/// Drive the selection from the slot commands, the steps and attack /
/// attack2 (see the module docs).
#[allow(clippy::too_many_arguments)]
pub fn menu_input(
    mut menu: ResMut<WeaponMenu>,
    times: Res<WeaponMenuTimes>,
    console: Option<Res<Console>>,
    (keys, mouse): (Res<ButtonInput<KeyCode>>, Res<ButtonInput<MouseButton>>),
    cursor: Query<&CursorOptions>,
    player: Option<Single<&Inventory, With<LocalPlayer>>>,
    weapons: Query<&Weapon>,
    panels: Option<Res<crate::client::front_end::Panels>>,
    captured: Option<Res<crate::client::binds::CapturedKeys>>,
    time: Res<Time>,
    (sounds, mut play): (Option<Res<crate::map::RoundSounds>>, MessageWriter<crate::map::PlaySound>),
) {
    let dt = time.delta_secs();
    let mut sound = |pick: fn(&crate::map::RoundSounds) -> &Option<String>| {
        if let Some(entry) = sounds.as_ref().and_then(|s| pick(s).clone()) {
            play.write(crate::map::PlaySound::ui(entry));
        }
    };
    if let Some((slot, left)) = menu.select {
        menu.select = (left > dt).then_some((slot, left - dt));
    }
    if let Some(t) = menu.shown_for.as_mut() {
        *t += dt;
    }
    if menu.shown_for.is_some() && menu.alpha(&times).is_none() {
        menu.shown_for = None;
        menu.choosing = false;
    }
    let empty = std::collections::BTreeMap::new();
    let binds = console.as_ref().map_or(&empty, |c| &c.binds);
    let playing = cursor.iter().next().is_some_and(crate::client::input::cursor_grabbed)
        && !panels.is_some_and(|p| p.number_keys_taken());
    let just =
        |c: &str| playing && crate::client::binds::just_pressed_free(binds, &keys, &mouse, captured.as_deref(), c);
    let attack_held =
        playing && crate::client::binds::pressed_free(binds, &keys, &mouse, captured.as_deref(), "+attack");
    if !attack_held {
        menu.swallow_fire = false;
    }
    let mut pressed = std::mem::take(&mut menu.pressed);
    let steps = std::mem::take(&mut menu.steps);
    let held: Vec<(Entity, u8)> = player
        .as_ref()
        .map(|inv| {
            inv.weapons
                .iter()
                .filter_map(|e| weapons.get(*e).ok().map(|w| (*e, w.slot)))
                .collect()
        })
        .unwrap_or_default();
    let hold = times.select_hold;
    if menu.takes_slots {
        pressed.extend((0..SLOTS).filter(|s| just(&format!("slot{}", s + 1))));
    } else {
        if let Some(&slot) = pressed.last() {
            // Fast switch: the command switches directly, like the key.
            menu.select = Some((slot, hold));
            return;
        }
        // invnext / invprev switch directly from the weapon in hand.
        let mut current = player.as_ref().and_then(|i| i.active);
        for step in &steps {
            current = step_weapon(&held, current, *step);
        }
        if !steps.is_empty()
            && let Some(pick) = current.and_then(|w| pick_of(&held, w))
        {
            menu.select = Some((pick, hold));
        }
        return;
    }
    if player.is_none() {
        return;
    }
    for slot in pressed {
        let current = menu.highlighted.filter(|_| menu.choosing);
        if let Some(next) = next_in_slot(&held, slot, current) {
            menu.highlighted = Some(next);
            menu.choosing = true;
            menu.shown_for = Some(0.0);
            sound(|s| &s.weapon_move);
        } else {
            sound(|s| &s.weapon_deny);
        }
    }
    for step in steps {
        // From the highlight while the row is open, else the weapon in hand.
        let current = menu
            .highlighted
            .filter(|_| menu.choosing && menu.shown_for.is_some())
            .or(player.as_ref().and_then(|i| i.active));
        if let Some(next) = step_weapon(&held, current, step) {
            menu.highlighted = Some(next);
            menu.choosing = true;
            menu.shown_for = Some(0.0);
            sound(|s| &s.weapon_move);
        }
    }
    if menu.choosing && menu.shown_for.is_some() {
        if just("+attack") {
            if let Some(pick) = menu.highlighted.and_then(|e| pick_of(&held, e)) {
                menu.select = Some((pick, hold));
                sound(|s| &s.weapon_pick);
            }
            menu.swallow_fire = true;
            menu.shown_for = None;
            menu.choosing = false;
        } else if just("+attack2") {
            menu.shown_for = None;
            menu.choosing = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelselect_closes_the_selection_without_a_pick() {
        let mut m = WeaponMenu {
            highlighted: Some(Entity::from_raw_u32(4).unwrap()),
            shown_for: Some(0.5),
            choosing: true,
            ..Default::default()
        };
        m.cancel();
        assert_eq!((m.highlighted, m.shown_for, m.choosing), (None, None, false));
        assert_eq!(m.select, None, "nothing picked");
    }

    #[test]
    fn slot_keys_pick_the_first_then_cycle() {
        let (a, b, c) = (
            Entity::from_raw_u32(1).unwrap(),
            Entity::from_raw_u32(2).unwrap(),
            Entity::from_raw_u32(3).unwrap(),
        );
        let held = [(a, 0), (b, 3), (c, 3)];
        assert_eq!(next_in_slot(&held, 0, None), Some(a));
        assert_eq!(next_in_slot(&held, 3, None), Some(b));
        assert_eq!(next_in_slot(&held, 3, Some(b)), Some(c));
        assert_eq!(next_in_slot(&held, 3, Some(c)), Some(b));
        // Open on another slot: that slot's first.
        assert_eq!(next_in_slot(&held, 3, Some(a)), Some(b));
        assert_eq!(next_in_slot(&held, 1, None), None);
    }

    #[test]
    fn inventory_steps_go_slot_by_slot_and_wrap() {
        let [knife, ak, usp, he, flash] = [1, 2, 3, 4, 5].map(Entity::from_raw_u32).map(Option::unwrap);
        // Inventory order isn't slot order; grenades share slot 3.
        let held = [(knife, 2), (ak, 0), (usp, 1), (he, 3), (flash, 3)];
        assert_eq!(step_weapon(&held, Some(ak), 1), Some(usp));
        assert_eq!(step_weapon(&held, Some(usp), 1), Some(knife));
        assert_eq!(step_weapon(&held, Some(he), 1), Some(flash));
        assert_eq!(step_weapon(&held, Some(flash), 1), Some(ak), "wraps");
        assert_eq!(step_weapon(&held, Some(ak), -1), Some(flash), "wraps back");
        assert_eq!(step_weapon(&held, None, 1), Some(ak));
        assert_eq!(step_weapon(&held, None, -1), Some(flash));
        assert_eq!(step_weapon(&[], None, 1), None);
        // The flashbang is the second weapon in its slot.
        assert_eq!(pick_of(&held, flash), Some(crate::core::Intent::pick(3, 1)));
        assert_eq!(crate::core::Intent::picked(crate::core::Intent::pick(3, 1)), (3, Some(1)));
        assert_eq!(crate::core::Intent::picked(3), (3, None));
    }

    #[test]
    fn the_row_fades_at_the_end() {
        let times = WeaponMenuTimes::default();
        let mut m = WeaponMenu {
            shown_for: Some(0.0),
            choosing: true,
            ..default()
        };
        assert_eq!(m.alpha(&times), Some(1.0));
        m.shown_for = Some(times.menu - times.fade / 2.0);
        assert!((m.alpha(&times).unwrap() - 0.5).abs() < 1e-4);
        m.shown_for = Some(times.menu);
        assert_eq!(m.alpha(&times), None);
        m.choosing = false;
        m.shown_for = Some(times.show - 0.01);
        assert!(m.alpha(&times).is_some());
    }
}
