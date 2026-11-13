//! Where an app's maps come from, when the map and the gameplay may be two
//! games' (docs/DESIGN.md, "Slots": the map is a slot of its own): each
//! game offers its map loader (`MapLoader`: `game:name` from its install
//! to `MapData`) and the parts its gameplay brings to any map
//! (`MapDressing`: its view models, character bodies, effect looks); an
//! app composes them (`MapSources`), so a Combat Arms soldier plays on a
//! Counter-Strike: Source map with neither game knowing the other.
//!
//! - A map of the gameplay's own game loads as its loader makes it (the
//!   loader already brings the game's parts).
//! - A map of another game is the world only: what its game brings to
//!   every map (`MapData::kit`, a `GameKit`: bodies, held and view models,
//!   HUD, radio, bot names and navigation, round sounds, muzzle flash,
//!   shells) is not even read: `load_with` gives the loader
//!   `MapLoadOptions::kit` = `KitLoad::Only(kept)`, so the kit's textures
//!   and materials aren't loaded with the map's (only the kept weapons'
//!   models and the impact decals are: the map's game's impacts are
//!   still drawn on it, `MapData::impacts`), and `dress` replaces what is
//!   left (`GameKit::stay`: a
//!   field added to the kit needs no line here, a loader that ignores the
//!   option is still stripped). Its spawn points' teams become the
//!   gameplay's (`SpawnTeams`; unlisted teams: any team), and the
//!   gameplay's dressing is added.
//! - Units: `MapData` is metres for every game (each loader applies its
//!   own game's world unit: Source's 0.0254 m inch, Combat Arms'
//!   `meters_per_unit`), the map's entity space keeps its own scale
//!   (`MapData::entity_scale`, `MapEntities::scale`), and a gameplay
//!   converts its own numbers with its own unit, so nothing is rescaled
//!   when the two differ.
//!
//! - Surfaces: a map of another game keeps its own surface names (its
//!   game's parts that stay, a gun of its game among them, use them), and
//!   the app's `SurfaceTable` for the pair (a data file the app owns, not
//!   either game) gives each its gameplay name (`MapData::surface_names`,
//!   spawned as `SurfaceNames`): by name, else by the Source game material
//!   letter the map's surface has (`sound::MapSurface::game_material`),
//!   else the table's default.
//! - Kept parts: the held and view models of the map's game whose keys
//!   start with one of `MapSources::kept` stay (another game's weapons an
//!   app offers in the gameplay's slots); with `MapSources::players`, also
//!   its players' bodies (after the gameplay's) and its HUD look
//!   (`KitLoad::Players`: an app whose players may be the map's game's,
//!   Lucker Party's `identity = "weapon"`).
//! - Guests: games whose parts dress maps of every other game
//!   (`MapSources::guests`, each a `MapDressing` that appends to the
//!   map): the party's Combat Arms maps get CS:S's player bodies, held
//!   and view models and HUD look from the CS:S install, so a player
//!   holding a CS:S gun there is CS:S's, not the shell's capsule. A guest
//!   is skipped on a map of its own game.
//!
//! The functions are plain `fn`s and the lists `'static` slices, so an
//! app's `MapSources` is a `static` and its `client::AppHooks::load_map`
//! a closure over it. What a loader or dressing needs of the app that
//! asked comes in its options (`MapLoadOptions::context`, the app's
//! `LoadContext`: Combat Arms keeps its install data there, so a load
//! never reaches for a process-wide memo), and what it works out for the
//! map's game goes with the map (`MapData::memos`, `memos::MapMemos`;
//! docs/DESIGN.md, "A game's loader and memos").

use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use bevy::prelude::*;
use serde::Deserialize;

use super::MapData;
use crate::core::Team;

