//! The sound reader, the Xbox ADPCM decoder and the distance rules on
//! synthetic data: every case of specs/halo_ce/sounds.md's test section
//! (a) that concerns data (the mono and stereo blocks byte for byte, the
//! block table, the distance resolution and attenuation tables, the gain
//! example), and a sound and a looping sound tag built with `map_fixture`.

use blam_formats::cache::{Bounds, MapFile, TagRef, class};
use blam_formats::sound::adpcm::{self, decode_mono_block, decode_stereo_block};
use blam_formats::sound::gain::{self, GainFactors};
use blam_formats::sound::looping::LoopingSound;
use blam_formats::sound::{
    Compression, Encoding, PitchRange, SOUND_CLASS_COUNT, SampleRate, ScaleModifiers, Sound, SoundClass,
};

use crate::map_fixture::{CacheFile, HEADER, Struct, TagData, data_ref_bytes, null_ref};

fn hex(s: &str) -> Vec<u8> {
    let s: String = s.split_whitespace().collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn padded(mut v: Vec<u8>, len: usize) -> Vec<u8> {
    v.resize(len, 0);
    v
}

fn block_a() -> Vec<u8> {
    padded(hex("e8030a00 078f53a1"), 36)
}

fn block_b() -> Vec<u8> {
    padded(hex("007d5500 7777ffff ffff"), 36)
}

fn block_c() -> Vec<u8> {
    padded(hex("00830200 88880000"), 36)
}

fn stereo_block() -> Vec<u8> {
    padded(hex("f4010000 0cfe1400 21436507 9fbadc0e 11111111 99999999"), 72)
}

#[test]
fn mono_block_a() {
    let s = decode_mono_block(&block_a().try_into().unwrap());
    #[rustfmt::skip]
    let head = [
        1000, 1034, 1039, 971, 961, 1024, 1114, 1150,
        1095, 1105, 1114, 1122, 1129, 1135, 1141, 1146,
        1151, 1155, 1159, 1162, 1165, 1168, 1170, 1172,
        1174, 1176, 1178, 1179, 1180, 1181, 1182, 1183,
        1184, 1185, 1185, 1185, 1185, 1185, 1185, 1185,
    ];
    assert_eq!(s[..40], head);
    assert!(s[40..].iter().all(|&x| x == 1185), "{:?}", &s[40..]);
}

#[test]
fn mono_block_b_clamps() {
    let s = decode_mono_block(&block_b().try_into().unwrap());
    assert_eq!(s[..5], [32000, 32767, 32767, 32767, 32767]);
    assert_eq!(s[5], -28669);
    assert!(s[6..=12].iter().all(|&x| x == -32768), "{:?}", &s[6..=12]);
    assert_eq!(s[13..=15], [-28673, -24949, -21564]);
    assert_eq!(s[63], 11920);
}

#[test]
fn mono_block_c_floors_the_index() {
    let s = decode_mono_block(&block_c().try_into().unwrap());
    assert_eq!(
        s[..10],
        [
            -32000, -32001, -32002, -32002, -32002, -32002, -32002, -32002, -32002, -32002
        ]
    );
}

#[test]
fn stereo_block_layout() {
    let [l, r] = decode_stereo_block(&stereo_block().try_into().unwrap());
    assert_eq!(
        l[..18],
        [
            500, 501, 504, 508, 515, 527, 547, 588, 594, 610, 625, 638, 650, 660, 670, 679, 686, 688
        ]
    );
    assert_eq!(l[63], 701);
    assert_eq!(
        r[..18],
        [
            -500, -593, -632, -692, -769, -859, -992, -1223, -1192, -1277, -1355, -1425, -1489, -1547, -1599, -1647,
            -1690, -1677
        ]
    );
    assert_eq!(r[63], -1564);
    // Interleaved as left, right.
    let pcm = adpcm::decode_stereo(&stereo_block());
    assert_eq!(pcm.len(), 128);
    assert_eq!(pcm[..4], [500, -500, 501, -593]);
    assert_eq!(pcm[126..], [701, -1564]);
}

#[test]
fn block_tables_of_the_spec() {
    // 36 + 36 bytes mono: 128 samples, each block decoding alone.
    let two = [block_a(), block_c()].concat();
    let s = adpcm::decode_mono(&two);
    assert_eq!(s.len(), 128);
    assert_eq!(s[..64], decode_mono_block(&block_a().try_into().unwrap()));
    assert_eq!(s[64..], decode_mono_block(&block_c().try_into().unwrap()));
    // 72 bytes stereo: 64 frames interleaved.
    assert_eq!(adpcm::decode(&stereo_block(), 2).len(), 128);
    assert_eq!(adpcm::frame_count(72, 2), 64);
    // 36 * 3 + 20: 192 samples, the remainder ignored.
    let three = [block_a(), block_b(), block_c(), vec![0xAA; 20]].concat();
    assert_eq!(three.len(), 128);
    assert_eq!(adpcm::decode_mono(&three).len(), 192);
    assert_eq!(adpcm::frame_count(three.len(), 1), 192);
    // A header index of 200 is clamped to 88 first.
    let mut hot = block_a();
    hot[2] = 200;
    let mut clamped = block_a();
    clamped[2] = 88;
    assert_eq!(adpcm::decode_mono(&hot), adpcm::decode_mono(&clamped));
    // The padding nibble (the high nibble of the last byte) is never decoded.
    let mut pad = block_a();
    pad[35] = 0xF0;
    assert_eq!(adpcm::decode_mono(&pad), adpcm::decode_mono(&block_a()));
    // Nothing decodes from less than a block.
    assert!(adpcm::decode_mono(&block_a()[..35]).is_empty());
    // Duration.
    assert!((15_488.0 / 22_050.0 - 0.7024f32).abs() < 1e-4);
}

#[test]
fn the_step_table_and_adjustments() {
    assert_eq!(adpcm::STEP_TABLE.len(), 89);
    assert_eq!(
        (adpcm::STEP_TABLE[0], adpcm::STEP_TABLE[10], adpcm::STEP_TABLE[88]),
        (7, 19, 32767)
    );
    assert!(adpcm::STEP_TABLE.windows(2).all(|w| w[0] < w[1]));
    for c in 0..8 {
        assert_eq!(adpcm::INDEX_ADJUST[c], adpcm::INDEX_ADJUST[c + 8]);
    }
}

fn sound_with(class: SoundClass, encoding: Encoding, min: f32, max: f32) -> Sound {
    let m = ScaleModifiers {
        skip_fraction: 1.0,
        gain: 1.0,
        pitch: 1.0,
    };
    Sound {
        path: "sound\\x".into(),
        flags: 0,
        class,
        sample_rate: SampleRate::Hz22050,
        minimum_distance: min,
        maximum_distance: max,
        skip_fraction: 0.0,
        random_pitch_bounds: Bounds { lower: 1.0, upper: 1.0 },
        inner_cone_angle: std::f32::consts::TAU,
        outer_cone_angle: std::f32::consts::TAU,
        outer_cone_gain: 1.0,
        gain_modifier: 0.5,
        maximum_bend_per_second: 0.0,
        scale_zero: m,
        scale_one: m,
        encoding,
        compression: Compression::XboxAdpcm,
        promotion_sound: TagRef {
            class: blam_formats::cache::ClassCode::from_text("snd!"),
            path_address: 0,
            id: blam_formats::cache::TagId::NONE,
        },
        promotion_count: 0,
        longest_permutation_length_ms: 0,
        pitch_ranges: Vec::<PitchRange>::new(),
    }
}

#[test]
fn distances_resolve_each_on_its_own() {
    let d = |class, min, max| sound_with(class, Encoding::Mono, min, max).distances();
    assert_eq!(d(SoundClass::WeaponFire, 0.0, 0.0), (4.0, 70.0));
    assert_eq!(d(SoundClass::ObjectImpacts, 1.4, 0.0), (1.4, 3.0));
    assert_eq!(d(SoundClass::VehicleEngine, 6.0, 18.0), (6.0, 18.0));
    assert_eq!(d(SoundClass::ParticleImpacts, 0.0, 0.0), (0.5, 3.0));
    assert_eq!(d(SoundClass::UnitDialog, 0.0, 0.0), (3.0, 20.0));
    assert_eq!(d(SoundClass::AmbientNature, 0.0, 0.0), (0.9, 5.0));
}

#[test]
fn class_table() {
    assert_eq!(SoundClass::ALL.len(), 31);
    assert!(SoundClass::ALL.windows(2).all(|w| w[0].index() < w[1].index()));
    for n in 0..SOUND_CLASS_COUNT as i32 {
        let named = [
            2, 3, 11, 12, 16, 17, 20, 21, 24, 25, 31, 36, 37, 38, 40, 41, 42, 43, 48, 49,
        ];
        assert_eq!(SoundClass::from_index(n).is_none(), named.contains(&n), "class {n}");
    }
    let fire = SoundClass::WeaponFire.defaults();
    assert_eq!(
        (fire.per_definition_limit, fire.per_object_limit, fire.preemption_ms),
        (4, 1, 0)
    );
    assert_eq!((fire.priority, fire.wet_gain), (4, 0.5));
    assert_eq!(SoundClass::ProjectileImpact.defaults().per_definition_limit, 6);
    assert!(SoundClass::UnitDialog.defaults().speech && !SoundClass::WeaponFire.defaults().speech);
    assert_eq!(SoundClass::ScriptedDialogPlayer.defaults().priority, 6);
    assert!(SoundClass::ScriptedDialogOther.is_scripted_dialog() && !SoundClass::Music.is_scripted_dialog());
    assert_eq!(SoundClass::GameEvent as u8, 50);
}

#[test]
fn attenuation_table() {
    let (m, big_m) = (4.0, 70.0);
    for (d, want) in [
        (0.0, 1.0),
        (4.0, 1.0),
        (5.0, 0.8),
        (8.0, 0.5),
        (16.0, 0.25),
        (40.0, 0.1),
        (70.0, 0.057_142_9),
    ] {
        let got = gain::mono_gain(m, d);
        assert!((got - want).abs() < 1e-6, "mono {d}: {got}");
        assert!(gain::mono_audible(big_m, d));
    }
    assert!(!gain::mono_audible(big_m, 70.01));
    for (d, want) in [(4.0, 1.0), (20.5, 0.75), (37.0, 0.5), (70.0, 0.0), (100.0, 0.0)] {
        let got = gain::stereo_gain(m, big_m, d);
        assert!((got - want).abs() < 1e-6, "stereo {d}: {got}");
    }
    // The spec's stereo row for weapon fire's distances.
    for (d, want) in [(5.0, 0.98485), (8.0, 0.93939), (16.0, 0.81818), (40.0, 0.45455)] {
        assert!((gain::stereo_gain(m, big_m, d) - want).abs() < 1e-5, "stereo {d}");
    }
    // Through a tag with the class defaults.
    let mono = sound_with(SoundClass::WeaponFire, Encoding::Mono, 0.0, 0.0);
    let stereo = sound_with(SoundClass::WeaponFire, Encoding::Stereo, 0.0, 0.0);
    assert!((mono.distance_gain(8.0) - 0.5).abs() < 1e-6);
    assert!((stereo.distance_gain(37.0) - 0.5).abs() < 1e-6);
    // Degenerate distances: 1 inside the minimum, 0 outside.
    assert_eq!(
        (gain::stereo_gain(5.0, 5.0, 4.0), gain::stereo_gain(5.0, 3.0, 6.0)),
        (1.0, 0.0)
    );
}

#[test]
fn gain_example() {
    // Permutation gain 1, modifier 0.5, scale 1 with both modifiers 1.
    let g = GainFactors {
        modifier: 0.5,
        ..Default::default()
    };
    assert_eq!(g.gain(), 0.5);
    assert_eq!(gain::hardware_volume(g.gain()), -602);
    assert_eq!(gain::hardware_volume(0.1), -2000);
    assert_eq!(gain::hardware_volume(0.0), -10_000);
    // At 8 WU (min 4) the mono law multiplies in.
    assert_eq!(
        GainFactors {
            fade: gain::mono_gain(4.0, 8.0),
            ..g
        }
        .gain(),
        0.25
    );
    // A melee sound: zero-scale gain 0, one-scale gain 1, modifier 0.5.
    let melee = GainFactors {
        modifier: 0.5,
        scale_zero: 0.0,
        scale_one: 1.0,
        scale: 0.5,
        ..Default::default()
    };
    assert_eq!(melee.gain(), 0.25);
    assert_eq!(GainFactors { scale: 0.0, ..melee }.gain(), 0.0);
    // The result is clamped to 0..1.
    assert_eq!(
        GainFactors {
            permutation: 4.0,
            ..Default::default()
        }
        .gain(),
        1.0
    );
    assert_eq!(gain::lerp(0.0, 1.0, 0.25), 0.25);
}

/// A sound tag with two permutations of one range (a split chain 0 -> 1),
/// their ADPCM bytes in the tag data (the file offsets point at them).
fn sound_map(compression: i16, rate: i16, encoding: i16) -> MapFile {
    let mut td = TagData::new(&[("scnr", "levels\\tiny"), ("snd!", "sound\\sfx\\tiny")]);
    let a = td.append(&block_a());
    let c = td.append(&block_c());
    let off = |addr: u32| (HEADER as u32 + (addr - blam_formats::cache::TAG_BASE)) as i32;
    let perm = |name: &str, next: i16, size: i32, at: i32| -> Vec<u8> {
        Struct::new(0x7C)
            .str(0x00, name)
            .f32(0x24, 1.0)
            .i16(0x28, 1)
            .i16(0x2A, next)
            .put(0x40, &data_ref_bytes(size, at, 0))
            .into()
    };
    let perms = td.block(&[perm("head", 1, 36, off(a)), perm("tail", -1, 36, off(c))]);
    let range: Vec<u8> = Struct::new(0x48)
        .str(0x00, "default")
        .f32(0x20, 1.0)
        .f32(0x28, 1.0)
        .i16(0x2C, 1)
        .f32(0x30, 1.0)
        .put(0x3C, &perms)
        .into();
    let ranges = td.block(&[range]);
    let s = Struct::new(0xA4)
        .u32(0x00, 2)
        .i16(0x04, 4)
        .i16(0x06, rate)
        .f32(0x14, 1.0)
        .f32(0x18, 1.0)
        .f32(0x1C, std::f32::consts::TAU)
        .f32(0x20, std::f32::consts::TAU)
        .f32(0x24, 1.0)
        .f32(0x28, 0.5)
        .f32(0x3C, 1.0)
        .f32(0x40, 0.0)
        .f32(0x44, 1.0)
        .f32(0x54, 1.0)
        .f32(0x58, 1.0)
        .f32(0x5C, 1.0)
        .i16(0x6C, encoding)
        .i16(0x6E, compression)
        .put(0x70, &null_ref("snd!"))
        .i32(0x84, 894)
        .put(0x98, &ranges);
    td.set_struct(1, &s.0);
    MapFile::from_image(CacheFile::new("tiny", 1, td.finish()).image()).unwrap()
}

fn read_sound(map: &MapFile) -> std::io::Result<Sound> {
    let tag = map.of_class(class::SOUND).next().unwrap().clone();
    Sound::read(map, &tag)
}

#[test]
fn a_sound_tag_reads_and_decodes() {
    let map = sound_map(1, 0, 0);
    let s = read_sound(&map).unwrap();
    assert_eq!(s.class, SoundClass::WeaponFire);
    assert_eq!(
        (s.sample_rate, s.encoding, s.compression),
        (SampleRate::Hz22050, Encoding::Mono, Compression::XboxAdpcm)
    );
    assert!(s.is_split() && s.flags == 2);
    assert_eq!(s.distances(), (4.0, 70.0));
    assert_eq!((s.gain_modifier, s.longest_permutation_length_ms), (0.5, 894));
    assert_eq!(s.random_pitch_bounds, Bounds { lower: 1.0, upper: 1.0 });
    assert_eq!(s.scale_zero.gain, 0.0);
    let r = &s.pitch_ranges[0];
    assert_eq!(
        (
            r.name.as_str(),
            r.natural_pitch,
            r.playback_rate,
            r.actual_permutation_count
        ),
        ("default", 1.0, 1.0, 1)
    );
    assert_eq!(r.bend_bounds, Bounds { lower: 0.0, upper: 1.0 });
    assert_eq!(r.permutations.len(), 2);
    assert_eq!(
        (r.permutations[0].next_permutation, r.permutations[1].next_permutation),
        (Some(1), None)
    );
    assert_eq!(s.permutation_frames(0, 0).unwrap(), 64);

    let one = s.decode_permutation(&map, 0, 1).unwrap();
    assert_eq!((one.rate, one.channels, one.frames()), (22_050, 1, 64));
    assert_eq!(one.samples[..3], [-32000, -32001, -32002]);
    assert_eq!(s.chain(0, 0).unwrap(), [0, 1]);
    let chain = s.decode_chain(&map, 0, 0).unwrap();
    assert_eq!(chain.samples.len(), 128);
    assert_eq!(chain.samples[..2], [1000, 1034]);
    assert_eq!(chain.samples[64], -32000);
    assert!(s.decode_permutation(&map, 0, 2).is_err());
    assert!((chain.seconds() - 128.0 / 22_050.0).abs() < 1e-9);
}

#[test]
fn refused_sounds() {
    // Compression other than Xbox ADPCM, mono at 44100 Hz, a bad rate.
    for (compression, rate, encoding) in [(0, 0, 0), (2, 0, 0), (3, 0, 0), (1, 1, 0), (1, 2, 0), (1, 0, 2)] {
        assert!(
            read_sound(&sound_map(compression, rate, encoding)).is_err(),
            "{compression} {rate} {encoding}"
        );
    }
    // Stereo at 44100 Hz is fine.
    let s = read_sound(&sound_map(1, 1, 1)).unwrap();
    assert_eq!((s.sample_rate.hz(), s.encoding.channels()), (44_100, 2));
}

#[test]
fn a_looping_sound_reads() {
    let mut td = TagData::new(&[
        ("scnr", "levels\\tiny"),
        ("lsnd", "sound\\loop"),
        ("snd!", "sound\\a"),
        ("snd!", "sound\\b"),
    ]);
    let (a, b) = (td.tag_ref(2), td.tag_ref(3));
    let track: Vec<u8> = Struct::new(0xA0)
        .u32(0x00, 3)
        .f32(0x04, 0.5)
        .f32(0x08, 1.5)
        .f32(0x0C, 2.5)
        .put(0x30, &a)
        .put(0x40, &b)
        .put(0x50, &null_ref("snd!"))
        .put(0x80, &null_ref("snd!"))
        .put(0x90, &null_ref("snd!"))
        .into();
    let tracks = td.block(&[track]);
    let detail: Vec<u8> = Struct::new(0x68)
        .put(0x00, &b)
        .f32s(0x10, &[2.0, 4.0])
        .f32(0x18, 0.75)
        .f32s(0x50, &[-3.0, 3.0])
        .f32s(0x58, &[-1.5, 1.5])
        .f32s(0x60, &[0.0, 0.0])
        .into();
    let details = td.block(&[detail]);
    let l = Struct::new(0x54)
        .u32(0x00, 2)
        .f32s(0x04, &[1.0, 1.0, 1.0])
        .f32s(0x10, &[1.0, 1.0, 1.0])
        .f32(0x20, 9.0)
        .put(0x2C, &null_ref("cdmg"))
        .put(0x3C, &tracks)
        .put(0x48, &details);
    td.set_struct(1, &l.0);
    let map = MapFile::from_image(CacheFile::new("tiny", 1, td.finish()).image()).unwrap();
    let tag = map.of_class(class::SOUND_LOOPING).next().unwrap().clone();
    let ls = LoopingSound::read(&map, &tag).unwrap();
    assert_eq!(ls.flags & blam_formats::sound::looping::FLAG_FAKE_IMPULSE, 2);
    assert_eq!(
        (ls.maximum_distance, ls.scale_lower_bound, ls.scale_upper_bound),
        (9.0, [1.0; 3], [1.0; 3])
    );
    assert!(ls.continuous_damage_effect.is_null());
    assert_eq!(ls.tracks.len(), 1);
    let t = &ls.tracks[0];
    assert_eq!(
        (t.flags, t.gain, t.fade_in_duration, t.fade_out_duration),
        (3, 0.5, 1.5, 2.5)
    );
    assert_eq!(map.tag(t.start_sound.id).unwrap().path, "sound\\a");
    assert_eq!(map.tag(t.loop_sound.id).unwrap().path, "sound\\b");
    assert!(t.end_sound.is_null() && t.alternate_loop_sound.is_null() && t.alternate_end_sound.is_null());
    let d = &ls.detail_sounds[0];
    assert_eq!(map.tag(d.sound.id).unwrap().path, "sound\\b");
    assert_eq!(d.random_period, Bounds { lower: 2.0, upper: 4.0 });
    assert_eq!(
        (
            d.gain,
            d.yaw_bounds.lower,
            d.pitch_bounds.upper,
            d.distance_bounds.upper
        ),
        (0.75, -3.0, 1.5, 0.0)
    );
}
