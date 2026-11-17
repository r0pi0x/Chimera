//! The pre-match lobby's screen (`chimera_net::prematch`: a server holding
//! a new match until every player has its map), drawn by the shell in
//! place of the match: while it shows (`lobby_shown`) it owns the screen
//! and the input (docs/plans/active/ugg-ui.md M9, "Staging between
//! matches"): the world's view off (`staging::WorldCover`), the HUDs
//! covered (`hud::HudCover`'s `LOBBY_COVER`, our Signal HUD too), no
//! player in play (the server stages everyone: `minigame::Staged`), and
//! the app owns the mouse and Esc (the party apps under a themed lobby:
//! Esc only closes what is open over it; Lucker Party's plain lobby: its
//! Escape menu). It shows over the loading panel of a player still downloading
//! or loading (an app whose menu draws on its own camera moves
//! `LobbyRoot` there, as the party apps' main menu does with the
//! console). Its actions (`LobbyAction`, the themed panel's buttons):
//! Ready (the key's line; a player still downloading or loading readies
//! up for when it has the map, `LobbyAsked`), Settings and Leave (the
//! app's, `LobbyAppAsked`, where it answers them, `LobbyAnswered`; else
//! Leave is `disconnect` and Settings nothing) and Report a bug (the
//! report dialog, `feedback::request`). An app's overlay over the lobby
//! (its Settings, a confirm, the report dialog: `LobbyOverlay`) hides the
//! panel's key hints; the lobby stays live under it. `lobby_press
//! <button>` presses one (tests, screenshots).
//!
//! - **The head**: the mode (the minigame's name), the map big, and what
//!   it waits for: "Waiting for 2 players..." with the time left of the
//!   longest wait ("starts anyway in 1:12"), or once counting down who
//!   it starts without.
//! - **The players**, one row each in the server's order (humans, then
//!   bots): the name with its tags (HOST for a listen server's host, BOT,
//!   YOU, the session wins as "2 WON": `LobbyRow::wins`), the status in
//!   its colour (connecting, downloading, loading the map, not ready,
//!   ready), for a downloader a bar with "MB done / total"
//!   and the percentage, for a loader its stage and share ("Building
//!   models 62%": `LobbyRow::stage`, `chimera_net::net::load_stages`; this
//!   machine's own row from its own stages, live while a host's
//!   simulation waits on its spawn), and the ping (or "BOT"); the local player's row
//!   on the gold band the match-end screen gives it (found by its client
//!   id: `net::client::LocalClientId`, a listen host's `HOST_ID`).
//! - **The key** (`LOBBY_KEY`, F3, in place of its bind while the lobby
//!   shows: `console::run_binds`), told under the players: a listen
//!   server's host starts the match now (`lobby_start`, before a countdown
//!   runs), a player on the map readies up or takes it back
//!   (`lobby_ready`, which counts where the server has players ready up,
//!   `mp_lobby_ready_up`).
//! - **The countdown** under it, big: "Match starts in" and the seconds.
//!
//! Under a theme with a logo pass (`logo_pass`), the lobby comes up and
//! goes (the match starting, a disconnect) at a pass's swap, staying up
//! until the slab covers it (`LobbyGate`).
//!
//! An app whose theme lays the lobby out (`chimera_ui::Theme`,
//! `[layout.lobby]`: the Ultimate Gun Game's) gets the kit's panel
//! instead (`lobby_panel`), from the same view, with the map's game, the
//! slots and the ladder's games (`LobbyView::game`, `slots`,
//! `guns_from`). Otherwise:
//!
//! In the match-end screen's face and shades (`match_end`: the HUD's
//! overlay font, `widgets::display_alpha`), sized by the window's height
//! and the rows (`scale`). `LobbyView` is the state, headless too (tests
//! read it); drawing is Bevy UI rebuilt when it changes. Nothing shows
//! without a lobby. Part of `ClientPlugins`.
//!
//! Sounds (the ready, the others readying, the countdown's last seconds and
//! its go) come from `ui_cues`, only in an app with UI sound.
//!
//! Known shortcuts: no sound for the countdown without UI sound (Lucker
//! Party, the plain apps).

use bevy::{prelude::*, window::PrimaryWindow};
use chimera_net::prematch::{NetLobby, status};
use chimera_sim::minigame::vote;

use crate::{
    core::{LocalPlayer, NetRole},
    net::HOST_ID,
};

/// Adds the screen's root and its state.
pub struct PreMatchLobbyPlugin;

