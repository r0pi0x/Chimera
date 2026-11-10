//! What playback hands the audio backend (`audio::sound::stereo_wav`),
//! decoded back with the WAV reader (`chimera_formats::wav`).

use chimera_present::audio::sound::stereo_wav;
use chimera_world::map::MapSoundClip;
use chimera_formats::wav;

/// What playback hands the audio backend: a 16-bit stereo WAV with each
/// channel's gain applied, which decodes back to the scaled samples.
#[test]
fn stereo_for_playback() {
    let clip = MapSoundClip {
        rate: 22050,
        channels: 1,
        samples: vec![1000i16, -2000, 3000].into(),
        loop_start: None,
    };
    let back = wav::decode(&stereo_wav(&clip, 0.5, 1.0)).unwrap();
    assert_eq!((back.rate, back.channels), (22050, 2));
    assert_eq!(&*back.samples, &[500, 1000, -1000, -2000, 1500, 3000]);
}
