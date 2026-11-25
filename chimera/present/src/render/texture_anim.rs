//! Animated textures (Source's AnimatedTexture proxy,
//! `MapData::texture_anims`): every world and prop material that binds
//! one of an animation's frames as its base texture or normal map shows
//! the frame for the time (`MapTextureAnim::frame`, a hard switch), on
//! whichever copy of the material it is (tinted brush entities, props' own
//! instances). Materials bind the first frame at load; the frames' own
//! images say which animation a material shows. Time counts from the
//! map's first drawn frame.
//!
//! The materials that bind a frame are kept in an index by animation
//! (`Index`: every material looked at once, then the asset stores'
//! additions, changes and removals), so an animation's frame change
//! touches its own materials only, and a material that comes into a store
//! later (a copy, a kit model's material drawn for the first time: those
//! wait outside the store, `DormantProps`) takes its animation's frame as
//! it comes. Before (feedback b33ceded and afd90ba5: `animate_textures` in
//! most of the spikes on dod_flash and de_dust2) each frame change of any
//! animation, most frames with several at their own rates, walked every
//! world and prop material and every dormant one.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use super::{prop_material::PropMaterial, world_material::WorldMaterial};
use crate::map::{MapData, MapTextureAnim};

/// The map's animations with their frames as images, and the frame each
/// shows now.
#[derive(Resource, Default)]
pub struct TextureAnims {
    anims: Vec<(MapTextureAnim, Vec<Handle<Image>>)>,
    /// Each frame's image: its animation.
    by_image: HashMap<AssetId<Image>, usize>,
    shown: Vec<usize>,
    /// When the map started drawing (the animations' time 0, as Source's
    /// proxies count from the map's start).
    start: Option<f32>,
    /// The materials binding a frame, by animation (module doc); None
    /// until every material was looked at once.
    index: Option<Index>,
}

/// The materials binding each animation's frames.
#[derive(Default)]
struct Index {
    world: Bound<WorldMaterial>,
    props: Bound<PropMaterial>,
}

/// One store's materials by animation, and each material's animations
/// (its base's and its normal map's).
struct Bound<A: Asset> {
    by_anim: Vec<HashSet<AssetId<A>>>,
    of: HashMap<AssetId<A>, [Option<u32>; 2]>,
}

impl<A: Asset> Default for Bound<A> {
    fn default() -> Self {
        Self { by_anim: Vec::new(), of: HashMap::new() }
    }
}

impl<A: Asset> Bound<A> {
    /// `id` binds the frames of `now`'s animations (none: forget it).
    /// Whether it changed.
    fn set(&mut self, id: AssetId<A>, now: [Option<u32>; 2]) -> bool {
        let old = self.of.get(&id).copied().unwrap_or([None; 2]);
        if old == now {
            return false;
        }
        for k in old.into_iter().flatten() {
            if let Some(s) = self.by_anim.get_mut(k as usize) {
                s.remove(&id);
            }
        }
        for k in now.into_iter().flatten() {
            if self.by_anim.len() <= k as usize {
                self.by_anim.resize_with(k as usize + 1, HashSet::new);
            }
            self.by_anim[k as usize].insert(id);
        }
        if now == [None; 2] {
            self.of.remove(&id);
        } else {
            self.of.insert(id, now);
        }
        true
    }

    fn len(&self) -> usize {
        self.of.len()
    }
}

impl TextureAnims {
    pub fn new(data: &MapData, textures: &[Handle<Image>]) -> Self {
        let mut me = Self::default();
        for anim in &data.texture_anims {
            let Some(frames) = anim
                .frames
                .iter()
                .map(|&i| textures.get(i).cloned())
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let k = me.anims.len();
            for f in &frames {
                me.by_image.insert(f.id(), k);
            }
            me.anims.push((anim.clone(), frames));
            me.shown.push(0);
        }
        me
    }

    /// The image a material binding `image` shows now, when it is an
    /// animation's frame and not that one.
    fn swap(&self, image: Option<&Handle<Image>>) -> Option<Handle<Image>> {
        let image = image?;
        let &k = self.by_image.get(&image.id())?;
        let want = &self.anims[k].1[self.shown[k]];
        (want != image).then(|| want.clone())
    }

    /// The animation whose frame `image` is.
    fn anim_of(&self, image: Option<&Handle<Image>>) -> Option<u32> {
        image.and_then(|i| self.by_image.get(&i.id())).map(|&k| k as u32)
    }

    fn world_anims(&self, m: &WorldMaterial) -> [Option<u32>; 2] {
        [self.anim_of(m.base.as_ref()), self.anim_of(m.normal.as_ref())]
    }

    fn prop_anims(&self, m: &PropMaterial) -> [Option<u32>; 2] {
        [self.anim_of(m.base.as_ref()), None]
    }

    /// How many world and prop materials bind a frame (tests); None before
    /// the first look.
    pub fn bound(&self) -> Option<(usize, usize)> {
        self.index.as_ref().map(|i| (i.world.len(), i.props.len()))
    }
}

