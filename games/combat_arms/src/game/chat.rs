//! Radio and chat (specs/combat_arms/radio_chat.md): how players talk in
//! a match.
//!
//! - `text`: the text commands (T1), the bad-word filter (F1, data: no
//!   words are built in) and the block list (B1). `limits`: the sender's
//!   rate window and repeat lock, the whispers' limits, the lobby's
//!   repeat lock and the server's silent limiter (L1 to L3), on an
//!   explicit millisecond clock. `radio`: the radio file (R1, the
//!   install's `RadioMsgSound` or ours in `data/radio.txt`), what picking
//!   an entry sends (R3) and what a receiver makes of it (R5). `show`:
//!   what a receiver shows of an incoming line (P2) and the lines kept
//!   (`ChatLog`). `client`: the local player's side as a state machine
//!   (the input bar and its tabs, C3 in order, whispers, clan lines, the
//!   radio menus). `voice`: the voice channels by mode and push-to-talk.
//! - Here the engine's side. All and team chat and radio go through the
//!   server, as the spec has it: the sender's client asks (`CaSayRequest`,
//!   `CaRadioRequest`, after its own checks), the server limits chat
//!   (`ServerLimits`: silent), relays the raw text to whoever reads it
//!   (all: everyone; team: the sender's team and the spectators on team
//!   255; radio: everyone or the sender's team, with no checks at all)
//!   and each receiving client decides what to show (`CaSayIn`,
//!   `CaRadioIn`): block list, Hunted, colours, filter, name colour. A
//!   game of one runs the same relay for its own player. Whispers and
//!   clan lines belong to online services that don't exist here: what
//!   goes to them is `CaWhisperOut`, `CaClanOut` and `CaAdminOut`
//!   messages, what comes from them `CaWhisperIn` and `CaClanIn`.
//! - Commands for binds (the front end's keys call them): `chat_open`
//!   (Enter), `chat_all`, `chat_team`, `chat_clan` (F2, F3, F4),
//!   `radio_report`, `radio_answer`, `radio_request` (B, V, N), `+voicechat`
//!   (T) and `voice_toggle` (F8). Cvars: `ca_chatfilter` (the filter
//!   option), `ca_chatregion` (1: the BR limiter set).
//!
//! Known shortcuts (docs/tech-debt.md; the most literal reading where the
//! spec is unclear):
//! - Aliases of the text commands, the labels and notice texts are ours
//!   (Open question 2): the string table's are unread. Notices that are
//!   string ids in the spec (`$BlockWhisper`, `$UserSpamBlockWhisper`) are
//!   kept as ids in the lines, resolved by the panel.
//! - The radio file is the install's `attributes/radiomsgsound.txt` when
//!   it reads (the name is a guess, `heavy::radio` checks it), else ours:
//!   silent. A voice set comes from `CaVoice` (nothing sets it yet:
//!   characters carry no `SoundTemplate`); without one the previous index
//!   (0) is kept.
//! - Name colours (`CaNameColour`), the GM accounts, the punishment
//!   minutes, the chat filter option and its lists, the clan, and the
//!   "accept whispers" and auto-reply options are settings on the client
//!   (`CaChat`), set by tests and later by the lobby; no service sets
//!   them. Whispers and clan lines have nobody on the other end.
//! - Voice: only the channel choice and the key's state are modelled; no
//!   audio is sent (Chimera's voice isn't wired to the channels).
//! - The wall clock is the game's clock (`SimClock`, a client's fixed
//!   clock); the 2000 ms radio cooldown, the lock and the lives are in it.
//! - The server's name stamp is the player's current name; the spec's
//!   chat log (server console) isn't written.
//! - Not modelled: the in-match panel's lobby-room variant, the Go4CA
//!   Cup (`ESL`) hiding of whispers, the megaphone lines (a shop's), the
//!   emote page (Hunted's G key, a later spec), paste for GM accounts.

pub mod client;
pub mod limits;
pub mod radio;
pub mod show;
pub mod text;
pub mod voice;

use std::collections::HashMap;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use self::{
    client::{ChatClient, Env, InputKey, Out, Tab},
    limits::{Region, ServerLimiter},
    radio::{RadioData, RadioSend},
    show::{Channel, Incoming, Peer, ShowOpts},
};
use super::{
    effects::sounds::CaSound,
    rules::{CaMatchView, LAST_MAN_STANDING, current_mode},
};
use crate::{
    console::{ConsoleAppExt, resource_cvar},
    core::{Dead, Health, LocalPlayer, MapLoads, NetRole, SimClock, Spectating, Team},
    net::{NetAppExt, NetCharacter, server::Player},
};

