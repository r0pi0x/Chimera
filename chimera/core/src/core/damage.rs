use bevy::prelude::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::*;

/// Where on a body a hit landed (Source hitgroups).
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Hitgroup {
    #[default]
    Generic,
    Head,
    Chest,
    Stomach,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

/// Where on a body a hit landed: Chimera's body regions, the superset of
/// what the games tell apart (docs/DESIGN.md "Hits across games"). Source's
/// hitgroups are the coarse view (`hitgroup`), Combat Arms' locations
/// (`BodyPart`) the armour's; a game that tells fewer apart maps onto
/// these (`from_hitgroup`).
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BodyRegion {
    #[default]
    Generic,
    Head,
    Neck,
    Chest,
    Stomach,
    Groin,
    LeftUpperArm,
    LeftForearm,
    LeftHand,
    RightUpperArm,
    RightForearm,
    RightHand,
    LeftThigh,
    LeftShin,
    LeftFoot,
    RightThigh,
    RightShin,
    RightFoot,
}

/// What armour covers a region as (Combat Arms' locations): the torso
/// takes in the neck, chest, stomach and anything generic.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BodyPart {
    Head,
    Torso,
    Arm,
    Leg,
    Groin,
}

impl BodyRegion {
    /// Every region, in order (`index`).
    pub const ALL: [BodyRegion; 18] = [
        BodyRegion::Generic,
        BodyRegion::Head,
        BodyRegion::Neck,
        BodyRegion::Chest,
        BodyRegion::Stomach,
        BodyRegion::Groin,
        BodyRegion::LeftUpperArm,
        BodyRegion::LeftForearm,
        BodyRegion::LeftHand,
        BodyRegion::RightUpperArm,
        BodyRegion::RightForearm,
        BodyRegion::RightHand,
        BodyRegion::LeftThigh,
        BodyRegion::LeftShin,
        BodyRegion::LeftFoot,
        BodyRegion::RightThigh,
        BodyRegion::RightShin,
        BodyRegion::RightFoot,
    ];

    /// Its place in `ALL`.
    pub fn index(self) -> usize {
        self as usize
    }

    /// Source's hitgroup for it (what `Damage` and hit events carry): the
    /// neck counts as chest, the groin as stomach, every part of a limb as
    /// that limb.
    pub fn hitgroup(self) -> Hitgroup {
        use BodyRegion::*;
        match self {
            Generic => Hitgroup::Generic,
            Head => Hitgroup::Head,
            Neck | Chest => Hitgroup::Chest,
            Stomach | Groin => Hitgroup::Stomach,
            LeftUpperArm | LeftForearm | LeftHand => Hitgroup::LeftArm,
            RightUpperArm | RightForearm | RightHand => Hitgroup::RightArm,
            LeftThigh | LeftShin | LeftFoot => Hitgroup::LeftLeg,
            RightThigh | RightShin | RightFoot => Hitgroup::RightLeg,
        }
    }

    /// A hitgroup's region, for what knows only hitgroups: a limb's upper
    /// part.
    pub fn from_hitgroup(g: Hitgroup) -> Self {
        match g {
            Hitgroup::Generic => BodyRegion::Generic,
            Hitgroup::Head => BodyRegion::Head,
            Hitgroup::Chest => BodyRegion::Chest,
            Hitgroup::Stomach => BodyRegion::Stomach,
            Hitgroup::LeftArm => BodyRegion::LeftUpperArm,
            Hitgroup::RightArm => BodyRegion::RightUpperArm,
            Hitgroup::LeftLeg => BodyRegion::LeftThigh,
            Hitgroup::RightLeg => BodyRegion::RightThigh,
        }
    }

    /// The part armour covers it as.
    pub fn part(self) -> BodyPart {
        use BodyRegion::*;
        match self {
            Head => BodyPart::Head,
            Generic | Neck | Chest | Stomach => BodyPart::Torso,
            Groin => BodyPart::Groin,
            LeftUpperArm | LeftForearm | LeftHand | RightUpperArm | RightForearm | RightHand => BodyPart::Arm,
            LeftThigh | LeftShin | LeftFoot | RightThigh | RightShin | RightFoot => BodyPart::Leg,
        }
    }

