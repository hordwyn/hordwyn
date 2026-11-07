//! DTOs of UX phase 2: data-source status and credentials (design.md 4), instance settings
//! with provenance, the admin area (addendum-1 3.4), the audit log, re-authentication
//! (DEC-191) and the setup wizard (design.md 5.1).
//!
//! Error codes the pages translate (`admin-error-<code>` in the web app): `reauth_required`,
//! `last_admin`, `self_role_change`, `self_disable`, `self_delete`,
//! `secure_cookies_need_https`, `fixed_by_config`, `invalid_setting`, `unknown_setting`,
//! `unknown_source`, `missing_field`, `unknown_field`, `setup_done`, `forbidden`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

// ---------------------------------------------------------------- sources (design.md 4.4)

/// One data source as the palette footer, the no-results card and the admin table show it.
///
/// `state` (admins): `working`, `not_set_up`, `key_rejected`, `key_unavailable`,
/// `rate_limited`, `circuit_open`, `disabled`. Members see `working`, `not_set_up`,
/// `needs_account`, `unavailable`, `slow`, `retrying`; disabled sources are hidden from them
/// and the detail fields are empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SourceStatusDto {
    pub id: String,
    pub name: String,
    pub kinds: Vec<String>,
    /// Kind groups served (`watch`, `read`, `listen`, `play`, ...).
    pub groups: Vec<String>,
    pub state: String,
    /// `missing`, `env`, `db`, `invalid`, `locked`, `not_needed` (admins only).
    #[serde(default)]
    pub credential: Option<String>,
    /// The variable that sets the key when it comes from the environment (admins only).
    #[serde(default)]
    pub env_var: Option<String>,
    #[serde(default)]
    pub calls_24h: u64,
    #[serde(default)]
    pub rate_limited_24h: u64,
    /// RFC 3339.
    #[serde(default)]
    pub last_ok_at: Option<String>,
    #[serde(default)]
    pub last_error_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    /// 401 or 403 when the key was rejected.
    #[serde(default)]
    pub last_auth_status: Option<u16>,
    /// When an open circuit lets the next request through (RFC 3339).
    #[serde(default)]
    pub retry_at: Option<String>,
    /// The source asks each member for their own credential (`TheTVDB` PIN).
    #[serde(default)]
    pub user_fields: Vec<String>,
    /// The caller has stored their own credential for it.
    #[serde(default)]
    pub user_credential_set: bool,
}

/// What `source_status` answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SourceStatusPageDto {
    pub sources: Vec<SourceStatusDto>,
    /// Kind groups with no working source at all (the admin nudge, "Board games: no source").
    pub groups_without_source: Vec<String>,
    /// The caller is an admin (shows "Set up" instead of "Ask admin").
    pub is_admin: bool,
    /// Admins only: the setup wizard was left before the end (the Home nudge, design.md 6.1).
    #[serde(default)]
    pub setup_open: bool,
}

/// A source a search skipped, and why (the reason line of design.md 4.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SkippedSourceDto {
    pub id: String,
    pub name: String,
    /// The same vocabulary as [`SourceStatusDto::state`] for the caller's role.
    pub state: String,
}

/// "Ask admin": a member asks for a source or a whole kind group (event
/// `admin.source_requested`, deduplicated per source per day).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct AskAdminDto {
    /// A connector id ...
    #[serde(default)]
    pub source: Option<String>,
    /// ... or a kind group without any source.
    #[serde(default)]
    pub group: Option<String>,
}

/// A stored credential as the UI shows it: never the value (addendum-1 2.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct CredentialMetaDto {
    /// `env`, `db`, `missing`, `locked`.
    pub source: String,
    pub env_var: Option<String>,
    /// RFC 3339.
    pub set_at: Option<String>,
    pub set_by: Option<String>,
    pub last_tested_at: Option<String>,
    /// `ok` or an error code (`key_rejected`, `unreachable`, `rate_limited`, ...).
    pub last_result: Option<String>,
    /// A stored value is shadowed by the environment.
    pub shadowed: bool,
}

