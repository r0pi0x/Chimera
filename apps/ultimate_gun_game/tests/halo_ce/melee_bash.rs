//! Halo's melee in the party: no knife slot (its kit is empty: slot keys
//! hand out nothing, never another game's knife), and its melee button
//! bashes with the Halo gun in hand, which stays in hand, the player
//! Halo's. On the party's simulation (the sandbox: every game's guns),
//! headless.

use bevy::prelude::*;
use chimera_core::{
    core::{ActionRegistry, LocalPlayer},
    slots::GameIdentity,
};
use chimera_sim::{
    minigame::{self, ladder::Ladder},
    testkit,
    weapon::{WeaponEvent, WeaponEventKind},
};
use combat_arms::game::movement;
use ultimate_gun_game::sandbox::SANDBOX;

use crate::parts::ugg;

const RIFLE: &str = "halo_ce:assault_rifle";

#[test]
fn halos_melee_button_bashes_with_the_gun_and_slot_keys_add_nothing() {
    let mut sim = ugg();
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(1.5);
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let r = ladder.rungs.iter().position(|id| *id == RIFLE).expect("Halo's rifle in the sandbox");
    assert!(minigame::rung_command::move_to(sim.app.world_mut(), me, r, &ladder), "the rifle at once");
    sim.seconds(1.5);
    // The knife's key (3) and the others: still the rifle.
    for slot in 0..6u8 {
        sim.intent(me).select = Some(slot);
        sim.ticks(1);
        sim.intent(me).select = None;
        sim.seconds(0.3);
        assert_eq!(testkit::active_id(sim.app.world(), me), Some(RIFLE), "slot key {}", slot + 1);
    }
    let bit = sim.app.world().resource::<ActionRegistry>().bit("melee");
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut bashed = false;
    for k in 0..30 {
        sim.intent(me).actions = if k == 0 { bit } else { 0 };
        sim.ticks(1);
        bashed |= cursor
            .read(sim.app.world().resource::<Messages<WeaponEvent>>())
            .any(|e| e.owner == me && matches!(e.kind, WeaponEventKind::Action("melee")));
    }
    assert!(bashed, "the melee button bashes");
    let w = sim.app.world();
    assert_eq!(testkit::active_id(w, me), Some(RIFLE), "the rifle still in hand");
    assert_eq!(w.get::<GameIdentity>(me).map(|g| g.0.as_str()), Some("halo_ce"));
}
