//! The Escape menu in the Signal look (feature `present`; a theme with
//! `[layout.esc]`, the Ultimate Gun Game's; docs/plans/active/ugg-ui.md
//! M7, the components page's frame 05): drawn by `esc_menu::show` in place
//! of the party's panel, same state (`EscMenu`), same actions.
//!
//! The match goes on, and the screen says so: a left panel over a
//! gradient shade (ink 92 % at the panel to clear by 70 % of the width),
//! so the race chip and the kill feed stay readable. The lockup, "Paused
//! for you only: the match goes on", the entries (data/main_menu.toml
//! `[[escape]]`: an index, the label, the HOST chip on Host settings, Esc
//! on Resume, a triangle on the others); the keyboard's selection
//! (`EscMenu::focus`, ↑↓ and Enter) a white slab with a slanted right
//! edge and the accent bar, wiping to its new row along the slant
//! (`layout.esc.wipe`); hover the faint tint and the right-edge tick. A
//! running vote dims its entry and shows the question, the counts, a pip
//! per vote needed, the seconds left and Vote yes (F1) / Vote no (F2).
//! The host's Disconnect asks first in a modal with the threat bar ("End
//! the game for N players?"), Stay first. The keys at the foot and "Match
//! live" with the threat mark after them (the look book's bottom right is
//! where a game HUD draws its ammo). Options and Host settings are
//! the Settings screen's (`settings`).

use bevy::prelude::*;
use chimera_ui::{
    IconImages, Role, Tone, UiSlant, UiStyle,
    controls::{Kind, State, Tick, Ui, Wipe},
    widgets::{self, TagKind},
    Prop, Timeline, Track,
};

use crate::esc_menu::{Action, MenuRoot, Page, ShownEntry};

/// The layout table.
const L: &str = "esc";

/// A running vote as the panel shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct VoteShown {
    pub title: String,
    pub yes: u32,
    pub no: u32,
    pub needed: u32,
    pub left: u64,
    pub mine: Option<u8>,
    pub may_vote: bool,
}

/// What to draw.
pub struct Drawn<'a> {
    pub page: Page,
    pub focus: usize,
    pub focus_since: f64,
    pub now: f64,
    pub entries: &'a [ShownEntry],
    pub notice: Option<&'a str>,
    pub vote: Option<&'a VoteShown>,
}

/// The menu's root and everything in it.
pub fn spawn(c: &mut Commands, ui: &Ui, images: &mut Assets<Image>, icons: &mut IconImages, d: &Drawn, z: i32) -> Entity {
    let t = ui.theme;
    let lay = |k: &str| t.layout(L, k);
    // The look book's 92 % at the panel to clear by 70 %, in the theme's
    // ink curve (`tone_a`), with the fall spread over more stops so it
    // never reads as a wall.
    let ink = |a: f32| t.tone_a(Tone::Ink, a);
    let root = c
        .spawn((
            MenuRoot,
            GlobalZIndex(z),
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
            BackgroundGradient::from(LinearGradient::to_right(vec![
                ColorStop::percent(ink(0.92), 0.0),
                ColorStop::percent(ink(0.85), 22.0),
                ColorStop::percent(ink(0.6), 36.0),
                ColorStop::percent(ink(0.3), 50.0),
                ColorStop::percent(ink(0.0), 68.0),
            ])),
            bevy::ui::FocusPolicy::Block,
        ))
        .id();
    if matches!(d.page, Page::Options(_) | Page::Host) {
        // The Settings screen draws these pages.
        return root;
    }
    let panel = ui.boxed(
        c,
        root,
        UiStyle { w: Some(lay("panel_w")), ..UiStyle::column() }.abs([Some(lay("panel_x")), Some(lay("panel_y")), None, None]),
    );
    ui.lockup(c, panel, images, icons, lay("lockup"));
    let ps = ui.boxed(c, panel, UiStyle { gap: Some(0.7), margin: Some([2.4, 0.0, 1.4, 0.0].into()), ..UiStyle::row() });
    ui.mark(c, ps, Tone::Acc, 0.5, 0.5);
    ui.text(c, ps, Role::Mono, 0.58, Tone::Fg2, "Paused for you only: the match goes on");
    let list = ui.boxed(c, panel, UiStyle::column());
    for (i, e) in d.entries.iter().enumerate() {
        entry(c, ui, list, i, e, d);
    }
    if let Some(v) = d.vote {
        vote_box(c, ui, panel, v);
    }
    if let Some(n) = d.notice {
        let line = ui.boxed(c, panel, UiStyle { gap: Some(0.6), margin: Some([1.0, 0.0, 0.0, 1.1].into()), ..UiStyle::row() });
        ui.mark(c, line, Tone::Acc, 0.42, 0.7);
        ui.text(c, line, Role::Mono, 0.56, Tone::AccText, n);
    }
    // The keys and the live line at the foot.
    let foot = ui.boxed(c, root, UiStyle { gap: Some(1.6), ..UiStyle::row() }.abs([Some(lay("panel_x")), None, None, Some(lay("foot_y"))]));
    for (k, what) in [("↑↓", "Select"), ("Enter", "Confirm"), ("Esc", "Resume")] {
        let g = ui.boxed(c, foot, UiStyle { gap: Some(0.45), ..UiStyle::row() });
        ui.kbd(c, g, k, 0.56, Tone::Fg);
        ui.text(c, g, Role::Mono, 0.56, Tone::Fg2, what);
    }
    // In the keys' line: the bottom right is a game HUD's ammo.
    let live = ui.boxed(c, foot, UiStyle { gap: Some(0.6), margin: Some([0.0, 0.0, 0.0, 1.4].into()), ..UiStyle::row() });
    ui.mark(c, live, Tone::Threat, 0.6, 0.6);
    ui.text(c, live, Role::Mono, 0.52, Tone::Fg2, "Match live • you can still be hit");
    if let Page::ConfirmDisconnect(n) = d.page {
        confirm(c, ui, root, n, d.focus);
    }
    root
}

