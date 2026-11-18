use super::*;
    use std::time::Duration;

    use avian3d::{
        collider_tree::{ColliderTreeProxyFlags, ColliderTreeProxyKey, ColliderTrees},
        prelude::*,
    };
    use bevy::prelude::*;

    /// The avian bug `set_prop_shown` works around (the module's known
    /// shortcuts): a collider whose body had `RigidBodyDisabled` keeps
    /// "body disabled" on its query-tree proxy after the component goes.
    /// When this fails, avian clears the flag itself and the workaround
    /// (and this test) can go.
    #[test]
    fn avian_still_leaves_body_disabled_on_the_proxy() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            PhysicsPlugins::default().build().disable::<PhysicsInterpolationPlugin>(),
        ))
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 64.0)));
        app.finish();
        app.cleanup();
        app.update();
        let body = app
            .world_mut()
            .spawn((RigidBody::Dynamic, Collider::cuboid(1.0, 1.0, 1.0), RigidBodyDisabled, Transform::default()))
            .id();
        for _ in 0..3 {
            app.update();
        }
        app.world_mut().entity_mut(body).remove::<RigidBodyDisabled>();
        for _ in 0..3 {
            app.update();
        }
        let key = *app.world().get::<ColliderTreeProxyKey>(body).expect("in the tree");
        let trees = app.world().resource::<ColliderTrees>();
        let proxy = trees.tree_for_type(key.tree_type()).get_proxy(key.id()).expect("its proxy");
        assert!(
            proxy.flags.contains(ColliderTreeProxyFlags::BODY_DISABLED),
            "avian clears the proxy's body-disabled flag now: drop the workaround in `set_prop_shown`"
        );
    }

