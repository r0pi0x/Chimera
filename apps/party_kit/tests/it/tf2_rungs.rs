//! TF2's guns as rungs (docs/plans/active/tf2.md, M2b): the original
//! `originals:gun_game_2fort` (minigames/gun_game_2fort.toml), a ladder
//! that mixes TF2's four guns with CS:S's, HL2:DM's and Combat Arms' (no
//! Halo), on the party's simulation headless on the greybox: a TF2 rung
//! gives its TF2 gun and makes the player TF2's (its identity and
//! movement) and the gun's class (its max health and top speed), a kill
//! with it climbs to the next rung, another game's gun, and the identity
//! follows the gun (the max health back to the party's); the Rocket
//! Launcher's rung kills too, with a rocket.

use bevy::prelude::*;
use chimera_core::{
    core::Health,
    slots::{GameIdentity, MovementSlot},
};
use chimera_sim::minigame::{self, ladder::Ladder};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::harness::CaSim;
use party_kit::PartyPartsPlugin;

use crate::gun_game::{held, line_up, no_finale, plain_rungs, rung, shoot_until_dead, soldier};

fn party() -> Sim {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    plain_rungs(sim.app.world_mut());
    no_finale(sim.app.world_mut());
    minigame::start(sim.app.world_mut(), originals::GUN_GAME_2FORT).expect("the gun game starts");
    sim
}

fn identity(w: &World, e: Entity) -> Option<String> {
    w.get::<GameIdentity>(e).map(|g| g.0.clone())
}

fn movement(w: &World, e: Entity) -> Option<&'static str> {
    w.get::<MovementSlot>(e).map(|m| m.0)
}

fn set_rung(sim: &mut Sim, e: Entity, r: usize) {
    let w = sim.app.world_mut();
    let l = w.resource::<Ladder>().clone();
    minigame::ladder::set_rung(w, e, r, &l);
    sim.seconds(1.2);
}

#[test]
fn a_tf2_rung_gives_its_gun_and_a_kill_climbs_to_the_next_games() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let tf2_rungs: Vec<usize> = (0..ladder.rungs.len()).filter(|r| ladder.rungs[*r].starts_with("tf2:")).collect();
    assert_eq!(tf2_rungs.len(), 4, "TF2's four guns");
    assert!(!ladder.rungs.iter().any(|w| w.starts_with("halo_ce:")), "no Halo");
    // The Scattergun's rung: TF2's gun, TF2's identity and movement.
    let r = tf2_rungs[0];
    set_rung(&mut sim, a, r);
    assert_eq!(held(&sim, a), Some(tf2::game::weapons::SCATTERGUN));
    let w = sim.app.world();
    assert_eq!(identity(w, a).as_deref(), Some("tf2"));
    assert_eq!(movement(w, a), Some(tf2::game::movement::ID));
    // The Scout's: 125 health, 400 units/s.
    let h = *w.get::<Health>(a).unwrap();
    assert!((h.max - 1.25).abs() < 1e-6 && (h.current - 1.25).abs() < 1e-6, "{h:?}");
    let traits = w.get::<source_engine::source_movement::SourceTraits>(a).unwrap();
    assert_eq!(traits.max_speed, Some(400.0));
    // A kill with it: the next rung, another game's gun, its identity.
    line_up(&mut sim, a, b, 3.0);
    assert!(shoot_until_dead(&mut sim, a, b, 10.0), "the Scattergun kills");
    sim.ticks(2);
    assert_eq!(rung(&sim, a), r + 1);
    let next = ladder.rungs[r + 1];
    assert!(!next.starts_with("tf2:"), "{next}");
    assert_eq!(held(&sim, a), Some(next));
    let game = next.split_once(':').unwrap().0;
    assert_eq!(identity(sim.app.world(), a).as_deref(), Some(game), "the identity follows {next}");
    let h = *sim.app.world().get::<Health>(a).unwrap();
    assert!((h.max - 1.0).abs() < 1e-6, "the party's max health back: {h:?}");
    // Each TF2 rung's gun in hand, and TF2's identity with it.
    for r in tf2_rungs {
        set_rung(&mut sim, a, r);
        let gun = ladder.rungs[r];
        assert_eq!(held(&sim, a), Some(gun));
        assert_eq!(identity(sim.app.world(), a).as_deref(), Some("tf2"), "{gun}");
    }
}

#[test]
fn the_rocket_launchers_rung_kills_with_a_rocket() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let r = ladder.rungs.iter().position(|w| *w == tf2::game::weapons::ROCKET_LAUNCHER).unwrap();
    set_rung(&mut sim, a, r);
    assert_eq!(held(&sim, a), Some(tf2::game::weapons::ROCKET_LAUNCHER));
    line_up(&mut sim, a, b, 6.0);
    assert!(shoot_until_dead(&mut sim, a, b, 12.0), "rockets kill");
    sim.ticks(2);
    assert_eq!(rung(&sim, a), r + 1);
}

