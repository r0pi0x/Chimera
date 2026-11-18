//! Core vocabulary shared by every game plugin. Keep this small: a concept
//! moves in here only after two or three games need it (README, "rule of
//! three").
//!
//! `Intent` (with its command number, stamped each tick in `SimSet::Commands`:
//! the tick offset by the character's `Seed`; `Intent::actions` carries a
//! game's named actions, `ActionRegistry`/`ActionAppExt::intent_action`),
//! `Velocity`, `MovementState`
//! (with the hull, the moving solid stood on and whether it is on a ladder),
//! `BaseVelocity`, `EntityGravity`, `MovingSolid`, `Health`, `Damage`/`Died`
//! messages (damage applied after weapons, `Died` carries the killing `Damage`;
//! `DamageKind`: bullet, melee, blast...; `Damage::weapon` names a thrown
//! weapon for kill notices; `Damage::force` the push that came with it),
//! `MapDamage` (a map's damage to a player before armour and whole points),
//! `Explosion` (radius damage asked for by map logic or rules: exploding props,
//! the bomb; the weapon it counts as), `Blinded` (a flash's white: hold and
//! fade), `SightBlocker` (a sphere sight doesn't pass: smoke), `Damageable`
//! (takes `Damage` without `Health`: breakables), `Hitgroup` and the
//! finer `BodyRegion` (with `BodyPart`, what armour covers), `Hitbox`/`Hitboxes`
//! (boxes or spheres with their region, and the shape queries every gun's
//! rule uses: docs/DESIGN.md "Hits across games"; `LazyHitboxes`, a
//! game's made only when a shot asks, read with `Hitboxes` through
//! `HitShapes`, with the box the game says holds them whatever they turn
//! out to be (`HitBound`: a ray that misses it leaves them unmade), physics layers (`RAGDOLL_LAYER`, also prop pieces;
//! `ITEM_LAYER` for loose items, which slide on their own copy of the
//! displacement mesh with its internal edges fixed (`ITEM_GROUND_LAYER`);
//! gameplay queries mask with `SOLID_LAYERS`, characters' bodies and movement
//! with `CHARACTER_FILTER`, which leaves loose items out), `MaxSpeed`
//! (equipment speed cap), `God`, `SpawnProtected` (spawn protection's
//! windows: attacks pass through the player, hits on it show nothing), `Team`, `Seed` (a character's own dice seed
//! for rolls that must repeat run to run: bots' choices and route noise,
//! buying, bullet spread; never entity ids, which shift with every spawn,
//! resources included), `Radio` (a team radio call: sender and the game's
//! command name), `SpawnPoint`, `LocalPlayer`, `UserInfo` (a player's own
//! settings the simulation reads, Source's userinfo: the local player's from
//! its console, a remote player's from its client), `PlayerIdMode`
//! (`mp_playerid`), `SimTick`, `SimClock` (the step's time and length, Source's
//! curtime: what selection, movement and the weapon frame read instead of
//! `Time`, so a replayed command sees its own time), `set_tick_length` (a new
//! map's tick: the fixed clock keeps running except on a client, so its time
//! stays with the frame's `Time<Virtual>`), `NetRole` (standalone, server or
//! client) and the `authoritative` run condition on server-only systems (bots,
//! rules, damage, logic, objectives), the predicted path for network play
//! (docs/plans/active/multiplayer.md): `Predict` schedules (`Select`,
//! `Movement`, `Weapons`, each run from its place in `FixedUpdate` by
//! `run_predicted`, or all in order by `predict` for a replayed command),
//! `FirstTimePredicted` (side effects only the first time a command runs),
//! `PredictedComponents` (components registered with
//! `PredictedAppExt::predicted`, saved and restored per entity; `predicted_net`
//! and `predicted_codec` parts also go to the client predicting them as one
//! blob), `RoundRestarts` (counted up by the rules at each new round,
//! `restart_round`, which triggers `RoundRestart`; the logic and map layers
//! put the map's entities back), `FreezeTime` (the rules hold
//! everyone still: a round's freeze time), the `SimSet` tick ordering (`Rules`,
//! `Commands`, `Movement`, `Weapons`), `keep_moved_positions` (before each
//! physics step a character's physics position is its movement's, to the bit:
//! the physics would undo steps below its 0.01 mm tolerance while the body is
//! awake); the collision world maps share with movement:
//! `MapBrush`/`MapBrushes` (brush planes for exact swept-box movement, ladder
//! flag), `MapPlayerClips` (which of them stop players only, and their
//! players-only collider kept in step, `publish_player_clips`), `MapBrushTree` (the BSP tree over the world's brushes: which of two
//! faces hit at the same distance a trace reports), `MapBrushCollider`,
//! `MapTerrain` (terrain triangles as thin brushes, with a grid) and
//! `MapTerrainCollider`, `MapWater` volumes; `Equip` (a map gives a character
//! items), `DamageFilter` (damage kinds a character ignores: a map's no-fall
//! filter); what map entities do to a character's controls (`MapControls`:
//! player_speedmod's movement clock (`time_scale`, read by the movement) and
//! taken buttons, game_ui's "at controls", a camera's freeze, weapons put away,
//! invulnerable; applied to its `Intent` by `apply_map_controls` after the
//! rules, on the server and on the client predicting it: a predicted part; a
//! respawn resets the clock and the camera), `MapView` (the map camera a
//! character views through), `Carried` (a character carried by another
//! entity, a vehicle's seat: every movement skips it, it makes no steps:
//! `Afoot`), `buttons` (Source's command button bits),
//! `ScoreChange` (game_score, applied by the rules); the players' standing
//! every game has: `Score`, `Dead`, `MapLoads`, `Bot` (a computer player),
//! `Npc` (a character that isn't a player: hostages), `ScoreMark` (what a
//! carried item marks on the scoreboard); `BuildVersion` (the app's stamp);
//! `DEFAULT_TICK_HZ`; what the client shell reads of a game's rules without
//! naming them: `JoinChoice` (how a player comes into a game), `MapCycle`,
//! `FadeToBlack`, `RespawnDue` (when the dead local player comes back, for
//! rules that respawn on a timer); the rules' `NewGame` hook is the
//! gameplay's (`gameplay::new_game`).
//!
//! `BrushGrid`: a grid over brushes' bounds, the Source movement's broad
//! phase over `MapBrushes` (identical results to testing every brush).

