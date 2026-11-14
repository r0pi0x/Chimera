//! Guns' full and short display names (docs/plans/active/gun-display-names.md).
//! The full name is the game's own display string (an app reads it from the
//! install); the short one serves the narrow places (callouts, the Tab
//! scoreboard's gun column, kill feeds, the match-end rows). A short name
//! comes from, in order: the game's override (`GunNameRules`, registered
//! from each game's data/display_names.toml with
//! `DisplayNamesAppExt::add_display_names`, as the draw exclusions are),
//! else the automatic split (`auto_short`) for a game whose file says
//! `split = true`, else the full name.
//!
//! The split (`split`, its word lists data/gun_names.toml, `NameWords`):
//! clean the name (`_` to spaces, full-width brackets, curly apostrophes,
//! a lone ` - `), move the variant tail (brackets, levels, ordinals, bare
//! numbers, `Mark II`, edition words) off the end, find the model (a word
//! with a letter and a digit, or a listed model, a `Dual`/`Twin` before it
//! kept), leave the skin words before it out, condense long phrases
//! (`Double Barrel` to `DB`). A name of 16 characters or fewer with no `_`
//! is its own short name. A result over 16 loses words from the end down to
//! the model (or, with no model, from the front) and stop words at its
//! edges; still over 16, the split gives none.
//!
//! The layout safety net (`fit`): where a name still doesn't fit its box,
//! condense, drop the variant tail, drop the skin, then cut at a word with
//! `…`; never wrap. The caller measures (`chimera_client`'s overlay with the
//! font, Combat Arms' kill feed by characters).
//!
//! Known shortcuts: the split's trimming to 16 (words off the end, or the
//! front without a model) is ours, beyond the plan's accept-or-override
//! rule: it keeps the override files short, and a trimmed name that reads
//! badly gets an override.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::Deserialize;

/// The split's word lists, as shipped.
pub const WORDS: &str = include_str!("../../data/gun_names.toml");

/// The longest a short name should be (characters).
pub const SHORT_MAX: usize = 16;

/// The split's word lists (data/gun_names.toml).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NameWords {
    pub variants: Vec<String>,
    pub levels: Vec<String>,
    pub marks: Vec<String>,
    pub models: Vec<String>,
    pub pairs: Vec<String>,
    pub stop: Vec<String>,
    pub condense: Vec<[String; 2]>,
}

impl NameWords {
    /// data/gun_names.toml, read (`GunNameRules` keeps a copy).
    pub fn shipped() -> NameWords {
        toml::from_str(WORDS).unwrap_or_else(|e| panic!("data/gun_names.toml: {e}"))
    }

    fn has(list: &[String], w: &str) -> bool {
        list.iter().any(|x| x.eq_ignore_ascii_case(w))
    }
}

/// `full` cleaned: `_` to spaces, full-width brackets to ASCII, curly
/// apostrophes straight, a lone ` - ` dropped, spaces collapsed.
pub fn clean(full: &str) -> String {
    let s: String = full
        .chars()
        .map(|c| match c {
            '_' => ' ',
            '（' => '(',
            '）' => ')',
            '\u{2018}' | '\u{2019}' => '\'',
            c => c,
        })
        .collect();
    s.split_whitespace()
        .map(|w| w.trim_end_matches(','))
        .filter(|w| !w.is_empty() && *w != "-")
        .collect::<Vec<_>>()
        .join(" ")
}

/// The words of a cleaned name, a bracketed group (`(limit)`, `(Red Dot)`)
/// one word.
fn words(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut open = false;
    for w in s.split_whitespace() {
        if open {
            let last = out.last_mut().expect("an open group");
            last.push(' ');
            last.push_str(w);
        } else {
            out.push(w.to_string());
        }
        let depth = w.matches('(').count() as i32 - w.matches(')').count() as i32;
        if depth > 0 {
            open = true;
        } else if depth < 0 || (open && w.contains(')')) {
            open = false;
        }
    }
    out
}

