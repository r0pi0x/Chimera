// Adapted from ICE (public domain, see THIRD_PARTY.md): ice.c
// Matthew Kwan's reference implementation of the ICE cipher, written July
// 1996 and placed in the public domain ("The ICE algorithm is public
// domain ... There are no patents or copyrights, so its use is
// unrestricted"), ported to Rust.

//! ICE, the 64-bit block cipher TF2's encrypted script files (`.ctx`) are
//! written with: a port of the designer's public-domain reference
//! implementation (THIRD_PARTY.md, "ICE"; approved 2026-11-15), which
//! replaced our earlier prose-only version (that one, built from
//! specs/tf2/ice.md, never reproduced the published test vectors).
//! `Ice` is a key's schedule at a level (Thin-ICE, level 0: 8 rounds;
//! ICE, level 1: 16; ICE-n: 16·n, nested) with `encrypt_block` /
//! `decrypt_block`; `encrypt_ctx` / `decrypt_ctx` are the files' framing
//! (plain ECB, a trailing partial block left in the clear; ice.md, "TF2's
//! encrypted script files"). The S-boxes (with the P-box folded in) are
//! built at compile time instead of on first use, as read-only statics.
//!
//! `CtxCipher` is the level `.ctx` files are decrypted at (`DEFAULT`,
//! the one the game's files use); `CHIMERA_TF2_ICE_LEVEL=thin|ice`
//! overrides it at run time (`CtxCipher::configured`), and `probe_ctx`
//! (`dump tf2 --ice-probe`) says which levels decrypt a real file with
//! the user's key.

/// Modulo values for the S-boxes, by S-box and row.
const SMOD: [[u32; 4]; 4] = [[333, 313, 505, 369], [379, 375, 319, 391], [361, 445, 451, 397], [397, 425, 395, 505]];

/// XOR values for the S-boxes, by S-box and row.
const SXOR: [[u32; 4]; 4] =
    [[0x83, 0x85, 0x9b, 0xcd], [0xcc, 0xa7, 0xad, 0x41], [0x4b, 0x2e, 0xd4, 0x33], [0xea, 0xcb, 0x2e, 0x04]];

/// Expanded permutation values for the P-box.
const PBOX: [u32; 32] = [
    0x00000001, 0x00000080, 0x00000400, 0x00002000, 0x00080000, 0x00200000, 0x01000000, 0x40000000, //
    0x00000008, 0x00000020, 0x00000100, 0x00004000, 0x00010000, 0x00800000, 0x04000000, 0x20000000, //
    0x00000004, 0x00000010, 0x00000200, 0x00008000, 0x00020000, 0x00400000, 0x08000000, 0x10000000, //
    0x00000002, 0x00000040, 0x00000800, 0x00001000, 0x00040000, 0x00100000, 0x02000000, 0x80000000,
];

/// The key rotation schedule.
const KEYROT: [usize; 16] = [0, 1, 2, 3, 2, 1, 3, 0, 1, 3, 2, 0, 3, 1, 0, 2];

/// Galois Field multiplication of `a` by `b`, modulo `m`: arithmetic
/// multiplication with additions and subtractions replaced by XOR.
const fn gf_mult(mut a: u32, mut b: u32, m: u32) -> u32 {
    let mut res = 0;
    while b != 0 {
        if b & 1 != 0 {
            res ^= a;
        }
        a <<= 1;
        b >>= 1;
        if a >= 256 {
            a ^= m;
        }
    }
    res
}

/// Galois Field exponentiation: `b` to the power of 7, modulo `m`.
pub const fn gf_exp7(b: u32, m: u32) -> u32 {
    if b == 0 {
        return 0;
    }
    let mut x = gf_mult(b, b, m);
    x = gf_mult(b, x, m);
    x = gf_mult(x, x, m);
    gf_mult(b, x, m)
}

/// The ICE 32-bit P-box permutation.
const fn perm32(mut x: u32) -> u32 {
    let mut res = 0;
    let mut i = 0;
    while x != 0 {
        if x & 1 != 0 {
            res |= PBOX[i];
        }
        i += 1;
        x >>= 1;
    }
    res
}

/// The four S-boxes, each entry already through the P-box.
type Sboxes = [[u32; 1024]; 4];