/// TF2's HUD in the party (`tf2::ui::party::CLIENT`): up while the local
/// player holds a TF2 gun, its root shown, the other game's HUD up again
/// on another game's rung; while it is up the shell's crosshair and status
/// are off (`GameDraws`), and its declared kill feed stays empty (the
/// app's feed shows its kills).
#[test]
fn tf2s_hud_follows_its_gun() {
    use chimera_client::client::hud::GameDraws;
    let (mut sim, me, _) = crate::kill_feed::every_hud_sim_and(true, |_| {});
    crate::hud::swap_to(&mut sim, me, 6);
    assert_eq!(crate::hud::hud(&sim), tf2::game::GAME, "TF2's gun, TF2's HUD");
    assert!(crate::hud::shown(&mut sim, tf2::game::GAME));
    let draws = *sim.app.world().resource::<GameDraws>();
    assert!(draws.crosshair && draws.status && !draws.killfeed, "{draws:?}");
    let feeds = chimera_client::client::kill_feed::GameKillFeeds::lines(sim.app.world());
    assert!(feeds.contains(&(tf2::game::GAME, 0)), "{feeds:?}");
    crate::hud::swap_to(&mut sim, me, 0);
    assert_ne!(crate::hud::hud(&sim), tf2::game::GAME);
    assert!(!crate::hud::shown(&mut sim, tf2::game::GAME));
}

/// weapons_more.md's eight guns as rungs (the brief's ladder on the
/// greybox, TF2 among the weapon games): each gives its gun and TF2's
/// identity; the Pistol, Revolver, Flame Thrower, Syringe Gun and Grenade
/// Launcher each kill.
#[test]
fn the_next_eight_tf2_guns_are_rungs_and_kill() {
    use tf2::game::weapons::{
        FLAMETHROWER, GRENADE_LAUNCHER, PISTOL_SCOUT, REVOLVER, SMG, SNIPER_RIFLE, STICKY_LAUNCHER, SYRINGE_GUN,
    };
    let guns = [PISTOL_SCOUT, SMG, SNIPER_RIFLE, REVOLVER, GRENADE_LAUNCHER, STICKY_LAUNCHER, FLAMETHROWER, SYRINGE_GUN];
    let text = crate::hl2dm_parts::ladder_text(&guns, false)
        .replace("\"hl2dm\", \"combat_arms\"]", "\"hl2dm\", \"tf2\", \"combat_arms\"]");
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    crate::gun_game::no_finale(sim.app.world_mut());
    use chimera_sim::minigame::MinigameAppExt;
    sim.app.add_minigames(&[("tf2_more.toml", &text)]).unwrap();
    minigame::start(sim.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    for (r, gun) in guns.iter().enumerate() {
        set_rung(&mut sim, a, r);
        assert_eq!(held(&sim, a), Some(*gun));
        assert_eq!(identity(sim.app.world(), a).as_deref(), Some("tf2"), "{gun}");
    }
    for gun in [GRENADE_LAUNCHER, PISTOL_SCOUT, REVOLVER, FLAMETHROWER, SYRINGE_GUN] {
        let r = guns.iter().position(|g| *g == gun).unwrap();
        set_rung(&mut sim, a, r);
        line_up(&mut sim, a, b, 3.0);
        // The Syringe Gun's rung is the last: its kill wins the match.
        assert!(shoot_until_dead(&mut sim, a, b, 12.0), "{gun} kills");
    }
}

/// The weapon ids `e` carries.
fn carried(sim: &Sim, e: Entity) -> Vec<&'static str> {
    let w = sim.app.world();
    w.get::<chimera_sim::weapon::Inventory>(e)
        .map(|i| i.weapons.iter().filter_map(|x| w.get::<chimera_sim::weapon::Weapon>(*x).map(|x| x.id)).collect())
        .unwrap_or_default()
}

/// A test copy of gun_game_2fort whose kit by game names TF2's knife (as
/// an automatic gun game's does: TF2's `PARTS.knife`), started with rung
/// seed `seed`, no finale.
fn melee_party(seed: u64) -> Sim {
    use chimera_sim::minigame::{MinigameAppExt, ladder::RungSeed};
    let text = originals::MINIGAMES
        .iter()
        .find(|(f, _)| f.ends_with("gun_game_2fort.toml"))
        .unwrap()
        .1
        .replace("id = \"originals:gun_game_2fort\"", "id = \"tests:tf2_melee\"")
        .replace("hl2dm = [\"hl2dm:weapon_crowbar\"] }", "hl2dm = [\"hl2dm:weapon_crowbar\"], tf2 = [\"tf2:tf_weapon_knife\"] }");
    assert!(text.contains("tf2 = [\"tf2:tf_weapon_knife\"]"));
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    sim.app.insert_resource(RungSeed(seed));
    sim.app.add_minigames(&[("tests/tf2_melee.toml", text.as_str())]).unwrap();
    no_finale(sim.app.world_mut());
    minigame::start(sim.app.world_mut(), "tests:tf2_melee").expect("the gun game starts");
    sim
}

