//! The random ladder's power curve (backlog "A natural power curve for the
//! random ladder"; DESIGN.md "Gun game rules"): weak guns first, power
//! rising, then the hard-hitting ones, specials, a grenade, melee last.
//! Its numbers are data, `data/power_curve.toml` (`PowerCurve`), tunable
//! per definition (`power_curve = { shares = { peak = 0.3 }, jitter =
//! 0.02 }` in a ladder's rules).
//!
//! - **Bands** (`Band`): warm-up, rising, peak, heavy, specials, grenade,
//!   melee, in that order. Each gun class goes to a band by the data
//!   (`[classes]`: pistols warm up; SMGs and shotguns rise; rifles and
//!   carbines peak; machine guns, marksman rifles, snipers and heavy
//!   pistols are heavy; launchers and flamethrowers special; grenades are
//!   the grenade band; melee weapons the knife finale's). A pistol whose
//!   body hit does `heavy_pistol_damage` or more plays as a heavy pistol
//!   (`band_of`: Combat Arms' install Desert Eagle, a copy of its
//!   Beretta's document).
//! - **Shares** (`band_counts`): each band has a share of the ladder
//!   (default 15/20/28/15/8/7/7%). Rung i of N sits at (i + 0.5) / N and
//!   takes the band covering that point; each ladder moves the boundaries
//!   between bands by up to `jitter` (seeded, order kept). The first rung
//!   is always warm-up, the last always melee (N >= 2). The melee share is
//!   also the knife finale's (`melee_rungs_for`: `chimera:ladder`'s
//!   `melee_rungs = "auto"` on a hand-written ladder), one number for both.
//! - **The power score** (`score`), which orders guns only inside a band:
//!   how fast the gun kills a baseline player (`health` 100, no armour, so
//!   the same for every gun) at each of `distances` (10, 15, 20 m), the
//!   score the mean of `1 / (ttk + reaction) * (1 + magazine term)` over
//!   them. At distance d, with the gun's stats (`GunStats`: damage per
//!   pellet on a body hit at d after its falloff, pellets, head
//!   multiplier, fire interval, pre-fire latency, the first shot's spread
//!   and the follow-ups' (`firing` into the firing spread), clip, reload):
//!
//!   ```text
//!   p(s)   = min(1, (target_radius / (s * d))^2)        hit rate of a pellet at spread s
//!   hit    = damage * (1 + headshot_share * (head - 1))  a hit, headshots averaged in
//!   k      = ceil(health / hit)                          pellet hits to kill
//!   n1, n2 = pellets * p(first), pellets * p(follow)     pellets landing: first shot, others
//!   shots  = 1                        if n1 >= k
//!            1 + (k - n1) / n2        else (rounded up when n2 >= 1)
//!   ttk    = (shots - 1) * interval + latency
//!          + floor((ceil(shots) - 1) / clip) * reload
//!   score  = (1 + magazine_weight * min(clip, magazine_full) / magazine_full)
//!          / (ttk + reaction)
//!   ```
//!
//!   The stats come from the weapon's data (`WeaponData`: `damage` with
//!   its falloff and hitgroups, `hitscan` pellets and spread, `trigger`
//!   cycle and latency, `magazine`), then from each game's readers of its
//!   own hooks' parameters (`PowerReaders`, `PowerAppExt::
//!   add_power_reader`: CS:S's accuracy, Combat Arms' damage bands,
//!   spread cone, fire rate, projectile and flame), which know what the
//!   game's numbers mean. A gun without damage data scores 0.
//! - `ladder_power` (console): every classed gun of the catalogue, by
//!   band, weakest first: its score, time to kill at 15 m and band.
//!
//! Known shortcuts: Power curve: the score is a model, not a measurement:
//! recoil, movement, burst modes, a spin-up's length and a scope's zoomed
//! spread (but CS:S's alternate accuracy) are left out, and Combat Arms'
//! install copies score with their template's fire cycle (their own is
//! read from the view model only when the weapon is built). Why it's OK
//! for now: it orders guns inside a band, where those matter less than
//! damage, rate and spread; tune the data after a playtest.

use std::collections::BTreeMap;

use bevy::prelude::*;
use chimera_core::{console::ConsoleAppExt, game_parts::GunClass};
use serde::Deserialize;

use super::auto::Rng;
use crate::weapon::data::{SpreadData, WeaponCatalog, WeaponData};

