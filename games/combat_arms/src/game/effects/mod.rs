//! Combat Arms' effects and sounds (specs/combat_arms/effects_sounds.md;
//! section numbers below are its), built where they are shown from the
//! weapon events every world sees (a network client re-traces others'
//! rounds), in `Update`:
//!
//! - **Shots** (`shot_effects`, 1, 2, 4-8, 11): the fire sound (yours
//!   local, others' 3D at their muzzle with the weapon's or the
//!   silencer's radius, `CaMods`: silenced level ℓ plays `SilencedFireSnd`ℓ,
//!   a silenced part without a level nothing); the third-person muzzle
//!   group (`hh_muzzle`, the muzzle part's first) for others when the fire
//!   point is seen; your FireFX group (`test`) at your flash point (a
//!   launcher's, its ammo's install record `CaWeaponFx::fire_record`:
//!   parented to the shooter, shown only when the fire point as the
//!   server sent it (`rules::compress_position`) passes the view test from
//!   your camera now, so most launches show nothing; others see the
//!   third-person muzzle group at the held launcher's `Flash` socket,
//!   turned toward the world origin: special_weapons.md Corrections
//!   (2026-10-31) 1); a
//!   casing (seen and near fire point, the FireFX ejecting); the muzzle or
//!   FireFX group again for each further pellet of a round; the tracer
//!   (`rules::TracerCounter`: yours by the last fitted part's tracer, the
//!   ammo's when it names none, others' by the ammo's; the muzzle part's
//!   look, else the ammo's; first ray only); per ray the impact: the surface type
//!   (characters 6, 4 for a head hit; the world's from `SurfaceGrid`,
//!   meshes tagged by `library::tag_surfaces`; nothing hit: sky, no
//!   impact), the ImpactFX's sound (the surface's list by ammo kind,
//!   one of four slots), its own group, and when the impact is seen the
//!   decal (`rules::CaMarks`: crowding, cap, fade) and the surface's
//!   burst (blood on characters); others' bullets fly by within 600 u
//!   (`rules::flyby`, played at the nearest point of the line, also for
//!   shots that hit nothing); your hits
//!   on characters queue a ding (`rules::DingQueue`, played next frame);
//!   dry fire makes none; each melee hit key (special_weapons.md 5.1)
//!   first its impact (one of four slots, R 153; a miss is a sky impact:
//!   silent), then its fire sound, on a miss too: yours `Alt_FireSnd`
//!   for a secondary swing (`fire_sound_of`), others' always `FireSnd`
//!   (Quirk 10). Fire sounds go on the shooter's fire voice
//!   (`fire_voice`: one per attacker and weapon, both buttons sharing it,
//!   a new one replacing the last; silenced shots take none). A round whose gun is gone
//!   when it is drawn (the gun game swaps it on the kill's tick) is drawn
//!   as that gun (`KnownWeapons`: by entity until its despawn's frame is
//!   drawn, by weapon ID for others' rounds a client hears after their gun
//!   changed).
//! - **View-model keys** (`key_sounds`): `sound_key n` plays the weapon's
//!   reload, select, deselect or misc sound (yours local, others' 3D, R
//!   `ReloadSndRadius`/`SelectSndRadius`, 2000 for misc; none from the
//!   dead); `SHELL_CASING` drops a casing
//!   for a weapon whose FireFX doesn't eject.
//! - **Grenades** (`grenade_effects`, 9): the ImpactFX by weapon
//!   (`[grenade]` in data/effects.toml; an impact projectile's, a rocket's
//!   or a launcher shell's, its weapon's `combat_arms:effects` `impact`,
//!   with the install's record from the merge: special_weapons.md 1.3):
//!   its sound at the blast (none on
//!   sky); the surface's grenade group when seen (that of the grenade's
//!   last bounce or touch, its own Metal if none: mid-air too; a
//!   launched round's, not grenade ammo, the surface's impact group:
//!   `launched_burst`, special_weapons.md Follow-up 8);
//!   the flash's and smoke's own groups always. Bounces (`bounce_sounds`,
//!   the server's): the surface's grenade sound, one per grenade.
//! - **Hurt** (`hurt_sounds`, the server's): a sound by damage kind and
//!   one by hit zone (none for burn and gas) at the victim's centre,
//!   radius 500, each kind one at a time across the server
//!   (`rules::HurtLimiter`).
//! - **Footsteps** (`footsteps`): yours by the sway phase
//!   (`rules::OwnSteps`: the bob's ramp restarts it after a stop, a step
//!   on the movement key, silent off the run flag), others' by their
//!   gait's animation keys (`rules::footsteps_between`: walking and
//!   crouching silent) from the character's centre, the surface under the
//!   feet (ladders 200), radius 1200, the model's `FootprintSndVolume`
//!   share added to the volume (ghillie -0.5), the listener's Awareness
//!   in place of the walker's Specter plus the perk's own sound
//!   (`footstep_sound`); your jump voice. With the party's
//!   `party_even_footsteps 1` (`map::sound::EvenFootsteps`, default off)
//!   others' steps fall off by the shared profile instead, CS:S's level
//!   and its volume for the pace (`even_step`: a landing is a jump's
//!   step, sprinting runs; the perks still scale it, the model's share
//!   doesn't).
//! - **Your voices** (`voice`): the kill voice, the sprint and idle sounds
//!   (and the HUD's heartbeat), local (Corrections 2026-10-22, 8).
//! - Hurt sounds, footsteps and voices are only our characters': one
//!   whose `slots::GameIdentity` is another game's (the party apps,
//!   `chimera_sim::identity`) sounds as that game's.
//! - **Liquids** (`volumes::in_liquid`, 5, 8, 11): an impact whose point
//!   plus normal is in a liquid volume (`submerge`) plays its sound at half
//!   volume and the surface's under-water group in place of its burst
//!   (none in stock data), and draws no tracer; casings in water fall at
//!   half gravity, keep a quarter per bounce and make no sound; a footstep
//!   on an unknown surface in water is Water's; grenade bounces in water
//!   are at half volume (`bounce_volume`, with the contact's loudness).
//! - **Stepping** (`step_effects`): decals age (frozen while paused),
//!   tracers fly, casings fall and bounce (`rules::Casing`; the first
//!   bounce's sound), effect groups play (`fx::CaFx` on Chimera's
//!   particle pool, from the requests `CaFxRequest`); groups and sounds
//!   that follow an entity (a projectile's look in flight, a face's eyes)
//!   are `attached`'s.
//!
//! The front end (`ui::effects`) draws decals, tracers, casings, lights
//! and the first-person muzzle flash; `sounds` plays `CaSound`s.
//!
//! Across games (`weapon::impact`): the impacts are drawn where Chimera's
//! route gives them to Combat Arms (its guns' where its looks are on the
//! map; another game's gun's, as `guest_fx`'s full metal jacket round,
//! where that game's can't be); its surface types go to and from
//! Chimera's classes by `SURFACES`.
//!
//! The follow-up answers (Open questions 1, 4, 6, 8, 17-25): others'
//! shot sounds start at their weapon (the fire point); dry fire is
//! silent; several camera jitters add up; the mission bomb's show and
//! beeps are `bomb`'s; the rules for the effect file's curves, cone
//! emitters, bounces and placement modes, a playing sound's gain and
//! low-pass, the 255-sound cap, the kill call-outs and map sounds are
//! `rules`' (below, what of them draws or plays).
//!
//! Known shortcuts (the spec's most literal reading where unclear;
//! docs/tech-debt.md):
//! - Third-person points: others' muzzle and breach (casing start) and
//!   their shots' sound are at their posed `RightHand` socket
//!   (`characters::CaPose`; the held model's own `Flash` and `Breach`
//!   sockets aren't applied); a launcher's muzzle group and your own
//!   FireFX group use the held model's own `Flash` socket as posed
//!   (`CaPose::flash`, the hand's where its model isn't loaded);
//!   the world's box for a launcher's compressed fire point is the box
//!   around its surface-tagged triangles (`SurfaceGrid::bounds`); hit-zone surfaces for others' shots on a
//!   network client are the body's (6): a client sees no hit zones.
//! - Liquids: the ammo's own under-water record (`UWBullet`) isn't
//!   swapped in and no bubble trail is drawn (section 6a); rays pass a
//!   water surface (as the game's); exit decals and debris are off (as in
//!   all stock records).
//! - The world's surface types come by texture (`library::tag_surfaces`);
//!   props and doors are the world's; movable objects take no decals
//!   (none move yet), and no server-side decals on movers.
//! - Others' footstep animation rate is 1 by the spec (characters.md 7),
//!   not for lack of animations (the stock bodies' load); a network client
//!   takes others' gait from their movement state as replicated.
//! - Kill call-outs are rules only (`rules::callouts`): the voice sets'
//!   files aren't named; map `SoundFX` objects are `music::ambient`'s
//!   (`rules::SoundFxState` keeps the rules' view of them); a playing
//!   sound's gain isn't updated as the listener moves and no 255-sound cap
//!   is kept (Chimera's one-shots: `rules::PlayingSound`, `LiveSounds`
//!   say what the game does); interface sounds are the UI's
//!   (`ui::desk`) but for the bomb's.
//! - Critical hits, ricochets, the pusher, screen tints and blast lights
//!   are absent or inert in the stock data (Open question 23) and not
//!   modelled.

pub mod attached;
pub mod bomb;
pub mod data;
pub mod fx;
pub mod library;
pub mod rules;
pub mod sounds;
pub mod trail;
pub mod voice;

use std::collections::HashMap;

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;

