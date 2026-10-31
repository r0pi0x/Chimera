//! The game HUD's weapon selection (CS:S `HudWeaponSelection`) and pickup
//! history (`HudHistoryResource`), drawn as the game draws them
//! (measured on CS:S captures with `refcmp hudcmp`).
//!
//! Selection: a row of `MaxSlots` boxes centred at the top of the screen,
//! the highlighted slot's box large (`LargeBoxWide` x `LargeBoxTall`) and
//! the others small (`SmallBoxSize`), `BoxGap` apart. Only the large box
//! is drawn; each slot that holds something shows its number in its box's
//! corner (the highlighted slot's bright, the others at half), and the
//! large box the weapon's icon (the `CSweapons` font: the kill icon's
//! letter in capitals; the outline face, `CSType`'s, when it isn't the
//! weapon you hold) and its localised name (`#Cstrike_WPNHUD_*`).
//!
//! The selection's logic is the shell's (`client::weapon_menu`, shared
//! with Combat Arms): slot keys and `slot1`..`slot5`, `invnext` /
//! `invprev` (CS:S's mouse-wheel commands), attack to pick, attack2 or
//! `cancelselect` to close, the selection's sounds (`map::RoundSounds`).
//! CS:S's own here: `hud_fastswitch` (0, its default: slot keys go through
//! the row when its HUD has `HudWeaponSelection`, `takes_slots`; 1: they
//! switch at once and the row shows the drawn weapon for a moment,
//! `show_deploys`), the row's timings (`TIMES`, ours), the no-op
//! `slot6`..`slot10` of its number keys, and the look:
//! `selection_layout`/`history_layout` are pure (tests).
//!
//! History: each weapon you get shows on the right, newest at the bottom
//! of the panel and older ones `history_gap` above, for
//! `HISTORY_SECONDS` (CS:S's `hud_drawhistory_time` default) and fading.
//!
//! Known shortcuts (docs/tech-debt.md): Weapon selection, pickup and buy sounds
//! (no spec; the install's game_sounds.txt entries, when each plays inferred
//! from its name): the HUD selection plays `Player.WeaponSelectionMoveSlot` as
//! it opens or its highlight moves, `Player.WeaponSelected` on a pick and
//! `Player.DenyWeaponSelection` for a slot key on an empty slot (none with
//! `hud_fastswitch 1`; `WeaponSelectionOpen`/`Close` unused); a weapon bought
//! or picked up plays `Player.PickupWeapon` from the player, armour or a kit
//! bought `BaseCombatCharacter.ItemPickup2`. Why it's OK for now: Captures
//! wanted in docs/plans/active/parity-gaps.md.

use std::collections::HashSet;

use bevy::prelude::*;

use super::game_hud::{Drawn, scaled};
use crate::client::{
    fonts::UiFonts,
    hud_text::{Align, GlyphFont, HudText, width},
};
use crate::{
    console::{Console, ConsoleAppExt},
    core::LocalPlayer,
    map::hud::{ActiveHud, GameHud},
    weapon::{Inventory, Weapon, WeaponEvent, WeaponEventKind},
};

// The selection's state and logic: the shell's, at their old paths.
use crate::client::weapon_menu::menu_input;
pub use crate::client::weapon_menu::{
    WeaponMenu, WeaponMenuPlugin, WeaponMenuTimes, next_in_slot, pick_of, step_weapon,
};

/// CS:S's weapon selection: the shell's (`WeaponMenuPlugin`) with CS:S's
/// timings, `hud_fastswitch` and the row and history drawn.
pub struct WeaponSelectPlugin;

