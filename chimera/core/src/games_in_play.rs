//! Which games are in play, and each game's systems stopped while its game
//! is not (docs/plans/active/game-logic-distribution.md, option a). An app
//! made of several games (a party: `party_kit`) registers every game's
//! systems, but a match plays a few of them: the others' systems would run
//! every tick for nothing (23% of a tick, measured).
//!
//! - **Gating** (`GateGameExt::gate_game`): the systems a game's plugins add
//!   inside `gate_game(game, ..)` whose type path starts in the game's own
//!   crate (`hl2dm::...`; generic engine systems for its types and shared
//!   family crates stay ungated) get one more run condition, the game's
//!   switch: an atomic flag shared with `GamesInPlay`, read in a few
//!   instructions without any world access. Every per-tick schedule is
//!   gated; startup and state-transition schedules (run once) are not.
//!   Bevy 0.20 has no way to put systems already added into a set, so the
//!   condition goes on each system (the same effect: a skipped system's
//!   queries never run). A system a game names with `serves_every_game`
//!   stays ungated (work it does for other games too). `gate_since` is the
//!   same for systems added since a moment (`systems_now`) in a list of
//!   crates: a party's client gates each guest's front end with it (its
//!   `GameClient::ui_crate` and its game's crate, per frame:
//!   `party_kit::client::gate_front_ends`), and `LateGatePlugin` (added
//!   with the first `gate_game`) the systems plugins add in
//!   `Plugin::finish`, in each gated game's crates.
//! - **In play**: a game enters play, and its systems run from then on,
//!   when anything of it appears: a character takes its `GameIdentity`
//!   (an observer, so the same tick), a weapon of it is made (its id's
//!   `game:` prefix: `chimera_sim`'s observer), the match's ladder names
//!   its guns (`chimera_sim`), its map spawns (`chimera_world`), or any
//!   entity carries one of its crate's components that no other crate's
//!   component requires (`watch_entities`, in `First`: a map's entities, a
//!   projectile, a test's own spawns; every game's parts on every
//!   character, `on_every_character`, are no sign). The base
//!   game of a composition is never gated. A game stays in play for the
//!   app's lifetime: what it put on characters, projectiles in flight and
//!   its timers keep being handled as before, so behaviour in play is
//!   unchanged. Apps without `GamesInPlay` gate nothing.
//! - **Each game's characters** (`GameCharacters`): a game in play beside
//!   others loops over its own characters only.
//! - **First entry** (`GameEntersPlay`, triggered): work a game wants done
//!   only once it is in play (its data documents) observes it.
//!
//! Known shortcuts: systems a game's plugins add in `Plugin::cleanup`, or
//! outside `gate_game` (and a client's front-end window), are not gated;
//! `Plugin::finish`'s are gated by crate alone (a game's plugin adding
//! another gated game's system there would gate it on that game); games
//! never leave play (a long-lived server that has played every game runs
//! them all); the front ends' install reads at `Startup` (kill icons, gun
//! names) are not deferred to `GameEntersPlay`: the main menu's Ladder
//! backdrop and gun lists show every game's guns before any is in play.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use bevy::{
    ecs::{
        archetype::ArchetypeId,
        component::ComponentId,
        resource::IsResource,
        schedule::{ConditionWithAccess, Schedules, SystemKey},
        system::ReadOnlySystem,
    },
    prelude::*,
};

/// One gated game: its id and its switch.
struct Gated {
    game: &'static str,
    flag: Arc<AtomicBool>,
}

/// Triggered when a gated game first enters play: what a game wants done
/// only once it plays (its data documents) observes it. A game not gated
/// is in play from the start and never triggers it
/// (`GamesInPlay::is_in_play` says so).
#[derive(Event, Clone, Debug)]
pub struct GameEntersPlay {
    pub game: &'static str,
}

