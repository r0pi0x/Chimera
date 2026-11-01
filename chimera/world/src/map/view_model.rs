//! First-person view models (any game), what games and the drawing
//! share: games load `MapViewModel`s with the map (or later, on demand:
//! `add_view_model`, its own textures in `LateViewModelTextures`), drive each character's
//! `ViewAnimator` (which sequence plays) and `ViewModelOffset` (bob and
//! sway), and send `ViewModelEvent`s (muzzle flashes, brass);
//! `render::view_model` draws the view model of the character whose
//! camera carries `ViewModelAnchor`, with the placement maths here
//! (`placement`, `reproject`, `attachment_transform`, the fields of view).
//!
//! First-person view models for any game: `MapViewModel`, `ViewModels`,
//! `ViewAnimator`, `ViewModelOffset`, `ViewModelSettings`, `EffectSettings`,
//! `ViewModelEvent`, the anchor and camera markers, `ViewModelScreen`,
//! `DynamicLight`, `HeldMuzzle` and the placement maths.

use std::{collections::HashMap, sync::Arc};

use bevy::prelude::*;

use super::{MapBone, MapModel, MapTexture, anim};

/// Render layer of view models and the light that lights them.
pub const VIEW_MODEL_LAYER: usize = 2;

/// A weapon's view model, keyed like `MapHeldModel` (the weapon ID).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MapViewModel {
    pub key: String,
    /// Meshes in the eye's space (camera axes: -Z forward, Y up; meters),
    /// skinned to `bones` in their reference pose.
    pub model: MapModel,
    pub bones: Vec<MapBone>,
    /// From the skeleton's space (the source game's axes and units) to the
    /// eye's.
    pub root: Transform,
    /// What it can play (None: it stays in the reference pose).
    pub animations: Option<Arc<anim::AnimSet>>,
    /// Built holding the weapon in the right hand (Source weapon scripts'
    /// `BuiltRightHanded`).
    pub right_handed: bool,
    /// May be mirrored to match the player's hand (`AllowFlipping`).
    pub allow_flipping: bool,
    /// Named points on bones (muzzle, ejection port).
    pub attachments: Vec<MapAttachment>,
    /// Where it is lit from, in the skeleton's space (Source: the model's
    /// illumination centre).
    pub light_origin: Vec3,
}

/// A point on a bone: `local` is in the bone's space (the skeleton's axes
/// and units).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MapAttachment {
    pub name: String,
    pub bone: usize,
    pub local: Transform,
}

/// The loaded map's view models.
#[derive(Resource, Clone, Default)]
pub struct ViewModels(pub Arc<Vec<MapViewModel>>);

impl ViewModels {
    pub fn get(&self, key: &str) -> Option<&MapViewModel> {
        self.0.iter().find(|v| v.key == key)
    }
}

/// The textures of the view models a game added after the map spawned
/// (`add_view_model`: loaded on demand), by key: their meshes' texture
/// indices are into these lists, not the map's. Removed with the map.
#[derive(Resource, Clone, Default)]
pub struct LateViewModelTextures(pub HashMap<String, Arc<Vec<MapTexture>>>);

/// Add `model` to the loaded map's view models (replacing one with its
/// key), its meshes' textures from `textures` (indices into that list):
/// a view model loaded after the map spawned. The drawing builds it on
/// its next frame (`render::view_model::build_late`); until then the
/// key draws as if it had none. With no map yet, it waits for one.
pub fn add_view_model(world: &mut World, model: MapViewModel, textures: Arc<Vec<MapTexture>>) {
    world
        .get_resource_or_init::<LateViewModelTextures>()
        .0
        .insert(model.key.clone(), textures);
    match world.get_resource_mut::<ViewModels>() {
        Some(mut models) => {
            let list = Arc::make_mut(&mut models.0);
            list.retain(|v| v.key != model.key);
            list.push(model);
        }
        None => world.insert_resource(ViewModels(Arc::new(vec![model]))),
    }
}

/// How view models are drawn (Source `viewmodel_fov`, `default_fov`,
/// `cl_righthand`, `r_drawviewmodel`). Games set the values and console
/// variables.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ViewModelSettings {
    /// Horizontal field of view at 4:3, degrees.
    pub fov: f32,
    /// The player's unzoomed horizontal field of view at 4:3, degrees; the
    /// view model zooms by as many degrees as the player's view does.
    pub default_fov: f32,
    /// 1: the weapon in the right hand.
    pub right_hand: u8,
    /// 0: no view model.
    pub draw: u8,
}

impl Default for ViewModelSettings {
    fn default() -> Self {
        Self {
            fov: 80.0,
            default_fov: 90.0,
            right_hand: 1,
            draw: 1,
        }
    }
}

/// A game's own view-model field of view, for the characters whose game
/// identity (`slots::GameIdentity`) it is: an app with several games
/// (Lucker Party) draws each game's guns at that game's `fov` and
/// `default_fov`, the rest of `ViewModelSettings` shared. A character with
/// no identity, or a game not listed, uses `ViewModelSettings` itself.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct GameViewModelFovs(pub Vec<GameFov>);

