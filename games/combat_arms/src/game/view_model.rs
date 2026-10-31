//! Combat Arms' first-person view models (specs/combat_arms/
//! view_models.md; section numbers below are its).
//!
//! Loading (`on_demand`: a weapon's when it is first wanted, in the
//! background, once per session; `add_view_models`: every one of ours
//! into a map, for tests): each weapon whose
//! data names a player-view model (`[models] view`, an archive path of the
//! install, with `view_skins`: its skins by texture index, 2, and
//! `view_offset`: its `Pos`, 4; its pieces drawn in the install
//! weapon's `PVRenderStyle#`, `render_style`) gets it as Chimera's
//! `MapViewModel`,
//! converted by `model` (the gun's pieces on the arm skeleton, the
//! `ANI_*` child's animations appended with their bindings' root
//! translation, 1), scaled by `VIEW_SCALE`, its origin at `Pos` (the
//! frame after `model`'s conversion is the camera's, -Z ahead: rotation
//! identity in camera space, 4). Each animation's key strings become its
//! sequence's events (cycle = key time / length), the sockets (`Flash`,
//! `Breach`; names without case) its attachments. The hand model
//! (`hands`: the character's from data/hands.toml, `CHARS_M_HAND`, V2-keyed)
//! is attached to every view model when it reads: its bones matched to
//! the weapon's nodes by name, its sleeve the character's (2). Drawn with
//! its own projection (`settings`: 50 degrees vertical, 5.1).
//!
//! Playing (`drive`, each rendered frame in `map::DriveAnimation`; the
//! character carries `OwnViewAnim`, so Chimera's generic driver leaves it
//! alone; with another game's gun in hand, a mashup's, it lets go of the
//! character and Chimera's driver plays that gun's view model): per
//! character, a fresh `machine::ViewMachine` at each draw (7:
//! `select`, `idle_0` and the full idle `idle_1` after 5-15 s still,
//! `prefire` but for pistols, knives and shotguns, `fire_0` /
//! `alt_fire_0`, `postfire` and the semi-automatic cancel, `reload` or
//! `PreReload`/`reload`/`PostReload`, the sprint pose
//! `PreRush`/`rush`/`PostRush`), its rates the weapon's fire, reload and
//! draw rates with the fitted attachments' and the item's bonuses
//! (`attachments::animation_rate`), fed the owner's fire, alt-fire,
//! reload start (the weapon's `ReloadStarted`), clip and total (a
//! throwable's clip empty from its `Thrown` to the end of its 400 ms
//! re-ready `reload`), sprint and movement. The pose drawn is the machine's after the weapon update and
//! before the frame's advance (Per-frame order; Quirk 1's one-frame pop on
//! non-reset switches included), written onto the `ViewAnimator`; the
//! advance's key strings go out as `CaViewKey` messages (8: `sound_key n`,
//! `THROW_KEY`, the fire keys, `SHELL_CASING`, entering pre- and
//! post-fire). Hidden (11) from the frame zoom leaves level 0 to the
//! frame the zoom-out ends, and while dead.
//!
//! Fitted parts (attachments.md "View model and world model"): a weapon
//! whose parts have first-person models shows a view model with them on
//! it (`parts`), made on demand; the machine plays on when it arrives.
//!
//! Placement (4, 6): `sway::Sway` steps first (the run flag, the moving
//! flag, speed per frame, footing; `CaSwayOption` is the profile's
//! `WeaponSway`), then the model's origin is `Pos` + (w, h, 0) + the shot
//! jiggle j (1, 1, 1) from the weapon's `Recoil` (`CaViewParams`, zero in
//! stock data), as the character's `ViewModelOffset`. The flash offset o
//! (`Pos` + `MuzzlePos` + sway + the draws per axis), the world flash
//! point F and breach point B (`BreachOffset`) are kept on `CaViewModel`;
//! a shot (the weapon's `Fired`) starts the muzzle flash timer (9: shown
//! while the clock is before the shot + the effect's length, the model
//! visible, the player alive and the camera out of liquid volumes unless
//! `UseUWMuzzleFX`); the shot's casing leaves from B
//! (`effects`: the ammo's FireFX says whether it ejects; 10's start
//! velocities and effects_sounds.md 8's motion). The front end draws the
//! flash (`ui::effects`: the muzzle part's or the weapon's
//! `PVMuzzleFXName` group at the `Flash` socket + (0, 0, 0.5)).
//!
//! Readable without keys: the player-view archives of pistols, shotguns
//! and machine guns (`GUNS_M_PV_Pistol`, `_SG`, `_MG`); the rifles',
//! SMGs', snipers', knives', grenades' and the hand models' need the V2
//! key, and a weapon whose view model doesn't load keeps the front end's
//! placeholder box (the map's warnings say which). Our data names the
//! M92FS's, 870's and M60's; with the V1 key every weapon of ours takes
//! the install's `PVModelNormal`, skins and `Pos` (`data`), so with both
//! keys all 13 draw their own models with the hands, textured
//! (`heavy::view_model::keyed_view_models_with_hands`).
//!
//! The 2026-10-22 corrections (view_models.md Corrections 1-6): a press
//! during `postfire` refires on the update that sees it done and a
//! semi-automatic cancel starts the whole sequence (`prefire` first); a
//! sprint cuts a knife swing or double-barrel shot at once (`PreRush`,
//! nothing hit after the cut), drops a grenade request that has not
//! reached its pin pull, and the full idle after `PostRush` starts from 0;
//! the fire rate is 1 + the weapon's `FireAniRate` (`CaItem::fire_rate`,
//! `attachments::fire_animation_rate`, also the shot cycle) + the parts',
//! the draw and reload add the vest's `SelectAniRate` / `ReloadAniRate`
//! (`gear` vests; the picture only: the weapon's own reload and draw
//! times don't add them yet).
//!
//! The follow-up answers (view_models.md Open questions 9-16): the
//! machine plays `reload_1` for the double barrel with rounds left, a
//! file's `deselect` on a holster (`ViewMachine::holster`; no stock file
//! has one), keeps the pieces (`Pieces`: the `HiddenPiece` list by the
//! total, the `ProjectilePiece` by shots, reloads and the draw) and the
//! attached effects (`PVAttachFX`: hidden at creation, shown with the
//! model, `HIDE_PVATTACHFX` / `SHOW_PVATTACHFX` keys; the weapon's
//! `PVAttachFXName#` groups played and drawn by `attached`); dry fire never plays
//! and is silent (C6); a flash hider's part takes the weapon's
//! `PVFlashHiderFXName`h (`effects::pv_muzzle`; none: no flash); the
//! sway's footing is the 150 u ray (`sway::can_step`) and a spectated
//! (another player's) gun sways by its keys alone; the world's horizontal
//! FOV at a camera reset is `place::WorldFov`.
//!
//! Known shortcuts (docs/tech-debt.md):
//! - The machine drives the picture only: in play the shots, hits, ammo
//!   and reload times stay the weapon frame's (`weapons`, `inventory`,
//!   `grenades`). Open question 9 (answered: the view model's keys are the
//!   client's only clock) stands against it: the weapon frame's cycle is
//!   the fire animation's length (`trigger.cycle`, 105 ms for the M4A1),
//!   the high-frame-rate limit of the frame-rounded cadence (116.7 ms at
//!   60 fps, 110 ms at 100 fps, case 33: the machine's), and the knife's
//!   hits keep inventory.md's timings (its 1 ms pre-fire; C1 says none).
//!   The machine's shots and reloads are checked against the spec's
//!   timelines in the scenario tests.
//! - The tracker counts exact frame time (the spec's test arithmetic), not
//!   whole milliseconds of a millisecond clock; the pose between samples
//!   is `model`'s resampling (`VIEW_SAMPLE_FPS`, nlerp between samples).
//! - The pieces' states aren't drawn (no piece hiding on the view model
//!   yet; the attached effects are, `attached`); no player-view
//!   attachments (no character record has any); the hands' file-name
//!   reset at the local character's creation isn't there (every draw
//!   builds fresh hands); the weapons' `PVFlashHiderFXName#` aren't read
//!   from the install (no stock weapon has them).
//! - Effects: key sounds and casings are `effects`' (CaViewKey).
//! - Projection (5.1): Chimera's view-model camera keeps its 1 cm near
//!   plane (`VIEW_SCALE` makes that the spec's 0.1 u) and no far plane
//!   (the spec's 7 u); `ViewModelSettings` would zoom the view model with
//!   the world's zoom, but the model is hidden whenever zoomed. The world
//!   camera's FOV is the shell's `PlayerFov`, which the Combat Arms app
//!   sets to `DEFAULT_FOV` (5.2: 75 degrees vertical, 91.31 at 4:3), kept
//!   vertical on every screen shape and recomputed every frame rather than
//!   at a world start only (the party apps keep the shell's 90).
//! - No left-handed mirroring (none in the game); the character's model
//!   id, face, uniform and colour are `hands::HandProfile`'s defaults
//!   (no character data yet); the hands' render styles aren't read
//!   (their default style).

