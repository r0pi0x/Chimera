//! What TF2's guns look like going off (specs/tf2/effects.md,
//! particles.md), the same on every map, TF2's or another game's (its
//! particle files and impact looks come with its own sets:
//! `maps::add_guest_looks` loads them onto another game's map,
//! `maps::extra_particles` with its own). Each gun's effect names are its
//! data's (`[hooks."tf2:looks"]`, `LooksParams`):
//!
//! - **Muzzle flashes** (effects.md 2): each shot of a bullet gun or
//!   launcher (`Fired`) starts its muzzle system (`muzzle`) at the gun's
//!   `muzzle` attachment: on the first-person view model, the attachment
//!   re-projected into the world view (`map::attached::FirstPerson`,
//!   `view_model::reproject`: right and up x tan(world FOV/2) /
//!   tan(view-model FOV/2)) and drawn as a world system with the world
//!   camera (particles.md 6: never the view-model camera, which drew it
//!   1.96x too big at FOV 90), depth tested against the world (a flash
//!   inside a wall you stand against is hidden by it, as in TF2), riding
//!   the animated attachment (TF2's "Movement Lock to Control Point" on the
//!   view model's: the flash goes with the gun's kick) through its
//!   `attached::AttachmentMark` (`mark_attachments`: this machine's own
//!   player's `muzzle` and `eject_brass`), else following the eye it was
//!   started from (`StartParticleSystem::follow`); else the
//!   held model's (`HeldAttachment`: what others see), else a little ahead
//!   of the eye under the gun (`MUZZLE_OFFSET`). None while the local
//!   player's view model is switched off (`r_drawviewmodel 0`).
//! - **The Minigun** (effects.md 3): no per-shot flash; its
//!   `while_firing` systems (`muzzle_minigun_constant` at `muzzle`,
//!   `eject_minigunbrass` at `eject_brass`) start when it enters its firing
//!   state (`Tf2Minigun::state`, another player's from its `HeldState`)
//!   and stop emitting when it leaves it (`StopParticleSystems`; live
//!   particles finish); also while another player's rounds arrive
//!   (`RemoteShot`, `REMOTE_FIRING_HOLD`), turned with its aim.
//! - **Tracers** (effects.md 6): one counter shared by every bullet drawn
//!   (`TracerCount`, every gun with a tracer, every shooter): a bullet
//!   whose count before it is a multiple of its gun's `tracer_every` (2;
//!   the Minigun's 4) draws `<tracer>_red`/`_blue` by the shooter's team
//!   (`_crit` appended on a critical shot: crits are off), from the
//!   muzzle as the flash's (else the bullet's start) to the bullet's end:
//!   control point 1 (`StartParticleSystem::end`), which the system's
//!   "move particles between 2 control points" heads for at its speed.
//!   Another player's (one drawn from snapshots, `NetDrawn`) waits the
//!   interpolation delay (`tracer_hold`), leaving when the drawn body
//!   fires: real TF2 draws others' flashes at once and their tracers about
//!   7 frames (at 60) later.
//! - **Trails** (effects.md 1): a gun's `trail` (`FlyerTrail`) starts on
//!   each of its flyers as it appears (`ShownItem`: `rockettrail` on
//!   the rocket, `pipebombtrail_<team>`, `stickybombtrail_<team>`,
//!   `nailtrails_medic_<team>`), following it without carrying what it
//!   left, stopped when it is gone (`psys::Follow`); its `under_water`
//!   one instead when its first place is in water, never chosen again
//!   (effects.md 9: `rockettrail_underwater`). A flyer drawn late (its
//!   data's `interp`, effects.md 11: `map::loose::DrawnLate`) trails
//!   from its copy, drawn `LateDelay` in the past.
//! - **The armed pulse** (effects.md 1): a gun's `armed` (`ArmedLook`)
//!   starts on each of its flyers `after` seconds after it appears (the
//!   sticky's `stickybomb_pulse_<team>` at its arming, 0.8 s), following
//!   it.
//! - **The Rocket Launcher's backblast** (effects.md 4): `rocketbackblast`
//!   at the launcher's `backblast` attachment on each shot, for everyone
//!   but the shooter (the held model's: never on this machine's own
//!   player).
//! - **Explosions** (effects.md 5; `blast::Tf2Explosion`, sent by the
//!   server to every client): by the gun's `explosion` (the Stickybomb
//!   Launcher's for pipes and stickies), held for `LateDelay` (as the
//!   flyer drawn late reaches it, effects.md 11): `water` when the centre is in
//!   water, `player` in the air or on a player, else `wall`, oriented to
//!   the surface's normal (zero angles in the air); an unnamed one is
//!   `ExplosionCore_wall`. The server traces the `Scorch` decal
//!   (`blast`).
//! - **Flames** (flames.md 3; `flame_stream`): the gun's `stream`
//!   (`StreamLooks`): one `new_flame` per burst (`new_flame_crit_red`/
//!   `_blue` on a crit, `flamethrower_underwater` with the eye under
//!   water), started at the muzzle as the flashes are (control point 0)
//!   and stopped when no round came for `burst_gap`; its control points 1
//!   to 30 are the flame points as this machine runs them
//!   (`flame::FlameShown`: the same rounds, seeds and motion as the
//!   server's damage points), the newest the active one each new sprite
//!   rides, so the fire is where the damage is.
//! - **The laser dot** (weapons_more.md 3.1): a gun's `dot` where a
//!   scoped holder aims (`dot`).
//! - **Impacts**: Source's impact effects, decals and impact sounds with
//!   TF2's style (`STYLE`): TF2 is an impact provider, so its guns' hits
//!   are TF2's own on any map that carries its looks (`maps::
//!   add_impact_looks`), never the map's game's or the app's base game's
//!   (Combat Arms' in the party apps). Which entry a surface plays is the
//!   map's sound scripts' (a name the map has, such as CS:S's
//!   `Concrete.BulletImpact` on de_dust2, plays the map's).
//!
//! Known shortcuts: a trail starts at the flyer's origin, not its model's
//! `trail` attachment; no crit glows on flyers (crits are off); control point 0 of the stream is the muzzle as drawn
//! (the view model's re-projected in first person), not the third-person
//! model's (flames.md Open questions 2); crits are off, so a crit stream
//! never starts; no airblast look; the per-gun names marked "pcf (inferred)" in effects.md 1 are
//! used until the weapon scripts are read; only guns with a tracer count
//! toward the shared tracer counter; the per-shot flash and the tracers
//! leave along the aim, the `while_firing` systems along the view (the
//! brass to its right), the backblast back along the aim, not the
//! attachments' own axes; fused
//! pipes and stickies send no surface, so they take the in-air look and
//! leave no scorch (effects.md Open questions); TF2's impact style
//! (ricochets, pellet grouping) has no spec yet: no ricochet, every
//! pellet's sound, as HL2:DM's (specs/tf2 names no impact rules).