/// One game's view-model field of view (degrees at 4:3, as
/// `ViewModelSettings`).
#[derive(Clone, Debug, PartialEq)]
pub struct GameFov {
    pub game: &'static str,
    pub fov: f32,
    pub default_fov: f32,
}

impl GameViewModelFovs {
    /// `base` with the fields of view of `game` (None: unchanged).
    pub fn settings_for(&self, base: ViewModelSettings, game: Option<&str>) -> ViewModelSettings {
        match game.and_then(|g| self.0.iter().find(|f| f.game == g)) {
            Some(f) => ViewModelSettings { fov: f.fov, default_fov: f.default_fov, ..base },
            None => base,
        }
    }

    /// `game`'s entry, added as it is given when it has none.
    pub fn entry(&mut self, game: &'static str, fov: f32, default_fov: f32) -> &mut GameFov {
        if let Some(i) = self.0.iter().position(|f| f.game == game) {
            return &mut self.0[i];
        }
        self.0.push(GameFov { game, fov, default_fov });
        self.0.last_mut().expect("just pushed")
    }
}

/// Which weapon effects are shown (Source `muzzleflash_light`,
/// `cl_ejectbrass`). Games set the values and console variables.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct EffectSettings {
    /// 1: muzzle flashes light their surroundings.
    pub muzzle_light: u8,
    /// 1: weapons eject shells.
    pub eject_brass: u8,
}

impl Default for EffectSettings {
    fn default() -> Self {
        Self {
            muzzle_light: 1,
            eject_brass: 1,
        }
    }
}

/// Where a character's view model sits relative to its eye (camera space:
/// X right, Y up, -Z forward; meters), e.g. walking bob and sway. Games
/// set it; it is mirrored with the model.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewModelOffset {
    pub translation: Vec3,
    pub rotation: Quat,
}

/// What a character's view model shows and plays. Games set the key and
/// drive the animator (simulation side, so it works headless); the drawing
/// follows it.
#[derive(Component, Clone, Debug, Default)]
pub struct ViewAnimator {
    /// The `MapViewModel` shown (None: nothing, e.g. dead).
    pub key: Option<String>,
    /// Plays on the view model's `AnimSet`.
    pub animator: Option<anim::Animator>,
    /// Not drawn for now, though still animated (e.g. behind a scope).
    pub hidden: bool,
}

impl ViewAnimator {
    /// Show `key`'s view model (a fresh animator on its sequences); no
    /// change if it is already shown.
    pub fn show(&mut self, key: Option<&str>, models: Option<&ViewModels>) -> bool {
        if self.key.as_deref() == key {
            return false;
        }
        self.key = key.map(str::to_string);
        self.animator = key
            .and_then(|k| models?.get(k)?.animations.clone())
            .map(anim::Animator::new);
        true
    }

    /// The sequence playing, by name.
    pub fn sequence(&self) -> Option<&str> {
        let a = self.animator.as_ref()?;
        Some(a.set.sequences.get(a.main?)?.name.as_str())
    }

    /// The activity of the sequence playing.
    pub fn activity(&self) -> Option<&str> {
        let a = self.animator.as_ref()?;
        Some(a.set.sequences.get(a.main?)?.activity.as_str())
    }
}

/// Effects a character's view-model animation asks for. Games send them
/// from animation events; the map shows them at the view model's
/// attachment for the first-person view, else at the weapon held by the
/// character's body.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct ViewModelEvent {
    pub owner: Entity,
    pub kind: ViewModelEventKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewModelEventKind {
    /// A muzzle flash at the view model's attachment (index).
    MuzzleFlash { attachment: usize },
    /// A shell ejected from the attachment (index): the shell type's name
    /// and speed (game units per second).
    EjectBrass {
        attachment: usize,
        shell: String,
        speed: f32,
    },
}

/// Put on a camera that is a child of a character (the client's
/// first-person camera): that character's view model is drawn at it.
#[derive(Component, Default)]
pub struct ViewModelAnchor;

/// On a `ViewModelAnchor`: draw this character's view model there instead
/// of the anchor's parent's (a spectator watching through someone's eyes).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewModelSource(pub Entity);

/// Whose view model an anchor (a child of `parent`) draws.
pub fn anchor_owner(parent: &ChildOf, source: Option<&ViewModelSource>) -> Entity {
    source.map_or(parent.parent(), |s| s.0)
}

/// The camera that draws view models (a child of the anchor).
#[derive(Component)]
pub struct ViewModelCamera;

/// Whether a view model is drawn mirrored (Source: `AllowFlipping` and
/// `BuiltRightHanded` != `cl_righthand`).
pub fn mirrored(right_handed: bool, allow_flipping: bool, right_hand: bool) -> bool {
    allow_flipping && right_handed != right_hand
}