impl Plugin for PreMatchLobbyPlugin {
    fn build(&self, app: &mut App) {
        super::staging::staging(app);
        super::hud::hud_cover(app);
        app.init_resource::<LobbyView>()
            .init_resource::<LobbyGate>()
            .init_resource::<LobbyAsked>()
            .init_resource::<LobbyOverlay>()
            .add_message::<LobbyAction>()
            .add_observer(pass_covered)
            .add_systems(Startup, spawn)
            .add_systems(
                Update,
                (
                    follow.in_set(LobbyFollow),
                    cover.after(LobbyFollow),
                    act.after(LobbyFollow),
                    draw.after(LobbyFollow).run_if(not(super::lobby_panel::themed)),
                ),
            );
        super::lobby_panel::add(app);
        use crate::console::ConsoleAppExt;
        app.console_command(
            "chimera:lobby_press",
            "lobby_press <ready|settings|report|leave>: press that button of the pre-match lobby (tests, screenshots).",
            |w, a| {
                let action = match a.first().map(String::as_str) {
                    Some("ready") => LobbyAction::Ready,
                    Some("settings") => LobbyAction::Settings,
                    Some("report") => LobbyAction::Report,
                    Some("leave") => LobbyAction::Leave,
                    other => return Err(format!("no button {other:?}")),
                };
                w.write_message(action);
                Ok(None)
            },
        );
    }
}

/// Whether the lobby's screen is up (`LobbyView::shown`): it owns the
/// screen and the input (no HUD, no world drawn, the mouse free, Esc the
/// app's).
pub fn lobby_shown(world: &World) -> bool {
    world.get_resource::<LobbyView>().is_some_and(|v| v.shown)
}

/// While the lobby shows, it stands in for the match: the world's view
/// off (`staging::WorldCover`) and the HUDs covered (`hud::HudCover`,
/// our Signal HUD too).
fn cover(
    view: Res<LobbyView>,
    mut world: ResMut<super::staging::WorldCover>,
    mut huds: ResMut<super::hud::HudCover>,
) {
    super::staging::WorldCover::put(&mut world, super::staging::LOBBY, view.shown);
    super::hud::HudCover::put(&mut huds, super::hud::LOBBY_COVER, view.shown);
}

/// What the lobby's buttons do (also sent by tests): the panel's Ready
/// (the key's line: the host's start, a player's ready or not),
/// Settings, Report a bug and Leave.
#[derive(Message, Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyAction {
    Ready,
    /// The app's settings over the lobby (`LobbyAppAsked`).
    Settings,
    /// The report dialog (`feedback::request`).
    Report,
    Leave,
}

/// What a player asked of this lobby while not on its map yet (still
/// downloading or loading): ready (`lobby_ready 1`) or not; reset when
/// the lobby goes.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LobbyAsked(pub bool);

/// An app that answers the lobby's Settings and Leave itself
/// (`LobbyAppAsked`: the party apps open their Settings over the lobby
/// and ask before leaving); without it, Leave is `disconnect` and
/// Settings does nothing.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct LobbyAnswered;

/// The lobby's Settings or Leave was pressed in an app that answers them
/// (`LobbyAnswered`).
#[derive(Event, Clone, Copy, Debug)]
pub struct LobbyAppAsked(pub LobbyAction);

/// An app's overlay is up over the lobby (its Settings, a confirm, the
/// report dialog: the app keeps it): the themed panel hides its key
/// hints; the lobby itself goes on under it.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LobbyOverlay(pub bool);

/// The lobby's actions: Ready runs what the key would (a player not yet
/// on the map readies up all the same: the server keeps it for when it
/// is), Settings and Leave the app's (`LobbyAppAsked`; else Leave is
/// `disconnect`), Report a bug the report dialog.
fn act(
    mut actions: MessageReader<LobbyAction>,
    view: Res<LobbyView>,
    mut asked: ResMut<LobbyAsked>,
    answered: Option<Res<LobbyAnswered>>,
    mut console: Option<ResMut<crate::console::Console>>,
    mut commands: Commands,
) {
    if !view.shown {
        actions.clear();
        if asked.0 {
            asked.0 = false;
        }
        return;
    }
    for a in actions.read() {
        let line = match a {
            LobbyAction::Ready => match view.key_line() {
                Some("lobby_start") => Some("lobby_start".to_string()),
                Some(_) => {
                    let ready = view.rows.iter().any(|r| r.local && r.status == status::READY);
                    Some(format!("lobby_ready {}", if ready { 0 } else { 1 }))
                }
                // Still downloading or loading: kept on the server for
                // when it has the map.
                None if !view.hosting && view.rows.iter().any(|r| r.local) => {
                    asked.0 = !asked.0;
                    Some(format!("lobby_ready {}", if asked.0 { 1 } else { 0 }))
                }
                None => None,
            },
            LobbyAction::Settings | LobbyAction::Leave if answered.is_some() => {
                commands.trigger(LobbyAppAsked(*a));
                None
            }
            LobbyAction::Settings => None,
            LobbyAction::Leave => Some("disconnect".to_string()),
            LobbyAction::Report => {
                commands.queue(|w: &mut World| {
                    if w.contains_resource::<super::feedback::Feedback>() {
                        super::feedback::request(w, true);
                    }
                });
                None
            }
        };
        if let (Some(line), Some(c)) = (line, console.as_deref_mut()) {
            c.submit(line);
        }
    }
}

