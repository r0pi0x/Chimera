//! The pre-match lobby drawn on the UI kit (`chimera_ui`), for an app
//! whose theme lays the lobby out (`[layout.lobby]`: the Ultimate Gun
//! Game's Signal, docs/plans/active/ugg-ui.md 3 and M1); without one the
//! shell's own panel (`prematch_lobby`) draws as before. Same state
//! (`prematch_lobby::LobbyView`), same root (`LobbyRoot`), same key (F3):
//! presentation only. Built from the kit's style layer (`style!`,
//! `UiText`, `UiSlant`) and its Look Book V marks (`widgets`).
//!
//! On a full-screen ink plate over the frozen map, a 16:9 frame (100 x
//! 56.25 cq, centred; wider windows get wider margins) with the theme's
//! numbers:
//!
//! - **Header**: the lockup (the kit's `mark` icon, ULTIMATE spread over
//!   the wordmark's width, the wordmark with `ss02`), "Lobby", and the
//!   tabs Invite, Match settings, Leave (shown, not clickable: Leave is
//!   Esc); a rule under it; `+` marks in the corners.
//! - **Left**: the Ready tag and count ("5 / 7", "players • 8 slots"
//!   when the slot count is known), the player table (`#`, player, tag,
//!   status): the slot number, the bot slot left of every name (the robot
//!   for a bot, `LobbyMark::Bot`), the name; HOST as a solid tag
//!   (`LobbyMark::Host`), YOU and HOST as the fused chip
//!   (`LobbyMark::YouHost`); the session wins after the name (trophy
//!   and count, only with any, the leader's in the accent: `LobbyWins`,
//!   from `LobbyRow::wins`); the status: the ping meter and the number
//!   (by the theme's `[network]` thresholds, "LAG" when bad, a dash for a
//!   bot), a slanted block (`ReadyBlock`: filled when ready, an outline
//!   otherwise; it pops, scale y 1.9 to 1 over
//!   `motion.dur.lobby.ready_pop`, when it turns ready) and its words;
//!   your row tinted and outlined (`widgets::row_you`,
//!   `LobbyPlayerRow::you`); free slots reading "Open" (`OpenSlot`).
//! - **Right**: the map's name big and its game, a map preview (a
//!   gradient tinted by the map's name stands in: no thumbnails yet),
//!   Mode and Guns from (the ladder's games), and "Starts in" with the
//!   countdown big and the waiting line beside it, over a tick bar
//!   (`CountdownBar`: pixel-snapped ticks lit as the countdown runs,
//!   stepped every frame from `UiClock`, the last
//!   `layout.lobby.countdown_hot` seconds in the accent).
//! - **Footer**: the keys: F3 and what it does now (`LobbyView::key_line`),
//!   Tab Players, Esc Leave.
//!
//! Rebuilt when the view, the window's units or the theme change; the
//! bar and the pops animate without a rebuild.
//!
//! Known shortcuts: the map preview is a gradient; the tabs are drawn
//! but not clickable; a downloader's bytes show as a percentage only;
//! no packet-loss mark (the lobby has no loss figure).

use std::collections::HashMap;

use bevy::prelude::*;
use chimera_net::prematch::status;
use chimera_ui::{
    IconImages, KitFonts,
    icons::{self, Icon}, Prop, Role, SlantMaterial, Theme, TickBarMaterial, TickLayout, Timeline, Tone, Track, UiClock,
    UiStyle, UiText, UiUnits, style,
    widgets::{self, PingLevel, TagKind},
};

use super::prematch_lobby::{LobbyContent, LobbyRoot, LobbyView};

/// The theme's lobby table.
const L: &str = "lobby";

/// Whether the theme draws the lobby here (`[layout.lobby]`).
pub fn themed(theme: Option<Res<Theme>>) -> bool {
    theme.is_some_and(|t| t.has_layout(L))
}

/// A player's mark, as drawn (tests find them).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyMark {
    /// A solid accent tag.
    Host,
    /// One chip, YOU then HOST, split on the slant.
    YouHost,
    /// The robot in the slot left of the name.
    Bot,
}

/// A player's row (`index` into `LobbyView::rows`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LobbyPlayerRow {
    pub index: usize,
    /// This machine's player: tinted and outlined.
    pub you: bool,
}

