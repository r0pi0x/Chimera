//! Party networking (docs/plans/active/architecture-review.md L3; M1-M3):
//! a server and clients on `NetSim`'s in-memory link with the party's
//! simulation on the greybox.
//!
//! - The match manifest (`net::manifest`): at join, what the match needs
//!   (its map, the games it reads, the minigame's id, hash and text); a
//!   client without the server's definition plays the server's.
//! - The mount doctor at join: a client whose report lacks a game the match
//!   requires is refused, told exactly which; one whose build can't play
//!   the server's minigame leaves before loading, told why.
//! - `ChangeMatch` (`minigame_net::change_match`): the server plays another
//!   minigame and the clients follow, connected, the scores anew.
//! - The playable set (`minigame_net::playable`, `PartyOffer`): only what
//!   every connected player can play is offered and changed to.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{console::Console, core::Score};
use chimera_net::{
    harness::NetSim,
    minigame_net::PartyOffer,
    net::{
        LastDisconnect, NetHooks,
        client::{JoinFailure, JoinProgress, Joined},
        manifest::{GameMount, LocalMounts, MountReport, ServerManifest},
        maps::sha256,
        memory::LinkConditions,
    },
};
use chimera_sim::{
    minigame::{self, ActiveMinigame, MinigameAppExt, Minigames, Modifier, ladder::{Ladder, Rung}},
    testkit,
};
use chimera_world::greybox::GreyboxMapPlugin;
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin};

/// A short ladder of two CS:S guns on the greybox, which only the server
/// has (a test's own: `minigame_load` on a real server).
pub const SHORT: &str = r#"
id = "test:short_ladder"
name = "Short ladder"
players = { min = 2, max = 8 }
maps = ["chimera:greybox"]
teams = "free_for_all"
time_limit = "5m"

[slots]
weapons = ["cs_source"]

[spawn]
respawn = { delay = "1s", protection = "0s" }
points = "map"

[rules]
module = "chimera:ladder"
ladder = ["cs_source:weapon_deagle", "cs_source:weapon_ak47"]
"#;

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin));
}

/// A server playing `minigame` (none: the gun game) and `clients` clients,
/// not yet joined.
pub fn party(clients: usize, minigame: Option<&str>) -> NetSim {
    party_and(clients, minigame, |_| {})
}

/// `party` with `extra` added to every app (the server's and the
/// clients': a client shell plugin under test).
pub fn party_and(clients: usize, minigame: Option<&str>, extra: fn(&mut App)) -> NetSim {
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        11,
        clients,
        move |app: &mut App| {
            setup(app);
            extra(app);
        },
    );
    sim.set_frame(TICK_INTERVAL);
    let id = match minigame {
        Some(text) => minigame::add(sim.server.app.world_mut(), "test", text).expect("the test minigame reads").id.clone(),
        None => DEFAULT_MINIGAME.to_string(),
    };
    minigame::start(sim.server.app.world_mut(), &id).expect("the minigame starts");
    sim
}

/// A mount doctor's report: these games installed, the others not.
pub fn mounts(installed: &[&str]) -> LocalMounts {
    LocalMounts(MountReport {
        games: ["cs_source", "combat_arms"]
            .iter()
            .map(|g| GameMount {
                game: g.to_string(),
                installed: installed.contains(g),
                build: None,
                notes: Vec::new(),
            })
            .collect(),
    })
}

/// Why client `i`'s game ended, and its kind.
pub fn failure(sim: &NetSim, i: usize) -> Option<(JoinFailure, String)> {
    sim.clients[i].app.world().resource::<JoinProgress>().failure.clone()
}

#[test]
fn a_joiner_hears_the_match_manifest() {
    let mut sim = party(1, None);
    sim.until_joined(900);
    let w = sim.clients[0].app.world();
    let m = &w.resource::<ServerManifest>().0;
    assert_eq!(m.map, "greybox");
    assert!(m.games.iter().all(|g| !g.required), "the greybox needs no install: {:?}", m.games);
    let wanted: Vec<&str> = m.games.iter().map(|g| g.game.as_str()).collect();
    assert!(wanted.contains(&"cs_source") && wanted.contains(&"combat_arms"), "the gun game's games: {wanted:?}");
    assert_eq!(m.weapons, ["cs_source", "combat_arms"]);
    assert!(m.slots.contains(&("hud".to_string(), "combat_arms".to_string())), "{:?}", m.slots);
    let offer = m.minigame.as_ref().expect("the minigame in the manifest");
    assert_eq!(offer.id, DEFAULT_MINIGAME);
    let text = sim.server.app.world().resource::<Minigames>().text(DEFAULT_MINIGAME).unwrap();
    assert_eq!(offer.hash, sha256(text.as_bytes()));
    assert_eq!(offer.text, text);
}