/// The games whose systems are gated (`GateGameExt::gate_game`) and which
/// of them are in play (module doc).
#[derive(Resource, Default)]
pub struct GamesInPlay {
    gated: Vec<Gated>,
    /// The game of a map named without a `game:` prefix (the app's first
    /// map loader's), set by the composer.
    pub bare_maps: Option<&'static str>,
    /// What put each game in play, in order (for reports and tests).
    pub why: Vec<(String, String)>,
    /// Systems (by type) that serve every game: never gated
    /// (`serves_every_game`).
    every: Vec<std::any::TypeId>,
    /// The crates whose systems each game's switch gates (its own, its
    /// front end's: `gate_since`).
    crates: Vec<(&'static str, &'static str)>,
    /// Archetypes `watch_entities` has looked at.
    seen: usize,
    /// Archetypes that hold a component of a gated game not in play yet.
    watched: Vec<(ArchetypeId, usize)>,
}

impl GamesInPlay {
    fn index(&self, game: &str) -> Option<usize> {
        self.gated.iter().position(|g| g.game.eq_ignore_ascii_case(game))
    }

    fn flag(&mut self, game: &'static str) -> Arc<AtomicBool> {
        let i = self.index(game).unwrap_or_else(|| {
            self.gated.push(Gated { game, flag: Arc::default() });
            self.gated.len() - 1
        });
        self.gated[i].flag.clone()
    }

    /// Whether `game` is in play: true for a game that is not gated.
    pub fn is_in_play(&self, game: &str) -> bool {
        self.index(game).is_none_or(|i| self.gated[i].flag.load(Ordering::Relaxed))
    }

    /// The gated games, each with whether it is in play.
    pub fn games(&self) -> impl Iterator<Item = (&'static str, bool)> + '_ {
        self.gated.iter().map(|g| (g.game, g.flag.load(Ordering::Relaxed)))
    }

    /// Puts `game` in play; the game's id if it was gated and out of play.
    fn enter_flag(&mut self, game: &str, why: &str) -> Option<&'static str> {
        let i = self.index(game)?;
        if self.gated[i].flag.swap(true, Ordering::Relaxed) {
            return None;
        }
        self.watched.retain(|&(_, w)| w != i);
        let game = self.gated[i].game;
        self.why.push((game.to_string(), why.to_string()));
        Some(game)
    }
}

/// `system`, a game's, serves every game, in play or not (CS:S's explosion
/// looks also draw a map's `env_explosion`, of no game): `gate_game` leaves
/// it ungated. Called in the game's plugin's `build` (inside its
/// `gate_game`; nothing in an app that gates no games). Use it rarely:
/// most of a game's work concerns only its own characters and things.
pub fn serves_every_game<M>(app: &mut App, system: impl IntoSystem<(), (), M>) {
    let ty = IntoSystem::into_system(system).system_type();
    if let Some(mut games) = app.world_mut().get_resource_mut::<GamesInPlay>() {
        games.every.push(ty);
    }
}

/// Puts `game` (a game id, any case) in play now: its gated systems run
/// from the next one on (module doc), `GameEntersPlay` is triggered.
/// Nothing in an app that gates no games. `why` says what put it in play
/// (`GamesInPlay::why`).
pub fn enter(world: &mut World, game: &str, why: &str) {
    let Some(game) = world.get_resource_mut::<GamesInPlay>().and_then(|mut g| g.enter_flag(game, why)) else {
        return;
    };
    debug!("game {game} enters play: {why}");
    world.trigger(GameEntersPlay { game });
}

/// Map `name` (`game:map`, or bare: `GamesInPlay::bare_maps`) spawns:
/// its game enters play.
pub fn enter_map(world: &mut World, name: &str) {
    let Some(games) = world.get_resource::<GamesInPlay>() else { return };
    if let Some(game) = game_of(name).map(str::to_string).or_else(|| games.bare_maps.map(str::to_string)) {
        enter(world, &game, &format!("its map {name}"));
    }
}

/// `enter` from where only `Commands` are at hand: at once when the game
/// is already in play or not gated, else when the commands apply.
pub fn enter_later(commands: &mut Commands, games: Option<&GamesInPlay>, game: &str, why: impl FnOnce() -> String) {
    if games.is_none_or(|g| g.is_in_play(game)) {
        return;
    }
    let (game, why) = (game.to_string(), why());
    commands.queue(move |w: &mut World| enter(w, &game, &why));
}

