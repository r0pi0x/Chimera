//! Sounds for any game (docs/plans/active/sound.md), the simulation's
//! side: the map carries named sound entries and decoded clips (and
//! soundscapes); game code writes `PlaySound`, and starts, changes and
//! stops long-lived sounds with `SoundControl` under a `SoundKey`; the
//! distance model (`distance_gain`, also for bots' hearing; a level is a
//! Source sound level, an attenuation, or an inner and outer radius with
//! a distance low-pass, `distance_cutoff`: LithTech's) and the
//! surface under a point (`SurfaceGrid`, footsteps) and volumes that
//! swap one surface's footsteps for another's (`StepRemap`), and the
//! party's one footstep profile for every game's characters
//! (`EvenFootsteps`, `party_even_footsteps`: CS:S's level and volumes).
//! Playback is `audio`.

use std::{collections::HashMap, sync::Arc};

use bevy::prelude::*;

/// Decoded audio: interleaved 16-bit samples (what the format readers
/// decode to).
pub use source_formats::wav::SoundClip as MapSoundClip;

/// A sound file decoded (RIFF WAVE, else MP3): what the presentation's
/// sound assets load (`audio::clip_asset`).
pub fn decode_clip(bytes: &[u8]) -> Result<MapSoundClip, String> {
    source_formats::wav::decode_any(bytes)
}

/// How a sound falls off with distance.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SoundLevel {
    /// Source sound level (0..255 "dB"); 0 is heard everywhere.
    Db(f32),
    /// Linear falloff, silent beyond 1000 / attenuation units (GoldSrc
    /// style, Source's "CompatibilityAttenuation").
    Attenuation(f32),
    /// An inner and outer radius (meters), the gain falling with the
    /// square of the distance between them (`radius_gain`: LithTech's
    /// sounds), and optionally a low-pass that closes with distance
    /// (`low_pass`: from `start` over `span` meters, `distance_cutoff`).
    Radius {
        inner: f32,
        outer: f32,
        low_pass: Option<(f32, f32)>,
    },
}

/// A value drawn uniformly from start..start + range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub start: f32,
    pub range: f32,
}

impl Interval {
    pub const fn fixed(v: f32) -> Self {
        Self { start: v, range: 0.0 }
    }
    pub fn draw(&self, unit: f32) -> f32 {
        self.start + self.range * unit
    }
}

/// A named sound (Source: a sound script entry).
#[derive(Clone, Debug)]
pub struct MapSoundEntry {
    /// Indices into `MapData::sound_clips`; one is picked at random.
    pub waves: Vec<usize>,
    pub volume: Interval,
    /// Percent of the recorded rate.
    pub pitch: Interval,
    pub level: SoundLevel,
    pub channel: u8,
    /// Bypasses the room DSP (Source: every wave marked `#`).
    pub dry: bool,
}

/// Ground properties that sounds use (Source surface properties).
#[derive(Clone, Debug, Default)]
pub struct MapSurface {
    pub step_left: Option<String>,
    pub step_right: Option<String>,
    /// Source game material letter ('C' concrete, 'D' dirt, ...).
    pub game_material: char,
    pub bullet_impact: Option<String>,
    pub impact_soft: Option<String>,
    pub impact_hard: Option<String>,
    /// How hard it sounds when hit (audiohardnessfactor).
    pub hardness: f32,
    /// Hitting something softer than this plays the soft impact.
    pub hard_threshold: f32,
    /// Impacts slower than this play the soft sound (0: no rule), u/s.
    pub hard_min_velocity: f32,
    /// Looping friction sounds while sliding (scraperough, scrapesmooth).
    pub scrape_rough: Option<String>,
    pub scrape_smooth: Option<String>,
    /// How rough it sounds to what slides on it (audioroughnessfactor).
    pub roughness: f32,
    /// Sliding on something smoother than this plays the smooth scrape
    /// (scrapeRoughThreshold).
    pub rough_threshold: f32,
}

