//! Models added in play built without stopping the frame (docs/performance.md,
//! "Nothing blocks the main thread"). What `map::add_view_model`,
//! `add_held_model` and `add_character_model` bring has to become images,
//! meshes and materials: copying texels, making mip levels, copying vertices,
//! a skinned mesh's bounds. All of that runs on a task here (`LateQueue::start`,
//! `AsyncComputeTaskPool`), with handles reserved in advance (`Stage`), so the
//! code that describes a model is the same as a map's spawn, which adds to the
//! stores directly (`Stores`).
//!
//! What must touch the world is only putting those assets into `Assets`, and
//! that is spread too (`Stage::fill`, at most `FILL_BYTES` of texels and vertices
//! and `FILL_MATERIALS` materials a frame), because each asset put in there
//! this frame is uploaded by the render thread this frame: a body is about 39 MB of
//! textures, which in one piece took the render world's frame to hundreds of
//! millions of instructions. The model comes out (`LateQueue::step`) a frame after its
//! last asset, once that has been prepared, and until then whoever wants it
//! keeps what it shows.
//!
//! A model's materials wait outside the store (`prop_material::DormantProps`,
//! when the app has it) until something draws them: then none of it reaches the
//! render thread here; without it, `FILL_MATERIALS` a frame go into the store.
//!
//! Known shortcuts (docs/tech-debt.md): one image or mesh is not split, so a
//! single asset larger than `FILL_BYTES` is that frame's whole budget and more.

use std::{collections::VecDeque, sync::atomic::{AtomicU64, Ordering}};

use bevy::{
    asset::{Asset, AssetHandleProvider},
    mesh::skinning::SkinnedMeshInverseBindposes,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};

use super::prop_material::{DormantProps, PropMaterial};

/// Bytes of texels and vertices one queue puts into `Assets` a frame (the
/// render thread uploads them that frame). Three queues run: at most three
/// times this a frame, about 2 ms of copying on the render thread.
pub const FILL_BYTES: usize = 4 * 1024 * 1024;

/// Materials one queue puts into `Assets` a frame (a bind group each on the
/// render thread).
pub const FILL_MATERIALS: usize = 12;

/// Which spawned map a queue's work is for (`next_generation`): a queue
/// drops what it has when the drawing it builds for is replaced.
pub fn next_generation() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Where a model's assets go as it is described: a map's spawn adds to the
/// stores (`Stores`), a task reserves handles (`Stage`).
pub trait AddAsset<A: Asset> {
    fn add_asset(&mut self, asset: A) -> Handle<A>;
}

/// The stores themselves (a map's spawn).
pub struct Stores<'a> {
    pub meshes: &'a mut Assets<Mesh>,
    pub materials: &'a mut Assets<PropMaterial>,
    pub bindposes: &'a mut Assets<SkinnedMeshInverseBindposes>,
    /// Where materials wait until something draws them (`DormantProps`);
    /// None: into `materials`.
    pub dormant: Option<&'a mut DormantProps>,
}

impl AddAsset<Mesh> for Stores<'_> {
    fn add_asset(&mut self, asset: Mesh) -> Handle<Mesh> {
        self.meshes.add(asset)
    }
}

impl AddAsset<PropMaterial> for Stores<'_> {
    fn add_asset(&mut self, asset: PropMaterial) -> Handle<PropMaterial> {
        match self.dormant.as_deref_mut() {
            Some(d) => d.add(self.materials, asset),
            None => self.materials.add(asset),
        }
    }
}

impl AddAsset<SkinnedMeshInverseBindposes> for Stores<'_> {
    fn add_asset(&mut self, asset: SkinnedMeshInverseBindposes) -> Handle<SkinnedMeshInverseBindposes> {
        self.bindposes.add(asset)
    }
}

/// The stores a queue fills.
pub struct LateStores<'a> {
    pub images: &'a mut Assets<Image>,
    pub stores: Stores<'a>,
}

/// One model's assets, made on a task: their handles are good at once, the
/// assets go in frame by frame (`fill`).
pub struct Stage {
    providers: Providers,
    images: VecDeque<(AssetId<Image>, Image)>,
    meshes: VecDeque<(AssetId<Mesh>, Mesh)>,
    bindposes: VecDeque<(AssetId<SkinnedMeshInverseBindposes>, SkinnedMeshInverseBindposes)>,
    materials: VecDeque<(Handle<PropMaterial>, PropMaterial)>,
}

