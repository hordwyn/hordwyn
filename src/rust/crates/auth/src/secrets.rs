//! Secrets at rest (addendum-1 section 2, OD-05 to OD-09): envelope encryption with a
//! versioned key-encryption key (KEK) from a key file or the environment, a random
//! data-encryption key (DEK) per value, AES-256-GCM for both layers and associated data that
//! binds every value to its purpose, row and field.
//!
//! Stored format (one `bytea`, fixed offsets):
//!
//! ```text
//! byte 0        format version (0x01)
//! bytes 1-2     KEK version (u16, big-endian)
//! bytes 3-14    DEK wrap nonce (12)
//! bytes 15-62   wrapped DEK (32 + 16 tag)
//! bytes 63-74   data nonce (12)
//! bytes 75-     ciphertext + 16-byte tag
//! ```
//!
//! Associated data is `sb1|<purpose>|<row>|<field>` for the value and the same plus `|dek`
//! for the wrapped DEK. Key material is held in `zeroize`d buffers and never printed.

use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use app::ports::{SecretCipher, SecretLocked, SecretSlot};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

/// Format byte of the stored value.
pub const FORMAT_V1: u8 = 0x01;
/// Associated-data tag of format 1 (the storage format, not a product name).
const AD_TAG: &str = "sb1";
const NONCE: usize = 12;
const KEY: usize = 32;
const TAG: usize = 16;
const WRAP_NONCE_AT: usize = 3;
const WRAPPED_AT: usize = WRAP_NONCE_AT + NONCE;
const DATA_NONCE_AT: usize = WRAPPED_AT + KEY + TAG;
const DATA_AT: usize = DATA_NONCE_AT + NONCE;

/// Environment variable with the keys (`v2:...,v1:...`), used when no key file exists.
pub const KEYS_ENV: &str = "APP__SECRETS__KEYS";
/// Environment variable naming the key file.
pub const KEY_FILE_ENV: &str = "APP__SECRETS__KEY_FILE";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecretsError {
    #[error("secrets key line {line}: {reason}")]
    BadKey { line: usize, reason: String },
    #[error("no secrets key is configured")]
    NoKeys,
    #[error("secrets key version v{0} is listed twice")]
    DuplicateVersion(u16),
    /// The database holds encrypted values but no key was found: refusing to start, since
    /// a new key could not decrypt them (addendum-1 2.2).
    #[error(
        "the database holds encrypted secrets but the key file {path} does not exist and \
         {KEYS_ENV} is not set; restore the key file (or set {KEYS_ENV}) and start again"
    )]
    MissingKey { path: String },
    #[error("secrets key file {path}: {error}")]
    Io { path: String, error: String },
}

/// Where the loaded keys came from (shown on the security page).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    File,
    Env,
    /// Generated on this start and written to the key file (OD-06).
    Generated,
    /// Generated in memory only (in-memory storage backend: tests, demos).
    Ephemeral,
}

impl KeySource {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Env => "env",
            Self::Generated => "generated",
            Self::Ephemeral => "ephemeral",
        }
    }
}

/// The loaded key versions; the first is active for new encryptions.
pub struct KeyRing {
    keys: Vec<(u16, Zeroizing<[u8; KEY]>)>,
}

impl std::fmt::Debug for KeyRing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyRing")
            .field("versions", &self.versions())
            .finish_non_exhaustive()
    }
}

fn random<const N: usize>() -> [u8; N] {
    crate::random_bytes::<N>()
}

fn cipher(key: &[u8; KEY]) -> Aes256Gcm {
    // a 32-byte slice is always a valid AES-256 key
    Aes256Gcm::new_from_slice(key).expect("32-byte key")
}

fn ad(slot: SecretSlot<'_>) -> String {
    format!("{AD_TAG}|{}|{}|{}", slot.purpose, slot.row, slot.field)
}

/// Key version stored in `blob` (bytes 1-2), if it is long enough and in format 1.
#[must_use]
pub fn blob_version(blob: &[u8]) -> Option<u16> {
    (blob.len() >= DATA_AT + TAG && blob[0] == FORMAT_V1)
        .then(|| u16::from_be_bytes([blob[1], blob[2]]))
}

