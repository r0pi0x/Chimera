//! Combat Arms' music and ambience (specs/combat_arms/music_ambience.md;
//! section numbers below are its):
//!
//! - `data` (data/music.toml): the files and their lengths, the menu
//!   playlist's entries, the cues by their attribute key, each mode's
//!   start track; the spec's inferred values stand in for what the
//!   install keeps in keyed attribute files (Open question 1).
//! - Chimera's `music::MusicPlayer` (2): the two stream slots, the linear
//!   fade-in and cross-fade, on the wall clock (`MusicClock`).
//! - `playlist` (3): the options' list with its ticks, rotation or a
//!   random skip, the Jukebox folder; `director` (3, 4): the per-frame
//!   menu update, and the cues of a match (loading, the mode's start
//!   track at the first real round, Quarantine's and Seize and Secure's
//!   cues, Arms Race's last stage, perk cues, the round and result
//!   sounds) from the match's facts (`MusicFacts`, `director::observe`);
//!   a party match's (a minigame's moments, Chimera's `minigame::
//!   moments`, when Combat Arms plays its music: `GameParts::music`)
//!   by Section 10 (`director::observe_party`), its interface sounds'
//!   files from data/music.toml's `[party]` (inferred). The party's own
//!   music (the `party_music` cvar: the start track once, then a shuffled
//!   playlist; 2 the original's loop; 0 off) is described in `director`.
//! - `trigger` (5): the server's `BGMTrigger` objects, `CaMusicOn` and
//!   `CaMusicOff` to every client, the late joiner's one trigger.
//! - `ambient` (6): the map's `SoundFX` objects as every client holds them
//!   (`CaAmbient`: all looping sounds live from the load, muted by
//!   distance with effects_sounds.md 10's gain); `ui::music` plays them
//!   and the music.
//! - `mix` (8): the two channels, music and effects, 0..100 with on/off
//!   switches, both silenced while the window is minimized (`CaVolumes`;
//!   `effects::sounds::play` scales effects by the effects master).
//!
//! The client alone decides the playlist, loading and mode music, the
//! cues and every volume; the server decides map triggers and map sounds
//! (9). A dedicated server runs none of the audio: it builds without it,
//! the client's state here is plain data, and only `ui::music` decodes
//! and plays.
//!
//! Debugging (docs/OBSERVABILITY.md): `ca_music` shows the player, the
//! playlist, the current screen and the last interface sounds;
//! `ca_ambient` lists the map's sounds and the gain at the player.
//!
//! Known shortcuts (docs/tech-debt.md): `party_music` isn't saved between runs
//! (set it in a config file or the console); the install's keyed attribute
//! files (the playlist's entries and default ticks, the interface sounds'
//! files, each mode's `StartBGM` and `LoopBGM`, `clientevent.txt`) aren't
//! read, so data/music.toml's inferred values stand in and no interface
//! sound has a file (the cues are counted in `CaMusic::heard`, silent);
//! the seasonal substitution isn't applied; the saved ticks and the
//! random-mode switch aren't stored (options.md hasn't landed: only
//! `Playlist::saved` and `apply`); the room data's map index isn't known,
//! so no mode has a per-map start track; the perk cues play when the local player's War Cry or Eagle Eye buff begins
//! (the replicated effects on a client: its free stamina and spread cut, so
//! Virulent's free stamina plays War Cry's cue too); Arms Race's last-stage event is
//! read from the players' `CaLadder` marks, which remote clients aren't
//! sent (a client hears it only on a listen server), not sent as an event;
//! "loading reports finished" isn't known, so the loading track is
//! stopped when the screen closes; a trigger's `OFF` does nothing in
//! another mode, like `ON`; a late joiner is told when its player
//! appears; voice-chat ducking and the volume classes (Quirk 7) aren't
//! modelled; ambient sounds are placed, not centred (no filters); a
//! sound set as `Sound` isn't resolved.

pub mod ambient;
pub mod data;
pub mod director;
pub mod mix;
pub mod playlist;
pub mod trigger;

use bevy::prelude::*;

use self::{
    ambient::CaAmbient,
    director::{CaMusic, MusicFacts, Screen, observe},
    mix::CaVolumes,
    trigger::{CaMapMusic, CaMusicOff, CaMusicOn},
};
use super::{
    effects::sounds::CaSound,
    objects::CaObjects,
    perks::{CaPerkEffects, CaPerkState},
    rules::{CaMatchView, CaRuleSettings, Phase, arms_race, objectives::CaObjectives, scoreboard},
};
use crate::{
    console::ConsoleAppExt,
    core::{Dead, Health, LocalPlayer, SimClock, Team},
    map::MapFile,
    net::NetAppExt,
};

/// The client's wall clock, ms (music fades and stream positions use it,
/// not game time). A test sets `manual` and moves `now` itself.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct MusicClock {
    pub now: f64,
    pub manual: bool,
}

/// The front end says the loading screen is up.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MusicLoading(pub bool);