/// Sound data the map carries.
#[derive(Clone, Debug, Default)]
pub struct MapSounds {
    /// Lower-case entry names.
    pub entries: HashMap<String, MapSoundEntry>,
    pub clips: Vec<MapSoundClip>,
    /// Lower-case surface names.
    pub surfaces: HashMap<String, MapSurface>,
    /// Ambience: named soundscapes and where each one applies.
    pub soundscapes: Vec<Soundscape>,
    pub soundscape_zones: Vec<SoundscapeZone>,
    pub soundscape_emitters: Vec<SoundscapeEmitter>,
    /// Volumes that swap one surface's footsteps for another's.
    pub step_remaps: Vec<StepRemap>,
}

/// A volume in which footsteps on one surface play another surface's
/// (Source's func_footstep_control): a character whose feet are inside
/// steps on `from` with `to`'s step sounds.
#[derive(Clone, Debug)]
pub struct StepRemap {
    /// Convex volumes (engine space), each as planes `(normal, offset)`
    /// with the outward normal: inside when `normal . p <= offset` for all.
    pub hulls: Vec<Vec<(Vec3, f32)>>,
    /// Lower-case surface names.
    pub from: String,
    pub to: String,
}

impl StepRemap {
    /// Whether an engine-space point is inside (on a face counts).
    pub fn contains(&self, p: Vec3) -> bool {
        self.hulls
            .iter()
            .any(|h| !h.is_empty() && h.iter().all(|(n, d)| n.dot(p) <= *d))
    }
}

/// A map's background ambience (Source soundscapes): loops that fade in
/// while it applies, and one-shots at random intervals.
#[derive(Clone, Debug, Default)]
pub struct Soundscape {
    pub name: String,
    /// The room preset it sets (`room::PRESETS`, Source's "dsp"); None
    /// leaves the room as it was.
    pub dsp: Option<u16>,
    /// The listener's own DSP preset it sets ("dsp_player"); kept and
    /// shown, not played (`room::RoomDsp::player`).
    pub dsp_player: Option<u16>,
    /// The room's level it sets ("dsp_volume"); None puts the user's
    /// back.
    pub dsp_volume: Option<f32>,
    /// The sound mixer it names ("soundmixer"); kept and shown, not
    /// played (no mixers yet).
    pub mixer: Option<String>,
    pub loops: Vec<ScapeLoop>,
    pub randoms: Vec<ScapeRandom>,
}

