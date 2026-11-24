//! Hits that don't land (specs/cs_source/weapons.md "Refused hits",
//! measured on the probe 2026-10-17): what they still do.
//!
//! - A teammate's bullet, knife or HE with friendly fire off: nothing. No
//!   health, no armour, no hurt or flesh-impact sound, no slowdown, no
//!   flinch (and no blood: tests/it/impact_effects.rs).
//! - A bullet on a noclipping player: no health, armour, sound or
//!   slowdown, but the view flinches by the walking rule (a bare chest a
//!   tenth of the damage, a bare head −12/±9, a vest-covered chest none,
//!   though the vest takes nothing).
//! - `God` (unmeasured: `god` couldn't be given to the probe's bots):
//!   like the refused hits, no armour, sound, slowdown or flinch.
//! - Hits that land: armour and sounds as before; an HE blast now sounds
//!   kevlar on an armoured player, the flesh impact on a bare one.
//! - The thrower's own HE hurts him the same with friendly fire on or off
//!   (measured 2026-10-17, grenades.md "Self damage"); its ear-ringing
//!   (`Deafened`) reaches only players the blast hurts (inferred: the
//!   server's DSP message can't be seen from the probe, capture C31).

use std::sync::Arc;

use avian3d::prelude::ColliderAabb;
use bevy::prelude::*;
use crate::stack::{
    core::{Deafened, FriendlyFire, God, Health, Team},
    games::cs_source::{
        TICK_INTERVAL,
        grenades::HEGRENADE,
        movement::{self, QuakeMovement, SourceMovementPlugin, to_engine},
        weapons::{AK47, CsWeaponsPlugin, KNIFE},
    },
    greybox::GreyboxMapPlugin,
    harness::{CsSim, Sim},
    map::{MapBrush, MapBrushes, MapSounds, MapSurface, PlaySound, sound::SoundBank},
    movement::noclip,
    weapon::{Armor, Inventory, StartingWeapons, ViewPunch, Weapon, give, grenade::spawn_projectile},
};

#[derive(Resource, Default)]
struct Heard(Vec<String>);

/// Hurt sounds (`Player.*`) and flesh impacts.
fn hear(mut sounds: MessageReader<PlaySound>, mut heard: ResMut<Heard>) {
    for s in sounds.read() {
        if s.entry.starts_with("Player.") || s.entry.starts_with("Flesh.") {
            heard.0.push(s.entry.clone());
        }
    }
}

/// Whose hearing blasts hit.
#[derive(Resource, Default)]
struct Deaf(Vec<Entity>);

fn deaf(mut deafened: MessageReader<Deafened>, mut deaf: ResMut<Deaf>) {
    deaf.0.extend(deafened.read().map(|d| d.target));
}

fn flat_floor(mut commands: Commands) {
    let (a, b) = (
        to_engine(Vec3::new(-16384.0, -16384.0, -64.0)),
        to_engine(Vec3::new(16384.0, 16384.0, 0.0)),
    );
    commands.insert_resource(MapBrushes(vec![MapBrush::from_box(a.min(b), a.max(b))]));
}

/// A flat floor, CS:S movement and weapons at CS:S's tick, players'
/// hitboxes `flesh` (its bullet impact `Flesh.BulletImpact`).
fn sim() -> Sim {
    sim_on(false)
}

/// `sim` on the greybox map instead (grenades need its world).
fn sim_on(greybox: bool) -> Sim {
    let mut sim = Sim::with(CsSim, |app| {
        app.insert_resource(StartingWeapons {
            team: vec![],
            all: vec![KNIFE],
        })
        .add_plugins((SourceMovementPlugin, CsWeaponsPlugin));
        if greybox {
            app.add_plugins(GreyboxMapPlugin);
        } else {
            app.add_systems(Startup, flat_floor);
        }
    });
    sim.set_tick_interval(TICK_INTERVAL);
    let mut sounds = MapSounds::default();
    for (name, impact) in [("flesh", Some("Flesh.BulletImpact")), ("player", None)] {
        sounds.surfaces.insert(
            name.into(),
            MapSurface {
                bullet_impact: impact.map(String::from),
                ..default()
            },
        );
    }
    sim.app
        .insert_resource(SoundBank(Arc::new(sounds)))
        .init_resource::<Heard>()
        .init_resource::<Deaf>()
        .add_systems(Last, (hear, deaf));
    sim
}

