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
//!   A gun with no `[damage]` (its round the hook's: Team Fortress 2's
//!   rocket, grenades, flame) shows its hook's base `damage` and, for an
//!   explosive, its `radius` (`hook_damage`). TF2's
//!   classes that carry a gun are its loadouts' (the tf2 game's
//!   `classes.toml`, which this layer can't read): only `arms` comes
//!   through, so a gun several classes carry needs that list handed over.
//! - **The view's state** (`GunView`): the games ticked in the left column
//!   (none: all), each game's facets ticked under it, the global tags
//!   ticked in the popover, the search words, the cursor's gun (`selected`)
//!   and whether the tag popover is open, with its own search
//!   (`tag_search`, `tags_found`). `shown` is what the tile grid
//!   lists, by class section in the classes' order, each section's guns in
//!   the ladder's order: a gun shows when its game is ticked (or none
//!   is), it has every facet ticked for its game, every global tag ticked
//!   (AND inside the list) and every search word is in its name, id, game,
//!   class or tags.
//! - **Families** (`set_families`, `SandboxGun::family`): the variants of
//!   one gun (Combat Arms' about 2,000 weapons are skins, editions and
//!   levels of some tens of guns) share a family, named by its base gun's
//!   id, and the grid shows a family as one tile (`GunView::tiles`: its
//!   face the base gun, or the first variant the filters leave, with how
//!   many they leave; the detail pane lists them to pick one). Two guns
//!   of one game and one class section are one family when they share
//!   their first-person model (the weapon data's `models.view`: a skin
//!   is the same model with other textures) or, in a game whose names
//!   are a base plus a variant (its display_names.toml says `split`:
//!   Combat Arms), their name's model (the display names' split,
//!   `weapon::names::split`: `M4A1` in "Midnight Warden M4A1 Lv.4"; an
//!   edition with a model file of its own), or when one's base name
//!   holds the other's whole, as words in a row ("Fiery Dragon AK 47"
//!   and "AK 47"; never Halo's Pistol and Plasma Pistol: no split). The
//!   base gun is the family's shortest name (the earliest rung among
//!   equals). Every variant stays a gun of the catalogue: a pin, the
//!   search and the console's `rung <id>` take any of them.
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
    /// An explosive's blast radius, metres (its hook's `radius`).
    pub blast_radius: Option<f32>,
}

/// Metres in a Source unit (an inch): the hooks' `radius` is in them.
pub const SOURCE_UNIT_M: f32 = 0.0254;

fn num(v: &toml::Value) -> Option<f32> {
    v.as_float().map(|f| f as f32).or_else(|| v.as_integer().map(|i| i as f32))
}

/// The damage a hook says (module doc): a gun whose round is the hook's
/// own (Team Fortress 2's rocket, grenade, sticky, flame, needle: no
/// `[damage]` in its data) carries its base `damage` there, and an
/// explosive its `radius` (Source units) too. The first hook, in the
/// data's order, that says `damage`.
pub fn hook_damage(d: &WeaponData) -> Option<(f32, Option<f32>)> {
    d.hooks.values().filter_map(|v| v.as_table()).find_map(|h| {
        let dmg = h.get("damage").and_then(num).filter(|v| *v > 0.0)?;
        Some((dmg, h.get("radius").and_then(num).filter(|r| *r > 0.0).map(|r| r * SOURCE_UNIT_M)))
    })
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
    /// Its family: the id of the base gun its variants share (module doc,
    /// "Families"; its own id when it has none, or before `set_families`).
    pub family: String,
}

/// A name's model as a family key (module doc, "Families"): the split's
/// model words, letters and digits only, lower case (None: no model).
pub fn name_model(name: &str, words: &chimera_sim::weapon::names::NameWords) -> Option<String> {
    let split = chimera_sim::weapon::names::split(name, words);
    let key: String =
        split.base.iter().take(split.model_words).flat_map(|w| w.chars()).filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect();
    (!key.is_empty()).then_some(key)
}

