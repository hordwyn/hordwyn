//! Storage port for integrations, with an in-memory adapter for tests.

use std::sync::Mutex;

use app::ports::PortError;
use async_trait::async_trait;
use domain::ids::UserId;
use uuid::Uuid;

use crate::{CallRecord, Integration};

#[async_trait]
pub trait IntegrationStore: Send + Sync {
    async fn insert(&self, i: &Integration) -> Result<(), PortError>;
    async fn by_token_hash(&self, hash: &str) -> Result<Option<Integration>, PortError>;
    async fn by_user(&self, user: UserId) -> Result<Vec<Integration>, PortError>;
    async fn delete(&self, id: Uuid) -> Result<(), PortError>;
    /// Record an attributed call: when, its status, the error and field if it was refused
    /// (`None` clears them), the redacted body, and the item it recorded (`None` keeps the
    /// earlier one).
    async fn touch(&self, id: Uuid, call: &CallRecord) -> Result<(), PortError>;
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

    async fn touch(&self, id: Uuid, call: &CallRecord) -> Result<(), PortError> {
        for i in self
            .0
            .lock()
            .expect("lock")
            .iter_mut()
            .filter(|i| i.id == id)
        {
            i.last_seen_at = Some(call.at);
            i.last_error.clone_from(&call.error);
            i.last_status = Some(call.status);
            i.last_error_field.clone_from(&call.field);
            i.last_payload.clone_from(&call.payload);
            if call.item.is_some() {
                i.last_item.clone_from(&call.item);
            }
        }
        Ok(())
    }
}
