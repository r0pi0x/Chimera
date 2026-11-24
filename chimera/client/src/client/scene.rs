//! Scene scripts: `--scene <file> --out <dir>` stages and photographs a
//! scene in one run, in-process (no driver script, no remote port): a text
//! file of console lines and a few directives, run frame by frame, with
//! the pictures and a log of what happened written to `--out`. With
//! `--offscreen` it is the agents' way to say "this body with this gun at
//! N metres" or "the moment it dies, and 15 frames later" and to take the
//! same pictures again after a code change.
//!
//! The file, a line each (`#` starts a comment):
//! - `wait <frames>`: go on that many frames later.
//! - `wait_for <fact> [timeout frames]` (default 1800): go on the frame the
//!   fact holds; past the timeout the run fails (exit code 3, the line
//!   named on stderr and in the log).
//! - `wait_for cvar <name> <value> [timeout frames]`: the same, for a
//!   console variable holding that value (numbers compare as numbers).
//! - `gwait <seconds>`: go on once that much game time (`Time<Virtual>`,
//!   which `host_timescale` slows) has passed: a moment after a shot that
//!   holds on any machine's frame rate (reference comparisons, e.g.
//!   tools/tf2ref, step through effects at `host_timescale 0.02`).
//! - `shot <name>`: save the window as `<out>/<name>.png` now.
//! - `shot_after <fact> <frames> <name>`: arm a picture the given frames
//!   after the next frame the fact holds (the script goes on at once; the
//!   run waits for armed pictures at its end, up to 1800 frames).
//! - `type <text>`: the text typed, a key press a character (into the
//!   console after `toggleconsole`, or a chat line); `key <name>
//!   [times]`: a key pressed (`tab`, `enter`, `escape`, `up`, `down`,
//!   ...: `KEYS`), the script going on the next frame.
//! - Anything else is a console line, submitted that frame (`setpos`,
//!   `setang`, `thirdperson`, `cam_idealdist`, `cam_idealyaw`, `rung`,
//!   `target add`, `target look`, `+attack`, ...).
//!
//! The facts (`Fact`), each from what the engine already tells:
//! `map_loaded` (a map is in and no match's map change is under way:
//! `minigame::MatchHold`, `minigame_net::MapChange`), `match`
//! (a minigame's match is live), `spawned` (the local player is alive),
//! `shot` (any `WeaponEventKind::ShotMade`), `hit` (any `DamageTaken`),
//! `kill` (a `Died` with a killer), `death` (any `Died`), `ragdoll` (a
//! body became a ragdoll, `map::ragdoll::Ragdolled`), `blast` (any
//! game's blast, `weapon::grenade::BlastDealt`, or a `core::Explosion`).
//! `idle` (no background work pending: loads, model builds, downloads,
//! navigation meshes: `chimera_core::tasks::pending`; like `map_loaded`, it
//! also holds in the first frames before anything started, so put it after
//! the console line that starts the work and a `wait 2`). `still` (no
//! logo pass between screens under way, `logo_pass::LogoPass`: a screen
//! change done, its slab gone, whatever the frame rate; also put it after
//! the console line that changes the screen and a `wait 2`). `shot`, `hit`,
//! `kill`, `death`, `ragdoll` and
//! `blast` hold on the frame they happen; `map_loaded`, `match`, `spawned`, `idle`
//! and `still` while they are so.
//!
//! The log, `<out>/scene.log`: every console line, directive, fact and
//! picture with its frame number, and every `HitMeasured`,
//! `KillMeasured`, `BlastMeasured` and `PropHit`
//! (`chimera_sim::minigame::targets`) as it came, and the console's
//! output (lines starting `| `: `target list`, `prop group`), so a scene
//! is also a measurement. The run exits when the script is done and its
//! pictures are saved (0), or on a timeout (3).
//!
//! A scene run is a tool run (`Args::tool_run`: loopback, no config
//! written) and serves no remote protocol.
//!
//! Known shortcuts: the frame count is the app's `Update` frames, so a
//! fact's frame and a picture's moment depend on the machine's frame rate
//! where the simulation's tick and the frame differ; the first picture
//! waits for no pipelines (`capture`'s `--screenshot` does): a scene
//! should `wait` some frames after `spawned` first. A game whose blast
//! writes no `BlastDealt` (TF2's own, for now) is no `blast`.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use bevy::{
    ecs::message::MessageCursor,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use chimera_sim::minigame::targets::{BlastMeasured, HitMeasured, KillMeasured, PropHit, blast_line};