fn is_ordinal(w: &str) -> bool {
    let digits = w.chars().take_while(|c| c.is_ascii_digit()).count();
    digits > 0 && ["st", "nd", "rd", "th"].iter().any(|s| w[digits..].eq_ignore_ascii_case(s))
}

fn is_number(w: &str) -> bool {
    (1..=4).contains(&w.len()) && w.chars().all(|c| c.is_ascii_digit())
}

fn is_level(w: &str, words: &NameWords) -> bool {
    let lower = w.to_ascii_lowercase();
    words.levels.iter().any(|l| {
        let Some(rest) = lower.strip_prefix(&l.to_ascii_lowercase()) else { return false };
        let rest = rest.strip_prefix('.').unwrap_or(rest);
        let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
        digits > 0 && (digits == rest.len() || rest[digits..].starts_with('('))
    })
}

fn is_roman(w: &str) -> bool {
    ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"].iter().any(|r| w.eq_ignore_ascii_case(r))
}

/// How many words at the end of `ws` are one variant piece (0: none).
fn variant_at_end(ws: &[String], words: &NameWords) -> usize {
    let Some(w) = ws.last() else { return 0 };
    if is_roman(w) && ws.len() >= 2 && NameWords::has(&words.marks, &ws[ws.len() - 2]) {
        return 2;
    }
    let bracket = w.starts_with('(') && w.ends_with(')');
    // An edition word with its bracket (`SE(Common)`).
    let edition = w.ends_with(')') && w.split_once('(').is_some_and(|(e, _)| NameWords::has(&words.variants, e));
    if bracket || edition || is_level(w, words) || is_ordinal(w) || is_number(w) || NameWords::has(&words.variants, w) {
        1
    } else {
        0
    }
}

/// Whether `ws` ends with a listed model of several words (`Tango 51`).
fn model_at_end(ws: &[String], words: &NameWords) -> bool {
    words.models.iter().any(|m| {
        let parts: Vec<&str> = m.split_whitespace().collect();
        parts.len() > 1
            && parts.len() <= ws.len()
            && parts.iter().zip(&ws[ws.len() - parts.len()..]).all(|(p, w)| p.eq_ignore_ascii_case(w))
    })
}

/// Whether `w` is a model by its shape: a letter and a digit, not a level
/// or an ordinal.
fn model_shaped(w: &str, words: &NameWords) -> bool {
    w.chars().any(|c| c.is_ascii_digit())
        && w.chars().any(|c| c.is_alphabetic())
        && !is_level(w, words)
        && !is_ordinal(w)
        && !w.starts_with('(')
}

/// Where the model starts in `ws` and how many words it takes.
fn find_model(ws: &[String], words: &NameWords) -> Option<(usize, usize)> {
    for i in 0..ws.len() {
        for m in &words.models {
            let parts: Vec<&str> = m.split_whitespace().collect();
            if i + parts.len() <= ws.len() && parts.iter().zip(&ws[i..]).all(|(p, w)| p.eq_ignore_ascii_case(w)) {
                return Some((i, parts.len()));
            }
        }
        if model_shaped(&ws[i], words) {
            return Some((i, 1));
        }
    }
    None
}

/// A name split: the skin (words before the model), the base (the model
/// and what follows it; empty without a model) and the variant tail.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Split {
    pub skin: Vec<String>,
    pub base: Vec<String>,
    pub variant: Vec<String>,
    /// How many words of `base` are the model (with a `Dual` before it).
    pub model_words: usize,
}

/// `full` split (`Split`; see the module doc).
pub fn split(full: &str, words: &NameWords) -> Split {
    let mut ws = self::words(&clean(full));
    let mut variant = Vec::new();
    while ws.len() > 1 {
        if model_at_end(&ws, words) {
            break;
        }
        let n = variant_at_end(&ws, words);
        if n == 0 || n >= ws.len() {
            break;
        }
        let tail = ws.split_off(ws.len() - n);
        variant.splice(0..0, tail);
    }
    match find_model(&ws, words) {
        Some((mut at, mut n)) => {
            if at > 0 && NameWords::has(&words.pairs, &ws[at - 1]) {
                at -= 1;
                n += 1;
            }
            let base = ws.split_off(at);
            Split { skin: ws, base, variant, model_words: n }
        }
        None => Split { skin: ws, base: Vec::new(), variant, model_words: 0 },
    }
}

