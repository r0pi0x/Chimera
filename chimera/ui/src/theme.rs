//! The kit's look as data: `Theme`, a resource read from TOML
//! (docs/plans/active/ugg-ui.md, 2.3). Palette tokens and the alphas every
//! other colour derives from (`Tone`), the type roles (`TypeSpec`: face,
//! weight, tracking in em, case, OpenType features), named easings and
//! durations (`motion`), the icon rule (`icons.prefer_game`), the slant and the smallest text in physical
//! pixels (`shape`), and per-screen layout numbers in cq (`layout`).
//!
//! An app inserts its own (the Ultimate Gun Game's is
//! `apps/ultimate_gun_game/data/theme.toml`); without one, screens read
//! `Theme::shell_default` (data/shell.toml: Trebuchet MS and the shell's
//! present colours), so apps that set no theme look as before. A theme's
//! TOML is laid over the default's, so a theme names only what differs
//! and a key a screen asks for is always there.
//!
//! Colours are display-space (sRGB) values, and so are the theme's
//! alphas; Bevy's UI blends in linear light, so the derived tones carry
//! alphas that look as the theme's do: light ones over the dark inks
//! (`fg_2` ... `acc_glow`) through `light_alpha`, the plate (ink over the
//! world) through `display_alpha`. Code fading a tone by hand applies the
//! same (`Theme::tone_a` does).
//!
//! Known shortcuts: a missing layout number or duration reads as 0 with a
//! warning instead of failing the theme's load.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::Deserialize;

use crate::motion::Ease;

/// The default theme's TOML (also the base every other theme lays over).
pub const SHELL_TOML: &str = include_str!("../data/shell.toml");

/// A colour token: the palette's own and the ones derived from them by
/// the theme's alphas (the prototype's `--fg-2` ... `--plate`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tone {
    Ink,
    Ink2,
    Ink3,
    Ink4,
    Fg,
    /// `fg` at `alpha.fg_2` (secondary text).
    Fg2,
    /// `fg` at `alpha.fg_3` (labels).
    Fg3,
    /// `fg` at `alpha.fg_4` (rules, unlit ticks).
    Fg4,
    /// `fg` at `alpha.fg_5` (faint fills).
    Fg5,
    Acc,
    /// `acc` at `alpha.acc_2` (accent fills).
    Acc2,
    /// `acc` at `alpha.acc_glow`.
    AccGlow,
    /// Text on an accent fill.
    OnAcc,
    /// Accent-coloured text (lighter than `acc` on ink).
    AccText,
    Threat,
    /// Light green: network good only (a quiet ping).
    Good,
    /// Amber: network warnings only (a bad ping), never gameplay.
    Warn,
    /// `ink` at `alpha.plate` (panels; no backdrop blur in Bevy).
    Plate,
    /// `fg` at `alpha.glow`.
    Glow,
}

impl Tone {
    /// Every tone, for tests and tools.
    pub const ALL: [Tone; 19] = [
        Tone::Ink,
        Tone::Ink2,
        Tone::Ink3,
        Tone::Ink4,
        Tone::Fg,
        Tone::Fg2,
        Tone::Fg3,
        Tone::Fg4,
        Tone::Fg5,
        Tone::Acc,
        Tone::Acc2,
        Tone::AccGlow,
        Tone::OnAcc,
        Tone::AccText,
        Tone::Threat,
        Tone::Good,
        Tone::Warn,
        Tone::Plate,
        Tone::Glow,
    ];

    /// The token's name in a theme's TOML and the prototype (`fg_2`).
    pub fn name(self) -> &'static str {
        match self {
            Tone::Ink => "ink",
            Tone::Ink2 => "ink_2",
            Tone::Ink3 => "ink_3",
            Tone::Ink4 => "ink_4",
            Tone::Fg => "fg",
            Tone::Fg2 => "fg_2",
            Tone::Fg3 => "fg_3",
            Tone::Fg4 => "fg_4",
            Tone::Fg5 => "fg_5",
            Tone::Acc => "acc",
            Tone::Acc2 => "acc_2",
            Tone::AccGlow => "acc_glow",
            Tone::OnAcc => "on_acc",
            Tone::AccText => "acc_text",
            Tone::Threat => "threat",
            Tone::Good => "good",
            Tone::Warn => "warn",
            Tone::Plate => "plate",
            Tone::Glow => "glow",
        }
    }

    pub fn from_name(name: &str) -> Option<Tone> {
        Tone::ALL.into_iter().find(|t| t.name() == name)
    }
}

