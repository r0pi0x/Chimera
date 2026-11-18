use std::sync::Arc;

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    character::CAPSULE_HEIGHT,
    character::spawn_character,
    console::{Command, Console, resource_cvar},
    core::{Health, Intent, MapBrush, MapBrushes, MovementState, SimSet, SpawnPoint, Team},
    map::nav::{NavLadder, NavMesh, STEP_HEIGHT, flags},
    weapon::{Inventory, grenade::Projectile},
};

use super::*;

impl Bot {
    /// The route being walked (engine space) and the next point's index.
    pub fn route(&self) -> (&[Vec3], usize) {
        (&self.route, self.next)
    }

    /// Where the bot is going (feet), whatever the reason.
    pub fn goal(&self) -> Option<Vec3> {
        self.goal
    }

    /// The point it looks at while no enemy is in sight.
    pub fn look_target(&self) -> Option<Vec3> {
        self.look_at
    }

    /// The hiding spot it is checking now.
    pub fn checking(&self) -> Option<Vec3> {
        self.corners.current()
    }

    pub fn role(&self) -> Role {
        self.orders.role
    }

    /// The site it heads for or holds (index into `Tactics::sites`).
    pub fn site(&self) -> Option<usize> {
        self.site
    }

    pub fn hold(&self) -> Option<&Hold> {
        self.hold.as_ref()
    }

    pub fn activity(&self) -> Activity {
        self.activity
    }

    /// Who it is answering, if a teammate called.
    pub fn assisting(&self) -> Option<Vec3> {
        self.assist.map(|a| a.0)
    }

    /// Send it somewhere (feet), before anything but a fight; None lets
    /// it go back to its orders. The route is worked out afresh.
    pub fn send_to(&mut self, at: Option<Vec3>) {
        self.move_to = at;
        self.route.clear();
        self.route_areas.clear();
        self.next = 0;
        self.repath = 0.0;
    }

    /// Route left to walk, m.
    pub fn route_left(&self) -> f32 {
        let rest = self.route.get(self.next..).unwrap_or(&[]);
        rest.windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    /// Start a life (a round, a respawn, new orders): forget the last
    /// one and take up the orders.
    pub(super) fn new_life(&mut self) {
        self.route.clear();
        self.route_areas.clear();
        self.next = 0;
        self.lead = None;
        self.lead_who = None;
        self.assist = None;
        self.order = None;
        self.urgent_until = 0.0;
        self.roam = None;
        self.target = None;
        self.toss = None;
        self.plan = None;
        self.avert = None;
        self.guard = None;
        self.guard_at = false;
        self.guard_skip = 0;
        self.site = self.orders.site;
        self.hold = self.orders.hold.clone();
        self.hold_since = None;
        self.hunting = false;
        self.corners = default();
        self.area = None;
        self.prev_area = None;
        self.stuck_count = 0;
        self.stuck_links.clear();
        let (lo, hi) = match self.orders.role {
            Role::Attack => ATTACK_PATIENCE,
            _ => DEFEND_PATIENCE,
        };
        self.patience = lo + (hi - lo) * self.rand() as f64;
    }

    /// The grenade throw planned or carried out lately.
    pub fn grenade_plan(&self) -> Option<&GrenadePlan> {
        self.plan.as_ref().map(|(p, _)| p)
    }

    /// Flashes held back so far since they would have blinded a teammate.
    pub fn flashes_held(&self) -> u32 {
        self.flashes_held
    }

    /// Throwing a grenade now.
    pub fn throwing(&self) -> bool {
        self.toss.is_some()
    }

    /// Looking away from a flash about to go off: from where.
    pub fn averting(&self) -> Option<Vec3> {
        self.avert.map(|a| a.from)
    }
}

/// Share of roaming goals picked at the map's objectives (bomb sites,
/// hostages) rather than anywhere, so teams meet where CS:S bots do.
pub(super) const OBJECTIVE_SHARE: f32 = 0.7;

/// The map's objective points (engine space, feet height of the volume's
/// bottom or the entity): bomb sites (`func_bomb_target`), hostage rescue
/// zones and hostages.
pub fn objectives(map: &crate::map::MapEntities) -> Vec<Vec3> {
    objectives_of(
        map,
        &[
            "func_bomb_target",
            "func_hostage_rescue",
            "hostage_entity",
        ],
    )
}

/// `objectives` of the given classes only.
pub fn objectives_of(map: &crate::map::MapEntities, classes: &[&str]) -> Vec<Vec3> {
    use crate::map::entities::{entity_rotation, entity_to_engine};
    map.entities
        .iter()
        .filter_map(|e| {
            let class = e.classname();
            if !classes.contains(&class) {
                return None;
            }
            match class {
                "func_bomb_target" | "func_hostage_rescue" => {
                    let points: Vec<Vec3> = e.hulls.iter().flat_map(|h| h.points.iter().copied()).collect();
                    if points.is_empty() {
                        return None;
                    }
                    let lo = points.iter().fold(Vec3::splat(f32::MAX), |a, p| a.min(*p));
                    let hi = points.iter().fold(Vec3::splat(f32::MIN), |a, p| a.max(*p));
                    let local = Vec3::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0, lo.z);
                    Some(entity_to_engine(
                        e.origin() + entity_rotation(e.angles()) * local,
                        map.scale,
                    ))
                }
                "hostage_entity" => Some(entity_to_engine(e.origin(), map.scale)),
                _ => None,
            }
        })
        .collect()
}

