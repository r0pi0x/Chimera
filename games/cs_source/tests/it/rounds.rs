//! Counter-Strike rounds (`chimera_rounds 1`): freeze, elimination and
//! time wins, money rewards, survivors keeping their weapons.

use bevy::prelude::*;
use crate::stack::{
    bot::Bot,
    console::Console,
    core::{Damage, FreezeTime, Hitgroup, Intent, Team},
    harness::{CsSim, Sim},
    movement::placeholder,
    rules::rounds::{Phase, RoundSettings, RoundState},
    weapon::economy::{BuyWindow, Money},
};

fn kill(sim: &mut Sim, attacker: Entity, target: Entity) {
    sim.app.world_mut().write_message(Damage {
        force: bevy::math::Vec3::ZERO,
        target,
        attacker: Some(attacker),
        amount: 10.0,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Chest,
        kind: Default::default(),
        weapon: None,
    });
    sim.ticks(2);
}

fn money(sim: &Sim, e: Entity) -> u32 {
    sim.app.world().get::<Money>(e).unwrap().0
}

fn phase(sim: &Sim) -> Phase {
    sim.app.world().resource::<RoundState>().phase
}

fn setup() -> (Sim, Entity, Entity, Entity) {
    let mut sim = Sim::new(CsSim, ());
    let t = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), placeholder::ID);
    let ct = sim.spawn_character(Vec3::new(4.0, 1.0, 0.0), placeholder::ID);
    let ct2 = sim.spawn_character(Vec3::new(8.0, 1.0, 0.0), placeholder::ID);
    sim.app.world_mut().entity_mut(t).insert(Team(1));
    sim.app.world_mut().entity_mut(ct).insert(Team(2));
    sim.app.world_mut().entity_mut(ct2).insert(Team(2));
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_freezetime 1; mp_roundtime 0.5; chimera_rounds 1");
    sim.ticks(3);
    (sim, t, ct, ct2)
}

#[test]
fn rounds_freeze_then_play_and_eliminations_pay() {
    let (mut sim, t, ct, ct2) = setup();
    assert!(matches!(phase(&sim), Phase::Freeze { .. }), "{:?}", phase(&sim));
    assert_eq!(money(&sim, t), 800);
    // No free rifle in rounds: a pistol and the knife (here none: Sim has
    // no weapons registered), bought up from there.
    assert!(sim.app.world().resource::<BuyWindow>().0.is_ok());
    // Frozen: walking does nothing.
    let start = sim.position(t);
    sim.intent(t).move_axis = Vec2::Y;
    sim.ticks(10);
    let moved = (sim.position(t) - start).with_y(0.0).length();
    assert!(moved < 0.01, "moved {moved} while frozen");
    sim.seconds(1.2);
    assert!(matches!(phase(&sim), Phase::Live { .. }), "{:?}", phase(&sim));

    // The terrorist kills one CT (+300), then the other: terrorists win.
    kill(&mut sim, t, ct);
    assert_eq!(money(&sim, t), 1100);
    assert!(matches!(phase(&sim), Phase::Live { .. }));
    kill(&mut sim, t, ct2);
    let winner = match phase(&sim) {
        Phase::Over { winner, .. } => winner,
        p => panic!("round not over: {p:?}"),
    };
    assert_eq!(winner, Some(Team(1)));
    // The bonuses wait for the next round's start, as CS:S's (measured:
    // the accounts change 5 s after the end, with the respawn).
    assert_eq!(money(&sim, t), 1400);
    assert_eq!(money(&sim, ct), 800);
    assert_eq!(sim.app.world().resource::<RoundState>().wins, [1, 0]);

    // The next round brings everyone back, frozen, and pays: 3000 for an
    // elimination on a map without bomb targets (3250 is a bomb map's;
    // measured, objectives.md Q5).
    sim.seconds(5.2);
    assert_eq!(money(&sim, t), 1400 + 3000);
    assert_eq!(money(&sim, ct), 800 + 1400, "first loss bonus");
    assert!(matches!(phase(&sim), Phase::Freeze { .. }), "{:?}", phase(&sim));
    assert_eq!(sim.app.world().resource::<RoundState>().number, 2);
    assert_eq!(
        sim.app.world().resource::<crate::stack::core::RoundRestarts>().0,
        2,
        "each round start restarts the map's entities"
    );
    assert!(sim.app.world().get::<crate::stack::rules::Dead>(ct).is_none());
}

