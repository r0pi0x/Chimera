//! Bullet impacts across games (docs/DESIGN.md, "Impacts across games"):
//! any game's gun on any game's map leaves an impact. What an impact
//! shows is picked from the hit surface, and each game names its surfaces
//! its own way (CS:S's surface properties and game material letters,
//! Combat Arms' numbered surface types), so Chimera keeps a small common
//! vocabulary, `SurfaceClass` (concrete, metal, wood, dirt, ...), and each
//! game registers how its names map onto it and back (`GameSurfaces`, in
//! the `SurfaceClasses` resource): a hit on a Combat Arms wall is
//! "concrete" to a CS:S gun's effects, a CS:S `metal` crate "Metal" to a
//! Combat Arms gun's.
//!
//! The map says whose impact looks it carries (`MapImpacts`, spawned as
//! `ActiveImpacts`): its own game's from its loader, a guest's from its
//! dressing (the party's Combat Arms maps carry CS:S's decals and effect
//! materials, `map::sources` guests). Which game draws an impact is
//! `chimera_sim::weapon::impact`'s choice: the gun's game, else the map's,
//! else Chimera's generic impact.

use bevy::prelude::*;

/// The common surface classes every game's surfaces map onto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SurfaceClass {
    Concrete,
    Metal,
    Wood,
    Dirt,
    Sand,
    Grass,
    Snow,
    Glass,
    Tile,
    Plastic,
    Cloth,
    Flesh,
    Water,
    /// Nothing better known: drawn as concrete.
    Default,
}

impl SurfaceClass {
    pub const ALL: [SurfaceClass; 14] = [
        Self::Concrete,
        Self::Metal,
        Self::Wood,
        Self::Dirt,
        Self::Sand,
        Self::Grass,
        Self::Snow,
        Self::Glass,
        Self::Tile,
        Self::Plastic,
        Self::Cloth,
        Self::Flesh,
        Self::Water,
        Self::Default,
    ];

    /// The colour Chimera's generic impact draws for the class (linear
    /// RGB; ours, by eye).
    pub fn mark_color(self) -> Vec3 {
        match self {
            Self::Metal => Vec3::new(0.08, 0.08, 0.09),
            Self::Wood => Vec3::new(0.12, 0.07, 0.03),
            Self::Dirt | Self::Grass => Vec3::new(0.10, 0.07, 0.04),
            Self::Sand => Vec3::new(0.30, 0.24, 0.15),
            Self::Snow => Vec3::new(0.55, 0.57, 0.60),
            Self::Glass => Vec3::new(0.35, 0.38, 0.40),
            Self::Flesh => Vec3::new(0.25, 0.01, 0.01),
            _ => Vec3::new(0.04, 0.035, 0.03),
        }
    }

    /// The colour of the dust a hit kicks up (linear RGB; ours).
    pub fn dust_color(self) -> Vec3 {
        match self {
            Self::Metal => Vec3::new(1.0, 0.8, 0.4),
            Self::Wood => Vec3::new(0.45, 0.32, 0.18),
            Self::Dirt | Self::Grass => Vec3::new(0.35, 0.28, 0.18),
            Self::Sand => Vec3::new(0.70, 0.60, 0.42),
            Self::Snow | Self::Water => Vec3::new(0.85, 0.88, 0.92),
            Self::Flesh => Vec3::new(0.5, 0.02, 0.02),
            _ => Vec3::new(0.55, 0.53, 0.50),
        }
    }

    /// A class from a plain English surface name (any game's names that
    /// say what they are: `concrete`, `Metal Door`, `Desert_Sand`), for
    /// games whose own table doesn't know a name.
    pub fn guess(name: &str) -> SurfaceClass {
        let n = name.to_ascii_lowercase();
        let has = |w: &[&str]| w.iter().any(|w| n.contains(w));
        if has(&["flesh", "player", "body", "head", "alien", "blood"]) {
            Self::Flesh
        } else if has(&["water", "slime", "liquid", "wade"]) {
            Self::Water
        } else if has(&["glass", "window"]) {
            Self::Glass
        } else if has(&["metal", "grate", "vent", "chain", "pipe", "computer", "canister", "gun"]) {
            Self::Metal
        } else if has(&["wood", "crate", "plank", "ladder"]) {
            Self::Wood
        } else if has(&["sand"]) {
            Self::Sand
        } else if has(&["snow", "ice"]) {
            Self::Snow
        } else if has(&["grass", "foliage"]) {
            Self::Grass
        } else if has(&["dirt", "mud", "gravel", "road", "ground"]) {
            Self::Dirt
        } else if has(&["tile", "ceramic", "porcelain", "pottery", "linoleum"]) {
            Self::Tile
        } else if has(&["plastic", "rubber", "cardboard", "paper"]) {
            Self::Plastic
        } else if has(&["cloth", "carpet", "fabric"]) {
            Self::Cloth
        } else if has(&["concrete", "cement", "rock", "stone", "brick", "plaster", "boulder"]) {
            Self::Concrete
        } else {
            Self::Default
        }
    }
}

