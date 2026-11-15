//! The words people type (docs/plans/active/hub.md, 5.1-5.3 and section
//! 11): invite codes (9 Crockford base32 characters, `XXX-XXX-XXX`, 45
//! bits), device link codes (6, `XXX-XXX`) and recovery keys (16 in four
//! groups, 80 bits), read back ignoring case, dashes and spaces, with `O`
//! read as `0` and `I`/`L` as `1`; stored only as the SHA-256 of their
//! normalized form. Usernames (3-20 of `[a-z0-9_]` after case folding, a
//! few reserved words refused) and display names (1-20 characters, no
//! control characters; the default is the username).

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

/// A free-form name (a legacy account's) made into a username candidate:
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
}