/// Where the lobby is in its passes (`logo_pass`): waiting for the swap
/// to come up, up, or still up until the swap takes it down.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LobbyGate {
    #[default]
    Off,
    Entering,
    In,
    Leaving,
}

/// The lobby comes up or goes at the pass's swap.
fn pass_covered(ev: On<super::logo_pass::PassCovered>, mut gate: ResMut<LobbyGate>, mut view: ResMut<LobbyView>) {
    if ev.has("lobby_enter") && *gate == LobbyGate::Entering {
        *gate = LobbyGate::In;
    }
    if ev.has("lobby_leave") && *gate == LobbyGate::Leaving {
        *gate = LobbyGate::Off;
        view.set_if_neq(LobbyView::default());
    }
}

/// `follow`: the view from the lobby (the panels draw after it).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LobbyFollow;

/// One player's row as shown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LobbyViewRow {
    pub name: String,
    pub bot: bool,
    /// A listen server's host (`HOST_ID`).
    pub host: bool,
    /// `prematch::status::*`.
    pub status: u8,
    /// The status's words.
    pub state: String,
    /// Downloading or loading: the share done (None: no bar).
    pub fraction: Option<f32>,
    /// Loading: the stage under way (`chimera_net::net::load_stages`), when told.
    pub step: Option<chimera_net::net::load_stages::LoadStep>,
    /// Downloading: "12.3 / 80.0 MB".
    pub bytes: String,
    /// "48 ms", "BOT" or empty.
    pub ping: String,
    /// The round trip, ms (a remote player's).
    pub ping_ms: Option<u16>,
    pub local: bool,
    /// Matches won this server run (`LobbyRow::wins`).
    pub wins: u32,
    /// The avatar's byte (`LobbyRow::avatar`; `prematch::NO_AVATAR`: none).
    pub avatar: u8,
    /// The party (`LobbyRow::party`; 0 alone).
    pub party: u8,
}

/// What the screen shows (headless too).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LobbyView {
    pub shown: bool,
    /// The minigame's name.
    pub mode: String,
    /// The map's name (without its game).
    pub map: String,
    /// What it waits for.
    pub line: String,
    pub rows: Vec<LobbyViewRow>,
    /// Seconds left of the countdown, once it began.
    pub countdown: Option<u32>,
    /// This machine is the server with its own player (a listen host): its
    /// key starts the match.
    pub hosting: bool,
    /// What the key does now, told under the players (empty: nothing).
    pub hint: String,
    /// The map's game, short (`CS:S`; empty when not registered).
    pub game: String,
    /// The server's player slots (`maxplayers`), when known.
    pub slots: Option<u32>,
    /// The games the ladder's guns come from, short, in rung order
    /// (empty off a ladder or before the client has it).
    pub guns_from: Vec<String>,
    /// It waits for the host's Start (`NetLobby::host_start`).
    pub host_start: bool,
}

/// The lobby's key while it shows (a Source key name), in place of its bind.
pub const LOBBY_KEY: &str = "f3";

impl LobbyView {
    /// The console line the lobby's key runs now: the host's
    /// `lobby_start` before a countdown runs, a player's `lobby_ready`
    /// once on the map; None otherwise.
    pub fn key_line(&self) -> Option<&'static str> {
        if !self.shown {
            return None;
        }
        if self.hosting {
            return self.countdown.is_none().then_some("lobby_start");
        }
        self.rows
            .iter()
            .any(|r| r.local && matches!(r.status, status::READY | status::NOT_READY))
            .then_some("lobby_ready")
    }
}

/// The key's words under the players, for what `key_line` does.
fn hint(v: &LobbyView) -> String {
    let key = LOBBY_KEY.to_uppercase();
    match v.key_line() {
        Some("lobby_start") => format!("Press {key} to start the match now"),
        Some(_) if v.rows.iter().any(|r| r.local && r.status == status::NOT_READY) => {
            format!("Press {key} when you are ready")
        }
        _ => String::new(),
    }
}

/// "1:12" or "45 s".
fn duration(secs: u32) -> String {
    if secs >= 60 { format!("{}:{:02}", secs / 60, secs % 60) } else { format!("{secs} s") }
}

