//! The secret filter of feedback bundles (docs/plans/active/feedback-reports.md,
//! 2.5). Every line of text that enters a bundle passes `line`; cvars pass
//! `secret_name` first.
//!
//! - A cvar is dropped when its name contains `password`, `token`, `key`,
//!   `secret`, `rcon`, `auth`, `fingerprint` or `asset_url`
//!   (`secret_name`).
//! - In a line, a word naming such a thing (`rcon_password`,
//!   `+chimera:asset_token`, `+chimera:hub` and a hub link (its invite
//!   code), a recovery key, `"sv_password"`, a bare `password`) keeps the
//!   word and hides everything after it (`(hidden)`, rcon's rule).
//!   Here a word's parts (split at `_ : . -`) are matched, so `keyboard`
//!   is no secret but `key`, `api_key` and `KEYS` are.
//! - `connect <address>` keeps the command and drops the address.
//! - Runs that look like tokens or keys (`chimera1_...`, long hex, long
//!   mixed runs of letters and digits) become `(hidden)`.
//! - Home folders (`/home/<name>`, `/Users/<name>`, `C:\Users\<name>`,
//!   and this machine's home) become `~`.
//! - IPv4 addresses (with a port) and IPv6 addresses become `(address)`.
//!
//! Known shortcuts: it errs on the side of hiding: a log line saying
//! "token refused" loses what follows, a version `1.2.3.4` reads as an
//! address, a long hash as a token.

/// Name parts that make a cvar or a command secret.
const SECRET_PARTS: &[&str] = &["password", "token", "key", "secret", "rcon", "auth", "fingerprint"];

/// Whether a cvar named `name` is kept out of a bundle.
pub fn secret_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("asset_url") || SECRET_PARTS.iter().any(|p| n.contains(p))
}

/// Whether a word in a line names a secret (by its parts).
fn secret_word(word: &str) -> bool {
    let w = word.to_ascii_lowercase();
    if w.contains("asset_url")
        || w.contains("password")
        || w.contains("secret")
        || w.contains("passwd")
        || w.contains("chimera:hub")
        || w.contains("chimera-hub:")
        || w.contains("recovery")
    {
        return true;
    }
    w.split(['_', ':', '.', '-']).any(|part| {
        let part = part.strip_suffix('s').unwrap_or(part);
        matches!(part, "token" | "key" | "rcon" | "auth" | "fingerprint" | "apikey" | "authorization" | "bearer")
    })
}

/// One line with secrets, home folders and addresses taken out.
pub fn line(text: &str) -> String {
    let text = text.replace(['\r', '\0'], "");
    let text = hide_after_secret(&text);
    let text = hide_connect(&text);
    let text = homes(&text);
    let text = addresses(&text);
    tokens(&text)
}

/// Every line of `text`.
pub fn text(text: &str) -> String {
    text.lines().map(line).collect::<Vec<_>>().join("\n") + if text.ends_with('\n') { "\n" } else { "" }
}

/// Everything after a word that names a secret becomes `(hidden)`.
fn hide_after_secret(text: &str) -> String {
    let mut start = None;
    for (i, c) in text.char_indices().chain(std::iter::once((text.len(), ' '))) {
        let inside = i < text.len() && (c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '.' | '-'));
        match (inside, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                if secret_word(&text[s..i]) {
                    // Keep a closing quote right after the name.
                    let end = if text[i..].starts_with('"') { i + 1 } else { i };
                    let rest = text[end..].trim();
                    return if rest.is_empty() {
                        text.to_string()
                    } else {
                        format!("{} (hidden)", &text[..end])
                    };
                }
                start = None;
            }
            _ => {}
        }
    }
    text.to_string()
}

/// `connect <address>`: the address goes.
fn hide_connect(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let Some(at) = lower.find("connect ") else { return text.to_string() };
    if at > 0 && lower[..at].chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
        return text.to_string();
    }
    let after = at + "connect ".len();
    let rest = &text[after..];
    let arg_len = rest.trim_start().find(|c: char| c.is_whitespace() || c == ';' || c == '"').unwrap_or(rest.trim_start().len());
    if arg_len == 0 {
        return text.to_string();
    }
    let lead = rest.len() - rest.trim_start().len();
    format!("{}(address){}", &text[..after + lead], &text[after + lead + arg_len..])
}

/// Home folders become `~`.
fn homes(text: &str) -> String {
    let mut out = text.to_string();
    if let Some(home) = dirs::home_dir().map(|h| h.to_string_lossy().into_owned()).filter(|h| h.len() > 3) {
        out = out.replace(&home, "~");
        out = out.replace(&home.replace('\\', "/"), "~");
    }
    // `/home/<name>`, `/Users/<name>`, `<drive>:\Users\<name>` (either slash).
    let mut result = String::with_capacity(out.len());
    let bytes = out.as_bytes();
    let mut i = 0;
    while i < out.len() {
        let rest = &out[i..];
        let lower = rest.get(..10).unwrap_or(rest).to_ascii_lowercase();
        let prefix = if lower.starts_with("/home/") {
            Some(6)
        } else if lower.starts_with("/users/") {
            Some(7)
        } else if bytes[i].is_ascii_alphabetic()
            && (lower[1..].starts_with(":\\users\\") || lower[1..].starts_with(":/users/"))
            && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
        {
            Some(9)
        } else {
            None
        };
        if let Some(p) = prefix {
            let name_len = rest[p..].find(['/', '\\', ' ', '"', '\'', ')', ',', ';']).unwrap_or(rest.len() - p);
            if name_len > 0 {
                result.push('~');
                i += p + name_len;
                continue;
            }
        }
        let c = rest.chars().next().expect("not at the end");
        result.push(c);
        i += c.len_utf8();
    }
    result
}