/// How one game's surface names map onto the classes and back.
#[derive(Clone, Copy, Debug)]
pub struct GameSurfaces {
    pub game: &'static str,
    /// The class of one of the game's surface names (None: unknown, then
    /// `SurfaceClass::guess`).
    pub classify: fn(&str) -> Option<SurfaceClass>,
    /// The game's surface name for a class (the name its own effects
    /// look up), None where it has nothing like it.
    pub name: fn(SurfaceClass) -> Option<&'static str>,
}

/// Every registered game's `GameSurfaces`.
#[derive(Resource, Clone, Debug, Default)]
pub struct SurfaceClasses(pub Vec<GameSurfaces>);

impl SurfaceClasses {
    pub fn of(&self, game: &str) -> Option<&GameSurfaces> {
        self.0.iter().find(|g| g.game.eq_ignore_ascii_case(game))
    }

    /// The class of `game`'s surface `name` (its table's, else a guess
    /// from the name).
    pub fn classify(&self, game: Option<&str>, name: &str) -> SurfaceClass {
        game.and_then(|g| self.of(g))
            .and_then(|g| (g.classify)(name))
            .unwrap_or_else(|| SurfaceClass::guess(name))
    }

    /// `name`, a surface of `from`'s, as `to` names it: unchanged within
    /// one game; else by class (`Default`'s name, then `None` when `to`
    /// has neither).
    pub fn translate(&self, from: Option<&str>, name: &str, to: &str) -> Option<String> {
        if from.is_some_and(|f| f.eq_ignore_ascii_case(to)) {
            return Some(name.to_string());
        }
        let class = self.classify(from, name);
        let target = self.of(to)?;
        (target.name)(class)
            .or_else(|| (target.name)(SurfaceClass::Default))
            .map(str::to_string)
    }
}

/// Register a game's surface classes (`SurfaceClasses`).
pub trait SurfaceClassesAppExt {
    fn register_surface_classes(&mut self, surfaces: GameSurfaces) -> &mut Self;
}

impl SurfaceClassesAppExt for App {
    fn register_surface_classes(&mut self, surfaces: GameSurfaces) -> &mut Self {
        let mut all = self.world_mut().get_resource_or_init::<SurfaceClasses>();
        if all.of(surfaces.game).is_none() {
            all.0.push(surfaces);
        }
        self
    }
}

/// A map's game and the games whose impact looks (decals, effect
/// materials, impact sounds) it carries: its loader adds its own game,
/// each dressing its game. Empty `looks`: not told (hand-made maps,
/// tests): every game's impacts are taken to be there.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MapImpacts {
    /// The map's own game (`cs_source` for de_dust2), set by
    /// `sources::MapSources` (else the first of `looks`).
    pub game: Option<String>,
    pub looks: Vec<String>,
}

impl MapImpacts {
    /// Note that `game`'s impact looks are on the map.
    pub fn add(&mut self, game: &str) {
        if !self.looks.iter().any(|g| g.eq_ignore_ascii_case(game)) {
            self.looks.push(game.to_string());
        }
    }

    pub fn map_game(&self) -> Option<&str> {
        self.game.as_deref().or(self.looks.first().map(String::as_str))
    }

    /// Whether `game`'s impacts can be drawn on the map.
    pub fn has(&self, game: &str) -> bool {
        self.looks.is_empty() || self.looks.iter().any(|g| g.eq_ignore_ascii_case(game))
    }
}

/// The loaded map's `MapImpacts`.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ActiveImpacts(pub MapImpacts);

#[cfg(test)]
mod tests {
    use super::*;

    fn a_name(c: SurfaceClass) -> Option<&'static str> {
        match c {
            SurfaceClass::Metal => Some("metal"),
            SurfaceClass::Default => Some("default"),
            _ => None,
        }
    }

    #[test]
    fn names_go_between_games_by_class() {
        let classes = SurfaceClasses(vec![
            GameSurfaces {
                game: "a",
                classify: |n| (n == "20").then_some(SurfaceClass::Metal),
                name: |_| None,
            },
            GameSurfaces {
                game: "b",
                classify: |_| None,
                name: a_name,
            },
        ]);
        assert_eq!(classes.translate(Some("a"), "20", "b").as_deref(), Some("metal"));
        assert_eq!(classes.translate(Some("a"), "5", "b").as_deref(), Some("default"));
        assert_eq!(classes.translate(Some("b"), "x", "b").as_deref(), Some("x"));
        assert_eq!(classes.translate(Some("b"), "metal", "a"), None);
        assert_eq!(classes.classify(None, "Desert_Sand"), SurfaceClass::Sand);
        assert_eq!(classes.classify(None, "Metal Door"), SurfaceClass::Metal);
    }

    #[test]
    fn a_map_without_looks_has_every_games() {
        let mut m = MapImpacts::default();
        assert!(m.has("anything"));
        m.add("a");
        assert!(m.has("A") && !m.has("b"));
        assert_eq!(m.map_game(), Some("a"));
    }
}