impl Plugin for WeaponSelectPlugin {
    fn build(&self, app: &mut App) {
        // The selection's logic and commands are the shell's; CS:S's
        // timings, when slot keys go through the row, and its look here.
        if !app.is_plugin_added::<WeaponMenuPlugin>() {
            app.add_plugins(WeaponMenuPlugin);
        }
        app.insert_resource(TIMES)
            .init_resource::<FastSwitch>()
            .init_resource::<History>()
            .add_systems(
                Update,
                (
                    takes_slots.before(menu_input),
                    (track_pickups, show_deploys, draw)
                        .chain()
                        .after(menu_input)
                        .run_if(resource_exists::<ActiveHud>),
                )
                    .distributive_run_if(crate::gameplay::hud_is(crate::game::GAME)),
            )
            // The look gone (a map without one, or put away for another
            // game's HUD): the row and history come down, slot keys switch
            // at once.
            .add_systems(Update, put_down.run_if(resource_removed::<ActiveHud>));
        crate::console::resource_cvar::<FastSwitch, u8>(
            app,
            "hud_fastswitch",
            "0: a slot key opens the weapon selection, attack takes the weapon (CS:S's default); 1: slot keys \
             switch at once.",
            |f| &mut f.0,
        );
        app.world_mut().resource_mut::<Console>().archive("hud_fastswitch");
        // CS:S's number keys 6-9 and 0 (its default binds): no weapon slot
        // there; the menus read the number keys themselves.
        for n in 6..=10 {
            app.console_command(
                &format!("slot{n}"),
                "Weapon slot (CS:S has no weapons there: does nothing; menus take the number keys).",
                |_, _| Ok(None),
            );
        }
    }
}

/// `hud_fastswitch`.
#[derive(Resource, Default)]
pub struct FastSwitch(pub u8);

/// The row and the history taken down; the slot keys no longer go
/// through the row.
fn put_down(parts: Query<Entity, With<SelectPart>>, mut menu: ResMut<WeaponMenu>, mut commands: Commands) {
    for e in &parts {
        commands.entity(e).despawn();
    }
    menu.takes_slots = false;
}

/// How long the selection shows: open waiting for a pick, showing a fast
/// switch, the last part of either it fades over (ours; the game's timing
/// isn't in its data files), and a pick held in the intent so a fixed
/// tick sees it.
pub const TIMES: WeaponMenuTimes = WeaponMenuTimes {
    menu: 3.0,
    show: 1.5,
    fade: 0.5,
    select_hold: 0.15,
};

/// Seconds a pickup shows (CS:S's `hud_drawhistory_time` default), the
/// last of which it fades over (ours).
const HISTORY_SECONDS: f32 = 5.0;
const HISTORY_FADE: f32 = 1.0;
/// The large box's darkening (measured: half the light behind it).
const LARGE_BOX_ALPHA: f32 = 0.5;
/// Unhighlighted slots' numbers draw at the panel's half alpha.
const DIM: f32 = 0.5;

/// Slot keys go through the row with `hud_fastswitch 0` when CS:S's HUD
/// has it (`HudWeaponSelection`); else they switch at once.
fn takes_slots(mut menu: ResMut<WeaponMenu>, fast: Res<FastSwitch>, hud: Option<Res<ActiveHud>>) {
    menu.takes_slots = fast.0 == 0
        && hud
            .as_ref()
            .is_some_and(|h| h.0.panels.contains_key("HudWeaponSelection"));
}

/// With fast switch on, a weapon drawn shows the row on it for a moment.
fn show_deploys(
    mut events: MessageReader<WeaponEvent>,
    mut menu: ResMut<WeaponMenu>,
    player: Option<Single<Entity, With<LocalPlayer>>>,
) {
    let Some(me) = player.map(|p| *p) else {
        events.clear();
        return;
    };
    for e in events.read() {
        if e.shown() && e.owner == me && matches!(e.kind, WeaponEventKind::Deployed) && !menu.takes_slots {
            menu.highlighted = Some(e.weapon);
            menu.choosing = false;
            menu.shown_for = Some(0.0);
        }
    }
}

