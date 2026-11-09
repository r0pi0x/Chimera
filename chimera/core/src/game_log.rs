//! Game events and the server log: what a game tells server plugins and
//! admins happened, by Source's names (specs name the events and log
//! triggers, e.g. objectives.md `bomb_dropped` and `triggered
//! "Dropped_The_Bomb"`).
//!
//! - **Game events** (`GameEvent`): a name and keyed values (Source's
//!   `modevents.res` names: `userid`, `site`, `x`...), written by the
//!   server's systems for anything that reads them (a plugin, a test, the
//!   bots). Not sent to clients.
//! - **Log lines** (`LogLine`): the half-life log standard's lines, a
//!   player written as `"Name<userid><uniqueid><TEAM>"`: `"<player>"
//!   triggered "Planted_The_Bomb"`, `World triggered "Round_Draw"`, `Team
//!   "CT" triggered "Bomb_Defused"`. `log on|off` turns logging on (off by
//!   default, as in Source); each line is stamped `L MM/DD/YYYY -
//!   HH:MM:SS: `, echoed to the console with `sv_logecho 1` (default) and
//!   appended to a file in the data folder's `logs` with `sv_logfile 1`
//!   (default; `logs/L<MMDD><NNN>.log`, a new one each `log on`), and the
//!   last `KEPT` lines stay in `ServerLog::lines` for tests and the remote
//!   protocol.
//!
//! Who a player is comes from the game's network layer (`LogSubjects`: the
//! default names a character by its `Name`, its entity index as the userid
//! and `BOT` or `STEAM_ID_LAN`), the team's name from the game
//! (`ServerLog::team_names`).
//!
//! Known shortcuts (docs/tech-debt.md): the stamp is UTC (Source's is the
//! server's local time); a bot's userid is its entity index (Source counts
//! bots with the players); humans' unique id is `STEAM_ID_LAN` (no Steam
//! authentication); one file per `log on`, not per map. Why it's OK for
//! now: parsers key on the line shapes; the values differ only where we
//! have no Steam or time zone.

use std::{collections::VecDeque, io::Write};

use bevy::{ecs::message::MessageCursor, prelude::*};

use crate::{
    console::{Console, ConsoleAppExt, Level},
    core::{Bot, Team},
};

/// Lines kept in `ServerLog::lines`.
pub const KEPT: usize = 512;

/// One value of a game event.
#[derive(Clone, Debug, PartialEq)]
pub enum EventValue {
    /// A player: its `userid` when written out.
    Player(Entity),
    Int(i64),
    Float(f32),
    Bool(bool),
    Str(String),
}

/// A game event (Source's `IGameEvent`): a name and keyed values.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct GameEvent {
    pub name: &'static str,
    pub keys: Vec<(&'static str, EventValue)>,
}

impl GameEvent {
    pub fn new(name: &'static str) -> Self {
        Self { name, keys: Vec::new() }
    }

    /// `userid` set to `who`.
    pub fn of(name: &'static str, who: Entity) -> Self {
        Self::new(name).with("userid", EventValue::Player(who))
    }

    pub fn with(mut self, key: &'static str, value: EventValue) -> Self {
        self.keys.push((key, value));
        self
    }

    pub fn get(&self, key: &str) -> Option<&EventValue> {
        self.keys.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    /// The player under `key`.
    pub fn player(&self, key: &str) -> Option<Entity> {
        match self.get(key)? {
            EventValue::Player(e) => Some(*e),
            _ => None,
        }
    }
}

/// A piece of a log line.
#[derive(Clone, Debug, PartialEq)]
pub enum LogPart {
    Text(String),
    /// Written `"Name<userid><uniqueid><TEAM>"`, quotes included.
    Player(Entity),
    /// A team's name (`ServerLog::team_names`), unquoted.
    Team(u8),
}

/// A server log line, before its stamp.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct LogLine(pub Vec<LogPart>);

/// A chat line a player said on the server (the network layer's
/// `deliver_say`), for a game's log lines: who, the cleaned text and
/// whether it was `say_team`.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct Said {
    pub from: Entity,
    pub text: String,
    pub team_only: bool,
}

impl LogLine {
    /// `"<who>" triggered "<what>"`.
    pub fn triggered(who: Entity, what: &str) -> Self {
        Self(vec![LogPart::Player(who), LogPart::Text(format!(" triggered \"{what}\""))])
    }

    /// `World triggered "<what>"`.
    pub fn world(what: &str) -> Self {
        Self(vec![LogPart::Text(format!("World triggered \"{what}\""))])
    }