/// Seconds a bot keeps chasing what it saw or heard.
pub(super) const MEMORY: f64 = 15.0;
/// A remembered or roaming goal counts as reached within this, m.
pub(super) const ARRIVED: f32 = 2.0;
/// A goal it was sent to counts as reached within half `ARRIVED` and this
/// much above or below, m.
pub(super) const SENT_RISE: f32 = 0.5;
/// A sound is heard when its distance gain at the bot is above this.
pub(super) const HEARING_GAIN: f32 = 0.05;

/// Bots hear enemies' sounds (shots, footsteps, impacts they cause).
pub(super) fn hear(
    mut sounds: MessageReader<crate::map::PlaySound>,
    bank: Option<Res<crate::map::sound::SoundBank>>,
    mut bots: Query<(&mut Bot, &Transform, &Team, &Health)>,
    teams: Query<(&Team, &Transform, Option<&Health>)>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();
    for s in sounds.read() {
        let (Some(source), Some(at)) = (s.source, s.at) else {
            continue;
        };
        let Ok((source_team, source_at, source_health)) = teams.get(source) else {
            continue;
        };
        // Not a dead one's (its last cry, its gun hitting the floor).
        if source_health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let level = bank
            .as_ref()
            .and_then(|b| b.0.entry(&s.entry).map(|e| e.level))
            .unwrap_or(crate::map::SoundLevel::Db(75.0));
        let feet = source_at.translation - Vec3::Y * CAPSULE_HEIGHT / 2.0;
        for (mut bot, t, team, health) in &mut bots {
            if team == source_team || health.current <= 0.0 {
                continue;
            }
            if crate::map::sound::distance_gain(level, t.translation.distance(at)) >= HEARING_GAIN {
                bot.lead = Some((feet, now));
                bot.lead_who = Some(source);
            }
        }
    }
}

/// Spawn a bot on `team` at a spawn point (the team's, if the map has
/// them), moving with the match's movement (`game::movement::match_movement`).
pub fn add_bot(world: &mut World, team: Team) -> Option<Entity> {
    let e = spawn_bot(world, team)?;
    if let Some(mut q) = world.get_resource_mut::<BotQuota>() {
        q.quota += 1;
    }
    Some(e)
}

