//! specs/combat_arms/radio_chat.md's test cases: the radio (1-13), chat
//! routing and colours (14-23), text commands (24-32), the filter
//! (33-38), the limits (39-48), whispers (49-51) and voice (52-54). The
//! cases are played on a small room of players (`Room`): each has its own
//! `ChatClient`, the server's side is the plugin's own relay rules
//! (`reads_say`, `hears_radio`, `ServerLimiter`) and the receiver's is
//! `receive_say` / `receive_radio`, the functions the game's systems call.
//! The wire (a server and two clients) and a game of one are `net_path`
//! and `standalone` below; the HUD's panels are tests/ui/radio_chat.rs.

use bevy::prelude::*;
use chimera_core::core::{Dead, Health, LocalPlayer, Team};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use combat_arms::{
    game::chat::{
        self, CaChat, CaRadioIn, CaRadioRequest, CaSayIn, CaSayRequest, CaVoice, ChatUiState,
        client::{AUTO_REPLY_TITLE, ChatClient, Env, InputKey, Out, Tab},
        limits::{Region, ServerLimiter},
        radio::{self, RadioData, RadioSound},
        show::{ChatLog, PanelLine, Peer, colours},
        text::{BadWords, BlockList, Cmd, Commands},
        voice::{self, Channel, NoChannel, VoiceKeys, VoiceMode, Voicer},
    },
    game::movement,
    harness::CaSim,
};

/// The radio file of the spec's test cases.
const RADIO_FILE: &str = r#"
[SoundTemplate]
Template0 = "Male"
Template1 = "Female"

[RadioClass]
RadioClass0 = "Report"
RadioClass1 = "Answer"
RadioClass2 = "Request"

[Report0]
ListText = "Enemy spotted"
Male_Snd0 = "male0.wav"
Male_Snd1 = "male1.wav"
Female_Snd0 = "female0.wav"
Chat0 = "Enemy spotted!"
Chat1 = "Contact!"
SendToEnemy = 0

[Answer0]
SendToEnemy = 1
3Dsound = 0
Male_Snd0 = "male_roger.wav"
Chat0 = "Roger"
"#;

fn data() -> RadioData {
    RadioData::parse(RADIO_FILE).expect("the spec's radio file")
}

/// A player in the room.
struct P {
    name: &'static str,
    team: u8,
    alive: bool,
    colour: u32,
    chat: ChatClient,
    at: [f32; 3],
    voice: Option<&'static str>,
}

impl P {
    fn peer(&self) -> Peer {
        Peer {
            name: self.name.into(),
            team: self.team,
            alive: self.alive,
        }
    }

    fn env(&self) -> Env {
        Env {
            own: self.name.into(),
            alive: self.alive,
            ..Default::default()
        }
    }

    fn last(&self) -> Option<&PanelLine> {
        self.chat.log.lines.back()
    }

    fn lines(&self) -> usize {
        self.chat.log.lines.len()
    }
}

/// A1, A2 on Alpha, B1 on Bravo, S a GM spectator on team 255 (index
/// order).
struct Room {
    players: Vec<P>,
    data: RadioData,
    hunted: bool,
    limiters: Vec<ServerLimiter>,
    /// Sounds each player heard from the last radio call.
    heard: Vec<Option<RadioSound>>,
}

const A1: usize = 0;
const A2: usize = 1;
const B1: usize = 2;
const S: usize = 3;

impl Room {
    fn new() -> Self {
        let p = |name, team, at| P {
            name,
            team,
            alive: true,
            colour: 0,
            chat: ChatClient::new(Region::NON_BR),
            at,
            voice: Some("Male"),
        };
        Self {
            players: vec![
                p("A1", 0, [100.0, 0.0, 0.0]),
                p("A2", 0, [0.0; 3]),
                p("B1", 1, [0.0; 3]),
                p("S", 255, [0.0; 3]),
            ],
            data: data(),
            hunted: false,
            limiters: vec![ServerLimiter::default(); 4],
            heard: vec![None; 4],
        }
    }

    fn peers(&self) -> Vec<Peer> {
        self.players.iter().map(P::peer).collect()
    }

    /// The server relays a chat line from `from`; each reader's client
    /// shows it. Returns who got it.
    fn deliver_say(&mut self, from: usize, team: bool, text: &str, now: f64) -> Vec<usize> {
        let text = chat::clean(text);
        if !self.limiters[from].accept(now, &text) {
            return Vec::new();
        }
        let peers = self.peers();
        let msg = CaSayIn {
            sender: Some(Entity::PLACEHOLDER),
            name: peers[from].name.clone(),
            name_colour: self.players[from].colour,
            team,
            text,
        };
        let mut got = Vec::new();
        for i in 0..self.players.len() {
            if !chat::reads_say(team, peers[from].team, peers[i].team) {
                continue;
            }
            got.push(i);
            let hunted = self.hunted;
            chat::receive_say(&mut self.players[i].chat, &peers[i], Some(&peers[from]), &msg, hunted, now);
        }
        got
    }

    /// The server relays a radio call (no checks); each hearer's client
    /// shows and plays it.
    fn deliver_radio(&mut self, from: usize, send: radio::RadioSend, now: f64) -> Vec<usize> {
        let peers = self.peers();
        let mut got = Vec::new();
        for i in 0..self.players.len() {
            if !chat::hears_radio(send.to_all, peers[from].team, peers[i].team) {
                continue;
            }
            got.push(i);
            let (who, colour) = if send.to_all {
                (from, self.players[from].colour)
            } else {
                (i, self.players[i].colour)
            };
            let msg = CaRadioIn {
                sender: Some(Entity::PLACEHOLDER),
                name: peers[from].name.clone(),
                name_colour: colour,
                at: send.at,
                class: send.class,
                entry: send.entry,
                r: send.r,
                t: send.t,
            };
            let hunted = self.hunted;
            let data = self.data.clone();
            self.heard[i] = chat::receive_radio(&mut self.players[i].chat, &data, &peers[i], Some(&peers[who]), &msg, hunted, now);
        }
        got
    }

    /// `who` types `text` and presses Enter at `now`; what the sender's
    /// client sends is relayed. Returns the readers.
    fn type_line(&mut self, who: usize, text: &str, now: f64) -> Vec<usize> {
        let outs = self.type_outs(who, text, now);
        let mut got = Vec::new();
        for out in outs {
            if let Out::Say { team, text } = out {
                got = self.deliver_say(who, team, &text, now);
            }
        }
        got
    }

