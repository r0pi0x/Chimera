//! The logic world in the ECS: built from the loaded map's entities, run
//! each fixed tick in the spec's order around the characters' movement
//! (entity_io.md "Per-tick order"): thinks, movers and +use before
//! movement; trigger touches, untouch, the event queue and removals after.
//! Each phase ends by bringing the nodes in line with the logic
//! (`sync_movers`, `sync_props`, ...): a mover's or prop's node that
//! already reads as the logic wants is passed over after one look through
//! a kept query (`MoverWants::differ`, `prop_differs`: the writes' own
//! tests; `SyncEveryNode` runs every write's code for every node, which
//! `map_logic::skipping_unchanged_nodes_changes_nothing` compares with).
//! What a mover's node should read is worked out again only for movers
//! whose logic entity may have changed since the last phase
//! (`LogicWorld::changed_since`, stamped by `get_mut`); the others' is
//! kept from then (`MoverLook`). A mover whose node read as wanted at the
//! last phase is passed over without a look while nothing its wants or
//! the node's test depend on changed: its entity, whether it rides a
//! physics body, its brushes' count, the node's components (their change
//! ticks since the last phase's writes, `MoverViewChanged`) and its
//! archetype (a component added or taken away); nodes two movers share
//! are written every phase as before (`MoverCache`; docs/performance.md,
//! "A change stamp for the movers").
//!
//! Covers: `carry_player`: a network client's own player pushed by its movers'
//! steps as `LogicSet::Pre` does; `LogicPlugin`: built from `map::MapEntities`,
//! runs `LogicSet::Pre` before movement, `LogicSet::Post` after, and
//! `LogicSet::Damage` after weapons (weapon `Damage` on breakable nodes); moves
//! mover nodes and their `core::MovingSolid` (those riding a physics body
//! again after the physics step, `follow_bodies`), disables the collider of what
//! isn't there, gives broken windows `map::BrushPanes` (on
//! `core::TRACE_ONLY_LAYER`: shots hit them, no game's characters meet them),
//! hides nodes of removed
//! entities, pairs prop nodes (`map::PropEntity`) with logic props (weapon
//! damage with its push and attacker, physics impacts on props, breakables and
//! players after the physics step (crush damage by the spec's tables, from each
//! body's velocity before and after it; a prop meeting a player as two
//! contacts that hurt both, as measured in CS:S; players crushed by the props
//! pressing on them past the stress limit), a broken prop's sound,
//! `core::Explosion` and `map::BreakProp` pieces from where its body is, motion
//! requests through `map::prop_physics`, `PropAwakened` into OnAwakened,
//! players' ground prop for pressure, hidden with `map::LogicHidden` and
//! not solid (collider and own `MovingSolid` brushes) once gone, put back at
//! `map::PropHome` at a restart; skin, body group and sequence as
//! `map::PropLook`/`map::PropSequence`), writes sprites' and dust volumes'
//! `map::EntityPart`, the light styles (`map::LightStyles`) and which
//! trigger_soundscape volumes the local player touches
//! (`map::SoundscapeTouches`); lit fires and flames as `map::fire::MapFires`,
//! props' collider boxes into the logic (fire), the `ent_fire` console command
//! and `chimera_logic_record` (the logic world records fired outputs and
//! deliveries, the latest 256), applies effects (burn damage as `core::Damage`
//! of kind `Burn`, a flame's `General.BurningObject` loop following what burns,
//! ambient sounds as `map::SoundControl`, keyed by entity, following the
//! source's mover or prop node); at a round restart (`core::RoundRestarts`)
//! stops the entities' sounds (a map change too) and re-creates the world from
//! the map's entities, `LogicWorld::round_restart`: all but
//! `world::ROUND_KEEP`, queue dropped, nodes shown again, game_text cleared:
//! gibs and shattered panes as `map` messages; players' AddOutput (targetname,
//! gravity, basevelocity, health, origin, rendermode, renderamt, rendercolor)
//! and the Alpha and Color inputs (`LogicWorld::player_looks`, written as
//! `map::RenderLook` on the character) and SetDamageFilter (as
//! `core::DamageFilter`), game_player_equip/player_weaponstrip (as
//! `core::Equip`).
//!
//! Known shortcuts (docs/tech-debt.md): avian (0.7 and 0.8) leaves "body
//! disabled" on a collider's query-tree proxy after `RigidBodyDisabled` is
//! removed (its `Discard` observer still sees the component), and the broad
//! phase pairs no proxy so marked: a restored prop would meet nothing (with
//! 0.7 a new contact that touched in its first step was also linked into an
//! island twice). `logic::bridge::set_prop_shown` re-enables the body before
//! its colliders and rebuilds the proxies of colliders that stayed solid.
//! Why it's OK for now: Contained to one function, guarded by
//! `map_logic::restored_prop_contacts_join_islands_once`;
//! `tests::avian_still_leaves_body_disabled_on_the_proxy` fails once avian
//! clears the flag itself, and the workaround can go.

