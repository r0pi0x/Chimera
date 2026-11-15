//! Quitting quickly (docs/performance.md, "Exit"): when an app is asked to
//! exit (`AppExit`: the console's `quit`, a menu's Quit, the window's
//! close button or Alt+F4, Ctrl+C on a server), the frame that asked runs
//! the `Shutdown` schedule after `Last`, and its one system
//! (`quit_quickly`) does what must outlast the process, then ends it
//! without dropping the app:
//!
//! 1. `Stage::Hide`: what the player sees go first (the client hides its
//!    window and mutes the sound), so the quit feels instant;
//! 2. `Stage::Persist`: what must reach the disk or the network (the
//!    client's `config.cfg` is written in `Last` before this, by the
//!    console's `save_on_exit`; the feedback outbox's reports still being
//!    written, the session's log ended (`diagnostics`), the logs,
//!    downloads stopped at a safe point). A step for something new to
//!    keep goes here, by `ShutdownAppExt::shutdown_flush`;
//! 3. `Stage::Settle`: what must be idle before the process ends (the
//!    client's render thread and the pipelines still compiling, so no
//!    thread is inside the GPU driver while `exit` tears it down);
//! 4. the exit call (`ExitCall`, `std::process::exit` with `AppExit`'s
//!    code), unless the render world makes it (`ExitInRender`: an app
//!    whose rendering isn't pipelined ends after that frame's render).
//!
//! Dropping the app instead freed every asset, map, collider and
//! catalogue one by one and joined threads (measured: docs/performance.md,
//! "Exit"); nothing of it needs doing when the process ends. Each step's
//! time is logged (`exit: <step> <ms> ms`).
//!
//! The clean path (`ExitMode::clean`: the app is dropped as before, for
//! leak checks and tools whose output is written as they drop) runs the
//! same steps without the exit call: `--clean-exit` on a client, the
//! `chimera:clean_exit 1` cvar anywhere, `CHIMERA_CLEAN_EXIT=1`; a
//! client's tool runs (`--screenshot`, `--frames`, `--offscreen` ...) are
//! clean by default.
//!
//! Only apps' binaries add `ShutdownPlugin` (the client's `run_app`, the
//! dedicated server's `run`); tests add it with `ExitCall::record` in place
//! of the process's exit.
//!
//! Known shortcuts:
//! - On Wayland a window can't be hidden (winit ignores it): the window
//!   stays until the process ends, a frame or two later.

use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use bevy::{
    app::MainScheduleOrder,
    ecs::{
        message::{MessageCursor, Messages},
        schedule::ScheduleLabel,
    },
    prelude::*,
};

use crate::console::resource_cvar;

/// The schedule after `Last` in which a frame that asked to exit runs
/// `quit_quickly`.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Shutdown;

/// When a shutdown step runs, in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// Out of the player's sight: the window, the sound.
    Hide,
    /// What must persist: files written, logs flushed, uploads queued.
    Persist,
    /// What must be idle before the process ends (the render thread).
    Settle,
}

type StepFn = Box<dyn FnMut(&mut World) + Send + Sync>;

struct Step {
    stage: Stage,
    name: &'static str,
    run: StepFn,
}

/// The steps run when the app exits (`ShutdownAppExt::shutdown_flush`),
/// by stage, then in the order added.
#[derive(Resource, Default)]
pub struct ShutdownFlush {
    steps: Vec<Step>,
    /// The steps already run (a quit runs them once).
    ran: bool,
}

impl ShutdownFlush {
    /// The steps' names in the order they run.
    pub fn names(&self) -> Vec<&'static str> {
        let mut steps: Vec<_> = self.steps.iter().map(|s| (s.stage, s.name)).collect();
        steps.sort_by_key(|(stage, _)| *stage);
        steps.into_iter().map(|(_, n)| n).collect()
    }
}

/// How the process ends: `std::process::exit` with the code, or (tests)
/// whatever `record` keeps.
#[derive(Resource, Clone)]
pub struct ExitCall(pub Arc<dyn Fn(i32) + Send + Sync>);

impl Default for ExitCall {
    fn default() -> Self {
        Self(Arc::new(|code| std::process::exit(code)))
    }
}

impl ExitCall {
    /// An exit call that only records its codes (tests: the process goes on).
    pub fn record() -> (Self, Arc<Mutex<Vec<i32>>>) {
        let codes = Arc::new(Mutex::new(Vec::new()));
        let kept = codes.clone();
        (Self(Arc::new(move |code| kept.lock().unwrap().push(code))), codes)
    }

