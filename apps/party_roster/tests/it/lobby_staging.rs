//! The pre-match lobby as a staging state between matches
//! (docs/plans/active/ugg-ui.md M9): a party server with the lobby on
//! (`chimera_net::prematch`) and clients with the party's front end (the
//! main menu, the Escape menu, the lobby's screen) whose maps load
//! through the shell's own load (`client::console`: on a task, spawned a
//! step a frame, settled). A two-rung ladder on two maps is won, the
//! match ends, the server rotates to the next map and holds the next
//! match in the lobby. `trace` records, frame by frame, what changes on
//! the server and on the first client (printed with `--nocapture`; the
//! M9 notes' timeline).
//!
//! The staging: during the lobby nobody is in play (every character
//! staged: no weapon, dead, out of the game) and each client shows the
//! lobby in place of the match (no HUD, no world drawn, the input the
//! lobby's) while the map loads; the themed lobby (the Ultimate Gun
//! Game's theme on the clients: `staged_signal_party`) has no Escape
//! menu, Esc only closing Settings and Leave's confirm over it, Settings
//! keeping the lobby live under it and staying open across the match's
//! start; the plain lobby (Lucker Party's) keeps the match's Escape
//! menu; the header's buttons and Ready work while a client still
//! loads; everyone spawns on the first rung at the countdown's end; the
//! map a match left is unloaded before the next lobby shows. Ticks only.

use std::time::Duration;

use bevy::prelude::*;
use chimera_client::client::{
    AppHooks,
    front_end::AppMenu,
    hud::HudCover,
    prematch_lobby::{LobbyGate, LobbyOverlay, LobbyView, PreMatchLobbyPlugin},
};
use chimera_core::{
    console::{Console, ConsoleAppExt},
    core::{Damage, DamageKind, Dead, Hitgroup, SimClock},
};
use chimera_net::{
    harness::NetSim,
    net::{
        NetCharacter,
        client::{JoinProgress, Joined},
        memory::LinkConditions,
    },
    prematch::{LobbySettings, NetLobby},
};
use chimera_sim::{
    minigame::{self, InMinigame, MatchHold, PreMatch, Staged, ladder::Rung},
    mode::{Match, Phase},
    weapon::Inventory,
};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    map::{LoadedMapName, MapData, MapFile},
};
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use chimera_client::client::{prematch_lobby::LobbyAction, staging::{MapLeft, WorldCover}};
use chimera_net::{net::client::Handshake, prematch::LobbyReady};
use party_roster::{
    PartyPartsPlugin,
    esc_menu::{self, Action, EscMenu, Page},
    main_menu::{MainMenuSystems, PartyMenu, ShownScreen},
};

use crate::{
    main_menu::menu_bits,
    rotation::{FakeLoad, Levels, ROTATING, SERVER_LOAD, server_loads},
};

/// A small map named `id` (a floor and a spawn point), as the shell's
/// load brings it in.
fn quick_map(id: &str) -> MapData {
    use chimera_world::map::MapBrush;
    let mut data = MapData {
        name: id.rsplit(':').next().unwrap_or(id).to_string(),
        entity_scale: 0.0254,
        ..default()
    };
    data.collision_brushes
        .push(MapBrush::from_box(Vec3::new(-20.0, -1.0, -20.0), Vec3::new(20.0, 0.0, 20.0)));
    data.spawns.push((Vec3::new(0.0, 0.1, 0.0), None));
    data.spawns.push((Vec3::new(4.0, 0.1, 0.0), None));
    data
}

/// Every app: the party, its front end, the lobby on (a server's), maps
/// through the shell's load.
fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin, PreMatchLobbyPlugin));
    menu_bits(app, false);
    app.insert_resource(AppHooks {
        build: "test",
        load_map: |id, _| Ok(quick_map(id)),
        load_map_file: |id, _, _| Ok(quick_map(id)),
        map_file_bytes: |_| None,
        unload_greybox: |_| {},
        respawn_greybox: |_| {},
    })
    .add_systems(
        Update,
        (chimera_client::client::console::finish_map_load, file_of_loaded_map).chain().before(MainMenuSystems),
    );
    chimera_net::prematch::enable(app);
    // Started by everyone being ready (these trace the start, not the
    // host's Start, which `host_lobby` covers).
    app.insert_resource(LobbySettings {
        enabled: 1,
        countdown: 1.0,
        host_start: 0,
        ..default()
    });
}

