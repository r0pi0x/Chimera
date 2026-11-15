//! The kit's faces and text roles (chimera_ui::fonts).

use bevy::{
    prelude::*,
    text::{FontCx, FontFeatureTag, FontFeatures, FontSource, LetterSpacing, TextPlugin},
};
use chimera_ui::{FontFiles, KitFonts, Role, Theme, fonts};

/// The Ultimate Gun Game's theme (read here as data; the app embeds it)
/// and our faces.
const SIGNAL: &str = include_str!("../../../../apps/ultimate_gun_game/data/theme.toml");
const FILES: FontFiles<'static> = fonts::OWN;

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
    // The shell's default theme: our faces too.
    let shell = fonts::text(&Theme::shell_default(), Some(&kit), Role::DisplayBold, 20.0).font;
    assert_eq!(shell.font, FontSource::Handle(kit.display_bold.clone().unwrap()));
    assert_eq!(face(Role::Text), kit.display_regular.clone().unwrap());
}

/// Every role of both themes is one of our faces (`display`, `mono`),
/// never a family looked up among the system's fonts.
#[test]
fn every_role_is_one_of_our_faces() {
    for theme in [Theme::shell_default(), Theme::from_toml(SIGNAL).unwrap()] {
        for role in Role::ALL {
            let spec = theme.type_spec(role.key()).unwrap();
            assert!(["display", "mono"].contains(&spec.face.as_str()), "{}: {role:?} is {}", theme.name, spec.face);
        }
    }
}

/// Bevy's default font is ours (Display Regular, the text face) once the
/// faces are installed, so text naming no font is never FiraMono or a
/// system face; installing again keeps the same faces.
#[test]
fn the_default_font_is_ours() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TextPlugin));
    let kit = KitFonts::install(app.world_mut()).expect("the text plugin is in");
    let again = KitFonts::install(app.world_mut()).unwrap();
    assert_eq!(kit.display_regular, again.display_regular);
    let fonts = app.world().resource::<Assets<Font>>();
    let default = fonts.get(AssetId::<Font>::default()).expect("a default font");
    assert_eq!(default.data.data(), fonts::OWN.display_regular, "Bevy's default font is UGG Display Regular");
    assert!(app.world().contains_resource::<KitFonts>());
}

/// The sentence role keeps its case and sets taller lines; the display
/// roles and Mono upper-case. The faces draw a real lower case: "a" is its
/// own glyph, narrower than "A", in every face.
#[test]
fn the_text_role_draws_lower_case() {
    use bevy::text::LineHeight;
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TextPlugin));
    let kit = KitFonts::load(&mut app.world_mut().resource_mut::<Assets<Font>>(), FILES);
    for theme in [Theme::shell_default(), Theme::from_toml(SIGNAL).unwrap()] {
        let text = fonts::text(&theme, Some(&kit), Role::Text, 16.0);
        assert!(!text.upper, "{}: the text role keeps case", theme.name);
        assert_eq!(text.case("Zoë typed this"), "Zoë typed this");
        assert_eq!(text.line, LineHeight::RelativeToFont(1.3));
        for role in [Role::Wordmark, Role::DisplayBlack, Role::DisplayBold, Role::DisplayRegular, Role::Mono] {
            let s = fonts::text(&theme, Some(&kit), role, 16.0);
            assert!(s.upper, "{}: {role:?} stays in capitals", theme.name);
            assert_eq!(s.case("Apply"), "APPLY");
        }
    }
    let theme = Theme::from_toml(SIGNAL).unwrap();
    for role in [Role::Text, Role::DisplayBold, Role::DisplayBlack, Role::Mono] {
        let f = fonts::text(&theme, Some(&kit), role, 32.0).font;
        for (low, cap) in [('a', 'A'), ('e', 'E'), ('n', 'N'), ('ë', 'Ë')] {
            assert!(kit.has_glyph(&f, low), "{role:?} draws {low}");
            let (wl, wc) = (kit.width(&f, None, &low.to_string()), kit.width(&f, None, &cap.to_string()));
            if role != Role::Mono {
                assert!(wl < wc, "{role:?}: {low} ({wl}) is its own glyph, narrower than {cap} ({wc})");
            }
        }
    }
    // Mono: one advance, but its own glyphs (the outlines differ).
    let mono = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/ugg-mono-regular.ttf")).unwrap();
    let face = skrifa::FontRef::from_index(&mono, 0).unwrap();
    use skrifa::MetadataProvider;
    let map = face.charmap();
    assert_ne!(map.map('a'), map.map('A'), "Mono's a is not its A");
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