/// `ws` with the condensed phrases (`Double Barrel` to `DB`).
pub fn condense(ws: &[String], words: &NameWords) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    'next: while i < ws.len() {
        for [from, to] in &words.condense {
            let parts: Vec<&str> = from.split_whitespace().collect();
            if i + parts.len() <= ws.len() && parts.iter().zip(&ws[i..]).all(|(p, w)| p.eq_ignore_ascii_case(w)) {
                out.push(to.clone());
                i += parts.len();
                continue 'next;
            }
        }
        out.push(ws[i].clone());
        i += 1;
    }
    out
}

fn chars(ws: &[String]) -> usize {
    ws.iter().map(|w| w.chars().count()).sum::<usize>() + ws.len().saturating_sub(1)
}

/// The automatic short name of `full` (`split`, condensed, trimmed to
/// `SHORT_MAX`); None when nothing of it fits.
pub fn auto_short(full: &str, words: &NameWords) -> Option<String> {
    if !full.contains('_') && full.chars().count() <= SHORT_MAX {
        return Some(full.to_string());
    }
    let s = split(full, words);
    let stop = |w: &String| NameWords::has(&words.stop, w);
    let mut ws;
    if s.base.is_empty() {
        ws = condense(&s.skin, words);
        while chars(&ws) > SHORT_MAX && ws.len() > 1 {
            ws.remove(0);
            while ws.len() > 1 && stop(&ws[0]) {
                ws.remove(0);
            }
        }
    } else {
        let keep = s.model_words;
        ws = condense(&s.base, words);
        let keep = keep.min(ws.len());
        while chars(&ws) > SHORT_MAX && ws.len() > keep {
            ws.pop();
            while ws.len() > keep && ws.last().is_some_and(stop) {
                ws.pop();
            }
        }
    }
    while ws.len() > 1 && ws.last().is_some_and(stop) {
        ws.pop();
    }
    let out = ws.join(" ");
    (!out.is_empty() && out.chars().count() <= SHORT_MAX).then_some(out)
}

/// `text` made to fit (`fits`): itself; condensed; without its variant
/// tail; without its skin; then cut at a word with `…` (a first word too
/// long cut inside it). Never wraps.
pub fn fit(text: &str, fits: impl Fn(&str) -> bool, words: &NameWords) -> String {
    if fits(text) {
        return text.to_string();
    }
    let s = split(text, words);
    let all: Vec<String> = s.skin.iter().chain(&s.base).chain(&s.variant).cloned().collect();
    let mut last = condense(&all, words);
    let without_tail: Vec<String> = s.skin.iter().chain(&s.base).cloned().collect();
    let without_skin = if s.base.is_empty() { without_tail.clone() } else { s.base.clone() };
    for c in [condense(&all, words), condense(&without_tail, words), condense(&without_skin, words)] {
        if c.is_empty() {
            continue;
        }
        if fits(&c.join(" ")) {
            return c.join(" ");
        }
        last = c;
    }
    for n in (1..last.len()).rev() {
        let t = format!("{}…", last[..n].join(" "));
        if fits(&t) {
            return t;
        }
    }
    let first: Vec<char> = last.first().map_or(Vec::new(), |w| w.chars().collect());
    for n in (1..first.len()).rev() {
        let t = format!("{}…", first[..n].iter().collect::<String>());
        if fits(&t) {
            return t;
        }
    }
    "…".to_string()
}

/// Where a short name came from (`GunNameRules::short`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameSource {
    /// The game's display_names.toml.
    Override,
    /// The automatic split.
    Split,
    /// The full name as it is.
    Full,
}

