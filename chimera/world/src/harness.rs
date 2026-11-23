//! Headless simulation for tests and tools: no window, no rendering, time
//! advanced by exact fixed ticks. Game-neutral: the simulation's plugins
//! come in as the `base` a `Sim` is built on (a game's set, e.g.
//! `cs_source::harness::CsSim`; the engine's alone, e.g.
//! `chimera_sim::SimBasePlugins`), so a crate's tests use the harness at
//! the layer they test (docs/OBSERVABILITY.md, section 1).
//!
//! Every `Sim` turns the "no installs" switch on (`chimera_core::mount::
//! offline`): its process finds no game install and no asset store, unless
//! the test is a heavy one.
//!
//! `Sim`: a headless app stepped by exact fixed ticks (the network's
//! server-and-clients `NetSim` is `chimera_net::harness`); `add_headless`
//! / `add_headless_with`: the engine without a window, as `Sim` and the
//! dedicated server run it; `Sim::pad` (and `CHIMERA_TEST_PAD=<n>` for every
//! `Sim`) spawns entities and a resource first, so a test can check its
//! result doesn't depend on entity ids; `emulate_mesh_extraction` stands in
//! for the render world's mesh extraction (takes a `RENDER_WORLD`-only
//! mesh's data, `MeshExtraction` keeps the copies and the meshes announced
//! changed after their data was taken: Bevy's "already been extracted"
//! error); `system_order` lists every schedule's systems;
//! `single_threaded_schedules` puts every schedule of the main world on
//! one thread (`set_executors`; the game, the dedicated server, `perfgate`, and every `Sim`:
//! tests run the executors that ship; `Sim::multi_threaded` or
//! `CHIMERA_EXECUTOR=multi` puts them back on Bevy's multi-threaded one);
//! `Sim`'s task pools are capped (`SIM_TASK_THREADS`) so parallel test
//! processes don't each start a thread per core, with one compute thread
//! so parallel queries run inline and a tick's work repeats; every `Sim`'s
//! systems are wrapped for counting (`metrics::work`; `Sim::work` measures
//! ticks by it, `Sim::state_hash` checks two runs played alike); `schedule_systems` and
//! `ambiguities` list a schedule's systems in run order and its
//! unordered conflicting pairs (the games' tests/it/schedules.rs: the
//! ambiguity ratchet and the server/client order comparison).

use std::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};
use chimera_core::{
    character::character_bundle,
    core::{DEFAULT_TICK_HZ, Intent, MovementState, SimTick, Team, Velocity},
    slots::set_movement,
};

pub struct Sim {
    pub app: App,
}

/// The physics steps `Sim::settle_frames` took (the match's clock stands
/// meanwhile; each step's time goes on from the last).
#[derive(Resource, Default)]
struct SettleSteps(u32);

/// Between the frames of a test that drives an `App` by hand while a
/// background task runs (a map's load, a model's build; counted in
/// `chimera_core::tasks::pending`): gives the pools the CPU while work is
/// pending, and little when none is (the 1 ms sleep this replaces
/// was paid every frame, and nothing was waited on). Prefer `Sim::run_until_idle` where the wait is
/// for the work to finish; this is for loops that look at each frame.
pub fn let_tasks_run() {
    // Work that isn't counted (a loader that doesn't use
    // `chimera_core::tasks::spawn`) still gets a moment.
    let pending = chimera_core::tasks::pending_here() > 0;
    std::thread::sleep(Duration::from_micros(if pending { 200 } else { 50 }));
}

/// Between the rounds of a test that waits on real sockets or threads of
/// its own (UDP loopback, a test HTTP server): the OS needs a moment to
/// deliver. Use `let_tasks_run` for the engine's own background work, and
/// a `NetSim` (in-memory link) where the test doesn't need real sockets.
pub fn let_io_run() {
    std::thread::sleep(Duration::from_micros(500));
}

/// Run `round` (a frame of each app, then a look at the state) until it
/// says done, at most `max_rounds`, letting real I/O run between rounds
/// (`let_io_run`). Panics naming `what` after the limit: a count, not a
/// clock, so a loaded machine doesn't flake it. Returns the rounds used.
#[track_caller]
pub fn pump_until(what: &str, max_rounds: u64, mut round: impl FnMut() -> bool) -> u64 {
    for n in 0..max_rounds {
        if round() {
            return n + 1;
        }
        let_io_run();
    }
    panic!("gave up waiting for {what} after {max_rounds} rounds");
}

/// A wait that ran out: `Sim::run_until`, `run_until_idle` and
/// `await_message` give up after `ticks` ticks (their limit, not a
/// clock: a slow machine doesn't change it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timeout {
    /// What was awaited ("" if the caller named nothing).
    pub what: String,
    /// Ticks run while waiting.
    pub ticks: u64,
    /// The sim's tick when it gave up.
    pub at_tick: u64,
    /// Background work still on its way then (`Sim::pending`).
    pub pending: usize,
}