pub mod attached;
pub mod hands;
pub mod machine;
pub mod on_demand;
pub mod parts;
pub mod place;
pub mod sway;

use std::{collections::HashMap, sync::Arc};

use bevy::prelude::*;
use serde::Deserialize;

use self::{
    machine::{AnimInfo, Class, Inputs, Rates, Setup, ViewKey, ViewMachine},
    place::{CameraAxes, FlashTimer},
    sway::{Sway, SwayInputs},
};
use super::{
    attachments::{CaMods, animation_rate, fire_animation_rate},
    inventory::{CaItem, UNIT},
    model,
    volumes::{CaVolume, in_liquid},
    weapons::CaZoom,
};
use crate::{
    core::{Health, Intent, LocalPlayer, MovementState, Velocity},
    map::{MapAttachment, MapData, MapViewModel, ViewAnimator, ViewModelOffset, ViewModelSettings, ViewModels, anim::AnimEvent},
    weapon::{
        Hitscan, Inventory, Magazine, ShellReload, Trigger, Weapon, WeaponEvent, WeaponEventKind, aim_of,
        data::WeaponDataAppExt,
        grenade::Throwable,
        view_anim::{OwnViewAnim, ViewAnimPlugin},
    },
};

/// The view model's horizontal field of view at 4:3 (degrees), from 5.1's
/// 50 degrees vertical (`PVModelFOV`): 2 atan(4/3 tan 25 degrees).
pub const VIEWMODEL_FOV: f32 = 63.74;
/// The player's unzoomed horizontal field of view at 4:3 (5.2: 75 degrees
/// vertical, 2 atan(4/3 tan 37.5 degrees)).
pub const DEFAULT_FOV: f32 = 91.31;