/// The server reads at most this many characters of a chat line.
pub const SERVER_CHAT_MAX: usize = 119;

// --- The protocol.

/// A client asks to say a line (after the sender's own checks).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CaSayRequest {
    pub team: bool,
    pub text: String,
}

/// A client's radio message (R3 step 5).
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct CaRadioRequest {
    pub at: [f32; 3],
    pub class: u8,
    pub entry: u8,
    pub r: u8,
    pub to_all: bool,
    pub t: u8,
}

impl From<RadioSend> for CaRadioRequest {
    fn from(s: RadioSend) -> Self {
        Self {
            at: s.at,
            class: s.class,
            entry: s.entry,
            r: s.r,
            to_all: s.to_all,
            t: s.t,
        }
    }
}

/// Server to each reader: a chat line (the raw text; the name is stamped
/// by the server).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct CaSayIn {
    /// The sender's character; None: the server's own text.
    #[entities]
    pub sender: Option<Entity>,
    pub name: String,
    pub name_colour: u32,
    pub team: bool,
    pub text: String,
}

/// Server to each hearer: a radio call. For a team-only call `sender` and
/// `name_colour` are the receiver's own (Quirk 1).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct CaRadioIn {
    #[entities]
    pub sender: Option<Entity>,
    pub name: String,
    pub name_colour: u32,
    pub at: [f32; 3],
    pub class: u8,
    pub entry: u8,
    pub r: u8,
    pub t: u8,
}

/// A whisper from the lobby service (W2).
#[derive(Message, Clone, Debug, Default, PartialEq)]
pub struct CaWhisperIn {
    pub from: String,
    pub body: String,
}

/// A clan room line (K1): the room id (`name@room`) and the text.
#[derive(Message, Clone, Debug, Default, PartialEq)]
pub struct CaClanIn {
    pub room_id: String,
    pub text: String,
}

/// For the online services: a whisper, a clan line and a GM command to
/// send.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct CaWhisperOut {
    pub to: String,
    pub body: String,
}

#[derive(Message, Clone, Debug, PartialEq)]
pub struct CaClanOut {
    pub text: String,
}

#[derive(Message, Clone, Debug, PartialEq)]
pub struct CaAdminOut {
    pub line: String,
}

// --- Components and resources.

/// A character's voice set (the radio's `SoundTemplate` name).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct CaVoice(pub String);

/// A player's name colour in chat lines (0xRRGGBB; 0 or absent: none).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaNameColour(pub u32);

/// The local player's chat (`client::ChatClient`).
#[derive(Resource, Default, Deref, DerefMut)]
pub struct CaChat(pub ChatClient);

/// The radio data in use.
#[derive(Resource, Deref)]
pub struct CaRadio(pub RadioData);

/// What the front end knows and the headless game cannot: the cursor is
/// grabbed, a fade, the HUD hidden. The defaults are "playing".
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ChatUiState {
    pub playing: bool,
    pub fading: bool,
    pub hud_hidden: bool,
    pub vote_on: bool,
}

impl Default for ChatUiState {
    fn default() -> Self {
        Self {
            playing: true,
            fading: false,
            hud_hidden: false,
            vote_on: false,
        }
    }
}

/// The server's limiter for each player's all and team chat.
#[derive(Resource, Default)]
pub struct ServerLimits(pub HashMap<Entity, ServerLimiter>);

/// The radio's random line (xorshift; seeded from the tick).
#[derive(Resource, Default)]
pub struct ChatRng(pub u32);

impl ChatRng {
    pub fn next(&mut self, seed: u32) -> usize {
        let mut x = if self.0 == 0 { seed | 1 } else { self.0 };
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x as usize
    }
}

/// The 1 to switch to the BR region's limits (`ca_chatregion`).
#[derive(Resource, Default)]
pub struct ChatRegion(pub u8);

/// The chat's systems (the front end's order against them).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CaChatSet;

pub struct CaChatPlugin;

