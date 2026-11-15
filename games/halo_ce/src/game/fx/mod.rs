//! Halo's weapon sounds and effects (specs/halo_ce/effects.md, sounds.md,
//! projectiles.md; M2 and M3 of docs/plans/active/halo-ce.md): the nine
//! guns, the grenades, their explosions and the items heard and seen on
//! every machine, on Halo's maps and any other.
//!
//! - `load`: the library (`HaloFx`) compiled from the donor map's tags
//!   once per app: the four weapons' firing, empty, misfire and impact
//!   effects, their particles, decals, lights, sounds (decoded as the
//!   engine's sound entries, `halo_ce:<tag path>`), the first-person
//!   graph's sound frames, the muzzle markers and the tracer and bolt
//!   looks; `dress` adds it to a map (its textures, sound clips, particle
//!   materials and decal groups to the map's, the library to its memos).
//! - `sound`: Halo's distance law on the engine's `SoundLevel` (`level`)
//!   and which sound an event plays (`event_sound`).
//! - `play`: an effect's life (events, delays, durations, parts made at
//!   once, particle entries spread by their distribution) at a set of
//!   location frames, turned into the engine's requests: `PlaySound`, the
//!   particle pool's groups, `PlaceDecal`, `HaloLight`.
//! - `shots`: what a shot shows: the firing effect at the muzzle, the
//!   muzzle light, the kick, the visible flight of each projectile (bolt,
//!   tracer, pellets) re-simulated from the shot on every machine (the
//!   server sends others' shots, `HaloShotNet`), its impacts by the
//!   surface's material; the authority's decals from the real impacts.
//! - `place`: where the effects go: the view model's animated markers for
//!   the shooter, the held model's for everyone else (`Placer`).
//! - `objects`: the thrown grenades (the server's, replicated), the
//!   projectiles' lights and attached effects in flight, detonations and a
//!   plasma grenade's countdown on every machine, items lying on the map
//!   and their pickup sound.
//!
//! Known shortcuts: see `play`, `shots`, `sound` and `load`; the full-body
//! shield shader (the shield's glow over the body) is later work, only the
//! impact's shield flare effect is drawn; lens flares are not drawn.

pub mod load;
pub mod objects;
pub mod place;
pub mod play;
pub mod shots;
pub mod sound;

use std::{collections::BTreeMap, sync::Arc};

use bevy::prelude::*;

pub use load::HaloFx;

/// A dynamic light an effect asks for (the presentation makes it: `ui`):
/// where (engine space), its colour at full intensity (linear RGB), its
/// radius (metres), how long (seconds), fading linearly to black.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct HaloLight {
    pub at: Vec3,
    pub color: Vec3,
    pub radius: f32,
    pub seconds: f32,
    /// Its fall-off's transition function (`load::FxLight::falloff`): the
    /// intensity is 1 minus the function of the elapsed fraction.
    pub falloff: i16,
    /// Whose effect made it (the shooter, an impact's shooter).
    pub owner: Option<Entity>,
    /// It goes with its owner (the muzzle light: a child of the shooter).
    pub follow: bool,
}

/// The library while a map plays (from its memos, or set by a test).
#[derive(Resource, Clone, Debug, Default)]
pub struct ActiveFx(pub Option<Arc<HaloFx>>);

/// The library of the map now loaded, from its memos.
fn take_library(memos: Option<Res<crate::map::MapMemos>>, mut active: ResMut<ActiveFx>) {
    let Some(memos) = memos else { return };
    if !memos.is_changed() {
        return;
    }
    if let Some(fx) = memos.get::<Arc<HaloFx>>() {
        active.0 = Some(fx.clone());
    }
}

/// Halo's sounds and effects: the library, the shots' presentation and
/// their network part.
pub struct HaloFxPlugin;

impl Plugin for HaloFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveFx>()
            .init_resource::<play::Effects>()
            .init_resource::<crate::map::particles::Particles>()
            .add_message::<HaloLight>()
            .add_message::<crate::map::PlaySound>()
            .add_message::<crate::map::PlaceDecal>()
            .add_systems(PreUpdate, take_library);
        shots::plugin(app);
        objects::plugin(app);
    }
}

/// Lower-case `halo_ce:` names of tag paths (sound entries, decal groups,
/// particle materials).
pub fn name_of(path: &str) -> String {
    format!("{}:{}", super::GAME, path.to_ascii_lowercase())
}

/// Effects by tag path, for tools and tests.
pub type ByPath = BTreeMap<String, usize>;
