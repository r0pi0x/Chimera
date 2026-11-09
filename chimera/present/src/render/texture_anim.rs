//! Animated textures (Source's AnimatedTexture proxy,
//! `MapData::texture_anims`): every world and prop material that binds
//! one of an animation's frames as its base texture or normal map shows
//! the frame for the time (`MapTextureAnim::frame`, a hard switch), on
//! whichever copy of the material it is (tinted brush entities, props' own
//! instances). Materials bind the first frame at load; the frames' own
//! images say which animation a material shows. Time counts from the
//! map's first drawn frame.

use std::collections::HashMap;

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
}

/// Show each animation's frame for the time: on a frame change, the
/// materials binding one of its frames take the new one.
pub fn animate_textures(
    time: Res<Time>,
    anims: Option<ResMut<TextureAnims>>,
    world: Option<ResMut<Assets<WorldMaterial>>>,
    props: Option<ResMut<Assets<PropMaterial>>>,
    dormant: Option<ResMut<super::prop_material::DormantProps>>,
) {
    let Some(mut anims) = anims else { return };
    if anims.anims.is_empty() {
        return;
    }
    let now = time.elapsed_secs();
    let t = now - *anims.start.get_or_insert(now);
    let mut changed = false;
    for k in 0..anims.anims.len() {
        let f = anims.anims[k].0.frame(t).min(anims.anims[k].1.len() - 1);
        if anims.shown[k] != f {
            anims.shown[k] = f;
            changed = true;
        }
    }
    if !changed {
        return;
    }
    if let Some(mut world) = world {
        let due: Vec<_> = world
            .iter()
            .filter(|(_, m)| anims.swap(m.base.as_ref()).is_some() || anims.swap(m.normal.as_ref()).is_some())
            .map(|(id, _)| id)
            .collect();
        for id in due {
            if let Some(mut m) = world.get_mut(id) {
                if let Some(b) = anims.swap(m.base.as_ref()) {
                    m.base = Some(b);
                }
                if let Some(n) = anims.swap(m.normal.as_ref()) {
                    m.normal = Some(n);
                }
            }
        }
    }
    if let Some(mut props) = props {
        let due: Vec<_> = props
            .iter()
            .filter(|(_, m)| anims.swap(m.base.as_ref()).is_some())
            .map(|(id, _)| id)
            .collect();
        for id in due {
            if let Some(mut m) = props.get_mut(id)
                && let Some(b) = anims.swap(m.base.as_ref())
            {
                m.base = Some(b);
            }
        }
    }
    // The kit's models' materials not drawn yet (`DormantProps`).
    if let Some(mut dormant) = dormant {
        for m in dormant.values_mut() {
            if let Some(b) = anims.swap(m.base.as_ref()) {
                m.base = Some(b);
            }
        }
    }
}