#[derive(Clone, Debug)]
pub struct ScapeLoop {
    /// Index into `clips`.
    pub clip: usize,
    pub volume: Interval,
    /// Percent.
    pub pitch: Interval,
    /// Sound level in dB (0: no falloff).
    pub level: f32,
    /// Index into the applying zone's or emitter's positions; None plays
    /// everywhere, unspatialized.
    pub position: Option<usize>,
    /// Bypasses the room DSP (a `#` wave).
    pub dry: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScapePosition {
    Ambient,
    Index(usize),
    /// Near the listener in a random direction.
    Random,
}

#[derive(Clone, Debug)]
pub struct ScapeRandom {
    pub clips: Vec<usize>,
    /// Seconds between plays.
    pub time: Interval,
    pub volume: Interval,
    pub pitch: Interval,
    pub level: Interval,
    pub position: ScapePosition,
    /// Bypasses the room DSP (every wave marked `#`).
    pub dry: bool,
}

/// A box that selects a soundscape for a listener inside it (Source
/// trigger_soundscape); the most recently entered one wins.
#[derive(Clone, Debug)]
pub struct SoundscapeZone {
    pub min: Vec3,
    pub max: Vec3,
    pub scape: usize,
    pub positions: Vec<Option<Vec3>>,
    /// The trigger entity (index into `MapData::entities`): with the logic
    /// layer, its touch code says when the listener is inside
    /// (`SoundscapeTouches`), not the box.
    pub entity: Option<usize>,
}

/// A point that selects a soundscape within its radius (env_soundscape).
#[derive(Clone, Debug)]
pub struct SoundscapeEmitter {
    pub at: Vec3,
    /// Meters; None: unlimited.
    pub radius: Option<f32>,
    pub scape: usize,
    pub positions: Vec<Option<Vec3>>,
    /// The entity (index into `MapData::entities`), which the logic
    /// enables and disables (`SoundscapeSwitches`).
    pub entity: Option<usize>,
    /// Disabled at map start (StartDisabled).
    pub start_disabled: bool,
}

impl MapSounds {
    pub fn entry(&self, name: &str) -> Option<&MapSoundEntry> {
        self.entries.get(&name.to_lowercase())
    }
    pub fn surface(&self, name: &str) -> Option<&MapSurface> {
        self.surfaces.get(&name.to_lowercase())
    }
    /// The surface whose steps play for a step on `surface` with the feet
    /// at `feet` (engine space): the first remap volume holding the feet
    /// that remaps `surface`, else `surface` itself.
    pub fn step_surface<'a>(&'a self, surface: &'a str, feet: Vec3) -> &'a str {
        self.step_remaps
            .iter()
            .find(|r| r.from.eq_ignore_ascii_case(surface) && r.contains(feet))
            .map_or(surface, |r| r.to.as_str())
    }
}

/// Play a named sound.
#[derive(Message, Clone, Debug)]
pub struct PlaySound {
    pub entry: String,
    /// Where (engine space); None plays it unspatialized.
    pub at: Option<Vec3>,
    /// Replaces the entry's volume (footsteps compute their own).
    pub volume: Option<f32>,
    /// Replaces the entry's pitch (percent; rules that draw their own).
    pub pitch: Option<f32>,
    /// Who makes it; with `channel`, a new sound replaces the old one.
    pub source: Option<Entity>,
    pub channel: Option<u8>,
}

impl PlaySound {
    pub fn at(entry: impl Into<String>, at: Vec3) -> Self {
        Self {
            entry: entry.into(),
            at: Some(at),
            volume: None,
            pitch: None,
            source: None,
            channel: None,
        }
    }

    /// Unspatialized (announcer, interface).
    pub fn ui(entry: impl Into<String>) -> Self {
        Self {
            at: None,
            ..Self::at(entry, Vec3::ZERO)
        }
    }
}

/// A sound of the game's rules (the bomb, hostages), not of anyone's
/// predicted action: played here (`relay_game_sounds` turns it into a
/// `PlaySound`) and, on a network server, sent to every client (`net`).
#[derive(Message, Clone, Debug)]
pub struct GameSound(pub PlaySound);

/// A sound of a character's own action that its network client predicts
/// (the bomb's key presses while arming, a defuse starting): played here
/// like a `GameSound`; a network server sends it to every client but
/// `by`'s, which played it when it predicted the action (`net`).
#[derive(Message, Clone, Debug)]
pub struct PredictedSound {
    pub by: Entity,
    pub sound: PlaySound,
}

/// Play the rules' sounds here (and characters' predicted ones).
pub fn relay_game_sounds(
    mut sounds: MessageReader<GameSound>,
    mut predicted: MessageReader<PredictedSound>,
    mut play: MessageWriter<PlaySound>,
) {
    for s in predicted.read() {
        play.write(s.sound.clone());
    }
    for s in sounds.read() {
        play.write(s.0.clone());
    }
}

/// The loaded map's sounds.
#[derive(Resource, Clone)]
pub struct SoundBank(pub Arc<MapSounds>);

/// Meters per Source unit: Source's own levels (`SoundLevel::Db`,
/// `Attenuation`) are defined over inches, whatever game plays them.
pub const SOURCE_UNIT: f32 = 0.0254;
/// Distances closer than this don't get louder (Source units).
const MIN_DISTANCE: f32 = 36.0;

/// Gain at `metres` from the source (spec open question 1: hypotheses H1
/// for sound levels, H2 for attenuation entries, until measured): a
/// Source level over Source units, a `Radius` one over metres, so any
/// game's sounds fall off right on any game's map.
pub fn distance_gain(level: SoundLevel, metres: f32) -> f32 {
    let units = metres / SOURCE_UNIT;
    match level {
        SoundLevel::Db(l) if l <= 0.0 => 1.0,
        SoundLevel::Db(l) => (10f32.powf((l - 60.0) / 20.0) * MIN_DISTANCE / units.max(MIN_DISTANCE)).min(1.0),
        SoundLevel::Attenuation(a) if a <= 0.0 => 1.0,
        SoundLevel::Attenuation(a) => (1.0 - units * a / 1000.0).clamp(0.0, 1.0),
        SoundLevel::Radius { inner, outer, .. } => radius_gain(inner, outer, metres),
    }
}

/// `party_even_footsteps` (a party setting, the server's and replicated;
/// default 0, so each game's steps keep their own levels): 1 makes every
/// game's player movement sounds (steps, jump and landing steps) of
/// characters other than the listener fall off with one shared profile,
/// CS:S's (`EVEN_STEP_LEVEL`, `even_step_volume`), so neither game's
/// identity is heard louder or farther than the other's. CS:S's steps are
/// that profile already; another game's movement applies it to its own
/// (Combat Arms' others' steps, `combat_arms::game::effects`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EvenFootsteps {
    pub on: u8,
}

