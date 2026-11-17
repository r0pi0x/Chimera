//! This machine's way into a map as stages (docs/plans/active/
//! lobby-between-maps.md): what the loading screens' bars and the
//! pre-match lobby's rows show, from the work itself, not the time.
//!
//! - **Download** (bytes): the join's or the level change's download
//!   (`client::JoinProgress::download`), or the next map's files fetched
//!   ahead during the match-end screen (`net::assets::MapAhead`). Listed
//!   only when this load has one.
//! - **Map**: the loader's task (`map::loading::LoadReporter`'s share).
//! - **Models**: the stepped spawn's parts and props
//!   (`map::loading::LoadWork`).
//! - **Physics**: the spawn's colliders, then the settle hooks
//!   (navigation, spawn spots, shadows; `LoadWork`).
//!
//! `stages` lists them, each done, running (with its share when known)
//! or pending; `current` is the running one, which a client reports to
//! its server (`prematch::LobbyReport::step`, `permille`).

use bevy::prelude::*;
pub use chimera_world::map::loading::LoadStep;
pub use chimera_world::map::loading::LoadWork;
use chimera_world::map::loading::LoadReporter;

use crate::net::client::{JoinProgress, JoinStage};

/// A stage's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StageState {
    Pending,
    /// Under way: its share done (None: not known yet).
    Running(Option<f32>),
    Done,
}

/// One stage now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage {
    pub step: LoadStep,
    pub state: StageState,
    /// The download's bytes done and in all (0: not known).
    pub bytes: Option<(u64, u64)>,
    /// The download's files done and in all (when the asset service told).
    pub files: Option<(u64, u64)>,
}

/// The running stage and its share (None: not known), if a load is under
/// way here.
pub fn current(world: &World) -> Option<(LoadStep, Option<f32>)> {
    stages(world).into_iter().find_map(|s| match s.state {
        StageState::Running(f) => Some((s.step, f)),
        _ => None,
    })
}

/// What the stages are made from (`stages` reads them from the world; a
/// staged screen makes its own).
#[derive(Clone, Copy, Debug, Default)]
pub struct Inputs<'a> {
    pub join: Option<&'a JoinProgress>,
    /// The next map's files fetched ahead, running: bytes done and in all.
    pub ahead: Option<(u64, u64)>,
    /// This machine's own map load is under way (`map::LocalMapLoad`).
    pub local: bool,
    /// Its spawn's and settling's work (`LoadWork`).
    pub work: Option<LoadWork>,
    /// The loader's share (`LoadReporter`).
    pub map_share: Option<f32>,
}

impl<'a> Inputs<'a> {
    pub fn of(world: &'a World) -> Self {
        let local = world.contains_resource::<chimera_world::map::LocalMapLoad>();
        Inputs {
            join: world.get_resource::<JoinProgress>(),
            ahead: world.get_resource::<crate::net::assets::MapAhead>().filter(|a| a.running()).map(|a| a.bytes()),
            local,
            work: world.get_resource::<LoadWork>().copied().filter(|_| local),
            map_share: world.get_resource::<LoadReporter>().and_then(|r| r.current()).map(|p| p.fraction),
        }
    }
}

/// The stages of the load under way here (empty when none is): the
/// download only when this load has one.
pub fn stages(world: &World) -> Vec<Stage> {
    stages_of(&Inputs::of(world))
}

