//! ICE (`game::ice`) against specs/tf2/ice.md's test cases: the GF(2^8)
//! arithmetic and S-box permutations (cases 6, 7), the spec's own derived
//! S-box and F values (11–13; they pin our code to the spec's reading),
//! the keyed permutation's symmetry (14), the paper's certification
//! triplets (1–3) searched over every `Reading`, round trips at Thin-ICE,
//! ICE and ICE-2 (5) and the `.ctx` framing (16, 17). Keys are throwaway
//! ones made here.

use tf2::game::ice::{self, CtxCipher, Fill, Ice, Reading};

/// A small deterministic generator for throwaway keys and blocks.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn bytes<const N: usize>(&mut self) -> [u8; N] {
        std::array::from_fn(|_| self.next() as u8)
    }
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn block(s: &str) -> [u8; 8] {
    hex(s).try_into().unwrap()
}

#[test]
fn gf_powers_are_the_specs() {
    // Case 6.
    assert_eq!(ice::gf_pow7(2, 333), 0x80);
    assert_eq!(ice::gf_pow7(3, 333), 0xff);
    assert_eq!(ice::gf_pow7(0, 333), 0);
    assert_eq!(ice::gf_pow7(1, 333), 1);
    // Case 7: every modulus gives a permutation.
    for m in [333, 313, 505, 369, 379, 375, 319, 391, 361, 445, 451, 397, 425, 395] {
        let mut seen = [false; 256];
        for y in 0..=255u8 {
            seen[ice::gf_pow7(y, m) as usize] = true;
        }
        assert!(seen.iter().all(|s| *s), "modulus {m}");
    }
}

#[test]
fn sboxes_and_f_are_the_specs_derived_values() {
    // Case 11.
    let inputs = [0x000, 0x001, 0x200, 0x201, 0x3ff];
    let expected: [[u8; 5]; 4] = [
        [0x17, 0x1f, 0x82, 0x95, 0x7a],
        [0x11, 0x43, 0xab, 0x2b, 0xe3],
        [0xcf, 0x2b, 0x81, 0x1d, 0xdf],
        [0xbc, 0x24, 0xe6, 0x99, 0x92],
    ];
    for (i, row) in expected.iter().enumerate() {
        let got: Vec<u8> = inputs.iter().map(|&x| ice::sbox(i, x)).collect();
        assert_eq!(got, row, "S{}", i + 1);
    }
    // Case 12.
    let zero = [0, 0, 0];
    for (x, f) in [(0, 0x642e6d6e), (1, 0xa6365d6e), (0x8000_0000, 0x643e656e), (0xffff_ffff, 0x7e5d51fc), (0x1234_5678, 0x45a8bb84)] {
        assert_eq!(ice::round(x, &zero, false), f, "F({x:#x})");
    }
    // Case 13.
    assert_eq!(ice::round(0x1234_5678, &[0xfffff, 0, 0], false), 0xd7eaa196);
    assert_eq!(ice::round(0x1234_5678, &[0, 0, 0xfffff], false), 0x5573b477);
}

#[test]
fn equal_halves_ignore_the_keyed_permutation() {
    // Case 14.
    let mut rng = Rng(7);
    for _ in 0..200 {
        let h = rng.next() as u32 & 0xffff;
        let x = (h << 16) | h;
        let sk3 = rng.next() as u32 & 0xfffff;
        assert_eq!(ice::round(x, &[1, 2, sk3], false), ice::round(x, &[1, 2, 0], false));
    }
}

/// The paper's three certification triplets (cases 1–3).
fn triplets_hold(reading: Reading) -> [bool; 3] {
    let plain = block("fedcba9876543210");
    let one = Ice::new(1, &hex("deadbeef01234567"), reading).unwrap();
    let thin = Ice::new(0, &hex("deadbeef01234567"), reading).unwrap();
    let two = Ice::new(2, &hex("00112233445566778899aabbccddeeff"), reading).unwrap();
    [
        one.encrypt_block(&plain) == block("7d6ef1ef30d47a96"),
        thin.encrypt_block(&plain) == block("de240d83a00a9cc0"),
        two.encrypt_block(&plain) == block("f94840d86972f21c"),
    ]
}

