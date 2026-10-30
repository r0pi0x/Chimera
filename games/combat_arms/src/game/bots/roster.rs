//! Adding and removing bots ("Does the game have bots", "Choosing the
//! bots", 10): the bot's profile and item set by the roster's dice, its
//! team (the one opposite the human), its weapons (`weapon::OwnKit`, so
//! every respawn gives them again) with the gun type's ammunition
//! multiplier, and the quota the server keeps (`MaxPlayer`).

use bevy::prelude::*;

use super::{
    BotSettings, CaBot,
    data::{BotData, Level, Template},
    math::{self, Dice},
};
use crate::{
    character::spawn_character,
    core::{Connecting, Health, Intent, Seed, Spectating, Team},
    weapon::{Inventory, Magazine, OwnKit, Weapon, grenade::Throwable},
};

/// The two teams (game_modes.md: 1 JSF "Alpha", 2 Creed "Bravo").
pub const ALPHA: u8 = 1;
pub const BRAVO: u8 = 2;

/// The roster's state: profiles used per level, the draw counter.
#[derive(Resource, Clone, Debug, Default)]
pub struct Roster {
    pub used: [Vec<usize>; 4],
    pub draws: u64,
}

/// A bot's kit: the weapon slots 0-5 (mid primary, long primary,
/// secondary, melee, grenade, mine).
pub type Kit = [Option<&'static str>; 6];

/// The weapon ids in `kit` that exist (`&'static` as the registry's).
fn static_id(world: &World, id: &str) -> Option<&'static str> {
    world.resource::<crate::weapon::WeaponRegistry>().find(id).map(|d| d.id)
}

/// The team a new bot joins: the one opposite the first human on a team
/// (10: all bots opposite the human); with no human, the smaller team
/// (Alpha on a tie).
pub fn auto_team(world: &mut World) -> u8 {
    let mut humans: Vec<(u64, u8)> = Vec::new();
    let mut counts = [0usize; 2];
    let mut q = world.query_filtered::<(Entity, &Team, Has<CaBot>), (
        With<Intent>,
        With<Health>,
        Without<Spectating>,
        Without<crate::core::Npc>,
    )>();
    for (e, t, bot) in q.iter(world) {
        if t.0 != ALPHA && t.0 != BRAVO {
            continue;
        }
        counts[(t.0 - 1) as usize] += 1;
        if !bot {
            humans.push((e.to_bits(), t.0));
        }
    }
    if let Some(&(_, t)) = humans.iter().min() {
        return if t == ALPHA { BRAVO } else { ALPHA };
    }
    if counts[1] < counts[0] { BRAVO } else { ALPHA }
}

/// Add one bot on `team` (None: `auto_team`) at the level `BotLevel`
/// says: its profile and item set by the dice, spawned as the mode
/// spawns a player who comes in (`rules::flow::enter_game`).
pub fn add_bot(world: &mut World, team: Option<u8>) -> Option<Entity> {
    let team = team.unwrap_or_else(|| auto_team(world));
    let settings = world.resource::<BotSettings>().clone();
    let level = Level::of(settings.level);
    let data = world.resource::<BotData>().clone();
    let count = data.profiles.count(level);
    // Step 1: the draw (ours: a counter through the dice, not the clock).
    let rho = {
        let mut roster = world.resource_mut::<Roster>();
        roster.draws += 1;
        Dice::new(settings.seed ^ roster.draws.wrapping_mul(0x5851_F42D)).next_u64() >> 16
    };
    let profile = {
        let mut roster = world.resource_mut::<Roster>();
        math::pick_profile(rho, count, &mut roster.used[level.index()])
    };
    let sets = data.sets(level);
    let set = sets.get((Dice::new(rho).next_u64() % sets.len().max(1) as u64) as usize).copied();
    let slots = set.map(|s| math::fill_set(s, rho)).unwrap_or_default();
    let mut kit: Kit = [None; 6];
    for (k, item) in slots[4..].iter().enumerate() {
        kit[k] = item.as_deref().and_then(|id| static_id(world, id));
    }
    let template = Template::new(level, (profile % 3) as u8);
    let name = format!("AI_{}_{}", if team == ALPHA { "Alpha" } else { "Bravo" }, profile);
    let movement = super::super::movement::ID;
    let e = {
        let mut commands = world.commands();
        let e = spawn_character(&mut commands, Transform::from_xyz(0.0, -1000.0, 0.0), Team(team), movement);
        e
    };
    world.flush();
    let bot = CaBot::new(profile, template, rho, kit, &data);
    world.entity_mut(e).insert((
        Name::new(name),
        Seed(rho),
        OwnKit(kit.iter().flatten().copied().collect()),
        super::super::weapons::ExactAim,
        // The bot's knife reaches 130 u (8.4), the player's 105.
        super::super::inventory::KnifeReach(super::brain::KNIFE_RANGE * super::super::inventory::UNIT),
        bot,
    ));
    // Into the game as a player coming in: the mode's spawn, or a free
    // spawn point without a mode.
    world.entity_mut(e).insert(Connecting);
    super::super::rules::flow::enter_game(world, e);
    Some(e)
}

