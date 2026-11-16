//! Change gun (B in the sandbox): the full-screen tile view, option B of
//! revision 2 as decided (the design rules, section 21; `sb2-gun-b`,
//! `sb2-gun-filters`, `sb2-gun-tf2`).
//!
//! - **Head**: the breadcrumb (Sandbox, B, Esc) and CHANGE GUN at the
//!   left; the search field at the top centre (no current-gun chip in
//!   this view): typing goes there, matching a gun's name, id, game,
//!   class and tags (`model::GunView::fits`).
//! - **Games** (left, 14 cq): All games and each game with its count (of
//!   the guns the other filters leave) and a line under it (how many are
//!   here, or one download for the whole game); a click ticks a game
//!   (white), All games clears them. A ticked game with facets of its own
//!   shows them under it as tick tags (Team Fortress 2: the classes).
//! - **The grid** (`chimera_ui::vlist`, a recycling grid of 5 tiles a row
//!   under folding class headers): a tile has its game's short name, its
//!   pinned slot, the gun's icon and name, and its foot: its delivery and
//!   fire mode, or its download state (`tile_foot`). A press selects, a
//!   second press switches (the kit's double-click rule); a header folds.
//!   Over the grid: TAGS (the global tag popover, eight groups as tick
//!   tags, and More filters greyed: Later; while it is open typing goes
//!   to its own "Find a tag", which keeps the tags holding the words,
//!   Enter ticks the first, Esc clears it, then closes the popover), the ticked tags with their
//!   crosses, and the count.
//! - **Detail** (right, 25 cq): the game, the name with the kit's icon,
//!   the 3D model's area (here: a stub picture of the gun, drag to turn;
//!   not here: Download to see the model; downloading: the progress),
//!   the stats (damage a hit, fire rate, magazine, range feel), hits to
//!   kill (a 100-point body, a Spartan's 115 of shield and health, the
//!   head), for a gun several classes carry the class to play it as
//!   (Play as: `GunView::body`, sent with the switch), the tags and the
//!   files.
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
//!   and digits go to the search (not with Ctrl or Alt held), but before
//!   anything is typed I shows the large model and R retries the
//!   selected gun's failed download (on any other gun R types); B types
//!   (Esc leaves). Ctrl+I and Ctrl+R do the same at any time; Ctrl+1–6
//!   pins; Backspace deletes a letter; Esc closes the popover, then
//!   clears the search, then closes the view.
//!
//! The user's decisions (the design rules, section 22): a tile's
//! download state is a bar along its bottom edge (its words in the
//! foot), so the keyboard cursor is the tile's accent outline; a drop
//! shows a ghost of the gun in the slot (`model::drop_hint`); the keys
//! below.
//!
//! A tile's name is cut to the tile with the kit's ellipsis
//! (`chimera_ui::Ellipsize`); the detail pane has it whole.
//!
//! Known shortcuts: a slot's tiles are spawned again when it shows
//! another row (the slots themselves stay; the kit's binding in place is
//! for text); the model area shows the kit's icon large, not a render;
//! the drag shows the slot under the pointer but no ghost tile.

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    ui::{DeprecatedInteraction as Interaction, FocusPolicy, RelativeCursorPosition},
    window::PrimaryWindow,
};
use chimera_ui::{
    Role, Tone, UiParityId, UiStyle, VCursor, VHit, VListEvent, VNav, VSection, VShape, VSlot, VirtualLists,
    controls::{Ui, clickable},
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

/// Tiles a row, a tile's height, a header's, the gap (cq).
pub const COLS: usize = 5;
pub const TILE_H: f32 = 5.6;
pub const HEAD_H: f32 = 1.6;
pub const GAP: f32 = 0.45;

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
    Switch,
    Close,
}

/// A pinned slot the pointer can drop on.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PinDrop(pub usize);

/// The view's parts (made at opening).
#[derive(Resource, Clone, Debug)]
pub struct Parts {
    pub root: Entity,
    pub search: Entity,
    pub games: Entity,
    pub bar: Entity,
    pub detail: Entity,
    pub pins: Entity,
    pub pop: Entity,
    pub view: Entity,
    size: (u32, u32),
}

/// The grid's sections as shown: (name, guns' indices).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Grid(pub Vec<(&'static str, Vec<usize>)>);

impl Grid {
    pub fn gun_at(&self, c: VCursor) -> Option<usize> {
        match c {
            VCursor::Item(s, i) => self.0.get(s)?.1.get(i).copied(),
            VCursor::Header(_) => None,
        }
    }

    pub fn cursor_of(&self, gun: usize) -> Option<VCursor> {
        self.0.iter().enumerate().find_map(|(s, (_, v))| v.iter().position(|g| *g == gun).map(|i| VCursor::Item(s, i)))
    }
}

