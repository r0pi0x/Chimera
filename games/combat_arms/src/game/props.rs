//! Models placed in Combat Arms' maps: world objects that name a model
//! file (`Filename`, e.g. the capture-the-flag `FlagItem`s,
//! specs/combat_arms/world_dat.md "How common things are expressed") become
//! Chimera props (`MapProp`) of the model converted by `model`, skinned
//! with the object's `Skin0`..`Skin7` textures, each piece drawn
//! in its render style (the piece's render-style index into the object's
//! `RenderStyle0`..`RenderStyle7` list, `render_style`: blended glass,
//! added or glowing pieces), lit by the world's light grid at their
//! position, and playing their first animation
//! (`CaPropsPlugin` gives each animated prop a `PropSequence`; Chimera's
//! `map::props::animate_props` plays it).
//!
//! In the Steam install the only such objects are the flags of the
//! capture-the-flag maps, whose model (`ATTACH_M\flag01_ground.ltb`) is
//! in a V2-keyed archive: without the V2 key they are skipped with a map
//! warning. `place` puts any model in a map (the `CHIMERA_CA_PROP`
//! development option of the app, docs/OBSERVABILITY.md).
//!
//! The stock maps' only such objects, the flags, name
//! `RS\EdgeAmp01.ltb` (not in our style table: opaque, the default).
//! World models (brush geometry) are drawn by their object's `BlendMode`
//! (`world`), not here.
//!
//! Known shortcuts (docs/tech-debt.md): the object's `Scale`,
//! `MoveToFloor`, `Rotate` (spinning pickups), `RenderStyle` (a number;
//! its meaning isn't specced) and game type are ignored (flags of every mode are placed; `rules::ctf` hides
//! them outside capture the flag and moves them with the flag's state);
//! props don't collide (`PropSolid::None`; pickups are the game modes'
//! item); the light probe is the light-grid sample nearest the object,
//! the same from every direction.

use std::sync::Arc;

use bevy::prelude::*;
use lithtech_formats::world::{self as lt, Object, World};

use super::{
    model::{self, LoadedModel},
    render_style,
};
use crate::map::{LightProbe, MapData, MapEntity, MapProp, PropRig, PropSequence, PropSolid};

/// Animated props start playing their first animation.
pub struct CaPropsPlugin;

impl Plugin for CaPropsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, start_props);
    }
}

/// Give each animated prop without one a `PropSequence` playing its first
/// sequence (LithTech props loop their animation from the start).
pub fn start_props(props: Query<(Entity, &PropRig), Without<PropSequence>>, mut commands: Commands) {
    for (e, rig) in &props {
        let Some(first) = rig.animator.set.sequences.first() else { continue };
        commands.entity(e).insert(PropSequence {
            play: None,
            default: first.name.clone(),
            round: 0,
        });
    }
}

/// The model file an object names (`Filename` ending in `.ltb`).
pub fn model_file(o: &Object) -> Option<&str> {
    o.string("Filename").filter(|f| f.to_lowercase().ends_with(".ltb"))
}

/// The object's skin list: `Skin0`..`Skin7` (empty strings kept, so
/// indices stay), or `Skin` alone when those are all empty.
pub fn skins(o: &Object) -> Vec<String> {
    let list: Vec<String> = (0..8)
        .map(|i| o.string(&format!("Skin{i}")).unwrap_or("").to_string())
        .collect();
    if list.iter().all(String::is_empty) {
        return o.string("Skin").map(|s| vec![s.to_string()]).unwrap_or_default();
    }
    list
}

/// The object's render-style list: `RenderStyle0`..`RenderStyle7`
/// (empty strings kept, so indices stay).
pub fn render_styles(o: &Object) -> Vec<String> {
    (0..8)
        .map(|i| o.string(&format!("RenderStyle{i}")).unwrap_or("").to_string())
        .collect()
}

/// An object's orientation in the engine's frame (the file's rotation
/// mirrored: S R S).
pub fn rotation(o: &Object) -> Quat {
    let [p, y, r] = o.rotation("Rotation").unwrap_or([0.0; 3]);
    let [right, up, forward] = lt::rotation_axes(p, y, r).map(|v| Vec3::from(lt::to_right_handed(v)));
    Quat::from_mat3(&Mat3::from_cols(right, up, -forward)).normalize()
}

/// The light grid's sample nearest `pos` (file space) as a probe (linear,
/// lightmap units), or None without a grid.
pub fn light_at(world: &World, pos: lt::Vec3) -> Option<LightProbe> {
    let g = &world.light_grid;
    if g.counts.contains(&0) || g.samples.is_empty() {
        return None;
    }
    let index = |a: usize| {
        let c = if g.cell[a] > 0.0 { ((pos[a] - g.origin[a]) / g.cell[a]).round() } else { 0.0 };
        c.clamp(0.0, (g.counts[a] - 1) as f32) as u32
    };
    let rgb = g.sample(index(0), index(1), index(2));
    let c = Vec3::from(rgb.map(|b| Color::srgb_u8(b, b, b).to_linear().red));
    Some(LightProbe {
        cube: [c; 6],
        lights: Vec::new(),
    })
}

