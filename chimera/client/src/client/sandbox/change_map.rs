//! Change map (M in the sandbox; docs/plans/completed/shared-picker.md, step
//! 4): the kit's shared picker (`chimera_ui::picker`, single pick, the
//! List look) over the whole screen, every map the sandbox can go to by
//! game, the one it plays on in the accent.
//!
//! - **Data** (`MapCards`, made when the view opens: `refresh`): the
//!   sandbox's own first (the range, the greybox), then each playable
//!   game's maps (the ladder's report, `auto::report`: its claimed maps
//!   and the installed ones its pool rules allow), by the games' full
//!   names (`super::game_name`).
//! - **Picking** a map (a click on its row, or Enter on the cursor's)
//!   submits `sandbox <map>`, the console command the main menu's Sandbox
//!   entry runs (Ultimate Gun Game's `sandbox`, f2132a134: no lobby, no
//!   end screen), and closes the view; the map it plays on already does
//!   nothing but close it (the command says so).
//! - **Keys** (`keys`): ↑↓ the row, ←→ fold or open a game (`picker::
//!   nav`), Enter picks, `/` or a click puts the keyboard in the search
//!   field (typing: the kit's text field; Enter or Esc lets go of it), a
//!   game's chip keeps its maps, M (the sandbox's `keys`) or Esc closes
//!   it. Opened from the sandbox by M (beside B, Change gun), the
//!   Escape menu's Change map, or `sandbox_map`.
//!
//! Known shortcuts: the view is built anew on each change (a few dozen
//! rows; Change gun's grid is updated in place instead); no map pictures
//! (a row a map).

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    ui::DeprecatedInteraction as Interaction,
};
use chimera_ui::{
    Role, Tone, UiParityId, UiStyle, VNav, VirtualLists,
    controls::Ui,
    picker::{self, PickerAction, PickerFrame, PickerGame, PickerGroup, PickerItem, PickerLook, PickerMode, PickerState, Pickers},
};

use super::{SandboxAction, SandboxScreen, SandboxUi, hud::kit_icon};
use crate::client::signal::Look;

/// The list's key (`VirtualLists`, `picker::Pickers`).
pub const LIST: &str = "sandbox.maps";

/// The sandbox's own maps' group (the range, the greybox).
pub const OWN: &str = "sandbox";

/// The picker's state (single pick: `current` the map in play).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MapPicker(pub PickerState);

/// The maps by game (`refresh`).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MapCards(pub Vec<PickerGame>);

/// The view's root.
#[derive(Component, Clone, Copy, Debug)]
pub struct ChangeMapView;

/// The maps of `caps`' games (`game:map`), after the sandbox's own.
pub fn games_of(caps: &[chimera_sim::minigame::auto::Capabilities]) -> Vec<PickerGame> {
    let item = |id: &str| PickerItem::new(id, id.rsplit(':').next().unwrap_or(id));
    let mut out = vec![PickerGame {
        id: OWN.into(),
        name: "Sandbox".into(),
        groups: vec![PickerGroup { name: String::new(), items: vec![PickerItem::new("range", "range"), PickerItem::new("greybox", "greybox")] }],
    }];
    out.extend(caps.iter().filter(|c| !c.maps.is_empty()).map(|c| PickerGame {
        id: c.game.to_string(),
        name: super::game_name(c.game),
        groups: vec![PickerGroup { name: String::new(), items: c.maps.iter().map(|m| item(m)).collect() }],
    }));
    out
}

/// Lists the maps again and marks the one in play (the view opens).
pub fn refresh(world: &mut World) {
    let mut caps = chimera_sim::minigame::auto::report(world);
    caps.retain(|c| c.playable());
    let games = games_of(&caps);
    let here = chimera_net::net::current_map(world);
    world.insert_resource(MapCards(games));
    let mut p = world.get_resource_or_init::<MapPicker>();
    p.0 = PickerState { current: Some(here), ..default() };
}

/// The frame: full screen, single pick, rows, the search and the games'
/// chips.
pub fn frame() -> PickerFrame {
    PickerFrame {
        w: 100.0 - 2.0 * super::hud::M,
        look: PickerLook::List,
        mode: PickerMode::Single,
        search: true,
        facets: vec!["game".into()],
        placeholder: "Find a map or a game".into(),
        ..default()
    }
}

