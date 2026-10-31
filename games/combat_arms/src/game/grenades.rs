//! Combat Arms' thrown weapons (specs/combat_arms/grenades.md): the frag
//! (M67), flash (XM84), smoke (M18), incendiary (M-23F) and nerve gas
//! (G7A) grenades, on Chimera's thrown-weapon framework (`weapon::grenade`).
//!
//! The weapons are data (data/weapons/m67.toml and the others, the
//! `combat_arms:grenade` hook: `GrenadeParams`), built as Chimera's
//! `Throwable` (count, pin, release, post-fire and re-ready, predicted with
//! the weapon frame) with `ThrowOptions` (3): the 700 ms pre-fire (pin
//! pull), then the 67 ms throw animation, which plays again for as long as
//! fire is held when it ends (`hold_loop`: the grenade stays in hand, no
//! cooking); the play running when fire is up is followed by the 366 ms
//! post-fire, whose fire key at 133 ms releases the grenade (900 ms after
//! a tap); with grenades left, the 400 ms reload re-readies the next. The
//! three kinds' animations are alike. The spent weapon is kept at total 0
//! by `inventory` with Chimera's `KeepSpent`; no default live drop on death
//! (this module's `death_release`). The projectile is made here
//! (`launch`, from this module's observer of Chimera's `Launch` on a
//! weapon with `CaGrenadeData`): from the eye along the view, perturbed by the weapon's
//! square spread (`throw_perturb`: weapon_firing.md's cone, the limits
//! multiplied by the dynamic factor g first, none when the top is 0: the
//! flash and smoke fly exactly along the view), at `speed` with none of
//! the thrower's velocity (4), started `FireOffset` along the throw when that
//! segment is clear (`fire_offset`, Corrections 1). It is a Chimera `Projectile` marked
//! `GameFlown` with this game's state (`CaGrenade`), so the network draws
//! it (`NetItem`, with its `ShownItem`: `thrown_key`, the projectile
//! record's model in its `World` pose at `ModelScale`, loaded by
//! `characters::models` when the install has it) and `Detonated` tells
//! the presentation and clients; its `Transform` is drawn between the
//! server frames below (`draw_between_frames`: on the line from the frame
//! before to the last, one frame behind, so the arc doesn't hop 50 ms at
//! a time); it
//! and its smoke cloud `SurvivesRestart`: a new round leaves grenades in
//! flight to go off on their fuse (15).
//!
//! On the server (authoritative), grenades, explosions and flash timers
//! run on server frames of their own (`CaServerFrames`: the shipped
//! server's 20 per second, each frame at the first tick at or after its
//! time, dt exactly 0.05 s), in the spec's per-tick order:
//!
//! 1. A grenade made since the last frame starts in this one: its start
//!    time is the frame before's, and it is updated and stepped in this
//!    frame (5, Q6: 49 steps before a 2.5 s fuse).
//! 2. Each grenade in creation order: the one-time nudge of a grenade
//!    that came to rest (6: back along the velocity it had at the start of
//!    the stopping step, by its half-height `box_half`.y × max(0.1, h)), the
//!    fuse (8: the first frame at or after start + `fuse`; or its hit
//!    points gone, 13), else its physics (`step`: |v|² < 0.1 zeroed;
//!    volumes (7, `volumes::CaVolume`: viscosity and current above 1 u/s,
//!    the liquids' −200 u/s² gravity; other games' water too: `volumes::BrushWater`); the
//!    quarter-gravity move of 5 under 1600 u/s²; a point against the
//!    world (traces-only detail and solid world models too): a mirror
//!    bounce with the engine's push (0.05 u/s per contact, twice) inside
//!    the surface's hardness (`CaGrenadeSurfaces`,
//!    data/grenade_surfaces.toml), the rest test on the horizontal speed,
//!    through invisible surfaces, gone on sky; characters whose boxes the
//!    step crosses (the character's, then its hit-box object's, the hit
//!    spheres' bounds) while the line of motion passes a hit sphere slow
//!    it by flesh for each, no mirror, every step it is inside
//!    (`character_touches`; not a party's spawn-protected, Chimera's
//!    `core::SpawnProtected` in its bullets window, which every grenade,
//!    bolt, blast and flash here leaves out: `GrenadeWorld::bodies`);
//!    props slow it by their surface the same way;
//!    grenades in flight whose box it crosses (`box_half`) slow both by
//!    metal, between one thrower's only with `hits_same_kind`). A
//!    detonation (8) sends the effect (`Detonated`) and applies its
//!    instant part at once: a frag's blast (9: everything whose box
//!    touches the sphere, players twice (Quirk 3), d to the centre, two
//!    cover traces (every world surface blocks, invisible and traces-only
//!    ones too; not characters, props or player blockers) and the head-top
//!    retry, the linear falloff from a quarter of the radius, friendly
//!    fire by `core::DamageRules`, the armour step for explosions on the
//!    torso (no stock gear lists them), death at 1 HP), other grenades hit
//!    and chained in the same frame (13), breakables without a cover test,
//!    a flash test (10: players once, protection the victim's gear's torso
//!    flash entries (`weapons::gear_torso`) plus `CaFlashProtection`, the
//!    body's yaw for the facing, a timer); a smoke's cloud is the clients'
//!    look only (11: `SmokeCloud` without a `SightBlocker`, so nothing in
//!    game logic sees it); fire and gas leave a `CaDamageSphere` (12).
//! 3. Damage spheres refresh the over-time entries of the players inside
//!    (`CaGrenadeEffects::over_time`).
//! 4. Flash timers that ran out send the flash (T ≥ 0.1 s) as Chimera's
//!    `core::Blinded` (white, fading by the end, T/2: its network message
//!    carries it to the victim's client; drawn over the HUD, under chat and
//!    menus) and `CaFlashed`; over-time entries deal their rate × dt
//!    through the armour step (burn halved by body armour with armour
//!    points left, poison by a gas mask only).
//!
//! Bullets hit a grenade's box (`weapons`); `shot_grenades` takes the
//! damage off its hit points and one at 1 or less goes off on the next
//! frame as at its fuse, credited to its thrower (13).
//!
//! `flash_white` is the client's screen curve (cos(π t / T) until T/2).
//! The server's "grenade in hand, pin pulled" flag (`CaPin`: on once the
//! pre-fire has ended while fire stays held, off when it is let go or the
//! weapon changes) releases a full-fuse grenade at 500 u/s along the body's
//! facing when its holder dies (14), ammo unchecked (Quirk 5).
//!
//! Units: the data's world units (1.8 cm each, by body size:
//! `inventory::UNIT`) and hit points inside; the world in metres and
//! health (1.0 = 100 HP).
//!
//! The throw's sounds, spin and pose (specs/combat_arms/grenades.md,
//! Corrections (2026-10-27) C1 to C3): `tumble` turns the grenade from
//! accumulated angles set each update (rates drawn at creation and each
//! bounce from the deterministic `shared_random`) and smooths it on a
//! client; `callout` sends the thrower's team the "grenade throwing"
//! voice when `GrenadeParams::call_out` is on.
//!
//! Corrections 2026-10-22 (b): the M-23F and G7A are one grenade, shown and
//! real (the spec's recommendation; the original shows 2 and ignores the
//! second throw: not copied); the Parting Gift grenade is the perk's own
//! stock frag on a fixed 2.5 s fuse (`perks::parting_gift`, a faithful
//! quirk: whatever the dead carried).
//!
//! Known shortcuts (the spec's most literal reading where it is unclear,
//! listed for its open questions):
//! - The throw starts on the press's tick (the spec's one or two client
//!   frames before the pre-fire are not modelled); each animation step
//!   waits for the first tick at or after its time (a held throw play lasts
//!   5 ticks, 75 ms); the projectile appears a tick after the release
//!   (`Throwable` throws strictly after its time). A fire press during the
//!   reload is not queued: fire must be held when it ends. The throw
//!   perk is Pitcher's (`perks::throw_force`).
//! - The hit-box object's box is the hit spheres' bounds (its own dims
//!   aren't in a spec). A character wearing another game's body (its
//!   identity's) is that body's hitboxes (`weapons::their_shapes`; a box
//!   as its bounding sphere), its box widened to hold them. A character touch snaps the grenade to where the
//!   step enters a hit sphere (the "short trace"), else leaves it at the
//!   step's end. Grenade-on-grenade touches don't move either. M-23F and
//!   G7A boxes are the M67's (PLACEHOLDER: not in the spec). Bounce
//!   sounds are `effects`' (each contact's `loudness`, half in water);
//!   a character touch's is at full loudness (6 gives the formula for
//!   world bounces only); bots don't hear them.
//! - Grenades don't tell the map's invisible polygons or sky portals apart
//!   (Combat Arms maps' surfaces have no names here): they bounce off clip
//!   walls and sky surfaces of the map; colliders named "invisible" and
//!   "sky" behave as the spec says. Doors don't move (`world`), so "wherever they are in their
//!   motion" is where they stand.
//! - The screen draws Chimera's white overlay over `Blinded` (no first-frame
//!   HUD hiding) with Combat Arms' cosine fade (`blinded_alpha`, set as the
//!   shell's `senses::FlashFade` by `ui::CaUiPlugin`; Chimera's own fade is
//!   linear and CS:S keeps it). Gear is known by its flash entries only.
//! - No drop-grenade perk (Q13); no debug bounce cap; no map cameras (a
//!   grenade going off while one is on would be removed without effect);
//!   a new round still ends a flash (Chimera's restart clears `Blinded`);
//!   the M67's knock-back is the infection mode's (Q14).
//! - Placeholder looks (L10 not lifted): a ball in flight without the
//!   install's model, a grey ball for the smoke (`ui::visuals`), no
//!   explosion or flash models. In flight the model holds its `World`
//!   pose from the throw (the `Hand` animation before it isn't drawn) and
//!   turns as `tumble` says.
//! - The sounds (C1) are `effects`' (the pin pull's `sound_key 10` plays
//!   the record's `MiscSound1`, each client deriving others' from the
//!   view-model keys it replays) and `callout`'s (the team's voice at the
//!   release, which does not follow the thrower once started); there is no
//!   flight sound and no release sound, as in the spec. The tumble and the
//!   client's smoothing of it are `tumble`'s (C2); the model is drawn in
//!   its `World` pose from creation, `Hand` never (C3).

pub mod callout;
pub mod tumble;

use std::collections::HashMap;

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

