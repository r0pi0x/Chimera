// Adapted from qbsp (MIT, see THIRD_PARTY.md): src/util.rs (VisdataIter, calculate_visdata_indices), src/data/visdata.rs
// Copyright (c) 2026 Noxmore. Kept: walking a run-length encoded row bit
// by bit, a zero byte followed by a count of zero bytes; changed: our
// types, a row's length bounded by the world's leaves, no cluster form.
//! Visibility (specs/goldsrc/bsp.md, "Visibility"): the potentially
//! visible set of a world leaf, decoded from the run-length encoded rows
//! of the visibility lump. Bit k of a row (least significant bit first in
//! each byte) stands for world leaf k + 1; a zero byte and a count stand
//! for that many zero bytes.

use super::Bsp;

/// The world leaves (1..=visleafs) a row marks visible; `row` starts at the
/// leaf's visibility offset. Excess bits past the last leaf are ignored.
pub fn decode_row(row: &[u8], vis_leaves: usize) -> Vec<u32> {
    let mut out = Vec::new();
    let mut leaf = 0usize; // bit index: leaf - 1
    let mut bytes = row.iter();
    while leaf < vis_leaves {
        let Some(&b) = bytes.next() else { break };
        if b == 0 {
            let Some(&n) = bytes.next() else { break };
            leaf += 8 * n as usize;
            continue;
        }
        for bit in 0..8 {
            if b & (1 << bit) != 0 && leaf + bit < vis_leaves {
                out.push((leaf + bit + 1) as u32);
            }
        }
        leaf += 8;
    }
    out
}

impl Bsp {
    /// The world leaves visible from `leaf` (`None`: no row, everything
    /// visible: leaf 0, a brush model's leaf or an unvised map).
    pub fn visible_leaves(&self, leaf: usize) -> Option<Vec<u32>> {
        let l = self.leaves.get(leaf)?;
        if leaf == 0 || l.vis_offset < 0 || self.visibility.is_empty() {
            return None;
        }
        let vis_leaves = self.models.first()?.vis_leaves.max(0) as usize;
        Some(decode_row(self.visibility.get(l.vis_offset as usize..)?, vis_leaves))
    }
}
