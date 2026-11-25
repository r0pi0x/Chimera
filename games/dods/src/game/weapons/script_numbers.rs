//! Where each DoD:S weapon's numbers come from (docs/plans/active/dods.md,
//! M1; specs/dods/weapons.md, "The scripts' fields"): the game's own
//! weapon script (`scripts/weapon_<name>.txt`, plain KeyValues, read
//! through DoD:S's mount: the install or the asset store) applied over the
//! committed document, whose numbers are the spec's (the fallback when no
//! script is served).
//!
//! - `apply_script` is weapons.md's mapping of one script onto one gun's
//!   document: `Damage` (`[damage] amount`), `Accuracy` (the cone and `dods:gun`'s
//!   `accuracy`), `SecondaryAccuracy`, `AccuracyMovePenalty` and `Recoil`
//!   (`dods:gun`), `FireDelay` (`[trigger] cycle`, rounded up to whole
//!   ticks: `fire_cycle`), `clip_size` (`[magazine] clip`),
//!   `DefaultAmmoClips` (its ammo's `max`: that many spare clips), the
//!   `SoundData` block's `single_shot`, `empty`, `deploy` and `reload`
//!   entries (`[sounds]`: `fire`, `empty`, `deploy`, `world_reload`, the
//!   reload others hear; each under DoD:S's name in a map's table,
//!   `maps::own_sound`) and the view and world models. A melee script
//!   (`dods:melee`): `Damage` as the swing's, five times it from behind
//!   (damage.md, "Melee"; the backstab's `damage`), `melee_hit`,
//!   `melee_hit_world` and `melee_miss` (or `single_shot`) as its sounds.
//!   Not mapped, as measured: `BulletsPerShot` (the MG42's 2 fires one
//!   bullet a shot; every gun fires one) and a melee script's `FireDelay`
//!   (0.1; a swing blocks 0.4 s, the document's).
//!   A field missing or malformed leaves the document's number; a script
//!   whose `WeaponType` doesn't fit the document (a melee script on a gun)
//!   is refused; a melee document is one with a primary swing
//!   (`[melee.primary]`), so a gun with a strike on its alt fire (the
//!   Thompson's and MP40's `[melee.secondary]`) reads its gun script.
//! - `GunNumbers` is the documents DoD:S's weapons register from: the
//!   committed ones (`builtin`: tests, the harness; their fire delays
//!   rounded too) or the scripts' (`from_scripts`: the apps, through
//!   `INFO.mount()`), with each weapon's source, logged once per app (an
//!   info line from a script, a warning for the fallback). The documents
//!   are also the set's server documents (`DodsGunsPlugin`): a client
//!   without the scripts plays with the server's numbers.
//!
//! Known shortcuts:
//! - A client with no scripts at its start reads them again only by
//!   taking a server's documents at the join.

use chimera_core::mount::keyvalues::{self, Escapes, Kv};
use toml::{Table, Value};

use crate::mount::Mount;

use super::super::maps::own_sound;

/// DoD:S's tick, seconds (weapons.md, "Units and conventions").
const TICK: f64 = super::super::TICK_INTERVAL;

/// A fire delay as the game times it: rounded up to whole ticks
/// (weapons.md, "Units and conventions": 0.085 s is 6 ticks, 0.09 s; 0.1 s
/// is 7, 0.105 s). A delay a hair over a whole number of ticks (its
/// decimal written in binary) is that number.
pub fn fire_cycle(delay: f32) -> f32 {
    let ticks = (delay as f64 / TICK - 1e-4).ceil().max(1.0);
    (ticks * TICK) as f32
}

/// What applying one script did to a weapon's document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Applied {
    /// The targets the script set (`[damage] amount`, ...).
    pub set: Vec<&'static str>,
    /// Malformed fields.
    pub notes: Vec<String>,
    /// Why nothing was applied (the script doesn't fit the weapon).
    pub refused: Option<String>,
}

/// The script's `WeaponData` block in `text` (None: none).
pub fn weapon_data(text: &str) -> Option<Kv> {
    keyvalues::parse(text, Escapes::Off).get("WeaponData").cloned()
}