impl NameSource {
    pub fn name(self) -> &'static str {
        match self {
            NameSource::Override => "override",
            NameSource::Split => "split",
            NameSource::Full => "full",
        }
    }
}

/// One override: an optional short and full name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NameOverride {
    pub short: Option<String>,
    pub full: Option<String>,
}

/// Every game's display-name rules (`DisplayNamesAppExt`): the split's
/// words (data/gun_names.toml), which games' names it shortens, and the
/// overrides by weapon id (any case; an id with `*` matches any run of
/// characters there).
#[derive(Resource, Clone, Debug)]
pub struct GunNameRules {
    words: NameWords,
    split_games: Vec<String>,
    exact: BTreeMap<String, NameOverride>,
    globs: Vec<(String, NameOverride)>,
}

fn glob(pattern: &str, s: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !s.starts_with(first) || s.len() < first.len() + last.len() || !s.ends_with(last) {
        return false;
    }
    let mut at = first.len();
    let end = s.len() - last.len();
    for p in &parts[1..parts.len() - 1] {
        match s[at..end].find(p) {
            Some(i) => at += i + p.len(),
            None => return false,
        }
    }
    true
}

impl Default for GunNameRules {
    fn default() -> Self {
        GunNameRules { words: NameWords::shipped(), split_games: Vec::new(), exact: BTreeMap::new(), globs: Vec::new() }
    }
}

impl GunNameRules {
    /// The split's (and the layout net's) words.
    pub fn words(&self) -> &NameWords {
        &self.words
    }

    /// The override for `id`, if any (an exact id before a pattern).
    pub fn override_for(&self, id: &str) -> Option<&NameOverride> {
        let id = id.to_ascii_lowercase();
        self.exact.get(&id).or_else(|| self.globs.iter().find(|(p, _)| glob(p, &id)).map(|(_, o)| o))
    }

    /// Whether `game`'s names get the automatic split.
    pub fn splits(&self, game: &str) -> bool {
        self.split_games.iter().any(|g| g == game)
    }

    /// Gun `id`'s full name: the override's, else `game_full` (the game's).
    pub fn full(&self, id: &str, game_full: &str) -> String {
        self.override_for(id).and_then(|o| o.full.clone()).unwrap_or_else(|| game_full.to_string())
    }

    /// Gun `id`'s short name for its full name `full`, and where it came
    /// from: the override, else the split (its game's file says `split`),
    /// else `full`.
    pub fn short(&self, id: &str, full: &str) -> (String, NameSource) {
        if let Some(s) = self.override_for(id).and_then(|o| o.short.clone()) {
            return (s, NameSource::Override);
        }
        let game = id.split_once(':').map_or("", |(g, _)| g);
        if self.splits(game)
            && let Some(s) = auto_short(full, &self.words)
        {
            let from = if s == full { NameSource::Full } else { NameSource::Split };
            return (s, from);
        }
        (full.to_string(), NameSource::Full)
    }

    /// `doc`'s rules added (`parse_display_names`).
    pub fn add(&mut self, doc: DisplayNames) {
        if doc.split && !self.splits(&doc.game) {
            self.split_games.push(doc.game.clone());
        }
        for n in doc.name {
            let o = NameOverride { short: n.short, full: n.full };
            for w in n.weapons {
                let w = w.to_ascii_lowercase();
                if w.contains('*') {
                    self.globs.push((w, o.clone()));
                } else {
                    self.exact.insert(w, o.clone());
                }
            }
        }
    }
}

/// A game's data/display_names.toml: its `game`, whether the split
/// shortens its names, and `[[name]]` overrides (`weapons`, `short`,
/// `full`).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DisplayNames {
    pub game: String,
    #[serde(default)]
    pub split: bool,
    #[serde(default)]
    pub name: Vec<NameDoc>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NameDoc {
    pub weapons: Vec<String>,
    #[serde(default)]
    pub short: Option<String>,
    #[serde(default)]
    pub full: Option<String>,
}

