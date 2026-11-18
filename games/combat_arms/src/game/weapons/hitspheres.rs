use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock},
};

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::{character::CharacterPartsExt, later::Later};
use chimera_sim::server_docs::DocsAppExt;
use serde::{Deserialize, Serialize};

use crate::{
    core::{
        Damage, DamageKind, DamageRules, Damageable, FirstTimePredicted, Health, Hitgroup, Intent, MovementState, NetRole,
        Predict, PredictedAppExt, PropSurface, SOLID_LAYERS, SimClock, Velocity,
    },
    map::{SurfaceNames, sound::SurfaceGrid},
    weapon::{
        AltModes, Burst, CarriedAppExt, Hitscan, ParamsAppExt, Inventory, NetPart, SpreadShape, ViewBackPush, ViewPunch,
        WeaponEvent,
        Armor, ArmorPieces, WeaponEventKind, WeaponFrame, Zoom, aim_of,
        hits,
        data::{WeaponData, WeaponDataAppExt},
        lagcomp,
        random::{Rng, shared_random},
    },
};

use crate::game::characters;
use crate::game::attachments::{CaNpc, CaSpecialRoll, DamageMods, RecoilMods, SpecialKind, SpreadMods, Victim};

pub use self::{
    classes::{ALT_FIRE, CaAltFire, CaAltVolley, CaSpin, CarryOver, PelletCarryOver, SPIN, SpinParams, SpinState, reported_hits},
    zoom::{CaFocusBlur, CaZoom, CaZoomSound, Scope, Sway, ZOOM, ZoomParams},
};

use super::*;

// ---------------------------------------------------------------------------
// Hit zones and armour

/// A hit sphere's location code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    Head,
    Torso,
    Arm,
    Leg,
    Groin,
}

/// One hit sphere of a skeleton (specs/combat_arms/characters.md 11):
/// its hit-table entry's node or socket name, radius, location and damage
/// factor, and `at`: its centre in the spec's measured standing pose (`LST`
/// at t = 0) in the character's frame (feet at the origin, +Y up, facing
/// -Z, +X its right), world units. Entries no stock body has, or with
/// radius 0, have radius 0 here (never hit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitSphere {
    pub name: &'static str,
    pub at: Vec3,
    pub radius: f32,
    pub location: Location,
    pub factor: f32,
    /// The skeleton (`characters::records` index: 0 male, 1 female).
    pub skeleton: usize,
    /// `Node#Priority`: the server's own walk picks the highest
    /// (`priority_sphere`; data/characters.toml: weapon_firing.md
    /// Follow-up (2026-11-14) 2 and 3, the install's for other tables).
    pub priority: f32,
}

use Location::{Arm, Groin, Head, Leg, Torso};

impl HitSphere {
    /// Its body region (`core::BodyRegion`, docs/DESIGN.md "Hits across games"):
    /// by location, the side by where it sits (`at.x`, as hits report it),
    /// the limb's part by its node's name (`*hand*`; `*arml*` the forearm;
    /// `*foot*`, `*toe*`; `*legl*` the shin; the rest of a limb, the
    /// pelvis spheres too, upper arm or thigh); the torso spheres chest
    /// but the neck (its hitgroup is the chest either way).
    pub fn region(&self) -> crate::core::BodyRegion {
        use crate::core::BodyRegion as R;
        // Lower-cased on the stack (every Combat Arms character's hitboxes
        // ask each tick); a long name on the heap.
        let mut buf = [0u8; 64];
        let heap;
        let n: &str = if self.name.len() <= buf.len() {
            let out = &mut buf[..self.name.len()];
            out.copy_from_slice(self.name.as_bytes());
            out.make_ascii_lowercase();
            std::str::from_utf8(out).expect("ASCII lower-casing keeps UTF-8")
        } else {
            heap = self.name.to_ascii_lowercase();
            &heap
        };
        // The side by where it sits; an install table's (no measured
        // place) by its name (`Left_*`, `Bip01 L *`, `*_L`).
        let left = if self.at == Vec3::ZERO {
            n.contains("left") || n.contains(" l ") || n.ends_with("_l") || n.ends_with(" l")
        } else {
            self.at.x < 0.0
        };
        let side = |l, r| if left { l } else { r };
        match self.location {
            Head => R::Head,
            Groin => R::Groin,
            Torso if n.contains("neck") => R::Neck,
            Torso => R::Chest,
            Arm if n.contains("hand") => side(R::LeftHand, R::RightHand),
            Arm if n.contains("arml") => side(R::LeftForearm, R::RightForearm),
            Arm => side(R::LeftUpperArm, R::RightUpperArm),
            Leg if n.contains("foot") || n.contains("toe") => side(R::LeftFoot, R::RightFoot),
            Leg if n.contains("legl") => side(R::LeftShin, R::RightShin),
            Leg => side(R::LeftThigh, R::RightThigh),
        }
    }
}