impl EvenFootsteps {
    /// Whether the setting is on (`None`: an app without it, off).
    pub fn is_on(s: Option<&Self>) -> bool {
        s.is_some_and(|s| s.on != 0)
    }
}

/// The shared footstep level with `party_even_footsteps 1`: CS:S's
/// (specs/cs_source/sounds.md: every footstep entry has
/// CompatibilityAttenuation 1.0), a linear falloff silent from 1000
/// Source units (25.4 m). CS:S's rather than a mean of both games' curves:
/// it is one game's tuned and specified behaviour, not a curve of ours,
/// and it leaves the CS:S identity exactly as in CS:S.
pub const EVEN_STEP_LEVEL: SoundLevel = SoundLevel::Attenuation(1.0);

/// A movement sound's pace, for the shared profile's volume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepPace {
    Walk,
    Run,
    Crouch,
    /// A jump's (or a landing's) one step.
    Jump,
}

/// The shared profile's volume for a step at `pace`: CS:S's on a hard
/// surface (sounds.md "When a footstep is checked"): 0.2 walking, 0.5
/// running, a ducked run's 0.5 x 0.65, a jump's step 1.0.
pub fn even_step_volume(pace: StepPace) -> f32 {
    match pace {
        StepPace::Walk => 0.2,
        StepPace::Run => 0.5,
        StepPace::Crouch => 0.5 * 0.65,
        StepPace::Jump => 1.0,
    }
}

/// The distance (metres) from which a sound of `level` is silent (None:
/// heard everywhere): Source's levels' falloff never reaches zero.
pub fn silent_from(level: SoundLevel) -> Option<f32> {
    match level {
        SoundLevel::Attenuation(a) if a > 0.0 => Some(1000.0 / a * SOURCE_UNIT),
        SoundLevel::Radius { inner, outer, .. } => Some(outer.min((inner * inner + (outer - inner).powi(2)).sqrt())),
        _ => None,
    }
}

/// The gain of a sound with inner radius `r` and outer radius `big_r` at
/// distance `d` (any one unit): 1 within `r` (a sound that starts there
/// keeps its full gain), `1 - (d² - r²) / (R - r)²` clamped to 0..1
/// between (zero from `sqrt(r² + (R - r)²)` on), 0 from `R`.
pub fn radius_gain(r: f32, big_r: f32, d: f32) -> f32 {
    if d <= r {
        return 1.0;
    }
    if d >= big_r || big_r <= r {
        return 0.0;
    }
    (1.0 - (d * d - r * r) / ((big_r - r) * (big_r - r))).clamp(0.0, 1.0)
}

