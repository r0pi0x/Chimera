//! Work counts: user-space instructions per system, counted on the
//! thread that runs it plus the compute pool's threads (docs/performance.md,
//! "Work counts"). The measure for judging a tick's cost: the same build and
//! scenario give the same counts on an idle box and a loaded one, where
//! wall time, CPU time and a whole thread's instructions (which take in
//! the task pools' waiting, spinning and stealing) do not.
//!
//! `count_systems` puts every system of a world's schedules that have not
//! run yet behind a wrapper (`Counted`) that reads the hardware counters
//! before and after the system runs and applies its commands, and adds
//! the difference, less any schedules it runs inside (`FixedMain`'s
//! runner, a map change), to the system's row. Counting is per thread
//! (`set_enabled`; off, the default, a wrapper costs a thread-local
//! read), and so are the rows: `WorkCounts::take` hands over what this
//! thread's systems counted since the last take; `diff_table`, `rises`
//! and `top` say which systems grew.
//!
//! For the counts to repeat, the scenario must (the `harness::Sim`'s
//! `measured` mode does both): run its schedules on one thread
//! (`SingleThreadedExecutor`) and have a compute pool of one thread, so
//! parallel queries run inline (Bevy's `par_iter` with one thread is a
//! plain loop, Bevy's transform propagation spawns no spinning workers);
//! the few tasks still spawned (Bevy's `mark_dirty_trees`, Avian's broad
//! phase) run on the pool's thread or the waiting one, so the pool's
//! threads registered with `register_pool_thread` count too.
//!
//! Known shortcuts: Linux only (hardware counters); elsewhere every
//! count is None and the gates skip. Run conditions and the executor's
//! own bookkeeping are in no row (in a tick's total, not its systems' sum).
//! Systems added after a schedule first ran aren't wrapped. Pool threads
//! are read on every system boundary while counting (two syscalls each),
//! so a counted tick takes longer in wall time. Another test's tasks on a
//! shared compute pool (several tests in one process: `cargo test`, not
//! `nextest`) land in whichever system is waiting: a test that compares
//! counts closely runs in a process of its own (`harness::own_process`).

use std::{
    any::TypeId,
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};

use bevy::ecs::{
    change_detection::{CheckChangeTicks, Tick},
    error::Result,
    schedule::{ApplyDeferred, InternedSystemSet, Schedules, SystemKey, SystemWithAccess},
    system::{RunSystemError, System, SystemIn, SystemStateFlags},
    world::{DeferredWorld, World, unsafe_world_cell::UnsafeWorldCell},
};
use bevy::utils::prelude::DebugName;

use super::InstructionCounter;

/// Rows' labels by index (`Counted::row`).
static LABELS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Compute pool threads (Linux thread ids) counted with every system.
static POOL: Mutex<Vec<(i32, Option<InstructionCounter>)>> = Mutex::new(Vec::new());

/// (world, schedule, system key) already wrapped.
static WRAPPED: Mutex<BTreeSet<(String, SystemKey)>> = Mutex::new(BTreeSet::new());

thread_local! {
    /// Counting on this thread (`set_enabled`).
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    /// This thread's instructions and runs per row: each thread counts
    /// the systems it runs (a test's `Sim` runs its schedules on the
    /// test's thread), so tests beside each other don't mix.
    static ROWS: RefCell<Vec<(u64, u64)>> = const { RefCell::new(Vec::new()) };
    static COUNTER: RefCell<Option<InstructionCounter>> = const { RefCell::new(None) };
    /// Open systems on this thread: instructions at entry, instructions
    /// of the systems run inside.
    static STACK: RefCell<Vec<(u64, u64)>> = const { RefCell::new(Vec::new()) };
}

/// Count the systems this thread runs from now on (true) or stop (false).
pub fn set_enabled(on: bool) {
    if on {
        // Open the counters before the first system needs them.
        let _ = now();
    }
    ENABLED.with(|e| e.set(on));
}

