//! Per-file age encryption for non-local backup destinations.
//!
//! Local destinations stay plaintext (FileVault protects them). External
//! paths and git remotes route through here so a stolen drive or a
//! public git repo can't be read without the matching age identity.
//!
//! Identities live in the macOS Keychain. `keygen()` mints a new
//! identity and stashes it there; `load_identity_for` fetches it back
//! at restore time.

use age::secrecy::ExposeSecret;
use age::x25519::{Identity, Recipient};
use age::{Decryptor, Encryptor};
use keyring::Entry;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use walkdir::WalkDir;

use crate::error::{BackupError, Result};
use crate::manifest::Manifest;

const KEYRING_SERVICE: &str = "tesela-backup";

/// Extension we append to a backup file once it's been encrypted. The
/// manifest still records the *plaintext* logical path (`notes/foo.md`)
/// so restore knows what to recreate; on disk the file is at
/// `notes/foo.md.age` and is opaque ciphertext.
pub const AGE_SUFFIX: &str = ".age";

/// Produce a brand-new keypair and stash it in the macOS Keychain under
/// `(service=tesela-backup, account=<mosaic_root>)`. Returns the public
/// recipient string the user passes to `backup --encrypt`.
pub fn keygen_for_mosaic(mosaic_root: &Path) -> Result<String> {
    let identity = Identity::generate();
    let recipient = identity.to_public();
    let id_string = identity.to_string().expose_secret().to_string();

    let account = keyring_account(mosaic_root);
    let entry = Entry::new(KEYRING_SERVICE, &account).map_err(map_keyring_err)?;
    entry.set_password(&id_string).map_err(map_keyring_err)?;
    Ok(recipient.to_string())
}

/// Fetch the identity for the given mosaic from Keychain. Returns
/// `Ok(None)` when no entry exists (caller can then prompt the user to
/// run `tesela backup keygen`).
pub fn load_identity_for_mosaic(mosaic_root: &Path) -> Result<Option<Identity>> {
    let account = keyring_account(mosaic_root);
    let entry = Entry::new(KEYRING_SERVICE, &account).map_err(map_keyring_err)?;
    match entry.get_password() {
        Ok(s) => {
            let id = Identity::from_str(&s).map_err(|e| {
                BackupError::Other(anyhow::anyhow!("invalid identity in keychain: {}", e))
            })?;
            Ok(Some(id))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(map_keyring_err(e)),
    }
}

// Test-only override: when set, `identity_for_manifest` returns this
// identity instead of touching the Keychain. Lets the integration
// tests exercise the full backup→encrypt→restore→decrypt round trip
// without polluting the user's real keychain.
#[cfg(test)]
thread_local! {
    pub(crate) static TEST_IDENTITY_OVERRIDE: std::cell::RefCell<Option<Identity>> =
        const { std::cell::RefCell::new(None) };
}

/// Look up the matching identity for a manifest's recipient string. We
/// store identities keyed by mosaic path (Keychain account), so we
/// derive the lookup from the manifest's `mosaic_root` field. If the
/// stored recipient doesn't match what the manifest expects, we fail
/// loud — that indicates a keyring/mosaic mismatch.
pub fn identity_for_manifest(manifest: &Manifest) -> Result<Identity> {
    #[cfg(test)]
    {
        if let Some(id) = TEST_IDENTITY_OVERRIDE.with(|cell| cell.borrow().clone()) {
            return Ok(id);
        }
    }
    let recipient = match &manifest.encryption {
        crate::manifest::ManifestEncryption::Age { recipient } => recipient,
        crate::manifest::ManifestEncryption::None => {
            return Err(BackupError::Other(anyhow::anyhow!(
                "manifest is not encrypted; nothing to load"
            )));
        }
    };
    let id = load_identity_for_mosaic(&manifest.mosaic_root)?.ok_or_else(|| {
        BackupError::Other(anyhow::anyhow!(
            "no age identity in Keychain for mosaic {} — run `tesela backup keygen` first",
            manifest.mosaic_root.display()
        ))
    })?;
    let observed = id.to_public().to_string();
    if &observed != recipient {
        return Err(BackupError::Other(anyhow::anyhow!(
            "Keychain identity ({}) doesn't match manifest recipient ({}); has the mosaic moved or the key been rotated?",
            observed,
            recipient
        )));
    }
    Ok(id)
}

/// Walk a staging directory and encrypt every captured file in place
/// (renaming `foo` → `foo.age`). Skips `manifest.json` so the index
/// stays human-readable + scriptable. The manifest's `files[].path`
/// values are left untouched; the .age suffix is implicit and applied
/// by `restore`.
pub fn encrypt_staging(staging: &Path, recipient_str: &str) -> Result<()> {
    let recipient = Recipient::from_str(recipient_str).map_err(|e| {
        BackupError::Other(anyhow::anyhow!("invalid recipient {}: {}", recipient_str, e))
    })?;

    for entry in WalkDir::new(staging) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.file_name().and_then(|n| n.to_str()) == Some(Manifest::FILENAME) {
            continue;
        }
        let mut target = path.as_os_str().to_owned();
        target.push(AGE_SUFFIX);
        let target = PathBuf::from(target);
        encrypt_file(path, &target, &recipient)?;
        fs::remove_file(path)?;
    }
    Ok(())
}

