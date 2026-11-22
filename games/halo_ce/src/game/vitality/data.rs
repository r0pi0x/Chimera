//! Halo's vitality and damage numbers as data, read at startup from the
//! game's own tables: `data/vitality.toml` (the multiplayer Spartan's
//! collision model, its materials and regions, the globals', unit's and
//! powerups' values and the engine's constants of
//! specs/halo_ce/health_shields.md) as `Spartan`, and `data/damage.toml`
//! (every multiplayer damage effect of projectiles.md's "Damage effects"
//! table, by our id) as `DamageEffects`. Both in the game's units (points,
//! fractions, seconds, WU); durations become ticks as the spec says
//! (`ticks`: f32 seconds x 30, truncated). The heavy test
//! `tests/it/heavy/vitality_data.rs` checks every number against the disc.
//!
//! Known shortcuts:
//! - Only the numbers the damage and vitality rules read are kept (no
//!   presentation fields: screen flash, rumble, camera, sound).
//! - A damage effect's vehicle passthrough penalty is in the data only
//!   where the spec gives one (the sniper bullet's 0.5); 0 elsewhere.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::Deserialize;

/// `data/vitality.toml`, compiled in.
pub const VITALITY: chimera_core::game_data::GameData = chimera_core::game_data!("data/vitality.toml");
/// `data/damage.toml`, compiled in.
pub const DAMAGE: chimera_core::game_data::GameData = chimera_core::game_data!("data/damage.toml");

/// Seconds as the engine's ticks: `trunc(seconds x 30)` in f32
/// (health_shields.md "Units and conventions": 0.15 s is 4 ticks).
pub fn ticks(seconds: f32) -> u32 {
    (seconds * 30.0).max(0.0) as u32
}

/// One collision model material (health_shields.md "Materials").
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Material {
    pub name: String,
    pub head: bool,
    /// The material type (21 cyborg).
    #[serde(rename = "type")]
    pub kind: usize,
    /// The shield leak, a fraction.
    pub leak: f32,
    /// Shield and body damage multipliers.
    pub shield: f32,
    pub body: f32,
}

/// One damage region.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Region {
    pub name: String,
    pub flags: u32,
    pub threshold: f32,
}

/// The collision model's vitality fields.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Model {
    pub tag: String,
    pub maximum_body: f32,
    pub maximum_shield: f32,
    pub flags: u32,
    pub indirect_material: usize,
    pub friendly_damage_resistance: f32,
    pub area_damage_effect_threshold: f32,
    pub body_damaged_threshold: f32,
    pub body_destroyed_threshold: f32,
    pub shield_material_type: usize,
    pub shield_failure_threshold: f32,
    pub failing_shield_leak: f32,
    pub minimum_stun_damage: f32,
    pub stun_time: f32,
    pub recharge_time: f32,
    pub recharge_velocity: f32,
    pub shield_damaged_threshold: f32,
    pub materials: Vec<Material>,
    pub regions: Vec<Region>,
}

impl Model {
    /// "always shields friendly damage" (flag bit 2).
    pub fn always_shields_friendly(&self) -> bool {
        self.flags & 4 != 0
    }
}

/// Chimera's body regions as the model's materials.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct RegionMaterials {
    pub head: usize,
    pub legs: usize,
    pub other: usize,
    /// The arms' (upper arm, forearm, hand), where the model has its own
    /// (the Elite's `arms`, body x0.5); None: `other`.
    #[serde(default)]
    pub arms: Option<usize>,
    /// A shield node's (the Jackal's disc: `body::hits::SHIELD_REGION`);
    /// None: `other`.
    #[serde(default)]
    pub shield: Option<usize>,
}

/// The globals' player information values.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Globals {
    pub minimum_stun_time: f32,
    pub maximum_stun_time: f32,
    pub stun_movement_penalty: f32,
    pub stun_turning_penalty: f32,
    pub stun_jumping_penalty: f32,
    pub run_forward_speed: f32,
}

/// The unit's ping values.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Unit {
    pub soft_ping_threshold: f32,
    pub soft_ping_interrupt_time: f32,
    pub hard_ping_threshold: f32,
}

/// An equipment's powerup.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Powerup {
    pub tag: String,
    #[serde(rename = "type")]
    pub kind: u16,
    pub time: f32,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Powerups {
    pub overshield: Powerup,
    pub active_camouflage: Powerup,
    pub health_pack: Powerup,
}

