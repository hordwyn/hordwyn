//! Raw object storage via opendal (fs, S3, memory). Crate-private on purpose: the only way to
//! write bytes from outside this crate is [`crate::StorageBudget`] (D-8, D-15 Q6).

use std::path::{Path, PathBuf};

use opendal::{ErrorKind, Operator, services};

use crate::config::{Backend, StorageConfig};

#[derive(Debug, thiserror::Error)]
#[error("storage backend: {0}")]
pub struct BackendError(pub String);

fn be(e: &opendal::Error) -> BackendError {
    BackendError(e.to_string())
}

/// The top-level prefixes (directories on the fs backend) of the blob store. Every key the
/// crate builds starts with one of them, and the move of an earlier layout goes over
/// [`prefix::ALL`]: a new prefix belongs here first (review V1OPS-3: `orig` was written but
/// not moved).
pub mod prefix {
    /// Files people hand in (`up/<user>/...`): today the import files. Pinned, in the backup.
    pub const UPLOAD: &str = "up";
    /// Image variants (`img/<hash>/<variant>.<format>`).
    pub const IMAGE: &str = "img";
    /// Cached provider responses (`http/<hash>`).
    pub const HTTP: &str = "http";
    /// Kept original images (`orig/<hash>`, `storage.keep_originals`).
    pub const ORIGINAL: &str = "orig";
    /// All of them, in the order they are moved.
    pub const ALL: [&str; 4] = [UPLOAD, IMAGE, HTTP, ORIGINAL];
}

/// The earlier root and the current one when the configuration asks for a move between two
/// different directories of the fs backend.
fn legacy_pair(cfg: &StorageConfig) -> Option<(&Path, &Path)> {
    if cfg.backend != Backend::Fs {
        return None;
    }
    let legacy = Path::new(cfg.fs_legacy_root.as_deref().filter(|l| !l.is_empty())?);
    let root = Path::new(&cfg.fs_root);
    let same = match (legacy.canonicalize(), root.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => legacy == root,
    };
    (!same).then_some((legacy, root))
}

/// The directories of the earlier layout that the next start of the server would move
/// (`storage.fs_legacy_root`), in the order of [`prefix::ALL`]. Changes nothing. The admin
/// CLI asks this before it opens the store and refuses to go on while the answer is not
/// empty (review V1OPS-5): a CLI of a new image, run while a server of the old image still
/// writes to the earlier directories, must not move them away under it.
#[must_use]
pub fn legacy_dirs_pending(cfg: &StorageConfig) -> Vec<PathBuf> {
    let Some((legacy, root)) = legacy_pair(cfg) else {
        return Vec::new();
    };
    prefix::ALL
        .iter()
        .filter(|name| legacy.join(name).is_dir() && !root.join(name).exists())
        .map(|name| legacy.join(name))
        .collect()
}

