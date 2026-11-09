//! Combat Arms' weapon attachments ("modifications"), from
//! specs/combat_arms/attachments.md (section names below are its): what can
//! be fitted to a weapon, when, and how each part changes it.
//!
//! - **Records** (`ModRecord`, "How attachments are described in the
//!   data"): one per attachment in data/attachments/mods.toml (our numbers
//!   from the spec's tables; the install's modification file over them by
//!   name, `data::attachment_table`), held as the `CaModTable` resource.
//!   Five slots (`kinds`): muzzle, sight, magazine, bullet, mechanical
//!   sight; the first three are the socket slots.
//! - **What a weapon accepts** (`combat_arms:attachments`, `AttachParams`):
//!   its accepted list (names, each with a variant index: the part model
//!   and the magazine bonus column), its default part per slot
//!   (`DefaultScopeAttach` and friends), whether the default sight is drawn,
//!   its fire and silenced sound radii, its record's per-slot lobby state
//!   (`slot_options`) and where part models sit (`PartOffsets`). The hook puts `CaAttachSlots`
//!   (that and the weapon's stock numbers), an empty `CaFitted` and
//!   `CaMods` on the weapon.
//! - **Fitting** ("Fitting: when and where", `fit_given`): the server, the
//!   first update a weapon exists unfitted (given at spawn or respawn, or
//!   handed over), fills each slot in order 0..4 from the owner's choice
//!   (a ladder rung's `CaRungParts` on the player for its gun, else
//!   `CaLoadout` on the player, else `CaDefaultLoadout`: per weapon,
//!   a list of names, each offered for its own slot), else the weapon's
//!   default for that slot, and fits each that passes the checks (listed,
//!   `Integrated`, `Kinds` 0..4, a socket slot's `Type` 0..5); a later
//!   part for the same slot replaces an earlier one; a rejected one leaves
//!   it empty. A fitted magazine refills the clip to the new size from the
//!   total (`min(total, clip_max)`). The fitted set (`CaFitted`: five
//!   record IDs) belongs to the weapon entity: it survives switching and
//!   travels with a dropped weapon, so a pickup keeps the dropper's parts
//!   and clip (no refill) - unless the item's five slots (empty ones filled
//!   with its weapon's defaults) and sight style equal the last chosen
//!   primary's: then the carried weapon and its parts stay and only its
//!   total and clip are replaced (Corrections 2026-10-22, 1, a faithful
//!   quirk; `inventory::same_fitted_set`).
//! - **Effects** (`apply_fitted`, on both sides whenever `CaFitted`
//!   changes): `CaMods` holds the fitted records and computes every
//!   attachment term from them; the weapon's parts get theirs:
//!   - damage (`weapons::CaDamage::mods`): the base x (1 + sum
//!     `Instdamage`), the victim's special-function factor before armour
//!     (InstDamage and Crit functions whose target group and probability
//!     pass), the client ray `Range + trunc(Range x sum RangePer)`, the
//!     server's own ray `Range x (1 + the sight's RangePer)` (Q12);
//!   - spread (`weapons::CaSpread::mods`): the rate bonuses, movement
//!     targets, firing caps, zoomed cone limits and dynamic factor of "The
//!     combined formulas", Q4 (the zoomed decrease sum in the shot growth)
//!     included;
//!   - recoil (`weapons::CaRecoil::mods`): the kick scale and cap from the
//!     last fitted part in slot order (Q1, behind `RecoilLastWins`; off:
//!     summed), the acceleration, the side-kick threshold (Q11), the
//!     return's cap scaled by the muzzle part alone;
//!   - the clip (`Magazine::size` = `ShotsPerClip` + the magazine's
//!     `ShotsPerClipPlus` in the weapon's column; the reserve's ceiling
//!     shrinks or grows with it, so the total stays `MaxAmount`: Q3) and
//!     the reload (a shell reload's three parts too), draw and fire cycle
//!     times (length / rate, the rates summed over the socket slots with
//!     the gear's, `gear::CaGearRates`: the vest's and the sets'; played
//!     at 1 above 2.0, clamped to `MIN_RATE` at or below 0);
//!   - zoom (`weapons::CaZoom::scope`): the sight's levels and FOVs; no
//!     sight, no zoom (Q10); the sight's sway additions.
//! - **Stun** (`movement::StunPenalty`): a player hit by another team's
//!   player gets the shooter's passing Stun functions (target Player, PH,
//!   PZ, Human or Zombie; never All, Npc, NH, NZ: Q8) as movement's
//!   P_stun for their duration. Knockback functions add to the ammo's
//!   knock-back of an infected victim.
//! - **Sound, HUD, models** (read by others): `CaMods::fire_sound`
//!   (silenced sound level and radius), `observer_sound` and
//!   `muzzle_light` (what others hear and see), `hearing_radius` (Q6),
//!   the sight's style for the HUD's range readout and gauge
//!   (`gauge_image`; ui::hud draws them), `part_models` (socket and
//!   variant per socket slot; the part models drawn on the view model,
//!   `view_model::parts`, and the held model,
//!   `characters::models::load_held_weapon`, from the records'
//!   `AttachModel#`/`AttachHHModel#` fields and the weapon's
//!   `PartOffsets`).
//!
//! Over the network the fitted set is a predicted weapon part (`CaFitted`,
//! `weapon::NetPart`): the server fits, the client takes the five IDs with
//! its player's state and derives the rest itself (`apply_fitted` runs in
//! the predicted tick, so a replay sees the parts). The stun goes to the
//! victim's client as a predicted component.
//!
//! Known shortcuts (the spec's most literal reading where it is unclear,
//! listed for its open questions):
//! - Numbers are re-summed when the fitted set changes, not at each use:
//!   the same values, as the set is fixed for the weapon's life ("When the
//!   numbers apply").
//! - The lobby isn't there: a player's parts are `CaLoadout` (Open question
//!   6), the `CHIMERA_CA_MODS` environment variable for the app
//!   (`weapon=Part,Part;weapon=...`). The per-slot option fields aren't
//!   read.
//! - Records the spec doesn't give per weapon: the accepted lists of the
//!   MP5A4, M60, M92FS and 870 are empty (the install's file fills them);
//!   the M4A1 lists the AR family's 51 (12 bullets, where the spec counts
//!   nine); the L96A1 the SR family's silencers, bullets and mechanical
//!   sights and `Magazine_C_SR` at column 0; the SR magazines' reload rates
//!   are the rifles'; magazine columns the spec doesn't give are 0; the
//!   Gold ITECH sight's range readout has `TargetRange` 10000.
//! - The special-function probability draw is the round's random stream
//!   (`CaSpecialRoll` forces it); the stun is applied where the round is
//!   delivered (the server), not on the victim's client, and sent to it;
//!   nothing is an AI yet (`CaNpc` marks one for the target groups); "Poison
//!   or stun forces mu S to 1" (weapon_firing.md) isn't done.
//! - The parts' muzzle effects, tracers and fire sounds are `effects`'
//!   (effects_sounds.md). Not done: flash hiders (no stock record names
//!   one), zoom sounds (their files aren't in a spec), the overlay
//!   picture (Chimera's placeholder overlay), dot-sight mode (`Type` 2, Q7:
//!   no record has it), `VectorsPerRoundPer`, `InstDamagePer` (no
//!   secondary attacks), the client ammo ceiling `MaxAmountPlus` (Q3:
//!   invisible), gear mods (`Kinds` 5..7), set items (their animation rates are applied: `gear`) and the gunsmith
//!   conversion.
//! - A loose weapon placed by a map or a test (no owner) is fitted with
//!   its defaults and keeps its clip.

