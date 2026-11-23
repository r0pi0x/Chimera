//! Account tickets (docs/plans/active/hub.md, 2.4): what the accounts
//! service (`chimera_services`) hands a signed-in device and every other
//! service checks with the accounts service's public key alone.
//!
//! - **The format**: `chimera2t.<payload>.<signature>`, both base64url
//!   without padding; the payload is `Claims` in postcard (version, key id,
//!   account and device ids, username, role, rights: `content` the asset
//!   service may serve, `games` it may play, `list`, `report`, `muted`;
//!   the audience, issued and expiry times, then the display name), the
//!   signature Ed25519 over the payload's bytes.
//! - **Keys** (`TicketKeys`): by key id (the first 16 hex digits of the
//!   public key's SHA-256), so a new key can be added and the old one kept
//!   until its tickets expire. A services box reads them from
//!   `<server store>/server/accounts/ticket.pub` (`<key id> <hex>` per
//!   line); the private key never leaves the accounts service.
//! - **Checking** (`TicketKeys::check`): the signature, the expiry (60 s
//!   of skew either way), the audience (`services`; later `join:<id>`).
//! - **Revocation** (`Revocations`): the accounts service's feed
//!   (`GET /internal/revoked?since=<seq>`, loopback, lines `<seq> account|device
//!   <id hex> <at>`): a ticket of a listed account or device issued at or
//!   before the entry's time is refused, so a revoked device stops at once,
//!   and a rights change or a ban that ends needs only a new ticket.
//! - **The device's request** (`request_message`): what a device signs to
//!   ask for a ticket (its id, the time, a random nonce), the same bytes on
//!   both sides.
//!
//! Tickets are bearer secrets for their 6 hours: never logged (`Debug`
//! of `Claims` names no secret; the ticket text is never printed).

use std::{
    collections::{BTreeMap, HashMap},
    fmt,
};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::store::{hex, sha256_hex, unhex};

/// Tickets start with this (`chimera1_` is an asset service's own token).
pub const PREFIX: &str = "chimera2t.";
/// The claims' format version.
pub const VERSION: u8 = 1;
/// A ticket's lifetime (decision 4): refreshed by the client at half-life.
pub const LIFETIME_SECS: u64 = 6 * 3600;
/// Clock skew allowed when checking expiry and issue time.
pub const SKEW_SECS: u64 = 60;
/// The audience of tickets for the services (assets, hub, feedback).
pub const SERVICES: &str = "services";
/// How far a device's request time may be from the service's.
pub const REQUEST_WINDOW_SECS: u64 = 300;

/// A random 16-byte id (accounts, devices).
pub type Id = [u8; 16];

/// A fresh random id.
pub fn new_id() -> Id {
    let mut id = [0u8; 16];
    getrandom::fill(&mut id).expect("the OS's randomness");
    id
}

/// An id as 32 hex digits.
pub fn id_hex(id: &Id) -> String {
    hex(id)
}

/// 32 hex digits as an id.
pub fn parse_id(text: &str) -> Option<Id> {
    unhex::<16>(text.trim())
}

/// What a ticket says.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    pub version: u8,
    pub key_id: String,
    pub account: Id,
    pub device: Id,
    /// The permanent handle (case-folded).
    pub username: String,
    /// `tester`, `host` or `owner`.
    pub role: String,
    /// What the asset service may serve it: `["*"]` all, else these games' content.
    pub content: Vec<String>,
    /// The hub's games it may play: `["*"]` all.
    pub games: Vec<String>,
    /// Servers it may list at once.
    pub list: u32,
    /// Whether its match results count.
    pub report: bool,
    /// Whether its feedback is dropped.
    pub muted: bool,
    pub audience: String,
    /// Unix seconds.
    pub issued: u64,
    pub expires: u64,
    /// What games show (`codes::display_name`), as when the ticket was
    /// issued. Last, so older readers ignore it (postcard leaves trailing
    /// bytes) and a ticket without it still reads (`decode`: the username).
    pub display_name: String,
}

/// `Claims` before `display_name` (tickets issued before 2026-11-23).
#[derive(Deserialize)]
struct ClaimsNoDisplay {
    version: u8,
    key_id: String,
    account: Id,
    device: Id,
    username: String,
    role: String,
    content: Vec<String>,
    games: Vec<String>,
    list: u32,
    report: bool,
    muted: bool,
    audience: String,
    issued: u64,
    expires: u64,
}

