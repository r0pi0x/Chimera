//! What a report (F10, `feedback`) says about the world, so one report
//! names a stutter's or a leak's cause (docs/plans/active/
//! feedback-reports.md, 2.3):
//!
//! - `world.txt` (`world_text`): a census (`metrics::census`: entities by
//!   kind, archetypes, systems, asset stores), the growth by kind since
//!   the session's first map and since this map (`census::Light`
//!   baselines taken 5 s after each map loads, cheap: non-empty
//!   archetypes only), the archetypes made since the map and by which
//!   inserts and removals (`made_since`), what comes and goes on the
//!   characters (`varies`), memory by kind (`chimera_mem`'s readout:
//!   the process's mappings, malloc, the GPU allocator, image and mesh
//!   stores), and the frame inspector's timeline of the last minutes'
//!   events (`frame_inspector::Inspector::timeline_text`).
//! - `game.txt`'s setup lines (`setup_text`): the role (offline, listen
//!   host, client), players and bots, the tick rate, the minigame, the
//!   local player's rung of the ladder and the games its rungs deal; and
//!   what is in the local player's hand (`hand_text`): the weapon, its
//!   draw, the view model's sequence and pose, and the sounds the player
//!   makes now (entry, channel, age), so a gun's sound or animation that
//!   goes on after a swap shows in the report.
//! - Events for the timeline (`frame_events`): a map loaded, a bot joined
//!   or left, the local rung and the minigame changed.
//!
//! Known shortcuts: "players" counts characters (with `Health`), bots
//! included; the baselines are the main world's only.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use chimera_core::metrics::{census, frame_events};

pub struct WorldReportPlugin;

impl Plugin for WorldReportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Baselines>().add_systems(
            Last,
            (watch_map, take_baseline.run_if(|b: Res<Baselines>| b.pending.as_ref().is_some_and(|(_, at)| Instant::now() >= *at)), watch_play),
        );
    }
}

/// The census baselines: the session's (its first map's) and this map's,
/// each with its map's name.
#[derive(Resource, Default)]
pub struct Baselines {
    pub session: Option<(String, census::Light)>,
    pub map: Option<(String, census::Light)>,
    /// A map loaded: take its baseline at this time.
    pending: Option<(String, Instant)>,
    /// The map name last seen.
    seen: String,
}

/// How long after a map's load its baseline is taken (spawning settled).
const SETTLE: Duration = Duration::from_secs(5);

fn watch_map(name: Option<Res<crate::map::LoadedMapName>>, mut b: ResMut<Baselines>) {
    let name = name.as_ref().map_or("", |n| n.0.as_str());
    if name != b.seen {
        if !name.is_empty() {
            frame_events::note("map", format!("map {name} loaded"));
        }
        b.pending = (!name.is_empty()).then(|| (name.to_string(), Instant::now() + SETTLE));
        b.seen = name.to_string();
    }
}

/// A loaded map's baseline, once settled (and the session's, the first).
fn take_baseline(world: &World, mut commands: Commands) {
    let light = census::Light::take(world);
    commands.queue(move |w: &mut World| {
        let mut b = w.resource_mut::<Baselines>();
        let Some((name, _)) = b.pending.take() else { return };
        if b.session.is_none() {
            b.session = Some((name.clone(), light.clone()));
        }
        b.map = Some((name, light));
    });
}

/// Bots joining and leaving, the local rung and the minigame changing.
fn watch_play(
    added: Query<Option<&Name>, Added<crate::core::Bot>>,
    mut removed: RemovedComponents<crate::core::Bot>,
    rung: Query<&chimera_sim::minigame::ladder::Rung, (With<crate::core::LocalPlayer>, Changed<chimera_sim::minigame::ladder::Rung>)>,
    minigame: Option<Res<chimera_sim::minigame::ActiveMinigame>>,
) {
    for name in &added {
        frame_events::note("bots", format!("bot {} joined", name.map_or("?", |n| n.as_str())));
    }
    let gone = removed.read().count();
    if gone > 0 {
        frame_events::note("bots", format!("{gone} bots left"));
    }
    for r in &rung {
        frame_events::note("match", format!("rung {}", r.0 + 1));
    }
    if let Some(m) = minigame.filter(|m| m.is_changed()) {
        frame_events::note("match", format!("minigame {}", m.def.id));
    }
}