/// Every reading tried against the triplets. None reproduces them
/// (ice.md, open question 1): when one does, it must become
/// `Reading::SPEC` (and this test then says so).
#[test]
fn no_reading_reproduces_the_papers_triplets_yet() {
    let all = Reading::all();
    assert_eq!(all[0], Reading::SPEC);
    let mut partial = Vec::new();
    let mut full = Vec::new();
    for r in &all {
        let held = triplets_hold(*r);
        if held.iter().all(|h| *h) {
            full.push(r.name());
        } else if held.iter().any(|h| *h) {
            partial.push((r.name(), held));
        }
    }
    eprintln!("{} readings tried; all three triplets: {full:?}; some: {partial:?}", all.len());
    if let Some(first) = full.first() {
        assert_eq!(
            first,
            &Reading::SPEC.name(),
            "reading {first} reproduces the paper's triplets: make it Reading::SPEC and update ice.md"
        );
    }
    assert!(full.is_empty() && partial.is_empty(), "a reading reproduces the triplets now: {full:?} {partial:?}");
}

#[test]
fn readings_have_unique_names_that_read_back() {
    let all = Reading::all();
    let names: std::collections::HashSet<String> = all.iter().map(Reading::name).collect();
    assert_eq!(names.len(), all.len());
    assert_eq!(Reading::SPEC.name(), "cycled-0123-bi-123");
    let blocked = Reading { fill: Fill::Blocked, roles: [2, 0, 1], ..Reading::SPEC };
    assert_eq!(Reading::from_name(&blocked.name()), Some(blocked));
    assert_eq!(CtxCipher::from_name(&CtxCipher::DEFAULT.name()), Some(CtxCipher::DEFAULT));
    assert_eq!(CtxCipher::DEFAULT.name(), "thin/cycled-0123-bi-123");
    assert_eq!(CtxCipher::from_name("ice/cycled-0123-bi-123").map(|c| c.level), Some(1));
}

#[test]
fn decrypt_undoes_encrypt_at_every_level() {
    // Case 5, for a few readings.
    let mut rng = Rng(0x1ce);
    let readings = [Reading::SPEC, Reading { fill: Fill::Blocked, ..Reading::SPEC }, Reading { draw_from_top: true, pbox_inverted: true, little_endian_block: true, ..Reading::SPEC }];
    for reading in readings {
        for level in [0u32, 1, 2] {
            let key: [u8; 16] = rng.bytes();
            let ice = Ice::new(level, &key, reading).unwrap();
            for _ in 0..300 {
                let x: [u8; 8] = rng.bytes();
                let c = ice.encrypt_block(&x);
                assert_ne!(c, x);
                assert_eq!(ice.decrypt_block(&c), x, "level {level} {}", reading.name());
            }
        }
    }
    // Thin-ICE and ICE differ for the same key.
    let key: [u8; 8] = rng.bytes();
    let x: [u8; 8] = rng.bytes();
    assert_ne!(
        Ice::new(0, &key, Reading::SPEC).unwrap().encrypt_block(&x),
        Ice::new(1, &key, Reading::SPEC).unwrap().encrypt_block(&x)
    );
    assert!(Ice::new(2, &key, Reading::SPEC).is_none(), "ICE-2 needs 16 bytes");
}

#[test]
fn ctx_framing_leaves_the_partial_block_plain() {
    // Cases 16, 17.
    let mut rng = Rng(99);
    let key: [u8; 8] = rng.bytes();
    let ice = CtxCipher::DEFAULT.ice(&key);
    let plain: [u8; 19] = rng.bytes();
    let enc = ice::encrypt_ctx(&ice, &plain);
    assert_eq!(enc.len(), 19);
    assert_eq!(enc[16..], plain[16..]);
    assert_eq!(enc[..8], ice.encrypt_block(&plain[..8].try_into().unwrap()));
    assert_eq!(ice::decrypt_ctx(&ice, &enc), plain);
    let five = [1u8, 2, 3, 4, 5];
    assert_eq!(ice::encrypt_ctx(&ice, &five), five);
    assert_eq!(ice::decrypt_ctx(&ice, &five), five);
}
