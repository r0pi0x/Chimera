//! `sound/materials.txt` (specs/goldsrc/mount.md, "`materials.txt`"):
//! lines of a surface letter and a texture name. `Materials::surface`
//! matches a face's texture name as the game does: the `-n`/`+n` prefix
//! and then one `{`, `!`, `~` or space dropped, both sides cut to 12
//! characters, compared ignoring case, the first entry winning; unlisted
//! names are concrete (`C`).

/// The surface of unlisted textures.
pub const DEFAULT_SURFACE: char = 'C';
/// Characters of a name the matching uses.
pub const NAME_CHARS: usize = 12;

/// The parsed list: (letter, name cut to 12 characters, lower case).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Materials {
    pub entries: Vec<(char, String)>,
}

fn cut(name: &str) -> String {
    name.chars().take(NAME_CHARS).collect::<String>().to_ascii_lowercase()
}

/// A texture name as matched: prefixes dropped, cut to 12 characters.
pub fn match_name(texture: &str) -> String {
    let mut s = texture;
    if let Some(rest) = s.strip_prefix(['-', '+']) {
        let mut c = rest.chars();
        c.next();
        s = c.as_str();
    }
    if let Some(rest) = s.strip_prefix(['{', '!', '~', ' ']) {
        s = rest;
    }
    cut(s)
}

impl Materials {
    pub fn parse(text: &str) -> Materials {
        let entries = text
            .lines()
            .filter_map(|l| {
                let l = l.split("//").next().unwrap_or("").trim();
                let mut w = l.split_whitespace();
                let letter = w.next()?.chars().next()?.to_ascii_uppercase();
                Some((letter, cut(w.next()?)))
            })
            .collect();
        Materials { entries }
    }

    /// The surface letter of a face's texture.
    pub fn surface(&self, texture: &str) -> char {
        let name = match_name(texture);
        self.entries
            .iter()
            .find(|(_, n)| *n == name)
            .map_or(DEFAULT_SURFACE, |(c, _)| *c)
    }
}
