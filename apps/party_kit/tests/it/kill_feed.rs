//! The party's one kill feed (`chimera_client::client::kill_feed`,
//! `party_kit::client::PartyHudPlugin`) and its radars: a kill just
//! before a rung swap is still in the feed after it, whatever HUD is up;
//! only the shell's feed has rows (CS:S's death notices and Combat Arms'
//! feed stay empty); without the party's flag the games' own feeds are as
//! before; a network client's feed has the server's entries; the radars
//! are hidden until `hud_radar 1`. Every party game with a HUD declares
//! its own feed (`kill_feed::GameKillFeeds`), and with each game's HUD up
//! (HL2:DM's, CS:S's, Combat Arms') a kill is exactly one line on screen,
//! the shell's. Headless, on `hud::party_with_huds`.

use std::{sync::Arc, time::Duration};

use bevy::prelude::*;
use chimera_client::client::kill_feed::{KillFeed, KillFeedPlugin, KillFeedRow, KillFeedStatePlugin, SharedKillFeed};
use chimera_core::{
    console::Console,
    core::{Damage, DamageKind, Died, God, Hitgroup},
};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_sim::minigame;
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::hud::{ActiveHud, HudCoord, HudPanel},
};
use combat_arms::{
    game::TICK_INTERVAL,
    harness::CaSim,
    ui::hud::{CaHud, radar::CaRadar},
};
use cs_source::ui::game_hud::Part;
use party_kit::{DEFAULT_MINIGAME, PartyHudPlugin, PartyPartsPlugin};

use crate::hud::{hud, party_with_huds_and, swap_to};

/// The party with both HUDs, a CS:S notices panel to draw into, a window,
/// the shell's feed widget and, with `party`, the party's HUD settings.
fn feed_sim(party: bool) -> (Sim, Entity, Entity) {
    let (mut sim, me, other) = party_with_huds_and(|app| {
        app.add_plugins(KillFeedPlugin);
        if party {
            app.add_plugins(PartyHudPlugin);
        }
    });
    let w = sim.app.world_mut();
    w.spawn(Window::default());
    let mut look = w.resource_mut::<ActiveHud>();
    Arc::make_mut(&mut look.0).panels.insert(
        "HudDeathNotice".into(),
        HudPanel {
            x: HudCoord::Start(0.0),
            y: HudCoord::Start(0.0),
            wide: 300.0,
            tall: 100.0,
            background: None,
            icon: Vec2::ZERO,
            digit: Vec2::ZERO,
            digit2: Vec2::ZERO,
            keys: Default::default(),
        },
    );
    sim.seconds(0.2);
    (sim, me, other)
}

fn kill(sim: &mut Sim, victim: Entity, killer: Entity, weapon: &'static str, hitgroup: Hitgroup) {
    sim.app.world_mut().write_message(Died {
        entity: victim,
        attacker: Some(killer),
        damage: Damage {
            target: victim,
            attacker: Some(killer),
            amount: 1.0,
            point: Vec3::ZERO,
            dir: Vec3::NEG_Z,
            hitgroup,
            kind: DamageKind::Bullet,
            weapon: Some(weapon),
            force: Vec3::ZERO,
        },
    });
}

/// The feed's entries as (victim, weapon, headshot).
fn entries(sim: &Sim) -> Vec<(Entity, Option<&'static str>, bool)> {
    let feed = sim.app.world().resource::<KillFeed>();
    feed.entries.iter().map(|e| (e.victim.entity, e.weapon, e.headshot)).collect()
}

/// Rows drawn by the shell's feed, CS:S's death notices and Combat Arms'
/// feed lines.
fn drawn(sim: &mut Sim) -> (usize, usize, usize) {
    let w = sim.app.world_mut();
    let shell = w.query::<&KillFeedRow>().iter(w).count();
    let notices = w
        .query::<(Entity, &Part)>()
        .iter(w)
        .find(|(_, p)| **p == Part::Notices)
        .map_or(0, |(e, _)| w.get::<Children>(e).map_or(0, |c| c.len()));
    let ca = w.resource::<CaHud>().feed.len();
    (shell, notices, ca)
}

