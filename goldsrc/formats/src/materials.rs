//! `sound/materials.txt`: lines of a surface letter and a texture name.
//! `Materials::surface` matches a face's texture name as a game does; two
//! rules (`Rule`):
//!
//! - **Half-Life's** (specs/goldsrc/mount.md, "`materials.txt`"): the
//!   `-n`/`+n` prefix and then one `{`, `!`, `~` or space dropped, both
//!   sides cut to 12 characters, compared ignoring case, the first entry
//!   winning;
//! - **Counter-Strike 1.6's** (specs/cs16/footsteps.md, "The surface
//!   type"): the same prefix handling, up to 1024 entries read, names kept
//!   to 16 characters, a line counting only when it starts with a letter
//!   and has a name ended by whitespace (a last line with no line break is
//!   dropped), the entries sorted by name ignoring case and searched by
//!   halving.
//!
//! Unlisted names are concrete (`C`).

/// The surface of unlisted textures.
pub const DEFAULT_SURFACE: char = 'C';
/// Characters of a name Half-Life's matching uses.
pub const NAME_CHARS: usize = 12;
/// Characters of a name Counter-Strike 1.6's matching uses.
pub const CS16_NAME_CHARS: usize = 16;
/// Entries Counter-Strike 1.6 reads.
pub const CS16_ENTRIES: usize = 1024;

/// How a game reads and matches the list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rule {
    /// 12 characters, every line, the first match.
    #[default]
    HalfLife,
    /// 16 characters, 1024 entries, a sorted search.
    Cs16,
}

impl Rule {
    /// Characters of a name this rule keeps.
    pub fn chars(self) -> usize {
        match self {
            Rule::HalfLife => NAME_CHARS,
            Rule::Cs16 => CS16_NAME_CHARS,
        }
    }
}

/// The parsed list: (letter, name cut to the rule's length, lower case);
/// for Counter-Strike 1.6 in sorted order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Materials {
    pub entries: Vec<(char, String)>,
    pub rule: Rule,
}

fn cut(name: &str, chars: usize) -> String {
    name.chars().take(chars).collect::<String>().to_ascii_lowercase()
}

/// A texture name with its prefixes dropped (`-0`, `+a`, then one `{`,
/// `!`, `~` or space), not cut.
pub fn strip_name(texture: &str) -> &str {
    let mut s = texture;
    if let Some(rest) = s.strip_prefix(['-', '+']) {
        let mut c = rest.chars();
        c.next();
        s = c.as_str();
    }
    if let Some(rest) = s.strip_prefix(['{', '!', '~', ' ']) {
        s = rest;
    }
    s
}

/// A texture name as Half-Life matches it: prefixes dropped, cut to 12
/// characters.
pub fn match_name(texture: &str) -> String {
    cut(strip_name(texture), NAME_CHARS)
}

impl Materials {
    /// The list under Half-Life's rule.
    pub fn parse(text: &str) -> Materials {
        Self::parse_with(text, Rule::HalfLife)
    }

    /// The list under `rule`.
    pub fn parse_with(text: &str, rule: Rule) -> Materials {
        match rule {
            Rule::HalfLife => {
                let entries = text
                    .lines()
                    .filter_map(|l| {
                        let l = l.split("//").next().unwrap_or("").trim();
                        let mut w = l.split_whitespace();
                        let letter = w.next()?.chars().next()?.to_ascii_uppercase();
                        Some((letter, cut(w.next()?, NAME_CHARS)))
                    })
                    .collect();
                Materials { entries, rule }
            }
            Rule::Cs16 => Materials { entries: cs16_entries(text), rule },
        }
    }

    /// The surface letter of a face's texture.
    pub fn surface(&self, texture: &str) -> char {
        let name = cut(strip_name(texture), self.rule.chars());
        let found = match self.rule {
            Rule::HalfLife => self.entries.iter().find(|(_, n)| *n == name),
            Rule::Cs16 => halve(&self.entries, &name),
        };
        found.map_or(DEFAULT_SURFACE, |(c, _)| *c)
    }
}

/// Counter-Strike 1.6's entries: those lines whose first non-space
/// character is a letter, followed by whitespace, a name and whitespace
/// after it (the line's own break counts: a last line without one is
/// dropped); at most 1024; sorted by name.
fn cs16_entries(text: &str) -> Vec<(char, String)> {
    let mut entries = Vec::new();
    let mut rest = text;
    while entries.len() < CS16_ENTRIES {
        let Some(end) = rest.find('\n') else { break };
        let line = &rest[..end];
        rest = &rest[end + 1..];
        let line = line.trim_start_matches([' ', '\t', '\r']);
        let mut chars = line.chars();
        let Some(letter) = chars.next().filter(char::is_ascii_alphabetic) else { continue };
        let after = chars.as_str();
        // Whitespace, then a name ended by whitespace (the break ends it).
        if !after.starts_with([' ', '\t']) {
            continue;
        }
        let name: String = after
            .trim_start_matches([' ', '\t'])
            .chars()
            .take_while(|c| !c.is_whitespace())
            .collect();
        if name.is_empty() {
            continue;
        }
        entries.push((letter.to_ascii_uppercase(), cut(&name, CS16_NAME_CHARS)));
    }
    entries.sort_by(|a, b| a.1.cmp(&b.1));
    entries
}

/// The entry named `name` in the sorted `entries`, by halving: of several
/// of one name, the one the search lands on.
fn halve<'a>(entries: &'a [(char, String)], name: &str) -> Option<&'a (char, String)> {
    let (mut lo, mut hi) = (0, entries.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        match name.cmp(entries[mid].1.as_str()) {
            std::cmp::Ordering::Equal => return Some(&entries[mid]),
            std::cmp::Ordering::Less => hi = mid,
            std::cmp::Ordering::Greater => lo = mid + 1,
        }
    }
    None
}