/// Weapons picked up, newest last, with seconds shown.
#[derive(Resource, Default)]
struct History {
    items: Vec<(&'static str, f32)>,
    known: HashSet<Entity>,
}

fn track_pickups(
    mut history: ResMut<History>,
    player: Option<Single<&Inventory, With<LocalPlayer>>>,
    weapons: Query<&Weapon>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    history.items.retain_mut(|(_, t)| {
        *t += dt;
        *t < HISTORY_SECONDS
    });
    let Some(inv) = player else {
        history.known.clear();
        return;
    };
    let now: HashSet<Entity> = inv.weapons.iter().copied().collect();
    for e in &inv.weapons {
        if !history.known.contains(e)
            && let Ok(w) = weapons.get(*e)
        {
            history.items.push((w.id, 0.0));
        }
    }
    history.known = now;
}

/// A weapon's short name: `cs_source:weapon_usp` -> `usp`.
fn short(id: &str) -> &str {
    let name = id.rsplit(':').next().unwrap_or(id);
    name.strip_prefix("weapon_").unwrap_or(name)
}

/// The localised name's token of a weapon by short name: its data's
/// `hud.name` (CS:S's `Cstrike_WPNHUD_*`).
fn name_token(short: &str) -> Option<&'static str> {
    crate::game::weapons::data_named(short)?.hud.name.as_deref()
}

/// A weapon's letter in the weapon fonts: its kill icon's (its data's
/// `hud.icon`; `d_<weapon>` without data).
fn letter(hud: &GameHud, short: &str) -> Option<char> {
    match crate::game::weapons::data_named(short).and_then(|d| d.hud.icon.as_deref()) {
        Some(icon) => hud.icons.get(icon),
        None => hud.icons.get(&format!("d_{short}")),
    }
    .map(|(_, c)| *c)
}

/// What the row shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// Slots that hold something.
    pub filled: Vec<u8>,
    /// The highlighted slot and weapon (short name), and whether it is
    /// the weapon held.
    pub highlighted: (u8, &'static str, bool),
}

fn rgba(hud: &GameHud, name: &str, fallback: [u8; 4], alpha: f32) -> Color {
    let c = hud.colors.get(name).copied().unwrap_or(fallback);
    Color::srgba_u8(c[0], c[1], c[2], (c[3] as f32 * alpha).round() as u8)
}

/// The selection row's parts in a `w` x `h` window at opacity `alpha`.
pub fn selection_layout(
    hud: &GameHud,
    font: &dyn Fn(&str) -> Option<(GlyphFont, f32)>,
    w: f32,
    h: f32,
    row: &Row,
    alpha: f32,
) -> Vec<Drawn> {
    let mut out = Vec::new();
    let Some(panel) = hud.panels.get("HudWeaponSelection") else {
        return out;
    };
    let s = h / 480.0;
    let num = |k: &str, d: f32| panel.num(k).unwrap_or(d);
    let small = scaled(num("SmallBoxSize", 60.0), s);
    let (large_w, large_h) = (
        scaled(num("LargeBoxWide", 108.0), s),
        scaled(num("LargeBoxTall", 80.0), s),
    );
    let gap = scaled(num("BoxGap", 8.0), s);
    let slots = num("MaxSlots", 5.0) as u8;
    let (hl_slot, weapon, held) = row.highlighted;
    let widths: Vec<f32> = (0..slots).map(|i| if i == hl_slot { large_w } else { small }).collect();
    let total = widths.iter().sum::<f32>() + gap * (slots.saturating_sub(1)) as f32;
    // Centred across the screen, at the panel's y.
    let mut x = ((w - total) / 2.0).trunc();
    let top = panel.y.resolve(h, s).trunc();
    let number_at = Vec2::new(
        scaled(num("SelectionNumberXPos", 4.0), s),
        scaled(num("SelectionNumberYPos", 4.0), s),
    );
    for (i, bw) in widths.iter().enumerate() {
        let i = i as u8;
        let corner = Vec2::new(x, top);
        if i == hl_slot {
            let rect = Rect::from_corners(corner, corner + Vec2::new(large_w, large_h));
            out.push(Drawn::Box(
                rect,
                Color::srgba(0.0, 0.0, 0.0, LARGE_BOX_ALPHA * alpha),
                4.0 * s,
            ));
            let letter = letter(hud, weapon);
            // The icon: the weapon font's capital, or the outline face when
            // it isn't the weapon held, its cell at (`IconXPos`,
            // `IconYPos`) in the box.
            let face = if held {
                font("CSweapons")
            } else {
                font("CSType").zip(font("CSweapons")).map(|(f, c)| (f.0, c.1))
            };
            if let (Some(c), Some((f, tall))) = (letter, face) {
                let text = if held {
                    c.to_ascii_uppercase()
                } else {
                    c.to_ascii_lowercase()
                }
                .to_string();
                out.push(Drawn::Text(HudText {
                    font: f,
                    tall,
                    color: rgba(hud, "FgColor", [255, 176, 0, 255], alpha),
                    at: Vec2::new(
                        x + scaled(num("IconXPos", 8.0), s),
                        top + scaled(num("IconYPos", 0.0), s),
                    ),
                    align: Align::Left,
                    text,
                }));
            }
            let name = name_token(weapon)
                .and_then(|t| hud.menus.as_ref().map(|m| m.string(t, "").to_string()))
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| weapon.to_uppercase());
            if let Some((f, tall)) = font("HudSelectionText") {
                out.push(Drawn::Text(HudText {
                    font: f,
                    tall,
                    text: name,
                    color: rgba(hud, "SelectionTextFg", [255, 220, 0, 200], alpha),
                    at: Vec2::new(x + (large_w / 2.0).trunc(), top + scaled(num("TextYPos", 68.0), s)),
                    align: Align::Centre,
                }));
            }
        }
        if row.filled.contains(&i)
            && let Some((f, tall)) = font("HudSelectionNumbers")
        {
            let dim = if i == hl_slot { 1.0 } else { DIM };
            out.push(Drawn::Text(HudText {
                font: f,
                tall,
                text: (i + 1).to_string(),
                color: rgba(hud, "SelectionNumberFg", [255, 220, 0, 200], alpha * dim),
                at: corner + number_at,
                align: Align::Left,
            }));
        }
        x += bw + gap;
    }
    out
}