/// Keeps the grid's sections from the filters, and the cursor and the
/// selected gun in step.
#[allow(clippy::too_many_arguments)]
pub fn sync_grid(
    ui: Res<SandboxUi>,
    cat: Res<SandboxCatalogue>,
    progress: Res<UnitProgress>,
    mut grid: ResMut<Grid>,
    mut lists: ResMut<VirtualLists>,
    mut sel: ResMut<GridSel>,
    mut pins: Local<super::model::Pins>,
) {
    if ui.screen != SandboxScreen::ChangeGun {
        sel.0 = None;
        return;
    }
    let shown = ui.view.shown(&cat.guns, &|g| SandboxCatalogue::files(g, &progress));
    let st = lists.state(LIST);
    st.set_shape(VShape::grid(COLS, HEAD_H, TILE_H, GAP));
    if grid.0 != shown {
        grid.0 = shown;
        st.set_sections(grid.0.iter().map(|(n, v)| VSection::new(*n, v.len())).collect());
    }
    // What the tiles show beyond the sections: the downloads, the pins.
    if progress.is_changed() || cat.is_changed() || *pins != ui.pins {
        *pins = ui.pins.clone();
        st.bump();
    }
    // The selected gun moves the cursor (opening, a search).
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
    if let Some(g) = grid.gun_at(c).and_then(|i| cat.guns.get(i))
        && ui.view.selected.as_deref() != Some(&g.id)
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
    console: Option<Res<crate::client::console::ConsoleUi>>,
    cat: Res<SandboxCatalogue>,
    progress: Res<UnitProgress>,
    mut ui: ResMut<SandboxUi>,
    mut lists: ResMut<VirtualLists>,
    mut out: MessageWriter<GunAction>,
    mut requests: MessageWriter<UnitRequest>,
) {
    if ui.screen != SandboxScreen::ChangeGun || console.is_some_and(|c| c.open) {
        typed.clear();
        return;
    }
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let k = |c: KeyCode| keys.just_pressed(c);
    let st = lists.state(LIST);
    if k(KeyCode::Escape) {
        typed.clear();
        ui.took_esc = true;
        if ui.view.tags_open && !ui.view.tag_search.is_empty() {
            ui.view.tag_search.clear();
        } else if ui.view.tags_open {
            ui.view.tags_open = false;
        } else if !ui.view.search.is_empty() {
            ui.view.search.clear();
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
        if k(key) {
            st.nav(nav);
        }
    }
    // `\` (by its physical key, so typing never takes it) and Enter on a
    // header: fold the class, or open it again and go back to the tile the
    // fold left.
    let fold_key = k(KeyCode::Backslash) || k(KeyCode::IntlBackslash);
    let enter = k(KeyCode::Enter) || k(KeyCode::NumpadEnter);
    match st.cursor {
        Some(VCursor::Item(sec, i)) if fold_key => {
            st.fold(sec, true);
            ui.view.fold_return = Some((sec, i));
        }
        Some(VCursor::Header(sec)) if fold_key || enter => {
            if st.is_folded(sec) {
                st.fold(sec, false);
                if let Some((rs, ri)) = ui.view.fold_return.take()
                    && rs == sec
                {
                    st.select(VCursor::Item(rs, ri));
                }
            } else {
                st.fold(sec, true);
            }
        }
        Some(VCursor::Item(..)) if enter => {
            out.write(GunAction::Switch);
        }
        _ => {}
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
    // Typing: letters and digits go to the search (not with
    // Ctrl or Alt held, and never `\`); before anything is typed, I is
    // the model view and R retries the selected gun's failed download
    // (otherwise R types).
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    for e in typed.read() {
        if e.state != ButtonState::Pressed || ctrl || alt || matches!(e.key_code, KeyCode::Backslash | KeyCode::IntlBackslash) {
            continue;
        }
        let empty = ui.view.search.is_empty();
        match &e.logical_key {
            Key::Backspace => {
                ui.view.search.pop();
            }
            Key::Space if !empty => ui.view.search.push(' '),
            Key::Character(c) if empty && c.eq_ignore_ascii_case("i") => ui.view.detail = !ui.view.detail,
            Key::Character(c) if empty && c.eq_ignore_ascii_case("r") && failed.is_some() => {
                if let Some((game, unit)) = failed.clone() {
                    requests.write(UnitRequest::Retry { game, unit });
                }
            }
            Key::Character(c) if ui.view.search.len() < 48 => ui.view.search.push_str(c),
            _ => {}
        }
    }
}

/// Clicks, the list's events and drags.
#[allow(clippy::too_many_arguments)]
fn pointer(
    pressed: Query<(&Interaction, &GunAction), Changed<Interaction>>,
    mut list_events: MessageReader<VListEvent>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    window: Query<&Window, With<PrimaryWindow>>,
    tiles: Query<(&Interaction, &VHit)>,
    slots: Query<&VSlot>,
    drops: Query<(&PinDrop, &RelativeCursorPosition)>,
    lists: Res<VirtualLists>,
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
            GunAction::Switch => {
                let Some(id) = world.resource::<SandboxUi>().view.selected.clone() else { continue };
                world.write_message(SandboxAction::Switch(id));
                world.resource_mut::<SandboxUi>().screen = SandboxScreen::None;
            }
            GunAction::Close => {
                world.write_message(SandboxAction::Close);
            }
        }
    }
}

