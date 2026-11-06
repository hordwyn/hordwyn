//! Sign-up rules and auth error codes, shared by the server and the web client so the live
//! checklist in the browser and the server's checks agree (design.md 5.2).

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

/// Usernames are `USERNAME_MIN_LEN..=USERNAME_MAX_LEN` characters.
pub const USERNAME_MIN_LEN: usize = 3;
pub const USERNAME_MAX_LEN: usize = 32;
/// Characters allowed besides `a-z` and `0-9`.
pub const USERNAME_PUNCTUATION: &str = "_.-";
/// Display names are at most this many characters (an empty one falls back to the username).
pub const DISPLAY_NAME_MAX_LEN: usize = 100;
/// Minimum password length (NIST SP 800-63B recommends at least 8; we ask for 10).
pub const PASSWORD_MIN_LEN: usize = 10;
pub const PASSWORD_MAX_LEN: usize = 1024;

/// The form a username is stored in: trimmed and lowercased ("Robin " -> "robin").
#[must_use]
pub fn normalize_username(raw: &str) -> String {
    raw.trim().to_lowercase()
}

/// The display name that is stored: trimmed; empty means the (normalised) username.
#[must_use]
pub fn normalize_display_name(raw: &str, username: &str) -> String {
    match raw.trim() {
        "" => username.to_string(),
        d => d.to_string(),
    }
}

/// One username rule; each has its own error code and message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum UsernameRule {
    TooShort,
    TooLong,
    /// Contains a space (the UI suggests `-` or `_`).
    Space,
    /// A character other than `a-z`, `0-9` and [`USERNAME_PUNCTUATION`].
    InvalidChars,
}

impl UsernameRule {
    /// In the order they are reported.
    pub const ALL: [Self; 4] = [
        Self::TooShort,
        Self::TooLong,
        Self::Space,
        Self::InvalidChars,
    ];

    /// Does `normalized` break this rule?
    #[must_use]
    pub fn broken_by(self, normalized: &str) -> bool {
        let n = normalized.chars().count();
        match self {
            Self::TooShort => n < USERNAME_MIN_LEN,
            Self::TooLong => n > USERNAME_MAX_LEN,
            Self::Space => normalized.chars().any(char::is_whitespace),
            Self::InvalidChars => normalized.chars().any(|c| {
                !(c.is_ascii_lowercase()
                    || c.is_ascii_digit()
                    || c.is_whitespace()
                    || USERNAME_PUNCTUATION.contains(c))
            }),
        }
    }

    #[must_use]
    pub fn error(self) -> AuthErrorCode {
        match self {
            Self::TooShort => AuthErrorCode::UsernameTooShort,
            Self::TooLong => AuthErrorCode::UsernameTooLong,
            Self::Space => AuthErrorCode::UsernameSpace,
            Self::InvalidChars => AuthErrorCode::UsernameInvalidChars,
        }
    }
}

/// Every rule `normalized` breaks, in [`UsernameRule::ALL`] order (for a live checklist).
#[must_use]
pub fn broken_username_rules(normalized: &str) -> Vec<UsernameRule> {
    UsernameRule::ALL
        .into_iter()
        .filter(|r| r.broken_by(normalized))
        .collect()
}

/// Check a normalised username ([`normalize_username`]).
///
/// # Errors
/// The first broken rule.
pub fn check_username(normalized: &str) -> Result<(), UsernameRule> {
    match broken_username_rules(normalized).first() {
        Some(r) => Err(*r),
        None => Ok(()),
    }
}

/// Answer of the username availability check. `username` is the normalised form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum UsernameAvailability {
    Available {
        username: String,
    },
    /// Up to three free variants are suggested.
    Taken {
        username: String,
        suggestions: Vec<String>,
    },
    Invalid {
        username: String,
        rule: UsernameRule,
    },
}

/// Body of `POST /auth/username-available`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct UsernameCheckDto {
    pub username: String,
    /// The invite token on `/invite/:token` (needed when sign-up is invite-only).
    #[serde(default)]
    pub invite: Option<String>,
}

/// The sign-up / sign-in form field an error belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthField {
    Username,
    DisplayName,
    Password,
    Totp,
}

