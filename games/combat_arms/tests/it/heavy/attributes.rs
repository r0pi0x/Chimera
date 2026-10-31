//! The attribute data of the user's Combat Arms install
//! (specs/combat_arms/attribute_data.md, "Where the data sits"), read
//! through our own `.rez` reader. Passes with a note without an install;
//! without a V1 key in chimera.local.toml only the plaintext directory is
//! checked and the loader must fall back to the built-in data. Nothing
//! read here is written anywhere.

use std::collections::BTreeSet;

use chimera_core::mount::config::LocalConfig;
use chimera_sim::weapon::data::WeaponSet;
use combat_arms::game::data::{WEAPON_DATA, attributes::*, merge_weapon_docs, open_attributes, read_attributes};
use combat_arms::game::{GAME, mount};
use lithtech_formats::attributes::Document;
use lithtech_formats::rez::RezArchive;
use chimera_core::mount::FileSource;

fn attributes_rez() -> Option<std::path::PathBuf> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    let path = install.join("Game").join("ATTRIBUTES.rez");
    if !path.is_file() {
        eprintln!("skipping: no {}", path.display());
        return None;
    }
    Some(path)
}

/// Every key the spec's field tables name for weapons and ammo (lower
/// case, list keys without their index), for open question 1.
fn known(key: &str) -> bool {
    const KNOWN: &[&str] = &[
        "name", "guntype", "clientweapontype", "anitype", "loweranitype", "shotsperclip", "ammoname",
        "infiniteammo", "reloadperbullet", "vectorsperround", "range", "effectrange", "alt_effectrange",
        "damagerfacter", "firedelay", "fireanirate", "rapidfirecount", "continuousfire", "hudfiremode",
        "tripleshotmode", "tripleshotroundnum", "tripleshotdelay", "tripleshotselectdelay", "tripleshotminperturb",
        "tripleshotmaxperturb", "tripleshotperturbincspeed", "tripleshotperturbdecspeed",
        "tripleshotmaxcamrecoilpitch", "tripleshotbasecamrecoilpitch", "minperturb", "maxperturb", "duckperturb",
        "moveduckperturb", "zoomedminperturb", "zoomedmaxperturb", "perturbincspeed", "perturbdecspeed",
        "zoomedperturbincspeed", "zoomedperturbdecspeed", "dynamicperturbfactor", "dynnamicperturbfactorrun",
        "dynnamicperturbfactorwalk", "dynnamicperturbfactorjump", "zoomtime", "movepenalty", "zoomedmovepenalty",
        "maxcamrecoilpitch", "basecamrecoilpitch", "basecamrecoilaccelpitch", "maxcamrecoilyaw", "basecamrecoilyaw",
        "maxcamrecoilwag", "basecamrecoilwagl", "basecamrecoilwagr", "maxrandompitch", "maxrandomwag",
        "basecambackjitter", "backjitterrecover", "cameraswayxspeed", "cameraswayyspeed", "cameraswayxfreq",
        "cameraswayyfreq", "disablecrosshair", "crosshairtype", "noselect", "grenadespeed", "grenadeattach",
        "widefiremaxnum", "widefireangledegree", "firesndradius", "reloadsndradius", "selectsndradius",
        "silencedfireradius", "dropbyplayerdead", "damageweapon", "checkreloadanitime", "hidden", "pos", "muzzlepos",
        "breachoffset", "hhmodelscale", "recoil", "muzzlepvposoff", "muzzlepvrotoff", "muzzlehhposoff",
        "muzzlehhrotoff", "scopepvposoff", "scopepvrotoff", "scopehhposoff", "scopehhrotoff", "pvskin",
        "pvrenderstyle", "hhmodel", "hhskin", "hhrenderstyle", "firesnd", "alt_firesnd", "dryfiresnd", "selectsnd",
        "deselectsnd", "reloadsnd", "silencedfiresnd", "pvmuzzlefxname", "hhmuzzlefxname", "hudactiveimage",
        "hudnoneactiveimage", "hudweaponnameimage", "weaponkillimage", "dotsightname",
    ];
    let base = key.trim_end_matches(|c: char| c.is_ascii_digit());
    KNOWN.contains(&base) || key.starts_with("pvmodel")
}

