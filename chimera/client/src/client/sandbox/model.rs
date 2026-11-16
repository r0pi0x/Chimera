//! The sandbox's gun catalogue and the Change gun view's state, as plain
//! data (the design rules, sections 18 to 21; no ECS, so tests drive it
//! directly).
//!
//! - **The catalogue** (`SandboxGun`, `catalogue_of`): every gun a
//!   sandbox may hand you (the ladder's rungs but its sentinel, one per
//!   rung), with its game, its name, its class's section, its kit icon,
//!   its global tags (`TagGroup`'s eight groups: type, delivery, fire mode,
//!   reload, optics, range feel, behaviour, files), its game's own tags
//!   (`game_tags`: Team Fortress 2's classes that carry it) and its stats
//!   (`GunStats`). Tags derive from the weapon data (`derive_tags`: the
//!   class, `hitscan`, `trigger`, `magazine`, `melee`, `damage` and the
//!   hooks: a `*:projectile`, a `*:device`, a `*:zoom`, Halo's heat, TF2's
//!   `arms`); `HAND_TAGS` lists the few that need hand data. TF2's
//!   classes that carry a gun are its loadouts' (the tf2 game's
//!   `classes.toml`, which this layer can't read): only `arms` comes
//!   through, so a gun several classes carry needs that list handed over.
//! - **The view's state** (`GunView`): the games ticked in the left column
//!   (none: all), each game's facets ticked under it, the global tags
//!   ticked in the popover, the search words, the cursor's gun (`selected`)
//!   and whether the tag popover is open. `shown` is what the tile grid
//!   lists, by class section in the classes' order, each section's guns in
//!   the ladder's order: a gun shows when its game is ticked (or none
//!   is), it has every facet ticked for its game, every global tag ticked
//!   (AND inside the list) and every search word is in its name, id, game,
//!   class or tags.
//! - **Pins** (`Pins`): six slots, keys 1 to 6; `pin` puts a gun in a slot
//!   (moving it if it was pinned elsewhere), `unpin` empties one. The bar's
//!   fade (`PinBar`): shown on a number key, Tab or a drag, faded after
//!   `PIN_IDLE` s idle.
//!
//! Known shortcuts: "energy", "charge", "burst", "spin-up", "iron sights",
//! "bounces", "penetrates", "afterburn", "sticks", "homing" and
//! "knockback" come only from `HAND_TAGS` (no data field says them yet);
//! the range feel is the class's, not measured.

use std::collections::{BTreeMap, BTreeSet};

use chimera_core::game_parts::GunClass;
use chimera_sim::weapon::data::WeaponData;

/// The global tags' groups, in the popover's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TagGroup {
    Type,
    Delivery,
    FireMode,
    Reload,
    Optics,
    RangeFeel,
    Behaviour,
    Files,
}

impl TagGroup {
    pub const ALL: [TagGroup; 8] = [
        TagGroup::Type,
        TagGroup::Delivery,
        TagGroup::FireMode,
        TagGroup::Reload,
        TagGroup::Optics,
        TagGroup::RangeFeel,
        TagGroup::Behaviour,
        TagGroup::Files,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TagGroup::Type => "Type",
            TagGroup::Delivery => "Delivery",
            TagGroup::FireMode => "Fire mode",
            TagGroup::Reload => "Reload",
            TagGroup::Optics => "Optics",
            TagGroup::RangeFeel => "Range feel",
            TagGroup::Behaviour => "Behaviour",
            TagGroup::Files => "Files",
        }
    }

    /// The group's tags, in the popover's order.
    pub fn tags(self) -> &'static [&'static str] {
        match self {
            TagGroup::Type => &["pistol", "smg", "rifle", "mg", "shotgun", "sniper", "launcher", "grenade", "flame", "energy", "placeable", "melee"],
            TagGroup::Delivery => &["hitscan", "projectile", "thrown", "stream", "placed", "melee"],
            TagGroup::FireMode => &["semi", "burst", "auto", "charge", "pump or bolt", "spin-up"],
            TagGroup::Reload => &["magazine", "shell by shell", "overheat", "clip"],
            TagGroup::Optics => &["iron sights", "scope", "zoom"],
            TagGroup::RangeFeel => &["close", "mid", "long"],
            TagGroup::Behaviour => &[
                "explosive", "splash", "self-damage", "knockback", "afterburn", "sticks", "homing", "bounces", "penetrates", "headshot",
            ],
            TagGroup::Files => &["here", "not here", "downloading"],
        }
    }
}