/// The line under the map: what the lobby waits for.
pub fn waiting_line(l: &NetLobby) -> String {
    let not_ready = l.rows.iter().filter(|r| !r.ready()).count();
    if l.countdown.is_some() {
        return match not_ready {
            0 => "Everyone is ready".into(),
            1 => "Starting without 1 player still loading (they join when ready)".into(),
            n => format!("Starting without {n} players still loading (they join when ready)"),
        };
    }
    if l.host_start {
        return "Waiting for the host to start".into();
    }
    let who = match l.waiting {
        0 => "Waiting for the round...".to_string(),
        1 => "Waiting for 1 player...".to_string(),
        n => format!("Waiting for {n} players..."),
    };
    match l.wait_left {
        Some(s) => format!("{who}   starts anyway in {}", duration(s)),
        None => who,
    }
}

/// A lobby row as shown, `local` for this machine's player.
pub fn view_row(r: &chimera_net::prematch::LobbyRow, local: bool) -> LobbyViewRow {
    let downloading = r.status == status::DOWNLOADING;
    let mb = |b: u64| b as f64 / 1e6;
    let (fraction, bytes) = match (downloading, r.total) {
        (true, 0) => (Some(0.0), if r.done > 0 { format!("{:.1} MB", mb(r.done)) } else { String::new() }),
        (true, t) => (
            Some((r.done as f64 / t as f64).clamp(0.0, 1.0) as f32),
            format!("{:.1} / {:.1} MB", mb(r.done), mb(t)),
        ),
        _ => (None, String::new()),
    };
    // Loading: the stage under way and its share (`LobbyRow::stage`).
    let stage = r.stage().filter(|_| r.status == status::LOADING);
    let fraction = fraction.or(stage.map(|(_, f)| f));
    LobbyViewRow {
        name: r.name.clone(),
        bot: r.bot(),
        host: r.id == Some(HOST_ID),
        status: r.status,
        state: match (r.bot(), downloading, fraction) {
            (true, ..) => "Ready".into(),
            (_, true, Some(f)) if r.total > 0 => format!("Downloading {:.0}%", f * 100.0),
            _ => match stage {
                Some((step, f)) => format!("{} {:.0}%", step.words(), f * 100.0),
                None => status::name(r.status).into(),
            },
        },
        step: stage.map(|(s, _)| s),
        fraction,
        bytes,
        ping_ms: r.ping_ms,
        ping: match (r.bot(), r.ping_ms) {
            (true, _) => "BOT".into(),
            (_, Some(p)) => format!("{p} ms"),
            _ => String::new(),
        },
        local,
        wins: r.wins,
        avatar: r.avatar,
        party: r.party,
    }
}

/// This machine's own load, for its own row (`chimera_net::net::load_stages`).
#[derive(bevy::ecs::system::SystemParam)]
pub struct OwnLoad<'w> {
    join: Option<Res<'w, chimera_net::net::client::JoinProgress>>,
    ahead: Option<Res<'w, chimera_net::net::assets::MapAhead>>,
    local: Option<Res<'w, chimera_world::map::LocalMapLoad>>,
    work: Option<Res<'w, chimera_net::net::load_stages::LoadWork>>,
    reporter: Option<Res<'w, chimera_world::map::loading::LoadReporter>>,
}

impl OwnLoad<'_> {
    /// The stage under way here and its share, with the download's bytes.
    fn current(&self) -> Option<(chimera_net::net::load_stages::LoadStep, Option<f32>, Option<(u64, u64)>)> {
        use chimera_net::net::load_stages::{Inputs, StageState, stages_of};
        let local = self.local.is_some();
        let i = Inputs {
            join: self.join.as_deref(),
            ahead: self.ahead.as_deref().filter(|a| a.running()).map(|a| a.bytes()),
            local,
            work: self.work.as_deref().copied().filter(|_| local),
            map_share: self.reporter.as_ref().and_then(|r| r.current()).map(|p| p.fraction),
        };
        stages_of(&i).into_iter().find_map(|s| match s.state {
            StageState::Running(f) => Some((s.step, f, s.bytes)),
            _ => None,
        })
    }
}

/// Row `r` with this machine's own stage now (`OwnLoad`).
fn own_row(
    r: &chimera_net::prematch::LobbyRow,
    (step, share, bytes): (chimera_net::net::load_stages::LoadStep, Option<f32>, Option<(u64, u64)>),
) -> chimera_net::prematch::LobbyRow {
    let mut r = r.clone();
    let download = step == chimera_net::net::load_stages::LoadStep::Download;
    r.status = if download { status::DOWNLOADING } else { status::LOADING };
    r.step = step.index();
    r.permille = chimera_net::net::load_stages::permille(share);
    (r.done, r.total) = bytes.filter(|_| download).unwrap_or((0, 0));
    r
}

