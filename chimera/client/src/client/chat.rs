//! The chat area (radio calls and game messages) and the hint text, laid
//! out like the game's own `HudChat` and `HudHintDisplay` panels when the
//! map has a HUD (`map::hud::ActiveHud`), else at CS:S's places. Anything
//! can add a line with `ChatLine` or a hint with `Hint`; lines are also
//! printed to the console.
//!
//! Text chat: Y (`messagemode`) and U (`messagemode2`) open a typing line
//! ("Say :", "Say (TEAM) :"), Enter sends it as `say` / `say_team` would,
//! Escape drops it; while it's open the keyboard types and plays nothing
//! else. Lines take the game's formats (`map::radio::SayFormats`) and
//! play its chat sound. Computer players don't chat.
//!
//! The chat area (`ChatLine` messages: coloured runs, also printed to the
//! console; 12 s per line, `hud_saytext_time`) in the HUD's `HudChat` box, and
//! hint text (`Hint` messages: buy refusals such as "You cannot carry any
//! more.") in its `HudHintDisplay` box, coloured from the scheme (a name's
//! team colour is the scheme's for CS:S and HL2:DM's own in its look:
//! Combine light blue, Rebels red, anyone else amber); text chat: Y
//! / U (`messagemode`, `messagemode2`) open a typing line ("Say :", "Say (TEAM)
//! :") that takes the keyboard (the game sees no keys) until Enter or Esc, sent
//! as `say` / `say_team` would (the game's `SayFormats`, the sender's nav place
//! in team chat, its chat sound); a line when a character gets or changes its
//! team ("Bot 2 is joining the Terrorist force", `SayFormats::joins`; not
//! while a recording plays back, `core::Playback`;
//! `GameMessagesPlugin`, also headless, with `say`/`say_team`); in a network
//! game `say` asks the server (`net::SayRequest`) and the lines it sends
//! (`net::ChatMessage`) are formatted here, its refusals (`net::Notice`) shown
//! as hints.
//!
//! Entries (`ChatEntry`): what is said and by whom (players' lines heard
//! or said here, a bot's, joins, leaves and renames as notices, the
//! server's word), each with its line in the game's format; the game's
//! chat shows that line (`route_classic`: a `ChatLine`, with its sound)
//! unless the Signal chat owns the chat (`SignalChatActive`:
//! `signal_chat`, the Ultimate Gun Game's look), which draws its own and
//! stands this box, its typing line and the refusals' hints down. Both
//! honour `cl_chat` (`ChatFilter`: 1 everyone's lines, 2 the team's only,
//! 0 none; notices and your own always) and the mutes (`ChatMutes`:
//! `ignore <name>`, `unignore`, `ignorelist`; people by user id, bots by
//! name, for the session) through `shows`.

use std::collections::VecDeque;

use bevy::{
    input::{
        ButtonState, InputSystems,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    window::CursorOptions,
};

use super::fonts::UiFonts;
/// What a chat entry is (the kit's, for games without the kit).
pub use chimera_ui::chat::LineKind;
use crate::{
    console::{Console, ConsoleAppExt, Level},
    core::{Health, LocalPlayer, Team},
    map::hud::{ActiveHud, HudCoord, HudPanel},
    map::radio::ChatColor,
};

pub struct ChatPlugin;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<GameMessagesPlugin>() {
            app.add_plugins(GameMessagesPlugin);
        }
        app.add_plugins(super::signal_chat::SignalChatPlugin)
            .add_message::<Hint>()
            .init_resource::<ChatLog>()
            .init_resource::<HintState>()
            .init_resource::<ChatInput>()
            // The game's own chat line and box, unless the Signal chat
            // owns the chat (`signal_chat`).
            .add_systems(PreUpdate, typing.after(InputSystems).run_if(classic))
            .add_systems(Update, ((collect, (draw_chat, draw_input).run_if(classic)).chain(), draw_hint));
        super::text_warm::TextWarmAppExt::warm_texts(app, warm_fonts);
        crate::console::resource_cvar::<ChatLog, f32>(
            app,
            "hud_saytext_time",
            "Seconds a chat line stays on screen.",
            |l| &mut l.seconds,
        );
        if let Some(mut console) = app.world_mut().get_resource_mut::<crate::console::Console>() {
            console.archive("hud_saytext_time");
        }
    }
}

/// Open the typing line (to the team: `team`), empty.
pub fn open_input(w: &mut World, team: bool) {
    *w.get_resource_or_init::<ChatInput>() = ChatInput { open: Some(team), ..default() };
}

/// The game's own chat draws (no Signal chat: `SignalChatActive`).
pub fn classic(active: Option<Res<SignalChatActive>>) -> bool {
    !active.is_some_and(|a| a.0)
}

