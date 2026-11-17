//! The one chat of an app with the Signal look (docs/plans/active/ugg-ui.md,
//! M10 "Chat"): every player line, team line, radio call, notice and
//! word of the server in one feed, drawn with the kit's chat
//! (`chimera_ui::chat`), in place of each game's own chat box and typing
//! line (`chat`'s, Combat Arms' panel) while the app's theme turns the
//! Signal HUD on (`signal::signal_on`) and the state is in
//! (`SignalChatActive`). The standalone games keep their own.
//!
//! - **Fed by** (`collect`): the chat's entries (`chat::ChatEntry`:
//!   players' lines heard from the server or said in a game of one, a
//!   bot's line, joins, leaves and renames as notices, the server's word
//!   and its refusals, "You are flooding the server"), the games' other
//!   lines (`chat::ChatLine`: objectives, team joins) as notices, Combat
//!   Arms' radio and lines (its `ui::talk`, as entries), the minigame's
//!   "X is on the last level!" (`MinigameHud`'s callout `last_rung_other`)
//!   and votes (`net::vote::VoteView`: called, passed, failed) as
//!   notices. Kills stay in the kill feed. Each line is printed to the
//!   console as well. A sender's tag: BOT for a bot (`core::Bot`, or a
//!   character with no owner), HOST for a listen server's host
//!   (`net::HOST_ID`). A name takes its colour from `chimera_ui::chat::
//!   name_color`, yours the accent's.
//! - **Shown**: `cl_chat` and the mutes (`chat::shows`: `ignore <name>`,
//!   or a click on a name in the open history) hide others' player lines,
//!   past ones too; your own always show. In play, bottom left above the
//!   health (`[layout.chat]`: `left`, `bottom`, `w`), the newest
//!   `lines` (6) for `hold` s (8.5), fading over `fade` s (1.0)
//!   (`chimera_ui::chat::line_alpha`, on the UI clock, as a `UiOpacity`:
//!   no rebuild); hidden under Settings and the lobby (a `SignalHud`).
//!   Docked in the lobby (under its root, `prematch_lobby::LobbyRoot`) and
//!   on the match end (`match_end_signal::SignalEndRoot`), where people
//!   wait: the history always open on its plate (`dock_*`), the field
//!   below it ("Press Enter to chat"); hidden while an app's overlay is
//!   over the lobby (`LobbyOverlay`).
//! - **Typing** (`typing`, before anything reads the keys): `messagemode`
//!   (Y) and `messagemode2` (U) open the field to everyone or the team
//!   while playing, and so do Combat Arms' Enter, F2 and F3 in an app
//!   with both (its `chat_open`, `chat_all`, `chat_team` hand over); in the
//!   lobby and on the match end Enter opens it too. The kit's field keys
//!   (`widgets::text_key`: the caret, selection, Ctrl+A, copy, cut,
//!   paste one line from `chimera_ui::clipboard::Clipboard`), Tab switches
//!   to the team and back, Enter sends (`chat::say`), Esc closes; at most
//!   127 bytes, control characters dropped. While it is open it takes
//!   every key: the binds, the menus and the game see none
//!   (`ButtonInput<KeyCode>` reset, the key messages drained), and the
//!   Escape menu and the cursor follow `chat::ChatInput` as before.
//! - **Sound**: `chat_in` (the app's UI sound table) for another's player
//!   line, at most one in `CUE_GAP` s; none for your own, notices or
//!   the server's word.
//!
//! `ui_scene chat [open|notice]` stages a feed of made-up lines in play
//! (open: the history and a typed line; notice: a notice newest), and
//! `ui_scene chat_lobby` the lobby's dock over a flat ground
//! (screenshots).
//!
//! Known shortcuts: the Tab board has no mute button yet (the board is
//! held open with the cursor captured): mute from the open history or
//! with `ignore`; a time stamp is the session's clock, not the wall's (no
//! local time zone without a date crate).

use std::collections::VecDeque;