/// The curve's data, as shipped.
pub const POWER_CURVE: &str = include_str!("../../data/power_curve.toml");

/// A band of the ladder, in climbing order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    WarmUp,
    Rising,
    Peak,
    Heavy,
    Specials,
    Grenade,
    Melee,
}

impl Band {
    pub const ALL: [Band; 7] = [
        Band::WarmUp,
        Band::Rising,
        Band::Peak,
        Band::Heavy,
        Band::Specials,
        Band::Grenade,
        Band::Melee,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Band::WarmUp => "warm_up",
            Band::Rising => "rising",
            Band::Peak => "peak",
            Band::Heavy => "heavy",
            Band::Specials => "specials",
            Band::Grenade => "grenade",
            Band::Melee => "melee",
        }
    }
}

/// Each band's share of the ladder.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shares {
    pub warm_up: f64,
    pub rising: f64,
    pub peak: f64,
    pub heavy: f64,
    pub specials: f64,
    pub grenade: f64,
    pub melee: f64,
}

impl Shares {
    /// In band order, scaled to add up to 1 (all 0: equal).
    pub fn normalized(&self) -> [f64; 7] {
        let raw = [
            self.warm_up,
            self.rising,
            self.peak,
            self.heavy,
            self.specials,
            self.grenade,
            self.melee,
        ]
        .map(|s| if s.is_finite() { s.max(0.0) } else { 0.0 });
        let sum: f64 = raw.iter().sum();
        if sum <= 0.0 {
            return [1.0 / 7.0; 7];
        }
        raw.map(|s| s / sum)
    }
}

/// The score's constants (`[score]`).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreParams {
    pub distances: Vec<f32>,
    pub health: f32,
    pub target_radius: f32,
    pub firing: f32,
    pub headshot_share: f32,
    pub default_head: f32,
    pub update: f32,
    pub reaction: f32,
    pub magazine_weight: f32,
    pub magazine_full: u32,
    pub heavy_pistol_damage: f32,
}

/// The power curve (`data/power_curve.toml`).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerCurve {
    pub shares: Shares,
    pub jitter: f64,
    pub classes: BTreeMap<GunClass, Band>,
    pub score: ScoreParams,
}

impl Default for PowerCurve {
    fn default() -> Self {
        Self::shipped()
    }
}

impl PowerCurve {
    /// The shipped curve (`POWER_CURVE`, read each time: a small table,
    /// read when a ladder is made).
    pub fn shipped() -> PowerCurve {
        toml::from_str(POWER_CURVE).expect("data/power_curve.toml reads")
    }

    /// The shipped curve with a definition's `power_curve` table over it
    /// (any of its keys; `shares` by band).
    pub fn tuned(table: &toml::Table) -> Result<PowerCurve, String> {
        let mut base: toml::Table = toml::from_str(POWER_CURVE).map_err(|e| e.to_string())?;
        merge(&mut base, table);
        toml::Value::Table(base).try_into().map_err(|e: toml::de::Error| format!("power_curve: {e}"))
    }

    /// The band a class plays in (unmapped: `Peak`).
    pub fn class_band(&self, class: GunClass) -> Band {
        self.classes.get(&class).copied().unwrap_or(Band::Peak)
    }

    /// The melee band's share (normalized).
    pub fn melee_share(&self) -> f64 {
        self.shares.normalized()[Band::Melee.index()]
    }
}

/// `over` merged into `base`, tables by key.
fn merge(base: &mut toml::Table, over: &toml::Table) {
    for (k, v) in over {
        match (base.get_mut(k), v) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge(b, o),
            _ => {
                base.insert(k.clone(), v.clone());
            }
        }
    }
}

/// How many rungs of a ladder of `n` each band gets: rung i at (i + 0.5) /
/// n takes the band whose share covers it, the boundaries moved by up to
/// the curve's `jitter` on `rng`; the first rung warm-up and the last
/// melee (n >= 2).
pub fn band_counts(n: usize, curve: &PowerCurve, rng: &mut Rng) -> [usize; 7] {
    let shares = curve.shares.normalized();
    let jitter = curve.jitter.clamp(0.0, 0.5);
    let mut bounds = [0.0f64; 7];
    let mut at = 0.0;
    for (k, s) in shares.iter().enumerate() {
        at += s;
        bounds[k] = at;
    }
    // The melee band's start exactly (`melee_rungs_for` agrees).
    bounds[5] = 1.0 - shares[6];
    bounds[6] = f64::INFINITY;
    let mut prev = 0.0;
    for b in bounds.iter_mut().take(6) {
        *b = (*b + (rng.unit() * 2.0 - 1.0) * jitter).clamp(prev, 1.0);
        prev = *b;
    }
    let mut counts = [0usize; 7];
    for i in 0..n {
        let pos = (i as f64 + 0.5) / n as f64;
        let mut band = bounds.iter().position(|b| pos < *b).unwrap_or(6);
        if i == 0 {
            band = 0;
        } else if i + 1 == n {
            band = 6;
        }
        counts[band] += 1;
    }
    counts
}

