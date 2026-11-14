//! Combat Arms' drawn HUD declares its fonts ahead (`ui::hud`'s warm
//! source for `client::text_warm`): with the warm-up done, the HUD's texts
//! and the first kill in its feed make no glyph atlas on their frame
//! (`TextWarm::unwarmed`, Bevy's `FontAtlasSet` counted). Headless, on
//! Bevy's own text pipeline and layout (a dummy camera), our font list
//! (no install) in Bevy's default face.

use bevy::{prelude::*, text::FontAtlasSet};
use chimera_client::client::text_warm::{self, TextWarm, TextWarmPlugin};
use chimera_core::core::{Damage, DamageKind, Died, Hitgroup, LocalPlayer, Team};
use chimera_sim::weapon::StartingWeapons;
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use combat_arms::{
    game::{inventory, movement},
    harness::CaSim,
    ui::hud::CaHudPlugin,
};

fn atlases(sim: &Sim) -> usize {
    sim.app.world().resource::<FontAtlasSet>().values().map(Vec::len).sum()
}

/// The HUD up with a local player and another (with the warm-up's
/// systems, run behind a loading screen first, or without), then the first
/// kill in its feed: the atlases that kill's frame made, and the warm-up.
fn first_kill(warm: bool) -> (usize, Option<(usize, usize)>) {
    use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
    let mut sim = Sim::with(CaSim, move |app| {
        app.add_plugins(GreyboxMapPlugin);
        app.init_resource::<Assets<Image>>()
            .init_resource::<Assets<bevy::image::TextureAtlasLayout>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Touches>()
            .init_resource::<bevy::picking::hover::HoverMap>()
            .init_resource::<bevy::picking::pointer::PointerMap>()
            .init_resource::<bevy::picking::events::PointerState>()
            .add_message::<bevy::picking::pointer::PointerInput>()
            .add_message::<bevy::picking::backend::PointerHits>()
            .init_asset::<Font>();
        app.add_plugins((bevy::text::TextPlugin, bevy::ui::UiPlugin, CaHudPlugin));
        if warm {
            app.add_plugins(TextWarmPlugin);
        }
        app.insert_resource(StartingWeapons { team: Vec::new(), all: vec![inventory::KNIFE] });
    });
    sim.app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: ComputedCameraValues {
                target_info: Some(RenderTargetInfo { physical_size: UVec2::new(1024, 768), scale_factor: 1.0 }),
                ..default()
            },
            viewport: Some(Viewport { physical_size: UVec2::new(1024, 768), ..default() }),
            ..default()
        },
    ));
    // The warm-up as behind the loading screen, before anyone plays.
    // The atlases' task is waited for before each frame: the frames are
    // then the same on every run, whatever the machine's load.
    let mut frames = 0;
    while warm && {
        text_warm::fill_pool(sim.app.world_mut());
        !text_warm::settle(sim.app.world_mut())
    } {
        sim.app.update();
        frames += 1;
        assert!(frames < 60, "the warm-up ends");
    }
    let me = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.app.world_mut().entity_mut(me).insert((LocalPlayer, Team(1), Name::new("Me")));
    let other = sim.spawn_character(greybox::SPAWNS[1], movement::ID);
    sim.app.world_mut().entity_mut(other).insert((Team(2), Name::new("Other")));
    sim.seconds(1.0);
    let shown = sim.app.world_mut().query::<&Text>().iter(sim.app.world()).filter(|t| !t.0.is_empty()).count();
    assert!(shown > 3, "the HUD draws its texts: {shown}");
    let before = atlases(&sim);
    sim.app.world_mut().write_message(Died {
        entity: other,
        attacker: Some(me),
        damage: Damage {
            target: other,
            attacker: Some(me),
            amount: 100.0,
            point: Vec3::ZERO,
            dir: Vec3::NEG_Z,
            hitgroup: Hitgroup::Head,
            kind: DamageKind::Bullet,
            weapon: Some(inventory::KNIFE),
            force: Vec3::ZERO,
        },
    });
    sim.ticks(5);
    let feed = sim.app.world_mut().query::<&Text>().iter(sim.app.world()).any(|t| t.0.contains("Other"));
    assert!(feed, "the feed names the victim");
    let made = atlases(&sim) - before;
    let w = sim.app.world().resource::<TextWarm>();
    (made, warm.then_some((w.unwarmed, w.missed)))
}

#[test]
fn the_huds_texts_make_no_atlas_on_their_frame_after_the_warm_up() {
    let (made, _) = first_kill(false);
    assert!(made > 0, "without the warm-up the feed's first line makes its font's atlas");
    let (made, warm) = first_kill(true);
    assert_eq!(warm, Some((0, 0)), "(unwarmed, missed): a HUD text made an atlas on its first frame (the log names its size)");
    assert_eq!(made, 0, "the kill feed's first line made an atlas");
}