use bevy::{
    input::{
        ButtonState, InputSystems,
        keyboard::KeyboardInput,
    },
    prelude::*,
    ui::DeprecatedInteraction as Interaction,
    window::CursorOptions,
};
use chimera_ui::{
    Theme, UiClock, UiCue,
    chat::{self, ChatName, FeedLine, LineKind, NameTag},
    controls::Ui,
};

use super::{
    chat::{ChatEntry, ChatFilter, ChatInput, ChatLine, ChatMutes, SignalChatActive, SenderInfo, sender_of, shows},
    signal::UiOpacity,
};
use crate::{
    console::{Console, Level},
    core::{LocalPlayer, Team},
    net::NetCharacter,
};

/// Lines kept (the history's).
pub const KEPT: usize = 60;
/// Another's line cues at most once in this many seconds.
pub const CUE_GAP: f64 = 1.5;
/// Lines the closed feed shows at most.
pub const LINES: usize = 6;
/// Lines the open history shows at most.
pub const HISTORY: usize = 12;
/// Text size, cq (typed text's, `chimera_ui::controls::TYPED`).
pub const SIZE: f32 = chimera_ui::controls::TYPED;

/// A line of the feed.
#[derive(Clone, Debug, PartialEq)]
pub struct FeedEntry {
    pub id: u64,
    pub kind: LineKind,
    pub sender: Option<Entity>,
    /// The sender's user id (None: a bot or no sender).
    pub owner: Option<u64>,
    pub own: bool,
    pub name: Option<String>,
    pub tag: Option<NameTag>,
    pub team: Option<Team>,
    pub text: String,
    /// When it came (the UI clock's seconds).
    pub at: f64,
}

impl FeedEntry {
    /// As `chat::shows` sees it.
    fn entry(&self) -> ChatEntry {
        ChatEntry { kind: self.kind, sender: self.sender, name: self.name.clone(), team: self.team, text: String::new(), classic: None, sound: None }
    }
}

/// The feed (module doc).
#[derive(Resource, Clone, Debug, Default)]
pub struct SignalChatLog {
    pub lines: VecDeque<FeedEntry>,
    next: u64,
    /// When another's line last cued (the UI clock).
    last_cue: Option<f64>,
    /// Lines staged by `ui_scene chat`: they don't fade.
    pub staged: bool,
}

impl SignalChatLog {
    /// Adds a line; the oldest goes past `KEPT`.
    pub fn push(&mut self, mut e: FeedEntry) -> u64 {
        self.next += 1;
        e.id = self.next;
        self.lines.push_back(e);
        while self.lines.len() > KEPT {
            self.lines.pop_front();
        }
        self.next
    }

    /// The lines this client shows (`cl_chat`, the mutes), oldest first.
    pub fn shown<'a>(&'a self, mutes: &'a ChatMutes, filter: ChatFilter) -> impl Iterator<Item = &'a FeedEntry> + 'a {
        self.lines.iter().filter(move |l| shows(&l.entry(), l.own, l.owner, mutes, filter))
    }
}

/// The feed's timing (`[layout.chat]` `hold` and `fade`, s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub hold: f32,
    pub fade: f32,
}

impl Timing {
    pub fn of(theme: Option<&Theme>) -> Self {
        let get = |k: &str, or: f32| theme.and_then(|t| t.layout_opt("chat", k)).unwrap_or(or);
        Timing { hold: get("hold", 8.5), fade: get("fade", chat::FADE) }
    }

    /// A line's opacity `age` s after it came.
    pub fn alpha(&self, age: f32) -> f32 {
        chat::line_alpha(age, self.hold, self.fade)
    }
}

/// The feed's state without drawing (tests, and under `SignalChatPlugin`).
pub struct SignalChatStatePlugin;

