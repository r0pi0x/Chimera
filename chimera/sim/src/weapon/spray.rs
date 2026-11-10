//! Players' sprays for any game (CS:S's `impulse 201`, T): a trace from the
//! eye along the view, up to `SprayRules::reach`, puts the player's logo
//! (its `cl_logofile` userinfo, CS:S's Options > Multiplayer "Spraypaint
//! image") on the static world it hits, as a `map::decal::Spray` on its own
//! entity: one per player (a new one replaces the old), at most one every
//! `decalfrequency` seconds (Source's server cvar, 10 by default), with the
//! game's spray sound from the player (`SprayRules::sound`, heard by all).
//! Sprays stay through round restarts and go with the map (a
//! `map::MapHooks::unload` hook). A network client asks the server
//! (`SprayRequested`); the server's `Spray` entities replicate (`net`) and
//! each client draws them from its own install's logos (`render::spray`),
//! or from the picture a spray carries when the logo is one the player
//! imported (`SprayPictureSource` finds it; the client sends it with its
//! request).
//!
//! Players' sprays (`impulse 201`): the eye's trace up to `SprayRules::reach`
//! onto the static world (not props, brush entities or characters) puts the
//! player's `cl_logofile` logo there as a `map::decal::Spray` (one per player,
//! the new replacing the old; the living only; `decalfrequency` seconds apart,
//! a replicated server cvar, default 10), upright on walls and turned away from
//! the player on floors, with `SprayRules::sound`; kept through rounds, gone
//! with the map; a network client asks the server (`SprayRequested`).
//!
//! Known shortcuts (docs/tech-debt.md): Sprays: the reach (128 units), the size
//! (64 units square, `games/cs_source/decals.rs`) and a floor spray turned with
//! its top away from the player are the well-known Counter-Strike values and
//! ours, not measured; only the static world takes them (no props or brush
//! entities); a refused spray (too soon, nothing in reach) says nothing; the
//! logo travels as its name (`cl_logofile` userinfo), so a logo missing from
//! an install isn't drawn there; an imported picture is reduced to 64 x 64
//! (`map::decal::SPRAY_PICTURE_MAX_SIDE`) and goes whole with each spray, not
//! once per player. Why it's OK for now: Sprays work with the stock logos;
//! capture C23 measures reach and size.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};

use crate::{
    core::{Health, Intent, MovementState, Spectating},
    map::decal::{Spray, SprayPicture},
};

/// `cl_logofile`'s default: CS:S's import slot (Options > Multiplayer >
/// Import Spray writes it).
pub const LOGO_FILE_DEFAULT: &str = "materials/vgui/logos/spray.vtf";
/// The userinfo key a player's logo travels as.
pub const LOGO_KEY: &str = "cl_logofile";

/// A game's spray rules: how far the eye's trace reaches (meters), the
/// sound entry played from the player, and `decalfrequency` (seconds
/// between a player's sprays).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct SprayRules {
    pub reach: f32,
    pub sound: Option<String>,
    pub frequency: f32,
}

impl Default for SprayRules {
    fn default() -> Self {
        Self {
            // UNMEASURED: 128 units, the well-known spray distance.
            reach: 128.0 * 0.0254,
            sound: None,
            // Source's `decalfrequency` default.
            frequency: 10.0,
        }
    }
}

/// The local player's `cl_logofile` (sent as userinfo).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct LogoFile(pub String);

impl Default for LogoFile {
    fn default() -> Self {
        Self(LOGO_FILE_DEFAULT.into())
    }
}

/// When a player last sprayed (simulation seconds).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct LastSpray(pub f64);

/// Finds the picture of a `cl_logofile` value that is the player's own
/// import (a game's front end sets it; None: every logo is the install's).
#[derive(Resource, Clone, Copy, Default)]
pub struct SprayPictureSource(pub Option<fn(&str) -> Option<SprayPicture>>);

