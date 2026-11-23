//! The actor variant tag (`actv`; specs/halo_ce/characters.md, "Where the
//! numbers live in tags this spec adds", and change_colors.md section 1
//! step 4): the fields a campaign character's look takes from its rank,
//! read as `ActorVariant`: the unit it drives, its forced shader
//! permutation and its change colour block (each entry a lower and an
//! upper RGB bound; one entry where present on the disc, slot A).
//!
//! - `ActorVariant::read(map, tag)`; `rank_name(path, unit_path)` gives the
//!   rank a variant's path names (`grunt major` of `grunt`: `major`).
//!
//! Known shortcuts:
//! - Only these fields are read; the AI's (actor, movement, weapon,
//!   grenades, vitality) are not, as nothing here uses them yet.

use std::io;

use crate::cache::{MapFile, RealRgb, TagEntry, TagRef, TagStruct, class};

/// The actor variant's main struct size.
pub const ACTV_SIZE: usize = 0x238;

/// At most this many change colours are applied (slots A to D).
pub const MAX_CHANGE_COLORS: usize = 4;

crate::tag_struct! {
    /// An actor variant's change colour (element 0x20): the bounds a slot is
    /// blended between at a random factor when the actor is created; the
    /// last 8 bytes are unused.
    pub struct ActorChangeColor: 0x20 {
        0x00 color_lower_bound: RealRgb,
        0x0C color_upper_bound: RealRgb,
    }
}

crate::tag_struct! {
    /// The fields of an actor variant this crate reads (absolute offsets).
    pub struct ActorVariant: 0x238 {
        0x000 flags: u32,
        /// Class `actr`.
        0x004 actor: TagRef,
        /// Class `unit` (a biped on the disc's characters).
        0x014 unit: TagRef,
        /// Class `actv`: what a squad's "major upgrade" swaps in.
        0x024 major_upgrade: TagRef,
        /// When not 0, every shader of the model draws this permutation
        /// (the Elite's rank sheen).
        0x20C forced_shader_permutation: i16,
        /// Slot i of the unit's change colours for entry i (at most 4).
        0x22C change_colors: Vec<ActorChangeColor>,
    }
}

impl ActorVariant {
    /// Read actor variant `tag`.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<ActorVariant> {
        if tag.class != class::ACTOR_VARIANT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {} `{}` is not an actor variant", map.header().name, tag.class, tag.path),
            ));
        }
        let v = map.tag_view(tag, ACTV_SIZE)?;
        let mut a = <ActorVariant as TagStruct>::read(&v)?;
        a.change_colors.truncate(MAX_CHANGE_COLORS);
        Ok(a)
    }
}

/// The rank a variant's path names, from its last part and its unit's:
/// what follows the unit's name (`characters\grunt\grunt major` of unit
/// `characters\grunt\grunt`: `major`), or the variant's own last part when
/// it does not start with the unit's; lower case, spaces kept.
pub fn rank_name(path: &str, unit_path: &str) -> String {
    let last = |p: &str| p.rsplit(['\\', '/']).next().unwrap_or(p).to_ascii_lowercase();
    let (own, unit) = (last(path), last(unit_path));
    match own.strip_prefix(&unit) {
        Some(rest) if rest.starts_with(' ') => rest.trim().to_string(),
        _ => own,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rank_is_what_follows_the_units_name() {
        assert_eq!(rank_name(r"characters\grunt\grunt major", r"characters\grunt\grunt"), "major");
        assert_eq!(rank_name(r"characters\grunt\grunt", r"characters\grunt\grunt"), "grunt");
        assert_eq!(rank_name(r"characters\grunt\gruntle", r"characters\grunt\grunt"), "gruntle");
    }
}