/// Whether this thread counts.
pub fn enabled() -> bool {
    ENABLED.with(Cell::get)
}

/// Count this thread of the compute pool along with every system (call
/// it from `TaskPoolThreadAssignmentPolicy::on_thread_spawn`; the
/// harness's measured mode does). Linux; elsewhere nothing.
pub fn register_pool_thread() {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: gettid has no preconditions.
        let tid = unsafe { libc::gettid() };
        if let Ok(mut p) = POOL.lock() {
            p.push((tid, None));
        }
    }
}

/// Pool threads registered.
pub fn pool_threads() -> usize {
    POOL.lock().map_or(0, |p| p.len())
}

/// Instructions so far of this thread plus the registered pool threads
/// (excluding this thread if it is one of them); None without hardware
/// counters.
pub fn now() -> Option<u64> {
    let own = COUNTER.with(|c| {
        let mut c = c.borrow_mut();
        c.get_or_insert_with(InstructionCounter::current).read()
    })?;
    let mut sum = own;
    #[cfg(target_os = "linux")]
    if let Ok(mut pool) = POOL.lock() {
        // SAFETY: gettid has no preconditions.
        let me = unsafe { libc::gettid() };
        for (tid, c) in pool.iter_mut() {
            if *tid == me {
                continue;
            }
            sum += c
                .get_or_insert_with(|| InstructionCounter::open(*tid))
                .read()
                .unwrap_or(0);
        }
    }
    Some(sum)
}

fn enter() {
    if let Some(at) = now() {
        STACK.with(|s| s.borrow_mut().push((at, 0)));
    }
}

fn leave(row: usize) {
    let Some(at_end) = now() else { return };
    let Some((at, inner)) = STACK.with(|s| s.borrow_mut().pop()) else {
        return;
    };
    let total = at_end.saturating_sub(at);
    STACK.with(|s| {
        if let Some(parent) = s.borrow_mut().last_mut() {
            parent.1 += total;
        }
    });
    ROWS.with(|r| {
        let mut rows = r.borrow_mut();
        if rows.len() <= row {
            rows.resize(row + 1, (0, 0));
        }
        rows[row].0 += total.saturating_sub(inner);
        rows[row].1 += 1;
    });
}

/// A system whose runs are counted (`count_systems` puts each system
/// behind one).
pub struct Counted {
    inner: SystemWithAccess,
    row: usize,
}

impl System for Counted {
    type In = ();
    type Out = ();

    fn name(&self) -> DebugName {
        self.inner.name()
    }

    fn system_type(&self) -> TypeId {
        self.inner.system_type()
    }

    fn flags(&self) -> SystemStateFlags {
        self.inner.flags()
    }

    unsafe fn run_unsafe(&mut self, input: SystemIn<'_, Self>, world: UnsafeWorldCell) -> Result<(), RunSystemError> {
        if !enabled() {
            // SAFETY: the caller's guarantees, passed on.
            return unsafe { self.inner.run_unsafe(input, world) };
        }
        enter();
        // SAFETY: the caller's guarantees, passed on.
        let r = unsafe { self.inner.run_unsafe(input, world) };
        leave(self.row);
        r
    }

    fn apply_deferred(&mut self, world: &mut World) {
        if !enabled() || !self.inner.has_deferred() {
            return self.inner.apply_deferred(world);
        }
        enter();
        self.inner.apply_deferred(world);
        leave(self.row);
    }

    fn queue_deferred(&mut self, world: DeferredWorld) {
        self.inner.queue_deferred(world);
    }

    fn initialize(&mut self, world: &mut World) -> bevy::ecs::system::SystemAccess {
        self.inner.initialize(world)
    }

    fn check_change_tick(&mut self, check: CheckChangeTicks) {
        self.inner.check_change_tick(check);
    }

    fn default_system_sets(&self) -> Vec<InternedSystemSet> {
        self.inner.default_system_sets()
    }