use chimera_sim::weapon::{WeaponEvent, WeaponEventKind, grenade::BlastDealt};

use super::ClientArgs;
use crate::core::{DamageTaken, Dead, Died, Explosion, LocalPlayer, MapLoads};

/// How long `wait_for` waits by default, and the end for armed pictures
/// (frames).
pub const DEFAULT_TIMEOUT: u64 = 1800;

/// Something that happens, which a scene waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fact {
    MapLoaded,
    Match,
    Spawned,
    Shot,
    Hit,
    Kill,
    Death,
    Ragdoll,
    Blast,
    Idle,
    Still,
}

impl Fact {
    pub const ALL: [Fact; 11] = [
        Fact::MapLoaded,
        Fact::Match,
        Fact::Spawned,
        Fact::Shot,
        Fact::Hit,
        Fact::Kill,
        Fact::Death,
        Fact::Ragdoll,
        Fact::Blast,
        Fact::Idle,
        Fact::Still,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Fact::MapLoaded => "map_loaded",
            Fact::Match => "match",
            Fact::Spawned => "spawned",
            Fact::Shot => "shot",
            Fact::Hit => "hit",
            Fact::Kill => "kill",
            Fact::Death => "death",
            Fact::Ragdoll => "ragdoll",
            Fact::Blast => "blast",
            Fact::Idle => "idle",
            Fact::Still => "still",
        }
    }

    pub fn parse(s: &str) -> Option<Fact> {
        Fact::ALL.into_iter().find(|f| f.name().eq_ignore_ascii_case(s))
    }

    fn bit(self) -> u16 {
        1 << (self as u16)
    }
}

/// One line of a scene.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Console(String),
    Wait(u64),
    WaitFor(Fact, u64),
    /// `wait_for cvar <name> <value> [timeout]`.
    WaitForCvar(String, String, u64),
    Shot(String),
    ShotAfter(Fact, u64, String),
    /// `type <text>`: each character as a key press, as a player types it.
    Type(String),
    /// `key <name> [times]`: a key pressed (`KEYS`' names).
    Key(KeyCode, u64),
    /// `gwait <seconds>`: go on after that much game (virtual) time.
    GameWait(f64),
}

/// The keys `key` presses, by name.
pub const KEYS: [(&str, KeyCode); 13] = [
    ("tab", KeyCode::Tab),
    ("enter", KeyCode::Enter),
    ("escape", KeyCode::Escape),
    ("up", KeyCode::ArrowUp),
    ("down", KeyCode::ArrowDown),
    ("left", KeyCode::ArrowLeft),
    ("right", KeyCode::ArrowRight),
    ("backspace", KeyCode::Backspace),
    ("delete", KeyCode::Delete),
    ("home", KeyCode::Home),
    ("end", KeyCode::End),
    ("space", KeyCode::Space),
    ("backquote", KeyCode::Backquote),
];