use std::sync::OnceLock;

use bevy::prelude::*;
use chimera_sim::server_docs::DocsAppExt;
use serde::{Deserialize, Serialize};

use super::{
    gear::CaGearRates,
    inventory::{CaItem, played_rate},
    movement::StunPenalty,
    weapons::{CaDamage, CaRecoil, CaSpread, CaZoom, Scope},
};
use crate::{
    core::{PredictedAppExt, Predict},
    weapon::{CarriedAppExt, Magazine, NetPart, SelectWeapons, ShellReload, Trigger, Weapon, data::WeaponDataAppExt},
};

/// The hook's name (docs/weapon-data.md, "Hooks").
pub const ATTACHMENTS: &str = "combat_arms:attachments";

// Constants (the spec's "Constants").

/// Slots: muzzle, sight, magazine, bullet, mechanical sight (`Kinds`).
pub const SLOTS: usize = 5;
pub const MUZZLE: usize = 0;
pub const SIGHT: usize = 1;
pub const MAGAZINE: usize = 2;
pub const BULLET: usize = 3;
pub const MECH_SIGHT: usize = 4;
/// Slots 0..2 may carry a model and count for animation rates.
pub const SOCKET_SLOTS: usize = 3;
/// Model variants (and magazine bonus columns) per attachment.
pub const MODEL_VARIANTS: usize = 10;
/// Most `ZoomFov#` entries read.
pub const ZOOM_LEVELS_MAX: usize = 10;
/// Silencer levels used: 1..=9 (of 10 sound entries).
pub const SILENCER_LEVELS: std::ops::RangeInclusive<i32> = 1..=9;
/// The gauge sight's images step by this, %.
pub const GAUGE_STEP: i32 = 10;
/// Ours ("Edge cases"): a summed rate at or below 0 plays at this.
pub const MIN_RATE: f32 = 0.01;
/// `Type` values.
pub const SILENCER: i32 = 0;
pub const SCOPE: i32 = 1;
pub const DOT_SIGHT: i32 = 2;
pub const FLASH_HIDER: i32 = 3;
pub const MUZZLE_ATTACHMENT: i32 = 4;
pub const MAGAZINE_TYPE: i32 = 5;

// ---------------------------------------------------------------------------
// Records

/// A special function's kind (`SFType#`).
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum SpecialKind {
    InstDamage,
    Stun,
    Knockback,
    Crit,
}

/// A special function's target group (`SFTargetType#`), by the victim.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum Target {
    #[default]
    All,
    Player,
    Npc,
    Human,
    Zombie,
    PH,
    PZ,
    NH,
    NZ,
}

/// What the target groups test of a victim.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Victim {
    /// An AI (`CaNpc`), not a player.
    pub npc: bool,
    /// An infected player or a zombie AI.
    pub zombie: bool,
}

impl Target {
    /// Whether `v` is in this group ("Bullet slot", target groups).
    pub fn matches(self, v: Victim) -> bool {
        match self {
            Target::All => true,
            Target::Player => !v.npc,
            Target::Npc => v.npc,
            Target::Human => !v.zombie,
            Target::Zombie => v.zombie,
            Target::PH => !v.npc && !v.zombie,
            Target::PZ => !v.npc && v.zombie,
            Target::NH => v.npc && !v.zombie,
            Target::NZ => v.npc && v.zombie,
        }
    }

    /// Whether a Stun or Knockback function of this group can apply on a
    /// player victim's client (Q8: All, Npc, NH and NZ never do).
    pub fn reaches_player_client(self) -> bool {
        !matches!(self, Target::All | Target::Npc | Target::NH | Target::NZ)
    }
}

fn one() -> f32 {
    1.0
}

/// One special function of an attachment (`SF*#`).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Special {
    #[serde(rename = "type")]
    pub kind: SpecialKind,
    #[serde(default)]
    pub target: Target,
    #[serde(default)]
    pub value: f32,
    /// s.
    #[serde(default)]
    pub duration: f32,
    /// q: passes when a uniform draw in [0, 1] is <= q; q <= 0 always.
    #[serde(default = "one")]
    pub probability: f32,
}

impl Special {
    pub fn passes(&self, draw: f32) -> bool {
        self.probability <= 0.0 || draw <= self.probability
    }
}

/// An attachment record (`Mod#`): the fields that do anything, in the
/// data's snake case; missing is 0 (or empty).
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ModRecord {
    /// `Name`: unique, the target of every link.
    pub name: String,
    /// `Kinds`: the slot, 0..4 (5..7 gear mods); none: never fitted.
    pub kinds: Option<i32>,
    /// `Type`: 0 silencer, 1 scope, 2 dot sight, 3 flash hider, 4 muzzle
    /// attachment, 5 magazine; none: unknown.
    #[serde(rename = "type")]
    pub kind: Option<i32>,
    /// `Integrated`: 0 never fitted.
    pub integrated: i32,
    /// `Socket`: where its model goes on the gun (none: no model).
    pub socket: Option<String>,
    /// `AttachModel0..9`: its first-person model per variant (an install
    /// path; empty: none), `AttachSkin<variant>_<texture index>` its
    /// skins, `AttachRenderStyle#` its pieces' render styles,
    /// `AttachScale` its scale (none: 1).
    pub attach_models: Vec<String>,
    pub attach_skins: Vec<Vec<String>>,
    pub attach_render_styles: Vec<String>,
    pub attach_scale: Option<[f32; 3]>,
    /// The third-person part: `HHModelSocket`, `AttachHHModel#`,
    /// `AttachHHSkin#_#`, `HHRenderStyle#`, `AttachHHModelScale`.
    pub hh_socket: Option<String>,
    pub attach_hh_models: Vec<String>,
    pub attach_hh_skins: Vec<Vec<String>>,
    pub hh_render_styles: Vec<String>,
    pub attach_hh_scale: Option<[f32; 3]>,
    /// `SilencerLevel`, `FlashHiderLevel` (1..9).
    pub silencer_level: i32,
    pub flash_hider_level: i32,
    /// `ZoomLevel` and `ZoomFov0..9` (degrees).
    pub zoom_level: i32,
    pub zoom_fov: Vec<f32>,
    /// `ZoomSpriteName`: the overlay picture while zoomed (an install path;
    /// empty: none).
    pub zoom_sprite: String,
    /// `ZoomSpriteScale` (0: missing, the reader's 0.001).
    pub zoom_sprite_scale: f32,
    /// `SightSystemStyle` (0 normal, 1 range readout, 2 gauge) and
    /// `TargetRange` (units).
    pub sight_style: i32,
    pub target_range: i32,
    /// `Instdamage`, `InstDamagePer`, `RangePer`: fractions.
    pub inst_damage: f32,
    pub inst_damage_per: f32,
    pub range_per: f32,
    /// `Fireanirate`, `Reloadanirate`, `Selectanirate`.
    pub fire_ani_rate: f32,
    pub reload_ani_rate: f32,
    pub select_ani_rate: f32,
    /// Spread: `PerturbIncSpeed`, `PerturbDecSpeed`,
    /// `ZoomedPerturbIncSpeed`, `ZoomedPerturbDecSpeed`, `WalkPerturb`,
    /// `RunPerturb`, `FirePerturb`, `FireDuckPerturb`, `ZoomedMinPerturb`,
    /// `ZoomedMaxPerturb`, `DynamicPerturbFactor`, `MinPerturbPer` (the
    /// crosshair's only).
    pub perturb_inc_speed: f32,
    pub perturb_dec_speed: f32,
    pub zoomed_perturb_inc_speed: f32,
    pub zoomed_perturb_dec_speed: f32,
    pub walk_perturb: f32,
    pub run_perturb: f32,
    pub fire_perturb: f32,
    pub fire_duck_perturb: f32,
    pub zoomed_min_perturb: f32,
    pub zoomed_max_perturb: f32,
    pub dynamic_perturb_factor: f32,
    pub min_perturb_per: f32,
    /// Recoil: `BaseCamRecoilPitch`, `BaseCamRecoilPitchPer`,
    /// `MaxCamRecoilPitchPer` (fractions, last fitted wins: Q1),
    /// `BaseCamRecoilAccelPitch` and `MaxRandomPitch` (added).
    pub base_cam_recoil_pitch: f32,
    pub base_cam_recoil_pitch_per: f32,
    pub max_cam_recoil_pitch_per: f32,
    pub base_cam_recoil_accel_pitch: f32,
    pub max_random_pitch: f32,
    /// `CameraSwayXSpeed`, `CameraSwayYSpeed`, `CameraSwayXFreq`,
    /// `CameraSwayYFreq`: added to the weapon's sway.
    pub camera_sway: [f32; 4],
    /// `VectorsPerRoundPer`: the client reads the magazine part's, the
    /// server the muzzle part's (Corrections 1; no stock record has one).

    pub vectors_per_round_per: f32,
    /// `ShotsPerClipPlus0..9`, `MaxAmountPlus0..9`,
    /// `SelectionAmountPlus0..9`: one per column.
    pub shots_per_clip_plus: Vec<i32>,
    pub max_amount_plus: Vec<i32>,
    pub selection_amount_plus: Vec<i32>,
    /// `SF*#`.
    pub special: Vec<Special>,
}