    /// The hit priority of a shape that gives none (Combat Arms' server
    /// walk picks the highest, weapon_firing.md Corrections 5): its data's
    /// defaults by part (torso and groin 3, head 2, limbs 1).
    pub fn default_priority(self) -> f32 {
        match self.part() {
            BodyPart::Torso | BodyPart::Groin => 3.0,
            BodyPart::Head => 2.0,
            BodyPart::Arm | BodyPart::Leg => 1.0,
        }
    }
}

/// Damage to apply: normalized like `Health` (1.0 = a standard player's
/// full health).
#[derive(Message, Clone, Debug)]
pub struct Damage {
    pub target: Entity,
    pub attacker: Option<Entity>,
    pub amount: f32,
    pub point: Vec3,
    /// Direction the damage travelled (unit).
    pub dir: Vec3,
    pub hitgroup: Hitgroup,
    /// How it was dealt (breakables scale damage by it).
    pub kind: DamageKind,
    /// The weapon that dealt it when that isn't what the attacker holds
    /// now (a thrown grenade), for kill notices; None: the held one.
    pub weapon: Option<&'static str>,
    /// The push that came with it (kg·m/s, engine space), already given
    /// to a moving body; zero for none. Props read it (a frozen prop
    /// that a strong enough push unfreezes).
    pub force: Vec3,
}

/// Damage a map deals a player (trigger_hurt) before armour and the
/// player's whole points: the weapon layer splits it with the armour worn
/// when `armour` (CS:S takes armour for every damage type but falls and
/// drowning: measured on trigger_hurt, specs/source/triggers.md question
/// 4), then health takes whole points (2.5 is 2, nothing carried). Without
/// a weapon layer (`MapDamageArmor` absent) only the truncation applies
/// (`map_damage_lands`).
#[derive(Message, Clone, Debug)]
pub struct MapDamage {
    pub damage: Damage,
    pub armour: bool,
}

/// The weapon layer turns `MapDamage` into `Damage` itself (with armour).
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct MapDamageArmor;

/// `amount` (normalized: 1.0 is 100 points) down to whole points.
pub fn whole_points(amount: f32) -> f32 {
    (amount * 100.0 + 1e-3).floor() / 100.0
}

/// `MapDamage` without a weapon layer: whole points, no armour.
pub fn map_damage_lands(mut map: MessageReader<MapDamage>, mut damage: MessageWriter<Damage>) {
    for m in map.read() {
        let mut d = m.damage.clone();
        d.amount = whole_points(d.amount);
        if d.amount > 0.0 {
            damage.write(d);
        }
    }
}

impl Damage {
    /// A suicide (the `kill` command: generic damage from the target
    /// itself), which CS:S carries out even in noclip (`Noclip`).
    pub fn is_suicide(&self) -> bool {
        self.attacker == Some(self.target) && self.kind == DamageKind::Generic && self.weapon.is_none()
    }
}

/// How an explosion is drawn and heard: the temp entity's flags
/// (specs/source/game_entities.md 6, `env_explosion`'s spawnflag values).
/// None: all of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlastFlags(pub u16);

impl BlastFlags {
    /// No flash and no core (smoke, embers, fireballs).
    pub const NO_FIREBALL: u16 = 4;
    /// No scorch on what's under it.
    pub const NO_DECAL: u16 = 16;
    /// No client explosion sound.
    pub const NO_SOUND: u16 = 64;
    /// No smoke puffs in the core.
    pub const NO_FIREBALL_SMOKE: u16 = 256;
    /// No debris (spark streaks, flecks).
    pub const NO_PARTICLES: u16 = 512;
    /// No spark showers under water (specs/hl2dm/effects_sounds.md, the
    /// explosion's sparks): every explosion made by code carries it.
    pub const NO_SPARKS: u16 = 32;
    /// The bits above, out of a map entity's spawnflags.
    pub fn from_spawnflags(flags: u32) -> Self {
        let known =
            Self::NO_FIREBALL | Self::NO_DECAL | Self::NO_SOUND | Self::NO_FIREBALL_SMOKE | Self::NO_PARTICLES | Self::NO_SPARKS;
        Self(flags as u16 & known)
    }
    pub fn has(self, flag: u16) -> bool {
        self.0 & flag != 0
    }
}