/// A scene's text as steps (line numbers from 1 with them), or the first
/// bad line.
pub fn parse(text: &str) -> Result<Vec<(usize, Step)>, String> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let num = |s: Option<&&str>, what: &str| -> Result<u64, String> {
            s.and_then(|s| s.parse().ok()).ok_or_else(|| format!("line {n}: {what}: expected a number of frames"))
        };
        let fact = |s: Option<&&str>| -> Result<Fact, String> {
            let s = s.ok_or_else(|| format!("line {n}: a fact is missing"))?;
            Fact::parse(s).ok_or_else(|| {
                format!("line {n}: no fact {s:?} (one of {})", Fact::ALL.map(Fact::name).join(", "))
            })
        };
        let step = match words[0] {
            "wait" => Step::Wait(num(words.get(1), "wait")?),
            "wait_for" if words.get(1) == Some(&"cvar") => {
                let (Some(name), Some(value)) = (words.get(2), words.get(3)) else {
                    return Err(format!("line {n}: wait_for cvar <name> <value> [timeout frames]"));
                };
                let timeout = words.get(4).map_or(Ok(DEFAULT_TIMEOUT), |_| num(words.get(4), "wait_for cvar's timeout"))?;
                Step::WaitForCvar(name.to_string(), value.to_string(), timeout)
            }
            "wait_for" => Step::WaitFor(
                fact(words.get(1))?,
                words.get(2).map_or(Ok(DEFAULT_TIMEOUT), |_| num(words.get(2), "wait_for's timeout"))?,
            ),
            "gwait" => Step::GameWait(
                words
                    .get(1)
                    .and_then(|s| s.parse::<f64>().ok())
                    .filter(|s| *s >= 0.0)
                    .ok_or_else(|| format!("line {n}: gwait <game seconds>"))?,
            ),
            "shot" => Step::Shot(words.get(1).ok_or_else(|| format!("line {n}: shot <name>"))?.to_string()),
            "shot_after" => Step::ShotAfter(
                fact(words.get(1))?,
                num(words.get(2), "shot_after")?,
                words.get(3).ok_or_else(|| format!("line {n}: shot_after <fact> <frames> <name>"))?.to_string(),
            ),
            "type" => Step::Type(
                line.strip_prefix("type")
                    .map(|t| t.strip_prefix(' ').unwrap_or(t).to_string())
                    .filter(|t| !t.is_empty())
                    .ok_or_else(|| format!("line {n}: type <text>"))?,
            ),
            "key" => {
                let name = words.get(1).ok_or_else(|| format!("line {n}: key <name> [times]"))?;
                let code = KEYS.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, c)| *c).ok_or_else(|| {
                    format!("line {n}: no key {name:?} (one of {})", KEYS.map(|(k, _)| k).join(", "))
                })?;
                Step::Key(code, words.get(2).map_or(Ok(1), |_| num(words.get(2), "key's times"))?)
            }
            _ => Step::Console(line.to_string()),
        };
        out.push((n, step));
    }
    Ok(out)
}

/// A scene being played.
#[derive(Resource)]
struct Scene {
    steps: Vec<(usize, Step)>,
    next: usize,
    out: PathBuf,
    frame: u64,
    resume: u64,
    /// `gwait`: the game time (`Time<Virtual>` elapsed) to go on at.
    resume_game: Option<f64>,
    waiting: Option<(Fact, u64, usize)>,
    /// `wait_for cvar`: the name, the value, the frame it gives up on, the line.
    waiting_cvar: Option<(String, String, u64, usize)>,
    armed: Vec<(Fact, u64, String)>,
    due: Vec<(u64, String)>,
    in_flight: Arc<AtomicUsize>,
    log: Vec<String>,
    ended: Option<u64>,
}

impl Scene {
    fn note(&mut self, text: impl Into<String>) {
        let line = format!("{:>6}  {}", self.frame, text.into());
        info!("scene: {line}");
        self.log.push(line);
    }
}

/// The facts of this frame (bits of `Fact::bit`).
#[derive(Resource, Default)]
struct Facts(u16);

/// Scene scripts (`--scene`): read at start, played in `Update`.
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        let Some(args) = app.world().get_resource::<ClientArgs>().map(|a| a.0.clone()) else { return };
        let Some(file) = args.scene.clone() else { return };
        let text = std::fs::read_to_string(&file).unwrap_or_else(|e| {
            eprintln!("error: --scene {}: {e}", file.display());
            std::process::exit(2);
        });
        let steps = parse(&text).unwrap_or_else(|e| {
            eprintln!("error: --scene {}: {e}", file.display());
            std::process::exit(2);
        });
        let out = args.scene_out.clone().unwrap_or_else(|| PathBuf::from("scene_out"));
        if let Err(e) = std::fs::create_dir_all(&out) {
            eprintln!("error: --out {}: {e}", out.display());
            std::process::exit(2);
        }
        app.insert_resource(Scene {
            steps,
            next: 0,
            out,
            frame: 0,
            resume: 0,
            resume_game: None,
            waiting: None,
            waiting_cvar: None,
            armed: Vec::new(),
            due: Vec::new(),
            in_flight: Arc::new(AtomicUsize::new(0)),
            log: vec![format!("scene {}", file.display())],
            ended: None,
        })
        .init_resource::<Facts>()
        .add_observer(|_: On<Add<chimera_world::map::ragdoll::Ragdolled>>, mut facts: ResMut<Facts>| {
            facts.0 |= Fact::Ragdoll.bit();
        })
        .add_systems(Update, (measured, play).chain());
    }
}