    /// `Team "<team>" triggered "<what>"`.
    pub fn team(team: Team, what: &str) -> Self {
        Self(vec![
            LogPart::Text("Team \"".into()),
            LogPart::Team(team.0),
            LogPart::Text(format!("\" triggered \"{what}\"")),
        ])
    }
}

/// Who a player is in the log.
#[derive(Clone, Debug, PartialEq)]
pub struct LogSubject {
    pub name: String,
    pub userid: u64,
    /// Source's unique id (a Steam id, `BOT`, `STEAM_ID_LAN`).
    pub unique: String,
}

/// Names a player for the log: the gameplay's rules' entry
/// (`gameplay::GameHooks`), else the one for every game, which the network
/// layer puts (`any_game_hook`), else `default_subject`.
#[derive(Clone, Copy)]
pub struct LogSubjects(pub fn(&World, Entity) -> Option<LogSubject>);

impl Default for LogSubjects {
    fn default() -> Self {
        Self(default_subject)
    }
}

pub fn default_subject(world: &World, e: Entity) -> Option<LogSubject> {
    let name = world.get::<Name>(e).map_or_else(|| "Player".into(), |n| n.as_str().to_string());
    let bot = world.get::<Bot>(e).is_some();
    Some(LogSubject {
        name,
        userid: e.index().index() as u64,
        unique: if bot { "BOT" } else { "STEAM_ID_LAN" }.into(),
    })
}

/// The server log's state and settings.
#[derive(Resource)]
pub struct ServerLog {
    /// `log on`.
    pub enabled: bool,
    /// `sv_logecho`: lines to the console too.
    pub echo: u8,
    /// `sv_logfile`: lines to a file too.
    pub file: u8,
    /// Each team number's name in the log (Source's team names; others
    /// "Unassigned").
    pub team_names: Vec<(u8, String)>,
    /// The newest lines, stamped.
    pub lines: VecDeque<String>,
    open: Option<std::fs::File>,
}

impl Default for ServerLog {
    fn default() -> Self {
        Self {
            enabled: false,
            echo: 1,
            file: 1,
            team_names: Vec::new(),
            lines: VecDeque::new(),
            open: None,
        }
    }
}

impl ServerLog {
    pub fn team_name(&self, team: u8) -> &str {
        self.team_names
            .iter()
            .find(|(t, _)| *t == team)
            .map_or("Unassigned", |(_, n)| n.as_str())
    }
}

pub struct GameLogPlugin;

impl Plugin for GameLogPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<GameEvent>()
            .add_message::<LogLine>()
            .add_message::<Said>()
            .init_resource::<ServerLog>()
            .init_resource::<crate::gameplay::GameHooks<LogSubjects>>()
            .add_systems(Last, write_lines);
        app.console_command(
            "log",
            "log <on|off>: the server log (`sv_logecho` to the console, `sv_logfile` to logs/).",
            |w, a| {
                let mut log = w.resource_mut::<ServerLog>();
                match a.first().map(|s| s.to_lowercase()).as_deref() {
                    Some("on" | "1") => {
                        log.enabled = true;
                        log.open = None;
                        Ok(Some("Server logging enabled.".into()))
                    }
                    Some("off" | "0") => {
                        log.enabled = false;
                        log.open = None;
                        Ok(Some("Server logging disabled.".into()))
                    }
                    _ => Ok(Some(format!(
                        "Usage: log <on|off>\ncurrently logging to: {}",
                        if log.enabled { "console" } else { "none" }
                    ))),
                }
            },
        );
        crate::console::resource_cvar::<ServerLog, u8>(
            app,
            "sv_logecho",
            "1: server log lines are echoed to the console.",
            |l| &mut l.echo,
        );
        crate::console::resource_cvar::<ServerLog, u8>(
            app,
            "sv_logfile",
            "1: server log lines go to a file in logs/.",
            |l| &mut l.file,
        );
    }
}

/// A player as the log writes it: `"Name<userid><uniqueid><TEAM>"`.
pub fn subject_text(world: &World, e: Entity) -> String {
    let resolve = crate::gameplay::active_hook::<LogSubjects>(world, |g| g.rules)
        .unwrap_or_default()
        .0;
    let s = resolve(world, e).unwrap_or_else(|| LogSubject {
        name: "unknown".into(),
        userid: e.index().index() as u64,
        unique: String::new(),
    });
    let team = world.get::<Team>(e).map_or(0, |t| t.0);
    let team = world
        .get_resource::<ServerLog>()
        .map_or_else(|| "Unassigned".to_string(), |l| l.team_name(team).to_string());
    format!("\"{}<{}><{}><{}>\"", s.name, s.userid, s.unique, team)
}