impl std::fmt::Display for Timeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let what = if self.what.is_empty() { "the condition" } else { &self.what };
        write!(
            f,
            "gave up waiting for {what} after {} ticks (sim tick {}, {} background tasks still pending)",
            self.ticks, self.at_tick, self.pending
        )
    }
}

impl std::error::Error for Timeout {}

/// Ticks a finished background task's result may need to be taken up
/// (a polling system runs once a tick; a result may start a next task).
const IDLE_SETTLE_TICKS: u32 = 3;

/// The longest `run_until_idle` waits for a background task on the wall
/// clock before giving up (a task that never ends; the tick limit
/// doesn't count while the pool works).
const IDLE_WALL_LIMIT: Duration = Duration::from_secs(180);

/// Threads of the task pools every `Sim` (`add_headless`) process starts:
/// one I/O, one async compute, one compute. nextest runs tests as parallel
/// processes, each would otherwise start a full set. One compute thread
/// makes Bevy's parallel queries plain loops on the thread running the
/// schedule and leaves no workers spinning while they wait for each other
/// (Bevy's transform propagation), so a tick's instructions repeat from
/// run to run (`metrics::work`; docs/performance.md, "Work counts"); while
/// a `Sim` counts (`Sim::work`) that thread is held busy
/// (`work::hold_compute_pool`) so scopes' tasks run on the schedules'
/// thread. The pools are process-wide;
/// the first app built in a process sets them.
pub const SIM_TASK_THREADS: usize = 3;

/// `Sim::work`'s result: each tick's instructions (this thread and the
/// compute pool's) and every system's over the window.
#[derive(Clone, Debug)]
pub struct Work {
    pub ticks: Vec<u64>,
    /// The world's archetypes after each tick: a tick that made one paid
    /// Bevy's one-off cost of matching it to every query
    /// (`steady_ticks` leaves those out).
    pub archetypes: Vec<usize>,
    pub systems: chimera_core::metrics::work::WorkCounts,
    /// The systems' instructions and runs by the crate each comes from
    /// (`WorkCounts::take_with_crates`).
    pub crates: std::collections::BTreeMap<String, (u64, u64)>,
}

impl Work {
    /// Every tick's instructions together.
    pub fn total(&self) -> u64 {
        self.ticks.iter().sum()
    }

    /// The median tick.
    pub fn median(&self) -> u64 {
        let mut t = self.ticks.clone();
        t.sort_unstable();
        t.get(t.len() / 2).copied().unwrap_or(0)
    }

    /// Instructions per tick (the mean).
    pub fn per_tick(&self) -> f64 {
        self.total() as f64 / self.ticks.len().max(1) as f64
    }

    /// The ticks that made no new archetype, nor followed one that did:
    /// the steady cost, without Bevy's one-off matching of a new
    /// archetype to every query.
    pub fn steady_ticks(&self) -> Vec<u64> {
        (0..self.ticks.len())
            .filter(|&i| i >= 2 && self.archetypes[i] == self.archetypes[i - 1] && self.archetypes[i - 1] == self.archetypes[i - 2])
            .map(|i| self.ticks[i])
            .collect()
    }
}

/// For a test that compares work counts closely (`Sim::work`): the
/// compute pool is the process's, and tests beside it in one process
/// (`cargo test`) put their tasks in it. True where the test has the
/// process to itself (nextest gives every test one, or this is the child
/// started here); otherwise runs `test` (its path in the test binary,
/// e.g. `session::a_match_repeats_tick_for_tick`) again alone in a child
/// process, asserts it passed and returns false: the caller returns.
pub fn own_process(test: &str) -> bool {
    const CHILD: &str = "CHIMERA_OWN_PROCESS";
    if std::env::var_os("NEXTEST").is_some() || std::env::var_os(CHILD).is_some() {
        return true;
    }
    let status = std::process::Command::new(std::env::current_exe().expect("the test binary"))
        .args(["--exact", test, "--nocapture", "--test-threads", "1"])
        .env(CHILD, "1")
        .status()
        .expect("the test binary runs again");
    assert!(status.success(), "{test} in its own process failed (its output is above)");
    false
}

/// The task pools `add_headless` makes (`SIM_TASK_THREADS`).
pub fn sim_task_pools() -> bevy::app::TaskPoolOptions {
    let mut o = bevy::app::TaskPoolOptions::with_num_threads(SIM_TASK_THREADS);
    let one = |spawn: Option<std::sync::Arc<dyn Fn() + Send + Sync>>| bevy::app::TaskPoolThreadAssignmentPolicy {
        min_threads: 1,
        max_threads: 1,
        percent: 1.0,
        on_thread_spawn: spawn,
        on_thread_destroy: None,
    };
    o.io = one(None);
    o.async_compute = one(None);
    o.compute = one(None);
    o
}

