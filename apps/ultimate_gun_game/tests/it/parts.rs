//! Each composed game's capability report (`chimera_sim::minigame::auto::
//! report`: CS:S's, HL2:DM's and Combat Arms' parts, checked against the
//! app) and the automatic gun game made from it: a ladder spread across
//! the games' guns, the same for the same seed, every game's maps in its pool,
//! played by the minigame runtime (each rung's gun makes its holder that
//! gun's game).

use bevy::prelude::*;
use chimera_core::{
    game_parts::GunClass,
    slots::{GameIdentity, MovementSlot},
};
use chimera_sim::{
    minigame::{self, Minigames, auto, ladder::Ladder},
    testkit,
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{game::movement, harness::CaSim};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, RUNGS, SEED, UltimateGunGamePlugin};

pub(crate) fn ugg() -> Sim {
    Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin, UltimateGunGamePlugin::default()))
}

/// The automatic gun game's ladder as `world` has it.
pub(crate) fn rungs(world: &World) -> Vec<String> {
    let def = world.resource::<Minigames>().get(MINIGAME).expect("the automatic gun game is made");
    def.rules.params["ladder"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

/// The powers of the guns of `caps` (`auto::powers`) as `world` scores them.
pub(crate) fn powers(world: &World, caps: &[auto::Capabilities]) -> chimera_sim::minigame::power::Powers {
    use chimera_sim::minigame::power::{PowerCurve, PowerReaders};
    let catalog = world.resource::<chimera_sim::weapon::data::WeaponCatalog>();
    auto::powers(caps, catalog, world.resource::<PowerReaders>(), &PowerCurve::shipped())
}

pub(crate) fn of(rungs: &[String], game: &str) -> usize {
    rungs.iter().filter(|id| id.starts_with(&format!("{game}:"))).count()
}

#[test]
fn each_game_reports_what_it_lends() {
    let sim = ugg();
    let report = auto::report(sim.app.world());
    let games: Vec<&str> = report.iter().map(|c| c.game).collect();
    let claimed_guns = |i: usize| party_kit::GAME_PARTS[i].guns.iter().filter(|g| g.class != GunClass::Grenade).count();
    assert_eq!(games, ["cs_source", "hl2dm", "combat_arms"]);
    for c in &report {
        assert!(c.missing.is_empty(), "every claimed part is in the app: {}", c.line());
        assert!(c.playable(), "{}", c.line());
        assert!(c.knife.is_some(), "{}", c.line());
        assert!(!c.maps.is_empty(), "{}", c.line());
    }
    // HL2:DM's bodies are M8's (the fallback for now); its HUD (M9) is its
    // own id.
    for c in [&report[0], &report[2]] {
        assert!(c.hud.is_some() && c.characters.is_some(), "{}", c.line());
    }
    assert!(report[1].hud == Some(hl2dm::game::GAME) && report[1].characters.is_none(), "{}", report[1].line());
    assert_eq!(report[1].guns.len(), claimed_guns(1), "HL2:DM's five guns: {}", report[1].line());
    assert_eq!(report[1].movement, Some(hl2dm::game::movement::ID));
    assert_eq!(report[0].guns.len(), claimed_guns(0), "every CS:S gun");
    assert_eq!(report[0].grenades, ["cs_source:weapon_hegrenade"]);
    assert_eq!(report[2].movement, Some(movement::ID));
    // Combat Arms' guns: the claimed ones and those its data gives a
    // class (its launchers, the crossbow; with an install, its weapons).
    assert!(report[2].guns.len() > claimed_guns(2), "{}", report[2].line());
    for id in ["combat_arms:rpg-7", "combat_arms:law", "combat_arms:m79", "combat_arms:assassin_bowgun_nor"] {
        assert!(report[2].guns.iter().any(|g| g.id == id), "{id}: {}", report[2].line());
    }
    assert!(!report[2].guns.iter().any(|g| g.id == "combat_arms:m136_at4"), "no damage: not a gun of the ladder");
    assert_eq!(report[2].grenades, ["combat_arms:m67"]);
}

#[test]
fn the_ladder_spreads_across_the_games_and_keeps_its_seed() {
    let mut sim = ugg();
    let first = rungs(sim.app.world());
    assert_eq!(first.len(), RUNGS - 1, "{RUNGS} rungs, one of them melee");
    assert!(of(&first, "cs_source") > 0 && of(&first, "combat_arms") > 0 && of(&first, "hl2dm") > 0, "{first:?}");
    let def = sim.app.world().resource::<Minigames>().get(MINIGAME).unwrap();
    assert_eq!(
        def.maps,
        [
            "cs_source:de_dust2",
            "hl2dm:dm_lockdown",
            "combat_arms:warehouse",
            "cs_source:cs_office",
            "hl2dm:dm_overwatch",
            "combat_arms:junk_flea",
            "combat_arms:darkhills",
            "combat_arms:bloodmoney"
        ],
        "every game's maps, in turn"
    );
    assert_eq!(def.rotation, minigame::def::Rotation::Random, "a random map, not one of the last ten");
    assert_eq!(def.spawn.kit.for_game(Some("cs_source")), ["cs_source:weapon_knife"], "each gun's game's knife");
    assert_eq!(def.spawn.kit.for_game(Some("combat_arms")), ["combat_arms:m9_knife"]);
    assert_eq!(def.spawn.kit.for_game(Some("hl2dm")), ["hl2dm:weapon_crowbar"]);
    // The same seed: the same ladder; another, another.
    ultimate_gun_game::generate(sim.app.world_mut(), SEED, RUNGS).unwrap();
    assert_eq!(rungs(sim.app.world()), first);
    ultimate_gun_game::generate(sim.app.world_mut(), SEED + 1, RUNGS).unwrap();
    assert_ne!(rungs(sim.app.world()), first);
    // `ugg_new` makes one and plays it.
    sim.app.world_mut().resource_mut::<chimera_core::console::Console>().submit("ugg_new 7 6 greybox");
    sim.ticks(2);
    assert_eq!(rungs(sim.app.world()).len(), 5, "6 rungs, one of them melee");
    let active = sim.app.world().resource::<minigame::ActiveMinigame>();
    assert_eq!(active.def.id, MINIGAME);
}

#[test]
fn a_player_climbs_as_each_guns_game() {
    let mut sim = ugg();
    let ladder = rungs(sim.app.world());
    minigame::start(sim.app.world_mut(), MINIGAME).expect("the automatic gun game starts");
    let a = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    let _b = sim.spawn_character(Vec3::new(2.0, 1.0, 0.0), movement::ID);
    sim.seconds(1.5);
    let l = sim.app.world().resource::<Ladder>().clone();
    for (i, id) in ladder.iter().enumerate().take(4) {
        minigame::ladder::set_rung(sim.app.world_mut(), a, i, &l);
        sim.seconds(1.2);
        let w = sim.app.world();
        assert_eq!(testkit::active_id(w, a), Some(l.rungs[i]));
        assert_eq!(l.rungs[i], id.as_str());
        let game = id.split_once(':').unwrap().0;
        assert_eq!(w.get::<GameIdentity>(a).map(|g| g.0.as_str()), Some(game), "rung {i}: {id}");
        let want = match game {
            "cs_source" => "cs_source:movement",
            "hl2dm" => hl2dm::game::movement::ID,
            _ => movement::ID,
        };
        assert_eq!(w.get::<MovementSlot>(a).map(|m| m.0), Some(want), "rung {i}: {id}");
    }
}