#[test]
fn install_attribute_data() {
    let Some(path) = attributes_rez() else { return };
    let keys = mount::keys().unwrap();
    let archive = RezArchive::open(&path, &keys).unwrap();
    // The plaintext directory: one ATTRIBUTES folder with the files by role.
    let entries = archive.entries();
    let size = |p: &str| entries.iter().find(|e| e.path == p).map(|e| e.size);
    // Steam build 26236938 (9,007,972 bytes in 25595979).
    assert_eq!(size(WEAPONS_FILE), Some(9_275_149));
    assert_eq!(size(SERVER_FILE), Some(2_176));
    assert!(size(MODELS_FILE).is_some());
    assert!(entries.iter().all(|e| e.path.starts_with("attributes/")), "a file outside ATTRIBUTES/");
    eprintln!("ATTRIBUTES.rez: {} files", entries.len());

    if archive.locked() {
        // No V1 key: the loader keeps the built-in data.
        assert_eq!(archive.read(WEAPONS_FILE).unwrap().unwrap_err().kind(), std::io::ErrorKind::PermissionDenied);
        let e = open_attributes(&path).unwrap_err();
        assert!(e.contains("encrypted"), "{e}");
        eprintln!("no V1 key: only the directory was checked");
        return;
    }

    let (attrs, problems) = read_attributes(|name| archive.read(name).unwrap());
    assert!(problems.is_empty(), "{problems:?}");
    let w = attrs.weapons.as_ref().unwrap();
    eprintln!("weapons file: {} weapons, {} ammo, {} gear", w.weapons.len(), w.ammo.len(), w.gear.len());
    let m4 = w.weapon("M4A1").expect("an M4A1 record");
    assert_eq!(m4.gun_type, 4);
    assert!(w.default_ammo(m4).is_some());
    let server = attrs.server.as_ref().unwrap();
    eprintln!("[Player]: {server:?}");
    let models = attrs.models.as_ref().unwrap();
    assert!(models.figure_types.len() >= 2);

    // Open question 1: keys of weapon records that no field table names.
    let doc = Document::parse(&archive.read(WEAPONS_FILE).unwrap().unwrap()).unwrap();
    let unknown: BTreeSet<_> = doc
        .records("Weapon")
        .iter()
        .flat_map(|s| s.iter().map(|(k, _)| k.to_string()))
        .filter(|k| !known(k))
        .collect();
    eprintln!("weapon keys in no field table ({}): {unknown:?}", unknown.len());

    // The merge loads with our documents.
    let docs = merge_weapon_docs(WEAPON_DATA, w, crate::attribute_data::meters_per_unit()).unwrap();
    let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let set = WeaponSet::parse(&refs, &[]).unwrap();
    eprintln!("merged: {} weapons", set.weapons.len());

    // Lucker Party's gun game (playtest 2026-10-24: the Remington 870 in
    // the secondary class). The merge takes each weapon's class from the
    // install's order lists (inventory.md), and the install lists the
    // Remington 870 (record 37) in the secondary class: slot 2 is the
    // original's, not a bug. The SMG, rifle and machine gun are primaries,
    // the Beretta a secondary. Every record of each name and its class is
    // printed; the merged slot must be the install's class.
    for (name, id, slot) in [
        ("Remington_870", "combat_arms:remington_870", 1),
        ("MP5A4", "combat_arms:mp5a4", 0),
        ("M4A1", "combat_arms:m4a1", 0),
        ("M60", "combat_arms:m60", 0),
        ("Beretta_M92FS", "combat_arms:beretta_m92fs", 1),
    ] {
        let records: Vec<(usize, &str, u8)> = w
            .weapons
            .iter()
            .enumerate()
            .filter(|(_, r)| combat_arms::game::data::slug(&r.name) == combat_arms::game::data::slug(name))
            .map(|(i, r)| (i, r.name.as_str(), w.inventory_class(i)))
            .collect();
        eprintln!("{name}: records (index, name, class) {records:?}");
        let merged = set.weapons.iter().find(|x| x.id == id).unwrap_or_else(|| panic!("{id} merged"));
        assert_eq!(merged.slot, slot, "{id}: class {} expected (records {records:?})", slot + 1);
    }
}

/// Every number under `path` in `a` against the same place in `b`:
/// `(path, ours, theirs)` where they differ by more than a factor of
/// two (or one is zero and the other isn't).
fn far_apart(path: &str, a: &toml::Value, b: Option<&toml::Value>, out: &mut Vec<(String, f64, f64)>) {
    use toml::Value;
    let num = |v: &Value| v.as_float().or_else(|| v.as_integer().map(|i| i as f64));
    match (a, b) {
        (Value::Table(t), Some(Value::Table(u))) => {
            for (k, v) in t {
                far_apart(&format!("{path}.{k}"), v, u.get(k), out);
            }
        }
        (Value::Array(t), Some(Value::Array(u))) => {
            for (i, v) in t.iter().enumerate() {
                far_apart(&format!("{path}[{i}]"), v, u.get(i), out);
            }
        }
        (x, Some(y)) => {
            if let (Some(p), Some(q)) = (num(x), num(y)) {
                let far = if p == 0.0 || q == 0.0 {
                    p != q
                } else {
                    (p / q).abs().max((q / p).abs()) > 2.0 || p.signum() != q.signum()
                };
                if far {
                    out.push((path.to_string(), p, q));
                }
            }
        }
        _ => {}
    }
}