    fn type_outs(&mut self, who: usize, text: &str, now: f64) -> Vec<Out> {
        let env = self.players[who].env();
        let chat = &mut self.players[who].chat;
        chat.sent_this_frame = false;
        assert!(chat.open_with_enter(&env), "the input opens");
        chat.type_text(text);
        chat.key(InputKey::Enter, &env, now)
    }

    /// `who` opens `class`'s menu and picks entry `n` (draw `r`).
    fn radio(&mut self, who: usize, class: u8, n: usize, r: usize, now: f64) -> Option<radio::RadioSend> {
        let env = self.players[who].env();
        let p = &mut self.players[who];
        p.chat.radio_key(class, &env, now);
        let data = self.data.clone();
        match p.chat.radio_choose(n, &data, p.voice, p.at, move |_| r, now) {
            Some(Out::Radio(s)) => Some(s),
            _ => None,
        }
    }
}

// --- Radio

/// 1. The report key opens the menu: title "Report", one line "Enemy
/// spotted", lines 2 to 9 hidden.
#[test]
fn case_01_report_menu() {
    let mut room = Room::new();
    let env = room.players[A1].env();
    room.players[A1].chat.radio_key(radio::REPORT, &env, 0.0);
    assert_eq!(room.players[A1].chat.radio_open, Some(radio::REPORT));
    let (title, entries) = room.data.menu(radio::REPORT).unwrap();
    assert_eq!(title, "Report");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].list_text, "Enemy spotted");
}

/// 2. Picking 1 with the draw giving r = 1: A1 and A2 show `[Report] A1 :
/// Contact!` in the radio colour for 10000 ms and play `male1.wav` in 3D
/// at (100, 0, 0), outer 1000 u, inner 70 u; B1 and S get nothing.
#[test]
fn case_02_report_to_the_team() {
    let mut room = Room::new();
    let send = room.radio(A1, radio::REPORT, 1, 1, 0.0).expect("sent");
    assert_eq!((send.class, send.entry, send.r, send.t, send.to_all), (0, 0, 1, 0, false));
    let got = room.deliver_radio(A1, send, 0.0);
    assert_eq!(got, [A1, A2]);
    for who in [A1, A2] {
        let line = room.players[who].last().expect("a line");
        assert_eq!(line.text, "[Report] A1 : Contact!");
        assert_eq!(line.colour, colours::RADIO);
        assert_eq!(line.life, 10_000.0);
        let sound = room.heard[who].clone().expect("a sound");
        assert_eq!(sound.file, "male1.wav");
        let played = chat::play(sound);
        assert_eq!(played.at, Some(Vec3::new(100.0, 0.0, 0.0)));
        assert_eq!(played.radii, (70.0, 1000.0));
    }
    assert_eq!((room.players[B1].lines(), room.players[S].lines()), (0, 0));
    assert!(room.heard[B1].is_none() && room.heard[S].is_none());
}

/// 3. A Female sender (one file), the draw 0: `[Report] A1 : Enemy
/// spotted!`, sound `female0.wav`.
#[test]
fn case_03_female_voice() {
    let mut room = Room::new();
    room.players[A1].voice = Some("Female");
    let send = room.radio(A1, radio::REPORT, 1, 0, 0.0).unwrap();
    assert_eq!((send.t, send.r), (1, 0));
    room.deliver_radio(A1, send, 0.0);
    assert_eq!(room.players[A2].last().unwrap().text, "[Report] A1 : Enemy spotted!");
    assert_eq!(room.heard[A2].as_ref().unwrap().file, "female0.wav");
}

/// 4. A Female message with t = 1 and r = 1 (impossible from a real
/// client): the line `Contact!`, the sound falls back to file 0.
#[test]
fn case_04_constructed_message_falls_back() {
    let mut room = Room::new();
    let send = radio::RadioSend {
        at: [100.0, 0.0, 0.0],
        class: 0,
        entry: 0,
        r: 1,
        to_all: false,
        t: 1,
    };
    room.deliver_radio(A1, send, 0.0);
    assert_eq!(room.players[A2].last().unwrap().text, "[Report] A1 : Contact!");
    assert_eq!(room.heard[A2].as_ref().unwrap().file, "female0.wav");
}

/// 5. The report key again 1500 ms after the send: nothing opens; at
/// 2000 ms it opens.
#[test]
fn case_05_cooldown() {
    let mut room = Room::new();
    room.radio(A1, radio::REPORT, 1, 1, 0.0).unwrap();
    let env = room.players[A1].env();
    let chat = &mut room.players[A1].chat;
    chat.radio_key(radio::REPORT, &env, 1500.0);
    assert_eq!(chat.radio_open, None);
    chat.radio_key(radio::REPORT, &env, 2000.0);
    assert_eq!(chat.radio_open, Some(radio::REPORT));
}

/// 6. Report open, then the answer key: the answer menu replaces it;
/// the answer key again closes it.
#[test]
fn case_06_switching_and_toggling() {
    let mut room = Room::new();
    let env = room.players[A1].env();
    let chat = &mut room.players[A1].chat;
    chat.radio_key(radio::REPORT, &env, 0.0);
    chat.radio_key(radio::ANSWER, &env, 1.0);
    assert_eq!(chat.radio_open, Some(radio::ANSWER));
    chat.radio_key(radio::ANSWER, &env, 2.0);
    assert_eq!(chat.radio_open, None);
}

/// 7. The answer menu: 0 sends nothing and leaves it open; 2 (only one
/// entry) sends nothing and leaves it open.
#[test]
fn case_07_dead_keys_leave_the_menu() {
    let mut room = Room::new();
    let env = room.players[A1].env();
    room.players[A1].chat.radio_key(radio::ANSWER, &env, 0.0);
    let data = room.data.clone();
    for n in [0, 2] {
        let out = room.players[A1].chat.radio_choose(n, &data, Some("Male"), [0.0; 3], |_| 0, 1.0);
        assert_eq!(out, None);
        assert_eq!(room.players[A1].chat.radio_open, Some(radio::ANSWER));
    }
    assert_eq!(room.players[A1].last_radio(), None);
}

impl P {
    fn last_radio(&self) -> Option<f64> {
        self.chat.last_radio
    }
}

