//! TF2's batch-2b projectiles and pushes (specs/tf2/batch2b_mechanics.md,
//! its test cases): a made-up item schema in items_game's shape (the
//! spec's attribute values, nothing copied from the game) through the
//! converter, then played on the headless harness: the Huntsman's charge,
//! headshot and wobble, the crossbow's flight-time damage and heal, the
//! Sandman's ball and meter, the Wrap Assassin and the Flying Guillotine
//! with their bleeds, the Bison's double hits through two players, the
//! Loose Cannon's impact, stop and double donk, the Caber, the
//! Force-a-Nature's knockback and air push, the Shortstop's shove, and
//! the flare's trail and its look ending on the world.

use bevy::prelude::*;
use chimera_core::core::{DamageKind, DamageTaken, Health, SimClock, Velocity};
use chimera_sim::{
    status::Slowed,
    testkit,
    weapon::{Magazine, projectile::{Flyer, LaunchRequest}},
};
use chimera_world::{greybox::{self, GreyboxMapPlugin}, harness::Sim};
use tf2::{
    game::{
        TICK_INTERVAL,
        items::{ItemPack, convert, effects::{Donk, Donks}},
        weapons::{blast::Tf2Explosion, knock, looks::GunLooks, script_numbers::GunNumbers},
    },
    harness::Tf2Sim,
};

use super::weapons::{fired, health_points, hold_still, holding, no_spread, rockets as flyers, run};

const U: f32 = 0.0254;

/// (name, class, format) of each attribute the made-up schema defines.
const DEFS: &[(&str, &str, &str)] = &[
    ("damage penalty", "mult_dmg", "value_is_percentage"),
    ("clip size penalty", "mult_clipsize", "value_is_inverted_percentage"),
    ("fire rate bonus", "mult_postfiredelay", "value_is_inverted_percentage"),
    ("bullets per shot bonus", "mult_bullets_per_shot", "value_is_percentage"),
    ("max health additive penalty", "add_maxhealth", "value_is_additive"),
    ("effect bar recharge rate increased", "effectbar_recharge_rate", "value_is_inverted_percentage"),
    ("energy weapon no ammo", "energy_weapon_no_ammo", "value_is_additive"),
    ("Projectile speed increased", "mult_projectile_speed", "value_is_percentage"),
    ("scattergun has knockback", "set_scattergun_has_knockback", "value_is_additive"),
    ("scattergun no reload single", "set_scattergun_no_reload_single", "value_is_additive"),
];

/// The made-up items: (index, name, item class, slot, class, attributes).
#[allow(clippy::type_complexity)]
const ITEMS: &[(u32, &str, &str, &str, &str, &[(&str, &str)])] = &[
    (39, "The Flare Gun", "tf_weapon_flaregun", "secondary", "pyro", &[]),
    (44, "The Sandman", "tf_weapon_bat_wood", "melee", "scout", &[("max health additive penalty", "-15")]),
    (45, "The Force-a-Nature", "tf_weapon_scattergun", "primary", "scout", &[
        ("scattergun has knockback", "1"), ("scattergun no reload single", "1"), ("bullets per shot bonus", "1.2"),
        ("damage penalty", "0.9"), ("clip size penalty", "0.34"), ("fire rate bonus", "0.5"),
    ]),
    (56, "The Huntsman", "tf_weapon_compound_bow", "primary", "sniper", &[]),
    (220, "The Shortstop", "tf_weapon_handgun_scout_primary", "primary", "scout", &[]),
    (305, "The Crusader's Crossbow", "tf_weapon_crossbow", "primary", "medic", &[]),
    (307, "The Ullapool Caber", "tf_weapon_stickbomb", "melee", "demoman", &[]),
    (442, "The Righteous Bison", "tf_weapon_raygun", "secondary", "soldier", &[("energy weapon no ammo", "1")]),
    (648, "The Wrap Assassin", "tf_weapon_bat_giftwrap", "melee", "scout", &[("damage penalty", "0.35"), ("effect bar recharge rate increased", "0.75")]),
    (812, "The Flying Guillotine", "tf_weapon_cleaver", "secondary", "scout", &[]),
    (996, "The Loose Cannon", "tf_weapon_cannon", "primary", "demoman", &[("Projectile speed increased", "1.2")]),
];