/// Sets every gun's `family` (module doc, "Families"). `view_models[i]` is
/// gun `i`'s first-person model file (None: unknown); `splits(game)` says
/// whether a game's names are a base plus a variant.
pub fn set_families(
    guns: &mut [SandboxGun],
    view_models: &[Option<String>],
    words: &chimera_sim::weapon::names::NameWords,
    splits: &dyn Fn(&str) -> bool,
) {
    // Union-find over the guns, joined through shared keys.
    let mut parent: Vec<usize> = (0..guns.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut by_key: BTreeMap<(String, &'static str, u8, String), usize> = BTreeMap::new();
    for i in 0..guns.len() {
        let g = &guns[i];
        let section = section_of(g);
        let keys = [
            view_models.get(i).cloned().flatten().filter(|m| !m.is_empty()).map(|m| (0u8, m.to_lowercase())),
            name_model(&g.name, words).filter(|_| splits(&g.game)).map(|m| (1u8, m)),
        ];
        for (kind, key) in keys.into_iter().flatten() {
            match by_key.entry((g.game.clone(), section, kind, key)) {
                std::collections::btree_map::Entry::Vacant(e) => {
                    e.insert(i);
                }
                std::collections::btree_map::Entry::Occupied(e) => {
                    let (a, b) = (root(&mut parent, *e.get()), root(&mut parent, i));
                    parent[a.max(b)] = a.min(b);
                }
            }
        }
    }
    // Each family's base: its shortest name, the earliest rung among equals.
    let bases = |parent: &mut Vec<usize>| {
        let mut base: BTreeMap<usize, usize> = BTreeMap::new();
        for i in 0..guns.len() {
            let r = root(parent, i);
            let b = base.entry(r).or_insert(i);
            let (now, new) = (&guns[*b], &guns[i]);
            if (new.name.trim().len(), new.rung) < (now.name.trim().len(), now.rung) {
                *b = i;
            }
        }
        base
    };
    // A family whose base's name holds another's whole (as words in a
    // row: "Fiery Dragon AK 47" holds "AK 47", "Deserteagle Canada"
    // holds "Deserteagle"; not "G36C" for "G3") is that gun's too.
    let base = bases(&mut parent);
    let named: Vec<(usize, Vec<String>)> =
        base.values().map(|&b| (b, guns[b].name.split_whitespace().map(str::to_lowercase).collect())).filter(|(_, w): &(usize, Vec<String>)| !w.is_empty()).collect();
    for (a, short) in &named {
        for (b, long) in &named {
            let same_shelf = guns[*a].game == guns[*b].game && splits(&guns[*a].game) && section_of(&guns[*a]) == section_of(&guns[*b]);
            if !same_shelf || long.len() <= short.len() || !long.windows(short.len()).any(|w| w == short.as_slice()) {
                continue;
            }
            let (ra, rb) = (root(&mut parent, *a), root(&mut parent, *b));
            if ra != rb {
                parent[ra.max(rb)] = ra.min(rb);
            }
        }
    }
    let base = bases(&mut parent);
    for i in 0..guns.len() {
        let r = root(&mut parent, i);
        guns[i].family = guns[base[&r]].id.clone();
    }
}

/// A tile of the grid (module doc, "Families"): a gun, or a family of
/// variants as one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tile {
    /// The gun the tile shows (an index into the catalogue).
    pub face: usize,
    /// The family's guns the filters leave, the face among them, in the
    /// ladder's order (one: a gun alone).
    pub variants: Vec<usize>,
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
        None => "Other",
    }
}

/// A gun's section: its class's, or Melee for a melee weapon (no class,
/// a swing: TF2's bat, a knife).
pub fn section_of(g: &SandboxGun) -> &'static str {
    if g.class.is_none() && g.tags.contains(&(TagGroup::Type, "melee")) { "Melee" } else { section_name(g.class) }
}

/// The sections' order.
pub const SECTIONS: [&str; 9] =
    ["Pistols", "SMGs", "Rifles and MGs", "Shotguns", "Sniper rifles", "Launchers and heavy", "Grenades", "Melee", "Other"];

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
        let melee_only = d.is_melee() && d.hitscan.is_none() && !projectile;
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
        } else if let Some(m) = d.melee.as_ref().and_then(|m| m.primary.as_ref()) {
            stats.damage = Some(m.damage);
        } else if let Some((dmg, radius)) = hook_damage(d) {
            stats.damage = Some(dmg);
            stats.blast_radius = radius;
        }
        let explosive = matches!(class, Some(GunClass::Launcher | GunClass::Grenade))
            || stats.blast_radius.is_some()
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
    /// The search words applied (the picker's search text, which owns the
    /// field, its caret and its focus: `change_gun::GunPicker`; `sync_grid`
    /// copies it here so the tiles and the counts follow it).
    pub search: String,
    /// The cursor's gun (an id).
    pub selected: Option<String>,
    /// The tag popover is open.
    pub tags_open: bool,
    /// The popover's own search ("Find a tag": typing goes there while
    /// it is open).
    pub tag_search: String,
    /// The large model view (I).
    pub detail: bool,
    /// The body to play a gun as (`tf2:soldier`), chosen in the detail
    /// pane for a gun several of its game's classes carry.
    pub body: Option<String>,
    /// The tile `\\` folded its class from (section, item): opening the
    /// class again goes back to it.
    pub fold_return: Option<(usize, usize)>,
}

