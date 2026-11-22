//! Others' radio calls in 3D (`party_voice_lines_3d`,
//! `map::sound::PartyVoices`; the party apps' default, 0 in CS:S): with
//! the setting off a non-teammate's call is not heard at all (CS:S's
//! team radio); on, a living non-teammate's call within a talking voice's
//! reach (76 m) plays at the caller, at Source's talking level, with no
//! chat line; beyond that reach nothing. A teammate's call is CS:S's
//! either way: in your head, with its chat line.

use std::sync::Arc;

use bevy::{
    ecs::message::{MessageCursor, Messages},
    prelude::*,
};

use crate::stack::{
    client::chat::ChatLine,
    console::Console,
    core::{LocalPlayer, Radio, Team},
    games::cs_source::ui::radio::{RADIO_CHANNEL, RadioHearPlugin},
    harness::{CsSim, Sim},
    map::{
        MapSoundEntry, MapSounds, PlaySound, SoundLevel,
        radio::{RadioCommand, RadioCommands},
        sound::{Interval, SoundBank, TALKING},
    },
    movement::placeholder,
};

fn radio() -> RadioCommands {
    RadioCommands {
        commands: vec![RadioCommand {
            command: "roger".into(),
            menu: Some((2, 1)),
            variants: vec![("Radio.Roger".into(), "Roger that.".into())],
        }],
        format: "\u{2}%s1 (RADIO): %s2".into(),
        ..default()
    }
}

fn bank() -> SoundBank {
    let mut sounds = MapSounds::default();
    sounds.entries.insert(
        "radio.roger".into(),
        MapSoundEntry {
            waves: vec![],
            volume: Interval::fixed(1.0),
            pitch: Interval::fixed(100.0),
            level: SoundLevel::Db(0.0),
            channel: 0,
            dry: false,
            hearing: None,
        },
    );
    SoundBank(Arc::new(sounds))
}

/// What one call made the local player hear: its sounds and chat lines.
fn hear(
    sim: &mut Sim,
    from: Entity,
    sounds: &mut MessageCursor<PlaySound>,
    lines: &mut MessageCursor<ChatLine>,
) -> (Vec<PlaySound>, usize) {
    sim.app.world_mut().write_message(Radio {
        sender: from,
        command: "roger".into(),
    });
    sim.ticks(1);
    let w = sim.app.world();
    let played = sounds.read(w.resource::<Messages<PlaySound>>()).cloned().collect();
    let shown = lines.read(w.resource::<Messages<ChatLine>>()).count();
    (played, shown)
}

#[test]
fn others_radio_calls_play_at_them_with_the_setting_on() {
    let mut sim = Sim::new(CsSim, RadioHearPlugin);
    sim.app.insert_resource(radio()).insert_resource(bank());
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), placeholder::ID);
    sim.app.world_mut().entity_mut(me).insert((Team(2), LocalPlayer));
    let mate = sim.spawn_character(Vec3::new(3.0, 1.0, 0.0), placeholder::ID);
    let enemy = sim.spawn_character(Vec3::new(20.0, 1.0, 0.0), placeholder::ID);
    let far = sim.spawn_character(Vec3::new(80.0, 1.0, 0.0), placeholder::ID);
    sim.app.world_mut().entity_mut(mate).insert(Team(2));
    sim.app.world_mut().entity_mut(enemy).insert(Team(3));
    sim.app.world_mut().entity_mut(far).insert(Team(3));
    sim.ticks(2);
    let mut sounds = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
    let mut lines = sim.app.world().resource::<Messages<ChatLine>>().get_cursor_current();

    // CS:S's own: off by default, the enemy's call isn't heard.
    let cvar = sim.app.world().resource::<Console>().cvar("party_voice_lines_3d").unwrap().clone();
    assert!(cvar.default == "0" && cvar.archive && cvar.userinfo, "the player's, saved, in the userinfo");
    let (played, shown) = hear(&mut sim, enemy, &mut sounds, &mut lines);
    assert!(played.is_empty() && shown == 0, "off: {played:?}");
    let (played, shown) = hear(&mut sim, mate, &mut sounds, &mut lines);
    assert_eq!((played.len(), played[0].at, shown), (1, None, 1), "a teammate's: in your head, with its line");

    sim.app.world_mut().resource_mut::<Console>().submit("party_voice_lines_3d 1");
    sim.ticks(1);
    let (played, shown) = hear(&mut sim, enemy, &mut sounds, &mut lines);
    assert_eq!((played.len(), shown), (1, 0), "on: the voice alone, once: {played:?}");
    let s = &played[0];
    assert_eq!((s.source, s.channel), (Some(enemy), Some(RADIO_CHANNEL)), "from the caller");
    let caller = sim.position(enemy);
    assert!(s.at.is_some_and(|at| at.distance(caller) < 0.01), "at the caller: {:?} vs {caller}", s.at);
    let level = sim.app.world().resource::<SoundBank>().0.entry(&s.entry).map(|e| e.level);
    assert_eq!(level, Some(TALKING), "{}", s.entry);
    // Out of reach (76.2 m): nothing.
    let (played, _) = hear(&mut sim, far, &mut sounds, &mut lines);
    assert!(played.is_empty(), "beyond reach: {played:?}");
    // A teammate's is unchanged.
    let (played, shown) = hear(&mut sim, mate, &mut sounds, &mut lines);
    assert_eq!((played.len(), played[0].at, shown), (1, None, 1));
}