/// `ui_scene lobby host`: the staged lobby seen as its listen host (the
/// `HOST_ID` row is yours, Start is yours), its slots.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct StagedLobbyHost {
    pub slots: u32,
}

#[allow(clippy::too_many_arguments)]
fn follow(
    staged: Option<Res<StagedLobbyHost>>,
    lobby: Query<&NetLobby>,
    me: Option<Res<chimera_net::net::client::LocalClientId>>,
    role: Option<Res<NetRole>>,
    host: Query<(), With<LocalPlayer>>,
    games: Option<Res<chimera_core::game_registry::Games>>,
    settings: Option<Res<chimera_net::net::NetSettings>>,
    join: Option<Res<chimera_net::net::client::JoinProgress>>,
    ladder: Option<Res<chimera_sim::minigame::ladder::Ladder>>,
    (mut passes, mut gate, first): (super::logo_pass::Passes, ResMut<LobbyGate>, Query<(), With<chimera_net::prematch::HostLobbyFirst>>),
    mut view: ResMut<LobbyView>,
    own: OwnLoad,
) {
    // A listen host's lobby shown from its Start (`prematch::show_first`):
    // seen as its host, as the staged one.
    let first = !first.is_empty();
    // Through the logo pass when the theme has one: up and down at its swap.
    let pass = passes.on();
    let Some(l) = lobby.iter().next() else {
        if view.shown {
            if pass {
                if *gate != LobbyGate::Leaving {
                    *gate = LobbyGate::Leaving;
                    passes.ask("lobby_leave");
                }
                return;
            }
            view.set_if_neq(LobbyView::default());
        }
        if *gate != LobbyGate::Off && *gate != LobbyGate::Entering {
            *gate = LobbyGate::Off;
        }
        return;
    };
    if pass {
        match *gate {
            LobbyGate::Off => {
                *gate = LobbyGate::Entering;
                passes.ask("lobby_enter");
                return;
            }
            LobbyGate::Entering => return,
            LobbyGate::Leaving => *gate = LobbyGate::In,
            LobbyGate::In => {}
        }
    }
    let mine = match role.as_deref() {
        _ if staged.is_some() || first => Some(HOST_ID),
        Some(NetRole::Client) => me.map(|m| m.0),
        Some(NetRole::Server) if !host.is_empty() => Some(HOST_ID),
        _ => None,
    };
    let mut v = LobbyView {
        shown: true,
        mode: l.mode.clone(),
        map: vote::map_name(&l.map).to_string(),
        line: waiting_line(l),
        rows: l
            .rows
            .iter()
            .map(|r| {
                let local = mine.is_some() && r.id == mine;
                match own.current().filter(|_| local && matches!(r.status, status::LOADING | status::DOWNLOADING)) {
                    Some(o) => view_row(&own_row(r, o), true),
                    None => view_row(r, local),
                }
            })
            .collect(),
        countdown: l.countdown,
        hosting: staged.is_some() || first || (role.as_deref() == Some(&NetRole::Server) && !host.is_empty()),
        hint: String::new(),
        game: String::new(),
        slots: None,
        guns_from: Vec::new(),
        host_start: l.host_start,
    };
    v.hint = hint(&v);
    let short = |id: &str| games.as_ref().and_then(|g| g.get(id)).map(|g| g.short_name.to_string());
    v.game = l.map.split_once(':').and_then(|(g, _)| short(g)).unwrap_or_default();
    v.slots = match role.as_deref() {
        _ if staged.is_some() => staged.map(|s| s.slots),
        _ if first => settings.map(|s| s.maxplayers).filter(|&n| n > 1),
        Some(NetRole::Server) => settings.map(|s| s.maxplayers).filter(|&n| n > 1),
        Some(NetRole::Client) => join.and_then(|j| j.server.as_ref().map(|s| s.max_players)).filter(|&n| n > 0),
        _ => None,
    };
    if let Some(ladder) = ladder {
        for rung in &ladder.rungs {
            if let Some(g) = rung.split_once(':').and_then(|(g, _)| short(g))
                && !v.guns_from.contains(&g)
            {
                v.guns_from.push(g);
            }
        }
    }
    view.set_if_neq(v);
}

/// The screen's root (hidden while there is no lobby).
#[derive(Component)]
pub struct LobbyRoot;

/// What `draw` (or the themed panel, `lobby_panel`) rebuilds.
#[derive(Component)]
pub struct LobbyContent;