/// The melee rungs a hand-written ladder of `guns` gun rungs ends with by
/// default: the melee band's `share` of the whole ladder (guns and melee),
/// as `band_counts` gives it without jitter, at least 1 (7%: 1 for 10 or
/// 20 guns, 2 for 30, 3 for 40).
pub fn melee_rungs_for(guns: usize, share: f64) -> usize {
    let share = share.clamp(0.0, 1.0);
    // The melee band of a ladder of n: the rungs at or past 1 - share, the
    // last one at least.
    let band = |n: usize| (0..n).filter(|i| (*i as f64 + 0.5) / n as f64 >= 1.0 - share).count().max(1);
    let mut m = 1;
    while band(guns + m) > m {
        m += 1;
    }
    m
}

/// What the score reads of a gun at one distance, in health (1 = the
/// baseline player's 100), seconds and offset per unit of distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunStats {
    /// A pellet's damage on a body hit at the distance (falloff applied).
    pub damage: f32,
    pub pellets: u32,
    /// The head's multiplier over the body's (None: the curve's default).
    pub head: Option<f32>,
    /// Seconds between shots.
    pub interval: f32,
    /// Seconds from the press to the first shot.
    pub latency: f32,
    /// The first shot's spread: its offset per unit of distance (a disc's
    /// radius).
    pub spread: f32,
    /// The follow-up shots' (None: the first's).
    pub follow: Option<f32>,
    /// 0: no magazine (never reloads).
    pub clip: u32,
    pub reload: f32,
}

/// Where a reader reads: the distance in the document's length units, and
/// the curve's `firing` and `update`.
#[derive(Clone, Copy, Debug)]
pub struct ReadAt<'a> {
    pub distance: f32,
    pub data: &'a WeaponData,
    pub firing: f32,
    pub update: f32,
}

/// A hook's parameters as its weapon's data gives them.
pub type HookParams = toml::Value;

/// A game's reader of one of its hooks' parameters, refining the stats.
pub type PowerReader = fn(&HookParams, &ReadAt, &mut GunStats);

