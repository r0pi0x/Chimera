//! The announcer's counting, on the server (specs/combat_arms/
//! kill_streaks.md A1; the vocabulary is `chimera_core::announcer`): from
//! every death by an enemy's hand, of any gun on any body, the killer's
//! multi-kill count (enemy kills each less than the pack's window after
//! the previous), the first kill of the round, the killing hit's special
//! place (head, groin: the gun's or the knife's `core::RegionHit`, else the damage's
//! hitgroup; never for blasts or burns), revenge and the killer's body (`core::FemaleBodies`),
//! written as `KillAnnounced` (`AnnounceSet`, before the network sends
//! the kill with them: `net::Killed::facts`).
//!
//! The pack is the server's setting (`sv_announcer`, replicated:
//! `AnnouncerSetting`), resolved into `ActiveAnnouncer` when it changes
//! and on each map (`random` picks by the map's name). Without a pack
//! nothing is counted or announced.
//!
//! - Teammates' kills and suicides neither count nor reset (A1).
//! - The killer's kill total (`KillFacts::total`) runs for the map.
//! - The first kill and the counts start again with each round
//!   (`core::RoundRestarts`) and map.
//!
//! Known shortcuts: the kill total (`KillFacts::total`) is this counter's
//! enemy kills per killer for the map, not a scoreboard entry (no
//! training-round or Bounty Hunter exceptions); whether the killer's own death resets the count is
//! open (kill_streaks.md Open question 2: the spec found nothing and
//! gives no reason to think it does, so it doesn't here; tested); a
//! knife's groin kill counts as a nutshot though the spec names no
//! knife callout (its hit location is the killing hit's, melee not among
//! the damage that can't headshot); Combat Arms' female skeleton data has
//! no groin sphere, so a woman's body can't be hit there (the female
//! groin voice is only reached by another game's body; Open question
//! 10 stays open); first kill per
//! round, not per match (Open question 3); no lucky shot (its rate comes
//! from a lobby: Open question 6); revenge after the pack's
//! `revenge_after` unanswered kills (the spec gives no number).

use std::collections::BTreeMap;

use bevy::prelude::*;
use chimera_core::announcer::{
    ActiveAnnouncer, AnnounceSet, AnnouncerPacks, AnnouncerSetting, KillAnnounced, KillFacts, SpecialShot,
};

use crate::{
    core::{BodyRegion, DamageKind, Died, FemaleBodies, RegionHit, RoundRestarts, Team},
    map::LoadedMapName,
    weapon::{Inventory, Weapon},
};

/// The counting and `sv_announcer`.
pub struct AnnouncePlugin;

impl Plugin for AnnouncePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnnouncerPacks>()
            .init_resource::<AnnouncerSetting>()
            .init_resource::<ActiveAnnouncer>()
            .init_resource::<KillCounter<Entity>>()
            .init_resource::<FemaleBodies>()
            .add_message::<Died>()
            .add_message::<RegionHit>()
            .add_message::<KillAnnounced>()
            .add_systems(
                PostUpdate,
                (choose_pack, count_kills)
                    .chain()
                    .in_set(AnnounceSet)
                    .run_if(crate::core::authoritative),
            );
        crate::console::resource_cvar::<AnnouncerSetting, String>(
            app,
            "chimera:announcer",
            "The kill announcer: default (the game's), none, random (one per map) or a pack's id.",
            |s| &mut s.0,
        );
    }
}

/// Per player, the time of the last enemy kill and the multi-kill count;
/// whether the round's first kill was made; per (killer, victim), kills
/// since the victim last killed the killer.
#[derive(Resource, Clone, Debug)]
pub struct KillCounter<K: Ord + Copy + Send + Sync + 'static> {
    streaks: BTreeMap<K, (f64, u8)>,
    first_done: bool,
    totals: BTreeMap<K, u32>,
    unanswered: BTreeMap<(K, K), u32>,
}

impl<K: Ord + Copy + Send + Sync + 'static> Default for KillCounter<K> {
    fn default() -> Self {
        Self {
            streaks: BTreeMap::new(),
            first_done: false,
            totals: BTreeMap::new(),
            unanswered: BTreeMap::new(),
        }
    }
}

/// What the counter says of an enemy kill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counted {
    pub multi: u8,
    pub first: bool,
    pub revenge: bool,
    /// The killer's enemy kills since the map began, this one included
    /// (rounds don't reset it: the plain-kill banner's number).
    pub total: u32,
}