/// A message's cursor, reading nothing where the message isn't registered.
fn read_count<M: Message>(world: &World, cursor: &mut MessageCursor<M>, pick: impl Fn(&M) -> bool) -> bool {
    world.get_resource::<Messages<M>>().is_some_and(|m| cursor.read(m).filter(|x| pick(x)).count() > 0)
}

/// The frame's facts from the engine's own (an exclusive read of the
/// messages: a scene run is a tool run).
fn facts_now(world: &mut World, cursors: &mut FactCursors) -> u16 {
    let mut bits = 0;
    // A map in and no change of map under way (a match's map change is
    // held until it is in: `minigame::MatchHold`, `MapChange`).
    let loads = world.get_resource::<MapLoads>().map(|m| m.0);
    let changing = world.contains_resource::<chimera_sim::minigame::MatchHold>()
        || world.contains_resource::<chimera_net::minigame_net::MapChange>();
    if world.contains_resource::<crate::map::LoadedMapName>() && !changing {
        bits |= Fact::MapLoaded.bit();
    }
    cursors.loads = loads;
    if chimera_core::tasks::pending() == 0 {
        bits |= Fact::Idle.bit();
    }
    if world.get_resource::<super::logo_pass::LogoPass>().is_none_or(|p| p.elapsed(0.0).is_none()) {
        bits |= Fact::Still.bit();
    }
    if world.get_resource::<chimera_sim::mode::Match>().is_some_and(|m| m.phase == chimera_sim::mode::Phase::Live) {
        bits |= Fact::Match.bit();
    }
    let alive = world.query_filtered::<(), (With<LocalPlayer>, Without<Dead>)>().iter(world).next().is_some();
    if alive {
        bits |= Fact::Spawned.bit();
    }
    if read_count(world, &mut cursors.weapon, |e| matches!(e.kind, WeaponEventKind::ShotMade { .. }) && !e.replay) {
        bits |= Fact::Shot.bit();
    }
    if read_count(world, &mut cursors.taken, |_| true) {
        bits |= Fact::Hit.bit();
    }
    let mut kill = false;
    if world.get_resource::<Messages<Died>>().is_some() {
        let m = world.resource::<Messages<Died>>();
        for d in cursors.died.read(m) {
            bits |= Fact::Death.bit();
            kill |= d.attacker.is_some_and(|a| a != d.entity);
        }
    }
    if kill {
        bits |= Fact::Kill.bit();
    }
    // Any game's blast (`BlastDealt`: grenades', rockets', map logic's
    // through the engine's blast) or an explosion with no blast rule.
    let dealt = read_count(world, &mut cursors.dealt, |_| true);
    if read_count(world, &mut cursors.blast, |_| true) || dealt {
        bits |= Fact::Blast.bit();
    }
    bits
}

#[derive(Default)]
struct FactCursors {
    weapon: MessageCursor<WeaponEvent>,
    taken: MessageCursor<DamageTaken>,
    died: MessageCursor<Died>,
    blast: MessageCursor<Explosion>,
    dealt: MessageCursor<BlastDealt>,
    loads: Option<u32>,
}

