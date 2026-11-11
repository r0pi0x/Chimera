//! An instructions-per-tick regression gate (docs/performance.md,
//! "Regression gate"): scenarios with fixed inputs that play out the same
//! on every run (`harness::Sim::repeatable`), measured by work counts
//! (`chimera_core::metrics::work`: user-space instructions in the tick's
//! systems, on the schedules' thread), compared
//! with a stored baseline per build profile. The same build counts the
//! same to a few hundredths of a per cent, idle or loaded; a failure
//! prints the systems that grew against the baseline's. CPU and wall
//! time are printed for reading, never gated.
//!
//! The gate is game-neutral: an app or game names its scenarios and
//! builds each as something to tick (`Gate`, `main`); this module parses
//! the command line, measures, compares and updates the baseline. CS:S's
//! scenarios and binary: games/cs_source/tools/src/bin/perfgate.rs.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::Instant,
};

use chimera_core::metrics::{Percentiles, counters::process_cpu_ns, hw_status, short, work};
use serde::{Deserialize, Serialize};

/// Default allowed rise (percent): see docs/performance.md, "Regression
/// gate", for the measured run-to-run spread it is chosen from (under
/// 0.1%; the rest is room for a change that isn't worth a look).
pub const DEFAULT_TOLERANCE: f64 = 1.0;

/// Systems kept per scenario in a baseline, the costliest first (a
/// failure's diff compares them).
const SYSTEMS_KEPT: usize = 80;

/// One scenario's measurements.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Measured {
    pub ticks: u64,
    /// The ticks' work counts together (`metrics::work`): what is gated.
    pub instructions_total: u64,
    /// Per tick.
    pub instructions_p50: f64,
    pub instructions_p99: f64,
    /// The costliest systems' instructions over the run (`SYSTEMS_KEPT`),
    /// by schedule and name: a failure's diff.
    #[serde(default)]
    pub systems: BTreeMap<String, u64>,
    /// The simulation's state after the run (`Scenario::state_hash`): the
    /// same on every run of a build when the scenario is deterministic.
    #[serde(default)]
    pub state_hash: Option<u64>,
    /// CPU time per tick, all threads (ms), and wall time per tick: for
    /// reading, not gated (they follow the machine's load).
    pub cpu_ms_p50: f64,
    pub cpu_ms_p99: f64,
    pub wall_ms_p50: f64,
    pub wall_ms_p99: f64,
}

#[derive(Serialize, Deserialize, Default)]
struct Baselines {
    /// Allowed rise, percent, and per scenario where its own run-to-run
    /// spread needs more (greybox: ticks of under a million instructions,
    /// where the task pools' overhead weighs more).
    tolerance_pct: f64,
    #[serde(default)]
    scenario_tolerance_pct: BTreeMap<String, f64>,
    /// By build profile (`playtest`, `debug`, ...), then scenario.
    profiles: BTreeMap<String, Profile>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct Profile {
    /// The commit the numbers were taken at, and the CPU.
    build: String,
    cpu: String,
    scenarios: BTreeMap<String, Measured>,
}

/// A scenario to measure: one tick at a time, and a hash of its state
/// (`harness::Sim::state_hash`) to tell whether two runs played alike.
pub trait Scenario {
    fn tick(&mut self);
    fn state_hash(&mut self) -> Option<u64> {
        None
    }
    /// The state hashed, one line per entity (`--repeat` prints the first
    /// that differ).
    fn state_lines(&mut self) -> Vec<String> {
        Vec::new()
    }
}

/// What a gate measures: its scenarios' names, its baseline file and the
/// build it runs (the commit, recorded with an updated baseline).
pub struct Gate<'a> {
    pub name: &'a str,
    pub scenarios: &'a [&'a str],
    pub baseline: PathBuf,
    pub build: &'a str,
}

struct Args {
    scenarios: Vec<String>,
    ticks: u64,
    warmup: u64,
    tolerance: Option<f64>,
    baseline: PathBuf,
    update: bool,
    json: Option<PathBuf>,
    repeat: bool,
}

