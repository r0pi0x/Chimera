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
//!   inside a wall you stand against is hidden by it, as in TF2), following
//!   the eye it was started from (`StartParticleSystem::follow`); else the
//!   held model's (`HeldAttachment`: what others see), else a little ahead
//!   of the eye under the gun (`MUZZLE_OFFSET`). None while the local
//!   player's view model is switched off (`r_drawviewmodel 0`).
//! - **The Minigun** (effects.md 3): no per-shot flash; its
//!   `while_firing` systems (`muzzle_minigun_constant` at `muzzle`,
//!   `eject_minigunbrass` at `eject_brass`) start when it enters its firing
//!   state (`Tf2Minigun::playing` the fire loop) and stop emitting when it
//!   leaves it (`StopParticleSystems`; live particles finish).
//! - **Tracers** (effects.md 6): one counter shared by every bullet drawn
//!   (`TracerCount`, every gun with a tracer, every shooter): a bullet
//!   whose count before it is a multiple of its gun's `tracer_every` (2;
//!   the Minigun's 4) draws `<tracer>_red`/`_blue` by the shooter's team
//!   (`_crit` appended on a critical shot: crits are off), from the
//!   muzzle as the flash's (else the bullet's start) to the bullet's end:
//!   control point 1 (`StartParticleSystem::end`), which the system's
//!   "move particles between 2 control points" heads for at its speed.
//! - **The Rocket Launcher's backblast** (effects.md 4): `rocketbackblast`
//!   at the launcher's `backblast` attachment on each shot, for everyone
//!   but the shooter (the held model's: never on this machine's own
//!   player).
//! - **Explosions** (effects.md 5; `blast::Tf2Explosion`, sent by the
//!   server to every client): by the gun's `explosion` (the Stickybomb
//!   Launcher's for pipes and stickies): `water` when the centre is in
//!   water, `player` in the air or on a player, else `wall`, oriented to
//!   the surface's normal (zero angles in the air); an unnamed one is
//!   `ExplosionCore_wall`. The server traces the `Scorch` decal
//!   (`blast`).
//! - **Flames**: each round of the Flame Thrower (one every 0.08 s while
//!   attack is held) starts a piece of TF2's `flamethrower` system
//!   (`flamethrower_blue` for team 2, BLU's) at the muzzle along the aim,
//!   emitting for one round's interval (`FLAME_PIECE`), so the stream
//!   follows the aim and ends when firing does.
//! - **Impacts**: Source's impact effects, decals and impact sounds with
//!   TF2's style (`STYLE`): TF2 is an impact provider, so its guns' hits
//!   are TF2's own on any map that carries its looks (`maps::
//!   add_impact_looks`), never the map's game's or the app's base game's
//!   (Combat Arms' in the party apps). Which entry a surface plays is the
//!   map's sound scripts' (a name the map has, such as CS:S's
//!   `Concrete.BulletImpact` on de_dust2, plays the map's).
//!
//! Known shortcuts: the flame is pieces of TF2's system per round, not
//! its own continuous emitter (flames.md is its own task); no airblast
//! look; the per-gun names marked "pcf (inferred)" in effects.md 1 are
//! used until the weapon scripts are read; only guns with a tracer count
//! toward the shared tracer counter; the per-shot flash and the tracers
//! leave along the aim, not the muzzle attachment's own axes (the
//! `while_firing` and backblast systems take the attachment's); fused
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
    minigun::{Loop, Tf2Minigun},
};
use crate::map::{
    ShowLocalBody,
    attached::{AnchorQuery, AttachPoint, first_person_shown},
    particles::ParticleSet,
    psys::{Follow, StartParticleSystem, StopParticleSystems},
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

    /// The system a shot of `gun` starts and for how long it emits (None:
    /// as its definition runs): its muzzle flash, or for the Flame Thrower
    /// a piece of its team's flames (`blu`: team 2's).
    pub fn shot_system(&self, gun: &str, blu: bool) -> Option<(&str, Option<f32>)> {
        if gun == super::FLAMETHROWER {
            return Some((FLAMES[blu as usize], Some(FLAME_PIECE)));
        }
        self.muzzle_of(gun).map(|n| (n, None))
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

/// The Flame Thrower's flames by team: RED's, BLU's (flamethrower.pcf).
pub const FLAMES: [&str; 2] = ["flamethrower", "flamethrower_blue"];

/// Seconds each round's piece of the flame emits: the round's interval
/// (0.08 s) and a little, so the pieces join.
pub const FLAME_PIECE: f32 = 0.09;

/// The gun's attachment the flashes and flames start from.
pub const MUZZLE_ATTACHMENT: &str = "muzzle";

/// TF2's impacts: Source's shared impact effects and sounds. No spec says
/// TF2's own rules yet (module doc): no ricochet, splashes of 4 to 8,
/// every pellet's impact sound, damage in whole points, a hit on a
/// player sounding its flesh impact (TF2 has no hurt sounds yet).
pub const STYLE: source_engine::effects::ImpactStyle = source_engine::effects::ImpactStyle {
    game: crate::game::GAME,
    ricochet: None,
    splash: (4.0, 8.0),
    group_radius: None,
    damage_unit: 1.0,
    flesh: "flesh",
    player_hits: true,
    blood: source_engine::effects::BloodView::Damage,
    melee_decal: None,
};

/// The systems TF2's guns start: loaded with its maps besides their own,
/// and onto other games' maps (`maps::add_guest_looks`): every name the
/// guns' data gives (`GunLooks::systems`) and both teams' flames.
pub fn particle_systems() -> &'static [&'static str] {
    // Leaked: the loaders want 'static names, and a map's load asks once.
    let names: Vec<&'static str> = GunLooks::default()
        .systems()
        .into_iter()
        .chain(FLAMES.map(str::to_string))
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
    looks: Res<GunLooks>,
    water: Option<Res<MapWater>>,
    mut blasts: MessageReader<Tf2Explosion>,
    mut start: MessageWriter<StartParticleSystem>,
) {
    for b in blasts.read() {
        let wet = in_water(water.as_deref(), b.at);
        let (name, forward) = Explosions::pick(looks.explosion_of(b.weapon), wet, b.normal, b.on_player);
        start.write(StartParticleSystem::new(name, b.at, forward));
    }
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
    /// Its x axis (out of the barrel), engine space.
    pub axis: Option<Vec3>,
}

/// Where an attachment is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MuzzleAt {
    View(ViewMuzzle),
    /// On the held model (engine space): place and x axis.
    Held(Vec3, Vec3),
}

