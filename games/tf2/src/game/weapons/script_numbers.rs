//! Where each TF2 gun's numbers come from (docs/plans/active/tf2.md,
//! section 3, decision 3; specs/tf2/weapon_scripts.md): the game's weapon
//! script when one is served (`scripts::read`: the asset store's
//! decrypted record, or the install decrypted with the user's key), else
//! the spec's public numbers that data/weapons/*.toml commit (the
//! fallback).
//!
//! - `apply_script` is the spec's mapping (weapon_scripts.md, "Mapping",
//!   and its corrections from the SDK): a script's first top-level block
//!   onto one gun's TOML document. The gun's kind decides what `Damage` is
//!   (an integer, a fraction cut toward zero: per pellet, per blast, per
//!   needle; the Sniper Rifle's and the Flame Thrower's are ignored, as
//!   the game does: the flame's 13 is fixed); `TimeFireDelay` the
//!   interval under the TOMLs' `set` timing (the flame's ignored);
//!   `TimeReload` the round's insert of the Grenade Launcher only (the
//!   other shell guns time it by their animation); `Range` the bullets'
//!   trace (`[hitscan] range`); `PunchAngle` the view kick (`tf2:gun`
//!   `punch`, `recoil`); the kind check (`ProjectileType`, `WeaponType`)
//!   refuses a script that doesn't fit the gun; each field is applied on
//!   its own, a missing or malformed one leaving the TOML's number. The
//!   melee weapons' (weapons_stock.md 6, `apply_melee`): `Damage`,
//!   `TimeFireDelay` as the hit and miss refires, `SmackDelay` as the
//!   swing's `delay`, 0.2 s without one; a melee script on a gun or a
//!   gun's on a melee weapon refused.
//! - `GunNumbers` is the documents TF2's guns are registered from: the
//!   built-in ones (`builtin`: tests, the harness) or the scripts'
//!   (`from_scripts`: the apps, through `INFO.mount()`, the store or the
//!   install with the key), with each gun's source; `log` says it once
//!   per gun per app (an info line from a script, a warning for the spec
//!   fallback). The documents are also the set's server documents
//!   (`Tf2GunsPlugin`, `server_docs`): a client whose own differ (no
//!   scripts yet, an older store) plays with the server's numbers.
//! - `note_sources`, at each load of TF2's guns (a map's kit, another
//!   game's map dressed with them), logs again what the scripts read
//!   through that load's mount would give.
//!
//! Known shortcuts:
//! - A client with no scripts at its start reads them again only by
//!   taking a server's documents at the join (no `AssetsArrived`
//!   re-read: the crate doesn't use the network layer).

use source_formats::keyvalues::Kv;
use toml::{Table, Value};

use super::super::scripts::{self, CtxKey, KvView, Script, ScriptSource};
use crate::mount::Mount;

/// What applying one script did to a gun's document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Applied {
    /// The targets the script set (`[damage] amount`, `tf2:rocket damage`...).
    pub set: Vec<&'static str>,
    /// Malformed fields, cross-check differences, ignored fields worth a word.
    pub notes: Vec<String>,
    /// Why nothing was applied (the kind check failed).
    pub refused: Option<String>,
}

/// The gun's kind as its document says (which hook delivers its harm).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Bullet,
    Sniper,
    Rocket,
    Grenade,
    Sticky,
    Needle,
    Flame,
    /// A melee weapon (its document has `[melee]`; weapons_stock.md 6).
    Melee,
}

impl Kind {
    fn of(doc: &Table) -> Kind {
        let has = |h: &str| doc.get("hooks").and_then(Value::as_table).is_some_and(|t| t.contains_key(h));
        if doc.get("melee").is_some_and(Value::is_table) {
            Kind::Melee
        } else if has(super::SNIPER_HOOK) {
            Kind::Sniper
        } else if has(super::ROCKET_HOOK) {
            Kind::Rocket
        } else if has(super::GRENADE_HOOK) {
            Kind::Grenade
        } else if has(super::STICKY_HOOK) {
            Kind::Sticky
        } else if has(super::NEEDLE_HOOK) {
            Kind::Needle
        } else if has(super::FLAME_HOOK) {
            Kind::Flame
        } else {
            Kind::Bullet
        }
    }