/// What a map load brings in (docs/plans/active/architecture-review.md,
/// F1): everything by default, less for a dedicated server or a test that
/// never draws the map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapLoadOptions {
    /// Source's `mat_hdr_level` (0 LDR; games without HDR data ignore it).
    pub hdr_level: u8,
    /// Decode the textures. Off: each texture is a placeholder of the
    /// right size (`MapTexture::width`, `height`; no texels, no mip
    /// levels), so materials and decals still find them and the map's
    /// logic, collision and sounds are the same; a map loaded so can't be
    /// drawn. A game whose loader doesn't decode textures at load ignores it.
    pub textures: bool,
    /// How much of the map's game's kit (`MapData::kit`) to load with the
    /// map: all of it by default; a map of another game than the
    /// gameplay's loads little or none (`MapSources::load_with`).
    pub kit: KitLoad,
    /// Where the loader reports its stages (`loading::LoadReporter`): the
    /// loading screen of the app that asked reads the same handle.
    pub progress: super::loading::LoadReporter,
    /// The app that asked (its `LoadContext` resource): what its games keep
    /// for their loads. A load with no app (a tool, a test) gets a fresh
    /// one.
    pub context: LoadContext,
}

/// An app's context for its map loads (`MapLoadOptions::context`): one
/// value per type that its games put there or fill on first use
/// (`chimera_core::memo::Memos`). Combat Arms keeps its install data in
/// it (`CaData`), so every load of the app reads the install the app
/// holds, and none of it outlives the app. A resource of each app (the
/// map's plugins insert it); clones share it.
#[derive(Resource, Clone, Default, Debug)]
pub struct LoadContext(pub chimera_core::memo::Memos);

impl std::ops::Deref for LoadContext {
    type Target = chimera_core::memo::Memos;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Two contexts are equal when they are one app's.
impl PartialEq for LoadContext {
    fn eq(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
}

impl Eq for LoadContext {}

impl LoadContext {
    /// The world's context (made when it has none).
    pub fn of(world: &mut World) -> Self {
        world.get_resource_or_init::<Self>().clone()
    }
}

/// How much of a map game's kit (`GameKit`) a loader reads from the
/// install with the map. The kit's textures and materials are read only
/// for what is wanted, so a mashup's map of another game costs its world
/// alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KitLoad {
    /// The whole kit: the map's game is the gameplay's.
    All,
    /// Only the held and view models whose keys start with one of these
    /// prefixes (`MapSources::kept`); everything else of the kit is left
    /// unread. `Only(&[])`: no kit.
    Only(&'static [&'static str]),
    /// `Only`'s models and what a player of the map's game wears and sees:
    /// its players' character bodies and its HUD look (`MapSources::
    /// players`: an app whose players may be that game's).
    Players(&'static [&'static str]),
}

impl KitLoad {
    /// Whether the kit's parts that aren't weapon models (character
    /// bodies, HUD, radio, bot names, round sounds, spray logos, muzzle
    /// flash, shells, navigation) are loaded. A loader reads the impact
    /// decals with any kit (they are the map's impact looks', which stay).
    pub fn parts(&self) -> bool {
        matches!(self, Self::All)
    }

    /// Whether the map's game's pickups are placed (HL2:DM's walk-over
    /// items and wall chargers): only with its own gameplay, the only one
    /// that runs them; on another game's gameplay (the party's) they would
    /// be inert, so they aren't there at all (docs/DESIGN.md, "Gun game
    /// rules").
    pub fn pickups(&self) -> bool {
        matches!(self, Self::All)
    }

    /// Whether the players' character bodies are loaded.
    pub fn bodies(&self) -> bool {
        matches!(self, Self::All | Self::Players(_))
    }

    /// Whether the game's HUD look is loaded.
    pub fn hud(&self) -> bool {
        matches!(self, Self::All | Self::Players(_))
    }

    /// Whether the held or view model `key` is loaded.
    pub fn wants(&self, key: &str) -> bool {
        match self {
            Self::All => true,
            Self::Only(kept) | Self::Players(kept) => kept.iter().any(|k| key.starts_with(k)),
        }
    }

