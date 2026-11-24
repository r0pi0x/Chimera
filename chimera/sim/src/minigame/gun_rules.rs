//! The host's Guns rules for drawn ladders (docs/plans/active/guns-picker-v2.md,
//! step 2): server cvars, applied at each draw to whatever is there then
//! (new installs, downloaded skins, a joiner's games), so nothing stored
//! goes stale.
//!
//! - `mp_guns_games_off`, `mp_guns_classes_off`: the games and classes that
//!   are off (empty: all on; a game or class that appears later is on). A
//!   gun's class is its effective one (`auto::effective_class`: a pistol
//!   that hits like a heavy pistol is one).
//! - `mp_guns_skins`: `one` (default; a gun family is one candidate, its
//!   skin drawn at the deal) or `all` (every skin a candidate).
//! - `mp_guns_short`: what a ladder does once its stages have no gun left
//!   to give (`WhenShort`): `shorter` (default), `repeat`, `borrow`.
//! - `mp_guns_ban`, `mp_guns_allow`: family ids or weapon ids
//!   (`WeaponData::family`; a weapon id is one skin). `mp_guns_off`
//!   (`auto::GunsOff`, the old picker's) counts as bans.
//!
//! `apply` decides each gun in this order: Unavailable (a game not every
//! player has, a game's data exclusions, no damage: removed before it, by
//! `auto::draw_pool`), then Ban, then Allowed, then the rules (games,
//! classes). An Allowed gun of a game that is off goes into one shared
//! group (`ALLOWED_GROUP`), dealt as one game's turn. Rules that leave no
//! gun don't play an empty ladder: every available gun plays and `Pool::
//! fallback` says so (the draw logs it, `GunRulesNote` keeps it for the
//! Host page).
//!
//! Presets (`Preset`): the rules as a small TOML document (no gun list,
//! only stable family or weapon ids), the built-ins in
//! chimera/sim/data/guns_presets.toml, a host's own in the config folder
//! (`console::cfg_dir()/guns/<name>.toml`). `guns_preset [name]` lists
//! them or sets a server's cvars from one (`Preset::lines`);
//! `guns_preset_save <name>` writes the cvars' values as one.

use bevy::prelude::*;
use chimera_core::game_parts::GunClass;

use super::auto::{Capabilities, GunsOff, Skipped};

/// The game of the shared group of Allowed guns whose games are off.
pub const ALLOWED_GROUP: &str = "allowed";

/// How Combat Arms' skins and editions are drawn (`mp_guns_skins`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Skins {
    /// A family is one candidate; the skin is drawn at the deal.
    #[default]
    One,
    /// Every skin is a candidate of its own.
    All,
}

impl std::str::FromStr for Skins {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "one" | "" => Ok(Skins::One),
            "all" => Ok(Skins::All),
            o => Err(format!("{o}: one or all")),
        }
    }
}

impl std::fmt::Display for Skins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Skins::One => "one",
            Skins::All => "all",
        })
    }
}

/// What a drawn ladder does when its stages have no gun left to give
/// (`mp_guns_short`; a short stage first gives its rungs to the nearest
/// stage with guns to spare, as always).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WhenShort {
    /// The ladder is shorter (the knife finale kept).
    #[default]
    Shorter,
    /// The stages' guns are dealt again, spread out: never the same gun
    /// twice in a row while another is there.
    Repeat,
    /// Guns of the classes the rules turned off, from games that are on,
    /// the nearest stage first; never a banned or unavailable one. None
    /// left: shorter.
    Borrow,
}

impl std::str::FromStr for WhenShort {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "shorter" | "" => Ok(WhenShort::Shorter),
            "repeat" => Ok(WhenShort::Repeat),
            "borrow" => Ok(WhenShort::Borrow),
            o => Err(format!("{o}: shorter, repeat or borrow")),
        }
    }
}

impl std::fmt::Display for WhenShort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            WhenShort::Shorter => "shorter",
            WhenShort::Repeat => "repeat",
            WhenShort::Borrow => "borrow",
        })
    }
}

/// The host's Guns rules (module doc), one cvar each.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct GunRules {
    /// Game ids, comma- or space-separated.
    pub games_off: String,
    /// Class names (`GunClass::name`, `grenade` too).
    pub classes_off: String,
    pub skins: Skins,
    pub short: WhenShort,
    /// Family or weapon ids.
    pub ban: String,
    /// Family or weapon ids.
    pub allow: String,
}

