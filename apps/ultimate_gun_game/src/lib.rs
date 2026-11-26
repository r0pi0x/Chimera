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
//! `ugg_new <seed>` replays one; `CHIMERA_RUNG_SEED=<n>` fixes the seed
//! of the rungs' extras (bodies, attachments, melee: `party_roster::launch_rung_seed`); the default plugin (tests) keeps `SEED`.
//! Its ladder has `ugg_rungs` rungs in all (`UggRungs`; `RUNGS` at launch,
//! the curve's `[rungs]` bounds), drawn from every gun of the games
//! every connected player has (`minigame_net::installed_by_all`, L3's
//! mount reports; their install's weapons too; never one that can't hurt
//! a player nor one a game's data excludes), under the host's Guns rules
//! (`auto::draw_pool`, `gun_rules`; logged with the ladder, a fallback in
//! `GunRulesNote`) on the power curve
//! (`auto::ladder`, `minigame::power`: warm-up pistols to melee, each
//! band's guns by power score); its pool every game's maps, their installs' too
//! but those a game's data/map_pool.toml denies (`auto::find_installed_maps`,
//! the apps only), rotated at random but the last `AVOID_RECENT` played
//! (`minigame::rotation`, seeded by the launch seed), drawn by the players
//! present (`minigame::map_fit`: maps too big for them out, the rest
//! weighted), the players voting for the next one on the end screen
//! (`map_vote`, `minigame::vote`: the rotation's pick and three others,
//! 20 s, ties to the pool's pick);
//! its kit each gun's game's knife (none for a game without one, Halo's
//! melee the held gun's bash: never another game's); no gun off the ground, any game's, placed or dropped
//! (`[pickups] weapons = false`: the ladder alone arms a player; a map's
//! guns aren't placed, since a gun lying there that can't be taken would only
//! mislead). `parts` (console) prints each game's capability report. A body
//! shown stays until the next is ready and a character whose game has no
//! body here wears another's (`game_parts::PreferRealModels`); Chimera's
//! loading stand-ins (`render::stand_in`) show only where nothing real can
//! yet, a ladder's models loading ahead (Combat Arms' `characters::preload`).
//!
//! `APP` (feature `present`): the binary's client app, the party's
//! (`party_roster::client::PARTY`) with its own command line (`args`), the
//! party's main-menu start over a background map (never the greybox or a
//! stand-in behind it) and its last word (`launch`: the main menu, the
//! plugin with a fresh seed, the minimal HUD, the Signal look: `signal`).
//!
//! Known shortcuts: the ladder isn't made again when a player without a
//! game joins mid-match; with no CS:S install the client starts (a server
//! offering its game files provides them: `party_roster::served_by_servers`),
//! but its own first map (the launch's, CS:S's) doesn't load; the
//! installs' maps are the host's (a client lacking one gets it as any
//! server map: CS:S's by download).

pub mod sandbox;
#[cfg(feature = "present")]
pub mod signal;

use bevy::prelude::*;
use chimera_core::{
    console::{Console, ConsoleAppExt, Level},
    game_parts::PartsRegistry,
};
use chimera_net::minigame_net::NextMatchHooks;
use chimera_sim::minigame::gun_rules::GunRules;
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
        // The players vote for the next map on the end screen: four
        // cards, one 20 s countdown that is also the next match's start
        // (the winner named for 1.6 s: the logo pass begins 1 s after the
        // close and covers the change at its swap).
        map_vote: Some("{ candidates = 4, time = 20, reveal = 1.6 }".into()),
        // The win freezes everyone for 2 s and goes straight to the
        // results (no intermission): the winner moment's card holds over
        // the frozen world meanwhile, and the match end with its vote
        // comes as the card moves into place (chimera_client's
        // winner_moment).
        end: Some("{ round_over = 2, intermission = 0 }".into()),
        // The ladder alone arms a player (the user's decision): no gun of
        // any game off the ground; other pickups stay possible.
        weapon_pickups: false,
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
    party_roster::GAME_PARTS.iter().map(|p| Capabilities::of(p, &empty, None)).collect()
}

/// The first map of minigame `id`'s pool: the automatic gun game's (every
/// game's maps, in turn), else an original's (`party_roster::first_map`).
pub fn first_map(id: &str) -> Result<String, String> {
    if id == MINIGAME {
        return Ok(auto::map_pool(&claimed(), &|_| true).remove(0));
    }
    party_roster::first_map(id)
}

