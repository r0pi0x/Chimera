//! Change gun (B in the sandbox): the full-screen tile view, option B of
//! revision 2 as decided (the design rules, section 21; `sb2-gun-b`,
//! `sb2-gun-filters`, `sb2-gun-tf2`).
//!
//! - **Head**: the breadcrumb (Sandbox, B, Esc) and CHANGE GUN at the
//!   left (no current-gun chip in this view). The search is the picker's
//!   own bar over the grid (`PickerFrame::search`): `/` or a click focuses
//!   it and typing goes there, matching a gun's name, id, game (long and
//!   short), class and tags (`model::search_text`, which the cards carry
//!   as `PickerItem::search_text`, so the picker's matching is the
//!   screen's wider one).
//! - **Games** (left, 14 cq): All games and each game with its count (of
//!   the guns the other filters leave) and a line under it (how many are
//!   here, or one download for the whole game); a click ticks a game
//!   (white), All games clears them. A ticked game with facets of its own
//!   shows them under it as tick tags (Team Fortress 2: the classes).
//! - **The grid** is the kit's shared picker (`chimera_ui::picker`,
//!   docs/plans/completed/shared-picker.md, step 3): single pick, the Cards
//!   look, its columns and card size from the grid's width and the
//!   theme's `[layout.picker]`; a picker game a class (`GunCards`, from
//!   `GunView::tiles`), folding under its header (the list's folds, read
//!   back into the picker's state, `picker::folds_of`). A card has its
//!   game's short name (the badge), its pinned slot (the mark), the gun's
//!   icon (`PickerPart::Art`, filled by `fill`) and name, and its foot:
//!   its delivery and fire mode, or its download state (`PickerPart::
//!   Foot`, `tile_foot`), a download's bar along its bottom edge (the
//!   card's `progress`). The variants of one gun are one card (`model`,
//!   "Families": Combat Arms' skins, editions and levels), saying how many
//!   beside the game's name (the card's `family`); the counts (the
//!   games', the classes', the bar's) are cards. A press selects, a
//!   second press switches (the kit's double-click rule); a header folds.
//!   Over the grid: TAGS (the global tag popover, eight groups as tick
//!   tags, and More filters greyed: Later; while it is open typing goes
//!   to its own "Find a tag", which keeps the tags holding the words,
//!   Enter ticks the first, Esc clears it, then closes the popover), the ticked tags with their
//!   crosses, and the count.
//! - **Detail** (the picker's side pane, 25 cq): the game, the name with the kit's icon,
//!   the 3D model's area (here: a stub picture of the gun, drag to turn;
//!   not here: Download to see the model; downloading: the progress),
//!   the stats (damage a hit, fire rate, magazine, range feel), hits to
//!   kill (a 100-point body, a Spartan's 115 of shield and health, the
//!   head), for a gun several classes carry the class to play it as
//!   (Play as: `GunView::body`, sent with the switch), the tags and the
//!   files. For a tile of several variants, the variants the filters
//!   leave as a list under the model (the base gun first; a press picks
//!   one: the pane, a pin and the switch are then that variant's; Tab and
//!   Shift+Tab step through them).
//! - **Foot**: the keys, and the pinned bar (smaller, at the right): drag
//!   a tile onto a slot (the slot under the pointer shows a ghost of the
//!   gun in an accent frame, the gun it replaces struck through under it,
//!   and the hint beside the bar says "Release: <gun> to <n>[, replacing
//!   <old>]": `model::drop_hint`), or Ctrl+1–6 pins the selected gun.
//! - **Keys** (the user's grid keys A): ←→ walk the tiles in reading
//!   order, wrapping row to row and class to class (a folded class's
//!   header is a stop); ↑↓ keep the column and stop on headers; on a
//!   header ← folds and → opens it. Enter switches to the tile's gun
//!   (downloading it first if it isn't here), or opens or folds a
//!   header. `\` (the physical key, Backslash or the ISO key beside the
//!   left Shift, never typed) folds the class the cursor is in and parks
//!   it on the header; `\` or Enter there opens it and puts the cursor
//!   back on the same tile. The detail pane follows the cursor. Letters
//!   do not go to the search until the field has the keyboard (`/`, a
//!   click on it, or Enter with nothing under the cursor); the field is
//!   the picker's search (`GunPicker`'s `PickerState`: a caret placed by a
//!   click, a selection by a drag, copy and paste), so letters and digits type at the
//!   caret, the arrows, Home and End move it (Left and Right don't move
//!   the grid's cursor then), Backspace and Delete edit, Ctrl+A/C/X/V
//!   work, and Enter or Esc lets go of the field (Enter: the cursor is
//!   on the first match; Enter again switches). With the field not
//!   focused B closes the view (B opens it, B closes it: the sandbox's
//!   `keys` reads B, once per press, never this view), I shows the
//!   large model and R retries the selected gun's failed download. Ctrl+I
//!   and Ctrl+R do the same at any time; Ctrl+1–6 pins; Esc closes the
//!   popover, then leaves the field, then clears the search, then closes
//!   the view.
//!
//! The user's decisions (the design rules, section 22): a tile's
//! download state is a bar along its bottom edge (its words in the
//! foot), so the keyboard cursor is the tile's accent outline; a drop
//! shows a ghost of the gun in the slot (`model::drop_hint`); the keys
//! below.
//!
//! A card's name is cut to the card with the kit's ellipsis
//! (`chimera_ui::Ellipsize`), and so is the detail pane's head: the name
//! has the pane's width less the icon's, which stays in the pane (the
//! variants list under the model has every variant's name).
//!
//! Known shortcuts: a slot's cards are spawned again when it shows
//! another row (the slots themselves stay; the kit's binding in place is
//! for text); the model area shows the kit's icon large, not a render;
//! the drag shows the slot under the pointer but no ghost tile; the
//! picker's search text is copied to `GunView::search` (by `sync_grid`),
//! because the tiles' families and the games' counts filter by it too.

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    ui::{DeprecatedInteraction as Interaction, RelativeCursorPosition},
    window::PrimaryWindow,
};
use chimera_ui::{
    Role, Theme, Tone, UiParityId, UiStyle, VCursor, VHit, VListEvent, VNav, VSlot, VirtualLists,
    controls::{State, Ui},
    picker::{self, PickerAction, PickerFamily, PickerFrame, PickerGame, PickerGroup, PickerId, PickerItem, PickerLook, PickerMode, PickerPart, PickerSlot, PickerState, Pickers},
    vlist::VLine,
};

use super::{
    SandboxAction, SandboxCatalogue, SandboxScreen, SandboxUi,
    hud::{GunFace, gun_icon, key_hint, kit_icon, pin_bar, progress_of, tag},
    model::{Files, SandboxGun, TagGroup, facets_of},
    units::{UnitProgress, UnitRequest, UnitState, size_words},
};
use crate::client::signal::Look;

/// The grid's list key (`VirtualLists`).
pub const LIST: &str = "sandbox.guns";

/// The games column's width, the body's gaps and the detail pane's
/// width (cq): the grid has the rest of the 100-wide frame.
pub const GAMES_W: f32 = 14.0;
pub const BODY_GAP: f32 = 1.3;
pub const DETAIL_W: f32 = 25.0;

/// The grid's picker frame (module doc, "The grid").
pub fn frame() -> PickerFrame {
    PickerFrame {
        w: 100.0 - 2.0 * super::hud::M - GAMES_W - BODY_GAP,
        look: PickerLook::Cards,
        mode: PickerMode::Single,
        foot: true,
        side: Some(DETAIL_W),
        list_cursor: true,
        search: true,
        placeholder: "Find a gun, a game or a tag".into(),
        ..Default::default()
    }
}

/// The grid's picker state (single pick; the list's folds read back).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct GunPicker(pub PickerState);

/// The grid's cards as the picker's data: a picker game a class section,
/// a card a tile (`model::Tile`).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct GunCards(pub Vec<PickerGame>);

/// A tile's card: the face gun, its family's count, its pin, its
/// download (the state as a tag: a change binds the card again).
fn card_of(g: &SandboxGun, family: usize, pins: &super::model::Pins, unit: &UnitState) -> PickerItem {
    let progress = matches!(unit, UnitState::Downloading { .. } | UnitState::Queued { .. }).then(|| (unit.share().clamp(0.0, 1.0) * 1000.0) as u16);
    PickerItem {
        badge: Some(g.game_short.clone()),
        mark: pins.slot_of(&g.id).map(|p| (p + 1).to_string()),
        family,
        progress,
        tags: vec![format!("files:{unit:?}")],
        search_text: Some(super::model::search_text(g)),
        ..PickerItem::new(g.id.clone(), g.name.clone())
    }
}

/// The detail pane's variants list: its scroll view's key, the rows it
/// shows at most and a row's height (cq).
pub const VARIANTS: &str = "sandbox.variants";
pub const VARIANT_ROWS: usize = 5;
pub const VARIANT_H: f32 = 1.25;

/// The view's z (over the HUD and the game's).
pub const Z: i32 = crate::client::minigame_scoreboard::SCOREBOARD_Z + 5;