/// An explosion to apply (radius damage and pushes, as a grenade's blast
/// with this magnitude and radius): from map logic (exploding props).
/// `damage` is normalized like `Damage::amount`, `radius` in meters.
#[derive(Message, Clone, Debug)]
pub struct Explosion {
    pub origin: Vec3,
    pub damage: f32,
    pub radius: f32,
    /// Who the damage counts for (kill notices, friendly fire).
    pub attacker: Option<Entity>,
    /// What exploded (it takes none of the damage).
    pub inflictor: Option<Entity>,
    /// A sound entry to play at the origin, if any.
    pub sound: Option<String>,
    /// The weapon kills count as (kill notices); None: a map explosion
    /// (`weapon::grenade::EXPLOSION_WEAPON`).
    pub weapon: Option<&'static str>,
    /// Its scorch mark goes at the next round restart (the bomb's, as in
    /// CS:S; `map::PlaceDecal::round`); others stay with the map.
    pub round_scorch: bool,
    /// Falloff override: a bell of this σ (m) instead of the explosion
    /// rule's (`weapon::grenade::Blast::bell_sigma`); None: the rule's,
    /// its σ scaled with this radius.
    pub bell_sigma: Option<f32>,
    /// Armour override (`Blast::armor_ratio`); None: the rule's.
    pub armor_ratio: Option<f32>,
    /// Nothing between stops or shields it (`Blast::through_walls`).
    pub through_walls: bool,
    /// How it is drawn and heard.
    pub flags: BlastFlags,
    /// What its damage is: blast (`env_explosion`'s default, the bomb's),
    /// blast with blast-surface (exploding props, `env_explosion`'s
    /// "surface only"), generic (`env_explosion`'s "generic damage").
    pub kind: DamageKind,
}

/// How damage was dealt (Source damage types, as far as anything here
/// tells them apart).
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageKind {
    #[default]
    Generic,
    /// Hitscan shots.
    Bullet,
    /// Melee swings (the knife).
    Melee,
    Blast,
    /// Blast with Source's blast-surface bit (64 + 134217728): exploding
    /// props and an `env_explosion` with "surface only"
    /// (specs/source/prop_damage.md, hl2dm/rules.md spawn grace). Blast
    /// to every rule (`is_blast`) but one that asks for blast alone.
    BlastSurface,
    /// Crushed by a mover.
    Crush,
    Fall,
    /// Fire (Source's burn type): env_fire and entity flames.
    Burn,
    /// Drowning: head under water too long (`games::cs_source::drown`).
    Drown,
    /// Source's poison and radiation types (a trigger_hurt's `damagetype`
    /// bits 131072 and 262144): HL2's suit lets them through.
    Poison,
    Radiation,
}

impl DamageKind {
    /// Carries the blast bit (blast alone or with blast-surface).
    pub fn is_blast(self) -> bool {
        matches!(self, Self::Blast | Self::BlastSurface)
    }
}

/// Give a character items a map names (`weapon_ak47`, `item_kevlar`, with
/// counts), after taking all its weapons when `strip`: the logic layer's
/// game_player_equip and player_weaponstrip; the weapon layer gives them.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct Equip {
    pub target: Entity,
    pub items: Vec<(String, u32)>,
    pub strip: bool,
}

/// Damage kinds a character takes none of (a map's damage filter on it:
/// bhop and surf maps turn fall damage off this way).
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct DamageFilter {
    pub blocked: Vec<DamageKind>,
}

