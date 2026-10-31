//! Ambient map sounds (music_ambience.md 6): the map's `SoundFX` objects
//! as a client holds them. The server keeps every looping sound for every
//! client whatever the distance, so each client has all of them live from
//! the map's load, audible or not, and mutes them by distance with the
//! gain rule of effects_sounds.md 10 (`gain`). The pure parts here: the
//! object's properties, the radii the wire carries, the gain; the
//! resource `CaAmbient` is the sounds of the loaded map with their state
//! (switched on, restarts, where its object is now: a keyframer carries
//! HGOILRIG's water sounds), which the front end plays (`ui::music`; a
//! placed sound with `PlayAttached` follows its object's anchor,
//! `objects::sound`) and a one-shot's restart plays as a `CaSound`.

use bevy::prelude::*;

use crate::game::objects::runtime::{Kind, Props};
use crate::map::sound::radius_gain;

/// The server clamps every sound's radii to this range (u).
pub const RADIUS_CLAMP: (f32, f32) = (1.0, 65_000.0);

/// One `SoundFX` object.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundFx {
    /// The object's index in the map's object world.
    pub id: u32,
    pub name: String,
    /// The sound file in the install's sound archives (as the object
    /// names it).
    pub file: String,
    /// The object's place (u, the world file's frame).
    pub at: Vec3,
    /// Outer radius R and inner radius r (u), clamped.
    pub outer: f32,
    pub inner: f32,
    /// 0..100.
    pub volume: f32,
    pub looping: bool,
    pub ambient: bool,
    pub attached: bool,
    pub filter: String,
    pub priority: i32,
    pub start_on: bool,
    /// Switched on now (a looping sound is live then).
    pub on: bool,
    /// How often it was started (the first start at load is not counted).
    pub plays: u32,
}

/// R and r clamped to the server's range.
pub fn clamp_radii(inner: f32, outer: f32) -> (f32, f32) {
    let (lo, hi) = RADIUS_CLAMP;
    (inner.clamp(lo, hi), outer.clamp(lo, hi))
}

/// The inner radius as clients rebuild it from the one byte the wire
/// carries, `floor(255 r / R)` of R (the byte wraps past 255).
pub fn wire_inner(inner: f32, outer: f32) -> f32 {
    let byte = ((255.0 * inner / outer).floor() as i64).rem_euclid(256);
    byte as f32 / 255.0 * outer
}

/// g(d) before the volume: 1 up to the (wire) inner radius, the squared
/// fall-off between, 0 from R. A sound whose inner radius is not below
/// the outer keeps full gain inside R and is muted from R (the formula
/// never applies; the wire's wrapped byte is not used).
pub fn gain(d: f32, inner: f32, outer: f32) -> f32 {
    if d >= outer {
        return 0.0;
    }
    if inner >= outer {
        return 1.0;
    }
    radius_gain(wire_inner(inner, outer), outer, d)
}

impl SoundFx {
    /// From a `SoundFX` object's properties (their defaults: `StartOn`
    /// true, `OuterRadius` 500, `InnerRadius` 100, `Volume` 100,
    /// `Ambient` true, `Loop` true, `PlayAttached` false).
    pub fn from_props(id: u32, name: &str, p: &Props, at: Vec3, on: bool, plays: u32) -> Self {
        let (inner, outer) = clamp_radii(p.real_or("InnerRadius", 100.0), p.real_or("OuterRadius", 500.0));
        Self {
            id,
            name: name.to_string(),
            file: p.string("Sound").trim().to_string(),
            at,
            outer,
            inner,
            volume: p.real_or("Volume", 100.0).clamp(0.0, 100.0),
            looping: p.bool_or("Loop", true),
            ambient: p.bool_or("Ambient", true),
            attached: p.bool_or("PlayAttached", false),
            filter: p.get("Filter").unwrap_or("UnFiltered").to_string(),
            priority: p.real_or("Priority", 0.0) as i32,
            start_on: p.bool_or("StartOn", true),
            on,
            plays,
        }
    }