/// A network client's `impulse 201`: what it asks the server for (`net`
/// sends it, the server runs `spray_with`), with the picture of an
/// imported logo.
#[derive(Message, Clone, Debug, Default, PartialEq)]
pub struct SprayRequested(pub Option<SprayPicture>);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<SprayRules>()
        .init_resource::<LogoFile>()
        .init_resource::<SprayPictureSource>()
        .add_message::<SprayRequested>();
    crate::console::resource_cvar::<SprayRules, f32>(
        app,
        "decalfrequency",
        "Seconds between a player's sprays (impulse 201; the server's, 10 by default).",
        |r| &mut r.frequency,
    );
    crate::console::resource_cvar::<LogoFile, String>(
        app,
        LOGO_KEY,
        "Your spray logo (impulse 201, T): a file under materials/vgui/logos/, e.g. materials/vgui/logos/spray_kilroy.vtf \
         (sent to the server as userinfo).",
        |l| &mut l.0,
    );
    {
        let mut console = app.world_mut().resource_mut::<crate::console::Console>();
        console.archive(LOGO_KEY);
        console.userinfo(LOGO_KEY);
    }
    app.init_resource::<crate::map::MapHooks>();
    app.world_mut()
        .resource_mut::<crate::map::MapHooks>()
        .unload
        .push(clear_sprays);
}

/// The logo a `cl_logofile` value names: the file's name under
/// `materials/vgui/logos/` without its extension (`spray_kilroy`), lower
/// case; None for anything else.
pub fn logo_name(file: &str) -> Option<String> {
    let file = file.trim().replace('\\', "/").to_lowercase();
    let name = file
        .strip_prefix("materials/")
        .unwrap_or(&file)
        .strip_prefix("vgui/logos/")?
        .trim_end_matches(".vtf");
    let ok =
        !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    ok.then(|| name.to_string())
}

/// `impulse 201` for `player`: on a network client a request to the server
/// (`SprayRequested`), else `spray`.
pub fn spray_or_ask(world: &mut World, player: Entity) -> Result<Option<String>, String> {
    if world.get_resource::<crate::core::NetRole>() == Some(&crate::core::NetRole::Client) {
        let file = world.get_resource::<LogoFile>().map(|l| l.0.clone()).unwrap_or_default();
        let picture = picture_of(world, &file);
        world.write_message(SprayRequested(picture));
        return Ok(None);
    }
    spray(world, player).map(Some)
}

/// The picture of `file` if it is an import (`SprayPictureSource`).
fn picture_of(world: &World, file: &str) -> Option<SprayPicture> {
    let source = world.get_resource::<SprayPictureSource>()?.0?;
    source(file).filter(SprayPicture::valid)
}

/// Spray `player`'s logo where it looks, if it may (see `spray_with`); the
/// picture of an imported logo is found here (for a player on this machine).
pub fn spray(world: &mut World, player: Entity) -> Result<String, String> {
    let file = world
        .get::<crate::core::UserInfo>(player)
        .and_then(|u| u.0.get(LOGO_KEY).cloned())
        .unwrap_or_else(|| LOGO_FILE_DEFAULT.into());
    let picture = picture_of(world, &file);
    spray_with(world, player, picture)
}

