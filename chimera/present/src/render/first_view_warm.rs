//! A map's first view warmed behind a cover (docs/performance.md, "A map's
//! first view"): while a map settles behind a party app's loading screen
//! the client's staging draws the whole map from above with a copy of a
//! world camera (`WarmFirstView`, its entity), so every mesh's pipeline
//! compiles there, not as the player first looks at it. What that view
//! can't show is drawn in front of it besides:
//!
//! - **Props** (`warm_props`): a prop with a fade band is culled beyond its
//!   far distance from each view (Bevy's `VisibilityRange`), a moving one
//!   with a fade distance hidden beyond it from the player's eye
//!   (`map::vis::FadeDistance`), so from above they weren't drawn (TF2's
//!   ctf_2fort: 61 visible props' pipelines compiling for 14 frames as the
//!   cover went on the host, 28 on a client). One copy of each kind of
//!   prop mesh (its vertex layout, or the mesh where only the GPU has it;
//!   its material's pipeline key, its shadow flags, skin and morphs, its
//!   fade band) a few meters in front of the camera, its range kept (the
//!   range's dithered pipeline).
//! - **Bodies and held models** (`warm_models`): a body nobody wears yet
//!   and a gun nobody holds yet are built but not in the world, so
//!   nothing drew them while the map settled: a match whose first rung
//!   put on another game's body (TF2's classes, DoD:S's: skinned meshes
//!   of another layout than the base game's soldiers) compiled that
//!   pipeline on the first frames after the cover (a joined client's first
//!   shown frame with one `PropMaterial` pipeline pending). One part of
//!   each kind of built body part and held model part beside the props.
//! - **View models**: `view_model::warm_view_models`, under each view
//!   model camera.

use std::collections::HashSet;

use bevy::{
    camera::visibility::{NoFrustumCulling, RenderLayers, VisibilityRange},
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{morph::MeshMorphWeights, skinning::SkinnedMesh},
    prelude::*,
};

use super::prop_material::{DormantProps, PropMaterial, prop};

/// A map settling behind a cover: the camera drawing it whole from above
/// (the client's `staging::OverviewCamera`), in while it is.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WarmFirstView(pub Entity);

/// The copies `warm_props` draws (and their root on the camera).
#[derive(Component)]
pub struct WarmCopy;

/// One copy of each kind of prop mesh, in front of the overview camera
/// (module doc), made again when the props change in number; gone with
/// the camera.
#[allow(clippy::type_complexity)]
pub fn warm_props(
    warm: Option<Res<WarmFirstView>>,
    props: Query<
        (
            &Mesh3d,
            &MeshMaterial3d<PropMaterial>,
            Option<&VisibilityRange>,
            Has<NotShadowCaster>,
            Has<NotShadowReceiver>,
            Option<&RenderLayers>,
            (Option<&SkinnedMesh>, Option<&MeshMorphWeights>),
        ),
        Without<WarmCopy>,
    >,
    materials: Option<Res<Assets<PropMaterial>>>,
    meshes: Option<Res<Assets<Mesh>>>,
    dormant: Option<Res<DormantProps>>,
    mut made: Local<Option<(Entity, usize, Entity)>>,
    mut commands: Commands,
) {
    use bevy::pbr::Material;
    let (Some(warm), Some(materials)) = (warm, materials) else {
        *made = None;
        return;
    };
    let count = props.iter().len();
    if made.is_some_and(|(camera, n, _)| camera == warm.0 && n == count) {
        return;
    }
    if let Some((_, _, root)) = made.take() {
        commands.entity(root).try_despawn();
    }
    let world_layer = RenderLayers::default();
    let mut kinds = HashSet::new();
    let root = commands
        .spawn((
            Name::new("Props warming"),
            WarmCopy,
            Transform::from_xyz(0.0, 0.0, -4.0),
            Visibility::Visible,
            ChildOf(warm.0),
        ))
        .id();
    let mut copies = 0;
    for (mesh, material, range, no_cast, no_receive, layers, (skin, morphs)) in &props {
        if layers.is_some_and(|l| !l.intersects(&world_layer)) {
            continue;
        }
        let Some(m) = prop(&materials, dormant.as_deref(), &material.0) else { continue };
        // Its vertex layout where the mesh is still here (a mesh only on the
        // GPU: the mesh itself).
        let layout = meshes.as_deref().and_then(|m| m.get(&mesh.0)).and_then(|m| {
            let ids: Vec<_> = m.try_attributes().ok()?.map(|(a, _)| a.id).collect();
            Some((ids, m.primitive_topology(), m.has_morph_targets()))
        });
        let kind = (
            layout.is_none().then(|| mesh.0.id()),
            layout,
            range.is_some(),
            std::mem::discriminant(&m.alpha_mode()),
            m.double_sided,
            m.cull_front,
            no_cast,
            no_receive,
            skin.is_some(),
            morphs.is_some(),
        );
        if !kinds.insert(kind) {
            continue;
        }
        let mut e = commands.spawn((
            WarmCopy,
            mesh.clone(),
            material.clone(),
            NoFrustumCulling,
            Transform::default(),
            Visibility::Inherited,
            ChildOf(root),
        ));
        if let Some(range) = range {
            e.insert(range.clone());
        }
        if no_cast {
            e.insert(NotShadowCaster);
        }
        if no_receive {
            e.insert(NotShadowReceiver);
        }
        // A skinned or morphed mesh's pipeline wants its skin and weights
        // (the original's joints: the copy is never seen).
        if let Some(skin) = skin {
            e.insert(skin.clone());
        }
        if let Some(morphs) = morphs {
            e.insert(morphs.clone());
        }
        copies += 1;
    }
    debug!("first view: {copies} kinds of prop mesh warming ({count} prop meshes)");
    *made = Some((warm.0, count, root));
}

