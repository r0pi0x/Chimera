//! An automatic gun game from whatever games an app composed
//! (docs/plans/active/ultimate-gun-game.md): each game's claim of its parts
//! (`chimera_core::game_parts`) checked against the app (`report`), a
//! ladder spread across the games' guns (`ladder`), a map pool of every
//! game's maps (`map_pool`) and the definition text that plays them
//! (`definition`) on `chimera:ladder` with `identity = "weapon"`.
//!
//! - `report`: per registered game, what it really lends here: its guns
//!   that are in the weapon catalogue (claimed, and every other weapon of
//!   its own whose data names a class: its install's weapons too), its
//!   grenades that kill (class `grenade`), its knife if there, its
//!   movement if registered, its claimed characters and HUD, and its
//!   maps (claimed, and the installed ones its pool rules don't deny:
//!   `map_fit::MapPool::listed`, a game's data/map_pool.toml;
//!   `InstalledMaps`, found once by `find_installed_maps` in an app that
//!   wants them, for the games with rules). A game is in a ladder with guns and movement, unless a scaffold
//!   (`GameParts::scaffold`; `Capabilities::playable`); the rest falls back (`fallbacks`): no
//!   HUD, `PartsRegistry::fallback_hud`; no characters, the shell's
//!   capsule (`map::CharacterModels::for_identity` finds none of its
//!   game); no maps, the other games'.
//! - `ladder`: `rungs` rungs in all, seeded and deterministic (SplitMix64;
//!   no hash maps), on the power curve (`minigame::power`): the rungs
//!   split over the bands (warm-up, rising, peak, heavy, specials,
//!   grenade, melee) by the curve's shares, boundaries jittered; each
//!   rung dealt a game and a gun of it (`DrawParams`, `[draw]` in
//!   data/power_curve.toml): the games shuffled and dealt one per rung,
//!   reshuffled once dealt out (equal shares; no game promised a rung;
//!   `games = "sqrt"` or `"count"` weight by gun count instead); within
//!   the game a random gun, the ones dealt in the server's recent ladders
//!   less likely (`Recency`, `recency`, `recovery`; never excluded, never
//!   across games), sorted by power score, weakest first; the melee band
//!   counted for the knife finale, which draws its weapons when the match
//!   starts.
//!   Two seeds share few rungs, and every gun comes up over many.
//! - `drawable`: what a draw may deal (the report's guns and grenades,
//!   filtered before `ladder`, by `ladder = "auto"` and the Ultimate Gun
//!   Game alike): not a weapon that can't damage a player (its data's
//!   damage paths all 0: `weapon::harm`; Combat Arms' RMS12, M136_AT4),
//!   nor one a game's data excludes (`DrawExclusions`, the no-player ones
//!   also out of the sandbox, `drop_not_players`; each with its
//!   reason: Combat Arms' skins drawn wrong); `skipped_lines` says which
//!   (logged when a ladder is drawn; the `ladder_skipped` command).
//!   Hand-written ladders may name them.
//! - `draw_pool`: `drawable_all`, the games some player lacks, then the
//!   host's Guns rules (`gun_rules`: bans, allowed, games, classes by
//!   `effective_class`, families by `family_of`); `ladder_rules` draws
//!   with them (`DrawRules`: a family one candidate, `WhenShort`).
//! - `map_pool`: every map of every game whose install the players have,
//!   the games in turn; none: the greybox. `map_pool_for` takes the
//!   player count too: only the maps `map_fit` lets in at that count. The
//!   definition's pool is every map (a rotation draws by the count at each
//!   change, `map_fit = true`: on by default with its random rotation).
//! - `definition`: the minigame's TOML (`MinigameDef`), filtered by
//!   `installed` (the games every player has: L3's mount reports, which
//!   the app reads; `chimera_net::minigame_net::installed_by_all`). Its
//!   kit is by game (`def::Kit::ByGame`): each gun's game's own knife,
//!   none for a game that lends none (Halo's melee is the held gun's
//!   bash: a slot key never hands its player another game's knife, or
//!   that game's body with it); its rotation random but
//!   the last maps played when `AutoOptions::avoid_recent` says
//!   (`minigame::rotation`).

use chimera_core::{
    game_parts::{GameParts, GunClass, GunPart, PartsRegistry},
    slots::MovementRegistry,
};

use bevy::prelude::*;

use super::gun_rules::WhenShort;
use super::power::{self, Band, DrawParams, GameWeighting, PowerCurve, PowerReaders, Powers};
use crate::weapon::data::WeaponCatalog;

/// What a game really lends in this app: its claim (`GameParts`) checked
/// against the weapon catalogue and the movement registry.
#[derive(Clone, Debug, PartialEq)]
pub struct Capabilities {
    pub game: &'static str,
    /// Its guns: the claimed ones and every other weapon of its own in
    /// the catalogue whose data names a gun class.
    pub guns: Vec<GunPart>,
    /// Its grenades that kill (class `Grenade`, claimed or in its data).
    pub grenades: Vec<&'static str>,
    pub knife: Option<&'static str>,
    pub movement: Option<&'static str>,
    pub characters: Option<&'static str>,
    pub hud: Option<&'static str>,
    /// Its claimed maps, then its installed maps (those its pool rules
    /// allow: `report`).
    pub maps: Vec<String>,
    /// Claimed parts the app doesn't have (a gun not in the catalogue, an
    /// unregistered movement), in words.
    pub missing: Vec<String>,
    /// Its parts are a scaffold's stand-ins (`GameParts::scaffold`): not
    /// playable whatever it lends.
    pub scaffold: bool,
}

impl Capabilities {
    /// Guns and movement, and not a scaffold: in a ladder.
    pub fn playable(&self) -> bool {
        !self.guns.is_empty() && self.movement.is_some() && !self.scaffold
    }

    /// `parts` checked against `catalog` and `movements`.
    pub fn of(parts: &GameParts, catalog: &WeaponCatalog, movements: Option<&MovementRegistry>) -> Self {
        Self::with_maps(parts, catalog, movements, &[])
    }

    /// `of`, with the maps of its install that join its pool
    /// (`InstalledMaps::of`, filtered by its rules: `report`).
    pub fn with_maps(
        parts: &GameParts,
        catalog: &WeaponCatalog,
        movements: Option<&MovementRegistry>,
        installed: &[String],
    ) -> Self {
        let mut missing = Vec::new();
        let mut all: Vec<GunPart> = parts
            .guns
            .iter()
            .filter(|g| {
                let ok = catalog.get(g.id).is_some();
                if !ok {
                    missing.push(format!("gun {} isn't in the weapon catalogue", g.id));
                }
                ok
            })
            .copied()
            .collect();
        // Its other weapons with a class in their data (its install's).
        let prefix = format!("{}:", parts.game);
        let claimed: bevy::platform::collections::HashSet<&'static str> = all.iter().map(|g| g.id).collect();
        for data in catalog.weapons().iter() {
            if !data.id.starts_with(&prefix) || claimed.contains(data.id.as_str()) {
                continue;
            }
            if let Some(class) = data.class {
                all.push(GunPart { id: &data.id, class });
            }
        }
        let grenades = all.iter().filter(|g| g.class == GunClass::Grenade).map(|g| g.id).collect();
        let guns = all.into_iter().filter(|g| g.class != GunClass::Grenade).collect();
        let knife = parts.knife.filter(|k| catalog.get(k).is_some());
        let movement = parts.movement.filter(|m| {
            let ok = movements.is_some_and(|r| r.get(m).is_some());
            if !ok {
                missing.push(format!("movement {m} isn't registered"));
            }
            ok
        });
        let mut maps: Vec<String> = parts.maps.iter().map(|m| m.to_string()).collect();
        for name in installed {
            let id = format!("{}:{}", parts.game, name.to_ascii_lowercase());
            if !maps.contains(&id) {
                maps.push(id);
            }
        }
        Self {
            game: parts.game,
            guns,
            grenades,
            knife,
            movement,
            characters: parts.characters,
            hud: parts.hud,
            maps,
            missing,
            scaffold: parts.scaffold,
        }
    }

    /// One line for the console (`parts`).
    pub fn line(&self) -> String {
        let or = |o: Option<&str>, fallback: &str| o.map_or(format!("none ({fallback})"), str::to_string);
        format!(
            "{}: {} guns, {} grenades, knife {}, movement {}, characters {}, HUD {}, maps {}{}{}",
            self.game,
            self.guns.len(),
            self.grenades.len(),
            or(self.knife, "no melee slot"),
            or(self.movement, "not in a ladder"),
            or(self.characters, "the stand-in body"),
            or(self.hud, "the fallback HUD"),
            if self.maps.is_empty() { "none (other games')".to_string() } else { self.maps.join(", ") },
            if self.playable() {
                ""
            } else if self.scaffold {
                "; not playable (a scaffold)"
            } else {
                "; not playable"
            },
            if self.missing.is_empty() { String::new() } else { format!("; missing: {}", self.missing.join("; ")) },
        )
    }
}