impl Plugin for CaChatPlugin {
    fn build(&self, app: &mut App) {
        let radio = RadioData::load(&super::data::CaData::of(app));
        app.init_resource::<CaChat>()
            .init_resource::<ChatUiState>()
            .init_resource::<ServerLimits>()
            .init_resource::<ChatRng>()
            .init_resource::<ChatRegion>()
            .insert_resource(CaRadio(radio))
            .add_message::<CaSayRequest>()
            .add_message::<CaRadioRequest>()
            .add_message::<CaSayIn>()
            .add_message::<CaRadioIn>()
            .add_message::<CaWhisperIn>()
            .add_message::<CaClanIn>()
            .add_message::<CaWhisperOut>()
            .add_message::<CaClanOut>()
            .add_message::<CaAdminOut>()
            .add_message::<CaSound>()
            .add_systems(
                Update,
                (
                    serve_standalone.run_if(standalone),
                    (hear_chat, hear_radio, hear_services, housekeeping).chain(),
                )
                    .chain()
                    .in_set(CaChatSet),
            );
        app.console_command(
            "combat_arms:ca_chatfilter",
            "ca_chatfilter [0|1]: this client filters the bad words from what it receives (an account option).",
            |w, a| {
                let mut chat = w.resource_mut::<CaChat>();
                if let Some(v) = a.first() {
                    chat.settings.filter_on = v.parse::<f32>().map_err(|_| "ca_chatfilter [0|1]")? != 0.0;
                }
                Ok(Some(format!("ca_chatfilter {}", chat.settings.filter_on as u8)))
            },
        );
        resource_cvar::<ChatRegion, u8>(
            app,
            "combat_arms:ca_chatregion",
            "1: the BR region's chat limits (5 lines in 10 s) instead of the others' (20 in 60 s).",
            |r| &mut r.0,
        );
        commands(app);
        app.net_part("combat_arms_chat", register);
    }
}

fn standalone(role: Option<Res<NetRole>>) -> bool {
    role.is_none_or(|r| *r == NetRole::Standalone)
}

/// The protocol (in this order on every side) and the server's systems.
fn register(app: &mut App) {
    app.add_client_message::<CaSayRequest>(Channel_::Ordered)
        .add_client_message::<CaRadioRequest>(Channel_::Ordered)
        .add_mapped_server_message::<CaSayIn>(Channel_::Ordered)
        .add_mapped_server_message::<CaRadioIn>(Channel_::Ordered)
        .add_systems(
            PreUpdate,
            receive
                .after(ServerSystems::Receive)
                .run_if(in_state(ServerState::Running)),
        );
}

use bevy_replicon::prelude::Channel as Channel_;

// --- Facts of the world.

/// The game clock, ms (a client's is its fixed clock).
pub fn now_ms(world: &World) -> f64 {
    let client = world.get_resource::<NetRole>() == Some(&NetRole::Client);
    let secs = if client {
        world.resource::<Time<Fixed>>().elapsed_secs_f64()
    } else {
        world
            .get_resource::<SimClock>()
            .map_or_else(|| world.resource::<Time>().elapsed_secs_f64(), |c| c.now)
    };
    secs * 1000.0
}

/// A character as a receiver knows it: its name, its team (255: a
/// spectator or none) and whether it lives.
pub fn peer_of(world: &World, e: Entity) -> Option<Peer> {
    world.get_entity(e).ok()?;
    let alive = world.get::<Dead>(e).is_none() && world.get::<Health>(e).is_none_or(|h| h.current > 0.0);
    let team = if world.get::<Spectating>(e).is_some() {
        255
    } else {
        world.get::<Team>(e).map_or(255, |t| t.0)
    };
    let name = world
        .get::<NetCharacter>(e)
        .map(|c| c.name.clone())
        .or_else(|| world.get::<Name>(e).map(|n| n.to_string()))
        .unwrap_or_else(|| "Player".into());
    Some(Peer { name, team, alive })
}

fn local_player(world: &mut World) -> Option<Entity> {
    chimera_core::query_cache::first::<With<LocalPlayer>>(world)
}

fn name_colour(world: &World, e: Entity) -> u32 {
    world.get::<CaNameColour>(e).map_or(0, |c| c.0)
}

fn is_hunted(world: &World) -> bool {
    current_mode(world)
        .or_else(|| world.get_resource::<CaMatchView>().and_then(|v| v.mode()))
        .is_some_and(|m| m.key == "HG")
}