use bevy::prelude::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// The version text's format (`format_version`); the build's own
/// `VERSION` is stamped where the app is built (`mashup::core::VERSION`),
/// so a commit doesn't recompile this crate.
pub mod version;


mod brush_grid;
mod character_state;
mod damage;
mod hitbox;
mod intent;
mod map_controls;
mod prediction;
#[cfg(test)]
mod tests;

pub use brush_grid::*;
pub use character_state::*;
pub use damage::*;
pub use hitbox::*;
pub use intent::*;
pub use map_controls::*;
pub use prediction::*;

/// Ordering of the fixed-tick simulation. Game plugins put their systems in
/// these sets so that, e.g., weapons always see this tick's movement state.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimSet {
    /// Match rules (rounds, respawns), before everything that acts on
    /// them this tick.
    Rules,
    /// Intents are final for the tick: each gets its command number.
    Commands,
    Movement,
    Weapons,
}

/// Stamp each intent with this tick's command number: the tick
/// (`SimClock::tick`; a network client's is the server tick its command
/// is for), offset by
/// the character's `Seed` (Source clients number their commands from
/// their own start, so two players firing on the same tick don't share a
/// spread pattern). Never from entity ids.
pub fn number_commands(clock: Res<SimClock>, mut intents: Query<(&mut Intent, Option<&Seed>)>) {
    for (mut intent, seed) in &mut intents {
        let n = (clock.tick as u32).wrapping_add(seed.map_or(0, |s| s.0 as u32));
        if intent.command != n {
            intent.command = n;
        }
    }
}

/// Count the tick and set the clock (`SimClock`) from `Time<Fixed>` (a
/// network client then sets it to the server tick it predicts,
/// `net::predict`).
pub fn start_tick(mut tick: ResMut<SimTick>, mut clock: ResMut<SimClock>, time: Res<Time<Fixed>>) {
    tick.0 += 1;
    *clock = SimClock {
        tick: tick.0,
        now: time.elapsed_secs_f64(),
        delta: time.delta(),
    };
}

