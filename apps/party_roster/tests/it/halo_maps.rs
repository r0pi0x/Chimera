//! Halo in the party, without its disc image (docs/plans/active/
//! halo-ce.md, M1 and M2): the party composes it (its game registered,
//! its surface classes, its pool rules, its four guns with its movement),
//! claims none of its maps (so a machine without the disc is offered
//! none: the pool is the installed maps its rules allow), and its maps'
//! surface names are Combat Arms' through the composed table. A Halo gun
//! as a rung makes its holder Halo's (its movement and shield) and fires
//! by Halo's rules on the party's tick, with no view model. With the
//! disc: `heavy::halo_maps`. Halo's HUD comes and goes with its gun:
//! `halos_hud_follows_its_gun` (and `mixed_with_halo`, the every-HUD
//! ladder of `kill_feed` and the overlay guard).

use chimera_sim::minigame::auto::{self, InstalledMaps};
use chimera_world::{harness::Sim, map::impact::SurfaceClasses};
use combat_arms::{game::effects::data::CaFxTables, harness::CaSim};
use party_roster::PartyPartsPlugin;

#[test]
fn halo_lends_its_guns_and_no_map_without_the_disc() {
    let sim = Sim::new(CaSim, PartyPartsPlugin);
    let w = sim.app.world();
    assert!(w.resource::<chimera_core::game_registry::Games>().get(halo_ce::game::GAME).is_some());
    assert!(w.resource::<SurfaceClasses>().of(halo_ce::game::GAME).is_some(), "its surfaces' classes");
    let report = auto::report(w);
    let halo = report.iter().find(|c| c.game == halo_ce::game::GAME).expect("Halo reports its parts");
    // Its lent parts are its guns and the grenade rung (a grenade-class part).
    assert_eq!(halo.guns.len() + halo.grenades.len(), halo_ce::game::parts::PARTS.guns.len(), "{}", halo.line());
    assert_eq!(halo.grenades.len(), 1, "{}", halo.line());
    assert!(halo.missing.is_empty() && halo.movement.is_some() && halo.playable(), "{}", halo.line());
    assert!(halo.maps.is_empty(), "no map claimed without the disc: {}", halo.line());
    // The other games still lend what they did.
    for c in report.iter().filter(|c| c.game != halo_ce::game::GAME) {
        assert!(c.playable(), "{}", c.line());
    }
}

#[test]
fn the_disc_images_maps_join_by_halos_pool_rules() {
    let mut sim = Sim::new(CaSim, PartyPartsPlugin);
    let all: Vec<String> = halo_ce::game::MULTIPLAYER_MAPS.iter().map(|m| m.to_string()).collect();
    sim.app.world_mut().insert_resource(InstalledMaps(vec![(halo_ce::game::GAME.into(), all)]));
    let report = auto::report(sim.app.world());
    let maps = &report.iter().find(|c| c.game == halo_ce::game::GAME).unwrap().maps;
    for must in ["halo_ce:hangemhigh", "halo_ce:bloodgulch"] {
        assert!(maps.iter().any(|m| m == must), "{must}: {maps:?}");
    }
    assert!(maps.len() < halo_ce::game::MULTIPLAYER_MAPS.len(), "some are kept out: {maps:?}");
    // The pool a gun game is made with has them only where every player
    // has the disc (here: no reports, so every game counts).
    let pool = auto::map_pool(&report, &|g| g != halo_ce::game::GAME);
    assert!(!pool.iter().any(|m| m.starts_with("halo_ce:")), "{pool:?}");
}

/// Every surface name Halo's maps use (its material types' generic names,
/// games/halo_ce/data/surfaces.toml) is named in the composed table, as a
/// surface type Combat Arms' impact table knows.
#[test]
fn halos_surfaces_are_combat_arms_through_the_table() {
    let table = party_roster::SURFACES
        .iter()
        .find(|t| t.map == halo_ce::game::GAME && t.gameplay == combat_arms::game::GAME)
        .expect("a table for Halo's maps");
    let rows = table.rows().unwrap();
    let ca = CaFxTables::built_in();
    for row in halo_ce::game::maps::surfaces::table().values() {
        let name = row.surface.as_str();
        let to = rows.names.get(name).unwrap_or_else(|| panic!("`{name}` (Halo's {}) isn't named", row.name));
        assert!(ca.surface_id(to).is_some(), "`{name}` as `{to}`: not one of Combat Arms' surface types");
    }
}

