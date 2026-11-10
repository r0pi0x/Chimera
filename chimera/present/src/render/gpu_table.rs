//! Per-entity values a shader reads by slot from one storage buffer
//! (`SlotTable`): a moving body's probe light (`probe_lit::ProbeTable`, read
//! by prop.wesl through `PropParams::probe_slot`) and a moving shadow's
//! projection (`shadows::ShadowFrames`, read by shadow.wesl through the
//! entity's `MeshTag`). Values that change every frame live here instead of
//! in a material (a changed material is prepared again: a new uniform buffer
//! and bind group, docs/performance.md, "Render performance") or in a mesh
//! (a rewritten mesh is freed and allocated again in Bevy's mesh slabs): the
//! materials and meshes stay unchanged while their entities move, and the
//! buffer is rewritten in place (Bevy writes a same-sized `ShaderBuffer`
//! into its existing GPU buffer; a grown one is a new buffer, and Bevy then
//! rebuilds the bind groups that hold it).
//!
//! Slot 0 is never handed out: it means "none" to the shaders. The buffer
//! is padded to a power of two of slots (at least 64), so it grows rarely.

use bevy::{asset::RenderAssetUsages, prelude::*, render::storage::ShaderBuffer};

/// Slots the buffer has at least.
const MIN_SLOTS: usize = 64;

/// Fixed-size records of `stride` vec4s each, one per slot, uploaded to the
/// `ShaderBuffer` at `handle` when changed (`upload`).
#[derive(Debug)]
pub struct SlotTable {
    handle: Handle<ShaderBuffer>,
    stride: usize,
    data: Vec<[f32; 4]>,
    free: Vec<u32>,
    dirty: bool,
    /// Slots in the buffer last uploaded (0: none yet).
    uploaded: usize,
    /// How many times the buffer was uploaded (tests, diagnostics).
    pub uploads: u64,
}

impl SlotTable {
    /// A table of `stride` vec4s a slot, uploaded to `handle`.
    pub fn new(handle: Handle<ShaderBuffer>, stride: usize) -> Self {
        Self {
            handle,
            stride,
            // Slot 0: "none".
            data: vec![[0.0; 4]; stride],
            free: Vec::new(),
            dirty: true,
            uploaded: 0,
            uploads: 0,
        }
    }

    pub fn handle(&self) -> &Handle<ShaderBuffer> {
        &self.handle
    }

    /// A slot of its own (zeroed), never 0.
    pub fn alloc(&mut self) -> u32 {
        if let Some(slot) = self.free.pop() {
            return slot;
        }
        let slot = self.data.len() / self.stride;
        self.data.resize(self.data.len() + self.stride, [0.0; 4]);
        self.dirty = true;
        slot as u32
    }

    /// Give `slot` back (0 is ignored).
    pub fn free(&mut self, slot: u32) {
        if slot == 0 || (slot as usize) * self.stride >= self.data.len() || self.free.contains(&slot) {
            return;
        }
        let s = slot as usize * self.stride;
        self.data[s..s + self.stride].fill([0.0; 4]);
        self.free.push(slot);
    }

    /// Slots handed out and not given back.
    pub fn live(&self) -> usize {
        self.data.len() / self.stride - 1 - self.free.len()
    }

    /// Slot `slot`'s record (zeros for one never handed out).
    pub fn get(&self, slot: u32) -> &[[f32; 4]] {
        let s = slot as usize * self.stride;
        self.data.get(s..s + self.stride).unwrap_or(&[])
    }

    /// Write slot `slot`'s record (`values`, at most `stride` vec4s, the
    /// rest zero); marks the table for upload only when it changed.
    pub fn set(&mut self, slot: u32, values: &[[f32; 4]]) {
        let s = slot as usize * self.stride;
        if slot == 0 || s + self.stride > self.data.len() {
            return;
        }
        let row = &mut self.data[s..s + self.stride];
        let mut changed = false;
        for (i, v) in row.iter_mut().enumerate() {
            let new = values.get(i).copied().unwrap_or([0.0; 4]);
            if *v != new {
                *v = new;
                changed = true;
            }
        }
        self.dirty |= changed;
    }

    /// Slots the uploaded buffer holds for `used` slots.
    fn capacity(used: usize) -> usize {
        used.max(MIN_SLOTS).next_power_of_two()
    }

    /// Upload the table if it changed (or the buffer is missing): the whole
    /// buffer, at the same size unless it outgrew it.
    pub fn upload(&mut self, buffers: &mut Assets<ShaderBuffer>) {
        if !self.dirty && buffers.contains(self.handle.id()) {
            return;
        }
        let slots = Self::capacity(self.data.len() / self.stride).max(self.uploaded);
        let mut data = Vec::with_capacity(slots * self.stride);
        data.extend_from_slice(&self.data);
        data.resize(slots * self.stride, [0.0; 4]);
        let _ = buffers.insert(self.handle.id(), ShaderBuffer::new(data, RenderAssetUsages::RENDER_WORLD));
        self.uploaded = slots;
        self.dirty = false;
        self.uploads += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_are_reused_and_zero_is_never_given() {
        let mut t = SlotTable::new(Handle::default(), 2);
        let (a, b) = (t.alloc(), t.alloc());
        assert_eq!((a, b), (1, 2));
        t.set(a, &[[1.0; 4], [2.0; 4]]);
        assert_eq!(t.get(a), &[[1.0; 4], [2.0; 4]]);
        t.free(a);
        assert_eq!(t.get(a), &[[0.0; 4]; 2], "freed: zeroed");
        assert_eq!(t.alloc(), a, "reused");
        t.set(0, &[[9.0; 4]]);
        assert_eq!(t.get(0), &[[0.0; 4]; 2], "slot 0 stays none");
        assert_eq!(t.live(), 2);
    }

    #[test]
    fn uploads_only_changes_at_one_size_until_outgrown() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<ShaderBuffer>();
        let mut buffers = app.world_mut().resource_mut::<Assets<ShaderBuffer>>();
        let mut t = SlotTable::new(Handle::default(), 3);
        t.upload(&mut buffers);
        let size = |b: &Assets<ShaderBuffer>| b.get(&Handle::<ShaderBuffer>::default()).unwrap().buffer_size();
        let first = size(&buffers);
        assert_eq!(first, (MIN_SLOTS * 3 * 16) as u64);
        let s = t.alloc();
        t.upload(&mut buffers);
        assert_eq!(t.uploads, 2, "a new slot");
        t.set(s, &[[1.0; 4]]);
        t.set(s, &[[1.0; 4]]);
        t.upload(&mut buffers);
        t.upload(&mut buffers);
        assert_eq!(t.uploads, 3, "one upload for a change, none without");
        assert_eq!(size(&buffers), first, "the same size: written in place");
        for _ in 0..MIN_SLOTS {
            t.alloc();
        }
        t.upload(&mut buffers);
        assert_eq!(size(&buffers), 2 * first, "outgrown: doubled");
    }
}