use std::collections::BTreeSet;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_core::core::{LocalPlayer, MapWater, Team};
use chimera_sim::weapon::{Inventory, Weapon, WeaponEvent, WeaponEventKind, aim_of};

use super::{
    blast::Tf2Explosion,
    damage::U,
    minigun::{State as MinigunState, Tf2Minigun},
};
use crate::map::{
    ShowLocalBody,
    loose::{DrawnLate, LateCopy, LateDelay},
    attached::{AnchorQuery, AttachPoint, AttachmentMark, MarksPlaced, first_person_shown},
    particles::ParticleSet,
    psys::{ControlPointState, Follow, ParticleControlPoints, StartParticleSystem, StartedSystemsRun, StopParticleSystems},
    view_model::{HeldAttachment, ViewAnimator, ViewModelCamera, ViewModelOffset, ViewModelSettings, ViewModels},
};

/// The explosion a blast with no name for its case shows (effects.md 5
/// step 3; the file spells it `ExplosionCore_Wall`, names match
/// ignoring case).
pub const EXPLOSION_FALLBACK: &str = "ExplosionCore_wall";

/// The hook that names a gun's looks in its data.
pub const LOOKS_HOOK: &str = "tf2:looks";

/// A system started on a named attachment of the gun.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct AttachedSystem {
    pub system: String,
    pub at: String,
}

/// A blast's three effects (effects.md 5): on a surface, in the air or on
/// a player, under water.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Explosions {
    #[serde(default)]
    pub wall: String,
    #[serde(default)]
    pub player: String,
    #[serde(default)]
    pub water: String,
}

impl Explosions {
    /// The effect for a blast in water or not, on a surface of `normal`
    /// (zero: in the air) or on a player, and its forward axis (engine
    /// space: along the normal; +X, zero angles, in the air).
    pub fn pick(looks: Option<&Self>, in_water: bool, normal: Vec3, on_player: bool) -> (&str, Vec3) {
        let air = on_player || normal == Vec3::ZERO;
        let field = looks.map(|l| if in_water { &l.water } else if air { &l.player } else { &l.wall });
        let effect = field.map(String::as_str).filter(|n| !n.is_empty()).unwrap_or(EXPLOSION_FALLBACK);
        let forward = if normal == Vec3::ZERO { Vec3::X } else { normal.normalize() };
        (effect, forward)
    }
}

/// A gun's looks as its data gives them (`[hooks."tf2:looks"]`).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct LooksParams {
    /// Its per-shot muzzle flash system (muzzle_flash.pcf).
    #[serde(default)]
    pub muzzle: Option<String>,
    /// Its tracer's name before the team and crit suffixes
    /// (bullet_tracers.pcf).
    #[serde(default)]
    pub tracer: Option<String>,
    /// One tracer for every this many bullets (the shared count).
    #[serde(default = "tracer_every")]
    pub tracer_every: u32,
    /// Systems that run while the gun is in its firing state (the
    /// Minigun's).
    #[serde(default)]
    pub while_firing: Vec<AttachedSystem>,
    /// Started on each shot for everyone but the shooter (the Rocket
    /// Launcher's backblast).
    #[serde(default)]
    pub backblast: Option<AttachedSystem>,
    /// Its blasts' effects (or another gun's: `explosion_of`).
    #[serde(default)]
    pub explosion: Option<Explosions>,
    /// The gun whose `explosion` its blasts show (the Grenade Launcher's
    /// pipes take the Stickybomb Launcher's, effects.md 5).
    #[serde(default)]
    pub explosion_of: Option<String>,
    /// A stream drawn on flame points (the Flame Thrower's: flames.md 3).
    #[serde(default)]
    pub stream: Option<StreamLooks>,
    /// The system each of its flyers trails (effects.md 1, 7).
    #[serde(default)]
    pub trail: Option<FlyerTrail>,
    /// A system started on each of its flyers once it is armed (the
    /// sticky's `stickybomb_pulse_<team>`, effects.md 1).
    #[serde(default)]
    pub armed: Option<ArmedLook>,
    /// The laser dot drawn where it aims while scoped (the Sniper
    /// Rifle's, weapons_more.md 3.1; `dot`).
    #[serde(default)]
    pub dot: Option<super::dot::DotLook>,
}

/// A system started on each flyer drawn as `shown`, `after` seconds after
/// it appears (when it arms), following it: the sticky's armed pulse.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ArmedLook {
    pub shown: String,
    pub system: String,
    #[serde(default)]
    pub team: bool,
    pub after: f32,
}

impl ArmedLook {
    /// Its system's name for a flyer of `team` (as `FlyerTrail::name`).
    pub fn name(&self, blu: Option<bool>) -> String {
        FlyerTrail { shown: self.shown.clone(), system: self.system.clone(), team: self.team, under_water: None }.name(blu)
    }
}

/// A trail started on each flyer of a gun as it appears (its `ShownItem`
/// is `shown`), following it without carrying what it left behind, and
/// stopped when it is gone (`psys::Follow`): the Rocket Launcher's
/// `rockettrail`. With `team` the name takes the owner's team suffix
/// (`_red`, `_blue`; RED when it has none: pipes and stickies).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FlyerTrail {
    pub shown: String,
    pub system: String,
    #[serde(default)]
    pub team: bool,
    /// Instead, for one whose first place is in water (effects.md 9:
    /// `rockettrail_underwater`, chosen once).
    #[serde(default)]
    pub under_water: Option<String>,
}

impl FlyerTrail {
    /// Its system's name for a flyer of `team` (`Some(true)`: BLU).
    pub fn name(&self, blu: Option<bool>) -> String {
        if !self.team {
            return self.system.clone();
        }
        format!("{}_{}", self.system, if blu == Some(true) { "blue" } else { "red" })
    }

    /// Its system for a flyer whose first place is in water (`wet`) or
    /// not.
    pub fn name_in(&self, blu: Option<bool>, wet: bool) -> String {
        match (&self.under_water, wet) {
            (Some(w), true) => w.clone(),
            _ => self.name(blu),
        }
    }
}

/// A gun's stream on its flame points (flames.md 3.1): one system per
/// burst, started at the attachment `at` (control point 0), its control
/// points 1 and up the points (`flame::FlameShown`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StreamLooks {
    pub system: String,
    /// A critical burst's, before the team suffix (`_red`, `_blue`).
    pub crit: String,
    /// With the eye under water.
    pub under_water: String,
    #[serde(default = "muzzle_name")]
    pub at: String,
    /// Seconds without a round after which a burst has ended.
    pub burst_gap: f32,
}

fn muzzle_name() -> String {
    MUZZLE_ATTACHMENT.into()
}

impl StreamLooks {
    /// The system a burst shows (flames.md 3.1): under water's, a crit's
    /// by team (`blu`), else the stream's.
    pub fn system_for(&self, crit: bool, blu: bool, under_water: bool) -> String {
        if under_water {
            self.under_water.clone()
        } else if crit {
            format!("{}_{}", self.crit, if blu { "blue" } else { "red" })
        } else {
            self.system.clone()
        }
    }

