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
//!   maps (claimed, and the installed ones its `map_patterns` match:
//!   `InstalledMaps`, found once by `find_installed_maps` in an app that
//!   wants them). A game is in a ladder with guns and movement
//!   (`Capabilities::playable`); the rest falls back (`fallbacks`): no
//!   HUD, `PartsRegistry::fallback_hud`; no characters, the shell's
//!   capsule (`map::CharacterModels::for_identity` finds none of its
//!   game); no maps, the other games'.
//! - `ladder`: `rungs` rungs in all, seeded and deterministic (SplitMix64;
//!   no hash maps), on the power curve (`minigame::power`): the rungs
//!   split over the bands (warm-up, rising, peak, heavy, specials,
//!   grenade, melee) by the curve's shares, boundaries jittered; each
//!   band's guns drawn from every game's guns of its classes (each game at
//!   least once; a game's guns weighted one over the square root of its
//!   count), sorted by power score, weakest first; the melee band counted
//!   for the knife finale, which draws its weapons when the match starts.
//!   Two seeds share few rungs, and every gun comes up over many.
//! - `map_pool`: every map of every game whose install the players have,
//!   the games in turn; none: the greybox.
//! - `definition`: the minigame's TOML (`MinigameDef`), filtered by
//!   `installed` (the games every player has: L3's mount reports, which
//!   the app reads; `chimera_net::minigame_net::installed_by_all`). Its
//!   kit is by game (`def::Kit::ByGame`): each gun's game's own knife,
//!   the base game's for a game that lends none; its rotation random but
//!   the last maps played when `AutoOptions::avoid_recent` says
//!   (`minigame::rotation`).

use chimera_core::{
    game_parts::{GameParts, GunClass, GunPart, PartsRegistry},
    slots::MovementRegistry,
};

use bevy::prelude::*;

use super::power::{self, Band, PowerCurve, PowerReaders, Powers};
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
    /// Its claimed maps, then its installed maps its `map_patterns` match.
    pub maps: Vec<String>,
    /// Claimed parts the app doesn't have (a gun not in the catalogue, an
    /// unregistered movement), in words.
    pub missing: Vec<String>,
}

impl Capabilities {
    /// Guns and movement: in a ladder.
    pub fn playable(&self) -> bool {
        !self.guns.is_empty() && self.movement.is_some()
    }

    /// `parts` checked against `catalog` and `movements`.
    pub fn of(parts: &GameParts, catalog: &WeaponCatalog, movements: Option<&MovementRegistry>) -> Self {
        Self::with_maps(parts, catalog, movements, &[])
    }

