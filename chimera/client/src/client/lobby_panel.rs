//! The pre-match lobby drawn on the UI kit (`chimera_ui`), for an app
//! whose theme lays the lobby out (`[layout.lobby]`: the Ultimate Gun
//! Game's Signal, docs/plans/active/ugg-ui.md 3, M1 and "Lobby: the
//! starting grid"); without one the shell's own panel (`prematch_lobby`)
//! draws as before. Same state (`prematch_lobby::LobbyView`), same root
//! (`LobbyRoot`), same key (F3): presentation only.
//!
//! The **starting grid** (the user's pick of 2026-11-22, direction A of
//! the lobby mock): on a full-screen ink plate over the frozen map, a
//! 16:9 frame (100 x 56.25 cq, centred) on the shell (`chimera_ui::shell`):
//!
//! - **The bar**: the breadcrumb ("‹ Host" for a listen host, "‹ Join"
//!   for a joiner) over the lobby's name (the server's, `LobbyView::
//!   server`; "Lobby" when unknown; no address); Back (a click on it, Esc
//!   or a right click) is Leave, which the party apps confirm first. At
//!   the right the party and the tiles as on every screen (Settings:
//!   `LobbyAction::Settings`). Hidden under an app's overlay
//!   (`prematch_lobby::LobbyOverlay`), which draws its own.
//! - **Chat**, the whole left column (`layout.lobby.mode_w` wide;
//!   `signal_chat` docks its box there with `[layout.chat]`).
//! - **Players**, the middle: the section's head ("Players", "n of m
//!   loaded", Invite to game: the party panel, `ShellTile::Invite`) over
//!   one lane a player (`LobbyPlayerRow`, a kit `item`), your party first
//!   then the other parties together, each party's lanes joined and edged
//!   in its colour (`PartyEdge`), then everyone alone (`list_order`). A
//!   lane: the avatar (`chimera_ui::avatar`: the player's pick; a bot the
//!   robot, `LobbyMark::Bot`; YOU its white frame), the name, HOST as an
//!   outlined tag (`LobbyMark::Host`), the session wins (`LobbyWins`),
//!   then the load bar across the rest (`Ui::load_line`, `row_load`:
//!   joining dim blue stripes sliding over the whole track, downloading a static striped half-
//!   translucent fill, loading a white fill, each with its share in
//!   words; ready no bar, only "Ready") and the ping (the kit's meter and
//!   ms; a bot none). For the host, Kick takes the ping's place on the
//!   lane under the pointer (`LobbyKickSwap`; `ui_lobby_kick 0` hides it).
//!   A long list scrolls with the kit's bar and its "more below" mark;
//!   lanes never shrink.
//! - **The map and mode**, the right column: the map's card (a gradient
//!   tinted by the map's name stands in) with its name, the mode named big
//!   with the ladder's rungs drawn small (`LobbyView::rungs`), the match's
//!   settings, the games the guns come from. At its foot the **waiting
//!   mark**: counting down, the seconds over the tick bar (`CountdownBar`);
//!   held by the host, "Held" over the kit's busy squares (`Ui::busy`);
//!   else who it waits for, over the busy squares.
//! - **The host's bar** (under the players): Start, Auto or Hold (the
//!   kit's segmented choice: `LobbyAction::Hold`, `lobby_hold`; Auto the
//!   default, Hold for a player who will be right back), Start now (F3,
//!   `LobbyAction::Ready`: `lobby_start`; off while the host's own map
//!   loads, `LobbyView::start_waits_for`) and Add bot (`LobbyAction::
//!   AddBot`), all one size (the kit's one button system). A player sees
//!   who hosts and how the match starts, and Leave.
//! - **Keys** at the foot (F3 Start for the host, Tab Players), hidden
//!   under an app's overlay. The lobby owns the screen and the mouse
//!   (`prematch_lobby::lobby_shown`).
//!
//! Rebuilt when the view, the party, the window's units or the theme
//! change; the tick bar, the load sweeps, the busy squares and the Kick
//! swap move without a rebuild.
//!
//! Known shortcuts: the map preview is a gradient; a lane's download size
//! isn't shown (the share is).