fn is_lms(world: &World) -> bool {
    current_mode(world)
        .or_else(|| world.get_resource::<CaMatchView>().and_then(|v| v.mode()))
        .is_some_and(|m| m.id == LAST_MAN_STANDING)
}

/// The chat's view of the world for the local player.
pub fn env(world: &mut World) -> Env {
    let ui = world.get_resource::<ChatUiState>().copied().unwrap_or_default();
    let me = local_player(world);
    let peer = me.and_then(|e| peer_of(world, e));
    let nothing = me.is_some_and(|e| world.get::<Team>(e).is_none() && world.get::<Spectating>(e).is_none());
    let lms = is_lms(world);
    let alive = peer.as_ref().is_none_or(|p| p.alive);
    let started = world.get_resource::<MapLoads>().is_some_and(|m| m.0 > 0) || me.is_some();
    Env {
        playing: ui.playing,
        fading: ui.fading,
        team_nothing: nothing,
        hud_hidden: ui.hud_hidden,
        alive,
        hud_built: started,
        weapon_prompt: false,
        vote_on: ui.vote_on,
        own: peer.map(|p| p.name).unwrap_or_default(),
        lms_dead: lms && !alive,
    }
}

/// Hand what a send produced to the server and the services.
pub fn dispatch(world: &mut World, outs: Vec<Out>) {
    for out in outs {
        match out {
            Out::Say { team, text } => {
                world.write_message(CaSayRequest { team, text });
            }
            Out::Radio(send) => {
                world.write_message(CaRadioRequest::from(send));
            }
            Out::Whisper { to, body } => {
                world.write_message(CaWhisperOut { to, body });
            }
            Out::Clan { text } => {
                world.write_message(CaClanOut { text });
            }
            Out::Admin { line } => {
                world.write_message(CaAdminOut { line });
            }
        }
    }
}

// --- The front end's entry points (also the tests').

/// Enter: open the input on the current tab.
pub fn open_enter(world: &mut World) -> bool {
    let env = env(world);
    world.resource_mut::<CaChat>().open_with_enter(&env)
}

/// A channel bind.
pub fn open_tab(world: &mut World, tab: Tab) -> bool {
    let env = env(world);
    world.resource_mut::<CaChat>().open_with_bind(tab, &env)
}

/// A key the open input handles.
pub fn input_key(world: &mut World, key: InputKey) {
    let env = env(world);
    let now = now_ms(world);
    let outs = world.resource_mut::<CaChat>().key(key, &env, now);
    dispatch(world, outs);
}

/// A radio key.
pub fn radio_key(world: &mut World, class: u8) {
    let env = env(world);
    let now = now_ms(world);
    world.resource_mut::<CaChat>().radio_key(class, &env, now);
}

/// A number key (0 to 9) while a radio menu is open; true when a menu was
/// open (the key is swallowed: no weapon switch).
pub fn radio_number(world: &mut World, n: usize) -> bool {
    radio_number_with(world, n, None)
}

/// `radio_number` with the random line fixed (tests).
pub fn radio_number_with(world: &mut World, n: usize, draw: Option<usize>) -> bool {
    if world.resource::<CaChat>().radio_open.is_none() {
        return false;
    }
    let now = now_ms(world);
    let me = local_player(world);
    let at = me
        .and_then(|e| world.get::<Transform>(e))
        .map_or([0.0; 3], |t| t.translation.to_array());
    let voice = me.and_then(|e| world.get::<CaVoice>(e)).map(|v| v.0.clone());
    let seed = world.get_resource::<SimClock>().map_or(1, |c| c.tick as u32).wrapping_mul(2_654_435_761);
    let r = match draw {
        Some(d) => d,
        None => world.resource_mut::<ChatRng>().next(seed),
    };
    let data = world.resource::<CaRadio>().0.clone();
    let out = world
        .resource_mut::<CaChat>()
        .radio_choose(n, &data, voice.as_deref(), at, move |_| r, now);
    dispatch(world, out.into_iter().collect());
    true
}

// --- Console commands the keys are bound to.