// ---------------------------------------------------------------------
// Drawing.

/// Opens or closes the view's root; builds its fixed parts at opening.
fn open_close(look: Look, ui_state: Res<SandboxUi>, parts: Option<Res<Parts>>, mut views: ResMut<chimera_ui::ScrollViews>, mut c: Commands) {
    let want = look.on() && ui_state.screen == SandboxScreen::ChangeGun;
    let size = (look.units.height as u32, (look.units.scale_factor * 100.0) as u32);
    if let Some(p) = parts.as_deref()
        && (!want || p.size != size)
    {
        c.entity(p.root).despawn();
        c.remove_resource::<Parts>();
        if !want {
            views.forget(LIST);
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
            FocusPolicy::Block,
            Interaction::default(),
            UiParityId::new("gun.view"),
            Name::new("Change gun"),
        ))
        .id();
    // The head: the breadcrumb and title, the search at the centre.
    let head = ui.boxed(&mut c, root, UiStyle { align: Some(AlignItems::FlexStart), h: Some(3.4), shrink: Some(0.0), ..UiStyle::row() });
    let crumbs = ui.boxed(&mut c, head, UiStyle { gap: Some(0.35), ..UiStyle::column() });
    let back = ui.boxed(&mut c, crumbs, UiStyle { gap: Some(0.45), align: Some(AlignItems::Center), ..UiStyle::row() });
    kit_icon(&mut c, &look, back, "back", 0.5, Tone::Fg2, 1.0);
    ui.text(&mut c, back, Role::Mono, 0.52, Tone::Fg2, "Sandbox");
    ui.kbd(&mut c, back, "Esc", 0.5, Tone::Fg3);
    let t = ui.text(&mut c, crumbs, Role::DisplayBold, 1.9, Tone::Fg, "Change gun");
    c.entity(t).insert(UiParityId::new("gun.title"));
    let search = c
        .spawn((
            UiStyle { gap: Some(0.6), align: Some(AlignItems::Center), w: Some(26.0), h: Some(2.1), pad: Some([0.0, 0.8, 0.0, 0.8].into()), ..UiStyle::row() }
                .abs([Some(57.0), Some(0.4), None, None])
                .bg_a(Tone::Fg, 0.05),
            Node::default(),
            UiTransform { translation: Val2::percent(-50.0, 0.0), ..default() },
            UiParityId::new("gun.search"),
            ChildOf(head),
        ))
        .id();
    // The body: games, the grid, the detail.
    let body = ui.boxed(&mut c, root, UiStyle { gap: Some(1.3), grow: Some(1.0), align: Some(AlignItems::Stretch), ..UiStyle::row() });
    c.entity(body).insert(Node { min_height: Val::Px(0.0), ..default() });
    let games = c.spawn((UiStyle { w: Some(14.0), gap: Some(0.1), shrink: Some(0.0), ..UiStyle::column() }, Node::default(), UiParityId::new("gun.games"), ChildOf(body))).id();
    let main = ui.boxed(&mut c, body, UiStyle { grow: Some(1.0), gap: Some(0.4), ..UiStyle::column() });
    c.entity(main).insert(Node { min_width: Val::Px(0.0), min_height: Val::Px(0.0), ..default() });
    let bar = ui.boxed(&mut c, main, UiStyle { gap: Some(0.35), align: Some(AlignItems::Center), h: Some(1.6), shrink: Some(0.0), ..UiStyle::row() });
    let list = ui.virtual_list(&mut c, main, LIST, UiStyle { grow: Some(1.0), ..UiStyle::row() }, 45.0, VShape::grid(COLS, HEAD_H, TILE_H, GAP));
    c.entity(list.scroll.frame).insert(Node { min_height: Val::Px(0.0), ..default() });
    for &slot in &list.slots {
        c.entity(slot).insert(Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, display: Display::None, column_gap: look.val(GAP), ..default() });
    }
    let detail = c
        .spawn((UiStyle { w: Some(25.0), gap: Some(0.5), pad: Some([0.0, 0.0, 0.0, 1.2].into()), shrink: Some(0.0), ..UiStyle::column() }, Node::default(), UiParityId::new("gun.detail"), ChildOf(body)))
        .id();
    // The foot: the keys, the pinned bar at the right.
    let foot = ui.boxed(&mut c, root, UiStyle { align: Some(AlignItems::FlexEnd), shrink: Some(0.0), ..UiStyle::row() });
    let keys = ui.boxed(&mut c, foot, UiStyle { gap: Some(1.1), align: Some(AlignItems::Center), h: Some(2.6), ..UiStyle::row() });
    for (k, w) in [
        ("\u{2191}\u{2193}\u{2190}\u{2192}", "Tile"),
        ("Enter", "Switch, or fold"),
        ("\\", "Fold this class"),
        ("I", "Model"),
        ("Ctrl 1-6", "Pin"),
        ("R", "Retry"),
        ("Type", "Find"),
        ("Esc", "Back to the range"),
    ] {
        key_hint(&mut c, &ui, keys, k, w, 0.5, Tone::Fg2);
    }
    ui.boxed(&mut c, foot, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    let pins = ui.boxed(&mut c, foot, UiStyle { gap: Some(0.9), align: Some(AlignItems::Center), ..UiStyle::row() });
    let pop = c.spawn((Node { position_type: PositionType::Absolute, ..default() }, GlobalZIndex(Z + 2), ChildOf(root))).id();
    c.insert_resource(Parts { root, search, games, bar, detail, pins, pop, view: list.scroll.view, size });
}

