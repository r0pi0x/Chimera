//! Hostages (specs/cs_source/objectives.md 9): characters with no team,
//! placed by the map's `hostage_entity`s at each round start, moved by
//! the game's own movement. A defender's +use makes one follow him (again:
//! stay); a following hostage walks the navigation mesh to its leader
//! (*hyp.*, Q10: stop within 100 units, walk within 250, run beyond; give
//! up beyond 2000, measured, or after 10 s out of sight, *hyp.*). A following hostage on its
//! way whose 26-unit-wide hull touches a rescue zone is rescued (measured,
//! Q11). Hurting and killing them is reported for the rules' money.
//!
//! Covers: `Hostage` characters (no team, the game's movement, `map::BodyName`
//! model, the four handed out in turn: `HostageModelCycle`, Q9; the map's
//! `skin` as `map::BodySkin`) spawned at
//! each round start at the map's origins (new entities every round, after
//! a round end or `mp_restartgame` alike, as CS:S's; measured, spec 9
//! "Spawn"), falling from there onto the floor and resting 1/32 above it
//! (`movement::SettleOnGround`; measured, spec 9 "Spawn"), always with the
//! match's own movement (`movement::match_movement`), never the local
//! player's current one (noclip left them in the air), +use to lead/stop (defenders only), following over the
//! nav mesh (stop/walk/run distances, give up when far or unseen), rescue in
//! a rescue zone (only while following and walking; none without zones),
//! `HostageTally`,
//! hurt/killed events, `mp_hostagepenalty` (`HostagePenalty`), how bullets
//! hurt them (`HostageRules::bullets` as each one's `weapon::BulletTarget`:
//! CS:S's own hitgroup multipliers, a second hit through the body, health
//! a float; measured, spec Q8).

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{
    MapObjectives, ObjectiveEvent, UNIT,
    bomb::{LastButtons, hull, user_of},
    use_search::{UseTarget, find_use},
};
use crate::{
    character::character_bundle,
    console::resource_cvar,
    core::{Damage, Died, Health, Intent, LocalPlayer, MaxSpeed, MovementState, SimSet, Team},
    map::{BodyName, BodySkin, GameSound, PlaySound, nav::NavMesh},
    slots::set_movement,
    weapon::{BulletTarget, HitgroupScale, Inventory},
};

/// Sound entries hostages use (the game's names).
#[derive(Clone, Debug, Default)]
pub struct HostageSounds {
    pub pain: Option<String>,
    pub start_follow: Option<String>,
    pub stop_follow: Option<String>,
    /// The announcer (everyone hears it).
    pub announce_touched: Option<String>,
    pub announce_rescued: Option<String>,
    pub announce_killed: Option<String>,
}

/// How hostages behave; CS:S's numbers (spec, mostly *hyp.*) by default,
/// models and sounds from the game.
#[derive(Resource, Clone, Debug)]
pub struct HostageRules {
    /// Character models to choose from (`map::BodyName`); empty: the
    /// default body.
    pub models: Vec<String>,
    /// Who may lead them.
    pub leader_team: Team,
    /// Health (1.0 = a player's 100).
    pub health: f32,
    /// Speeds, units/s, and the distances (units) where they apply.
    pub walk_speed: f32,
    pub run_speed: f32,
    pub stop_within: f32,
    pub walk_within: f32,
    /// Give up following beyond this (units) or after this long out of
    /// sight (seconds).
    pub give_up_distance: f32,
    pub give_up_unseen: f32,
    /// Half the width of the box that must touch a rescue zone (units;
    /// CS:S's hostage hull is 26 x 26 x 72, measured, Q11).
    pub rescue_half_width: f32,
    /// `mp_hostagepenalty`: hostage kills before being removed (0: never).
    pub penalty: u32,
    /// How bullets hurt them (on each hostage as its `BulletTarget`).
    pub bullets: BulletTarget,
    pub sounds: HostageSounds,
}

/// CS:S's hostage hitgroup multipliers (measured, spec Constants,
/// Hostages, "the rule"): not the players'.
pub const HOSTAGE_HITGROUPS: HitgroupScale = HitgroupScale {
    head: 2.5,
    chest: 1.5,
    stomach: 1.75,
    arm: 0.75,
    leg: 0.6,
};

