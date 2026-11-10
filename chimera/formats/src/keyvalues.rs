//! KeyValues text: Valve's `"key" "value"` / `"name" { ... }` files, read by
//! one tokenizer (`tokens`) and a tree (`Kv`) with its parsers.
//!
//! Escapes are an explicit choice per file type (`Escapes`), not a property
//! of whichever tokenizer a module happened to import:
//!
//! - `Escapes::On`: a `\` in a quoted string takes the next character
//!   literally (`\"`, `\\`). Steam's own files (`libraryfolders.vdf`,
//!   `appmanifest_*.acf`, whose Windows paths are written `C:\\Steam`) and
//!   our saved files (`serverbrowser.vdf`); `parse` reads them.
//! - `Escapes::Off`: a `\` is an ordinary character and a quoted string
//!   ends at the next `"`. The game's script and resource files as we read
//!   them today (materials, surface properties, `.res` layouts, schemes,
//!   manifests, sound scripts, `.scr` option scripts): their Windows paths
//!   are written with single backslashes. `parse_res` and `parse_pc` read
//!   them.
//!
//! Which game files the engine itself reads with escapes on is a parity
//! question for a spec session (docs/plans/active/architecture-review.md,
//! P7); until a spec says otherwise each caller keeps the behaviour it
//! had.
//!
//! Localization files (`resource/*_english.txt`) have their own escapes
//! (`\n`, `\t`) and live in `localization`.

/// Whether `\` escapes the next character inside quoted strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Escapes {
    /// `\x` is `x` (`\"`, `\\`): Steam's files and our own.
    On,
    /// `\` is literal: the game's scripts and resource files.
    Off,
}

/// The tokens of KeyValues text: quoted strings (quotes removed, escapes
/// applied when `escapes` is on), bare words (ended by whitespace, a quote
/// or a brace), `{` and `}`; `//` comments skipped.
pub fn tokens(text: &str, escapes: Escapes) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' | '}' => out.push(c.to_string()),
            '"' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' if escapes == Escapes::On => {
                            if let Some(n) = chars.next() {
                                s.push(n);
                            }
                        }
                        '"' => break,
                        _ => s.push(c),
                    }
                }
                out.push(s);
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            c if c.is_whitespace() => {}
            c => {
                let mut s = c.to_string();
                while let Some(&n) = chars.peek() {
                    if n.is_whitespace() || n == '{' || n == '}' || n == '"' {
                        break;
                    }
                    s.push(n);
                    chars.next();
                }
                out.push(s);
            }
        }
    }
    out
}

/// A KeyValues tree: each key holds a string or a block of named children,
/// in file order (a key may repeat).
#[derive(Clone, Debug, PartialEq)]
pub enum Kv {
    Value(String),
    Block(Vec<(String, Kv)>),
}

impl Kv {
    /// The first child named `key` (any case), in a block.
    pub fn get(&self, key: &str) -> Option<&Kv> {
        match self {
            Kv::Block(items) => items.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            Kv::Value(_) => None,
        }
    }

    /// The value of child `key`, when it is a value.
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Kv::Value(v) => Some(v),
            Kv::Block(_) => None,
        }
    }

    /// This value's text (None for a block).
    pub fn str_value(&self) -> Option<&str> {
        match self {
            Kv::Value(v) => Some(v),
            Kv::Block(_) => None,
        }
    }

    /// A block's children (empty for a value).
    pub fn items(&self) -> &[(String, Kv)] {
        match self {
            Kv::Block(items) => items,
            Kv::Value(_) => &[],
        }
    }
}

