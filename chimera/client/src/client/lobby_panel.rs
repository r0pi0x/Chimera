//! The pre-match lobby drawn on the UI kit (`chimera_ui`), for an app
//! whose theme lays the lobby out (`[layout.lobby]`: the Ultimate Gun
//! Game's Signal, docs/plans/active/ugg-ui.md 3 and M1); without one the
//! shell's own panel (`prematch_lobby`) draws as before. Same state
//! (`prematch_lobby::LobbyView`), same root (`LobbyRoot`), same key (F3):
//! presentation only. Built from the kit's style layer (`style!`,
//! `UiText`, `UiSlant`) and its Look Book V marks (`widgets`).
//!
//! On a full-screen ink plate over the frozen map, a 16:9 frame (100 x
//! 56.25 cq, centred) on the shell (`chimera_ui::shell`; docs/plans/
//! active/ugg-ui.md "Signal revision 8"):
//!
//! - **The bar**: the breadcrumb ("‹ Host" for a listen host, "‹ Join"
//!   for a joiner) over "Lobby"; Back (a click on it, Esc or a right
//!   click) is Leave, which the party apps confirm first. At the right the
//!   party and the tiles: Settings (`LobbyAction::Settings`: the party
//!   apps' Settings over the lobby), Report a bug. Hidden under an app's
//!   overlay (`prematch_lobby::LobbyOverlay`), which draws its own.
//! - **Mode**, pinned at the left (`layout.lobby.mode_w`): the minigame,
//!   Free for all, the match's settings (`LobbyView::settings`: its length,
//!   the host's that matter to a player), the games the guns come from,
//!   one per line in full.
//! - **The list** (Call of Duty's, free for all): everyone in one list,
//!   you first (then your party: `LobbyRow::party`), then the server's order;
//!   "Loaded n / m" and the slots above. A row (`LobbyPlayerRow`): the
//!   slot number, the avatar (`chimera_ui::avatar`, from `LobbyRow::
//!   avatar`; YOU its white frame; a bot the robot, `LobbyMark::Bot`), the
//!   name in Display, HOST as an outlined tag (`LobbyMark::Host`), the
//!   session wins (trophy and count, the leader's in the accent:
//!   `LobbyWins`), the status as the progress component (variant B:
//!   loaded is ready, filled; downloading a hollow bar filling with its
//!   share; loading the same with its stage's word (Loading, Models,
//!   Physics: `chimera_net::net::load_stages`), striped until told; connecting
//!   striped) and the ping (the bars and
//!   ms; the loss mark on a bad line; a bot a dim meter and a dash). Free
//!   slots read "Open" (`OpenSlot`).
//! - **Up next**, at the right: the map's big card (a gradient tinted by
//!   the map's name stands in, "Preview coming") with its name and game;
//!   once the countdown runs, STANDBY and its seconds over the tick bar
//!   (`CountdownBar`: pixel-snapped ticks lit as it runs, stepped every
//!   frame from `UiClock`, the last `layout.lobby.countdown_hot` seconds
//!   in the accent).
//! - **The action bar**: the status sentence (how many have loaded; what
//!   starts the match: "Waiting for the host to start" while the first
//!   lobby waits for the host, `NetLobby::host_start`) and, for the
//!   host, **Start** [F3], the big hero (`LobbyAction::Ready`: the host's
//!   `lobby_start`; F3, not the mock's Enter, which opens the docked
//!   chat); off, with the sentence saying what it waits for, while the
//!   host's own map loads (`LobbyView::start_waits_for`). Nobody readies up: loaded is ready. A player who isn't the
//!   host gets **Leave** (the kit's secondary button, `LobbyAction::Leave`)
//!   there too, to go before a match it didn't want starts.
//! - **Keys** at the foot (F3 Start for the host, Tab Players), hidden
//!   under an app's overlay. The lobby owns the screen and the mouse
//!   (`prematch_lobby::lobby_shown`), so its buttons take clicks through a
//!   download and a load.
//!
//! Rebuilt when the view, the party, the window's units or the theme
//! change; the tick bar animates without a rebuild.
//!
//! Known shortcuts: the map preview is a gradient; no parties yet, so no
//! party edges (a party's coloured left edge) and nobody but you is
//! grouped; Invite isn't drawn; a loader's share isn't
//! known (striped); the loss mark stands for a bad ping (the lobby has no
//! loss figure).