/// 8. `Answer0` (everyone hears it, no 3D): A1, A2, B1 and S all show
/// `[Answer] A1 : Roger` in the radio colour and play `male_roger.wav` as
/// a plain sound at full volume.
#[test]
fn case_08_answer_to_everyone() {
    let mut room = Room::new();
    let send = room.radio(A1, radio::ANSWER, 1, 0, 0.0).unwrap();
    assert!(send.to_all);
    let got = room.deliver_radio(A1, send, 0.0);
    assert_eq!(got, [A1, A2, B1, S]);
    for who in 0..4 {
        let line = room.players[who].last().unwrap();
        assert_eq!((line.text.as_str(), line.colour), ("[Answer] A1 : Roger", colours::RADIO));
        let sound = room.heard[who].clone().unwrap();
        assert_eq!(sound.file, "male_roger.wav");
        assert_eq!(sound.at, None);
        let played = chat::play(sound);
        assert_eq!((played.at, played.volume), (None, 100.0));
    }
}

/// 9. A dead player pressing a radio key: no menu.
#[test]
fn case_09_dead_cannot_open() {
    let mut room = Room::new();
    room.players[A1].alive = false;
    let env = room.players[A1].env();
    room.players[A1].chat.radio_key(radio::REPORT, &env, 0.0);
    assert_eq!(room.players[A1].chat.radio_open, None);
}

/// 10. The report menu open and the player killed: the menu goes when
/// the player starts dying (in a world).
#[test]
fn case_10_dying_closes_the_menu() {
    let mut sim = world();
    let w = sim.app.world_mut();
    chat::radio_key(w, radio::REPORT);
    assert_eq!(w.resource::<CaChat>().radio_open, Some(radio::REPORT));
    let me = local(w);
    w.entity_mut(me).insert(Dead { since: 0.0 });
    sim.ticks(2);
    assert_eq!(sim.app.world().resource::<CaChat>().radio_open, None);
}

/// The radio's keys are Combat Arms' own: a player who is another game's
/// (a CS:S gun in hand in a party app, `GameIdentity`) opens no menu
/// (playtest 2026-11-17: B opened the radio over a CS:S player's Change
/// gun); Combat Arms' again, it opens.
#[test]
fn the_radio_is_only_a_combat_arms_players() {
    let mut sim = world();
    let w = sim.app.world_mut();
    let me = local(w);
    w.entity_mut(me).insert(chimera_core::slots::GameIdentity("cs_source".into()));
    chat::radio_key(w, radio::REPORT);
    assert_eq!(w.resource::<CaChat>().radio_open, None, "a CS:S player has no radio");
    w.entity_mut(me).insert(chimera_core::slots::GameIdentity(combat_arms::game::GAME.into()));
    chat::radio_key(w, radio::REPORT);
    assert_eq!(w.resource::<CaChat>().radio_open, Some(radio::REPORT));
}

/// 11. A2 has A1 on the block list: no line, but the sound plays.
#[test]
fn case_11_muting_keeps_the_sound() {
    let mut room = Room::new();
    assert!(room.players[A2].chat.block.add("A1", "A2"));
    let send = room.radio(A1, radio::REPORT, 1, 1, 0.0).unwrap();
    room.deliver_radio(A1, send, 0.0);
    assert_eq!(room.players[A2].lines(), 0);
    assert_eq!(room.heard[A2].as_ref().unwrap().file, "male1.wav");
    assert_eq!(room.players[A1].lines(), 1);
}

/// 12. The report menu open and a number key pressed while it is also the
/// primary weapon key: entry 1 is sent and the key is swallowed (the
/// caller gets `true`, so the weapon selection never sees it).
#[test]
fn case_12_number_keys_are_swallowed() {
    let mut sim = world();
    let w = sim.app.world_mut();
    assert!(!chat::radio_number(w, 1), "no menu open: the weapon key is the game's");
    chat::radio_key(w, radio::REPORT);
    assert!(chat::radio_number_with(w, 1, Some(1)));
    let sent: Vec<CaRadioRequest> = w.resource_mut::<Messages<CaRadioRequest>>().drain().collect();
    assert_eq!(sent.len(), 1);
    assert_eq!((sent[0].class, sent[0].entry), (0, 0));
    assert_eq!(w.resource::<CaChat>().radio_open, None);
}

/// 13. A voice set not in the list: t = 255; receivers show the text
/// with chat line 0 and play nothing.
#[test]
fn case_13_unknown_voice_set() {
    let mut room = Room::new();
    room.players[A1].voice = Some("Robot");
    let send = room.radio(A1, radio::REPORT, 1, 1, 0.0).unwrap();
    assert_eq!(send.t, 255);
    room.deliver_radio(A1, send, 0.0);
    assert_eq!(room.players[A2].last().unwrap().text, "[Report] A1 : Enemy spotted!");
    assert!(room.heard[A2].is_none());
}

// --- Chat routing and colours

fn colour_of(room: &Room, who: usize) -> Option<[u8; 4]> {
    room.players[who].last().map(|l| l.colour)
}

/// 14. A1 sends "hi" on All: every client shows `A1 : hi`; A1 and A2 in
/// green, B1 and S (team 255) in red; 10000 ms.
#[test]
fn case_14_all_chat() {
    let mut room = Room::new();
    let got = room.type_line(A1, "hi", 0.0);
    assert_eq!(got, [A1, A2, B1, S]);
    for who in 0..4 {
        let l = room.players[who].last().unwrap();
        assert_eq!((l.text.as_str(), l.life), ("A1 : hi", 10_000.0));
    }
    assert_eq!(colour_of(&room, A1), Some(colours::ALL_TEAMMATE));
    assert_eq!(colour_of(&room, A2), Some(colours::ALL_TEAMMATE));
    assert_eq!(colour_of(&room, B1), Some(colours::ALL_ENEMY));
    assert_eq!(colour_of(&room, S), Some(colours::ALL_ENEMY));
}

/// 15. A1 sends "push" on Team: A1 in white, A2 and S in grey, B1
/// nothing.
#[test]
fn case_15_team_chat() {
    let mut room = Room::new();
    room.players[A1].chat.tab = Tab::Team;
    let got = room.type_line(A1, "push", 0.0);
    assert_eq!(got, [A1, A2, S]);
    assert_eq!(colour_of(&room, A1), Some(colours::OWN_TEAM));
    assert_eq!(colour_of(&room, A2), Some(colours::TEAM));
    assert_eq!(colour_of(&room, S), Some(colours::TEAM));
    assert_eq!(room.players[B1].lines(), 0);
}