/// A text role (`fonts::Role` picks one by name): which face, how heavy,
/// how tracked.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TypeSpec {
    /// `display` or `mono` (the kit's faces, `fonts::KitFonts`), or a
    /// family name found among the loaded fonts (`trebuchet ms`).
    pub face: String,
    /// 1-1000; with the kit's faces it also picks the face (400, 700, 900).
    #[serde(default = "default_weight")]
    pub weight: u16,
    /// Letter spacing in em (`LetterSpacing::Px(tracking * size)`).
    #[serde(default)]
    pub tracking: f32,
    /// Upper-case the text (the UGG faces map lower case to capitals
    /// themselves; a fallback face needs this).
    #[serde(default)]
    pub upper: bool,
    /// OpenType features turned on, four-letter tags (`ss02`).
    #[serde(default)]
    pub features: Vec<String>,
    /// Hinting on (small Mono; risk 1 in the plan).
    #[serde(default)]
    pub hinted: bool,
}

fn default_weight() -> u16 {
    400
}

/// The slant and the smallest text.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Shape {
    /// The one slant, degrees (12).
    pub slant_deg: f32,
    /// No text under this many physical pixels (the prototype's
    /// `max(9px, ...)`).
    pub min_text_px: f32,
}

#[derive(Clone, Debug, Deserialize)]
struct Raw {
    name: String,
    palette: BTreeMap<String, String>,
    alpha: BTreeMap<String, f32>,
    shape: Shape,
    #[serde(rename = "type")]
    types: BTreeMap<String, TypeSpec>,
    motion: RawMotion,
    #[serde(default)]
    layout: BTreeMap<String, BTreeMap<String, f32>>,
    icons: IconRule,
    network: Network,
}

/// The ping meter's thresholds and its quiet bars' alpha.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Network {
    /// Under this many ms: quiet (four bars, `good`).
    pub ping_quiet_ms: u32,
    /// Under this: fine (three bars, `fg`).
    pub ping_fine_ms: u32,
    /// Under this: a warning (two bars, `warn`); at or over: bad (one, `threat`).
    pub ping_warn_ms: u32,
    /// The quiet bars' alpha (display space).
    pub good_alpha: f32,
}

/// Which icons the theme's screens show (`icons::choose`).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct IconRule {
    /// Each game's own kill icon wherever it has one, ours only where none
    /// (plan section 6, item 1); off: ours everywhere.
    pub prefer_game: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct RawMotion {
    ease: BTreeMap<String, String>,
    #[serde(default)]
    dur: BTreeMap<String, BTreeMap<String, f32>>,
}

/// The kit's look (module doc). Read with `Theme::from_toml`; cheap to
/// clone.
#[derive(Resource, Clone, Debug)]
pub struct Theme {
    /// The theme's name (`shell`, `signal`).
    pub name: String,
    tones: BTreeMap<Tone, Color>,
    alphas: BTreeMap<String, f32>,
    types: BTreeMap<String, TypeSpec>,
    pub shape: Shape,
    eases: BTreeMap<String, Ease>,
    durs: BTreeMap<String, BTreeMap<String, f32>>,
    layout: BTreeMap<String, BTreeMap<String, f32>>,
    pub icons: IconRule,
    pub network: Network,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::shell_default()
    }
}

/// `b` laid over `a`, table by table.
fn overlay(a: &mut toml::Table, b: toml::Table) {
    for (k, v) in b {
        match (a.get_mut(&k), v) {
            (Some(toml::Value::Table(at)), toml::Value::Table(bt)) => overlay(at, bt),
            (_, v) => {
                a.insert(k, v);
            }
        }
    }
}

fn parse_hex(s: &str) -> Result<Color, String> {
    Srgba::hex(s.trim()).map(Color::Srgba).map_err(|e| format!("colour `{s}`: {e}"))
}

impl Theme {
    /// The shell's own look (data/shell.toml): Trebuchet MS, today's
    /// colours. What screens use when the app inserts no theme.
    pub fn shell_default() -> Theme {
        Theme::from_toml(SHELL_TOML).expect("chimera_ui's data/shell.toml is a valid theme")
    }

