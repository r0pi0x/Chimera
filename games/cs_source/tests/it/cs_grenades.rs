//! Scenario tests for CS:S's grenades (specs/cs_source/grenades.md) on the
//! greybox map at CS:S's tick (0.015 s): buying and carry limits (G31),
//! the pin and release timing (G17, G30), the throw (G1), flight and
//! bounces against floors, walls and players (G10-G14), the fuse (G16), HE
//! damage by distance, walls, static props, props and players in between
//! (G18-G20, Q10), the bounce heights and stop time of a throw (spec 4), kill
//! credit, flash blindness, what blasts and flashes do to hearing (5.5,
//! 6.4) and the HE's view shake, the smoke cloud's life and bots' sight through
//! it, round restarts, and a primed grenade dropped by the dying.

use crate::common::sim;
use bevy::{ecs::message::Messages, prelude::*};
use crate::stack::{
    bot::{Bot, BotConfig},
    core::{Blinded, Damage, DamageKind, Deafened, Died, Health, Hitgroup, SightBlocker, Team},
    games::cs_source::{
        TICK_INTERVAL,
        grenades::{
            DRAW_TIME, FLASH_MUFFLE, FLASHBANG, HEGRENADE, SHOCK_MUFFLE, SMOKEGRENADE, THROW_TIME, he_shake,
        },
        weapons::{AK47, CsWeaponsPlugin},
    },
    greybox::{self, GreyboxMapPlugin},
    harness::{CsSim, Sim},
    movement::placeholder,
    weapon::{
        Inventory, Weapon, WeaponEvent, WeaponEventKind,
        economy::{Money, buy},
        give,
        grenade::{Detonated, Projectile, SmokeCloud, Throwable, spawn_projectile},
    },
};

const UNIT: f32 = 0.0254;

fn active(sim: &Sim, p: Entity) -> Option<Entity> {
    sim.app.world().get::<Inventory>(p).unwrap().active
}

fn active_id(sim: &Sim, p: Entity) -> Option<&'static str> {
    active(sim, p)
        .and_then(|w| sim.app.world().get::<Weapon>(w))
        .map(|w| w.id)
}

fn grenade_of(sim: &Sim, p: Entity, id: &str) -> Option<Entity> {
    let inv = sim.app.world().get::<Inventory>(p)?;
    inv.weapons
        .iter()
        .copied()
        .find(|w| sim.app.world().get::<Weapon>(*w).is_some_and(|x| x.id == id))
}

fn projectiles(sim: &mut Sim) -> Vec<(Entity, Projectile, Vec3)> {
    let w = sim.app.world_mut();
    w.query::<(Entity, &Projectile, &Transform)>()
        .iter(w)
        .map(|(e, p, t)| (e, p.clone(), t.translation))
        .collect()
}

fn clouds(sim: &mut Sim) -> Vec<(SmokeCloud, SightBlocker)> {
    let w = sim.app.world_mut();
    w.query::<(&SmokeCloud, &SightBlocker)>()
        .iter(w)
        .map(|(c, s)| (c.clone(), *s))
        .collect()
}

fn health(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<Health>(p).unwrap().current
}

/// A character at `at` holding `id`, drawn.
fn holder(sim: &mut Sim, at: Vec3, id: &str) -> Entity {
    let p = sim.spawn_character(at, placeholder::ID);
    sim.ticks(1);
    give(sim.app.world_mut(), p, id).unwrap();
    sim.ticks(1);
    assert_eq!(active_id(sim, p), Some(id));
    sim.seconds(DRAW_TIME as f64 + 0.05);
    p
}

/// A grenade of `id` (held by a fresh character far away, so it's the
/// thrower) placed at `at` with `velocity`.
fn place(sim: &mut Sim, id: &str, at: Vec3, velocity: Vec3) -> (Entity, Entity) {
    let thrower = holder(sim, Vec3::new(36.0, 1.0, 36.0), id);
    let weapon = grenade_of(sim, thrower, id).unwrap();
    let g = spawn_projectile(sim.app.world_mut(), weapon, at, velocity).unwrap();
    (thrower, g)
}

fn died(sim: &Sim) -> Vec<Died> {
    let messages = sim.app.world().resource::<Messages<Died>>();
    let mut cursor = messages.get_cursor();
    cursor.read(messages).cloned().collect()
}

#[test]
fn buying_respects_prices_carry_limits_and_keeps_the_hand() {
    let mut sim = sim();
    let p = holder(&mut sim, greybox::SPAWNS[0], AK47);
    sim.app.world_mut().entity_mut(p).insert(Money(2000));
    let w = sim.app.world_mut();
    buy(w, p, "flashbang").unwrap();
    buy(w, p, "flashbang").unwrap();
    // G31: a third is refused and costs nothing.
    assert_eq!(buy(w, p, "flashbang").unwrap_err(), "You cannot carry any more.");
    assert_eq!(w.get::<Money>(p), Some(&Money(1600)));
    buy(w, p, "hegrenade").unwrap();
    assert!(buy(w, p, "hegrenade").is_err());
    buy(w, p, "smokegrenade").unwrap();
    assert!(buy(w, p, "smokegrenade").is_err());
    assert_eq!(w.get::<Money>(p), Some(&Money(1000)));
    sim.ticks(2);
    // All three types at once in slot 3 (the 4th key), and the rifle still
    // in hand.
    assert_eq!(active_id(&sim, p), Some(AK47));
    let flash = grenade_of(&sim, p, FLASHBANG).unwrap();
    assert_eq!(sim.app.world().get::<Throwable>(flash).unwrap().count, 2);
    for id in [HEGRENADE, FLASHBANG, SMOKEGRENADE] {
        let w = grenade_of(&sim, p, id).unwrap();
        assert_eq!(sim.app.world().get::<Weapon>(w).unwrap().slot, 3);
    }
}