fn commands(app: &mut App) {
    app.console_command("combat_arms:chat_open", "Open the chat input on the current tab (Enter).", |w, _| {
        open_enter(w);
        Ok(None)
    });
    for (name, tab, help) in [
        ("chat_all", Tab::All, "Chat to everyone (F2)."),
        ("chat_team", Tab::Team, "Chat to your team (F3)."),
        ("chat_clan", Tab::Clan, "Chat to your clan (F4)."),
    ] {
        app.console_command(&format!("combat_arms:{name}"), help, move |w, _| {
            open_tab(w, tab);
            Ok(None)
        });
    }
    for (name, class, help) in [
        ("radio_report", radio::REPORT, "The radio's report menu (B)."),
        ("radio_answer", radio::ANSWER, "The radio's answer menu (V)."),
        ("radio_request", radio::REQUEST, "The radio's request menu (N)."),
    ] {
        app.console_command(&format!("combat_arms:{name}"), help, move |w, _| {
            radio_key(w, class);
            Ok(None)
        });
    }
    for (sign, on) in [("+", true), ("-", false)] {
        app.console_command(
            &format!("combat_arms:{sign}voicechat"),
            "Push to talk (T).",
            move |w, _| {
                voice_key(w, on);
                Ok(None)
            },
        );
    }
    app.console_command("combat_arms:voice_toggle", "Voice output on or off (F8).", |w, _| {
        let now = now_ms(w);
        let mut chat = w.resource_mut::<CaChat>();
        for l in chat.voice.toggle_output(now) {
            chat.log.push(l);
        }
        Ok(None)
    });
}

/// The voice channel of the local player under the current mode.
pub fn local_voice_channel(world: &mut World) -> Result<voice::Channel, voice::NoChannel> {
    let mode = current_mode(world)
        .or_else(|| world.get_resource::<CaMatchView>().and_then(|v| v.mode()))
        .map_or(voice::VoiceMode::Teams, |m| voice::mode_of(m.key));
    let me = local_player(world).and_then(|e| peer_of(world, e));
    match me {
        Some(p) => voice::channel(mode, voice::Voicer { team: p.team, alive: p.alive }),
        None => Err(voice::NoChannel::Disconnected),
    }
}

/// The push-to-talk key goes down or up.
pub fn voice_key(world: &mut World, down: bool) {
    let now = now_ms(world);
    if down {
        let ch = local_voice_channel(world);
        let mut chat = world.resource_mut::<CaChat>();
        if let Some(line) = chat.voice.press(ch, now) {
            chat.log.push(line);
        }
    } else {
        world.resource_mut::<CaChat>().voice.release();
    }
}

// --- The server.

/// A reader of chat: where its messages go and which character it is.
#[derive(Clone, Copy)]
pub(crate) enum Target {
    Local,
    Client(ClientId),
}

pub(crate) fn send<M: Message>(world: &mut World, to: Target, message: M)
where
    ToClients<M>: Message,
{
    match to {
        Target::Local => {
            world.write_message(message);
        }
        Target::Client(id) => {
            world.write_message(ToClients {
                targets: SendTargets::Single(id),
                message,
            });
        }
    }
}

/// Everyone who reads chat on this machine, with their characters: a
/// game of one has its player; a server its remote players and its host.
pub(crate) fn readers(world: &mut World) -> Vec<(Target, Entity)> {
    if world.get_resource::<NetRole>().is_none_or(|r| *r == NetRole::Standalone) {
        return local_player(world).map(|e| (Target::Local, e)).into_iter().collect();
    }
    let mut out: Vec<(Target, Entity)> = world
        .query::<(Entity, &Player)>()
        .iter(world)
        .map(|(c, p)| (Target::Client(ClientId::Client(c)), p.character))
        .collect();
    if let Some(host) = local_player(world) {
        out.push((Target::Client(ClientId::Server), host));
    }
    out
}

/// Whether a reader on `to` reads the sender's chat: all chat everyone;
/// team chat the sender's team and the spectators (team 255).
pub fn reads_say(team_chat: bool, sender_team: u8, reader_team: u8) -> bool {
    !team_chat || reader_team == sender_team || reader_team == 255
}

/// Whether a reader hears a radio call: everyone for `to_all`, else the
/// sender's team (spectators on another team id get nothing).
pub fn hears_radio(to_all: bool, sender_team: u8, reader_team: u8) -> bool {
    to_all || reader_team == sender_team
}

/// Make a chat line safe: no control characters, at most 119 characters.
pub fn clean(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(SERVER_CHAT_MAX).collect()
}

