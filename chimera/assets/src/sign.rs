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
