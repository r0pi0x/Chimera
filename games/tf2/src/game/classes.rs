//! TF2's nine classes as data (games/tf2/data/classes.toml, built in;
//! specs/tf2/classes.md and movement.md): each class's index, max health,
//! top speed, standing eye height, air jumps, player model and arms, the
//! classes whose bodies it may also show (`needs`: their asset sets,
//! `players/<class>` by `set_of`, come with its own in a match's offer,
//! `Classes::sets_for`; none yet, the Spy's disguise will need all), and
//! which classes each gun can make its holder (`Classes::of_gun_all`: the
//! file's `[guns]` table, one class or a list, else the gun's `tf2:gun`
//! hook's `arms`; the first is the gun's own, `of_gun`).
//!
//! A gun of several classes (the Shotgun: Engineer, Soldier, Pyro, Heavy;
//! the Pistol: Scout, Engineer) gets its class when the ladder is drawn,
//! before the match: TF2's rung extras (`HOOK`, Chimera's
//! `ladder::RungExtrasHook`) draw one of the gun's classes at random from
//! the ladder's seeded generator for each TF2 rung (the value `class`, its
//! name after the gun in the HUD's rung line), kept in the ladder's
//! extras that the server shares with its clients, so every player and
//! bot on the rung, on every machine, is that class. A player arriving
//! on a rung (`ladder::OnRung`, on every machine) wears its drawn class
//! (`RungClass`, `follow_rung`); the class a held gun makes its holder is
//! that one when the gun can be it, else the gun's own (`Classes::held`).
//! Each TF2 rung's players carry its class's stock melee weapon
//! (`Class::melee`, data/classes.toml) in place of the kit's melee, as
//! Combat Arms' rungs carry theirs (`ladder::RungExtras::melee`).
//! A player holding a TF2 gun is that class (`Tf2Class`): its speed, eyes
//! and air jumps (`super::movement`), its health (`super::movement::
//! class_health`), its body (`super::bodies`), its first-person arms
//! (`super::view`), its third-person stance (`super::player_anim`), its
//! gun's per-class reload (`super::movement::select_class`) and its HUD's
//! numbers. The offer names only the drawn classes' sets
//! (`super::bodies::asset_names`).

use std::collections::BTreeMap;

use bevy::prelude::*;
use chimera_sim::minigame::{
    auto::Rng,
    ladder::{Ladder, OnRung, RungDraw, RungExtras, RungExtrasAppExt, RungExtrasHook, RungExtrasHooks},
};
use serde::Deserialize;

/// The class table and the guns' classes, as written.
pub const DATA: &str = include_str!("../../data/classes.toml");

/// One class (classes.md, "The class table").
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Class {
    /// The game's class index (1 Scout ... 9 Engineer).
    pub index: u8,
    /// Max health, points.
    pub health: f32,
    /// Top speed, units/s.
    pub speed: f32,
    /// Standing eye height, units.
    pub eye: f32,
    #[serde(default)]
    pub air_jumps: u8,
    /// Landing sound and view punch only above 580 units/s (the Scout's,
    /// movement.md "Falling and landing").
    #[serde(default)]
    pub loud_landings_only: bool,
    pub model: String,
    /// The first-person arms' class (`c_<arms>_arms.mdl`).
    pub arms: String,
    /// Its stock melee weapon (weapons_stock.md 1): a TF2 rung of this
    /// class carries it in place of the kit's melee (`HOOK`).
    #[serde(default)]
    pub melee: Option<String>,
    /// The classes whose bodies a player of this one may also show (the
    /// Spy's disguise, once built): their sets come with this class's
    /// (`Classes::sets_for`).
    #[serde(default)]
    pub needs: Vec<String>,
    /// Its name (the table's key), filled in after reading.
    #[serde(skip)]
    pub name: String,
}

/// A `[guns]` entry: one class or a list (the first the gun's own).
#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
struct Doc {
    classes: BTreeMap<String, Class>,
    #[serde(default)]
    guns: BTreeMap<String, OneOrMany>,
}

/// The classes and the guns' classes: a resource of TF2's movement
/// (`super::movement::add_movement` inserts the built-in table).
#[derive(Resource, Clone)]
pub struct Classes {
    /// By index order.
    pub classes: Vec<Class>,
    guns: BTreeMap<String, Vec<String>>,
    arms: BTreeMap<String, String>,
}