/// The root of the copies `warm_models` draws, and what was built when
/// they were picked (bodies, held models).
#[derive(Component)]
pub struct WarmModels(usize, usize);

/// A mesh's kind for a pipeline: its vertex layout where the mesh is
/// still here (a mesh only on the GPU: the mesh itself).
type Layout = (Option<AssetId<Mesh>>, Option<(Vec<bevy::mesh::MeshVertexAttributeId>, bevy::mesh::PrimitiveTopology, bool)>);

fn layout_of(meshes: Option<&Assets<Mesh>>, mesh: &Handle<Mesh>) -> Layout {
    let layout = meshes.and_then(|m| m.get(mesh)).and_then(|m| {
        let ids: Vec<_> = m.try_attributes().ok()?.map(|(a, _)| a.id).collect();
        Some((ids, m.primitive_topology(), m.has_morph_targets()))
    });
    (layout.is_none().then(|| mesh.id()), layout)
}

/// One part of each kind of built body part (skinned, its bones all one
/// joint: never seen) and held model part, in front of the overview
/// camera (module doc), made again when more are built; gone with the
/// camera.
pub fn warm_models(
    warm: Option<Res<WarmFirstView>>,
    play: Option<Res<super::bodies::PlayModels>>,
    held: Option<Res<super::bodies::HeldAssets>>,
    materials: Option<Res<Assets<PropMaterial>>>,
    meshes: Option<Res<Assets<Mesh>>>,
    dormant: Option<Res<DormantProps>>,
    warmed: Query<(Entity, &WarmModels, &ChildOf)>,
    mut commands: Commands,
) {
    use bevy::pbr::Material;
    let (Some(warm), Some(materials)) = (warm, materials) else {
        for (e, ..) in &warmed {
            commands.entity(e).try_despawn();
        }
        return;
    };
    let bodies = play.as_ref().map_or(0, |p| p.built().count());
    let guns = held.as_ref().map_or(0, |h| h.0.len());
    if warmed.iter().any(|(_, w, at)| at.parent() == warm.0 && (w.0, w.1) == (bodies, guns)) {
        return;
    }
    for (e, ..) in &warmed {
        commands.entity(e).try_despawn();
    }
    let root = commands
        .spawn((
            Name::new("Bodies and held models warming"),
            WarmCopy,
            WarmModels(bodies, guns),
            Transform::from_xyz(0.0, 0.0, -4.0),
            Visibility::Visible,
            ChildOf(warm.0),
        ))
        .id();
    let mut kinds = HashSet::new();
    let mut copies = 0;
    let mut kind_of = |mesh: &Handle<Mesh>, material: &Handle<PropMaterial>, skinned: bool| {
        let m = prop(&materials, dormant.as_deref(), material)?;
        let kind = (layout_of(meshes.as_deref(), mesh), std::mem::discriminant(&m.alpha_mode()), m.double_sided, m.cull_front, skinned);
        kinds.insert(kind).then_some(())
    };
    for body in play.iter().flat_map(|p| p.built()) {
        let mut joint = None;
        for (mesh, material) in &body.parts {
            if kind_of(mesh, material, true).is_none() {
                continue;
            }
            let joint = *joint.get_or_insert_with(|| commands.spawn((body.root, Visibility::Inherited, ChildOf(root))).id());
            commands.spawn((
                WarmCopy,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                SkinnedMesh { inverse_bindposes: body.bindposes.clone(), joints: vec![joint; body.bones.len().max(1)] },
                NoFrustumCulling,
                ChildOf(joint),
            ));
            copies += 1;
        }
    }
    // In key order: the same copies each time.
    let mut keys: Vec<&String> = held.iter().flat_map(|h| h.0.keys()).collect();
    keys.sort();
    for key in keys {
        let Some(parts) = held.as_ref().and_then(|h| h.0.get(key)) else { continue };
        for (mesh, material) in &parts.whole {
            if kind_of(mesh, material, false).is_none() {
                continue;
            }
            commands.spawn((
                WarmCopy,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                NoFrustumCulling,
                Transform::default(),
                Visibility::Inherited,
                ChildOf(root),
            ));
            copies += 1;
        }
    }
    debug!("first view: {copies} kinds of body and held model part warming ({bodies} bodies, {guns} held models built)");
}