/// CS:S's rounds reach observers as the mode runtime's steps
/// (`chimera_sim::mode::ModeStep`), each once and in order, inside the
/// round's step: a win's round over in the tick it is decided, and at the next
/// round's start its end and the new round's setup in one tick, the map's
/// round restart (`core::RoundRestarts`, `core::RoundRestart`) counted by
/// the new round's `MissionStart`.
#[test]
fn rounds_steps_are_triggered_in_order_inside_the_step() {
    use chimera_sim::mode::ModeStep;
    use crate::stack::core::{RoundRestart, RoundRestarts, SimClock};
    #[derive(Resource, Default)]
    struct Steps(Vec<(u64, String, u32)>);
    let (mut sim, t, ct, ct2) = setup();
    sim.app
        .init_resource::<Steps>()
        .add_observer(|s: On<ModeStep>, c: Res<SimClock>, r: Option<Res<RoundRestarts>>, mut steps: ResMut<Steps>| {
            steps.0.push((c.tick, format!("{:?}", s.0), r.map_or(0, |r| r.0)));
        })
        .add_observer(|r: On<RoundRestart>, c: Res<SimClock>, mut steps: ResMut<Steps>| {
            steps.0.push((c.tick, "RoundRestart".into(), r.0));
        });
    sim.seconds(1.2);
    assert!(matches!(phase(&sim), Phase::Live { .. }), "{:?}", phase(&sim));
    kill(&mut sim, t, ct);
    sim.app.world_mut().write_message(Damage {
        force: Vec3::ZERO,
        target: ct2,
        attacker: Some(t),
        amount: 10.0,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Chest,
        kind: Default::default(),
        weapon: None,
    });
    while !matches!(phase(&sim), Phase::Over { .. }) {
        sim.ticks(1);
    }
    let won_at = sim.app.world().resource::<SimClock>().tick;
    let over: Vec<(u64, String)> =
        std::mem::take(&mut sim.app.world_mut().resource_mut::<Steps>().0).into_iter().map(|(t, e, _)| (t, e)).collect();
    assert_eq!(over, [(won_at, "RoundOver".to_string())], "the win's round over in its tick");
    let restarts = sim.app.world().resource::<RoundRestarts>().0;
    while sim.app.world().resource::<RoundRestarts>().0 == restarts {
        sim.ticks(1);
    }
    let at = sim.app.world().resource::<SimClock>().tick;
    let steps = std::mem::take(&mut sim.app.world_mut().resource_mut::<Steps>().0);
    let want: Vec<(u64, String, u32)> = [
        ("RoundEnd", restarts),
        ("RoundEnded", restarts),
        ("Prepare { first: false }", restarts),
        ("RoundRestart", restarts + 1),
        ("MissionStart", restarts + 1),
    ]
    .into_iter()
    .map(|(e, r)| (at, e.to_string(), r))
    .collect();
    assert_eq!(steps, want, "the round's end and the next round's start in one tick");
}

/// Objectives.md Q5 (measured 2026-10-17: de_dust2 with its bomb targets
/// removed before a restart, `mp_roundtime 1`): on a map without objectives
/// the clock ends nothing, the round goes on until a side is dead.
/// `chimera_untimed_win 1` (ours): the defenders win on time.
#[test]
fn time_ends_nothing_without_objectives_and_buying_closes() {
    let (mut sim, t, ct, _) = setup();
    let s = sim.app.world().resource::<RoundSettings>().clone();
    sim.seconds(1.2);
    // Buy period: 90 s by default, longer than this 30 s round; shorten it.
    sim.app.world_mut().resource_mut::<Console>().submit("mp_buytime 0.1");
    sim.seconds(6.5);
    assert!(sim.app.world().resource::<BuyWindow>().0.is_err(), "buy period over");
    sim.seconds(40.0);
    assert!(matches!(phase(&sim), Phase::Live { .. }), "the clock ran out: {:?}", phase(&sim));
    sim.app.world_mut().resource_mut::<Console>().submit("chimera_untimed_win 1");
    sim.ticks(2);
    match phase(&sim) {
        Phase::Over { winner, .. } => assert_eq!(winner, Some(Team(2))),
        p => panic!("{p:?}"),
    }
    // Paid as the next round starts.
    sim.seconds(5.2);
    assert_eq!(money(&sim, ct), s.start_money + s.win_bonus);
    assert_eq!(money(&sim, t), s.start_money + s.loss_bonus);
    let _ = sim.app.world().get::<Intent>(t);
}

/// Players on one side only (a lone player, or a minigame map with one
/// team's spawns): all of them dead is a draw at once, not a wait for
/// the round clock; one still alive keeps the round going.
#[test]
fn one_side_all_dead_is_a_draw() {
    let mut sim = Sim::new(CsSim, ());
    let a = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), placeholder::ID);
    let b = sim.spawn_character(Vec3::new(4.0, 1.0, 0.0), placeholder::ID);
    for e in [a, b] {
        sim.app.world_mut().entity_mut(e).insert(Team(1));
    }
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_freezetime 0; mp_roundtime 5; chimera_rounds 1");
    sim.ticks(3);
    // The world kills them (falls, map hurts): no friendly fire.
    let fall = |sim: &mut Sim, target: Entity| {
        sim.app.world_mut().write_message(Damage {
            force: Vec3::ZERO,
            target,
            attacker: None,
            amount: 10.0,
            point: Vec3::ZERO,
            dir: Vec3::X,
            hitgroup: Hitgroup::Generic,
            kind: Default::default(),
            weapon: None,
        });
        sim.ticks(2);
    };
    fall(&mut sim, a);
    assert!(matches!(phase(&sim), Phase::Live { .. }), "one still alive: {:?}", phase(&sim));
    fall(&mut sim, b);
    match phase(&sim) {
        Phase::Over { winner, .. } => assert_eq!(winner, None, "a draw"),
        p => panic!("round not over: {p:?}"),
    }
    sim.seconds(5.2);
    assert_eq!(sim.app.world().resource::<RoundState>().number, 2, "the next round");
    // A draw pays nobody (measured, objectives.md Q5: a lone CT's draws).
    for e in [a, b] {
        assert_eq!(money(&sim, e), 800);
    }
}