/// A click in the view.
#[derive(Component, Message, Clone, Debug, PartialEq, Eq)]
pub enum GunAction {
    /// A game in the left column (None: All games).
    Game(Option<String>),
    Facet(String, String),
    Tag(TagGroup, &'static str),
    TagsButton,
    Download(String),
    /// The body to play the selected gun as (None: the rung's own).
    Body(Option<String>),
    /// Pick a variant of the selected tile's gun (the detail pane's list).
    Variant(String),
    Switch,
    /// The tag popover's search clear key.
    ClearTagSearch,
    Close,
}

/// A pinned slot the pointer can drop on.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PinDrop(pub usize);

/// The view's parts (made at opening).
#[derive(Resource, Clone, Debug)]
pub struct Parts {
    pub root: Entity,
    /// The picker's bar (its search field).
    pub picker_bar: Option<Entity>,
    pub games: Entity,
    pub bar: Entity,
    pub detail: Entity,
    pub pins: Entity,
    pub pop: Entity,
    pub view: Entity,
    size: (u32, u32),
}

/// The grid's sections as shown: (name, its tiles' faces as guns'
/// indices), and each tile's variants by its face (`model::Tile`).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Grid(pub Vec<(&'static str, Vec<usize>)>, pub std::collections::BTreeMap<usize, Vec<usize>>);

impl Grid {
    /// The variants of the tile whose face is `face` (itself alone when
    /// it has none).
    pub fn variants_of(&self, face: usize) -> &[usize] {
        self.1.get(&face).map_or(&[], Vec::as_slice)
    }

    /// The face of the tile gun `gun` is on (its own, or its family's).
    pub fn face_of(&self, gun: usize) -> Option<usize> {
        self.1.iter().find(|(f, v)| **f == gun || v.contains(&gun)).map(|(f, _)| *f)
    }

    pub fn gun_at(&self, c: VCursor) -> Option<usize> {
        match c {
            VCursor::Item(s, i) => self.0.get(s)?.1.get(i).copied(),
            VCursor::Header(_) => None,
        }
    }

    pub fn cursor_of(&self, gun: usize) -> Option<VCursor> {
        let gun = self.face_of(gun)?;
        self.0.iter().enumerate().find_map(|(s, (_, v))| v.iter().position(|g| *g == gun).map(|i| VCursor::Item(s, i)))
    }
}

/// Keeps the grid's sections and the picker's cards from the filters,
/// the picker's folds from the list's, and the cursor and the selected
/// gun in step.
#[allow(clippy::too_many_arguments)]
pub fn sync_grid(
    mut ui: ResMut<SandboxUi>,
    cat: Res<SandboxCatalogue>,
    progress: Res<UnitProgress>,
    theme: Res<Theme>,
    mut grid: ResMut<Grid>,
    mut lists: ResMut<VirtualLists>,
    mut sel: ResMut<GridSel>,
    (mut pickers, mut gp, mut cards): (ResMut<Pickers>, ResMut<GunPicker>, ResMut<GunCards>),
) {
    if ui.screen != SandboxScreen::ChangeGun {
        sel.0 = None;
        return;
    }
    // The search words the picker holds are the tiles' filter too (a
    // family stays one card, the counts follow).
    if ui.view.search != gp.0.search {
        ui.view.search = gp.0.search.clone();
    }
    let tiles = ui.view.tiles(&cat.guns, &|g| SandboxCatalogue::files(g, &progress));
    let shown: Vec<(&'static str, Vec<usize>)> = tiles.iter().map(|(n, t)| (*n, t.iter().map(|t| t.face).collect())).collect();
    let games: Vec<PickerGame> = tiles
        .iter()
        .map(|(n, t)| PickerGame {
            id: n.to_string(),
            name: n.to_string(),
            groups: vec![PickerGroup {
                name: String::new(),
                items: t
                    .iter()
                    .filter_map(|t| cat.guns.get(t.face).map(|g| card_of(g, t.variants.len(), &ui.pins, &SandboxCatalogue::unit_state(g, &progress))))
                    .collect(),
            }],
        })
        .collect();
    let variants: std::collections::BTreeMap<usize, Vec<usize>> = tiles.into_iter().flat_map(|(_, t)| t).map(|t| (t.face, t.variants)).collect();
    if grid.1 != variants {
        grid.1 = variants;
    }
    if grid.0 != shown {
        grid.0 = shown;
    }
    // The picker's folds are the list's (the keys fold both; a staged or
    // tested fold is the list's alone).
    let mut state = gp.0.clone();
    state.folded = picker::folds_of(lists.state(LIST), LIST);
    // (The keys fold the state at once: the view's snapshot tells.)
    if cards.0 != games || pickers.0.get(LIST).is_none_or(|v| v.state != state) {
        cards.0 = games;
        gp.0 = state;
        picker::update(&theme, &mut pickers, LIST, &cards.0, &gp.0, frame(), &mut lists);
    }
    // The selected gun moves the cursor (opening, a search).
    let st = lists.state(LIST);
    if sel.0 != ui.view.selected {
        sel.0 = ui.view.selected.clone();
        if let Some(c) = ui.view.selected.as_ref().and_then(|id| cat.guns.iter().position(|g| g.id == *id)).and_then(|i| grid.cursor_of(i)) {
            st.select(c);
        }
    }
}

/// The gun the cursor was last put on for (`sync_grid` moves the cursor
/// when the selection changes otherwise: opening, a search).
#[derive(Resource, Default)]
pub struct GridSel(pub Option<String>);

/// The cursor moves the selection.
fn follow_cursor(lists: Res<VirtualLists>, grid: Res<Grid>, cat: Res<SandboxCatalogue>, mut ui: ResMut<SandboxUi>, mut sel: ResMut<GridSel>) {
    if ui.screen != SandboxScreen::ChangeGun {
        return;
    }
    let Some(c) = lists.get(LIST).and_then(|s| s.cursor) else { return };
    // (A variant picked on the cursor's tile stays picked.)
    let on_tile = |id: &str| grid.gun_at(c).is_some_and(|f| grid.variants_of(f).iter().any(|v| cat.guns.get(*v).is_some_and(|g| g.id == id)));
    if let Some(g) = grid.gun_at(c).and_then(|i| cat.guns.get(i))
        && ui.view.selected.as_deref() != Some(&g.id)
        && !ui.view.selected.as_deref().is_some_and(on_tile)
    {
        ui.view.selected = Some(g.id.clone());
        sel.0 = Some(g.id.clone());
    }
}

/// The keys while open (module doc, "Keys").
#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut typed: MessageReader<KeyboardInput>,
    owned: Option<Res<crate::client::binds::OwnedKeys>>,
    console: Option<Res<crate::client::console::ConsoleUi>>,
    cat: Res<SandboxCatalogue>,
    progress: Res<UnitProgress>,
    grid: Res<Grid>,
    mut ui: ResMut<SandboxUi>,
    mut lists: ResMut<VirtualLists>,
    (pickers, mut gp): (Res<Pickers>, ResMut<GunPicker>),
    mut out: MessageWriter<GunAction>,
    mut requests: MessageWriter<UnitRequest>,
    mut own: Local<chimera_ui::clipboard::Clipboard>,
    shared: Option<ResMut<chimera_ui::clipboard::Clipboard>>,
) {
    if ui.screen != SandboxScreen::ChangeGun || console.is_some_and(|c| c.open) {
        typed.clear();
        return;
    }
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let k = |c: KeyCode| owned.as_deref().map_or_else(|| keys.just_pressed(c), |o| o.just_pressed(&keys, c));
    let st = lists.state(LIST);
    if k(KeyCode::Escape) {
        typed.clear();
        ui.took_esc = true;
        if ui.view.tags_open && !ui.view.tag_search.is_empty() {
            ui.view.tag_search.clear();
        } else if ui.view.tags_open {
            ui.view.tags_open = false;
        } else if gp.0.search_focus {
            gp.0.search_focus = false;
        } else if !gp.0.search.is_empty() {
            gp.0.clear_search();
        } else {
            out.write(GunAction::Close);
        }
        return;
    }
    // The tag popover open: typing finds a tag ("Find a tag"), Enter
    // ticks the first one found.
    if ui.view.tags_open {
        let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
        for e in typed.read() {
            if e.state != ButtonState::Pressed || ctrl || alt || matches!(e.key_code, KeyCode::Backslash | KeyCode::IntlBackslash) {
                continue;
            }
            match &e.logical_key {
                Key::Backspace => {
                    ui.view.tag_search.pop();
                }
                Key::Space if !ui.view.tag_search.is_empty() => ui.view.tag_search.push(' '),
                Key::Character(c) if ui.view.tag_search.len() < 32 => ui.view.tag_search.push_str(c),
                _ => {}
            }
        }
        if (k(KeyCode::Enter) || k(KeyCode::NumpadEnter))
            && !ui.view.tag_search.is_empty()
            && let Some((g, t)) = ui.view.tags_found().first().map(|(g, t)| (*g, t[0]))
        {
            ui.view.tick_tag((g, t));
            ui.view.tag_search.clear();
        }
        return;
    }
    for (key, nav) in [
        (KeyCode::ArrowUp, VNav::Up),
        (KeyCode::ArrowDown, VNav::Down),
        (KeyCode::ArrowLeft, VNav::Left),
        (KeyCode::ArrowRight, VNav::Right),
        (KeyCode::PageUp, VNav::PageUp),
        (KeyCode::PageDown, VNav::PageDown),
    ] {
        // Left and Right move the focused field's caret.
        let in_field = gp.0.search_focus && matches!(nav, VNav::Left | VNav::Right);
        if k(key) && !in_field {
            // The picker's grid keys (a header's ← and → fold it there and
            // in the list).
            match pickers.0.get(LIST) {
                Some(view) => {
                    picker::nav(&mut gp.0, view, st, nav);
                }
                None => {
                    st.nav(nav);
                }
            }
        }
    }
    // A header's fold, in the picker's state and its list (`picker::fold`).
    let fold_sec = |gp: &mut PickerState, st: &mut chimera_ui::VListState, sec: usize, open: bool| match pickers.0.get(LIST).and_then(|v| v.sections.get(sec).map(|s| (v, s.0))) {
        Some((view, row)) => picker::fold(gp, view, st, row, open),
        None => st.fold(sec, !open),
    };
    // `\` (by its physical key, so typing never takes it) and Enter on a
    // header: fold the class, or open it again and go back to the tile the
    // fold left.
    let fold_key = k(KeyCode::Backslash) || k(KeyCode::IntlBackslash);
    let enter = k(KeyCode::Enter) || k(KeyCode::NumpadEnter);
    let focus = gp.0.search_focus;
    if focus && enter {
        // Confirms the first match: the field lets go; the cursor is on it
        // (the selection follows the search), Enter again switches.
        gp.0.search_focus = false;
    } else if enter && !focus && st.cursor.is_none() {
        gp.0.focus_search();
    }
    let enter = enter && !focus;
    match st.cursor {
        Some(VCursor::Item(sec, i)) if fold_key => {
            fold_sec(&mut gp.0, st, sec, false);
            ui.view.fold_return = Some((sec, i));
        }
        Some(VCursor::Header(sec)) if fold_key || enter => {
            if st.is_folded(sec) {
                fold_sec(&mut gp.0, st, sec, true);
                if let Some((rs, ri)) = ui.view.fold_return.take()
                    && rs == sec
                {
                    st.select(VCursor::Item(rs, ri));
                }
            } else {
                fold_sec(&mut gp.0, st, sec, false);
            }
        }
        Some(VCursor::Item(..)) if enter => {
            out.write(GunAction::Switch);
        }
        _ => {}
    }
    // Tab and Shift+Tab: the next and the last variant of the tile's gun.
    if k(KeyCode::Tab)
        && let Some(face) = st.cursor.and_then(|c| grid.gun_at(c))
    {
        let vs = grid.variants_of(face);
        let now = ui.view.selected.as_deref().and_then(|id| vs.iter().position(|v| cat.guns.get(*v).is_some_and(|g| g.id == id)));
        if vs.len() > 1 {
            let back = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
            let next = match (now, back) {
                (Some(i), false) => (i + 1) % vs.len(),
                (Some(i), true) => (i + vs.len() - 1) % vs.len(),
                (None, _) => 0,
            };
            if let Some(g) = cat.guns.get(vs[next]) {
                out.write(GunAction::Variant(g.id.clone()));
            }
        }
    }
    let selected = ui.view.selected.clone();
    if ctrl {
        for (i, d) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6].into_iter().enumerate() {
            if k(d)
                && let Some(g) = &selected
            {
                ui.pins.pin(i, g);
            }
        }
    }
    let failed_now = selected.as_deref().and_then(|id| cat.get(id)).and_then(|g| match (&g.unit, SandboxCatalogue::unit_state(g, &progress)) {
        (Some(u), UnitState::Failed { .. }) => Some((g.game.clone(), u.clone())),
        _ => None,
    });
    // Ctrl+I and Ctrl+R: the same at any time.
    if ctrl && k(KeyCode::KeyI) {
        ui.view.detail = !ui.view.detail;
    }
    if ctrl
        && k(KeyCode::KeyR)
        && let Some((game, unit)) = failed_now.clone()
    {
        requests.write(UnitRequest::Retry { game, unit });
    }
    let failed = selected
        .as_deref()
        .and_then(|id| cat.get(id))
        .and_then(|g| match (&g.unit, SandboxCatalogue::unit_state(g, &progress)) {
            (Some(u), UnitState::Failed { .. }) => Some((g.game.clone(), u.clone())),
            _ => None,
        });
    // Typing: with the field focused letters and digits go to the search
    // (not with Ctrl or Alt held, and never `\`); otherwise (B, which
    // closes the view, is the sandbox's `keys`') `/` focuses the field, I is the model view and R retries the
    // selected gun's failed download.
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let clipboard = match shared {
        Some(c) => c.into_inner(),
        None => &mut *own,
    };
    for e in typed.read() {
        if e.state != ButtonState::Pressed || alt || matches!(e.key_code, KeyCode::Backslash | KeyCode::IntlBackslash) {
            continue;
        }
        if gp.0.search_focus {
            // The kit's text field: a caret, a selection, copy and paste.
            if let Some(key) = crate::client::widgets::text_key(&e.logical_key, e.key_code, ctrl, shift) {
                gp.0.search_key(key, clipboard);
            }
            continue;
        }
        if ctrl {
            continue;
        }
        match &e.logical_key {
            Key::Character(c) if c.as_str() == "/" => gp.0.focus_search(),
            Key::Character(c) if c.eq_ignore_ascii_case("i") => ui.view.detail = !ui.view.detail,
            Key::Character(c) if c.eq_ignore_ascii_case("r") => {
                if let Some((game, unit)) = failed.clone() {
                    requests.write(UnitRequest::Retry { game, unit });
                }
            }
            _ => {}
        }
    }
}