#[test]
fn a_kill_just_before_a_swap_is_still_in_the_one_feed_after_it() {
    let (mut sim, me, other) = feed_sim(true);
    assert_eq!(hud(&sim), cs_source::game::GAME, "the Glock: CS:S's HUD");
    kill(&mut sim, other, me, cs_source::game::weapons::GLOCK, Hitgroup::Head);
    sim.seconds(0.1);
    // The kill made the rung: Combat Arms' gun, Combat Arms' HUD.
    swap_to(&mut sim, me, 1);
    assert_eq!(hud(&sim), combat_arms::game::GAME);
    assert_eq!(
        entries(&sim),
        [(other, Some(cs_source::game::weapons::GLOCK), true)],
        "the kill made with CS:S's HUD up, after the swap"
    );
    assert_eq!(drawn(&mut sim), (1, 0, 0), "one row, the shell's; no game's own");
    // A Combat Arms kill now, and back to CS:S's HUD: both rows still
    // there, CS:S's notices still empty.
    kill(&mut sim, other, me, "combat_arms:beretta_m92fs", Hitgroup::Chest);
    sim.seconds(0.3);
    swap_to(&mut sim, me, 2);
    assert_eq!(hud(&sim), cs_source::game::GAME);
    assert_eq!(entries(&sim).len(), 2);
    assert_eq!(drawn(&mut sim), (2, 0, 0), "two rows, the shell's only");
    let feed = sim.app.world().resource::<KillFeed>();
    let first = &feed.entries[0];
    assert!(first.killer.as_ref().is_some_and(|k| k.local) && !first.suicide && !first.team_kill);
    // Gone after its lifetime.
    sim.seconds(f64::from(chimera_client::client::kill_feed::LIFETIME) + 0.5);
    assert!(entries(&sim).is_empty());
    assert_eq!(drawn(&mut sim).0, 0);
}

#[test]
fn without_the_partys_flag_the_games_feeds_are_their_own() {
    let (mut sim, me, other) = feed_sim(false);
    assert!(!sim.app.world().contains_resource::<SharedKillFeed>());
    // The local player killed (its rung, so its HUD, stays).
    kill(&mut sim, me, other, cs_source::game::weapons::GLOCK, Hitgroup::Chest);
    sim.seconds(0.3);
    assert_eq!(hud(&sim), cs_source::game::GAME);
    // (Combat Arms' HUD state runs whatever HUD is up here: its drawing
    // is what `CaHudPlugin` keeps to its own HUD.)
    let (shell, notices, _) = drawn(&mut sim);
    assert_eq!((shell, notices), (0, 1), "CS:S's death notice; the shell's feed not drawn");
    swap_to(&mut sim, me, 1);
    assert_eq!(hud(&sim), combat_arms::game::GAME);
    kill(&mut sim, me, other, "combat_arms:beretta_m92fs", Hitgroup::Chest);
    sim.seconds(0.3);
    assert_eq!(drawn(&mut sim).2, 2, "Combat Arms' own feed lines");
    assert_eq!(drawn(&mut sim).0, 0);
}

#[test]
fn the_radars_are_hidden_in_the_party_until_hud_radar_1() {
    // Plain: Combat Arms' corner map shows (the local player alive).
    let (mut sim, me, _) = feed_sim(false);
    swap_to(&mut sim, me, 1);
    assert_eq!(hud(&sim), combat_arms::game::GAME);
    assert!(sim.app.world().resource::<CaRadar>().visible, "the radar without the party's setting");
    // The party: hidden, and back with `hud_radar 1`.
    let (mut sim, me, _) = feed_sim(true);
    swap_to(&mut sim, me, 1);
    assert!(!sim.app.world().resource::<CaRadar>().visible, "no radar in the party");
    sim.app.world_mut().resource_mut::<Console>().submit("hud_radar 1");
    sim.seconds(0.2);
    assert!(sim.app.world().resource::<CaRadar>().visible, "hud_radar 1 brings it back");
}