#[test]
fn pin_release_throw_fuse_and_the_spent_weapon_goes() {
    let mut sim = sim();
    let p = holder(&mut sim, greybox::SPAWNS[0], HEGRENADE);
    // G30: the pin comes out on the press.
    sim.intent(p).fire = true;
    sim.ticks(1);
    let w = grenade_of(&sim, p, HEGRENADE).unwrap();
    assert!(sim.app.world().get::<Throwable>(w).unwrap().pin);
    sim.ticks(30);
    assert!(projectiles(&mut sim).is_empty(), "held: nothing thrown yet");
    // G17: release seen on tick R, the grenade appears on R + 7 ...
    sim.intent(p).fire = false;
    sim.ticks(7);
    assert!(projectiles(&mut sim).is_empty());
    sim.ticks(1);
    let thrown = projectiles(&mut sim);
    assert_eq!(thrown.len(), 1);
    // G1: level view, standing: 600 units/s 10 degrees up (after one tick
    // of 0.4 gravity).
    let v = thrown[0].1.velocity / UNIT;
    assert!((v.z + 590.88).abs() < 0.1 && v.x.abs() < 0.1, "{v}");
    assert!((v.y - (104.19 - 320.0 * 0.015)).abs() < 0.1, "{v}");
    assert_eq!(thrown[0].1.thrower, Some(p));
    // ... and goes off 104 ticks later (G16: 1.56 s, R + 111).
    sim.ticks(103);
    assert_eq!(projectiles(&mut sim).len(), 1);
    sim.ticks(1);
    assert!(projectiles(&mut sim).is_empty());
    // The last grenade thrown, the weapon is gone and the rifle drawn.
    sim.seconds(THROW_TIME as f64);
    assert!(grenade_of(&sim, p, HEGRENADE).is_none());
    sim.ticks(2);
    assert_eq!(active_id(&sim, p), Some(AK47));
}

/// Thrown flush against a wall, the grenade starts where its box meets
/// the wall and bounces back, never inside or beyond it (measured
/// 2026-10-18, grenades.md Q6: 4 units off a panel at 45°).
#[test]
fn a_grenade_thrown_into_a_wall_starts_in_front_of_it() {
    let mut sim = sim();
    // Facing the long crate's face at z -19.5, the hull (16 units) a
    // hair off it.
    let p = holder(&mut sim, Vec3::new(0.0, 1.0, -19.5 + 16.2 * UNIT), FLASHBANG);
    sim.intent(p).yaw = 0.0;
    sim.intent(p).pitch = 0.0;
    sim.ticks(2);
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    let mut seen = 0;
    for _ in 0..100 {
        sim.ticks(1);
        for (_, _, at) in projectiles(&mut sim) {
            seen += 1;
            assert!(at.z > -19.5 + 1.9 * UNIT, "the grenade at {at} is in or past the wall");
        }
    }
    assert!(seen > 50, "thrown");
}

#[test]
fn a_second_flashbang_is_drawn_after_the_first_throw() {
    let mut sim = sim();
    let p = holder(&mut sim, greybox::SPAWNS[0], FLASHBANG);
    buy(sim.app.world_mut(), p, "flashbang").unwrap();
    // A tap: press one tick, release the next (throws at once).
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    sim.ticks(10);
    assert_eq!(projectiles(&mut sim).len(), 1);
    sim.seconds(THROW_TIME as f64 + DRAW_TIME as f64 + 0.05);
    assert_eq!(active_id(&sim, p), Some(FLASHBANG));
    let w = grenade_of(&sim, p, FLASHBANG).unwrap();
    assert_eq!(sim.app.world().get::<Throwable>(w).unwrap().count, 1);
    // It can be thrown again.
    sim.intent(p).fire = true;
    sim.ticks(1);
    assert!(sim.app.world().get::<Throwable>(w).unwrap().pin);
}

#[test]
fn view_model_events_pin_then_throw() {
    let mut sim = sim();
    let p = holder(&mut sim, greybox::SPAWNS[0], SMOKEGRENADE);
    sim.intent(p).fire = true;
    sim.ticks(3);
    sim.intent(p).fire = false;
    sim.ticks(1);
    let messages = sim.app.world().resource::<Messages<WeaponEvent>>();
    let mut cursor = messages.get_cursor();
    let kinds: Vec<String> = cursor
        .read(messages)
        .filter(|e| e.owner == p)
        .map(|e| format!("{:?}", e.kind))
        .collect();
    assert!(kinds.iter().any(|k| k == "Thrown"), "{kinds:?}");
    assert!(matches!(WeaponEventKind::PinPulled, WeaponEventKind::PinPulled));
}

/// Measured 2026-10-18 (grenades.md Q17): `drop` with an HE grenade in
/// hand leaves it there (a pistol the same way is dropped).
#[test]
fn the_drop_key_keeps_a_grenade_in_hand() {
    use crate::stack::weapon::drop::drop_weapon;
    let mut sim = sim();
    for (k, id) in [HEGRENADE, FLASHBANG, SMOKEGRENADE].into_iter().enumerate() {
        let p = holder(&mut sim, Vec3::new(k as f32 * 3.0, 0.9, 8.0), id);
        assert!(drop_weapon(sim.app.world_mut(), p, true).is_none(), "{id} dropped");
        sim.ticks(2);
        assert_eq!(active_id(&sim, p), Some(id));
    }
}

#[test]
fn dropped_grenade_bounces_and_rests_on_the_floor() {
    let mut sim = sim();
    let (_, g) = place(&mut sim, HEGRENADE, Vec3::new(0.0, 1.0, 5.0), Vec3::ZERO);
    sim.seconds(1.4);
    let p = projectiles(&mut sim).into_iter().find(|x| x.0 == g).unwrap();
    assert!(p.1.resting, "at rest after a few bounces");
    assert_eq!(p.1.velocity, Vec3::ZERO);
    assert!((p.2.y - 2.0 * UNIT).abs() < 0.01, "lies on the floor: {}", p.2.y);
}

