//! Player avatars (the kit's design rules, revision 6, sections 12 and
//! 14; docs/plans/active/ugg-ui.md "Signal revision 8"): a square, one
//! of four glyphs (`GLYPHS`: the logo's three characters, each alone, and
//! the player's head and shoulders; icons.toml's `avatar_*`) tinted with
//! one of eight colours (`PALETTE`) on that colour's own deep ground. 4 x
//! 8 = 32 looks (`AvatarLook`), drawn at random on the first run and kept
//! in the local profile (the app's cvars); two players may share one,
//! the name always sits beside it. Bots get the robot on `ink_4`.
//!
//! - `spawn`: the square at a size, its glyph filling it (the busts run
//!   off the square's foot); YOU a white inner frame (2 px or 4.5 % of the
//!   side); *not looking* (the window unfocused) the glyph at 20 % under a
//!   crossed-out eye (`eye_off`); *idle* (5 min) the whole avatar greyed.
//! - `spawn_strip`: the edge strip at the avatar's right (status option
//!   B): its level is the state (`EdgeState`: white Ready, cobalt In a
//!   match, a hollow outline filling from the foot while loading, a dim
//!   outline otherwise), a white cap with an ink crown on top for the
//!   party's leader (only in a party of two or more).
//!
//! Lobby rows show none of the status on the avatar: their status is the
//! row's progress component (`controls::Ui::progress`).

use bevy::prelude::*;

use crate::{
    IconImages, Tone,
    controls::Ui,
};

/// The glyphs' icon keys (icons.toml), in the profile's order.
pub const GLYPHS: [&str; 4] = ["avatar_operator", "avatar_trooper", "avatar_vanguard", "avatar_player"];

/// The glyphs' names, as the profile page lists them.
pub const GLYPH_NAMES: [&str; 4] = ["Operator", "Trooper", "Vanguard", "Player"];

/// The colours: (name, tint, ground). Light tints (L 75-90 %) on the same
/// hue's deep ground (L 8-14 %); no red (threat), amber (warn), mint
/// (good) or cobalt (the hero).
pub const PALETTE: [(&str, &str, &str); 8] = [
    ("Ice", "#8EDCF5", "#0E2A36"),
    ("Teal", "#5FD4C4", "#0B2A28"),
    ("Lime", "#CFE874", "#232A0C"),
    ("Sand", "#E8D7B0", "#2C2517"),
    ("Rose", "#FF9CCB", "#3A1229"),
    ("Orchid", "#E3A0FF", "#2D1240"),
    ("Violet", "#A9A6FF", "#1C1A48"),
    ("Steel", "#C4CEDC", "#1A212C"),
];

/// One of the 32 looks: a glyph (0-3) and a colour (0-7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AvatarLook {
    pub glyph: u8,
    pub colour: u8,
}

impl AvatarLook {
    /// "No avatar": a bot, or a player of an older build.
    pub const NONE: u8 = 255;

    pub fn new(glyph: u8, colour: u8) -> Self {
        AvatarLook { glyph: glyph % GLYPHS.len() as u8, colour: colour % PALETTE.len() as u8 }
    }

    /// From its wire byte (`glyph * 8 + colour`); None for `NONE` or out of
    /// range.
    pub fn from_byte(b: u8) -> Option<Self> {
        let (g, c) = (b / 8, b % 8);
        ((g as usize) < GLYPHS.len()).then_some(AvatarLook { glyph: g, colour: c })
    }

    /// Its wire byte.
    pub fn byte(self) -> u8 {
        self.glyph * 8 + self.colour
    }

    /// A random look from a seed (the first run's draw: glyph and colour
    /// drawn separately).
    pub fn random(seed: u64) -> Self {
        // SplitMix64: a well-spread hash of the seed.
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        AvatarLook::new((z % 4) as u8, ((z >> 8) % 8) as u8)
    }

    /// A stand-in look for a name (a player whose build sends none): its
    /// hash's.
    pub fn for_name(name: &str) -> Self {
        let h = name.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
        AvatarLook::random(h)
    }

