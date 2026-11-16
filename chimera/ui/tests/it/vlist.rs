//! The recycling list (chimera_ui::vlist): the layout and the keys as
//! data, then a built list scrolled by the wheel (the layout's sizes set
//! by hand, as in `scroll`): the pool stays the same entities, slots
//! rebind only when a line moves into them, the double press activates;
//! and the instruction count of scrolling 2,000 items against a rebuild
//! of the rows in view.

use std::collections::BTreeSet;

use bevy::{
    ecs::schedule::SingleThreadedExecutor,
    input::mouse::{AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
    ui::{ComputedNode, DeprecatedInteraction as Interaction, RelativeCursorPosition, UiGlobalTransform},
};
use chimera_ui::{
    ChimeraUiPlugin, ScrollViews, Theme, UiClock, UiStyle, UiUnits,
    controls::{DOUBLE_CLICK_SECS, Ui},
    vlist::{self, VCursor, VHit, VLine, VListEvent, VListParts, VNav, VSection, VShape, VSlot, VirtualLists},
};

const SIGNAL: &str = include_str!("../../../../apps/ultimate_gun_game/data/theme.toml");
const DT: f32 = 1.0 / 60.0;
const PX: f32 = 19.2;
const VIEW: f32 = 20.0;

fn grid() -> VShape {
    VShape::grid(5, 1.8, 5.6, 0.4)
}

fn sections() -> Vec<VSection> {
    vec![VSection::new("pistols", 9), VSection::new("smgs", 7), VSection::new("rifles", 6)]
}

fn state(secs: Vec<VSection>, shape: VShape) -> chimera_ui::VListState {
    let mut s = chimera_ui::VListState::default();
    s.set_shape(shape);
    s.set_sections(secs);
    s.view_h = VIEW;
    s
}

#[test]
fn a_grid_lays_out_headers_and_rows_of_n() {
    let l = vlist::layout(&sections(), &BTreeSet::new(), &grid());
    // pistols: header, 5, 4; smgs: header, 5, 2; rifles: header, 5, 1.
    assert_eq!(l.lines.len(), 9);
    assert_eq!(l.lines[2], VLine::Row { section: 0, first: 5, count: 4 });
    assert_eq!(l.lines[3], VLine::Header(1));
    let want = 3.0 * 1.8 + 6.0 * 5.6 + 8.0 * 0.4;
    assert!((l.total - want).abs() < 1e-4, "{} vs {want}", l.total);
    // Folded: the header stays, its rows go.
    let folded: BTreeSet<String> = ["smgs".to_string()].into();
    let f = vlist::layout(&sections(), &folded, &grid());
    assert_eq!(f.lines.len(), 7);
    assert_eq!(f.lines[3], VLine::Header(1));
    assert_eq!(f.lines[4], VLine::Header(2));
}

#[test]
fn visible_lines_are_those_touching_the_window() {
    let l = vlist::layout(&[VSection::plain("all", 2000)], &BTreeSet::new(), &VShape::list(2.0));
    assert_eq!(l.visible(0.0, VIEW), 0..10);
    assert_eq!(l.visible(1.0, 1.0 + VIEW), 0..11, "a part line at each end");
    assert_eq!(l.visible(3990.0, 4010.0), 1995..2000);
}

#[test]
fn keys_move_through_tiles_and_stop_on_headers() {
    let mut s = state(sections(), grid());
    assert!(s.nav(VNav::Down), "the first key puts the cursor on the first line");
    assert_eq!(s.cursor, Some(VCursor::Header(0)));
    s.nav(VNav::Down);
    assert_eq!(s.cursor, Some(VCursor::Item(0, 0)));
    s.nav(VNav::Right);
    s.nav(VNav::Right);
    assert_eq!(s.cursor, Some(VCursor::Item(0, 2)));
    s.nav(VNav::Down);
    assert_eq!(s.cursor, Some(VCursor::Item(0, 7)), "the same column a row down");
    s.nav(VNav::Down);
    assert_eq!(s.cursor, Some(VCursor::Header(1)), "a header is a stop");
    s.nav(VNav::Down);
    assert_eq!(s.cursor, Some(VCursor::Item(1, 0)), "a header's column is the first");
    // ← → run in reading order across rows and sections.
    s.select(VCursor::Item(0, 8));
    s.nav(VNav::Right);
    assert_eq!(s.cursor, Some(VCursor::Item(1, 0)));
    s.nav(VNav::Left);
    assert_eq!(s.cursor, Some(VCursor::Item(0, 8)));
    // A short last row: ↓ lands on its last tile.
    s.select(VCursor::Item(1, 4));
    s.nav(VNav::Down);
    assert_eq!(s.cursor, Some(VCursor::Item(1, 6)));
    s.nav(VNav::End);
    assert_eq!(s.cursor, Some(VCursor::Item(2, 5)));
    s.nav(VNav::Home);
    assert_eq!(s.cursor, Some(VCursor::Header(0)));
}

#[test]
fn a_header_folds_on_left_and_opens_on_right() {
    let mut s = state(sections(), grid());
    s.select(VCursor::Header(1));
    let lines = s.layout.lines.len();
    assert!(s.nav(VNav::Left), "← on a header folds it");
    assert!(s.is_folded(1));
    assert_eq!(s.layout.lines.len(), lines - 2);
    assert!(!s.nav(VNav::Left), "folded already");
    assert!(s.nav(VNav::Right), "→ opens it");
    assert!(!s.is_folded(1));
    assert!(s.nav(VNav::Right), "→ again steps in");
    assert_eq!(s.cursor, Some(VCursor::Item(1, 0)));
    // Folding the cursor's section puts it on the header.
    s.fold(1, true);
    assert_eq!(s.cursor, Some(VCursor::Header(1)));
    s.toggle_fold(1);
    assert!(!s.is_folded(1));
}

#[test]
fn a_filtered_out_cursor_falls_back_to_its_header() {
    let mut s = state(sections(), grid());
    s.select(VCursor::Item(2, 5));
    s.set_sections(vec![VSection::new("pistols", 9), VSection::new("smgs", 7), VSection::new("rifles", 2)]);
    assert_eq!(s.cursor, Some(VCursor::Header(2)));
}

// ---------------------------------------------------------------------
// A built list.

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::text::TextPlugin)).add_plugins(ChimeraUiPlugin);
    app.insert_resource(UiUnits::new(1080.0, 1.0));
    app.insert_resource(Theme::from_toml(SIGNAL).unwrap());
    app.insert_resource(UiClock::manual());
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<AccumulatedMouseScroll>();
    // On this thread: the count below sees every system.
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

