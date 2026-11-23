//! DoD:S's classes on the party's rungs (M3b of docs/plans/active/dods.md;
//! specs/dods/classes.md, "Loadouts"): the twelve classes as data
//! (games/dods/data/classes.toml, built in): each class's body (a
//! `bodies::CLASSES` id), its melee weapon and the weapons it carries,
//! primary first. A DoD:S weapon makes its holder one of the classes that
//! carry it (`Classes::of_gun_all`; its own class the first listing it).
//!
//! A weapon of several classes (the Colt, the P38, the frags, the knife and
//! spade) gets its class when the ladder is drawn, before the match, as
//! TF2's do: DoD:S's rung extras (`HOOK`, Chimera's
//! `ladder::RungExtrasHook`) draw one of its classes from the ladder's
//! seeded generator for each DoD:S rung (the value `class`, its name after
//! the gun in the HUD's rung line), kept in the ladder's extras that the
//! server shares with its clients, so every player on the rung, on every
//! machine, is that class. A player arriving on a rung (`ladder::OnRung`,
//! on every machine) carries its drawn class (`RungClass`, `follow_rung`);
//! the class a held weapon makes its holder is that one when the weapon can
//! be it, else the weapon's own (`Classes::held`). A DoD:S rung's players
//! carry their class's melee weapon (the knife for the Allies, the spade
//! for the Axis; the Assault classes none: the kit's stays) in place of the
//! kit's (`ladder::RungExtras::melee`).
//!
//! What the class decides: the body (`bodies::wear_classes`). Every class
//! has 100 health and runs at 220 (classes.md, "Health and speed"), so
//! nothing else follows it.
//!
//! Known shortcuts:
//! - No team: a free-for-all's players wear the class's own nation's body
//!   (a Garand is always American), not their team's.

use std::collections::BTreeMap;

use bevy::prelude::*;
use chimera_core::game_data::GameData;
use chimera_sim::minigame::{
    auto::Rng,
    ladder::{Ladder, OnRung, RungDraw, RungExtras, RungExtrasAppExt, RungExtrasHook, RungExtrasHooks},
};
use serde::Deserialize;

/// The classes and what they carry, as written.
pub const DATA: GameData = chimera_core::game_data!("data/classes.toml");

/// One class (classes.md, "Loadouts").
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Class {
    /// Its class body's id (`dods:us_rifleman`, `bodies::CLASSES`).
    pub id: String,
    /// Its melee weapon, carried on a DoD:S rung of the class in place of
    /// the kit's (none: the Assault classes').
    #[serde(default)]
    pub melee: Option<String>,
    /// The weapons it carries, primary first.
    pub guns: Vec<String>,
}

impl Class {
    /// Its body's player model.
    pub fn model(&self) -> &'static str {
        super::bodies::class(&self.id).map_or(super::bodies::DEFAULT_MODEL, |b| b.model)
    }

    /// Its name as the HUD shows it (`US Machine Gunner`).
    pub fn name(&self) -> &'static str {
        super::bodies::class(&self.id).map_or("", |b| b.name)
    }
}

#[derive(Deserialize)]
struct Doc {
    classes: Vec<Class>,
}

/// The classes in the file's order (Allies, then Axis).
#[derive(Resource, Clone, Debug)]
pub struct Classes {
    pub classes: Vec<Class>,
    guns: BTreeMap<String, Vec<usize>>,
}