use self::tumble::Tumble;
use super::{
    inventory::{CaSelection, UNIT},
    volumes::{CaVolume, LIQUID_GRAVITY},
    weapons::{
        ArmourHit, BURN, CaSpread, EXPLOSION, FLASH, HitSpheres, Location, MALE, POISON, armour_and_zone,
        body_geometry, box_entry, gear_torso, square_dir,
    },
};
use crate::{
    core::{
        Blinded, Damage, DamageKind, DamageRules, Damageable, Died, Health, Hitgroup, Intent,
        MovementState, Predict, PropSurface, SOLID_LAYERS, SimClock, SimSet,
    },
    map::{PropEntity, loose::ShownItem},
    net::NetAppExt,
    weapon::{
        Inventory, Weapon, WeaponFrame, WeaponState,
        data::{WeaponData, WeaponDataAppExt},
        grenade::{
            Blast, Detonated, Flash, Flight, GameFlown, GrenadeEffect, GrenadeKind, Launch, Projectile, Smoke,
            SmokeCloud, SurvivesRestart, ThrowOptions, ThrowRule, Throwable, spawn_projectile,
        },
        random::{Rng, shared_random},
    },
};

/// The hook name (docs/weapon-data.md, "Hooks").
pub const HOOK: &str = "combat_arms:grenade";

/// The stock loadout's grenade (data/weapons/m67.toml).
pub const M67: &str = "combat_arms:m67";
pub const XM84: &str = "combat_arms:xm84";
pub const M18: &str = "combat_arms:m18";
pub const M23F: &str = "combat_arms:m23f";
pub const G7A: &str = "combat_arms:g7a";

// Constants (world units, seconds; the spec's "Constants").

/// The engine's gravity for every non-player object, u/s² (down).
pub const OBJECT_GRAVITY: f32 = 1600.0;
/// Displacement per step uses this × g dt², not ½ (Quirk 1).
pub const GRAVITY_INTEGRATION_FACTOR: f32 = 0.25;
/// A velocity with |v|² below this, (u/s)², is zeroed before the step.
pub const SLOW_STOP: f32 = 0.1;
/// The engine's push into a surface per contact, u/s; a bounce takes it
/// twice, inside the hardness (the formula's −0.1 n, Q18).
pub const SURFACE_PUSH: f32 = 0.05;
/// A grenade's own surface (what other grenades touching it meet).
pub const GRENADE_SURFACE: &str = "metal";
/// Viscosity and current act only above this speed, u/s.
pub const VISCOSITY_MIN_SPEED: f32 = 1.0;
/// `GrenadeDampenPercent`: what a bounce loses on a surface not in the
/// table.
pub const DEFAULT_DAMPEN: f32 = 0.35;
/// `GrenadeMinVelMag`: a floor hit slower than this horizontally stops it,
/// u/s.
pub const REST_SPEED: f32 = 50.0;
/// A surface is a floor when its normal's up part is above this.
pub const REST_FLOOR_NORMAL: f32 = 0.75;
/// Blast and flash are full inside this fraction of the radius.
pub const BLAST_FULL_FRACTION: f32 = 0.25;
/// Objects whose dims vector is longer than this (u) have the excess taken
/// off their blast distance (characters too, by their dims: Corrections 2).
pub const BIG_OBJECT_SLACK: f32 = 100.0;
/// `FlashBangDelay`: blast to blindness facing it, s; facing away a third.
pub const FLASH_DELAY: f64 = 0.25;
pub const FLASH_DELAY_BACK: f64 = FLASH_DELAY / 3.0;
/// Flash time facing away, as a fraction.
pub const FLASH_BACK_FACTOR: f32 = 0.25;
/// Shorter flashes (s) aren't sent.
pub const FLASH_MIN_TIME: f32 = 0.1;
/// A flash amount at most this does nothing.
pub const FLASH_MIN_AMOUNT: f32 = 0.001;
/// `WhiteScreenDecl` and `CapturedDecl` defaults (every stock weapon).
pub const WHITE_DECL: f32 = 2.0;
pub const CAPTURED_DECL: f32 = 1.0;
/// The live grenade released when its holder dies, u/s.
pub const DEATH_RELEASE_SPEED: f32 = 500.0;
/// Speed lost per unit of a volume's viscosity, u/s².
pub const LIQUID_VISCOSITY_SCALE: f32 = 5000.0;
/// Damage type 1, bleeding (caltrops).
pub const BLEED: i32 = 1;
/// A grenade at this many HP or fewer after damage goes off.
pub const DESTROYED_AT: f32 = 1.0;
/// A player at this many HP or fewer after damage dies.
pub const DEATH_AT: f32 = 1.0;
/// The shipped dedicated server's frame, s (20 frames a second).
pub const SERVER_FRAME: f64 = 0.05;
/// A detonation after a touch is backed off along the motion this far, u.
pub const DETONATION_BACKOFF: f32 = 0.1;
/// The over-time damage's cover traces start this far towards the
/// target, u.
pub const OVER_TIME_TRACE_START: f32 = 1.0;
/// Spin rates' ranges, rad/s (pitch; yaw and roll).
pub const SPIN_PITCH: f32 = std::f32::consts::PI;
pub const SPIN_YAW_ROLL: f32 = std::f32::consts::TAU;
/// The spin rates (pitch, yaw, roll; rad/s), uniform in their ranges:
/// drawn when a grenade is made and at each bounce (C2).
pub fn spin_rates(rng: &mut Rng) -> Vec3 {
    Vec3::new(
        (rng.next() * 2.0 - 1.0) * SPIN_PITCH,
        (rng.next() * 2.0 - 1.0) * SPIN_YAW_ROLL,
        (rng.next() * 2.0 - 1.0) * SPIN_YAW_ROLL,
    )
}

/// Kept between a grenade and the surface it touched, u (ours: the next
/// step's trace starts outside it).
const SKIN: f32 = 1e-3;

// ---------------------------------------------------------------------------
// Data

/// What a grenade's detonation does at once (its ammo's `AreaDamageType`).
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AreaType {
    /// No instant part (fire, gas).
    None,
    /// A blast (type 7).
    Explode,
    /// A flash test (type 34).
    Flash,
    /// A smoke cloud on the clients; nothing on the server.
    Smoke,
}

/// The kind of an over-time damage sphere (`ProgDamageType`).
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OverTimeKind {
    /// Type 3: halved by body armour with points left.
    Burn,
    /// Type 9: unprotected.
    Poison,
    /// Type 1: bleeding (caltrops), unprotected.
    Bleed,
}

/// A damage sphere's numbers (`ProgDamage*`): HP/s, u, s.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OverTimeParams {
    pub kind: OverTimeKind,
    pub rate: f32,
    pub radius: f32,
    pub lifetime: f32,
    pub duration: f32,
}

/// The `combat_arms:grenade` hook's parameters (data/weapons/m67.toml
/// says where each comes from). World units, seconds, hit points.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GrenadeParams {
    /// `SpawnedAmount`, `MaxAmount`.
    pub spawned: u32,
    pub max: u32,
    /// The view-model animations (3): pre-fire (pin pull), the throw
    /// (repeated while fire is held), the post-fire with the release's
    /// key time inside it, and the reload that re-readies the next.
    pub prefire: f32,
    pub throw_loop: f32,
    pub postfire: f32,
    pub release_key: f32,
    pub reload: f32,
    /// `MinPerturb`, `MaxPerturb` (thousandths), `DynamicPerturbFactor`.
    pub perturb: [u32; 2],
    #[serde(default)]
    pub dynamic_perturb: f32,
    /// The projectile's `Lifetime` and `Velocity`; `DropVelocity`.
    pub fuse: f32,
    pub speed: f32,
    #[serde(default)]
    pub drop_speed: f32,
    /// The ammo's `Health`: the live grenade's hit points.
    pub health: f32,
    /// `CanHitSameKind`.
    #[serde(default)]
    pub hits_same_kind: bool,
    /// `AreaDamage`, `AreaDamageRadius`, `AreaDamageType`.
    #[serde(default)]
    pub area_damage: f32,
    #[serde(default)]
    pub area_radius: f32,
    pub area_type: AreaType,
    /// The lasting sphere (fire, gas).
    #[serde(default)]
    pub over_time: Option<OverTimeParams>,
    /// The grenade's box, half-extents (u): its world model's first
    /// animation × its scale 2 (`grenade_box`); y is the half-height of
    /// the at-rest nudge.
    pub box_half: [f32; 3],
    /// The projectile record's `FireOffset` (u; Corrections 1): the
    /// server starts the grenade this far along the throw when that
    /// segment is clear (0: at the eye; the stock value is Open question
    /// 23's).
    #[serde(default)]
    pub fire_offset: f32,
    /// The release makes the team's call-out (`callout`): the weapon's
    /// `PlayAutoThrowSound` (default on) and `Guntype` 7. Ours for the
    /// five stock grenades; the install's record says.
    #[serde(default)]
    pub call_out: bool,
    /// The projectile record's looks and sounds (`ActivateSound`,
    /// `FinishSound`, ...) and its create sound (special_weapons.md
    /// Corrections (2026-10-31) 3).
    #[serde(default)]
    pub look: super::special::ProjectileLook,
    #[serde(default)]
    pub create_sound: String,
    #[serde(default)]
    pub create_radius: f32,
    /// The projectile record's `Model` (an archive path of the install):
    /// the grenade as drawn in flight, in its `World` pose; None: the front
    /// end's placeholder. `ModelScale` (stock 2 per axis; the box above is
    /// the model's first animation's times it) and `Skin#` (archive paths
    /// by texture index).
    #[serde(default)]
    pub thrown_model: Option<String>,
    #[serde(default = "stock_model_scale")]
    pub thrown_scale: [f32; 3],
    #[serde(default)]
    pub thrown_skins: Vec<String>,
}

/// The stock grenades' `ModelScale` (grenades.md, Data).
fn stock_model_scale() -> [f32; 3] {
    [2.0; 3]
}

/// The key of weapon `id`'s grenade as drawn in flight (its `ShownItem`
/// and held-model key; the thrown model, `GrenadeParams::thrown_model`).
pub fn thrown_key(id: &str) -> String {
    format!("{id}{THROWN_SUFFIX}")
}

/// What `thrown_key` appends to a weapon id.
pub const THROWN_SUFFIX: &str = "#thrown";

/// Weapon part: a Combat Arms grenade's numbers, and its document's units
/// (metres per length unit, health per damage point).
#[derive(Component, Clone, Debug)]
pub struct CaGrenadeData {
    pub params: GrenadeParams,
    pub unit: f32,
    pub hp: f32,
}

