//! `CaData`: the data one app plays Combat Arms with, a `Resource` its
//! plugins read (architecture review E2, X3). It was process-wide: a
//! switch (`use_builtin_data_only`) and a pile of `OnceLock` caches
//! (weapon set, attachments, character records, gear, movement,
//! missions), so every app in a process (a `NetSim`'s server and clients,
//! the tests of one binary) shared whichever data loaded first.
//!
//! A `CaData` holds the install's attribute files (or none) and what is
//! derived from them, each derived table filled on first use *inside this
//! value*, so two apps with different data never see each other's:
//!
//! - `CaData::builtin()`: the built-in documents only, never the install
//!   (the harness's `CaSim`, scenario tests);
//! - `CaData::install()`: the install's attribute data over the built-in
//!   when it can be read (the games' default, `CaData::default()`);
//! - `CaData::builtin_with_weapons(docs)`: the built-in data with other
//!   weapon documents (tests, minigame tuning).
//!
//! `builtin()` is one value for the process (read-only tables of ours).
//! `install()` is the install's: the apps of a process share it while any
//! holds it (the asset store's `shared` register keeps only a weak
//! reference), and it is freed with the last; each Combat Arms app holds
//! it in its resource or its map loads' context (`map::LoadContext`,
//! `install_for`), so its loads never reach for a process-wide memo. Its
//! parts read from the install fill on first use: the attribute files
//! (`ATTRIBUTES.rez` takes seconds to read and decrypt), the mounted
//! archives (`mount`, themselves shared the same way) and what the game
//! works out from them once per install (`memo`: the converted character
//! models, the view models loaded on demand, the parsed effect file, the
//! front end's UI data). The weapon set built from it is leaked (weapon
//! IDs are `&'static str`), once per install value, and so are the
//! install's hit and skeleton tables (`static_tables`).
//!
//! Known shortcuts:
//! - A new install value after the last one was freed reads the files
//!   again and leaks its weapon set and tables again (only a process
//!   whose apps all went, a test binary's next test).

use std::{
    collections::BTreeSet,
    ops::Deref,
    sync::{Arc, OnceLock},
};

use bevy::prelude::*;
use toml::Table;

use super::{
    InstallAttributes, WEAPON_DATA, WeaponRecord, doc_id, install_weapons_by_id, is_install_name, merge_mods,
    merge_weapon_docs_with_mods, open_attributes, with_install_movement,
};
use crate::{mount::Mount, weapon::data::WeaponSet};

use super::super::{
    attachments::CaModTable,
    characters::records::CharacterData,
    gear::records::GearData,
    missions::InstallMissions,
    movement::{MovementData, movement_data},
};

/// The install data's and mount's keys in the asset store's `shared`
/// register.
const INSTALL_KEY: &str = "combat_arms:install";
const MOUNT_KEY: &str = "combat_arms:mount";

/// The data an app plays Combat Arms with (module doc). Cheap to clone;
/// derefs to `CaDataInner`.
#[derive(Resource, Clone)]
pub struct CaData(Arc<CaDataInner>);

impl Default for CaData {
    fn default() -> Self {
        Self::install()
    }
}

impl Deref for CaData {
    type Target = CaDataInner;
    fn deref(&self) -> &CaDataInner {
        &self.0
    }
}

/// What a `CaData` holds.
pub struct CaDataInner {
    builtin_only: bool,
    /// Built by hand (`with_attributes`): its files name no install file.
    hand_built: bool,
    /// The install's attribute files: given (`with_attributes`), none
    /// (`builtin`), or read on first use (`install`).
    attributes: OnceLock<Option<InstallAttributes>>,
    /// The install's mounted archives, asked once (`mount`).
    mount: OnceLock<Option<Arc<Mount>>>,
    /// What the game works out from this data once (`memo`).
    memos: chimera_core::memo::Memos,
    /// The built-in weapon documents of this data.
    docs: Vec<(String, String)>,
    weapon_set: OnceLock<&'static WeaponSet>,
    /// The documents `weapon_set` was parsed from (merged or built in).
    weapon_docs: OnceLock<&'static [(String, String)]>,
    attachments: OnceLock<CaModTable>,
    movement: OnceLock<MovementData>,
    gear: OnceLock<GearData>,
    characters: OnceLock<CharacterData>,
    missions: OnceLock<Option<InstallMissions>>,
    builtin_ids: OnceLock<BTreeSet<String>>,
    /// `install_weapon`'s index: each id to its record's place in the
    /// install's weapons file (`install_weapons_by_id`), made on first use.
    install_index: OnceLock<bevy::platform::collections::HashMap<String, usize>>,
}