use self::{
    data::{AmmoKind, CaFxTables, CaWeaponFx, EFFECTS, ImpactFx},
    fx::{CaFx, effect_frame},
    rules::{Casing, CasingHit, CasingStep, DingQueue, Gait, HurtKind, HurtLimiter, OwnSteps, StepInputs, TracerCounter, TracerPath},
    sounds::{CHANNEL_FIRE, CHANNEL_GRENADE, CHANNEL_IMPACT, CHANNEL_SHELL, CaSound, CaSoundFiles},
};
use super::{
    attachments::{CaMods, MUZZLE, ObserverSound},
    characters::records::Selection,
    grenades::{CaBlastSurface, CaGrenade},
    inventory::UNIT,
    view_model::{CaViewKey, CaViewModel, machine::ViewKey},
    volumes::{CaVolume, in_liquid},
    weapons::CaZoom,
};
use crate::{
    core::{Damage, DamageKind, Health, Hitgroup, Intent, LocalPlayer, MovementState, SOLID_LAYERS, Team, authoritative},
    map::{
        particles::{ParticleMaterials, Particles, WorldTracer},
        SurfaceNames,
        sound::SurfaceGrid,
    },
    slots::GameIdentity,
    weapon::{
        Inventory, Weapon, WeaponEvent, WeaponEventKind, aim_of, data::WeaponDataAppExt, grenade::Detonated,
        remote::RemoteShot,
    },
};

/// The player's effect options (the game's console variables).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct CaFxSettings {
    /// `Tracers`.
    pub tracers: bool,
    /// `ShellCasings`.
    pub shell_casings: bool,
    /// `MarkShow`.
    pub mark_show: bool,
    /// Play fly-bys where the original does (Quirk 6: near the world's
    /// origin); off: at the nearest point of the shot's line.
    pub original_flyby: bool,
}

impl Default for CaFxSettings {
    fn default() -> Self {
        Self {
            tracers: true,
            shell_casings: true,
            mark_show: true,
            original_flyby: false,
        }
    }
}

/// An effect group to play (whether or not the effect file has it: a
/// missing group plays nothing).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct CaFxRequest {
    pub group: String,
    /// Metres.
    pub at: Vec3,
    /// The effect frame's +Z (a surface normal or the shot's direction).
    pub normal: Vec3,
    pub kind: FxKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FxKind {
    /// Another's muzzle (third person).
    Muzzle,
    /// Your own FireFX group at your flash point.
    OwnFire,
    /// Your own launcher's FireFX group, parented to the shooter
    /// (special_weapons.md Corrections (2026-10-31) 1): made at his point,
    /// each key placing itself by its own mode.
    OwnParented(Entity),
    /// An ImpactFX's own group (smoke, flash).
    Ammo,
    /// A surface's impact burst (blood on characters).
    Surface,
    /// A surface's grenade explosion.
    Grenade,
}

/// A tracer flying.
#[derive(Clone, Debug, PartialEq)]
pub struct CaTracer {
    /// Units.
    pub path: TracerPath,
    pub look: data::TracerFx,
    pub age: f32,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct CaTracers(pub Vec<CaTracer>);

/// Tracer counters by weapon entity.
#[derive(Resource, Clone, Debug, Default)]
pub struct CaTracerCounters(pub HashMap<Entity, TracerCounter>);

/// A casing (section 8): its motion in units, drawn at its `Transform`
/// (metres) with its FireFX's model.
#[derive(Component, Clone, Debug)]
pub struct CaShell {
    pub owner: Entity,
    /// The gun it came from: its first bounce sounds on that gun's shell
    /// channel (`sounds::CHANNEL_SHELL`; None: no channel).
    pub weapon: Option<Entity>,
    pub casing: Casing,
    /// The FireFX record (its model, skin, scale).
    pub fire: String,
    pub serial: u64,
    /// The first bounce's sound: the surface's.
    pub sounded: bool,
}

impl CaShell {
    /// Its scale (`ShellScale` × the growth).
    pub fn scale(&self) -> f32 {
        self.casing.scale()
    }
}

/// Clips' lengths (s) by file, for the server's hurt limit.
#[derive(Resource, Default)]
pub struct CaSoundLengths(pub HashMap<String, f32>);

/// Others' footstep clocks (s of their gait's animation) and last place.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct OthersSteps {
    pub clock: f32,
    pub last: Option<Vec3>,
    pub airborne: bool,
}

/// Where the effects run in `Update` (the front end draws after it).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaEffectsSet;

pub struct CaEffectsPlugin;

impl Plugin for CaEffectsPlugin {
    fn build(&self, app: &mut App) {
        use crate::{map::impact::SurfaceClassesAppExt, net::NetAppExt, weapon::impact::ImpactAppExt};
        app.net_part("combat_arms_flight_looks", attached::register);
        app.register_impact_provider(crate::game::GAME)
            .register_surface_classes(SURFACES);
        app.register_weapon_hook::<CaWeaponFx>(EFFECTS, |e, _, p| {
            e.insert(p.clone());
        })
        .insert_resource(CaFxTables::built_in().clone())
        .init_resource::<CaFxSettings>()
        .init_resource::<rules::CaMarks>()
        .init_resource::<CaTracers>()
        .init_resource::<CaTracerCounters>()
        .init_resource::<CaFx>()
        .init_resource::<DingQueue>()
        .init_resource::<KnownWeapons>()
        .init_resource::<HurtLimiter>()
        .init_resource::<CaSoundFiles>()
        .init_resource::<CaSoundLengths>()
        .add_message::<crate::game::grenades::CaBlastSurface>()
        .add_message::<CaSound>()
        .add_message::<CaFxRequest>()
        .add_message::<RemoteShot>()
        .add_message::<crate::map::GameSound>()
        .add_message::<crate::map::PlaySound>()
        .add_message::<crate::map::SoundControl>()
        .add_message::<crate::game::view_model::attached::CaLoopSound>()
        .init_resource::<crate::game::rungs::InfectionSound>()
        .add_observer(crate::game::rungs::infection_moment)
        .add_observer(attached::on_shown)
        .add_observer(attached::fly_sound_stop)
        .add_observer(attached::face_groups)
        // One-frame particles (attached sprites, trails, flares) are drawn
        // on the frame they are added: before the pool steps.
        .configure_sets(Update, CaEffectsSet.before(crate::map::particles::ParticleSet::Step))
        .add_systems(
            Update,
            (
                play_dings,
                remember_weapons,
                shot_effects,
                forget_weapons,
                key_sounds,
                crate::game::grenades::callout::hear,
                grenade_effects,
                bomb::bomb_effects,
                footsteps,
                voice::voices,
                step_effects,
                attached::follow,
                attached::flight,
                attached::projectile_sounds,
                spawn_fx,
                sounds::play,
                crate::game::view_model::attached::sounds,
                sounds::loops,
            )
                .chain()
                .in_set(CaEffectsSet)
                .after(crate::map::DriveAnimation),
        )
        .add_systems(
            Update,
            (hurt_sounds, bounce_sounds)
                .run_if(authoritative)
                .in_set(CaEffectsSet)
                .before(sounds::play),
        );
    }
}

// ---------------------------------------------------------------------------
// The camera

/// The camera effects are seen from: the local player's eye and view.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// Metres.
    pub at: Vec3,
    pub forward: Vec3,
    pub player: Entity,
}

impl Camera {
    /// The gates (section 2) for a fire point and an impact (metres).
    pub fn gates(&self, fire: Vec3, impact: Vec3, others: bool) -> rules::Gates {
        rules::gates(self.at / UNIT, self.forward, fire / UNIT, impact / UNIT, others)
    }
}

type CameraQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform, Option<&'static MovementState>, &'static Intent), With<LocalPlayer>>;

fn camera(q: &CameraQuery) -> Option<Camera> {
    let (e, t, state, intent) = q.iter().next()?;
    Some(Camera {
        at: t.translation + state.map_or(Vec3::ZERO, |s| s.eye_offset),
        forward: aim_of(intent.yaw, intent.pitch) * Vec3::NEG_Z,
        player: e,
    })
}

/// The surface type at a world point (metres): the nearest tagged drawn
/// triangle within 4 u; none: Unknown (0).
pub fn world_surface(grid: Option<&SurfaceGrid>, at: Vec3) -> u16 {
    world_surface_named(grid, None, at)
}

/// `world_surface` on a map whose surfaces may be another game's
/// (`names`: a mashup's, `map::SurfaceNames`).
pub fn world_surface_named(grid: Option<&SurfaceGrid>, names: Option<&SurfaceNames>, at: Vec3) -> u16 {
    grid.and_then(|g| g.nearest(at, 4.0 * UNIT))
        .map_or(0, |s| surface_type(s, names, CaFxTables::built_in()))
}

/// A world triangle's surface (its `MapMesh::surface`) as a type: Combat
/// Arms' own maps tag the type's number (`library::tag_surfaces`);
/// another game's map keeps its own names, which the app's table gives
/// as Combat Arms' type names (`map::SurfaceNames`), found by name in
/// `tables`; anything else: Unknown (0).
pub fn surface_type(s: &str, names: Option<&SurfaceNames>, tables: &CaFxTables) -> u16 {
    s.parse()
        .ok()
        .or_else(|| tables.surface_id(names.map_or(s, |n| n.resolve(s))))
        .unwrap_or(0)
}

/// The effects another game's gun's impacts take when Combat Arms draws
/// them (`weapon::impact`: that game can't on this map): a full metal
/// jacket round's (the rifles' and pistols' `BulletFmj`), no tracer or
/// fly-by.
pub fn guest_fx() -> CaWeaponFx {
    CaWeaponFx {
        ammo_kind: AmmoKind::Bullet,
        impact: "BulletFmj".into(),
        ..Default::default()
    }
}

