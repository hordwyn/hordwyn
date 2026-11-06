//! Raw object storage via opendal (fs, S3, memory). Crate-private on purpose: the only way to
//! write bytes from outside this crate is [`crate::StorageBudget`] (D-8, D-15 Q6).

use opendal::{ErrorKind, Operator, services};

use crate::config::{Backend, StorageConfig};

#[derive(Debug, thiserror::Error)]
#[error("storage backend: {0}")]
pub struct BackendError(pub String);

fn be(e: &opendal::Error) -> BackendError {
    BackendError(e.to_string())
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