    pub fn glyph_key(self) -> &'static str {
        GLYPHS[self.glyph as usize % GLYPHS.len()]
    }

    pub fn tint(self) -> Color {
        hex(PALETTE[self.colour as usize % PALETTE.len()].1)
    }

    pub fn ground(self) -> Color {
        hex(PALETTE[self.colour as usize % PALETTE.len()].2)
    }

    /// The colour's name ("Ice").
    pub fn colour_name(self) -> &'static str {
        PALETTE[self.colour as usize % PALETTE.len()].0
    }
}

fn hex(h: &str) -> Color {
    Srgba::hex(h).map(Color::Srgba).unwrap_or(Color::WHITE)
}

/// Who the avatar is and how they are (the marks on the avatar itself).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AvatarMarks {
    /// Your own: the white inner frame.
    pub you: bool,
    /// The window isn't focused: the crossed-out eye.
    pub away: bool,
    /// Idle for 5 min: greyed.
    pub idle: bool,
}

/// What an avatar shows: a look, or the bot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Face {
    Look(AvatarLook),
    Bot,
}

/// An avatar node (its look and marks, for tests and live updates).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Avatar {
    pub face: Face,
    pub marks: AvatarMarks,
}

/// The YOU frame inside an avatar.
#[derive(Component, Clone, Copy, Debug)]
pub struct YouFrame;

/// The crossed-out eye over an away avatar.
#[derive(Component, Clone, Copy, Debug)]
pub struct AwayEye;

/// Greys a colour as the idle avatar does (luma, then darkened to .55).
pub fn grey(c: Color) -> Color {
    let s = c.to_srgba();
    let l = (0.2126 * s.red + 0.7152 * s.green + 0.0722 * s.blue) * 0.55;
    Color::srgba(l, l, l, s.alpha)
}

