//! The looping sounds attached to Halo's objects in flight
//! (specs/halo_ce/objects.md "Attachments": an `lsnd` attachment is a
//! looping sound on the object; sounds.md "Looping sounds"): every shown
//! Halo object of a projectile's key (`fx`'s shown projectiles: the
//! rockets and needles flown again on every machine, the grenades as the
//! server or a client draws them) asks each frame for its projectile's
//! looping sound attachments (`library::AudioLibrary::projectiles`: the
//! rocket's flight) following it; a projectile gone stops asking, so its
//! loop stops by its tracks' rules. Every machine plays what it shows: no
//! message of its own.
//!
//! Known shortcuts: an attachment's scale is 1 (its function is not
//! evaluated) and it sits at the object's origin (its marker's offset is
//! not applied); the weapons' looping attachments are compiled
//! (`AudioLibrary::weapons`) but not played: none of the nine guns'
//! weapon tags on the disc has one that a held weapon's state drives here.

use bevy::prelude::*;

use super::{
    library::ActiveAudio,
    looping::{LoopRequest, LoopSource, owner_key},
};
use crate::map::loose::ShownItem;

/// The owner kind of the projectiles' loops (`looping::owner_key`).
pub const PROJECTILE_KIND: u8 = 3;

/// Each shown projectile's looping sounds (module doc).
pub fn projectiles(active: Res<ActiveAudio>, shown: Query<(Entity, &ShownItem, &Transform)>, mut requests: MessageWriter<LoopRequest>) {
    let Some(lib) = active.0.as_deref() else { return };
    if lib.projectiles.is_empty() {
        return;
    }
    for (e, item, t) in &shown {
        let Some(tag) = item.0.strip_prefix(super::super::view::load::OBJECT_PREFIX) else { continue };
        let Some(loops) = lib.projectiles.get(tag) else { continue };
        for (i, sound) in loops.iter().enumerate() {
            requests.write(LoopRequest {
                key: owner_key(PROJECTILE_KIND, (e.to_bits() << 3) | i as u64),
                sound: sound.clone(),
                source: LoopSource::Follow(e, t.translation),
                scale: 1.0,
                alternate: false,
            });
        }
    }
}