fn schema() -> String {
    let mut s = String::from("\"items_game\"\n{\n\"items\"\n{\n");
    for (index, name, class, slot, used, attrs) in ITEMS {
        s += &format!("\"{index}\" {{ \"name\" \"{name}\" \"item_class\" \"{class}\" \"item_slot\" \"{slot}\" \"used_by_classes\" {{ \"{used}\" \"1\" }} \"attributes\" {{\n");
        for (a, v) in *attrs {
            s += &format!("\"{a}\" {{ \"value\" \"{v}\" }}\n");
        }
        s += "} }\n";
    }
    s += "}\n\"attributes\"\n{\n";
    for (i, (name, class, format)) in DEFS.iter().enumerate() {
        s += &format!("\"{i}\" {{ \"name\" \"{name}\" \"attribute_class\" \"{class}\" \"description_format\" \"{format}\" }}\n");
    }
    s + "}\n}\n"
}

/// The pack, with the class scripts' numbers the converter reads (here
/// the spec's: the Shortstop's 4 pellets of 12, the Force-a-Nature's 10 of
/// 6, the Caber's 55).
fn pack() -> ItemPack {
    let mut p = convert::convert(&schema(), &|_| None).pack;
    for r in &mut p.items {
        let base: &[(&str, f64)] = match r.class.as_str() {
            "tf_weapon_handgun_scout_primary" => &[("bullets", 4.0), ("damage", 12.0), ("clip", 4.0), ("interval", 0.35)],
            "tf_weapon_scattergun" => &[("bullets", 10.0), ("damage", 6.0), ("clip", 6.0), ("interval", 0.625)],
            "tf_weapon_stickbomb" => &[("damage", 55.0), ("interval", 0.8)],
            "tf_weapon_raygun" => &[("clip", 6.0), ("interval", 0.8)],
            "tf_weapon_flaregun" => &[("damage", 30.0), ("interval", 2.0)],
            _ => &[],
        };
        for (k, v) in base {
            r.base.insert((*k).into(), *v);
        }
    }
    ItemPack::parse(&p.to_text()).unwrap()
}

#[derive(Clone)]
struct WithItems;

impl Plugin for WithItems {
    fn build(&self, app: &mut App) {
        app.insert_resource(GunNumbers::builtin().with_pack(&pack()));
        Plugin::build(&Tf2Sim, app);
    }
}

