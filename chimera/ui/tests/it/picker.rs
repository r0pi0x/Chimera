//! The kit's picker (chimera_ui::picker): the rows, boxes, toggles and
//! folds as data (the Maps dialog's rules, any catalogue), the search and
//! the chips, the cards' grid for a frame's width, and a built picker's
//! slots bound in both looks and both modes.

use std::collections::BTreeSet;

use bevy::{ecs::schedule::SingleThreadedExecutor, input::mouse::AccumulatedMouseScroll, prelude::*};
use chimera_ui::{
    ChimeraUiPlugin, Theme, UiClock, UiStyle, UiUnits, VirtualLists,
    controls::Ui,
    picker::{
        self, PickerAction, PickerCard, PickerFrame, PickerGame, PickerGroup, PickerItem, PickerLook, PickerMode, PickerRow, PickerState,
        Pickers, Tri, TriBox, TriHit,
    },
};

const SIGNAL: &str = include_str!("../../../../apps/ultimate_gun_game/data/theme.toml");

fn item(id: &str, tags: &[&str]) -> PickerItem {
    PickerItem { tags: tags.iter().map(|s| s.to_string()).collect(), icon: Some("rifle".into()), ..PickerItem::new(id, id) }
}

/// Two games: one with a group of a map with two children and another
/// map; one with ungrouped guns.
fn games() -> Vec<PickerGame> {
    let child = |id: &str| PickerItem { parent: Some("g:big".into()), ..item(id, &[]) };
    vec![
        PickerGame {
            id: "g".into(),
            name: "Game".into(),
            groups: vec![PickerGroup {
                name: "Official maps".into(),
                items: vec![item("g:big", &["size:large"]), child("g:big@west"), child("g:big@east"), item("g:small", &["size:small"])],
            }],
        },
        PickerGame {
            id: "h".into(),
            name: "Other".into(),
            groups: vec![PickerGroup {
                name: String::new(),
                items: vec![item("ak", &["class:rifle"]), item("awp", &["class:sniper"]), item("mp5", &["class:smg"])],
            }],
        },
    ]
}

fn row_of(p: &PickerState, games: &[PickerGame], id: &str) -> PickerRow {
    p.rows(games).into_iter().find(|r| PickerState::item(games, r).is_some_and(|i| i.id == id)).unwrap()
}

#[test]
fn rows_fold_and_an_ungrouped_game_has_no_group_row() {
    let games = games();
    let mut p = PickerState::default();
    // game, group, 4 items; game, 3 items.
    assert_eq!(p.rows(&games).len(), 2 + 4 + 1 + 3);
    p.fold(&games, &PickerRow::Game(0), false);
    assert_eq!(p.rows(&games).len(), 1 + 1 + 3);
    p.fold(&games, &PickerRow::Game(0), true);
    // ← on an ungrouped item folds its game and puts the cursor on it.
    p.cursor = p.rows(&games).iter().position(|r| *r == row_of(&p, &games, "awp")).unwrap();
    assert!(p.left(&games));
    assert_eq!(p.rows(&games)[p.cursor], PickerRow::Game(1));
    assert!(p.is_folded(&games, &PickerRow::Game(1)));
    assert!(p.right(&games));
    assert!(!p.is_folded(&games, &PickerRow::Game(1)));
}

#[test]
fn a_parents_children_go_off_with_it_and_one_stays_on_by_its_mark() {
    let games = games();
    let mut p = PickerState::default();
    let (big, west) = (row_of(&p, &games, "g:big"), row_of(&p, &games, "g:big@west"));
    assert_eq!(p.items_of(&games, &big).len(), 3, "an item's row takes its children");
    assert_eq!(p.counts(&games), (7, 7, 2));
    p.toggle(&games, &west);
    assert_eq!((p.state(&games, &west), p.state(&games, &big), p.state(&games, &PickerRow::Game(0))), (Tri::Off, Tri::Mixed, Tri::Mixed));
    p.toggle(&games, &big);
    assert_eq!(p.state(&games, &big), Tri::On);
    p.toggle(&games, &big);
    assert_eq!(p.open.clone().unwrap(), BTreeSet::from(["g:big".to_string()]));
    p.toggle(&games, &west);
    assert_eq!(p.open.clone().unwrap(), BTreeSet::from(["g:big".to_string(), "+g:big@west".to_string()]));
    assert_eq!(p.state(&games, &big), Tri::Mixed);
    p.off = p.open.take().unwrap();
    assert_eq!(p.off_line(), "+g:big@west,g:big");
    assert_eq!(PickerState::off_from_line(&p.off_line()), p.off);
    p.all_off(&games);
    assert_eq!(p.counts(&games).0, 0);
}

