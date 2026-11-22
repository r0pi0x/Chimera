//! `playdemo <file.dem>` (and `--playdemo <file>` on the command line):
//! a CS:S demo watched in our game, as CS:S's `playdemo` plays one. The
//! file is read (`game::demo::load`; a bare name is looked for in the
//! install's `cstrike/` too, `.dem` added when missing), its map loaded
//! as the `map` command loads one (bots kicked first), then it plays
//! (`game::demo::start`): the local player watches as a spectator, the
//! camera on the first player in first person; CS:S's spectator keys
//! (jump: first person, chase, free look; attack/attack2: next and
//! previous player; `spec_mode`, `spec_next`, `spec_prev`, `spec_player`)
//! pick the view. `demo_timescale`, `demo_pause`/`demo_resume`/
//! `demo_togglepause`, `demo_gototick`, `demo_info` and `stopdemo` are the
//! simulation's (`game::demo`). Chat and radio lines from the demo go to
//! the chat area.

use std::sync::Arc;

use bevy::prelude::*;

use crate::{
    client::{
        chat::ChatLine,
        console::start_map_load,
        spectate::{SpecMode, Spectator},
    },
    console::{Console, ConsoleAppExt},
    core::{MapLoads, NetRole},
    game::demo::{self, CsDemo, DemoPlayer, DemoText},
    map::LoadedMapName,
};

pub struct DemoUiPlugin;

impl Plugin for DemoUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (start_when_loaded, follow_a_player, chat_lines))
            .console_command(
                "playdemo",
                "playdemo <file.dem>: watch a CS:S demo (its map loads first). A bare name is looked for in the \
                 install's cstrike/ folder too. demo_timescale, demo_pause, demo_resume, demo_gototick, stopdemo.",
                |w, a| {
                    let name = a.first().ok_or("playdemo <file.dem>")?;
                    let path = find(name).ok_or_else(|| format!("playdemo: no demo file {name}"))?;
                    let demo = Arc::new(demo::load(&path)?);
                    let map = demo.map().to_string();
                    let game = crate::game_registry::default_game(w).ok_or("no game registered")?;
                    let id = game.map_id(&map);
                    let msg = format!("playing {} on {map}", path.display());
                    if w.get_resource::<NetRole>() == Some(&NetRole::Client) {
                        crate::net::disconnect(w, crate::net::client::BY_USER);
                    }
                    w.resource_mut::<Console>().submit("bot_kick");
                    let loaded = w.get_resource::<LoadedMapName>().is_some_and(|m| m.0 == id)
                        && w.get_resource::<crate::map::MapFile>().is_some();
                    if loaded {
                        w.insert_resource(PendingDemo { demo, loads: None });
                    } else {
                        let loads = w.get_resource::<MapLoads>().map_or(0, |l| l.0);
                        start_map_load(w, &id, None);
                        w.insert_resource(PendingDemo {
                            demo,
                            loads: Some(loads),
                        });
                    }
                    Ok(Some(msg))
                },
            );
    }
}

/// A demo waiting for its map: it starts once more maps have loaded than
/// `loads` (None: the map is there already).
#[derive(Resource)]
struct PendingDemo {
    demo: Arc<CsDemo>,
    loads: Option<u32>,
}

/// The demo file `name` names: as given, with `.dem` added, or in the
/// install's `cstrike/` folder.
fn find(name: &str) -> Option<std::path::PathBuf> {
    let mut names = vec![std::path::PathBuf::from(name)];
    if !name.to_lowercase().ends_with(".dem") {
        names.push(format!("{name}.dem").into());
    }
    let install = crate::game::INFO
        .install()
        .map(|p| p.join("cstrike"));
    let more: Vec<std::path::PathBuf> = install
        .iter()
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .collect();
    names.into_iter().chain(more).find(|p| p.is_file())
}

/// Start the pending demo once its map is in.
fn start_when_loaded(world: &mut World) {
    let Some(pending) = world.get_resource::<PendingDemo>() else {
        return;
    };
    let loads = world.get_resource::<MapLoads>().map_or(0, |l| l.0);
    if pending.loads.is_some_and(|before| loads <= before) {
        return;
    }
    let pending = world.remove_resource::<PendingDemo>().expect("checked");
    demo::start(world, pending.demo);
    world.insert_resource(FollowAPlayer);
}

/// The camera goes to a player in first person when the first ones are in.
#[derive(Resource)]
struct FollowAPlayer;

fn follow_a_player(
    follow: Option<Res<FollowAPlayer>>,
    player: Option<Res<DemoPlayer>>,
    names: Query<&Name>,
    mut spec: ResMut<Spectator>,
    mut commands: Commands,
) {
    let (Some(_), Some(player)) = (follow, player) else {
        return;
    };
    // The first by name, as the spectator's cycle orders them.
    let first = player
        .puppets
        .values()
        .copied()
        .min_by_key(|e| names.get(*e).map(|n| crate::client::spectate::natural(n.as_str())).ok());
    let Some(first) = first else { return };
    spec.pending.set_mode = Some(SpecMode::InEye);
    spec.pending.pick = Some(first);
    commands.remove_resource::<FollowAPlayer>();
}

/// The demo's chat and radio lines in the chat area.
fn chat_lines(mut text: MessageReader<DemoText>, mut lines: MessageWriter<ChatLine>) {
    for t in text.read() {
        lines.write(ChatLine::plain(t.0.clone()));
    }
}