/// Freeze time holds bots as it holds the player (CS:S freezes
/// everyone): a bot that sees an enemy, and would strafe, stays put
/// until the freeze ends.
#[test]
fn bots_are_held_in_the_freeze_time_too() {
    // On the greybox's floor, so walking moves.
    let mut sim = Sim::new(CsSim, crate::stack::greybox::GreyboxMapPlugin);
    let places = [
        Vec3::new(0.0, 1.0, 25.0),
        Vec3::new(2.0, 1.0, 25.0),
        Vec3::new(2.0, 1.0, 15.0),
    ];
    let [t, bot, ct] = places.map(|at| sim.spawn_character(at, placeholder::ID));
    sim.app.world_mut().entity_mut(t).insert(Team(1));
    sim.app.world_mut().entity_mut(bot).insert((Team(1), Bot::default()));
    sim.app.world_mut().entity_mut(ct).insert(Team(2));
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_freezetime 2; mp_roundtime 0.5; chimera_rounds 1");
    sim.ticks(3);
    assert!(matches!(phase(&sim), Phase::Freeze { .. }), "{:?}", phase(&sim));
    assert!(sim.app.world().resource::<FreezeTime>().0);
    // The round put everyone at the spawns: back where the bot sees the
    // enemy.
    for (e, at) in [t, bot, ct].into_iter().zip(places) {
        sim.app.world_mut().get_mut::<Transform>(e).unwrap().translation = at;
    }
    sim.ticks(10);
    let flat = |sim: &Sim, e: Entity| sim.position(e).with_y(0.0);
    let start = [t, bot, ct].map(|e| flat(&sim, e));
    sim.intent(t).move_axis = Vec2::Y;
    sim.seconds(1.5);
    assert!(sim.app.world().get::<Bot>(bot).unwrap().target == Some(ct), "the bot sees the enemy");
    for (e, at) in [t, bot, ct].into_iter().zip(start) {
        let moved = flat(&sim, e).distance(at);
        assert!(moved < 0.01, "{e} moved {moved} while frozen");
    }
    // Once the round is live the bot strafes (the test would see it move).
    sim.seconds(1.0);
    assert!(matches!(phase(&sim), Phase::Live { .. }), "{:?}", phase(&sim));
    assert!(!sim.app.world().resource::<FreezeTime>().0);
    let moved = flat(&sim, bot).distance(start[1]);
    assert!(moved > 0.1, "the bot moved {moved} after the freeze");
}

/// The dead stop being solid (bots and players walk over the spot, a
/// ragdoll takes the body's place) until they respawn.
#[test]
fn the_dead_are_not_solid_until_the_next_round() {
    let (mut sim, t, ct, _) = setup();
    sim.seconds(1.2);
    kill(&mut sim, t, ct);
    sim.ticks(1);
    assert!(sim.app.world().get::<avian3d::prelude::ColliderDisabled>(ct).is_some());
    assert!(sim.app.world().get::<avian3d::prelude::ColliderDisabled>(t).is_none());
}

/// `mp_restartgame n` (the debug UI's "Restart game") starts the game
/// over n seconds later: round 1, no wins, the start money.
#[test]
fn restartgame_starts_over_after_its_delay() {
    let (mut sim, t, ct, ct2) = setup();
    sim.seconds(1.2);
    kill(&mut sim, t, ct);
    kill(&mut sim, t, ct2);
    assert_eq!(sim.app.world().resource::<RoundState>().wins, [1, 0]);
    assert_ne!(money(&sim, t), 800);
    sim.app.world_mut().resource_mut::<Console>().submit("mp_restartgame 1");
    sim.seconds(0.5);
    assert_eq!(sim.app.world().resource::<RoundState>().wins, [1, 0], "not yet");
    sim.seconds(0.7);
    let state = sim.app.world().resource::<RoundState>().clone();
    assert_eq!((state.number, state.wins, state.restart_at), (1, [0, 0], None));
    assert!(matches!(state.phase, Phase::Freeze { .. }), "{:?}", state.phase);
    assert_eq!(money(&sim, t), 800);
    assert!(sim.app.world().get::<crate::stack::rules::Dead>(ct).is_none(), "everyone is back");
}

