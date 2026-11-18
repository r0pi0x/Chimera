//! The looping sounds attached to Halo's objects in flight and to
//! held guns (specs/halo_ce/objects.md "Attachments": an `lsnd` attachment is a
//! looping sound on the object; sounds.md "Looping sounds"): every shown
//! Halo object of a projectile's key (`fx`'s shown projectiles: the
//! rockets and needles flown again on every machine, the grenades as the
//! server or a client draws them) asks each frame for its projectile's
//! looping sound attachments (`library::AudioLibrary::projectiles`: the
//! rocket's flight) following it; a projectile gone stops asking, so its
//! loop stops by its tracks' rules. Every machine plays what it shows: no
//! message of its own.
//!
//! A held Halo gun's looping sound attachments (`weapons`) play at its
//! holder, scaled by the attachment's function of the gun's state (the
//! plasma pistol's `plasma rifle\charging` by its function D, the primary
//! charge: heard while it charges, silent otherwise), on every machine
//! from the replicated gun state.
//!
//! Known shortcuts: a projectile's attachment's scale is 1 (its function
//! is not evaluated) and every attachment sits at its object's origin
//! (its marker's offset is not applied); a held gun's functions are
//! evaluated afresh each frame (a periodic function stands at time 0).

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

/// The owner kind of the held weapons' loops (`looping::owner_key`).
pub const WEAPON_KIND: u8 = 4;

/// The scale of a weapon loop whose primary scale is `scale` (objects.md
/// "Attachments": 0 none, full; 1 to 4 the outgoing function A to D)
/// for the weapon's functions `outgoing`.
pub fn weapon_loop_scale(scale: i16, outgoing: &[blam_formats::object::function::FunctionOutput; 4]) -> f32 {
    match scale {
        1..=4 => outgoing[(scale - 1) as usize].value.clamp(0.0, 1.0),
        _ => 1.0,
    }
}

/// Each living holder's Halo gun's looping sound attachments
/// (`library::AudioLibrary::weapons`: the plasma pistol's charge hum), at
/// the holder, scaled by the attachment's function of the gun's state
/// (`view::displays`: its exports and object functions); a loop whose
/// scale is 0 isn't asked for, so it stops.
#[allow(clippy::type_complexity)]
pub fn weapons(
    active: Res<ActiveAudio>,
    memos: Option<Res<crate::map::MapMemos>>,
    holders: Query<(Entity, &chimera_sim::weapon::Inventory, &Transform, Option<&crate::core::Health>)>,
    guns: Query<(&chimera_sim::weapon::Weapon, &crate::game::weapons::HaloGun, &crate::game::weapons::HaloWeapon)>,
    mut requests: MessageWriter<LoopRequest>,
) {
    use crate::game::view::displays;
    let Some(lib) = active.0.as_deref() else { return };
    if lib.weapons.is_empty() {
        return;
    }
    let shows = memos.as_deref().and_then(|m| m.get::<displays::GunDisplays>());
    for (e, inv, t, health) in &holders {
        if health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let Some((weapon, state, numbers)) = inv.active.and_then(|w| guns.get(w).ok()) else { continue };
        let Some(loops) = lib.weapons.get(&numbers.0.tag.to_ascii_lowercase()) else { continue };
        let mut functions = displays::Functions::default();
        if let Some(d) = shows
            && let Some(program) = d.guns.get(weapon.id)
        {
            displays::step(program, &|code| displays::export(code, &numbers.0, state), None, &d.tables, &mut functions);
        }
        for (i, l) in loops.iter().enumerate() {
            let scale = weapon_loop_scale(l.scale, &functions.outgoing);
            if scale <= 0.0 {
                continue;
            }
            requests.write(LoopRequest {
                key: owner_key(WEAPON_KIND, (e.to_bits() << 3) | i as u64),
                sound: l.sound.clone(),
                source: LoopSource::Follow(e, t.translation),
                scale,
                alternate: false,
            });
        }
    }
}