/// The pointer in the picker's search field (`picker::SearchPointer`): a
/// press focuses it and places the caret, a drag selects, a double press
/// selects all.
fn search_pointer(pointer: picker::SearchPointer, ui: Res<SandboxUi>, mut gp: ResMut<GunPicker>, mut press: Local<chimera_ui::controls::TextPress>) {
    if ui.screen == SandboxScreen::ChangeGun {
        pointer.run(LIST, &mut gp.0, &mut press);
    } else {
        press.release();
    }
}

/// Clicks, the list's events and drags.
#[allow(clippy::too_many_arguments)]
fn pointer(
    pressed: Query<(&Interaction, &GunAction), Changed<Interaction>>,
    headers: Query<(&Interaction, &PickerAction), Changed<Interaction>>,
    (pickers, mut gp): (Res<Pickers>, ResMut<GunPicker>),
    mut vlists: ResMut<VirtualLists>,
    mut list_events: MessageReader<VListEvent>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    window: Query<&Window, With<PrimaryWindow>>,
    tiles: Query<(&Interaction, &VHit)>,
    slots: Query<&VSlot>,
    drops: Query<(&PinDrop, &RelativeCursorPosition)>,
    grid: Res<Grid>,
    cat: Res<SandboxCatalogue>,
    mut ui: ResMut<SandboxUi>,
    mut press_at: Local<Option<(Vec2, String)>>,
    mut out: MessageWriter<GunAction>,
) {
    if ui.screen != SandboxScreen::ChangeGun {
        list_events.clear();
        *press_at = None;
        return;
    }
    for (i, a) in &pressed {
        if *i == Interaction::Pressed {
            out.write(a.clone());
        }
    }
    // A class's header (the picker's): fold or open it, the cursor on it.
    // (A card's press is the list's: `VHit`, below.)
    if let Some(view) = pickers.0.get(LIST) {
        for (i, a) in &headers {
            if *i == Interaction::Pressed {
                match a {
                    PickerAction::FocusSearch => gp.0.focus_search(),
                    PickerAction::ClearSearch => {
                        gp.0.clear_search();
                        gp.0.search_focus = true;
                    }
                    _ => {}
                }
            }
            if let (Interaction::Pressed, PickerAction::Fold(row)) = (*i, *a) {
                let st = vlists.state(LIST);
                let open = view.rows.get(row).is_some_and(|r| gp.0.is_folded(&view.games, r));
                picker::fold(&mut gp.0, view, st, row, open);
                if let Some(c) = view.cursor_of(row) {
                    st.select(c);
                }
            }
        }
    }
    let lists = &*vlists;
    for e in list_events.read() {
        if let VListEvent::Activate { key, .. } = e
            && key == LIST
        {
            out.write(GunAction::Switch);
        }
    }
    // A drag: a press on a tile, then a move of 8 px.
    let at = window.single().ok().and_then(|w| w.cursor_position());
    let Some(m) = mouse.as_deref() else { return };
    if m.just_pressed(MouseButton::Left)
        && let Some(at) = at
    {
        let st = lists.get(LIST);
        let gun = tiles.iter().find(|(i, h)| **i == Interaction::Pressed && h.col.is_some()).and_then(|(_, h)| {
            let line = slots.get(h.slot).ok()?.line?;
            match *st?.layout.lines.get(line)? {
                VLine::Row { section, first, .. } => grid.gun_at(VCursor::Item(section, first + h.col?)),
                VLine::Header(_) => None,
            }
        });
        *press_at = gun.and_then(|g| cat.guns.get(g)).map(|g| (at, g.id.clone()));
    }
    if m.pressed(MouseButton::Left) {
        if let (Some((p0, gun)), Some(at)) = (press_at.as_ref(), at)
            && (ui.drag.is_some() || at.distance(*p0) > 8.0)
        {
            let over = drops.iter().find(|(_, r)| r.cursor_over).map(|(d, _)| d.0);
            let next = super::Drag { gun: gun.clone(), at, over };
            if ui.drag.as_ref() != Some(&next) {
                ui.drag = Some(next);
            }
        }
    } else {
        *press_at = None;
        // Only touched when there is a drag (a mutable touch is a change,
        // which rebinds the grid); a staged one stays.
        if ui.drag.is_some()
            && !ui.staged
            && let Some(d) = ui.drag.take()
            && let Some(slot) = d.over
        {
            ui.pins.pin(slot, &d.gun);
        }
    }
}

