//! ICE, the 64-bit block cipher TF2's encrypted script files (`.ctx`) are
//! written with: our own code, from specs/tf2/ice.md alone (no other
//! implementation was read). `Ice` is a key's schedule at a level
//! (Thin-ICE, level 0: 8 rounds; ICE, level 1: 16; ICE-n: 16·n, nested)
//! with `encrypt_block` / `decrypt_block`; `encrypt_ctx` / `decrypt_ctx`
//! are the files' framing (plain ECB, a trailing partial block left in
//! the clear; ice.md, "TF2's encrypted script files").
//!
//! The spec's description does not reproduce the paper's three
//! certification triplets (ice.md, open question 1), so the places where
//! its prose admits more than one reading are switches of a `Reading`:
//! how the 60 drawn key bits fill the three subkeys (`Fill`), which key
//! word feeds which of the four key registers and in which byte order,
//! which end of a register a bit is drawn from, whether the bit put back
//! is inverted, which end a subkey is filled from, which generated value
//! plays SK1, SK2 and SK3, whether the round's rotation is added or
//! subtracted, how the block's halves are loaded and which way the P-box
//! table is read. `Reading::SPEC` is the spec's own reading (its
//! preferred "(b) cycled" fill), the default; `Reading::all` lists every
//! combination, which the tests try against the paper's triplets and
//! `dump tf2 --ice-probe` against a real `.ctx` with the user's key
//! (`probe_ctx`). `CtxCipher` is a level with a reading, as `.ctx` files
//! are decrypted (`DEFAULT`: Thin-ICE in the spec's reading); its stable
//! name (`thin/cycled-0123-bi-123`) is what the probe prints, and
//! `CHIMERA_TF2_ICE_READING=<name>` selects one at run time
//! (`CtxCipher::configured`) so the probe's answer can be tried before it
//! is fixed in code.
//!
//! Known shortcuts:
//! - No reading reproduces the paper's triplets (ice.md, open question 1;
//!   the tests record it). The reading `.ctx` files need is settled by
//!   `dump tf2 --ice-probe` with the user's key; until then `SPEC` is a
//!   guess.

/// The four S-boxes' XOR offsets by row (ice.md, "S-box XOR offsets").
const OFFSETS: [[u8; 4]; 4] = [
    [0x83, 0x85, 0x9b, 0xcd],
    [0xcc, 0xa7, 0xad, 0x41],
    [0x4b, 0x2e, 0xd4, 0x33],
    [0xea, 0xcd, 0x2e, 0x04],
];

/// The four S-boxes' moduli by row: degree-8 polynomials over GF(2)
/// (ice.md, "S-box moduli").
const MODULI: [[u16; 4]; 4] = [
    [333, 313, 505, 369],
    [379, 375, 319, 391],
    [361, 445, 451, 397],
    [397, 425, 395, 505],
];

/// The P-box: for each output bit, 31 first, the S-box (1..4) and its bit
/// that lands there (ice.md, "P-box").
const PBOX: [(u8, u8); 32] = [
    (1, 7), (4, 7), (3, 7), (2, 7), (2, 6), (3, 6), (1, 6), (4, 6),
    (3, 5), (2, 5), (4, 5), (1, 5), (4, 4), (1, 4), (2, 4), (3, 4),
    (2, 3), (3, 3), (4, 3), (1, 3), (1, 2), (4, 2), (2, 2), (3, 2),
    (4, 1), (1, 1), (3, 1), (2, 1), (3, 0), (2, 0), (1, 0), (4, 0),
];

/// The level-1 schedule's register rotation by round (ice.md, "Key
/// rotations").
const KEY_ROTATION: [usize; 16] = [0, 1, 2, 3, 2, 1, 3, 0, 1, 3, 2, 0, 3, 1, 0, 2];

/// Which subkey a round's draw `d` (0..59) goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fill {
    /// Draws 0–19 to the first, 20–39 the second, 40–59 the third
    /// (ice.md, reading (a)).
    Blocked,
    /// Groups of four draws to the first, second, third, first, ...
    /// (reading (b), the spec's choice).
    Cycled,
    /// Draw `d` to subkey `d mod 3`.
    BitCycled,
}

