//! The kit's faces and text roles (docs/plans/active/ugg-ui.md, 2.4).
//!
//! `KitFonts` holds our own faces (`OWN`, chimera/ui/fonts/: UGG Display
//! Black, Bold and Regular, one family by weight 900/700/400, and UGG
//! Mono; OFL-1.1, ours, with lower case since v0.5), embedded in the
//! binary (`include_bytes!`, then `Font::from_bytes`: no file I/O, the
//! same on Linux and Windows, offscreen and headless). `install` loads
//! them once per app (`ChimeraUiPlugin` and the client's fonts plugin
//! call it) and puts Display Regular, the text face, in Bevy's default
//! font, so text that names no font is drawn in ours, never in Bevy's
//! FiraMono or a system face. Parley also registers each under its
//! embedded family, so `FontSource::Family("UGG Display")` with
//! `FontWeight(900)` finds the Black face; the kit hands out the exact
//! face's handle.
//!
//! `Role` names a text role of the theme (`theme::TypeSpec`); `text` turns
//! a role and a size into the `TextFont` (face, weight, OpenType features:
//! `ss02` on the wordmark), the `LetterSpacing` (tracking in em times the
//! size), `FontHinting`, the line height (`TypeSpec::line`) and the case
//! rule (`TypeSpec::upper`: the display roles and the small Mono labels
//! upper-case their text, `Role::Text` keeps it). `Role::Text` is the
//! sentence face (Display Regular in sentence case): descriptions, help,
//! dialog bodies, notices, typed text, names. A role whose face is no kit
//! face resolves by family name and weight.

use bevy::{
    prelude::*,
    text::{FontFeatureTag, FontFeatures, FontHinting, FontSize, FontSource, FontWeight, LetterSpacing, LineHeight},
};

use crate::theme::{Theme, TypeSpec};

/// The family the Display faces carry (their typographic family name).
pub const DISPLAY_FAMILY: &str = "UGG Display";
/// The Mono face's family.
pub const MONO_FAMILY: &str = "UGG Mono";

/// Our faces' files, embedded (module doc).
pub const OWN: FontFiles<'static> = FontFiles {
    display_black: include_bytes!("../fonts/ugg-display-black.ttf"),
    display_bold: include_bytes!("../fonts/ugg-display-bold.ttf"),
    display_regular: include_bytes!("../fonts/ugg-display-regular.ttf"),
    mono: include_bytes!("../fonts/ugg-mono-regular.ttf"),
};

/// A text role of the theme (`[type.<name>]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// The game's name in the lockup: Black, `ss02`.
    Wordmark,
    DisplayBlack,
    DisplayBold,
    DisplayRegular,
    /// Data: numbers in tables, timers, keys, cvar names; and the small
    /// tracked labels (section heads, kv keys, tags).
    Mono,
    /// Sentences and what people typed (module doc): Display Regular in
    /// its own case, lines 1.3 em tall for the descenders.
    Text,
}

impl Role {
    pub const ALL: [Role; 6] =
        [Role::Wordmark, Role::DisplayBlack, Role::DisplayBold, Role::DisplayRegular, Role::Mono, Role::Text];

    /// Its table in a theme's TOML.
    pub fn key(self) -> &'static str {
        match self {
            Role::Wordmark => "wordmark",
            Role::DisplayBlack => "display_black",
            Role::DisplayBold => "display_bold",
            Role::DisplayRegular => "display_regular",
            Role::Mono => "mono",
            Role::Text => "text",
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
    /// Each face's bytes by its handle, to measure text
    /// (`text_fit`: `KitFonts::width`).
    pub bytes: Vec<(AssetId<Font>, std::sync::Arc<[u8]>)>,
}

impl KitFonts {
    /// Our faces (`OWN`) in `world`, once: made into fonts and inserted as
    /// the `KitFonts` resource, and Display Regular put in Bevy's default
    /// font (module doc). A `KitFonts` already there is kept. Without
    /// `Assets<Font>` (no text plugin: headless tests) nothing happens.
    pub fn install(world: &mut World) -> Option<KitFonts> {
        if let Some(kit) = world.get_resource::<KitFonts>() {
            return Some(kit.clone());
        }
        let mut fonts = world.get_resource_mut::<Assets<Font>>()?;
        let kit = KitFonts::load(&mut fonts, OWN);
        // The default handle's asset: Bevy's text plugin put FiraMono there.
        let _ = fonts.insert(AssetId::default(), Font::from_bytes(OWN.display_regular.to_vec()));
        world.insert_resource(kit.clone());
        Some(kit)
    }

    /// The four faces made into `Font` assets.
    pub fn load(fonts: &mut Assets<Font>, files: FontFiles) -> KitFonts {
        let mut bytes = Vec::new();
        let mut add = |b: &[u8]| {
            let h = fonts.add(Font::from_bytes(b.to_vec()));
            bytes.push((h.id(), std::sync::Arc::<[u8]>::from(b)));
            Some(h)
        };
        let (display_black, display_bold, display_regular, mono) =
            (add(files.display_black), add(files.display_bold), add(files.display_regular), add(files.mono));
        KitFonts { display_black, display_bold, display_regular, mono, bytes }
    }

    /// The bytes of the face `id` (one of the kit's).
    pub fn bytes_of(&self, id: AssetId<Font>) -> Option<&[u8]> {
        self.bytes.iter().find(|(i, _)| *i == id).map(|(_, b)| &b[..])
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
    /// The role's line height (`TypeSpec::line`).
    pub line: LineHeight,
}

impl Styled {
    /// The components for a `Text`/`TextSpan` entity. Lines are the role's
    /// height: one font size for capitals (the look book's
    /// `font: <size>/1`; Bevy's default is 1.2), so a text's box is the
    /// look book's; more for `Role::Text`, whose descenders reach below
    /// the faces' line box.
    pub fn bundle(&self) -> (TextFont, LetterSpacing, FontHinting, LineHeight) {
        (self.font.clone(), self.spacing, self.hinting, self.line)
    }

    /// `text` in the role's case.
    pub fn case(&self, text: &str) -> String {
        if self.upper { text.to_uppercase() } else { text.to_string() }
    }
}

/// A capitals role's line height (`Styled::bundle`): one font size.
pub const LINE_HEIGHT: LineHeight = LineHeight::RelativeToFont(1.0);

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
        line: if spec.line == 1.0 { LINE_HEIGHT } else { LineHeight::RelativeToFont(spec.line) },
    }
}