    fn get_last_run(&self) -> Tick {
        self.inner.get_last_run()
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.inner.set_last_run(last_run);
    }
}

/// Put every system of the world's schedules that have not run yet behind
/// a counting wrapper (once; schedules that ran keep theirs as they are:
/// their systems' access is set, and a wrapper would need it again).
/// Returns how many systems it wrapped.
pub fn count_systems(world: &mut World) -> usize {
    let world_id = world.id();
    let Some(mut schedules) = world.get_resource_mut::<Schedules>() else {
        return 0;
    };
    let mut wrapped = 0;
    let mut done = WRAPPED.lock().unwrap_or_else(|e| e.into_inner());
    for (label, schedule) in schedules.iter_mut() {
        if schedule.systems().is_ok() {
            continue; // Initialized: its systems ran or were built.
        }
        let label = format!("{label:?}");
        let world_label = format!("{world_id:?} {label}");
        let systems = &mut schedule.graph_mut().systems;
        let keys: Vec<_> = systems.iter().map(|(k, s, _)| (k, s.system_type())).collect();
        for (key, ty) in keys {
            if ty == TypeId::of::<ApplyDeferred>() {
                continue;
            }
            let id = (world_label.clone(), key);
            if !done.insert(id) {
                continue;
            }
            let Some(slot) = systems.get_mut(key) else { continue };
            let name = slot.name();
            let inner = std::mem::replace(
                slot,
                SystemWithAccess::new(Box::new(bevy::ecs::system::IntoSystem::into_system(noop))),
            );
            let row = {
                let mut labels = LABELS.lock().unwrap_or_else(|e| e.into_inner());
                labels.push(format!("{label} {}", name.shortname()));
                labels.len() - 1
            };
            *slot = SystemWithAccess::new(Box::new(Counted { inner, row }));
            wrapped += 1;
        }
    }
    wrapped
}

fn noop() {}

/// Instructions and runs per system (label: schedule and system name).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkCounts(pub BTreeMap<String, (u64, u64)>);

impl WorkCounts {
    /// The systems this thread ran since the last take, and zero them.
    pub fn take() -> Self {
        let mut out = BTreeMap::new();
        let rows = ROWS.with(|r| std::mem::take(&mut *r.borrow_mut()));
        let labels = LABELS.lock().unwrap_or_else(|e| e.into_inner());
        for (i, (ins, runs)) in rows.into_iter().enumerate() {
            if runs > 0
                && let Some(label) = labels.get(i)
            {
                let e: &mut (u64, u64) = out.entry(label.clone()).or_default();
                e.0 += ins;
                e.1 += runs;
            }
        }
        Self(out)
    }

    /// Add `other`'s rows to these.
    pub fn add(&mut self, other: &Self) {
        for (k, v) in &other.0 {
            let e = self.0.entry(k.clone()).or_default();
            e.0 += v.0;
            e.1 += v.1;
        }
    }

    /// Instructions of every system together.
    pub fn total(&self) -> u64 {
        self.0.values().map(|v| v.0).sum()
    }

    /// The `n` costliest systems, most first: (label, instructions, runs).
    pub fn top(&self, n: usize) -> Vec<(&str, u64, u64)> {
        let mut rows: Vec<_> = self.0.iter().map(|(k, v)| (k.as_str(), v.0, v.1)).collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        rows.truncate(n);
        rows
    }

    /// Each system's instructions scaled by `1 / by` (per tick, when the
    /// counts are over `by` ticks).
    pub fn per(&self, by: u64) -> Self {
        let by = by.max(1);
        Self(self.0.iter().map(|(k, v)| (k.clone(), (v.0 / by, v.1 / by))).collect())
    }