/// The loaded map's file (`MapFile`, which the join's check compares) as
/// a game's loader leaves it on a client: the test maps have none of
/// their own (the server's load, `server_loads`, sets its own).
fn file_of_loaded_map(world: &mut World) {
    if world.get_resource::<chimera_core::core::NetRole>() != Some(&chimera_core::core::NetRole::Client) {
        return;
    }
    let Some(map) = world.get_resource::<LoadedMapName>().map(|m| m.0.clone()) else { return };
    let name = chimera_net::net::maps::map_name(&map).to_string();
    if !map.is_empty() && world.get_resource::<MapFile>().is_none_or(|f| f.name != name) {
        world.insert_resource(MapFile { name, hash: None });
    }
}

/// `setup` with Settings and the report dialog (the Ultimate Gun Game's
/// front end).
fn setup_signal(app: &mut App) {
    setup(app);
    app.add_plugins((party_roster::settings::SettingsPlugin, party_roster::feedback_signal::FeedbackSignalPlugin));
}

/// `staged_party` whose clients draw in the Ultimate Gun Game's theme
/// (the themed lobby, the Signal Escape menu and Settings).
pub(crate) fn staged_signal_party() -> NetSim {
    let mut sim = staged_party_with(setup_signal);
    for c in &mut sim.clients {
        c.app
            .insert_resource(chimera_ui::Theme::from_toml(include_str!("../../../ultimate_gun_game/data/theme.toml")).unwrap())
            .insert_resource(chimera_ui::UiUnits::new(1080.0, 1.0));
    }
    sim
}

/// A party server with the lobby on and two clients, the server's
/// `changelevel` a test map coming in `SERVER_LOAD` frames later.
pub(crate) fn staged_party() -> NetSim {
    staged_party_with(setup)
}

fn staged_party_with(setup: fn(&mut App)) -> NetSim {
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        11,
        2,
        setup,
    );
    sim.set_frame(TICK_INTERVAL);
    let id = minigame::add(sim.server.app.world_mut(), "test", ROTATING).expect("the test minigame reads").id.clone();
    minigame::start(sim.server.app.world_mut(), &id).expect("the minigame starts");
    sim.server.app.init_resource::<Levels>().add_systems(Update, server_loads);
    sim.server.app.console_command("changelevel", "changelevel <map>: a test's.", |w, a| {
        let map = a.first().cloned().unwrap_or_default();
        w.resource_mut::<Levels>().0.push(map.clone());
        w.insert_resource(LoadedMapName(map.clone()));
        w.insert_resource(FakeLoad(map, SERVER_LOAD));
        Ok(None)
    });
    sim
}