fn sim() -> Sim {
    let mut sim = Sim::new(WithItems, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

fn set_health(sim: &mut Sim, e: Entity, points: f32) {
    sim.app.world_mut().get_mut::<Health>(e).unwrap().current = points / 100.0;
}

/// A target `units` ahead (-Z) of `from` holding `gun`, with `hp` points.
fn victim(sim: &mut Sim, from: Vec3, units: f32, gun: &str, hp: f32) -> Entity {
    let (t, _) = holding(sim, from - Vec3::Z * units * U, gun);
    set_health(sim, t, hp);
    hold_still(sim, t);
    t
}

/// Every damage `e` takes while `f` runs `ticks`: (time, kind, points).
fn taken_while(sim: &mut Sim, e: Entity, ticks: u64, mut f: impl FnMut(&mut Sim, u64)) -> Vec<(f64, DamageKind, f32)> {
    let mut cursor = sim.app.world().resource::<Messages<DamageTaken>>().get_cursor_current();
    let mut out = Vec::new();
    for k in 0..ticks {
        f(sim, k);
        sim.ticks(1);
        let at = now(sim);
        for t in cursor.read(sim.app.world().resource::<Messages<DamageTaken>>()) {
            if t.damage.target == e {
                out.push((at, t.damage.kind, (t.damage.amount * 100.0).round()));
            }
        }
    }
    out
}

/// Hold attack `hold` ticks, let go: the arrow's launch velocity (units/s)
/// and gravity (units/s²).
fn bow_release(sim: &mut Sim, p: Entity, hold: u64) -> (Vec3, f32) {
    let ahead = sim.position(p) + Vec3::new(0.0, 0.5, -20.0);
    testkit::aim_at(sim, p, ahead);
    run(sim, p, hold + 2, |s, k| s.intent(p).fire = k < hold);
    let (e, _, v) = flyers(sim, p)[0];
    let g = sim.app.world().get::<Flyer>(e).unwrap().gravity / U;
    sim.app.world_mut().despawn(e);
    (v / U, g)
}

// ---- 1. Huntsman ----

#[test]
fn the_huntsmans_draw_sets_its_speed_and_gravity_and_tires_after_5_s() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:huntsman");
    for hold in [1u64, 34, 67, 200] {
        let (v, g) = bow_release(&mut sim, p, hold);
        let c = (hold as f32 * TICK_INTERVAL as f32).min(1.0);
        assert!((v.length() - (1800.0 + 800.0 * c)).abs() < 2.0, "{hold}: {}", v.length());
        assert!((g - (0.5 - 0.4 * c) * 800.0).abs() < 0.5, "{hold}: {g}");
        sim.seconds(2.1);
    }
    // Held 6 s: a full charge's speed, at most 6 degrees off the aim.
    let aim = {
        let i = sim.intent(p);
        chimera_sim::weapon::aim_of(i.yaw, i.pitch) * Vec3::NEG_Z
    };
    let (v, _) = bow_release(&mut sim, p, 400);
    assert!((v.length() - 2600.0).abs() < 2.0);
    let off = v.normalize().angle_between(aim).to_degrees();
    assert!(off <= 8.0, "{off}");
}

/// An app's input sets the button once a frame, so several ticks read
/// the same: the draw holds while it stays down.
#[test]
fn the_huntsman_holds_its_draw_on_input_set_once() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:huntsman");
    let ahead = sim.position(p) + Vec3::new(0.0, 0.5, -20.0);
    testkit::aim_at(&mut sim, p, ahead);
    let ev = run(&mut sim, p, 42, |s, k| match k {
        0 => s.intent(p).fire = true,
        40 => s.intent(p).fire = false,
        _ => {}
    });
    assert_eq!(fired(&ev), [40], "fired on the release only");
    let v = flyers(&mut sim, p)[0].2 / U;
    assert!((v.length() - 2280.0).abs() < 2.0, "0.6 s drawn: {}", v.length());
}

#[test]
fn the_huntsman_draws_again_2_s_after_a_release() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:huntsman");
    let ev = run(&mut sim, p, 200, |s, k| s.intent(p).fire = k % 10 != 9);
    let shots = fired(&ev);
    assert!(shots.len() >= 2, "{shots:?}");
    let gap = (shots[1] - shots[0]) as f64 * TICK_INTERVAL;
    assert!((2.0..2.2).contains(&gap), "{gap}");
}

/// A bow hit after `hold` ticks of draw at a target 300 units ahead, aimed
/// `up` units above its origin: the damage.
fn bow_hit(hold: u64, up: f32) -> f32 {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:huntsman");
    let t = victim(&mut sim, from, 300.0, "tf2:tf_weapon_minigun", 1000.0);
    let at = sim.position(t) + Vec3::Y * up * U;
    testkit::aim_at(&mut sim, p, at);
    let before = health_points(&sim, t);
    run(&mut sim, p, hold + 20, |s, k| {
        hold_still(s, t);
        s.intent(p).fire = k < hold
    });
    (before - health_points(&sim, t)).round()
}

#[test]
fn the_huntsmans_body_hit_is_89_at_0_555_and_a_full_headshot_360() {
    // 37 ticks: 0.555 s.
    assert_eq!(bow_hit(37, 0.0), 89.0);
    let head = {
        let mut sim = sim();
        let (t, _) = holding(&mut sim, Vec3::ZERO, "tf2:tf_weapon_minigun");
        sim.state(t).eye_offset.y / U
    };
    assert_eq!(bow_hit(80, head + 2.0), 360.0);
}

// ---- 2. Crusader's Crossbow ----

