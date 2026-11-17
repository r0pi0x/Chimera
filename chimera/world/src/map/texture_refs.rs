//! Every texture and cubemap index a map's parts hold (`TextureRefs`):
//! what moves when parts converted on their own (their own texture list,
//! from 0) are appended to a map's (`shift`), as a guest's dressing kept
//! for the session is (`source_engine::content::guest`). Each part visits
//! its indices into `MapData::textures` (`Ref::Texture`) and
//! `MapData::cubemaps` (`Ref::Cubemap`); a field that holds one and is
//! missed here keeps its old index, which a test of each user compares
//! against a fresh conversion (`source_engine` guest tests: the dressed
//! map serialized both ways).

use super::{
    GameKit, MapCharacterModel, MapEnvmap, MapHeldModel, MapMesh, MapMeshLook, MapModel, MapMuzzleFlash, MapPhong,
    MapTextureAnim,
    decal::{MapDecal, MapDecals},
    hud::GameHud,
    particles::ParticleMaterial,
    shells::MapShell,
    view_model::MapViewModel,
};

/// Which list an index points into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ref {
    /// `MapData::textures`.
    Texture,
    /// `MapData::cubemaps`.
    Cubemap,
}

/// A part holding indices into the map's textures or cubemaps.
pub trait TextureRefs {
    /// Call `f` with each index this part holds (mutable: a caller moves
    /// them).
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize));

    /// Every index moved past a map's `textures` and `cubemaps` (the
    /// counts of what the part is appended after).
    fn shift(&mut self, textures: usize, cubemaps: usize) {
        if textures == 0 && cubemaps == 0 {
            return;
        }
        self.texture_refs(&mut |r, i| match r {
            Ref::Texture => *i += textures,
            Ref::Cubemap => *i += cubemaps,
        });
    }
}

fn opt(i: &mut Option<usize>, r: Ref, f: &mut dyn FnMut(Ref, &mut usize)) {
    if let Some(i) = i {
        f(r, i);
    }
}

fn phong(p: &mut Option<Box<MapPhong>>, f: &mut dyn FnMut(Ref, &mut usize)) {
    if let Some(p) = p {
        opt(&mut p.exponent_texture, Ref::Texture, f);
        opt(&mut p.lightwarp, Ref::Texture, f);
        opt(&mut p.phong_warp, Ref::Texture, f);
    }
}

fn envmap(e: &mut Option<MapEnvmap>, f: &mut dyn FnMut(Ref, &mut usize)) {
    if let Some(e) = e {
        opt(&mut e.cubemap, Ref::Cubemap, f);
        if let super::EnvmapMask::Texture(i) = &mut e.mask {
            f(Ref::Texture, i);
        }
    }
}

/// The look's parts a mesh and a skin's look share.
#[allow(clippy::too_many_arguments)]
fn look(
    texture: &mut Option<usize>,
    normal_map: &mut Option<usize>,
    phong_: &mut Option<Box<MapPhong>>,
    envmap_: &mut Option<MapEnvmap>,
    detail: &mut Option<super::MapDetail>,
    selfillum: &mut Option<super::MapSelfIllum>,
    skin_frames: &mut Option<super::MapSkinFrames>,
    layer: &mut Option<super::MapLayer>,
    f: &mut dyn FnMut(Ref, &mut usize),
) {
    opt(texture, Ref::Texture, f);
    opt(normal_map, Ref::Texture, f);
    phong(phong_, f);
    envmap(envmap_, f);
    if let Some(d) = detail {
        f(Ref::Texture, &mut d.texture);
    }
    if let Some(s) = selfillum {
        opt(&mut s.mask, Ref::Texture, f);
    }
    if let Some(s) = skin_frames {
        s.frames.iter_mut().for_each(|i| f(Ref::Texture, i));
    }
    if let Some(l) = layer {
        opt(&mut l.texture, Ref::Texture, f);
    }
}

impl TextureRefs for MapMesh {
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize)) {
        look(
            &mut self.texture,
            &mut self.normal_map,
            &mut self.phong,
            &mut self.envmap,
            &mut self.detail,
            &mut self.selfillum,
            &mut self.skin_frames,
            &mut self.layer,
            f,
        );
        if let Some(b) = &mut self.blend {
            opt(&mut b.texture, Ref::Texture, f);
            opt(&mut b.normal_map, Ref::Texture, f);
            opt(&mut b.mask, Ref::Texture, f);
        }
        if let Some(e) = &mut self.flat_envmap {
            f(Ref::Texture, &mut e.texture);
        }
        opt(&mut self.tint_mask, Ref::Texture, f);
        opt(&mut self.pane_mask, Ref::Texture, f);
        self.frames.iter_mut().for_each(|i| f(Ref::Texture, i));
        if let Some(eye) = &mut self.eye {
            opt(&mut eye.iris, Ref::Texture, f);
        }
    }
}

