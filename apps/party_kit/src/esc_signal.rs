//! The Escape menu in the Signal look (feature `present`; a theme with
//! `[layout.esc]`, the Ultimate Gun Game's; docs/plans/active/ugg-ui.md
//! M7, the components page's frame 05): drawn by `esc_menu::show` in place
//! of the party's panel, same state (`EscMenu`), same actions.
//!
//! The match goes on, and the screen says so: a left panel over a
//! shade (ink 92 % at the panel easing to clear by 68 % of the width,
//! dithered: `chimera_ui::ShadeMaterial`), so the race chip and the kill
//! feed stay readable; the games' own HUDs are hidden under it
//! (`esc_menu::cover_huds`, `client::hud::HudCover`), Settings' pages
//! included. The lockup, "Paused
//! for you only: the match goes on", the entries (data/main_menu.toml
//! `[[escape]]`: an index, the label, the HOST chip on Host settings, Esc
//! on Resume, a triangle on the others); the keyboard's selection
//! (`EscMenu::focus`: ↑↓ and Enter, or the pointer while it leads,
//! `chimera_ui::PointerLead`) a white slab with a slanted right
//! edge and the accent bar, wiping to its new row along the slant
//! (`layout.esc.wipe`); hover the faint tint (one highlight at a time: no tick over the slab). A
//! running vote dims its entry and shows the question, the counts, a pip
//! per vote needed, the seconds left and Vote yes (F1) / Vote no (F2).
//! The host's Disconnect asks first in a modal with the threat bar ("End
//! the game for N players?"), Stay first. The keys at the foot and "Match
//! live" with the threat mark after them. Options and Host settings are
//! the Settings screen's (`settings`).
//!
//! Over the themed pre-match lobby (`Drawn::lobby`) there is no panel:
//! only Leave's confirm ("Leave the lobby?", Stay first) on a flat shade
//! as deep as Settings' (`settings::LOBBY_SHADE`), so the lobby reads dimmed under
//! it and its header doesn't ghost through; Settings brings its own.

use bevy::prelude::*;
use chimera_ui::{
    IconImages, Role, ShadeMaterial, Tone, UiSlant, UiStyle,
    controls::{Kind, NOTE, SENTENCE, State, Ui, Wipe},
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
    /// Over the themed pre-match lobby: its overlays only (no panel).
    pub lobby: bool,
    pub page: Page,
    pub focus: usize,
    pub focus_since: f64,
    pub now: f64,
    pub entries: &'a [ShownEntry],
    pub notice: Option<&'a str>,
    pub vote: Option<&'a VoteShown>,
}

/// The menu's root and everything in it; the shade drawn by
/// `ShadeMaterial` when its assets are there (else a gradient node).
pub fn spawn(
    c: &mut Commands,
    ui: &Ui,
    images: &mut Assets<Image>,
    icons: &mut IconImages,
    shades: Option<&mut Assets<ShadeMaterial>>,
    d: &Drawn,
    z: i32,
) -> Entity {
    let t = ui.theme;
    let lay = |k: &str| t.layout(L, k);
    let root = c
        .spawn((
            MenuRoot,
            GlobalZIndex(z),
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
            bevy::ui::FocusPolicy::Block,
        ))
        .id();
    // The look book's 92 % at the panel to clear by 70 %, in the theme's
    // ink curve, eased (no corners where it would read as a wall) and
    // dithered per pixel: a gradient node's stops are 8-bit, and the
    // near-opaque ink banded in wide stripes.
    if d.lobby {
        // Over the lobby: Settings draws its own page and shade; Leave's
        // confirm brings a flat one.
        if d.page == Page::ConfirmLeave {
            confirm(c, ui, root, crate::settings::LOBBY_SHADE, "Leave the lobby?", "You go back to the main menu; the match starts without you.", "Leave", d.focus);
        }
        return root;
    }
    match shades {
        Some(shades) => {
            c.entity(root).insert(MaterialNode(shades.add(ShadeMaterial::new(t, Tone::Ink, 0.92, 0.12, 0.68))));
        }
        None => {
            let ink = |a: f32| t.tone_a(Tone::Ink, a);
            c.entity(root).insert(BackgroundGradient::from(LinearGradient::to_right(vec![
                ColorStop::percent(ink(0.92), 0.0),
                ColorStop::percent(ink(0.85), 22.0),
                ColorStop::percent(ink(0.6), 36.0),
                ColorStop::percent(ink(0.3), 50.0),
                ColorStop::percent(ink(0.0), 68.0),
            ])));
        }
    }
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
    ui.sentence(c, ps, NOTE, Tone::Fg2, "Paused for you only: the match goes on");
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
    // In the keys' line.
    let live = ui.boxed(c, foot, UiStyle { gap: Some(0.6), margin: Some([0.0, 0.0, 0.0, 1.4].into()), ..UiStyle::row() });
    ui.mark(c, live, Tone::Threat, 0.6, 0.6);
    ui.sentence(c, live, NOTE, Tone::Fg2, "Match live • you can still be hit");
    if let Page::ConfirmDisconnect(n) = d.page {
        let who = if n == 1 { "1 player".to_string() } else { format!("{n} players") };
        confirm(c, ui, root, 0.72, &format!("End the game for {who}?"), "You are the host: leaving closes the server.", "End the game", d.focus);
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
            ui.sentence(c, acts, NOTE, Tone::Fg3, "You can't vote on this one");
        }
        Some(m) => {
            ui.sentence(c, acts, NOTE, Tone::Fg2, if m == 1 { "You voted yes" } else { "You voted no" });
        }
    }
}

/// A leave (the host's, the lobby's): a modal with the threat bar over a
/// flat shade (ink at `shade`), Stay first.
#[allow(clippy::too_many_arguments)]
fn confirm(c: &mut Commands, ui: &Ui, root: Entity, shade: f32, title: &str, line: &str, go: &str, focus: usize) {
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
            BackgroundColor(ui.color(Tone::Ink, shade)),
            bevy::ui::FocusPolicy::Block,
            ChildOf(root),
        ))
        .id();
    let m = ui.modal(c, shade, 32.0, true);
    let ttl = ui.boxed(c, m, UiStyle { gap: Some(0.6), margin: Some([0.7, 0.0, 0.6, 0.0].into()), ..UiStyle::row() });
    ui.mark(c, ttl, Tone::Threat, 0.8, 0.9);
    ui.text(c, ttl, Role::DisplayBold, 1.35, Tone::Fg, title);
    ui.paragraph(c, m, Role::Text, SENTENCE, Tone::Fg2, line);
    let acts = ui.boxed(c, m, UiStyle { gap: Some(0.5), margin: Some([1.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() }.justify(JustifyContent::FlexEnd));
    let stay = ui.button(c, acts, Kind::Primary, "Stay", Some("Esc"), chimera_ui::controls::HERO_H, State::focused(focus == 0));
    c.entity(stay).insert(Action::Back);
    let end = ui.button(c, acts, Kind::Danger, go, None, 2.4, State::focused(focus == 1));
    c.entity(end).insert(Action::DisconnectYes);
}
