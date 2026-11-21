//! The words people type (docs/plans/active/hub.md, 5.1-5.3 and section
//! 11): invite codes (9 Crockford base32 characters, `XXX-XXX-XXX`, 45
//! bits), device link codes (6, `XXX-XXX`) and recovery keys (16 in four
//! groups, 80 bits), read back ignoring case, dashes and spaces, with `O`
//! read as `0` and `I`/`L` as `1`; stored only as the SHA-256 of their
//! normalized form. Usernames (3-20 of `[a-z0-9_]` after case folding, a
//! few reserved words refused) and display names (1-20 characters, no
//! control characters; the default is the username).
//!
//! A username a player **chooses** (sign-up, a rename;
//! docs/plans/active/onboarding.md) also starts with a letter
//! (`choose_username`, `UsernameRules` for the live checklist); the
//! looser `username` reads names an owner set. A taken one
//! answers with free suggestions (`suggestions`: the name with a digit,
//! with `_` and a short word, then cut and numbered), the same on the
//! hub and in the client.

use sha2::{Digest, Sha256};

/// Crockford's base32 alphabet (no I, L, O, U).
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// An invite code's characters.
pub const INVITE_LEN: usize = 9;
/// A link code's characters.
pub const LINK_LEN: usize = 6;
/// A recovery key's characters.
pub const RECOVERY_LEN: usize = 16;
/// Usernames' and display names' longest.
pub const NAME_MAX: usize = 20;
pub const USERNAME_MIN: usize = 3;

/// Words no username may be.
pub const RESERVED: &[&str] = &[
    "admin", "administrator", "owner", "root", "system", "chimera", "hub", "server", "guest", "moderator", "mod",
    "support", "staff", "null", "none", "anonymous", "everyone", "console", "host",
];

/// `len` random characters grouped by three (four for a recovery key).
pub fn generate(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    getrandom::fill(&mut bytes).expect("the OS's randomness");
    let chars: Vec<char> = bytes.iter().map(|b| ALPHABET[(b & 31) as usize] as char).collect();
    let group = if len == RECOVERY_LEN { 4 } else { 3 };
    chars.chunks(group).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("-")
}

/// `text` as its canonical characters: upper case, no dashes or spaces,
/// O as 0, I and L as 1; None if anything else is in it.
pub fn normalize(text: &str) -> Option<String> {
    let mut out = String::new();
    for c in text.chars() {
        let c = match c.to_ascii_uppercase() {
            '-' | ' ' => continue,
            'O' => '0',
            'I' | 'L' => '1',
            c if ALPHABET.contains(&(c as u8)) && c.is_ascii() => c,
            _ => return None,
        };
        out.push(c);
    }
    Some(out)
}

/// What a code is stored as: the SHA-256 (hex) of its normalized form.
pub fn hash(text: &str) -> Option<String> {
    let n = normalize(text)?;
    Some(Sha256::digest(n.as_bytes()).iter().map(|b| format!("{b:02x}")).collect())
}

/// `text` case-folded, if it is a valid username; else why not.
pub fn username(text: &str) -> Result<String, String> {
    let folded = text.trim().to_ascii_lowercase();
    if !(USERNAME_MIN..=NAME_MAX).contains(&folded.len()) {
        return Err(format!("a username is {USERNAME_MIN} to {NAME_MAX} characters"));
    }
    if !folded.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
        return Err("a username is letters, digits and `_` only".into());
    }
    if RESERVED.contains(&folded.as_str()) {
        return Err(format!("{folded:?} is reserved"));
    }
    Ok(folded)
}

/// Which of a chosen username's rules `text` meets (the checklist under
/// the field): its length, its characters, its first character and not
/// being reserved. Availability is the hub's to say.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UsernameRules {
    pub length: bool,
    pub characters: bool,
    pub starts_with_letter: bool,
    pub not_reserved: bool,
}

impl UsernameRules {
    pub fn of(text: &str) -> Self {
        let folded = text.trim().to_ascii_lowercase();
        Self {
            length: (USERNAME_MIN..=NAME_MAX).contains(&folded.len()),
            characters: folded.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
            starts_with_letter: folded.bytes().next().is_some_and(|b| b.is_ascii_lowercase()),
            not_reserved: !RESERVED.contains(&folded.as_str()),
        }
    }

    pub fn all(&self) -> bool {
        self.length && self.characters && self.starts_with_letter && self.not_reserved
    }
}

/// A username a player chooses: `username`'s rules and a letter first.
pub fn choose_username(text: &str) -> Result<String, String> {
    let folded = username(text)?;
    if !folded.starts_with(|c: char| c.is_ascii_lowercase()) {
        return Err("a username starts with a letter".into());
    }
    Ok(folded)
}

/// Words a suggestion may end with.
const SUFFIXES: &[&str] = &["gg", "fps", "one", "pro", "x"];