fn table<'a>(doc: &'a mut Table, key: &str) -> &'a mut Table {
    let v = doc.entry(key.to_string()).or_insert_with(|| Value::Table(Table::new()));
    if !v.is_table() {
        *v = Value::Table(Table::new());
    }
    v.as_table_mut().expect("a table")
}

fn hook<'a>(doc: &'a mut Table, name: &str) -> &'a mut Table {
    table(table(doc, "hooks"), name)
}

/// `data` (a script's `WeaponData`) applied to the weapon document `doc`.
pub fn apply_script(data: &Kv, doc: &mut Table) -> Applied {
    let mut out = Applied::default();
    // A melee weapon swings on the attack; a gun's alt-fire strike
    // (`[melee.secondary]` alone) keeps it a gun.
    let melee = doc.get("melee").and_then(|m| m.get("primary")).is_some_and(Value::is_table);
    let kind = data.str("WeaponType").map(|t| t.trim().to_ascii_lowercase());
    if let Some(k) = &kind
        && (k == "melee") != melee
    {
        out.refused = Some(format!("a {k} script on a {} document", if melee { "melee" } else { "gun's" }));
        return out;
    }
    let num = |key: &str, notes: &mut Vec<String>| -> Option<f64> {
        let v = data.str(key)?;
        match v.trim().parse::<f64>() {
            Ok(n) if n.is_finite() => Some(n),
            _ => {
                notes.push(format!("{key} {v:?} is not a number"));
                None
            }
        }
    };
    if melee {
        if let Some(d) = num("Damage", &mut out.notes) {
            let swing = table(table(doc, "melee"), "primary");
            swing.insert("damage".into(), Value::Float(d));
            // damage.md, "Melee": from behind five times as much (the
            // document's rule of what is behind).
            if let Some(b) = swing.get_mut("backstab").and_then(Value::as_table_mut) {
                b.insert("damage".into(), Value::Float(d * 5.0));
            }
            out.set.push("[melee.primary] damage, backstab");
        }
        let sounds = data.get("SoundData");
        let sound = |k: &str| sounds.and_then(|s| s.str(k)).map(str::trim).filter(|s| !s.is_empty()).map(own_sound);
        let swing = table(table(doc, "melee"), "primary");
        for (from, to) in [("melee_hit", "sound_hit"), ("melee_hit_world", "sound_hit_world")] {
            if let Some(s) = sound(from) {
                swing.insert(to.into(), Value::String(s));
            }
        }
        if let Some(s) = sound("melee_miss").or_else(|| sound("single_shot")) {
            swing.insert("sound_miss".into(), Value::String(s));
            out.set.push("[melee.primary] sounds");
        }
    } else if explosive(doc) {
        // An explosive's blast isn't its script's `Damage` (damage.md,
        // "Explosions") and its cycle is measured: its clip, spare clips,
        // sounds and models only.
        apply_clips(data, doc, &mut out, &num);
        apply_sounds(data, doc, &mut out);
    } else {
        if let Some(d) = num("Damage", &mut out.notes) {
            table(doc, "damage").insert("amount".into(), Value::Float(d));
            out.set.push("[damage] amount");
        }
        if let Some(a) = num("Accuracy", &mut out.notes) {
            let mut cone = Table::new();
            cone.insert("template_disc".into(), Value::Float(a));
            table(doc, "hitscan").insert("spread".into(), Value::Table(cone));
            hook(doc, super::GUN_HOOK).insert("accuracy".into(), Value::Float(a));
            out.set.push("accuracy");
        }
        for (key, to) in [("SecondaryAccuracy", "alt_accuracy"), ("AccuracyMovePenalty", "move_penalty"), ("Recoil", "recoil")] {
            // A gun with no sights has no alt accuracy to set.
            if to == "alt_accuracy" && !doc.contains_key("alt") {
                continue;
            }
            if let Some(v) = num(key, &mut out.notes) {
                hook(doc, super::GUN_HOOK).insert(to.into(), Value::Float(v));
                out.set.push(to_label(to));
            }
        }
        if let Some(d) = num("FireDelay", &mut out.notes).filter(|d| *d > 0.0) {
            table(doc, "trigger").insert("cycle".into(), Value::Float(d));
            out.set.push("[trigger] cycle");
        }
        apply_clips(data, doc, &mut out, &num);
        apply_sounds(data, doc, &mut out);
    }
    let model = |k: &str| data.str(k).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    if let Some(v) = model("viewmodel") {
        let m = table(doc, "models");
        m.insert("view".into(), Value::String(v));
        m.insert("view_right_handed".into(), Value::Boolean(true));
        out.set.push("[models] view");
    }
    if let Some(w) = model("playermodel") {
        table(doc, "models").insert("world".into(), Value::String(w));
        out.set.push("[models] world");
    }
    out
}