#[test]
fn a_client_plays_the_servers_definition_it_lacked() {
    let mut sim = party(2, Some(SHORT));
    assert!(sim.clients[0].app.world().resource::<Minigames>().get("test:short_ladder").is_none());
    sim.until_joined(900);
    sim.ticks(120);
    for i in 0..2 {
        let w = sim.clients[i].app.world();
        assert_eq!(w.resource::<ActiveMinigame>().def.id, "test:short_ladder", "client {i}");
        assert_eq!(w.resource::<Ladder>().rungs, ["cs_source:weapon_deagle", "cs_source:weapon_ak47"], "client {i}");
        assert_eq!(w.resource::<Minigames>().text("test:short_ladder"), Some(SHORT), "client {i} keeps the text");
    }
}

#[test]
fn a_client_lacking_a_required_game_is_refused_with_what_it_lacks() {
    let mut sim = party(0, None);
    // The match reads CS:S's install (as on a CS:S map: the greybox
    // needs none).
    sim.server
        .app
        .world_mut()
        .resource_mut::<NetHooks>()
        .manifest
        .push(|_, m| m.need("cs_source", true));
    let lacking = sim.add_client(|w| w.insert_resource(mounts(&["combat_arms"])));
    let has = sim.add_client(|w| w.insert_resource(mounts(&["cs_source"])));
    sim.until(900, |s| s.clients[has].app.world().contains_resource::<Joined>() && failure(s, lacking).is_some());
    let (kind, reason) = failure(&sim, lacking).expect("refused");
    assert_eq!(kind, JoinFailure::Mounts, "{reason}");
    assert!(
        reason.starts_with("This match needs CS:S (cs_source), which isn't installed on your machine"),
        "exactly what's missing: {reason}"
    );
    assert_eq!(sim.clients[lacking].app.world().resource::<LastDisconnect>().0, reason);
    assert!(sim.character_of(lacking).is_none(), "no character for the refused client");
    assert!(sim.clients[has].app.world().contains_resource::<Joined>(), "one with the game joins");
    // A client without a report (another app's) is taken at its word.
    let silent = sim.add_client(|_| {});
    sim.until(900, |s| s.clients[silent].app.world().contains_resource::<Joined>());
    assert!(sim.clients[silent].app.world().contains_resource::<Joined>());
}

fn double_check(v: &toml::Value) -> Result<(), String> {
    v.as_bool().map(|_| ()).ok_or_else(|| "a boolean".into())
}

#[test]
fn a_client_that_cant_play_the_servers_minigame_leaves_before_loading() {
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions::default(),
        12,
        0,
        setup,
    );
    sim.set_frame(TICK_INTERVAL);
    // A modifier only the server's build has.
    sim.server.app.add_modifier(Modifier {
        name: "test_double",
        check: double_check,
        apply: |_, _| {},
    });
    let text = format!("{SHORT}\n[modifiers]\ntest_double = true\n").replace("test:short_ladder", "test:modded");
    let w = sim.server.app.world_mut();
    minigame::add(w, "test", &text).expect("the server can play it");
    minigame::start(w, "test:modded").expect("starts");
    let c = sim.add_client(|_| {});
    sim.until(900, |s| failure(s, c).is_some());
    let (kind, reason) = failure(&sim, c).expect("left");
    assert_eq!(kind, JoinFailure::Mounts, "{reason}");
    assert!(
        reason.starts_with("The server's minigame test:modded can't be played by this build")
            && reason.contains("no modifier `test_double`"),
        "{reason}"
    );
    assert!(!sim.clients[c].app.world().contains_resource::<Joined>(), "never loaded");
    let out: Vec<String> = sim.clients[c].app.world().resource::<Console>().output.iter().map(|l| l.text.clone()).collect();
    assert!(!out.iter().any(|l| l.contains("playing test:modded")), "{out:?}");
}

