//! The globals reader on a synthetic `matg` tag with the numbers of
//! specs/halo_ce's movement.md, health_shields.md and multiplayer_rules.md
//! (section 11.1): the player information, falling damage, multiplayer
//! information, grenades and weapon list, the announcer events and their
//! queue rule, the rejections.

// The spec's numbers are written as it states them, rounded.
#![allow(clippy::approx_constant, clippy::excessive_precision)]

use blam_formats::cache::{MapFile, TagEntry};
use blam_formats::globals::{ANNOUNCER_SOUNDS, AnnouncerEvent, Globals, OTHER_BLOCKS};
use blam_formats::object::GrenadeType;

use crate::map_fixture::{CacheFile, Struct, TagData, null_ref};

const SCNR: usize = 0;
const WEAP: usize = 1;
const BIPD: usize = 2;
const SND: usize = 3;
const JPT: usize = 4;
const PROJ: usize = 5;
const MATG: usize = 6;

fn tags() -> Vec<(&'static str, &'static str)> {
    vec![
        ("scnr", "scenarios\\syn"),
        ("weap", "weapons\\flag\\flag"),
        ("bipd", "characters\\cyborg\\cyborg"),
        ("snd!", "sound\\dialog\\multiplayer1\\play_ball"),
        ("jpt!", "globals\\falling"),
        ("proj", "weapons\\frag grenade\\frag grenade"),
        ("matg", "globals\\globals"),
    ]
}

fn map_of(sounds: usize, empty_player: bool, grenades: usize) -> MapFile {
    let mut td = TagData::new(&tags());
    td.set_struct(SCNR, &Struct::new(0x5B0).0);
    let snd = td.tag_ref(SND).to_vec();
    let sound_elems: Vec<Vec<u8>> = (0..sounds).map(|_| snd.clone()).collect();
    let sound_block = td.block(&sound_elems);
    let weap = td.tag_ref(WEAP).to_vec();
    let vehicles = td.block(&[weap.clone(), weap.clone(), weap.clone()]);
    let mp = Struct::new(0xA0)
        .put(0x00, &td.tag_ref(WEAP))
        .put(0x10, &td.tag_ref(BIPD))
        .put(0x20, &vehicles)
        .put(0x4C, &td.tag_ref(WEAP))
        .put(0x5C, &sound_block);
    let mp_block = td.block(&[mp.into()]);
    let player = Struct::new(0xF4)
        .put(0x00, &td.tag_ref(BIPD))
        .f32(0x2C, 0.512)
        .f32(0x30, 1.25)
        .f32s(0x34, &[2.25, 2.0, 2.0, 0.32, 0.9, 0.65, 0.6, 0.16, 0.035])
        .f32s(0x68, &[0.0, 0.05, 0.0])
        .f32s(0x80, &[0.8, 0.8, 1.0, 0.0, 1.25])
        // globals.md's synthetic element's idle words (made-up: 8 s, 12 s, 0.5).
        .f32s(0x9C, &[8.0, 12.0, 0.5]);
    let player_block = if empty_player {
        td.block(&[])
    } else {
        td.block(&[player.into()])
    };
    let falling = Struct::new(0x98)
        .f32s(0x08, &[3.0, 6.0])
        .put(0x10, &td.tag_ref(JPT))
        .f32(0x28, 16.5)
        .put(0x2C, &td.tag_ref(JPT))
        .put(0x3C, &null_ref("jpt!"))
        .f32(0x8C, 0.3430028)
        .f32s(0x90, &[0.14625688, 0.20683846]);
    let falling_block = td.block(&[falling.into()]);
    let grenade = || -> Vec<u8> {
        Struct::new(0x44)
            .i16(0, 4)
            .i16(2, 0)
            .put(0x04, &null_ref("effe"))
            .put(0x14, &null_ref("grhi"))
            .put(0x24, &null_ref("eqip"))
            .put(0x34, &td.tag_ref(PROJ))
            .into()
    };
    let elems: Vec<Vec<u8>> = (0..grenades).map(|_| grenade()).collect();
    let grenade_block = td.block(&elems);
    let weapons = td.block(&[weap.clone(), weap]);
    let diff = td.block(&[Struct::new(0x10).into()]);
    let main = Struct::new(0x1AC)
        .put(0x11C, &diff)
        .put(0x128, &grenade_block)
        .put(0x14C, &weapons)
        .put(0x164, &mp_block)
        .put(0x170, &player_block)
        .put(0x188, &falling_block);
    td.set_struct(MATG, &main.0);
    MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap()
}

fn tag(map: &MapFile) -> &TagEntry {
    &map.tags()[MATG]
}