    /// Whether `ProjectileType` fits (None: it says nothing we know). An
    /// absent one fits (only the flame's is absent in the stock scripts;
    /// a script with fewer fields is still read), a present one on the
    /// flame doesn't.
    fn projectile_matches(self, projectile: Option<&str>) -> Option<bool> {
        let Some(p) = projectile else { return Some(true) };
        if self == Kind::Flame {
            return Some(false);
        }
        let p = p.to_ascii_lowercase();
        let said = if p.contains("remote") {
            Kind::Sticky
        } else if p.contains("pipe") {
            Kind::Grenade
        } else if p.contains("rocket") {
            Kind::Rocket
        } else if p.contains("syringe") {
            Kind::Needle
        } else if p.contains("bullet") {
            Kind::Bullet
        } else {
            return None;
        };
        Some(said == self || (said == Kind::Bullet && self == Kind::Sniper))
    }

    /// The hook whose `damage` the script's `Damage` sets.
    fn damage_hook(self) -> Option<&'static str> {
        match self {
            Kind::Rocket => Some(super::ROCKET_HOOK),
            Kind::Grenade => Some(super::GRENADE_HOOK),
            Kind::Sticky => Some(super::STICKY_HOOK),
            Kind::Needle => Some(super::NEEDLE_HOOK),
            Kind::Flame => Some(super::FLAME_HOOK),
            Kind::Bullet | Kind::Sniper | Kind::Melee => None,
        }
    }
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

/// A real, keeping an integer an integer when the new value is whole and
/// the old one was one.
fn number(old: Option<&Value>, x: f64) -> Value {
    if matches!(old, Some(Value::Integer(_))) && x.fract() == 0.0 {
        Value::Integer(x as i64)
    } else {
        Value::Float(x)
    }
}

/// A positive real field: Some(Ok) present and well-formed, Some(Err)
/// present but malformed (noted), None absent.
fn positive(data: &KvView, key: &str, notes: &mut Vec<String>) -> Option<f64> {
    let text = data.string(key)?;
    match text.parse::<f64>() {
        Ok(x) if x > 0.0 && x.is_finite() => Some(x),
        _ => {
            notes.push(format!("{key} {text:?} is malformed: kept the data's"));
            None
        }
    }
}

/// An integer field (a fraction or a word is malformed, noted).
fn integer(data: &KvView, key: &str, notes: &mut Vec<String>) -> Option<i64> {
    let text = data.string(key)?;
    match text.parse::<i64>() {
        Ok(i) => Some(i),
        Err(_) => {
            notes.push(format!("{key} {text:?} is not a whole number: kept the data's"));
            None
        }
    }
}

