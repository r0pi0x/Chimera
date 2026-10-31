//! Ultimate Gun Game (apps/ultimate_gun_game, crate `ultimate_gun_game`,
//! binary `ultimate_gun_game`; docs/plans/active/ultimate-gun-game.md): the
//! gun game with guns and maps from every game the app composes; each rung
//! makes the player its gun's game (movement, body, HUD: `identity =
//! "weapon"`). A test bed for games as optional parts: a game takes part
//! with what it lends (`chimera_core::game_parts`), its guns and movement
//! required, its characters, HUD and maps optional (the fallbacks:
//! `chimera_sim::minigame::auto`).
//!
//! Built on the party's glue (`party_kit`: both games composed, the
//! minigame runtime and its network part, the client with both HUDs), not
//! a fork of it: this crate adds only `UltimateGunGamePlugin`, which makes
//! the definition `MINIGAME` from the registered games' parts once the app
//! is built (`Plugin::finish`: the weapon catalogue and movements are
//! there), again with the players' installs at the first frame, at every
//! rotation (`minigame_net::NextMatchHooks`: a new ladder from the next
//! seed, `next_seed`, for the players then connected) and on `ugg_new
//! [seed] [rungs] [map]` (a new ladder for the players connected, played
//! at once: `minigame_net::change_match`). The apps (client and dedicated
//! server, apps/ultimate_gun_game_server: `UltimateGunGamePlugin::launch`) take a fresh seed at launch
//! (`random_seed`), said on the console and in the log with each ladder;
//! `ugg_new <seed>` replays one; the default plugin (tests) keeps `SEED`.
//! Its ladder has `RUNGS` rungs in all, drawn from every gun of the games
//! every connected player has (`minigame_net::installed_by_all`, L3's
//! mount reports; their install's weapons too) on the power curve
//! (`auto::ladder`, `minigame::power`: warm-up pistols to melee, each
//! band's guns by power score); its pool every game's maps, their installs' too
//! (`auto::find_installed_maps`, the apps only), rotated at random but the
//! last `AVOID_RECENT` played (`minigame::rotation`, seeded by the launch
//! seed), the players voting for the next one on the end screen
//! (`map_vote`, `minigame::vote`: the rotation's pick and two others);
//! its kit each gun's game's knife (the base game's, `BASE`, for a
//! game without one). `parts` (console) prints each game's capability
//! report.
//!
//! Known shortcuts: the ladder isn't made again when a player without a
//! game joins mid-match; with no CS:S install the client still stops
//! (`party_kit::requirements`), its pool's first map (the launch's)
//! being CS:S's; the installs' maps are the host's (a client lacking one
//! gets it as any server map: CS:S's by download).

use bevy::prelude::*;
use chimera_core::{
    console::{Console, ConsoleAppExt, Level},
    game_parts::PartsRegistry,
};
use chimera_net::minigame_net::NextMatchHooks;
use chimera_sim::{
    minigame::{
        self, MinigameDef,
        auto::{self, AutoOptions, Capabilities, Rng},
        power::PowerReaders,
        rotation::MapHistory,
    },
    weapon::data::WeaponCatalog,
};

/// The automatic gun game's id.
pub const MINIGAME: &str = "ultimate_gun_game:auto";

/// Rungs in a ladder, the melee finale's too (12 guns and grenades, 1
/// melee rung on the shipped curve).
pub const RUNGS: usize = 13;

/// The seed of `UltimateGunGamePlugin::default()` (the tests'); the apps
/// take a fresh one at launch (`random_seed`).
pub const SEED: u64 = 1;

/// Seeds are below this (short enough to type: `ugg_new <seed>`).
pub const SEEDS: u64 = 1_000_000;

/// The maps a rotation skips: the last this many played.
pub const AVOID_RECENT: usize = 10;

/// The base game: binds, tick and bots, the knife and HUD of a game that
/// lends none, the HUD before a gun's game takes it (Combat Arms', as
/// Lucker Party's).
pub const BASE: &str = "combat_arms";

