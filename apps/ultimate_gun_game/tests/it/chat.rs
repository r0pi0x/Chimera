//! The one chat (`chimera_client::client::signal_chat`, plan ugg-ui M10)
//! between remote clients of a dedicated server (`NetSim`: two or three
//! clients, the party's simulation on the greybox, the Ultimate Gun Game's
//! Signal theme and sound table): a client's line reaches the others'
//! feed and its own, the cue `chat_in` on the others' only; team lines
//! only within the team when there are teams; the server's flood limit
//! (four lines in a burst, then one a second) drops the rest and tells
//! the sender in its feed; a client that ignores a player sees none of
//! their lines and hears no cue; the lobby's chat works while it waits;
//! joins show as notices, not as player lines.

use std::time::Duration;

use bevy::prelude::*;
use chimera_client::client::{
    audio::SoundOptionsPlugin,
    chat::{ChatFilter, ChatMutes, LineKind},
    logo_pass::LogoPassPlugin,
    prematch_lobby::{LobbyView, PreMatchLobbyPlugin},
    signal_chat::{FeedEntry, SignalChatLog, SignalChatStatePlugin},
    ui_sound::{UiSoundPlugin, UiSounds, UiVoices},
};
use chimera_core::{console::Console, core::Team};
use chimera_net::{
    harness::NetSim,
    net::{NetCharacter, chat::Flood, memory::LinkConditions},
};
use chimera_ui::{ChimeraUiPlugin, UiClock};
use chimera_world::greybox::GreyboxMapPlugin;
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin, signal};

const LINK: LinkConditions = LinkConditions {
    latency: Duration::from_millis(20),
    jitter: Duration::ZERO,
    loss: 0.0,
};

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin, UltimateGunGamePlugin::default()));
    chimera_net::prematch::enable(app);
    app.insert_resource(chimera_ui::Theme::from_toml(signal::THEME).unwrap());
    app.add_plugins((ChimeraUiPlugin, LogoPassPlugin, PreMatchLobbyPlugin, SignalChatStatePlugin));
    app.add_plugins((SoundOptionsPlugin, UiSoundPlugin));
    if !app.world().contains_resource::<Assets<Image>>() {
        app.init_asset::<Image>();
    }
    app.insert_resource(UiClock::manual());
    app.insert_resource(UiSounds::silent(signal::UI_SOUND_TABLE).unwrap());
}

fn sim(clients: usize, lobby: bool) -> NetSim {
    let mut sim = NetSim::new(CaSim, LINK, 11, clients, setup);
    sim.set_frame(TICK_INTERVAL);
    let lines: &[&str] =
        if lobby { &["mp_lobby 1", "mp_lobby_ready_up 1", "mp_lobby_min_players 3", "mp_lobby_countdown 5"] } else { &["mp_lobby 0"] };
    for l in lines {
        sim.server.app.world_mut().resource_mut::<Console>().submit(*l);
    }
    chimera_net::minigame_net::change_match(sim.server.app.world_mut(), MINIGAME, Some("greybox")).unwrap();
    sim.until_joined(900);
    step(&mut sim, 120);
    sim
}

fn step(sim: &mut NetSim, n: u64) {
    for _ in 0..n {
        sim.step();
        for c in &mut sim.clients {
            c.app.world_mut().resource_mut::<UiClock>().step(TICK_INTERVAL as f32);
        }
    }
}

/// Client `i` types `line` in its console.
fn con(sim: &mut NetSim, i: usize, line: &str) {
    sim.clients[i].app.world_mut().resource_mut::<Console>().submit(line);
}

/// Client `i`'s name as everyone sees it.
fn name_of(sim: &mut NetSim, i: usize) -> String {
    let me = sim.local_player(i).unwrap();
    sim.clients[i].app.world().get::<NetCharacter>(me).unwrap().name.clone()
}

/// The lines client `i` shows (its `cl_chat` and mutes), oldest first.
fn shown(sim: &NetSim, i: usize) -> Vec<FeedEntry> {
    let w = sim.clients[i].app.world();
    let (log, mutes, filter) = (w.resource::<SignalChatLog>(), w.resource::<ChatMutes>(), *w.resource::<ChatFilter>());
    log.shown(mutes, filter).cloned().collect()
}

/// Client `i`'s players' lines with `text`.
fn said(sim: &NetSim, i: usize, text: &str) -> Vec<FeedEntry> {
    shown(sim, i).into_iter().filter(|l| l.kind.player() && l.text == text).collect()
}

fn cues(sim: &NetSim, i: usize) -> usize {
    sim.clients[i].app.world().resource::<UiVoices>().played.iter().filter(|p| p.event == "chat_in").count()
}