impl Classes {
    /// Parse `text` (the format of `DATA`), with the guns' `arms` from
    /// TF2's weapon data for guns the table leaves out.
    pub fn parse(text: &str) -> Result<Classes, String> {
        let doc: Doc = toml::from_str(text).map_err(|e| format!("classes.toml: {e}"))?;
        let mut classes: Vec<Class> = doc
            .classes
            .into_iter()
            .map(|(name, mut c)| {
                c.name = name;
                c
            })
            .collect();
        classes.sort_by_key(|c| c.index);
        let guns: BTreeMap<String, Vec<String>> = doc
            .guns
            .into_iter()
            .map(|(g, c)| {
                (g, match c {
                    OneOrMany::One(c) => vec![c],
                    OneOrMany::Many(c) => c,
                })
            })
            .collect();
        for (gun, list) in &guns {
            if list.is_empty() {
                return Err(format!("classes.toml: {gun} names no class"));
            }
            for class in list {
                if !classes.iter().any(|c| &c.name == class) {
                    return Err(format!("classes.toml: {gun} names no class {class}"));
                }
            }
        }
        for c in &classes {
            if let Some(n) = c.needs.iter().find(|n| !classes.iter().any(|x| &x.name == *n)) {
                return Err(format!("classes.toml: {} needs no class {n}", c.name));
            }
        }
        let arms = super::weapons::tf2_weapons()
            .weapons
            .into_iter()
            .filter_map(|w| {
                let hook = w.hooks.get(super::weapons::GUN_HOOK).or_else(|| w.hooks.get(super::weapons::MELEE_HOOK))?;
                let arms = hook.get("arms")?.as_str()?.to_string();
                Some((w.id, arms))
            })
            .collect();
        Ok(Classes { classes, guns, arms })
    }

    /// The class named `name` (`scout`).
    pub fn get(&self, name: &str) -> Option<&Class> {
        self.classes.iter().find(|c| c.name == name)
    }

    /// The class of index `index`.
    pub fn by_index(&self, index: u8) -> Option<&Class> {
        self.classes.iter().find(|c| c.index == index)
    }

    /// The classes `classes` (names) and those they need (`Class::needs`,
    /// followed through), each once, in class order: whose bodies a match
    /// with players of `classes` shows.
    pub fn with_needs<'a>(&self, classes: impl IntoIterator<Item = &'a str>) -> Vec<&Class> {
        let mut want: Vec<&str> = classes.into_iter().collect();
        let mut i = 0;
        while let Some(name) = want.get(i).copied() {
            for n in self.get(name).map(|c| c.needs.as_slice()).unwrap_or_default() {
                if !want.contains(&n.as_str()) {
                    want.push(n);
                }
            }
            i += 1;
        }
        self.classes.iter().filter(|c| want.contains(&c.name.as_str())).collect()
    }

    /// Every class the guns among `guns` can make their holder
    /// (`of_gun_all`) with those they need (`with_needs`): the bodies a
    /// kit handing out `guns` may show, whatever its ladder draws.
    pub fn for_guns<'a>(&self, guns: impl IntoIterator<Item = &'a str>) -> Vec<&Class> {
        let all: Vec<&str> = guns.into_iter().flat_map(|g| self.of_gun_all(g)).map(|c| c.name.as_str()).collect();
        self.with_needs(all)
    }

    /// The asset sets of classes `classes` (names) and those they need
    /// (`with_needs`): `players/<class>` each (`set_of`), in class order.
    pub fn sets_of<'a>(&self, classes: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        self.with_needs(classes).into_iter().map(|c| set_of(&c.name)).collect()
    }

    /// The asset sets of the guns' own classes (`of_gun`, nothing drawn)
    /// and those they need: `players/<class>` each (`set_of`).
    pub fn sets_for<'a>(&self, guns: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        let own: Vec<&str> = guns.into_iter().filter_map(|g| self.of_gun(g)).map(|c| c.name.as_str()).collect();
        self.sets_of(own)
    }

    /// The gun's own class (the first `of_gun_all`): what it makes its
    /// holder where no class was drawn.
    pub fn of_gun(&self, id: &str) -> Option<&Class> {
        self.of_gun_all(id).into_iter().next()
    }

    /// Every class gun `id` can make its holder, its own first: the
    /// `[guns]` table's, else the class whose arms hold it; none for
    /// another game's gun.
    pub fn of_gun_all(&self, id: &str) -> Vec<&Class> {
        if let Some(list) = self.guns.get(id) {
            return list.iter().filter_map(|c| self.get(c)).collect();
        }
        let Some(arms) = self.arms.get(id) else { return Vec::new() };
        self.classes.iter().find(|c| &c.arms == arms || &c.name == arms).into_iter().collect()
    }

    /// The class gun `id` makes a holder whose rung drew `drawn`
    /// (`RungClass`): that one when the gun can be it, else the gun's own.
    pub fn held(&self, id: &str, drawn: Option<&str>) -> Option<&Class> {
        let all = self.of_gun_all(id);
        drawn
            .and_then(|d| all.iter().find(|c| c.name == d).copied())
            .or_else(|| all.first().copied())
    }

    /// One of gun `id`'s classes drawn on `rng` (its only one without
    /// drawing; none for another game's gun).
    pub fn draw(&self, id: &str, rng: &mut Rng) -> Option<&Class> {
        let all = self.of_gun_all(id);
        match all.len() {
            0 => None,
            1 => Some(all[0]),
            n => Some(all[rng.below(n)]),
        }
    }
}