/// Whether `list` (comma- or space-separated) names `id`, case aside.
pub fn listed(list: &str, id: &str) -> bool {
    list.split([',', ' ', '\t', '\n']).any(|w| !w.is_empty() && w.eq_ignore_ascii_case(id))
}

/// `list`'s entries, in order, without blanks.
pub fn entries(list: &str) -> Vec<&str> {
    list.split([',', ' ', '\t', '\n']).filter(|w| !w.is_empty()).collect()
}

impl GunRules {
    pub fn game_on(&self, game: &str) -> bool {
        !listed(&self.games_off, game)
    }

    pub fn class_on(&self, class: GunClass) -> bool {
        !listed(&self.classes_off, class.name())
    }

    /// Banned by its id or its family's.
    pub fn banned(&self, id: &str, family: &str) -> bool {
        listed(&self.ban, id) || listed(&self.ban, family)
    }

    /// Allowed by its id or its family's.
    pub fn allowed(&self, id: &str, family: &str) -> bool {
        listed(&self.allow, id) || listed(&self.allow, family)
    }

    /// Game `game` on or off (`games_off`).
    pub fn set_game_on(&mut self, game: &str, on: bool) {
        self.games_off = set_listed(&self.games_off, game, !on);
    }

    /// Class `class` on or off (`classes_off`).
    pub fn set_class_on(&mut self, class: GunClass, on: bool) {
        self.classes_off = set_listed(&self.classes_off, class.name(), !on);
    }

    /// Solo: `game` alone of `games` on; solo already, every game on
    /// again.
    pub fn solo(&mut self, game: &str, games: &[&str]) {
        let alone = self.game_on(game) && games.iter().filter(|g| !g.eq_ignore_ascii_case(game)).all(|g| !self.game_on(g));
        self.games_off = if alone { String::new() } else { games.iter().filter(|g| !g.eq_ignore_ascii_case(game)).copied().collect::<Vec<_>>().join(",") };
    }

    /// How the draw decides a gun family whose rules (games, classes) let
    /// it in or not (`by_rules`): Ban, then Allowed, then the rules.
    pub fn decided(&self, family: &str, by_rules: bool) -> Decided {
        if listed(&self.ban, family) {
            Decided::Banned
        } else if listed(&self.allow, family) {
            Decided::Allowed
        } else if by_rules {
            Decided::In
        } else {
            Decided::Out
        }
    }

    /// A family's one-click exception (the Guns dialog's card): banned
    /// back to the rules; one the rules let in banned; one they leave out
    /// allowed, or an allowed one back to the rules. The new decision.
    pub fn cycle_family(&mut self, family: &str, by_rules: bool) -> Decided {
        match self.decided(family, by_rules) {
            Decided::Banned => self.ban = set_listed(&self.ban, family, false),
            Decided::Allowed => self.allow = set_listed(&self.allow, family, false),
            Decided::In => self.ban = set_listed(&self.ban, family, true),
            Decided::Out => self.allow = set_listed(&self.allow, family, true),
        }
        self.decided(family, by_rules)
    }
}

/// How the draw decides a gun family (`GunRules::decided`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decided {
    Banned,
    Allowed,
    In,
    Out,
}

impl Decided {
    /// In a ladder's pool.
    pub fn dealt(self) -> bool {
        matches!(self, Decided::Allowed | Decided::In)
    }

    /// An exception to the rules (banned or allowed).
    pub fn exception(self) -> bool {
        matches!(self, Decided::Banned | Decided::Allowed)
    }
}

/// `list` (comma- or space-separated) with `id` in it (`on`) or out of
/// it (case aside), comma-separated, its order kept.
pub fn set_listed(list: &str, id: &str, on: bool) -> String {
    let mut out: Vec<&str> = entries(list).into_iter().filter(|w| !w.eq_ignore_ascii_case(id)).collect();
    if on {
        out.push(id);
    }
    out.join(",")
}

/// Each stage's rungs of a ladder of `rungs` (`power::band_counts` with
/// no jitter: what the Guns dialog shows; a match's draw moves the
/// boundaries a little).
pub fn stage_rungs(rungs: usize, curve: &super::power::PowerCurve) -> [usize; 7] {
    let mut c = curve.clone();
    c.jitter = 0.0;
    super::power::band_counts(rungs, &c, &mut super::auto::Rng::new(0))
}