/// Before each physics step: the physics' copy of a character's position
/// (`Position`) is where its movement put it, to the bit. The physics
/// takes a moved transform only when it moved more than its tolerance
/// (0.01 mm), and after the step writes its copy back over the transform
/// of every body that is awake: a smaller step of the movement's (the
/// metres-to-units round trip moves a player put at a spawn point by a
/// float step or two) was undone while the body was awake and kept while
/// it slept (half a second after its last bigger move), and kept in a
/// network client's replay, which runs no physics. So the outcome of a
/// tick hung on the physics' sleep state, which a server and its client
/// don't share (found by the soak: a respawned player mispredicted by
/// micrometres on each tick for half a second; games/cs_source/tests/it/net_prediction.rs,
/// `a_float_step_below_the_physics_tolerance_is_predicted`).
///
/// A change marks it changed, as the physics' own sync would (it wakes
/// the body and moves its collider).
pub fn keep_moved_positions(
    mut bodies: Query<
        (&Transform, &mut avian3d::prelude::Position),
        (With<avian3d::prelude::CustomPositionIntegration>, Without<ChildOf>),
    >,
) {
    for (t, mut p) in &mut bodies {
        if p.0.to_array().map(f32::to_bits) != t.translation.to_array().map(f32::to_bits) {
            p.0 = t.translation;
        }
    }
}

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActionRegistry>()
            .register_type::<Intent>()
            .register_type::<Velocity>()
            .register_type::<MovementState>()
            .register_type::<Health>()
            .register_type::<Team>()
            .register_type::<Unassigned>()
            .register_type::<ChoosingClass>()
            .register_type::<PlayerClass>()
            .register_type::<UserInfo>()
            .register_type::<Seed>()
            .register_type::<MaxSpeed>()
            .register_type::<BaseVelocity>()
            .register_type::<EntityGravity>()
            .register_type::<Hitboxes>()
            .register_type::<Damageable>()
            .add_message::<Damage>()
            .add_message::<MapDamage>()
            .add_message::<Equip>()
            .add_message::<Explosion>()
            .add_message::<Deafened>()
            .add_message::<Died>()
            .add_message::<Destroyed>()
            .add_message::<RegionHit>()
            .add_message::<DamageTaken>()
            .add_message::<Radio>()
            .init_resource::<FriendlyFire>()
            .add_systems(FixedUpdate, apply_damage.after(SimSet::Weapons).run_if(authoritative))
            .add_systems(
                FixedUpdate,
                map_damage_lands
                    .after(SimSet::Weapons)
                    .before(apply_damage)
                    .run_if(authoritative.and_then(not(resource_exists::<MapDamageArmor>))),
            )
            .register_type::<SpawnPoint>()
            .register_type::<LocalPlayer>()
            .register_type::<Connecting>()
            .register_type::<Spectating>()
            .register_type::<SimTick>()
            .init_resource::<SimTick>()
            .init_resource::<SimClock>()
            .init_resource::<NetRole>()
            .init_resource::<FirstTimePredicted>()
            .init_resource::<PredictedComponents>()
            .add_systems(FixedFirst, start_tick)
            .init_resource::<RoundRestarts>()
            .init_resource::<MapLadders>()
            .add_systems(First, (publish_brush_ladders, publish_player_clips))
            .init_resource::<FreezeTime>()
            .init_resource::<RespawnDue>()
            .configure_sets(
                FixedUpdate,
                (SimSet::Rules, SimSet::Commands, SimSet::Movement, SimSet::Weapons).chain(),
            )
            .add_systems(FixedUpdate, number_commands.in_set(SimSet::Commands))
            .add_message::<ScoreChange>()
            .add_systems(
                FixedUpdate,
                apply_map_controls.after(SimSet::Rules).before(SimSet::Commands),
            )
            .predicted_net::<MapControls>()
            .add_systems(FixedUpdate, run_predicted(Predict::Movement).in_set(SimSet::Movement))
            .add_systems(
                FixedPostUpdate,
                keep_moved_positions.before(avian3d::prelude::PhysicsSystems::Prepare),
            )
            .predicted_net::<Transform>()
            .predicted_net::<Velocity>()
            .predicted_net::<MovementState>()
            .predicted_net::<BaseVelocity>()
            .predicted_net::<MaxSpeed>();
        // On every character from the spawn (`character::CharacterPartsExt::
        // on_every_character`'s way, which this layer can't name).
        app.register_required_components::<Intent, MaxSpeed>();
        for stage in Predict::ALL {
            app.init_schedule(stage);
        }
    }
}

// Collision world shared by maps and movement: exact brushes for swept-box
// movement, and water volumes.

