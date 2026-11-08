//! One webhook call as the sender's card remembers it (design.md 6.9, R-38): the body is read
//! after the token is known, so a refused body is attributed to its integration with the
//! offending field and the body itself, secrets redacted and cut to [`MAX_PAYLOAD`] bytes.

use serde_json::Value;

use crate::{IntegrationError, WatchedEvent};

/// Most bytes of a remembered body.
pub const MAX_PAYLOAD: usize = 4096;

const HIDDEN: &str = "********";

/// Keys whose values are never kept: any key that contains one of these, in any case
/// (`X-Api-Key`, `api_token`, `clientSecret`, `sessionId`, `set_cookie`, ...; UX3-4). Broad on
/// purpose: hiding an innocent `passage` costs less than keeping a password.
const SECRET_PARTS: [&str; 12] = [
    "token",
    "secret",
    "pass",
    "pwd",
    "apikey",
    "api_key",
    "api-key",
    "authorization",
    "bearer",
    "credential",
    "session",
    "cookie",
];

fn is_secret_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    k == "auth" || k == "key" || SECRET_PARTS.iter().any(|p| k.contains(p))
}

/// A JSON web token: three base64url parts, the first a JSON object (`eyJ`).
fn has_jwt(s: &str) -> bool {
    s.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
        .any(|w| {
            w.starts_with("eyJ") && w.split('.').count() == 3 && w.split('.').all(|p| !p.is_empty())
        })
}

/// A value that looks like a credential whatever its key: an integration or signing token
/// anywhere in it, an `Authorization` header value, a JWT (UX3-4).
fn is_secret_value(s: &str) -> bool {
    let lower = s.trim_start().to_ascii_lowercase();
    s.contains(crate::TOKEN_PREFIX)
        || s.contains("whsec_")
        || lower.starts_with("bearer ")
        || lower.starts_with("basic ")
        || has_jwt(s)
}

/// `v` with the values of secret-looking keys and secret-looking strings replaced, at any
/// depth.
#[must_use]
pub fn redact(v: &Value) -> Value {
    match v {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| {
                    let v = if is_secret_key(k) {
                        Value::String(HIDDEN.into())
                    } else {
                        redact(v)
                    };
                    (k.clone(), v)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(redact).collect()),
        Value::String(s) if is_secret_value(s) => Value::String(HIDDEN.into()),
        other => other.clone(),
    }
}