/// The engine with no window or rendering: what `Sim` runs (stepped by
/// hand) under its base plugins, its task pools capped
/// (`SIM_TASK_THREADS`).
pub fn add_headless(app: &mut App) {
    add_headless_with(
        app,
        MinimalPlugins.build().set(bevy::app::TaskPoolPlugin {
            task_pool_options: sim_task_pools(),
        }),
    );
}

/// The same with `minimal` (`MinimalPlugins`, e.g. with a run loop that
/// waits between frames: the dedicated server).
pub fn add_headless_with(app: &mut App, minimal: bevy::app::PluginGroupBuilder) {
    app.add_plugins((
        minimal,
        TransformPlugin,
        // avian watches mesh assets even when no collider uses them.
        AssetPlugin::default(),
        bevy::mesh::MeshPlugin,
        // Avian as every app runs it (no interpolation: headless, it
        // would make `Transform` show the previous tick).
        crate::physics_plugins(),
    ));
}

/// The main world's schedules run on Bevy's multi-threaded executor
/// (`Sim::multi_threaded`, `CHIMERA_EXECUTOR=multi`): `Sim::work` then
/// leaves the compute pool to them.
#[derive(Resource)]
struct MultiThreaded;

/// How many schedules the world had when `Sim::work` last put them all on
/// one thread.
#[derive(Resource)]
struct KnownSchedules(usize);

