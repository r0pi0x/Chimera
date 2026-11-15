//! Manifests signed by the server (docs/plans/active/asset-service.md, 4.1
//! and section 8): the server's Ed25519 key signs each manifest's exact
//! bytes; the signature sits beside it in the store
//! (`manifests/<game>/<build>.sig`, hex) and travels with it
//! (`X-Chimera-Signature`). A client checks it against the server's public
//! key (pinned with `[assets] key`, else asked of the server over HTTPS)
//! before using the manifest; the manifest's record ids then vouch for
//! every record. The private key is a file of the server's
//! (`<store>/server/signing.key`, hex, created on first use, mode 0600 on
//! Unix), never printed, sent or logged.

use std::{fs, io, path::Path};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::store::{hex, unhex, write_atomic};

/// The server's signing key, loaded from `path` or created there.
pub fn load_or_create_key(path: &Path) -> io::Result<SigningKey> {
    if let Ok(text) = fs::read_to_string(path) {
        let bytes = unhex::<32>(&text).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, format!("{}: not a signing key", path.display()))
        })?;
        return Ok(SigningKey::from_bytes(&bytes));
    }
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| io::Error::other(format!("no randomness: {e}")))?;
    write_atomic(path, hex(&seed).as_bytes())?;
    restrict(path);
    Ok(SigningKey::from_bytes(&seed))
}

/// Only the owner may read `path` (Unix; elsewhere the folder's ACL).
pub fn restrict(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// The public key as hex (what `/key` serves and `[assets] key` pins).
pub fn public_hex(key: &SigningKey) -> String {
    hex(key.verifying_key().as_bytes())
}

/// `text`'s signature, hex.
pub fn sign(key: &SigningKey, text: &[u8]) -> String {
    hex(&key.sign(text).to_bytes())
}

/// Whether `signature` (hex) is `public`'s (hex) over `text`.
pub fn verify(public: &str, text: &[u8], signature: &str) -> Result<(), String> {
    let public = unhex::<32>(public).ok_or("the server's key is not a 32-byte hex key")?;
    let public = VerifyingKey::from_bytes(&public).map_err(|e| format!("the server's key: {e}"))?;
    let signature = unhex::<64>(signature).ok_or("the signature is not 64 bytes of hex")?;
    public
        .verify_strict(text, &Signature::from_bytes(&signature))
        .map_err(|_| "bad signature: the manifest isn't the server's (tampered, or another key)".to_string())
}

/// The signature file beside a manifest file.
pub fn sig_path(manifest: &Path) -> std::path::PathBuf {
    manifest.with_extension("sig")
}

/// Sign the manifest file at `manifest` (writes its `.sig`).
pub fn sign_manifest_file(key: &SigningKey, manifest: &Path) -> io::Result<String> {
    let text = fs::read(manifest)?;
    let signature = sign(key, &text);
    write_atomic(&sig_path(manifest), signature.as_bytes())?;
    Ok(signature)
}

/// Sign `game`'s manifest of `build` (None: its newest) in a server's
/// `store` with that store's key (`server::key_path`, created on first
/// use), and nothing else: a manifest packed on another machine and
/// copied in with its records is signed where it is served, with no
/// install there (`chimera_pack sign`). The build signed and the key's
/// public half (hex).
pub fn sign_stored(store: &crate::Store, game: &str, build: Option<&str>) -> io::Result<(String, String)> {
    sign_in(store, game, build, true)
}

/// `sign_stored` for a store that already has its server's key: no key is
/// created (a folder that isn't a server's store is refused, not turned
/// into one), as `chimera_assets sign` runs it on the asset server's
/// machine.
pub fn sign_stored_with_existing_key(
    store: &crate::Store,
    game: &str,
    build: Option<&str>,
) -> io::Result<(String, String)> {
    sign_in(store, game, build, false)
}

fn sign_in(store: &crate::Store, game: &str, build: Option<&str>, create_key: bool) -> io::Result<(String, String)> {
    let build = match build {
        Some(b) => b.to_string(),
        None => store
            .latest_build(game)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no manifest of {game} in the store")))?,
    };
    let path = store.manifest_path(game, &build);
    if !path.is_file() {
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("{}: no such manifest", path.display())));
    }
    let key_file = crate::server::key_path(store);
    if !create_key && !key_file.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "{}: this store has no signing key (it is not a running asset server's store; the server creates its key when it first starts)",
                key_file.display()
            ),
        ));
    }
    let key = load_or_create_key(&key_file)?;
    sign_manifest_file(&key, &path)?;
    Ok((build, public_hex(&key)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Manifest, Store};

    /// Signing one game's newest manifest (or a named build) writes its
    /// `.sig` alone, verifiable with the store's key; another game's and
    /// an older build's stay unsigned.
    #[test]
    fn sign_stored_signs_one_games_manifest_only() {
        let dir = std::env::temp_dir().join(format!("chimera-sign-stored-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::new(&dir);
        for (game, build) in [("disc_game", "01.10.12.2276"), ("disc_game", "01.01.14.2342"), ("other", "7")] {
            Manifest::new(game, build, None).save(&store.manifest_path(game, build)).unwrap();
        }
        let (build, public) = sign_stored(&store, "disc_game", None).unwrap();
        assert_eq!(build, "01.10.12.2276", "the newest by name");
        let path = store.manifest_path("disc_game", &build);
        let signature = fs::read_to_string(sig_path(&path)).unwrap();
        verify(&public, &fs::read(&path).unwrap(), &signature).unwrap();
        assert!(!sig_path(&store.manifest_path("disc_game", "01.01.14.2342")).exists());
        assert!(!sig_path(&store.manifest_path("other", "7")).exists());
        // A named build; a missing one is an error, not a new key's use.
        sign_stored(&store, "disc_game", Some("01.01.14.2342")).unwrap();
        assert!(sig_path(&store.manifest_path("disc_game", "01.01.14.2342")).exists());
        assert!(sign_stored(&store, "disc_game", Some("2")).is_err());
        assert!(sign_stored(&store, "absent", None).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    /// The existing-key signing never makes a key: a store without one is
    /// refused and stays without.
    #[test]
    fn signing_with_the_existing_key_refuses_a_store_without_one() {
        let dir = std::env::temp_dir().join(format!("chimera-sign-nokey-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::new(&dir);
        Manifest::new("g", "1", None).save(&store.manifest_path("g", "1")).unwrap();
        let e = sign_stored_with_existing_key(&store, "g", None).unwrap_err();
        assert!(e.to_string().contains("no signing key"), "{e}");
        assert!(!crate::server::key_path(&store).exists(), "no key made");
        assert!(!sig_path(&store.manifest_path("g", "1")).exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