/// Admin > Data sources: status, credential, licence record and the fields to ask for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct AdminSourceDto {
    pub status: SourceStatusDto,
    pub credential: CredentialMetaDto,
    /// Field names of the instance credential (`api_key`, `client_id`, ...); labels are
    /// `admin-source-field-<name>`.
    pub fields: Vec<String>,
    pub attribution: String,
    pub terms_url: String,
    /// Hosted mode: agreement on file (design.md 4.1); `None` when never recorded.
    pub agreement_on_file: Option<bool>,
    /// Hosted mode and nothing recorded yet: the page shows a warning.
    pub licence_warning: bool,
}

/// Replace a credential: every field of the spec; absent or `null` means "keep" only for
/// the whole credential (the request replaces it as one value).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct CredentialInputDto {
    pub fields: BTreeMap<String, String>,
}

/// Body of Test: test the given fields, or the stored credential when `fields` is absent.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SourceTestInputDto {
    #[serde(default)]
    pub fields: Option<BTreeMap<String, String>>,
}

/// Result of Test (design.md 5.1: time plus one sample result, specific errors).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SourceTestDto {
    pub ok: bool,
    /// `ok`, `key_rejected`, `unreachable`, `rate_limited`, `circuit_open`, `missing_key`,
    /// `failed`.
    pub result: String,
    pub elapsed_ms: u64,
    pub sample_title: Option<String>,
    pub http_status: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct LicenceInputDto {
    pub agreement_on_file: bool,
}

/// Settings > Data sources for members: their own credentials (`TheTVDB` PIN).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct UserCredentialDto {
    pub source: String,
    pub name: String,
    pub fields: Vec<String>,
    pub set: bool,
    /// The stored value cannot be decrypted right now.
    pub locked: bool,
    pub set_at: Option<String>,
}

// ---------------------------------------------------------------- settings (addendum-1 3.2)

/// One instance setting with its provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SettingDto {
    /// `instance.hosting`, `auth.signup`, `auth.session_days`, `auth.secure_cookies`,
    /// `storage.max_total_bytes`, `storage.high_watermark`, `storage.low_watermark`.
    pub key: String,
    /// The effective value.
    pub value: serde_json::Value,
    /// `env`, `file`, `db`, `default`.
    pub source: String,
    /// The environment variable that sets it (`source = env`), shown as "set by env VAR".
    pub env_var: Option<String>,
    /// The variable that would override it (always named, for the help text).
    pub env_name: String,
    /// `false` when set by env or file (read-only in the UI).
    pub editable: bool,
    /// A database value ignored while env or file sets the key.
    pub shadowed_value: Option<serde_json::Value>,
    /// Changing it needs a fresh proof (DEC-191).
    pub needs_reauth: bool,
    /// The change applies from the next session (cookie security, session length).
    pub applies_next_session: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct InstanceSettingsDto {
    pub settings: Vec<SettingDto>,
    /// `public_url_not_https` (cookie security), `rotation_due`, `licence_missing`.
    pub warnings: Vec<String>,
    /// `server.public_url` (env/file, read-only).
    pub public_url: String,
}

/// `PUT /admin/settings`: keys to change; `null` resets a key to its default.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SettingsPatchDto {
    pub values: BTreeMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------- admin pages

/// One row of Admin > Overview: status dot + word + one line + one action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct HealthRowDto {
    /// `database`, `search`, `storage`, `jobs`, `mail`, `data_sources`,
    /// `identity_provider`, `secrets`.
    pub id: String,
    /// `ok`, `warning`, `error`, `off`.
    pub status: String,
    /// Message key `admin-health-<id>-<detail>` plus arguments.
    pub detail: String,
    pub args: BTreeMap<String, String>,
}

/// The read-only brand card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct BrandCardDto {
    pub name: String,
    pub short_name: String,
    pub default_locale: String,
    /// Where the brand is configured (`APP__BRAND__DIR`).
    pub dir: String,

    /// The accent pair of the light and the dark theme with its contrast (design.md 6.13).
    #[serde(default)]
    pub swatches: Vec<BrandSwatchDto>,
}