/// The host's drawn-ladder length, rungs in all with the knife finale's
/// (an app registers its cvar: the Ultimate Gun Game's `ugg_rungs`; the
/// Host page's Guns line reads it). Within the curve's `[rungs]`.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LadderRungs(pub usize);

/// Why `apply` left a gun out.
pub const WHY_BANNED: &str = "the host banned it";
pub const WHY_GAME_OFF: &str = "its game is off";
pub const WHY_CLASS_OFF: &str = "its class is off";

/// What `apply` leaves a draw.
#[derive(Clone, Debug, Default)]
pub struct Pool {
    /// The games' guns the rules let in, and the Allowed guns of games that
    /// are off as one more game (`ALLOWED_GROUP`) when there are any.
    pub caps: Vec<Capabilities>,
    /// The guns only their class keeps out, of games that are on, for
    /// `WhenShort::Borrow`.
    pub reserve: Vec<Capabilities>,
    pub skipped: Vec<Skipped>,
    /// The rules left no gun: every available gun plays, and this says so.
    pub fallback: Option<String>,
}

/// The last draw's fallback note (`Pool::fallback`), for the Host page;
/// None when the rules were kept.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct GunRulesNote(pub Option<String>);

/// The fallback's words.
pub const FALLBACK: &str = "The Guns rules leave no gun to deal, so every available gun plays";

fn shell(c: &Capabilities) -> Capabilities {
    Capabilities {
        guns: Vec::new(),
        grenades: Vec::new(),
        ..c.clone()
    }
}

/// The shared group of Allowed guns whose games are off.
pub fn allowed_group() -> Capabilities {
    Capabilities {
        game: ALLOWED_GROUP,
        guns: Vec::new(),
        grenades: Vec::new(),
        knife: None,
        movement: Some(ALLOWED_GROUP),
        characters: None,
        hud: None,
        maps: Vec::new(),
        missing: Vec::new(),
        scaffold: false,
    }
}

/// `caps` (the available guns: Unavailable already out) under `rules`
/// and the old guns off (`legacy`, bans too): Ban, then Allowed, then the
/// games and classes (module doc). `family` names a gun's family (its own
/// id when it has none); `class` its effective class.
pub fn apply(
    rules: &GunRules,
    legacy: Option<&GunsOff>,
    caps: Vec<Capabilities>,
    family: &dyn Fn(&str) -> String,
    class: &dyn Fn(&str, GunClass) -> GunClass,
) -> Pool {
    let mut pool = Pool::default();
    let mut group = allowed_group();
    for c in &caps {
        let on = rules.game_on(c.game);
        let mut kept = shell(c);
        let mut reserve = shell(c);
        let weapons = c.guns.iter().map(|g| (g.id, Some(*g))).chain(c.grenades.iter().map(|g| (*g, None)));
        for (id, gun) in weapons {
            let fam = family(id);
            let cls = gun.map_or(GunClass::Grenade, |g| class(id, g.class));
            let mut skip = |why: &str| pool.skipped.push(Skipped { id, why: why.to_string() });
            let into = if rules.banned(id, &fam) || legacy.is_some_and(|o| o.has(id)) {
                skip(WHY_BANNED);
                None
            } else if rules.allowed(id, &fam) {
                Some(if on { &mut kept } else { &mut group })
            } else if !on {
                skip(WHY_GAME_OFF);
                None
            } else if !rules.class_on(cls) {
                skip(WHY_CLASS_OFF);
                Some(&mut reserve)
            } else {
                Some(&mut kept)
            };
            if let Some(to) = into {
                match gun {
                    Some(g) => to.guns.push(g),
                    None => to.grenades.push(id),
                }
            }
        }
        pool.caps.push(kept);
        if !reserve.guns.is_empty() || !reserve.grenades.is_empty() {
            pool.reserve.push(reserve);
        }
    }
    if !group.guns.is_empty() || !group.grenades.is_empty() {
        pool.caps.push(group);
    }
    let had = caps.iter().any(|c| !c.guns.is_empty());
    if had && pool.caps.iter().all(|c| c.guns.is_empty()) {
        return Pool {
            caps,
            fallback: Some(FALLBACK.to_string()),
            ..default()
        };
    }
    pool
}