/// The low-pass cut-off of a sound at `metres` from the listener, as a
/// fraction of the full bandwidth (1: unfiltered): a `Radius` level's
/// `low_pass` (start, span) between its radii (`low_pass_fraction`);
/// 1 for every other level.
pub fn distance_cutoff(level: SoundLevel, metres: f32) -> f32 {
    let SoundLevel::Radius {
        inner,
        outer,
        low_pass: Some((start, span)),
    } = level
    else {
        return 1.0;
    };
    let d = metres;
    if d <= inner || d >= outer {
        return 1.0;
    }
    low_pass_fraction(d, start, span)
}

/// `c = clamp(1 - cos(pi u / 2), 0.1, 1)` with `u = clamp(1 - (d - start)
/// / span, 0, 1)` (any one unit): 1 up to `start`, 0.1 from about
/// `start + 0.75 span` on.
pub fn low_pass_fraction(d: f32, start: f32, span: f32) -> f32 {
    let u = if span > 0.0 {
        (1.0 - (d - start) / span).clamp(0.0, 1.0)
    } else if d <= start {
        1.0
    } else {
        0.0
    };
    (1.0 - (std::f32::consts::FRAC_PI_2 * u).cos()).clamp(0.1, 1.0)
}

/// World triangles tagged with their surface, on a horizontal grid, to
/// find the surface under a point (footsteps).
#[derive(Resource, Default)]
pub struct SurfaceGrid {
    names: Vec<String>,
    /// Triangle corners and surface index.
    tris: Vec<([Vec3; 3], u16)>,
    grid: HashMap<(i32, i32), Vec<u32>>,
}

impl SurfaceGrid {
    const CELL: f32 = 2.0;

    pub fn new(data: &super::MapData) -> Self {
        let mut me = Self::default();
        let mut index: HashMap<String, u16> = HashMap::new();
        for m in data
            .meshes
            .iter()
            .filter(|m| !m.skybox && m.entity.is_none() && !m.material.starts_with("decal:"))
        {
            let Some(surface) = &m.surface else { continue };
            let mut id_of = |surface: &String| {
                *index.entry(surface.clone()).or_insert_with(|| {
                    me.names.push(surface.clone());
                    (me.names.len() - 1) as u16
                })
            };
            let first = id_of(surface);
            // A blended surface's second property, where the vertex alphas
            // say the second texture shows.
            let second = m
                .surface2
                .as_ref()
                .filter(|_| m.blend_weights.len() == m.positions.len())
                .map(&mut id_of);
            for t in m.indices.as_chunks::<3>().0 {
                let tri = t.map(|i| Vec3::from(m.positions[i as usize]));
                let id = match second {
                    Some(id2) if second_surface(t.map(|i| m.blend_weights[i as usize])) => id2,
                    _ => first,
                };
                let i = me.tris.len() as u32;
                let lo = tri[0].min(tri[1]).min(tri[2]);
                let hi = tri[0].max(tri[1]).max(tri[2]);
                for gx in (lo.x / Self::CELL).floor() as i32..=(hi.x / Self::CELL).floor() as i32 {
                    for gz in (lo.z / Self::CELL).floor() as i32..=(hi.z / Self::CELL).floor() as i32 {
                        me.grid.entry((gx, gz)).or_default().push(i);
                    }
                }
                me.tris.push((tri, id));
            }
        }
        me
    }

