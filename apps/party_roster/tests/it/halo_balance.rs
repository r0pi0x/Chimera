//! Halo's balance across games in the party (the cross-game table,
//! party_kit's data/party_balance.toml, `party_roster::balance`; Halo's
//! mechanisms in `halo_ce::game::cross_game`; the user's decision of
//! 2026-11-15), on the party's composed simulation with the other games'
//! real gun data:
//!
//! - **Incoming**: another game's gun on a full Halo player counts 1.5
//!   times against the shield, then the body (133 effective hit points
//!   where any other character has 100; 200 in a raw match): each
//!   gun's body hit at 10 m from its data (`power::stats`, the games'
//!   own readers) through the engine's armour step (`weapon::HitArmor`,
//!   as a gun's hit is) and health, hits to kill counted, the switch on
//!   and off; a CS:S AK-47 by real fire kills one in 4 body hits.
//! - **Outgoing**: Halo's guns, explosives and melee on a bare character
//!   and a Combat Arms soldier with its gear, through Halo's damage seam,
//!   with the party's table (Halo's outgoing 0.75: a Halo point one hit
//!   point) and in a raw match (1.333).
//! - **The leak**: through real rung changes, a character leaving Halo's
//!   rung gets back the armour it brought (CS:S none, HL2:DM's suit,
//!   Combat Arms' gear as it was), never the shield's value, the switch on
//!   or off; Halo's rung to another keeps the shield as it is.

use bevy::{ecs::system::RunSystemOnce, prelude::*};
use chimera_core::core::{BodyRegion, Damage, DamageKind, Health};
use chimera_sim::{
    minigame::power::{self, PowerCurve, PowerReaders},
    weapon::{HitArmor, data::WeaponCatalog},
};
use chimera_world::harness::Sim;
use chimera_sim::weapon::balance::{Balance, game_of};
use halo_ce::game::{
    damage::HaloHit,
    vitality::{DamageEffects, EffectId, Vitality, ids},
    weapons,
};

use crate::{
    gun_game::{alive, line_up, soldier},
    halo_ladder::{party, set_rung},
};

/// The halo_ladder's rung of Halo's assault rifle.
const HALO_RUNG: usize = 1;