/// The Signal chat owns the chat (`signal_chat`: the app's theme turns
/// the Signal HUD on and the chat's state is in): the entries
/// (`ChatEntry`) go to it, not to the game's lines (`ChatLine`), and the
/// game's chat box and typing line stand down. A game's own chat (Combat
/// Arms') reads it too.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SignalChatActive(pub bool);

/// A chat entry with what it is (`chimera_ui::chat::LineKind`) and who
/// said it: players' lines (heard from the server, a game of one's own,
/// a bot's), notices (joins, leaves, renames) and the server's word.
/// The game's own chat shows its `classic` line (`route_classic`: as a
/// `ChatLine`, with its `sound`); the Signal chat its own
/// (`signal_chat`).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct ChatEntry {
    pub kind: chimera_ui::chat::LineKind,
    pub sender: Option<Entity>,
    pub name: Option<String>,
    pub team: Option<Team>,
    pub text: String,
    /// How the game's own chat shows it.
    pub classic: Option<ChatLine>,
    /// The game's chat sound with it.
    pub sound: Option<String>,
}

impl ChatEntry {
    /// A notice (`text` a sentence), shown as `classic` in the game's chat.
    pub fn notice(text: impl Into<String>, classic: Option<ChatLine>) -> Self {
        Self { kind: chimera_ui::chat::LineKind::Notice, sender: None, name: None, team: None, text: text.into(), classic, sound: None }
    }
}

/// The players whose lines this client hides (`ignore <name>`, a click on
/// a name in the open chat): people by user id (`NetCharacter::owner`,
/// kept through map changes and new characters), bots by name. For the
/// session, not saved (as `net::voice::VoiceMutes`).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ChatMutes {
    pub users: std::collections::BTreeSet<u64>,
    pub names: std::collections::BTreeSet<String>,
}

impl ChatMutes {
    /// Whether a sender with this user id (None: a bot) and name is muted.
    pub fn muted(&self, owner: Option<u64>, name: &str) -> bool {
        match owner {
            Some(id) => self.users.contains(&id),
            None => self.names.contains(&name.to_lowercase()),
        }
    }

    /// Mute (`on`) or unmute; whether it changed.
    pub fn set(&mut self, owner: Option<u64>, name: &str, on: bool) -> bool {
        match (owner, on) {
            (Some(id), true) => self.users.insert(id),
            (Some(id), false) => self.users.remove(&id),
            (None, true) => self.names.insert(name.to_lowercase()),
            (None, false) => self.names.remove(&name.to_lowercase()),
        }
    }
}

/// `cl_chat`: whose lines this client shows (1 everyone's, 2 the team's
/// only, 0 none but its own); notices and the server's word always.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChatFilter(pub u8);

impl Default for ChatFilter {
    fn default() -> Self {
        Self(1)
    }
}

/// Whether this client shows `e` (`own`: the local player said it;
/// `owner`: the sender's user id, None for a bot): `cl_chat`, then the
/// mutes. Your own lines always show.
pub fn shows(e: &ChatEntry, own: bool, owner: Option<u64>, mutes: &ChatMutes, filter: ChatFilter) -> bool {
    use chimera_ui::chat::LineKind;
    if own || !e.kind.player() {
        return true;
    }
    let by_filter = match filter.0 {
        0 => false,
        2 => e.kind == LineKind::Team,
        _ => true,
    };
    by_filter && !(e.sender.is_some() && mutes.muted(owner, e.name.as_deref().unwrap_or_default()))
}

/// The sender's user id and whether it is the local player.
pub type SenderInfo<'a> = (Option<&'a crate::net::NetCharacter>, Has<LocalPlayer>);

/// `(own, owner)` of an entry's sender.
pub fn sender_of(e: &ChatEntry, who: &Query<SenderInfo>) -> (bool, Option<u64>) {
    match e.sender.and_then(|s| who.get(s).ok()) {
        Some((net, local)) => (local, net.and_then(|n| n.owner)),
        None => (false, None),
    }
}

/// The entries in the game's own chat (no Signal chat): each its classic
/// line and sound, unless muted or filtered out (`shows`).
#[allow(clippy::too_many_arguments)]
pub fn route_classic(
    mut entries: MessageReader<ChatEntry>,
    active: Option<Res<SignalChatActive>>,
    mutes: Option<Res<ChatMutes>>,
    filter: Option<Res<ChatFilter>>,
    who: Query<SenderInfo>,
    mut chat: MessageWriter<ChatLine>,
    mut play: MessageWriter<crate::map::PlaySound>,
) {
    if active.is_some_and(|a| a.0) {
        entries.clear();
        return;
    }
    let mutes = mutes.as_deref().cloned().unwrap_or_default();
    let filter = filter.as_deref().copied().unwrap_or_default();
    for e in entries.read() {
        let (own, owner) = sender_of(e, &who);
        if !shows(e, own, owner, &mutes, filter) {
            continue;
        }
        if let Some(line) = &e.classic {
            chat.write(line.clone());
        }
        if let Some(s) = &e.sound {
            play.write(crate::map::PlaySound::ui(s.clone()));
        }
    }
}