/// The install's data as the game merges it (weapons with the
/// modification file, the body type, the attachment records) loads, and
/// the numbers that differ from our spec defaults by more than a factor
/// of two are listed (a sanity check: printed, not asserted).
#[test]
fn merged_numbers_against_ours() {
    use combat_arms::game::{
        attachments::CaModTable,
        data::{merge_mods, merge_movement, merge_weapon_docs_with_mods},
        movement::{MOVEMENT_DATA, MovementData},
    };
    // The archive, or its files through the asset store standing in for
    // the install (decrypted there).
    let attrs = match attributes_rez() {
        Some(path) => open_attributes(&path),
        None => match combat_arms::game::INFO.mount() {
            Some(m) if combat_arms::game::INFO.served_alone() => combat_arms::game::data::attributes_from_mount(&m),
            _ => return,
        },
    };
    let Ok(attrs) = attrs else {
        eprintln!("skipping: no V1 key");
        return;
    };
    let w = attrs.weapons.as_ref().unwrap();
    let mpu = crate::attribute_data::meters_per_unit();
    let docs = merge_weapon_docs_with_mods(WEAPON_DATA, w, attrs.mods.as_ref(), mpu).unwrap();
    let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let set = WeaponSet::parse(&refs, &[]).unwrap();
    let new = docs.len() - WEAPON_DATA.len();
    eprintln!("merged: {} weapons ({new} from the install)", set.weapons.len());
    assert!(new > 0);
    // Each of ours has its install record.
    let ca = combat_arms::game::data::CaData::install();
    for id in ca.builtin_weapon_ids() {
        let r = combat_arms::game::data::install_weapon_in(w, id);
        eprintln!(
            "{id}: install record {:?}, AniType {:?} (ours {}), HHModel {:?}",
            r.map(|r| &r.name),
            r.map(|r| (r.ani_type, r.lower_ani_type)),
            ca.characters().weapon(Some(id)).ani_type,
            r.map(|r| &r.hh_model)
        );
        assert!(r.is_some(), "{id}: no install record");
    }
    let mut differ = Vec::new();
    for (name, text) in WEAPON_DATA {
        let ours: toml::Value = text.parse::<toml::Table>().unwrap().into();
        let (_, merged) = docs.iter().find(|(n, _)| n == name).unwrap();
        let theirs: toml::Value = merged.parse::<toml::Table>().unwrap().into();
        far_apart(name, &ours, Some(&theirs), &mut differ);
    }
    for (p, a, b) in &differ {
        eprintln!("weapon {p}: ours {a}, install {b}");
    }
    // Every merged weapon: a hit-scan range that is positive.
    for (name, text) in &docs {
        let doc: toml::Table = text.parse().unwrap();
        if let Some(r) = doc.get("hitscan").and_then(|h| h.get("range")).and_then(|r| r.as_float()) {
            assert!(r > 0.0 && r.is_finite(), "{name}: range {r}");
        }
    }

    // Movement: the body type over ours.
    let ours: MovementData = toml::from_str(MOVEMENT_DATA).unwrap();
    let mut merged = ours.clone();
    merge_movement(&mut merged, attrs.models.as_ref().unwrap());
    let (a, b) = (&ours.character.figure, &merged.character.figure);
    for (k, x, y) in [
        ("dash_speed", a.dash_speed, b.dash_speed),
        ("dash_gauge", a.dash_gauge, b.dash_gauge),
        ("gauge_recovery", a.gauge_recovery, b.gauge_recovery),
        ("max_stamina", a.max_stamina, b.max_stamina),
        ("variable_speed", a.variable_speed, b.variable_speed),
        ("variable_weight", a.variable_weight, b.variable_weight),
        ("variable_jump", a.variable_jump, b.variable_jump),
        ("stiff_duration", a.stiff_duration, b.stiff_duration),
    ] {
        eprintln!("movement figure.{k}: ours {x}, install {y}");
        assert!(y.is_finite(), "{k}");
    }

    // Attachments: the modification file's records over ours by name.
    let built_in = CaModTable::built_in();
    let mut table = built_in.clone();
    merge_mods(&mut table, attrs.mods.as_ref().unwrap());
    eprintln!("attachments: {} ours, {} merged", built_in.records.len(), table.records.len());
    let mut mod_differ = Vec::new();
    for r in &built_in.records {
        let Some(m) = table.records.iter().find(|m| m.name == r.name) else { continue };
        let (x, y) = (toml::Value::try_from(r).unwrap(), toml::Value::try_from(m).unwrap());
        far_apart(&r.name, &x, Some(&y), &mut mod_differ);
    }
    for (p, a, b) in &mod_differ {
        eprintln!("attachment {p}: ours {a}, install {b}");
    }
}
