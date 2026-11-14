//! The match's effects on screen (`client::match_effects`): under the
//! Signal look an air strike's banner opens over the crosshair and folds
//! into its chip in the top-left stack after about 2 s, the screen's edges
//! pulse red on each second of the countdown (never a fill), the chip
//! counts down to the impact; effects' chips stack (threats first) and
//! slide back as one leaves; without the Signal look the plain banner
//! says it as the host's HUD text did, and no chips show.

use bevy::prelude::*;
use chimera_client::client::match_effects::{
    BannerClip, BannerRoot, ChipText, EdgePulse, EffectChip, MatchEffectsPlugin, PlainText, edge_opacity, plain_line, ShownEffect,
};
use chimera_core::core::SimClock;
use chimera_sim::{
    SimBasePlugins,
    minigame::effects::{Effect, MatchEffects},
};
use chimera_ui::{Theme, UiClock, UiUnits};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

const SIGNAL: &str = "name = \"test\"\n[flags.hud]\nsignal = true\n";

fn sim(signal: bool) -> Sim {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins((GreyboxMapPlugin, MatchEffectsPlugin));
    });
    if signal {
        sim.app.insert_resource(Theme::from_toml(SIGNAL).expect("theme"));
    }
    sim.app.insert_resource(UiClock::manual());
    sim.app.init_resource::<MatchEffects>();
    frame(&mut sim, 0.0);
    sim
}

fn frame(sim: &mut Sim, dt: f32) {
    sim.app.world_mut().resource_mut::<UiClock>().step(dt);
    sim.app.update();
}

fn run(sim: &mut Sim, secs: f32) {
    for _ in 0..(secs * 60.0).round() as usize {
        frame(sim, 1.0 / 60.0);
    }
}

fn strike(now: f64) -> Effect {
    Effect {
        kind: "air_strike".into(),
        title: "Air strike".into(),
        detail: "Take cover".into(),
        threat: true,
        color: [255, 48, 32],
        starts: now,
        impact_at: Some(now + 3.0),
        ends: now + 5.0,
        ..default()
    }
}

fn moon(now: f64, secs: f64) -> Effect {
    Effect {
        kind: "moon_gravity".into(),
        title: "Moon gravity".into(),
        detail: "Jump higher • fall slower".into(),
        countdown: "Gravity returns in {n}".into(),
        over: "Gravity is back!".into(),
        color: [50, 110, 255],
        starts: now,
        ends: now + secs,
        ..default()
    }
}

fn start(sim: &mut Sim, e: Effect) {
    sim.app.world_mut().resource_mut::<MatchEffects>().start(e);
}

fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

/// How far open the banner is (0-1).
fn banner_w(sim: &mut Sim) -> f32 {
    let w = sim.app.world_mut();
    let up = w.query_filtered::<&Visibility, With<BannerRoot>>().single(w).unwrap() != &Visibility::Hidden;
    let open = w.query::<&BannerClip>().single(w).unwrap().0;
    if up { open } else { 0.0 }
}

fn edge(sim: &mut Sim) -> f32 {
    let w = sim.app.world_mut();
    w.query::<&EdgePulse>().single(w).unwrap().0
}

/// The chips: (id, slot, y px, width px).
fn chips(sim: &mut Sim) -> Vec<(u32, usize, f32, f32)> {
    let w = sim.app.world_mut();
    let mut v: Vec<_> = w
        .query::<(&EffectChip, &UiTransform, &Node)>()
        .iter(w)
        .map(|(c, t, n)| {
            let y = match t.translation.y {
                Val::Px(v) => v,
                _ => 0.0,
            };
            let wd = match n.width {
                Val::Px(v) => v,
                _ => 0.0,
            };
            (c.id, c.slot, y, wd)
        })
        .collect();
    v.sort_by_key(|c| c.1);
    v
}

fn chip_line(sim: &mut Sim) -> String {
    let w = sim.app.world_mut();
    let mut parts: Vec<(ChipText, String)> = w.query::<(&ChipText, &Text)>().iter(w).map(|(c, t)| (*c, t.0.clone())).collect();
    parts.sort_by_key(|p| p.0 as u8);
    parts.into_iter().map(|p| p.1).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
}

