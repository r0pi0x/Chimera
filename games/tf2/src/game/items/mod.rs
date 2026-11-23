//! TF2's unlocks (docs/plans/active/tf2-weapons.md, batches 0 and 1;
//! specs/tf2/item_attributes.md): each one a stock weapon class plus
//! attributes, read from a converted pack, never from the game at run time.
//!
//! **The converter** (`convert`, `CONVERTER` version `VERSION`): host-side,
//! `chimera_pack sets tf2 --only items` (`generated`) reads the install's
//! item schema (`scripts/items/items_game.txt`) and each item class's
//! weapon script (decrypted with the host's key through
//! `scripts::decrypting`; the key never printed) and writes one file,
//! `PACK_PATH` (`chimera/gen/tf2.items/v<VERSION>/items.toml`), into the
//! install build's manifest as set `items`: a record per weapon item
//! (its class, slot, classes, models, folded attributes, base and
//! effective numbers). Deterministic; `VERSION` is bumped whenever an
//! output byte could change. Nothing of the schema is committed.
//!
//! **In the game** (`load`, `ItemWeapon`): the pack is read from the
//! game's mount (the asset store's set, layered under the install or
//! standing in for it); data/items.toml (ours) names the items we play,
//! by definition index, with the weapon of ours each item class starts
//! from (`[bases]`). Each becomes a weapon document (`docs::item_doc`)
//! registered with the stock ones (`weapons::script_numbers::GunNumbers`,
//! so the server's documents carry them to clients), named `tf2:<name>`
//! (`tf2:winger`), its classes added to the class table
//! (`classes::Classes::add_gun`) and its holder's numbers on its
//! `tf2:item` hook (`mods`). A ladder draws one that deals damage
//! (its base's `class` kept); the jumpers don't. Without the pack (no
//! store set, an older store) no item is offered and stock plays on.
//! Wearables (`wearable`) make no weapon.
//!
//! Known shortcuts:
//! - An item looks like its base weapon only where the base's looks are
//!   keyed by the weapon's data (its hooks); its view and held models,
//!   kill icon and view-model animations are keyed by the stock ids, so
//!   an item shows the loading stand-in gun until those learn items
//!   (`model` and `world_model` are in the pack for that).

pub mod convert;
pub mod docs;
pub mod mods;

use std::collections::BTreeMap;

use serde::Deserialize;

pub use convert::{ItemPack, ItemRecord};

use crate::mount::Mount;

/// The converter's name (docs/formats.md).
pub const CONVERTER: &str = "tf2:item_stats";

/// Its version: bumped by hand whenever any output byte could change.
pub const VERSION: u32 = 1;

/// Where its output lives in TF2's manifest.
pub const GEN_ROOT: &str = "chimera/gen/tf2.items";

/// The pack's file.
pub const PACK_PATH: &str = "chimera/gen/tf2.items/v1/items.toml";

/// The generated set holding it.
pub const SET: &str = "items";

/// The item schema in the install.
pub const SCHEMA: &str = "scripts/items/items_game.txt";

/// The hook an item weapon's player numbers ride on.
pub const ITEM_HOOK: &str = "tf2:item";

/// data/items.toml (ours).
pub const DATA: chimera_core::game_data::GameData = chimera_core::game_data!("data/items.toml");

/// One item of data/items.toml.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ItemDef {
    pub index: u32,
    pub batch: u32,
    #[serde(default)]
    pub gun_game: Option<bool>,
    #[serde(default)]
    pub wearable: bool,
    #[serde(default)]
    pub note: Option<String>,
}

/// data/items.toml.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ItemsData {
    pub bases: BTreeMap<String, String>,
    pub items: BTreeMap<String, ItemDef>,
}

impl ItemsData {
    pub fn builtin() -> Self {
        toml::from_str(DATA.text()).unwrap_or_else(|e| panic!("tf2 data/items.toml: {e}"))
    }
}

/// An item weapon made from the pack.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemWeapon {
    /// Our id (`tf2:winger`).
    pub id: String,
    pub index: u32,
    /// Its name (the pack's, "The " left off).
    pub name: String,
    /// The weapon of ours it starts from.
    pub base: String,
    /// The classes that use it.
    pub classes: Vec<String>,
    /// Its document (TOML text).
    pub doc: String,
}

/// The pack's display name for `r`.
pub fn display_name(r: &ItemRecord) -> String {
    let n = r.name.trim();
    n.strip_prefix("The ").unwrap_or(n).to_string()
}