/// The server's part of a chat line from `from` (S1, step 2): the rate
/// and repeat limits (silent drops), then the relay.
pub fn deliver_say(world: &mut World, from: Entity, team_chat: bool, text: &str) -> bool {
    let text = clean(text);
    if text.is_empty() {
        return false;
    }
    let now = now_ms(world);
    let ok = world.resource_mut::<ServerLimits>().0.entry(from).or_default().accept(now, &text);
    if !ok {
        return false;
    }
    let Some(s) = peer_of(world, from) else { return false };
    let colour = name_colour(world, from);
    let sender = world.get::<Replicated>(from).map(|_| from);
    for (target, viewer) in readers(world) {
        let Some(v) = peer_of(world, viewer) else { continue };
        if !reads_say(team_chat, s.team, v.team) {
            continue;
        }
        let sender = match target {
            Target::Client(ClientId::Client(_)) => sender,
            _ => Some(from),
        };
        send(
            world,
            target,
            CaSayIn {
                sender,
                name: s.name.clone(),
                name_colour: colour,
                team: team_chat,
                text: text.clone(),
            },
        );
    }
    true
}

/// The server's relay of a radio call from `from` (R4): no checks.
pub fn deliver_radio(world: &mut World, from: Entity, r: CaRadioRequest) {
    let Some(s) = peer_of(world, from) else { return };
    let colour = name_colour(world, from);
    for (target, viewer) in readers(world) {
        let Some(v) = peer_of(world, viewer) else { continue };
        if !hears_radio(r.to_all, s.team, v.team) {
            continue;
        }
        // A team call carries the receiver's own client id and name
        // colour (Quirk 1).
        let (who, c) = if r.to_all { (from, colour) } else { (viewer, name_colour(world, viewer)) };
        let who = match target {
            Target::Client(ClientId::Client(_)) => world.get::<Replicated>(who).map(|_| who),
            _ => Some(who),
        };
        send(
            world,
            target,
            CaRadioIn {
                sender: who,
                name: s.name.clone(),
                name_colour: c,
                at: r.at,
                class: r.class,
                entry: r.entry,
                r: r.r,
                t: r.t,
            },
        );
    }
}

/// A game of one: its player's requests, served as a server would.
fn serve_standalone(world: &mut World) {
    let says: Vec<CaSayRequest> = {
        let mut m = world.resource_mut::<Messages<CaSayRequest>>();
        m.drain().collect()
    };
    let radios: Vec<CaRadioRequest> = {
        let mut m = world.resource_mut::<Messages<CaRadioRequest>>();
        m.drain().collect()
    };
    let Some(me) = local_player(world) else { return };
    for s in says {
        deliver_say(world, me, s.team, &s.text);
    }
    for r in radios {
        deliver_radio(world, me, r);
    }
}

/// A network server: the clients' requests (the host's too).
fn receive(
    mut says: MessageReader<FromClient<CaSayRequest>>,
    mut radios: MessageReader<FromClient<CaRadioRequest>>,
    mut commands: Commands,
) {
    for r in says.read() {
        let (client, team, text) = (r.client_id, r.message.team, r.message.text.clone());
        commands.queue(move |w: &mut World| {
            if let Some(from) = speaker(w, client) {
                deliver_say(w, from, team, &text);
            }
        });
    }
    for r in radios.read() {
        let (client, req) = (r.client_id, r.message);
        commands.queue(move |w: &mut World| {
            if let Some(from) = speaker(w, client) {
                deliver_radio(w, from, req);
            }
        });
    }
}

/// The character a request came from.
fn speaker(world: &mut World, client: ClientId) -> Option<Entity> {
    match client.entity() {
        Some(c) => world.get::<Player>(c).map(|p| p.character),
        None => local_player(world),
    }
}

// --- The receiving client.

fn show_opts(chat: &ChatClient, hunted: bool) -> ShowOpts<'_> {
    ShowOpts {
        filter_on: chat.settings.filter_on,
        hunted,
        block: &chat.block,
        words: &chat.settings.words,
    }
}

/// P2 for one incoming all or team line: `sender` is the sender as this
/// client's table has them (None: an unknown client id, dropped).
pub fn receive_say(chat: &mut ChatClient, me: &Peer, sender: Option<&Peer>, m: &CaSayIn, hunted: bool, now: f64) {
    let inc = Incoming {
        channel: if m.team { Channel::Team } else { Channel::All },
        sender,
        name: &m.name,
        name_colour: m.name_colour,
        class: None,
        text: &m.text,
    };
    if let Some(line) = show::show(me, &show_opts(chat, hunted), &inc, now) {
        chat.log.push(line);
    }
}