/// Ammo in rounds (CS:S today, measured: specs/cs_source/weapons.md M19):
/// the spawn pistols and bought guns come full, and ammo is free, by the
/// box (`primammo`, `secammo` fill; `buyammo1` buys one) up to the type's
/// maximum.
#[test]
fn rounds_ammo_is_bought_by_the_box() {
    use crate::stack::{
        core::LocalPlayer,
        games::cs_source::{
            TICK_INTERVAL,
            weapons::{CsWeaponsPlugin, GLOCK, M4A1, USP},
        },
        greybox::GreyboxMapPlugin,
        weapon::{Inventory, Magazine, Weapon, economy::buy},
    };
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    let t = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), placeholder::ID);
    let ct = sim.spawn_character(Vec3::new(4.0, 1.0, 0.0), placeholder::ID);
    sim.app.world_mut().entity_mut(t).insert(Team(1));
    // The CT is the player; the terrorist shops as a bot.
    sim.app.world_mut().entity_mut(ct).insert((Team(2), LocalPlayer));
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_freezetime 5; chimera_rounds 1");
    sim.ticks(3);
    let gun = |sim: &Sim, e: Entity, id: &str| -> (u32, u32) {
        let w = sim.app.world();
        let inv = w.get::<Inventory>(e).unwrap();
        let g = inv
            .weapons
            .iter()
            .find(|g| w.get::<Weapon>(**g).is_some_and(|x| x.id == id))
            .unwrap_or_else(|| panic!("no {id}"));
        let m = w.get::<Magazine>(*g).unwrap();
        (m.clip, m.reserve)
    };
    // The bot: Glock 20 + 120, kevlar for 650 of its 800.
    assert_eq!(gun(&sim, t, GLOCK), (20, 120));
    assert_eq!(money(&sim, t), 150);
    // The player: USP 12 + 100.
    assert_eq!(gun(&sim, ct, USP), (12, 100));
    assert_eq!(money(&sim, ct), 800);
    // The M4A1 (3100 of 16000): full.
    sim.app.world_mut().entity_mut(ct).insert(Money(16000));
    buy(sim.app.world_mut(), ct, "m4a1").unwrap();
    assert_eq!(gun(&sim, ct, M4A1), (30, 90));
    assert_eq!(
        buy(sim.app.world_mut(), ct, "primammo").unwrap_err(),
        "You cannot carry any more."
    );
    // Emptied: one free box of 30, then a free fill to 90.
    let set_reserve = |sim: &mut Sim, id: &str, n: u32| {
        let w = sim.app.world_mut();
        let g = w
            .get::<Inventory>(ct)
            .unwrap()
            .weapons
            .iter()
            .copied()
            .find(|g| w.get::<Weapon>(*g).is_some_and(|x| x.id == id))
            .unwrap();
        w.get_mut::<Magazine>(g).unwrap().reserve = n;
    };
    set_reserve(&mut sim, M4A1, 0);
    buy(sim.app.world_mut(), ct, "buyammo1").unwrap();
    assert_eq!((gun(&sim, ct, M4A1).1, money(&sim, ct)), (30, 12900));
    buy(sim.app.world_mut(), ct, "primammo").unwrap();
    assert_eq!((gun(&sim, ct, M4A1).1, money(&sim, ct)), (90, 12900));
    // .45 ACP: boxes of 25, at most 100.
    set_reserve(&mut sim, USP, 0);
    buy(sim.app.world_mut(), ct, "buyammo2").unwrap();
    assert_eq!(gun(&sim, ct, USP).1, 25);
    buy(sim.app.world_mut(), ct, "secammo").unwrap();
    assert_eq!((gun(&sim, ct, USP).1, money(&sim, ct)), (100, 12900));
}

#[test]
fn a_map_change_starts_a_fresh_game_and_brings_the_dead_back() {
    let (mut sim, t, ct, ct2) = setup();
    sim.ticks(80);
    kill(&mut sim, t, ct);
    assert!(sim.app.world().get::<crate::stack::rules::Dead>(ct).is_some());
    // What loading a map does: a new game, not the old round's clock.
    crate::stack::rules::new_game(sim.app.world_mut());
    sim.ticks(3);
    assert!(sim.app.world().get::<crate::stack::rules::Dead>(ct).is_none(), "the dead respawn on the new map");
    assert!(matches!(phase(&sim), Phase::Freeze { .. }), "{:?}", phase(&sim));
    assert_eq!(sim.app.world().resource::<RoundState>().number, 1);
    assert_eq!(money(&sim, ct2), 800);
}