/// A world material shows its animations' frames for now.
fn show_world(anims: &TextureAnims, store: &mut Assets<WorldMaterial>, id: AssetId<WorldMaterial>) {
    let due = store.get(id).is_some_and(|m| anims.swap(m.base.as_ref()).is_some() || anims.swap(m.normal.as_ref()).is_some());
    if due && let Some(mut m) = store.get_mut(id) {
        if let Some(b) = anims.swap(m.base.as_ref()) {
            m.base = Some(b);
        }
        if let Some(n) = anims.swap(m.normal.as_ref()) {
            m.normal = Some(n);
        }
    }
}

/// A prop material shows its animation's frame for now.
fn show_prop(anims: &TextureAnims, store: &mut Assets<PropMaterial>, id: AssetId<PropMaterial>) {
    if let Some(b) = store.get(id).and_then(|m| anims.swap(m.base.as_ref()))
        && let Some(mut m) = store.get_mut(id)
    {
        m.base = Some(b);
    }
}

/// What a store's event says of the index; whether `id` binds a frame now
/// and is new to it (to be shown the frame for now).
fn reindex<A: Asset>(bound: &mut Bound<A>, store: &Assets<A>, ev: &AssetEvent<A>, anims_of: impl Fn(&A) -> [Option<u32>; 2]) -> Option<AssetId<A>> {
    match *ev {
        AssetEvent::Added { id } | AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id } => {
            let now = store.get(id).map_or([None; 2], &anims_of);
            (bound.set(id, now) && now != [None; 2]).then_some(id)
        }
        AssetEvent::Removed { id } | AssetEvent::Unused { id } => {
            if store.get(id).is_none() {
                bound.set(id, [None; 2]);
            }
            None
        }
    }
}

/// Show each animation's frame for the time: on a frame change, the
/// materials binding one of its frames take the new one.
pub fn animate_textures(
    time: Res<Time>,
    anims: Option<ResMut<TextureAnims>>,
    mut world: Option<ResMut<Assets<WorldMaterial>>>,
    mut props: Option<ResMut<Assets<PropMaterial>>>,
    world_events: Option<MessageReader<AssetEvent<WorldMaterial>>>,
    prop_events: Option<MessageReader<AssetEvent<PropMaterial>>>,
) {
    // A store's events exist where its store does (tests build their own).
    let (mut world_events, mut prop_events) = (world_events.filter(|_| world.is_some()), prop_events.filter(|_| props.is_some()));
    let Some(mut anims) = anims.filter(|a| !a.anims.is_empty()) else {
        world_events.iter_mut().for_each(|r| r.clear());
        prop_events.iter_mut().for_each(|r| r.clear());
        return;
    };
    // The frames for now.
    let now = time.elapsed_secs();
    let t = now - *anims.start.get_or_insert(now);
    let mut changed = Vec::new();
    for k in 0..anims.anims.len() {
        let f = anims.anims[k].0.frame(t).min(anims.anims[k].1.len() - 1);
        if anims.shown[k] != f {
            anims.shown[k] = f;
            changed.push(k);
        }
    }
    // The index: every material once, then what the stores changed; a
    // material new to it shows the frame for now at once.
    let mut index = anims.index.take();
    match &mut index {
        None => {
            world_events.iter_mut().for_each(|r| r.clear());
            prop_events.iter_mut().for_each(|r| r.clear());
            let mut i = Index::default();
            if let Some(w) = world.as_deref() {
                for (id, m) in w.iter() {
                    i.world.set(id, anims.world_anims(m));
                }
            }
            if let Some(p) = props.as_deref() {
                for (id, m) in p.iter() {
                    i.props.set(id, anims.prop_anims(m));
                }
            }
            // Every animation's materials, once.
            changed = (0..anims.anims.len()).collect();
            index = Some(i);
        }
        Some(i) => {
            if let (Some(store), Some(events)) = (world.as_deref_mut(), world_events.as_mut()) {
                let new: Vec<_> = events.read().filter_map(|ev| reindex(&mut i.world, store, ev, |m| anims.world_anims(m))).collect();
                for id in new {
                    show_world(&anims, store, id);
                }
            }
            if let (Some(store), Some(events)) = (props.as_deref_mut(), prop_events.as_mut()) {
                let new: Vec<_> = events.read().filter_map(|ev| reindex(&mut i.props, store, ev, |m| anims.prop_anims(m))).collect();
                for id in new {
                    show_prop(&anims, store, id);
                }
            }
        }
    }
    let i = index.as_ref().expect("indexed above");
    for &k in &changed {
        if let (Some(store), Some(ids)) = (world.as_deref_mut(), i.world.by_anim.get(k)) {
            for &id in ids {
                show_world(&anims, store, id);
            }
        }
        if let (Some(store), Some(ids)) = (props.as_deref_mut(), i.props.by_anim.get(k)) {
            for &id in ids {
                show_prop(&anims, store, id);
            }
        }
    }
    anims.index = index;
}