impl ModRecord {
    /// The slot it occupies (its `Kinds`, 0..4); None for anything else.
    pub fn slot(&self) -> Option<usize> {
        self.kinds.filter(|k| (0..SLOTS as i32).contains(k)).map(|k| k as usize)
    }

    /// Whether it can go on a weapon at all ("Which weapon accepts which"
    /// 2 to 4): integrated, a weapon slot, and in a socket slot a `Type`
    /// 0..5 (ours on both sides, "Edge cases").
    pub fn weapon_part(&self) -> bool {
        let Some(slot) = self.slot() else { return false };
        self.integrated != 0 && (slot >= SOCKET_SLOTS || self.kind.is_some_and(|t| (0..=MAGAZINE_TYPE).contains(&t)))
    }

    /// Its first-person model for variant `c` and that variant's skins
    /// (None: no model there).
    pub fn view_model(&self, c: usize) -> Option<(&str, &[String])> {
        let m = self.attach_models.get(c).filter(|m| !m.is_empty())?;
        Some((m, self.attach_skins.get(c).map_or(&[][..], Vec::as_slice)))
    }

    /// Its third-person model for variant `c` and its skins.
    pub fn held_model(&self, c: usize) -> Option<(&str, &[String])> {
        let m = self.attach_hh_models.get(c).filter(|m| !m.is_empty())?;
        Some((m, self.attach_hh_skins.get(c).map_or(&[][..], Vec::as_slice)))
    }

    /// Column `c` of a per-variant list (0 outside 0..9 or missing).
    fn column(list: &[i32], c: Option<usize>) -> i32 {
        c.filter(|c| *c < MODEL_VARIANTS).and_then(|c| list.get(c).copied()).unwrap_or(0)
    }

    /// The scope it gives: `ZoomLevel` levels at `ZoomFov0..` (a level past
    /// the entries stays at the previous FOV, "Edge cases"); None for a
    /// part that isn't a zooming scope (`Type` 1).
    pub fn scope(&self) -> Option<Scope> {
        if self.kind != Some(SCOPE) || self.zoom_level <= 0 {
            return None;
        }
        let levels = (self.zoom_level as usize).min(ZOOM_LEVELS_MAX);
        let mut fov = Vec::with_capacity(levels);
        let mut last = super::weapons::zoom::FOV_X;
        for i in 0..levels {
            last = self.zoom_fov.get(i).copied().unwrap_or(last);
            fov.push(last);
        }
        Some(Scope {
            name: self.name.clone(),
            fov,
            sprite: self.zoom_sprite.clone(),
            sprite_scale: self.zoom_sprite_scale,
        })
    }
}

/// Parse an attachments document (data/attachments/mods.toml's format):
/// `[[mod]]` records, a `name` with `{class}` expanded once per
/// `classes` entry.
pub fn parse_mods(text: &str) -> Result<Vec<ModRecord>, String> {
    let mut doc: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    let entries = match doc.remove("mod") {
        Some(toml::Value::Array(a)) => a,
        Some(_) => return Err("`mod` must be an array of tables".into()),
        None => Vec::new(),
    };
    if let Some(k) = doc.keys().next() {
        return Err(format!("unknown key `{k}`"));
    }
    let mut out = Vec::new();
    for (i, entry) in entries.into_iter().enumerate() {
        let toml::Value::Table(mut t) = entry else {
            return Err(format!("mod {i}: not a table"));
        };
        let classes: Vec<String> = match t.remove("classes") {
            Some(v) => v.try_into().map_err(|e| format!("mod {i}: classes: {e}"))?,
            None => vec![String::new()],
        };
        let record: ModRecord = toml::Value::Table(t)
            .try_into()
            .map_err(|e| format!("mod {i}: {e}"))?;
        if record.name.is_empty() {
            return Err(format!("mod {i}: no name"));
        }
        if classes.len() > 1 || !classes[0].is_empty() {
            if !record.name.contains("{class}") {
                return Err(format!("mod {}: `classes` without {{class}} in the name", record.name));
            }
        }
        for c in &classes {
            out.push(ModRecord {
                name: record.name.replace("{class}", c),
                ..record.clone()
            });
        }
    }
    Ok(out)
}

/// The built-in attachment data (data/attachments/mods.toml).
pub const MOD_DATA: &str = include_str!("../../data/attachments/mods.toml");

/// Every attachment record there is, in order (an index is a record's
/// ID on the network).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaModTable {
    pub records: Vec<ModRecord>,
}

impl CaModTable {
    /// The built-in records (bad built-in data panics; the tests load it).
    pub fn built_in() -> &'static CaModTable {
        static TABLE: OnceLock<CaModTable> = OnceLock::new();
        TABLE.get_or_init(|| CaModTable {
            records: parse_mods(MOD_DATA).unwrap_or_else(|e| panic!("Combat Arms attachments mods.toml: {e}")),
        })
    }

    /// A record's ID by name (case-insensitive, first match).
    pub fn find(&self, name: &str) -> Option<u16> {
        if name.is_empty() {
            return None;
        }
        self.records
            .iter()
            .position(|r| r.name.eq_ignore_ascii_case(name))
            .map(|i| i as u16)
    }

    pub fn get(&self, id: u16) -> Option<&ModRecord> {
        self.records.get(id as usize)
    }

    /// Add `r`, replacing a record of the same name (an override).
    pub fn insert(&mut self, r: ModRecord) {
        match self.find(&r.name) {
            Some(i) => self.records[i as usize] = r,
            None => self.records.push(r),
        }
    }
}