/// A gun's body hit at 10 m, every pellet landing, from its data and its
/// game's power reader (normalized: 1 = 100 hit points), as its own game
/// deals it (the party's per-game scale is the hit's, `gun_hit`), and its
/// head's multiplier.
pub(crate) fn per_hit(sim: &Sim, id: &str) -> (f32, f32) {
    let w = sim.app.world();
    let data = w.resource::<WeaponCatalog>().get(id).unwrap_or_else(|| panic!("{id} in the catalogue"));
    let mut readers = w.get_resource::<PowerReaders>().cloned().unwrap_or_default();
    readers.game_damage = Default::default();
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

/// The party's table, the switch on, else a raw match.
pub(crate) fn party_table(on: bool) -> Balance {
    let t = party_roster::balance::table();
    if on { t } else { t.raw() }
}

/// The party (halo_ladder's), with the table `cfg`: a player on the
/// first rung and one (the victim) on Halo's.
pub(crate) fn halo_victim(cfg: Balance) -> (Sim, Entity, Entity) {
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

/// Each gun's hits to kill a full Halo player with the table `cfg`,
/// with its hit's damage: (id, region, per hit, hits).
fn table(cfg: Balance) -> Vec<(&'static str, BodyRegion, f32, u32)> {
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
/// in a raw match (200, each gun its own game's number); each row printed
/// (`-- --nocapture`).
#[test]
fn other_games_guns_on_a_halo_player_count_the_incoming_scale() {
    let off = table(party_table(false));
    let on = table(party_table(true));
    let table = party_roster::balance::table();
    let k = table.incoming(Some("halo_ce"), None);
    assert_eq!(k, 1.5);
    println!("gun, region, per hit (HP), bare 100, Halo raw (200), Halo on (133)");
    let out = |id: &str| table.outgoing(game_of(id));
    for ((id, region, raw, today), (_, _, raw_on, now)) in off.iter().zip(&on) {
        assert_eq!(raw, raw_on);
        println!("{id} {region:?}: {:.1} {} {today} {now}", raw * out(id) * 100.0, bare(raw * out(id)));
        // The counted pool: 200 points as they come in a raw match, 200 /
        // 1.5 with the table (as the party deals it: HL2:DM's 1.5).
        assert_eq!(*today, (2.0 / raw - 1e-4).ceil() as u32, "{id} {region:?} raw");
        assert_eq!(*now, (2.0 / (raw * out(id) * k) - 1e-4).ceil() as u32, "{id} {region:?} on");
    }
    let row = |t: &[(&str, BodyRegion, f32, u32)], id: &str, region: BodyRegion| {
        t.iter().find(|r| r.0 == id && r.1 == region).map(|r| (bare(r.2 * out(id)), r.3)).unwrap()
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

// ---------------------------------------------------------------------------
// Outgoing: a Halo source on another game's character, through Halo's own
// conversion (a point 1/75 of its health) and the party's table
// (data/party_balance.toml: Halo's outgoing 0.75, so a point is a hit point).

/// How a row's hit is dealt.
#[derive(Clone, Copy, Debug)]
enum Way {
    /// A direct hit on the chest or the head, `n` at once (a shell's
    /// pellets).
    Direct(EffectId, BodyRegion, u32),
    /// An explosion `m` metres from the target's centre.
    Blast(EffectId, f32),
    /// A melee at speed scale `s`, from behind when `behind`.
    Melee(EffectId, f32, bool),
}

/// The rows of the report's table.
const OUTGOING: [(&str, Way); 19] = [
    ("assault rifle", Way::Direct(ids::ASSAULT_RIFLE_BULLET, BodyRegion::Chest, 1)),
    ("pistol, body", Way::Direct(ids::PISTOL_BULLET, BodyRegion::Chest, 1)),
    ("pistol, head", Way::Direct(ids::PISTOL_BULLET, BodyRegion::Head, 1)),
    ("sniper rifle, body", Way::Direct(ids::SNIPER_BULLET, BodyRegion::Chest, 1)),
    ("plasma rifle", Way::Direct(ids::PLASMA_RIFLE_BOLT, BodyRegion::Chest, 1)),
    ("plasma pistol, tapped", Way::Direct(ids::PLASMA_PISTOL_BOLT, BodyRegion::Chest, 1)),
    ("shotgun, point blank (15 pellets)", Way::Direct(ids::SHOTGUN_PELLET, BodyRegion::Chest, 15)),
    ("rocket, direct", Way::Blast(ids::ROCKET_EXPLOSION, 0.0)),
    ("rocket blast, 2 m", Way::Blast(ids::ROCKET_EXPLOSION, 2.0)),
    ("rocket blast, 4 m", Way::Blast(ids::ROCKET_EXPLOSION, 4.0)),
    ("rocket blast, 6 m", Way::Blast(ids::ROCKET_EXPLOSION, 6.0)),
    ("frag, 2 m", Way::Blast(ids::FRAG_EXPLOSION, 2.0)),
    ("frag, 4 m", Way::Blast(ids::FRAG_EXPLOSION, 4.0)),
    ("frag, 6 m", Way::Blast(ids::FRAG_EXPLOSION, 6.0)),
    ("frag, 7.5 m", Way::Blast(ids::FRAG_EXPLOSION, 7.5)),
    ("melee, standing", Way::Melee(ids::ASSAULT_RIFLE_MELEE, 0.0, false)),
    ("melee, running", Way::Melee(ids::ASSAULT_RIFLE_MELEE, 1.0, false)),
    ("melee, lunging", Way::Melee(ids::ASSAULT_RIFLE_MELEE, 1.5, false)),
    ("melee, from behind", Way::Melee(ids::ASSAULT_RIFLE_MELEE, 0.0, true)),
];

/// One of `way`'s hits on `target` (the effect's mean draw), applied.
fn halo_hit(sim: &mut Sim, target: Entity, way: Way) {
    const WU: f32 = 3.048;
    let fx = sim.app.world().resource::<DamageEffects>().clone();
    let at = sim.position(target);
    let look = sim
        .app
        .world()
        .get::<chimera_core::core::Intent>(target)
        .map_or(Vec3::NEG_Z, |i| chimera_sim::weapon::aim_of(i.yaw, 0.0) * Vec3::NEG_Z);
    let hits: Vec<HaloHit> = match way {
        Way::Direct(effect, region, n) => {
            let amount = fx.get(effect).unwrap().amount(1.0, 0.5);
            (0..n).map(|_| HaloHit::direct(target, effect, amount, 1.0, Some(region), at, Vec3::NEG_Z, None)).collect()
        }
        Way::Blast(effect, m) => {
            let f = fx.get(effect).unwrap();
            let [r0, r1] = f.radius;
            let d = m / WU;
            let s = if r1 <= r0 { 1.0 } else { (1.0 - (d - r0) / (r1 - r0)).clamp(0.0, 1.0) };
            vec![HaloHit::area(target, effect, f.amount(s, 0.5), s, at - Vec3::X * m, Vec3::X, None)]
        }
        Way::Melee(effect, s, behind) => {
            let amount = fx.get(effect).unwrap().amount(s, 0.5);
            let from = if behind { at - look } else { at + look };
            vec![HaloHit::melee(target, effect, amount, s, from, (at - from).normalize(), None)]
        }
    };
    let w = sim.app.world_mut();
    for h in hits {
        w.write_message(h.weapon(weapons::ASSAULT_RIFLE));
    }
    sim.ticks(1);
}

/// `gun_game::line_up` without the respawn protection's two seconds (a
/// kill's victim, not hurt-proof): `target` alive, 3 m before `shooter`.
fn back_in_play(sim: &mut Sim, shooter: Entity, target: Entity) {
    for _ in 0..(6.0 * sim.tick_hz()) as u32 {
        if alive(sim, target) {
            break;
        }
        sim.ticks(1);
    }
    assert!(alive(sim, target), "respawned");
    sim.app.world_mut().entity_mut(target).remove::<(
        chimera_core::core::God,
        chimera_core::core::SpawnProtected,
        chimera_sim::minigame::runtime::Protected,
    )>();
    let from = crate::gun_game::OPEN;
    crate::gun_game::place(sim, shooter, from);
    crate::gun_game::place(sim, target, from + Vec3::new(0.0, 0.0, -3.0));
    // Settle, and finish drawing what the last kill gave.
    sim.seconds(0.5);
}

/// `way`'s hits to kill a full `target`, put on rung `r` (the ladder's)
/// first.
fn halo_hits_to_kill(sim: &mut Sim, a: Entity, target: Entity, r: usize, way: Way) -> u32 {
    // A death with no attacker costs a rung: put back while it is dead, so
    // its respawn (waited for in `line_up`) comes with the rung's gear.
    if crate::gun_game::rung(sim, target) != r {
        set_rung(sim, target, r);
    }
    back_in_play(sim, a, target);
    if crate::gun_game::rung(sim, target) != r {
        set_rung(sim, target, r);
        sim.seconds(1.2);
        back_in_play(sim, a, target);
    }
    // Bare: no armour (another rung's would stay as kevlar); Combat Arms':
    // its full gear.
    let mut e = sim.app.world_mut().entity_mut(target);
    if r == 0 {
        e.remove::<chimera_sim::weapon::Armor>();
    } else {
        e.insert(chimera_sim::weapon::Armor { amount: 1.0, helmet: false });
    }
    sim.ticks(2);
    for n in 1..=40 {
        halo_hit(sim, target, way);
        if !alive(sim, target) {
            return n;
        }
    }
    panic!("{way:?}: no kill in 40 hits");
}

/// The rows' hits to kill on a bare CS:S character (rung 0) and a Combat
/// Arms soldier with its gear (rung 2), with the party's table or in a
/// raw match.
fn outgoing(sim: &mut Sim, a: Entity, b: Entity, table: bool, r: usize) -> Vec<u32> {
    sim.app.insert_resource(party_table(table));
    OUTGOING.iter().map(|(_, way)| halo_hits_to_kill(sim, a, b, r, *way)).collect()
}

/// `outgoing` for both tables and both victims in one party, a victim at a
/// time (no switching between rows): ((bare before, bare after), (Combat
/// Arms before, after)) rows.
fn outgoing_all() -> (Vec<(u32, u32)>, Vec<(u32, u32)>) {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    // The victim is back after 0.3 s, not 3 (its wait is no part of
    // what is measured).
    sim.app.world_mut().entity_mut(b).insert(chimera_sim::minigame::runtime::RespawnDelay(0.3));
    let mut rows = Vec::new();
    for r in [0, 2] {
        let before = outgoing(&mut sim, a, b, false, r);
        let after = outgoing(&mut sim, a, b, true, r);
        rows.push(before.into_iter().zip(after).collect::<Vec<_>>());
    }
    let ca = rows.pop().unwrap();
    (rows.pop().unwrap(), ca)
}

/// Halo's guns on other games' characters, before (a point 1.333 hit
/// points) and after the party's table (one): hits to kill a bare
/// character and a Combat Arms soldier with its gear; printed
/// (`-- --nocapture`).
#[test]
fn halos_guns_on_other_games_characters_count_a_point_a_hit_point() {
    let (bare, ca) = outgoing_all();
    let before: Vec<(u32, u32)> = bare.iter().zip(&ca).map(|(b, c)| (b.0, c.0)).collect();
    let after: Vec<(u32, u32)> = bare.iter().zip(&ca).map(|(b, c)| (b.1, c.1)).collect();
    assert_eq!(party_roster::balance::table().outgoing("halo_ce"), 0.75);
    println!("row: bare before/after, Combat Arms before/after");
    for (((name, _), b), a) in OUTGOING.iter().zip(&before).zip(&after) {
        println!("{name}: {} / {}, {} / {}", b.0, a.0, b.1, a.1);
    }
    let row = |name: &str| {
        let k = OUTGOING.iter().position(|(n, _)| *n == name).unwrap();
        (before[k], after[k])
    };
    // The brief's: the assault rifle 8 then 10 bullets; the pistol 2 then
    // 3 body shots, a headshot a kill either way; the sniper rifle 1 body
    // shot either way (101 points: 135 then 101 hit points); a melee from
    // behind kills either way.
    assert_eq!((row("assault rifle").0.0, row("assault rifle").1.0), (8, 10));
    assert_eq!((row("pistol, body").0.0, row("pistol, body").1.0), (2, 3));
    assert_eq!((row("pistol, head").0.0, row("pistol, head").1.0), (1, 1));
    assert_eq!((row("sniper rifle, body").0.0, row("sniper rifle, body").1.0), (1, 1));
    assert_eq!((row("melee, from behind").0.0, row("melee, from behind").1.0), (1, 1));
    // Never stronger than before.
    for ((name, _), (b, a)) in OUTGOING.iter().zip(before.iter().zip(&after)) {
        assert!(a.0 >= b.0 && a.1 >= b.1, "{name}: {b:?} -> {a:?}");
    }
}

// ---------------------------------------------------------------------------
// The armour a character brings to Halo's rung is what it gets back when it
// leaves (`cross_game::Brought`), never the shield's value.

/// CS:S's, Halo's, HL2:DM's and Combat Arms' rungs, Halo's between each.
const SWITCHES: [&str; 6] = [
    "cs_source:weapon_glock",
    weapons::ASSAULT_RIFLE,
    "hl2dm:weapon_smg1",
    weapons::PISTOL,
    "combat_arms:m4a1",
    weapons::PLASMA_RIFLE,
];

/// The party playing `SWITCHES` with the table `cfg`, and one player.
fn switching(cfg: Balance) -> (Sim, Entity) {
    let text = crate::hl2dm_parts::ladder_text(&SWITCHES, false).replace(
        "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]",
        "weapons = [\"cs_source\", \"hl2dm\", \"halo_ce\", \"combat_arms\"]",
    );
    let mut sim = crate::halo_ladder::party_with(&text);
    sim.app.insert_resource(cfg);
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    (sim, a)
}

/// `e` onto rung `r`, its gun in hand.
fn onto(sim: &mut Sim, e: Entity, r: usize) {
    set_rung(sim, e, r);
    sim.seconds(1.2);
    assert_eq!(crate::gun_game::held(sim, e), Some(SWITCHES[r]), "rung {r}");
}

/// `e`'s armour: (amount, helmet), None without.
fn armour(sim: &Sim, e: Entity) -> Option<(f32, bool)> {
    sim.app.world().get::<chimera_sim::weapon::Armor>(e).map(|a| (a.amount, a.helmet))
}

/// `e` on Halo's rung `r`, its shield recharged to full (whatever it
/// brought: the shield's start).
fn halo_full(sim: &mut Sim, e: Entity, r: usize) {
    onto(sim, e, r);
    sim.seconds(10.5);
    let v = sim.app.world().get::<Vitality>(e).expect("Halo's");
    assert_eq!(v.shield, 1.0, "recharged");
}

/// Through real rung changes: a CS:S player (no armour) leaves Halo's rung
/// with no kevlar; an HL2:DM one with its suit as before (30 points); a
/// Combat Arms one with its gear as before (half of it, not the shield's
/// full value);
/// Halo's rung to another keeps the shield as it is. The same in a raw
/// match: the leak's fix is a behaviour, not a number.
#[test]
fn leaving_halo_gives_back_the_armour_it_brought() {
    for on in [true, false] {
        let cfg = party_table(on);
        let (mut sim, a) = switching(cfg);
        // CS:S -> Halo -> CS:S.
        onto(&mut sim, a, 0);
        assert_eq!(armour(&sim, a), None, "CS:S: none");
        halo_full(&mut sim, a, 1);
        onto(&mut sim, a, 0);
        assert_eq!(armour(&sim, a), None, "on {on}: CS:S after Halo");
        // HL2:DM -> Halo -> HL2:DM.
        // (The suit's armour, 30 points of it used up.)
        onto(&mut sim, a, 2);
        assert!(armour(&sim, a).is_some(), "HL2:DM's suit");
        sim.app.world_mut().get_mut::<chimera_sim::weapon::Armor>(a).unwrap().amount = 0.3;
        sim.ticks(2);
        halo_full(&mut sim, a, 3);
        onto(&mut sim, a, 2);
        assert_eq!(armour(&sim, a), Some((0.3, false)), "on {on}: HL2:DM after Halo");
        // Combat Arms (its gear, half used) -> Halo -> Combat Arms.
        onto(&mut sim, a, 4);
        sim.app.world_mut().get_mut::<chimera_sim::weapon::Armor>(a).unwrap().amount = 0.5;
        sim.ticks(2);
        halo_full(&mut sim, a, 5);
        onto(&mut sim, a, 4);
        assert_eq!(armour(&sim, a).map(|a| a.0), Some(0.5), "on {on}: Combat Arms after Halo");
        assert!(sim.app.world().get::<chimera_sim::weapon::ArmorPieces>(a).is_some(), "its gear on");
        // Halo -> Halo: the shield as it is.
        onto(&mut sim, a, 1);
        sim.seconds(10.5);
        let unit = if on { 1.5 } else { 1.0 };
        sim.app.world_mut().get_mut::<chimera_sim::weapon::Armor>(a).unwrap().amount = 0.4 / unit;
        sim.ticks(2);
        let before = sim.app.world().get::<Vitality>(a).unwrap().shield;
        assert!((before - 0.4).abs() < 1e-5, "{before}");
        set_rung(&mut sim, a, 3);
        sim.ticks(3);
        assert_eq!(crate::gun_game::held(&sim, a), Some(weapons::PISTOL));
        let v = sim.app.world().get::<Vitality>(a).unwrap();
        assert_eq!(v.shield, before, "on {on}: the shield kept from one Halo rung to the next");
        assert!(v.delay > 0, "still waiting to recharge");
    }
}

// ---------------------------------------------------------------------------
// HL2:DM's guns in the party (data/party_balance.toml: its outgoing 1.5), by
// their real data, on a bare character, a Combat Arms soldier with its
// gear and a full Halo player; and the ladder's power of every gun.

/// HL2:DM's five guns of the ladder.
const HL2: [&str; 5] = ["hl2dm:weapon_pistol", "hl2dm:weapon_357", "hl2dm:weapon_smg1", "hl2dm:weapon_ar2", "hl2dm:weapon_shotgun"];

/// `target` on rung `r` (0 bare CS:S, 1 Halo, 2 Combat Arms), back in
/// play and full: no armour on CS:S, the full gear on Combat Arms, a full
/// shield on Halo.
fn full_on(sim: &mut Sim, a: Entity, target: Entity, r: usize) {
    if crate::gun_game::rung(sim, target) != r {
        set_rung(sim, target, r);
    }
    back_in_play(sim, a, target);
    if crate::gun_game::rung(sim, target) != r {
        set_rung(sim, target, r);
        sim.seconds(1.2);
        back_in_play(sim, a, target);
    }
    match r {
        0 => {
            sim.app.world_mut().entity_mut(target).remove::<chimera_sim::weapon::Armor>();
        }
        2 => {
            sim.app.world_mut().entity_mut(target).insert(chimera_sim::weapon::Armor { amount: 1.0, helmet: false });
        }
        _ => {
            for _ in 0..(11.0 * sim.tick_hz()) as u32 {
                if sim.app.world().get::<Vitality>(target).is_some_and(|v| v.shield == 1.0 && v.body == 1.0) {
                    break;
                }
                sim.ticks(1);
            }
        }
    }
    sim.ticks(2);
}

/// HL2:DM's guns' hits to kill at the party's scale `scale`: for each gun
/// (bare, Combat Arms with gear, Halo player), and its own shot (all
/// pellets, before the scale) and cycle.
fn hl2_rows(scale: f32) -> Vec<(&'static str, f32, f32, [u32; 3])> {
    let mut sim = party();
    let mut table = party_roster::balance::table();
    table.games.entry("hl2dm".into()).or_default().outgoing = scale;
    sim.app.insert_resource(table);
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    sim.app.world_mut().entity_mut(b).insert(chimera_sim::minigame::runtime::RespawnDelay(0.3));
    let mut out = Vec::new();
    for id in HL2 {
        let (shot, _) = per_hit(&sim, id);
        let data = sim.app.world().resource::<WeaponCatalog>().get(id).unwrap();
        let readers = sim.app.world().get_resource::<PowerReaders>().cloned().unwrap_or_default();
        let cycle = power::stats(data, &readers, &PowerCurve::shipped(), 10.0).interval;
        let mut hits = [0; 3];
        for (k, r) in [0, 2, HALO_RUNG].into_iter().enumerate() {
            full_on(&mut sim, a, b, r);
            hits[k] = hits_to_kill(&mut sim, b, id, shot, BodyRegion::Chest);
        }
        out.push((id, shot, cycle, hits));
    }
    out
}

/// HL2:DM's guns at 1.0, 1.5 (the party's) and 2.0: hits to kill and
/// seconds (hits - 1 cycles) on each victim; printed (`-- --nocapture`).
#[test]
fn hl2dms_guns_take_the_party_s_scale() {
    assert_eq!(party_roster::balance::table().outgoing("hl2dm"), 1.5);
    let mut at = Vec::new();
    for scale in [1.0, 1.5, 2.0] {
        let rows = hl2_rows(scale);
        for (id, shot, cycle, hits) in &rows {
            let secs = hits.map(|n| (n - 1) as f32 * cycle);
            println!(
                "x{scale}: {id}: {:.1} a shot, every {:.3} s: bare {} ({:.2} s), Combat Arms {} ({:.2} s), Halo {} ({:.2} s)",
                shot * 100.0 * scale,
                cycle,
                hits[0],
                secs[0],
                hits[1],
                secs[1],
                hits[2],
                secs[2]
            );
        }
        at.push(rows);
    }
    let bare = |k: usize, id: &str| at[k].iter().find(|r| r.0 == id).map(|r| (r.1, r.3[0])).unwrap();
    // At 1.0 HL2:DM's own numbers: the SMG1 20, the AR2 10, the pistol 13,
    // the .357 2, the shotgun 2 shells.
    let own: Vec<u32> = HL2.iter().map(|id| bare(0, id).1).collect();
    assert_eq!(own, [13, 2, 20, 10, 2]);
    // At 1.5: 9, 1 (112.5), 14, 7 and still 2 shells (94.5).
    let party: Vec<u32> = HL2.iter().map(|id| bare(1, id).1).collect();
    assert_eq!(party, [9, 1, 14, 7, 2]);
    assert!((bare(1, "hl2dm:weapon_357").0 * 1.5 - 1.125).abs() < 1e-4);
    assert!((bare(1, "hl2dm:weapon_shotgun").0 * 1.5 - 0.945).abs() < 1e-4);
}

/// The ladder's power of every classed gun of the party, by band then
/// score (`ladder_power`'s table), with the party's per-game table and
/// without it; HL2:DM's guns marked `*`, Halo's `#` (`-- --nocapture`).
#[test]
fn the_ladder_s_power_with_and_without_the_party_s_table() {
    let mut sim = crate::gun_game::party();
    let curve = PowerCurve::shipped();
    for with in [false, true] {
        let table = if with { party_roster::balance::table().power_table() } else { Default::default() };
        sim.app.world_mut().resource_mut::<PowerReaders>().game_damage = table;
        println!("ladder power, {}:", if with { "the party's table" } else { "no table" });
        for (id, class, p) in power::table(sim.app.world(), &curve) {
            let mark = if id.starts_with("hl2dm:") { "*" } else if id.starts_with("halo_ce:") { "#" } else { " " };
            println!("{mark} {:<9} {:>6.3}  ttk {:>5.2} s  {} ({})", p.band.name(), p.score, p.ttk, id, class.name());
        }
    }
}