    /// End the process (or record) with `code`.
    pub fn exit(&self, code: i32) {
        (self.0)(code)
    }
}

/// Whether an exit drops the app (`clean`) or ends the process after the
/// shutdown steps. The `chimera:clean_exit` cvar.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExitMode {
    pub clean: u8,
}

impl Default for ExitMode {
    /// Quick, unless `CHIMERA_CLEAN_EXIT=1`.
    fn default() -> Self {
        Self {
            clean: u8::from(crate::env::var("CHIMERA_CLEAN_EXIT").is_ok_and(|v| v.trim() == "1")),
        }
    }
}

impl ExitMode {
    /// 1: the app is dropped by its runner.
    pub fn is_clean(&self) -> bool {
        self.clean == 1
    }

    /// 2: the app taken apart a piece at a time, each timed, then the
    /// process ended (`teardown_timed`; where a clean exit's time goes).
    pub fn is_measured(&self) -> bool {
        self.clean == 2
    }
}

/// In the main world when the render world ends the process (its
/// rendering isn't pipelined: it runs after the main schedule on the same
/// thread, and its last frame waits for the compiling pipelines first).
#[derive(Resource, Default)]
pub struct ExitInRender;

/// The exit decided by `quit_quickly`: the process ends with `code`
/// through `call` (read by the render world when it makes the call).
#[derive(Resource, Clone)]
pub struct QuittingNow {
    pub code: i32,
    pub call: ExitCall,
}

/// The process's exit code for `exit`.
pub fn exit_code(exit: &AppExit) -> i32 {
    match exit {
        AppExit::Success => 0,
        AppExit::Error(code) => i32::from(code.get()),
    }
}

pub trait ShutdownAppExt {
    /// Run `step` (named `name` in the log) at `stage` when the app exits,
    /// on the quick path and the clean one.
    fn shutdown_flush(
        &mut self,
        stage: Stage,
        name: &'static str,
        step: impl FnMut(&mut World) + Send + Sync + 'static,
    ) -> &mut Self;
}

impl ShutdownAppExt for App {
    fn shutdown_flush(
        &mut self,
        stage: Stage,
        name: &'static str,
        step: impl FnMut(&mut World) + Send + Sync + 'static,
    ) -> &mut Self {
        self.init_resource::<ShutdownFlush>();
        self.world_mut().resource_mut::<ShutdownFlush>().steps.push(Step {
            stage,
            name,
            run: Box::new(step),
        });
        self
    }
}

/// The quick exit: the `Shutdown` schedule after `Last`, `quit_quickly`
/// in it, `ExitMode` (`chimera:clean_exit`), the exit call, and the
/// logs flushed as the last `Persist` step.
pub struct ShutdownPlugin;

impl Plugin for ShutdownPlugin {
    fn build(&self, app: &mut App) {
        app.init_schedule(Shutdown);
        app.world_mut().resource_mut::<MainScheduleOrder>().insert_after(Last, Shutdown);
        app.init_resource::<ShutdownFlush>()
            .init_resource::<ExitMode>()
            .init_resource::<ExitCall>()
            .add_systems(Shutdown, quit_quickly);
        resource_cvar::<ExitMode, u8>(
            app,
            "chimera:clean_exit",
            "1: quitting drops the whole app before the process ends (slow; for leak checks); 0 (default): \
             what must be saved is, then the process ends at once; 2: the app taken apart piece by piece, \
             each piece's time printed, then the process ends (where a clean exit's time goes).",
            |m| &mut m.clean,
        );
    }

    fn finish(&self, app: &mut App) {
        // Last of the Persist steps: whatever the others printed.
        app.shutdown_flush(Stage::Persist, "logs", |_| {
            use std::io::Write;
            let _ = std::io::stdout().flush();
            let _ = std::io::stderr().flush();
        });
    }
}

/// In the frame that asked to exit: the shutdown steps, then (unless the
/// exit is clean or the render world makes it) the exit call.
pub fn quit_quickly(world: &mut World) {
    let Some(code) = asked_to_exit(world) else { return };
    // Once: the exit message is still there next frame (a clean exit's
    // runner, or a test's recorded exit call, may run one).
    if world.get_resource::<ShutdownFlush>().is_some_and(|f| f.ran) {
        return;
    }
    let start = Instant::now();
    info!("exit: asked (code {code})");
    run_steps(world);
    let clean = world.get_resource::<ExitMode>().is_some_and(ExitMode::is_clean);
    if clean {
        info!("exit: steps done in {} ms; dropping the app (clean exit)", start.elapsed().as_millis());
        return;
    }
    let call = world.get_resource::<ExitCall>().cloned().unwrap_or_default();
    info!("exit: steps done in {} ms; ending the process", start.elapsed().as_millis());
    if world.get_resource::<ExitMode>().is_some_and(ExitMode::is_measured) {
        // Nothing of the world is left to run another frame on.
        teardown_timed(world);
        call.exit(code);
        return;
    }
    {
        use std::io::Write;
        let _ = std::io::stderr().flush();
    }
    if world.contains_resource::<ExitInRender>() {
        world.insert_resource(QuittingNow { code, call });
        return;
    }
    call.exit(code);
}