fn entry(c: &mut Commands, ui: &Ui, list: Entity, i: usize, e: &ShownEntry, d: &Drawn) {
    let t = ui.theme;
    let h = t.layout(L, "item_h");
    let sel = d.page == Page::Main && i == d.focus && e.action.is_some();
    let off = e.action.is_none();
    let base = Color::NONE;
    let over = if off { base } else { ui.color(Tone::Fg, 0.06) };
    let mut b = c.spawn((
        UiStyle { h: Some(h), gap: Some(1.1), pad: Some([0.0, 1.5, 0.0, 1.1].into()), ..UiStyle::row() },
        Node::default(),
        BackgroundColor(base),
        chimera_ui::controls::Hover { base, over },
        chimera_ui::controls::clickable(),
        ChildOf(list),
    ));
    if let Some(a) = e.action {
        b.insert(a);
    }
    let b = b.id();
    if i > 0 {
        c.spawn((
            Node { position_type: PositionType::Absolute, left: ui.units.val(1.1), right: Val::ZERO, top: Val::ZERO, height: Val::Px(ui.units.hairline()), ..default() },
            BackgroundColor(ui.color(Tone::Fg, 0.07)),
            Pickable::IGNORE,
            ChildOf(b),
        ));
    }
    if sel {
        // The white slab, wiping in along the slant from where it moved.
        let wipe = t.layout(L, "wipe");
        let mut slab = c.spawn((
            Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, ..default() },
            UiSlant { accent: Some((Tone::Acc, t.layout(L, "bar_accent"))), ..UiSlant::new(Tone::Fg, 1.0).edges(false, true) },
            Pickable::IGNORE,
            ChildOf(b),
        ));
        if ((d.now - d.focus_since) as f32) < wipe {
            slab.insert((
                Timeline::new([Track::new(Prop::ClipRight, 1.0, 0.0).dur(wipe).ease(chimera_ui::Ease::OUT)]).started(d.focus_since),
                Wipe { despawn_done: false },
            ));
        }
    }
    let ink = if sel { Tone::Ink } else if off { Tone::Fg3 } else { Tone::Fg };
    let small = if sel { Tone::Ink } else { Tone::Fg3 };
    let ix = ui.boxed(c, b, UiStyle { w: Some(1.3), shrink: Some(0.0), ..UiStyle::row() });
    ui.text(c, ix, Role::Mono, 0.58, small, &format!("{:02}", i + 1));
    ui.text(c, b, Role::DisplayBold, t.layout(L, "label"), ink, &e.label);
    if e.host {
        let (node, label) = widgets::tag(t, ui.kit, &ui.units, TagKind::Host, "Host", 0.5);
        let tag = c.spawn((node, Pickable::IGNORE, ChildOf(b))).id();
        c.spawn((label, ChildOf(tag)));
    }
    ui.boxed(c, b, UiStyle::row().grow(1.0));
    match e.key {
        Some(k) => ui.text(c, b, Role::Mono, 0.52, small, k),
        None => ui.text(c, b, Role::Mono, 0.8, small, "▸"),
    };
    if !off {
        // Hover's tick (the slab shows the selection).
        c.spawn((
            Tick { always: false },
            Node { position_type: PositionType::Absolute, right: ui.units.val(0.45), top: percent(30.0), bottom: percent(30.0), width: ui.units.val(0.42), ..default() },
            UiSlant::new(if sel { Tone::Ink } else { Tone::Fg }, 1.0),
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(b),
        ));
    }
}

