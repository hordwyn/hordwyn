# OD-12: re-authentication window for sign-in-related admin changes

Research for the open owner question OD-12 (docs/design/ux/addendum-1.md sections 3.3,
3.6 and 5; docs/HANDOFF.md section 4.3). Date of research: 2026-11-06. All facts below
come from public documentation pages read on that date; the URL is given with every
fact. Where a page does not state a value the table says "not stated"; where a page
could not be opened the item is marked UNVERIFIED. No source code of any project was
read (KICKOFF rule 6, R-P1): for AGPL/GPL projects (Nextcloud, Grafana, Forgejo 9+,
Mastodon, Discourse, Bitwarden, Vaultwarden) only their documentation sites, help
centres, wikis or app-store release notes were used, and nothing is quoted from their
UI. Standards (OWASP, NIST, OpenID) are quoted where the exact wording matters.

Placeholder until the owner decides: 10 minutes (DEC-172).

## 1. The question

Before an admin changes how people sign in (sign-in mode, OIDC providers and group
rules, sign-up mode, session length and cookie security) and before a person changes
their own sign-in credentials (password, TOTP, linked provider), we ask for a fresh
proof of identity, like GitHub's "sudo mode". The open points are:

- how long such a proof stays valid (the window);
- whether the window is sliding (each gated action extends it) or fixed (counted from
  the moment of the proof);
- which actions it gates;
- what an admin who has no password (signed in only through an OIDC provider) does.

## 2. Proposal in short

1. **Window: 15 minutes, sliding, with an absolute cap of 1 hour.** Each gated action
   that succeeds inside the window restarts the 15 minutes; one hour after the proof
   the window closes no matter what. Signing in counts as a proof, so an admin who has
   just signed in is not asked twice. The values are constants in `auth` (not an admin
   setting; an env override `APP__AUTH__REAUTH_MINUTES` is cheap to add later).
2. **Gated actions: the sign-in surface only.** Admin: sign-in mode (`page`/`auto`),
   provider add/edit/disable/delete and the group-to-role rules, sign-up mode, session
   length and cookie security, granting or revoking the admin role, re-opening the
   setup wizard. Own account: change password, enrol or remove TOTP, link or unlink an
   OIDC identity, change the e-mail address, revoke all other sessions. Everything else
   in the admin area (storage, mail, data-source keys, notification defaults) is covered
   by the audit log and is not gated.
3. **Proof methods: password (plus TOTP when enrolled) in a dialog, or a round trip to
   the identity provider with `max_age=0` for people without a password.** On return we
   require `auth_time` in the ID token, within the last 5 minutes, for the same `iss`
   and `sub` as the session. We never skip the check for provider-only accounts (GitHub
   and Nextcloud do skip it; see section 3); if the provider does not return a fresh
   `auth_time`, the action is refused with a message that names the provider, and the
   admin can fall back to `/sign-in/local` only if the account has a password.

Reasoning in section 4; the draft owner question in section 6.

## 3. What established platforms and self-hosted apps do

### 3.1 Comparison table

Columns: window (default), sliding or fixed, what it gates, how passkeys/TOTP/SSO are
handled. "n/s" = not stated in the documentation read. Sources follow in 3.2.

