//! First-person view models (any game), what games and the drawing
//! share: games load `MapViewModel`s with the map (or later, on demand:
//! `add_view_model`, its own textures in `LateViewModelTextures`), drive each character's
//! `ViewAnimator` (which sequence plays, which meshes are hidden) and
//! `ViewModelOffset` (bob and sway), and send `ViewModelEvent`s (muzzle flashes, brass);
//! `render::view_model` draws the view model of the character whose
//! camera carries `ViewModelAnchor`, with the placement maths here
//! (`placement`, `reproject`, `attachment_transform`, the fields of view).
//!
//! First-person view models for any game: `MapViewModel`, `ViewModels`,
//! `ViewAnimator`, `ViewModelOffset`, `ViewModelSettings`, `EffectSettings`,
//! `ViewModelEvent`, the anchor and camera markers, `ViewModelScreen`,
//! `DynamicLight`, `HeldMuzzle`, `HeldAttachment` and the placement maths.

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
pub struct ViewModels(pub Arc<Vec<Arc<MapViewModel>>>);

impl ViewModels {
    /// The list of `models`.
    pub fn of(models: impl IntoIterator<Item = MapViewModel>) -> Self {
        Self(Arc::new(models.into_iter().map(Arc::new).collect()))
    }

    pub fn get(&self, key: &str) -> Option<&MapViewModel> {
        self.0.iter().find(|v| v.key == key).map(|v| &**v)
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
/// its next frames (`render::view_model::build_late`); until then the
/// key draws as if it had none. With no map yet, it waits for one.
pub fn add_view_model(world: &mut World, model: impl Into<Arc<MapViewModel>>, textures: Arc<Vec<MapTexture>>) {
    let model = model.into();
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

/// The screen shake the local view model gets on top of the view's, in
/// the view's own frame (metres; roll in radians): Source places the
/// view model at the shaken eye and shakes it again at 10 %
/// (specs/cs_source/view_models.md 1, step 4; specs/tf2/viewmodel.md 3,
/// step 6). Written by the client's shake, added to the view model's
/// placement when drawn.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewModelShake {
    pub offset: Vec3,
    pub roll: f32,
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
    /// `default_fov` is the player's own unzoomed field of view
    /// (`fov_desired`, which the client keeps in it: `follow_player`), so
    /// the view model keeps its `fov` whatever the player's base is
    /// (HL2:DM, player_models.md 12.4). False: `default_fov` stays as set
    /// (CS:S's 90, Combat Arms').
    pub follows_player: bool,
}

/// An app of one game whose `ViewModelSettings.default_fov` is the
/// player's unzoomed field of view (`GameFov::follows_player`'s meaning
/// for the app's only game): the client keeps it so.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct SettingsFollowPlayerFov;

impl GameViewModelFovs {
    /// The games that follow the player's field of view take `base` (the
    /// player's unzoomed field of view) as their `default_fov`.
    pub fn follow_player(&mut self, base: f32) {
        for f in self.0.iter_mut().filter(|f| f.follows_player) {
            if f.default_fov != base {
                f.default_fov = base;
            }
        }
    }

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
        self.0.push(GameFov { game, fov, default_fov, follows_player: false });
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
    /// Meshes of the model (indices into its `model.meshes`) not drawn
    /// for now, the rest drawn (Combat Arms' fired rocket: a piece by
    /// name, `MapModel::mesh_names`). Cleared when another model shows.
    pub hidden_meshes: Vec<usize>,
    /// Meshes of the model that show one of their base texture's frames
    /// (`MapMesh::frames`) picked by the game, as (mesh index, frame): a
    /// number or a gauge on the gun (Halo's ammunition counter); the rest
    /// show their own texture. Cleared when another model shows.
    pub mesh_frames: Vec<(usize, usize)>,
    /// Bones turned by the game on top of the animation, as (bone index
    /// in the shown model's skeleton, radians about the bone's own z
    /// axis): TF2's Minigun barrel spinning in first person. Cleared when
    /// another model shows.
    pub bone_rolls: Vec<(usize, f32)>,
}

impl ViewAnimator {
    /// Show `key`'s view model (a fresh animator on its sequences, every
    /// mesh drawn); no change if it is already shown, unless it was shown
    /// before its model came in (true then: the animator starts now).
    pub fn show(&mut self, key: Option<&str>, models: Option<&ViewModels>) -> bool {
        if self.key.as_deref() == key {
            // Shown before its model was there (a map's view models in
            // after the weapon was given: a ladder's first rung at a
            // match's start): started now, or the drawing keeps hiding a
            // model with animations and none playing.
            if self.animator.is_none()
                && let Some(set) = key.and_then(|k| models?.get(k)?.animations.clone())
            {
                self.animator = Some(anim::Animator::new(set));
                return true;
            }
            return false;
        }
        self.key = key.map(str::to_string);
        self.hidden_meshes.clear();
        self.mesh_frames.clear();
        self.bone_rolls.clear();
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
    /// A sound event of a reload's sequence that other players hear too
    /// (`ViewAnimRules::others_reload`): after this many seconds. None:
    /// the owner alone does.
    pub others: Option<f32>,
    /// An event of a reload's sequence (one a reload started: whole clip,
    /// or a one-at-a-time reload's start, loop or end).
    pub reload: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewModelEventKind {
    /// A muzzle flash at the view model's attachment (index).
    MuzzleFlash { attachment: usize },
    /// A muzzle flash of a kind the game's kit names
    /// (`GameKit::muzzle_flash_kinds`: "PISTOL", "COMBINE").
    MuzzleFlashOf { attachment: usize, kind: String },
    /// A shell ejected from the attachment (index): the shell type's name
    /// and speed (game units per second).
    EjectBrass {
        attachment: usize,
        shell: String,
        speed: f32,
    },
    /// A sound entry the animation plays (Source's event 5004), heard by
    /// the view model's owner alone (`chimera_sim::weapon::view_anim`:
    /// specs/tf2/viewmodel_sounds.md 1.1).
    Sound { entry: String },
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

/// One of a held weapon's attachments (`MapHeldModel::attachments`): a
/// child of the held model, at the attachment; `index` in the model's
/// list (`map::attached` draws sprites and beams here).
#[derive(Component, Clone, Debug)]
pub struct HeldAttachment {
    pub owner: Entity,
    pub index: usize,
    pub name: String,
}

/// Each bone's place in its skeleton's reference pose (the skeleton's
/// space), from `bones` (parents before children).
pub fn reference_world(bones: &[MapBone]) -> Vec<Mat4> {
    let pose: Vec<anim::BonePose> = bones.iter().map(|b| (b.rotation, b.position)).collect();
    posed_world(bones, &pose)
}

/// Each bone's place in the skeleton's space with its local `pose`
/// (`anim::Animator::pose`'s), parents from `bones`.
pub fn posed_world(bones: &[MapBone], pose: &[anim::BonePose]) -> Vec<Mat4> {
    let mut out: Vec<Mat4> = Vec::with_capacity(bones.len());
    for (i, b) in bones.iter().enumerate() {
        let (r, t) = pose.get(i).copied().unwrap_or((b.rotation, b.position));
        let local = Mat4::from_rotation_translation(r, t);
        out.push(match b.parent.and_then(|p| out.get(p)) {
            Some(parent) => *parent * local,
            None => local,
        });
    }
    out
}

/// `arms` with `weapon` bone-merged onto it by bone name (Source's
/// bone merge, as TF2's `c_` view models are drawn: the class's arms
/// play, the weapon follows them; docs/plans/active/tf2.md, decision 5):
/// one view model, the arms' skeleton, animations and placement. Each of
/// the weapon's bones follows the arms' bone of the same name (case
/// ignored), or its nearest ancestor's that has one; its attachments are
/// put on the arms' bones (the muzzle). The weapon's own bones without a
/// match go rigid with that ancestor (they play nothing of the arms'),
/// where `own_pose` puts them (the weapon's own pose, its idle: TF2's
/// minigun's barrel; None: its reference pose).
///
/// With the arms' animations (`merge_as_joints`) each weapon bone becomes
/// a joint of its own under the arms bone it follows, bound in the
/// weapon's reference pose and posed on that bone (the animation set's
/// defaults: no offset from it), so its meshes keep the weapon's own
/// skinning: a vertex weighted across bones whose reference poses differ
/// from the arms' (TF2's glove unlocks' fingers, bound open where the
/// arms' are bound in a fist) deforms as in the game. Without them the
/// meshes are moved from the weapon's reference pose into the arms' (by
/// the matched bone's place in each) and skinned to the arms' bones,
/// which is exact only for a vertex on one bone. Skins and lower levels
/// of detail are dropped (the meshes show the skin they were loaded
/// with). Errors when no bone matches.
pub fn merge_by_bone_name(
    arms: MapViewModel,
    weapon: &MapViewModel,
    own_pose: Option<&[anim::BonePose]>,
) -> Result<MapViewModel, String> {
    merge_keeping(arms, weapon, own_pose, &[])
}

/// `merge_by_bone_name`, the weapon's bones named in `keep` (that match
/// no arms bone) kept as joints of their own, appended to the arms'
/// skeleton under the arms bone they follow, at their own pose, with
/// their vertices: a game turns them (`ViewAnimator::bone_rolls`: TF2's
/// Minigun `barrel`). The animation doesn't pose them.
pub fn merge_keeping(
    mut arms: MapViewModel,
    weapon: &MapViewModel,
    own_pose: Option<&[anim::BonePose]>,
    keep: &[&str],
) -> Result<MapViewModel, String> {
    let index = |name: &str| arms.bones.iter().position(|b| b.name.eq_ignore_ascii_case(name));
    // Per weapon bone: the arms' bone it follows and the weapon's bone
    // that matched (itself or an ancestor).
    let mut follows: Vec<Option<(usize, usize)>> = Vec::with_capacity(weapon.bones.len());
    for (k, b) in weapon.bones.iter().enumerate() {
        let own = index(&b.name).map(|j| (j, k));
        let inherited = b.parent.filter(|p| *p < k).and_then(|p| follows[p]);
        follows.push(own.or(inherited));
    }
    let Some(first) = follows.iter().flatten().next().copied() else {
        return Err(format!("{}: no bone of it is on {}'s skeleton", weapon.key, arms.key));
    };
    let arms_ref = reference_world(&arms.bones);
    let weapon_ref = reference_world(&weapon.bones);
    let weapon_posed = own_pose.map_or_else(|| weapon_ref.clone(), |p| posed_world(&weapon.bones, p));
    // Bone `k`'s place relative to its matched ancestor `a`'s.
    let relative = |a: usize, k: usize| -> Mat4 {
        if a == k { Mat4::IDENTITY } else { weapon_posed[a].inverse() * weapon_posed[k] }
    };
    let (to_eye, from_eye) = (arms.root.to_matrix(), weapon.root.to_matrix().inverse());
    if arms.animations.as_mut().and_then(Arc::get_mut).is_some_and(|set| set.defaults.len() <= arms.bones.len()) {
        let rel: Vec<Mat4> = follows.iter().enumerate().map(|(k, f)| relative(f.unwrap_or(first).1, k)).collect();
        let attached: Vec<(usize, Mat4)> = weapon
            .attachments
            .iter()
            .map(|a| {
                let (j, matched) = follows.get(a.bone).copied().flatten().unwrap_or(first);
                let local = if a.bone < weapon.bones.len() { relative(matched, a.bone) } else { Mat4::IDENTITY };
                (j, local * a.local.to_matrix())
            })
            .collect();
        return Ok(merge_as_joints(arms, weapon, &follows, first, &rel, &weapon_ref, to_eye * from_eye, keep, attached));
    }
    // Per weapon bone: from the weapon's eye space to the arms'.
    let mut moves: Vec<(usize, Mat4)> = follows
        .iter()
        .enumerate()
        .map(|(k, f)| {
            let (j, a) = f.unwrap_or(first);
            (j, to_eye * arms_ref[j] * relative(a, k) * weapon_ref[k].inverse() * from_eye)
        })
        .collect();
    // Kept bones: a joint of their own under the arms bone they follow, at
    // their place relative to it; their vertices on it.
    let mut kept = Vec::new();
    for (k, b) in weapon.bones.iter().enumerate() {
        // Named, and matched by an ancestor, not by its own name.
        let Some((j, a)) = follows[k].filter(|(_, a)| *a != k) else { continue };
        if !keep.iter().any(|n| b.name.eq_ignore_ascii_case(n)) {
            continue;
        }
        let (_, rotation, position) = relative(a, k).to_scale_rotation_translation();
        moves[k].0 = arms.bones.len() + kept.len();
        kept.push(MapBone { name: b.name.clone(), parent: Some(j), position, rotation });
    }
    arms.bones.extend(kept);
    let arms_meshes = arms.model.meshes.len();
    arms.model.mesh_names.resize(arms_meshes, String::new());
    for (i, mesh) in weapon.model.meshes.iter().enumerate() {
        let mut m = mesh.clone();
        let rigid = |v: usize| -> (Mat4, [u16; 4]) {
            let (joints, weights) = (m.joints.get(v).copied(), m.joint_weights.get(v).copied());
            let Some(joints) = joints else {
                return (moves[first.1].1, [first.0 as u16; 4]);
            };
            let weights = weights.unwrap_or([1.0, 0.0, 0.0, 0.0]);
            let mut blended = Mat4::ZERO;
            let mut out = [0u16; 4];
            for n in 0..4 {
                let (j, mv) = moves.get(joints[n] as usize).copied().unwrap_or((first.0, moves[first.1].1));
                out[n] = j as u16;
                blended += mv * weights[n];
            }
            (blended, out)
        };
        let per_vertex: Vec<(Mat4, [u16; 4])> = (0..m.positions.len()).map(rigid).collect();
        for (v, (mv, joints)) in per_vertex.into_iter().enumerate() {
            m.positions[v] = mv.transform_point3(Vec3::from(m.positions[v])).to_array();
            if let Some(n) = m.normals.get_mut(v) {
                *n = mv.transform_vector3(Vec3::from(*n)).normalize_or_zero().to_array();
            }
            if let Some(j) = m.joints.get_mut(v) {
                *j = joints;
            }
        }
        // An unskinned weapon mesh: all of it on the first matched bone.
        if mesh.joints.is_empty() {
            m.joints = vec![[first.0 as u16; 4]; m.positions.len()];
            m.joint_weights = vec![[1.0, 0.0, 0.0, 0.0]; m.positions.len()];
        }
        arms.model.meshes.push(m);
        arms.model.mesh_names.push(weapon.model.mesh_names.get(i).cloned().unwrap_or_default());
    }
    arms.model.skins.clear();
    arms.model.lods.clear();
    for a in &weapon.attachments {
        let (j, matched) = follows.get(a.bone).copied().flatten().unwrap_or(first);
        // In the matched bone's frame (the same as the arms' bone's).
        let local = if a.bone < weapon.bones.len() { relative(matched, a.bone) } else { Mat4::IDENTITY } * a.local.to_matrix();
        arms.attachments.push(MapAttachment {
            name: a.name.clone(),
            bone: j,
            local: Transform::from_matrix(local),
        });
    }
    Ok(arms)
}

/// `merge_keeping` on arms that play (their animation set ours alone to
/// grow): weapon bone `k` becomes joint `arms.bones.len() + k`, a child
/// of the arms bone it follows (`follows`, else `first`), its rest the
/// weapon's reference pose (`weapon_ref`: the bind its vertices keep) and
/// its pose in every sequence `rel[k]` from that bone (identity for a
/// bone matched by name: it is where the arms' bone is; its place from
/// its matched ancestor for one of its own), added to the set's defaults
/// and bases. The meshes stay as the weapon has them (`eye`: from its eye
/// space to the arms'), their joints renumbered. A kept bone's joint
/// keeps its name (a game turns it); the others are `<name>#merged`, so
/// a lookup by name still finds the arms' bone. Attachments (`attached`:
/// arms bone and place on it) as the baked merge puts them.
#[allow(clippy::too_many_arguments)]
fn merge_as_joints(
    mut arms: MapViewModel,
    weapon: &MapViewModel,
    follows: &[Option<(usize, usize)>],
    first: (usize, usize),
    rel: &[Mat4],
    weapon_ref: &[Mat4],
    eye: Mat4,
    keep: &[&str],
    attached: Vec<(usize, Mat4)>,
) -> MapViewModel {
    let arms_ref = reference_world(&arms.bones);
    let base = arms.bones.len();
    let rest: Vec<anim::BonePose> = arms.bones.iter().map(|b| (b.rotation, b.position)).collect();
    let mut posed: Vec<anim::BonePose> = Vec::with_capacity(weapon.bones.len());
    let mut joints: Vec<(MapBone, String)> = Vec::with_capacity(weapon.bones.len());
    for (k, b) in weapon.bones.iter().enumerate() {
        let (j, a) = follows[k].unwrap_or(first);
        let kept = a != k && keep.iter().any(|n| b.name.eq_ignore_ascii_case(n));
        let name = if kept { b.name.clone() } else { format!("{}#merged", b.name) };
        let (_, rotation, position) = (arms_ref[j].inverse() * weapon_ref[k]).to_scale_rotation_translation();
        let (_, r, t) = rel[k].to_scale_rotation_translation();
        posed.push((r, t));
        joints.push((MapBone { name: name.clone(), parent: Some(j), position, rotation }, name.to_lowercase()));
    }
    if let Some(set) = arms.animations.as_mut().and_then(Arc::get_mut) {
        // Every pose one entry per bone: the arms' own past the set's
        // (their rest), then the weapon's joints.
        let grow = |list: &mut Vec<anim::BonePose>| {
            let have = list.len();
            list.extend_from_slice(&rest[have.min(base)..]);
            list.extend_from_slice(&posed);
        };
        grow(&mut set.defaults);
        for b in &mut set.bases {
            grow(b);
        }
        if !set.parents.is_empty() {
            set.parents.resize(base, None);
            set.parents.extend(joints.iter().map(|(b, _)| b.parent));
        }
        if !set.bone_names.is_empty() {
            set.bone_names.resize(base, String::new());
            set.bone_names.extend(joints.iter().map(|(_, n)| n.clone()));
        }
    }
    arms.bones.extend(joints.into_iter().map(|(b, _)| b));
    let arms_meshes = arms.model.meshes.len();
    arms.model.mesh_names.resize(arms_meshes, String::new());
    let joint = |k: u16| if (k as usize) < weapon.bones.len() { (base + k as usize) as u16 } else { (base + first.1) as u16 };
    for (i, mesh) in weapon.model.meshes.iter().enumerate() {
        let mut m = mesh.clone();
        for p in &mut m.positions {
            *p = eye.transform_point3(Vec3::from(*p)).to_array();
        }
        for n in &mut m.normals {
            *n = eye.transform_vector3(Vec3::from(*n)).normalize_or_zero().to_array();
        }
        if mesh.joints.is_empty() {
            // An unskinned weapon mesh: all of it on the first matched bone.
            m.joints = vec![[(base + first.1) as u16; 4]; m.positions.len()];
            m.joint_weights = vec![[1.0, 0.0, 0.0, 0.0]; m.positions.len()];
        } else {
            for j in &mut m.joints {
                *j = j.map(joint);
            }
        }
        arms.model.meshes.push(m);
        arms.model.mesh_names.push(weapon.model.mesh_names.get(i).cloned().unwrap_or_default());
    }
    arms.model.skins.clear();
    arms.model.lods.clear();
    for (a, (j, local)) in weapon.attachments.iter().zip(attached) {
        arms.attachments.push(MapAttachment { name: a.name.clone(), bone: j, local: Transform::from_matrix(local) });
    }
    arms
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{MapMesh, MapModel};

    fn bone(name: &str, parent: Option<usize>, x: f32) -> MapBone {
        MapBone { name: name.into(), parent, position: Vec3::new(x, 0.0, 0.0), rotation: Quat::IDENTITY }
    }

    fn model(key: &str, bones: Vec<MapBone>, meshes: Vec<MapMesh>) -> MapViewModel {
        MapViewModel {
            key: key.into(),
            model: MapModel { meshes, ..MapModel::default() },
            bones,
            root: Transform::IDENTITY,
            animations: None,
            right_handed: true,
            allow_flipping: false,
            attachments: Vec::new(),
            light_origin: Vec3::ZERO,
        }
    }

    /// The weapon's meshes go from its reference pose into the arms'
    /// (the matched bone's place in each), skinned to the arms' bone;
    /// a bone of its own follows its matched ancestor, an attachment
    /// on it lands on the arms' bone at the same place.
    #[test]
    fn a_weapon_follows_the_arms_bone_of_its_name() {
        let arms = model("arms", vec![bone("root", None, 0.0), bone("weapon_bone", Some(0), 10.0)], vec![MapMesh::default()]);
        let mesh = MapMesh {
            positions: vec![[3.0, 0.0, 0.0], [5.0, 1.0, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]; 2],
            joints: vec![[0, 0, 0, 0], [1, 0, 0, 0]],
            joint_weights: vec![[1.0, 0.0, 0.0, 0.0]; 2],
            ..MapMesh::default()
        };
        let mut weapon = model("gun", vec![bone("WEAPON_BONE", None, 2.0), bone("barrel", Some(0), 3.0)], vec![mesh]);
        weapon.attachments.push(MapAttachment { name: "muzzle".into(), bone: 1, local: Transform::from_xyz(1.0, 0.0, 0.0) });
        let merged = merge_by_bone_name(arms.clone(), &weapon, None).unwrap();
        assert_eq!(merged.model.meshes.len(), 2);
        let m = &merged.model.meshes[1];
        // Weapon bone at 2 in its model, at 10 on the arms: +8.
        assert_eq!(m.positions, vec![[11.0, 0.0, 0.0], [13.0, 1.0, 0.0]]);
        assert_eq!(m.joints, vec![[1, 1, 1, 1], [1, 1, 1, 1]]);
        let a = &merged.attachments[0];
        assert_eq!(a.bone, 1);
        // The barrel is 3 past the weapon bone, the muzzle 1 past it.
        assert!((a.local.translation - Vec3::new(4.0, 0.0, 0.0)).length() < 1e-5);
        let none = model("other", vec![bone("x", None, 0.0)], Vec::new());
        assert!(merge_by_bone_name(none, &weapon, None).is_err());
        // Its own pose for its own bone: the barrel turned a quarter about
        // Z there (its reference pose straight) turns the barrel's
        // vertex about the weapon bone's place.
        let turned = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let pose = [(Quat::IDENTITY, Vec3::new(2.0, 0.0, 0.0)), (turned, Vec3::new(3.0, 0.0, 0.0))];
        let merged = merge_by_bone_name(arms, &weapon, Some(&pose)).unwrap();
        let p = Vec3::from(merged.model.meshes[1].positions[1]);
        // (5, 1, 0) is 0 along the barrel and 1 up from it: turned, 1 back.
        assert!((p - Vec3::new(12.0, 0.0, 0.0)).length() < 1e-4, "{p}");
        // Kept (`merge_keeping`): the barrel a joint of its own under the
        // arms' weapon bone, 3 past it, its vertex on it, the rest as before.
        let kept = merge_keeping(arms_kept(), &weapon, None, &["barrel"]).unwrap();
        assert_eq!(kept.bones.len(), 3);
        let b = &kept.bones[2];
        assert_eq!((b.name.as_str(), b.parent), ("barrel", Some(1)));
        assert!((b.position - Vec3::new(3.0, 0.0, 0.0)).length() < 1e-5);
        let m = &kept.model.meshes[1];
        assert_eq!(m.positions, vec![[11.0, 0.0, 0.0], [13.0, 1.0, 0.0]]);
        assert_eq!(m.joints, vec![[1, 1, 1, 1], [2, 1, 1, 1]], "weighted to the barrel only");
    }

    /// On arms that play, a weapon vertex weighted across two bones
    /// whose reference poses differ from the arms' (turned a quarter, set
    /// apart) lands, at any pose of the arms, where Source's bone merge
    /// puts it: each weapon bone at its arms bone's place, the vertex
    /// skinned from the weapon's own reference pose. Baking it into the
    /// arms' reference pose (one blended move) misses when the two arms
    /// bones turn apart.
    #[test]
    fn a_vertex_on_two_merged_bones_skins_as_the_weapon_binds_it() {
        // Its own animation set (the merge grows it).
        let playing = || {
            let mut arms = model("arms", vec![bone("root", None, 0.0), bone("hand", Some(0), 10.0), bone("finger", Some(1), 2.0)], vec![MapMesh::default()]);
            let defaults: Vec<anim::BonePose> = arms.bones.iter().map(|b| (b.rotation, b.position)).collect();
            arms.animations = Some(Arc::new(anim::AnimSet { defaults: defaults.clone(), bases: vec![defaults], ..default() }));
            arms
        };
        let arms = playing();
        let quarter = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let mut hand = bone("HAND", None, 3.0);
        hand.rotation = quarter;
        let mut finger = bone("finger", Some(0), 0.0);
        finger.position = Vec3::new(0.0, 4.0, 0.0);
        finger.rotation = Quat::from_rotation_x(0.7);
        let mesh = MapMesh {
            positions: vec![[1.0, 5.0, 0.5]],
            normals: vec![[0.0, 1.0, 0.0]],
            joints: vec![[0, 1, 0, 0]],
            joint_weights: vec![[0.5, 0.5, 0.0, 0.0]],
            ..MapMesh::default()
        };
        let weapon = model("glove", vec![hand, finger], vec![mesh]);
        let merged = merge_by_bone_name(playing(), &weapon, None).unwrap();
        assert_eq!(merged.bones.len(), 5);
        assert_eq!((merged.bones[3].name.as_str(), merged.bones[3].parent), ("HAND#merged", Some(1)));
        assert_eq!(merged.model.meshes[1].joints, vec![[3, 4, 3, 3]]);
        let set = merged.animations.clone().unwrap();
        assert_eq!(set.defaults.len(), 5);
        // The arms' finger bent a third of a turn, the hand moved.
        let mut pose = set.defaults.clone();
        pose[1] = (Quat::from_rotation_y(0.4), Vec3::new(9.0, 1.0, 0.0));
        pose[2].0 = Quat::from_rotation_z(2.1);
        let posed = posed_world(&merged.bones, &pose);
        let bind = reference_world(&merged.bones);
        let m = &merged.model.meshes[1];
        let v = Vec3::from(m.positions[0]);
        let got: Vec3 = (0..2).map(|n| (posed[m.joints[0][n] as usize] * bind[m.joints[0][n] as usize].inverse()).transform_point3(v) * 0.5).sum();
        let weapon_ref = reference_world(&weapon.bones);
        let arms_posed = posed_world(&arms.bones, &pose[..3]);
        let original = Vec3::from(weapon.model.meshes[0].positions[0]);
        let want: Vec3 = [(1, 0), (2, 1)].iter().map(|&(j, k)| (arms_posed[j] * weapon_ref[k].inverse()).transform_point3(original) * 0.5).sum();
        assert!((got - want).length() < 1e-4, "{got} vs {want}");
    }

    fn arms_kept() -> MapViewModel {
        model("arms", vec![bone("root", None, 0.0), bone("weapon_bone", Some(0), 10.0)], vec![MapMesh::default()])
    }
}