    /// Every name it can show.
    pub fn names(&self) -> Vec<String> {
        vec![self.system.clone(), self.system_for(true, false, false), self.system_for(true, true, false), self.under_water.clone()]
    }
}

fn tracer_every() -> u32 {
    2
}

/// A tracer's system: `base` with the shooter's team (`_red`, or `_blue`
/// when `blu`) and `_crit` for a critical shot (effects.md 6).
pub fn tracer_name(base: &str, blu: bool, crit: bool) -> String {
    format!("{base}_{}{}", if blu { "blue" } else { "red" }, if crit { "_crit" } else { "" })
}

/// Whether the bullet counted `n` (before it) draws a tracer at one in
/// `every` (effects.md 6).
pub fn draws_tracer(n: u32, every: u32) -> bool {
    n % every.max(1) == 0
}

/// Each gun's looks, read from the data: (gun, looks).
fn looks_table() -> Vec<(String, LooksParams)> {
    super::tf2_weapons()
        .weapons
        .into_iter()
        .filter_map(|w| {
            let p: LooksParams = w.hooks.get(LOOKS_HOOK)?.clone().try_into().unwrap_or_else(|e| panic!("{} {LOOKS_HOOK}: {e}", w.id));
            Some((w.id, p))
        })
        .collect()
}

/// The guns' looks the game's plugin holds (read once from the data).
#[derive(Resource, Clone, Debug)]
pub struct GunLooks(Vec<(String, LooksParams)>);

impl Default for GunLooks {
    fn default() -> Self {
        Self(looks_table())
    }
}

impl GunLooks {
    /// A gun's looks.
    pub fn of(&self, gun: &str) -> Option<&LooksParams> {
        self.0.iter().find(|(g, _)| g == gun).map(|(_, p)| p)
    }

    /// Every gun's laser dot.
    pub fn dots(&self) -> impl Iterator<Item = &super::dot::DotLook> {
        self.0.iter().filter_map(|(_, l)| l.dot.as_ref())
    }

    /// A gun's muzzle flash system.
    pub fn muzzle_of(&self, gun: &str) -> Option<&str> {
        self.of(gun)?.muzzle.as_deref()
    }

    /// A gun's tracer base and rate.
    pub fn tracer_of(&self, gun: &str) -> Option<(&str, u32)> {
        let l = self.of(gun)?;
        Some((l.tracer.as_deref()?, l.tracer_every))
    }

    /// The effects a blast of `gun` shows (its own, or the gun's it names).
    pub fn explosion_of(&self, gun: &str) -> Option<&Explosions> {
        let l = self.of(gun)?;
        match &l.explosion_of {
            Some(other) => self.of(other)?.explosion.as_ref(),
            None => l.explosion.as_ref(),
        }
    }

    /// The trail of a flyer drawn as `shown` (`ShownItem`), if a gun names one.
    pub fn trail_of_shown(&self, shown: &str) -> Option<&FlyerTrail> {
        self.0.iter().filter_map(|(_, l)| l.trail.as_ref()).find(|t| t.shown == shown)
    }

    /// The armed look of a flyer drawn as `shown`, if a gun names one.
    pub fn armed_of_shown(&self, shown: &str) -> Option<&ArmedLook> {
        self.0.iter().filter_map(|(_, l)| l.armed.as_ref()).find(|t| t.shown == shown)
    }

    /// A gun's stream on flame points.
    pub fn stream_of(&self, gun: &str) -> Option<&StreamLooks> {
        self.of(gun)?.stream.as_ref()
    }

    /// Every system the guns name (for the loaders).
    pub fn systems(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (_, l) in &self.0 {
            out.extend(l.muzzle.clone());
            if let Some(t) = &l.tracer {
                for (blu, crit) in [(false, false), (true, false), (false, true), (true, true)] {
                    out.push(tracer_name(t, blu, crit));
                }
            }
            out.extend(l.while_firing.iter().map(|s| s.system.clone()));
            out.extend(l.backblast.iter().map(|s| s.system.clone()));
            out.extend(l.stream.iter().flat_map(StreamLooks::names));
            if let Some(t) = &l.trail {
                out.extend([t.name(Some(false)), t.name(Some(true))]);
                out.extend(t.under_water.clone());
            }
            if let Some(a) = &l.armed {
                out.extend([a.name(Some(false)), a.name(Some(true))]);
            }
            if let Some(e) = &l.explosion {
                out.extend([&e.wall, &e.player, &e.water].into_iter().filter(|n| !n.is_empty()).cloned());
            }
        }
        out.push(EXPLOSION_FALLBACK.into());
        let mut seen = BTreeSet::new();
        out.retain(|n| seen.insert(n.to_lowercase()));
        out
    }
}

/// Bullets drawn so far, every gun's and every shooter's (effects.md 6:
/// one counter on the client).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TracerCount(pub u32);

/// Where a muzzle flash starts from the eye when the gun's `muzzle`
/// attachment isn't drawn: forward, right, up (units).
pub const MUZZLE_OFFSET: Vec3 = Vec3::new(28.0, 11.0, -11.0);

/// The gun's attachment the flashes and flames start from.
pub const MUZZLE_ATTACHMENT: &str = "muzzle";

/// TF2's impacts: Source's shared impact effects and sounds. No spec says
/// TF2's own rules yet (module doc): no ricochet, splashes of 4 to 8,
/// every pellet's impact sound, damage in whole points, a hit on a
/// player sounding its flesh impact (TF2 has no hurt sounds yet); its
/// blood its own (`blood`, effects.md 8).
pub const STYLE: source_engine::effects::ImpactStyle = source_engine::effects::ImpactStyle {
    game: crate::game::GAME,
    ricochet: None,
    splash: (4.0, 8.0),
    group_radius: None,
    damage_unit: 1.0,
    flesh: "flesh",
    player_hits: true,
    blood: source_engine::effects::BloodView::Game,
    melee_decal: None,
    melee_own_only: true,
};

/// The systems TF2's guns start: loaded with its maps besides their own,
/// and onto other games' maps (`maps::add_guest_looks`): every name the
/// guns' data gives (`GunLooks::systems`: the flame stream's too).
pub fn particle_systems() -> &'static [&'static str] {
    // Leaked: the loaders want 'static names, and a map's load asks once.
    let names: Vec<&'static str> = GunLooks::default()
        .systems()
        .into_iter()
        .chain(super::blood::BloodLooks::default().systems())
        .map(|n| &*Box::leak(n.into_boxed_str()))
        .collect();
    Box::leak(names.into_boxed_slice())
}

/// The files they are in.
pub const PARTICLE_FILES: &[&str] = &[
    "particles/explosion.pcf",
    "particles/muzzle_flash.pcf",
    "particles/flamethrower.pcf",
    "particles/bullet_tracers.pcf",
    "particles/shellejection.pcf",
    "particles/rocketbackblast.pcf",
    "particles/rockettrail.pcf",
    "particles/stickybomb.pcf",
    "particles/nailtrails.pcf",
    "particles/blood_impact.pcf",
];

