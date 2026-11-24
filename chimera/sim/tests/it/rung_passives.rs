//! Rung passives (`minigame::passives`, `ladder::RungExtras::passives`;
//! docs/plans/active/rung-passives.md) on the test kit, with a stand-in
//! hook and table for the kit's game: on by default (turned off by the
//! definition), the ladder's other draws the same on or off (the draw's
//! own stream); drawn from the seed the same on every run, at the odds
//! (0 never, 1 always), at most `max`, one per `group`, a second rarer
//! (`next`: the defaults 0.2, 2 and a quarter, over many seeds); only the
//! rung character's own game's items; the conflict rule (the gun's slot,
//! its keys); the utility items under their own option, never in a slot
//! a passive or the gun takes; none on the melee band; the definition's
//! table over the game's; the item given on arriving (kept over a
//! respawn), taken on leaving the rung; a passive's melee kill climbs
//! and never demotes.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Hitgroup};
use chimera_sim::{
    minigame::{
        self, MinigameAppExt, MinigamePlugin,
        auto::Rng,
        ladder::{Ladder, Rung, RungDraw, RungExtras, RungExtrasAppExt, RungExtrasHook, RungSeed},
        passives::{self, PassiveTable, RungGun, RungPassive},
    },
    movement::placeholder,
    testkit::{self, PISTOL, RIFLE, SHOTGUN},
    weapon::{Inventory, Weapon, data::{WeaponCatalog, WeaponDataAppExt}},
};
use chimera_world::{greybox, harness::Sim};

const DEF: &str = r#"
id = "kit:gun_game"
name = "Kit gun game"
players = { min = 2, max = 4 }
maps = ["chimera:greybox"]
teams = "free_for_all"
time_limit = "60s"

[spawn]
respawn = { delay = "1s", protection = "0.5s" }
points = "map"
kit = ["kit:knife"]

[rules]
module = "chimera:ladder"
ladder = ["kit:rifle", "kit:pistol", "kit:shotgun"]
PASSIVES
"#;

/// The kit game's table: boots take the secondary slot (never with the
/// Pistol), the shield attack2 (never with the Rifle), the pack and the
/// boots share no group, the spurs are the boots' group; another game's
/// cape (never drawn on the kit's rungs); the jar a utility item in the
/// secondary slot (its odds 0 unless a definition tunes them).
const TABLE: &str = r#"
odds = 0.25
max = 1
slot_names = ["primary", "secondary", "melee"]
[utility]
odds = 0
[gun_keys]
"kit:rifle" = ["attack2"]
[[passive]]
item = "kit:boots"
slot = "secondary"
group = "feet"
[[passive]]
item = "kit:spurs"
slot = "secondary"
group = "feet"
[[passive]]
item = "kit:shield"
slot = "melee"
keys = ["attack2"]
[[passive]]
item = "kit:pack"
group = "back"
[[passive]]
item = "other:cape"
group = "back"
weight = 50
[[passive]]
item = "kit:jar"
slot = "secondary"
kind = "active"
"#;

const ITEMS: [&str; 4] = ["kit:boots", "kit:spurs", "kit:shield", "kit:pack"];
/// Registered too: another game's item, a utility item.
const MORE: [&str; 2] = ["other:cape", "kit:jar"];
/// The value the stand-in hook rolls on the ladder's generator (the
/// passives' draw mustn't move it).
const ROLL: &str = "roll";

fn def(passives: &str) -> String {
    DEF.replace("PASSIVES", passives)
}

fn draw(world: &World, d: &RungDraw, rng: &mut Rng) -> Option<RungExtras> {
    let table = PassiveTable::parse(TABLE).unwrap();
    let slot = world.resource::<WeaponCatalog>().get(d.weapon).map_or(0, |w| w.slot);
    let gun = RungGun {
        game: "kit",
        class: "*",
        slot: table.gun_slot(d.weapon, "*", slot),
        keys: table.gun_keys(d.weapon),
    };
    let passives = passives::draw(&table, d, &gun, rng, &|id| passives::registered(world, id));
    // Another draw on the ladder's generator after the passives'.
    let roll = rng.below(1_000_000).to_string();
    Some(RungExtras { passives, values: vec![(ROLL.into(), roll)], ..default() })
}

const HOOK: RungExtrasHook = RungExtrasHook {
    game: "kit",
    options: &[],
    melee: |_| Vec::new(),
    characters: |_| Vec::new(),
    draw,
    apply: |world, e, x| passives::wear(world, e, "kit", &x.map(|x| x.passives.clone()).unwrap_or_default()),
};