/// The games' readers, by hook name (`PowerAppExt::add_power_reader`),
/// applied in registration order.
#[derive(Resource, Clone, Default)]
pub struct PowerReaders(pub Vec<(&'static str, PowerReader)>);

pub trait PowerAppExt {
    /// `reader` refines a gun's stats when its data names hook `hook`.
    fn add_power_reader(&mut self, hook: &'static str, reader: PowerReader) -> &mut Self;
}

impl PowerAppExt for App {
    fn add_power_reader(&mut self, hook: &'static str, reader: PowerReader) -> &mut Self {
        self.world_mut().get_resource_or_init::<PowerReaders>().0.push((hook, reader));
        self
    }
}

/// A gun's stats at `metres` from its data and the readers.
pub fn stats(data: &WeaponData, readers: &PowerReaders, curve: &PowerCurve, metres: f32) -> GunStats {
    let length = data.units.length.max(1e-6);
    let distance = metres / length;
    let unit = data.units.damage;
    let mut s = GunStats {
        damage: 0.0,
        pellets: 1,
        head: None,
        interval: 1.0,
        latency: 0.0,
        spread: 0.0,
        follow: None,
        clip: 0,
        reload: 0.0,
    };
    if let Some(d) = &data.damage {
        let body = if d.hitgroups.chest > 0.0 { d.hitgroups.chest } else { 1.0 };
        s.damage = d.amount * unit * body * crate::weapon::falloff(d.falloff, d.falloff_step, distance);
        s.head = Some(d.hitgroups.head / body);
    }
    if let Some(h) = &data.hitscan {
        s.pellets = h.pellets.max(1);
        s.spread = match h.spread {
            Some(SpreadData::Template(x)) => x,
            Some(SpreadData::Disc { inaccuracy, spread }) => inaccuracy + spread,
            None => 0.0,
        };
    }
    if let Some(t) = &data.trigger {
        s.interval = t.cycle;
        s.latency = t.latency as f32 * curve.score.update;
    }
    if let Some(m) = &data.magazine {
        s.clip = m.clip;
        s.reload = m.reload;
    }
    let at = ReadAt {
        distance,
        data,
        firing: curve.score.firing,
        update: curve.score.update,
    };
    for (hook, read) in &readers.0 {
        if let Some(p) = data.hooks.get(*hook) {
            read(p, &at, &mut s);
        }
    }
    s
}

/// The time to kill the baseline player with `s` at `metres`, seconds
/// (infinite: it can't).
pub fn time_to_kill(s: &GunStats, p: &ScoreParams, metres: f32) -> f32 {
    let health = p.health * 0.01;
    let rate = |spread: f32| {
        let reach = spread * metres;
        if reach <= p.target_radius { 1.0 } else { (p.target_radius / reach).powi(2) }
    };
    let head = s.head.unwrap_or(p.default_head);
    let hit = s.damage * (1.0 + p.headshot_share * (head - 1.0));
    if hit <= 0.0 || !hit.is_finite() {
        return f32::INFINITY;
    }
    let k = (health / hit - 1e-4).ceil().max(1.0);
    let first = s.pellets as f32 * rate(s.spread);
    let next = s.pellets as f32 * rate(s.follow.unwrap_or(s.spread));
    let shots = if first >= k {
        1.0
    } else {
        let rest = (k - first) / next;
        1.0 + if next >= 1.0 { (rest - 1e-4).ceil() } else { rest }
    };
    let reloads = if s.clip > 0 { ((shots.ceil() - 1.0) / s.clip as f32).floor() } else { 0.0 };
    (shots - 1.0).max(0.0) * s.interval + s.latency + reloads * s.reload
}

/// A gun's power: its band, score and time to kill at the middle distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Power {
    pub band: Band,
    pub score: f32,
    pub ttk: f32,
}

/// `data`'s score (the mean over the curve's distances) and time to kill
/// at the middle one.
pub fn score(data: &WeaponData, readers: &PowerReaders, curve: &PowerCurve) -> (f32, f32) {
    let p = &curve.score;
    if p.distances.is_empty() {
        return (0.0, f32::INFINITY);
    }
    let clip = data.magazine.as_ref().map_or(0, |m| m.clip);
    let full = p.magazine_full.max(1);
    let mag = if clip == 0 { 1.0 } else { clip.min(full) as f32 / full as f32 };
    let bonus = 1.0 + p.magazine_weight * if clip == 1 { 0.0 } else { mag };
    let mut sum = 0.0;
    let mut middle = f32::INFINITY;
    for (i, d) in p.distances.iter().enumerate() {
        let ttk = time_to_kill(&stats(data, readers, curve, *d), p, *d);
        if i == p.distances.len() / 2 {
            middle = ttk;
        }
        sum += if ttk.is_finite() { bonus / (ttk + p.reaction.max(1e-3)) } else { 0.0 };
    }
    (sum / p.distances.len() as f32, middle)
}

/// The mean body-hit damage of `data` over the curve's distances, in hit
/// points.
fn body_damage(data: &WeaponData, readers: &PowerReaders, curve: &PowerCurve) -> f32 {
    let d = &curve.score.distances;
    if d.is_empty() {
        return 0.0;
    }
    d.iter().map(|m| stats(data, readers, curve, *m).damage).sum::<f32>() / d.len() as f32 * 100.0
}

/// A gun's band: its class's, a hard-hitting pistol a heavy pistol's.
pub fn band_of(class: GunClass, data: Option<&WeaponData>, readers: &PowerReaders, curve: &PowerCurve) -> Band {
    if class == GunClass::Pistol
        && let Some(d) = data
        && body_damage(d, readers, curve) >= curve.score.heavy_pistol_damage
    {
        return curve.class_band(GunClass::HeavyPistol);
    }
    curve.class_band(class)
}

/// Each gun's power, by id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Powers(pub BTreeMap<&'static str, Power>);

impl Powers {
    /// The powers of `guns` (id and class) from the catalogue's data
    /// (a gun not in it: its class's band, score 0).
    pub fn of(
        guns: impl IntoIterator<Item = (&'static str, GunClass)>,
        catalog: &WeaponCatalog,
        readers: &PowerReaders,
        curve: &PowerCurve,
    ) -> Self {
        let mut out = BTreeMap::new();
        for (id, class) in guns {
            let data = catalog.get(id);
            let (score, ttk) = data.map_or((0.0, f32::INFINITY), |d| score(d, readers, curve));
            out.insert(
                id,
                Power {
                    band: band_of(class, data, readers, curve),
                    score,
                    ttk,
                },
            );
        }
        Self(out)
    }

    pub fn get(&self, id: &str) -> Option<&Power> {
        self.0.get(id)
    }
}

/// Every classed weapon of the world's catalogue (its data's `class`, or
/// a game's claim) with its power, by band then score.
pub fn table(world: &World, curve: &PowerCurve) -> Vec<(&'static str, GunClass, Power)> {
    let empty = WeaponCatalog::default();
    let catalog = world.get_resource::<WeaponCatalog>().unwrap_or(&empty);
    let readers = world.get_resource::<PowerReaders>().cloned().unwrap_or_default();
    let mut guns: Vec<(&'static str, GunClass)> = Vec::new();
    for c in super::auto::report(world) {
        for g in c.guns.iter().copied().chain(c.grenades.iter().map(|id| chimera_core::game_parts::GunPart {
            id,
            class: GunClass::Grenade,
        })) {
            if !guns.iter().any(|(id, _)| *id == g.id) {
                guns.push((g.id, g.class));
            }
        }
    }
    for set in &catalog.sets {
        for w in &set.weapons {
            if let Some(d) = catalog.get(&w.id)
                && let Some(class) = d.class
                && !guns.iter().any(|(id, _)| *id == d.id.as_str())
            {
                guns.push((&d.id, class));
            }
        }
    }
    let powers = Powers::of(guns.iter().copied(), catalog, &readers, curve);
    let mut out: Vec<(&'static str, GunClass, Power)> =
        guns.into_iter().filter_map(|(id, c)| powers.get(id).map(|p| (id, c, *p))).collect();
    out.sort_by(|a, b| a.2.band.cmp(&b.2.band).then(a.2.score.total_cmp(&b.2.score)).then(a.0.cmp(b.0)));
    out
}

/// The `ladder_power` console command.
pub fn plugin(app: &mut App) {
    app.init_resource::<PowerReaders>().console_command(
        "chimera:ladder_power",
        "ladder_power [filter]: every classed gun's power score (higher: stronger), time to kill at the middle \
         distance and band, by band then score (data/power_curve.toml); a filter keeps ids containing it.",
        |w, a| {
            let filter = a.first().map(|f| f.to_ascii_lowercase());
            let curve = &PowerCurve::shipped();
            let rows: Vec<String> = table(w, curve)
                .into_iter()
                .filter(|(id, ..)| filter.as_ref().is_none_or(|f| id.to_ascii_lowercase().contains(f)))
                .map(|(id, class, p)| {
                    format!("{:<9} {:>6.3}  ttk {:>5.2} s  {} ({})", p.band.name(), p.score, p.ttk, id, class.name())
                })
                .collect();
            Ok(Some(if rows.is_empty() { "no classed guns".into() } else { rows.join("\n") }))
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve() -> PowerCurve {
        PowerCurve::shipped()
    }

    #[test]
    fn the_shipped_curve_reads_and_adds_up() {
        let c = curve();
        let s = c.shares.normalized();
        assert!((s.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!((c.melee_share() - 0.07).abs() < 1e-9);
        assert_eq!(c.class_band(GunClass::Pistol), Band::WarmUp);
        assert_eq!(c.class_band(GunClass::Launcher), Band::Specials);
        assert_eq!(c.class_band(GunClass::HeavyPistol), Band::Heavy);
        // Every class has a band.
        for k in GunClass::ALL.iter().chain([&GunClass::Grenade]) {
            assert!(c.classes.contains_key(k), "{k:?}");
        }
        let t: toml::Table = toml::from_str("shares = { peak = 0.5 }\njitter = 0.0").unwrap();
        let tuned = PowerCurve::tuned(&t).unwrap();
        assert_eq!(tuned.jitter, 0.0);
        assert_eq!(tuned.shares.peak, 0.5);
        assert_eq!(tuned.shares.warm_up, 0.15);
        assert!(PowerCurve::tuned(&toml::from_str("bogus = 1").unwrap()).is_err());
    }

    #[test]
    fn shares_give_the_expected_counts_within_the_jitter() {
        let c = curve();
        let mut flat = c.clone();
        flat.jitter = 0.0;
        // No jitter: the shares' own counts (a rung on a boundary takes
        // the band above it: 30 rungs' fifth sits at 0.15, rising).
        assert_eq!(band_counts(10, &flat, &mut Rng::new(1)), [1, 2, 3, 2, 1, 0, 1]);
        assert_eq!(band_counts(20, &flat, &mut Rng::new(1)), [3, 4, 6, 3, 1, 2, 1]);
        assert_eq!(band_counts(30, &flat, &mut Rng::new(1)), [4, 6, 9, 4, 3, 2, 2]);
        for n in [10usize, 20, 30] {
            let exact = band_counts(n, &flat, &mut Rng::new(0));
            let mut seen = std::collections::BTreeSet::new();
            for seed in 0..500 {
                let k = band_counts(n, &c, &mut Rng::new(seed));
                assert_eq!(k.iter().sum::<usize>(), n);
                assert!(k[0] >= 1 && k[6] >= 1, "{n}: a warm-up and a melee rung: {k:?}");
                // A boundary moves at most 3% of n (and a rung's
                // half-step), so a band at most two rungs either way.
                let slack = (2.0 * 0.03 * n as f64).ceil() as usize + 1;
                for b in 0..7 {
                    assert!(k[b].abs_diff(exact[b]) <= slack, "{n}: band {b} {k:?} vs {exact:?}");
                }
                seen.insert(k);
            }
            assert!(seen.len() > 1, "{n}: the jitter moves the boundaries");
        }
    }

    #[test]
    fn melee_rungs_for_match_the_share() {
        assert_eq!(melee_rungs_for(10, 0.07), 1);
        assert_eq!(melee_rungs_for(12, 0.07), 1);
        assert_eq!(melee_rungs_for(20, 0.07), 1);
        assert_eq!(melee_rungs_for(28, 0.07), 2);
        assert_eq!(melee_rungs_for(40, 0.07), 3);
        assert_eq!(melee_rungs_for(5, 0.0), 1);
        // The same as the band counts of the whole ladder, without jitter.
        let mut flat = curve();
        flat.jitter = 0.0;
        for guns in 2..60 {
            let m = melee_rungs_for(guns, 0.07);
            assert_eq!(band_counts(guns + m, &flat, &mut Rng::new(0))[6], m, "{guns} guns");
        }
    }

    fn stats_of(damage: f32, pellets: u32, interval: f32, spread: f32, clip: u32) -> GunStats {
        GunStats {
            damage,
            pellets,
            head: Some(1.0),
            interval,
            latency: 0.0,
            spread,
            follow: None,
            clip,
            reload: 2.0,
        }
    }

    #[test]
    fn time_to_kill_counts_shots_misses_and_reloads() {
        let p = curve().score;
        // 4 hits of 0.3, 0.1 s apart: 0.3 s.
        assert!((time_to_kill(&stats_of(0.3, 1, 0.1, 0.0, 30), &p, 15.0) - 0.3).abs() < 1e-5);
        // One shot kills: 0.
        assert_eq!(time_to_kill(&stats_of(1.2, 1, 1.5, 0.0, 5), &p, 15.0), 0.0);
        // Half the shots land (spread twice the target's radius... squared:
        // a quarter): four hits take 16 shots.
        let wide = 2.0 * p.target_radius / 15.0;
        assert!((time_to_kill(&stats_of(0.3, 1, 0.1, wide, 100), &p, 15.0) - 1.5).abs() < 1e-4);
        // A clip of 2: one reload in 4 shots.
        assert!((time_to_kill(&stats_of(0.3, 1, 0.1, 0.0, 2), &p, 15.0) - 2.3).abs() < 1e-5);
        // Pellets: 8 of 0.1, all landing: 2 shots.
        assert!((time_to_kill(&stats_of(0.1, 8, 1.0, 0.0, 8), &p, 15.0) - 1.0).abs() < 1e-5);
        assert!(time_to_kill(&stats_of(0.0, 1, 0.1, 0.0, 30), &p, 15.0).is_infinite());
    }
}
