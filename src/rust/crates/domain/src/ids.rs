//! Typed identifiers. All ids are `UUIDv7` values generated outside the domain (`IdGen` port),
//! so clients can create them offline.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! id_type {
    ($($(#[$m:meta])* $name:ident),* $(,)?) => {$(
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            #[must_use]
            pub const fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl From<Uuid> for $name {
            fn from(u: Uuid) -> Self {
                Self(u)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    )*};
}

id_type!(
    /// A user account on this instance.
    UserId,
    /// A person in a user's address book.
    CompanionId,
    /// A catalog item (movie, book, board game, ...).
    ItemId,
    /// A user's library entry for one catalog item.
    EntryId,
    /// An append-only progress event.
    EventId,
    /// A review.
    ReviewId,
    /// A user list.
    ListId,
    /// A session (play, watch party, gaming session).
    SessionId,
    /// A companion link request.
    LinkRequestId,
    /// An invitation that promotes a companion to a user.
    InviteId,
);
