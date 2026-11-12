//! The arms and hands of the view model (specs/combat_arms/
//! view_models.md 1 and 2): a separate skinned hand model chosen by the
//! player's character, its bones matched to the weapon model's nodes by
//! name (case ignored), its sleeve texture from the character, face and
//! uniform. `choose` picks the hand model and its skins from a
//! character-model table (`HandTable`); `choose_in` from the worn
//! character's record (`profile_of`: the cosmetic character's or the
//! body's model, after the face's redirect, its face and camouflage):
//! the install's `HandIndex`, `HandsSkin#` and `Hand#` records
//! (`records::ModelRecord::hands`, `CharacterData::hand_records`), else
//! ours by record id (data/hands.toml: the stock bodies), else the
//! default body's. A look whose hands differ from the default
//! character's has a view model of its own, keyed `<view key>#h<model>_
//! <face>_<uniform>` (`key_for`, `split_key`): the worn look's, made on
//! demand like a fitted part's (`on_demand`), so a ladder rung's body
//! brings its own hands. `attach` puts a converted hand model onto a view
//! model.
//!
//! Known shortcuts:
//! - The hand's vertices are moved from the hand's bind pose into the
//!   weapon skeleton's (per influence, weighted) so that they skin with
//!   the weapon's bones; exact when the two binds agree per bone (the
//!   stock hand has the weapon's arm nodes) or a vertex has one bone.
//! - No colour variants are in our table yet (the code reads them; every
//!   player's model colour is none), no overridable models or clan marks
//!   (skins 7, 8); the uniform item index is the look's camouflage index;
//!   the faces' `RedirectForHandsSkin` aren't read from the install.
//! - Only the local player's own look picks its hands (a spectated
//!   player's gun shows the default character's unless the local player's
//!   look made the same model), and a look change shows at once, not at
//!   the next draw (Open question 13: a rung's look and gun may arrive in
//!   either order on a client).

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::Deserialize;

use super::super::characters::records::{CharacterData, Selection};
use crate::map::{MapBone, MapMesh, MapViewModel};

/// What tattoo-slot skins become (2).
pub const NO_TATTOO: &str = "textures/8xnone.dtx";
/// The tattoo slots.
pub const TATTOO_SLOTS: &[&str] = &["pv_l_arm", "pv_r_arm"];

/// One `[HandN]` record.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct HandRecord {
    /// `ModelFile`.
    pub model: String,
    /// `Skin0..`.
    pub skins: Vec<String>,
    /// Colour-variant skin lists, by model colour.
    pub colour_skins: BTreeMap<u32, Vec<String>>,
}

/// A character model's hand fields.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct CharacterHands {
    /// The body (`Model#`) record id this is for (data/hands.toml).
    pub id: i32,
    /// `HandIndex` (-1: no hand model).
    pub hand_index: i32,
    /// `HandsSkin0..`: per face, a pattern with one two-digit number
    /// (`%02d`), filled with the uniform item index.
    pub hands_skins: Vec<String>,
    pub colour_hands_skins: BTreeMap<u32, Vec<String>>,
}

/// A face piece's redirect for the sleeve (`RedirectForHandsSkin`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Face {
    pub redirect_for_hands_skin: Option<u32>,
}

/// The hand records and the character models' hand fields.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct HandTable {
    pub hands: Vec<HandRecord>,
    pub models: Vec<CharacterHands>,
    /// By face piece id (missing: no redirect).
    pub faces: BTreeMap<u32, Face>,
}

/// Our table (data/hands.toml).
pub fn builtin() -> HandTable {
    toml::from_str(include_str!("../../../data/hands.toml")).expect("data/hands.toml")
}

/// What the player wears (the local profile's face and uniform: Quirk 5).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct HandProfile {
    /// The character model id (a cosmetic costume's when one is worn,
    /// after the face's redirect).
    pub model: i32,
    pub face: u32,
    /// The equipped uniform (camo) item index.
    pub uniform: u32,
    /// The model colour (None: none, every player's).
    pub colour: Option<u32>,
    /// The player's character exists (else the hand keeps its record's
    /// own skins).
    pub character: bool,
}

/// The hand model to attach and its skins.
#[derive(Clone, Debug, PartialEq)]
pub struct HandChoice {
    pub model: String,
    pub skins: Vec<String>,
}

/// Fill a `%02d` pattern.
fn fill(pattern: &str, n: u32) -> String {
    pattern.replacen("%02d", &format!("{n:02}"), 1)
}