/// The rung extras' value naming a TF2 rung's drawn class.
pub const CLASS_VALUE: &str = "class";
/// The rung extras' option for the class's melee (`rung_extras = {
/// melee = false }`: the kit's own).
pub const MELEE_OPTION: &str = "melee";

/// TF2's rung extras (module doc): each TF2 rung's class, drawn with the
/// ladder, and its melee weapon.
pub const HOOK: RungExtrasHook = RungExtrasHook {
    game: super::GAME,
    options: &[CLASS_VALUE, MELEE_OPTION],
    melee: |_| Vec::new(),
    characters: |_| Vec::new(),
    draw: draw_rung,
    apply: |_, _, _| {},
};

/// A TF2 rung's draw: one of its gun's classes (`Classes::draw`), named
/// after the gun in the HUD when the gun has several, and that class's
/// melee weapon (`Class::melee`: the Scout's Bat with the Scattergun),
/// which the rung's players carry in place of the kit's melee
/// (`ladder::RungExtras::melee`, Chimera's `runtime::KitMelee`: the kit's
/// weapon of its slot; a kit without one there gets none). None on the
/// melee band; with `rung_extras = { class = false }` the gun's own class
/// (its melee, no drawn class); with `{ melee = false }` no melee.
fn draw_rung(world: &World, d: &RungDraw, rng: &mut Rng) -> Option<RungExtras> {
    if d.melee {
        return None;
    }
    let table = world.get_resource::<Classes>().cloned().unwrap_or_default();
    let drawn = d.options.on(CLASS_VALUE);
    let class = if drawn { table.draw(d.weapon, rng)? } else { table.of_gun(d.weapon)? };
    let melee = class.melee.clone().filter(|_| d.options.on(MELEE_OPTION));
    if !drawn && melee.is_none() {
        return None;
    }
    let several = drawn && table.of_gun_all(d.weapon).len() > 1;
    Some(RungExtras {
        game: super::GAME.into(),
        values: if drawn { vec![(CLASS_VALUE.into(), class.name.clone())] } else { Vec::new() },
        label: several.then(|| title(&class.name)),
        melee,
        ..default()
    })
}

/// A class's name as the HUD shows it (`demoman` -> `Demoman`).
fn title(name: &str) -> String {
    let mut c = name.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// The class `extras` drew (a TF2 rung's), if any.
pub fn drawn_class(extras: Option<&RungExtras>) -> Option<&str> {
    extras.filter(|x| x.game == super::GAME)?.value(CLASS_VALUE)
}

/// The classes `ladder`'s TF2 rungs make their players: each rung's drawn
/// class, else its gun's own; each once, in rung order.
pub fn ladder_classes<'a>(table: &'a Classes, ladder: &Ladder) -> Vec<&'a Class> {
    let mut out: Vec<&Class> = Vec::new();
    for (i, gun) in ladder.rungs.iter().enumerate() {
        if let Some(c) = table.held(gun, drawn_class(ladder.extras(i)))
            && !out.iter().any(|x| x.name == c.name)
        {
            out.push(c);
        }
    }
    out
}