fn encrypt_file(src: &Path, dst: &Path, recipient: &Recipient) -> Result<()> {
    let mut input = File::open(src)?;
    let output = File::create(dst)?;
    let encryptor =
        Encryptor::with_recipients(std::iter::once(recipient as &dyn age::Recipient))
            .map_err(|e| BackupError::Other(anyhow::anyhow!("age encryptor: {}", e)))?;
    let mut writer = encryptor
        .wrap_output(output)
        .map_err(|e| BackupError::Other(anyhow::anyhow!("age wrap_output: {}", e)))?;
    io::copy(&mut input, &mut writer)?;
    writer
        .finish()
        .map_err(|e| BackupError::Other(anyhow::anyhow!("age finish: {}", e)))?;
    Ok(())
}

/// Decrypt a `<rel>.age` file from `backup_root` into a Vec<u8>. Caller
/// then SHA-checks the plaintext against the manifest entry. Streaming
/// directly to disk would also work but the plaintext is what we
/// hash + write, so buffering in RAM keeps the unpack path simple.
pub fn decrypt_file_bytes(backup_root: &Path, rel: &str, identity: &Identity) -> Result<Vec<u8>> {
    let mut src = PathBuf::from(rel);
    src.as_mut_os_string().push(AGE_SUFFIX);
    let full = backup_root.join(&src);
    let file = File::open(&full).map_err(|e| {
        BackupError::Other(anyhow::anyhow!(
            "open encrypted file {}: {}",
            full.display(),
            e
        ))
    })?;
    let decryptor = Decryptor::new(file)
        .map_err(|e| BackupError::Other(anyhow::anyhow!("age decryptor: {}", e)))?;
    let mut reader = decryptor
        .decrypt(std::iter::once(identity as &dyn age::Identity))
        .map_err(|e| BackupError::Other(anyhow::anyhow!("age decrypt: {}", e)))?;
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    Ok(buf)
}

fn keyring_account(mosaic_root: &Path) -> String {
    mosaic_root.to_string_lossy().into_owned()
}

fn map_keyring_err(e: keyring::Error) -> BackupError {
    BackupError::Other(anyhow::anyhow!("keyring: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn round_trip_a_single_file() {
        let identity = Identity::generate();
        let recipient = identity.to_public().to_string();

        let tmp = TempDir::new().unwrap();
        let staging = tmp.path().join("stage");
        fs::create_dir_all(staging.join("notes")).unwrap();
        fs::write(staging.join("notes/foo.md"), b"hello world").unwrap();

        encrypt_staging(&staging, &recipient).unwrap();
        // Plain file gone, .age sibling present
        assert!(!staging.join("notes/foo.md").exists());
        assert!(staging.join("notes/foo.md.age").exists());

        let plaintext = decrypt_file_bytes(&staging, "notes/foo.md", &identity).unwrap();
        assert_eq!(plaintext, b"hello world");
    }

    #[test]
    fn encrypt_skips_manifest_json() {
        let identity = Identity::generate();
        let recipient = identity.to_public().to_string();

        let tmp = TempDir::new().unwrap();
        let staging = tmp.path().to_path_buf();
        fs::write(staging.join("manifest.json"), b"{}").unwrap();
        fs::write(staging.join("payload.bin"), b"\x00\x01\x02").unwrap();

        encrypt_staging(&staging, &recipient).unwrap();
        assert!(
            staging.join("manifest.json").exists(),
            "manifest.json must stay plaintext"
        );
        assert!(!staging.join("manifest.json.age").exists());
        assert!(staging.join("payload.bin.age").exists());
        assert!(!staging.join("payload.bin").exists());
    }
}
