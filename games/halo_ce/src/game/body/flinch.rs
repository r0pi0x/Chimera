//! A Halo body's flinch (specs/halo_ce/health_shields.md "Stun and pings",
//! animations.md "Damage and death", "Per-tick order"): the soft ping, one
//! overlay of the unit damage table's s-ping slot played over whatever the
//! body does, on every machine that draws the body.
//!
//! **What plays.** The multiplayer Spartan's 44 s-ping slots all hold
//! `s-ping front gut` (9 frames, three permutations), so neither the hit's
//! direction nor its part shows: the flinch is that slot's
//! (`PING_SLOT`) with a permutation drawn by its cumulative weights
//! (`variants`, `choose`; the permutations are sequences `<slot>%<k>`,
//! `anims::build`). It starts at frame 0, advances one frame per Halo tick
//! and is removed by the tick that reaches its last frame (frames 0 to 7
//! are posed: 8 ticks, about 0.27 s); a hit while one plays at a frame of
//! at most `SOFT_PING_INTERRUPT_TICKS` (always, for 9 frames) starts
//! nothing. It is an overlay on the right of the base (or the throw), under
//! the aim and look screens and the transition blend; it starts no blend.
//!
//! **What tells a machine of a hit** (`hit_between`): nothing about pings
//! is sent, so each machine reads the body's replicated vitality
//! (`vitality::Vitality`, replicated to every client; the server's own),
//! a snapshot (`Seen`) kept in `HaloBody` from one tick to the next. A
//! hit is a raised shield delay (every hit that runs the shield step, and
//! every other game's hit, `vitality::take_foreign`), a fade timer
//! restarted (any shield or body damage, overcharging included) or a
//! body that dropped. Not a flinch: a dead unit (the corpse plays the
//! death), or a shield broken by the hit (a forced hard ping: the Spartan
//! has no h-ping, so nothing shows). The server, the shooter's machine and
//! every other client see the same values, so every machine flinches the
//! same body, any game's weapon alike; the permutation is drawn from those
//! values (`draw`), so it is the same everywhere too.
//!
//! Known shortcuts:
//! - A client flinches when the snapshot that carries the hit arrives
//!   (some ticks after the server); hits merged into one snapshot are one
//!   flinch, and one merged with the hit that breaks the shield none.
//! - A hit that changes none of the replicated values (a shield hit below
//!   the minimum stun damage that leaves the shield above 0 and its fade
//!   unrestarted: below the negligible amount) does not flinch.
//! - The weapon's hand IK is not played at all, so "IK off while a soft
//!   ping plays" holds trivially.
//! - The first-person view has no ping (the spec gives it none).

use blam_formats::animation::{BodyPart, DamageDirection, DamageKind, UnitDamageSlot};

use super::anims::damage_key;
use crate::map::anim::{AnimSet, BonePose};

/// The unit's soft ping interrupt ticks (the Spartan: 9).
pub const SOFT_PING_INTERRUPT_TICKS: usize = 9;

/// The event carrying a permutation's cumulative weight (in its `cycle`).
pub const WEIGHT_EVENT: &str = "halo:weight";

/// The slot every flinch plays (module doc).
pub fn ping_slot() -> UnitDamageSlot {
    UnitDamageSlot::new(DamageKind::SoftPing, DamageDirection::Front, BodyPart::Gut)
}

/// The sequence name of permutation `k` of the slot sequence `name`
/// (0: the slot's own).
pub fn permutation_key(name: &str, k: usize) -> String {
    if k == 0 { name.to_string() } else { format!("{name}%{k}") }
}

/// The permutations of the ping slot in `set`: sequence and cumulative
/// weight, in chain order (empty: no ping).
pub fn variants(set: &AnimSet) -> Vec<(usize, f32)> {
    let name = damage_key(ping_slot());
    (0..16)
        .map_while(|k| set.names.get(&permutation_key(&name, k)).copied())
        .map(|s| {
            let w = set.sequences[s].events.iter().find(|e| e.name == WEIGHT_EVENT).map_or(1.0, |e| e.cycle);
            (s, w)
        })
        .collect()
}