#[test]
fn a_line_reaches_the_other_client_and_its_own_with_a_cue_only_there() {
    let mut sim = sim(2, false);
    let me = name_of(&mut sim, 0);
    let before = (cues(&sim, 0), cues(&sim, 1));
    con(&mut sim, 0, "say gg, one more?");
    step(&mut sim, 30);
    let theirs = said(&sim, 1, "gg, one more?");
    assert_eq!(theirs.len(), 1, "client 1 shows it once: {:?}", shown(&sim, 1));
    assert_eq!(theirs[0].name.as_deref(), Some(me.as_str()));
    assert_eq!(theirs[0].kind, LineKind::All);
    assert!(!theirs[0].own);
    let mine = said(&sim, 0, "gg, one more?");
    assert_eq!(mine.len(), 1, "the sender sees its own line once");
    assert!(mine[0].own);
    assert_eq!(cues(&sim, 1) - before.1, 1, "the other client cues it");
    assert_eq!(cues(&sim, 0) - before.0, 0, "no cue for your own line");
    // Case kept, control characters out, at most 127 bytes.
    con(&mut sim, 1, &format!("say MiXeD\u{7} {}", "x".repeat(200)));
    step(&mut sim, 30);
    let long: Vec<FeedEntry> = shown(&sim, 0).into_iter().filter(|l| l.text.starts_with("MiXeD")).collect();
    assert_eq!(long.len(), 1);
    assert_eq!(long[0].text.len(), 127, "{}", long[0].text);
    assert!(!long[0].text.contains('\u{7}'));
}

#[test]
fn team_lines_stay_within_the_team() {
    let mut sim = sim(3, false);
    let (a, b, c) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap(), sim.character_of(2).unwrap());
    // Teams: clients 0 and 1 on team 2, client 2 on team 3.
    for (e, t) in [(a, 2), (b, 2), (c, 3)] {
        sim.server.app.world_mut().entity_mut(e).insert(Team(t));
    }
    step(&mut sim, 10);
    con(&mut sim, 0, "say_team rotate B");
    step(&mut sim, 30);
    assert_eq!(said(&sim, 1, "rotate B").len(), 1, "a teammate reads it");
    assert_eq!(said(&sim, 1, "rotate B")[0].kind, LineKind::Team);
    assert!(said(&sim, 2, "rotate B").is_empty(), "the other team doesn't");
    assert_eq!(said(&sim, 0, "rotate B").len(), 1, "the sender sees its own");
    // `cl_chat 2`: team lines only.
    con(&mut sim, 1, "cl_chat 2");
    con(&mut sim, 2, "say hi all");
    con(&mut sim, 0, "say_team hi team");
    step(&mut sim, 30);
    assert!(said(&sim, 1, "hi all").is_empty(), "cl_chat 2 hides everyone's lines");
    assert_eq!(said(&sim, 1, "hi team").len(), 1, "but shows the team's");
    assert_eq!(said(&sim, 0, "hi all").len(), 1, "cl_chat 1 shows everyone's");
}

#[test]
fn the_server_drops_a_flood_and_tells_the_sender() {
    let mut sim = sim(2, false);
    for i in 0..7 {
        con(&mut sim, 0, &format!("say spam {i}"));
    }
    step(&mut sim, 30);
    let got: Vec<String> = shown(&sim, 1).into_iter().filter(|l| l.text.starts_with("spam")).map(|l| l.text).collect();
    assert_eq!(got, ["spam 0", "spam 1", "spam 2", "spam 3"], "four in a burst");
    assert!(sim.server.app.world().resource::<Flood>().dropped >= 3);
    let told = shown(&sim, 0).into_iter().filter(|l| l.kind == LineKind::System && l.text.contains("flooding")).count();
    assert!(told >= 1, "the sender is told, in its feed: {:?}", shown(&sim, 0));
    // A second later, one more goes through.
    step(&mut sim, (1.2 / TICK_INTERVAL) as u64);
    con(&mut sim, 0, "say after");
    step(&mut sim, 30);
    assert_eq!(said(&sim, 1, "after").len(), 1);
}

#[test]
fn an_ignored_player_shows_nothing_and_cues_nothing() {
    let mut sim = sim(2, false);
    let them = name_of(&mut sim, 0);
    con(&mut sim, 0, "say before");
    step(&mut sim, 30);
    assert_eq!(said(&sim, 1, "before").len(), 1);
    con(&mut sim, 1, &format!("ignore {them}"));
    step(&mut sim, 2);
    let cued = cues(&sim, 1);
    con(&mut sim, 0, "say muted line");
    step(&mut sim, 30);
    assert!(said(&sim, 1, "muted line").is_empty(), "hidden");
    assert!(said(&sim, 1, "before").is_empty(), "past lines too");
    assert_eq!(cues(&sim, 1), cued, "no cue");
    assert_eq!(said(&sim, 0, "muted line").len(), 1, "the sender still sees it");
    con(&mut sim, 1, &format!("unignore {them}"));
    step(&mut sim, 2);
    assert_eq!(said(&sim, 1, "muted line").len(), 1, "shown again once unmuted");
}

#[test]
fn the_lobby_chat_works_while_it_waits_and_joins_are_notices() {
    // Three players needed: two clients wait in the lobby.
    let mut sim = sim(2, true);
    assert!(sim.clients[0].app.world().resource::<LobbyView>().shown, "the lobby shows");
    con(&mut sim, 1, "say waiting for one more");
    step(&mut sim, 30);
    assert_eq!(said(&sim, 0, "waiting for one more").len(), 1);
    assert!(sim.clients[0].app.world().resource::<LobbyView>().shown, "still in the lobby");
    // Each client saw the other join: a notice, no name of a player line.
    let joins: Vec<FeedEntry> = shown(&sim, 0).into_iter().filter(|l| l.text.contains("joined")).collect();
    assert!(!joins.is_empty(), "{:?}", shown(&sim, 0));
    assert!(joins.iter().all(|l| l.kind == LineKind::Notice && l.name.is_none()));
}
