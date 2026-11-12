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
    /// Message key `admin-health-<id>-<detail>` plus arguments. Status `unknown` with
    /// `not_checked`: the probe job has not written this row yet.
    pub detail: String,
    /// An argument `at` (RFC 3339) comes with `at_local` (`YYYY-MM-DDTHH:MM` in the admin's
    /// time zone, DEC-336).
    pub args: BTreeMap<String, String>,
    /// When the `admin.probes` job wrote the row (RFC 3339), and in the admin's zone.
    #[serde(default)]
    pub checked_at: Option<String>,
    #[serde(default)]
    pub checked_local: Option<String>,
    /// Older than three minutes: "not checked recently".
    #[serde(default)]
    pub stale: bool,
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
    /// Set while no admin has confirmed the backup of the active secrets key (OPS-4): the
    /// overview shows the reminder.
    #[serde(default)]
    pub key_backup: Option<KeyBackupDto>,
}

/// The secrets key an admin is asked to back up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct KeyBackupDto {
    /// The active key version.
    pub version: u16,
    /// Path of the key file; `None` when the keys come from the environment.
    pub key_file: Option<String>,
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
    /// The role is managed by this provider (`role_source = provider`): badged "via group"
    /// and not editable here.
    #[serde(default)]
    pub role_via_provider: Option<String>,
    /// The person's linked sign-ins (the row menu's "Unlink provider", DEC-336).
    #[serde(default)]
    pub identities: Vec<LinkedIdentityDto>,
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
    /// The client address (DEC-284: the peer, or the forwarded address from a trusted proxy).
    #[serde(default)]
    pub client_addr: Option<String>,
    #[serde(default)]
    pub request_id: Option<String>,
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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ReauthInputDto {
    pub password: String,
    /// TOTP code or a recovery code, when the account has an authenticator.
    #[serde(default)]
    pub totp: Option<String>,
}

impl std::fmt::Debug for ReauthInputDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReauthInputDto(***)")
    }
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
    /// With `oidc` in `methods`: the provider's display name for "Confirm with <provider>"
    /// and where the round trip starts (add `&next=` with the local path of the gated page).
    #[serde(default)]
    pub oidc_provider: Option<String>,
    #[serde(default)]
    pub oidc_start: Option<String>,
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
    /// The checklist ids the server knows are done (DEC-292).
    #[serde(default)]
    pub done: Vec<String>,
    /// The secrets key file to back up (OD-06), when it was generated.
    pub key_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct HostingInputDto {
    /// `home` or `hosted`.
    pub hosting: String,
}

/// Admin > Instance > Mail (addendum-1 3.2): the `mail.*` settings with provenance, whether a
/// password is stored (never the password) and the last test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct MailSettingsDto {
    pub settings: Vec<SettingDto>,
    /// Host and sender are set.
    pub configured: bool,
    pub password_set: bool,
    /// `db`, `env` (the configuration sets it) or `none`.
    pub password_source: String,
    pub last_test_at: Option<String>,
    /// `ok` or `failed: <reason>`.
    pub last_test_result: Option<String>,
}

/// Replace the SMTP password; `null` removes it. Write-only.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct MailPasswordDto {
    pub password: Option<String>,
}

impl std::fmt::Debug for MailPasswordDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MailPasswordDto")
            .field("password", &self.password.as_ref().map(|_| "***"))
            .finish()
    }
}

/// "Send a test" to `to` (default: the admin's own address).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct MailTestInputDto {
    #[serde(default)]
    pub to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct MailTestDto {
    pub ok: bool,
    /// `ok` or `failed: <reason>`.
    pub result: String,
    pub at: String,
}

/// The Disabled switch of a source (DEC-289).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SourceEnabledDto {
    pub enabled: bool,
}

// ---------------------------------------------------------------- OIDC providers (WP3-2)

/// Admin > Sign-in and SSO: the providers and the exact URIs to register at each provider
/// (addendum-1 4.2). Error codes: `invalid_slug`, `slug_taken`, `insecure_issuer`,
/// `invalid_issuer`, `admin_groups_required`, `self_demotion`, `fixed_by_config`,
/// `last_sign_in_method`, `role_managed_by_provider`, `identity_in_use`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct OidcAdminDto {
    pub providers: Vec<OidcProviderDto>,
    /// `<public_url>/api/v1/auth/oidc/callback`, one for every provider.
    pub redirect_uri: String,
    /// `<public_url>/signed-out` (RP-initiated logout, WP3-3).
    pub post_logout_redirect_uri: String,
    /// `<public_url>/api/v1/auth/oidc/backchannel-logout` (WP3-3).
    pub backchannel_logout_uri: String,
}

/// One provider. The client secret is write-only: only whether one is stored.
// Wire format: independent yes/no fields the admin page shows one by one.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct OidcProviderDto {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub issuer: String,
    pub client_id: String,
    pub secret_set: bool,
    /// The stored secret cannot be decrypted (its key version is not loaded).
    pub secret_locked: bool,
    pub scopes: Vec<String>,
    pub enabled: bool,
    pub is_default: bool,
    pub auto_create: bool,
    pub groups_claim: String,
    pub allowed_groups: Vec<String>,
    pub admin_groups: Vec<String>,
    /// `local` or `provider`.
    pub role_source: String,
    /// `db`, or `env` (from `auth.oidc` in the configuration, read-only).
    pub provenance: String,
    /// `APP__AUTH__OIDC` for the environment provider.
    pub env_var: Option<String>,
    pub capabilities: OidcCapabilitiesDto,
    pub discovery_fetched_at: Option<String>,
    pub last_test_at: Option<String>,
    pub last_test: Option<OidcTestDto>,
    pub last_callback_at: Option<String>,
    /// `ok` or `failed: <code>`.
    pub last_callback_result: Option<String>,
    /// Accounts with an identity linked through this provider (DEC-388).
    #[serde(default)]
    pub linked_accounts: u64,
}