/// The map's `IgnoreFallDamage` / `IgnoreFallDamageWithoutReset` inputs
/// on a player (specs/hl2dm/movement.md "Falling and landing"): a landing
/// before `until` (s, `SimClock::now`) deals no fall damage; with `reset`
/// the first such landing ends the window.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct IgnoreFallDamage {
    pub until: f64,
    pub reset: bool,
}


/// What a blast or a flash does to a character's hearing for a while
/// (Source: a player DSP preset): what they hear is mixed `mix` toward a
/// muffled copy (low-passed at `cutoff` Hz, times `wet_gain`), and a
/// ringing tone (`ring_hz`, amplitude `ring_gain` times the mix) plays.
/// Full for `hold` seconds, then fading out over `fade` (linearly, or
/// exponentially down to 1 % by its end).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HearingEffect {
    pub hold: f32,
    pub fade: f32,
    pub exponential: bool,
    pub mix: f32,
    pub cutoff: f32,
    pub wet_gain: f32,
    pub ring_hz: f32,
    pub ring_gain: f32,
}

impl HearingEffect {
    /// Strength (0-1) `t` seconds after it started.
    pub fn level(&self, t: f32) -> f32 {
        if t < 0.0 || t >= self.hold + self.fade {
            0.0
        } else if t <= self.hold {
            1.0
        } else {
            let x = (t - self.hold) / self.fade;
            if self.exponential {
                // e^-4.6 = 1 %: then cut.
                (-4.6 * x).exp()
            } else {
                1.0 - x
            }
        }
    }

    /// Seconds until it is over.
    pub fn length(&self) -> f32 {
        self.hold + self.fade
    }
}

/// A character's hearing is hit (`HearingEffect`): clients apply it to
/// what the local player hears.
#[derive(Message, Clone, Copy, Debug)]
pub struct Deafened {
    pub target: Entity,
    pub effect: HearingEffect,
}

/// A character's blindness (a flashbang): the screen goes white at
/// `alpha` (0-1), holds until `fade_start` (seconds, the simulation
/// clock) and fades out linearly by `end`. Clients draw it; bots don't
/// see while it is strong.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Blinded {
    pub alpha: f32,
    pub fade_start: f64,
    pub end: f64,
}

impl Blinded {
    /// Whiteness at `now`.
    pub fn alpha_at(&self, now: f64) -> f32 {
        if now >= self.end {
            0.0
        } else if now <= self.fade_start || self.end <= self.fade_start {
            self.alpha
        } else {
            self.alpha * ((self.end - now) / (self.end - self.fade_start)) as f32
        }
    }

    /// `alpha_at` as a plain function (a default fade curve).
    pub fn alpha_at_now(b: &Blinded, now: f64) -> f32 {
        b.alpha_at(now)
    }

    /// The stronger of this blindness and `other` (a new flash only
    /// raises what is left of the current one).
    pub fn raised(self, other: Blinded, now: f64) -> Blinded {
        if self.alpha_at(now) >= other.alpha && self.end >= other.end {
            self
        } else if other.alpha >= self.alpha_at(now) && other.end >= self.end {
            other
        } else {
            // One is whiter, the other lasts longer: keep both maxima.
            Blinded {
                alpha: self.alpha_at(now).max(other.alpha),
                fade_start: self.fade_start.max(other.fade_start),
                end: self.end.max(other.end),
            }
        }
    }
}

/// Something sight doesn't pass (a smoke cloud): a sphere. A sight line
/// is blocked when more than `radius` of it lies inside (Counter-Strike
/// bots don't see through smoke; the exact rule isn't known, see
/// specs/cs_source/grenades.md Q16).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct SightBlocker {
    pub centre: Vec3,
    pub radius: f32,
}