/// What the rebuilt parts show.
#[derive(Clone, Debug, Default, PartialEq)]
struct Shown {
    view: super::model::GunView,
    pins: Vec<Option<GunFace>>,
    current: Option<String>,
    drop: Option<usize>,
    dragging: bool,
    progress: UnitProgress,
    catalogue: usize,
    grid_count: usize,
}

#[allow(clippy::too_many_arguments)]
fn draw(
    look: Look,
    ui_state: Res<SandboxUi>,
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
        pins: ui_state.pins.0.iter().map(|p| p.as_deref().map(face)).collect(),
        current: ui_state.current.clone(),
        drop: ui_state.drag.as_ref().and_then(|d| d.over),
        dragging: ui_state.drag.is_some(),
        progress: progress.clone(),
        catalogue: cat.guns.len(),
        grid_count: grid.0.iter().map(|s| s.1.len()).sum(),
    };
    let fresh = last.as_ref().is_none_or(|(r, _)| *r != p.root);
    let prev = if fresh { None } else { last.as_ref().map(|l| l.1.clone()) };
    if prev.as_ref() == Some(&next) {
        return;
    }
    let ui = Ui::new(&look.theme, look.fonts.as_deref(), *look.units);
    // Rebuild every part (they're small; the grid is the list's own).
    for e in [p.search, p.games, p.bar, p.detail, p.pins, p.pop] {
        c.entity(e).despawn_children();
    }
    search_field(&mut c, &ui, &look, p.search, &next.view.search);
    games_column(&mut c, &ui, p.games, &cat.guns, &ui_state, &progress);
    filter_bar(&mut c, &ui, &look, p.bar, &ui_state, next.grid_count, &cat.guns, &progress);
    if let Some(g) = ui_state.view.selected.as_deref().and_then(|id| cat.get(id)) {
        detail(&mut c, &ui, &look, p.detail, g, &progress, ui_state.view.detail, ui_state.pins.slot_of(&g.id), ui_state.view.body.as_deref());
    }
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
    if ui_state.view.tags_open {
        tag_popover(&mut c, &ui, &look, p.pop, &ui_state);
    }
    *last = Some((p.root, next));
}

fn search_field(c: &mut Commands, ui: &Ui, look: &Look, parent: Entity, text: &str) {
    kit_icon(c, look, parent, "search", 0.8, Tone::Fg3, 1.0);
    let w = ui.boxed(c, parent, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    if text.is_empty() {
        ui.sentence(c, w, 0.78, Tone::Fg3, "Find a gun, a game or a tag");
    } else {
        ui.text_plain(c, w, Role::Text, 0.84, Tone::Fg, text);
        c.spawn((Node { width: Val::Px(look.hair().max(1.5)), height: look.val(1.0), ..default() }, BackgroundColor(look.tone(Tone::Fg)), ChildOf(w)));
    }
    ui.kbd(c, parent, "Type", 0.44, Tone::Fg3);
    c.spawn((
        Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, bottom: Val::ZERO, height: Val::Px(look.hair()), ..default() },
        BackgroundColor(look.tone(if text.is_empty() { Tone::Fg3 } else { Tone::Acc })),
        ChildOf(parent),
    ));
}