/// Above the bounds of the meshes on a render layer (the world's 0, the 3D
/// sky's `SKYBOX_LAYER`), looking down, far enough that a 90-degree view
/// holds them all, and the far plane past them; none without meshes. The
/// overview camera's pose, and the sky camera's meanwhile.
#[allow(clippy::type_complexity)]
pub fn overview_pose<F: bevy::ecs::query::QueryFilter>(
    meshes: &Query<
        (&bevy::camera::primitives::Aabb, &GlobalTransform, Option<&RenderLayers>),
        F,
    >,
    layer: usize,
) -> Option<(Transform, f32)> {
    let wanted = RenderLayers::layer(layer);
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for (aabb, at, layers) in meshes {
        if layers.is_some_and(|l| !l.intersects(&wanted)) || (layers.is_none() && layer != 0) {
            continue;
        }
        let c = at.transform_point(Vec3::from(aabb.center));
        let r = (at.affine().matrix3 * Vec3::from(aabb.half_extents)).abs();
        // An axis-aligned box around the turned one, wide enough.
        let r = Vec3::splat(r.length());
        lo = lo.min(c - r);
        hi = hi.max(c + r);
    }
    if lo.x > hi.x || !(hi - lo).is_finite() {
        return None;
    }
    let centre = (lo + hi) * 0.5;
    let half = (hi - lo) * 0.5;
    // Rounded, so the pose holds still as meshes come and go.
    let height = (half.y + half.x.max(half.z) * 1.1 + 1.0).ceil();
    let eye = (centre + Vec3::Y * height).round();
    let far = (height + half.y + 10.0).ceil();
    Some((Transform::from_translation(eye).looking_at(eye - Vec3::Y, Vec3::NEG_Z), far))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bevy::asset::RenderAssetUsages;

    use super::*;
    use crate::render::bodies::{HeldAssets, HeldParts};

    fn held(mesh: &Handle<Mesh>, material: &Handle<PropMaterial>) -> HeldParts {
        HeldParts {
            bone: "hand".into(),
            whole: vec![(mesh.clone(), material.clone())],
            own: Vec::new(),
            muzzle: None,
            attachments: Vec::new(),
            merged: Vec::new(),
            turned: Vec::new(),
        }
    }

    /// While a map warms behind a cover, one part of each kind of built
    /// held model is drawn in front of the overview camera, though nobody
    /// holds it yet; a model built later is added, and they go with the
    /// warming.
    #[test]
    fn a_part_of_each_kind_of_built_model_is_drawn_while_warming() {
        let mut app = App::new();
        app.insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<PropMaterial>::default())
            .add_systems(Update, warm_models);
        let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        ));
        let mut store = app.world_mut().resource_mut::<Assets<PropMaterial>>();
        let plain = store.add(PropMaterial::default());
        let same = store.add(PropMaterial::default());
        let two_sided = store.add(PropMaterial { double_sided: true, ..default() });
        let guns: HashMap<String, HeldParts> =
            [("a:one".to_string(), held(&mesh, &plain)), ("a:two".to_string(), held(&mesh, &same))].into();
        app.insert_resource(HeldAssets(guns));
        let copies = |app: &mut App| {
            let w = app.world_mut();
            w.query_filtered::<&MeshMaterial3d<PropMaterial>, With<WarmCopy>>().iter(w).map(|m| m.0.clone()).collect::<Vec<_>>()
        };
        // Not warming: nothing drawn.
        app.update();
        assert!(copies(&mut app).is_empty());
        let camera = app.world_mut().spawn(Transform::default()).id();
        app.insert_resource(WarmFirstView(camera));
        app.update();
        assert_eq!(copies(&mut app), [plain.clone()], "two guns of one kind: one copy");
        // A model of another kind built meanwhile.
        app.world_mut().resource_mut::<HeldAssets>().0.insert("b:three".into(), held(&mesh, &two_sided));
        app.update();
        app.update();
        let now = copies(&mut app);
        assert!(now.len() == 2 && now.contains(&plain) && now.contains(&two_sided), "{now:?}");
        // The warming over: the copies go.
        app.world_mut().remove_resource::<WarmFirstView>();
        app.update();
        app.update();
        assert!(copies(&mut app).is_empty());
    }
}
