//! The logo pass between screens (`client::logo_pass`): under a theme
//! with a pass, a move runs at the swap (600 ms of 1.2 s) while the ink
//! slab covers the whole window (16:9, 21:9, 4:3), two asks in one lead
//! share one swap, a keyed ask isn't taken twice; reduced motion fades in
//! 150 ms with the swap at 75; the lobby's end waits for the swap with
//! the lobby still up; the match end comes up at a swap and the map
//! change runs under a pass begun a swap before it; without the theme
//! every move happens at once. The clock is stepped by hand
//! (`UiClock::manual`).

use bevy::{ecs::system::RunSystemOnce, prelude::*};
use chimera_client::client::{
    logo_pass::{self, LogoPass, LogoPassPlugin, PassCovered, Passes, ReduceMotion, covers},
    match_end::{MatchEndGate, MatchEndPlugin, MatchEndView},
    prematch_lobby::{LobbyView, PreMatchLobbyPlugin},
};
use chimera_core::core::{LocalPlayer, NetRole, SimClock};
use chimera_net::{
    net::HOST_ID,
    prematch::{LobbyRow, NetLobby, status},
};
use chimera_sim::{
    SimBasePlugins,
    minigame::{
        NextMatch,
        hud::MinigameHud,
        results::{MatchResults, ResultRow},
    },
};
use chimera_ui::{ChimeraUiPlugin, Theme, UiClock};
use chimera_world::harness::Sim;

/// The pass's numbers (the Ultimate Gun Game's).
const THEME: &str = r#"
name = "test"
[shape]
slant_deg = 12.0
min_text_px = 9.0
[motion.dur.pass]
total = 1.2
swap = 0.6
mark_in_at = 0.33
mark_in = 0.16
mark_out_at = 0.70
mark_out = 0.12
[layout.pass]
band_acc_w = 1.1
band_acc_x = -2.6
band_ink_w = 140.0
band_ink_x = 0.0
band_acc2_w = 10.0
band_acc2_x = 141.0
band_fg_w = 5.0
band_fg_x = 152.0
from = -175.0
travel = 287.0
mark_w = 13.0
mark_h = 9.33
mark_x = 81.5
"#;

fn sim(theme: Option<&str>) -> Sim {
    let theme = theme.map(|t| Theme::from_toml(t).unwrap());
    let mut sim = Sim::with(SimBasePlugins, move |app| {
        if let Some(t) = theme {
            app.insert_resource(t);
        }
        app.add_plugins((ChimeraUiPlugin, LogoPassPlugin, PreMatchLobbyPlugin)).insert_resource(UiClock::manual());
        if !app.world().contains_resource::<Assets<Image>>() {
            app.init_asset::<Image>();
        }
    });
    sim.ticks(1);
    sim
}

fn step(sim: &mut Sim, secs: f32) {
    sim.app.world_mut().resource_mut::<UiClock>().step(secs);
    sim.ticks(1);
}

/// The keys swapped so far, in order.
#[derive(Resource, Default)]
struct Swapped(Vec<&'static str>);

fn record(sim: &mut Sim) {
    sim.app.init_resource::<Swapped>().add_observer(|ev: On<PassCovered>, mut s: ResMut<Swapped>| s.0.extend(ev.keys.iter().copied()));
}

/// Asks for `key` through the pass: whether it waits for the swap.
fn ask(sim: &mut Sim, key: &'static str) -> bool {
    sim.app.world_mut().run_system_once(move |mut p: Passes| p.ask(key)).unwrap()
}

fn swapped(sim: &Sim) -> Vec<&'static str> {
    sim.app.world().resource::<Swapped>().0.clone()
}

#[test]
fn logo_pass_swaps_at_half_while_covered() {
    let theme = Theme::from_toml(THEME).unwrap();
    for aspect in [16.0 / 9.0, 21.0 / 9.0, 4.0 / 3.0] {
        assert!(covers(&theme, 0.5, aspect), "covered at the swap at {aspect}");
        assert!(!covers(&theme, 0.0, aspect) && !covers(&theme, 1.0, aspect), "open at both ends at {aspect}");
    }
    let mut s = sim(Some(THEME));
    record(&mut s);
    for key in ["a", "b", "once", "once"] {
        assert!(ask(&mut s, key), "{key} waits for the swap");
    }
    step(&mut s, 0.0);
    assert_eq!(s.app.world().resource::<LogoPass>().runs, 1, "one pass for asks in one lead");
    step(&mut s, 0.59);
    assert!(swapped(&s).is_empty(), "nothing swaps while the slab comes");
    step(&mut s, 0.02);
    assert_eq!(swapped(&s), ["a", "b", "once"], "all at the swap, a key taken once");
    assert!(s.app.world().resource::<LogoPass>().active(), "the trailing edge still reveals");
    // Asked while the slab still covers: covered at once, no new pass.
    assert!(ask(&mut s, "late"));
    s.ticks(1);
    assert_eq!(swapped(&s), ["a", "b", "once", "late"]);
    step(&mut s, 0.6);
    assert!(!s.app.world().resource::<LogoPass>().active());
    let lp = s.app.world().resource::<LogoPass>();
    assert_eq!((lp.runs, lp.swaps), (1, 1));
}