/// What the provider's discovery document advertises.
// Wire format: one flag per advertised capability (Discovery 1.0, the logout specs).
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct OidcCapabilitiesDto {
    pub end_session: bool,
    pub backchannel_logout: bool,
    pub backchannel_logout_session: bool,
    pub frontchannel_logout: bool,
    pub pkce_s256: bool,
    /// RFC 9207 `authorization_response_iss_parameter_supported`.
    pub iss_parameter: bool,
}

/// Create or change a provider. `client_secret` is only read on create; change it with
/// `PUT /admin/oidc/providers/{id}/secret`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct OidcProviderInputDto {
    pub slug: String,
    pub display_name: String,
    pub issuer: String,
    pub client_id: String,
    #[serde(default)]
    pub client_secret: Option<String>,
    /// Default `openid profile email`; `openid` is always added.
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub auto_create: bool,
    #[serde(default)]
    pub groups_claim: Option<String>,
    #[serde(default)]
    pub allowed_groups: Vec<String>,
    #[serde(default)]
    pub admin_groups: Vec<String>,
    /// `local` (default) or `provider` (needs `admin_groups`, OD-16).
    #[serde(default)]
    pub role_source: Option<String>,
}

const fn yes() -> bool {
    true
}

impl std::fmt::Debug for OidcProviderInputDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcProviderInputDto")
            .field("slug", &self.slug)
            .field("issuer", &self.issuer)
            .field("client_id", &self.client_id)
            .field("client_secret", &self.client_secret.as_ref().map(|_| "***"))
            .finish_non_exhaustive()
    }
}

/// Replace the client secret; `null` removes it. Write-only.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct OidcSecretDto {
    pub secret: Option<String>,
}

impl std::fmt::Debug for OidcSecretDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcSecretDto")
            .field("secret", &self.secret.as_ref().map(|_| "***"))
            .finish()
    }
}

/// Outcome of a provider Test: a fresh discovery and what it advertises, and whether the
/// testing admin's last sign-in through it carried the groups claim (`null`: the admin has
/// no identity at this provider).
// Wire format: one flag per Test outcome.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct OidcTestDto {
    pub ok: bool,
    /// `ok` or `failed: <reason>`.
    pub discovery: String,
    pub end_session: bool,
    pub backchannel_logout: bool,
    pub frontchannel_logout: bool,
    pub pkce_s256: bool,
    pub iss_parameter: bool,
    pub groups_claim_seen: Option<bool>,
    pub at: String,
    /// OD-19, from the last silent probe (a `prompt=none` round trip by an admin):
    /// `silent_ok`, `silent_login_required` or `silent_unsupported`; `None`: not probed. A
    /// provider that shows its own page instead of answering never comes back, which also
    /// means `silent_unsupported`. Auto mode goes silent only after `silent_ok`.
    #[serde(default)]
    pub silent_probe: Option<String>,
    #[serde(default)]
    pub silent_probe_at: Option<String>,
    /// od-12 4.4 step 5, from the last `max_age=0` round trip: did the provider report a
    /// fresh `auth_time`? `None`: not probed.
    #[serde(default)]
    pub max_age_probe: Option<bool>,
    #[serde(default)]
    pub max_age_probe_at: Option<String>,
    /// Where the admin's browser starts the probes (top-level navigation; add `&next=` with
    /// a local path to come back to, which then gets `?probe=<outcome>`).
    #[serde(default)]
    pub silent_probe_url: Option<String>,
    #[serde(default)]
    pub max_age_probe_url: Option<String>,
}

/// A provider button on the sign-in page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct OidcButtonDto {
    pub slug: String,
    pub display_name: String,
    pub is_default: bool,
}

/// "Is this your account?" (addendum-1 4.2 step 3): the account the provider's verified
/// email matches, to be confirmed with its password.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct OidcLinkPendingDto {
    pub username: String,
    pub display_name: String,
    /// The provider's display name.
    pub provider: String,
    /// The account has a password (else: "ask your admin to link it").
    pub has_password: bool,
    /// No password, a stored address and mail works: "Email me a link" (DEC-312).
    #[serde(default)]
    pub can_email: bool,
}

/// "Email me a link" was sent (to this address, partly hidden).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct OidcLinkEmailedDto {
    pub sent_to: String,
    /// Minutes the link works.
    pub minutes: i64,
}

/// The code of an emailed link (`/sign-in/link?code=...`).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct OidcLinkCodeDto {
    pub code: String,
}

impl std::fmt::Debug for OidcLinkCodeDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OidcLinkCodeDto(***)")
    }
}

/// The password (and TOTP when enrolled) of the matched account.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct OidcLinkConfirmDto {
    pub password: String,
    #[serde(default)]
    pub totp: Option<String>,
}

impl std::fmt::Debug for OidcLinkConfirmDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OidcLinkConfirmDto(***)")
    }
}

/// Settings > Account > Linked sign-ins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct LinkedIdentityDto {
    pub id: String,
    /// The provider's display name, or the issuer when the provider is gone.
    pub provider: String,
    pub provider_slug: Option<String>,
    pub issuer: String,
    pub linked_at: String,
    /// Unlinking would leave a way to sign in (a password or another provider).
    pub can_unlink: bool,
}