impl Default for HostageRules {
    fn default() -> Self {
        Self {
            // Measured (spec Q8): their own multipliers, a bullet through
            // one of their boxes may hit another, health a float.
            bullets: BulletTarget {
                hitgroups: Some(HOSTAGE_HITGROUPS),
                hits_inside: true,
                exact: true,
            },
            models: Vec::new(),
            leader_team: Team(2),
            health: 1.0,
            walk_speed: 100.0,
            run_speed: 250.0,
            stop_within: 100.0,
            walk_within: 250.0,
            // CS:S (measured 2026-10-18, objectives.md Q10): a leader put
            // 1980 units off is still followed, 2060 off is given up at
            // once.
            give_up_distance: 2000.0,
            give_up_unseen: 10.0,
            rescue_half_width: 13.0,
            penalty: 13,
            sounds: HostageSounds::default(),
        }
    }
}

/// A hostage: a character that isn't a player (`core::Npc`).
#[derive(Component, Clone, Debug, Default)]
#[require(crate::core::Npc)]
pub struct Hostage {
    /// Its index in `MapObjectives::hostages`.
    pub index: usize,
    /// Whom it follows.
    pub leader: Option<Entity>,
    /// Someone led it already this round (the first touch pays).
    pub taken: bool,
    route: Vec<Vec3>,
    next: usize,
    repath: f32,
    /// Seconds the leader has been out of sight.
    unseen: f32,
    progress: (Vec3, f32),
}

impl Hostage {
    /// A hostage as a network client shows it (the server leads it):
    /// which one and whom it follows.
    pub fn shown(index: usize, leader: Option<Entity>) -> Self {
        Self {
            index,
            leader,
            ..default()
        }
    }
}

/// The next hostage model (an index into `HostageRules::models`): CS:S
/// hands them out in turn to every hostage spawned, round after round and
/// across maps (measured, spec Q9).
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct HostageModelCycle(pub usize);

/// This round's hostages: how many, rescued, killed.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostageTally {
    pub total: u32,
    pub rescued: u32,
    pub killed: u32,
}

impl HostageTally {
    /// Every hostage still alive has been rescued, at least one (spec 7).
    pub fn all_rescued(&self) -> bool {
        self.rescued > 0 && self.rescued + self.killed >= self.total
    }
}

/// Hostages a player has killed (`mp_hostagepenalty`).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct HostageKills(pub u32);

/// Why a player is warned or removed for killing hostages.
#[derive(Message, Clone, Debug, PartialEq)]
pub enum HostagePenalty {
    /// One more and they're out.
    Warning {
        who: Entity,
    },
    Removed {
        who: Entity,
    },
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<HostageRules>()
        .init_resource::<HostageTally>()
        .init_resource::<HostageModelCycle>()
        .add_message::<HostagePenalty>()
        .add_systems(
            FixedUpdate,
            (
                follow
                    .after(SimSet::Rules)
                    .after(crate::weapon::SelectWeapons)
                    .before(SimSet::Movement),
                rescue.after(SimSet::Movement).before(SimSet::Weapons),
                hurt.after(crate::core::apply_damage),
            )
                .run_if(crate::core::authoritative)
                .run_if(super::plays),
        );
    resource_cvar::<HostageRules, u32>(
        app,
        "mp_hostagepenalty",
        "How many hostages a player may kill before being removed (0: no limit).",
        |r| &mut r.penalty,
    );
}

