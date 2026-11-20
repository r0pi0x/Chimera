//! Drawn loose items: a dropped weapon's meshes (`map::loose::LooseItem`)
//! and an item that moves itself (`map::loose::ShownItem`, e.g. a thrown
//! grenade or the planted bomb, `SitOnOrigin`), each lit where it is
//! (`probe_lit`, its own material instances). The item components are the
//! simulation's requests (a held model's key); the body is
//! `map::loose::attach_loose`'s, the meshes these systems add, from the
//! held models' whole meshes (`render::bodies::HeldAssets`) placed by the
//! loose form's frame (`map::loose::LooseAssets`). A shown item that
//! appears outside the playable area on a map with a 3D skybox is in the
//! skybox (HL2's headcrab canister before it enters the world): drawn on
//! the sky camera's layer, as particles out there are (`render::particles`).
//! A shown item whose held model is still being built (added late,
//! `bodies::build_late_held`) waits undrawn and asks for it first
//! (`bodies::LateWanted::shown`), as a loose item waits.

use bevy::prelude::*;

use super::{bodies::HeldAssets, probe_lit};
use bevy::camera::visibility::RenderLayers;
use crate::map::{
    MapPart, PlayableArea, SKYBOX_LAYER,
    loose::{LooseAssets, LooseItem, ShownItem, SitOnOrigin},
};
use super::sky::ActiveMapHas3dSky;

/// Marks a `LooseItem` whose meshes were added.
#[derive(Component)]
pub struct LooseItemDrawn;

/// Give new loose items their meshes, lit where they lie.
pub fn draw_loose(
    items: Query<(Entity, &LooseItem), Without<LooseItemDrawn>>,
    assets: Option<Res<LooseAssets>>,
    held: Option<Res<HeldAssets>>,
    mut materials: Option<ResMut<Assets<crate::render::prop_material::PropMaterial>>>,
    dormant: Option<Res<crate::render::prop_material::DormantProps>>,
    mut commands: Commands,
) {
    let (Some(assets), Some(held)) = (assets, held) else {
        return;
    };
    for (e, item) in &items {
        // Until its held model is there (one added late:
        // `bodies::build_late_held`), it waits undrawn.
        let (Some(a), Some(parts)) = (assets.0.get(&item.0), held.0.get(&item.0)) else {
            continue;
        };
        // `try_`: an item can be gone by the time this applies (picked up,
        // or a network client's copy despawned, in the same frame).
        commands.entity(e).try_insert(LooseItemDrawn);
        // Its own materials, lit where it lies (probe_lit).
        let own: Vec<_> = parts
            .whole
            .iter()
            .map(|(_, m)| match materials.as_mut() {
                Some(assets) => probe_lit::instance(assets, dormant.as_deref(), m),
                None => m.clone(),
            })
            .collect();
        commands
            .entity(e)
            .try_insert(probe_lit::ProbeLit::new(own.clone(), Vec3::ZERO));
        let parts: Vec<_> = parts.whole.iter().map(|(mesh, _)| mesh.clone()).zip(own).collect();
        let frame = a.frame;
        commands.queue(move |w: &mut World| {
            if w.get_entity(e).is_err() {
                return;
            }
            for (mesh, material) in parts {
                w.spawn((Mesh3d(mesh), MeshMaterial3d(material), frame, ChildOf(e)));
            }
        });
    }
}

/// Marks a `ShownItem` whose meshes were added.
#[derive(Component)]
pub struct ShownItemDrawn;