impl Plugin for SignalChatStatePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<super::chat::GameMessagesPlugin>() {
            app.add_plugins(super::chat::GameMessagesPlugin);
        }
        if !app.world().contains_resource::<Messages<UiCue>>() {
            app.add_message::<UiCue>();
        }
        app.init_resource::<SignalChatLog>()
            .init_resource::<SignalChatActive>()
            .init_resource::<ChatInput>()
            .init_resource::<UiClock>()
            .add_systems(First, sync_active)
            .add_systems(Update, collect.after(super::chat::route_classic));
    }
}

/// The Signal chat drawn and typed into (`ChatPlugin` adds it).
pub struct SignalChatPlugin;

impl Plugin for SignalChatPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<SignalChatStatePlugin>() {
            app.add_plugins(SignalChatStatePlugin);
        }
        app.init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .add_systems(PreUpdate, typing.after(InputSystems).run_if(active))
            .add_systems(Update, (draw, fade_lines, mute_clicks).chain().after(collect));
        super::signal::setup(app);
        super::overlay::layer(app);
        let scene: super::ui_scene::SceneHook = stage_scene;
        super::ui_scene::UiSceneHooks::add(app.world_mut(), "chat", scene);
        let lobby: super::ui_scene::SceneHook = stage_lobby_scene;
        super::ui_scene::UiSceneHooks::add(app.world_mut(), "chat_lobby", lobby);
    }
}

/// The Signal chat owns the chat.
pub fn active(a: Option<Res<SignalChatActive>>) -> bool {
    a.is_some_and(|a| a.0)
}

/// `SignalChatActive` from the theme (`flags.hud.signal`).
fn sync_active(theme: Option<Res<Theme>>, mut a: ResMut<SignalChatActive>) {
    let on = super::signal::signal_on(theme.as_deref());
    if a.0 != on {
        a.0 = on;
    }
}

/// Words for a vote's issue.
fn issue_words(issue: &str, detail: &str) -> String {
    let what = match issue {
        "skipmap" => "skip the map".to_string(),
        "kick" => format!("kick {detail}"),
        "changelevel" => format!("change the map to {detail}"),
        "nextlevel" => format!("play {detail} next"),
        "restartgame" => "restart the game".to_string(),
        other if detail.is_empty() => other.to_string(),
        other => format!("{other} {detail}"),
    };
    what.trim().to_string()
}