/// What reserves handles (from any thread).
#[derive(Clone)]
pub struct Providers {
    images: AssetHandleProvider,
    meshes: AssetHandleProvider,
    bindposes: AssetHandleProvider,
    materials: AssetHandleProvider,
}

impl Providers {
    pub fn of(late: &LateStores) -> Self {
        Self {
            images: late.images.get_handle_provider(),
            meshes: late.stores.meshes.get_handle_provider(),
            bindposes: late.stores.bindposes.get_handle_provider(),
            materials: late.stores.materials.get_handle_provider(),
        }
    }
}

fn mesh_bytes(mesh: &Mesh) -> usize {
    // About a skinned vertex, and the indices.
    mesh.count_vertices() * 56 + mesh.indices().map_or(0, |i| i.len() * 4)
}

impl Stage {
    fn new(providers: Providers) -> Self {
        Self {
            providers,
            images: VecDeque::new(),
            meshes: VecDeque::new(),
            bindposes: VecDeque::new(),
            materials: VecDeque::new(),
        }
    }

    /// Put in this frame's share, images and meshes before what uses them;
    /// true when nothing is left. At least one asset goes in each call, however
    /// large.
    fn fill(&mut self, to: &mut LateStores) -> bool {
        let mut bytes = 0;
        while bytes < FILL_BYTES {
            let Some((id, image)) = self.images.pop_front() else { break };
            bytes += image.data.as_ref().map_or(0, Vec::len);
            let _ = to.images.insert(id, image);
        }
        while bytes < FILL_BYTES {
            let Some((id, mesh)) = self.meshes.pop_front() else { break };
            bytes += mesh_bytes(&mesh);
            let _ = to.stores.meshes.insert(id, mesh);
        }
        if !self.images.is_empty() || !self.meshes.is_empty() {
            return false;
        }
        for (id, b) in self.bindposes.drain(..) {
            let _ = to.stores.bindposes.insert(id, b);
        }
        if let Some(d) = to.stores.dormant.as_deref_mut() {
            // Dormant until drawn: nothing for the render world yet.
            for (handle, material) in self.materials.drain(..) {
                d.insert(&handle, material);
            }
        }
        for _ in 0..FILL_MATERIALS {
            let Some((handle, material)) = self.materials.pop_front() else { break };
            let _ = to.stores.materials.insert(handle.id(), material);
        }
        self.materials.is_empty()
    }
}

impl AddAsset<Image> for Stage {
    fn add_asset(&mut self, asset: Image) -> Handle<Image> {
        let h = self.providers.images.reserve_handle().typed::<Image>();
        self.images.push_back((h.id(), asset));
        h
    }
}

impl AddAsset<Mesh> for Stage {
    fn add_asset(&mut self, asset: Mesh) -> Handle<Mesh> {
        let h = self.providers.meshes.reserve_handle().typed::<Mesh>();
        self.meshes.push_back((h.id(), asset));
        h
    }
}

impl AddAsset<SkinnedMeshInverseBindposes> for Stage {
    fn add_asset(&mut self, asset: SkinnedMeshInverseBindposes) -> Handle<SkinnedMeshInverseBindposes> {
        let h = self.providers.bindposes.reserve_handle().typed::<SkinnedMeshInverseBindposes>();
        self.bindposes.push_back((h.id(), asset));
        h
    }
}

impl AddAsset<PropMaterial> for Stage {
    fn add_asset(&mut self, asset: PropMaterial) -> Handle<PropMaterial> {
        let h = self.providers.materials.reserve_handle().typed::<PropMaterial>();
        self.materials.push_back((h.clone(), asset));
        h
    }
}

/// Models being built, in the order they were started: each comes out whole,
/// in that order (a body's place in `PlayModels` is its index).
pub struct LateQueue<T> {
    generation: u64,
    tasks: VecDeque<(String, Task<(T, Stage)>)>,
    /// The front model going into the stores, then settling a frame.
    filling: Option<(String, T, Stage)>,
    settled: Option<(String, T)>,
}