#[test]
fn reduced_motion_fades_in_150_ms() {
    let mut s = sim(Some(THEME));
    s.app.insert_resource(ReduceMotion { on: 1 });
    record(&mut s);
    ask(&mut s, "a");
    step(&mut s, 0.0);
    assert!(s.app.world().resource::<LogoPass>().fading());
    step(&mut s, 0.07);
    assert!(swapped(&s).is_empty());
    step(&mut s, 0.01);
    assert_eq!(swapped(&s), ["a"], "the swap at 75 ms");
    step(&mut s, 0.08);
    assert!(!s.app.world().resource::<LogoPass>().active());
}

#[test]
fn without_the_theme_moves_happen_at_once() {
    let mut s = sim(None);
    assert!(!ask(&mut s, "a"), "the caller moves at once");
    assert_eq!(s.app.world().resource::<LogoPass>().runs, 0);
}

#[test]
fn the_lobby_stays_up_until_the_swap() {
    let mut s = sim(Some(THEME));
    s.app.insert_resource(NetRole::Server);
    s.app.world_mut().spawn(LocalPlayer);
    let lobby = s
        .app
        .world_mut()
        .spawn(NetLobby {
            map: "cs_source:de_dust2".into(),
            mode: "Ultimate Gun Game".into(),
            rows: vec![LobbyRow { id: Some(HOST_ID), name: "kestrel".into(), status: status::READY, ..default() }],
            countdown: Some(1),
            ..default()
        })
        .id();
    // Its coming up is a pass too: shown at that pass's swap.
    step(&mut s, 0.0);
    assert!(!s.app.world().resource::<LobbyView>().shown, "not before the swap");
    step(&mut s, 0.61);
    assert!(s.app.world().resource::<LobbyView>().shown);
    step(&mut s, 0.7);
    assert!(!s.app.world().resource::<LogoPass>().active());
    // The match starts: the lobby is gone from the server.
    s.app.world_mut().despawn(lobby);
    step(&mut s, 0.0);
    assert!(s.app.world().resource::<LobbyView>().shown, "still up as the slab comes");
    assert!(s.app.world().resource::<LogoPass>().leading());
    step(&mut s, 0.61);
    assert!(!s.app.world().resource::<LobbyView>().shown, "gone at the swap");
    assert_eq!(s.app.world().resource::<LogoPass>().runs, 2);
}

/// The match end comes up at a pass's swap; the map change after it runs
/// under a second pass begun a swap's length before the change, so the
/// slab covers it.
#[test]
fn the_map_change_runs_under_the_pass() {
    let theme = Theme::from_toml(THEME).unwrap();
    let mut s = Sim::with(SimBasePlugins, move |app| {
        app.insert_resource(theme)
            .add_plugins((ChimeraUiPlugin, LogoPassPlugin, MatchEndPlugin))
            .insert_resource(UiClock::manual());
        if !app.world().contains_resource::<Assets<Image>>() {
            app.init_asset::<Image>();
        }
    });
    s.ticks(1);
    let ann = s.app.world_mut().spawn(Name::new("Ann")).id();
    let results = MatchResults {
        title: "Kit gun game".into(),
        rungs: 3,
        rows: vec![ResultRow {
            name: "Ann".into(),
            kills: 7,
            deaths: 2,
            rung: Some(1),
            winner: true,
            player: Some(ann),
            gun: None,
            wins: 1,
        }],
    };
    s.app.insert_resource(MinigameHud { title: "Kit gun game".into(), results: Some(results), ..default() });
    let now = s.app.world().resource::<SimClock>().now;
    s.app.insert_resource(NextMatch { map: Some("cs_source:de_dust2".into()), at: now + 5.0 });
    step(&mut s, 0.0);
    assert!(!s.app.world().resource::<MatchEndView>().shown, "not before the swap");
    assert!(s.app.world().resource::<LogoPass>().pending("match_end"));
    step(&mut s, 0.61);
    assert!(s.app.world().resource::<MatchEndView>().shown, "up at the swap");
    step(&mut s, 0.7);
    assert!(!s.app.world().resource::<LogoPass>().active());
    assert_eq!(*s.app.world().resource::<MatchEndGate>(), MatchEndGate::In);
    // The change comes within a swap's length: the second pass leads.
    let now = s.app.world().resource::<SimClock>().now;
    s.app.world_mut().resource_mut::<NextMatch>().at = now + 0.5;
    step(&mut s, 0.0);
    let lp = s.app.world().resource::<LogoPass>();
    assert!(lp.pending("next_match"), "the slab comes before the change");
    assert_eq!(lp.runs, 2);
    assert_eq!(*s.app.world().resource::<MatchEndGate>(), MatchEndGate::Changing);
    // Asked once: the screen stays up under it.
    step(&mut s, 0.1);
    assert_eq!(s.app.world().resource::<LogoPass>().runs, 2);
    assert!(s.app.world().resource::<MatchEndView>().shown);
    // The next match: the screen goes with the results.
    s.app.world_mut().remove_resource::<NextMatch>();
    s.app.world_mut().resource_mut::<MinigameHud>().results = None;
    step(&mut s, 0.6);
    assert!(!s.app.world().resource::<MatchEndView>().shown);
    assert_eq!(*s.app.world().resource::<MatchEndGate>(), MatchEndGate::Off);
}

/// `ui_pass <p>` holds a pass still for screenshots.
#[test]
fn ui_pass_holds_for_screenshots() {
    let mut s = sim(Some(THEME));
    logo_pass::hold(s.app.world_mut(), 0.5);
    step(&mut s, 5.0);
    let lp = s.app.world().resource::<LogoPass>();
    assert_eq!(lp.elapsed(0.0), Some(0.6), "held at the swap");
    assert!(!lp.leading());
}