/// `text` (a game's display_names.toml) read and checked: every id is the
/// game's, every entry names a short or a full name, and a short one is
/// 1 to `SHORT_MAX` characters.
pub fn parse_display_names(text: &str) -> Result<DisplayNames, String> {
    let doc: DisplayNames = toml::from_str(text).map_err(|e| e.to_string())?;
    let prefix = format!("{}:", doc.game);
    for n in &doc.name {
        if n.short.is_none() && n.full.is_none() {
            return Err(format!("{:?}: neither short nor full", n.weapons));
        }
        if let Some(s) = &n.short
            && !(1..=SHORT_MAX).contains(&s.chars().count())
        {
            return Err(format!("{:?}: short name {s:?} isn't 1 to {SHORT_MAX} characters", n.weapons));
        }
        if let Some(w) = n.weapons.iter().find(|w| !w.starts_with(&prefix)) {
            return Err(format!("{w}: not one of {}'s weapons", doc.game));
        }
        if n.weapons.is_empty() {
            return Err("an entry with no weapons".into());
        }
    }
    Ok(doc)
}

pub trait DisplayNamesAppExt {
    /// A game's display names (`parse_display_names`; `name` for the
    /// error, which panics: shipped data) added to `GunNameRules`.
    fn add_display_names(&mut self, name: &str, text: &str) -> &mut Self;
}

impl DisplayNamesAppExt for App {
    fn add_display_names(&mut self, name: &str, text: &str) -> &mut Self {
        let doc = parse_display_names(text).unwrap_or_else(|e| panic!("{name}: {e}"));
        self.world_mut().get_resource_or_init::<GunNameRules>().add(doc);
        self
    }
}