/// +use presses on hostages (spec 9, "Use"): one press, one hostage.
#[allow(clippy::type_complexity)]
pub(super) fn use_hostages(
    users: Query<
        (
            Entity,
            &Intent,
            &Transform,
            Option<&MovementState>,
            Option<&Health>,
            Option<&Team>,
        ),
        Without<Hostage>,
    >,
    last: Res<LastButtons>,
    mut hostages: Query<(Entity, &mut Hostage, &Transform, Option<&MovementState>, &Health)>,
    characters: Query<(), With<Intent>>,
    spatial: SpatialQuery,
    rules: Res<HostageRules>,
    mut events: MessageWriter<ObjectiveEvent>,
    mut play: MessageWriter<GameSound>,
) {
    if hostages.is_empty() {
        return;
    }
    let filter = SpatialQueryFilter::default().with_mask(crate::core::SOLID_LAYERS);
    let clear = |a: Vec3, b: Vec3| -> bool {
        let d = b - a;
        let Ok(dir) = Dir3::new(d) else { return true };
        spatial
            .cast_ray_predicate(a, dir, d.length(), true, &filter, &|e| !characters.contains(e))
            .is_none()
    };
    let targets: Vec<UseTarget> = hostages
        .iter()
        .filter(|(.., h)| h.current > 0.0)
        .map(|(e, _, t, s, _)| {
            let (lo, hi) = hull(t, s);
            UseTarget { entity: e, lo, hi }
        })
        .collect();
    for (e, intent, t, state, health, team) in &users {
        let pressed = intent.use_key && !last.get(e).use_key;
        if !pressed || health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let Some(found) = find_use(&user_of(t, intent, state), &targets, &clear) else {
            continue;
        };
        let Ok((_, mut h, ht, ..)) = hostages.get_mut(found) else {
            continue;
        };
        if team != Some(&rules.leader_team) {
            events.write(ObjectiveEvent::HostageRefused { who: e });
            continue;
        }
        let sound = if h.leader == Some(e) {
            h.leader = None;
            events.write(ObjectiveEvent::HostageStops {
                hostage: found,
                leader: e,
            });
            &rules.sounds.stop_follow
        } else {
            let first = !h.taken;
            h.taken = true;
            h.leader = Some(e);
            h.unseen = 0.0;
            h.repath = 0.0;
            events.write(ObjectiveEvent::HostageFollows {
                hostage: found,
                leader: e,
                first,
            });
            if first && let Some(s) = &rules.sounds.announce_touched {
                play.write(GameSound(PlaySound::ui(s.clone())));
            }
            &rules.sounds.start_follow
        };
        if let Some(s) = sound {
            play.write(GameSound(PlaySound {
                pitch: None,
                entry: s.clone(),
                at: Some(ht.translation),
                volume: None,
                source: Some(found),
                channel: Some(2),
            }));
        }
    }
}

/// Following hostages walk toward their leader.
#[allow(clippy::type_complexity)]
fn follow(
    mut hostages: Query<(
        Entity,
        &mut Hostage,
        &mut Intent,
        &Transform,
        Option<&MovementState>,
        &Health,
    )>,
    leaders: Query<(&Transform, Option<&MovementState>, Option<&Health>), Without<Hostage>>,
    characters: Query<(), With<Intent>>,
    spatial: SpatialQuery,
    nav: Option<Res<NavMesh>>,
    rules: Res<HostageRules>,
    mut events: MessageWriter<ObjectiveEvent>,
    mut commands: Commands,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let filter = SpatialQueryFilter::default().with_mask(crate::core::SOLID_LAYERS);
    for (e, mut h, mut intent, t, state, health) in &mut hostages {
        intent.move_axis = Vec2::ZERO;
        intent.jump = false;
        if health.current <= 0.0 {
            h.leader = None;
            continue;
        }
        let Some(leader) = h.leader else { continue };
        let Ok((lt, ls, lh)) = leaders.get(leader) else {
            h.leader = None;
            continue;
        };
        let feet = t.translation.with_y(hull(t, state).0.y);
        let goal = lt.translation.with_y(hull(lt, ls).0.y);
        let distance = (goal - feet).xz().length();
        // Out of sight for long, too far, or the leader dead: it stays.
        let eye = t.translation + Vec3::Y * 0.6;
        let to = lt.translation + Vec3::Y * 0.6 - eye;
        let seen = Dir3::new(to).is_ok_and(|dir| {
            spatial
                .cast_ray_predicate(eye, dir, to.length(), true, &filter, &|x| !characters.contains(x))
                .is_none()
        });
        h.unseen = if seen { 0.0 } else { h.unseen + dt };
        if lh.is_some_and(|x| x.current <= 0.0)
            || distance > rules.give_up_distance * UNIT
            || h.unseen > rules.give_up_unseen
        {
            h.leader = None;
            events.write(ObjectiveEvent::HostageStops { hostage: e, leader });
            continue;
        }
        if distance < rules.stop_within * UNIT {
            h.route.clear();
            continue;
        }
        let speed = if distance < rules.walk_within * UNIT {
            rules.walk_speed
        } else {
            rules.run_speed
        };
        commands.entity(e).insert(MaxSpeed(Some(speed * UNIT)));
        // The route over the navigation mesh (straight without one).
        h.repath -= dt;
        h.progress.1 += dt;
        let mut stuck = false;
        if h.progress.1 >= 1.5 {
            stuck = feet.distance(h.progress.0) < 0.3;
            h.progress = (feet, 0.0);
        }
        if h.repath <= 0.0 || h.next >= h.route.len() || stuck {
            h.repath = 1.0;
            h.route = nav
                .as_deref()
                .and_then(|n| n.route(feet, goal))
                .unwrap_or_else(|| vec![goal]);
            h.next = 0;
        }
        while h.next < h.route.len() && (h.route[h.next] - feet).xz().length() < 20.0 * UNIT {
            h.next += 1;
        }
        let point = h.route.get(h.next).copied().unwrap_or(goal);
        let d = point - feet;
        if d.xz().length() > 1e-3 {
            intent.yaw = (-d.x).atan2(-d.z);
        }
        intent.pitch = 0.0;
        intent.move_axis = Vec2::Y;
        intent.jump = stuck || (d.y > crate::map::nav::STEP_HEIGHT && d.xz().length() < 1.5);
    }
}

