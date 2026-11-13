//! Form discipline for sign-up and sign-in (design.md 5.2, 2.4): the rules a field has are
//! shown before typing, checked live, and every server error code lands at its field (or,
//! for codes about the whole form, in one alert). This module holds the logic as plain
//! functions over [`I18n`] so it is tested without a browser; the pages are in
//! `auth_pages`.

use contracts::auth::{
    AuthErrorCode, AuthField, DISPLAY_NAME_MAX_LEN, PASSWORD_MAX_LEN, PASSWORD_MIN_LEN,
    USERNAME_MAX_LEN, USERNAME_MIN_LEN, UsernameRule, broken_username_rules,
    normalize_display_name, normalize_username,
};
use fluent_bundle::FluentValue;
use ui::RuleState;

use crate::i18n::I18n;

/// Element id of the control for `field` (labels, error summary links and focus use it).
#[must_use]
pub(crate) const fn field_id(field: AuthField) -> &'static str {
    match field {
        AuthField::Username => "username",
        AuthField::DisplayName => "display_name",
        AuthField::Password => "password",
        AuthField::Totp => "totp",
    }
}

/// Where the message for a server error code goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Placement {
    /// Inline at the field and in the error summary.
    Field(AuthField),
    /// One alert for the whole form (closed sign-up, wrong credentials, throttle, unknown).
    Form,
    /// Not an error: sign-in continues with the one-time code step.
    TotpStep,
}

#[must_use]
pub(crate) fn placement(code: &str) -> Placement {
    match AuthErrorCode::from_code(code) {
        Some(AuthErrorCode::TotpRequired) => Placement::TotpStep,
        Some(c) => c.field().map_or(Placement::Form, Placement::Field),
        None => Placement::Form,
    }
}

/// The message for a form-level code (`auth-error-<code>`, or the generic one).
#[must_use]
pub(crate) fn form_message(i: &I18n, code: &str) -> String {
    let key = format!("auth-error-{code}");
    let text = i.t(&key);
    if text == key {
        i.t("error-generic")
    } else {
        text
    }
}

/// [`form_message`], with the time left of a wait after failed attempts (`retry_after_secs`,
/// SEC-5): in seconds below a minute, else in minutes.
#[must_use]
pub(crate) fn form_message_after(i: &I18n, code: &str, retry_after_secs: Option<u64>) -> String {
    match retry_after_secs {
        Some(secs)
            if secs < 60
                && AuthErrorCode::from_code(code) == Some(AuthErrorCode::TooManyAttempts) =>
        {
            i.t_args(
                "auth-wait-seconds",
                &[("seconds", FluentValue::from(secs.max(1)))],
            )
        }
        Some(secs) if AuthErrorCode::from_code(code) == Some(AuthErrorCode::TooManyAttempts) => {
            let minutes = secs.div_ceil(60).max(1);
            i.t_args(
                "auth-lockout-minutes",
                &[("minutes", FluentValue::from(minutes))],
            )
        }
        _ => form_message(i, code),
    }
}

/// The availability check was throttled: the account can still be created.
#[must_use]
pub(crate) fn check_throttled_message(i: &I18n) -> String {
    i.t("auth-check-throttled")
}

fn n(v: usize) -> FluentValue<'static> {
    FluentValue::from(v)
}

fn s(v: &str) -> FluentValue<'static> {
    FluentValue::from(v.to_string())
}

fn limits() -> [(&'static str, FluentValue<'static>); 2] {
    [("min", n(USERNAME_MIN_LEN)), ("max", n(USERNAME_MAX_LEN))]
}

/// Hint under the username label, shown before typing.
#[must_use]
pub(crate) fn username_hint(i: &I18n) -> String {
    i.t_args("auth-username-hint", &limits())
}

/// The live checklist under the username: length, no spaces, allowed characters. All rules
/// are neutral while the field is empty.
#[must_use]
pub(crate) fn username_rules(i: &I18n, raw: &str) -> Vec<(String, RuleState)> {
    let normalized = normalize_username(raw);
    let broken = broken_username_rules(&normalized);
    let state = |rules: &[UsernameRule]| {
        if normalized.is_empty() {
            RuleState::Pending
        } else if rules.iter().any(|r| broken.contains(r)) {
            RuleState::Fail
        } else {
            RuleState::Pass
        }
    };
    vec![
        (
            i.t_args("auth-username-rule-length", &limits()),
            state(&[UsernameRule::TooShort, UsernameRule::TooLong]),
        ),
        (
            i.t("auth-username-rule-space"),
            state(&[UsernameRule::Space]),
        ),
        (
            i.t("auth-username-rule-chars"),
            state(&[UsernameRule::InvalidChars]),
        ),
    ]
}