/// A global tag: its group and its word.
pub type Tag = (TagGroup, &'static str);

/// Tags no data field says yet, by gun id (module doc, "Known shortcuts").
pub const HAND_TAGS: &[(&str, &[Tag])] = &[
    ("hl2dm:weapon_ar2", &[(TagGroup::Type, "energy")]),
    ("halo_ce:plasma_pistol", &[(TagGroup::Type, "energy"), (TagGroup::FireMode, "charge")]),
    ("halo_ce:plasma_rifle", &[(TagGroup::Type, "energy")]),
    ("halo_ce:needler", &[(TagGroup::Behaviour, "homing"), (TagGroup::Behaviour, "sticks")]),
    ("halo_ce:pistol", &[(TagGroup::Optics, "zoom")]),
    ("hl2dm:weapon_crossbow", &[(TagGroup::Behaviour, "sticks")]),
    ("tf2:tf_weapon_minigun", &[(TagGroup::FireMode, "spin-up")]),
    ("tf2:tf_weapon_flamethrower", &[(TagGroup::Behaviour, "afterburn")]),
    ("tf2:tf_weapon_pipebomblauncher", &[(TagGroup::Behaviour, "sticks"), (TagGroup::FireMode, "charge")]),
    ("tf2:tf_weapon_grenadelauncher", &[(TagGroup::Behaviour, "bounces")]),
    ("tf2:tf_weapon_rocketlauncher", &[(TagGroup::Behaviour, "knockback")]),
    ("cs_source:weapon_famas", &[(TagGroup::FireMode, "burst")]),
    ("cs_source:weapon_glock", &[(TagGroup::FireMode, "burst")]),
    ("cs_source:weapon_awp", &[(TagGroup::Behaviour, "penetrates")]),
];

/// A gun's numbers for the detail pane (None: not in its data).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GunStats {
    pub damage: Option<f32>,
    /// Shots a minute.
    pub fire_rate: Option<f32>,
    pub magazine: Option<u32>,
    pub range_feel: Option<&'static str>,
    pub head_mult: Option<f32>,
    pub price: Option<u32>,
}

/// One gun of the catalogue (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SandboxGun {
    pub id: String,
    /// Its rung, 0-based (what switching to it moves you to).
    pub rung: usize,
    /// The game's id, full name and short name (`cs_source`,
    /// "Counter-Strike: Source", "CS:S").
    pub game: String,
    pub game_name: String,
    pub game_short: String,
    pub name: String,
    pub class: Option<GunClass>,
    /// The kit's icon by class (`signal::icon_class`).
    pub icon_class: Option<&'static str>,
    pub tags: BTreeSet<Tag>,
    /// Its game's own facets (Team Fortress 2: the classes that carry it).
    pub game_tags: Vec<String>,
    pub stats: GunStats,
    /// The download unit its files are in (the asset side's name; None:
    /// part of the base install).
    pub unit: Option<String>,
}

/// A class section's heading, in the tile grid.
pub fn section_name(class: Option<GunClass>) -> &'static str {
    match class {
        Some(GunClass::Pistol | GunClass::HeavyPistol) => "Pistols",
        Some(GunClass::Smg) => "SMGs",
        Some(GunClass::Carbine | GunClass::Rifle | GunClass::Heavy) => "Rifles and MGs",
        Some(GunClass::Shotgun) => "Shotguns",
        Some(GunClass::Marksman | GunClass::Sniper) => "Sniper rifles",
        Some(GunClass::Launcher | GunClass::Flamethrower) => "Launchers and heavy",
        Some(GunClass::Grenade) => "Grenades",
        None => "Melee and other",
    }
}

/// The sections' order.
pub const SECTIONS: [&str; 8] =
    ["Pistols", "SMGs", "Rifles and MGs", "Shotguns", "Sniper rifles", "Launchers and heavy", "Grenades", "Melee and other"];