/// The class a player's rung drew (`HOOK`), on every machine from its
/// `ladder::OnRung` (`follow_rung`); empty off a TF2 rung. On every
/// character from its spawn, empty (docs/DESIGN.md, "ECS style").
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct RungClass(pub String);

/// A player arrived on a rung (`ladder::OnRung`, on every machine): its
/// drawn class on, or off on a rung that drew none.
pub fn follow_rung(on: On<Insert<OnRung>>, rungs: Query<(&OnRung, Option<&RungClass>)>, mut commands: Commands) {
    let e = on.event_target();
    let Ok((rung, have)) = rungs.get(e) else { return };
    match drawn_class(rung.extras.as_ref()) {
        Some(c) if have.is_none_or(|h| h.0 != c) => {
            commands.entity(e).insert(RungClass(c.to_string()));
        }
        // Kept on, empty (no class: `Classes::held` takes the gun's own):
        // put on and off it was an archetype for every combination of the
        // player's other parts (docs/DESIGN.md, "ECS style").
        None if have.is_some_and(|h| !h.0.is_empty()) => {
            commands.entity(e).try_insert(RungClass(String::new()));
        }
        _ => {}
    }
}

/// TF2's rung classes on an app: the hook and the observer (once).
pub fn add_rung_classes(app: &mut App) {
    let has = app
        .world()
        .get_resource::<RungExtrasHooks>()
        .is_some_and(|h| h.0.iter().any(|x| x.game == super::GAME));
    if has {
        return;
    }
    use chimera_core::character::CharacterPartsExt;
    app.add_rung_extras(HOOK).add_observer(follow_rung).on_every_character::<RungClass>();
}

/// The asset set of class `name`'s body (`chimera_pack sets tf2`,
/// `super::sets::class_files`): its player model with its animations and
/// textures, its first-person arms and its voice's waves, `players/<name>`
/// (`players/heavy`).
pub fn set_of(name: &str) -> String {
    format!("players/{name}")
}

/// The built-in table, parsed (a fresh copy each call: systems read the
/// `Classes` resource).
pub fn classes() -> Classes {
    Classes::parse(DATA).unwrap_or_else(|e| panic!("{e}"))
}

impl Default for Classes {
    fn default() -> Self {
        classes()
    }
}

/// The class a player without a TF2 gun in hand is until it holds one
/// (the Soldier).
pub const DEFAULT: &str = "soldier";

/// A TF2 player's class (its index), from the gun in hand
/// (`super::movement::select_class`, predicted: every machine works it out
/// from the same inventory). The default is the Soldier (index 3).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tf2Class(pub u8);

impl Tf2Class {
    /// Its row of `table` (the default class's when the index has none).
    pub fn class<'a>(&self, table: &'a Classes) -> &'a Class {
        table
            .by_index(self.0)
            .or_else(|| table.get(DEFAULT))
            .unwrap_or_else(|| table.classes.first().expect("a class"))
    }
}