/// Give new shown items their meshes.
pub fn attach_shown(
    items: Query<(Entity, &ShownItem, Option<&SitOnOrigin>, Option<&Transform>), Without<ShownItemDrawn>>,
    assets: Option<Res<LooseAssets>>,
    held: Option<Res<HeldAssets>>,
    mut materials: Option<ResMut<Assets<crate::render::prop_material::PropMaterial>>>,
    dormant: Option<Res<crate::render::prop_material::DormantProps>>,
    sky: (Option<Res<PlayableArea>>, Option<Res<ActiveMapHas3dSky>>),
    mut wanted: Option<ResMut<super::bodies::LateWanted>>,
    real: Res<Time<Real>>,
    mut commands: Commands,
) {
        let (Some(assets), Some(held)) = (assets, held) else {
        return;
    };
    // Keys shown now whose held model is still on its way (added late:
    // `bodies::build_late_held`, which builds these at once).
    let mut missing = std::collections::BTreeSet::new();
    // Where it appears decides its layer (a shown item outside the
    // playable area stays out there: the skybox's are their own items).
    let in_sky = |t: Option<&Transform>| match (&sky.0, &sky.1, t) {
        (Some(area), Some(_), Some(t)) => t.translation.cmplt(area.0.0).any() || t.translation.cmpgt(area.0.1).any(),
        _ => false,
    };
    for (e, item, sit, at) in &items {
        let layers = in_sky(at).then(|| RenderLayers::layer(SKYBOX_LAYER));
        // Until its held model is there it waits undrawn, as a loose
        // item does: a TF2 sticky fired before the late-built
        // `tf2:sticky` was ready stayed invisible for its whole life.
        let (Some(a), Some(parts)) = (assets.0.get(&item.0), held.0.get(&item.0)) else {
            if !held.0.contains_key(&item.0) {
                missing.insert(item.0.clone());
            }
            continue;
        };
        let mut ent = commands.entity(e);
        ent.try_insert((ShownItemDrawn, MapPart, Visibility::default()));
        {
            let mut frame = a.frame;
            if let Some(SitOnOrigin(turn)) = sit {
                let m = Mat3::from_quat(*turn);
                let half = m.x_axis.abs() * a.half.x + m.y_axis.abs() * a.half.y + m.z_axis.abs() * a.half.z;
                frame = Transform::from_rotation(*turn) * frame;
                frame.translation.y += half.y;
            }
            // Its own materials, lit where it is (probe_lit).
            let own: Vec<_> = parts
                .whole
                .iter()
                .map(|(_, m)| match materials.as_mut() {
                    Some(assets) => probe_lit::instance(assets, dormant.as_deref(), m),
                    None => m.clone(),
                })
                .collect();
            ent.insert(probe_lit::ProbeLit::new(own.clone(), Vec3::ZERO));
            ent.with_children(|c| {
                for ((mesh, _), material) in parts.whole.iter().zip(own) {
                    let mut part = c.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material), frame));
                    if let Some(l) = &layers {
                        part.insert(l.clone());
                    }
                }
            });
        }
    }
    if let Some(w) = wanted.as_mut()
        && (w.shown.len() != missing.len() || !missing.iter().all(|k| w.shown.contains_key(k)))
    {
        let now = real.elapsed_secs_f64();
        w.shown.retain(|k, _| missing.contains(k));
        for k in missing {
            w.shown.entry(k).or_insert(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::bodies::HeldParts;
    use std::collections::HashMap;

    /// An item despawned before its meshes are put in place (picked up, a
    /// network client's copy gone, the same frame) is no error.
    #[test]
    fn an_item_gone_in_the_same_frame_is_no_error() {
        let mut app = App::new();
        app.set_error_handler(bevy::ecs::error::panic);
        let mut assets = HashMap::new();
        // A 0.2 m box with no surface.
        let gun = crate::map::loose::LooseAsset {
            frame: Transform::IDENTITY,
            half: Vec3::splat(0.1),
            surface: None,
            nose: None,
        };
        assets.insert("gun".to_string(), gun);
        app.insert_resource(LooseAssets(assets));
        let mut held = HashMap::new();
        held.insert(
            "gun".to_string(),
            HeldParts {
                bone: String::new(),
                whole: vec![(Handle::default(), Handle::default())],
                own: Vec::new(),
                muzzle: None,
                attachments: Vec::new(),
                merged: Vec::new(),
            },
        );
        app.insert_resource(HeldAssets(held));
        fn take(items: Query<Entity, With<LooseItem>>, mut commands: Commands) {
            for e in &items {
                commands.entity(e).despawn();
            }
        }
        // Both queue their commands; the despawn applies first.
        app.add_systems(Update, (take, draw_loose).chain_ignore_deferred());
        app.world_mut().spawn(LooseItem("gun".into()));
        app.update();
        let w = app.world_mut();
        assert_eq!(w.query::<&LooseItem>().iter(w).count(), 0);
        assert_eq!(w.query::<&Mesh3d>().iter(w).count(), 0, "no orphan parts");
    }

    /// A shown item (a flyer) whose held model isn't built yet waits
    /// undrawn and asks for it (`LateWanted::shown`), and gets its meshes
    /// once it is there; before, it was marked drawn with nothing and
    /// flew unseen for good.
    #[test]
    fn a_shown_item_waits_for_its_late_model() {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin);
        app.insert_resource(LooseAssets(HashMap::new()));
        app.insert_resource(HeldAssets(HashMap::new()));
        app.init_resource::<crate::render::bodies::LateWanted>();
        app.add_systems(Update, attach_shown);
        let e = app.world_mut().spawn((ShownItem("game:sticky".into()), Transform::default())).id();
        app.update();
        assert!(app.world().get::<ShownItemDrawn>(e).is_none(), "waits");
        assert!(app.world().resource::<crate::render::bodies::LateWanted>().shown.contains_key("game:sticky"), "asked for");
        let asset = crate::map::loose::LooseAsset { frame: Transform::IDENTITY, half: Vec3::splat(0.1), surface: None, nose: None };
        app.world_mut().resource_mut::<LooseAssets>().0.insert("game:sticky".into(), asset);
        let parts = HeldParts {
            bone: String::new(),
            whole: vec![(Handle::default(), Handle::default())],
            own: Vec::new(),
            muzzle: None,
            attachments: Vec::new(),
            merged: Vec::new(),
        };
        app.world_mut().resource_mut::<HeldAssets>().0.insert("game:sticky".into(), parts);
        app.update();
        assert!(app.world().get::<ShownItemDrawn>(e).is_some(), "drawn once there");
        assert!(app.world().resource::<crate::render::bodies::LateWanted>().shown.is_empty());
    }
}