/// What the timeline follows, one frame.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Frame {
    pub server: Vec<(&'static str, String)>,
    pub client: Vec<(&'static str, String)>,
}

/// The server's and client `i`'s state now.
pub(crate) fn frame(sim: &mut NetSim, i: usize) -> Frame {
    let me = sim.character_of(i);
    let s = sim.server.app.world();
    let char_state = |w: &World, e: Option<Entity>| match e {
        None => "none".to_string(),
        Some(e) => format!(
            "{}{} guns {} rung {:?}",
            if w.get::<Dead>(e).is_some() { "dead" } else { "alive" },
            if w.get::<minigame::runtime::Staged>(e).is_some() { " staged" } else { "" },
            w.get::<Inventory>(e).map_or(0, |inv| inv.weapons.len()),
            w.get::<Rung>(e).map(|r| r.0)
        ),
    };
    let server = vec![
        ("map", s.get_resource::<LoadedMapName>().map(|m| m.0.clone()).unwrap_or_default()),
        ("map file", s.get_resource::<MapFile>().map(|m| m.name.clone()).unwrap_or("-".into())),
        ("phase", format!("{:?}", s.resource::<Match>().phase)),
        ("hold", s.contains_resource::<MatchHold>().to_string()),
        ("lobby", s.contains_resource::<PreMatch>().to_string()),
        ("player", char_state(s, me)),
    ];
    let lp = sim.local_player(i);
    let c = sim.clients[i].app.world_mut();
    let lobby = c.query::<&NetLobby>().iter(c).next().map(|l| format!("countdown {:?} waiting {}", l.countdown, l.waiting));
    let c = sim.clients[i].app.world();
    let world_off = c
        .get_resource::<chimera_client::client::staging::WorldCover>()
        .map_or("-".into(), |w| w.covered().to_string());
    let client = vec![
        ("join", format!("{:?}", c.resource::<JoinProgress>().stage)),
        ("joined map", c.get_resource::<Joined>().map(|j| j.map.clone()).unwrap_or("-".into())),
        ("load", format!("{:?}", chimera_client::client::console::map_load_stage(c))),
        ("map file", c.get_resource::<MapFile>().map(|m| m.name.clone()).unwrap_or("-".into())),
        ("screen", format!("{:?}", c.resource::<PartyMenu>().screen)),
        ("shown", format!("{:?}", c.resource::<ShownScreen>().screen)),
        ("NetLobby", lobby.unwrap_or("-".into())),
        ("lobby view", format!("{} {:?}", c.resource::<LobbyView>().shown, c.resource::<LobbyGate>())),
        ("player", char_state(c, lp)),
        ("hud covered", c.resource::<HudCover>().covered().to_string()),
        ("world off", world_off),
        ("input to menus", c.resource::<AppMenu>().open.to_string()),
        ("esc menu", c.resource::<EscMenu>().open.to_string()),
    ];
    Frame { server, client }
}

/// A timeline: each frame's changes, with the server's clock.
#[derive(Default)]
pub(crate) struct Trace {
    last: Option<Frame>,
    pub lines: Vec<String>,
    frames: u64,
}

impl Trace {
    pub fn note(&mut self, sim: &mut NetSim, i: usize, what: &str) {
        let t = sim.server.app.world().resource::<SimClock>().now;
        self.lines.push(format!("{t:8.2}s  ** {what}"));
    }

    pub fn step(&mut self, sim: &mut NetSim, i: usize) {
        self.frames += 1;
        let now = frame(sim, i);
        let t = sim.server.app.world().resource::<SimClock>().now;
        let changes = |side: &str, a: &[(&str, String)], b: Option<&[(&'static str, String)]>| -> Vec<String> {
            a.iter()
                .enumerate()
                .filter(|(k, (_, v))| b.is_none_or(|b| b[*k].1 != *v))
                .map(|(_, (n, v))| format!("{side}.{n}={v}"))
                .collect()
        };
        let mut all = changes("server", &now.server, self.last.as_ref().map(|l| l.server.as_slice()));
        all.extend(changes("client", &now.client, self.last.as_ref().map(|l| l.client.as_slice())));
        if !all.is_empty() {
            self.lines.push(format!("{t:8.2}s  f{:<5} {}", self.frames, all.join("  ")));
        }
        self.last = Some(now);
    }

    pub fn print(&self) {
        for l in &self.lines {
            println!("{l}");
        }
    }
}

/// Steps until `done` (at most `max` frames), tracing every frame.
pub(crate) fn traced(sim: &mut NetSim, trace: &mut Trace, max: u64, mut done: impl FnMut(&mut NetSim) -> bool) -> bool {
    for _ in 0..max {
        sim.step();
        trace.step(sim, 0);
        if done(sim) {
            return true;
        }
    }
    false
}

/// `by` kills `victim` on the server with one hit.
fn hit(sim: &mut NetSim, victim: Entity, by: Entity) {
    let at = sim.server.position(victim);
    sim.server.app.world_mut().write_message(Damage {
        target: victim,
        attacker: Some(by),
        amount: 5.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
}

/// `ok`, else the timeline so far and a panic saying `what`.
pub(crate) fn check(ok: bool, trace: &Trace, what: &str) {
    if !ok {
        trace.print();
        panic!("{what}");
    }
}

pub(crate) fn live(sim: &NetSim) -> bool {
    let w = sim.server.app.world();
    w.resource::<Match>().phase == chimera_sim::mode::Phase::Live && !w.contains_resource::<PreMatch>()
}

/// The whole run: the first join into a lobby, the first match won, the
/// match end, the rotation's map change, the next lobby and its match.
/// `--nocapture` prints the timeline (the M9 notes' "before" and "after").
#[test]
fn trace_match_end_to_next_match() {
    let mut sim = staged_party();
    let mut trace = Trace::default();
    trace.note(&mut sim, 0, "two clients join a server whose first match waits in the lobby");
    check(traced(&mut sim, &mut trace, 3000, |s| live(s)), &trace, "the first match starts");
    trace.note(&mut sim, 0, "the first match is live; Ann (client 0) kills Ben twice");
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    for (e, name) in [(a, "Ann"), (b, "Ben")] {
        let w = sim.server.app.world_mut();
        w.entity_mut(e).insert(Name::new(name));
        w.get_mut::<NetCharacter>(e).unwrap().name = name.into();
    }
    for _ in 0..2 {
        let back = traced(&mut sim, &mut trace, 600, |s| s.server.app.world().get::<Dead>(b).is_none());
        check(back, &trace, "the victim is back");
        traced(&mut sim, &mut trace, 30, |_| false);
        hit(&mut sim, b, a);
    }
    let moved = traced(&mut sim, &mut trace, 4000, |s| !s.server.app.world().resource::<Levels>().0.is_empty());
    check(moved, &trace, "the match ends and the server moves on");
    trace.note(&mut sim, 0, "the server changes level");
    let next = traced(&mut sim, &mut trace, 6000, |s| live(s) && s.server.app.world().resource::<LoadedMapName>().0 == "test:next");
    check(next, &trace, "the next match starts on the next map");
    trace.note(&mut sim, 0, "the next match is live");
    traced(&mut sim, &mut trace, 60, |_| false);
    trace.print();
    let _ = sim.clients[0].app.world().resource::<Console>();
}

/// Every character on the server held out of play by the lobby: staged,
/// dead, no weapon, not in the minigame.
fn all_staged(sim: &mut NetSim) -> Result<(), String> {
    let w = sim.server.app.world_mut();
    let mut q = w.query::<(&NetCharacter, Has<Staged>, Has<Dead>, Option<&Inventory>, Has<InMinigame>)>();
    let mut n = 0;
    for (c, staged, dead, inv, playing) in q.iter(w) {
        n += 1;
        let guns = inv.map_or(0, |i| i.weapons.len());
        if !staged || !dead || guns > 0 || playing {
            return Err(format!("{}: staged {staged}, dead {dead}, {guns} guns, in the minigame {playing}", c.name));
        }
    }
    if n == 0 { Err("no characters".into()) } else { Ok(()) }
}

/// Client `i` shows the lobby in place of the match: its screen up, the
/// HUDs covered, the world's view off, the input the lobby's (the mouse
/// free), its own player (if it has one yet) dead without a weapon.
pub(crate) fn lobby_owns(sim: &mut NetSim, i: usize) -> Result<(), String> {
    let me = sim.local_player(i);
    let w = sim.clients[i].app.world();
    let shown = w.resource::<LobbyView>().shown;
    let huds = w.resource::<HudCover>().covered();
    let world = w.resource::<WorldCover>().covered();
    let input = w.resource::<AppMenu>().open;
    let player = me.map(|e| (w.get::<Dead>(e).is_some(), w.get::<Inventory>(e).map_or(0, |i| i.weapons.len())));
    if shown && huds && world && input && player.is_none_or(|(dead, guns)| dead && guns == 0) {
        Ok(())
    } else {
        Err(format!("client {i}: lobby {shown}, HUDs covered {huds}, world off {world}, input to menus {input}, player (dead, guns) {player:?}"))
    }
}

/// A held client's handshake, taken as it comes (its join stops after the
/// welcome, as while a download or a load runs).
#[derive(Resource, Default)]
struct HeldJoin(Option<Handshake>);

fn take_handshake(world: &mut World) {
    if world.contains_resource::<HeldJoin>()
        && let Some(h) = world.remove_resource::<Handshake>()
    {
        world.resource_mut::<HeldJoin>().0 = Some(h);
    }
}

pub(crate) fn hold_join(sim: &mut NetSim, i: usize) {
    sim.clients[i].app.init_resource::<HeldJoin>().add_systems(RunFixedMainLoop, take_handshake);
}

pub(crate) fn release_join(sim: &mut NetSim, i: usize) {
    let w = sim.clients[i].app.world_mut();
    let h = w.remove_resource::<HeldJoin>().and_then(|h| h.0).expect("the held handshake");
    w.insert_resource(h);
}

fn tap(sim: &mut NetSim, i: usize, key: KeyCode) {
    sim.clients[i].app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
    sim.step();
    let mut keys = sim.clients[i].app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
    sim.step();
}

pub(crate) fn lobby_view(sim: &NetSim, i: usize) -> bool {
    sim.clients[i].app.world().resource::<LobbyView>().shown
}

#[test]
fn nobody_plays_in_the_lobby_and_everyone_spawns_at_its_end() {
    let mut sim = staged_party();
    hold_join(&mut sim, 1);
    assert!(sim.until(900, |s| s.clients[0].app.world().contains_resource::<Joined>() && lobby_view(s, 0) && lobby_view(s, 1)));
    // Client 1 still loads (its join held); client 0 is on the map. For
    // two seconds nothing is in play and both show only the lobby.
    for _ in 0..120 {
        sim.step();
        all_staged(&mut sim).unwrap();
        lobby_owns(&mut sim, 0).unwrap();
        lobby_owns(&mut sim, 1).unwrap();
        assert!(sim.server.app.world().contains_resource::<PreMatch>());
    }
    assert_eq!(sim.server.app.world().resource::<Match>().phase, Phase::Waiting, "the clock hasn't started");
    // Client 1 has the map: the countdown, then everyone on the first rung.
    release_join(&mut sim, 1);
    assert!(sim.until(900, |s| !s.server.app.world().contains_resource::<PreMatch>()), "the lobby ends");
    assert!(sim.until(30, |s| s.server.app.world().resource::<Match>().phase == Phase::Live), "the clock starts");
    for i in 0..2 {
        let e = sim.character_of(i).unwrap();
        let w = sim.server.app.world();
        assert!(w.get::<Staged>(e).is_none() && w.get::<Dead>(e).is_none() && w.get::<InMinigame>(e).is_some(), "player {i} spawned");
        assert_eq!(w.get::<Rung>(e).map(|r| r.0), Some(0), "player {i} on the first rung");
        assert!(w.get::<Inventory>(e).is_some_and(|inv| !inv.weapons.is_empty()), "player {i} armed");
    }
    // The clients: the lobby gone, the match theirs again.
    assert!(sim.until(60, |s| (0..2).all(|i| !lobby_view(s, i))));
    sim.ticks(5);
    for i in 0..2 {
        let w = sim.clients[i].app.world();
        assert!(!w.resource::<WorldCover>().covered() && !w.resource::<HudCover>().covered(), "client {i} sees the match");
        assert!(!w.resource::<AppMenu>().open, "client {i} plays");
    }
}

/// Client `i`'s Escape menu.
fn esc(sim: &NetSim, i: usize) -> &EscMenu {
    sim.clients[i].app.world().resource::<EscMenu>()
}

/// Where client `i`'s Settings shows (None: shut).
fn settings_ctx(sim: &NetSim, i: usize) -> Option<party_roster::settings::Context> {
    sim.clients[i].app.world().resource::<party_roster::settings::Settings>().ctx
}

fn overlay(sim: &NetSim, i: usize) -> bool {
    sim.clients[i].app.world().resource::<LobbyOverlay>().0
}

/// Client 0 joined and in the lobby, no countdown for a minute.
fn in_lobby(sim: &mut NetSim) {
    sim.server.app.world_mut().resource_mut::<LobbySettings>().countdown = 60.0;
    assert!(sim.until(900, |s| s.clients[0].app.world().contains_resource::<Joined>() && lobby_view(s, 0)));
}

#[test]
fn escape_in_the_themed_lobby_only_closes_what_is_open() {
    let mut sim = staged_signal_party();
    in_lobby(&mut sim);
    // Nothing open: Esc is the breadcrumb's Back, Leave, which asks first
    // (no Escape menu); Esc again stays.
    for _ in 0..2 {
        tap(&mut sim, 0, KeyCode::Escape);
        sim.ticks(1);
        let m = esc(&sim, 0);
        assert!(m.open && m.lobby && m.page == Page::ConfirmLeave, "{m:?}");
        tap(&mut sim, 0, KeyCode::Escape);
        assert!(!esc(&sim, 0).open, "{:?}", esc(&sim, 0));
        lobby_owns(&mut sim, 0).unwrap();
    }
    // Settings over the lobby; Esc closes it.
    sim.clients[0].app.world_mut().write_message(LobbyAction::Settings);
    sim.ticks(2);
    {
        let m = esc(&sim, 0);
        assert!(m.open && m.lobby && m.page == Page::Options(0), "{m:?}");
    }
    assert_eq!(settings_ctx(&sim, 0), Some(party_roster::settings::Context::Game));
    assert!(overlay(&sim, 0), "the lobby's key hints hide under it");
    tap(&mut sim, 0, KeyCode::Escape);
    assert!(!esc(&sim, 0).open && settings_ctx(&sim, 0).is_none() && !overlay(&sim, 0), "Esc closed Settings: {:?}", esc(&sim, 0));
    // Leave asks first, Stay first; Esc closes it.
    sim.clients[0].app.world_mut().write_message(LobbyAction::Leave);
    sim.ticks(2);
    {
        let m = esc(&sim, 0);
        assert!(m.open && m.lobby && m.page == Page::ConfirmLeave && m.focus == 0, "{m:?}");
    }
    tap(&mut sim, 0, KeyCode::Escape);
    assert!(!esc(&sim, 0).open, "Esc closed the confirm");
    // Settings' Back closes it too (no first page behind it).
    sim.clients[0].app.world_mut().write_message(LobbyAction::Settings);
    sim.ticks(2);
    sim.clients[0].app.world_mut().write_message(Action::Back);
    sim.ticks(2);
    assert!(!esc(&sim, 0).open);
    lobby_owns(&mut sim, 0).unwrap();
    assert!(sim.clients[0].app.world().resource::<JoinProgress>().failure.is_none(), "still connected");
    // No lobby entries in the menu's data.
    assert!(!esc_menu::shown_entries(true, true, false).iter().any(|e| e.action == Some(Action::Leave)), "the match's menu has no Leave");
}

#[test]
fn escape_in_the_plain_lobby_opens_the_match_menu() {
    // Lucker Party's: no themed lobby, no header buttons; Esc is the
    // match's Escape menu as before.
    let mut sim = staged_party();
    in_lobby(&mut sim);
    tap(&mut sim, 0, KeyCode::Escape);
    {
        let m = esc(&sim, 0);
        assert!(m.open && !m.lobby && m.page == Page::Main, "{m:?}");
    }
    tap(&mut sim, 0, KeyCode::Escape);
    assert!(!esc(&sim, 0).open);
    lobby_owns(&mut sim, 0).unwrap();
}

/// The server's players ready (`LobbyReady`).
fn ready_on_server(sim: &mut NetSim) -> usize {
    let w = sim.server.app.world_mut();
    w.query_filtered::<(), With<LobbyReady>>().iter(w).count()
}

/// Client `i`'s own row in its lobby reads ready.
fn ready_here(sim: &NetSim, i: usize) -> bool {
    lobby_view(sim, i)
        && sim.clients[i].app.world().resource::<LobbyView>().rows.iter().any(|r| r.local && r.status == chimera_net::prematch::status::READY)
}

#[test]
fn settings_over_the_lobby_keeps_it_live_and_ready_kept() {
    let mut sim = staged_signal_party();
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_lobby_ready_up 1");
    assert!(sim.until(900, |s| (0..2).all(|i| s.clients[i].app.world().contains_resource::<Joined>() && lobby_view(s, i))));
    sim.server.app.world_mut().resource_mut::<LobbySettings>().countdown = 60.0;
    // Client 0 readies up, then opens Settings over the lobby.
    sim.clients[0].app.world_mut().write_message(LobbyAction::Ready);
    assert!(sim.until(120, |s| ready_here(s, 0)), "client 0 ready");
    sim.clients[0].app.world_mut().write_message(LobbyAction::Settings);
    sim.ticks(2);
    assert_eq!(settings_ctx(&sim, 0), Some(party_roster::settings::Context::Game));
    // Under it the lobby goes on: client 1 readies and the countdown
    // starts, both seen on client 0; its own Ready holds.
    let others_ready = |s: &NetSim| {
        s.clients[0].app.world().resource::<LobbyView>().rows.iter().any(|r| !r.local && r.status == chimera_net::prematch::status::READY)
    };
    assert!(!others_ready(&sim));
    sim.clients[1].app.world_mut().write_message(LobbyAction::Ready);
    assert!(sim.until(120, |s| others_ready(s)), "client 1's Ready seen under Settings");
    assert!(sim.until(120, |s| s.clients[0].app.world().resource::<LobbyView>().countdown.is_some()), "the countdown seen under Settings");
    let before = sim.clients[0].app.world().resource::<LobbyView>().countdown;
    assert!(
        sim.until(200, |s| s.clients[0].app.world().resource::<LobbyView>().countdown != before),
        "the countdown runs under Settings"
    );
    assert!(ready_here(&sim, 0) && ready_on_server(&mut sim) == 2, "client 0's Ready kept");
    assert!(esc(&sim, 0).open && settings_ctx(&sim, 0).is_some() && overlay(&sim, 0), "Settings still open");
    // Closed: the lobby as it was, still ready.
    tap(&mut sim, 0, KeyCode::Escape);
    assert!(settings_ctx(&sim, 0).is_none() && !overlay(&sim, 0));
    assert!(ready_here(&sim, 0));
    lobby_owns(&mut sim, 0).unwrap();
}

#[test]
fn settings_stay_open_across_the_match_start() {
    let mut sim = staged_signal_party();
    in_lobby(&mut sim);
    sim.clients[0].app.world_mut().write_message(LobbyAction::Settings);
    sim.ticks(2);
    sim.clients[0].app.world_mut().resource_mut::<Console>().submit("settings_set chimera_fullscreen 2");
    sim.ticks(2);
    let pending = |s: &NetSim| s.clients[0].app.world().resource::<party_roster::settings::Settings>().pending.clone();
    assert_eq!(pending(&sim).get("chimera_fullscreen").map(String::as_str), Some("2"), "a display change waiting");
    // The match starts under it (the host's start: a short countdown).
    sim.server.app.world_mut().resource_mut::<LobbySettings>().countdown = 1.0;
    sim.server.app.world_mut().resource_mut::<Console>().submit("lobby_start 1");
    assert!(sim.until(900, |s| !s.server.app.world().contains_resource::<PreMatch>()), "the lobby ends");
    assert!(sim.until(120, |s| !lobby_view(s, 0)), "the lobby gone on client 0");
    sim.ticks(10);
    {
        let m = esc(&sim, 0);
        assert!(m.open && !m.lobby && m.page == Page::Options(0), "Settings stays, now the match's: {m:?}");
    }
    assert_eq!(settings_ctx(&sim, 0), Some(party_roster::settings::Context::Game));
    assert_eq!(pending(&sim).get("chimera_fullscreen").map(String::as_str), Some("2"), "nothing lost");
    assert!(sim.clients[0].app.world().resource::<AppMenu>().open, "the input still Settings'");
    // Back goes to the match's menu, as for Settings opened in play.
    sim.clients[0].app.world_mut().resource_mut::<Console>().submit("settings_press leave_discard");
    sim.ticks(2);
    sim.clients[0].app.world_mut().write_message(Action::Back);
    sim.ticks(2);
    let m = esc(&sim, 0);
    assert!(m.open && m.page == Page::Main, "{m:?}");
}

#[test]
fn the_lobby_buttons_work_while_a_client_still_loads() {
    let mut sim = staged_signal_party();
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_lobby_ready_up 1");
    hold_join(&mut sim, 1);
    assert!(sim.until(900, |s| lobby_view(s, 1) && s.character_of(1).is_some()));
    assert!(!sim.clients[1].app.world().contains_resource::<Joined>(), "client 1 still loads");
    lobby_owns(&mut sim, 1).unwrap();
    // Ready while loading: kept on the server for when it has the map.
    sim.clients[1].app.world_mut().write_message(LobbyAction::Ready);
    assert!(sim.until(120, |s| ready_on_server(s) == 1), "client 1 readied up while loading");
    // Settings over the lobby and the loading panel; Esc closes it.
    sim.clients[1].app.world_mut().write_message(LobbyAction::Settings);
    sim.ticks(2);
    {
        let m = esc(&sim, 1);
        assert!(m.open && m.lobby && m.page == Page::Options(0), "{m:?}");
    }
    assert!(settings_ctx(&sim, 1).is_some() && overlay(&sim, 1));
    tap(&mut sim, 1, KeyCode::Escape);
    assert!(!esc(&sim, 1).open && settings_ctx(&sim, 1).is_none());
    // Report a bug: the report's capture (a made-up frame), then its
    // dialog; Esc closes it and nothing else.
    use chimera_client::client::feedback::{Feedback, FeedbackCapture};
    sim.clients[1].app.insert_resource(FeedbackCapture::fake());
    sim.clients[1].app.world_mut().write_message(LobbyAction::Report);
    sim.ticks(2);
    assert!(sim.clients[1].app.world().resource::<Feedback>().active(), "the report asked for");
    assert!(sim.until(60, |s| s.clients[1].app.world().resource::<Feedback>().shown()), "the report's dialog");
    // Once the dialog shows, not while its frame is taken (the lobby
    // stays in the report's screenshot).
    sim.ticks(1);
    assert!(overlay(&sim, 1), "the key hints hide under the report");
    // The dialog reads typed keys (`KeyboardInput`), the menus held ones.
    sim.clients[1].app.world_mut().write_message(bevy::input::keyboard::KeyboardInput {
        key_code: KeyCode::Escape,
        logical_key: bevy::input::keyboard::Key::Escape,
        state: bevy::input::ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    tap(&mut sim, 1, KeyCode::Escape);
    assert!(!sim.clients[1].app.world().resource::<Feedback>().active(), "Esc closed the report");
    assert!(!esc(&sim, 1).open && !overlay(&sim, 1), "and opened nothing");
    // Leave: asked first (Stay first), then gone.
    sim.clients[1].app.world_mut().write_message(LobbyAction::Leave);
    sim.ticks(2);
    {
        let m = esc(&sim, 1);
        assert!(m.open && m.lobby && m.page == Page::ConfirmLeave && m.focus == 0, "{m:?}");
    }
    assert!(!sim.clients[1].app.world().contains_resource::<Joined>(), "client 1 still loads");
    sim.clients[1].app.world_mut().write_message(Action::DisconnectYes);
    let players = |s: &mut NetSim| {
        let w = s.server.app.world_mut();
        w.query::<&chimera_net::net::server::Player>().iter(w).count()
    };
    assert!(sim.until(300, |s| players(s) == 1), "client 1 left");
    assert!(sim.server.app.world().contains_resource::<PreMatch>(), "the lobby goes on for the others");
}

#[test]
fn the_map_left_is_unloaded_under_the_next_lobby() {
    let mut sim = staged_party();
    let mut trace = Trace::default();
    check(traced(&mut sim, &mut trace, 3000, |s| live(s)), &trace, "the first match starts");
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    let parts = |s: &mut NetSim| {
        let w = s.clients[0].app.world_mut();
        w.query_filtered::<(), With<chimera_world::map::MapPart>>().iter(w).count()
    };
    assert!(parts(&mut sim) > 0, "the first map is in on the client");
    for _ in 0..2 {
        let back = traced(&mut sim, &mut trace, 600, |s| s.server.app.world().get::<Dead>(b).is_none());
        check(back, &trace, "the victim is back");
        traced(&mut sim, &mut trace, 30, |_| false);
        hit(&mut sim, b, a);
    }
    let over = traced(&mut sim, &mut trace, 4000, |s| s.server.app.world().resource::<Match>().phase == Phase::MatchOver);
    check(over, &trace, "the match is over");
    // From the match's end to the next lobby on the client: the old map
    // goes first, under the loading panel's cover.
    let mut left_at = None;
    let mut lobby_at = None;
    for f in 0..6000u32 {
        sim.step();
        trace.step(&mut sim, 0);
        let w = sim.clients[0].app.world();
        if left_at.is_none() && w.contains_resource::<MapLeft>() {
            assert!(w.resource::<WorldCover>().covered(), "unloaded under a cover");
            left_at = Some(f);
            assert_eq!(parts(&mut sim), 0, "nothing of the old map stays");
        }
        if lobby_at.is_none() && lobby_view(&sim, 0) && sim.clients[0].app.world().resource::<LobbyView>().map == "next" {
            lobby_at = Some(f);
            assert!(sim.clients[0].app.world().resource::<WorldCover>().covered(), "the lobby covers the world");
        }
        if left_at.is_some() && lobby_at.is_some() {
            break;
        }
    }
    let (Some(left), Some(lobby)) = (left_at, lobby_at) else {
        trace.print();
        panic!("unloaded at {left_at:?}, the next lobby at {lobby_at:?}");
    };
    // The lobby between maps comes up at the change (docs/plans/active/
    // lobby-between-maps.md), the map left goes under its cover.
    assert!(left.abs_diff(lobby) < 120, "unloaded (frame {left}) around the lobby's showing (frame {lobby})");
    assert_eq!(sim.clients[0].app.world().resource::<MapLeft>().0, "greybox");
    // The staging's replication follows the lobby's by a few frames.
    assert!(sim.until(10, |s| lobby_owns(s, 0).is_ok()), "{:?}", lobby_owns(&mut sim, 0));
}