/// 16. A dead A1's team chat in Elimination: A2 (alive) and S show it.
#[test]
fn case_16_dead_team_chat() {
    let mut room = Room::new();
    room.players[A1].alive = false;
    room.players[A1].chat.tab = Tab::Team;
    room.type_line(A1, "rush B", 0.0);
    assert_eq!(room.players[A2].last().unwrap().text, "A1 : rush B");
    assert_eq!(room.players[S].last().unwrap().text, "A1 : rush B");
}

/// 17. Last Man Standing, A1 dead (HUD dead), Enter on "gg": the input
/// closes, nothing is sent, no notice.
#[test]
fn case_17_lms_dead_cannot_chat() {
    let mut p = Room::new().players.remove(A1);
    p.alive = false;
    let env = Env {
        lms_dead: true,
        ..p.env()
    };
    assert!(p.chat.open_with_enter(&env));
    p.chat.type_text("gg");
    let out = p.chat.key(InputKey::Enter, &env, 0.0);
    assert!(out.is_empty());
    assert!(!p.chat.is_open());
    assert_eq!(p.lines(), 0);
}

/// 18. Hunted, A1 dead sends All "lol": B1 (alive) shows nothing; A2
/// (dead) shows `A1 : lol`.
#[test]
fn case_18_hunted_living_do_not_read_the_dead() {
    let mut room = Room::new();
    room.hunted = true;
    room.players[A1].alive = false;
    room.players[A2].alive = false;
    room.type_line(A1, "lol", 0.0);
    assert_eq!(room.players[B1].lines(), 0);
    assert_eq!(room.players[A2].last().unwrap().text, "A1 : lol");
}

/// 19. One Man Army (everyone on Alpha): every receiver shows A1's All
/// line in green.
#[test]
fn case_19_one_team_modes_are_green() {
    let mut room = Room::new();
    for p in &mut room.players {
        p.team = 0;
    }
    room.type_line(A1, "hi", 0.0);
    for who in 0..4 {
        assert_eq!(colour_of(&room, who), Some(colours::ALL_TEAMMATE), "{who}");
    }
}

/// 20. A player named `GMx` sends All "hello": every receiver shows it in
/// the GM colour.
#[test]
fn case_20_gm_colour() {
    let mut room = Room::new();
    room.players[A1].name = "GMx";
    room.type_line(A1, "hello", 0.0);
    for who in 0..4 {
        assert_eq!(colour_of(&room, who), Some(colours::GM), "{who}");
    }
}

/// 21. A1 has a name colour; A2 receives "A1 rocks": the line is `A1 : A1
/// rocks` with the first `A1` in the name colour.
#[test]
fn case_21_name_colour() {
    let mut room = Room::new();
    room.players[A1].colour = 0x112233;
    room.type_line(A1, "A1 rocks", 0.0);
    let l = room.players[A2].last().unwrap();
    assert_eq!(l.text, "A1 : A1 rocks");
    assert_eq!(l.name, Some((0, 2, [0x11, 0x22, 0x33, 255])));
    assert_eq!(l.colour, colours::ALL_TEAMMATE);
}

/// 22. F3 then Esc, then Enter: the input opens on Team (the tab
/// persists).
#[test]
fn case_22_tab_persists() {
    let mut p = Room::new().players.remove(A1);
    let env = p.env();
    assert!(p.chat.open_with_bind(Tab::Team, &env));
    p.chat.key(InputKey::Esc, &env, 0.0);
    assert!(!p.chat.is_open());
    p.chat.sent_this_frame = false;
    assert!(p.chat.open_with_enter(&env));
    assert_eq!(p.chat.tab, Tab::Team);
}

/// 23. Entering a new map: the tab is All.
#[test]
fn case_23_new_map_resets_the_tab() {
    let mut sim = world();
    let w = sim.app.world_mut();
    w.resource_mut::<CaChat>().tab = Tab::Team;
    let loads = w.get_resource::<chimera_core::core::MapLoads>().map_or(0, |m| m.0);
    w.insert_resource(chimera_core::core::MapLoads(loads + 1));
    sim.ticks(2);
    assert_eq!(sim.app.world().resource::<CaChat>().tab, Tab::All);
}

// --- Commands

fn command_set() -> Commands {
    Commands::from_values(&[
        (Cmd::Whisper, "/w/whisper"),
        (Cmd::Normal, "/a/all"),
        (Cmd::Block, "/block"),
        (Cmd::WhisperIgnore, "/ignore"),
    ])
}

fn typist() -> Room {
    let mut room = Room::new();
    for p in &mut room.players {
        p.chat.settings.commands = command_set();
    }
    room
}

/// 24 (corrected 2026-10-22): the first matching alias in the listed order
/// counts. With `/w/whisper`, `/whisper Bob hi there` matches `/w`: a
/// whisper to "hisper" with the body "Bob hi there".
#[test]
fn case_24_whisper_command() {
    let mut room = typist();
    let outs = room.type_outs(A1, "/whisper Bob hi there", 0.0);
    assert_eq!(
        outs,
        [Out::Whisper {
            to: "hisper".into(),
            body: "Bob hi there".into()
        }]
    );
    let l = room.players[A1].last().unwrap();
    assert_eq!(l.text, "To \"hisper\" : Bob hi there");
    assert_eq!(l.colour, [66, 158, 83, 255]);
    assert_eq!(l.life, 10_000.0);
}

/// 24, the other order: with `/whisper/w` the long alias comes first, so
/// `/whisper Bob hi there` is a whisper to Bob, and `/wave` still matches
/// `/w`.
#[test]
fn case_24_whisper_command_long_alias_first() {
    let mut room = Room::new();
    for p in &mut room.players {
        p.chat.settings.commands = Commands::from_values(&[(Cmd::Whisper, "/whisper/w")]);
    }
    let outs = room.type_outs(A1, "/whisper Bob hi there", 0.0);
    assert_eq!(outs, [Out::Whisper { to: "Bob".into(), body: "hi there".into() }]);
    assert!(room.type_outs(A1, "/wave", 0.0).is_empty());
}