/// `Sim::pad`'s resource.
#[derive(Resource)]
struct Padding(#[allow(dead_code)] usize);

/// `CHIMERA_TEST_PAD`, or 0.
fn pad_from_env() -> usize {
    chimera_core::env::var("CHIMERA_TEST_PAD").ok().and_then(|v| v.parse().ok()).unwrap_or(0)
}

impl Sim {
    /// A headless app with the simulation `base` (a game's set of plugins,
    /// or the engine's) plus `plugins` (a map, and any game plugins whose
    /// implementations the test uses).
    pub fn new<B, M>(base: impl bevy::app::Plugins<B>, plugins: impl bevy::app::Plugins<M>) -> Self {
        Self::with(base, |app| {
            app.add_plugins(plugins);
        })
    }

    /// Like `new`, with what `setup` adds after `base` (a `NetSim` builds
    /// several apps from one setup).
    pub fn with<B>(base: impl bevy::app::Plugins<B>, setup: impl FnOnce(&mut App)) -> Self {
        // No install or asset store behind a `Sim` (the machine's files
        // would change what a test sees): `mount::offline`.
        chimera_core::mount::offline::forbid_installs();
        let mut app = App::new();
        app.insert_resource(chimera_core::tasks::TestRun);
        add_headless(&mut app);
        app.add_plugins(base);
        setup(&mut app);
        Self::start(app)
    }

    fn start(mut app: App) -> Self {
        // The executors the game and the dedicated server run
        // (`CHIMERA_EXECUTOR=multi`: Bevy's, as there).
        if chimera_core::env::var("CHIMERA_EXECUTOR").as_deref() != Ok("multi") {
            single_threaded_schedules(&mut app);
            let count = app.world().get_resource::<bevy::ecs::schedule::Schedules>().map_or(0, |s| s.iter().count());
            app.insert_resource(KnownSchedules(count));
        } else {
            app.insert_resource(MultiThreaded);
        }
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / DEFAULT_TICK_HZ,
        )));
        app.finish();
        app.cleanup();
        // Avian optimizes its collider trees on a compute task alongside
        // the narrow phase and solver: done on the pool's thread while this
        // one runs other systems, its work would land in whichever system
        // was running. In the tick's own order instead (the same trees).
        if let Some(mut o) = app.world_mut().get_resource_mut::<avian3d::collider_tree::ColliderTreeOptimization>() {
            o.use_compute_task = false;
        }
        // `CHIMERA_REPEATABLE=1`: every `Sim` waits for its tasks (`repeatable`).
        if chimera_core::env::var("CHIMERA_REPEATABLE").as_deref() == Ok("1") {
            app.insert_resource(chimera_core::tasks::WaitForTasks);
        }
        // Every system behind a counter (idle until `Sim::work` or
        // `metrics::work::set_enabled`): before the first run, while the
        // schedules' systems are still in their graphs.
        chimera_core::metrics::work::count_systems(app.world_mut());
        // Startup, and one tick so the physics spatial index contains the map.
        app.update();
        let mut sim = Self { app };
        sim.pad(pad_from_env());
        sim.ticks(1);
        sim
    }

    /// Run the main world's schedules (`single_threaded_schedules`' set)
    /// on Bevy's multi-threaded executor from now on: every `Sim` starts
    /// on the single-threaded one the game and the dedicated server run;
    /// a test of the other opts out here (`CHIMERA_EXECUTOR=multi` does it
    /// for every `Sim`).
    pub fn multi_threaded(&mut self) {
        set_executors(self.app.world_mut(), || bevy::ecs::schedule::MultiThreadedExecutor::new());
        self.app.insert_resource(MultiThreaded);
    }

    /// Spawn `n` empty entities and a resource: every entity spawned
    /// after gets another id (since Bevy 0.19 resources are entities too).
    /// Nothing may depend on entity ids; `CHIMERA_TEST_PAD=<n>` pads
    /// every `Sim` this way, to check a test's result doesn't change.
    pub fn pad(&mut self, n: usize) {
        if n == 0 {
            return;
        }
        let world = self.app.world_mut();
        for _ in 0..n {
            world.spawn_empty();
        }
        world.insert_resource(Padding(n));
    }

    /// Run fixed ticks of `secs` from now on (e.g. a game's own tick).
    pub fn set_tick_interval(&mut self, secs: f64) {
        let step = Duration::from_secs_f64(secs);
        self.app.insert_resource(Time::<Fixed>::from_duration(step));
        self.app.insert_resource(TimeUpdateStrategy::ManualDuration(step));
    }

    pub fn tick_hz(&self) -> f64 {
        self.app
            .world()
            .resource::<Time<Fixed>>()
            .timestep()
            .as_secs_f64()
            .recip()
    }

    /// Advance exactly `n` fixed ticks.
    pub fn ticks(&mut self, n: u64) {
        let target = self.tick() + n;
        let mut frames = 0u64;
        while self.tick() < target {
            assert!(
                !self.app.world().contains_resource::<crate::map::MapSettling>(),
                "Sim::ticks while a map settles (`map::MapSettling`): no tick runs then; use `settle_frames`"
            );
            self.app.update();
            frames += 1;
            assert!(frames < 1_000_000, "Sim::ticks: no tick in {frames} frames");
        }
    }

    /// `n` frames of a map settling behind a loading screen
    /// (`map::MapSettling`: no tick runs), each with its one physics step
    /// (`map::settle_physics_step`), as the client's settling has them.
    pub fn settle_frames(&mut self, n: u32) {
        for _ in 0..n {
            self.app.update();
            let w = self.app.world_mut();
            let step = w.get_resource::<SettleSteps>().map_or(0, |s| s.0);
            w.insert_resource(SettleSteps(step + 1));
            crate::map::settle_physics_step(w, step);
        }
    }

    /// Play out the same on every run from now on, whatever the machine's
    /// load: tasks' results (a navigation mesh, a body's model) land on the
    /// tick they are first polled and a map spawns in one go
    /// (`chimera_core::tasks::WaitForTasks`). For measuring (`work`) and
    /// determinism checks (`state_hash`); not for tests of what happens
    /// while a task runs. `CHIMERA_REPEATABLE=1` does it for every `Sim`
    /// from its start.
    pub fn repeatable(&mut self) -> &mut Self {
        self.app.insert_resource(chimera_core::tasks::WaitForTasks);
        self
    }

    /// Run `n` ticks counting their work (`metrics::work`): each tick's
    /// instructions in its systems, on this thread, and every
    /// system's over the `n`. None without hardware counters (Windows,
    /// `perf_event_paranoid` 3): callers skip their checks. The measure to
    /// judge a tick's cost by (docs/performance.md, "Work counts").
    pub fn work(&mut self, n: u64) -> Option<Work> {
        use chimera_core::metrics::work;
        // Holding the compute pool (scopes' tasks on this thread) would
        // stop a multi-threaded executor's systems, which run on it.
        let multi = self.app.world().contains_resource::<MultiThreaded>();
        if multi {
            work::set_enabled_sharing_pool(true);
        } else {
            // Schedules made since the start (a plugin's, a test's) are on
            // Bevy's default executor, whose systems would wait for the
            // held pool: every one on this thread, as the `Sim` runs them.
            let w = self.app.world_mut();
            let count = w.get_resource::<bevy::ecs::schedule::Schedules>().map_or(0, |s| s.iter().count());
            if w.get_resource::<KnownSchedules>().is_none_or(|k| k.0 != count) {
                set_executors(w, bevy::ecs::schedule::SingleThreadedExecutor::new);
                w.insert_resource(KnownSchedules(count));
            }
            work::set_enabled(true);
        }
        let _ = work::WorkCounts::take();
        let mut ticks = Vec::with_capacity(n as usize);
        let mut archetypes = Vec::with_capacity(n as usize);
        let mut last = work::now();
        for _ in 0..n {
            self.ticks(1);
            let now = work::now();
            if let (Some(a), Some(b)) = (last, now) {
                ticks.push(b.saturating_sub(a));
            }
            archetypes.push(self.app.world().archetypes().len());
            last = work::now();
        }
        let (systems, crates) = work::WorkCounts::take_with_crates();
        work::set_enabled(false);
        if work::multiplexed() {
            eprintln!(
                "work counts: the kernel multiplexed the instruction counter (more hardware events open on this \
                 thread than the PMU has counters): the counts are estimates and won't repeat"
            );
        }
        (ticks.len() as u64 == n).then_some(Work { ticks, archetypes, systems, crates })
    }

    /// A hash of the simulation's state: every entity's place, turn,
    /// velocity and health, by entity, and the tick. Two runs of a
    /// scenario that played alike hash alike (a check that what a test
    /// measures repeats: docs/performance.md, "Work counts").
    pub fn state_hash(&mut self) -> u64 {
        use std::hash::{Hash, Hasher};
        let rows = self.state_rows();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.tick().hash(&mut h);
        for (e, _, bits) in &rows {
            e.to_bits().hash(&mut h);
            bits.hash(&mut h);
        }
        h.finish()
    }

    /// What `state_hash` hashes, by entity (sorted): the entity, its
    /// `Name` and the bits of its place, turn, velocity and health. Two
    /// runs that hash apart differ in some row (`perfgate --repeat`
    /// prints the first ones).
    pub fn state_rows(&mut self) -> Vec<(Entity, Option<String>, Vec<u32>)> {
        let world = self.app.world_mut();
        let mut rows: Vec<_> = world
            .query::<(Entity, Option<&Name>, Option<&Transform>, Option<&Velocity>, Option<&chimera_core::core::Health>)>()
            .iter(world)
            .filter(|(_, _, t, v, h)| t.is_some() || v.is_some() || h.is_some())
            .map(|(e, name, t, v, h)| {
                let mut bits = Vec::new();
                if let Some(t) = t {
                    bits.extend(t.translation.to_array().map(f32::to_bits));
                    bits.extend(t.rotation.to_array().map(f32::to_bits));
                }
                if let Some(v) = v {
                    bits.extend(v.0.to_array().map(f32::to_bits));
                }
                if let Some(h) = h {
                    bits.push(h.current.to_bits());
                }
                (e, name.map(|n| n.as_str().to_string()), bits)
            })
            .collect();
        rows.sort_unstable_by_key(|r| r.0);
        rows
    }

    /// Advance by whole ticks covering `d`. Game time (`Time<Virtual>`,
    /// `Time<Fixed>`) and `Time<Real>` both move by it, at once and exactly
    /// (the app's `TimeUpdateStrategy::ManualDuration`: nothing in a `Sim`
    /// reads the wall clock for `Time`): a test jumps "5 s later" with
    /// `sim.advance(Duration::from_secs(5))`.
    pub fn advance(&mut self, d: Duration) {
        self.seconds(d.as_secs_f64());
    }

    /// Tick until `done` holds (checked before the first tick and after
    /// each), at most `max_ticks`. Ok: the ticks it took; Err: `Timeout`.
    /// Use instead of a fixed number of ticks wherever the test waits for
    /// something to happen. `expect_until` names the wait and panics.
    pub fn run_until(&mut self, mut done: impl FnMut(&mut World) -> bool, max_ticks: u64) -> Result<u64, Timeout> {
        self.run_until_named("", &mut done, max_ticks)
    }

    fn run_until_named(&mut self, what: &str, done: &mut dyn FnMut(&mut World) -> bool, max_ticks: u64) -> Result<u64, Timeout> {
        let mut ran = 0;
        loop {
            if done(self.app.world_mut()) {
                return Ok(ran);
            }
            if ran >= max_ticks {
                return Err(Timeout {
                    what: what.to_string(),
                    ticks: ran,
                    at_tick: self.tick(),
                    pending: self.pending(),
                });
            }
            if self.pending() > 0 {
                let_tasks_run();
            }
            self.ticks(1);
            ran += 1;
        }
    }

    /// `run_until`, panicking with `what` ("the map to load") and the
    /// state when it runs out; returns the ticks it took.
    #[track_caller]
    pub fn expect_until(&mut self, what: &str, mut done: impl FnMut(&mut World) -> bool, max_ticks: u64) -> u64 {
        match self.run_until_named(what, &mut done, max_ticks) {
            Ok(n) => n,
            Err(t) => panic!("{t}"),
        }
    }

    /// Tick until the first message `M` written from now on, and return it
    /// (a clone; the tick that wrote it has run). Err after `max_ticks`
    /// without one.
    pub fn await_message<M: Message + Clone>(&mut self, max_ticks: u64) -> Result<M, Timeout> {
        let mut cursor = self.app.world().resource::<Messages<M>>().get_cursor_current();
        let mut found = None;
        let what = format!("a {} message", std::any::type_name::<M>());
        self.run_until_named(
            &what,
            &mut |w| {
                let messages = w.resource::<Messages<M>>();
                found = cursor.read(messages).next().cloned();
                found.is_some()
            },
            max_ticks,
        )?;
        Ok(found.expect("the wait ended on a message"))
    }

    /// Background work this `Sim` waits for: what this thread started
    /// (`chimera_core::tasks::pending_here`; its systems run here), or on
    /// Bevy's multi-threaded executor, whose systems start work on the
    /// pool's threads, the process's (`pending`).
    pub fn pending(&self) -> usize {
        if self.app.world().contains_resource::<MultiThreaded>() {
            chimera_core::tasks::pending()
        } else {
            chimera_core::tasks::pending_here()
        }
    }

    /// Run background work to completion: tick until no task or job
    /// counted in `Sim::pending` is left and the systems
    /// that take up results have had `IDLE_SETTLE_TICKS` ticks (a result
    /// may start the next task: a map's load, then its models), at most
    /// `max_ticks`. While work is pending it waits a little between ticks
    /// so the pools run, and gives up after a wall limit of 3 minutes
    /// too. Ok: the ticks it took. Replaces sleeping for loads, model
    /// builds, downloads and navigation meshes. Work not started through
    /// `chimera_core::tasks::spawn` / `thread` is invisible to it. The
    /// count is this thread's work (`Sim::pending`): other tests in the
    /// process (`cargo test`'s threads) don't hold it up.
    pub fn run_until_idle(&mut self, max_ticks: u64) -> Result<u64, Timeout> {
        let began = std::time::Instant::now();
        let mut ran = 0;
        let mut quiet = 0;
        loop {
            let pending = self.pending();
            if pending == 0 {
                quiet += 1;
                if quiet > IDLE_SETTLE_TICKS {
                    return Ok(ran);
                }
            } else {
                quiet = 0;
                // The pools work on their own threads; give them the CPU.
                std::thread::sleep(Duration::from_micros(300));
            }
            if ran >= max_ticks || began.elapsed() > IDLE_WALL_LIMIT {
                return Err(Timeout {
                    what: "background work to finish".to_string(),
                    ticks: ran,
                    at_tick: self.tick(),
                    pending,
                });
            }
            self.ticks(1);
            ran += 1;
        }
    }

    /// Advance by whole ticks covering `secs` seconds.
    pub fn seconds(&mut self, secs: f64) {
        self.ticks((secs * self.tick_hz()).round() as u64);
    }

    pub fn tick(&self) -> u64 {
        self.app.world().resource::<SimTick>().0
    }

    pub fn spawn_character(&mut self, position: Vec3, movement: &'static str) -> Entity {
        let world = self.app.world_mut();
        let entity = world
            .spawn(character_bundle(Transform::from_translation(position), Team(0)))
            .id();
        set_movement(entity, movement).apply(world);
        entity
    }

    pub fn set_movement(&mut self, entity: Entity, movement: &'static str) {
        set_movement(entity, movement).apply(self.app.world_mut());
    }

    pub fn intent(&mut self, entity: Entity) -> Mut<'_, Intent> {
        self.app
            .world_mut()
            .get_mut::<Intent>(entity)
            .expect("entity has no Intent")
    }

    pub fn position(&self, entity: Entity) -> Vec3 {
        self.app
            .world()
            .get::<Transform>(entity)
            .expect("no Transform")
            .translation
    }

    pub fn velocity(&self, entity: Entity) -> Vec3 {
        self.app.world().get::<Velocity>(entity).expect("no Velocity").0
    }

    pub fn state(&self, entity: Entity) -> MovementState {
        self.app
            .world()
            .get::<MovementState>(entity)
            .expect("no MovementState")
            .clone()
    }
}