fn usage(gate: &Gate) -> String {
    format!(
        "\
usage: {name} [--scenario <name>]... [--ticks N] [--warmup N] [--tolerance PCT]
                [--baseline <file>] [--update] [--json <file>]
  Runs headless scenarios with fixed inputs and counts the user-space
  instructions of each tick's systems with hardware counters (the same on
  every run, idle or loaded); fails when a scenario's total rises more
  than the tolerance above the baseline for this build profile, and
  prints the systems that grew. Skips (exit 0) where counters are
  unavailable (Windows).
  --scenario <name>  {scenarios} (default: all
                     available)
  --ticks N          ticks measured per scenario (default 2000)
  --warmup N         ticks run first, not measured (default 300)
  --tolerance PCT    allowed rise in percent (default: the baseline's)
  --baseline <file>  default {baseline}
  --update           write this run's numbers as the baseline for this
                     build profile (after checking a change is wanted)
  --json <file>      also write this run's numbers there
  --repeat           check the scenarios repeat: build each twice, tick both
                     side by side and report the first tick their states
                     differ (and the entities that differ); measures nothing",
        name = gate.name,
        scenarios = gate.scenarios.join(", "),
        baseline = gate.baseline.display()
    )
}

fn parse_args(gate: &Gate) -> Result<Args, String> {
    let mut a = Args {
        scenarios: Vec::new(),
        ticks: 2000,
        warmup: 300,
        tolerance: None,
        baseline: gate.baseline.clone(),
        update: false,
        json: None,
        repeat: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--scenario" => a.scenarios.push(value()?),
            "--ticks" => a.ticks = value()?.parse().map_err(|e| format!("--ticks: {e}"))?,
            "--warmup" => a.warmup = value()?.parse().map_err(|e| format!("--warmup: {e}"))?,
            "--tolerance" => a.tolerance = Some(value()?.parse().map_err(|e| format!("--tolerance: {e}"))?),
            "--baseline" => a.baseline = value()?.into(),
            "--json" => a.json = Some(value()?.into()),
            "--update" => a.update = true,
            "--repeat" => a.repeat = true,
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    for s in &a.scenarios {
        if !gate.scenarios.contains(&s.as_str()) {
            return Err(format!("unknown scenario {s}"));
        }
    }
    Ok(a)
}

/// The gate's binary: parse the command line, build each wanted scenario
/// with `scenario` (None: skipped, e.g. its map isn't installed), measure,
/// then compare with the baseline or (`--update`) write it. Exits the
/// process with the verdict.
pub fn main<'s>(gate: Gate, mut scenario: impl FnMut(&str) -> Option<Box<dyn Scenario + 's>>) {
    let args = match parse_args(&gate) {
        Ok(a) => a,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{e}");
            }
            eprintln!("{}", usage(&gate));
            std::process::exit(2);
        }
    };
    // Counters first: without them there is nothing to gate.
    if work::now().is_none() && !args.repeat {
        println!("{}: skipped, hardware counters {}", gate.name, hw_status());
        return;
    }
    let profile = build_profile();
    let wanted: Vec<&str> = if args.scenarios.is_empty() {
        gate.scenarios.to_vec()
    } else {
        args.scenarios.iter().map(String::as_str).collect()
    };
    if args.repeat {
        let mut apart = Vec::new();
        for name in &wanted {
            let (Some(mut a), Some(mut b)) = (scenario(name), scenario(name)) else {
                println!("{name}: skipped (map not installed or cached)");
                continue;
            };
            let total = args.warmup + args.ticks;
            let mut first = None;
            for t in 0..total {
                a.tick();
                b.tick();
                if a.state_hash() != b.state_hash() {
                    first = Some(t);
                    break;
                }
            }
            match first {
                None => println!("{name}: {total} ticks, the same state on every tick"),
                Some(t) => {
                    println!("{name}: the states differ from tick {t} on (after the scenario's start):");
                    let (la, lb) = (a.state_lines(), b.state_lines());
                    for (x, y) in la.iter().zip(&lb).filter(|(x, y)| x != y).take(12) {
                        println!("  {x}\n  {y}");
                    }
                    if la.len() != lb.len() {
                        println!("  {} entities against {}", la.len(), lb.len());
                    }
                    apart.push(name.to_string());
                }
            }
        }
        if !apart.is_empty() {
            eprintln!("{}: {} didn't repeat", gate.name, apart.join(", "));
            std::process::exit(1);
        }
        return;
    }
    let mut results = BTreeMap::new();
    for name in wanted {
        let Some(mut s) = scenario(name) else {
            println!("{name}: skipped (map not installed or cached)");
            continue;
        };
        let mut r = measure(|| s.tick(), args.warmup, args.ticks);
        r.state_hash = s.state_hash();
        println!(
            "{name}: {} ticks, instructions per tick p50 {} p99 {} total {}; \
             cpu ms p50 {:.3} p99 {:.3}; wall ms p50 {:.3} p99 {:.3} (read only); state {}",
            r.ticks,
            short(r.instructions_p50),
            short(r.instructions_p99),
            short(r.instructions_total as f64),
            r.cpu_ms_p50,
            r.cpu_ms_p99,
            r.wall_ms_p50,
            r.wall_ms_p99,
            r.state_hash.map_or("-".to_string(), |h| format!("{h:016x}"))
        );
        results.insert(name.to_string(), r);
    }
    let mut baselines: Baselines = std::fs::read_to_string(&args.baseline)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Baselines {
            tolerance_pct: DEFAULT_TOLERANCE,
            ..Default::default()
        });
    let this = Profile {
        build: gate.build.to_string(),
        cpu: cpu_model(),
        scenarios: results.clone(),
    };
    if let Some(path) = &args.json
        && let Err(e) = std::fs::write(path, serde_json::to_string_pretty(&this).unwrap())
    {
        eprintln!("{}: {e}", path.display());
    }
    if args.update {
        let entry = baselines.profiles.entry(profile.clone()).or_default();
        entry.build = this.build.clone();
        entry.cpu = this.cpu.clone();
        entry.scenarios.extend(results);
        if let Some(t) = args.tolerance {
            baselines.tolerance_pct = t;
        }
        if let Some(dir) = args.baseline.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match std::fs::write(&args.baseline, serde_json::to_string_pretty(&baselines).unwrap() + "\n") {
            Ok(()) => println!("baseline for `{profile}` written to {}", args.baseline.display()),
            Err(e) => {
                eprintln!("{}: {e}", args.baseline.display());
                std::process::exit(1);
            }
        }
        return;
    }
    let Some(base) = baselines.profiles.get(&profile) else {
        println!(
            "no baseline for build profile `{profile}` in {}: nothing compared (--update writes one)",
            args.baseline.display()
        );
        return;
    };
    if base.cpu != this.cpu {
        println!("note: the baseline was taken on {:?}, this is {:?}", base.cpu, this.cpu);
    }
    let mut failed = Vec::new();
    for (name, r) in &this.scenarios {
        let Some(b) = base.scenarios.get(name) else {
            println!("{name}: no baseline");
            continue;
        };
        if b.ticks != r.ticks {
            println!(
                "{name}: baseline has {} ticks, this run {}: not compared",
                b.ticks, r.ticks
            );
            continue;
        }
        let tolerance = args
            .tolerance
            .or(baselines.scenario_tolerance_pct.get(name).copied())
            .unwrap_or(baselines.tolerance_pct);
        let change = (r.instructions_total as f64 / b.instructions_total.max(1) as f64 - 1.0) * 100.0;
        let verdict = if change > tolerance {
            failed.push(format!("{name} (tolerance {tolerance}%)"));
            "FAIL"
        } else if change < -tolerance {
            "faster (update the baseline if intended)"
        } else {
            "ok"
        };
        println!(
            "{name}: {} instructions vs baseline {} ({change:+.2}%, tolerance {tolerance}%): {verdict}",
            short(r.instructions_total as f64),
            short(b.instructions_total as f64)
        );
        if let (Some(a), Some(h)) = (b.state_hash, r.state_hash)
            && a != h
        {
            println!(
                "{name}: played differently from the baseline (state {h:016x}, baseline {a:016x}): a change of \
                 behaviour, or something in the scenario isn't deterministic"
            );
        }
        if change.abs() > tolerance && !b.systems.is_empty() {
            // The systems both kept (each keeps its costliest: one just
            // past the other's cut would look new).
            let counts = |m: &BTreeMap<String, u64>, other: &BTreeMap<String, u64>| {
                work::WorkCounts(m.iter().filter(|(k, _)| other.contains_key(*k)).map(|(k, v)| (k.clone(), (*v, 0))).collect())
            };
            let (now, base) = (counts(&r.systems, &b.systems), counts(&b.systems, &r.systems));
            println!("{name}: systems by instructions over the run, against the baseline (the costliest both kept):");
            print!("{}", now.diff_table(&base, 20));
        }
    }
    if !failed.is_empty() {
        eprintln!(
            "{}: {} rose by more than the tolerance in instructions: the systems above grew (`perf record -e \
             instructions:u` on the scenario says where), or if the cost is wanted, `{} --update` \
             (docs/performance.md, \"Regression gate\")",
            gate.name,
            failed.join(", "),
            gate.name
        );
        std::process::exit(1);
    }
}