impl CaData {
    fn new(builtin_only: bool, attributes: Option<Option<InstallAttributes>>, docs: Vec<(String, String)>) -> Self {
        Self(Arc::new(Self::inner(builtin_only, attributes, docs)))
    }

    /// `attributes`: None to read the install's on first use.
    fn inner(builtin_only: bool, attributes: Option<Option<InstallAttributes>>, docs: Vec<(String, String)>) -> CaDataInner {
        let cell = OnceLock::new();
        if let Some(a) = attributes {
            let _ = cell.set(a);
        }
        CaDataInner {
            builtin_only,
            hand_built: false,
            attributes: cell,
            mount: OnceLock::new(),
            memos: Default::default(),
            docs,
            weapon_set: OnceLock::new(),
            weapon_docs: OnceLock::new(),
            attachments: OnceLock::new(),
            movement: OnceLock::new(),
            gear: OnceLock::new(),
            characters: OnceLock::new(),
            missions: OnceLock::new(),
            builtin_ids: OnceLock::new(),
            install_index: OnceLock::new(),
        }
    }

    fn built_in_docs() -> Vec<(String, String)> {
        WEAPON_DATA.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect()
    }

    /// The built-in data only: never the install's attribute files, its
    /// mount or its sounds (scenario tests: their numbers are ours).
    pub fn builtin() -> Self {
        Self::builtin_static().clone()
    }

    /// `builtin()` by reference: its tables are read-only tables for code
    /// with no app to ask (`characters::records::builtin`).
    pub fn builtin_static() -> &'static Self {
        static BUILTIN: OnceLock<CaData> = OnceLock::new();
        BUILTIN.get_or_init(|| Self::new(true, Some(None), Self::built_in_docs()))
    }

    /// The install's attribute data over the built-in when it can be read
    /// (`[games.combat_arms] path` and the `V1` key in the local config);
    /// else the built-in numbers, with the install's files still
    /// mountable. Shared by the apps of the process while any holds it
    /// (module doc); its files are read on first use.
    pub fn install() -> Self {
        Self(crate::mount::assets::shared::shared(INSTALL_KEY, || {
            Self::inner(false, None, Self::built_in_docs())
        }))
    }

    /// The install data of the app whose map loads have `context`
    /// (`map::LoadContext`: Combat Arms' plugin puts it there): what its
    /// loaders and on-demand loads read. `install()` with no app.
    pub fn install_for(context: &crate::map::LoadContext) -> Self {
        context.get::<CaData>().map_or_else(Self::install, |d| (*d).clone())
    }

    /// Whether `self` and `other` are one data value.
    pub fn same(&self, other: &CaData) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// The built-in data with `docs` as the weapon documents (name, TOML
    /// text) instead of `WEAPON_DATA`: a test's or a minigame's own
    /// weapon tuning. Not shared with any other app.
    pub fn builtin_with_weapons(docs: Vec<(String, String)>) -> Self {
        Self::new(true, Some(None), docs)
    }

    /// An install's attribute files, as read, over the built-in data
    /// (tests that build an install by hand). Not shared.
    pub fn with_attributes(attributes: InstallAttributes) -> Self {
        let mut inner = Self::inner(false, Some(Some(attributes)), Self::built_in_docs());
        inner.hand_built = true;
        Self(Arc::new(inner))
    }

    /// The built-in numbers with `mount` as the install's files (tests of
    /// what plays or reads a file: sounds built in the test). Not shared.
    pub fn builtin_with_mount(mount: Mount) -> Self {
        let mut inner = Self::inner(false, Some(None), Self::built_in_docs());
        inner.hand_built = true;
        let _ = inner.mount.set(Some(Arc::new(mount)));
        Self(Arc::new(inner))
    }

    /// The app's data: its resource if a plugin or the game's setup put
    /// one there, else the default (inserted now, so every plugin of the
    /// app agrees whatever order they build in). The install data its map
    /// loads read goes into its `map::LoadContext` too (`install_for`):
    /// held while the app lives.
    pub fn of(app: &mut App) -> Self {
        app.init_resource::<CaData>();
        let context = app.world_mut().get_resource_or_init::<crate::map::LoadContext>().clone();
        context.get_or_init(Self::install);
        app.world().resource::<CaData>().clone()
    }
}

impl CaDataInner {
    /// Whether this data reads nothing of the install at run time (its
    /// numbers, mount and sounds: the effects go silent too).
    pub fn builtin_only(&self) -> bool {
        self.builtin_only
    }