impl Classes {
    /// Parse `text` (the format of `DATA`): every class's body must be one
    /// of `bodies::CLASSES`, every listed weapon one of DoD:S's.
    pub fn parse(text: &str) -> Result<Classes, String> {
        let doc: Doc = toml::from_str(text).map_err(|e| format!("classes.toml: {e}"))?;
        let mut guns: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, c) in doc.classes.iter().enumerate() {
            if super::bodies::class(&c.id).is_none() {
                return Err(format!("classes.toml: no class body {}", c.id));
            }
            for g in c.guns.iter().chain(c.melee.iter()) {
                if !g.starts_with("dods:") {
                    return Err(format!("classes.toml: {} carries {g}, not DoD:S's", c.id));
                }
                let list = guns.entry(g.clone()).or_default();
                if !list.contains(&i) {
                    list.push(i);
                }
            }
        }
        Ok(Classes { classes: doc.classes, guns })
    }

    /// The class of body id `id`.
    pub fn get(&self, id: &str) -> Option<&Class> {
        self.classes.iter().find(|c| c.id == id)
    }

    /// Every class weapon `id` can make its holder, its own first (file
    /// order); none for another game's weapon.
    pub fn of_gun_all(&self, id: &str) -> Vec<&Class> {
        self.guns.get(id).map(|l| l.iter().map(|i| &self.classes[*i]).collect()).unwrap_or_default()
    }

    /// The weapon's own class (the first carrying it).
    pub fn of_gun(&self, id: &str) -> Option<&Class> {
        self.of_gun_all(id).into_iter().next()
    }

    /// The class weapon `id` makes a holder whose rung drew `drawn`: that
    /// one when the weapon can be it, else the weapon's own.
    pub fn held(&self, id: &str, drawn: Option<&str>) -> Option<&Class> {
        let all = self.of_gun_all(id);
        drawn.and_then(|d| all.iter().find(|c| c.id == d).copied()).or_else(|| all.first().copied())
    }

    /// One of weapon `id`'s classes drawn on `rng` (its only one without
    /// drawing).
    pub fn draw(&self, id: &str, rng: &mut Rng) -> Option<&Class> {
        let all = self.of_gun_all(id);
        match all.len() {
            0 => None,
            1 => Some(all[0]),
            n => Some(all[rng.below(n)]),
        }
    }
}

/// The built-in table, parsed.
pub fn classes() -> Classes {
    Classes::parse(&DATA).unwrap_or_else(|e| panic!("{e}"))
}

impl Default for Classes {
    fn default() -> Self {
        classes()
    }
}

/// The rung extras' value naming a DoD:S rung's drawn class.
pub const CLASS_VALUE: &str = "class";
/// The rung extras' option for the class's melee (`rung_extras = {
/// melee = false }`: the kit's own).
pub const MELEE_OPTION: &str = "melee";

/// DoD:S's rung extras (module doc): each DoD:S rung's class, drawn with
/// the ladder, and its melee weapon.
pub const HOOK: RungExtrasHook = RungExtrasHook {
    game: super::GAME,
    options: &[CLASS_VALUE, MELEE_OPTION],
    melee: |_| Vec::new(),
    characters: |_| Vec::new(),
    draw: draw_rung,
    apply: |_, _, _| {},
};

/// A DoD:S rung's draw: one of its weapon's classes (`Classes::draw`;
/// with `rung_extras = { class = false }` the weapon's own), named after
/// the gun in the HUD when the weapon has several, and that class's melee
/// weapon. None on the melee band.
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
        values: if drawn { vec![(CLASS_VALUE.into(), class.id.clone())] } else { Vec::new() },
        label: several.then(|| class.name().to_string()),
        melee,
        ..default()
    })
}

/// The class `extras` drew (a DoD:S rung's), if any.
pub fn drawn_class(extras: Option<&RungExtras>) -> Option<&str> {
    extras.filter(|x| x.game == super::GAME)?.value(CLASS_VALUE)
}

/// The classes `ladder`'s DoD:S rungs make their players: each rung's drawn
/// class, else its weapon's own; each once, in rung order.
pub fn ladder_classes<'a>(table: &'a Classes, ladder: &Ladder) -> Vec<&'a Class> {
    let mut out: Vec<&Class> = Vec::new();
    for (i, gun) in ladder.rungs.iter().enumerate() {
        if let Some(c) = table.held(gun, drawn_class(ladder.extras(i)))
            && !out.iter().any(|x| x.id == c.id)
        {
            out.push(c);
        }
    }
    out
}