/// What the search matches of a gun: its name, id, game (long and short),
/// section, tags and its game's tags. The picker's cards carry it
/// (`PickerItem::search_text`), so the picker's search matches as the
/// screen's always did.
pub fn search_text(g: &SandboxGun) -> String {
    format!(
        "{} {} {} {} {} {} {}",
        g.name,
        g.id,
        g.game_name,
        g.game_short,
        section_of(g),
        g.tags.iter().map(|t| t.1).collect::<Vec<_>>().join(" "),
        g.game_tags.join(" ")
    )
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
        let hay = search_text(g).to_lowercase();
        words.iter().all(|w| hay.contains(w.as_str()))
    }

    pub fn fits(&self, g: &SandboxGun, files: Files) -> bool {
        (self.games.is_empty() || self.games.contains(&g.game)) && self.fits_but_games(g, files)
    }

    /// What the grid lists: (section name, its tiles' faces as indices
    /// into `guns`), sections in `SECTIONS` order, empty ones left out
    /// (`tiles`, without the variants).
    pub fn shown(&self, guns: &[SandboxGun], files: &dyn Fn(&SandboxGun) -> Files) -> Vec<(&'static str, Vec<usize>)> {
        self.tiles(guns, files).into_iter().map(|(s, t)| (s, t.into_iter().map(|t| t.face).collect())).collect()
    }

    /// The grid's tiles by section (module doc, "Families"): a family's
    /// guns that fit are one tile, where its first one stands in the
    /// ladder's order; its face is the family's base gun when that fits,
    /// else the first that does.
    pub fn tiles(&self, guns: &[SandboxGun], files: &dyn Fn(&SandboxGun) -> Files) -> Vec<(&'static str, Vec<Tile>)> {
        let mut out: Vec<(&'static str, Vec<Tile>)> = SECTIONS.iter().map(|s| (*s, Vec::new())).collect();
        // (section, family) -> its tile's place in the section.
        let mut at: BTreeMap<(usize, &str), usize> = BTreeMap::new();
        for (i, g) in guns.iter().enumerate() {
            if !self.fits(g, files(g)) {
                continue;
            }
            let s = section_of(g);
            let Some(sec) = out.iter().position(|(n, _)| *n == s) else { continue };
            let family = if g.family.is_empty() { g.id.as_str() } else { g.family.as_str() };
            match at.get(&(sec, family)) {
                Some(&t) => {
                    let tile = &mut out[sec].1[t];
                    tile.variants.push(i);
                    if g.id == family {
                        tile.face = i;
                    }
                }
                None => {
                    at.insert((sec, family), out[sec].1.len());
                    out[sec].1.push(Tile { face: i, variants: vec![i] });
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

    /// The popover's tags by group that hold every word of
    /// `tag_search` (in a tag or its group's label), groups with none
    /// left out.
    pub fn tags_found(&self) -> Vec<(TagGroup, Vec<&'static str>)> {
        let words: Vec<String> = self.tag_search.split_whitespace().map(str::to_lowercase).collect();
        TagGroup::ALL
            .iter()
            .filter_map(|g| {
                let label = g.label().to_lowercase();
                let tags: Vec<&'static str> =
                    g.tags().iter().copied().filter(|t| words.iter().all(|w| t.contains(w.as_str()) || label.contains(w.as_str()))).collect();
                (!tags.is_empty()).then_some((*g, tags))
            })
            .collect()
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

/// The drop hint beside the pinned bar while a gun is dragged over slot
/// `slot` (0-based), `old` the gun pinned there: "Release: Rocket Launcher
/// to 2, replacing M4A1".
pub fn drop_hint(gun: &str, slot: usize, old: Option<&str>) -> String {
    match old {
        Some(o) if o != gun => format!("Release: {gun} to {}, replacing {o}", slot + 1),
        _ => format!("Release: {gun} to {}", slot + 1),
    }
}