    /// Whether any weapon model is loaded.
    pub fn any_models(&self) -> bool {
        match self {
            Self::All => true,
            Self::Only(kept) | Self::Players(kept) => !kept.is_empty(),
        }
    }
}

impl Default for MapLoadOptions {
    fn default() -> Self {
        Self { hdr_level: 0, textures: true, kit: KitLoad::All, progress: default(), context: default() }
    }
}

impl MapLoadOptions {
    /// Everything, at `mat_hdr_level` `hdr_level`.
    pub fn hdr(hdr_level: u8) -> Self {
        Self { hdr_level, ..Self::default() }
    }

    /// No textures (a dedicated server, a test that never draws).
    pub fn lean() -> Self {
        Self { textures: false, ..Self::default() }
    }

    /// These options for the app whose context is `context`.
    pub fn in_context(self, context: &LoadContext) -> Self {
        Self { context: context.clone(), ..self }
    }
}

/// One game's maps (`game:name`) from its install.
#[derive(Debug)]
pub struct MapLoader {
    /// The game's id, the prefix of its map ids (`cs_source`).
    pub game: &'static str,
    /// Map `name` (without the prefix) with `options` (the app's context
    /// in `options.context`).
    pub load: fn(&str, &MapLoadOptions) -> Result<MapData, String>,
}

/// What one game's gameplay brings to any map: its view models,
/// character bodies and held models, effect looks (added to a map of
/// another game after its own game's kit is replaced, `GameKit::stay`).
#[derive(Debug)]
pub struct MapDressing {
    /// The game's id (`combat_arms`).
    pub game: &'static str,
    /// Add the game's parts to `map` for the load `options` (the app's
    /// context in `options.context`); an error (no install) leaves the
    /// map undressed, with a warning.
    pub dress: fn(&mut MapData, &MapLoadOptions) -> Result<(), String>,
}

/// Spawn teams of another game's maps as the gameplay's teams: map
/// `game`'s team `from` is the gameplay's `to` (CS:S's terrorists, 1, as
/// Combat Arms' Alpha, 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnTeams {
    pub game: &'static str,
    pub from: u8,
    pub to: u8,
}

/// One game's surface names as another's: a data file an app owns for
/// the pair (TOML: `[names]` the map's surface property names, lower-case,
/// to the gameplay's; `[materials]` Source game material letters to the
/// gameplay's, for names not listed; `default`, for the rest).
#[derive(Clone, Copy, Debug)]
pub struct SurfaceTable {
    /// The maps' game (`cs_source`).
    pub map: &'static str,
    /// The gameplay's game (`combat_arms`).
    pub gameplay: &'static str,
    /// The table's text (the app's data file, built in).
    pub text: &'static str,
}

/// A `SurfaceTable`'s rows.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceRows {
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub names: HashMap<String, String>,
    #[serde(default)]
    pub materials: HashMap<char, String>,
}

impl SurfaceTable {
    /// The table's rows (names lower-cased).
    pub fn rows(&self) -> Result<SurfaceRows, String> {
        let mut rows: SurfaceRows =
            toml::from_str(self.text).map_err(|e| format!("{} to {} surfaces: {e}", self.map, self.gameplay))?;
        rows.names = rows.names.into_iter().map(|(k, v)| (k.to_lowercase(), v)).collect();
        rows.materials = rows.materials.into_iter().map(|(k, v)| (k.to_ascii_uppercase(), v)).collect();
        Ok(rows)
    }
}

impl SurfaceRows {
    /// The gameplay's name for the map's surface `name` (lower-case),
    /// whose game material letter is `material` (None: the map doesn't
    /// say).
    pub fn get(&self, name: &str, material: Option<char>) -> Option<&str> {
        self.names
            .get(name)
            .or_else(|| self.materials.get(&material?.to_ascii_uppercase()))
            .or(self.default.as_ref())
            .map(String::as_str)
    }

