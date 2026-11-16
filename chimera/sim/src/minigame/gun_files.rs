//! Whether a gun's files are here to hold it now (the sandbox's
//! progressive downloads, docs/backlog.md): the seam between a rung switch
//! (`rung_command`, the sandbox's picks) and the asset side above this
//! crate (`chimera_net::net::units` registers its hook), so the switch can
//! wait for a gun's files instead of loading them on the frame.
//!
//! - `GunFiles`: hooks `fn(&mut World, weapon id) -> bool`, each answering
//!   whether its side has the gun's files, and asking for them (in the
//!   background) when it hasn't. None registered: always here.
//! - `gun_ready(world, id)`: every hook says here. Call it each frame
//!   until true, then switch. A download that failed doesn't hold it up
//!   (the gun's files then load on demand, as without the asset side); the
//!   player retries from the Change gun view.

use bevy::prelude::*;

/// The hooks (module doc).
#[derive(Resource, Clone, Default)]
pub struct GunFiles(pub Vec<fn(&mut World, &str) -> bool>);

/// Whether gun `weapon`'s files are here (module doc); asks for them when
/// not.
pub fn gun_ready(world: &mut World, weapon: &str) -> bool {
    let hooks = world.get_resource::<GunFiles>().map(|h| h.0.clone()).unwrap_or_default();
    let mut ready = true;
    for hook in hooks {
        ready &= hook(world, weapon);
    }
    ready
}