/// Weapon part (the server's): "grenade in hand, pin pulled" (3), from the
/// pin's start while fire stays held.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct CaPin {
    /// When the pin pull started (fire held since).
    pub since: Option<f64>,
    /// The third-person pre-fire has ended with fire still held.
    pub on: bool,
}

/// Grenade bounce hardness by surface name (data/grenade_surfaces.toml).
#[derive(Resource, Clone, Debug, Default)]
pub struct CaGrenadeSurfaces(pub Vec<(String, f32)>);

impl CaGrenadeSurfaces {
    /// The hardness of `name` (clamped to [0, 1]); unlisted surfaces keep
    /// 1 − `DEFAULT_DAMPEN`.
    pub fn hardness(&self, name: Option<&str>) -> f32 {
        let name = name.map(str::to_lowercase);
        name.and_then(|n| self.0.iter().find(|(s, _)| *s == n).map(|(_, h)| *h))
            .unwrap_or(1.0 - DEFAULT_DAMPEN)
            .clamp(0.0, 1.0)
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Doc {
            surface: Vec<Row>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Row {
            names: Vec<String>,
            hardness: f32,
        }
        let doc: Doc = toml::from_str(text).map_err(|e| e.to_string())?;
        Ok(Self(
            doc.surface
                .into_iter()
                .flat_map(|r| r.names.into_iter().map(move |n| (n.to_lowercase(), r.hardness)))
                .collect(),
        ))
    }

    /// The built-in table (data/grenade_surfaces.toml).
    pub fn built_in() -> Self {
        Self::parse(include_str!("../../data/grenade_surfaces.toml")).expect("grenade_surfaces.toml")
    }
}

/// Flash protection a victim has beyond its gear's (`weapon::ArmorPieces`' flash
/// entries, summed with this; set items, tests).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct CaFlashProtection(pub f32);

/// The last flash sent to this player: when it arrived (the server's
/// clock) and its flash time T, s.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CaFlashed {
    pub at: f64,
    pub time: f32,
}

/// The clients' smoke cloud's look (11; Corrections 2026-10-22, 2): a
/// PLACEHOLDER sphere spread out to 3 m within 0.5 s (one burst of 10
/// sprites of 200-300 u, friction 0.98 per second), gone by 10 s (each
/// sprite lives 8-10 s).
pub fn smoke_look() -> Smoke {
    Smoke {
        max_radius: 300.0 * UNIT,
        expand_time: 0.5,
        fade_start: 8.0,
        fade_end: 10.0,
        pop_speed: 0.0,
        sight_radius: 0.0,
        sound: None,
    }
}

/// The `combat_arms:grenade` hook: the weapon as Chimera's `Throwable`
/// with Combat Arms' throw (`ThrowOptions`, `launch`), its numbers
/// (`CaGrenadeData`) and the server's pin flag (`CaPin`).
pub fn throwable(e: &mut EntityWorldMut, d: &'static WeaponData, p: &GrenadeParams) {
    let unit = d.units.length;
    let hp = d.units.damage;
    // Read by the network's clients for a detonation's kind (and by
    // nothing else here: this module flies and detonates the grenade).
    let effect = match p.area_type {
        AreaType::Flash => GrenadeEffect::Flash(Flash {
            model: |_, _| None,
            sound: None,
            hearing: None,
            through_smoke: 1.0,
        }),
        AreaType::Smoke => GrenadeEffect::Smoke(smoke_look()),
        AreaType::Explode | AreaType::None => GrenadeEffect::Blast(Blast {
            damage: p.area_damage * hp,
            radius: p.area_radius * unit,
            bell_sigma: None,
            legacy: false,
            armor_ratio: None,
            quantum: 0.0,
            probe_up: 0.0,
            probe_down: 0.0,
            pull_out: 0.0,
            src_lift: 0.0,
            body_target: (0.5, 0.5),
            force: 0.0,
            force_jitter: (1.0, 1.0),
            max_push_speed: 0.0,
            ragdolls: None,
            scorch: None,
            sound: None,
            hearing: None,
            shake: None,
            shield: None,
            through_walls: false,
        }),
    };
    e.insert((
        Throwable {
            count: p.spawned.min(p.max.max(p.spawned)),
            max: p.max,
            // The pre-fire, then the throw animation (`hold_loop`); the
            // post-fire's key releases it; the reload re-readies (3).
            pin_time: p.prefire,
            throw_delay: p.release_key,
            throw_time: p.postfire,
            redraw_time: p.reload,
            fuse: p.fuse,
            // Unused: `launch` makes the projectile and this module flies
            // it (`GameFlown`).
            throw: ThrowRule {
                pitch_lift: 0.0,
                pitch_scale: 1.0,
                pitch_scale_up: 1.0,
                speed_per_deg: 0.0,
                speed_max: p.speed * unit,
                forward: 0.0,
            },
            flight: Flight {
                half: 0.0,
                gravity: OBJECT_GRAVITY * unit,
                elasticity: 1.0 - DEFAULT_DAMPEN,
                player_elasticity: 1.0,
                elasticity_cap: 1.0,
                stop_speed: REST_SPEED * unit,
                floor_normal: REST_FLOOR_NORMAL,
                breakable_damage: 0.0,
                breakable_slowdown: 1.0,
                water_slowdown: 1.0,
                water_entry: 1.0,
                max_velocity: f32::MAX,
                check_interval: SERVER_FRAME as f32,
                spin_roll: 0.0,
                spin_pitch: 0.0,
            },
            effect,
            pin_sounds: Vec::new(),
            bounce_sound: None,
            water_sounds: None,
            pin: false,
            throw_at: None,
            redraw_at: None,
        },
        ThrowOptions {
            release_at_pin_end: true,
            hold_loop: Some(p.throw_loop),
            drop_primed: false,
            game_launch: true,
        },
        CaGrenadeData {
            params: p.clone(),
            unit,
            hp,
        },
        CaPin::default(),
    ));
}

// ---------------------------------------------------------------------------
// The live grenade

/// One contact of a grenade's flight (for tests and tools): the step, the
/// point (u) and the velocity after it (u/s), and its bounce sound's
/// loudness (6, step 4: min(1, |v''| h / 200), v'' the damped velocity
/// before the rest test; 1 for a character touch).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub step: u32,
    pub at: Vec3,
    pub velocity: Vec3,
    pub rest: bool,
    pub loudness: f32,
}

/// The surface a detonating grenade last bounced off or touched
/// (`CaGrenade::last_surface`), sent with its `Detonated` for the clients'
/// explosion look (`effects`): the projectile it belongs to.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct CaBlastSurface {
    pub projectile: Entity,
    pub surface: Option<String>,
}

/// A bounce sound's loudness: min(1, |v''| h / 200) (6, step 4).
pub fn bounce_loudness(v: Vec3, n: Vec3, h: f32) -> f32 {
    let h = h.clamp(0.0, 1.0);
    let damped = h * (v - 2.0 * v.dot(n) * n - 2.0 * SURFACE_PUSH * n);
    (damped.length() * h / 200.0).min(1.0)
}

/// A Combat Arms grenade in the world (the server's), on its `Projectile`.
/// Position and velocity in world units.
#[derive(Component, Clone, Debug)]
pub struct CaGrenade {
    pub weapon: &'static str,
    pub params: GrenadeParams,
    pub unit: f32,
    pub hp: f32,
    pub thrower: Option<Entity>,
    /// u, u/s.
    pub pos: Vec3,
    pub velocity: Vec3,
    /// Where it was at the server frame before (u): it is drawn on the
    /// line from there to `pos` over the frame that follows
    /// (`draw_between_frames`), one server frame behind.
    pub drawn_from: Vec3,
    /// The server frame its start time is (the one before the first it is
    /// stepped in; None until then).
    pub created: Option<u64>,
    /// Physics steps run.
    pub steps: u32,
    pub resting: bool,
    /// The one-time nudge of a grenade that came to rest, u.
    pub nudge: Option<Vec3>,
    /// It touched something in its last step, moving along `last_dir`.
    pub touched: bool,
    pub last_dir: Vec3,
    /// Hit points left.
    pub health: f32,
    /// Creation order (updates run in it).
    pub seq: u64,
    pub contacts: Vec<Contact>,
    /// The surface (the map's tag; "6" for a character) of its last bounce
    /// or touch: what clients see it detonate on, even in mid-air
    /// (`effects_sounds.md` Corrections 2026-10-22, 1). None: it never
    /// touched anything (its own metal).
    pub last_surface: Option<String>,
    /// Its hit points are gone (13): it goes off on the next frame.
    pub destroyed: bool,
    /// Visual spin: the accumulated angles and their rates (`tumble`).
    pub tumble: Tumble,
}

/// Server frames (`SERVER_FRAME` apart, the first tick at or after each
/// one's time runs it) and the grenades' creation counter.
#[derive(Resource, Clone, Copy, Debug)]
pub struct CaServerFrames {
    pub interval: f64,
    pub frame: u64,
    pub next_seq: u64,
}

impl Default for CaServerFrames {
    fn default() -> Self {
        Self {
            interval: SERVER_FRAME,
            frame: 0,
            next_seq: 0,
        }
    }
}

impl CaServerFrames {
    pub fn time(&self) -> f64 {
        self.frame as f64 * self.interval
    }

    /// How far the clock (`now`, s) is into the server frame after the
    /// last one run (0..1): what is drawn between frames is this far from
    /// its place at the frame before to its place now (`draw_between_frames`,
    /// `special::sync_objects`).
    pub fn blend(&self, now: f64) -> f32 {
        (((now - self.time()) / self.interval) as f32).clamp(0.0, 1.0)
    }
}

/// A pending flash: the victim's server timer (10).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlashTimer {
    pub victim: Entity,
    pub due: f64,
    pub time: f32,
}

/// An over-time damage entry (12): HP/s for the seconds left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverTime {
    pub victim: Entity,
    /// The object that applies it: entries are keyed by it (a sphere's
    /// refresh replaces its own, another's adds, up to `PROG_SLOTS`).
    pub source: Option<Entity>,
    pub kind: OverTimeKind,
    pub rate: f32,
    pub remaining: f32,
    pub attacker: Option<Entity>,
    pub weapon: &'static str,
}

/// The server's lasting grenade effects on players.
#[derive(Resource, Clone, Debug, Default)]
pub struct CaGrenadeEffects {
    pub flashes: Vec<FlashTimer>,
    pub over_time: Vec<OverTime>,
}