/// Who is hit.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Victim {
    /// The other team.
    Enemy,
    /// The attacker's team, `mp_friendlyfire 0`.
    Teammate,
    /// The other team, noclipping.
    Noclipping,
    /// The other team, under `god`.
    Godlike,
}
use Victim::*;

/// What a hit did.
#[derive(Debug)]
struct Outcome {
    /// Health and armour lost (points).
    health: f32,
    armour: f32,
    heard: Vec<String>,
    /// The victim's velocity modifier (1 without Source movement).
    modifier: f32,
    /// Its view punch, Source degrees.
    punch: Vec3,
    /// Its hearing hit (`Deafened`).
    deafened: bool,
}

/// A victim standing at the origin with `armor`, 500 health (it lives
/// through anything), and an attacker `distance` units away on Source −X.
fn scene(sim: &mut Sim, victim: Victim, armor: Option<Armor>, distance: f32) -> (Entity, Entity) {
    scene_at(sim, to_engine(Vec3::new(0.0, 0.0, 36.5)), victim, armor, distance)
}

/// `scene` with the victim at `at` (engine space).
fn scene_at(sim: &mut Sim, at: Vec3, victim: Victim, armor: Option<Armor>, distance: f32) -> (Entity, Entity) {
    let target = sim.spawn_character(at, movement::ID);
    let attacker = sim.spawn_character(at + to_engine(Vec3::new(-distance, 0.0, 0.0)), movement::ID);
    let w = sim.app.world_mut();
    w.entity_mut(attacker).insert(Team(1));
    w.entity_mut(target)
        .insert((Team(if victim == Teammate { 1 } else { 2 }), Health {
            current: 5.0,
            max: 5.0,
        }));
    if let Some(a) = armor {
        w.entity_mut(target).insert(a);
    }
    if victim == Godlike {
        w.entity_mut(target).insert(God);
    }
    w.insert_resource(FriendlyFire(0));
    sim.ticks(2);
    if victim == Noclipping {
        sim.set_movement(target, noclip::ID);
        sim.ticks(1);
    }
    (target, attacker)
}

fn outcome(sim: &Sim, target: Entity, armor: Option<Armor>) -> Outcome {
    let w = sim.app.world();
    let v = w.get::<ViewPunch>(target).map_or(Vec3::ZERO, |v| v.0);
    Outcome {
        health: (5.0 - w.get::<Health>(target).unwrap().current) * 100.0,
        armour: armor.map_or(0.0, |a| (a.amount - w.get::<Armor>(target).unwrap().amount) * 100.0),
        heard: w.resource::<Heard>().0.clone(),
        modifier: w.get::<QuakeMovement>(target).map_or(1.0, |m| m.velocity_modifier),
        punch: Vec3::new(-v.x, v.y, v.z).map(f32::to_degrees),
        deafened: w.resource::<Deaf>().0.contains(&target),
    }
}

/// One `gun` hit at `frac` of the victim's height (0 feet, 1 top) from
/// `distance` units (a knife: its slash).
fn hit(gun: &str, frac: f32, distance: f32, victim: Victim, armor: Option<Armor>) -> Outcome {
    let mut sim = sim();
    let (target, shooter) = scene(&mut sim, victim, armor, distance);
    give(sim.app.world_mut(), shooter, gun).unwrap();
    sim.seconds(1.5);
    let b = *sim.app.world().get::<ColliderAabb>(target).unwrap();
    let mut point = sim.position(target);
    point.y = b.min.y + (b.max.y - b.min.y) * frac;
    let d = point - (sim.position(shooter) + sim.state(shooter).eye_offset);
    {
        let mut i = sim.intent(shooter);
        i.yaw = (-d.x).atan2(-d.z);
        i.pitch = d.y.atan2(d.xz().length());
    }
    sim.app.world_mut().resource_mut::<Heard>().0.clear();
    sim.intent(shooter).fire = true;
    sim.ticks(1);
    sim.intent(shooter).fire = false;
    let o = outcome(&sim, target, armor);
    // The hit's sounds are out by now; later ticks change nothing.
    sim.seconds(0.3);
    let later = outcome(&sim, target, armor);
    assert_eq!((later.health, later.armour), (o.health, o.armour));
    Outcome { heard: later.heard, ..o }
}

