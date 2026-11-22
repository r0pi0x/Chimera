//! The install as an asset source (architecture review L4): a texture
//! and a sound of the user's CS:S install load through `cs_source://`
//! with the game's loaders, and come out as the by-hand decode makes
//! them (`material::decode_bytes` and `render::to_image`, the WAV reader).

use std::sync::Arc;

use bevy::{
    asset::{LoadState, UntypedAssetId},
    prelude::*,
};
use chimera_present::{
    assets::{InstallSource, InstallSourcesPlugin, install_path},
    audio::clip_asset::{SoundAsset, SoundAssetPlugin},
};
use cs_source_ui::textures::{VtfLoader, VtfSettings, vtf_image};

fn wait(app: &mut App, handle: UntypedAssetId) -> LoadState {
    for _ in 0..20_000 {
        app.update();
        let state = app.world().resource::<AssetServer>().load_state(handle);
        if !matches!(state, LoadState::Loading | LoadState::NotLoaded) {
            return state;
        }
        chimera_world::harness::let_io_run();
    }
    panic!("still loading");
}

#[test]
fn install_files_load_as_assets() {
    let Some(mount) = cs_source::game::INFO.mount().map(Arc::new) else {
        eprintln!("no CS:S install: skipped");
        return;
    };
    let shared = mount.clone();
    let mut app = App::new();
    app.add_plugins((
        InstallSourcesPlugin(vec![InstallSource::with(cs_source::game::GAME, move || Some(shared.clone()))]),
        MinimalPlugins,
        AssetPlugin::default(),
        SoundAssetPlugin,
    ))
    .init_asset::<Image>()
    .register_asset_loader(VtfLoader { bc: true });
    let server = app.world().resource::<AssetServer>().clone();

    let file = "materials/console/background01.vtf";
    let image: Handle<Image> = server.load(install_path(cs_source::game::GAME, "Materials\\Console\\Background01.VTF"));
    assert!(wait(&mut app, image.id().untyped()).is_loaded());
    let want = vtf_image(&mount.read(file).unwrap(), VtfSettings::default(), true).unwrap();
    let got = app.world().resource::<Assets<Image>>().get(&image).unwrap();
    assert_eq!(got.texture_descriptor, want.texture_descriptor);
    assert_eq!(got.data, want.data);
    assert!(got.width() >= 512, "the menu background, {} wide", got.width());

    let wav = "sound/ui/buttonclick.wav";
    let sound: Handle<SoundAsset> = server.load(install_path(cs_source::game::GAME, wav));
    assert!(wait(&mut app, sound.id().untyped()).is_loaded());
    let want = chimera_world::map::sound::decode_clip(&mount.read(wav).unwrap()).unwrap();
    let got = &app.world().resource::<Assets<SoundAsset>>().get(&sound).unwrap().0;
    assert_eq!((got.rate, got.channels, &got.samples[..]), (want.rate, want.channels, &want.samples[..]));

    let missing: Handle<Image> = server.load(install_path(cs_source::game::GAME, "materials/no/such.vtf"));
    assert!(wait(&mut app, missing.id().untyped()).is_failed());
}