// ---------------------------------------------------------------------------
// The weapon's side

/// An entry of a weapon's accepted list: a name, or a name with its
/// variant index (`ModModel#`, default 0).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Accepted {
    Name(String),
    Variant(AcceptedVariant),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AcceptedVariant {
    pub name: String,
    pub variant: i32,
}

impl Accepted {
    pub fn name(&self) -> &str {
        match self {
            Accepted::Name(n) => n,
            Accepted::Variant(v) => &v.name,
        }
    }

    pub fn variant(&self) -> i32 {
        match self {
            Accepted::Name(_) => 0,
            Accepted::Variant(v) => v.variant,
        }
    }
}

/// A weapon's default part per slot (`DefaultMuzzleAttach`,
/// `DefaultScopeAttach`, `DefaultMagazineAttach`, `DefaultBulletAttach`,
/// `DefaultMechSightAttach`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Defaults {
    pub muzzle: Option<String>,
    pub sight: Option<String>,
    pub magazine: Option<String>,
    pub bullet: Option<String>,
    pub mech_sight: Option<String>,
}

impl Defaults {
    pub fn get(&self, slot: usize) -> Option<&str> {
        match slot {
            MUZZLE => self.muzzle.as_deref(),
            SIGHT => self.sight.as_deref(),
            MAGAZINE => self.magazine.as_deref(),
            BULLET => self.bullet.as_deref(),
            MECH_SIGHT => self.mech_sight.as_deref(),
            _ => None,
        }
        .filter(|n| !n.is_empty())
    }
}

fn yes() -> bool {
    true
}
fn fire_radius() -> i32 {
    1000
}

/// `combat_arms:attachments`: what a weapon accepts and its own numbers
/// the parts read.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AttachParams {
    /// The accepted list (the modification file's `ModName#`/`ModModel#`).
    #[serde(default)]
    pub accepts: Vec<Accepted>,
    #[serde(default)]
    pub defaults: Defaults,
    /// `DefaultScopeVisible`: the default sight's model is drawn.
    #[serde(default = "yes")]
    pub default_sight_visible: bool,
    /// `FireSndRadius`, units.
    #[serde(default = "fire_radius")]
    pub fire_sound_radius: i32,
    /// `SilencedFireRadius1..10`, units.
    #[serde(default)]
    pub silenced_radius: Vec<i32>,
    /// The modification record's per-slot lobby state, slot order
    /// (`MuzzleOPtion`, `SightOption`, `MagazineOption`, `BulletOption`,
    /// `MechSightOption`): 0 no part goes there, 1 free, 2 taken by a
    /// built-in part, -1 unset; empty: not known. Fitting doesn't read it
    /// (gameplay checks the list); a rung's draw does (`rungs`).
    #[serde(default)]
    pub slot_options: Vec<i32>,
    /// Where its parts' models sit at their sockets.
    #[serde(default)]
    pub part_offsets: PartOffsets,
}

/// A part model's place at its socket, in the socket's frame (units;
/// the rotation as the data gives it, radians: yaw, pitch, roll).
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct PartOffset {
    pub pos: [f32; 3],
    pub rot: [f32; 3],
}

/// The weapon's offsets for its parts' models ("View model and world
/// model"): `MuzzlePVPosOff`/`RotOff` and `ScopePVPosOff`/`RotOff` (first
/// person), `MuzzleHHPosOff`/`RotOff` and `ScopeHHPosOff`/`RotOff` (third
/// person).
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct PartOffsets {
    pub muzzle_pv: PartOffset,
    pub scope_pv: PartOffset,
    pub muzzle_hh: PartOffset,
    pub scope_hh: PartOffset,
}

impl PartOffsets {
    /// Slot `slot`'s offset, first person (`pv`) or third: the muzzle's
    /// for the muzzle slot, the scope's for the sight (none for others).
    pub fn of(&self, slot: usize, pv: bool) -> PartOffset {
        match (slot, pv) {
            (MUZZLE, true) => self.muzzle_pv,
            (SIGHT, true) => self.scope_pv,
            (MUZZLE, false) => self.muzzle_hh,
            (SIGHT, false) => self.scope_hh,
            _ => PartOffset::default(),
        }
    }
}

impl Default for AttachParams {
    fn default() -> Self {
        Self {
            accepts: Vec::new(),
            defaults: Defaults::default(),
            default_sight_visible: true,
            fire_sound_radius: fire_radius(),
            silenced_radius: Vec::new(),
            slot_options: Vec::new(),
            part_offsets: PartOffsets::default(),
        }
    }
}

impl AttachParams {
    /// The variant index of `name` in the accepted list (first entry wins;
    /// None: not listed).
    pub fn variant_of(&self, name: &str) -> Option<i32> {
        self.accepts
            .iter()
            .find(|a| a.name().eq_ignore_ascii_case(name))
            .map(Accepted::variant)
    }
}

/// A weapon's stock numbers the parts change (from its data, before any
/// part).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stock {
    /// `ShotsPerClip`, the reserve's ceiling, the reload's length (s).
    pub clip: u32,
    pub reserve_max: u32,
    pub reload: f32,
    /// The fire cycle and the draw (select animation) lengths, s.
    pub cycle: f32,
    pub draw: f32,
    /// A shell-by-shell reload's start, each shell and finish
    /// (`ShellReload`), s: at the reload rate too.
    pub shell: Option<(f32, f32, f32)>,
    /// The trigger's closing part (`Trigger::postfire`, the `postfire`
    /// animation), s: at the fire rate.
    pub postfire: f32,
}

/// What a weapon accepts (`combat_arms:attachments`) and its stock numbers.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct CaAttachSlots {
    pub params: AttachParams,
    pub stock: Stock,
}

/// The parts fitted to a weapon: a record ID per slot (`CaModTable`),
/// and whether it has been through fitting. The server's; predicted with
/// the weapon.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaFitted {
    pub slots: [Option<u16>; SLOTS],
    pub given: bool,
}

impl NetPart for CaFitted {
    type Net = ([Option<u16>; SLOTS], bool);
    fn net(&self) -> Self::Net {
        (self.slots, self.given)
    }
    fn set_net(&mut self, (slots, given): Self::Net) {
        // Only on a change: the parts are derived from it.
        if self.slots != slots || self.given != given {
            self.slots = slots;
            self.given = given;
        }
    }
}

/// One fitted part: its record and the weapon's variant index for it.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub id: u16,
    /// The column (None: outside 0..9).
    pub variant: Option<usize>,
    pub record: ModRecord,
    /// The weapon's default for its slot.
    pub default: bool,
}

/// A weapon's fitted parts as records (derived from `CaFitted`), with every
/// attachment term of "The combined formulas".
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct CaMods {
    pub parts: [Option<Part>; SLOTS],
    /// The weapon's `AttachParams` sound radii and `DefaultScopeVisible`.
    pub fire_sound_radius: i32,
    pub silenced_radius: Vec<i32>,
    pub default_sight_visible: bool,
}

/// What the shooter's own shot sounds like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FireSound {
    /// The silenced sound's level (`SilencedFireSnd`<level>); None: the
    /// plain `FireSnd`.
    pub silenced: Option<i32>,
    /// Audible radius, units.
    pub radius: i32,
}