/// Metres per view-model unit: 0.1, so that Chimera's view-model near
/// plane (1 cm) cuts where the game's (`reallyclose_near`, 0.1 u, 5.1)
/// does. The view model has its own camera, so the scale changes nothing
/// else on screen.
pub const VIEW_SCALE: f32 = 0.1;

/// The weapon data's hook: `combat_arms:view_model` (`CaViewParams`).
pub const VIEW: &str = "combat_arms:view_model";

/// The colour of a piece whose skin doesn't read (keyed texture
/// archives without the V2 key): a gunmetal grey, so the shape reads.
const UNSKINNED: [u8; 3] = [90, 92, 96];

/// `combat_arms:view_model`: the weapon's placement data (camera axes,
/// units) and first-person muzzle effect.
#[derive(Component, Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct CaViewParams {
    /// `MuzzlePos`: the flash offset from `Pos`.
    pub muzzle_pos: [f32; 3],
    /// `BreachOffset`: the shells' start from the flash point.
    pub breach_offset: [f32; 3],
    /// `Recoil`: the shot jiggle's ranges (zero in every stock record).
    pub recoil: [f32; 3],
    /// `PVMuzzleFXName` (empty: no first-person flash).
    pub muzzle_fx: String,
    /// The effect's length (s).
    pub flash_time: f32,
    /// `UseUWMuzzleFX`: the flash shows under water too.
    pub under_water_flash: bool,
    /// The view model throws casings (kept as data; which shots eject is
    /// the ammo's FireFX, `effects`).
    pub shells: bool,
    /// `PVFlashHiderFXName`h by flash-hider level h (9: a part with
    /// `FlashHiderLevel` h takes it; a missing one: no flash). No stock
    /// weapon has any.
    pub flash_hider_fx: Vec<String>,
    /// `PVAttachFXName0..`: the attached effect groups (3; `attached`),
    /// and `PVAttachFXAlwaysShow`.
    pub attach_fx: Vec<String>,
    pub attach_fx_always: bool,
}

impl Default for CaViewParams {
    fn default() -> Self {
        Self {
            muzzle_pos: [0.0; 3],
            breach_offset: [0.0; 3],
            recoil: [0.0; 3],
            muzzle_fx: String::new(),
            flash_time: place::FLASH_TIME as f32,
            under_water_flash: false,
            shells: false,
            flash_hider_fx: Vec::new(),
            attach_fx: Vec::new(),
            attach_fx_always: false,
        }
    }
}

/// The profile's `WeaponSway` option (0 to 10; the sway's scale is it /
/// 10). The measured Steam profile had 10.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct CaSwayOption(pub u8);

impl Default for CaSwayOption {
    fn default() -> Self {
        Self(10)
    }
}

/// A key event of a character's view model (8), or entering pre-/post-fire.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct CaViewKey {
    pub owner: Entity,
    pub key: ViewKey,
}

/// A character's view model as Combat Arms runs it: the state machine of
/// the weapon drawn, the sway, the shot jiggle, the muzzle flash and the
/// points effects start from.
#[derive(Component, Clone, Debug, Default)]
pub struct CaViewModel {
    /// The weapon drawn and its machine (None: no view model for it).
    pub weapon: Option<Entity>,
    pub machine: Option<ViewMachine>,
    pub sway: Sway,
    /// This frame's jiggle (sum on all axes, Quirk 2) and its draws.
    pub jiggle: f32,
    pub draws: Vec3,
    /// The model's camera-space origin (units, the game's axes).
    pub origin: Vec3,
    /// The flash offset o (units, camera axes), the world flash point F and
    /// breach point B (metres).
    pub flash_offset: Vec3,
    pub flash_point: Vec3,
    pub breach_point: Vec3,
    pub flash: FlashTimer,
    /// While shown: the flash's place in the view-model camera's space
    /// (metres, Bevy axes): the `Flash` socket + (0, 0, 0.5) socket units.
    pub flash_at: Option<Vec3>,
    /// Hidden behind a scope (11).
    pub hidden: bool,
    /// A throwable's grenade left the hand (`Thrown`): its clip is 0 to
    /// the machine until its re-ready (`reload`) ends (Grenade timings Q5).
    pub thrown: bool,
    rng: u32,
}