/// The player named `name` (any case; a unique start of a name will do):
/// their user id (None: a bot) and their name.
fn find_player(w: &mut World, name: &str) -> Result<(Option<u64>, String), String> {
    let want = name.to_lowercase();
    let mut all: Vec<(Option<u64>, String)> = w
        .query::<(&crate::net::NetCharacter, Has<LocalPlayer>)>()
        .iter(w)
        .filter(|(_, local)| !local)
        .map(|(c, _)| (c.owner, c.name.clone()))
        .collect();
    all.extend(
        w.query_filtered::<&Name, (With<crate::core::Bot>, Without<crate::net::NetCharacter>)>()
            .iter(w)
            .map(|n| (None, n.to_string())),
    );
    if let Some(p) = all.iter().find(|(_, n)| n.to_lowercase() == want) {
        return Ok(p.clone());
    }
    let starts: Vec<&(Option<u64>, String)> = all.iter().filter(|(_, n)| n.to_lowercase().starts_with(&want)).collect();
    match starts.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(format!("No player called {name}.")),
        _ => Err(format!("More than one player's name starts with {name}.")),
    }
}

/// `ignore` / `unignore`: mute or unmute a player's chat lines here.
pub fn ignore(w: &mut World, name: &str, on: bool) -> Result<Option<String>, String> {
    let (owner, shown) = find_player(w, name)?;
    w.get_resource_or_init::<ChatMutes>().set(owner, &shown, on);
    Ok(Some(if on { format!("Not showing {shown}'s chat.") } else { format!("Showing {shown}'s chat again.") }))
}

/// Game messages in the chat, without a window (tests): `say` and
/// `say_team`, a network game's chat lines and refusals, players joining a
/// team ("Bot 2 is joining the Terrorist force", the game's own strings,
/// `map::radio::SayFormats::joins`).
pub struct GameMessagesPlugin;

impl Plugin for GameMessagesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ChatLine>()
            .add_message::<ChatEntry>()
            .init_resource::<ChatMutes>()
            .init_resource::<ChatFilter>()
            .add_message::<Hint>()
            .add_message::<crate::net::ChatMessage>()
            .add_message::<crate::net::Notice>()
            .add_message::<crate::net::NameChanged>()
            .add_message::<crate::net::PlayerCame>()
            .add_message::<crate::map::PlaySound>()
            .add_message::<crate::map::MapSay>()
            .add_systems(
                Update,
                ((team_joins, heard_says, notices, name_changes, comings_and_goings, map_says), route_classic).chain(),
            );
        for (name, team, help) in [
            ("say", false, "Say something to everyone."),
            ("say_team", true, "Say something to your team."),
        ] {
            app.console_command(name, help, move |w, a| {
                say(w, &a.join(" "), team);
                Ok(None)
            });
        }
        app.init_resource::<ChatInput>();
        for (name, team, help) in [
            ("messagemode", false, "Type a chat line to everyone (Y)."),
            ("messagemode2", true, "Type a chat line to your team (U)."),
        ] {
            app.console_command(name, help, move |w, _| {
                open_input(w, team);
                Ok(None)
            });
        }
        app.console_command("ignore", "ignore <name>: hide that player's chat lines (this session).", |w, a| {
            ignore(w, &a.join(" "), true)
        })
        .console_command("unignore", "unignore <name>: show that player's chat lines again.", |w, a| ignore(w, &a.join(" "), false))
        .console_command("ignorelist", "The players whose chat lines are hidden.", |w, _| {
            let mutes = w.get_resource::<ChatMutes>().cloned().unwrap_or_default();
            let users: Vec<String> = w
                .query::<&crate::net::NetCharacter>()
                .iter(w)
                .filter(|c| c.owner.is_some_and(|o| mutes.users.contains(&o)))
                .map(|c| c.name.clone())
                .chain(mutes.names.iter().cloned())
                .collect();
            Ok(Some(if users.is_empty() { "Nobody is ignored.".into() } else { users.join(", ") }))
        });
        crate::console::resource_cvar::<ChatFilter, u8>(
            app,
            "cl_chat",
            "Whose chat lines show: 1 everyone's, 2 the team's only, 0 none (notices and your own always).",
            |f| &mut f.0,
        );
        if let Some(mut console) = app.world_mut().get_resource_mut::<crate::console::Console>() {
            console.archive("cl_chat");
        }
    }
}