#[test]
fn walls_keep_045_and_players_0135() {
    let mut sim = sim();
    // G11: into the east wall (inner face x = 40) at 10 m/s.
    let (_, g) = place(&mut sim, SMOKEGRENADE, Vec3::new(38.0, 2.0, 0.0), Vec3::X * 10.0);
    sim.ticks(20);
    let p = projectiles(&mut sim).into_iter().find(|x| x.0 == g).unwrap();
    assert!((p.1.velocity.x + 4.5).abs() < 0.05, "{}", p.1.velocity);
    // G12: into a player.
    let victim = sim.spawn_character(Vec3::new(5.0, 0.9, -2.0), placeholder::ID);
    sim.ticks(1);
    let (_, g) = place(&mut sim, SMOKEGRENADE, Vec3::new(2.0, 1.0, -2.0), Vec3::X * 10.0);
    sim.ticks(20);
    let p = projectiles(&mut sim).into_iter().find(|x| x.0 == g).unwrap();
    assert!((p.1.velocity.x + 1.35).abs() < 0.05, "{}", p.1.velocity);
    assert!(health(&sim, victim) > 0.99);
}

/// The blast point of a grenade resting on the floor: 0.6 units up.
fn he_damage_at(sim: &mut Sim, victim_at: Vec3, grenade_at: Vec3) -> (f32, Entity, Entity) {
    let victim = sim.spawn_character(victim_at, placeholder::ID);
    sim.ticks(1);
    let (thrower, _) = place(sim, HEGRENADE, grenade_at, Vec3::ZERO);
    let before = health(sim, victim);
    sim.seconds(1.7);
    (before - health(sim, victim), victim, thrower)
}

#[test]
fn he_damage_falls_off_with_distance_g18() {
    let mut sim = sim();
    let at = Vec3::new(0.0, 0.9, 10.0);
    let g = Vec3::new(0.0, 2.0 * UNIT, 10.0 - 200.0 * UNIT);
    let (lost, victim, _) = he_damage_at(&mut sim, at, g);
    // Aimed between 70 % and 100 % of the eye height above the feet.
    let eye = sim.position(victim) + sim.state(victim).eye_offset;
    let src = Vec3::new(g.x, 1.6 * UNIT, g.z);
    let far = (eye - src).length() / UNIT;
    let near = (eye.with_y(0.7 * eye.y) - src).length() / UNIT;
    // CS:S today (sv_legacy_grenade_damage 0, measured): a bell of σ
    // 350/3 units.
    let bell = |d: f32| 100.0 * (-(d * d) / (2.0 * (350.0f32 / 3.0).powi(2))).exp();
    let (lo, hi) = (bell(far), bell(near));
    let hp = lost * 100.0;
    assert!(hp >= lo.floor() - 0.01 && hp <= hi + 0.01, "{hp} not in {lo}..{hi}");
    // The original game's linear falloff with sv_legacy_grenade_damage 1.
    let mut sim2 = self::sim();
    sim2.app.world_mut().resource_mut::<crate::stack::console::Console>().submit("sv_legacy_grenade_damage 1");
    let (lost, ..) = he_damage_at(&mut sim2, at, g);
    let (lo, hi) = (100.0 - far / 3.5, 100.0 - near / 3.5);
    let hp = lost * 100.0;
    assert!(hp >= lo.floor() - 0.01 && hp <= hi + 0.01, "legacy {hp} not in {lo}..{hi}");
}

#[test]
fn he_is_blocked_by_walls_not_players_g19_g20() {
    let mut sim = sim();
    // Behind the long crate (z -20.5..-19.5, 3 m tall).
    let (lost, ..) = he_damage_at(&mut sim, Vec3::new(0.0, 0.9, -23.0), Vec3::new(0.0, 0.06, -18.0));
    assert_eq!(lost, 0.0, "the wall blocks it");
    // Another player between doesn't stop it (it shields a quarter).
    let mut sim = self::sim();
    let shield = sim.spawn_character(Vec3::new(5.0, 0.9, 8.0), placeholder::ID);
    let (lost, ..) = he_damage_at(&mut sim, Vec3::new(5.0, 0.9, 6.0), Vec3::new(5.0, 0.06, 10.0));
    assert!(lost > 0.2, "{lost}");
    assert!(health(&sim, shield) < 1.0);
    // Out of reach (G21).
    let mut sim = self::sim();
    let (lost, ..) = he_damage_at(&mut sim, Vec3::new(0.0, 0.9, 0.0), Vec3::new(0.0, 0.06, 10.5));
    assert_eq!(lost, 0.0);
}

/// Measured 2026-10-18 (grenades.md Q10): a static prop stops a blast
/// whole, as the world; a prop entity (a crate) in the way leaves 0.767
/// of it, a player 0.733.
#[test]
fn props_and_players_between_shield_part_of_a_blast() {
    use avian3d::prelude::{Collider, RigidBody};
    use crate::stack::map::{MapPropCollider, PropEntity};
    let victim_at = Vec3::new(5.0, 0.9, 6.0);
    let grenade_at = Vec3::new(5.0, 0.06, 9.0);
    // A 2 m box between, at z 7.5: a static prop or a prop entity.
    let lost = |what: u8| {
        let mut sim = self::sim();
        let mut e = sim.app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(2.0, 2.0, 0.5),
            Transform::from_xyz(5.0, 1.0, 7.5),
        ));
        match what {
            0 => {
                e.despawn();
            }
            1 => {
                e.insert(MapPropCollider);
            }
            _ => {
                e.insert((MapPropCollider, PropEntity(0)));
            }
        }
        he_damage_at(&mut sim, victim_at, grenade_at).0
    };
    let open = lost(0);
    assert!(open > 0.3, "{open}");
    assert_eq!(lost(1), 0.0, "a static prop blocks it");
    let crate_ = lost(2) / open;
    assert!((0.7..0.83).contains(&crate_), "behind a crate: {crate_} of it");
    // Behind another player.
    let mut sim = self::sim();
    sim.spawn_character(Vec3::new(5.0, 0.9, 7.5), placeholder::ID);
    let player = he_damage_at(&mut sim, victim_at, grenade_at).0 / open;
    assert!((0.66..0.8).contains(&player), "behind a player: {player} of it");
}

