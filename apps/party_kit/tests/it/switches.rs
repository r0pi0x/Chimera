//! What a switch of game does to a gun game player (playtest 2026-10-24):
//! its feet stay on the floor whatever the two games' hulls (CS:S's 62
//! and 45 unit box, Combat Arms' 100 and 80 unit cylinder: a crouched
//! Combat Arms player switching to CS:S stood with its feet 7.6 units
//! into the floor), crouched as it was, out of the walls; the knife is the
//! gun's game's (a knife death costs a rung with either); a CS:S player
//! in the party moves exactly as in CS:S; Combat Arms' bodies go as in
//! its free-for-all; and every rung's gun is in its game's class and
//! sounds when fired.

use bevy::prelude::*;
use chimera_core::{
    core::Dead,
    slots::{Footing, MovementRegistry, MovementSlot},
};
use chimera_sim::{
    minigame::ladder::{self, Ladder, Rung},
    weapon::{Inventory, Weapon},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::game::{inventory::KNIFE as CA_KNIFE, movement::CaMovement};
use cs_source::game::{
    movement::{self as cs_movement, SourceMovement},
    weapons::{self as cs_weapons, KNIFE as CS_KNIFE},
};

use crate::gun_game::{held, line_up, party, place, rung, shoot_until_dead, soldier};

/// `e`'s footing as its movement keeps it.
fn footing(sim: &Sim, e: Entity) -> Footing {
    let w = sim.app.world();
    let slot = w.get::<MovementSlot>(e).expect("a movement").0;
    let f = w.resource::<MovementRegistry>().get(slot).and_then(|m| m.footing).expect("a movement with feet");
    f(w, e).expect("its footing")
}

fn crouched(sim: &Sim, e: Entity) -> bool {
    let w = sim.app.world();
    w.get::<SourceMovement>(e).map(|m| m.ducked).or(w.get::<CaMovement>(e).map(|m| m.crouch_rate >= 1.0)).unwrap()
}

fn game_of(sim: &Sim, e: Entity) -> &'static str {
    let slot = sim.app.world().get::<MovementSlot>(e).unwrap().0;
    slot.split_once(':').unwrap().0
}

pub(crate) fn set_rung(sim: &mut Sim, e: Entity, r: usize) {
    let w = sim.app.world_mut();
    let l = w.resource::<Ladder>().clone();
    ladder::set_rung(w, e, r, &l);
}

/// The ids of the weapons `e` carries.
fn carried(sim: &Sim, e: Entity) -> Vec<&'static str> {
    let w = sim.app.world();
    w.get::<Inventory>(e).unwrap().weapons.iter().filter_map(|x| w.get::<Weapon>(*x)).map(|x| x.id).collect()
}

/// The greybox's floor is at 0: a player on it has its feet within the
/// ground test's reach above it (CS:S keeps up to 2 units, 5 cm), never
/// below.
fn assert_on_floor(sim: &Sim, e: Entity, what: &str) {
    let f = footing(sim, e);
    assert!(f.feet.y > -1e-3 && f.feet.y < 0.06, "{what}: feet at {} ({:?}, {})", f.feet.y, f, game_of(sim, e));
}