/// An incendiary's or gas grenade's invisible sphere (12), metres for the
/// centre, world units for the radius.
#[derive(Component, Clone, Debug)]
pub struct CaDamageSphere {
    /// The shooter never burns in it (the rocket class's flames).
    pub exempt_thrower: bool,
    pub centre: Vec3,
    pub params: OverTimeParams,
    pub unit: f32,
    pub hp: f32,
    pub thrower: Option<Entity>,
    pub weapon: &'static str,
    pub created: u64,
    pub until: f64,
}

/// Over-time entries a character can hold (special_weapons.md constants).
pub const PROG_SLOTS: usize = 20;

/// Add `entry` to `list`: it replaces the one with the same victim, kind
/// and source; a character holds at most `PROG_SLOTS`, a full one giving
/// up the entry with the least damage left only for one that would do more.
pub fn add_over_time(list: &mut Vec<OverTime>, entry: OverTime) {
    if let Some(o) = list
        .iter_mut()
        .find(|o| o.victim == entry.victim && o.kind == entry.kind && o.source == entry.source)
    {
        *o = entry;
        return;
    }
    let left = |o: &OverTime| o.rate * o.remaining;
    let mine: Vec<usize> = (0..list.len()).filter(|&i| list[i].victim == entry.victim).collect();
    if mine.len() < PROG_SLOTS {
        list.push(entry);
        return;
    }
    let weakest = mine
        .into_iter()
        .min_by(|&a, &b| left(&list[a]).total_cmp(&left(&list[b])))
        .unwrap();
    if left(&entry) > left(&list[weakest]) {
        list[weakest] = entry;
    }
}

/// A smoke cloud's end (the server removes it).
#[derive(Component, Clone, Copy, Debug)]
pub struct CaSmoke {
    pub until: f64,
}

/// Linear falloff (9): full within a quarter of `radius`, none at it.
pub fn falloff(radius: f32, d: f32) -> f32 {
    let r0 = BLAST_FULL_FRACTION * radius;
    if d <= r0 {
        1.0
    } else if d <= radius {
        1.0 - (d - r0) / (radius - r0)
    } else {
        0.0
    }
}

/// One step of flight (5) in world units: from the velocity at the start,
/// the move and the stored velocity, under `gravity` (u/s², signed).
pub fn flight_step(v: Vec3, gravity: f32, dt: f32) -> (Vec3, Vec3) {
    let g = Vec3::new(0.0, gravity, 0.0);
    (v * dt + GRAVITY_INTEGRATION_FACTOR * g * dt * dt, v + g * dt)
}

/// The throw's cone n (thousandths, 4): weapon_firing.md's, the limits
/// `perturb` multiplied by the dynamic factor `g` and truncated first,
/// then n_min + ⌊(n_max − n_min) p⌋ g; none at all when n_max is 0.
pub fn throw_perturb(perturb: [u32; 2], g: f32, p: f32) -> f32 {
    let lo = (perturb[0].min(perturb[1]) as f64 * g as f64).floor();
    let hi = (perturb[1] as f64 * g as f64).floor();
    if hi <= 0.0 {
        return 0.0;
    }
    (lo + ((hi - lo) * p as f64).floor() * g as f64) as f32
}

/// How many times a character slows a grenade in one step (6): a touch
/// for each of its boxes the step `from`..`to` crosses (`min`..`max`, the
/// character's; then its hit-box object's, the bounds of its hit
/// `spheres`), counted only when the infinite line through `from` along
/// `v` passes within a sphere's radius.
pub fn character_touches(from: Vec3, to: Vec3, v: Vec3, min: Vec3, max: Vec3, spheres: &[(Vec3, f32)]) -> u32 {
    let Some(line) = v.try_normalize() else { return 0 };
    let passes = spheres.iter().any(|(c, r)| {
        let w = *c - from;
        (w - line * w.dot(line)).length_squared() <= r * r
    });
    if !passes {
        return 0;
    }
    let d = to - from;
    let (len, dir) = (d.length(), d.normalize_or_zero());
    let crosses = |lo: Vec3, hi: Vec3| box_entry(lo, hi, from, dir, len).is_some();
    let (slo, shi) = spheres.iter().fold((Vec3::MAX, Vec3::MIN), |(a, b), (c, r)| {
        (a.min(*c - Vec3::splat(*r)), b.max(*c + Vec3::splat(*r)))
    });
    crosses(min, max) as u32 + (!spheres.is_empty() && crosses(slo, shi)) as u32
}

/// A bounce (6): the post-gravity `v` mirrored on `n`, the engine's push
/// (0.05 u/s per contact, twice) inside, all kept by the hardness `h`; and
/// whether it comes to rest.
pub fn bounce(v: Vec3, n: Vec3, h: f32) -> (Vec3, bool) {
    let out = h.clamp(0.0, 1.0) * (v - 2.0 * v.dot(n) * n - 2.0 * SURFACE_PUSH * n);
    if out.xz().length() < REST_SPEED && n.y > REST_FLOOR_NORMAL {
        (Vec3::ZERO, true)
    } else {
        (out, false)
    }
}

/// The screen's whiteness `t` seconds into a flash of `time` (10):
/// clamp(1 − W S(u)), S the half-cosine to u = 0.5, then u.
pub fn flash_white(t: f32, time: f32) -> f32 {
    if time <= 0.0 {
        return 0.0;
    }
    let u = t / time;
    let s = if u < 0.5 {
        0.5 * (1.0 - (std::f32::consts::PI * u).cos())
    } else {
        u
    };
    (1.0 - WHITE_DECL * s).clamp(0.0, 1.0)
}

/// The flash's whiteness now from Chimera's `Blinded` as this module
/// writes it (white from `fade_start`, gone at `end` = T/2 later).
pub fn blinded_white(b: &Blinded, now: f64) -> f32 {
    let time = (2.0 * (b.end - b.fade_start)) as f32;
    flash_white((now - b.fade_start) as f32, time)
}

/// The white the client overlay draws for a flash (Corrections
/// 2026-10-22, 1): the cosine curve instead of Chimera's linear fade,
/// scaled by the blindness's alpha (1 for the flashes this module sends).
/// The client shell's `FlashFade` for Combat Arms.
pub fn blinded_alpha(b: &Blinded, now: f64) -> f32 {
    if now >= b.end || b.end <= b.fade_start {
        return if now >= b.end { 0.0 } else { b.alpha };
    }
    b.alpha * blinded_white(b, now.max(b.fade_start))
}

// ---------------------------------------------------------------------------
// Plugin

/// Combat Arms' grenades: the hook (before the weapon data loads:
/// `CombatArmsPlugins` adds this before `weapons`), the server's frames
/// and effects, the throw's sprint gate and the death release.
pub struct CaGrenadesPlugin;

impl Plugin for CaGrenadesPlugin {
    fn build(&self, app: &mut App) {
        app.register_weapon_hook::<GrenadeParams>(HOOK, throwable)
            .init_resource::<CaServerFrames>()
            .init_resource::<CaGrenadeEffects>()
            .insert_resource(CaGrenadeSurfaces::built_in())
            .add_message::<Detonated>()
            .add_message::<CaBlastSurface>()
            .add_message::<Damage>()
            .add_message::<Died>()
            .add_message::<callout::CaThrowCallout>()
            .add_observer(on_launch)
            // Predicted with the throw: sprint holds a grenade's fire.
            .add_systems(Predict::Weapons, sprint_gate.before(WeaponFrame))
            .add_systems(
                FixedUpdate,
                (
                    (pin_flags, server_frames)
                        .chain()
                        .after(WeaponFrame)
                        .in_set(SimSet::Weapons),
                    death_release.after(crate::core::apply_damage),
                    shot_grenades.before(server_frames).in_set(SimSet::Weapons),
                )
                    .run_if(crate::core::authoritative),
            );
        // A client draws a flying grenade's turn smoothed (C2).
        app.add_systems(
            Update,
            tumble::smooth.run_if(bevy::ecs::schedule::common_conditions::resource_equals(crate::core::NetRole::Client)),
        );
        // The call-out reaches a remote player as a server message.
        app.net_part("combat_arms_callout", |app| {
            use bevy_replicon::prelude::{Channel, ServerMessageAppExt};
            app.add_mapped_server_message::<callout::CaThrowCallout>(Channel::Ordered);
        });
    }
}

/// Damage to a live grenade (bullets, 13) comes off its hit points; at 1
/// or less it goes off on the next server frame, as at its fuse.
fn shot_grenades(mut damage: MessageReader<Damage>, mut grenades: Query<&mut CaGrenade>) {
    for d in damage.read() {
        let Ok(mut g) = grenades.get_mut(d.target) else { continue };
        if d.amount <= 0.0 || g.destroyed {
            continue;
        }
        g.health -= d.amount / g.hp;
        if g.health <= DESTROYED_AT {
            g.destroyed = true;
        }
    }
}

/// Fire is ignored while sprinting (3.1), in the sprint pose and its
/// post-sprint time: an idle grenade in hand can't start its throw.
fn sprint_gate(
    owners: Query<(&Inventory, &MovementState, Option<&CaSelection>)>,
    mut weapons: Query<(&Weapon, &Throwable, &mut WeaponState), With<CaGrenadeData>>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    let dt = clock.dt() as f64;
    for (weapon, t, mut st) in &mut weapons {
        let Some(owner) = weapon.owner else { continue };
        let Ok((inv, state, sel)) = owners.get(owner) else { continue };
        if t.primed() || t.redraw_at.is_some() || inv.active.is_none() {
            continue;
        }
        let held_until = if state.sprinting || sel.is_some_and(|s| s.sprint_pose) {
            now + dt
        } else {
            sel.map_or(0.0, |s| s.sprint_out_until)
        };
        if held_until > now + 1e-5 && st.next_primary < held_until {
            st.next_primary = held_until;
        }
    }
}

/// The server's "grenade in hand, pin pulled" flag (3).
fn pin_flags(
    owners: Query<(&Intent, &Inventory)>,
    mut weapons: Query<(Entity, &Weapon, &Throwable, &CaGrenadeData, &mut CaPin)>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    for (entity, weapon, t, data, mut pin) in &mut weapons {
        let held = weapon
            .owner
            .and_then(|o| owners.get(o).ok())
            .is_some_and(|(intent, inv)| inv.active == Some(entity) && intent.fire);
        let mut next = *pin;
        if !held {
            next = CaPin::default();
        } else {
            if next.since.is_none() && t.pin {
                next.since = Some(now);
            }
            if let Some(s) = next.since
                && now + 1e-5 >= s + data.params.prefire as f64
            {
                next.on = true;
            }
        }
        if *pin != next {
            *pin = next;
        }
    }
}

