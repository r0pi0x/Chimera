//! Source player movement, from specs/cs_source/movement.md (lift S6 of
//! docs/plans/active/hl2dm.md: CS:S's, now every Source game's): a
//! swept-box kinematic controller. Friction and acceleration toward a wish
//! direction, air acceleration with the 30-unit cap that makes
//! air-strafing work, split gravity, jumping, ground detection, slide
//! moves with crease handling, stair stepping, sticking to slopes and
//! stairs, and ducking.
//!
//! The files (docs/plans/active/shared-movement.md, the layout the lift
//! into Chimera keeps): `config` (the config, its cvars by name, the
//! configs by movement id), `state` (`SourceMovement`), `hooks` (what a
//! game sets before the step: `MoveSpeed`, `MoveTraits`, `MoveStance`,
//! `StepSounds`; `SourceSpeed`, `SourceTraits` and `SourceStance` are
//! their names here), `units`, `trace` (the box sweep),
//! `mover` (one character's tick, in parts: ground, air, duck, jump,
//! water, ladders, falls, steps, stuck), `step` (the system),
//! `useable_ladders` (ladder lines), `shadow`, `pushaway`, `drown`, and
//! `plugin` (the systems and a movement's registration:
//! `SourceMovementPlugin`, `QuakeMovementAppExt`): the part Chimera gets,
//! which names nothing of this file's or `source_ladders`'
//! (`architecture::source_movement_core_names_no_wrapper`). Source's own
//! is here and in `source_ladders`: `SourceMovementAppExt::
//! source_movement` (Source's cvars beside the shared ones, its key
//! impulse), the hooks' Source names, and the `func_useableladder` reader.
//!
//! Each Source game registers it under its own id with its own config
//! (`SourceMovementAppExt::source_movement`: `cs_source:movement` with
//! CS:S's measured values, `hl2dm:movement` with HL2:DM's, each in
//! `SourceMovementConfigs` by its id; the movement itself has no game's
//! defaults, only the shared code's `SourceMovementConfig::reference`),
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
//! tick's max speed before the step (`SourceSpeed`, ordered before
//! `SourceMovementStep`: HL2:DM's walk, run and sprint), and a
//! character's own kind its speed, eyes and air jumps (`SourceTraits`:
//! TF2's classes), and a game's own stance beyond standing and ducking
//! its box, eye, speed and jump (`SourceStance`: DoD:S's prone, held as a
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
mod plugin;
pub mod pushaway;
pub mod shadow;
mod source_ladders;
mod state;
mod step;
mod trace;
mod units;
pub mod useable_ladders;

pub use config::{MovementConfigs, QUAKE_CVARS, SourceMovementConfig, SourceMovementConfigs, config_of};
pub use hooks::{MoveSpeed, MoveStance, MoveTraits, StepSounds};
/// The hooks under Source's names (the Source games' code).
pub use hooks::{MoveSpeed as SourceSpeed, MoveStance as SourceStance, MoveTraits as SourceTraits};
pub use mover::falls::punch_spring;
pub use plugin::{QuakeMovementAppExt, SourceMovementPlugin, SourceMovementStep};
pub use source_ladders::ladders_of;
pub use state::{SettleOnGround, SourceMovement};
pub use units::{to_engine, to_source};
pub use useable_ladders::{EntityLadders, ForcedMove, LadderFrom, UseableLadder, UseableLadders};

use bevy::prelude::*;

/// Source's own console variables, beside the shared ones (`QUAKE_CVARS`).
pub const SOURCE_CVARS: &[(&str, &str)] = &[
    ("sv_enablebunnyhopping", "1 removes the jump speed cap"),
    ("sv_autobunnyhopping", "1: holding jump keeps jumping"),
    ("sv_footsteps", "0 silences footsteps, jump and landing sounds"),
    ("cl_forwardspeed", "what a held move key sends"),
    (
        "sv_ladder_dampen",
        "sideways ladder input kept when climbing (CS:S 0.2)",
    ),
    (
        "sv_ladder_angle",
        "how squarely into a ladder the dampening starts (CS:S -0.707)",
    ),
];

/// Every console variable of a Source movement: the shared ones, then
/// Source's (what a Source game tells the shell its movement's cvars are).
pub const CVARS: &[(&str, &str)] = &{
    let mut all = [("", ""); QUAKE_CVARS.len() + SOURCE_CVARS.len()];
    let mut i = 0;
    while i < all.len() {
        all[i] = if i < QUAKE_CVARS.len() { QUAKE_CVARS[i] } else { SOURCE_CVARS[i - QUAKE_CVARS.len()] };
        i += 1;
    }
    all
};

/// Register a Source movement.
pub trait SourceMovementAppExt {
    /// Movement `id` on the shared movement (`QuakeMovementAppExt::
    /// quake_movement`) as a Source game's: with its `config` (None: the
    /// reference values, `SourceMovementConfig::reference`; one set for
    /// `id` before, by `SourceMovementConfigs::set`, stays), its console
    /// variables (`CVARS`: the shared ones and Source's) under `namespace`
    /// (`hl2dm:sv_gravity`; None: bare names, a Source game's own by the
    /// console's rule: CS:S's), Source's key impulse and the map's
    /// `func_useableladder`s read (`source_ladders`).
    fn source_movement(
        &mut self,
        id: &'static str,
        config: Option<SourceMovementConfig>,
        namespace: Option<&'static str>,
    ) -> &mut Self;
}

impl SourceMovementAppExt for App {
    fn source_movement(
        &mut self,
        id: &'static str,
        config: Option<SourceMovementConfig>,
        namespace: Option<&'static str>,
    ) -> &mut Self {
        // Its move keys count half on their first command (Source's key
        // impulse, measured on real CS:S, HL2:DM and TF2).
        self.world_mut().get_resource_or_init::<crate::core::HalfFirstPress>().0.push(id);
        if !self.is_plugin_added::<source_ladders::SourceLaddersPlugin>() {
            self.add_plugins(source_ladders::SourceLaddersPlugin);
        }
        self.quake_movement(
            id,
            config.unwrap_or_else(SourceMovementConfig::reference),
            &[QUAKE_CVARS, SOURCE_CVARS],
            namespace,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `CVARS` is the two lists in order, and the config knows each name.
    #[test]
    fn cvars_are_the_shared_ones_then_sources() {
        assert_eq!(CVARS.len(), 15);
        assert!(CVARS.iter().eq(QUAKE_CVARS.iter().chain(SOURCE_CVARS)));
        assert_eq!((CVARS[0].0, CVARS[8].0, CVARS[9].0, CVARS[14].0), ("sv_accelerate", "sv_bounce", "sv_enablebunnyhopping", "sv_ladder_angle"));
        let c = SourceMovementConfig::reference();
        assert!(CVARS.iter().all(|(name, _)| c.get_cvar(name).is_some()));
    }
}