/// What another player hears of a shot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObserverSound {
    Plain,
    Silenced { level: i32 },
    /// A silenced type with no usable level: nothing.
    Nothing,
}

/// Where a part's model goes (`part_models`).
#[derive(Clone, Debug, PartialEq)]
pub struct PartModel {
    pub slot: usize,
    /// The part's record (`Name`).
    pub name: String,
    pub socket: String,
    /// `AttachModel<variant>`.
    pub variant: usize,
    /// Drawn (a default sight with `DefaultScopeVisible` 0 isn't).
    pub visible: bool,
}

/// The spread terms (`weapons::CaSpread::mods`): sums over the fitted set.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpreadMods {
    pub inc: f32,
    pub dec: f32,
    pub zoomed_inc: f32,
    pub zoomed_dec: f32,
    pub walk: f32,
    pub run: f32,
    pub fire: f32,
    pub fire_duck: f32,
    pub zoomed_min: f32,
    pub zoomed_max: f32,
    pub dynamic: f32,
}

/// The recoil terms (`weapons::CaRecoil::mods`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RecoilMods {
    /// p_b and p_J (Q1).
    pub pitch: f32,
    pub max_pitch: f32,
    /// Added to `BaseCamRecoilAccelPitch` and `MaxRandomPitch`.
    pub accel: f32,
    pub random_pitch: f32,
    /// The muzzle part's `MaxCamRecoilPitchPer`: the return's cap.
    pub return_max: f32,
}

/// The damage terms (`weapons::CaDamage::mods`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DamageMods {
    /// Sum of `Instdamage`.
    pub damage: f32,
    /// Sum of `RangePer`, and the sight's alone (the server's ray).
    pub range_per: f32,
    pub sight_range_per: f32,
    /// Every fitted part's special functions, slot order.
    pub specials: Vec<Special>,
    /// `VectorsPerRoundPer` as each side reads it (Corrections 1): the
    /// client's rays from the magazine part's, the server's expected count
    /// from the muzzle part's.
    pub pellets_client: f32,
    pub pellets_server: f32,
    /// The summed reload rate bonus (the server's reload window).
    pub reload_rate: f32,
}

impl DamageMods {
    /// A shot's rays for `n` (`VectorsPerRound`) with a pellet fraction
    /// `v`: n + floor(n v).
    pub fn rays(&self, n: u32, v: f32) -> u32 {
        (n as i64 + (n as f64 * v as f64).floor() as i64).max(0) as u32
    }

    /// The client ray (units): Range + trunc(Range x sum RangePer).
    pub fn client_range(&self, range: f64) -> f64 {
        range + (range * self.range_per as f64).trunc()
    }

    /// The server's own ray (units): Range x (1 + the sight's RangePer).
    pub fn server_range(&self, range: f64) -> f64 {
        range * (1.0 + self.sight_range_per as f64)
    }

    /// The victim-side factor (step 3): 1 + the InstDamage and Crit
    /// values whose target and probability pass, `draw(j)` the j-th
    /// function's uniform draw.
    pub fn victim_factor(&self, victim: Victim, mut draw: impl FnMut(usize) -> f32) -> f32 {
        let mut sum = 0.0;
        for (j, s) in self.specials.iter().enumerate() {
            if matches!(s.kind, SpecialKind::InstDamage | SpecialKind::Crit)
                && s.target.matches(victim)
                && s.passes(draw(j))
            {
                sum += s.value;
            }
        }
        1.0 + sum
    }

    /// The Stun (or Knockback) functions that apply to `victim`: (summed
    /// value, summed duration), each passing its target and probability
    /// (on a player, never All, Npc, NH or NZ: Q8).
    pub fn stun(&self, kind: SpecialKind, victim: Victim, mut draw: impl FnMut(usize) -> f32) -> Option<(f32, f32)> {
        let mut out: Option<(f32, f32)> = None;
        for (j, s) in self.specials.iter().enumerate() {
            if s.kind != kind || !s.target.matches(victim) || (!victim.npc && !s.target.reaches_player_client()) {
                continue;
            }
            if s.passes(draw(j)) {
                let o = out.get_or_insert((0.0, 0.0));
                o.0 += s.value;
                o.1 += s.duration;
            }
        }
        out
    }
}

impl CaMods {
    pub fn fitted(&self) -> impl Iterator<Item = &Part> {
        self.parts.iter().flatten()
    }

    pub fn part(&self, slot: usize) -> Option<&Part> {
        self.parts.get(slot).and_then(Option::as_ref)
    }

    /// Sum of a field over the fitted set.
    pub fn sum(&self, f: impl Fn(&ModRecord) -> f32) -> f32 {
        self.fitted().map(|p| f(&p.record)).sum()
    }

    /// Sum over the socket slots only.
    pub fn socket_sum(&self, f: impl Fn(&ModRecord) -> f32) -> f32 {
        self.parts[..SOCKET_SLOTS].iter().flatten().map(|p| f(&p.record)).sum()
    }

    pub fn damage(&self) -> DamageMods {
        DamageMods {
            damage: self.sum(|r| r.inst_damage),
            range_per: self.sum(|r| r.range_per),
            sight_range_per: self.part(SIGHT).map_or(0.0, |p| p.record.range_per),
            specials: self.fitted().flat_map(|p| p.record.special.iter().cloned()).collect(),
            pellets_client: self.part(MAGAZINE).map_or(0.0, |p| p.record.vectors_per_round_per),
            pellets_server: self.part(MUZZLE).map_or(0.0, |p| p.record.vectors_per_round_per),
            reload_rate: self.socket_sum(|r| r.reload_ani_rate),

        }
    }

    pub fn spread(&self) -> SpreadMods {
        SpreadMods {
            inc: self.sum(|r| r.perturb_inc_speed),
            dec: self.sum(|r| r.perturb_dec_speed),
            zoomed_inc: self.sum(|r| r.zoomed_perturb_inc_speed),
            zoomed_dec: self.sum(|r| r.zoomed_perturb_dec_speed),
            walk: self.sum(|r| r.walk_perturb),
            run: self.sum(|r| r.run_perturb),
            fire: self.sum(|r| r.fire_perturb),
            fire_duck: self.sum(|r| r.fire_duck_perturb),
            zoomed_min: self.sum(|r| r.zoomed_min_perturb),
            zoomed_max: self.sum(|r| r.zoomed_max_perturb),
            dynamic: self.sum(|r| r.dynamic_perturb_factor),
        }
    }

    /// The recoil terms; `last_wins`: Q1 (the kick fractions of the last
    /// fitted part in slot order), else summed.
    pub fn recoil(&self, last_wins: bool) -> RecoilMods {
        let (mut pitch, mut max_pitch) = (0.0, 0.0);
        for p in self.fitted() {
            let r = &p.record;
            let (pb, pj) = (r.base_cam_recoil_pitch + r.base_cam_recoil_pitch_per, r.max_cam_recoil_pitch_per);
            if last_wins {
                (pitch, max_pitch) = (pb, pj);
            } else {
                pitch += pb;
                max_pitch += pj;
            }
        }
        RecoilMods {
            pitch,
            max_pitch,
            accel: self.sum(|r| r.base_cam_recoil_accel_pitch),
            random_pitch: self.sum(|r| r.max_random_pitch),
            return_max: self.part(MUZZLE).map_or(0.0, |p| p.record.max_cam_recoil_pitch_per),
        }
    }

