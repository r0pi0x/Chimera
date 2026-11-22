//! Halo's grenade rung (`halo_ce::game::weapons::rung`) in the party,
//! headless on the party's simulation with stand-in bodies
//! (`halo_ladder::party_with`): Halo lends it as a part of the grenade
//! band (an automatic ladder's report deals it as a grenade, and no other
//! weapon of any game declares a rung hint); its holder has the pistol
//! with no round loaded and none in reserve that nothing refills, and
//! Halo's four frags (the rung's own supply with the ladder's
//! `refill_ammo`: timed, one back every `refill_seconds`), no plasma grenade, with `halo_party_grenades` off; the fire
//! button dry-fires and throws nothing, the grenade button throws a frag
//! (spent, coming back over time); a frag kill is in the kill feed as the frag
//! grenade (name and id) and advances the thrower; the next Halo rung has
//! no grenades again. The rung hint: set on arriving (Halo's grenade
//! action, "throw"), gone after its seconds, back on a dry fire; the key it
//! names is the one bound now (a rebind follows); a rung without one shows
//! none.

use bevy::prelude::*;
use chimera_client::client::{minigame_scoreboard::gun_name, rung_hint};
use chimera_core::core::{ActionRegistry, LocalPlayer};
use chimera_sim::{
    minigame::{auto, hud::MinigameHud},
    weapon::{Magazine, WeaponEvent, WeaponEventKind, data::WeaponCatalog},
};
use chimera_world::harness::Sim;
use halo_ce::game::weapons::{
    self, HaloGun, HaloHands, HaloProjectile,
    grenades::{GRENADE_ACTION, PartyGrenades},
    rung::FRAG_RUNG,
};

use crate::{
    play::{alive, held, line_up, rung, soldier},
    halo_party::party_with,
    play::set_rung,
};

const LADDER: [&str; 4] = ["cs_source:weapon_glock", FRAG_RUNG, weapons::ASSAULT_RIFLE, "combat_arms:mp5a4"];

fn party() -> Sim {
    let text = crate::play::ladder_text(&LADDER, false).replace(
        "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]",
        "weapons = [\"cs_source\", \"hl2dm\", \"halo_ce\", \"combat_arms\"]",
    );
    // (The kill feed's state is `party_with`'s.)
    party_with(&text)
}

fn hands(sim: &Sim, e: Entity) -> HaloHands {
    *sim.app.world().get::<HaloHands>(e).expect("Halo's hands")
}

fn gun(sim: &Sim, e: Entity) -> (i16, i16, u32, u32) {
    let w = sim.app.world();
    let g = w.get::<chimera_sim::weapon::Inventory>(e).and_then(|i| i.active).expect("a weapon in hand");
    let h = w.get::<HaloGun>(g).expect("a Halo gun");
    let m = w.get::<Magazine>(g).expect("its magazine");
    (h.loaded, h.reserve, m.clip, m.reserve)
}

fn frags_flying(sim: &mut Sim) -> usize {
    let w = sim.app.world_mut();
    w.query::<&HaloProjectile>().iter(w).filter(|p| p.id == FRAG_RUNG).count()
}

fn grenade_bit(sim: &Sim) -> u32 {
    sim.app.world().resource::<ActionRegistry>().bit(GRENADE_ACTION)
}

fn dry_fires(sim: &Sim, cursor: &mut bevy::ecs::message::MessageCursor<WeaponEvent>, e: Entity) -> usize {
    cursor.read(sim.app.world().resource::<Messages<WeaponEvent>>()).filter(|m| m.owner == e && !m.replay && matches!(m.kind, WeaponEventKind::DryFire)).count()
}

#[test]
fn halo_lends_the_frag_rung_to_the_grenade_band_and_only_it_has_a_hint() {
    let sim = party();
    let w = sim.app.world();
    let caps = auto::report(w);
    let halo = caps.iter().find(|c| c.game == halo_ce::game::GAME).expect("Halo's report");
    assert_eq!(halo.grenades, vec![FRAG_RUNG], "Halo's grenade-band part");
    assert!(halo.guns.iter().all(|g| g.id != FRAG_RUNG), "not among its guns");
    let catalog = w.resource::<WeaponCatalog>();
    assert_eq!(catalog.get(FRAG_RUNG).and_then(|d| d.class), Some(chimera_core::game_parts::GunClass::Grenade));
    // Every weapon any game lends (every rung a ladder may deal): only
    // the frag rung declares a hint.
    let hinted: Vec<&str> = caps
        .iter()
        .flat_map(|c| c.guns.iter().map(|g| g.id).chain(c.grenades.iter().copied()).chain(c.knife))
        .filter(|id| catalog.get(id).is_some_and(|d| d.rung_hint.is_some()))
        .collect();
    assert_eq!(hinted, vec![FRAG_RUNG]);
    let hint = catalog.get(FRAG_RUNG).unwrap().rung_hint.clone().unwrap();
    assert_eq!((hint.action.as_str(), hint.verb.as_str()), (GRENADE_ACTION, "throw"));
    assert_eq!(gun_name(FRAG_RUNG, None, Some(catalog)), "Frag Grenade", "the rung's display name");
}

