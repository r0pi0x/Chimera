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
//!   models with what they draw, their muzzle flash and spent shells, and
//!   the impact decals are: the map's game's impacts are
//!   still drawn on it, `MapData::impacts`), and `dress` replaces what is
//!   left (`GameKit::stay`: a
//!   field added to the kit needs no line here, a loader that ignores the
//!   option is still stripped). Its spawn points' teams become the
//!   gameplay's through the neutral sides (each game's `MapWords::teams`;
//!   a team either game doesn't name: any team), and the gameplay's
//!   dressing is added.
//! - Units: `MapData` is metres for every game (each loader applies its
//!   own game's world unit: Source's 0.0254 m inch, Combat Arms'
//!   `meters_per_unit`), the map's entity space keeps its own scale
//!   (`MapData::entity_scale`, `MapEntities::scale`), and a gameplay
//!   converts its own numbers with its own unit, so nothing is rescaled
//!   when the two differ.
//!
//! - Surfaces: a map of another game keeps its own surface names (its
//!   game's parts that stay, a gun of its game among them, use them), and
//!   each gets its gameplay name (`MapData::surface_names`, spawned as
//!   `SurfaceNames`) through the neutral surface kinds: the map's game
//!   says which kind each of its names is (`SurfaceWords::names`; a name
//!   it doesn't list is a kind's own name), the gameplay's game what it
//!   calls each kind (`SurfaceWords::kinds`); a name whose kind the
//!   gameplay doesn't know goes by the Source game material letter the
//!   map's surface has (`sound::MapSurface::game_material`, the map's
//!   game's `materials`), else the gameplay's `default`. Each game's words
//!   are its own (`MapLoader::words`): a new game maps into the kinds
//!   once, not into each other game's names.
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
//!   holding a CS:S gun there is CS:S's, not the stand-in body. A guest
//!   is skipped on a map of its own game.
//!
//! - Side by side: the gameplay's and the guests' dressings first run at
//!   once, each on a map of its own (`dress_ahead`), so what they keep for
//!   the session (`kept`) converts in parallel, then in order on the map
//!   (copies of what was kept: the same map as dressing in turn). Each
//!   game's view models are handed over as it is done
//!   (`MapLoadOptions::early`, `EarlyViewModels`): the client's code-built
//!   map kit draws the gun in hand from them before the whole kit is in.
//!
//! The functions are plain `fn`s and the lists `'static` slices, so an
//! app's `MapSources` is a `static` and its `client::AppHooks::load_map`
//! a closure over it. What a loader or dressing needs of the app that
//! asked comes in its options (`MapLoadOptions::context`, the app's
//! `LoadContext`: Combat Arms keeps its install data there, so a load
//! never reaches for a process-wide memo), and what it works out for the
//! map's game goes with the map (`MapData::memos`, `memos::MapMemos`;
//! docs/DESIGN.md, "A game's loader and memos").
//!
//! Known shortcuts (docs/tech-debt.md): `dress_ahead` dresses each game
//! twice, the first on a map of its own that is thrown away (a kept
//! dressing's copy made once more; a dressing that keeps nothing, Halo's,
//! does its work twice), and the guests' conversions running at once raise
//! the load's peak memory. Why it's OK for now: a kept copy is tens of
//! milliseconds and Halo's dressing under a second, on threads of their
//! own beside the conversions; the gun in hand no longer waits for the
//! sum of every game's conversion (docs/performance.md, "The gun in hand
//! ahead of the sandbox's kit").

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
    /// None: the level the map's game starts at (`SourceGame::
    /// hdr_default_level`), what a player who never set `mat_hdr_level`
    /// gets; the client asks that way, everything else with a level.
    pub hdr_level: Option<u8>,
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
    /// The games whose parts the match on this map can deal (its ladder's
    /// guns' games and its definition's kit and slots: `chimera_net`'s
    /// `minigame_net::dealt_games`); a guest of another game isn't
    /// dressed (`MapSources::dress`). None: any game (no match known, a
    /// sandbox, a client).
    pub dealt: Option<Arc<BTreeSet<String>>>,
    /// Where each dressing's view models go as soon as that dressing is
    /// done, before the whole map is (`EarlyViewModels`; the client's
    /// code-built map kit: the gun in hand drawn while the other games'
    /// parts still convert). None: nobody takes them.
    pub early: Option<EarlyViewModels>,
}

