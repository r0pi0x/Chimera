//! Player movement of the Quake lineage, from specs/cs_source/movement.md
//! (Source's, lifted from CS:S's and then from `source_engine`): a
//! swept-box kinematic controller. Friction and acceleration toward a wish
//! direction, air acceleration with the 30-unit cap that makes
//! air-strafing work, split gravity, jumping, ground detection, slide
//! moves with crease handling, stair stepping, sticking to slopes and
//! stairs, and ducking.
//!
//! It is Chimera's (docs/plans/active/shared-movement.md): the movement of
//! the Quake lineage, which Source's games run through their wrapper
//! (`source_engine::source_movement`: `SourceMovementAppExt::
//! source_movement`, Source's cvars beside the shared ones, its key
//! impulse, its `func_useableladder` reader, and the Source names of the
//! types here) and a GoldSrc game will run with its own config. The
//! files: `config` (the config, its cvars by name, the configs by
//! movement id), `state` (`QuakeMovement`), `hooks` (what a game sets
//! before the step: `MoveSpeed`, `MoveTraits`, `MoveStance`,
//! `StepSounds`), `units`, `trace` (the box sweep; `trace::hulls`: a map's
//! clip hulls, `ClipHulls` and `MapClipHulls`, traced in its place by a
//! movement that asks for them), `mover` (one
//! character's tick, in parts: ground, air, duck, jump, water, ladders,
//! `lines` (ladder lines), falls, steps, stuck), `step` (the system),
//! `shadow`, `pushaway` (with `props`, the service any movement uses to
//! meet physics props by the map's rule: Combat Arms' and Halo's too),
//! `drown`; this file is the root: the systems as
//! one plugin (`QuakeMovementPlugin`), the set games order their own
//! per-tick choices against (`MovementStep`), and a movement's
//! registration (`QuakeMovementAppExt::quake_movement`: its id, config,
//! console variables by list and namespace, its envelope and its seat).
//!
//! GoldSrc's rules are switches of the config, off in `reference()` and
//! for every Source game (docs/plans/active/shared-movement.md, S7-S9 and
//! the hand-off): the world traced through a map's clip hulls
//! (`clip_hulls`), commands of whole milliseconds (`command_msec`), edge
//! friction (`edge_friction`), and the smaller ones a GoldSrc game's spec
//! names (`ground_snap`, `unduck_at_once`, `duck_ends_on_time`,
//! `jump_sets_speed`, `bunnyhop_cut`, `modifier_per_tick`, `punch_decay`,
//! `clip_stop_epsilon`, `rising_friction`, `water_jump_up`,
//! `walk_scales_keys`); CS 1.6 sets them (`cs16::game::movement`).
//!
//! Each Source game registers it under its own id with its own config
//! (`SourceMovementAppExt::source_movement`: `cs_source:movement` with
//! CS:S's measured values, `hl2dm:movement` with HL2:DM's, each in
//! `QuakeConfigs` by its id; the movement itself has no game's
//! defaults, only the shared code's `QuakeConfig::reference`),
//! and its cvars under its namespace; a character's config follows its
//! `MovementSlot` (`MovementConfigs`). What differs between the games is
//! config: CS:S's stamina, bunny-hop cap, walk scale, ladder dampening and
//! measured hulls; HL2's fall thresholds and fixed or fractional fall
//! damage, the swim lift of the player's own speed, footsteps of the
//! character's model (`StepSounds`, none while ducked), climbing
//! `func_useableladder`s instead of ladder brushes (`useable_ladders`:
//! the map's ladders, mounting, forced moves, climbing and dismounts), and
//! no CS:S drowning; TF2's box width, 0.2 s duck counted from the press's
//! tick, bunny-hop cap as a share of the player's top speed, the ground
//! speed clamp, the backpedal clamp, the raised acceleration of slow
//! wishes, the 250 units/s ground leave, air jumps (the Scout's), no jump
//! fully ducked, the air-duck limit by releases, the re-duck delay, no
//! duck in water and fall damage as a share of max health with its
//! spread. A game may choose the
//! tick's max speed before the step (`MoveSpeed`, ordered before
//! `MovementStep`: HL2:DM's walk, run and sprint), and a
//! character's own kind its speed, eyes and air jumps (`MoveTraits`:
//! TF2's classes), and a game's own stance beyond standing and ducking
//! its box, eye, speed and jump (`MoveStance`: DoD:S's prone, held as a
//! lower ducked box; leaving it stands up where there is room, else stays
//! ducked until there is, or lands ducked as if duck were held: a crouch;
//! a stance held is no duck for the input's scale), and an unduck that
//! counts its release tick (`unduck_counts_first_tick`: DoD:S's). A game's push (`core::ExternalPush`, TF2's knockback)
//! is honoured as every movement does: a lift takes the body off the
//! ground as logic does (`BaseVelocity::unground`); its ground limit cuts
//! the horizontal speed, after the first grounded tick's friction, to
//! that share of the tick's max speed (the held weapon's, the game's or
//! the class's); a push that took the footing (TF2's airblast) waits for
//! it, slides at its share of the friction, keeps any upward speed in
//! the air and accelerates in the air at its share until it lands.
//!
//! The maths runs in Source units (inches, Z up) like the spec; the
//! character's `Transform` (engine meters, Y up) is the centre of the
//! standing box, so feet = origin - 36 units. Ladders and water (swimming,
//! water jumps) follow the spec's sections; other games' liquid and ladder
//! volumes (`WaterZone`, `LadderZone`: Combat Arms' maps') count as the
//! map's water and ladder brushes (a ladder zone is swept as a solid); a
//! movement whose game has no ladders (TF2) climbs them on another game's
//! map all the same (`climbs_contents`); for
//! a movement climbing contents ladders on another game's map, the map's
//! ladder lines (`core::MapLadders::lines`, HL2's) are thin solid ladder
//! posts (docs/DESIGN.md "Ladders across games"). Player clips are among
//! the brushes swept; another game's players-only surfaces (Combat Arms'
//! collision blockers, `core::PLAYER_CLIP_LAYER`) stop it through its
//! physics casts ("Clips across games"). Not implemented yet: base
//! velocity (conveyors, water currents). Landings and wall slams roll the
//! view (the punch angle's roll, `MovementState::view_roll`); hard
//! landings deal fall damage (specs/cs_source/fall_damage.md) and play
//! its sound. A bullet hit's velocity modifier (CS:S, weapons.md M18,
//! set by `hit_reaction`) multiplies the max speed and recovers by 1/3 a
//! second after each move. A head under water too long drowns
//! (`drown`, from the water level kept here). Footsteps play the
//! surface underfoot's steps, or a footstep volume's replacement
//! (func_footstep_control, `content::sound::step_remaps`). A character with
//! `SettleOnGround` (hostages) goes down onto the floor its ground test
//! finds (players keep the gap of up to 2 units, as Source's do).
//! NPCs (`core::Npc`: CS:S's hostages) block every player whatever
//! `chimera_noblock` says.
//!
//! Known shortcuts (docs/tech-debt.md): Two departures from Source's box trace,
//! made so surfing works (`Tracer::sweep_brushes`): the plane a box stops
//! against is the one it really crosses last, not the last by backed-off
//! fraction (Source's choice picks a curved ramp's seam bevel, facing back
//! along the ramp: a "rampbug" stop, which surf servers patch out with plugins;
//! kept always on, no cvar, by the user's decision), and moves go in as a start
//! and a delta (exact parallel tests far from the origin). Not compared with
//! CS:S tick for tick; the movecmp-derived tests and dust2/nuke walking pass
//! unchanged. Why it's OK for now: Surfing stopped dead on most surf_sedona
//! ramps; a movecmp surf scenario would tell how often CS:S itself rampbugs.
//!
//! Known shortcuts (docs/tech-debt.md): Source movement
//! (specs/cs_source/movement.md) lacks base velocity (conveyors, water
//! currents); its landing roll keeps the movement spec's spring (the shared
//! code's), on roll alone, apart from the weapons' punch (recoil and the hit
//! flinch, `weapon::ViewPunch`, which decay by CS:S's measured linear rule;
//! weapons.md M18 says landings decay that way too, not checked). A landing
//! out of water counts `floating_bonus` less on a floating body
//! (`map::buoyancy::Floating`: one the water lifts, our reading of
//! "floating") and adds a descending ground's speed (at least 0.1); the
//! ground material's maxspeedfactor and jumpfactor apply while on the
//! ground (in the air: 1); a map's `IgnoreFallDamage` window
//! (`core::IgnoreFallDamage`) spares fall damage. CS:S values, hulls, ducking, stamina and the bunny-hop cap are
//! measured with movecmp/the probe. Not modelled: the bots' automatic jump-duck
//! (bots only). An exactly 18-unit drop is at the edge of the stick-to-ground
//! reach (unmeasured). Max speed comes from the held weapon (`MaxSpeed`). Why
//! it's OK for now: Walking, stopping, jumping and crouch-walking match CS:S
//! tick for tick.
//!
//! Known shortcuts (docs/tech-debt.md): Ladder brush tops
//! (`games::cs_source::movement`, `LADDER_MAX_NORMAL_Z`): the ladder probe
//! ignores a ladder face whose normal is 0.7 or more vertical (a ladder brush's
//! top or bottom), our choice: the spec is silent, and taking the top for a
//! ladder (normal +z) stuck you there (the climb velocity along a vertical
//! normal left only sideways movement looking down, and a jump added the 270
//! jump-off push to the jump). Before: 36 ladder tops on the stock maps grabbed
//! you (`games/cs_source/tests/it/heavy/map_ladders.rs`), now 0. Confirmed in the CS:S client
//! (2026-10-10, feet at (1607, -114, -136) on de_train's ladder top, pitch 30
//! down): forward walks off the top, a jump is a normal 302 one. Climbing
//! ladder steps share the ground's step timer (sounds spec 3), so a short climb
//! right after a ground step (de_train's car ladders looking up, about 0.3 s)
//! hears none; CS:S's doing the same is unmeasured. Holding walk or duck
//! silences ladder steps too (our CS:S walk rule, sounds spec Q3; the spec's
//! ducked ladder step is 0.325). No CS:S surface is `climbable`, so climbable
//! materials (spec "What counts as a ladder") aren't read. Why it's OK for now:
//! Every stock ladder climbed long enough steps; tops walk and jump normally.
//!
//! Known shortcuts (docs/tech-debt.md): Source movement sweeps concave or
//! rounded props (182 of dust2's 295) with avian shape casts; world brushes,
//! convex props (hull within 1 cm of 75% of the surface, from the visible mesh,
//! not `.phy`) and displacement triangles (as 2-unit-thick solids with bevel
//! planes, `core::MapTerrain`; CS:S's own displacement collision isn't public)
//! are swept exactly. A static prop set to collide by its physics model ("Use
//! VPhysics") whose model has none blocks nothing (our reading of the public
//! prop_static docs, not measured): 91 of surf_nebula's 230 props (Propper ramp
//! models over player-clip ramps; their meshes as colliders stopped 13 of 84
//! surf rides dead), 206 of kz_hikari_od_nh_v2's 485, 55 of
//! gg_fy_tactic_fight's, 44 of surf_halloween_tf2's. Why it's OK for now: Exact
//! on crates and boxes; terrain matches CS:S in movecmp (a crease's edge
//! bevels count, as in CS:S: movement.md open question 19). Of
//! brushes or triangles stopping a trace at the same back-off, the one
//! really reached first gives the normal (our rule, matching the probe's
//! dust2 walk; movement.md open question 19). Probe for the
//! prop rule: a map with a VPhysics static prop whose model has no `.phy`, walk
//! into it (`getpos` stops or passes).