/// Spawns an avatar `size` cq square under `parent` (module doc).
#[allow(clippy::too_many_arguments)]
pub fn spawn(ui: &Ui, c: &mut Commands, parent: Entity, images: &mut Assets<Image>, icons: &mut IconImages, face: Face, marks: AvatarMarks, size: f32) -> Entity {
    let u = ui.units;
    let side = u.snap(u.cq(size));
    let (ground, tint, key) = match face {
        Face::Look(l) => (l.ground(), l.tint(), l.glyph_key()),
        Face::Bot => (ui.color(Tone::Ink4, 1.0), ui.color(Tone::Fg2, 1.0), "bot"),
    };
    let (ground, tint) = if marks.idle { (grey(ground), grey(tint)) } else { (ground, tint) };
    let root = c
        .spawn((
            Avatar { face, marks },
            Node { width: Val::Px(side), height: Val::Px(side), flex_shrink: 0.0, overflow: Overflow::clip(), ..default() },
            BackgroundColor(ground),
            ChildOf(parent),
        ))
        .id();
    // The glyph fills the square (a bot's robot sits in its middle).
    let px = u.phys(side).round().max(1.0) as u32;
    let (gpx, inset) = match face {
        Face::Bot => ((px as f32 * 0.56).round() as u32, side * 0.22),
        Face::Look(_) => (px, 0.0),
    };
    let image = icons.get(images, key, gpx).unwrap_or_default();
    let glyph_a = if marks.away { 0.2 } else { 1.0 };
    c.spawn((
        ImageNode { image, color: tint.with_alpha(tint.alpha() * glyph_a), ..default() },
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(inset),
            top: Val::Px(inset),
            width: Val::Px(gpx as f32 / u.scale_factor),
            height: Val::Px(gpx as f32 / u.scale_factor),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(root),
    ));
    if !marks.you {
        // A faint keyline in the tint keeps the square's edge on dark.
        c.spawn((
            Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, right: Val::ZERO, bottom: Val::ZERO, ..default() },
            Outline::new(Val::Px(u.hairline()), Val::Px(-u.hairline()), tint.with_alpha(0.16)),
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
    if marks.away {
        let e = (side * 0.6).round();
        let epx = u.phys(e).round().max(1.0) as u32;
        let image = icons.get(images, "eye_off", epx).unwrap_or_default();
        c.spawn((
            AwayEye,
            ImageNode { image, color: ui.color(Tone::Fg, 1.0), ..default() },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(side * 0.2),
                top: Val::Px(side * 0.2),
                width: Val::Px(epx as f32 / u.scale_factor),
                height: Val::Px(epx as f32 / u.scale_factor),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
    if marks.you {
        let w = (2.0 / u.scale_factor).max(u.snap(side * 0.045));
        c.spawn((
            YouFrame,
            Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, right: Val::ZERO, bottom: Val::ZERO, ..default() },
            Outline::new(Val::Px(w), Val::Px(-w), Color::WHITE),
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
    root
}

/// The edge strip's level (module doc).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum EdgeState {
    /// Online, nothing to show: a dim outline.
    #[default]
    Idle,
    /// Loading or downloading: a hollow outline filling from the foot.
    Loading(f32),
    /// Ready (in a lobby): white.
    Ready,
    /// In a match: cobalt.
    InMatch,
}

/// The edge strip node.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct EdgeStrip {
    pub state: EdgeState,
    pub lead: bool,
}

/// Spawns the edge strip beside an avatar `size` cq tall: .17 of it wide,
/// the leader's cap .17 tall on top (module doc).
pub fn spawn_strip(ui: &Ui, c: &mut Commands, parent: Entity, images: &mut Assets<Image>, icons: &mut IconImages, state: EdgeState, lead: bool, size: f32) -> Entity {
    let u = ui.units;
    let side = u.snap(u.cq(size));
    let w = u.snap(side * 0.17).max(2.0);
    let gap = (2.0 / u.scale_factor).max(u.snap(side * 0.045));
    let strip = c
        .spawn((
            EdgeStrip { state, lead },
            Node { width: Val::Px(w), height: Val::Px(side), flex_direction: FlexDirection::Column, row_gap: Val::Px(gap), flex_shrink: 0.0, ..default() },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    if lead {
        let cap = c
            .spawn((
                Node { width: Val::Px(w), height: Val::Px(w), justify_content: JustifyContent::Center, align_items: AlignItems::Center, flex_shrink: 0.0, ..default() },
                BackgroundColor(ui.color(Tone::Fg, 1.0)),
                ChildOf(strip),
            ))
            .id();
        let px = u.phys(w * 0.52).round().max(1.0) as u32;
        let image = icons.get(images, "crown", px).unwrap_or_default();
        c.spawn((
            ImageNode { image, color: ui.color(Tone::Ink, 1.0), ..default() },
            Node { width: Val::Px(w * 0.72), height: Val::Px(px as f32 / u.scale_factor), ..default() },
            ChildOf(cap),
        ));
    }
    let hl = u.hairline() * 1.5;
    let (bg, outline) = match state {
        EdgeState::Ready => (ui.color(Tone::Fg, 1.0), None),
        EdgeState::InMatch => (ui.color(Tone::Acc, 1.0), None),
        EdgeState::Loading(_) => (Color::NONE, Some(ui.color(Tone::Fg2, 1.0))),
        EdgeState::Idle => (Color::NONE, Some(ui.color(Tone::Fg4, 1.0))),
    };
    let level = c
        .spawn((
            Node { flex_grow: 1.0, width: Val::Px(w), ..default() },
            BackgroundColor(bg),
            ChildOf(strip),
        ))
        .id();
    if let Some(o) = outline {
        c.entity(level).insert(Outline::new(Val::Px(hl), Val::Px(-hl), o));
    }
    if let EdgeState::Loading(f) = state {
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::ZERO,
                right: Val::ZERO,
                bottom: Val::ZERO,
                height: Val::Percent(f.clamp(0.0, 1.0) * 100.0),
                ..default()
            },
            BackgroundColor(ui.color(Tone::Fg2, 1.0)),
            ChildOf(level),
        ));
    }
    strip
}