/// The view models of each dressing of a load, handed over as each
/// dressing is done (`MapSources::dress` dresses the guests at once, each
/// on its own map first): a game's view models with the textures they
/// name (their own list, from 0, as `view_model::add_view_model` takes
/// them). A client whose map is drawn before its kit (the code-built
/// maps', `load_code_map`) adds them, so a gun given meanwhile shows when
/// its own game's parts are in, not when every game's are.
#[derive(Clone, Debug, Default)]
pub struct EarlyViewModels(Arc<std::sync::Mutex<Vec<EarlyGame>>>);

/// One game's early view models (`EarlyViewModels`).
#[derive(Clone, Debug)]
pub struct EarlyGame {
    pub game: &'static str,
    pub view_models: Vec<super::MapViewModel>,
    /// The textures they name.
    pub textures: Arc<Vec<super::MapTexture>>,
    /// The time from the load's dressing's start to this game's parts.
    pub after: std::time::Duration,
}

impl PartialEq for EarlyViewModels {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for EarlyViewModels {}

impl EarlyViewModels {
    /// Hand over `map`'s view models (a dressing's map: its textures its
    /// own), each with the textures it names, once per game.
    pub fn publish(&self, game: &'static str, map: &MapData, after: std::time::Duration) {
        use super::texture_refs::{Ref, TextureRefs};
        if map.kit.view_models.is_empty() {
            return;
        }
        let mut list = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if list.iter().any(|g| g.game == game) {
            return;
        }
        // Only the textures they name, renumbered from 0.
        let mut renumber: HashMap<usize, usize> = HashMap::new();
        let mut textures = Vec::new();
        let mut view_models = map.kit.view_models.clone();
        for v in &mut view_models {
            v.texture_refs(&mut |r, i| {
                if r != Ref::Texture {
                    return;
                }
                let Some(t) = map.textures.get(*i) else { return };
                *i = *renumber.entry(*i).or_insert_with(|| {
                    textures.push(t.clone());
                    textures.len() - 1
                });
            });
        }
        list.push(EarlyGame { game, view_models, textures: Arc::new(textures), after });
    }

    /// What was handed over since the last `take`.
    pub fn take(&self) -> Vec<EarlyGame> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
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
    /// bodies, HUD, radio, bot names, round sounds, spray logos,
    /// navigation) are loaded. A loader reads the impact decals with any
    /// kit (they are the map's impact looks', which stay), and the muzzle
    /// flash and spent shells with any weapon model (`any_models`: what
    /// its guns draw).
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

    /// Whether any weapon model is loaded (and with them what the guns
    /// draw: muzzle flashes, spent shells).
    pub fn any_models(&self) -> bool {
        match self {
            Self::All => true,
            Self::Only(kept) | Self::Players(kept) => !kept.is_empty(),
        }
    }
}

impl Default for MapLoadOptions {
    fn default() -> Self {
        Self { hdr_level: Some(0), textures: true, kit: KitLoad::All, progress: default(), context: default(), dealt: None, early: None }
    }
}

impl MapLoadOptions {
    /// Everything, at `mat_hdr_level` `hdr_level`.
    pub fn hdr(hdr_level: u8) -> Self {
        Self { hdr_level: Some(hdr_level), ..Self::default() }
    }

    /// The level these load at for a game whose own default is
    /// `game_default`: the one asked for, else the game's.
    pub fn hdr_level_or(&self, game_default: u8) -> u8 {
        self.hdr_level.unwrap_or(game_default)
    }

    /// No textures (a dedicated server, a test that never draws).
    pub fn lean() -> Self {
        Self { textures: false, ..Self::default() }
    }

    /// These options for the app whose context is `context`.
    pub fn in_context(self, context: &LoadContext) -> Self {
        Self { context: context.clone(), ..self }
    }