/// Hit table `index`: ours for the male and female (measured), the
/// install's past them (characters.md 13.6: `CaData::install`'s
/// `CharacterData::hit_spheres`, the only data with bodies on them);
/// else the male's.
pub fn hit_table(index: usize) -> &'static [HitSphere] {
    match index {
        0 => *MALE,
        1 => *FEMALE,
        i => characters::records::install_tables().hit_spheres.get(i).map_or(*MALE, Vec::as_slice),
    }
}

/// Skeleton `skel`'s (index `index`) hit table.
pub fn make_hit_table(skel: &characters::records::SkeletonRecord, index: usize) -> Vec<HitSphere> {
    // An install table has no measured positions (its spheres are only
    // where a body model poses them); ours drop what no stock body has.
    let measured = skel.spheres.iter().any(|s| s.at.is_some());
    let table: Vec<HitSphere> = skel
        .spheres
        .iter()
        .map(|s| HitSphere {
            name: s.name.clone().leak(),
            at: s.at.map_or(Vec3::ZERO, |a| characters::skeleton::to_body(Vec3::from(a), characters::skeleton::STAND_FEET)),
            radius: if s.absent || (measured && s.at.is_none()) { 0.0 } else { s.radius },
            location: match s.location {
                1 => Head,
                3 => Arm,
                4 => Leg,
                5 => Groin,
                _ => Torso,
            },
            factor: s.factor,
            skeleton: index,
            priority: s.priority.unwrap_or_else(|| skel.hit_priority.of(s.location)),
        })
        .collect();
    table
}

/// The male skeleton's hit table (table 0, the default male body).
pub static MALE: LazyLock<&'static [HitSphere]> = LazyLock::new(|| characters::records::builtin().hit_spheres[0].as_slice());
/// The female skeleton's (table 1).
pub static FEMALE: LazyLock<&'static [HitSphere]> = LazyLock::new(|| characters::records::builtin().hit_spheres[1].as_slice());

/// A character's hit table (`MALE` when it has none; `characters` sets
/// it from the body's skeleton).
#[derive(Component, Clone, Copy, Debug)]
pub struct HitSpheres(pub &'static [HitSphere]);

/// A named sphere of the male skeleton.
pub fn male(name: &str) -> &'static HitSphere {
    MALE.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no sphere {name}"))
}