/// "Will be saved as robin ..." when normalisation changes what was typed (it is shown,
/// never applied silently).
#[must_use]
pub(crate) fn username_saved_as(i: &I18n, raw: &str) -> Option<String> {
    let normalized = normalize_username(raw);
    if normalized.is_empty() || normalized == raw {
        return None;
    }
    // only spaces at the ends removed, or (also) capitals lowered
    let key = if raw.trim() == normalized {
        "auth-username-saved-as-trimmed"
    } else {
        "auth-username-saved-as"
    };
    Some(i.t_args(key, &[("username", s(&normalized))]))
}

fn invalid_chars(normalized: &str) -> String {
    let mut out: Vec<char> = Vec::new();
    for c in normalized.chars() {
        if UsernameRule::InvalidChars.broken_by(&c.to_string()) && !out.contains(&c) {
            out.push(c);
        }
    }
    out.iter()
        .map(char::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The message for a broken username rule, with a fix where one exists (design.md 2.4).
#[must_use]
pub(crate) fn username_rule_message(i: &I18n, rule: UsernameRule, normalized: &str) -> String {
    let count = normalized.chars().count();
    let value = s(normalized);
    match rule {
        UsernameRule::TooShort => i.t_args(
            "auth-username-too-short",
            &[("value", value), ("min", n(USERNAME_MIN_LEN))],
        ),
        UsernameRule::TooLong => i.t_args(
            "auth-username-too-long",
            &[
                ("value", value),
                ("count", n(count)),
                ("max", n(USERNAME_MAX_LEN)),
            ],
        ),
        UsernameRule::Space => {
            let words: Vec<&str> = normalized.split_whitespace().collect();
            i.t_args(
                "auth-username-space",
                &[
                    ("value", value),
                    ("dash", s(&words.join("-"))),
                    ("underscore", s(&words.join("_"))),
                ],
            )
        }
        UsernameRule::InvalidChars => i.t_args(
            "auth-username-invalid-chars",
            &[("value", value), ("chars", s(&invalid_chars(normalized)))],
        ),
    }
}

/// "'anna' is already taken. Free: anna2, anna.k."
#[must_use]
pub(crate) fn username_taken_message(i: &I18n, username: &str, suggestions: &[String]) -> String {
    i.t_args(
        "auth-username-taken",
        &[
            ("username", s(username)),
            ("count", n(suggestions.len())),
            ("suggestions", s(&suggestions.join(", "))),
        ],
    )
}

/// The inline error of the username field: required, the first broken rule, or taken
/// (`taken` holds the suggestions when the server said the current name is taken).
#[must_use]
pub(crate) fn username_error(i: &I18n, raw: &str, taken: Option<&[String]>) -> Option<String> {
    let normalized = normalize_username(raw);
    if normalized.is_empty() {
        return Some(i.t("auth-username-required"));
    }
    if let Some(rule) = broken_username_rules(&normalized).first() {
        return Some(username_rule_message(i, *rule, &normalized));
    }
    taken.map(|sugg| username_taken_message(i, &normalized, sugg))
}

#[must_use]
pub(crate) fn password_hint(i: &I18n) -> String {
    i.t_args("auth-password-hint", &[("min", n(PASSWORD_MIN_LEN))])
}

/// The live length rule with a character count.
#[must_use]
pub(crate) fn password_rules(i: &I18n, password: &str) -> Vec<(String, RuleState)> {
    let count = password.chars().count();
    let state = match count {
        0 => RuleState::Pending,
        c if (PASSWORD_MIN_LEN..=PASSWORD_MAX_LEN).contains(&c) => RuleState::Pass,
        _ => RuleState::Fail,
    };
    vec![(
        i.t_args(
            "auth-password-rule-length",
            &[("min", n(PASSWORD_MIN_LEN)), ("count", n(count))],
        ),
        state,
    )]
}

/// The inline error of a new password.
#[must_use]
pub(crate) fn password_error(i: &I18n, password: &str) -> Option<String> {
    let count = password.chars().count();
    if count == 0 {
        Some(i.t("auth-password-required"))
    } else if count < PASSWORD_MIN_LEN {
        Some(i.t_args(
            "auth-password-too-short",
            &[("count", n(count)), ("min", n(PASSWORD_MIN_LEN))],
        ))
    } else if count > PASSWORD_MAX_LEN {
        Some(i.t_args(
            "auth-password-too-long",
            &[("count", n(count)), ("max", n(PASSWORD_MAX_LEN))],
        ))
    } else {
        None
    }
}

/// The inline error of the display name (empty is fine: it becomes the username).
#[must_use]
pub(crate) fn display_name_error(i: &I18n, display: &str, username: &str) -> Option<String> {
    let stored = normalize_display_name(display, &normalize_username(username));
    let count = stored.chars().count();
    (count > DISPLAY_NAME_MAX_LEN).then(|| {
        i.t_args(
            "auth-display-name-too-long",
            &[("count", n(count)), ("max", n(DISPLAY_NAME_MAX_LEN))],
        )
    })
}

/// What the person typed into the sign-up form.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SignUpValues {
    pub username: String,
    pub display_name: String,
    pub password: String,
}

/// Every problem the browser can see before sending, in field order.
#[must_use]
pub(crate) fn sign_up_errors(
    i: &I18n,
    v: &SignUpValues,
    taken: Option<&[String]>,
) -> Vec<(AuthField, String)> {
    [
        (AuthField::Username, username_error(i, &v.username, taken)),
        (
            AuthField::DisplayName,
            display_name_error(i, &v.display_name, &v.username),
        ),
        (AuthField::Password, password_error(i, &v.password)),
    ]
    .into_iter()
    .filter_map(|(f, m)| m.map(|m| (f, m)))
    .collect()
}

/// The message for a field-level server code: the same text the live check would show,
/// so the summary and the inline message agree; `auth-error-<code>` otherwise.
#[must_use]
pub(crate) fn server_field_message(i: &I18n, code: &str, v: &SignUpValues) -> String {
    let normalized = normalize_username(&v.username);
    let local = match AuthErrorCode::from_code(code) {
        Some(AuthErrorCode::UsernameTaken) => Some(username_taken_message(i, &normalized, &[])),
        Some(c) if c.field() == Some(AuthField::Username) => username_error(i, &v.username, None),
        Some(AuthErrorCode::DisplayNameTooLong) => {
            display_name_error(i, &v.display_name, &v.username)
        }
        Some(AuthErrorCode::PasswordTooShort | AuthErrorCode::PasswordTooLong) => {
            password_error(i, &v.password)
        }
        _ => None,
    };
    local.unwrap_or_else(|| form_message(i, code))
}

/// Heading of the error summary: "There are 2 problems".
#[must_use]
pub(crate) fn summary_title(i: &I18n, count: usize) -> String {
    i.t_args("auth-error-summary", &[("count", n(count))])
}

/// Sign-in checks only that both fields are filled (no rules: that would hint at accounts).
#[must_use]
pub(crate) fn sign_in_errors(i: &I18n, username: &str, password: &str) -> Vec<(AuthField, String)> {
    let mut out = Vec::new();
    if username.trim().is_empty() {
        out.push((AuthField::Username, i.t("auth-sign-in-username-required")));
    }
    if password.is_empty() {
        out.push((AuthField::Password, i.t("auth-sign-in-password-required")));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn en() -> I18n {
        I18n::new("en", BTreeMap::new())
    }

    fn de() -> I18n {
        I18n::new("de", BTreeMap::new())
    }

    fn states(rules: &[(String, RuleState)]) -> Vec<RuleState> {
        rules.iter().map(|(_, s)| *s).collect()
    }

    #[test]
    fn every_server_code_has_a_place() {
        use AuthErrorCode as C;
        assert_eq!(
            placement("username_space"),
            Placement::Field(AuthField::Username)
        );
        assert_eq!(
            placement("username_taken"),
            Placement::Field(AuthField::Username)
        );
        assert_eq!(
            placement("display_name_too_long"),
            Placement::Field(AuthField::DisplayName)
        );
        assert_eq!(
            placement("password_too_short"),
            Placement::Field(AuthField::Password)
        );
        assert_eq!(placement("invalid_totp"), Placement::Field(AuthField::Totp));
        assert_eq!(placement("totp_required"), Placement::TotpStep);
        for c in [
            C::InvalidCredentials,
            C::SignupClosed,
            C::InvalidInvite,
            C::TooManyAttempts,
        ] {
            assert_eq!(placement(c.code()), Placement::Form, "{c:?}");
        }
        assert_eq!(placement("internal"), Placement::Form);
        assert_eq!(field_id(AuthField::DisplayName), "display_name");
    }

    #[test]
    fn no_generic_check_the_fields_message() {
        let i = en();
        let v = SignUpValues {
            username: "anna k".into(),
            display_name: String::new(),
            password: "abc123".into(),
        };
        for c in AuthErrorCode::ALL {
            let msg = match placement(c.code()) {
                Placement::Field(_) => server_field_message(&i, c.code(), &v),
                _ => form_message(&i, c.code()),
            };
            assert!(!msg.contains("check the fields"), "{c:?}: {msg}");
            assert!(!msg.starts_with("auth-"), "{c:?} untranslated: {msg}");
        }
        assert_eq!(form_message(&i, "internal"), i.t("error-generic"));
        assert_eq!(i.t("auth-error-invalid"), "auth-error-invalid");
    }

    #[test]
    fn username_checklist_follows_the_rules() {
        let i = en();
        assert_eq!(
            states(&username_rules(&i, "")),
            [RuleState::Pending; 3],
            "neutral before typing"
        );
        assert_eq!(
            states(&username_rules(&i, "Robin")),
            [RuleState::Pass; 3],
            "capitals never fail"
        );
        assert_eq!(
            states(&username_rules(&i, "anna k")),
            [RuleState::Pass, RuleState::Fail, RuleState::Pass]
        );
        assert_eq!(
            states(&username_rules(&i, "me@x")),
            [RuleState::Pass, RuleState::Pass, RuleState::Fail]
        );
        assert_eq!(
            states(&username_rules(&i, "ab")),
            [RuleState::Fail, RuleState::Pass, RuleState::Pass]
        );
        let texts: Vec<String> = username_rules(&i, "").into_iter().map(|r| r.0).collect();
        assert_eq!(
            texts,
            ["3 to 32 characters", "No spaces", "Only a-z, 0-9 and _ . -"]
        );
    }

    #[test]
    fn normalisation_is_shown_not_silent() {
        let i = en();
        assert_eq!(
            username_saved_as(&i, "Robin").as_deref(),
            Some("Will be saved as robin (capitals become lowercase)")
        );
        assert_eq!(
            username_saved_as(&i, " anna ").as_deref(),
            Some("Will be saved as anna (spaces at the start and end are removed)")
        );
        assert_eq!(username_saved_as(&i, "anna"), None);
        assert_eq!(username_saved_as(&i, "  "), None);
        assert_eq!(
            username_saved_as(&de(), "Robin").as_deref(),
            Some("Wird als robin gespeichert (Gro\u{df}buchstaben werden klein)")
        );
    }

    #[test]
    fn username_errors_suggest_a_fix() {
        let i = en();
        assert_eq!(
            username_error(&i, "anna k", None).as_deref(),
            Some("'anna k' has a space. Try 'anna-k' or 'anna_k'.")
        );
        assert_eq!(
            username_error(&de(), "anna k", None).as_deref(),
            Some("'anna k' enth\u{e4}lt ein Leerzeichen. Versuch 'anna-k' oder 'anna_k'.")
        );
        let taken = ["anna2".to_string(), "anna.k".to_string()];
        assert_eq!(
            username_error(&i, "Anna", Some(&taken)).as_deref(),
            Some("'anna' is already taken. Free: anna2, anna.k.")
        );
        assert_eq!(
            username_error(&de(), "anna", Some(&taken)).as_deref(),
            Some("'anna' ist schon vergeben. Frei: anna2, anna.k.")
        );
        assert_eq!(
            username_error(&i, "anna", Some(&[])).as_deref(),
            Some("'anna' is already taken.")
        );
        assert_eq!(
            username_error(&i, "me@x!", None).as_deref(),
            Some(
                "'me@x!' contains characters that are not allowed: @ !. Use only a-z, 0-9 and _ . -"
            )
        );
        assert_eq!(
            username_error(&i, "ab", None).as_deref(),
            Some("'ab' is too short: use at least 3 characters.")
        );
        assert_eq!(
            username_error(&i, "", None),
            Some(i.t("auth-username-required"))
        );
        assert_eq!(username_error(&i, "Robin", None), None);
    }

    #[test]
    fn password_rule_counts_characters() {
        let i = en();
        assert_eq!(states(&password_rules(&i, "")), [RuleState::Pending]);
        assert_eq!(states(&password_rules(&i, "abc123")), [RuleState::Fail]);
        assert_eq!(
            password_rules(&i, "abc123")[0].0,
            "At least 10 characters (6 so far)"
        );
        assert_eq!(
            states(&password_rules(&i, "correct horse battery")),
            [RuleState::Pass]
        );
        assert_eq!(
            password_error(&i, "abc123").as_deref(),
            Some("The password has 6 characters; it needs at least 10.")
        );
        assert_eq!(
            password_error(&de(), "abc123").as_deref(),
            Some("Das Passwort hat 6 Zeichen; es braucht mindestens 10.")
        );
        assert_eq!(password_error(&i, "correct horse battery"), None);
    }

    #[test]
    fn sign_up_errors_come_in_field_order_and_match_the_server_text() {
        let i = en();
        let v = SignUpValues {
            username: "anna k".into(),
            display_name: "x".repeat(101),
            password: "abc123".into(),
        };
        let errs = sign_up_errors(&i, &v, None);
        let fields: Vec<AuthField> = errs.iter().map(|e| e.0).collect();
        assert_eq!(
            fields,
            [
                AuthField::Username,
                AuthField::DisplayName,
                AuthField::Password
            ]
        );
        assert_eq!(errs[0].1, server_field_message(&i, "username_space", &v));
        assert_eq!(
            errs[1].1,
            server_field_message(&i, "display_name_too_long", &v)
        );
        assert_eq!(
            errs[2].1,
            server_field_message(&i, "password_too_short", &v)
        );
        assert_eq!(summary_title(&i, 2), "There are 2 problems");
        assert_eq!(summary_title(&i, 1), "There is one problem");
        assert_eq!(summary_title(&de(), 2), "Es gibt 2 Probleme");
        let ok = SignUpValues {
            username: "Robin".into(),
            display_name: String::new(),
            password: "correct horse battery".into(),
        };
        assert_eq!(sign_up_errors(&i, &ok, None).len(), 0);
    }

    /// UX0-4 (design.md 5.2), SEC-5: the wait names the time left, in seconds below a minute
    /// and in minutes from there; the availability check has its own throttle text (there
    /// were no failed attempts).
    #[test]
    fn a_wait_names_the_time_left() {
        let i = en();
        assert_eq!(
            form_message_after(&i, "too_many_attempts", Some(61)),
            "Too many failed attempts. Try again in 2 minutes."
        );
        assert_eq!(
            form_message_after(&i, "too_many_attempts", Some(30)),
            "Too many failed attempts. Try again in 30 seconds."
        );
        assert_eq!(
            form_message_after(&i, "too_many_attempts", Some(1)),
            "Too many failed attempts. Try again in 1 second."
        );
        assert_eq!(
            form_message_after(&de(), "too_many_attempts", Some(600)),
            "Zu viele Fehlversuche. Versuch es in 10 Minuten noch einmal."
        );
        assert_eq!(
            form_message_after(&de(), "too_many_attempts", Some(60)),
            "Zu viele Fehlversuche. Versuch es in 1 Minute noch einmal."
        );
        assert_eq!(
            form_message_after(&de(), "too_many_attempts", Some(8)),
            "Zu viele Fehlversuche. Versuch es in 8 Sekunden noch einmal."
        );
        assert_eq!(
            form_message_after(&i, "too_many_attempts", None),
            form_message(&i, "too_many_attempts")
        );
        assert_eq!(
            form_message_after(&i, "invalid_credentials", Some(5)),
            "Wrong username or password."
        );
        assert_eq!(
            check_throttled_message(&i),
            "Too many checks right now; you can still create the account."
        );
        assert_ne!(check_throttled_message(&de()), check_throttled_message(&i));
    }

    #[test]
    fn sign_in_only_asks_for_both_fields() {
        let i = en();
        assert_eq!(sign_in_errors(&i, "anna", "x").len(), 0);
        let errs = sign_in_errors(&i, " ", "");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].0, AuthField::Username);
        // one non-enumerating message for wrong credentials
        assert_eq!(
            form_message(&i, "invalid_credentials"),
            "Wrong username or password."
        );
    }
}