/// "The gun does not reload after the round ends" (playtest 2026-10-10):
/// survivors keep their weapons and the rounds in them (CS:S: nothing is
/// refilled at a round start), and pressing reload works in the next
/// round's freeze time as after it (CS:S only stops moving and attacking
/// while frozen).
#[test]
fn survivors_keep_their_clip_and_reload_in_the_freeze() {
    use crate::stack::{
        games::cs_source::{TICK_INTERVAL, weapons::CsWeaponsPlugin},
        greybox::GreyboxMapPlugin,
        weapon::{Inventory, Magazine},
    };
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    let t = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), placeholder::ID);
    let ct = sim.spawn_character(Vec3::new(4.0, 1.0, 0.0), placeholder::ID);
    // A person (not a bot: bots autobuy at the round's start and would
    // draw what they bought).
    sim.app
        .world_mut()
        .entity_mut(t)
        .insert((Team(1), crate::stack::core::LocalPlayer));
    sim.app.world_mut().entity_mut(ct).insert(Team(2));
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_freezetime 4; mp_roundtime 0.5; chimera_rounds 1");
    sim.seconds(4.2);
    assert!(matches!(phase(&sim), Phase::Live { .. }), "{:?}", phase(&sim));
    let pistol = sim.app.world().get::<Inventory>(t).unwrap().active.expect("a pistol drawn");
    let clip = |sim: &Sim| sim.app.world().get::<Magazine>(pistol).unwrap().clip;
    let full = clip(&sim);
    sim.app.world_mut().get_mut::<Magazine>(pistol).unwrap().clip = 5;
    kill(&mut sim, t, ct);
    assert!(matches!(phase(&sim), Phase::Over { .. }), "{:?}", phase(&sim));
    sim.seconds(5.2);
    assert!(matches!(phase(&sim), Phase::Freeze { .. }), "{:?}", phase(&sim));
    assert_eq!(sim.app.world().get::<Inventory>(t).unwrap().active, Some(pistol), "kept");
    assert_eq!(clip(&sim), 5, "a survivor's clip is as it was");
    // Reload pressed in the freeze: the pistol reloads (2.3 s at most).
    sim.intent(t).reload = true;
    sim.ticks(2);
    sim.intent(t).reload = false;
    sim.seconds(3.0);
    assert!(matches!(phase(&sim), Phase::Freeze { .. }), "{:?}", phase(&sim));
    assert_eq!(clip(&sim), full, "reloaded in the freeze");
    // And after it.
    sim.seconds(1.0);
    assert!(matches!(phase(&sim), Phase::Live { .. }), "{:?}", phase(&sim));
    sim.app.world_mut().get_mut::<Magazine>(pistol).unwrap().clip = 3;
    sim.intent(t).reload = true;
    sim.ticks(2);
    sim.intent(t).reload = false;
    sim.seconds(3.0);
    assert_eq!(clip(&sim), full, "reloaded in the round");
}

/// `autobuy` buys down CS:S's default list (one primary: the CT's first
/// is the M4A1), `rebuy` buys again what was carried after the last buy
/// (the player's lists are its `cl_autobuy` / `cl_rebuy` userinfo).
#[test]
fn autobuy_takes_the_list_and_rebuy_restores_the_last_buy() {
    use crate::stack::{
        core::LocalPlayer,
        games::cs_source::{
            TICK_INTERVAL,
            grenades::FLASHBANG,
            weapons::{CsWeaponsPlugin, M4A1},
        },
        greybox::GreyboxMapPlugin,
        weapon::{
            Armor, Inventory, Weapon,
            economy::{DefuseKit, buy},
            grenade::Throwable,
        },
    };
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    let ct = sim.spawn_character(Vec3::new(4.0, 1.0, 0.0), placeholder::ID);
    sim.app.world_mut().entity_mut(ct).insert((Team(2), LocalPlayer));
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_freezetime 5; chimera_rounds 1");
    sim.ticks(3);
    sim.app.world_mut().entity_mut(ct).insert(Money(16000));
    let carried = |sim: &Sim, id: &str| -> Option<Entity> {
        let w = sim.app.world();
        w.get::<Inventory>(ct)?
            .weapons
            .iter()
            .copied()
            .find(|g| w.get::<Weapon>(*g).is_some_and(|x| x.id == id))
    };
    // m4a1 (ak47 is the terrorists'), ammo, kit, kevlar and helmet.
    let said = buy(sim.app.world_mut(), ct, "autobuy").unwrap();
    assert!(said.contains("m4a1"), "{said}");
    assert!(carried(&sim, M4A1).is_some());
    let w = sim.app.world();
    assert_eq!(w.get::<Armor>(ct).map(|a| (a.amount, a.helmet)), Some((1.0, true)));
    assert!(w.get::<DefuseKit>(ct).is_some());
    // 3100 + 200 + 1000 (the M4A1 comes full; ammo is free).
    assert_eq!(money(&sim, ct), 16000 - 3100 - 200 - 1000);
    // Only one primary: famas and the rest are skipped, and a second
    // autobuy with the M4A1 carried buys no other gun.
    assert!(!said.contains("famas"), "{said}");
    buy(sim.app.world_mut(), ct, "flashbang").unwrap();
    buy(sim.app.world_mut(), ct, "flashbang").unwrap();
    // Lost it all (a death): rebuy brings back the rifle, its ammo, the
    // kit, the armour and both flashbangs.
    let m4 = carried(&sim, M4A1).unwrap();
    let flash = carried(&sim, FLASHBANG).unwrap();
    {
        let w = sim.app.world_mut();
        w.get_mut::<Inventory>(ct).unwrap().weapons.retain(|e| *e != m4 && *e != flash);
        w.despawn(m4);
        w.despawn(flash);
        w.entity_mut(ct).remove::<(Armor, DefuseKit)>();
        w.entity_mut(ct).insert(Money(16000));
    }
    sim.ticks(2);
    let said = buy(sim.app.world_mut(), ct, "rebuy").unwrap();
    assert!(carried(&sim, M4A1).is_some(), "{said}");
    let flash = carried(&sim, FLASHBANG).expect("flashbangs");
    let w = sim.app.world();
    assert_eq!(w.get::<Throwable>(flash).unwrap().count, 2, "{said}");
    assert!(w.get::<Armor>(ct).is_some_and(|a| a.helmet), "{said}");
    assert!(w.get::<DefuseKit>(ct).is_some(), "{said}");
    // Nothing missing: nothing to rebuy but ammo, which is full.
    assert!(buy(sim.app.world_mut(), ct, "rebuy").is_err());
}