/// The spec's mapping (weapon_scripts.md, "Applying a script"): `data`,
/// a script's first top-level block, onto `doc`, one gun's TOML document.
pub fn apply_script(data: KvView, doc: &mut Table) -> Applied {
    let mut out = Applied::default();
    let kind = Kind::of(doc);
    // A melee script (`MeleeWeapon` 1) fits a melee weapon only, and a
    // melee weapon only a melee script (weapons_stock.md 6).
    let melee_script = data.string("MeleeWeapon").is_some_and(|m| m.trim() == "1")
        || data.string("WeaponType").is_some_and(|t| t.eq_ignore_ascii_case("melee"));
    if melee_script != (kind == Kind::Melee) {
        out.refused = Some(if melee_script {
            "a melee script (MeleeWeapon 1) on a gun: nothing applied".into()
        } else {
            "a gun's script on a melee weapon: nothing applied".into()
        });
        return out;
    }
    if kind == Kind::Melee {
        return apply_melee(data, doc, out);
    }
    // The kind check.
    if kind.projectile_matches(data.string("ProjectileType")) == Some(false) {
        out.refused = Some(format!(
            "ProjectileType {:?} doesn't fit this gun ({kind:?}): nothing applied",
            data.string("ProjectileType").unwrap_or("absent")
        ));
        return out;
    }
    if let (Some(t), Some(slot)) = (data.string("WeaponType"), doc.get("slot").and_then(Value::as_integer)) {
        let want = match t.to_ascii_lowercase().as_str() {
            "primary" => Some(0),
            "secondary" => Some(1),
            _ => None,
        };
        if want.is_some_and(|w| w != slot) {
            out.refused = Some(format!("WeaponType {t:?} doesn't fit slot {slot}: nothing applied"));
            return out;
        }
    }
    let notes = &mut out.notes;
    let mut set = Vec::new();

    // Damage, by kind: an integer, a fraction cut toward zero.
    if let Some(d) = positive(&data, "Damage", notes).map(f64::trunc).filter(|d| *d > 0.0) {
        match kind.damage_hook() {
            // The game ignores the flame's: each flame carries 13.
            Some(_) if kind == Kind::Flame => {}
            Some(h) => {
                let hk = hook(doc, h);
                let old = hk.get("damage").cloned();
                hk.insert("damage".into(), number(old.as_ref(), d));
                set.push(match kind {
                    Kind::Rocket => "tf2:rocket damage",
                    Kind::Grenade => "tf2:grenade damage",
                    Kind::Sticky => "tf2:sticky damage",
                    _ => "tf2:needle damage",
                });
            }
            None if kind == Kind::Bullet => {
                let t = table(doc, "damage");
                let old = t.get("amount").cloned();
                t.insert("amount".into(), number(old.as_ref(), d));
                set.push("[damage] amount");
            }
            None => {} // the Sniper Rifle's isn't its damage (the game ignores it)
        }
    }
    // Pellets and spread: the bullet guns and the rifle.
    if matches!(kind, Kind::Bullet | Kind::Sniper) {
        if let Some(n) = integer(&data, "BulletsPerShot", notes) {
            if n >= 1 {
                table(doc, "hitscan").insert("pellets".into(), Value::Integer(n));
                set.push("[hitscan] pellets");
            } else {
                notes.push(format!("BulletsPerShot {n} is malformed: kept the data's"));
            }
        }
        if let Some(text) = data.string("Spread") {
            match text.parse::<f64>() {
                Ok(s) if s >= 0.0 && s.is_finite() => {
                    let mut spread = Table::new();
                    spread.insert("template".into(), Value::Float(s));
                    table(doc, "hitscan").insert("spread".into(), Value::Table(spread));
                    set.push("[hitscan] spread");
                }
                _ => notes.push(format!("Spread {text:?} is malformed: kept the data's")),
            }
        }
        // The bullets' trace.
        if let Some(r) = positive(&data, "Range", notes) {
            let t = table(doc, "hitscan");
            let old = t.get("range").cloned();
            t.insert("range".into(), number(old.as_ref(), r));
            set.push("[hitscan] range");
        }
    }
    // The interval (not the flame's: its TimeFireDelay is another rate).
    if kind != Kind::Flame
        && let Some(c) = positive(&data, "TimeFireDelay", notes)
    {
        table(doc, "trigger").insert("cycle".into(), Value::Float(c));
        set.push("[trigger] cycle");
    }
    // The clip: -1 is no clip (the data's stays).
    if let Some(c) = integer(&data, "clip_size", notes) {
        if c >= 1 {
            table(doc, "magazine").insert("clip".into(), Value::Integer(c));
            set.push("[magazine] clip");
            if let Some(Ok(d)) = data.string("default_clip").map(|t| t.parse::<i64>())
                && d != c
            {
                notes.push(format!("default_clip {d} differs from clip_size {c}"));
            }
        } else if c != -1 {
            notes.push(format!("clip_size {c} is malformed: kept the data's"));
        }
    }
    // A round's insert: only the Grenade Launcher's (the other shell guns
    // time it by their animation; a whole clip's never by this).
    let shells = doc
        .get("magazine")
        .and_then(Value::as_table)
        .and_then(|m| m.get("shells"))
        .is_some_and(Value::is_table);
    if let Some(r) = positive(&data, "TimeReload", notes)
        && shells
        && kind == Kind::Grenade
    {
        let m = table(doc, "magazine");
        m.insert("reload".into(), Value::Float(r));
        table(m, "shells").insert("insert".into(), Value::Float(r));
        set.push("[magazine] shells.insert and reload");
    }
    // The blast radius: the Demoman's launchers (the rocket's script has none).
    if matches!(kind, Kind::Grenade | Kind::Sticky)
        && let Some(r) = positive(&data, "DamageRadius", notes)
    {
        let h = kind.damage_hook().expect("a blast");
        let hk = hook(doc, h);
        let old = hk.get("radius").cloned();
        hk.insert("radius".into(), number(old.as_ref(), r));
        set.push(if kind == Kind::Grenade { "tf2:grenade radius" } else { "tf2:sticky radius" });
    }
    // The view kick: whole degrees up from P - 1 to P + 1 after each shot.
    if let Some(text) = data.string("PunchAngle") {
        match text.parse::<f64>() {
            Ok(p) if p >= 0.0 && p.is_finite() => {
                let hk = hook(doc, super::GUN_HOOK);
                hk.insert("punch".into(), Value::Float(p));
                set.push("tf2:gun punch");
            }
            _ => notes.push(format!("PunchAngle {text:?} is malformed: kept the data's")),
        }
    }
    // Checked, never applied.
    if let Some(n) = data.string("AmmoPerShot").and_then(|t| t.parse::<i64>().ok())
        && n != 1
    {
        notes.push(format!("AmmoPerShot {n}: one round per shot kept"));
    }
    if let Some(sounds) = data.block("SoundData") {
        let ours = doc.get("sounds").and_then(Value::as_table);
        for (role, field) in [("single_shot", "fire"), ("empty", "empty")] {
            if let (Some(theirs), Some(mine)) = (sounds.string(role), ours.and_then(|s| s.get(field)).and_then(Value::as_str))
                && !theirs.eq_ignore_ascii_case(mine)
            {
                notes.push(format!("SoundData {role} differs from [sounds] {field}"));
            }
        }
    }
    out.set = set;
    out
}