    /// Whether the match can deal `game`'s parts (`dealt`; any without).
    pub fn deals(&self, game: &str) -> bool {
        self.dealt.as_ref().is_none_or(|d| d.iter().any(|g| g.eq_ignore_ascii_case(game)))
    }
}

/// The games the loaded map's guests were dressed for (its load's
/// `MapLoadOptions::dealt`; None: any): a match that deals another game
/// loads the map again (`chimera_net`'s `minigame_net`).
#[derive(Resource, Clone, Debug, Default)]
pub struct MapDealt(pub Option<Arc<BTreeSet<String>>>);

/// One game's maps (`game:name`) from its install.
#[derive(Debug)]
pub struct MapLoader {
    /// The game's id, the prefix of its map ids (`cs_source`).
    pub game: &'static str,
    /// Map `name` (without the prefix) with `options` (the app's context
    /// in `options.context`).
    pub load: fn(&str, &MapLoadOptions) -> Result<MapData, String>,
    /// Its words for what maps of different games share (its spawn teams
    /// as the neutral sides, its surfaces as the neutral kinds), both as a
    /// map's game and as the gameplay's (None: it has none: its maps'
    /// teams spawn anyone under another game's gameplay, their surfaces
    /// keep their names).
    pub words: Option<&'static MapWords>,
}

/// The kit of a map built in code (`code_maps::CODE_MAPS`: the greybox, the
/// range), as a map of game `chimera`: `chimera:range` loads an empty
/// map (no world, spawns or sun: the code-built map is spawned beside it,
/// `greybox::respawn`; its entity space is metres, `entity_scale` 1) that `MapSources::dress` dresses like any other
/// game's map, so the party's bodies, held and view models and HUD looks
/// are on the code-built maps too. An app whose `MapSources` lists it gets
/// that (the client's `load_code_map`, the dedicated server's
/// `swap_map` and start load `chimera:<id>` after spawning the code map);
/// without it a code-built map has no kit.
pub static CODE_MAP_KIT: MapLoader = MapLoader {
    game: CODE_MAP_GAME,
    load: code_map_kit,
    words: None,
};

/// The game id of the code-built maps' kit (`CODE_MAP_KIT`).
pub const CODE_MAP_GAME: &str = "chimera";

fn code_map_kit(name: &str, _: &MapLoadOptions) -> Result<MapData, String> {
    let id = Some(name)
        .filter(|n| !n.is_empty())
        .and_then(super::code_maps::code_map)
        .ok_or_else(|| format!("{CODE_MAP_GAME}:{name}: no map built in code by that name"))?;
    Ok(MapData {
        name: format!("{CODE_MAP_GAME}:{id}"),
        // Code-built maps are in metres: entities a dressing adds to the
        // kit are in that space (a scale of 0 made every conversion into
        // entity space divide by zero: `entities::engine_to_entity`).
        entity_scale: 1.0,
        ..Default::default()
    })
}

/// Whether `map` is a code-built map's kit (`CODE_MAP_KIT`'s): no world of
/// its own, so no sun either.
pub fn is_code_map_kit(map: &MapData) -> bool {
    map.name
        .strip_prefix("chimera:")
        .is_some_and(|n| !n.is_empty() && super::code_maps::code_map(n).is_some())
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

/// The neutral sides a game's spawn teams are (`MapWords::teams`): the
/// first and the second of two teams (CS:S's terrorists and counter-
/// terrorists, TF2's RED and BLU, Halo's red and blue, Combat Arms' Alpha
/// and Bravo).
pub const SIDE_FIRST: u8 = 1;
pub const SIDE_SECOND: u8 = 2;

/// One game's words for what maps of different games share, both ways:
/// as a map's game (its words to the neutral ones) and as the gameplay's
/// (the neutral ones to its own). A composer puts game A's map under game
/// B's gameplay through A's and B's words: no table per pair of games.
#[derive(Clone, Copy, Debug)]
pub struct MapWords {
    /// Its spawn teams and the neutral side each is (`SIDE_FIRST`,
    /// `SIDE_SECOND`): `(own team, side)`.
    pub teams: &'static [(u8, u8)],
    /// Its surfaces and the neutral kinds (TOML, `SurfaceWords`; read from
    /// the checkout in a dev build, built in otherwise). None: it says
    /// nothing of surfaces (its maps' keep their names).
    pub surfaces: Option<chimera_core::game_data::GameData>,
}

impl MapWords {
    /// Team `own` of this game's maps as `gameplay`'s team (None: either
    /// game doesn't name the side).
    pub fn team_as(&self, gameplay: &MapWords, own: u8) -> Option<u8> {
        let side = self.teams.iter().find(|(t, _)| *t == own)?.1;
        gameplay.teams.iter().find(|(_, s)| *s == side).map(|(t, _)| *t)
    }
}

/// A game's surfaces in the neutral kinds (`MapWords::surfaces`, TOML).
/// The kinds are Source's surface property names (concrete, metal_box,
/// wood_crate, gravel, ladder, flesh, ...: the finest set our games
/// speak; `impact::SurfaceClass` is their coarse grouping). As a map's
/// game: `[names]` its own surface names to kinds (a name not listed is
/// its own kind), `[materials]` Source game material letters to kinds,
/// for names whose kind the gameplay doesn't know. As the gameplay:
/// `[kinds]` kinds to its own surface names, `default` for the rest.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceWords {
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub names: HashMap<String, String>,
    #[serde(default)]
    pub materials: HashMap<char, String>,
    #[serde(default)]
    pub kinds: HashMap<String, String>,
}

impl SurfaceWords {
    /// `game`'s words read from `text` (names and kinds lower-cased,
    /// letters upper-cased).
    pub fn read(game: &str, text: &str) -> Result<SurfaceWords, String> {
        let mut w: SurfaceWords = toml::from_str(text).map_err(|e| format!("{game}'s surface words: {e}"))?;
        w.names = w.names.into_iter().map(|(k, v)| (k.to_lowercase(), v.to_lowercase())).collect();
        w.materials = w.materials.into_iter().map(|(k, v)| (k.to_ascii_uppercase(), v.to_lowercase())).collect();
        w.kinds = w.kinds.into_iter().map(|(k, v)| (k.to_lowercase(), v)).collect();
        Ok(w)
    }
}

/// A map's game's surface words and its gameplay's, together: the
/// gameplay's name for each of the map's.
#[derive(Clone, Debug, Default)]
pub struct SurfaceRows {
    pub map: SurfaceWords,
    pub gameplay: SurfaceWords,
}

impl SurfaceRows {
    /// The gameplay's name for the map's surface `name` (lower-case),
    /// whose game material letter is `material` (None: the map doesn't
    /// say): by its kind, else by its letter's kind, else the gameplay's
    /// default.
    pub fn get(&self, name: &str, material: Option<char>) -> Option<&str> {
        let kind = self.map.names.get(name).map_or(name, String::as_str);
        let by_letter = || self.map.materials.get(&material?.to_ascii_uppercase()).and_then(|k| self.gameplay.kinds.get(k));
        self.gameplay.kinds.get(kind).or_else(by_letter).or(self.gameplay.default.as_ref()).map(String::as_str)
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

/// An app's maps: the loaders it chose (each with its game's words,
/// `MapLoader::words`), its gameplay's dressing and the map game's parts
/// it keeps.
#[derive(Debug)]
pub struct MapSources {
    /// The loaders; a bare map name (no `game:`) is the first's.
    pub loaders: &'static [&'static MapLoader],
    /// The game whose gameplay runs on every map (None: maps load as
    /// their loaders make them).
    pub gameplay: Option<&'static MapDressing>,
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

/// The game's loader's part of a map's load (`MapSources::load_with`); the
/// dressings report the rest (their parts are often the slower: every
/// guest game's players and guns, the first time in a session).
pub const LOADER_SHARE: f32 = 0.6;

impl MapSources {
    /// The words of `game` (its loader's), if the app has its loader.
    pub fn words(&self, game: &str) -> Option<&'static MapWords> {
        self.loader(game)?.words
    }

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
        // The load's progress in parts (`LoadReporter::within`): the
        // game's loader, then the dressings, each its own span.
        let whole = options.progress.clone();
        options.progress = whole.within(0.0, LOADER_SHARE);
        let mut map = (loader.load)(name, &options)?;
        options.progress = whole.within(LOADER_SHARE, 1.0);
        map.impacts.game = Some(loader.game.to_string());
        self.dress(loader.game, &mut map, &options);
        options.progress.report(1.0, "Adding every game's players and guns");
        Ok(map)
    }