#[test]
fn the_holder_has_an_empty_pistol_and_frags_and_throws_with_the_grenade_button() {
    let mut sim = party();
    assert_eq!(sim.app.world().get_resource::<PartyGrenades>(), Some(&PartyGrenades(false)), "the switch off");
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.app.world_mut().entity_mut(a).insert(LocalPlayer);
    sim.seconds(1.5);
    set_rung(&mut sim, a, 1);
    sim.seconds(1.5);
    assert_eq!(held(&sim, a), Some(FRAG_RUNG));
    assert_eq!(
        sim.app.world().get::<chimera_core::slots::GameIdentity>(a).map(|g| g.0.as_str()),
        Some(halo_ce::game::GAME),
        "a Halo player"
    );
    // The hint on arriving.
    let now = sim.app.world().resource::<Time>().elapsed_secs_f64();
    let h = sim.app.world().resource::<MinigameHud>().rung_hint.clone().expect("the rung's hint");
    assert_eq!((h.action.as_str(), h.verb.as_str()), (GRENADE_ACTION, "throw"));
    assert!(h.shown(now), "shown on arriving: {h:?} at {now}");
    let h = hands(&sim, a);
    assert_eq!(h.throw.counts, [4, 0], "Halo's four frags, no plasma");
    assert_eq!(h.throw.selected(), Some(0), "the frag plate");
    assert_eq!(gun(&sim, a), (0, 0, 4, 0), "the empty pistol; its magazine states the frags");
    // The ladder's refill gives nothing to the pistol.
    sim.seconds(2.0);
    assert_eq!(gun(&sim, a).0, 0);
    assert_eq!(gun(&sim, a).1, 0);
    // The hint goes after its seconds.
    sim.seconds(4.0);
    let now = sim.app.world().resource::<Time>().elapsed_secs_f64();
    assert!(!sim.app.world().resource::<MinigameHud>().rung_hint.as_ref().unwrap().shown(now), "gone");
    // The fire button: a dry fire, nothing thrown; the hint again.
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut dry = 0;
    for i in 0..(1.0 * sim.tick_hz()) as u32 {
        sim.intent(a).fire = i % 10 < 5;
        sim.ticks(1);
        dry += dry_fires(&sim, &mut cursor, a);
    }
    sim.intent(a).fire = false;
    assert!(dry > 0, "the empty pistol's click");
    assert_eq!(frags_flying(&mut sim), 0, "no throw on the fire button");
    assert_eq!(hands(&sim, a).throw.counts, [4, 0]);
    let now = sim.app.world().resource::<Time>().elapsed_secs_f64();
    assert!(sim.app.world().resource::<MinigameHud>().rung_hint.as_ref().unwrap().shown(now), "the hint again on the dry fire");
    // The grenade button: a frag thrown, given back.
    let bit = grenade_bit(&sim);
    sim.intent(a).actions |= bit;
    let mut thrown = false;
    for _ in 0..(0.8 * sim.tick_hz()) as u32 {
        sim.ticks(1);
        thrown |= frags_flying(&mut sim) > 0;
    }
    sim.intent(a).actions &= !bit;
    assert!(thrown, "a frag in flight");
    sim.seconds(1.0);
    assert!(hands(&sim, a).throw.counts[0] < 4, "not given back at once (the supply is timed)");
    // The next Halo rung: no grenades with the switch off.
    set_rung(&mut sim, a, 2);
    sim.seconds(1.5);
    assert_eq!(held(&sim, a), Some(weapons::ASSAULT_RIFLE));
    assert_eq!(hands(&sim, a).throw.counts, [0, 0], "none on the assault rifle's rung");
    assert_eq!(hands(&sim, a).throw.selected(), None, "no plate");
    assert!(sim.app.world().resource::<MinigameHud>().rung_hint.is_none(), "a rung without a hint shows none");
    let _ = b;
}