/// A throw leaving the hand of one of our grenades (Chimera's `Launch`,
/// the server's): its projectile (`launch`), in the throw's place among
/// its commands. Other weapons' launches are their games'.
fn on_launch(l: On<Launch>, ours: Query<(), With<CaGrenadeData>>, mut commands: Commands) {
    if ours.contains(l.weapon) {
        let l = *l.event();
        commands.queue(move |w: &mut World| launch(w, l));
    }
}

/// The projectile of a throw (4): from the eye along the view bent by the
/// weapon's square spread, at its speed, with none of the thrower's
/// velocity (Quirk 2).
pub fn launch(world: &mut World, l: Launch) {
    let Some(data) = world.get::<CaGrenadeData>(l.weapon).cloned() else {
        return;
    };
    let p = &data.params;
    let fraction = world.get::<CaSpread>(l.weapon).map_or(0.0, |s| s.fraction());
    let n = throw_perturb(p.perturb, p.dynamic_perturb, fraction);
    let mut rng = shared_random(l.command, HOOK, 0);
    let (u1, u2) = (rng.next(), rng.next());
    let aim = Quat::from_euler(EulerRot::YXZ, l.yaw, l.pitch, 0.0);
    let dir = square_dir(aim, n / 1000.0, u1, u2);
    let at = fire_offset(world, l.thrower, l.eye, dir, p.fire_offset * data.unit);
    if p.call_out {
        callout::send(world, l.thrower, l.command);
    }
    super::effects::sounds::write_create_sound(world, &p.create_sound, p.create_radius, p.call_out, l.eye);
    // Pitcher's force (gear_perks.md 14; 1 without it).
    let force = super::perks::throw_force(world, l.thrower);
    spawn_grenade(world, l.weapon, Some(l.thrower), at, dir * p.speed * data.unit * force);
}

/// Where the server starts a throw (Corrections 1): `eye` moved `offset`
/// (m) along `dir` when nothing but the thrower is on that segment, else
/// the eye.
pub fn fire_offset(world: &mut World, thrower: Entity, eye: Vec3, dir: Vec3, offset: f32) -> Vec3 {
    if offset == 0.0 {
        return eye;
    }
    let Ok(ray) = Dir3::new(dir * offset.signum()) else { return eye };
    let mut state = bevy::ecs::system::SystemState::<SpatialQuery>::new(world);
    let Ok(spatial) = state.get(world) else { return eye };
    let filter = SpatialQueryFilter::from_excluded_entities([thrower]).with_mask(SOLID_LAYERS);
    match spatial.cast_ray(eye, ray, offset.abs(), true, &filter) {
        Some(_) => eye,
        None => eye + dir * offset,
    }
}

/// A live grenade of `weapon`'s kind (a Combat Arms `Throwable`) at `at`
/// (m) with `velocity` (m/s), credited to `thrower`, full fuse: Chimera's
/// `Projectile`, flown here. None for a weapon that isn't one.
pub fn spawn_grenade(
    world: &mut World,
    weapon: Entity,
    thrower: Option<Entity>,
    at: Vec3,
    velocity: Vec3,
) -> Option<Entity> {
    let data = world.get::<CaGrenadeData>(weapon)?.clone();
    let id = world.get::<Weapon>(weapon)?.id;
    let e = spawn_projectile(world, weapon, at, velocity)?;
    let seq = {
        let mut frames = world.resource_mut::<CaServerFrames>();
        frames.next_seq += 1;
        frames.next_seq
    };
    let mut rng = shared_random(seq as u32, "combat_arms:spin", 0);
    let spin = spin_rates(&mut rng);
    let mut ent = world.entity_mut(e);
    if let Some(mut p) = ent.get_mut::<Projectile>() {
        p.thrower = thrower;
    }
    ent.insert((
        GameFlown,
        // A new round leaves it (15).
        SurvivesRestart,
        // Drawn with its projectile record's model (`thrown_key`).
        ShownItem(thrown_key(id)),
        // Its sounds in flight and at rest (special_weapons.md Corrections
        // (2026-10-31) 3).
        super::effects::attached::CaFlightSounds::default(),
        super::effects::attached::CaFlightLook(data.params.look.clone()),
        CaGrenade {
            weapon: id,
            params: data.params.clone(),
            unit: data.unit,
            hp: data.hp,
            thrower,
            pos: at / data.unit,
            drawn_from: at / data.unit,
            velocity: velocity / data.unit,
            created: None,
            steps: 0,
            resting: false,
            nudge: None,
            touched: false,
            last_dir: Vec3::ZERO,
            health: data.params.health,
            seq,
            contacts: Vec::new(),
            last_surface: None,
            destroyed: false,
            tumble: Tumble::new(spin),
        },
    ));
    Some(e)
}

/// A player dying with the pin pulled releases a live grenade of that
/// weapon at its hull centre, 500 u/s along the body's facing, full fuse,
/// ammo unchecked (14, Quirk 5).
fn death_release(
    mut died: MessageReader<Died>,
    owners: Query<(&Inventory, &Transform, &MovementState, &Intent)>,
    mut weapons: Query<(&CaGrenadeData, &mut CaPin)>,
    mut commands: Commands,
) {
    for d in died.read() {
        let Ok((inv, t, state, intent)) = owners.get(d.entity) else {
            continue;
        };
        let Some(active) = inv.active else { continue };
        let Ok((data, mut pin)) = weapons.get_mut(active) else {
            continue;
        };
        if !pin.on {
            continue;
        }
        *pin = CaPin::default();
        let centre = t.translation + (state.hull_min + state.hull_max) / 2.0;
        let velocity = intent.yaw_rotation() * Vec3::NEG_Z * DEATH_RELEASE_SPEED * data.unit;
        let thrower = d.entity;
        commands.queue(move |w: &mut World| {
            spawn_grenade(w, active, Some(thrower), centre, velocity);
        });
    }
}

// ---------------------------------------------------------------------------
// The server's frames