/// What a click does.
fn act(world: &mut World) {
    let actions: Vec<GunAction> = world.resource_mut::<Messages<GunAction>>().drain().collect();
    for a in actions {
        match a {
            GunAction::Game(g) => world.resource_mut::<SandboxUi>().view.tick_game(g.as_deref()),
            GunAction::Facet(g, f) => world.resource_mut::<SandboxUi>().view.tick_facet(&g, &f),
            GunAction::Tag(g, t) => world.resource_mut::<SandboxUi>().view.tick_tag((g, t)),
            GunAction::TagsButton => {
                let mut ui = world.resource_mut::<SandboxUi>();
                ui.view.tags_open = !ui.view.tags_open;
                ui.view.tag_search.clear();
            }
            GunAction::Download(id) => {
                let req = world.resource::<SandboxCatalogue>().get(&id).and_then(|g| {
                    let u = g.unit.clone()?;
                    let failed = matches!(SandboxCatalogue::unit_state(g, world.resource::<UnitProgress>()), UnitState::Failed { .. });
                    Some(if failed { UnitRequest::Retry { game: g.game.clone(), unit: u } } else { UnitRequest::Fetch { game: g.game.clone(), unit: u } })
                });
                if let Some(r) = req {
                    world.write_message(r);
                }
            }
            GunAction::Body(b) => world.resource_mut::<SandboxUi>().view.body = b,
            GunAction::Variant(id) => {
                let mut ui = world.resource_mut::<SandboxUi>();
                if ui.view.selected.as_deref() != Some(id.as_str()) {
                    ui.view.selected = Some(id);
                }
            }
            GunAction::Switch => {
                let Some(id) = world.resource::<SandboxUi>().view.selected.clone() else { continue };
                world.write_message(SandboxAction::Switch(id));
                world.resource_mut::<SandboxUi>().screen = SandboxScreen::None;
            }
            GunAction::ClearTagSearch => world.resource_mut::<SandboxUi>().view.tag_search.clear(),
            GunAction::Close => {
                world.resource_mut::<GunPicker>().0.search_focus = false;
                world.write_message(SandboxAction::Close);
            }
        }
    }
}

// ---------------------------------------------------------------------
// Drawing.

/// Opens or closes the view's root; builds its fixed parts at opening.
#[allow(clippy::too_many_arguments)]
fn open_close(
    look: Look,
    ui_state: Res<SandboxUi>,
    parts: Option<Res<Parts>>,
    mut views: ResMut<chimera_ui::ScrollViews>,
    (mut lists, mut pickers, gp, cards): (ResMut<VirtualLists>, ResMut<Pickers>, Res<GunPicker>, Res<GunCards>),
    mut c: Commands,
) {
    let want = look.on() && ui_state.screen == SandboxScreen::ChangeGun;
    let size = (look.units.height as u32, (look.units.scale_factor * 100.0) as u32);
    if let Some(p) = parts.as_deref()
        && (!want || p.size != size)
    {
        c.entity(p.root).despawn();
        c.remove_resource::<Parts>();
        if !want {
            views.forget(LIST);
            pickers.0.remove(LIST);
        }
        return;
    }
    if !want || parts.is_some() {
        return;
    }
    let ui = Ui::new(&look.theme, look.fonts.as_deref(), *look.units);
    let m = super::hud::M;
    let root = c
        .spawn((
            UiStyle { gap: Some(0.9), pad: Some([1.8, m, 1.2, m].into()), ..UiStyle::column() }.abs([Some(0.0), Some(0.0), Some(0.0), Some(0.0)]).bg_a(Tone::Ink, 0.97),
            Node::default(),
            GlobalZIndex(Z),
            chimera_ui::controls::catch_pointer(),
            UiParityId::new("gun.view"),
            Name::new("Change gun"),
        ))
        .id();
    // The head: the breadcrumb and title (the search is the picker's bar).
    let head = ui.boxed(&mut c, root, UiStyle { align: Some(AlignItems::FlexStart), h: Some(3.4), shrink: Some(0.0), ..UiStyle::row() });
    let crumbs = ui.boxed(&mut c, head, UiStyle { gap: Some(0.35), ..UiStyle::column() });
    let back = ui.boxed(&mut c, crumbs, UiStyle { gap: Some(0.45), align: Some(AlignItems::Center), ..UiStyle::row() });
    kit_icon(&mut c, &look, back, "back", 0.5, Tone::Fg2, 1.0);
    ui.text(&mut c, back, Role::Mono, 0.52, Tone::Fg2, "Sandbox");
    ui.kbd(&mut c, back, "Esc", 0.5, Tone::Fg3);
    let t = ui.text(&mut c, crumbs, Role::DisplayBold, 1.9, Tone::Fg, "Change gun");
    c.entity(t).insert(UiParityId::new("gun.title"));
    // The body: games, the grid, the detail.
    // The body: games, then the picker (the tags bar over the grid, the
    // detail its side pane).
    let body = ui.boxed(&mut c, root, UiStyle { gap: Some(BODY_GAP), grow: Some(1.0), align: Some(AlignItems::Stretch), ..UiStyle::row() });
    c.entity(body).insert(Node { min_height: Val::Px(0.0), ..default() });
    let games = c.spawn((UiStyle { w: Some(GAMES_W), gap: Some(0.1), shrink: Some(0.0), ..UiStyle::column() }, Node::default(), UiParityId::new("gun.games"), ChildOf(body))).id();
    let main = ui.boxed(&mut c, body, UiStyle { grow: Some(1.0), gap: Some(0.4), ..UiStyle::column() });
    c.entity(main).insert(Node { min_width: Val::Px(0.0), min_height: Val::Px(0.0), ..default() });
    let bar = ui.boxed(&mut c, main, UiStyle { gap: Some(0.35), align: Some(AlignItems::Center), h: Some(1.6), shrink: Some(0.0), w: Some(frame().w - DETAIL_W - look.theme.layout(picker::L, "side_gap")), ..UiStyle::row() });
    let built = picker::spawn_parts(&mut c, &ui, main, LIST, &cards.0, &gp.0, frame(), &mut lists, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    let detail = built.side.expect("the frame asks for a side pane");
    c.entity(detail).insert((UiStyle { w: Some(DETAIL_W), gap: Some(0.5), shrink: Some(0.0), ..UiStyle::column() }, UiParityId::new("gun.detail")));
    // The foot: the keys, the pinned bar at the right.
    // The hints wrap (and give way to the pinned bar, which never leaves
    // the screen): at any aspect ratio the hotbar stays in view.
    let foot = ui.boxed(&mut c, root, UiStyle { gap: Some(1.5), align: Some(AlignItems::FlexEnd), shrink: Some(0.0), ..UiStyle::row() });
    let keys = ui.boxed(&mut c, foot, UiStyle { gap: Some(1.1), align: Some(AlignItems::Center), min_w: Some(0.0), grow: Some(1.0), shrink: Some(1.0), ..UiStyle::row() });
    c.entity(keys).insert(Node { flex_wrap: FlexWrap::Wrap, row_gap: look.val(0.3), ..default() });
    for (k, w) in [
        ("\u{2191}\u{2193}\u{2190}\u{2192}", "Tile"),
        ("Enter", "Switch, or fold"),
        ("\\", "Fold this class"),
        ("Tab", "Variant"),
        ("I", "Model"),
        ("Ctrl 1-6", "Pin"),
        ("R", "Retry"),
        ("/", "Search (click it too)"),
        ("B", "Close"),
        ("Esc", "Leave the search, then back"),
    ] {
        key_hint(&mut c, &ui, keys, k, w, 0.5, Tone::Fg2);
    }
    let pins = ui.boxed(&mut c, foot, UiStyle { gap: Some(0.9), align: Some(AlignItems::Center), shrink: Some(0.0), ..UiStyle::row() });
    let pop = c.spawn((Node { position_type: PositionType::Absolute, ..default() }, GlobalZIndex(Z + 2), ChildOf(root))).id();
    c.insert_resource(Parts { root, picker_bar: built.bar, games, bar, detail, pins, pop, view: built.list, size });
}

/// What the rebuilt parts show.
#[derive(Clone, Debug, Default, PartialEq)]
struct Shown {
    view: super::model::GunView,
    /// The picker's search as drawn: text, caret, focus.
    search: (String, chimera_ui::text_edit::Caret, bool),
    pins: Vec<Option<GunFace>>,
    current: Option<String>,
    drop: Option<usize>,
    dragging: bool,
    progress: UnitProgress,
    catalogue: usize,
    grid_count: usize,
    /// The selected tile's variants (the detail pane's list).
    variants: Vec<usize>,
}

#[allow(clippy::too_many_arguments)]
fn draw(
    look: Look,
    ui_state: Res<SandboxUi>,
    (gp, cards): (Res<GunPicker>, Res<GunCards>),
    cat: Res<SandboxCatalogue>,
    progress: Res<UnitProgress>,
    grid: Res<Grid>,
    parts: Option<Res<Parts>>,
    mut last: Local<Option<(Entity, Shown)>>,
    mut c: Commands,
) {
    let Some(p) = parts.as_deref() else {
        *last = None;
        return;
    };
    let face = |id: &str| cat.get(id).map(GunFace::of).unwrap_or_else(|| GunFace::bare(id));
    let next = Shown {
        view: ui_state.view.clone(),
        search: (gp.0.search.clone(), gp.0.caret, gp.0.search_focus),
        pins: ui_state.pins.0.iter().map(|p| p.as_deref().map(face)).collect(),
        current: ui_state.current.clone(),
        drop: ui_state.drag.as_ref().and_then(|d| d.over),
        dragging: ui_state.drag.is_some(),
        progress: progress.clone(),
        catalogue: cat.guns.len(),
        grid_count: grid.0.iter().map(|s| s.1.len()).sum(),
        variants: ui_state
            .view
            .selected
            .as_deref()
            .and_then(|id| cat.guns.iter().position(|g| g.id == id))
            .and_then(|i| grid.face_of(i))
            .map(|f| grid.variants_of(f).to_vec())
            .unwrap_or_default(),
    };
    let fresh = last.as_ref().is_none_or(|(r, _)| *r != p.root);
    let prev = if fresh { None } else { last.as_ref().map(|l| l.1.clone()) };
    if prev.as_ref() == Some(&next) {
        return;
    }
    let ui = Ui::new(&look.theme, look.fonts.as_deref(), *look.units);
    // Rebuild only the parts whose inputs changed (a click on a tile
    // changes the detail, not the games, the bar or the pins): what stays
    // is not drawn again, so nothing flickers.
    let strip = |v: &super::model::GunView| {
        let mut v = v.clone();
        v.selected = None;
        v.body = None;
        v.detail = Default::default();
        v
    };
    let (new_all, prev_s) = (prev.is_none(), prev.as_ref());
    let search_changed = new_all || prev_s.is_some_and(|o| o.search != next.search);
    let side_changed = new_all
        || prev_s.is_some_and(|o| {
            strip(&o.view) != strip(&next.view) || o.progress != next.progress || o.catalogue != next.catalogue || o.grid_count != next.grid_count
        });
    let detail_changed = new_all
        || prev_s.is_some_and(|o| {
            o.view.selected != next.view.selected
                || o.view.body != next.view.body
                || o.view.detail != next.view.detail
                || o.variants != next.variants
                || o.progress != next.progress
                || o.pins != next.pins
        });
    let pins_changed = new_all || prev_s.is_some_and(|o| o.pins != next.pins || o.current != next.current || o.drop != next.drop || o.dragging != next.dragging);
    if search_changed {
        if let Some(bar) = p.picker_bar {
            picker::refill_bar(&mut c, &ui, bar, LIST, &cards.0, &gp.0, &frame());
        }
    }
    if side_changed {
        for e in [p.games, p.bar, p.pop] {
            c.entity(e).despawn_children();
        }
        games_column(&mut c, &ui, p.games, &cat.guns, &ui_state, &progress);
        filter_bar(&mut c, &ui, &look, p.bar, &ui_state, next.grid_count, &cat.guns, &progress);
        if ui_state.view.tags_open {
            tag_popover(&mut c, &ui, &look, p.pop, &ui_state);
        }
    }
    if detail_changed {
        c.entity(p.detail).despawn_children();
        if let Some(g) = ui_state.view.selected.as_deref().and_then(|id| cat.get(id)) {
            let variants: Vec<&SandboxGun> = next.variants.iter().filter_map(|i| cat.guns.get(*i)).collect();
            detail(&mut c, &ui, &look, p.detail, g, &variants, &progress, ui_state.view.detail, ui_state.pins.slot_of(&g.id), ui_state.view.body.as_deref());
        }
    }
    if pins_changed {
        c.entity(p.pins).despawn_children();
        let hint = ui.boxed(&mut c, p.pins, UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), ..UiStyle::row() });
        let ghost = ui_state.drag.as_ref().and_then(|d| Some((d.over?, face(&d.gun))));
        match &ghost {
            Some((slot, g)) => {
                ui.mark(&mut c, hint, Tone::Acc, 0.3, 0.7);
                let old = next.pins.get(*slot).cloned().flatten().map(|o| o.name);
                let t = ui.text(&mut c, hint, Role::Mono, 0.46, Tone::Fg, &super::model::drop_hint(&g.name, *slot, old.as_deref()));
                c.entity(t).insert(UiParityId::new("gun.drop.hint"));
            }
            None => {
                ui.text(&mut c, hint, Role::Mono, 0.46, Tone::Fg, "Ctrl+1-6 on a tile, or drag a gun here");
            }
        }
        let slots = pin_bar(&mut c, &ui, &look, p.pins, &next.pins, next.current.as_deref().and_then(|g| ui_state.pins.slot_of(g)), ghost, 0.72, 1.0);
        for (i, s) in slots.into_iter().enumerate() {
            c.entity(s).insert((PinDrop(i), RelativeCursorPosition::default()));
        }
    }
    *last = Some((p.root, next));
}