/// The engine's constants (health_shields.md "Constants").
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Constants {
    pub overcharge_step: f32,
    pub overshield_max: f32,
    pub overshield_drain: f32,
    pub overshield_floor: f32,
    pub negligible: f32,
    pub damage_fade: f32,
    pub recent_hold: u32,
    pub camo_step: f32,
    pub camo_fire_default: f32,
    pub camo_fire_floor: f32,
    pub melee_lunge_scale: f32,
    pub melee_airborne_ticks: u32,
    pub impulse_lift: f32,
    pub ping_rear_cone: f32,
    pub ping_front_cone: f32,
    pub attackers: usize,
    pub credit_window: u64,
    pub assist_window: u64,
    pub health_option_min: f32,
    pub health_option_max: f32,
}

/// `data/vitality.toml`: a multiplayer Spartan's vitality (a resource).
#[derive(Resource, Clone, Debug, PartialEq, Deserialize)]
pub struct Spartan {
    pub model: Model,
    pub regions: RegionMaterials,
    pub globals: Globals,
    pub unit: Unit,
    pub powerups: Powerups,
    pub constants: Constants,
}

impl Spartan {
    /// The table compiled in (a test checks it parses).
    pub fn load() -> Self {
        toml::from_str(&VITALITY).expect("data/vitality.toml parses (a test checks it)")
    }

    /// Whether the shield protects the body at all: a shield, and the
    /// indirect material (what area and other games' hits land on) not
    /// leaking every hit through it (the Jackal's arm shield does).
    pub fn shield_covers_body(&self) -> bool {
        let leak = self.model.materials.get(self.model.indirect_material).map_or(0.0, |m| m.leak);
        self.model.maximum_shield > 0.0 && leak < 1.0
    }

    /// The shield delay, ticks: trunc(stun time x 30) = 180.
    pub fn shield_delay(&self) -> u32 {
        ticks(self.model.stun_time)
    }

    /// The material a hit on `region` lands on (None: no region, an area
    /// hit: the indirect material).
    pub fn material_of(&self, region: Option<crate::core::BodyRegion>) -> usize {
        use crate::core::BodyRegion::*;
        match region {
            None => self.model.indirect_material,
            Some(Head) => self.regions.head,
            Some(LeftShin | LeftFoot | RightShin | RightFoot) => self.regions.legs,
            Some(LeftUpperArm | LeftForearm | LeftHand | RightUpperArm | RightForearm | RightHand) => {
                self.regions.arms.unwrap_or(self.regions.other)
            }
            Some(Groin) => self.regions.shield.unwrap_or(self.regions.other),
            Some(_) => self.regions.other,
        }
    }
}

/// A damage effect's category (projectiles.md "Damage part", 0x1C6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    None,
    Falling,
    Bullet,
    Grenade,
    #[serde(rename = "high explosive")]
    HighExplosive,
    Sniper,
    Melee,
    Flame,
    #[serde(rename = "mounted weapon")]
    MountedWeapon,
    Vehicle,
    Plasma,
    Needle,
    Shotgun,
}

impl Category {
    /// The tag's code.
    pub fn code(self) -> u16 {
        self as u16
    }

    /// The engine's damage kind for it (what kill notices, ragdolls and
    /// other games' reactions read).
    pub fn kind(self) -> crate::core::DamageKind {
        use crate::core::DamageKind;
        match self {
            Category::Falling => DamageKind::Fall,
            Category::Grenade | Category::HighExplosive => DamageKind::Blast,
            Category::Melee => DamageKind::Melee,
            Category::Flame => DamageKind::Burn,
            Category::Vehicle => DamageKind::Crush,
            Category::None => DamageKind::Generic,
            Category::Bullet
            | Category::Sniper
            | Category::MountedWeapon
            | Category::Plasma
            | Category::Needle
            | Category::Shotgun => DamageKind::Bullet,
        }
    }
}

/// A damage effect's side effect (0x1C4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum SideEffect {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "harmless")]
    Harmless,
    #[serde(rename = "lethal to the unsuspecting")]
    LethalToTheUnsuspecting,
    #[serde(rename = "emp")]
    Emp,
}

impl SideEffect {
    pub fn code(self) -> u16 {
        self as u16
    }
}