    /// The install's mount (its archives' directories read once, shared
    /// by the data values that hold it: the module doc); `None` without
    /// an install, or in `builtin()` data (no map, sound or texture reads
    /// the install).
    pub fn mount(&self) -> Option<Arc<Mount>> {
        if self.builtin_only {
            return None;
        }
        self.mount
            .get_or_init(|| crate::mount::assets::shared::shared_opt(MOUNT_KEY, || super::super::INFO.mount()))
            .clone()
    }

    /// Whether the install has `path`: `None` when this data can't say
    /// (no install, `builtin()` or hand-built data: its file names are
    /// stand-ins).
    pub fn install_has(&self, path: &str) -> Option<bool> {
        if self.hand_built {
            return None;
        }
        self.mount().map(|m| m.contains(path))
    }

    /// The install's attribute data: `None` without an install, with
    /// `builtin()`, or when `ATTRIBUTES.rez` can't be read (no V1 key:
    /// logged). Read on first use.
    pub fn attributes(&self) -> Option<&InstallAttributes> {
        self.attributes
            .get_or_init(|| {
                let install = crate::mount::config::LocalConfig::load().ok()?.game_path(super::super::GAME)?;
                let path = install.join("Game").join("ATTRIBUTES.rez");
                if !path.is_file() {
                    // Served from the asset store (`mount::served`): the
                    // archive's files, recorded at their paths.
                    let mount = self.mount()?;
                    return super::attributes_from_mount(&mount)
                        .map_err(|e| info!("Combat Arms: built-in data only: {e}"))
                        .ok();
                }
                open_attributes(&path)
                    .map_err(|e| info!("Combat Arms: built-in data only: {e}"))
                    .ok()
            })
            .as_ref()
    }

    /// The value of type `T` this data worked out once (`init` the first
    /// time): what the game reads from the install per install, not per
    /// app or map (the converted character models, the view models loaded
    /// on demand, the parsed effect file, the UI data). Freed with the
    /// data.
    pub fn memo<T: std::any::Any + Send + Sync>(&self, init: impl FnOnce() -> T) -> Arc<T> {
        self.memos.get_or_init(init)
    }

    /// The server's `ReloadCheckTime` (attribute_data.md Corrections 3):
    /// the install's `[TimeWeight]`, else the built-in 0.8.
    pub fn reload_check_time(&self) -> f64 {
        use super::attributes::ServerTimeWeight;
        self.attributes()
            .and_then(|a| a.time_weight)
            .map_or(ServerTimeWeight::BUILT_IN, |t| t.reload_check_time)
    }