/// A player's session wins beside the name (trophy and count, only with
/// any; the session's leader in the accent).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LobbyWins {
    pub wins: u32,
    pub leader: bool,
}

/// A row's status block: filled when ready.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadyBlock {
    pub ready: bool,
}

/// A free slot's row.
#[derive(Component, Clone, Copy, Debug)]
pub struct OpenSlot;

/// The countdown's tick bar: its material and where the countdown is.
#[derive(Component, Clone, Debug)]
pub struct CountdownBar {
    pub material: Handle<TickBarMaterial>,
    pub ticks: u32,
    /// The countdown's length and seconds left (None: not counting).
    pub total: u32,
    pub left: Option<u32>,
    /// When `left` last changed (`UiClock`).
    pub since: f64,
    /// Seconds from the end lit in the accent.
    pub hot: f32,
}

impl CountdownBar {
    /// Ticks lit and the first hot one at `now`: lit as the countdown
    /// runs (`left` counts whole seconds; the second under way runs on
    /// the clock).
    pub fn lit(&self, now: f64) -> (u32, u32) {
        let n = self.ticks as f32;
        let total = self.total.max(1) as f32;
        let hot = ((n * (total - self.hot) / total).floor().max(0.0)) as u32;
        let Some(left) = self.left else { return (0, hot) };
        let elapsed = (total - left as f32 + (now - self.since).clamp(0.0, 1.0) as f32).clamp(0.0, total);
        ((n * elapsed / total).round() as u32, hot)
    }
}

/// The lobby panel's systems (added by `PreMatchLobbyPlugin`).
pub fn add(app: &mut App) {
    app.add_systems(Update, (draw, step_bar).chain().after(super::prematch_lobby::LobbyFollow).run_if(themed));
}

#[derive(Default)]
struct Drawn {
    key: Option<(LobbyView, u32, u32, String)>,
    /// When each player's block turned ready (by name), for its pop.
    popped: HashMap<String, f64>,
    /// The countdown's length: the most seconds seen since it began.
    total: u32,
    left: Option<u32>,
    since: f64,
    /// The tabs' triangle by height in physical px.
    tri: HashMap<u32, Handle<Image>>,
}

/// The tabs' small right-pointing triangle.
fn triangle() -> Icon {
    Icon { name: "Triangle".into(), w: 46.0, h: 56.0, shapes: vec![vec![vec![[0.0, 0.0], [46.0, 28.0], [0.0, 56.0]]]] }
}

/// `map` to a pair of colours, for the map preview's stand-in.
fn map_tint(map: &str) -> (Color, Color) {
    let h = map.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    let hue = (h % 360) as f32;
    (Color::hsl(hue, 0.22, 0.30), Color::hsl(220.0, 0.30, 0.10))
}

/// What the builders share.
struct Panel<'a> {
    theme: &'a Theme,
    kit: Option<&'a KitFonts>,
    units: UiUnits,
}