/// The options of a ladder of `rungs` from `seed`.
pub fn options(seed: u64, rungs: usize) -> AutoOptions {
    AutoOptions {
        id: MINIGAME.into(),
        name: "Ultimate Gun Game".into(),
        rungs,
        seed,
        base: BASE,
        time_limit: 600.0,
        avoid_recent: Some(AVOID_RECENT),
        // The players vote for the next map on the end screen (the
        // built-in settings, chimera/sim/data/map_vote.toml).
        map_vote: Some("true".into()),
        curve: chimera_sim::minigame::power::PowerCurve::shipped(),
    }
}

/// A fresh seed (the clock's, mixed): a launch's.
pub fn random_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    Rng::new(nanos ^ ((std::process::id() as u64) << 32)).next() % SEEDS
}

/// The seed after `seed`: the next match's ladder (a launch's seed
/// decides every ladder after it).
pub fn next_seed(seed: u64) -> u64 {
    Rng::new(seed ^ 0x5EED).next() % SEEDS
}

/// The games' claimed parts as reports without the app (the maps are
/// right; the guns aren't checked): for the command line's first map.
fn claimed() -> Vec<Capabilities> {
    let empty = WeaponCatalog::default();
    party_kit::GAME_PARTS.iter().map(|p| Capabilities::of(p, &empty, None)).collect()
}

/// The first map of minigame `id`'s pool: the automatic gun game's (every
/// game's maps, in turn), else an original's (`party_kit::first_map`).
pub fn first_map(id: &str) -> Result<String, String> {
    if id == MINIGAME {
        return Ok(auto::map_pool(&claimed(), &|_| true).remove(0));
    }
    party_kit::first_map(id)
}

/// The automatic gun game from the games' parts in `world`, its ladder of
/// `rungs` from `seed` over the games every player has, added as
/// `MINIGAME` (replacing one made before).
pub fn generate(world: &mut World, seed: u64, rungs: usize) -> Result<&'static MinigameDef, String> {
    let caps = auto::report(world);
    let games: Vec<&'static str> = caps.iter().map(|c| c.game).collect();
    let installed = chimera_net::minigame_net::installed_by_all(world, &games);
    let opts = options(seed, rungs);
    let powers = {
        let empty = WeaponCatalog::default();
        let catalog = world.get_resource::<WeaponCatalog>().unwrap_or(&empty);
        let readers = world.get_resource::<PowerReaders>().cloned().unwrap_or_default();
        auto::powers(&caps, catalog, &readers, &opts.curve)
    };
    let text = auto::definition(&caps, &powers, &opts, &|g| installed.contains(&g))?;
    minigame::add(world, MINIGAME, &text)
}

/// The seed and rungs the definition was last made with.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoLadder {
    pub seed: u64,
    pub rungs: usize,
}

/// The ladder of `l` made and said on the console and in the log.
fn make(world: &mut World, l: AutoLadder, why: &str) -> Result<&'static MinigameDef, String> {
    world.insert_resource(l);
    let made = generate(world, l.seed, l.rungs);
    let text = match &made {
        Ok(def) => {
            let guns = def.rules.params.get("ladder").and_then(|v| v.as_array()).map_or(0, Vec::len);
            let melee = def.rules.params.get("melee_rungs").and_then(|v| v.as_integer()).unwrap_or(0) as usize;
            let ladder = guns + melee;
            format!("Ultimate Gun Game: {why}: seed {} ({ladder} rungs; `ugg_new {}` replays it)", l.seed, l.seed)
        }
        Err(e) => format!("Ultimate Gun Game: {why}: seed {}: {e}", l.seed),
    };
    info!("{text}");
    let level = if made.is_ok() { Level::Info } else { Level::Error };
    world.resource_mut::<Console>().print(level, text);
    made
}