fn knife(id: &str) -> String {
    format!(
        "id = \"{id}\"\nbase = \"kit:base\"\nslot = 2\ndraw = 0.5\n\n[melee.primary]\nrange = 2.0\nhull = 0.5\n\
         facing_cos = 0.7\ndamage = 50.0\nhit_refire = 0.5\nmiss_refire = 0.4\nmiss_refire_other = 0.5\nforce = 0.0\n"
    )
}

fn item(id: &str) -> String {
    format!("id = \"{id}\"\nbase = \"kit:base\"\nslot = 4\ndraw = 0.5\n")
}

fn sim_with(def: &str, seed: u64) -> Sim {
    let mut sim = Sim::with(chimera_sim::SimBasePlugins, |app| {
        app.add_plugins((greybox::GreyboxMapPlugin, MinigamePlugin));
        testkit::add_weapons(app);
        let mut docs = vec![("kit:knife.toml".to_string(), knife("kit:knife"))];
        docs.extend(ITEMS.iter().chain(&MORE).map(|id| (format!("{id}.toml"), item(id))));
        let docs: Vec<(&str, &str)> = docs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        app.add_weapon_data(&docs).expect("the weapons load");
        app.add_rung_extras(HOOK);
        app.insert_resource(RungSeed(seed));
        app.add_minigames(&[("kit_gun_game.toml", def)]).expect("the definition reads");
    });
    minigame::start(sim.app.world_mut(), "kit:gun_game").expect("it starts");
    sim
}

fn ladder(sim: &Sim) -> Ladder {
    sim.app.world().resource::<Ladder>().clone()
}

fn passives_of(l: &Ladder) -> Vec<Vec<String>> {
    (0..l.rungs.len()).map(|r| l.extras(r).map(|x| x.passives.clone()).unwrap_or_default()).collect()
}

fn player(sim: &mut Sim) -> Entity {
    sim.spawn_character(greybox::SPAWNS[0], placeholder::ID)
}

fn worn(sim: &Sim, p: Entity) -> Vec<&'static str> {
    let w = sim.app.world();
    let inv = w.get::<Inventory>(p).unwrap();
    inv.weapons.iter().filter(|e| w.get::<RungPassive>(**e).is_some()).filter_map(|e| w.get::<Weapon>(*e).map(|x| x.id)).collect()
}

fn kill(sim: &mut Sim, p: Entity, by: Entity, kind: DamageKind, weapon: Option<&'static str>) {
    let at = sim.position(p);
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: Some(by),
        amount: 500.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind,
        weapon,
        force: Vec3::ZERO,
    });
    sim.ticks(2);
}

fn settle(sim: &mut Sim) {
    sim.seconds(1.7);
}

fn rung(sim: &Sim, p: Entity) -> usize {
    sim.app.world().get::<Rung>(p).map_or(0, |r| r.0)
}

fn rolls(l: &Ladder) -> Vec<Option<String>> {
    l.extras.iter().map(|x| x.as_ref().and_then(|x| x.value(ROLL)).map(str::to_string)).collect()
}

#[test]
fn passives_are_on_by_default_and_change_no_other_draw() {
    let on: usize = (0..48).map(|s| passives_of(&ladder(&sim_with(&def(""), s))).iter().filter(|r| !r.is_empty()).count()).sum();
    assert!(on > 0, "on by default");
    for seed in 0..16 {
        let with = ladder(&sim_with(&def("rung_extras = { passives = { odds = 1 } }"), seed));
        for off in ["rung_extras = { passives = false }", "rung_extras = { passives = { on = false, odds = 1 } }"] {
            let l = ladder(&sim_with(&def(off), seed));
            assert!(passives_of(&l).iter().all(Vec::is_empty), "seed {seed}, {off}: {:?}", l.extras);
            assert_eq!(rolls(&l), rolls(&with), "seed {seed}: the ladder's other draws are the same with passives on or off");
        }
    }
    let l = ladder(&sim_with(&def("rung_extras = false"), 1));
    assert!(l.extras.iter().all(|x| x.is_none()));
}

