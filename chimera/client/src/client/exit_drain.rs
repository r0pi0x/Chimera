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
//! The quick exit (`chimera_core::shutdown`) ends the process without
//! dropping the app, so the same wait comes before its exit call: with
//! pipelined rendering its `Settle` step (`settle_render`) takes the render
//! world back from the render thread (that frame's draw done, the thread
//! idle) and waits for the pipelines in it; with the render world on the
//! main thread (`ExitInRender`) that world's last `Cleanup`, after the
//! wait, makes the exit call (`QuittingNow`). The window is hidden first
//! (`Stage::Hide`, `hide_windows`), so the wait isn't seen.
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
        pipelined_rendering::RenderAppChannels,
        render_resource::{CachedPipelineState, PipelineCache},
    },
};
use chimera_core::shutdown::{ExitInRender, ExitMode, QuittingNow, ShutdownAppExt, Stage};

/// How long the last frame waits for compiling pipelines at most.
pub const MAX_WAIT: Duration = Duration::from_secs(30);

/// Set in the render world when the main world has asked to exit.
/// The quick exit's call, when the main world handed it to this one.
#[derive(Resource, Default)]
struct Exiting(bool, Option<QuittingNow>);

pub struct ExitDrainPlugin;

impl Plugin for ExitDrainPlugin {
    fn build(&self, app: &mut App) {
        app.shutdown_flush(Stage::Hide, "hide the window", hide_windows)
            .shutdown_flush(Stage::Settle, "render thread", settle_render);
        let Some(render) = app.get_sub_app_mut(RenderApp) else { return };
        render
            .init_resource::<Exiting>()
            .add_systems(ExtractSchedule, see_exit)
            .add_systems(Render, (wait_for_pipelines, exit_from_render).chain().in_set(RenderSystems::Cleanup));
    }

    fn cleanup(&self, app: &mut App) {
        // Still a sub-app here (after `PipelinedRenderingPlugin`'s cleanup,
        // which takes it to its thread): the render world runs on the main
        // thread after the main schedule, and makes the quick exit's call.
        if app.get_sub_app(RenderApp).is_some() {
            app.init_resource::<ExitInRender>();
        }
    }
}

fn see_exit(
    exit: Extract<Option<Res<Messages<AppExit>>>>,
    quitting: Extract<Option<Res<QuittingNow>>>,
    mut exiting: ResMut<Exiting>,
) {
    if exit.as_ref().is_some_and(|m| !m.is_empty()) {
        exiting.0 = true;
    }
    if let Some(q) = quitting.as_ref() {
        exiting.1 = Some((**q).clone());
    }
}

fn wait_for_pipelines(exiting: Res<Exiting>, cache: Option<Res<PipelineCache>>) {
    let Some(cache) = cache.filter(|_| exiting.0) else { return };
    wait_logged(&cache);
}

/// Wait for `cache`'s compiling pipelines (at most `MAX_WAIT`), logged.
fn wait_logged(cache: &PipelineCache) {
    let compiling = compiling(cache);
    if compiling == 0 {
        return;
    }
    match wait_until_compiled(cache, MAX_WAIT) {
        Ok(took) => info!("exit: waited {} ms for {compiling} pipelines to compile", took.as_millis()),
        Err(left) => warn!("exit: {left} pipelines still compiling after {}s", MAX_WAIT.as_secs()),
    }
}

/// The render world's last frame, on the main thread: the quick exit.
fn exit_from_render(exiting: Res<Exiting>) {
    if let Some(q) = &exiting.1 {
        q.call.exit(q.code);
    }
}

/// `Stage::Hide`: every window hidden at once (winit's own systems would
/// only do it next frame). Not on Wayland, which can't hide one.
fn hide_windows(_: &mut World) {
    bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
        for window in windows.windows.values() {
            window.set_visible(false);
        }
    });
}

/// `Stage::Settle` of the quick exit with pipelined rendering: the render
/// world back from its thread (the frame it draws finished; the thread
/// then waits for the next, idle), and its compiling pipelines waited for.
/// The render world is kept (never dropped): the process ends next.
fn settle_render(world: &mut World) {
    let mode = world.get_resource::<ExitMode>().copied().unwrap_or_default();
    if mode.is_clean() {
        return;
    }
    let Some(executor) = world.get_resource::<bevy::ecs::schedule::MainThreadExecutor>().cloned() else {
        return;
    };
    let Some(mut channels) = world.get_resource_mut::<RenderAppChannels>() else { return };
    // As `renderer_extract` waits for it: the main thread's tasks pumped.
    let render = bevy::tasks::ComputeTaskPool::get()
        .scope_with_executor(true, Some(&*executor.0), |s| {
            s.spawn(async { channels.recv().await });
        })
        .pop()
        .flatten();
    let Some(mut render) = render else { return };
    if let Some(cache) = render.world_mut().get_resource::<PipelineCache>() {
        wait_logged(cache);
    }
    if mode.is_measured() {
        // `chimera:clean_exit 2`: the render world's drop (its meshes,
        // textures and pipelines, the wgpu device) timed.
        let start = std::time::Instant::now();
        drop(render);
        eprintln!("exit: render world dropped in {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
        return;
    }
    std::mem::forget(render);
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