/// Move the blob directories of an earlier layout into `root` (`storage.fs_legacy_root`,
/// OPS-3): the container image used to keep the blobs in `/data` itself, around the secrets
/// key; they now live in `/data/blobs`. Each directory of [`prefix::ALL`] found under
/// `legacy` is renamed into `root` unless `root` already has it (then nothing is merged and
/// the earlier directory stays where it is). Nothing else under `legacy` is touched. Safe to
/// run at every start and from several processes of the same build at once: a directory
/// another process has just moved is simply no longer there.
///
/// # Errors
/// A rename that fails (for example across mounts), with both paths.
pub(crate) fn adopt_legacy_root(legacy: &Path, root: &Path) -> Result<(), BackendError> {
    for name in prefix::ALL {
        let (from, to) = (legacy.join(name), root.join(name));
        if !from.is_dir() {
            continue;
        }
        if to.exists() {
            tracing::warn!(
                from = %from.display(),
                to = %to.display(),
                "blob directory of the earlier layout left in place: the new one exists already"
            );
            continue;
        }
        match std::fs::rename(&from, &to) {
            Ok(()) => tracing::info!(
                from = %from.display(),
                to = %to.display(),
                "moved the blob directory of the earlier layout"
            ),
            // another process of the same deployment moved it between the check and now
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && to.exists() => {}
            Err(e) => {
                return Err(BackendError(format!(
                    "moving the blob directory {} to {}: {e}; move it by hand or set \
                     storage.fs_root to the earlier directory",
                    from.display(),
                    to.display()
                )));
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
pub(crate) struct RawStore {
    op: Operator,
}

impl RawStore {
    pub(crate) fn open(cfg: &StorageConfig) -> Result<Self, BackendError> {
        let op = match cfg.backend {
            Backend::Fs => {
                std::fs::create_dir_all(&cfg.fs_root)
                    .map_err(|e| BackendError(format!("{}: {e}", cfg.fs_root)))?;
                if let Some((legacy, root)) = legacy_pair(cfg) {
                    adopt_legacy_root(legacy, root)?;
                }
                Operator::new(services::Fs::default().root(&cfg.fs_root)).map_err(|e| be(&e))?
            }
            Backend::S3 => {
                // S3 needs opendal's HTTP transport; install it explicitly instead of relying
                // on a process constructor (opendal 0.59.5 `install_default`).
                opendal::install_default();
                let s3 = &cfg.s3;
                let mut b = services::S3::default()
                    .bucket(&s3.bucket)
                    .region(&s3.region)
                    .root(&s3.root)
                    // only explicit configuration, no ~/.aws or instance metadata lookups
                    .disable_config_load()
                    .disable_ec2_metadata();
                if let Some(ep) = &s3.endpoint {
                    b = b.endpoint(ep);
                }
                if let Some(k) = &s3.access_key_id {
                    b = b.access_key_id(k);
                }
                if let Some(s) = &s3.secret_access_key {
                    b = b.secret_access_key(s);
                }
                Operator::new(b).map_err(|e| be(&e))?
            }
            Backend::Memory => Operator::new(services::Memory::default()).map_err(|e| be(&e))?,
        };
        Ok(Self { op })
    }

    /// Unconditional write; callers know from the index whether the key exists (no HEAD).
    pub(crate) async fn write(&self, key: &str, bytes: Vec<u8>) -> Result<(), BackendError> {
        self.op
            .write(key, bytes)
            .await
            .map(|_| ())
            .map_err(|e| be(&e))
    }

    pub(crate) async fn read(&self, key: &str) -> Result<Option<Vec<u8>>, BackendError> {
        match self.op.read(key).await {
            Ok(buf) => Ok(Some(buf.to_vec())),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(be(&e)),
        }
    }

    /// Batch delete (one request per batch on S3 where supported).
    pub(crate) async fn delete(&self, keys: &[String]) -> Result<(), BackendError> {
        if keys.is_empty() {
            return Ok(());
        }
        self.op
            .delete_iter(keys.iter().map(String::as_str))
            .await
            .map_err(|e| be(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "blob-layout-{}-{name}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fs_config(legacy: &Path, root: &Path) -> StorageConfig {
        StorageConfig {
            backend: Backend::Fs,
            fs_root: root.display().to_string(),
            fs_legacy_root: Some(legacy.display().to_string()),
            ..StorageConfig::default()
        }
    }

    /// OPS-3: an instance whose blobs lie around the secrets key (`/data/up`, `/data/img`,
    /// `/data/http`, `/data/secrets.key`) starts on the new layout and still reads every
    /// object; the key and anything else in the earlier root stay where they are.
    #[tokio::test]
    async fn the_earlier_blob_directories_are_moved_and_stay_readable() {
        let data = temp("move");
        std::fs::create_dir_all(data.join("up/u1/imports")).unwrap();
        std::fs::write(data.join("up/u1/imports/a"), b"upload").unwrap();
        std::fs::create_dir_all(data.join("img/h1")).unwrap();
        std::fs::write(data.join("img/h1/card.webp"), b"variant").unwrap();
        std::fs::create_dir_all(data.join("http")).unwrap();
        std::fs::write(data.join("http/h2"), b"cached").unwrap();
        // review V1OPS-3: kept originals (`storage.keep_originals`) are the fourth prefix
        std::fs::create_dir_all(data.join("orig")).unwrap();
        std::fs::write(data.join("orig/h1"), b"original").unwrap();
        std::fs::write(data.join("secrets.key"), b"v1:key").unwrap();
        std::fs::create_dir_all(data.join("other")).unwrap();
        let root = data.join("blobs");

        let store = RawStore::open(&fs_config(&data, &root)).unwrap();
        assert_eq!(
            store.read("up/u1/imports/a").await.unwrap().as_deref(),
            Some(&b"upload"[..])
        );
        assert_eq!(
            store.read("img/h1/card.webp").await.unwrap().as_deref(),
            Some(&b"variant"[..])
        );
        assert_eq!(
            store.read("http/h2").await.unwrap().as_deref(),
            Some(&b"cached"[..])
        );
        assert_eq!(
            store.read("orig/h1").await.unwrap().as_deref(),
            Some(&b"original"[..])
        );
        for gone in ["up", "img", "http", "orig"] {
            assert!(!data.join(gone).exists(), "{gone} left behind");
        }
        assert_eq!(std::fs::read(data.join("secrets.key")).unwrap(), b"v1:key");
        assert!(!root.join("secrets.key").exists());
        assert!(data.join("other").is_dir());

        // the next start (and a second process) finds nothing to do
        let again = RawStore::open(&fs_config(&data, &root)).unwrap();
        assert_eq!(
            again.read("up/u1/imports/a").await.unwrap().as_deref(),
            Some(&b"upload"[..])
        );
        std::fs::remove_dir_all(&data).unwrap();
    }

    /// Review V1OPS-5: the admin CLI must not move the directories (an older server may
    /// still run on the volume), so it asks what a start of the server would move.
    #[tokio::test]
    async fn the_directories_a_start_would_move_are_named_and_left_alone() {
        let data = temp("pending");
        let root = data.join("blobs");
        std::fs::create_dir_all(data.join("img")).unwrap();
        std::fs::create_dir_all(data.join("orig")).unwrap();
        std::fs::create_dir_all(data.join("up")).unwrap();
        std::fs::create_dir_all(root.join("up")).unwrap();
        std::fs::create_dir_all(data.join("other")).unwrap();
        let cfg = fs_config(&data, &root);
        assert_eq!(
            legacy_dirs_pending(&cfg),
            vec![data.join("img"), data.join("orig")]
        );
        assert!(data.join("img").is_dir(), "asking moves nothing");
        // the server's start moves them; afterwards nothing is pending
        RawStore::open(&cfg).unwrap();
        assert_eq!(legacy_dirs_pending(&cfg).len(), 0);
        // nothing is pending without the setting, with the same root, or on another backend
        std::fs::create_dir_all(data.join("http")).unwrap();
        assert_eq!(legacy_dirs_pending(&cfg), vec![data.join("http")]);
        let none = StorageConfig {
            fs_legacy_root: None,
            ..cfg.clone()
        };
        assert_eq!(legacy_dirs_pending(&none).len(), 0);
        assert_eq!(legacy_dirs_pending(&fs_config(&data, &data)).len(), 0);
        let memory = StorageConfig {
            backend: Backend::Memory,
            ..cfg
        };
        assert_eq!(legacy_dirs_pending(&memory).len(), 0);
        std::fs::remove_dir_all(&data).unwrap();
    }

    /// Nothing is merged or overwritten, and "keep the old layout" (both settings name the
    /// same directory) moves nothing.
    #[tokio::test]
    async fn an_existing_directory_wins_and_the_same_root_is_left_alone() {
        let data = temp("keep");
        let root = data.join("blobs");
        std::fs::create_dir_all(data.join("up")).unwrap();
        std::fs::write(data.join("up/old"), b"old").unwrap();
        std::fs::create_dir_all(root.join("up")).unwrap();
        std::fs::write(root.join("up/new"), b"new").unwrap();
        let store = RawStore::open(&fs_config(&data, &root)).unwrap();
        assert_eq!(
            store.read("up/new").await.unwrap().as_deref(),
            Some(&b"new"[..])
        );
        assert_eq!(store.read("up/old").await.unwrap(), None);
        assert_eq!(std::fs::read(data.join("up/old")).unwrap(), b"old");

        let same = RawStore::open(&fs_config(&data, &data)).unwrap();
        assert_eq!(
            same.read("up/old").await.unwrap().as_deref(),
            Some(&b"old"[..])
        );
        assert!(data.join("up/old").exists());

        // without the setting an earlier directory next to the root is not looked at
        let plain = temp("plain");
        std::fs::create_dir_all(plain.join("up")).unwrap();
        let cfg = StorageConfig {
            fs_legacy_root: None,
            ..fs_config(&plain, &plain.join("blobs"))
        };
        RawStore::open(&cfg).unwrap();
        assert!(plain.join("up").is_dir());
        std::fs::remove_dir_all(&data).unwrap();
        std::fs::remove_dir_all(&plain).unwrap();
    }
}