/// A payload's claims, with or without the display name.
fn decode(payload: &[u8]) -> Result<Claims, TicketError> {
    if let Ok(c) = postcard::from_bytes::<Claims>(payload) {
        return Ok(c);
    }
    let c: ClaimsNoDisplay = postcard::from_bytes(payload).map_err(|_| TicketError::Malformed)?;
    Ok(Claims {
        version: c.version,
        key_id: c.key_id,
        account: c.account,
        device: c.device,
        display_name: c.username.clone(),
        username: c.username,
        role: c.role,
        content: c.content,
        games: c.games,
        list: c.list,
        report: c.report,
        muted: c.muted,
        audience: c.audience,
        issued: c.issued,
        expires: c.expires,
    })
}

impl Claims {
    /// Whether the asset service may serve it `content` (a content game).
    pub fn may_fetch(&self, content: &str) -> bool {
        self.content.iter().any(|c| c == "*" || c == content)
    }
}

/// Why a ticket was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TicketError {
    Malformed,
    UnknownKey,
    BadSignature,
    Expired,
    NotYetValid,
    WrongAudience,
    Revoked,
}

impl fmt::Display for TicketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TicketError::Malformed => "not a ticket",
            TicketError::UnknownKey => "a ticket signed by a key this service doesn't know",
            TicketError::BadSignature => "a ticket with a bad signature",
            TicketError::Expired => "the ticket expired: ask the accounts service for a new one",
            TicketError::NotYetValid => "the ticket isn't valid yet (check this computer's clock)",
            TicketError::WrongAudience => "the ticket isn't for this service",
            TicketError::Revoked => "this device or account was revoked",
        })
    }
}

/// The key id of `public`.
pub fn key_id(public: &VerifyingKey) -> String {
    sha256_hex(public.as_bytes())[..16].to_string()
}

/// A ticket for `claims` signed by `key` (`claims.key_id` is set to its id).
pub fn issue(key: &SigningKey, claims: &Claims) -> String {
    let mut claims = claims.clone();
    claims.key_id = key_id(&key.verifying_key());
    let payload = postcard::to_allocvec(&claims).expect("claims serialize");
    let signature = key.sign(&payload);
    format!("{PREFIX}{}.{}", b64::encode(&payload), b64::encode(&signature.to_bytes()))
}

/// The claims of `ticket` without checking anything (a client reading its
/// own ticket's expiry).
pub fn peek(ticket: &str) -> Result<Claims, TicketError> {
    let (payload, _) = split(ticket)?;
    decode(&payload)
}

fn split(ticket: &str) -> Result<(Vec<u8>, Vec<u8>), TicketError> {
    let rest = ticket.trim().strip_prefix(PREFIX).ok_or(TicketError::Malformed)?;
    let (payload, signature) = rest.split_once('.').ok_or(TicketError::Malformed)?;
    Ok((
        b64::decode(payload).ok_or(TicketError::Malformed)?,
        b64::decode(signature).ok_or(TicketError::Malformed)?,
    ))
}

/// The ticket public keys a service trusts, by key id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TicketKeys {
    pub keys: BTreeMap<String, VerifyingKey>,
}

impl TicketKeys {
    pub fn single(public: VerifyingKey) -> Self {
        let mut keys = BTreeMap::new();
        keys.insert(key_id(&public), public);
        Self { keys }
    }