#[test]
fn he_kills_are_credited_to_the_thrower_with_the_grenade() {
    let mut sim = sim();
    let victim = sim.spawn_character(Vec3::new(0.0, 0.9, 5.0), placeholder::ID);
    sim.ticks(1);
    sim.app.world_mut().get_mut::<Health>(victim).unwrap().current = 0.1;
    let (thrower, _) = place(&mut sim, HEGRENADE, Vec3::new(0.0, 0.06, 4.0), Vec3::ZERO);
    let mut found = None;
    for _ in 0..130 {
        sim.ticks(1);
        if let Some(d) = died(&sim).into_iter().find(|d| d.entity == victim) {
            found = Some(d);
            break;
        }
    }
    let d = found.expect("the victim died");
    assert_eq!(d.attacker, Some(thrower));
    assert_eq!(d.damage.weapon, Some(HEGRENADE));
    assert_eq!(d.damage.kind, DamageKind::Blast);
}

#[test]
fn flashes_blind_by_view_and_walls() {
    let mut sim = sim();
    let facing = sim.spawn_character(Vec3::new(0.0, 0.9, 8.0), placeholder::ID);
    let away = sim.spawn_character(Vec3::new(2.0, 0.9, 8.0), placeholder::ID);
    let hidden = sim.spawn_character(Vec3::new(0.0, 0.9, -23.0), placeholder::ID);
    // Grenade at (0, 1.6, 5): `facing` looks at it (-Z), `away` looks +Z.
    sim.intent(away).yaw = std::f32::consts::PI;
    sim.ticks(1);
    let (thrower, _) = place(&mut sim, FLASHBANG, Vec3::new(1.0, 1.6, 5.0), Vec3::ZERO);
    // Hold it in the air: it would fall; the fuse is 1.56 s, so watch the
    // blind state right after.
    sim.seconds(1.6);
    let blind = |sim: &Sim, e: Entity| sim.app.world().get::<Blinded>(e).copied();
    let f = blind(&sim, facing).expect("facing is blinded");
    let a = blind(&sim, away).expect("facing away still is, less");
    assert!(f.alpha > a.alpha || f.end > a.end, "{f:?} vs {a:?}");
    assert!(blind(&sim, hidden).is_none(), "behind the crate");
    // The thrower (far away, 45 m) isn't reached by the fit's radius.
    assert!(blind(&sim, thrower).is_none());
    // It wears off (6.8 s at most).
    sim.seconds(7.5);
    let now = sim.app.world().resource::<Time>().elapsed_secs_f64();
    assert_eq!(blind(&sim, facing).unwrap().alpha_at(now), 0.0);
}

/// Measured 2026-10-18 (grenades.md Q12): a flash going off in a smoke
/// cloud blinds for 0.56 of the time it would in the open.
#[test]
fn a_flash_in_smoke_blinds_for_less() {
    let blinded_for = |smoke: bool| {
        let mut sim = sim();
        let victim = sim.spawn_character(Vec3::new(0.0, 0.9, 8.0), placeholder::ID);
        if smoke {
            sim.app.world_mut().spawn(SightBlocker {
                centre: Vec3::new(0.0, 1.0, 5.0),
                radius: 2.0,
            });
        }
        sim.ticks(1);
        place(&mut sim, FLASHBANG, Vec3::new(0.0, 1.6, 5.0), Vec3::ZERO);
        sim.seconds(1.6);
        let b = sim.app.world().get::<Blinded>(victim).copied().expect("blinded");
        let now = sim.app.world().resource::<Time>().elapsed_secs_f64();
        (b.end - now, b.alpha)
    };
    let (open, a) = blinded_for(false);
    let (smoked, b) = blinded_for(true);
    assert_eq!(a, b, "as white");
    let ratio = smoked / open;
    assert!((0.5..0.62).contains(&ratio), "{smoked} of {open} s: {ratio}");
}

/// Measured 2026-10-18 (grenades.md Q12, `flashmeas.py`): another player
/// or a crate on the line from the flash to the eye stops it whole; a
/// player beside the line doesn't, nor does glass (de_nuke's windows).
#[test]
fn players_and_props_on_the_line_hide_a_flash() {
    use crate::stack::map::{MapBrushCollider, MapPropCollider, PropEntity, SeeThrough};
    use avian3d::prelude::{Collider, RigidBody};
    // 0: nothing, 1: a player between, 2: a player beside, 3: a crate.
    let blinded = |what: u8| {
        let mut sim = sim();
        let victim = sim.spawn_character(Vec3::new(0.0, 0.9, 8.0), placeholder::ID);
        match what {
            1 => {
                sim.spawn_character(Vec3::new(0.0, 0.9, 6.5), placeholder::ID);
            }
            2 => {
                sim.spawn_character(Vec3::new(1.2, 0.9, 6.5), placeholder::ID);
            }
            3 => {
                sim.app.world_mut().spawn((
                    RigidBody::Static,
                    Collider::cuboid(1.6, 1.6, 1.6),
                    Transform::from_xyz(0.0, 0.8, 6.5),
                    MapPropCollider,
                    PropEntity(0),
                ));
            }
            // A pane of glass across the line (a map volume with window
            // contents), then the same pane as a solid wall.
            4 | 5 => {
                let mut e = sim.app.world_mut().spawn((
                    RigidBody::Static,
                    Collider::cuboid(4.0, 4.0, 0.1),
                    Transform::from_xyz(0.0, 1.0, 6.5),
                    MapBrushCollider,
                ));
                if what == 4 {
                    e.insert(SeeThrough);
                }
            }
            _ => {}
        }
        sim.ticks(1);
        place(&mut sim, FLASHBANG, Vec3::new(0.0, 0.06, 5.0), Vec3::ZERO);
        sim.seconds(1.6);
        sim.app.world().get::<Blinded>(victim).is_some()
    };
    assert!(blinded(0), "in the open");
    assert!(!blinded(1), "behind a player");
    assert!(blinded(2), "a player beside the line");
    assert!(!blinded(3), "behind a crate");
    assert!(blinded(4), "through glass");
    assert!(!blinded(5), "behind a wall");
}