/// Chat lines the server sent (a network game): in the game's format,
/// with its sound.
fn heard_says(
    mut heard: MessageReader<crate::net::ChatMessage>,
    radio: Option<Res<crate::map::radio::RadioCommands>>,
    mut out: MessageWriter<ChatEntry>,
) {
    use chimera_ui::chat::LineKind;
    let formats = radio.map(|r| r.say.clone()).unwrap_or_default();
    for m in heard.read() {
        if m.console {
            out.write(ChatEntry {
                kind: LineKind::System,
                sender: None,
                name: None,
                team: None,
                text: m.text.clone(),
                classic: Some(ChatLine::plain(format!("Console: {}", m.text))),
                sound: None,
            });
            continue;
        }
        let runs = formats.line(&m.name, &m.text, m.team, m.alive, m.team_only, m.place.as_deref());
        out.write(ChatEntry {
            kind: if m.team_only { LineKind::Team } else { LineKind::All },
            sender: m.sender,
            name: Some(m.name.clone()),
            team: m.team.map(Team),
            text: m.text.clone(),
            classic: Some(ChatLine(runs, m.team.map(Team))),
            sound: formats.sound.clone(),
        });
    }
}

/// A map's `say` lines in a game of one: the local player's own (as a
/// listen server's host says them; a network game's go through the
/// server, `net::chat`).
fn map_says(
    mut says: MessageReader<crate::map::MapSay>,
    role: Option<Res<crate::core::NetRole>>,
    mut commands: Commands,
) {
    if role.is_some_and(|r| *r != crate::core::NetRole::Standalone) {
        says.clear();
        return;
    }
    for s in says.read() {
        let text = s.0.clone();
        commands.queue(move |w: &mut World| say(w, &text, false));
    }
}

/// Players renamed in a network game ("* X changed name to Y").
fn name_changes(
    mut renamed: MessageReader<crate::net::NameChanged>,
    radio: Option<Res<crate::map::radio::RadioCommands>>,
    mut out: MessageWriter<ChatEntry>,
) {
    let formats = radio.map(|r| r.say.clone()).unwrap_or_default();
    for n in renamed.read() {
        out.write(ChatEntry::notice(format!("{} is now {}", n.old, n.new), Some(ChatLine(formats.renamed(&n.old, &n.new), None))));
    }
}

/// Players joining and leaving a network game ("Player X has joined the
/// game", "Player X left the game (Disconnect by user.)", the engine's
/// strings).
fn comings_and_goings(
    mut came: MessageReader<crate::net::PlayerCame>,
    radio: Option<Res<crate::map::radio::RadioCommands>>,
    mut out: MessageWriter<ChatEntry>,
) {
    let formats = radio.map(|r| r.say.clone()).unwrap_or_default();
    for c in came.read() {
        let text = match &c.left {
            None => format!("{} joined the game", c.name),
            Some(why) if !why.trim().is_empty() => format!("{} left the game ({})", c.name, why.trim().trim_end_matches('.')),
            Some(_) => format!("{} left the game", c.name),
        };
        out.write(ChatEntry::notice(text, Some(ChatLine(formats.came(&c.name, c.left.as_deref()), None))));
    }
}

/// Why the server refused a request (a buy, a team, a line while
/// flooding), as a hint; under the Signal chat as the server's word in it.
fn notices(
    mut notices: MessageReader<crate::net::Notice>,
    active: Option<Res<SignalChatActive>>,
    mut hints: MessageWriter<Hint>,
    mut out: MessageWriter<ChatEntry>,
) {
    let signal = active.is_some_and(|a| a.0);
    for n in notices.read() {
        if signal {
            out.write(ChatEntry {
                kind: chimera_ui::chat::LineKind::System,
                sender: None,
                name: None,
                team: None,
                text: n.text.clone(),
                classic: None,
                sound: None,
            });
        } else {
            hints.write(Hint(n.text.clone()));
        }
    }
}

/// A line when a character first has a team or changes it (once the
/// map's strings are loaded; none while a recording plays back,
/// `core::Playback`).
fn team_joins(
    radio: Option<Res<crate::map::radio::RadioCommands>>,
    who: Query<
        (
            Entity,
            &Team,
            Option<&Name>,
            Has<LocalPlayer>,
            Option<&crate::net::NetCharacter>,
            (Has<crate::core::Spectating>, Has<crate::core::Unassigned>),
        ),
        With<crate::core::Intent>,
    >,
    mut known: Local<std::collections::HashMap<Entity, u8>>,
    settings: Option<Res<crate::net::NetSettings>>,
    playback: Option<Res<crate::core::Playback>>,
    mut chat: MessageWriter<ChatLine>,
) {
    let Some(radio) = radio else { return };
    // A recording playing back (a demo's puppets coming and going) says
    // only what the recording says: its teams are remembered, not told.
    let quiet = playback.is_some();
    let mut now = std::collections::HashMap::with_capacity(known.len());
    for (e, team, name, local, net, (spectating, unassigned)) in &who {
        // On no team yet (the join menus): nothing to say.
        if unassigned {
            continue;
        }
        // The spectators by their own number (`SayFormats::joins`).
        let key = if spectating { crate::map::radio::SPECTATORS } else { team.0 };
        now.insert(e, key);
        if quiet || known.get(&e) == Some(&key) {
            continue;
        }
        // A network game names everyone (us too) as the server does.
        let name = super::shown_name(e, local, net, name, settings.as_deref());
        if let Some(runs) = radio.say.join(&name, key) {
            chat.write(ChatLine(runs, Some(*team)));
        }
    }
    *known = now;
}