/// What a gun with no display string is called: its id's last part, past
/// `weapon_`, `_` and `-` as spaces, in capitals.
pub fn id_name(id: &str) -> String {
    let last = id.rsplit(':').next().unwrap_or(id);
    let last = last.strip_prefix("weapon_").unwrap_or(last);
    last.chars().map(|c| if c == '_' || c == '-' { ' ' } else { c.to_ascii_uppercase() }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn short(full: &str) -> Option<String> {
        auto_short(full, &NameWords::shipped())
    }

    #[test]
    fn the_plans_samples_split() {
        assert_eq!(short("Kagutsuchi's Bloom SCAR-L lv.1(limit)").as_deref(), Some("SCAR-L"));
        assert_eq!(short("Field Camo USP Tactical SE CORE").as_deref(), Some("USP Tac."));
        assert_eq!(short("Glock23_Champion").as_deref(), Some("Glock23"));
        assert_eq!(short("M4A1 Frostbite lv.2(limit)").as_deref(), Some("M4A1 Frostbite"));
        assert_eq!(short("Sun God Ra AR-15 Double Barrel (Mythic)").as_deref(), Some("AR-15 DB"));
        assert_eq!(short("Poseidon's Wrath SG50 LV.1 (Limit)").as_deref(), Some("SG50"));
        assert_eq!(short("Stadium Legends AK-47 lv.1(limit)").as_deref(), Some("AK-47"));
        assert_eq!(short("Obsidian Gild AN-94 Lv.1 (limit)").as_deref(), Some("AN-94"));
        assert_eq!(short("Roaring White Dragon AR-15 Double Shot").as_deref(), Some("AR-15 DS"));
        assert_eq!(short("Visionary AK-47 Gold-Plated (Event)").as_deref(), Some("AK-47"));
        assert_eq!(short("limit Imperial Spirit M4A1 lv1").as_deref(), Some("M4A1"));
        assert_eq!(short("Bren Light Machine Gun").as_deref(), Some("Bren Light MG"));
        assert_eq!(short("M419 Combat CQB SE CAMO Black MOD Mark II").as_deref(), Some("M419 Combat CQB"));
        // Short enough already, no `_`: as it is.
        assert_eq!(short("AK-47 (Event)").as_deref(), Some("AK-47 (Event)"));
        // A model ending in a number keeps it; an edition with its bracket goes.
        assert_eq!(short("Bloody Hunter Tango 51").as_deref(), Some("Tango 51"));
        assert_eq!(short("Agent 13 P220 SE(Common)").as_deref(), Some("P220"));
        assert_eq!(short("The Egyptian God Seth M21E,").as_deref(), Some("M21E"));
        assert_eq!(short("9X19MM SIDEARM").as_deref(), Some("9X19MM SIDEARM"));
    }

    #[test]
    fn the_parts_of_a_split() {
        let w = &NameWords::shipped();
        let s = split("Sun God Ra AR-15 Double Barrel (Mythic)", w);
        assert_eq!(s.skin, ["Sun", "God", "Ra"]);
        assert_eq!(s.base, ["AR-15", "Double", "Barrel"]);
        assert_eq!(s.variant, ["(Mythic)"]);
        let s = split("Dual Kriss Vector Gold", w);
        assert_eq!((s.base[0].as_str(), s.model_words), ("Dual", 2));
        assert_eq!(s.variant, ["Gold"]);
        assert_eq!(clean("P220 (SE) - Majestic\u{2019}s  Dragon（Loong）"), "P220 (SE) Majestic's Dragon(Loong)");
        // A one-word name keeps its word.
        assert_eq!(split("Gold", w).skin, ["Gold"]);
    }

    #[test]
    fn fit_condenses_then_cuts_at_a_word() {
        let w = &NameWords::shipped();
        let under = |n: usize| move |s: &str| s.chars().count() <= n;
        assert_eq!(fit("M4A1", under(10), w), "M4A1");
        assert_eq!(fit("AR-15 Double Barrel", under(10), w), "AR-15 DB");
        assert_eq!(fit("Sun God Ra AR-15 Double Barrel (Mythic)", under(10), w), "AR-15 DB");
        assert_eq!(fit("LEONE YG1265 AUTO SHOTGUN", under(16), w), "YG1265 AUTO…");
        assert_eq!(fit("Jackhammerish", under(6), w), "Jackh…");
        assert_eq!(fit("anything", under(0), w), "…");
    }

    #[test]
    fn overrides_win_over_the_split() {
        let mut r = GunNameRules::default();
        r.add(
            parse_display_names(
                "game = \"kit\"\nsplit = true\n[[name]]\nweapons = [\"kit:ra_*\"]\nshort = \"Ra AR-15 DB\"\n\
                 [[name]]\nweapons = [\"kit:p220\"]\nfull = \"P220 Dragon\"\n",
            )
            .unwrap(),
        );
        assert_eq!(r.short("kit:ra_mythic", "Sun God Ra AR-15 Double Barrel (Mythic)"), ("Ra AR-15 DB".into(), NameSource::Override));
        assert_eq!(r.short("kit:other", "Glock23_Champion"), ("Glock23".into(), NameSource::Split));
        assert_eq!(r.short("kit:other", "Glock"), ("Glock".into(), NameSource::Full));
        assert_eq!(r.short("other:x", "Glock23_Champion"), ("Glock23_Champion".into(), NameSource::Full), "not a split game");
        assert_eq!(r.full("kit:P220", "P220 (SE) - Dragon"), "P220 Dragon");
        assert!(parse_display_names("game = \"kit\"\n[[name]]\nweapons = [\"other:x\"]\nshort = \"X\"").is_err());
        assert!(parse_display_names("game = \"kit\"\n[[name]]\nweapons = [\"kit:x\"]\nshort = \"seventeen chars xx\"").is_err());
        assert!(glob("a*b*c", "axxbyyc") && !glob("a*b", "ab_c") && glob("a*", "a"));
        assert_eq!(id_name("combat_arms:hqraid_boss-minigun"), "HQRAID BOSS MINIGUN");
    }
}