/// Whether `doc` is an explosive's (`explosives`' hooks).
fn explosive(doc: &Table) -> bool {
    doc.get("hooks").is_some_and(|h| {
        [super::explosives::GRENADE_HOOK, super::explosives::RIFLE_GRENADE_HOOK, super::explosives::ROCKET_HOOK].iter().any(|k| h.get(*k).is_some())
    })
}

/// `clip_size` (`[magazine] clip`) and `DefaultAmmoClips` (its ammo's
/// `max`: that many spare clips).
fn apply_clips(data: &Kv, doc: &mut Table, out: &mut Applied, num: &dyn Fn(&str, &mut Vec<String>) -> Option<f64>) {
    let clip = num("clip_size", &mut out.notes).filter(|c| *c >= 1.0).map(|c| c as i64);
    if let Some(c) = clip
        && doc.contains_key("magazine")
    {
        table(doc, "magazine").insert("clip".into(), Value::Integer(c));
        out.set.push("[magazine] clip");
    }
    let clip = clip.or_else(|| doc.get("magazine")?.get("clip")?.as_integer());
    let ammo = doc.get("magazine").and_then(|m| m.get("ammo")).and_then(Value::as_str).map(str::to_string);
    if let (Some(spare), Some(clip), Some(ammo)) = (num("DefaultAmmoClips", &mut out.notes), clip, ammo)
        && spare >= 0.0
    {
        table(table(doc, "ammo"), &ammo).insert("max".into(), Value::Integer(spare as i64 * clip));
        out.set.push("ammo max");
    }
    let _ = data;
}

/// The `SoundData` block's shot, empty, deploy and reload entries.
fn apply_sounds(data: &Kv, doc: &mut Table, out: &mut Applied) {
    let sounds = data.get("SoundData");
    let sound = |k: &str| sounds.and_then(|s| s.str(k)).map(str::trim).filter(|s| !s.is_empty()).map(own_sound);
    let mut any = false;
    for (from, to) in [("single_shot", "fire"), ("empty", "empty"), ("deploy", "deploy"), ("reload", "world_reload")] {
        if let Some(s) = sound(from) {
            table(doc, "sounds").insert(to.into(), Value::String(s));
            any = true;
        }
    }
    if any {
        out.set.push("[sounds]");
    }
}

fn to_label(key: &str) -> &'static str {
    match key {
        "alt_accuracy" => "alt accuracy",
        "move_penalty" => "move penalty",
        _ => "recoil",
    }
}

/// The fire delay of a document rounded up to ticks (`fire_cycle`): a
/// gun's cycle, a melee swing's refires.
fn round_cycle(doc: &mut Table) {
    let round = |t: &mut Table, k: &str| {
        if let Some(c) = t.get(k).and_then(|c| c.as_float().or_else(|| c.as_integer().map(|i| i as f64))) {
            t.insert(k.into(), Value::Float(fire_cycle(c as f32) as f64));
        }
    };
    if let Some(t) = doc.get_mut("trigger").and_then(Value::as_table_mut) {
        round(t, "cycle");
    }
    if let Some(t) = doc.get_mut("melee").and_then(|m| m.get_mut("primary")).and_then(Value::as_table_mut) {
        for k in ["hit_refire", "miss_refire", "miss_refire_other"] {
            round(t, k);
        }
    }
}

