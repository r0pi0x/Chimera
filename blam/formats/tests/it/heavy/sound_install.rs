//! The sound reader and the Xbox ADPCM decoder on the user's Halo disc
//! image, against section (b) of specs/halo_ce/sounds.md: the assault
//! rifle's fire sound and bloodgulch's stereo ambience (fields, sample
//! counts, decoded values, the chain), and a sweep that decodes every
//! permutation of bloodgulch.map checking each block's reserved byte, step
//! index and padding nibble and the sample count rule. Each test passes
//! with a note when there is no image.

use std::{sync::OnceLock, time::Instant};

use blam_formats::cache::{Bounds, MapFile, class};
use blam_formats::disc::DiscImage;
use blam_formats::sound::looping::LoopingSound;
use blam_formats::sound::{Compression, Encoding, SampleRate, Sound, SoundClass};

use super::disc_install::{SKIP, image_path};

fn load(name: &str) -> Option<MapFile> {
    let path = image_path().or_else(|| {
        eprintln!("{SKIP}");
        None
    })?;
    let disc = DiscImage::open(&path).unwrap();
    Some(MapFile::load(&disc.read_file(&format!("maps/{name}.map")).unwrap()).unwrap())
}

fn bloodgulch() -> Option<&'static MapFile> {
    static MAP: OnceLock<Option<MapFile>> = OnceLock::new();
    MAP.get_or_init(|| load("bloodgulch")).as_ref()
}

fn sound(map: &MapFile, path: &str) -> Sound {
    let t = map
        .find(class::SOUND, path)
        .unwrap_or_else(|| panic!("no sound `{path}`"));
    Sound::read(map, t).unwrap()
}

const FIRE: &str = "sound\\sfx\\weapons\\assault rifle\\fire";
const AMBIENCE: &str = "sound\\sfx\\ambience\\multiplayer\\countryday2";

fn stats(s: &[i16]) -> (i16, i16, f64, f64) {
    let n = s.len() as f64;
    let mean = s.iter().map(|&x| f64::from(x)).sum::<f64>() / n;
    let rms = (s.iter().map(|&x| f64::from(x).powi(2)).sum::<f64>() / n).sqrt();
    (*s.iter().min().unwrap(), *s.iter().max().unwrap(), rms, mean)
}

fn channel(pcm: &[i16], c: usize) -> Vec<i16> {
    pcm.iter().skip(c).step_by(2).copied().collect()
}

#[test]
fn assault_rifle_fire_fields() {
    let Some(map) = bloodgulch() else { return };
    let tag = map.find(class::SOUND, FIRE).unwrap();
    assert_eq!((tag.index(), tag.id.0, tag.address), (458, 0xE33E_01CA, 0x8085_336C));
    let s = Sound::read(map, tag).unwrap();
    assert_eq!(s.class, SoundClass::WeaponFire);
    assert_eq!(
        (s.sample_rate, s.encoding, s.compression, s.flags),
        (SampleRate::Hz22050, Encoding::Mono, Compression::XboxAdpcm, 0)
    );
    assert_eq!(
        (s.minimum_distance, s.maximum_distance, s.gain_modifier),
        (0.0, 0.0, 0.5)
    );
    assert_eq!(s.distances(), (4.0, 70.0));
    assert_eq!(s.skip_fraction, 0.0);
    let tau = std::f32::consts::TAU;
    assert!((s.inner_cone_angle - tau).abs() < 1e-5 && (s.outer_cone_angle - tau).abs() < 1e-5);
    assert_eq!(s.outer_cone_gain, 1.0);
    assert_eq!(s.longest_permutation_length_ms, 894);
    assert_eq!(s.pitch_ranges.len(), 1);
    let r = &s.pitch_ranges[0];
    assert_eq!(
        (
            r.name.as_str(),
            r.natural_pitch,
            r.actual_permutation_count,
            r.playback_rate
        ),
        ("default", 1.0, 4, 1.0)
    );
    assert_eq!(r.bend_bounds, Bounds { lower: 0.0, upper: 1.0 });
    let names: Vec<_> = r.permutations.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["brown2_1", "brown2_2", "brown2_3", "brown2_5"]);
    assert!(
        r.permutations
            .iter()
            .all(|p| p.next_permutation.is_none() && p.gain == 1.0 && p.skip_fraction == 0.0)
    );
    let sizes: Vec<_> = r
        .permutations
        .iter()
        .map(|p| (p.sample_size, p.sample_offset))
        .collect();
    assert_eq!(
        sizes,
        [
            (8712, 0x019C_3800),
            (8712, 0x019C_5C00),
            (7452, 0x019C_8000),
            (9864, 0x019C_9E00)
        ]
    );
    let frames: Vec<_> = (0..4).map(|i| s.permutation_frames(0, i).unwrap()).collect();
    assert_eq!(frames, [15_488, 15_488, 13_248, 17_536]);
    // 894 ms is the longest permutation's bytes / 11.025.
    assert!((9864.0f64 / 11.025 - 894.0).abs() < 1.0);
}