use avian3d::prelude::{
    AngularVelocity, Collider, ColliderAabb, ColliderDisabled, CollisionLayers, CollisionStart, ComputedMass,
    LayerMask, LinearVelocity, PhysicsSystems, Position, RigidBody, RigidBodyDisabled, Rotation,
};
use bevy::{ecs::message::MessageCursor, prelude::*};
use chimera_core::query_cache::{cached_query, first};

use super::classes::ServerLine;
use super::hud::{HudEvent, HudMessages, HudShow, ScreenFades};
use super::prop_damage::{Hit, Motion, PropExplosion};
use super::world::{Collision, Effect, EntId, LogicWorld, Player, SOLID_SKIN, SWEEP_EPS, Who};
use crate::console::{Console, ConsoleAppExt, Level};
use crate::core::{
    BaseVelocity, Damage, DamageKind, Damageable, EntityGravity, Explosion, Health, Hitgroup, Intent, LocalPlayer,
    MapBrush, MapBrushes, MapTerrain, MovementState, MovingSolid, RoundRestarts, SimSet, Team, Velocity,
};
use crate::map::breakables::{FallingPane, GibPiece, GlassImpact, GlassShatter, SpawnGibs};
use crate::map::entities::{engine_to_entity, entity_rotation, entity_to_engine, rotation_to_engine};
use crate::map::{LogicHidden, VisClusters};
use crate::map::{
    BreakProp, BrushPanes, EntityPart, LightStyles, MapBrushEntity, MapEntities, PlaySound, PropEntity, PropHome,
    PropIndex, PropLook, PropSequence, SoundControl, SoundKey, SoundLevel, SoundscapeSwitches, SoundscapeTouches,
    StartSound,
};


mod carry;
mod effects;
mod physics_sync;
mod spawn;
#[cfg(test)]
mod tests;

pub use carry::*;
pub use effects::*;
pub use physics_sync::*;
pub use spawn::*;

/// The running logic world of the loaded map.
#[derive(Resource)]
pub struct Logic {
    pub world: LogicWorld,
    /// Meters per entity unit.
    pub scale: f32,
    /// Mover entity -> its ECS node.
    pub nodes: Vec<(EntId, Entity)>,
    /// Prop entity (`classes::Class::Prop`) -> its prop node
    /// (`map::PropEntity`).
    pub props: Vec<(EntId, Entity)>,
    /// Server settings a map changed, with their values before (put back
    /// when another map loads).
    pub restore: Vec<(String, String)>,
    /// The `MapEntities` this world was built from.
    source: std::sync::Arc<Vec<crate::map::MapEntity>>,
    /// `core::RoundRestarts` this world has seen.
    restarts: u32,
    /// The map's placed weapons (`weapon_*` entities), by index.
    weapons: Vec<usize>,
    /// What `sync_movers` saw of each mover at the last phase.
    movers_seen: MoverCache,
}

/// Where the logic runs in the fixed tick.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum LogicSet {
    /// Thinks, movers, +use (before movement).
    Pre,
    /// Touches, untouch, the event queue (after movement).
    Post,
    /// Damage dealt to breakables this tick (after weapons).
    Damage,
}

// A one-shot sound map logic played here: `map::LogicOneShot` (the
// network forwards it without naming the logic), at its old path.
pub use crate::map::LogicOneShot;

/// Play a one-shot logic sound here and tell clients (`LogicOneShot`).
fn logic_sound(world: &mut World, sound: PlaySound) {
    world.write_message(LogicOneShot(sound.clone()));
    world.write_message(sound);
}

pub struct LogicPlugin;

/// The map's hurting trigger volumes in engine space (m), for spawns to
/// keep out of (`map::SpawnHazards`).
fn hurt_hazards(world: &mut World) -> Vec<crate::core::MapBrush> {
    let Some(logic) = world.get_resource::<Logic>() else { return Vec::new() };
    let s = logic.scale;
    // Entity space (x, y, z up) to engine (x, z, -y), times the scale.
    let to_engine = |v: Vec3| Vec3::new(v.x, v.z, -v.y) * s;
    super::triggers::hurt_zones(&logic.world)
        .into_iter()
        .map(|b| crate::core::MapBrush {
            planes: b.planes.iter().map(|(n, d)| (to_engine(*n) / s, d * s)).collect(),
            min: to_engine(Vec3::new(b.min.x, b.max.y, b.min.z)),
            max: to_engine(Vec3::new(b.max.x, b.min.y, b.max.z)),
            ladder: false,
            surface: None,
        })
        .collect()
}

