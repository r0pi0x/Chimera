//! The numbers of the one hit path every gun shares
//! (docs/DESIGN.md "Hits across games"): a gun's damage per body region
//! (`RegionScale`, `RegionDamage`: its game's table), what armour covers
//! per region and damage class (`Coverage`, `ArmorPiece`, `ArmorRules`:
//! the armour's game's data) and how a hit splits between armour and
//! health (`absorb`), so any game's gun works on any game's body and
//! armour without anything written per pair.
//!
//! `absorb`: P, the coverage of the hit region's part for the damage
//! class (summed over the pieces worn, at most 1), lets through
//! `(1 - 2P) + P x ratio` of the raw damage (the gun's armour ratio:
//! CS:S's `WeaponArmorRatio`, 1 for a gun with none); armour loses the
//! rest times the pieces' wear; both truncated by the gun's quantum (what
//! armour has left is truncated). CS:S's kevlar (P 0.5, wear 0.5) is then
//! weapons.md M7's split exactly (`deliver::armor_split`); Combat Arms'
//! gear (its protection fractions, wear 1 when consumable, ratio 1) is
//! weapon_firing.md's armour step. How armour runs out is its game's rule
//! (`Depletion`). HL2's suit (lift C2, specs/hl2dm/suit.md "Armour") is
//! the same shape: P 0.8 on every part, wear 1 (2 against blasts), with a
//! catch-all entry that leaves classes out (`Coverage::except`: falls,
//! drowning, poison, radiation) and its own armour quantum
//! (`absorb_quanta`: whole armour points, health's fraction kept for its
//! remainder, `core::DamageRemainder`).

use crate::core::{BodyPart, BodyRegion, DamageKind};

/// A damage multiplier per body region (a gun's table; `BodyRegion`'s
/// order).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionScale(pub [f32; 18]);

impl Default for RegionScale {
    /// Every region 1.
    fn default() -> Self {
        Self([1.0; 18])
    }
}

impl RegionScale {
    /// The multiplier for `region`.
    pub fn get(&self, region: BodyRegion) -> f32 {
        self.0[region.index()]
    }

    /// Set `region`'s multiplier.
    pub fn set(&mut self, region: BodyRegion, value: f32) {
        self.0[region.index()] = value;
    }

    /// From Source's five hitgroup multipliers: each region takes its
    /// hitgroup's (`BodyRegion::hitgroup`: the neck as chest, the groin as
    /// stomach, every part of a limb as the limb), Generic 1.
    pub fn from_hitgroups(h: &super::HitgroupScale) -> Self {
        let mut s = Self::default();
        for r in BodyRegion::ALL {
            s.set(r, h.get(r.hitgroup()));
        }
        s
    }
}

/// What armour coverage is looked up by: Chimera's damage classes (a
/// game maps its own types onto these; numbers past `OWN` are a game's
/// own, which only its armour lists).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct DamageClass(pub u16);

impl DamageClass {
    pub const GENERIC: Self = Self(0);
    pub const BULLET: Self = Self(1);
    pub const MELEE: Self = Self(2);
    pub const BLAST: Self = Self(3);
    pub const BURN: Self = Self(4);
    pub const POISON: Self = Self(5);
    pub const FLASH: Self = Self(6);
    pub const CRUSH: Self = Self(7);
    /// Falls, drowning and radiation: armour that leaves classes out
    /// (HL2's suit, `Coverage::except`) tells them apart; CS:S's kevlar
    /// and Combat Arms' gear treat them as any class.
    pub const FALL: Self = Self(8);
    pub const DROWN: Self = Self(9);
    pub const RADIATION: Self = Self(10);
    /// The first number of a game's own classes.
    pub const OWN: u16 = 1000;

    /// The class of a damage kind.
    pub fn of(kind: DamageKind) -> Self {
        match kind {
            DamageKind::Bullet => Self::BULLET,
            DamageKind::Melee => Self::MELEE,
            DamageKind::Blast => Self::BLAST,
            DamageKind::Burn => Self::BURN,
            DamageKind::Crush => Self::CRUSH,
            DamageKind::Fall => Self::FALL,
            DamageKind::Drown => Self::DROWN,
            DamageKind::Generic => Self::GENERIC,
        }
    }
}

