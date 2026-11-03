//! The HUD follows the local player's game (the gun game's `identity =
//! "weapon"`): with a CS:S gun CS:S's HUD is up (its panels from the map's
//! CS:S HUD look, the minigame's panel on it) and Combat Arms' is down;
//! with a Combat Arms gun the other way round, Combat Arms' with the
//! minigame's panel; and back. Headless, on the party's simulation with
//! both HUDs' plugins and the shell's switching (`client::hud::
//! HudSwitchPlugin`), a stand-in CS:S HUD look as the map's. A kill that
//! moves the player up a rung and into the other game shows one level-up
//! callout, the new HUD's: Combat Arms' own, or the shell's on CS:S's.

use std::{collections::HashMap, sync::Arc};

use bevy::prelude::*;
use chimera_client::client::{
    callouts::{CalloutRoot, CalloutView, CalloutsPlugin},
    fonts::UiFonts,
    hud::{GameDraws, HudDraws, HudLookOwner, HudRoot, HudSwitchPlugin},
};
use chimera_core::{core::LocalPlayer, gameplay::Gameplay};
use chimera_sim::minigame::{
    self,
    ladder::{self, Ladder},
};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::hud::{ActiveHud, GameHud, HudCoord, HudPanel},
};
use combat_arms::{
    game::movement,
    harness::CaSim,
    ui::hud::{CaHud, CaHudStatePlugin},
};
use cs_source::ui::{
    game_hud::{GameHudPlugin, Part},
    minigame_hud::{MinigameHudPlugin, PanelLine},
};
use party_kit::{DEFAULT_MINIGAME, PartyPartsPlugin};

use crate::gun_game::{held, line_up, shoot_until_dead};

/// A CS:S HUD look with health and armour panels, as a CS:S map's.
pub(crate) fn cs_look() -> ActiveHud {
    let mut hud = GameHud::default();
    for (name, x) in [("HudHealth", 16.0), ("HudArmor", 148.0)] {
        hud.panels.insert(
            name.into(),
            HudPanel {
                x: HudCoord::Start(x),
                y: HudCoord::End(42.0),
                wide: 102.0,
                tall: 36.0,
                background: Some([0, 0, 0, 96]),
                icon: Vec2::new(8.0, 2.0),
                digit: Vec2::new(50.0, 2.0),
                digit2: Vec2::ZERO,
                keys: HashMap::new(),
            },
        );
    }
    ActiveHud(Arc::new(hud), HashMap::new())
}

/// The party with both HUDs (Combat Arms' state and a stand-in root for
/// its drawn panels, CS:S's panels and the minigame's panel on it), a
/// local soldier and another, the gun game playing.
pub(crate) fn party_with_huds() -> (Sim, Entity, Entity) {
    party_with_huds_and(|_| {})
}

/// `party_with_huds` with `extra` plugins added too.
pub(crate) fn party_with_huds_and(extra: impl FnOnce(&mut App)) -> (Sim, Entity, Entity) {
    let mut sim = Sim::with(CaSim, |app| {
        extra(app);
        app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin))
            .init_resource::<UiFonts>()
            .add_plugins((HudSwitchPlugin, CaHudStatePlugin, GameHudPlugin, MinigameHudPlugin))
            .insert_resource(HudLookOwner(cs_source::game::GAME))
            .insert_resource(cs_look());
        // Combat Arms' drawn HUD: a root and what it draws instead of the
        // shell's (`ui::hud::CaHudPlugin` spawns its roots so, with
        // `HudRoot`, and registers its `GameDraws`).
        app.world_mut().spawn((Node::default(), HudRoot(combat_arms::game::GAME)));
        app.insert_resource(HudDraws(vec![(combat_arms::game::GAME, combat_arms::ui::hud::DRAWS)]));
    });
    crate::gun_game::plain_rungs(sim.app.world_mut());
    crate::gun_game::no_finale(sim.app.world_mut());
    minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let other = sim.spawn_character(Vec3::new(3.0, 1.0, 0.0), movement::ID);
    sim.seconds(1.5);
    (sim, me, other)
}

pub(crate) fn hud(sim: &Sim) -> &'static str {
    sim.app.world().resource::<Gameplay>().hud
}

/// Whether a root of `game`'s HUD shows.
pub(crate) fn shown(sim: &mut Sim, game: &str) -> bool {
    let w = sim.app.world_mut();
    let roots: Vec<Visibility> = w
        .query::<(&HudRoot, &Visibility)>()
        .iter(w)
        .filter(|(r, _)| r.0 == game)
        .map(|(_, v)| *v)
        .collect();
    assert!(!roots.is_empty(), "{game}'s HUD has roots");
    roots.iter().all(|v| *v != Visibility::Hidden)
}

fn cs_panels(sim: &mut Sim) -> usize {
    let w = sim.app.world_mut();
    w.query_filtered::<(), With<Part>>().iter(w).count()
}