/// `stages` from its inputs.
pub fn stages_of(i: &Inputs) -> Vec<Stage> {
    let join = i.join.filter(|j| j.failure.is_none());
    let joining = join.is_some_and(|j| !matches!(j.stage, JoinStage::Idle | JoinStage::Joined));
    let (ahead, local) = (i.ahead, i.local);
    if !joining && !local && ahead.is_none() {
        return Vec::new();
    }
    let downloading = join.is_some_and(|j| matches!(j.stage, JoinStage::Verifying | JoinStage::Downloading));
    let downloaded = join.is_some_and(|j| {
        j.download.is_some() || j.log.iter().any(|(s, _)| matches!(s, JoinStage::Verifying | JoinStage::Downloading))
    });
    let (bytes, files) = match (join.and_then(|j| j.download.as_ref()), ahead) {
        (Some(d), _) => (Some((d.done, d.total)), (d.files_total > 0).then_some((d.files_done, d.files_total))),
        (None, Some(a)) => (Some(a), None),
        _ => (None, None),
    };
    let with_download = downloaded || downloading || ahead.is_some();
    // Where it is: the download, the map's stages (this machine's own
    // load), or before them (connecting, a level change not yet loading).
    let (at, share) = if downloading || (ahead.is_some() && !local) {
        let share = bytes.filter(|(_, t)| *t > 0).map(|(d, t)| (d as f64 / t as f64).clamp(0.0, 1.0) as f32);
        (Some(LoadStep::Download), share)
    } else if let Some(w) = i.work {
        match w.step {
            LoadStep::Map | LoadStep::Download => (Some(LoadStep::Map), i.map_share),
            step => (Some(step), (w.total > 0).then(|| w.fraction())),
        }
    } else if local || join.is_some_and(|j| matches!(j.stage, JoinStage::ChangingLevel | JoinStage::LoadingMap)) {
        (Some(LoadStep::Map), if local { i.map_share } else { None })
    } else {
        (None, None)
    };
    LoadStep::ALL
        .into_iter()
        .filter(|s| *s != LoadStep::Download || with_download)
        .map(|step| Stage {
            step,
            state: match at {
                Some(a) if step < a => StageState::Done,
                Some(a) if step == a => StageState::Running(share),
                _ => StageState::Pending,
            },
            bytes: (step == LoadStep::Download).then_some(bytes).flatten(),
            files: (step == LoadStep::Download).then_some(files).flatten(),
        })
        .collect()
}

/// A share in thousandths (the network's form).
pub fn permille(share: Option<f32>) -> u16 {
    share.map_or(0, |f| (f.clamp(0.0, 1.0) * 1000.0).round() as u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::client::DownloadInfo;

    fn kinds(w: &World) -> Vec<(LoadStep, StageState)> {
        stages(w).into_iter().map(|s| (s.step, s.state)).collect()
    }

    #[test]
    fn nothing_loading_has_no_stages() {
        let mut w = World::new();
        assert!(stages(&w).is_empty());
        w.insert_resource(JoinProgress { stage: JoinStage::Joined, ..default() });
        assert!(stages(&w).is_empty());
    }

    #[test]
    fn a_join_downloads_then_loads_each_stage_in_turn() {
        use StageState::*;
        let mut w = World::new();
        let mut j = JoinProgress { stage: JoinStage::Downloading, ..default() };
        j.download = Some(DownloadInfo { file: "files".into(), from: "asset service".into(), done: 25, total: 100, files_done: 1, files_total: 4 });
        w.insert_resource(j);
        let s = stages(&w);
        assert_eq!(s.len(), 4, "the download and the map's three");
        assert_eq!((s[0].step, s[0].state, s[0].bytes, s[0].files), (LoadStep::Download, Running(Some(0.25)), Some((25, 100)), Some((1, 4))));
        assert!(s[1..].iter().all(|s| s.state == Pending));
        // The map's load on its task: the loader's share.
        {
            let mut j = w.resource_mut::<JoinProgress>();
            j.stage = JoinStage::LoadingMap;
            j.log.push((JoinStage::Downloading, 0.0));
            j.download = None;
        }
        w.insert_resource(chimera_world::map::LocalMapLoad { map: "test:m".into() });
        w.insert_resource(LoadWork::default());
        let r = LoadReporter::default();
        r.report(0.4, "LoadingProgress_LoadMap");
        w.insert_resource(r);
        assert_eq!(kinds(&w), vec![(LoadStep::Download, Done), (LoadStep::Map, Running(Some(0.4))), (LoadStep::Models, Pending), (LoadStep::Physics, Pending)]);
        // The spawn, then the physics.
        w.insert_resource(LoadWork { step: LoadStep::Models, done: 3, total: 12, colliders: 0 });
        assert_eq!(current(&w), Some((LoadStep::Models, Some(0.25))));
        w.insert_resource(LoadWork { step: LoadStep::Physics, done: 9, total: 10, colliders: 8 });
        assert_eq!(current(&w), Some((LoadStep::Physics, Some(0.9))));
        assert_eq!(permille(Some(0.9)), 900);
    }

    #[test]
    fn a_hosts_own_load_has_no_download() {
        let mut w = World::new();
        w.insert_resource(chimera_world::map::LocalMapLoad { map: "test:m".into() });
        assert_eq!(stages(&w).iter().map(|s| s.step).collect::<Vec<_>>(), vec![LoadStep::Map, LoadStep::Models, LoadStep::Physics]);
        assert_eq!(current(&w), Some((LoadStep::Map, None)));
    }
}