/// The automatic gun game from the games' parts in `world`, its ladder of
/// `rungs` from `seed` over the games every player has, added as
/// `MINIGAME` (replacing one made before).
pub fn generate(world: &mut World, seed: u64, rungs: usize) -> Result<&'static MinigameDef, String> {
    generate_as(world, seed, rungs, false)
}

/// `generate`; `again`: the ladder of the match already made, drawn again
/// (the installs became known), not a new one for the recency memory
/// (`auto::Recency`: guns dealt in the server's recent ladders are less
/// likely; `ugg_new <seed>` replays a ladder only if that memory is the
/// same, or `recency = 0` in data/power_curve.toml).
fn generate_as(world: &mut World, seed: u64, rungs: usize, again: bool) -> Result<&'static MinigameDef, String> {
    if again {
        world.get_resource_or_init::<auto::Recency>().rewind();
    }
    let caps = auto::report(world);
    let games: Vec<&'static str> = caps.iter().map(|c| c.game).collect();
    let installed = chimera_net::minigame_net::installed_by_all(world, &games);
    // Only guns that can hurt someone, no game's data leaves out and every
    // player has, then the host's Guns rules (`auto::draw_pool`,
    // `gun_rules`).
    let (pool, families) = auto::draw_pool(world, caps.clone(), &|g| installed.contains(&g));
    if !pool.skipped.is_empty() {
        info!("Ultimate Gun Game: {}", auto::skipped_lines(&pool.skipped));
    }
    if let Some(note) = &pool.fallback {
        warn!("Ultimate Gun Game: {note}");
    }
    world.insert_resource(chimera_sim::minigame::gun_rules::GunRulesNote(pool.fallback.clone()));
    let short = world.get_resource::<chimera_sim::minigame::gun_rules::GunRules>().map(|r| r.short).unwrap_or_default();
    let rules = auto::DrawRules {
        short,
        families: Some(&families),
        reserve: &pool.reserve,
    };
    // Every game stays in `pool.caps` (its maps too), its guns or none.
    let caps = &pool.caps;
    let opts = options(seed, rungs);
    let powers = {
        let empty = WeaponCatalog::default();
        let catalog = world.get_resource::<WeaponCatalog>().unwrap_or(&empty);
        let readers = world.get_resource::<PowerReaders>().cloned().unwrap_or_default();
        let all: Vec<Capabilities> = caps.iter().chain(&pool.reserve).cloned().collect();
        auto::powers(&all, catalog, &readers, &opts.curve)
    };
    let recent = world.get_resource::<auto::Recency>().cloned().unwrap_or_default();
    let text = auto::definition_with(caps, &powers, &opts, &recent, &|g| installed.contains(&g), &rules)?;
    let def = minigame::add(world, MINIGAME, &text)?;
    // What was dealt is remembered for the next ladders.
    let dealt: Vec<&str> = def
        .rules
        .params
        .get("ladder")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    world.get_resource_or_init::<auto::Recency>().record(dealt, &opts.curve.draw);
    let host_rules = world.get_resource::<GunRules>().cloned().unwrap_or_default();
    let setting = world.get_resource::<UggRungs>().map_or(rungs, |r| r.0);
    world.insert_resource(Drawn { rules: host_rules, setting, rungs });
    Ok(def)
}

/// The Guns rules and rung count the ladder in `Minigames` was drawn with.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
struct Drawn {
    rules: GunRules,
    /// `ugg_rungs` then.
    setting: usize,
    /// The rungs asked of the draw (`AutoLadder`).
    rungs: usize,
}