impl<T> Default for LateQueue<T> {
    fn default() -> Self {
        Self {
            generation: 0,
            tasks: VecDeque::new(),
            filling: None,
            settled: None,
        }
    }
}

impl<T: Send + 'static> LateQueue<T> {
    /// Drop what was started for another drawing (`next_generation`).
    pub fn for_generation(&mut self, generation: u64) {
        if self.generation != generation {
            *self = Self {
                generation,
                ..Self::default()
            };
        }
    }

    /// Whether `key` is on its way.
    pub fn has(&self, key: &str) -> bool {
        self.tasks.iter().any(|(k, _)| k == key)
            || self.filling.as_ref().is_some_and(|(k, ..)| k == key)
            || self.settled.as_ref().is_some_and(|(k, _)| k == key)
    }

    /// Models on their way.
    pub fn len(&self) -> usize {
        self.tasks.len() + self.filling.is_some() as usize + self.settled.is_some() as usize
    }

    /// Whether nothing is on its way.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Describe model `key` on a task (`cook`: what comes out, its assets
    /// added to the `Stage`).
    pub fn start(&mut self, key: String, providers: Providers, cook: impl FnOnce(&mut Stage) -> T + Send + 'static) {
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let mut stage = Stage::new(providers);
            let out = cook(&mut stage);
            (out, stage)
        });
        self.tasks.push_back((key, task));
    }

    /// This frame's step: the model that is done, if any.
    pub fn step(&mut self, to: &mut LateStores) -> Option<(String, T)> {
        if let Some(done) = self.settled.take() {
            return Some(done);
        }
        if self.filling.is_none()
            && let Some((_, task)) = self.tasks.front_mut()
            && let Some((out, stage)) = block_on(poll_once(task))
            && let Some((key, _)) = self.tasks.pop_front()
        {
            self.filling = Some((key, out, stage));
        }
        if let Some((_, _, stage)) = &mut self.filling
            && stage.fill(to)
            && let Some((key, out, _)) = self.filling.take()
        {
            self.settled = Some((key, out));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };

    use super::*;

    fn image(side: u32) -> Image {
        Image::new_uninit(
            Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        )
    }

    fn image_with_data(side: u32) -> Image {
        let mut i = image(side);
        i.data = Some(vec![0; (side * side * 4) as usize]);
        i
    }

    #[test]
    fn images_go_in_a_frames_share_at_a_time_and_handles_are_good_at_once() {
        let mut images = Assets::<Image>::default();
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<PropMaterial>::default();
        let mut bindposes = Assets::<SkinnedMeshInverseBindposes>::default();
        let mut to = LateStores {
            images: &mut images,
            stores: Stores {
                meshes: &mut meshes,
                materials: &mut materials,
                bindposes: &mut bindposes,
                dormant: None,
            },
        };
        let mut stage = Stage::new(Providers::of(&to));
        // Each image is 1 MiB: FILL_BYTES of them a frame.
        let handles: Vec<_> = (0..10).map(|_| stage.add_asset(image_with_data(512))).collect();
        assert!(!stage.fill(&mut to));
        let first = handles.iter().filter(|h| to.images.contains(h.id())).count();
        assert_eq!(first * 1024 * 1024, FILL_BYTES, "one frame's share");
        let mut frames = 2;
        while !stage.fill(&mut to) {
            frames += 1;
        }
        assert_eq!(frames, 3, "10 MiB at 4 MiB a frame");
        assert!(handles.iter().all(|h| to.images.contains(h.id())));
    }

    #[test]
    fn one_image_over_the_share_still_goes_in() {
        let mut images = Assets::<Image>::default();
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<PropMaterial>::default();
        let mut bindposes = Assets::<SkinnedMeshInverseBindposes>::default();
        let mut to = LateStores {
            images: &mut images,
            stores: Stores {
                meshes: &mut meshes,
                materials: &mut materials,
                bindposes: &mut bindposes,
                dormant: None,
            },
        };
        let mut stage = Stage::new(Providers::of(&to));
        let h = stage.add_asset(image_with_data(2048));
        assert!(stage.fill(&mut to));
        assert!(to.images.contains(h.id()));
    }
}