/// `add_bot` without raising `bot_quota` (the quota's own adds).
pub(super) fn spawn_bot(world: &mut World, team: Team) -> Option<Entity> {
    let spawns: Vec<(Transform, Option<Team>)> = world
        .query::<(&Transform, &SpawnPoint)>()
        .iter(world)
        .map(|(t, s)| (*t, s.team))
        .collect();
    let numbers: Vec<u32> = world.query::<&Bot>().iter(world).map(|b| b.number).collect();
    let bots = numbers.len();
    // The lowest free number, so seeds repeat run to run whatever else
    // was spawned (entity ids shift with every spawn).
    let number = (1..).find(|n| !numbers.contains(n)).unwrap_or(1);
    let seed = path::hash64(0xB075_EED5 ^ (team.0 as u64) << 40, number as usize);
    let round = world.get_resource::<crate::core::RoundRestarts>().map_or(0, |r| r.0);
    let own: Vec<&Transform> = spawns.iter().filter(|s| s.1 == Some(team)).map(|s| &s.0).collect();
    let pool: Vec<&Transform> = if own.is_empty() {
        spawns.iter().map(|s| &s.0).collect()
    } else {
        own
    };
    // Skip the first spawn, where the local player starts.
    let at = **pool.get((bots + 1) % pool.len().max(1)).or(pool.first())?;
    // The match's movement, not the local player's current one (a bot
    // added while the player flew with noclip flew too).
    let movement = crate::game::movement::match_movement(world)?;
    let yaw = at.rotation.to_euler(EulerRot::YXZ).0;
    // A name from the game's bot profiles at `bot_difficulty` (one no bot
    // here has), else "Bot <number>".
    let quota = world.get_resource::<BotQuota>().cloned().unwrap_or_default();
    let level = crate::map::bot_profiles::difficulty::of_level(quota.difficulty);
    let taken: Vec<String> = world
        .query::<&Bot>()
        .iter(world)
        .filter_map(|b| b.profile.clone())
        .collect();
    let profile = world
        .get_resource::<crate::map::bot_profiles::BotProfiles>()
        .and_then(|p| p.choose(level, team.0, &taken, seed >> 7))
        .cloned();
    let name = match &profile {
        Some(p) if quota.prefix.trim().is_empty() => p.name.clone(),
        Some(p) => format!("{} {}", quota.prefix.trim(), p.name),
        None => format!("Bot {number}"),
    };
    let difficulty = profile.as_ref().map_or(level, |p| if p.difficulty & level != 0 { level } else { p.difficulty });
    let mut commands = world.commands();
    // Characters aren't rotated; the spawn's facing becomes the look yaw.
    let e = spawn_character(
        &mut commands,
        Transform::from_translation(at.translation),
        team,
        movement,
    );
    commands.entity(e).insert((
        Name::new(name),
        crate::core::Seed(seed),
        {
            let mut bot = Bot {
                number,
                seed,
                profile: profile.map(|p| p.name),
                difficulty,
                ..default()
            };
            bot.reseed(round);
            bot
        },
        Intent { yaw, ..default() },
    ));
    world.flush();
    Some(e)
}

impl Bot {
    /// Its number ("Bot 3").
    pub fn number(&self) -> u32 {
        self.number
    }

    /// Restart the dice for a round: from the bot's seed and the round
    /// number only.
    pub(super) fn reseed(&mut self, round: u32) {
        self.rng = path::hash64(self.seed ^ 0x2545_F491_4F6C_DD1D, round as usize) | 1;
    }

    /// A roll of its dice, 0 to 1 (seeded: `reseed`), for rules outside
    /// the brain (its votes: `game::votes`).
    pub fn roll(&mut self) -> f32 {
        self.rand()
    }

    pub(super) fn rand(&mut self) -> f32 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Where to aim on a character: chest height above its origin (the
/// origin is the capsule's centre).
pub(super) const AIM_HEIGHT: f32 = 0.35;
/// Fire when the aim is within this many degrees of the target.
pub(super) const FIRE_CONE_DEG: f32 = 3.0;
/// Bots can't see while a flash leaves the screen whiter than this.
pub(super) const BLIND_ALPHA: f32 = 0.5;
/// Seconds between new aim wobbles.
pub(super) const AIM_REROLL: f32 = 0.4;

