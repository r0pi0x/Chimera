//! The Spartan's warning sounds (specs/halo_ce/low_health.md 3,
//! hud.md 6 "Warning sounds"): the unit HUD interface's sound elements,
//! each a sound (`snd!`, one shot) or a looping sound (`lsnd`) with the
//! states that play it, read from the game's own tags at the map's
//! conversion (`audio::kit`: `library::Warning`; nothing of them is in
//! the repo). Each frame a state word is rebuilt from the local player's
//! vitality (`word`): bit 0 the shield is charging, 1 it fell (the
//! 15-tick hit hold of `ShieldMark`), 2 `0 < s < 0.25`, 3 `s = 0`,
//! 4 `body < 0.25`, 5 dead. An element whose bits meet the word plays
//! (a loop is asked for every frame, a one-shot once per latch), one
//! whose bits do not stops (a loop is no longer asked for; its latch
//! clears). All are interface sounds, unspatialized, at scale 0. Nothing
//! plays while dead.
//!
//! The cues are the player's own state, not the HUD's: they play whatever
//! gun is in hand (a party app's Halo character holding another game's),
//! and on a network client from the replicated `Vitality`.
//!
//! Known shortcuts: the shield bits assume a game type with shields; the
//! overshield's slow drain above 1.0 counts as a fall (the spec's HUD
//! lowers its memory with it, so no `shield_hit`; only the bar's mark and
//! this sound differ, and only for an overshield draining).

use bevy::prelude::*;

use super::{
    super::vitality::{Vitality, mark::ShieldMark},
    library::{ActiveAudio, Warning},
    looping::{LoopRequest, LoopSource, owner_key},
};
use crate::{
    core::{Dead, Health, LocalPlayer},
    map::PlaySound,
};

/// The state word's bits (hud.md 6).
pub const CHARGING: u32 = 1;
pub const FELL: u32 = 1 << 1;
pub const LOW_SHIELD: u32 = 1 << 2;
pub const EMPTY_SHIELD: u32 = 1 << 3;
pub const LOW_BODY: u32 = 1 << 4;
pub const DEAD: u32 = 1 << 5;

/// Below this the shield is "low" and the body is "low" (strict).
pub const LOW_BELOW: f32 = 0.25;

/// The owner kind of the warnings' loops (`looping::owner_key`).
pub const WARNING_KIND: u8 = 5;

/// What the state word reads of the unit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vitals {
    pub alive: bool,
    pub shield: f32,
    pub body: f32,
    pub charging: bool,
}

/// The state word of a unit whose shield bookkeeping says `fell`.
pub fn word(v: &Vitals, fell: bool) -> u32 {
    if !v.alive {
        return DEAD;
    }
    let mut w = 0;
    if v.charging {
        w |= CHARGING;
    }
    if fell {
        w |= FELL;
    }
    if v.shield > 0.0 && v.shield < LOW_BELOW {
        w |= LOW_SHIELD;
    }
    if v.shield <= 0.0 {
        w |= EMPTY_SHIELD;
    }
    if v.body < LOW_BELOW {
        w |= LOW_BODY;
    }
    w
}

/// What the word asks of the sounds this frame: the elements (indices)
/// whose loops play, and those whose one-shots start now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plays {
    pub loops: Vec<usize>,
    pub shots: Vec<usize>,
}

/// The warnings' memory of one local player: the shield's mark and each
/// one-shot's latch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Warnings {
    pub mark: ShieldMark,
    latched: Vec<bool>,
}

impl Warnings {
    /// Forget everything (the unit died or is gone).
    pub fn clear(&mut self) {
        *self = Warnings::default();
    }

    /// The shield's bookkeeping for tick `now` (once per Halo tick).
    pub fn tick(&mut self, shield: f32, now: i64) {
        self.mark.update(shield, now);
    }

    /// This frame's word of `v` (after the ticks), and what it plays by
    /// `elements`' rule.
    pub fn frame(&mut self, elements: &[Warning], v: &Vitals) -> Plays {
        if !v.alive {
            self.clear();
            return Plays::default();
        }
        let w = word(v, self.mark.last > v.shield);
        self.latched.resize(elements.len(), false);
        let mut out = Plays::default();
        for (i, e) in elements.iter().enumerate() {
            if e.flags & w != 0 {
                if e.looped {
                    out.loops.push(i);
                } else if !self.latched[i] {
                    self.latched[i] = true;
                    out.shots.push(i);
                }
            } else {
                self.latched[i] = false;
            }
        }
        out
    }
}

/// The local player's warnings (module doc).
#[allow(clippy::too_many_arguments)]
pub fn play(
    active: Res<ActiveAudio>,
    me: Query<(&Vitality, Option<&Health>, Has<Dead>), With<LocalPlayer>>,
    time: Res<Time>,
    mut state: Local<Warnings>,
    mut last: Local<Option<i64>>,
    mut requests: MessageWriter<LoopRequest>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let Some(lib) = active.0.as_deref().filter(|l| !l.warnings.is_empty()) else { return };
    let Some((v, health, dead)) = me.iter().next() else {
        state.clear();
        *last = None;
        return;
    };
    let alive = !dead && !v.dead && health.is_none_or(|h| h.current > 0.0);
    let now = (time.elapsed_secs_f64() * 30.0).floor() as i64;
    if alive {
        let from = last.map_or(now, |t| (t + 1).max(now - 30));
        for t in from..=now {
            state.tick(v.shield, t);
        }
    }
    *last = Some(now);
    let vitals = Vitals { alive, shield: v.shield, body: v.body, charging: v.charging };
    let plays = state.frame(&lib.warnings, &vitals);
    for i in plays.loops {
        let e = &lib.warnings[i];
        requests.write(LoopRequest {
            key: owner_key(WARNING_KIND, i as u64),
            sound: e.sound.clone(),
            source: LoopSource::Unspatialized,
            scale: e.scale,
            alternate: false,
        });
    }
    for i in plays.shots {
        if let Some(info) = lib.sounds.get(&lib.warnings[i].sound) {
            sounds.write(PlaySound::ui(info.entry.clone()));
        }
    }
}