fn node(h_cq: f32) -> (ComputedNode, UiGlobalTransform) {
    let size = Vec2::new(600.0, h_cq * PX);
    (ComputedNode { size, inverse_scale_factor: 1.0, ..default() }, UiGlobalTransform::from(bevy::math::Affine2::from_translation(size * 0.5)))
}

/// A row's label, bound in place.
#[derive(Component)]
struct Label(Entity);

/// How many binds the screen did (a slot's line or generation changed).
#[derive(Resource, Default)]
struct Binds(usize);

fn bind(mut slots: Query<(&VSlot, &Label), Changed<VSlot>>, mut texts: Query<&mut Text>, mut n: ResMut<Binds>) {
    for (s, l) in &mut slots {
        if let (Some(line), Ok(mut t)) = (s.line, texts.get_mut(l.0)) {
            t.0.clear();
            use std::fmt::Write;
            let _ = write!(t.0, "Item {line}");
            n.0 += 1;
        }
    }
}

fn list(app: &mut App, items: usize) -> VListParts {
    let theme = app.world().resource::<Theme>().clone();
    let units = *app.world().resource::<UiUnits>();
    app.world_mut().resource_mut::<VirtualLists>().state("guns").set_shape(VShape::list(2.0));
    app.world_mut().resource_mut::<VirtualLists>().state("guns").set_sections(vec![VSection::plain("all", items)]);
    app.init_resource::<Binds>().add_systems(Update, bind);
    let world = app.world_mut();
    let root = world.spawn(Node::default()).id();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut c = Commands::new(&mut queue, world);
    let ui = Ui::new(&theme, None, units);
    let p = ui.virtual_list(&mut c, root, "guns", UiStyle::row().h(VIEW), VIEW, VShape::list(2.0));
    // Each slot's template, made once.
    for &s in &p.slots {
        let t = ui.text_plain(&mut c, s, chimera_ui::Role::Mono, 0.6, chimera_ui::Tone::Fg, "");
        c.entity(t).insert(chimera_ui::controls::clickable());
        c.entity(t).insert(VHit { slot: s, col: Some(0) });
        c.entity(s).insert(Label(t));
    }
    queue.apply(world);
    world.entity_mut(p.scroll.view).insert(node(VIEW));
    world.entity_mut(p.scroll.content).insert(node(items as f32 * 2.0));
    world.entity_mut(p.scroll.track).insert(node(VIEW));
    let rel = RelativeCursorPosition { cursor_over: true, normalized: Some(Vec2::ZERO) };
    world.entity_mut(p.scroll.view).insert(rel);
    frame(app);
    frame(app);
    p
}