/// Spray `player`'s logo where it looks, if it may (alive, `decalfrequency`
/// since its last, a static surface in reach), with `picture` when the logo
/// is an import (a client's, as it sent it: refused if out of range).
/// Returns what happened.
pub fn spray_with(world: &mut World, player: Entity, picture: Option<SprayPicture>) -> Result<String, String> {
    if picture.as_ref().is_some_and(|p| !p.valid()) {
        return Err("bad spray picture".into());
    }
    let rules = world.get_resource::<SprayRules>().cloned().unwrap_or_default();
    let alive = world.get::<Health>(player).is_some_and(|h| h.current > 0.0);
    if !alive || world.get::<Spectating>(player).is_some() {
        return Err("only the living spray".into());
    }
    let now = world
        .get_resource::<crate::core::SimClock>()
        .map(|c| c.now)
        .unwrap_or_else(|| world.resource::<Time>().elapsed_secs_f64());
    if world
        .get::<LastSpray>(player)
        .is_some_and(|l| now - l.0 < rules.frequency as f64)
    {
        return Err("too soon to spray again".into());
    }
    let file = world
        .get::<crate::core::UserInfo>(player)
        .and_then(|u| u.0.get(LOGO_KEY).cloned())
        .unwrap_or_else(|| LOGO_FILE_DEFAULT.into());
    let logo = logo_name(&file).ok_or_else(|| format!("no spray logo {file}"))?;
    let (Some(t), Some(look)) = (
        world.get::<Transform>(player).copied(),
        world.get::<Intent>(player).map(|i| i.look_rotation()),
    ) else {
        return Err("nobody to spray".into());
    };
    let eye = t.translation + world.get::<MovementState>(player).map_or(Vec3::ZERO, |s| s.eye_offset);
    let forward = look * Vec3::NEG_Z;
    let (point, normal) = trace(world, player, eye, forward, rules.reach).ok_or("nothing in reach to spray on")?;
    // Upright on walls; on floors and ceilings its top points away from
    // the player.
    let up = if normal.y.abs() < 0.7 {
        Vec3::Y.reject_from(normal).normalize_or_zero()
    } else {
        forward.reject_from(normal).normalize_or(Vec3::X)
    };
    // One per player: the new one replaces the old.
    let old: Vec<Entity> = world
        .query::<(Entity, &Spray)>()
        .iter(world)
        .filter(|(_, s)| s.owner == player)
        .map(|(e, _)| e)
        .collect();
    for e in old {
        world.despawn(e);
    }
    world.spawn((
        Name::new("Spray"),
        Spray {
            owner: player,
            logo: logo.clone(),
            point,
            normal,
            up,
            picture,
        },
        Transform::default(),
        Visibility::default(),
    ));
    world.entity_mut(player).insert(LastSpray(now));
    super::economy::owner_sound(world, player, rules.sound);
    Ok(format!("sprayed {logo}"))
}

/// The static world surface the eye's ray meets first within `reach`
/// (characters, props and brush entities stop it but take no spray).
fn trace(world: &mut World, player: Entity, eye: Vec3, dir: Vec3, reach: f32) -> Option<(Vec3, Vec3)> {
    let dir = Dir3::new(dir).ok()?;
    let mut state: SystemState<(
        SpatialQuery,
        Query<&RigidBody>,
        Query<&ChildOf>,
        Query<
            (),
            Or<(
                With<crate::map::PropIndex>,
                With<crate::map::MapBrushEntity>,
                With<Intent>,
            )>,
        >,
    )> = SystemState::new(world);
    let (spatial, bodies, parents, movable) = state.get(world).ok()?;
    let filter = SpatialQueryFilter::default()
        .with_mask(crate::core::SOLID_LAYERS)
        .with_excluded_entities([player]);
    let hit = spatial.cast_ray(eye, dir, reach, true, &filter)?;
    let chain = || std::iter::once(hit.entity).chain(parents.iter_ancestors(hit.entity));
    let fixed = bodies.get(hit.entity).is_ok_and(|b| *b == RigidBody::Static);
    if !fixed || chain().any(|e| movable.contains(e)) || hit.normal.dot(*dir) > -0.1 {
        return None;
    }
    Some((eye + *dir * hit.distance, hit.normal.normalize_or_zero()))
}

/// Every spray goes with the map (`map::MapHooks::unload`).
pub fn clear_sprays(world: &mut World) {
    let all: Vec<Entity> = world.query_filtered::<Entity, With<Spray>>().iter(world).collect();
    for e in all {
        world.despawn(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_names_from_logo_files() {
        assert_eq!(
            logo_name("materials/vgui/logos/spray_kilroy.vtf").as_deref(),
            Some("spray_kilroy")
        );
        assert_eq!(logo_name("Materials\\VGUI\\logos\\Spray.vtf").as_deref(), Some("spray"));
        assert_eq!(logo_name("vgui/logos/spray_nowar").as_deref(), Some("spray_nowar"));
        assert_eq!(logo_name("materials/vgui/logos/ui/spray.vtf"), None);
        assert_eq!(logo_name("materials/../cfg/config.cfg"), None);
        assert_eq!(logo_name(""), None);
    }
}
