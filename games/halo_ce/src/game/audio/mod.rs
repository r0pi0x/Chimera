//! Halo's sounds that last (specs/halo_ce/sounds.md "Looping sounds",
//! "Sound environment, background sound, sound scenery"; effects.md
//! "Material effects"; animations.md "Footsteps"): the looping sound
//! player and its owners, and the Spartan's footsteps. The one-shots of
//! the weapons, impacts and explosions are `fx`'s.
//!
//! - `library`: the sound data compiled from a map file's tags
//!   (`AudioLibrary`: sounds, looping sounds, the projectiles' and
//!   weapons' looping attachments, the footstep sounds, the level's part),
//!   carried in a map entity and its clips in the map's sounds.
//! - `looping`: the player: owners' `LoopRequest`s each frame to the
//!   engine's long-lived sounds (`SoundControl`) and one-shots
//!   (`PlaySound`).
//! - `level`: the level's background sound by the listener's cluster, its
//!   sound environment, its sound scenery.
//! - `attached`: the looping sounds attached to the shown projectiles (the
//!   rocket's flight).
//! - `steps`: the Spartan's footsteps, jumps and landings by the surface's
//!   material.
//!
//! Where the data comes from (`kit`, `dress`): a Halo map's own file at
//! its conversion (`maps::audio`: the kit and the level's part, through the
//! converted-map cache), and on another game's map the donor map's kit
//! (`GUEST`, after `fx`'s guest dressing). Everything here is
//! presentation computed on each machine from what it already has (the
//! map, the replicated players and the shown projectiles): no network
//! message of its own.

pub mod attached;
pub mod level;
pub mod library;
pub mod looping;
pub mod steps;

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use blam_formats::{
    cache::{MapFile, class},
    effect::MaterialEffects,
    object::Biped,
    projectile::Projectile,
    weapon::Weapon,
};

use self::library::{ActiveAudio, AudioSet, Compiler};
use crate::map::{LoadContext, MapData, MapLoadOptions, PlaySound, SoundControl};

/// The footstep kinds played (effects.md, "Material effects").
pub const STEP_KINDS: usize = 6;

/// The kit's part of the library from `file`: the guns' projectiles' and
/// weapons' looping sound attachments, the Spartan's footstep sounds.
pub fn kit(c: &mut Compiler) {
    let file = c.file;
    let guns = super::fx::load::gun_tags();
    let guns: Vec<&str> = guns.iter().map(String::as_str).collect();
    for p in super::fx::load::projectile_tags(file, &guns) {
        let Some(t) = file.find(class::PROJECTILE, &p) else { continue };
        let Ok(proj) = Projectile::read(file, t) else { continue };
        let loops = c.attached_loops(&proj.object.attachments);
        if !loops.is_empty() {
            c.set.library.projectiles.insert(p, loops);
        }
    }
    for g in &guns {
        let Some(t) = file.find(class::WEAPON, g) else { continue };
        let Ok(w) = Weapon::read(file, t) else { continue };
        let loops = c.attached_loops(&w.object.attachments);
        if !loops.is_empty() {
            c.set.library.weapons.insert(g.to_ascii_lowercase(), loops);
        }
    }
    let foot = file
        .find(class::BIPED, super::body::load::BIPED)
        .and_then(|t| Biped::read(file, t).ok())
        .and_then(|b| file.tag(b.own.footsteps.id))
        .and_then(|t| MaterialEffects::read(file, t).ok());
    if let Some(foot) = foot {
        for kind in foot.kinds.iter().take(STEP_KINDS) {
            let row = kind.iter().map(|m| c.sound(&m.sound).unwrap_or_default()).collect();
            c.set.library.footsteps.push(row);
        }
    }
}

/// The donor's kit an app's loads share.
#[derive(Default)]
struct Donor(Mutex<Option<Arc<AudioSet>>>);

/// The donor map's kit for an app (made once per app).
pub fn donor(context: &LoadContext) -> Result<Arc<AudioSet>, String> {
    let memo = context.get_or_init(Donor::default);
    let mut slot = memo.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = &*slot {
        return Ok(s.clone());
    }
    let mount = super::maps::mount_for(context)?;
    let path = super::maps::map_path(super::body::load::DONOR_MAP);
    let bytes = mount.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let file = MapFile::load(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let mut c = Compiler::new(&file);
    kit(&mut c);
    let set = Arc::new(c.set);
    *slot = Some(set.clone());
    Ok(set)
}

/// Halo's sounds on `map` (another game's), from the donor map.
pub fn dress(map: &mut MapData, options: &MapLoadOptions) -> Result<(), String> {
    let set = donor(&options.context)?;
    library::add(map, &set);
    Ok(())
}

/// Halo on the maps of other games: `fx`'s guest dressing (its players'
/// body, its guns' effects), then its kit's sounds.
pub static GUEST: crate::map::MapDressing = crate::map::MapDressing {
    game: super::GAME,
    dress: |map, options| {
        (super::fx::load::GUEST.dress)(map, options)?;
        if let Err(e) = dress(map, options) {
            map.warnings.push(format!("no Halo sounds: {e}"));
        }
        Ok(())
    },
};

/// The order of a frame's audio work: the listener, the owners' requests,
/// the player.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HaloAudioSet {
    Listen,
    Ask,
    Play,
}

/// Halo's lasting sounds (module doc).
pub struct HaloAudioPlugin;

impl Plugin for HaloAudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveAudio>()
            .init_resource::<looping::Loops>()
            .init_resource::<looping::Listener>()
            .init_resource::<level::ListenerZone>()
            .init_resource::<level::ListenerEnvironment>()
            .add_message::<looping::LoopRequest>()
            .add_message::<SoundControl>()
            .add_message::<PlaySound>()
            .configure_sets(Update, (HaloAudioSet::Listen, HaloAudioSet::Ask, HaloAudioSet::Play).chain())
            .add_systems(
                Update,
                (
                    (library::follow, looping::listen, level::environment).chain().in_set(HaloAudioSet::Listen),
                    (level::background, level::scenery, attached::projectiles).in_set(HaloAudioSet::Ask),
                    looping::play.in_set(HaloAudioSet::Play),
                ),
            );
        steps::plugin(app);
    }
}