/// Which sphere a segment from `start` to `end` hits: of those it passes
/// (radius > 0), the one with the smallest |p|^2 - r^2 (p the centre from
/// `start`). Spheres as (centre, radius).
pub fn pick_sphere(spheres: &[(Vec3, f32)], start: Vec3, end: Vec3) -> Option<usize> {
    let seg = end - start;
    let len2 = seg.length_squared().max(1e-12);
    spheres
        .iter()
        .enumerate()
        .filter(|(_, (c, r))| {
            if *r <= 0.0 {
                return false;
            }
            let t = ((*c - start).dot(seg) / len2).clamp(0.0, 1.0);
            (start + seg * t).distance_squared(*c) <= r * r
        })
        .map(|(i, (c, r))| (i, (*c - start).length_squared() - r * r))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Which sphere a character found by the server's own walk is hit in
/// (weapon_firing.md Corrections 5): of the spheres (centre, radius) whose
/// centre is within its radius of the infinite line through `at` along
/// the unit `dir`, wherever along the line, the one with the highest
/// `priority`; on a tie the later one. None: the line passes them all.
pub fn priority_sphere(spheres: &[(Vec3, f32)], data: &[HitSphere], at: Vec3, dir: Vec3) -> Option<usize> {
    spheres
        .iter()
        .enumerate()
        .filter(|(_, (c, r))| {
            let p = *c - at;
            *r > 0.0 && (p - dir * p.dot(dir)).length_squared() <= r * r
        })
        .map(|(i, _)| (i, data.get(i).map_or(0.0, |s| s.priority)))
        .fold(None, |best: Option<(usize, f32)>, (i, pr)| match best {
            Some((_, b)) if b > pr => best,
            _ => Some((i, pr)),
        })
        .map(|(i, _)| i)
}

/// The direction sector of a bullet travelling along `dir` at a victim
/// facing `forward` with `right` (0..11, pi/6 each).
pub fn sector(forward: Vec3, right: Vec3, dir: Vec3) -> u32 {
    let theta = std::f32::consts::PI + forward.dot(dir).atan2(right.dot(dir));
    ((theta / (std::f32::consts::PI / 6.0)).floor() as i32).clamp(0, 11) as u32
}

/// From behind: sectors 7 to 11 (Q5: lopsided).
pub fn is_behind(forward: Vec3, right: Vec3, dir: Vec3) -> bool {
    (7..=11).contains(&sector(forward, right, dir))
}

/// One piece of gear's protection (Q15): per damage type
/// (`InstDamageType`), a fraction for the head, torso, arm and leg, and
/// whether it is used up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gear {
    pub protects: &'static [Protection],
}

/// Gear whose only entry is a flash protection of `$v` on the head and
/// torso (grenades.md section 10).
macro_rules! flash_gear {
    ($v:expr) => {
        Gear {
            protects: &[Protection {
                damage_type: FLASH,
                head: $v,
                torso: $v,
                arm: 0.0,
                leg: 0.0,
                consumable: false,
            }],
        }
    };
}