    /// `map` (of `game`) made ready for the gameplay: unchanged when the
    /// gameplay is the map's own game (or there is none); else its game's
    /// parts taken off, its spawn teams mapped (`teams`) and the
    /// gameplay's parts added (for the load `options`).
    /// Each dressing reports its share of `options.progress` as it is done
    /// (the gameplay's, then each guest's, the same share each).
    pub fn dress(&self, game: &str, map: &mut MapData, options: &MapLoadOptions) {
        let gameplay = self.gameplay.filter(|g| !g.game.eq_ignore_ascii_case(game));
        // Only the guests the match can deal (`MapLoadOptions::dealt`).
        let guests: Vec<&MapDressing> =
            self.guests.iter().copied().filter(|g| !g.game.eq_ignore_ascii_case(game) && options.deals(g.game)).collect();
        let parts = (gameplay.is_some() as usize + guests.len()).max(1) as f32;
        let all: Vec<&MapDressing> = gameplay.into_iter().chain(guests.iter().copied()).collect();
        dress_ahead(&all, map, options);
        let mut done = 0.0;
        let step = |done: &mut f32| {
            *done += 1.0;
            options.progress.report(*done / parts, "Adding every game's players and guns");
        };
        if let Some(gameplay) = gameplay {
            let began = std::time::Instant::now();
            self.dress_with(gameplay, game, map, options);
            dressed(gameplay.game, &map.name, began);
            step(&mut done);
        }
        for guest in guests {
            let began = std::time::Instant::now();
            if let Err(e) = (guest.dress)(map, options) {
                map.warnings.push(format!("{}'s parts: {e}", guest.game));
            }
            dressed(guest.game, &map.name, began);
            step(&mut done);
        }
    }