/// Standing and crouched, every switch between the games' movements (the
/// rungs' swaps) leaves the feet on the floor and the player crouched as
/// it was; it walks off at once.
#[test]
fn a_switch_keeps_the_feet_on_the_floor_standing_and_crouched() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    assert_eq!(game_of(&sim, a), "cs_source");
    assert_on_floor(&sim, a, "the Glock, standing");
    for crouch in [false, true] {
        sim.intent(a).crouch = crouch;
        sim.seconds(1.0);
        assert_eq!(crouched(&sim, a), crouch);
        for (r, game) in [(1, "combat_arms"), (2, "cs_source"), (5, "combat_arms"), (6, "cs_source")] {
            set_rung(&mut sim, a, r);
            sim.ticks(1);
            let what = format!("rung {r}, crouched {crouch}");
            assert_eq!(game_of(&sim, a), game, "{what}");
            assert_on_floor(&sim, a, &format!("{what}, the switch's tick"));
            assert_eq!(crouched(&sim, a), crouch, "{what}: crouched as it was");
            sim.seconds(0.3);
            assert_on_floor(&sim, a, &format!("{what}, settled"));
            assert_eq!(crouched(&sim, a), crouch, "{what}: still");
            // It walks off (stuck in the floor, it didn't).
            let from = sim.position(a);
            sim.intent(a).move_axis = Vec2::Y;
            sim.seconds(0.6);
            sim.intent(a).move_axis = Vec2::ZERO;
            let moved = (sim.position(a) - from).xz().length();
            assert!(moved > 0.3, "{what}: walked {moved} m");
            assert_on_floor(&sim, a, &format!("{what}, walked"));
            sim.intent(a).move_axis = -Vec2::Y;
            sim.seconds(0.6);
            sim.intent(a).move_axis = Vec2::ZERO;
        }
    }
    // Let go of crouch: it stands up as the game it is.
    sim.intent(a).crouch = false;
    sim.seconds(1.0);
    assert!(!crouched(&sim, a));
    assert_on_floor(&sim, a, "stood up");
}

/// Against a wall a switch to CS:S's wider box (16 units from the
/// centre, Combat Arms' 15 u x 1.8 cm = 10.6) puts the box out of the
/// wall, not into it; and a jump's switch keeps the player in the air.
#[test]
fn a_switch_by_a_wall_or_in_the_air_stays_out_of_solids() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    set_rung(&mut sim, a, 1);
    sim.seconds(1.0);
    assert_eq!(game_of(&sim, a), "combat_arms");
    // Against the east wall (its face at x = 40).
    let at = sim.position(a);
    place(&mut sim, a, Vec3::new(40.0 - 15.0 * 0.018 - 0.005, at.y, 0.0));
    sim.seconds(0.5);
    let x = sim.position(a).x;
    assert!(x > 39.6, "Combat Arms' hull at the wall: x {x}");
    set_rung(&mut sim, a, 2);
    sim.ticks(1);
    assert_eq!(game_of(&sim, a), "cs_source");
    let half = 16.0 * 0.0254;
    let x = sim.position(a).x;
    assert!(x <= 40.0 - half + 1e-3, "CS:S's box out of the wall: x {x}");
    assert_on_floor(&sim, a, "by the wall");
    // It walks along the wall.
    let from = sim.position(a);
    sim.intent(a).move_axis = Vec2::Y;
    sim.seconds(0.5);
    sim.intent(a).move_axis = Vec2::ZERO;
    assert!((sim.position(a) - from).length() > 0.3, "walks off the wall");
    // Mid-jump: the switch neither drops it to the floor nor lifts it.
    sim.seconds(0.5);
    sim.intent(a).jump = true;
    sim.ticks(10);
    sim.intent(a).jump = false;
    let up = footing(&sim, a).feet.y;
    assert!(up > 0.2, "in the air: {up}");
    set_rung(&mut sim, a, 3);
    sim.ticks(1);
    assert_eq!(game_of(&sim, a), "combat_arms");
    let feet = footing(&sim, a).feet.y;
    assert!((feet - up).abs() < 0.1, "still in the air: {up} -> {feet}");
    sim.seconds(1.5);
    assert_on_floor(&sim, a, "landed");
}