use bevy::{prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_net::prematch::status;
use chimera_ui::{
    IconImages, KitFonts, Role, SlantMaterial, Theme, TickBarMaterial, TickLayout, Tone, UiClock, UiSlant, UiStyle, UiUnits,
    controls::{Kind, Size, State},
    load::Load,
    style,
    widgets::{self, PingLevel},
};

use super::prematch_lobby::{LobbyAction, LobbyAsked, LobbyContent, LobbyOverlay, LobbyRoot, LobbyView, LobbyViewRow};

/// The theme's lobby table.
const L: &str = "lobby";

/// The players' scroll view's key (`ScrollViews`).
pub const PLAYERS_VIEW: &str = "lobby_players";

/// Whether the theme draws the lobby here (`[layout.lobby]`).
pub fn themed(theme: Option<Res<Theme>>) -> bool {
    theme.is_some_and(|t| t.has_layout(L))
}

/// A player's mark, as drawn (tests find them).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyMark {
    /// The outlined HOST tag.
    Host,
    /// The robot in the avatar's place.
    Bot,
}

/// A player's lane (`index` into `LobbyView::rows`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LobbyPlayerRow {
    pub index: usize,
    /// This machine's player: tinted, its avatar framed.
    pub you: bool,
}

/// A player's session wins beside the name (trophy and count, only with
/// any; the session's leader in the accent).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LobbyWins {
    pub wins: u32,
    pub leader: bool,
}

/// The host's Kick and the ping it replaces on a lane: `kick` shows while
/// the lane (or the Kick) is under the pointer, the ping otherwise.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LobbyKickSwap {
    pub lane: Entity,
    pub kick: bool,
}

/// The waiting mark's state, as drawn (tests read it).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyWait {
    Counting,
    Held,
    Waiting,
}

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
    app.add_systems(Update, (draw, step_bar, kick_swap).chain().after(super::prematch_lobby::LobbyFollow).run_if(themed))
        .add_systems(Update, press.before(super::prematch_lobby::LobbyFollow));
}

/// A click on one of the lobby's buttons (`LobbyAction`).
fn press(pressed: Query<(&Interaction, &LobbyAction), Changed<Interaction>>, mut out: MessageWriter<LobbyAction>) {
    for (i, a) in &pressed {
        if *i == Interaction::Pressed {
            out.write(*a);
        }
    }
}

/// Kick in the ping's place on the lane under the pointer.
fn kick_swap(lanes: Query<&Interaction>, mut swaps: Query<(Entity, &LobbyKickSwap, &mut Visibility)>) {
    let over: Vec<Entity> = swaps
        .iter()
        .filter(|(e, s, _)| s.kick && lanes.get(*e).is_ok_and(|i| *i != Interaction::None))
        .map(|(_, s, _)| s.lane)
        .collect();
    for (_, s, mut vis) in &mut swaps {
        let hovered = over.contains(&s.lane) || lanes.get(s.lane).is_ok_and(|i| *i != Interaction::None);
        let show = hovered == s.kick;
        vis.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    }
}

#[derive(Default)]
struct Drawn {
    key: Option<(LobbyView, u32, u32, String)>,
    /// The countdown's length: the most seconds seen since it began.
    total: u32,
    left: Option<u32>,
    since: f64,
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
    units: UiUnits,
}