impl<K: Ord + Copy + Send + Sync + 'static> KillCounter<K> {
    /// An enemy kill at `now` (seconds): the killer's count (raised when
    /// strictly less than `window_ms` after its previous enemy kill, to
    /// the millisecond, else 1), the first kill, revenge.
    pub fn enemy_kill(&mut self, killer: K, victim: K, now: f64, window_ms: u32, revenge_after: u32) -> Counted {
        let streak = self.streaks.entry(killer).or_insert((f64::NEG_INFINITY, 0));
        let within = ((now - streak.0) * 1000.0).round() < window_ms as f64;
        streak.1 = if within { streak.1.saturating_add(1) } else { 1 };
        streak.0 = now;
        let multi = streak.1;
        let first = !self.first_done;
        self.first_done = true;
        let owed = self.unanswered.remove(&(victim, killer)).unwrap_or(0);
        let revenge = revenge_after > 0 && owed >= revenge_after;
        *self.unanswered.entry((killer, victim)).or_insert(0) += 1;
        let total = self.totals.entry(killer).or_insert(0);
        *total += 1;
        Counted { multi, first, revenge, total: *total }
    }

    /// A new round: the first kill again, counts from nothing.
    pub fn new_round(&mut self) {
        self.first_done = false;
        self.streaks.clear();
    }

    /// A new map: all of it.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// What the pack was chosen from last.
#[derive(Default)]
struct Chosen {
    setting: Option<String>,
    map: Option<String>,
    packs: usize,
    restarts: u32,
}

/// `ActiveAnnouncer` from the setting, again when it, the packs or the map
/// change; the counter starts again with each map and round.
fn choose_pack(
    setting: Res<AnnouncerSetting>,
    packs: Res<AnnouncerPacks>,
    map: Option<Res<LoadedMapName>>,
    restarts: Option<Res<RoundRestarts>>,
    mut active: ResMut<ActiveAnnouncer>,
    mut counter: ResMut<KillCounter<Entity>>,
    mut chosen: Local<Chosen>,
) {
    let map_name = map.map(|m| m.0.clone());
    let restarts = restarts.map_or(0, |r| r.0);
    if chosen.restarts != restarts {
        chosen.restarts = restarts;
        counter.new_round();
    }
    if chosen.setting.as_deref() == Some(setting.0.as_str()) && chosen.map == map_name && chosen.packs == packs.0.len() {
        return;
    }
    if chosen.map != map_name {
        counter.reset();
    }
    // The map's name picks a random pack (FNV-1a).
    let seed = map_name
        .as_deref()
        .unwrap_or("")
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    active.0 = setting.resolve(&packs, seed);
    *chosen = Chosen {
        setting: Some(setting.0.clone()),
        map: map_name,
        packs: packs.0.len(),
        restarts,
    };
}

/// The special place of a killing hit: the gun's full region when it is
/// the damage's (same hitgroup), else the hitgroup's; none for damage that
/// can't headshot.
pub fn special_shot(kind: DamageKind, hitgroup_region: BodyRegion, gun_region: Option<BodyRegion>) -> SpecialShot {
    if matches!(kind, DamageKind::Blast | DamageKind::Burn) {
        return SpecialShot::None;
    }
    let region = gun_region
        .filter(|r| r.hitgroup() == hitgroup_region.hitgroup())
        .unwrap_or(hitgroup_region);
    match region {
        BodyRegion::Head => SpecialShot::Head,
        BodyRegion::Groin => SpecialShot::Groin,
        _ => SpecialShot::None,
    }
}