#[test]
fn a_network_client_has_the_servers_entries() {
    let setup = |app: &mut App| {
        app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin, KillFeedStatePlugin));
    };
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        7,
        2,
        setup,
    );
    sim.set_frame(TICK_INTERVAL);
    minigame::start(sim.server.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    sim.until_joined(900);
    sim.ticks(300);
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    assert!(sim.until(600, |s| s.server.app.world().get::<God>(b).is_none()));
    let at = sim.server.position(b);
    sim.server.app.world_mut().write_message(Damage {
        target: b,
        attacker: Some(a),
        amount: 500.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Head,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(20);
    let view = |w: &World| -> Vec<(String, String, Option<&'static str>, bool)> {
        w.resource::<KillFeed>()
            .entries
            .iter()
            .map(|e| {
                (
                    e.killer.as_ref().map(|k| k.name.clone()).unwrap_or_default(),
                    e.victim.name.clone(),
                    e.weapon,
                    e.headshot,
                )
            })
            .collect()
    };
    let server = view(sim.server.app.world());
    assert_eq!(server.len(), 1, "the server's kill: {server:?}");
    assert!(server[0].2.is_some() && server[0].3, "the gun in hand, a headshot: {server:?}");
    for i in 0..2 {
        assert_eq!(view(sim.clients[i].app.world()), server, "client {i}'s feed");
    }
    // The killer's own client marks its row as the local player's.
    let w = sim.clients[0].app.world();
    assert!(w.resource::<KillFeed>().entries[0].killer.as_ref().is_some_and(|k| k.local));
}

/// Every line of the shell's feed is as tall whatever its icon (a CS:S
/// glyph, a Combat Arms image, a tall image, none), and each icon sits in
/// a box as tall as the line that centres it vertically, scaled down to
/// leave a margin when it is taller. (The headless app has no layout:
/// the nodes' set sizes and alignment are what is checked.)
#[test]
fn lines_are_equal_in_height_and_icons_centred() {
    use chimera_client::client::weapon_icons::{WeaponIcon, WeaponIcons};
    let (mut sim, me, other) = feed_sim(true);
    let mut icons = WeaponIcons::default();
    icons.register("cs_source", |_| Some(WeaponIcon::Glyph { font: Handle::default(), ch: 'a', em: 80.0 }));
    icons.register("combat_arms", |id| {
        let size = if id.ends_with("tall") { Vec2::new(40.0, 200.0) } else { Vec2::new(120.0, 30.0) };
        Some(WeaponIcon::Image { image: Handle::default(), size })
    });
    sim.app.insert_resource(icons);
    sim.app.world_mut().resource_mut::<KillFeed>().max = 8;
    for w in [cs_source::game::weapons::GLOCK, "combat_arms:beretta_m92fs", "combat_arms:tall", "none:thing"] {
        kill(&mut sim, other, me, w, Hitgroup::Chest);
        sim.seconds(0.1);
    }
    sim.seconds(0.3);
    let w = sim.app.world_mut();
    let rows: Vec<Entity> = w.query_filtered::<Entity, With<KillFeedRow>>().iter(w).collect();
    assert_eq!(rows.len(), 4);
    let tall = |e: Entity| w.get::<Node>(e).unwrap().height;
    let line = tall(rows[0]);
    assert!(matches!(line, Val::Px(h) if h > 0.0), "{line:?}");
    let Val::Px(line_px) = line else { unreachable!() };
    let mut boxes = 0;
    for r in &rows {
        assert_eq!(tall(*r), line, "every line the same height");
        for c in w.get::<Children>(*r).unwrap().to_vec() {
            let n = w.get::<Node>(c).unwrap();
            if n.align_items == AlignItems::Center && n.height == line {
                boxes += 1;
                for i in w.get::<Children>(c).unwrap().to_vec() {
                    let inner = w.get::<Node>(i).unwrap();
                    if let Val::Px(h) = inner.height {
                        assert!(h < line_px, "the image leaves a margin: {h} in {line_px}");
                    }
                }
            }
        }
    }
    assert_eq!(boxes, 3, "the three iconed lines have a centring box, the fourth words");
}

/// The party on the greybox playing HL2:DM's mixed ladder (HL2:DM's
/// pistol, CS:S's Glock, HL2:DM's SMG, Combat Arms' Beretta, ...), with
/// every game's HUD (`party_kit::client::CLIENTS`: CS:S's panels on the
/// map's CS:S look, Combat Arms' state, HL2:DM's on its stand-in look as
/// the map's guest), the
/// shell's feed widget, a window and, with `party`, the party's HUD
/// settings (the one feed). The local player, a killer and its victims.
fn every_hud_sim(party: bool) -> (Sim, Entity, Entity) {
    use chimera_client::client::{fonts::UiFonts, hud::{HudDraws, HudLookOwner, HudRoot, HudSwitchPlugin}};
    use chimera_sim::minigame::MinigameAppExt;
    use combat_arms::ui::hud::CaHudStatePlugin;
    use cs_source::ui::{game_hud::GameHudPlugin, minigame_hud::MinigameHudPlugin};
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins(KillFeedPlugin);
        if party {
            app.add_plugins(PartyHudPlugin);
        }
        app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin))
            .init_resource::<UiFonts>()
            // What HL2:DM's scoreboard reads of the shell's input.
            .init_resource::<chimera_client::client::console::HeldActions>()
            .init_resource::<ButtonInput<KeyCode>>()
            // What its drawing writes to (no renderer here).
            .init_asset::<chimera_client::client::hud_text::HudGlyphMaterial>()
            .init_asset::<Image>()
            .add_plugins((HudSwitchPlugin, CaHudStatePlugin, GameHudPlugin, MinigameHudPlugin, hl2dm::ui::Hl2dmHudPlugin))
            .insert_resource(HudLookOwner(cs_source::game::GAME))
            .insert_resource(crate::hud::cs_look())
            .insert_resource(chimera_world::map::hud::GuestLooks(vec![hl2_look()]));
        app.world_mut().spawn((Node::default(), HudRoot(combat_arms::game::GAME)));
        app.world_mut()
            .get_resource_or_init::<HudDraws>()
            .0
            .push((combat_arms::game::GAME, combat_arms::ui::hud::DRAWS));
    });
    crate::gun_game::plain_rungs(sim.app.world_mut());
    crate::gun_game::no_finale(sim.app.world_mut());
    sim.app.add_minigames(&[("hl2dm.toml", &crate::hl2dm_parts::mixed_text(false))]).unwrap();
    minigame::start(sim.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.app.world_mut().entity_mut(me).insert(chimera_core::core::LocalPlayer);
    let killer = sim.spawn_character(Vec3::new(3.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.seconds(1.5);
    let w = sim.app.world_mut();
    w.spawn(Window::default());
    let mut look = w.resource_mut::<ActiveHud>();
    Arc::make_mut(&mut look.0).panels.insert(
        "HudDeathNotice".into(),
        HudPanel {
            x: HudCoord::Start(0.0),
            y: HudCoord::Start(0.0),
            wide: 300.0,
            tall: 100.0,
            background: None,
            icon: Vec2::ZERO,
            digit: Vec2::ZERO,
            digit2: Vec2::ZERO,
            keys: Default::default(),
        },
    );
    sim.seconds(0.2);
    (sim, me, killer)
}

/// HL2:DM's stand-in look (its spec's layout and colours, no install) as
/// the map's guest look: HL2:DM's HUD builds on it while it is up.
fn hl2_look() -> ActiveHud {
    let look = hl2dm::ui::look::spec_look(hl2dm::ui::frame::Screen::base());
    let mut hud = (*look.hud).clone();
    hud.game = hl2dm::game::GAME.into();
    let mut menus = chimera_world::map::hud::GameMenus::default();
    menus.strings.insert(hl2dm::game::hud::ANIMATIONS.into(), String::new());
    hud.menus = Some(menus);
    ActiveHud(Arc::new(hud), Default::default())
}

/// Every feed line on screen: the shell's rows and every game's own
/// feed's lines (`GameKillFeeds`), by game.
fn feed_lines(sim: &mut Sim) -> (usize, Vec<(&'static str, usize)>) {
    use chimera_client::client::kill_feed::GameKillFeeds;
    let w = sim.app.world_mut();
    let shell = w.query::<&KillFeedRow>().iter(w).count();
    (shell, GameKillFeeds::lines(w))
}

/// Every game the party composes with a HUD has declared its own kill
/// feed, so the one-feed checks below count it: a game added to the party
/// without one fails here (declare it with `register_game_kill_feed`, and
/// gate it with `kill_feed::OwnKillFeed`).
#[test]
fn every_party_game_with_a_hud_declares_its_own_feed() {
    let (sim, _, _) = every_hud_sim(true);
    let feeds = sim.app.world().resource::<chimera_client::client::kill_feed::GameKillFeeds>();
    for client in party_kit::client::CLIENTS.iter().filter(|c| c.hud.is_some()) {
        assert!(
            feeds.0.iter().any(|(g, _)| *g == client.game),
            "{}'s HUD declares no kill feed (add its HUD plugin to `every_hud_sim` if it is new)",
            client.game
        );
    }
}

/// With each game's HUD up in the party (HL2:DM's: the user's 2026-11-12
/// recording had its death notices under the shell's feed), a kill is
/// exactly one line on screen: the shell's row, no game's own.
#[test]
fn with_every_games_hud_a_kill_is_one_line() {
    for (rung, game) in [(0, hl2dm::game::GAME), (1, cs_source::game::GAME), (3, combat_arms::game::GAME)] {
        let (mut sim, me, killer) = every_hud_sim(true);
        swap_to(&mut sim, me, rung);
        assert_eq!(hud(&sim), game, "rung {rung}'s HUD");
        let victim = sim.spawn_character(Vec3::new(-3.0, 1.0, 0.0), combat_arms::game::movement::ID);
        sim.seconds(0.2);
        kill(&mut sim, victim, killer, cs_source::game::weapons::GLOCK, Hitgroup::Chest);
        sim.seconds(0.3);
        let (shell, games) = feed_lines(&mut sim);
        assert_eq!(hud(&sim), game, "still {game}'s HUD");
        assert_eq!(shell, 1, "{game}'s HUD up: the shell's row");
        assert!(games.iter().all(|(_, n)| *n == 0), "{game}'s HUD up: no game's own feed lines: {games:?}");
    }
}

/// The same HUDs without the party's flag (as each game's own app):
/// HL2:DM's HUD shows its own death notice and the shell draws none.
#[test]
fn without_the_partys_flag_hl2dms_hud_has_its_own_notice() {
    let (mut sim, me, killer) = every_hud_sim(false);
    swap_to(&mut sim, me, 0);
    assert_eq!(hud(&sim), hl2dm::game::GAME);
    let victim = sim.spawn_character(Vec3::new(-3.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.seconds(0.2);
    kill(&mut sim, victim, killer, cs_source::game::weapons::GLOCK, Hitgroup::Chest);
    sim.seconds(0.3);
    let (shell, games) = feed_lines(&mut sim);
    assert_eq!(shell, 0, "the shell's feed not drawn");
    assert!(games.contains(&(hl2dm::game::GAME, 1)), "HL2:DM's death notice: {games:?}");
}