/// What `emulate_mesh_extraction` saw: each mesh as the render world
/// would hold it, and the meshes it couldn't extract.
#[derive(Resource, Default)]
pub struct MeshExtraction {
    /// The last data extracted per mesh (the render world's copy).
    pub extracted: std::collections::HashMap<AssetId<Mesh>, Mesh>,
    /// Meshes announced as added or modified whose data was already taken:
    /// in a real run, Bevy's "RenderMesh with RenderAssetUsages ==
    /// RENDER_WORLD cannot be extracted: The asset has already been
    /// extracted" error.
    pub failures: Vec<AssetId<Mesh>>,
}

/// Headless stand-in for the render world's mesh extraction, so a test can
/// catch a mesh announced as changed after the render world took its
/// data: each frame (in `Last`, after the asset events go out in
/// `PostUpdate`) a mesh added or modified this frame is extracted as Bevy
/// does: a `RENDER_WORLD`-only one has its data taken from the main world,
/// others are copied. Readers of such meshes look in `MeshExtraction`.
pub fn emulate_mesh_extraction(app: &mut App) {
    app.init_resource::<MeshExtraction>().add_systems(Last, extract_meshes);
}

fn extract_meshes(
    mut events: MessageReader<AssetEvent<Mesh>>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    mut out: ResMut<MeshExtraction>,
) {
    let Some(mut meshes) = meshes else {
        events.clear();
        return;
    };
    let mut due: Vec<AssetId<Mesh>> = Vec::new();
    for e in events.read() {
        match e {
            AssetEvent::Added { id } | AssetEvent::Modified { id } if !due.contains(id) => due.push(*id),
            _ => {}
        }
    }
    use bevy::asset::RenderAssetUsages;
    for id in due {
        let Some(mesh) = meshes.get_mut_untracked(id) else { continue };
        if !mesh.asset_usage.contains(RenderAssetUsages::RENDER_WORLD) {
            continue;
        }
        if mesh.asset_usage == RenderAssetUsages::RENDER_WORLD {
            match mesh.take_gpu_data() {
                Ok(data) => {
                    out.extracted.insert(id, data);
                }
                Err(_) => out.failures.push(id),
            }
        } else {
            let copy = mesh.clone();
            out.extracted.insert(id, copy);
        }
    }
}