impl Panel<'_> {
    fn lay(&self, k: &str) -> f32 {
        self.theme.layout(L, k)
    }

    fn lay_or(&self, k: &str, or: f32) -> f32 {
        self.theme.layout_opt(L, k).unwrap_or(or)
    }

    /// A styled box under `parent`.
    fn boxed(&self, c: &mut Commands, parent: Entity, s: UiStyle) -> Entity {
        c.spawn((s, Node::default(), ChildOf(parent))).id()
    }

    /// A text in a role at `size` cq under `parent`, in capitals.
    fn text(&self, c: &mut Commands, parent: Entity, role: Role, size: f32, tone: Tone, text: &str) -> Entity {
        self.cased(c, parent, role, size, tone, &text.to_uppercase())
    }

    /// A text as it is (a sentence, a player's name).
    fn cased(&self, c: &mut Commands, parent: Entity, role: Role, size: f32, tone: Tone, text: &str) -> Entity {
        c.spawn((
            Text::new(text),
            chimera_ui::UiText::new(role, size, tone),
            // `case_texts` upper-cases a role that does on every change of
            // the text, and `Ellipsize` rewrites a name: without this the
            // name flipped between capitals and as written.
            chimera_ui::KeepCase,
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
            ChildOf(parent),
        ))
        .id()
    }

    /// A section's head: the accent mark, the title, a count, a hairline
    /// to the end (the mock's `fsh`). Returns the head row.
    fn head(&self, c: &mut Commands, parent: Entity, title: &str, count: Option<&str>) -> Entity {
        let h = self.boxed(c, parent, style!(row, gap: 0.7, margin: [0.0, 0.0, 0.8, 0.0]).align(AlignItems::Center));
        let lean = 1.05 * self.theme.tan();
        c.spawn((style!(row, w: 0.55 + lean, h: 1.05).shrink(0.0), Node::default(), UiSlant::new(Tone::Acc, 1.0), ChildOf(h)));
        self.text(c, h, Role::DisplayBold, 0.86, Tone::Fg, title);
        if let Some(n) = count {
            self.text(c, h, Role::Mono, 0.54, Tone::Fg3, n);
        }
        let rule = self.boxed(c, h, style!(row, h: 0.0).grow(1.0));
        c.entity(rule).insert((Node { height: px(self.units.hairline()), ..default() }, BackgroundColor(self.theme.tone(Tone::Fg5))));
        h
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
    (view, asked, overlay, party): (Res<LobbyView>, Res<LobbyAsked>, Option<Res<LobbyOverlay>>, Res<chimera_ui::shell::Party>),
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
        format!("{}{}{}{}{:?}", theme.name, kit.is_some(), asked.0, overlay.as_ref().is_some_and(|o| o.0), *party),
    );
    if drawn.key.as_ref() == Some(&key) && !theme.is_changed() && !kit.as_ref().is_some_and(|k| k.is_changed()) {
        return;
    }
    drawn.key = Some(key);
    visibility.set_if_neq(if view.shown { Visibility::Inherited } else { Visibility::Hidden });
    for e in &content {
        commands.entity(e).despawn();
    }
    if !view.shown {
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
    let p = Panel { theme: &theme, units: *units };
    let c = &mut commands;

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

    // The shell's bar: the breadcrumb (Back is Leave, which the app
    // confirms) over the lobby's name, the party, the system tiles.
    let ui = chimera_ui::controls::Ui::new(&theme, kit.as_deref(), *units);
    let parent = if view.hosting { "Host" } else { "Join" };
    let title = if view.server.trim().is_empty() { "Lobby" } else { view.server.trim() };
    let bar = chimera_ui::shell::Bar {
        left: chimera_ui::shell::Left::Crumb { parent, title },
        party: &party.members,
        party_max: party.max,
        invite: party.invite,
        open: None,
    };
    let parts = chimera_ui::shell::bar(&ui, c, frame, images, icons, &bar);
    if let Some(e) = parts.tile(chimera_ui::shell::ShellTile::Settings) {
        c.entity(e).insert(LobbyAction::Settings);
    }
    c.entity(root).insert(chimera_ui::shell::ShellScreen);
    // Under an app's overlay (which has its own header) the header
    // doesn't ghost through its shade.
    let overlaid = overlay.as_ref().is_some_and(|o| o.0);
    if overlaid && let Some(b) = parts.bar {
        c.entity(b).insert(Visibility::Hidden);
    }

    // The window: the chat's column at the left (`signal_chat` docks it),
    // the players in the middle, the map, the mode and the waiting mark
    // at the right, the host's bar along the foot.
    let sh = chimera_ui::shell::ShellLayout::of(&theme);
    let win = chimera_ui::shell::window(&ui, c, frame);
    if overlaid {
        c.entity(win).insert(Visibility::Hidden);
    }
    let body_bottom = sh.act_h + 1.4;
    let mode_w = p.lay("mode_w");
    let map_w = p.lay("map_w");

    // Players.
    let list = p.boxed(
        c,
        win,
        style!(column).abs([Some(sh.rows_in + mode_w + 2.8), Some(1.6), Some(sh.rows_in + map_w + 1.8), Some(body_bottom)]),
    );
    let loaded = view.rows.iter().filter(|r| r.bot || matches!(r.status, status::READY | status::NOT_READY)).count();
    let count = match view.slots {
        Some(s) => format!("{loaded} of {} loaded \u{b7} {s} slots", view.rows.len()),
        None => format!("{loaded} of {} loaded", view.rows.len()),
    };
    let head = p.head(c, list, "Players", Some(&count));
    // Invite to game: the party panel (its "+"), live with a hub.
    let inv = ui.button_icon(c, head, Kind::Secondary, Some("plus"), "Invite to game", None, Size::Small, State { disabled: !party.invite, ..State::NONE });
    if party.invite {
        c.entity(inv).insert(chimera_ui::shell::ShellTile::Invite);
    }
    let scroll = ui.scroll(c, list, PLAYERS_VIEW, UiStyle { grow: Some(1.0), shrink: Some(1.0), ..UiStyle::row() }, UiStyle { gap: Some(0.3), ..UiStyle::column() });
    // Lanes never shrink: the list scrolls in what is left.
    c.entity(scroll.frame).entry::<Node>().and_modify(|mut n| {
        n.min_height = Val::Px(0.0);
        n.flex_basis = Val::Px(0.0);
    });
    let lanes = scroll.content;
    let order = list_order(&view.rows);
    let mine = view.rows.iter().find(|r| r.local).map(|r| r.party).filter(|p| *p != 0);
    let top_wins = view.rows.iter().map(|r| r.wins).max().unwrap_or(0);
    let row_h = p.lay("row_h");
    let cw_av = p.lay("avatar");
    let name_w = p.lay_or("name_w", 11.0);
    let ping_w = p.lay("ping_w");
    let (bar_h, words_w) = (p.lay_or("bar_h", 0.6), p.lay_or("words_w", 7.6));
    let kick_ok = view.hosting && view.kick;
    let mut last_party: Option<u8> = None;
    for (n, &i) in order.iter().enumerate() {
        let row = &view.rows[i];
        let you = row.local;
        // A party's lanes join; groups stand apart.
        let party = (row.party != 0).then_some(row.party);
        let joined = n > 0 && party.is_some() && party == last_party;
        last_party = party;
        let lane_style = style!(row, gap: 0.8, pad: [0.0, 0.7, 0.0, 0.7], h: row_h, margin: [if n == 0 || joined { 0.0 } else { 0.45 }, 0.0, 0.0, 0.0])
            .shrink(0.0)
            .bg_a(Tone::Fg, if you { 0.07 } else { 0.0 });
        let r = ui.item(c, lanes, lane_style, State::NONE, false);
        c.entity(r).insert(LobbyPlayerRow { index: i, you });
        if let Some(pid) = party {
            // The party's edge: a thin solid colour down its lanes' left
            // (yours cobalt, another party one of the avatar palette's).
            let colour = if Some(pid) == mine { theme.tone(Tone::Acc) } else { party_colour(pid) };
            c.spawn((
                PartyEdge(pid),
                Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, width: units.val(0.22), ..default() },
                BackgroundColor(colour),
                Pickable::IGNORE,
                ChildOf(r),
            ));
        }
        let face = match chimera_ui::avatar::AvatarLook::from_byte(row.avatar) {
            _ if row.bot => chimera_ui::avatar::Face::Bot,
            Some(l) => chimera_ui::avatar::Face::Look(l),
            None => chimera_ui::avatar::Face::Look(chimera_ui::avatar::AvatarLook::for_name(&row.name)),
        };
        let av = chimera_ui::avatar::spawn(&ui, c, r, images, icons, face, chimera_ui::avatar::AvatarMarks { you, ..default() }, cw_av);
        if row.bot {
            c.entity(av).insert(LobbyMark::Bot);
        }
        let who = p.boxed(c, r, UiStyle { min_w: Some(0.0), ..style!(row, gap: 0.6, w: name_w).shrink(0.0) });
        let name = p.cased(c, who, Role::DisplayBold, p.lay("name"), if you { Tone::Fg } else { Tone::Fg }, &row.name);
        c.entity(name).insert(chimera_ui::Ellipsize::in_parent(row.name.clone()));
        if row.host {
            let t = p.boxed(c, who, style!(row, pad: [0.2, 0.32, 0.16, 0.32]).outline(Tone::Fg3, 1.0).shrink(0.0));
            c.entity(t).insert(LobbyMark::Host);
            p.text(c, t, Role::Mono, 0.46, Tone::Fg2, "Host");
        }
        if row.wins > 0 {
            let leader = row.wins == top_wins;
            let tone = if leader { Tone::AccText } else { Tone::Fg2 };
            let w = p.boxed(c, who, style!(row, gap: 0.18).shrink(0.0));
            c.entity(w).insert(LobbyWins { wins: row.wins, leader });
            p.icon(c, w, icons, images, "trophy", p.lay("tag"), tone);
            p.text(c, w, Role::Mono, p.lay("tag"), tone, &row.wins.to_string());
        }
        // The load across the rest of the lane.
        let (load, words) = row_load(row);
        ui.load_line(c, r, load, &words, clock.now, bar_h, words_w, 0.56);
        // The ping (a bot none); for the host, Kick in its place under
        // the pointer.
        let end = p.boxed(c, r, style!(row, w: ping_w).shrink(0.0).justify(JustifyContent::FlexEnd));
        let pc = p.boxed(c, end, style!(row, gap: 0.45).justify(JustifyContent::FlexEnd));
        if let (Some(ms), false) = (row.ping_ms, row.bot) {
            let level = PingLevel::from_ms(&theme, ms as u32);
            let meter = widgets::spawn_ping_meter(c, slants, &theme, &units, 0.97, level);
            c.entity(meter).insert(ChildOf(pc));
            p.text(c, pc, Role::Mono, 0.54, Tone::Fg3, &format!("{ms}"));
        }
        if kick_ok && !you && !row.host {
            c.entity(pc).insert(LobbyKickSwap { lane: r, kick: false });
            let holder = p.boxed(c, end, style!(row).abs([None, Some(0.0), Some(0.0), Some(0.0)]).align(AlignItems::Center));
            c.entity(holder).insert((LobbyKickSwap { lane: r, kick: true }, Visibility::Hidden));
            let k = ui.button(c, holder, Kind::Destructive, "Kick", None, Size::Small, State::NONE);
            c.entity(k).insert((LobbyAction::Kick(i.min(255) as u8), LobbyKickSwap { lane: r, kick: true }));
        }
    }
    ui.scroll_more(c, &scroll, "More players below", 2.4);

    // The map, the mode, the waiting mark.
    let right = p.boxed(c, win, style!(column, w: map_w).abs([None, Some(1.6), Some(sh.rows_in), Some(body_bottom)]));
    p.head(c, right, "Map", None);
    let (a, b) = map_tint(&view.map);
    let tile = c
        .spawn((
            style!(column, h: p.lay("card_h")),
            Node { overflow: Overflow::clip(), ..default() },
            BackgroundGradient::from(LinearGradient::degrees(160.0, vec![ColorStop::percent(a, 0.0), ColorStop::percent(b, 100.0)])),
            ChildOf(right),
        ))
        .id();
    c.spawn((
        Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::percent(Color::NONE, 45.0),
            ColorStop::percent(theme.tone_a(Tone::Ink, 0.8), 100.0),
        ])),
        ChildOf(tile),
    ));
    let pc = p.boxed(c, tile, style!(row).abs([Some(1.0), Some(1.0), None, None]));
    p.text(c, pc, Role::Mono, 0.5, Tone::Fg3, "Preview coming");
    let cap = p.boxed(c, tile, style!(row).abs([Some(1.0), None, None, Some(0.9)]));
    p.text(c, cap, Role::DisplayBlack, p.lay("map_big"), Tone::Fg, &view.map);

    // The mode, named big (the gun game at a glance), its ladder small.
    let mode = p.boxed(c, right, style!(column, gap: 0.35, margin: [1.0, 0.0, 0.0, 0.0]));
    p.text(c, mode, Role::Mono, 0.54, Tone::Fg3, "Mode");
    p.text(c, mode, Role::DisplayBlack, 1.6, Tone::Fg, &view.mode);
    if view.rungs > 1 {
        let lad = p.boxed(c, mode, style!(row, gap: 0.16, h: 0.8, margin: [0.25, 0.0, 0.0, 0.0]));
        for k in 0..view.rungs {
            let tone = if k == 0 { Tone::Fg } else if k + 1 == view.rungs { Tone::Acc } else { Tone::Fg4 };
            c.spawn((style!(row).grow(1.0).shrink(1.0), Node { height: percent(100.0), min_width: Val::ZERO, ..default() }, UiSlant::new(tone, 1.0), ChildOf(lad)));
        }
        let ends = p.boxed(c, mode, style!(row).justify(JustifyContent::SpaceBetween));
        p.text(c, ends, Role::Mono, 0.5, Tone::Fg3, "Rung 1");
        p.text(c, ends, Role::Mono, 0.5, Tone::Fg3, &format!("{} rungs", view.rungs));
    }
    let kv = |c: &mut Commands, k: &str, v: &str| {
        let r = c
            .spawn((
                style!(row, gap: 1.0, pad: [0.25, 0.0]).justify(JustifyContent::SpaceBetween),
                Node { border: UiRect::bottom(px(units.hairline())), ..default() },
                BorderColor::all(theme.tone(Tone::Fg5)),
                ChildOf(mode),
            ))
            .id();
        p.text(c, r, Role::Mono, 0.54, Tone::Fg3, k);
        p.text(c, r, Role::Mono, 0.6, Tone::Fg, v);
    };
    let first_kv = p.boxed(c, mode, style!(row, h: 0.4));
    let _ = first_kv;
    // The match's settings, so a player can leave before being put into
    // one they don't want (`LobbyView::settings`).
    for (k, v) in &view.settings {
        kv(c, k, v);
    }
    if !view.guns_from.is_empty() {
        kv(c, "Guns from", &view.guns_from.join(" \u{b7} "));
    }
    p.boxed(c, right, style!(column).grow(1.0));

    // The waiting mark.
    let wait = p.boxed(c, right, style!(column, gap: 0.4));
    let not_in: Vec<&LobbyViewRow> = view.rows.iter().filter(|r| !r.bot && !matches!(r.status, status::READY | status::NOT_READY)).collect();
    if let Some(left) = view.countdown {
        c.entity(wait).insert(LobbyWait::Counting);
        let k = if not_in.is_empty() { "Everyone loaded \u{b7} starts in" } else { "Starts in" };
        p.text(c, wait, Role::Mono, 0.56, Tone::AccText, k);
        let hot = (left as f32) <= p.lay("countdown_hot");
        p.text(c, wait, Role::DisplayBlack, p.lay("big"), if hot { Tone::AccText } else { Tone::Fg }, &format!("{}:{:02}", left / 60, left % 60));
        let n = p.lay("countdown_ticks").max(1.0) as u32;
        let box_w = units.phys(units.cq(map_w)).floor() as u32;
        let box_h = units.phys(units.cq(p.lay("ticks_h"))).floor() as u32;
        let layout = TickLayout::new(box_w, box_h, n);
        let mut mat = TickBarMaterial::new(&theme, n, layout);
        let bar = CountdownBar { material: Handle::default(), ticks: n, total: drawn.total, left: view.countdown, since: drawn.since, hot: p.lay("countdown_hot") };
        let (lit, hot) = bar.lit(clock.now);
        mat.set(lit, hot);
        let handle = ticks.add(mat);
        let sf = units.scale_factor.max(1e-3);
        let well = p.boxed(c, wait, style!(row, h: p.lay("ticks_h")));
        c.spawn((
            CountdownBar { material: handle.clone(), ..bar },
            MaterialNode(handle),
            Node { width: px(layout.w as f32 / sf), height: px(layout.h as f32 / sf), ..default() },
            ChildOf(well),
        ));
    } else if view.host_start {
        c.entity(wait).insert(LobbyWait::Held);
        p.text(c, wait, Role::Mono, 0.56, Tone::Fg3, if not_in.is_empty() { "Everyone loaded" } else { "Held" });
        p.text(c, wait, Role::DisplayBlack, 2.0, Tone::Fg, "Held by host");
        ui.busy(c, wait, 12, 0.5);
        let line = if view.hosting { "Press Start now when everyone is here.".to_string() } else { format!("{} holds the start.", host_name(&view)) };
        p.cased(c, wait, Role::Text, 0.66, Tone::Fg2, &line);
    } else {
        c.entity(wait).insert(LobbyWait::Waiting);
        let n = not_in.len();
        let k = match n {
            0 => "Waiting for the round".to_string(),
            1 => "Waiting for 1 player".to_string(),
            n => format!("Waiting for {n} players"),
        };
        p.text(c, wait, Role::Mono, 0.56, Tone::Fg3, &k);
        let who = match not_in.as_slice() {
            [] => String::new(),
            [one] => one.name.clone(),
            [one, rest @ ..] => format!("{} +{}", one.name, rest.len()),
        };
        if !who.is_empty() {
            let t = p.cased(c, wait, Role::DisplayBlack, 2.0, Tone::Fg, &who);
            c.entity(t).insert(chimera_ui::Ellipsize::in_parent(who.clone()));
        }
        ui.busy(c, wait, 12, 0.5);
        p.cased(c, wait, Role::Text, 0.66, Tone::Fg2, "Starts by itself once everyone has loaded.");
    }

    // The host's bar: Auto or Hold, Start now, Add bot, one size; a
    // player: who hosts, how it starts, and Leave.
    let bar = chimera_ui::shell::action_bar(&ui, c, win);
    p.boxed(c, bar, style!(row, w: mode_w + 2.0).shrink(0.0));
    if view.hosting {
        p.text(c, bar, Role::Mono, 0.54, Tone::Fg3, "Start");
        let held = view.host_start;
        let seg = ui.segmented_sized(c, bar, &["Auto", "Hold"], Some(usize::from(held)), &[], State::NONE, Some(10.0), Size::Regular);
        c.entity(seg[0]).insert(LobbyAction::Hold(false));
        c.entity(seg[1]).insert(LobbyAction::Hold(true));
        // F3 (the lobby's key; Enter is the docked chat's). Off, saying
        // so, while the host's own map is still loading.
        let key = super::prematch_lobby::LOBBY_KEY.to_uppercase();
        let waits = view.start_waits_for().is_some() || view.countdown.is_some();
        let go = ui.button(c, bar, Kind::Primary, "Start now", Some(&key), Size::Regular, State { disabled: waits, ..State::NONE });
        if !waits {
            c.entity(go).insert(LobbyAction::Ready);
        }
        let bot = ui.button_icon(c, bar, Kind::Secondary, Some("bot"), "Add bot", None, Size::Regular, State::NONE);
        c.entity(bot).insert(LobbyAction::AddBot);
        let note = match view.start_waits_for() {
            Some(w) => format!("{w}."),
            None if kick_ok => "Point at a player to kick.".into(),
            None => String::new(),
        };
        let st = p.boxed(c, bar, style!(row).grow(1.0));
        if !note.is_empty() {
            ui.sentence(c, st, 0.7, Tone::Fg3, &note);
        }
    } else {
        let st = p.boxed(c, bar, style!(row).grow(1.0));
        ui.sentence(c, st, chimera_ui::controls::SENTENCE, Tone::Fg2, &action_line(&view, loaded));
        // A player may leave before the match it didn't want starts.
        let leave = ui.button(c, bar, Kind::Secondary, "Leave", None, Size::Regular, State::NONE);
        c.entity(leave).insert(LobbyAction::Leave);
    }

    // Footer: the keys (not under an app's overlay, which has its own).
    if overlaid {
        return;
    }
    let key = super::prematch_lobby::LOBBY_KEY.to_uppercase();
    let mut keys: Vec<(&str, &str)> = Vec::new();
    if view.key_line() == Some("lobby_start") {
        keys.push((&key, "Start"));
    }
    keys.push(("Tab", "Players"));
    chimera_ui::shell::keys(&ui, c, frame, &keys);
}

