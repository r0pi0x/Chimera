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
