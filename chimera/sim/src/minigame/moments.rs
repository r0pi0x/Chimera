//! What a playing minigame's match is for the local player, frame by
//! frame, for a game's music director (docs/DESIGN.md, "Announcers"):
//! `MinigameMoments`, rebuilt every frame on every machine that shows the
//! match (a network client from what it is sent), naming no game. The
//! director of the game that plays the match's music (`music`: the
//! definition's `music`, else the first game whose parts lend a music
//! director, `GameParts::music`) turns its changes into its cues; Combat
//! Arms' does (`combat_arms::game::music::director`, its party cues:
//! specs/combat_arms/music_ambience.md Section 10). The moments:
//!
//! - the match's phase (`MomentPhase`: waiting, live, over): the start of
//!   play is the phase going live, while the local player is `alive` or
//!   not;
//! - how many players are on the ladder's last rung (`last_rung`; 0 off a
//!   ladder), from the replicated rungs;
//! - the respawn countdown's whole seconds left (`respawn_in`, from
//!   `core::RespawnDue`);
//! - the decided match for the local player (`result`): won, lost, a
//!   draw.
//!
//! While this machine's own map load is under way
//! (`map::local_load_under_way`: from its start, or a network client's
//! join or the server's change of level, until the game is entered behind
//! the loading screen) the moments hold still (a live match's held as
//! waiting, no respawn countdown), so a match that goes live meanwhile
//! starts for the director (its start cue) only once the local player is
//! in.
//!
//! The callouts' moments (a rung up, the last rung's call) sound through
//! the cue packs (`chimera_core::cues`), not here.

use bevy::prelude::*;

use super::{ActiveMinigame, MatchResults, ladder};
use crate::{
    core::{Dead, Health, LocalPlayer, RespawnDue, SimClock},
    mode::{Match, Phase},
};
use chimera_core::{game_parts::PartsRegistry, query_cache::cached_query};

/// The match's phase as a director sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MomentPhase {
    /// Waiting for players (or about to start).
    #[default]
    Waiting,
    /// Live: kills count.
    Live,
    /// The match is decided or over.
    Over,
}

/// The local player's view of the playing match (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct MinigameMoments {
    /// A minigame plays here.
    pub playing: bool,
    /// The game whose music director plays the match (None: none does).
    pub music: Option<&'static str>,
    pub phase: MomentPhase,
    /// The local player lives.
    pub alive: bool,
    /// Players on the ladder's last rung.
    pub last_rung: u32,
    /// Whole seconds until the dead local player comes back.
    pub respawn_in: Option<u32>,
    /// The decided match for the local player: `Some(Some(true))` won,
    /// `Some(Some(false))` lost, `Some(None)` a draw.
    pub result: Option<Option<bool>>,
}

/// The game that plays a match of `def_music`'s choice: a game whose
/// parts lend a music director by id, `none`, or None the first that
/// lends one.
pub fn music_of(parts: Option<&PartsRegistry>, choice: Option<&str>) -> Option<&'static str> {
    let parts = parts?;
    match choice {
        Some(c) if c.eq_ignore_ascii_case("none") => None,
        Some(c) => parts.get(c).filter(|p| p.music).map(|p| p.game),
        None => parts.music(),
    }
}

/// `MinigameMoments` for this frame (written only when it changes), at
/// the end of `hud::update` (after its `core::RespawnDue`).
pub fn update(world: &mut World) {
    let Some(active) = world.get_resource::<ActiveMinigame>().copied() else {
        if world.get_resource::<MinigameMoments>().is_some_and(|m| m.playing) {
            world.insert_resource(MinigameMoments::default());
        }
        return;
    };
    // This machine's own map still loading (`map::local_load_under_way`):
    // the moments hold (a live match's as waiting), so no start cue sounds
    // behind the loading screen; play's start comes once the local player
    // is in.
    if crate::map::local_load_under_way(world) {
        if let Some(m) = world.get_resource::<MinigameMoments>()
            && (m.phase == MomentPhase::Live || m.respawn_in.is_some())
        {
            let held = MinigameMoments {
                phase: if m.phase == MomentPhase::Live { MomentPhase::Waiting } else { m.phase },
                respawn_in: None,
                ..m.clone()
            };
            world.insert_resource(held);
        }
        return;
    }
    let me = chimera_core::query_cache::first::<With<LocalPlayer>>(world);
    let alive =
        me.is_some_and(|e| world.get::<Dead>(e).is_none() && world.get::<Health>(e).is_some_and(|h| h.current > 0.0));
    let phase = match world.get_resource::<Match>().map(|m| m.phase) {
        Some(Phase::Live) => MomentPhase::Live,
        Some(Phase::RoundOver { .. } | Phase::Intermission { .. } | Phase::MatchOver) => MomentPhase::Over,
        _ => MomentPhase::Waiting,
    };
    let rungs = world.get_resource::<ladder::Ladder>().map_or(0, |l| l.rungs.len());
    let last_rung = if rungs == 0 {
        0
    } else {
        cached_query::<&ladder::Rung, With<super::InMinigame>, _>(world, |q, w| {
            q.iter(w).filter(|r| r.0 + 1 == rungs).count() as u32
        })
    };
    let now = world.get_resource::<SimClock>().map_or(0.0, |c| c.now);
    let respawn_in = world
        .get_resource::<RespawnDue>()
        .and_then(|d| d.0)
        .map(|d| (d.at - now).ceil())
        .filter(|s| *s >= 1.0)
        .map(|s| s as u32);
    let result = world.get_resource::<MatchResults>().map(|r| {
        let winner = r.rows.iter().find(|row| row.winner);
        winner.map(|w| w.player.is_some_and(|p| Some(p) == me))
    });
    let moments = MinigameMoments {
        playing: true,
        music: music_of(world.get_resource::<PartsRegistry>(), active.def.music.as_deref()),
        phase,
        alive,
        last_rung,
        respawn_in,
        result,
    };
    if world.get_resource::<MinigameMoments>() != Some(&moments) {
        world.insert_resource(moments);
    }
}
