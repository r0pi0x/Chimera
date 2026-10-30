//! Every Combat Arms copy with a timing table (the gun game's pool and the
//! rest: knives, specials) fires,
//! reloads, draws and swings on its own view model's animations, from the
//! user's install (docs/plans/active/ca-weapon-timings.md): each weapon
//! with a `combat_arms:timing` table is built as the game builds it (its
//! registry entry: every hook) and must carry its model's numbers: a gun's
//! cycle `fire_0` / (1 + `FireAniRate`), reload `reload`, draw `select`,
//! post-fire `postfire` (none without one) and latency (3 to 2 without a
//! `prefire`, its whole length with one); a spinning gun's spin-up,
//! spin-down and loop, a round per loop key; a flame gun's loop, keys and
//! spacing; a knife's swings and hit keys. Prints the counts (copies whose template's cycle already was their
//! own, before; read, after; post-fires, latencies, spinning, flame and
//! knife copies, guns with a pre-fire of non-zero length; the read times)
//! and the first few that differ.
//!
//! Needs the V1 key (the weapons file) and, for the rifles', SMGs' and
//! snipers' view models, the V2 key in chimera.local.toml. Skips with a
//! note without the first; models that don't read are counted.
//! `--nocapture` for the counts.

use std::time::Instant;

use bevy::prelude::*;
use chimera_core::mount::config::LocalConfig;
use chimera_sim::weapon::{self, Magazine, ShellReload, Trigger, Weapon};
use chimera_world::harness::Sim;
use combat_arms::{
    game::{
        GAME,
        attachments::fire_animation_rate,
        data::CaData,
        gear::records::GearData,
        inventory::{CaItem, Knife},
        movement,
        special::CaFlame,
        weapons::{
            classes::CaSpin,
            timing::{self, AnimTimes, TIMING, ViewTimes},
        },
    },
    harness::CaSimWith,
};

#[derive(Default, Debug)]
struct Counts {
    pool: usize,
    timed: usize,
    unread: usize,
    no_fire: usize,
    own_before: usize,
    own_after: usize,
    guns: usize,
    postfire: usize,
    semi_postfire: usize,
    latency_2: usize,
    long_prefire: usize,
    spin: usize,
    flame: usize,
    knife: usize,
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn every_copy_times_on_its_own_view_model() {
    if LocalConfig::load().ok().and_then(|c| c.game_path(GAME)).is_none() {
        eprintln!("skipping: no Combat Arms install configured");
        return;
    }
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.weapons.as_ref()).is_none() {
        eprintln!("skipping: the install's weapons file isn't readable (the V1 key)");
        return;
    }
    let mut sim = Sim::with(CaSimWith(ca.clone()), |_| {});
    let owner = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    let tick = 1.0 / sim.tick_hz();
    let mut n = Counts::default();
    let (mut read_s, mut slowest) = (0.0f64, (0.0f64, String::new()));
    let mut wrong = Vec::new();
    let mut paths = std::collections::BTreeSet::new();
    for d in &ca.weapon_set().weapons {
        // The gun game's pool, and every other copy with a timing table
        // (knives, specials).
        if d.class.is_some() {
            n.pool += 1;
        }
        if !d.hooks.contains_key(TIMING) {
            continue;
        }
        n.timed += 1;
        let path = d.models.view.as_deref().unwrap();
        paths.insert(path.to_string());
        let key = combat_arms::game::model::normalize(path);
        let world = sim.app.world_mut();
        let t = match world.resource::<ViewTimes>().0.get(&key) {
            Some(t) => t.clone(),
            None => {
                let t0 = Instant::now();
                let t = timing::read(path);
                let took = t0.elapsed().as_secs_f64();
                read_s += took;
                if took > slowest.0 {
                    slowest = (took, path.to_string());
                }
                world.resource_mut::<ViewTimes>().0.insert(key, t.clone());
                t
            }
        };
        let Some(t) = t else {
            n.unread += 1;
            println!("  unread: {} ({path})", d.id);
            continue;
        };
        let Some(fire) = t.fire else {
            n.no_fire += 1;
            println!("  no fire_0: {} ({path})", d.id);
            continue;
        };
        // Built as the game builds it: every hook.
        let e = weapon::spawn_weapon(world, &d.id, owner).expect("registered");
        let problem = check(world, e, d, &t, fire, tick, &mut n);
        if let Some(p) = problem {
            wrong.push(format!("{}: {p}", d.id));
        } else {
            n.own_after += 1;
        }
        world.despawn(e);
    }
    let gear = GearData::built_in();
    let gear_doc = ca.gear().to_doc();
    println!(
        "pool {} (with a class); {} copies timed by their view model: {} on their own timing (before: {} by chance); \
         {} models unread, {} without fire_0; reading {} distinct models took {read_s:.2} s ({:.1} ms a model, slowest {:.1} ms {})",
        n.pool,
        n.timed,
        n.own_after,
        n.own_before,
        n.unread,
        n.no_fire,
        paths.len(),
        1000.0 * read_s / paths.len().max(1) as f64,
        1000.0 * slowest.0,
        slowest.1
    );
    println!(
        "guns {}: {} with a post-fire ({} semi-automatic), {} on latency 2 (no prefire), {} with a pre-fire of non-zero length; \
         spinning {}, flame {}, knives {}",
        n.guns, n.postfire, n.semi_postfire, n.latency_2, n.long_prefire, n.spin, n.flame, n.knife
    );
    println!(
        "gear records as a document: {} bytes (ours alone: {})",
        gear_doc.len(),
        gear.to_doc().len()
    );
    assert_eq!(GearData::parse(&gear_doc).unwrap(), *ca.gear(), "the install's gear records round-trip");
    for w in wrong.iter().take(20) {
        println!("  {w}");
    }
    assert!(n.timed > 0, "no copy has a timing table");
    assert!(wrong.is_empty(), "{} copies don't time on their models", wrong.len());
}