/// The permutation for a uniform draw `r` in [0, 1): the first whose
/// cumulative weight is at least `r`, else the last.
pub fn choose(variants: &[(usize, f32)], r: f32) -> Option<usize> {
    variants.iter().find(|(_, w)| *w >= r).or(variants.last()).map(|(s, _)| *s)
}

/// A running flinch: its sequence and frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ping {
    pub sequence: usize,
    pub frame: usize,
}

/// One Halo tick of a running flinch: advanced, None once it reaches its
/// last frame (`frames` its length).
pub fn advance(ping: Option<Ping>, frames: usize) -> Option<Ping> {
    let p = ping?;
    let f = p.frame + 1;
    (f + 1 < frames.max(1)).then_some(Ping { frame: f, ..p })
}

/// A hit's flinch on `running` (module doc): the new one, or the running
/// one kept (a frame at most the interrupt ticks), or none in `set`.
pub fn start(running: Option<Ping>, set: &AnimSet, r: f32) -> Option<Ping> {
    if running.is_some_and(|p| p.frame <= SOFT_PING_INTERRUPT_TICKS) {
        return running;
    }
    choose(&variants(set), r).map(|sequence| Ping { sequence, frame: 0 }).or(running)
}

/// The flinch's overlay at its frame onto `pose` (the usual convention: q
/// x q_o on the right, translations added; parts it does not track are
/// untouched).
pub fn apply(set: &AnimSet, ping: Ping, pose: &mut [BonePose]) {
    let Some(anim) = set.sequences.get(ping.sequence).and_then(|s| s.anims.first()).and_then(|a| set.animations.get(*a)) else {
        return;
    };
    let at = |n: usize| ping.frame.min(n.saturating_sub(1));
    for t in &anim.tracks {
        let Some(bone) = pose.get_mut(t.bone) else { continue };
        if let Some(q) = t.rotation.as_deref().and_then(|r| r.get(at(r.len()))) {
            bone.0 = (bone.0 * *q).normalize();
        }
        if let Some(p) = t.position.as_deref().and_then(|r| r.get(at(r.len()))) {
            bone.1 += *p;
        }
    }
}

/// What a machine knows of a body's vitality between two ticks (module
/// doc).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seen {
    pub shield: f32,
    pub body: f32,
    pub delay: u32,
    pub shield_fade: Option<u32>,
    pub body_fade: Option<u32>,
    pub depleted: bool,
    pub dead: bool,
}

impl Seen {
    pub fn of(v: &super::super::vitality::Vitality) -> Self {
        Self {
            shield: v.shield,
            body: v.body,
            delay: v.delay,
            shield_fade: v.shield_fade,
            body_fade: v.body_fade,
            depleted: v.depleted,
            dead: v.dead,
        }
    }
}

/// A fade timer started again (a hit set it to 0 since `old`).
fn restarted(old: Option<u32>, new: Option<u32>) -> bool {
    match (old, new) {
        (None, Some(_)) => true,
        (Some(o), Some(n)) => n < o,
        _ => false,
    }
}

/// Whether the body was hit between `old` and `new` in a way that
/// flinches it (module doc).
pub fn hit_between(old: &Seen, new: &Seen) -> bool {
    if new.dead || old.dead {
        return false;
    }
    // The hit that breaks the shield: a forced hard ping, no h-ping.
    if new.depleted && !old.depleted {
        return false;
    }
    new.delay > old.delay || restarted(old.shield_fade, new.shield_fade) || restarted(old.body_fade, new.body_fade) || new.body < old.body
}

/// The permutation's draw for a hit seen as `new`: from its values, so
/// every machine that sees them draws the same (0 to 1).
pub fn draw(new: &Seen) -> f32 {
    let mut h: u32 = 0x9E37_79B9;
    for b in [new.shield.to_bits(), new.body.to_bits(), new.delay] {
        h = (h ^ b).wrapping_mul(0x85EB_CA6B).rotate_left(13);
    }
    (h >> 8) as f32 / (1u32 << 24) as f32
}