fn measured(
    world: &mut World,
    mut hits: Local<MessageCursor<HitMeasured>>,
    mut kills: Local<MessageCursor<KillMeasured>>,
    mut more: Local<(MessageCursor<BlastMeasured>, MessageCursor<PropHit>, u64)>,
) {
    let mut lines = Vec::new();
    // The console's output since the last frame (`target list`, `prop
    // group`), each line marked.
    if let Some(c) = world.get_resource::<crate::console::Console>() {
        let new = (c.printed.saturating_sub(more.2) as usize).min(c.output.len());
        lines.extend(c.output[c.output.len() - new..].iter().map(|l| format!("| {}", l.text)));
        more.2 = c.printed;
    }
    if let Some(m) = world.get_resource::<Messages<BlastMeasured>>() {
        lines.extend(more.0.read(m).map(blast_line));
    }
    if let Some(m) = world.get_resource::<Messages<PropHit>>() {
        for p in more.1.read(m) {
            lines.push(format!(
                "PropHit attacker={:?} prop={:?} piece={} distance={:.1}m t={:.3}",
                p.attacker, p.prop, p.piece, p.distance_m, p.time
            ));
        }
    }
    if let Some(m) = world.get_resource::<Messages<HitMeasured>>() {
        for h in hits.read(m) {
            lines.push(format!(
                "HitMeasured attacker={:?} victim={:?} gun={} damage={:.1} unscaled={:.1} group={:?} region={:?} distance={:.1}m line={:?} health {:.1}->{:.1} shield {:.1}->{:.1} hits={} t={:.3}",
                h.attacker,
                h.victim,
                h.gun.unwrap_or("?"),
                h.damage * 100.0,
                h.damage_unscaled * 100.0,
                h.hitgroup,
                h.region,
                h.distance_m,
                h.line_m,
                h.health_before * 100.0,
                h.health_after * 100.0,
                h.shield_before * 100.0,
                h.shield_after * 100.0,
                h.hits,
                h.time
            ));
        }
    }
    if let Some(m) = world.get_resource::<Messages<KillMeasured>>() {
        for k in kills.read(m) {
            lines.push(format!(
                "KillMeasured attacker={:?} victim={:?} gun={} shots={} hits={} headshots={} first_hit->kill={:.2}s first_shot->kill={:.2}s distance={:.1}m line={:?} damage={:.1} t={:.3}",
                k.attacker,
                k.victim,
                k.gun.unwrap_or("?"),
                k.shots_fired,
                k.shots_hit,
                k.headshots,
                k.from_first_hit,
                k.from_first_shot,
                k.distance_m,
                k.line_m,
                k.total_damage * 100.0,
                k.time
            ));
        }
    }
    if !lines.is_empty() {
        let mut scene = world.resource_mut::<Scene>();
        for l in lines {
            scene.note(l);
        }
    }
}

/// Save the window as `name` in the scene's folder.
fn shot(world: &mut World, name: &str) {
    let (path, flight) = {
        let mut scene = world.resource_mut::<Scene>();
        let path = scene.out.join(format!("{name}.png"));
        scene.note(format!("shot {}", path.display()));
        (path, scene.in_flight.clone())
    };
    flight.fetch_add(1, Ordering::Relaxed);
    world.spawn(Screenshot::primary_window()).observe(save_to_disk(path)).observe(move |_: On<ScreenshotCaptured>| {
        flight.fetch_sub(1, Ordering::Relaxed);
    });
}

/// A console variable's value now.
fn cvar_value(world: &mut World, name: &str) -> Option<String> {
    let get = world.get_resource::<crate::console::Console>()?.cvar(name)?.get.clone();
    get(world)
}

/// Whether the cvar holds `value` (numbers equal as numbers: `1` is `1.0`).
fn cvar_is(world: &mut World, name: &str, value: &str) -> bool {
    let Some(now) = cvar_value(world, name) else { return false };
    now == value || matches!((now.parse::<f64>(), value.parse::<f64>()), (Ok(a), Ok(b)) if a == b)
}

fn finish(world: &mut World, code: u8, why: Option<String>) {
    let mut scene = world.resource_mut::<Scene>();
    if let Some(why) = &why {
        scene.note(format!("FAILED: {why}"));
        eprintln!("error: scene: {why}");
    } else {
        scene.note("done");
    }
    let path = scene.out.join("scene.log");
    let text = scene.log.join("\n") + "\n";
    if let Err(e) = std::fs::write(&path, text) {
        eprintln!("error: {}: {e}", path.display());
    }
    if code != 0 {
        // The apps' mains drop the run's exit code: end here with it.
        std::process::exit(i32::from(code));
    }
    world.write_message(AppExit::Success);
}