/// A match of `id` is about to start (`MatchStarting`): the ladder drawn
/// again, same seed, if the host's Guns rules or `ugg_rungs` changed since
/// it was drawn. The launch ladder is made before the command line's `+`
/// lines run, so `+mp_guns_games_off hl2dm` reaches it only here.
fn redraw_if_rules_changed(world: &mut World, id: &str) {
    if id != MINIGAME {
        return;
    }
    let rules = world.get_resource::<GunRules>().cloned().unwrap_or_default();
    let Some(drawn) = world.get_resource::<Drawn>().cloned() else { return };
    let setting = world.get_resource::<UggRungs>().map_or(drawn.setting, |r| r.0);
    if drawn.rules == rules && drawn.setting == setting {
        return;
    }
    // A changed `ugg_rungs` sets the length; else the draw's own stays.
    let now = Drawn { rules, setting, rungs: if drawn.setting == setting { drawn.rungs } else { setting } };
    let seed = world.resource::<AutoLadder>().seed;
    let rungs = chimera_sim::minigame::power::PowerCurve::shipped().rungs.clamp(now.rungs);
    world.insert_resource(AutoLadder { seed, rungs });
    if let Err(e) = generate_as(world, seed, rungs, true) {
        world.resource_mut::<Console>().print(Level::Error, e);
    }
}

/// The seed and rungs the definition was last made with.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoLadder {
    pub seed: u64,
    pub rungs: usize,
}

/// `ugg_rungs` (`gun_rules::LadderRungs`): the host's ladder length,
/// rungs in all with the knife finale's (the Guns dialog's Rungs stepper;
/// within `[rungs]` of chimera/sim/data/power_curve.toml, default 13).
/// From the next ladder.
pub use chimera_sim::minigame::gun_rules::LadderRungs as UggRungs;

/// The ladder of `l` made and said on the console and in the log; its
/// rungs within the curve's bounds (`RungBounds`), a note when moved.
fn make(world: &mut World, mut l: AutoLadder, why: &str) -> Result<&'static MinigameDef, String> {
    let bounds = chimera_sim::minigame::power::PowerCurve::shipped().rungs;
    let rungs = bounds.clamp(l.rungs);
    if rungs != l.rungs {
        let note = format!("Ultimate Gun Game: {} rungs is outside {} to {}: {rungs}", l.rungs, bounds.min, bounds.max);
        info!("{note}");
        world.resource_mut::<Console>().print(Level::Warn, note);
        l.rungs = rungs;
    }
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
        rungs: world.get_resource::<UggRungs>().map_or(last.rungs, |r| r.0),
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
    /// The seed of the rungs' extras (bodies, attachments, melee), when
    /// not the ladder's own: the apps take `CHIMERA_RUNG_SEED`'s.
    pub rung_seed: Option<u64>,
}

impl Default for UltimateGunGamePlugin {
    fn default() -> Self {
        Self {
            seed: SEED,
            rungs: RUNGS,
            find_maps: false,
            rung_seed: None,
        }
    }
}

impl UltimateGunGamePlugin {
    /// The apps': a fresh seed (`random_seed`), the installs' maps, the
    /// rung seed `CHIMERA_RUNG_SEED` fixes (else the ladder's seed).
    pub fn launch() -> Self {
        Self {
            seed: random_seed(),
            rungs: RUNGS,
            find_maps: true,
            rung_seed: party_roster::rung_seed_override(),
        }
    }
}

/// The app's id on a hub (`chimera_net::net::hub::HubGame`).
pub const HUB_GAME: &str = "ultimate_gun_game";