const fn sboxes_init() -> Sboxes {
    let mut sbox = [[0u32; 1024]; 4];
    let mut i = 0;
    while i < 1024 {
        let col = ((i >> 1) & 0xff) as u32;
        let row = (i & 0x1) | ((i & 0x200) >> 8);
        sbox[0][i] = perm32(gf_exp7(col ^ SXOR[0][row], SMOD[0][row]) << 24);
        sbox[1][i] = perm32(gf_exp7(col ^ SXOR[1][row], SMOD[1][row]) << 16);
        sbox[2][i] = perm32(gf_exp7(col ^ SXOR[2][row], SMOD[2][row]) << 8);
        sbox[3][i] = perm32(gf_exp7(col ^ SXOR[3][row], SMOD[3][row]));
        i += 1;
    }
    sbox
}

static SBOX: Sboxes = sboxes_init();

/// One round's subkey: three 20-bit values.
pub type Subkey = [u32; 3];

/// The single-round ICE f function.
pub fn ice_f(p: u32, sk: &Subkey) -> u32 {
    // Left and right half expansions (40-bit values).
    let tl = ((p >> 16) & 0x3ff) | (((p >> 14) | (p << 18)) & 0xffc00);
    let tr = (p & 0x3ff) | ((p << 2) & 0xffc00);
    // The salt permutation.
    let mut al = sk[2] & (tl ^ tr);
    let mut ar = al ^ tr;
    al ^= tl;
    // XOR with the subkey.
    al ^= sk[0];
    ar ^= sk[1];
    // S-box lookup and permutation.
    SBOX[0][(al >> 10) as usize] | SBOX[1][(al & 0x3ff) as usize] | SBOX[2][(ar >> 10) as usize] | SBOX[3][(ar & 0x3ff) as usize]
}

/// A key's schedule at a level, ready to encrypt and decrypt blocks.
#[derive(Clone)]
pub struct Ice {
    size: usize,
    keysched: Vec<Subkey>,
}

impl std::fmt::Debug for Ice {
    // Never the subkeys: they are the key.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Ice {{ rounds: {} }}", self.keysched.len())
    }
}

impl Ice {
    /// The schedule of `key` at `level` (0: Thin-ICE, 8 rounds; n ≥ 1:
    /// 16·n rounds and an 8·n-byte key). None when `key` is shorter than
    /// the level needs (bytes past that are ignored).
    pub fn new(level: u32, key: &[u8]) -> Option<Ice> {
        let (size, rounds) = if level < 1 { (1, 8) } else { (level as usize, 16 * level as usize) };
        if key.len() < 8 * size {
            return None;
        }
        let mut ice = Ice { size, keysched: vec![[0; 3]; rounds] };
        ice.set(key);
        Some(ice)
    }

    /// Set 8 rounds `[n, n+7]` of the key schedule.
    fn sched_build(&mut self, kb: &mut [u16; 4], n: usize, keyrot: &[usize]) {
        for (i, &kr) in keyrot.iter().take(8).enumerate() {
            let isk = &mut self.keysched[n + i];
            *isk = [0; 3];
            for j in 0..15 {
                let curr_sk = &mut isk[j % 3];
                for k in 0..4 {
                    let curr_kb = &mut kb[(kr + k) & 3];
                    let bit = *curr_kb & 1;
                    *curr_sk = (*curr_sk << 1) | bit as u32;
                    *curr_kb = (*curr_kb >> 1) | ((bit ^ 1) << 15);
                }
            }
        }
    }

    /// Set the key schedule.
    fn set(&mut self, key: &[u8]) {
        let word = |at: usize| ((key[at] as u16) << 8) | key[at + 1] as u16;
        if self.keysched.len() == 8 {
            let mut kb = [0u16; 4];
            for i in 0..4 {
                kb[3 - i] = word(i * 2);
            }
            self.sched_build(&mut kb, 0, &KEYROT);
            return;
        }
        let rounds = self.keysched.len();
        for i in 0..self.size {
            let mut kb = [0u16; 4];
            for j in 0..4 {
                kb[3 - j] = word(i * 8 + j * 2);
            }
            self.sched_build(&mut kb, i * 8, &KEYROT);
            self.sched_build(&mut kb, rounds - 8 - i * 8, &KEYROT[8..]);
        }
    }

    /// The number of rounds (8, or 16 per 8 key bytes).
    pub fn rounds(&self) -> usize {
        self.keysched.len()
    }

    /// Encrypt one 8-byte block.
    pub fn encrypt_block(&self, ptext: &[u8; 8]) -> [u8; 8] {
        let mut l = u32::from_be_bytes(ptext[..4].try_into().unwrap());
        let mut r = u32::from_be_bytes(ptext[4..].try_into().unwrap());
        for i in (0..self.keysched.len()).step_by(2) {
            l ^= ice_f(r, &self.keysched[i]);
            r ^= ice_f(l, &self.keysched[i + 1]);
        }
        store(r, l)
    }

