//! Physics props as HL2:DM's game plays them (specs/hl2dm/gravity_gun.md
//! 8, 9, Corrections 1), around the gravity gun (`weapons::physcannon`):
//!
//! - **Impact damage to players** at HL2:DM's energy scale, 4
//!   (`source_logic::logic::PlayerImpactScale`; CS:S's 1), its stress
//!   crushing too; held bodies deal and take none (Chimera's bridge reads
//!   `map::held::Held`).
//! - **Credit** (`credit`): a prop's impact on a player within 1 s of its
//!   influence (`map::held::PhysicsInfluence`: the last punt, grab, drop
//!   or launch) names that player the attacker; later, the prop itself (a
//!   world kill). The death notice's weapon is `physics`. The influencer
//!   hurt by his own prop is spared only right after his touch (within
//!   the bridge's impact measurement, `SELF_SPARED_TICKS`), else it is
//!   his own damage (a suicide if it kills); a teammate's credited impact
//!   is refused with `mp_friendlyfire` 0 (the engine's rule, after this).
//! - `prop_physics_multiplayer` in modes 1 and 2 collides with players as
//!   a `prop_physics` (Corrections 1: HL2:DM drops CS:S's exception).
//! - **`+use` at props** (`use_props`, `hl2mp_allow_pickup` 0, the
//!   default): a fresh press finding no logic entity but a usable prop
//!   within 80 units (spawnflag 256, or at most 35 kg and 128 units on each
//!   side, movable or flag 64, not flag 512) plays `HL2Player.Use` (and
//!   fires its `OnPlayerUse` with 256); an unusable prop plays
//!   `HL2Player.UseDeny`; nothing is carried (with `hl2mp_allow_pickup` 1
//!   a prop passing the carry test is picked up: `carry`).
//!
//! Known shortcuts: an explosive prop's blast credit
//! (the last attacker, else the influencer within 2 s) is the logic's
//! last-attacker rule only; the 10× first impact after a launch from the
//! hold and first-impact breaks aren't done. `prop_physics_respawnable`
//! (section 13) is source_logic's prop (`RespawnTime`, 60 s by default, the
//! prop stays with its name and outputs, hidden with its body off, and
//! comes back at its home pose after the time if its box there is free of
//! solid things, else tries again each second); the free-box test is the
//! physics engine's, players included.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    core::{Damage, DamageKind, Intent, MovementState, SimClock, SimSet},
    map::{
        MapEntities, PhysicsProp, PlaySound, PropEntity, PushAway,
        entities::FireEntityOutput,
        held::{Held, PhysicsInfluence},
        prop_physics::FrozenBody,
    },
};

/// HL2:DM's players' physics impact energy scale (9).
pub const PLAYER_IMPACT_SCALE: f32 = 4.0;
/// An impact on a player this long after the prop's influence credits
/// the influencer (s).
pub const CREDIT_WINDOW: f64 = 1.0;
/// The influencer's own prop spares him for this many ticks after his
/// touch: the impact the bridge measures over its first steps (the spec's
/// "same tick"; our impacts land 4 steps after they begin).
pub const SELF_SPARED_TICKS: f64 = 5.0;
/// The death notice's class for a prop's kill.
pub const PHYSICS_WEAPON: &str = "prop_physics";

/// `+use` reach (doors_buttons.md, suit.md "Wall chargers": 80 units).
pub const USE_REACH: f32 = 80.0;
/// `+use` carry limits (8): summed mass, each box side.
pub const USE_MASS: f32 = 35.0;
pub const USE_SIZE: f32 = 128.0;
const SF_USE_OUTPUT: u32 = 256;
const SF_ENABLE_ON_GUN: u32 = 64;
const SF_NO_PICKUP: u32 = 512;

const U: f32 = 0.0254;

/// Before the damage lands (and before the rules' refusals): a prop's
/// impact on a player names its influencer within the window.
pub fn credit(
    mut damage: MessageMutator<Damage>,
    props: Query<Option<&PhysicsInfluence>, With<PhysicsProp>>,
    players: Query<(), With<Intent>>,
    clock: Res<SimClock>,
) {
    let (now, dt) = (clock.now, clock.dt() as f64);
    for d in damage.read() {
        if d.kind != DamageKind::Crush || d.weapon.is_some() || !players.contains(d.target) {
            continue;
        }
        let Some(prop) = d.attacker else { continue };
        let Ok(influence) = props.get(prop) else { continue };
        d.weapon = Some(PHYSICS_WEAPON);
        let Some(i) = influence.filter(|i| now - i.at <= CREDIT_WINDOW + 1e-6) else { continue };
        if i.by == d.target && now - i.at <= SELF_SPARED_TICKS * dt + 1e-6 {
            d.amount = 0.0;
            continue;
        }
        d.attacker = Some(i.by);
    }
}

