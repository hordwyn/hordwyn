//! Notifications (D-17): event catalogue, subscriptions with filters, quiet hours, digests
//! and rate limits, templates with brand and admin overrides, the inbox, a delivery log with
//! dedupe, and the channels behind the [`Notifier`] port.
#![deny(unsafe_code)]

mod bridge;
pub mod catalogue;
pub mod channels;
mod deliver;
mod engine;
pub mod model;
#[cfg(feature = "pg")]
pub mod pg;
pub mod plan;
pub mod store;
pub mod template;

pub use bridge::AdminBridge;
pub use deliver::{Deliverer, Message, Notifier, SendError};
pub use engine::{DELIVERY_TOPIC, Engine, INBOX_TOPIC};
pub use store::{MemNotifyStore, NotifyStore};

/// Job sending one delivery (`{"delivery_id": ...}`).
pub const DELIVER_JOB: &str = "notify.deliver";
/// Job sending a channel's digest (`{"channel_id": ..., "until": ...}`).
pub const DIGEST_JOB: &str = "notify.digest";