    /// The gain at `d` u from the sound: the distance gain times
    /// `Volume`/100 (the effects master `s` goes on top).
    pub fn gain_at(&self, d: f32) -> f32 {
        gain(d, self.inner, self.outer) * self.volume / 100.0
    }

    /// A looping, switched-on sound: live on every client from the load,
    /// audible or not.
    pub fn live(&self) -> bool {
        self.looping && self.on
    }
}

/// The loaded map's `SoundFX` objects as this machine holds them.
#[derive(Resource, Default, Clone, Debug)]
pub struct CaAmbient {
    pub sounds: Vec<SoundFx>,
    /// The entities list they were built from (its address).
    pub built_from: usize,
    /// What `ui::music` and the one-shots have taken: per sound, the
    /// `plays` count last handled and whether its load-time start was.
    pub handled: std::collections::HashMap<u32, (u32, bool)>,
}

impl CaAmbient {
    /// The live sounds (looping and on).
    pub fn live(&self) -> impl Iterator<Item = &SoundFx> {
        self.sounds.iter().filter(|s| s.live())
    }

    /// The live sounds the listener at `at` (u) hears now: the sound and
    /// its gain times the effects master `s`.
    pub fn audible(&self, at: Vec3, s: f32) -> Vec<(&SoundFx, f32)> {
        self.live()
            .map(|f| (f, f.gain_at(f.at.distance(at)) * s))
            .filter(|(_, g)| *g > 0.0)
            .collect()
    }
}

/// Build (or refresh) the list from the object world: the objects of
/// class `SoundFX` with their state.
pub fn sync(a: &mut CaAmbient, objects: &crate::game::objects::CaObjects) {
    let from = objects
        .from
        .as_ref()
        .map_or(0, |f| std::sync::Arc::as_ptr(f) as *const u8 as usize);
    let n = objects.world.objects.iter().filter(|o| o.class == "SoundFX").count();
    if a.built_from != from || a.sounds.len() != n {
        a.built_from = from;
        a.handled.clear();
        a.sounds = objects
            .world
            .objects
            .iter()
            .enumerate()
            .filter(|(_, o)| o.class == "SoundFX")
            .map(|(i, o)| {
                let (on, plays) = match o.kind {
                    Kind::Switchable { on, plays } => (on, plays),
                    _ => (true, 0),
                };
                SoundFx::from_props(i as u32, &o.name, &o.props, o.pose.pos, on, plays)
            })
            .collect();
        return;
    }
    for s in &mut a.sounds {
        let Some(o) = objects.world.objects.get(s.id as usize) else { continue };
        if let Kind::Switchable { on, plays } = o.kind {
            s.on = on;
            s.plays = plays;
        }
        // Where its object is now (a keyframer or an attachment moves it:
        // world_objects.md 14, HGOILRIG's water sounds).
        s.at = o.pose.pos;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, tol: f32) {
        assert!((a - b).abs() <= tol, "{a} vs {b}");
    }

    #[test]
    fn wire_radii() {
        close(wire_inner(4000.0, 6000.0), 4000.0, 1e-3);
        close(wire_inner(1000.0, 2000.0), 996.078, 1e-2);
        // r = 2R sends 254.
        close(wire_inner(1000.0, 500.0), 254.0 / 255.0 * 500.0, 1e-3);
        assert_eq!(clamp_radii(0.0, 90_000.0), (1.0, 65_000.0));
    }

    #[test]
    fn gain_rules() {
        let g = |d| gain(d, 4000.0, 6000.0);
        close(g(3000.0), 1.0, 1e-6);
        close(g(4200.0), 0.59, 0.005);
        close(g(4400.0), 0.16, 0.005);
        assert_eq!(g(4600.0), 0.0);
        assert_eq!(gain(499.0, 600.0, 500.0), 1.0);
        assert_eq!(gain(501.0, 600.0, 500.0), 0.0);
    }
}
