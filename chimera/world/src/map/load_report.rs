//! Where a map load's work goes, by phase: the game's loader, each
//! game's dressing (the gameplay's, each guest's), each kept dressing's
//! conversion or read (`map::kept`). Each phase has its instructions
//! (its thread's and the threads' it starts: the same from run to run,
//! whatever the box's load), its wall time (for reading only) and what
//! the allocator had in use more when it ended (bytes; a phase that ran
//! beside others on other threads counts theirs too, so byte counts are
//! told apart only with `CHIMERA_LOAD_SERIAL=1`, which dresses the
//! games one after another). docs/performance.md, "Every game
//! installed".
//!
//! The phases are kept in the app's `LoadContext` until someone takes
//! them (`take`): the client's start map and the dedicated server's
//! print them on stderr with the map's numbers (before the log is up);
//! every load logs its own (`MapSources::load_with`, `log_since`). Tests read them (`take`) to say what each
//! guest costs.

use std::sync::Mutex;

use chimera_core::metrics::{InstructionCounter, memory};

use super::LoadContext;

/// One phase of a load.
#[derive(Clone, Debug, PartialEq)]
pub struct Phase {
    /// What ran (`loader cs_source`, `dress tf2`, `kept tf2: read from
    /// disk`).
    pub name: String,
    /// Wall time, ms (reading only: it moves with the box's load).
    pub ms: f64,
    /// Instructions of the phase's thread and the threads it started
    /// (None: no hardware counters).
    pub instructions: Option<u64>,
    /// The allocator's bytes in use at its end less at its start (None:
    /// no counts on this platform).
    pub bytes: Option<i64>,
}

impl Phase {
    /// `dress tf2: 1,234 M instructions, 451 ms, +120 MB`.
    pub fn line(&self) -> String {
        let instructions = self.instructions.map_or(String::new(), |i| format!("{} M instructions, ", i / 1_000_000));
        let bytes = self.bytes.map_or(String::new(), |b| format!(", {:+} MB in use", b / (1 << 20)));
        format!("{}: {instructions}{:.0} ms{bytes}", self.name, self.ms)
    }
}

/// The phases of the app's loads not taken yet (`LoadContext`).
#[derive(Default)]
struct Phases(Mutex<Vec<Phase>>);

/// How many phases a context keeps when nobody takes them (a session of
/// map changes on an app that never reads them).
const KEEP: usize = 256;

/// `f` run as phase `name` of a load of `context`'s app: counted and
/// kept for `take`.
pub fn phase<T>(context: &LoadContext, name: impl Into<String>, f: impl FnOnce() -> T) -> T {
    let mut running = Running::start();
    let out = f();
    running.end(context, name);
    out
}

/// A phase under way, for one whose name is known only at its end
/// (`kept`: converted, or read from disk).
pub struct Running {
    began: std::time::Instant,
    counter: InstructionCounter,
    from: Option<u64>,
    in_use: Option<u64>,
}

impl Running {
    pub fn start() -> Self {
        let mut counter = InstructionCounter::current_with_children();
        let from = counter.read();
        Self { began: std::time::Instant::now(), counter, from, in_use: memory::malloc().map(|m| m.in_use) }
    }

    /// The phase's instructions so far.
    pub fn instructions(&mut self) -> Option<u64> {
        self.from.zip(self.counter.read()).map(|(a, b)| b.saturating_sub(a))
    }

    /// The phase ended: kept in `context` as `name`; returned too.
    pub fn end(&mut self, context: &LoadContext, name: impl Into<String>) -> Phase {
        let phase = Phase {
            name: name.into(),
            ms: self.began.elapsed().as_secs_f64() * 1000.0,
            instructions: self.instructions(),
            bytes: self.in_use.zip(memory::malloc()).map(|(a, b)| b.in_use as i64 - a as i64),
        };
        let phases = context.get_or_init(Phases::default);
        let mut kept = phases.0.lock().unwrap_or_else(|e| e.into_inner());
        if kept.len() >= KEEP {
            kept.remove(0);
        }
        kept.push(phase.clone());
        phase
    }
}

/// The phases of `context`'s loads since the last `take`, in the order
/// they ended.
pub fn take(context: &LoadContext) -> Vec<Phase> {
    context.get::<Phases>().map_or_else(Vec::new, |p| std::mem::take(&mut *p.0.lock().unwrap_or_else(|e| e.into_inner())))
}

/// How many phases `context` keeps now (a mark for `log_since`).
pub fn mark(context: &LoadContext) -> usize {
    context.get::<Phases>().map_or(0, |p| p.0.lock().unwrap_or_else(|e| e.into_inner()).len())
}

/// The phases kept since `mark`, logged (info) as the load of `what`;
/// they stay for `take`.
pub fn log_since(context: &LoadContext, mark: usize, what: &str) {
    let Some(phases) = context.get::<Phases>() else { return };
    let kept = phases.0.lock().unwrap_or_else(|e| e.into_inner());
    let lines: Vec<String> = kept.iter().skip(mark.min(kept.len())).map(Phase::line).collect();
    if !lines.is_empty() {
        bevy::log::info!("load of {what}: {}", lines.join("; "));
    }
}

/// `CHIMERA_LOAD_SERIAL=1`: the games dress one after another (no
/// `dress_ahead`), so each phase's bytes are its own (measuring).
pub fn serial() -> bool {
    chimera_core::env::var("CHIMERA_LOAD_SERIAL").is_ok_and(|v| v == "1")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phases are kept in the order they end, per context, and taken once.
    #[test]
    fn phases_are_kept_per_context_until_taken() {
        let (a, b) = (LoadContext::default(), LoadContext::default());
        assert_eq!(phase(&a, "one", || 1), 1);
        let mut running = Running::start();
        let v: Vec<u8> = vec![1; 1 << 20];
        let two = running.end(&a, format!("two: {}", v.len()));
        assert_eq!(two.name, "two: 1048576");
        assert!(two.line().starts_with("two: 1048576: "), "{}", two.line());
        let names: Vec<String> = take(&a).into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["one", "two: 1048576"]);
        assert!(take(&a).is_empty() && take(&b).is_empty());
    }

    /// A context nobody reads keeps the newest phases only.
    #[test]
    fn an_unread_context_keeps_a_bounded_list() {
        let c = LoadContext::default();
        for i in 0..KEEP + 3 {
            phase(&c, i.to_string(), || ());
        }
        let kept = take(&c);
        assert_eq!(kept.len(), KEEP);
        assert_eq!(kept[0].name, "3");
    }
}