impl Plugin for UltimateGunGamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AutoLadder {
            seed: self.seed,
            rungs: self.rungs,
        })
        // The rotation's random picks follow the launch's seed too, as
        // the rungs' extras do (`ladder::RungSeed`).
        .insert_resource(UggRungs(self.rungs))
        .insert_resource(MapHistory::seeded(self.seed))
        // The app's game on a hub: what its servers' heartbeats say and
        // the Internet tab lists (docs/plans/active/hub.md 4.3).
        .insert_resource(chimera_net::net::hub::HubGame(HUB_GAME.into()))
        .insert_resource(chimera_sim::minigame::ladder::RungSeed(self.rung_seed.unwrap_or(self.seed)))
        // Any real body rather than a stand-in (`game_parts::PreferRealModels`).
        .init_resource::<chimera_core::game_parts::PreferRealModels>()
        .console_command(
            "chimera:ugg_new",
            "ugg_new [seed] [rungs] [map]: a new Ultimate Gun Game ladder for the players connected, played at once \
             (on `map`, else this one if in its pool, else its pool's first).",
            |w, a| {
                let last = *w.resource::<AutoLadder>();
                let seed = a.first().map(|s| s.parse().map_err(|_| format!("not a seed: {s}"))).transpose()?;
                let rungs = a.get(1).map(|s| s.parse().map_err(|_| format!("not a count: {s}"))).transpose()?;
                let ladder = AutoLadder {
                    seed: seed.unwrap_or_else(|| next_seed(last.seed)),
                    rungs: rungs.unwrap_or_else(|| w.get_resource::<UggRungs>().map_or(last.rungs, |r| r.0)),
                };
                make(w, ladder, "ugg_new")?;
                chimera_net::minigame_net::change_match(w, MINIGAME, a.get(2).map(String::as_str)).map(Some)
            },
        )
        .insert_resource(chimera_sim::minigame::rung_command::HoldRungs(sandbox::SANDBOX))
        .console_command(
            "chimera:sandbox",
            "sandbox [map | help]: play alone with every game's guns, nothing advancing or ending (`rung` picks the gun; `sandbox help`).",
            |w, a| sandbox::command(w, a, BASE),
        )
        .console_command("chimera:parts", "parts: what each composed game lends (its capability report).", |w, _| {
            Ok(Some(auto::report(w).iter().map(Capabilities::line).collect::<Vec<_>>().join("\n")))
        })
        .add_systems(PostStartup, with_installs);
        chimera_core::console::resource_cvar::<UggRungs, usize>(
            app,
            "ugg_rungs",
            "Server: rungs in an Ultimate Gun Game ladder, the knife finale's too (5 to 40; the Guns dialog's Rungs). From the next ladder.",
            |r| &mut r.0,
        );
        // The match's length (the sandbox has none to speak of): a scene's
        // timer shots, `+mp_timelimit 35`.
        chimera_sim::minigame::overrides::override_cvar(app, "mp_timelimit", "time_limit", "Server: the match's length in seconds.");
        app.world_mut().get_resource_or_init::<NextMatchHooks>().0.push(next_ladder);
        app.add_observer(|e: On<chimera_sim::minigame::MatchStarting>, mut commands: Commands| {
            let id = e.id.clone();
            commands.queue(move |w: &mut World| {
                redraw_if_rules_changed(w, &id);
                sandbox::regenerate_on_start(w, &id, BASE);
            });
        });
    }

    fn finish(&self, app: &mut App) {
        let world = app.world_mut();
        // A game without a HUD of its own shows the base game's.
        world.get_resource_or_init::<PartsRegistry>().fallback_hud = BASE;
        if self.find_maps {
            chimera_core::metrics::boot::within("the installs' maps", || auto::find_installed_maps(world));
        }
        let l = *world.resource::<AutoLadder>();
        chimera_core::metrics::boot::within("the ladder", || {
            let _ = make(world, l, "the ladder");
        });
        chimera_core::metrics::boot::within("the sandbox", || {
            let _ = sandbox::generate(world, BASE);
        });
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
    if let Err(e) = generate_as(world, l.seed, l.rungs, true) {
        world.resource_mut::<Console>().print(Level::Error, e);
    }
    let _ = sandbox::generate(world, BASE);
}

/// The app's own default binds, over every game's (`binds::AppBind`):
/// F10 opens the report dialog (`client::feedback`), in the shared binds
/// and so under every game's layer; CS:S's `quit prompt` there is
/// dropped (quitting is the Escape menu's), and a saved config whose F10
/// still holds it gets the report key once. Y and U open the chat to
/// everyone and to the team (`messagemode`, `messagemode2`: the Signal
/// chat, `client::signal_chat`), as in CS:S; Combat Arms' Enter, F2 and F3
/// open it too.
#[cfg(feature = "present")]
pub const APP_BINDS: &[chimera_client::client::binds::AppBind] = &[
    chimera_client::client::binds::AppBind { key: "f10", command: "feedback", replaces: &["quit prompt"] },
    chimera_client::client::binds::AppBind { key: "y", command: "messagemode", replaces: &[] },
    chimera_client::client::binds::AppBind { key: "u", command: "messagemode2", replaces: &[] },
];