/// The CS:S minigame panel's lines, as text.
pub(crate) fn cs_minigame_lines(sim: &mut Sim) -> Vec<(PanelLine, String)> {
    let w = sim.app.world_mut();
    w.query::<(&PanelLine, &Text)>().iter(w).map(|(k, t)| (*k, t.0.clone())).collect()
}

pub(crate) fn swap_to(sim: &mut Sim, me: Entity, rung: usize) {
    let w = sim.app.world_mut();
    let l = w.resource::<Ladder>().clone();
    ladder::set_rung(w, me, rung, &l);
    sim.seconds(0.5);
}

/// CS:S's HUD with a CS:S gun, Combat Arms' with the Beretta, CS:S's
/// again with the Desert Eagle: each time only the active one's nodes are
/// up and the minigame's panel is on it.
#[test]
fn the_hud_follows_the_gun_with_the_minigame_on_both() {
    let (mut sim, me, _other) = party_with_huds();
    let title = sim.app.world().resource::<chimera_sim::minigame::hud::MinigameHud>().title.clone();
    assert!(!title.contains('{'), "{title}");
    // The Glock: CS:S's HUD.
    assert_eq!(held(&sim, me), Some(cs_source::game::weapons::GLOCK));
    assert_eq!(hud(&sim), cs_source::game::GAME);
    assert!(!shown(&mut sim, combat_arms::game::GAME), "Combat Arms' HUD down");
    assert!(cs_panels(&mut sim) > 0, "CS:S's health panel up");
    assert_eq!(*sim.app.world().resource::<GameDraws>(), GameDraws::default(), "the shell's crosshair for CS:S");
    let lines = cs_minigame_lines(&mut sim);
    assert!(lines.contains(&(PanelLine::Title, title.clone())), "the minigame on CS:S's HUD: {lines:?}");
    assert!(lines.iter().filter(|(k, _)| *k == PanelLine::Status).count() >= 2, "its rung and more: {lines:?}");
    assert!(shown(&mut sim, cs_source::game::GAME), "the panel shows");
    // The Beretta: Combat Arms'.
    swap_to(&mut sim, me, 1);
    assert_eq!(held(&sim, me), Some("combat_arms:beretta_m92fs"));
    assert_eq!(hud(&sim), combat_arms::game::GAME);
    assert!(shown(&mut sim, combat_arms::game::GAME), "Combat Arms' HUD up");
    assert!(!shown(&mut sim, cs_source::game::GAME), "CS:S's minigame panel down");
    assert_eq!(cs_panels(&mut sim), 0, "CS:S's panels taken down");
    assert!(!sim.app.world().contains_resource::<ActiveHud>(), "CS:S's look put away");
    assert!(sim.app.world().resource::<GameDraws>().crosshair, "Combat Arms draws its own");
    let ca = sim.app.world().resource::<CaHud>().minigame.clone().expect("the minigame on Combat Arms' HUD");
    assert_eq!(ca.title, title);
    assert!(ca.lines.len() >= 2, "{ca:?}");
    // The Desert Eagle: CS:S's again, built anew.
    swap_to(&mut sim, me, 2);
    assert_eq!(held(&sim, me), Some(cs_source::game::weapons::DEAGLE));
    assert_eq!(hud(&sim), cs_source::game::GAME);
    assert!(sim.app.world().contains_resource::<ActiveHud>(), "the look back");
    assert!(cs_panels(&mut sim) > 0, "CS:S's panels built again");
    assert!(!shown(&mut sim, combat_arms::game::GAME));
    assert!(shown(&mut sim, cs_source::game::GAME));
    let lines = cs_minigame_lines(&mut sim);
    assert!(lines.iter().any(|(_, t)| t.starts_with("Rung 3/")), "the third rung shown: {lines:?}");
}

/// HL2:DM's HUD in the party (the identity of a player holding an HL2:DM
/// gun: `GameParts::hud`, M9): its panels hang from a root that shows only
/// while HL2:DM's HUD is the gameplay's (the gun in hand follows
/// `GameParts::hud_of`). With the map's look CS:S's it draws nothing of
/// its own (the shell's HUD stands in).
#[test]
fn hl2dms_hud_is_down_under_other_games_guns() {
    let (mut sim, me, _other) = party_with_huds_and(|app| {
        app.add_plugins(hl2dm::ui::Hl2dmHudPlugin);
        // Its root (the plugin spawns it at startup, which the harness skips).
        app.world_mut().spawn((Node::default(), HudRoot(hl2dm::game::GAME)));
    });
    assert_eq!(party_kit::GAME_PARTS[1].game, hl2dm::game::GAME);
    assert_eq!(party_kit::GAME_PARTS[1].hud, Some(hl2dm::game::GAME), "HL2:DM lends its HUD's id");
    assert_eq!(hud(&sim), cs_source::game::GAME);
    assert!(!shown(&mut sim, hl2dm::game::GAME), "down under a CS:S gun");
    // CS:S's HUD stays up, HL2:DM's builds no panels on CS:S's map (the
    // look is CS:S's), whichever way the local gun's identity goes.
    assert!(shown(&mut sim, cs_source::game::GAME));
    assert!(cs_panels(&mut sim) > 0);
    assert!(sim.app.world().resource::<hl2dm::ui::systems::Hl2dmHud>().hud.is_none());
    let _ = me;
}