/// Corrections 2026-10-22 (3): a setting command followed only by spaces
/// is sent whole as chat; the block list and the auto-reply stay.
#[test]
fn setting_command_with_only_spaces_is_chat() {
    let mut room = Room::new();
    for p in &mut room.players {
        p.chat.settings.commands = Commands::from_values(&[
            (Cmd::Block, "/block"),
            (Cmd::UnBlock, "/unblock"),
            (Cmd::WhisperAutoAnswer, "/auto"),
        ]);
    }
    for (i, line) in ["/block   ", "/unblock   ", "/auto   "].into_iter().enumerate() {
        let outs = room.type_outs(A1, line, i as f64 * 20_000.0);
        assert_eq!(outs, [Out::Say { team: false, text: line.into() }], "{line:?}");
    }
    assert!(room.players[A1].chat.settings.auto_reply.is_empty());
}

/// 25. `/w Bob`: nothing sent; the input closes.
#[test]
fn case_25_whisper_without_a_body() {
    let mut room = typist();
    assert!(room.type_outs(A1, "/w Bob", 0.0).is_empty());
    assert!(!room.players[A1].chat.is_open());
}

/// 26. `/wave` matches `/w` (no word boundary): "ave" is the target with
/// no body; nothing sent.
#[test]
fn case_26_no_word_boundary() {
    let mut room = typist();
    assert!(room.type_outs(A1, "/wave", 0.0).is_empty());
    assert!(!room.players[A1].chat.is_open());
    assert_eq!(room.players[A1].lines(), 0);
}

/// 27. On the Team tab, `/a go` sends " go" as team chat: Alpha shows
/// `A1 :  go` (two spaces).
#[test]
fn case_27_normal_command_keeps_the_tab() {
    let mut room = typist();
    room.players[A1].chat.tab = Tab::Team;
    let outs = room.type_outs(A1, "/a go", 0.0);
    assert_eq!(
        outs,
        [Out::Say {
            team: true,
            text: " go".into()
        }]
    );
    room.deliver_say(A1, true, " go", 0.0);
    assert_eq!(room.players[A2].last().unwrap().text, "A1 :  go");
    assert_eq!(colour_of(&room, A2), Some(colours::TEAM));
}

/// 28. On the Clan tab, `/a hi` goes to All.
#[test]
fn case_28_normal_command_from_the_clan_tab() {
    let mut room = typist();
    room.players[A1].chat.tab = Tab::Clan;
    let outs = room.type_outs(A1, "/a hi", 0.0);
    assert_eq!(
        outs,
        [Out::Say {
            team: false,
            text: " hi".into()
        }]
    );
}

/// 29. `/ignore Bob` is not handled: it goes out as All chat with the
/// command text.
#[test]
fn case_29_unhandled_commands_are_broadcast() {
    let mut room = typist();
    room.type_line(A1, "/ignore Bob", 0.0);
    for who in 0..4 {
        assert_eq!(room.players[who].last().unwrap().text, "A1 : /ignore Bob");
    }
}

/// 30. `/block A2`: a red reply; A2's later lines and radio text are no
/// longer shown to A1; A2 is not told.
#[test]
fn case_30_block_command() {
    let mut room = typist();
    room.type_outs(A1, "/block A2", 0.0);
    let reply = room.players[A1].last().unwrap().clone();
    assert_eq!((reply.colour, reply.life), (colours::SYSTEM, 10_000.0));
    let before = room.players[A1].lines();
    room.type_line(A2, "hello", 1.0);
    let send = room.radio(A2, radio::REPORT, 1, 1, 1.0).unwrap();
    room.deliver_radio(A2, send, 1.0);
    assert_eq!(room.players[A1].lines(), before, "nothing new for A1");
    assert_eq!(room.players[B1].lines(), 1, "the others read it");
    assert_eq!(room.players[A2].lines(), 2, "A2 sees its own lines and is told nothing else");
}

/// 31. The block entry `bo?` drops `Bob` and `BOX`, not `Bobby`.
#[test]
fn case_31_wildcard_question_mark() {
    let list = BlockList {
        entries: vec!["bo?".into()],
    };
    assert!(list.blocks("Bob", "A1"));
    assert!(list.blocks("BOX", "A1"));
    assert!(!list.blocks("Bobby", "A1"));
}

/// 32. The entry `*` drops everyone but names starting `GM` / `gm` and
/// your own.
#[test]
fn case_32_wildcard_star() {
    let list = BlockList {
        entries: vec!["*".into()],
    };
    assert!(list.blocks("A2", "A1"));
    assert!(!list.blocks("GMx", "A1"));
    assert!(!list.blocks("gmx", "A1"));
    assert!(!list.blocks("A1", "A1"));
}

// --- The filter

fn words() -> BadWords {
    BadWords {
        convert: vec!["abc".into(), "xyzzy".into()],
        whitelist: vec!["zabcz".into()],
        live: Vec::new(),
    }
}

/// 33-37: the filter's examples.
#[test]
fn case_33_to_37_filter() {
    let w = words();
    assert_eq!(w.filter("ABC def"), "** def", "33");
    assert_eq!(w.filter("fooabcbar"), "foo**bar", "34");
    assert_eq!(w.filter("zabcz abc"), "zabcz abc", "35");
    assert_eq!(w.filter("xyzzy and xyzzy"), "** and xyzzy", "36");
    assert_eq!(w.filter("abc abc"), "** **", "37");
}

/// 38. A sender named `abcman` sends All "hi": filtering receivers show
/// `**man : hi` with no name colour; the others `abcman : hi`.
#[test]
fn case_38_filtered_names() {
    let mut room = Room::new();
    room.players[A1].name = "abcman";
    room.players[A1].colour = 0x112233;
    room.players[A2].chat.settings.filter_on = true;
    room.players[A2].chat.settings.words = words();
    room.players[B1].chat.settings.words = words();
    room.type_line(A1, "hi", 0.0);
    let filtered = room.players[A2].last().unwrap();
    assert_eq!((filtered.text.as_str(), filtered.name), ("**man : hi", None));
    let plain = room.players[B1].last().unwrap();
    assert_eq!(plain.text, "abcman : hi");
    assert!(plain.name.is_some());
}

// --- Limits

fn say_at(p: &mut P, text: &str, now: f64) -> Vec<Out> {
    let env = p.env();
    p.chat.sent_this_frame = false;
    p.chat.input = Some(text.into());
    p.chat.submit(&env, now)
}