/// Every registered game's capabilities here, in registration order.
pub fn report(world: &World) -> Vec<Capabilities> {
    let Some(registry) = world.get_resource::<PartsRegistry>() else {
        return Vec::new();
    };
    let empty = WeaponCatalog::default();
    let catalog = world.get_resource::<WeaponCatalog>().unwrap_or(&empty);
    let movements = world.get_resource::<MovementRegistry>();
    let installed = world.get_resource::<InstalledMaps>();
    let none = super::map_fit::MapPool::default();
    let pool = world.get_resource::<super::map_fit::MapPool>().unwrap_or(&none);
    registry
        .all()
        .map(|p| {
            let maps: Vec<String> = installed
                .map_or(&[][..], |i| i.of(p.game))
                .iter()
                .filter(|n| pool.has_rules(p.game) && pool.listed(p.game, n).is_ok())
                .cloned()
                .collect();
            Capabilities::with_maps(p, catalog, movements, &maps)
        })
        .collect()
}

/// Weapons a drawn ladder never deals, with why (`DrawExclusionsAppExt`:
/// each game's data, e.g. Combat Arms' data/ladder_exclusions.toml). A
/// hand-written ladder may still name them. Kept in
/// `power::PowerReaders::exclusions`, not a resource of its own.
///
/// The second list holds the ids (lower case) that are no player's at
/// all (an NPC's, a boss's, a turret, a test record: `[[exclude]]
/// not_player = true`, or what a game's loader finds by its data, e.g.
/// Combat Arms' weapons without a shop item): they are left out of the
/// sandbox's Change gun list too (`drop_not_players`), where the other
/// exclusions' guns (a skin drawn wrong, a stun launcher) stay.
#[derive(Clone, Debug, Default)]
pub struct DrawExclusions(pub Vec<(String, String)>, pub std::collections::BTreeSet<String>);

impl DrawExclusions {
    /// Whether `id` is no player's (`not_player`).
    pub fn not_player(&self, id: &str) -> bool {
        self.1.contains(&id.to_ascii_lowercase())
    }

    /// Why `id` is left out (None: it isn't).
    pub fn reason(&self, id: &str) -> Option<&str> {
        self.0.iter().find(|(w, _)| w.eq_ignore_ascii_case(id)).map(|(_, r)| r.as_str())
    }
}

/// A list of draw exclusions as data: `[[exclude]]` tables of `weapons`
/// (ids) and their `reason`.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ExclusionsDoc {
    #[serde(default)]
    exclude: Vec<ExclusionDoc>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ExclusionDoc {
    weapons: Vec<String>,
    reason: String,
    /// The records are no player's (`DrawExclusions`' second list).
    #[serde(default)]
    not_player: bool,
}

/// `text` (a game's draw exclusions, `[[exclude]] weapons = [..] reason
/// = ".."`) read.
pub fn parse_exclusions(text: &str) -> Result<Vec<(String, String)>, String> {
    let doc: ExclusionsDoc = toml::from_str(text).map_err(|e| e.to_string())?;
    Ok(doc
        .exclude
        .into_iter()
        .flat_map(|e| e.weapons.into_iter().map(move |w| (w, e.reason.clone())))
        .collect())
}

/// The ids of `text`'s exclusions that are `not_player`.
pub fn parse_not_players(text: &str) -> Result<Vec<String>, String> {
    let doc: ExclusionsDoc = toml::from_str(text).map_err(|e| e.to_string())?;
    Ok(doc.exclude.into_iter().filter(|e| e.not_player).flat_map(|e| e.weapons).collect())
}

pub trait DrawExclusionsAppExt {
    /// The exclusions of `text` (`parse_exclusions`; `name` for the error,
    /// which panics: shipped data).
    fn add_draw_exclusions(&mut self, name: &str, text: &str) -> &mut Self;
}

impl DrawExclusionsAppExt for App {
    fn add_draw_exclusions(&mut self, name: &str, text: &str) -> &mut Self {
        let list = parse_exclusions(text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let nobody = parse_not_players(text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut readers = self.world_mut().get_resource_or_init::<PowerReaders>();
        readers.exclusions.0.extend(list);
        readers.exclusions.1.extend(nobody.into_iter().map(|id| id.to_ascii_lowercase()));
        self
    }
}

/// Why a drawn ladder skipped a weapon (`drawable`).
#[derive(Clone, Debug, PartialEq)]
pub struct Skipped {
    pub id: &'static str,
    /// "no damage" (`weapon::harm`), or the exclusion's reason.
    pub why: String,
}

/// What a drawn ladder may deal: `caps`' guns and grenades without those
/// that can't damage a player (their data says so, `weapon::harm::
/// harmless`) or a game's data excludes (`DrawExclusions`), then those
/// the host took out (`GunsOff`); the skipped ones, in report order.
pub fn drawable(world: &World, caps: &mut [Capabilities]) -> Vec<Skipped> {
    let mut skipped = drawable_all(world, caps);
    // The host's guns off (`mp_guns_off`, the Host page's Guns picker),
    // unless that would leave no gun.
    if let Some(off) = world.get_resource::<GunsOff>().filter(|o| !o.0.trim().is_empty())
        && caps.iter().flat_map(|c| c.guns.iter()).any(|g| !off.has(g.id))
    {
        let mut out = |id: &'static str| {
            let gone = off.has(id);
            if gone {
                skipped.push(Skipped { id, why: GunsOff::WHY.to_string() });
            }
            !gone
        };
        for c in caps.iter_mut() {
            c.guns.retain(|g| out(g.id));
            c.grenades.retain(|g| out(g));
        }
    }
    skipped
}

/// `caps` without the guns and grenades that are no player's
/// (`DrawExclusions::not_player`: NPC, boss, turret and test records);
/// the sandbox's Change gun list keeps every other gun.
pub fn drop_not_players(world: &World, caps: &mut [Capabilities]) {
    let Some(readers) = world.get_resource::<PowerReaders>().filter(|r| !r.exclusions.1.is_empty()) else { return };
    let nobody = &readers.exclusions;
    for c in caps.iter_mut() {
        c.guns.retain(|g| !nobody.not_player(g.id));
        c.grenades.retain(|g| !nobody.not_player(g));
    }
}

/// `drawable` before the host's guns off (`GunsOff`): what the Host
/// page's Guns picker lists.
pub fn drawable_all(world: &World, caps: &mut [Capabilities]) -> Vec<Skipped> {
    let empty = WeaponCatalog::default();
    let catalog = world.get_resource::<WeaponCatalog>().unwrap_or(&empty);
    let none = PowerReaders::default();
    let registered = world.get_resource::<PowerReaders>().unwrap_or(&none);
    let (readers, excluded) = (&registered.harm, &registered.exclusions);
    let mut skipped = Vec::new();
    // The exclusions by id (lower case), the first of an id as `reason`
    // finds it: thousands of guns are asked.
    let mut reasons: std::collections::BTreeMap<String, &str> = std::collections::BTreeMap::new();
    for (id, r) in &excluded.0 {
        reasons.entry(id.to_ascii_lowercase()).or_insert(r.as_str());
    }
    let mut why = |id: &'static str| -> Option<String> {
        let r = if let Some(r) = (!reasons.is_empty()).then(|| reasons.get(&id.to_ascii_lowercase())).flatten() {
            Some(r.to_string())
        } else if catalog.get(id).is_some_and(|d| crate::weapon::harm::harmless(d, readers)) {
            Some("no damage".to_string())
        } else {
            None
        };
        if let Some(w) = &r {
            skipped.push(Skipped { id, why: w.clone() });
        }
        r
    };
    for c in caps.iter_mut() {
        c.guns.retain(|g| why(g.id).is_none());
        c.grenades.retain(|g| why(g).is_none());
    }
    skipped
}

/// A gun's class for the host's rules: its data's, a pistol that hits
/// like a heavy pistol one (`power::band_of`).
pub fn effective_class(id: &str, class: GunClass, catalog: &WeaponCatalog, readers: &PowerReaders, curve: &PowerCurve) -> GunClass {
    if class == GunClass::Pistol && power::band_of(class, catalog.get(id), readers, curve) == curve.class_band(GunClass::HeavyPistol) {
        return GunClass::HeavyPistol;
    }
    class
}

/// A gun's family (`WeaponData::family`; its own id without one).
pub fn family_of(catalog: &WeaponCatalog, id: &str) -> String {
    catalog.get(id).and_then(|d| d.family.clone()).unwrap_or_else(|| id.to_string())
}

