//! What a world holds, counted (docs/performance.md, "Map changes that
//! slowed the tick"): entities by archetype (their component sets), observers,
//! systems per schedule, resources and every reflect-registered asset
//! store's length. `chimera_census` (the console; `command`, registered by
//! the client's `perf`) prints it; `chimera_census diff` what changed
//! since the last one or a saved one, new archetypes' and tables'
//! component sets included (the way to find what grows per map change:
//! take one, change maps, diff).
//! Headless tests take a `Census` of a `Sim`'s world and compare two.
//! `chimera_census varies [<component>]` lists what comes and goes on the
//! archetypes holding a component (`Health`: the characters); `chimera_census
//! age <n> [sparse]` puts a character through `n` new archetypes (a table
//! each, unless `sparse`): a long session's world at once, to measure what
//! that costs a frame (docs/performance.md, "A session's frame rate").
//! `CensusAssets` counts asset stores that aren't reflect-registered (the
//! materials, the sounds).

use std::collections::{BTreeMap, BTreeSet};

use bevy::{asset::ReflectAsset, ecs::observer::Observer, prelude::*};

/// Asset stores the census counts besides the reflect-registered ones
/// (`CensusAppExt::census_asset`).
#[derive(Resource, Default)]
pub struct CensusAssets(Vec<(&'static str, fn(&World) -> usize)>);

/// `census_asset`: a store for the census to count.
pub trait CensusAppExt {
    fn census_asset<A: Asset>(&mut self, name: &'static str) -> &mut Self;
}

impl CensusAppExt for App {
    fn census_asset<A: Asset>(&mut self, name: &'static str) -> &mut Self {
        self.world_mut().get_resource_or_init::<CensusAssets>().0.push((name, |w| {
            w.get_resource::<Assets<A>>().map_or(0, |a| a.len())
        }));
        self
    }
}

/// One world's counts. Names are short type names; archetype keys their
/// sorted component names joined by `+`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Census {
    pub entities: usize,
    /// Archetypes with entities.
    pub archetypes: usize,
    /// Every archetype and table the world has made (Bevy never frees
    /// one), and the component types registered.
    pub archetypes_made: usize,
    pub tables: usize,
    pub components: usize,
    pub observers: usize,
    pub resources: usize,
    /// Entities per archetype with any (component sets).
    pub by_archetype: BTreeMap<String, usize>,
    /// Every archetype's component set, empty ones too.
    pub made: BTreeSet<String>,
    /// Every table's component set (an archetype's table components).
    pub made_tables: BTreeSet<String>,
    /// Systems per schedule label.
    pub systems: BTreeMap<String, usize>,
    /// Assets per reflect-registered `Assets<T>` (short name of `T`).
    pub assets: BTreeMap<String, usize>,
}

impl Census {
    pub fn take(world: &mut World) -> Self {
        let mut by_archetype = BTreeMap::new();
        let mut made = BTreeSet::new();
        let mut made_tables = BTreeSet::new();
        let components = world.components();
        let names_of = |ids: &mut dyn Iterator<Item = bevy::ecs::component::ComponentId>| {
            let mut names: Vec<String> =
                ids.filter_map(|c| components.get_name(c)).map(|n| n.shortname().to_string()).collect();
            names.sort();
            names.join("+")
        };
        for a in world.archetypes().iter() {
            made_tables.insert(names_of(&mut a.table_components()));
            let key = names_of(&mut a.iter_components());
            if !a.is_empty() {
                *by_archetype.entry(key.clone()).or_insert(0) += a.len() as usize;
            }
            made.insert(key);
        }
        let archetypes = world.archetypes().iter().filter(|a| !a.is_empty()).count();
        let archetypes_made = world.archetypes().len();
        let tables = world.storages().tables.iter().count();
        let components = world.components().len();
        let entities = world.entities().count_spawned() as usize;
        let observers = world.query::<&Observer>().iter(world).count();
        let resources = world.iter_resources().count();
        let systems = world
            .get_resource::<Schedules>()
            .map(|s| s.iter().map(|(label, s)| (format!("{label:?}"), s.systems_len())).collect())
            .unwrap_or_default();
        let mut assets = BTreeMap::new();
        if let Some(counted) = world.get_resource::<CensusAssets>() {
            for (name, len) in &counted.0 {
                assets.insert(name.to_string(), len(world));
            }
        }
        if let Some(registry) = world.get_resource::<AppTypeRegistry>().cloned() {
            for reg in registry.read().iter() {
                if let Some(ra) = reg.data::<ReflectAsset>() {
                    let name = reg.type_info().type_path_table().short_path().to_string();
                    assets.insert(name, ra.len(world));
                }
            }
        }
        Self {
            entities,
            archetypes,
            archetypes_made,
            tables,
            components,
            observers,
            resources,
            by_archetype,
            made,
            made_tables,
            systems,
            assets,
        }
    }

    /// The counts that differ from `before`, as `name: before -> now`
    /// lines (totals first, then archetypes, systems and assets).
    pub fn diff(&self, before: &Census) -> Vec<String> {
        let mut out = Vec::new();
        for (name, a, b) in [
            ("entities", before.entities, self.entities),
            ("archetypes", before.archetypes, self.archetypes),
            ("archetypes made", before.archetypes_made, self.archetypes_made),
            ("tables", before.tables, self.tables),
            ("components", before.components, self.components),
            ("observers", before.observers, self.observers),
            ("resources", before.resources, self.resources),
        ] {
            if a != b {
                out.push(format!("{name}: {a} -> {b}"));
            }
        }
        for k in self.made.difference(&before.made) {
            out.push(format!("new archetype: {k}"));
        }
        for k in self.made_tables.difference(&before.made_tables) {
            out.push(format!("new table: {k}"));
        }
        for (kind, was, now) in [
            ("entities", &before.by_archetype, &self.by_archetype),
            ("systems", &before.systems, &self.systems),
            ("assets", &before.assets, &self.assets),
        ] {
            let keys: BTreeSet<_> = was.keys().chain(now.keys()).collect();
            for k in keys {
                let (a, b) = (was.get(k).copied().unwrap_or(0), now.get(k).copied().unwrap_or(0));
                if a != b {
                    out.push(format!("{kind} {k}: {a} -> {b}"));
                }
            }
        }
        out
    }

    /// A readout: totals, the `top` largest archetypes, systems per
    /// schedule and non-empty asset stores.
    pub fn report(&self, top: usize) -> String {
        let mut s = format!(
            "census: {} entities in {} archetypes ({} made, {} tables, {} component types), {} observers, \
             {} resources, {} systems\n",
            self.entities,
            self.archetypes,
            self.archetypes_made,
            self.tables,
            self.components,
            self.observers,
            self.resources,
            self.systems.values().sum::<usize>()
        );
        let mut arch: Vec<_> = self.by_archetype.iter().collect();
        arch.sort_by(|a, b| b.1.cmp(a.1));
        for (k, n) in arch.into_iter().take(top) {
            s += &format!("  {n:6}  {k}\n");
        }
        s += "systems:";
        for (k, n) in &self.systems {
            s += &format!(" {k} {n},");
        }
        s += "\nassets:";
        for (k, n) in self.assets.iter().filter(|(_, n)| **n > 0) {
            s += &format!(" {k} {n},");
        }
        s
    }
}

/// The censuses `chimera_census` took: the last (key "") and those saved
/// by name.
#[derive(Resource, Default)]
struct Censuses(BTreeMap<String, Census>);

/// `chimera_census`'s help line (the client's `perf` registers it).
pub const HELP: &str = "chimera_census [<top>|diff [<name>]|save <name>|varies [<component>]|age <n> [sparse]]: count \
     entities by archetype, observers, systems per schedule, resources and assets by type (the <top> largest \
     archetypes, default 15); `save <name>` keeps this census under a name; `diff`: what changed since the last \
     census (or since the one saved as <name>), new archetypes and tables included; `varies`: what comes and goes \
     on the archetypes holding <component> (default Health: the characters); `age`: put a character through <n> \
     new archetypes, a table each unless `sparse` (a long session's world at once).";

/// Marks for `age`: table components, and sparse-set ones.
#[derive(Component)]
struct AgeTable<const N: usize>;

#[derive(Component)]
#[component(storage = "SparseSet")]
struct AgeSparse<const N: usize>;

macro_rules! age_toggle {
    ($t:ident, $e:expr, $bit:expr, $on:expr, $($n:literal)*) => {
        match $bit {
            $($n => { if $on { $e.insert($t::<$n>); } else { $e.remove::<$t<$n>>(); } })*
            _ => {}
        }
    };
}

/// Toggle mark `bit` (below 16) on `e`.
fn age_mark(e: &mut EntityWorldMut, bit: u32, on: bool, sparse: bool) {
    if sparse {
        age_toggle!(AgeSparse, e, bit, on, 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15);
    } else {
        age_toggle!(AgeTable, e, bit, on, 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15);
    }
}

/// How many combinations `age` has: one more mark than this would pass
/// any session.
pub const AGE_MOST: u32 = 65535;

/// A character through `n` new archetypes: each step of a Gray code turns
/// one mark on or off, so each visits one set not seen. Every query
/// matching characters matches them, as it does a real session's. Err
/// without a character.
pub fn age(world: &mut World, n: u32, sparse: bool) -> Result<(usize, usize), String> {
    // By name: this layer doesn't know the characters' components.
    let components = world.components();
    let e = world
        .archetypes()
        .iter()
        .filter(|a| !a.is_empty())
        .find(|a| {
            a.iter_components()
                .filter_map(|c| components.get_name(c))
                .any(|n| n.shortname().to_string() == "Health")
        })
        .and_then(|a| a.entities().first().map(|e| e.id()))
        .ok_or("no character to age")?;
    let n = n.min(AGE_MOST);
    let mut entity = world.entity_mut(e);
    let mut code = 0u32;
    for i in 1..=n {
        let bit = i.trailing_zeros();
        code ^= 1 << bit;
        age_mark(&mut entity, bit, code & (1 << bit) != 0, sparse);
    }
    for bit in 0..16 {
        if code & (1 << bit) != 0 {
            age_mark(&mut entity, bit, false, sparse);
        }
    }
    Ok((world.archetypes().len(), world.storages().tables.iter().count()))
}

/// The components that come and go on the archetypes holding `component`:
/// how many of them hold each, the most first, then how many tables they
/// make and the components all of them hold.
fn varies(census: &Census, component: &str) -> String {
    let holding = |sets: &BTreeSet<String>| -> Vec<BTreeSet<String>> {
        sets.iter()
            .map(|k| k.split('+').map(str::to_string).collect::<BTreeSet<String>>())
            .filter(|s| s.contains(component))
            .collect()
    };
    let archetypes = holding(&census.made);
    let tables = holding(&census.made_tables);
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for s in &archetypes {
        for c in s {
            *counts.entry(c.as_str()).or_default() += 1;
        }
    }
    let always = counts.values().filter(|n| **n == archetypes.len()).count();
    let mut rows: Vec<(&str, usize)> = counts.into_iter().filter(|(_, n)| *n != archetypes.len()).collect();
    rows.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let list: Vec<String> = rows.iter().map(|(c, n)| format!("{c} {n}")).collect();
    format!(
        "{} archetypes and {} tables hold {component}; {always} components are on all of them; the rest: {}",
        archetypes.len(),
        tables.len(),
        list.join(", ")
    )
}

/// `chimera_census [<top>|diff [<name>]|save <name>|varies [<component>]|age <n> [sparse]]`.
pub fn command(w: &mut World, a: &[String]) -> Result<Option<String>, String> {
    if a.first().map(String::as_str) == Some("age") {
        let n = a.get(1).and_then(|n| n.parse().ok()).ok_or("age: how many?")?;
        let (made, tables) = age(w, n, a.get(2).map(String::as_str) == Some("sparse"))?;
        return Ok(Some(format!("aged: {made} archetypes and {tables} tables made")));
    }
    let now = Census::take(w);
    let name = a.get(1).cloned().unwrap_or_default();
    let mut saved = w.get_resource_or_init::<Censuses>();
    let last = saved.0.insert(String::new(), now.clone());
    match a.first().map(String::as_str) {
        Some("save") => {
            saved.0.insert(name.clone(), now);
            Ok(Some(format!("census saved as '{name}'")))
        }
        Some("varies") => Ok(Some(varies(&now, a.get(1).map_or("Health", String::as_str)))),
        Some("diff") => {
            let before = if name.is_empty() { last } else { saved.0.get(&name).cloned() };
            let Some(before) = before else { return Ok(Some("no census to compare with".into())) };
            let d = now.diff(&before);
            Ok(Some(if d.is_empty() { "census: no change".into() } else { d.join("\n") }))
        }
        top => Ok(Some(now.report(top.and_then(|t| t.parse().ok()).unwrap_or(15)))),
    }
}