fn last_text(p: &P) -> String {
    p.last().map(|l| l.text.clone()).unwrap_or_default()
}

/// 39. Twenty different lines at t = 0..19 s; the 21st at 20 s is
/// refused (red notice) and a block runs to 200 s: 100 s and 199 s are
/// refused with a notice each; 200 s sends.
#[test]
fn case_39_client_rate_limit() {
    let mut room = Room::new();
    let p = &mut room.players[A1];
    for i in 0..20 {
        assert_eq!(say_at(p, &format!("m{i}"), i as f64 * 1000.0).len(), 1, "{i}");
    }
    for t in [20_000.0, 100_000.0, 199_000.0] {
        let before = p.lines();
        assert!(say_at(p, "late", t).is_empty(), "{t}");
        assert_eq!(p.lines(), before + 1, "a notice at {t}");
        let n = p.last().unwrap();
        assert_eq!((n.text.as_str(), n.colour, n.life), ("$SpamBlockChat", colours::SYSTEM, 10_000.0));
    }
    assert_eq!(say_at(p, "late", 200_000.0).len(), 1);
}

/// 40. Twenty lines at 0..19 s, then at 60 s: sent (the slot from t = 0
/// is free).
#[test]
fn case_40_slot_frees_after_the_window() {
    let mut room = Room::new();
    let p = &mut room.players[A1];
    for i in 0..20 {
        say_at(p, &format!("m{i}"), i as f64 * 1000.0);
    }
    assert_eq!(say_at(p, "again", 60_000.0).len(), 1);
}

/// 41. Server only: 21 different messages at t = 0..20 s: the 21st is
/// dropped and the block ends at 200 s; 199.999 s dropped, 200 s
/// accepted.
#[test]
fn case_41_server_rate_limit() {
    let mut s = ServerLimiter::default();
    for i in 0..20 {
        assert!(s.accept(i as f64 * 1000.0, &format!("m{i}")), "{i}");
    }
    assert!(!s.accept(20_000.0, "m20"));
    assert!(!s.accept(199_999.0, "x"));
    assert!(s.accept(200_000.0, "x"));
}

/// 42. "go" eleven times, 1 s apart: sends 1 to 10 leave the client,
/// receivers show the first 9 (the server drops the 10th and blocks A1 to
/// 69 s); the 11th attempt at 10 s is refused with the same-sentence
/// notice for 5000 ms; any line at 68.999 s is refused; at 69 s it is
/// sent and shown.
#[test]
fn case_42_repeat_lock() {
    let mut room = Room::new();
    let mut sent = 0;
    for i in 0..10 {
        let t = i as f64 * 1000.0;
        let outs = say_at(&mut room.players[A1], "go", t);
        assert_eq!(outs.len(), 1, "send {}", i + 1);
        sent += 1;
        room.deliver_say(A1, false, "go", t);
    }
    assert_eq!(sent, 10);
    assert_eq!(room.players[A2].lines(), 9, "the server dropped the tenth");
    assert!(say_at(&mut room.players[A1], "go", 10_000.0).is_empty());
    let n = room.players[A1].last().unwrap();
    assert_eq!((n.text.as_str(), n.life), ("$SameSentence", 5000.0));
    assert!(say_at(&mut room.players[A1], "other", 68_999.0).is_empty());
    let outs = say_at(&mut room.players[A1], "other", 69_000.0);
    assert_eq!(outs.len(), 1);
    room.deliver_say(A1, false, "other", 69_000.0);
    assert_eq!(room.players[A2].last().unwrap().text, "A1 : other");
}

/// 43. "a", "b", "a", "b", ... alternating never locks.
#[test]
fn case_43_alternating_lines() {
    let mut room = Room::new();
    for i in 0..30 {
        let text = if i % 2 == 0 { "a" } else { "b" };
        assert_eq!(say_at(&mut room.players[A1], text, i as f64 * 5000.0).len(), 1, "{i}");
    }
}

/// 44. "hey" to Bob four times within 10 s: three go out; the 4th shows
/// `$UserSpamBlockWhisper` and the input stays open. A different text at
/// +30 s is refused; any text more than 60 s after the 4th is sent.
#[test]
fn case_44_whisper_repeats() {
    let mut room = typist();
    let p = &mut room.players[A1];
    let mut sent = 0;
    for i in 0..4 {
        let outs = say_at(p, "/w Bob hey", i as f64 * 3000.0);
        sent += outs.len();
    }
    assert_eq!(sent, 3);
    assert_eq!(last_text(p), "$UserSpamBlockWhisper");
    assert!(p.chat.is_open(), "the input stays open");
    assert!(say_at(p, "/w Bob other", 9000.0 + 30_000.0).is_empty());
    assert_eq!(say_at(p, "/w Bob other", 9000.0 + 60_001.0).len(), 1);
}

/// 45. Twenty different people once each within 100 s; a 21st whisper at
/// 120 s shows `$BlockWhisper`; 180 s after the first it is sent.
#[test]
fn case_45_global_whisper_limit() {
    let mut room = typist();
    let p = &mut room.players[A1];
    for i in 0..20 {
        assert_eq!(say_at(p, &format!("/w P{i} hi"), i as f64 * 5000.0).len(), 1);
    }
    assert!(say_at(p, "/w Last hi", 120_000.0).is_empty());
    assert_eq!(last_text(p), "$BlockWhisper");
    assert_eq!(say_at(p, "/w Last hi", 180_000.0).len(), 1);
}

/// 46. BR build: 5 All lines within 2 s; the 6th at 3 s is refused,
/// blocked until 63 s.
#[test]
fn case_46_br_limits() {
    let mut p = Room::new().players.remove(A1);
    p.chat = ChatClient::new(Region::BR);
    for i in 0..5 {
        assert_eq!(say_at(&mut p, &format!("m{i}"), i as f64 * 500.0).len(), 1);
    }
    assert!(say_at(&mut p, "m5", 3000.0).is_empty());
    assert!(say_at(&mut p, "m5", 62_999.0).is_empty());
    assert_eq!(say_at(&mut p, "m5", 63_000.0).len(), 1);
}