#[test]
fn assault_rifle_fire_samples() {
    let Some(map) = bloodgulch() else { return };
    let s = sound(map, FIRE);
    let p0 = s.decode_permutation(map, 0, 0).unwrap();
    assert_eq!((p0.rate, p0.channels, p0.samples.len()), (22_050, 1, 15_488));
    assert_eq!(p0.samples[..8], [-155, -144, -174, -237, -373, -666, -1297, -2654]);
    assert_eq!(p0.samples[15_486..], [0, 0]);
    let (min, max, rms, mean) = stats(&p0.samples);
    assert_eq!((min, max), (-32768, 31859));
    assert!((rms - 5676.3).abs() < 0.5, "rms {rms}");
    assert!((mean - 4.3).abs() < 0.1, "mean {mean}");
    // Permutations 1 to 3: RMS to two digits, first samples.
    for (i, (rms_want, first)) in [(6400.0, -957), (6100.0, -104), (5000.0, -100)].into_iter().enumerate() {
        let p = s.decode_permutation(map, 0, i + 1).unwrap();
        assert_eq!(p.samples[0], first, "permutation {} first sample", i + 1);
        let (_, _, rms, _) = stats(&p.samples);
        assert!(
            (rms - rms_want).abs() < 0.03 * rms_want,
            "permutation {} rms {rms}",
            i + 1
        );
        assert_eq!(p.samples.len(), s.permutation_frames(0, i + 1).unwrap());
    }
    // A single chunk's chain is the permutation itself.
    assert_eq!(s.decode_chain(map, 0, 0).unwrap(), p0);
    // 0.7024 s for the first (15 488 samples at 22 050 Hz).
    assert!((p0.seconds() - 0.7024).abs() < 1e-4);
}

#[test]
fn countryday2_stereo_chain() {
    let Some(map) = bloodgulch() else { return };
    let s = sound(map, AMBIENCE);
    assert_eq!(
        (s.class, s.sample_rate, s.encoding, s.flags),
        (SoundClass::AmbientNature, SampleRate::Hz44100, Encoding::Stereo, 3)
    );
    assert_eq!(
        (s.minimum_distance, s.maximum_distance, s.gain_modifier),
        (0.0, 0.0, 1.0)
    );
    assert_eq!(s.distances(), (0.9, 5.0));
    assert!(s.is_split());
    assert_eq!(s.pitch_ranges.len(), 1);
    let r = &s.pitch_ranges[0];
    assert_eq!((r.permutations.len(), r.actual_permutation_count), (21, 4));
    let next: Vec<i32> = r
        .permutations
        .iter()
        .map(|p| p.next_permutation.map_or(-1, |n| n as i32))
        .collect();
    assert_eq!(
        next,
        [
            4, 8, 11, 16, 5, 6, 7, -1, 9, 10, -1, 12, 13, 14, 15, -1, 17, 18, 19, 20, -1
        ]
    );
    assert_eq!(
        (r.permutations[0].sample_size, r.permutations[0].sample_offset),
        (65_520, 0x0221_E000)
    );
    assert_eq!(s.permutation_frames(0, 0).unwrap(), 58_240);

    let p0 = s.decode_permutation(map, 0, 0).unwrap();
    assert_eq!((p0.rate, p0.channels, p0.frames()), (44_100, 2, 58_240));
    assert!((p0.seconds() - 1.32).abs() < 0.01);
    let (l, r) = (channel(&p0.samples, 0), channel(&p0.samples, 1));
    assert_eq!(l[..8], [-37, -350, 113, 174, 230, 77, 123, 81]);
    assert_eq!(r[..8], [-267, -730, -175, -101, -33, 398, 454, 403]);
    let (min, max, rms, mean) = stats(&l);
    assert_eq!((min, max), (-1440, 1612));
    assert!(
        (rms - 361.2).abs() < 2.0 && (mean - 25.5).abs() < 0.2,
        "left rms {rms} mean {mean}"
    );
    let (min, max, rms, mean) = stats(&r);
    assert_eq!((min, max), (-2026, 1788));
    assert!(
        (rms - 435.0).abs() < 5.0 && (mean - 0.9).abs() < 0.2,
        "right rms {rms} mean {mean}"
    );
    assert_eq!(l[l.len() - 3..], [-465, 82, 181]);
    assert_eq!(r[r.len() - 3..], [65, -17, -91]);

    // The chain of head 3, and every permutation visited exactly once from
    // the four heads.
    assert_eq!(s.chain(0, 3).unwrap(), [3, 16, 17, 18, 19, 20]);
    let last = &r_perm(&s, 20);
    assert_eq!((last.sample_size, last.sample_offset), (2880, 0x0233_AA00));
    assert_eq!(s.permutation_frames(0, 20).unwrap(), 2560);
    let mut seen: Vec<usize> = (0..4).flat_map(|h| s.chain(0, h).unwrap()).collect();
    seen.sort_unstable();
    assert_eq!(seen, (0..21).collect::<Vec<_>>());
    let c3 = s.decode_chain(map, 0, 3).unwrap();
    let want: usize = [3, 16, 17, 18, 19, 20]
        .iter()
        .map(|&i| s.permutation_frames(0, i).unwrap())
        .sum();
    assert_eq!((c3.frames(), c3.channels), (want, 2));
}