/// The smack delay of a melee script without `SmackDelay` (weapons_stock.md
/// 6: every stock melee but the Knife).
pub const SMACK_DELAY_DEFAULT: f64 = 0.2;

/// A melee script onto a melee weapon's document (weapons_stock.md 6):
/// `Damage` each swing's damage, `TimeFireDelay` each swing's hit and miss
/// refires (one interval for both), `SmackDelay` each swing's `delay`
/// (absent: 0.2 s); `Secondary_SmackDelay` isn't mapped; the swing and hit
/// sounds checked.
fn apply_melee(data: KvView, doc: &mut Table, mut out: Applied) -> Applied {
    let notes = &mut out.notes;
    let mut set = Vec::new();
    let damage = positive(&data, "Damage", notes);
    let interval = positive(&data, "TimeFireDelay", notes);
    let delay = positive(&data, "SmackDelay", notes).unwrap_or(SMACK_DELAY_DEFAULT);
    let swings: Vec<String> = table(doc, "melee").keys().filter(|k| *k == "primary" || *k == "secondary").cloned().collect();
    for name in &swings {
        let swing = table(table(doc, "melee"), name);
        if let Some(d) = damage {
            let old = swing.get("damage").cloned();
            swing.insert("damage".into(), number(old.as_ref(), d));
        }
        if let Some(i) = interval {
            for key in ["hit_refire", "miss_refire", "miss_refire_other"] {
                swing.insert(key.into(), Value::Float(i));
            }
        }
        swing.insert("delay".into(), Value::Float(delay));
    }
    if damage.is_some() {
        set.push("[melee] damage");
    }
    if interval.is_some() {
        set.push("[melee] refires");
    }
    set.push("[melee] delay");
    if let Some(sounds) = data.block("SoundData") {
        let ours = doc.get("melee").and_then(|m| m.get("primary")).and_then(Value::as_table);
        for (role, field) in [("melee_miss", "sound_swing"), ("melee_hit", "sound_hit"), ("melee_hit_world", "sound_hit_world")] {
            if let (Some(theirs), Some(mine)) = (sounds.string(role), ours.and_then(|s| s.get(field)).and_then(Value::as_str))
                && !theirs.eq_ignore_ascii_case(mine)
            {
                notes.push(format!("SoundData {role} differs from [melee.primary] {field}"));
            }
        }
    }
    out.set = set;
    out
}