/// Damage flags (0x1C8), the bits the rules read.
pub mod flags {
    pub const DOES_NOT_HURT_OWNER: u32 = 1 << 0;
    pub const CAN_CAUSE_HEADSHOTS: u32 = 1 << 1;
    pub const PINGS_RESISTANT_UNITS: u32 = 1 << 2;
    pub const DOES_NOT_HURT_FRIENDS: u32 = 1 << 3;
    pub const DOES_NOT_PING_UNITS: u32 = 1 << 4;
    pub const DETONATES_EXPLOSIVES: u32 = 1 << 5;
    pub const ONLY_HURTS_SHIELDS: u32 = 1 << 6;
    pub const CAUSES_FLAMING_DEATH: u32 = 1 << 7;
    /// The HUD's damage arrows point down whatever the side.
    pub const DAMAGE_INDICATORS_ALWAYS_POINT_DOWN: u32 = 1 << 8;
    pub const SKIPS_SHIELDS: u32 = 1 << 9;
    pub const CAN_CAUSE_MULTIPLAYER_HEADSHOTS: u32 = 1 << 11;
}

/// Material types with a modifier slot (0 to 32).
pub const MATERIAL_TYPES: usize = 33;

/// One damage effect's numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct DamageEffect {
    pub tag: String,
    pub category: Category,
    pub side_effect: SideEffect,
    pub flags: u32,
    /// The tag's own flag bit 0: area damage not scaled by distance.
    pub no_distance_scaling: bool,
    /// The damage lower bound ("minimum") and the upper bound pair, points.
    pub lower: f32,
    pub upper: [f32; 2],
    /// Falloff and cutoff radius, WU.
    pub radius: [f32; 2],
    pub core: f32,
    pub passthrough: f32,
    /// Active camouflage damage.
    pub camo: f32,
    /// Stun, maximum stun, stun time (s).
    pub stun: [f32; 3],
    /// Instantaneous acceleration, WU/s.
    pub accel: f32,
    /// Material modifiers by type (0 to 32).
    pub modifiers: [f32; MATERIAL_TYPES],
}

impl DamageEffect {
    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    /// The modifier for material type `t` (0 past the table).
    pub fn modifier(&self, t: usize) -> f32 {
        self.modifiers.get(t).copied().unwrap_or(0.0)
    }

    /// The event's amount (projectiles.md behaviour 18, the weapons' side
    /// of the seam): `(1 - s) lower + s U`, U = `upper.0 + u (upper.1 -
    /// upper.0)` for a uniform draw `u` in 0 to 1.
    pub fn amount(&self, s: f32, u: f32) -> f32 {
        self.amount_with(s, self.upper[0] + u * (self.upper[1] - self.upper[0]))
    }

    /// The same with the upper bound's draw `U` itself (the spec's tests'
    /// "rnd").
    pub fn amount_with(&self, s: f32, draw: f32) -> f32 {
        (1.0 - s) * self.lower + draw * s
    }
}

/// A damage effect's id of ours (`data/damage.toml`'s key): what a Halo
/// weapon, projectile, melee or fall names in its hit (`damage::HaloHit`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectId(pub &'static str);