    /// `dress` with the gameplay's own parts.
    fn dress_with(&self, gameplay: &MapDressing, game: &str, map: &mut MapData, options: &MapLoadOptions) {
        map.kit = std::mem::take(&mut map.kit).stay(self.kept, self.players);
        // The map game's bodies go after the gameplay's (whose first is
        // the one everything without an identity is measured against).
        let kept_bodies = std::mem::take(&mut map.kit.characters);
        let (words, theirs) = (self.words(game), self.words(gameplay.game));
        if let (Some(words), Some(theirs)) = (words, theirs) {
            match (words.surfaces, theirs.surfaces) {
                (Some(own), Some(other)) => match (SurfaceWords::read(game, own.text()), SurfaceWords::read(gameplay.game, other.text())) {
                    (Ok(map_side), Ok(gameplay_side)) => {
                        map.surface_names = SurfaceRows { map: map_side, gameplay: gameplay_side }.translate(map)
                    }
                    (Err(e), _) | (_, Err(e)) => map.warnings.push(e),
                },
                _ => {}
            }
        }
        for (_, team) in &mut map.spawns {
            *team = team.and_then(|t| Some(Team(words?.team_as(theirs?, t.0)?)));
        }
        if let Err(e) = (gameplay.dress)(map, options) {
            map.warnings.push(format!("{}'s parts: {e}", gameplay.game));
        }
        map.kit.characters.extend(kept_bodies);
    }
}

/// Each of `dressings` dressed at once on a map of its own (empty, as
/// `map` is named and scaled), on threads of its own: what the dressings
/// keep for the session (`kept`: the Source games', Combat Arms') is
/// converted side by side, not one after another (a cold sandbox kit was
/// the sum of five games' conversions, 33 s on a tester's PC, and the gun
/// in hand waited for all of them), and each game's view models are handed
/// over as it is done (`MapLoadOptions::early`). The dressing proper then
/// runs in order on `map` (copies of what was kept, so `map` is the same
/// as before). A dressing that keeps nothing (Halo's donors are its own)
/// runs twice; its second run reads what its first left in the app's
/// context. One dressing alone: nothing ahead.
fn dress_ahead(dressings: &[&MapDressing], map: &MapData, options: &MapLoadOptions) {
    if dressings.len() < 2 {
        return;
    }
    let began = std::time::Instant::now();
    let parts = dressings.len() as f32;
    let done = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for d in dressings {
            let done = &done;
            s.spawn(move || {
                let mut own = MapData { name: map.name.clone(), entity_scale: map.entity_scale, ..Default::default() };
                let started = std::time::Instant::now();
                if (d.dress)(&mut own, options).is_ok() {
                    if let Some(early) = &options.early {
                        early.publish(d.game, &own, began.elapsed());
                    }
                }
                dressed(d.game, &map.name, started);
                // The progress as each is done (the dressing proper after
                // takes little more).
                let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                options.progress.report(n as f32 / parts, "Adding every game's players and guns");
            });
        }
    });
}