/// The exit code when the app was asked to exit, as the runner reads it
/// (`App::should_exit`: an error if any, else success).
pub fn asked_to_exit(world: &World) -> Option<i32> {
    let messages = world.get_resource::<Messages<AppExit>>()?;
    let mut all = MessageCursor::<AppExit>::default();
    let exits: Vec<&AppExit> = all.read(messages).collect();
    if exits.is_empty() {
        return None;
    }
    Some(exits.iter().find(|e| e.is_error()).map_or(0, |e| exit_code(e)))
}

/// `chimera:clean_exit 2`: the main world taken apart (each resource
/// removed, then the entities, then the non-send data), each piece's time
/// on stderr, the slowest first, as a clean exit's drop would spend it.
pub fn teardown_timed(world: &mut World) {
    let start = Instant::now();
    let resources: Vec<(bevy::ecs::component::ComponentId, String)> =
        world.iter_resources().map(|(id, info, _)| (id, info.name().to_string())).collect();
    let mut times: Vec<(String, f64)> = Vec::new();
    for (id, name) in resources {
        let t = Instant::now();
        world.remove_resource_by_id(id);
        times.push((name, t.elapsed().as_secs_f64() * 1000.0));
    }
    let t = Instant::now();
    let entities = world.entities().count_spawned();
    world.clear_entities();
    times.push((format!("{entities} entities"), t.elapsed().as_secs_f64() * 1000.0));
    let t = Instant::now();
    world.clear_non_send();
    times.push(("non-send data".into(), t.elapsed().as_secs_f64() * 1000.0));
    let total: f64 = times.iter().map(|(_, ms)| ms).sum();
    times.sort_by(|a, b| b.1.total_cmp(&a.1));
    eprintln!("exit: main world taken apart in {:.1} ms; the slowest:", start.elapsed().as_secs_f64() * 1000.0);
    for (name, ms) in times.iter().take(25) {
        eprintln!("  {ms:9.2} ms  {name}");
    }
    eprintln!("  {:9.2} ms  ({} pieces in all)", total, times.len());
}

/// Run every shutdown step once, by stage, each one's time logged.
pub fn run_steps(world: &mut World) {
    let Some(mut flush) = world.get_resource_mut::<ShutdownFlush>() else { return };
    if std::mem::replace(&mut flush.ran, true) {
        return;
    }
    let mut steps = std::mem::take(&mut flush.steps);
    steps.sort_by_key(|s| s.stage);
    for step in &mut steps {
        let start = Instant::now();
        (step.run)(world);
        info!("exit: {} {} ms", step.name, start.elapsed().as_millis());
    }
    world.resource_mut::<ShutdownFlush>().steps = steps;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A quit runs the steps by stage once and ends the process with the
    /// runner's code (an error over a success).
    #[test]
    fn a_quit_runs_the_steps_then_exits_with_its_code() {
        let mut app = App::new();
        let (exit, codes) = ExitCall::record();
        let ran = Arc::new(Mutex::new(Vec::new()));
        app.add_plugins((MinimalPlugins, crate::console::ConsolePlugin, ShutdownPlugin))
            .insert_resource(exit)
            .insert_resource(ExitMode { clean: 0 });
        for (stage, name) in [(Stage::Settle, "c"), (Stage::Hide, "a"), (Stage::Persist, "b")] {
            let ran = ran.clone();
            app.shutdown_flush(stage, name, move |_| ran.lock().unwrap().push(name));
        }
        app.finish();
        assert_eq!(app.world().resource::<ShutdownFlush>().names(), ["a", "b", "logs", "c"]);
        app.update();
        assert!(codes.lock().unwrap().is_empty());
        app.world_mut().write_message(AppExit::Success);
        app.world_mut().write_message(AppExit::from_code(3));
        app.update();
        app.update();
        assert_eq!(*ran.lock().unwrap(), ["a", "b", "c"]);
        assert_eq!(*codes.lock().unwrap(), [3]);
    }
}