/// The chat typing line: open (to the team or not) and its text.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct ChatInput {
    /// Some(true): to the team.
    pub open: Option<bool>,
    pub text: String,
    /// The caret and selection in `text` (the Signal chat's field).
    pub caret: super::widgets::Caret,
}

/// Most characters a chat line takes (Source's say limit, 127 bytes).
const MAX_SAY: usize = 127;

/// Say `text` as the local player to everyone or (`team_only`) their
/// team: a chat line in the game's format, and its sound.
pub fn say(w: &mut World, text: &str, team_only: bool) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    // A network game: the server says who reads it (a listen server's
    // host too: replicon hands its request to the server).
    if w.get_resource::<crate::core::NetRole>()
        .is_some_and(|r| *r != crate::core::NetRole::Standalone)
    {
        w.write_message(crate::net::SayRequest {
            text: crate::net::chat::clean_say(text),
            team_only,
        });
        return;
    }
    type Me<'a> = (
        Entity,
        Option<&'a Team>,
        Option<&'a Health>,
        Has<crate::core::Dead>,
        Option<&'a GlobalTransform>,
        Has<crate::core::Spectating>,
    );
    // A spectator speaks with no team ("*SPEC*", "(Spectator)").
    let Some((me_e, team, alive, at)) = w
        .query_filtered::<Me, With<LocalPlayer>>()
        .iter(w)
        .next()
        .map(|(e, t, h, dead, at, spec)| {
            (
                e,
                t.copied().filter(|_| !spec),
                !dead && h.is_none_or(|h| h.current > 0.0),
                at.map(|g| g.translation()),
            )
        })
    else {
        return;
    };
    let formats = w
        .get_resource::<crate::map::radio::RadioCommands>()
        .map(|r| r.say.clone())
        .unwrap_or_default();
    let place = w.get_resource::<crate::map::nav::NavMesh>().zip(at).and_then(|(nav, at)| {
        let area = nav.area_at(at)?;
        nav.places.get(nav.areas[area].place?).cloned()
    });
    let me = w
        .get_resource::<crate::net::NetSettings>()
        .map_or_else(|| "Player".to_string(), |s| s.name.clone());
    let runs = formats.line(&me, text, team.map(|t| t.0), alive, team_only, place.as_deref());
    let text = crate::net::chat::clean_say(text);
    w.write_message(ChatEntry {
        kind: if team_only { chimera_ui::chat::LineKind::Team } else { chimera_ui::chat::LineKind::All },
        sender: Some(me_e),
        name: Some(me),
        team,
        text,
        classic: Some(ChatLine(runs, team)),
        sound: formats.sound,
    });
}

/// The typing line: Y / U open it while playing; then the keyboard types
/// into it, Enter sends, Escape drops it, and the game sees no keys.
#[allow(clippy::too_many_arguments)]
fn typing(
    mut input: ResMut<ChatInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut events: MessageReader<KeyboardInput>,
    cursor: Query<&CursorOptions>,
    console: Option<Res<super::console::ConsoleUi>>,
    menu: Option<Res<super::front_end::MenuStatus>>,
    (binds, mouse): (Option<Res<crate::console::Console>>, Option<Res<ButtonInput<MouseButton>>>),
    mut commands: Commands,
) {
    let busy = console.is_some_and(|c| c.open) || menu.is_some_and(|m| m.open);
    if input.open.is_none() {
        let playing = cursor.iter().next().is_some_and(super::input::cursor_grabbed);
        // The chat keys (`messagemode`, Y; `messagemode2`, U).
        let pressed = |c: &str| match (&binds, &mouse) {
            (Some(b), Some(m)) => super::binds::just_pressed(&b.binds, &keys, m, c),
            _ => false,
        };
        let (all, team) = (pressed("messagemode"), pressed("messagemode2"));
        if !busy && playing && (all || team) {
            input.open = Some(team);
            input.text.clear();
            keys.reset_all();
        }
        // This frame's keys aren't typed (the Y or U itself).
        events.clear();
        return;
    }
    if busy {
        events.clear();
        return;
    }
    for e in events.read() {
        if e.state != ButtonState::Pressed {
            continue;
        }
        match (&e.logical_key, e.key_code) {
            (_, KeyCode::Enter | KeyCode::NumpadEnter) => {
                let text = std::mem::take(&mut input.text);
                let team = input.open.take() == Some(true);
                commands.queue(move |w: &mut World| say(w, &text, team));
                break;
            }
            (_, KeyCode::Escape) => {
                *input = ChatInput::default();
                break;
            }
            (Key::Backspace, _) => {
                input.text.pop();
            }
            (Key::Space, _) => type_text(&mut input.text, " "),
            (Key::Character(s), _) => type_text(&mut input.text, s),
            _ => {}
        }
    }
    keys.reset_all();
}

