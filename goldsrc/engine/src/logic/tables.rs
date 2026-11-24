//! The fixed tables of specs/goldsrc/entities.md: sounds by index (doors,
//! buttons, platforms, breakables' materials), the light styles' preset
//! patterns, and how bright a style's letter is.

/// A door's moving sound (`movesnd`), relative to `sound/`.
pub fn door_move(index: i32) -> Option<String> {
    (1..=10).contains(&index).then(|| format!("doors/doormove{index}.wav"))
}

/// A door's stop sound (`stopsnd`).
pub fn door_stop(index: i32) -> Option<String> {
    (1..=8).contains(&index).then(|| format!("doors/doorstop{index}.wav"))
}

/// The button-sound table (`sounds`, `locked_sound`, `unlocked_sound`).
pub fn button(index: i32) -> Option<String> {
    Some(match index {
        0 => return None,
        1..=11 => format!("buttons/button{index}.wav"),
        12 => "buttons/latchlocked1.wav".into(),
        13 => "buttons/latchunlocked1.wav".into(),
        14 => "buttons/lightswitch2.wav".into(),
        21..=25 => format!("buttons/lever{}.wav", index - 20),
        _ => "buttons/button9.wav".into(),
    })
}

/// A train's moving sound (`movesnd`, the platform table).
pub fn plat_move(index: i32) -> Option<String> {
    let name = match index {
        1 => "bigmove1",
        2 => "bigmove2",
        3 => "elevmove1",
        4 => "elevmove2",
        5 => "elevmove3",
        6 => "freightmove1",
        7 => "freightmove2",
        8 => "heavymove1",
        9 => "rackmove1",
        10 => "railmove1",
        11 => "squeekmove1",
        12 => "talkmove1",
        13 => "talkmove2",
        _ => return None,
    };
    Some(format!("plats/{name}.wav"))
}

/// A train's stop sound (`stopsnd`, the platform table).
pub fn plat_stop(index: i32) -> Option<String> {
    let name = match index {
        1 => "bigstop1",
        2 => "bigstop2",
        3 => "freightstop1",
        4 => "heavystop2",
        5 => "rackstop1",
        6 => "railstop1",
        7 => "squeekstop1",
        8 => "talkstop1",
        _ => return None,
    };
    Some(format!("plats/{name}.wav"))
}

/// A breakable's material (`material`; out of range: wood).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Glass,
    Wood,
    Metal,
    Flesh,
    CinderBlock,
    CeilingTile,
    Computer,
    UnbreakableGlass,
    Rocks,
}

impl Material {
    pub fn of(index: i32) -> Self {
        match index {
            0 => Self::Glass,
            2 => Self::Metal,
            3 => Self::Flesh,
            4 => Self::CinderBlock,
            5 => Self::CeilingTile,
            6 => Self::Computer,
            7 => Self::UnbreakableGlass,
            8 => Self::Rocks,
            _ => Self::Wood,
        }
    }

    /// Its break sounds (one is picked).
    pub fn break_sounds(self) -> &'static [&'static str] {
        match self {
            Self::Glass | Self::UnbreakableGlass => &["debris/bustglass1.wav", "debris/bustglass2.wav"],
            Self::Wood => &["debris/bustcrate1.wav", "debris/bustcrate2.wav"],
            Self::Metal | Self::Computer => &["debris/bustmetal1.wav", "debris/bustmetal2.wav"],
            Self::Flesh => &["debris/bustflesh1.wav", "debris/bustflesh2.wav"],
            Self::Rocks | Self::CinderBlock => &["debris/bustconcrete1.wav", "debris/bustconcrete2.wav"],
            Self::CeilingTile => &["debris/bustceiling.wav"],
        }
    }

    /// Its debris model when the breakable names none (`gibmodel`).
    pub fn gibs(self) -> &'static str {
        match self {
            Self::Glass | Self::UnbreakableGlass => "models/glassgibs.mdl",
            Self::Wood => "models/woodgibs.mdl",
            Self::Metal => "models/metalplategibs.mdl",
            Self::Flesh => "models/fleshgibs.mdl",
            Self::CinderBlock => "models/cindergibs.mdl",
            Self::CeilingTile => "models/ceilinggibs.mdl",
            Self::Computer => "models/computergibs.mdl",
            Self::Rocks => "models/rockgibs.mdl",
        }
    }
}

/// The patterns of light styles 0-12 and 63 the game sets at map start
/// (entities.md, "light styles"); other styles: None.
pub fn preset_style(style: u8) -> Option<&'static str> {
    Some(match style {
        0 => "m",
        1 => "mmnmmommommnonmmonqnmmo",
        2 => "abcdefghijklmnopqrstuvwxyzyxwvutsrqponmlkjihgfedcba",
        3 => "mmmmmaaaaammmmmaaaaaabcdefgabcdefg",
        4 => "mamamamamama",
        5 => "jklmnopqrstuvwxyzyxwvutsrqponmlkj",
        6 => "nmonqnmomnmomomno",
        7 => "mmmaaaabcdefgmmmmaaaammmaamm",
        8 => "mmmaaammmaaammmabcdefaaaammmmabcdefmmmaaaa",
        9 => "aaaaaaaazzzzzzzz",
        10 => "mmamammmmammamamaaamammma",
        11 => "abcdefghijklmnopqrrqponmlkjihgfedcba",
        12 => "mmnnmmnnnmmnn",
        63 => "a",
        _ => return None,
    })
}

/// Letters a second a style's pattern advances (client side).
pub const STYLE_RATE: f64 = 10.0;

/// The letter of `pattern` at client time `t` (seconds), looping.
pub fn style_letter(pattern: &str, t: f64) -> u8 {
    let bytes = pattern.as_bytes();
    if bytes.is_empty() {
        return b'm';
    }
    bytes[((t * STYLE_RATE) as usize) % bytes.len()]
}

/// A style letter's brightness, `m` 1 (normal), `a` 0, linear in the
/// letter (the exact scale is the spec's open question 11).
pub fn letter_brightness(letter: u8) -> f32 {
    f32::from(letter.clamp(b'a', b'z') - b'a') / 12.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_tables() {
        assert_eq!(door_move(6).as_deref(), Some("doors/doormove6.wav"));
        assert_eq!(door_move(11), None);
        assert_eq!(door_stop(9), None);
        assert_eq!(button(24).as_deref(), Some("buttons/lever4.wav"));
        assert_eq!(button(13).as_deref(), Some("buttons/latchunlocked1.wav"));
        assert_eq!(button(40).as_deref(), Some("buttons/button9.wav"));
        assert_eq!(button(0), None);
        assert_eq!(plat_move(3).as_deref(), Some("plats/elevmove1.wav"));
        assert_eq!(Material::of(42), Material::Wood);
        assert_eq!(Material::of(2).gibs(), "models/metalplategibs.mdl");
    }

    #[test]
    fn styles() {
        assert_eq!(preset_style(10), Some("mmamammmmammamamaaamammma"));
        assert_eq!(preset_style(32), None);
        assert_eq!(style_letter("ab", 0.15), b'b');
        assert_eq!(style_letter("ab", 0.25), b'a');
        assert_eq!(letter_brightness(b'm'), 1.0);
        assert_eq!(letter_brightness(b'a'), 0.0);
    }
}
