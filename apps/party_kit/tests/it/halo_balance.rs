//! Halo's balance across games in the party (`halo_ce::game::cross_game`,
//! its data/party_balance.toml; the user's decision of 2026-11-15), on
//! the party's composed simulation with the other games' real gun data:
//!
//! - **Incoming**: another game's gun on a full Halo player counts 1.5
//!   times against the shield, then the body (133 effective hit points
//!   where any other character has 100; 200 with the switch off): each
//!   gun's body hit at 10 m from its data (`power::stats`, the games'
//!   own readers) through the engine's armour step (`weapon::HitArmor`,
//!   as a gun's hit is) and health, hits to kill counted, the switch on
//!   and off; a CS:S AK-47 by real fire kills one in 4 body hits.

use bevy::{ecs::system::RunSystemOnce, prelude::*};
use chimera_core::core::{BodyRegion, Damage, DamageKind, Health};
use chimera_sim::{
    minigame::power::{self, PowerCurve, PowerReaders},
    weapon::{HitArmor, data::WeaponCatalog},
};
use chimera_world::harness::Sim;
use halo_ce::game::{cross_game::CrossGame, vitality::Vitality, weapons};

use crate::{
    gun_game::{alive, line_up, soldier},
    halo_ladder::{party, set_rung},
};

/// The halo_ladder's rung of Halo's assault rifle.
const HALO_RUNG: usize = 1;

/// A gun's body hit at 10 m, every pellet landing, from its data and its
/// game's power reader (normalized: 1 = 100 hit points), and its head's
/// multiplier.
pub(crate) fn per_hit(sim: &Sim, id: &str) -> (f32, f32) {
    let w = sim.app.world();
    let data = w.resource::<WeaponCatalog>().get(id).unwrap_or_else(|| panic!("{id} in the catalogue"));
    let readers = w.get_resource::<PowerReaders>().cloned().unwrap_or_default();
    let s = power::stats(data, &readers, &PowerCurve::shipped(), 10.0);
    (s.damage * s.pellets as f32, s.head.unwrap_or(1.0))
}

/// One hit of `raw` (normalized) by gun `id` on `target`'s `region`, as a
/// gun's hit is dealt: the engine's armour step, then health (no
/// attacker: no kill credit, so the ladder stays as it is).
pub(crate) fn gun_hit(sim: &mut Sim, target: Entity, id: &'static str, raw: f32, region: BodyRegion) {
    sim.app
        .world_mut()
        .run_system_once(move |mut armor: HitArmor, mut out: MessageWriter<Damage>| {
            let mut d = Damage {
                target,
                attacker: None,
                amount: raw,
                point: Vec3::ZERO,
                dir: Vec3::NEG_Z,
                hitgroup: region.hitgroup(),
                kind: DamageKind::Bullet,
                weapon: Some(id),
                force: Vec3::ZERO,
            };
            armor.apply_region(&mut d, region, raw, Some(1.0), 0.0);
            out.write(d);
        })
        .unwrap();
    sim.ticks(1);
}

/// Hits of `raw` by `id` on `target`'s `region` until it dies (at most 60).
pub(crate) fn hits_to_kill(sim: &mut Sim, target: Entity, id: &'static str, raw: f32, region: BodyRegion) -> u32 {
    for n in 1..=60 {
        gun_hit(sim, target, id, raw, region);
        if !alive(sim, target) || sim.app.world().get::<Health>(target).is_some_and(|h| h.current <= 0.0) {
            return n;
        }
    }
    panic!("{id}: no kill in 60 hits");
}

/// The party (halo_ladder's), with the balance `cfg`: a player on the
/// first rung and one (the victim) on Halo's.
pub(crate) fn halo_victim(cfg: CrossGame) -> (Sim, Entity, Entity) {
    let mut sim = party();
    sim.app.insert_resource(cfg);
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    set_rung(&mut sim, b, HALO_RUNG);
    sim.seconds(1.2);
    assert_eq!(crate::gun_game::held(&sim, b), Some(weapons::ASSAULT_RIFLE));
    (sim, a, b)
}

/// `b` back in play, unprotected, on Halo's rung with a full shield (a
/// death with no attacker cost it a rung).
fn ready(sim: &mut Sim, a: Entity, b: Entity) {
    line_up(sim, a, b, 3.0);
    if crate::gun_game::rung(sim, b) != HALO_RUNG {
        set_rung(sim, b, HALO_RUNG);
        sim.seconds(1.2);
        line_up(sim, a, b, 3.0);
    }
    // The armour it brought back is the shield's start (docs/DESIGN.md
    // "Armour when the identity switches"): recharged by Halo's rule.
    for _ in 0..(11.0 * sim.tick_hz()) as u32 {
        if sim.app.world().get::<Vitality>(b).is_some_and(|v| v.shield == 1.0) {
            break;
        }
        sim.ticks(1);
    }
    let v = sim.app.world().get::<Vitality>(b).unwrap_or_else(|| panic!("a Halo player: {:?}", crate::gun_game::held(sim, b)));
    assert_eq!((v.shield, v.body), (1.0, 1.0));
}

/// Hits to kill a bare 100-point character with `per` a hit.
fn bare(per: f32) -> u32 {
    (1.0 / per - 1e-4).ceil() as u32
}