/// What a draw under the host's rules may deal (`gun_rules`): `caps`
/// without what is Unavailable (`drawable_all`: no damage, a game's data
/// exclusions; a game `installed` says some player lacks), then the
/// host's `GunRules` and `GunsOff` (`gun_rules::apply`), the families of
/// `mp_guns_skins one` for `DrawRules`.
pub fn draw_pool(world: &World, mut caps: Vec<Capabilities>, installed: &dyn Fn(&str) -> bool) -> (super::gun_rules::Pool, std::collections::BTreeMap<&'static str, String>) {
    use super::gun_rules::{self, GunRules, Skins};
    let mut skipped = drawable_all(world, &mut caps);
    for c in caps.iter_mut().filter(|c| !installed(c.game)) {
        for id in c.guns.drain(..).map(|g| g.id).chain(c.grenades.drain(..)) {
            skipped.push(Skipped { id, why: "a player lacks its game".to_string() });
        }
    }
    let empty = WeaponCatalog::default();
    let catalog = world.get_resource::<WeaponCatalog>().unwrap_or(&empty);
    let none = PowerReaders::default();
    let readers = world.get_resource::<PowerReaders>().unwrap_or(&none);
    let curve = PowerCurve::shipped();
    let rules = world.get_resource::<GunRules>().cloned().unwrap_or_default();
    let family = |id: &str| family_of(catalog, id);
    let class = |id: &str, c: GunClass| effective_class(id, c, catalog, readers, &curve);
    let legacy = world.get_resource::<GunsOff>().filter(|o| !o.0.trim().is_empty());
    let mut pool = gun_rules::apply(&rules, legacy, caps, &family, &class);
    skipped.append(&mut pool.skipped);
    pool.skipped = skipped;
    let mut families = std::collections::BTreeMap::new();
    if rules.skins == Skins::One {
        for c in pool.caps.iter().chain(&pool.reserve) {
            for id in c.guns.iter().map(|g| g.id).chain(c.grenades.iter().copied()) {
                if let Some(f) = catalog.get(id).and_then(|d| d.family.clone()) {
                    families.insert(id, f);
                }
            }
        }
    }
    (pool, families)
}

/// `mp_guns_off`: the guns and grenades a host took out of drawn ladders
/// (weapon ids, comma-separated; the Host page's Guns picker sets it).
/// `drawable` leaves them out, unless that would leave no gun; a
/// hand-written ladder may still name them.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct GunsOff(pub String);

impl GunsOff {
    /// The reason `drawable` gives.
    pub const WHY: &'static str = "the host left it out";

    /// Whether `id` is off (case aside).
    pub fn has(&self, id: &str) -> bool {
        self.0.split(',').map(str::trim).any(|w| !w.is_empty() && w.eq_ignore_ascii_case(id))
    }
}

/// Registers `mp_guns_off` (`GunsOff`).
pub fn guns_off_cvar(app: &mut App) {
    app.init_resource::<GunsOff>();
    crate::console::resource_cvar::<GunsOff, String>(
        app,
        "mp_guns_off",
        "Server: the guns drawn ladders leave out, comma-separated weapon ids (the Host page's Guns picker sets it). Taking every gun out takes none. From the next ladder.",
        |g| &mut g.0,
    );
}

/// One gun's names (`drawable_short_names`).
#[derive(Clone, Debug, PartialEq)]
pub struct GunNameRow {
    pub id: &'static str,
    pub full: String,
    pub short: String,
    pub from: crate::weapon::names::NameSource,
}

/// The names of every gun and grenade `parts`' game can deal on a drawn
/// ladder (`minigame::auto::drawable` in `world`): each one's full name
/// (`game_full`, the game's strings, else its data's plain `hud.name`,
/// else `id_name`, an override's `full` over them) and short name by the
/// world's `GunNameRules`. What each game's heavy test checks against
/// `weapon::names::SHORT_MAX`.
pub fn drawable_short_names(
    world: &World,
    parts: &GameParts,
    game_full: &std::collections::BTreeMap<String, String>,
) -> Vec<GunNameRow> {
    use crate::weapon::names::{GunNameRules, id_name};
    let empty = WeaponCatalog::default();
    let catalog = world.get_resource::<WeaponCatalog>().unwrap_or(&empty);
    let none = GunNameRules::default();
    let rules = world.get_resource::<GunNameRules>().unwrap_or(&none);
    let mut caps = vec![Capabilities::of(parts, catalog, None)];
    drawable(world, &mut caps);
    caps.iter()
        .flat_map(|c| c.guns.iter().map(|g| g.id).chain(c.grenades.iter().copied()))
        .map(|id| {
            let game = game_full.get(id).cloned().unwrap_or_else(|| {
                catalog
                    .get(id)
                    .and_then(|d| d.hud.name.clone())
                    .filter(|n| !crate::weapon::names::is_string_token(n))
                    .unwrap_or_else(|| id_name(id))
            });
            let full = rules.full(id, &game);
            let (short, from) = rules.short(id, &full);
            GunNameRow { id, full, short, from }
        })
        .collect()
}

/// `skipped` in a line or a few: the count, then the ids by reason.
pub fn skipped_lines(skipped: &[Skipped]) -> String {
    let mut reasons: Vec<&str> = Vec::new();
    for s in skipped {
        if !reasons.contains(&s.why.as_str()) {
            reasons.push(&s.why);
        }
    }
    let mut out = format!("drawn ladders skip {} weapons", skipped.len());
    for r in reasons {
        let ids: Vec<&str> = skipped.iter().filter(|s| s.why == r).map(|s| s.id).collect();
        out.push_str(&format!("\n  {r} ({}): {}", ids.len(), ids.join(", ")));
    }
    out
}

/// A small deterministic generator (SplitMix64).
#[derive(Clone, Copy, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// In `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// In `0..n` (n > 0).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn shuffle<T>(items: &mut [T], rng: &mut Rng) {
    for i in (1..items.len()).rev() {
        items.swap(i, rng.below(i + 1));
    }
}

/// A drawn ladder (`ladder`): its gun and grenade rungs in order, each
/// rung's band, and how many melee rungs end it (the knife finale's,
/// drawn when the match starts: `chimera:ladder`'s `melee_rungs`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawn {
    pub rungs: Vec<&'static str>,
    pub bands: Vec<Band>,
    pub melee: usize,
}