/// 47. Lobby: "sell" three times all go out; a 4th line of any text
/// within 60000 ms shows the same-sentence notice and is not sent.
#[test]
fn case_47_lobby_repeat_lock() {
    let mut p = Room::new().players.remove(A1);
    for i in 0..3 {
        assert!(p.chat.lobby_send("sell", i as f64 * 1000.0));
    }
    assert!(!p.chat.lobby_send("buy", 10_000.0));
    assert_eq!(last_text(&p), "$SameSentence");
    assert!(p.chat.lobby_send("buy", 62_001.0));
}

/// 48. A punished account with 125 minutes sends Team "x": a yellow
/// notice with 2 hours, nothing sent; with 30 minutes the "less than an
/// hour" notice.
#[test]
fn case_48_punishment() {
    let mut p = Room::new().players.remove(A1);
    p.chat.tab = Tab::Team;
    p.chat.settings.punished_minutes = 125;
    assert!(say_at(&mut p, "x", 0.0).is_empty());
    let n = p.last().unwrap().clone();
    assert!(n.text.contains("2 hours"), "{}", n.text);
    assert_eq!((n.colour, n.life), (colours::NOTICE, 10_000.0));
    p.chat.settings.punished_minutes = 30;
    assert!(say_at(&mut p, "x", 1.0).is_empty());
    assert!(p.last().unwrap().text.contains("less than an hour"));
}

// --- Whispers

/// 49. A1 sets the auto-reply with the command and the remainder " afk";
/// Bob whispers "yo": A1 shows the whisper in (60, 230, 92) and Bob gets
/// the title followed by " afk".
#[test]
fn case_49_auto_reply() {
    let mut p = Room::new().players.remove(A1);
    say_at(&mut p, "/autoanswer afk", 0.0);
    assert_eq!(p.chat.settings.auto_reply, " afk");
    let env = p.env();
    let outs = p.chat.whisper_in("Bob", "yo", &env, 1.0);
    let l = p.last().unwrap();
    assert_eq!((l.text.as_str(), l.colour), ("From \"Bob\" : yo", [60, 230, 92, 255]));
    assert_eq!(
        outs,
        [Out::Whisper {
            to: "Bob".into(),
            body: format!("{AUTO_REPLY_TITLE} afk")
        }]
    );
}

/// 50. Bob's own auto-reply arrives at A1: shown, and A1 sends none back.
#[test]
fn case_50_auto_replies_do_not_bounce() {
    let mut p = Room::new().players.remove(A1);
    p.chat.settings.auto_reply = " afk".into();
    let env = p.env();
    let outs = p.chat.whisper_in("Bob", &format!("{AUTO_REPLY_TITLE} brb"), &env, 0.0);
    assert!(outs.is_empty());
    assert_eq!(p.lines(), 1);
}

/// 51. Whispers from C, D, C, E: the list is E, D, C; Up shows `/w E `,
/// `/w D `, `/w C `, then `/w E ` again.
#[test]
fn case_51_recent_partners() {
    let mut p = Room::new().players.remove(A1);
    let env = p.env();
    for from in ["C", "D", "C", "E"] {
        p.chat.whisper_in(from, "hi", &env, 0.0);
    }
    assert_eq!(p.chat.whisper.recent, ["E", "D", "C"]);
    assert!(p.chat.open_with_enter(&env));
    let mut shown = Vec::new();
    for _ in 0..4 {
        p.chat.key(InputKey::Up, &env, 0.0);
        shown.push(p.chat.input.clone().unwrap());
    }
    assert_eq!(shown, ["/w E ", "/w D ", "/w C ", "/w E "]);
}

// --- Voice

/// 52. Spy Hunt, a Bravo player presses push-to-talk: a red "disabled"
/// line; no voice channel.
#[test]
fn case_52_spy_hunt_bravo_has_no_voice() {
    let ch = voice::channel(voice::mode_of("BNT"), Voicer { team: 1, alive: true });
    assert_eq!(ch, Err(NoChannel::NotAlpha));
    let mut keys = VoiceKeys::default();
    let line = keys.press(ch, 0.0).expect("a line");
    assert!(line.text.contains("disabled"));
    assert_eq!(line.colour, colours::SYSTEM);
    assert_eq!(voice::channel(voice::mode_of("BNT"), Voicer { team: 0, alive: true }), Ok(Channel::Alpha));
}

/// 53. Last Man Standing, A1 dies: it leaves the channel and push-to-talk
/// shows "disabled in this mode".
#[test]
fn case_53_lms_dead_leave_voice() {
    let mode = voice::mode_of("LMS");
    assert_eq!(mode, VoiceMode::SharedAlive);
    assert_eq!(voice::channel(mode, Voicer { team: 0, alive: true }), Ok(Channel::Alpha));
    let ch = voice::channel(mode, Voicer { team: 0, alive: false });
    assert_eq!(ch, Err(NoChannel::DeadInMode));
    let line = VoiceKeys::default().press(ch, 0.0).unwrap();
    assert!(line.text.contains("disabled in this mode"), "{}", line.text);
}

/// 54. Elimination, A1 holds push-to-talk for 3 s: the talk icon shows
/// for 3 s; only Alpha's channel hears A1.
#[test]
fn case_54_push_to_talk() {
    let mode = voice::mode_of("TSV");
    let players = [
        ("A1", Voicer { team: 0, alive: true }),
        ("A2", Voicer { team: 0, alive: true }),
        ("B1", Voicer { team: 1, alive: true }),
    ];
    let channels: Vec<(&str, Result<Channel, NoChannel>)> = players.iter().map(|(n, v)| (*n, voice::channel(mode, *v))).collect();
    let mut keys = VoiceKeys::default();
    assert!(keys.press(channels[0].1, 0.0).is_none());
    assert!(keys.talking);
    assert_eq!(voice::listeners(Channel::Alpha, &channels), ["A1", "A2"]);
    keys.release();
    assert!(!keys.talking);
}

// --- Building blocks the cases rely on