/// Where it draws: over the HUDs, the match-end screen
/// (`match_end::MATCH_END_Z`) and an app's loading panel, under the Tab
/// scoreboard (`minigame_scoreboard::SCOREBOARD_Z`).
pub const LOBBY_Z: i32 = 77;

const TEXT: Color = Color::srgb(0.95, 0.95, 0.95);
const DIM: Color = Color::srgb(0.7, 0.7, 0.72);
const GOLD: Color = Color::srgb(1.0, 0.82, 0.25);
const LOCAL_BG: Color = Color::srgba(0.85, 0.62, 0.1, 0.32);
const READY: Color = Color::srgb(0.45, 0.88, 0.45);
const DOWNLOADING: Color = Color::srgb(1.0, 0.72, 0.25);
const LOADING: Color = Color::srgb(0.5, 0.75, 1.0);
const NOT_READY: Color = Color::srgb(1.0, 0.5, 0.42);
const BAR_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.12);
/// Name, status, progress (bar and bytes), ping, on a 720-line screen.
const WIDTHS: [f32; 4] = [220.0, 170.0, 250.0, 70.0];
const GAP: f32 = 16.0;
const CELL: f32 = 17.0;
/// The key's line under the players.
const HINT: f32 = 15.0;
/// A name's tags (HOST, BOT, YOU) after it.
const TAG: f32 = 11.0;

/// Text on this machine's own row, which sits on the gold band: near-black
/// (the status colours and the grey ping do not read there; the status word
/// says what the colour would).
const ON_LOCAL: Color = Color::srgb(0.06, 0.05, 0.03);
/// The download bar's track and fill on the own row: a dark track, a white fill.
const LOCAL_BAR_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.4);
const LOCAL_BAR_FILL: Color = Color::WHITE;

/// The status text's colour; `local` for this machine's own row.
fn status_text(s: u8, local: bool) -> Color {
    if local { ON_LOCAL } else { status_color(s) }
}

fn status_color(s: u8) -> Color {
    match s {
        status::READY => READY,
        status::DOWNLOADING => DOWNLOADING,
        status::LOADING => LOADING,
        status::NOT_READY => NOT_READY,
        _ => DIM,
    }
}

/// The screen's scale at a window `height` pixels tall with `rows` rows:
/// for a 720-line screen, less when that many rows wouldn't fit.
pub fn scale(height: f32, rows: usize) -> f32 {
    // From the top: the padding, the panel (its padding, three lines, the
    // table's margin and padding, the header and the rows with their
    // gaps), the countdown's box; a fifth over for the fonts' line boxes.
    // The key's line under the players is counted always.
    let panel = 2.0 * 14.0
        + (17.0 + 34.0 + 20.0 + HINT) * 1.3
        + 3.0 * 6.0
        + 10.0
        + 2.0 * 8.0
        + (CELL * 1.3 + 4.0 + 4.0) * (rows + 1) as f32;
    let countdown = 12.0 + 2.0 * 4.0 + (22.0 + 64.0) * 1.3;
    let need = 0.15 * 720.0 + 1.2 * (panel + countdown) + 10.0;
    height.max(240.0) / 720f32.max(need)
}

fn spawn(mut commands: Commands) {
    commands.spawn((
        LobbyRoot,
        Visibility::Hidden,
        GlobalZIndex(LOBBY_Z),
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0),
            height: percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            // Under the games' minigame panel (its title and rung lines).
            padding: UiRect::top(percent(15.0)),
            ..default()
        },
    ));
}

fn label(p: &mut ChildSpawnerCommands, text: impl Into<String>, font: TextFont, color: Color) {
    p.spawn((Text::new(text.into()), font, TextColor(color), TextLayout::justify(Justify::Center)));
}

fn cell(p: &mut ChildSpawnerCommands, text: String, width: f32, right: bool, color: Color, font: &TextFont, s: f32) {
    p.spawn((
        Text::new(text),
        font.clone(),
        TextColor(color),
        TextLayout::new(if right { Justify::Right } else { Justify::Left }, LineBreak::NoWrap),
        Node {
            width: px(width * s),
            flex_shrink: 0.0,
            overflow: Overflow::clip(),
            ..default()
        },
    ));
}

/// The name cell: the name, then its tags small (`colors`: the name's,
/// the tags').
fn name_cell(
    p: &mut ChildSpawnerCommands,
    name: &str,
    tags: &[&str],
    colors: (Color, Color),
    font: &TextFont,
    tag_font: &TextFont,
    s: f32,
) {
    p.spawn(Node {
        width: px(WIDTHS[0] * s),
        flex_shrink: 0.0,
        column_gap: px(6.0 * s),
        align_items: AlignItems::Center,
        overflow: Overflow::clip(),
        ..default()
    })
    .with_children(|c| {
        c.spawn((
            Text::new(name.to_string()),
            font.clone(),
            TextColor(colors.0),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
            Node { flex_shrink: 1.0, overflow: Overflow::clip(), ..default() },
        ));
        for t in tags {
            c.spawn((
                Text::new(t.to_string()),
                tag_font.clone(),
                TextColor(colors.1),
                TextLayout::new(Justify::Left, LineBreak::NoWrap),
                Node { flex_shrink: 0.0, ..default() },
            ));
        }
    });
}