/// An explosion on the wire (the server's `Tf2Explosion`, to clients).
#[derive(Message, Clone, Debug, Serialize, Deserialize)]
pub struct ExplosionMsg {
    pub at: Vec3,
    pub normal: Vec3,
    pub on_player: bool,
    pub weapon: String,
}

/// Whether `p` is in the map's water (point contents, effects.md 5 step 1).
pub fn in_water(water: Option<&MapWater>, p: Vec3) -> bool {
    water.is_some_and(|w| w.0.iter().any(|v| v.brush.planes.iter().all(|(n, d)| n.dot(p) - d <= 0.0)))
}

/// Each explosion's looks (every machine that draws): the gun's effect
/// for where it went off (effects.md 5).
fn explosions(
    time: Res<Time>,
    looks: Res<GunLooks>,
    water: Option<Res<MapWater>>,
    delay: Option<Res<LateDelay>>,
    mut blasts: MessageReader<Tf2Explosion>,
    mut held: Local<Vec<(f64, Tf2Explosion)>>,
    mut start: MessageWriter<StartParticleSystem>,
) {
    let now = time.elapsed_secs_f64();
    let wait = delay.map_or(0.0, |d| d.0 as f64);
    held.extend(blasts.read().map(|b| (now + wait, *b)));
    held.retain(|(due, b)| {
        if *due > now + 1e-9 {
            return true;
        }
        let wet = in_water(water.as_deref(), b.at);
        let (name, forward) = Explosions::pick(looks.explosion_of(b.weapon), wet, b.normal, b.on_player);
        start.write(StartParticleSystem::new(name, b.at, forward));
        false
    });
}

/// Where a gun's attachments are drawn for each owner this frame: the
/// first-person view model's, else the held model's.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Muzzles<'w, 's> {
    time: Res<'w, Time>,
    owners: Query<
        'w,
        's,
        (
            Option<&'static ViewAnimator>,
            Option<&'static ViewModelOffset>,
            Option<&'static chimera_core::core::MapControls>,
            Option<&'static chimera_core::core::Intent>,
            Option<&'static Transform>,
            Has<LocalPlayer>,
        ),
    >,
    anchors: AnchorQuery<'w, 's>,
    vm_cameras: Query<'w, 's, &'static Projection, With<ViewModelCamera>>,
    held: Query<'w, 's, (&'static HeldAttachment, &'static GlobalTransform)>,
    models: Option<Res<'w, ViewModels>>,
    settings: Option<Res<'w, ViewModelSettings>>,
    third_person: Option<Res<'w, ShowLocalBody>>,
    marks: Query<'w, 's, (Entity, &'static AttachmentMark)>,
}

/// An attachment on the first-person view model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewMuzzle {
    /// The eye it is drawn from (the view model's anchor) and its turn.
    pub anchor: Entity,
    pub turn: Quat,
    /// The attachment in the eye's frame where the view-model camera
    /// draws it, and where it appears in the world view (re-projected:
    /// where its effects start, particles.md 6).
    pub local: Vec3,
    pub world_local: Vec3,
    /// Both in engine space.
    pub at: Vec3,
    pub world: Vec3,
    /// The attachment's `AttachmentMark`, placed this frame: what a
    /// system started here follows, to ride the gun's kick.
    pub mark: Option<Entity>,
}

/// Where an attachment is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MuzzleAt {
    View(ViewMuzzle),
    /// On the held model (engine space).
    Held(Vec3),
}

impl MuzzleAt {
    /// Where it appears in the world view (engine space).
    pub fn world(&self) -> Vec3 {
        match self {
            Self::View(v) => v.world,
            Self::Held(p) => *p,
        }
    }
}

impl Muzzles<'_, '_> {
    /// `owner`'s muzzle as it appears in the world view (engine space),
    /// if its gun's is drawn.
    pub fn of(&self, owner: Entity) -> Option<Vec3> {
        self.placed(owner).map(|m| m.world())
    }

    /// `owner`'s muzzle (`placed_at`).
    pub fn placed(&self, owner: Entity) -> Option<MuzzleAt> {
        self.placed_at(owner, MUZZLE_ATTACHMENT)
    }

    /// `owner`'s gun's attachment `name`: on its first-person view model
    /// (re-projected into the world view), else on its held model.
    pub fn placed_at(&self, owner: Entity, name: &str) -> Option<MuzzleAt> {
        let at = AttachPoint::named(name);
        let (view, offset, controls, ..) = self.owners.get(owner).ok()?;
        let fp = first_person_shown(
            owner,
            view,
            offset,
            controls,
            &self.anchors,
            &self.vm_cameras,
            self.models.as_deref(),
            self.settings.as_deref().copied().unwrap_or_default(),
            self.third_person.as_ref().is_some_and(|t| t.0),
            self.time.elapsed_secs_f64(),
        );
        match fp {
            Some(f) => {
                let local = f.camera_point(&at)?;
                let world_local = f.fovs.map_or(local, |(w, v)| crate::map::view_model::reproject(local, w, v));
                let mark = self.marks.iter().find(|(_, m)| m.owner == owner && m.shown && m.at == at).map(|(e, _)| e);
                Some(MuzzleAt::View(ViewMuzzle {
                    mark,
                    anchor: f.anchor,
                    turn: f.eye.rotation(),
                    local,
                    world_local,
                    at: f.eye.transform_point(local),
                    world: f.eye.transform_point(world_local),
                }))
            }
            None => self
                .held
                .iter()
                .find(|(h, _)| h.owner == owner && at.is(h.index, &h.name))
                .map(|(_, g)| MuzzleAt::Held(g.translation())),
        }
    }

    /// This machine's own player seen in first person with the view
    /// model switched off (`r_drawviewmodel 0`): no first-person effects
    /// (effects.md 2).
    pub fn view_model_off(&self, owner: Entity) -> bool {
        let local = self.owners.get(owner).is_ok_and(|(.., local)| local);
        let third = self.third_person.as_ref().is_some_and(|t| t.0);
        local && !third && self.settings.as_deref().is_some_and(|s| s.draw == 0)
    }

    /// The turn of the view `m` is seen along: the eye's on a
    /// first-person gun, else `owner`'s aim (its `Intent`'s yaw and pitch:
    /// a character's `Transform` doesn't turn), else its body's.
    pub fn view_turn(&self, owner: Entity, m: &MuzzleAt) -> Quat {
        match m {
            MuzzleAt::View(v) => v.turn,
            MuzzleAt::Held(..) => match self.owners.get(owner) {
                Ok((.., Some(i), _, _)) => aim_of(i.yaw, i.pitch),
                Ok((.., t, _)) => t.map_or(Quat::IDENTITY, |t| t.rotation),
                Err(_) => Quat::IDENTITY,
            },
        }
    }

    /// Whether `owner` is this machine's own player.
    pub fn is_local(&self, owner: Entity) -> bool {
        self.owners.get(owner).is_ok_and(|(.., local)| local)
    }

