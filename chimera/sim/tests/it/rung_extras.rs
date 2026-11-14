//! The ladder's rung extras (`minigame::ladder::RungExtras`, a game's
//! `RungExtrasHook`) on the test kit, with a stand-in hook for the kit's
//! game: drawn once per ladder from the seeded generator, the same for
//! every player on a rung (its value applied to each, its melee weapon in
//! place of the kit's), again after a respawn and for a late joiner;
//! another seed draws others; the knife finale draws none unless a game
//! makes it its character's (the melee band); `rung_extras = false` turns
//! them off; the HUD's rung line carries the label; a character's rung
//! (`RungCharacter`) gives its weapon alone; the observers of `OnRung`
//! fire once on arriving at every rung and once on leaving it, never for
//! a respawn on the same rung; a tuned option (a table) is on and keeps
//! its table.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Hitgroup, LocalPlayer};
use chimera_sim::{
    minigame::{
        self, MinigameAppExt, MinigamePlugin,
        auto::Rng,
        hud::MinigameHud,
        ladder::{Ladder, OnRung, Rung, RungCharacter, RungDraw, RungExtras, RungExtrasAppExt, RungExtrasHook, RungSeed},
    },
    movement::placeholder,
    testkit::{self, PISTOL, RIFLE},
    weapon::{Inventory, Weapon, data::WeaponDataAppExt},
};
use chimera_world::{greybox, harness::Sim};

/// `DEF`'s options line.
const OPTIONS: &str = "rung_extras = { ghost = false, finale_ghost = false }";

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
ladder = ["kit:pistol", "kit:rifle", "kit:shotgun"]
rung_extras = { ghost = false, finale_ghost = false }
"#;

fn knife(id: &str) -> String {
    format!(
        "id = \"{id}\"\nbase = \"kit:base\"\nslot = 2\ndraw = 0.5\n\n[melee.primary]\nrange = 2.0\nhull = 0.5\n\
         facing_cos = 0.7\ndamage = 50.0\nhit_refire = 0.5\nmiss_refire = 0.4\nmiss_refire_other = 0.5\nforce = 0.0\n"
    )
}

/// What the stand-in hook put on a player: its rung's number.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
struct Drawn(String);

const MELEES: [&str; 3] = ["kit:knife_b", "kit:knife_c", "kit:knife_d"];

fn draw(_: &World, d: &RungDraw, rng: &mut Rng) -> Option<RungExtras> {
    if d.melee {
        // `finale_ghost`: the melee band's rung the ghost's, else nothing.
        return d.options.on("finale_ghost").then(|| RungExtras {
            character: Some(RungCharacter {
                id: "kit:ghost".into(),
                weapon: GHOST_KNIFE.into(),
            }),
            ..default()
        });
    }
    let n = rng.next() % 1_000_000;
    Some(RungExtras {
        values: vec![("n".into(), n.to_string())],
        label: d.options.on("label").then(|| format!("L{n}")),
        melee: d.options.on("melee").then(|| MELEES[rng.below(MELEES.len())].to_string()),
        // `ghost`: the second rung makes its players a ghost, its knife alone.
        character: (d.options.on("ghost") && d.rung == 1 && d.rungs.len() == 3).then(|| RungCharacter {
            id: "kit:ghost".into(),
            weapon: GHOST_KNIFE.into(),
        }),
        ..default()
    })
}

fn apply(world: &mut World, e: Entity, x: Option<&RungExtras>) {
    match x.and_then(|x| x.value("n")) {
        Some(n) => {
            world.entity_mut(e).insert(Drawn(n.to_string()));
        }
        None => {
            world.entity_mut(e).remove::<Drawn>();
        }
    }
}

const HOOK: RungExtrasHook = RungExtrasHook {
    game: "kit",
    options: &["label", "melee", "ghost", "finale_ghost"],
    melee: |_| Vec::new(),
    characters: |_| vec![GHOST_KNIFE],
    draw,
    apply,
};

/// The ghost's one weapon.
const GHOST_KNIFE: &str = "kit:knife_ghost";