impl KeyRing {
    /// Parse key lines `v<n>:<base64 of 32 bytes>`, newest first, separated by newlines or
    /// commas. Blank lines and `#` comments are ignored.
    ///
    /// # Errors
    /// A malformed line, a repeated version, or no key at all.
    pub fn parse(text: &str) -> Result<Self, SecretsError> {
        let mut keys: Vec<(u16, Zeroizing<[u8; KEY]>)> = Vec::new();
        for (i, raw) in text.split(['\n', ',']).enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let bad = |reason: &str| SecretsError::BadKey {
                line: i + 1,
                reason: reason.into(),
            };
            let (v, b64) = line
                .split_once(':')
                .ok_or_else(|| bad("expected v<n>:<base64>"))?;
            let version: u16 = v
                .strip_prefix('v')
                .and_then(|n| n.parse().ok())
                .filter(|n| *n > 0)
                .ok_or_else(|| bad("the version is v1, v2, ..."))?;
            let mut bytes = Zeroizing::new(
                STANDARD
                    .decode(b64.trim())
                    .map_err(|_| bad("the key is not base64"))?,
            );
            if bytes.len() != KEY {
                return Err(bad("the key must be 32 bytes"));
            }
            let mut key = Zeroizing::new([0u8; KEY]);
            key.copy_from_slice(&bytes);
            bytes.zeroize();
            if keys.iter().any(|(k, _)| *k == version) {
                return Err(SecretsError::DuplicateVersion(version));
            }
            keys.push((version, key));
        }
        if keys.is_empty() {
            return Err(SecretsError::NoKeys);
        }
        Ok(Self { keys })
    }

    /// A new ring with one random key `v1`, and its key-file text.
    #[must_use]
    pub fn generate() -> (Self, Zeroizing<String>) {
        let key = Zeroizing::new(random::<KEY>());
        let text = Zeroizing::new(format!(
            "# Secrets key: back it up together with the database. Newest version first.\n\
             v1:{}\n",
            STANDARD.encode(*key)
        ));
        (
            Self {
                keys: vec![(1, key)],
            },
            text,
        )
    }

    /// The version used for new encryptions.
    #[must_use]
    pub fn active_version(&self) -> u16 {
        self.keys[0].0
    }

    /// Loaded versions, newest first.
    #[must_use]
    pub fn versions(&self) -> Vec<u16> {
        self.keys.iter().map(|(v, _)| *v).collect()
    }

    fn key(&self, version: u16) -> Option<&[u8; KEY]> {
        self.keys
            .iter()
            .find(|(v, _)| *v == version)
            .map(|(_, k)| &**k)
    }

    fn wrap(&self, slot: SecretSlot<'_>, dek: &[u8; KEY]) -> (u16, [u8; NONCE], Vec<u8>) {
        self.wrap_with(slot, dek, random::<NONCE>())
    }

    fn wrap_with(
        &self,
        slot: SecretSlot<'_>,
        dek: &[u8; KEY],
        nonce: [u8; NONCE],
    ) -> (u16, [u8; NONCE], Vec<u8>) {
        let (version, kek) = (&self.keys[0].0, &self.keys[0].1);
        let dek_ad = format!("{}|dek", ad(slot));
        let wrapped = cipher(kek)
            .encrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: dek,
                    aad: dek_ad.as_bytes(),
                },
            )
            .expect("AES-GCM encryption of 32 bytes");
        (*version, nonce, wrapped)
    }

    fn unwrap_dek(
        &self,
        slot: SecretSlot<'_>,
        blob: &[u8],
    ) -> Result<Zeroizing<[u8; KEY]>, SecretLocked> {
        let version = blob_version(blob);
        let locked = SecretLocked { version };
        let kek = version.and_then(|v| self.key(v)).ok_or(locked.clone())?;
        let nonce: [u8; NONCE] = blob[WRAP_NONCE_AT..WRAPPED_AT]
            .try_into()
            .map_err(|_| locked.clone())?;
        let dek_ad = format!("{}|dek", ad(slot));
        let plain = Zeroizing::new(
            cipher(kek)
                .decrypt(
                    &Nonce::from(nonce),
                    Payload {
                        msg: &blob[WRAPPED_AT..DATA_NONCE_AT],
                        aad: dek_ad.as_bytes(),
                    },
                )
                .map_err(|_| locked.clone())?,
        );
        let mut dek = Zeroizing::new([0u8; KEY]);
        dek.copy_from_slice(plain.get(..KEY).ok_or(locked)?);
        Ok(dek)
    }

    fn assemble(version: u16, wrap_nonce: &[u8], wrapped: &[u8], rest: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(DATA_NONCE_AT + rest.len());
        out.push(FORMAT_V1);
        out.extend_from_slice(&version.to_be_bytes());
        out.extend_from_slice(wrap_nonce);
        out.extend_from_slice(wrapped);
        out.extend_from_slice(rest);
        out
    }

    /// Encrypt `plaintext` for `slot` with a fresh DEK, wrapped by the active KEK.
    #[must_use]
    pub fn encrypt(&self, slot: SecretSlot<'_>, plaintext: &[u8]) -> Vec<u8> {
        let dek = Zeroizing::new(random::<KEY>());
        self.encrypt_with(slot, plaintext, &dek, random::<NONCE>(), random::<NONCE>())
    }

    /// [`KeyRing::encrypt`] with given DEK and nonces (the golden-vector test).
    fn encrypt_with(
        &self,
        slot: SecretSlot<'_>,
        plaintext: &[u8],
        dek: &[u8; KEY],
        wrap_nonce: [u8; NONCE],
        data_nonce: [u8; NONCE],
    ) -> Vec<u8> {
        let value_ad = ad(slot);
        let ct = cipher(dek)
            .encrypt(
                &Nonce::from(data_nonce),
                Payload {
                    msg: plaintext,
                    aad: value_ad.as_bytes(),
                },
            )
            .expect("AES-GCM encryption");
        let (version, wrap_nonce, wrapped) = self.wrap_with(slot, dek, wrap_nonce);
        let mut rest = Vec::with_capacity(NONCE + ct.len());
        rest.extend_from_slice(&data_nonce);
        rest.extend_from_slice(&ct);
        Self::assemble(version, &wrap_nonce, &wrapped, &rest)
    }

    /// Decrypt a value of `slot`.
    ///
    /// # Errors
    /// [`SecretLocked`] for an unknown version, a wrong key, another slot or tampered bytes.
    pub fn decrypt(
        &self,
        slot: SecretSlot<'_>,
        blob: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, SecretLocked> {
        let dek = self.unwrap_dek(slot, blob)?;
        let locked = SecretLocked {
            version: blob_version(blob),
        };
        let nonce: [u8; NONCE] = blob[DATA_NONCE_AT..DATA_AT]
            .try_into()
            .map_err(|_| locked.clone())?;
        let value_ad = ad(slot);
        cipher(&dek)
            .decrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: &blob[DATA_AT..],
                    aad: value_ad.as_bytes(),
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| locked)
    }

    /// Re-wrap the DEK with the active KEK, keeping the value ciphertext ("rewrapping leaves
    /// the DEKs unchanged", OWASP key management). `None` when already on the active version.
    ///
    /// # Errors
    /// [`SecretLocked`] when the DEK cannot be unwrapped.
    pub fn rewrap(
        &self,
        slot: SecretSlot<'_>,
        blob: &[u8],
    ) -> Result<Option<Vec<u8>>, SecretLocked> {
        if blob_version(blob) == Some(self.active_version()) {
            // still prove the value is readable, so a wrong key is reported, not skipped
            self.unwrap_dek(slot, blob)?;
            return Ok(None);
        }
        let dek = self.unwrap_dek(slot, blob)?;
        let (version, wrap_nonce, wrapped) = self.wrap(slot, &dek);
        Ok(Some(Self::assemble(
            version,
            &wrap_nonce,
            &wrapped,
            &blob[DATA_NONCE_AT..],
        )))
    }
}