/// One reading of the spec's ambiguous places (module doc). Every
/// combination of the fields is a cipher; `SPEC` is the spec's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Reading {
    pub fill: Fill,
    /// For each key register KB0..KB3, which 16-bit word of the key feeds
    /// it (word 0 = key bytes 0 and 1). The spec: `[0, 1, 2, 3]`.
    pub registers: [u8; 4],
    /// A register's word taken as (second byte, first byte) instead of
    /// (first, second).
    pub swap_register_bytes: bool,
    /// Draw a register's bit 15 and rotate left (the bit put back into
    /// bit 0), instead of its bit 0 rotating right (put back into bit 15).
    pub draw_from_top: bool,
    /// The drawn bit is put back complemented (the spec) or as it was.
    pub invert: bool,
    /// A subkey filled from its bit 19 down (each draw shifted in at the
    /// top) instead of shifted in at bit 0.
    pub fill_from_top: bool,
    /// Which generated 20-bit value is SK1, SK2 and SK3 (the spec: the
    /// first, second and third).
    pub roles: [u8; 3],
    /// Draw `d` uses register `(d mod 4 − KR) mod 4` instead of `+ KR`.
    pub rotation_subtracts: bool,
    /// The block's halves loaded little-endian instead of big-endian.
    pub little_endian_block: bool,
    /// The P-box table read the other way (the S-box word's bit `o` goes
    /// to the position the table names for output bit `o`).
    pub pbox_inverted: bool,
}

impl Default for Reading {
    fn default() -> Self {
        Self::SPEC
    }
}

const PERMUTATIONS_4: [[u8; 4]; 24] = {
    let mut out = [[0u8; 4]; 24];
    let mut n = 0;
    let mut i = 0;
    while i < 256 {
        let (a, b, c, d) = (i & 3, (i >> 2) & 3, (i >> 4) & 3, (i >> 6) & 3);
        if a != b && a != c && a != d && b != c && b != d && c != d {
            out[n] = [a as u8, b as u8, c as u8, d as u8];
            n += 1;
        }
        i += 1;
    }
    out
};