    /// `name` started at `m` facing `forward` (engine space), following
    /// what carries it: the eye on a first-person gun, else `owner`.
    pub fn start_on(&self, owner: Entity, m: &MuzzleAt, name: &str, forward: Vec3) -> StartParticleSystem {
        let follow = match m {
            // On the attachment's mark: riding the animated muzzle.
            MuzzleAt::View(ViewMuzzle { mark: Some(mark), turn, .. }) => {
                Some(Follow { entity: *mark, local: Vec3::ZERO, forward: turn.inverse() * forward, carry: true })
            }
            MuzzleAt::View(v) => Some(Follow { entity: v.anchor, local: v.world_local, forward: v.turn.inverse() * forward, carry: true }),
            MuzzleAt::Held(p) => self.owners.get(owner).ok().and_then(|(.., t, _)| t).map(|t| Follow {
                entity: owner,
                local: t.compute_affine().inverse().transform_point3(*p),
                forward: t.rotation.inverse() * forward,
                carry: true,
            }),
        };
        StartParticleSystem { follow, ..StartParticleSystem::new(name, m.world(), forward) }
    }
}

/// The attachments first-person systems start on (the per-shot flash, the
/// Minigun's and the stream's at `muzzle`, the brass at `eject_brass`).
const MARKED: &[&str] = &[MUZZLE_ATTACHMENT, "eject_brass"];

/// An `AttachmentMark` on each of `MARKED` for this machine's own player,
/// so its first-person systems ride the animated gun (effects.md 2).
fn mark_attachments(players: Query<Entity, (With<LocalPlayer>, With<ViewAnimator>)>, marks: Query<&AttachmentMark>, mut commands: Commands) {
    for owner in &players {
        for name in MARKED {
            let at = AttachPoint::named(name);
            if !marks.iter().any(|m| m.owner == owner && m.at == at) {
                commands.spawn((Name::new(format!("Attachment mark ({name})")), AttachmentMark::new(owner, at), Transform::default()));
            }
        }
    }
}

/// How long after another player's last round their gun still counts as
/// firing for its `while_firing` systems (s): two of the Minigun's
/// 0.105 s steps, so a dropped round doesn't stop it.
pub const REMOTE_FIRING_HOLD: f64 = 0.25;

/// A trail on each flyer as it appears (`FlyerTrail`), following it.
#[allow(clippy::type_complexity)]
fn flyer_trails(
    looks: Res<GunLooks>,
    flyers: Query<
        (Entity, &chimera_world::map::loose::ShownItem, &Transform, Option<&chimera_sim::weapon::projectile::Flyer>, Option<&LateCopy>),
        Added<chimera_world::map::loose::ShownItem>,
    >,
    late: Option<Res<DrawnLate>>,
    water: Option<Res<MapWater>>,
    teams: Query<&Team>,
    mut start: MessageWriter<StartParticleSystem>,
) {
    for (e, shown, at, flyer, copy) in &flyers {
        // One drawn late trails from its copy (effects.md 11).
        if !DrawnLate::drawn_here(late.as_deref(), &shown.0, copy.is_some()) {
            continue;
        }
        let Some(trail) = looks.trail_of_shown(&shown.0) else { continue };
        let owner = copy.map_or(flyer.and_then(|f| f.owner), |c| c.owner);
        let blu = owner.and_then(|o| teams.get(o).ok()).map(|t| super::super::bodies::skin_of(Some(*t)) == 1);
        let forward = at.rotation * Vec3::X;
        // Chosen once, by the water at its first place (effects.md 9).
        let wet = in_water(water.as_deref(), at.translation);
        start.write(StartParticleSystem {
            follow: Some(Follow { entity: e, local: Vec3::ZERO, forward: Vec3::X, carry: false }),
            ..StartParticleSystem::new(trail.name_in(blu, wet), at.translation, forward)
        });
    }
}

/// When a flyer with an armed look appeared (`ArmedLook`), until its
/// look has started.
#[derive(Component, Clone, Copy, Debug)]
pub struct ArmsAt(f64);

/// Each flyer's armed look, `after` seconds after it appeared, following
/// it (`psys::Follow`: stopped when the flyer is gone).
fn armed_looks(
    looks: Res<GunLooks>,
    time: Res<Time>,
    added: Query<(Entity, &chimera_world::map::loose::ShownItem, Has<LateCopy>), Added<chimera_world::map::loose::ShownItem>>,
    arming: Query<(Entity, &ArmsAt, &chimera_world::map::loose::ShownItem, &Transform, Option<&chimera_sim::weapon::projectile::Flyer>, Option<&LateCopy>)>,
    late: Option<Res<DrawnLate>>,
    teams: Query<&Team>,
    mut start: MessageWriter<StartParticleSystem>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    for (e, shown, copy) in &added {
        if looks.armed_of_shown(&shown.0).is_some() && DrawnLate::drawn_here(late.as_deref(), &shown.0, copy) {
            commands.entity(e).insert(ArmsAt(now));
        }
    }
    for (e, at, shown, t, flyer, copy) in &arming {
        let Some(armed) = looks.armed_of_shown(&shown.0) else { continue };
        if now < at.0 + armed.after as f64 {
            continue;
        }
        commands.entity(e).remove::<ArmsAt>();
        let owner = copy.map_or(flyer.and_then(|f| f.owner), |c| c.owner);
        let blu = owner.and_then(|o| teams.get(o).ok()).map(|t| super::super::bodies::skin_of(Some(*t)) == 1);
        start.write(StartParticleSystem {
            follow: Some(Follow { entity: e, local: Vec3::ZERO, forward: Vec3::X, carry: false }),
            ..StartParticleSystem::new(armed.name(blu), t.translation, t.rotation * Vec3::X)
        });
    }
}

/// Each shot's muzzle flash (and the backblast for others).
fn muzzle_flashes(
    looks: Res<GunLooks>,
    mut events: MessageReader<WeaponEvent>,
    weapons: Query<&Weapon>,
    muzzles: Muzzles,
    mut start: MessageWriter<StartParticleSystem>,
) {
    for e in events.read() {
        let WeaponEventKind::Fired { origin, yaw, pitch, .. } = e.kind else { continue };
        if !e.shown() {
            continue;
        }
        let Ok(weapon) = weapons.get(e.weapon).map(|w| w.id) else { continue };
        let aim = aim_of(yaw, pitch);
        let forward = aim * Vec3::NEG_Z;
        // The backblast, seen by everyone but the shooter (effects.md 4).
        if let Some(b) = looks.of(weapon).and_then(|l| l.backblast.as_ref())
            && !muzzles.is_local(e.owner)
            && let Some(m @ MuzzleAt::Held(..)) = muzzles.placed_at(e.owner, &b.at)
        {
            start.write(muzzles.start_on(e.owner, &m, &b.system, -forward));
        }
        let Some(name) = looks.muzzle_of(weapon) else { continue };
        if muzzles.view_model_off(e.owner) {
            continue;
        }
        let s = match muzzles.placed(e.owner) {
            // A flash goes with the gun drawn in the view (the eye's
            // frame), at the muzzle re-projected into the world view.
            Some(m @ MuzzleAt::View(_)) => muzzles.start_on(e.owner, &m, name, forward),
            m => StartParticleSystem::new(name, m.map_or_else(|| ahead_of(origin, aim), |m| m.world()), forward),
        };
        start.write(s);
    }
}