impl TextureRefs for MapMeshLook {
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize)) {
        look(
            &mut self.texture,
            &mut self.normal_map,
            &mut self.phong,
            &mut self.envmap,
            &mut self.detail,
            &mut self.selfillum,
            &mut self.skin_frames,
            &mut self.layer,
            f,
        );
    }
}

impl TextureRefs for MapModel {
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize)) {
        self.meshes.iter_mut().for_each(|m| m.texture_refs(f));
        self.skins.iter_mut().flatten().for_each(|l| l.texture_refs(f));
        for lod in &mut self.lods {
            lod.meshes.iter_mut().flatten().for_each(|m| m.texture_refs(f));
        }
    }
}

impl TextureRefs for MapCharacterModel {
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize)) {
        self.model.texture_refs(f);
    }
}

impl TextureRefs for MapHeldModel {
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize)) {
        self.model.texture_refs(f);
        self.merged.iter_mut().for_each(|(_, m)| m.texture_refs(f));
        if let Some(m) = &mut self.own {
            m.texture_refs(f);
        }
    }
}

impl TextureRefs for MapViewModel {
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize)) {
        self.model.texture_refs(f);
    }
}

impl TextureRefs for MapShell {
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize)) {
        self.model.texture_refs(f);
    }
}

impl TextureRefs for MapDecal {
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize)) {
        f(Ref::Texture, &mut self.texture);
    }
}

impl TextureRefs for MapDecals {
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize)) {
        self.decals.iter_mut().for_each(|d| d.texture_refs(f));
    }
}

impl TextureRefs for ParticleMaterial {
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize)) {
        opt(&mut self.texture, Ref::Texture, f);
    }
}

impl TextureRefs for MapMuzzleFlash {
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize)) {
        opt(&mut self.texture, Ref::Texture, f);
        self.alt_textures.iter_mut().for_each(|i| f(Ref::Texture, i));
    }
}

impl TextureRefs for GameHud {
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize)) {
        // Sorted by name: the same order on every run.
        let mut sprites: Vec<_> = self.sprites.iter_mut().collect();
        sprites.sort_by(|a, b| a.0.cmp(b.0));
        sprites.into_iter().for_each(|(_, s)| f(Ref::Texture, &mut s.texture));
    }
}

impl TextureRefs for MapTextureAnim {
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize)) {
        self.frames.iter_mut().for_each(|i| f(Ref::Texture, i));
    }
}

/// A kit's parts that draw: bodies, held and view models, decals, HUD
/// looks (each `Arc` made its own when shared), muzzle flashes and shells.
impl TextureRefs for GameKit {
    fn texture_refs(&mut self, f: &mut dyn FnMut(Ref, &mut usize)) {
        self.characters.iter_mut().for_each(|c| c.texture_refs(f));
        self.held.iter_mut().for_each(|h| h.texture_refs(f));
        self.view_models.iter_mut().for_each(|v| v.texture_refs(f));
        self.decals.texture_refs(f);
        for hud in self.hud.iter_mut().chain(self.guest_huds.iter_mut()) {
            std::sync::Arc::make_mut(hud).texture_refs(f);
        }
        if let Some(m) = &mut self.muzzle_flash {
            m.texture_refs(f);
        }
        self.muzzle_flash_kinds.iter_mut().for_each(|(_, m)| m.texture_refs(f));
        self.shells.iter_mut().for_each(|s| s.texture_refs(f));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each kind of index a mesh holds moves, the cubemap's by the
    /// cubemaps' count.
    #[test]
    fn a_shift_moves_every_index_of_a_mesh() {
        let mut mesh = MapMesh {
            texture: Some(0),
            normal_map: Some(1),
            frames: vec![2, 3],
            envmap: Some(MapEnvmap {
                cubemap: Some(0),
                mask: super::super::EnvmapMask::Texture(4),
                tint: [1.0; 3],
                contrast: 0.0,
                saturation: 1.0,
                fresnel: 0.0,
                imposter: false,
                light_mask: None,
            }),
            ..Default::default()
        };
        mesh.shift(10, 3);
        assert_eq!((mesh.texture, mesh.normal_map), (Some(10), Some(11)));
        assert_eq!(mesh.frames, [12, 13]);
        let e = mesh.envmap.unwrap();
        assert_eq!(e.cubemap, Some(3));
        assert_eq!(e.mask, super::super::EnvmapMask::Texture(14));
    }
}
