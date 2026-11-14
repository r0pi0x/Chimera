//! The kit's faces and text roles (docs/plans/active/ugg-ui.md, 2.4).
//!
//! `KitFonts` holds the app's own faces, made from bytes the app embeds
//! (`include_bytes!`, then `Font::from_bytes`: no file I/O, the same on
//! Linux and Windows, offscreen and headless): Display Black, Bold and
//! Regular (one family, `UGG Display`, by weight 900/700/400) and Mono.
//! Parley also registers each under its embedded family, so
//! `FontSource::Family("UGG Display")` with `FontWeight(900)` finds the
//! Black face; the kit hands out the exact face's handle.
//!
//! `Role` names a text role of the theme (`theme::TypeSpec`); `text` turns
//! a role and a size into the `TextFont` (face, weight, OpenType features:
//! `ss02` on the wordmark), the `LetterSpacing` (tracking in em times the
//! size) and `FontHinting`. A role whose face is no kit face (the shell's
//! default theme: Trebuchet MS) resolves by family name and weight.

use bevy::{
    prelude::*,
    text::{FontFeatureTag, FontFeatures, FontHinting, FontSize, FontSource, FontWeight, LetterSpacing},
};

use crate::theme::{Theme, TypeSpec};

/// The family the Display faces carry (their typographic family name).
pub const DISPLAY_FAMILY: &str = "UGG Display";
/// The Mono face's family.
pub const MONO_FAMILY: &str = "UGG Mono";

/// A text role of the theme (`[type.<name>]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// The game's name in the lockup: Black, `ss02`.
    Wordmark,
    DisplayBlack,
    DisplayBold,
    DisplayRegular,
    /// Labels, numbers in tables, keys, tags.
    Mono,
}

impl Role {
    pub const ALL: [Role; 5] = [Role::Wordmark, Role::DisplayBlack, Role::DisplayBold, Role::DisplayRegular, Role::Mono];

    /// Its table in a theme's TOML.
    pub fn key(self) -> &'static str {
        match self {
            Role::Wordmark => "wordmark",
            Role::DisplayBlack => "display_black",
            Role::DisplayBold => "display_bold",
            Role::DisplayRegular => "display_regular",
            Role::Mono => "mono",
        }
    }
}

/// The app's font files (the bytes it embeds).
#[derive(Clone, Copy, Debug)]
pub struct FontFiles<'a> {
    pub display_black: &'a [u8],
    pub display_bold: &'a [u8],
    pub display_regular: &'a [u8],
    pub mono: &'a [u8],
}

/// The kit's faces (module doc). Absent or empty: roles resolve by family
/// name (`text`).
#[derive(Resource, Clone, Debug, Default)]
pub struct KitFonts {
    pub display_black: Option<Handle<Font>>,
    pub display_bold: Option<Handle<Font>>,
    pub display_regular: Option<Handle<Font>>,
    pub mono: Option<Handle<Font>>,
}

impl KitFonts {
    /// The four faces made into `Font` assets.
    pub fn load(fonts: &mut Assets<Font>, files: FontFiles) -> KitFonts {
        let mut add = |b: &[u8]| Some(fonts.add(Font::from_bytes(b.to_vec())));
        KitFonts {
            display_black: add(files.display_black),
            display_bold: add(files.display_bold),
            display_regular: add(files.display_regular),
            mono: add(files.mono),
        }
    }

    /// The face for a `face` name and weight: `display` by weight (the
    /// nearest of 400/700/900), `mono`; `None` for any other face or a
    /// face not loaded.
    pub fn face(&self, face: &str, weight: u16) -> Option<Handle<Font>> {
        match face {
            "display" => match weight {
                0..=549 => self.display_regular.clone(),
                550..=799 => self.display_bold.clone(),
                _ => self.display_black.clone(),
            },
            "mono" => self.mono.clone(),
            _ => None,
        }
    }
}

/// The family name a role's face resolves by when the kit has no handle.
pub fn family_of(face: &str) -> &str {
    match face {
        "display" => DISPLAY_FAMILY,
        "mono" => MONO_FAMILY,
        f => f,
    }
}

/// A role at a size: what a text entity needs (`bundle`), and the role's
/// case rule (`case`).
#[derive(Clone, Debug, PartialEq)]
pub struct Styled {
    pub font: TextFont,
    pub spacing: LetterSpacing,
    pub hinting: FontHinting,
    pub upper: bool,
}

impl Styled {
    /// The components for a `Text`/`TextSpan` entity.
    pub fn bundle(&self) -> (TextFont, LetterSpacing, FontHinting) {
        (self.font.clone(), self.spacing, self.hinting)
    }

    /// `text` in the role's case.
    pub fn case(&self, text: &str) -> String {
        if self.upper { text.to_uppercase() } else { text.to_string() }
    }
}

/// The `TextFont` for a type spec at `px` logical pixels.
pub fn text_font(spec: &TypeSpec, fonts: Option<&KitFonts>, px: f32) -> TextFont {
    let font = match fonts.and_then(|f| f.face(&spec.face, spec.weight)) {
        Some(h) => FontSource::Handle(h),
        None => FontSource::Family(family_of(&spec.face).into()),
    };
    let mut features = FontFeatures::builder();
    for f in &spec.features {
        let b = f.as_bytes();
        if b.len() == 4 {
            features = features.enable(FontFeatureTag::new(&[b[0], b[1], b[2], b[3]]));
        }
    }
    TextFont {
        font,
        font_size: FontSize::Px(px),
        weight: FontWeight(spec.weight),
        font_features: features.build(),
        ..default()
    }
}

/// A role of `theme` at `px` logical pixels (already scaled: `units`).
pub fn text(theme: &Theme, fonts: Option<&KitFonts>, role: Role, px: f32) -> Styled {
    let spec = theme
        .type_spec(role.key())
        .expect("every theme lays over the shell's, which has every role");
    Styled {
        font: text_font(spec, fonts, px),
        spacing: LetterSpacing::Px(spec.tracking * px),
        hinting: if spec.hinted { FontHinting::Enabled } else { FontHinting::Disabled },
        upper: spec.upper,
    }
}