/// The knife is the gun's game's: CS:S's with a CS:S gun, Combat Arms'
/// with a Combat Arms gun (swapped with the rung), so drawing it keeps
/// the player its gun's game.
#[test]
fn the_knife_is_the_guns_games() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    assert_eq!(carried(&sim, a), vec![CS_KNIFE, cs_weapons::GLOCK]);
    for (r, gun, knife, game) in [
        (1, "combat_arms:beretta_m92fs", CA_KNIFE, "combat_arms"),
        (2, cs_weapons::DEAGLE, CS_KNIFE, "cs_source"),
        (3, "combat_arms:mp5a4", CA_KNIFE, "combat_arms"),
    ] {
        set_rung(&mut sim, a, r);
        sim.seconds(1.0);
        let mut have = carried(&sim, a);
        have.sort_unstable();
        let mut want = vec![knife, gun];
        want.sort_unstable();
        assert_eq!(have, want, "rung {r}: the gun and its game's knife only");
        assert_eq!(held(&sim, a), Some(gun), "rung {r}: the gun drawn");
        // The knife (class key 3) keeps the game.
        sim.intent(a).select = Some(2);
        sim.ticks(1);
        sim.intent(a).select = None;
        sim.seconds(0.5);
        assert_eq!(held(&sim, a), Some(knife), "rung {r}: the knife");
        assert_eq!(game_of(&sim, a), game, "rung {r}: the knife is {game}'s");
    }
    // A rung-up with the knife in hand: the new gun and its knife.
    set_rung(&mut sim, a, 4);
    sim.seconds(1.0);
    assert_eq!(held(&sim, a), Some(cs_weapons::P90));
    assert!(carried(&sim, a).contains(&CS_KNIFE) && !carried(&sim, a).contains(&CA_KNIFE));
    // A respawn on a Combat Arms rung: Combat Arms' knife.
    sim.app.world_mut().entity_mut(a).insert(Rung(5));
    chimera_sim::weapon::equip::strip(sim.app.world_mut(), a);
    chimera_sim::minigame::runtime::respawn(sim.app.world_mut(), a);
    sim.seconds(1.0);
    let mut have = carried(&sim, a);
    have.sort_unstable();
    assert_eq!(have, vec![CA_KNIFE, "combat_arms:remington_870"]);
}

/// A knife death costs a rung with either game's knife.
#[test]
fn a_knife_death_costs_a_rung_with_either_knife() {
    for (killer_rung, knife) in [(0, CS_KNIFE), (1, CA_KNIFE)] {
        let mut sim = party();
        let a = soldier(&mut sim);
        let b = soldier(&mut sim);
        sim.seconds(1.0);
        if killer_rung > 0 {
            set_rung(&mut sim, a, killer_rung);
        }
        sim.app.world_mut().entity_mut(b).insert(Rung(3));
        sim.seconds(1.0);
        sim.intent(a).select = Some(2);
        sim.ticks(1);
        sim.intent(a).select = None;
        line_up(&mut sim, a, b, 1.2);
        assert_eq!(held(&sim, a), Some(knife));
        assert!(shoot_until_dead(&mut sim, a, b, 8.0), "{knife} kills");
        assert_eq!(rung(&sim, b), 2, "{knife}: a rung down");
        assert_eq!(rung(&sim, a), killer_rung + 1, "{knife}: the killer climbs");
    }
}