    /// Combat Arms' weapons as data, parsed once per `CaData`: the
    /// built-in documents, with the install's attribute data over them
    /// when it can be read. Bad built-in data panics (the tests load it);
    /// merged data that fails falls back to it. Leaked (`&'static`, as
    /// every weapon set), once per process for the same documents
    /// (`interned`): a client's startup map loads with install data of its
    /// own, freed after, and its set (about 240 MB) stayed beside the
    /// app's (docs/performance.md, "A client's memory").
    pub fn weapon_set(&self) -> &'static WeaponSet {
        self.weapon_set.get_or_init(|| {
            let docs: Vec<(&str, &str)> = self.docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
            let mods = self.attributes().and_then(|a| a.mods.as_ref());
            let merged = self.attributes().and_then(|a| a.weapons.as_ref()).and_then(|w| {
                let mpu = movement_data().meters_per_unit as f64;
                merge_weapon_docs_with_mods(&docs, w, mods, mpu)
                    .map_err(|e| warn!("Combat Arms: the install's weapons weren't merged: {e}"))
                    .ok()
            });
            let parsed = merged.and_then(|merged| {
                interned(merged, |m| {
                    let refs: Vec<(&str, &str)> = m.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
                    WeaponSet::parse(&refs, &[])
                        .map_err(|e| warn!("Combat Arms: the install's weapons don't load, using the built-in ones: {e}"))
                        .ok()
                })
            });
            let (set, used) = parsed.unwrap_or_else(|| {
                interned(self.docs.clone(), |_| {
                    Some(WeaponSet::parse(&docs, &[]).unwrap_or_else(|e| panic!("Combat Arms weapon data: {e}")))
                })
                .expect("the built-in set parses or panics")
            });
            let _ = self.weapon_docs.set(used);
            set
        })
    }

    /// The weapon documents (name, TOML text) `weapon_set` is parsed
    /// from, in registry order: the install's merged over ours, or ours
    /// (what a server sends a client whose own differ: `served`).
    pub fn weapon_docs(&self) -> &[(String, String)] {
        self.weapon_set();
        self.weapon_docs.get().expect("set with the weapon set")
    }

    /// The attachment records: the built-in ones (data/attachments/mods.toml)
    /// with the install's modification file over them by name.
    pub fn attachments(&self) -> &CaModTable {
        self.attachments.get_or_init(|| {
            let mut table = CaModTable::built_in().clone();
            if let Some(mods) = self.attributes().and_then(|a| a.mods.as_ref()) {
                merge_mods(&mut table, mods);
            }
            table
        })
    }

    /// The movement data with the install's body type over it.
    pub fn movement(&self) -> &MovementData {
        self.movement
            .get_or_init(|| with_install_movement(movement_data().clone(), self.attributes()))
    }

    /// The gear data: ours with the install's records over them.
    pub fn gear(&self) -> &GearData {
        self.gear.get_or_init(|| {
            let data = GearData::built_in();
            match self.attributes().and_then(|a| a.gear.as_ref()) {
                Some(install) => data.with_install(install),
                None => data,
            }
        })
    }

    /// The character data (data/characters.toml) with the install's
    /// marks, models and weapon looks over it.
    pub fn characters(&self) -> &CharacterData {
        self.characters.get_or_init(|| CharacterData::load(self))
    }

    /// The install's missions (the mounted archives); `None` in
    /// `builtin()` data or without them.
    pub fn missions(&self) -> Option<&InstallMissions> {
        self.missions
            .get_or_init(|| {
                if self.builtin_only {
                    return None;
                }
                let mount = self.mount()?;
                Some(super::super::missions::read_install(&|p| mount.read(p).ok()))
            })
            .as_ref()
    }

    /// The IDs of our own weapon documents (the weapons a player is
    /// given: `inventory`, the modes, the bots). The models a map loads
    /// are theirs only: the install adds some 2,000 weapons, too many to
    /// load per map.
    pub fn builtin_weapon_ids(&self) -> &BTreeSet<String> {
        self.builtin_ids.get_or_init(|| {
            self.docs
                .iter()
                .filter_map(|(_, text)| {
                    let t = text.parse::<Table>().ok()?;
                    // A detonator is never seen in a hand (no held model).
                    if t.get("hooks").and_then(|h| h.get(super::super::special::CONTROLLER)).is_some() {
                        return None;
                    }
                    doc_id(&t).map(String::from)
                })
                .collect()
        })
    }

    /// Our weapon document for an install weapon `Name` (`is_install_name`:
    /// the mission files' objective weapons, missions.md F3), if the set
    /// has it.
    pub fn weapon_for_name(&self, name: &str) -> Option<&'static str> {
        self.weapon_set()
            .weapons
            .iter()
            .find(|w| w.id.strip_prefix("combat_arms:").is_some_and(|id| is_install_name(name, id)))
            .map(|w| w.id.as_str())
    }

    /// Our weapon document for an install weapon record's index (the
    /// mission files' weapon ids: Arms Race ladders, the infected's and
    /// the agent's loadouts), with the V1 key; None without the install
    /// or a document for it.
    pub fn weapon_for_index(&self, index: i32) -> Option<&'static str> {
        let weapons = self.attributes()?.weapons.as_ref()?;
        let name = &weapons.weapons.get(usize::try_from(index).ok()?)?.name;
        self.weapon_for_name(name)
    }

    /// The install's record of weapon `id` (ours or one merged from the
    /// install: `combat_arms:<slug of its Name>`), when the install's
    /// weapons file was read.
    pub fn install_weapon(&self, id: &str) -> Option<&WeaponRecord> {
        // By an index: the file has some 2,400 weapons, and a search made
        // each one's slug again on every call (a zoomed sniper asks every
        // tick: docs/performance.md, "Performance pass").
        let file = self.attributes()?.weapons.as_ref()?;
        let index = self.install_index.get_or_init(|| {
            install_weapons_by_id(file)
                .into_iter()
                .filter_map(|(id, r)| Some((id, file.weapons.iter().position(|w| std::ptr::eq(w, r))?)))
                .collect()
        });
        index.get(id).map(|i| &file.weapons[*i])
    }
}

/// The weapon set parsed from `docs` (`parse`, leaked), once per process
/// for the same documents (`shared::leaked`): install data made again (a
/// client's startup map's, an app made again) costs no second set. None
/// when `parse` gives none (nothing kept).
fn interned(
    docs: Vec<(String, String)>,
    parse: impl FnOnce(&[(String, String)]) -> Option<WeaponSet>,
) -> Option<(&'static WeaponSet, &'static [(String, String)])> {
    let (docs, set) = crate::mount::assets::shared::leaked("combat_arms:weapon_set", docs, |d| parse(d))?;
    Some((set, docs.as_slice()))
}