| Product | Window | Sliding? | Gates | Passkey / TOTP / SSO handling | Src |
|---|---|---|---|---|---|
| GitHub sudo mode | 2 hours, not configurable | sliding ("any sensitive action ... will reset the timer") | account deletion, e-mail changes, SSH keys, OAuth app authorisation, client secrets, PATs, webhooks, org invitations and roles, 2FA enforcement and org security settings, rulesets, recovery codes | password, passkey, security key, GitHub Mobile, TOTP; SMS excluded; SSO-only accounts (CAS/SAML on GHES, EMU on GHEC) get no sudo prompt at all; "Proof of Presence" (preview, Entra ID) re-authenticates at the IdP | S1-S4 |
| GitLab Admin Mode | 6 hours, "you cannot change this timeout limit" | n/s (active until disabled or 6 h) | the whole Admin area and admin API scope; not Git access | 2FA, OmniAuth and LDAP "are supported"; OIDC step-up for Admin Mode is an experiment (17.11/18.4) where the IdP must return required `acr`/`amr` claims and the step-up session "typically" lasts about 10 minutes (IdP token lifetime) | G1, G7 |
| GitLab per-action checks | per request (current password) | - | change password, disable 2FA, register OTP; WebAuthn registration may skip the password for IdP sign-ins; MR approval setting "require user re-authentication (password or SAML)" | password or SAML re-auth for approvals | G3-G5, G9 |
| Google Account "Verify it's you" | n/s (7-day restriction after new sign-in methods) | n/s | change password, 2SV, data download, saved passwords, Gmail forwarding/filters, ownership and budget changes | signed-in device, recovery phone, security key or Authenticator; password not listed as a method | K1, K4 |
| Google Admin console / Cloud session control | admin console session fixed at 1 hour, "can't be modified"; Cloud re-auth 1 to 24 hours (default n/s) | fixed ("doesn't include how long a user has been inactive") | all admin console use; Cloud console, gcloud, apps with Cloud scopes | re-auth method: "Password or Security key" (admin's choice) | K2, K3 |
| Microsoft Entra ID | sign-in frequency default "rolling window of 90 days"; "every time" option (5-minute clock-skew tolerance, strict mode 10 s); PIM re-auth reuse window 10 minutes | default rolling; PIM window fixed | protected actions (Conditional Access policies, cross-tenant settings, hard delete, named locations); PIM role activation (1 to 24 hours) | protected actions are satisfied by an authentication strength (passwordless or phishing-resistant MFA); PIM "require MFA" may be silently satisfied by an earlier strong sign-in unless "every time" is set | M1-M3 |
| AWS | policy condition `aws:MultiFactorAuthAge` (example 3600 s); IAM user console session 12 hours; Identity Center 8 hours default (15 min to 90 days) | fixed age since the MFA sign-in | whatever the policy names (example: stopping/terminating instances); root "Account settings" edit prompts to re-authenticate (window n/s) | TOTP MFA via STS only; not U2F keys, not federated users | A1-A4 |
| Nextcloud password confirmation | n/s in docs (the often quoted 30 minutes is UNVERIFIED) | n/s | "critical actions" marked with the `#[PasswordConfirmationRequired]` attribute; trusted IP ranges can bypass | OIDC sessions (`user_oidc` 7.0) carry a scope that skips the confirmation; app passwords n/s | N1-N5 |
| Grafana | none; sessions: inactive 7 days (sliding), maximum 30 days (fixed), token rotation 10 minutes | - | self password change needs the old password; no admin re-auth documented | n/s | R1, R2 |
| Gitea / Forgejo | none documented; session 1 day, remember-me 31 days | - | own password change needs the current password; API token creation needs Basic auth with the password (+ OTP header with 2FA) | n/s; "sudo" in their docs means admin impersonation, not re-auth | T1-T4 |
| Keycloak | AIA re-auth: "defaults to five minutes" (configurable per required action; `max_age` can only shorten it); step-up `Max Age` per level (0 = every time) | fixed from `auth_time` | application-initiated actions (update password, configure TOTP, update e-mail) and any step-up level a client requests | supports `max_age`, `prompt=login`, `acr_values`/`claims`; adding or deleting a credential needs the level of that credential | C1, C2 |
| authentik | n/s: no re-authentication option, `max_age` or `prompt=login` found in the docs; session duration default "seconds=0" (browser session) | n/s | user switching can be made to re-run password/MFA stages via a policy expression (example: MFA only within 24 h) | n/s | H1-H4 |
| Okta | app policy "Prompt for authentication": every time / after N time / when no global session; 10-second grace; Admin Console: re-auth "every time" recommended, session max 12 h recommended (max 24 h), idle 15 min recommended (max 2 h) | n/s | per app; the Admin Console; editing own security methods needs two enrolled authenticators | any enrolled factors; separate intervals for password and for other factors | O1-O3 |
| Stripe dashboard | "a short period", length n/s | n/s | "changes to your account" (list n/s); cannot be turned off | passkeys and security keys recommended, then authenticator app, SMS, e-mail link, ID upload | P1 |
| Cloudflare, npm | per action, no window | - | 2FA management, e-mail change (password each time); npm: OTP per publish/settings write ("auth-and-writes" default) | security keys/WebAuthn or TOTP; Cloudflare e-mail change blocked under SSO | F1, F2, Q1, Q2 |
| Bitwarden / Vaultwarden | per action (master password or e-mailed code), window n/s; Vaultwarden `/admin` session 20 minutes (`ADMIN_SESSION_LIFETIME`) | n/s | export, API key, e-mail change, 2FA setup, delete account; per-item master password re-prompt | re-prompt not available without a master password (SSO trusted devices, Key Connector) | B1-B5, V1 |
| django-sudo (BSD) | `SUDO_COOKIE_AGE` 10800 s = 3 hours | n/s | views with `@sudo_required`; "an implementation of GitHub's Sudo Mode for Django" | password | D1, D2 |
| Laravel `password.confirm` (MIT) | "three hours" (`password_timeout`) | n/s | routes behind the middleware | password | L1 |
| sudo_rails (MIT) / Devise (MIT) | sudo_rails `sudo_session_duration` 30 minutes (nil = until reset); Devise: per request (`current_password`), `timeout_in` 30 minutes idle | sudo_rails n/s (has `extend_sudo_session!`); Devise sliding | sudo_rails: controller actions listed in `sudo only:`; Devise: update/destroy with password | password (Devise/Clearance strategies) | Y1-Y4 |
| Apple Stolen Device Protection | 1-hour security delay (not a window) | - | Apple Account password, security settings, biometrics, passcode | biometric, wait, biometric again | X1-X3 |
| Mastodon | password challenge exists (3.0.0 release notes); window n/s in any allowed source | n/s | 2FA settings (per the release notes); more n/s | a 3.0.0 fix mentions the challenge "for non-database users", so accounts without a local password are handled somehow (not stated how); WebAuthn as 2FA since 3.3.0 | Z1-Z3 |
| Discourse | no sudo window; password required before the 2FA management page; e-mailed confirmation link for backup download and granting admin; "maximum session age" default 1440 h = 60 days | n/s | 2FA setup/disable, password and e-mail changes (second-factor check), admin grant and backup (e-mail link) | passkeys can confirm some security-tab actions but not some admin screens (2023 announcement); SSO n/s | E1-E6 |
| Home Assistant | none; access token 30 min, refresh token removed after 90 days unused | sliding (refresh by use) | own password change needs the current password; nothing else | MFA skipped on trusted networks; no OIDC provider documented | W1-W4 |

Observed clusters:

- **5 to 20 minutes, fixed:** identity providers and privileged-elevation features
  (Keycloak AIA 5 min, Entra PIM re-auth reuse 10 min, GitLab OIDC step-up about 10
  min, Vaultwarden admin 20 min, Okta admin idle 15 min).
- **30 minutes to 3 hours:** general-purpose "sudo mode" libraries and services that
  gate many everyday actions (sudo_rails 30 min, GitHub 2 h sliding, django-sudo and
  Laravel 3 h, GitLab Admin Mode 6 h).
- **Per action, no window:** Cloudflare, npm, Bitwarden, Devise, GitLab's own password
  and 2FA pages, Gitea, Mastodon, Discourse (plus e-mailed links for admin grants).
- **Standards:** no number for the gate itself; ASVS 5.0 7.5.1 (L2) requires full
  re-authentication before changes to e-mail or MFA configuration, 6.8.4 (L2) requires
  verifying recentness from the IdP's own data; NIST 800-63B-4 AAL2 session limits are
  24 hours overall and 1 hour of inactivity (section 3.5).
- **Provider-only accounts:** GitHub and Nextcloud skip the prompt entirely; GitLab
  (experiment), Entra (authentication context) and Keycloak (`max_age`, step-up) send
  the person back to the IdP. The OpenID Connect spec gives exactly the tool for that:
  `max_age` / `prompt=login` plus the `auth_time` claim (section 3.4).

### 3.2 Sources for the table (checked 2026-11-06)

GitHub
- S1 https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/sudo-mode
  "GitHub has a two-hour session timeout period before prompting you for
  authentication again." "During this time, any sensitive action that you perform will
  reset the timer." Methods: password, passkey, security key, GitHub Mobile, TOTP;
  "Text messages are not supported for use on the sudo prompt."
- S2 https://docs.github.com/en/enterprise-server@latest/authentication/keeping-your-account-and-data-secure/sudo-mode
  "If your GitHub Enterprise Server instance uses an external authentication method
  like CAS or SAML SSO, you will not receive prompts to enter sudo mode."
- S3 https://docs.github.com/en/enterprise-cloud@latest/authentication/keeping-your-account-and-data-secure/sudo-mode
  EMU: "only the setup user will receive prompts to enter sudo mode"; Proof of
  Presence (public preview) "requires you to reauthenticate through your enterprise's
  identity provider".
- S4 https://docs.github.com/en/enterprise-cloud@latest/admin/managing-iam/understanding-iam-for-enterprises/abilities-and-restrictions-of-managed-user-accounts
  managed users "do not see the sudo prompt when taking sensitive actions".

GitLab (MIT; docs only)
- G1 https://docs.gitlab.com/administration/settings/sign_in_restrictions/ "Admin Mode
  times out after six hours, and you cannot change this timeout limit." "2FA, OmniAuth
  providers, and LDAP authentication are supported by Admin Mode."
- G2 https://docs.gitlab.com/administration/settings/account_and_limit_settings/
  session duration setting (default value not stated; inactivity-based; "Expire session
  from creation date" option).
- G3 https://docs.gitlab.com/user/profile/ ; G4 https://docs.gitlab.com/user/profile/user_passwords/
  (current password for a password change); G5
  https://docs.gitlab.com/user/profile/account/two_factor_authentication/ (current
  password to disable 2FA; WebAuthn registration may not need it after IdP sign-in).
- G7 https://docs.gitlab.com/administration/auth/oidc/ step-up authentication for
  Admin Mode (experiment, GitLab 17.11; group level 18.4): step-up sessions "expire
  based on the identity provider (IdP) token expiration time, typically around 10
  minutes".
- G9 https://docs.gitlab.com/user/project/merge_requests/approvals/settings/ "Require
  user re-authentication (password or SAML) to approve".
- G10 https://docs.gitlab.com/development/authentication/ "Regenerate the session ID on
  any privilege change: sign-in, 2FA pass, password change, admin-mode entry."

Google
- K1 https://support.google.com/accounts/answer/7162782 "Verify it's you" page: no
  validity window; 7-day restriction after adding a new sign-in method.
- K2 https://support.google.com/a/answer/7576830 (redirects to
  https://knowledge.workspace.google.com/admin/security/set-session-length-for-google-services)
  "By default, the web session length for Google services is 14 days." "The session
  length for admins using the Google Admin console is set to one hour and can't be
  modified."
- K3 https://support.google.com/a/answer/9368756 (redirects to
  https://knowledge.workspace.google.com/admin/security/set-session-length-for-google-cloud-services)
  "The minimum frequency allowed is 1 hour, and the maximum is 24 hours." "It is the
  fixed time that elapses before the user needs to sign in again." Re-authentication
  method "Password or Security key".
- K4 https://support.google.com/a/answer/6002699 (security challenges; a super admin
  can switch the challenge off for 10 minutes).

