//! Rung passives (`minigame::passives`, `ladder::RungExtras::passives`;
//! docs/plans/active/rung-passives.md) on the test kit, with a stand-in
//! hook and table for the kit's game: off by default (nothing drawn, the
//! ladder's other draws unchanged); on, drawn from the seed the same on
//! every run, at the odds (0 never, 1 always), at most `max`, one per
//! `group`; the conflict rule (the gun's slot, its keys); none on the
//! melee band; the definition's table over the game's; the item given
//! on arriving (kept over a respawn), taken on leaving the rung; a
//! passive's melee kill climbs and never demotes.

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
/// boots share no group, the spurs are the boots' group.
const TABLE: &str = r#"
odds = 0.25
max = 1
slot_names = ["primary", "secondary", "melee"]
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
"#;

const ITEMS: [&str; 4] = ["kit:boots", "kit:spurs", "kit:shield", "kit:pack"];

fn def(passives: &str) -> String {
    DEF.replace("PASSIVES", passives)
}

fn draw(world: &World, d: &RungDraw, rng: &mut Rng) -> Option<RungExtras> {
    let table = PassiveTable::parse(TABLE).unwrap();
    let slot = world.resource::<WeaponCatalog>().get(d.weapon).map_or(0, |w| w.slot);
    let gun = RungGun {
        class: "*",
        slot: table.gun_slot(d.weapon, "*", slot),
        keys: table.gun_keys(d.weapon),
    };
    let passives = passives::draw(&table, d, &gun, rng, &|id| passives::registered(world, id));
    (!passives.is_empty()).then(|| RungExtras { passives, ..default() })
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
        docs.extend(ITEMS.iter().map(|id| (format!("{id}.toml"), item(id))));
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

#[test]
fn passives_are_off_by_default_and_change_no_other_draw() {
    for seed in 0..16 {
        let l = ladder(&sim_with(&def(""), seed));
        assert!(l.extras.iter().all(|x| x.is_none()), "seed {seed}: {:?}", l.extras);
    }
    let l = ladder(&sim_with(&def("rung_extras = { passives = { on = false, odds = 1 } }"), 1));
    assert!(l.extras.iter().all(|x| x.is_none()), "`on = false` wins over the odds");
    let l = ladder(&sim_with(&def("rung_extras = false"), 1));
    assert!(l.extras.iter().all(|x| x.is_none()));
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
    let drawn: usize = (0..80).map(|s| passives_of(&ladder(&sim_with(&on, s))).iter().filter(|r| !r.is_empty()).count()).sum();
    assert!((30..=90).contains(&drawn), "{drawn} of 240 at 0.25");
}

#[test]
fn max_two_takes_two_of_different_groups() {
    let two = def("rung_extras = { passives = { odds = 1, max = 2 } }");
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