/// A horizontal field of view given for a 4:3 screen, for a screen of
/// `aspect` (width / height). Degrees.
pub fn aspect_fov(fov_43: f32, aspect: f32) -> f32 {
    2.0 * ((fov_43.to_radians() / 2.0).tan() * aspect / (4.0 / 3.0))
        .atan()
        .to_degrees()
}

/// The vertical field of view of a horizontal 4:3 one (the same on every
/// screen). Degrees.
pub fn vertical_fov(fov_43: f32) -> f32 {
    2.0 * ((fov_43.to_radians() / 2.0).tan() * 0.75).atan().to_degrees()
}

/// The view model's horizontal 4:3 field of view: zoomed by as many
/// degrees as the player's (`player_fov`, 4:3) is below `default_fov`.
pub fn view_model_fov(settings: &ViewModelSettings, player_fov: f32) -> f32 {
    (settings.fov - (settings.default_fov - player_fov)).clamp(0.1, 179.9)
}

/// A point seen in the view-model pass (camera space), moved so that it
/// appears at the same place on screen in the world pass: right and up
/// scaled by tan(world fov / 2) / tan(view-model fov / 2), any matching
/// pair of fields of view (both vertical or both horizontal), in degrees.
pub fn reproject(p: Vec3, world_fov: f32, view_model_fov: f32) -> Vec3 {
    let k = (world_fov.to_radians() / 2.0).tan() / (view_model_fov.to_radians() / 2.0).tan().max(1e-6);
    Vec3::new(p.x * k, p.y * k, p.z)
}

/// The view model's placement under its camera: the offset, mirrored
/// across the camera's vertical plane (X) when `mirror` (the mirror acts
/// on the final model, so it mirrors the offset too).
pub fn placement(offset: &ViewModelOffset, mirror: bool) -> Transform {
    if !mirror {
        return Transform::from_translation(offset.translation).with_rotation(offset.rotation);
    }
    let (t, q) = (offset.translation, offset.rotation);
    Transform {
        translation: Vec3::new(-t.x, t.y, t.z),
        rotation: Quat::from_xyzw(q.x, -q.y, -q.z, q.w),
        scale: Vec3::new(-1.0, 1.0, 1.0),
    }
}

/// An attachment of `model` posed by `pose` (local bone poses), placed by
/// `placement`: in camera space.
pub fn attachment_transform(
    model: &MapViewModel,
    pose: &[anim::BonePose],
    attachment: usize,
    placement: &Transform,
) -> Option<Mat4> {
    let a = model.attachments.get(attachment)?;
    let mut global: Vec<Mat4> = Vec::with_capacity(model.bones.len());
    for (i, b) in model.bones.iter().enumerate() {
        let (q, p) = pose.get(i).copied().unwrap_or((b.rotation, b.position));
        let local = Mat4::from_rotation_translation(q, p);
        global.push(match b.parent.and_then(|p| global.get(p)) {
            Some(parent) => *parent * local,
            None => local,
        });
    }
    Some(placement.to_matrix() * model.root.to_matrix() * *global.get(a.bone)? * a.local.to_matrix())
}

/// A picture on a view model's screen: Source's VGUI screens on view
/// models (the C4's keypad display), drawn on a quad from the model's
/// `controlpanel0_ll` attachment to its `controlpanel0_ur`, along the lower
/// attachment's x and y axes. Put on the character whose view
/// model `key` shows it; whoever draws the picture keeps `image` current.
#[derive(Component, Clone, Debug)]
pub struct ViewModelScreen {
    pub key: String,
    pub image: Handle<Image>,
}

/// The screen's corners on a view model: the bone they sit on, the lower
/// corner's placement on it and the screen's size along its x and y axes
/// (the skeleton's units).
pub fn screen_corners(model: &MapViewModel) -> Option<(usize, Transform, Vec2)> {
    let find = |name: &str| model.attachments.iter().find(|a| a.name.eq_ignore_ascii_case(name));
    let (ll, ur) = (find("controlpanel0_ll")?, find("controlpanel0_ur")?);
    if ll.bone != ur.bone {
        return None;
    }
    let d = ur.local.translation - ll.local.translation;
    let size = Vec2::new(d.dot(ll.local.rotation * Vec3::X), d.dot(ll.local.rotation * Vec3::Y));
    (size.x.abs() > 1e-4 && size.y.abs() > 1e-4).then_some((ll.bone, ll.local, size))
}

/// A brief point light (muzzle flash) that lights the world, props and
/// view models (see `dynamic_light` in world.wesl and prop.wesl): its
/// radius shrinks to 0 over its life.
#[derive(Component, Clone, Copy, Debug)]
pub struct DynamicLight {
    pub radius: f32,
    pub life: f32,
    pub age: f32,
}

/// Where a character's held weapon shoots from (its world model's muzzle
/// attachment): a child of the held model.
#[derive(Component)]
pub struct HeldMuzzle {
    pub owner: Entity,
}