/// Every piece of gear there is, by name (`Gear` goes over the network as
/// its place here).
pub const GEARS: &[(&str, Gear)] = &[
    ("default body", DEFAULT_BODY_GEAR),
    ("light body", LIGHT_BODY_GEAR),
    ("heavy body", HEAVY_BODY_GEAR),
    ("default head", DEFAULT_HEAD_GEAR),
    ("gas mask", GAS_MASK),
    // Gear with a flash entry (grenades.md section 10, its torso value,
    // which is its head value too), by data name; a name with two records
    // takes its first. Only the flash entries: their other protection
    // isn't in a spec here.
    ("Anti-Flash", flash_gear!(1.0)),
    ("Skull_head", flash_gear!(1.0)),
    ("Codename_Raven", flash_gear!(1.0)),
    ("Prison_Recon_S_grade", flash_gear!(1.0)),
    ("Assassin_F", flash_gear!(1.0)),
    ("Assassin_M", flash_gear!(1.0)),
    ("Protect_flash_100", flash_gear!(1.0)),
    ("Ballistic_Mask", flash_gear!(0.8)),
    ("SnowMan_Head", flash_gear!(0.8)),
    ("Davil_Mask", flash_gear!(0.8)),
    ("Guy_Fawkes_mask", flash_gear!(0.8)),
    ("Protect_flash_75", flash_gear!(0.75)),
    ("Anti-Flash_70", flash_gear!(0.7)),
    ("Recruit_Survival_Bandana", flash_gear!(0.7)),
    ("Protect_flash_45", flash_gear!(0.55)),
    ("Protect_flash_55", flash_gear!(0.55)),
    ("Protect_fire_24", flash_gear!(0.55)),
    ("Anti-Flash_50", flash_gear!(0.5)),
    ("Codename_Viper", flash_gear!(0.5)),
    ("Powered_Suit_Helmet_Extreme", flash_gear!(0.5)),
    ("Stek_Headgear", flash_gear!(0.5)),
    ("RoboSkull_HauserRider_G", flash_gear!(0.5)),
    ("Protect_flash_25", flash_gear!(0.45)),
    ("Protect_flash_35", flash_gear!(0.45)),
    ("Protect_fire_22", flash_gear!(0.45)),
    ("RoboSkull_HauserRider_S", flash_gear!(0.4)),
    ("Protect_fire_20", flash_gear!(0.35)),
    ("Anti-Flash_30", flash_gear!(0.3)),
    ("Eyepatch_Skull", flash_gear!(0.3)),
    ("Powered_Suit_Helmet_Hard", flash_gear!(0.3)),
    ("Protect_gas_20", flash_gear!(0.25)),
    ("Eyepatch", flash_gear!(0.15)),
    ("Protect_flash_15", flash_gear!(0.15)),
    ("Protect_fire_16", flash_gear!(0.15)),
    ("Protect_gas_16", flash_gear!(0.15)),
    ("Protect_flash_12", flash_gear!(0.12)),
    ("Protect_flash_9", flash_gear!(0.09)),
    ("Infected", flash_gear!(1.0)),
    ("Host_Infected", flash_gear!(1.0)),
];

/// A piece of gear by its name in `GEARS`.
pub fn gear(name: &str) -> Option<Gear> {
    GEARS.iter().find(|(n, _)| *n == name).map(|(_, g)| *g)
}

/// The sum of `pieces`' torso entries for `damage_type` (flash protection
/// sums them, grenades.md section 10), before any clamp. Only entries
/// made for that type count (not an every-class kit such as CS:S's
/// kevlar, which is no `ArmorPieces`).
pub fn gear_torso(pieces: &[hits::ArmorPiece], damage_type: i32) -> f32 {
    let class = Some(class_of(damage_type));
    pieces.iter().flat_map(|p| p.entries.iter()).filter(|e| e.class == class).map(|e| e.torso).sum()
}

impl Gear {
    /// A piece with these entries, interned: the same entries are always
    /// the same slice (a piece goes over the network by its entries, and
    /// a client decoding it must not grow memory each time).
    pub fn new(entries: &[Protection]) -> Gear {
        static INTERNED: std::sync::Mutex<Vec<&'static [Protection]>> = std::sync::Mutex::new(Vec::new());
        let mut list = INTERNED.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(found) = list.iter().find(|s| s.iter().eq(entries.iter())) {
            return Gear { protects: found };
        }
        let leaked: &'static [Protection] = Box::leak(entries.to_vec().into_boxed_slice());
        list.push(leaked);
        Gear { protects: leaked }
    }
}

// A piece goes over the network by its entries: the install's pieces
// are not in `GEARS`.
impl Serialize for Gear {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.protects.serialize(s)
    }
}

impl<'de> Deserialize<'de> for Gear {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let entries = Vec::<Protection>::deserialize(d)?;
        Ok(Gear::new(&entries))
    }
}

/// What a piece of gear does against one damage type.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Protection {
    pub damage_type: i32,
    pub head: f32,
    pub torso: f32,
    pub arm: f32,
    pub leg: f32,
    pub consumable: bool,
}

