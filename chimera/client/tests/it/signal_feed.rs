//! The kill feed in the Signal look (`client::kill_feed::signal_feed`):
//! only under a theme that turns it on; newest at the bottom; your kills
//! marked with the accent, rows where you died with a quiet outline; a
//! row lives 5 s, then fades and goes, and the rows under it close the
//! gap.

use bevy::prelude::*;
use chimera_client::client::kill_feed::{
    FeedName, KillEntry, KillFeed, KillFeedPlugin, KillFeedRow, SharedKillFeed,
    signal_feed::{RowMark, SignalRow},
};
use chimera_core::core::DamageKind;
use chimera_sim::SimBasePlugins;
use chimera_ui::{Theme, Timeline, UiClock};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

const SIGNAL: &str = "name = \"test\"\n[flags.hud]\nsignal = true\n";

fn side(name: &str, local: bool, bot: bool) -> FeedName {
    FeedName { entity: Entity::PLACEHOLDER, name: name.into(), team: None, local, bot }
}

fn entry(id: u64, killer: FeedName, victim: FeedName, at: f32) -> KillEntry {
    KillEntry {
        id,
        killer: Some(killer),
        victim,
        weapon: Some("kit:weapon_rifle"),
        kind: DamageKind::Bullet,
        headshot: false,
        suicide: false,
        team_kill: false,
        at,
    }
}

fn frame(sim: &mut Sim, dt: f32) {
    sim.app.world_mut().resource_mut::<UiClock>().step(dt);
    sim.app.update();
}

fn rows(sim: &mut Sim) -> Vec<(u64, RowMark, SignalRow)> {
    let w = sim.app.world_mut();
    let mut v: Vec<(u64, RowMark, SignalRow)> =
        w.query::<(&KillFeedRow, &RowMark, &SignalRow)>().iter(w).map(|(r, m, s)| (r.0, *m, *s)).collect();
    v.sort_by_key(|r| r.0);
    v
}

fn sim(signal: bool) -> Sim {
    let mut sim = Sim::new(SimBasePlugins, (GreyboxMapPlugin, KillFeedPlugin));
    if signal {
        sim.app.insert_resource(Theme::from_toml(SIGNAL).expect("theme"));
    }
    sim.app.insert_resource(UiClock::manual());
    sim.app.insert_resource(SharedKillFeed);
    frame(&mut sim, 0.0);
    sim
}

#[test]
fn kill_feed_marks_local_kills_and_deaths() {
    let mut sim = sim(true);
    let now = sim.app.world().resource::<Time>().elapsed_secs();
    {
        let mut feed = sim.app.world_mut().resource_mut::<KillFeed>();
        feed.entries.push(entry(0, side("Larry", false, false), side("Rook", false, true), now));
        feed.entries.push(entry(1, side("Me", true, false), side("Vex", false, false), now));
        feed.entries.push(entry(2, side("Biscuit", false, true), side("Me", true, false), now));
    }
    frame(&mut sim, 0.05);
    let got = rows(&mut sim);
    assert_eq!(got.iter().map(|r| r.1).collect::<Vec<_>>(), [RowMark::Plain, RowMark::Mine, RowMark::Died]);
    assert_eq!(got.iter().map(|r| r.2.index).collect::<Vec<_>>(), [0, 1, 2], "newest at the bottom");
    {
        let w = sim.app.world_mut();
        let old_root = w
            .query_filtered::<&Visibility, With<chimera_client::client::kill_feed::KillFeedRoot>>()
            .single(w)
            .unwrap();
        assert_eq!(*old_root, Visibility::Hidden, "the old feed stays down");
        // Your kill: the accent border; your death: a quiet white one.
        let acc = Theme::from_toml(SIGNAL).unwrap().tone(chimera_ui::Tone::Acc);
        let border = |w: &mut World, id: u64| {
            w.query::<(&KillFeedRow, &BorderColor)>().iter(w).find(|(r, _)| r.0 == id).map(|(_, b)| b.top).unwrap()
        };
        let mine = border(w, 1);
        assert_eq!(mine.to_srgba().with_alpha(1.0), acc.to_srgba().with_alpha(1.0));
        let died = border(w, 2).to_srgba();
        assert!(died.red > 0.8 && died.alpha < 0.5, "a quiet white outline: {died:?}");
    }

    // The oldest outlives 5 s: it fades and goes; the others move up.
    sim.app.world_mut().resource_mut::<KillFeed>().entries[0].at = now - 5.5;
    frame(&mut sim, 0.05);
    let got = rows(&mut sim);
    assert!(got[0].2.leaving, "leaving");
    assert_eq!((got[1].2.index, got[2].2.index), (0, 1), "the gap closes");
    for _ in 0..8 {
        frame(&mut sim, 0.05);
    }
    let got = rows(&mut sim);
    assert_eq!(got.iter().map(|r| r.0).collect::<Vec<_>>(), [1, 2], "gone after its fade");
    let w = sim.app.world_mut();
    assert!(w.query::<&Timeline>().iter(w).count() >= 2);
}

#[test]
fn the_plain_feed_without_the_signal_theme() {
    let mut sim = sim(false);
    let now = sim.app.world().resource::<Time>().elapsed_secs();
    sim.app.world_mut().resource_mut::<KillFeed>().entries.push(entry(0, side("A", false, false), side("B", false, false), now));
    frame(&mut sim, 0.05);
    frame(&mut sim, 0.05);
    assert!(rows(&mut sim).is_empty());
    let w = sim.app.world_mut();
    assert_eq!(w.query::<&KillFeedRow>().iter(w).count(), 1, "today's feed");
}
