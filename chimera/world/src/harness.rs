//! Headless simulation for tests and tools: no window, no rendering, time
//! advanced by exact fixed ticks. Game-neutral: the simulation's plugins
//! come in as the `base` a `Sim` is built on (a game's set, e.g.
//! `cs_source::harness::CsSim`; the engine's alone, e.g.
//! `chimera_sim::SimBasePlugins`), so a crate's tests use the harness at
//! the layer they test (docs/OBSERVABILITY.md, section 1).
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
//! processes don't each start a thread per core; `schedule_systems` and
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

/// Threads of the task pools every `Sim` (`add_headless`) process starts (Bevy's
/// `TaskPoolOptions::with_num_threads`: one I/O, one async compute, two
/// compute): nextest runs tests as parallel processes, each would
/// otherwise start a full set. The pools are process-wide; the first app
/// built in a process sets them.
pub const SIM_TASK_THREADS: usize = 4;

/// The engine with no window or rendering: what `Sim` runs (stepped by
/// hand) under its base plugins, its task pools capped
/// (`SIM_TASK_THREADS`).
pub fn add_headless(app: &mut App) {
    add_headless_with(
        app,
        MinimalPlugins.build().set(bevy::app::TaskPoolPlugin {
            task_pool_options: bevy::app::TaskPoolOptions::with_num_threads(SIM_TASK_THREADS),
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
        let mut app = App::new();
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
        }
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / DEFAULT_TICK_HZ,
        )));
        app.finish();
        app.cleanup();
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
        while self.tick() < target {
            self.app.update();
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