    /// The box around the world's triangles (engine space), if it has any.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut it = self.tris.iter().flat_map(|(t, _)| t.iter().copied());
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
    }

    /// The surface of the highest upward-facing triangle under `p` (engine
    /// space), at most `reach` meters below it.
    pub fn below(&self, p: Vec3, reach: f32) -> Option<&str> {
        let key = ((p.x / Self::CELL).floor() as i32, (p.z / Self::CELL).floor() as i32);
        let mut best: Option<(f32, u16)> = None;
        for &i in self.grid.get(&key)? {
            let ([a, b, c], id) = self.tris[i as usize];
            let n = (b - a).cross(c - a);
            if n.y <= 1e-6 {
                continue;
            }
            // Barycentric in the horizontal plane.
            let (a2, b2, c2, q) = (a.xz(), b.xz(), c.xz(), p.xz());
            let area = (b2 - a2).perp_dot(c2 - a2);
            if area.abs() < 1e-9 {
                continue;
            }
            let w1 = (q - a2).perp_dot(c2 - a2) / area;
            let w2 = (b2 - a2).perp_dot(q - a2) / area;
            if w1 < 0.0 || w2 < 0.0 || w1 + w2 > 1.0 {
                continue;
            }
            let y = a.y + (b.y - a.y) * w1 + (c.y - a.y) * w2;
            if y <= p.y + 0.01 && p.y - y <= reach && best.is_none_or(|(by, _)| y > by) {
                best = Some((y, id));
            }
        }
        best.map(|(_, id)| self.names[id as usize].as_str())
    }

    /// The surface of the triangle nearest to `p` within `reach` meters
    /// (bullet impacts on walls, contacts).
    pub fn nearest(&self, p: Vec3, reach: f32) -> Option<&str> {
        let lo = ((p.xz() - reach) / Self::CELL).floor().as_ivec2();
        let hi = ((p.xz() + reach) / Self::CELL).floor().as_ivec2();
        let mut best: Option<(f32, u16)> = None;
        for gx in lo.x..=hi.x {
            for gz in lo.y..=hi.y {
                for &i in self.grid.get(&(gx, gz)).into_iter().flatten() {
                    let (tri, id) = self.tris[i as usize];
                    let d = closest_on_triangle(p, tri).distance_squared(p);
                    if d <= reach * reach && best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, id));
                    }
                }
            }
        }
        best.map(|(_, id)| self.names[id as usize].as_str())
    }
}

/// Whether a blended surface's triangle (its corners' blend weights,
/// the displacement vertex alpha / 255) takes the material's
/// `$surfaceprop2`: when their mean is over one half, i.e. the second
/// texture is the one mostly showing (Source picks the surface per
/// displacement collision triangle from its vertices' alpha; the exact
/// threshold isn't in the specs: docs/plans/active/parity-gaps.md).
pub fn second_surface(weights: [f32; 3]) -> bool {
    weights.iter().sum::<f32>() > 1.5
}

/// The point of triangle `t` closest to `p`.
fn closest_on_triangle(p: Vec3, [a, b, c]: [Vec3; 3]) -> Vec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// Who a long-lived sound belongs to (chosen by its owner).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SoundKey(pub u64);

/// Keys with this bit (and not bit 63, the logic's) belong to the
/// soundscape's loops (`audio::soundscape`), which `StopAll` leaves alone.
pub const SOUNDSCAPE_KEYS: u64 = 1 << 62;

impl SoundKey {
    pub fn soundscape(self) -> bool {
        self.0 >> 62 == 1
    }
}

/// Start, change or stop a long-lived sound.
#[derive(Message, Clone, Debug)]
pub enum SoundControl {
    /// Replaces whatever plays under the same key.
    Start(StartSound),
    /// New volume and/or pitch (percent) for a playing sound.
    Change {
        key: SoundKey,
        volume: Option<f32>,
        pitch: Option<f32>,
    },
    Stop(SoundKey),
    /// Every long-lived sound but the soundscape's loops (map change,
    /// round restart).
    StopAll,
}