/// One gun's numbers' source, as the log says it.
#[derive(Clone, Debug, PartialEq)]
pub struct GunSource {
    pub gun: String,
    /// Whether the script set any of its numbers.
    pub from_script: bool,
    pub source: String,
}

/// `script` applied to the TOML document `text` of `gun`: the document
/// (the text as it was when nothing applied) and its source.
pub fn gun_doc(gun: &str, text: &str, script: &Script) -> (String, GunSource) {
    let fallback = |why: String| (text.to_string(), GunSource { gun: gun.into(), from_script: false, source: why });
    if script.source == ScriptSource::Missing {
        return fallback(format!(
            "spec fallback (specs/tf2/weapons.md; no script: {})",
            script.note.as_deref().unwrap_or("not found")
        ));
    }
    let Some(data) = script.data() else {
        return fallback(format!("spec fallback (script {} has no block)", script.name));
    };
    let mut doc: Table = match text.parse() {
        Ok(d) => d,
        Err(e) => return fallback(format!("spec fallback (its data: {e})")),
    };
    let applied = apply_script(data, &mut doc);
    let notes = if applied.notes.is_empty() { String::new() } else { format!("; notes: {}", applied.notes.join("; ")) };
    if let Some(why) = applied.refused {
        return fallback(format!("spec fallback (script {} from {}: {why}{notes})", script.name, script.source.describe()));
    }
    if applied.set.is_empty() {
        return fallback(format!("spec fallback (script {} from {} set nothing{notes})", script.name, script.source.describe()));
    }
    let out = toml::to_string(&doc).unwrap_or_else(|_| text.to_string());
    let source = format!(
        "script {} from {} (set {}{notes})",
        script.name,
        script.source.describe(),
        applied.set.join(", ")
    );
    (out, GunSource { gun: gun.into(), from_script: true, source })
}

/// The documents TF2's guns register from, and each gun's source (a
/// resource: one an app has before `Tf2GunsPlugin` is the one used).
#[derive(bevy::prelude::Resource, Clone, Debug, PartialEq)]
pub struct GunNumbers {
    pub docs: Vec<(String, String)>,
    pub sources: Vec<GunSource>,
    /// The unlocks made from the item pack (`items`), their documents
    /// after the stock ones in `docs`.
    pub items: Vec<crate::game::items::ItemWeapon>,
}

impl GunNumbers {
    /// The committed documents as they are (the spec's numbers).
    pub fn builtin() -> Self {
        Self {
            docs: super::DATA.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect(),
            sources: Vec::new(),
            items: Vec::new(),
        }
    }

    /// These numbers with the unlocks of `pack` (`items::weapons`: those
    /// data/items.toml names), their documents appended; what was left
    /// out is noted in `sources`' log.
    pub fn with_pack(mut self, pack: &crate::game::items::ItemPack) -> Self {
        let data = crate::game::items::ItemsData::builtin();
        let (items, notes) = crate::game::items::weapons(pack, &data, &self.docs);
        for n in notes {
            self.sources.push(GunSource { gun: "items".into(), from_script: false, source: n });
        }
        for i in &items {
            self.docs.push((format!("items/{}.toml", i.id.trim_start_matches("tf2:")), i.doc.clone()));
            self.sources.push(GunSource {
                gun: i.id.clone(),
                from_script: true,
                source: format!("item {} on {} from {}", i.index, i.base, crate::game::items::PACK_PATH),
            });
        }
        self.items.extend(items);
        self
    }