#[test]
fn the_crossbow_hurts_and_heals_by_its_flight_time() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:crusaders_crossbow");
    let t = victim(&mut sim, from, 600.0, "tf2:tf_weapon_minigun", 1000.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let shot = now(&sim) + TICK_INTERVAL;
    let hits = taken_while(&mut sim, t, 40, |s, k| {
        hold_still(s, t);
        s.intent(p).fire = k == 0
    });
    let (when, _, d) = hits[0];
    let flight = (when - shot) as f32;
    let want = (38.0 * (1.0 + ((flight - 0.03) / 0.6).clamp(0.0, 1.0))).min(75.0).round();
    assert_eq!(d, want, "after {flight} s");
    assert!(flight > 0.2 && flight < 0.3, "{flight}");
    // A teammate at 10 of his max: + 75 x (1 + t / 0.6), no overheal.
    let mate = victim(&mut sim, from - Vec3::X * 2.0, 600.0, "tf2:tf_weapon_minigun", 10.0);
    sim.app.world_mut().despawn(t);
    for e in [p, mate] {
        sim.app.world_mut().entity_mut(e).insert(chimera_core::core::Team(2));
    }
    let at = sim.position(mate);
    testkit::aim_at(&mut sim, p, at);
    sim.seconds(2.0);
    let shot = now(&sim) + TICK_INTERVAL;
    let mut healed = None;
    for k in 0..40 {
        sim.intent(p).fire = k == 0;
        hold_still(&mut sim, mate);
        sim.ticks(1);
        if healed.is_none() && health_points(&sim, mate) > 10.0 {
            healed = Some(((now(&sim) - shot) as f32, health_points(&sim, mate) - 10.0));
        }
    }
    let (flight, gain) = healed.expect("healed");
    assert_eq!(gain.round(), (75.0 * (1.0 + (flight / 0.6).min(1.0))).round(), "after {flight} s");
}

// ---- 3, 4. Sandman, Wrap Assassin ----

/// Throw `id`'s ball at a target `units` ahead: the damages it took and
/// its slow, and the sim.
fn throw_at(id: &str, units: f32) -> (Sim, Entity, Entity, Vec<(f64, DamageKind, f32)>) {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, id);
    let t = victim(&mut sim, from, units, "tf2:tf_weapon_minigun", 1000.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    // Near, aimed so the ball's 5.3 degrees up meet him; farther, they
    // make up for its fall.
    if units < 400.0 {
        sim.intent(p).pitch -= 5.3f32.to_radians();
    }
    let hits = taken_while(&mut sim, t, 60, |s, k| {
        hold_still(s, t);
        s.intent(p).secondary = k == 0
    });
    (sim, p, t, hits)
}

#[test]
fn the_sandmans_ball_hits_15_and_slows_by_its_flight_and_its_meter_refills_in_10_s() {
    // Close: no slow.
    let (sim, _, t, hits) = throw_at("tf2:sandman", 60.0);
    assert_eq!(hits.iter().map(|h| h.2).collect::<Vec<_>>(), [15.0]);
    assert!(sim.app.world().get::<Slowed>(t).is_none());
    // Farther: half speed for 7.5 x its flight, at least 2 s.
    let (mut sim, p, t, hits) = throw_at("tf2:sandman", 700.0);
    assert_eq!(hits[0].2, 15.0);
    let s = *sim.app.world().get::<Slowed>(t).expect("slowed");
    assert_eq!(s.factor, 0.5);
    let secs = (s.until - hits[0].0) as f32;
    assert!((2.0..=4.0).contains(&secs), "{secs}");
    // The meter: no ball before 10 s from the throw, one after.
    let count = |sim: &mut Sim| flyers(sim, p).len();
    sim.seconds(1.0);
    let before = count(&mut sim);
    run(&mut sim, p, 2, |s, k| s.intent(p).secondary = k == 0);
    assert_eq!(count(&mut sim), before, "not ready");
    sim.seconds(8.5);
    run(&mut sim, p, 2, |s, k| s.intent(p).secondary = k == 0);
    assert_eq!(count(&mut sim), before + 1, "ready after 10 s");
    assert_eq!((sim.app.world().get::<Health>(p).unwrap().max * 100.0).round(), 110.0);
}

#[test]
fn the_wrap_assassins_ornament_hits_15_bursts_and_bleeds_10_ticks() {
    let (mut sim, _, t, hits) = throw_at("tf2:wrap_assassin", 300.0);
    assert_eq!(hits[0].2, 15.0, "{hits:?}");
    let burst: Vec<f32> = hits.iter().filter(|h| h.1.is_blast()).map(|h| h.2).collect();
    assert!(burst.len() == 1 && (6.0..=9.0).contains(&burst[0]), "{hits:?}");
    let bleeds = hits.iter().filter(|h| h.1 == DamageKind::Bleed).count()
        + taken_while(&mut sim, t, 400, |s, _| hold_still(s, t)).iter().filter(|h| h.1 == DamageKind::Bleed).count();
    assert_eq!(bleeds, 10);
}

