//! A minigame's callouts in the shell (`client::callouts`): hidden with
//! none; the minigame's callout centre screen with `{player}` and
//! `{weapon}` filled in (the player's name, the gun's display name); its
//! cue sounded once, from the first registered cue pack; a cue the pack
//! lacks is silent; gone with the callout.

use bevy::prelude::*;
use chimera_client::client::callouts::{CalloutLine, CalloutRoot, CalloutView, CalloutsPlugin};
use chimera_core::cues::CueAppExt;
use chimera_sim::{
    SimBasePlugins,
    minigame::hud::{Callout, HudLine, MinigameHud, ShownCallout},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

const PACK: &str = r#"
id = "kit"
name = "Kit"
game = "kit"
[sounds]
rung_up = "up.wav"
"#;

fn shown(sim: &mut Sim) -> Option<Vec<String>> {
    let w = sim.app.world_mut();
    let v = *w.query_filtered::<&Visibility, With<CalloutRoot>>().single(w).unwrap();
    let mut lines: Vec<(usize, String, Visibility)> =
        w.query::<(&CalloutLine, &Text, &Visibility)>().iter(w).map(|(l, t, v)| (l.0, t.0.clone(), *v)).collect();
    lines.sort_by_key(|l| l.0);
    (v != Visibility::Hidden).then(|| lines.into_iter().filter(|l| l.2 != Visibility::Hidden).map(|l| l.1).collect())
}

fn set(sim: &mut Sim, callout: Option<Callout>, id: u32) {
    let now = sim.app.world().resource::<Time>().elapsed_secs_f64();
    sim.app.world_mut().resource_mut::<MinigameHud>().callout = callout.map(|callout| ShownCallout { callout, since: now, id });
}

#[test]
fn a_callout_shows_named_and_sounds_its_cue_once() {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins((GreyboxMapPlugin, CalloutsPlugin)).init_resource::<MinigameHud>();
        app.register_cues(PACK);
    });
    let other = sim.app.world_mut().spawn(Name::new("Bob")).id();
    sim.ticks(2);
    assert_eq!(shown(&mut sim), None, "none");

    let up = Callout::new(2, "rung_up", vec![HudLine::new("Level up: {weapon}").weapon("kit:rifle"), HudLine::new("Level 2/3")]);
    set(&mut sim, Some(up), 1);
    sim.ticks(3);
    let lines = shown(&mut sim).expect("shown");
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[0], "Level up: RIFLE", "no data names it: the id's last part");
    assert_eq!(lines[1], "Level 2/3");
    let view = sim.app.world().resource::<CalloutView>().clone();
    assert_eq!(view.heard, ["rung_up"], "once, though shown for several frames");
    assert_eq!(view.to_play.len(), 1);
    assert_eq!((view.to_play[0].0.id.as_str(), view.to_play[0].1.as_str()), ("kit", "up.wav"));

    // Another player's last rung: named; a cue the pack lacks is silent.
    let last = Callout::new(4, "last_rung_other", vec![HudLine::new("{player} is on the last level!").player(other)]);
    set(&mut sim, Some(last), 2);
    sim.ticks(2);
    assert_eq!(shown(&mut sim), Some(vec!["Bob is on the last level!".to_string()]));
    let view = sim.app.world().resource::<CalloutView>().clone();
    assert_eq!(view.heard, ["rung_up", "last_rung_other"]);
    assert_eq!(view.to_play.len(), 1, "no file for it");

    set(&mut sim, None, 2);
    sim.ticks(2);
    assert_eq!(shown(&mut sim), None, "gone with the callout");
}

/// The callout is sized for a 720-line screen (its first line big) and
/// grows with the window, as the match-end screen does: at 1440 lines
/// twice the size, and again when the window changes.
#[test]
fn the_callout_scales_with_the_windows_height() {
    use bevy::window::{PrimaryWindow, WindowResolution};
    use chimera_client::client::callouts::line_size;
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins((GreyboxMapPlugin, CalloutsPlugin)).init_resource::<MinigameHud>();
    });
    let window = sim
        .app
        .world_mut()
        .spawn((Window { resolution: WindowResolution::from((2560u32, 1440u32)), ..default() }, PrimaryWindow))
        .id();
    sim.ticks(2);
    let sizes = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        let mut s: Vec<(usize, FontSize)> = w.query::<(&CalloutLine, &TextFont)>().iter(w).map(|(l, f)| (l.0, f.font_size)).collect();
        s.sort_by_key(|l| l.0);
        s
    };
    let s = sizes(&mut sim);
    assert_eq!(s[0].1, FontSize::Px(line_size(0) * 2.0), "the big line");
    assert_eq!(s[1].1, FontSize::Px(line_size(1) * 2.0));
    assert!(line_size(0) > line_size(1));
    sim.app.world_mut().get_mut::<Window>(window).unwrap().resolution = WindowResolution::from((1280u32, 720u32));
    sim.ticks(2);
    assert_eq!(sizes(&mut sim)[0].1, FontSize::Px(line_size(0)));
}