/// The item weapons of `pack` that data/items.toml names, built over
/// `docs` (the stock documents, file name and text) in data order. Items
/// the pack lacks, wearables and items with no base are left out (said
/// in the second list).
pub fn weapons(pack: &ItemPack, data: &ItemsData, docs: &[(String, String)]) -> (Vec<ItemWeapon>, Vec<String>) {
    let mut out = Vec::new();
    let mut notes = Vec::new();
    let by_id: BTreeMap<String, toml::Table> = docs
        .iter()
        .filter_map(|(_, t)| {
            let t = t.parse::<toml::Table>().ok()?;
            Some((t.get("id")?.as_str()?.to_string(), t))
        })
        .collect();
    let mut defs: Vec<(&String, &ItemDef)> = data.items.iter().collect();
    defs.sort_by_key(|(_, d)| d.index);
    for (name, def) in defs {
        if def.wearable {
            continue;
        }
        let Some(r) = pack.get(def.index) else {
            notes.push(format!("{name} ({}): not in the pack", def.index));
            continue;
        };
        let Some(base) = data.bases.get(&r.class) else {
            notes.push(format!("{name} ({}): no base for {}", def.index, r.class));
            continue;
        };
        let Some(text) = flat(base, &by_id, 0) else {
            notes.push(format!("{name}: no document {base}"));
            continue;
        };
        let id = format!("tf2:{name}");
        let shown = display_name(r);
        let doc = docs::item_doc(text, &id, &shown, r, def.gun_game);
        out.push(ItemWeapon {
            id,
            index: def.index,
            name: shown,
            base: base.clone(),
            classes: r.classes.clone(),
            doc: toml::to_string(&doc).expect("a document serializes"),
        });
    }
    (out, notes)
}

/// Document `id` with its bases merged in (as the weapon data merges
/// them: tables key by key, `class` and `abstract` not inherited), its
/// `base` gone: an item's document stands alone.
fn flat(id: &str, docs: &BTreeMap<String, toml::Table>, depth: usize) -> Option<toml::Table> {
    fn merge(a: &mut toml::Table, b: &toml::Table) {
        for (k, v) in b {
            match (a.get_mut(k), v) {
                (Some(toml::Value::Table(x)), toml::Value::Table(y)) => merge(x, y),
                _ => {
                    a.insert(k.clone(), v.clone());
                }
            }
        }
    }
    let own = docs.get(id)?;
    let mut t = match own.get("base").and_then(toml::Value::as_str) {
        Some(b) if depth < 8 => {
            let mut t = flat(b, docs, depth + 1)?;
            t.remove("abstract");
            t.remove("class");
            t
        }
        _ => toml::Table::new(),
    };
    merge(&mut t, own);
    t.remove("base");
    Some(t)
}

/// The pack on `mount`, if it has one (a malformed one said).
pub fn read_pack(mount: &Mount) -> Option<ItemPack> {
    let bytes = mount.read(PACK_PATH).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    ItemPack::parse(&text).map_err(|e| bevy::log::warn!("tf2: {PACK_PATH}: {e}")).ok()
}

/// The generated sets for `chimera_pack sets tf2 --only items`: the pack
/// converted from `mount` (the install, `.ctx` scripts decrypted with the
/// host's key), when `only` asks for it.
pub fn generated(mount: &Mount, only: &str) -> Vec<(String, Vec<(String, Vec<u8>)>)> {
    if !(SET.starts_with(only) || only.starts_with(SET)) {
        return Vec::new();
    }
    let schema = match mount.read(SCHEMA) {
        Ok(b) => b,
        Err(e) => {
            println!("{SET}: {SCHEMA}: {e}");
            return Vec::new();
        }
    };
    let text = String::from_utf8_lossy(&schema);
    let script = |class: &str| {
        let s = super::scripts::read(mount, class, None);
        (s.source != super::scripts::ScriptSource::Missing).then_some(s.root)
    };
    let c = convert::convert(&text, &script);
    for n in &c.notes {
        println!("{SET}: {n}");
    }
    println!("{SET}: {} weapon items, {} item class scripts read", c.pack.items.len(), c.pack.scripts.len());
    vec![(SET.to_string(), vec![(PACK_PATH.to_string(), c.pack.to_text().into_bytes())])]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_reads_and_names_batch_one() {
        let d = ItemsData::builtin();
        assert_eq!(d.items.values().filter(|i| i.batch == 1).count(), 33);
        let mut idx: Vec<u32> = d.items.values().map(|i| i.index).collect();
        idx.sort();
        idx.dedup();
        assert_eq!(idx.len(), d.items.len(), "an index once");
        let stock: Vec<String> =
            crate::game::weapons::tf2_weapons().weapons.into_iter().map(|w| w.id).collect();
        for b in d.bases.values() {
            assert!(stock.contains(b), "{b}");
        }
        assert!(PACK_PATH.starts_with(&format!("{GEN_ROOT}/v{VERSION}/")));
    }
}