/// The music and ambience.
pub struct CaMusicPlugin;

impl Plugin for CaMusicPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CaMusic>()
            .init_resource::<CaVolumes>()
            .init_resource::<MusicClock>()
            .init_resource::<MusicLoading>()
            .init_resource::<MusicFacts>()
            .init_resource::<CaAmbient>()
            .init_resource::<CaMapMusic>()
            .add_message::<CaMusicOn>()
            .add_message::<CaMusicOff>()
            .add_observer(trigger::late_join)
            .add_systems(
                Update,
                (
                    advance_clock,
                    gather,
                    drive,
                    trigger::hear,
                    ambient_sync,
                )
                    .chain()
                    .in_set(CaMusicSet),
            )
            .console_cvar(
                "party_music",
                "party_music <0|1|2>: a party match's music: 0 off, 1 the start track once then a shuffled playlist \
                 of Combat Arms' tracks, 2 Combat Arms' original (the start track looping). Volume: the musicvolume option.",
                &data::MusicData::built_in().party.default_music.to_string(),
                |w| w.get_resource::<CaMusic>().map(|m| m.party_music.to_string()),
                |w, v| {
                    let n: u8 = v.trim().parse().map_err(|_| "party_music is 0, 1 or 2".to_string())?;
                    if n > 2 {
                        return Err("party_music is 0, 1 or 2".into());
                    }
                    w.resource_mut::<CaMusic>().party_music = n;
                    Ok(())
                },
            )
            .console_command(
                "ca_music",
                "ca_music: the music player's slots and gains, the playlist's cursor and ticks, the screen and the \
                 interface sounds started.",
                |w, _| Ok(Some(describe(w))),
            )
            .console_command(
                "ca_ambient",
                "ca_ambient: the map's SoundFX objects (file, radii, volume, on) and their gain at the local player.",
                |w, _| Ok(Some(describe_ambient(w))),
            );
        app.net_part("combat_arms_music", |app| {
            use bevy_replicon::prelude::{Channel, ServerMessageAppExt};
            app.add_server_message::<CaMusicOn>(Channel::Ordered)
                .add_server_message::<CaMusicOff>(Channel::Ordered);
        });
    }
}

/// Where the music's systems run.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CaMusicSet;

fn advance_clock(time: Res<Time<Real>>, mut clock: ResMut<MusicClock>) {
    if !clock.manual {
        clock.now = time.elapsed_secs_f64() * 1000.0;
    }
}

/// The facts of this frame (the client's view of the match).
fn gather(world: &mut World) {
    let loading = world.resource::<MusicLoading>().0;
    // A party match (a minigame plays): its moments, Section 10.
    if let Some(party) = world
        .get_resource::<chimera_sim::minigame::moments::MinigameMoments>()
        .filter(|m| m.playing)
        .cloned()
    {
        use chimera_sim::minigame::moments::MomentPhase;
        let screen = if loading {
            Screen::Loading
        } else if party.phase == MomentPhase::Over && party.result.is_some() {
            Screen::Result
        } else {
            Screen::Match
        };
        let facts = MusicFacts {
            screen,
            mode: world.resource::<CaMusic>().data.party.mode.clone(),
            alive: party.alive,
            party: Some(party),
            ..default()
        };
        if *world.resource::<MusicFacts>() != facts {
            world.insert_resource(facts);
        }
        return;
    }
    let view = world.resource::<CaMatchView>().clone();
    let in_match = world.get_resource::<MapFile>().is_some() || !view.mode.is_empty();
    let screen = if loading {
        Screen::Loading
    } else if !in_match {
        Screen::Menu
    } else if view.phase == Phase::MatchOver {
        Screen::Result
    } else {
        Screen::Match
    };
    let me = chimera_core::query_cache::first::<With<LocalPlayer>>(world);
    let alive =
        me.is_some_and(|e| world.get::<Dead>(e).is_none() && world.get::<Health>(e).is_none_or(|h| h.current > 0.0));
    let my_team = me.and_then(|e| world.get::<Team>(e)).map_or(0, |t| t.0);
    let now = world.get_resource::<SimClock>().map_or(0.0, |c| c.now);
    let round_left = (view.phase == Phase::Live)
        .then_some(view.round_ends)
        .flatten()
        .map(|t| t - now);
    let move_time = world.resource::<CaRuleSettings>().objectives.flag_move_time as f64;
    let capture_left = world
        .resource::<CaObjectives>()
        .pole
        .filter(|p| p.team.is_some())
        .map(|p| move_time - (now - p.since));
    // Arms Race's last stage: the ladder marks each player who reached it.
    let last_stage = if view.mode == "VWRM" {
        world
            .query::<&arms_race::CaLadder>()
            .iter(world)
            .filter(|l| l.last_stage)
            .count() as u32
    } else {
        0
    };
    let first = screen == Screen::Result
        && me.is_some_and(|me| {
            let mut rows = scoreboard::rows(world);
            scoreboard::sort(&mut rows);
            rows.first().is_some_and(|r| r.player == me)
        });
    // War Cry's and Eagle Eye's buffs: the server's state where there is
    // one, else the effects it replicates.
    let (war_cry, eagle_eye) = me.map_or((false, false), |e| {
        if let Some(s) = world.get::<CaPerkState>(e) {
            (s.war_cry.is_some(), s.eagle_eye.is_some())
        } else if let Some(fx) = world.get::<CaPerkEffects>(e) {
            (fx.free_stamina, fx.spread_cut > 0.0)
        } else {
            (false, false)
        }
    });
    let facts = MusicFacts {
        screen,
        mode: view.mode.clone(),
        phase: view.phase,
        warmup: view.warmup,
        winner: view.winner,
        result: view.result,
        round_left,
        my_team,
        alive,
        capture_left,
        last_stage,
        first,
        war_cry,
        eagle_eye,
        party: None,
    };
    if *world.resource::<MusicFacts>() != facts {
        world.insert_resource(facts);
    }
}