/// Everything the feed shows (module doc).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn collect(
    mut entries: MessageReader<ChatEntry>,
    mut lines: MessageReader<ChatLine>,
    on: Res<SignalChatActive>,
    mut log: ResMut<SignalChatLog>,
    clock: Res<UiClock>,
    (mutes, filter): (Res<ChatMutes>, Res<ChatFilter>),
    who: Query<SenderInfo>,
    marks: Query<(Has<crate::core::Bot>, Option<&NetCharacter>)>,
    (hud, votes): (Option<Res<chimera_sim::minigame::hud::MinigameHud>>, Option<Res<crate::net::vote::VoteView>>),
    names: Query<(Option<&NetCharacter>, Option<&Name>, Has<LocalPlayer>)>,
    settings: Option<Res<crate::net::NetSettings>>,
    mut seen: Local<(u32, Option<u32>, bool)>,
    (mut console, mut cues): (Option<ResMut<Console>>, MessageWriter<UiCue>),
) {
    if !on.0 {
        entries.clear();
        lines.clear();
        return;
    }
    let now = clock.now;
    let add = |log: &mut SignalChatLog, e: FeedEntry, console: &mut Option<ResMut<Console>>| {
        if let Some(c) = console.as_mut() {
            let line = match (&e.name, e.kind) {
                (Some(n), LineKind::Team) => format!("(team) {n}: {}", e.text),
                (Some(n), LineKind::Radio) => format!("(radio) {n}: {}", e.text),
                (Some(n), _) => format!("{n}: {}", e.text),
                (None, _) => e.text.clone(),
            };
            c.print(Level::Info, line);
        }
        log.push(e);
    };
    for e in entries.read() {
        let (own, owner) = sender_of(e, &who);
        let tag = e.sender.and_then(|s| marks.get(s).ok()).and_then(|(bot, net)| match (bot, net) {
            (true, _) | (false, Some(NetCharacter { owner: None, .. })) => Some(NameTag::Bot),
            (false, Some(NetCharacter { owner: Some(id), .. })) if *id == crate::net::HOST_ID => Some(NameTag::Host),
            _ => None,
        });
        let entry = FeedEntry { id: 0, kind: e.kind, sender: e.sender, owner, own, name: e.name.clone(), tag, team: e.team, text: e.text.clone(), at: now };
        // Another's line, shown here: its cue, at most one a gap.
        if e.kind.player() && !own && shows(e, own, owner, &mutes, *filter) && log.last_cue.is_none_or(|t| now - t >= CUE_GAP) {
            log.last_cue = Some(now);
            cues.write(UiCue::new("chat_in"));
        }
        add(&mut log, entry, &mut console);
    }
    for l in lines.read() {
        let text: String = l.0.iter().map(|(_, s)| s.as_str()).collect::<String>().trim().to_string();
        if text.is_empty() {
            continue;
        }
        add(&mut log, FeedEntry { id: 0, kind: LineKind::Notice, sender: None, owner: None, own: false, name: None, tag: None, team: l.1, text, at: now }, &mut console);
    }
    // Another player on the last level (the callout's), once a callout.
    if let Some(shown) = hud.as_ref().and_then(|h| h.callout.as_ref())
        && shown.id != seen.0
    {
        seen.0 = shown.id;
        if shown.callout.cue == Some("last_rung_other")
            && let Some(first) = shown.callout.lines.first()
        {
            let player = |e: Entity| {
                let (net, name, local) = names.get(e).unwrap_or((None, None, false));
                super::shown_name(e, local, net, name, settings.as_deref())
            };
            let text = super::callouts::fill(first, player, |w| w.to_string());
            add(&mut log, FeedEntry { id: 0, kind: LineKind::Notice, sender: None, owner: None, own: false, name: None, tag: None, team: None, text, at: now }, &mut console);
        }
    }
    // Votes: called, then how they ended.
    if let Some(v) = votes {
        let id = v.active.as_ref().map(|a| a.id);
        if id.is_some() && id != seen.1 {
            let a = v.active.as_ref().expect("checked");
            let text = format!("{} called a vote to {}", a.caller, issue_words(&a.issue, &a.detail));
            add(&mut log, FeedEntry { id: 0, kind: LineKind::Notice, sender: None, owner: None, own: false, name: None, tag: None, team: None, text, at: now }, &mut console);
        }
        seen.1 = id.or(seen.1);
        let ended = v.ended.is_some();
        if ended && !seen.2
            && let Some((crate::net::vote::VoteNews::Ended { issue, detail, failure, .. }, _)) = &v.ended
        {
            let what = issue_words(issue, detail);
            let text = match failure {
                None => format!("The vote to {what} passed"),
                Some(crate::net::vote::VoteFailure::Quorum) => format!("The vote to {what} failed: not enough players voted"),
                Some(crate::net::vote::VoteFailure::YesNo) => format!("The vote to {what} failed: not enough voted yes"),
            };
            add(&mut log, FeedEntry { id: 0, kind: LineKind::Notice, sender: None, owner: None, own: false, name: None, tag: None, team: None, text, at: now }, &mut console);
        }
        seen.2 = ended;
    }
}

/// Where the chat is docked (where people wait), else in play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dock {
    Play,
    Lobby(Entity),
    MatchEnd(Entity),
}

impl Dock {
    pub fn docked(self) -> bool {
        self != Dock::Play
    }
}