/// The game binary's app (`ultimate_gun_game`, main.rs): the party's
/// client app (`party_roster::client::PARTY`) on Chimera's client app,
/// playing the automatic gun game (`with_defaults_for` with `MINIGAME`
/// and `first_map`) unless it starts at the party's main menu
/// (`party_roster::start_at_menu`: nothing on the command line plays; the
/// menu over its own backdrop, then a background map at hand, never the
/// greybox), with the main menu
/// (Play hosts the automatic gun game, Join connects), the Ultimate Gun
/// Game's parts with a fresh seed (`UltimateGunGamePlugin::launch`) and
/// the minimal HUD last (`launch`).
#[cfg(feature = "present")]
pub static APP: chimera_client::client::app::GameApp = chimera_client::client::app::GameApp {
    bin: "ultimate_gun_game",
    title: "Ultimate Gun Game",
    about: || {
        format!(
            "ultimate_gun_game: a gun game whose ladder is every composed game's guns; each gun makes you its\n\
             game. Starts {MINIGAME} on its first map; `ugg_new [seed] [rungs]` (console) a new ladder, `parts`\n\
             what each game lends. The options are the shell's (below).\n"
        )
    },
    hooks: chimera_client::client::app::GameAppHooks {
        args: Some(args),
        start: Some(party_roster::start_at_menu),
        server: Some("ultimate_gun_game_server"),
        launch: Some(launch),
        ..party_roster::client::PARTY.hooks
    },
    ..party_roster::client::PARTY
};

/// The command line as the app plays it: as given at the main menu (no
/// map, minigame, server or capture on it: `main_menu::starts_at_menu`),
/// else the automatic gun game on its pool's first map unless it names
/// others (`party_roster::with_defaults_for`).
#[cfg(feature = "present")]
pub fn args(args: chimera_client::client::Args) -> chimera_client::client::Args {
    let args = sandbox::args(args);
    if party_roster::main_menu::starts_at_menu(&args) {
        args
    } else {
        party_roster::with_defaults_for(args, MINIGAME, &first_map)
    }
}

