//! Playing a slice of a big map (docs/plans/active/arena-slices.md, A1):
//! the host's `arena` console command, the party apps' only (never a
//! game's own app). The slices are the games' data (`chimera_sim::
//! minigame::arena`, Halo's data/arena_slices.toml); what playing one does
//! is `chimera_world::map::play_area`'s, sent to clients with the match
//! (`chimera_net::minigame_net`, `NetMinigame::arena`).
//!
//! - `arena <slice>` (`halo_ce:bloodgulch@west_base`, `bloodgulch@west_base`,
//!   or the id alone when one map has it): the slice from now on, on its
//!   map. Already on that map with nothing made on it yet (a dedicated
//!   server's first frame: `+map halo_ce:bloodgulch +arena west_base`), the
//!   play area is made at once; otherwise the map is loaded (again):
//!   `changelevel <map>`, which in a party app with a minigame playing is
//!   that minigame's next match there.
//! - `arena off`: the whole map again (the map loaded again if a slice
//!   plays); `arena`: what plays; `arena list`: the slices.
//! - A slice is for its map: another map coming in forgets it
//!   (`forget_on_other_map`), so a rotation or a vote away from it plays
//!   whole maps. A2 makes slices pool entries of their own.
//!
//! The server's (or a standalone game's): a client is told the server
//! picks.

use bevy::prelude::*;
use chimera_core::{
    console::{Console, ConsoleAppExt},
    core::NetRole,
};
use chimera_sim::minigame::arena::{self, ArenaSlices};
use chimera_world::map::{
    nav::NavMesh,
    play_area::{ArenaMap, ArenaPlan, PlayArea},
};

/// The `arena` command and forgetting a slice on another map.
pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.console_command(
            "chimera:arena",
            "arena <map@slice> | arena off | arena list: play a slice of a big map (its wall, spawns and bots held \
             inside), from now on on its map (loaded again unless nothing was made on it yet); off: whole maps.",
            command,
        )
        .add_systems(PreUpdate, forget_on_other_map.run_if(resource_added::<ArenaMap>));
    }
}

/// `arena [what]`.
fn command(world: &mut World, args: &[String]) -> Result<Option<String>, String> {
    if world.get_resource::<NetRole>() == Some(&NetRole::Client) {
        return Err("the server picks the arena; you are a client of it".into());
    }
    match args.first().map(String::as_str) {
        None => Ok(Some(match world.get_resource::<PlayArea>() {
            Some(a) => format!(
                "playing {} ({}): {:.0} m2, {} put back inside so far",
                a.slice.key(),
                a.slice.name,
                a.polygon.area(),
                a.put_backs
            ),
            None => "playing the whole map".into(),
        })),
        Some("list") => {
            let all = world.get_resource::<ArenaSlices>().map(|s| s.0.clone()).unwrap_or_default();
            if all.is_empty() {
                return Ok(Some("no arena slices".into()));
            }
            Ok(Some(
                all.iter()
                    .map(|d| {
                        let area = d.slice.polygon().map_or(0.0, |p| p.area());
                        let players = d.players.map_or(String::new(), |[lo, hi]| format!(", {lo} to {hi} players"));
                        format!("{} ({}): {area:.0} m2{players}", d.slice.key(), d.slice.name)
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            ))
        }
        Some("off") => {
            let playing = world.get_resource::<PlayArea>().map(|a| a.slice.map.clone());
            arena::plan(world, None)?;
            match playing {
                Some(map) => {
                    world.resource_mut::<Console>().submit(format!("changelevel {map}"));
                    Ok(Some(format!("whole maps again: {map} loads again")))
                }
                None => Ok(Some("whole maps".into())),
            }
        }
        Some(name) => {
            let slice = arena::plan(world, Some(name))?.ok_or("no slice")?;
            let here = world.get_resource::<ArenaMap>().map(|m| m.0.clone());
            let fresh = !world.contains_resource::<chimera_sim::minigame::spawns::SpawnSpots>()
                && !world.contains_resource::<NavMesh>();
            if here.as_deref().is_some_and(|m| m.eq_ignore_ascii_case(&slice.map)) && fresh {
                chimera_world::map::play_area::follow_plan(world);
                return Ok(Some(format!("playing {} ({}) here", slice.key(), slice.name)));
            }
            world.resource_mut::<Console>().submit(format!("changelevel {}", slice.map));
            Ok(Some(format!("playing {} ({}): {} loads", slice.key(), slice.name, slice.map)))
        }
    }
}

/// Another map in the world: the slice planned for a map is forgotten
/// (the server's).
fn forget_on_other_map(here: Res<ArenaMap>, role: Option<Res<NetRole>>, plan: Option<ResMut<ArenaPlan>>) {
    if role.is_some_and(|r| *r == NetRole::Client) {
        return;
    }
    let Some(mut plan) = plan else { return };
    if plan.0.as_ref().is_some_and(|s| !s.map.eq_ignore_ascii_case(&here.0)) {
        info!("arena: {} is not the slice's map: whole maps again", here.0);
        plan.0 = None;
    }
}