/// The redacted body as compact JSON, at most [`MAX_PAYLOAD`] bytes (a longer one is cut at a
/// character boundary and ends in `...`).
#[must_use]
pub fn remembered(v: &Value) -> String {
    let text = redact(v).to_string();
    if text.len() <= MAX_PAYLOAD {
        return text;
    }
    let mut end = MAX_PAYLOAD - 3;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

/// The first name between backticks in a serde message (unknown field, then the name).
fn quoted(message: &str) -> Option<&str> {
    let (_, rest) = message.split_once('`')?;
    rest.split_once('`').map(|(name, _)| name)
}

/// The field a deserialisation error is about: `series.ids`, `season`, `episode_title`.
#[must_use]
pub fn field_of(e: &serde_path_to_error::Error<serde_json::Error>) -> Option<String> {
    let path = e.path().to_string();
    let path = (path != ".").then_some(path);
    let message = e.inner().to_string();
    // a missing field is reported at its parent; an unknown one already at itself
    let named = (message.starts_with("missing field") || message.starts_with("unknown field"))
        .then(|| quoted(&message))
        .flatten();
    match (path, named) {
        (Some(p), Some(n)) if p == n || p.ends_with(&format!(".{n}")) => Some(p),
        (Some(p), Some(n)) => Some(format!("{p}.{n}")),
        (None, Some(n)) => Some(n.to_string()),
        (p, None) => p,
    }
}

/// A body refused before it was understood.
fn refused(field: Option<String>, message: String) -> IntegrationError {
    IntegrationError::Refused { field, message }
}

/// Read a webhook body: the event, and what to remember of the body (`None` when it is not
/// JSON: a body that cannot be redacted is not kept).
///
/// # Errors
/// [`IntegrationError::Refused`] for a body that is not JSON or not a [`WatchedEvent`].
pub fn parse(body: &[u8]) -> (Result<WatchedEvent, IntegrationError>, Option<String>) {
    let value: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => {
            return (
                Err(refused(None, format!("the body is not JSON: {e}"))),
                None,
            );
        }
    };
    let payload = Some(remembered(&value));
    // from the bytes, not the value: the first problem in the order the sender wrote it
    let mut de = serde_json::Deserializer::from_slice(body);
    let event = serde_path_to_error::deserialize::<_, WatchedEvent>(&mut de)
        .map_err(|e| refused(field_of(&e), e.inner().to_string()));
    (event, payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(body: &Value) -> Option<String> {
        match parse(body.to_string().as_bytes()).0 {
            Err(IntegrationError::Refused { field, .. }) => field,
            other => panic!("refused expected: {other:?}"),
        }
    }

    #[test]
    fn the_offending_field_is_named() {
        let ok = serde_json::json!({"kind": "movie", "title": "Heat", "ids": {"tmdb": "949"}});
        assert!(parse(ok.to_string().as_bytes()).0.is_ok());
        assert_eq!(
            field(&serde_json::json!({"kind": "movie", "title": "Heat", "tmdb": "949"})),
            Some("tmdb".into())
        );
        assert_eq!(
            field(&serde_json::json!({"kind": "episode", "title": "x",
                "series": {"title": "S", "ids": {}, "episode_title": "y"}})),
            Some("series.episode_title".into())
        );
        assert_eq!(
            field(&serde_json::json!({"title": "Heat"})),
            Some("kind".into())
        );
        assert_eq!(
            field(&serde_json::json!({"kind": "movie", "title": "Heat", "season": "two"})),
            Some("season".into())
        );
        assert_eq!(
            field(&serde_json::json!({"kind": "concert", "title": "Heat"})),
            Some("kind".into())
        );
        let (r, payload) = parse(b"not json");
        assert!(matches!(
            r,
            Err(IntegrationError::Refused { field: None, .. })
        ));
        assert_eq!(payload, None);
    }

    #[test]
    fn secrets_are_redacted_at_any_depth() {
        let body = serde_json::json!({
            "title": "Heat", "api_token": "t1", "Password": "p1",
            "nested": [{"clientSecret": "s1", "auth": {"user": "u"}, "fine": "f"}],
        });
        let kept = remembered(&body);
        for secret in ["t1", "p1", "s1", "\"u\""] {
            assert!(!kept.contains(secret), "{kept}");
        }
        assert!(kept.contains("\"fine\":\"f\""), "{kept}");
        assert!(kept.contains("\"title\":\"Heat\""), "{kept}");
    }

    /// UX3-4: every common secret spelling of a key (case-insensitive substring), and
    /// token-shaped values under innocent keys.
    #[test]
    fn secret_spellings_and_token_shaped_values_are_redacted() {
        let keys = [
            "X-Api-Key",
            "api_key",
            "APIKEY",
            "api-key",
            "Authorization",
            "bearer",
            "pass",
            "passwd",
            "pwd",
            "password",
            "client_secret",
            "access_token",
            "session",
            "sessionId",
            "Cookie",
            "set_cookie",
            "auth",
            "key",
            "credentials",
        ];
        let mut body = serde_json::Map::new();
        for (i, k) in keys.iter().enumerate() {
            body.insert((*k).to_string(), Value::String(format!("v{i}-plain")));
        }
        let jwt = "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJ4In0.c2lnbmF0dXJl";
        for (k, v) in [
            ("extra", "itk_0123456789abcdef"),
            ("note", "whsec_abcdef0123"),
            ("url", "Bearer abc.def"),
            ("hint", jwt),
            ("link", "https://x.example/cb?token=itk_99887766"),
        ] {
            body.insert(k.into(), Value::String(v.into()));
        }
        body.insert("title".into(), Value::String("Heat".into()));
        body.insert("year".into(), Value::from(1995));
        let kept = remembered(&Value::Object(body));
        for (i, key) in keys.iter().enumerate() {
            assert!(!kept.contains(&format!("v{i}-plain")), "{key} kept: {kept}");
        }
        for secret in ["itk_", "whsec_", "Bearer", "eyJ"] {
            assert!(!kept.contains(secret), "{secret}: {kept}");
        }
        assert!(kept.contains("\"title\":\"Heat\""), "{kept}");
        assert!(kept.contains("\"year\":1995"), "{kept}");
    }

    #[test]
    fn long_bodies_are_cut() {
        let long = serde_json::json!({"title": "\u{e9}".repeat(5000)});
        let kept = remembered(&long);
        assert!(kept.len() <= MAX_PAYLOAD, "{}", kept.len());
        assert!(kept.ends_with("..."));
    }
}