impl Gear {
    /// The piece as Chimera's armour coverage (`weapon::hits`): its first
    /// entry per damage type, by class (`class_of`), the groin uncovered,
    /// worn by what it blocks when consumable.
    pub fn coverage(&self) -> impl Iterator<Item = hits::Coverage> + '_ {
        self.protects
            .iter()
            .enumerate()
            .filter(|(i, p)| self.protects[..*i].iter().all(|o| o.damage_type != p.damage_type))
            .map(|(_, p)| hits::Coverage {
                class: Some(class_of(p.damage_type)),
                head: p.head,
                torso: p.torso,
                arm: p.arm,
                leg: p.leg,
                groin: 0.0,
                wear: if p.consumable { 1.0 } else { 0.0 },
                except: 0,
            })
    }
}

pub(super) const fn bullet(head: f32, body: f32) -> Protection {
    Protection {
        damage_type: BULLET,
        head,
        torso: body,
        arm: body,
        leg: body,
        consumable: true,
    }
}

/// Body gear's burn protection: 0.5 everywhere, the head too.
pub(super) const BODY_BURN: Protection = Protection {
    damage_type: BURN,
    head: 0.5,
    torso: 0.5,
    arm: 0.5,
    leg: 0.5,
    consumable: true,
};

/// The spec's common gear ("Armour"): example values until the
/// characters spec gives the table (Q7). Body gear covers bullets and
/// burn, head gear bullets, the gas mask poison (not used up); nothing
/// covers melee or explosions.
pub const DEFAULT_BODY_GEAR: Gear = Gear {
    protects: &[bullet(0.0, 0.15), BODY_BURN],
};
pub const LIGHT_BODY_GEAR: Gear = Gear {
    protects: &[bullet(0.0, 0.05), BODY_BURN],
};
pub const HEAVY_BODY_GEAR: Gear = Gear {
    protects: &[bullet(0.0, 0.30), BODY_BURN],
};
pub const DEFAULT_HEAD_GEAR: Gear = Gear {
    protects: &[bullet(0.25, 0.0)],
};
pub const GAS_MASK: Gear = Gear {
    protects: &[Protection {
        damage_type: POISON,
        head: 1.0,
        torso: 1.0,
        arm: 1.0,
        leg: 1.0,
        consumable: false,
    }],
};

/// A shooter whose rounds go exactly where it looks: no spread cone, one
/// ray per shot (no extra pellets) and no recoil kick. Combat Arms' bots
/// (bots.md 7.3, Quirk 7), whose brain picks each shot's aim point.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[component(storage = "SparseSet")]
pub struct ExactAim;

/// Combat Arms' armour: Chimera's `weapon::Armor` (its amount: the
/// points, AP, over `HIT_POINTS`) with the gear worn as `ArmorPieces`
/// (`pieces_of`). The server's; replicated to clients (`game::net`).
pub fn gear_armour(points: f32, gear: &[Gear]) -> (Armor, ArmorPieces) {
    (
        Armor {
            amount: points / HIT_POINTS,
            helmet: false,
        },
        pieces_of(gear),
    )
}

/// The gear worn as Chimera's armour pieces (`Gear::coverage`), running
/// out by what they block.
pub fn pieces_of(gear: &[Gear]) -> ArmorPieces {
    ArmorPieces {
        pieces: gear.iter().map(|g| hits::ArmorPiece { entries: g.coverage().collect() }).collect(),
        depletion: hits::Depletion::Blocked,
        armour_quantum: None,
    }
}

/// A model's `MaxArmor` 100 with the default body gear.
pub fn default_armour() -> (Armor, ArmorPieces) {
    gear_armour(100.0, &[DEFAULT_BODY_GEAR])
}

/// Armour points (AP) of `armor`.
pub fn points(armor: &Armor) -> f32 {
    armor.amount * HIT_POINTS
}

/// What the armour step needs of a hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArmourHit {
    /// `InstDamageType`.
    pub damage_type: i32,
    /// The hit's location (torso for types without zones).
    pub location: Location,
    /// The zone factor (1 for types without zones).
    pub zone: f32,
    /// An infected victim: its groin counts as torso.
    pub infected: bool,
}