/// Parse KeyValues text into a block of its top-level entries. A key with
/// nothing after it, or right before a `}`, is dropped; a stray `}` at the
/// top level ends the text.
pub fn parse(text: &str, escapes: Escapes) -> Kv {
    fn block(t: &[String], i: &mut usize) -> Vec<(String, Kv)> {
        let mut out = Vec::new();
        while *i < t.len() {
            if t[*i] == "}" {
                *i += 1;
                break;
            }
            let key = t[*i].clone();
            *i += 1;
            match t.get(*i).map(String::as_str) {
                Some("{") => {
                    *i += 1;
                    out.push((key, Kv::Block(block(t, i))));
                }
                Some("}") | None => {}
                Some(v) => {
                    out.push((key, Kv::Value(v.to_string())));
                    *i += 1;
                }
            }
        }
        out
    }
    let t = tokens(text, escapes);
    let mut i = 0;
    Kv::Block(block(&t, &mut i))
}

/// Parse a game script or resource file (escapes off; `.res` layouts,
/// schemes, `propdata.txt`, ...): platform conditionals (`[$WIN32]`) are
/// dropped, whatever they say. Unlike `parse`, a key right before a `}`
/// takes the `}` as its value (kept as the files have always been read).
pub fn parse_res(text: &str) -> Kv {
    let tokens: Vec<String> = tokens(text, Escapes::Off)
        .into_iter()
        .filter(|t| !(t.starts_with('[') && t.ends_with(']')))
        .collect();
    fn block(t: &[String], i: &mut usize) -> Kv {
        let mut items = Vec::new();
        while *i < t.len() && t[*i] != "}" {
            let key = t[*i].clone();
            *i += 1;
            if *i >= t.len() {
                break;
            }
            if t[*i] == "{" {
                *i += 1;
                let b = block(t, i);
                *i += 1; // the closing brace
                items.push((key, b));
            } else {
                items.push((key, Kv::Value(t[*i].clone())));
                *i += 1;
            }
        }
        Kv::Block(items)
    }
    let mut i = 0;
    block(&tokens, &mut i)
}

/// Whether a platform condition (`$WIN32`, `!$OSX`, `$X360 || $PS3`)
/// holds for the Windows PC game.
pub fn condition_holds(cond: &str) -> bool {
    let c = cond.trim().trim_start_matches('[').trim_end_matches(']');
    c.split("||").any(|any| {
        any.split("&&").all(|term| {
            let term = term.trim();
            let (neg, name) = match term.strip_prefix('!') {
                Some(n) => (true, n),
                None => (false, term),
            };
            let name = name.trim().trim_start_matches('$').to_uppercase();
            let on = matches!(name.as_str(), "WIN32" | "WINDOWS");
            on != neg
        })
    })
}

