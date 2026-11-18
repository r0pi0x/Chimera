//! The systems' wall times the hitch log names (`metrics::work`, "Timing";
//! the game client turns them on, `client::app::run_app`): each frame's
//! slowest systems, and what timing costs a run (docs/performance.md,
//! "Stutter on Beaver Creek").

use chimera_core::metrics::work;
use chimera_sim::SimBasePlugins;
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

/// A greybox tick timed and not: the frame's slowest systems come out
/// named, slowest first, and timing costs each system run a few hundred
/// instructions (two reads of the time-stamp counter and the row's sum), counted.
#[test]
fn timed_systems_name_the_slowest_and_cost_little() {
    // The switch is the process's: alone, so no other test's ticks pay it.
    if !chimera_world::harness::own_process("system_times::timed_systems_name_the_slowest_and_cost_little") {
        return;
    }
    let mut sim = Sim::new(SimBasePlugins, GreyboxMapPlugin);
    sim.repeatable();
    sim.ticks(20);
    let Some(off) = sim.work(200) else {
        eprintln!("no hardware counters: the cost isn't measured");
        return;
    };
    assert!(work::take_frame_times(3).is_empty(), "nothing timed while off");
    work::set_timing(true);
    let on = sim.work(200).expect("counters");
    let slowest = work::take_frame_times(3);
    work::set_timing(false);
    assert_eq!(slowest.len(), 3, "{slowest:?}");
    assert!(slowest.windows(2).all(|w| w[0].1 >= w[1].1), "slowest first: {slowest:?}");
    assert!(slowest.iter().all(|(name, _, runs)| name.contains(' ') && *runs > 0), "schedule and system: {slowest:?}");
    assert!(work::take_frame_times(3).is_empty(), "taken: the next frame starts empty");
    let runs = on.systems.0.values().map(|v| v.1).sum::<u64>() as f64 / 200.0;
    let per_run = (on.median() as f64 - off.median() as f64) / runs;
    println!("timing: {:.0} system runs a tick, tick {} -> {} instructions, {per_run:.0} a run", runs, off.median(), on.median());
    assert!(per_run < 300.0, "timing costs {per_run:.0} instructions a system run");
}