/// Where a system starts from the eye `origin` looking along `aim` when
/// the gun's attachment isn't drawn (`MUZZLE_OFFSET`).
fn ahead_of(origin: Vec3, aim: Quat) -> Vec3 {
    let o = MUZZLE_OFFSET * U;
    origin + aim * Vec3::NEG_Z * o.x + aim * Vec3::X * o.y + aim * Vec3::Y * o.z
}

/// The flame stream (flames.md 3): a new `new_flame` (a crit's or under
/// water's by `StreamLooks::system_for`) when a burst starts, at the
/// gun's muzzle as the flashes' (control point 0, carried with it),
/// stopped when it ends (its sprites live out); every frame its control
/// points 1 to 30 are the drawn copy of the points (`flame::FlameShown`:
/// slot i at control point i + 1, its life the duration, its box the
/// radius) and the newest is the active one, which new sprites ride. A
/// different name mid-burst (crit on or off) stops the old and starts the
/// new.
#[allow(clippy::type_complexity)]
pub fn flame_stream(
    time: Res<Time>,
    looks: Res<GunLooks>,
    mut guns: Query<(Entity, &Weapon, &super::flame::Tf2Flame, &super::flame::FlameShown, Option<&mut ParticleControlPoints>)>,
    owners: Query<(&Inventory, &Transform, Option<&chimera_core::core::MovementState>, Option<&chimera_core::core::Intent>)>,
    teams: Query<&Team>,
    muzzles: Muzzles,
    mut running: Local<std::collections::BTreeMap<Entity, String>>,
    mut start: MessageWriter<StartParticleSystem>,
    mut stop: MessageWriter<StopParticleSystems>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    let mut seen = BTreeSet::new();
    for (e, w, flame, shown, cps) in &mut guns {
        let Some(stream) = looks.stream_of(w.id) else { continue };
        seen.insert(e);
        // Its control points: every live point's.
        let half = flame.params.half();
        let points = ParticleControlPoints {
            points: shown
                .points
                .points
                .iter()
                .map(|p| {
                    let state = ControlPointState { position: p.at, velocity: p.velocity + p.carried, radius: half, duration: p.life };
                    (p.slot as usize + 1, state)
                })
                .collect(),
            active: shown.points.newest().map(|p| p.slot as usize + 1),
        };
        match cps {
            Some(mut c) => {
                if *c != points {
                    *c = points;
                }
            }
            None => {
                commands.entity(e).insert(points);
            }
        }
        // The burst.
        let owner = w.owner.filter(|o| owners.get(*o).is_ok_and(|(i, ..)| i.active == Some(e)));
        let firing = owner.is_some() && shown.points.last_round.is_some_and(|t| now - t <= stream.burst_gap as f64 + 1e-6);
        let blu = owner.and_then(|o| teams.get(o).ok()).is_some_and(|t| super::super::bodies::skin_of(Some(*t)) == 1);
        let name = firing.then(|| stream.system_for(false, blu, shown.points.under_water));
        if running.get(&e) == name.as_ref() {
            continue;
        }
        if running.remove(&e).is_some() {
            stop.write(StopParticleSystems { tag: e });
        }
        let (Some(name), Some(owner)) = (name, owner) else { continue };
        let Ok((_, at, state, intent)) = owners.get(owner) else { continue };
        let aim = intent.map_or(at.rotation, |i| aim_of(i.yaw, i.pitch));
        let forward = aim * Vec3::NEG_Z;
        let s = match muzzles.placed_at(owner, &stream.at) {
            Some(m) => muzzles.start_on(owner, &m, &name, forward),
            None => {
                let eye = at.translation + state.map_or(Vec3::ZERO, |s| s.eye_offset);
                StartParticleSystem::new(name.clone(), ahead_of(eye, aim), forward)
            }
        };
        start.write(StartParticleSystem { tag: Some(e), ..s });
        running.insert(e, name);
    }
    // Guns gone: their systems stop.
    let gone: Vec<Entity> = running.keys().copied().filter(|e| !seen.contains(e)).collect();
    for e in gone {
        running.remove(&e);
        stop.write(StopParticleSystems { tag: e });
    }
}

/// A tracer of another player's shot waiting for its body to fire
/// (`tracers`): when (this machine's time), whose, the bullet's start and
/// end, the system.
#[derive(Clone, Debug)]
pub struct HeldTracer {
    pub at: f64,
    pub owner: Entity,
    pub from: Vec3,
    pub to: Vec3,
    pub name: String,
}

/// How long another player's tracer waits: the interpolation delay
/// others are drawn with (`InterpClock::delay`), so it leaves when the
/// drawn body fires, as in TF2 (its flash comes at once, its tracers and
/// fire gesture about 7 frames at 60 later: `thirdperson.tf2`); none for
/// this machine's own player or without a clock.
pub fn tracer_hold(own: bool, delay: Option<f64>) -> f64 {
    if own { 0.0 } else { delay.unwrap_or(0.0).clamp(0.0, 0.5) }
}

/// Each bullet's tracer by the shared count (effects.md 6).
#[allow(clippy::too_many_arguments)]
fn tracers(
    time: Res<Time>,
    looks: Res<GunLooks>,
    mut events: MessageReader<WeaponEvent>,
    weapons: Query<&Weapon>,
    teams: Query<&Team>,
    drawn: Query<(), With<crate::map::interp::NetDrawn>>,
    clock: Option<Res<chimera_net::net::interp::InterpClock>>,
    muzzles: Muzzles,
    mut count: ResMut<TracerCount>,
    mut held: Local<Vec<HeldTracer>>,
    mut start: MessageWriter<StartParticleSystem>,
) {
    let now = time.elapsed_secs_f64();
    let go = |owner: Entity, from: Vec3, to: Vec3, name: String, start: &mut MessageWriter<StartParticleSystem>| {
        let at = if muzzles.view_model_off(owner) { None } else { muzzles.of(owner) };
        let at = at.unwrap_or(from);
        let forward = (to - at).try_normalize().unwrap_or(Vec3::NEG_Z);
        start.write(StartParticleSystem { end: Some(to), ..StartParticleSystem::new(name, at, forward) });
    };
    held.retain(|h| {
        if h.at > now {
            return true;
        }
        go(h.owner, h.from, h.to, h.name.clone(), &mut start);
        false
    });
    for e in events.read() {
        let WeaponEventKind::Shot { from, to, .. } = e.kind else { continue };
        if !e.shown() {
            continue;
        }
        let Ok(weapon) = weapons.get(e.weapon).map(|w| w.id) else { continue };
        let Some((base, every)) = looks.tracer_of(weapon) else { continue };
        let n = count.0;
        count.0 = n.wrapping_add(1);
        if !draws_tracer(n, every) {
            continue;
        }
        let blu = teams.get(e.owner).is_ok_and(|t| super::super::bodies::skin_of(Some(*t)) == 1);
        let name = tracer_name(base, blu, false);
        let hold = tracer_hold(!drawn.contains(e.owner), clock.as_ref().map(|c| c.delay));
        if hold > 0.0 {
            held.push(HeldTracer { at: now + hold, owner: e.owner, from, to, name });
        } else {
            go(e.owner, from, to, name, &mut start);
        }
    }
}