impl CaViewModel {
    fn roll(&mut self) -> f32 {
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// The view-model settings Combat Arms starts with.
pub fn settings() -> ViewModelSettings {
    ViewModelSettings {
        fov: VIEWMODEL_FOV,
        default_fov: DEFAULT_FOV,
        right_hand: 1,
        draw: 1,
    }
}

/// `ca_viewmodel_fov`: raise it for a smaller gun (saved).
pub fn register_fov_cvar(app: &mut App) {
    crate::weapon::view_anim::fov_cvar(
        app,
        "ca_viewmodel_fov",
        "Combat Arms' view model field of view (horizontal degrees at 4:3, 63.74 to 110; zooms with the view).",
        super::GAME,
        (VIEWMODEL_FOV, 110.0),
    );
}

/// Combat Arms' view models: the data hook (before the weapons load), the
/// machine, sway and effects, on Chimera's view-model types.
pub struct CaViewModelPlugin;

impl Plugin for CaViewModelPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<ViewAnimPlugin>() {
            app.add_plugins(ViewAnimPlugin);
        }
        app.register_weapon_hook::<CaViewParams>(VIEW, |e, _, p| {
            e.insert(p.clone());
        })
        .insert_resource(settings())
        .init_resource::<CaSwayOption>()
        .init_resource::<hands::HandProfile>()
        .add_message::<CaViewKey>()
        .add_systems(
            Update,
            (
                on_demand::load_on_demand.run_if(resource_exists::<on_demand::LoadOnDemand>),
                drive
                    .in_set(crate::map::DriveAnimation)
                    .before(crate::weapon::view_anim::drive),
                attached::track.in_set(crate::map::DriveAnimation).after(drive),
            ),
        );
        register_fov_cvar(app);
    }
}

/// Whether weapon `id` is another game's (`cs_source:weapon_glock`): one
/// whose game prefix names another registered game. An id without a
/// prefix, or with an unregistered one (a test's), is ours.
fn is_foreign(id: &str, games: Option<&chimera_core::game_registry::Games>) -> bool {
    id.split_once(':')
        .is_some_and(|(game, _)| game != super::GAME && games.is_some_and(|g| g.get(game).is_some()))
}

/// The animations of a view model as the machine sees them: length (ms)
/// and keys from each sequence's events.
pub fn anim_infos(set: &crate::map::anim::AnimSet) -> Vec<AnimInfo> {
    set.sequences
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let length = (set.duration(i) * 1000.0).round() as u32;
            let mut keys: Vec<(u32, String)> = s
                .events
                .iter()
                .map(|e| ((e.cycle * length as f32).round() as u32, e.name.clone()))
                .collect();
            // Stand-ins without keys: one at each end.
            if keys.len() <= 1 && length > 0 {
                let strings: Vec<(u32, &str)> = keys.iter().map(|(t, s)| (*t, s.as_str())).collect();
                keys = AnimInfo::new(&s.name, length, &strings).keys;
            }
            AnimInfo {
                name: s.name.clone(),
                length,
                keys,
            }
        })
        .collect()
}

/// View model `path` for weapon `key`, skinned with `skins` (textures by
/// the model's texture index), its origin at `offset` (the game's camera
/// space: units, +Z ahead), its textures added to `map`.
pub fn load(
    path: &str,
    key: &str,
    skins: &[String],
    offset: [f32; 3],
    map: &mut MapData,
    file: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<MapViewModel, String> {
    let loaded = model::load(path, file)?;
    let mut m = model::convert_at(&loaded, VIEW_SCALE, &skin_of(skins), model::VIEW_SAMPLE_FPS);
    // Its pieces' render styles: the install weapon's `PVRenderStyle#`
    // (view_models.md 2; none without it: the default style).
    let ca = super::data::CaData::install();
    let styles = ca.install_weapon(key).map_or(&[][..], |r| &r.pv_render_styles[..]);
    for (mesh, lod) in m.meshes.iter_mut().zip(model::drawn_lods(&loaded.model)) {
        let style = super::render_style::of_list(styles, lod.render_style);
        super::render_style::apply_with(mesh, style, skins, model::second_slot(lod));
    }
    super::world::model_textures(&mut m.meshes, map, file);
    for mesh in m.meshes.iter_mut().filter(|m| m.texture.is_none()) {
        mesh.color = UNSKINNED;
    }
    // Just built: nothing else holds the rig or its animations.
    let rig = m
        .rig
        .take()
        .and_then(|r| Arc::try_unwrap(r).ok())
        .ok_or_else(|| format!("{path}: no skeleton"))?;
    let mut animations = Arc::try_unwrap(rig.animations).map_err(|_| format!("{path}: animations shared"))?;
    // The origin moves to `offset`: the skeleton's root and the meshes
    // (in the eye's space at the reference pose) alike.
    let offset = Vec3::from(lithtech_formats::ltb::to_rh_vec(offset)) * VIEW_SCALE;
    for p in m.meshes.iter_mut().flat_map(|m| m.positions.iter_mut()) {
        *p = (Vec3::from(*p) + offset).to_array();
    }
    // Played by the machine: none loops on its own (`rush` loops there).
    for (s, entry) in animations.sequences.iter_mut().zip(&loaded.list.entries) {
        s.looping = false;
        let length = entry.length().max(1) as f32;
        s.events = entry
            .animation
            .keys
            .iter()
            .map(|k| AnimEvent {
                cycle: k.time as f32 / length,
                name: k.string.clone(),
                ..default()
            })
            .collect();
    }
    let attachments = loaded
        .model
        .sockets
        .iter()
        .map(|s| {
            let q = Quat::from_array(lithtech_formats::ltb::to_rh_quat(s.rotation));
            MapAttachment {
                name: s.name.clone(),
                bone: s.node as usize,
                local: Transform::from_translation(Vec3::from(lithtech_formats::ltb::to_rh_vec(s.position)))
                    .with_rotation(if q.length_squared() > 0.0 { q.normalize() } else { Quat::IDENTITY }),
            }
        })
        .collect();
    Ok(MapViewModel {
        key: key.to_string(),
        model: m,
        bones: rig.bones,
        root: rig.root.with_translation(offset),
        animations: Some(Arc::new(animations)),
        right_handed: true,
        allow_flipping: false,
        attachments,
        light_origin: Vec3::ZERO,
    })
}

/// A piece's skin from a skin list by its first texture index.
fn skin_of(skins: &[String]) -> impl Fn(&lithtech_formats::ltb::Lod) -> String + '_ {
    move |lod| {
        let i = lod.used_textures().first().copied().unwrap_or(-1);
        usize::try_from(i)
            .ok()
            .and_then(|i| skins.get(i))
            .map(|s| model::normalize(s))
            .unwrap_or_default()
    }
}