/// A gun game whose ladder is Halo's assault rifle, CS:S's Glock, Halo's
/// pistol: its holder is Halo's on Halo's rungs (identity, movement, the
/// shield as `Vitality`), not on the other, and Halo's guns fire on the
/// party's 64 Hz tick with no view model to draw.
#[test]
fn a_halo_rung_makes_its_holder_halos() {
    use chimera_core::slots::{GameIdentity, MovementSlot};
    use chimera_sim::{
        minigame::{self, MinigameAppExt, ladder::Ladder},
        weapon::Magazine,
    };
    use chimera_world::greybox::GreyboxMapPlugin;
    use halo_ce::game::{movement, vitality::Vitality, weapons};

    use crate::gun_game::{held, no_finale, soldier};

    let ladder = [weapons::ASSAULT_RIFLE, "cs_source:weapon_glock", weapons::PISTOL];
    let text = crate::hl2dm_parts::ladder_text(&ladder, false).replace(
        "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]",
        "weapons = [\"cs_source\", \"hl2dm\", \"halo_ce\", \"combat_arms\"]",
    );
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    no_finale(sim.app.world_mut());
    sim.app.add_minigames(&[("halo.toml", &text)]).unwrap();
    minigame::start(sim.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    let identity = |sim: &Sim| sim.app.world().get::<GameIdentity>(a).map(|g| g.0.clone());
    assert_eq!(held(&sim, a), Some(weapons::ASSAULT_RIFLE));
    assert_eq!(identity(&sim).as_deref(), Some("halo_ce"));
    assert_eq!(sim.app.world().get::<MovementSlot>(a).map(|m| m.0), Some(movement::ID));
    assert!(sim.app.world().get::<Vitality>(a).is_some(), "a Halo holder wears the shield");
    // It fires: 15 a second, a round off the magazine each.
    let gun = sim.app.world().get::<chimera_sim::weapon::Inventory>(a).unwrap().active.unwrap();
    sim.intent(a).fire = true;
    sim.seconds(1.0);
    sim.intent(a).fire = false;
    let clip = sim.app.world().get::<Magazine>(gun).unwrap().clip;
    assert!((44..=46).contains(&clip), "about 15 rounds in a second: {clip} left");
    let set_rung = |sim: &mut Sim, r: usize| {
        let w = sim.app.world_mut();
        let l = w.resource::<Ladder>().clone();
        minigame::ladder::set_rung(w, a, r, &l);
    };
    set_rung(&mut sim, 1);
    sim.seconds(1.2);
    assert_eq!(identity(&sim).as_deref(), Some("cs_source"));
    assert!(sim.app.world().get::<Vitality>(a).is_none(), "the shield goes with Halo");
    set_rung(&mut sim, 2);
    sim.seconds(1.2);
    assert_eq!(held(&sim, a), Some(weapons::PISTOL));
    assert_eq!(identity(&sim).as_deref(), Some("halo_ce"));
}

/// The mixed ladder (`hl2dm_parts::LADDER`) with Halo's assault rifle as a
/// last rung (5), Halo's guns allowed: the every-HUD party's
/// (`kill_feed::every_hud_sim`), so each game with a HUD has a rung.
pub(crate) fn mixed_with_halo() -> String {
    let mut ladder = crate::hl2dm_parts::LADDER.to_vec();
    ladder.push(halo_ce::game::weapons::ASSAULT_RIFLE);
    crate::hl2dm_parts::ladder_text(&ladder, false).replace(
        "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]",
        "weapons = [\"cs_source\", \"hl2dm\", \"halo_ce\", \"combat_arms\"]",
    )
}

/// Halo's HUD in the party (`halo_ce_ui::party::CLIENT`): up while the
/// local player holds a Halo gun, its root hidden and the other game's HUD
/// up again on another game's rung; while it is up the shell's fallback
/// crosshair and status are off (`GameDraws`), and its declared kill feed
/// stays empty.
#[test]
fn halos_hud_follows_its_gun() {
    use chimera_client::client::hud::GameDraws;
    let (mut sim, me, _) = crate::kill_feed::every_hud_sim_and(true, |_| {});
    crate::hud::swap_to(&mut sim, me, 5);
    assert_eq!(crate::hud::hud(&sim), halo_ce::game::GAME, "Halo's gun, Halo's HUD");
    assert!(crate::hud::shown(&mut sim, halo_ce::game::GAME));
    let draws = *sim.app.world().resource::<GameDraws>();
    assert!(draws.crosshair && draws.status && !draws.killfeed, "{draws:?}");
    let feeds = chimera_client::client::kill_feed::GameKillFeeds::lines(sim.app.world());
    assert!(feeds.contains(&(halo_ce::game::GAME, 0)), "{feeds:?}");
    crate::hud::swap_to(&mut sim, me, 0);
    assert_eq!(crate::hud::hud(&sim), hl2dm::game::GAME);
    assert!(!crate::hud::shown(&mut sim, halo_ce::game::GAME), "Halo's root hidden on HL2:DM's rung");
    assert!(crate::hud::shown(&mut sim, hl2dm::game::GAME), "HL2:DM's up");
    crate::hud::swap_to(&mut sim, me, 5);
    assert!(crate::hud::shown(&mut sim, halo_ce::game::GAME), "back with the gun");
}