#[test]
fn air_strike_banner_folds_into_its_chip() {
    let mut sim = sim(true);
    let t0 = now(&sim);
    start(&mut sim, strike(t0));
    frame(&mut sim, 0.0);
    run(&mut sim, 0.2);
    assert!(banner_w(&mut sim) < 0.01, "the banner opens at 0.25 s");
    run(&mut sim, 0.5);
    let open = banner_w(&mut sim);
    assert!(open > 0.95, "open from the centre: {open}");
    assert!(chips(&mut sim).is_empty(), "no chip while the banner is up");
    // The edge pulses on each second of the countdown (impact at 3 s:
    // at 0, 1 and 2 s), over a base of 0.2.
    run(&mut sim, 0.35);
    let peak = edge(&mut sim);
    run(&mut sim, 0.5);
    let low = edge(&mut sim);
    assert!(peak > 0.7 && low < 0.5 && low >= 0.2, "a pulse at 1 s: {peak} then {low}");

    // At 2 s it folds into its chip.
    run(&mut sim, 0.85);
    assert!(banner_w(&mut sim) < 0.01, "folded by 2.28 s");
    let c = chips(&mut sim);
    assert_eq!(c.len(), 1, "its chip: {c:?}");
    assert!(c[0].3 > 100.0, "wiped in: {c:?}");
    assert_eq!(chip_line(&mut sim), "IMPACT IN 1");
    run(&mut sim, 0.8);
    assert_eq!(chip_line(&mut sim), "IMPACT • STAY DOWN");
    // Its end (5 s): the chip goes.
    run(&mut sim, 2.0);
    assert!(chips(&mut sim).is_empty(), "gone with the strike: {:?}", chips(&mut sim));
    run(&mut sim, 2.5);
    assert_eq!(edge(&mut sim), 0.0);
}

#[test]
fn effects_chips_stack_and_leave() {
    let mut sim = sim(true);
    let t0 = now(&sim);
    start(&mut sim, moon(t0, 30.0));
    run(&mut sim, 2.8);
    let c = chips(&mut sim);
    assert_eq!(c.len(), 1, "moon gravity's chip: {c:?}");
    assert_eq!(edge(&mut sim), 0.0, "no edge for a harmless effect");
    let moon_id = c[0].0;
    let pitch = UiUnits::default().cq(3.05);
    // An air strike: its chip takes the first slot, the moon's slides down.
    let t1 = now(&sim);
    start(&mut sim, strike(t1));
    run(&mut sim, 2.8);
    let c = chips(&mut sim);
    assert_eq!(c.len(), 2, "{c:?}");
    assert!(c[0].0 != moon_id && c[1].0 == moon_id, "the threat first: {c:?}");
    assert!((c[1].2 - pitch).abs() < 1.0, "the moon one slot down: {c:?}");
    // The strike over: the moon's chip slides back up.
    run(&mut sim, 3.0);
    let c = chips(&mut sim);
    assert_eq!(c.len(), 1, "{c:?}");
    assert!(c[0].2.abs() < 1.0, "back in the first slot: {c:?}");
}

#[test]
fn the_plain_look_says_it_as_text() {
    let mut sim = sim(false);
    let t0 = now(&sim);
    start(&mut sim, strike(t0));
    run(&mut sim, 0.5);
    {
        let w = sim.app.world_mut();
        let (t, v) = w.query_filtered::<(&Text, &Visibility), With<PlainText>>().single(w).unwrap();
        assert_eq!(t.0, "AIR STRIKE INCOMING!\nTAKE COVER!");
        assert_ne!(*v, Visibility::Hidden);
    }
    assert!(banner_w(&mut sim) < 0.01 && edge(&mut sim) == 0.0, "no Signal banner or edge");
    run(&mut sim, 2.5);
    assert!(chips(&mut sim).is_empty(), "no chips");

    // Moon gravity's lines: the start, the countdown, the end.
    let shown = |t: f32, secs: f64| plain_line(&ShownEffect { e: moon(0.0, secs), t0: 0.0 }, t).map(|l| l.0);
    assert_eq!(shown(0.5, 60.0).as_deref(), Some("Moon gravity!\nfor 60 seconds"));
    assert_eq!(shown(30.0, 60.0), None);
    assert_eq!(shown(57.5, 60.0).as_deref(), Some("Gravity returns in 3"));
    assert_eq!(shown(61.0, 60.0).as_deref(), Some("Gravity is back!"));
    let mut stopped = moon(0.0, 20.0);
    stopped.stopped = true;
    assert_eq!(plain_line(&ShownEffect { e: stopped, t0: 0.0 }, 21.0), None, "no end line when stopped");
}

#[test]
fn the_edge_pulses_each_second() {
    // Look book: impact at 4.2 s, pulses at .2, 1.2, 2.2, 3.2 s.
    for tp in [0.2, 1.2, 2.2, 3.2] {
        let at = edge_opacity(tp + 0.08, 4.2, 6.0);
        let before = edge_opacity(tp - 0.01, 4.2, 6.0);
        assert!(at > 0.85 && at > before + 0.3, "a pulse at {tp}: {before} then {at}");
    }
    // 80 ms attack, 320 ms decay.
    let a = edge_opacity(1.2 + 0.04, 4.2, 6.0);
    let d = edge_opacity(1.2 + 0.08 + 0.32, 4.2, 6.0);
    assert!((a - (0.2 + 0.68 * 0.5)).abs() < 0.02, "halfway up at 40 ms: {a}");
    assert!((d - (0.2 + 0.68 / std::f32::consts::E)).abs() < 0.02, "1/e after 320 ms: {d}");
    assert_eq!(edge_opacity(4.2 + 0.08, 4.2, 6.0), 1.0, "the impact's 1.3x, capped");
    assert_eq!(edge_opacity(9.0, 4.2, 6.0), 0.0);
}