/// Remove bot `e`.
pub fn kick(world: &mut World, e: Entity) {
    let weapons = world.get::<Inventory>(e).map(|i| i.weapons.clone()).unwrap_or_default();
    for w in weapons {
        if let Ok(w) = world.get_entity_mut(w) {
            w.despawn();
        }
    }
    if let Ok(b) = world.get_entity_mut(e) {
        b.despawn();
    }
}

/// The bots' weapons as they come (each life's kit, `OwnKit`): the gun
/// type's ammunition multiplier on the reserve (8.1: the ammo's normal
/// maximum times it, the clip included), the ranged and thrown reach
/// fixed at the first life (8.2, 8.5).
/// A normal system with declared queries: it writes the magazines and the
/// bot in place, so the rest of the tick sees them as the exclusive
/// version's writes (plan ecs-cleanup step 5); a bot is visited only
/// while its new life's kit is not set up yet.
#[allow(clippy::type_complexity)]
pub(super) fn arm(
    data: Res<BotData>,
    mut bots: Query<(&mut CaBot, &Inventory), Without<Connecting>>,
    mut weapons: Query<(&Weapon, Option<&mut Magazine>, Option<&mut Throwable>)>,
) {
    for (mut b, inv) in &mut bots {
        if b.armed || inv.weapons.is_empty() {
            continue;
        }
        let template = b.template;
        let (mut ranged, mut thrown_kinds) = (0.0f32, Vec::new());
        for &w in &inv.weapons {
            let Ok((weapon, magazine, throwable)) = weapons.get_mut(w) else { continue };
            let id = weapon.id;
            let Some(gun) = data.gun(id, template) else { continue };
            let kind = data.gun_type(id).unwrap_or_default().to_string();
            if let Some(mut m) = magazine {
                let total = (m.reserve_max as f32 * gun.ammo).round() as u32;
                m.reserve = total.saturating_sub(m.clip);
                if (m.reserve > 0 || m.clip > 0) && kind != "Knife" {
                    ranged = ranged.max(gun.range.1);
                }
            }
            if let Some(mut t) = throwable {
                t.count *= gun.ammo.max(1.0) as u32;
                if t.count > 0 {
                    thrown_kinds.push(kind.clone());
                }
            }
            if kind == "Mine" {
                thrown_kinds.push(kind);
            }
        }
        let kinds: Vec<&str> = thrown_kinds.iter().map(String::as_str).collect();
        let thrown = thrown_reach(&data, template, &kinds);
        b.armed = true;
        if !b.reach_fixed {
            b.ranged_reach = ranged;
            b.thrown_reach = thrown;
            b.reach_fixed = true;
        }
    }
}

/// The thrown reach (8.5, u): the largest range of the thrown weapons
/// carried (gun types `kinds`), a mine counting with the default gun
/// type's range (Quirk 11).
pub fn thrown_reach(data: &BotData, template: Template, kinds: &[&str]) -> f32 {
    kinds
        .iter()
        .filter_map(|k| {
            let row = if *k == "Mine" { "Default" } else { k };
            data.guns.get(row).map(|g| g.at(template).range.1)
        })
        .fold(0.0, f32::max)
}

/// The server keeps `MaxPlayer` bots (none by default): adds them one a
/// tick, kicks the newest when there are too many.
pub(super) fn keep_quota(world: &mut World) {
    let settings = world.resource::<BotSettings>();
    if !settings.quota_set && settings.quota == 0 {
        return;
    }
    let want = settings.quota as usize;
    let mut bots: Vec<(u64, Entity)> =
        chimera_core::query_cache::cached_query::<Entity, (With<CaBot>, Without<crate::core::Npc>), _>(world, |q, world| {
            q.iter(world).map(|e| (e.to_bits(), e)).collect()
        });
    if bots.len() < want {
        add_bot(world, None);
    } else if bots.len() > want {
        bots.sort();
        if let Some(&(_, e)) = bots.last() {
            kick(world, e);
        }
    }
}