/// The guns' `while_firing` systems: started when a gun enters its firing
/// state in its owner's hands, stopped when it leaves it (effects.md 3).
#[allow(clippy::too_many_arguments)]
fn firing_looks(
    time: Res<Time>,
    looks: Res<GunLooks>,
    guns: Query<(Entity, &Weapon, &Tf2Minigun)>,
    owners: Query<&Inventory>,
    eyes: Query<(&Transform, Option<&chimera_core::core::MovementState>, Option<&chimera_core::core::Intent>)>,
    muzzles: Muzzles,
    mut remote: MessageReader<chimera_sim::weapon::remote::RemoteShot>,
    mut shots: MessageReader<WeaponEvent>,
    mut heard: Local<std::collections::BTreeMap<Entity, f64>>,
    mut running: Local<BTreeSet<Entity>>,
    mut start: MessageWriter<StartParticleSystem>,
    mut stop: MessageWriter<StopParticleSystems>,
) {
    let t = time.elapsed_secs_f64();
    // Others' guns: their state arrives with what they hold
    // (`HeldState`), a little after their rounds may; firing is also a
    // round of theirs within `REMOTE_FIRING_HOLD`.
    for r in remote.read() {
        if let Some(w) = owners.get(r.shooter).ok().and_then(|i| i.active) {
            heard.insert(w, t);
        }
    }
    for e in shots.read() {
        if matches!(e.kind, WeaponEventKind::Fired { .. }) && e.shown() {
            heard.insert(e.weapon, t);
        }
    }
    heard.retain(|_, at| t - *at <= REMOTE_FIRING_HOLD);
    let mut now = BTreeSet::new();
    for (e, w, gun) in &guns {
        let held = w.owner.and_then(|o| owners.get(o).ok()).is_some_and(|i| i.active == Some(e));
        if !held || (gun.state != MinigunState::Firing && !heard.contains_key(&e)) {
            continue;
        }
        let Some(systems) = looks.of(w.id).map(|l| &l.while_firing).filter(|s| !s.is_empty()) else { continue };
        now.insert(e);
        if running.contains(&e) {
            continue;
        }
        let Some(owner) = w.owner else { continue };
        if muzzles.view_model_off(owner) {
            continue;
        }
        for s in systems {
            let Some(m) = muzzles.placed_at(owner, &s.at) else {
                // No gun drawn (not loaded yet): the muzzle's a little
                // ahead of the eye under the gun, as a flash's.
                if s.at == MUZZLE_ATTACHMENT
                    && let Ok((at, state, intent)) = eyes.get(owner)
                {
                    let aim = intent.map_or(at.rotation, |i| aim_of(i.yaw, i.pitch));
                    let eye = at.translation + state.map_or(Vec3::ZERO, |s| s.eye_offset);
                    start.write(StartParticleSystem { tag: Some(e), ..StartParticleSystem::new(s.system.clone(), ahead_of(eye, aim), aim * Vec3::NEG_Z) });
                }
                continue;
            };
            // Out of the barrel along the view, the brass out to the
            // right (module doc, Known shortcuts).
            let turn = muzzles.view_turn(owner, &m);
            let forward = if s.at == MUZZLE_ATTACHMENT { turn * Vec3::NEG_Z } else { turn * Vec3::X };
            start.write(StartParticleSystem { tag: Some(e), ..muzzles.start_on(owner, &m, &s.system, forward) });
        }
    }
    for e in running.difference(&now) {
        stop.write(StopParticleSystems { tag: *e });
    }
    *running = now;
}

/// The server: this frame's explosions to every client.
fn forward(mut blasts: MessageReader<Tf2Explosion>, mut out: MessageWriter<bevy_replicon::prelude::ToClients<ExplosionMsg>>) {
    use bevy_replicon::prelude::*;
    for b in blasts.read() {
        out.write(ToClients {
            targets: SendTargets::CLIENTS_ONLY,
            message: ExplosionMsg { at: b.at, normal: b.normal, on_player: b.on_player, weapon: b.weapon.into() },
        });
    }
}

/// A client: the server's explosions as its own (a gun it doesn't know
/// shows the fallback explosion).
fn receive(mut fx: MessageReader<ExplosionMsg>, mut out: MessageWriter<Tf2Explosion>) {
    for f in fx.read() {
        let weapon = super::GUNS.into_iter().find(|g| *g == f.weapon).unwrap_or("");
        out.write(Tf2Explosion { at: f.at, normal: f.normal, on_player: f.on_player, weapon });
    }
}

/// The protocol: explosions, from the server.
fn register(app: &mut App) {
    use bevy_replicon::prelude::*;
    use crate::core::NetRole;
    app.add_server_message::<ExplosionMsg>(Channel::Unreliable)
        .add_systems(PostUpdate, forward.before(ServerSystems::Send).run_if(resource_equals(NetRole::Server)))
        .add_systems(PreUpdate, receive.after(ClientSystems::Receive).run_if(resource_equals(NetRole::Client)));
}

