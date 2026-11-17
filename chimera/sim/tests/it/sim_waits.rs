//! Contract tests of the harness's clock and waits (`Sim::advance`,
//! `run_until`, `expect_until`, `await_message`, `run_until_idle`):
//! docs/OBSERVABILITY.md, section 1, "Time and waiting".

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::tasks;
use chimera_sim::SimBasePlugins;
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

fn sim() -> Sim {
    Sim::new(SimBasePlugins, GreyboxMapPlugin)
}

#[test]
fn advance_moves_game_and_real_time_at_once() {
    let mut sim = sim();
    let real = sim.app.world().resource::<Time<Real>>().elapsed();
    let virt = sim.app.world().resource::<Time<Virtual>>().elapsed();
    sim.advance(Duration::from_secs(5));
    let w = sim.app.world();
    assert!(((w.resource::<Time<Real>>().elapsed() - real).as_secs_f64() - 5.0).abs() < 0.05);
    assert!(((w.resource::<Time<Virtual>>().elapsed() - virt).as_secs_f64() - 5.0).abs() < 0.05);
}

#[test]
fn run_until_counts_ticks_and_times_out_with_the_state() {
    let mut sim = sim();
    let start = sim.tick();
    let took = sim.run_until(|w| w.resource::<chimera_core::core::SimTick>().0 >= start + 7, 100).unwrap();
    assert_eq!(took, 7);
    let err = sim.run_until(|_| false, 5).unwrap_err();
    assert_eq!(err.ticks, 5);
    let text = err.to_string();
    assert!(text.contains("5 ticks") && text.contains("pending"), "{text}");
}

#[test]
#[should_panic(expected = "the door to open")]
fn expect_until_names_what_it_awaited() {
    sim().expect_until("the door to open", |_| false, 3);
}

#[derive(Message, Clone, Debug, PartialEq)]
struct Ping(u64);

#[test]
fn await_message_returns_the_first_one_written() {
    let mut sim = sim();
    sim.app.add_message::<Ping>().add_systems(
        FixedUpdate,
        |tick: Res<chimera_core::core::SimTick>, mut out: MessageWriter<Ping>| {
            if tick.0.is_multiple_of(10) {
                out.write(Ping(tick.0));
            }
        },
    );
    let Ping(at) = sim.await_message::<Ping>(30).expect("a ping within 30 ticks");
    assert!(at.is_multiple_of(10));
    assert!(sim.await_message::<Ping>(3).is_err(), "the next one is ten ticks away");
}

#[derive(Resource, Default)]
struct Job(Option<bevy::tasks::Task<u64>>, Option<u64>);

#[test]
fn run_until_idle_waits_for_a_background_task_and_its_pickup() {
    let mut sim = sim();
    sim.app.init_resource::<Job>().add_systems(Update, |mut job: ResMut<Job>| {
        if let Some(t) = job.0.as_mut()
            && let Some(v) = bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(t))
        {
            job.0 = None;
            job.1 = Some(v);
        }
    });
    let pool = bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
    let task = tasks::spawn(&pool, async { (0..20_000_000u64).fold(0, |a, b| a ^ b.wrapping_mul(31)) });
    sim.app.world_mut().resource_mut::<Job>().0 = Some(task);
    assert!(tasks::pending() >= 1);
    sim.run_until_idle(10_000).expect("the task finishes");
    assert_eq!(tasks::pending(), 0);
    assert!(sim.app.world().resource::<Job>().1.is_some(), "its result was taken up");
}

#[test]
fn run_until_idle_gives_up_on_work_that_never_ends() {
    let mut sim = sim();
    let _busy = tasks::Busy::new();
    let err = sim.run_until_idle(20).unwrap_err();
    assert!(err.pending >= 1 && err.ticks == 20, "{err}");
}
