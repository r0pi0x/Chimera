//! The console's scope and the games' namespaces
//! (docs/plans/active/console-namespaces.md): each frame before the
//! console runs its queue, the registered games' namespaces and aliases
//! (`GameInfo::console_namespaces`: `css`, `ca`) go to the console
//! (`Console::add_namespace`), and the scope short names resolve in when
//! none is pinned (`Console::auto_scope`) follows the game played: the
//! local player's `GameIdentity` (the party apps: the game of the gun in
//! hand), else the app's default game. Beside `console`, which knows no
//! game or slot.

use bevy::prelude::*;

use crate::{
    console::{Console, run_queue},
    core::LocalPlayer,
    game_registry::Games,
    slots::GameIdentity,
};

/// `follow_scope` before the console's queue.
pub struct ConsoleScopePlugin;

impl Plugin for ConsoleScopePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Console>()
            .add_systems(PreUpdate, follow_scope.before(run_queue));
    }
}

/// The games' namespaces (when a game was registered since), and the
/// automatic scope.
pub fn follow_scope(
    mut console: ResMut<Console>,
    games: Option<Res<Games>>,
    me: Query<&GameIdentity, With<LocalPlayer>>,
    mut known: Local<usize>,
) {
    if let Some(games) = &games
        && games.all().count() != *known
    {
        for g in games.all() {
            console.add_namespace(g.namespaces());
        }
        *known = games.all().count();
    }
    let game = me
        .iter()
        .next()
        .map(|i| i.0.clone())
        .or_else(|| games.and_then(|g| g.default_game()).map(|g| g.id.to_string()));
    let auto = game.map(|g| console.canonical_namespace(&g));
    if console.auto_scope != auto {
        console.auto_scope = auto;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::{Resolved, tests::namespaced};

    /// The scope follows the local player's identity (the gun in hand),
    /// else the default game; a pinned one stays.
    #[test]
    fn the_scope_follows_the_game_played() {
        let mut app = namespaced();
        app.add_plugins(ConsoleScopePlugin);
        let x = |app: &App| app.world().resource::<Console>().resolve("x");
        let found = |full: &str| Resolved::Found(full.into());
        app.update();
        assert_eq!(app.world().resource::<Console>().auto_scope, None, "no game, no player");
        let me = app.world_mut().spawn((LocalPlayer, GameIdentity("combat_arms".into()))).id();
        app.update();
        assert_eq!(x(&app), found("combat_arms:x"));
        app.world_mut().entity_mut(me).insert(GameIdentity("cs_source".into()));
        app.update();
        assert_eq!(x(&app), found("cs_source:x"));
        // Pinned: the identity doesn't move it.
        crate::console::tests::run(&mut app, "scope ca");
        assert_eq!(x(&app), found("combat_arms:x"));
        crate::console::tests::run(&mut app, "scope auto");
        assert_eq!(x(&app), found("cs_source:x"));
    }
}