/// The built-in presets' file.
pub const PRESETS: &str = include_str!("../../data/guns_presets.toml");

/// A Guns preset: the host's rules by name (module doc). Missing keys are
/// the defaults; `rungs` None keeps the server's.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Preset {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub games_off: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub classes_off: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub skins: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub short: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rungs: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ban: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<String>,
}

#[derive(serde::Deserialize)]
struct PresetFile {
    preset: Vec<Preset>,
}

impl Preset {
    /// The built-in presets (`PRESETS`).
    pub fn builtin() -> Vec<Preset> {
        toml::from_str::<PresetFile>(PRESETS).expect("data/guns_presets.toml reads").preset
    }

    /// One preset's TOML text (a file, Copy as text).
    pub fn to_text(&self) -> String {
        toml::to_string(self).unwrap_or_default()
    }

    /// A preset from its text; its values checked.
    pub fn from_text(text: &str) -> Result<Preset, String> {
        let p: Preset = toml::from_str(text).map_err(|e| e.to_string())?;
        p.skins.parse::<Skins>()?;
        p.short.parse::<WhenShort>()?;
        Ok(p)
    }

    /// The preset of `rules` (and `rungs`), named `name`.
    pub fn of(name: &str, rules: &GunRules, rungs: Option<usize>) -> Preset {
        let list = |s: &str| entries(s).into_iter().map(str::to_string).collect();
        Preset {
            name: name.to_string(),
            games_off: list(&rules.games_off),
            classes_off: list(&rules.classes_off),
            skins: if rules.skins == Skins::default() { String::new() } else { rules.skins.to_string() },
            short: if rules.short == WhenShort::default() { String::new() } else { rules.short.to_string() },
            rungs,
            ban: list(&rules.ban),
            allow: list(&rules.allow),
        }
    }

    /// Its rules (`rungs` aside).
    pub fn rules(&self) -> GunRules {
        GunRules {
            games_off: self.games_off.join(","),
            classes_off: self.classes_off.join(","),
            skins: self.skins.parse().unwrap_or_default(),
            short: self.short.parse().unwrap_or_default(),
            ban: self.ban.join(","),
            allow: self.allow.join(","),
        }
    }

    /// The console lines that set a server to it (`ugg_rungs` only when it
    /// names rungs).
    pub fn lines(&self) -> Vec<String> {
        use chimera_core::console::quote;
        let r = self.rules();
        let mut out = vec![
            format!("mp_guns_games_off {}", quote(&r.games_off)),
            format!("mp_guns_classes_off {}", quote(&r.classes_off)),
            format!("mp_guns_skins {}", r.skins),
            format!("mp_guns_short {}", r.short),
            format!("mp_guns_ban {}", quote(&r.ban)),
            format!("mp_guns_allow {}", quote(&r.allow)),
            "mp_guns_off \"\"".to_string(),
        ];
        if let Some(n) = self.rungs {
            out.push(format!("ugg_rungs {n}"));
        }
        out
    }
}

/// The preset among `presets` that `rules` (and `rungs`) are, if any: the
/// first whose rules are the same and whose rungs are the same or unset.
pub fn matching<'a>(presets: &'a [Preset], rules: &GunRules, rungs: usize) -> Option<&'a Preset> {
    presets.iter().find(|p| p.rules() == *rules && p.rungs.is_none_or(|n| n == rungs))
}

/// The Host page's Guns line: the preset's name (or "Custom" with what
/// differs), the rungs, and a fallback note when the last draw had one
/// ("No explosives · 13 rungs").
pub fn summary(presets: &[Preset], rules: &GunRules, rungs: usize, note: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    match matching(presets, rules, rungs) {
        Some(p) => parts.push(p.name.clone()),
        None => {
            parts.push("Custom".into());
            let count = |l: &str, one: &str, many: &str| match entries(l).len() {
                0 => None,
                1 => Some(format!("1 {one}")),
                n => Some(format!("{n} {many}")),
            };
            parts.extend(count(&rules.games_off, "game off", "games off"));
            parts.extend(count(&rules.classes_off, "class off", "classes off"));
            parts.extend(count(&rules.ban, "ban", "bans"));
            parts.extend(count(&rules.allow, "allowed", "allowed"));
        }
    }
    parts.push(format!("{rungs} rungs"));
    let mut line = parts.join(" \u{00b7} ");
    if let Some(n) = note {
        line.push_str(&format!(" \u{00b7} {n}"));
    }
    line
}