    /// `<key id> <hex>` per line (`#` comments); a line's id must be its
    /// key's.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut keys = BTreeMap::new();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let (id, public) = line.split_once(' ').ok_or_else(|| format!("{line:?}: `<key id> <hex>`"))?;
            let bytes = unhex::<32>(public.trim()).ok_or_else(|| format!("{id}: not a 32-byte hex key"))?;
            let key = VerifyingKey::from_bytes(&bytes).map_err(|e| format!("{id}: {e}"))?;
            if key_id(&key) != id {
                return Err(format!("{id}: the id isn't its key's"));
            }
            keys.insert(id.to_string(), key);
        }
        Ok(Self { keys })
    }

    pub fn to_text(&self) -> String {
        self.keys.iter().map(|(id, k)| format!("{id} {}\n", hex(k.as_bytes()))).collect()
    }

    /// `ticket`'s claims if it is valid at `now` for `audience` (revocation
    /// is the caller's: `Revocations::refuses`).
    pub fn check(&self, ticket: &str, audience: &str, now: u64) -> Result<Claims, TicketError> {
        let (payload, signature) = split(ticket)?;
        let claims = decode(&payload)?;
        if claims.version != VERSION {
            return Err(TicketError::Malformed);
        }
        let key = self.keys.get(&claims.key_id).ok_or(TicketError::UnknownKey)?;
        let signature: [u8; 64] = signature.try_into().map_err(|_| TicketError::Malformed)?;
        key.verify_strict(&payload, &Signature::from_bytes(&signature))
            .map_err(|_| TicketError::BadSignature)?;
        if now >= claims.expires + SKEW_SECS {
            return Err(TicketError::Expired);
        }
        if claims.issued > now + SKEW_SECS {
            return Err(TicketError::NotYetValid);
        }
        if claims.audience != audience {
            return Err(TicketError::WrongAudience);
        }
        Ok(claims)
    }
}

/// The revocation feed as a service last read it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Revocations {
    /// The last entry read (`?since=` asks for those after it).
    pub seq: u64,
    pub accounts: HashMap<Id, u64>,
    pub devices: HashMap<Id, u64>,
}

impl Revocations {
    /// Whether `claims` was issued at or before a revocation of its account
    /// or device.
    pub fn refuses(&self, claims: &Claims) -> bool {
        let at = |m: &HashMap<Id, u64>, id: &Id| m.get(id).is_some_and(|at| claims.issued <= *at);
        at(&self.accounts, &claims.account) || at(&self.devices, &claims.device)
    }

    /// Fold in the feed's answer (`<seq> account|device <id hex> <at>` per
    /// line); how many entries were new.
    pub fn apply(&mut self, feed: &str) -> Result<usize, String> {
        let mut n = 0;
        for line in feed.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let words: Vec<&str> = line.split(' ').collect();
            let [seq, kind, id, at] = words.as_slice() else {
                return Err(format!("revocation feed: {line:?}"));
            };
            let bad = || format!("revocation feed: {line:?}");
            let (seq, at) = (seq.parse::<u64>().map_err(|_| bad())?, at.parse::<u64>().map_err(|_| bad())?);
            let id = parse_id(id).ok_or_else(bad)?;
            if seq <= self.seq {
                continue;
            }
            let map = match *kind {
                "account" => &mut self.accounts,
                "device" => &mut self.devices,
                _ => return Err(bad()),
            };
            let e = map.entry(id).or_insert(0);
            *e = (*e).max(at);
            self.seq = seq;
            n += 1;
        }
        Ok(n)
    }
}

/// What a device signs to ask for a ticket.
pub fn request_message(device: &Id, time: u64, nonce: &[u8; 16]) -> Vec<u8> {
    format!("chimera-ticket-request/1\n{}\n{time}\n{}\n", hex(device), hex(nonce)).into_bytes()
}