fn type_tag(class: Option<GunClass>) -> Option<&'static str> {
    Some(match class? {
        GunClass::Pistol | GunClass::HeavyPistol => "pistol",
        GunClass::Smg => "smg",
        GunClass::Carbine | GunClass::Rifle | GunClass::Marksman => "rifle",
        GunClass::Heavy => "mg",
        GunClass::Shotgun => "shotgun",
        GunClass::Sniper => "sniper",
        GunClass::Launcher => "launcher",
        GunClass::Grenade => "grenade",
        GunClass::Flamethrower => "flame",
    })
}

fn range_feel(class: Option<GunClass>, melee: bool) -> Option<&'static str> {
    if melee {
        return Some("close");
    }
    Some(match class? {
        GunClass::Pistol | GunClass::Smg | GunClass::Shotgun | GunClass::Flamethrower | GunClass::HeavyPistol => "close",
        GunClass::Carbine | GunClass::Rifle | GunClass::Heavy | GunClass::Launcher | GunClass::Grenade => "mid",
        GunClass::Marksman | GunClass::Sniper => "long",
    })
}

fn hook<'a>(d: &'a WeaponData, suffix: &str) -> Option<&'a toml::Table> {
    d.hooks.iter().find(|(k, _)| k.ends_with(suffix)).and_then(|(_, v)| v.as_table())
}

fn any_key(t: &toml::Table, pred: &dyn Fn(&str) -> bool) -> bool {
    t.iter().any(|(k, v)| pred(k) || v.as_table().is_some_and(|t| any_key(t, pred)))
}

/// A gun's global tags, its game's facets and stats from its data
/// (module doc); `class` is its part's class when the games' parts say
/// one (else the data's).
pub fn derive_tags(id: &str, d: Option<&WeaponData>, class: Option<GunClass>) -> (BTreeSet<Tag>, Vec<String>, GunStats) {
    let mut t: BTreeSet<Tag> = BTreeSet::new();
    let mut facets = Vec::new();
    let mut stats = GunStats::default();
    let class = class.or_else(|| d.and_then(|d| d.class));
    if let Some(w) = type_tag(class) {
        t.insert((TagGroup::Type, w));
    }
    if let Some(d) = d {
        let projectile = hook(d, ":projectile").is_some() || hook(d, "halo_ce:weapon").is_some_and(|h| h.contains_key("projectile"));
        let device = hook(d, ":device").is_some();
        let melee_only = d.melee.is_some() && d.hitscan.is_none() && !projectile;
        if device {
            t.insert((TagGroup::Type, "placeable"));
            t.insert((TagGroup::Delivery, "placed"));
        } else if class == Some(GunClass::Grenade) {
            t.insert((TagGroup::Delivery, "thrown"));
        } else if class == Some(GunClass::Flamethrower) {
            t.insert((TagGroup::Delivery, "stream"));
        } else if projectile {
            t.insert((TagGroup::Delivery, "projectile"));
        } else if d.hitscan.is_some() {
            t.insert((TagGroup::Delivery, "hitscan"));
        } else if melee_only {
            t.insert((TagGroup::Delivery, "melee"));
            t.insert((TagGroup::Type, "melee"));
        }
        if let Some(tr) = &d.trigger
            && !melee_only
        {
            t.insert((TagGroup::FireMode, if tr.automatic { "auto" } else { "semi" }));
            if tr.cycle > 0.0 {
                stats.fire_rate = Some((60.0 / tr.cycle).round());
            }
        }
        let heat = hook(d, "halo_ce:weapon").and_then(|h| h.get("heat_per_round")).and_then(|v| v.as_float().or(v.as_integer().map(|i| i as f64)));
        if heat.is_some_and(|h| h > 0.0) {
            t.insert((TagGroup::Reload, "overheat"));
        } else if let Some(m) = &d.magazine {
            stats.magazine = Some(m.clip);
            if m.shells.is_some() {
                t.insert((TagGroup::Reload, "shell by shell"));
                t.insert((TagGroup::FireMode, "pump or bolt"));
            } else if m.clip > 0 {
                t.insert((TagGroup::Reload, "magazine"));
            }
        }
        if hook(d, ":zoom").is_some() || d.alt.is_some() {
            t.insert((TagGroup::Optics, if matches!(class, Some(GunClass::Sniper | GunClass::Marksman)) { "scope" } else { "zoom" }));
        }
        if let Some(dmg) = &d.damage {
            stats.damage = Some(dmg.amount);
            stats.head_mult = Some(dmg.hitgroups.head);
            if dmg.hitgroups.head > 1.0 {
                t.insert((TagGroup::Behaviour, "headshot"));
            }
        } else if let Some(m) = &d.melee {
            stats.damage = Some(m.primary.damage);
        }
        let explosive = matches!(class, Some(GunClass::Launcher | GunClass::Grenade))
            || d.hooks.values().filter_map(|v| v.as_table()).any(|h| any_key(h, &|k| k.contains("explo") || k == "blast_radius" || k == "splash_radius"));
        if explosive {
            t.insert((TagGroup::Behaviour, "explosive"));
            t.insert((TagGroup::Behaviour, "splash"));
            t.insert((TagGroup::Behaviour, "self-damage"));
        }
        stats.price = d.buy.as_ref().map(|b| b.price).filter(|p| *p > 0);
        // Team Fortress 2: the classes that carry it.
        if let Some(arms) = hook(d, "tf2:gun").and_then(|h| h.get("arms")) {
            let mut add = |s: &str| {
                let mut c = s.chars();
                facets.push(c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default());
            };
            match arms {
                toml::Value::String(s) => add(s),
                toml::Value::Array(a) => a.iter().filter_map(|v| v.as_str()).for_each(add),
                _ => {}
            }
        }
        stats.range_feel = range_feel(class, melee_only);
    } else {
        stats.range_feel = range_feel(class, false);
        if matches!(class, Some(GunClass::Launcher | GunClass::Grenade)) {
            t.insert((TagGroup::Behaviour, "explosive"));
            t.insert((TagGroup::Behaviour, "splash"));
        }
    }
    if let Some(r) = stats.range_feel {
        t.insert((TagGroup::RangeFeel, r));
    }
    for (gid, tags) in HAND_TAGS {
        if *gid == id {
            t.extend(tags.iter().copied());
        }
    }
    (t, facets, stats)
}

