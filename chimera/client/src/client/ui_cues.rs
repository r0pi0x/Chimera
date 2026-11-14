//! The shell's screens' UI cues (docs/plans/active/ugg-ui.md, "UI
//! sound"): what the lobby, the HUD and the match end send to the `ui`
//! bus (`ui_sound`), read off their state as it changes, so no screen's
//! drawing code changes. Runs only while an app's sounds are in
//! (`ui_sound::UiSounds`); `ui_sound` plays them only under a theme with
//! UI sound on, so other apps hear nothing.
//!
//! - **Lobby** (`prematch_lobby::LobbyView`): your row turning ready
//!   (`lobby_ready`) or back (`lobby_unready`), another player readying
//!   (`lobby_peer`, once a frame), the countdown's seconds (`count_tick`
//!   from 10 to 4, `count_tick_3`, `_2`, `_1`), and its end (`count_go`:
//!   0 shown, or the lobby closing from 1).
//! - **Rank-up** (the local player's `Rung` on the `Ladder`): a rung up
//!   plays `rank_up` at p = rung / (rungs - 1) (`rank_progress`), the
//!   last rung `rank_knife`; a rung up within `CHAIN_SECS` of the last
//!   layers `rank_chain` over it.
//! - **Tab** (`race_chip::BoardOpen`): `tab_open`, `tab_close`.
//! - **Kill feed**: your own kill entering it (`feed_self`).
//! - **Match end** (`match_end::MatchEndView`): the screen coming up with
//!   a winner (`end_reveal`); the map vote: your vote (`vote_cast_1` to
//!   `_4` by card), someone else's (`vote_other`), its last 10 seconds
//!   (`count_tick`) and its close (`vote_lock`).
//!
//! The logo pass's `pass_sweep` is sent by the pass itself (M1's
//! `logo_pass`, where a pass begins); the menus' cues by the app's menus
//! (`party_kit`), the air strike's by the app.

use bevy::prelude::*;
use chimera_net::prematch::status;
use chimera_sim::minigame::{
    ladder::{Ladder, Rung},
    vote::MapVote,
};
use chimera_ui::UiCue;

use super::{
    kill_feed::KillFeed,
    match_end::MatchEndView,
    prematch_lobby::LobbyView,
    race_chip::BoardOpen,
    ui_sound::UiSounds,
};
use crate::core::{LocalPlayer, SimClock};

/// A rung up this soon after the last one chains (the toast's 1.15 s).
pub const CHAIN_SECS: f64 = 1.15;

/// Sends the screens' cues (module doc).
pub struct UiCuesPlugin;

impl Plugin for UiCuesPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<Messages<UiCue>>() {
            app.add_message::<UiCue>();
        }
        app.add_systems(
            Update,
            (lobby, rank, tab, feed, match_end).run_if(resource_exists::<UiSounds>),
        );
    }
}

/// The rank-up's progress for reaching `rung` (0 the first gun) on a
/// ladder of `rungs`: rung / (rungs - 1).
pub fn rank_progress(rung: usize, rungs: usize) -> f32 {
    if rungs < 2 {
        return 1.0;
    }
    (rung as f32 / (rungs - 1) as f32).clamp(0.0, 1.0)
}

/// The cues of a rung up to `rung` on a ladder of `rungs` (`chained`: within
/// `CHAIN_SECS` of the last).
pub fn rank_cues(rung: usize, rungs: usize, chained: bool) -> Vec<UiCue> {
    let mut out = vec![if rungs >= 2 && rung + 1 >= rungs {
        UiCue::new("rank_knife")
    } else {
        UiCue::at("rank_up", rank_progress(rung, rungs))
    }];
    if chained {
        out.push(UiCue::new("rank_chain"));
    }
    out
}

/// The cues between two states of the lobby.
pub fn lobby_cues(prev: &LobbyView, next: &LobbyView) -> Vec<UiCue> {
    let mut out = Vec::new();
    if next.shown && prev.shown {
        let was = |name: &str, local: bool| {
            prev.rows.iter().find(|r| r.local == local && (local || r.name == name)).map(|r| r.status)
        };
        let mut peer = false;
        for r in &next.rows {
            let before = was(&r.name, r.local);
            if r.local {
                if r.status == status::READY && before == Some(status::NOT_READY) {
                    out.push(UiCue::new("lobby_ready"));
                } else if r.status == status::NOT_READY && before == Some(status::READY) {
                    out.push(UiCue::new("lobby_unready"));
                }
            } else if r.status == status::READY && before.is_none_or(|b| b != status::READY) && !r.bot {
                peer = true;
            }
        }
        if peer {
            out.push(UiCue::new("lobby_peer"));
        }
    }
    if next.shown && next.countdown != prev.countdown {
        match next.countdown {
            Some(0) => out.push(UiCue::new("count_go")),
            Some(1) => out.push(UiCue::new("count_tick_1")),
            Some(2) => out.push(UiCue::new("count_tick_2")),
            Some(3) => out.push(UiCue::new("count_tick_3")),
            Some(n) if n <= 10 => out.push(UiCue::new("count_tick")),
            _ => {}
        }
    }
    if prev.shown && !next.shown && matches!(prev.countdown, Some(1)) {
        out.push(UiCue::new("count_go"));
    }
    out
}

fn lobby(view: Option<Res<LobbyView>>, mut prev: Local<LobbyView>, mut out: MessageWriter<UiCue>) {
    let Some(view) = view else { return };
    if *view == *prev {
        return;
    }
    out.write_batch(lobby_cues(&prev, &view));
    *prev = view.clone();
}

#[derive(Default)]
struct RankSeen {
    rungs: usize,
    rung: Option<usize>,
    last_up: Option<f64>,
}