fn frame(app: &mut App) {
    app.world_mut().resource_mut::<UiClock>().step(DT);
    app.update();
    let w = app.world_mut();
    w.resource_mut::<ButtonInput<MouseButton>>().clear();
    *w.resource_mut::<AccumulatedMouseScroll>() = AccumulatedMouseScroll::default();
}

fn wheel(app: &mut App, notches: f32) {
    let mut w = AccumulatedMouseScroll::default();
    w.unit = MouseScrollUnit::Line;
    w.delta = Vec2::new(0.0, -notches);
    app.insert_resource(w);
}

fn entity_count(app: &mut App) -> usize {
    app.world_mut().query::<Entity>().iter(app.world()).count()
}

#[test]
fn scrolling_rebinds_the_pool_in_place() {
    let mut app = app();
    let p = list(&mut app, 2000);
    assert_eq!(p.slots.len(), vlist::pool_size(VIEW, &VShape::list(2.0)), "the lines a view holds, plus two");
    let content_h = app.world().get::<Node>(p.scroll.content).unwrap().height;
    assert_eq!(content_h, Val::Px(4000.0 * PX), "the content as tall as 2,000 rows");
    let first_binds = app.world().resource::<Binds>().0;
    assert_eq!(first_binds, 10, "the rows in view bound once");
    let before = entity_count(&mut app);
    wheel(&mut app, 3.0);
    for _ in 0..60 {
        frame(&mut app);
    }
    assert_eq!(entity_count(&mut app), before, "no entity spawned or despawned while scrolling");
    let off = app.world().resource::<ScrollViews>().get("guns").unwrap().offset;
    assert!(off > 9.0, "scrolled by the pixel: {off}");
    // Lines that scrolled in were bound once each; the rest kept theirs.
    let rows_passed = (off / 2.0).floor() as usize;
    let binds = app.world().resource::<Binds>().0 - first_binds;
    assert!(binds <= rows_passed + 2, "{binds} binds for {rows_passed} rows passed");
    // Every shown slot sits at its line's top.
    for &s in &p.slots {
        let (slot, n) = (app.world().get::<VSlot>(s).unwrap(), app.world().get::<Node>(s).unwrap());
        if let Some(l) = slot.line {
            assert_eq!(n.top, Val::Px((l as f32 * 2.0 * PX).round()), "snapped to the pixel");
            assert_eq!(n.display, Display::Flex);
        }
    }
}

#[test]
fn keys_reveal_the_cursor_far_down() {
    let mut app = app();
    list(&mut app, 2000);
    app.world_mut().resource_mut::<VirtualLists>().state("guns").select(VCursor::Item(0, 500));
    for _ in 0..90 {
        frame(&mut app);
    }
    let off = app.world().resource::<ScrollViews>().get("guns").unwrap().offset;
    assert!(off <= 1000.0 && off + VIEW >= 1002.0, "row 500 in view: offset {off}");
}

#[test]
fn a_press_selects_and_a_second_press_activates() {
    let mut app = app();
    let p = list(&mut app, 50);
    let hit = |app: &mut App, slot: Entity| {
        let label = app.world().get::<Label>(slot).unwrap().0;
        app.world_mut().entity_mut(label).insert(Interaction::Pressed);
        frame(app);
        app.world_mut().entity_mut(label).insert(Interaction::None);
        let events: Vec<VListEvent> = app.world_mut().resource_mut::<Messages<VListEvent>>().drain().collect();
        events
    };
    let slot = p.slots[3];
    let e = hit(&mut app, slot);
    assert_eq!(e, vec![VListEvent::Select { key: "guns".into(), at: VCursor::Item(0, 3) }]);
    assert_eq!(app.world().resource::<VirtualLists>().get("guns").unwrap().cursor, Some(VCursor::Item(0, 3)));
    let e = hit(&mut app, slot);
    assert_eq!(e, vec![VListEvent::Activate { key: "guns".into(), at: VCursor::Item(0, 3) }], "a double press");
    // Too slow: two selects.
    hit(&mut app, slot);
    for _ in 0..((DOUBLE_CLICK_SECS / DT as f64) as usize + 2) {
        frame(&mut app);
    }
    let e = hit(&mut app, slot);
    assert!(matches!(e[..], [VListEvent::Select { .. }]), "{e:?}");
}