/// The game of an id with a `game:` prefix (`hl2dm` of `hl2dm:weapon_ar2`).
pub fn game_of(id: &str) -> Option<&str> {
    id.split_once(':').map(|(g, _)| g).filter(|g| !g.is_empty())
}

/// The crate a type path starts in (`<impl ...>`, references and closures
/// included).
pub fn crate_of(path: &str) -> &str {
    path.trim_start_matches(['<', '&']).split("::").next().unwrap_or("")
}

/// Schedules that run once (startup, state transitions): never gated.
fn runs_once(label: &str) -> bool {
    ["PreStartup", "Startup", "PostStartup", "StateTransition", "OnEnter", "OnExit", "OnTransition"]
        .iter()
        .any(|s| label.starts_with(s))
}

/// Each schedule's systems at a moment (`systems_now`): what was added
/// after it is gated by `gate_since`.
#[derive(Resource, Default, Clone)]
pub struct SystemsAt(Vec<(String, Vec<SystemKey>)>);

/// The systems registered now (`gate_since`'s start).
pub fn systems_now(app: &App) -> SystemsAt {
    SystemsAt(keys(app.world()))
}

/// Gives every system added since `since` whose type path starts in one of
/// `crates` game `game`'s switch as a run condition (per-tick and per-frame
/// schedules; not those that run once, nor `serves_every_game`'s); the
/// crates are remembered as the game's, for the systems plugins add later
/// in `Plugin::finish` (`LateGatePlugin`). Nothing in an app that gates no
/// games (`GateGameExt::gate_game` starts gating). Returns how many systems
/// it gated.
pub fn gate_since(app: &mut App, since: &SystemsAt, game: &'static str, crates: &[&'static str]) -> usize {
    let Some(mut games) = app.world_mut().get_resource_mut::<GamesInPlay>() else {
        return 0;
    };
    let flag = games.flag(game);
    for c in crates {
        if !games.crates.contains(&(game, *c)) {
            games.crates.push((game, *c));
        }
    }
    let every = games.every.clone();
    let mut gated = 0;
    app.world_mut().resource_scope(|world, mut schedules: Mut<Schedules>| {
        for (label, schedule) in schedules.iter_mut() {
            let label = format!("{label:?}");
            if runs_once(&label) {
                continue;
            }
            let old = since.0.iter().find(|(l, _)| *l == label).map(|(_, k)| k);
            let systems = &mut schedule.graph_mut().systems;
            let new: Vec<SystemKey> = systems
                .iter()
                .filter(|(k, s, _)| {
                    old.is_none_or(|o| !o.contains(k))
                        && crates.contains(&crate_of(&s.name().as_string()))
                        && !every.contains(&s.system_type())
                })
                .map(|(k, _, _)| k)
                .collect();
            for key in new {
                if let Some(s) = systems.get(key) {
                    trace!("game {game}: {label} {} gated", s.name());
                }
                let Some(conditions) = systems.get_conditions_mut(key) else { continue };
                let flag = flag.clone();
                let mut condition: Box<dyn ReadOnlySystem<In = (), Out = bool>> =
                    Box::new(IntoSystem::into_system(move || flag.load(Ordering::Relaxed)));
                // Initialized now: the schedule may have initialized the
                // system already (a second initialization is harmless).
                let access = condition.initialize(world);
                let mut c = ConditionWithAccess::new(condition);
                c.access = access;
                conditions.push(c);
                gated += 1;
            }
        }
    });
    debug!("game {game}: {gated} systems of {crates:?} gated");
    gated
}