/// Add typed text, without control characters, up to `MAX_SAY` bytes.
fn type_text(line: &mut String, s: &str) {
    for c in s.chars().filter(|c| !c.is_control()) {
        if line.len() + c.len_utf8() > MAX_SAY {
            return;
        }
        line.push(c);
    }
}

#[derive(Component)]
struct InputText;

/// The typing line under the chat lines: "Say :" or "Say (TEAM) :" (the
/// engine's `chat_say` strings) and the text with a cursor.
fn draw_input(
    input: Res<ChatInput>,
    hud: Option<Res<ActiveHud>>,
    fonts: Res<UiFonts>,
    windows: Query<&Window>,
    shown: Query<Entity, With<InputText>>,
    mut built: Local<Option<(ChatInput, f32)>>,
    mut commands: Commands,
) {
    let Some(window) = windows.iter().next() else { return };
    let (w, h) = (window.width(), window.height());
    let key = (input.clone(), h);
    if built.as_ref() == Some(&key) {
        return;
    }
    *built = Some(key);
    for e in &shown {
        commands.entity(e).despawn();
    }
    let Some(team) = input.open else { return };
    let scale = h / 480.0;
    let p = panel(
        hud.as_deref(),
        "HudChat",
        (HudCoord::Start(10.0), HudCoord::Start(275.0), 320.0, 120.0),
    );
    // The scheme's ChatFont (bold Verdana, drop shadow).
    let font = fonts.client("ChatFont", h, 12.0);
    let root = commands
        .spawn((
            InputText,
            Text::default(),
            font.clone(),
            TextShadow::default(),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            Node {
                position_type: PositionType::Absolute,
                left: px(p.x.resolve(w, scale)),
                top: px(p.y.resolve(h, scale) + p.tall * scale + 2.0 * scale),
                width: px(p.wide * scale),
                padding: UiRect::axes(px(3.0 * scale), px(1.0 * scale)),
                ..default()
            },
            GlobalZIndex(42),
        ))
        .id();
    let prompt = if team { "Say (TEAM) :" } else { "Say :" };
    commands.spawn((
        TextSpan::new(format!("{prompt} ")),
        font.clone(),
        TextColor(Color::srgb_u8(255, 176, 0)),
        ChildOf(root),
    ));
    commands.spawn((
        TextSpan::new(format!("{}_", input.text)),
        font,
        TextColor(Color::WHITE),
        ChildOf(root),
    ));
}

/// A chat line: coloured runs of text.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct ChatLine(pub Vec<(ChatColor, String)>, pub Option<crate::core::Team>);

impl ChatLine {
    /// A line in the normal colour.
    pub fn plain(text: impl Into<String>) -> Self {
        Self(vec![(ChatColor::Normal, text.into())], None)
    }
}

/// A hint across the screen (`HudHintDisplay`), replacing the one shown.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct Hint(pub String);

/// Seconds a chat line stays (Source's `hud_saytext_time` default).
pub const CHAT_SECONDS: f32 = 12.0;
/// Seconds a hint stays (a guess; docs/tech-debt.md).
pub const HINT_SECONDS: f32 = 4.0;
/// Lines kept at most.
const MAX_LINES: usize = 8;

/// Chat lines shown: runs, the sender's team, when they go.
#[derive(Resource)]
pub struct ChatLog {
    pub lines: VecDeque<(ChatLine, f32)>,
    /// Seconds a new line stays (`hud_saytext_time`).
    pub seconds: f32,
    version: u64,
}

impl Default for ChatLog {
    fn default() -> Self {
        Self {
            lines: VecDeque::new(),
            seconds: CHAT_SECONDS,
            version: 0,
        }
    }
}

impl ChatLog {
    /// Add a line at `now`; the oldest goes past `MAX_LINES`.
    pub fn push(&mut self, line: ChatLine, now: f32) {
        self.lines.push_back((line, now + self.seconds.max(0.0)));
        while self.lines.len() > MAX_LINES {
            self.lines.pop_front();
        }
        self.version += 1;
    }

    pub fn expire(&mut self, now: f32) {
        let before = self.lines.len();
        self.lines.retain(|(_, until)| *until > now);
        if self.lines.len() != before {
            self.version += 1;
        }
    }
}

#[derive(Resource, Default)]
struct HintState {
    text: Option<(String, f32)>,
    version: u64,
}