/// Combat Arms' surface types by class (`map::impact`): the types of
/// data/effects.toml by id (the world's `SurfaceGrid` names), else by
/// name (another map's names through `SurfaceNames`), and the type each
/// class takes on other games' maps.
pub const SURFACES: crate::map::impact::GameSurfaces = crate::map::impact::GameSurfaces {
    game: crate::game::GAME,
    classify: |name| {
        use crate::map::impact::SurfaceClass as C;
        Some(match name.trim().parse::<u16>().ok()? {
            4..=7 => C::Flesh,
            10 | 90 => C::Tile,
            11 => C::Concrete,
            20 | 101 | 200 => C::Metal,
            30 | 100 => C::Wood,
            40 | 41 => C::Glass,
            60 => C::Sand,
            70 | 74 => C::Dirt,
            71 => C::Snow,
            73 => C::Grass,
            80 | 81 => C::Cloth,
            201 | 204 => C::Water,
            _ => return None,
        })
    },
    name: |class| {
        use crate::map::impact::SurfaceClass as C;
        Some(match class {
            C::Concrete | C::Default => "11",
            C::Metal => "20",
            C::Wood => "30",
            C::Dirt => "70",
            C::Sand => "60",
            C::Grass => "73",
            C::Snow => "71",
            C::Glass => "40",
            C::Tile => "10",
            C::Plastic => "90",
            C::Cloth => "81",
            C::Flesh => "6",
            C::Water => "204",
        })
    },
};

/// The world's surfaces as Combat Arms' types (`surface_type`): the map's
/// tagged triangles and, on another game's map, its names as Combat
/// Arms'.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Ground<'w> {
    grid: Option<Res<'w, SurfaceGrid>>,
    names: Option<Res<'w, SurfaceNames>>,
    classes: Option<Res<'w, crate::map::impact::SurfaceClasses>>,
    impacts: Option<Res<'w, crate::map::impact::ActiveImpacts>>,
}

impl Ground<'_> {
    /// The box around the world (metres), where the map has surfaces.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        self.grid.as_deref()?.bounds()
    }

    /// The type of surface `s`: as `surface_type`, and where that finds
    /// none (another game's surface the app's table doesn't name, which
    /// would be Unknown, with no bullet hole), the type of its class
    /// (`map::impact::SurfaceClasses`).
    pub fn ty(&self, s: &str) -> u16 {
        let named = s
            .parse()
            .ok()
            .or_else(|| CaFxTables::built_in().surface_id(self.names.as_deref().map_or(s, |n| n.resolve(s))));
        named
            .or_else(|| {
                let map_game = self.impacts.as_deref().and_then(|i| i.0.map_game());
                self.classes
                    .as_deref()?
                    .translate(map_game, s, crate::game::GAME)?
                    .parse()
                    .ok()
            })
            .unwrap_or(0)
    }

    /// `world_surface`'s: the nearest tagged triangle within 4 u.
    pub fn at(&self, at: Vec3) -> u16 {
        self.grid.as_deref().and_then(|g| g.nearest(at, 4.0 * UNIT)).map_or(0, |s| self.ty(s))
    }

    /// The nearest tagged triangle's type within `reach` (metres).
    pub fn nearest(&self, at: Vec3, reach: f32) -> Option<u16> {
        self.grid.as_deref()?.nearest(at, reach).map(|s| self.ty(s))
    }

    /// The type of the triangle below `at` within `reach` (metres).
    pub fn below(&self, at: Vec3, reach: f32) -> Option<u16> {
        self.grid.as_deref()?.below(at, reach).map(|s| self.ty(s))
    }
}

/// A deterministic stream of draws in 0..1.
#[derive(Default)]
pub struct Rolls(u64);

impl Rolls {
    pub fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

// ---------------------------------------------------------------------------
// Shots

/// What the weapons a shot names say.
type WeaponFxQuery<'w, 's> =
    Query<'w, 's, (&'static Weapon, Option<&'static CaWeaponFx>, Option<&'static CaMods>, Option<&'static CaZoom>)>;

/// What a weapon seen here said (its effects and fitted parts), kept past
/// its despawn: the gun game swaps the gun on the kill's tick, so the
/// killing round's events (`Update`) name a gun already gone (playtest
/// 2026-10-28: its fire sound was lost). By entity until the frame it is
/// despawned in has drawn its events (`forget_weapons`), and by weapon ID
/// for others' rounds a network client hears after their gun changed.
#[derive(Resource, Default)]
pub struct KnownWeapons {
    by_entity: std::collections::BTreeMap<Entity, KnownWeapon>,
    by_id: std::collections::BTreeMap<&'static str, KnownWeapon>,
}

#[derive(Clone)]
struct KnownWeapon {
    fx: Option<CaWeaponFx>,
    mods: Option<CaMods>,
}

impl KnownWeapons {
    /// What weapon event `e` was made with, the weapon gone or not.
    fn of<'a>(
        &'a self,
        weapons: &'a WeaponFxQuery,
        e: &WeaponEvent,
    ) -> Option<(Option<&'a CaWeaponFx>, Option<&'a CaMods>, Option<&'a CaZoom>)> {
        if let Ok((_, fx, mods, zoom)) = weapons.get(e.weapon) {
            return Some((fx, mods, zoom));
        }
        let known = self.by_entity.get(&e.weapon).or_else(|| match e.kind {
            WeaponEventKind::Fired { id, .. } => self.by_id.get(id),
            _ => None,
        })?;
        Some((known.fx.as_ref(), known.mods.as_ref(), None))
    }
}

#[allow(clippy::type_complexity)]
fn remember_weapons(
    new: Query<
        (Entity, &Weapon, Option<&CaWeaponFx>, Option<&CaMods>),
        Or<(Added<Weapon>, Changed<CaWeaponFx>, Changed<CaMods>)>,
    >,
    mut known: ResMut<KnownWeapons>,
) {
    for (e, w, fx, mods) in &new {
        let k = KnownWeapon {
            fx: fx.cloned(),
            mods: mods.cloned(),
        };
        known.by_id.insert(w.id, k.clone());
        known.by_entity.insert(e, k);
    }
}

/// Weapons despawned this frame, their events drawn: forgotten by entity.
fn forget_weapons(mut gone: RemovedComponents<Weapon>, mut known: ResMut<KnownWeapons>) {
    for e in gone.read() {
        known.by_entity.remove(&e);
    }
}

/// The fire voice of an event's shooter and weapon: one per attacker and
/// weapon, shared by both buttons (a new fire or alt-fire sound stops
/// the last; another weapon's or player's doesn't: effects_sounds.md
/// Corrections (2026-10-30) 2). The weapon entity is the voice's source
/// (the shooter's when the gun is gone: others' rounds after a swap).
pub fn fire_voice(e: &WeaponEvent) -> Entity {
    if e.weapon == Entity::PLACEHOLDER { e.owner } else { e.weapon }
}

/// The fire sound of a weapon for its own shooter (`own`) or others: the
/// file and radius (u); None: nothing (a silenced part with no level).
pub fn fire_sound<'a>(fx: &'a CaWeaponFx, mods: Option<&CaMods>, own: bool) -> Option<(&'a str, f32)> {
    fire_sound_of(fx, mods, own, false)
}

/// `fire_sound` for a shot or swing started with the secondary button
/// when `alt` (the attacker's own client only: others pass false, Quirk
/// 10): `Alt_FireSnd` when the weapon names one, a silenced file still
/// winning (effects_sounds.md Corrections (2026-10-30) 2).
pub fn fire_sound_of<'a>(fx: &'a CaWeaponFx, mods: Option<&CaMods>, own: bool, alt: bool) -> Option<(&'a str, f32)> {
    let file = if alt && !fx.alt_fire_sound.is_empty() { &fx.alt_fire_sound } else { &fx.fire_sound };
    let plain = (file.as_str(), mods.map_or(fx.fire_radius, |m| m.fire_sound_radius as f32));
    let Some(m) = mods else {
        return (!plain.0.is_empty()).then_some(plain);
    };
    let level = if own {
        m.fire_sound().silenced
    } else {
        match m.observer_sound() {
            ObserverSound::Plain => None,
            ObserverSound::Silenced { level } => Some(level),
            ObserverSound::Nothing => return None,
        }
    };
    let out = match level {
        Some(l) => (
            fx.silenced_sounds.get(l as usize - 1)?.as_str(),
            m.silenced_radius.get(l as usize - 1).copied().unwrap_or(m.fire_sound_radius) as f32,
        ),
        None => plain,
    };
    (!out.0.is_empty()).then_some(out)
}

/// The tracer frequency F a shooter's own client counts with: the last
/// fitted part's (slot order) when it names a tracer, else the ammo's. A
/// later fitted part with no tracer resets the choice to none
/// (Corrections 2026-10-22, 3).
pub fn own_tracer_frequency(t: &CaFxTables, fx: &CaWeaponFx, mods: Option<&CaMods>) -> Option<u32> {
    let ammo = t.tracer(&fx.tracer).map(|r| r.frequency);
    let part = mods.and_then(|m| m.fitted().last()).and_then(|p| {
        t.attachment(&p.record.name)
            .and_then(|a| a.tracer.as_deref())
            .and_then(|n| t.tracer(n))
            .map(|r| r.frequency)
    });
    part.or(ammo)
}

/// The tracer's look: the muzzle part's tracer if it names one, else the
/// ammo's; None when the ammo has no tracer (then nothing is drawn).
pub fn tracer_look<'a>(t: &'a CaFxTables, fx: &CaWeaponFx, mods: Option<&CaMods>) -> Option<&'a data::TracerFx> {
    let ammo = t.tracer(&fx.tracer)?;
    let muzzle = mods
        .and_then(|m| m.part(MUZZLE))
        .and_then(|p| t.attachment(&p.record.name))
        .and_then(|a| a.tracer.as_deref())
        .and_then(|n| t.tracer(n));
    Some(muzzle.unwrap_or(ammo))
}