/// Whether weapon `e` (built from `d`) carries model `t`'s numbers; counts.
fn check(
    world: &World,
    e: Entity,
    d: &chimera_sim::weapon::data::WeaponData,
    t: &AnimTimes,
    fire: f32,
    tick: f64,
    n: &mut Counts,
) -> Option<String> {
    let item = world.get::<CaItem>(e);
    let rate = fire_animation_rate(item.map_or(0.0, |i| i.fire_rate), 0.0);
    let draw = world.get::<Weapon>(e).map(|w| w.draw_time);
    if t.select.is_some_and(|s| item.is_none_or(|i| i.select_rate == 0.0) && !draw.is_some_and(|dr| close(dr, s))) {
        return Some(format!("draw {draw:?} (model {:?})", t.select));
    }
    if let Some(k) = world.get::<Knife>(e) {
        n.knife += 1;
        let a = &k.attacks[0];
        if !close(a.length, fire) || (!t.fire_keys.is_empty() && a.hits != t.fire_keys) {
            return Some(format!("slash {} {:?} (model {fire} {:?})", a.length, a.hits, t.fire_keys));
        }
        if let (Some(stab), Some(alt)) = (k.attacks.get(1), t.alt_fire)
            && (!close(stab.length, alt) || (!t.alt_fire_keys.is_empty() && stab.hits != t.alt_fire_keys))
        {
            return Some(format!("stab {} {:?} (model {alt} {:?})", stab.length, stab.hits, t.alt_fire_keys));
        }
        return None;
    }
    let tr = world.get::<Trigger>(e)?;
    let keys = t.fire_keys.iter().filter(|&&k| k < fire - 1e-6).count();
    if let Some(s) = world.get::<CaSpin>(e) {
        n.spin += 1;
        let p = s.params;
        let cycle = fire / keys.max(1) as f32 / rate;
        println!(
            "  spinning: {} (spin-up {}, spin-down {}, loop {} with {keys} keys: {:.1} ms a round, reversible {})",
            d.id,
            p.prefire,
            p.postfire,
            p.loop_len,
            1000.0 * cycle,
            p.reversible
        );
        if !close(p.loop_len, fire) || !close(p.prefire, t.prefire.unwrap_or(0.0)) || !close(tr.cycle, cycle) || tr.latency != 1 {
            return Some(format!("spin {p:?}, cycle {}, latency {}", tr.cycle, tr.latency));
        }
        return None;
    }
    if let Some(f) = world.get::<CaFlame>(e) {
        n.flame += 1;
        let p = f.params;
        if !close(p.loop_len, fire) || !close(tr.cycle, p.interval / rate) || (keys > 0 && p.keys as usize != keys) {
            return Some(format!("flame {p:?}, cycle {}", tr.cycle));
        }
        return None;
    }
    n.guns += 1;
    let cycle = fire / rate;
    if d.trigger.as_ref().is_some_and(|tr| close(tr.cycle, cycle)) {
        n.own_before += 1;
    }
    let postfire = t.postfire.unwrap_or(0.0) / rate;
    if postfire > 0.0 {
        n.postfire += 1;
        if !tr.automatic {
            n.semi_postfire += 1;
        }
    }
    if tr.latency == 2 && d.trigger.as_ref().is_some_and(|tr| tr.latency == 3) {
        n.latency_2 += 1;
    }
    if t.prefire.is_some() {
        n.long_prefire += 1;
    }
    // A class that plays the pre-fire: an update sooner without one in
    // the model, its whole length with one (weapon_classes.md Corrections
    // (2026-10-30) 1: the non-spinning minigun copies' 1.33 s spin-up).
    let want_latency = match d.trigger.as_ref().map_or(0, |tr| tr.latency) {
        3 if !t.has_prefire => 2,
        3 => timing::prefire_latency(t.prefire.unwrap_or(0.0) / rate, tick),
        l => l,
    };
    let reload = world
        .get::<ShellReload>(e)
        .map(|s| s.insert)
        .or(world.get::<Magazine>(e).map(|m| m.reload_time));
    let ok = close(tr.cycle, cycle)
        && close(tr.postfire, postfire)
        && tr.latency == want_latency
        && t.reload.is_none_or(|r| reload == Some(r));
    (!ok).then(|| {
        format!(
            "cycle {} (model {cycle}), post-fire {} ({postfire}), latency {} ({want_latency}), reload {reload:?} ({:?})",
            tr.cycle, tr.postfire, tr.latency, t.reload
        )
    })
}