/// `mp_maxrounds`: as the last round ends the game on the map is over
/// (CS:S starts its intermission in the same second as the round's end,
/// parity-gaps.md C9): everyone held still through the intermission
/// (`mp_chattime`), then the next map of the cycle (`map <name>` without
/// a network), or, with no cycle, a fresh game on this one.
#[test]
fn the_game_ends_after_mp_maxrounds_with_an_intermission() {
    use crate::stack::{
        core::FreezeTime,
        rules::game_end::MapCycle,
    };
    let (mut sim, t, ct, ct2) = setup();
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_maxrounds 1; mp_chattime 2");
    sim.seconds(1.2);
    kill(&mut sim, t, ct);
    kill(&mut sim, t, ct2);
    assert!(matches!(phase(&sim), Phase::Over { .. }));
    // The intermission at once, instead of the end pause and round 2.
    sim.ticks(2);
    let state = sim.app.world().resource::<RoundState>().clone();
    assert!(state.game_over.is_some(), "{state:?}");
    assert_eq!(state.number, 1);
    assert!(sim.app.world().resource::<FreezeTime>().0, "held still");
    assert!(sim.app.world().resource::<BuyWindow>().0.is_err());
    let start = sim.position(t);
    sim.intent(t).move_axis = Vec2::Y;
    sim.ticks(10);
    assert!((sim.position(t) - start).with_y(0.0).length() < 0.01, "frozen in the intermission");
    sim.intent(t).move_axis = Vec2::ZERO;
    // No map cycle: a fresh game here once it ends.
    sim.seconds(2.2);
    let state = sim.app.world().resource::<RoundState>().clone();
    assert!(state.game_over.is_none(), "{state:?}");
    assert_eq!((state.number, state.wins), (1, [0, 0]));
    // With a cycle, the next map is asked for (`map`, standing in here
    // for the client's).
    sim.app.world_mut().resource_mut::<Console>().add_command(crate::stack::console::Command {
        name: "map".into(),
        help: String::new(),
        run: std::sync::Arc::new(|_, a| Ok(Some(format!("loading {}", a.join(" "))))),
        complete: None,
    });
    sim.app.world_mut().insert_resource(MapCycle {
        maps: vec!["de_dust2".into(), "de_aztec".into()],
        current: "de_dust2".into(),
    });
    sim.seconds(1.2);
    kill(&mut sim, t, ct);
    kill(&mut sim, t, ct2);
    sim.seconds(7.5);
    let out: Vec<String> = sim.app.world().resource::<Console>().output.iter().map(|l| l.text.clone()).collect();
    assert!(out.iter().any(|l| l.contains("loading de_aztec")), "{:?}", &out[out.len().saturating_sub(5)..]);
    assert_eq!(sim.app.world().resource::<RoundState>().game_over, Some(f64::INFINITY), "held until it loads");
}

/// `mp_timelimit` in deathmatch: the intermission comes when the time is
/// up, not at a round's end; `timeleft` says what is left.
#[test]
fn the_time_limit_ends_a_deathmatch_game() {
    use crate::stack::core::FreezeTime;
    let mut sim = Sim::new(CsSim, ());
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), placeholder::ID);
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_timelimit 0.05; timeleft");
    sim.ticks(3);
    let said = |sim: &Sim| sim.app.world().resource::<Console>().output.iter().map(|l| l.text.clone()).collect::<Vec<_>>();
    assert!(said(&sim).iter().any(|l| l.starts_with("Time remaining on map: 0:0")), "{:?}", said(&sim));
    sim.seconds(2.0);
    assert!(sim.app.world().resource::<RoundState>().game_over.is_none());
    sim.seconds(1.5);
    assert!(sim.app.world().resource::<RoundState>().game_over.is_some(), "3 s are up");
    assert!(sim.app.world().resource::<FreezeTime>().0);
    let _ = p;
}