impl SecretCipher for KeyRing {
    fn seal(&self, slot: SecretSlot<'_>, plaintext: &[u8]) -> Vec<u8> {
        self.encrypt(slot, plaintext)
    }

    fn open(&self, slot: SecretSlot<'_>, blob: &[u8]) -> Result<Vec<u8>, SecretLocked> {
        self.decrypt(slot, blob)
            .map(|mut z| std::mem::take(&mut *z))
    }

    fn rewrap(&self, slot: SecretSlot<'_>, blob: &[u8]) -> Result<Option<Vec<u8>>, SecretLocked> {
        Self::rewrap(self, slot, blob)
    }
}

/// How to find the keys at start-up (addendum-1 2.2: key file first, then the environment).
#[derive(Clone, PartialEq, Eq)]
pub struct KeyOptions {
    pub key_file: PathBuf,
    /// `APP__SECRETS__KEYS`.
    pub env_keys: Option<Zeroizing<String>>,
    /// Use an in-memory key when nothing is configured (`secrets.ephemeral`, test harnesses
    /// only; refused when encrypted rows exist).
    pub ephemeral: bool,
}

impl std::fmt::Debug for KeyOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyOptions")
            .field("key_file", &self.key_file)
            .field("env_keys", &self.env_keys.as_ref().map(|_| "***"))
            .field("ephemeral", &self.ephemeral)
            .finish()
    }
}