const PERMUTATIONS_3: [[u8; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];

impl Reading {
    /// The spec's reading: reading (b), the registers in key order,
    /// bits drawn from bit 0 and put back inverted at bit 15, subkeys
    /// filled from bit 0, SK1 SK2 SK3 in order, big-endian halves.
    pub const SPEC: Reading = Reading {
        fill: Fill::Cycled,
        registers: [0, 1, 2, 3],
        swap_register_bytes: false,
        draw_from_top: false,
        invert: true,
        fill_from_top: false,
        roles: [0, 1, 2],
        rotation_subtracts: false,
        little_endian_block: false,
        pbox_inverted: false,
    };

    /// Every combination of the switches (`SPEC` first).
    pub fn all() -> Vec<Reading> {
        let mut out = vec![Self::SPEC];
        for fill in [Fill::Cycled, Fill::Blocked, Fill::BitCycled] {
            for registers in PERMUTATIONS_4 {
                for roles in PERMUTATIONS_3 {
                    for bits in 0..64u32 {
                        let r = Reading {
                            fill,
                            registers,
                            roles,
                            swap_register_bytes: bits & 1 != 0,
                            draw_from_top: bits & 2 != 0,
                            invert: bits & 4 == 0,
                            fill_from_top: bits & 8 != 0,
                            rotation_subtracts: bits & 16 != 0,
                            little_endian_block: bits & 32 != 0,
                            pbox_inverted: false,
                        };
                        for pbox_inverted in [false, true] {
                            let r = Reading { pbox_inverted, ..r };
                            if r != Self::SPEC {
                                out.push(r);
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// A stable name: `fill-registers-switches-roles`, e.g. the spec's
    /// `cycled-0123-bi-123` (switch letters: `s` swapped register bytes,
    /// `t` drawn from the top, `b` drawn from the bottom, `i` inverted,
    /// `n` not inverted, `f` subkeys filled from the top, `m` rotation
    /// subtracted, `l` little-endian block, `p` P-box inverted).
    pub fn name(&self) -> String {
        let fill = match self.fill {
            Fill::Blocked => "blocked",
            Fill::Cycled => "cycled",
            Fill::BitCycled => "bitcycled",
        };
        let regs: String = self.registers.iter().map(|r| char::from(b'0' + r)).collect();
        let mut sw = String::new();
        if self.swap_register_bytes {
            sw.push('s');
        }
        sw.push(if self.draw_from_top { 't' } else { 'b' });
        sw.push(if self.invert { 'i' } else { 'n' });
        for (on, c) in [
            (self.fill_from_top, 'f'),
            (self.rotation_subtracts, 'm'),
            (self.little_endian_block, 'l'),
            (self.pbox_inverted, 'p'),
        ] {
            if on {
                sw.push(c);
            }
        }
        let roles: String = self.roles.iter().map(|r| char::from(b'1' + r)).collect();
        format!("{fill}-{regs}-{sw}-{roles}")
    }

    /// The reading `name` names (`name`'s inverse).
    pub fn from_name(name: &str) -> Option<Reading> {
        Self::all().into_iter().find(|r| r.name() == name.trim())
    }

}

/// Multiply in GF(2^8) modulo `m` (carry-less, reduced; ice.md, "S-box
/// contents").
const fn gf_mul(mut a: u16, mut b: u16, m: u16) -> u16 {
    let mut out = 0;
    while b != 0 {
        if b & 1 != 0 {
            out ^= a;
        }
        b >>= 1;
        a <<= 1;
        if a & 0x100 != 0 {
            a ^= m;
        }
    }
    out
}

/// `y^7` in GF(2^8) modulo `m`.
pub const fn gf_pow7(y: u8, m: u16) -> u8 {
    let y = y as u16;
    let mut out = 1;
    let mut i = 0;
    while i < 7 {
        out = gf_mul(out, y, m);
        i += 1;
    }
    out as u8
}

/// S-box `i` (0..3) at a 10-bit input: row from the outer bits, column
/// from the inner eight.
pub const fn sbox(i: usize, x: u16) -> u8 {
    let row = ((((x >> 9) & 1) << 1) | (x & 1)) as usize;
    let col = ((x >> 1) & 0xff) as u8;
    gf_pow7(col ^ OFFSETS[i][row], MODULI[i][row])
}

/// The four S-boxes with the P-box folded in: each entry the S-box's 8
/// output bits already at their places in F's output.
type Tables = [[u32; 1024]; 4];

const fn make_tables(pbox_inverted: bool) -> Tables {
    let mut t = [[0u32; 1024]; 4];
    let mut i = 0;
    while i < 4 {
        let mut x = 0;
        while x < 1024 {
            // The S-box word: S1 in bits 31..24, S2 23..16, S3 15..8,
            // S4 7..0.
            let word = (sbox(i, x as u16) as u32) << (24 - 8 * i);
            t[i][x] = permute(word, pbox_inverted);
            x += 1;
        }
        i += 1;
    }
    t
}

/// Built at compile time: read-only tables, no process-wide state.
static TABLES: Tables = make_tables(false);
static TABLES_INVERTED: Tables = make_tables(true);

fn tables(pbox_inverted: bool) -> &'static Tables {
    if pbox_inverted { &TABLES_INVERTED } else { &TABLES }
}

/// The P-box on an S-box word (S1 in its top byte ... S4 in its bottom).
const fn permute(word: u32, inverted: bool) -> u32 {
    let mut out = 0;
    let mut k = 0;
    while k < 32 {
        let (s, bit) = PBOX[k];
        let to = 31 - k as u32;
        let from = (24 - 8 * (s as u32 - 1)) + bit as u32;
        if inverted {
            out |= ((word >> to) & 1) << from;
        } else {
            out |= ((word >> from) & 1) << to;
        }
        k += 1;
    }
    out
}

/// One round's subkeys: SK1, SK2, SK3 (20 bits each).
pub type Subkeys = [u32; 3];

/// The round function F (ice.md, "The round function F").
pub fn round(p: u32, sk: &Subkeys, pbox_inverted: bool) -> u32 {
    let e1 = ((p & 3) << 8) | (p >> 24);
    let e2 = (p >> 16) & 0x3ff;
    let e3 = (p >> 8) & 0x3ff;
    let e4 = p & 0x3ff;
    let (mut a, mut b) = ((e1 << 10) | e2, (e3 << 10) | e4);
    let d = sk[2] & (a ^ b);
    a ^= d ^ sk[0];
    b ^= d ^ sk[1];
    let t = tables(pbox_inverted);
    t[0][(a >> 10) as usize] | t[1][(a & 0x3ff) as usize] | t[2][(b >> 10) as usize] | t[3][(b & 0x3ff) as usize]
}

/// The level-1 schedule of one 8-byte key chunk: 16 rounds' subkeys
/// (ice.md, "Level-1 key schedule").
pub fn schedule16(key: &[u8; 8], reading: &Reading) -> [Subkeys; 16] {
    let word = |w: u8| {
        let (hi, lo) = (key[2 * w as usize] as u16, key[2 * w as usize + 1] as u16);
        if reading.swap_register_bytes { (lo << 8) | hi } else { (hi << 8) | lo }
    };
    let mut kb = reading.registers.map(word);
    let mut out = [[0u32; 3]; 16];
    for (n, round) in out.iter_mut().enumerate() {
        let kr = KEY_ROTATION[n];
        let mut generated = [0u32; 3];
        for d in 0..60usize {
            let reg = if reading.rotation_subtracts { (d % 4 + 4 - kr) % 4 } else { (d % 4 + kr) % 4 };
            let r = &mut kb[reg];
            let back = |b: u16| if reading.invert { b ^ 1 } else { b };
            let b = if reading.draw_from_top {
                let b = *r >> 15;
                *r = (*r << 1) | back(b);
                b
            } else {
                let b = *r & 1;
                *r = (*r >> 1) | (back(b) << 15);
                b
            } as u32;
            let target = match reading.fill {
                Fill::Blocked => d / 20,
                Fill::Cycled => (d / 4) % 3,
                Fill::BitCycled => d % 3,
            };
            let g = &mut generated[target];
            *g = if reading.fill_from_top { (*g >> 1) | (b << 19) } else { ((*g << 1) | b) & 0xfffff };
        }
        *round = reading.roles.map(|i| generated[i as usize]);
    }
    out
}

/// A key's round subkeys at a level, ready to encrypt and decrypt blocks.
#[derive(Clone)]
pub struct Ice {
    rounds: Vec<Subkeys>,
    reading: Reading,
}

impl std::fmt::Debug for Ice {
    // Never the subkeys: they are the key.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Ice {{ rounds: {}, reading: {} }}", self.rounds.len(), self.reading.name())
    }
}

impl Ice {
    /// The schedule of `key` at `level` (0: Thin-ICE, 8 rounds; n ≥ 1:
    /// 16·n rounds, `key` 8·n bytes, chunk 1 first) in `reading`. None
    /// when `key` is shorter than the level needs.
    pub fn new(level: u32, key: &[u8], reading: Reading) -> Option<Ice> {
        let chunks = level.max(1) as usize;
        if key.len() < 8 * chunks {
            return None;
        }
        let mut rounds: Vec<Subkeys> = Vec::new();
        for c in 0..chunks {
            let chunk: [u8; 8] = key[8 * c..8 * c + 8].try_into().ok()?;
            let s = schedule16(&chunk, &reading);
            let half = rounds.len() / 2;
            rounds.splice(half..half, s);
        }
        if level == 0 {
            rounds.truncate(8);
        }
        Some(Ice { rounds, reading })
    }

        pub fn reading(&self) -> Reading {
        self.reading
    }

    fn load(&self, block: &[u8; 8]) -> (u32, u32) {
        let half = |b: &[u8]| {
            let b: [u8; 4] = b.try_into().unwrap();
            if self.reading.little_endian_block { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) }
        };
        (half(&block[..4]), half(&block[4..]))
    }

    fn store(&self, l: u32, r: u32) -> [u8; 8] {
        let half = |v: u32| if self.reading.little_endian_block { v.to_le_bytes() } else { v.to_be_bytes() };
        let mut out = [0u8; 8];
        out[..4].copy_from_slice(&half(l));
        out[4..].copy_from_slice(&half(r));
        out
    }

    fn crypt(&self, block: &[u8; 8], order: impl Iterator<Item = usize>) -> [u8; 8] {
        let (mut l, mut r) = self.load(block);
        for i in order {
            l ^= round(r, &self.rounds[i], self.reading.pbox_inverted);
            std::mem::swap(&mut l, &mut r);
        }
        // No exchange after the last round: undo the loop's last one.
        self.store(r, l)
    }

    pub fn encrypt_block(&self, block: &[u8; 8]) -> [u8; 8] {
        self.crypt(block, 0..self.rounds.len())
    }

    pub fn decrypt_block(&self, block: &[u8; 8]) -> [u8; 8] {
        self.crypt(block, (0..self.rounds.len()).rev())
    }
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

/// What a `.ctx` file is decrypted with: a level (0 Thin-ICE, the spec's
/// understanding, or 1 ICE; ice.md, open question 3) and a `Reading`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CtxCipher {
    pub level: u32,
    pub reading: Reading,
}

impl CtxCipher {
    /// The spec's: Thin-ICE in the spec's reading.
    pub const DEFAULT: CtxCipher = CtxCipher { level: 0, reading: Reading::SPEC };

    /// `thin/<reading>` or `ice/<reading>` (`Reading::name`).
    pub fn name(&self) -> String {
        let level = if self.level == 0 { "thin" } else { "ice" };
        format!("{level}/{}", self.reading.name())
    }

    /// The cipher `name` names (`name`'s inverse).
    pub fn from_name(name: &str) -> Option<CtxCipher> {
        let (level, reading) = name.trim().split_once('/')?;
        let level = match level {
            "thin" => 0,
            "ice" => 1,
            _ => return None,
        };
        Some(CtxCipher { level, reading: Reading::from_name(reading)? })
    }

    /// `CHIMERA_TF2_ICE_READING=<name>` (what `dump tf2 --ice-probe`
    /// prints) when it names one, else `DEFAULT`.
    pub fn configured() -> CtxCipher {
        std::env::var(READING_ENV).ok().and_then(|n| Self::from_name(&n)).unwrap_or(Self::DEFAULT)
    }

    /// The key's schedule in this cipher.
    pub fn ice(&self, key: &[u8; 8]) -> Ice {
        Ice::new(self.level, key, self.reading).expect("an 8-byte key fits levels 0 and 1")
    }
}

/// The environment variable that selects a `CtxCipher` by name.
pub const READING_ENV: &str = "CHIMERA_TF2_ICE_READING";

/// Which ciphers (levels 0 and 1, every reading) decrypt `ctx_bytes` (a
/// real `.ctx` file's bytes) to text whose KeyValues has a block at its
/// top, for `key`: the oracle of ice.md's open question 1 (`dump tf2
/// --ice-probe`). Each candidate is tried on the first blocks before the
/// whole file.
pub fn probe_ctx(key: &[u8; 8], ctx_bytes: &[u8]) -> Vec<CtxCipher> {
    let head = &ctx_bytes[..ctx_bytes.len().min(32) & !7];
    let mut out = Vec::new();
    for reading in Reading::all() {
        for level in [0, 1] {
            let cipher = CtxCipher { level, reading };
            let ice = cipher.ice(key);
            if !looks_like_text(&decrypt_ctx(&ice, head)) {
                continue;
            }
            if super::scripts::parses(&decrypt_ctx(&ice, ctx_bytes)) {
                out.push(cipher);
            }
        }
    }
    out
}