/// R5 for one incoming radio call: the line (as P2 shows it), then the
/// sound whatever became of the line.
pub fn receive_radio(
    chat: &mut ChatClient,
    data: &RadioData,
    me: &Peer,
    sender: Option<&Peer>,
    m: &CaRadioIn,
    hunted: bool,
    now: f64,
) -> Option<radio::RadioSound> {
    let (text, sound) = radio::receive(data, m.class, m.entry, m.r, m.t, m.at);
    let class = data.classes.get(m.class as usize).map(|c| c.name.as_str());
    let inc = Incoming {
        channel: Channel::Radio,
        sender,
        name: &m.name,
        name_colour: m.name_colour,
        class,
        text: &text,
    };
    if let Some(line) = show::show(me, &show_opts(chat, hunted), &inc, now) {
        chat.log.push(line);
    }
    sound
}

/// Incoming all and team lines.
fn hear_chat(world: &mut World) {
    let lines: Vec<CaSayIn> = world.resource_mut::<Messages<CaSayIn>>().drain().collect();
    if lines.is_empty() {
        return;
    }
    let now = now_ms(world);
    let hunted = is_hunted(world);
    let Some(me) = local_player(world).and_then(|e| peer_of(world, e)) else { return };
    for m in lines {
        let Some(sender) = m.sender else {
            // The server's own text: a bad-news message, not in the panel.
            world.resource_mut::<CaChat>().log.notices.push((m.text, now));
            continue;
        };
        let peer = peer_of(world, sender);
        receive_say(&mut world.resource_mut::<CaChat>(), &me, peer.as_ref(), &m, hunted, now);
    }
}

/// Incoming radio calls.
fn hear_radio(world: &mut World) {
    let calls: Vec<CaRadioIn> = world.resource_mut::<Messages<CaRadioIn>>().drain().collect();
    if calls.is_empty() {
        return;
    }
    let now = now_ms(world);
    let hunted = is_hunted(world);
    let data = world.resource::<CaRadio>().0.clone();
    let Some(me) = local_player(world).and_then(|e| peer_of(world, e)) else { return };
    for m in calls {
        let peer = m.sender.and_then(|s| peer_of(world, s));
        let sound = receive_radio(&mut world.resource_mut::<CaChat>(), &data, &me, peer.as_ref(), &m, hunted, now);
        if let Some(s) = sound {
            world.write_message(play(s));
        }
    }
}

/// A radio sound as the game plays sounds.
pub fn play(s: radio::RadioSound) -> CaSound {
    match s.at {
        Some((at, radius)) => CaSound::at(s.file, Vec3::from(at), radius),
        None => CaSound::local(s.file),
    }
}

/// Whispers and clan lines from the services.
fn hear_services(world: &mut World) {
    let whispers: Vec<CaWhisperIn> = world.resource_mut::<Messages<CaWhisperIn>>().drain().collect();
    let clans: Vec<CaClanIn> = world.resource_mut::<Messages<CaClanIn>>().drain().collect();
    if whispers.is_empty() && clans.is_empty() {
        return;
    }
    let now = now_ms(world);
    let env = env(world);
    for w in whispers {
        let outs = world.resource_mut::<CaChat>().whisper_in(&w.from, &w.body, &env, now);
        dispatch(world, outs);
    }
    for c in clans {
        world.resource_mut::<CaChat>().clan_in(&c.room_id, &c.text, now);
    }
}

/// Per frame: a new map resets the tab; dying closes the radio menu; the
/// "sent this frame" flag; the region's limits.
fn housekeeping(world: &mut World, mut seen: Local<u32>, mut last_region: Local<u8>) {
    let loads = world.get_resource::<MapLoads>().map_or(0, |m| m.0);
    let region = world.resource::<ChatRegion>().0;
    let alive = local_player(world).and_then(|e| peer_of(world, e)).is_none_or(|p| p.alive);
    let mut chat = world.resource_mut::<CaChat>();
    if loads != *seen {
        *seen = loads;
        chat.enter_world();
    }
    if region != *last_region {
        *last_region = region;
        let r = if region == 1 { Region::BR } else { Region::NON_BR };
        chat.limits.region = r;
        chat.whisper.region = r;
    }
    if !alive {
        chat.radio_close();
    }
    chat.end_frame();
}
