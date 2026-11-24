//! `liblist.gam` and the content folder chain (specs/goldsrc/mount.md):
//! `LibList::parse` reads the `key "value"` lines (keys ignoring case,
//! `//` comments, trailing text after the closing quote ignored);
//! `LibList::folder_chain` gives the folders searched, highest priority
//! first: the language folder when asked for, the game folder, then the
//! fallback (`fallback_dir`, else `valve`). The `_addon`, `_downloads`,
//! `_hd` and `_lv` folders are left out on purpose.

/// The fallback folder when `liblist.gam` names none.
pub const DEFAULT_FALLBACK: &str = "valve";

/// A parsed `liblist.gam`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LibList {
    pub pairs: Vec<(String, String)>,
}

impl LibList {
    pub fn parse(text: &str) -> LibList {
        let mut pairs = Vec::new();
        for line in text.lines() {
            let line = line.split("//").next().unwrap_or("").trim();
            let Some((key, rest)) = line.split_once(|c: char| c.is_whitespace()) else {
                continue;
            };
            let rest = rest.trim_start();
            let value = match rest.strip_prefix('"') {
                Some(r) => r.split('"').next().unwrap_or(""),
                None => rest.split_whitespace().next().unwrap_or(""),
            };
            pairs.push((key.to_string(), value.to_string()));
        }
        LibList { pairs }
    }

    /// A key's value, ignoring case (the last occurrence wins).
    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs
            .iter()
            .rev()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    /// The game's display name (`game`).
    pub fn game(&self) -> Option<&str> {
        self.get("game")
    }

    /// The fallback folder: `fallback_dir`, else `valve`.
    pub fn fallback(&self) -> &str {
        self.get("fallback_dir")
            .filter(|v| !v.is_empty())
            .unwrap_or(DEFAULT_FALLBACK)
    }

    /// The default multiplayer spawn class (`mpentity`).
    pub fn mp_entity(&self) -> Option<&str> {
        self.get("mpentity")
    }

    /// The folders a file is looked up in, highest priority first, for the
    /// game folder `game` (`cstrike`) and an optional language
    /// (`<game>_<language>`, client installs only). The fallback is not
    /// repeated when it is the game folder itself.
    pub fn folder_chain(&self, game: &str, language: Option<&str>) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(l) = language.filter(|l| !l.is_empty() && !l.eq_ignore_ascii_case("english")) {
            out.push(format!("{game}_{l}"));
        }
        out.push(game.to_string());
        if !self.fallback().eq_ignore_ascii_case(game) {
            out.push(self.fallback().to_string());
        }
        out
    }
}