#[test]
fn the_defaults_are_a_fifth_two_at_most_the_second_at_a_quarter() {
    let t = PassiveTable::parse("").unwrap();
    assert_eq!((t.odds, t.max, t.next), (0.2, 2, 0.25));
    assert_eq!((t.utility.odds, t.utility.max), (0.2, 1));
    // Over many seeds: about a fifth of the rungs draw one, and a second
    // comes at a quarter of that (5 % of those with one), never more often
    // than a fifth of the firsts.
    let d = def("rung_extras = { passives = { odds = 0.2, max = 2, next = 0.25 } }");
    let (mut rungs, mut one, mut two) = (0, 0, 0);
    for seed in 0..400 {
        // The Shotgun's rung may draw any two (no slot or key taken).
        let p = passives_of(&ladder(&sim_with(&d, seed)));
        rungs += 1;
        one += (!p[2].is_empty()) as usize;
        two += (p[2].len() == 2) as usize;
    }
    assert!((55..=110).contains(&one), "{one} of {rungs} drew one at 0.2");
    assert!((1..=12).contains(&two) && two * 5 <= one, "{two} of {one} drew a second at 0.2 x 0.25");
}

#[test]
fn only_the_rung_characters_own_games_items_are_drawn() {
    // The cape (another game's, weight 50) is never drawn on the kit's
    // rungs, whatever the odds.
    let always = def("rung_extras = { passives = { odds = 1, max = 2 } }");
    for seed in 0..32 {
        let p = passives_of(&ladder(&sim_with(&always, seed)));
        assert!(p.iter().flatten().all(|i| i.starts_with("kit:")), "seed {seed}: {p:?}");
        assert!(p.iter().any(|r| !r.is_empty()));
    }
    // The helper keeps to the game it's given.
    let table = PassiveTable::parse(TABLE).unwrap();
    let opts = chimera_sim::minigame::ladder::RungOptions { keys: [("passives".to_string(), true)].into(), tables: [("passives".to_string(), toml::from_str("odds = 1").unwrap())].into(), ..default() };
    let defn = minigame::def::MinigameDef::parse("kit_gun_game.toml", &def("")).unwrap();
    let rungs = ["kit:shotgun"];
    let d = RungDraw { def: &defn, rung: 0, weapon: "kit:shotgun", options: &opts, rungs: &rungs, drawn: &[], melee: false };
    for game in ["kit", "other"] {
        let gun = RungGun { game, class: "*", slot: "primary".into(), keys: &[] };
        for s in 0..16 {
            let got = passives::draw(&table, &d, &gun, &Rng::new(s), &|_| true);
            assert!(!got.is_empty() && got.iter().all(|i| passives::game_of(i) == game), "{game}: {got:?}");
        }
    }
}

#[test]
fn utility_items_have_their_own_option_and_never_share_a_slot() {
    // Off at the table's odds 0; at 1, the jar on every rung whose gun and
    // passive leave the secondary slot free.
    let d = def("rung_extras = { passives = false, utility = { odds = 1 } }");
    for seed in 0..16 {
        let p = passives_of(&ladder(&sim_with(&d, seed)));
        assert_eq!(p[0], ["kit:jar"], "seed {seed}: the Rifle's rung: {p:?}");
        assert!(p[1].is_empty(), "seed {seed}: the Pistol takes the secondary slot: {p:?}");
        assert_eq!(p[2], ["kit:jar"], "seed {seed}: {p:?}");
    }
    let both = def("rung_extras = { passives = { odds = 1 }, utility = { odds = 1 } }");
    for seed in 0..32 {
        let p = passives_of(&ladder(&sim_with(&both, seed)));
        for r in &p {
            let boots = r.iter().any(|i| ["kit:boots", "kit:spurs"].contains(&i.as_str()));
            assert!(!(boots && r.contains(&"kit:jar".to_string())), "seed {seed}: the boots took the secondary slot: {p:?}");
        }
    }
    let off = def("rung_extras = { passives = false, utility = false }");
    assert!(passives_of(&ladder(&sim_with(&off, 3))).iter().all(Vec::is_empty));
    assert!(passives_of(&ladder(&sim_with(&def("rung_extras = { passives = false }"), 3))).iter().all(Vec::is_empty), "the table's utility odds 0");
}

#[test]
fn on_the_draw_follows_the_seed_the_odds_and_the_conflict_rule() {
    let always = def("rung_extras = { passives = { odds = 1 } }");
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..48 {
        let p = passives_of(&ladder(&sim_with(&always, seed)));
        assert_eq!(p, passives_of(&ladder(&sim_with(&always, seed))), "seed {seed}: the same seed, the same draw");
        assert!(p.iter().all(|r| r.len() == 1), "seed {seed}: odds 1, one each (max 1): {p:?}");
        // The Rifle (primary, attack2): no shield; the Pistol (secondary):
        // no boots or spurs.
        assert!(p[0][0] != "kit:shield", "seed {seed}: {p:?}");
        assert!(!["kit:boots", "kit:spurs"].contains(&p[1][0].as_str()), "seed {seed}: {p:?}");
        seen.extend(p.into_iter().flatten());
    }
    assert_eq!(seen.len(), ITEMS.len(), "every item drawn somewhere: {seen:?}");
    let never = def("rung_extras = { passives = { odds = 0 } }");
    assert!(passives_of(&ladder(&sim_with(&never, 3))).iter().all(Vec::is_empty), "odds 0: never");
    // A quarter of the rungs over many seeds at the table's odds.
    let on = def("rung_extras = { passives = true }");
    assert_eq!(passives_of(&ladder(&sim_with(&on, 7))), passives_of(&ladder(&sim_with(&def(""), 7))), "on by default");
    let drawn: usize = (0..80).map(|s| passives_of(&ladder(&sim_with(&on, s))).iter().filter(|r| !r.is_empty()).count()).sum();
    assert!((30..=90).contains(&drawn), "{drawn} of 240 at 0.25");
}