/// An HE going off on the floor 100 units from the victim, thrown by the
/// attacker.
fn blast(victim: Victim, armor: Option<Armor>) -> Outcome {
    let mut sim = sim_on(true);
    let at = Vec3::new(0.0, 0.93, 10.0);
    let (target, thrower) = scene_at(&mut sim, at, victim, armor, 1000.0);
    he(&mut sim, thrower, at.with_y(0.05) + to_engine(Vec3::new(-100.0, 0.0, 0.0)));
    outcome(&sim, target, armor)
}

/// An HE of `thrower`'s going off at `at` (engine space), and 1.8 s for
/// it to.
fn he(sim: &mut Sim, thrower: Entity, at: Vec3) {
    give(sim.app.world_mut(), thrower, HEGRENADE).unwrap();
    sim.seconds(1.5);
    let weapon = sim
        .app
        .world()
        .get::<Inventory>(thrower)
        .unwrap()
        .weapons
        .iter()
        .copied()
        .find(|w| sim.app.world().get::<Weapon>(*w).is_some_and(|x| x.id == HEGRENADE))
        .unwrap();
    sim.app.world_mut().resource_mut::<Heard>().0.clear();
    spawn_projectile(sim.app.world_mut(), weapon, at, Vec3::ZERO).unwrap();
    sim.seconds(1.8);
}

/// The thrower's own HE on the floor 64 units in front of him, a teammate
/// 300 units away, `mp_friendlyfire ff`: what it did to the thrower.
fn own_blast(ff: u8) -> Outcome {
    let mut sim = sim_on(true);
    let at = Vec3::new(0.0, 0.93, 10.0);
    let (_, thrower) = scene_at(&mut sim, at, Teammate, None, 300.0);
    sim.app.world_mut().entity_mut(thrower).insert(Health {
        current: 5.0,
        max: 5.0,
    });
    sim.app.insert_resource(FriendlyFire(ff));
    let feet = sim.position(thrower).with_y(0.05);
    he(&mut sim, thrower, feet + to_engine(Vec3::new(64.0, 0.0, 0.0)));
    outcome(&sim, thrower, None)
}

const VEST: Armor = Armor {
    amount: 1.0,
    helmet: false,
};

/// Measured: an AK-47 in the chest of an armoured teammate with friendly
/// fire off left 100 health and 100 armour, no `player_hurt`, no sound, no
/// slowdown, no flinch; the same shot with friendly fire on took 9 and 1,
/// sounded kevlar and slowed to 0.65.
#[test]
fn a_teammates_bullet_with_friendly_fire_off_does_nothing() {
    let enemy = hit(AK47, 0.72, 200.0, Enemy, Some(VEST));
    assert!(enemy.health > 0.0 && enemy.armour > 0.0, "{enemy:?}");
    assert_eq!(enemy.heard, ["Player.DamageKevlar"]);
    assert_eq!(enemy.modifier, 0.65);
    for armor in [Some(VEST), None] {
        let o = hit(AK47, 0.72, 200.0, Teammate, armor);
        assert_eq!((o.health, o.armour), (0.0, 0.0), "{o:?}");
        assert!(o.heard.is_empty(), "silent, no flesh impact either: {:?}", o.heard);
        assert_eq!((o.modifier, o.punch), (1.0, Vec3::ZERO), "{o:?}");
    }
}

/// Measured: a teammate's knife slash with friendly fire off took no
/// armour and made no kevlar sound (only the knife's own hit sound); on
/// friendly fire, 5 health, 1 armour and the kevlar hit.
#[test]
fn a_teammates_knife_with_friendly_fire_off_does_nothing() {
    let enemy = hit(KNIFE, 0.5, 40.0, Enemy, Some(VEST));
    assert!(enemy.health > 0.0 && enemy.armour > 0.0, "{enemy:?}");
    assert_eq!(enemy.heard, ["Player.DamageKevlar"]);
    let o = hit(KNIFE, 0.5, 40.0, Teammate, Some(VEST));
    assert_eq!((o.health, o.armour), (0.0, 0.0), "{o:?}");
    assert!(o.heard.is_empty(), "{:?}", o.heard);
    assert_eq!(o.modifier, 1.0);
}

