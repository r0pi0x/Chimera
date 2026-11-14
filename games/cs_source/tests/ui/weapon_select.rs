//! Switching weapons as CS:S players do besides the slot keys: `invnext`
//! / `invprev` (the mouse wheel's commands) step through every weapon,
//! slot by slot, and a pick names one weapon in a shared slot (the second
//! grenade); buying and picking up play the game's sounds;
//! `hud_fastswitch` is on by default and a saved config keeps it so; a
//! drawn weapon shows the row for a moment, except under a minigame.

use bevy::{
    ecs::message::{MessageCursor, Messages},
    prelude::*,
};
use crate::stack::{
    console::Console,
    core::{Intent, LocalPlayer, Team},
    games::cs_source::{
        ui::weapon_select::{WeaponMenu, WeaponSelectPlugin},
        TICK_INTERVAL,
        grenades::{FLASHBANG, HEGRENADE},
        weapons::{AK47, CsWeaponsPlugin, KNIFE},
    },
    greybox::{self, GreyboxMapPlugin},
    harness::{CsSim, Sim},
    map::GameSound,
    movement::placeholder,
    weapon::{Inventory, Weapon, economy::buy, give},
};

/// CS:S's sounds of a weapon and an item bought (its shop data,
/// games/cs_source/data/economy.toml).
const PICKUP_SOUND: &str = "Player.PickupWeapon";
const ITEM_SOUND: &str = "BaseCombatCharacter.ItemPickup2";

fn sim() -> (Sim, Entity) {
    let mut sim = Sim::with(CsSim, |app| {
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_plugins((GreyboxMapPlugin, CsWeaponsPlugin, WeaponSelectPlugin));
    });
    sim.set_tick_interval(TICK_INTERVAL);
    let p = sim.spawn_character(greybox::SPAWNS[0], placeholder::ID);
    sim.app.world_mut().entity_mut(p).insert((LocalPlayer, Team(2)));
    sim.ticks(2);
    (sim, p)
}

fn active_id(sim: &Sim, p: Entity) -> &'static str {
    let w = sim.app.world();
    let a = w.get::<Inventory>(p).unwrap().active.expect("no active weapon");
    w.get::<Weapon>(a).unwrap().id
}

/// Run a console command, then pass the weapon selection's pick to the
/// intent (as the local input does) until the switch is done.
fn command(sim: &mut Sim, p: Entity, line: &str) {
    sim.app.world_mut().resource_mut::<Console>().submit(line);
    for _ in 0..60 {
        sim.ticks(1);
        let pick = sim.app.world().resource::<WeaponMenu>().selected_slot();
        sim.intent(p).select = pick;
    }
}

#[test]
fn invnext_and_invprev_step_through_every_weapon_by_slot() {
    let (mut sim, p) = sim();
    give(sim.app.world_mut(), p, HEGRENADE).unwrap();
    give(sim.app.world_mut(), p, FLASHBANG).unwrap();
    // The rifle in hand (a grenade given to a fresh character may be
    // drawn first).
    sim.intent(p).select = Some(0);
    sim.ticks(60);
    sim.intent(p).select = None;
    sim.ticks(2);
    assert_eq!(active_id(&sim, p), AK47);
    // Fast switch (no game HUD here): each step switches at once.
    command(&mut sim, p, "invprev");
    assert_eq!(active_id(&sim, p), FLASHBANG, "back from the first slot wraps to the last weapon");
    command(&mut sim, p, "invprev");
    assert_eq!(active_id(&sim, p), HEGRENADE, "the other weapon in the same slot");
    command(&mut sim, p, "invnext");
    assert_eq!(active_id(&sim, p), FLASHBANG);
    command(&mut sim, p, "invnext");
    assert_eq!(active_id(&sim, p), AK47, "wraps to the first slot");
    command(&mut sim, p, "invnext; invnext");
    assert_eq!(active_id(&sim, p), KNIFE, "two notches at once: pistol, then knife");
}

#[test]
fn a_pick_draws_the_second_weapon_in_a_shared_slot() {
    let (mut sim, p) = sim();
    give(sim.app.world_mut(), p, HEGRENADE).unwrap();
    give(sim.app.world_mut(), p, FLASHBANG).unwrap();
    sim.ticks(2);
    let slot = {
        let w = sim.app.world();
        let inv = w.get::<Inventory>(p).unwrap();
        let flash = inv.weapons.iter().find(|e| w.get::<Weapon>(**e).unwrap().id == FLASHBANG).unwrap();
        w.get::<Weapon>(*flash).unwrap().slot
    };
    // The slot key draws the first grenade; a pick names the second.
    sim.intent(p).select = Some(slot);
    sim.ticks(60);
    assert_eq!(active_id(&sim, p), HEGRENADE);
    sim.intent(p).select = None;
    sim.ticks(2);
    sim.intent(p).select = Some(Intent::pick(slot, 1));
    sim.ticks(60);
    assert_eq!(active_id(&sim, p), FLASHBANG);
}

/// The rules' sounds (`GameSound`: buying, picking up) sent over `n`
/// ticks, each once (`rules` keeps its place between calls).
fn heard(sim: &mut Sim, rules: &mut MessageCursor<GameSound>, n: u32) -> Vec<String> {
    let mut out = Vec::new();
    for _ in 0..n {
        sim.ticks(1);
        let w = sim.app.world();
        out.extend(rules.read(w.resource::<Messages<GameSound>>()).map(|s| s.0.entry.clone()));
    }
    out
}