/// The guns of the table: (id, region).
const GUNS: [(&str, BodyRegion); 9] = [
    ("cs_source:weapon_ak47", BodyRegion::Chest),
    ("cs_source:weapon_ak47", BodyRegion::Head),
    ("cs_source:weapon_awp", BodyRegion::Chest),
    ("cs_source:weapon_deagle", BodyRegion::Chest),
    ("cs_source:weapon_mp5navy", BodyRegion::Chest),
    ("hl2dm:weapon_357", BodyRegion::Chest),
    ("hl2dm:weapon_smg1", BodyRegion::Chest),
    ("tf2:tf_weapon_scattergun", BodyRegion::Chest),
    ("combat_arms:m4a1", BodyRegion::Chest),
];

/// Each gun's hits to kill a full Halo player with the balance `cfg`,
/// with its hit's damage: (id, region, per hit, hits).
fn table(cfg: CrossGame) -> Vec<(&'static str, BodyRegion, f32, u32)> {
    let (mut sim, a, b) = halo_victim(cfg);
    let mut out = Vec::new();
    for (id, region) in GUNS {
        let (body, head) = per_hit(&sim, id);
        let raw = if region == BodyRegion::Head { body * head } else { body };
        ready(&mut sim, a, b);
        let n = hits_to_kill(&mut sim, b, id, raw, region);
        out.push((id, region, raw, n));
    }
    out
}

/// Another game's gun on a full Halo player: 1.5 times against the shield
/// and the body with the switch on (133 effective hit points), as it comes
/// with it off (200); each row printed (`-- --nocapture`).
#[test]
fn other_games_guns_on_a_halo_player_count_the_incoming_scale() {
    let off = table(CrossGame::off());
    let on = table(CrossGame::load());
    let k = CrossGame::load().incoming();
    assert_eq!(k, 1.5);
    println!("gun, region, per hit (HP), bare 100, Halo off (200), Halo on (133)");
    for ((id, region, raw, today), (_, _, raw_on, now)) in off.iter().zip(&on) {
        assert_eq!(raw, raw_on);
        println!("{id} {region:?}: {:.1} {} {today} {now}", raw * 100.0, bare(*raw));
        // The counted pool: 200 points as they come, 200 / 1.5 with it on.
        assert_eq!(*today, (2.0 / raw - 1e-4).ceil() as u32, "{id} {region:?} off");
        assert_eq!(*now, (2.0 / (raw * k) - 1e-4).ceil() as u32, "{id} {region:?} on");
    }
    let row = |t: &[(&str, BodyRegion, f32, u32)], id: &str, region: BodyRegion| {
        t.iter().find(|r| r.0 == id && r.1 == region).map(|r| (bare(r.2), r.3)).unwrap()
    };
    // The brief's two: the AK-47 3 on a bare character, 6 today, 4 now; the
    // AWP 1, 2 and still 2 (one shot from a scale of 200 / 114 = 1.754).
    assert_eq!(row(&off, "cs_source:weapon_ak47", BodyRegion::Chest), (3, 6));
    assert_eq!(row(&on, "cs_source:weapon_ak47", BodyRegion::Chest), (3, 4));
    assert_eq!(row(&off, "cs_source:weapon_awp", BodyRegion::Chest), (1, 2));
    assert_eq!(row(&on, "cs_source:weapon_awp", BodyRegion::Chest), (1, 2));
    // The attacker's own head multiplier as before: an AK-47 headshot (x4).
    assert_eq!(row(&off, "cs_source:weapon_ak47", BodyRegion::Head).1, 2);
    assert_eq!(row(&on, "cs_source:weapon_ak47", BodyRegion::Head).1, 1);
}

/// The whole chain by real fire: a CS:S AK-47 at 3 m kills a full Halo
/// player in 4 body hits (the shield's, then the body's), not 6.
#[test]
fn a_real_ak47_kills_a_halo_player_in_four_body_hits() {
    let text = crate::hl2dm_parts::ladder_text(&["cs_source:weapon_ak47", weapons::ASSAULT_RIFLE, "cs_source:weapon_glock"], false).replace(
        "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]",
        "weapons = [\"cs_source\", \"hl2dm\", \"halo_ce\", \"combat_arms\"]",
    );
    let mut sim = crate::halo_ladder::party_with(&text);
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    set_rung(&mut sim, a, 0);
    set_rung(&mut sim, b, 1);
    sim.seconds(1.2);
    line_up(&mut sim, a, b, 3.0);
    let mut hits = 0;
    let mut last = (1.0f32, 1.0f32);
    for _ in 0..20 {
        let at = sim.position(b);
        chimera_sim::testkit::aim_at(&mut sim, a, at);
        sim.intent(a).fire = true;
        sim.ticks(1);
        // A pause after each round: the first shot's accuracy.
        sim.intent(a).fire = false;
        sim.seconds(0.4);
        if !alive(&sim, b) {
            hits += 1;
            break;
        }
        let v = sim.app.world().get::<Vitality>(b).unwrap();
        let now = (v.shield, v.body);
        if now.0 < last.0 || now.1 < last.1 {
            hits += 1;
        }
        last = now;
    }
    assert!(!alive(&sim, b), "the Halo player died");
    assert_eq!(hits, 4, "body hits to kill");
}