    /// `of`, with the maps its install holds (`InstalledMaps::of`).
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
        for set in &catalog.sets {
            for w in &set.weapons {
                if !w.id.starts_with(&prefix) || all.iter().any(|g| g.id == w.id) {
                    continue;
                }
                let Some(data) = catalog.get(&w.id) else { continue };
                if let Some(class) = data.class {
                    all.push(GunPart { id: &data.id, class });
                }
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
        for name in installed.iter().filter(|n| parts.wants_map(n)) {
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
            or(self.knife, "the base game's"),
            or(self.movement, "not in a ladder"),
            or(self.characters, "a capsule"),
            or(self.hud, "the fallback HUD"),
            if self.maps.is_empty() { "none (other games')".to_string() } else { self.maps.join(", ") },
            if self.playable() { "" } else { "; not playable" },
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
    registry
        .all()
        .map(|p| Capabilities::with_maps(p, catalog, movements, installed.map_or(&[], |i| i.of(p.game))))
        .collect()
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

/// One candidate: (weapon, its game, its weight).
type Candidate = (&'static str, usize, f64);

/// A ladder of `rungs` rungs in all (melee included) of `games` (each
/// playable), seeded, on the power curve (`power`): the rungs split over
/// the bands by the curve's shares (`power::band_counts`, boundaries
/// jittered, the first rung warm-up, the last melee); a band whose pool is
/// empty or short gives its rungs to the nearest band with guns to spare
/// (the earlier on a tie; none left: the ladder is shorter); each band's
/// guns drawn at random from its pool, every game's guns mixed (a game's
/// weighted one over the square root of its count, so a game with many
/// guns gives more rungs but doesn't drown the others; each game at least
/// once when it can be), then sorted by power score, weakest first. The
/// grenade band holds the games' grenades; the melee band is only counted
/// (`Drawn::melee`).
pub fn ladder(games: &[&Capabilities], powers: &Powers, curve: &PowerCurve, rungs: usize, seed: u64) -> Drawn {
    let games: Vec<&Capabilities> = games.iter().copied().filter(|c| c.playable()).collect();
    if games.is_empty() || rungs == 0 {
        return Drawn::default();
    }
    let mut rng = Rng::new(seed);
    let mut counts = power::band_counts(rungs, curve, &mut rng);
    let melee = counts[Band::Melee.index()];
    const DRAWN: usize = 6;
    let mut pools: [Vec<Candidate>; DRAWN] = Default::default();
    let seen = |pools: &[Vec<Candidate>; DRAWN], id: &str| pools.iter().any(|p| p.iter().any(|c| c.0 == id));
    for (i, c) in games.iter().enumerate() {
        let weight = 1.0 / (c.guns.len().max(1) as f64).sqrt();
        for gun in &c.guns {
            let band = powers.get(gun.id).map_or_else(|| curve.class_band(gun.class), |p| p.band);
            if band != Band::Melee && !seen(&pools, gun.id) {
                pools[band.index()].push((gun.id, i, weight));
            }
        }
        let weight = 1.0 / (c.grenades.len().max(1) as f64).sqrt();
        for g in &c.grenades {
            if !seen(&pools, g) {
                pools[Band::Grenade.index()].push((g, i, weight));
            }
        }
    }
    // Short bands give their rungs to the nearest band with guns to spare.
    for b in 0..DRAWN {
        while counts[b] > pools[b].len() {
            let to = (0..DRAWN)
                .filter(|&t| t != b && counts[t] < pools[t].len())
                .min_by_key(|&t| (t.abs_diff(b), t));
            counts[b] -= 1;
            if let Some(t) = to {
                counts[t] += 1;
            }
        }
    }
    let mut picked: [Vec<Candidate>; DRAWN] = Default::default();
    for b in 0..DRAWN {
        while picked[b].len() < counts[b] && !pools[b].is_empty() {
            let total: f64 = pools[b].iter().map(|c| c.2).sum();
            let mut at = rng.unit() * total;
            let mut chosen = pools[b].len() - 1;
            for (k, c) in pools[b].iter().enumerate() {
                if at < c.2 {
                    chosen = k;
                    break;
                }
                at -= c.2;
            }
            picked[b].push(pools[b].remove(chosen));
        }
    }
    // Each game at least once: a missing game's gun in place of a gun of a
    // game that has more than one, in a band where it has one left.
    let mut order: Vec<usize> = (0..games.len()).collect();
    shuffle(&mut order, &mut rng);
    for g in order {
        let of = |picked: &[Vec<Candidate>; DRAWN], g: usize| picked.iter().flatten().filter(|c| c.1 == g).count();
        if of(&picked, g) > 0 {
            continue;
        }
        let bands: Vec<usize> = (0..DRAWN)
            .filter(|&b| pools[b].iter().any(|c| c.1 == g) && picked[b].iter().any(|c| of(&picked, c.1) > 1))
            .collect();
        if bands.is_empty() {
            continue;
        }
        let b = bands[rng.below(bands.len())];
        let out: Vec<usize> = (0..picked[b].len()).filter(|&k| of(&picked, picked[b][k].1) > 1).collect();
        let k = out[rng.below(out.len())];
        let mine: Vec<usize> = (0..pools[b].len()).filter(|&k| pools[b][k].1 == g).collect();
        let new = pools[b].remove(mine[rng.below(mine.len())]);
        let old = std::mem::replace(&mut picked[b][k], new);
        pools[b].push(old);
    }
    let score = |id: &str| powers.get(id).map_or(0.0, |p| p.score);
    let mut out = Drawn {
        melee,
        ..Default::default()
    };
    for (b, band) in picked.iter_mut().enumerate() {
        band.sort_by(|x, y| score(x.0).total_cmp(&score(y.0)).then(x.0.cmp(y.0)));
        out.rungs.extend(band.iter().map(|c| c.0));
        out.bands.extend(std::iter::repeat_n(Band::ALL[b], band.len()));
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
/// ones a game's `map_patterns` match to its maps. Absent (a test, an
/// app that never looked): claimed maps only.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct InstalledMaps(pub Vec<(String, Vec<String>)>);

impl InstalledMaps {
    pub fn of(&self, game: &str) -> &[String] {
        self.0.iter().find(|(g, _)| g.eq_ignore_ascii_case(game)).map_or(&[], |(_, m)| m)
    }
}

/// `InstalledMaps` from each registered game's install (mounted once;
/// kept when already there).
pub fn find_installed_maps(world: &mut World) {
    if world.contains_resource::<InstalledMaps>() {
        return;
    }
    let games: Vec<&'static chimera_core::game_registry::GameInfo> = world
        .get_resource::<chimera_core::game_registry::Games>()
        .map(|g| g.all().collect())
        .unwrap_or_default();
    let wanted = world.get_resource::<PartsRegistry>().cloned().unwrap_or_default();
    let found = games
        .into_iter()
        .filter(|g| wanted.get(g.id).is_some_and(|p| !p.map_patterns.is_empty()))
        .filter_map(|g| Some((g.id.to_string(), (g.map_names)(&g.mount()?))))
        .collect();
    world.insert_resource(InstalledMaps(found));
}

/// What a player of game `game` gets where its parts lack one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fallbacks {
    /// The HUD drawn: the game's, else the registry's fallback.
    pub hud: &'static str,
    /// Its own bodies (its character kit), else the shell's capsule.
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
    installed: &dyn Fn(&str) -> bool,
) -> Result<String, String> {
    let games: Vec<&Capabilities> = caps.iter().filter(|c| c.playable() && installed(c.game)).collect();
    let drawn = ladder(&games, powers, &opts.curve, opts.rungs, opts.seed);
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
    // The kit by game: each ladder game's own knife, else the base game's.
    let fallback = base.and_then(|b| b.knife);
    let mut kit: Vec<String> = Vec::new();
    for game in sets.clone() {
        let own = caps.iter().find(|c| c.game == game).and_then(|c| c.knife);
        if let Some(knife) = own.or(fallback) {
            kit.push(format!("{} = {}", q(&game), list(&[knife.to_string()])));
            let of = knife.split_once(':').map_or(knife, |(g, _)| g);
            if !sets.iter().any(|s| s == of) {
                sets.push(of.to_string());
            }
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
    Ok(format!(
        "# An automatic gun game (chimera_sim::minigame::auto), seed {seed}.\n\
         id = {id}\nname = {name}\n\
         description = \"Climb a ladder of every game's guns; each gun makes you its game.\"\n\
         players = {{ min = 2, max = 16 }}\nmaps = {maps}\nteams = \"free_for_all\"\n\
         time_limit = {time}\nrotate = true\n{rotation}\n\
         [slots]\n{hud}binds = {base}\ntick = {base}\nbots = {base}\n{movement}\
         identity = \"weapon\"\nweapons = {sets}\n\n\
         [spawn]\nrespawn = {{ delay = \"2s\", protection = \"2s\" }}\npoints = \"random_hidden\"\n\
         min_distance = 20.32\nkit = {kit}\n\n\
         [rules]\nmodule = \"chimera:ladder\"\nladder = {ladder}\nadvance_on = \"kill\"\n\
         demote_on = \"melee_death\"\n{finale}refill_ammo = true\nwin = {{ kind = \"ladder_end\" }}\n",
        seed = opts.seed,
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

    #[test]
    fn a_ladder_climbs_the_bands_and_takes_every_game() {
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
                for g in ["a", "b", "c"] {
                    assert!(l.rungs.iter().any(|id| id.starts_with(&format!("{g}:"))), "{g} in {l:?}");
                }
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
            curve: PowerCurve::shipped(),
        };
        let caps = [a, c, d];
        let refs: Vec<&Capabilities> = caps.iter().collect();
        let text = definition(&caps, &powers_of(&refs), &opts, &|g| g != "c").unwrap();
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
        assert!(definition(&caps, &powers_of(&refs), &opts, &|_| false).is_err());
    }

    #[test]
    fn installed_maps_join_by_pattern() {
        static P: GameParts = GameParts {
            game: "g",
            guns: &[],
            knife: None,
            movement: None,
            characters: None,
            hud: None,
            maps: &["g:de_one"],
            map_patterns: &["de_*"],
            announcer: None,
            match_won_sound: None,
            match_lost_sound: None,
            music: false,
            cues: None,
        };
        let installed: Vec<String> = ["de_one", "DE_Two", "cs_three", "test_x"].iter().map(|s| s.to_string()).collect();
        let c = Capabilities::with_maps(&P, &WeaponCatalog::default(), None, &installed);
        assert_eq!(c.maps, ["g:de_one", "g:de_two"]);
    }
}
