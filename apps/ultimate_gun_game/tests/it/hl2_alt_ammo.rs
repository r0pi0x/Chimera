//! An HL2:DM rung's alt-fire rounds (the user's call, 2026-11-12;
//! docs/DESIGN.md "Gun game rules"): the SMG1's rung gives 2 contact
//! grenades, the AR2's 2 energy balls (their hooks' `loadout = 2`, given
//! to a weapon the rules hand out: `minigame::LoadoutWeapon`), with no
//! pickups to find more; the third attack2 clicks empty. Given again with
//! the rung (a respawn on it, a rung swap), the count is 2 again.

use bevy::prelude::*;
use chimera_sim::{
    minigame::{
        self,
        ladder::{self, Ladder},
        runtime,
    },
    weapon::{Inventory, Weapon, projectile::LaunchRequest},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim, map::PlaySound};
use combat_arms::{game::movement, harness::CaSim};
use hl2dm::game::weapons::{AR2, SMG1, launch::Hl2Rounds};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin};

/// The automatic gun game with the SMG1 and the AR2 as its first rungs,
/// and a player in it.
fn playing() -> (Sim, Entity) {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin, UltimateGunGamePlugin { seed: 3, rungs: 13, ..Default::default() }));
    minigame::start(sim.app.world_mut(), MINIGAME).expect("the automatic gun game starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.seconds(1.5);
    let mut l = sim.app.world().resource::<Ladder>().clone();
    l.rungs[0] = SMG1;
    l.rungs[1] = AR2;
    // Their own rungs draw nothing of another game's.
    for x in l.extras.iter_mut().take(2) {
        *x = None;
    }
    sim.app.world_mut().insert_resource(l);
    (sim, me)
}

/// `me` on `rung`, its weapon drawn.
fn on_rung(sim: &mut Sim, me: Entity, rung: usize) -> Entity {
    let l = sim.app.world().resource::<Ladder>().clone();
    ladder::set_rung(sim.app.world_mut(), me, rung, &l);
    sim.seconds(1.5);
    held(sim, me, l.rungs[rung])
}

fn held(sim: &Sim, me: Entity, id: &str) -> Entity {
    let w = sim.app.world();
    let gun = w.get::<Inventory>(me).unwrap().weapons.iter().copied().find(|g| w.get::<Weapon>(*g).is_some_and(|x| x.id == id));
    let gun = gun.unwrap_or_else(|| panic!("holds {id}"));
    assert_eq!(w.get::<Inventory>(me).unwrap().active, Some(gun), "{id} drawn");
    gun
}

fn rounds(sim: &Sim, gun: Entity) -> u32 {
    sim.app.world().get::<Hl2Rounds>(gun).expect("its own rounds").count
}

/// Three attack2 presses 2 s apart: the launches and the empty sounds
/// (`empty`) heard.
fn three_presses(sim: &mut Sim, me: Entity, empty: &str) -> (usize, usize) {
    let mut launches = sim.app.world().resource::<Messages<LaunchRequest>>().get_cursor_current();
    let mut sounds = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
    let (mut launched, mut clicks) = (0, 0);
    for _ in 0..3 {
        sim.intent(me).secondary = true;
        sim.ticks(1);
        sim.intent(me).secondary = false;
        for _ in 0..(2.0 * sim.tick_hz()) as u32 {
            sim.ticks(1);
            let w = sim.app.world();
            launched += launches.read(w.resource::<Messages<LaunchRequest>>()).count();
            clicks += sounds.read(w.resource::<Messages<PlaySound>>()).filter(|s| s.entry == empty).count();
        }
    }
    (launched, clicks)
}

fn two_then_empty(gun_id: &'static str, rung: usize, empty: &str) {
    let (mut sim, me) = playing();
    let gun = on_rung(&mut sim, me, rung);
    assert_eq!(rounds(&sim, gun), 2, "{gun_id}: 2 alt-fire rounds on its rung");
    let (launched, clicks) = three_presses(&mut sim, me, empty);
    assert_eq!(launched, 2, "{gun_id}: two launched");
    assert_eq!(rounds(&sim, gun), 0);
    assert!(clicks >= 1, "{gun_id}: the third press clicks empty");
    // A respawn on the same rung gives the rung again: 2 more.
    runtime::respawn(sim.app.world_mut(), me);
    sim.seconds(1.5);
    let gun = held(&sim, me, gun_id);
    assert_eq!(rounds(&sim, gun), 2, "{gun_id}: refilled with the rung");
}

#[test]
fn an_smg1_rung_fires_two_grenades_then_clicks_empty() {
    two_then_empty(SMG1, 0, "Weapon_SMG1.Empty");
}

#[test]
fn an_ar2_rung_fires_two_balls_then_clicks_empty() {
    two_then_empty(AR2, 1, "Weapon_IRifle.Empty");
}