/// A rotation (`NextMatchHooks`): the next match's ladder from the next
/// seed, for the players then connected.
fn next_ladder(world: &mut World, id: &str, map: &str) {
    if id != MINIGAME {
        return;
    }
    let last = *world.resource::<AutoLadder>();
    let l = AutoLadder {
        seed: next_seed(last.seed),
        rungs: last.rungs,
    };
    let _ = make(world, l, &format!("a new ladder for {map}"));
}

/// The automatic gun game made from the composed games' parts, the
/// fallback HUD for a game without one (the base game's), a new ladder at
/// each rotation (`NextMatchHooks`), and its console commands (`ugg_new`,
/// `parts`).
pub struct UltimateGunGamePlugin {
    pub seed: u64,
    pub rungs: usize,
    /// Look for the maps the installs hold (`auto::find_installed_maps`)
    /// for the pool: the apps do; tests (default) keep the claimed maps.
    pub find_maps: bool,
}

impl Default for UltimateGunGamePlugin {
    fn default() -> Self {
        Self {
            seed: SEED,
            rungs: RUNGS,
            find_maps: false,
        }
    }
}

impl UltimateGunGamePlugin {
    /// The apps': a fresh seed (`random_seed`), the installs' maps.
    pub fn launch() -> Self {
        Self {
            seed: random_seed(),
            rungs: RUNGS,
            find_maps: true,
        }
    }
}

impl Plugin for UltimateGunGamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AutoLadder {
            seed: self.seed,
            rungs: self.rungs,
        })
        // The rotation's random picks follow the launch's seed too, as
        // the rungs' extras do (`ladder::RungSeed`).
        .insert_resource(MapHistory::seeded(self.seed))
        .insert_resource(chimera_sim::minigame::ladder::RungSeed(self.seed))
        .console_command(
            "ugg_new",
            "ugg_new [seed] [rungs] [map]: a new Ultimate Gun Game ladder for the players connected, played at once \
             (on `map`, else this one if in its pool, else its pool's first).",
            |w, a| {
                let last = *w.resource::<AutoLadder>();
                let seed = a.first().map(|s| s.parse().map_err(|_| format!("not a seed: {s}"))).transpose()?;
                let rungs = a.get(1).map(|s| s.parse().map_err(|_| format!("not a count: {s}"))).transpose()?;
                let ladder = AutoLadder {
                    seed: seed.unwrap_or_else(|| next_seed(last.seed)),
                    rungs: rungs.unwrap_or(last.rungs),
                };
                make(w, ladder, "ugg_new")?;
                chimera_net::minigame_net::change_match(w, MINIGAME, a.get(2).map(String::as_str)).map(Some)
            },
        )
        .console_command("parts", "parts: what each composed game lends (its capability report).", |w, _| {
            Ok(Some(auto::report(w).iter().map(Capabilities::line).collect::<Vec<_>>().join("\n")))
        })
        .add_systems(PostStartup, with_installs);
        app.world_mut().get_resource_or_init::<NextMatchHooks>().0.push(next_ladder);
    }

    fn finish(&self, app: &mut App) {
        let world = app.world_mut();
        // A game without a HUD of its own shows the base game's.
        world.get_resource_or_init::<PartsRegistry>().fallback_hud = BASE;
        if self.find_maps {
            auto::find_installed_maps(world);
        }
        let l = *world.resource::<AutoLadder>();
        let _ = make(world, l, "the ladder");
    }
}

/// The ladder again once this machine's mount doctor has reported
/// (`LocalMounts`, inserted in a client's Startup): its own installs
/// filter it too.
fn with_installs(world: &mut World) {
    if !world.contains_resource::<chimera_net::net::manifest::LocalMounts>() {
        return;
    }
    let l = *world.resource::<AutoLadder>();
    if let Err(e) = generate(world, l.seed, l.rungs) {
        world.resource_mut::<Console>().print(Level::Error, e);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_first_map_is_the_pools() {
        assert_eq!(super::first_map(super::MINIGAME).unwrap(), "cs_source:de_dust2");
        assert_eq!(super::first_map(party_kit::DEFAULT_MINIGAME).unwrap(), "cs_source:de_dust2");
    }
}