    /// The fitted magazine's `ShotsPerClipPlus` in the weapon's column.
    pub fn clip_plus(&self) -> i32 {
        self.part(MAGAZINE)
            .map_or(0, |p| ModRecord::column(&p.record.shots_per_clip_plus, p.variant))
    }

    /// The client's ammo ceiling bonus (`MaxAmountPlus`; Q3: no effect).
    pub fn max_amount_plus(&self) -> i32 {
        self.part(MAGAZINE)
            .map_or(0, |p| ModRecord::column(&p.record.max_amount_plus, p.variant))
    }

    /// clip_max for a weapon of `ShotsPerClip` `clip`.
    pub fn clip_max(&self, clip: u32) -> u32 {
        (clip as i64 + self.clip_plus() as i64).max(0) as u32
    }

    /// The summed rates (socket slots): fire, reload, draw.
    pub fn rates(&self) -> (f32, f32, f32) {
        (
            self.socket_sum(|r| r.fire_ani_rate),
            self.socket_sum(|r| r.reload_ani_rate),
            self.socket_sum(|r| r.select_ani_rate),
        )
    }

    /// The sight's scope (None: no zoom).
    pub fn scope(&self) -> Option<Scope> {
        self.part(SIGHT).and_then(|p| p.record.scope())
    }

    /// The sight's (`SightSystemStyle`, `TargetRange`).
    pub fn sight_style(&self) -> (i32, i32) {
        self.part(SIGHT).map_or((0, 0), |p| (p.record.sight_style, p.record.target_range))
    }

    /// The range readout's reach (units) when the sight has one (style 1,
    /// `TargetRange` > 0).
    pub fn range_readout(&self) -> Option<i32> {
        let (style, range) = self.sight_style();
        (style == 1 && range > 0).then_some(range)
    }

    /// Whether the sight is a gauge sight (style 2).
    pub fn gauge(&self) -> bool {
        self.sight_style().0 == 2
    }

    /// The muzzle part's silencer level, when usable (1..9).
    pub fn silencer_level(&self) -> Option<i32> {
        self.part(MUZZLE)
            .map(|p| p.record.silencer_level)
            .filter(|l| SILENCER_LEVELS.contains(l))
    }

    /// The shooter's own shot ("Sound"): the silenced sound and radius of
    /// the muzzle part's level, else the plain one.
    pub fn fire_sound(&self) -> FireSound {
        match self
            .silencer_level()
            .and_then(|l| self.silenced_radius.get(l as usize - 1).map(|r| (l, *r)))
        {
            Some((level, radius)) => FireSound {
                silenced: Some(level),
                radius,
            },
            None => FireSound {
                silenced: None,
                radius: self.fire_sound_radius,
            },
        }
    }

    /// What others hear: silenced when the muzzle part's `Type` is a
    /// silencer or a muzzle attachment, then by its level (none usable:
    /// nothing).
    pub fn observer_sound(&self) -> ObserverSound {
        let silenced_type = self
            .part(MUZZLE)
            .and_then(|p| p.record.kind)
            .is_some_and(|t| t == SILENCER || t == MUZZLE_ATTACHMENT);
        match (silenced_type, self.silencer_level()) {
            (false, _) => ObserverSound::Plain,
            (true, Some(level)) => ObserverSound::Silenced { level },
            (true, None) => ObserverSound::Nothing,
        }
    }

    /// Others see the muzzle light (not of a silenced shot).
    pub fn muzzle_light(&self) -> bool {
        self.observer_sound() == ObserverSound::Plain
    }

    /// The AI hearing radius (Q6: entry level + 1).
    pub fn hearing_radius(&self) -> i32 {
        self.silencer_level()
            .and_then(|l| self.silenced_radius.get(l as usize).copied())
            .unwrap_or(self.fire_sound_radius)
    }

    /// The part models ("View model and world model"): each socket slot's
    /// part with a socket and a variant in 0..9.
    pub fn part_models(&self) -> Vec<PartModel> {
        self.parts[..SOCKET_SLOTS]
            .iter()
            .enumerate()
            .filter_map(|(slot, p)| {
                let p = p.as_ref()?;
                let socket = p.record.socket.clone().filter(|s| !s.is_empty())?;
                Some(PartModel {
                    slot,
                    name: p.record.name.clone(),
                    socket,
                    variant: p.variant?,
                    visible: !(slot == SIGHT && p.default && !self.default_sight_visible),
                })
            })
            .collect()
    }

    /// The crosshair's attachment terms (`hud_crosshair.md` A3): each
    /// fitted part's (`MinPerturbPer`, `ZoomedMaxPerturb`), slot order.
    pub fn crosshair_terms(&self) -> Vec<(i32, i32)> {
        self.fitted()
            .map(|p| (p.record.min_perturb_per as i32, p.record.zoomed_max_perturb as i32))
            .collect()
    }

    /// The sight's sway additions (x speed, y speed, x freq, y freq).
    pub fn sway(&self) -> [f32; 4] {
        let mut s = [0.0; 4];
        for p in self.fitted() {
            for (a, b) in s.iter_mut().zip(p.record.camera_sway) {
                *a += b;
            }
        }
        s
    }
}

/// The gauge sight's image number for an integer percentage `p`
/// ("Crosshair, HUD and overlay"): 0, 10, ..., 100, the nearest ten, 5
/// rounding up.
pub fn gauge_image(p: i32) -> i32 {
    (p + 5) - (p + 5).rem_euclid(GAUGE_STEP)
}

/// The rate an animation plays at for a summed rate (the cap of
/// weapon_firing.md Q6; at or below 0, `MIN_RATE`: "Edge cases").
pub fn animation_rate(rate: f32) -> f32 {
    if rate <= 0.0 {
        warn!("Combat Arms: an animation rate of {rate} (attachments); played at {MIN_RATE}");
        MIN_RATE
    } else {
        played_rate(rate)
    }
}

/// The fire animation rate (view_models.md Corrections 6): 1 + the weapon
/// record's `FireAniRate` + the parts' fire bonuses. It times the first-
/// person fire animation and the weapon's shot cycle alike.
pub fn fire_animation_rate(weapon: f32, parts: f32) -> f32 {
    animation_rate(1.0 + weapon + parts)
}

// ---------------------------------------------------------------------------
// The player's side

/// The parts a player wants, per weapon (Open question 6: the lobby's
/// choice): weapon ID (or the part after `combat_arms:`) to attachment
/// names, each offered for its own slot, later ones replacing earlier
/// ones. On a player; `CaDefaultLoadout` for those without one.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct CaLoadout {
    pub parts: Vec<(String, Vec<String>)>,
}

impl CaLoadout {
    /// One weapon's parts.
    pub fn with(mut self, weapon: &str, parts: &[&str]) -> Self {
        self.parts.push((weapon.to_string(), parts.iter().map(|s| s.to_string()).collect()));
        self
    }

    /// The parts named for weapon `id`.
    pub fn for_weapon(&self, id: &str) -> Vec<&str> {
        let short = id.rsplit(':').next().unwrap_or(id);
        self.parts
            .iter()
            .filter(|(w, _)| w.eq_ignore_ascii_case(id) || w.eq_ignore_ascii_case(short))
            .flat_map(|(_, p)| p.iter().map(String::as_str))
            .collect()
    }