/// The world as grenades see it (read only).
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct GrenadeWorld<'w, 's> {
    pub(crate) spatial: SpatialQuery<'w, 's>,
    colliders: Query<'w, 's, &'static ColliderOf>,
    surfaces: Query<'w, 's, &'static PropSurface>,
    rigid: Query<'w, 's, &'static RigidBody>,
    props: Query<'w, 's, (), With<PropEntity>>,
    people: Query<'w, 's, (), With<Intent>>,
    #[allow(clippy::type_complexity)]
    characters: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Intent,
            &'static MovementState,
            Option<&'static ColliderAabb>,
            Option<&'static HitSpheres>,
            &'static Health,
            Option<&'static CaFlashProtection>,
            (Option<&'static crate::slots::GameIdentity>, Option<&'static crate::core::Hitboxes>),
        ),
        Without<CaGrenade>,
    >,
    breakables: Query<'w, 's, (Entity, &'static ColliderAabb), (With<Damageable>, Without<Intent>)>,
    /// Special weapons' objects (rockets, mines, ...) a blast damages.
    pub(crate) targets: Query<'w, 's, (Entity, &'static super::special::CaTarget)>,
    prop_boxes: Query<'w, 's, (Entity, &'static ColliderAabb), With<PropEntity>>,
    volumes: Query<'w, 's, &'static CaVolume>,
    pub(crate) table: Res<'w, CaGrenadeSurfaces>,
    pub(crate) entities: &'w bevy::ecs::entity::Entities,
    /// Spawn-protected characters (Chimera's `core::SpawnProtected`, a
    /// party's): not among `bodies` in their bullets window.
    protected: (Query<'w, 's, &'static crate::core::SpawnProtected>, Option<Res<'w, crate::core::SimClock>>),
}

/// What a character is to grenades: its box (m), centre (m), spheres (m)
/// and head top (m, y).
pub(crate) struct Body {
    pub(crate) entity: Entity,
    pub(crate) min: Vec3,
    pub(crate) max: Vec3,
    pub(crate) centre: Vec3,
    pub(crate) spheres: Vec<(Vec3, f32)>,
    /// Each sphere's location and damage factor (the hit table's).
    pub(crate) zones: Vec<(Location, f32)>,
    pub(crate) head_top: f32,
    pub(crate) facing: Vec3,
    pub(crate) health: f32,
    flash_protection: f32,
}

impl GrenadeWorld<'_, '_> {
    pub(crate) fn body(&self, collider: Entity) -> Entity {
        self.colliders.get(collider).map_or(collider, |c| c.body)
    }

    pub(crate) fn surface(&self, collider: Entity) -> Option<String> {
        self.surfaces
            .get(collider)
            .or_else(|_| self.surfaces.get(self.body(collider)))
            .ok()
            .map(|s| s.0.to_lowercase())
    }

    pub(crate) fn invisible(&self, collider: Entity) -> bool {
        self.surface(collider).as_deref() == Some("invisible")
    }

    /// The first world surface on the segment (m): distance (m), normal,
    /// collider. Characters, props, `skip` and invisible surfaces are
    /// passed; traces-only detail and solid world models are met (5, 6).
    pub(crate) fn cast(&self, from: Vec3, to: Vec3, skip: Option<Entity>) -> Option<(f32, Vec3, Entity)> {
        let d = to - from;
        let dir = Dir3::new(d).ok()?;
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        self.spatial
            .cast_ray_predicate(from, dir, d.length(), true, &filter, &|e| {
                let body = self.body(e);
                Some(body) != skip && !self.people.contains(body) && !self.props.contains(body) && !self.invisible(e)
            })
            .map(|h| (h.distance, h.normal, h.entity))
    }

    /// The first prop on the segment (m): the fraction along it and its
    /// surface's hardness (6: a touch, no mirror).
    pub(crate) fn prop_touch(&self, from: Vec3, to: Vec3) -> Option<(f32, f32)> {
        let d = to - from;
        let (len, dir) = (d.length(), d.normalize_or_zero());
        self.prop_boxes
            .iter()
            .filter_map(|(e, b)| {
                let t = box_entry(b.min, b.max, from, dir, len)?;
                Some((t / len.max(1e-9), self.table.hardness(self.surface(e).as_deref())))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// Whether the world stops the segment (m) one way (cover, 9): every
    /// static or kinematic surface queries meet (invisible and traces-only
    /// ones too, not player blockers), not characters or props.
    pub(crate) fn blocked_one_way(&self, from: Vec3, to: Vec3) -> bool {
        let d = to - from;
        let Ok(dir) = Dir3::new(d) else { return false };
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        self.spatial
            .cast_ray_predicate(from, dir, d.length(), true, &filter, &|e| {
                let body = self.body(e);
                let solid = self
                    .rigid
                    .get(e)
                    .or_else(|_| self.rigid.get(body))
                    .is_ok_and(|rb| !rb.is_dynamic());
                solid && !self.people.contains(body) && !self.props.contains(body)
            })
            .is_some()
    }

    /// Both cover traces (blast to target and back) clear.
    pub(crate) fn clear(&self, a: Vec3, b: Vec3) -> bool {
        !self.blocked_one_way(a, b) && !self.blocked_one_way(b, a)
    }

    /// The living characters, in no particular order (not the
    /// spawn-protected in their bullets window).
    pub(crate) fn bodies(&self) -> Vec<Body> {
        let mut out = Vec::new();
        let now = self.protected.1.as_ref().map_or(0.0, |c| c.now);
        for (entity, t, intent, state, aabb, spheres, health, flash, (identity, boxes)) in &self.characters {
            // The spawn-protected in their bullets window aren't there:
            // grenades and bolts fly through, blasts and flashes leave
            // them out.
            if health.current <= 0.0 || self.protected.0.get(entity).is_ok_and(|p| p.passes_attacks(now)) {
                continue;
            }
            let data = spheres.map_or(*MALE, |s| s.0);
            let Some((min, max, spheres)) = body_geometry(t, intent, state, aabb, data) else {
                continue;
            };
            // Another game's body: its own shapes (a box as its bounding
            // sphere, as our rules take it), the box holding them.
            if let Some(shapes) = super::weapons::their_shapes(t, intent, aabb, identity, boxes) {
                let (min, max) = super::weapons::box_holding(min, max, &shapes);
                let head_top = shapes
                    .0
                    .iter()
                    .filter(|h| h.region.part() == crate::core::BodyPart::Head)
                    .map(|h| h.bounds().1.y)
                    .fold(f32::MIN, f32::max);
                out.push(Body {
                    entity,
                    min,
                    max,
                    centre: (min + max) / 2.0,
                    zones: shapes
                        .0
                        .iter()
                        .map(|h| {
                            let zone = h.scale.unwrap_or_else(|| super::weapons::zone_factor(h.region));
                            (super::weapons::location_of(h.region), zone)
                        })
                        .collect(),
                    spheres: shapes.0.iter().map(|h| (h.center, h.reach())).collect(),
                    head_top: if head_top == f32::MIN { max.y } else { head_top },
                    facing: intent.yaw_rotation() * Vec3::NEG_Z,
                    health: health.current,
                    flash_protection: flash.map_or(0.0, |f| f.0),
                });
                continue;
            }
            let head_top = data
                .iter()
                .zip(&spheres)
                .filter(|(s, _)| s.location == Location::Head)
                .map(|(_, (c, r))| c.y + r)
                .fold(f32::MIN, f32::max);
            out.push(Body {
                entity,
                min,
                max,
                centre: (min + max) / 2.0,
                zones: data.iter().map(|s| (s.location, s.factor)).collect(),
                spheres,
                head_top: if head_top == f32::MIN { max.y } else { head_top },
                facing: intent.yaw_rotation() * Vec3::NEG_Z,
                health: health.current,
                flash_protection: flash.map_or(0.0, |f| f.0),
            });
        }
        out
    }

    /// The volumes `p` (m) is inside (7): (liquid, viscosity, current
    /// u/s) each (other games' water too: `volumes::BrushWater`).
    pub(crate) fn volumes_at(&self, p: Vec3) -> Vec<(bool, f32, Vec3)> {
        let mut out: Vec<(bool, f32, Vec3)> = self
            .volumes
            .iter()
            .filter(|v| v.contains(p))
            .map(|v| (v.liquid, v.viscosity, v.current))
            .collect();
        // (Other games' water is `Water` volumes already: `volumes::BrushWater`.)
        out
    }
}

/// Where a segment first enters a sphere: the fraction along it.
pub(crate) fn sphere_entry(from: Vec3, to: Vec3, centre: Vec3, r: f32) -> Option<f32> {
    let d = to - from;
    let a = d.length_squared();
    if a <= 0.0 || r <= 0.0 {
        return None;
    }
    let m = from - centre;
    let c = m.length_squared() - r * r;
    if c <= 0.0 {
        return Some(0.0);
    }
    let b = m.dot(d);
    let disc = b * b - a * c;
    if disc < 0.0 {
        return None;
    }
    let t = (-b - disc.sqrt()) / a;
    (0.0..=1.0).contains(&t).then_some(t)
}

/// What a frame changes besides the grenades: damage dealt (with each
/// target's health as it goes), detonations, effects.
pub(crate) struct Frame {
    pub(crate) time: f64,
    pub(crate) now: f64,
    pub(crate) dt: f32,
    pub(crate) frame: u64,
    health: HashMap<Entity, f32>,
    pub(crate) gone: Vec<Entity>,
    pub(crate) queue: Vec<Entity>,
}

impl Frame {
    /// Server frame number `frame` at `time` (the game's clock `now`, s).
    pub(crate) fn new(frame: u64, time: f64, now: f64, dt: f32) -> Self {
        Self {
            time,
            now,
            dt,
            frame,
            health: HashMap::new(),
            gone: Vec::new(),
            queue: Vec::new(),
        }
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Out<'w, 's> {
    pub(crate) damage: MessageWriter<'w, Damage>,
    pub(crate) detonated: MessageWriter<'w, Detonated>,
    pub(crate) blast_surface: MessageWriter<'w, CaBlastSurface>,
    pub(crate) rules: DamageRules<'w, 's>,
    pub(crate) armour: Query<'w, 's, (&'static mut crate::weapon::Armor, Option<&'static crate::weapon::ArmorPieces>)>,
    pub(crate) effects: ResMut<'w, CaGrenadeEffects>,
    pub(crate) commands: Commands<'w, 's>,
    pub(crate) ai: super::weapons::AiHits<'w, 's>,
}

impl Out<'_, '_> {
    /// Deal `amount` (health units) to `target` by `attacker`: refused by
    /// the team rules, death at `DEATH_AT` HP or less (the rest of its
    /// health), counted against its health this frame.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn deal(
        &mut self,
        f: &mut Frame,
        target: Entity,
        health: f32,
        amount: f32,
        kind: DamageKind,
        attacker: Option<Entity>,
        weapon: &'static str,
        point: Vec3,
        hp: f32,
    ) {
        let mut damage = Damage {
            target,
            attacker,
            amount,
            point,
            dir: Vec3::ZERO,
            hitgroup: Hitgroup::Generic,
            kind,
            weapon: Some(weapon),
            force: Vec3::ZERO,
        };
        if amount <= 0.0 || self.rules.refuse(&damage) {
            return;
        }
        let left = f.health.entry(target).or_insert(health);
        if *left <= 0.0 {
            return;
        }
        if *left - amount <= DEATH_AT * hp + 1e-6 {
            damage.amount = amount.max(*left);
        }
        *left -= damage.amount;
        self.damage.write(damage);
    }

    /// `amount` (health) of `damage_type` on `victim`'s torso after its
    /// armour (weapon_firing.md, Armour), which takes its share if the
    /// hit lands; an AI's x `AIDamageFactor` first and capped on a player
    /// after (weapon_firing.md Corrections 7).
    pub(crate) fn armoured(&mut self, victim: Entity, attacker: Option<Entity>, amount: f32, damage_type: i32, hp: f32) -> f32 {
        let amount = amount * self.ai.factor(attacker);
        let lands = !self.spared(victim, attacker);
        let hit = ArmourHit {
            damage_type,
            location: Location::Torso,
            zone: 1.0,
            infected: false,
        };
        let after = match self.armour.get_mut(victim) {
            Ok((a, pieces)) => armour_and_zone(amount / hp, hit, Some(a.into_inner()), pieces, lands),
            Err(_) => armour_and_zone(amount / hp, hit, None, None, lands),
        };
        self.ai.cap(attacker, victim, after, false) * hp
    }

    /// Whether the team rules spare `victim` from `attacker`'s grenade.
    pub(crate) fn spared(&self, victim: Entity, attacker: Option<Entity>) -> bool {
        self.rules.refuse(&Damage {
            target: victim,
            attacker,
            amount: 1.0,
            point: Vec3::ZERO,
            dir: Vec3::ZERO,
            hitgroup: Hitgroup::Generic,
            kind: DamageKind::Blast,
            weapon: None,
            force: Vec3::ZERO,
        })
    }
}

pub(crate) type Grenades<'w, 's> = Query<'w, 's, (Entity, &'static mut CaGrenade, &'static mut Transform, &'static mut Projectile)>;

/// Run the server frames due by this tick.
#[allow(clippy::too_many_arguments)]
pub(crate) fn server_frames(
    mut frames: ResMut<CaServerFrames>,
    clock: Res<SimClock>,
    mut grenades: Grenades,
    mut spheres: Query<(Entity, &CaDamageSphere)>,
    smokes: Query<(Entity, &CaSmoke)>,
    world: GrenadeWorld,
    mut out: Out,
) {
    let due = ((clock.now + 1e-9) / frames.interval).floor() as u64;
    while frames.frame < due {
        frames.frame += 1;
        let mut f = Frame::new(frames.frame, frames.time(), clock.now, frames.interval as f32);
        run_frame(&mut f, &mut grenades, &mut spheres, &world, &mut out);
    }
    draw_between_frames(&frames, clock.now, &mut grenades);
    for (e, s) in &smokes {
        if clock.now >= s.until {
            out.commands.entity(e).try_despawn();
        }
    }
}

/// Where a grenade is drawn (its `Transform`, what the network and the
/// presentation show) between server frames: on the line from its place
/// at the frame before (`drawn_from`) to its place now, by how far the
/// clock is into the server frame after this one. A grenade moves only on
/// the server's 20 frames a second (its `pos`, which everything in the
/// game reads); drawn at those frames it hopped 50 ms at a time at every
/// rendered frame between, so the arc looked wrong. The look is one
/// server frame behind, as a client between two server updates.
pub(crate) fn draw_between_frames(frames: &CaServerFrames, now: f64, grenades: &mut Grenades) {
    let a = frames.blend(now);
    for (_, g, mut transform, _) in grenades.iter_mut() {
        let at = g.drawn_from.lerp(g.pos, a) * g.unit;
        if transform.translation != at {
            transform.translation = at;
        }
    }
}

fn run_frame(
    f: &mut Frame,
    grenades: &mut Grenades,
    spheres: &mut Query<(Entity, &CaDamageSphere)>,
    world: &GrenadeWorld,
    out: &mut Out,
) {
    // 1-2. Grenades in creation order; what each is to the others (6).
    let mut order: Vec<(u64, Entity)> = grenades.iter().map(|(e, g, ..)| (g.seq, e)).collect();
    order.sort_unstable_by_key(|(s, _)| *s);
    let mut others: Vec<Other> = grenades
        .iter()
        .map(|(e, g, ..)| Other {
            entity: e,
            pos: g.pos,
            half: Vec3::from(g.params.box_half),
            thrower: g.thrower,
            resting: g.resting,
        })
        .collect();
    for (_, e) in order {
        if f.gone.contains(&e) {
            continue;
        }
        let fused = {
            let Ok((_, mut g, mut transform, _)) = grenades.get_mut(e) else {
                continue;
            };
            // New: its start time is the frame before; stepped in this one.
            let created = *g.created.get_or_insert(f.frame - 1);
            g.drawn_from = g.pos;
            if let Some(n) = g.nudge.take() {
                g.pos += n;
                g.drawn_from = g.pos;
                transform.translation = g.pos * g.unit;
            }
            let elapsed = (f.frame - created) as f64 * f.dt as f64;
            g.destroyed || elapsed + 1e-9 >= g.params.fuse as f64
        };
        if fused {
            f.queue.push(e);
            detonations(f, grenades, world, out);
            continue;
        }
        let touched = {
            let Ok((_, mut g, mut transform, mut projectile)) = grenades.get_mut(e) else {
                continue;
            };
            if g.resting {
                continue;
            }
            // The spin comes first in an update and only while it moves
            // (C2): the rotation is set from the angles; a bounce below
            // draws the rates the next update uses.
            transform.rotation = g.tumble.step(f.dt);
            let Some(touched) = step(f, e, &mut g, world, &others) else {
                // Into the sky: gone without a trace.
                f.gone.push(e);
                out.commands.entity(e).try_despawn();
                continue;
            };
            projectile.velocity = g.velocity * g.unit;
            if let Some(o) = others.iter_mut().find(|o| o.entity == e) {
                o.pos = g.pos;
                o.resting = g.resting;
            }
            touched
        };
        // The grenades it touched meet its metal.
        let metal = world.table.hardness(Some(GRENADE_SURFACE));
        for o in touched {
            if let Ok((_, mut other, _, mut projectile)) = grenades.get_mut(o) {
                other.velocity *= metal;
                projectile.velocity = other.velocity * other.unit;
            }
        }
    }
    // 3. Lasting explosions refresh the entries of the players inside.
    let bodies = world.bodies();
    for (e, s) in spheres.iter() {
        if s.thrower.is_some_and(|t| !world.entities.contains(t)) || f.time + 1e-9 >= s.until {
            out.commands.entity(e).try_despawn();
            continue;
        }
        if s.created == f.frame {
            continue;
        }
        let r = s.params.radius * s.unit;
        for b in &bodies {
            let to = b.centre;
            if to.distance(s.centre) > r || out.spared(b.entity, s.thrower) {
                continue;
            }
            if s.exempt_thrower && Some(b.entity) == s.thrower {
                continue;
            }
            let start = s.centre + (to - s.centre).normalize_or_zero() * OVER_TIME_TRACE_START * s.unit;
            if !world.clear(start, to) && !world.clear(start, to.with_y(b.head_top)) {
                continue;
            }
            let entry = OverTime {
                victim: b.entity,
                source: Some(e),
                kind: s.params.kind,
                rate: s.params.rate * s.hp,
                remaining: s.params.duration,
                attacker: s.thrower,
                weapon: s.weapon,
            };
            add_over_time(&mut out.effects.over_time, entry);
        }
    }
    // 4. Each player's flash timer and over-time damage.
    let timers = std::mem::take(&mut out.effects.flashes);
    for t in timers {
        if f.time + 1e-9 < t.due {
            out.effects.flashes.push(t);
            continue;
        }
        if t.time >= FLASH_MIN_TIME
            && let Ok(mut ent) = out.commands.get_entity(t.victim)
        {
            ent.insert((
                Blinded {
                    alpha: 1.0,
                    fade_start: f.now,
                    end: f.now + (t.time / WHITE_DECL) as f64,
                },
                CaFlashed { at: f.now, time: t.time },
            ));
        }
    }
    let entries = std::mem::take(&mut out.effects.over_time);
    for mut o in entries {
        let Some(b) = bodies.iter().find(|b| b.entity == o.victim) else {
            continue;
        };
        let mut amount = o.rate * f.dt;
        let hp = 0.01;
        let damage_type = match o.kind {
            OverTimeKind::Burn => BURN,
            OverTimeKind::Poison => POISON,
            OverTimeKind::Bleed => BLEED,
        };
        amount = out.armoured(o.victim, o.attacker, amount, damage_type, hp);
        let kind = match o.kind {
            OverTimeKind::Burn => DamageKind::Burn,
            OverTimeKind::Poison | OverTimeKind::Bleed => DamageKind::Generic,
        };
        out.deal(f, o.victim, b.health, amount, kind, o.attacker, o.weapon, b.centre, hp);
        o.remaining -= f.dt;
        if o.remaining > 1e-4 {
            out.effects.over_time.push(o);
        }
    }
}

/// Another grenade as one stepping sees it (6): its box (u) and thrower.
struct Other {
    entity: Entity,
    pos: Vec3,
    half: Vec3,
    thrower: Option<Entity>,
    resting: bool,
}

/// One physics step (5-7) of grenade `me`, not at rest: the grenades it
/// touched (each slowed by its metal in turn), or None when it went into
/// the sky.
fn step(f: &Frame, me: Entity, g: &mut CaGrenade, world: &GrenadeWorld, others: &[Other]) -> Option<Vec<Entity>> {
    let dt = f.dt;
    let unit = g.unit;
    g.touched = false;
    let mut v = g.velocity;
    if v.length_squared() < SLOW_STOP {
        v = Vec3::ZERO;
    }
    // Volumes (7): viscosity then current (above 1 u/s, viscosity above
    // 0), each volume in turn; a liquid's gravity.
    let mut gravity = -OBJECT_GRAVITY;
    for (liquid, c, current) in world.volumes_at(g.pos * unit) {
        let speed = v.length();
        if c > 0.0 && speed > VISCOSITY_MIN_SPEED {
            v = v / speed * (speed - LIQUID_VISCOSITY_SCALE * c * dt).max(0.0) + current * dt;
        }
        if liquid {
            gravity = LIQUID_GRAVITY;
        }
    }
    // The velocity at the start of the step, before its gravity: an
    // at-rest nudge goes back along it (6).
    let start_dir = v.normalize_or_zero();
    let (delta, after) = flight_step(v, gravity, dt);
    g.velocity = after;
    g.steps += 1;
    if delta == Vec3::ZERO {
        return Some(Vec::new());
    }
    let from = g.pos;
    let to = from + delta;
    let dir = delta.normalize();
    // The world.
    let wall = world.cast(from * unit, to * unit, g.thrower);
    let wall_t = wall.map_or(f32::MAX, |(d, ..)| d / (delta.length() * unit));
    let mut touched = Vec::new();
    // Other grenades in flight whose box the step crosses: both slowed by
    // the other's metal; between one thrower's, only if this kind touches
    // its own.
    for o in others {
        if o.entity == me || o.resting || (o.thrower == g.thrower && !g.params.hits_same_kind) {
            continue;
        }
        if box_entry(o.pos - o.half, o.pos + o.half, from, dir, delta.length())
            .is_some_and(|t| t / delta.length() < wall_t)
        {
            g.velocity *= world.table.hardness(Some(GRENADE_SURFACE));
            g.touched = true;
            touched.push(o.entity);
        }
    }
    // Props the step crosses: slowed by their surface, no mirror.
    if let Some((t, h)) = world.prop_touch(from * unit, to * unit)
        && t < wall_t
    {
        g.velocity *= h;
        g.touched = true;
    }
    // Characters (not the thrower): a touch per box crossed while the line
    // of motion passes a hit sphere, each slowing it by flesh, no mirror.
    let flesh = world.table.hardness(Some("flesh"));
    let mut snap: Option<f32> = None;
    for b in world.bodies() {
        if Some(b.entity) == g.thrower
            || box_entry(b.min, b.max, from * unit, dir, delta.length() * unit)
                .is_none_or(|t| t / (delta.length() * unit) >= wall_t)
        {
            continue;
        }
        let n = character_touches(from * unit, to * unit, g.velocity, b.min, b.max, &b.spheres);
        if n == 0 {
            continue;
        }
        g.velocity *= flesh.powi(n as i32);
        g.touched = true;
        g.last_surface = Some("6".to_string());
        // Snapped to where the step enters a sphere (the short trace).
        let entry = b
            .spheres
            .iter()
            .filter_map(|(c, r)| sphere_entry(from * unit, to * unit, *c, *r))
            .fold(f32::MAX, f32::min);
        if entry < wall_t && snap.is_none_or(|s| entry < s) {
            snap = Some(entry);
        }
        g.contacts.push(Contact {
            step: g.steps,
            at: from + delta * entry.min(1.0),
            velocity: g.velocity,
            rest: false,
            loudness: 1.0,
        });
    }
    if let Some(t) = snap {
        g.pos = from + delta * t;
        g.last_dir = dir;
        return Some(touched);
    }
    let Some((distance, n, collider)) = wall else {
        g.pos = to;
        g.last_dir = dir;
        return Some(touched);
    };
    let surface = world.surface(collider);
    if surface.as_deref() == Some("sky") {
        return None;
    }
    let n = n.normalize_or(Vec3::Y);
    let at = from + dir * (distance / unit);
    let h = world.table.hardness(surface.as_deref());
    let loudness = bounce_loudness(g.velocity, n, h);
    let (v2, rest) = bounce(g.velocity, n, h);
    g.pos = at + n * SKIN;
    g.velocity = v2;
    g.touched = true;
    g.last_surface = Some(surface.clone().unwrap_or_else(|| "0".to_string()));
    g.last_dir = dir;
    g.contacts.push(Contact {
        step: g.steps,
        at,
        velocity: v2,
        rest,
        loudness,
    });
    // A new spin at every bounce.
    let mut rng = shared_random(g.seq as u32, "combat_arms:spin", g.steps as i32);
    g.tumble.rates = spin_rates(&mut rng);
    if rest {
        g.resting = true;
        g.nudge = Some(-start_dir * g.params.box_half[1] * h.max(0.1));
    }
    Some(touched)
}

/// Detonate the queued grenades (8), and those their blasts set off (13).
fn detonations(f: &mut Frame, grenades: &mut Grenades, world: &GrenadeWorld, out: &mut Out) {
    while let Some(e) = f.queue.pop() {
        if f.gone.contains(&e) {
            continue;
        }
        let Ok((_, g, ..)) = grenades.get(e) else { continue };
        let g = g.clone();
        f.gone.push(e);
        out.commands.entity(e).try_despawn();
        let at_u = if g.touched { g.pos - g.last_dir * DETONATION_BACKOFF } else { g.pos };
        let at = at_u * g.unit;
        let p = &g.params;
        out.detonated.write(Detonated {
            kind: match p.area_type {
                AreaType::Flash => GrenadeKind::Flash,
                AreaType::Smoke => GrenadeKind::Smoke,
                AreaType::Explode | AreaType::None => GrenadeKind::Blast,
            },
            weapon: g.weapon,
            thrower: g.thrower,
            at,
            ground: None,
            projectile: e,
            shake: None,
        });
        out.blast_surface.write(CaBlastSurface {
            projectile: e,
            surface: g.last_surface.clone(),
        });
        if p.area_damage > 0.0 && p.area_radius > 0.0 {
            match p.area_type {
                AreaType::Explode => blast(f, e, &g, at, grenades, world, out),
                AreaType::Flash => flash(f, &g, at, world, out),
                AreaType::Smoke | AreaType::None => {}
            }
        }
        if p.area_type == AreaType::Smoke {
            let look = smoke_look();
            out.commands.spawn((
                Name::new("Smoke cloud"),
                SmokeCloud {
                    centre: at,
                    started: f.now,
                    smoke: look.clone(),
                    grenade: None,
                    weapon: Some(g.weapon),
                },
                CaSmoke {
                    until: f.now + look.fade_end as f64,
                },
                SurvivesRestart,
                Transform::from_translation(at),
            ));
        }
        if let Some(o) = p.over_time
            && o.lifetime > 0.0
        {
            out.commands.spawn((
                Name::new("Damage sphere"),
                Transform::from_translation(at),
                CaDamageSphere {
                    exempt_thrower: false,
                    centre: at,
                    params: o,
                    unit: g.unit,
                    hp: g.hp,
                    thrower: g.thrower,
                    weapon: g.weapon,
                    created: f.frame,
                    until: f.time + o.lifetime as f64,
                },
            ));
        }
    }
}

/// The blast distance (u) of an object `d` (u) from the blast whose
/// half-dims are `dims` (u): less the length of its dims over
/// `BIG_OBJECT_SLACK` (9; every object, characters too: Corrections 2).
pub fn blast_distance(d: f32, dims: Vec3) -> f32 {
    (d - (dims.length() - BIG_OBJECT_SLACK).max(0.0)).max(0.0)
}

/// The closest point of a box to `p`.
pub(crate) fn box_distance(min: Vec3, max: Vec3, p: Vec3) -> f32 {
    p.clamp(min, max).distance(p)
}

/// One blast's numbers (9): the damage at the centre and the radius (world
/// units), its damage type, whether a player takes it twice (Quirk 3), the
/// attacker and the weapon.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Area {
    pub damage: f32,
    pub radius: f32,
    pub damage_type: i32,
    pub twice: bool,
    pub thrower: Option<Entity>,
    pub weapon: &'static str,
    pub unit: f32,
    pub hp: f32,
}

impl Area {
    /// The damage at distance `d` (linear falloff, `falloff`).
    pub(crate) fn at(&self, d: f32) -> f32 {
        self.damage * falloff(self.radius, d)
    }
}

/// The blast `a` at `at` (m) on the players it reaches: each living
/// character whose box is within the radius, `amount(body, d)` the damage
/// there (distance to the centre, world units), cover traces both ways
/// with a second try to the head top (not for `direct`, the character a
/// projectile struck), the armour step, the team rules; twice when `a`
/// says so (the player object and its hit-box object, Quirk 3).
pub(crate) fn blast_bodies(
    f: &mut Frame,
    at: Vec3,
    a: &Area,
    direct: Option<Entity>,
    amount: &dyn Fn(&Body, f32) -> f32,
    world: &GrenadeWorld,
    out: &mut Out,
) {
    let (unit, hp) = (a.unit, a.hp);
    for b in world.bodies() {
        if box_distance(b.min, b.max, at) > a.radius * unit {
            continue;
        }
        // The big-object slack applies to characters too, by their dims
        // (Corrections 2: nothing for a stock body).
        let d = blast_distance(b.centre.distance(at) / unit, (b.max - b.min) / 2.0 / unit);
        if d > a.radius {
            continue;
        }
        if direct != Some(b.entity) && !world.clear(at, b.centre) && !world.clear(at, b.centre.with_y(b.head_top)) {
            continue;
        }
        let n = amount(&b, d);
        if n <= 0.0 {
            continue;
        }
        // Explosion damage through the armour step (no stock gear lists
        // it: whole, armour points untouched).
        for _ in 0..(if a.twice { 2 } else { 1 }) {
            let damage = out.armoured(b.entity, a.thrower, n * hp, a.damage_type, hp);
            out.deal(f, b.entity, b.health, damage, DamageKind::Blast, a.thrower, a.weapon, b.centre, hp);
        }
    }
}

/// The blast `a` at `at` (m) on breakables and special weapons' objects:
/// no cover test (an object's distance has the excess of its dims over
/// `BIG_OBJECT_SLACK` taken off).
pub(crate) fn blast_things(at: Vec3, a: &Area, world: &GrenadeWorld, out: &mut Out) {
    let (unit, hp, r) = (a.unit, a.hp, a.radius);
    let mut one = |e: Entity, min: Vec3, max: Vec3| {
        if box_distance(min, max, at) > r * unit {
            return;
        }
        let centre = (min + max) / 2.0;
        let dims = (max - min) / 2.0 / unit;
        let slack = (dims.length() - BIG_OBJECT_SLACK).max(0.0);
        let d = (centre.distance(at) / unit - slack).max(0.0);
        if d > r {
            return;
        }
        out.damage.write(Damage {
            target: e,
            attacker: a.thrower,
            amount: a.at(d) * hp,
            point: centre,
            dir: (centre - at).normalize_or_zero(),
            hitgroup: Hitgroup::Generic,
            kind: DamageKind::Blast,
            weapon: Some(a.weapon),
            force: Vec3::ZERO,
        });
    };
    for (e, aabb) in &world.breakables {
        one(e, aabb.min, aabb.max);
    }
    for (e, t) in &world.targets {
        one(e, t.centre - t.half, t.centre + t.half);
    }
}

/// A frag's blast (9) at `at` (m).
#[allow(clippy::too_many_arguments)]
fn blast(
    f: &mut Frame,
    me: Entity,
    g: &CaGrenade,
    at: Vec3,
    grenades: &mut Grenades,
    world: &GrenadeWorld,
    out: &mut Out,
) {
    let p = &g.params;
    let r = p.area_radius;
    let area = Area {
        damage: p.area_damage,
        radius: r,
        damage_type: EXPLOSION,
        twice: true,
        thrower: g.thrower,
        weapon: g.weapon,
        unit: g.unit,
        hp: g.hp,
    };
    // Players and their hitboxes: the same blast twice (Quirk 3).
    blast_bodies(f, at, &area, None, &|_, d| area.at(d), world, out);
    // Other live grenades (13).
    let others: Vec<Entity> = grenades
        .iter()
        .filter(|(e, ..)| *e != me && !f.gone.contains(e))
        .map(|(e, ..)| e)
        .collect();
    for o in others {
        let Ok((_, mut other, ..)) = grenades.get_mut(o) else { continue };
        let pos = other.pos * other.unit;
        let d = pos.distance(at) / g.unit;
        if d > r || !world.clear(at, pos) {
            continue;
        }
        other.health -= area.at(d);
        if other.health <= DESTROYED_AT {
            f.queue.push(o);
        }
    }
    // Breakables and special weapons' objects: no cover test.
    blast_things(at, &area, world, out);
}

/// A flash test (10) at `at` (m): each player in sight gets a timer.
fn flash(f: &mut Frame, g: &CaGrenade, at: Vec3, world: &GrenadeWorld, out: &mut Out) {
    let p = &g.params;
    let unit = g.unit;
    let r = p.area_radius;
    for b in world.bodies() {
        if box_distance(b.min, b.max, at) > r * unit {
            continue;
        }
        let d = b.centre.distance(at) / unit;
        if d > r {
            continue;
        }
        if !world.clear(at, b.centre) && !world.clear(at, b.centre.with_y(b.head_top)) {
            continue;
        }
        if out.spared(b.entity, g.thrower) {
            continue;
        }
        // The gear's torso flash entries summed, with any other protection.
        let gear = out.armour.get(b.entity).ok().and_then(|(_, p)| p).map_or(0.0, |p| gear_torso(&p.pieces, FLASH));
        let protection = (gear + b.flash_protection).clamp(0.0, 1.0);
        let amount = p.area_damage * falloff(r, d) * (1.0 - protection);
        if amount <= FLASH_MIN_AMOUNT {
            continue;
        }
        let facing = b.facing.dot(b.centre - at) < 0.0;
        let (delay, time) = if facing {
            (FLASH_DELAY, amount)
        } else {
            (FLASH_DELAY_BACK, amount * FLASH_BACK_FACTOR)
        };
        let timer = FlashTimer {
            victim: b.entity,
            due: f.time + delay,
            time,
        };
        let list = &mut out.effects.flashes;
        match list.iter_mut().find(|t| t.victim == b.entity) {
            Some(t) => *t = timer,
            None => list.push(timer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_surface_table_parses() {
        let t = CaGrenadeSurfaces::built_in();
        assert_eq!(t.hardness(Some("Desert Sand")), 0.1);
        assert_eq!(t.hardness(Some("chain-link fence")), 0.5);
        assert_eq!(t.hardness(Some("concrete")), 0.65);
        assert_eq!(t.hardness(None), 0.65);
        assert_eq!(t.hardness(Some("sky")), 0.0);
    }

    #[test]
    fn falloff_is_full_to_a_quarter() {
        assert_eq!(falloff(450.0, 100.0), 1.0);
        assert!((falloff(450.0, 300.0) - 0.4444).abs() < 1e-4);
        assert_eq!(falloff(450.0, 450.0), 0.0);
        assert_eq!(falloff(450.0, 460.0), 0.0);
    }

    #[test]
    fn the_flash_curve_is_a_half_cosine() {
        assert_eq!(flash_white(0.0, 25.0), 1.0);
        assert!((flash_white(6.25, 25.0) - 0.7071).abs() < 1e-4);
        assert!((flash_white(8.333, 25.0) - 0.5).abs() < 1e-3);
        assert_eq!(flash_white(12.5, 25.0), 0.0);
    }
}