/// Every schedule's systems in the order they run (built now if they
/// haven't run yet), one schedule after another by name: the spawn dump
/// (`games/cs_source/tests/it/heavy/map_spawn_dump.rs`) and the game's
/// `CHIMERA_SYSTEM_DUMP` (chimera_client's `client::app`), for refactors that
/// move systems between plugins or crates.
pub fn system_order(world: &mut World) -> String {
    use std::fmt::Write as _;
    let mut labels: Vec<(String, bevy::ecs::schedule::InternedScheduleLabel)> = world
        .resource::<Schedules>()
        .iter()
        .map(|(label, s)| (format!("{label:?}"), s.label()))
        .collect();
    labels.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = String::new();
    for (name, label) in labels {
        let _ = writeln!(out, "{name}:");
        world.schedule_scope(label, |world, schedule| {
            schedule.initialize(world).expect("schedule builds");
            for (_, system) in schedule.systems().expect("initialized") {
                let _ = writeln!(out, "  {}", system.name());
            }
        });
    }
    out
}

/// Run every schedule of the main world on one thread instead of Bevy's
/// multi-threaded executor: the per-frame and fixed schedules, and the
/// ones they run (prediction's `core::Predict` stages every tick and each
/// replayed command, `StateTransition`, `SpawnScene`, egui's passes); the
/// games (whose shell does the render world's too), the dedicated server
/// and `perfgate`.
///
/// A tick runs a few hundred systems, nearly all tiny: handing each to a
/// worker thread and waking the next cost more than the systems did, and
/// much more on a busy machine (docs/performance.md, "Frame time pass";
/// a 32-player server spent half its CPU time on it: multiplayer.md,
/// "Capacity"). Systems that iterate many entities still spread over the
/// task pool themselves. Schedules a plugin made without setting an
/// executor get Bevy's multi-threaded one: the prediction stages ran on
/// it until "Party frame-rate pass", third pass (docs/performance.md),
/// their unordered systems in whatever order the workers took them.
/// Every schedule is set, so one added later is too; call this once the
/// plugins are in (schedules made after that keep their own).
pub fn single_threaded_schedules(app: &mut App) {
    set_executors(app.world_mut(), bevy::ecs::schedule::SingleThreadedExecutor::new);
}