Microsoft Entra ID
- M1 https://learn.microsoft.com/en-us/entra/identity/conditional-access/concept-session-lifetime
  "The Microsoft Entra ID default configuration for user sign-in frequency is a
  rolling window of 90 days." Every-time prompts are limited to "once every five
  minutes" (clock skew); "Every time strict (preview)" uses a 10-second window.
  Rationale quoted: "Users who habitually enter credentials without thinking might
  unintentionally provide them to malicious prompts."
- M2 https://learn.microsoft.com/en-us/entra/identity/role-based-access-control/protected-actions-overview
  satisfied by an authentication context bound to a Conditional Access policy;
  "users are prompted only when needed".
- M3 https://learn.microsoft.com/en-us/entra/id-governance/privileged-identity-management/pim-how-to-change-default-settings
  activation "can be from one to 24 hours"; "When a user reauthenticates for one role
  activation, a 10-minute window applies."

AWS
- A1 https://docs.aws.amazon.com/IAM/latest/UserGuide/id_credentials_mfa_configure-api-require.html
  `aws:MultiFactorAuthAge` compared to "a value (such as 3600 seconds)"; fixed age.
- A2 https://docs.aws.amazon.com/IAM/latest/UserGuide/id_roles_use_switch-role-console.html
  "IAM user sessions are 12 hours by default".
- A3 https://docs.aws.amazon.com/singlesignon/latest/userguide/user-interactive-sessions.html
  "The default session duration for user interactive sessions is 8 hours ... from a
  minimum of 15 minutes to a maximum of 90 days."