fn vote_box(c: &mut Commands, ui: &Ui, panel: Entity, v: &VoteShown) {
    let b = ui.boxed(
        c,
        panel,
        UiStyle { gap: Some(0.6), pad: Some([1.0, 1.1].into()), margin: Some([1.0, 0.0, 0.0, 1.1].into()), ..UiStyle::column() }.bg_a(Tone::Fg, 0.04),
    );
    ui.focus_bar(c, b, 0.22);
    let tagrow = ui.boxed(c, b, UiStyle::row());
    let (node, label) = widgets::tag(ui.theme, ui.kit, &ui.units, TagKind::Accent, "Vote", 0.5);
    let tag = c.spawn((node, ChildOf(tagrow))).id();
    c.spawn((label, ChildOf(tag)));
    ui.text(c, b, Role::DisplayBold, 0.8, Tone::Fg, &v.title);
    let counts = ui.boxed(c, b, UiStyle { gap: Some(1.0), ..UiStyle::row() });
    ui.text(c, counts, Role::Mono, 0.56, Tone::Fg2, &format!("Yes {} • No {}", v.yes, v.no));
    let pips = ui.boxed(c, counts, UiStyle { gap: Some(0.16), ..UiStyle::row() });
    for k in 0..v.needed.max(1) {
        ui.mark(c, pips, if k < v.yes { Tone::Fg } else { Tone::Fg4 }, 0.5, 0.42);
    }
    ui.text(c, counts, Role::Mono, 0.56, Tone::Fg2, &format!("{} needed • {} s", v.needed, v.left));
    let acts = ui.boxed(c, b, UiStyle { gap: Some(0.4), margin: Some([0.2, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
    match v.mine {
        None if v.may_vote => {
            let y = ui.button(c, acts, Kind::Secondary, "Vote yes", Some("F1"), 1.9, State::NONE);
            c.entity(y).insert(Action::VoteYes);
            let n = ui.button(c, acts, Kind::Secondary, "Vote no", Some("F2"), 1.9, State::NONE);
            c.entity(n).insert(Action::VoteNo);
        }
        None => {
            ui.text(c, acts, Role::Mono, 0.56, Tone::Fg3, "You can't vote on this one");
        }
        Some(m) => {
            ui.text(c, acts, Role::Mono, 0.56, Tone::Fg2, if m == 1 { "You voted yes" } else { "You voted no" });
        }
    }
}

/// The host's leave: a modal with the threat bar, Stay first.
fn confirm(c: &mut Commands, ui: &Ui, root: Entity, people: usize, focus: usize) {
    let shade = c
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(ui.color(Tone::Ink, 0.72)),
            bevy::ui::FocusPolicy::Block,
            ChildOf(root),
        ))
        .id();
    let m = ui.modal(c, shade, 32.0, true);
    let ttl = ui.boxed(c, m, UiStyle { gap: Some(0.6), margin: Some([0.7, 0.0, 0.6, 0.0].into()), ..UiStyle::row() });
    ui.mark(c, ttl, Tone::Threat, 0.8, 0.9);
    let who = if people == 1 { "1 player".to_string() } else { format!("{people} players") };
    ui.text(c, ttl, Role::DisplayBold, 1.35, Tone::Fg, &format!("End the game for {who}?"));
    ui.paragraph(c, m, Role::Mono, 0.62, Tone::Fg2, "You are the host: leaving closes the server.");
    let acts = ui.boxed(c, m, UiStyle { gap: Some(0.5), margin: Some([1.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() }.justify(JustifyContent::FlexEnd));
    let stay = ui.button(c, acts, Kind::Primary, "Stay", Some("Esc"), 2.4, State::focused(focus == 0));
    c.entity(stay).insert(Action::Back);
    let end = ui.button(c, acts, Kind::Danger, "End the game", None, 2.4, State::focused(focus == 1));
    c.entity(end).insert(Action::DisconnectYes);
}