impl Panel<'_> {
    fn lay(&self, k: &str) -> f32 {
        self.theme.layout(L, k)
    }

    /// A styled box under `parent`.
    fn boxed(&self, c: &mut Commands, parent: Entity, s: UiStyle) -> Entity {
        c.spawn((s, Node::default(), ChildOf(parent))).id()
    }

    /// A text in a role at `size` cq under `parent` (upper case: the UGG
    /// faces draw caps only, a fallback face as well).
    fn text(&self, c: &mut Commands, parent: Entity, role: Role, size: f32, tone: Tone, text: &str) -> Entity {
        c.spawn((
            Text::new(text.to_uppercase()),
            UiText::new(role, size, tone),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
            ChildOf(parent),
        ))
        .id()
    }

    /// A solid tag under `parent`.
    fn tag(&self, c: &mut Commands, parent: Entity, kind: TagKind, text: &str) -> Entity {
        let e = widgets::spawn_pair(c, widgets::tag(self.theme, self.kit, &self.units, kind, text, self.lay("tag")));
        c.entity(e).insert(ChildOf(parent));
        e
    }

    /// The accent label tag (Ready, Map, Starts in), at its own width.
    fn label(&self, c: &mut Commands, parent: Entity, text: &str) {
        let wrap = self.boxed(c, parent, style!(row));
        self.tag(c, wrap, TagKind::Accent, text);
    }

    /// A value big with its unit beside it.
    fn big(&self, c: &mut Commands, parent: Entity, value: &str, size: f32, unit: &str, top: f32) {
        let row = self.boxed(c, parent, style!(row, gap: 1.0, margin: [top, 0.0, 0.0, 0.0]).align(AlignItems::Baseline));
        self.text(c, row, Role::DisplayBold, size, Tone::Fg, value);
        let u = self.text(c, row, Role::Mono, self.lay("label"), Tone::Fg3, unit);
        c.entity(u).insert(TextLayout::new(Justify::Left, LineBreak::WordBoundary));
    }

    /// The kit's icon `key`, `h` cq tall, in `tone`.
    fn icon(&self, c: &mut Commands, parent: Entity, icons: &mut IconImages, images: &mut Assets<Image>, key: &str, h: f32, tone: Tone) {
        let u = self.units;
        let px = u.phys(u.cq(h)).round().max(1.0) as u32;
        let aspect = icons.icons().get(key).map_or(1.0, |i| i.aspect());
        let image = icons.get(images, key, px).unwrap_or_default();
        c.spawn((
            ImageNode { image, color: self.theme.tone(tone), ..default() },
            Node {
                width: Val::Px((aspect * px as f32).ceil() / u.scale_factor),
                height: Val::Px(px as f32 / u.scale_factor),
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(parent),
        ));
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    view: Res<LobbyView>,
    theme: Res<Theme>,
    units: Res<UiUnits>,
    clock: Res<UiClock>,
    kit: Option<Res<KitFonts>>,
    mut root: Query<(Entity, &mut Visibility, &mut Node), With<LobbyRoot>>,
    content: Query<Entity, With<LobbyContent>>,
    (mut images, mut icons, mut slants, mut ticks): (
        Option<ResMut<Assets<Image>>>,
        Option<ResMut<IconImages>>,
        Option<ResMut<Assets<SlantMaterial>>>,
        Option<ResMut<Assets<TickBarMaterial>>>,
    ),
    mut drawn: Local<Drawn>,
    mut commands: Commands,
) {
    let drawn: &mut Drawn = &mut drawn;
    let Some((root, mut visibility, mut root_node)) = root.iter_mut().next() else { return };
    // The countdown's length and when its seconds changed.
    match view.countdown {
        Some(left) => {
            if drawn.left.is_none() {
                drawn.total = left;
            }
            drawn.total = drawn.total.max(left);
            if drawn.left != Some(left) {
                drawn.since = clock.now;
            }
        }
        None => drawn.total = 0,
    }
    drawn.left = view.countdown;
    let key = (
        view.clone(),
        (units.height * 8.0) as u32,
        (units.scale_factor * 1000.0) as u32,
        format!("{}{}", theme.name, kit.is_some()),
    );
    if drawn.key.as_ref() == Some(&key) && !theme.is_changed() && !kit.as_ref().is_some_and(|k| k.is_changed()) {
        return;
    }
    // The pops: rows that turned ready since the last drawing.
    if let Some((old, ..)) = &drawn.key {
        for r in &view.rows {
            let was = old.rows.iter().find(|o| o.name == r.name).map(|o| o.status);
            if r.status == status::READY && was.is_some_and(|s| s != status::READY) {
                drawn.popped.insert(r.name.clone(), clock.now);
            }
        }
    }
    drawn.key = Some(key);
    visibility.set_if_neq(if view.shown { Visibility::Inherited } else { Visibility::Hidden });
    for e in &content {
        commands.entity(e).despawn();
    }
    if !view.shown {
        drawn.popped.clear();
        return;
    }
    if root_node.padding != UiRect::ZERO {
        root_node.padding = UiRect::ZERO;
    }
    let (Some(images), Some(icons), Some(slants), Some(ticks)) =
        (images.as_deref_mut(), icons.as_deref_mut(), slants.as_deref_mut(), ticks.as_deref_mut())
    else {
        return;
    };
    let pop = theme.dur(L, "ready_pop");
    drawn.popped.retain(|_, at| clock.now - *at < pop as f64);
    let p = Panel { theme: &theme, kit: kit.as_deref(), units: *units };
    let c = &mut commands;
    let tan = theme.tan();
    let hair = units.hairline();

    let content = c
        .spawn((
            LobbyContent,
            UiStyle { w_pct: Some(100.0), h_pct: Some(100.0), ..style!(row).abs([Some(0.0), Some(0.0), None, None]) }
                .justify(JustifyContent::Center),
            Node::default(),
            BackgroundColor(theme.tone_a(Tone::Ink, 0.96)),
            ChildOf(root),
        ))
        .id();
    let frame = p.boxed(c, content, style!(row, w: 100.0, h: 56.25).shrink(0.0));
    let m = p.lay("margin");

    // Corners.
    let k = p.lay("corner");
    for (x, y) in [(k, k), (100.0 - k, k), (k, 56.25 - k), (100.0 - k, 56.25 - k)] {
        let s = 1.1;
        let plus = p.boxed(c, frame, style!(row, w: s, h: s).abs([Some(x - s / 2.0), Some(y - s / 2.0), None, None]));
        let line = theme.tone(Tone::Fg3);
        c.spawn((
            Node { position_type: PositionType::Absolute, left: percent(50.0), width: px(hair), height: percent(100.0), ..default() },
            BackgroundColor(line),
            ChildOf(plus),
        ));
        c.spawn((
            Node { position_type: PositionType::Absolute, top: percent(50.0), height: px(hair), width: percent(100.0), ..default() },
            BackgroundColor(line),
            ChildOf(plus),
        ));
    }

    // Header: the lockup, the title, the tabs.
    let header = p.boxed(c, frame, style!(row, gap: 3.0).abs([Some(m), Some(p.lay("header_y")), Some(m), None]));
    let lk = p.lay("lockup");
    let lockup = p.boxed(c, header, style!(row, gap: 0.55 * lk));
    p.icon(c, lockup, icons, images, "mark", 2.05 * lk, Tone::Fg);
    let words = p.boxed(c, lockup, style!(column, gap: 0.3 * lk));
    let ult = p.boxed(c, words, style!(row).justify(JustifyContent::SpaceBetween));
    for ch in "ULTIMATE".chars() {
        p.text(c, ult, Role::DisplayRegular, 0.62 * lk, Tone::Fg2, &ch.to_string());
    }
    p.text(c, words, Role::Wordmark, lk, Tone::Fg, "GUN GAME");
    p.text(c, header, Role::DisplayBold, p.lay("title"), Tone::Fg, "Lobby");
    p.boxed(c, header, style!(row).grow(1.0));
    let tabs = p.boxed(c, header, style!(row, gap: 0.45));
    for (label, outline) in [("Invite", false), ("Match settings", false), ("Leave", true)] {
        let (bg, fg) = if outline { (Tone::Ink, Tone::Fg) } else { (Tone::Fg, Tone::Ink) };
        let mut s = style!(row, gap: 1.6, pad: [0.5, 0.6, 0.45, 0.7], min_w: 7.4).justify(JustifyContent::SpaceBetween).bg(bg);
        if outline {
            s = s.outline(Tone::Fg3, 1.0);
        }
        let tab = p.boxed(c, tabs, s);
        p.text(c, tab, Role::Mono, p.lay("kv"), fg, label);
        let px = units.phys(units.cq(0.45)).round().max(1.0) as u32;
        let tri = drawn.tri.entry(px).or_insert_with(|| images.add(icons::image(&triangle(), px))).clone();
        c.spawn((
            ImageNode { image: tri, color: theme.tone(fg), ..default() },
            Node { width: Val::Px((px as f32 * 46.0 / 56.0).ceil() / units.scale_factor), height: Val::Px(px as f32 / units.scale_factor), ..default() },
            ChildOf(tab),
        ));
    }
    c.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: units.val(m),
            right: units.val(m),
            top: units.val(p.lay("rule_y")),
            height: px(hair),
            ..default()
        },
        BackgroundColor(theme.tone(Tone::Fg4)),
        ChildOf(frame),
    ));

    // Left: the ready count and the players.
    let ready = view.rows.iter().filter(|r| r.status == status::READY).count();
    let left = p.boxed(c, frame, style!(column, w: p.lay("left_w")).abs([Some(m), Some(p.lay("col_y")), None, None]));
    p.label(c, left, "Ready");
    let unit = match view.slots {
        Some(s) => format!("players \u{2022} {s} slots"),
        None => "players".into(),
    };
    p.big(c, left, &format!("{ready} / {}", view.rows.len()), p.lay("big"), &unit, 0.7);
    let table = p.boxed(c, left, style!(column, margin: [2.3, 0.0, 0.0, 0.0]));
    // The four columns: #, player, tag, status.
    let widths = [Some(p.lay("col_pos")), None, Some(p.lay("col_tag")), Some(p.lay("col_status"))];
    let cell = |c: &mut Commands, row: Entity, i: usize| -> Entity {
        let mut s = style!(row, gap: if i == 3 { 0.7 } else { 0.5 }).shrink(0.0);
        match widths[i] {
            Some(w) => s = s.w(w),
            None => s = s.grow(1.0).shrink(1.0),
        }
        if i == 3 {
            s = s.justify(JustifyContent::FlexEnd);
        }
        let e = c.spawn((s, Node::default(), ChildOf(row))).id();
        if i == 1 {
            c.entity(e).insert(UiStyle { min_w: Some(0.0), ..style!(row, gap: 0.5).grow(1.0).shrink(1.0) });
        }
        e
    };
    let head = c
        .spawn((
            style!(row, gap: 1.0, pad: [0.0, 1.0, 0.7, 1.0]),
            Node { border: UiRect::bottom(px(hair)), ..default() },
            BorderColor::all(theme.tone(Tone::Fg4)),
            ChildOf(table),
        ))
        .id();
    for (i, h) in ["#", "Player", "Tag", "Status"].into_iter().enumerate() {
        let e = cell(c, head, i);
        p.text(c, e, Role::Mono, p.lay("head"), Tone::Fg3, h);
    }
    let top_wins = view.rows.iter().map(|r| r.wins).max().unwrap_or(0);
    for (i, row) in view.rows.iter().enumerate() {
        let you = row.local;
        let r = c
            .spawn((
                LobbyPlayerRow { index: i, you },
                style!(row, gap: 1.0, pad: [0.0, 1.0], h: p.lay("row_h")),
                Node { border: UiRect::bottom(px(hair)), ..default() },
                BorderColor::all(theme.tone(Tone::Fg5)),
                ChildOf(table),
            ))
            .id();
        if you {
            c.entity(r).insert(widgets::row_you(&theme, &units));
        }
        let n = cell(c, r, 0);
        p.text(c, n, Role::Mono, p.lay("label"), Tone::Fg3, &format!("{:02}", i + 1));
        let who = cell(c, r, 1);
        let slot = c.spawn((widgets::bot_slot(images, icons, &theme, &units, row.bot, p.lay("bot"), Tone::Fg2), ChildOf(who))).id();
        if row.bot {
            c.entity(slot).insert(LobbyMark::Bot);
        }
        p.text(c, who, Role::DisplayBold, p.lay("name"), Tone::Fg, &row.name);
        if row.wins > 0 {
            // The session wins: the leader's in the accent.
            let leader = row.wins == top_wins;
            let tone = if leader { Tone::AccText } else { Tone::Fg2 };
            let w = p.boxed(c, who, style!(row, gap: 0.18).shrink(0.0));
            c.entity(w).insert(LobbyWins { wins: row.wins, leader });
            p.icon(c, w, icons, images, "trophy", p.lay("tag"), tone);
            p.text(c, w, Role::Mono, p.lay("tag"), tone, &row.wins.to_string());
        }
        let tag = cell(c, r, 2);
        match (row.host, you) {
            (true, true) => {
                let chip = widgets::spawn_you_host(c, slants, &theme, kit.as_deref(), &units, p.lay("tag"));
                c.entity(chip).insert((LobbyMark::YouHost, ChildOf(tag)));
            }
            (true, false) => {
                let t = p.tag(c, tag, TagKind::Host, "Host");
                c.entity(t).insert(LobbyMark::Host);
            }
            _ => {}
        }
        // Status: the ping, the block, the words.
        let st = cell(c, r, 3);
        let ping = p.boxed(c, st, style!(row, gap: 0.3, margin: [0.0, 0.9, 0.0, 0.0]));
        let size = p.lay("tag");
        match row.ping_ms {
            Some(ms) if !row.bot => {
                let level = PingLevel::from_ms(&theme, ms as u32);
                let meter = widgets::spawn_ping_meter(c, slants, &theme, &units, size * 0.92, level);
                c.entity(meter).insert(ChildOf(ping));
                let number = if level.tone == Tone::Good { Tone::Fg3 } else { level.tone };
                p.text(c, ping, Role::Mono, size, number, &ms.to_string());
                if level.tone == Tone::Threat {
                    let lag = p.boxed(c, ping, style!(row, pad: [0.1, 0.18, 0.07, 0.18]).outline(Tone::Threat, 1.0));
                    p.text(c, lag, Role::Mono, size * 0.7, Tone::Threat, "Lag");
                }
            }
            _ if row.bot => {
                p.text(c, ping, Role::Mono, size, Tone::Fg4, "-");
            }
            _ => {}
        }
        let ready = row.status == status::READY;
        let (bw, bh) = (p.lay("block_w"), p.lay("block_h"));
        // A slanted node's box holds the lean: width + height x tan.
        let block = SlantMaterial::new(&theme, theme.tone(if ready { Tone::Fg } else { Tone::Fg3 }));
        let block = if ready { block } else { block.outline(1.0) };
        let mut b = c.spawn((
            ReadyBlock { ready },
            MaterialNode(slants.add(block)),
            Node { width: units.val(bw + bh * tan), height: units.val(bh), flex_shrink: 0.0, ..default() },
            UiTransform::IDENTITY,
            ChildOf(st),
        ));
        if let Some(at) = drawn.popped.get(&row.name) {
            let mut tl = Timeline::new([Track::new(Prop::ScaleY, 1.9, 1.0).dur(pop).ease(theme.ease("out"))]);
            tl.start = Some(*at);
            tl.step(clock.now);
            let s = tl.value_or(Prop::ScaleY, 1.0);
            b.insert((tl, UiTransform { scale: Vec2::new(1.0, s), ..UiTransform::IDENTITY }));
        }
        p.text(c, st, Role::Mono, p.lay("label"), if ready { Tone::Fg } else { Tone::Fg3 }, &row.state);
    }
    // Free slots.
    let open = view.slots.map_or(0, |s| (s as usize).saturating_sub(view.rows.len()));
    let shown = if view.rows.len() + open <= 10 { open } else { open.min(1) };
    for k in 0..shown {
        let r = c
            .spawn((
                OpenSlot,
                style!(row, gap: 1.0, pad: [0.0, 1.0], h: p.lay("open_h"), margin: [if k == 0 { 0.4 } else { 0.2 }, 0.0, 0.0, 0.0])
                    .outline(Tone::Fg4, 1.0),
                Node::default(),
                ChildOf(table),
            ))
            .id();
        let n = cell(c, r, 0);
        p.text(c, n, Role::Mono, p.lay("label"), Tone::Fg3, &format!("{:02}", view.rows.len() + k + 1));
        let who = cell(c, r, 1);
        let label = if shown < open { format!("Open \u{2022} {open} slots") } else { "Open".into() };
        p.text(c, who, Role::Mono, p.lay("label"), Tone::Fg3, &label);
    }

    // Right: the map, the mode, the countdown.
    let right_w = 100.0 - p.lay("right_x") - m;
    let right = p.boxed(c, frame, style!(column).abs([Some(p.lay("right_x")), Some(p.lay("col_y")), Some(m), None]));
    p.label(c, right, "Map");
    let name = p.boxed(c, right, style!(row, margin: [0.7, 0.0, 0.0, 0.0]));
    p.text(c, name, Role::DisplayBold, p.lay("map_big"), Tone::Fg, &view.map);
    if !view.game.is_empty() {
        let g = p.boxed(c, right, style!(row, margin: [0.7, 0.0, 0.0, 0.0]));
        p.text(c, g, Role::Mono, p.lay("kv"), Tone::Fg2, &view.game);
    }
    // The preview's stand-in.
    let (a, b) = map_tint(&view.map);
    let tile = c
        .spawn((
            style!(column, h: p.lay("map_h"), margin: [1.4, 0.0, 0.0, 0.0]).outline(Tone::Fg4, 1.0),
            Node { overflow: Overflow::clip(), ..default() },
            BackgroundGradient::from(LinearGradient::degrees(110.0, vec![ColorStop::percent(a, 0.0), ColorStop::percent(b, 100.0)])),
            ChildOf(right),
        ))
        .id();
    c.spawn((
        Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::percent(Color::NONE, 50.0),
            ColorStop::percent(theme.tone_a(Tone::Ink, 0.65), 100.0),
        ])),
        ChildOf(tile),
    ));
    let cap = p.boxed(c, tile, style!(row).abs([Some(1.0), None, None, Some(0.9)]));
    p.text(c, cap, Role::Mono, 0.58, Tone::Fg, "Map preview");
    // Mode and guns.
    let kv = p.boxed(c, right, style!(column, margin: [1.2, 0.0, 0.0, 0.0]));
    let guns = view.guns_from.join(" \u{2022} ");
    for (key, value) in [("Mode", view.mode.as_str()), ("Guns from", guns.as_str())] {
        if value.is_empty() {
            continue;
        }
        let r = c
            .spawn((
                style!(row, gap: 1.0, pad: [0.6, 0.0]),
                Node { border: UiRect::bottom(px(hair)), ..default() },
                BorderColor::all(theme.tone(Tone::Fg5)),
                ChildOf(kv),
            ))
            .id();
        let dt = p.boxed(c, r, style!(row, w: 7.6).shrink(0.0));
        p.text(c, dt, Role::Mono, p.lay("kv"), Tone::Fg3, key);
        p.text(c, r, Role::Mono, p.lay("kv"), Tone::Fg, value);
    }
    // The countdown.
    let cd = p.boxed(c, right, style!(column, margin: [1.4, 0.0, 0.0, 0.0]));
    p.label(c, cd, "Starts in");
    let big = match view.countdown {
        Some(s) => format!("{:02}:{:02}", s / 60, s % 60),
        None => "--:--".into(),
    };
    p.big(c, cd, &big, p.lay("big"), &view.line, 0.6);
    let n = p.lay("countdown_ticks").max(1.0) as u32;
    let box_w = units.phys(units.cq(right_w)).floor() as u32;
    let box_h = units.phys(units.cq(p.lay("ticks_h"))).floor() as u32;
    let layout = TickLayout::new(box_w, box_h, n);
    let mut mat = TickBarMaterial::new(&theme, n, layout);
    let bar = CountdownBar {
        material: Handle::default(),
        ticks: n,
        total: drawn.total,
        left: view.countdown,
        since: drawn.since,
        hot: p.lay("countdown_hot"),
    };
    let (lit, hot) = bar.lit(clock.now);
    mat.set(lit, hot);
    let handle = ticks.add(mat);
    let sf = units.scale_factor.max(1e-3);
    let well = p.boxed(c, cd, style!(row, h: p.lay("ticks_h"), margin: [1.0, 0.0, 0.0, 0.0]));
    c.spawn((
        CountdownBar { material: handle.clone(), ..bar },
        MaterialNode(handle),
        Node { width: px(layout.w as f32 / sf), height: px(layout.h as f32 / sf), ..default() },
        ChildOf(well),
    ));

    // Footer: the keys.
    let key_words = match view.key_line() {
        Some("lobby_start") => Some("Start now"),
        Some(_) if view.rows.iter().any(|r| r.local && r.status == status::NOT_READY) => Some("Ready up"),
        Some(_) => Some("Not ready"),
        None => None,
    };
    let foot = p.boxed(c, frame, style!(row, gap: 2.2).abs([Some(m), None, None, Some(p.lay("foot_y"))]));
    let keys = [
        (super::prematch_lobby::LOBBY_KEY.to_uppercase(), key_words),
        ("Tab".into(), Some("Players")),
        ("Esc".into(), Some("Leave")),
    ];
    for (k, words) in keys {
        let Some(words) = words else { continue };
        let item = p.boxed(c, foot, style!(row, gap: 0.55));
        let cap = widgets::spawn_pair(c, widgets::kbd(&theme, kit.as_deref(), &units, &k, p.lay("key")));
        c.entity(cap).insert(ChildOf(item));
        p.text(c, item, Role::Mono, p.lay("key"), Tone::Fg2, words);
    }
}

/// The countdown bar's ticks, every frame from the clock (no rebuild).
fn step_bar(clock: Res<UiClock>, bars: Query<&CountdownBar>, mut mats: Option<ResMut<Assets<TickBarMaterial>>>) {
    let Some(mats) = mats.as_deref_mut() else { return };
    for bar in &bars {
        let (lit, hot) = bar.lit(clock.now);
        if let Some(m) = mats.get(&bar.material)
            && (m.counts.y != lit as f32 || m.counts.z != hot as f32)
            && let Some(mut m) = mats.get_mut(&bar.material)
        {
            m.set(lit, hot);
        }
    }
}
