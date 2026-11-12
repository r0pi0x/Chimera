//! The listener's hearing for any game: while a `core::HearingEffect` runs
//! (a blast or flash nearby; Source's player DSP), every sound played is
//! mixed toward a muffled copy (one-pole low-pass, scaled) and a ringing
//! tone plays. `Hearing` holds the effect (the client sets it for the
//! local player); the audio thread reads `HearingMix`, which every
//! playing clip (`live_sound::LiveClip`) applies sample by sample.

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

use bevy::{
    audio::{AudioPlayer, AudioSink, AudioSinkPlayback, ChannelCount, Decodable, PlaybackSettings, SampleRate, Source},
    prelude::*,
    reflect::TypePath,
};

use crate::core::HearingEffect;

/// The listener's hearing: the effect running and when it started
/// (seconds, `Time` elapsed), and what the audio thread reads.
#[derive(Resource, Default)]
pub struct Hearing {
    pub effect: Option<(HearingEffect, f64)>,
    pub mix: Arc<HearingMix>,
    /// The eye is in water (`water::WaterView`): everything muffled
    /// (`UNDERWATER`).
    pub underwater: bool,
}

/// What the listener hears with its eye in water: (mix, low-pass cutoff
/// Hz, wet gain). Source switches to its `dsp_water` room preset there
/// (engine-side, its parameters UNMEASURED); ours is a steady muffle.
pub const UNDERWATER: (f32, f32, f32) = (1.0, 600.0, 0.8);

impl Hearing {
    /// Strength of the running effect at `now` (0 without one).
    pub fn level(&self, now: f64) -> f32 {
        self.effect.map_or(0.0, |(e, start)| e.level((now - start) as f32))
    }

    /// A new effect replaces the running one (Source sets the player's
    /// DSP preset anew), unless what is left of that one is stronger.
    pub fn apply(&mut self, effect: HearingEffect, now: f64) {
        if let Some((old, _)) = self.effect
            && old.mix * self.level(now) > effect.mix
        {
            return;
        }
        self.effect = Some((effect, now));
    }

    /// What the playing sounds get at `now`: (mix, cutoff Hz, wet gain,
    /// ring tone Hz, ring amplitude).
    pub fn targets(&self, now: f64) -> (f32, f32, f32, f32, f32) {
        let effect = self.effect.map(|(e, _)| {
            let mix = e.mix * self.level(now);
            (mix, e.cutoff, e.wet_gain, e.ring_hz, e.ring_gain * mix)
        });
        let (mix, cutoff, wet, ring_hz, ring) = effect.unwrap_or((0.0, 0.0, 1.0, 0.0, 0.0));
        if !self.underwater {
            return (mix, cutoff, wet, ring_hz, ring);
        }
        // Under water the muffle holds; a blast's ring still sounds, and
        // its own muffle wins where it is the duller one.
        let (w_mix, w_cutoff, w_wet) = UNDERWATER;
        let cutoff = if mix > 0.0 { cutoff.min(w_cutoff) } else { w_cutoff };
        (mix.max(w_mix), cutoff, w_wet.min(if mix > 0.0 { wet } else { 1.0 }), ring_hz, ring)
    }
}

/// What the audio thread reads (f32 bits), set each frame.
#[derive(Debug)]
pub struct HearingMix {
    mix: AtomicU32,
    cutoff: AtomicU32,
    wet: AtomicU32,
    ring_hz: AtomicU32,
    ring: AtomicU32,
}

impl Default for HearingMix {
    fn default() -> Self {
        let v = |x: f32| AtomicU32::new(x.to_bits());
        Self {
            mix: v(0.0),
            cutoff: v(0.0),
            wet: v(1.0),
            ring_hz: v(0.0),
            ring: v(0.0),
        }
    }
}

impl HearingMix {
    fn set(&self, (mix, cutoff, wet, ring_hz, ring): (f32, f32, f32, f32, f32)) {
        self.mix.store(mix.to_bits(), Ordering::Relaxed);
        self.cutoff.store(cutoff.to_bits(), Ordering::Relaxed);
        self.wet.store(wet.to_bits(), Ordering::Relaxed);
        self.ring_hz.store(ring_hz.to_bits(), Ordering::Relaxed);
        self.ring.store(ring.to_bits(), Ordering::Relaxed);
    }

    fn get(a: &AtomicU32) -> f32 {
        f32::from_bits(a.load(Ordering::Relaxed))
    }
}

/// The per-clip state of the muffle: smoothed mix, the low-pass and its
/// coefficient for the clip's rate.
#[derive(Clone, Debug, Default)]
pub struct Muffle {
    mix: f32,
    cutoff: f32,
    alpha: f32,
    low: (f32, f32),
}

