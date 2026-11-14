//! The rank-up toast (`client::rank_toast`, the Signal look): a rung up
//! of the local player shows the toast under the chip for about 1.15 s,
//! the number rolling up (old out the top, new from below) with the
//! chip's own rung and tick in step, the gun's short name over "CLASS •
//! GAME" and the next gun; a second rung up within the hold chains (the
//! toast stays and rolls again); under the shell's default theme there is
//! none and the rung-up callout keeps its text, which the Signal look
//! leaves to the toast.

use bevy::prelude::*;
use chimera_client::client::{
    callouts::{CalloutView, CalloutsPlugin},
    console::HeldActions,
    minigame_scoreboard::{GunNames, MinigameScoreboardPlugin},
    rank_toast::{GunSlot, NextSlot, RankEvent, RankToast, RankToastRoot, Roll, RollNew, RollOld, TickPop, ToastTimes, pose},
};
use chimera_core::core::{LocalPlayer, Score};
use chimera_sim::{
    SimBasePlugins,
    minigame::{
        hud::{Callout, HudLine, MinigameHud, ShownCallout},
        ladder::{Ladder, Rung},
        runtime::InMinigame,
    },
    weapon::data::WeaponDataAppExt,
};
use chimera_ui::{Theme, UiClock};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

const SIGNAL: &str = "name = \"test\"\n[flags.hud]\nsignal = true\n";

fn sim(signal: bool) -> (Sim, Entity) {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins((GreyboxMapPlugin, MinigameScoreboardPlugin, CalloutsPlugin));
        app.add_weapon_data(&[
            ("p.toml", "id = \"kit:weapon_pistol\"\nslot = 1\ndraw = 0.5\nclass = \"pistol\"\n"),
            ("r.toml", "id = \"kit:weapon_rifle\"\nslot = 0\ndraw = 0.5\nclass = \"carbine\"\n"),
            ("s.toml", "id = \"other:weapon_shotgun\"\nslot = 0\ndraw = 0.5\nclass = \"shotgun\"\n"),
        ])
        .expect("the data reads");
    });
    if signal {
        sim.app.insert_resource(Theme::from_toml(SIGNAL).expect("theme"));
    }
    sim.app.insert_resource(UiClock::manual());
    sim.app.init_resource::<HeldActions>();
    sim.app.insert_resource(Ladder {
        rungs: vec!["kit:weapon_pistol", "kit:weapon_rifle", "other:weapon_shotgun", "kit:weapon_pistol"],
        ..default()
    });
    sim.app.world_mut().insert_resource(GunNames::default());
    sim.app.world_mut().insert_resource(MinigameHud { title: "Gun game".into(), ..default() });
    let me = sim
        .app
        .world_mut()
        .spawn((InMinigame, LocalPlayer, Name::new("Alpha"), Rung(0), Score { kills: 1, deaths: 0 }))
        .id();
    sim.app.world_mut().spawn((InMinigame, Name::new("Bob"), Rung(0), Score::default()));
    for _ in 0..3 {
        frame(&mut sim, 1.0 / 60.0);
    }
    (sim, me)
}

fn frame(sim: &mut Sim, dt: f32) {
    sim.app.world_mut().resource_mut::<UiClock>().step(dt);
    sim.app.update();
}

/// `secs` of frames at 60 Hz.
fn run(sim: &mut Sim, secs: f32) {
    let n = (secs * 60.0).round() as usize;
    for _ in 0..n {
        frame(sim, 1.0 / 60.0);
    }
}

fn toast_up(sim: &mut Sim) -> bool {
    let w = sim.app.world_mut();
    w.query_filtered::<&Visibility, With<RankToastRoot>>().iter(w).any(|v| *v != Visibility::Hidden)
}

fn rung_up(sim: &mut Sim, me: Entity, to: usize) {
    sim.app.world_mut().entity_mut(me).insert(Rung(to));
}

/// Texts under the toast's gun slots and next slot.
fn toast_texts(sim: &mut Sim) -> Vec<String> {
    let w = sim.app.world_mut();
    let mut stack: Vec<Entity> = w.query_filtered::<Entity, Or<(With<GunSlot>, With<NextSlot>)>>().iter(w).collect();
    let mut out = Vec::new();
    while let Some(e) = stack.pop() {
        if let Some(t) = w.get::<Text>(e) {
            out.push(t.0.clone());
        }
        if let Some(c) = w.get::<Children>(e) {
            stack.extend(c.iter());
        }
    }
    out
}

/// The toast's (not the chip's) rolling digits: (old text, old y, new
/// text, new y).
fn digits(sim: &mut Sim, chip: bool) -> Vec<(String, f32, bool)> {
    let w = sim.app.world_mut();
    let rolls: Vec<(Roll, Vec<Entity>)> =
        w.query::<(&Roll, &Children)>().iter(w).filter(|(r, _)| r.chip == chip).map(|(r, c)| (*r, c.iter().collect())).collect();
    let mut out = Vec::new();
    for (_, children) in rolls {
        for c in children {
            let e = w.entity(c);
            let (Some(t), Some(tf)) = (e.get::<Text>(), e.get::<UiTransform>()) else { continue };
            let y = match tf.translation.y {
                Val::Px(v) => v,
                _ => 0.0,
            };
            out.push((t.0.clone(), y, e.contains::<RollOld>()));
        }
    }
    let _ = RollNew;
    out
}