/// One theme's accent: `primary` behind `primary-foreground` text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct BrandSwatchDto {
    /// `light` or `dark`.
    pub theme: String,
    pub primary: String,
    pub foreground: String,
    /// WCAG contrast ratio; `None` when a value is not a colour the server can read.
    pub ratio: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct AdminOverviewDto {
    pub health: Vec<HealthRowDto>,
    /// Health row ids that need attention (the attention block).
    pub attention: Vec<String>,
    pub recent: Vec<AuditEntryDto>,
    pub brand: BrandCardDto,
    pub hosting: String,
    /// Shown in hosted mode until every source has a licence record.
    pub hosted_banner: bool,
}

/// Admin > Users.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct AdminUserDto {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub role: String,
    pub disabled: bool,
    /// `password`, `totp`, `oidc`.
    pub signs_in_with: Vec<String>,
    pub last_seen_at: Option<String>,
    pub created_at: String,
    pub is_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct RoleInputDto {
    /// `admin` or `member`.
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct DisabledInputDto {
    pub disabled: bool,
}

/// An admin invite (sign-up without a companion).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct AdminInviteDto {
    pub id: String,
    pub email: Option<String>,
    pub note: Option<String>,
    pub created_by: Option<String>,
    pub created_at: String,
    pub expires_at: String,
    /// `open`, `used`, `revoked`, `expired`.
    pub state: String,
    pub used_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct NewAdminInviteDto {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

/// The new invite with its link, shown once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct AdminInviteCreatedDto {
    pub invite: AdminInviteDto,
    pub token: String,
    /// `<public_url>/sign-up?invite=<token>`.
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct LaneCountsDto {
    /// `interactive`, `user_import`, `background_refresh`, `prefetch`.
    pub lane: String,
    pub queued: u64,
    pub running: u64,
    pub dead: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct FailedJobDto {
    pub id: String,
    pub lane: String,
    pub kind: String,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub failed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct CronDto {
    pub name: String,
    pub schedule: String,
    pub kind: String,
    pub next_run_at: String,
}

/// Admin > Storage and jobs (OD-14: one page).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct StorageJobsDto {
    /// `fs`, `s3`, `memory`.
    pub backend: String,
    pub used_bytes: u64,
    pub pinned_bytes: u64,
    pub cache_bytes: u64,
    pub max_total_bytes: u64,
    pub effective_cap: u64,
    pub high_watermark_bytes: u64,
    pub low_watermark_bytes: u64,
    pub at_hard_cap: bool,
    /// Bytes eviction would free now.
    pub evictable_bytes: u64,
    /// The editable budget numbers with provenance (OD-10).
    pub settings: Vec<SettingDto>,
    pub lanes: Vec<LaneCountsDto>,
    pub failed: Vec<FailedJobDto>,
    pub cron: Vec<CronDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct EvictionDto {
    pub objects: u64,
    pub bytes: u64,
    pub before: u64,
    pub after: u64,
}

/// One key version of the secrets key ring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct KeyVersionDto {
    pub version: u16,
    pub active: bool,
    pub loaded: bool,
    /// Encrypted values on this version.
    pub rows: u64,
    pub first_seen_at: Option<String>,
    /// Loaded, not active and no value uses it: the line can go (the app never edits it).
    pub removable: bool,
}

/// Secrets at rest (addendum-1 2.2, Admin > Instance > Secrets).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SecretsStatusDto {
    /// `file`, `env`, `generated`, `ephemeral`.
    pub key_source: String,
    pub key_file: String,
    pub active_version: u16,
    pub versions: Vec<KeyVersionDto>,
    /// Values on versions that are not loaded.
    pub locked_rows: u64,
    /// Values on an older, loaded version (Re-encrypt now).
    pub rows_to_rewrap: u64,
    /// OD-09: the active version is older than two years.
    pub rotation_due: bool,
}

/// Admin > Security and sessions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SecurityDto {
    /// `auth.signup`, `auth.session_days`, `auth.secure_cookies` with provenance.
    pub settings: Vec<SettingDto>,
    pub warnings: Vec<String>,
    pub secrets: SecretsStatusDto,
    /// Re-authentication window (DEC-191), fixed.
    pub reauth_slide_minutes: i64,
    pub reauth_cap_minutes: i64,
}

/// Re-encryption run (job queued, counts so far).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct RewrapDto {
    pub rewrapped: u64,
    pub locked: u64,
}

