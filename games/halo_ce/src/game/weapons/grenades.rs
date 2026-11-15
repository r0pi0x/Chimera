//! Halo's grenades (specs/halo_ce/projectiles.md 15, weapons.md 14,
//! animations.md "Gameplay events driven by animations",
//! multiplayer_rules.md 4.6 and 5.5): not a held weapon but a count of
//! each kind a unit carries and a throw on the grenade button (the named
//! action `GRENADE_ACTION`), run in Halo's own tick beside the weapons
//! (`systems::run`), as the game does; the projectiles are
//! data/grenades.toml's `frag` and `plasma` (`params::Projectile`, flown
//! by `flight` like any other).
//!
//! The throw (`Throw::step`): on the press with a grenade of the current
//! kind, the weapon in hand not preventing it (flag 6) and no throw
//! running, the throw starts at frame 0 (settling a reload, ending a
//! melee, leaving the zoom); the holder's 38-frame `throw-grenade`
//! animation advances one frame a tick from the next tick: at frame 2 the
//! grenade is taken from the count and created in the hand (its arming and
//! timer count from then; it does not move), at the key frame 8 it is
//! released from 0.05 WU left of the camera at aim x grenade velocity / 30
//! (the thrower's velocity not added), and the throw ends after frame 37,
//! the holder busy (the triggers released) until then. A throw cut short
//! (the thrower dead with the grenade in hand) releases it with power p =
//! ticks held / (key frame - 2 + 1): p x full + (1 - p) x aim x U(0.02,
//! 0.046667).
//!
//! Counts: a new life starts with the frag grenade's maximum of frags
//! (4) and no plasma grenades, each limited by the cap the player count
//! sets when the match begins (4; 2 from 5 players; 1 from 9; it never
//! goes down: the most characters seen); a grenade item adds one up to
//! the globals' maximum (4).
//!
//! The selected kind (`Throw::selected`: projectiles.md 15, hud.md 10):
//! the frag at a new life (the plasma grenade when only that is carried),
//! changed by the rotate-grenades button (the named action
//! `SWITCH_ACTION`, one change per press, riding the command's action
//! bits) to the other kind when it has any, else not at all; a throw takes
//! the selected kind (none when its count is 0), the last one thrown moves
//! the choice to the other kind when that has any, and a grenade picked
//! up while the selected kind is out becomes the selected kind
//! (`Throw::pick_up`). The specs name the control and the HUD's "current
//! type" but not these three rules: they are the port's.
//!
//! Known shortcuts: the starting
//! equipment's grenade flags and the game types' grenade options are not
//! read (the port's games are generic slayer); the thrown grenade's spin
//! (presentation) is not kept.

use bevy::prelude::*;
use serde::Deserialize;

use super::{explode::Explosions, params::Projectile};

/// The grenade button's named action.
pub const GRENADE_ACTION: &str = "grenade";

/// The rotate-grenades button's named action.
pub const SWITCH_ACTION: &str = "grenade_switch";

/// data/grenades.toml.
pub const DATA: &str = include_str!("../../../data/grenades.toml");

/// The kinds, in the globals' order.
pub const FRAG: usize = 0;
pub const PLASMA: usize = 1;

/// The throw's numbers (data/grenades.toml `[throw]`).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ThrowData {
    /// The globals' maximum count of each kind.
    pub maximum: [u8; 2],
    /// The unit's grenade velocity, WU per second.
    pub velocity: f32,
    /// The globals' player grenade origin (forward, left, up), WU.
    pub origin: [f32; 3],
    /// The throw animation's frames, key frame, and the frame the grenade
    /// appears in the hand.
    pub frames: i32,
    pub key_frame: i32,
    pub created_frame: i32,
    /// A cut throw's speed range (WU per tick).
    pub cut_speed: [f32; 2],
    /// Starting counts (frag, plasma) before the cap.
    pub start: [u8; 2],
    /// (players at least, cap), the first that applies.
    pub caps: Vec<[u32; 2]>,
}

/// The grenades' numbers: the throw, the explosions' constants and the two
/// projectiles.
#[derive(Resource, Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GrenadeData {
    pub throw: ThrowData,
    pub explosions: Explosions,
    pub frag: Projectile,
    pub plasma: Projectile,
}

impl GrenadeData {
    /// The table compiled in (a test checks it parses).
    pub fn load() -> Self {
        toml::from_str(DATA).unwrap_or_else(|e| panic!("data/grenades.toml: {e}"))
    }

    pub fn projectile(&self, kind: usize) -> &Projectile {
        if kind == PLASMA { &self.plasma } else { &self.frag }
    }