use bevy::{prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_net::prematch::status;
use chimera_ui::{
    IconImages, KitFonts,
    Role, SlantMaterial, Theme, TickBarMaterial, TickLayout, Tone, UiClock,
    UiStyle, UiText, UiUnits, style,
    widgets::{self, PingLevel, TagKind},
};

use super::prematch_lobby::{LobbyAction, LobbyAsked, LobbyContent, LobbyOverlay, LobbyRoot, LobbyView};

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
    app.add_systems(Update, (draw, step_bar).chain().after(super::prematch_lobby::LobbyFollow).run_if(themed))
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

    /// A text in a role at `size` cq under `parent`, in capitals.
    fn text(&self, c: &mut Commands, parent: Entity, role: Role, size: f32, tone: Tone, text: &str) -> Entity {
        self.cased(c, parent, role, size, tone, &text.to_uppercase())
    }

    /// A text as it is (a sentence, a player's name): `Role::Text`, ours.
    fn cased(&self, c: &mut Commands, parent: Entity, role: Role, size: f32, tone: Tone, text: &str) -> Entity {
        c.spawn((
            Text::new(text),
            UiText::new(role, size, tone),
            // `case_texts` upper-cases a role that does on every change of
            // the text, and `Ellipsize` rewrites a name: without this the
            // name flipped between capitals and as written.
            chimera_ui::KeepCase,
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
    let p = Panel { theme: &theme, kit: kit.as_deref(), units: *units };
    let c = &mut commands;
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

    // The shell's bar: the breadcrumb (Back is Leave, which the app
    // confirms) over the title, the party, the system tiles.
    let ui = chimera_ui::controls::Ui::new(&theme, kit.as_deref(), *units);
    let parent = if view.hosting { "Host" } else { "Join" };
    let bar = chimera_ui::shell::Bar {
        left: chimera_ui::shell::Left::Crumb { parent, title: "Lobby" },
        party: &party.members,
        party_max: party.max, invite: party.invite,
        open: None,
    };
    let parts = chimera_ui::shell::bar(&ui, c, frame, images, icons, &bar);
    if let Some(e) = parts.tile(chimera_ui::shell::ShellTile::Settings) {
        c.entity(e).insert(LobbyAction::Settings);
    }
    c.entity(root).insert(chimera_ui::shell::ShellScreen);
    let header = parts.bar.unwrap_or(frame);
    let rule = parts.bar.unwrap_or(frame);
    // Under an app's overlay (which has its own header) the header
    // doesn't ghost through its shade.
    let overlaid = overlay.as_ref().is_some_and(|o| o.0);
    if overlaid {
        c.entity(header).insert(Visibility::Hidden);
        let _ = rule;
    }

    // The window: the mode at the left, the list in the middle, the map
    // at the right, the action bar along its foot.
    let sh = chimera_ui::shell::ShellLayout::of(&theme);
    let win = chimera_ui::shell::window(&ui, c, frame);
    if overlaid {
        c.entity(win).insert(Visibility::Hidden);
    }
    let body_bottom = sh.act_h + 1.4;
    let mode_w = p.lay("mode_w");
    let map_w = p.lay("map_w");

    // The mode, pinned at the left.
    let mode = p.boxed(
        c,
        win,
        style!(column, gap: 0.6, w: mode_w).abs([Some(sh.rows_in), Some(1.6), None, Some(body_bottom)]),
    );
    c.spawn((
        Node { position_type: PositionType::Absolute, right: units.val(-1.4), top: Val::ZERO, bottom: Val::ZERO, width: px(hair), ..default() },
        BackgroundColor(theme.tone(Tone::Fg5)),
        ChildOf(mode),
    ));
    p.label(c, mode, "Mode");
    p.text(c, mode, Role::DisplayBold, 1.25, Tone::Fg, &view.mode);
    let kv = |c: &mut Commands, k: &str, v: &str| {
        let r = c
            .spawn((
                style!(row, gap: 1.0, pad: [0.6, 0.0]),
                Node { border: UiRect::bottom(px(hair)), ..default() },
                BorderColor::all(theme.tone(Tone::Fg5)),
                ChildOf(mode),
            ))
            .id();
        let dt = p.boxed(c, r, style!(row, w: 8.2).shrink(0.0));
        p.text(c, dt, Role::Mono, 0.56, Tone::Fg3, k);
        p.text(c, r, Role::Mono, 0.6, Tone::Fg, v);
    };
    kv(c, "Mode", "Free for all");
    // The match's settings, so a player can leave before being put into
    // one they don't want (`LobbyView::settings`).
    for (k, v) in &view.settings {
        kv(c, k, v);
    }
    if !view.guns_from.is_empty() {
        let g = p.boxed(c, mode, style!(column, gap: 0.35, margin: [0.6, 0.0, 0.0, 0.0]));
        p.text(c, g, Role::Mono, 0.56, Tone::Fg3, "Guns from");
        for game in &view.guns_from {
            p.text(c, g, Role::DisplayRegular, 0.78, Tone::Fg, &full_game_name(game));
        }
    }

    // The list: everyone, your party first; status a progress bar, ping
    // the bars and ms.
    let list = p.boxed(
        c,
        win,
        style!(column).abs([Some(sh.rows_in + mode_w + 2.8), Some(1.6), Some(sh.rows_in + map_w + 1.8), Some(body_bottom)]),
    );
    let loaded = view.rows.iter().filter(|r| r.status == status::READY).count();
    let head = p.boxed(c, list, style!(row, gap: 1.2, margin: [0.0, 0.0, 0.6, 0.0]).align(AlignItems::Center));
    let lt = p.boxed(c, head, style!(column).align(AlignItems::FlexStart));
    p.label(c, lt, "Loaded");
    p.text(c, head, Role::DisplayBlack, p.lay("big"), Tone::Fg, &format!("{loaded} / {}", view.rows.len()));
    let unit = match view.slots {
        Some(s) => format!("players \u{2022} {s} slots"),
        None => "players".into(),
    };
    p.text(c, head, Role::Mono, 0.6, Tone::Fg3, &unit);
    // The columns: #, avatar, player, status, ping.
    let (cw_ix, cw_av, cw_st, cw_ping) = (1.6, p.lay("avatar"), p.lay("status_w"), p.lay("ping_w"));
    let cols = p.boxed(c, list, style!(row, gap: 0.8, pad: [0.0, 0.9, 0.45, 0.3]));
    for (w, h) in [(Some(cw_ix), "#"), (Some(cw_av), ""), (None, "Player"), (Some(cw_st), "Status"), (Some(cw_ping), "Ping")] {
        let mut st = style!(row).shrink(0.0);
        st = match w {
            Some(w) => st.w(w),
            None => st.grow(1.0),
        };
        if h == "Status" {
            st = st.justify(JustifyContent::Center);
        }
        let e = p.boxed(c, cols, st);
        p.text(c, e, Role::Mono, 0.5, Tone::Fg3, h);
    }
    // Your party first (you, then your friends), then the other parties,
    // each together, then everyone alone, as the server lists them
    // (humans by join, then bots).
    let order = list_order(&view.rows);
    let mine = view.rows.iter().find(|r| r.local).map(|r| r.party).filter(|p| *p != 0);
    let top_wins = view.rows.iter().map(|r| r.wins).max().unwrap_or(0);
    let row_h = p.lay("row_h");
    let mut last_party: Option<u8> = None;
    for (n, &i) in order.iter().enumerate() {
        let row = &view.rows[i];
        let you = row.local;
        // A party's rows join (no gap between them); groups stand apart.
        let party = (row.party != 0).then_some(row.party);
        let joined = n > 0 && party.is_some() && party == last_party;
        last_party = party;
        let r = c
            .spawn((
                LobbyPlayerRow { index: i, you },
                style!(row, gap: 0.8, pad: [0.0, 0.9, 0.0, 0.3], h: row_h, margin: [if n == 0 || joined { 0.0 } else { 0.45 }, 0.0, 0.0, 0.0])
                    .bg_a(Tone::Fg, if you { 0.06 } else { 0.025 })
                    .outline(Tone::Fg5, 1.0),
                Node::default(),
                ChildOf(list),
            ))
            .id();
        if let Some(pid) = party {
            // The party's edge: a thin solid colour down the rows' left
            // (yours cobalt, another party one of the avatar palette's).
            let colour = if Some(pid) == mine { theme.tone(Tone::Acc) } else { party_colour(pid) };
            c.spawn((
                PartyEdge(pid),
                Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, width: units.val(0.26), ..default() },
                BackgroundColor(colour),
                ChildOf(r),
            ));
        }
        let ix = p.boxed(c, r, style!(row, w: cw_ix).shrink(0.0).justify(JustifyContent::FlexEnd));
        p.text(c, ix, Role::Mono, 0.56, Tone::Fg3, &format!("{:02}", n + 1));
        let face = match chimera_ui::avatar::AvatarLook::from_byte(row.avatar) {
            _ if row.bot => chimera_ui::avatar::Face::Bot,
            Some(l) => chimera_ui::avatar::Face::Look(l),
            None => chimera_ui::avatar::Face::Look(chimera_ui::avatar::AvatarLook::for_name(&row.name)),
        };
        let av = chimera_ui::avatar::spawn(&ui, c, r, images, icons, face, chimera_ui::avatar::AvatarMarks { you, ..default() }, cw_av);
        if row.bot {
            c.entity(av).insert(LobbyMark::Bot);
        }
        let who = p.boxed(c, r, UiStyle { min_w: Some(0.0), ..style!(row, gap: 0.6).grow(1.0).shrink(1.0) });
        let name = p.cased(c, who, Role::DisplayBold, p.lay("name"), Tone::Fg, &row.name);
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
        // Status: the progress component (variant B).
        let stc = p.boxed(c, r, style!(row, w: cw_st).shrink(0.0).justify(JustifyContent::Center));
        let (prog, words) = row_progress(row);
        ui.progress(c, stc, prog, &words, cw_st - 0.7, 1.15, 0.56, true);
        // Ping: the bars and ms; a bot a dim meter and a dash; the loss
        // mark on a bad line.
        let pc = p.boxed(c, r, style!(row, w: cw_ping, gap: 0.45).shrink(0.0));
        match row.ping_ms {
            Some(ms) if !row.bot => {
                let level = PingLevel::from_ms(&theme, ms as u32);
                let meter = widgets::spawn_ping_meter(c, slants, &theme, &units, 0.97, level);
                c.entity(meter).insert(ChildOf(pc));
                p.text(c, pc, Role::Mono, 0.58, Tone::Fg2, &format!("{ms} ms"));
                if level.tone == Tone::Threat {
                    p.icon(c, pc, icons, images, "loss", 0.8, Tone::Threat);
                }
            }
            _ => {
                let meter = widgets::spawn_ping_meter(c, slants, &theme, &units, 0.97, PingLevel { lit: 0, tone: Tone::Fg4, alpha: 1.0 });
                c.entity(meter).insert(ChildOf(pc));
                p.text(c, pc, Role::Mono, 0.58, Tone::Fg4, "-");
            }
        }
    }
    // Free slots, up to the server's.
    let open = view.slots.map_or(0, |s| (s as usize).saturating_sub(view.rows.len()));
    let shown = open.min(8usize.saturating_sub(view.rows.len()).max(1));
    for k in 0..shown {
        let r = c
            .spawn((
                OpenSlot,
                style!(row, gap: 0.8, pad: [0.0, 0.9, 0.0, 0.3], h: row_h, margin: [0.45, 0.0, 0.0, 0.0]).outline(Tone::Fg5, 1.0),
                Node::default(),
                ChildOf(list),
            ))
            .id();
        let ix = p.boxed(c, r, style!(row, w: cw_ix).shrink(0.0).justify(JustifyContent::FlexEnd));
        p.text(c, ix, Role::Mono, 0.56, Tone::Fg4, &format!("{:02}", view.rows.len() + k + 1));
        p.boxed(c, r, style!(row, w: cw_av, h: cw_av).shrink(0.0).outline(Tone::Fg5, 1.0));
        let label = if shown < open && k + 1 == shown { format!("Open \u{2022} {open} slots") } else { "Open".into() };
        p.text(c, r, Role::DisplayBold, p.lay("name"), Tone::Fg4, &label);
    }

    // The map, up next: a big card (a gradient stands in for previews).
    let right = p.boxed(c, win, style!(column, w: map_w).abs([None, Some(1.6), Some(sh.rows_in), Some(body_bottom)]));
    p.label(c, right, "Up next");
    let (a, b) = map_tint(&view.map);
    let tile = c
        .spawn((
            style!(column, h: p.lay("card_h"), margin: [0.7, 0.0, 0.0, 0.0]),
            Node { overflow: Overflow::clip(), ..default() },
            BackgroundGradient::from(LinearGradient::degrees(160.0, vec![ColorStop::percent(a, 0.0), ColorStop::percent(b, 100.0)])),
            ChildOf(right),
        ))
        .id();
    c.spawn((
        Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::percent(Color::NONE, 55.0),
            ColorStop::percent(theme.tone_a(Tone::Ink, 0.75), 100.0),
        ])),
        ChildOf(tile),
    ));
    let pc = p.boxed(c, tile, style!(row).abs([Some(1.0), Some(1.0), None, None]));
    p.text(c, pc, Role::Mono, 0.5, Tone::Fg3, "Preview coming");
    let cap = p.boxed(c, tile, style!(row, gap: 0.8).abs([Some(1.0), None, None, Some(1.0)]).align(AlignItems::Baseline));
    p.text(c, cap, Role::DisplayBlack, p.lay("map_big"), Tone::Fg, &view.map);
    if !view.game.is_empty() {
        p.text(c, cap, Role::Mono, 0.6, Tone::Fg2, &view.game);
    }
    // Standby: the countdown and its ticks, once it runs.
    if let Some(left) = view.countdown {
        let cd = p.boxed(c, right, style!(column, margin: [1.2, 0.0, 0.0, 0.0]));
        let line = p.boxed(c, cd, style!(row, gap: 1.0).align(AlignItems::Baseline));
        p.text(c, line, Role::Mono, 0.6, Tone::Fg2, "Standby");
        p.text(c, line, Role::Mono, p.lay("big") * 0.8, Tone::Fg, &format!("{}:{:02}", left / 60, left % 60));
        let n = p.lay("countdown_ticks").max(1.0) as u32;
        let box_w = units.phys(units.cq(map_w)).floor() as u32;
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
        let well = p.boxed(c, cd, style!(row, h: p.lay("ticks_h"), margin: [0.7, 0.0, 0.0, 0.0]));
        c.spawn((
            CountdownBar { material: handle.clone(), ..bar },
            MaterialNode(handle),
            Node { width: px(layout.w as f32 / sf), height: px(layout.h as f32 / sf), ..default() },
            ChildOf(well),
        ));
    }

    // The action bar: the status sentence; Start, the host's hero. Nobody
    // readies up: loaded is ready.
    let bar = chimera_ui::shell::action_bar(&ui, c, win);
    let line = action_line(&view, loaded);
    let st = p.boxed(c, bar, style!(row).grow(1.0));
    ui.sentence(c, st, chimera_ui::controls::SENTENCE, Tone::Fg2, &line);
    if !view.hosting {
        // A player may leave before the match it didn't want starts.
        let leave = ui.button(c, bar, chimera_ui::controls::Kind::Secondary, "Leave", None, chimera_ui::controls::BUTTON_H, chimera_ui::controls::State::NONE);
        c.entity(leave).insert(LobbyAction::Leave);
    }
    if view.hosting && view.countdown.is_none() {
        // F3 (the lobby's key; Enter is the docked chat's).
        let key = super::prematch_lobby::LOBBY_KEY.to_uppercase();
        // Off, saying so, while the host's own map is still loading
        // (`LobbyView::start_waits_for`): no match starts before it is in.
        let waits = view.start_waits_for().is_some();
        let state = chimera_ui::controls::State { disabled: waits, ..chimera_ui::controls::State::NONE };
        let go = ui.button(c, bar, chimera_ui::controls::Kind::Primary, "Start", Some(&key), chimera_ui::controls::HERO_H, state);
        if !waits {
            c.entity(go).insert(LobbyAction::Ready);
        }
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

/// The list's order (module doc): your party, other parties together
/// (by their first member's place), then the rest, each in the server's
/// order.
pub fn list_order(rows: &[super::prematch_lobby::LobbyViewRow]) -> Vec<usize> {
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

/// A party's coloured edge on one of its rows (its party id).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartyEdge(pub u8);

/// A row's progress and its words (variant B: the word and the share
/// inside the bar).
fn row_progress(r: &super::prematch_lobby::LobbyViewRow) -> (chimera_ui::controls::Progress, String) {
    use chimera_ui::controls::Progress;
    match r.status {
        _ if r.bot => (Progress::Ready, "Loaded".into()),
        status::READY | status::NOT_READY => (Progress::Ready, "Loaded".into()),
        status::DOWNLOADING => match r.fraction {
            Some(f) if f > 0.0 => (Progress::Running(f), "Downloading".into()),
            _ => (Progress::Connecting, "Downloading".into()),
        },
        status::LOADING => match (r.step, r.fraction) {
            (Some(step), Some(f)) => (Progress::Running(f), step.short().into()),
            _ => (Progress::Connecting, "Loading".into()),
        },
        _ => (Progress::Connecting, "Connecting".into()),
    }
}

/// The action bar's sentence: how many have loaded and what starts the
/// match.
pub fn action_line(v: &LobbyView, loaded: usize) -> String {
    let n = v.rows.len();
    if let Some(left) = v.countdown {
        return format!("{loaded} of {n} loaded. The match starts in {left} s.");
    }
    if let Some(wait) = v.start_waits_for() {
        return format!("{loaded} of {n} loaded. {wait}; Start opens once it is in.");
    }
    if v.hosting && v.host_start {
        format!("{loaded} of {n} loaded. The match starts when you press Start.")
    } else if v.hosting {
        // A later lobby (between maps): it starts by itself; Start is early.
        format!("{loaded} of {n} loaded. It starts once everyone has loaded, or press Start.")
    } else if v.host_start {
        format!("{loaded} of {n} loaded. Waiting for the host to start.")
    } else {
        format!("{loaded} of {n} loaded. {}", v.line)
    }
}

/// A game's full name from its short one (the ladder's "Guns from").
fn full_game_name(short: &str) -> String {
    match short {
        "CS:S" => "Counter-Strike: Source".into(),
        "CA" => "Combat Arms".into(),
        "HL2:DM" => "Half-Life 2: Deathmatch".into(),
        other => other.into(),
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