- A4 https://docs.aws.amazon.com/IAM/latest/UserGuide/root-user-password.html "You are
  prompted to re-authenticate for security purposes." (root account settings; window
  not stated).

Nextcloud (AGPL; docs, FAQ and app-store release notes only)
- N1 https://docs.nextcloud.com/server/stable/developer_manual/digging_deeper/javascript-apis.html
  (`@nextcloud/password-confirmation` for controller methods carrying the
  `#[PasswordConfirmationRequired]` attribute; no window stated).
- N2 https://docs.nextcloud.com/server/latest/admin_manual/configuration_server/config_sample_php_parameters.html
  `session_lifetime` "Defaults to 60*60*24 seconds (24 hours)";
  `remember_login_cookie_lifetime` "Defaults to 60*60*24*15 seconds (15 days)";
  `allowed_no_password_confirmation_ranges` "List of trusted IP ranges that can bypass
  password confirmation."
- N3 https://nextcloud.com/secure/ (password confirmation for security-critical admin
  actions, paraphrased).
- N4 https://apps.nextcloud.com/apps/user_oidc/releases (6.2.0 added password
  confirmation in admin settings; 7.0.0 marks OIDC session tokens with a scope that
  skips password validation; paraphrased).
- N5 https://nextcloud.com/faq/ licence statement (AGPLv3).

Grafana (AGPL; docs only)
- R1 https://grafana.com/docs/grafana/latest/setup-grafana/configure-grafana/
  `login_maximum_inactive_lifetime_duration` "Default is 7 days (7d)";
  `login_maximum_lifetime_duration` "Default is 30 days (30d)";
  `token_rotation_interval_minutes` "The default is each 10 minutes."
- R2 https://grafana.com/docs/grafana/latest/administration/user-management/user-preferences/
  (old password for a self password change).

Gitea (MIT) and Forgejo (GPL-3.0+ from v9; docs only)
- T1 https://docs.gitea.com/administration/config-cheat-sheet `SESSION_LIFE_TIME`
  "default is 86400 (1 day)"; `LOGIN_REMEMBER_DAYS` default 31.
- T2 https://docs.gitea.com/help/faq (own password change needs the current password).
- T3 https://docs.gitea.com/development/api-usage (token creation needs Basic auth;
  `X-Gitea-OTP` header with 2FA).
- T4 https://forgejo.org/docs/latest/admin/config-cheat-sheet/ (same values);
  https://forgejo.org/faq/ licence statement.

Keycloak (Apache-2.0)
- C1 https://www.keycloak.org/docs/latest/server_admin/ step-up: "The option Max Age in
  the condition determines how long (how much seconds) the subsequent authentication
  level is valid." AIA: "If the policy is not configured, it defaults to five
  minutes."; "The max_age can be currently used only to make the value shorter than the
  default five minutes."; Update Email: "the user will be asked to re-authenticate if
  the last authentication is older than 5 minutes (300 seconds)".
- C2 https://www.keycloak.org/securing-apps/javascript-adapter (`maxAge`: "Specifies
  maximum time since the authentication of user happened.").

authentik (MIT per https://api.goauthentik.io/reference/authentik/)
- H1 https://docs.goauthentik.io/add-secure-apps/providers/oauth2/ (no `max_age`,
  `prompt` or re-authentication documented).
- H2 https://docs.goauthentik.io/add-secure-apps/flows-stages/flow/ (flow
  authentication requirement; no re-auth option).
- H3 https://docs.goauthentik.io/add-secure-apps/flows-stages/stages/user_login/ and
  https://docs.goauthentik.io/security/security-hardening/ ("Session duration defaults
  to seconds=0").
- H4 https://docs.goauthentik.io/users-sources/user/user-switching/ (policy expression
  example that re-runs password/MFA stages).

Okta
- O1 https://help.okta.com/oie/en-us/content/topics/identity-engine/policies/add-app-sign-on-policy-rule.htm
  "This is also called the re-authentication frequency"; "A 10-second grace period
  applies after a user authenticates."
- O2 https://help.okta.com/oie/en-us/content/topics/identity-engine/policies/modify-first-party-app-policies.htm
  "Okta recommends re-authentication every time for the Okta Admin Console."; admin
  session max lifetime recommended 12 hours (maximum 24 hours), max idle recommended 15
  minutes (maximum 2 hours), citing NIST guidance.
- O3 https://help.okta.com/oie/en-us/content/topics/identity-engine/enduser/eu-settings.htm
  (verify with two enrolled authenticators before editing settings).

Stripe, Cloudflare, npm, Apple
- P1 https://support.stripe.com/questions/why-do-i-need-to-verify-my-identity-in-the-dashboard
  (step-up "when making changes to your account"; "you may not be prompted again for a
  short period").
- F1 https://developers.cloudflare.com/fundamentals/user-profiles/2fa/ ;
  F2 https://developers.cloudflare.com/fundamentals/user-profiles/change-password-or-email/
  (password re-entered per action).
- Q1 https://docs.npmjs.com/about-two-factor-authentication ;
  Q2 https://docs.npmjs.com/configuring-two-factor-authentication ("By default, 2FA is
  enabled for authorization and writes.").
- X1 https://support.apple.com/en-us/101567 ; X2 https://support.apple.com/en-us/120340
  (one-hour security delay); X3
  https://support.apple.com/guide/mac-help/change-sign-security-settings-appleaccount-mchlf00522d9/mac

Bitwarden (help site only) and Vaultwarden (wiki only)
- B1 https://bitwarden.com/help/vault-timeout/ ; B2
  https://bitwarden.com/help/master-password-re-prompt/ ; B3
  https://bitwarden.com/help/export-your-data/ ; B4
  https://bitwarden.com/help/personal-api-key/ ; B5
  https://bitwarden.com/help/setup-two-step-login-authenticator/
- V1 https://github.com/dani-garcia/vaultwarden/wiki/Enabling-admin-page (admin session
  20 minutes, `ADMIN_SESSION_LIFETIME`; paraphrased).