fn play(world: &mut World, mut cursors: Local<FactCursors>) {
    let bits = facts_now(world, &mut cursors) | std::mem::take(&mut world.resource_mut::<Facts>().0);
    {
        let mut scene = world.resource_mut::<Scene>();
        scene.frame += 1;
        let frame = scene.frame;
        for f in Fact::ALL {
            // Level facts are noted when they begin only.
            if bits & f.bit() != 0 && !matches!(f, Fact::Match | Fact::Spawned | Fact::MapLoaded | Fact::Idle | Fact::Still) {
                scene.note(format!("fact {}", f.name()));
            }
        }
        let mut fired = Vec::new();
        scene.armed.retain(|(f, after, name)| {
            if bits & f.bit() != 0 {
                fired.push((frame + after, name.clone()));
                false
            } else {
                true
            }
        });
        for (at, name) in fired {
            scene.note(format!("armed picture {name} for frame {at}"));
            scene.due.push((at, name));
        }
    }
    // Pictures due now.
    let frame = world.resource::<Scene>().frame;
    let now: Vec<String> = {
        let mut scene = world.resource_mut::<Scene>();
        let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut scene.due).into_iter().partition(|(at, _)| *at <= frame);
        scene.due = later;
        now.into_iter().map(|(_, n)| n).collect()
    };
    for name in now {
        shot(world, &name);
    }
    // The script.
    loop {
        let scene = world.resource::<Scene>();
        if let Some(ended) = scene.ended {
            let left = scene.armed.len() + scene.due.len() + scene.in_flight.load(Ordering::Relaxed);
            if left == 0 {
                finish(world, 0, None);
            } else if frame > ended + DEFAULT_TIMEOUT {
                let names: Vec<String> = scene.armed.iter().map(|(f, _, n)| format!("{n} (on {})", f.name())).collect();
                finish(world, 3, Some(format!("pictures never taken at the end: {}", names.join(", "))));
            }
            return;
        }
        if let Some((fact, until, line)) = scene.waiting {
            if bits & fact.bit() != 0 {
                let mut scene = world.resource_mut::<Scene>();
                scene.waiting = None;
                scene.note(format!("line {line}: {} holds", fact.name()));
            } else if frame >= until {
                finish(world, 3, Some(format!("line {line}: wait_for {} timed out", fact.name())));
                world.resource_mut::<Scene>().ended = Some(u64::MAX / 2);
                return;
            } else {
                return;
            }
        }
        if let Some((name, value, until, line)) = world.resource::<Scene>().waiting_cvar.clone() {
            if cvar_is(world, &name, &value) {
                let mut scene = world.resource_mut::<Scene>();
                scene.waiting_cvar = None;
                scene.note(format!("line {line}: {name} is {value}"));
            } else if frame >= until {
                let now = cvar_value(world, &name).unwrap_or_else(|| "(no such cvar)".into());
                finish(world, 3, Some(format!("line {line}: wait_for cvar {name} {value} timed out (it is {now})")));
                world.resource_mut::<Scene>().ended = Some(u64::MAX / 2);
                return;
            } else {
                return;
            }
        }
        let now = world.resource::<Time<Virtual>>().elapsed_secs_f64();
        if let Some(at) = world.resource::<Scene>().resume_game {
            if now < at {
                return;
            }
            world.resource_mut::<Scene>().resume_game = None;
        }
        let scene = world.resource::<Scene>();
        if frame < scene.resume {
            return;
        }
        let Some((line, step)) = scene.steps.get(scene.next).cloned() else {
            let mut scene = world.resource_mut::<Scene>();
            scene.ended = Some(frame);
            scene.note("script ended");
            return;
        };
        world.resource_mut::<Scene>().next += 1;
        match step {
            Step::Console(text) => {
                world.resource_mut::<Scene>().note(format!("> {text}"));
                world.resource_mut::<crate::console::Console>().submit(text);
            }
            Step::Wait(n) => {
                let mut scene = world.resource_mut::<Scene>();
                scene.resume = frame + n;
                return;
            }
            Step::GameWait(s) => {
                world.resource_mut::<Scene>().resume_game = Some(now + s);
                return;
            }
            Step::WaitFor(f, timeout) => {
                let mut scene = world.resource_mut::<Scene>();
                scene.note(format!("line {line}: wait_for {} (up to {timeout} frames)", f.name()));
                scene.waiting = Some((f, frame + timeout, line));
                // Checked from the next frame (this frame's facts are
                // read already).
                return;
            }
            Step::WaitForCvar(name, value, timeout) => {
                let mut scene = world.resource_mut::<Scene>();
                scene.note(format!("line {line}: wait_for cvar {name} {value} (up to {timeout} frames)"));
                scene.waiting_cvar = Some((name, value, frame + timeout, line));
                return;
            }
            Step::Shot(name) => shot(world, &name),
            Step::Type(text) => {
                world.resource_mut::<Scene>().note(format!("type {text}"));
                for c in text.chars() {
                    let (code, key) = match c {
                        ' ' => (KeyCode::Space, bevy::input::keyboard::Key::Space),
                        _ => (KeyCode::Unidentified(bevy::input::keyboard::NativeKeyCode::Unidentified), bevy::input::keyboard::Key::Character(c.to_string().into())),
                    };
                    press(world, code, key, Some(c));
                }
            }
            Step::Key(code, times) => {
                world.resource_mut::<Scene>().note(format!("key {code:?} x{times}"));
                let key = match code {
                    KeyCode::Tab => bevy::input::keyboard::Key::Tab,
                    KeyCode::Enter => bevy::input::keyboard::Key::Enter,
                    KeyCode::Escape => bevy::input::keyboard::Key::Escape,
                    KeyCode::ArrowUp => bevy::input::keyboard::Key::ArrowUp,
                    KeyCode::ArrowDown => bevy::input::keyboard::Key::ArrowDown,
                    KeyCode::ArrowLeft => bevy::input::keyboard::Key::ArrowLeft,
                    KeyCode::ArrowRight => bevy::input::keyboard::Key::ArrowRight,
                    KeyCode::Backspace => bevy::input::keyboard::Key::Backspace,
                    KeyCode::Delete => bevy::input::keyboard::Key::Delete,
                    KeyCode::Home => bevy::input::keyboard::Key::Home,
                    KeyCode::End => bevy::input::keyboard::Key::End,
                    KeyCode::Space => bevy::input::keyboard::Key::Space,
                    _ => bevy::input::keyboard::Key::Character("`".into()),
                };
                for _ in 0..times {
                    press(world, code, key.clone(), None);
                }
                // One key a frame: each press seen as it would be typed.
                world.resource_mut::<Scene>().resume = frame + 1;
                return;
            }
            Step::ShotAfter(f, n, name) => {
                let mut scene = world.resource_mut::<Scene>();
                scene.note(format!("line {line}: shot_after {} {n} {name}", f.name()));
                scene.armed.push((f, n, name));
            }
        }
    }
}