/// Where a gun's files are (the Files group, from the download state).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Files {
    Here,
    NotHere,
    Downloading,
}

impl Files {
    pub fn tag(self) -> &'static str {
        match self {
            Files::Here => "here",
            Files::NotHere => "not here",
            Files::Downloading => "downloading",
        }
    }
}

/// The Change gun view's state (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GunView {
    /// Games ticked in the left column (empty: all games).
    pub games: BTreeSet<String>,
    /// Each game's facets ticked (game id → facet words).
    pub facets: BTreeMap<String, BTreeSet<String>>,
    /// Global tags ticked (AND).
    pub tags: BTreeSet<Tag>,
    pub search: String,
    /// The cursor's gun (an id).
    pub selected: Option<String>,
    /// The tag popover is open.
    pub tags_open: bool,
    /// The detail pane is open (I or Enter on nothing pinned; module
    /// doc of `change_gun`).
    pub detail: bool,
}

impl GunView {
    fn words(&self) -> Vec<String> {
        self.search.split_whitespace().map(str::to_lowercase).collect()
    }

    /// Whether `g` passes everything but the game ticks (the game
    /// column's counts use it).
    pub fn fits_but_games(&self, g: &SandboxGun, files: Files) -> bool {
        if let Some(f) = self.facets.get(&g.game)
            && !f.iter().all(|w| g.game_tags.iter().any(|t| t == w))
        {
            return false;
        }
        for &(group, word) in &self.tags {
            let has = if group == TagGroup::Files { files.tag() == word } else { g.tags.contains(&(group, word)) };
            if !has {
                return false;
            }
        }
        let words = self.words();
        if words.is_empty() {
            return true;
        }
        let hay = format!(
            "{} {} {} {} {} {} {}",
            g.name,
            g.id,
            g.game_name,
            g.game_short,
            section_name(g.class),
            g.tags.iter().map(|t| t.1).collect::<Vec<_>>().join(" "),
            g.game_tags.join(" ")
        )
        .to_lowercase();
        words.iter().all(|w| hay.contains(w.as_str()))
    }