/// The hand model and skins for `profile` (1, 2) from `table` (its
/// model `profile.model`, else its first); None for hand index -1 (no
/// arms).
pub fn choose(table: &HandTable, profile: &HandProfile) -> Option<HandChoice> {
    let model = table.models.iter().find(|m| m.id == profile.model).or(table.models.first())?;
    choose_with(&table.hands, model, &table.faces, profile)
}

/// `choose` with the hand records `hands`, the body's hand fields `model`
/// and the faces' redirects `faces`.
fn choose_with(hands: &[HandRecord], model: &CharacterHands, faces: &BTreeMap<u32, Face>, profile: &HandProfile) -> Option<HandChoice> {
    let hand = hands.get(usize::try_from(model.hand_index).ok()?)?;
    let mut skins = profile
        .colour
        .and_then(|c| hand.colour_skins.get(&c))
        .filter(|l| !l.is_empty())
        .unwrap_or(&hand.skins)
        .clone();
    if profile.character {
        let f = faces
            .get(&profile.face)
            .and_then(|f| f.redirect_for_hands_skin)
            .unwrap_or(profile.face) as usize;
        let list = profile
            .colour
            .and_then(|c| model.colour_hands_skins.get(&c))
            .filter(|l| !l.is_empty())
            .unwrap_or(&model.hands_skins);
        let sleeve = match list.get(f) {
            Some(p) => fill(p, profile.uniform),
            None => hand.skins.first().cloned().unwrap_or_default(),
        };
        match skins.first_mut() {
            Some(s) => *s = sleeve,
            None => skins.push(sleeve),
        }
    }
    for s in &mut skins {
        if TATTOO_SLOTS.iter().any(|t| s.eq_ignore_ascii_case(t)) {
            *s = NO_TATTOO.to_string();
        }
    }
    Some(HandChoice {
        model: hand.model.clone(),
        skins,
    })
}

/// What separates a view-model key from its hands (`key_for`).
pub const SEP: char = '#';

/// The hands `look` wears in `data` (1): the cosmetic character's model,
/// else the body's, after the face's redirect; its face and its
/// camouflage index as the uniform item; no model colour.
pub fn profile_of(data: &CharacterData, look: &Selection) -> HandProfile {
    HandProfile {
        model: data.hands_record(look).id,
        face: u32::try_from(look.face).unwrap_or(0),
        uniform: u32::try_from(look.camouflage).unwrap_or(0),
        colour: None,
        character: true,
    }
}

/// The hand model and skins `profile` wears in `data` (`choose`): the
/// record's hand fields and the hand records `data` carries (the
/// install's, else ours, `CharacterData::load`); a record without hands
/// (an NPC body): the default body's.
pub fn choose_in(data: &CharacterData, profile: &HandProfile) -> Option<HandChoice> {
    let model = data
        .model(profile.model)
        .and_then(|m| m.hands.as_ref())
        .or(data.default_model().hands.as_ref())?;
    choose_with(&data.hand_records, model, &BTreeMap::new(), profile)
}

/// The default character's hands in `data` (the default look's): those
/// every view model without a key of hands (`key_for`) wears.
pub fn default_choice(data: &CharacterData) -> Option<HandChoice> {
    choose_in(data, &profile_of(data, &Selection::default()))
}

/// The hands part of a view-model key for `look` (`SEP`, then
/// `h<model>_<face>_<uniform>`), None when they are the default
/// character's (paths compared without case or slash kind).
pub fn key_for(data: &CharacterData, look: &Selection) -> Option<String> {
    let profile = profile_of(data, look);
    let norm = |c: Option<HandChoice>| {
        c.map(|c| {
            let n = |s: &str| s.to_ascii_lowercase().replace('\\', "/");
            (n(&c.model), c.skins.iter().map(|s| n(s)).collect::<Vec<_>>())
        })
    };
    if norm(choose_in(data, &profile)) == norm(default_choice(data)) {
        return None;
    }
    Some(format!("{SEP}h{}_{}_{}", profile.model, profile.face, profile.uniform))
}

/// View-model key `key` with the hands part `hands` (`key_for`).
pub fn with_hands(key: &str, hands: Option<&str>) -> String {
    match hands {
        Some(h) => format!("{key}{h}"),
        None => key.to_string(),
    }
}