/// The host's name ("The host" when not listed).
fn host_name(v: &LobbyView) -> String {
    v.rows.iter().find(|r| r.host).map_or_else(|| "The host".to_string(), |r| r.name.clone())
}

/// The list's order (module doc): your party, other parties together
/// (by their first member's place), then the rest, each in the server's
/// order.
pub fn list_order(rows: &[LobbyViewRow]) -> Vec<usize> {
    let mine = rows.iter().find(|r| r.local).map(|r| r.party);
    let first_of = |p: u8| rows.iter().position(|r| r.party == p).unwrap_or(0);
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by_key(|&i| {
        let r = &rows[i];
        let group = match (r.local, r.party) {
            (true, _) => (0, 0),
            (_, 0) => (2, i),
            (_, p) if Some(p) == mine => (0, 0),
            (_, p) => (1, first_of(p)),
        };
        (group, !r.local, i)
    });
    order
}

/// Another party's edge colour: one of the avatar palette's tints.
fn party_colour(party: u8) -> Color {
    chimera_ui::avatar::AvatarLook::new(0, party.wrapping_add(3) % 8).tint()
}

/// A party's coloured edge on one of its lanes (its party id).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartyEdge(pub u8);

/// A lane's load and its words (module doc; the user's mapping of
/// 2026-11-22): connecting is joining (nothing measured yet); fetching
/// the match's files downloading; the map, the models, the physics
/// loading, each by its stage's word; on the map (and every bot) ready.
pub fn row_load(r: &LobbyViewRow) -> (Load, String) {
    match r.status {
        _ if r.bot => (Load::Ready, "Ready".into()),
        status::READY | status::NOT_READY => (Load::Ready, "Ready".into()),
        status::DOWNLOADING => (Load::Downloading(r.fraction.unwrap_or(0.0)), "Downloading".into()),
        status::LOADING => match r.step {
            Some(step) => (Load::Loading(r.fraction.unwrap_or(0.0)), step.short().into()),
            None => (Load::Loading(r.fraction.unwrap_or(0.0)), "Loading".into()),
        },
        _ => (Load::Joining, "Joining".into()),
    }
}

/// A player's sentence in the bar: who hosts and how the match starts.
pub fn action_line(v: &LobbyView, loaded: usize) -> String {
    let n = v.rows.len();
    let host = host_name(v);
    if let Some(left) = v.countdown {
        return format!("{loaded} of {n} loaded. The match starts in {left} s.");
    }
    if v.host_start {
        format!("{host} hosts and holds the start.")
    } else {
        format!("{host} hosts. The match starts by itself once everyone has loaded.")
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