/// Base64url without padding (RFC 4648, section 5).
pub mod b64 {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    pub fn encode(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
            for i in 0..=chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            }
        }
        out
    }

    pub fn decode(text: &str) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(text.len() * 3 / 4);
        let (mut acc, mut bits) = (0u32, 0u32);
        for c in text.bytes() {
            let v = ALPHABET.iter().position(|a| *a == c)? as u32;
            acc = acc << 6 | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
                acc &= (1 << bits) - 1;
            }
        }
        // Leftover bits must be zero padding (one encoding per byte string).
        (bits < 6 && acc == 0).then_some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(now: u64) -> Claims {
        Claims {
            version: VERSION,
            key_id: String::new(),
            account: [1; 16],
            device: [2; 16],
            username: "tester".into(),
            role: "tester".into(),
            content: vec!["cs_source".into()],
            games: vec!["*".into()],
            list: 1,
            report: false,
            muted: false,
            audience: SERVICES.into(),
            issued: now,
            expires: now + LIFETIME_SECS,
            display_name: "Tester".into(),
        }
    }

    #[test]
    fn a_ticket_without_a_display_name_still_reads_and_older_readers_skip_it() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let keys = TicketKeys::single(key.verifying_key());
        // A ticket as issued before the display name: its payload ends at `expires`.
        let mut c = claims(1000);
        c.key_id = key_id(&key.verifying_key());
        let full = postcard::to_allocvec(&c).unwrap();
        let old = &full[..full.len() - 1 - c.display_name.len()];
        let signed = |payload: &[u8]| format!("{PREFIX}{}.{}", b64::encode(payload), b64::encode(&key.sign(payload).to_bytes()));
        let read = keys.check(&signed(old), SERVICES, 1000).unwrap();
        assert_eq!((read.username.as_str(), read.display_name.as_str()), ("tester", "tester"));
        // A new ticket as an older reader sees it: the same claims, the name left over.
        let older: ClaimsNoDisplay = postcard::from_bytes(&full).unwrap();
        assert_eq!((older.username.as_str(), older.expires), ("tester", c.expires));
        assert_eq!(keys.check(&signed(&full), SERVICES, 1000).unwrap().display_name, "Tester");
    }

    #[test]
    fn base64url_round_trips() {
        for n in 0..70u8 {
            let bytes: Vec<u8> = (0..n).map(|i| i.wrapping_mul(37)).collect();
            let text = b64::encode(&bytes);
            assert!(!text.contains('=') && !text.contains('+') && !text.contains('/'));
            assert_eq!(b64::decode(&text).unwrap(), bytes);
        }
        assert_eq!(b64::encode(b"hello"), "aGVsbG8");
        assert!(b64::decode("a").is_none() && b64::decode("aGVsbG8=").is_none());
    }

    /// A ticket checks with the public key alone; a changed byte, another
    /// key, an expired one or another audience fails.
    #[test]
    fn a_ticket_checks_with_the_public_key_alone() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let keys = TicketKeys::single(key.verifying_key());
        let now = 1_800_000_000;
        let ticket = issue(&key, &claims(now));
        assert!(ticket.starts_with(PREFIX));
        let got = keys.check(&ticket, SERVICES, now + 10).unwrap();
        assert_eq!((got.username.as_str(), got.key_id.len()), ("tester", 16));
        assert!(got.may_fetch("cs_source") && !got.may_fetch("hl2dm"));
        assert_eq!(peek(&ticket).unwrap(), got);
        // A changed byte anywhere: refused.
        for i in [PREFIX.len() + 3, ticket.len() / 2, ticket.len() - 2] {
            let mut bytes = ticket.clone().into_bytes();
            bytes[i] = if bytes[i] == b'A' { b'B' } else { b'A' };
            let changed = String::from_utf8(bytes).unwrap();
            assert!(keys.check(&changed, SERVICES, now).is_err(), "byte {i}");
        }
        let other = SigningKey::from_bytes(&[8; 32]);
        assert_eq!(
            TicketKeys::single(other.verifying_key()).check(&ticket, SERVICES, now),
            Err(TicketError::UnknownKey)
        );
        assert_eq!(keys.check(&ticket, SERVICES, now + LIFETIME_SECS + SKEW_SECS), Err(TicketError::Expired));
        assert!(keys.check(&ticket, SERVICES, now + LIFETIME_SECS + SKEW_SECS - 1).is_ok(), "skew");
        assert_eq!(keys.check(&ticket, "join:abc", now), Err(TicketError::WrongAudience));
        assert_eq!(keys.check(&ticket, SERVICES, now - 3600), Err(TicketError::NotYetValid));
        assert_eq!(keys.check("chimera1_abc", SERVICES, now), Err(TicketError::Malformed));
        // Keys file round trip; a wrong id refused.
        let text = keys.to_text();
        assert_eq!(TicketKeys::parse(&text).unwrap(), keys);
        assert!(TicketKeys::parse(&text.replacen(&text[..2], "zz", 1)).is_err());
    }

    #[test]
    fn revocations_refuse_tickets_issued_before_them() {
        let mut r = Revocations::default();
        let c = claims(1000);
        assert!(!r.refuses(&c));
        let feed = format!("1 device {} 1000\n2 account {} 500\n", id_hex(&[2; 16]), id_hex(&[9; 16]));
        assert_eq!(r.apply(&feed).unwrap(), 2);
        assert_eq!(r.apply(&feed).unwrap(), 0, "seen entries skipped");
        assert_eq!(r.seq, 2);
        assert!(r.refuses(&c), "issued at the revocation's second");
        let later = Claims { issued: 1001, ..c.clone() };
        assert!(!r.refuses(&later), "a ticket issued after (a ban ended, rights changed) passes");
        assert!(r.apply("3 device zz 1").is_err());
    }
}