mod config;
pub mod drown;
mod hooks;
mod mover;
pub mod props;
pub mod pushaway;
pub mod shadow;
mod state;
mod step;
mod trace;
mod units;

pub use config::{MovementConfigs, QUAKE_CVARS, QuakeConfig, QuakeConfigs, config_of};
pub use hooks::{MoveSpeed, MoveStance, MoveTraits, StepSounds};
pub use mover::falls::punch_spring;
pub use mover::lines::{self, EntityLadders, ForcedMove, LadderFrom, UseableLadder, UseableLadders};
pub use state::{SettleOnGround, QuakeMovement};
pub use step::command_msec;
pub use trace::hulls::{ClipHulls, HullContents, HullHit, MapClipHulls};
pub use units::{to_engine, to_units};

// `crate::core`, `crate::map`, `crate::slots` and `crate::console` inside,
// as in the crates the files came from.
use chimera_core::{console, core, slots};
use chimera_world::map;

use bevy::prelude::*;

use crate::{
    core::PredictedAppExt,
    map::MapScopedApp,
    mover::stuck::{footing, seat},
    slots::RegisterSlots,
    step::step,
};

/// The movement's systems, once per app (`QuakeMovementAppExt::
/// quake_movement` adds it): the step, the physics shadow, the push-away,
/// the drowning, the map's ladder lines, and the configs by movement id
/// (`QuakeConfigs`).
pub struct QuakeMovementPlugin;