    /// Every surface name `map` uses (its sound surfaces, its world's and
    /// its models' meshes, its models' and brushes' surface properties)
    /// with its gameplay name.
    pub fn translate(&self, map: &MapData) -> HashMap<String, String> {
        let mut names: BTreeSet<String> = map.sounds.surfaces.keys().cloned().collect();
        let meshes = map.meshes.iter().chain(map.models.iter().flat_map(|m| &m.meshes));
        names.extend(meshes.flat_map(|m| [&m.surface, &m.surface2]).flatten().cloned());
        names.extend(map.models.iter().filter_map(|m| m.surfaceprop.clone()));
        names
            .into_iter()
            .map(|n| n.to_lowercase())
            .filter_map(|n| {
                let material = map.sounds.surfaces.get(&n).map(|s| s.game_material);
                let to = self.get(&n, material)?.to_string();
                Some((n, to))
            })
            .collect()
    }
}

/// The loaded map's surface names as the gameplay's (`MapData::
/// surface_names`): empty when they are the gameplay's own.
#[derive(Resource, Clone, Debug, Default)]
pub struct SurfaceNames(pub Arc<HashMap<String, String>>);

impl SurfaceNames {
    /// The gameplay's name of the map's surface `name` (any case); None:
    /// no translation (the map's own name is the gameplay's, or unknown).
    pub fn get(&self, name: &str) -> Option<&str> {
        if self.0.is_empty() {
            return None;
        }
        self.0.get(&name.to_lowercase()).map(String::as_str)
    }

    /// `name` as the gameplay knows it: its translation, else itself.
    pub fn resolve<'a>(&'a self, name: &'a str) -> &'a str {
        self.get(name).unwrap_or(name)
    }
}

/// An app's maps: the loaders it chose, its gameplay's dressing, the
/// team mapping of other games' spawns, the surface tables and the map
/// game's parts it keeps.
#[derive(Debug)]
pub struct MapSources {
    /// The loaders; a bare map name (no `game:`) is the first's.
    pub loaders: &'static [&'static MapLoader],
    /// The game whose gameplay runs on every map (None: maps load as
    /// their loaders make them).
    pub gameplay: Option<&'static MapDressing>,
    pub teams: &'static [SpawnTeams],
    /// Other games' surface names as the gameplay's, one table per pair.
    pub surfaces: &'static [SurfaceTable],
    /// Key prefixes of the map game's held and view models that stay on
    /// its maps (`cs_source:weapon_`: CS:S's guns an app offers).
    pub kept: &'static [&'static str],
    /// What a player of the map's game wears and sees stays too: its
    /// character bodies (after the gameplay's own) and HUD look
    /// (`KitLoad::Players`; an app whose players may be the map's game's,
    /// `slots::GameIdentity`).
    pub players: bool,
    /// Games whose parts dress every map of another game (the party's
    /// Combat Arms maps get CS:S's players and guns): each `dress` is
    /// called with the map after the gameplay's own dressing, and appends
    /// to the map's textures. Not for a map of the guest's own game.
    pub guests: &'static [&'static MapDressing],
}