fn games_column(c: &mut Commands, ui: &Ui, parent: Entity, guns: &[SandboxGun], s: &SandboxUi, progress: &UnitProgress) {
    let l = ui.boxed(c, parent, UiStyle { margin: Some([0.15, 0.0, 0.45, 0.0].into()), ..UiStyle::row() });
    ui.text(c, l, Role::Mono, 0.46, Tone::Fg3, "Games");
    let fits = |g: &SandboxGun| s.view.fits_but_games(g, SandboxCatalogue::files(g, progress));
    // The counts are tiles: a gun's variants count once (`model`, "Families").
    let tiles = |of: &dyn Fn(&SandboxGun) -> bool| {
        guns.iter().filter(|g| of(g) && fits(g)).map(|g| (g.game.as_str(), super::model::section_of(g), g.family.as_str())).collect::<std::collections::BTreeSet<_>>().len()
    };
    let all = tiles(&|_| true);
    let row = |c: &mut Commands, name: &str, count: usize, sel: bool, sub: Option<String>, new: bool, action: GunAction| {
        let tone = if sel { Tone::Ink } else if count == 0 { Tone::Fg4 } else { Tone::Fg2 };
        let st = UiStyle { gap: Some(0.3), pad: Some([0.45, 0.7, 0.45, 0.7].into()), min_w: None, ..UiStyle::column() };
        let r = {
            let e = ui.item(c, parent, st, State::NONE, sel);
            c.entity(e).insert((Node { min_height: ui.units.val(2.0), justify_content: JustifyContent::Center, ..default() }, action));
            e
        };
        let line = ui.boxed(c, r, UiStyle { gap: Some(0.35), align: Some(AlignItems::Center), ..UiStyle::row() });
        ui.text(c, line, Role::DisplayRegular, 0.7, tone, name);
        if new {
            tag(c, ui, line, "New", 0.4, Some((Tone::Acc, Tone::OnAcc)), Tone::Acc);
        }
        ui.boxed(c, line, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        ui.text(c, line, Role::Mono, 0.54, if sel { Tone::Ink } else { Tone::Fg3 }, &count.to_string());
        if let Some(sub) = sub {
            ui.text(c, r, Role::Mono, 0.42, if sel { Tone::Ink } else { Tone::Fg3 }, &sub);
        }
        r
    };
    row(c, "All games", all, s.view.games.is_empty(), None, false, GunAction::Game(None));
    ui.boxed(c, parent, UiStyle { h: Some(0.05), margin: Some([0.3, 0.0, 0.3, 0.0].into()), ..UiStyle::row() }.bg(Tone::Fg5));
    let mut games: Vec<(&str, &str)> = Vec::new();
    for g in guns {
        if !games.iter().any(|(id, _)| *id == g.game) {
            games.push((&g.game, &g.game_name));
        }
    }
    for (id, name) in games {
        let mine: Vec<&SandboxGun> = guns.iter().filter(|g| g.game == id).collect();
        let count = tiles(&|g| g.game == id);
        let here = mine.iter().filter(|g| SandboxCatalogue::files(g, progress) == Files::Here).count();
        // One download for the whole game: every gun shares one unit.
        let units: std::collections::BTreeSet<&str> = mine.iter().filter_map(|g| g.unit.as_deref()).collect();
        let sub = if here == mine.len() {
            None
        } else if units.len() == 1 && mine.iter().all(|g| g.unit.is_some()) {
            let u = units.iter().next().unwrap();
            let bytes = match progress.state(id, u) {
                UnitState::NotHere { bytes } | UnitState::Queued { bytes } => bytes,
                UnitState::Downloading { total, .. } => total,
                _ => 0,
            };
            Some(format!("One download \u{00b7} {}", size_words(bytes)))
        } else {
            Some(format!("{here} of {} here", mine.len()))
        };
        let sel = s.view.games.contains(id);
        row(c, name, count, sel, sub, id == "tf2", GunAction::Game(Some(id.to_string())));
        let facets = facets_of(guns, id);
        if sel && !facets.is_empty() {
            let f = c
                .spawn((
                    UiStyle { gap: Some(0.35), pad: Some([0.6, 0.0, 0.8, 0.7].into()), margin: Some([0.0, 0.0, 0.0, 0.7].into()), ..UiStyle::column() },
                    Node::default(),
                    ChildOf(parent),
                ))
                .id();
            c.spawn((Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, width: Val::Px(ui.units.hairline()), ..default() }, BackgroundColor(ui.color(Tone::Fg4, 1.0)), ChildOf(f)));
            ui.text(c, f, Role::Mono, 0.42, Tone::Fg3, "Class");
            let chips = c.spawn((UiStyle { gap: Some(0.25), ..UiStyle::row() }, Node { flex_wrap: FlexWrap::Wrap, row_gap: ui.units.val(0.25), ..default() }, ChildOf(f))).id();
            let ticked = s.view.facets.get(id);
            for fc in facets {
                let on = ticked.is_some_and(|t| t.contains(&fc));
                let t = press_tag(c, ui, chips, &fc, on);
                c.entity(t).insert(GunAction::Facet(id.to_string(), fc.clone()));
            }
        }
    }
}