/// Each enemy kill's facts (`KillAnnounced`).
#[allow(clippy::too_many_arguments)]
fn count_kills(
    mut died: MessageReader<Died>,
    mut regions: MessageReader<RegionHit>,
    active: Res<ActiveAnnouncer>,
    clock: Res<crate::core::SimClock>,
    teams: Query<&Team>,
    female: Option<Res<FemaleBodies>>,
    inventories: Query<&Inventory>,
    weapons: Query<&Weapon>,
    mut counter: ResMut<KillCounter<Entity>>,
    mut last_region: Local<Vec<(Entity, BodyRegion)>>,
    mut out: MessageWriter<KillAnnounced>,
) {
    for r in regions.read() {
        match last_region.iter_mut().find(|(e, _)| *e == r.target) {
            Some(slot) => slot.1 = r.region,
            None => last_region.push((r.target, r.region)),
        }
    }
    for d in died.read() {
        let gun_region = last_region.iter().position(|(e, _)| *e == d.entity).map(|i| last_region.swap_remove(i).1);
        let Some(pack) = active.0.as_ref() else { continue };
        let Some(killer) = d.attacker.filter(|k| *k != d.entity) else { continue };
        let teammates = matches!((teams.get(killer), teams.get(d.entity)), (Ok(a), Ok(b)) if a == b);
        if teammates {
            continue;
        }
        let counted = counter.enemy_kill(killer, d.entity, clock.now, pack.window_ms, pack.revenge_after);
        let weapon = d.damage.weapon.or_else(|| {
            let held = inventories.get(killer).ok()?.active?;
            weapons.get(held).ok().map(|w| w.id)
        });
        out.write(KillAnnounced {
            victim: d.entity,
            killer,
            weapon,
            facts: KillFacts {
                pack: pack.id.clone(),
                multi: counted.multi,
                first: counted.first,
                shot: special_shot(d.damage.kind, BodyRegion::from_hitgroup(d.damage.hitgroup), gun_region),
                revenge: counted.revenge,
                total: counted.total,
                killer_female: female.as_ref().is_some_and(|f| f.0.contains(&killer)),
            },
        });
    }
    // Hits that didn't kill: only the latest per body matters, and only
    // until the next one.
    if last_region.len() > 256 {
        last_region.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u32 = 5000;

    fn kill(c: &mut KillCounter<u32>, at: f64) -> Counted {
        c.enemy_kill(1, 2, at, W, 2)
    }

    /// Cases 1-6: counts 1 to 8 a second apart; 5.000 s restarts, 4.999
    /// s doesn't.
    #[test]
    fn counts_within_the_window() {
        let mut c = KillCounter::default();
        let counts: Vec<u8> = (0..8).map(|i| kill(&mut c, i as f64).multi).collect();
        assert_eq!(counts, [1, 2, 3, 4, 5, 6, 7, 8]);
        let mut c = KillCounter::default();
        kill(&mut c, 0.0);
        assert_eq!(kill(&mut c, 5.0).multi, 1, "exactly the window restarts");
        let mut c = KillCounter::default();
        kill(&mut c, 0.0);
        assert_eq!(kill(&mut c, 4.999).multi, 2);
    }

    /// kill_streaks.md B2a: the total counts the killer's enemy kills for
    /// the map, through rounds and out of the window.
    #[test]
    fn the_total_runs_for_the_map() {
        let mut c = KillCounter::default();
        assert_eq!(kill(&mut c, 0.0).total, 1);
        assert_eq!(kill(&mut c, 20.0).total, 2, "outside the window: count 1, total 2");
        c.new_round();
        assert_eq!(kill(&mut c, 40.0).total, 3, "a round doesn't reset it");
        assert_eq!(c.enemy_kill(3, 2, 41.0, W, 2).total, 1, "per killer");
        c.reset();
        assert_eq!(kill(&mut c, 50.0).total, 1, "a map does");
    }

    /// Case 7: the first enemy kill of the round is the first kill; a new
    /// round has one again.
    #[test]
    fn first_kill_once_a_round() {
        let mut c = KillCounter::default();
        assert!(kill(&mut c, 0.0).first);
        assert!(!c.enemy_kill(3, 2, 10.0, W, 2).first);
        c.new_round();
        assert!(c.enemy_kill(3, 2, 20.0, W, 2).first);
    }

    /// Killed twice by the same enemy unanswered: killing them is revenge,
    /// once.
    #[test]
    fn revenge_after_unanswered_kills() {
        let mut c = KillCounter::default();
        c.enemy_kill(2, 1, 0.0, W, 2);
        c.enemy_kill(2, 1, 10.0, W, 2);
        assert!(c.enemy_kill(1, 2, 20.0, W, 2).revenge);
        assert!(!c.enemy_kill(1, 2, 30.0, W, 2).revenge, "answered");
        c.enemy_kill(2, 1, 40.0, W, 2);
        assert!(!c.enemy_kill(1, 2, 50.0, W, 2).revenge, "one isn't enough");
    }

    /// Open question 2: the killer's own death doesn't reset the count (a
    /// death is no kill: the counter hears of enemy kills only).
    #[test]
    fn the_killers_death_leaves_the_count() {
        let mut c = KillCounter::default();
        kill(&mut c, 0.0);
        // Killed at 1 s (by 2, a kill of 2's), killing again at 3 s.
        c.enemy_kill(2, 1, 1.0, W, 2);
        assert_eq!(kill(&mut c, 3.0).multi, 2);
    }

    /// Head and groin from the gun's region; a groin shot's damage reads
    /// as the stomach; blasts never.
    #[test]
    fn special_shots() {
        use BodyRegion as R;
        assert_eq!(special_shot(DamageKind::Bullet, R::Head, None), SpecialShot::Head);
        assert_eq!(special_shot(DamageKind::Bullet, R::Stomach, Some(R::Groin)), SpecialShot::Groin);
        assert_eq!(special_shot(DamageKind::Bullet, R::Stomach, None), SpecialShot::None);
        assert_eq!(special_shot(DamageKind::Bullet, R::Chest, Some(R::Groin)), SpecialShot::None, "another hit's");
        assert_eq!(special_shot(DamageKind::Blast, R::Head, Some(R::Head)), SpecialShot::None);
        assert_eq!(special_shot(DamageKind::Melee, R::Head, None), SpecialShot::Head);
    }
}