/// A fitted part's model `path` (`parts`), skinned with `skins` by
/// texture index, converted (meshes in metres, in the pose of its first
/// animation at t = 0: a part is left on its default animation,
/// attachments.md Corrections (2026-10-30) 5), its textures added to
/// `map`; with each mesh's render-style index.
pub fn load_part(
    path: &str,
    skins: &[String],
    map: &mut MapData,
    file: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<(Vec<crate::map::MapMesh>, Vec<i32>), String> {
    let loaded = model::load_own(path, file)?;
    let mut m = model::convert(&loaded, VIEW_SCALE, &skin_of(skins));
    if let Some(entry) = loaded.list.entries.first() {
        model::pose_meshes(&loaded.model, entry, &mut m.meshes, VIEW_SCALE);
    }
    let styles = model::drawn_lods(&loaded.model).iter().map(|l| l.render_style).collect();
    super::world::model_textures(&mut m.meshes, map, file);
    for mesh in m.meshes.iter_mut().filter(|m| m.texture.is_none()) {
        mesh.color = UNSKINNED;
    }
    Ok((m.meshes, styles))
}

/// The hand model of `choice`, converted (meshes in metres at its bind
/// pose, its bones), its textures added to `map`.
pub fn load_hands(
    choice: &hands::HandChoice,
    map: &mut MapData,
    file: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<(Vec<crate::map::MapMesh>, Vec<crate::map::MapBone>), String> {
    let loaded = model::load(&choice.model, file)?;
    let mut m = model::convert(&loaded, VIEW_SCALE, &skin_of(&choice.skins));
    super::world::model_textures(&mut m.meshes, map, file);
    for mesh in m.meshes.iter_mut().filter(|m| m.texture.is_none()) {
        mesh.color = UNSKINNED;
    }
    let bones = model::bones(&loaded.model);
    Ok((m.meshes, bones))
}

/// Every Combat Arms weapon's view model that loads, into `map` (a
/// warning for each that doesn't), with the default character's hands
/// when they read.
pub fn add_view_models(map: &mut MapData, data: &super::data::CaDataInner, file: &dyn Fn(&str) -> Option<Vec<u8>>) {
    let choice = hands::choose(&hands::builtin(), &hands::HandProfile {
        character: true,
        ..default()
    });
    let hand = match choice.as_ref().map(|c| load_hands(c, map, file)) {
        Some(Ok(h)) => Some(h),
        Some(Err(e)) => {
            map.warnings.push(format!("view-model hands: {e}"));
            None
        }
        None => None,
    };
    let ours = data.builtin_weapon_ids();
    for w in data.weapon_set().weapons.iter().filter(|w| ours.contains(&w.id)) {
        let Some(path) = w.models.view.as_deref() else { continue };
        let offset = w.models.view_offset.unwrap_or_default();
        match load(path, &w.id, &w.models.view_skins, offset, map, file) {
            Ok(mut v) => {
                if let Some((meshes, bones)) = &hand {
                    let at = Vec3::from(lithtech_formats::ltb::to_rh_vec(offset)) * VIEW_SCALE;
                    if let Err(e) = hands::attach(&mut v, meshes.clone(), bones, VIEW_SCALE, at) {
                        map.warnings.push(format!("{} view-model hands: {e}", w.id));
                    }
                }
                map.kit.view_models.push(v)
            }
            Err(e) => map.warnings.push(format!("{} view model: {e}", w.id)),
        }
    }
}

/// The weapon's class for the machine (inventory.md's by data slot;
/// primaries with pellets are shotguns).
fn class_of(w: &Weapon, pellets: u32) -> Class {
    match w.slot {
        1 => Class::Secondary,
        2 => Class::Melee,
        3 => Class::Throw,
        4 => Class::Special,
        _ if pellets > 1 => Class::Shotgun,
        _ => Class::Primary,
    }
}

/// The game's camera-space vector (left-handed, units) in the view-model
/// camera's (Bevy axes, metres).
pub fn to_view_space(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, -v.z) * VIEW_SCALE
}

type WeaponParts<'a> = (
    &'a Weapon,
    Option<&'a Magazine>,
    Option<&'a Trigger>,
    Option<&'a ShellReload>,
    Option<&'a Throwable>,
    (
        Option<&'a CaZoom>,
        Option<&'a CaViewParams>,
        Option<&'a CaMods>,
        Option<&'a CaItem>,
        Option<&'a Hitscan>,
        Option<&'a super::gear::CaGearRates>,
    ),
);