impl SightBlocker {
    /// Length of the segment `from`-`to` inside the sphere.
    pub fn chord(&self, from: Vec3, to: Vec3) -> f32 {
        let d = to - from;
        let len = d.length();
        if len < 1e-6 || self.radius <= 0.0 {
            return 0.0;
        }
        let dir = d / len;
        let t = (self.centre - from).dot(dir);
        let closest = from + dir * t;
        let h2 = self.radius * self.radius - closest.distance_squared(self.centre);
        if h2 <= 0.0 {
            return 0.0;
        }
        let h = h2.sqrt();
        ((t + h).min(len) - (t - h).max(0.0)).max(0.0)
    }

    pub fn blocks(&self, from: Vec3, to: Vec3) -> bool {
        self.chord(from, to) > self.radius
    }
}

/// `mp_playerid` (the server's, sent to clients): whose names the target
/// ID shows: 0 everyone's, 1 teammates' only, 2 nobody's.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerIdMode(pub u8);

/// Whether characters on the same team hurt each other (CS:S
/// `mp_friendlyfire`, default 0). Team 0 (no team) is never friendly.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FriendlyFire(pub u8);

/// Whether team rules refuse this damage: teammates don't hurt each other
/// unless friendly fire is on (your own damage, e.g. falling, always
/// counts).
pub fn refused_by_team(d: &Damage, teams: &Query<&Team>, friendly_fire: Option<&FriendlyFire>) -> bool {
    let teammate = d
        .attacker
        .filter(|a| *a != d.target)
        .is_some_and(|a| matches!((teams.get(a), teams.get(d.target)), (Ok(x), Ok(y)) if x == y && x.0 != 0));
    teammate && !friendly_fire.is_some_and(|f| f.0 != 0)
}

/// Why damage is refused (`DamageRules::refusal`). What a refused hit
/// still does depends on why (CS:S, measured 2026-10-17;
/// specs/cs_source/weapons.md "Refused hits"): a teammate's hit with
/// friendly fire off does nothing at all (no armour, sound, blood,
/// slowdown or flinch); a noclipping player's hit still bleeds and
/// flinches, nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The target noclips (`Noclip`).
    Noclip,
    /// A teammate's, friendly fire off (`refused_by_team`).
    Team,
    /// The map made the target invulnerable (`MapControls`).
    Invulnerable,
    /// The target's damage filter blocks this kind (`DamageFilter`).
    Filter,
}

/// What decides whether damage is refused before it reaches health:
/// team rules, a map's invulnerability and damage filters, a noclipping
/// target (`Noclip`); and whether it lands (`lands`: not refused, and not
/// on a `God` target, which `apply_damage`'s health query leaves out).
#[derive(bevy::ecs::system::SystemParam)]
pub struct DamageRules<'w, 's> {
    noclip: Query<'w, 's, (), With<Noclip>>,
    god: Query<'w, 's, (), With<God>>,
    teams: Query<'w, 's, &'static Team>,
    filters: Query<'w, 's, &'static DamageFilter>,
    controls: Query<'w, 's, &'static MapControls>,
    friendly_fire: Option<Res<'w, FriendlyFire>>,
}

impl DamageRules<'_, '_> {
    pub fn refuse(&self, d: &Damage) -> bool {
        self.refusal(d).is_some()
    }

    /// Why `d` is refused, if it is.
    pub fn refusal(&self, d: &Damage) -> Option<Refusal> {
        if self.noclip.contains(d.target) && !d.is_suicide() {
            Some(Refusal::Noclip)
        } else if refused_by_team(d, &self.teams, self.friendly_fire.as_deref()) {
            Some(Refusal::Team)
        } else if self.controls.get(d.target).is_ok_and(|c| c.invulnerable) {
            Some(Refusal::Invulnerable)
        } else if self.filters.get(d.target).is_ok_and(|f| f.blocked.contains(&d.kind)) {
            Some(Refusal::Filter)
        } else {
            None
        }
    }

    /// Whether `d` is taken: not refused, the target not under `God`.
    /// Only such hits cost armour (`weapon::HitArmor`), make hurt sounds
    /// and slow the victim down.
    pub fn lands(&self, d: &Damage) -> bool {
        !self.god.contains(d.target) && !self.refuse(d)
    }
}