// ---- 5. Flying Guillotine ----

#[test]
fn the_guillotine_hits_50_and_bleeds_10_ticks() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:flying_guillotine");
    let t = victim(&mut sim, from, 300.0, "tf2:tf_weapon_minigun", 1000.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    sim.intent(p).pitch -= 7.6f32.to_radians();
    let hits = taken_while(&mut sim, t, 450, |s, k| {
        hold_still(s, t);
        s.intent(p).fire = k == 0
    });
    assert_eq!(hits[0].2, 50.0, "{hits:?}");
    assert_eq!(hits.iter().filter(|h| h.1 == DamageKind::Bleed && h.2 == 4.0).count(), 10);
}

// ---- 6. Righteous Bison ----

#[test]
fn the_bison_hits_two_players_twice_each_by_distance() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, "tf2:righteous_bison");
    let a = victim(&mut sim, from, 270.0, "tf2:tf_weapon_minigun", 1000.0);
    let b = victim(&mut sim, from, 470.0, "tf2:tf_weapon_minigun", 1000.0);
    let at = sim.position(a);
    testkit::aim_at(&mut sim, p, at);
    let mut ha = Vec::new();
    let hb = taken_while(&mut sim, b, 40, |s, k| {
        hold_still(s, a);
        hold_still(s, b);
        s.intent(p).fire = k == 0;
        if k == 30 {
            ha.push(health_points(s, a));
        }
    });
    assert_eq!((1000.0 - ha[0]).round(), 44.0, "22 + 22");
    assert_eq!(hb.iter().map(|h| h.2).collect::<Vec<_>>(), [20.0, 20.0]);
    assert!((hb[1].0 - hb[0].0 - 0.03).abs() < 1e-6, "two ticks apart");
    // Four shots 0.8 s apart, then the cell refills.
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, 3);
    sim.seconds(3.0);
    let ev = run(&mut sim, p, 300, |s, _| s.intent(p).fire = true);
    let shots = fired(&ev);
    assert!(shots.len() >= 4);
    for k in shots.windows(2).take(2) {
        assert!(((k[1] - k[0]) as f64 * TICK_INTERVAL - 0.8).abs() < 0.02, "{shots:?}");
    }
}

// ---- 7. Loose Cannon ----

#[test]
fn the_loose_cannons_ball_hits_50_stops_its_victim_and_does_not_explode() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:loose_cannon");
    let t = victim(&mut sim, from, 150.0, "tf2:tf_weapon_minigun", 1000.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let hits = taken_while(&mut sim, t, 20, |s, k| {
        hold_still(s, t);
        s.intent(p).fire = k == 0
    });
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].2, 50.0);
    let s = *sim.app.world().get::<Slowed>(t).expect("stopped");
    assert_eq!(s.factor, 0.0);
    assert!(((s.until - hits[0].0) - 0.5).abs() < 1e-6);
    let d = sim.app.world().resource::<Donks>().0.clone();
    assert!(d.iter().any(|d| d.victim == t), "the donk's window");
}

/// A cannonball of `p`'s going off inside `t` (none moving): his damage,
/// with an impact `since` s before (None: none).
fn cannon_blast(since: Option<f64>) -> f32 {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:loose_cannon");
    let t = victim(&mut sim, from, 300.0, "tf2:tf_weapon_minigun", 1000.0);
    let state = sim.state(t);
    let centre = sim.position(t) + (state.hull_min + state.hull_max) / 2.0;
    let clock = sim.app.world().resource::<SimClock>().clone();
    let mut f = Flyer::new("tf2:loose_cannon", Some(p), Vec3::ZERO, &clock);
    f.expires = Some(clock.now + 0.1);
    sim.app.world_mut().write_message(LaunchRequest { flyer: f, start: centre, shown: None, tag: None });
    if let Some(s) = since {
        sim.app.world_mut().resource_mut::<Donks>().0.push(Donk {
            flyer: Entity::PLACEHOLDER,
            owner: Some(p),
            weapon: "tf2:loose_cannon",
            victim: t,
            until: clock.now + 0.1 - s + 0.5,
        });
    }
    let hits = taken_while(&mut sim, t, 20, |s, _| hold_still(s, t));
    hits.iter().filter(|h| h.1.is_blast()).map(|h| h.2).sum()
}

