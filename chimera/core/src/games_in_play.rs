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
//!   stays ungated (work it does for other games too).
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
//! - **First entry** (`GameEntersPlay`, triggered): work a game wants done
//!   only once it is in play (its data documents) observes it.
//!
//! Known shortcuts: systems a game's plugins add in `Plugin::finish` or
//! `cleanup`, or outside `gate_game`, are not gated; games never leave play
//! (a long-lived server that has played every game runs them all).

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

/// Gating a game's systems on an app (module doc).
pub trait GateGameExt {
    /// Runs `add` (a game's plugins, systems), then gives the systems it
    /// added whose type path starts in crate `game` the game's switch as a
    /// run condition. The first call inserts `GamesInPlay` and its watch.
    fn gate_game(&mut self, game: &'static str, add: impl FnOnce(&mut App)) -> &mut Self;
}

impl GateGameExt for App {
    fn gate_game(&mut self, game: &'static str, add: impl FnOnce(&mut App)) -> &mut Self {
        if !self.world().contains_resource::<GamesInPlay>() {
            self.init_resource::<GamesInPlay>();
            self.add_systems(First, watch_entities);
            self.add_observer(identity_enters);
        }
        let before = keys(self.world());
        add(self);
        let flag = self.world_mut().resource_mut::<GamesInPlay>().flag(game);
        let every = self.world().resource::<GamesInPlay>().every.clone();
        let mut gated = 0;
        self.world_mut().resource_scope(|world, mut schedules: Mut<Schedules>| {
            for (label, schedule) in schedules.iter_mut() {
                let label = format!("{label:?}");
                if runs_once(&label) {
                    continue;
                }
                let old = before.iter().find(|(l, _)| *l == label).map(|(_, k)| k);
                let systems = &mut schedule.graph_mut().systems;
                let new: Vec<SystemKey> = systems
                    .iter()
                    .filter(|(k, s, _)| {
                        old.is_none_or(|o| !o.contains(k))
                            && crate_of(&s.name().as_string()) == game
                            && !every.contains(&s.system_type())
                    })
                    .map(|(k, _, _)| k)
                    .collect();
                for key in new {
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
        debug!("game {game}: {gated} systems gated");
        self
    }
}

/// Each schedule's system keys (to tell what `gate_game`'s `add` added).
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
    fn crates_and_games_of_paths() {
        assert_eq!(crate_of("<hl2dm::game::Foo as Bar>::run"), "hl2dm");
        assert_eq!(crate_of("tf2::game::bodies::wear_bodies"), "tf2");
        assert_eq!(game_of("hl2dm:weapon_ar2"), Some("hl2dm"));
        assert_eq!(game_of("knife"), None);
    }
}