/// Put `loaded` in `map` at `translation` (metres, engine frame) turned by
/// `rotation`, its pieces textured from `skins` (by each piece's first
/// texture index) and drawn in the render styles of `styles` (by each
/// piece's render-style index), as an entity prop of class `class` (so
/// it can animate). Returns the prop's index.
pub fn place(
    map: &mut MapData,
    loaded: &LoadedModel,
    skins: &[String],
    styles: &[String],
    class: &str,
    translation: Vec3,
    rotation: Quat,
    lighting: Option<LightProbe>,
    scale: f32,
    file: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> usize {
    let mut m = model::convert(loaded, scale, &|lod| {
        let i = lod.used_textures().first().copied().unwrap_or(-1);
        usize::try_from(i)
            .ok()
            .and_then(|i| skins.get(i))
            .map(|s| model::normalize(s))
            .unwrap_or_default()
    });
    for (mesh, lod) in m.meshes.iter_mut().zip(model::drawn_lods(&loaded.model)) {
        render_style::apply(mesh, render_style::of_list(styles, lod.render_style));
    }
    super::world::model_textures(&mut m.meshes, map, file);
    map.models.push(m);
    map.entities.push(MapEntity {
        keyvalues: vec![
            ("classname".into(), class.into()),
            ("model".into(), loaded.path.clone()),
        ],
        ..default()
    });
    map.props.push(MapProp {
        model: map.models.len() - 1,
        pose: None,
        ragdoll: None,
        translation,
        rotation,
        solid: PropSolid::None,
        skybox: false,
        lighting: Some(lighting.unwrap_or(LightProbe {
            cube: [Vec3::splat(0.5); 6],
            lights: Vec::new(),
        })),
        vertex_light: None,
        casts_shadow: loaded.model.shadow_enabled(),
        physics: None,
        fade: None,
        parent: None,
        entity: Some(map.entities.len() - 1),
        skin: 0,
        body: 0,
        detail: false,
    });
    map.props.len() - 1
}

/// Every visible object of `world` that names a model, placed in `map`
/// (models that don't read, e.g. in keyed archives, are skipped with one
/// warning each).
pub fn add_objects(world: &World, map: &mut MapData, scale: f32, file: &dyn Fn(&str) -> Option<Vec<u8>>) {
    let mut cache: Vec<(String, Option<Arc<LoadedModel>>)> = Vec::new();
    for o in &world.objects {
        let Some(path) = model_file(o) else { continue };
        if o.bool("Visible") == Some(false) {
            continue;
        }
        let key = model::normalize(path);
        let loaded = match cache.iter().find(|(k, _)| *k == key) {
            Some((_, l)) => l.clone(),
            None => {
                let l = match model::load(&key, file) {
                    Ok(l) => Some(Arc::new(l)),
                    Err(e) => {
                        map.warnings.push(format!("{} model: {e}", o.class));
                        None
                    }
                };
                cache.push((key, l.clone()));
                l
            }
        };
        let Some(loaded) = loaded else { continue };
        let pos = o.pos().unwrap_or([0.0; 3]);
        let at = Vec3::from(lt::to_right_handed(pos)) * scale;
        place(
            map,
            &loaded,
            &skins(o),
            &render_styles(o),
            &o.class,
            at,
            rotation(o),
            light_at(world, pos),
            scale,
            file,
        );
        // The object's name and team, so a game mode finds its prop (a
        // flag it moves and hides: `rules::ctf`).
        if let Some(e) = map.entities.last_mut() {
            for key in ["Name", "Teams", "GameType"] {
                if let Some(v) = o.string(key) {
                    e.keyvalues.push((key.to_string(), v.to_string()));
                }
            }
        }
    }
}

/// The development option that puts a model in front of the first spawn
/// point: `CHIMERA_CA_PROP=<model path>[,<skin>...][;<render style>...]`
/// (archive paths, e.g. `props/headquaters/hq_flag01.ltb`; render styles
/// by index, e.g. `;RS\default.ltb,RS\glass.ltb`),
/// docs/OBSERVABILITY.md.
pub const DEV_PROP_VAR: &str = "CHIMERA_CA_PROP";

/// Metres ahead of the first spawn point the development prop stands.
const DEV_PROP_DISTANCE: f32 = 3.0;

/// `DEV_PROP_VAR`'s model, `DEV_PROP_DISTANCE` ahead of the first spawn
/// point at its feet, its front toward it (a warning when it doesn't
/// read).
pub fn dev_prop(world: &World, map: &mut MapData, spec: &str, scale: f32, file: &dyn Fn(&str) -> Option<Vec<u8>>) {
    let (spec, styles) = spec.split_once(';').unwrap_or((spec, ""));
    let mut parts = spec.split(',').map(str::trim).filter(|s| !s.is_empty());
    let Some(path) = parts.next() else { return };
    let skins: Vec<String> = parts.map(String::from).collect();
    let styles: Vec<String> = styles.split(',').map(|s| s.trim().to_string()).collect();
    let loaded = match model::load(path, file) {
        Ok(l) => l,
        Err(e) => {
            map.warnings.push(format!("{DEV_PROP_VAR}: {e}"));
            return;
        }
    };
    let feet = map.spawns.first().map_or(Vec3::ZERO, |s| s.0);
    let yaw = map.spawn_yaws.first().copied().unwrap_or(0.0);
    // An `Intent` yaw: 0 faces -Z.
    let at = feet + Quat::from_rotation_y(yaw) * Vec3::NEG_Z * DEV_PROP_DISTANCE;
    let light = light_at(world, lt::to_right_handed((at / scale).to_array()));
    // Its front (LithTech +Z, ours -Z) toward the spawn.
    let rotation = Quat::from_rotation_y(yaw + std::f32::consts::PI);
    let n = place(map, &loaded, &skins, &styles, "chimera_dev_prop", at, rotation, light, scale, file);
    info!("{DEV_PROP_VAR}: {} as prop {n} at {at}", loaded.path);
}