    /// The `n` systems whose instructions rose most from `base` to `self`:
    /// (label, base, now), largest rise first.
    pub fn rises(&self, base: &Self, n: usize) -> Vec<(String, u64, u64)> {
        let mut keys: Vec<&String> = self.0.keys().chain(base.0.keys()).collect();
        keys.sort();
        keys.dedup();
        let mut rows: Vec<_> = keys
            .into_iter()
            .map(|k| {
                (
                    k.clone(),
                    base.0.get(k).map_or(0, |v| v.0),
                    self.0.get(k).map_or(0, |v| v.0),
                )
            })
            .collect();
        rows.sort_by(|a, b| {
            (b.2 as i128 - b.1 as i128)
                .cmp(&(a.2 as i128 - a.1 as i128))
                .then(a.0.cmp(&b.0))
        });
        rows.truncate(n);
        rows
    }

    /// A table of `rises` for a failure message: one line per system,
    /// instructions in `base` and `self`, and the change.
    pub fn diff_table(&self, base: &Self, n: usize) -> String {
        let mut out = format!(
            "{:>12} {:>12} {:>12}  system (top {n} rises; totals {} -> {})\n",
            "base",
            "now",
            "change",
            super::short(base.total() as f64),
            super::short(self.total() as f64)
        );
        for (label, b, a) in self.rises(base, n) {
            out += &format!("{b:>12} {a:>12} {:>+12}  {label}\n", a as i128 - b as i128);
        }
        out
    }

    /// The counts as `label instructions runs` lines (a baseline file).
    pub fn to_lines(&self) -> String {
        self.0.iter().map(|(k, v)| format!("{k}\t{}\t{}\n", v.0, v.1)).collect()
    }

    /// Parse `to_lines`' output.
    pub fn from_lines(text: &str) -> Self {
        Self(
            text.lines()
                .filter_map(|l| {
                    let mut it = l.rsplitn(3, '\t');
                    let runs = it.next()?.parse().ok()?;
                    let ins = it.next()?.parse().ok()?;
                    Some((it.next()?.to_string(), (ins, runs)))
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::*;

    #[derive(Resource, Default)]
    struct N(u64);

    fn busy(mut n: ResMut<N>) {
        for i in 0..100_000u64 {
            n.0 = n.0.wrapping_mul(31).wrapping_add(std::hint::black_box(i));
        }
    }

    fn light(mut n: ResMut<N>) {
        n.0 += 1;
    }

    #[test]
    fn systems_are_counted_and_repeat() {
        let mut world = World::new();
        world.init_resource::<N>();
        let mut schedule = bevy::ecs::schedule::Schedule::new(bevy::app::Update);
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
        schedule.add_systems((busy, light).chain());
        world.add_schedule(schedule);
        assert_eq!(count_systems(&mut world), 2);
        assert_eq!(count_systems(&mut world), 0, "once");
        set_enabled(true);
        if now().is_none() {
            eprintln!("skipping: no hardware counters ({})", super::super::hw_status());
            return;
        }
        let mut runs = Vec::new();
        for _ in 0..3 {
            let _ = WorkCounts::take();
            world.run_schedule(bevy::app::Update);
            runs.push(WorkCounts::take());
        }
        set_enabled(false);
        let busy_row = runs[1]
            .0
            .iter()
            .find(|(k, _)| k.contains("busy"))
            .map(|(_, v)| *v)
            .unwrap();
        let light_row = runs[1]
            .0
            .iter()
            .find(|(k, _)| k.contains("light"))
            .map(|(_, v)| *v)
            .unwrap();
        assert_eq!(busy_row.1, 1);
        assert!(busy_row.0 > 200_000, "{busy_row:?}");
        assert!(light_row.0 < busy_row.0 / 10, "{light_row:?} {busy_row:?}");
        // The same work counts the same, to well under a per cent.
        let a = runs[1].total() as f64;
        let b = runs[2].total() as f64;
        assert!((a - b).abs() / a < 0.005, "{a} {b}");
        assert_eq!(world.resource::<N>().0 > 0, true);
    }
}