#[test]
fn max_two_takes_two_of_different_groups() {
    let two = def("rung_extras = { passives = { odds = 1, max = 2, next = 1 } }");
    for seed in 0..32 {
        let p = passives_of(&ladder(&sim_with(&two, seed)));
        // The Shotgun's rung (primary, no keys): two, never both feet.
        assert_eq!(p[2].len(), 2, "seed {seed}: {p:?}");
        let feet = p[2].iter().filter(|i| ["kit:boots", "kit:spurs"].contains(&i.as_str())).count();
        assert!(feet <= 1, "seed {seed}: {p:?}");
    }
}

#[test]
fn the_melee_band_draws_no_passive() {
    let d = def("knife_finale = \"kit:knife\"\nrung_extras = { passives = { odds = 1 } }");
    for seed in 0..8 {
        let l = ladder(&sim_with(&d, seed));
        assert_eq!(l.rungs.last(), Some(&"kit:knife"));
        assert!(l.extras.last().unwrap().is_none(), "seed {seed}: {:?}", l.extras);
    }
}

#[test]
fn the_rungs_item_is_worn_kept_over_a_respawn_and_taken_on_leaving() {
    let d = def("demote_on = \"melee_death\"\nrung_extras = { passives = { odds = 1 } }");
    let mut sim = sim_with(&d, 5);
    let l = ladder(&sim);
    let p = passives_of(&l);
    let a = player(&mut sim);
    let b = player(&mut sim);
    settle(&mut sim);
    assert_eq!(worn(&sim, a), [p[0][0].as_str()], "the rung's item worn");
    assert_eq!(worn(&sim, b), [p[0][0].as_str()]);
    // b dies on the rung (by a's rifle): a climbs, b respawns with it.
    kill(&mut sim, b, a, DamageKind::Bullet, Some(RIFLE));
    settle(&mut sim);
    assert_eq!(rung(&sim, a), 1);
    assert_eq!(worn(&sim, a), [p[1][0].as_str()], "the next rung's in place of the first's");
    assert_eq!(worn(&sim, b), [p[0][0].as_str()], "kept over the respawn");
    // The guns stay as they were (the item takes no gun's place).
    let held: Vec<&str> = {
        let w = sim.app.world();
        w.get::<Inventory>(a).unwrap().weapons.iter().filter_map(|e| w.get::<Weapon>(*e).map(|x| x.id)).collect()
    };
    assert!(held.contains(&PISTOL) && held.contains(&"kit:knife") && !held.contains(&SHOTGUN), "{held:?}");
}

#[test]
fn a_passive_kill_climbs_and_never_demotes() {
    let d = def("demote_on = \"melee_death\"\nrung_extras = { passives = { odds = 1 } }");
    let mut sim = sim_with(&d, 5);
    let p = passives_of(&ladder(&sim));
    let a = player(&mut sim);
    let b = player(&mut sim);
    settle(&mut sim);
    kill(&mut sim, a, b, DamageKind::Bullet, Some(RIFLE));
    settle(&mut sim);
    assert_eq!((rung(&sim, a), rung(&sim, b)), (0, 1));
    // a's item (rung 1's: a stomp, a bash) kills b on rung 2 with melee
    // damage: a climbs, b keeps his rung.
    let item: &'static str = Box::leak(p[0][0].clone().into_boxed_str());
    kill(&mut sim, b, a, DamageKind::Melee, Some(item));
    settle(&mut sim);
    assert_eq!((rung(&sim, a), rung(&sim, b)), (1, 1), "the passive's kill counts and steals no rung");
    // The kit's knife does demote.
    kill(&mut sim, b, a, DamageKind::Melee, Some("kit:knife"));
    settle(&mut sim);
    assert_eq!((rung(&sim, a), rung(&sim, b)), (2, 0));
}