/// How fast the applied mix follows a change, per frame of audio.
const MIX_SMOOTHING: f32 = 0.002;

/// The one-pole low-pass coefficient for `cutoff` Hz at `rate`.
pub fn low_pass_alpha(cutoff: f32, rate: f32) -> f32 {
    if cutoff <= 0.0 || rate <= 0.0 {
        return 1.0;
    }
    1.0 - (-std::f32::consts::TAU * cutoff / rate).exp()
}

impl Muffle {
    /// One stereo frame through the muffle.
    pub fn frame(&mut self, mix: &HearingMix, rate: f32, l: f32, r: f32) -> (f32, f32) {
        let target = HearingMix::get(&mix.mix);
        self.mix += (target - self.mix) * MIX_SMOOTHING;
        let cutoff = HearingMix::get(&mix.cutoff);
        if cutoff != self.cutoff {
            self.cutoff = cutoff;
            self.alpha = low_pass_alpha(cutoff, rate);
        }
        self.low.0 += self.alpha * (l - self.low.0);
        self.low.1 += self.alpha * (r - self.low.1);
        if self.mix < 1e-4 {
            return (l, r);
        }
        let wet = HearingMix::get(&mix.wet) * self.mix;
        let dry = 1.0 - self.mix;
        (l * dry + self.low.0 * wet, r * dry + self.low.1 * wet)
    }
}

/// The ringing: a sine whose pitch and amplitude follow `HearingMix`;
/// it ends once silent and marks itself gone.
#[derive(Asset, TypePath)]
pub struct RingTone {
    mix: Arc<HearingMix>,
    alive: Arc<AtomicBool>,
}

const RING_RATE: u32 = 44100;

pub struct RingDecoder {
    mix: Arc<HearingMix>,
    alive: Arc<AtomicBool>,
    phase: f32,
    amplitude: f32,
    right: Option<f32>,
}

impl Iterator for RingDecoder {
    type Item = bevy::audio::Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(r) = self.right.take() {
            return Some(r);
        }
        let target = HearingMix::get(&self.mix.ring);
        self.amplitude += (target - self.amplitude) * MIX_SMOOTHING;
        if target <= 0.0 && self.amplitude < 1e-4 {
            self.alive.store(false, Ordering::Relaxed);
            return None;
        }
        let hz = HearingMix::get(&self.mix.ring_hz);
        self.phase = (self.phase + hz / RING_RATE as f32).fract();
        let s = (self.phase * std::f32::consts::TAU).sin() * self.amplitude;
        self.right = Some(s);
        Some(s)
    }
}

impl Source for RingDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(RING_RATE).unwrap()
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

impl Decodable for RingTone {
    type Decoder = RingDecoder;

    fn decoder(&self) -> RingDecoder {
        RingDecoder {
            mix: self.mix.clone(),
            alive: self.alive.clone(),
            phase: 0.0,
            amplitude: 0.0,
            right: None,
        }
    }
}

/// The ringing tone playing, if any: its entity and whether it still runs.
#[derive(Resource, Default)]
struct Ringing(Option<(Entity, Arc<AtomicBool>)>);

/// Registered by `sound::SoundPlugin`.
pub(super) struct HearingPlugin;

impl Plugin for HearingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hearing>()
            .init_resource::<Ringing>()
            .add_systems(
                PostUpdate,
                (eye_in_water, drive.run_if(resource_exists::<Assets<RingTone>>)).chain(),
            );
    }

    fn finish(&self, app: &mut App) {
        super::record::add_audio_source::<RingTone>(app);
    }
}

/// Whether the eye is in water this frame (the map's `WaterView`).
fn eye_in_water(view: Option<Res<crate::map::water::WaterView>>, mut hearing: ResMut<Hearing>) {
    let under = view.is_some_and(|v| v.under.is_some());
    if hearing.underwater != under {
        hearing.underwater = under;
    }
}

