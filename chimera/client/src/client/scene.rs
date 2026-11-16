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
//! - `shot <name>`: save the window as `<out>/<name>.png` now.
//! - `shot_after <fact> <frames> <name>`: arm a picture the given frames
//!   after the next frame the fact holds (the script goes on at once; the
//!   run waits for armed pictures at its end, up to 1800 frames).
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
//! `shot`, `hit`, `kill`, `death`, `ragdoll` and
//! `blast` hold on the frame they happen; `map_loaded`, `match` and
//! `spawned` while they are so.
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
}

impl Fact {
    pub const ALL: [Fact; 9] =
        [Fact::MapLoaded, Fact::Match, Fact::Spawned, Fact::Shot, Fact::Hit, Fact::Kill, Fact::Death, Fact::Ragdoll, Fact::Blast];

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
    Shot(String),
    ShotAfter(Fact, u64, String),
}

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
            "wait_for" => Step::WaitFor(
                fact(words.get(1))?,
                words.get(2).map_or(Ok(DEFAULT_TIMEOUT), |_| num(words.get(2), "wait_for's timeout"))?,
            ),
            "shot" => Step::Shot(words.get(1).ok_or_else(|| format!("line {n}: shot <name>"))?.to_string()),
            "shot_after" => Step::ShotAfter(
                fact(words.get(1))?,
                num(words.get(2), "shot_after")?,
                words.get(3).ok_or_else(|| format!("line {n}: shot_after <fact> <frames> <name>"))?.to_string(),
            ),
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
    waiting: Option<(Fact, u64, usize)>,
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
            waiting: None,
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
                "HitMeasured attacker={:?} victim={:?} gun={} damage={:.1} unscaled={:.1} group={:?} region={:?} distance={:.1}m health {:.1}->{:.1} shield {:.1}->{:.1} hits={} t={:.3}",
                h.attacker,
                h.victim,
                h.gun.unwrap_or("?"),
                h.damage * 100.0,
                h.damage_unscaled * 100.0,
                h.hitgroup,
                h.region,
                h.distance_m,
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
                "KillMeasured attacker={:?} victim={:?} gun={} shots={} hits={} headshots={} first_hit->kill={:.2}s first_shot->kill={:.2}s distance={:.1}m damage={:.1} t={:.3}",
                k.attacker,
                k.victim,
                k.gun.unwrap_or("?"),
                k.shots_fired,
                k.shots_hit,
                k.headshots,
                k.from_first_hit,
                k.from_first_shot,
                k.distance_m,
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
            if bits & f.bit() != 0 && !matches!(f, Fact::Match | Fact::Spawned | Fact::MapLoaded) {
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
            Step::WaitFor(f, timeout) => {
                let mut scene = world.resource_mut::<Scene>();
                scene.note(format!("line {line}: wait_for {} (up to {timeout} frames)", f.name()));
                scene.waiting = Some((f, frame + timeout, line));
                // Checked from the next frame (this frame's facts are
                // read already).
                return;
            }
            Step::Shot(name) => shot(world, &name),
            Step::ShotAfter(f, n, name) => {
                let mut scene = world.resource_mut::<Scene>();
                scene.note(format!("line {line}: shot_after {} {n} {name}", f.name()));
                scene.armed.push((f, n, name));
            }
        }
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
        assert_eq!(parse("wait_for spawned").unwrap()[0].1, Step::WaitFor(Fact::Spawned, DEFAULT_TIMEOUT));
    }
}