/// C2: the Enter that sends does not reopen the input on the same frame;
/// nothing opens during a fade, with the HUD hidden or with no team.
#[test]
fn open_conditions() {
    let mut p = Room::new().players.remove(A1);
    let env = p.env();
    assert!(p.chat.open_with_enter(&env));
    p.chat.type_text("x");
    p.chat.key(InputKey::Enter, &env, 0.0);
    assert!(!p.chat.open_with_enter(&env), "not on the frame a line was sent");
    p.chat.end_frame();
    for blocked in [
        Env { fading: true, ..env.clone() },
        Env { hud_hidden: true, ..env.clone() },
        Env { team_nothing: true, ..env.clone() },
        Env { playing: false, ..env.clone() },
    ] {
        assert!(!p.chat.open_with_enter(&blocked));
    }
    assert!(p.chat.open_with_enter(&env));
    assert!(!p.chat.open_with_enter(&env), "already open");
    // GM accounts' channel binds do nothing.
    let mut gm = Room::new().players.remove(A1);
    gm.chat.settings.gm = true;
    assert!(!gm.chat.open_with_bind(Tab::Team, &env));
    assert_eq!(gm.chat.tab, Tab::All);
}

/// R2: a vote on screen, a mid-way weapon prompt or an unbuilt HUD keep
/// the radio menu shut.
#[test]
fn radio_open_conditions() {
    let mut p = Room::new().players.remove(A1);
    let env = p.env();
    for blocked in [
        Env { vote_on: true, ..env.clone() },
        Env { weapon_prompt: true, ..env.clone() },
        Env { hud_built: false, ..env.clone() },
    ] {
        p.chat.radio_key(radio::REPORT, &blocked, 0.0);
        assert_eq!(p.chat.radio_open, None);
    }
    p.chat.radio_key(radio::REPORT, &env, 0.0);
    assert_eq!(p.chat.radio_open, Some(radio::REPORT));
}

/// The server's reading rules (C1, R4): all chat everyone; team chat the
/// team and the spectators; a team radio call only the sender's team.
#[test]
fn relay_rules() {
    for (sender, reader) in [(0, 0), (0, 1), (1, 255), (255, 255)] {
        assert!(chat::reads_say(false, sender, reader));
    }
    assert!(chat::reads_say(true, 0, 0) && chat::reads_say(true, 0, 255));
    assert!(!chat::reads_say(true, 0, 1));
    assert!(chat::hears_radio(true, 0, 255) && chat::hears_radio(true, 0, 1));
    assert!(chat::hears_radio(false, 0, 0));
    assert!(!chat::hears_radio(false, 0, 255) && !chat::hears_radio(false, 0, 1));
}

/// A line of spaces is sent; the 120th keystroke is ignored; a message at
/// the limit is whole.
#[test]
fn edge_lengths() {
    let mut room = Room::new();
    assert_eq!(room.type_line(A1, "   ", 0.0).len(), 4);
    let outs = room.type_outs(A1, &"x".repeat(200), 1.0);
    let Out::Say { text, .. } = &outs[0] else { panic!() };
    assert_eq!(text.chars().count(), 119);
}

/// Team radio carries the receiver's own id: the Hunted dead-sender check
/// never hides it (Quirk 1).
#[test]
fn quirk_01_team_radio_ignores_the_dead_sender_rule() {
    let mut room = Room::new();
    room.hunted = true;
    room.players[A1].alive = false;
    let send = radio::RadioSend {
        at: [0.0; 3],
        class: 0,
        entry: 0,
        r: 0,
        to_all: false,
        t: 0,
    };
    room.deliver_radio(A1, send, 0.0);
    assert_eq!(room.players[A2].lines(), 1, "a living teammate still reads the dead player's team radio");
    // The same call to everyone is the sender's own, so it is hidden.
    let all = radio::RadioSend { to_all: true, ..send };
    let before = room.players[B1].lines();
    room.deliver_radio(A1, all, 0.0);
    assert_eq!(room.players[B1].lines(), before);
}

/// The radio file reads as the spec says: defaults, keys, the guard on
/// missing classes.
#[test]
fn radio_file_defaults() {
    let d = data();
    assert_eq!(d.templates, ["Male", "Female"]);
    assert_eq!(d.classes.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Report", "Answer", "Request"]);
    let report = d.entry(0, 0).unwrap();
    assert!(report.sound_3d && !report.send_to_enemy);
    assert_eq!(report.radius, 1000.0);
    assert_eq!(report.sounds[0].len(), 2);
    assert!(d.entry(0, 1).is_none() && d.entry(2, 0).is_none());
    // The built-in menus read too.
    let b = RadioData::builtin();
    assert_eq!(b.classes.len(), 3);
}

/// The history keeps the last twenty lines for the user report,
/// concatenated.
#[test]
fn history_of_twenty() {
    let mut log = ChatLog::default();
    for i in 0..25 {
        log.push(PanelLine::new(format!("{i};"), colours::TEAM, 0.0, 1.0));
    }
    assert_eq!(log.history.len(), 20);
    assert!(log.report().starts_with("5;6;"));
    assert!(log.report().ends_with("24;"));
}

// --- A game of one and the wire

fn world() -> Sim {
    let mut sim = Sim::new(CaSim, GreyboxMapPlugin);
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    sim.ticks(3);
    sim
}

fn local(world: &mut World) -> Entity {
    world.query_filtered::<Entity, With<LocalPlayer>>().iter(world).next().expect("the player")
}

/// A game of one runs the same relay for its own player: its All line
/// comes back as `name : text` in the teammate colour; its radio call is
/// heard, with its sound.
#[test]
fn standalone() {
    let mut sim = world();
    let w = sim.app.world_mut();
    let me = local(w);
    w.entity_mut(me).insert((Name::new("A1"), CaVoice("Male".into()), Team(0)));
    w.resource_mut::<ChatUiState>().playing = true;
    assert!(chat::open_enter(w));
    w.resource_mut::<CaChat>().type_text("hi");
    chat::input_key(w, InputKey::Enter);
    sim.ticks(3);
    let w = sim.app.world_mut();
    let chat = w.resource::<CaChat>();
    let l = chat.log.lines.back().expect("the line came back");
    assert_eq!((l.text.as_str(), l.colour), ("A1 : hi", colours::ALL_TEAMMATE));
    // Radio: report entry 1 (builtin menus: silent), the line is heard.
    chat::radio_key(w, radio::REPORT);
    assert!(chat::radio_number_with(w, 1, Some(0)));
    sim.ticks(3);
    let w = sim.app.world_mut();
    let text = w.resource::<CaChat>().log.lines.back().unwrap().text.clone();
    assert!(text.starts_with("[Report] A1 : "), "{text}");
    let _ = (Health { current: 1.0, max: 1.0 }, CaSayRequest::default());
}