/// The keys while open (module doc).
#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut typed: MessageReader<KeyboardInput>,
    owned: Option<Res<crate::client::binds::OwnedKeys>>,
    console: Option<Res<crate::client::console::ConsoleUi>>,
    mut ui: ResMut<SandboxUi>,
    mut p: ResMut<MapPicker>,
    pickers: Res<Pickers>,
    mut lists: ResMut<VirtualLists>,
    mut out: MessageWriter<PickerAction>,
    mut close: MessageWriter<SandboxAction>,
    (mut own, shared): (Local<chimera_ui::clipboard::Clipboard>, Option<ResMut<chimera_ui::clipboard::Clipboard>>),
) {
    if ui.screen != SandboxScreen::ChangeMap || console.is_some_and(|c| c.open) {
        typed.clear();
        return;
    }
    let k = |c: KeyCode| owned.as_deref().map_or_else(|| keys.just_pressed(c), |o| o.just_pressed(&keys, c));
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let enter = k(KeyCode::Enter) || k(KeyCode::NumpadEnter);
    if k(KeyCode::Escape) {
        typed.clear();
        ui.took_esc = true;
        if p.0.search_focus {
            p.0.search_focus = false;
        } else if !p.0.search.is_empty() {
            p.0.clear_search();
        } else {
            close.write(SandboxAction::Close);
        }
        return;
    }
    let focus = p.0.search_focus;
    if focus && enter {
        p.0.search_focus = false;
    } else if enter {
        out.write(PickerAction::Pick(p.0.cursor));
    }
    let clipboard = match shared {
        Some(c) => c.into_inner(),
        None => &mut *own,
    };
    for e in typed.read() {
        if e.state != ButtonState::Pressed {
            continue;
        }
        // (The field's state per key: `/` and what follows it come in
        // one frame.)
        if p.0.search_focus {
            if let Some(key) = chimera_ui::text_edit::text_key(&e.logical_key, e.key_code, ctrl, shift) {
                p.0.search_key(key, clipboard);
            }
        } else if matches!(&e.logical_key, Key::Character(c) if c.as_str() == "/") {
            p.0.focus_search();
        }
    }
    let Some(view) = pickers.0.get(LIST) else { return };
    for (key, nav) in [(KeyCode::ArrowUp, VNav::Up), (KeyCode::ArrowDown, VNav::Down), (KeyCode::ArrowLeft, VNav::Left), (KeyCode::ArrowRight, VNav::Right), (KeyCode::PageUp, VNav::PageUp), (KeyCode::PageDown, VNav::PageDown)] {
        if k(key) && !(focus && matches!(nav, VNav::Left | VNav::Right)) {
            picker::nav(&mut p.0, view, lists.state(LIST), nav);
        }
    }
}

/// Clicks (and the keys' picks): a row picks (module doc, "Picking"), a
/// header folds, a chip ticks, the search field takes the keyboard.
#[allow(clippy::too_many_arguments)]
fn act(
    pressed: Query<(&Interaction, &PickerAction), Changed<Interaction>>,
    mut msgs: ResMut<Messages<PickerAction>>,
    mut ui: ResMut<SandboxUi>,
    mut p: ResMut<MapPicker>,
    cards: Res<MapCards>,
    pickers: Res<Pickers>,
    mut c: Commands,
) {
    if ui.screen != SandboxScreen::ChangeMap {
        return;
    }
    let mut todo: Vec<PickerAction> = msgs.drain().collect();
    todo.extend(pressed.iter().filter(|(i, _)| **i == Interaction::Pressed).map(|(_, a)| *a));
    let games = &cards.0;
    for a in todo {
        let rows = p.0.rows(games);
        match a {
            PickerAction::Pick(i) => {
                let Some(id) = rows.get(i).and_then(|r| PickerState::item(games, r)).map(|it| it.id.clone()) else { continue };
                ui.screen = SandboxScreen::None;
                c.queue(move |w: &mut World| {
                    if let Some(mut console) = w.get_resource_mut::<chimera_core::console::Console>() {
                        console.submit(format!("sandbox {id}"));
                    }
                });
                return;
            }
            PickerAction::Fold(i) => {
                if let Some(r) = rows.get(i) {
                    let open = p.0.is_folded(games, r);
                    p.0.cursor = i;
                    p.0.fold(games, r, open);
                }
            }
            PickerAction::Chip(i) => {
                if let Some(chip) = pickers.0.get(LIST).and_then(|v| v.chips.get(i)) {
                    if !p.0.chips.remove(chip) {
                        p.0.chips.insert(chip.clone());
                    }
                    p.0.cursor = 0;
                }
            }
            PickerAction::FocusSearch => p.0.focus_search(),
            PickerAction::ClearSearch => {
                p.0.clear_search();
                p.0.search_focus = true;
            }
            PickerAction::Toggle(_) | PickerAction::Done | PickerAction::Cancel | PickerAction::AllOff => {}
        }
    }
}