/// The third-person muzzle group: the muzzle part's, else (a flash hider of
/// level h, when the part names none) the weapon's third-person
/// flash-hider group h-1, else the weapon's (Corrections 2026-10-22, 12).
pub fn hh_muzzle<'a>(t: &'a CaFxTables, fx: &'a CaWeaponFx, mods: Option<&CaMods>) -> &'a str {
    let part = mods.and_then(|m| m.part(MUZZLE));
    if let Some(own) = part
        .and_then(|p| t.attachment(&p.record.name))
        .and_then(|a| a.hh_muzzle.as_deref())
    {
        return own;
    }
    part.map(|p| p.record.flash_hider_level)
        .filter(|l| *l >= 1)
        .and_then(|l| fx.hh_flash_hider.get(l as usize - 1))
        .map_or(&fx.hh_muzzle, String::as_str)
}

/// The first-person muzzle group: a muzzle part with a flash hider
/// (`FlashHiderLevel` h > 0, view_models.md 9, Open question 16) takes the
/// weapon's `PVFlashHiderFXName`h (`hider`, by level; none: no flash at
/// all); else the muzzle part's group, else the weapon's
/// (`PVMuzzleFXName`, the view model's `muzzle_fx`). Empty: no flash.
pub fn pv_muzzle<'a>(t: &'a CaFxTables, view: &'a str, hider: &'a [String], mods: Option<&CaMods>) -> &'a str {
    let part = mods.and_then(|m| m.part(MUZZLE));
    if let Some(level) = part.map(|p| p.record.flash_hider_level).filter(|l| *l > 0) {
        return hider.get(level as usize).map_or("", String::as_str);
    }
    part.and_then(|p| t.attachment(&p.record.name))
        .and_then(|a| a.pv_muzzle.as_deref())
        .unwrap_or(view)
}

/// The impact sound of a surface for an ammo kind (section 11): one of
/// four slots of its list (an empty slot: silence).
pub fn impact_sound<'a>(record: &'a ImpactFx, surface: &'a data::Surface, kind: AmmoKind, roll: f32) -> Option<&'a str> {
    if record.sound != "surface" {
        return (!record.sound.is_empty()).then_some(record.sound.as_str());
    }
    let list = match kind {
        AmmoKind::Bullet => &surface.bullet,
        AmmoKind::Melee => &surface.melee,
        AmmoKind::Thrown => &surface.projectile,
    };
    rules::draw_slot(list, 4, roll)
}

/// One impact's decisions (section 6), for the systems and the tests.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Impact {
    pub sound: Option<(String, f32)>,
    pub ammo_fx: Option<String>,
    /// Decal sprite and width (u).
    pub mark: Option<(String, f32)>,
    pub burst: Option<String>,
    /// Under water (5): the sound at half volume.
    pub under_water: bool,
}

/// The impact sound's volume (0..100): 50 under water (11).
impl Impact {
    pub fn sound_volume(&self) -> f32 {
        if self.under_water { 50.0 } else { 100.0 }
    }
}

/// An impact under water (5: its point + normal inside a liquid volume):
/// the surface's under-water group (`UWImpactFXName`, `UWGrenadeFXName`
/// for thrown ammo; none in stock data) in place of its burst, when the
/// impact is seen and the record does surface effects; half volume for
/// its sound. The ammo's own under-water record isn't swapped in (its
/// contents aren't in the spec: the record is kept).
pub fn submerge(out: &mut Impact, record: &ImpactFx, surface: &data::Surface, kind: AmmoKind, seen: bool) {
    out.under_water = true;
    if seen && record.surface_fx && !surface.sky {
        let group = if kind == AmmoKind::Thrown { &surface.uw_grenade_fx } else { &surface.uw_impact_fx };
        out.burst = (!group.is_empty()).then(|| group.clone());
    }
}

/// What an impact on surface `surface` with record `record` makes, by
/// section 6's order and gates. `decals_take`: the hit object takes decals
/// (the static world); `own_far_unzoomed`: your own shot at a far impact
/// while not zoomed; rolls for the sound slot and the decal size.
#[allow(clippy::too_many_arguments)]
pub fn impact(
    record: &ImpactFx,
    surface: &data::Surface,
    kind: AmmoKind,
    seen: bool,
    decals_take: bool,
    own_far_unzoomed: bool,
    settings: &CaFxSettings,
    rolls: [f32; 2],
) -> Impact {
    let mut out = Impact::default();
    if surface.sky {
        return out;
    }
    out.sound = impact_sound(record, surface, kind, rolls[0]).map(|s| (s.to_string(), record.sound_radius));
    if !record.fx.is_empty() && (seen || record.area) {
        out.ammo_fx = Some(record.fx.clone());
    }
    if seen {
        if record.create_mark
            && surface.shows_mark
            && settings.mark_show
            && decals_take
            && !own_far_unzoomed
            && !surface.character
        {
            let sprite = if record.mark == "surface" { surface.mark.clone() } else { record.mark.clone() };
            if !sprite.is_empty() {
                out.mark = Some((sprite, rules::mark_width(record.mark_scale, surface.mark_scale, rolls[1])));
            }
        }
        if record.surface_fx {
            let group = if kind == AmmoKind::Thrown { &surface.grenade_fx } else { &surface.impact_fx };
            if !group.is_empty() {
                out.burst = Some(group.clone());
            }
        }
    }
    out
}

#[derive(Default)]
struct ShotState {
    rolls: Rolls,
    serial: u64,
}