/// The `OnRung` observers' calls, as they came: (player, "arrive" or
/// "leave", the rung).
#[derive(Resource, Default)]
struct Moments(Vec<(Entity, &'static str, usize)>);

fn count_moments(app: &mut App) {
    app.init_resource::<Moments>()
        .add_observer(|on: On<Insert<OnRung>>, q: Query<&OnRung>, mut m: ResMut<Moments>| {
            m.0.push((on.event_target(), "arrive", q.get(on.event_target()).unwrap().index));
        })
        .add_observer(|on: On<Discard<OnRung>>, q: Query<&OnRung>, mut m: ResMut<Moments>| {
            m.0.push((on.event_target(), "leave", q.get(on.event_target()).unwrap().index));
        });
}

fn sim_with(def: &str, seed: u64) -> Sim {
    let mut sim = Sim::with(chimera_sim::SimBasePlugins, |app| {
        app.add_plugins((greybox::GreyboxMapPlugin, MinigamePlugin));
        testkit::add_weapons(app);
        let docs: Vec<(String, String)> =
            ["kit:knife", GHOST_KNIFE].iter().chain(MELEES.iter()).map(|id| (format!("{id}.toml"), knife(id))).collect();
        let docs: Vec<(&str, &str)> = docs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        app.add_weapon_data(&docs).expect("the knives load");
        app.add_rung_extras(HOOK);
        count_moments(app);
        app.insert_resource(RungSeed(seed));
        app.add_minigames(&[("kit_gun_game.toml", def)]).expect("the definition reads");
    });
    minigame::start(sim.app.world_mut(), "kit:gun_game").expect("it starts");
    sim
}

fn player(sim: &mut Sim) -> Entity {
    sim.spawn_character(greybox::SPAWNS[0], placeholder::ID)
}

fn ladder(sim: &Sim) -> Ladder {
    sim.app.world().resource::<Ladder>().clone()
}

fn drawn(sim: &Sim, p: Entity) -> Option<String> {
    sim.app.world().get::<Drawn>(p).map(|d| d.0.clone())
}

fn carried(sim: &Sim, p: Entity) -> Vec<&'static str> {
    let w = sim.app.world();
    let inv = w.get::<Inventory>(p).unwrap();
    inv.weapons.iter().filter_map(|e| w.get::<Weapon>(*e).map(|x| x.id)).collect()
}

fn kill(sim: &mut Sim, p: Entity, by: Entity) {
    let at = sim.position(p);
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: Some(by),
        amount: 500.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(2);
}

fn settle(sim: &mut Sim) {
    sim.seconds(1.7);
}

/// Rung `rung`'s drawn value and melee weapon.
fn rung_set(l: &Ladder, rung: usize) -> (String, &'static str) {
    let x = l.extras(rung).expect("drawn");
    let melee = MELEES.iter().find(|m| Some(**m) == x.melee.as_deref()).expect("a melee weapon");
    (x.value("n").unwrap().to_string(), melee)
}

#[test]
fn every_player_on_a_rung_gets_its_extras_again_after_a_respawn_and_when_joining_late() {
    let mut sim = sim_with(DEF, 1);
    let l = ladder(&sim);
    assert_eq!(l.extras.len(), 3, "one per rung");
    let a = player(&mut sim);
    let b = player(&mut sim);
    settle(&mut sim);
    let (n0, melee0) = rung_set(&l, 0);
    for p in [a, b] {
        assert_eq!(drawn(&sim, p).as_deref(), Some(n0.as_str()), "the rung's value on everyone");
        assert_eq!(carried(&sim, p), [melee0, PISTOL], "the rung's melee weapon in place of the kit's knife");
    }
    // A respawn on the rung: the same.
    kill(&mut sim, b, a);
    settle(&mut sim);
    assert_eq!(sim.app.world().get::<Rung>(a).map(|r| r.0), Some(1));
    let (n1, melee1) = rung_set(&l, 1);
    assert_eq!(drawn(&sim, a), Some(n1.clone()), "the next rung's, at the swap");
    let have = carried(&sim, a);
    assert!(have.contains(&melee1) && have.contains(&RIFLE), "{have:?}");
    assert!(melee0 == melee1 || !have.contains(&melee0), "the last rung's melee weapon gone: {have:?}");
    assert_eq!(drawn(&sim, b), Some(n0.clone()), "back on rung 1 after the respawn");
    assert_eq!(carried(&sim, b), [melee0, PISTOL]);
    // A late joiner on the lowest rung (b's, the first).
    let c = player(&mut sim);
    settle(&mut sim);
    assert_eq!(drawn(&sim, c), Some(n0), "the late joiner gets the rung's");
    assert_eq!(carried(&sim, c), [melee0, PISTOL]);
}

#[test]
fn another_seed_or_another_ladder_draws_other_extras() {
    let one = ladder(&sim_with(DEF, 1)).extras;
    assert_eq!(one, ladder(&sim_with(DEF, 1)).extras, "the same seed, the same draws");
    assert_ne!(one, ladder(&sim_with(DEF, 2)).extras, "another seed, others");
    // The next ladder this host makes (the next map's) draws anew.
    let mut sim = sim_with(DEF, 1);
    minigame::start(sim.app.world_mut(), "kit:gun_game").unwrap();
    assert_ne!(one, ladder(&sim).extras);
}

#[test]
fn the_knife_finale_draws_nothing_and_keeps_its_knife() {
    let def = DEF.replace("[rules]", "[rules]\nknife_finale = \"kit:knife\"");
    let mut sim = sim_with(&def, 1);
    let l = ladder(&sim);
    assert_eq!(l.rungs.last(), Some(&"kit:knife"));
    assert!(l.extras(3).is_none(), "the finale's rung draws none");
    let a = player(&mut sim);
    let b = player(&mut sim);
    settle(&mut sim);
    for _ in 0..3 {
        kill(&mut sim, b, a);
        settle(&mut sim);
    }
    assert_eq!(sim.app.world().get::<Rung>(a).map(|r| r.0), Some(3));
    assert_eq!(carried(&sim, a), ["kit:knife"], "the finale's own knife alone");
    assert_eq!(drawn(&sim, a), None, "nothing of the hook's on it");
}

#[test]
fn rung_extras_off_draws_none_and_an_option_off_leaves_its_part_out() {
    let def = DEF.replace(OPTIONS, "rung_extras = false");
    let mut sim = sim_with(&def, 1);
    assert_eq!(ladder(&sim).extras, [None, None, None], "one per rung, none drawn");
    let a = player(&mut sim);
    let _b = player(&mut sim);
    settle(&mut sim);
    assert_eq!(drawn(&sim, a), None);
    assert_eq!(carried(&sim, a), ["kit:knife", PISTOL], "the kit's knife, as without extras");

    let def = DEF.replace(OPTIONS, "rung_extras = { melee = false, ghost = false, finale_ghost = false }");
    let mut sim = sim_with(&def, 1);
    let l = ladder(&sim);
    assert!(l.extras(0).is_some_and(|x| x.melee.is_none() && x.value("n").is_some()), "{:?}", l.extras(0));
    let a = player(&mut sim);
    let _b = player(&mut sim);
    settle(&mut sim);
    assert!(drawn(&sim, a).is_some());
    assert_eq!(carried(&sim, a), ["kit:knife", PISTOL]);
}

#[test]
fn the_huds_rung_line_carries_the_label() {
    let mut sim = sim_with(DEF, 1);
    let a = player(&mut sim);
    let _b = player(&mut sim);
    sim.app.world_mut().entity_mut(a).insert(LocalPlayer);
    settle(&mut sim);
    let n = rung_set(&ladder(&sim), 0).0;
    let lines: Vec<String> = sim.app.world().resource::<MinigameHud>().lines.iter().map(|l| l.text.clone()).collect();
    assert!(lines.iter().any(|l| l.starts_with("Rung 1/3") && l.ends_with(&format!("(L{n})"))), "{lines:?}");
}

#[test]
fn a_characters_rung_makes_its_players_the_character_holding_its_weapon_alone_until_the_next_rung() {
    let def = DEF.replace(OPTIONS, "demote_on_suicide = false\nrung_extras = { finale_ghost = false }");
    let mut sim = sim_with(&def, 1);
    let l = ladder(&sim);
    assert_eq!(l.character(1).map(|c| c.id.as_str()), Some("kit:ghost"));
    assert_eq!(l.weapon_at(1, sim.app.world().get_resource()), Some(GHOST_KNIFE), "the rung's weapon is the ghost's");
    let a = player(&mut sim);
    let b = player(&mut sim);
    settle(&mut sim);
    let moments = |sim: &Sim, p: Entity| -> Vec<(&'static str, usize)> {
        sim.app.world().resource::<Moments>().0.iter().filter(|m| m.0 == p).map(|m| (m.1, m.2)).collect()
    };
    let on = |sim: &Sim, p: Entity| sim.app.world().get::<OnRung>(p).cloned().expect("on a rung");
    for p in [a, b] {
        assert_eq!(moments(&sim, p), [("arrive", 0)], "the first rung's arrival, once");
        assert_eq!(on(&sim, p).from, None, "its first rung");
    }
    // a climbs onto the ghost's rung: its knife alone, the moments once.
    kill(&mut sim, b, a);
    settle(&mut sim);
    assert_eq!(sim.app.world().get::<Rung>(a).map(|r| r.0), Some(1));
    assert_eq!(carried(&sim, a), [GHOST_KNIFE], "the character's weapon alone: no gun, no kit");
    assert_eq!(on(&sim, a).character().map(|c| c.id.as_str()), Some("kit:ghost"));
    assert_eq!(on(&sim, a).from, Some(0));
    assert_eq!(moments(&sim, a), [("arrive", 0), ("leave", 0), ("arrive", 1)]);
    assert_eq!(moments(&sim, b), [("arrive", 0)], "b's respawn on its rung: nothing");
    // Killed by the world and back on the same rung: still the ghost, no
    // new moment.
    let at = sim.position(a);
    sim.app.world_mut().write_message(Damage {
        target: a,
        attacker: None,
        amount: 500.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(2);
    settle(&mut sim);
    assert_eq!(carried(&sim, a), [GHOST_KNIFE], "a respawn on the rung: the same");
    assert_eq!(moments(&sim, a), [("arrive", 0), ("leave", 0), ("arrive", 1)], "nothing for a respawn");
    // The next rung: itself again, its gun and the kit back, the leave once.
    kill(&mut sim, b, a);
    settle(&mut sim);
    assert_eq!(sim.app.world().get::<Rung>(a).map(|r| r.0), Some(2));
    let have = carried(&sim, a);
    assert!(!have.contains(&GHOST_KNIFE), "the ghost's weapon gone: {have:?}");
    assert!(have.contains(&"kit:shotgun"), "the next rung's gun: {have:?}");
    assert!(on(&sim, a).character().is_none());
    assert_eq!(moments(&sim, a), [("arrive", 0), ("leave", 0), ("arrive", 1), ("leave", 1), ("arrive", 2)]);
    // Gone: the leave once more.
    sim.app.world_mut().despawn(a);
    assert_eq!(moments(&sim, a).last(), Some(&("leave", 2)));
}

/// A key tuned with a table of its own (`rung_extras = { label = { ... } }`):
/// on, its table kept for the hook (`RungOptions::table`); the other keys
/// as before.
#[test]
fn a_tuned_option_is_on_and_keeps_its_table() {
    let def = DEF.replace(OPTIONS, "rung_extras = { ghost = false, finale_ghost = false, label = { often = 0.5 } }");
    let sim = sim_with(&def, 3);
    let o = &ladder(&sim).options;
    assert!(o.on("label") && !o.on("ghost") && o.on("melee"));
    assert_eq!(o.table("label").and_then(|t| t.get("often")).and_then(|v| v.as_float()), Some(0.5));
    assert!(o.table("melee").is_none());
    assert!(ladder(&sim).extras.iter().flatten().any(|x| x.label.is_some()), "the label still drawn");
}

/// The melee band (the knife finale's rung): a game's draw may make it its
/// character's (its weapon alone in place of the finale's knife), else it
/// keeps its knife.
#[test]
fn the_melee_band_is_a_characters_only_when_a_game_draws_one() {
    let def = DEF.replace(OPTIONS, "knife_finale = \"kit:knife\"\nrung_extras = { ghost = false, finale_ghost = true }");
    let mut sim = sim_with(&def, 1);
    let l = ladder(&sim);
    assert_eq!(l.rungs.len(), 4);
    assert!((0..3).all(|i| l.character(i).is_none()), "no gun rung the ghost's");
    assert_eq!(l.character(3).map(|c| c.id.as_str()), Some("kit:ghost"), "{:?}", l.extras);
    assert!(l.extras(3).is_some_and(|x| x.melee.is_none() && x.values.is_empty()));
    let a = player(&mut sim);
    let b = player(&mut sim);
    settle(&mut sim);
    for _ in 0..3 {
        kill(&mut sim, b, a);
        settle(&mut sim);
    }
    assert_eq!(sim.app.world().get::<Rung>(a).map(|r| r.0), Some(3));
    assert_eq!(carried(&sim, a), [GHOST_KNIFE], "the ghost's weapon alone, not the finale's knife");
}

/// `DEF` with `n` gun rungs and `extra` rules lines.
fn long_def(n: usize, extra: &str) -> String {
    let guns = vec!["\"kit:pistol\""; n].join(", ");
    let base = if extra.contains("rung_extras") { DEF.replace(OPTIONS, "") } else { DEF.to_string() };
    base.replace("ladder = [\"kit:pistol\", \"kit:rifle\", \"kit:shotgun\"]", &format!("ladder = [{guns}]\n{extra}"))
}

#[test]
fn the_melee_band_grows_with_the_ladder_and_each_rung_draws_its_own_melee_weapon() {
    for (guns, melee) in [(3, 1), (10, 1), (20, 1), (30, 2), (60, 4)] {
        let sim = sim_with(&long_def(guns, "knife_finale = true"), 1);
        let l = ladder(&sim);
        assert_eq!(l.melee_rungs, melee, "{guns} guns");
        assert_eq!(l.rungs.len(), guns + melee);
        let band = &l.rungs[guns..];
        assert!(band.iter().all(|w| w.starts_with("kit:knife") && *w != GHOST_KNIFE), "melee weapons, not hands: {band:?}");
        let mut distinct = band.to_vec();
        distinct.sort();
        distinct.dedup();
        assert_eq!(distinct.len(), band.len(), "no repeats where the pool (4 knives) has enough: {band:?}");
        assert!(l.knife_only(guns) && !l.knife_only(guns - 1));
    }
    // A number, and the draw varies with the seed.
    let sim = sim_with(&long_def(10, "knife_finale = true\nmelee_rungs = 3"), 1);
    assert_eq!(ladder(&sim).melee_rungs, 3);
    let seen: std::collections::BTreeSet<_> =
        (1..12).map(|s| ladder(&sim_with(&long_def(10, "knife_finale = true"), s)).rungs[10]).collect();
    assert!(seen.len() > 1, "the knife varies: {seen:?}");
    // A named weapon is the last rung's; more melee rungs than knives repeat.
    let sim = sim_with(&long_def(10, "knife_finale = \"kit:knife_c\"\nmelee_rungs = 3"), 1);
    let l = ladder(&sim);
    assert_eq!(l.rungs.last(), Some(&"kit:knife_c"));
    assert_eq!(l.rungs[10..12].iter().filter(|w| **w == "kit:knife_c").count(), 0);
    let sim = sim_with(&long_def(10, "knife_finale = true\nmelee_rungs = 6"), 1);
    assert_eq!(ladder(&sim).melee_rungs, 6);
    // Off: no melee rungs at all.
    let sim = sim_with(&long_def(10, "knife_finale = false"), 1);
    let l = ladder(&sim);
    assert_eq!((l.melee_rungs, l.rungs.len(), l.knife_finale), (0, 10, false));
    assert!(!l.knife_only(9));
}

#[test]
fn a_character_takes_at_most_one_melee_rung_and_the_last_rung_stays_melee() {
    let def = long_def(40, "knife_finale = true\nrung_extras = { ghost = false, finale_ghost = true }");
    let sim = sim_with(&def, 1);
    let l = ladder(&sim);
    assert_eq!(l.melee_rungs, 3);
    let characters: Vec<usize> = (0..l.rungs.len()).filter(|i| l.character(*i).is_some()).collect();
    assert_eq!(characters.len(), 1, "once a ladder: {:?}", l.extras);
    assert!(characters[0] >= 40, "on a melee rung: {characters:?}");
    // The infected's rung still counts as a melee rung.
    assert!(l.knife_only(characters[0]));
    assert!(l.knife_only(l.rungs.len() - 1));
}

#[test]
fn a_bad_melee_rungs_is_refused() {
    for (extra, says) in [("melee_rungs = 0", "at least 1"), ("melee_rungs = \"many\"", "not a number")] {
        let def = long_def(3, &format!("knife_finale = true\n{extra}"));
        let mut sim = Sim::with(chimera_sim::SimBasePlugins, |app| {
            app.add_plugins((greybox::GreyboxMapPlugin, MinigamePlugin));
            testkit::add_weapons(app);
            app.add_minigames(&[("kit_gun_game.toml", &def)]).ok();
        });
        let err = minigame::start(sim.app.world_mut(), "kit:gun_game").map(|_| ()).unwrap_err();
        assert!(err.contains(says), "{extra}: {err}");
    }
}

#[test]
fn a_full_game_runs_up_through_the_melee_rungs_and_the_last_ones_kill_wins() {
    use chimera_sim::mode::{Match, Phase};
    let mut sim = sim_with(&DEF.replace("[rules]", "[rules]\nknife_finale = true\nmelee_rungs = 2"), 1);
    let l = ladder(&sim);
    assert_eq!(l.rungs.len(), 5);
    let a = player(&mut sim);
    let b = player(&mut sim);
    settle(&mut sim);
    for rung in 1..5 {
        kill(&mut sim, b, a);
        settle(&mut sim);
        assert_eq!(sim.app.world().get::<Rung>(a).map(|r| r.0), Some(rung));
    }
    assert_eq!(carried(&sim, a), [l.rungs[4]], "a melee weapon alone on the last rung");
    assert_eq!(sim.app.world().resource::<Match>().phase, Phase::Live);
    kill(&mut sim, b, a);
    assert!(matches!(sim.app.world().resource::<Match>().phase, Phase::RoundOver { .. } | Phase::MatchOver), "{:?}", sim.app.world().resource::<Match>().phase);
}

/// `WeaponCatalog::weapons` is every weapon once, as `get` gives it, in
/// the order the sets first name them, and `with_hook` those naming a
/// hook; both follow the catalogue when a set is added (a ladder's draws
/// walk them instead of every set: docs/performance.md, "A new ladder's
/// frame"). The powers made twice are the same (`Powers::of` keeps them).
#[test]
fn the_catalogues_weapons_are_each_once_and_follow_its_sets() {
    use chimera_sim::{
        minigame::power::{PowerCurve, PowerReaders, Powers},
        weapon::data::WeaponCatalog,
    };
    let mut sim = sim_with(DEF, 1);
    let walk = |c: &WeaponCatalog| {
        let mut ids: Vec<&'static str> = Vec::new();
        for set in &c.sets {
            for w in &set.weapons {
                if let Some(d) = c.get(&w.id)
                    && !ids.contains(&d.id.as_str())
                {
                    ids.push(&d.id);
                }
            }
        }
        ids
    };
    let catalog = sim.app.world().resource::<WeaponCatalog>().clone();
    let ids: Vec<&str> = catalog.weapons().iter().map(|w| w.id.as_str()).collect();
    assert!(ids.len() > MELEES.len() && ids.contains(&GHOST_KNIFE));
    assert_eq!(ids, walk(&catalog));
    for w in catalog.weapons().iter() {
        assert!(std::ptr::eq(*w, catalog.get(&w.id).unwrap()), "{}: not the catalogue's own data", w.id);
    }
    let hooked = |c: &WeaponCatalog, hook: &str| -> Vec<&'static str> {
        c.weapons().iter().filter(|w| w.has_hook(hook)).map(|w| w.id.as_str()).collect()
    };
    let hook = catalog.weapons().iter().find_map(|w| w.hooks.keys().next().cloned());
    if let Some(hook) = &hook {
        let with: Vec<&str> = catalog.with_hook(hook).iter().map(|w| w.id.as_str()).collect();
        assert!(!with.is_empty());
        assert_eq!(with, hooked(&catalog, hook));
    }
    assert!(catalog.with_hook("kit:no_such_hook").is_empty());
    // A set more: the lists are the new catalogue's.
    let more = knife("kit:knife_late");
    sim.app.add_weapon_data(&[("late.toml", more.as_str())]).expect("the late knife loads");
    let later = sim.app.world().resource::<WeaponCatalog>().clone();
    let ids: Vec<&str> = later.weapons().iter().map(|w| w.id.as_str()).collect();
    assert!(ids.contains(&"kit:knife_late"));
    assert_eq!(ids, walk(&later));
    if let Some(hook) = &hook {
        let with: Vec<&str> = later.with_hook(hook).iter().map(|w| w.id.as_str()).collect();
        assert_eq!(with, hooked(&later, hook));
    }
    // Powers asked twice (the second from what the first kept), and with
    // another curve.
    let readers = sim.app.world().get_resource::<PowerReaders>().cloned().unwrap_or_default();
    let guns = || testkit::STAND_IN.guns.iter().map(|g| (g.id, g.class)).collect::<Vec<_>>();
    assert!(guns().iter().all(|(id, _)| later.get(id).is_some()), "the kit's guns are in the catalogue");
    let curve = PowerCurve::shipped();
    let first = Powers::of(guns(), &later, &readers, &curve);
    assert_eq!(first, Powers::of(guns(), &later, &readers, &curve));
    let mut other = curve.clone();
    other.score.reaction += 0.5;
    let changed = Powers::of(guns(), &later, &readers, &other);
    assert_ne!(first, changed, "another curve scores anew");
    assert_eq!(changed, Powers::of(guns(), &later, &PowerReaders::default(), &other), "the kit has no readers");
    assert_eq!(first, Powers::of(guns(), &later, &readers, &curve));
}
