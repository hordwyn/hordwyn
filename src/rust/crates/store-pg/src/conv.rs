//! Conversions between domain values and column representations.

use app::ports::RepoError;
use domain::catalog::EntityState;
use domain::companion::RequestState;
use domain::hlc::Hlc;
use domain::session::CoopOutcome;
use domain::user::Role;

pub(crate) fn corrupt(what: &str, v: &str) -> RepoError {
    RepoError::Backend(format!("unexpected {what} value `{v}` in database"))
}

pub(crate) fn hlc(s: &str) -> Result<Hlc, RepoError> {
    s.parse().map_err(|_| corrupt("hlc", s))
}

pub(crate) fn json<T: serde::de::DeserializeOwned>(
    what: &str,
    v: serde_json::Value,
) -> Result<T, RepoError> {
    serde_json::from_value(v).map_err(|e| RepoError::Backend(format!("bad {what} json: {e}")))
}

pub(crate) fn to_json<T: serde::Serialize>(v: &T) -> Result<serde_json::Value, RepoError> {
    serde_json::to_value(v).map_err(|e| RepoError::Backend(e.to_string()))
}

pub(crate) const fn role_str(r: Role) -> &'static str {
    match r {
        Role::Admin => "admin",
        Role::Member => "member",
    }
}

pub(crate) fn role(s: &str) -> Result<Role, RepoError> {
    match s {
        "admin" => Ok(Role::Admin),
        "member" => Ok(Role::Member),
        _ => Err(corrupt("role", s)),
    }
}

pub(crate) const fn request_state_str(s: RequestState) -> &'static str {
    match s {
        RequestState::Pending => "pending",
        RequestState::Accepted => "accepted",
        RequestState::Declined => "declined",
        RequestState::Cancelled => "cancelled",
    }
}

pub(crate) fn request_state(s: &str) -> Result<RequestState, RepoError> {
    match s {
        "pending" => Ok(RequestState::Pending),
        "accepted" => Ok(RequestState::Accepted),
        "declined" => Ok(RequestState::Declined),
        "cancelled" => Ok(RequestState::Cancelled),
        _ => Err(corrupt("request state", s)),
    }
}

pub(crate) const fn entity_state_str(s: EntityState) -> &'static str {
    match s {
        EntityState::Stub => "stub",
        EntityState::Partial => "partial",
        EntityState::Full => "full",
        EntityState::Failed => "failed",
    }
}

pub(crate) fn entity_state(s: &str) -> Result<EntityState, RepoError> {
    match s {
        "stub" => Ok(EntityState::Stub),
        "partial" => Ok(EntityState::Partial),
        "full" => Ok(EntityState::Full),
        "failed" => Ok(EntityState::Failed),
        _ => Err(corrupt("entity state", s)),
    }
}

pub(crate) const fn coop_str(c: CoopOutcome) -> &'static str {
    c.as_str()
}

pub(crate) fn coop(s: &str) -> Result<CoopOutcome, RepoError> {
    CoopOutcome::parse(s).ok_or_else(|| corrupt("coop outcome", s))
}

/// Postgres integer columns are signed; domain counts are unsigned.
pub(crate) fn to_i32(v: Option<u32>) -> Result<Option<i32>, RepoError> {
    v.map(|x| i32::try_from(x).map_err(|_| RepoError::Backend(format!("{x} out of range"))))
        .transpose()
}

pub(crate) fn to_u32(v: Option<i32>) -> Result<Option<u32>, RepoError> {
    v.map(|x| u32::try_from(x).map_err(|_| corrupt("count", &x.to_string())))
        .transpose()
}