/// Following hostages whose hull touches a rescue zone while they walk
/// are rescued: gone, counted (measured, Q11: an idle hostage in a zone,
/// or one standing near its leader there, is not; a map without rescue
/// zones rescues nobody, not even at the defenders' spawns).
#[allow(clippy::type_complexity)]
fn rescue(
    hostages: Query<(Entity, &Hostage, &Transform, Option<&MovementState>, &Health, &Intent)>,
    objectives: Res<MapObjectives>,
    rules: Res<HostageRules>,
    mut tally: ResMut<HostageTally>,
    mut events: MessageWriter<ObjectiveEvent>,
    mut play: MessageWriter<GameSound>,
    mut commands: Commands,
) {
    for (e, h, t, state, health, intent) in &hostages {
        // Following and on its way (`follow` set this tick's move).
        if health.current <= 0.0 || h.leader.is_none() || intent.move_axis == Vec2::ZERO {
            continue;
        }
        let (lo, hi) = rescue_box(t, state, rules.rescue_half_width);
        if !objectives.rescue_zones.iter().any(|z| z.overlaps(lo, hi)) {
            continue;
        }
        tally.rescued += 1;
        commands.entity(e).despawn();
        if let Some(s) = &rules.sounds.announce_rescued {
            play.write(GameSound(PlaySound::ui(s.clone())));
        }
        events.write(ObjectiveEvent::HostageRescued {
            hostage: e,
            leader: h.leader,
        });
    }
}

/// The box that must touch a rescue zone: the movement hull's height,
/// CS:S's hostage width around its middle.
fn rescue_box(t: &Transform, state: Option<&MovementState>, half_width_units: f32) -> (Vec3, Vec3) {
    let (lo, hi) = hull(t, state);
    let (c, w) = ((lo + hi) / 2.0, half_width_units * UNIT);
    (Vec3::new(c.x - w, lo.y, c.z - w), Vec3::new(c.x + w, hi.y, c.z + w))
}

/// Hostages hurt and killed (spec 9, "Damage"), and the penalty for
/// killing them.
#[allow(clippy::too_many_arguments)]
fn hurt(
    mut damage: MessageReader<Damage>,
    mut died: MessageReader<Died>,
    hostages: Query<&Transform, With<Hostage>>,
    mut kills: Query<(Option<&mut HostageKills>, Has<LocalPlayer>)>,
    rules: Res<HostageRules>,
    mut tally: ResMut<HostageTally>,
    mut events: MessageWriter<ObjectiveEvent>,
    mut penalties: MessageWriter<HostagePenalty>,
    mut play: MessageWriter<GameSound>,
    mut commands: Commands,
) {
    for d in damage.read() {
        let Ok(t) = hostages.get(d.target) else { continue };
        if d.amount <= 0.0 {
            continue;
        }
        if let Some(s) = &rules.sounds.pain {
            play.write(GameSound(PlaySound {
                pitch: None,
                entry: s.clone(),
                at: Some(t.translation),
                volume: None,
                source: Some(d.target),
                channel: Some(2),
            }));
        }
        events.write(ObjectiveEvent::HostageHurt {
            hostage: d.target,
            attacker: d.attacker,
            amount: d.amount,
        });
    }
    for d in died.read() {
        if !hostages.contains(d.entity) {
            continue;
        }
        tally.killed += 1;
        if let Some(s) = &rules.sounds.announce_killed {
            play.write(GameSound(PlaySound::ui(s.clone())));
        }
        events.write(ObjectiveEvent::HostageKilled {
            hostage: d.entity,
            attacker: d.attacker,
        });
        let Some(who) = d.attacker else { continue };
        let Ok((count, local)) = kills.get_mut(who) else {
            continue;
        };
        let n = count.as_ref().map_or(0, |c| c.0) + 1;
        match count {
            Some(mut c) => c.0 = n,
            None => {
                commands.entity(who).try_insert(HostageKills(n));
            }
        }
        if rules.penalty > 0 && n + 1 == rules.penalty {
            penalties.write(HostagePenalty::Warning { who });
        } else if rules.penalty > 0 && n >= rules.penalty {
            penalties.write(HostagePenalty::Removed { who });
            // Computer players leave; the local player can't be kicked
            // from its own game (told only).
            if !local {
                commands.entity(who).try_despawn();
            }
        }
    }
}