/// The ids of every effect in `data/damage.toml` (a test checks they are
/// the table's keys).
pub mod ids {
    use super::EffectId;
    pub const ASSAULT_RIFLE_BULLET: EffectId = EffectId("assault_rifle_bullet");
    pub const PISTOL_BULLET: EffectId = EffectId("pistol_bullet");
    pub const SHOTGUN_PELLET: EffectId = EffectId("shotgun_pellet");
    pub const SNIPER_BULLET: EffectId = EffectId("sniper_bullet");
    pub const PLASMA_RIFLE_BOLT: EffectId = EffectId("plasma_rifle_bolt");
    pub const PLASMA_PISTOL_BOLT: EffectId = EffectId("plasma_pistol_bolt");
    pub const CHARGED_BOLT: EffectId = EffectId("charged_bolt");
    pub const NEEDLE_IMPACT: EffectId = EffectId("needle_impact");
    pub const NEEDLE_DETONATION: EffectId = EffectId("needle_detonation");
    pub const NEEDLER_EXPLOSION: EffectId = EffectId("needler_explosion");
    pub const NEEDLER_SHOCK_WAVE: EffectId = EffectId("needler_shock_wave");
    pub const FRAG_EXPLOSION: EffectId = EffectId("frag_explosion");
    pub const FRAG_SHOCK_WAVE: EffectId = EffectId("frag_shock_wave");
    pub const PLASMA_GRENADE_EXPLOSION: EffectId = EffectId("plasma_grenade_explosion");
    pub const PLASMA_GRENADE_ATTACHED: EffectId = EffectId("plasma_grenade_attached");
    pub const PLASMA_GRENADE_SHOCK_WAVE: EffectId = EffectId("plasma_grenade_shock_wave");
    pub const ROCKET_EXPLOSION: EffectId = EffectId("rocket_explosion");
    pub const TANK_SHELL_EXPLOSION: EffectId = EffectId("tank_shell_explosion");
    pub const FLAME_EXPLOSION: EffectId = EffectId("flame_explosion");
    pub const WARTHOG_BULLET: EffectId = EffectId("warthog_bullet");
    pub const SCORPION_BULLET: EffectId = EffectId("scorpion_bullet");
    pub const GHOST_BOLT: EffectId = EffectId("ghost_bolt");
    pub const ASSAULT_RIFLE_MELEE: EffectId = EffectId("assault_rifle_melee");
    pub const PISTOL_MELEE: EffectId = EffectId("pistol_melee");
    pub const SHOTGUN_MELEE: EffectId = EffectId("shotgun_melee");
    pub const PLASMA_RIFLE_MELEE: EffectId = EffectId("plasma_rifle_melee");
    pub const SNIPER_RIFLE_MELEE: EffectId = EffectId("sniper_rifle_melee");
    pub const PLASMA_PISTOL_MELEE: EffectId = EffectId("plasma_pistol_melee");
    pub const NEEDLER_MELEE: EffectId = EffectId("needler_melee");
    pub const ROCKET_LAUNCHER_MELEE: EffectId = EffectId("rocket_launcher_melee");
    pub const FLAMETHROWER_MELEE: EffectId = EffectId("flamethrower_melee");
    pub const BALL_MELEE: EffectId = EffectId("ball_melee");
    pub const FLAG_MELEE: EffectId = EffectId("flag_melee");
    pub const CYBORG_MELEE: EffectId = EffectId("cyborg_melee");
    pub const FALLING: EffectId = EffectId("falling");
    pub const DISTANCE: EffectId = EffectId("distance");
    pub const VEHICLE_COLLISION: EffectId = EffectId("vehicle_collision");
    pub const FLAMING_DEATH: EffectId = EffectId("flaming_death");
    pub const VEHICLE_HIT_ENVIRONMENT: EffectId = EffectId("vehicle_hit_environment");
    pub const VEHICLE_KILLED_UNIT: EffectId = EffectId("vehicle_killed_unit");

    /// The campaign's (d20's, not on `bloodgulch`): the Hunter's cannon's
    /// explosion and the Hunter's melee (`campaign`, the Hunter's rung).
    pub const FUEL_ROD_EXPLOSION: EffectId = EffectId("fuel_rod_explosion");
    pub const HUNTER_MELEE: EffectId = EffectId("hunter_melee");
    /// The campaign's ids (`FUEL_ROD_EXPLOSION`, `HUNTER_MELEE`).
    pub const CAMPAIGN: [EffectId; 2] = [FUEL_ROD_EXPLOSION, HUNTER_MELEE];

    /// Every id of the multiplayer maps' (on `bloodgulch`), in the table's
    /// order.
    pub const ALL: [EffectId; 40] = [
        ASSAULT_RIFLE_BULLET,
        PISTOL_BULLET,
        SHOTGUN_PELLET,
        SNIPER_BULLET,
        PLASMA_RIFLE_BOLT,
        PLASMA_PISTOL_BOLT,
        CHARGED_BOLT,
        NEEDLE_IMPACT,
        NEEDLE_DETONATION,
        NEEDLER_EXPLOSION,
        NEEDLER_SHOCK_WAVE,
        FRAG_EXPLOSION,
        FRAG_SHOCK_WAVE,
        PLASMA_GRENADE_EXPLOSION,
        PLASMA_GRENADE_ATTACHED,
        PLASMA_GRENADE_SHOCK_WAVE,
        ROCKET_EXPLOSION,
        TANK_SHELL_EXPLOSION,
        FLAME_EXPLOSION,
        WARTHOG_BULLET,
        SCORPION_BULLET,
        GHOST_BOLT,
        ASSAULT_RIFLE_MELEE,
        PISTOL_MELEE,
        SHOTGUN_MELEE,
        PLASMA_RIFLE_MELEE,
        SNIPER_RIFLE_MELEE,
        PLASMA_PISTOL_MELEE,
        NEEDLER_MELEE,
        ROCKET_LAUNCHER_MELEE,
        FLAMETHROWER_MELEE,
        BALL_MELEE,
        FLAG_MELEE,
        CYBORG_MELEE,
        FALLING,
        DISTANCE,
        VEHICLE_COLLISION,
        FLAMING_DEATH,
        VEHICLE_HIT_ENVIRONMENT,
        VEHICLE_KILLED_UNIT,
    ];
}