Libraries
- D1 https://pypi.org/pypi/django-sudo/json (licence BSD); D2
  https://django-sudo.readthedocs.io/en/latest/config/index.html (`SUDO_COOKIE_AGE`
  10800).
- L1 https://laravel.com/docs/12.x/authentication#password-confirmation "After
  confirming their password, a user will not be asked to confirm their password again
  for three hours."
- Y1 https://rubygems.org/gems/sudo_rails and Y2 https://github.com/markets/sudo_rails
  (MIT; `sudo_session_duration` default 30 minutes; `extend_sudo_session!`).
- Y3 https://www.rubydoc.info/github/heartcombo/devise/Devise (`timeout_in` 30
  minutes); Y4
  https://www.rubydoc.info/github/heartcombo/devise/Devise/Models/DatabaseAuthenticatable
  (`update_with_password`).
- https://guides.rubyonrails.org/security.html (require the old password for password
  and e-mail changes; reset links 15 minutes).
- https://docs.djangoproject.com/en/stable/topics/auth/default/ (Django ships no sudo
  mode; `PasswordChangeForm` vs `SetPasswordForm`).

### 3.3 Mastodon, Discourse, Home Assistant

Mastodon (AGPL; release-notes pages and docs.joinmastodon.org only, nothing opened in
the repository itself)
- Z1 https://github.com/mastodon/mastodon/releases/tag/v3.0.0 lists "Add password
  challenge to 2FA settings, e-mail notifications" and "Fix 2FA challenge and password
  challenge for non-database users". No validity window is stated.
- Z2 https://github.com/mastodon/mastodon/releases/tag/v3.3.0 "Add WebAuthn as an
  alternative 2FA method"; "Fix 2FA/sign-in token sessions being valid after password
  change".
- Z3 https://docs.joinmastodon.org/user/contacts/ (account settings: change e-mail,
  set a new password, revoke sessions and apps, enable 2FA; no confirmation window
  stated). A search snippet claims the challenge is remembered for one hour; the only
  source for that is a pull request, which is off limits (rule 6), so it is UNVERIFIED
  and not used.

Discourse (GPL-2.0; meta.discourse.org topics read through their public JSON views,
nothing from the repository)
- E1 https://meta.discourse.org/t/283014.json password change goes through the
  e-mailed reset link from Preferences > Security; no in-app current-password field.
- E2 https://meta.discourse.org/t/76689.json (staff thread, 2017-2018) password before
  the 2FA setup is shown; second-factor checks on password and e-mail changes; a
  limited-minutes unlock window was proposed but no duration was settled.
- E3 https://meta.discourse.org/t/116279.json (2019) the second-factor management page
  is reached only after confirming the password; validity of that confirmation not
  stated.
- E4 https://meta.discourse.org/t/52603.json (2016-2018) a password re-prompt before
  admin actions was proposed and never built; E5
  https://meta.discourse.org/t/60164.json backup download and granting admin require
  clicking an e-mailed confirmation link (2017).
- E6 https://meta.discourse.org/t/285589.json (passkeys announcement, 2023-11-16)
  passkeys can confirm some sensitive actions in the Security tab but not yet some
  admin-only screens; https://meta.discourse.org/t/394095.json "maximum session age"
  site setting, default 1440 hours (60 days).

Home Assistant (Apache-2.0)
- W1 https://www.home-assistant.io/docs/authentication/ change password asks for the
  current password and then offers to sign out other sessions; refresh tokens per
  device; a refresh token is considered unused after 90 days and removed.
- W2 https://contributors.home-assistant.io/core/internals/auth-api/ access token
  `expires_in` 1800 (30 minutes); refresh token expires when unused for 90 days;
  long-lived tokens 10 years.
- W3 https://www.home-assistant.io/docs/authentication/multi-factor-auth/ MFA after the
  password at login; nothing about MFA before settings changes.
- W4 https://www.home-assistant.io/docs/authentication/providers/ trusted networks skip
  password and MFA.

Takeaway from the three: community self-hosted apps without an IdP background either
ask for the password per action (Mastodon, Discourse) or not at all (Home Assistant);
none documents a window. The documented windows all come from products with an
identity or developer-platform focus (section 3.1).

### 3.4 OpenID Connect: the tool for provider-only accounts

https://openid.net/specs/openid-connect-core-1_0.html (checked 2026-11-06):

- `prompt=login`: "The Authorization Server SHOULD prompt the End-User for
  reauthentication. If it cannot reauthenticate the End-User, it MUST return an error,
  typically login_required."
- `max_age`: "Specifies the allowable elapsed time in seconds since the last time the
  End-User was actively authenticated by the OP. If the elapsed time is greater than
  this value, the OP MUST attempt to actively re-authenticate the End-User. ... When
  max_age is used, the ID Token returned MUST include an auth_time Claim Value. Note
  that max_age=0 is equivalent to prompt=login."
- `auth_time` (section 2): "Time when the End-User authentication occurred. ... When a
  max_age request is made or when auth_time is requested as an Essential Claim, then
  this Claim is REQUIRED; otherwise, its inclusion is OPTIONAL."

Auth0's guidance (https://auth0.com/docs/authenticate/login/max-age-reauthentication,
checked 2026-11-06) explains why `max_age=0` is preferable to `prompt=login`: with
`prompt=login` "the RP has no way to validate that a re-authentication action has
taken place"; the client "must validate that re-authentication has taken place using
the auth_time claim"; `max_age=0` makes `auth_time` mandatory and is "impervious to the
same kind of client tampering".

