//! A game's hooks are keyed by game (`gameplay::GameHooks`,
//! docs/plans/active/ecs-cleanup.md step 2): with two games' hooks
//! registered, in either order, the network and the engine call the
//! gameplay's game's (`Gameplay::rules`; `Gameplay::bots` for the bots'),
//! not the last plugin's, and a gameplay switch switches them. On the
//! engine's base, the greybox and the placeholder movement, with two
//! stand-in games: a join over the network (`JoinRules`), a bot kicked
//! (`BotHooks`), the server's freeze (`Holds`), a fresh game on a map
//! (`gameplay::NewGame`) and a player in the log (`game_log::LogSubjects`).

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{
    console,
    core::{self, Bot},
    game_log::{self, LogSubject, LogSubjects},
    gameplay::{self, Gameplay, GameplayAppExt, NewGame, switch_gameplay},
    slots::Loadout,
};
use chimera_net::{
    harness::NetSim,
    net::{BotHooks, Holds, JoinRules, memory::LinkConditions, parts::enter_at_spawn},
};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::greybox::GreyboxMapPlugin;

/// A stand-in game: its id, and its hooks name it.
trait Game: 'static {
    const ID: &'static str;
}
struct GameA;
impl Game for GameA {
    const ID: &'static str = "game_a";
}
struct GameB;
impl Game for GameB {
    const ID: &'static str = "game_b";
}

/// Which game's hook ran, in order.
#[derive(Resource, Default)]
struct Calls(Vec<String>);

/// The game whose join rules brought this character in.
#[derive(Component)]
struct EnteredBy(&'static str);

fn call(w: &mut World, what: String) {
    w.get_resource_or_init::<Calls>().0.push(what);
}

fn enter<G: Game>(w: &mut World, e: Entity) {
    enter_at_spawn(w, e);
    w.entity_mut(e).insert(EnteredBy(G::ID));
}

fn kick<G: Game>(w: &mut World, e: Entity) {
    call(w, format!("{} kick", G::ID));
    w.despawn(e);
}

fn sync_quota<G: Game>(w: &mut World) {
    call(w, format!("{} sync_quota", G::ID));
}

fn new_game<G: Game>(w: &mut World) {
    call(w, format!("{} new_game", G::ID));
}

fn subject<G: Game>(_: &World, _: Entity) -> Option<LogSubject> {
    Some(LogSubject {
        name: G::ID.into(),
        userid: 0,
        unique: String::new(),
    })
}

fn register<G: Game>(app: &mut App) {
    app.game_hook(
        G::ID,
        JoinRules {
            enter: enter::<G>,
            ..default()
        },
    )
    .game_hook(
        G::ID,
        BotHooks {
            kick: kick::<G>,
            sync_quota: sync_quota::<G>,
            ..default()
        },
    )
    .game_hook(
        G::ID,
        Holds {
            // Each game's freeze ends at a time of its own.
            freeze_ends: |_| Some(G::ID.len() as f64 + if G::ID == GameA::ID { 1.0 } else { 2.0 }),
            frozen: |i| i.clone(),
        },
    )
    .game_hook(G::ID, NewGame(new_game::<G>))
    .game_hook(G::ID, LogSubjects(subject::<G>));
}

/// Both games' hooks, A's plugin first or B's.
fn setup(a_first: bool) -> impl Fn(&mut App) + Send + Sync + 'static {
    move |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin).insert_resource(Loadout {
            movement: placeholder::ID,
        });
        if a_first {
            register::<GameA>(app);
            register::<GameB>(app);
        } else {
            register::<GameB>(app);
            register::<GameA>(app);
        }
    }
}

/// A gameplay of these games: `rules`' rules and `bots`' bots.
fn gameplay(rules: &'static str, bots: &'static str) -> Gameplay {
    Gameplay {
        rules,
        bots,
        hud: rules,
        binds: rules,
        tick: Duration::from_secs_f64(1.0 / core::DEFAULT_TICK_HZ),
    }
}