/// What a piece of armour does against one damage class (None: every
/// class): the fraction it covers on each part, and how many armour
/// points it loses per point it takes (CS:S's kevlar 0.5; Combat Arms'
/// gear 1 if used up, else 0).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Coverage {
    pub class: Option<DamageClass>,
    pub head: f32,
    pub torso: f32,
    pub arm: f32,
    pub leg: f32,
    pub groin: f32,
    pub wear: f32,
    /// Classes this entry leaves out (bit n: class n, for classes below
    /// 64): a catch-all entry (`class` None) that covers every class but
    /// those (HL2's suit: all but fall, drown, poison, radiation, and
    /// blasts, which its own entry covers at another wear).
    #[serde(default)]
    pub except: u64,
}

impl Coverage {
    /// The fraction on `part`.
    pub fn at(&self, part: BodyPart) -> f32 {
        match part {
            BodyPart::Head => self.head,
            BodyPart::Torso => self.torso,
            BodyPart::Arm => self.arm,
            BodyPart::Leg => self.leg,
            BodyPart::Groin => self.groin,
        }
    }

    /// Whether it applies to `class`.
    pub fn against(&self, class: DamageClass) -> bool {
        self.class.is_none_or(|c| c == class) && !(class.0 < 64 && self.except & (1 << class.0) != 0)
    }

    /// `except` for these classes.
    pub fn except_of(classes: &[DamageClass]) -> u64 {
        classes.iter().filter(|c| c.0 < 64).fold(0, |m, c| m | 1 << c.0)
    }
}

/// How armour runs out (its game's rule): when it can't take its share,
/// health takes `raw - amount / wear` (`Cost`, CS:S: measured on C4
/// blasts, objectives.md Q3) or `raw - amount` once what it blocks
/// reaches what is left (`Blocked`, Combat Arms' armour step).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Depletion {
    #[default]
    Cost,
    Blocked,
}

/// One piece of a game's own armour (`weapon::ArmorPieces`: Combat Arms'
/// gear): its coverage entries, at most one per damage class.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ArmorPiece {
    pub entries: Vec<Coverage>,
}

/// What armour did to one hit (`absorb`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Absorbed {
    /// What health takes (truncated by the quantum).
    pub to_health: f32,
    /// What the armour loses.
    pub taken: f32,
    /// Armour covered the part (it had some, coverage there, and the gun's
    /// ratio applies).
    pub covered: bool,
}

/// Split `raw` damage on `part` between armour of `amount` (pieces'
/// coverage entries `entries`, each piece's first entry for the class)
/// and health: the gun's `ratio` (None: armour doesn't apply), damage
/// `class`, `quantum` (0 keeps fractions), the armour's `depletion` rule.
/// See the module doc.
pub fn absorb<'a>(
    raw: f32,
    part: BodyPart,
    class: DamageClass,
    ratio: Option<f32>,
    quantum: f32,
    amount: f32,
    entries: impl IntoIterator<Item = &'a Coverage>,
    depletion: Depletion,
) -> Absorbed {
    absorb_quanta(raw, part, class, ratio, (quantum, quantum), amount, entries, depletion)
}