/// The machine's setup for a weapon.
/// `vest`: the character model's reload and select bonuses, for a weapon
/// without the gear's rates of its own (`gear::CaGearRates`: the vest's
/// and the sets', the ones its reload and draw times use).
fn setup_of(parts: &WeaponParts, vest: (f32, f32)) -> Setup {
    let (w, magazine, trigger, shells, throwable, (_, params, mods, item, hitscan, gear)) = parts;
    let (fire, reload, select) = mods.map_or((0.0, 0.0, 0.0), |m| m.rates());
    let (gear_fire, model_reload, model_select) = gear.map_or((0.0, vest.0, vest.1), |g| (g.fire, g.reload, g.select));
    let fire = fire + gear_fire;
    Setup {
        class: class_of(w, hitscan.map_or(1, |h| h.pellets)),
        per_press: trigger.map_or(0, |t| if t.automatic { t.per_press } else { t.per_press.max(1) }),
        shells: shells.is_some(),
        clip_size: magazine.map_or(if throwable.is_some() { 1 } else { 0 }, |m| m.size),
        rates: Rates {
            fire: fire_animation_rate(item.map_or(0.0, |i| i.fire_rate), fire),
            reload: animation_rate(1.0 + model_reload + reload),
            draw: animation_rate(1.0 + item.map_or(0.0, |i| i.select_rate) + model_select + select),
        },
        attach_fx: params.map_or(0, |p| p.attach_fx.len()),
        attach_fx_always: params.is_some_and(|p| p.attach_fx_always),
        ..Setup::default()
    }
}