/// The script a document names (`dods:gun`'s or `dods:melee`'s `script`).
pub fn script_of(doc: &Table) -> Option<String> {
    let hooks = doc.get("hooks")?;
    let h = [super::GUN_HOOK, super::MELEE_HOOK, super::explosives::GRENADE_HOOK, super::explosives::RIFLE_GRENADE_HOOK, super::explosives::ROCKET_HOOK]
        .iter()
        .find_map(|k| hooks.get(*k))?;
    h.get("script")?.as_str().map(str::to_string)
}

/// One weapon's numbers' source, as the log says it.
#[derive(Clone, Debug, PartialEq)]
pub struct GunSource {
    pub gun: String,
    /// Whether the script set any of its numbers.
    pub from_script: bool,
    pub source: String,
}

/// The documents DoD:S's weapons register from, and each one's source (a
/// resource: one an app has before `DodsGunsPlugin` is the one used).
#[derive(bevy::prelude::Resource, Clone, Debug, PartialEq)]
pub struct GunNumbers {
    pub docs: Vec<(String, String)>,
    pub sources: Vec<GunSource>,
}

impl GunNumbers {
    /// The committed documents (the spec's numbers), fire delays rounded.
    pub fn builtin() -> Self {
        Self::with_scripts(|_| None)
    }

    /// The documents with each weapon's script read through DoD:S's mount
    /// (`INFO.mount()`: the install and/or the asset store).
    pub fn from_scripts() -> Self {
        let mount = super::super::INFO.mount();
        Self::from_mount(mount.as_ref())
    }

    /// The documents with each weapon's script read through `mount` (None:
    /// every one's the spec fallback).
    pub fn from_mount(mount: Option<&Mount>) -> Self {
        Self::with_scripts(|name| {
            let m = mount?;
            m.read(&format!("scripts/{name}.txt")).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
        })
    }

    /// The documents with each weapon's script text from `script` (by
    /// name; None: not served).
    pub fn with_scripts(script: impl Fn(&str) -> Option<String>) -> Self {
        let mut docs = Vec::new();
        let mut sources = Vec::new();
        for (name, text) in super::DATA {
            let Ok(mut doc) = text.parse::<Table>() else {
                docs.push((name.to_string(), text.to_string()));
                continue;
            };
            let id = doc.get("id").and_then(Value::as_str).unwrap_or(name).to_string();
            if let Some(s) = script_of(&doc) {
                let source = match script(&s).map(|t| (weapon_data(&t), t)) {
                    None => GunSource { gun: id, from_script: false, source: format!("spec fallback (no script {s})") },
                    Some((None, _)) => GunSource { gun: id, from_script: false, source: format!("spec fallback ({s} has no WeaponData)") },
                    Some((Some(data), _)) => {
                        let mut applied = doc.clone();
                        let a = apply_script(&data, &mut applied);
                        let notes = if a.notes.is_empty() { String::new() } else { format!("; notes: {}", a.notes.join("; ")) };
                        match a.refused {
                            Some(why) => GunSource { gun: id, from_script: false, source: format!("spec fallback ({s}: {why})") },
                            None if a.set.is_empty() => {
                                GunSource { gun: id, from_script: false, source: format!("spec fallback ({s} set nothing{notes})") }
                            }
                            None => {
                                doc = applied;
                                GunSource { gun: id, from_script: true, source: format!("script {s} (set {}{notes})", a.set.join(", ")) }
                            }
                        }
                    }
                };
                sources.push(source);
            }
            round_cycle(&mut doc);
            docs.push((name.to_string(), toml::to_string(&doc).unwrap_or_else(|_| text.to_string())));
        }
        Self { docs, sources }
    }

    /// The documents parsed as weapon data (as `DodsGunsPlugin` loads them).
    pub fn parse(&self) -> Result<chimera_sim::weapon::data::WeaponSet, String> {
        let docs: Vec<(&str, &str)> = self.docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
        chimera_sim::weapon::data::WeaponSet::parse(&docs, &[])
    }

    /// Whether every weapon's numbers are the committed ones.
    pub fn is_builtin(&self) -> bool {
        self.sources.iter().all(|s| !s.from_script)
    }