/// Where the chat is now: the lobby's root while the themed lobby shows
/// and nothing is over it, the match end's while it shows, else play;
/// None: nowhere (an overlay over the lobby).
#[allow(clippy::type_complexity)]
fn dock_now(
    lobby: Option<&super::prematch_lobby::LobbyView>,
    overlay: Option<&super::prematch_lobby::LobbyOverlay>,
    end: Option<&super::match_end::MatchEndView>,
    lobby_root: Option<Entity>,
    end_root: Option<Entity>,
) -> Option<Dock> {
    if lobby.is_some_and(|l| l.shown) {
        if overlay.is_some_and(|o| o.0) {
            return None;
        }
        return Some(lobby_root.map_or(Dock::Play, Dock::Lobby));
    }
    if end.is_some_and(|e| e.shown)
        && let Some(r) = end_root
    {
        return Some(Dock::MatchEnd(r));
    }
    Some(Dock::Play)
}

/// The keys (module doc). Runs before anything else reads them.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn typing(
    mut input: ResMut<ChatInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut events: ResMut<Messages<KeyboardInput>>,
    cursor: Query<&CursorOptions>,
    console: Option<Res<super::console::ConsoleUi>>,
    menu: Option<Res<super::front_end::MenuStatus>>,
    (binds, mouse): (Option<Res<Console>>, Option<Res<ButtonInput<MouseButton>>>),
    (lobby, overlay, end): (
        Option<Res<super::prematch_lobby::LobbyView>>,
        Option<Res<super::prematch_lobby::LobbyOverlay>>,
        Option<Res<super::match_end::MatchEndView>>,
    ),
    mut clipboard: Option<ResMut<chimera_ui::clipboard::Clipboard>>,
    mut commands: Commands,
) {
    if console.as_ref().is_some_and(|c| c.open) {
        return;
    }
    let docked = matches!(dock_now(lobby.as_deref(), overlay.as_deref(), end.as_deref(), Some(Entity::PLACEHOLDER), Some(Entity::PLACEHOLDER)), Some(d) if d.docked());
    if input.open.is_none() {
        let menu_open = menu.is_some_and(|m| m.open);
        let playing = cursor.iter().next().is_some_and(super::input::cursor_grabbed) && !menu_open;
        let pressed = |c: &str| match (&binds, &mouse) {
            (Some(b), Some(m)) => super::binds::just_pressed(&b.binds, &keys, m, c),
            _ => false,
        };
        let (all, team) = (pressed("messagemode"), pressed("messagemode2"));
        let enter = docked && (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter));
        if (playing && (all || team)) || (docked && (all || team || enter)) {
            *input = ChatInput { open: Some(team), ..default() };
            keys.reset_all();
            // The opening key isn't typed.
            events.clear();
        }
        return;
    }
    let (ctrl, shift) = (
        keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight),
        keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
    );
    let mut memory = None;
    let board = match clipboard.as_deref_mut() {
        Some(c) => c,
        None => memory.insert(chimera_ui::clipboard::Clipboard::memory(None)),
    };
    let typed: Vec<KeyboardInput> = events.drain().collect();
    for e in typed {
        if e.state != ButtonState::Pressed {
            continue;
        }
        match e.key_code {
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let text = std::mem::take(&mut input.text);
                let team = input.open.take() == Some(true);
                input.caret = default();
                commands.queue(move |w: &mut World| super::chat::say(w, &text, team));
                break;
            }
            KeyCode::Escape => {
                *input = ChatInput::default();
                break;
            }
            KeyCode::Tab => {
                input.open = input.open.map(|t| !t);
                continue;
            }
            _ => {}
        }
        if let Some(k) = super::widgets::text_key(&e.logical_key, e.key_code, ctrl, shift) {
            let ChatInput { text, caret, .. } = &mut *input;
            caret.key(text, k, board, chat::MAX_CHARS, false);
        }
    }
    // The game, the binds and the menus see no key while it is open, nor
    // on the frame it closed.
    keys.reset_all();
}

/// The chat's root in play.
#[derive(Component)]
#[require(super::hud::SignalHud)]
pub struct ChatPlayRoot;

/// The chat's root docked (the lobby, the match end).
#[derive(Component)]
pub struct ChatDockRoot;