fn io_err(path: &Path, e: &std::io::Error) -> SecretsError {
    SecretsError::Io {
        path: path.display().to_string(),
        error: e.to_string(),
    }
}

/// Write a new key file readable by the owner only (0600 on Unix); never overwrites.
fn write_key_file(path: &Path, text: &str) -> Result<(), SecretsError> {
    use std::io::Write;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| io_err(dir, &e))?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path).map_err(|e| io_err(path, &e))?;
    f.write_all(text.as_bytes())
        .and_then(|()| f.sync_all())
        .map_err(|e| io_err(path, &e))
}

#[cfg(unix)]
fn warn_if_readable_by_others(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(m) = std::fs::metadata(path)
        && m.permissions().mode() & 0o077 != 0
    {
        tracing::warn!(
            path = %path.display(),
            "the secrets key file is readable by other users; restrict it with chmod 600"
        );
    }
}

#[cfg(not(unix))]
fn warn_if_readable_by_others(_path: &Path) {}

/// Load the key ring: the key file if it exists, else `APP__SECRETS__KEYS`, else a new key.
/// A new key is written to the key file (OD-06), unless `has_encrypted_rows` says the
/// database already holds values a new key could not open: then start-up is refused.
///
/// # Errors
/// An unreadable or malformed key source, or [`SecretsError::MissingKey`].
pub fn load(
    opts: &KeyOptions,
    has_encrypted_rows: bool,
) -> Result<(KeyRing, KeySource), SecretsError> {
    let path = &opts.key_file;
    if path.exists() {
        warn_if_readable_by_others(path);
        let text = Zeroizing::new(std::fs::read_to_string(path).map_err(|e| io_err(path, &e))?);
        if opts.env_keys.is_some() {
            tracing::info!(
                path = %path.display(),
                "both the secrets key file and {KEYS_ENV} are set; the key file wins"
            );
        }
        return Ok((KeyRing::parse(&text)?, KeySource::File));
    }
    if let Some(env) = &opts.env_keys {
        return Ok((KeyRing::parse(env)?, KeySource::Env));
    }
    if opts.ephemeral && !has_encrypted_rows {
        tracing::warn!(
            "no secrets key configured; using a temporary key (secrets.ephemeral, tests only): \
             encrypted values are unreadable after a restart"
        );
        return Ok((KeyRing::generate().0, KeySource::Ephemeral));
    }
    if has_encrypted_rows {
        return Err(SecretsError::MissingKey {
            path: path.display().to_string(),
        });
    }
    let (ring, text) = KeyRing::generate();
    write_key_file(path, &text)?;
    tracing::info!(
        path = %path.display(),
        "generated a new secrets key; back it up together with the database"
    );
    Ok((ring, KeySource::Generated))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLOT: SecretSlot<'static> = SecretSlot {
        purpose: "connector_credential",
        row: "018f0000-0000-7000-8000-000000000001",
        field: "secret",
    };

    fn ring(lines: &str) -> KeyRing {
        KeyRing::parse(lines).unwrap()
    }

    fn key_line(v: u16, byte: u8) -> String {
        format!("v{v}:{}", STANDARD.encode([byte; 32]))
    }

    #[test]
    fn round_trip_and_layout() {
        let r = ring(&key_line(1, 7));
        let blob = r.encrypt(SLOT, b"tmdb-token");
        assert_eq!(blob[0], FORMAT_V1);
        assert_eq!(blob_version(&blob), Some(1));
        assert_eq!(blob.len(), DATA_AT + b"tmdb-token".len() + TAG);
        assert_eq!(&*r.decrypt(SLOT, &blob).unwrap(), b"tmdb-token");
        // fresh nonce and DEK per value
        assert_ne!(r.encrypt(SLOT, b"tmdb-token"), blob);
    }

    #[test]
    fn another_slot_does_not_decrypt() {
        let r = ring(&key_line(1, 7));
        let blob = r.encrypt(SLOT, b"x");
        for other in [
            SecretSlot {
                purpose: "channel_secret",
                ..SLOT
            },
            SecretSlot {
                row: "018f0000-0000-7000-8000-000000000002",
                ..SLOT
            },
            SecretSlot {
                field: "pin",
                ..SLOT
            },
        ] {
            assert_eq!(
                r.decrypt(other, &blob).unwrap_err(),
                SecretLocked { version: Some(1) }
            );
        }
    }

    #[test]
    fn unknown_version_and_wrong_key_are_locked() {
        let blob = ring(&key_line(1, 7)).encrypt(SLOT, b"x");
        let v2_only = ring(&key_line(2, 8));
        assert_eq!(
            v2_only.decrypt(SLOT, &blob).unwrap_err(),
            SecretLocked { version: Some(1) }
        );
        let wrong = ring(&key_line(1, 9));
        assert!(wrong.decrypt(SLOT, &blob).is_err());
        assert_eq!(
            wrong.decrypt(SLOT, b"short").unwrap_err(),
            SecretLocked { version: None }
        );
    }

    #[test]
    fn every_tampered_byte_fails() {
        let r = ring(&key_line(1, 7));
        let blob = r.encrypt(SLOT, b"abc");
        for i in 0..blob.len() {
            let mut t = blob.clone();
            t[i] ^= 0x01;
            assert!(r.decrypt(SLOT, &t).is_err(), "byte {i}");
        }
    }

    #[test]
    fn rotation_rewraps_without_touching_the_value() {
        let old = ring(&key_line(1, 7));
        let blob = old.encrypt(SLOT, b"secret");
        let both = ring(&format!("{}\n{}", key_line(2, 8), key_line(1, 7)));
        assert_eq!(both.active_version(), 2);
        let new = both.rewrap(SLOT, &blob).unwrap().expect("re-wrapped");
        assert_eq!(blob_version(&new), Some(2));
        assert_eq!(
            new[DATA_NONCE_AT..],
            blob[DATA_NONCE_AT..],
            "value unchanged"
        );
        assert_eq!(&*both.decrypt(SLOT, &new).unwrap(), b"secret");
        // idempotent: already on the active version
        assert_eq!(both.rewrap(SLOT, &new).unwrap(), None);
        // v1 may now be dropped
        assert_eq!(
            &*ring(&key_line(2, 8)).decrypt(SLOT, &new).unwrap(),
            b"secret"
        );
    }

    #[test]
    fn parsing_rules() {
        let r = ring(&format!(
            "# comment\n\n{}, {}\n",
            key_line(3, 1),
            key_line(1, 2)
        ));
        assert_eq!(r.versions(), vec![3, 1]);
        assert_eq!(KeyRing::parse("").unwrap_err(), SecretsError::NoKeys);
        assert!(matches!(
            KeyRing::parse("x1:abc"),
            Err(SecretsError::BadKey { line: 1, .. })
        ));
        assert!(matches!(
            KeyRing::parse(&format!("v1:{}", STANDARD.encode([1u8; 16]))),
            Err(SecretsError::BadKey { .. })
        ));
        assert_eq!(
            KeyRing::parse(&format!("{},{}", key_line(1, 1), key_line(1, 2))).unwrap_err(),
            SecretsError::DuplicateVersion(1)
        );
    }

    #[test]
    fn debug_prints_no_key_material() {
        let line = key_line(1, 0x41);
        let r = ring(&line);
        let shown = format!("{r:?}");
        assert!(!shown.contains(&line[3..]), "{shown}");
        assert!(shown.contains("versions"), "{shown}");
        let opts = KeyOptions {
            key_file: "k".into(),
            env_keys: Some(Zeroizing::new(line.clone())),
            ephemeral: false,
        };
        assert!(!format!("{opts:?}").contains(&line[3..]));
        assert!(!format!("{:?}", KeyRing::generate().1.len()).contains(&line[3..]));
    }

    /// Golden vector: a value written by format 1 must stay readable, so a future change of
    /// algorithm is a new format byte, never a silent change.
    #[test]
    fn golden_vector_of_format_1() {
        let r = ring(&key_line(1, 0x11));
        let made = r.encrypt_with(SLOT, b"golden", &[0x22; 32], [0x42; 12], [0x44; 12]);
        let made_hex = hex_of(&made);
        // cross-checked with Node.js crypto (OpenSSL AES-256-GCM), 2026-11-07
        let hex = "01000142424242424242424242424233af3db4bebd57085f6f2af6024696d23bf9a8ea5d\
                   f63e4c4bb0b47a7b4bd69fe5fb2e2995781f6bc40708e8ab9ae9854444444444444444\
                   44444444f560c907874d57578f8ce89ee2c8c3c12fc16adc1c25";
        let blob: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(made_hex, hex.replace(' ', ""), "format 1 changed");
        assert_eq!(&*r.decrypt(SLOT, &blob).unwrap(), b"golden");
    }

    #[test]
    fn load_prefers_the_file_then_env_then_generates_once() {
        let dir = std::env::temp_dir().join(format!("secrets-{}", uuid_like()));
        let path = dir.join("secrets.key");
        let env = Some(Zeroizing::new(key_line(5, 3)));
        let opts = KeyOptions {
            key_file: path.clone(),
            env_keys: env.clone(),
            ephemeral: false,
        };
        // no file: env
        let (r, src) = load(&opts, true).unwrap();
        assert_eq!((r.versions(), src), (vec![5], KeySource::Env));
        // nothing: refuse when the database holds encrypted rows
        let none = KeyOptions {
            env_keys: None,
            ..opts.clone()
        };
        assert!(matches!(
            load(&none, true),
            Err(SecretsError::MissingKey { .. })
        ));
        // nothing and an empty database: generate the file with 0600
        let (r, src) = load(&none, false).unwrap();
        assert_eq!((r.versions(), src), (vec![1], KeySource::Generated));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        // the file now wins over env, and is read back as the same key
        let blob = r.encrypt(SLOT, b"v");
        let (again, src) = load(&opts, true).unwrap();
        assert_eq!(src, KeySource::File);
        assert_eq!(&*again.decrypt(SLOT, &blob).unwrap(), b"v");
        // ephemeral when asked for and nothing is configured, but never over encrypted rows
        // a temporary key could not open (UX2-1)
        let eph = KeyOptions {
            key_file: dir.join("absent.key"),
            env_keys: None,
            ephemeral: true,
        };
        assert!(matches!(
            load(&eph, true),
            Err(SecretsError::MissingKey { .. })
        ));
        assert_eq!(load(&eph, false).unwrap().1, KeySource::Ephemeral);
        assert!(!dir.join("absent.key").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn hex_of(bytes: &[u8]) -> String {
        use std::fmt::Write;
        bytes.iter().fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
    }

    fn uuid_like() -> String {
        hex_of(&crate::random_bytes::<8>())
    }
}