/// Every error code the sign-up and sign-in endpoints answer with (besides `internal`).
/// The web client translates `auth-error-<code>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthErrorCode {
    UsernameTooShort,
    UsernameTooLong,
    UsernameSpace,
    UsernameInvalidChars,
    UsernameTaken,
    DisplayNameTooLong,
    PasswordTooShort,
    PasswordTooLong,
    InvalidInvite,
    SignupClosed,
    /// Wrong username or password (deliberately not saying which).
    InvalidCredentials,
    TotpRequired,
    InvalidTotp,
    TooManyAttempts,
}

impl AuthErrorCode {
    pub const ALL: [Self; 14] = [
        Self::UsernameTooShort,
        Self::UsernameTooLong,
        Self::UsernameSpace,
        Self::UsernameInvalidChars,
        Self::UsernameTaken,
        Self::DisplayNameTooLong,
        Self::PasswordTooShort,
        Self::PasswordTooLong,
        Self::InvalidInvite,
        Self::SignupClosed,
        Self::InvalidCredentials,
        Self::TotpRequired,
        Self::InvalidTotp,
        Self::TooManyAttempts,
    ];

    /// The stable code in `ErrorDto::code`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UsernameTooShort => "username_too_short",
            Self::UsernameTooLong => "username_too_long",
            Self::UsernameSpace => "username_space",
            Self::UsernameInvalidChars => "username_invalid_chars",
            Self::UsernameTaken => "username_taken",
            Self::DisplayNameTooLong => "display_name_too_long",
            Self::PasswordTooShort => "password_too_short",
            Self::PasswordTooLong => "password_too_long",
            Self::InvalidInvite => "invalid_invite",
            Self::SignupClosed => "signup_closed",
            Self::InvalidCredentials => "invalid_credentials",
            Self::TotpRequired => "totp_required",
            Self::InvalidTotp => "invalid_totp",
            Self::TooManyAttempts => "too_many_attempts",
        }
    }

    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.code() == code)
    }

    /// The field to show the message at; `None` means a message for the whole form.
    #[must_use]
    pub const fn field(self) -> Option<AuthField> {
        match self {
            Self::UsernameTooShort
            | Self::UsernameTooLong
            | Self::UsernameSpace
            | Self::UsernameInvalidChars
            | Self::UsernameTaken => Some(AuthField::Username),
            Self::DisplayNameTooLong => Some(AuthField::DisplayName),
            Self::PasswordTooShort | Self::PasswordTooLong => Some(AuthField::Password),
            Self::InvalidTotp => Some(AuthField::Totp),
            Self::InvalidInvite
            | Self::SignupClosed
            | Self::InvalidCredentials
            | Self::TotpRequired
            | Self::TooManyAttempts => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames_are_normalised_then_checked() {
        assert_eq!(normalize_username("  Robin "), "robin");
        assert_eq!(check_username(&normalize_username(" Robin ")), Ok(()));
        assert_eq!(check_username("ab"), Err(UsernameRule::TooShort));
        assert_eq!(check_username(&"a".repeat(33)), Err(UsernameRule::TooLong));
        assert_eq!(check_username("anna k"), Err(UsernameRule::Space));
        assert_eq!(check_username("me@x.org"), Err(UsernameRule::InvalidChars));
        assert_eq!(check_username("a.b-c_9"), Ok(()));
        assert_eq!(
            broken_username_rules("a @"),
            vec![UsernameRule::Space, UsernameRule::InvalidChars]
        );
        assert_eq!(normalize_display_name("  ", "anna"), "anna");
        assert_eq!(normalize_display_name(" Anna K ", "anna"), "Anna K");
    }

    #[test]
    fn codes_round_trip_and_name_their_field() {
        for c in AuthErrorCode::ALL {
            assert_eq!(AuthErrorCode::from_code(c.code()), Some(c));
        }
        assert_eq!(AuthErrorCode::from_code("invalid"), None);
        assert_eq!(
            UsernameRule::Space.error().field(),
            Some(AuthField::Username)
        );
        assert_eq!(AuthErrorCode::InvalidCredentials.field(), None);
    }

    #[test]
    fn availability_is_tagged_json() {
        let v = serde_json::to_value(UsernameAvailability::Invalid {
            username: "a b".into(),
            rule: UsernameRule::Space,
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({"status": "invalid", "username": "a b", "rule": "space"})
        );
    }
}