/// Clip and total ammo as the machine sees them (no magazine: always
/// one; a throwable: one in hand of its count, none from its throw to its
/// re-ready, `thrown`).
fn ammo_of(parts: &WeaponParts, thrown: bool) -> (u32, u32) {
    let (_, magazine, _, _, throwable, _) = parts;
    match (magazine, throwable) {
        (Some(m), _) => (m.clip, m.clip + m.reserve),
        (None, Some(t)) => (if thrown { 0 } else { t.count.min(1) }, t.count),
        _ => (1, 1),
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn drive(
    time: Res<Time>,
    models: Option<Res<ViewModels>>,
    sway_option: Res<CaSwayOption>,
    mut characters: Query<(
        Entity,
        &Inventory,
        &Intent,
        (
            Option<&MovementState>,
            Option<&Health>,
            Option<&Velocity>,
            Option<&Transform>,
            Option<&super::movement::CaMovement>,
        ),
        (
            Option<&mut ViewAnimator>,
            Option<&mut CaViewModel>,
            Option<&mut ViewModelOffset>,
            Has<OwnViewAnim>,
            Has<LocalPlayer>,
            Option<&super::characters::records::Selection>,
            Has<super::characters::CaRungLook>,
        ),
    )>,
    gear: Option<Res<super::gear::CaGearData>>,
    weapons: Query<WeaponParts>,
    states: Query<&chimera_sim::weapon::WeaponState>,
    mut events: MessageReader<WeaponEvent>,
    mut keys: MessageWriter<CaViewKey>,
    mut tables: Local<HashMap<String, Arc<Vec<AnimInfo>>>>,
    mut commands: Commands,
    volumes: Query<&CaVolume>,
    spatial: avian3d::prelude::SpatialQuery,
    games: Option<Res<chimera_core::game_registry::Games>>,
) {
    let (dt, now) = (time.delta_secs_f64(), time.elapsed_secs_f64());
    let events: Vec<&WeaponEvent> = events.read().filter(|e| e.shown()).collect();
    if models.as_ref().is_some_and(|m| m.is_changed()) {
        tables.clear();
    }
    for (e, inventory, intent, (state, health, velocity, transform, movement), (view, vm, offset, owned, local, look, rung)) in &mut characters {
        // Another game's gun in hand (a mashup's): its view model is
        // Chimera's generic driver's, with that game's rules
        // (`weapon::view_anim`). Ours lets go of the character until a gun
        // of ours is back.
        if inventory
            .active
            .and_then(|w| weapons.get(w).ok())
            .is_some_and(|p| is_foreign(p.0.id, games.as_deref()))
        {
            if owned {
                commands.entity(e).remove::<OwnViewAnim>();
                if let Some(mut offset) = offset {
                    *offset = ViewModelOffset::default();
                }
            }
            continue;
        }
        let (Some(mut view), Some(mut vm), Some(mut offset), true) = (view, vm, offset, owned) else {
            commands.entity(e).insert((
                ViewAnimator::default(),
                CaViewModel {
                    rng: e.to_bits() as u32 ^ 0x2545_f491,
                    ..default()
                },
                ViewModelOffset::default(),
                OwnViewAnim,
            ));
            continue;
        };
        let alive = health.is_none_or(|h| h.current > 0.0);
        // The character model's reload and select bonuses (Corrections 6),
        // for a weapon without its own `CaGearRates`.
        let model = gear
            .as_deref()
            .map_or((0.0, 0.0), |g| g.0.vest_rates(look.map_or(0, |l| super::gear::vest_of(l, rung))));
        let active = inventory.active.filter(|_| alive);
        let parts = active.and_then(|w| weapons.get(w).ok());
        // With drawn parts, the view model with them (`parts::view_key`)
        // once it is made, the bare gun's until then.
        let with_parts = parts.as_ref().map(|p| self::parts::view_key(p.0.id, p.5.2));
        let loaded = |k: &str| models.as_ref().is_some_and(|m| m.get(k).is_some());
        let key = match (&with_parts, parts.as_ref()) {
            (Some(k), _) if loaded(k) => Some(k.as_str()),
            (_, Some(p)) => Some(p.0.id),
            _ => None,
        };
        // Its view model loaded since the weapon was drawn (`on_demand`):
        // drawn afresh.
        if models.as_ref().is_some_and(|m| m.is_changed()) && view.animator.is_none() && key.is_some_and(loaded) {
            view.key = None;
        }
        // The same gun with its parts now made: the machine plays on.
        let same_gun = vm.weapon == active
            && vm.machine.is_some()
            && view.animator.is_some()
            && view.key.as_deref().map(|k| self::parts::split_key(k).0) == key.map(|k| self::parts::split_key(k).0);
        // A new weapon in hand: a fresh model drawing.
        let shown_changed = view.show(key, models.as_deref()) && !same_gun;
        if shown_changed || vm.weapon != active {
            vm.weapon = active;
            vm.machine = None;
            vm.thrown = false;
            vm.flash = FlashTimer::default();
            if let (Some(p), Some(set)) = (parts.as_ref(), view.animator.as_ref().map(|a| a.set.clone())) {
                let table = tables
                    .entry(p.0.id.to_string())
                    .or_insert_with(|| Arc::new(anim_infos(&set)))
                    .clone();
                let seed = vm.roll().to_bits();
                let mut m = ViewMachine::draw(table, setup_of(p, model), seed);
                let (clip, total) = p.1.map_or((0, 0), |m| (m.clip, m.clip + m.reserve));
                m.draw_ammo(clip, total);
                vm.machine = Some(m);
            }
        }
        let Some(parts) = parts else {
            vm.flash_at = None;
            continue;
        };
        let (_, _, _, _, _, (zoom, params, ..)) = parts;
        let params = params.cloned().unwrap_or_default();
        let state = state.cloned().unwrap_or_default();
        // 11: hidden from the zoom-in to the end of the zoom-out.
        vm.hidden = zoom.is_some_and(|z| z.level > 0 || z.transition.is_some());
        view.hidden = vm.hidden;
        let shown = !vm.hidden && alive;
        if let Some(m) = vm.machine.as_mut() {
            m.set_visible(shown);
        }
        // 6: the sway, before the weapon update: the footing test (a
        // ray 150 u down from the hull centre when neither on the ground
        // nor a ladder nor jumped); another player's (spectated) gun by
        // its keys alone.
        let speed = velocity.map_or(0.0, |v| v.0.length() / UNIT) as f64;
        let moving = intent.move_axis != Vec2::ZERO || intent.jump;
        let zoomed = vm.hidden;
        let footing = state.on_ground || state.on_ladder || {
            let jumped = movement.is_some_and(|m| m.in_jump);
            let floor = (!jumped)
                .then(|| transform.map(|t| t.translation))
                .flatten()
                .and_then(|at| {
                    let filter = avian3d::prelude::SpatialQueryFilter::from_mask(crate::core::SOLID_LAYERS)
                        .with_excluded_entities([e]);
                    spatial.cast_ray(at, Dir3::NEG_Y, sway::FOOTING_RAY as f32 * UNIT, true, &filter)
                })
                .map(|hit| hit.distance as f64 / UNIT as f64);
            sway::can_step(false, false, false, jumped, floor)
        };
        vm.sway.step(&SwayInputs {
            dt,
            clock: now,
            active: alive && (footing || !local),
            run: (!intent.walk || state.sprinting) && !state.crouching,
            moving,
            speed,
            spectating: !local,
            weapon: true,
            zoomed,
            scale: sway_option.0.min(10) as f64 / 10.0,
        });
        // 7: the weapon update.
        let mine: Vec<&WeaponEventKind> = events.iter().filter(|ev| ev.owner == e).map(|ev| &ev.kind).collect();
        let reload = mine.iter().any(|k| matches!(k, WeaponEventKind::ReloadStarted));
        let shot = mine.iter().any(|k| matches!(k, WeaponEventKind::Fired { .. }));
        if mine.iter().any(|k| matches!(k, WeaponEventKind::Thrown)) {
            vm.thrown = true;
        }
        // The weapon frame fires before this update: its round counts as
        // still in the clip, and as fire held, for the machine's gate.
        let (clip, total) = ammo_of(&parts, vm.thrown);
        let (clip, total) = if shot { (clip + 1, total + 1) } else { (clip, total) };
        let inputs = Inputs {
            fire: intent.fire || shot && parts.0.slot != 2,
            alt_fire: intent.secondary,
            reload,
            clip,
            total,
            sprinting: state.sprinting,
            moving,
        };
        if let Some(m) = vm.machine.as_mut() {
            m.set_rates(setup_of(&parts, model).rates);
            m.set_weapon_reloading(active.and_then(|w| states.get(w).ok()).is_some_and(|s| s.reloading()));
            let out = m.update(&inputs);
            if out.reloaded {
                vm.thrown = false;
            }
            for k in out.events {
                keys.write(CaViewKey { owner: e, key: k });
            }
        }
        // A shot: the jiggle's draws and the flash timer.
        let recoil = Vec3::from(params.recoil);
        vm.draws = if shot {
            let rolls = [vm.roll(), vm.roll(), vm.roll()];
            place::jiggle_draws(recoil, rolls)
        } else {
            Vec3::ZERO
        };
        vm.jiggle = vm.draws.x + vm.draws.y + vm.draws.z;
        if shot {
            vm.flash.shot(now);
        }
        // 4: the model's camera-space position (`Pos` is in the root).
        let sway = (vm.sway.w as f32, vm.sway.h as f32);
        let jiggle = vm.jiggle;
        vm.origin = place::model_origin(Vec3::ZERO, sway, jiggle);
        offset.translation = to_view_space(vm.origin);
        offset.rotation = Quat::IDENTITY;
        // The drawn pose: after the update, before the advance.
        if let (Some(m), Some(animator)) = (vm.machine.as_ref(), view.animator.as_mut()) {
            if let Some((a, t)) = m.shown() {
                let length = m.anims().get(a).map_or(0, |i| i.length).max(1) as f32;
                animator.main = Some(a);
                animator.cycle = (t as f32 / length).clamp(0.0, 1.0);
                animator.fading.clear();
            }
        }
        // 4, 9, 10: the flash offset, flash and breach points.
        let pos = models
            .as_deref()
            .and_then(|m| m.get(parts.0.id))
            .map_or(Vec3::ZERO, |v| {
                let t = v.root.translation / VIEW_SCALE;
                Vec3::new(t.x, t.y, -t.z)
            });
        vm.flash_offset = place::flash_offset(pos, Vec3::from(params.muzzle_pos), sway, vm.draws);
        let (eye, axes) = camera_of(transform, &state, intent);
        vm.flash_point = place::flash_point(eye, &axes, vm.flash_offset * UNIT);
        vm.breach_point = place::breach_point(vm.flash_point, &axes, Vec3::from(params.breach_offset) * UNIT);
        let visible = !vm.hidden && vm.machine.is_some();
        // The camera in a liquid volume (9).
        let under_water = in_liquid(volumes.iter(), eye);
        let shown = !params.muzzle_fx.is_empty()
            && vm.flash.update(
                now,
                params.flash_time as f64,
                visible,
                alive,
                under_water && !params.under_water_flash,
            );
        vm.flash_at = if shown {
            flash_socket(models.as_deref(), parts.0.id, &view, &offset, now)
        } else {
            None
        };
        // 5.7 / 8: the engine's advance, its keys.
        if let Some(m) = vm.machine.as_mut() {
            for k in m.advance(dt) {
                keys.write(CaViewKey { owner: e, key: k });
            }
        }
    }
}

/// The camera: the eye (metres, world) and its axes in the game's sense
/// (right, up, forward).
fn camera_of(transform: Option<&Transform>, state: &MovementState, intent: &Intent) -> (Vec3, CameraAxes) {
    let eye = transform.map_or(Vec3::ZERO, |t| t.translation) + state.eye_offset;
    let q = aim_of(intent.yaw, intent.pitch);
    (
        eye,
        CameraAxes {
            right: q * Vec3::X,
            up: q * Vec3::Y,
            forward: q * Vec3::NEG_Z,
        },
    )
}

/// The `Flash` socket's place (+ the stock effect's offset) in the
/// view-model camera's space, posed as drawn.
fn flash_socket(
    models: Option<&ViewModels>,
    key: &str,
    view: &ViewAnimator,
    offset: &ViewModelOffset,
    now: f64,
) -> Option<Vec3> {
    let model = models?.get(key)?;
    let i = model.attachments.iter().position(|a| a.name.eq_ignore_ascii_case("flash"))?;
    let pose = view.animator.as_ref()?.pose(now);
    let placed = crate::map::view_model::placement(offset, false);
    let m = crate::map::view_model::attachment_transform(model, &pose, i, &placed)?;
    let fx = place::FLASH_SOCKET_OFFSET;
    Some(m.transform_point3(Vec3::new(fx.x, fx.y, -fx.z)))
}