impl Default for Tf2Class {
    fn default() -> Self {
        Tf2Class(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is classes.md's (test case 1) and every gun the brief
    /// names has its class.
    #[test]
    fn the_table_is_the_specs() {
        let c = classes();
        let row = |n: &str| {
            let c = c.get(n).unwrap();
            (c.health, c.speed, c.eye)
        };
        assert_eq!(row("scout"), (125.0, 400.0, 65.0));
        assert_eq!(row("soldier"), (200.0, 240.0, 68.0));
        assert_eq!(row("pyro"), (175.0, 300.0, 68.0));
        assert_eq!(row("demoman"), (175.0, 280.0, 68.0));
        assert_eq!(row("heavy"), (300.0, 230.0, 75.0));
        assert_eq!(row("engineer"), (125.0, 300.0, 68.0));
        assert_eq!(row("medic"), (150.0, 320.0, 75.0));
        assert_eq!(row("sniper"), (125.0, 300.0, 75.0));
        assert_eq!(row("spy"), (125.0, 320.0, 75.0));
        assert_eq!(c.classes.iter().map(|c| c.index).collect::<Vec<_>>(), (1..=9).collect::<Vec<_>>());
        let of = |g: &str| c.of_gun(g).map(|c| c.name.as_str());
        assert_eq!(of("tf2:tf_weapon_scattergun"), Some("scout"));
        assert_eq!(of("tf2:tf_weapon_rocketlauncher"), Some("soldier"));
        assert_eq!(of("tf2:tf_weapon_minigun"), Some("heavy"));
        assert_eq!(of("tf2:tf_weapon_shotgun_primary"), Some("engineer"));
        assert_eq!(of("tf2:tf_weapon_pistol_scout"), Some("scout"));
        assert_eq!(of("tf2:tf_weapon_sniperrifle"), Some("sniper"));
        assert_eq!(of("tf2:tf_weapon_flamethrower"), Some("pyro"));
        assert_eq!(of("cs_source:weapon_ak47"), None);
        assert_eq!(c.get("scout").unwrap().air_jumps, 1);
        assert_eq!(c.get("heavy").unwrap().air_jumps, 0);
        assert_eq!(Tf2Class::default().class(&c).name, DEFAULT);
    }

    /// A class's sets are its own and those of the classes it needs,
    /// followed through, each once; an unknown need is refused.
    #[test]
    fn needs_pull_in_other_classes_sets() {
        let c = classes();
        let names = |v: Vec<&Class>| v.into_iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        assert_eq!(
            c.sets_for(["tf2:tf_weapon_minigun", "tf2:tf_weapon_scattergun", "tf2:tf_weapon_pistol_scout", "x:y"]),
            ["players/scout", "players/heavy"]
        );
        assert!(names(c.for_guns(["tf2:tf_weapon_revolver"])) == ["spy"], "no disguise yet: the Spy alone");
        let text = DATA
            .replace("needs = []", "needs = [\"medic\"]")
            .replace("arms = \"medic\"", "arms = \"medic\"\nneeds = [\"pyro\"]");
        let t = Classes::parse(&text).unwrap();
        assert_eq!(names(t.for_guns(["tf2:tf_weapon_revolver"])), ["medic", "pyro", "spy"]);
        assert!(Classes::parse(&DATA.replace("needs = []", "needs = [\"nobody\"]")).is_err());
    }

    /// The guns of several classes list every class that carries them in
    /// stock TF2, their own first; a draw over many seeds gives each, a
    /// gun of one class always that one; a drawn class the gun can't be
    /// falls back to its own.
    #[test]
    fn several_classes_drawn_at_random() {
        let c = classes();
        let names = |v: Vec<&Class>| v.into_iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(c.of_gun_all("tf2:tf_weapon_shotgun_primary")), ["engineer", "soldier", "pyro", "heavy"]);
        assert_eq!(names(c.of_gun_all("tf2:tf_weapon_pistol_scout")), ["scout", "engineer"]);
        assert_eq!(names(c.of_gun_all("tf2:tf_weapon_minigun")), ["heavy"]);
        assert!(c.of_gun_all("cs_source:weapon_ak47").is_empty());
        let mut seen = std::collections::BTreeMap::<String, u32>::new();
        for seed in 0..400 {
            let class = c.draw("tf2:tf_weapon_shotgun_primary", &mut Rng::new(seed)).unwrap();
            *seen.entry(class.name.clone()).or_default() += 1;
            assert_eq!(c.draw("tf2:tf_weapon_minigun", &mut Rng::new(seed)).unwrap().name, "heavy");
        }
        assert_eq!(seen.keys().cloned().collect::<Vec<_>>(), ["engineer", "heavy", "pyro", "soldier"], "{seen:?}");
        assert!(seen.values().all(|n| *n > 50), "roughly even: {seen:?}");
        assert_eq!(c.held("tf2:tf_weapon_shotgun_primary", Some("pyro")).unwrap().name, "pyro");
        assert_eq!(c.held("tf2:tf_weapon_shotgun_primary", Some("scout")).unwrap().name, "engineer");
        assert_eq!(c.held("tf2:tf_weapon_shotgun_primary", None).unwrap().name, "engineer");
        assert_eq!(c.held("tf2:tf_weapon_minigun", Some("pyro")).unwrap().name, "heavy");
        assert_eq!(title("demoman"), "Demoman");
        // A list naming an unknown class is refused.
        let bad = DATA.replace("\"pyro\", \"heavy\"]", "\"pyro\", \"nobody\"]");
        assert!(bad != DATA && Classes::parse(&bad).is_err());
    }
}