#[test]
fn smoke_cloud_grows_blocks_bots_and_goes() {
    let mut sim = sim();
    sim.app.world_mut().resource_mut::<BotConfig>().stop = 1;
    sim.app.world_mut().resource_mut::<BotConfig>().dont_shoot = 1;
    // A bot at z 14 facing an enemy at z -6, the grenade between.
    let bot = sim.spawn_character(Vec3::new(0.0, 0.9, 14.0), placeholder::ID);
    let enemy = sim.spawn_character(Vec3::new(0.0, 0.9, -6.0), placeholder::ID);
    sim.app.world_mut().entity_mut(bot).insert((Team(1), Bot::default()));
    sim.app.world_mut().entity_mut(enemy).insert(Team(2));
    sim.ticks(5);
    let target = |sim: &Sim| sim.app.world().get::<Bot>(bot).unwrap().target;
    assert_eq!(target(&sim), Some(enemy), "in plain sight");
    let (thrower, g) = place(&mut sim, SMOKEGRENADE, Vec3::new(0.0, 0.06, 4.0), Vec3::ZERO);
    // The thrower is the bot's teammate (else it would be seen instead).
    sim.app.world_mut().entity_mut(thrower).insert(Team(1));
    sim.seconds(1.6);
    let c = clouds(&mut sim);
    assert_eq!(c.len(), 1, "popped after the fuse once still");
    sim.seconds(1.0);
    let (cloud, blocker) = clouds(&mut sim).remove(0);
    assert!(
        (blocker.radius - 240.0 * UNIT * 0.75).abs() < 1e-3,
        "{}",
        blocker.radius
    );
    assert_eq!(cloud.grenade, Some(g));
    sim.ticks(5);
    assert_eq!(target(&sim), None, "hidden by the smoke");
    // Gone by 20 s (measured), with the spent grenade.
    sim.seconds(17.0);
    assert_eq!(clouds(&mut sim).len(), 1, "fading at 18 s");
    sim.seconds(2.0);
    assert!(clouds(&mut sim).is_empty());
    assert!(sim.app.world().get_entity(g).is_err());
    sim.ticks(5);
    assert_eq!(target(&sim), Some(enemy));
}

/// The smoke cloud is drawn (spec 7.3): its sprites in the particle pool,
/// in the cloud, with the cloud's alpha, also on a map loaded after the
/// game has run a while (the main menu, the last map). The fixed clock
/// used to start over at a map load while the frame's clock ran on: the
/// cloud, stamped on the tick's clock and drawn by the frame's, looked
/// that much older, so after half a minute at the menu it was born fully
/// faded ("smoke grenades are just not working"), while bots still
/// couldn't see through it.
#[test]
fn smoke_clouds_are_drawn_after_a_map_load() {
    use crate::stack::map::particles::{MapParticles, ParticleMaterial, ParticleMaterials, Particles};
    let mut sim = sim();
    sim.seconds(30.0);
    crate::stack::swap_map(
        sim.app.world_mut(),
        "greybox",
        None,
        std::time::Duration::from_secs_f64(TICK_INTERVAL),
    );
    // Stand-in for the map's material (no install needed).
    sim.app.insert_resource(ParticleMaterials(MapParticles {
        materials: vec![ParticleMaterial {
            name: "particle/particle_smokegrenade1".into(),
            ..default()
        }],
    }));
    sim.ticks(2);
    let at = Vec3::new(0.0, 0.06, 4.0);
    place(&mut sim, SMOKEGRENADE, at, Vec3::ZERO);
    sim.seconds(1.6);
    assert_eq!(clouds(&mut sim).len(), 1, "popped");
    sim.seconds(2.0);
    let (cloud, _) = clouds(&mut sim).remove(0);
    let pool = sim.app.world().resource::<Particles>();
    let sprites: Vec<_> = pool
        .groups
        .iter()
        .flat_map(|g| &g.particles)
        .filter(|p| p.material == 0)
        .collect();
    assert!(sprites.len() >= 32, "{} of 64 sprites drawn", sprites.len());
    // The inner 70 % at full alpha, the edge fading (spec 7.3).
    let solid = sprites.iter().filter(|s| s.alpha > 0.99).count();
    assert!(solid >= 16, "{solid} sprites at full alpha");
    for s in &sprites {
        assert!(s.alpha > 0.0 && s.alpha <= 1.0, "a sprite drawn at alpha {}", s.alpha);
        assert!(
            s.position.distance(cloud.centre) < 240.0 * UNIT,
            "a sprite out of the cloud at {}",
            s.position
        );
    }
}

#[test]
fn a_new_round_clears_grenades_and_clouds() {
    let mut sim = sim();
    place(&mut sim, SMOKEGRENADE, Vec3::new(0.0, 0.06, 4.0), Vec3::ZERO);
    sim.seconds(2.0);
    place(&mut sim, HEGRENADE, Vec3::new(3.0, 0.06, 4.0), Vec3::ZERO);
    assert_eq!(clouds(&mut sim).len(), 1);
    assert!(!projectiles(&mut sim).is_empty());
    crate::stack::core::restart_round(sim.app.world_mut());
    sim.ticks(2);
    assert!(clouds(&mut sim).is_empty());
    assert!(projectiles(&mut sim).is_empty());
}

#[test]
fn dying_with_the_pin_out_drops_a_live_grenade() {
    let mut sim = sim();
    let p = holder(&mut sim, greybox::SPAWNS[0], HEGRENADE);
    sim.intent(p).fire = true;
    sim.ticks(5);
    sim.app.world_mut().write_message(Damage {
        force: bevy::math::Vec3::ZERO,
        target: p,
        attacker: None,
        amount: 2.0,
        point: Vec3::ZERO,
        dir: Vec3::Y,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Generic,
        weapon: None,
    });
    sim.ticks(3);
    let thrown = projectiles(&mut sim);
    assert_eq!(thrown.len(), 1, "dropped live");
    assert!(thrown[0].1.velocity.xz().length() < 0.01, "only the body's velocity");
}