/// A CS:S player in the party moves exactly as in CS:S: the same run,
/// jump, air strafe and crouch inputs from the same place give the same
/// path, tick for tick, on the party's simulation (Combat Arms' with
/// CS:S's movement for the CS:S identity) and on CS:S's own.
#[test]
fn a_cs_source_player_moves_as_in_cs_source() {
    // The party's: a gun game player on the Glock.
    let mut party = party();
    let a = soldier(&mut party);
    let _b = soldier(&mut party);
    party.seconds(1.5);
    assert_eq!(held(&party, a), Some(cs_weapons::GLOCK));
    assert_eq!(game_of(&party, a), "cs_source");
    // CS:S's own: its movement alone on the engine's base, at its default
    // max speed, 250 (the Glock's).
    let mut css = Sim::new(chimera_sim::SimBasePlugins, (GreyboxMapPlugin, cs_movement::SourceMovementPlugin));
    // Both at CS:S's own tick (the party's, Combat Arms', is the same 15
    // ms), one fixed tick a frame.
    css.set_tick_interval(cs_source::game::TICK_INTERVAL);
    party.set_tick_interval(combat_arms::game::TICK_INTERVAL);
    let p = css.spawn_character(Vec3::new(10.0, 1.0, 0.0), cs_movement::ID);
    css.seconds(1.5);
    let speed = party.app.world().get::<chimera_core::core::MaxSpeed>(a).and_then(|s| s.0).expect("the Glock's max speed");
    assert!((speed / 0.0254 - 250.0).abs() < 1e-3, "the Glock's 250: {}", speed / 0.0254);
    assert!(css.app.world().get::<chimera_core::core::MaxSpeed>(p).is_none_or(|s| s.0.is_none()), "CS:S's default, 250");
    assert_eq!(party.tick_hz(), css.tick_hz(), "the same tick");
    // Both at the same place, still, looking the same way.
    let start = Vec3::new(10.0, footing(&party, a).feet.y + 36.0 * 0.0254, 5.0);
    for (sim, e) in [(&mut party, a), (&mut css, p)] {
        place(sim, e, start);
        sim.app.world_mut().get_mut::<chimera_core::core::Velocity>(e).unwrap().0 = Vec3::ZERO;
        sim.intent(e).yaw = 0.0;
        sim.seconds(0.5);
    }
    let script = |t: u32, i: &mut chimera_core::core::Intent| {
        *i = chimera_core::core::Intent {
            yaw: if (60..110).contains(&t) { (t - 60) as f32 * 0.02 } else { i.yaw },
            move_axis: match t {
                0..40 => Vec2::Y,
                40..60 => Vec2::new(1.0, 1.0).normalize(),
                60..110 => Vec2::X,
                110..150 => -Vec2::Y,
                _ => Vec2::ZERO,
            },
            jump: (40..42).contains(&t) || (120..122).contains(&t),
            crouch: (140..170).contains(&t),
            pitch: i.pitch,
            ..default()
        };
    };
    let from = [party.position(a), css.position(p)];
    assert!((from[0] - from[1]).length() < 1e-4, "both settled at the same place: {from:?}");
    let mut worst = 0.0f32;
    for t in 0..200 {
        let mut path = Vec::new();
        for (k, (sim, e)) in [(&mut party, a), (&mut css, p)].into_iter().enumerate() {
            script(t, &mut sim.intent(e));
            sim.ticks(1);
            path.push(sim.position(e) - from[k]);
        }
        let d = path[0].distance(path[1]);
        worst = worst.max(d);
        assert!(d < 1e-3, "tick {t}: the party's {} vs CS:S's {} ({d} m apart)", path[0], path[1]);
    }
    println!("worst difference {worst} m");
    assert!(party.app.world().get::<Dead>(a).is_none());
}

/// Combat Arms' bodies in the gun game go as in its free-for-all (One Man
/// Army, specs/combat_arms/characters.md 10): 5 s after the owner's
/// respawn and the death animation's end, not at the next round.
#[test]
fn combat_arms_bodies_go_as_in_its_free_for_all() {
    use combat_arms::game::characters::CaBody;
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    // b Combat Arms' (the Beretta's), killed by a.
    set_rung(&mut sim, b, 1);
    sim.seconds(1.0);
    line_up(&mut sim, a, b, 4.0);
    assert!(shoot_until_dead(&mut sim, a, b, 8.0));
    sim.ticks(2);
    let bodies = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        w.query::<&CaBody>().iter(w).count()
    };
    assert_eq!(bodies(&mut sim), 1, "Combat Arms' body left");
    // The respawn (2 s), the death animation (at most 7.8 s), then 5 s.
    sim.seconds(4.0);
    assert!(sim.app.world().get::<Dead>(b).is_none(), "respawned");
    assert_eq!(bodies(&mut sim), 1, "kept a while after the respawn");
    sim.seconds(12.0);
    assert_eq!(bodies(&mut sim), 0, "gone");
}

/// Every rung's gun is in its game's class: Combat Arms' shotgun, SMG,
/// rifle and machine gun primaries (class key 1), the pistols
/// secondaries (2), the knives melee (3), CS:S's alike.
#[test]
fn every_rung_gun_is_in_its_class() {
    let sim = party();
    let catalog = sim.app.world().resource::<chimera_sim::weapon::data::WeaponCatalog>();
    let ladder = sim.app.world().resource::<Ladder>();
    for gun in &ladder.rungs {
        let slot = catalog.get(gun).unwrap_or_else(|| panic!("{gun} in the catalogue")).slot;
        let pistol = ["cs_source:weapon_glock", "cs_source:weapon_deagle", "combat_arms:beretta_m92fs"].contains(gun);
        assert_eq!(slot, if pistol { 1 } else { 0 }, "{gun}: {}", if pistol { "a secondary" } else { "a primary" });
    }
    for knife in [CS_KNIFE, CA_KNIFE] {
        assert_eq!(catalog.get(knife).unwrap().slot, 2, "{knife}: melee");
    }
}