    /// The documents with each gun's script read through `mount` (None:
    /// every gun's the spec fallback) and applied.
    pub fn from_mount(mount: Option<&Mount>, key: Option<&CtxKey>) -> Self {
        let numbers = Self::with_scripts_of(mount, key);
        match mount.and_then(crate::game::items::read_pack) {
            Some(pack) => numbers.with_pack(&pack),
            None => numbers,
        }
    }

    fn with_scripts_of(mount: Option<&Mount>, key: Option<&CtxKey>) -> Self {
        Self::with_scripts(|name| match mount {
            Some(m) => scripts::read(m, name, key),
            None => Script {
                name: name.into(),
                source: ScriptSource::Missing,
                root: Kv::Block(Vec::new()),
                note: Some("no TF2 mount (no install, no asset store)".into()),
            },
        })
    }

    /// `from_mount` with TF2's mount (`INFO.mount()`: the store and/or
    /// the install) and the configured key.
    pub fn from_scripts() -> Self {
        let mount = super::super::INFO.mount();
        let key = if mount.is_some() { scripts::key_or_warn() } else { None };
        Self::from_mount(mount.as_ref(), key.as_ref())
    }

    /// The documents with each gun's script from `script` (by name).
    pub fn with_scripts(script: impl Fn(&str) -> Script) -> Self {
        let mut docs = Vec::new();
        let mut sources = Vec::new();
        for (name, text) in super::DATA {
            let doc: Option<Table> = text.parse().ok();
            let gun = doc.as_ref().and_then(|d| d.get("id")).and_then(Value::as_str).map(str::to_string);
            let script_name = doc.as_ref().and_then(|d| {
                let hooks = d.get("hooks")?;
                let hook = hooks.get(super::GUN_HOOK).or_else(|| hooks.get(super::MELEE_HOOK))?;
                hook.get("script")?.as_str().map(str::to_string)
            });
            match (gun, script_name) {
                (Some(gun), Some(s)) if super::all_weapons().any(|g| g == gun) => {
                    let (text, source) = gun_doc(&gun, text, &script(&s));
                    docs.push((name.to_string(), text));
                    sources.push(source);
                }
                _ => docs.push((name.to_string(), text.to_string())),
            }
        }
        Self { docs, sources, items: Vec::new() }
    }

    /// The documents parsed as weapon data (as `Tf2GunsPlugin` loads them).
    pub fn parse(&self) -> Result<chimera_sim::weapon::data::WeaponSet, String> {
        let docs: Vec<(&str, &str)> = self.docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
        chimera_sim::weapon::data::WeaponSet::parse(&docs, &[])
    }