fn r_perm(s: &Sound, i: usize) -> blam_formats::sound::Permutation {
    s.pitch_ranges[0].permutations[i].clone()
}

/// Check every block of a permutation's raw bytes; returns the blocks seen.
fn check_blocks(what: &str, bytes: &[u8], stereo: bool) -> usize {
    let block = if stereo { 72 } else { 36 };
    assert_eq!(
        bytes.len() % block,
        0,
        "{what}: size {} is not a whole number of blocks",
        bytes.len()
    );
    for (i, b) in bytes.chunks_exact(block).enumerate() {
        let headers = if stereo { 2 } else { 1 };
        for h in 0..headers {
            assert_eq!(b[4 * h + 3], 0, "{what} block {i}: reserved byte");
            assert!(b[4 * h + 2] <= 88, "{what} block {i}: step index {}", b[4 * h + 2]);
        }
        // The padding nibble: the high nibble of each channel's last data byte.
        let pads: &[usize] = if stereo { &[8 + 8 * 7 + 3, 8 + 8 * 7 + 7] } else { &[35] };
        for &p in pads {
            assert_eq!(b[p] >> 4, 0, "{what} block {i}: padding nibble");
        }
    }
    bytes.len() / block
}

/// Decode every permutation of every sound of a map.
fn decode_everything(map: &MapFile) {
    let t0 = Instant::now();
    let (mut sounds, mut perms, mut blocks, mut samples, mut chains) = (0, 0, 0usize, 0usize, 0);
    for tag in map.of_class(class::SOUND) {
        let s = Sound::read(map, tag).unwrap_or_else(|e| panic!("{}: {e}", tag.path));
        sounds += 1;
        let stereo = s.encoding == Encoding::Stereo;
        let channels = s.encoding.channels();
        for (ri, r) in s.pitch_ranges.iter().enumerate() {
            assert!(!r.permutations.is_empty(), "{}: empty range", tag.path);
            assert!(r.actual_permutation_count <= r.permutations.len());
            for (pi, p) in r.permutations.iter().enumerate() {
                let what = format!("{} range {ri} permutation {pi}", tag.path);
                let bytes = map.file_bytes(p.sample_offset, p.sample_size).unwrap();
                blocks += check_blocks(&what, bytes, stereo);
                let pcm = s.decode_permutation(map, ri, pi).unwrap();
                // frames = floor(bytes / block) x 64.
                let frames = p.sample_size / if stereo { 72 } else { 36 } * 64;
                assert_eq!(pcm.frames(), frames, "{what}: frames");
                assert_eq!(pcm.samples.len(), frames * channels, "{what}: samples");
                assert_eq!(s.permutation_frames(ri, pi).unwrap(), frames, "{what}");
                assert_eq!(pcm.rate, s.sample_rate.hz());
                assert_eq!(pcm.channels as usize, channels);
                samples += pcm.samples.len();
                perms += 1;
            }
            // A split sound's chains from the heads visit every chunk once.
            if s.is_split() {
                let mut seen: Vec<usize> = (0..r.actual_permutation_count)
                    .flat_map(|h| s.chain(ri, h).unwrap())
                    .collect();
                seen.sort_unstable();
                assert_eq!(
                    seen,
                    (0..r.permutations.len()).collect::<Vec<_>>(),
                    "{}: chains",
                    tag.path
                );
                chains += r.actual_permutation_count;
            }
        }
    }
    eprintln!(
        "{}: decoded {perms} permutations of {sounds} sounds ({blocks} blocks, {samples} samples, {chains} chains checked) in {:?}",
        map.header().name,
        t0.elapsed()
    );
}