/// Hand the effect's state to the audio thread; start the ringing tone
/// when it rings and none plays.
fn drive(
    mut hearing: ResMut<Hearing>,
    mut ringing: ResMut<Ringing>,
    mut tones: ResMut<Assets<RingTone>>,
    sinks: Query<&AudioSink>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    if let Some((e, start)) = hearing.effect
        && now - start >= e.length() as f64
    {
        hearing.effect = None;
    }
    let targets = hearing.targets(now);
    hearing.mix.set(targets);
    let running = ringing
        .0
        .as_ref()
        .is_some_and(|(_, alive)| alive.load(Ordering::Relaxed));
    if !running {
        if let Some((e, _)) = ringing.0.take() {
            if let Ok(sink) = sinks.get(e) {
                sink.stop();
            }
            commands.entity(e).try_despawn();
        }
        if targets.4 > 0.0 {
            let alive = Arc::new(AtomicBool::new(true));
            let handle = tones.add(RingTone {
                mix: hearing.mix.clone(),
                alive: alive.clone(),
            });
            let e = commands.spawn((AudioPlayer(handle), PlaybackSettings::DESPAWN)).id();
            ringing.0 = Some((e, alive));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RING: HearingEffect = HearingEffect {
        hold: 1.6,
        fade: 1.0,
        exponential: true,
        mix: 0.7,
        cutoff: 1000.0,
        wet_gain: 0.25,
        ring_hz: 3000.0,
        ring_gain: 0.25,
    };

    #[test]
    fn effect_holds_then_fades() {
        assert_eq!(RING.level(-0.1), 0.0);
        assert_eq!(RING.level(0.0), 1.0);
        assert_eq!(RING.level(1.6), 1.0);
        // Exponential: e^-4.6x of the fade's fraction x, cut at its end.
        assert!((RING.level(2.1) - (-2.3f32).exp()).abs() < 1e-5);
        assert!(RING.level(2.59) < 0.011);
        assert_eq!(RING.level(2.61), 0.0);
        let linear = HearingEffect {
            exponential: false,
            ..RING
        };
        assert!((linear.level(2.1) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn under_water_everything_is_muffled_and_a_ring_still_sounds() {
        let mut h = Hearing {
            underwater: true,
            ..default()
        };
        let (mix, cutoff, wet, _, ring) = h.targets(0.0);
        assert_eq!((mix, cutoff, wet, ring), (UNDERWATER.0, UNDERWATER.1, UNDERWATER.2, 0.0));
        h.apply(RING, 10.0);
        let (mix, cutoff, _, hz, ring) = h.targets(11.0);
        assert_eq!((mix, hz), (1.0, 3000.0));
        assert_eq!(cutoff, UNDERWATER.1, "the duller of the two");
        assert!(ring > 0.0);
        h.underwater = false;
        assert_eq!(h.targets(11.0).1, 1000.0, "out of the water: the blast's own");
    }

    #[test]
    fn duck_and_ring_follow_the_level() {
        let mut h = Hearing::default();
        assert_eq!(h.targets(0.0), (0.0, 0.0, 1.0, 0.0, 0.0));
        h.apply(RING, 10.0);
        let (mix, cutoff, wet, hz, ring) = h.targets(11.0);
        assert_eq!((mix, cutoff, wet, hz), (0.7, 1000.0, 0.25, 3000.0));
        assert!((ring - 0.175).abs() < 1e-6);
        // What a full-scale sound keeps at full strength: dry 0.3 plus the
        // quarter-gain wet 0.7 (low frequencies): 47.5 %.
        assert!(((1.0 - mix) + mix * wet - 0.475).abs() < 1e-6);
        // Halfway into the fade.
        let (mix, ..) = h.targets(12.1);
        assert!((mix - 0.7 * (-2.3f32).exp()).abs() < 1e-5);
        // A weaker effect doesn't cut a stronger one short; once that one
        // has faded below it, it replaces it.
        let weak = HearingEffect { mix: 0.2, ..RING };
        h.apply(weak, 11.0);
        assert_eq!(h.effect.unwrap().0, RING);
        h.apply(weak, 12.4);
        assert_eq!(h.effect.unwrap().0, weak);
    }

    #[test]
    fn muffle_low_passes_and_ducks() {
        let mix = HearingMix::default();
        let mut m = Muffle::default();
        // Off: passes through.
        assert_eq!(m.frame(&mix, 44100.0, 0.5, -0.5), (0.5, -0.5));
        mix.set((0.7, 1000.0, 0.25, 0.0, 0.0));
        // A 10 kHz tone through it, once the mix has settled.
        let mut peak: f32 = 0.0;
        for i in 0..20000 {
            let x = (i as f32 * std::f32::consts::TAU * 10000.0 / 44100.0).sin();
            let (l, _) = m.frame(&mix, 44100.0, x, x);
            if i > 10000 {
                peak = peak.max(l.abs());
            }
        }
        // Mostly the dry 30 %: the low-pass takes the rest out.
        assert!(peak > 0.3 && peak < 0.35, "{peak}");
        assert!((low_pass_alpha(1000.0, 44100.0) - 0.1329).abs() < 1e-3);
    }
}