/// Every rung's gun makes its fire sound when fired: CS:S's a `PlaySound`
/// of its data's entry (the map's sound bank has it: `heavy::` on the
/// installs' maps), Combat Arms' its own fire sound.
#[test]
fn every_rung_gun_sounds_when_fired() {
    use chimera_world::map::PlaySound;
    use combat_arms::game::effects::sounds::CaSound;
    let mut sim = party();
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    let rungs = sim.app.world().resource::<Ladder>().rungs.clone();
    for (r, gun) in rungs.iter().enumerate() {
        set_rung(&mut sim, a, r);
        sim.seconds(2.0);
        assert_eq!(held(&sim, a), Some(*gun));
        let want = sim
            .app
            .world()
            .resource::<chimera_sim::weapon::data::WeaponCatalog>()
            .get(gun)
            .and_then(|d| d.sounds.as_ref())
            .and_then(|s| s.fire.clone());
        sim.app.world_mut().resource_mut::<Messages<PlaySound>>().clear();
        sim.app.world_mut().resource_mut::<Messages<CaSound>>().clear();
        let mut heard = false;
        for t in 0..20 {
            sim.intent(a).fire = t % 2 == 0;
            sim.ticks(1);
            let w = sim.app.world_mut();
            heard |= match &want {
                Some(entry) => w.resource_mut::<Messages<PlaySound>>().drain().any(|m| m.entry == *entry),
                None => w.resource_mut::<Messages<CaSound>>().drain().count() > 0,
            };
        }
        sim.intent(a).fire = false;
        assert!(heard, "{gun} sounds when fired ({want:?})");
    }
}

/// Q (`lastinv`) in the gun game goes by weapon slot, most recently drawn
/// first: a rung's swap of the gun (and of the knife, to the gun's game's)
/// leaves the order alone, so Q is always gun <-> knife, whichever game's
/// each is and whichever slot the gun is in.
#[test]
fn q_goes_between_the_gun_and_the_knife_across_rungs() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    let q = |sim: &mut Sim| {
        sim.intent(a).last_weapon = true;
        sim.ticks(1);
        sim.intent(a).last_weapon = false;
        sim.seconds(0.5);
    };
    // Spawned with the Glock in hand: Q reaches the knife.
    assert_eq!(held(&sim, a), Some(cs_weapons::GLOCK));
    sim.intent(a).select = Some(2);
    sim.ticks(1);
    sim.intent(a).select = None;
    sim.seconds(0.5);
    assert_eq!(held(&sim, a), Some(CS_KNIFE));
    q(&mut sim);
    assert_eq!(held(&sim, a), Some(cs_weapons::GLOCK));
    let rungs = [
        (1, "combat_arms:beretta_m92fs", CA_KNIFE),
        (2, cs_weapons::DEAGLE, CS_KNIFE),
        (3, "combat_arms:mp5a4", CA_KNIFE),
        (4, cs_weapons::P90, CS_KNIFE),
        // A shotgun: a secondary in Combat Arms.
        (5, "combat_arms:remington_870", CA_KNIFE),
        (6, cs_weapons::AK47, CS_KNIFE),
    ];
    for (i, (r, gun, knife)) in rungs.into_iter().enumerate() {
        if i % 2 == 1 {
            // The knife in hand when the rung comes.
            q(&mut sim);
        }
        set_rung(&mut sim, a, r);
        sim.seconds(1.0);
        assert_eq!(held(&sim, a), Some(gun), "rung {r}: the gun");
        q(&mut sim);
        assert_eq!(held(&sim, a), Some(knife), "rung {r}: Q goes to the knife");
        q(&mut sim);
        assert_eq!(held(&sim, a), Some(gun), "rung {r}: Q goes back to the gun");
    }
}