/// A key pressed and let go, as the window would send it (`type`, `key`).
fn press(world: &mut World, key_code: KeyCode, logical_key: bevy::input::keyboard::Key, text: Option<char>) {
    use bevy::input::{ButtonState, keyboard::KeyboardInput};
    let window = world
        .query_filtered::<Entity, With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .next()
        .unwrap_or(Entity::PLACEHOLDER);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        world.write_message(KeyboardInput {
            key_code,
            logical_key: logical_key.clone(),
            state,
            text: text.map(|c| c.to_string().into()),
            repeat: false,
            window,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scene_reads_its_directives_and_console_lines() {
        let steps = parse("# a scene\nwait 30\nwait_for death 90\nshot a\nshot_after kill 15 b\nsetang 0 90 # look\n\n").unwrap();
        let only: Vec<Step> = steps.into_iter().map(|(_, s)| s).collect();
        assert_eq!(
            only,
            vec![
                Step::Wait(30),
                Step::WaitFor(Fact::Death, 90),
                Step::Shot("a".into()),
                Step::ShotAfter(Fact::Kill, 15, "b".into()),
                Step::Console("setang 0 90".into()),
            ]
        );
        assert!(parse("wait_for nothing").unwrap_err().contains("no fact"));
        assert!(parse("wait x").is_err());
        assert_eq!(
            parse("wait_for cvar sv_x 1 30\nwait_for idle").unwrap()[0].1,
            Step::WaitForCvar("sv_x".into(), "1".into(), 30)
        );
        assert_eq!(parse("wait_for idle").unwrap()[0].1, Step::WaitFor(Fact::Idle, DEFAULT_TIMEOUT));
        assert_eq!(parse("wait_for still 60").unwrap()[0].1, Step::WaitFor(Fact::Still, 60));
        assert!(parse("wait_for cvar sv_x").is_err());
        assert_eq!(parse("wait_for spawned").unwrap()[0].1, Step::WaitFor(Fact::Spawned, DEFAULT_TIMEOUT));
        assert_eq!(parse("type map de_").unwrap()[0].1, Step::Type("map de_".into()));
        assert_eq!(parse("key tab 2").unwrap()[0].1, Step::Key(KeyCode::Tab, 2));
        assert!(parse("key nope").unwrap_err().contains("no key"));
        assert_eq!(parse("gwait 0.05").unwrap()[0].1, Step::GameWait(0.05));
        assert!(parse("gwait -1").is_err());
    }
}
