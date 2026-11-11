//! Ending a client without a crash in the GPU driver: the render world's
//! last frame waits for the pipelines still compiling on Bevy's async
//! compute threads (`PipelineCache`'s `Creating` tasks) before the app is
//! dropped.
//!
//! Without it, a run that ends while pipelines compile (an offscreen run's
//! `--frames`, a `quit` or a closed window early in a map) drops the render
//! world while a worker thread is still inside the driver compiling one.
//! The task then holds the last reference to the wgpu device, so the
//! device is destroyed on that worker, while the main thread has returned
//! from `main` and runs libc's `exit` handlers, the NVIDIA driver's
//! teardown among them. Seen as `free(): invalid size` or another heap
//! abort, `pthread_cond_destroy` aborting under `vkDestroyDevice` on an
//! "Async Compute" thread, or a SIGSEGV in `libnvidia-gpucomp` or
//! `create_pipeline_layout`, always with the main thread in `exit`
//! (core dumps of `ultimate_gun_game` and `cs_source` offscreen runs,
//! 2026-11-10).
//!
//! The render world sees the exit at extraction (`Messages<AppExit>` is
//! filled in the frame that ends the app, and both the schedule runner and
//! winit's stop after that frame's update, which extracts), and its
//! `Cleanup` then waits until every started pipeline task has finished
//! (async-task drops a task's future before it marks it finished, so the
//! device and layouts it held are released by then; the results stay in
//! the cache and drop with the render world on the main thread). Queued
//! pipelines that never started are left alone.
//!
//! Known shortcuts:
//! - The wait is capped at `MAX_WAIT`; a driver compiling longer than that
//!   still races the exit, with a warning.
//! - Only the pipeline cache's tasks are waited for; nothing else of ours
//!   touches the device off the render thread.

use std::time::{Duration, Instant};

use bevy::{
    ecs::message::Messages,
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        render_resource::{CachedPipelineState, PipelineCache},
    },
};

/// How long the last frame waits for compiling pipelines at most.
pub const MAX_WAIT: Duration = Duration::from_secs(30);

/// Set in the render world when the main world has asked to exit.
#[derive(Resource, Default)]
struct Exiting(bool);

pub struct ExitDrainPlugin;

impl Plugin for ExitDrainPlugin {
    fn build(&self, app: &mut App) {
        let Some(render) = app.get_sub_app_mut(RenderApp) else { return };
        render
            .init_resource::<Exiting>()
            .add_systems(ExtractSchedule, see_exit)
            .add_systems(Render, wait_for_pipelines.in_set(RenderSystems::Cleanup));
    }
}

fn see_exit(exit: Extract<Option<Res<Messages<AppExit>>>>, mut exiting: ResMut<Exiting>) {
    if exit.as_ref().is_some_and(|m| !m.is_empty()) {
        exiting.0 = true;
    }
}

fn wait_for_pipelines(exiting: Res<Exiting>, cache: Option<Res<PipelineCache>>) {
    let Some(cache) = cache.filter(|_| exiting.0) else { return };
    let compiling = compiling(&cache);
    if compiling == 0 {
        return;
    }
    match wait_until_compiled(&cache, MAX_WAIT) {
        Ok(took) => info!("exit: waited {} ms for {compiling} pipelines to compile", took.as_millis()),
        Err(left) => warn!("exit: {left} pipelines still compiling after {}s", MAX_WAIT.as_secs()),
    }
}

/// Pipelines of `cache` whose compile task is still running (or queued
/// on the task pool).
pub fn compiling(cache: &PipelineCache) -> usize {
    cache
        .pipelines()
        .filter(|p| matches!(&p.state, CachedPipelineState::Creating(task) if !task.is_finished()))
        .count()
}

/// Wait until no task of `cache` is compiling, at most `limit`: Ok with
/// how long it took, Err with how many were left.
pub fn wait_until_compiled(cache: &PipelineCache, limit: Duration) -> Result<Duration, usize> {
    let start = Instant::now();
    loop {
        let left = compiling(cache);
        if left == 0 {
            return Ok(start.elapsed());
        }
        if start.elapsed() >= limit {
            return Err(left);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