// ---------------------------------------------------------------------
// The count: the recycling list against today's rebuild of the window.

#[derive(Component)]
struct Naive;

/// Today's way (the Maps dialog, the server list): when the window of
/// rows changes, despawn them and spawn the rows now in view.
fn naive_rebuild(
    views: Res<ScrollViews>,
    theme: Res<Theme>,
    units: Res<UiUnits>,
    content: Query<Entity, With<Naive>>,
    mut last: Local<Option<usize>>,
    mut c: Commands,
) {
    let Some(s) = views.get("guns") else { return };
    let first = (s.inside() / 2.0).floor() as usize;
    if *last == Some(first) {
        return;
    }
    *last = Some(first);
    let Ok(root) = content.single() else { return };
    c.entity(root).despawn_children();
    let ui = Ui::new(&theme, None, *units);
    for i in first..first + 11 {
        let r = ui.boxed(&mut c, root, UiStyle::row().h(2.0));
        ui.text_plain(&mut c, r, chimera_ui::Role::Mono, 0.6, chimera_ui::Tone::Fg, &format!("Item {i}"));
    }
}

fn scroll_count(app: &mut App) -> Option<u64> {
    let mut counter = chimera_core::metrics::InstructionCounter::current();
    counter.read()?;
    wheel(app, 40.0);
    let a = counter.read()?;
    for _ in 0..120 {
        frame(app);
    }
    let b = counter.read()?;
    Some(b - a)
}

/// A plain scroll view over 2,000 rows' height, its rows rebuilt by
/// `naive_rebuild` (`rows`), or none (the cost of the scroll itself).
fn plain(rows: bool) -> App {
    let mut naive = app();
    {
        let theme = naive.world().resource::<Theme>().clone();
        let units = *naive.world().resource::<UiUnits>();
        if rows {
            naive.add_systems(Update, naive_rebuild);
        }
        let world = naive.world_mut();
        let root = world.spawn(Node::default()).id();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut c = Commands::new(&mut queue, world);
        let p = Ui::new(&theme, None, units).scroll(&mut c, root, "guns", UiStyle::row().h(VIEW), UiStyle::column());
        c.entity(p.content).insert(Naive);
        queue.apply(world);
        world.entity_mut(p.view).insert(node(VIEW));
        world.entity_mut(p.content).insert(node(4000.0));
        world.entity_mut(p.track).insert(node(VIEW));
        world.entity_mut(p.view).insert(RelativeCursorPosition { cursor_over: true, normalized: Some(Vec2::ZERO) });
        frame(&mut naive);
        frame(&mut naive);
    }
    naive
}

#[test]
fn scrolling_2000_items_costs_less_than_a_rebuild() {
    let mut virt = app();
    list(&mut virt, 2000);
    let mut naive = plain(true);
    let mut base = plain(false);
    let (Some(v), Some(n), Some(b)) = (scroll_count(&mut virt), scroll_count(&mut naive), scroll_count(&mut base)) else {
        eprintln!("no hardware instruction counter here: the count is skipped");
        return;
    };
    let off = |a: &App| a.world().resource::<ScrollViews>().get("guns").unwrap().offset;
    assert!((off(&virt) - off(&naive)).abs() < 0.01, "both scrolled alike");
    // The rows' own share: less the scroll view's frames with no rows.
    let (v, n) = (v.saturating_sub(b), n.saturating_sub(b));
    eprintln!(
        "120 frames scrolling {:.0} cq of 2,000 rows, beyond the bare view's {b}: recycling {v} instructions, rebuild {n} ({:.1}x)",
        off(&virt),
        n as f64 / v.max(1) as f64
    );
    assert!(v * 2 < n, "the recycling list costs under half the rebuild: {v} vs {n}");
}