/// A host's presets' folder (`console::cfg_dir()/guns`).
pub fn presets_dir() -> Option<std::path::PathBuf> {
    chimera_core::console::cfg_dir().map(|d| d.join("guns"))
}

/// A file name for preset `name`: letters, digits, `-` and `_`, the rest
/// `_`, lower case.
pub fn file_name(name: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c.to_ascii_lowercase() } else { '_' })
        .collect();
    format!("{}.toml", if stem.is_empty() { "preset".into() } else { stem })
}

/// The host's own presets in `dir`, by name (unreadable files skipped).
pub fn saved_in(dir: &std::path::Path) -> Vec<Preset> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut paths: Vec<std::path::PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "toml")).collect();
    paths.sort();
    paths.iter().filter_map(|p| Preset::from_text(&std::fs::read_to_string(p).ok()?).ok()).collect()
}

/// Preset `name` (case aside): a host's own first, else a built-in.
pub fn find(name: &str) -> Option<Preset> {
    let saved = presets_dir().map(|d| saved_in(&d)).unwrap_or_default();
    saved.into_iter().chain(Preset::builtin()).find(|p| p.name.eq_ignore_ascii_case(name.trim()))
}

/// Registers the rules' cvars (`GunRules`) and `GunRulesNote`.
pub fn cvars(app: &mut App) {
    use crate::console::resource_cvar;
    app.init_resource::<GunRules>().init_resource::<GunRulesNote>();
    resource_cvar::<GunRules, String>(
        app,
        "mp_guns_games_off",
        "Server: the games whose guns drawn ladders leave out, comma-separated game ids (empty: every game). From the next ladder.",
        |r| &mut r.games_off,
    );
    resource_cvar::<GunRules, String>(
        app,
        "mp_guns_classes_off",
        "Server: the gun classes drawn ladders leave out (pistol, smg, shotgun, carbine, rifle, heavy, marksman, sniper, heavy_pistol, launcher, flamethrower, grenade). From the next ladder.",
        |r| &mut r.classes_off,
    );
    resource_cvar::<GunRules, Skins>(
        app,
        "mp_guns_skins",
        "Server: one (a gun's skins and editions are one candidate, the skin drawn at the deal) or all (each its own). From the next ladder.",
        |r| &mut r.skins,
    );
    resource_cvar::<GunRules, WhenShort>(
        app,
        "mp_guns_short",
        "Server: when the guns can't fill a ladder: shorter, repeat (guns come back, spread out) or borrow (classes turned off, from games that are on). From the next ladder.",
        |r| &mut r.short,
    );
    resource_cvar::<GunRules, String>(
        app,
        "mp_guns_ban",
        "Server: gun families or weapon ids drawn ladders never deal, comma-separated (over every rule but availability). From the next ladder.",
        |r| &mut r.ban,
    );
    resource_cvar::<GunRules, String>(
        app,
        "mp_guns_allow",
        "Server: gun families or weapon ids drawn ladders may deal whatever the game and class rules say (a ban still wins). From the next ladder.",
        |r| &mut r.allow,
    );
    use chimera_core::console::ConsoleAppExt;
    app.console_command(
        "chimera:guns_preset",
        "guns_preset [name]: the Guns presets (built-in and the host's own, config folder guns/), or set the server's Guns rules to one. From the next ladder.",
        |w, a| {
            if a.is_empty() {
                let saved = presets_dir().map(|d| saved_in(&d)).unwrap_or_default();
                let names: Vec<String> = Preset::builtin().iter().chain(&saved).map(|p| p.name.clone()).collect();
                return Ok(Some(names.join("\n")));
            }
            let name = a.join(" ");
            let p = find(&name).ok_or_else(|| format!("no Guns preset named {name}"))?;
            let mut console = w.resource_mut::<chimera_core::console::Console>();
            for line in p.lines() {
                console.submit(&line);
            }
            Ok(Some(format!("Guns: {}", p.name)))
        },
    )
    .console_command(
        "chimera:guns_preset_save",
        "guns_preset_save <name>: save the server's Guns rules (and ugg_rungs) as a preset in the config folder (guns/<name>.toml).",
        |w, a| {
            let name = a.join(" ");
            if name.trim().is_empty() {
                return Err("guns_preset_save <name>".into());
            }
            let rules = w.get_resource::<GunRules>().cloned().unwrap_or_default();
            let get = w.resource::<chimera_core::console::Console>().cvar("ugg_rungs").map(|c| c.get.clone());
            let rungs = get.and_then(|g| g(w)).and_then(|v| v.parse().ok());
            let dir = presets_dir().ok_or("no config folder")?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let path = dir.join(file_name(&name));
            std::fs::write(&path, Preset::of(name.trim(), &rules, rungs).to_text()).map_err(|e| e.to_string())?;
            Ok(Some(format!("wrote {}", path.display())))
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_core::game_parts::GunPart;

    fn cap(game: &'static str, guns: &[(&'static str, GunClass)], grenades: &[&'static str]) -> Capabilities {
        Capabilities {
            game,
            guns: guns.iter().map(|&(id, class)| GunPart { id, class }).collect(),
            grenades: grenades.to_vec(),
            knife: None,
            movement: Some("m"),
            characters: None,
            hud: None,
            maps: Vec::new(),
            missing: Vec::new(),
            scaffold: false,
        }
    }

    fn caps() -> Vec<Capabilities> {
        vec![
            cap("a", &[("a:glock", GunClass::Pistol), ("a:awp", GunClass::Sniper), ("a:ak_gold", GunClass::Rifle)], &["a:he"]),
            cap("b", &[("b:mp5", GunClass::Smg), ("b:rpg", GunClass::Launcher)], &[]),
        ]
    }

    fn family(id: &str) -> String {
        if id.starts_with("a:ak") { "a:ak".into() } else { id.into() }
    }

    fn same(_: &str, c: GunClass) -> GunClass {
        c
    }

    fn ids(pool: &Pool) -> Vec<(&str, Vec<&str>)> {
        pool.caps
            .iter()
            .map(|c| (c.game, c.guns.iter().map(|g| g.id).chain(c.grenades.iter().copied()).collect()))
            .collect()
    }

    #[test]
    fn default_rules_keep_everything() {
        let p = apply(&GunRules::default(), None, caps(), &family, &same);
        assert_eq!(ids(&p), [("a", vec!["a:glock", "a:awp", "a:ak_gold", "a:he"]), ("b", vec!["b:mp5", "b:rpg"])]);
        assert!(p.fallback.is_none() && p.reserve.is_empty());
    }

    #[test]
    fn games_and_classes_off_leave_guns_out_and_class_ones_wait_in_reserve() {
        let r = GunRules {
            games_off: "b".into(),
            classes_off: "sniper, grenade".into(),
            ..default()
        };
        let p = apply(&r, None, caps(), &family, &same);
        assert_eq!(ids(&p), [("a", vec!["a:glock", "a:ak_gold"]), ("b", vec![])]);
        // Only the class keeps the AWP and the grenade out; game b's guns
        // aren't borrowed.
        assert_eq!(p.reserve.len(), 1);
        assert_eq!(p.reserve[0].guns.iter().map(|g| g.id).collect::<Vec<_>>(), ["a:awp"]);
        assert_eq!(p.reserve[0].grenades, ["a:he"]);
    }

    #[test]
    fn a_ban_names_a_family_or_one_skin_and_the_old_guns_off_are_bans() {
        let r = GunRules { ban: "a:ak".into(), ..default() };
        let p = apply(&r, Some(&GunsOff("b:rpg".into())), caps(), &family, &same);
        assert_eq!(ids(&p), [("a", vec!["a:glock", "a:awp", "a:he"]), ("b", vec!["b:mp5"])]);
        assert!(p.skipped.iter().all(|s| s.why == WHY_BANNED));
        let r = GunRules { ban: "A:AK_GOLD".into(), ..default() };
        assert!(!ids(&apply(&r, None, caps(), &family, &same))[0].1.contains(&"a:ak_gold"));
    }

    /// Unavailable, then Ban, then Allowed, then the rules.
    #[test]
    fn precedence_ban_over_allowed_over_rules_and_allowed_of_games_off_share_a_group() {
        let r = GunRules {
            games_off: "b".into(),
            classes_off: "sniper".into(),
            ban: "a:ak, b:mp5".into(),
            allow: "a:ak, a:awp, b:rpg, b:mp5".into(),
            ..default()
        };
        let p = apply(&r, None, caps(), &family, &same);
        // Banned beats allowed (the AK family, the MP5); allowed beats a
        // class off (the AWP) and a game off (the RPG, in the group).
        assert_eq!(ids(&p), [("a", vec!["a:glock", "a:awp", "a:he"]), ("b", vec![]), (ALLOWED_GROUP, vec!["b:rpg"])]);
        assert!(p.caps[2].playable());
    }

    #[test]
    fn the_effective_class_decides() {
        let heavy = |id: &str, c: GunClass| if id == "a:glock" { GunClass::HeavyPistol } else { c };
        let r = GunRules { classes_off: "heavy_pistol".into(), ..default() };
        let p = apply(&r, None, caps(), &family, &heavy);
        assert!(!ids(&p)[0].1.contains(&"a:glock"));
    }

    #[test]
    fn rules_that_leave_no_gun_play_every_gun_and_say_so() {
        let r = GunRules { games_off: "a b".into(), ..default() };
        let p = apply(&r, None, caps(), &family, &same);
        assert_eq!(p.fallback.as_deref(), Some(FALLBACK));
        assert_eq!(ids(&p)[0].1.len(), 4);
    }

    #[test]
    fn presets_read_round_trip_and_set_the_cvars() {
        let built = Preset::builtin();
        assert_eq!(built[0].name, "All guns");
        assert_eq!(built[0].rules(), GunRules::default());
        let r = GunRules {
            games_off: "tf2".into(),
            classes_off: "sniper,launcher".into(),
            short: WhenShort::Borrow,
            ban: "combat_arms:ak47".into(),
            ..default()
        };
        let p = Preset::of("Friday night", &r, Some(17));
        let text = p.to_text();
        assert!(!text.contains("skins"), "defaults stay out: {text}");
        let back = Preset::from_text(&text).unwrap();
        assert_eq!(back, p);
        assert_eq!(back.rules(), r);
        assert!(back.lines().contains(&"ugg_rungs 17".to_string()));
        assert!(Preset::from_text("name = \"x\"\nshort = \"maybe\"").is_err());
        assert_eq!(file_name("Friday night!"), "friday_night_.toml");
    }

    #[test]
    fn the_summary_names_the_preset_or_what_differs() {
        let built = Preset::builtin();
        assert_eq!(summary(&built, &GunRules::default(), 13, None), "All guns \u{00b7} 13 rungs");
        let r = built[1].rules();
        assert_eq!(summary(&built, &r, 9, None), "No explosives \u{00b7} 9 rungs");
        let r = GunRules { games_off: "tf2".into(), ban: "a b".into(), ..default() };
        assert_eq!(summary(&built, &r, 13, Some("x")), "Custom \u{00b7} 1 game off \u{00b7} 2 bans \u{00b7} 13 rungs \u{00b7} x");
    }

    #[test]
    fn rule_edits_keep_the_lists_and_a_card_cycles_ban_and_allow() {
        let mut r = GunRules::default();
        r.set_game_on("tf2", false);
        r.set_game_on("hl2dm", false);
        r.set_game_on("TF2", true);
        assert_eq!(r.games_off, "hl2dm");
        r.set_class_on(GunClass::Sniper, false);
        assert_eq!(r.classes_off, "sniper");
        r.solo("a", &["a", "b", "c"]);
        assert_eq!(r.games_off, "b,c");
        r.solo("a", &["a", "b", "c"]);
        assert_eq!(r.games_off, "", "solo again: all on");
        assert_eq!(r.cycle_family("x:ak", true), Decided::Banned);
        assert_eq!(r.cycle_family("x:ak", true), Decided::In);
        assert_eq!(r.cycle_family("x:rpg", false), Decided::Allowed);
        assert_eq!(r.allow, "x:rpg");
        assert_eq!(r.cycle_family("x:rpg", false), Decided::Out);
        let s = stage_rungs(13, &crate::minigame::power::PowerCurve::shipped());
        assert_eq!(s.iter().sum::<usize>(), 13);
        assert_eq!(s[6], 1, "the knife last");
    }

    #[test]
    fn the_cvars_values_read_and_write() {
        assert_eq!("Repeat".parse::<WhenShort>(), Ok(WhenShort::Repeat));
        assert_eq!(WhenShort::Borrow.to_string(), "borrow");
        assert_eq!("all".parse::<Skins>(), Ok(Skins::All));
        assert!("some".parse::<Skins>().is_err());
    }
}