impl MuzzleAt {
    /// Where it appears in the world view (engine space).
    pub fn world(&self) -> Vec3 {
        match self {
            Self::View(v) => v.world,
            Self::Held(p, _) => *p,
        }
    }

    /// Its x axis, if known (engine space).
    pub fn axis(&self) -> Option<Vec3> {
        match self {
            Self::View(v) => v.axis,
            Self::Held(_, a) => Some(*a),
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
                Some(MuzzleAt::View(ViewMuzzle {
                    anchor: f.anchor,
                    turn: f.eye.rotation(),
                    local,
                    world_local,
                    at: f.eye.transform_point(local),
                    world: f.eye.transform_point(world_local),
                    axis: f.camera_axis(&at).map(|a| f.eye.rotation() * a),
                }))
            }
            None => self
                .held
                .iter()
                .find(|(h, _)| h.owner == owner && at.is(h.index, &h.name))
                .map(|(_, g)| MuzzleAt::Held(g.translation(), g.rotation() * Vec3::X)),
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

    /// Whether `owner` is this machine's own player.
    pub fn is_local(&self, owner: Entity) -> bool {
        self.owners.get(owner).is_ok_and(|(.., local)| local)
    }

    /// `name` started at `m` facing `forward` (engine space), following
    /// what carries it: the eye on a first-person gun, else `owner`.
    pub fn start_on(&self, owner: Entity, m: &MuzzleAt, name: &str, forward: Vec3) -> StartParticleSystem {
        let follow = match m {
            MuzzleAt::View(v) => Some(Follow { entity: v.anchor, local: v.world_local, forward: v.turn.inverse() * forward }),
            MuzzleAt::Held(p, _) => self.owners.get(owner).ok().and_then(|(.., t, _)| t).map(|t| Follow {
                entity: owner,
                local: t.compute_affine().inverse().transform_point3(*p),
                forward: t.rotation.inverse() * forward,
            }),
        };
        StartParticleSystem { follow, ..StartParticleSystem::new(name, m.world(), forward) }
    }
}

/// Each shot's muzzle flash (and the backblast for others), and each
/// Flame Thrower round's flames.
fn muzzle_flashes(
    looks: Res<GunLooks>,
    mut events: MessageReader<WeaponEvent>,
    weapons: Query<&Weapon>,
    teams: Query<&Team>,
    muzzles: Muzzles,
    mut start: MessageWriter<StartParticleSystem>,
) {
    for e in events.read() {
        let WeaponEventKind::Fired { origin, yaw, pitch, .. } = e.kind else { continue };
        if !e.shown() {
            continue;
        }
        let Ok(weapon) = weapons.get(e.weapon).map(|w| w.id) else { continue };
        let blu = teams.get(e.owner).is_ok_and(|t| t.0 == 2);
        let aim = aim_of(yaw, pitch);
        let forward = aim * Vec3::NEG_Z;
        // The backblast, seen by everyone but the shooter (effects.md 4).
        if let Some(b) = looks.of(weapon).and_then(|l| l.backblast.as_ref())
            && !muzzles.is_local(e.owner)
            && let Some(m @ MuzzleAt::Held(..)) = muzzles.placed_at(e.owner, &b.at)
        {
            start.write(muzzles.start_on(e.owner, &m, &b.system, m.axis().unwrap_or(-forward)));
        }
        let Some((name, emit_for)) = looks.shot_system(weapon, blu) else { continue };
        if emit_for.is_none() && muzzles.view_model_off(e.owner) {
            continue;
        }
        let o = MUZZLE_OFFSET * U;
        let ahead = || origin + forward * o.x + aim * Vec3::X * o.y + aim * Vec3::Y * o.z;
        let s = match muzzles.placed(e.owner) {
            // A flash goes with the gun drawn in the view (the eye's
            // frame), at the muzzle re-projected into the world view.
            Some(m @ MuzzleAt::View(_)) if emit_for.is_none() => muzzles.start_on(e.owner, &m, name, forward),
            // The flames stream into the world from where the muzzle
            // appears there.
            m => StartParticleSystem { emit_for, ..StartParticleSystem::new(name, m.map_or_else(ahead, |m| m.world()), forward) },
        };
        start.write(s);
    }
}

/// Each bullet's tracer by the shared count (effects.md 6).
fn tracers(
    looks: Res<GunLooks>,
    mut events: MessageReader<WeaponEvent>,
    weapons: Query<&Weapon>,
    teams: Query<&Team>,
    muzzles: Muzzles,
    mut count: ResMut<TracerCount>,
    mut start: MessageWriter<StartParticleSystem>,
) {
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
        let at = if muzzles.view_model_off(e.owner) { None } else { muzzles.of(e.owner) };
        let at = at.unwrap_or(from);
        let forward = (to - at).try_normalize().unwrap_or(Vec3::NEG_Z);
        start.write(StartParticleSystem { end: Some(to), ..StartParticleSystem::new(tracer_name(base, blu, false), at, forward) });
    }
}