/// `mp_autoteambalance` (CS:S's default 1): a side two players larger
/// gives one to the other as the next round starts (computer players
/// first, the fewest kills, the highest entity: ours, capture C11);
/// never onto a side the map has no spawns for; 0 leaves teams alone.
#[test]
fn uneven_teams_are_balanced_as_the_next_round_starts() {
    use crate::stack::{core::SpawnPoint, rules::Score};
    let run = |balance: &str, ct_spawns: bool, humans: bool| {
        let mut sim = Sim::new(CsSim, ());
        sim.app.world_mut().spawn(SpawnPoint { team: Some(Team(1)) });
        if ct_spawns {
            sim.app.world_mut().spawn(SpawnPoint { team: Some(Team(2)) });
        }
        let ts: Vec<Entity> = (0..3)
            .map(|i| sim.spawn_character(Vec3::new(i as f32 * 4.0, 1.0, 0.0), placeholder::ID))
            .collect();
        let ct = sim.spawn_character(Vec3::new(0.0, 1.0, 8.0), placeholder::ID);
        for &e in &ts {
            sim.app.world_mut().entity_mut(e).insert(Team(1));
            if humans {
                sim.app.world_mut().entity_mut(e).insert(crate::stack::core::RemotePlayer);
            }
        }
        sim.app.world_mut().entity_mut(ct).insert(Team(2));
        sim.app
            .world_mut()
            .resource_mut::<Console>()
            .submit(&format!("mp_freezetime 0; mp_roundtime 5; chimera_rounds 1; mp_autoteambalance {balance}"));
        sim.ticks(3);
        // The first terrorist has kills: it stays.
        sim.app.world_mut().entity_mut(ts[0]).insert(Score { kills: 3, deaths: 0 });
        assert_eq!(sim.app.world().resource::<RoundState>().number, 1);
        kill(&mut sim, ts[0], ct);
        assert!(matches!(phase(&sim), Phase::Over { .. }), "{:?}", phase(&sim));
        sim.seconds(5.2);
        assert_eq!(sim.app.world().resource::<RoundState>().number, 2, "the next round");
        let teams: Vec<u8> = ts.iter().map(|e| sim.app.world().get::<Team>(*e).unwrap().0).collect();
        // Which of the two without kills is the higher entity.
        (teams, if ts[2] > ts[1] { 2 } else { 1 })
    };
    let (teams, moves) = run("1", true, true);
    let mut want = vec![1, 1, 1];
    want[moves] = 2;
    assert_eq!(teams, want, "one of the fewest kills moves; 2 against 2");
    let run = |balance: &str, spawns: bool, humans: bool| run(balance, spawns, humans).0;
    assert_eq!(run("0", true, true), vec![1, 1, 1], "off");
    assert_eq!(run("1", false, true), vec![1, 1, 1], "no counter-terrorist spawns");
    assert_eq!(run("1", true, false), vec![1, 1, 1], "bots never move (CS:S, parity-gaps.md C11)");
}

/// `mp_tkpunish 1`: a team killer dies as the next round starts (CS:S's
/// default 0 leaves it alone).
#[test]
fn tkpunish_kills_team_killers_as_the_next_round_starts() {
    use crate::stack::core::Health;
    for punish in [0, 1] {
        let (mut sim, t, ct, ct2) = setup();
        sim.app
            .world_mut()
            .resource_mut::<Console>()
            .submit(&format!("mp_friendlyfire 1; mp_tkpunish {punish}"));
        sim.seconds(1.2);
        assert!(matches!(phase(&sim), Phase::Live { .. }));
        kill(&mut sim, ct, ct2);
        kill(&mut sim, t, ct);
        assert!(matches!(phase(&sim), Phase::Over { .. }), "{:?}", phase(&sim));
        sim.seconds(5.2);
        assert_eq!(sim.app.world().resource::<RoundState>().number, 2);
        sim.ticks(3);
        let alive = |e| sim.app.world().get::<Health>(e).unwrap().current > 0.0;
        assert_eq!(alive(ct), punish == 0, "mp_tkpunish {punish}: the team killer");
        assert!(alive(ct2) && alive(t), "the others live");
    }
}

/// The movement `e` has now.
fn movement(sim: &Sim, e: Entity) -> &'static str {
    sim.app.world().get::<crate::stack::slots::MovementSlot>(e).unwrap().0
}

/// `setup`'s counter-terrorist `ct` as the local player, flying with the
/// `noclip` command (the game's and the server's, which the headless
/// `CsSim` lacks).
fn noclipped(sim: &mut Sim, ct: Entity) {
    use crate::stack::{core::LocalPlayer, movement::noclip};
    if !sim.app.world().resource::<Console>().command("noclip").is_some() {
        crate::stack::net::cheats::player_commands(&mut sim.app);
    }
    sim.app.world_mut().entity_mut(ct).insert(LocalPlayer);
    sim.app.world_mut().resource_mut::<Console>().submit("noclip");
    sim.ticks(1);
    assert_eq!(movement(sim, ct), noclip::ID);
}