fn collect(
    mut lines: MessageReader<ChatLine>,
    mut log: ResMut<ChatLog>,
    console: Option<ResMut<Console>>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    let mut console = console;
    for l in lines.read() {
        if let Some(c) = console.as_mut() {
            c.print(Level::Info, l.0.iter().map(|(_, s)| s.as_str()).collect::<String>());
        }
        log.push(l.clone(), now);
    }
    log.expire(now);
}

/// A panel from the HUD, or CS:S's numbers for it.
fn panel(hud: Option<&ActiveHud>, name: &str, fallback: (HudCoord, HudCoord, f32, f32)) -> HudPanel {
    hud.and_then(|h| h.0.panels.get(name).cloned()).unwrap_or(HudPanel {
        x: fallback.0,
        y: fallback.1,
        wide: fallback.2,
        tall: fallback.3,
        background: None,
        icon: Vec2::ZERO,
        digit: Vec2::ZERO,
        digit2: Vec2::ZERO,
        keys: Default::default(),
    })
}

/// A run's colour: team colours from the HUD's scheme; the normal and
/// location colours are guesses (docs/tech-debt.md).
pub fn run_color(hud: Option<&ActiveHud>, c: ChatColor, team: Option<crate::core::Team>) -> Color {
    let named = |n: &str| hud.and_then(|h| h.0.color(n));
    match c {
        ChatColor::Normal => Color::srgb_u8(255, 255, 255),
        ChatColor::Location => Color::srgb_u8(153, 255, 153),
        // HL2:DM's own (rules.md "Chat"): Combine is team 2, Rebels 3, and
        // everyone else (DM players, spectators) is amber.
        ChatColor::Team if hud.is_some_and(|h| h.0.game == "hl2dm") => match team.map(|t| t.0) {
            Some(2) => Color::srgb_u8(153, 204, 255),
            Some(3) => Color::srgb_u8(255, 63, 63),
            _ => Color::srgb_u8(255, 178, 0),
        },
        ChatColor::Team => match team.map(|t| t.0) {
            Some(1) => named("T_Red").unwrap_or(Color::srgb_u8(255, 64, 64)),
            Some(2) => named("CT_Blue").unwrap_or(Color::srgb_u8(153, 204, 255)),
            _ => named("team0").unwrap_or(Color::srgb_u8(204, 204, 204)),
        },
    }
}

#[derive(Component)]
struct ChatRoot;

fn draw_chat(
    log: Res<ChatLog>,
    hud: Option<Res<ActiveHud>>,
    fonts: Res<UiFonts>,
    windows: Query<&Window>,
    root: Query<Entity, With<ChatRoot>>,
    mut built: Local<(u64, f32)>,
    mut commands: Commands,
) {
    let Some(window) = windows.iter().next() else { return };
    let (w, h) = (window.width(), window.height());
    let scale = h / 480.0;
    if *built == (log.version, h) && (!root.is_empty() || log.lines.is_empty()) {
        return;
    }
    *built = (log.version, h);
    for e in &root {
        commands.entity(e).despawn();
    }
    if log.lines.is_empty() {
        return;
    }
    let hud = hud.as_deref();
    let p = panel(
        hud,
        "HudChat",
        (HudCoord::Start(10.0), HudCoord::Start(275.0), 320.0, 120.0),
    );
    let root = commands
        .spawn((
            ChatRoot,
            Node {
                position_type: PositionType::Absolute,
                left: px(p.x.resolve(w, scale)),
                top: px(p.y.resolve(h, scale)),
                width: px(p.wide * scale),
                height: px(p.tall * scale),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                overflow: Overflow::clip(),
                ..default()
            },
            GlobalZIndex(41),
        ))
        .id();
    let font = fonts.client("ChatFont", h, 12.0);
    for (line, _) in &log.lines {
        let text = commands
            .spawn((
                Text::default(),
                font.clone(),
                TextShadow::default(),
                ChildOf(root),
            ))
            .id();
        for (c, s) in &line.0 {
            commands.spawn((
                TextSpan::new(s.clone()),
                font.clone(),
                TextColor(run_color(hud, *c, line.1)),
                ChildOf(text),
            ));
        }
    }
}

#[derive(Component)]
struct HintText;