    pub fn fits(&self, g: &SandboxGun, files: Files) -> bool {
        (self.games.is_empty() || self.games.contains(&g.game)) && self.fits_but_games(g, files)
    }

    /// What the grid lists: (section name, the guns' indices into `guns`),
    /// sections in `SECTIONS` order, empty ones left out.
    pub fn shown(&self, guns: &[SandboxGun], files: &dyn Fn(&SandboxGun) -> Files) -> Vec<(&'static str, Vec<usize>)> {
        let mut out: Vec<(&'static str, Vec<usize>)> = SECTIONS.iter().map(|s| (*s, Vec::new())).collect();
        for (i, g) in guns.iter().enumerate() {
            if self.fits(g, files(g)) {
                let s = section_name(g.class);
                if let Some(sec) = out.iter_mut().find(|(n, _)| *n == s) {
                    sec.1.push(i);
                }
            }
        }
        out.retain(|(_, v)| !v.is_empty());
        out
    }

    /// Ticks or unticks a game in the left column ("All games": `None`
    /// clears them).
    pub fn tick_game(&mut self, game: Option<&str>) {
        match game {
            None => self.games.clear(),
            Some(g) => {
                if !self.games.remove(g) {
                    self.games.insert(g.to_string());
                }
            }
        }
    }

    pub fn tick_facet(&mut self, game: &str, facet: &str) {
        let f = self.facets.entry(game.to_string()).or_default();
        if !f.remove(facet) {
            f.insert(facet.to_string());
        }
        if f.is_empty() {
            self.facets.remove(game);
        }
    }

    pub fn tick_tag(&mut self, tag: Tag) {
        if !self.tags.remove(&tag) {
            self.tags.insert(tag);
        }
    }
}

/// A game's facets across the catalogue (its guns' `game_tags`), sorted.
pub fn facets_of(guns: &[SandboxGun], game: &str) -> Vec<String> {
    let set: BTreeSet<&String> = guns.iter().filter(|g| g.game == game).flat_map(|g| g.game_tags.iter()).collect();
    set.into_iter().cloned().collect()
}

/// The pinned slots' count (keys 1 to 6).
pub const PIN_SLOTS: usize = 6;

/// Seconds of idle before the pinned bar fades.
pub const PIN_IDLE: f64 = 4.0;

/// The pinned guns (module doc).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pins(pub [Option<String>; PIN_SLOTS]);

impl Pins {
    /// Pins `gun` in `slot` (0-based), moving it if pinned elsewhere.
    pub fn pin(&mut self, slot: usize, gun: &str) {
        if slot >= PIN_SLOTS {
            return;
        }
        for s in self.0.iter_mut() {
            if s.as_deref() == Some(gun) {
                *s = None;
            }
        }
        self.0[slot] = Some(gun.to_string());
    }

    pub fn unpin(&mut self, slot: usize) {
        if let Some(s) = self.0.get_mut(slot) {
            *s = None;
        }
    }

    /// The slot `gun` is pinned in.
    pub fn slot_of(&self, gun: &str) -> Option<usize> {
        self.0.iter().position(|s| s.as_deref() == Some(gun))
    }

    pub fn any(&self) -> bool {
        self.0.iter().any(Option::is_some)
    }
}

/// The pinned bar's showing (module doc): when it was last woken.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PinBar {
    pub woke: Option<f64>,
}

impl PinBar {
    pub fn wake(&mut self, now: f64) {
        self.woke = Some(now);
    }

    /// Its opacity at `now`: 1 while shown, fading over `fade` s after
    /// `PIN_IDLE` s idle; `held` (Tab, a drag) keeps it up.
    pub fn alpha(&self, now: f64, held: bool, fade: f64) -> f32 {
        if held {
            return 1.0;
        }
        let Some(w) = self.woke else { return 0.0 };
        let idle = now - w - PIN_IDLE;
        if idle <= 0.0 {
            1.0
        } else {
            (1.0 - idle / fade.max(1e-3)).clamp(0.0, 1.0) as f32
        }
    }
}