    /// A theme from its TOML, laid over the default's (module doc).
    pub fn from_toml(text: &str) -> Result<Theme, String> {
        let mut base: toml::Table = SHELL_TOML.parse().map_err(|e| format!("shell.toml: {e}"))?;
        let over: toml::Table = text.parse().map_err(|e| format!("theme: {e}"))?;
        overlay(&mut base, over);
        let raw: Raw = toml::Value::Table(base).try_into().map_err(|e| format!("theme: {e}"))?;
        let pal = |k: &str| -> Result<Color, String> {
            parse_hex(raw.palette.get(k).ok_or_else(|| format!("theme: palette.{k} missing"))?)
        };
        let alpha = |k: &str| -> Result<f32, String> {
            raw.alpha.get(k).copied().ok_or_else(|| format!("theme: alpha.{k} missing"))
        };
        let (fg, acc, ink) = (pal("fg")?, pal("acc")?, pal("ink")?);
        let mut tones = BTreeMap::new();
        for t in Tone::ALL {
            let c = match t {
                Tone::Fg2 | Tone::Fg3 | Tone::Fg4 | Tone::Fg5 | Tone::Glow => fg.with_alpha(light_alpha(alpha(t.name())?)),
                Tone::Acc2 | Tone::AccGlow => acc.with_alpha(light_alpha(alpha(t.name())?)),
                Tone::Plate => display_alpha(ink.with_alpha(alpha(t.name())?)),
                _ => pal(t.name())?,
            };
            tones.insert(t, c);
        }
        let mut eases = BTreeMap::new();
        for (k, v) in &raw.motion.ease {
            eases.insert(k.clone(), Ease::parse(v).map_err(|e| format!("theme: motion.ease.{k}: {e}"))?);
        }
        for (name, spec) in &raw.types {
            for f in &spec.features {
                if f.len() != 4 || !f.is_ascii() {
                    return Err(format!("theme: type.{name}: feature `{f}` is no four-letter tag"));
                }
            }
        }
        Ok(Theme {
            alphas: raw.alpha.clone(),
            name: raw.name,
            tones,
            types: raw.types,
            shape: raw.shape,
            eases,
            durs: raw.motion.dur,
            layout: raw.layout,
            icons: raw.icons,
            network: raw.network,
        })
    }

    /// A colour token.
    pub fn tone(&self, t: Tone) -> Color {
        self.tones[&t]
    }

    /// A token faded to `a` in display space: `light_alpha` for the light
    /// tones, the dark ones (inks, plate, `on_acc`) as `display_alpha`.
    pub fn tone_a(&self, t: Tone, a: f32) -> Color {
        let c = self.tone(t);
        let k = match t {
            Tone::Ink | Tone::Ink2 | Tone::Ink3 | Tone::Ink4 | Tone::Plate | Tone::OnAcc => {
                1.0 - (1.0 - a.clamp(0.0, 1.0)).powf(2.2)
            }
            _ => light_alpha(a),
        };
        c.with_alpha(c.alpha() * k)
    }

    /// A token alpha as the theme writes it (display space: `fg_2` .66),
    /// before `light_alpha`/`display_alpha`.
    pub fn alpha(&self, name: &str) -> Option<f32> {
        self.alphas.get(name).copied()
    }

    /// A text role's spec by name (`wordmark`, `display_bold`, `mono`, ...).
    pub fn type_spec(&self, role: &str) -> Option<&TypeSpec> {
        self.types.get(role)
    }

    /// tan of the slant (0.2126 at 12°).
    pub fn tan(&self) -> f32 {
        self.shape.slant_deg.to_radians().tan()
    }

    /// A named easing (`out`, `in`, `io`, `pop`, `expo_out`, ...);
    /// `Ease::Linear` with a warning when the theme has none by that name.
    pub fn ease(&self, name: &str) -> Ease {
        match self.eases.get(name) {
            Some(e) => *e,
            None => Ease::parse(name).unwrap_or_else(|_| {
                warn!("theme `{}`: no easing `{name}`", self.name);
                Ease::Linear
            }),
        }
    }

    /// A piece's duration or offset in seconds (`motion.dur.<piece>.<key>`).
    pub fn dur(&self, piece: &str, key: &str) -> f32 {
        self.durs.get(piece).and_then(|p| p.get(key)).copied().unwrap_or_else(|| {
            warn!("theme `{}`: no motion.dur.{piece}.{key}", self.name);
            0.0
        })
    }

    /// A screen's layout number in cq (`layout.<screen>.<key>`).
    pub fn layout(&self, screen: &str, key: &str) -> f32 {
        self.layout_opt(screen, key).unwrap_or_else(|| {
            warn!("theme `{}`: no layout.{screen}.{key}", self.name);
            0.0
        })
    }

    pub fn layout_opt(&self, screen: &str, key: &str) -> Option<f32> {
        self.layout.get(screen).and_then(|s| s.get(key)).copied()
    }
}

/// The alpha that blends a light colour over a dark one in linear light
/// (Bevy's UI) as `a` does in display space (the look book's): `a^2.2`.
/// Exact over black, close over the theme's inks: `fg` at .66 shows as
/// the prototype's `--fg-2`, `fg` at .04 as its faint hairlines (blended
/// as given, they would show about five times as bright).
pub fn light_alpha(a: f32) -> f32 {
    a.clamp(0.0, 1.0).powf(2.2)
}

/// `color` with its alpha raised so it covers what is under it as much as
/// that alpha does blended in display space (Bevy's UI blends in linear
/// light): `1 - (1 - a)^2.2`. The same rule as the shell's
/// `widgets::display_alpha`.
pub fn display_alpha(color: Color) -> Color {
    let a = color.alpha().clamp(0.0, 1.0);
    color.with_alpha(1.0 - (1.0 - a).powf(2.2))
}