/// Running when it dies: the dropped grenade keeps the runner's velocity
/// (CS:S, measured 2026-10-18, grenades.md Q17: a bot killed at 250 units/s
/// with the pin out left a projectile moving at 250 along its run).
#[test]
fn dying_on_the_run_with_the_pin_out_drops_it_at_the_runners_speed() {
    use crate::stack::core::Velocity;
    // CS:S's own movement (it keeps `Velocity`).
    let mut sim = Sim::new(
        CsSim,
        (GreyboxMapPlugin, CsWeaponsPlugin, crate::stack::games::cs_source::movement::SourceMovementPlugin),
    );
    sim.set_tick_interval(TICK_INTERVAL);
    let p = sim.spawn_character(greybox::SPAWNS[0], crate::stack::games::cs_source::movement::ID);
    sim.ticks(1);
    give(sim.app.world_mut(), p, HEGRENADE).unwrap();
    sim.seconds(DRAW_TIME as f64 + 0.05);
    assert_eq!(active_id(&sim, p), Some(HEGRENADE));
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(0.6);
    sim.intent(p).fire = true;
    sim.ticks(5);
    let running = sim.app.world().get::<Velocity>(p).unwrap().0;
    assert!(running.xz().length() > 200.0 * UNIT, "running: {running}");
    sim.app.world_mut().write_message(Damage {
        force: bevy::math::Vec3::ZERO,
        target: p,
        attacker: None,
        amount: 2.0,
        point: Vec3::ZERO,
        dir: Vec3::Y,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Generic,
        weapon: None,
    });
    sim.ticks(1);
    let thrown = projectiles(&mut sim);
    assert_eq!(thrown.len(), 1, "dropped live");
    let v = thrown[0].1.velocity.xz();
    assert!((v - running.xz()).length() < 0.02 * running.xz().length(), "{v} vs the runner's {running}");
}

/// Messages of type `M` written while the sim runs `seconds`, tick by tick.
fn collect<M: Message + Clone>(sim: &mut Sim, seconds: f64) -> Vec<M> {
    let mut out = Vec::new();
    let mut cursor = sim.app.world().resource::<Messages<M>>().get_cursor_current();
    for _ in 0..(seconds / TICK_INTERVAL as f64).ceil() as usize {
        sim.ticks(1);
        out.extend(cursor.read(sim.app.world().resource::<Messages<M>>()).cloned());
    }
    out
}

#[test]
fn flashes_muffle_hearing_by_blindness() {
    let mut sim = sim();
    // Close and looking at it: blind ~6.8 s, the long muffle; facing away:
    // 0.4 of that (measured), the medium one.
    let facing = sim.spawn_character(Vec3::new(0.0, 0.9, 8.0), placeholder::ID);
    let away = sim.spawn_character(Vec3::new(2.0, 0.9, 8.0), placeholder::ID);
    sim.intent(away).yaw = std::f32::consts::PI;
    sim.ticks(1);
    place(&mut sim, FLASHBANG, Vec3::new(1.0, 1.6, 5.0), Vec3::ZERO);
    let deaf = collect::<Deafened>(&mut sim, 1.7);
    let of = |e: Entity| deaf.iter().find(|d| d.target == e).map(|d| d.effect);
    assert_eq!(of(facing), Some(FLASH_MUFFLE[0]));
    assert_eq!(of(away), Some(FLASH_MUFFLE[1]));
    let b = sim.app.world().get::<Blinded>(facing).unwrap();
    let a = sim.app.world().get::<Blinded>(away).unwrap();
    assert!(b.end - b.fade_start > a.end - a.fade_start || b.end > a.end);
}

#[test]
fn he_blasts_shock_hearing_and_shake() {
    let mut sim = sim();
    // 100 units away: ~70 hp of blast, the shock muffle (with its ring).
    let near = sim.spawn_character(Vec3::new(0.0, 0.9, 10.0), placeholder::ID);
    // 400 units: out of reach, nothing.
    let far = sim.spawn_character(Vec3::new(3.0, 0.9, 10.0 + 400.0 * UNIT), placeholder::ID);
    sim.ticks(1);
    place(&mut sim, HEGRENADE, Vec3::new(0.0, 0.06, 10.0 - 100.0 * UNIT), Vec3::ZERO);
    let mut deaf_at = sim.app.world().resource::<Messages<Deafened>>().get_cursor_current();
    let mut det_at = sim.app.world().resource::<Messages<Detonated>>().get_cursor_current();
    let (mut deaf, mut det) = (Vec::new(), Vec::new());
    for _ in 0..120 {
        sim.ticks(1);
        let w = sim.app.world();
        deaf.extend(deaf_at.read(w.resource::<Messages<Deafened>>()).cloned());
        det.extend(det_at.read(w.resource::<Messages<Detonated>>()).cloned());
    }
    assert_eq!(deaf.iter().find(|d| d.target == near).map(|d| d.effect), Some(SHOCK_MUFFLE));
    assert!(deaf.iter().all(|d| d.target != far));
    assert_eq!(det.len(), 1);
    assert_eq!(det[0].shake, Some(he_shake()));
}