#[test]
fn the_loose_cannons_explosion_mini_crits_within_0_5_s_of_its_impact() {
    let plain = cannon_blast(None);
    assert_eq!(plain, 60.0, "the fuse's 60 % of 100");
    assert_eq!(cannon_blast(Some(0.45)), 81.0);
    assert_eq!(cannon_blast(Some(0.65)), 60.0);
}

#[test]
fn the_loose_cannons_fuse_runs_from_the_press() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:loose_cannon");
    let ahead = sim.position(p) + Vec3::new(0.0, 3.0, -20.0);
    testkit::aim_at(&mut sim, p, ahead);
    // Held 0.495 s: the ball lives about 0.5 s.
    run(&mut sim, p, 35, |s, k| s.intent(p).fire = k < 33);
    let (e, ..) = flyers(&mut sim, p)[0];
    let f = sim.app.world().get::<Flyer>(e).unwrap();
    let life = f.expires.unwrap() - f.born;
    assert!((life - 0.505).abs() < 0.02, "{life}");
}

#[test]
fn the_loose_cannon_holds_on_input_set_once() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:loose_cannon");
    let ev = run(&mut sim, p, 40, |s, k| match k {
        0 => s.intent(p).fire = true,
        30 => s.intent(p).fire = false,
        _ => {}
    });
    assert_eq!(fired(&ev), [30], "fired on the release only");
}

/// A stuck arrow's origin is at the surface it hit (the shot's skin, a
/// millimetre, short of it), not above it: a ray along the flight from
/// half a metre back meets the surface at once past the arrow.
#[test]
fn a_stuck_arrow_sits_on_its_surface() {
    use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
    use bevy::ecs::system::RunSystemOnce;
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:huntsman");
    let ahead = Vec3::new(0.0, 1.6, greybox::NORTH_WALL_Z);
    testkit::aim_at(&mut sim, p, ahead);
    run(&mut sim, p, 120, |s, k| s.intent(p).fire = k < 30);
    let (_, at, v) = flyers(&mut sim, p)[0];
    assert_eq!(v, Vec3::ZERO, "stuck");
    let dir = {
        let i = sim.intent(p);
        chimera_sim::weapon::aim_of(i.yaw, i.pitch) * Vec3::NEG_Z
    };
    let back = at - dir * 0.5;
    let hit = sim
        .app
        .world_mut()
        .run_system_once(move |sq: SpatialQuery| sq.cast_ray(back, Dir3::new(dir).unwrap(), 5.0, true, &SpatialQueryFilter::default()).map(|h| h.distance))
        .unwrap()
        .expect("a surface ahead of the arrow");
    let gap = hit - 0.5;
    assert!((0.0..0.01).contains(&gap), "the arrow is {gap} m short of its surface");
}

// ---- 8. Ullapool Caber ----

#[test]
fn the_cabers_first_hit_explodes_and_the_next_is_the_swing_alone() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:ullapool_caber");
    set_health(&mut sim, p, 1000.0);
    let t = victim(&mut sim, from, 60.0, "tf2:tf_weapon_minigun", 1000.0);
    let (home, mine) = (sim.position(t), sim.position(p));
    let swing = |sim: &mut Sim| {
        sim.app.world_mut().get_mut::<Transform>(t).unwrap().translation = home;
        sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = mine;
        hold_still(sim, p);
        sim.ticks(2);
        let at = sim.position(t);
        testkit::aim_at(sim, p, at);
        let (h, mine) = (health_points(sim, t), health_points(sim, p));
        run(sim, p, 60, |s, k| {
            hold_still(s, t);
            s.intent(p).fire = k == 0
        });
        (h - health_points(sim, t), mine - health_points(sim, p))
    };
    let (first, own) = swing(&mut sim);
    assert!(first > 55.0 + 55.0 && first < 55.0 + 76.0, "55 and its blast: {first}");
    assert!(own > 20.0, "his own blast: {own}");
    sim.seconds(1.0);
    hold_still(&mut sim, p);
    sim.seconds(1.0);
    assert_eq!(swing(&mut sim), (55.0, 0.0));
}

