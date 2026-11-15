//! The script reader (`game::scripts`) on fixtures made here: plain
//! KeyValues text encrypted as a `.ctx` file is (`ice::encrypt_ctx`, a
//! trailing partial block left plain) with a throwaway key this test
//! makes, then read back: as plain bytes (the asset store's record),
//! decrypted with the key, missing without one; through a mount and
//! through the packer's decrypting layer (`scripts::decrypting`); and the
//! probe (`ice::probe_ctx`) finding the level a file was written at.
//! No real key or game file is used.

use std::io;

use chimera_core::mount::{Entry, FileSource, Mount};
use tf2::game::{
    ice::{self, CtxCipher},
    scripts::{self, CtxKey, ScriptSource},
};

/// Made-up weapon script text (not the game's), 8·k + 5 bytes long so the
/// last block is partial.
fn fixture() -> Vec<u8> {
    let mut text = String::from(
        "WeaponData\r\n{\r\n\t\"printname\"\t\"#Fixture_Gun\"\r\n\t\"Damage\"\t\"6\"\r\n\t\"Spread\"\t\"0.0675\"\r\n\
         \t\"Bullets\"\t\"10\"\r\n\t// a comment\r\n\t\"SoundData\"\r\n\t{\r\n\t\t\"single_shot\"\t\"Fixture.Single\"\r\n\t}\r\n",
    );
    while (text.len() + 3) % 8 != 5 {
        text.push(' ');
    }
    text.push_str("}\r\n");
    assert_eq!(text.len() % 8, 5);
    text.into_bytes()
}

fn throwaway_key() -> CtxKey {
    // Made at run time from the process id and the clock: never the real
    // key, never the same twice.
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64
        ^ std::process::id() as u64;
    CtxKey(std::array::from_fn(|i| b'a' + ((seed >> (i * 5)) % 26) as u8))
}

#[test]
fn an_encrypted_fixture_reads_back_with_its_key() {
    let plain = fixture();
    let key = throwaway_key();
    let enc = ice::encrypt_ctx(&CtxCipher::DEFAULT.ice(&key.0), &plain);
    assert_ne!(enc, plain);
    assert_eq!(enc[enc.len() - 5..], plain[plain.len() - 5..], "the partial block stays plain");
    assert!(!scripts::parses(&enc));

    let s = scripts::from_bytes("tf_weapon_fixture", &enc, Some(&key), CtxCipher::DEFAULT);
    assert_eq!(s.source, ScriptSource::Decrypted, "{:?}", s.note);
    let data = s.data().expect("WeaponData");
    assert_eq!(data.string("printname"), Some("#Fixture_Gun"));
    assert_eq!(data.integer("damage"), Some(6));
    assert_eq!(data.number("Spread"), Some(0.0675));
    assert_eq!(data.integer("Spread"), None);
    assert_eq!(data.block("SoundData").and_then(|b| b.string("single_shot")), Some("Fixture.Single"));
    assert!(data.block("printname").is_none());

    // The store's record: plain bytes, no key needed.
    let p = scripts::from_bytes("tf_weapon_fixture", &plain, None, CtxCipher::DEFAULT);
    assert_eq!(p.source, ScriptSource::Plain);
    assert_eq!(p.root, s.root);

    // No key, or the wrong cipher: missing, with a note that never holds
    // the key.
    let none = scripts::from_bytes("tf_weapon_fixture", &enc, None, CtxCipher::DEFAULT);
    assert_eq!(none.source, ScriptSource::Missing);
    assert!(none.data().is_none());
    let other = CtxCipher { level: 1, ..CtxCipher::DEFAULT };
    let wrong = scripts::from_bytes("tf_weapon_fixture", &enc, Some(&key), other);
    assert_eq!(wrong.source, ScriptSource::Missing);
    let key_text = String::from_utf8(key.0.to_vec()).unwrap();
    for note in [none.note.unwrap(), wrong.note.unwrap()] {
        assert!(!note.contains(&key_text), "a note shows the key");
    }
    assert!(!format!("{key:?}").contains(&key_text));
}