/// A drawn line: which (`FeedEntry::id`) and whether it fades (closed in
/// play).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeedLineId {
    pub id: u64,
    pub fades: bool,
}

/// What the chat was last built for.
#[derive(Clone, Debug, PartialEq)]
struct Built {
    dock: Option<Dock>,
    ids: Vec<u64>,
    input: ChatInput,
    mutes: ChatMutes,
    units: (f32, f32),
}

/// The chat drawn (module doc): rebuilt when what shows changes.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    look: super::signal::Look,
    on: Res<SignalChatActive>,
    log: Res<SignalChatLog>,
    input: Res<ChatInput>,
    (mutes, filter): (Res<ChatMutes>, Res<ChatFilter>),
    clock: Res<UiClock>,
    layer: Option<Res<super::overlay::OverlayLayer>>,
    (lobby, overlay, end): (
        Option<Res<super::prematch_lobby::LobbyView>>,
        Option<Res<super::prematch_lobby::LobbyOverlay>>,
        Option<Res<super::match_end::MatchEndView>>,
    ),
    lobby_root: Query<Entity, With<super::prematch_lobby::LobbyRoot>>,
    end_root: Query<Entity, With<super::match_end_signal::SignalEndRoot>>,
    roots: Query<Entity, Or<(With<ChatPlayRoot>, With<ChatDockRoot>)>>,
    mut built: Local<Option<Built>>,
    mut commands: Commands,
) {
    let dock = if on.0 && look.on() {
        dock_now(lobby.as_deref(), overlay.as_deref(), end.as_deref(), lobby_root.iter().next(), end_root.iter().next())
    } else {
        None
    };
    let t = Timing::of(Some(&look.theme));
    let open = input.open.is_some();
    let docked = dock.is_some_and(|d| d.docked());
    // The lines this build shows.
    let shown: Vec<&FeedEntry> = log.shown(&mutes, *filter).collect();
    let ids: Vec<u64> = if open || docked {
        shown.iter().rev().take(if docked && !open { LINES } else { HISTORY }).map(|l| l.id).collect()
    } else {
        shown
            .iter()
            .rev()
            .filter(|l| log.staged || t.alpha((clock.now - l.at) as f32) > 0.0)
            .take(LINES)
            .map(|l| l.id)
            .collect()
    };
    let key = Built {
        dock,
        ids: ids.clone(),
        input: input.clone(),
        mutes: mutes.clone(),
        units: (look.units.height, look.units.scale_factor),
    };
    if built.as_ref() == Some(&key) && (dock.is_none() || !roots.is_empty()) {
        return;
    }
    *built = Some(key);
    for e in &roots {
        commands.entity(e).despawn();
    }
    let Some(dock) = dock else { return };
    if !docked && ids.is_empty() && !open {
        return;
    }
    let get = |k: &str, or: f32| look.layout("chat", k, or);
    let root = match dock {
        Dock::Play => {
            let Some(layer) = layer else { return };
            commands
                .spawn((
                    ChatPlayRoot,
                    Name::new("Signal chat"),
                    ChildOf(layer.0),
                    GlobalZIndex(46),
                    Pickable::IGNORE,
                    Node {
                        position_type: PositionType::Absolute,
                        left: look.val(get("left", 2.2)),
                        bottom: look.val(get("bottom", 8.4)),
                        width: look.val(get("w", 30.0)),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::FlexEnd,
                        row_gap: look.val(0.18),
                        ..default()
                    },
                ))
                .id()
        }
        Dock::Lobby(parent) | Dock::MatchEnd(parent) => {
            let lobby = matches!(dock, Dock::Lobby(_));
            let k = |key: &str, or: f32| get(&format!("{}_{key}", if lobby { "dock" } else { "end" }), or);
            commands
                .spawn((
                    ChatDockRoot,
                    Name::new("Signal chat (docked)"),
                    ChildOf(parent),
                    ZIndex(20),
                    Node {
                        position_type: PositionType::Absolute,
                        left: look.val(k("left", if lobby { 4.6 } else { 4.6 })),
                        bottom: look.val(k("bottom", if lobby { 7.0 } else { 8.0 })),
                        width: look.val(k("w", if lobby { 34.0 } else { 30.0 })),
                        min_height: look.val(k("h", if lobby { 11.0 } else { 9.0 })),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::FlexEnd,
                        row_gap: look.val(0.18),
                        padding: UiRect::all(look.val(0.5)),
                        ..default()
                    },
                ))
                .id()
        }
    };
    let ui = Ui::new(&look.theme, look.fonts.as_deref(), *look.units);
    let history = open || docked;
    if history {
        ui.chat_plate(&mut commands, root);
    }
    let by_id = |id: u64| shown.iter().find(|l| l.id == id).copied();
    for id in ids.iter().rev() {
        let Some(l) = by_id(*id) else { continue };
        let color = if l.own { look.tone(chimera_ui::Tone::AccText) } else { chat::name_color(l.name.as_deref().unwrap_or_default()) };
        let line = FeedLine {
            kind: l.kind,
            name: l.name.as_deref(),
            name_color: color,
            tag: l.tag,
            text: &l.text,
            stamp: history.then(|| chat::stamp(l.at)),
            muted: false,
        };
        let row = ui.chat_line(&mut commands, root, &line, SIZE, !history);
        commands.entity(row).insert(FeedLineId { id: l.id, fades: !history && !log.staged });
    }
    if open || docked {
        let team = input.open == Some(true);
        let cursor = input.open.map(|_| input.caret.cursor(&input.text));
        let hint = if docked { "Press Enter to chat" } else { "" };
        ui.chat_field(&mut commands, root, team, &input.text, cursor, hint, SIZE);
    }
}