/// The guns' `while_firing` systems: started when a gun enters its firing
/// state in its owner's hands, stopped when it leaves it (effects.md 3).
fn firing_looks(
    looks: Res<GunLooks>,
    guns: Query<(Entity, &Weapon, &Tf2Minigun)>,
    owners: Query<&Inventory>,
    muzzles: Muzzles,
    mut running: Local<BTreeSet<Entity>>,
    mut start: MessageWriter<StartParticleSystem>,
    mut stop: MessageWriter<StopParticleSystems>,
) {
    let mut now = BTreeSet::new();
    for (e, w, gun) in &guns {
        let held = w.owner.and_then(|o| owners.get(o).ok()).is_some_and(|i| i.active == Some(e));
        if !held || gun.playing != Loop::Fire {
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
        let aim = muzzles.placed(owner).and_then(|m| m.axis()).unwrap_or(Vec3::NEG_Z);
        for s in systems {
            let Some(m) = muzzles.placed_at(owner, &s.at) else { continue };
            let forward = if s.at == MUZZLE_ATTACHMENT { aim } else { m.axis().unwrap_or(aim) };
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
            (explosions, muzzle_flashes, tracers, firing_looks)
                .after(ParticleSet::Step)
                .before(ParticleSet::Draw)
                .after(crate::map::attached::EyePlaced)
                .after(crate::map::DriveAnimation),
        )
        .net_part("tf2_looks", register);
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
        assert_eq!(looks.shot_system(w::FLAMETHROWER, true), Some(("flamethrower_blue", Some(FLAME_PIECE))));
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