/// IPv4 (with an optional port) and IPv6 addresses become `(address)`.
fn addresses(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let b = text.as_bytes();
    let mut i = 0;
    while i < text.len() {
        let boundary = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'.' || b[i - 1] == b':');
        if boundary && let Some(len) = ipv4_at(&text[i..]).or_else(|| ipv6_at(&text[i..])) {
            out.push_str("(address)");
            i += len;
            continue;
        }
        let c = text[i..].chars().next().expect("not at the end");
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// The length of an IPv4 address (and `:port`) at the start of `s`.
fn ipv4_at(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 0;
    for part in 0..4 {
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() && i - start < 3 {
            i += 1;
        }
        if i == start || s[start..i].parse::<u16>().ok()? > 255 {
            return None;
        }
        if part < 3 {
            if b.get(i) != Some(&b'.') {
                return None;
            }
            i += 1;
        }
    }
    if b.get(i).is_some_and(|c| c.is_ascii_digit() || *c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)) {
        return None;
    }
    if b.get(i) == Some(&b':') && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    Some(i)
}

/// The length of an IPv6 address at the start of `s` (hex groups and
/// colons, with `::` or seven colons, ending at a word's edge).
fn ipv6_at(s: &str) -> Option<usize> {
    let len = s.find(|c: char| !(c.is_ascii_hexdigit() || c == ':' || c == '.')).unwrap_or(s.len());
    let mut cand = &s[..len];
    // A sentence's colon or dot after it isn't part of it.
    while cand.ends_with('.') || (cand.ends_with(':') && !cand.ends_with("::")) {
        cand = &cand[..cand.len() - 1];
    }
    if s[len..].chars().next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let colons = cand.matches(':').count();
    let ok = cand.chars().any(|c| c.is_ascii_hexdigit())
        && (cand.contains("::") && colons >= 2 || colons == 7)
        && cand.split(':').all(|g| g.len() <= 4 || g.contains('.'));
    ok.then_some(cand.len())
}

/// Token-like runs become `(hidden)`.
fn tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if looks_like_token(run) {
            out.push_str("(hidden)");
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+' | '/' | '=') {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    out
}

fn looks_like_token(run: &str) -> bool {
    if run.to_ascii_lowercase().contains("chimera1_") {
        return true;
    }
    // Paths are many short parts: judge each part between slashes.
    run.split('/').any(|part| {
        let digits = part.bytes().filter(u8::is_ascii_digit).count();
        let letters = part.bytes().filter(u8::is_ascii_alphabetic).count();
        let hex = part.bytes().all(|c| c.is_ascii_hexdigit());
        (hex && part.len() >= 16 && digits >= 3 && letters >= 1)
            || (part.len() >= 20 && digits >= 4 && letters >= 4)
            || (part.len() >= 40 && digits >= 1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_addresses_homes_and_tokens_go() {
        assert!(secret_name("rcon_password") && secret_name("sv_password") && secret_name("chimera:asset_token"));
        assert!(secret_name("chimera:asset_url") && secret_name("chimera:feedback_fingerprint"));
        assert!(!secret_name("fps_max") && !secret_name("sensitivity"));
        assert_eq!(line("] rcon_password hunter2"), "] rcon_password (hidden)");
        assert_eq!(line("+chimera:asset_token abc"), "+chimera:asset_token (hidden)");
        assert_eq!(line("+chimera:hub chimera-hub://h:1/ab?code=ABC-DEF-GHJ bob"), "+chimera:hub (hidden)");
        assert_eq!(line("your recovery key ABCD-EFGH-JKMN-PQRS"), "your recovery (hidden)");
        assert!(!line(&format!("ticket chimera2t.{}.{}", "Ab1".repeat(30), "Xy9".repeat(28))).contains("Ab1Ab1"));
        assert_eq!(line("\"sv_password\" = \"x\" ( def. \"\" )"), "\"sv_password\" (hidden)");
        assert_eq!(line("password"), "password");
        assert_eq!(line("keyboard layout us"), "keyboard layout us");
        assert_eq!(line("connect myhost.example:27015"), "connect (address)");
        assert_eq!(line("joined 10.1.2.3:27015 ok"), "joined (address) ok");
        assert_eq!(line("at [2001:db8::7]:27015"), "at [(address)]:27015");
        assert_eq!(line("v ::1 x"), "v (address) x");
        assert_eq!(line("bevy::prelude::Foo at 12:34:56"), "bevy::prelude::Foo at 12:34:56");
        assert_eq!(line(concat!("C:\\", "Users", "\\Alice\\AppData\\x")), "~\\AppData\\x");
        assert_eq!(line(concat!("/", "home", "/bob/.cache/chimera")), "~/.cache/chimera");
        assert_eq!(line("got chimera1_0123abcd"), "got (hidden)");
        assert_eq!(line(&format!("id {}", "ab12".repeat(16))), "id (hidden)");
        assert_eq!(
            line("materials/models/weapons/v_models/rif_ak47/ak47_ref.vtf"),
            "materials/models/weapons/v_models/rif_ak47/ak47_ref.vtf"
        );
        assert_eq!(line("de_dust2 rung 3 of 20"), "de_dust2 rung 3 of 20");
    }
}