/// Lines fade on the UI clock (`Timing`), without a rebuild.
fn fade_lines(look: super::signal::Look, log: Res<SignalChatLog>, clock: Res<UiClock>, mut rows: Query<(Entity, &FeedLineId, Option<&mut UiOpacity>)>, mut commands: Commands) {
    let t = Timing::of(Some(&look.theme));
    for (e, row, op) in &mut rows {
        if !row.fades {
            continue;
        }
        let Some(l) = log.lines.iter().find(|l| l.id == row.id) else { continue };
        let a = t.alpha((clock.now - l.at) as f32);
        match op {
            Some(mut o) => {
                if o.0 != a {
                    o.0 = a;
                }
            }
            None => {
                commands.entity(e).insert(UiOpacity(a));
            }
        }
    }
}

/// A click on a name in the open history mutes that player, or unmutes.
#[allow(clippy::type_complexity)]
fn mute_clicks(
    input: Res<ChatInput>,
    log: Res<SignalChatLog>,
    names: Query<(Entity, Option<&Interaction>), With<ChatName>>,
    rows: Query<&FeedLineId>,
    clicks: Query<(&Interaction, &ChatName, &ChildOf), Changed<Interaction>>,
    mut mutes: ResMut<ChatMutes>,
    mut commands: Commands,
) {
    // Clickable only while the history is open (the cursor is free).
    for (e, interaction) in &names {
        if interaction.is_none() && input.open.is_some() {
            commands.entity(e).insert(chimera_ui::controls::clickable());
        }
    }
    for (i, name, parent) in &clicks {
        if *i != Interaction::Pressed {
            continue;
        }
        let Ok(row) = rows.get(parent.parent()) else { continue };
        let Some(l) = log.lines.iter().find(|l| l.id == row.id) else { continue };
        if l.own || l.sender.is_none() {
            continue;
        }
        let on = !mutes.muted(l.owner, &name.0);
        mutes.set(l.owner, &name.0, on);
    }
}