/// Measured: a teammate's HE with friendly fire off left 100 armour; an
/// enemy's (friendly fire on in the probe) took 64 and 11 and sounded
/// kevlar, on a bare player the flesh impact.
#[test]
fn blasts_take_armour_and_sound_only_where_they_land() {
    let enemy = blast(Enemy, Some(VEST));
    assert!(enemy.health > 0.0 && enemy.armour > 0.0, "{enemy:?}");
    assert_eq!(enemy.heard, ["Player.DamageKevlar"]);
    let bare = blast(Enemy, None);
    assert!(bare.health > 0.0, "{bare:?}");
    assert_eq!(bare.heard, ["Flesh.BulletImpact"]);
    for victim in [Teammate, Noclipping, Godlike] {
        let o = blast(victim, Some(VEST));
        assert_eq!((o.health, o.armour), (0.0, 0.0), "{victim:?}: {o:?}");
        assert!(o.heard.is_empty(), "{victim:?}: {:?}", o.heard);
    }
}

/// Inferred (grenades.md 5.5, capture C31): the ear-ringing goes with the
/// blast's damage, so players it can't hurt don't get it.
#[test]
fn only_players_a_blast_hurts_have_their_ears_ring() {
    let enemy = blast(Enemy, Some(VEST));
    assert!(enemy.deafened, "{enemy:?}");
    for victim in [Teammate, Noclipping, Godlike] {
        let o = blast(victim, Some(VEST));
        assert!(!o.deafened, "{victim:?}: {o:?}");
    }
}

/// Measured (grenades.md "Self damage"): the same throws hurt the thrower
/// alike with `mp_friendlyfire` 0 and 1 (64 units from the blast 74-79
/// either way, 161 units 34-35), by the bell every player gets; his ears
/// ring either way.
#[test]
fn the_throwers_own_he_hurts_him_whatever_friendly_fire_says() {
    let off = own_blast(0);
    let on = own_blast(1);
    assert!(off.health > 60.0 && off.health < 90.0, "{off:?}");
    assert_eq!(off.health, on.health, "{off:?} {on:?}");
    assert!(off.deafened && on.deafened, "{off:?} {on:?}");
}

/// Measured on a noclipping bot: an AK-47 in the bare chest took nothing,
/// made no sound and left its speed modifier at 1, but punched its view
/// −4 (as walking: a tenth of the damage, at most 4); in a bare head
/// (−12, 0, ±9); on a vest, no punch (the vest covers, takes nothing).
#[test]
fn a_noclipping_players_hit_still_flinches_and_nothing_else() {
    let chest = hit(AK47, 0.72, 200.0, Noclipping, None);
    assert_eq!((chest.health, chest.armour), (0.0, 0.0), "{chest:?}");
    assert!(chest.heard.is_empty(), "{:?}", chest.heard);
    assert_eq!(chest.modifier, 1.0);
    assert!(
        chest.punch.x < -3.0 && chest.punch.y == 0.0 && chest.punch.z == 0.0,
        "the walking flinch: {:?}",
        chest.punch
    );
    let head = hit(AK47, 0.92, 200.0, Noclipping, None);
    assert!(
        (head.punch.x + 12.0).abs() < 1e-4 && (head.punch.z.abs() - 9.0).abs() < 1e-4,
        "{:?}",
        head.punch
    );
    let vest = hit(AK47, 0.72, 200.0, Noclipping, Some(VEST));
    assert_eq!((vest.armour, vest.punch), (0.0, Vec3::ZERO), "{vest:?}");
}

/// God mode (ours; unmeasured): nothing lands, so nothing of the hit
/// either: no armour, sound, slowdown or flinch.
#[test]
fn god_mode_takes_no_armour_and_hears_nothing() {
    for gun in [AK47, KNIFE] {
        let distance = if gun == KNIFE { 40.0 } else { 200.0 };
        let o = hit(gun, 0.72, distance, Godlike, Some(VEST));
        assert_eq!((o.health, o.armour), (0.0, 0.0), "{gun}: {o:?}");
        assert!(o.heard.is_empty(), "{gun}: {:?}", o.heard);
        assert_eq!((o.modifier, o.punch), (1.0, Vec3::ZERO), "{gun}: {o:?}");
    }
}