#[test]
fn the_search_and_the_chips_leave_matching_items_and_their_headers() {
    let games = games();
    let mut p = PickerState { search: "aw".into(), ..default() };
    assert_eq!(p.rows(&games), vec![PickerRow::Game(1), row_of(&PickerState::default(), &games, "awp")], "a game with none left is gone");
    p.search = "sniper".into();
    assert_eq!(p.rows(&games).len(), 2, "a tag's words match");
    p.search.clear();
    // Within a facet any, across facets all; `game:` is the item's game.
    p.chips = BTreeSet::from(["class:rifle".to_string(), "class:smg".to_string()]);
    assert_eq!(p.rows(&games).len(), 3);
    p.chips.insert("game:g".into());
    assert!(p.rows(&games).is_empty());
    p.chips = BTreeSet::from(["game:g".to_string()]);
    assert_eq!(p.rows(&games).len(), 6);
    // A header's box is of what it shows: toggling the game under a
    // search sets only the matches.
    p.chips.clear();
    p.search = "small".into();
    p.toggle(&games, &PickerRow::Game(0));
    assert_eq!(p.open.clone().unwrap(), BTreeSet::from(["g:small".to_string()]));
    // The chips the data offers, by facet.
    assert_eq!(PickerState::chips_of(&games, &["class", "game"]), ["class:rifle", "class:sniper", "class:smg", "game:g", "game:h"]);
}

#[test]
fn a_narrow_frame_gets_fewer_smaller_cards() {
    let t = Theme::from_toml(SIGNAL).unwrap();
    let (full_cols, full_w, full_h) = picker::card_grid(&t, 68.0);
    let (dlg_cols, dlg_w, dlg_h) = picker::card_grid(&t, 44.4);
    assert!(dlg_cols < full_cols, "{dlg_cols} < {full_cols}");
    assert!(dlg_w <= full_w + 1e-3 && dlg_h <= full_h + 1e-3);
    for (w, (cols, cw, _)) in [(68.0, (full_cols, full_w, full_h)), (44.4, (dlg_cols, dlg_w, dlg_h))] {
        let gap = t.layout("picker", "card_gap");
        assert!((cols as f32 * cw + (cols as f32 - 1.0) * gap - w).abs() < 1e-3, "the cards fill the width");
        assert!(cw >= t.layout("picker", "card_min_w") - 1e-3 && cw <= t.layout("picker", "card_max_w") + 1e-3);
    }
    assert_eq!(picker::card_grid(&t, 3.0).0, 1, "one column at least");
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::text::TextPlugin, bevy::image::ImagePlugin::default()))
        .add_plugins(ChimeraUiPlugin);
    app.insert_resource(UiUnits::new(1080.0, 1.0));
    app.insert_resource(Theme::from_toml(SIGNAL).unwrap());
    app.insert_resource(UiClock::manual());
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<AccumulatedMouseScroll>();
    app.edit_schedule(PreUpdate, |s| {
        s.set_executor(SingleThreadedExecutor::new());
    });
    app.edit_schedule(Update, |s| {
        s.set_executor(SingleThreadedExecutor::new());
    });
    app.edit_schedule(PostUpdate, |s| {
        s.set_executor(SingleThreadedExecutor::new());
    });
    app
}

/// Builds a picker in a frame `w` cq wide and runs a few frames.
fn build(look: PickerLook, mode: PickerMode, state: PickerState) -> App {
    let frame = PickerFrame { w: 60.0, look, mode, search: true, facets: vec!["class".into()], placeholder: "Find".into(), ..default() };
    build_with(games(), frame, state)
}