fn draw_hint(
    mut hints: MessageReader<Hint>,
    mut state: ResMut<HintState>,
    hud: Option<Res<ActiveHud>>,
    fonts: Res<UiFonts>,
    windows: Query<&Window>,
    shown: Query<Entity, With<HintText>>,
    time: Res<Time>,
    mut built: Local<(u64, f32)>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs();
    for h in hints.read() {
        state.text = Some((h.0.clone(), now + HINT_SECONDS));
        state.version += 1;
    }
    if state.text.as_ref().is_some_and(|(_, until)| now > *until) {
        state.text = None;
        state.version += 1;
    }
    let Some(window) = windows.iter().next() else { return };
    let (w, h) = (window.width(), window.height());
    if *built == (state.version, h) {
        return;
    }
    *built = (state.version, h);
    for e in &shown {
        commands.entity(e).despawn();
    }
    let Some((text, _)) = &state.text else { return };
    let scale = h / 480.0;
    let hud = hud.as_deref();
    let p = panel(
        hud,
        "HudHintDisplay",
        (HudCoord::Centre(-240.0), HudCoord::Centre(60.0), 480.0, 100.0),
    );
    let fg = hud.and_then(|h| h.0.color("HintMessageFg")).unwrap_or(Color::WHITE);
    let bg = hud
        .and_then(|h| h.0.color("HintMessageBg"))
        .unwrap_or(Color::srgba_u8(0, 0, 0, 60));
    let inset = p.num("text_xpos").unwrap_or(8.0) * scale;
    // The panel's box; the text centred, on its bottom ("center_y" -1).
    let root = commands
        .spawn((
            HintText,
            Node {
                position_type: PositionType::Absolute,
                left: px(p.x.resolve(w, scale)),
                top: px(p.y.resolve(h, scale)),
                width: px(p.wide * scale),
                height: px(p.tall * scale),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(45),
        ))
        .id();
    commands.spawn((
        Text::new(text.clone()),
        fonts.client("HudHintText", h, 12.0),
        TextColor(fg),
        TextLayout::justify(Justify::Center),
        BackgroundColor(bg),
        Node {
            padding: UiRect::all(px(inset)),
            border_radius: BorderRadius::all(px(4.0 * scale)),
            ..default()
        },
        ChildOf(root),
    ));
}


/// The chat's and hints' scheme fonts at the window's size, laid out ahead
/// (`text_warm`).
fn warm_fonts(window: super::text_warm::PrimaryWindows, fonts: Option<Res<UiFonts>>) -> Vec<super::text_warm::WarmText> {
    let Some(fonts) = fonts else { return Vec::new() };
    let h = super::text_warm::window_height(&window);
    vec![fonts.client("ChatFont", h, 12.0).into(), fonts.client("HudHintText", h, 12.0).into()]
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_expire_and_overflow() {
        let mut log = ChatLog::default();
        for i in 0..10 {
            log.push(ChatLine::plain(format!("{i}")), i as f32);
        }
        assert_eq!(log.lines.len(), MAX_LINES);
        assert_eq!(log.lines[0].0, ChatLine::plain("2"));
        log.expire(2.0 + CHAT_SECONDS + 0.5);
        assert_eq!(log.lines.front().map(|l| &l.0), Some(&ChatLine::plain("3")));
        // `hud_saytext_time` sets how long a new line stays.
        log.seconds = 3.0;
        log.push(ChatLine::plain("short"), 100.0);
        assert_eq!(log.lines.back().map(|l| l.1), Some(103.0));
    }

    #[test]
    fn typed_text_has_no_control_characters_and_a_limit() {
        let mut line = String::new();
        type_text(&mut line, "hi\u{8}\t!");
        assert_eq!(line, "hi!");
        type_text(&mut line, &"x".repeat(200));
        assert_eq!(line.len(), MAX_SAY);
    }

    #[test]
    fn team_runs_take_the_scheme_colours() {
        use crate::core::Team;
        assert_eq!(
            run_color(None, ChatColor::Team, Some(Team(1))),
            Color::srgb_u8(255, 64, 64)
        );
        assert_eq!(
            run_color(None, ChatColor::Team, Some(Team(2))),
            Color::srgb_u8(153, 204, 255)
        );
    }

    /// hl2dm/hud.md 12, case 70, rules.md "Chat": in HL2:DM Combine (team
    /// 2) is light blue, Rebels (3) red, anyone else (DM, spectators)
    /// amber; CS:S's look keeps its own.
    #[test]
    fn hl2dm_names_take_its_team_colours() {
        use crate::core::Team;
        use crate::map::hud::GameHud;
        let look = |game: &str| ActiveHud(std::sync::Arc::new(GameHud { game: game.into(), ..default() }), Default::default());
        let hl2 = look("hl2dm");
        let c = |hud: &ActiveHud, t: Option<u8>| run_color(Some(hud), ChatColor::Team, t.map(Team));
        assert_eq!(c(&hl2, Some(2)), Color::srgb_u8(153, 204, 255));
        assert_eq!(c(&hl2, Some(3)), Color::srgb_u8(255, 63, 63));
        for t in [None, Some(0), Some(1)] {
            assert_eq!(c(&hl2, t), Color::srgb_u8(255, 178, 0));
        }
        let cs = look("cs_source");
        assert_eq!(c(&cs, Some(1)), Color::srgb_u8(255, 64, 64));
        assert_eq!(c(&cs, Some(2)), Color::srgb_u8(153, 204, 255));
        assert_eq!(c(&cs, None), Color::srgb_u8(204, 204, 204));
    }
}