/// The text of a line, unstamped.
pub fn line_text(world: &World, line: &LogLine) -> String {
    line.0
        .iter()
        .map(|p| match p {
            LogPart::Text(t) => t.clone(),
            LogPart::Player(e) => subject_text(world, *e),
            LogPart::Team(t) => world
                .get_resource::<ServerLog>()
                .map_or_else(|| "Unassigned".to_string(), |l| l.team_name(*t).to_string()),
        })
        .collect()
}

/// `L MM/DD/YYYY - HH:MM:SS: ` for seconds since the Unix epoch (UTC).
pub fn stamp(unix: u64) -> String {
    let (days, secs) = (unix / 86400, unix % 86400);
    let (y, m, d) = civil(days as i64);
    format!(
        "L {m:02}/{d:02}/{y:04} - {:02}:{:02}:{:02}: ",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A new file in the data folder's `logs` (`L<MMDD><NNN>.log`).
fn open_file() -> Option<std::fs::File> {
    let dir = crate::console::cfg_dir()?.parent()?.join("logs");
    std::fs::create_dir_all(&dir).ok()?;
    let date = stamp(now());
    let (m, d) = (&date[2..4], &date[5..7]);
    (0..1000).find_map(|n| {
        std::fs::OpenOptions::new()
            .append(true)
            .create_new(true)
            .open(dir.join(format!("L{m}{d}{n:03}.log")))
            .ok()
    })
}

/// This frame's log lines, stamped, to where the settings send them.
fn write_lines(world: &mut World, mut cursor: Local<MessageCursor<LogLine>>) {
    let lines: Vec<LogLine> = cursor
        .read(world.resource::<Messages<LogLine>>())
        .cloned()
        .collect();
    if lines.is_empty() || !world.resource::<ServerLog>().enabled {
        return;
    }
    let prefix = stamp(now());
    let texts: Vec<String> = lines.iter().map(|l| format!("{prefix}{}", line_text(world, l))).collect();
    let echo = {
        let mut log = world.resource_mut::<ServerLog>();
        if log.file != 0 && log.open.is_none() {
            log.open = open_file();
        }
        for t in &texts {
            if log.file != 0
                && let Some(f) = log.open.as_mut()
            {
                let _ = writeln!(f, "{t}");
            }
            log.lines.push_back(t.clone());
            if log.lines.len() > KEPT {
                log.lines.pop_front();
            }
        }
        log.echo != 0
    };
    if echo && let Some(mut console) = world.get_resource_mut::<Console>() {
        for t in texts {
            console.print(Level::Info, t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_read_as_dates() {
        assert_eq!(stamp(0), "L 01/01/1970 - 00:00:00: ");
        // 2026-10-18 08:12:07 UTC.
        assert_eq!(stamp(1_792_311_127), "L 10/18/2026 - 08:12:07: ");
        // A leap day.
        assert_eq!(stamp(951_782_400), "L 02/29/2000 - 00:00:00: ");
    }

    #[test]
    fn lines_name_players_and_teams() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, crate::console::ConsolePlugin, GameLogPlugin));
        app.world_mut().resource_mut::<ServerLog>().team_names = vec![(1, "TERRORIST".into()), (2, "CT".into())];
        let bot = app.world_mut().spawn((Name::new("Bob"), Bot, Team(1))).id();
        let w = app.world();
        let id = bot.index().index();
        assert_eq!(
            line_text(w, &LogLine::triggered(bot, "Got_The_Bomb")),
            format!("\"Bob<{id}><BOT><TERRORIST>\" triggered \"Got_The_Bomb\"")
        );
        assert_eq!(line_text(w, &LogLine::team(Team(2), "Bomb_Defused")), "Team \"CT\" triggered \"Bomb_Defused\"");
        assert_eq!(line_text(w, &LogLine::world("Round_Draw")), "World triggered \"Round_Draw\"");
        // Off by default; on, lines are kept (and echoed).
        app.world_mut().write_message(LogLine::world("Round_Start"));
        app.update();
        assert!(app.world().resource::<ServerLog>().lines.is_empty());
        app.world_mut().resource_mut::<ServerLog>().file = 0;
        app.world_mut().resource_mut::<ServerLog>().enabled = true;
        app.world_mut().write_message(LogLine::world("Round_Start"));
        app.update();
        let log = app.world().resource::<ServerLog>();
        assert_eq!(log.lines.len(), 1);
        assert!(log.lines[0].starts_with("L ") && log.lines[0].ends_with(": World triggered \"Round_Start\""));
    }
}