/// A tick tag: Mono .46, outlined; on, a white fill.
/// `tick_tag` that's pressed (a facet, a tag, the body to play as): the
/// kit's item, ticked the white (selected) fill.
fn press_tag(c: &mut Commands, ui: &Ui, parent: Entity, text: &str, on: bool) -> Entity {
    let s = UiStyle { pad: Some([0.26, 0.4, 0.22, 0.4].into()), shrink: Some(0.0), ..UiStyle::row() };
    let s = if on { s } else { s.outline(Tone::Fg4, 1.0) };
    let b = ui.item(c, parent, s, State::NONE, on);
    ui.text(c, b, Role::Mono, 0.46, if on { Tone::Ink } else { Tone::Fg2 }, text);
    b
}

pub fn tick_tag(c: &mut Commands, ui: &Ui, parent: Entity, text: &str, on: bool) -> Entity {
    let s = UiStyle { pad: Some([0.26, 0.4, 0.22, 0.4].into()), shrink: Some(0.0), ..UiStyle::row() };
    let s = if on { s.bg(Tone::Fg) } else { s.outline(Tone::Fg4, 1.0) };
    let b = c.spawn((s, Node::default(), ChildOf(parent))).id();
    ui.text(c, b, Role::Mono, 0.46, if on { Tone::Ink } else { Tone::Fg2 }, text);
    b
}

