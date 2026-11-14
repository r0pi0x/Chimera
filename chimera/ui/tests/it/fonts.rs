//! The kit's faces and text roles (chimera_ui::fonts).

use bevy::{
    prelude::*,
    text::{FontCx, FontFeatureTag, FontFeatures, FontSource, LetterSpacing, TextPlugin},
};
use chimera_ui::{FontFiles, KitFonts, Role, Theme, fonts};

/// The Ultimate Gun Game's faces and theme (read here as data; the app
/// embeds them).
const SIGNAL: &str = include_str!("../../../../apps/ultimate_gun_game/data/theme.toml");
const FILES: FontFiles<'static> = FontFiles {
    display_black: include_bytes!("../../../../apps/ultimate_gun_game/fonts/ugg-display-black.ttf"),
    display_bold: include_bytes!("../../../../apps/ultimate_gun_game/fonts/ugg-display-bold.ttf"),
    display_regular: include_bytes!("../../../../apps/ultimate_gun_game/fonts/ugg-display-regular.ttf"),
    mono: include_bytes!("../../../../apps/ultimate_gun_game/fonts/ugg-mono-regular.ttf"),
};

/// The faces register under their families, Display by weight (400, 700,
/// 900), and each role picks its face's handle.
#[test]
fn fonts_resolve_by_family_and_weight() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TextPlugin));
    let kit = KitFonts::load(&mut app.world_mut().resource_mut::<Assets<Font>>(), FILES);
    app.update();
    let mut cx = app.world_mut().resource_mut::<FontCx>();
    let family = cx.collection.family_by_name(fonts::DISPLAY_FAMILY).expect("UGG Display registered");
    let mut weights: Vec<f32> = family.fonts().iter().map(|f| f.weight().value()).collect();
    weights.sort_by(f32::total_cmp);
    assert_eq!(weights, vec![400.0, 700.0, 900.0]);
    assert!(cx.collection.family_by_name(fonts::MONO_FAMILY).is_some(), "UGG Mono registered");

    let theme = Theme::from_toml(SIGNAL).unwrap();
    let face = |r| match fonts::text(&theme, Some(&kit), r, 20.0).font.font {
        FontSource::Handle(h) => h,
        other => panic!("{r:?}: {other:?}"),
    };
    assert_eq!(face(Role::Wordmark), kit.display_black.clone().unwrap());
    assert_eq!(face(Role::DisplayBlack), kit.display_black.clone().unwrap());
    assert_eq!(face(Role::DisplayBold), kit.display_bold.clone().unwrap());
    assert_eq!(face(Role::DisplayRegular), kit.display_regular.clone().unwrap());
    assert_eq!(face(Role::Mono), kit.mono.clone().unwrap());
    // Without the faces a role resolves by family and weight.
    let by_name = fonts::text(&theme, None, Role::DisplayBlack, 20.0).font;
    assert_eq!(by_name.font, FontSource::Family(fonts::DISPLAY_FAMILY.into()));
    assert_eq!(by_name.weight.0, 900);
    // The shell's default theme: Trebuchet MS.
    let shell = fonts::text(&Theme::shell_default(), Some(&kit), Role::DisplayBold, 20.0).font;
    assert_eq!(shell.font, FontSource::Family("trebuchet ms".into()));
}

/// The wordmark turns on `ss02` (its plain A); no other role does. Its
/// tracking is .03 em.
#[test]
fn wordmark_enables_ss02() {
    let theme = Theme::from_toml(SIGNAL).unwrap();
    let w = fonts::text(&theme, None, Role::Wordmark, 40.0);
    assert_eq!(w.font.font_features, FontFeatures::builder().enable(FontFeatureTag::new(b"ss02")).build());
    assert_eq!(w.spacing, LetterSpacing::Px(0.03 * 40.0));
    for r in [Role::DisplayBlack, Role::DisplayBold, Role::DisplayRegular, Role::Mono] {
        assert_eq!(fonts::text(&theme, None, r, 40.0).font.font_features, FontFeatures::default(), "{r:?}");
    }
}