/// Source's `use <weapon>` draws a carried weapon by name (scripts and
/// binds use it: `use weapon_smokegrenade`); an unknown or uncarried one
/// changes nothing.
#[test]
fn use_draws_a_carried_weapon_by_name() {
    use crate::stack::core::LocalPlayer;
    let mut sim = sim();
    let p = holder(&mut sim, greybox::SPAWNS[0], SMOKEGRENADE);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    let run = |sim: &mut Sim, line: &str| {
        let words: Vec<String> = line.split(' ').map(String::from).collect();
        crate::stack::console::execute(sim.app.world_mut(), &words, 0);
        sim.seconds(1.0);
    };
    run(&mut sim, "use weapon_ak47");
    assert_eq!(active_id(&sim, p), Some(AK47));
    run(&mut sim, "use weapon_flashbang");
    assert_eq!(active_id(&sim, p), Some(AK47), "not carried");
    run(&mut sim, "use weapon_smokegrenade");
    assert_eq!(active_id(&sim, p), Some(SMOKEGRENADE));
}

/// Step 8: a grenade entering water plays `BaseEntity.EnterWater`, one
/// leaving it `BaseEntity.ExitWater` (the greybox's tank).
#[test]
fn grenades_play_water_sounds_crossing_the_surface() {
    use crate::stack::map::PlaySound;
    let mut sim = sim();
    sim.ticks(2);
    let centre = (greybox::TANK_MIN + greybox::TANK_MAX) / 2.0;
    let entries = |sounds: Vec<PlaySound>| sounds.into_iter().map(|s| s.entry).collect::<Vec<_>>();
    // From above, falling in.
    let (_, g) = place(&mut sim, SMOKEGRENADE, Vec3::new(centre.x, 6.0, centre.y), Vec3::ZERO);
    let heard = entries(collect::<PlaySound>(&mut sim, 1.5));
    assert_eq!(heard.iter().filter(|e| *e == "BaseEntity.EnterWater").count(), 1, "{heard:?}");
    assert!(!heard.iter().any(|e| e == "BaseEntity.ExitWater"), "{heard:?}");
    sim.app.world_mut().despawn(g);
    // From below, thrown up through the surface.
    let (_, _) = place(&mut sim, SMOKEGRENADE, Vec3::new(centre.x, 1.0, centre.y), Vec3::Y * 40.0);
    let heard = entries(collect::<PlaySound>(&mut sim, 1.0));
    assert!(heard.iter().any(|e| e == "BaseEntity.ExitWater"), "{heard:?}");
}

/// The spec's flight rules (grenades.md section 4) run by hand in inches on
/// a floor `y = 0` and a wall `x = wall`, for a ±2 in box: gravity 320 in/s²
/// (0.4 x 800) integrated per 0.015 s tick as the exact parabola, mirror
/// then scale by 0.45 on every contact, a floor contact keeps the rest of
/// the tick (a second sweep of the new velocity) and a wall contact loses
/// it, stop when the scaled velocity is under 30 in/s (floor: rest; wall:
/// velocity 0). Returns each tick's (x, y, vx, vy) of the box centre.
fn spec_flight(mut p: (f32, f32), mut v: (f32, f32), wall: Option<f32>, ticks: usize) -> Vec<[f32; 4]> {
    let (dt, g, e, half) = (0.015f32, 320.0f32, 0.45f32, 2.0f32);
    let mut out = vec![];
    let mut rest = false;
    for _ in 0..ticks {
        if !rest {
            let vy2 = v.1 - g * dt;
            let mut step = (v.0 * dt, (v.1 + vy2) / 2.0 * dt);
            v.1 = vy2;
            for _ in 0..2 {
                // First plane the centre reaches within this step.
                let tf = (step.1 < 0.0).then(|| (half - p.1) / step.1);
                let tw = wall.and_then(|w| (step.0 > 0.0).then(|| (w - half - p.0) / step.0));
                let (t, floor) = match (tf, tw) {
                    (Some(a), Some(b)) if a.min(b) < 1.0 => (a.min(b), a <= b),
                    (Some(a), None) if a < 1.0 => (a, true),
                    (None, Some(b)) if b < 1.0 => (b, false),
                    _ => {
                        p = (p.0 + step.0, p.1 + step.1);
                        break;
                    }
                };
                p = (p.0 + step.0 * t, p.1 + step.1 * t);
                let mut r = if floor { (v.0, -v.1) } else { (-v.0, v.1) };
                for c in [&mut r.0, &mut r.1] {
                    if c.abs() < 0.1 {
                        *c = 0.0;
                    }
                }
                r = (r.0 * e, r.1 * e);
                if (r.0 * r.0 + r.1 * r.1).sqrt() < 30.0 {
                    v = (0.0, 0.0);
                    rest = floor;
                    break;
                }
                v = r;
                if !floor {
                    break;
                }
                step = (r.0 * (1.0 - t) * dt, r.1 * (1.0 - t) * dt);
            }
        }
        out.push([p.0, p.1, v.0, v.1]);
    }
    out
}

/// Throws a grenade of `id` as the spec's level-view throw (3: speed 600 in/s
/// 10 degrees up) from `from` (inches) and compares it, tick by tick, with
/// `spec_flight`; returns the per-tick (x, y, vx, vy) in inches.
fn bounce_run(id: &str, from: (f32, f32), wall: Option<f32>, ticks: usize, exact: usize) -> Vec<[f32; 4]> {
    let mut sim = sim();
    let (s, c) = 10f32.to_radians().sin_cos();
    let v0 = (600.0 * c, 600.0 * s);
    let at = Vec3::new(from.0 * UNIT, from.1 * UNIT, 0.0);
    let (_, g) = place(&mut sim, id, at, Vec3::new(v0.0, v0.1, 0.0) * UNIT);
    // Keep it from going off during the record.
    sim.app.world_mut().get_mut::<Projectile>(g).unwrap().fuse = 100.0;
    let mut got = vec![];
    for _ in 0..ticks {
        sim.ticks(1);
        let p = projectiles(&mut sim).into_iter().find(|x| x.0 == g).unwrap();
        got.push([p.2.x / UNIT, p.2.y / UNIT, p.1.velocity.x / UNIT, p.1.velocity.y / UNIT]);
    }
    let want = spec_flight(from, v0, wall, ticks);
    // Tick by tick, positions within 0.3 in (our box stops 1 mm, 0.04 in,
    // short of what it touches) and velocities within 0.05 in/s, for the
    // first `exact` ticks. A grazing contact (a last hop of a few inches
    // a second) can land a tick apart from there on, because of that
    // 0.04 in, so the rest is compared as bounces below.
    for (i, (a, b)) in got.iter().zip(&want).enumerate().take(exact) {
        assert!(
            (a[0] - b[0]).abs() < 0.3 && (a[1] - b[1]).abs() < 0.3 && (a[2] - b[2]).abs() < 0.05 && (a[3] - b[3]).abs() < 0.05,
            "{id} tick {i}: ours {a:?}, spec {b:?}"
        );
    }
    got
}