/// Run `warmup` ticks, then measure `ticks` ticks one at a time (`tick`
/// runs one) by their work counts (`metrics::work`, which the scenario's
/// systems must be wrapped for: `harness::Sim` does it).
pub fn measure(mut tick: impl FnMut(), warmup: u64, ticks: u64) -> Measured {
    for _ in 0..warmup {
        tick();
    }
    // CPU time from the process's clock, not per-thread counter groups:
    // more hardware events on the thread than the PMU has counters would
    // have the kernel multiplex them, and the counts become estimates.
    let (mut ins, mut cpu, mut wall) = (Vec::new(), Vec::new(), Vec::new());
    work::set_enabled(true);
    let _ = work::WorkCounts::take();
    let mut total = 0u64;
    for _ in 0..ticks {
        let (c0, w0, t0) = (process_cpu_ns().unwrap_or(0), work::now().unwrap_or(0), Instant::now());
        tick();
        let w = work::now().unwrap_or(0).saturating_sub(w0);
        let wall_ms = t0.elapsed().as_secs_f64() * 1e3;
        let c = process_cpu_ns().unwrap_or(0).saturating_sub(c0);
        ins.push(w as f64);
        total += w;
        cpu.push(c as f64 / 1e6);
        wall.push(wall_ms);
    }
    let systems = work::WorkCounts::take();
    work::set_enabled(false);
    if work::multiplexed() {
        println!(
            "note: the kernel multiplexed the instruction counter (more hardware events on the thread than the PMU \
             has counters): these counts are estimates and won't repeat"
        );
    }
    let p = |v: &[f64]| Percentiles::of(v).unwrap_or_default();
    let (pi, pc, pw) = (p(&ins), p(&cpu), p(&wall));
    Measured {
        ticks,
        instructions_total: total,
        instructions_p50: pi.p50,
        instructions_p99: pi.p99,
        systems: systems.top(SYSTEMS_KEPT).into_iter().map(|(k, i, _)| (k.to_string(), i)).collect(),
        state_hash: None,
        cpu_ms_p50: pc.p50,
        cpu_ms_p99: pc.p99,
        wall_ms_p50: pw.p50,
        wall_ms_p99: pw.p99,
    }
}

/// Which build this binary is (`playtest`, `debug`, `release`): the
/// target folder it runs from.
fn build_profile() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent()?.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|t| {
            t.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| std::env::consts::ARCH.to_string())
}