    /// Each weapon's source, once: info from a script, a warning for the
    /// fallback.
    pub fn log(&self) {
        for s in &self.sources {
            if s.from_script {
                bevy::log::info!("dods: {}'s numbers: {}", s.gun, s.source);
            } else {
                bevy::log::warn!("dods: {}'s numbers: {}", s.gun, s.source);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_delays_round_up_to_whole_ticks() {
        // weapons.md, "Units and conventions" (measured).
        assert!((fire_cycle(0.085) - 0.09).abs() < 1e-6);
        assert!((fire_cycle(0.1) - 0.105).abs() < 1e-6);
        assert!((fire_cycle(0.09) - 0.09).abs() < 1e-6, "six ticks stay six");
        assert!((fire_cycle(0.05) - 0.06).abs() < 1e-6);
        assert!((fire_cycle(1.85) - 1.86).abs() < 1e-6);
    }

    #[test]
    fn a_script_sets_the_mapped_fields() {
        let mut doc: Table = "[trigger]\ncycle = 0.5\n[magazine]\nclip = 1\nammo = \"dods:x\"\n[damage]\namount = 1.0\n[alt]\nkind = \"scope\"\n[hooks.\"dods:gun\"]\nscript = \"weapon_x\"\naccuracy = 0.5\n"
            .parse()
            .unwrap();
        let text = "WeaponData { \"WeaponType\" \"Rifle\" \"Damage\" \"80\" \"Accuracy\" \"0.014\" \"SecondaryAccuracy\" \"0.0\" \"FireDelay\" \"0.37\" \"clip_size\" \"8\" \"DefaultAmmoClips\" \"11\" \"BulletsPerShot\" \"two\" \"viewmodel\" \"models/v.mdl\" SoundData { \"single_shot\" \"Weapon_X.Shoot\" } }";
        let a = apply_script(&weapon_data(text).unwrap(), &mut doc);
        assert!(a.refused.is_none());
        assert_eq!(doc["damage"]["amount"].as_float(), Some(80.0));
        assert_eq!(doc["magazine"]["clip"].as_integer(), Some(8));
        assert_eq!(doc["ammo"]["dods:x"]["max"].as_integer(), Some(88));
        assert_eq!(doc["hooks"]["dods:gun"]["alt_accuracy"].as_float(), Some(0.0));
        assert_eq!(doc["sounds"]["fire"].as_str(), Some("dods:Weapon_X.Shoot"));
        assert_eq!(doc["models"]["view"].as_str(), Some("models/v.mdl"));
        assert!(a.notes.is_empty(), "BulletsPerShot isn't read: {:?}", a.notes);
    }

    #[test]
    fn a_melee_script_on_a_gun_is_refused() {
        let mut doc: Table = "[damage]\namount = 1.0\n".parse().unwrap();
        let a = apply_script(&weapon_data("WeaponData { \"WeaponType\" \"Melee\" \"Damage\" \"60\" }").unwrap(), &mut doc);
        assert!(a.refused.is_some());
        assert_eq!(doc["damage"]["amount"].as_float(), Some(1.0));
    }

    #[test]
    fn a_gun_with_an_alt_fire_strike_reads_its_gun_script() {
        // The Thompson's and MP40's stock strike: `[melee.secondary]`, no primary.
        let mut doc: Table = "[damage]\namount = 1.0\n[melee.secondary]\ndamage = 60.0\n".parse().unwrap();
        let text = "WeaponData { \"WeaponType\" \"SubMG\" \"Damage\" \"40\" \"viewmodel\" \"models/v.mdl\" }";
        let a = apply_script(&weapon_data(text).unwrap(), &mut doc);
        assert!(a.refused.is_none(), "{:?}", a.refused);
        assert_eq!(doc["damage"]["amount"].as_float(), Some(40.0));
        assert_eq!(doc["models"]["view"].as_str(), Some("models/v.mdl"));
        assert_eq!(doc["melee"]["secondary"]["damage"].as_float(), Some(60.0), "the strike untouched");
    }

    #[test]
    fn a_missing_script_is_the_spec_fallback() {
        let n = GunNumbers::builtin();
        assert!(n.is_builtin());
        assert!(n.sources.iter().all(|s| s.source.starts_with("spec fallback")));
        n.parse().expect("the committed documents parse");
    }
}