/// A long-lived sound to start.
#[derive(Clone, Debug, Default)]
pub struct StartSound {
    pub key: SoundKey,
    /// A sound entry of the map's `SoundBank` (games add raw files as
    /// entries of their own).
    pub entry: String,
    /// Where (engine space); None plays it unspatialized.
    pub at: Option<Vec3>,
    /// Plays from this entity's position while it exists (else `at`).
    pub follow: Option<Entity>,
    /// Override the entry's volume, pitch (percent) and level.
    pub volume: Option<f32>,
    pub pitch: Option<f32>,
    pub level: Option<SoundLevel>,
    /// Play this clip of the bank (`entry` then only names it).
    pub clip: Option<usize>,
    /// Loop even without a loop point (from the start).
    pub looping: bool,
    /// Bypass the room (Source's `#` waves).
    pub dry: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{MapData, MapMesh};

    /// A radius level: full within the inner radius, the squared falloff
    /// between, silent from the outer; the low-pass ramp between them.
    #[test]
    fn radius_levels() {
        assert_eq!(radius_gain(1.0, 10.0, 0.5), 1.0);
        assert!((radius_gain(1.0, 10.0, 5.0) - (1.0 - 24.0 / 81.0)).abs() < 1e-6);
        assert_eq!(radius_gain(1.0, 10.0, 10.0), 0.0);
        let level = SoundLevel::Radius {
            inner: 1.0,
            outer: 40.0,
            low_pass: Some((20.0, 10.0)),
        };
        let units = |m: f32| m;
        assert_eq!(distance_cutoff(level, units(10.0)), 1.0);
        assert!((distance_cutoff(level, units(25.0)) - (1.0 - (std::f32::consts::FRAC_PI_4).cos())).abs() < 1e-4);
        assert_eq!(distance_cutoff(level, units(39.0)), 0.1);
        assert_eq!(distance_cutoff(SoundLevel::Db(75.0), units(39.0)), 1.0);
        assert!((distance_gain(level, units(5.0)) - radius_gain(1.0, 40.0, 5.0)).abs() < 1e-5);
    }

    /// A blended surface's triangles take `$surfaceprop2` where their
    /// corners' mean blend weight is over one half.
    #[test]
    fn blended_surfaces_pick_the_second_property() {
        assert!(!second_surface([0.0, 0.0, 0.0]));
        assert!(!second_surface([1.0, 0.5, 0.0]));
        assert!(second_surface([1.0, 0.5, 0.01]));
        assert!(second_surface([1.0, 1.0, 1.0]));
        // Two floor triangles side by side: the left one's corners show
        // the first texture, the right one's the second.
        let mesh = MapMesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
                [3.0, 0.0, 0.0],
                [3.0, 0.0, 1.0],
                [4.0, 0.0, 0.0],
            ],
            indices: vec![0, 1, 2, 3, 4, 5],
            blend_weights: vec![0.0, 0.2, 0.1, 1.0, 0.9, 0.7],
            surface: Some("dirt".into()),
            surface2: Some("grass".into()),
            ..Default::default()
        };
        let data = MapData {
            meshes: vec![mesh.clone()],
            ..Default::default()
        };
        let grid = SurfaceGrid::new(&data);
        assert_eq!(grid.below(Vec3::new(0.2, 0.1, 0.2), 0.5), Some("dirt"));
        assert_eq!(grid.below(Vec3::new(3.2, 0.1, 0.2), 0.5), Some("grass"));
        assert_eq!(grid.nearest(Vec3::new(3.2, 0.05, 0.2), 0.3), Some("grass"));
        // Without weights (or a second property) the first everywhere.
        let data = MapData {
            meshes: vec![MapMesh {
                blend_weights: Vec::new(),
                ..mesh.clone()
            }],
            ..Default::default()
        };
        assert_eq!(SurfaceGrid::new(&data).below(Vec3::new(3.2, 0.1, 0.2), 0.5), Some("dirt"));
    }
}