/// `build` with its data and frame.
fn build_with(games: Vec<PickerGame>, frame: PickerFrame, state: PickerState) -> App {
    let mut app = app();
    app.world_mut()
        .run_system_once(move |theme: Res<Theme>, units: Res<UiUnits>, mut lists: ResMut<VirtualLists>, mut c: Commands| {
            let ui = Ui::new(&theme, None, *units);
            let root = c.spawn(Node { width: Val::Px(1920.0), height: Val::Px(1080.0), ..default() }).id();
            picker::spawn(&mut c, &ui, root, "test.picker", &games, &state, frame.clone(), &mut lists, UiStyle { grow: Some(1.0), ..UiStyle::column() });
        })
        .unwrap();
    for _ in 0..4 {
        app.update();
    }
    app
}

use bevy::ecs::system::RunSystemOnce;

fn count<C: Component>(app: &mut App) -> usize {
    app.world_mut().query::<&C>().iter(app.world()).count()
}

#[test]
fn a_multi_list_binds_rows_with_boxes_and_header_hits() {
    let mut app = build(PickerLook::List, PickerMode::Multi, PickerState::default());
    assert!(app.world().resource::<Pickers>().0.contains_key("test.picker"), "the snapshot the binding reads");
    // Headers: two games, one group: a hit each; every row a box.
    assert_eq!(count::<TriHit>(&mut app), 3);
    assert_eq!(count::<TriBox>(&mut app), 10);
    let acts: Vec<PickerAction> = app.world_mut().query::<&PickerAction>().iter(app.world()).copied().collect();
    assert!(acts.contains(&PickerAction::Fold(0)) && acts.contains(&PickerAction::Toggle(0)) && acts.contains(&PickerAction::Toggle(2)));
    assert!(acts.contains(&PickerAction::FocusSearch) && acts.contains(&PickerAction::Chip(0)), "the search and the chips");
    assert!(!acts.iter().any(|a| matches!(a, PickerAction::Pick(_))), "no picks in multi mode");
}

#[test]
fn single_cards_pick_and_have_no_boxes() {
    let state = PickerState { current: Some("awp".into()), ..default() };
    let mut app = build(PickerLook::Cards, PickerMode::Single, state);
    assert_eq!(count::<TriBox>(&mut app), 0, "no boxes in single mode");
    assert_eq!(count::<PickerCard>(&mut app), 7, "a card an item (children too)");
    let acts: Vec<PickerAction> = app.world_mut().query::<&PickerAction>().iter(app.world()).copied().collect();
    assert!(acts.iter().any(|a| matches!(a, PickerAction::Pick(_))));
    assert!(acts.contains(&PickerAction::Fold(0)), "a game's header folds");
    let view = &app.world().resource::<Pickers>().0["test.picker"];
    assert!(view.card.0 >= 2, "a 60 cq frame holds several columns: {:?}", view.card);
}

/// Typing in the search field: the kit's text field (a caret, a
/// selection, paste), the rows narrowed, the cursor back on the first.
#[test]
fn typing_edits_the_search_at_the_caret() {
    use chimera_ui::text_edit::{Edit, TextKey};
    let games = games();
    let mut st = PickerState { cursor: 4, ..default() };
    let mut clip = chimera_ui::clipboard::Clipboard::memory(None);
    st.focus_search();
    assert!(st.search_focus && st.search_cursor().is_some());
    for ch in ["s", "m", "a"] {
        assert!(st.search_key(TextKey::Edit(Edit::Insert(ch.into())), &mut clip));
    }
    assert_eq!(st.search, "sma");
    assert_eq!(st.cursor, 0, "a new search starts at the top");
    assert!(st.rows(&games).iter().all(|r| PickerState::item(&games, r).is_none_or(|i| i.id.contains("sma"))));
    st.search_key(TextKey::Edit(Edit::Home(false)), &mut clip);
    st.search_key(TextKey::Edit(Edit::Insert("g:".into())), &mut clip);
    assert_eq!(st.search, "g:sma", "typed at the caret");
    st.search_key(TextKey::Edit(Edit::SelectAll), &mut clip);
    st.search_key(TextKey::Copy, &mut clip);
    st.search_key(TextKey::Edit(Edit::Backspace), &mut clip);
    assert_eq!(st.search, "");
    st.search_key(TextKey::Paste, &mut clip);
    assert_eq!(st.search, "g:sma", "pasted back");
    st.clear_search();
    assert_eq!((st.search.as_str(), st.caret.at), ("", 0));
}