/// What the app adds last: the party's main menu (shown at start when the
/// command line doesn't play; Play starts `MINIGAME`), the Ultimate Gun
/// Game's parts with a fresh seed, the minimal HUD, its look (`signal`),
/// the sandbox's screens (`chimera_client::client::sandbox`) and its own
/// binds (`APP_BINDS`).
#[cfg(feature = "present")]
fn launch(app: &mut App, args: &chimera_client::client::Args) {
    let at_menu = party_roster::main_menu::starts_at_menu(args);
    {
        let mut d = app.world_mut().get_resource_or_init::<chimera_client::client::binds::BindDefaults>();
        for b in APP_BINDS {
            d.add_app(*b);
        }
    }
    // The background maps at hand without a download, before the menu.
    app.insert_resource(party_roster::main_menu::BackgroundMaps::app(party_roster::client::background_at_hand))
        .add_plugins(party_roster::main_menu::MainMenuPlugin {
            at_start: at_menu,
            minigame: MINIGAME.into(),
            // A tool run (`+mainmenu` photographing it) keeps nothing. A
            // start that joins (`+connect`: a join link, the first join's
            // relaunch) keeps them too: it comes back to this menu.
            keep_recent: !args.tool_run(),
        })
        .add_plugins(party_roster::menu_backdrop::MenuBackdropPlugin)
    .add_plugins((UltimateGunGamePlugin::launch(), chimera_client::client::hud_minimal::MinimalHudPlugin))
        .add_plugins(signal::SignalPlugin)
        // The sandbox's screens (its HUD, Change gun, the bench).
        .add_plugins(chimera_client::client::sandbox::SandboxUiPlugin)
        .add_systems(Update, sandbox::camera_state);
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_first_map_is_the_pools() {
        assert_eq!(super::first_map(super::MINIGAME).unwrap(), "cs_source:de_dust2");
        assert_eq!(super::first_map(party_roster::DEFAULT_MINIGAME).unwrap(), "cs_source:de_dust2");
    }

    /// F10 reports in the app (over the shared binds, so under every
    /// game's layer); the CS:S app keeps CS:S's quit prompt.
    #[cfg(feature = "present")]
    #[test]
    fn f10_reports_here_and_quits_in_the_cs_source_app() {
        use chimera_client::client::binds::BindDefaults;
        let mut ugg = BindDefaults::default();
        ugg.use_game(&combat_arms::game::INFO);
        for b in super::APP_BINDS {
            ugg.add_app(*b);
        }
        let mut console = chimera_core::console::Console::default();
        chimera_client::client::binds::bind_defaults(&mut console.binds, true, &ugg.default_binds());
        for (game, binds) in &party_roster::GAME_BINDS {
            console.ship_layer(game, binds);
        }
        assert_eq!(party_roster::GAME_BINDS.len(), 4);
        for (game, _) in &party_roster::GAME_BINDS {
            assert_eq!(console.effective_binds(Some(game)).get("f10").map(String::as_str), Some("feedback"), "{game}");
        }
        let mut css = BindDefaults::default();
        let cs = party_roster::BUNDLES.iter().find(|b| b.info.id == "cs_source").expect("CS:S composed").info;
        css.use_game(cs);
        assert!(css.default_binds().contains(&("f10".into(), "quit prompt".into())));
    }

    /// Every action a player on Halo's grenade rung needs (move, fire,
    /// reload, melee, throw, the grenade kind) has a key under Halo's
    /// layer of the party's binds; and a config saved before Halo had a
    /// layer (playtest 2026-11-17: the throw "unbound" on Windows) gets
    /// the throw back on G at start (`bindgamedefaults missing`), while a
    /// throw the player moved to another key stays there.
    #[cfg(feature = "present")]
    #[test]
    fn a_halo_grenade_rung_has_every_key_it_needs_even_after_an_old_config() {
        use chimera_client::client::binds::{BindDefaults, bind_defaults, keys_for};
        let fresh = || {
            let mut ugg = BindDefaults::default();
            ugg.use_game(&combat_arms::game::INFO);
            for b in super::APP_BINDS {
                ugg.add_app(*b);
            }
            let mut console = chimera_core::console::Console::default();
            bind_defaults(&mut console.binds, true, &ugg.default_binds());
            for (game, binds) in &party_roster::GAME_BINDS {
                console.ship_layer(game, binds);
            }
            console
        };
        let halo = "halo_ce";
        let needs = ["+forward", "+back", "+moveleft", "+moveright", "+jump", "+attack", "+reload", "+melee", "+grenade", "+grenade_switch"];
        let check = |console: &chimera_core::console::Console, why: &str| {
            let binds = console.effective_binds(Some(halo));
            for a in needs {
                assert!(!keys_for(&binds, a).is_empty(), "{a} has no key in Halo's binds ({why})");
            }
        };
        check(&fresh(), "fresh");
        // A config written before Halo's layer: `unbindgameall`, then
        // only the other games' layers.
        let mut old = fresh();
        old.layers.remove(halo);
        assert!(keys_for(&old.effective_binds(Some(halo)), "+grenade").is_empty(), "the old config's state");
        assert!(old.restore_missing_layer_binds());
        check(&old, "an old config, restored");
        assert_eq!(keys_for(&old.effective_binds(Some(halo)), "+grenade"), ["g"]);
        assert!(!old.restore_missing_layer_binds(), "once");
        // The throw moved to H by the player: G stays as the shared binds have it.
        let mut moved = fresh();
        moved.layers.get_mut(halo).unwrap().remove("g");
        moved.layers.get_mut(halo).unwrap().insert("h".into(), "+grenade".into());
        moved.restore_missing_layer_binds();
        assert_eq!(keys_for(&moved.effective_binds(Some(halo)), "+grenade"), ["h"]);
    }

    #[test]
    fn the_launch_rungs_are_the_curves_default() {
        assert_eq!(super::RUNGS, chimera_sim::minigame::power::PowerCurve::shipped().rungs.default);
    }

    #[test]
    fn the_rung_seed_is_the_ladders_unless_one_is_fixed() {
        use bevy::prelude::App;
        use chimera_sim::minigame::ladder::RungSeed;
        let seed_of = |plugin: super::UltimateGunGamePlugin| {
            let mut app = App::new();
            app.add_plugins(plugin);
            app.world().resource::<RungSeed>().0
        };
        // No fixed seed: today's, the ladder's own.
        assert_eq!(seed_of(super::UltimateGunGamePlugin { seed: 7, ..Default::default() }), 7);
        // Fixed: the draws follow it, not the ladder's seed.
        let fixed = |seed| super::UltimateGunGamePlugin { seed, rung_seed: Some(42), ..Default::default() };
        assert_eq!(seed_of(fixed(7)), 42);
        assert_eq!(seed_of(fixed(8)), 42);
    }
}