/// A TF2 rung carries its class's stock melee weapon (specs/tf2/
/// weapons_stock.md 9; `tf2::game::classes::HOOK`'s rung extras, the
/// mechanism Combat Arms' rung melee uses: `runtime::KitMelee` in place
/// of the kit's knife): the Scout's Bat with the Scattergun, the Heavy's
/// Fists with the Minigun, the Soldier's Shovel with the Rocket Launcher,
/// the drawn class's with the Shotgun; another game's rung after it
/// carries that game's knife again. On a test copy of gun_game_2fort
/// whose kit by game names TF2's knife (as an automatic gun game's does:
/// TF2's `PARTS.knife`); the shipped definition is unchanged.
#[test]
fn a_tf2_rung_carries_its_classs_melee() {
    use tf2::game::{classes, weapons as w};
    let mut sim = melee_party(0);
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let table = classes::classes();
    let mut seen = 0;
    for (r, gun) in ladder.rungs.iter().enumerate() {
        set_rung(&mut sim, a, r);
        let has = carried(&sim, a);
        if gun.starts_with("tf2:") {
            let drawn = classes::drawn_class(ladder.extras(r));
            let class = table.held(gun, drawn).unwrap();
            let want = class.melee.as_deref().unwrap();
            assert!(has.contains(&want), "{gun} ({}): {has:?}", class.name);
            assert!(!has.contains(&w::KNIFE) || want == w::KNIFE, "the kit's knife swapped: {has:?}");
            match *gun {
                w::SCATTERGUN => assert_eq!(want, w::BAT),
                w::MINIGUN => assert_eq!(want, w::FISTS),
                w::ROCKET_LAUNCHER => assert_eq!(want, w::SHOVEL),
                _ => {}
            }
            seen += 1;
        } else {
            assert!(!has.iter().any(|h| w::MELEE.contains(h)), "{gun}: no TF2 melee off TF2's rungs: {has:?}");
        }
    }
    assert_eq!(seen, 4);
}

/// The melee follows the class drawn for a gun of several classes (the
/// Shotgun: Engineer, Soldier, Pyro, Heavy): over rung seeds, the
/// Shotgun's rung's extras carry the drawn class's melee (the Soldier's
/// Shovel, the Heavy's Fists, the Pyro's Fire Axe, the Engineer's
/// Wrench), and a player on it carries that melee in place of the kit's
/// knife and is that class with the Shotgun in hand.
#[test]
fn the_shotguns_drawn_class_carries_its_own_melee() {
    use std::collections::BTreeMap;
    use tf2::game::{classes, weapons as w};
    let table = classes::classes();
    let want = |c: &str| match c {
        "soldier" => w::SHOVEL,
        "heavy" => w::FISTS,
        "pyro" => w::FIREAXE,
        "engineer" => w::WRENCH,
        other => panic!("the Shotgun drew {other}"),
    };
    // The first seed drawing each class.
    let mut seeds: BTreeMap<String, u64> = BTreeMap::new();
    for seed in 0..40 {
        let sim = melee_party(seed);
        let l = sim.app.world().resource::<Ladder>().clone();
        let r = l.rungs.iter().position(|g| *g == w::SHOTGUN).expect("the Shotgun's rung");
        let x = l.extras(r).expect("the Shotgun's rung drew");
        let drawn = classes::drawn_class(Some(x)).expect("a drawn class").to_string();
        assert_eq!(x.melee.as_deref(), Some(want(&drawn)), "seed {seed}: {drawn}");
        assert_eq!(table.get(&drawn).and_then(|c| c.melee.as_deref()), Some(want(&drawn)));
        seeds.entry(drawn).or_insert(seed);
        if seeds.len() == 4 {
            break;
        }
    }
    assert!(seeds.contains_key("soldier") && seeds.contains_key("heavy"), "the Soldier and the Heavy drawn: {seeds:?}");
    for (class, seed) in [("soldier", seeds["soldier"]), ("heavy", seeds["heavy"])] {
        let mut sim = melee_party(seed);
        let a = soldier(&mut sim);
        let _b = soldier(&mut sim);
        sim.seconds(1.5);
        let l = sim.app.world().resource::<Ladder>().clone();
        let r = l.rungs.iter().position(|g| *g == w::SHOTGUN).unwrap();
        set_rung(&mut sim, a, r);
        let has = carried(&sim, a);
        assert!(has.contains(&want(class)), "{class} with the Shotgun: {has:?}");
        assert!(!has.contains(&w::KNIFE), "the kit's knife swapped: {has:?}");
        for other in w::MELEE.iter().filter(|m| **m != want(class)) {
            assert!(!has.contains(other), "{class}: another class's melee {other}: {has:?}");
        }
        assert_eq!(held(&sim, a), Some(w::SHOTGUN));
        let index = table.get(class).unwrap().index;
        assert_eq!(sim.app.world().get::<classes::Tf2Class>(a).map(|c| c.0), Some(index), "{class} with the Shotgun in hand");
    }
}