Keycloak honours `max_age` and `prompt=login` (C1, C2). authentik's documentation does
not say whether it honours them (H1); this must be tested against the owner's authentik
in M11 and, if it does not, the fallback in section 4.4 applies.

### 3.5 Standards: OWASP and NIST

None of the standards names a number for a sudo-style window. They require the
mechanism (re-authenticate before changes that affect authentication) and leave the
length to a documented risk decision; the numbers they do give are session timeouts.
All quotes checked 2026-11-06.

OWASP ASVS 5.0.0 (May 2025),
https://raw.githubusercontent.com/OWASP/ASVS/master/5.0/en/0x16-V7-Session-Management.md
and .../0x15-V6-Authentication.md (official OWASP/ASVS repository, CC BY-SA 4.0):
- 7.5.1 (L2): "Verify that the application requires full re-authentication before
  allowing modifications to sensitive account attributes which may affect
  authentication such as email address, phone number, MFA configuration, or other
  information used in account recovery."
- 7.5.2 (L2): "Verify that users are able to view and (having authenticated again with
  at least one factor) terminate any or all currently active sessions."
- 7.5.3 (L3): "Verify that the application requires further authentication with at
  least one factor or secondary verification before performing highly sensitive
  transactions or operations."
- 7.1.1 (L2): the inactivity timeout and absolute maximum session lifetime must be
  "documented ... and that the documentation includes justification for any deviations
  from NIST SP 800-63B re-authentication requirements." 7.3.1 and 7.3.2 (L2) require an
  inactivity timeout and an absolute maximum lifetime "according to risk analysis and
  documented security decisions."
- 6.2.3 (L1): "Verify that password change functionality requires the user's current
  and new password."
- 6.8.4 (L2): "Verify that, if an application uses a separate Identity Provider (IdP)
  and expects specific authentication strength, methods, or recentness for specific
  functions, the application verifies this using the information returned by the IdP."
  This is the requirement behind section 4.4 (check `auth_time`, do not trust the
  redirect alone).

OWASP ASVS 4.0.3 (October 2021),
https://raw.githubusercontent.com/OWASP/ASVS/master/4.0/en/0x12-V3-Session-management.md
- 3.3.2 (NIST 7.2): "If authenticators permit users to remain logged in, verify that
  re-authentication occurs periodically both when actively used or after an idle
  period." Levels: L1 "30 days"; L2 "12 hours or 30 minutes of inactivity, 2FA
  optional"; L3 "12 hours or 15 minutes of inactivity, with 2FA".
- 3.7.1 (L1-L3): "Verify the application ensures a full, valid login session or
  requires re-authentication or secondary verification before allowing any sensitive
  transactions or account modifications."
- 3.3.4 (L2/L3): view and "(having re-entered login credentials)" log out of other
  sessions. 3.6.1/3.6.2 (L3): the relying party specifies a maximum authentication time
  to the credential service provider and the CSP reports the last authentication event
  (the OIDC `max_age`/`auth_time` pair).

OWASP Session Management Cheat Sheet,
https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html
- "Common idle timeouts ranges are 2-5 minutes for high-value applications and 15-30
  minutes for low risk applications. Absolute timeouts depend on how long a user
  usually uses the application. If the application is intended to be used by an office
  worker for a full day, an appropriate absolute timeout range could be between 4 and 8
  hours."
- "Web applications should require reauthentication after high-risk events such as:"
  "Changes to critical user information (e.g., password, email address)".
- "The session ID must be renewed or regenerated by the web application after any
  privilege level change within the associated user session."
- The word "sudo" does not appear on the page.

OWASP Authentication Cheat Sheet,
https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
- "Require Re-authentication for Sensitive Features": "it's important to require the
  current credentials for an account before updating sensitive account information
  such as the user's password or email address -- or before sensitive transactions
  ... Without this countermeasure, an attacker may be able to execute sensitive
  transactions through a CSRF or XSS attack without needing to know the user's current
  credentials. Additionally, an attacker may get temporary physical access to a user's
  browser or steal their session ID to take over the user's session."
- "Re-authentication Mechanisms": "Bound Authenticators - Require an authenticator
  already bound to the account, such as the current password or a passkey."; "Invalidate
  sessions after re-authentication and rotate tokens".

NIST SP 800-63B rev 3 (June 2017), https://pages.nist.gov/800-63-3/sp800-63b.html
- 4.2.3 (AAL2): "authentication of the subscriber SHALL be repeated at least once per
  12 hours during an extended usage session, regardless of user activity.
  Reauthentication of the subscriber SHALL be repeated following any period of
  inactivity lasting 30 minutes or longer."
- 4.3.3 (AAL3): 12 hours, 15 minutes of inactivity, "Reauthentication SHALL use both
  authentication factors."
- 7.2: "Prior to session expiration, the reauthentication time limit SHALL be extended
  by prompting the subscriber for the authentication factor(s) specified in Table 7-1."
  Table 7-1: AAL2 "Presentation of a memorized secret or biometric", with the note that
  "a memorized secret or biometric, and not a physical authenticator, is required
  because the session secret is something you have".