/// Up to `n` free usernames like `wanted` (`taken` says which are in
/// use): `wanted` cleaned into a chosen username's shape, then with a
/// digit (2-9), with `_` and a short word, then numbered 10-99; each cut
/// to fit 20 characters. Deterministic, so the hub and a test agree.
pub fn suggestions(wanted: &str, n: usize, taken: impl Fn(&str) -> bool) -> Vec<String> {
    let mut base: String = username_from(wanted.trim()).trim_matches('_').to_string();
    if !base.starts_with(|c: char| c.is_ascii_lowercase()) {
        base.insert(0, 'p');
    }
    let base = base.trim_end_matches('_').to_string();
    let fit = |stem: &str, tail: &str| -> String {
        let keep = NAME_MAX.saturating_sub(tail.len());
        let mut s: String = stem.chars().take(keep).collect();
        s.push_str(tail);
        s
    };
    let mut out: Vec<String> = Vec::new();
    let offer = |name: String, out: &mut Vec<String>| {
        if out.len() < n && choose_username(&name).is_ok() && !taken(&name) && !out.contains(&name) {
            out.push(name);
        }
    };
    if base.len() >= USERNAME_MIN {
        offer(base.clone(), &mut out);
    }
    for d in 2..=9 {
        offer(fit(&base, &d.to_string()), &mut out);
        if out.len() >= n.min(1) {
            break;
        }
    }
    for w in SUFFIXES {
        offer(fit(&base, &format!("_{w}")), &mut out);
        if out.len() >= n.min(2) {
            break;
        }
    }
    for d in 10..100 {
        offer(fit(&base, &format!("_{d}")), &mut out);
        if out.len() >= n {
            break;
        }
    }
    out
}

/// `text` trimmed, if it is a valid display name.
pub fn display_name(text: &str) -> Result<String, String> {
    let t = text.trim();
    let n = t.chars().count();
    if n == 0 || n > NAME_MAX {
        return Err(format!("a display name is 1 to {NAME_MAX} characters"));
    }
    if t.chars().any(char::is_control) {
        return Err("a display name has no control characters".into());
    }
    Ok(t.to_string())
}

/// A free-form name made into a username candidate:
/// folded, other characters as `_`, cut to `NAME_MAX`, padded to the
/// minimum.
pub fn username_from(text: &str) -> String {
    let mut out: String = text
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_lowercase() || c.is_ascii_digit() { c } else { '_' })
        .take(NAME_MAX)
        .collect();
    while out.len() < USERNAME_MIN {
        out.push('_');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_read_back_however_they_are_typed() {
        let code = generate(INVITE_LEN);
        assert_eq!((code.len(), code.matches('-').count()), (11, 2));
        assert_eq!(normalize(&code.to_lowercase().replace('-', " ")).unwrap(), code.replace('-', ""));
        assert_eq!(normalize("o1l-IOx"), Some("01110X".into()));
        assert_eq!(normalize("oil-ab"), Some("011AB".into()));
        assert_eq!(normalize("abu"), None, "U isn't in the alphabet");
        assert_eq!(hash("abc-def"), hash("ABCDEF"));
        let key = generate(RECOVERY_LEN);
        assert_eq!((key.len(), key.matches('-').count()), (19, 3));
        assert_ne!(generate(INVITE_LEN), generate(INVITE_LEN));
    }

    #[test]
    fn username_rules() {
        assert_eq!(username("Alice_01").unwrap(), "alice_01");
        assert_eq!(username(&"a".repeat(20)).unwrap().len(), 20);
        assert!(username(&"a".repeat(21)).is_err(), "at most 20");
        assert!(username("ab").is_err(), "at least 3");
        assert!(username("al ice").is_err() && username("alice!").is_err() && username("ålice").is_err());
        assert!(username("Admin").unwrap_err().contains("reserved"));
        assert_eq!(display_name("  Alice the Great  ").unwrap(), "Alice the Great");
        assert!(display_name(&"x".repeat(21)).is_err() && display_name("").is_err() && display_name("a\nb").is_err());
        assert_eq!(display_name(&"é".repeat(20)).unwrap().chars().count(), 20, "characters, not bytes");
        assert_eq!(username_from("Bob.Smith-PC-with-a-long-name"), "bob_smith_pc_with_a_");
        assert_eq!(username_from("x"), "x__");
    }

    #[test]
    fn chosen_names_and_suggestions() {
        assert!(choose_username("9lives").unwrap_err().contains("letter"));
        assert!(choose_username("_x_y").is_err());
        assert_eq!(choose_username("Rocket").unwrap(), "rocket");
        let r = UsernameRules::of("9 lives!");
        assert!(r.length && !r.characters && !r.starts_with_letter && r.not_reserved);
        assert!(UsernameRules::of("sam_rocket").all());
        assert!(!UsernameRules::of("host").not_reserved);
        let taken = |n: &str| ["rocket", "rocket2"].contains(&n);
        assert_eq!(suggestions("rocket", 3, taken), ["rocket3", "rocket_gg", "rocket_10"]);
        assert_eq!(suggestions("Sam", 1, |_| false), ["sam"]);
        let long = suggestions(&"a".repeat(20), 3, |n| n.len() == 20 && n.bytes().all(|b| b == b'a'));
        assert!(long.iter().all(|n| n.len() <= NAME_MAX && choose_username(n).is_ok()), "{long:?}");
        assert_eq!(suggestions("9 lives", 1, |_| false), ["p9_lives"]);
        assert!(suggestions("admin", 2, |_| false).iter().all(|n| n != "admin"));
    }
}