impl Drawn {
    /// Rungs in all, the melee ones too.
    pub fn len(&self) -> usize {
        self.rungs.len() + self.melee
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One candidate: (weapon, its game's index in the draw).
type Candidate = (&'static str, usize);

/// The server session's memory of what was dealt (`Recency::record`, one
/// call per ladder made): each gun's heat, raised when a ladder deals it
/// and cooled by `DrawParams::recovery` at every ladder after. It makes a
/// gun less likely to be dealt (`DrawParams::recency`), never excluded,
/// and only against the guns of its own game. Not saved: a server restart
/// starts it empty.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Recency {
    heat: std::collections::BTreeMap<String, f64>,
    /// The heats before the last `record`, for `rewind`.
    prior: Option<std::collections::BTreeMap<String, f64>>,
}

impl Recency {
    /// A gun's heat (0: not dealt lately).
    pub fn heat(&self, id: &str) -> f64 {
        self.heat.get(id).copied().unwrap_or(0.0)
    }

    /// A ladder made: every heat cools, then the dealt guns heat up by 1.
    pub fn record<'a>(&mut self, dealt: impl IntoIterator<Item = &'a str>, draw: &DrawParams) {
        self.prior = Some(self.heat.clone());
        let keep = draw.recovery.clamp(0.0, 1.0);
        self.heat.retain(|_, h| {
            *h *= keep;
            *h > 1e-3
        });
        for id in dealt {
            *self.heat.entry(id.to_string()).or_insert(0.0) += 1.0;
        }
    }

    /// Back to before the last `record`: the same match's ladder drawn
    /// again (once the installs are known) isn't a second ladder.
    pub fn rewind(&mut self) {
        if let Some(prior) = self.prior.take() {
            self.heat = prior;
        }
    }
}

/// A ladder of `rungs` rungs in all (melee included) of `games` (each
/// playable), seeded, on the power curve (`power`), remembering nothing
/// (`ladder_with`).
pub fn ladder(games: &[&Capabilities], powers: &Powers, curve: &PowerCurve, rungs: usize, seed: u64) -> Drawn {
    ladder_with(games, powers, curve, rungs, seed, &Recency::default())
}

/// A ladder of `rungs` rungs in all (melee included) of `games` (each
/// playable), seeded, on the power curve (`power`): the rungs split over
/// the bands by the curve's shares (`power::band_counts`, boundaries
/// jittered, the first rung warm-up, the last melee); a band whose pool is
/// empty or short gives its rungs to the nearest band with guns to spare
/// (the earlier on a tie; none left: the ladder is shorter). Each rung
/// (in a shuffled order) is then dealt a game, and a gun of that game in
/// its band:
///
/// - the game: with `DrawParams::games` = equal (the default), from a
///   shuffled deck of the games dealt one per rung and reshuffled once
///   dealt out (a game without a gun left in the rung's band is passed
///   over until a later rung), so every game's share is equal however many
///   guns it has; no game is promised a rung, and runs are possible;
///   sqrt or count: at random, weighted by the square root or the count
///   of the game's guns in the band.
/// - the gun: at random from the game's guns in the band, a gun's weight
///   one over one plus `DrawParams::recency` times its heat in `recent`
///   (never 0, never against another game's gun).
///
/// Each band is then sorted by power score, weakest first. The grenade
/// band holds the games' grenades; the melee band is only counted
/// (`Drawn::melee`).
pub fn ladder_with(
    games: &[&Capabilities],
    powers: &Powers,
    curve: &PowerCurve,
    rungs: usize,
    seed: u64,
    recent: &Recency,
) -> Drawn {
    ladder_rules(games, powers, curve, rungs, seed, recent, &DrawRules::default())
}

/// The host's rules a draw follows beyond its pool (`gun_rules`): what a
/// ladder does when short, the families dealt as one, the guns to borrow.
#[derive(Clone, Copy, Debug, Default)]
pub struct DrawRules<'a> {
    pub short: WhenShort,
    /// Each gun's family (`mp_guns_skins one`): a family is one candidate,
    /// in the band of its first gun, its gun drawn at the deal among its
    /// guns there. None: every gun its own.
    pub families: Option<&'a std::collections::BTreeMap<&'static str, String>>,
    /// The guns `WhenShort::Borrow` takes (`gun_rules::Pool::reserve`).
    pub reserve: &'a [Capabilities],
}

/// `ladder_with` under the host's `rules` (`DrawRules`): with families,
/// a family is one candidate whose gun is drawn when dealt; once the
/// bands' hand-off has nothing left to give, `WhenShort` says whether the
/// lost rungs are dropped (shorter), dealt again from the bands' guns
/// (repeat: to the band with the most guns per rung, nearest first; the
/// ladder then laid out in rounds, so a gun comes back only after the
/// others: `spread`), or
/// taken from `rules.reserve`, the nearest band first (borrow).
pub fn ladder_rules(
    games: &[&Capabilities],
    powers: &Powers,
    curve: &PowerCurve,
    rungs: usize,
    seed: u64,
    recent: &Recency,
    rules: &DrawRules,
) -> Drawn {
    use std::collections::BTreeMap;
    let games: Vec<&Capabilities> = games.iter().copied().filter(|c| c.playable()).collect();
    if games.is_empty() || rungs == 0 {
        return Drawn::default();
    }
    let draw = &curve.draw;
    let mut rng = Rng::new(seed);
    let mut counts = power::band_counts(rungs, curve, &mut rng);
    let melee = counts[Band::Melee.index()];
    const DRAWN: usize = 6;
    let band_of = |id: &str, class: GunClass| powers.get(id).map_or_else(|| curve.class_band(class), |p| p.band);
    // Each weapon once, in the first pool that takes it; a family once,
    // its other guns its members.
    let mut seen: bevy::platform::collections::HashSet<&'static str> = Default::default();
    let mut reps: BTreeMap<String, &'static str> = BTreeMap::new();
    let mut members: BTreeMap<&'static str, Vec<&'static str>> = BTreeMap::new();
    let mut add = |pools: &mut [Vec<Candidate>; DRAWN], id: &'static str, band: Band, game: usize| {
        if band == Band::Melee || !seen.insert(id) {
            return;
        }
        if let Some(fam) = rules.families.and_then(|f| f.get(id)) {
            if let Some(rep) = reps.get(fam) {
                members.entry(*rep).or_default().push(id);
                return;
            }
            reps.insert(fam.clone(), id);
            members.insert(id, vec![id]);
        }
        pools[band.index()].push((id, game));
    };
    let mut pools: [Vec<Candidate>; DRAWN] = Default::default();
    for (i, c) in games.iter().enumerate() {
        for gun in &c.guns {
            add(&mut pools, gun.id, band_of(gun.id, gun.class), i);
        }
        for g in &c.grenades {
            add(&mut pools, g, Band::Grenade, i);
        }
    }
    // Borrow's pool: the reserve's guns by band, each dealt as the game of
    // its own (or the first game when its game isn't in the draw).
    let mut reserve: [Vec<Candidate>; DRAWN] = Default::default();
    if rules.short == WhenShort::Borrow {
        for c in rules.reserve {
            let game = games.iter().position(|g| g.game == c.game).unwrap_or(0);
            for gun in &c.guns {
                add(&mut reserve, gun.id, band_of(gun.id, gun.class), game);
            }
            for g in &c.grenades {
                add(&mut reserve, g, Band::Grenade, game);
            }
        }
    }
    let sizes: [usize; DRAWN] = std::array::from_fn(|b| pools[b].len());
    // Short bands give their rungs to the nearest band with guns to spare;
    // the rungs none can take are lost (each one's band).
    let mut lost: Vec<usize> = Vec::new();
    for b in 0..DRAWN {
        while counts[b] > pools[b].len() {
            let to = (0..DRAWN)
                .filter(|&t| t != b && counts[t] < pools[t].len())
                .min_by_key(|&t| (t.abs_diff(b), t));
            counts[b] -= 1;
            match to {
                Some(t) => counts[t] += 1,
                None => lost.push(b),
            }
        }
    }
    match rules.short {
        WhenShort::Shorter => {}
        WhenShort::Repeat => {
            for b in lost {
                // The band with the most guns per rung (a band of one gun
                // last), the nearest on a tie.
                let to = (0..DRAWN)
                    .filter(|&t| sizes[t] > 0)
                    .min_by_key(|&t| (sizes[t] < 2, (counts[t] + 1).div_ceil(sizes[t]), t.abs_diff(b), t));
                if let Some(t) = to {
                    counts[t] += 1;
                }
            }
        }
        WhenShort::Borrow => {
            for b in lost {
                let from = (0..DRAWN).filter(|&t| !reserve[t].is_empty()).min_by_key(|&t| (t.abs_diff(b), t));
                if let Some(t) = from {
                    let k = rng.below(reserve[t].len());
                    pools[b].push(reserve[t].remove(k));
                    counts[b] += 1;
                }
            }
        }
    }
    // The rungs to deal, in a shuffled order so the deck's turns don't
    // follow the bands.
    let mut slots: Vec<usize> = (0..DRAWN).flat_map(|b| std::iter::repeat_n(b, counts[b])).collect();
    shuffle(&mut slots, &mut rng);
    let mut deck: Vec<usize> = Vec::new();
    let mut picked: [Vec<Candidate>; DRAWN] = Default::default();
    let heat = |id: &'static str| members.get(id).map_or_else(|| recent.heat(id), |m| m.iter().map(|g| recent.heat(g)).fold(0.0, f64::max));
    for b in slots {
        // Repeat: a band dealt out deals its guns again.
        if pools[b].is_empty() && rules.short == WhenShort::Repeat {
            let mut again: Vec<Candidate> = Vec::new();
            for c in &picked[b] {
                if !again.contains(c) {
                    again.push(*c);
                }
            }
            pools[b] = again;
        }
        let has = |pool: &[Candidate], g: usize| pool.iter().any(|c| c.1 == g);
        let game = match draw.games {
            GameWeighting::Equal => {
                let mut at = deck.iter().position(|&g| has(&pools[b], g));
                if at.is_none() {
                    // Dealt out for this band: a new shuffled pass behind
                    // what is left.
                    let mut pass: Vec<usize> = (0..games.len()).collect();
                    shuffle(&mut pass, &mut rng);
                    deck.extend(pass);
                    at = deck.iter().position(|&g| has(&pools[b], g));
                }
                at.map(|k| deck.remove(k))
            }
            weighting => {
                let weight = |g: usize| {
                    let n = pools[b].iter().filter(|c| c.1 == g).count() as f64;
                    if weighting == GameWeighting::Sqrt { n.sqrt() } else { n }
                };
                let total: f64 = (0..games.len()).map(weight).sum();
                let mut at = rng.unit() * total;
                let mut chosen = None;
                for g in 0..games.len() {
                    let w = weight(g);
                    if w <= 0.0 {
                        continue;
                    }
                    chosen = Some(g);
                    if at < w {
                        break;
                    }
                    at -= w;
                }
                chosen
            }
        };
        let Some(game) = game else { continue };
        // A gun of the game, the recently dealt less likely.
        let mine: Vec<usize> = (0..pools[b].len()).filter(|&k| pools[b][k].1 == game).collect();
        let weights: Vec<f64> = mine.iter().map(|&k| 1.0 / (1.0 + draw.recency.max(0.0) * heat(pools[b][k].0))).collect();
        let total: f64 = weights.iter().sum();
        let mut at = rng.unit() * total;
        let mut chosen = mine.len() - 1;
        for (k, w) in weights.iter().enumerate() {
            if at < *w {
                chosen = k;
                break;
            }
            at -= w;
        }
        let (mut id, g) = pools[b].remove(mine[chosen]);
        // A family's gun drawn now (its skin).
        if let Some(m) = members.get(id).filter(|m| m.len() > 1) {
            id = m[rng.below(m.len())];
        }
        picked[b].push((id, g));
    }
    let score = |id: &str| powers.get(id).map_or(0.0, |p| p.score);
    let mut out = Drawn {
        melee,
        ..Default::default()
    };
    let mut laid: Vec<(&'static str, Band)> = Vec::new();
    for (b, band) in picked.iter_mut().enumerate() {
        band.sort_by(|x, y| score(x.0).total_cmp(&score(y.0)).then(x.0.cmp(y.0)));
        laid.extend(band.iter().map(|c| (c.0, Band::ALL[b])));
    }
    for (id, band) in spread(&laid) {
        out.rungs.push(id);
        out.bands.push(band);
    }
    out
}

/// The ladder's rungs (weakest band first) laid out in rounds when a gun
/// repeats (`WhenShort::Repeat`): each gun once a round in ladder order,
/// so a gun comes back only after the others; a round that would start
/// with the gun that ended the last starts one later. Without repeats: as
/// given.
fn spread(rungs: &[(&'static str, Band)]) -> Vec<(&'static str, Band)> {
    let mut distinct: Vec<((&'static str, Band), usize)> = Vec::new();
    for c in rungs {
        match distinct.iter_mut().find(|(d, _)| d.0 == c.0) {
            Some((_, n)) => *n += 1,
            None => distinct.push((*c, 1)),
        }
    }
    if distinct.len() == rungs.len() {
        return rungs.to_vec();
    }
    let mut out: Vec<(&'static str, Band)> = Vec::new();
    let mut round = 0;
    while out.len() < rungs.len() {
        let mut this: Vec<(&'static str, Band)> = distinct.iter().filter(|(_, n)| *n > round).map(|(c, _)| *c).collect();
        if this.len() > 1 && out.last().is_some_and(|l| l.0 == this[0].0) {
            this.rotate_left(1);
        }
        out.extend(this);
        round += 1;
    }
    out
}

/// The powers of every gun and grenade of `caps` (`power::Powers::of`).
pub fn powers(caps: &[Capabilities], catalog: &WeaponCatalog, readers: &PowerReaders, curve: &PowerCurve) -> Powers {
    let guns = caps.iter().flat_map(|c| {
        c.guns
            .iter()
            .map(|g| (g.id, g.class))
            .chain(c.grenades.iter().map(|g| (*g, GunClass::Grenade)))
    });
    Powers::of(guns, catalog, readers, curve)
}

/// Every map of the games in `caps` that `installed` says the players
/// have, the games in turn; none: the greybox.
pub fn map_pool(caps: &[Capabilities], installed: &dyn Fn(&str) -> bool) -> Vec<String> {
    map_pool_for(caps, installed, None)
}

/// `map_pool` with `players` present: only the maps `pool` lets in at
/// that count by `settings` (`map_fit::MapPool::judge`); None: every map.
/// None let in: the greybox.
pub fn map_pool_for(
    caps: &[Capabilities],
    installed: &dyn Fn(&str) -> bool,
    players: Option<(&super::map_fit::MapPool, &super::map_fit::FitSettings, usize)>,
) -> Vec<String> {
    let mut out = every_map(caps, installed);
    if let Some((pool, settings, n)) = players {
        out.retain(|m| m == "greybox" || pool.judge(m, n, settings).weight() > 0.0);
        if out.is_empty() {
            out.push("greybox".into());
        }
    }
    out
}

fn every_map(caps: &[Capabilities], installed: &dyn Fn(&str) -> bool) -> Vec<String> {
    let lists: Vec<&Vec<String>> = caps.iter().filter(|c| installed(c.game)).map(|c| &c.maps).collect();
    let longest = lists.iter().map(|l| l.len()).max().unwrap_or(0);
    let mut out = Vec::new();
    for i in 0..longest {
        for l in &lists {
            if let Some(m) = l.get(i)
                && !out.contains(m)
            {
                out.push(m.clone());
            }
        }
    }
    if out.is_empty() {
        out.push("greybox".into());
    }
    out
}

/// The maps each registered game's install holds (`GameInfo::map_names`,
/// by game id), found once (`find_installed_maps`): `report` adds the
/// ones a game's pool rules allow to its maps. Absent (a test, an
/// app that never looked): claimed maps only.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct InstalledMaps(pub Vec<(String, Vec<String>)>);

impl InstalledMaps {
    pub fn of(&self, game: &str) -> &[String] {
        self.0.iter().find(|(g, _)| g.eq_ignore_ascii_case(game)).map_or(&[], |(_, m)| m)
    }
}

/// `InstalledMaps` from each registered game's install that has pool
/// rules (`map_fit::MapPoolAppExt`; mounted once; kept when already
/// there).
pub fn find_installed_maps(world: &mut World) {
    if world.contains_resource::<InstalledMaps>() {
        return;
    }
    let games: Vec<&'static chimera_core::game_registry::GameInfo> = world
        .get_resource::<chimera_core::game_registry::Games>()
        .map(|g| g.all().collect())
        .unwrap_or_default();
    let wanted = world.get_resource::<PartsRegistry>().cloned().unwrap_or_default();
    let pool = world.get_resource::<super::map_fit::MapPool>().cloned().unwrap_or_default();
    let found = games
        .into_iter()
        .filter(|g| wanted.get(g.id).is_some() && pool.has_rules(g.id))
        .filter_map(|g| Some((g.id.to_string(), (g.map_names)(&g.mount()?))))
        .collect();
    world.insert_resource(InstalledMaps(found));
}

/// What a player of game `game` gets where its parts lack one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fallbacks {
    /// The HUD drawn: the game's, else the registry's fallback.
    pub hud: &'static str,
    /// Its own bodies (its character kit), else the stand-in body (`render::stand_in`).
    pub own_body: bool,
    /// Its own maps in the pool, else other games' only.
    pub own_maps: bool,
}

/// `game`'s fallbacks (None: no parts registered for it).
pub fn fallbacks(registry: &PartsRegistry, game: &str) -> Option<Fallbacks> {
    let p = registry.get(game)?;
    Some(Fallbacks {
        hud: registry.hud_of(game)?,
        own_body: p.characters.is_some(),
        own_maps: !p.maps.is_empty(),
    })
}

/// How an automatic gun game is made.
#[derive(Clone, Debug)]
pub struct AutoOptions {
    pub id: String,
    pub name: String,
    /// Rungs in all, the melee finale's too.
    pub rungs: usize,
    pub seed: u64,
    /// The game whose binds, tick and bots the match uses, whose knife a
    /// game without one gets, whose HUD is drawn before a gun's game
    /// takes it.
    pub base: &'static str,
    /// The match's length, seconds.
    pub time_limit: f32,
    /// After a match: a random map of the pool, not one of the last this
    /// many played (`def::Rotation::Random`); None: the pool's next.
    pub avoid_recent: Option<usize>,
    /// The end screen's map vote (`def::MinigameDef::map_vote`, `vote`):
    /// its TOML value (`"true"`, `"{ candidates = 4 }"`); None: none.
    pub map_vote: Option<String>,
    /// The match end's spans (`def::MinigameDef::end`): its TOML value
    /// (`"{ round_over = 2, intermission = 0 }"`); None: the defaults.
    pub end: Option<String>,
    /// Whether guns can be taken off the ground (`def::PickupsDef::weapons`).
    pub weapon_pickups: bool,
    /// The power curve the ladder follows (`PowerCurve::shipped`).
    pub curve: PowerCurve,
}

/// TOML-quoted.
fn q(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

fn list(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| q(s)).collect::<Vec<_>>().join(", "))
}

/// The automatic gun game's definition text: the ladder of the playable
/// games every player has (`powers`: their guns' power, `auto::powers`), its
/// melee band as the knife finale's `melee_rungs`, the pool of their
/// maps; an error when no game is left to play.
pub fn definition(
    caps: &[Capabilities],
    powers: &Powers,
    opts: &AutoOptions,
    recent: &Recency,
    installed: &dyn Fn(&str) -> bool,
) -> Result<String, String> {
    definition_with(caps, powers, opts, recent, installed, &DrawRules::default())
}

/// `definition` with the host's rules (`DrawRules`, `draw_pool`); the
/// shared group of Allowed guns (`gun_rules::ALLOWED_GROUP`) counts as
/// installed (its guns' games were checked before).
pub fn definition_with(
    caps: &[Capabilities],
    powers: &Powers,
    opts: &AutoOptions,
    recent: &Recency,
    installed: &dyn Fn(&str) -> bool,
    rules: &DrawRules,
) -> Result<String, String> {
    let games: Vec<&Capabilities> = caps
        .iter()
        .filter(|c| c.playable() && (c.game == super::gun_rules::ALLOWED_GROUP || installed(c.game)))
        .collect();
    let drawn = ladder_rules(&games, powers, &opts.curve, opts.rungs, opts.seed, recent, rules);
    let ladder = drawn.rungs;
    if ladder.is_empty() {
        return Err(format!(
            "no game lends guns and movement that every player has ({})",
            caps.iter().map(|c| c.game).collect::<Vec<_>>().join(", ")
        ));
    }
    let base = caps.iter().find(|c| c.game == opts.base);
    let mut sets: Vec<String> = Vec::new();
    for id in &ladder {
        let game = id.split_once(':').map_or(*id, |(g, _)| g).to_string();
        if !sets.contains(&game) {
            sets.push(game);
        }
    }
    // The kit by game: each ladder game's own knife, none for a game
    // without one (never another game's: its slot key would make the
    // player that game's).
    let mut kit: Vec<String> = Vec::new();
    for game in &sets {
        if let Some(knife) = caps.iter().find(|c| c.game == game).and_then(|c| c.knife) {
            kit.push(format!("{} = {}", q(game), list(&[knife.to_string()])));
        }
    }
    let kit = format!("{{ {} }}", kit.join(", "));
    let ladder: Vec<String> = ladder.iter().map(|s| s.to_string()).collect();
    let maps = map_pool(caps, installed);
    let movement = base.and_then(|b| b.movement).map(|m| format!("movement = {}\n", q(m))).unwrap_or_default();
    let hud = base.and_then(|b| b.hud).unwrap_or(chimera_core::game_parts::MINIMAL_HUD);
    let hud = if hud == chimera_core::game_parts::MINIMAL_HUD { String::new() } else { format!("hud = {}\n", q(hud)) };
    let rotation = opts
        .avoid_recent
        .map(|n| format!("rotation = \"random\"\navoid_recent = {n}\n"))
        .unwrap_or_default();
    let rotation = match &opts.map_vote {
        Some(v) => format!("{rotation}map_vote = {v}\n"),
        None => rotation,
    };
    let rotation = match &opts.end {
        Some(e) => format!("{rotation}end = {e}\n"),
        None => rotation,
    };
    Ok(format!(
        "# An automatic gun game (chimera_sim::minigame::auto), seed {seed}.\n\
         id = {id}\nname = {name}\n\
         description = \"Climb a ladder of every game's guns; each gun makes you its game.\"\n\
         players = {{ min = 2, max = 16 }}\nmaps = {maps}\nteams = \"free_for_all\"\n\
         time_limit = {time}\nrotate = true\n{rotation}\n\
         [slots]\n{hud}binds = {base}\ntick = {base}\nbots = {base}\n{movement}\
         identity = \"weapon\"\nweapons = {sets}\n\n\
         [spawn]\nrespawn = {{ delay = \"2s\", protection = \"2s\" }}\npoints = \"scored\"\n\
         min_distance = 20.32\nkit = {kit}\n\n\
         {pickups}[rules]\nmodule = \"chimera:ladder\"\nladder = {ladder}\nadvance_on = \"kill\"\n\
         demote_on = \"melee_death\"\n{finale}refill_ammo = true\nwin = {{ kind = \"ladder_end\" }}\n",
        seed = opts.seed,
        pickups = if opts.weapon_pickups { "" } else { "[pickups]\nweapons = false\n\n" },
        id = q(&opts.id),
        name = q(&opts.name),
        maps = list(&maps),
        time = opts.time_limit,
        base = q(opts.base),
        sets = list(&sets),
        ladder = list(&ladder),
        finale = if drawn.melee > 0 {
            format!("knife_finale = true\nmelee_rungs = {}\n", drawn.melee)
        } else {
            "knife_finale = false\n".to_string()
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The host's guns off leave drawn ladders, by id whatever the case,
    /// grenades too; all of them off takes none.
    #[test]
    fn the_hosts_guns_off_are_left_out_unless_none_would_be_left() {
        use chimera_core::game_parts::{GunClass, GunPart};
        let caps = || {
            vec![Capabilities {
                game: "g",
                guns: vec![GunPart { id: "ak", class: GunClass::Rifle }, GunPart { id: "awp", class: GunClass::Sniper }],
                grenades: vec!["he"],
                knife: None,
                movement: None,
                characters: None,
                hud: None,
                maps: Vec::new(),
                missing: Vec::new(),
                scaffold: false,
            }]
        };
        let mut world = World::new();
        world.insert_resource(GunsOff("AWP, he".into()));
        let mut c = caps();
        let skipped = drawable(&world, &mut c);
        assert_eq!(c[0].guns.iter().map(|g| g.id).collect::<Vec<_>>(), ["ak"]);
        assert!(c[0].grenades.is_empty());
        assert_eq!(skipped.iter().map(|s| (s.id, s.why.as_str())).collect::<Vec<_>>(), [("awp", GunsOff::WHY), ("he", GunsOff::WHY)]);
        world.insert_resource(GunsOff("ak,awp".into()));
        let mut c = caps();
        assert!(drawable(&world, &mut c).is_empty(), "every gun off: none taken");
        assert_eq!(c[0].guns.len(), 2);
    }
    #[test]
    fn a_not_player_record_is_never_drawn_nor_listed_but_a_bad_skin_stays_in_the_sandbox() {
        use chimera_core::game_parts::{GunClass, GunPart};
        let text = "[[exclude]]\nreason = \"an NPC's gun\"\nnot_player = true\nweapons = [\"g:Npc\"]\n\
                    [[exclude]]\nreason = \"bad skin\"\nweapons = [\"g:skin\"]\n";
        let mut app = App::new();
        app.add_draw_exclusions("test", text);
        let world = app.world();
        let caps = || {
            vec![Capabilities {
                game: "g",
                guns: ["g:ak", "g:npc", "g:skin"].map(|id| GunPart { id, class: GunClass::Rifle }).to_vec(),
                grenades: vec![],
                knife: None,
                movement: None,
                characters: None,
                hud: None,
                maps: Vec::new(),
                missing: Vec::new(),
                scaffold: false,
            }]
        };
        // The draw and the Guns picker (`drawable_all`): neither is left.
        let mut c = caps();
        let skipped = drawable_all(world, &mut c);
        assert_eq!(c[0].guns.iter().map(|g| g.id).collect::<Vec<_>>(), ["g:ak"]);
        assert_eq!(skipped.iter().map(|s| (s.id, s.why.as_str())).collect::<Vec<_>>(), [("g:npc", "an NPC's gun"), ("g:skin", "bad skin")]);
        // The sandbox's list: only the not-player one goes.
        let mut c = caps();
        drop_not_players(world, &mut c);
        assert_eq!(c[0].guns.iter().map(|g| g.id).collect::<Vec<_>>(), ["g:ak", "g:skin"]);
    }

    use crate::minigame::power::Power;

    /// `n` guns of `game`, their classes in turn from `classes`.
    fn guns_of(game: &'static str, n: usize, classes: &[GunClass]) -> Capabilities {
        // Leaked: a test's few ids.
        let guns = (0..n)
            .map(|i| GunPart {
                id: Box::leak(format!("{game}:gun{i}").into_boxed_str()),
                class: classes[i % classes.len()],
            })
            .collect();
        Capabilities {
            game,
            guns,
            grenades: vec![],
            knife: None,
            movement: Some("m"),
            characters: None,
            hud: None,
            maps: vec![],
            missing: vec![],
            scaffold: false,
        }
    }

    fn guns(game: &'static str, n: usize) -> Capabilities {
        guns_of(game, n, &GunClass::ALL)
    }

    fn with_grenade(mut c: Capabilities) -> Capabilities {
        c.grenades = vec![Box::leak(format!("{}:frag", c.game).into_boxed_str())];
        c
    }

    /// Each gun's band by its class, a made-up score that isn't the
    /// guns' order.
    fn powers_of(caps: &[&Capabilities]) -> Powers {
        let curve = &PowerCurve::shipped();
        let mut out = std::collections::BTreeMap::new();
        for (k, g) in caps.iter().flat_map(|c| &c.guns).enumerate() {
            let score = ((k * 37) % 23) as f32 + 1.0;
            out.insert(g.id, Power { band: curve.class_band(g.class), score, ttk: 1.0 / score });
        }
        for g in caps.iter().flat_map(|c| &c.grenades) {
            out.insert(*g, Power { band: Band::Grenade, score: 1.0, ttk: 1.0 });
        }
        Powers(out)
    }

    fn draw(caps: &[&Capabilities], rungs: usize, seed: u64) -> Drawn {
        ladder(caps, &powers_of(caps), &PowerCurve::shipped(), rungs, seed)
    }

    fn class_of(caps: &[&Capabilities], id: &str) -> Option<GunClass> {
        caps.iter().flat_map(|c| &c.guns).find(|g| g.id == id).map(|g| g.class)
    }

    fn draw_with(caps: &[&Capabilities], rungs: usize, seed: u64, rules: &DrawRules) -> Drawn {
        let mut all: Vec<&Capabilities> = caps.to_vec();
        all.extend(rules.reserve.iter());
        ladder_rules(caps, &powers_of(&all), &PowerCurve::shipped(), rungs, seed, &Recency::default(), rules)
    }

    /// With families, a family is dealt at most once a ladder, as one of
    /// its guns; every gun of it comes up over seeds.
    #[test]
    fn a_family_is_one_candidate_its_skin_drawn_at_the_deal() {
        let a = guns_of("a", 12, &[GunClass::Rifle]);
        let fam: std::collections::BTreeMap<&'static str, String> = a.guns.iter().take(10).map(|g| (g.id, "a:ak".to_string())).collect();
        let rules = DrawRules { families: Some(&fam), ..default() };
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..200 {
            let d = draw_with(&[&a], 13, seed, &rules);
            let in_family = d.rungs.iter().filter(|id| fam.contains_key(**id)).count();
            assert!(in_family <= 1, "seed {seed}: {:?}", d.rungs);
            // Three candidates (the family, gun10, gun11) for the rungs.
            assert_eq!(d.rungs.len(), 3, "seed {seed}");
            seen.extend(d.rungs.iter().filter(|id| fam.contains_key(**id)).copied());
        }
        assert_eq!(seen.len(), 10, "every skin comes up");
    }

    /// Shorter drops the rungs no band can fill; repeat deals them again,
    /// never the same gun twice in a row while there is another.
    #[test]
    fn when_short_shorter_or_repeat_spread_out() {
        let a = with_grenade(guns_of("a", 3, &[GunClass::Pistol, GunClass::Smg, GunClass::Rifle]));
        let short = draw_with(&[&a], 13, 1, &DrawRules::default());
        assert_eq!(short.len(), 4 + short.melee - 0);
        assert_eq!(short.rungs.len(), 4);
        for seed in 0..50 {
            let rules = DrawRules { short: WhenShort::Repeat, ..default() };
            let d = draw_with(&[&a], 13, seed, &rules);
            assert_eq!(d.len(), 13, "seed {seed}: {:?}", d.rungs);
            assert!(d.rungs.windows(2).all(|w| w[0] != w[1]), "seed {seed}: {:?}", d.rungs);
        }
    }

    /// Borrow fills the lost rungs from the reserve, the nearest band
    /// first, and only from it.
    #[test]
    fn when_short_borrow_takes_the_reserve_nearest_first() {
        let a = guns_of("a", 2, &[GunClass::Pistol]);
        let reserve = vec![guns_of("a2", 2, &[GunClass::Sniper]), guns_of("a3", 1, &[GunClass::Smg])];
        // The reserve's games are the draw's own game (`a`): rename.
        let mut r: Vec<Capabilities> = reserve;
        for c in &mut r {
            c.game = "a";
        }
        let rules = DrawRules { short: WhenShort::Borrow, reserve: &r, ..default() };
        let d = draw_with(&[&a], 6, 3, &rules);
        assert_eq!(d.rungs.len(), 5, "{:?}", d.rungs);
        assert!(d.rungs.contains(&"a3:gun0"), "the SMG (nearest) is borrowed: {:?}", d.rungs);
        let few = DrawRules { short: WhenShort::Borrow, reserve: &r[1..], ..default() };
        assert_eq!(draw_with(&[&a], 13, 3, &few).rungs.len(), 3, "the reserve runs out: shorter");
    }

    /// Allowed guns of games that are off are one game in the deck: one
    /// allowed gun doesn't take a whole game's share.
    #[test]
    fn allowed_guns_of_games_off_share_one_deck_turn() {
        let a = guns("a", 60);
        let mut group = super::super::gun_rules::allowed_group();
        group.guns = guns("b", 22).guns;
        let caps = [&a, &group];
        let mut theirs = 0;
        for seed in 0..100 {
            let d = draw_with(&caps, 13, seed, &DrawRules::default());
            theirs += d.rungs.iter().filter(|id| id.starts_with("b:")).count();
        }
        let share = theirs as f64 / 1200.0;
        assert!((0.35..0.65).contains(&share), "{share}");
    }

    #[test]
    fn a_ladder_climbs_the_bands() {
        let (a, b, c) = (with_grenade(guns("a", 60)), guns("b", 33), guns("c", 4));
        let caps = [&a, &b, &c];
        let powers = powers_of(&caps);
        for n in [10usize, 13, 20, 30] {
            for seed in 0..60 {
                let l = draw(&caps, n, seed);
                assert_eq!(l.len(), n, "{n} rungs in all: {l:?}");
                assert!(l.melee >= 1, "a melee rung at least");
                assert_eq!(l.bands.len(), l.rungs.len());
                assert!(l.bands.windows(2).all(|w| w[0] <= w[1]), "warm-up to grenade in order: {:?}", l.bands);
                assert_eq!(l.bands[0], Band::WarmUp, "a warm-up rung first");
                assert_eq!(class_of(&caps, l.rungs[0]), Some(GunClass::Pistol), "a pistol first: {l:?}");
                let mut once = l.rungs.clone();
                once.sort();
                once.dedup();
                assert_eq!(once.len(), l.rungs.len(), "a gun once");
                // Inside a band, weakest first.
                for w in 0..l.rungs.len().saturating_sub(1) {
                    if l.bands[w] == l.bands[w + 1] {
                        let (x, y) = (powers.get(l.rungs[w]).unwrap().score, powers.get(l.rungs[w + 1]).unwrap().score);
                        assert!(x <= y, "scores climb in {:?}: {l:?}", l.bands[w]);
                    }
                }
                // Launchers and flamethrowers only in the specials band, the
                // grenade only in its own.
                for (id, band) in l.rungs.iter().zip(&l.bands) {
                    match class_of(&caps, id) {
                        Some(GunClass::Launcher | GunClass::Flamethrower) => assert_eq!(*band, Band::Specials),
                        None => assert_eq!(*band, Band::Grenade, "{id}"),
                        _ => assert!(*band < Band::Specials, "{id}"),
                    }
                }
            }
        }
        // More rungs than guns: every gun once, then melee.
        let few = guns("f", 3);
        let l = draw(&[&few], 10, 1);
        assert_eq!(l.rungs.len(), 3, "{l:?}");
    }

    /// The share of the dealt rungs each game got, over `n` seeded ladders.
    fn shares(caps: &[&Capabilities], curve: &PowerCurve, recent: &Recency, n: u64) -> Vec<f64> {
        let powers = powers_of(caps);
        let mut got = vec![0usize; caps.len()];
        for seed in 0..n {
            for id in ladder_with(caps, &powers, curve, 20, seed, recent).rungs {
                let g = caps.iter().position(|c| id.starts_with(&format!("{}:", c.game))).unwrap();
                got[g] += 1;
            }
        }
        let all: usize = got.iter().sum();
        got.iter().map(|g| *g as f64 / all as f64).collect()
    }

    fn with_weighting(games: GameWeighting) -> PowerCurve {
        let mut c = PowerCurve::shipped();
        c.draw.games = games;
        c
    }

    #[test]
    fn games_get_equal_shares_whatever_their_gun_counts() {
        let (a, b, c) = (guns("a", 60), guns("b", 24), guns("c", 8));
        let caps = [&a, &b, &c];
        let equal = shares(&caps, &PowerCurve::shipped(), &Recency::default(), 400);
        for (g, s) in equal.iter().enumerate() {
            assert!((s - 1.0 / 3.0).abs() < 0.04, "game {g} has {s:.3}: {equal:?}");
        }
        // sqrt and count follow the guns' numbers instead.
        let sqrt = shares(&caps, &with_weighting(GameWeighting::Sqrt), &Recency::default(), 400);
        assert!(sqrt[0] > sqrt[1] && sqrt[1] > sqrt[2], "{sqrt:?}");
        assert!(sqrt[2] > 0.1, "{sqrt:?}");
        let count = shares(&caps, &with_weighting(GameWeighting::Count), &Recency::default(), 400);
        assert!(count[0] > 0.5 && count[2] < sqrt[2], "{count:?} against {sqrt:?}");
    }

    #[test]
    fn recency_lowers_repeats_but_never_excludes_and_stays_in_the_game() {
        let (a, b) = (guns("a", 40), guns("b", 3));
        let caps = [&a, &b];
        let powers = powers_of(&caps);
        let curve = PowerCurve::shipped();
        let mut off = curve.clone();
        off.draw.recency = 0.0;
        // Matches in a row on one server: the share of rungs that were
        // also in the match before.
        let repeat = |curve: &PowerCurve| {
            let mut mem = Recency::default();
            let mut last: Vec<&str> = Vec::new();
            let (mut same, mut all) = (0usize, 0usize);
            let mut b_rungs = 0usize;
            for seed in 0..300u64 {
                let d = ladder_with(&caps, &powers, curve, 13, seed, &mem);
                same += d.rungs.iter().filter(|id| last.contains(id)).count();
                all += d.rungs.len();
                b_rungs += d.rungs.iter().filter(|id| id.starts_with("b:")).count();
                mem.record(d.rungs.iter().copied(), &curve.draw);
                last = d.rungs;
            }
            (same as f64 / all as f64, b_rungs as f64 / all as f64)
        };
        let (on_repeat, on_b) = repeat(&curve);
        let (off_repeat, off_b) = repeat(&off);
        assert!(on_repeat < off_repeat * 0.9, "{on_repeat:.3} with recency, {off_repeat:.3} without");
        // The few-gun game keeps its share (recency only cycles its guns).
        assert!((on_b - off_b).abs() < 0.04, "{on_b:.3} against {off_b:.3}");
        // Never excluded: a gun at a huge heat still comes up.
        let mut hot = Recency::default();
        for g in a.guns.iter().chain(&b.guns) {
            for _ in 0..5 {
                hot.record([g.id], &curve.draw);
            }
        }
        let drawn: Vec<Drawn> = (0..200).map(|s| ladder_with(&caps, &powers, &curve, 13, s, &hot)).collect();
        for g in b.guns.iter() {
            assert!(drawn.iter().any(|d| d.rungs.contains(&g.id)), "{} excluded", g.id);
        }
        // Seeded draws stay deterministic with a memory.
        assert_eq!(ladder_with(&caps, &powers, &curve, 13, 5, &hot), ladder_with(&caps, &powers, &curve, 13, 5, &hot));
    }

    #[test]
    fn recency_cools_over_matches_and_rewinds() {
        let d = DrawParams { games: GameWeighting::Equal, recency: 2.0, recovery: 0.5 };
        let mut mem = Recency::default();
        mem.record(["a:x"], &d);
        assert_eq!(mem.heat("a:x"), 1.0);
        mem.record(["a:y"], &d);
        assert_eq!((mem.heat("a:x"), mem.heat("a:y")), (0.5, 1.0));
        mem.rewind();
        assert_eq!((mem.heat("a:x"), mem.heat("a:y")), (1.0, 0.0));
    }

    #[test]
    fn draw_settings_are_data_a_definition_tunes() {
        let c = PowerCurve::shipped();
        assert_eq!(c.draw.games, GameWeighting::Equal);
        let t: toml::Table = toml::from_str("draw = { games = \"sqrt\", recency = 0.0 }").unwrap();
        let tuned = PowerCurve::tuned(&t).unwrap();
        assert_eq!((tuned.draw.games, tuned.draw.recency, tuned.draw.recovery), (GameWeighting::Sqrt, 0.0, c.draw.recovery));
    }

    #[test]
    fn no_pistol_starts_on_the_weakest_smg() {
        let a = guns_of("a", 20, &[GunClass::Smg, GunClass::Rifle, GunClass::Sniper, GunClass::Shotgun]);
        let caps = [&a];
        let powers = powers_of(&caps);
        for seed in 0..40 {
            let l = draw(&caps, 10, seed);
            assert_eq!(l.bands[0], Band::Rising, "{l:?}");
            let first = powers.get(l.rungs[0]).unwrap().score;
            let rising: Vec<f32> =
                l.rungs.iter().zip(&l.bands).filter(|(_, b)| **b == Band::Rising).map(|(id, _)| powers.get(id).unwrap().score).collect();
            assert!(rising.iter().all(|s| *s >= first), "the weakest of its band first: {l:?}");
        }
    }

    #[test]
    fn a_ladder_is_the_seeds_and_seeds_differ_widely() {
        let (a, b) = (guns("a", 44), guns("b", 33));
        let caps = [&a, &b];
        assert_eq!(draw(&caps, 13, 42), draw(&caps, 13, 42));
        let ladders: Vec<Vec<&str>> = (0..400).map(|s| draw(&caps, 13, s).rungs).collect();
        // Two ladders share few rungs.
        let mut shared = 0;
        let mut worst = 0;
        for w in ladders.windows(2) {
            let n = w[0].iter().filter(|id| w[1].contains(id)).count();
            shared += n;
            worst = worst.max(n);
        }
        let mean = shared as f64 / (ladders.len() - 1) as f64;
        assert!(mean < 12.0 * 0.4, "on average {mean:.1} of 12 rungs shared");
        assert!(worst < 10, "never nearly the same: {worst}");
        // Every gun comes up.
        for g in a.guns.iter().chain(&b.guns) {
            assert!(ladders.iter().any(|l| l.contains(&g.id)), "{} never drawn", g.id);
        }
    }

    #[test]
    fn grenade_rungs_come_after_the_specials_before_melee() {
        let (a, b) = (with_grenade(guns("a", 44)), with_grenade(guns("b", 33)));
        let caps = [&a, &b];
        let mut counts = [0; 3];
        for seed in 0..300 {
            let l = draw(&caps, 20, seed);
            let at: Vec<usize> = (0..l.rungs.len()).filter(|&i| l.rungs[i].ends_with(":frag")).collect();
            assert!(at.len() <= 2, "{l:?}");
            counts[at.len()] += 1;
            assert!(at.iter().all(|&i| l.bands[i] == Band::Grenade && i + 1 >= l.rungs.len() - 1), "{l:?}");
        }
        assert!(counts[1] > 0 && counts[2] > 0, "{counts:?}");
        // No grenade: its rungs go to the specials.
        let c = guns("c", 44);
        let l = draw(&[&c], 20, 3);
        assert_eq!(l.len(), 20);
        assert!(l.bands.iter().all(|b| *b != Band::Grenade));
    }

    #[test]
    fn games_without_movement_or_install_stay_out() {
        let a = guns("a", 12);
        let mut b = guns("b", 12);
        b.movement = None;
        assert!(draw(&[&a, &b], 6, 3).rungs.iter().all(|id| id.starts_with("a:")));
        let mut c = guns("c", 12);
        c.maps = vec!["c:map1".into(), "c:map2".into()];
        let mut d = guns("d", 12);
        d.maps = vec!["d:map1".into()];
        let opts = AutoOptions {
            id: "test:auto".into(),
            name: "Auto".into(),
            rungs: 10,
            seed: 9,
            base: "a",
            time_limit: 600.0,
            avoid_recent: Some(10),
            map_vote: Some("true".into()),
            end: None,
            weapon_pickups: true,
            curve: PowerCurve::shipped(),
        };
        let caps = [a, c, d];
        let refs: Vec<&Capabilities> = caps.iter().collect();
        let text = definition(&caps, &powers_of(&refs), &opts, &Recency::default(), &|g| g != "c").unwrap();
        let def = crate::minigame::MinigameDef::parse("auto", &text).expect(&text);
        assert_eq!(def.maps, ["d:map1"], "c isn't installed: its maps go too");
        assert_eq!(def.rotation, crate::minigame::def::Rotation::Random);
        assert_eq!(def.avoid_recent, 10);
        let ladder = def.rules.params["ladder"].as_array().unwrap();
        assert!(ladder.iter().all(|id| !id.as_str().unwrap().starts_with("c:")), "{ladder:?}");
        // The melee band is the finale's.
        let melee = def.rules.params["melee_rungs"].as_integer().unwrap() as usize;
        assert!(melee >= 1 && ladder.len() + melee == 10, "{text}");
        assert_eq!(def.rules.params["knife_finale"].as_bool(), Some(true));
        assert_eq!(map_pool(&caps, &|_| true), ["c:map1", "d:map1", "c:map2"], "in turn");
        assert_eq!(map_pool(&caps[..1], &|_| true), ["greybox"]);
        assert!(definition(&caps, &powers_of(&refs), &opts, &Recency::default(), &|_| false).is_err());
    }

    static P: GameParts = GameParts {
        game: "g",
        guns: &[],
        knife: None,
        movement: None,
        characters: None,
        hud: None,
        maps: &["g:de_one"],
        announcer: None,
        match_won_sound: None,
        match_lost_sound: None,
        music: false,
        cues: None,
        scaffold: false,
    };

    #[test]
    fn installed_maps_join_unless_the_rules_deny_them() {
        use crate::minigame::map_fit::MapPoolAppExt;
        let mut app = App::new();
        app.world_mut().get_resource_or_init::<PartsRegistry>().register(&P);
        let installed: Vec<String> = ["de_one", "DE_Two", "cs_three", "test_x"].iter().map(|s| s.to_string()).collect();
        app.insert_resource(InstalledMaps(vec![("g".into(), installed)]));
        // No rules: the claimed maps only.
        assert_eq!(report(app.world())[0].maps, ["g:de_one"]);
        app.add_map_pool_rules("g", "test", "[[deny]]\nmaps = [\"test_*\"]\nreason = \"a test map\"");
        assert_eq!(report(app.world())[0].maps, ["g:de_one", "g:de_two", "g:cs_three"]);
    }

    #[test]
    fn the_pool_for_a_count_leaves_out_the_maps_too_big_for_it() {
        use crate::minigame::map_fit::{FitSettings, MapPool, parse_fitness};
        let mut c = guns("g", 2);
        c.maps = vec!["g:tiny".into(), "g:huge".into(), "g:new".into()];
        let mut pool = MapPool::default();
        let fit = "[maps.tiny]\narea_m2 = 500\nsight_m = 8\nspawns = 8\nspots = 100\n\
                   [maps.huge]\narea_m2 = 9000\nsight_m = 8\nspawns = 32\nspots = 5000\n";
        pool.add_fitness("g", parse_fitness(fit).unwrap());
        let s = FitSettings::shipped();
        let caps = [c];
        assert_eq!(map_pool(&caps, &|_| true), ["g:tiny", "g:huge", "g:new"], "no count: every map");
        assert_eq!(map_pool_for(&caps, &|_| true, Some((&pool, &s, 4))), ["g:tiny", "g:new"]);
        assert_eq!(map_pool_for(&caps, &|_| true, Some((&pool, &s, 12))), ["g:tiny", "g:huge", "g:new"]);
    }
}
