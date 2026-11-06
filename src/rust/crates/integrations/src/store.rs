//! Storage port for integrations, with an in-memory adapter for tests.

use std::sync::Mutex;

use app::ports::PortError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::ids::UserId;
use uuid::Uuid;

use crate::Integration;

#[async_trait]
pub trait IntegrationStore: Send + Sync {
    async fn insert(&self, i: &Integration) -> Result<(), PortError>;
    async fn by_token_hash(&self, hash: &str) -> Result<Option<Integration>, PortError>;
    async fn by_user(&self, user: UserId) -> Result<Vec<Integration>, PortError>;
    async fn delete(&self, id: Uuid) -> Result<(), PortError>;
    /// Record a call: when, and the error if it failed (`None` clears it).
    async fn touch(
        &self,
        id: Uuid,
        at: DateTime<Utc>,
        error: Option<String>,
    ) -> Result<(), PortError>;
}

#[derive(Default)]
pub struct MemIntegrationStore(Mutex<Vec<Integration>>);

#[async_trait]
impl IntegrationStore for MemIntegrationStore {
    async fn insert(&self, i: &Integration) -> Result<(), PortError> {
        self.0.lock().expect("lock").push(i.clone());
        Ok(())
    }

    async fn by_token_hash(&self, hash: &str) -> Result<Option<Integration>, PortError> {
        Ok(self
            .0
            .lock()
            .expect("lock")
            .iter()
            .find(|i| i.token_hash == hash)
            .cloned())
    }

    async fn by_user(&self, user: UserId) -> Result<Vec<Integration>, PortError> {
        Ok(self
            .0
            .lock()
            .expect("lock")
            .iter()
            .filter(|i| i.user == user)
            .cloned()
            .collect())
    }

    async fn delete(&self, id: Uuid) -> Result<(), PortError> {
        self.0.lock().expect("lock").retain(|i| i.id != id);
        Ok(())
    }

    async fn touch(
        &self,
        id: Uuid,
        at: DateTime<Utc>,
        error: Option<String>,
    ) -> Result<(), PortError> {
        for i in self
            .0
            .lock()
            .expect("lock")
            .iter_mut()
            .filter(|i| i.id == id)
        {
            i.last_seen_at = Some(at);
            i.last_error.clone_from(&error);
        }
        Ok(())
    }
}