/// The class a player's rung drew (`HOOK`), on every machine from its
/// `ladder::OnRung` (`follow_rung`); empty off a DoD:S rung. On every
/// character from its spawn, empty (docs/DESIGN.md, "ECS style").
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct RungClass(pub String);

/// A player arrived on a rung (`ladder::OnRung`, on every machine): its
/// drawn class on, or emptied on a rung that drew none.
pub fn follow_rung(on: On<Insert<OnRung>>, rungs: Query<(&OnRung, Option<&RungClass>)>, mut commands: Commands) {
    let e = on.event_target();
    let Ok((rung, have)) = rungs.get(e) else { return };
    match drawn_class(rung.extras.as_ref()) {
        Some(c) if have.is_none_or(|h| h.0 != c) => {
            commands.entity(e).insert(RungClass(c.to_string()));
        }
        None if have.is_some_and(|h| !h.0.is_empty()) => {
            commands.entity(e).try_insert(RungClass(String::new()));
        }
        _ => {}
    }
}

/// DoD:S's rung classes on an app: the table, the hook and the observer
/// (once).
pub fn add_rung_classes(app: &mut App) {
    app.init_resource::<Classes>();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::weapons as w;

    /// The table is classes.md's: twelve classes, each weapon's own class
    /// its primary's, the shared ones listing every class that carries
    /// them, every DoD:S weapon some class's.
    #[test]
    fn the_table_is_the_specs() {
        let c = classes();
        assert_eq!(c.classes.len(), 12);
        let own = |g: &str| c.of_gun(g).map(|c| c.id.as_str());
        assert_eq!(own(w::GARAND), Some("dods:us_rifleman"));
        assert_eq!(own(w::MG42), Some("dods:axis_mg"));
        assert_eq!(own(w::PSCHRECK), Some("dods:axis_rocket"));
        assert_eq!(own(w::C96), Some("dods:axis_rocket"));
        assert_eq!(own(w::M1CARBINE), Some("dods:us_rocket"));
        assert_eq!(own("cs_source:weapon_ak47"), None);
        let all = |g: &str| c.of_gun_all(g).into_iter().map(|c| c.id.as_str()).collect::<Vec<_>>();
        assert_eq!(all(w::COLT), ["dods:us_assault", "dods:us_sniper", "dods:us_mg"]);
        assert_eq!(all(w::FRAG_GER), ["dods:axis_assault", "dods:axis_support"]);
        assert_eq!(all(w::SPADE).len(), 5);
        for g in w::GUNS.iter().chain(w::MELEE).chain(w::EXPLOSIVES) {
            assert!(c.of_gun(g).is_some(), "{g} is no class's");
        }
        assert_eq!(c.get("dods:us_assault").unwrap().melee, None);
        assert_eq!(c.get("dods:axis_mg").unwrap().model(), "models/player/german_mg.mdl");
        assert!(Classes::parse(&DATA.replace("dods:us_mg\"", "dods:us_nobody\"")).is_err());
    }

    /// A shared weapon's class is drawn among its classes; a drawn class
    /// the held weapon can't be falls back to its own.
    #[test]
    fn shared_weapons_draw_a_class() {
        let c = classes();
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..200 {
            seen.insert(c.draw(w::P38, &mut Rng::new(seed)).unwrap().id.clone());
            assert_eq!(c.draw(w::GARAND, &mut Rng::new(seed)).unwrap().id, "dods:us_rifleman");
        }
        assert_eq!(seen.len(), 3, "{seen:?}");
        assert_eq!(c.held(w::KNIFE, Some("dods:us_mg")).unwrap().id, "dods:us_mg");
        assert_eq!(c.held(w::KNIFE, Some("dods:axis_mg")).unwrap().id, "dods:us_rifleman");
    }
}