/// A game script or resource file as the Windows PC game reads it
/// (escapes off): an entry followed by a platform condition that doesn't
/// hold there is left out.
pub fn parse_pc(text: &str) -> Kv {
    let t = tokens(text, Escapes::Off);
    let cond = |t: &[String], i: usize| t.get(i).filter(|s| s.starts_with('[') && s.ends_with(']')).cloned();
    fn block(t: &[String], i: &mut usize, cond: &dyn Fn(&[String], usize) -> Option<String>) -> Kv {
        let mut items = Vec::new();
        while *i < t.len() && t[*i] != "}" {
            let key = t[*i].clone();
            *i += 1;
            let mut keep = true;
            // A condition between a block's name and its brace.
            if let Some(c) = cond(t, *i) {
                keep &= condition_holds(&c);
                *i += 1;
            }
            if *i >= t.len() {
                break;
            }
            let value = if t[*i] == "{" {
                *i += 1;
                let b = block(t, i, cond);
                *i += 1; // the closing brace
                b
            } else {
                let v = Kv::Value(t[*i].clone());
                *i += 1;
                v
            };
            if let Some(c) = cond(t, *i) {
                keep &= condition_holds(&c);
                *i += 1;
            }
            if keep {
                items.push((key, value));
            }
        }
        Kv::Block(items)
    }
    let mut i = 0;
    block(&t, &mut i, &cond)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn escapes_blocks_and_comments() {
        let root = parse(
            "// c\n\"a\" { \"path\" \"C:\\\\Steam \\\"x\\\"\" b { c d } }",
            Escapes::On,
        );
        let a = root.get("A").unwrap();
        assert_eq!(a.str("path"), Some("C:\\Steam \"x\""));
        assert_eq!(a.get("b").unwrap().str("c"), Some("d"));
        assert_eq!(a.items().len(), 2);
    }

    /// The two modes agree on everything but a `\` inside quotes.
    #[test]
    fn tokens_with_and_without_escapes() {
        let common = "// comment \"x\"\n\"a\" { b \"c d\"\n\tlist{x}  bare\"q\" }";
        let want = s(&["a", "{", "b", "c d", "list", "{", "x", "}", "bare", "q", "}"]);
        assert_eq!(tokens(common, Escapes::On), want);
        assert_eq!(tokens(common, Escapes::Off), want);

        // Steam writes Windows paths with doubled backslashes and escapes
        // quotes (libraryfolders.vdf); our saved files too.
        let steam = r#""path" "C:\\Program Files (x86)\\Steam" "name" "say \"hi\"""#;
        assert_eq!(
            tokens(steam, Escapes::On),
            s(&["path", r"C:\Program Files (x86)\Steam", "name", "say \"hi\""])
        );

        // The game's scripts write single backslashes, kept as written
        // (a material's `$basetexture` path, a sound script's wave).
        let script = r#""$basetexture" "models\props\crate" "wave" "weapons\ak47\ak47-1.wav""#;
        assert_eq!(
            tokens(script, Escapes::Off),
            s(&[
                "$basetexture",
                r"models\props\crate",
                "wave",
                r"weapons\ak47\ak47-1.wav"
            ])
        );
        // With escapes on, the same text would lose its backslashes.
        assert_eq!(tokens(script, Escapes::On)[1], "modelspropscrate");

        // A backslash before a closing quote: off ends the string there,
        // on escapes the quote.
        assert_eq!(tokens(r#""dir\" next"#, Escapes::Off), s(&["dir\\", "next"]));
        assert_eq!(tokens(r#""dir\" next"#, Escapes::On), s(&["dir\" next"]));
        // An unterminated string runs to the end.
        assert_eq!(tokens("\"open", Escapes::Off), s(&["open"]));
    }

    #[test]
    fn steam_and_res_trees_differ_only_at_the_edges() {
        let text = "\"root\" { \"k\" \"v\" \"blk\" { \"x\" \"1\" } }";
        assert_eq!(parse(text, Escapes::Off), parse_res(text));
        // A key right before a closing brace: dropped by `parse`, the brace
        // as its value in `parse_res`.
        let dangling = "\"root\" { \"k\" }";
        assert!(parse(dangling, Escapes::Off).get("root").unwrap().items().is_empty());
        assert_eq!(parse_res(dangling).get("root").unwrap().str("k"), Some("}"));
    }

    #[test]
    fn res_conditionals() {
        let text = "\"a\" \"1\" [$X360] \"a\" \"2\" [$WIN32] \"b\" [!$WIN32] { \"c\" \"d\" } \"e\" \"f\"";
        // `parse_res` drops the conditions but keeps every entry.
        let all = parse_res(text);
        assert_eq!(all.items().len(), 4);
        assert_eq!(all.str("a"), Some("1"));
        // `parse_pc` keeps only what holds on Windows.
        let pc = parse_pc(text);
        assert_eq!(pc.items().iter().filter(|(k, _)| k == "a").count(), 1);
        assert_eq!(pc.str("a"), Some("2"));
        assert!(pc.get("b").is_none());
        assert_eq!(pc.str("e"), Some("f"));
    }

    #[test]
    fn conditions_as_the_windows_pc_reads_them() {
        assert!(condition_holds("[$WIN32]"));
        assert!(condition_holds("[!$OSX]"));
        assert!(!condition_holds("[$X360]"));
        assert!(!condition_holds("[$POSIX]"));
        assert!(!condition_holds("[$LINUX]"));
        assert!(condition_holds("[$X360 || $WIN32]"));
        assert!(!condition_holds("[$WIN32 && $X360]"));
        assert!(condition_holds("[$WIN32 && !$X360]"));
        assert!(!condition_holds("[$OSX || $LINUX]"));
    }
}