/// Subtract damage from health; announce deaths once (public so other
/// systems can order themselves after it).
#[allow(clippy::type_complexity)]
pub fn apply_damage(
    mut damage: MessageReader<Damage>,
    mut health: Query<(&mut Health, Option<&mut DamageRemainder>, Option<&DamageScale>, Has<HealthHeld>), Without<God>>,
    rules: DamageRules,
    mut died: MessageWriter<Died>,
    mut taken: MessageWriter<DamageTaken>,
) {
    for d in damage.read() {
        if rules.refuse(d) {
            continue;
        }
        let Ok((mut h, remainder, scale, held)) = health.get_mut(d.target) else { continue };
        if h.current <= 0.0 {
            continue;
        }
        let mut d = std::borrow::Cow::Borrowed(d);
        if let Some(s) = scale.filter(|s| s.0 != 1.0) {
            d.to_mut().amount *= s.0;
        }
        if let Some(mut r) = remainder {
            // Whole points, the fraction carried (HL2's health): a hit
            // that comes to no whole point does nothing.
            let Some(points) = r.take(d.amount * 100.0) else { continue };
            d.to_mut().amount = points / 100.0;
        }
        let d = d.as_ref();
        taken.write(DamageTaken {
            damage: d.clone(),
            directed: d.attacker.is_some_and(|a| a != d.target),
        });
        if held {
            continue;
        }
        h.current -= d.amount;
        if h.current <= 0.0 {
            h.current = 0.0;
            died.write(Died {
                entity: d.target,
                attacker: d.attacker,
                damage: d.clone(),
            });
        }
    }
}

/// A sandbox's practice target (`chimera_sim::minigame::targets`): what
/// counts, races or tells kills leaves it out (the scores and the ladder's
/// lead, the announcer; the HUD's race chip and kill feed, which read it
/// or `targets::is_target`). Its kills and deaths are measured instead.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PracticeTarget;

/// Takes hits but keeps its health (a sandbox's protected practice target,
/// `chimera_sim::minigame::targets`): `apply_damage` writes its
/// `DamageTaken` as for anyone (after `DamageScale` and
/// `DamageRemainder`), but takes nothing from `Health` and writes no
/// `Died`. Unlike `God`, every hit lands (armour and a shield take theirs:
/// whoever put it on puts those back).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(storage = "SparseSet")]
pub struct HealthHeld;

/// What every hit on this character's health counts (a game's rule for
/// its characters: Halo's player in a party app, where another game's
/// damage counts 1.5 times, `halo_ce::game::cross_game`): `apply_damage`
/// multiplies each `Damage` on it by this before health (and
/// `DamageRemainder`) takes it, so `Damage::amount` stays what the hit
/// dealt and its `DamageTaken` and `Died` carry what health lost. A game
/// whose own hits must land as they are writes them divided by it.
/// Without it, health takes the damage as it comes.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[component(storage = "SparseSet")]
pub struct DamageScale(pub f32);

/// Health that takes whole points and carries the fraction of each hit
/// (Half-Life 2's, specs/hl2dm/suit.md "Fractional damage"): the
/// fractions add up here, and when they reach a point the next hit takes
/// it too (at most one a hit). A hit that comes to no whole point does
/// nothing to health (no `DamageTaken`). Never reset, not even on
/// respawn. Without it, health takes the damage as it comes.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DamageRemainder(pub f32);

impl DamageRemainder {
    /// The whole points a hit of `points` takes (None: none), carrying its
    /// fraction.
    pub fn take(&mut self, points: f32) -> Option<f32> {
        // A hair of float noise (0.2 x 5 is 1.0000001) is no fraction.
        let points = (points * 1e4).round() / 1e4;
        let mut whole = points.floor();
        self.0 += points - whole;
        if self.0 >= 1.0 - 1e-4 {
            whole += 1.0;
            self.0 = (self.0 - 1.0).max(0.0);
        }
        (whole > 0.0).then_some(whole)
    }
}