#[test]
fn a_frag_kill_is_the_frag_grenades_and_advances_the_thrower() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    set_rung(&mut sim, a, 1);
    line_up(&mut sim, a, b, 6.0);
    let bit = grenade_bit(&sim);
    let mut killed = false;
    for i in 0..(20.0 * sim.tick_hz()) as u32 {
        let at = sim.position(b) - Vec3::Y * 0.9;
        chimera_sim::testkit::aim_at(&mut sim, a, at);
        let down = (i / 8) % 2 == 0;
        let mut intent = sim.intent(a);
        intent.actions = if down { intent.actions | bit } else { intent.actions & !bit };
        drop(intent);
        sim.ticks(1);
        if !alive(&sim, b) {
            killed = true;
            break;
        }
    }
    sim.intent(a).actions &= !bit;
    assert!(killed, "a frag killed b");
    sim.ticks(2);
    let feed = sim.app.world().resource::<chimera_client::client::kill_feed::KillFeed>();
    let last = feed.entries.last().expect("in the feed");
    assert_eq!((last.victim.entity, last.weapon), (b, Some(FRAG_RUNG)), "the frag grenade's kill");
    sim.seconds(1.2);
    assert_eq!(rung(&sim, a), 2, "the frag's kill advanced the thrower");
}

#[test]
fn the_hints_key_is_the_one_bound_now() {
    let mut c = chimera_core::console::Console::default();
    assert_eq!(rung_hint::bound_key(Some(&c), None, GRENADE_ACTION), "unbound");
    c.binds.insert("g".into(), "+grenade".into());
    assert_eq!(rung_hint::bound_key(Some(&c), None, GRENADE_ACTION), "G");
    // A rebind (the game's own layer over the shared binds too).
    c.binds.remove("g");
    c.binds.insert("q".into(), "+grenade".into());
    assert_eq!(rung_hint::bound_key(Some(&c), None, GRENADE_ACTION), "Q");
    c.layers.entry("halo_ce".into()).or_default().insert("f".into(), "+grenade".into());
    c.binds.remove("q");
    assert_eq!(rung_hint::bound_key(Some(&c), Some("halo_ce"), GRENADE_ACTION), "F");
    let v = rung_hint::RungHintView { key: "G".into(), verb: "throw".into(), ..default() };
    assert_eq!(v.text(), "Press G to throw");
}

/// The hint's place (`rung_hint::rect`, a generous 16 x 2.4 cq box)
/// clear, in both aspect families, of the party's status band and chat
/// dock (bottom left), Halo's party-layout shield and health block
/// (bottom left: the disc's 134-unit block from the safe frame's left
/// edge) and the kill feed and top-centre column (the top).
#[test]
fn the_hint_sits_bottom_centre_clear_of_the_huds() {
    use halo_ce_ui::hud::layout::{Canvas, PARTY_STATUS_BAND, SAFE_X};
    for (w, h) in [(1920.0, 1080.0), (1280.0, 720.0), (1280.0, 960.0), (1024.0, 768.0), (2560.0, 1080.0)] {
        let units = chimera_ui::UiUnits::new(chimera_ui::units::layout_height(w, h), 1.0);
        let (bw, bh) = (units.cq(16.0), units.cq(2.4));
        let r = rung_hint::rect(w, h, bw, bh);
        assert!(((r[0] + r[2]) / 2.0 - w / 2.0).abs() < 0.5, "centred: {r:?}");
        // Above the party's status band.
        assert!(r[3] <= h - units.cq(PARTY_STATUS_BAND), "{w}x{h}: above the status band: {r:?}");
        // Right of the chat's dock (4.4 + 18 cq) and Halo's block.
        assert!(r[0] >= units.cq(4.4 + 18.0), "{w}x{h}: clear of the chat: {r:?}");
        let c = Canvas::of(w, h);
        let block_right = (SAFE_X as f32 + 134.0) * c.scale;
        assert!(r[0] > block_right, "{w}x{h}: clear of Halo's shield block ({block_right}): {r:?}");
        // In the lower half: nowhere near the top's feed and column.
        assert!(r[1] > h / 2.0, "{w}x{h}: {r:?}");
    }
}

/// The frag count of `e` (what the HUD shows).
fn frags(sim: &Sim, e: Entity) -> u8 {
    hands(sim, e).throw.counts[0]
}

/// A holder on the frag rung at engine tick length `dt`, settled.
fn holder(dt: f64) -> (Sim, Entity) {
    let mut sim = party();
    sim.set_tick_interval(dt);
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    set_rung(&mut sim, a, 1);
    sim.seconds(1.5);
    assert_eq!(held(&sim, a), Some(FRAG_RUNG));
    (sim, a)
}

