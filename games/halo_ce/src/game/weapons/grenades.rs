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
//! The selected kind (weapons.md 14 "Selecting the grenade type", hud.md
//! 10): the unit's *current* type (thrown and shown), its *desired* type
//! and the input layer's *wanted* type, all none at a new life. Each tick
//! the input layer (`Throw::input`) takes wanted from desired when wanted
//! is none or empty, moves it to "the next type from wanted" on the
//! rotate-grenades press (the named action `SWITCH_ACTION`, one step per
//! press, riding the command's action bits) or when it is none or empty,
//! and makes it the desired type when it is not none; the unit's update
//! (`Throw::update`), when not busy (a throw, a melee), makes current
//! "settle from desired" when that is not none. So: frag after a stock
//! spawn, plasma after one with both kinds, the other kind once the shown
//! one runs out (when the throw ends), the empty kind kept when both are
//! out, a pickup never changing a kind that has grenades.
//!
//! The three types share the one replicated byte `Throw::selected` (two
//! bits each: current, desired, wanted; 2 is none), so the network
//! message is the one the selected kind had.
//!
//! **In the party apps** (`PartyGrenades`, registered by Halo's party
//! bundle only: `party`; the console's `halo_party_grenades <0|1>`, 0 by
//! default): off, a holder of a Halo gun there carries no grenades (a new
//! life starts with none, any it has are taken at once, so a rung change
//! and a respawn leave none, and the type stays none: the HUD draws no
//! grenade plate), and the grenade and rotate-grenades buttons do nothing;
//! on, as in Halo's own app. Halo's own app and server never register it
//! and keep the counts above. The setting is each side's own (a host's
//! change is not sent to its clients; their prediction is corrected).
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
    /// The current, desired and wanted types, two bits each from bit 0
    /// (0 frag, 1 plasma, 2 none; module doc): `selected()`, `desired()`,
    /// `wanted()`.
    pub selected: u8,
}

/// No grenade type (in `Throw::selected`'s two-bit fields).
const NONE: u8 = 2;

/// A type read from two bits.
fn kind(bits: u8) -> Option<usize> {
    (bits & 3 < NONE).then_some(usize::from(bits & 3))
}

/// The count of `k` (none read as 0).
fn count_of(counts: [u8; 2], k: Option<usize>) -> u8 {
    counts[k.unwrap_or(FRAG).min(PLASMA)]
}

/// "Next type from k" (weapons.md 14): the type other than k (none read as
/// 0) when it has grenades, else k when it has, else none.
pub fn next_from(counts: [u8; 2], k: Option<usize>) -> Option<usize> {
    let k = k.unwrap_or(FRAG).min(PLASMA);
    let other = PLASMA - k;
    if counts[other] > 0 {
        Some(other)
    } else if counts[k] > 0 {
        Some(k)
    } else {
        None
    }
}

/// "Settle from k": k when it has grenades, else the other type when it
/// has, else none.
pub fn settle_from(counts: [u8; 2], k: Option<usize>) -> Option<usize> {
    let k = k.unwrap_or(FRAG).min(PLASMA);
    if counts[k] > 0 { Some(k) } else { next_from(counts, Some(k)) }
}

impl Throw {
    /// The holder is busy throwing (its triggers read released).
    pub fn busy(&self) -> bool {
        self.frame.is_some()
    }

    /// A throw state for a new life with these counts: every type none
    /// (set by the first update).
    pub fn new(counts: [u8; 2]) -> Self {
        Self { counts, selected: NONE | NONE << 2 | NONE << 4, ..default() }
    }

    fn field(&self, shift: u8) -> Option<usize> {
        kind(self.selected >> shift)
    }

    fn set(&mut self, shift: u8, k: Option<usize>) {
        let bits = k.map_or(NONE, |k| k.min(PLASMA) as u8);
        self.selected = (self.selected & !(3 << shift)) | bits << shift;
    }

    /// The current type: thrown and shown on the HUD (even at a count of
    /// 0); none on a new life before its first update.
    pub fn selected(&self) -> Option<usize> {
        self.field(0)
    }

    /// The unit's desired type.
    pub fn desired(&self) -> Option<usize> {
        self.field(2)
    }

    /// The input layer's wanted type.
    pub fn wanted(&self) -> Option<usize> {
        self.field(4)
    }

    /// The kind a press throws: the current one, while any are left.
    pub fn current(&self) -> Option<usize> {
        self.selected().filter(|k| self.counts[*k] > 0)
    }

    /// The input layer's tick (weapons.md 14, steps 1 to 3): `press` the
    /// rotate-grenades button went down this tick.
    pub fn input(&mut self, press: bool) {
        let mut wanted = self.wanted();
        if wanted.is_none() || count_of(self.counts, wanted) == 0 {
            wanted = self.desired();
        }
        if press || wanted.is_none() || count_of(self.counts, wanted) == 0 {
            wanted = next_from(self.counts, wanted);
        }
        self.set(4, wanted);
        if wanted.is_some() {
            self.set(2, wanted);
        }
    }

    /// The unit's update: when the desired type differs from the current
    /// one and the unit is not `busy`, current becomes "settle from
    /// desired" when that is not none.
    pub fn update(&mut self, busy: bool) {
        if self.desired() != self.selected()
            && !busy
            && let Some(k) = settle_from(self.counts, self.desired())
        {
            self.set(0, Some(k));
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

/// Whether a party app's Halo players carry grenades (module doc, "In the
/// party apps"): absent in Halo's own app (they do); `false` by default
/// where Halo's party bundle registered it.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PartyGrenades(pub bool);

impl PartyGrenades {
    /// Grenades are kept from Halo's players: a party app with the switch
    /// off.
    pub fn none(this: Option<&PartyGrenades>) -> bool {
        this.is_some_and(|p| !p.0)
    }
}

/// The party bundle's grenade switch: `PartyGrenades(false)` and the
/// console's `halo_party_grenades`.
pub fn party(app: &mut App) {
    app.init_resource::<PartyGrenades>();
    chimera_core::console::ConsoleAppExt::console_command(
        app,
        "halo_party_grenades",
        "halo_party_grenades <0|1>: Halo's frag and plasma grenades for a Halo gun's holder in a party app (0 by default: none, the buttons do nothing).",
        |w, args| {
            match args.first().map(String::as_str) {
                Some("1") => w.insert_resource(PartyGrenades(true)),
                Some("0") => w.insert_resource(PartyGrenades(false)),
                _ => return Ok(Some(format!("halo_party_grenades {}", u8::from(w.get_resource::<PartyGrenades>().is_some_and(|p| p.0))))),
            }
            Ok(None)
        },
    );
}