    /// Which of each gun's mapped numbers differ from the committed data's
    /// (`<gun> damage`, `pellets/spread`, `clip/insert`, `cycle`, `punch`,
    /// `hooks`): names only, never the numbers.
    pub fn differences(&self) -> Result<Vec<String>, String> {
        use chimera_sim::weapon::data::WeaponData;
        let (data, mapped) = (super::tf2_weapons(), self.parse()?);
        let amount = |d: &WeaponData| d.damage.as_ref().map(|d| d.amount);
        let pellets = |d: &WeaponData| d.hitscan.as_ref().map(|h| (h.pellets, format!("{:?}", h.spread)));
        let clip = |d: &WeaponData| d.magazine.as_ref().map(|m| (m.clip, m.reload, m.shells.as_ref().map(|s| s.insert)));
        let cycle = |d: &WeaponData| d.trigger.as_ref().map(|t| t.cycle);
        let melee = |d: &WeaponData| {
            d.melee.as_ref().and_then(|m| m.primary.as_ref()).map(|p| format!("{} {} {} {}", p.damage, p.hit_refire, p.miss_refire, p.delay))
        };
        let punch = |d: &WeaponData| d.hooks.get(super::GUN_HOOK).and_then(|g| chimera_sim::weapon::harm::num(g, "punch"));
        // The hooks but the view kick (its own row).
        let hooks = |d: &WeaponData| {
            let mut h = d.hooks.clone();
            if let Some(toml::Value::Table(g)) = h.get_mut(super::GUN_HOOK) {
                g.remove("punch");
            }
            h
        };
        let mut out = Vec::new();
        for gun in super::all_weapons() {
            let (Some(a), Some(b)) = (data.get(gun), mapped.get(gun)) else { continue };
            let close = cycle(a).zip(cycle(b)).is_none_or(|(x, y)| (x - y).abs() <= 1e-4);
            for (what, same) in [
                ("damage", amount(a) == amount(b)),
                ("pellets/spread", pellets(a) == pellets(b)),
                ("clip/insert", clip(a) == clip(b)),
                ("cycle", close),
                ("melee", melee(a) == melee(b)),
                ("punch", punch(a).unwrap_or(0.0) == punch(b).unwrap_or(0.0)),
                ("hooks", hooks(a) == hooks(b)),
            ] {
                if !same {
                    out.push(format!("{gun} {what}"));
                }
            }
        }
        Ok(out)
    }

    /// Whether every gun's numbers are the built-in ones.
    pub fn is_builtin(&self) -> bool {
        self.sources.iter().all(|s| !s.from_script)
    }

    /// Each gun's source, once: info from a script, a warning for the
    /// fallback.
    pub fn log(&self) {
        for s in &self.sources {
            if s.from_script {
                bevy::log::info!("tf2: {}'s numbers: {}", s.gun, s.source);
            } else {
                bevy::log::warn!("tf2: {}'s numbers: {}", s.gun, s.source);
            }
        }
    }
}

/// Each gun's script asked for through `mount` and its numbers' source
/// logged (a warning while it is the spec's).
pub fn note_sources(mount: &Mount) {
    let key = scripts::key_or_warn();
    GunNumbers::from_mount(Some(mount), key.as_ref()).log();
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_formats::keyvalues::parse_pc;

    fn script(text: &str) -> Script {
        Script { name: "tf_weapon_made_up".into(), source: ScriptSource::Plain, root: parse_pc(text), note: None }
    }

    #[test]
    fn a_missing_script_is_the_spec_fallback() {
        let missing = Script { name: "x".into(), source: ScriptSource::Missing, root: Kv::Block(Vec::new()), note: Some("no key".into()) };
        let (text, s) = gun_doc("tf2:x", "id = \"tf2:x\"\n", &missing);
        assert_eq!(text, "id = \"tf2:x\"\n");
        assert!(!s.from_script && s.source.starts_with("spec fallback"), "{}", s.source);
    }

    #[test]
    fn malformed_fields_keep_the_data() {
        let mut doc: Table = "slot = 0\n[trigger]\ncycle = 0.3\n[magazine]\nclip = 9\n[hitscan]\npellets = 1\n[damage]\namount = 5.0\n".parse().unwrap();
        let s = script("\"Odd\" { \"clip_size\" \"8.5\" \"BulletsPerShot\" \"two\" \"TimeFireDelay\" \"0\" \"damage\" \"7.5\" }");
        let a = apply_script(s.data().unwrap(), &mut doc);
        assert_eq!(a.set, ["[damage] amount"]);
        assert_eq!(doc["damage"]["amount"].as_float(), Some(7.0), "cut toward zero");
        assert_eq!(doc["magazine"]["clip"].as_integer(), Some(9));
        assert_eq!(doc["trigger"]["cycle"].as_float(), Some(0.3));
        assert_eq!(a.notes.len(), 3, "{:?}", a.notes);
    }
}