fn set_frags(sim: &mut Sim, e: Entity, n: u8) {
    sim.app.world_mut().get_mut::<HaloHands>(e).unwrap().throw.counts[0] = n;
}

/// Seconds (from the call) at which the count rose, over `secs`.
fn rises(sim: &mut Sim, e: Entity, secs: f64) -> Vec<f64> {
    let dt = 1.0 / sim.tick_hz();
    let (mut out, mut last) = (vec![], frags(sim, e));
    for i in 1..=(secs / dt).round() as u64 {
        sim.ticks(1);
        let n = frags(sim, e);
        assert!(n <= 4, "never above the maximum");
        if n > last {
            out.push(i as f64 * dt);
        }
        last = n;
    }
    out
}

#[test]
fn four_throws_in_quick_succession_empty_the_hand_and_nothing_gives_one_back_at_once() {
    let (mut sim, a) = holder(1.0 / 30.0);
    assert_eq!(frags(&sim, a), 4);
    let bit = grenade_bit(&sim);
    let mut most = 4;
    let mut at = None;
    for i in 0..(8.0 * sim.tick_hz()) as u32 {
        let down = (i / 8) % 2 == 0;
        let mut intent = sim.intent(a);
        intent.actions = if down { intent.actions | bit } else { intent.actions & !bit };
        drop(intent);
        sim.ticks(1);
        most = most.min(frags(&sim, a));
        if frags(&sim, a) == 0 && at.is_none() {
            at = Some(i);
        }
        if at.is_some() {
            break;
        }
    }
    sim.intent(a).actions &= !bit;
    assert_eq!((most, frags(&sim, a)), (0, 0), "four thrown, none left");
    let t = at.expect("emptied") as f64 / sim.tick_hz();
    assert!(t < 5.0, "four thrown before the first returns: {t} s");
}

#[test]
fn a_frag_returns_every_refill_seconds_up_to_the_maximum_and_no_further() {
    let (mut sim, a) = holder(1.0 / 30.0);
    set_frags(&mut sim, a, 0);
    let r = rises(&mut sim, a, 24.0);
    assert_eq!(frags(&sim, a), 4);
    assert_eq!(r.len(), 4, "four returns and no more: {r:?}");
    for (k, t) in r.iter().enumerate() {
        let want = 5.0 * (k + 1) as f64;
        assert!((t - want).abs() < 0.1, "return {k} at {t}, wanted {want}");
        // None before the interval.
    }
    // From 3 (one thrown): one interval, not from the last throw.
    set_frags(&mut sim, a, 3);
    sim.seconds(3.0);
    set_frags(&mut sim, a, 2);
    let r = rises(&mut sim, a, 11.0);
    assert_eq!(r.len(), 2, "{r:?}");
    assert!((r[0] - 2.0).abs() < 0.2 && (r[1] - 7.0).abs() < 0.2, "the interval runs from the drop below the maximum, not each throw: {r:?}");
}

#[test]
fn the_refill_times_are_the_same_at_two_tick_rates() {
    let times = |dt: f64| {
        let (mut sim, a) = holder(dt);
        set_frags(&mut sim, a, 1);
        rises(&mut sim, a, 16.0)
    };
    let (slow, fast) = (times(1.0 / 30.0), times(0.015));
    assert_eq!((slow.len(), fast.len()), (3, 3), "{slow:?} {fast:?}");
    for (s, f) in slow.iter().zip(&fast) {
        assert!((s - f).abs() < 0.07, "{slow:?} vs {fast:?}");
    }
}

#[test]
fn arriving_on_the_rung_and_respawning_give_four() {
    let (mut sim, a) = holder(1.0 / 30.0);
    assert_eq!(frags(&sim, a), 4, "on arriving");
    set_frags(&mut sim, a, 1);
    sim.seconds(2.0);
    // Away and back: the arrival gives four again.
    set_rung(&mut sim, a, 2);
    sim.seconds(1.5);
    set_rung(&mut sim, a, 1);
    sim.seconds(1.5);
    assert_eq!(frags(&sim, a), 4, "on arriving again");
    // A new life.
    set_frags(&mut sim, a, 0);
    sim.seconds(1.0);
    assert_eq!(frags(&sim, a), 0);
    sim.app.world_mut().get_mut::<chimera_core::core::Health>(a).unwrap().current = 0.0;
    sim.seconds(0.5);
    sim.app.world_mut().get_mut::<chimera_core::core::Health>(a).unwrap().current = 100.0;
    sim.seconds(0.5);
    assert_eq!(frags(&sim, a), 4, "on respawning");
}