/// The pickup history's icons, `items` oldest first with their opacity.
pub fn history_layout(
    hud: &GameHud,
    font: &dyn Fn(&str) -> Option<(GlyphFont, f32)>,
    w: f32,
    h: f32,
    items: &[(&str, f32)],
) -> Vec<Drawn> {
    let mut out = Vec::new();
    let (Some(panel), Some((f, tall))) = (hud.panels.get("HudHistoryResource"), font("CSweaponsSmall")) else {
        return out;
    };
    let s = h / 480.0;
    let gap = scaled(panel.num("history_gap").unwrap_or(50.0), s);
    let right = (panel.x.resolve(w, s) + panel.wide * s).trunc() - scaled(HISTORY_RIGHT, s);
    let bottom = (panel.y.resolve(h, s) + panel.tall * s).trunc();
    for (i, (weapon, alpha)) in items.iter().rev().enumerate() {
        let Some(c) = letter(hud, weapon) else { continue };
        let text = c.to_ascii_uppercase().to_string();
        let top = bottom - gap * (i as f32 + 1.0);
        if top < 0.0 {
            break;
        }
        out.push(Drawn::Text(HudText {
            font: f.clone(),
            tall,
            color: rgba(hud, "FgColor", [255, 176, 0, 255], *alpha),
            at: Vec2::new(right, top),
            align: Align::InkRight,
            text,
        }));
    }
    out
}

/// The history icons' ink's right edge from the panel's (virtual units;
/// measured on CS:S captures).
const HISTORY_RIGHT: f32 = 20.0;

/// A drawn part, by its place in the drawn list.
#[derive(Component)]
struct SelectPart(usize);