/// Every schedule in `world` on a new executor from `make`. (A render
/// world's extract schedule also needs `set_apply_final_deferred(false)`
/// again: chimera_client's `single_threaded_schedules`.)
pub fn set_executors<E: bevy::ecs::schedule::SystemExecutor + 'static>(world: &mut World, make: impl Fn() -> E) {
    let Some(mut schedules) = world.get_resource_mut::<bevy::ecs::schedule::Schedules>() else { return };
    for (_, s) in schedules.iter_mut() {
        s.set_executor(make());
    }
}

/// `label`'s systems by name in the order the single-threaded executor
/// runs them (the schedule built now if it hasn't run); empty if the app
/// has no such schedule.
pub fn schedule_systems(world: &mut World, label: impl bevy::ecs::schedule::ScheduleLabel) -> Vec<String> {
    let mut out = Vec::new();
    let _ = world.try_schedule_scope(label, |world, schedule| {
        schedule.initialize(world).expect("schedule builds");
        for (_, system) in schedule.systems().expect("initialized") {
            out.push(system.name().to_string());
        }
    });
    out
}

/// The schedules the simulation's order matters in, by name: the fixed
/// tick's and prediction's (`Predict`, which a client replays): the
/// ambiguity ratchet's and the server/client order comparison's.
pub fn simulation_schedules() -> [(&'static str, bevy::ecs::schedule::InternedScheduleLabel); 8] {
    use bevy::ecs::schedule::ScheduleLabel;
    use chimera_core::core::Predict;
    [
        ("FixedUpdate", FixedUpdate.intern()),
        ("FixedPostUpdate", FixedPostUpdate.intern()),
        ("FixedLast", FixedLast.intern()),
        ("Predict::Select", Predict::Select.intern()),
        ("Predict::PlaceMovers", Predict::PlaceMovers.intern()),
        ("Predict::Movement", Predict::Movement.intern()),
        ("Predict::CarryRiders", Predict::CarryRiders.intern()),
        ("Predict::Weapons", Predict::Weapons.intern()),
    ]
}

/// Of `pairs` (systems with conflicting access and no order between
/// them: `ambiguities`), those `a` and `b` (two compositions'
/// `schedule_systems`, e.g. a server's and its client's) run in opposite
/// orders: where the two would compute something differently. Pairs
/// either composition lacks are left out; a name that occurs twice
/// counts at its first place. Systems that don't conflict may run in any
/// order without changing a result, and ordered ones keep their order.
pub fn ordered_differently(a: &[String], b: &[String], pairs: &[(String, String)]) -> Vec<(String, String)> {
    let place = |list: &[String]| {
        let mut at = std::collections::BTreeMap::new();
        for (i, name) in list.iter().enumerate() {
            at.entry(name.clone()).or_insert(i);
        }
        at
    };
    let (pa, pb) = (place(a), place(b));
    let mut out: Vec<(String, String)> = pairs
        .iter()
        .filter(|(x, y)| match (pa.get(x), pa.get(y), pb.get(x), pb.get(y)) {
            (Some(ax), Some(ay), Some(bx), Some(by)) => (ax < ay) != (bx < by),
            _ => false,
        })
        .cloned()
        .collect();
    out.sort();
    out.dedup();
    out
}

/// One `simulation_schedules` entry of a game's schedule ratchet: the
/// counts its tests/it/schedules.rs stores, which may fall (lower them
/// there), never rise.
pub struct ScheduleRatchet {
    pub schedule: &'static str,
    /// `ambiguities` in the dedicated server's composition.
    pub server: usize,
    /// `ambiguities` in the game's (client's) composition.
    pub client: usize,
    /// Of those pairs, the ones the two run in opposite orders
    /// (`ordered_differently`): what a client would compute otherwise
    /// than its server.
    pub opposite: usize,
}

/// The ambiguity ratchet and the server/client order comparison
/// (docs/plans/active/architecture-review.md, E1 2-3) for a game's two
/// compositions (apps built as its dedicated server and its game add
/// their plugins, finished): each count against `stored`. Err lists
/// what rose, with the pairs to order; counts that fell are printed.
pub fn check_schedule_ratchet(server: &mut App, client: &mut App, stored: &[ScheduleRatchet]) -> Result<(), String> {
    let mut report = String::new();
    for (name, label) in simulation_schedules() {
        let stored = stored
            .iter()
            .find(|r| r.schedule == name)
            .unwrap_or_else(|| panic!("{name} not in the stored ratchet"));
        let s_order = schedule_systems(server.world_mut(), label);
        let c_order = schedule_systems(client.world_mut(), label);
        assert!(!s_order.is_empty() || name.starts_with("Predict"), "{name}: no systems on the server");
        let s = ambiguities(server.world_mut(), label);
        let c = ambiguities(client.world_mut(), label);
        let both: Vec<_> = s.iter().chain(&c).cloned().collect();
        let opposite = ordered_differently(&s_order, &c_order, &both);
        eprintln!(
            "{name}: ambiguous pairs server {} client {}, in opposite orders {} (stored {} {} {})",
            s.len(),
            c.len(),
            opposite.len(),
            stored.server,
            stored.client,
            stored.opposite
        );
        for (what, pairs, max) in [
            ("ambiguous pairs on the server", &s, stored.server),
            ("ambiguous pairs on the client", &c, stored.client),
            ("conflicting pairs the server and the client run in opposite orders", &opposite, stored.opposite),
        ] {
            if pairs.len() > max {
                report += &format!("{name}: {} {what}, {max} stored:\n", pairs.len());
                for (a, b) in pairs.iter() {
                    report += &format!("  {a}\n    {b}\n");
                }
            } else if pairs.len() < max {
                eprintln!("  {name}: {what} fell to {}: lower the stored number", pairs.len());
            }
        }
    }
    if report.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "order the new pairs (`before`/`after`, a set; `ambiguous_with` where either order is right):\n{report}"
        ))
    }
}

/// `label`'s ambiguous pairs: systems whose access conflicts with no
/// order between them (ignored ambiguities left out), by name, each pair
/// and the list sorted; the single-threaded executor runs such a pair in
/// whatever order the graph sorts to, which another system's
/// constraints can change.
pub fn ambiguities(world: &mut World, label: impl bevy::ecs::schedule::ScheduleLabel) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let _ = world.try_schedule_scope(label, |world, schedule| {
        schedule.initialize(world).expect("schedule builds");
        let names: std::collections::BTreeMap<_, _> = schedule
            .systems()
            .expect("initialized")
            .map(|(key, system)| (key, system.name().to_string()))
            .collect();
        for (a, b, _) in schedule.graph().conflicting_systems().iter() {
            let (a, b) = (names[a].clone(), names[b].clone());
            out.push(if a <= b { (a, b) } else { (b, a) });
        }
    });
    out.sort();
    out
}