/// A round that left a barrel this frame: its first ray may carry the
/// tracer.
struct Round {
    fire: Vec3,
    first_ray: bool,
    /// The muzzle (or own FireFX) group the round's first weapon effect
    /// made: every further pellet's weapon effect makes it again
    /// (Corrections 2026-10-22, 10).
    muzzle: Option<CaFxRequest>,
    /// Rays (pellets) seen so far.
    rays: u32,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn shot_effects(
    (mut events, mut remote, holders, known): (
        MessageReader<WeaponEvent>,
        MessageReader<RemoteShot>,
        Query<&Inventory>,
        Res<KnownWeapons>,
    ),
    cameras: CameraQuery,
    weapons: WeaponFxQuery,
    characters: Query<(Has<Health>, Has<Intent>, Option<&CaViewModel>, Option<&Transform>)>,
    hands: Query<(&super::characters::CaPose, &Transform, &MovementState)>,
    grid: Ground,
    tables: Res<CaFxTables>,
    settings: Res<CaFxSettings>,
    mut marks: ResMut<rules::CaMarks>,
    mut tracers: ResMut<CaTracers>,
    mut counters: ResMut<CaTracerCounters>,
    mut dings: ResMut<DingQueue>,
    mut sounds: MessageWriter<CaSound>,
    mut fx: MessageWriter<CaFxRequest>,
    (mut state, volumes, protected, route, show_local): (
        Local<ShotState>,
        Query<&CaVolume>,
        Query<(), With<super::rules::spawns::CaProtected>>,
        crate::weapon::impact::ImpactRoute,
        Option<Res<crate::map::ShowLocalBody>>,
    ),
    mut commands: Commands,
) {
    // Others' rounds on a network client arrive as `RemoteShot`s (their
    // traces as shots): each is a round fired, with the gun in hand when
    // it is the round's, else (swapped since) the gun known by its ID.
    let fired: Vec<WeaponEvent> = remote
        .read()
        .map(|r| {
            let held = holders.get(r.shooter).ok().and_then(|i| i.active);
            WeaponEvent {
                owner: r.shooter,
                weapon: held
                    .filter(|w| weapons.get(*w).is_ok_and(|x| x.0.id == r.weapon))
                    .unwrap_or(Entity::PLACEHOLDER),
                kind: WeaponEventKind::Fired {
                    id: r.weapon,
                    origin: r.origin,
                    yaw: r.yaw,
                    pitch: r.pitch,
                    seed: r.seed,
                    spread: r.spread,
                    mode: r.mode,
                },
                replay: false,
            }
        })
        .collect();
    let events: Vec<&WeaponEvent> = fired.iter().chain(events.read().filter(|e| e.shown())).collect();
    if events.is_empty() {
        return;
    }
    let cam = camera(&cameras);
    let me = cam.map(|c| c.player);
    // Head hits this frame (the blood's surface type).
    let heads: Vec<Entity> = events
        .iter()
        .filter_map(|e| match e.kind {
            WeaponEventKind::Hit {
                target,
                hitgroup: Hitgroup::Head,
                ..
            } => Some(target),
            _ => None,
        })
        .collect();
    let mut rounds: HashMap<Entity, Round> = HashMap::new();
    let guest_fx = guest_fx();
    for e in events {
        let mine = Some(e.owner) == me;
        // The gun may be gone (the gun game swaps it on the kill):
        // what it was (`KnownWeapons`).
        let Some((wfx, mods, zoom)) = known.of(&weapons, e) else { continue };
        // Combat Arms draws this event's impacts (`weapon::impact`: its
        // guns' where its looks are on the map; another game's gun's where
        // that game's can't be drawn).
        let draws = route.draws(crate::game::GAME, e);
        let guest = wfx.is_none();
        let wfx = match wfx {
            Some(w) => w,
            // Another game's gun: its impacts alone, as a full metal
            // jacket round's (`guest_fx`).
            None if draws
                && matches!(e.kind, WeaponEventKind::Shot { .. } | WeaponEventKind::ShotContinued { .. }) =>
            {
                &guest_fx
            }
            None => continue,
        };
        let fire_record = wfx.fire_of(&tables);
        match &e.kind {
            WeaponEventKind::Fired { origin, yaw, pitch, .. } => {
                let dir = aim_of(*yaw, *pitch) * Vec3::NEG_Z;
                let vm = characters.get(e.owner).ok().and_then(|c| c.2);
                // Others': their weapon's socket (`RightHand`, characters.md
                // 3), else the shot's origin.
                let hand = hands
                    .get(e.owner)
                    .ok()
                    .and_then(|(p, t, s)| Some(p.world(t, s, UNIT, p.right_hand?)));
                // The held launcher's own `Flash` socket as posed this tick
                // (the hand's where its model has none).
                let flash = hands
                    .get(e.owner)
                    .ok()
                    .and_then(|(p, t, s)| Some(p.world(t, s, UNIT, p.flash_point()?)));
                // A launcher's (a projectile round's) fire-only event comes
                // from the server's fire position (the shooter's camera when
                // he fired), compressed to the world's grid; his own client
                // takes it as received, the others' replace it with the held
                // launcher's `Flash` socket (special_weapons.md Corrections
                // (2026-10-31) 1). The shooter in a third-person camera is
                // one of the others.
                let launcher = wfx.ammo_kind == AmmoKind::Thrown;
                let world_box = grid.bounds();
                let own = mine && (!launcher || !show_local.as_ref().is_some_and(|s| s.0));
                let fire = if launcher {
                    let sent = world_box.map_or(*origin, |b| rules::compress_position(*origin, b));
                    if own { sent } else { flash.unwrap_or(sent) }
                } else if mine {
                    vm.map_or(*origin, |v| v.flash_point)
                } else {
                    hand.unwrap_or(*origin)
                };
                let mut round = Round {
                    fire,
                    first_ray: true,
                    muzzle: None,
                    rays: 0,
                };
                // The fire sound (a launcher's event carries none for the
                // others: it is ignored for a player's projectile).
                if (!launcher || mine)
                    && let Some((file, radius)) = fire_sound(wfx, mods, mine)
                {
                    let silenced = mods.is_some_and(|m| m.silencer_level().is_some());
                    // Others' shots sound at their weapon (Open question 18).
                    let mut s = if mine { CaSound::local(file) } else { CaSound::at(file, fire, radius) };
                    if !silenced {
                        s = s.from_source(fire_voice(e), CHANNEL_FIRE);
                    }
                    sounds.write(s);
                }
                let Some(cam) = cam else {
                    rounds.insert(e.owner, round);
                    continue;
                };
                let gates = cam.gates(fire, fire + dir, !mine);
                if own {
                    // The shooter's own launcher group shows only when the
                    // received point passes the view test from the camera as
                    // it is when the event arrives.
                    let seen = !launcher || gates.fire_seen;
                    if seen && let Some(f) = fire_record.filter(|f| !f.fx.is_empty()) {
                        round.muzzle = Some(CaFxRequest {
                            group: f.fx.clone(),
                            at: fire,
                            normal: dir,
                            kind: if launcher { FxKind::OwnParented(e.owner) } else { FxKind::OwnFire },
                        });
                    }
                } else if gates.fire_seen {
                    let group = hh_muzzle(&tables, wfx, mods);
                    if !group.is_empty() {
                        // A launcher's event has no impact position: the
                        // muzzle group turns toward the world origin.
                        let normal = if launcher {
                            (rules::unset_impact(world_box) - fire).normalize_or(dir)
                        } else {
                            dir
                        };
                        round.muzzle = Some(CaFxRequest {
                            group: group.to_string(),
                            at: fire,
                            normal,
                            kind: FxKind::Muzzle,
                        });
                    }
                }
                if let Some(m) = &round.muzzle {
                    fx.write(m.clone());
                }
                rounds.insert(e.owner, round);
                // A casing (8).
                if let Some(f) = fire_record
                    && f.shells
                    && (own || !launcher)
                    && settings.shell_casings
                    && gates.fire_seen
                    && !gates.fire_far
                {
                    let breach = if mine { vm.map_or(fire, |v| v.breach_point) } else { fire };
                    // The camera's frame (first person) or the shot's: the
                    // same view here.
                    let frame = aim_of(*yaw, *pitch);
                    state.serial += 1;
                    let serial = state.serial;
                    spawn_casing(&mut commands, &mut state.rolls, (e.owner, Some(e.weapon)), &wfx.fire, f, breach, frame, mine, serial);
                }
            }
            WeaponEventKind::Shot { from, to, hit, normal } | WeaponEventKind::ShotContinued { from, to, hit, normal } => {
                let first = matches!(e.kind, WeaponEventKind::Shot { .. });
                let round = rounds.get_mut(&e.owner);
                let fire = round.as_ref().map_or(*from, |r| r.fire);
                let dir = (*to - *from).normalize_or_zero();
                let n = normal.unwrap_or(-dir);
                // The impact point, backed off 1 u along the ray; under
                // water when it + the normal is in a liquid (5).
                let at = *to - dir * UNIT;
                let wet = hit.is_some() && in_liquid(volumes.iter(), at + n * UNIT);
                // 4: the tracer, once per ray, drawn on a round's first.
                if first && !guest && wfx.ammo_kind == AmmoKind::Bullet {
                    let f = if mine {
                        own_tracer_frequency(&tables, wfx, mods)
                    } else {
                        tables.tracer(&wfx.tracer).map(|t| t.frequency)
                    };
                    let first_ray = round.is_some_and(|r| std::mem::take(&mut r.first_ray));
                    if let Some(f) = f
                        && counters.0.entry(e.weapon).or_default().ray(f)
                        && first_ray
                        && settings.tracers
                        && !wet
                        && let Some(look) = tracer_look(&tables, wfx, mods)
                        && let Some(path) = TracerPath::new(fire / UNIT, *to / UNIT, look.velocity, look.length)
                    {
                        if tracers.0.len() >= rules::TRACER_LIST_MAX {
                            tracers.0.remove(0);
                        }
                        tracers.0.push(CaTracer {
                            path,
                            look: look.clone(),
                            age: 0.0,
                        });
                    }
                }
                // Every further pellet's weapon effect makes the muzzle group
                // again (Corrections 2026-10-22, 10).
                if first && let Some(r) = rounds.get_mut(&e.owner) {
                    if r.rays > 0
                        && let Some(m) = &r.muzzle
                    {
                        fx.write(m.clone());
                    }
                    r.rays += 1;
                }
                // 11: fly-by of others' bullets, whether or not the ray hit
                // anything (a miss is a sky impact at the ray's end;
                // Corrections 2026-10-22, 2).
                if first
                    && !mine
                    && !guest
                    && wfx.ammo_kind == AmmoKind::Bullet
                    && let Some(cam) = cam
                    && let Some(record) = tables.impact(&wfx.impact)
                    && let Some(near) = rules::flyby(cam.at / UNIT, fire / UNIT, *to / UNIT, record.sound_radius)
                    && let Some(file) = rules::draw_any(&tables.sounds.flyby, state.rolls.next())
                {
                    let at = if settings.original_flyby {
                        rules::flyby_quirk_position(cam.at / UNIT, near)
                    } else {
                        near
                    };
                    let [r, big] = tables.sounds.flyby_radii;
                    sounds.write(CaSound {
                        radii: (r, big),
                        ..CaSound::at(file, at * UNIT, big)
                    });
                }
                let Some(hit) = hit.as_ref().filter(|_| draws) else { continue };
                // A spawn-protected player absorbs the round: no blood, no
                // impact (game_modes.md S6, Open question 25).
                if protected.contains(*hit) {
                    continue;
                }
                let (has_health, is_character, ..) = characters.get(*hit).unwrap_or((false, false, None, None));
                let character = has_health && is_character;
                let ty = if character {
                    if heads.contains(hit) { 4 } else { 6 }
                } else {
                    grid.at(*to)
                };
                let surface = tables.surface(ty);
                let Some(record) = tables.impact(&wfx.impact) else { continue };
                let gates = cam.map(|c| c.gates(fire, at, !mine)).unwrap_or_default();
                let zoomed = zoom.is_some_and(|z| z.level > 0);
                let rolls = [state.rolls.next(), state.rolls.next()];
                let mut out = impact(
                    record,
                    surface,
                    wfx.ammo_kind,
                    gates.impact_seen,
                    !character,
                    mine && gates.impact_far && !zoomed,
                    &settings,
                    rolls,
                );
                if wet {
                    submerge(&mut out, record, surface, wfx.ammo_kind, gates.impact_seen);
                }
                emit_impact(&out, at, n, Some(e.weapon), &mut sounds, &mut fx, &mut marks, &mut state.rolls, record);
            }
            WeaponEventKind::Swing { at, secondary, .. } => {
                // special_weapons.md 5.1: each hit key's impact (from the
                // attacker's own ray, before that hit's fire sound), then
                // its fire sound, on a miss too (a miss is a sky impact:
                // silent).
                if let Some((point, n, hit)) = at.as_ref().filter(|_| draws) {
                    let (has_health, is_character, ..) = characters.get(*hit).unwrap_or((false, false, None, None));
                    // A character: its head surface on a head hit, else its body's.
                    let ty = if has_health && is_character {
                        if heads.contains(hit) { 4 } else { 6 }
                    } else {
                        grid.at(*point)
                    };
                    if let Some(record) = tables.impact(&wfx.impact) {
                        let seen = cam.is_some_and(|c| c.gates(*point, *point, !mine).impact_seen);
                        let rolls = [state.rolls.next(), state.rolls.next()];
                        let mut out = impact(record, tables.surface(ty), AmmoKind::Melee, seen, false, false, &settings, rolls);
                        if in_liquid(volumes.iter(), *point + *n * UNIT) {
                            submerge(&mut out, record, tables.surface(ty), AmmoKind::Melee, seen);
                        }
                        emit_impact(&out, *point, *n, Some(e.weapon), &mut sounds, &mut fx, &mut marks, &mut state.rolls, record);
                    }
                }
                // Yours: `Alt_FireSnd` for a secondary swing; others always
                // hear `FireSnd` (Quirk 10). One fire voice per player and
                // weapon: a hit's fire sound replaces the last.
                if let Some((file, radius)) = fire_sound_of(wfx, mods, mine, mine && *secondary) {
                    let origin = characters.get(e.owner).ok().and_then(|c| c.3).map_or(Vec3::ZERO, |t| t.translation);
                    let s = if mine { CaSound::local(file) } else { CaSound::at(file, origin, radius) };
                    sounds.write(s.from_source(fire_voice(e), CHANNEL_FIRE));
                }
            }
            // Dry fire never plays and makes no sound (Open question 25,
            // view_models.md C6).
            WeaponEventKind::DryFire => {}
            WeaponEventKind::Hit { target, .. } if mine => {
                let (has_health, is_character, ..) = characters.get(*target).unwrap_or((false, false, None, None));
                if has_health && is_character {
                    dings.push();
                }
            }
            _ => {}
        }
    }
}

/// Write an impact's sound, groups and decal. A gun's round's impact
/// sound plays on that gun's impact channel (`source`: the weapon,
/// `sounds::CHANNEL_IMPACT`), so its next round's replaces it.
#[allow(clippy::too_many_arguments)]
fn emit_impact(
    out: &Impact,
    at: Vec3,
    normal: Vec3,
    source: Option<Entity>,
    sounds: &mut MessageWriter<CaSound>,
    fx: &mut MessageWriter<CaFxRequest>,
    marks: &mut rules::CaMarks,
    rolls: &mut Rolls,
    record: &ImpactFx,
) {
    if let Some((file, radius)) = &out.sound {
        let s = CaSound::at(file, at, *radius).volume(out.sound_volume());
        sounds.write(match source {
            Some(w) => s.from_source(w, CHANNEL_IMPACT),
            None => s,
        });
    }
    if let Some(g) = &out.ammo_fx {
        fx.write(CaFxRequest {
            group: g.clone(),
            at,
            normal,
            kind: FxKind::Ammo,
        });
    }
    if let Some((sprite, width)) = &out.mark {
        marks.add(rules::Mark {
            at: at / UNIT,
            normal,
            angle: rolls.next() * std::f32::consts::TAU,
            width: *width,
            sprite: sprite.clone(),
            grey: record.color_ratio(),
            age: 0.0,
            serial: 0,
        });
    }
    if let Some(g) = &out.burst {
        fx.write(CaFxRequest {
            group: g.clone(),
            at,
            normal,
            kind: FxKind::Surface,
        });
    }
}

/// A casing from `breach` (metres) in the frame of `aim` (the camera's
/// first person, the shot's third).
#[allow(clippy::too_many_arguments)]
fn spawn_casing(
    commands: &mut Commands,
    rolls: &mut Rolls,
    (owner, weapon): (Entity, Option<Entity>),
    fire: &str,
    record: &data::FireFx,
    breach: Vec3,
    aim: Quat,
    first_person: bool,
    serial: u64,
) {
    let local = if record.velocity != [0.0; 3] {
        Vec3::from(record.velocity)
    } else {
        rules::casing_velocity([rolls.next(), rolls.next(), rolls.next()], false)
    };
    // (right, up, forward) in the frame's axes.
    let velocity = aim * Vec3::X * local.x + aim * Vec3::Y * local.y + aim * Vec3::NEG_Z * local.z;
    let mut spin = Vec2::ZERO;
    for s in [&mut spin.x, &mut spin.y] {
        let v = rules::SHELL_SPIN.0 + (rules::SHELL_SPIN.1 - rules::SHELL_SPIN.0) * rolls.next();
        *s = if rolls.next() < 0.5 { -v } else { v };
    }
    commands.spawn((
        Name::new("Casing"),
        CaShell {
            owner,
            weapon,
            casing: Casing {
                at: breach / UNIT,
                velocity,
                spin,
                angles: Vec2::ZERO,
                orientation: Quat::IDENTITY,
                bounces: 0,
                age: 0.0,
                resting: false,
                in_water: false,
                first_person,
            },
            fire: fire.to_string(),
            serial,
            sounded: false,
        },
        Transform::from_translation(breach).with_rotation(aim),
    ));
}

// ---------------------------------------------------------------------------
// View-model keys

#[allow(clippy::type_complexity)]
fn key_sounds(
    mut keys: MessageReader<CaViewKey>,
    cameras: CameraQuery,
    owners: Query<(&Inventory, Option<&Transform>, Option<&MovementState>, Option<&CaViewModel>, &Intent, Option<&Health>)>,
    weapons: Query<(Option<&CaWeaponFx>, Option<&CaMods>)>,
    tables: Res<CaFxTables>,
    settings: Res<CaFxSettings>,
    mut sounds: MessageWriter<CaSound>,
    mut state: Local<ShotState>,
    mut commands: Commands,
) {
    let me = camera(&cameras).map(|c| c.player);
    for k in keys.read() {
        let Ok((inv, t, ms, vm, intent, health)) = owners.get(k.owner) else { continue };
        let Some((Some(fx), mods)) = inv.active.and_then(|w| weapons.get(w).ok()) else { continue };
        let mine = Some(k.owner) == me;
        match k.key {
            ViewKey::Sound(n) => {
                let Some((file, radius)) = fx.key_sound(n) else { continue };
                // No key sound is sent to others while the player is dead
                // (Corrections 2026-10-22, 4).
                if !mine && health.is_some_and(|h| h.current <= 0.0) {
                    continue;
                }
                let eye = t.map_or(Vec3::ZERO, |t| t.translation) + ms.map_or(Vec3::ZERO, |m| m.eye_offset);
                let at = vm.map_or(eye, |v| v.flash_point);
                sounds.write(if mine { CaSound::local(file) } else { CaSound::at(file, at, radius) });
            }
            ViewKey::ShellCasing if mine => {
                let Some(record) = tables.fire(&fx.fire) else { continue };
                if record.shells || !settings.shell_casings {
                    continue;
                }
                let Some(vm) = vm else { continue };
                state.serial += 1;
                let serial = state.serial;
                let shown = data::FireFx {
                    shells: true,
                    ..record.clone()
                };
                let _ = mods;
                spawn_casing(
                    &mut commands,
                    &mut state.rolls,
                    (k.owner, inv.active),
                    &fx.fire,
                    &shown,
                    vm.breach_point,
                    aim_of(intent.yaw, intent.pitch),
                    true,
                    serial,
                );
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Dings

/// The hit-confirm dings queued last frame (Quirk 11: the next character
/// update), local, one of the two files.
fn play_dings(mut dings: ResMut<DingQueue>, tables: Res<CaFxTables>, mut sounds: MessageWriter<CaSound>, mut rolls: Local<Rolls>) {
    for _ in 0..dings.drain() {
        if let Some(f) = rules::draw_any(&tables.sounds.ding, rolls.next()) {
            sounds.write(CaSound::local(f));
        }
    }
}

// ---------------------------------------------------------------------------
// Grenades

#[allow(clippy::too_many_arguments)]
fn grenade_effects(
    mut detonations: MessageReader<Detonated>,
    mut surfaces: MessageReader<CaBlastSurface>,
    cameras: CameraQuery,
    grid: Ground,
    tables: Res<CaFxTables>,
    settings: Res<CaFxSettings>,
    mut sounds: MessageWriter<CaSound>,
    mut fx: MessageWriter<CaFxRequest>,
    mut marks: ResMut<rules::CaMarks>,
    mut rolls: Local<Rolls>,
    volumes: Query<&CaVolume>,
    known: Res<KnownWeapons>,
) {
    let cam = camera(&cameras);
    let last: Vec<CaBlastSurface> = surfaces.read().cloned().collect();
    for d in detonations.read() {
        // A grenade's record by weapon (`[grenade]`), else an impact
        // projectile's: its weapon's ImpactFX (the ammo's `ImpactFXName`,
        // the install's record when the tables lack it; special_weapons.md
        // 1.3), played as a launched round's.
        let launched_fx = known.by_id.get(d.weapon).and_then(|k| k.fx.as_ref());
        let record = match tables.grenade.get(d.weapon) {
            Some(g) => tables.impact(&g.impact),
            None => launched_fx.and_then(|w| w.impact_of(&tables)),
        };
        let launched = tables.grenade.get(d.weapon).is_none() && launched_fx.is_some_and(|w| w.launched);
        let Some(record) = record else { continue };
        // The surface of the grenade's last bounce or touch, wherever it
        // is now; one that touched nothing is its own metal, never Air
        // (Corrections 2026-10-22, 1). Without the server's word (a
        // network client): the nearest tagged triangle, else metal.
        let ty = match last.iter().find(|l| l.projectile == d.projectile) {
            Some(l) => l.surface.as_deref().map_or(GRENADE_OWN_SURFACE, |n| grid.ty(n)),
            None => grid.nearest(d.at, GRENADE_SURFACE_REACH * UNIT).unwrap_or(GRENADE_OWN_SURFACE),
        };
        let surface = tables.surface(ty);
        let seen = cam.is_some_and(|c| c.gates(d.at, d.at, true).impact_seen);
        let mut out = impact(
            record,
            surface,
            AmmoKind::Thrown,
            seen,
            true,
            false,
            &settings,
            [rolls.next(), rolls.next()],
        );
        // In water (its point + the up normal in a liquid): the surface's
        // under-water grenade group, half volume (Edge cases).
        let wet = in_liquid(volumes.iter(), d.at + Vec3::Y * UNIT);
        if wet {
            submerge(&mut out, record, surface, AmmoKind::Thrown, seen);
        }
        // Launched (not grenade) ammo: the surface's impact group, as a
        // bullet's (special_weapons.md Follow-up 8).
        if launched && out.burst.is_some() {
            out.burst = launched_burst(surface, wet);
        }
        // The grenade's look is its surface's grenade group.
        if let Some(b) = out.burst.take() {
            fx.write(CaFxRequest {
                group: b,
                at: d.at,
                normal: Vec3::Y,
                kind: FxKind::Grenade,
            });
        }
        emit_impact(&out, d.at, Vec3::Y, None, &mut sounds, &mut fx, &mut marks, &mut rolls, record);
    }
}

/// A launched round's surface group (special_weapons.md Follow-up 8): the
/// surface's `ImpactFXName` (under water `UWImpactFXName`), none when
/// empty.
pub fn launched_burst(surface: &data::Surface, under_water: bool) -> Option<String> {
    let g = if under_water { &surface.uw_impact_fx } else { &surface.impact_fx };
    (!g.is_empty()).then(|| g.clone())
}

/// The Water surface type (footsteps on an unknown surface in water).
pub const WATER_SURFACE: u16 = 204;

/// How far (u) a detonation's point may lie from a surface for its type
/// to count when the server's last-contact surface isn't known.
pub const GRENADE_SURFACE_REACH: f32 = 16.0;

/// A thrown grenade's own object surface (Metal): what a detonation that
/// never touched anything uses.
pub const GRENADE_OWN_SURFACE: u16 = 20;

/// A grenade bounce sound's volume (grenades.md 6, step 4): 100, 50 in
/// water, × the contact's loudness (min(1, |v''| h / 200)).
pub fn bounce_volume(in_water: bool, loudness: f32) -> f32 {
    (if in_water { 50.0 } else { 100.0 }) * loudness.clamp(0.0, 1.0)
}

/// The server's bounce sounds: each new contact of a grenade plays its
/// surface's grenade sound (a new one replaces the grenade's last) at
/// `bounce_volume`.
fn bounce_sounds(
    grenades: Query<(Entity, &CaGrenade), Changed<CaGrenade>>,
    grid: Ground,
    tables: Res<CaFxTables>,
    mut seen: Local<HashMap<Entity, usize>>,
    mut sounds: MessageWriter<CaSound>,
    volumes: Query<&CaVolume>,
) {
    for (e, g) in &grenades {
        let n = seen.entry(e).or_insert(0);
        for c in g.contacts.iter().skip(*n) {
            let at = c.at * g.unit;
            let surface = tables.surface(grid.at(at));
            if surface.grenade.is_empty() {
                continue;
            }
            let radii = rules::server_radii(rules::inner_radius(surface.grenade_radius), surface.grenade_radius);
            let volume = bounce_volume(in_liquid(volumes.iter(), at), c.loudness);
            sounds.write(
                sounds::server_sound(&surface.grenade, at, radii)
                    .volume(volume)
                    .from_source(e, CHANNEL_GRENADE),
            );
        }
        *n = g.contacts.len();
    }
}

// ---------------------------------------------------------------------------
// Hurt

/// The hurt kinds of a hit: by damage type, then by zone.
pub fn hurt_kinds(kind: DamageKind, zone: Hitgroup, gas: bool) -> [Option<HurtKind>; 2] {
    let by_type = match kind {
        DamageKind::Bullet | DamageKind::Melee => Some(HurtKind::Guns),
        DamageKind::Blast => Some(HurtKind::Explosions),
        DamageKind::Burn => Some(HurtKind::Burning),
        DamageKind::Fall => Some(HurtKind::Falling),
        _ if gas => Some(HurtKind::Gas),
        _ => None,
    };
    // Burn, poison and bleed damage count as a torso hit: no head or groin
    // sound (Corrections 2026-10-22, 9).
    let by_zone = match zone {
        _ if kind == DamageKind::Burn || gas => None,
        Hitgroup::Head => Some(HurtKind::Head),
        // The groin's zone.
        Hitgroup::Stomach => Some(HurtKind::Groin),
        _ => None,
    };
    [by_type, by_zone]
}

/// A hurt kind's files.
pub fn hurt_files(t: &CaFxTables, k: HurtKind) -> &[String] {
    let s = &t.sounds;
    match k {
        HurtKind::Guns => &s.guns,
        HurtKind::Explosions => &s.explosions,
        HurtKind::Burning => &s.burning,
        HurtKind::Gas => &s.gas,
        HurtKind::Falling => &s.falling,
        HurtKind::Head => &s.head,
        HurtKind::Groin => &s.groin,
    }
}

/// A hurt kind's radius (u): its 3D auto-cast record's. The install's
/// records (autocastsound.txt, `3DsoundRadius`) give 500 for every kind
/// (Open question 27), the table's one value.
pub fn hurt_radius(t: &CaFxTables, _kind: HurtKind) -> f32 {
    t.sounds.hurt_radius
}

/// The length a hurt sound is taken to play when its clip isn't loaded
/// (no install): ours.
pub const HURT_DEFAULT_LENGTH: f32 = 1.0;

#[allow(clippy::too_many_arguments)]
fn hurt_sounds(
    mut damage: MessageReader<Damage>,
    players: Query<(&Transform, Option<&GameIdentity>), (With<Intent>, With<Health>)>,
    tables: Res<CaFxTables>,
    lengths: Res<CaSoundLengths>,
    mut limiter: ResMut<HurtLimiter>,
    time: Res<Time<Real>>,
    mut sounds: MessageWriter<CaSound>,
    mut rolls: Local<Rolls>,
) {
    let now = time.elapsed_secs_f64();
    for d in damage.read() {
        let Ok((victim, identity)) = players.get(d.target) else { continue };
        // Only our characters groan: another game's identity (the party
        // apps' `identity = "weapon"`) sounds as that game's.
        if d.amount <= 0.0 || !GameIdentity::allows(identity, super::GAME) {
            continue;
        }
        let gas = d.weapon.is_some_and(|w| w.ends_with("g7a"));
        for k in hurt_kinds(d.kind, d.hitgroup, gas).into_iter().flatten() {
            let Some(file) = rules::draw_any(hurt_files(&tables, k), rolls.next()) else { continue };
            let length = lengths.0.get(file).copied().unwrap_or(HURT_DEFAULT_LENGTH) as f64;
            if !limiter.try_start(k, now, length) {
                continue;
            }
            // Both sound at the victim's centre, not the hit point; each
            // kind takes its own radius (Corrections 2026-10-22, 9).
            let r = hurt_radius(&tables, k);
            let radii = rules::server_radii(rules::inner_radius(r), r);
            sounds.write(sounds::server_sound(file, victim.translation, radii));
        }
    }
}

// ---------------------------------------------------------------------------
// Footsteps

#[allow(clippy::type_complexity)]
fn footsteps(
    time: Res<Time>,
    mut players: Query<(
        Entity,
        &Transform,
        Option<&MovementState>,
        Option<&Intent>,
        Option<&Health>,
        Has<LocalPlayer>,
        Option<&mut OwnSteps>,
        Option<&mut OthersSteps>,
        (Option<&super::perks::CaPerkEffects>, Option<&Team>, Option<&Selection>, Option<&GameIdentity>),
    )>,
    listener: Query<(&super::perks::CaPerkEffects, Option<&Team>), With<LocalPlayer>>,
    ca: Option<Res<super::data::CaData>>,
    grid: Ground,
    tables: Res<CaFxTables>,
    mut sounds: MessageWriter<CaSound>,
    mut rolls: Local<Rolls>,
    mut last: Local<HashMap<Entity, (Vec3, bool)>>,
    mut commands: Commands,
    volumes: Query<&CaVolume>,
    even: Option<Res<crate::map::sound::EvenFootsteps>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let even = crate::map::sound::EvenFootsteps::is_on(even.as_deref());
    // What the listener's Awareness does to enemy steps (gear_perks.md 14).
    let listener = listener.single().ok().filter(|(fx, _)| fx.awareness).map(|(fx, t)| (*fx, t.map(|t| t.0)));
    for (e, t, ms, intent, health, local, own, others, (perk, team, look, identity)) in &mut players {
        let Some(ms) = ms else { continue };
        let alive = health.is_none_or(|h| h.current > 0.0);
        let (before, was_ground) = last.insert(e, (t.translation, ms.on_ground)).unwrap_or((t.translation, ms.on_ground));
        // Only our characters' steps: another game's identity (the party
        // apps' `identity = "weapon"`) steps as that game's.
        if !GameIdentity::allows(identity, super::GAME) {
            continue;
        }
        let moved = t.translation.with_y(0.0).distance(before.with_y(0.0)) / UNIT;
        let landed = ms.on_ground && !was_ground;
        let jumped = !ms.on_ground && was_ground && intent.is_some_and(|i| i.jump);
        let feet = t.translation + Vec3::Y * ms.hull_min.y;
        let surface = |foot_roll: f32, rolls: &mut Rolls| -> Option<String> {
            let ty = if ms.on_ladder {
                200
            } else {
                let ty = grid.below(feet + Vec3::Y * 50.0 * UNIT, 100.0 * UNIT).unwrap_or(0);
                // An unknown surface inside a water volume is Water.
                if ty == 0 && in_liquid(volumes.iter(), feet) { WATER_SURFACE } else { ty }
            };
            let s = tables.surface(ty);
            if s.invisible {
                return None;
            }
            let list = if foot_roll < 0.5 { &s.right } else { &s.left };
            rules::draw_slot(list, 2, rolls.next()).map(str::to_string)
        };
        if local {
            let Some(mut own) = own else {
                commands.entity(e).insert(OwnSteps::default());
                continue;
            };
            let walk = intent.is_some_and(|i| i.walk);
            let inputs = StepInputs {
                dt,
                // The movement key itself, not displacement (Corrections
                // 2026-10-22, 5).
                moving: intent.is_some_and(|i| i.move_axis != Vec2::ZERO),
                can_step: ms.on_ground || ms.on_ladder,
                run: (!walk || ms.sprinting) && !ms.crouching,
                jumped,
                landed,
                alive,
            };
            for foot in own.step(&inputs) {
                let roll = if foot == rules::Foot::Right { 0.0 } else { 1.0 };
                if let Some(f) = surface(roll, &mut rolls) {
                    sounds.write(CaSound::local(f));
                }
            }
            if jumped && alive && let Some(f) = rules::draw_any(&tables.sounds.jump, rolls.next()) {
                sounds.write(CaSound::local(f));
            }
            continue;
        }
        let Some(mut steps) = others else {
            commands.entity(e).insert(OthersSteps::default());
            continue;
        };
        if !alive {
            continue;
        }
        let speed = moved / dt;
        let gait = if !ms.on_ground {
            Gait::Jump
        } else if speed < 10.0 {
            Gait::Still
        } else if ms.crouching {
            Gait::Crouch
        } else if intent.is_some_and(|i| i.walk) && !ms.sprinting {
            Gait::Walk
        } else if ms.sprinting {
            Gait::Sprint
        } else {
            Gait::Run
        };
        let landing = landed && steps.airborne && gait != Gait::Jump;
        let n = match gait {
            // A jump's one key is its landing.
            Gait::Jump => 0,
            _ if landing => 1,
            _ => rules::footsteps_between(gait, steps.clock, steps.clock + dt),
        };
        steps.airborne = !ms.on_ground;
        steps.clock = if matches!(gait, Gait::Run | Gait::Sprint) { steps.clock + dt } else { 0.0 };
        // A listener's Awareness plays an enemy's steps at its own volume
        // and radius and ignores the walker's Specter (the perk routine
        // returns before it), plus one sound of the perk's own at the
        // step's volume from before the scaling; otherwise Specter quiets
        // the walker's own steps (sprinting: its dash volume). Then the
        // model's `FootprintSndVolume` share is added (Corrections
        // 2026-10-22, 6, 7).
        let share = ca
            .as_deref()
            .map_or(0.0, |c| c.characters().effective(look.unwrap_or(&Selection::default())).0.footprint_volume);
        let aware = listener.filter(|(_, mine)| *mine != team.map(|t| t.0));
        let specter = perk.map_or(1.0, |p| if ms.sprinting { p.dash_volume } else { p.step_volume });
        let awareness = aware.map(|(fx, _)| (fx.aware_volume, fx.aware_radius));
        let (volume, radius, extra) = footstep_sound(awareness, specter, share);
        // `party_even_footsteps 1`: the shared profile (CS:S's level and
        // its volume for the pace) instead of the radii; the perks still
        // scale it (Awareness's radius factor stretches the falloff), the
        // model's share doesn't (it is a body's own loudness).
        let (volume, extra, level) = match even.then(|| even_step(gait, landing, awareness.map(|(_, r)| r))) {
            Some((level, pace)) => (
                pace * footstep_sound(awareness, specter, 0.0).0,
                extra.map(|_| pace * 100.0),
                Some(level),
            ),
            None => (volume, extra, None),
        };
        let shape = |s: CaSound, volume: f32| {
            let s = s.volume(volume);
            match level {
                Some(level) => s.with_level(level),
                None => s,
            }
        };
        // Others' steps sound from the character's centre; only the
        // surface ray starts at the foot.
        let centre = t.translation;
        for i in 0..n {
            let roll = if (steps.clock * 1000.0) as u32 / 300 % 2 == 0 && i == 0 { 0.0 } else { 1.0 };
            if let Some(f) = surface(roll, &mut rolls) {
                sounds.write(shape(CaSound::at(f, centre, radius), volume));
            }
            if let Some(volume) = extra
                && let Some(f) = rules::draw_any(&tables.sounds.awareness, rolls.next())
            {
                sounds.write(shape(CaSound::at(f, centre, radius), volume));
            }
        }
    }
}

/// A step's level and volume (0..1) in the party's shared profile
/// (`party_even_footsteps 1`, `map::sound::EVEN_STEP_LEVEL`): the pace
/// from the gait (a landing is the jump's one step; sprinting runs), the
/// falloff stretched by the listener's Awareness radius factor.
pub fn even_step(gait: Gait, landing: bool, aware_radius: Option<f32>) -> (crate::map::SoundLevel, f32) {
    use crate::map::sound::{EVEN_STEP_LEVEL, StepPace, even_step_volume};
    let pace = match gait {
        _ if landing => StepPace::Jump,
        Gait::Walk => StepPace::Walk,
        Gait::Crouch => StepPace::Crouch,
        _ => StepPace::Run,
    };
    let level = match (EVEN_STEP_LEVEL, aware_radius) {
        (crate::map::SoundLevel::Attenuation(a), Some(r)) if r > 0.0 => crate::map::SoundLevel::Attenuation(a / r),
        (level, _) => level,
    };
    (level, even_step_volume(pace))
}

/// An enemy step's volume (0..100) and radius (u), and the volume of the
/// Awareness perk's own extra sound (None without the perk), for the
/// listener's `awareness` (volume and radius factors), the walker's
/// Specter factor and the model's `FootprintSndVolume` share.
pub fn footstep_sound(awareness: Option<(f32, f32)>, specter: f32, share: f32) -> (f32, f32, Option<f32>) {
    let base = 100.0;
    let (volume, radius, extra) = match awareness {
        Some((v, r)) => (base * v, rules::FOOTSTEP_RADIUS * r, Some(base)),
        None => (base * specter, rules::FOOTSTEP_RADIUS, None),
    };
    (volume + volume * share, radius, extra)
}

// ---------------------------------------------------------------------------
// Stepping

#[allow(clippy::too_many_arguments)]
fn step_effects(
    time: Res<Time>,
    mut marks: ResMut<rules::CaMarks>,
    mut tracers: ResMut<CaTracers>,
    mut shells: Query<(Entity, &mut CaShell, &mut Transform)>,
    owners: Query<&Transform, Without<CaShell>>,
    characters: Query<(), With<Intent>>,
    spatial: SpatialQuery,
    grid: Ground,
    tables: Res<CaFxTables>,
    mut sounds: MessageWriter<CaSound>,
    mut last_owner: Local<HashMap<Entity, Vec3>>,
    mut commands: Commands,
    volumes: Query<&CaVolume>,
) {
    let dt = time.delta_secs();
    marks.step(dt);
    for t in &mut tracers.0 {
        t.age += dt;
    }
    tracers.0.retain(|t| t.path.ends(t.age).is_some());
    let filter = SpatialQueryFilter::from_mask(SOLID_LAYERS);
    let mut alive: Vec<(u64, Entity)> = Vec::new();
    let mut moved: HashMap<Entity, Vec3> = HashMap::new();
    for (e, mut s, mut t) in &mut shells {
        // Your own displacement this frame (Quirk 15).
        let carry = *moved.entry(s.owner).or_insert_with(|| {
            let now = owners.get(s.owner).map_or(Vec3::ZERO, |o| o.translation);
            let before = last_owner.get(&s.owner).copied().unwrap_or(now);
            (now - before) / UNIT
        });
        // In water: half gravity, a quarter kept per bounce, no sound (8).
        s.casing.in_water = in_liquid(volumes.iter(), s.casing.at * UNIT);
        let step = s.casing.step(dt, carry, |from, to| {
            let d = (to - from) * UNIT;
            let dir = Dir3::new(d).ok()?;
            // Characters are passed through.
            let hit = spatial.cast_ray_predicate(from * UNIT, dir, d.length(), true, &filter, &|e| {
                !characters.contains(e)
            })?;
            let at = from + (to - from) * (hit.distance / d.length());
            let surface = tables.surface(grid.at(at * UNIT));
            Some(CasingHit {
                at,
                normal: hit.normal,
                removes: surface.sky || surface.invisible,
            })
        });
        match step {
            CasingStep::Removed => {
                commands.entity(e).despawn();
                continue;
            }
            CasingStep::FirstBounce if !s.casing.in_water && !s.sounded => {
                s.sounded = true;
                let surface = tables.surface(grid.at(s.casing.at * UNIT));
                if !surface.shell.is_empty() {
                    let sound = CaSound::at(&surface.shell, s.casing.at * UNIT, surface.shell_radius);
                    sounds.write(match s.weapon {
                        Some(w) => sound.from_source(w, CHANNEL_SHELL),
                        None => sound,
                    });
                }
            }
            _ => {}
        }
        t.translation = s.casing.at * UNIT;
        t.rotation = s.casing.orientation;
        alive.push((s.serial, e));
    }
    for owner in moved.keys() {
        if let Ok(o) = owners.get(*owner) {
            last_owner.insert(*owner, o.translation);
        }
    }
    // The list's cap: the oldest go.
    if alive.len() > rules::SHELL_LIST_MAX {
        alive.sort_unstable();
        for (_, e) in &alive[..alive.len() - rules::SHELL_LIST_MAX] {
            commands.entity(*e).despawn();
        }
    }
}

/// Start the requested groups (when the effect file is loaded) and play
/// the groups on Chimera's particle pool.
#[allow(clippy::too_many_arguments)]
fn spawn_fx(
    time: Res<Time>,
    mut requests: MessageReader<CaFxRequest>,
    mut fx: ResMut<CaFx>,
    particles: Option<ResMut<Particles>>,
    materials: Option<Res<ParticleMaterials>>,
    world: WorldTracer,
    parents: Query<(&Transform, Option<&super::characters::CaPose>, Option<&MovementState>)>,
) {
    let Some(lib) = library::library() else {
        requests.clear();
        return;
    };
    for r in requests.read() {
        match r.kind {
            // On the shooter: where he is, with his sockets (`Flash`).
            FxKind::OwnParented(parent) => {
                let Ok((t, pose, state)) = parents.get(parent) else { continue };
                let pose = attached::pose_of(t, pose.zip(state));
                fx.attach_once(&lib, &r.group, parent, pose);
            }
            _ => {
                fx.spawn(&lib, &r.group, r.at, effect_frame(r.normal));
            }
        }
    }
    let (Some(mut particles), Some(materials)) = (particles, materials) else { return };
    fx.step(time.delta_secs(), &lib, &mut particles, &materials.0, &world);
}
