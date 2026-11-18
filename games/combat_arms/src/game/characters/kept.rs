//! A map's character-model index (`models::CaModelIndex`: rigs, body
//! groups, held models' sockets) as bytes, for Combat Arms' guest dressing
//! kept on disk (`world::KEPT_MEMOS`, Chimera's `map::kept`): read back,
//! the same index a conversion registers, nothing parsed again.
//!
//! A rig is its parsed body file (`ltb::Model`) and its animation list,
//! whose animations are shared: with the model, between the rigs of one
//! body tree and with the install data's child models (`model::
//! ChildModels`, the animation files every body of a sex reads). Each
//! animation is written once (a pool, by its `Arc`) and the models, child
//! models and lists name it by place, so a read gives one copy shared the
//! same way; the child models go back into the install data's
//! `ChildModels` (unless one was read meanwhile), so a body loaded later
//! shares them too. The rigs' paths that shared one rig share it again.
//! The bytes are the same for the same index (everything in key or
//! registration order).

use std::{collections::BTreeMap, sync::Arc};

use lithtech_formats::{
    anim::{AnimList, Entry},
    ltb::{Animation, Model},
};
use serde::{Deserialize, Serialize};

use super::{
    models::{BodyGroup, CaModelIndex},
    skeleton::Rig,
};
use crate::game::{data::CaDataInner, model::ChildModels};

/// A model without its animations, which it names by pool place.
#[derive(Serialize, Deserialize)]
struct SavedModel {
    model: Model,
    animations: Vec<u32>,
}

#[derive(Serialize, Deserialize)]
struct SavedRig {
    /// The body files it serves.
    paths: Vec<String>,
    model: SavedModel,
    /// Each list entry: its animation's place, child slot, box and root
    /// translation.
    entries: Vec<(u32, usize, [f32; 3], [f32; 3])>,
    offsets: Vec<Option<usize>>,
}

/// The index as written.
#[derive(Serialize, Deserialize)]
struct Saved {
    animations: Vec<Animation>,
    children: Vec<(String, SavedModel)>,
    rigs: Vec<SavedRig>,
    groups: Vec<BodyGroup>,
    held_sockets: Vec<((String, String), (String, [f32; 3]))>,
}

/// The animation pool being written: each `Arc` once.
#[derive(Default)]
struct Pool {
    places: BTreeMap<usize, u32>,
    animations: Vec<Animation>,
}

impl Pool {
    fn place(&mut self, a: &Arc<Animation>) -> u32 {
        let key = Arc::as_ptr(a) as usize;
        if let Some(p) = self.places.get(&key) {
            return *p;
        }
        let p = self.animations.len() as u32;
        self.animations.push((**a).clone());
        self.places.insert(key, p);
        p
    }

    fn model(&mut self, m: &Model) -> SavedModel {
        let animations = m.animations.iter().map(|a| self.place(a)).collect();
        let mut model = m.clone();
        model.animations.clear();
        SavedModel { model, animations }
    }
}

fn model_of(saved: SavedModel, pool: &[Arc<Animation>]) -> Result<Model, String> {
    let mut model = saved.model;
    model.animations = saved
        .animations
        .iter()
        .map(|&i| pool.get(i as usize).cloned().ok_or("an animation out of the pool"))
        .collect::<Result<_, _>>()?;
    Ok(model)
}

/// `index` as bytes, its child models from `ca`'s `ChildModels`.
pub fn save(index: &CaModelIndex, ca: &CaDataInner) -> Result<Vec<u8>, String> {
    let mut pool = Pool::default();
    // The rigs, each once (its paths in order), in their first path's order.
    let mut rigs: Vec<(Arc<Rig>, Vec<String>)> = Vec::new();
    for (path, rig) in index.rigs() {
        match rigs.iter_mut().find(|(r, _)| Arc::ptr_eq(r, rig)) {
            Some((_, paths)) => paths.push(path.clone()),
            None => rigs.push((rig.clone(), vec![path.clone()])),
        }
    }
    let mut saved_rigs = Vec::new();
    for (rig, paths) in &rigs {
        let model = pool.model(&rig.model);
        let entries = rig
            .list
            .entries
            .iter()
            .map(|e| (pool.place(&e.animation), e.child, e.dims, e.translation))
            .collect();
        saved_rigs.push(SavedRig {
            paths: paths.clone(),
            model,
            entries,
            offsets: rig.list.offsets.clone(),
        });
    }
    // The child models whose animations the rigs play.
    let mut children = Vec::new();
    for (path, child) in ca.memo(ChildModels::default).all() {
        let used = child
            .animations
            .iter()
            .any(|a| pool.places.contains_key(&(Arc::as_ptr(a) as usize)));
        if used {
            children.push((path, pool.model(&child)));
        }
    }
    let saved = Saved {
        animations: pool.animations,
        children,
        rigs: saved_rigs,
        groups: index.groups().to_vec(),
        held_sockets: index
            .held_sockets()
            .iter()
            .map(|(k, (bone, at))| (k.clone(), (bone.clone(), at.to_array())))
            .collect(),
    };
    postcard::to_allocvec(&saved).map_err(|e| e.to_string())
}

/// `save`'s bytes back as an index; its child models kept in `ca`'s
/// `ChildModels`.
pub fn load(bytes: &[u8], ca: &CaDataInner) -> Result<CaModelIndex, String> {
    let saved: Saved = postcard::from_bytes(bytes).map_err(|e| e.to_string())?;
    let pool: Vec<Arc<Animation>> = saved.animations.into_iter().map(Arc::new).collect();
    let shared = ca.memo(ChildModels::default);
    for (path, child) in saved.children {
        shared.keep(&path, Arc::new(model_of(child, &pool)?));
    }
    let mut rigs = BTreeMap::new();
    for r in saved.rigs {
        let model = model_of(r.model, &pool)?;
        let entries = r
            .entries
            .into_iter()
            .map(|(a, child, dims, translation)| {
                Ok(Entry {
                    animation: pool.get(a as usize).cloned().ok_or("an animation out of the pool")?,
                    child,
                    dims,
                    translation,
                })
            })
            .collect::<Result<Vec<_>, &str>>()?;
        let list = AnimList {
            entries,
            offsets: r.offsets,
            ..Default::default()
        };
        let rig = Arc::new(Rig::new(model, list));
        for p in r.paths {
            rigs.insert(p, rig.clone());
        }
    }
    let held = saved
        .held_sockets
        .into_iter()
        .map(|(k, (bone, at))| (k, (bone, bevy::math::Vec3::from_array(at))))
        .collect();
    Ok(CaModelIndex::from_parts(rigs, saved.groups, held))
}