/// The server changes match mid-session (`change_match`): both clients
/// follow without reconnecting (the same connection, character and
/// joined state), play the new minigame (the server's definition, which
/// they didn't have), and the scores start anew; then back again.
#[test]
fn change_match_keeps_the_players_and_resets_the_scores() {
    let mut sim = party(2, None);
    sim.until_joined(900);
    sim.ticks(120);
    minigame::add(sim.server.app.world_mut(), "test", SHORT).unwrap();
    let chars: Vec<Entity> = (0..2).map(|i| sim.character_of(i).unwrap()).collect();
    let mine: Vec<Entity> = (0..2).map(|i| sim.local_player(i).unwrap()).collect();
    // A score to reset.
    sim.server.app.world_mut().get_mut::<Score>(chars[0]).unwrap().kills = 3;
    assert!(sim.until(120, |s| s.clients[0].app.world().get::<Score>(mine[0]).is_some_and(|k| k.kills == 3)));
    let said = chimera_net::minigame_net::change_match(sim.server.app.world_mut(), "test:short_ladder", None).unwrap();
    assert!(said.contains("on greybox"), "{said}");
    let ok = sim.until(600, |s| {
        (0..2).all(|i| {
            s.clients[i].app.world().get_resource::<ActiveMinigame>().is_some_and(|a| a.def.id == "test:short_ladder")
        })
    });
    assert!(ok, "both clients follow");
    sim.ticks(180);
    for i in 0..2 {
        let w = sim.clients[i].app.world();
        assert!(w.contains_resource::<Joined>(), "client {i} still in");
        assert!(failure(&sim, i).is_none(), "client {i} never left");
        assert_eq!(sim.character_of(i), Some(chars[i]), "client {i}'s character kept");
        assert_eq!(sim.local_player(i), Some(mine[i]), "client {i}'s own player kept");
        let w = sim.clients[i].app.world();
        assert_eq!(w.resource::<Ladder>().rungs, ["cs_source:weapon_deagle", "cs_source:weapon_ak47"]);
        assert_eq!(w.get::<Score>(mine[i]).map(|s| s.kills), Some(0), "client {i}: scores anew");
        assert_eq!(w.get::<Rung>(mine[i]).map(|r| r.0), Some(0), "client {i} on the first rung");
    }
    assert_eq!(sim.server.app.world().get::<Score>(chars[0]).unwrap().kills, 0);
    let held = |sim: &mut NetSim, i: usize| {
        let me = sim.local_player(i).unwrap();
        testkit::active_id(sim.clients[i].app.world(), me)
    };
    assert!(sim.until(600, |s| held(s, 0) == Some("cs_source:weapon_deagle")), "the new ladder's first gun");
    // And back to the gun game, which they had.
    chimera_net::minigame_net::change_match(sim.server.app.world_mut(), DEFAULT_MINIGAME, Some("greybox")).unwrap();
    let ok = sim.until(600, |s| {
        (0..2).all(|i| s.clients[i].app.world().get_resource::<ActiveMinigame>().is_some_and(|a| a.def.id == DEFAULT_MINIGAME))
    });
    assert!(ok, "back to the gun game");
    assert!(sim.until(600, |s| held(s, 1) == Some(cs_source::game::weapons::GLOCK)));
    assert!((0..2).all(|i| sim.clients[i].app.world().contains_resource::<Joined>()));
}

/// The playable set (M3): the server offers only the minigames every
/// connected player can play (their mount doctor reports), and refuses to
/// change to another; once the player lacking a game leaves, it offers it.
#[test]
fn the_party_offers_only_what_every_player_can_play() {
    let mut sim = party(0, Some(SHORT));
    let full = sim.add_client(|w| w.insert_resource(mounts(&["cs_source", "combat_arms"])));
    let partial = sim.add_client(|w| {
        w.insert_resource(mounts(&["combat_arms"]));
        w.resource_mut::<chimera_net::net::NetSettings>().name = "Lacking".into();
    });
    sim.until_joined(900);
    sim.ticks(30);
    let offer = |sim: &NetSim, i: usize| {
        sim.clients[i].app.world().get_resource::<PartyOffer>().map(|o| o.minigames.clone())
    };
    for i in [full, partial] {
        assert_eq!(offer(&sim, i), Some(vec!["test:short_ladder".to_string()]), "client {i}: the gun game's dust2 needs CS:S");
    }
    let set = chimera_net::minigame_net::playable(sim.server.app.world_mut());
    let gun = set.iter().find(|p| p.id == DEFAULT_MINIGAME).unwrap();
    assert_eq!(gun.map, "cs_source:de_dust2");
    assert_eq!(gun.lacking, [("Lacking".to_string(), "cs_source".to_string())]);
    let refused = chimera_net::minigame_net::change_match(sim.server.app.world_mut(), DEFAULT_MINIGAME, None).unwrap_err();
    assert_eq!(
        refused,
        "not every player can play originals:gun_game_dust2 on cs_source:de_dust2: Lacking lacks CS:S (cs_source)"
    );
    assert_eq!(sim.server.app.world().resource::<ActiveMinigame>().def.id, "test:short_ladder", "still playing");
    // On the greybox every player can.
    chimera_net::minigame_net::change_match(sim.server.app.world_mut(), DEFAULT_MINIGAME, Some("greybox")).unwrap();
    // The console's view on the server.
    sim.server.app.world_mut().resource_mut::<Console>().submit("party_offer");
    sim.ticks(2);
    let out: Vec<String> = sim.server.app.world().resource::<Console>().output.iter().map(|l| l.text.clone()).collect();
    assert!(
        out.iter().any(|l| l.contains("(originals:gun_game_dust2 on cs_source:de_dust2: Lacking lacks CS:S (cs_source))")),
        "{out:?}"
    );
    sim.remove_client(partial);
    sim.ticks(30);
    assert_eq!(
        offer(&sim, full),
        Some(vec![DEFAULT_MINIGAME.to_string(), "test:short_ladder".to_string()]),
        "everyone left can play both"
    );
}