/// A dressing's time in the log (debug; info when it took a while).
fn dressed(game: &str, map: &str, began: std::time::Instant) {
    let ms = began.elapsed().as_secs_f64() * 1000.0;
    if ms >= 100.0 {
        bevy::log::info!("map dressing: {game}'s parts on {map} in {ms:.0} ms");
    } else {
        bevy::log::debug!("map dressing: {game}'s parts on {map} in {ms:.0} ms");
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::default;

    use super::*;

    /// Every code-built map's kit has a real entity scale (metres): with
    /// 0, a position converted into its entity space was not a number
    /// (the sandbox's crash on the range).
    #[test]
    fn the_code_built_maps_kit_has_a_positive_entity_scale() {
        for id in crate::map::code_maps::CODE_MAPS {
            let kit = (CODE_MAP_KIT.load)(id, &MapLoadOptions::default()).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(is_code_map_kit(&kit), "{}", kit.name);
            assert_eq!(kit.entity_scale, 1.0, "{id}: metres");
            let p = crate::map::entities::engine_to_entity(bevy::math::Vec3::new(1.0, 2.0, 3.0), kit.entity_scale);
            assert!(p.is_finite(), "{id}: {p}");
        }
        assert!((CODE_MAP_KIT.load)("no_such_map", &MapLoadOptions::default()).is_err());
    }

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
                nose: None,
                turned: Vec::new(),
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

    // a's teams are b's the other way round; a's surfaces go to b's by
    // their kinds, by a's material letters, else b's default.
    static A_WORDS: MapWords = MapWords {
        teams: &[(1, SIDE_FIRST), (2, SIDE_SECOND)],
        surfaces: Some(chimera_core::game_data::GameData::from_text("[materials]\nN = \"beach\"\n")),
    };
    static B_WORDS: MapWords = MapWords {
        teams: &[(2, SIDE_FIRST), (1, SIDE_SECOND)],
        surfaces: Some(chimera_core::game_data::GameData::from_text(
            "default = \"Stone\"\n[kinds]\nWood_Crate = \"Wood\"\nbeach = \"Sand\"\n",
        )),
    };
    static A: MapLoader = MapLoader { game: "a", load: a_map, words: Some(&A_WORDS) };
    static B: MapLoader = MapLoader { game: "b", load: b_map, words: Some(&B_WORDS) };
    static B_PARTS: MapDressing = MapDressing { game: "b", dress: b_parts };
    static SOURCES: MapSources = MapSources {
        loaders: &[&A, &B],
        gameplay: Some(&B_PARTS),
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
            assert!(!o.textures && o.hdr_level == Some(0), "{o:?}");
            Ok(MapData { name: name.into(), ..default() })
        }
        static L: MapLoader = MapLoader { game: "l", load: lean_only, words: None };
        static S: MapSources =
            MapSources { loaders: &[&L], gameplay: None, kept: &[], players: false, guests: &[] };
        S.load_with("l:x", &MapLoadOptions::lean()).unwrap();
        assert!(MapLoadOptions::default().textures, "everything by default");
    }

    #[test]
    fn a_load_without_a_level_leaves_it_to_each_maps_game() {
        // Mixed sources (Ultimate Gun Game): the level asked for, or none,
        // reaches each game's loader as it is; the loader applies its own
        // default (`SourceGame::hdr_default_level`) where there is none.
        fn level_seen(name: &str, o: &MapLoadOptions) -> Result<MapData, String> {
            Ok(MapData { name: format!("{name}/{}", o.hdr_level_or(if name == "tf" { 2 } else { 0 })), ..default() })
        }
        static T: MapLoader = MapLoader { game: "t", load: level_seen, words: None };
        static C: MapLoader = MapLoader { game: "c", load: level_seen, words: None };
        static S: MapSources =
            MapSources { loaders: &[&T, &C], gameplay: None, kept: &[], players: false, guests: &[] };
        let none = MapLoadOptions { hdr_level: None, ..default() };
        assert_eq!(S.load_with("t:tf", &none).unwrap().name, "tf/2");
        assert_eq!(S.load_with("c:cs", &none).unwrap().name, "cs/0");
        let set = MapLoadOptions::hdr(1);
        assert_eq!(S.load_with("t:tf", &set).unwrap().name, "tf/1", "a set level wins over the game's");
        assert_eq!(S.load_with("c:cs", &set).unwrap().name, "cs/1");
        assert_eq!(S.load_with("t:tf", &MapLoadOptions::hdr(0)).unwrap().name, "tf/0", "an explicit 0 is a level");
    }

    #[test]
    fn only_another_games_map_leaves_its_kit_unloaded() {
        fn kit_seen(name: &str, o: &MapLoadOptions) -> Result<MapData, String> {
            // The loader reports the kit it was asked for in the name.
            Ok(MapData { name: format!("{name}/{:?}", o.kit), ..default() })
        }
        static A2: MapLoader = MapLoader { game: "a", load: kit_seen, words: None };
        static B2: MapLoader = MapLoader { game: "b", load: kit_seen, words: None };
        static S2: MapSources = MapSources {
            loaders: &[&A2, &B2],
            gameplay: Some(&B_PARTS),
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

    /// With `players`, the map game's player bodies (those worn by name
    /// too: a class's body, DoD:S's) and HUD look stay (asked of the loader
    /// as `KitLoad::Players`, which loads no hostages), after the
    /// gameplay's own bodies; its other parts still go.
    #[test]
    fn a_map_for_players_of_its_game_keeps_their_bodies_and_hud() {
        fn with_players(name: &str, o: &MapLoadOptions) -> Result<MapData, String> {
            assert!(matches!(o.kit, KitLoad::Players(_)) && o.kit.bodies() && o.kit.hud() && !o.kit.parts());
            let mut map = a_map(name, o)?;
            map.kit.characters.push(body("a", None));
            map.kit.characters.push(body("a", Some("a_class")));
            map.kit.hud = Some(default());
            Ok(map)
        }
        fn b_bodies(map: &mut MapData, _: &MapLoadOptions) -> Result<(), String> {
            map.kit.characters.push(body("b", Some("b_soldier")));
            Ok(())
        }
        static A3: MapLoader = MapLoader { game: "a", load: with_players, words: None };
        static B_BODIES: MapDressing = MapDressing { game: "b", dress: b_bodies };
        static S3: MapSources = MapSources {
            loaders: &[&A3, &B],
            gameplay: Some(&B_BODIES),
            kept: &["a:gun"],
            players: true,
            guests: &[],
        };
        let map = S3.load("a:x", 0).unwrap();
        let bodies: Vec<_> = map.kit.characters.iter().map(|c| (c.game, c.name.as_deref())).collect();
        assert_eq!(
            bodies,
            [(Some("b"), Some("b_soldier")), (Some("a"), None), (Some("a"), Some("a_class"))],
            "the gameplay's first; the named class body kept"
        );
        assert!(map.kit.hud.is_some(), "the HUD look stays");
        assert!(map.kit.nav.is_none(), "the rest goes");
        assert_eq!(map.kit.held.len(), 1);
    }

    /// A guest dresses the maps of the other games only, after the
    /// gameplay's own dressing, and a failing one is a warning.
    /// A view model of game `game` named `key`, its one mesh on texture
    /// `texture` of the dressing's list.
    fn view_model(key: &str, texture: usize) -> crate::map::MapViewModel {
        crate::map::MapViewModel {
            key: key.into(),
            model: crate::map::MapModel {
                meshes: vec![crate::map::MapMesh { texture: Some(texture), ..default() }],
                ..default()
            },
            bones: Vec::new(),
            root: default(),
            animations: None,
            right_handed: true,
            allow_flipping: false,
            attachments: Vec::new(),
            light_origin: default(),
        }
    }

    fn texture(name: &str) -> crate::map::MapTexture {
        crate::map::MapTexture { name: name.into(), srgb: true, mips: Vec::new(), width: 1, height: 1, rgba8: vec![0; 4], bc: None }
    }

    /// The slow guest's dressing waits for this (the test lets it go).
    static SLOW_GOES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    /// The guests are dressed side by side, and each one's view models are
    /// handed over as soon as it is done (`MapLoadOptions::early`): a fast
    /// guest's gun is out while a slow one still converts, each view model
    /// with the textures it names (renumbered from 0); the map comes out
    /// as a dressing in turn makes it.
    #[test]
    fn each_guests_view_models_come_as_it_is_done_not_after_every_guest() {
        fn guns(map: &mut MapData, game: &str) {
            let base = map.textures.len();
            map.textures.extend([texture(&format!("{game}/unused")), texture(&format!("{game}/gun"))]);
            map.kit.view_models.push(view_model(&format!("{game}:gun"), base + 1));
        }
        fn fast(map: &mut MapData, _: &MapLoadOptions) -> Result<(), String> {
            guns(map, "fast");
            Ok(())
        }
        fn slow(map: &mut MapData, _: &MapLoadOptions) -> Result<(), String> {
            while !SLOW_GOES.load(std::sync::atomic::Ordering::Acquire) {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            guns(map, "slow");
            Ok(())
        }
        static SLOW: MapDressing = MapDressing { game: "slow", dress: slow };
        static FAST: MapDressing = MapDressing { game: "fast", dress: fast };
        static S: MapSources = MapSources {
            loaders: &[&A, &B],
            gameplay: Some(&B_PARTS),
            kept: &[],
            players: false,
            // The slow one first: dressed in turn, the fast one waited for it.
            guests: &[&SLOW, &FAST],
        };
        let early = EarlyViewModels::default();
        let options = MapLoadOptions { early: Some(early.clone()), ..default() };
        let load = std::thread::spawn(move || S.load_with("a:x", &options));
        // The fast guest's gun comes while the slow one is still held.
        let began = std::time::Instant::now();
        let fast_one = loop {
            if let Some(g) = early.take().into_iter().find(|g| g.game == "fast") {
                break g;
            }
            assert!(began.elapsed().as_secs() < 30, "the fast guest's view models never came");
            std::thread::sleep(std::time::Duration::from_millis(1));
        };
        assert!(!load.is_finished(), "the load still waits for the slow guest");
        assert_eq!(fast_one.view_models.len(), 1);
        let v = &fast_one.view_models[0];
        assert_eq!(v.key, "fast:gun");
        let t = v.model.meshes[0].texture.expect("a texture");
        assert_eq!(fast_one.textures.len(), 1, "only the textures its view models name");
        assert_eq!(fast_one.textures[t].name, "fast/gun", "its index names its own texture");
        SLOW_GOES.store(true, std::sync::atomic::Ordering::Release);
        let map = load.join().unwrap().unwrap();
        let slow_one = early.take().into_iter().find(|g| g.game == "slow").expect("the slow guest's too");
        assert_eq!(slow_one.textures[slow_one.view_models[0].model.meshes[0].texture.unwrap()].name, "slow/gun");
        // The map: the guests in their order, as dressing each in turn does.
        let keys: Vec<&str> = map.kit.view_models.iter().map(|v| v.key.as_str()).collect();
        assert_eq!(keys, ["slow:gun", "fast:gun"]);
        for v in &map.kit.view_models {
            let game = v.key.split(':').next().unwrap();
            let t = v.model.meshes[0].texture.unwrap();
            assert_eq!(map.textures[t].name, format!("{game}/gun"), "{}", v.key);
        }
        // Without a taker, nothing is handed over (and the load is the same).
        let plain = S.load_with("a:x", &MapLoadOptions::default()).unwrap();
        assert_eq!(plain.kit.view_models.len(), 2);
        assert!(early.take().is_empty());
    }

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
            kept: &[],
            players: false,
            guests: &[&A_GUEST, &C_GUEST],
        };
        let other = S4.load("b:y", 0).unwrap();
        assert!(other.kit.characters.iter().any(|c| c.name.as_deref() == Some("a_guest")), "a's guest on b's map");
        assert!(other.warnings.iter().any(|w| w.contains("c's parts: no install")), "{:?}", other.warnings);
        let own = S4.load("a:x", 0).unwrap();
        assert!(own.kit.characters.iter().all(|c| c.name.as_deref() != Some("a_guest")), "not on its own game's map");
        // A match that deals none of a's parts: a isn't dressed.
        let dealt = MapLoadOptions { dealt: Some(Arc::new(BTreeSet::from(["b".to_string()]))), ..MapLoadOptions::default() };
        let undealt = S4.load_with("b:y", &dealt).unwrap();
        assert!(undealt.kit.characters.iter().all(|c| c.name.as_deref() != Some("a_guest")), "a isn't dealt");
        assert!(!undealt.warnings.iter().any(|w| w.contains("c's parts")), "nor c");
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
        static L: MapLoader = MapLoader { game: "a", load: loader, words: None };
        static D: MapDressing = MapDressing { game: "b", dress: dressing };
        static S: MapSources =
            MapSources { loaders: &[&L], gameplay: Some(&D), kept: &[], players: false, guests: &[] };
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
        assert_eq!(names.get("WOOD_CRATE"), Some("Wood"), "by its kind");
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