    /// `weapon=Part,Part;weapon=Part` (the `CHIMERA_CA_MODS` form).
    pub fn parse(text: &str) -> Self {
        let mut out = Self::default();
        for entry in text.split(';').filter(|s| !s.trim().is_empty()) {
            let Some((weapon, parts)) = entry.split_once('=') else {
                warn!("CHIMERA_CA_MODS: `{entry}` isn't weapon=Part,Part");
                continue;
            };
            out.parts.push((
                weapon.trim().to_string(),
                parts.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect(),
            ));
        }
        out
    }
}

/// The parts a ladder rung fits to its gun for every player on it
/// (`game::rungs`), in place of the player's own (`CaLoadout`): weapon
/// `weapon`'s, each offered for its own slot; a slot none is offered for
/// keeps the weapon's default. On every character, empty (no weapon's)
/// off a Combat Arms rung.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
#[component(storage = "SparseSet")]
pub struct CaRungParts {
    pub weapon: String,
    pub parts: Vec<String>,
}

/// The parts of every player without a `CaLoadout` (the app's
/// `CHIMERA_CA_MODS`; none by default: stock weapons).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaDefaultLoadout(pub CaLoadout);

/// An AI character (no AI exists yet): the special functions' target
/// groups (`Npc`, `NH`, `NZ`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaNpc {
    pub zombie: bool,
}

/// Q1's fidelity flag: on (the game's), the kick fraction of the last
/// fitted part wins; off, the fractions are summed.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecoilLastWins(pub bool);

impl Default for RecoilLastWins {
    fn default() -> Self {
        Self(true)
    }
}

/// A fixed special-function probability draw (tests, debugging); None:
/// the round's random stream.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct CaSpecialRoll(pub Option<f32>);

/// Which parts go in which slot ("Fitting", steps 2 and 3): each of
/// `chosen` is offered for its own slot (a later one for the same slot
/// replaces an earlier one), a slot nobody offered for takes `defaults`'
/// part; each offered part that the weapon lists and that can go on a
/// weapon lands in its own `Kinds` slot. Returns (record ID, variant) per
/// slot and whether each came from the defaults.
pub fn fit(table: &CaModTable, params: &AttachParams, chosen: &[&str]) -> [Option<(u16, i32, bool)>; SLOTS] {
    let mut offered: [Vec<(&str, bool)>; SLOTS] = Default::default();
    for name in chosen {
        // An unknown name, or one that is no weapon part, is no offer for
        // any slot: it is rejected (its slot still takes the default).
        if let Some(slot) = table.find(name).and_then(|i| table.get(i)).and_then(ModRecord::slot) {
            offered[slot].push((name, false));
        }
    }
    for (slot, o) in offered.iter_mut().enumerate() {
        if o.is_empty()
            && let Some(d) = params.defaults.get(slot)
        {
            o.push((d, true));
        }
    }
    let mut out: [Option<(u16, i32, bool)>; SLOTS] = Default::default();
    for o in &offered {
        for (name, default) in o {
            let Some(id) = table.find(name) else { continue };
            let Some(r) = table.get(id) else { continue };
            let Some(variant) = params.variant_of(&r.name) else { continue };
            if !r.weapon_part() {
                continue;
            }
            out[r.slot().unwrap()] = Some((id, variant, *default));
        }
    }
    out
}

/// The parts as records (`CaMods`) for a weapon's `CaFitted`.
pub fn mods_of(table: &CaModTable, slots: &CaAttachSlots, fitted: &CaFitted) -> CaMods {
    mods_for(table, &slots.params, &fitted.slots)
}

/// The parts as records (`CaMods`) of a weapon with `params` and fitted
/// record ids `fitted`.
pub fn mods_for(table: &CaModTable, params: &AttachParams, fitted: &[Option<u16>; SLOTS]) -> CaMods {
    let mut parts: [Option<Part>; SLOTS] = Default::default();
    for (s, id) in fitted.iter().enumerate() {
        let Some(record) = id.and_then(|i| table.get(i)) else { continue };
        let variant = params
            .variant_of(&record.name)
            .filter(|v| (0..MODEL_VARIANTS as i32).contains(v))
            .map(|v| v as usize);
        let default = params.defaults.get(s).is_some_and(|d| d.eq_ignore_ascii_case(&record.name));
        parts[s] = Some(Part {
            id: id.unwrap(),
            variant,
            record: record.clone(),
            default,
        });
    }
    CaMods {
        parts,
        fire_sound_radius: params.fire_sound_radius,
        silenced_radius: params.silenced_radius.clone(),
        default_sight_visible: params.default_sight_visible,
    }
}

/// The server fits each weapon not yet fitted ("Fitting: when and where").
#[allow(clippy::type_complexity)]
fn fit_given(
    mut weapons: Query<(&Weapon, &CaAttachSlots, &mut CaFitted, Option<&mut Magazine>)>,
    loadouts: Query<&CaLoadout>,
    rung_parts: Query<&CaRungParts>,
    everyone: Res<CaDefaultLoadout>,
    table: Res<CaModTable>,
) {
    for (w, slots, mut fitted, mag) in &mut weapons {
        if fitted.given {
            continue;
        }
        let owner = w.owner.filter(|o| *o != Entity::PLACEHOLDER);
        let loadout = owner.and_then(|o| loadouts.get(o).ok()).unwrap_or(&everyone.0);
        // A ladder rung's parts for its gun, in place of the owner's choice.
        let rung = owner.and_then(|o| rung_parts.get(o).ok()).filter(|r| r.weapon == w.id);
        let chosen = match rung {
            Some(r) => r.parts.iter().map(String::as_str).collect(),
            None if owner.is_some() => loadout.for_weapon(w.id),
            None => Vec::new(),
        };
        let parts = fit(&table, &slots.params, &chosen);
        fitted.slots = parts.map(|p| p.map(|(id, ..)| id));
        fitted.given = true;
        // A fitted magazine refills the clip to its size from the total
        // (not a loose weapon's: it keeps its clip).
        if owner.is_some()
            && let Some(mut m) = mag
            && fitted.slots[MAGAZINE].is_some()
        {
            let mods = mods_of(&table, slots, &fitted);
            let size = mods.clip_max(slots.stock.clip);
            let reserve_max = reserve_ceiling(&slots.stock, size);
            let total = m.clip + m.reserve;
            m.size = size;
            m.reserve_max = reserve_max;
            m.clip = total.min(size);
            m.reserve = total - m.clip;
        }
    }
}

/// The reserve's ceiling for a clip of `size`: the total `MaxAmount`
/// stays (Q3).
fn reserve_ceiling(stock: &Stock, size: u32) -> u32 {
    (stock.reserve_max + stock.clip).saturating_sub(size)
}