NIST SP 800-63B-4 (final, July 2025), https://pages.nist.gov/800-63-4/sp800-63b.html
(publication record https://csrc.nist.gov/pubs/sp/800/63/b/4/final)
- 2.2.3 (AAL2): "A definite reauthentication overall timeout SHALL be established, which
  SHOULD be no more than 24 hours at AAL2. The inactivity timeout SHOULD be no more
  than 1 hour. When the inactivity timeout has occurred but the overall timeout has not
  yet occurred, the verifier MAY allow the subscriber to reauthenticate using only a
  successful password or biometric comparison in conjunction with the session secret".
  Table 1: AAL1 "30 days overall"; AAL2 "24 hours overall / 1 hour inactivity / Single
  factor required"; AAL3 "12 hours overall / 15 minutes inactivity".
- 2.3.3 (AAL3): "the overall timeout for reauthentication SHALL be no more than 12
  hours. The inactivity timeout SHOULD be no more than 15 minutes."
- 5.2: "Session activity SHALL reset the inactivity timeout, and successful
  reauthentication during a session SHALL reset both timeouts."
- Rev 4 has no sentence tying reauthentication to sensitive actions; the nearest is
  4.1.2 on binding a new authenticator: "the CSP SHALL ensure that the process requires
  authentication at either the maximum AAL currently available in the subscriber
  account or the maximum AAL at which the new authenticator will be used, whichever is
  lower."

What this means for us: ASVS 7.5.1 (L2) is the requirement our gate implements, and it
asks for "full" re-authentication for changes to MFA configuration and e-mail, which is
why the proof includes the TOTP step when the account has one. NIST 800-63B-4 relaxes
AAL2 session timeouts (24 h / 1 h) and allows a single factor to re-open a session; our
30-day sliding session (DEC-069) is well outside AAL2 either way, which is a reason to
keep the gate short rather than long: the gate is the only place a stolen or left-open
session meets a credential check.

## 4. Proposal and reasoning

### 4.1 Window: 15 minutes sliding, 1 hour cap

- Our gated set is small and rare (section 4.2): an admin touches sign-in settings a
  few times a year, usually in one sitting of several related edits (add a provider,
  paste the secret, run Test, set the mode). That is the "privileged elevation" cluster
  (Keycloak AIA 5 min, Entra PIM 10 min, Vaultwarden admin 20 min, Okta admin idle 15
  min), not the "sudo mode over many everyday actions" cluster where 2 to 3 hours is
  usual (GitHub, django-sudo, Laravel). A long window there is an answer to prompt
  fatigue over frequent actions; we do not have frequent actions.
- 15 minutes sliding instead of 10 minutes fixed: a fixed 10 minutes can expire in the
  middle of a provider setup (copying a client secret from another tab, waiting for a
  Test), and the retry after a refused save is exactly the "ask twice in one flow" that
  D12 forbids. Sliding follows GitHub's documented behaviour ("any sensitive action ...
  will reset the timer"). The 1-hour absolute cap keeps the sliding window from being
  held open all day by occasional edits; it matches Google's fixed 1-hour admin console
  session, the lower bound of Google Cloud's re-auth policy, and NIST 800-63B-4's AAL2
  inactivity limit of 1 hour (the strictest session figure that still allows a single
  factor to re-open the session).
- The TOTP step is part of the proof when the account has TOTP: ASVS 5.0 7.5.1 asks for
  "full re-authentication" before MFA and e-mail changes, and NIST rev 3 table 7-1
  explains why a password (something you know) and not only the session cookie
  (something you have) must be presented. The password alone would also be the weaker
  factor to leave in charge of disabling TOTP.
- Signing in counts as the proof (`reauth_at = signed_in_at`), as Keycloak does by
  reading `auth_time` of the login. Nobody is asked for a password twice in a row.
- Not an admin setting: GitHub and GitLab both document their windows as fixed. One
  constant avoids another row in Admin > Instance (OD-01 density) and avoids the odd
  case of a gated setting that controls its own gate. An env override is a cheap later
  addition if an operator asks.
- Where it is stored: server side on the session row (`reauth_at`, `reauth_method`),
  never in a separate cookie (django-sudo's cookie approach needs a second signed
  cookie and a second expiry to reason about). Sign-out and "revoke other sessions"
  clear it with the session. The session id is regenerated on a successful proof
  (GitLab's development guideline G10: regenerate on "admin-mode entry").

### 4.2 Gated actions

The rule: **anything that changes who can sign in, or how, needs a fresh proof; anything
else is covered by the audit log.** Concretely:

| Area | Gated (fresh proof) | Not gated (audit row only) |
|---|---|---|
| Admin > Sign-in and SSO | sign-in mode (`page`/`auto`); provider add, edit, disable, delete; group-to-role rules; auto-create setting | running Test on a provider |
| Admin > Instance | sign-up mode; session length; cookie security; re-opening the setup wizard | hosting mode, mail settings, secrets rotation, storage budget, notification defaults |
| Admin > Users | grant or revoke the admin role; disable or delete a user | invites, display names |
| Own account (Settings > You) | change password; enrol or remove TOTP; view recovery codes; link or unlink an OIDC identity; change e-mail; revoke all other sessions | profile, theme, language, notification channels |
| Data sources | - | keys (encrypted, audited) |

This matches the union of what GitHub, GitLab, Cloudflare and Bitwarden gate on the
account side (password, 2FA, e-mail, linked apps, recovery codes, sessions) and what
Entra's protected actions and GitLab Admin Mode gate on the admin side (access policy
and identity configuration). It is also the set that, if changed by a hijacked session,
would let an attacker keep access after the real admin notices.

The password change dialog asks for the current password anyway (every product in the
table does); that entry counts as the proof and refreshes the window, so there is no
second prompt.

### 4.3 Proof methods

- **Password, then TOTP when enrolled:** a dialog (D15 allows a dialog for security
  re-authentication) with the password field and, after success, the TOTP step; the
  same throttle as sign-in (DEC-155). Recovery codes are accepted in the TOTP step.
- **Passkeys:** not in v1 (DEC-071: WebAuthn later). When they arrive, a passkey is a
  valid proof on its own, as GitHub, Stripe and Okta allow.
- **Provider round trip** for accounts whose only credential is an OIDC identity
  (section 4.4).
- The audit row of the gated action records the proof method (`password`,
  `password+totp`, `oidc:<provider>`).

### 4.4 Admins without a password (OIDC-only)

Do not skip the check (GitHub skips it for SSO-only accounts and Nextcloud marks OIDC
sessions to skip it; both are weaker than what the standard offers, and in our `auto`
mode most admins will be provider-only). Instead:

1. The dialog shows one button, "Confirm with <provider name>" (plus the password form
   when the account also has a password).
2. We start an authorization request to the provider the session was created with,
   with `max_age=0`, a new `state` and `nonce`, and the return URL of the gated page;
   the pending action is kept server side and re-shown after the round trip (no form
   data lost: D12).
3. On the callback we require, in addition to the normal ID token checks (A-18):
   `auth_time` present, `auth_time >= now - 300 s` (5 minutes, Keycloak's default AIA
   age, tolerant of slow MFA and clock skew), `iss` and `sub` equal to the session's
   identity. Then `reauth_at = now`, method `oidc:<provider>`.
4. If `auth_time` is missing or old (a provider that ignores `max_age`), the action is
   refused with "<provider> did not confirm a fresh sign-in"; the admin can use the
   password form if the account has one. There is no silent pass-through. In `auto`
   mode at least one admin has a password and an authenticator (addendum 4.3), so an
   instance can never be locked out of its own sign-in settings by a provider that does
   not honour `max_age`.
5. The provider's Test (addendum 3.4) gains one line: "Fresh sign-in (`max_age`)
   supported: yes/no", recorded from a test round trip by the acting admin, so the
   limitation is visible before it bites.

### 4.5 What this costs

One timestamp and one enum on the session row; one `require_fresh_auth` guard used by
the handlers in 4.2; one dialog component (`ReauthDialog`) in `ui`; one extra callback
mode in the OIDC handler (`purpose=reauth`); strings in both locales. Tests: window
expiry, sliding refresh, cap, sign-in counts as proof, OIDC `auth_time` accepted and
refused, session regeneration.

## 5. Alternatives considered

| Option | Window | Why not |
|---|---|---|
| Keep the placeholder | 10 minutes, fixed | expires mid-flow during a provider setup and then asks twice (D12); nobody in the table uses a fixed 10 minutes for app-side admin actions (Entra's 10 minutes is a reuse window between PIM activations) |
| GitHub-style | 2 hours, sliding | made for dozens of everyday actions; for a handful of rare settings it means an admin who confirmed once at lunch is still elevated at the end of the afternoon on a shared machine |
| Per action, no window | - | Cloudflare and npm do it; for us it would ask for the password on every save while setting up a provider (D12) |
| Configurable in the admin UI | admin chooses | one more row under the density budget; a setting that gates itself; GitHub and GitLab do not offer it either |
| Skip for OIDC-only admins | - | GitHub and Nextcloud do; it turns the gate off for exactly the accounts `auto` mode makes common |

## 6. Draft question for the owner's next review page

**OD-12 Re-authentication before sign-in settings change.** When an admin changes how
people sign in (sign-in mode, identity providers, sign-up policy, session and cookie
settings, admin roles) or someone changes their own password, TOTP, e-mail or linked
provider, we ask them to confirm it is really them. How long should one confirmation
stay valid?

- **Recommended: 15 minutes, extended by every confirmed change, never longer than 1
  hour in total.** Signing in counts as a confirmation. Admins without a password are
  sent to their identity provider for a fresh sign-in (`max_age=0`) and we verify the
  provider's `auth_time`; the check is never skipped. This sits between what identity
  providers use for privileged actions (Keycloak 5 min, Microsoft Entra 10 min) and
  what general "sudo mode" products use (GitHub 2 h, Laravel 3 h), and the extension
  avoids asking twice in one editing session.
- Alternative A: keep 10 minutes, fixed (the current placeholder). Simpler, but it can
  run out while you are copying a client secret from another tab.
- Alternative B: 2 hours like GitHub, extended on every change. Fewer prompts; a longer
  time in which a left-open browser can change sign-in settings.
- Alternative C: ask on every single change, no window (Cloudflare and npm style).
  Strictest; several prompts during one provider setup.

Not in question (applies to all options): which actions are gated (list in section
4.2), password plus TOTP when enrolled as the proof, and the provider round trip for
password-less admins.

## 7. Verification notes

- UNVERIFIED: Nextcloud's often quoted 30-minute password-confirmation window is not in
  its public documentation; only the mechanism is. Sentry's sudo-mode help article
  could not be opened (sign-in wall). The Keycloak securing-apps guide URL returned 404;
  the javascript-adapter page was used instead. AWS console sign-in page returned no
  body; the switch-role page was used for the 12-hour session.
- UNVERIFIED: Mastodon's challenge validity (a search snippet says one hour; the only
  source is a pull request, off limits). meta.discourse.org refused HTML fetches (403);
  the public JSON views of the same topics were read instead.
- Not found in documentation (searched 2026-11-06): any re-authentication feature in
  Grafana, Gitea, Forgejo, authentik, Home Assistant; a per-action re-auth window for
  Google Account, Microsoft account, Stripe, Cloudflare, npm, Bitwarden, Mastodon,
  Discourse.
- Licences recorded only as stated on the cited pages (Nextcloud FAQ, Forgejo FAQ,
  Gitea docs, authentik API reference, PyPI, RubyGems); Mastodon, Discourse, Bitwarden,
  Vaultwarden and Home Assistant licences are from memory and were treated as GPL-family
  (docs only) regardless.
- Google Workspace admin pages now live on knowledge.workspace.google.com; the
  support.google.com URLs redirect there.
