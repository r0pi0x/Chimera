//! The mix's limit (`audio::sound::limit_mix`, `live_sound::MixLimit`):
//! voices whose peaks (times their gains and the master volume) sum past
//! full scale turn every clip down to keep the sum under it, and the limit
//! goes back to 1 once they end. TF2's rocket and its blast (full-scale
//! stereo waves at gain 1) clipped the output for 1-2 s without it.

use bevy::{audio::GlobalVolume, prelude::*};
use chimera_present::audio::{
    live_sound::MixLimit,
    sound::{MIX_CEILING, clip_peaks, mix_limit_for},
};
use chimera_world::{harness::Sim, map::MapSoundClip};

use crate::voice_limits::{shot, sim_of};

#[test]
fn full_scale_voices_together_are_turned_down_under_full_scale() {
    // Pure: two full-scale voices at volume 1 come to the ceiling; one
    // at volume 0.5 is left alone.
    assert!((mix_limit_for(2.0, 1.0) * 2.0 - MIX_CEILING).abs() < 1e-5);
    assert_eq!(mix_limit_for(1.0, 0.5), 1.0);
    let clip = MapSoundClip { rate: 100, channels: 2, samples: vec![0, 16384, -32768, 0].into(), loop_start: None };
    assert_eq!(&*clip_peaks(&clip), &[0.5, 1.0]);
    // Live: two full-scale shots at once.
    let mut sim = sim_of(i16::MAX);
    sim.app.insert_resource(GlobalVolume::new(bevy::audio::Volume::Linear(1.0)));
    sim.ticks(1);
    let mix_limit = |sim: &Sim| sim.app.world().resource::<MixLimit>().get();
    assert_eq!(mix_limit(&sim), 1.0, "nothing playing");
    sim.app.world_mut().write_message(shot(None, None));
    sim.app.world_mut().write_message(shot(None, None));
    sim.app.update();
    let limit = mix_limit(&sim);
    assert!((limit - MIX_CEILING / 2.0).abs() < 0.01, "two full-scale voices: {limit}");
    // The one-second clips end: back to 1.
    let tick = sim.app.world().resource::<Time<Fixed>>().timestep().as_secs_f32();
    sim.ticks((1.2 / tick) as u64);
    assert_eq!(mix_limit(&sim), 1.0);
}