    /// The cap for a match that began with `players`.
    pub fn cap(&self, players: u32) -> u8 {
        self.throw
            .caps
            .iter()
            .find(|c| players >= c[0])
            .map_or(self.throw.maximum[FRAG], |c| c[1] as u8)
    }

    /// A new life's counts under `cap`.
    pub fn start(&self, cap: u8) -> [u8; 2] {
        self.throw.start.map(|n| n.min(cap))
    }
}

/// What a throw did this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrowEvent {
    /// The throw began (frame 0): settle the reload, end the melee, unzoom.
    Started,
    /// The grenade of this kind is in the hand (taken from the count).
    Created(usize),
    /// It leaves the hand.
    Released(usize),
    /// The throw's animation ended.
    Ended,
}

/// A throw in progress: its frame and kind (part of `systems::HaloHands`).
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Throw {
    /// The animation's frame (None: no throw).
    pub frame: Option<i32>,
    pub kind: u8,
    /// The counts carried (frag, plasma).
    pub counts: [u8; 2],
    /// The selected kind (0 frag, 1 plasma): what a press throws and the
    /// HUD shows.
    pub selected: u8,
}

impl Throw {
    /// The holder is busy throwing (its triggers read released).
    pub fn busy(&self) -> bool {
        self.frame.is_some()
    }

    /// A throw state for a new life with these counts: the frag selected,
    /// or the plasma grenade when only that is carried.
    pub fn new(counts: [u8; 2]) -> Self {
        Self { counts, selected: if counts[FRAG] == 0 && counts[PLASMA] > 0 { PLASMA as u8 } else { FRAG as u8 }, ..default() }
    }

    /// The selected kind (the HUD's, even at a count of 0).
    pub fn selected(&self) -> usize {
        usize::from(self.selected).min(PLASMA)
    }

    /// The kind a press throws: the selected one, while any are left.
    pub fn current(&self) -> Option<usize> {
        let k = self.selected();
        (self.counts[k] > 0).then_some(k)
    }

    /// The rotate-grenades press: the other kind when it has any.
    pub fn switch(&mut self) {
        let other = PLASMA - self.selected();
        if self.counts[other] > 0 {
            self.selected = other as u8;
        }
    }

    /// A grenade of `kind` picked up (the count already raised): it
    /// becomes the selected kind when the selected one has none.
    pub fn pick_up(&mut self, kind: usize) {
        if self.counts[self.selected()] == 0 {
            self.selected = kind.min(PLASMA) as u8;
        }
    }

    /// One tick: `press` the button's press this tick, `allowed` the
    /// weapon in hand allows a throw.
    pub fn step(&mut self, d: &ThrowData, press: bool, allowed: bool) -> Vec<ThrowEvent> {
        let mut out = Vec::new();
        match self.frame {
            Some(f) => {
                let f = f + 1;
                self.frame = Some(f);
                let kind = self.kind as usize;
                if f == d.created_frame {
                    self.counts[kind] = self.counts[kind].saturating_sub(1);
                    if self.counts[kind] == 0 && self.counts[PLASMA - kind.min(PLASMA)] > 0 {
                        self.selected = (PLASMA - kind.min(PLASMA)) as u8;
                    }
                    out.push(ThrowEvent::Created(kind));
                }
                if f == d.key_frame {
                    out.push(ThrowEvent::Released(kind));
                }
                if f + 1 >= d.frames {
                    self.frame = None;
                    out.push(ThrowEvent::Ended);
                }
            }
            None => {
                if press
                    && allowed
                    && let Some(kind) = self.current()
                {
                    self.frame = Some(0);
                    self.kind = kind as u8;
                    out.push(ThrowEvent::Started);
                }
            }
        }
        out
    }
}

/// Where a released grenade starts and its velocity (WU, WU per tick,
/// engine axes): the camera plus the origin along the aim, left and up,
/// at aim x velocity / 30.
pub fn release(d: &ThrowData, camera: Vec3, aim: Vec3) -> (Vec3, Vec3) {
    let left = Vec3::Y.cross(aim).try_normalize().unwrap_or(Vec3::Y);
    let up = aim.cross(left).normalize_or_zero();
    let [i, j, k] = d.origin;
    (camera + aim * i + left * j + up * k, aim * (d.velocity / 30.0))
}

/// A cut throw's velocity at power `p` with the draw `u` (0 to 1).
pub fn cut(d: &ThrowData, full: Vec3, aim: Vec3, p: f32, u: f32) -> Vec3 {
    if p >= 1.0 {
        return full;
    }
    let [lo, hi] = d.cut_speed;
    full * p + aim * (lo + (hi - lo) * u) * (1.0 - p)
}