/// Bounce apex heights (centre y over the floor-resting centre, 2 in) of a
/// run, the tick each floor contact happened on, and the tick it stopped.
fn bounces(run: &[[f32; 4]]) -> (Vec<f32>, Vec<usize>, Option<usize>) {
    let (mut apex, mut contacts) = (vec![], vec![]);
    for i in 1..run.len() {
        if run[i][3] > 0.0 && run[i - 1][3] <= 0.0 && run[i][1] < 6.0 {
            contacts.push(i);
        }
        if run[i][3] < 0.0 && run[i - 1][3] >= 0.0 && i > 1 {
            apex.push(run[i - 1][1] - 2.0);
        }
    }
    let stop = (1..run.len()).find(|&i| run[i][2] == 0.0 && run[i][3] == 0.0);
    (apex, contacts, stop)
}

#[test]
fn css_grenades_bounce_as_the_spec_says_on_floor_and_wall() {
    // Expected numbers, by hand from the spec (inches, seconds). The
    // level-view throw (section 3) is 600 in/s 10 degrees up: vx = 600 cos
    // 10 = 590.88, vy = 600 sin 10 = 104.19. Released with the box centre
    // 20 in up; the floor holds the centre at 2 in. Gravity 0.4 x 800 = 320.
    //  - apex of the throw: 18 + 104.19^2 / 640 = 34.96 over the resting
    //    centre height (ours 34.94).
    //  - first floor contact: vy_in = sqrt(104.19^2 + 2*320*18) = 150.1 down,
    //    rebound 0.45 x 150.1 = 67.5 up (the tick's gravity trims it to 66.8
    //    up), apex 67.5^2 / 640 = 7.1 (the tick-sampled parabola gives 7.4);
    //    vx 590.88 -> 265.90 (= 0.45 x 590.88).
    //  - second contact: falls 7.4 -> vy_in about 66.8, rebound 0.45 x 66.8
    //    = 30.1 up (just over the 30 in/s stop), vx 265.90 -> 119.65 (apex
    //    about 1.9).
    //  - third contact: vx 119.65 and vy about 35 in: 0.45 x hypot = 57 in/s
    //    is over 30, so it hops once more (vx 53.85, vy 15.7, apex about
    //    1); the contact after it has 0.45 x hypot(53.85, 20) = 26 in/s,
    //    under 30: it stops there, with no sliding (no ground friction).
    //  - it rests about 1.6 s (105 ticks) after the throw; x then
    //    stays put.
    // `spec_flight` applies exactly the spec's per-tick rules and gives
    // those numbers (the contact ticks 52, 81, 96 and the stop on 105); the
    // test requires ours to follow it tick by tick up to the third contact
    // and to agree to a tick on the contact and stop times after it.
    for id in [HEGRENADE, FLASHBANG, SMOKEGRENADE] {
        // Floor: thrown 20 in up (centre) from x = -30 m, nothing in the way.
        let from = (-30.0 / UNIT, 20.0);
        let run = bounce_run(id, from, None, 200, 94);
        let (apex, contacts, stop) = bounces(&run);
        let want = bounces(&spec_flight(from, (590.88, 104.19), None, 200));
        assert_eq!(contacts.len(), want.1.len(), "{id}: floor contacts {contacts:?} vs spec {:?}", want.1);
        for (a, b) in contacts.iter().zip(&want.1) {
            assert!(a.abs_diff(*b) <= 1, "{id}: contact ticks {contacts:?} vs spec {:?}", want.1);
        }
        assert_eq!(contacts[..2], [52, 81], "{id}: contact ticks");
        let (stop, spec_stop) = (stop.expect("{id} stops: no rolling"), want.2.unwrap());
        assert!(stop.abs_diff(spec_stop) <= 1, "{id}: stops on tick {stop}, spec {spec_stop}");
        // Apexes over the resting height: the throw's 34.96, then 7.4, 1.9.
        for (a, b) in apex.iter().zip(&want.0) {
            assert!((a - b).abs() < 0.5, "{id}: apex {apex:?} vs spec {:?}", want.0);
        }
        assert!((apex[0] - 34.96).abs() < 0.1 && (apex[1] - 7.4).abs() < 0.2, "{apex:?}");
        // Once stopped it stays put: no sliding or rolling.
        assert!((run[199][0] - run[stop][0]).abs() < 1e-3 && run[199][1] == run[stop][1]);

        // Wall: the east wall's inner face is at x = 40 m; thrown at it from
        // 4 m away. The wall contact sends vx to -0.45 x 590.88 = -265.9
        // and vy to 0.45 x (vy - 4.8) (the tick's gravity, then the same
        // scale; no second sweep).
        let wall = 40.0 / UNIT;
        let from = (wall - 4.0 / UNIT, 20.0);
        let run = bounce_run(id, from, Some(wall), 120, 120);
        let i = (1..run.len()).find(|&i| run[i][2] < 0.0).expect("hits the wall");
        assert!(
            (run[i][2] + 0.45 * 590.88).abs() < 0.05,
            "{id}: off the wall at {} in/s, spec {}",
            run[i][2],
            -0.45 * 590.88
        );
        assert!((run[i][3] - 0.45 * (run[i - 1][3] - 4.8)).abs() < 0.05, "vy scaled by 0.45 too");
        assert!(bounces(&run).2.is_some(), "{id}: falls, bounces and stops");
    }
}