/// Whether the shell's callout overlay shows, and its lines.
fn shell_callout(sim: &mut Sim) -> Option<Vec<String>> {
    let w = sim.app.world_mut();
    let shown = w.query_filtered::<&Visibility, With<CalloutRoot>>().iter(w).any(|v| *v != Visibility::Hidden);
    shown.then(|| w.resource::<CalloutView>().lines.clone())
}

/// The level-up callouts drawn: the shell's overlay and Combat Arms' (its
/// HUD up, a callout in its minigame panel).
fn callouts_drawn(sim: &mut Sim) -> Vec<Vec<String>> {
    let mut drawn: Vec<Vec<String>> = shell_callout(sim).into_iter().collect();
    if hud(sim) == combat_arms::game::GAME
        && let Some(m) = sim.app.world().resource::<CaHud>().minigame.clone()
        && !m.callout.is_empty()
    {
        drawn.push(m.callout);
    }
    drawn
}

/// A kill with the Glock (CS:S) gives the Beretta (Combat Arms): one
/// level-up callout, Combat Arms' (the shell's hidden under its HUD); the
/// next kill gives the Desert Eagle (CS:S): one again, the shell's. Never
/// two on any frame of the swap.
#[test]
fn a_rung_up_across_games_shows_one_level_up_callout() {
    let (mut sim, me, other) = party_with_huds_and(|app| {
        app.add_plugins(CalloutsPlugin);
    });
    let kill = |sim: &mut Sim| {
        line_up(sim, me, other, 3.0);
        assert!(shoot_until_dead(sim, me, other, 4.0), "the kill");
        let mut seen = Vec::new();
        for _ in 0..30 {
            sim.ticks(1);
            let drawn = callouts_drawn(sim);
            assert!(drawn.len() <= 1, "one callout at a time: {drawn:?} (HUD {})", hud(sim));
            seen.extend(drawn);
        }
        seen.last().cloned().expect("a level-up callout")
    };
    assert_eq!(hud(&sim), cs_source::game::GAME);
    let up = kill(&mut sim);
    assert_eq!(held(&sim, me), Some("combat_arms:beretta_m92fs"));
    assert_eq!(hud(&sim), combat_arms::game::GAME);
    assert_eq!(shell_callout(&mut sim), None, "the shell's hidden under Combat Arms' HUD");
    assert!(up.len() >= 2, "{up:?}");
    assert!(up[0].starts_with("Level up: "), "{up:?}");
    assert!(up[1].starts_with("Level 2/"), "{up:?}");
    // Back to CS:S: the shell's, Combat Arms' down.
    let up = kill(&mut sim);
    assert_eq!(held(&sim, me), Some(cs_source::game::weapons::DEAGLE));
    assert_eq!(hud(&sim), cs_source::game::GAME);
    let shell = shell_callout(&mut sim).expect("the shell's on CS:S's HUD");
    assert_eq!(shell, up);
    assert!(up[1].starts_with("Level 3/"), "{up:?}");
    let lines = cs_minigame_lines(&mut sim);
    assert!(!lines.iter().any(|(_, t)| t.starts_with("Rung up!")), "no second banner in CS:S's panel: {lines:?}");
}

/// The gun game's title names the map played, not a fixed one: after a
/// rotation to de_aztec it reads "Gun Game: Aztec", on Warehouse
/// "Gun Game: Warehouse" (the definition's name has `{map}`).
#[test]
fn the_title_follows_the_map_played() {
    let (mut sim, _me, _other) = party_with_huds();
    for (map, title) in [
        ("cs_source:de_aztec", "Gun Game: Aztec"),
        ("combat_arms:warehouse", "Gun Game: Warehouse"),
        ("cs_source:de_dust2", "Gun Game: Dust 2"),
    ] {
        sim.app.world_mut().insert_resource(chimera_world::map::LoadedMapName(map.into()));
        sim.seconds(0.3);
        assert_eq!(sim.app.world().resource::<chimera_sim::minigame::hud::MinigameHud>().title, title, "on {map}");
        let lines = cs_minigame_lines(&mut sim);
        assert!(lines.contains(&(PanelLine::Title, title.to_string())), "CS:S's panel on {map}: {lines:?}");
    }
}
