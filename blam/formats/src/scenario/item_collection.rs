//! The item collection tag (`itmc`; specs/halo_ce/scenario.md, "Item
//! collection"): permutations with weights and item references (weapons
//! and equipment), and the collection's own spawn time, plus the spec's
//! weighted draw as a pure function of an integer the caller draws.

use std::io;

use crate::cache::{MapFile, TagEntry, TagRef, TagStruct, class};

/// The item collection's main struct, in bytes.
pub const ITEM_COLLECTION_SIZE: usize = 0x5C;

crate::tag_struct! {
    /// One permutation (84 bytes).
    pub struct ItemPermutation: 0x54 {
        /// Relative chance.
        0x20 weight: f32,
        /// Class `weap` or `eqip` (object tags).
        0x24 item: TagRef,
    }
}

crate::tag_struct! {
    /// The item collection's main struct (0x5C bytes).
    pub struct ItemCollection: 0x5C {
        0x00 permutations: Vec<ItemPermutation>,
        /// Seconds; 0 means unspecified (then 30).
        0x0C spawn_time: i16,
    }
}

impl ItemCollection {
    /// Read the `itmc` tag `tag` of `map`.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<ItemCollection> {
        let v = map.tag_view(tag, ITEM_COLLECTION_SIZE)?;
        if tag.class != class::ITEM_COLLECTION {
            return Err(v.error(
                0,
                format_args!("tag is {} `{}`, not an item collection", tag.class, tag.path),
            ));
        }
        <ItemCollection as TagStruct>::read(&v)
    }

    /// The sum of the weights, truncated to a whole number: the range the
    /// game draws its integer from, `[0, total)`.
    pub fn total_weight(&self) -> u32 {
        let sum: f32 = self.permutations.iter().map(|p| p.weight).sum();
        if sum.is_finite() && sum > 0.0 { sum as u32 } else { 0 }
    }

    /// The permutation an integer draw `r` in `[0, total_weight())` picks:
    /// walk the permutations in order subtracting each weight (as a real
    /// number); the first after which `r` is negative. `None` for a
    /// collection with no permutation, or a draw the weights do not reach.
    pub fn pick(&self, r: u32) -> Option<&ItemPermutation> {
        let mut r = r as f32;
        for p in &self.permutations {
            r -= p.weight;
            if r < 0.0 {
                return Some(p);
            }
        }
        None
    }
}