/// The looks, on any machine that draws (systems that only write
/// messages the presentation reads, harmless without it).
pub fn add(app: &mut App) {
    use chimera_net::net::parts::NetAppExt;
    use crate::map::impact::SurfaceClassesAppExt;
    app.init_resource::<GunLooks>()
        .init_resource::<TracerCount>()
        .add_message::<StartParticleSystem>()
        .add_message::<StopParticleSystems>()
        .add_plugins((
            source_engine::effects::impact_effects::ImpactEffectsPlugin(STYLE),
            source_engine::effects::impacts::ImpactSoundsPlugin(STYLE),
        ))
        .register_surface_classes(source_engine::effects::impacts::surfaces(crate::game::GAME))
        .add_systems(
            Update,
            (
                (explosions, muzzle_flashes, tracers, firing_looks, flyer_trails, armed_looks, super::dot::laser_dots),
                (super::flame::shown_flames.run_if(not(resource_exists::<chimera_net::dedicated::Serving>)), flame_stream)
                    .chain()
                    .before(StartedSystemsRun),
            )
                .after(ParticleSet::Step)
                .before(ParticleSet::Draw)
                .after(crate::map::attached::EyePlaced)
                .after(crate::map::DriveAnimation)
                .after(MarksPlaced),
        )
        .add_systems(Update, mark_attachments)
        .net_part("tf2_looks", register);
    super::blood::add(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bullet_gun_and_launcher_names_its_muzzle_in_data() {
        use super::super as w;
        let looks = GunLooks::default();
        let muzzle_of = |g| looks.muzzle_of(g);
        for gun in [w::SCATTERGUN, w::SHOTGUN, w::PISTOL_SCOUT, w::SMG, w::SNIPER_RIFLE, w::REVOLVER, w::GRENADE_LAUNCHER, w::STICKY_LAUNCHER, w::SYRINGE_GUN] {
            assert!(muzzle_of(gun).is_some_and(|n| n.starts_with("muzzle_")), "{gun}");
        }
        assert_eq!(looks.muzzle_of(w::FLAMETHROWER), None, "its stream is its muzzle effect (effects.md 1)");
        assert!(muzzle_of(w::ROCKET_LAUNCHER).is_none(), "no rocket-launcher muzzle system exists (effects.md 1)");
        assert!(particle_systems().contains(&"muzzle_pipelauncher") && particle_systems().contains(&"ExplosionCore_Wall"));
    }

    /// effects.md 1 and 3: the Minigun has no per-shot flash, its
    /// constant muzzle and brass run while it fires.
    #[test]
    fn the_minigun_has_no_shot_flash_only_its_firing_systems() {
        let looks = GunLooks::default();
        let m = looks.of(super::super::MINIGUN).expect("the minigun's looks");
        assert_eq!(m.muzzle, None);
        assert_eq!(
            m.while_firing,
            vec![
                AttachedSystem { system: "muzzle_minigun_constant".into(), at: "muzzle".into() },
                AttachedSystem { system: "eject_minigunbrass".into(), at: "eject_brass".into() },
            ]
        );
        assert_eq!(looks.tracer_of(super::super::MINIGUN), Some(("bullet_tracer01", 4)));
    }

    /// effects.md 1 (the per-gun tracer names, "pcf (inferred)" ones
    /// included) and 6 (every 2nd bullet, the Minigun's 4th).
    #[test]
    fn others_tracers_wait_for_their_drawn_body() {
        assert_eq!(tracer_hold(true, Some(0.1)), 0.0);
        assert!((tracer_hold(false, Some(0.1)) - 0.1).abs() < 1e-9);
        assert_eq!(tracer_hold(false, None), 0.0);
        assert_eq!(tracer_hold(false, Some(3.0)), 0.5);
    }

    #[test]
    fn tracer_names_and_rates_follow_the_spec() {
        use super::super as w;
        let looks = GunLooks::default();
        for (gun, base) in [
            (w::SCATTERGUN, "bullet_scattergun_tracer01"),
            (w::SHOTGUN, "bullet_shotgun_tracer01"),
            (w::PISTOL_SCOUT, "bullet_pistol_tracer01"),
            (w::REVOLVER, "bullet_pistol_tracer01"),
            (w::SMG, "bullet_tracer01"),
        ] {
            assert_eq!(looks.tracer_of(gun), Some((base, 2)), "{gun}");
        }
        for gun in [w::SNIPER_RIFLE, w::ROCKET_LAUNCHER, w::GRENADE_LAUNCHER, w::STICKY_LAUNCHER, w::SYRINGE_GUN, w::FLAMETHROWER] {
            assert_eq!(looks.tracer_of(gun), None, "{gun}");
        }
        assert_eq!(tracer_name("bullet_scattergun_tracer01", false, true), "bullet_scattergun_tracer01_red_crit");
        assert_eq!(tracer_name("bullet_tracer01", true, false), "bullet_tracer01_blue");
        // A Scattergun blast from a count of 0: pellets 1, 3, 5, 7, 9.
        let drawn: Vec<u32> = (0..10).filter(|n| draws_tracer(*n, 2)).map(|n| n + 1).collect();
        assert_eq!(drawn, [1, 3, 5, 7, 9]);
        // 8 Minigun shots of 4 bullets: 8 tracers.
        assert_eq!((0..32).filter(|n| draws_tracer(*n, 4)).count(), 8);
        for name in ["bullet_scattergun_tracer01_blue", "bullet_tracer01_red_crit", "muzzle_minigun_constant", "eject_minigunbrass", "rocketbackblast"] {
            assert!(particle_systems().contains(&name), "{name} loads");
        }
    }

    /// effects.md 9: the rocket's trail is chosen once by the water at
    /// its first place; 10: TF2's melee impacts show on the attacker's
    /// machine only.
    #[test]
    fn the_rocket_trail_under_water_and_melee_impacts_own_only() {
        use chimera_sim::weapon::{WeaponEvent, WeaponEventKind};
        let looks = GunLooks::default();
        let t = looks.trail_of_shown("tf2:tf_weapon_rocketlauncher/rocket").expect("the rocket's trail");
        assert_eq!(t.name_in(None, false), "rockettrail");
        assert_eq!(t.name_in(None, true), "rockettrail_underwater");
        assert!(particle_systems().contains(&"rockettrail_underwater") && particle_systems().contains(&"blood_spray_red_01_far"));
        let swing = WeaponEvent {
            owner: Entity::PLACEHOLDER,
            weapon: Entity::PLACEHOLDER,
            kind: WeaponEventKind::Swing { hit: true, secondary: false, at: None, line: (Vec3::ZERO, Vec3::ZERO) },
            replay: false,
        };
        assert!(STYLE.skips(&swing, false) && !STYLE.skips(&swing, true));
    }

    /// effects.md 5: which effect a blast shows, oriented how.
    #[test]
    fn explosions_pick_by_water_air_and_surface() {
        use super::super as w;
        let looks = GunLooks::default();
        let rocket = looks.explosion_of(w::ROCKET_LAUNCHER);
        assert!(rocket.is_some());
        let wall = Vec3::X;
        assert_eq!(Explosions::pick(rocket, false, wall, false), ("ExplosionCore_Wall", Vec3::X));
        assert_eq!(Explosions::pick(rocket, false, wall, true).0, "ExplosionCore_MidAir", "on a player: in the air");
        assert_eq!(Explosions::pick(rocket, false, Vec3::ZERO, false), ("ExplosionCore_MidAir", Vec3::X), "zero angles");
        // Pipes and stickies read the Stickybomb Launcher's.
        assert_eq!(looks.explosion_of(w::GRENADE_LAUNCHER), looks.explosion_of(w::STICKY_LAUNCHER));
        let sticky = looks.explosion_of(w::GRENADE_LAUNCHER);
        assert_eq!(Explosions::pick(sticky, true, wall, false).0, "ExplosionCore_MidAir_underwater");
        assert_eq!(Explosions::pick(None, false, wall, false).0, EXPLOSION_FALLBACK);
        assert_eq!(looks.of(w::ROCKET_LAUNCHER).and_then(|l| l.backblast.clone()), Some(AttachedSystem { system: "rocketbackblast".into(), at: "backblast".into() }));
    }
}