/// The facts into moments, the menu update and the fade; the interface
/// sounds the moments started are played.
fn drive(
    facts: Res<MusicFacts>,
    clock: Res<MusicClock>,
    mut prev: Local<MusicFacts>,
    mut music: ResMut<CaMusic>,
    mut out: MessageWriter<CaSound>,
) {
    observe(&mut music, &prev, &facts, clock.now);
    *prev = facts.clone();
    music.frame(clock.now);
    let keys = std::mem::take(&mut music.sounds);
    // A party match's sounds have their files (`[party.interface]`).
    let table = if facts.party.is_some() { &music.data.party.interface } else { &music.data.interface };
    for k in keys {
        if let Some(file) = table.get(&k).filter(|f| !f.is_empty()) {
            out.write(CaSound::local(file.clone()));
        }
    }
}

/// The map's `SoundFX` objects: the list, and a one-shot's starts.
fn ambient_sync(objects: Res<CaObjects>, mut ambient: ResMut<CaAmbient>, mut out: MessageWriter<CaSound>) {
    ambient::sync(&mut ambient, &objects);
    let scale = if objects.scale > 0.0 { objects.scale } else { 1.0 };
    let a = &mut *ambient;
    for s in a.sounds.iter().filter(|s| !s.looping) {
        let entry = a.handled.entry(s.id).or_insert((0, false));
        // The start at the map's load (StartOn), then each restart.
        let start = (s.start_on && s.on && !entry.1) || s.plays != entry.0;
        entry.1 = true;
        entry.0 = s.plays;
        if start && s.on && !s.file.is_empty() {
            let at = super::objects::geometry::to_engine(s.at, scale);
            out.write(CaSound {
                radii: (
                    if s.inner < s.outer {
                        ambient::wire_inner(s.inner, s.outer)
                    } else {
                        s.inner
                    },
                    s.outer,
                ),
                ..CaSound::at(s.file.to_lowercase().replace('\\', "/"), at, s.outer).volume(s.volume)
            });
        }
    }
}

fn describe(w: &World) -> String {
    let m = w.resource::<CaMusic>();
    let v = w.resource::<CaVolumes>();
    let mut s = format!(
        "screen {:?}, paused {}, current {:?}, music master {:.2} ({} files known)\n",
        m.screen,
        m.paused,
        m.current,
        v.music(),
        m.files.len()
    );
    for (st, g) in m.player.gains() {
        s += &format!("  {} gain {:.2}{}\n", st.file, g, if st.looping { " loop" } else { "" });
    }
    let sel: Vec<String> = m
        .playlist
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            format!(
                "{}{}{}",
                if i as i64 == m.playlist.cursor { ">" } else { "" },
                e.name,
                if e.ticked { "" } else { "(off)" }
            )
        })
        .collect();
    s += &format!(
        "  playlist ({}): {}\n",
        if m.playlist.random { "random" } else { "rotation" },
        sel.join(" ")
    );
    s += &format!("  sounds: {:?}", m.heard.iter().rev().take(8).collect::<Vec<_>>());
    s
}

fn describe_ambient(w: &mut World) -> String {
    let scale = w.resource::<CaObjects>().scale.max(1e-6);
    let me = w
        .query_filtered::<&Transform, With<LocalPlayer>>()
        .iter(w)
        .next()
        .map(|t| super::objects::geometry::from_engine(t.translation, scale));
    let a = w.resource::<CaAmbient>();
    let mut s = format!("{} SoundFX\n", a.sounds.len());
    for f in &a.sounds {
        let g = me.map(|p| f.gain_at(f.at.distance(p)));
        s += &format!(
            "  {} {} R {} r {} V {} {}{} gain {}\n",
            f.name,
            f.file,
            f.outer,
            f.inner,
            f.volume,
            if f.looping { "loop " } else { "once " },
            if f.on { "on" } else { "off" },
            g.map_or("-".into(), |g| format!("{g:.2}")),
        );
    }
    s
}