impl Plugin for QuakeMovementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<QuakeConfigs>()
            .init_resource::<UseableLadders>()
            .init_resource::<EntityLadders>()
            .map_scoped::<MapClipHulls>()
            .add_message::<crate::map::PlaySound>()
            .add_message::<crate::map::entities::FireEntityOutput>()
            .add_systems(First, lines::build_ladders)
            .add_systems(crate::core::Predict::Movement, step.in_set(MovementStep))
            .predicted_net::<QuakeMovement>()
            .configure_sets(crate::core::Predict::Movement, MovementStep.before(crate::core::AfterMovements));
        props::add(app);
        app.add_plugins(drown::DrowningPlugin);
    }
}

/// The movement step: a game's own per-tick choices (HL2:DM's speed
/// selection, `MoveSpeed`) go before it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct MovementStep;

/// Register a movement on the shared movement.
pub trait QuakeMovementAppExt {
    /// Movement `id` on the shared movement (`QuakeMovementPlugin`, added
    /// once), with its `config` (one set for `id` before, by
    /// `QuakeConfigs::set`, stays) and the console variables of
    /// `cvars` (lists of (name, help): `config::QUAKE_CVARS` and the
    /// engine's own; each name one `QuakeConfig::set_cvar`
    /// knows), under `namespace` (`hl2dm:sv_gravity`; None: bare names).
    fn quake_movement(
        &mut self,
        id: &'static str,
        config: QuakeConfig,
        cvars: &[&'static [(&'static str, &'static str)]],
        namespace: Option<&'static str>,
    ) -> &mut Self;
}

impl QuakeMovementAppExt for App {
    fn quake_movement(
        &mut self,
        id: &'static str,
        config: QuakeConfig,
        cvars: &[&'static [(&'static str, &'static str)]],
        namespace: Option<&'static str>,
    ) -> &mut Self {
        if !self.is_plugin_added::<QuakeMovementPlugin>() {
            self.add_plugins(QuakeMovementPlugin);
        }
        let defaults = config;
        {
            let mut all = self.world_mut().get_resource_or_init::<QuakeConfigs>();
            if all.get(id).is_none() {
                all.0.push((id, defaults.clone()));
            }
        }
        for (name, help) in cvars.iter().flat_map(|list| list.iter()) {
            let name = *name;
            let full = match namespace {
                Some(ns) => format!("{ns}:{name}"),
                None => name.to_string(),
            };
            crate::console::ConsoleAppExt::console_cvar(
                self,
                &full,
                help,
                &defaults.get_cvar(name).unwrap_or_default(),
                move |w| w.get_resource::<QuakeConfigs>().and_then(|c| c.get(id)).and_then(|c| c.get_cvar(name)),
                move |w, v| {
                    w.get_resource_mut::<QuakeConfigs>()
                        .and_then(|c| c.into_inner().get_mut(id).map(|c| c.set_cvar(name, v)))
                        .ok_or("no Source movement")?
                },
            );
        }
        // The game's character's body, the envelope its maps carry
        // (`chimera_core::envelope`): the box's half width, heights, step.
        if let Some(game) = chimera_core::core::movement_game(id) {
            use chimera_core::envelope::MovementEnvelopeAppExt;
            self.add_movement_envelope(game, defaults.envelope());
        }
        self.register_movement::<QuakeMovement>(id).seat_movement(id, footing, seat)
    }
}