#[test]
fn a_key_text_must_be_8_characters_and_errors_never_show_it() {
    assert!(CtxKey::from_text("abcdefgh", "test").is_ok());
    let e = CtxKey::from_text("secretkey9", "test").unwrap_err();
    assert!(!e.contains("secretkey9"));
}

/// Files in memory, as one mount layer.
struct Files(Vec<(String, Vec<u8>)>);

impl FileSource for Files {
    fn name(&self) -> String {
        "fixture".into()
    }

    fn entries(&self) -> Vec<Entry> {
        self.0.iter().map(|(p, b)| Entry { path: p.clone(), size: b.len() as u64 }).collect()
    }

    fn read(&self, path: &str) -> Option<io::Result<Vec<u8>>> {
        self.0.iter().find(|(p, _)| p == path).map(|(_, b)| Ok(b.clone()))
    }
}

fn mount_of(files: &[(&str, Vec<u8>)]) -> Mount {
    let mut m = Mount::default();
    m.push(Files(files.iter().map(|(p, b)| (p.to_string(), b.clone())).collect()));
    m
}

#[test]
fn the_reader_and_the_packers_layer_read_through_a_mount() {
    let plain = fixture();
    let key = throwaway_key();
    let enc = ice::encrypt_ctx(&CtxCipher::configured().ice(&key.0), &plain);
    let files = [("scripts/tf_weapon_fixture.ctx", enc.clone()), ("scripts/other.txt", b"x".to_vec())];

    let mount = mount_of(&files);
    assert_eq!(scripts::read(&mount, "tf_weapon_fixture", Some(&key)).source, ScriptSource::Decrypted);
    assert_eq!(scripts::read(&mount, "tf_weapon_fixture", None).source, ScriptSource::Missing);
    assert_eq!(scripts::read(&mount, "tf_weapon_nothing", Some(&key)).source, ScriptSource::Missing);

    // The packer's view with the key: the script reads decrypted (what
    // the store's record holds), other files as they are.
    let packed = scripts::decrypting(mount_of(&files), Some(key.clone()));
    assert_eq!(packed.read("scripts/tf_weapon_fixture.ctx").unwrap(), plain);
    assert_eq!(packed.read("scripts/other.txt").unwrap(), b"x");
    assert_eq!(scripts::read(&packed, "tf_weapon_fixture", None).source, ScriptSource::Plain);

    // Without a key, the scripts are left out.
    let hidden = scripts::decrypting(mount_of(&files), None);
    assert!(!hidden.contains("scripts/tf_weapon_fixture.ctx"));
    assert_eq!(hidden.entries().len(), 1);
    assert!(hidden.files_in("scripts").iter().all(|p| !p.ends_with(".ctx")));
}

/// The install's encrypted copy first and no key: the asset store's plain
/// record under it serves (a host's server or a client with both).
#[test]
fn a_store_copy_under_an_encrypted_install_copy_serves_without_a_key() {
    let plain = fixture();
    let key = throwaway_key();
    let enc = ice::encrypt_ctx(&CtxCipher::configured().ice(&key.0), &plain);
    let mut mount = mount_of(&[("scripts/tf_weapon_fixture.ctx", enc)]);
    mount.push(Files(vec![("scripts/tf_weapon_fixture.ctx".into(), plain)]));
    let s = scripts::read(&mount, "tf_weapon_fixture", None);
    assert_eq!(s.source, ScriptSource::Plain);
    assert_eq!(s.data().and_then(|d| d.integer("Damage")), Some(6));
}

#[test]
fn the_probe_finds_the_cipher_a_file_was_written_with() {
    let plain = fixture();
    let key = throwaway_key();
    let cipher = CtxCipher { level: 1 };
    let enc = ice::encrypt_ctx(&cipher.ice(&key.0), &plain);
    let found = ice::probe_ctx(&key.0, &enc);
    assert_eq!(found, [cipher], "{:?}", found.iter().map(CtxCipher::name).collect::<Vec<_>>());
}
