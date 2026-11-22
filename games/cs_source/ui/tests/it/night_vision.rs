//! Night vision goggles (CS:S's `buy nvgs`, `nightvision`): bought for
//! $1250 once, kept through rounds while alive, lost at death and bought
//! back by rebuy; the `nightvision` command turns them on and off with
//! the game's sounds, and only with goggles.

use bevy::{ecs::message::Messages, prelude::*};
use crate::stack::{
    client::night_vision::{NightVisionOn, NightVisionPlugin},
    console::Console,
    core::{Damage, Hitgroup, LocalPlayer, Team},
    games::cs_source::{
        TICK_INTERVAL,
        weapons::CsWeaponsPlugin,
    },
    greybox::GreyboxMapPlugin,
    harness::{CsSim, Sim},
    map::PlaySound,
    movement::placeholder,
    rules::rounds::RoundState,
    weapon::economy::{Money, NightVision, buy},
};

/// CS:S's night vision price and sound entries (its shop data,
/// games/cs_source/data/economy.toml).
const NIGHTVISION_PRICE: u32 = 1250;
const NIGHTVISION_ON: &str = "Player.NightVisionOn";
const NIGHTVISION_OFF: &str = "Player.NightVisionOff";

fn setup() -> (Sim, Entity, Entity) {
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, CsWeaponsPlugin, NightVisionPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    let ct = sim.spawn_character(Vec3::new(4.0, 1.0, 0.0), placeholder::ID);
    let t = sim.spawn_character(Vec3::new(-4.0, 1.0, 0.0), placeholder::ID);
    sim.app.world_mut().entity_mut(ct).insert((Team(2), LocalPlayer));
    sim.app.world_mut().entity_mut(t).insert(Team(1));
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_freezetime 1; mp_roundtime 0.5; chimera_rounds 1; chimera_untimed_win 1");
    sim.ticks(3);
    sim.app.world_mut().entity_mut(ct).insert(Money(16000));
    (sim, ct, t)
}

fn has(sim: &Sim, e: Entity) -> bool {
    sim.app.world().get::<NightVision>(e).is_some()
}

fn money(sim: &Sim, e: Entity) -> u32 {
    sim.app.world().get::<Money>(e).unwrap().0
}

#[test]
fn night_vision_is_bought_once_kept_through_rounds_and_lost_at_death() {
    let (mut sim, ct, t) = setup();
    assert!(!has(&sim, ct));
    buy(sim.app.world_mut(), ct, "nvgs").unwrap();
    assert!(has(&sim, ct));
    assert_eq!(money(&sim, ct), 16000 - NIGHTVISION_PRICE);
    // Once: CS:S's words, nothing paid.
    assert_eq!(
        buy(sim.app.world_mut(), ct, "nvgs").unwrap_err(),
        "You already have one!"
    );
    assert_eq!(money(&sim, ct), 16000 - NIGHTVISION_PRICE);
    // The round runs out (no one dies; `chimera_untimed_win 1`, as this
    // greybox has no objectives): the next one keeps them.
    let round = sim.app.world().resource::<RoundState>().number;
    sim.seconds(40.0);
    assert!(sim.app.world().resource::<RoundState>().number > round, "a new round");
    assert!(has(&sim, ct), "kept while alive");
    // Killed: gone.
    sim.app.world_mut().write_message(Damage {
        force: Vec3::ZERO,
        target: ct,
        attacker: Some(t),
        amount: 10.0,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Chest,
        kind: default(),
        weapon: None,
    });
    sim.ticks(2);
    assert!(!has(&sim, ct), "lost at death");
    // Rebuy (its NightVision category) buys them back.
    let round = sim.app.world().resource::<RoundState>().number;
    sim.seconds(8.0);
    assert!(sim.app.world().resource::<RoundState>().number > round, "a new round");
    sim.app.world_mut().entity_mut(ct).insert(Money(16000));
    let said = buy(sim.app.world_mut(), ct, "rebuy").unwrap();
    assert!(said.contains("nvgs"), "{said}");
    assert!(has(&sim, ct));
}

#[test]
fn nightvision_turns_the_goggles_on_and_off_with_their_sounds() {
    let (mut sim, ct, _) = setup();
    let mut cursor = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
    let mut heard = |sim: &Sim| -> Vec<String> {
        cursor
            .read(sim.app.world().resource::<Messages<PlaySound>>())
            .map(|s| s.entry.clone())
            .collect()
    };
    let on = |sim: &Sim| sim.app.world().resource::<NightVisionOn>().0;
    let say = |sim: &mut Sim, line: &str| {
        sim.app.world_mut().resource_mut::<Console>().submit(line);
        sim.ticks(1);
    };
    // Without goggles: nothing.
    say(&mut sim, "nightvision");
    assert!(!on(&sim));
    heard(&sim);
    buy(sim.app.world_mut(), ct, "nvgs").unwrap();
    sim.ticks(1);
    heard(&sim);
    say(&mut sim, "nightvision");
    assert!(on(&sim));
    assert!(heard(&sim).iter().any(|e| e == NIGHTVISION_ON));
    say(&mut sim, "nightvision");
    assert!(!on(&sim));
    assert!(heard(&sim).iter().any(|e| e == NIGHTVISION_OFF));
    // On, then the goggles are gone (death): off, silently.
    say(&mut sim, "nightvision");
    assert!(on(&sim));
    sim.app.world_mut().entity_mut(ct).remove::<NightVision>();
    sim.ticks(2);
    assert!(!on(&sim));
}