fn calls(world: &mut World) -> Vec<String> {
    std::mem::take(&mut world.get_resource_or_init::<Calls>().0)
}

/// The game whose `enter` brought client `i`'s character in on the server.
fn entered_by(sim: &mut NetSim, i: usize) -> Option<&'static str> {
    let e = sim.character_of(i)?;
    sim.server.app.world().get::<EnteredBy>(e).map(|b| b.0)
}

/// A joining client comes in by the gameplay's rules' `JoinRules`, in
/// either registration order; after a switch, the next one by the other
/// game's.
#[test]
fn a_join_follows_the_gameplay_in_either_plugin_order() {
    for a_first in [true, false] {
        let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(a_first));
        switch_gameplay(sim.server.app.world_mut(), gameplay(GameA::ID, GameA::ID));
        sim.add_client(|_| {});
        sim.until_joined(600);
        assert!(sim.until(200, |s| entered_by(s, 0).is_some()), "client 0 came in");
        assert_eq!(
            entered_by(&mut sim, 0),
            Some(GameA::ID),
            "A's rules (A first: {a_first})"
        );

        switch_gameplay(sim.server.app.world_mut(), gameplay(GameB::ID, GameB::ID));
        sim.add_client(|_| {});
        sim.until_joined(600);
        assert!(sim.until(200, |s| entered_by(s, 1).is_some()), "client 1 came in");
        assert_eq!(
            entered_by(&mut sim, 1),
            Some(GameB::ID),
            "B's rules after the switch (A first: {a_first})"
        );
    }
}

/// The bots' hooks are the gameplay's bots' game's (here not its rules'),
/// the holds, the fresh game and the log's names its rules' game's; in
/// either registration order, and a switch switches them.
#[test]
fn each_hook_is_the_gameplays_games_in_either_plugin_order() {
    for a_first in [true, false] {
        let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(a_first));
        sim.ticks(2);
        for (rules, bots, freeze_ends) in [(GameA::ID, GameB::ID, 7.0), (GameB::ID, GameA::ID, 8.0)] {
            let world = sim.server.app.world_mut();
            switch_gameplay(world, gameplay(rules, bots));
            calls(world);

            // `kick <name>` (a bot): the bots' game's.
            let bot = world.spawn((Bot, Name::new("Zed"))).id();
            console::execute(world, &["kick".into(), "Zed".into()], 0);
            assert!(world.get_entity(bot).is_err(), "kicked");
            assert_eq!(calls(world), [format!("{bots} kick")], "A first: {a_first}");

            // The server's freeze (`capture_own_states`), a client's hold.
            assert_eq!(
                (Holds::active(world).freeze_ends)(world),
                Some(freeze_ends),
                "A first: {a_first}"
            );

            // A map loaded: the rules' fresh game.
            gameplay::new_game(world);
            assert_eq!(calls(world), [format!("{rules} new_game")], "A first: {a_first}");

            // A player in the server log.
            let p = world.spawn(Name::new("someone")).id();
            assert!(
                game_log::subject_text(world, p).starts_with(&format!("\"{rules}<")),
                "A first: {a_first}"
            );
        }
    }
}

/// A gameplay whose game registered none (a minigame's rules) gets each
/// kind's fallback: the network's log names (`any_game_hook`), the
/// defaults of the rest (no fresh game).
#[test]
fn a_game_without_hooks_gets_the_defaults() {
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(true));
    sim.ticks(2);
    let world = sim.server.app.world_mut();
    switch_gameplay(world, gameplay("game_c", "game_c"));
    calls(world);
    gameplay::new_game(world);
    assert!(calls(world).is_empty(), "no fresh game");
    assert_eq!((Holds::active(world).freeze_ends)(world), None);
    let p = world.spawn(Name::new("someone")).id();
    assert!(
        game_log::subject_text(world, p).starts_with("\"someone<"),
        "the network's names"
    );
}