#[test]
fn rank_toast_rolls_and_holds_1_15_s() {
    let (mut sim, me) = sim(true);
    assert!(!toast_up(&mut sim), "no toast before a rung up");
    rung_up(&mut sim, me, 1);
    frame(&mut sim, 0.0);
    run(&mut sim, 0.05);
    assert!(toast_up(&mut sim), "the toast wipes in");

    // Mid-roll: the old number leaving upward, the new one rising.
    run(&mut sim, 0.15);
    let d = digits(&mut sim, false);
    let old = d.iter().find(|x| x.2).expect("the old number");
    let new = d.iter().find(|x| !x.2).expect("the new number");
    assert_eq!((old.0.as_str(), new.0.as_str()), ("01", "02"), "{d:?}");
    assert!(new.1 > 0.5, "the new number rises from below: {d:?}");
    // The chip's own rung rolls in step, its tick pops.
    let chip = digits(&mut sim, true);
    assert!(chip.iter().any(|x| !x.2 && x.0 == "02" && x.1 > 0.5), "the chip's rung rolls too: {chip:?}");
    {
        let w = sim.app.world_mut();
        let pops: Vec<(Visibility, f32)> = w.query::<(&Visibility, &UiTransform, &TickPop)>().iter(w).map(|(v, t, _)| (*v, t.scale.y)).collect();
        assert!(pops.iter().any(|(v, s)| *v != Visibility::Hidden && *s > 1.2), "the chip's tick pops: {pops:?}");
    }
    let texts = toast_texts(&mut sim);
    for want in ["RIFLE", "CARBINE • KIT", "SHOTGUN"] {
        assert!(texts.iter().any(|t| t == want), "{want} in {texts:?}");
    }

    // Settled: the new number in place, held to 0.92 s.
    run(&mut sim, 0.5);
    let d = digits(&mut sim, false);
    assert!(d.iter().any(|x| !x.2 && x.0 == "02" && x.1.abs() < 0.5), "settled: {d:?}");
    assert!(toast_up(&mut sim), "held");
    // Gone by 1.16 s.
    run(&mut sim, 0.5);
    assert!(!toast_up(&mut sim), "gone after its 1.15 s");

    // A rung down shows nothing.
    rung_up(&mut sim, me, 0);
    run(&mut sim, 0.3);
    assert!(!toast_up(&mut sim), "a rung down shows no toast");
}

#[test]
fn rank_toast_chains_within_the_hold() {
    let (mut sim, me) = sim(true);
    rung_up(&mut sim, me, 1);
    run(&mut sim, 0.5);
    rung_up(&mut sim, me, 2);
    frame(&mut sim, 0.0);
    run(&mut sim, 0.2);
    let d = digits(&mut sim, false);
    assert!(d.iter().any(|x| x.2 && x.0 == "02") && d.iter().any(|x| !x.2 && x.0 == "03"), "rolls again 02 to 03: {d:?}");
    // Past the first one's 1.16 s: still up, the chain held from the second.
    run(&mut sim, 0.6);
    assert!(toast_up(&mut sim), "the chain keeps the toast up");
    let texts = toast_texts(&mut sim);
    assert!(texts.iter().any(|t| t == "SHOTGUN") && texts.iter().any(|t| t == "SHOTGUN • OTHER"), "{texts:?}");
    run(&mut sim, 0.5);
    assert!(!toast_up(&mut sim), "gone 1.16 s after the last rung up");
}

#[test]
fn the_pose_follows_the_look_book() {
    let d = ToastTimes::default();
    let ev = |at: f64, from: usize| RankEvent { at, from, to: from + 1, gun: default(), next: None };
    let one = [ev(0.0, 5)];
    assert!(pose(one.iter().collect(), -0.01, &d).is_none());
    let p = pose(one.iter().collect(), 0.1, &d).unwrap();
    assert!(p.enter > 0.9 && p.exit == 0.0 && p.rolling());
    let p = pose(one.iter().collect(), 0.3, &d).unwrap();
    assert!(p.new_y < 0.0, "the roll overshoots: {}", p.new_y);
    assert!(pose(one.iter().collect(), 1.17, &d).is_none(), "1.16 s in all");
    // A rung up 1 s after: still leaving, so it chains; 1.2 s after: anew.
    let chain = [ev(0.0, 5), ev(1.0, 6)];
    let p = pose(chain.iter().collect(), 1.05, &d).unwrap();
    assert_eq!((p.k, p.j), (1, 0));
    assert!(p.old_gun.is_some() && p.enter == 1.0);
    let anew = [ev(0.0, 5), ev(1.2, 6)];
    let p = pose(anew.iter().collect(), 1.25, &d).unwrap();
    assert_eq!((p.k, p.j), (1, 1));
    assert!(p.enter < 1.0 && p.old_gun.is_none());
}

#[test]
fn rank_toast_needs_the_signal_theme_and_takes_the_rung_up_callout() {
    for signal in [false, true] {
        let (mut sim, me) = sim(signal);
        rung_up(&mut sim, me, 1);
        let up = Callout::new(2, "rung_up", vec![HudLine::new("Level up: {weapon}").weapon("kit:weapon_rifle")]);
        sim.app.world_mut().resource_mut::<MinigameHud>().callout = Some(ShownCallout { callout: up, since: 0.0, id: 1 });
        run(&mut sim, 0.3);
        assert_eq!(toast_up(&mut sim), signal, "the toast only under the Signal look");
        let view = sim.app.world().resource::<CalloutView>().clone();
        assert_eq!(view.lines.is_empty(), signal, "the callout's text only without it: {:?}", view.lines);
        assert_eq!(view.heard, ["rung_up"], "its cue either way");
        let _ = sim.app.world().resource::<RankToast>();
    }
}