/// A new round's hostages: last round's go, the map's come back.
pub(super) fn round_start(world: &mut World) {
    let old: Vec<Entity> = world.query_filtered::<Entity, With<Hostage>>().iter(world).collect();
    for e in old {
        world.despawn(e);
    }
    let spawns = world.resource::<MapObjectives>().hostages.clone();
    *world.resource_mut::<HostageTally>() = HostageTally {
        total: spawns.len() as u32,
        ..default()
    };
    if spawns.is_empty() {
        return;
    }
    let rules = world.resource::<HostageRules>().clone();
    // The match's movement, never the local player's current one (noclip
    // gave every hostage noclip: they hung where the map puts them instead
    // of falling onto the floor as CS:S's do, spec 9 "Spawn").
    let movement = crate::game::movement::match_movement(world);
    for (i, s) in spawns.iter().enumerate() {
        spawn_hostage(world, i, s, &rules, movement);
    }
}

fn spawn_hostage(
    world: &mut World,
    index: usize,
    s: &super::HostageSpawn,
    rules: &HostageRules,
    movement: Option<&'static str>,
) -> Entity {
    let at = s.feet + Vec3::Y * crate::map::SPAWN_LIFT;
    let mut e = world.spawn(character_bundle(Transform::from_translation(at), Team(0)));
    e.remove::<Team>().insert((
        Name::new(format!("Hostage {}", index + 1)),
        Hostage { index, ..default() },
        Intent {
            yaw: s.yaw,
            ..default()
        },
        Health {
            current: rules.health,
            max: rules.health,
        },
        // No weapons for them.
        Inventory::default(),
        MaxSpeed(Some(rules.walk_speed * UNIT)),
        rules.bullets,
        // They fall from the map's origin and rest on the floor, 1/32
        // above it, as CS:S's do (measured, spec 9 "Spawn").
        crate::game::movement::SettleOnGround,
    ));
    let id = e.id();
    // The map's skin stays, whatever the model (measured, spec Q9).
    if s.skin != 0 {
        world.entity_mut(id).insert(BodySkin(s.skin));
    }
    // Which model: neither the map's model nor its `HostageType` picks one;
    // each hostage spawned takes the next of the four in turn, the turn
    // going on over rounds and maps (measured, spec Q9).
    if !rules.models.is_empty() {
        let mut cycle = world.get_resource_or_init::<HostageModelCycle>();
        let k = cycle.0 % rules.models.len();
        cycle.0 = k + 1;
        world.entity_mut(id).insert(BodyName(rules.models[k].clone()));
    }
    if let Some(m) = movement {
        set_movement(id, m).apply(world);
    }
    id
}

/// Spawn the map's hostages now (tests, tools): as at a round start.
pub fn spawn_all(world: &mut World) {
    round_start(world);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_rescued_counts_the_living() {
        let t = |total, rescued, killed| HostageTally { total, rescued, killed };
        assert!(!t(4, 3, 0).all_rescued());
        assert!(t(4, 4, 0).all_rescued());
        assert!(t(4, 3, 1).all_rescued(), "the fourth is dead");
        // Measured (Q11, 2026-10-17): 3 rescued and the 4th dead, or 2 and
        // 2, end the round (CTs win, all rescued); all 4 dead doesn't.
        assert!(t(4, 2, 2).all_rescued());
        assert!(!t(4, 0, 4).all_rescued(), "none rescued: the round goes on");
    }
}