/// Every damage effect by id (a resource).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct DamageEffects(pub BTreeMap<String, DamageEffect>);

impl DamageEffects {
    /// The table compiled in (a test checks it parses).
    pub fn load() -> Self {
        Self::parse(&DAMAGE).expect("data/damage.toml parses (a test checks it)")
    }

    /// An effect's index in the table's (sorted) order: what the network
    /// carries (`fx::victim::HaloVictimNet`).
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.0.keys().position(|k| k == id)
    }

    /// The effect at `index_of`'s index.
    pub fn at(&self, index: usize) -> Option<(&String, &DamageEffect)> {
        self.0.iter().nth(index)
    }

    /// Parse a table in `data/damage.toml`'s form.
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: File = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut out = BTreeMap::new();
        for (id, row) in &file.effect {
            let base = match &row.like {
                Some(other) => file.effect.get(other).ok_or_else(|| format!("{id}: like {other}: no such effect"))?,
                None => row,
            };
            let field = |name: &str| format!("{id}: no {name}");
            let modifiers = modifiers(base.modifiers.as_deref().unwrap_or(&[])).map_err(|e| format!("{id}: {e}"))?;
            out.insert(
                id.clone(),
                DamageEffect {
                    tag: row.tag.clone(),
                    category: base.category.ok_or_else(|| field("category"))?,
                    side_effect: base.side_effect.ok_or_else(|| field("side_effect"))?,
                    flags: base.flags.ok_or_else(|| field("flags"))?,
                    no_distance_scaling: base.no_distance_scaling.unwrap_or(false),
                    lower: base.lower.ok_or_else(|| field("lower"))?,
                    upper: base.upper.ok_or_else(|| field("upper"))?,
                    radius: base.radius.ok_or_else(|| field("radius"))?,
                    core: base.core.ok_or_else(|| field("core"))?,
                    passthrough: base.passthrough.unwrap_or(0.0),
                    camo: base.camo.ok_or_else(|| field("camo"))?,
                    stun: base.stun.ok_or_else(|| field("stun"))?,
                    accel: base.accel.ok_or_else(|| field("accel"))?,
                    modifiers,
                },
            );
        }
        Ok(Self(out))
    }

    /// The effect with id `id`.
    pub fn get(&self, id: EffectId) -> Option<&DamageEffect> {
        self.0.get(id.0)
    }
}

#[derive(Deserialize)]
struct File {
    effect: BTreeMap<String, Row>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    tag: String,
    like: Option<String>,
    category: Option<Category>,
    side_effect: Option<SideEffect>,
    flags: Option<u32>,
    no_distance_scaling: Option<bool>,
    lower: Option<f32>,
    upper: Option<[f32; 2]>,
    radius: Option<[f32; 2]>,
    core: Option<f32>,
    passthrough: Option<f32>,
    camo: Option<f32>,
    stun: Option<[f32; 3]>,
    accel: Option<f32>,
    modifiers: Option<Vec<Group>>,
}

#[derive(Deserialize)]
struct Group {
    value: f32,
    types: String,
}

/// The modifier table of the spec's "value: types" groups.
fn modifiers(groups: &[Group]) -> Result<[f32; MATERIAL_TYPES], String> {
    let mut out = [0.0; MATERIAL_TYPES];
    let mut seen = [false; MATERIAL_TYPES];
    for g in groups {
        for part in g.types.split(',').map(str::trim) {
            let (a, b) = match part.split_once('-') {
                Some((a, b)) => (a.trim(), b.trim()),
                None => (part, part),
            };
            let (a, b): (usize, usize) = (
                a.parse().map_err(|_| format!("bad type {part}"))?,
                b.parse().map_err(|_| format!("bad type {part}"))?,
            );
            if a > b || b >= MATERIAL_TYPES {
                return Err(format!("bad types {part}"));
            }
            for t in a..=b {
                if seen[t] {
                    return Err(format!("type {t} twice"));
                }
                seen[t] = true;
                out[t] = g.value;
            }
        }
    }
    Ok(out)
}