/// Draw the row and the history: one entity per drawn part, reused.
#[allow(clippy::too_many_arguments)]
fn draw(
    hud: Option<Res<ActiveHud>>,
    fonts: Res<UiFonts>,
    menu: Res<WeaponMenu>,
    times: Res<WeaponMenuTimes>,
    history: Res<History>,
    windows: Query<&Window>,
    player: Option<Single<&Inventory, With<LocalPlayer>>>,
    weapons: Query<&Weapon>,
    mut parts: Query<(
        Entity,
        &SelectPart,
        &mut Node,
        Option<&mut HudText>,
        Option<&mut BackgroundColor>,
    )>,
    mut commands: Commands,
) {
    // Gone mid-frame (the map unloaded: back to the menu).
    let Some(hud) = hud else { return };
    let Some(window) = windows.iter().next() else { return };
    let (w, h) = (window.width(), window.height());
    let font = |name: &str| fonts.hud_font(name, h);
    let mut drawn = Vec::new();
    if let (Some(alpha), Some(inv), Some(hl)) = (menu.alpha(&times), player.as_ref(), menu.highlighted)
        && let Ok(hw) = weapons.get(hl)
    {
        let filled: Vec<u8> = inv
            .weapons
            .iter()
            .filter_map(|e| weapons.get(*e).ok())
            .map(|w| w.slot)
            .collect();
        let row = Row {
            filled,
            highlighted: (hw.slot, short(hw.id), inv.active == Some(hl)),
        };
        drawn.extend(selection_layout(&hud.0, &font, w, h, &row, alpha));
    }
    let items: Vec<(&str, f32)> = history
        .items
        .iter()
        .map(|(id, t)| (short(id), ((HISTORY_SECONDS - t) / HISTORY_FADE).clamp(0.0, 1.0)))
        .collect();
    if player.is_some() {
        drawn.extend(history_layout(&hud.0, &font, w, h, &items));
    }
    // Boxes and texts need different entities (a box has a background, a
    // text a material): rebuild when the kinds change, else update.
    let kinds: Vec<bool> = drawn.iter().map(|d| matches!(d, Drawn::Text(_))).collect();
    let mut have: Vec<_> = parts.iter_mut().collect();
    let same_kinds = have.len() == kinds.len() && {
        have.sort_by_key(|(_, i, ..)| i.0);
        have.iter()
            .zip(&kinds)
            .all(|((_, _, _, t, _), is_text)| t.is_some() == *is_text)
    };
    if !same_kinds {
        for (e, ..) in &have {
            commands.entity(*e).despawn();
        }
        for (i, d) in drawn.into_iter().enumerate() {
            let node = Node {
                position_type: PositionType::Absolute,
                ..default()
            };
            match d {
                Drawn::Box(r, c, radius) => commands.spawn((
                    SelectPart(i),
                    Node {
                        left: px(r.min.x),
                        top: px(r.min.y),
                        width: px(r.width()),
                        height: px(r.height()),
                        border_radius: BorderRadius::all(px(radius)),
                        ..node
                    },
                    BackgroundColor(c),
                    GlobalZIndex(42),
                )),
                Drawn::Text(t) => commands.spawn((SelectPart(i), node, t, GlobalZIndex(43))),
            };
        }
        return;
    }
    for ((_, _, node, text, bg), d) in have.iter_mut().zip(drawn) {
        match d {
            Drawn::Box(r, c, radius) => {
                node.left = px(r.min.x);
                node.top = px(r.min.y);
                node.width = px(r.width());
                node.height = px(r.height());
                node.border_radius = BorderRadius::all(px(radius));
                if let Some(bg) = bg
                    && bg.0 != c
                {
                    bg.0 = c;
                }
            }
            Drawn::Text(t) => {
                if let Some(current) = text
                    && **current != t
                {
                    **current = t;
                }
            }
        }
    }
}

/// The width of a text in a scheme font (for tests and layout checks).
pub fn text_width(font: &dyn Fn(&str) -> Option<(GlyphFont, f32)>, name: &str, text: &str) -> Option<f32> {
    font(name).map(|(f, tall)| width(&f.data, tall, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_names_and_tokens() {
        crate::game::weapons::cs_weapons();
        assert_eq!(short("cs_source:weapon_mp5navy"), "mp5navy");
        assert_eq!(name_token("usp"), Some("Cstrike_WPNHUD_USP45"));
        assert_eq!(name_token("hegrenade"), Some("Cstrike_WPNHUD_HE_Grenade"));
        assert_eq!(name_token("nope"), None);
    }
}