fn rank(
    ladder: Option<Res<Ladder>>,
    me: Query<&Rung, With<LocalPlayer>>,
    time: Res<Time<Real>>,
    mut seen: Local<RankSeen>,
    mut out: MessageWriter<UiCue>,
) {
    let rungs = ladder.map_or(0, |l| l.rungs.len());
    let rung = me.iter().next().map(|r| r.0);
    if rungs != seen.rungs {
        // A new ladder: no rank-up for where it starts.
        *seen = RankSeen { rungs, rung, last_up: None };
        return;
    }
    if let (Some(was), Some(now)) = (seen.rung, rung)
        && now > was
    {
        let t = time.elapsed_secs_f64();
        let chained = seen.last_up.is_some_and(|l| t - l < CHAIN_SECS);
        out.write_batch(rank_cues(now, rungs, chained));
        seen.last_up = Some(t);
    }
    seen.rung = rung;
}

fn tab(open: Option<Res<BoardOpen>>, mut prev: Local<bool>, mut out: MessageWriter<UiCue>) {
    let now = open.is_some_and(|o| o.0);
    if now != *prev {
        out.write(UiCue::new(if now { "tab_open" } else { "tab_close" }));
        *prev = now;
    }
}

fn feed(feed: Option<Res<KillFeed>>, mut last: Local<Option<u64>>, mut out: MessageWriter<UiCue>) {
    let Some(feed) = feed else { return };
    let newest = feed.entries.iter().map(|e| e.id).max();
    let Some(newest) = newest else { return };
    if let Some(seen) = *last {
        let mine = feed
            .entries
            .iter()
            .any(|e| e.id > seen && !e.suicide && e.killer.as_ref().is_some_and(|k| k.local) && !e.victim.local);
        if mine {
            out.write(UiCue::new("feed_self"));
        }
    }
    *last = Some(newest.max(last.unwrap_or(0)));
}

#[derive(Default)]
struct EndSeen {
    view: MatchEndView,
    vote_left: Option<i64>,
}

/// The cues between two states of the match-end screen.
pub fn match_end_cues(prev: &MatchEndView, next: &MatchEndView) -> Vec<UiCue> {
    const CAST: [&str; 4] = ["vote_cast_1", "vote_cast_2", "vote_cast_3", "vote_cast_4"];
    let mut out = Vec::new();
    if next.shown && !prev.shown && next.winner.is_some() {
        out.push(UiCue::new("end_reveal"));
    }
    if let Some(v) = next.vote.as_ref().filter(|_| next.shown) {
        let before = prev.vote.as_ref().filter(|_| prev.shown);
        let mine_before = before.and_then(|b| b.mine);
        if let Some(m) = v.mine
            && mine_before != Some(m)
        {
            out.push(UiCue::new(CAST[m.min(3)]));
        }
        if let Some(b) = before {
            let total = |c: &[u32]| c.iter().sum::<u32>();
            let mine_moved = v.mine != b.mine;
            // Someone else's vote: the count grew more than your own did.
            let grew = total(&v.counts) as i64 - total(&b.counts) as i64;
            let own = i64::from(mine_moved && b.mine.is_none());
            if grew > own {
                out.push(UiCue::new("vote_other"));
            }
            if b.winner.is_none() && v.winner.is_some() {
                out.push(UiCue::new("vote_lock"));
            }
        }
    }
    out
}

fn match_end(
    view: Option<Res<MatchEndView>>,
    vote: Option<Res<MapVote>>,
    clock: Option<Res<SimClock>>,
    mut seen: Local<EndSeen>,
    mut out: MessageWriter<UiCue>,
) {
    let Some(view) = view else { return };
    if *view != seen.view {
        out.write_batch(match_end_cues(&seen.view, &view));
        seen.view = view.clone();
    }
    // The vote's last 10 seconds tick.
    let left = match (view.shown, view.vote.as_ref(), vote, clock) {
        (true, Some(v), Some(m), Some(c)) if v.open() => Some((m.closes - c.now).ceil() as i64),
        _ => None,
    };
    if left != seen.vote_left {
        if let Some(n) = left
            && (1..=10).contains(&n)
            && seen.vote_left.is_some()
        {
            out.write(UiCue::new("count_tick"));
        }
        seen.vote_left = left;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(rung: usize, rungs: usize) -> String {
        let t = super::super::ui_sound::SoundTable::from_toml(
            "[event.rank_up]\nfile = \"rank_p{nn}\"\ngrid = 16\n[event.rank_knife]\nfile = \"rank_14_knife\"\n",
        )
        .unwrap();
        let c = &rank_cues(rung, rungs, false)[0];
        t.events[c.event.as_ref()].file_for(c.value)
    }

    #[test]
    fn rank_up_picks_the_nearest_render() {
        // 8 rungs: p = r / 7.
        assert_eq!(file(1, 8), "rank_p02"); // 2.29
        assert_eq!(file(3, 8), "rank_p07"); // 6.86
        assert_eq!(file(6, 8), "rank_p14"); // 13.71
        assert_eq!(file(7, 8), "rank_14_knife");
        // 14 rungs: p = r / 13.
        assert_eq!(file(1, 14), "rank_p01"); // 1.23
        assert_eq!(file(6, 14), "rank_p07"); // 7.38
        assert_eq!(file(12, 14), "rank_p15"); // 14.77
        assert_eq!(file(13, 14), "rank_14_knife");
        // 30 rungs: p = r / 29.
        assert_eq!(file(1, 30), "rank_p01"); // 0.55
        assert_eq!(file(15, 30), "rank_p08"); // 8.28
        assert_eq!(file(28, 30), "rank_p15"); // 15.45
        assert_eq!(file(29, 30), "rank_14_knife");
        assert_eq!(rank_cues(3, 8, true).len(), 2, "a chain layers rank_chain");
    }
}