/// CS:S puts a noclipping player back to walking at `mp_restartgame`
/// (measured on the probe, specs/cs_source/movement.md "Noclip and
/// spawning"); ours did keep the noclip. A character given noclip as its
/// movement outright (a test's, `--movement chimera:noclip`) keeps it.
#[test]
fn restartgame_ends_noclip() {
    use crate::stack::movement::noclip;
    let (mut sim, t, ct, _) = setup();
    sim.seconds(1.2);
    noclipped(&mut sim, ct);
    let flyer = sim.spawn_character(Vec3::new(12.0, 1.0, 0.0), noclip::ID);
    sim.app.world_mut().entity_mut(flyer).insert(Team(2));
    sim.app.world_mut().resource_mut::<Console>().submit("mp_restartgame 1");
    sim.seconds(0.5);
    assert_eq!(movement(&sim, ct), noclip::ID, "until the restart");
    sim.seconds(0.7);
    assert_eq!(sim.app.world().resource::<RoundState>().number, 1);
    assert_eq!(movement(&sim, ct), placeholder::ID, "walking again");
    assert_eq!(movement(&sim, t), placeholder::ID);
    assert_eq!(movement(&sim, flyer), noclip::ID, "noclip as its movement");
    // The toggle still works afterwards.
    sim.app.world_mut().resource_mut::<Console>().submit("noclip");
    sim.ticks(1);
    assert_eq!(movement(&sim, ct), noclip::ID);
}

/// A round won while a survivor flies: it keeps the noclip through the
/// round's end and walks at the next round's start (CS:S, measured).
#[test]
fn noclip_lasts_to_the_next_round() {
    use crate::stack::movement::noclip;
    let (mut sim, t, ct, _) = setup();
    sim.seconds(1.2);
    noclipped(&mut sim, ct);
    kill(&mut sim, ct, t);
    assert!(matches!(phase(&sim), Phase::Over { .. }), "{:?}", phase(&sim));
    sim.seconds(4.0);
    assert!(matches!(phase(&sim), Phase::Over { .. }));
    assert_eq!(movement(&sim, ct), noclip::ID, "still flying after the round's end");
    sim.seconds(1.2);
    assert_eq!(sim.app.world().resource::<RoundState>().number, 2);
    assert_eq!(movement(&sim, ct), placeholder::ID, "walking in the next round");
}

/// Dying while noclipping: the player comes back walking, at the next
/// round (rounds) or its respawn (deathmatch), as CS:S respawns it
/// (measured with the death and a respawn, CS_RespawnPlayer).
#[test]
fn the_dead_respawn_out_of_noclip() {
    for rounds in [true, false] {
        let (mut sim, t, ct, ct2) = setup();
        if !rounds {
            sim.app.world_mut().resource_mut::<Console>().submit("chimera_rounds 0; mp_respawn_delay 1");
            sim.ticks(3);
        }
        sim.seconds(1.2);
        noclipped(&mut sim, ct);
        // Others' damage doesn't reach a noclipping player: it dies by
        // its own hand (the `kill` command, as measured).
        kill(&mut sim, t, ct);
        assert!(sim.app.world().get::<crate::stack::rules::Dead>(ct).is_none(), "noclip takes no damage");
        kill(&mut sim, ct, ct);
        assert!(sim.app.world().get::<crate::stack::rules::Dead>(ct).is_some());
        if rounds {
            kill(&mut sim, t, ct2);
            sim.seconds(5.2);
            assert_eq!(sim.app.world().resource::<RoundState>().number, 2);
        } else {
            sim.seconds(1.2);
        }
        assert!(sim.app.world().get::<crate::stack::rules::Dead>(ct).is_none(), "rounds {rounds}: back");
        assert_eq!(movement(&sim, ct), placeholder::ID, "rounds {rounds}: walking");
    }
}

/// Game_entities.md question 8 (measured 2026-10-17 on the probe: two
/// game_scores of 5 with flag 2 on a CT bot made the log's "CT scored 10",
/// kept through the next round ends): a map's team points add to the
/// team's score, the rounds won, and stay.
#[test]
fn team_points_from_the_map_add_to_rounds_won_and_stay() {
    let (mut sim, t, ct, ct2) = setup();
    sim.seconds(1.2);
    sim.app.world_mut().write_message(crate::stack::core::ScoreChange {
        target: ct,
        points: 5,
        team: true,
        allow_negative: false,
    });
    sim.ticks(2);
    assert_eq!(sim.app.world().resource::<RoundState>().wins, [0, 5]);
    kill(&mut sim, ct, t);
    sim.seconds(5.5);
    assert_eq!(sim.app.world().resource::<RoundState>().wins, [0, 6], "kept, and the round's win on top");
    let _ = ct2;
}

/// A death by the world (a fall, a fire) or by one's own hand costs a kill
/// (CS:S, measured 2026-10-17: "committed suicide with world", frags −1).
#[test]
fn a_death_by_the_world_costs_a_kill() {
    use crate::stack::core::Score;
    let (mut sim, t, ct, _) = setup();
    sim.seconds(1.2);
    sim.app.world_mut().write_message(Damage {
        force: Vec3::ZERO,
        target: ct,
        attacker: None,
        amount: 10.0,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Generic,
        kind: Default::default(),
        weapon: None,
    });
    sim.ticks(2);
    let s = sim.app.world().get::<Score>(ct).cloned().unwrap();
    assert_eq!((s.kills, s.deaths), (-1, 1));
    // Killed by an enemy: no loss, the enemy's +1.
    kill(&mut sim, ct, t);
    let s = sim.app.world().get::<Score>(t).cloned().unwrap();
    assert_eq!((s.kills, s.deaths), (0, 1));
}