/// Instance defaults of the notification rules (Admin > Notification defaults).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct NotificationDefaultsDto {
    pub defaults: Vec<crate::SubscriptionDto>,
    pub events: Vec<crate::EventTypeDto>,
}

// ---------------------------------------------------------------- audit

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct AuditEntryDto {
    pub id: String,
    pub at: String,
    /// `user`, `system`, `cli`.
    pub actor_kind: String,
    pub actor_id: Option<String>,
    pub actor_name: Option<String>,
    /// `setting.changed`, `secret.replaced`, `secret.cleared`, `secrets.rewrapped`,
    /// `role.changed`, `user.disabled`, `user.enabled`, `user.deleted`, `invite.created`,
    /// `invite.revoked`, `licence.recorded`, `wizard.completed`, `eviction.run`,
    /// `job.retried`, `job.discarded`.
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    /// Old and new values (secrets never), proof method of gated actions.
    pub detail: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct AuditPageDto {
    pub entries: Vec<AuditEntryDto>,
    /// Pass as `before` for the next page.
    pub next_before: Option<String>,
}

/// Filters of the audit log.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct AuditQueryDto {
    #[serde(default)]
    pub actor: Option<String>,
    /// Exact action or a prefix ending in `.`.
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub target_type: Option<String>,
    /// RFC 3339.
    #[serde(default)]
    pub before: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
    /// 1 to 500, default 50.
    #[serde(default)]
    pub limit: Option<u32>,
}

// ---------------------------------------------------------------- re-authentication

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ReauthInputDto {
    pub password: String,
    /// TOTP code or a recovery code, when the account has an authenticator.
    #[serde(default)]
    pub totp: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ReauthStatusDto {
    pub fresh: bool,
    /// RFC 3339.
    pub until: Option<String>,
    /// `password`, `password+totp`, `oidc:<provider>`.
    pub method: Option<String>,
    pub has_password: bool,
    pub has_totp: bool,
    /// Proof methods the dialog offers: `password` (then TOTP when `has_totp`); `oidc`
    /// arrives with phase 3.
    pub methods: Vec<String>,
    /// Whole minutes until the window closes (server clock), for "confirmed for 12 more
    /// minutes".
    #[serde(default)]
    pub minutes_left: Option<i64>,
}

/// TOTP enrolment started: the secret to add to an authenticator app (confirm with a code).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct TotpEnrolmentDto {
    pub otpauth_url: String,
    pub secret_base32: String,
}

/// TOTP confirmed: the recovery codes, shown once (OD-08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct RecoveryCodesDto {
    pub codes: Vec<String>,
}

// ---------------------------------------------------------------- setup wizard

/// One data-source card of wizard step 3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SetupSourceDto {
    pub id: String,
    pub name: String,
    pub kinds: Vec<String>,
    pub fields: Vec<String>,
    /// `env` (shown as set, read-only), `db`, `invalid` (a key typed here was rejected),
    /// `missing`, `locked`, `not_needed`.
    pub credential: String,
    pub env_var: Option<String>,
    /// Hosted mode asks for "Agreement on file".
    pub agreement_on_file: Option<bool>,
}

/// The wizard state machine (design.md 5.1 and 4.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SetupStateDto {
    /// `account` (no user yet), `hosting`, `sources`, `done`.
    pub step: String,
    /// Steps of this mode in order (headless installs see `sources` read-only).
    pub steps: Vec<String>,
    pub hosting: String,
    /// The hosting mode is set by env or file (read-only card).
    pub hosting_fixed: bool,
    pub sources: Vec<SetupSourceDto>,
    /// Every source key comes from the environment: step 3 is a read-only summary.
    pub sources_read_only: bool,
    /// The checklist of the Done step: ids `first_item`, `invite`, `media_server`,
    /// `notification_defaults`, `backup_key`, `identity_provider`.
    pub checklist: Vec<String>,
    /// The secrets key file to back up (OD-06), when it was generated.
    pub key_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct HostingInputDto {
    /// `home` or `hosted`.
    pub hosting: String,
}