impl Plugin for LogicPlugin {
    fn build(&self, app: &mut App) {
        use crate::map::SpawnHazardAppExt;
        app.add_spawn_hazards(hurt_hazards);
        // The logic's part of the network (movers, HUD messages).
        super::net::plugin(app);
        // Its entities for tools (the client's console and debug UI).
        app.insert_resource(crate::map::logic_view::LogicInspect {
            names: |w| {
                let logic = w.get_resource::<Logic>()?;
                let ids = logic.world.ids().into_iter();
                Some(ids.filter_map(|id| logic.world.get(id)).map(|e| (e.targetname.clone(), e.classname.clone())).collect())
            },
            entities: inspect_entities,
            summary: |w| w.get_resource::<Logic>().map(|l| (l.world.ids().len(), l.world.round)),
            fired: inspect_fired,
            messages: |w, n| {
                w.get_resource::<Logic>()
                    .map_or_else(Vec::new, |l| l.world.log.iter().rev().take(n).cloned().collect())
            },
        });
        app.init_resource::<HudMessages>()
            .init_resource::<ScreenFades>()
            .add_message::<HudEvent>()
            .add_message::<super::MapSay>()
            .add_message::<super::MapShake>()
            .add_message::<crate::map::SparkBurst>()
            .add_message::<crate::core::ScoreChange>()
            .add_message::<PlaySound>()
            .add_message::<LogicOneShot>()
            .add_message::<SoundControl>()
            .add_message::<SpawnGibs>()
            .add_message::<GlassShatter>()
            .add_message::<GlassImpact>()
            .add_message::<FallingPane>()
            .configure_sets(
                FixedUpdate,
                (
                    LogicSet::Pre
                        .after(SimSet::Rules)
                        .after(crate::core::apply_map_controls)
                        .before(SimSet::Movement),
                    LogicSet::Post
                        .after(SimSet::Movement)
                        .after(crate::map::entities::AnchorSet)
                        .before(SimSet::Weapons),
                    LogicSet::Damage.after(SimSet::Weapons),
                )
                    // Map logic is the server's.
                    .run_if(crate::core::authoritative),
            )
            .init_resource::<LogicRecord>()
            .add_systems(FixedUpdate, (load, apply_record, pre).chain().in_set(LogicSet::Pre))
            .add_systems(FixedUpdate, post.in_set(LogicSet::Post))
            // One after another: each takes the logic (`fire_outputs` after
            // `damage` as before; the set pieces' three after it).
            .add_systems(
                FixedUpdate,
                (damage, fire_outputs, fire_output_values, fire_keyed_outputs, take_ai_sounds)
                    .chain()
                    .in_set(LogicSet::Damage),
            )
            .add_message::<crate::map::entities::FireEntityOutput>()
            .add_message::<crate::map::entities::FireEntityOutputValue>()
            .add_message::<crate::map::entities::MapEntityInput>()
            .add_message::<crate::map::entities::FireKeyedOutput>()
            .add_message::<crate::map::entities::MapEntitySpawned>()
            .add_message::<crate::map::entities::MapEntityRemoved>()
            .add_message::<crate::map::entities::AiSound>()
            // Physics impacts on props and players (after the physics
            // step; the damage lands next tick).
            .add_message::<CollisionStart>()
            .add_message::<BreakProp>()
            .add_message::<Explosion>()
            .add_message::<crate::map::prop_physics::PropAwakened>()
            .init_resource::<PreStep>()
            .init_resource::<Impacts>()
            .init_resource::<PlayerImpactScale>()
            .add_systems(
                FixedPostUpdate,
                record_velocities.before(PhysicsSystems::StepSimulation),
            )
            .add_systems(FixedPostUpdate, (impacts, stress).chain().after(PhysicsSystems::Writeback))
            .add_systems(
                FixedPostUpdate,
                follow_bodies.after(PhysicsSystems::Writeback).run_if(crate::core::authoritative),
            );
        app.console_command(
            "ent_fire",
            "ent_fire <target> <input> [value]: send a map entity an input (names, wildcards, classnames), \
             the local player as activator.",
            |w, a| {
                let (Some(target), Some(input)) = (a.first(), a.get(1)) else {
                    return Err("ent_fire <target> <input> [value]".into());
                };
                let value = match a.get(2..).filter(|v| !v.is_empty()) {
                    Some(v) => super::Value::Str(v.join(" ")),
                    None => super::Value::Void,
                };
                let local = w
                    .query_filtered::<Entity, With<LocalPlayer>>()
                    .iter(w)
                    .next()
                    .map(super::world::Who::Player);
                let mut logic = w.get_resource_mut::<Logic>().ok_or("no map logic loaded")?;
                logic.world.queue_input(target, input, value.clone(), 0.0, local);
                info!("ent_fire {target} {input} {value:?}");
                Ok(None)
            },
        );
        app.console_command(
            "ent_dump",
            "ent_dump <target>: print map entities' logic state (class, place, a mover's pose and \
             speed, parent link), entity space.",
            |w, a| {
                let Some(target) = a.first() else {
                    return Err("ent_dump <target>".into());
                };
                let logic = w.get_resource::<Logic>().ok_or("no map logic loaded")?;
                let lines: Vec<String> = logic
                    .world
                    .resolve(target, None, None)
                    .into_iter()
                    .take(32)
                    .map(|who| logic.world.describe(who))
                    .collect();
                Ok(Some(if lines.is_empty() {
                    format!("no entity '{target}'")
                } else {
                    lines.join("\n")
                }))
            },
        );
        crate::console::resource_cvar::<LogicRecord, u8>(
            app,
            "chimera:logic_record",
            "1: record the outputs map entities fire and the inputs they get (the latest few hundred; \
             the debug UI's Logic tab lists them).",
            |r| &mut r.0,
        );
    }
}