/// The lines' drawn sizes at a known resolution: 33 and 21 px on 1080 lines,
/// 44 and 28 on 1440 (the playtest's "far too big": the first line was 60 on
/// 1080, twice Combat Arms' own callout's), so at most 3.1 % of the height.
#[test]
fn a_callouts_text_is_a_small_share_of_the_screen() {
    use bevy::{text::FontSize, window::{PrimaryWindow, WindowResolution}};
    for (w, h, first, rest) in [(1920u32, 1080u32, 33.0, 21.0), (2560, 1440, 44.0, 28.0), (1280, 720, 22.0, 14.0)] {
        let mut sim = Sim::with(SimBasePlugins, |app| {
            app.add_plugins((GreyboxMapPlugin, CalloutsPlugin)).init_resource::<MinigameHud>();
        });
        sim.app.world_mut().spawn((Window { resolution: WindowResolution::new(w, h), ..default() }, PrimaryWindow));
        sim.ticks(2);
        let world = sim.app.world_mut();
        let sizes: Vec<(usize, f32)> = world
            .query::<(&CalloutLine, &TextFont)>()
            .iter(world)
            .map(|(l, f)| match f.font_size {
                FontSize::Px(px) => (l.0, px),
                ref other => panic!("not in pixels: {other:?}"),
            })
            .collect();
        for (line, px) in sizes {
            let want = if line == 0 { first } else { rest };
            assert!((px - want).abs() < 0.01, "{w}x{h} line {line}: {px} px");
            assert!(px / h as f32 <= 0.031, "{w}x{h} line {line}: {px} px is too big");
        }
    }
}

/// A kill of the local player's whose character said no kill line
/// (`kill_voice::KillVoiceOutcome`, unvoiced) sounds the pack's rung-up
/// cue once, at its own gain (`[volumes]`); a voiced kill, another
/// player's kill, and one whose rung up sounds its cue, don't.
#[test]
fn an_unvoiced_kill_sounds_the_rung_up_cue() {
    use chimera_client::client::callouts::{CHIME_WAIT, KILL_CHIME};
    use chimera_core::core::LocalPlayer;
    use chimera_sim::kill_voice::KillVoiceOutcome;
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins((GreyboxMapPlugin, CalloutsPlugin)).init_resource::<MinigameHud>();
        app.register_cues(&format!("{PACK}[volumes]\nrung_up = 1.8\n"));
    });
    let me = sim.app.world_mut().spawn(LocalPlayer).id();
    let other = sim.app.world_mut().spawn_empty().id();
    sim.ticks(2);
    let kill = |sim: &mut Sim, killer: Entity, voiced: bool| {
        sim.app.world_mut().write_message(KillVoiceOutcome { killer, victim: other, voiced });
    };
    let chimes = |sim: &Sim| sim.app.world().resource::<CalloutView>().heard.iter().filter(|h| *h == KILL_CHIME).count();
    kill(&mut sim, me, false);
    sim.seconds(CHIME_WAIT + 0.2);
    assert_eq!(chimes(&sim), 1, "unvoiced: the rung-up sound");
    let view = sim.app.world().resource::<CalloutView>().clone();
    let (_, file, gain) = view.to_play.last().expect("queued").clone();
    assert_eq!((file.as_str(), gain), ("up.wav", 1.8), "at the cue's own gain");
    kill(&mut sim, me, true);
    kill(&mut sim, other, false);
    sim.seconds(CHIME_WAIT + 0.2);
    assert_eq!(chimes(&sim), 1, "voiced, or someone else's: nothing");
    // The kill's rung up sounds `rung_up` itself: no second one.
    kill(&mut sim, me, false);
    let up = Callout::new(2, "rung_up", vec![HudLine::new("Level up")]);
    set(&mut sim, Some(up), 7);
    sim.seconds(CHIME_WAIT + 0.2);
    assert_eq!(chimes(&sim), 1, "the rung up said it");
}