/// Damage `d` (hit points) of `hit` after armour: returns what health
/// loses, and takes the armour's share. With `lands` false (a refused hit)
/// the armour is left whole. The shared armour step
/// (`weapon::hits::absorb`, ratio 1, fractions kept, `Depletion::Blocked`)
/// with the gear as coverage (`Gear::coverage`), then the zone factor.
pub fn armour_and_zone(d: f32, hit: ArmourHit, armor: Option<&mut Armor>, pieces: Option<&ArmorPieces>, lands: bool) -> f32 {
    let zone = hit.zone;
    let Some(armor) = armor else { return (d * zone).max(0.0) };
    let location = if hit.infected && hit.location == Groin { Torso } else { hit.location };
    let ap = points(armor);
    let (entries, depletion) = crate::weapon::coverage(armor, pieces);
    let a = hits::absorb(d, part_of(location), class_of(hit.damage_type), Some(1.0), 0.0, ap, entries, depletion);
    if a.covered && lands {
        armor.amount = (ap - a.taken).max(0.0) / HIT_POINTS;
    }
    (a.to_health * zone).max(0.0)
}

/// Chimera's damage class of a damage type (`InstDamageType`): bullet,
/// burn, explosion (blast), poison, flash and melee as Chimera's, any
/// other as a class of Combat Arms' own (`DamageClass::OWN` + the type).
pub fn class_of(damage_type: i32) -> hits::DamageClass {
    match damage_type {
        BULLET => hits::DamageClass::BULLET,
        BURN => hits::DamageClass::BURN,
        EXPLOSION => hits::DamageClass::BLAST,
        POISON => hits::DamageClass::POISON,
        FLASH => hits::DamageClass::FLASH,
        MELEE => hits::DamageClass::MELEE,
        t => hits::DamageClass(hits::DamageClass::OWN.saturating_add(t.clamp(0, i32::from(u16::MAX)) as u16)),
    }
}

/// An armour location as Chimera's body part.
pub fn part_of(location: Location) -> crate::core::BodyPart {
    match location {
        Head => crate::core::BodyPart::Head,
        Torso => crate::core::BodyPart::Torso,
        Arm => crate::core::BodyPart::Arm,
        Leg => crate::core::BodyPart::Leg,
        Groin => crate::core::BodyPart::Groin,
    }
}

/// Steps 1 to 4 of "Damage per ray" before penetration and the victim:
/// base x the roll (`u` in [0, 1] picks within it) x `behind` x the band
/// at `distance` metres. Health units.
pub fn base_damage(d: &CaDamage, u: f32, behind: bool, distance: f32) -> f32 {
    let roll = 1.0 - d.roll + 2.0 * d.roll * u;
    let back = if behind { BEHIND_BONUS } else { 1.0 };
    d.damage * roll * back * band_scale(d, distance)
}

/// The range band's multiplier at `distance` metres (edges inclusive at
/// the top; beyond the last, or without bands, 1: Q8).
pub fn band_scale(d: &CaDamage, distance: f32) -> f32 {
    let Some(bands) = d.bands else { return 1.0 };
    // Edges are whole units: compare in units, a hair of float slack.
    let units = distance / d.unit;
    bands
        .iter()
        .find(|(edge, _)| units <= edge / d.unit + 1e-3)
        .map_or(1.0, |(_, m)| *m)
}

/// A uniform square spread: the aim's forward bent by (2 u1 - 1) `half`
/// right and (2 u2 - 1) `half` up, normalised (`half` = n / 1000).
pub fn square_dir(aim: Quat, half: f32, u1: f32, u2: f32) -> Vec3 {
    let f = aim * Vec3::NEG_Z;
    let r = aim * Vec3::X;
    let up = aim * Vec3::Y;
    (f + r * ((2.0 * u1 - 1.0) * half) + up * ((2.0 * u2 - 1.0) * half)).normalize()
}

