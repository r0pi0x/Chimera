//! ICE (`game::ice`, the port of the public-domain reference) against
//! specs/tf2/ice.md's test cases: the designer's certification triplets
//! for ICE, Thin-ICE and ICE-2 both ways (cases 1–4), the GF(2^8)
//! arithmetic and S-box permutations (cases 6, 7), the keyed
//! permutation's symmetry (14), round trips at Thin-ICE, ICE and ICE-2
//! (5) and the `.ctx` framing (16, 17). Keys are throwaway ones made
//! here or the published test keys.

use tf2::game::ice::{self, CtxCipher, Ice};

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
fn the_certification_triplets_hold() {
    // Cases 1–4: (level, key, ciphertext of fedcba9876543210).
    let plain = block("fedcba9876543210");
    for (level, key, expected) in [
        (1, "deadbeef01234567", "7d6ef1ef30d47a96"),
        (0, "deadbeef01234567", "de240d83a00a9cc0"),
        (2, "00112233445566778899aabbccddeeff", "f94840d86972f21c"),
    ] {
        let ice = Ice::new(level, &hex(key)).unwrap();
        assert_eq!(ice.encrypt_block(&plain), block(expected), "level {level} encrypt");
        assert_eq!(ice.decrypt_block(&block(expected)), plain, "level {level} decrypt");
    }
}

#[test]
fn gf_powers_are_the_specs() {
    // Case 6.
    assert_eq!(ice::gf_exp7(2, 333), 0x80);
    assert_eq!(ice::gf_exp7(3, 333), 0xff);
    assert_eq!(ice::gf_exp7(0, 333), 0);
    assert_eq!(ice::gf_exp7(1, 333), 1);
    // Case 7: every modulus gives a permutation.
    for m in [333, 313, 505, 369, 379, 375, 319, 391, 361, 445, 451, 397, 425, 395] {
        let mut seen = [false; 256];
        for y in 0..=255u32 {
            seen[ice::gf_exp7(y, m) as usize] = true;
        }
        assert!(seen.iter().all(|s| *s), "modulus {m}");
    }
}

#[test]
fn equal_halves_ignore_the_keyed_permutation() {
    // Case 14.
    let mut rng = Rng(7);
    for _ in 0..200 {
        let h = rng.next() as u32 & 0xffff;
        let x = (h << 16) | h;
        let sk3 = rng.next() as u32 & 0xfffff;
        assert_eq!(ice::ice_f(x, &[1, 2, sk3]), ice::ice_f(x, &[1, 2, 0]));
    }
}

#[test]
fn ciphers_have_names_that_read_back() {
    assert_eq!(CtxCipher::from_name(&CtxCipher::DEFAULT.name()), Some(CtxCipher::DEFAULT));
    assert_eq!(CtxCipher::from_name("ice").map(|c| c.level), Some(1));
    assert_eq!(CtxCipher::from_name("thin").map(|c| c.level), Some(0));
    assert_eq!(CtxCipher::from_name("cycled"), None);
}

#[test]
fn decrypt_undoes_encrypt_at_every_level() {
    // Case 5.
    let mut rng = Rng(0x1ce);
    for level in [0u32, 1, 2] {
        let key: [u8; 16] = rng.bytes();
        let ice = Ice::new(level, &key).unwrap();
        assert_eq!(ice.rounds(), [8, 16, 32][level as usize]);
        for _ in 0..1000 {
            let x: [u8; 8] = rng.bytes();
            let c = ice.encrypt_block(&x);
            assert_ne!(c, x);
            assert_eq!(ice.decrypt_block(&c), x, "level {level}");
        }
    }
    // Thin-ICE and ICE differ for the same key.
    let key: [u8; 8] = rng.bytes();
    let x: [u8; 8] = rng.bytes();
    assert_ne!(Ice::new(0, &key).unwrap().encrypt_block(&x), Ice::new(1, &key).unwrap().encrypt_block(&x));
    assert!(Ice::new(2, &key).is_none(), "ICE-2 needs 16 bytes");
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