/// Builds the view again when what it shows changed (module doc, "Known
/// shortcuts").
#[allow(clippy::too_many_arguments)]
fn draw(
    look: Look,
    ui_state: Res<SandboxUi>,
    p: Res<MapPicker>,
    cards: Res<MapCards>,
    roots: Query<Entity, With<ChangeMapView>>,
    mut lists: ResMut<VirtualLists>,
    mut views: ResMut<chimera_ui::ScrollViews>,
    mut last: Local<Option<(MapPicker, MapCards, (u32, u32))>>,
    mut c: Commands,
) {
    let want = look.on() && ui_state.screen == SandboxScreen::ChangeMap;
    let size = (look.units.height as u32, (look.units.scale_factor * 100.0) as u32);
    let next = (p.clone(), cards.clone(), size);
    if want && roots.iter().next().is_some() && last.as_ref() == Some(&next) {
        return;
    }
    for r in &roots {
        c.entity(r).despawn();
    }
    if !want {
        if last.take().is_some() {
            views.forget(LIST);
            c.queue(|w: &mut World| {
                w.get_resource_or_init::<Pickers>().0.remove(LIST);
            });
        }
        return;
    }
    *last = Some(next);
    let ui = Ui::new(&look.theme, look.fonts.as_deref(), *look.units);
    let m = super::hud::M;
    let root = c
        .spawn((
            UiStyle { gap: Some(0.9), pad: Some([1.8, m, 1.2, m].into()), ..UiStyle::column() }.abs([Some(0.0), Some(0.0), Some(0.0), Some(0.0)]).bg_a(Tone::Ink, 0.97),
            Node::default(),
            GlobalZIndex(super::change_gun::Z),
            chimera_ui::controls::catch_pointer(),
            ChangeMapView,
            UiParityId::new("map.view"),
            Name::new("Change map"),
        ))
        .id();
    let head = ui.boxed(&mut c, root, UiStyle { gap: Some(0.35), shrink: Some(0.0), ..UiStyle::column() });
    let back = ui.boxed(&mut c, head, UiStyle { gap: Some(0.45), align: Some(AlignItems::Center), ..UiStyle::row() });
    kit_icon(&mut c, &look, back, "back", 0.5, Tone::Fg2, 1.0);
    ui.text(&mut c, back, Role::Mono, 0.52, Tone::Fg2, "Sandbox");
    ui.kbd(&mut c, back, "Esc", 0.5, Tone::Fg3);
    let title = ui.boxed(&mut c, head, UiStyle { gap: Some(1.2), align: Some(AlignItems::Baseline), ..UiStyle::row() });
    let t = ui.text(&mut c, title, Role::DisplayBold, 1.9, Tone::Fg, "Change map");
    c.entity(t).insert(UiParityId::new("map.title"));
    let on = p.0.current.clone().unwrap_or_default();
    ui.text(&mut c, title, Role::Mono, 0.56, Tone::Fg2, &format!("On {on}"));
    let body = ui.boxed(&mut c, root, UiStyle { gap: Some(0.6), grow: Some(1.0), ..UiStyle::column() });
    c.entity(body).insert(Node { min_height: Val::Px(0.0), ..default() });
    picker::spawn(&mut c, &ui, body, LIST, &cards.0, &p.0, frame(), &mut lists, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    let foot = ui.boxed(&mut c, root, UiStyle { gap: Some(1.1), align: Some(AlignItems::Center), shrink: Some(0.0), ..UiStyle::row() });
    for (k, w) in [("\u{2191}\u{2193}", "Map"), ("\u{2190}\u{2192}", "Fold / open a game"), ("Enter", "Go there"), ("/", "Search (click it too)"), ("M", "Close"), ("Esc", "Leave the search, then back")] {
        super::hud::key_hint(&mut c, &ui, foot, k, w, 0.5, Tone::Fg2);
    }
}

/// The pointer in the search field (`picker::SearchPointer`).
fn search_pointer(pointer: chimera_ui::picker::SearchPointer, ui: Res<SandboxUi>, mut p: ResMut<MapPicker>, mut press: Local<chimera_ui::controls::TextPress>) {
    if ui.screen == SandboxScreen::ChangeMap {
        pointer.run(LIST, &mut p.0, &mut press);
    } else {
        press.release();
    }
}

/// The view's systems.
pub fn add(app: &mut App) {
    app.init_resource::<MapPicker>().init_resource::<MapCards>().init_resource::<Pickers>().add_message::<PickerAction>().add_systems(
        Update,
        (keys, search_pointer, act, draw)
            .chain()
            .after(super::SandboxSystems)
            .run_if(resource_exists::<chimera_ui::Theme>)
            .run_if(resource_exists::<chimera_ui::UiUnits>),
    );
}

/// Made-up maps (`ui_scene sandbox map`).
pub fn staged_maps() -> Vec<PickerGame> {
    use chimera_sim::minigame::auto::Capabilities;
    let caps = |game: &'static str, maps: &[&str]| Capabilities {
        game,
        guns: Vec::new(),
        grenades: Vec::new(),
        knife: None,
        movement: None,
        characters: None,
        hud: None,
        maps: maps.iter().map(|m| format!("{game}:{m}")).collect(),
        missing: Vec::new(),
    };
    games_of(&[
        caps("cs_source", &["de_dust2", "de_inferno", "de_nuke", "cs_office", "cs_italy"]),
        caps("hl2dm", &["dm_lockdown", "dm_overwatch", "dm_steamlab"]),
        caps("tf2", &["ctf_2fort", "cp_dustbowl"]),
    ])
}