/// `world.txt`.
pub fn world_text(world: &mut World) -> String {
    let now = census::Census::take(world);
    let mut out = now.report(20);
    out += "\n\n";
    let light = census::Light::take(world);
    let (session, map) = {
        let b = world.get_resource::<Baselines>();
        (b.and_then(|b| b.session.clone()), b.and_then(|b| b.map.clone()))
    };
    for (title, base) in [("since the session's first map", &session), ("since this map", &map)] {
        match base {
            Some((name, before)) => {
                out += &format!("growth {title} ({name}, 5 s after its load):\n");
                for l in light.growth(before, 15) {
                    out += &format!("{l}\n");
                }
            }
            None => out += &format!("growth {title}: no baseline yet\n"),
        }
    }
    if let Some((_, before)) = &map {
        out += &format!("{}\n", census::made_since(world, before.archetypes_made, 8));
    }
    out += &format!("characters: {}\n", census::varies(&now, "Health"));
    out += "\nmemory:\n";
    out += &super::memory::readout(world, 0);
    out += "\n\n";
    if let Some(i) = world.get_resource::<super::frame_inspector::FrameInspector>()
        && let Ok(i) = i.0.lock()
    {
        out += &i.timeline_text();
    }
    out
}

/// `game.txt`'s setup lines: role, players and bots, tick rate, the
/// minigame, the rung and the games the ladder deals.
pub fn setup_text(world: &mut World) -> String {
    use crate::core::NetRole;
    let role = match world.get_resource::<NetRole>().copied().unwrap_or_default() {
        NetRole::Standalone => "offline",
        NetRole::Server => "listen host",
        NetRole::Client => "client",
    };
    let bots = world.query_filtered::<(), With<crate::core::Bot>>().iter(world).count();
    let characters = world.query_filtered::<(), With<crate::core::Health>>().iter(world).count();
    let mut out = format!("role: {role}; characters {characters}, bots {bots}");
    if let Some(t) = world.get_resource::<Time<Fixed>>() {
        out += &format!("; tick {:.1} Hz", 1.0 / t.timestep().as_secs_f64().max(1e-6));
    }
    out += "\n";
    let rung = world
        .query_filtered::<&chimera_sim::minigame::ladder::Rung, With<crate::core::LocalPlayer>>()
        .iter(world)
        .next()
        .map(|r| r.0);
    if let Some(l) = world.get_resource::<chimera_sim::minigame::ladder::Ladder>()
        && !l.rungs.is_empty()
    {
        let mut games: Vec<(&str, usize)> = Vec::new();
        for r in &l.rungs {
            let g = r.split_once(':').map_or("?", |(g, _)| g);
            match games.iter_mut().find(|(n, _)| *n == g) {
                Some(e) => e.1 += 1,
                None => games.push((g, 1)),
            }
        }
        let games: Vec<String> = games.iter().map(|(g, n)| format!("{g} {n}")).collect();
        out += &format!(
            "ladder: {} rungs{}; games dealt: {}\n",
            l.rungs.len(),
            rung.map_or(String::new(), |r| format!(", on rung {}", r + 1)),
            games.join(", ")
        );
    }
    out
}

/// `game.txt`'s hand lines (module doc): the local player's weapon in
/// hand, its draw, its view model's sequence and pose, the sounds the
/// player makes.
pub fn hand_text(world: &mut World) -> String {
    use chimera_sim::weapon::{Inventory, Weapon, view_anim::ViewPose};
    let now = world.get_resource::<Time>().map_or(0.0, |t| t.elapsed_secs_f64());
    let tick = world.get_resource::<crate::core::SimClock>().map_or(0.0, |c| c.now);
    let Some((player, inv)) = world
        .query_filtered::<(Entity, &Inventory), With<crate::core::LocalPlayer>>()
        .iter(world)
        .next()
        .map(|(e, i)| (e, i.clone()))
    else {
        return String::new();
    };
    let mut out = String::new();
    let held = inv.active.and_then(|w| world.get::<Weapon>(w).map(|x| (w, x.id)));
    match held {
        Some((w, id)) => {
            let pose = world.get::<ViewPose>(w).map_or(0, |p| p.0);
            out += &format!(
                "in hand: {id} ({} carried), ready in {:.3} s, view pose {pose}",
                inv.weapons.len(),
                (inv.next_attack - tick).max(0.0)
            );
        }
        None => out += &format!("in hand: nothing ({} carried)", inv.weapons.len()),
    }
    if let Some(v) = world.get::<crate::map::ViewAnimator>(player) {
        let cycle = v.animator.as_ref().map_or(0.0, |a| a.cycle);
        out += &format!("; view model {} at cycle {cycle:.2}", v.sequence().unwrap_or("-"));
    }
    out += "\n";
    let mut sounds: Vec<(f64, String)> = world
        .query::<&chimera_present::audio::sound::Playing>()
        .iter(world)
        .filter(|p| p.source() == Some(player) && p.sounding(now))
        .map(|p| (p.started(), format!("{} (channel {}, {:.1} s)", p.entry(), p.channel().map_or("-".into(), |c| c.to_string()), now - p.started())))
        .collect();
    sounds.sort_by(|a, b| a.0.total_cmp(&b.0));
    let sounds: Vec<String> = sounds.into_iter().map(|s| s.1).collect();
    out += &format!("sounds from the player: {}\n", if sounds.is_empty() { "none".to_string() } else { sounds.join(", ") });
    out
}
