//! How far a map load has come, for a loading screen: loaders call
//! `LoadReporter::report` at their stages (from the loading thread; the
//! reporter comes with `MapLoadOptions`), the client reads `current`
//! while it waits. Each app (and each load) has its own reporter, so
//! apps in one process don't share progress.
//!
//! A load's stages (`LoadStep`, docs/plans/active/lobby-between-maps.md):
//! the download (bytes; `chimera_net::net::load_stages` reads the join's), the
//! map (the loader's task: `LoadReporter`), the models (the stepped
//! spawn's parts and props) and the physics (the spawn's held-back
//! colliders, then the settle hooks: navigation, spawn spots, shadows).
//! The spawn and the settling count their work into `LoadWork`, so the
//! bars follow the work done, not the time.

use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use bevy::prelude::Resource;

/// A load's progress: the fraction done (0..1) and the stage, as a
/// localisation token the game words (`LoadingProgress_LoadMap`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoadProgress {
    pub fraction: f32,
    pub stage: &'static str,
}

/// One load's progress, shared between the loader (on a background
/// thread, which can't touch the `World`) and whoever shows it. The app
/// holds it as a resource (`start_map_load` replaces it with a fresh one
/// for each load); the load gets a clone in `MapLoadOptions::progress`.
/// Apps in one process (a listen server and its client, `NetSim`'s
/// server and clients) each have their own. Equal when they are the same
/// handle.
#[derive(Resource, Clone, Debug, Default)]
pub struct LoadReporter(Arc<Mutex<Option<LoadProgress>>>);

impl LoadReporter {
    /// A loader reached `stage` with `fraction` of the work done.
    pub fn report(&self, fraction: f32, stage: &'static str) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(LoadProgress {
            fraction: fraction.clamp(0.0, 1.0),
            stage,
        });
    }

    /// The latest report of this load (none before the first).
    pub fn current(&self) -> Option<LoadProgress> {
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl PartialEq for LoadReporter {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for LoadReporter {}

/// A stage of getting into a map, in order: what the loading screens'
/// bars and the lobby's rows show (`LobbyRow::step` sends `index`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LoadStep {
    /// The match's files (bytes).
    Download,
    /// The loader's task (`LoadReporter`'s share).
    #[default]
    Map,
    /// The spawn's parts and props, a step a frame (`LoadWork`).
    Models,
    /// The spawn's colliders, then the settle hooks (`LoadWork`).
    Physics,
}

impl LoadStep {
    pub const ALL: [LoadStep; 4] = [LoadStep::Download, LoadStep::Map, LoadStep::Models, LoadStep::Physics];

    /// The stage as a number (0-3), for the network.
    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(i: u8) -> Option<LoadStep> {
        Self::ALL.get(i as usize).copied()
    }

    /// The stage's words.
    pub fn words(self) -> &'static str {
        match self {
            LoadStep::Download => "Downloading",
            LoadStep::Map => "Loading map",
            LoadStep::Models => "Building models",
            LoadStep::Physics => "Physics and navigation",
        }
    }

    /// The stage's short word (a lobby row's bar).
    pub fn short(self) -> &'static str {
        match self {
            LoadStep::Download => "Downloading",
            LoadStep::Map => "Loading",
            LoadStep::Models => "Models",
            LoadStep::Physics => "Physics",
        }
    }
}

/// This app's own map load past its loader: the stage (`Models` or
/// `Physics`) and its work done of all (the spawn's steps and props, then
/// its colliders and the settle hooks). Written by the stepped spawn
/// (`spawn::begin_spawn`, `spawn_steps`, `insert_colliders`) and a front
/// end's settling; reset when a load starts. Read only while a local load
/// is under way (`map::local_load_under_way`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadWork {
    pub step: LoadStep,
    pub done: u64,
    pub total: u64,
    /// The colliders the physics stage began with (its settle hooks come
    /// after them).
    pub colliders: u64,
}

impl LoadWork {
    /// The share done (0 while nothing is known).
    pub fn fraction(&self) -> f32 {
        if self.total == 0 { 0.0 } else { (self.done as f64 / self.total as f64).clamp(0.0, 1.0) as f32 }
    }
}

/// Where a load's time goes: a loader calls `lap` after each stage; the
/// stages land in `MapData::load_times` (and the log), for finding what
/// makes a map slow to load (docs/OBSERVABILITY.md, "Load times").
pub struct LoadTimer {
    start: Instant,
    last: Instant,
    pub stages: Vec<(&'static str, f32)>,
}

impl Default for LoadTimer {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last: now,
            stages: Vec::new(),
        }
    }
}

impl LoadTimer {
    /// `stage` ended now: the seconds since the last lap.
    pub fn lap(&mut self, stage: &'static str) {
        let now = Instant::now();
        self.stages.push((stage, (now - self.last).as_secs_f32()));
        self.last = now;
    }

    /// Seconds since the start.
    pub fn total(&self) -> f32 {
        self.start.elapsed().as_secs_f32()
    }

    /// One line: the total, then the stages over 50 ms, slowest first.
    pub fn summary(&self) -> String {
        let mut slow: Vec<_> = self.stages.iter().filter(|s| s.1 >= 0.05).collect();
        slow.sort_by(|a, b| b.1.total_cmp(&a.1));
        let parts: Vec<String> = slow.iter().map(|(n, t)| format!("{n} {t:.2}")).collect();
        format!("{:.2} s ({})", self.total(), parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two apps in one process: each load's progress is its own.
    #[test]
    fn reporters_are_separate() {
        let (a, b) = (LoadReporter::default(), LoadReporter::default());
        assert_ne!(a, b);
        let ta = {
            let a = a.clone();
            std::thread::spawn(move || a.report(0.5, "a"))
        };
        let tb = {
            let b = b.clone();
            std::thread::spawn(move || b.report(2.0, "b"))
        };
        ta.join().unwrap();
        tb.join().unwrap();
        assert_eq!(a.current(), Some(LoadProgress { fraction: 0.5, stage: "a" }));
        assert_eq!(b.current(), Some(LoadProgress { fraction: 1.0, stage: "b" }));
        assert_eq!(a.clone(), a);
        let mut w = bevy::prelude::World::new();
        w.insert_resource(a.clone());
        assert_eq!(w.resource::<LoadReporter>().current(), a.current());
    }
}