/// The hands view-model key `key` wears (`split_key`): those its hands
/// part names (`choose_in`), else the default character's; what
/// `on_demand::load_weapon` attaches and `chimera_pack sets` records.
pub fn choice_for_key(data: &CharacterData, key: &str) -> Option<HandChoice> {
    match split_key(key).1 {
        Some(p) => choose_in(data, &p),
        None => default_choice(data),
    }
}

/// A view-model key without its hands part, and the hands it names (None:
/// the default character's).
pub fn split_key(key: &str) -> (&str, Option<HandProfile>) {
    let Some((rest, hands)) = key.split_once(SEP) else { return (key, None) };
    let profile = hands.strip_prefix('h').and_then(|h| {
        let mut it = h.split('_');
        let model = it.next()?.parse().ok()?;
        let face = it.next()?.parse().ok()?;
        let uniform = it.next()?.parse().ok()?;
        Some(HandProfile {
            model,
            face,
            uniform,
            colour: None,
            character: true,
        })
    });
    (rest, profile)
}

/// Global bind matrices of a skeleton (parents before children).
pub fn globals(bones: &[MapBone]) -> Vec<Mat4> {
    let mut out: Vec<Mat4> = Vec::with_capacity(bones.len());
    for b in bones {
        let local = Mat4::from_rotation_translation(b.rotation, b.position);
        let g = match b.parent.and_then(|p| out.get(p)) {
            Some(parent) => *parent * local,
            None => local,
        };
        out.push(g);
    }
    out
}

/// Each hand bone's weapon node, by name with case ignored (1); an error
/// names the first hand bone the weapon lacks.
pub fn match_bones(hand: &[MapBone], weapon: &[MapBone]) -> Result<Vec<usize>, String> {
    hand.iter()
        .map(|h| {
            weapon
                .iter()
                .position(|w| w.name.eq_ignore_ascii_case(&h.name))
                .ok_or_else(|| format!("hand bone {:?} has no weapon node", h.name))
        })
        .collect()
}

/// Put the hand model (`meshes` skinned to `bones`, positions in metres
/// at its bind pose with `scale` metres per unit) onto `view`: its joints
/// become the weapon's matching nodes, its vertices move into the
/// weapon skeleton's bind pose and to the view model's origin (`offset`,
/// metres). Returns the bone mapping.
pub fn attach(
    view: &mut MapViewModel,
    meshes: Vec<MapMesh>,
    bones: &[MapBone],
    scale: f32,
    offset: Vec3,
) -> Result<Vec<usize>, String> {
    let map = match_bones(bones, &view.bones)?;
    let (hand_g, weapon_g) = (globals(bones), globals(&view.bones));
    // Per hand bone: from the hand's bind to the weapon's (units).
    let to_weapon: Vec<Mat4> = map
        .iter()
        .enumerate()
        .map(|(h, &w)| weapon_g[w] * hand_g[h].inverse())
        .collect();
    for mut m in meshes {
        for (v, p) in m.positions.iter_mut().enumerate() {
            let units = Vec3::from(*p) / scale;
            let (moved, normal) = blend(&to_weapon, m.joints.get(v), m.joint_weights.get(v), units, m.normals.get(v));
            *p = (moved * scale + offset).to_array();
            if let (Some(n), Some(out)) = (normal, m.normals.get_mut(v)) {
                *out = n.to_array();
            }
        }
        for j in m.joints.iter_mut().flat_map(|j| j.iter_mut()) {
            *j = map.get(*j as usize).copied().unwrap_or(0) as u16;
        }
        view.model.meshes.push(m);
    }
    Ok(map)
}

pub(crate) fn blend(
    to: &[Mat4],
    joints: Option<&[u16; 4]>,
    weights: Option<&[f32; 4]>,
    p: Vec3,
    n: Option<&[f32; 3]>,
) -> (Vec3, Option<Vec3>) {
    let (Some(joints), Some(weights)) = (joints, weights) else {
        return (p, n.map(|n| Vec3::from(*n)));
    };
    let (mut out, mut normal, mut total) = (Vec3::ZERO, Vec3::ZERO, 0.0);
    for (j, w) in joints.iter().zip(weights) {
        if *w <= 0.0 {
            continue;
        }
        let m = to.get(*j as usize).copied().unwrap_or(Mat4::IDENTITY);
        out += m.transform_point3(p) * *w;
        if let Some(n) = n {
            normal += m.transform_vector3(Vec3::from(*n)) * *w;
        }
        total += w;
    }
    if total <= 0.0 {
        return (p, n.map(|n| Vec3::from(*n)));
    }
    (out / total, n.map(|_| normal.normalize_or_zero()))
}