fn games_column(c: &mut Commands, ui: &Ui, parent: Entity, guns: &[SandboxGun], s: &SandboxUi, progress: &UnitProgress) {
    let l = ui.boxed(c, parent, UiStyle { margin: Some([0.15, 0.0, 0.45, 0.0].into()), ..UiStyle::row() });
    ui.text(c, l, Role::Mono, 0.46, Tone::Fg3, "Games");
    let fits = |g: &SandboxGun| s.view.fits_but_games(g, SandboxCatalogue::files(g, progress));
    let all = guns.iter().filter(|g| fits(g)).count();
    let row = |c: &mut Commands, name: &str, count: usize, sel: bool, sub: Option<String>, new: bool, action: GunAction| {
        let tone = if sel { Tone::Ink } else if count == 0 { Tone::Fg4 } else { Tone::Fg2 };
        let mut st = UiStyle { gap: Some(0.3), pad: Some([0.45, 0.7, 0.45, 0.7].into()), min_w: None, ..UiStyle::column() };
        if sel {
            st = st.bg(Tone::Fg);
        }
        let r = c.spawn((st, Node { min_height: ui.units.val(2.0), justify_content: JustifyContent::Center, ..default() }, action, clickable(), ChildOf(parent))).id();
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
        let count = mine.iter().filter(|g| fits(g)).count();
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
                let t = tick_tag(c, ui, chips, &fc, on);
                c.entity(t).insert((GunAction::Facet(id.to_string(), fc.clone()), clickable()));
            }
        }
    }
}

/// A tick tag: Mono .46, outlined; on, a white fill.
pub fn tick_tag(c: &mut Commands, ui: &Ui, parent: Entity, text: &str, on: bool) -> Entity {
    let s = UiStyle { pad: Some([0.26, 0.4, 0.22, 0.4].into()), shrink: Some(0.0), ..UiStyle::row() };
    let s = if on { s.bg(Tone::Fg) } else { s.outline(Tone::Fg4, 1.0) };
    let b = c.spawn((s, Node::default(), ChildOf(parent))).id();
    ui.text(c, b, Role::Mono, 0.46, if on { Tone::Ink } else { Tone::Fg2 }, text);
    b
}