/// `absorb` with the armour's own quantum: `quanta` is (health's, what
/// armour has left's). HL2's suit loses whole armour points while health
/// keeps the fraction (its remainder, `core::DamageRemainder`).
#[allow(clippy::too_many_arguments)]
pub fn absorb_quanta<'a>(
    raw: f32,
    part: BodyPart,
    class: DamageClass,
    ratio: Option<f32>,
    quanta: (f32, f32),
    amount: f32,
    entries: impl IntoIterator<Item = &'a Coverage>,
    depletion: Depletion,
) -> Absorbed {
    let (quantum, armour_quantum) = quanta;
    let uncovered = Absorbed {
        to_health: super::deliver::quantize(raw, quantum),
        taken: 0.0,
        covered: false,
    };
    let Some(ratio) = ratio else { return uncovered };
    if amount <= 0.0 {
        return uncovered;
    }
    let (mut sum, mut worn) = (0.0f32, 0.0f32);
    for e in entries.into_iter().filter(|e| e.against(class)) {
        let c = e.at(part);
        sum += c;
        worn += c * e.wear;
    }
    if sum <= 0.0 {
        return uncovered;
    }
    let p = sum.clamp(0.0, 1.0);
    let wear = worn / sum;
    let through = ((1.0 - 2.0 * p) + p * ratio).clamp(0.0, 1.0);
    let to_health = raw * through;
    let blocked = raw - to_health;
    let cost = blocked * wear;
    let runs_out = match depletion {
        Depletion::Cost => cost > amount,
        Depletion::Blocked => !(amount - blocked > 0.0 || wear <= 0.001),
    };
    if runs_out {
        let health = match depletion {
            Depletion::Cost => raw - amount / wear,
            Depletion::Blocked => raw - amount,
        };
        return Absorbed {
            to_health: super::deliver::quantize(health.max(0.0), quantum),
            taken: cost.min(amount),
            covered: true,
        };
    }
    let left = if armour_quantum > 0.0 {
        super::deliver::quantize(amount - cost, armour_quantum)
    } else {
        amount - cost
    };
    Absorbed {
        to_health: super::deliver::quantize(to_health, quantum),
        taken: amount - left,
        covered: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapon::{HELMET, KEVLAR, armor_split};

    #[test]
    fn kevlar_is_cs_s_measured_split_exactly() {
        // weapons.md M7 and objectives.md Q3, over a sweep of damage,
        // ratios and armour left: bit for bit `armor_split`.
        for raw in [0.2, 0.15, 0.65, 0.24477, 0.35946, 0.26, 3.76, 4.47, 0.4373, 1.44, 0.0991] {
            for ratio in [1.0, 1.05, 1.4, 1.5, 1.55, 1.7, 1.95, 2.0] {
                for amount in [1.0, 0.98, 0.1, 0.03, 0.005] {
                    for quantum in [0.01, 0.0] {
                        let a = absorb(raw, BodyPart::Torso, DamageClass::BULLET, Some(ratio), quantum, amount, &[KEVLAR], Depletion::Cost);
                        let (h, t) = armor_split(raw, ratio, amount, quantum);
                        assert_eq!((a.to_health, a.taken, a.covered), (h, t, true), "{raw} x {ratio} on {amount} ({quantum})");
                    }
                }
            }
        }
    }

    #[test]
    fn kevlar_covers_by_part_and_the_helmet_the_head() {
        let hit = |part, entries: &[Coverage]| {
            absorb(0.36, part, DamageClass::BULLET, Some(1.55), 0.01, 1.0, entries, Depletion::Cost).covered
        };
        for part in [BodyPart::Torso, BodyPart::Arm, BodyPart::Groin] {
            assert!(hit(part, &[KEVLAR]), "{part:?}");
        }
        assert!(!hit(BodyPart::Leg, &[KEVLAR, HELMET]));
        assert!(!hit(BodyPart::Head, &[KEVLAR]));
        assert!(hit(BodyPart::Head, &[KEVLAR, HELMET]));
        // No ratio, no armour points: nothing.
        assert!(!absorb(0.36, BodyPart::Torso, DamageClass::BULLET, None, 0.01, 1.0, &[KEVLAR], Depletion::Cost).covered);
        assert!(!absorb(0.36, BodyPart::Torso, DamageClass::BULLET, Some(1.0), 0.01, 0.0, &[KEVLAR], Depletion::Cost).covered);
    }

    /// Combat Arms' default body gear and head gear (weapon_firing.md
    /// "Armour"), as coverage.
    const BODY: Coverage = Coverage {
        class: Some(DamageClass::BULLET),
        head: 0.0,
        torso: 0.15,
        arm: 0.15,
        leg: 0.15,
        groin: 0.0,
        wear: 1.0,
        except: 0,
    };
    const HEAD: Coverage = Coverage {
        class: Some(DamageClass::BULLET),
        head: 0.25,
        torso: 0.0,
        arm: 0.0,
        leg: 0.0,
        groin: 0.0,
        wear: 1.0,
        except: 0,
    };

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn combat_arms_gear_is_its_armour_step() {
        // Hit points: cases 6, 7, 9, 48 (the zone factor comes after).
        let ca = |raw, part, amount, entries: &[Coverage]| {
            absorb(raw, part, DamageClass::BULLET, Some(1.0), 0.0, amount, entries, Depletion::Blocked)
        };
        let a = ca(35.0, BodyPart::Torso, 100.0, &[BODY]);
        assert!(close(a.to_health, 29.75) && close(a.taken, 5.25), "{a:?}");
        let a = ca(35.0, BodyPart::Head, 100.0, &[BODY, HEAD]);
        assert!(close(a.to_health, 26.25) && close(a.taken, 8.75), "{a:?}");
        let a = ca(35.0, BodyPart::Torso, 3.0, &[BODY]);
        assert!(close(a.to_health, 32.0) && close(a.taken, 3.0), "{a:?}");
        let a = ca(35.0, BodyPart::Groin, 100.0, &[BODY, HEAD]);
        assert!(close(a.to_health, 35.0) && a.taken == 0.0 && !a.covered, "{a:?}");
        // Another class: not covered.
        let a = absorb(35.0, BodyPart::Torso, DamageClass::MELEE, Some(1.0), 0.0, 100.0, &[BODY], Depletion::Blocked);
        assert!(close(a.to_health, 35.0) && a.taken == 0.0, "{a:?}");
        // Not used up (wear 0): blocks, never runs out.
        let mask = Coverage { wear: 0.0, ..BODY };
        let a = ca(35.0, BodyPart::Torso, 1.0, &[mask]);
        assert!(close(a.to_health, 29.75) && a.taken == 0.0, "{a:?}");
    }

    #[test]
    fn across_games_the_shared_rule_applies() {
        // A CS:S gun (ratio 1.55) on Combat Arms' body gear blocks
        // 0.15 x 0.45; a Combat Arms gun (ratio 1) on kevlar half.
        let a = absorb(36.0, BodyPart::Torso, DamageClass::BULLET, Some(1.55), 1.0, 100.0, &[BODY], Depletion::Blocked);
        assert_eq!(a.to_health, 33.0, "{a:?}");
        let a = absorb(0.35, BodyPart::Torso, DamageClass::BULLET, Some(1.0), 0.0, 1.0, &[KEVLAR], Depletion::Cost);
        assert!(close(a.to_health, 0.175) && close(a.taken, 0.0875), "{a:?}");
    }

    /// HL2's suit (specs/hl2dm/suit.md tests 1-5): 0.8 on every part, a
    /// catch-all entry leaving out falls, drowning, poison, radiation and
    /// blasts, blasts at wear 2; armour in whole points, health exact.
    #[test]
    fn a_suit_leaving_classes_out() {
        let all = |class, wear, except| Coverage {
            class,
            head: 0.8,
            torso: 0.8,
            arm: 0.8,
            leg: 0.8,
            groin: 0.8,
            wear,
            except,
        };
        let out = Coverage::except_of(&[
            DamageClass::FALL,
            DamageClass::DROWN,
            DamageClass::POISON,
            DamageClass::RADIATION,
            DamageClass::BLAST,
        ]);
        let suit = [all(None, 1.0, out), all(Some(DamageClass::BLAST), 2.0, 0)];
        let hit = |raw: f32, class, armour: f32| {
            let a = absorb_quanta(raw / 100.0, BodyPart::Leg, class, Some(1.0), (0.0, 0.01), armour / 100.0, &suit, Depletion::Cost);
            ((a.to_health * 100.0 * 1e3).round() / 1e3, ((armour / 100.0 - a.taken) * 100.0).round())
        };
        assert_eq!(hit(30.0, DamageClass::BULLET, 100.0), (6.0, 76.0));
        assert_eq!(hit(30.0, DamageClass::BLAST, 100.0), (6.0, 52.0));
        assert_eq!(hit(30.0, DamageClass::BULLET, 10.0), (20.0, 0.0));
        assert_eq!(hit(30.0, DamageClass::BLAST, 10.0), (25.0, 0.0));
        assert_eq!(hit(8.0, DamageClass::BULLET, 100.0), (1.6, 93.0), "whole armour points, health exact");
        for class in [DamageClass::FALL, DamageClass::DROWN, DamageClass::POISON, DamageClass::RADIATION] {
            assert_eq!(hit(10.0, class, 50.0), (10.0, 50.0), "{class:?} bypasses");
        }
        assert_eq!(hit(10.0, DamageClass::GENERIC, 100.0), (2.0, 92.0));
        // A game's own class (Combat Arms') is covered by the catch-all.
        assert_eq!(hit(10.0, DamageClass(DamageClass::OWN + 3), 100.0), (2.0, 92.0));
        // CS:S's kevlar leaves nothing out.
        assert!(KEVLAR.against(DamageClass::FALL) && KEVLAR.against(DamageClass::DROWN));
    }

    #[test]
    fn regions_take_their_hitgroup_s_multiplier() {
        let s = RegionScale::from_hitgroups(&crate::weapon::HitgroupScale {
            head: 4.0,
            chest: 1.0,
            stomach: 1.25,
            arm: 1.0,
            leg: 0.75,
        });
        assert_eq!(s.get(BodyRegion::Head), 4.0);
        assert_eq!(s.get(BodyRegion::Groin), 1.25);
        assert_eq!(s.get(BodyRegion::Neck), 1.0);
        assert_eq!(s.get(BodyRegion::LeftFoot), 0.75);
        assert_eq!(s.get(BodyRegion::RightHand), 1.0);
        assert_eq!(s.get(BodyRegion::Generic), 1.0);
    }
}