#[allow(clippy::type_complexity)]
fn draw(
    view: Res<LobbyView>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut root: Query<(Entity, &mut Visibility), With<LobbyRoot>>,
    content: Query<Entity, With<LobbyContent>>,
    fonts: Option<Res<super::fonts::UiFonts>>,
    mut drawn: Local<Option<(LobbyView, u32)>>,
    mut commands: Commands,
) {
    let Some((root, mut visibility)) = root.iter_mut().next() else { return };
    let height = window.iter().next().map_or(720.0, |w| w.height());
    let s = scale(height, view.rows.len());
    let key = (s * 1000.0) as u32;
    if drawn.as_ref().is_some_and(|(v, k)| v == &*view && *k == key) && !fonts.as_ref().is_some_and(|f| f.is_changed()) {
        return;
    }
    *drawn = Some((view.clone(), key));
    visibility.set_if_neq(if view.shown { Visibility::Inherited } else { Visibility::Hidden });
    for e in &content {
        commands.entity(e).despawn();
    }
    if !view.shown {
        return;
    }
    let fonts = fonts.as_deref();
    let font = |size: f32, bold: bool| super::fonts::overlay_font(fonts, size * s, bold);
    let panel = commands
        .spawn((
            LobbyContent,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(6.0 * s),
                padding: UiRect::axes(px(26.0 * s), px(14.0 * s)),
                ..default()
            },
            BackgroundColor(super::widgets::display_alpha(Color::srgba(0.02, 0.02, 0.03, 0.72))),
            ChildOf(root),
        ))
        .id();
    let cell_font = font(CELL, false);
    let cell_bold = font(CELL, true);
    commands.entity(panel).with_children(|p| {
        label(p, view.mode.to_uppercase(), font(17.0, false), DIM);
        label(p, view.map.clone(), font(34.0, true), GOLD);
        label(p, view.line.clone(), font(20.0, false), TEXT);
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(4.0 * s),
            margin: UiRect::top(px(10.0 * s)),
            padding: UiRect::all(px(8.0 * s)),
            ..default()
        })
        .insert(BackgroundColor(super::widgets::display_alpha(Color::srgba(0.0, 0.0, 0.0, 0.45))))
        .with_children(|t| {
            t.spawn(Node { column_gap: px(GAP * s), ..default() }).with_children(|r| {
                for (i, (h, w)) in ["Player", "Status", "Download", "Ping"].iter().zip(WIDTHS).enumerate() {
                    cell(r, h.to_string(), w, i == 3, DIM, &cell_font, s);
                }
            });
            for row in &view.rows {
                t.spawn((
                    Node {
                        column_gap: px(GAP * s),
                        padding: UiRect::vertical(px(2.0 * s)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(if row.local { super::widgets::display_alpha(LOCAL_BG) } else { Color::NONE }),
                ))
                .with_children(|r| {
                    let f = if row.local { &cell_bold } else { &cell_font };
                    let name_color = if row.local { ON_LOCAL } else if row.bot { DIM } else { TEXT };
                    let tag_color = if row.local { ON_LOCAL } else { GOLD };
                    // The session wins as one more tag ("2 WON").
                    let won = format!("{} WON", row.wins);
                    let tags: Vec<&str> = [(row.host, "HOST"), (row.bot, "BOT"), (row.local, "YOU"), (row.wins > 0, won.as_str())]
                        .into_iter()
                        .filter_map(|(on, t)| on.then_some(t))
                        .collect();
                    name_cell(r, &row.name, &tags, (name_color, tag_color), f, &font(TAG, true), s);
                    cell(r, row.state.clone(), WIDTHS[1], false, status_text(row.status, row.local), f, s);
                    // The download: a bar and its bytes.
                    r.spawn(Node {
                        width: px(WIDTHS[2] * s),
                        flex_shrink: 0.0,
                        column_gap: px(8.0 * s),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|d| {
                        if let Some(fr) = row.fraction {
                            d.spawn((
                                Node {
                                    width: px(90.0 * s),
                                    height: px(8.0 * s),
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                                BackgroundColor(super::widgets::display_alpha(if row.local { LOCAL_BAR_BG } else { BAR_BG })),
                            ))
                            .with_children(|b| {
                                b.spawn((
                                    Node {
                                        width: percent(100.0 * fr),
                                        height: percent(100.0),
                                        ..default()
                                    },
                                    BackgroundColor(if row.local { LOCAL_BAR_FILL } else { DOWNLOADING }),
                                ));
                            });
                            d.spawn((
                                Text::new(row.bytes.clone()),
                                font(CELL - 3.0, false),
                                TextColor(if row.local { ON_LOCAL } else { TEXT }),
                                TextLayout::new(Justify::Left, LineBreak::NoWrap),
                            ));
                        }
                    });
                    cell(r, row.ping.clone(), WIDTHS[3], true, if row.local { ON_LOCAL } else { DIM }, &cell_font, s);
                });
            }
        });
        if !view.hint.is_empty() {
            label(p, view.hint.clone(), font(HINT, false), GOLD);
        }
    });
    if let Some(left) = view.countdown {
        commands.spawn((
            LobbyContent,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                margin: UiRect::top(px(12.0 * s)),
                padding: UiRect::axes(px(36.0 * s), px(4.0 * s)),
                ..default()
            },
            BackgroundColor(super::widgets::display_alpha(Color::srgba(0.02, 0.02, 0.03, 0.72))),
            ChildOf(root),
        ))
        .with_children(|c| {
            label(c, "Match starts in", font(22.0, false), TEXT);
            label(c, left.to_string(), font(64.0, true), GOLD);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_net::prematch::LobbyRow;

    #[test]
    fn the_own_row_status_is_dark_on_the_gold_band_and_others_keep_their_colour() {
        for st in [status::READY, status::DOWNLOADING, status::LOADING] {
            assert_eq!(status_text(st, true), ON_LOCAL);
            assert_eq!(status_text(st, false), status_color(st));
        }
    }

    #[test]
    fn rows_and_lines_read_as_the_lobby_stands() {
        let row = |id: Option<u64>, name: &str, status: u8, done: u64, total: u64| LobbyRow {
            id,
            name: name.into(),
            status,
            done,
            total,
            step: chimera_net::prematch::NO_STEP,
            permille: 0,
            ping_ms: id.map(|_| 48),
            wins: 0,
            avatar: 0,
            party: 0,
        };
        let mut l = NetLobby {
            map: "cs_source:de_dust2".into(),
            mode: "Ultimate Gun Game".into(),
            rows: vec![
                row(Some(1), "Ann", status::READY, 0, 0),
                row(Some(2), "Ben", status::DOWNLOADING, 25_000_000, 100_000_000),
                row(Some(3), "Cat", status::LOADING, 0, 0),
                row(None, "Bot Dan", status::READY, 0, 0),
            ],
            waiting: 2,
            wait_left: Some(72),
            countdown: None,
            host_start: false,
        };
        assert_eq!(waiting_line(&l), "Waiting for 2 players...   starts anyway in 1:12");
        let ben = view_row(&l.rows[1], true);
        assert_eq!((ben.state.as_str(), ben.bytes.as_str(), ben.fraction), ("Downloading 25%", "25.0 / 100.0 MB", Some(0.25)));
        assert!(ben.local);
        assert_eq!(view_row(&l.rows[3], false).ping, "BOT");
        assert_eq!(view_row(&l.rows[0], false).ping, "48 ms");
        assert_eq!(view_row(&l.rows[2], false).state, "Loading map");
        l.countdown = Some(5);
        assert_eq!(waiting_line(&l), "Starting without 2 players still loading (they join when ready)");
    }

    #[test]
    fn the_key_starts_for_the_host_and_readies_a_player_on_the_map() {
        let me = |status| LobbyViewRow {
            status,
            local: true,
            ..default()
        };
        let mut v = LobbyView {
            shown: true,
            hosting: true,
            rows: vec![me(status::READY)],
            ..default()
        };
        assert_eq!(v.key_line(), Some("lobby_start"));
        assert_eq!(hint(&v), "Press F3 to start the match now");
        v.countdown = Some(4);
        assert_eq!(v.key_line(), None, "a countdown under way isn't started again");
        v = LobbyView {
            shown: true,
            rows: vec![me(status::NOT_READY)],
            ..default()
        };
        assert_eq!(v.key_line(), Some("lobby_ready"));
        assert_eq!(hint(&v), "Press F3 when you are ready");
        v.rows = vec![me(status::READY)];
        assert_eq!((v.key_line(), hint(&v).as_str()), (Some("lobby_ready"), ""), "takes it back");
        v.rows = vec![me(status::DOWNLOADING)];
        assert_eq!(v.key_line(), None, "not on the map yet");
        v.shown = false;
        assert_eq!(v.key_line(), None);
    }
}