/// The grid keys on cards: ←→ in reading order across games, a game's
/// header folds on ← (the picker's folds, which the list follows, so the
/// folded header is a stop) and opens on →.
#[test]
fn the_grid_keys_walk_cards_and_fold_headers() {
    let frame = PickerFrame { w: 60.0, look: PickerLook::Cards, mode: PickerMode::Single, ..default() };
    let mut app = build_with(games(), frame.clone(), PickerState { cursor: 2, ..default() });
    let w = app.world_mut();
    let games = games();
    let mut st = PickerState { cursor: 2, ..default() };
    let theme = w.resource::<Theme>().clone();
    let view = w.resource::<Pickers>().0["test.picker"].clone();
    let mut lists = w.resource_mut::<VirtualLists>();
    let list = lists.state("test.picker");
    // Right walks the cards (rows 2..=5 are the first game's items, the
    // group header has no card).
    let rows = st.rows(&games);
    assert!(picker::nav(&mut st, &view, list, chimera_ui::VNav::Right));
    assert!(matches!(rows[st.cursor], PickerRow::Item(..)) && st.cursor == 3);
    // Up to the header, then ← folds the game by the picker's state.
    while !matches!(rows[st.cursor], PickerRow::Game(_)) {
        picker::nav(&mut st, &view, list, chimera_ui::VNav::Up);
    }
    assert_eq!(st.cursor, 0);
    assert!(picker::nav(&mut st, &view, list, chimera_ui::VNav::Left));
    assert!(st.folded.contains("g"), "folded by the picker");
    // Built again: the list folds the section too, its header a ←→ stop.
    let view = picker::prepare(&theme, "test.picker", &games, &st, frame, &mut lists);
    let list = lists.state("test.picker");
    assert!(list.is_folded(0), "the list follows the picker's fold");
    assert!(list.stops().contains(&chimera_ui::VCursor::Header(0)));
    assert!(picker::nav(&mut st, &view, list, chimera_ui::VNav::Right));
    assert!(!st.folded.contains("g"), "→ opens it");
}

/// A card's parts: the badge with a family's count, the mark, a download's
/// bar, the slots the screen fills (art without a kit icon, the foot), a
/// single pick's `VHit`, and the side pane.
#[test]
fn cards_carry_badges_families_bars_and_slots() {
    use chimera_ui::picker::{PickerEdge, PickerFamily, PickerPart, PickerSlot};
    let mut games = games();
    let it = &mut games[1].groups[0].items[0];
    (it.badge, it.mark, it.family, it.progress, it.icon) = (Some("CS:S".into()), Some("2".into()), 4, Some(500), None);
    let frame = PickerFrame { w: 80.0, look: PickerLook::Cards, mode: PickerMode::Single, foot: true, side: Some(20.0), ..default() };
    let mut app = build_with(games, frame, PickerState::default());
    let w = app.world_mut();
    let fams: Vec<PickerFamily> = w.query::<&PickerFamily>().iter(w).copied().collect();
    assert_eq!(fams, vec![PickerFamily(4)]);
    let edges: Vec<PickerEdge> = w.query::<&PickerEdge>().iter(w).copied().collect();
    assert_eq!(edges, vec![PickerEdge(0.5)]);
    let slots: Vec<PickerSlot> = w.query::<&PickerSlot>().iter(w).cloned().collect();
    assert_eq!(slots.iter().filter(|s| s.part == PickerPart::Foot).count(), 7, "every card a foot");
    assert_eq!(slots.iter().filter(|s| s.part == PickerPart::Art).map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["ak"]);
    assert_eq!(w.query::<&chimera_ui::VHit>().iter(w).count(), 7, "single cards select through the list");
    let view = &w.resource::<Pickers>().0["test.picker"];
    let (cols, card_w, _) = view.card;
    assert!(cols as f32 * card_w <= 80.0 - 20.0, "the side pane's width is the list's less");
}
