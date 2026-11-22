//! Full-auto fire as the mixer gets it (headless; backlog, gun game
//! playtest 2026-10-25: holding the trigger clipped the audio). Every
//! sound the effects start plays through Chimera's playback
//! (`chimera_present::audio::sound`) with a full-scale stand-in clip of a
//! second, as long as a gunshot's ring; the test counts the voices
//! sounding at once and reads their gains.
//!
//! The fire sound was already one voice (the shooter's fire channel);
//! the pile was the rounds' impact and casing sounds, a new voice each
//! at almost full gain up close: 18 at once for one shooter before, now
//! each gun's impact and casing channels (`effects::sounds::
//! CHANNEL_IMPACT`, `CHANNEL_SHELL`).

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::core::{Health, LocalPlayer};
use chimera_present::audio::{
    SoundPlugin,
    live_sound::LiveClip,
    sound::{Playing, SoundListener},
};
use chimera_sim::weapon::{self, Inventory};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::{
        MapSoundClip, MapSoundEntry, MapSounds, SoundLevel,
        sound::{Interval, SoundBank},
    },
};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        effects::{
            CaEffectsSet, rules,
            sounds::{CHANNEL_FIRE, CaSound},
        },
        inventory::UNIT,
        movement,
    },
    harness::CaSim,
};

/// The stand-in clip's length (s).
const CLIP_SECONDS: f32 = 1.0;
const RATE: u32 = 22050;

/// Give every sound the effects ask for an entry (a full-scale clip of
/// `CLIP_SECONDS`), as the install's loader would: the first request of
/// each is lost (stocked here the frame after).
fn stock(mut asked: MessageReader<CaSound>, bank: Option<ResMut<SoundBank>>) {
    let Some(mut bank) = bank else { return };
    for s in asked.read() {
        let entry = s.entry().to_lowercase();
        if bank.0.entry(&entry).is_some() {
            continue;
        }
        let sounds = Arc::make_mut(&mut bank.0);
        sounds.clips.push(MapSoundClip {
            rate: RATE,
            channels: 1,
            samples: vec![i16::MAX; (RATE as f32 * CLIP_SECONDS) as usize].into(),
            loop_start: None,
        });
        let level = match s.at {
            Some(_) => rules::sound_level_radii(s.radii.0, s.radii.1, UNIT),
            None => SoundLevel::Db(0.0),
        };
        sounds.entries.insert(
            entry,
            MapSoundEntry {
                waves: vec![sounds.clips.len() - 1],
                volume: Interval::fixed(1.0),
                pitch: Interval::fixed(100.0),
                level,
                channel: 0,
                dry: false,
                hearing: None,
            },
        );
    }
}

fn sim() -> Sim {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, SoundPlugin))
            .init_asset::<LiveClip>()
            .add_systems(Update, stock.before(CaEffectsSet));
    });
    sim.set_tick_interval(TICK_INTERVAL);
    if !sim.app.world().contains_resource::<SoundBank>() {
        sim.app.insert_resource(SoundBank(Arc::new(MapSounds::default())));
    }
    sim
}

fn player(sim: &mut Sim, at: Vec3) -> Entity {
    let p = sim.spawn_character(at, movement::ID);
    sim.seconds(0.5);
    sim.app.world_mut().entity_mut(p).insert(Health { current: 1.0, max: 1.0 });
    p
}

fn arm(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    let w = weapon::give(sim.app.world_mut(), p, id).unwrap();
    sim.seconds(1.5);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w));
    w
}

/// What sounds now (not faded, the clip not over).
#[derive(Default, Debug)]
struct Mix {
    all: usize,
    /// The shooter's fire sounds.
    fire: usize,
    /// From the shooter or their gun.
    shooters: usize,
    /// The loudest voice's gain.
    loudest: f32,
}

fn mix(sim: &mut Sim, shooter: Entity, gun: Entity) -> Mix {
    let now = sim.app.world().resource::<Time>().elapsed_secs_f64();
    let w = sim.app.world_mut();
    let mut m = Mix::default();
    for p in w.query::<&Playing>().iter(w).filter(|p| p.sounding(now)) {
        let (l, r) = p.gains();
        m.all += 1;
        m.loudest = m.loudest.max(l.max(r));
        // The fire voice is per attacker and weapon: the gun's.
        if p.source() == Some(gun) && p.channel() == Some(CHANNEL_FIRE) {
            m.fire += 1;
        }
        if p.source() == Some(shooter) || p.source() == Some(gun) {
            m.shooters += 1;
        }
    }
    m
}

/// Three seconds of M4A1 full auto at the floor just ahead, yours (local)
/// and another player's beside you (3D): one fire voice per shooter, a
/// gun's impacts and casings one voice each, every gain at most 1, and a
/// handful of voices in all instead of a pile.
#[test]
fn full_auto_keeps_a_few_voices() {
    let mut sim = sim();
    let me = player(&mut sim, Vec3::new(20.0, 1.0, 20.0));
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.app.world_mut().spawn((SoundListener, Transform::default(), ChildOf(me)));
    let other = player(&mut sim, Vec3::new(22.0, 1.0, 20.0));
    let mine = arm(&mut sim, me, "combat_arms:m4a1");
    let theirs = arm(&mut sim, other, "combat_arms:m4a1");
    let shooters = [me, other];
    for p in shooters {
        sim.intent(p).pitch = -0.3;
    }
    // A short burst first: every sound gets its entry.
    for p in shooters {
        sim.intent(p).fire = true;
    }
    sim.seconds(0.5);
    for p in shooters {
        sim.intent(p).fire = false;
    }
    sim.seconds(1.5);
    for p in shooters {
        sim.intent(p).fire = true;
    }
    let (mut most, mut fired) = (0, 0);
    for _ in 0..(3.0 / TICK_INTERVAL) as usize {
        sim.ticks(1);
        let (a, b) = (mix(&mut sim, me, mine), mix(&mut sim, other, theirs));
        assert!(a.fire <= 1 && b.fire <= 1, "one fire voice per shooter: {a:?} {b:?}");
        assert!(a.shooters <= 3 && b.shooters <= 3, "fire, impact, casing: {a:?} {b:?}");
        assert!(a.loudest <= 1.0, "{a:?}");
        most = most.max(a.all);
        fired += a.fire.min(1);
    }
    assert!(fired > 0, "the fire sounds played");
    assert!(most <= 6, "voices at once, two shooters: {most}");
}