#[test]
fn buying_and_picking_up_play_the_pickup_sounds() {
    let (mut sim, p) = sim();
    let mut rules = MessageCursor::<GameSound>::default();
    heard(&mut sim, &mut rules, 2);
    // A grenade: nothing is dropped for it.
    buy(sim.app.world_mut(), p, "hegrenade").unwrap();
    let after_gun = heard(&mut sim, &mut rules, 2);
    assert!(after_gun.iter().any(|e| e == PICKUP_SOUND), "{after_gun:?}");
    buy(sim.app.world_mut(), p, "vesthelm").unwrap();
    let after_vest = heard(&mut sim, &mut rules, 2);
    assert!(after_vest.iter().any(|e| e == ITEM_SOUND), "{after_vest:?}");
    assert!(!after_vest.iter().any(|e| e == PICKUP_SOUND), "{after_vest:?}");
    // A gun walked over: dropped, then taken back after the touch delay.
    let gun = crate::stack::weapon::drop::drop_weapon(sim.app.world_mut(), p, true).expect("dropped");
    let at = sim.position(p);
    let mut picked = Vec::new();
    for _ in 0..150 {
        if let Some(mut t) = sim.app.world_mut().get_mut::<Transform>(gun) {
            t.translation = at;
        }
        picked.extend(heard(&mut sim, &mut rules, 1));
    }
    assert!(sim.app.world().get_entity(gun).is_err(), "picked up");
    assert_eq!(picked.iter().filter(|e| *e == PICKUP_SOUND).count(), 1, "{picked:?}");
}

/// `hud_fastswitch` is 1 by default (ours; CS:S's own is 0): slot keys
/// switch at once, the selection row doesn't take them. config.cfg saves
/// only cvars off their default, so a config written before (when 0 was
/// the default, never written) leaves it at 1; a player's own 0 is
/// written and read back.
#[test]
fn fast_switch_is_on_by_default_and_saved_configs_keep_it() {
    use crate::stack::{console::config_text, games::cs_source::ui::weapon_select::FastSwitch};
    let (mut sim, _) = sim();
    let fast = |sim: &Sim| sim.app.world().resource::<FastSwitch>().0;
    assert_eq!(fast(&sim), 1);
    let cvar = sim.app.world().resource::<Console>().cvar("hud_fastswitch").unwrap().clone();
    assert_eq!((cvar.default.as_str(), cvar.archive), ("1", true));
    // The default isn't written: an old config (no line) keeps 1.
    let text = config_text(sim.app.world_mut());
    assert!(!text.contains("hud_fastswitch"), "{text}");
    // A player's 0 is written, and read back.
    sim.app.world_mut().resource_mut::<Console>().submit("hud_fastswitch 0");
    sim.ticks(1);
    assert_eq!(fast(&sim), 0);
    let text = config_text(sim.app.world_mut());
    // Under its full name (`cs_source:hud_fastswitch`).
    let line = text
        .lines()
        .find(|l| l.split_whitespace().next().is_some_and(|n| n.ends_with("hud_fastswitch")))
        .expect("written");
    assert!(line.ends_with(" 0"), "{line}");
    sim.app.world_mut().resource_mut::<Console>().submit("hud_fastswitch 1");
    sim.ticks(1);
    sim.app.world_mut().resource_mut::<Console>().submit(line);
    sim.ticks(1);
    assert_eq!(fast(&sim), 0);
}

/// With fast switch, a weapon drawn shows the selection row on it for a
/// moment (CS:S's), but not while a minigame plays: the party's gun game
/// swaps guns on every kill and its race chip, top centre where the row
/// draws, shows the gun (the row overlapped it).
#[test]
fn a_drawn_weapon_shows_the_row_except_under_a_minigame() {
    let (mut sim, p) = sim();
    // CS:S's HUD look in (the row draws with it).
    sim.app.world_mut().insert_resource(crate::stack::map::hud::ActiveHud(
        std::sync::Arc::new(crate::stack::map::hud::GameHud::default()),
        Default::default(),
    ));
    sim.app.world_mut().init_resource::<chimera_client::client::fonts::UiFonts>();
    give(sim.app.world_mut(), p, AK47).unwrap();
    give(sim.app.world_mut(), p, KNIFE).unwrap();
    sim.ticks(2);
    let slot_of = |sim: &Sim, id: &str| {
        let w = sim.app.world();
        let inv = w.get::<Inventory>(p).unwrap();
        let e = inv.weapons.iter().find(|e| w.get::<Weapon>(**e).unwrap().id == id).unwrap();
        w.get::<Weapon>(*e).unwrap().slot
    };
    // Draws `id` by its slot key; whether the row showed on the way.
    let draw = |sim: &mut Sim, id: &str| -> bool {
        let slot = slot_of(sim, id);
        sim.intent(p).select = Some(slot);
        let mut shown = false;
        for _ in 0..60 {
            sim.ticks(1);
            shown |= sim.app.world().resource::<WeaponMenu>().shown_for.is_some();
        }
        sim.intent(p).select = None;
        sim.ticks(2);
        assert_eq!(active_id(sim, p), id);
        shown
    };
    let first = if active_id(&sim, p) == KNIFE { AK47 } else { KNIFE };
    let second = if first == KNIFE { AK47 } else { KNIFE };
    assert!(draw(&mut sim, first), "CS:S shows the row on a drawn weapon");
    // Let the row go before the next draw.
    sim.ticks(400);
    sim.app.world_mut().insert_resource(chimera_sim::minigame::hud::MinigameHud {
        title: "Gun game".into(),
        ..Default::default()
    });
    assert!(!draw(&mut sim, second), "no row while a minigame plays");
}