#[allow(clippy::too_many_arguments)]
fn filter_bar(c: &mut Commands, ui: &Ui, look: &Look, parent: Entity, s: &SandboxUi, shown: usize, guns: &[SandboxGun], progress: &UnitProgress) {
    let b = c
        .spawn((
            UiStyle { gap: Some(0.35), align: Some(AlignItems::Center), pad: Some([0.3, 0.5, 0.3, 0.5].into()), ..UiStyle::row() }
                .bg_a(Tone::Fg, if s.view.tags_open { 0.14 } else { 0.04 })
                .outline(if s.view.tags_open { Tone::Fg2 } else { Tone::Fg3 }, 1.0),
            Node::default(),
            GunAction::TagsButton,
            clickable(),
            UiParityId::new("gun.tags"),
            ChildOf(parent),
        ))
        .id();
    kit_icon(c, look, b, "filter", 0.6, Tone::Fg, 1.0);
    ui.text(c, b, Role::Mono, 0.48, Tone::Fg, "Tags");
    for &(g, t) in &s.view.tags {
        let chip = c
            .spawn((UiStyle { gap: Some(0.3), align: Some(AlignItems::Center), pad: Some([0.26, 0.4, 0.22, 0.4].into()), ..UiStyle::row() }.bg(Tone::Fg), Node::default(), GunAction::Tag(g, t), clickable(), ChildOf(parent)))
            .id();
        ui.text(c, chip, Role::Mono, 0.46, Tone::Ink, t);
        kit_icon(c, look, chip, "close", 0.36, Tone::Ink, 1.0);
    }
    let more = ui.boxed(c, parent, UiStyle { gap: Some(0.35), align: Some(AlignItems::Center), pad: Some([0.3, 0.5, 0.3, 0.5].into()), margin: Some([0.0, 0.0, 0.0, 0.2].into()), ..UiStyle::row() }.outline(Tone::Fg5, 1.0));
    ui.text(c, more, Role::Mono, 0.46, Tone::Fg4, "More filters");
    let later = ui.boxed(c, more, UiStyle { pad: Some([0.12, 0.25, 0.12, 0.25].into()), ..UiStyle::row() }.outline(Tone::Fg5, 1.0));
    ui.text(c, later, Role::Mono, 0.38, Tone::Fg3, "Later");
    ui.boxed(c, parent, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    let here = guns.iter().filter(|g| SandboxCatalogue::files(g, progress) == Files::Here).count();
    let words = if shown == guns.len() { format!("{shown} guns \u{00b7} {here} here") } else { format!("{shown} of {} guns", guns.len()) };
    ui.text(c, parent, Role::Mono, 0.48, Tone::Fg3, &words);
}

fn tag_popover(c: &mut Commands, ui: &Ui, look: &Look, parent: Entity, s: &SandboxUi) {
    let p = c
        .spawn((
            UiStyle { w: Some(36.0), gap: Some(0.6), pad: Some([0.9, 1.0, 0.9, 1.0].into()), ..UiStyle::column() }.abs([Some(16.6), Some(8.4), None, None]).bg(Tone::Ink2).outline(Tone::Fg3, 1.0),
            Node::default(),
            BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.7), Val::ZERO, look.val(1.4), Val::ZERO, look.val(3.0)),
            FocusPolicy::Block,
            Interaction::default(),
            UiParityId::new("gun.popover"),
            ChildOf(parent),
        ))
        .id();
    let find = ui.boxed(c, p, UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), h: Some(1.7), pad: Some([0.0, 0.6, 0.0, 0.6].into()), ..UiStyle::row() }.bg_a(Tone::Fg, 0.05));
    c.entity(find).insert(UiParityId::new("gun.popover.find"));
    let typed = &s.view.tag_search;
    kit_icon(c, look, find, "search", 0.7, if typed.is_empty() { Tone::Fg3 } else { Tone::Fg }, 1.0);
    if typed.is_empty() {
        ui.sentence(c, find, 0.7, Tone::Fg3, "Find a tag");
    } else {
        ui.text_plain(c, find, Role::Text, 0.74, Tone::Fg, typed);
        c.spawn((Node { width: Val::Px(look.hair().max(1.5)), height: look.val(0.9), ..default() }, BackgroundColor(look.tone(Tone::Fg)), ChildOf(find)));
    }
    ui.boxed(c, find, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    ui.text(c, find, Role::Mono, 0.42, Tone::Fg3, if typed.is_empty() { "Type to find \u{00b7} every game's guns carry these" } else { "Enter ticks the first \u{00b7} Esc clears" });
    c.spawn((
        Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, bottom: Val::ZERO, height: Val::Px(look.hair()), ..default() },
        BackgroundColor(look.tone(if typed.is_empty() { Tone::Fg4 } else { Tone::Acc })),
        ChildOf(find),
    ));
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
            let e = tick_tag(c, ui, chips, t, on);
            c.entity(e).insert((GunAction::Tag(g, t), clickable()));
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