#[test]
fn decode_every_permutation_of_bloodgulch() {
    let Some(map) = bloodgulch() else { return };
    decode_everything(map);
    let n = map.of_class(class::SOUND).count();
    assert_eq!(n, 315);
    let perms: usize = map
        .of_class(class::SOUND)
        .map(|t| {
            Sound::read(map, t)
                .unwrap()
                .pitch_ranges
                .iter()
                .map(|r| r.permutations.len())
                .sum::<usize>()
        })
        .sum();
    assert_eq!(perms, 916);
}

#[test]
fn bloodgulch_measured_counts() {
    let Some(map) = bloodgulch() else { return };
    let sounds: Vec<Sound> = map
        .of_class(class::SOUND)
        .map(|t| Sound::read(map, t).unwrap())
        .collect();
    assert_eq!(sounds.iter().map(|s| s.pitch_ranges.len()).sum::<usize>(), 322);
    let mono22 = sounds.iter().filter(|s| s.encoding == Encoding::Mono).count();
    let stereo22 = sounds
        .iter()
        .filter(|s| s.encoding == Encoding::Stereo && s.sample_rate == SampleRate::Hz22050)
        .count();
    let stereo44 = sounds
        .iter()
        .filter(|s| s.encoding == Encoding::Stereo && s.sample_rate == SampleRate::Hz44100)
        .count();
    assert_eq!((mono22, stereo22, stereo44), (269, 45, 1));
    let bytes: usize = sounds
        .iter()
        .flat_map(|s| &s.pitch_ranges)
        .flat_map(|r| &r.permutations)
        .map(|p| p.sample_size)
        .sum();
    assert_eq!(bytes, 12_229_380);
    assert_eq!(
        sounds
            .iter()
            .filter(|s| s.minimum_distance == 0.0 && s.maximum_distance == 0.0)
            .count(),
        248
    );
    // The longest permutation: sniper_posing, 66 240 bytes.
    let longest = sounds
        .iter()
        .flat_map(|s| &s.pitch_ranges)
        .flat_map(|r| &r.permutations)
        .map(|p| p.sample_size)
        .max()
        .unwrap();
    assert_eq!(longest, 66_240);
    assert_eq!(map.of_class(class::SOUND_LOOPING).count(), 14);
    // Every looping sound reads, and its sounds resolve.
    for t in map.of_class(class::SOUND_LOOPING) {
        let l = LoopingSound::read(map, t).unwrap();
        assert!((1..=4).contains(&l.tracks.len()), "{}", t.path);
        for k in &l.tracks {
            assert!(map.tag(k.loop_sound.id).is_some(), "{}: loop sound", t.path);
        }
    }
}

/// The sound environments (sounds.md, "Sound environment"): bloodgulch has
/// one; a30's three read the spec's values.
#[test]
fn sound_environments() {
    use blam_formats::sound::environment::SoundEnvironment;
    let Some(map) = bloodgulch() else { return };
    assert_eq!(map.of_class(class::SOUND_ENVIRONMENT).count(), 1);
    for t in map.of_class(class::SOUND_ENVIRONMENT) {
        SoundEnvironment::read(map, t).unwrap();
    }
    let a30 = load("a30").unwrap();
    let env = |name: &str| {
        let t = a30
            .of_class(class::SOUND_ENVIRONMENT)
            .find(|t| t.path.ends_with(name))
            .unwrap_or_else(|| panic!("no environment `{name}`"));
        SoundEnvironment::read(&a30, t).unwrap()
    };
    let close = |a: f32, b: f32| (a - b).abs() <= 0.0005 * b.abs().max(1.0);
    let check = |e: SoundEnvironment, want: [f32; 12]| {
        let got = [
            e.room_intensity,
            e.room_intensity_hf,
            e.decay_time,
            e.decay_hf_ratio,
            e.reflections_intensity,
            e.reflections_delay,
            e.reverb_intensity,
            e.reverb_delay,
            e.diffusion,
            e.density,
            e.hf_reference,
            e.priority as f32,
        ];
        for (g, w) in got.iter().zip(want) {
            assert!(close(*g, w), "{got:?} against {want:?}");
        }
    };
    check(env("mountains"), [0.316, 0.0562, 1.49, 0.21, 0.0128, 0.3, 0.009, 0.1, 0.27, 1.0, 5000.0, 0.0]);
    check(env("hallway"), [0.316, 0.71, 1.49, 0.59, 0.077, 0.007, 0.166, 0.011, 1.0, 1.0, 5000.0, 0.0]);
    check(env("concert_hall"), [0.316, 0.5623, 3.92, 0.7, 0.0767, 0.02, 0.0997, 0.029, 1.0, 1.0, 5000.0, 0.0]);
}