/// Gating a game's systems on an app (module doc).
pub trait GateGameExt {
    /// Runs `add` (a game's plugins, systems), then gives the systems it
    /// added whose type path starts in crate `game` the game's switch as a
    /// run condition (`gate_since`). The first call inserts `GamesInPlay`,
    /// its watch and `LateGatePlugin`.
    fn gate_game(&mut self, game: &'static str, add: impl FnOnce(&mut App)) -> &mut Self;
}

impl GateGameExt for App {
    fn gate_game(&mut self, game: &'static str, add: impl FnOnce(&mut App)) -> &mut Self {
        if !self.world().contains_resource::<GamesInPlay>() {
            self.init_resource::<GamesInPlay>();
            self.add_systems(First, watch_entities);
            self.add_observer(identity_enters);
            self.add_plugins(LateGatePlugin);
        }
        let before = systems_now(self);
        add(self);
        gate_since(self, &before, game, &[game]);
        self
    }
}

/// Gates what the games' plugins add in `Plugin::finish`: its own `finish`
/// (run in plugin order: it is added with the first `gate_game`, before the
/// games' plugins) notes the systems there are, its `cleanup` (after every
/// plugin's `finish`) gates those added since in each gated game's crates
/// (`gate_since`'s). Systems added in `cleanup` stay ungated.
pub struct LateGatePlugin;

impl Plugin for LateGatePlugin {
    fn build(&self, _: &mut App) {}

    fn finish(&self, app: &mut App) {
        let at = systems_now(app);
        app.insert_resource(at);
    }

    fn cleanup(&self, app: &mut App) {
        let Some(at) = app.world_mut().remove_resource::<SystemsAt>() else { return };
        let Some(games) = app.world().get_resource::<GamesInPlay>() else { return };
        let mut by_game: Vec<(&'static str, Vec<&'static str>)> = Vec::new();
        for &(game, c) in &games.crates {
            match by_game.iter_mut().find(|(g, _)| *g == game) {
                Some((_, cs)) => cs.push(c),
                None => by_game.push((game, vec![c])),
            }
        }
        for (game, crates) in by_game {
            gate_since(app, &at, game, &crates);
        }
    }
}

/// Each schedule's system keys (to tell what was added since).
fn keys(world: &World) -> Vec<(String, Vec<SystemKey>)> {
    world.get_resource::<Schedules>().map_or_else(Vec::new, |s| {
        s.iter()
            .map(|(label, schedule)| (format!("{label:?}"), schedule.graph().systems.iter().map(|(k, _, _)| k).collect()))
            .collect()
    })
}

/// A character taking a gated game's identity puts the game in play at once.
fn identity_enters(
    on: On<Insert<crate::slots::GameIdentity>>,
    identities: Query<&crate::slots::GameIdentity>,
    games: Res<GamesInPlay>,
    mut commands: Commands,
) {
    if let Ok(identity) = identities.get(on.event_target()) {
        enter_later(&mut commands, Some(&games), &identity.0, || format!("{} took its identity", on.event_target()));
    }
}

/// Each tick (`First`): a game with any entity that holds one of its
/// crate's components enters play (resources aside, and components another
/// crate's require). New archetypes are looked at once; then only the
/// watched ones' lengths are read.
fn watch_entities(
    mut games: ResMut<GamesInPlay>,
    archetypes: &bevy::ecs::archetype::Archetypes,
    components: &bevy::ecs::component::Components,
    mut commands: Commands,
) {
    let games = &mut *games;
    if games.seen == archetypes.len() && games.watched.is_empty() {
        return;
    }
    if games.gated.iter().all(|g| g.flag.load(Ordering::Relaxed)) {
        games.seen = archetypes.len();
        return;
    }
    let resource = components.component_id::<IsResource>();
    for archetype in archetypes.iter().skip(games.seen) {
        if resource.is_some_and(|r| archetype.contains(r)) {
            continue;
        }
        // A component another crate's requires (every game's parts on
        // every character: `on_every_character`, required by `Intent`)
        // says nothing of its game: only one the game put on is a sign.
        let owned: Vec<(ComponentId, String)> = archetype
            .iter_components()
            .filter_map(|c| Some((c, crate_of(&components.get_name(c)?).to_string())))
            .collect();
        for (c, krate) in &owned {
            let Some(i) = games.gated.iter().position(|g| !g.flag.load(Ordering::Relaxed) && g.game == krate) else {
                continue;
            };
            let required = owned.iter().any(|(o, k)| {
                k != krate
                    && components
                        .get_info(*o)
                        .is_some_and(|info| info.required_components().iter_ids().any(|r| r == *c))
            });
            if !required && !games.watched.contains(&(archetype.id(), i)) {
                games.watched.push((archetype.id(), i));
            }
        }
    }
    games.seen = archetypes.len();
    let mut entered: Vec<&'static str> = Vec::new();
    for &(a, i) in &games.watched {
        let game = games.gated[i].game;
        if let Some(a) = archetypes.get(a).filter(|a| !a.is_empty())
            && !entered.contains(&game)
        {
            entered.push(game);
            let names: Vec<String> =
                a.iter_components().filter_map(|c| components.get_name(c)).map(|n| n.as_string()).collect();
            let why = format!("entity {} holds {}", a.entities()[0].id(), names.join(", "));
            commands.queue(move |w: &mut World| enter(w, game, &why));
        }
    }
}

/// Each game's characters (by `GameIdentity`), kept as identities come,
/// change and go: a game's per-character loop visits its own characters
/// only (`of`, then `Query::get`), instead of scanning every character and
/// checking whose it is. No component of its own, so no new archetypes.
/// `track_game_characters` installs it (any app; once).
#[derive(Resource, Default, Debug)]
pub struct GameCharacters(Vec<(String, Vec<Entity>)>);

impl GameCharacters {
    /// `game`'s characters (any case), by entity.
    pub fn of(&self, game: &str) -> &[Entity] {
        self.0.iter().find(|(g, _)| g.eq_ignore_ascii_case(game)).map_or(&[], |(_, v)| v.as_slice())
    }