    /// Decrypt one 8-byte block.
    pub fn decrypt_block(&self, ctext: &[u8; 8]) -> [u8; 8] {
        let mut l = u32::from_be_bytes(ctext[..4].try_into().unwrap());
        let mut r = u32::from_be_bytes(ctext[4..].try_into().unwrap());
        for i in (1..self.keysched.len()).rev().step_by(2) {
            l ^= ice_f(r, &self.keysched[i]);
            r ^= ice_f(l, &self.keysched[i - 1]);
        }
        store(r, l)
    }
}

/// The output block: `first` then `second`, big-endian.
fn store(first: u32, second: u32) -> [u8; 8] {
    let mut out = [0u8; 8];
    out[..4].copy_from_slice(&first.to_be_bytes());
    out[4..].copy_from_slice(&second.to_be_bytes());
    out
}

fn ctx(bytes: &[u8], f: impl Fn(&[u8; 8]) -> [u8; 8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut blocks = bytes.chunks_exact(8);
    for b in blocks.by_ref() {
        out.extend_from_slice(&f(b.try_into().unwrap()));
    }
    out.extend_from_slice(blocks.remainder());
    out
}

/// A `.ctx` file's bytes decrypted: each whole 8-byte block, in order;
/// the 0–7 trailing bytes as they are.
pub fn decrypt_ctx(ice: &Ice, bytes: &[u8]) -> Vec<u8> {
    ctx(bytes, |b| ice.decrypt_block(b))
}

/// Plain bytes encrypted as a `.ctx` file is (`decrypt_ctx`'s inverse).
pub fn encrypt_ctx(ice: &Ice, bytes: &[u8]) -> Vec<u8> {
    ctx(bytes, |b| ice.encrypt_block(b))
}

/// Whether `bytes` read as script text: no NUL but as trailing padding,
/// nearly every byte printable ASCII, whitespace or UTF-8 beyond ASCII.
pub fn looks_like_text(bytes: &[u8]) -> bool {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    let body = &bytes[..end];
    if body.is_empty() || body.contains(&0) {
        return false;
    }
    let odd = body
        .iter()
        .filter(|&&b| !(b.is_ascii_graphic() || b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' || b >= 0x80))
        .count();
    let high = body.iter().filter(|&&b| b >= 0x80).count();
    odd == 0 && high * 20 <= body.len()
}

/// The level `.ctx` files are decrypted at: 0 (Thin-ICE) or 1 (ICE),
/// both taking the 8-byte key (ice.md, open question 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CtxCipher {
    pub level: u32,
}

impl CtxCipher {
    /// The game's: Thin-ICE (checked on the install's scripts with the
    /// user's key, `dump tf2 --ice-probe`).
    pub const DEFAULT: CtxCipher = CtxCipher { level: 0 };

    /// `thin` (level 0) or `ice` (level 1).
    pub fn name(&self) -> String {
        if self.level == 0 { "thin".into() } else { "ice".into() }
    }

    /// The cipher `name` names (`name`'s inverse).
    pub fn from_name(name: &str) -> Option<CtxCipher> {
        match name.trim() {
            "thin" => Some(CtxCipher { level: 0 }),
            "ice" => Some(CtxCipher { level: 1 }),
            _ => None,
        }
    }

    /// `CHIMERA_TF2_ICE_LEVEL=<name>` when it names one, else `DEFAULT`.
    pub fn configured() -> CtxCipher {
        std::env::var(LEVEL_ENV).ok().and_then(|n| Self::from_name(&n)).unwrap_or(Self::DEFAULT)
    }

    /// The key's schedule at this level.
    pub fn ice(&self, key: &[u8; 8]) -> Ice {
        Ice::new(self.level, key).expect("an 8-byte key fits levels 0 and 1")
    }
}

/// The environment variable that selects a `CtxCipher` by name.
pub const LEVEL_ENV: &str = "CHIMERA_TF2_ICE_LEVEL";

/// Which levels (0, 1) decrypt `ctx_bytes` (a real `.ctx` file's bytes)
/// with `key` to text whose KeyValues has a block at its top (`dump tf2
/// --ice-probe`).
pub fn probe_ctx(key: &[u8; 8], ctx_bytes: &[u8]) -> Vec<CtxCipher> {
    [0, 1]
        .into_iter()
        .map(|level| CtxCipher { level })
        .filter(|c| super::scripts::parses(&decrypt_ctx(&c.ice(key), ctx_bytes)))
        .collect()
}