/// `ui_scene chat [open|notice]` (module doc).
fn stage_scene(w: &mut World, args: &[String]) -> Result<(), String> {
    let mode = args.first().map(String::as_str);
    if mode == Some("off") {
        w.insert_resource(SignalChatLog::default());
        *w.resource_mut::<ChatInput>() = ChatInput::default();
        return Ok(());
    }
    stage_lines(w, mode == Some("notice"));
    if mode == Some("open") {
        let text = "gg, one more on de_dust2?".to_string();
        let n = text.chars().count();
        *w.resource_mut::<ChatInput>() = ChatInput { open: Some(false), text, caret: super::widgets::Caret { at: n, anchor: n } };
    }
    Ok(())
}

/// A made-up lobby's entity (`ui_scene chat_lobby`).
#[derive(Component)]
struct StagedLobby;

/// `ui_scene chat_lobby`: the lines in the lobby's dock, over a made-up
/// lobby (a `NetLobby` of four players, as a server would replicate it).
fn stage_lobby_scene(w: &mut World, args: &[String]) -> Result<(), String> {
    let old: Vec<Entity> = w.query_filtered::<Entity, With<StagedLobby>>().iter(w).collect();
    for e in old {
        w.despawn(e);
    }
    if args.first().map(String::as_str) == Some("off") {
        w.insert_resource(SignalChatLog::default());
        return Ok(());
    }
    stage_lines(w, true);
    use chimera_net::prematch::{LobbyRow, NetLobby, status};
    let row = |id: Option<u64>, name: &str, s: u8, ping: Option<u16>| LobbyRow {
        id,
        name: name.into(),
        status: s,
        done: 0,
        total: 0,
        step: chimera_net::prematch::NO_STEP,
        permille: 0,
        ping_ms: ping,
        wins: 0,
        avatar: if id.is_some() { (name.len() as u8 * 5) % 32 } else { chimera_net::prematch::NO_AVATAR },
        party: 0,
    };
    w.spawn((
        StagedLobby,
        NetLobby {
            map: "cs_source:de_dust2".into(),
            mode: "Ultimate Gun Game".into(),
            rows: vec![
                row(Some(crate::net::HOST_ID), "kestrel", status::READY, None),
                row(Some(2), "nadia.exe", status::READY, Some(38)),
                row(Some(3), "pixelpirate", status::NOT_READY, Some(64)),
                row(None, "Sgt. Biscuit", status::READY, None),
            ],
            waiting: 1,
            wait_left: Some(24),
            countdown: None,
            host_start: false,
        },
    ));
    Ok(())
}

/// Made-up lines, newest last (a notice newest with `notice`).
fn stage_lines(w: &mut World, notice: bool) {
    let now = w.get_resource::<UiClock>().map_or(0.0, |c| c.now);
    let mut log = SignalChatLog { staged: true, ..default() };
    let line = |kind, name: Option<&str>, tag, own, text: &str, ago: f64| FeedEntry {
        id: 0,
        kind,
        sender: name.map(|_| Entity::PLACEHOLDER),
        owner: None,
        own,
        name: name.map(str::to_string),
        tag,
        team: None,
        text: text.into(),
        at: now - ago,
    };
    for l in [
        line(LineKind::Notice, None, None, false, "pixelpirate joined the game", 62.0),
        line(LineKind::All, Some("kestrel"), Some(NameTag::Host), false, "welcome! first to the knife wins", 48.0),
        line(LineKind::All, Some("Sgt. Biscuit"), Some(NameTag::Bot), false, "Gotcha, friend.", 31.0),
        line(LineKind::All, Some("nadia.exe"), None, false, "who put the AR2 on rung 3 lol", 12.0),
        line(LineKind::All, Some("You"), None, true, "it's random every match", 6.0),
        line(LineKind::System, None, None, false, "You are flooding the server: wait a moment.", 3.0),
    ] {
        log.push(l);
    }
    if notice {
        log.push(line(LineKind::Notice, None, None, false, "nadia.exe is on the last level!", 0.5));
    }
    w.insert_resource(log);
}