    fn add(&mut self, game: &str, e: Entity) {
        let i = self.0.iter().position(|(g, _)| g.eq_ignore_ascii_case(game)).unwrap_or_else(|| {
            self.0.push((game.to_ascii_lowercase(), Vec::new()));
            self.0.len() - 1
        });
        let list = &mut self.0[i].1;
        if let Err(at) = list.binary_search(&e) {
            list.insert(at, e);
        }
    }

    fn remove(&mut self, game: &str, e: Entity) {
        if let Some((_, list)) = self.0.iter_mut().find(|(g, _)| g.eq_ignore_ascii_case(game))
            && let Ok(at) = list.binary_search(&e)
        {
            list.remove(at);
        }
    }
}

/// Keeps `GameCharacters` (once per app).
pub fn track_game_characters(app: &mut App) {
    if app.world().contains_resource::<GameCharacters>() {
        return;
    }
    app.init_resource::<GameCharacters>().add_observer(character_joins).add_observer(character_leaves);
}

fn character_joins(
    on: On<Insert<crate::slots::GameIdentity>>,
    identities: Query<&crate::slots::GameIdentity>,
    mut chars: ResMut<GameCharacters>,
) {
    if let Ok(identity) = identities.get(on.event_target()) {
        chars.add(&identity.0, on.event_target());
    }
}

/// Before an identity is replaced or removed (the entity despawned too).
fn character_leaves(
    on: On<Discard<crate::slots::GameIdentity>>,
    identities: Query<&crate::slots::GameIdentity>,
    mut chars: ResMut<GameCharacters>,
) {
    if let Ok(identity) = identities.get(on.event_target()) {
        chars.remove(&identity.0, on.event_target());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Runs(u32);

    #[derive(Component)]
    struct OfThisCrate;

    fn count(mut runs: ResMut<Runs>) {
        runs.0 += 1;
    }

    /// An app whose `count` is this crate's game's (a test's game is its
    /// crate: `chimera_core`).
    fn gated_app() -> App {
        let mut app = App::new();
        app.init_resource::<Runs>().gate_game("chimera_core", |app| {
            app.add_systems(Update, count);
        });
        app
    }

    #[test]
    fn a_gated_game_runs_only_once_in_play() {
        let mut app = gated_app();
        app.update();
        assert_eq!(app.world().resource::<Runs>().0, 0, "out of play: skipped");
        assert!(!app.world().resource::<GamesInPlay>().is_in_play("chimera_core"));
        app.add_observer(|on: On<GameEntersPlay>, mut runs: ResMut<Runs>| {
            assert_eq!(on.game, "chimera_core");
            runs.0 += 100;
        });
        enter(app.world_mut(), "chimera_core", "a test");
        enter(app.world_mut(), "chimera_core", "again");
        assert_eq!(app.world().resource::<Runs>().0, 100, "entering play triggers once");
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Runs>().0, 102, "in play: runs every update");
        assert!(app.world().resource::<GamesInPlay>().is_in_play("other"), "a game not gated plays");
    }

    #[test]
    fn an_entity_of_the_game_or_its_identity_puts_it_in_play() {
        let mut app = gated_app();
        app.update();
        app.world_mut().spawn(OfThisCrate);
        app.update();
        assert!(app.world().resource::<GamesInPlay>().is_in_play("chimera_core"), "its component on an entity");
        assert_eq!(app.world().resource::<Runs>().0, 1, "the same update (`First` before `Update`)");

        let mut app = gated_app();
        app.world_mut().spawn(crate::slots::GameIdentity("Chimera_Core".into()));
        assert!(app.world().resource::<GamesInPlay>().is_in_play("chimera_core"), "an identity, at once");
    }

    #[test]
    fn a_system_serving_every_game_is_not_gated() {
        fn always(mut runs: ResMut<Runs>) {
            runs.0 += 10;
        }
        let mut app = App::new();
        app.init_resource::<Runs>().gate_game("chimera_core", |app| {
            serves_every_game(app, always);
            app.add_systems(Update, (count, always));
        });
        app.update();
        assert_eq!(app.world().resource::<Runs>().0, 10, "only the one serving every game ran");
    }

    #[test]
    fn systems_added_in_finish_and_after_a_moment_are_gated() {
        struct Late;
        impl Plugin for Late {
            fn build(&self, _: &mut App) {}
            fn finish(&self, app: &mut App) {
                app.add_systems(Update, count);
            }
        }
        fn front(mut runs: ResMut<Runs>) {
            runs.0 += 10;
        }
        let mut app = App::new();
        app.init_resource::<Runs>().gate_game("chimera_core", |app| {
            app.add_plugins(Late);
        });
        let since = systems_now(&app);
        app.add_systems(Update, front);
        assert_eq!(gate_since(&mut app, &since, "chimera_core", &["chimera_core"]), 1, "the front end's system");
        app.finish();
        app.cleanup();
        app.update();
        assert_eq!(app.world().resource::<Runs>().0, 0, "out of play: neither ran");
        enter(app.world_mut(), "chimera_core", "a test");
        app.update();
        assert_eq!(app.world().resource::<Runs>().0, 11, "in play: both ran");
    }

    #[test]
    fn each_games_characters_follow_their_identities() {
        use crate::slots::GameIdentity;
        let mut app = App::new();
        track_game_characters(&mut app);
        let a = app.world_mut().spawn(GameIdentity("tf2".into())).id();
        let b = app.world_mut().spawn(GameIdentity("hl2dm".into())).id();
        let chars = |app: &App, g: &str| app.world().resource::<GameCharacters>().of(g).to_vec();
        assert_eq!(chars(&app, "tf2"), [a]);
        app.world_mut().entity_mut(b).insert(GameIdentity("TF2".into()));
        let mut both = [a, b];
        both.sort();
        assert_eq!(chars(&app, "tf2"), both, "kept sorted, any case");
        assert!(chars(&app, "hl2dm").is_empty());
        app.world_mut().entity_mut(a).remove::<GameIdentity>();
        app.world_mut().despawn(b);
        assert!(chars(&app, "tf2").is_empty());
    }

    #[test]
    fn crates_and_games_of_paths() {
        assert_eq!(crate_of("<hl2dm::game::Foo as Bar>::run"), "hl2dm");
        assert_eq!(crate_of("tf2::game::bodies::wear_bodies"), "tf2");
        assert_eq!(game_of("hl2dm:weapon_ar2"), Some("hl2dm"));
        assert_eq!(game_of("knife"), None);
    }
}
