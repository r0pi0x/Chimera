//! Each composed game's capability report (`chimera_sim::minigame::auto::
//! report`: CS:S's, HL2:DM's and Combat Arms' parts, checked against the
//! app) and the automatic gun game made from it: a ladder spread across
//! the games' guns, the same for the same seed, every game's maps in its pool,
//! played by the minigame runtime (each rung's gun makes its holder that
//! gun's game).

// The helpers every binary includes (tests/common/parts.rs), here for the
// modules that say `crate::parts::`.
#[path = "../common/parts.rs"]
mod helpers;
pub(crate) use helpers::*;

use bevy::prelude::*;
use chimera_core::{
    game_parts::GunClass,
    slots::{GameIdentity, MovementSlot},
};
use chimera_sim::{
    minigame::{self, Minigames, auto, ladder::Ladder},
    testkit,
};
use combat_arms::game::movement;
use ultimate_gun_game::{MINIGAME, RUNGS, SEED};

#[test]
fn each_game_reports_what_it_lends() {
    let sim = ugg();
    let mut report = auto::report(sim.app.world());
    let games: Vec<&str> = report.iter().map(|c| c.game).collect();
    // The roster's guests in order, then the base game (Combat Arms).
    let mut want: Vec<&str> = party_roster::GUESTS.iter().map(|b| b.info.id).collect();
    want.push("combat_arms");
    assert_eq!(games, want);
    // TF2 lends its twelve guns (M2b's four, tf2-guns2's eight) with its
    // movement, no knife, body or pool map yet (docs/plans/active/tf2.md).
    let tf2 = report.remove(report.iter().position(|c| c.game == "tf2").unwrap());
    assert!(tf2.playable() && tf2.missing.is_empty() && tf2.maps.is_empty(), "{}", tf2.line());
    assert_eq!(tf2.guns.len(), 12, "{}", tf2.line());
    // Halo lends eight held guns (not the flamethrower) and the Hunter's
    // own cannon (a rung of its own, dealt only with campaign bodies on)
    // with its movement, and claims no map (they come from its disc image:
    // tests/it/halo_maps.rs in party_kit). The others as before.
    // DoD:S lends its guns (M1a's, the machine guns, M1b's launchers) and
    // its two grenades with its knife and movement, and claims no pool map
    // yet (docs/plans/active/dods.md).
    let dods = report.remove(report.iter().position(|c| c.game == "dods").unwrap());
    assert!(dods.playable() && dods.missing.is_empty() && dods.maps.is_empty() && dods.knife.is_some(), "{}", dods.line());
    let lent = party_roster::GAME_PARTS.iter().find(|p| p.game == "dods").unwrap().guns.iter().filter(|g| g.class != GunClass::Grenade).count();
    assert!(lent >= 12 && dods.guns.len() == lent, "{}", dods.line());
    let halo = report.remove(2);
    assert!(halo.playable() && halo.missing.is_empty() && halo.maps.is_empty(), "{}", halo.line());
    assert_eq!(halo.guns.len(), 9, "{}", halo.line());
    let parts: Vec<_> = party_roster::GAME_PARTS.iter().filter(|p| !["halo_ce", "tf2", "dods"].contains(&p.game)).collect();
    let claimed_guns = |i: usize| parts[i].guns.iter().filter(|g| g.class != GunClass::Grenade).count();
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
    // The same seed: the same ladder (with the server's memory of dealt
    // guns as it was: `auto::Recency` lightens guns dealt lately); another,
    // another.
    sim.app.world_mut().remove_resource::<chimera_sim::minigame::auto::Recency>();
    ultimate_gun_game::generate(sim.app.world_mut(), SEED, RUNGS).unwrap();
    assert_eq!(rungs(sim.app.world()), first);
    ultimate_gun_game::generate(sim.app.world_mut(), SEED + 1, RUNGS).unwrap();
    assert_ne!(rungs(sim.app.world()), first);
    // `ugg_new` makes one and plays it (its map's load and kit on the
    // tick they are asked for, whatever the machine's load).
    sim.repeatable();
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
        // The gun's game's movement (its parts'), Combat Arms' without one.
        let want = auto::report(w).iter().find(|c| c.game == game).and_then(|c| c.movement).unwrap_or(movement::ID);
        assert_eq!(w.get::<MovementSlot>(a).map(|m| m.0), Some(want), "rung {i}: {id}");
    }
}