#[test]
fn reads_the_blocks_the_specs_use() {
    let map = map_of(ANNOUNCER_SOUNDS, false, 2);
    let g = Globals::read(&map, tag(&map)).unwrap();
    let p = &g.player;
    assert_eq!(map.tag(p.unit.id).unwrap().path, "characters\\cyborg\\cyborg");
    assert_eq!((p.walking_speed, p.double_speed_multiplier), (0.512, 1.25));
    assert_eq!(
        (p.run_forward_speed, p.run_backward_speed, p.run_sideways_speed),
        (2.25, 2.0, 2.0)
    );
    assert_eq!(p.run_acceleration, 0.32);
    assert_eq!(
        (p.sneak_forward_speed, p.sneak_backward_speed, p.sneak_sideways_speed),
        (0.9, 0.65, 0.6)
    );
    assert_eq!((p.sneak_acceleration, p.airborne_acceleration), (0.16, 0.035));
    assert_eq!(p.grenade_origin, [0.0, 0.05, 0.0]);
    assert_eq!(
        (p.stun_movement_penalty, p.stun_turning_penalty, p.stun_jumping_penalty),
        (0.8, 0.8, 1.0)
    );
    assert_eq!((p.minimum_stun_time, p.maximum_stun_time), (0.0, 1.25));
    assert_eq!(p.maximum_stun_ticks(), 37);
    let idle = (p.first_person_idle_time.lower, p.first_person_idle_time.upper, p.first_person_idle_skip);
    assert_eq!(idle, (8.0, 12.0, 0.5));
    assert_eq!(p.first_person_idle_ticks(), (240, 360));

    let f = &g.falling;
    assert_eq!(
        (f.harmful_falling_distance.lower, f.harmful_falling_distance.upper),
        (3.0, 6.0)
    );
    assert_eq!(f.maximum_falling_distance, 16.5);
    assert_eq!(map.tag(f.falling_damage.id).unwrap().path, "globals\\falling");
    assert!(f.vehicle_hit_environment_damage.is_null());
    assert_eq!(f.maximum_falling_velocity, 0.3430028);
    assert_eq!(
        (f.harmful_falling_velocity.lower, f.harmful_falling_velocity.upper),
        (0.14625688, 0.20683846)
    );

    let m = &g.multiplayer;
    assert_eq!(map.tag(m.flag.id).unwrap().path, "weapons\\flag\\flag");
    assert_eq!(map.tag(m.unit.id).unwrap().path, "characters\\cyborg\\cyborg");
    assert_eq!(m.vehicles.len(), 3);
    assert_eq!(m.sounds.len(), 43);
    assert_eq!(
        map.tag(m.sound(AnnouncerEvent::PlayBall).unwrap().id)
            .unwrap()
            .class
            .to_string(),
        "snd!"
    );
    assert!(m.sound(AnnouncerEvent::CountdownTimerEnd).is_some());

    assert_eq!(g.grenades.len(), 2);
    for kind in [GrenadeType::Frag, GrenadeType::Plasma] {
        let gr = g.grenade(kind).unwrap();
        assert_eq!((gr.maximum_count, gr.mp_spawn_default), (4, 0));
        assert_eq!(map.tag(gr.projectile.id).unwrap().class.to_string(), "proj");
    }
    assert_eq!(g.weapon_list.len(), 2);
    assert_eq!(g.difficulty_count, 1);
    assert_eq!(g.block_counts.len(), OTHER_BLOCKS.len());
}

#[test]
fn a_short_sound_list_and_the_refusals() {
    let map = map_of(5, false, 1);
    let g = Globals::read(&map, tag(&map)).unwrap();
    assert!(g.multiplayer.sound(AnnouncerEvent::DoubleKill).is_none());
    assert!(g.multiplayer.sound(AnnouncerEvent::OneMinuteToWin).is_some());
    assert!(g.grenade(GrenadeType::Plasma).is_none());
    // Too many sounds, an empty player block, another class.
    let map = map_of(44, false, 2);
    let err = Globals::read(&map, tag(&map)).unwrap_err();
    assert!(err.to_string().contains("announcer sounds"), "{err}");
    assert!(err.to_string().contains("globals\\globals"), "{err}");
    let map = map_of(43, true, 2);
    let err = Globals::read(&map, tag(&map)).unwrap_err();
    assert!(err.to_string().contains("player information"), "{err}");
    let err = Globals::read(&map, &map.tags()[JPT]).unwrap_err();
    assert!(err.to_string().contains("not the globals"), "{err}");
}

#[test]
fn announcer_events_follow_the_specs_order_and_queue_rule() {
    assert_eq!(AnnouncerEvent::ALL.len(), ANNOUNCER_SOUNDS);
    for (i, e) in AnnouncerEvent::ALL.iter().enumerate() {
        assert_eq!(e.code() as usize, i);
    }
    assert_eq!(AnnouncerEvent::from_code(14), Some(AnnouncerEvent::DoubleKill));
    assert_eq!(AnnouncerEvent::from_code(43), None);
    // Immediate: 26 to 31 and 42.
    let immediate: Vec<i16> = AnnouncerEvent::ALL
        .iter()
        .filter(|e| e.is_immediate())
        .map(|e| e.code())
        .collect();
    assert_eq!(immediate, [26, 27, 28, 29, 30, 31, 42]);
    // Section 9's rows: stored ms -> queue ticks.
    for (ms, ticks) in [
        (1260, 42),
        (1802, 59),
        (2448, 78),
        (561, 21),
        (97, 7),
        (1008, 35),
        (3379, 106),
        (143, 9),
    ] {
        assert_eq!(AnnouncerEvent::queue_ticks(ms), ticks, "{ms} ms");
    }
}