#[allow(clippy::too_many_arguments)]
fn filter_bar(c: &mut Commands, ui: &Ui, look: &Look, parent: Entity, s: &SandboxUi, shown: usize, guns: &[SandboxGun], progress: &UnitProgress) {
    let b = {
        let e = ui.item(c, parent, UiStyle { gap: Some(0.35), align: Some(AlignItems::Center), pad: Some([0.3, 0.5, 0.3, 0.5].into()), ..UiStyle::row() }.bg_a(Tone::Fg, if s.view.tags_open { 0.14 } else { 0.04 }).outline(if s.view.tags_open { Tone::Fg2 } else { Tone::Fg3 }, 1.0), State::NONE, false);
        c.entity(e).insert((GunAction::TagsButton, UiParityId::new("gun.tags")));
        e
    };
    kit_icon(c, look, b, "filter", 0.6, Tone::Fg, 1.0);
    ui.text(c, b, Role::Mono, 0.48, Tone::Fg, "Tags");
    for &(g, t) in &s.view.tags {
        let chip = {
            let e = ui.item(c, parent, UiStyle { gap: Some(0.3), align: Some(AlignItems::Center), pad: Some([0.26, 0.4, 0.22, 0.4].into()), ..UiStyle::row() }, State::NONE, true);
            c.entity(e).insert(GunAction::Tag(g, t));
            e
        };
        ui.text(c, chip, Role::Mono, 0.46, Tone::Ink, t);
        kit_icon(c, look, chip, "close", 0.36, Tone::Ink, 1.0);
    }
    let more = ui.boxed(c, parent, UiStyle { gap: Some(0.35), align: Some(AlignItems::Center), pad: Some([0.3, 0.5, 0.3, 0.5].into()), margin: Some([0.0, 0.0, 0.0, 0.2].into()), ..UiStyle::row() }.outline(Tone::Fg5, 1.0));
    ui.text(c, more, Role::Mono, 0.46, Tone::Fg4, "More filters");
    let later = ui.boxed(c, more, UiStyle { pad: Some([0.12, 0.25, 0.12, 0.25].into()), ..UiStyle::row() }.outline(Tone::Fg5, 1.0));
    ui.text(c, later, Role::Mono, 0.38, Tone::Fg3, "Later");
    ui.boxed(c, parent, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    let here = guns.iter().filter(|g| SandboxCatalogue::files(g, progress) == Files::Here).count();
    // Guns are tiles (a gun's variants count once); every variant is in
    // the catalogue.
    let every = guns.iter().map(|g| (g.game.as_str(), super::model::section_of(g), g.family.as_str())).collect::<std::collections::BTreeSet<_>>().len();
    let words = match (shown == every, every == guns.len()) {
        (true, true) => format!("{shown} guns \u{00b7} {here} here"),
        (true, false) => format!("{shown} guns \u{00b7} {} with their variants", guns.len()),
        (false, _) => format!("{shown} of {every} guns"),
    };
    ui.text(c, parent, Role::Mono, 0.48, Tone::Fg3, &words);
}

fn tag_popover(c: &mut Commands, ui: &Ui, look: &Look, parent: Entity, s: &SandboxUi) {
    let p = c
        .spawn((
            UiStyle { w: Some(36.0), gap: Some(0.6), pad: Some([0.9, 1.0, 0.9, 1.0].into()), ..UiStyle::column() }.abs([Some(16.6), Some(8.4), None, None]).bg(Tone::Ink2).outline(Tone::Fg3, 1.0),
            Node::default(),
            BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.7), Val::ZERO, look.val(1.4), Val::ZERO, look.val(3.0)),
            chimera_ui::controls::catch_pointer(),
            UiParityId::new("gun.popover"),
            ChildOf(parent),
        ))
        .id();
    // The kit's search field, the hints beside it; typing goes to it
    // while the popover is open (the caret at the end).
    let typed = &s.view.tag_search;
    let find = ui.boxed(c, p, UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), ..UiStyle::row() });
    c.entity(find).insert(UiParityId::new("gun.popover.find"));
    let cur = (!typed.is_empty()).then(|| chimera_ui::controls::TextCursor::from(typed.len()));
    let field = ui.search_field(c, find, typed, "Find a tag", cur, 13.0, chimera_ui::controls::State::focused(true));
    if let Some(k) = field.clear {
        c.entity(k).insert(GunAction::ClearTagSearch);
    }
    ui.boxed(c, find, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    ui.text(c, find, Role::Mono, 0.42, Tone::Fg3, if typed.is_empty() { "Type to find \u{00b7} every game's guns carry these" } else { "Enter ticks the first \u{00b7} Esc clears" });
    let found = s.view.tags_found();
    if found.is_empty() {
        ui.sentence(c, p, 0.66, Tone::Fg3, "No tag holds that.");
    }
    for (g, tags) in found {
        let row = ui.boxed(c, p, UiStyle { gap: Some(0.6), align: Some(AlignItems::FlexStart), ..UiStyle::row() });
        let l = ui.boxed(c, row, UiStyle { w: Some(5.4), pad: Some([0.25, 0.0, 0.0, 0.0].into()), shrink: Some(0.0), ..UiStyle::row() });
        ui.text(c, l, Role::Mono, 0.42, Tone::Fg3, g.label());
        let chips = c.spawn((UiStyle { gap: Some(0.25), grow: Some(1.0), ..UiStyle::row() }, Node { flex_wrap: FlexWrap::Wrap, row_gap: ui.units.val(0.25), ..default() }, ChildOf(row))).id();
        for t in tags {
            let on = s.view.tags.contains(&(g, t));
            let e = press_tag(c, ui, chips, t, on);
            c.entity(e).insert(GunAction::Tag(g, t));
        }
    }
    // More filters: later (greyed).
    let more = ui.boxed(c, p, UiStyle { gap: Some(0.6), pad: Some([0.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
    let l = ui.boxed(c, more, UiStyle { w: Some(5.4), shrink: Some(0.0), ..UiStyle::column() });
    ui.text(c, l, Role::Mono, 0.42, Tone::Fg3, "More filters");
    ui.text(c, l, Role::Mono, 0.42, Tone::Fg3, "Later");
    let nums = ui.boxed(c, more, UiStyle { gap: Some(0.4), grow: Some(1.0), ..UiStyle::column() });
    for pair in [["Damage a hit", "Fire rate"], ["DPS", "Hits to kill"], ["Magazine", "Blast radius"]] {
        let r = ui.boxed(c, nums, UiStyle { gap: Some(1.0), ..UiStyle::row() });
        for n in pair {
            let f = ui.boxed(c, r, UiStyle { w_pct: Some(50.0), gap: Some(0.4), align: Some(AlignItems::Center), ..UiStyle::row() });
            let lb = ui.boxed(c, f, UiStyle { w: Some(4.4), shrink: Some(0.0), ..UiStyle::row() });
            ui.text_a(c, lb, Role::Mono, 0.42, Tone::Fg2, 0.45, n);
            ui.boxed(c, f, UiStyle { grow: Some(1.0), h: Some(0.05), ..UiStyle::row() }.bg(Tone::Fg4));
            ui.text_a(c, f, Role::Mono, 0.42, Tone::Fg2, 0.45, "Any");
        }
    }
}

/// A card's foot (module doc), in the picker's foot slot: the delivery
/// and fire mode, or the download state (its bar along the card's bottom
/// edge is the picker's, `PickerItem::progress`).
fn tile_foot(c: &mut Commands, ui: &Ui, look: &Look, foot: Entity, g: &SandboxGun, st: &UnitState) {
    c.entity(foot).insert(Node { justify_content: JustifyContent::Center, ..default() });
    match st {
        UnitState::Here => {
            let words: Vec<&str> = g.tags.iter().filter(|t| matches!(t.0, TagGroup::Delivery | TagGroup::FireMode | TagGroup::Optics)).map(|t| t.1).take(2).collect();
            ui.text(c, foot, Role::Mono, 0.36, Tone::Fg3, &words.join(" \u{00b7} "));
        }
        UnitState::NotHere { bytes } => {
            let b = {
                let e = ui.item(c, foot, UiStyle { gap: Some(0.3), align: Some(AlignItems::Center), pad: Some([0.12, 0.25, 0.12, 0.25].into()), ..UiStyle::row() }.outline(Tone::Fg5, 1.0), State::NONE, false);
                c.entity(e).insert(GunAction::Download(g.id.clone()));
                e
            };
            kit_icon(c, look, b, "download", 0.5, Tone::Fg2, 1.0);
            ui.text(c, b, Role::Mono, 0.38, Tone::Fg2, &size_words(*bytes));
        }
        UnitState::Queued { .. } => {
            ui.text(c, foot, Role::Mono, 0.38, Tone::Fg3, "Queued");
        }
        UnitState::Downloading { .. } => {
            ui.text(c, foot, Role::Mono, 0.38, Tone::Fg2, &format!("{:.0}%", st.share() * 100.0));
        }
        UnitState::Failed { .. } => {
            ui.mark(c, foot, Tone::Threat, 0.3, 0.6);
            ui.text(c, foot, Role::Mono, 0.38, Tone::Fg, "Failed");
            ui.kbd(c, foot, "R", 0.38, Tone::Fg2);
        }
    }
}

/// Fills the picker's slots in the grid's cards and headers (module doc,
/// "The grid"): a card's gun icon and its foot, a folded class's preview
/// (its first few guns' icons); the parity ids of a card and its family
/// count.
#[allow(clippy::too_many_arguments)]
fn fill(
    look: Look,
    grid: Res<Grid>,
    cat: Res<SandboxCatalogue>,
    progress: Res<UnitProgress>,
    slots: Query<(Entity, &PickerSlot, &ChildOf), Added<PickerSlot>>,
    fams: Query<(Entity, &ChildOf), Added<PickerFamily>>,
    up: Query<&ChildOf>,
    ids: Query<&PickerId>,
    mut c: Commands,
) {
    let ui = Ui::new(&look.theme, look.fonts.as_deref(), *look.units);
    for (e, slot, parent) in &slots {
        if slot.list != LIST {
            continue;
        }
        match slot.part {
            PickerPart::Art => {
                let Some(g) = cat.get(&slot.id) else { continue };
                let tone = if SandboxCatalogue::unit_state(g, &progress).is_here() { Tone::Fg } else { Tone::Fg2 };
                gun_icon(&mut c, &look, e, &g.id, g.icon_class, 1.55, tone, 1.0);
                c.entity(parent.parent()).insert(UiParityId::new(format!("gun.tile.{}", g.id)));
            }
            PickerPart::Foot => {
                let Some(g) = cat.get(&slot.id) else { continue };
                tile_foot(&mut c, &ui, &look, e, g, &SandboxCatalogue::unit_state(g, &progress));
            }
            PickerPart::Head => {
                let Some((_, guns)) = grid.0.iter().find(|(n, _)| *n == slot.id) else { continue };
                for &i in guns.iter().take(6) {
                    if let Some(g) = cat.guns.get(i) {
                        gun_icon(&mut c, &look, e, &g.id, g.icon_class, 0.5, Tone::Fg4, 1.0);
                    }
                }
            }
        }
    }
    // A family's count: the parity id its tests find (the card's id).
    for (e, parent) in &fams {
        let card = up.get(parent.parent()).map(ChildOf::parent).ok();
        if let Some(id) = card.and_then(|k| ids.get(k).ok()) {
            c.entity(e).insert(UiParityId::new(format!("gun.tile.variants.{}", id.0)));
        }
    }
}

/// Hits to kill a body of `hp` at `dmg` a hit (None: not measured).
pub fn hits_to_kill(hp: f32, dmg: Option<f32>) -> Option<u32> {
    let d = dmg.filter(|d| *d > 0.0)?;
    Some((hp / d).ceil() as u32)
}

#[allow(clippy::too_many_arguments)]
fn detail(c: &mut Commands, ui: &Ui, look: &Look, parent: Entity, g: &SandboxGun, variants: &[&SandboxGun], progress: &UnitProgress, big: bool, pinned: Option<usize>, body: Option<&str>) {
    c.spawn((Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, width: Val::Px(look.hair()), ..default() }, BackgroundColor(look.tone(Tone::Fg5)), ChildOf(parent)));
    ui.text(c, parent, Role::Mono, 0.46, Tone::Fg3, &g.game_name);
    // The head: the name in the one growing cell, cut to it with the
    // kit's ellipsis (a long variant's name never pushes the icon out of
    // the pane), the kit's icon at the right at its own width.
    let name = ui.boxed(c, parent, UiStyle { gap: Some(0.9), align: Some(AlignItems::Center), ..UiStyle::row() });
    let cell = ui.boxed(c, name, UiStyle { grow: Some(1.0), min_w: Some(0.0), ..UiStyle::column() });
    c.entity(cell).insert(UiParityId::new("gun.detail.name.cell"));
    let n = ui.text(c, cell, Role::DisplayBold, 1.35, Tone::Fg, &g.name);
    c.entity(n).insert((UiParityId::new("gun.detail.name"), chimera_ui::Ellipsize::in_parent(g.name.to_uppercase())));
    let icon = ui.boxed(c, name, UiStyle { shrink: Some(0.0), ..UiStyle::row() });
    c.entity(icon).insert(UiParityId::new("gun.detail.icon"));
    let key = g.icon_class.unwrap_or("rifle");
    kit_icon(c, look, icon, key, 0.95, Tone::Fg2, 1.0);
    // The model's area (a stub picture until a render can come).
    let st = SandboxCatalogue::unit_state(g, progress);
    let viewer = c
        .spawn((
            UiStyle { h: Some(if big { 20.0 } else if variants.len() > 1 { 7.0 } else { 11.0 }), justify: Some(JustifyContent::Center), align: Some(AlignItems::Center), shrink: Some(0.0), ..UiStyle::column() }.bg(Tone::Ink).outline(Tone::Fg5, 1.0),
            Node::default(),
            UiParityId::new("gun.model"),
            ChildOf(parent),
        ))
        .id();
    let vt = ui.boxed(c, viewer, UiStyle::row().abs([Some(0.5), Some(0.45), None, None]));
    ui.text(c, vt, Role::Mono, 0.4, Tone::Fg3, "3D model");
    match &st {
        UnitState::Here => {
            gun_icon(c, look, viewer, &g.id, g.icon_class, if big { 5.0 } else { 2.9 }, Tone::Fg2, 1.0);
            let hud = ui.boxed(c, viewer, UiStyle { justify: Some(JustifyContent::SpaceBetween), ..UiStyle::row() }.abs([Some(0.5), None, Some(0.5), Some(0.4)]));
            ui.text(c, hud, Role::Mono, 0.4, Tone::Fg3, "Drag to turn");
            ui.text(c, hud, Role::Mono, 0.4, Tone::Fg3, if big { "I: smaller" } else { "I: larger" });
        }
        UnitState::NotHere { .. } | UnitState::Failed { .. } => {
            gun_icon(c, look, viewer, &g.id, g.icon_class, 2.9, Tone::Fg5, 1.0);
            let b = {
                let e = ui.item(c, viewer, UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), pad: Some([0.55, 0.8, 0.55, 0.8].into()), ..UiStyle::row() }.abs([None, None, None, None]).bg_a(Tone::Ink, 0.6).outline(Tone::Fg3, 1.0), State::NONE, false);
                c.entity(e).insert(GunAction::Download(g.id.clone()));
                e
            };
            kit_icon(c, look, b, "download", 0.7, Tone::Fg, 1.0);
            ui.text(c, b, Role::Mono, 0.5, Tone::Fg, if matches!(st, UnitState::Failed { .. }) { "Failed: R retries" } else { "Download to see the model" });
        }
        _ => {
            gun_icon(c, look, viewer, &g.id, g.icon_class, 2.9, Tone::Fg4, 1.0);
            let (p, words) = progress_of(&st);
            let b = ui.boxed(c, viewer, UiStyle::row().abs([None, None, None, Some(1.2)]));
            ui.progress(c, b, p, &words, 12.0, 1.2, 0.44, true);
        }
    }
    // The variants of this gun the filters leave: a press picks one.
    if variants.len() > 1 {
        ui.section(c, parent, &format!("Variants \u{00b7} {}", variants.len()), None);
        let rows = variants.len().min(VARIANT_ROWS) as f32;
        let list = ui.scroll(c, parent, VARIANTS, UiStyle { h: Some(rows * VARIANT_H), shrink: Some(0.0), ..UiStyle::row() }, UiStyle::column());
        c.entity(list.frame).insert(UiParityId::new("gun.variants"));
        for v in variants {
            let on = v.id == g.id;
            let row = {
                let e = ui.item(c, list.content, UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), h: Some(VARIANT_H), pad: Some([0.0, 0.6, 0.0, 0.7].into()), shrink: Some(0.0), ..UiStyle::row() }.bg_a(Tone::Fg, if on { 0.1 } else { 0.0 }), State::NONE, false);
                c.entity(e).insert((Node { min_width: Val::Px(0.0), overflow: Overflow::clip_x(), ..default() }, GunAction::Variant(v.id.clone()), UiParityId::new(format!("gun.variant.{}", v.id))));
                e
            };
            if on {
                ui.focus_bar(c, row, 0.2);
                chimera_ui::scroll::scroll_into_view(c, row);
            }
            ui.text(c, row, Role::DisplayRegular, 0.56, if on { Tone::Fg } else { Tone::Fg2 }, &v.name);
            ui.boxed(c, row, UiStyle { grow: Some(1.0), ..UiStyle::row() });
            let note = match (v.id == v.family, SandboxCatalogue::unit_state(v, progress)) {
                (_, UnitState::NotHere { bytes }) => size_words(bytes),
                (_, UnitState::Queued { .. } | UnitState::Downloading { .. }) => "Downloading".into(),
                (_, UnitState::Failed { .. }) => "Failed".into(),
                (true, UnitState::Here) => "Base".into(),
                (false, UnitState::Here) => v.stats.damage.map_or(String::new(), |d| format!("{d:.0} a hit")),
            };
            ui.text(c, row, Role::Mono, 0.38, Tone::Fg3, &note);
        }
    }
    // Stats.
    let s = &g.stats;
    let grid = ui.boxed(c, parent, UiStyle { gap: Some(0.05), margin: Some([0.2, 0.0, 0.0, 0.0].into()), ..UiStyle::row() }.bg(Tone::Fg5));
    // An explosive's base damage is its blast's centre: the radius under it.
    let base = match s.blast_radius {
        Some(r) => format!("Base \u{00b7} {r:.1} m blast"),
        None => "Base".to_string(),
    };
    for (label, value, unit) in [
        ("Damage a hit", s.damage.map(|d| format!("{d:.1}")), base.as_str()),
        ("Fire rate", s.fire_rate.map(|r| format!("{r:.0}")), "A minute"),
        ("Magazine", s.magazine.filter(|m| *m > 0).map(|m| m.to_string()), ""),
        ("Range feel", s.range_feel.map(|r| r[..1].to_uppercase() + &r[1..]), ""),
    ] {
        let cell = ui.boxed(c, grid, UiStyle { gap: Some(0.25), grow: Some(1.0), pad: Some([0.45, 0.5, 0.45, 0.5].into()), ..UiStyle::column() }.bg(Tone::Ink2));
        ui.text(c, cell, Role::Mono, 0.38, Tone::Fg3, label);
        ui.text(c, cell, Role::Mono, 1.0, if value.is_some() { Tone::Fg } else { Tone::Fg4 }, &value.unwrap_or_else(|| "\u{2014}".into()));
        if !unit.is_empty() {
            ui.text(c, cell, Role::Mono, 0.38, Tone::Fg3, unit);
        }
    }
    // Hits to kill.
    ui.section(c, parent, "Hits to kill \u{00b7} body \u{00b7} 10 m", None);
    let htk = ui.boxed(c, parent, UiStyle { gap: Some(0.4), ..UiStyle::row() });
    let head_mult = s.head_mult.unwrap_or(1.0);
    for (label, n) in [
        ("100-point body", hits_to_kill(100.0, s.damage)),
        ("Spartan", hits_to_kill(115.0, s.damage)),
        ("Head", hits_to_kill(100.0, s.damage.map(|d| d * head_mult))),
    ] {
        let cell = ui.boxed(c, htk, UiStyle { gap: Some(0.25), grow: Some(1.0), pad: Some([0.45, 0.5, 0.45, 0.5].into()), ..UiStyle::column() }.outline(Tone::Fg5, 1.0));
        ui.text(c, cell, Role::Mono, 0.4, Tone::Fg3, label);
        ui.text(c, cell, Role::Mono, 1.45, if n.is_some() { Tone::Fg } else { Tone::Fg4 }, &n.map_or("\u{2014}".into(), |n| n.to_string()));
        if n.is_none() {
            ui.text(c, cell, Role::Mono, 0.36, Tone::Fg3, "Not measured");
        }
    }
    // A gun several of its game's classes carry: the class to play it as.
    if g.game_tags.len() > 1 {
        ui.section(c, parent, "Play as", None);
        let chips = c.spawn((UiStyle { gap: Some(0.25), ..UiStyle::row() }, Node { flex_wrap: FlexWrap::Wrap, row_gap: ui.units.val(0.25), ..default() }, ChildOf(parent))).id();
        let own = body.filter(|b| b.starts_with(&format!("{}:", g.game)));
        let e = press_tag(c, ui, chips, "Its own", own.is_none());
        c.entity(e).insert(GunAction::Body(None));
        for class in &g.game_tags {
            let id = format!("{}:{}", g.game, class.to_lowercase());
            let e = press_tag(c, ui, chips, class, own == Some(id.as_str()));
            c.entity(e).insert(GunAction::Body(Some(id)));
        }
    }
    // Tags.
    ui.section(c, parent, "Tags", None);
    let chips = c.spawn((UiStyle { gap: Some(0.25), ..UiStyle::row() }, Node { flex_wrap: FlexWrap::Wrap, row_gap: ui.units.val(0.25), ..default() }, ChildOf(parent))).id();
    for f in &g.game_tags {
        tick_tag(c, ui, chips, &format!("Class {f}"), false);
    }
    if let Some(p) = s.price {
        tick_tag(c, ui, chips, &format!("Price ${p}"), false);
    }
    for t in g.tags.iter().filter(|t| t.0 != TagGroup::RangeFeel) {
        tick_tag(c, ui, chips, t.1, false);
    }
    if let Some(p) = pinned {
        tick_tag(c, ui, chips, &format!("Pinned {}", p + 1), true);
    }
    // Files.
    ui.section(c, parent, "Files", None);
    let f = ui.boxed(c, parent, UiStyle { gap: Some(0.6), align: Some(AlignItems::Center), ..UiStyle::row() });
    match &st {
        UnitState::Here => {
            ui.mark(c, f, Tone::Fg, 0.4, 0.6);
            ui.text(c, f, Role::Mono, 0.46, Tone::Fg, "Here");
        }
        UnitState::NotHere { bytes } => {
            let b = {
                let e = ui.item(c, f, UiStyle { gap: Some(0.35), align: Some(AlignItems::Center), pad: Some([0.25, 0.4, 0.25, 0.4].into()), ..UiStyle::row() }.outline(Tone::Fg5, 1.0), State::NONE, false);
                c.entity(e).insert(GunAction::Download(g.id.clone()));
                e
            };
            kit_icon(c, look, b, "download", 0.6, Tone::Fg2, 1.0);
            ui.text(c, b, Role::Mono, 0.46, Tone::Fg2, &size_words(*bytes));
            ui.sentence(c, f, 0.62, Tone::Fg3, "Not here yet. Click to download, or pick it.");
        }
        other => {
            let (p, words) = progress_of(other);
            ui.progress(c, f, p, &words, 10.0, 1.1, 0.44, true);
            if let UnitState::Failed { reason } = other {
                ui.sentence(c, f, 0.62, Tone::Fg3, reason);
            }
        }
    }
}

/// The view's systems.
pub fn add(app: &mut App) {
    app.init_resource::<Grid>()
        .init_resource::<GridSel>()
        .init_resource::<GunPicker>()
        .init_resource::<GunCards>()
        .init_resource::<Pickers>()
        .add_message::<GunAction>()
        .add_message::<KeyboardInput>()
        .add_systems(
        Update,
        (keys, search_pointer, pointer, act, sync_grid, follow_cursor, open_close, draw)
            .chain()
            .after(super::SandboxSystems)
            .run_if(resource_exists::<chimera_ui::Theme>)
            .run_if(resource_exists::<chimera_ui::UiUnits>),
    );
    app.add_systems(PostUpdate, fill.after(picker::bind_slots).before(bevy::ui::UiSystems::Prepare).run_if(resource_exists::<chimera_ui::Theme>));
}