/// A tile's foot (module doc): the delivery and fire mode, or the
/// download state (along the bottom edge).
fn tile_foot(c: &mut Commands, ui: &Ui, look: &Look, tile: Entity, g: &SandboxGun, st: &UnitState, on: bool) {
    let foot = ui.boxed(c, tile, UiStyle { gap: Some(0.4), align: Some(AlignItems::Center), h: Some(0.9), ..UiStyle::row() });
    match st {
        UnitState::Here => {
            let words: Vec<&str> = g.tags.iter().filter(|t| matches!(t.0, TagGroup::Delivery | TagGroup::FireMode | TagGroup::Optics)).map(|t| t.1).take(2).collect();
            ui.text(c, foot, Role::Mono, 0.36, Tone::Fg3, &words.join(" \u{00b7} "));
        }
        UnitState::NotHere { bytes } => {
            let b = c
                .spawn((
                    UiStyle { gap: Some(0.3), align: Some(AlignItems::Center), pad: Some([0.12, 0.25, 0.12, 0.25].into()), ..UiStyle::row() }.outline(Tone::Fg5, 1.0),
                    Node::default(),
                    GunAction::Download(g.id.clone()),
                    clickable(),
                    ChildOf(foot),
                ))
                .id();
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
    // The bottom edge: the download's bar (the cursor is an outline, so
    // they never stack).
    if matches!(st, UnitState::Downloading { .. } | UnitState::Queued { .. }) && !on {
        let edge = c
            .spawn((
                Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, bottom: Val::ZERO, height: look.val(0.24), ..default() },
                BackgroundColor(look.theme.tone(Tone::Fg5)),
                ChildOf(tile),
            ))
            .id();
        c.spawn((
            Node { width: Val::Percent(st.share() * 100.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(look.theme.tone(Tone::Fg2)),
            ChildOf(edge),
        ));
    }
}

/// Binds the grid's slots (module doc, "Known shortcuts": a rebound slot
/// is filled again).
#[allow(clippy::too_many_arguments)]
fn bind(
    look: Look,
    lists: Res<VirtualLists>,
    grid: Res<Grid>,
    cat: Res<SandboxCatalogue>,
    progress: Res<UnitProgress>,
    ui_state: Res<SandboxUi>,
    slots: Query<(Entity, &VSlot), Changed<VSlot>>,
    mut c: Commands,
) {
    let Some(st) = lists.get(LIST) else { return };
    let ui = Ui::new(&look.theme, look.fonts.as_deref(), *look.units);
    for (slot, s) in &slots {
        c.entity(slot).despawn_children();
        let Some(line) = s.line.and_then(|l| st.layout.lines.get(l)) else { continue };
        match *line {
            VLine::Header(sec) => {
                let Some((name, guns)) = grid.0.get(sec) else { continue };
                let folded = st.is_folded(sec);
                let focus = st.cursor == Some(VCursor::Header(sec));
                let h = c
                    .spawn((
                        UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), pad: Some([0.0, 0.4, 0.0, 0.4].into()), grow: Some(1.0), ..UiStyle::row() }.bg_a(Tone::Fg, if focus { 0.08 } else { 0.0 }),
                        Node::default(),
                        VHit { slot, col: None },
                        clickable(),
                        ChildOf(slot),
                    ))
                    .id();
                if focus {
                    ui.focus_bar(&mut c, h, 0.24);
                }
                if folded {
                    ui.hairline(&mut c, h, Tone::Fg5, 1.0);
                }
                kit_icon(&mut c, &look, h, if folded { "chev_right" } else { "chevron" }, 0.5, Tone::Fg2, 1.0);
                ui.text(&mut c, h, Role::Mono, 0.5, if focus { Tone::Fg } else { Tone::Fg2 }, name);
                ui.text(&mut c, h, Role::Mono, 0.48, Tone::Fg4, &guns.len().to_string());
                if folded {
                    // A preview of what's folded: the first few icons.
                    ui.boxed(&mut c, h, UiStyle { grow: Some(1.0), ..UiStyle::row() });
                    let prev = ui.boxed(&mut c, h, UiStyle { gap: Some(0.7), ..UiStyle::row() });
                    for &i in guns.iter().take(6) {
                        if let Some(g) = cat.guns.get(i) {
                            gun_icon(&mut c, &look, prev, &g.id, g.icon_class, 0.5, Tone::Fg4, 1.0);
                        }
                    }
                }
            }
            VLine::Row { section, first, count } => {
                for col in 0..COLS {
                    // Never wider than its share (a long name is cut, not
                    // pushing its neighbours: `min_width` 0 under flex).
                    let cell = c.spawn((Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), height: Val::Percent(100.0), ..default() }, ChildOf(slot))).id();
                    if col >= count {
                        continue;
                    }
                    let Some(g) = grid.gun_at(VCursor::Item(section, first + col)).and_then(|i| cat.guns.get(i)) else { continue };
                    let on = st.cursor == Some(VCursor::Item(section, first + col));
                    let unit = SandboxCatalogue::unit_state(g, &progress);
                    let here = unit.is_here();
                    let tile = c
                        .spawn((
                            UiStyle { gap: Some(0.45), align: Some(AlignItems::Center), justify: Some(JustifyContent::Center), grow: Some(1.0), pad: Some([0.0, 0.4, 0.0, 0.4].into()), ..UiStyle::column() }
                                .bg_a(Tone::Fg, if on { 0.1 } else { 0.035 })
                                .outline(if on { Tone::Acc } else { Tone::Fg5 }, 1.0),
                            Node { min_width: Val::Px(0.0), overflow: Overflow::clip_x(), ..default() },
                            VHit { slot, col: Some(col) },
                            clickable(),
                            UiParityId::new(format!("gun.tile.{}", g.id)),
                            ChildOf(cell),
                        ))
                        .id();
                    if on {
                        // The cursor: an accent outline two pixels in.
                        let px = (2.0 * look.units.scale_factor).round() / look.units.scale_factor;
                        c.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::ZERO,
                                right: Val::ZERO,
                                top: Val::ZERO,
                                bottom: Val::ZERO,
                                border: UiRect::all(Val::Px(px)),
                                ..default()
                            },
                            BorderColor::all(look.tone(Tone::Acc)),
                            Pickable::IGNORE,
                            ChildOf(tile),
                        ));
                    }
                    let top = ui.boxed(&mut c, tile, UiStyle { justify: Some(JustifyContent::SpaceBetween), align: Some(AlignItems::Center), ..UiStyle::row() }.abs([Some(0.4), Some(0.38), Some(0.4), None]));
                    ui.text(&mut c, top, Role::Mono, 0.38, Tone::Fg3, &g.game_short);
                    if let Some(p) = ui_state.pins.slot_of(&g.id) {
                        let pm = ui.boxed(&mut c, top, UiStyle { w: Some(0.95), h: Some(0.95), justify: Some(JustifyContent::Center), align: Some(AlignItems::Center), ..UiStyle::row() }.bg_a(Tone::Fg, 0.1).outline(Tone::Fg3, 1.0));
                        ui.text(&mut c, pm, Role::Mono, 0.42, Tone::Fg, &(p + 1).to_string());
                    }
                    let tone = if here { Tone::Fg } else { Tone::Fg2 };
                    let ic = ui.boxed(&mut c, tile, UiStyle { margin: Some([0.5, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
                    gun_icon(&mut c, &look, ic, &g.id, g.icon_class, 1.55, tone, 1.0);
                    // The name cut to the tile with the kit's ellipsis
                    // (`chimera_ui::Ellipsize`): long ones never overlap.
                    let name = ui.text(&mut c, tile, Role::DisplayRegular, 0.66, tone, &g.name);
                    c.entity(name).insert(chimera_ui::Ellipsize::in_parent(g.name.to_uppercase()));
                    tile_foot(&mut c, &ui, &look, tile, g, &unit, on);
                }
            }
        }
    }
}