/// Multiplayer props in server modes 1 and 2 collide with players.
pub fn solid_multiplayer(mut props: Query<&mut PhysicsProp, Added<PhysicsProp>>) {
    for mut p in &mut props {
        if matches!(p.push, PushAway::Solid | PushAway::NonSolid) {
            p.push = PushAway::Collide;
        }
    }
}

/// A fresh `+use` press at a physics prop (8).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn use_props(
    players: Query<
        (Entity, &Intent, &Transform, &MovementState, &source_engine::source_movement::SourceMovement),
        Without<super::carry::Carrying>,
    >,
    props: Query<(Option<&PhysicsProp>, Option<&FrozenBody>, Option<&RigidBody>, Option<&PropEntity>), Without<Held>>,
    of: Query<&ColliderOf>,
    logic: Option<Res<source_logic::logic::Logic>>,
    entities: Option<Res<MapEntities>>,
    spatial: SpatialQuery,
    mut play: MessageWriter<PlaySound>,
    mut outputs: MessageWriter<FireEntityOutput>,
    carry: Option<Res<super::carry::CarryRules>>,
    mut commands: Commands,
) {
    let allow = carry.as_ref().is_some_and(|c| c.allow_pickup != 0);
    for (who, intent, at, state, me) in &players {
        if !intent.use_key || me.use_held {
            continue;
        }
        // The map's own use found something: that is the use.
        if logic
            .as_ref()
            .is_some_and(|l| l.world.use_presses.iter().any(|(e, found)| *e == who && *found))
        {
            continue;
        }
        let eye = at.translation + state.eye_offset;
        let dir = chimera_sim::weapon::aim_of(intent.yaw, intent.pitch) * Vec3::NEG_Z;
        let Ok(dir) = Dir3::new(dir) else { continue };
        let filter = SpatialQueryFilter::from_excluded_entities([who]).with_mask(crate::core::SOLID_LAYERS);
        let Some(hit) = spatial.cast_ray(eye, dir, USE_REACH * U, true, &filter) else { continue };
        let body = of.get(hit.entity).map_or(hit.entity, |o| o.body);
        let Ok((prop, frozen, rb, index)) = props.get(body) else { continue };
        let (mass, bounds) = match (prop, frozen) {
            (Some(p), _) => (p.mass, p.bounds),
            (None, Some(f)) => (f.physics.mass, f.bounds),
            _ => continue,
        };
        let flags: u32 = index
            .and_then(|i| entities.as_ref()?.entities.get(i.0)?.get("spawnflags")?.trim().parse().ok())
            .unwrap_or(0);
        let size = (bounds.1 - bounds.0).abs() / U;
        let movable = rb.is_some_and(|r| r.is_dynamic()) || flags & SF_ENABLE_ON_GUN != 0;
        let carry_ok = mass <= USE_MASS && size.max_element() <= USE_SIZE + 1e-3 && movable && flags & SF_NO_PICKUP == 0;
        let usable = flags & SF_USE_OUTPUT != 0 || carry_ok;
        let entry = if usable { "HL2Player.Use" } else { "HL2Player.UseDeny" };
        play.write(PlaySound {
            pitch: None,
            entry: entry.into(),
            at: Some(eye),
            volume: None,
            source: Some(who),
            channel: None,
        });
        if flags & SF_USE_OUTPUT != 0
            && let Some(i) = index
        {
            outputs.write(FireEntityOutput {
                map_index: i.0,
                output: "OnPlayerUse".into(),
                activator: Some(who),
            });
        }
        // `hl2mp_allow_pickup` 1: carried (`carry`), unless it is what the
        // player stands on.
        if allow && carry_ok && state.ground != Some(body) {
            commands.queue(move |w: &mut World| super::carry::pick_up(w, who, body));
        }
    }
}

/// HL2:DM's physics rules in its own game.
pub struct Hl2PhysicsPlugin;

impl Plugin for Hl2PhysicsPlugin {
    fn build(&self, app: &mut App) {
        super::carry::plugin(app);
        app.insert_resource(source_logic::logic::PlayerImpactScale(PLAYER_IMPACT_SCALE))
            .add_systems(
                FixedUpdate,
                (
                    solid_multiplayer,
                    use_props
                        .after(source_logic::logic::LogicSet::Pre)
                        .before(SimSet::Movement)
                        .run_if(crate::core::authoritative),
                    credit
                        .after(SimSet::Weapons)
                        .before(crate::core::apply_damage)
                        .run_if(crate::core::authoritative),
                ),
            );
    }
}