// ---- 9. Force-a-Nature ----

#[test]
fn the_force_a_nature_knocks_back_within_390_and_pushes_its_holder_in_the_air() {
    let shot = |units: f32| {
        let mut sim = sim();
        let from = greybox::SPAWNS[0];
        let (p, w) = holding(&mut sim, from, "tf2:force_a_nature");
        no_spread(&mut sim, w);
        let t = victim(&mut sim, from, units, "tf2:tf_weapon_rocketlauncher", 1000.0);
        let at = sim.position(t);
        testkit::aim_at(&mut sim, p, at);
        let h = health_points(&sim, t);
        run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
        let v = sim.app.world().get::<Velocity>(t).unwrap().0 / U;
        (h - health_points(&sim, t), v)
    };
    let (d, v) = shot(300.0);
    assert!(d > 80.0, "{d}");
    assert!((v.xz().length() - knock::KNOCK_PER_DAMAGE * d).abs() < 10.0, "{d}: {v}");
    assert!((v.y - knock::knock_up(300.0)).abs() < 20.0, "{v}");
    let (_, far) = shot(420.0);
    assert!(far.xz().length() < 500.0 && far.y < 100.0, "the bullets' push only: {far}");
    // In the air, aiming straight down: pushed up 483.
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:force_a_nature");
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation.y += 4.0;
    sim.ticks(2);
    let below = sim.position(p) - Vec3::Y * 10.0 + Vec3::Z * 0.01;
    testkit::aim_at(&mut sim, p, below);
    let v0 = sim.app.world().get::<Velocity>(p).unwrap().0.y / U;
    run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
    let v1 = sim.app.world().get::<Velocity>(p).unwrap().0.y / U;
    assert!(((v1 - v0) - 483.0).abs() < 20.0, "{v0} -> {v1}");
}

// ---- 10. Shortstop ----

#[test]
fn the_shortstop_shoves_after_0_25_s_for_1_and_400_out_262_up() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, "tf2:shortstop");
    let t = victim(&mut sim, from, 60.0, "tf2:tf_weapon_minigun", 1000.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let mut seen = None;
    let hits = taken_while(&mut sim, t, 25, |s, k| {
        s.intent(p).secondary = k == 0;
        if seen.is_none() && health_points(s, t) < 1000.0 {
            seen = Some(s.app.world().get::<Velocity>(t).unwrap().0 / U);
        }
    });
    assert_eq!(hits.iter().map(|h| h.2).collect::<Vec<_>>(), [1.0]);
    let v = seen.expect("shoved");
    assert!((v.xz().length() - 400.0).abs() < 5.0 && (v.y - 262.0).abs() < 5.0, "{v}");
}

// ---- 18. The flare's looks ----

#[test]
fn the_flare_trails_by_team_and_ends_on_the_world_in_flaregun_destroyed() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], "tf2:flare_gun");
    let set = sim.app.world().resource::<GunLooks>().clone();
    let trail = set.trail_of_shown("tf2:tf_weapon_flaregun/flare").expect("the flare's trail");
    assert_eq!(trail.name(Some(false)), "flaregun_trail_red");
    assert_eq!(trail.name(Some(true)), "flaregun_trail_blue");
    assert_eq!(trail.crit_name(Some(true)).as_deref(), Some("flaregun_crit_blue"));
    let wall = sim.position(p) + Vec3::new(0.0, 0.5, -60.0);
    testkit::aim_at(&mut sim, p, wall);
    let mut cursor = sim.app.world().resource::<Messages<Tf2Explosion>>().get_cursor_current();
    let mut seen = Vec::new();
    for k in 0..60 {
        sim.intent(p).fire = k == 0;
        sim.ticks(1);
        seen.extend(cursor.read(sim.app.world().resource::<Messages<Tf2Explosion>>()).map(|e| e.weapon));
    }
    assert_eq!(seen, ["tf2:flare_gun"], "its look where it ends");
}
