//! The party's keyed hooks (`gameplay::GameHooks`,
//! docs/plans/active/ecs-cleanup.md step 2). Before step 2 the party ran
//! Combat Arms' single-slot hooks (its plugin is the only game's that
//! inserted them) even while a minigame is the rules; now a minigame's
//! rules (`minigame::MINIGAME`) get the minigame's: a fresh game on a map
//! only counts the load (`core::MapLoads`; Combat Arms' restarted its
//! match, the minigame's `mode::Match` too), a predicted freeze keeps the
//! look only, as the match's end holds the server's players
//! (`runtime::end_frozen`; Combat Arms' kept firing), a joiner comes in at
//! a free spawn point (as Combat Arms' `ca_mode none` did). The bots'
//! hooks stay Combat Arms' (the minigame's bots slot).

use bevy::prelude::*;
use chimera_core::{
    core::{Bot, Intent, MapLoads},
    gameplay::{self, Gameplay, switch_gameplay},
};
use chimera_net::net::{BotHooks, Holds, JoinRules};
use chimera_sim::{
    minigame::{self, runtime::end_frozen},
    mode::{Match, Phase},
};
use combat_arms::game::bots::BotSettings;

use crate::net_party::party;

fn held(world: &World) -> Intent {
    let i = Intent {
        fire: true,
        move_axis: Vec2::Y,
        yaw: 0.5,
        ..default()
    };
    (Holds::active(world).frozen)(&i)
}

/// While the minigame plays: the minigame's hooks, Combat Arms' bots'.
/// Back on Combat Arms' rules: Combat Arms' again.
#[test]
fn a_minigame_has_its_own_hooks_and_combat_arms_bots() {
    let mut sim = party(0, None);
    sim.ticks(2);
    let w = sim.server.app.world_mut();
    let gameplay = w.resource::<Gameplay>().clone();
    assert_eq!(
        (gameplay.rules, gameplay.bots),
        (minigame::MINIGAME, combat_arms::game::GAME)
    );

    w.resource_mut::<Match>().phase = Phase::Setup { until: 9.0 };
    let loads = w.get_resource::<MapLoads>().map_or(0, |l| l.0);
    gameplay::new_game(w);
    assert_eq!(w.resource::<MapLoads>().0, loads + 1, "the load counted");
    assert_eq!(
        w.resource::<Match>().phase,
        Phase::Setup { until: 9.0 },
        "the match not started over"
    );
    assert_eq!((Holds::active(w).freeze_ends)(w), None);
    let (i, end) = (held(w), end_frozen(&held(w)));
    assert!(!i.fire && i.move_axis == Vec2::ZERO && i.yaw == 0.5, "the look only");
    assert_eq!(
        (i.fire, i.move_axis, i.yaw),
        (end.fire, end.move_axis, end.yaw),
        "as the match's end holds"
    );
    assert_eq!((JoinRules::active(w).choice)(w), "auto");

    w.resource_mut::<BotSettings>().quota = 5;
    let bot = w.spawn((Bot, Name::new("Zed"))).id();
    (BotHooks::active(w).kick)(w, bot);
    assert_eq!(w.resource::<BotSettings>().quota, 4, "Combat Arms' kick");

    switch_gameplay(w, Gameplay::of(&combat_arms::game::INFO));
    assert_eq!((Holds::active(w).freeze_ends)(w), Some(9.0), "Combat Arms' setup");
    assert!(held(w).fire, "Combat Arms' freeze keeps firing");
    gameplay::new_game(w);
    assert_eq!(w.resource::<Match>().phase, Phase::Waiting, "Combat Arms' fresh match");
}