/// Hits to kill a body of `hp` at `dmg` a hit (None: not measured).
pub fn hits_to_kill(hp: f32, dmg: Option<f32>) -> Option<u32> {
    let d = dmg.filter(|d| *d > 0.0)?;
    Some((hp / d).ceil() as u32)
}

#[allow(clippy::too_many_arguments)]
fn detail(c: &mut Commands, ui: &Ui, look: &Look, parent: Entity, g: &SandboxGun, progress: &UnitProgress, big: bool, pinned: Option<usize>, body: Option<&str>) {
    c.spawn((Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, width: Val::Px(look.hair()), ..default() }, BackgroundColor(look.tone(Tone::Fg5)), ChildOf(parent)));
    ui.text(c, parent, Role::Mono, 0.46, Tone::Fg3, &g.game_name);
    let name = ui.boxed(c, parent, UiStyle { gap: Some(0.9), align: Some(AlignItems::Center), ..UiStyle::row() });
    let n = ui.text(c, name, Role::DisplayBold, 1.35, Tone::Fg, &g.name);
    c.entity(n).insert(UiParityId::new("gun.detail.name"));
    ui.boxed(c, name, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    let key = g.icon_class.unwrap_or("rifle");
    kit_icon(c, look, name, key, 0.95, Tone::Fg2, 1.0);
    // The model's area (a stub picture until a render can come).
    let st = SandboxCatalogue::unit_state(g, progress);
    let viewer = c
        .spawn((
            UiStyle { h: Some(if big { 20.0 } else { 11.0 }), justify: Some(JustifyContent::Center), align: Some(AlignItems::Center), shrink: Some(0.0), ..UiStyle::column() }.bg(Tone::Ink).outline(Tone::Fg5, 1.0),
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
            let b = c
                .spawn((
                    UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), pad: Some([0.55, 0.8, 0.55, 0.8].into()), ..UiStyle::row() }
                        .abs([None, None, None, None])
                        .bg_a(Tone::Ink, 0.6)
                        .outline(Tone::Fg3, 1.0),
                    Node::default(),
                    GunAction::Download(g.id.clone()),
                    clickable(),
                    ChildOf(viewer),
                ))
                .id();
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
        let e = tick_tag(c, ui, chips, "Its own", own.is_none());
        c.entity(e).insert((GunAction::Body(None), clickable()));
        for class in &g.game_tags {
            let id = format!("{}:{}", g.game, class.to_lowercase());
            let e = tick_tag(c, ui, chips, class, own == Some(id.as_str()));
            c.entity(e).insert((GunAction::Body(Some(id)), clickable()));
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
            let b = c
                .spawn((UiStyle { gap: Some(0.35), align: Some(AlignItems::Center), pad: Some([0.25, 0.4, 0.25, 0.4].into()), ..UiStyle::row() }.outline(Tone::Fg5, 1.0), Node::default(), GunAction::Download(g.id.clone()), clickable(), ChildOf(f)))
                .id();
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
    app.init_resource::<Grid>().init_resource::<GridSel>().add_message::<GunAction>().add_message::<KeyboardInput>().add_systems(
        Update,
        (keys, pointer, act, sync_grid, follow_cursor, open_close, draw)
            .chain()
            .after(super::SandboxSystems)
            .run_if(resource_exists::<chimera_ui::Theme>)
            .run_if(resource_exists::<chimera_ui::UiUnits>),
    );
    app.add_systems(PostUpdate, bind.after(chimera_ui::vlist::place_slots).before(bevy::ui::UiSystems::Prepare).run_if(resource_exists::<chimera_ui::Theme>));
}