/// Every weapon whose fitted set changed: its parts take the terms
/// (both sides; in the predicted tick, so a client's replay has them).
#[allow(clippy::type_complexity)]
fn apply_fitted(
    mut weapons: Query<
        (
            &CaAttachSlots,
            &CaFitted,
            &mut CaMods,
            Option<&mut Magazine>,
            Option<&mut Trigger>,
            (Option<&mut CaSpread>, Option<&mut CaRecoil>, Option<&mut CaDamage>, Option<&mut CaZoom>),
            (Option<&CaItem>, Option<&CaGearRates>, Option<&mut ShellReload>),
        ),
        Or<(Changed<CaFitted>, Changed<CaGearRates>)>,
    >,
    table: Res<CaModTable>,
    last_wins: Res<RecoilLastWins>,
) {
    for (slots, fitted, mut have, mag, trigger, (spread, recoil, damage, zoom), (item, gear, shells)) in &mut weapons {
        let mods = mods_of(&table, slots, fitted);
        let stock = slots.stock;
        let (fire, reload, _) = mods.rates();
        // The gear's terms (the vest's and the sets'): one sum with the
        // parts' (attachments.md "Animation rates").
        let gear = gear.copied().unwrap_or_default();
        let reload_rate = animation_rate(1.0 + reload + gear.reload);
        if let (Some((start, insert, finish)), Some(mut s)) = (stock.shell, shells) {
            let want = (start / reload_rate, insert / reload_rate, finish / reload_rate);
            if (s.start, s.insert, s.finish) != want {
                (s.start, s.insert, s.finish) = want;
            }
        }
        if let Some(mut m) = mag {
            let size = mods.clip_max(stock.clip);
            let reserve_max = reserve_ceiling(&stock, size);
            let reload_time = stock.reload / reload_rate;
            if m.size != size || m.reserve_max != reserve_max || m.reload_time != reload_time {
                m.size = size;
                m.reserve_max = reserve_max;
                m.reload_time = reload_time;
            }
        }
        if let Some(mut t) = trigger {
            // The weapon's own `FireAniRate` is part of the rate (Corrections 6).
            let rate = fire_animation_rate(item.map_or(0.0, |i| i.fire_rate), fire + gear.fire);
            let (cycle, postfire) = (stock.cycle / rate, stock.postfire / rate);
            if t.cycle != cycle || t.postfire != postfire {
                t.cycle = cycle;
                t.postfire = postfire;
            }
        }
        if let Some(mut s) = spread {
            let m = mods.spread();
            if s.mods != m {
                s.mods = m;
            }
        }
        if let Some(mut r) = recoil {
            let m = mods.recoil(last_wins.0);
            if r.mods != m {
                r.mods = m;
            }
        }
        if let Some(mut d) = damage {
            let m = mods.damage();
            if d.mods != m {
                d.mods = m;
            }
        }
        if let Some(mut z) = zoom {
            let scope = mods.scope();
            let sway = mods.sway();
            if z.scope != scope || z.sway_add != sway {
                z.scope = scope;
                z.sway_add = sway;
            }
        }
        if *have != mods {
            *have = mods;
        }
    }
}

/// The draw time of each weapon whose fitted set or gear rates changed:
/// the select animation over its summed rate (the item's own bonuses with
/// the parts' and the gear's, inventory.md 6.4); a weapon without parts
/// (knives, grenades, specials) gets the item's and the gear's terms too
/// (inventory.md Corrections (2026-10-30) 2), on its `DrawStock`.
#[allow(clippy::type_complexity)]
fn apply_draw(
    mut weapons: Query<
        (&mut Weapon, &CaAttachSlots, &CaMods, Option<&CaItem>, Option<&CaGearRates>),
        Or<(Changed<CaMods>, Changed<CaGearRates>)>,
    >,
    mut bare: Query<
        (&mut Weapon, &super::inventory::DrawStock, Option<&CaItem>, &CaGearRates),
        (Without<CaAttachSlots>, Or<(Changed<CaGearRates>, Changed<super::inventory::DrawStock>)>),
    >,
) {
    for (mut w, slots, mods, item, gear) in &mut weapons {
        let (_, _, select) = mods.rates();
        let gear = gear.map_or(0.0, |g| g.select);
        let draw = slots.stock.draw / animation_rate(1.0 + item.map_or(0.0, |i| i.select_rate) + select + gear);
        if w.draw_time != draw {
            w.draw_time = draw;
        }
    }
    for (mut w, stock, item, gear) in &mut bare {
        let draw = stock.0 / animation_rate(1.0 + item.map_or(0.0, |i| i.select_rate) + gear.select);
        if w.draw_time != draw {
            w.draw_time = draw;
        }
    }
}

/// Combat Arms' attachments: the hook, the record table, fitting and the
/// parts' terms. Registered before the weapons (their data uses the
/// hook).
pub struct CaAttachmentsPlugin;

impl Plugin for CaAttachmentsPlugin {
    fn build(&self, app: &mut App) {
        use chimera_core::character::CharacterPartsExt;
        app.on_every_character::<CaRungParts>();
        let loadout = std::env::var("CHIMERA_CA_MODS").map(|s| CaLoadout::parse(&s)).unwrap_or_default();
        let table = super::data::CaData::of(app).attachments().clone();
        app.register_weapon_hook::<AttachParams>(ATTACHMENTS, |e, d, p| {
            let mag = e.get::<Magazine>();
            let stock = Stock {
                clip: mag.map_or(0, |m| m.size),
                reserve_max: mag.map_or(0, |m| m.reserve_max),
                reload: mag.map_or(0.0, |m| m.reload_time),
                cycle: e.get::<Trigger>().map_or(0.0, |t| t.cycle),
                draw: d.draw,
                shell: e.get::<ShellReload>().map(|s| (s.start, s.insert, s.finish)),
                postfire: e.get::<Trigger>().map_or(0.0, |t| t.postfire),
            };
            e.insert((
                CaAttachSlots {
                    params: p.clone(),
                    stock,
                },
                CaFitted::default(),
                CaMods::default(),
                CaGearRates::default(),
            ));
        })
        .server_docs(
            super::data::served::ATTACHMENTS,
            super::data::served::attachments_doc(&table),
            chimera_sim::server_docs::DocApply::With(super::data::served::apply_attachments),
        )
        .insert_resource(table)
        .insert_resource(CaDefaultLoadout(loadout))
        .init_resource::<RecoilLastWins>()
        .init_resource::<CaSpecialRoll>()
        .add_systems(
            Predict::Select,
            (
                fit_given.run_if(crate::core::authoritative),
                super::gear::weapon_rates,
                apply_fitted,
                apply_draw,
            )
                .chain()
                .before(SelectWeapons),
        )
        .predicted_part::<CaFitted>()
        .predicted_net::<StunPenalty>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_records_parse() {
        let t = CaModTable::built_in();
        let a = &t.records[t.find("silencer_a_ar").unwrap() as usize];
        assert_eq!((a.slot(), a.kind, a.inst_damage), (Some(MUZZLE), Some(SILENCER), -0.10));
        assert!(t.find("Silencer_A_Pistol").is_some());
        assert!(t.find("Silencer_A_SG").is_none());
        let names: std::collections::HashSet<_> = t.records.iter().map(|r| r.name.to_lowercase()).collect();
        assert_eq!(names.len(), t.records.len(), "names are unique");
        for r in &t.records {
            assert!(!r.name.contains('{'), "{}", r.name);
        }
    }

    #[test]
    fn gauge_images() {
        assert_eq!([gauge_image(73), gauge_image(80), gauge_image(95), gauge_image(4), gauge_image(5)], [
            70, 80, 100, 0, 10
        ]);
    }

    #[test]
    fn target_groups() {
        let player = Victim::default();
        let zombie_ai = Victim { npc: true, zombie: true };
        assert!(Target::Player.matches(player) && !Target::NZ.matches(player));
        assert!(Target::NZ.matches(zombie_ai) && !Target::Player.matches(zombie_ai));
        assert!(!Target::All.reaches_player_client() && Target::Player.reaches_player_client());
    }
}