impl MapSources {
    /// The loader of `game`, if the app has one.
    pub fn loader(&self, game: &str) -> Option<&'static MapLoader> {
        self.loaders.iter().copied().find(|l| l.game.eq_ignore_ascii_case(game))
    }

    /// Map `id` (`game:name`, or a bare name of the first loader's game)
    /// for this app's gameplay.
    pub fn load(&self, id: &str, hdr_level: u8) -> Result<MapData, String> {
        self.load_with(id, &MapLoadOptions::hdr(hdr_level))
    }

    /// `load` with `options` (a server's: `MapLoadOptions::lean`).
    pub fn load_with(&self, id: &str, options: &MapLoadOptions) -> Result<MapData, String> {
        let (loader, name) = match id.split_once(':') {
            Some((game, name)) => (
                self.loader(game)
                    .ok_or_else(|| format!("{id}: no map loader for `{game}` in this app"))?,
                name,
            ),
            None => (*self.loaders.first().ok_or("this app loads no maps")?, id),
        };
        // A map of another game than the gameplay's is the world only (its
        // kit is replaced by `dress`): the loader reads none of the kit
        // but the kept models.
        let mut options = options.clone();
        if self.gameplay.is_some_and(|g| !g.game.eq_ignore_ascii_case(loader.game)) {
            options.kit = if self.players { KitLoad::Players(self.kept) } else { KitLoad::Only(self.kept) };
        }
        let mut map = (loader.load)(name, &options)?;
        map.impacts.game = Some(loader.game.to_string());
        self.dress(loader.game, &mut map, &options);
        Ok(map)
    }

    /// `map` (of `game`) made ready for the gameplay: unchanged when the
    /// gameplay is the map's own game (or there is none); else its game's
    /// parts taken off, its spawn teams mapped (`teams`) and the
    /// gameplay's parts added (for the load `options`).
    pub fn dress(&self, game: &str, map: &mut MapData, options: &MapLoadOptions) {
        if let Some(gameplay) = self.gameplay.filter(|g| !g.game.eq_ignore_ascii_case(game)) {
            self.dress_with(gameplay, game, map, options);
        }
        for guest in self.guests.iter().filter(|g| !g.game.eq_ignore_ascii_case(game)) {
            if let Err(e) = (guest.dress)(map, options) {
                map.warnings.push(format!("{}'s parts: {e}", guest.game));
            }
        }
    }

    /// `dress` with the gameplay's own parts.
    fn dress_with(&self, gameplay: &MapDressing, game: &str, map: &mut MapData, options: &MapLoadOptions) {
        map.kit = std::mem::take(&mut map.kit).stay(self.kept, self.players);
        // The map game's bodies go after the gameplay's (whose first is
        // the one everything without an identity is measured against).
        let kept_bodies = std::mem::take(&mut map.kit.characters);
        if let Some(table) = self
            .surfaces
            .iter()
            .find(|t| t.map.eq_ignore_ascii_case(game) && t.gameplay.eq_ignore_ascii_case(gameplay.game))
        {
            match table.rows() {
                Ok(rows) => map.surface_names = rows.translate(map),
                Err(e) => map.warnings.push(e),
            }
        }
        for (_, team) in &mut map.spawns {
            *team = team.and_then(|t| {
                self.teams
                    .iter()
                    .find(|s| s.game.eq_ignore_ascii_case(game) && s.from == t.0)
                    .map(|s| Team(s.to))
            });
        }
        if let Err(e) = (gameplay.dress)(map, options) {
            map.warnings.push(format!("{}'s parts: {e}", gameplay.game));
        }
        map.kit.characters.extend(kept_bodies);
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::default;

    use super::*;

    fn a_map(name: &str, _: &MapLoadOptions) -> Result<MapData, String> {
        let mut sounds = crate::map::sound::MapSounds::default();
        for (name, letter) in [("sand", 'N'), ("rock", 'C'), ("weird", '?')] {
            let surface = crate::map::sound::MapSurface {
                game_material: letter,
                ..default()
            };
            sounds.surfaces.insert(name.into(), surface);
        }
        let mut map = MapData {
            name: format!("a:{name}"),
            spawns: vec![(Vec3::ZERO, Some(Team(1))), (Vec3::X, Some(Team(2))), (Vec3::Y, Some(Team(7)))],
            sounds: Arc::new(sounds),
            ..default()
        };
        map.meshes.push(crate::map::MapMesh {
            surface: Some("wood_crate".into()),
            ..default()
        });
        for key in ["a:gun", "a:knife"] {
            map.kit.held.push(crate::map::MapHeldModel {
                key: key.into(),
                model: default(),
                bone: String::new(),
                muzzle: None,
                attachments: Vec::new(),
                merged: Vec::new(),
                own: None,
            });
        }
        map.kit.nav = Some(Default::default());
        Ok(map)
    }

    fn b_map(name: &str, _: &MapLoadOptions) -> Result<MapData, String> {
        let mut map = MapData {
            name: format!("b:{name}"),
            ..default()
        };
        map.kit.nav = Some(Default::default());
        Ok(map)
    }

    fn b_parts(map: &mut MapData, _: &MapLoadOptions) -> Result<(), String> {
        map.warnings.push("b's parts".into());
        Ok(())
    }

    static A: MapLoader = MapLoader { game: "a", load: a_map };
    static B: MapLoader = MapLoader { game: "b", load: b_map };
    static B_PARTS: MapDressing = MapDressing { game: "b", dress: b_parts };
    static SOURCES: MapSources = MapSources {
        loaders: &[&A, &B],
        gameplay: Some(&B_PARTS),
        teams: &[SpawnTeams { game: "a", from: 1, to: 2 }, SpawnTeams { game: "a", from: 2, to: 1 }],
        surfaces: &[SurfaceTable {
            map: "a",
            gameplay: "b",
            text: "default = \"Stone\"\n[names]\nWood_Crate = \"Wood\"\n[materials]\nN = \"Sand\"\n",
        }],
        kept: &["a:gun"],
        players: false,
        guests: &[],
    };

    #[test]
    fn another_games_map_is_dressed_for_the_gameplay() {
        let map = SOURCES.load("a:x", 0).unwrap();
        assert_eq!(map.name, "a:x");
        assert!(map.kit.nav.is_none(), "a's navigation taken off");
        assert_eq!(map.warnings, ["b's parts"]);
        let teams: Vec<_> = map.spawns.iter().map(|(_, t)| t.map(|t| t.0)).collect();
        assert_eq!(teams, [Some(2), Some(1), None], "mapped; unlisted teams spawn anyone");
        let kept: Vec<&str> = map.kit.held.iter().map(|h| h.key.as_str()).collect();
        assert_eq!(kept, ["a:gun"], "the kept prefix stays");
    }

    #[test]
    fn a_dressed_map_keeps_nothing_of_its_games_kit_but_the_kept_and_their_looks() {
        let mut map = a_map("x", &MapLoadOptions::default()).unwrap();
        map.kit.round_sounds.draw = Some("a:draw".into());
        map.kit.muzzle_flash = Some(default());
        map.kit.decals.groups.insert("impact.concrete".into(), vec![(1.0, 0)]);
        map.source_fx_mut().detail_props = None;
        SOURCES.dress("a", &mut map, &MapLoadOptions::default());
        let kit = &map.kit;
        assert!(kit.round_sounds.draw.is_none());
        assert!(kit.muzzle_flash.is_some(), "its muzzle flash stays with its kept guns");
        assert_eq!(kit.held.len(), 1, "the kept prefix stays");
        assert!(kit.decals.groups.contains_key("impact.concrete"), "its impact decals stay, with its impact looks");
        assert!(!map.extras.is_empty(), "the map's own extensions are not the kit");
    }

    #[test]
    fn options_reach_the_loader() {
        fn lean_only(name: &str, o: &MapLoadOptions) -> Result<MapData, String> {
            assert!(!o.textures && o.hdr_level == 0, "{o:?}");
            Ok(MapData { name: name.into(), ..default() })
        }
        static L: MapLoader = MapLoader { game: "l", load: lean_only };
        static S: MapSources =
            MapSources { loaders: &[&L], gameplay: None, teams: &[], surfaces: &[], kept: &[], players: false, guests: &[] };
        S.load_with("l:x", &MapLoadOptions::lean()).unwrap();
        assert!(MapLoadOptions::default().textures, "everything by default");
    }

    #[test]
    fn only_another_games_map_leaves_its_kit_unloaded() {
        fn kit_seen(name: &str, o: &MapLoadOptions) -> Result<MapData, String> {
            // The loader reports the kit it was asked for in the name.
            Ok(MapData { name: format!("{name}/{:?}", o.kit), ..default() })
        }
        static A2: MapLoader = MapLoader { game: "a", load: kit_seen };
        static B2: MapLoader = MapLoader { game: "b", load: kit_seen };
        static S2: MapSources = MapSources {
            loaders: &[&A2, &B2],
            gameplay: Some(&B_PARTS),
            teams: &[],
            surfaces: &[],
            kept: &["a:gun"],
            players: false,
            guests: &[],
        };
        assert_eq!(S2.load("a:x", 0).unwrap().name, "x/Only([\"a:gun\"])", "the world and the kept guns");
        assert_eq!(S2.load("b:x", 0).unwrap().name, "x/All", "the gameplay's own game: its whole kit");
        let kit = KitLoad::Only(&["a:gun"]);
        assert!(!kit.parts() && kit.any_models());
        assert!(kit.wants("a:gun_ak") && !kit.wants("a:knife"));
        assert!(!KitLoad::Only(&[]).any_models() && KitLoad::All.wants("any") && KitLoad::All.parts());
    }

    fn body(game: &'static str, name: Option<&str>) -> crate::map::MapCharacterModel {
        crate::map::MapCharacterModel {
            game: Some(game),
            team: None,
            name: name.map(str::to_string),
            class: None,
            model: default(),
            hitboxes: Vec::new(),
            bones: Vec::new(),
            root: Transform::IDENTITY,
            animations: None,
            boxes: Vec::new(),
            ragdoll: None,
        }
    }

    /// With `players`, the map game's player bodies and HUD look stay
    /// (asked of the loader as `KitLoad::Players`), after the gameplay's
    /// own bodies; its other parts still go.
    #[test]
    fn a_map_for_players_of_its_game_keeps_their_bodies_and_hud() {
        fn with_players(name: &str, o: &MapLoadOptions) -> Result<MapData, String> {
            assert!(matches!(o.kit, KitLoad::Players(_)) && o.kit.bodies() && o.kit.hud() && !o.kit.parts());
            let mut map = a_map(name, o)?;
            map.kit.characters.push(body("a", None));
            map.kit.characters.push(body("a", Some("a_hostage")));
            map.kit.hud = Some(default());
            Ok(map)
        }
        fn b_bodies(map: &mut MapData, _: &MapLoadOptions) -> Result<(), String> {
            map.kit.characters.push(body("b", Some("b_soldier")));
            Ok(())
        }
        static A3: MapLoader = MapLoader { game: "a", load: with_players };
        static B_BODIES: MapDressing = MapDressing { game: "b", dress: b_bodies };
        static S3: MapSources = MapSources {
            loaders: &[&A3, &B],
            gameplay: Some(&B_BODIES),
            teams: &[],
            surfaces: &[],
            kept: &["a:gun"],
            players: true,
            guests: &[],
        };
        let map = S3.load("a:x", 0).unwrap();
        let bodies: Vec<_> = map.kit.characters.iter().map(|c| (c.game, c.name.as_deref())).collect();
        assert_eq!(bodies, [(Some("b"), Some("b_soldier")), (Some("a"), None)], "the gameplay's first; no hostage");
        assert!(map.kit.hud.is_some(), "the HUD look stays");
        assert!(map.kit.nav.is_none(), "the rest goes");
        assert_eq!(map.kit.held.len(), 1);
    }

    /// A guest dresses the maps of the other games only, after the
    /// gameplay's own dressing, and a failing one is a warning.
    #[test]
    fn a_guest_dresses_the_maps_of_other_games() {
        fn a_guest(map: &mut MapData, _: &MapLoadOptions) -> Result<(), String> {
            map.kit.characters.push(body("a", Some("a_guest")));
            Ok(())
        }
        fn a_broken_guest(_: &mut MapData, _: &MapLoadOptions) -> Result<(), String> {
            Err("no install".into())
        }
        static A_GUEST: MapDressing = MapDressing { game: "a", dress: a_guest };
        static C_GUEST: MapDressing = MapDressing { game: "c", dress: a_broken_guest };
        static S4: MapSources = MapSources {
            loaders: &[&A, &B],
            gameplay: Some(&B_PARTS),
            teams: &[],
            surfaces: &[],
            kept: &[],
            players: false,
            guests: &[&A_GUEST, &C_GUEST],
        };
        let other = S4.load("b:y", 0).unwrap();
        assert!(other.kit.characters.iter().any(|c| c.name.as_deref() == Some("a_guest")), "a's guest on b's map");
        assert!(other.warnings.iter().any(|w| w.contains("c's parts: no install")), "{:?}", other.warnings);
        let own = S4.load("a:x", 0).unwrap();
        assert!(own.kit.characters.iter().all(|c| c.name.as_deref() != Some("a_guest")), "not on its own game's map");
    }

    /// The app's context reaches its loader and the gameplay's dressing:
    /// two apps' loads each see their own.
    #[test]
    fn the_apps_context_reaches_the_loader_and_the_dressing() {
        #[derive(Debug)]
        struct Who(u32);
        fn loader(name: &str, o: &MapLoadOptions) -> Result<MapData, String> {
            let app = o.context.get::<Who>().map_or(0, |a| a.0);
            Ok(MapData { name: format!("{name}@{app}"), ..default() })
        }
        fn dressing(map: &mut MapData, o: &MapLoadOptions) -> Result<(), String> {
            map.warnings.push(format!("dressed for {}", o.context.get::<Who>().map_or(0, |a| a.0)));
            Ok(())
        }
        static L: MapLoader = MapLoader { game: "a", load: loader };
        static D: MapDressing = MapDressing { game: "b", dress: dressing };
        static S: MapSources =
            MapSources { loaders: &[&L], gameplay: Some(&D), teams: &[], surfaces: &[], kept: &[], players: false, guests: &[] };
        let (one, two) = (LoadContext::default(), LoadContext::default());
        one.insert(Who(1));
        two.insert(Who(2));
        assert_ne!(one, two);
        let a = S.load_with("a:x", &MapLoadOptions::lean().in_context(&one)).unwrap();
        let b = S.load_with("a:x", &MapLoadOptions::lean().in_context(&two)).unwrap();
        assert_eq!((a.name.as_str(), a.warnings[0].as_str()), ("x@1", "dressed for 1"));
        assert_eq!((b.name.as_str(), b.warnings[0].as_str()), ("x@2", "dressed for 2"));
        assert_eq!(S.load("a:x", 0).unwrap().name, "x@0", "no app: a fresh context");
    }

    #[test]
    fn surfaces_get_the_gameplays_names() {
        let map = SOURCES.load("a:x", 0).unwrap();
        let names = SurfaceNames(Arc::new(map.surface_names.clone()));
        assert_eq!(names.get("WOOD_CRATE"), Some("Wood"), "by name");
        assert_eq!(names.get("sand"), Some("Sand"), "by game material letter");
        assert_eq!(names.get("weird"), Some("Stone"), "else the default");
        assert_eq!(names.resolve("unseen"), "unseen");
        let own = SOURCES.load("b:y", 0).unwrap();
        assert!(own.surface_names.is_empty(), "the gameplay's own map keeps its names");
    }

    #[test]
    fn the_gameplays_own_map_loads_as_its_loader_makes_it() {
        let map = SOURCES.load("b:y", 0).unwrap();
        assert!(map.kit.nav.is_some() && map.warnings.is_empty());
        assert_eq!(SOURCES.load("x", 0).unwrap().name, "a:x", "a bare name is the first loader's");
        assert!(SOURCES.load("c:z", 0).unwrap_err().contains("no map loader for `c`"));
    }
}
