# Design addendum 1: disclosure, secrets at rest, admin area, OIDC and SSO

Status: proposal v1, 2026-11-06; owner rulings applied (owner review 2026-11-06, DEC-171, DEC-172; see section 5). Extends `design.md` (proposal v1) and `research.md`.
Written for the owner and for the developer who implements it. Four requirements from
the owner's review are covered, each with: research findings (cited, numbered `A-nn`
so they do not collide with `R-nn`), concrete design decisions, changes to `design.md`
by section number, and new owner decisions with a default (`OD-nn`).

Conventions as in `design.md`: the product is "[Brand]"; codename only in
`config/brand/default/brand.toml`; ASCII throughout except the German microcopy.
Mockups for this addendum: `mockups/admin.html` and `mockups/signin-sso.html`
(linked from `mockups/index.html`).

Honesty note. Sources were fetched on 2026-11-06. Licences were checked before any
third-party documentation was read; projects whose code or docs are GPL-family were
skipped and are named as skipped (section 4.1). Where a page could not be opened the
finding says so. Nothing is quoted from a GPL-family project. Crate licences are quoted
from the crates.io API, not from memory. Where this addendum states how a provider
behaves, it states only what that provider's own documentation says; everything else is
the standard's wording.

Contents

1. Decluttering and progressive disclosure
2. Encryption of secrets at rest
3. Instance admin settings
4. OIDC with authentik and other providers; true SSO
5. Consolidated owner decisions

---

## 1. Decluttering and progressive disclosure

Owner requirement: "No need to have all options open by default; it should look sleek
but do the job; a11y must be respected."

### 1.1 Findings

**A-01. Progressive disclosure: show the few important options first, the rest on
request; at most two levels.** NN/g defines it as deferring "advanced or rarely used
features to a secondary screen, making applications easier to learn and less
error-prone", with two steps: "Initially, show users only a few of the most important
options" and "Offer a larger set of specialized options upon request." Being on the
initial display signals importance. The two things to get right are "the right split
between initial and secondary features" and that "It must be obvious how users progress
from the primary to the secondary disclosure levels." Hard limit: "designs that go
beyond 2 disclosure levels typically have low usability because users often get lost
when moving between the levels." Staged disclosure (wizards) is the linear variant,
right "when you can divide a task into distinct steps that have little interaction".
Evidence: Nielsen, "Progressive Disclosure", NN/g, 2006,
https://www.nngroup.com/articles/progressive-disclosure/ [primary; proprietary text,
quoted briefly with attribution].

**A-02. Accordions and collapsed sections cost interaction and hide content; use them
when people need only a few of the sections, let several stay open, remember the
state, and never hide what most people need.** NN/g: "Accordions shorten pages and
reduce scrolling, but they increase the interaction cost by requiring people to decide
on topic headings"; "Hiding content behind navigation diminishes people's awareness of
it"; avoid them "when your audience needs most or all of the content on the page";
if used, "give people the capability to open multiple sections at a time" and the state
"should remain in that state until the user changes it"; the heading must be a button,
collapsed content must be "both visually hidden and programmatically unreachable";
"the caret and the plus work best"; "Avoid hiding any crucial information within the
collapsed panels." Also: little content plus accordions looks empty; deep hierarchies
want tabs or local navigation instead.
Evidence: Loranger, "Accordions Are Not Always the Answer for Complex Content on
Desktops", NN/g, 2014, https://www.nngroup.com/articles/accordions-complex-content/ ;
Wang, "Accordions on Desktop: When and How to Use", NN/g, 2023,
https://www.nngroup.com/articles/accordions-on-desktop/ [primary].

**A-03. Cognitive load: remove extraneous elements, build on familiar layouts, and
offload reading, remembering and deciding onto defaults and re-displayed data.** NN/g:
"redundant links, irrelevant images, and meaningless typography flourishes slow users
down"; "Look for anything in your design that requires users to read text, remember
information, or make a decision" and ask whether you "can ... show a picture,
re-display previously entered information, or set a smart default".
Evidence: Whitenton, "Minimize Cognitive Load to Maximize Usability", NN/g, 2013,
https://www.nngroup.com/articles/minimize-cognitive-load/ [primary].

**A-04. Calm technology principles: least attention, periphery, works when it fails,
the minimum technology that solves the problem.** Amber Case's eight principles include
"Technology should require the smallest possible amount of attention", "Give people
what they need to solve their problem, and nothing more", "Technology should work even
when it fails" ("Does it default to a usable state or does it break down completely?"),
and "Slim the feature set down so that the product does what it needs to do and no
more." On interruptions: alerts belong to deletion of important content, emergencies,
or things the person opted into.
Evidence: https://calmtech.com/ (Amber Case, Calm Tech Institute; all rights reserved,
registered trademark) [primary, opinion-level source].

**A-05. WAI-ARIA disclosure pattern: a button with `aria-expanded`, optional
`aria-controls`, toggled by Enter and Space.** "The element that shows and hides the
content has role button"; `aria-expanded` is "true" when visible, "false" when hidden.
Evidence: W3C WAI, APG "Disclosure (Show/Hide) Pattern",
https://www.w3.org/WAI/ARIA/apg/patterns/disclosure/ [primary; W3C Document License,
reuse with attribution, no modification].

**A-06. WAI-ARIA accordion pattern: each header is a button inside a heading with
`aria-level`; single-open, multi-open and always-one-open are all allowed; all headers
are in the Tab sequence; do not add `role=region` to more than about six panels.**
Evidence: W3C WAI, APG "Accordion Pattern",
https://www.w3.org/WAI/ARIA/apg/patterns/accordion/ [primary].

**A-07. GOV.UK Details: a native `details`/`summary` link for "information that only
some users will need"; "Do not use the details component to hide information that the
majority of your users will need"; prefer it to tabs or an accordion "if you only have
1 section of content"; keep the summary text "short and descriptive". Research caveats:
some users avoid the link because they fear it leaves the page; some voice-control
users may struggle with it; the expanded state was not announced on iOS before v5.7.0.**
Evidence: GOV.UK Design System, "Details",
https://design-system.service.gov.uk/components/details/ (content under the Open
Government Licence v3.0; the GOV.UK Frontend code is MIT,
https://raw.githubusercontent.com/alphagov/govuk-frontend/main/LICENSE.txt, "The MIT
License (MIT)", Crown Copyright 2017) [primary].

**A-08. WCAG 2.2 criteria that decide how disclosure and density are built.**
- 2.4.11 Focus Not Obscured (Minimum), AA: "When a user interface component receives
  keyboard focus, the component is not entirely hidden due to author-created content."
  Sticky headers and footers are the typical offenders; `scroll-padding` (technique
  C43) is a sufficient fix; accordions that push content down cannot obscure anything;
  a proper modal dialog always passes.
  https://www.w3.org/WAI/WCAG22/Understanding/focus-not-obscured-minimum.html
- 2.5.8 Target Size (Minimum), AA: pointer targets "at least 24 by 24 CSS pixels",
  except spaced undersized targets (a 24 px circle on each does not intersect another
  target), equivalent controls on the same page, inline targets in text, user-agent
  controls, and essential presentations. "The requirement is independent of the zoom
  factor of the page." Example: 20 x 20 targets with 4 px gaps pass, with no gap fail.
  https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html
- 3.3.7 Redundant Entry, A: information entered earlier in the same process "is
  either: auto-populated, or available for the user to select", unless essential, needed
  for security, or no longer valid. Browser autocomplete does not count; "it is the
  content (the website) that needs to provide the stored information". A checkbox
  "same as above" and keeping data after an error are named examples.
  https://www.w3.org/WAI/WCAG22/Understanding/redundant-entry.html
- 3.2.6 Consistent Help, A: when help mechanisms (contact details, contact mechanism,
  self-help, automated contact) repeat across pages they "occur in the same order
  relative to other page content". It does not require help to exist.
  https://www.w3.org/WAI/WCAG22/Understanding/consistent-help.html
- 2.4.13 Focus Appearance is AAA (2 px perimeter, 3:1 change); noted so nobody budgets
  for it as AA.
  https://www.w3.org/WAI/WCAG22/Understanding/focus-appearance.html
All [primary], W3C Document License. Together with R-41 (contrast, focus visible,
reduced motion, keyboard) these are the a11y floor of the density rules below.

**A-09. Apple's Human Interface Guidelines page on Settings could not be read from this
host (client-rendered page, empty body).** Nothing from it is used; the "settings are
for rarely changed things, decide in context where possible" advice below is design
judgement consistent with A-01 and A-03. [not available]

### 1.2 Design decisions: the density budget

A rule set the team applies to every new screen. Each rule names its evidence.

**D1. One primary action per screen, visible without scrolling at 360 x 640 and at
1280 x 720.** Everything that is not needed to do that action is secondary. (A-01,
A-03; design.md 6.1 "one primary action per screen".)

**D2. The initial view of a form or settings page shows at most 5 inputs or 7 rows.**
Beyond that, group the rest under one disclosure per topic, each named by its contents
and status. The limit is a budget, not a target: fewer is better. (A-01 "only a few of
the most important options"; A-03.)

**D3. Exactly two levels: page and disclosure.** No disclosure inside a disclosure. A
third level is a separate page with its own URL and H1. (A-01: beyond two levels
"users often get lost".)

**D4. What goes behind a disclosure: things fewer than half of the people on that page
need on a typical visit.** What most people need stays open; a disclosure never hides
status, errors or the primary action. (A-02, A-07 "Do not use ... to hide information
that the majority of your users will need".)

**D5. A closed disclosure still tells you what is inside and whether it needs you.**
The summary line carries the section name, a one-line contents hint and, where
relevant, a status word with a dot. A section with an error or a required empty field
opens itself on render. (A-02 "Hiding content behind navigation diminishes people's
awareness of it"; A-07 "short and descriptive".)

**D6. Several disclosures may be open at once; the open state is remembered per user
and page (localStorage key `app-disclosure:<page>:<section>`); "Expand all / Collapse
all" appears when a page has four or more.** (A-02, A-06.)

**D7. Implementation: native `details`/`summary` for static content; the APG disclosure
button (`aria-expanded`, `aria-controls`) for anything with form controls or lazy
loading; header is a `button` inside a heading of the right level; caret icon
(`chevron-right` closed, `chevron-down` open), both heading and icon clickable;
collapsed content is `hidden`, not merely visually hidden.** (A-05, A-06, A-07 caveat
about iOS announcements, A-02.)

**D8. Hiding is not removing: every secondary option stays reachable within one
action from the page where its effect is visible.** No settings move to another page
unless they have their own URL in 3.3. (A-01 "obvious how users progress".)

**D9. Advanced and rare settings have the word "Advanced" or the rarity in the summary
("Rarely needed at home").** Setting names are nouns; summaries list what is inside so
nobody opens a section to find out. (A-07.)

**D10. Targets: 24 x 24 CSS px minimum everywhere, 44 x 44 on mobile primary
controls (design.md 8); dense icon rows get a 4 px gap minimum so the spacing exception
holds; nothing relies on zoom.** (A-08 2.5.8.)

**D11. Sticky bars and disclosures never cover focus: `scroll-padding-top` 3.5 rem,
`scroll-padding-bottom` = tab bar height, and disclosures push content down rather than
overlaying it.** (A-08 2.4.11; design.md 7.4.)

**D12. Never ask twice in one flow: the wizard, the log sheet, the invite and the
channel setup reuse what was typed earlier (prefill or "same as ...").** Exception:
passwords and the TOTP code when re-authentication is for security. (A-08 3.3.7.)

**D13. Help lives in the same place on every page: the "?" shortcut help and the
brand's support link sit last in the account menu, and the docs link sits last in the
settings side nav.** (A-08 3.2.6.)

**D14. Status before decoration: on every admin or settings page the first element
after the lead is the thing that needs attention, in a sentence with one action; when
nothing needs attention the block is not rendered.** (A-04, design.md 3.7.)

**D15. Interruptions: dialogs only for destructive confirmations, security
re-authentication, and things the person asked for; everything else is inline or a
`role=status` toast.** (A-04, R-38.)

**D16. Numbers to watch (phase 4 instrumentation, design.md 9): count of open
disclosures at the moment of a save, and how often a disclosure is opened within 2 s of
page load. A section that is opened on more than half of the visits is promoted to the
open set (D4 says it belongs there).**

### 1.3 Page by page: what is open, what moves behind a disclosure, what is removed

Numbers refer to `design.md` section 6 and the flows in section 5. "Open" means in the
initial render. "Disclosure" means a D7 widget on the same page. "Removed" means not
built, or moved to the place named.

**6.1 Home.** Open: greeting line, the "needs attention" block when it applies (admin
nudge; offline banner), Continue (up to 6), Up next (6 posters), This year (4 numbers).
Disclosure: none; Recent activity becomes a section with 5 rows and a "More" link to
History instead of 10 rows (half the page height on a phone). Removed: the one-line
summary under the greeting ("38 movies this year...") merges into This year; the
secondary link on each Continue card goes into the card's quick menu (one visible
button per card, D1).

**6.2 Library.** Open: H1 with count, Add, group segmented control, kind chips, status
tabs, filter text box, sort, view toggle, the grid. Disclosure: a single "Filters"
button opens a popover (desktop) or sheet (mobile) with tags, facets, rating, owned;
active filters still render as removable badges in the open row. Removed from the open
row: Import (it lives under Settings > Import and in the empty state; D2 row budget),
the separate Tags/facets/Rating/Owned buttons (replaced by the one Filters button).
Bulk bar unchanged (appears only in list view with a selection).

**6.3 Item page.** Open: cover, facts card with 3 rows (status, progress or next
step, rating), title block, action bar (one primary + "With..." + overflow), rating/
tags/lists row, the kind-specific tab. Disclosure: the facts card gets "More facts"
(added, started, finished, rewatches, owned flags); companions row collapses to
"Watched with Anna and 2 others" with expansion. Removed: nothing; "report wrong
match" stays in the overflow menu. Notes tab: private note open, "Publish as review"
and spoiler flag behind a "Share this note" disclosure (fewer than half publish).

**6.4 Palette.** Open: input, scope segments, sections capped at 5 rows (was 8) with
"more"; footer shows source status only while a source is not Working (D14). Removed:
the `Enter`/`Ctrl+Enter` hints on every row; they show once on the active row only
(already the design) and in `?`.

**6.5 Lists.** Open: tabs, cards. Disclosure: list page description (edit in place
when empty is a link "Add a description"); share dialog unchanged (three radios are the
whole content). Removed: the freshness line on cards when it is older than 30 days
(shown as nothing instead of "6 months ago", calm periphery, A-04).

**6.6 History.** Open: group switcher, stream by day. Disclosure: "Filters" (person,
year, shared with me) as in 6.2; the side stats panel collapses to the 3 most relevant
numbers (this period vs last) with "All stats". Removed from this page: Export (it
lives under Settings > Import and export; the History H1 keeps a text link).

**6.7 People.** Open: request cards, tabs, person cards with 2 numbers (together, last)
instead of 4. Disclosure: the remaining stats on the person page. Removed: duplicate
suggestion card becomes a single line above the grid with "Review" (it was a card).

**6.8 Notifications.** Open: Inbox. Channels tab: cards with name, type, health, one
action (Send test); the per-type add form opens as a sheet, not inline (the inline form
made the page twice as long when adding). Rules tab: the matrix shows only events that
can fire on this instance (a kind without a source has no "new episode" row) and the
instance defaults are a collapsed group "Instance defaults (locked)" at the end. Log
tab: errors expand per row (already the design).

**6.9 Integrations.** Open: sender cards (name, health, last item, one action). The
three-step connect panel is a sheet. Disclosure: the last refused payload under
"Show the last call" on a card with an error (opens itself on error, D5).

**6.10 Data sources (admin).** Open: the attention block, the table (Source, Kinds,
Status, action; Credential and Last 24 h move into the expanded row). Disclosure: each
row expands to the source card (fields, Test, how-to, attribution preview, Disabled
switch); the "More sources" row in the wizard stays as in the mockup. Removed from
this page: the attribution list (it lives on the public About page only; the admin page
links to it).

**6.11 Imports and export.** Open: one card per format with one line and a Choose file
button. Disclosure: "What maps to what" in the dry-run preview (counts open, the
field table behind the disclosure). Removed: nothing.

**6.12 Settings > Account.** Open: display name, email, theme, locale (4 inputs, D2).
Disclosure: "Sign-in" (password change, authenticator, linked providers; status in the
summary: "Password and authenticator, authentik"), "Delete account" (destructive, last).
Removed: username shown as a read-only line under the display name instead of a field.

**6.13 Admin.** Rebuilt in section 3 of this addendum; the mockup `admin.html` applies
D1-D15.

**6.14 Auth pages.** Sign-in: with one or more providers, the password form is behind
"Sign in with a password instead" (section 4, `signin-sso.html`); with none, it is
open and there is no disclosure (D4 either way). Sign-up: unchanged (4 fields, 1 of
them optional and collapsed on invite pages where the display name is prefilled from
the companion, D12).

**5.1 Wizard.** Step 3: the first two sources whose kinds match the hosting choice are
open; the rest are one disclosure row each, as mocked. Step 4 checklist stays.

**5.4 Log sheets.** `WatchedSheet`: date chips and people open; rating, note, rewatch
behind "More" (D4: fewer than half rate at log time, R-02). `PlaySheet`: players and
scores open; expansions, scenario, notes, photo, location behind "More" (already the
design). Both remember the open state (D6) so a person who always rates sees rating
open.

**5.8 Channel setup.** Per-type fields open; "Quiet hours and digest" behind a
disclosure with the current value in the summary ("Immediately, no quiet hours").

### 1.4 Changes to design.md by section

- 2.9 Spacing: add "disclosure summary height 2.75 rem, 1 rem padding, caret 16 px".
- 3.7 Settings and admin: add rules D2, D4, D5, D14 as page anatomy ("H1, lead,
  attention block, open set, disclosures").
- 6.1 to 6.14: apply the open/disclosure/removed lists in 1.3 (above) as amendments.
- 7.2 Add to `ui`: `Disclosure` (APG button + panel, remembers state, `open` prop,
  status slot) and `DisclosureGroup` (Expand all / Collapse all); `Details` is the
  native variant for static help text.
- 8 Accessibility: add 2.4.11 (already), 2.5.8 wording "24 x 24 CSS px minimum,
  spacing exception documented per component", 3.3.7 and 3.2.6 as listed in A-08.
- 9 Phase 0: add `Disclosure` and `DisclosureGroup` to the component list; Phase 4:
  add the D16 metrics.

### 1.5 Owner decisions (section 1)

- **OD-01** Density budget numbers (D2): 5 inputs / 7 rows open per page. Default: as
  stated; may be loosened to 7 / 10 on desktop-only admin pages. **Approved as defaulted** (owner review 2026-11-06, DEC-172). WCAG 2.2 AA is a requirement everywhere together with this budget (DEC-171).
- **OD-02** Remembered disclosure state per user (D6) vs always closed. Default:
  remembered (localStorage, no server round trip). **Approved as defaulted** (owner review 2026-11-06, DEC-172).
- **OD-03** Home: Recent activity shortened to 5 rows with a link. Default: yes. **Approved as defaulted** (owner review 2026-11-06, DEC-172).
- **OD-04** Library: one "Filters" button instead of four filter buttons in the open
  row. Default: yes. **Approved as defaulted** (owner review 2026-11-06, DEC-172).

---

## 2. Encryption of secrets at rest

Owner decision: yes, encrypt. Scope: data-source keys entered in the UI, notification
channel tokens and webhook secrets (DEC-140 reversed), OIDC client secrets entered in
the UI, TOTP secrets. Current state in the code: `credentials.totp_secret` is a plain
`bytea`; channel secrets sit in the channel's JSON config in Postgres (DEC-140); OIDC
client secrets and connector keys come from the environment only (`config::Secret`,
DEC-061); sessions, invites and integration tokens are stored as blake3 hashes
(DEC-067, DEC-022, DEC-151).

### 2.1 Findings

**A-10. OWASP Cryptographic Storage: AES with a 256-bit key in an authenticated mode
(GCM first), keys stored apart from data, envelope encryption with a KEK that wraps a
DEK, rotation by re-encryption or by key ids, old keys kept for backups, passwords
hashed not encrypted.** Quotes: "For symmetric encryption AES with a key that's at
least 128 bits (ideally 256 bits) and a secure mode should be used as the preferred
algorithm."; "The most commonly used authenticated modes are GCM and CCM, which should
be used as a first preference."; "Where possible, encryption keys should be stored in
a separate location from encrypted data." ("if the data is stored in a database, the
keys should be stored in the filesystem"); "The Data Encryption Key (DEK) is used to
encrypt the data." / "The Key Encryption Key (KEK) is used to encrypt the DEK." / "the
KEK must be stored separately from the DEK."; rotation: "Decrypting it and
re-encrypting it with the new key" is preferred over "Marking each item with the ID of
the key"; "old keys should generally be stored for a certain period after they have
been retired, in case old backups of copies of the data need to be decrypted.";
"Passwords should not be stored using reversible encryption". It also says "Avoid
storing keys in environment variables, as these can be accidentally exposed through
functions such as phpinfo() or through the /proc/self/environ file." ChaCha20-Poly1305
is not mentioned on the page.
Evidence: https://cheatsheetseries.owasp.org/cheatsheets/Cryptographic_Storage_Cheat_Sheet.html
(CC BY-SA 4.0) [primary].

**A-11. OWASP Key Management: one purpose per key; KEK at least as strong as what it
wraps; integrity-protected key storage; AES-GCM with random 96-bit IVs limited to
2^32 operations per key; KEK retirement by re-wrapping DEKs; manual rotations
logged.** Quotes: "a single key should be used for only one purpose"; "when using
Advanced Encryption Standard in Galois/Counter Mode (AES-GCM) with random 96-bit
initialization vectors (IVs), limit each key to at most 2^32 (4,294,967,296)
encryption operations across all devices sharing that key to limit IV-collision risk
(NIST SP 800-38D 8.3)"; "To retire a Key Encryption Key (KEK), re-wrap stored Data
Encryption Keys (DEKs) under the replacement KEK before destroying the old one.";
"Rewrapping leaves the DEKs unchanged"; "Data that has been encrypted with lost
cryptographic keys will never be recovered."; cryptoperiod for symmetric data keys
"up to 2 years" of originator usage for low-volume applications (citing NIST SP
800-57 Part 1 Rev 5, 5.3.6).
Evidence: https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html
(CC BY-SA 4.0) [primary]; NIST SP 800-38D (Nov 2007, revision announced 2024-03-06)
https://csrc.nist.gov/pubs/sp/800/38/d/final and NIST SP 800-57 Part 1 Rev 5 (May
2020) https://csrc.nist.gov/pubs/sp/800/57/pt1/r5/final [primary; landing pages read,
the section text is quoted through OWASP].

**A-12. OWASP Secrets Management: rotate, audit who used which secret when, zero
memory after use, never log secrets, back up secrets encrypted and test restores,
environment variables are the least preferred injection method.** Quotes (verbatim
part): "Rotating certain keys, such as encryption keys, might trigger full or partial
data re-encryption."; "User credentials are excluded from regular rotation."; audit at
minimum who requested a secret, when it was used, when it was updated and by whom;
"After a secret has been used, the memory it occupied should be zeroed out". From the
sheet's later sections (read through a summary, so not quoted): secrets must never be
logged; AES-256-GCM is named for confidentiality plus integrity; keys are not stored
next to the secrets they encrypt unless themselves encrypted; environment variables
"may be included in logs or system dumps" and are the fallback when file or
orchestrator injection is impossible.
Evidence: https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html
(CC BY-SA 4.0) [primary, partly summarised].

**A-13. NIST SP 800-63B on OTP and password storage.** 5.1.4.2: "the symmetric keys
used by authenticators are also present in the verifier, and SHALL be strongly
protected against compromise." (no mechanism prescribed). 5.1.1.2: "Memorized secrets
SHALL be salted and hashed using a suitable one-way key derivation function." 5.1.2.2:
look-up secrets (recovery codes) "SHALL be stored in a form that is resistant to
offline attacks" (hashed; salted KDF below 112 bits of entropy). The OWASP MFA sheet
has no server-side seed storage rule; it recommends recovery codes at enrolment.
Evidence: https://pages.nist.gov/800-63-3/sp800-63b.html (June 2017, updated
2020-03-02; superseded by SP 800-63-4 on 2025-08-01, not re-read for this addendum);
https://cheatsheetseries.owasp.org/cheatsheets/Multifactor_Authentication_Cheat_Sheet.html
[primary].

**A-14. ChaCha20-Poly1305 (RFC 8439) and XChaCha20-Poly1305 (expired draft).** RFC
8439: 256-bit key, "A 96-bit nonce -- different for each invocation with the same
key", 128-bit tag; "The most important security consideration in implementing this
document is the uniqueness of the nonce"; the nonce "MUST NOT be randomly generated"
(2.6) because 96 bits are too few for random use under a long-lived key. XChaCha
(draft-irtf-cfrg-xchacha-03, expired 2020, "not endorsed by the IETF"): 192-bit nonce;
"users can safely generate a random 192-bit nonce for each message and not worry
about nonce-reuse vulnerabilities" (2^80 messages at 2^-32 collision risk).
Evidence: https://www.rfc-editor.org/rfc/rfc8439 (Informational, June 2018);
https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-xchacha-03 [primary].

**A-15. Crate licences (crates.io API, 2026-11-06), all permissive, all RustCrypto
except secrecy.**

| Crate | Version | Licence (as published) | Repository |
|---|---|---|---|
| aes-gcm | 0.11.1 (2026-08-21) | Apache-2.0 OR MIT | https://github.com/RustCrypto/AEADs |
| chacha20poly1305 | 0.11.0 (2026-06-28) | Apache-2.0 OR MIT | https://github.com/RustCrypto/AEADs |
| aead | 0.6.1 (2026-06-17) | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| hkdf | 0.13.0 (2026-03-30), already in the lock file | MIT OR Apache-2.0 | https://github.com/RustCrypto/KDFs/ |
| zeroize | 1.9.1 (2026-10-06), already in the lock file | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| secrecy | 0.10.3 (2024-10-09) | Apache-2.0 OR MIT | https://github.com/iqlusioninc/crates/tree/main/secrecy |

Evidence: https://crates.io/api/v1/crates/aes-gcm ,
https://crates.io/api/v1/crates/chacha20poly1305 , https://crates.io/api/v1/crates/aead ,
https://crates.io/api/v1/crates/hkdf , https://crates.io/api/v1/crates/zeroize ,
https://crates.io/api/v1/crates/secrecy [primary]. `cargo-deny` already allows MIT and
Apache-2.0 (KICKOFF section 4). No GPL-family crate is involved.

**A-16. 12-factor: config in the environment.** "The twelve-factor app stores config in
environment variables"; config is "everything that is likely to vary between deploys",
including "Credentials to external services"; "unlike config files, there is little
chance of them being checked into the code repo accidentally". This conflicts with
OWASP's "Avoid storing keys in environment variables" (A-10, A-12); the design below
resolves it by accepting both a file and an env var for the key, and by documenting
the trade-off.
Evidence: https://12factor.net/config (Adam Wiggins, last updated 2017) [primary].

### 2.2 Design

**Threat model (what encryption buys).** A database dump, a backup file, a read-only
SQL injection or a leaked replica must not yield usable third-party credentials, TOTP
seeds or OIDC client secrets. It does not protect against an attacker who controls the
running process (they can call decrypt) or the host (they can read the key file). It
also makes "the secret is never shown again" true in the database, not only in the UI.

**Key hierarchy (envelope encryption).**

```
KEK (key-encryption key), 256 bit, versioned, from the operator, never in the database
  |
  +-- wraps --> DEK (data-encryption key), 256 bit, random, one per secret row,
                stored wrapped next to the ciphertext
                  |
                  +-- encrypts --> the secret value (AEAD with associated data)
```

One DEK per row is the classic envelope shape (A-10, A-11): rotating the KEK re-wraps
DEKs only ("Rewrapping leaves the DEKs unchanged"), so a KEK rotation never touches the
secret ciphertext and never needs the plaintext in memory longer than one wrap call.
Rotating a DEK (on a suspected compromise of the data, not of the key) re-encrypts
that row. Secret counts are tiny (tens to a few thousand rows), so neither job is
expensive.

**Algorithm.** AES-256-GCM for both layers, random 96-bit nonce per encryption, 128-bit
tag (A-10 "GCM ... first preference"; NIST SP 800-38D; A-11). The 2^32-operations
bound per key is irrelevant at our volume (each DEK encrypts one value a handful of
times over its life; the KEK wraps at most the number of secret rows), but the
implementation still counts KEK operations per version and warns in the overview at
2^31. Alternative: XChaCha20-Poly1305 with a random 192-bit nonce (A-14) removes the
nonce bound entirely and has no hardware dependency, but its specification is an
expired draft, which weighs against it for a product that may be audited by operators.
Decision OD-05: AES-256-GCM by default, the AEAD is behind one trait so the alternative
is a one-line swap and a format version bump. Crates: `aes-gcm` (and `aead`), `hkdf`,
`zeroize` (A-15).

**Associated data (AAD).** Every encryption binds the ciphertext to where it belongs
so a ciphertext copied into another row, column or purpose fails to decrypt (A-11
integrity, OWASP "one purpose per key"). AAD is the UTF-8 string
`"<format>|<purpose>|<row id>|<field>"`, for example
`"sb1|connector_credential|018f...uuid|secret"` (the format tag is the storage
format version, not the product name; see below for the purposes). The wrapped DEK uses
the same AAD plus `"|dek"`. Row ids are UUIDv7 primary keys, so the AAD is known at
insert time; for a table whose row id is created in the same statement the code
generates the id first (already the pattern for UUIDv7 ids).

Purposes (one per field that holds a secret; the list is the registry the rotation job
walks):

| Purpose | Table.field | Notes |
|---|---|---|
| `connector_credential` | `connector_credentials.secret` (new, design.md 4.2) | instance and per-user rows; the JSON of one or more fields (key, secret, PIN) encrypted as one value |
| `channel_secret` | `notification_channels.secret` (new column; moved out of the JSON config) | tokens, SMTP password, webhook signing secret (HMAC needs the plaintext, so it cannot be hashed) |
| `oidc_client_secret` | `oidc_providers.client_secret` (new table, section 4) | providers configured in the UI |
| `smtp_password` | `instance_settings.smtp_password` (section 3) | when SMTP is configured in the UI |
| `totp_secret` | `credentials.totp_secret` | migrated in place: a new `totp_secret_enc` column, backfill job, old column dropped in a later migration |
| `cookie_key` | derived, not stored | HKDF from the KEK with info `"oidc-pending-cookie"` to sign the OIDC pending-login cookie (today `PendingLogin` is stored unsigned in the cookie, so a party who can set cookies for the host could fix `state`; signing closes that) |

**Storage format.** One `bytea` column per secret field, self-describing:

```
byte 0        format version (0x01)
bytes 1-2     KEK version (u16, big-endian)
bytes 3-14    DEK wrap nonce (12)
bytes 15-62   wrapped DEK (32 + 16 tag)
bytes 63-74   data nonce (12)
bytes 75-     ciphertext + 16-byte tag
```

Fixed offsets keep decryption allocation-free and make `kek_version` queryable with a
`substring` for the "n secrets still on v1" count without decrypting anything.

**Where the KEK comes from (12-factor vs OWASP, A-10, A-12, A-16).** Two sources,
first match wins:

1. `APP__SECRETS__KEY_FILE` (default `<data_dir>/secrets.key`, mode 0600): a text file
   with one key per line, `v<n>:<base64 32 bytes>`, newest first. This is the
   recommended path (OWASP: keys on the filesystem, apart from the database; docker and
   podman secrets mount as files).
2. `APP__SECRETS__KEYS`: the same lines joined with `,`, for operators who insist on
   environment variables (12-factor). The docs state the trade-off in one sentence.

The first entry is the active version for new encryptions; all entries are available
for decryption. The key never enters the database, logs or the API; `Debug` prints
`***` as for `config::Secret` (DEC-061); the in-memory copy is a `zeroize`d buffer
(A-12).

**First start, missing key, rotated key.**

- First start with no key file and no env var: generate `v1`, write the file with
  0600, log one line, and show it in the wizard's "Done" checklist ("Back up
  `secrets.key` together with the database"). The alternative, refusing to start, puts
  a crypto decision in front of a family install before anything works. OD-06.
- Key file absent at start but the database already holds encrypted rows (detected by
  `select 1 from ... where secret is not null limit 1` per registered purpose): refuse
  to start with a message that names the file path and the env var. There is nothing
  an admin could do in the UI, and starting without the key would silently break
  sign-in for TOTP users.
- Key present but a row's `kek_version` is not among the loaded versions: that row
  is `locked`. The owning feature degrades: a locked data-source key shows "Key
  unavailable (secrets key v1 is not loaded)" and the connector is skipped like
  `invalid` (design.md 4.4); a locked channel pauses with the same reason in the
  delivery log; a locked OIDC provider is hidden from the sign-in page and its status
  row says why; a locked TOTP secret makes the second step fail with "Your
  authenticator cannot be checked right now; ask your admin" and the admin overview
  shows the count. Locked rows are never deleted automatically. The admin either
  restores the old key version (rows unlock on the next read) or replaces the secret.
- A wrong key (right version number, wrong bytes) is indistinguishable from a
  tampered row: the AEAD tag fails. Reported as the same `locked` state with the
  wording "cannot be decrypted with key v1"; the health row suggests checking the key
  file.

**Rotation.** Admin > Instance > Secrets (mockup `admin.html`, annotation 7) and
`cli secrets status | rotate | prune`:

1. Operator adds `v2` as the first line of the key file (or env) and restarts (or sends
   SIGHUP; the key set is re-read on reload).
2. The overview shows "Key v2 active; n of m secrets still on v1" and a "Re-encrypt
   now" action; the `background_refresh` lane also schedules the job nightly while any
   row is on an old version. The job walks the purpose registry in batches of 100,
   unwraps each DEK with the old KEK, wraps it with the active one, writes the row with
   an optimistic check on the old bytes, and logs counts (never values) to the audit
   log. It is idempotent and resumable (rows already on the active version are skipped).
3. When no row references `v1`, "Remove key v1" becomes available: it only tells the
   operator it is safe to delete the line; the app never edits the key file. Old keys
   stay in the file for as long as backups that contain the old ciphertext are kept
   (A-10).
4. DEK rotation ("Re-encrypt values") exists as a separate action for a suspected data
   compromise; it re-encrypts every value with a fresh DEK and is otherwise the same job.

Cryptoperiod: the docs recommend rotating the KEK every 2 years or on staff change (A-11)
and show the three commands; nothing enforces it beyond a soft reminder in the
overview at 2 years since `v<active>` was first seen (date recorded in
`instance_settings`).

**Backup and restore.** The operations page in the Antora docs ("backup") gains: the
database dump plus `secrets.key` are one backup unit; a dump restored without the key
yields `locked` rows for every secret; restoring an older dump with a newer key file
works as long as the older versions are still listed; `cli secrets status` prints the
version histogram and the count of locked rows so a restore can be verified before
opening the instance. Export (DEC-118 generic format) never contains secrets.

**What stays hashed (not encrypted).** Anything the app only needs to compare:
password hashes (argon2id, DEC-068; A-13 "SHALL be salted and hashed"), session tokens
(blake3, DEC-067), invite tokens (DEC-022), share-link tokens, integration bearer
tokens (DEC-151), and new TOTP recovery codes (8 codes, 10 characters each from a
32-symbol alphabet = 50 bits, so salted argon2id per code as A-13 requires below 112
bits; shown once at enrolment, each usable once). Hashing stays the stronger choice
there because a dump then yields nothing reusable at all, which encryption cannot
offer for a value the app must present to a third party.

**Code shape.** A new `secrets` crate (no workspace deps except `domain` for ids; in
the D-3 dependency rule it sits beside `auth`): `SecretBox` (the encoded bytes),
`KeyRing` (loaded versions, active version, `encrypt(purpose, row, field, plaintext)`,
`decrypt(...)`), `Locked` error, a `Purpose` registry trait that each owning crate
implements for the rotation job, and a `testkit` fake with a fixed key. `config` gains
`[secrets] key_file`, `keys` (both `Secret`), the loader with the version parsing and
the refusal rule above. Tests: round trip; AAD mismatch fails; unknown version gives
`Locked`; rotation job converges and is idempotent; a tampered byte fails; the
`Debug` of every type prints no key material; a golden-vector test for the format so a
future XChaCha switch is a new format byte, not a silent change.

### 2.3 UI implications

- **Write-only fields.** A stored secret renders as a read-only line "Set 12 Oct 2026
  by robin" with a Replace button; the field never contains the value or a run of
  masking characters that could be mistaken for it (mockup `admin.html`, annotation 7).
  Replace opens an empty password-type input with `autocomplete=off`, Test (where the
  purpose has a test) and Save; Cancel keeps the old value. The API contract changes
  from DEC-140's "`********` means unchanged" to "field absent or `null` means
  unchanged, a string means replace"; `********` is no longer special.
- **Show once.** Generated secrets (signed-webhook secret, integration token, recovery
  codes) are shown once with Copy, as today (DEC-142, 5.9), and the page says so
  before generating.
- **Status words.** The `Credential` state of design.md 4.2 gains `locked`; the
  source table, channel cards, provider table and the Account > Sign-in disclosure show
  it with the reason sentence from 2.2.
- **Members.** Per-user credentials (TheTVDB PIN, Hardcover) use the same field pattern
  under Settings > Data sources; a member never sees whether the instance key is
  encrypted or locked beyond "temporarily unavailable" (design.md 4.4).
- **Audit.** Every replace, re-encryption run and key version change writes an audit
  row with purpose and key version, never the value (A-12; section 3).

### 2.4 Changes to design.md by section

- 4.2 Where keys live: replace "Encrypted with a key from the environment
  (`APP__SECRETS__KEY`), following the pattern already used for channel secrets
  (DEC-140 ...)" with a reference to this section (key file first, env second,
  envelope format, `locked` state). Add `locked` to the `Credential` states in 4.2 and
  to the table in 4.4 ("Key unavailable" for admins, "temporarily unavailable" for
  members).
- 5.8 Channel setup: "secrets as password fields 'stored encrypted, never shown
  again'" becomes true; add the Replace pattern for existing channels.
- 6.12 Account: TOTP enrolment adds recovery codes (shown once); the Sign-in disclosure
  shows "Authenticator: on (set up 3 Mar 2026)" with Reset.
- 6.13 Admin: add the Secrets disclosure (section 3 below).
- 7.2 `ui`: `CopyField` gains a "shown once" variant; new `SecretField` (write-only
  field with set-on line and Replace).
- 9 Phase 2: add the `secrets` crate, the migrations and the TOTP backfill before the
  `connector_credentials` table lands (so no connector key is ever stored plain).
- DECISIONS to add when implemented: reverse DEC-140; record the algorithm, format and
  crates with the URLs in A-15.

### 2.5 Owner decisions (section 2)

- **OD-05** AEAD: AES-256-GCM (NIST/OWASP named, hardware accelerated) or
  XChaCha20-Poly1305 (random-nonce safe, expired draft). Default: AES-256-GCM. **Approved as defaulted** (owner review 2026-11-06, DEC-172). Encrypting secrets at rest (this whole section) is **decided** (owner review 2026-11-06, DEC-171).
- **OD-06** First start without a key: auto-generate `secrets.key` in the data
  directory (and tell the admin to back it up) or refuse to start. Default:
  auto-generate. **Approved as defaulted** (owner review 2026-11-06, DEC-172).
- **OD-07** Key source precedence: file first, env second, both documented. Default:
  as stated. **Approved as defaulted** (owner review 2026-11-06, DEC-172).
- **OD-08** TOTP recovery codes added with the migration (hashed, shown once).
  Default: yes. **Approved as defaulted** (owner review 2026-11-06, DEC-172).
- **OD-09** Soft reminder to rotate the KEK after 2 years. Default: yes, overview
  line only, no enforcement. **Approved as defaulted** (owner review 2026-11-06, DEC-172).

---

## 3. Instance admin settings

Owner requirement: a proper admin area (instance settings: hosting mode, sign-up
policy, data sources and keys, OIDC providers, SMTP, storage budget, users and roles,
invites, audit log of admin actions, background jobs and health), with a clear split
between database/UI and config file/environment, no lock-out, and an uncluttered
information architecture.

### 3.1 Findings

The evidence is A-16 (12-factor), A-10/A-12 (secrets handling and auditing), R-50
(patterns in MIT/Apache self-hosted apps: env preset shown read-only, run-once
installer, test before save, admin-only credentials) and section 1 (density). Two
additional observations from the code: `AppConfig` already validates and renders a
configuration reference (config crate, KICKOFF M05), and `SignupMode` exists in both
`config` and `auth` with the invite default (DEC-070), so the admin UI changes a stored
value that the server reads at request time, not a file.

### 3.2 Design decisions: what lives where

Rule: **environment (and the config file) holds what the process needs before it can
talk to the database, and what differs per deployment; the database holds policy and
content that an admin changes while the instance runs.** Env always wins over the
database; the UI shows an env-provided value read-only with a "from environment" badge
and the variable name (R-50, A-16), and shows a shadowed database value as "stored
value ignored while `APP__...` is set" so nobody wonders why a save had no effect.

| Setting | Lives in | Why |
|---|---|---|
| database URL, listen address, `server.public_url`, brand dir, telemetry, storage backend and S3 credentials, Meilisearch URL and master key, secrets key, trusted proxies, `--role` | env / file | needed before the DB is reachable, or per deployment; `public_url` is also a security boundary (redirect URIs, cookies) that should not be editable from a web session |
| `instance.hosting` (home/hosted), sign-up mode, invites, roles, per-source licence record | DB | policy, changed by admins at runtime (design.md 4.1) |
| data-source keys and per-user credentials | DB (encrypted), env override | section 2 |
| OIDC providers (several), SSO mode, group-to-role rules | DB (encrypted secret), env override for one provider as today (`auth.oidc`) | section 4; env keeps the headless path |
| SMTP (host, port, security, user, password, from) | DB (encrypted password), env override as today (`[mail]`) | runtime, testable from the UI |
| storage budget numbers (`max_total_bytes`, watermarks, variants) | DB, editable in the admin UI; env / file override stays | owner ruling OD-10 (owner review 2026-11-06, DEC-172): editable in the UI; a value set in env shows read-only with the variable name |
| session days, cookie `secure` | DB, editable in the admin UI; env / file override stays | owner ruling OD-11 (owner review 2026-11-06, DEC-172): same env rule; see 3.3 for the lock-out guards |
| TOTP issuer | env / file (as today) | rarely changed |
| notification instance defaults and locks, template overrides | DB | content |
| audit log, job state, health | DB (read-only in the UI) | records |

An `instance_settings` table (single row, versioned with HLC like other LWW fields,
D-12) holds the DB-side values; `AppConfig` stays the env/file side; a `Settings`
service merges them per request with provenance (`Env`, `Db`, `Default`) so every
screen can render the badge.

### 3.3 Lock-out prevention

- The last account with the admin role cannot be demoted, disabled or deleted (server
  check, not UI only); a group-to-role rule (section 4) that would remove the acting
  admin's role is refused with "This rule would remove your own admin role".
- Changing sign-in settings (SSO mode, provider edits, sign-up mode) requires a fresh
  authentication within the last 10 minutes (password or provider re-auth), like a
  password change; A-08 3.3.7 allows re-entry "required to ensure the security".
- Enabling SSO-only (section 4) requires at least one admin with a password and an
  enrolled authenticator, and `/sign-in/local` always exists.
- Session length and cookie security are editable in the admin UI (OD-11). Changing cookie
  security (`secure`) applies from the **next session**, so the current admin session keeps
  working; the UI warns when `server.public_url` is not https before `secure` can be turned
  on (a `secure` cookie over plain http would never be sent back and would lock everyone
  out). A value set by env is read-only and shows the variable name.
- The acting admin's own session is never revoked by their own settings change; role
  changes to oneself are refused.
- CLI escape hatches that need shell access, not a browser: `cli admin promote
  <username>`, `cli admin reset-password <username>` (prints a one-time link),
  `cli admin disable-sso-only`, `cli secrets status`. They write audit rows with actor
  `cli`.
- Setup wizard lock (design.md 4.3) unchanged; `cli setup reopen` exists for a wiped
  admin account.

### 3.4 Information architecture (mockup `admin.html`)

Settings keeps one side nav with two groups ("You", "Instance (admin)"); the admin
group has eight entries, each a page with the anatomy "H1, one-sentence lead, attention
block (only when something needs attention), open set, disclosures" (D2, D5, D14):

| Entry | Open by default | Behind disclosure | Notes |
|---|---|---|---|
| Overview | attention block; 8 health rows (database, search, storage, jobs, mail, data sources, identity provider, secrets at rest), each dot + word + one line + one action; Recent admin actions (3); Brand card (read-only) | row detail (last error, timings) | replaces 6.13 "Overview" and "Branding" |
| Instance | hosting mode, sign-up mode, public address (env, read-only) | Mail (SMTP, Save and send a test), Secrets at rest, Advanced (session length and cookie security, editable, env value read-only with the variable name; search backend and image variants as before) | mockup annotations 4-8 |
| Sign-in and SSO | providers table (name, issuer, linked accounts, status), "Add a provider" | per-provider detail (client id, write-only secret, scopes, Test = discovery + logout capabilities), SSO mode radios, account rules (auto-create, group-to-role) | section 4 |
| Users | filter, table (person, role, signs in with, last seen), row menu, Invite | invites list ("Open invites (2)" disclosure above the table), disabled users via the filter chip | sign-up policy is on Instance, not here (one place per setting) |
| Data sources | attention block, table (source, kinds, status, action) | row expands to the source card | design.md 6.10 trimmed as in 1.3 |
| Notification defaults | rules matrix for instance defaults | template overrides | design.md 6.13 |
| Storage and jobs | storage: budget bar, tiers, "Run eviction now"; jobs: lanes with queued/running counts, failed jobs (retry, discard), cron list with next run | S3 daily counters, job history | combines two rarely visited pages into one |
| Audit log | table (when, who, what, detail), filter by actor and kind, Export CSV | none | new |

Nothing in the admin group is reachable for members; the "Admin" item in the account
menu links to Overview. Mobile: the side nav becomes a horizontal scroller (existing
rule), tables become stacked cards with the same four fields.

**Audit log of admin actions.** Table `admin_audit` (id UUIDv7, at, actor user id or
`system`/`cli`, action enum, target type and id, detail JSON without secret values,
client address, request id); append-only (no update or delete grants for the app role
beyond insert; retention job deletes rows older than the configured period, default 365
days); actions: setting changed (old and new value, except secrets), secret replaced
(purpose, key version), re-encryption run, provider added/changed/disabled, SSO mode
changed, role changed, user disabled/deleted, invite created/revoked, local
break-glass sign-in, wizard completed, eviction run. Each row is written in the same
transaction as the change (outbox pattern already exists). A-12 lists exactly these as
the minimum audit set for secrets.

**Background jobs and health.** Health probes already exist for the overview rows
(design.md 3.7); add: identity provider (discovery reachable, cached age, last
callback result), secrets (active version, rows per version, locked count), mail (last
test result). Probes run on the `background_refresh` lane every 60 s and are read
from a table, never computed in the request (pages never wait on third parties,
KICKOFF mission).

### 3.5 Changes to design.md by section

- 3.3 Section map, Admin: replace the five items with the eight entries above.
- 3.7: add the env/DB rule and the provenance badge.
- 4.1: the hosting switch is edited on Admin > Instance (was "the admin UI edits it";
  now a named place).
- 4.3 wizard: step 4 checklist gains "Back up `secrets.key`" (OD-06) and "Connect an
  identity provider" (section 4) as optional rows.
- 6.13 Admin: replace with 3.4; Branding becomes the Overview card.
- 7.2: add `SecretField`, `ProvenanceBadge` ("from environment"), `AuditTable` (domain
  widget in `web`).
- 9 Phase 2: admin area scope widened to the eight entries; audit log and
  `instance_settings` land with the credential model, because the first secret written
  through the UI must already produce an audit row.

### 3.6 Owner decisions (section 3)

- **OD-10** Storage budget numbers: env/file only, or editable in the UI. Default was
  env/file. **Owner ruling: editable in the admin UI; the env override stays and shows
  read-only with the variable name** (owner review 2026-11-06, DEC-172).
- **OD-11** Session length and cookie security: env-only, or editable. Default was env-only.
  **Owner ruling: editable in the admin UI, same env override rule; a cookie-security change
  applies on the next session, the UI warns when the public URL is not https, and the change
  must not lock the admin out** (owner review 2026-11-06, DEC-172).
- **OD-12** Re-authentication window for sign-in-related admin changes: 10 minutes.
  Default: 10 minutes. **Still open** (owner review 2026-11-06, DEC-172): research what established
  self-hosted apps use for re-authentication windows (sudo-mode style) and propose a value
  on the next review; 10 minutes applies until then.
- **OD-13** Audit retention: 365 days, exportable. Default: 365 days. **Approved as defaulted** (owner review 2026-11-06, DEC-172).
- **OD-14** Storage and jobs as one page. Default: one page. **Approved as defaulted** (owner review 2026-11-06, DEC-172).

---

## 4. OIDC with authentik and other providers; true SSO

Owner requirement: full OIDC with authentik and other providers, several at once,
account linking and auto-provisioning, group-to-role mapping (admin), logout in all
three standard forms, and "true SSO": when the browser already has a session at the
identity provider, sign the user in without showing our login form. Plus an instance
option "SSO only / auto-redirect" with a safe break-glass local admin login.

What exists (DEC-072, `crates/auth/src/oidc.rs`, `config::OidcConfig`): one provider
from the environment, discovery cached once per process, authorization code + PKCE
S256, `state`/`nonce`/verifier kept in a 10-minute HttpOnly cookie, identities keyed
by `(issuer, subject)`, auto-create behind `auto_create`, no logout handling, no group
mapping, no linking of an existing local account, no `prompt` parameter.

### 4.1 Findings

**Licence gate first.** Before reading any provider documentation its licence was
checked (KICKOFF rule 6):

| Provider | Code licence | Docs licence | Read? |
|---|---|---|---|
| authentik | MIT (core); `authentik/enterprise/` under the proprietary "EE License" (source-available, subscription for production) | `website/` is CC BY-SA 4.0 (own LICENSE file); images all rights reserved | yes (docs 2026.8) |
| Keycloak | Apache-2.0 | Apache-2.0 (`docs/documentation` README) | yes (26.8.2) |
| Authelia | Apache-2.0 | same repo, no separate note | yes (v4.39.28) |
| Zitadel | **AGPL-3.0-only** ("This repository is licensed under the GNU Affero General Public License v3.0"; `apps/docs/` Apache-2.0, `apps/login/` MIT per LICENSING.md) | Apache-2.0 for the docs folder | **no**: the code is AGPL, so under rule 6 nothing from Zitadel was read; it is only named here |
| Pocket ID | BSD-2-Clause | website repo, no separate note | yes (docs v2.19.2) |
| Kanidm | MPL-2.0 (allowed, file-level copyleft) | same repo (book/) | yes (stable book) |
| Google | n/a (service) | Google developer docs; the page footer was not captured in the fetch, so the content licence is not quoted | yes, standard behaviour only |
| Microsoft Entra ID | n/a (service) | `MicrosoftDocs/entra-docs`: README says CC BY 4.0 for docs, MIT for code; the LICENSE file itself contains the MIT text | yes, standard behaviour only |

Evidence: https://github.com/goauthentik/authentik/blob/main/LICENSE ,
https://github.com/goauthentik/authentik/blob/main/website/LICENSE ,
https://github.com/goauthentik/authentik/blob/main/authentik/enterprise/LICENSE ,
https://github.com/keycloak/keycloak/blob/main/LICENSE.txt ,
https://github.com/authelia/authelia/blob/master/LICENSE ,
https://github.com/zitadel/zitadel/blob/main/LICENSE and
https://github.com/zitadel/zitadel/blob/main/LICENSING.md ,
https://github.com/pocket-id/pocket-id/blob/main/LICENSE ,
https://github.com/kanidm/kanidm/blob/master/LICENSE.md ,
https://github.com/MicrosoftDocs/entra-docs/blob/main/LICENSE [all primary].

**A-17. OpenID Connect Core: `prompt=none` is the silent-authentication mechanism; the
defined failures are `login_required`, `interaction_required`,
`account_selection_required`, `consent_required`.** 3.1.2.1, `prompt=none`: "The
Authorization Server MUST NOT display any authentication or consent user interface
pages. An error is returned if an End-User is not already authenticated or the Client
does not have pre-configured consent for the requested Claims ... The error code will
typically be login_required, interaction_required, or another code defined in Section
3.1.2.6 ... This can be used as a method to check for existing authentication and/or
consent." `none` may not be combined with other values. 3.1.2.3: the server "MUST
return an error if an End-User is not already Authenticated or could not be silently
Authenticated." `id_token_hint`: "When possible, an id_token_hint SHOULD be present
when prompt=none is used ... however, the server SHOULD respond successfully when
possible, even if it is not present." `max_age`: elapsed time since active
authentication; "max_age=0 is equivalent to prompt=login". `login_hint` is at the OP's
discretion. 3.1.2.6 error codes: `login_required` "The Authorization Server requires
End-User authentication."; `interaction_required` "requires End-User interaction of
some form"; `account_selection_required`; `consent_required`; all "MAY be returned
when the prompt parameter value ... is none". Errors come back on the redirect URI
with `state`, unless the redirect URI itself is invalid.
Evidence: https://openid.net/specs/openid-connect-core-1_0.html (Final, errata set 2,
2023-12-15; OIDF notice permits reproduction for implementing with attribution)
[primary].

**A-18. Only `iss` + `sub` identify a person; email, name and preferred_username
must not.** 5.7: "the only guaranteed unique identifier for a given End-User is the
combination of the iss Claim and the sub Claim." "other Claims such as email,
phone_number, preferred_username, and name MUST NOT be used as unique identifiers".
5.1: `preferred_username` "MAY be any valid JSON string including special characters
such as @, /, or whitespace"; `email_verified` true means "the OP took affirmative
steps to ensure that this e-mail address was controlled by the End-User". ID token
validation (3.1.3.7): issuer must exactly match discovery, `aud` must contain the
client id, signature with the issuer's keys, `exp`, `nonce` equal to the one sent.
Google says the same: "A Google Account can have multiple email addresses at
different points in time, but the sub value is never changed."
Evidence: Core spec as above; https://developers.google.com/identity/openid-connect/openid-connect
[primary].

**A-19. Discovery.** The document is at `<issuer>/.well-known/openid-configuration`
("If the Issuer value contains a path component, any terminating / MUST be removed
before appending"); `issuer` "MUST be identical to the iss Claim value". The logout
capabilities are advertised by the logout specs' metadata: `end_session_endpoint`
(RP-Initiated 2.1, "MUST be included ... when RP-Initiated Logout and Discovery are
supported"), `backchannel_logout_supported` / `backchannel_logout_session_supported`
(Back-Channel 2.1), `frontchannel_logout_supported` (Front-Channel 3),
`check_session_iframe` (Session Management 3.3). `prompt_values_supported` is not
part of the Discovery spec, so support for `prompt=none` cannot be read from metadata
and must be probed.
Evidence: https://openid.net/specs/openid-connect-discovery-1_0.html (Final,
2023-12-15) [primary].

**A-20. RP-initiated logout.** The RP redirects the browser to `end_session_endpoint`
with `id_token_hint` (RECOMMENDED), optional `client_id`, `post_logout_redirect_uri`
(must be pre-registered, exact match; "An id_token_hint is also RECOMMENDED when this
parameter is included") and `state`. "the OP SHOULD ask the End-User whether to log
out of the OP as well" and "MUST ask" when no valid `id_token_hint` is given. "It is up
to the RP whether to locally log out the End-User before redirecting"; the spec notes
that OP-to-RP notifications "are unreliable" and the user "might not approve the OP
logging out". Requests are idempotent.
Evidence: https://openid.net/specs/openid-connect-rpinitiated-1_0.html (Final,
2022-09-12) [primary].

**A-21. Back-channel logout is the reliable one; front-channel and the session iframe
depend on third-party cookies, which browsers block or partition.** Back-Channel: OP
POSTs `logout_token` (a signed JWT with `events` containing
`http://schemas.openid.net/event/backchannel-logout`, `sub` and/or `sid`, no `nonce`)
to the RP's registered `backchannel_logout_uri`; the RP validates it like an ID token,
clears the sessions, answers 200 (400 on failure); "the RP's back-channel logout URI
must be reachable from all the OPs used" (no NAT between OP and RP); "can be more
reliable than communication through the User Agent". Front-Channel: the OP renders
`<iframe src="frontchannel_logout_uri">`; 4.1: "the frontchannel_logout_uri might not
be able to access the RP's login state when rendered by the OP in an iframe because
the iframe is in a different origin"; "OpenID Connect Back-Channel Logout 1.0 is not
known to be affected by these developments." Session Management (check_session_iframe,
postMessage polling) 5.1: "cookies and web storage APIs may not be available in the OP
frame loaded in the RP context ... Cookie based implementations might then return
changed for every single call, resulting in infinite loops of re-authentications."
Browser state: Safari blocks all third-party cookies by default since Safari 13.1 /
iOS 13.4 (March 2020; "the first mainstream browser to fully block third-party cookies
by default"; the recommended patterns are a top-level OAuth redirect establishing a
first-party cookie, or the Storage Access API); Firefox partitions them per top-level
site by default for all desktop users ("Total Cookie Protection", June 2022); Chrome
decided in April 2025 "to maintain our current approach to offering users third-party
cookie choice" and "will not be rolling out a new standalone prompt", Incognito blocks
them by default, and in October 2025 retired most Privacy Sandbox APIs while keeping
CHIPS, FedCM and the Storage Access API. So an iframe-based silent auth works in
default Chrome today, fails in Safari and in partitioned Firefox, and is not something
an instance can rely on.
Evidence: https://openid.net/specs/openid-connect-backchannel-1_0.html (Final, errata
set 1, 2023-12-15); https://openid.net/specs/openid-connect-frontchannel-1_0.html
(Final, 2022-09-12); https://openid.net/specs/openid-connect-session-1_0.html (Final,
2022-09-12); https://webkit.org/blog/10218/full-third-party-cookie-blocking-and-more/ ;
https://blog.mozilla.org/en/products/firefox/firefox-rolls-out-total-cookie-protection-by-default-to-all-users/ ;
https://privacysandbox.google.com/blog/privacy-sandbox-next-steps ;
https://privacysandbox.google.com/blog/update-on-plans-for-privacy-sandbox-technologies
[all primary].

**A-22. OAuth 2.0 (RFC 6749), PKCE (RFC 7636) and the Security BCP (RFC 9700, BCP 240,
January 2025).** RFC 6749 4.1 steps A-E; `state` RECOMMENDED, CSRF (10.12): "The
client MUST implement CSRF protection for its redirection URI ... a value that binds
the request to the user-agent's authenticated state"; code lifetime 10 minutes,
single use; error codes on the redirect include `access_denied`, `server_error`
("a 500 ... cannot be returned to the client via an HTTP redirect") and
`temporarily_unavailable`. RFC 7636: verifier 43-128 chars, "If the client is capable
of using S256, it MUST use S256"; mismatch yields `invalid_grant`. RFC 9700: "Public
clients MUST use PKCE", "For confidential clients, the use of PKCE [RFC7636] is
RECOMMENDED", "this advice applies to all kinds of OAuth clients, including web
applications"; exact redirect URI matching; "When an OAuth client can interact with
more than one authorization server, a defense against mix-up attacks (see Section
4.4) is REQUIRED": store the issuer per request, compare with the `iss` response
parameter (RFC 9207) or the ID token `iss`, abort on mismatch; "the resource owner
password credentials grant ... MUST NOT be used"; implicit "SHOULD NOT" be used; state
"SHOULD be invalidated by the client after its first use"; if `state` carries
application data "clients MUST protect state against tampering and swapping".
Evidence: https://www.rfc-editor.org/rfc/rfc6749 ; https://www.rfc-editor.org/rfc/rfc7636 ;
https://www.rfc-editor.org/rfc/rfc9700 ; https://www.rfc-editor.org/rfc/rfc9207
[primary].

**A-23. Provider behaviour (from their own docs; only what the docs say).**

| Provider | Discovery | PKCE | Groups in claims | Logout | `prompt=none` in docs |
|---|---|---|---|---|---|
| authentik 2026.8 | `/application/o/<slug>/.well-known/openid-configuration`; issuer per slug by default | supported; public clients "should use the authorization code flow with PKCE" | default `profile` scope mapping "returns the user's group names in the groups claim"; claims in ID token only when "Include claims in id_token" is on, always at UserInfo | RP-initiated via `/application/o/<slug>/end-session/` (by default only the app session ends, authentik session stays); back-channel (POST logout_token with `sid`, triggered on logout, admin session deletion, deactivation, expiry; "preview" since 2025.8) and front-channel (iframe, user-initiated only); one "Logout URI" + "Logout Method" per provider | not mentioned |
| Keycloak 26.8.2 | `/realms/<realm>/.well-known/openid-configuration` | enforceable via client policy (`pkce-enforcer`, S256) | protocol mappers (group membership, realm roles) exist; the exact mapper text was beyond the fetched part of the guide | all four specs; docs recommend back-channel ("more reliable and secure"); RP-initiated needs `client_id` or `id_token_hint` with `post_logout_redirect_uri` | yes: "returns a login_required error if the user has not logged in"; may return `interaction_required` |
| Authelia 4.39 | `/.well-known/openid-configuration` | `pkce_challenge_method: S256` per client, `enforce_pkce` globally | `groups` scope -> `groups` claim, at UserInfo by default; claims policy can force it into the ID token | **none**: RP-Initiated, Session Management, Front- and Back-Channel all "None" in the support chart ("planned") | not mentioned |
| Pocket ID 2.19 | `/.well-known/openid-configuration` | PKCE for public clients | `groups` scope -> group names array | `/api/oidc/end-session`; client has `backchannelLogoutURL` and `logoutCallbackURLs` (API fields) | not mentioned |
| Kanidm | `/oauth2/openid/<client>/.well-known/openid-configuration` | **required** ("PKCE S256 code verification"; disable only with a `warning-insecure-...` command) | `groups`, `groups_name`, `groups_spn` scopes; custom claim maps per group | not documented (no logout spec in the feature list) | not mentioned |
| Google | `https://accounts.google.com/.well-known/openid-configuration` | S256 supported | no groups | no end_session (only token revocation) | yes: "does not display any authentication or user consent screens; it will return an error if the user is not already authenticated" |
| Microsoft Entra ID | `https://login.microsoftonline.com/<tenant>/v2.0/.well-known/openid-configuration` (`common`, `organizations`, tenant id) | yes | `groups` optional claim (object ids by default; at most 200 groups in a JWT, overage otherwise) | `/oauth2/v2.0/logout` with `post_logout_redirect_uri`, front-channel single sign-out (one URL per app); back-channel not on the page | yes: "The prompt=none parameter ... should be paired with a login_hint"; errors when "no signed-in user" or "multiple users are signed in but no hint was provided" |

Evidence: https://docs.goauthentik.io/add-secure-apps/providers/oauth2/ ,
https://docs.goauthentik.io/add-secure-apps/providers/oauth2/frontchannel_and_backchannel_logout/ ,
https://docs.goauthentik.io/add-secure-apps/providers/property-mappings/ ,
https://docs.goauthentik.io/add-secure-apps/flows-stages/flow/ (CC BY-SA 4.0);
https://www.keycloak.org/securing-apps/oidc-layers ,
https://www.keycloak.org/docs/latest/server_admin/index.html (Apache-2.0);
https://www.authelia.com/integration/openid-connect/introduction/ ,
https://www.authelia.com/configuration/identity-providers/openid-connect/clients/ ,
https://www.authelia.com/integration/openid-connect/openid-connect-1.0-claims/ ;
https://pocket-id.org/docs/setup/connect-an-app , https://pocket-id.org/docs/guides/scopes-and-claims ;
https://kanidm.github.io/kanidm/stable/integrations/oauth2.html ;
https://developers.google.com/identity/openid-connect/openid-connect ;
https://learn.microsoft.com/en-us/entra/identity-platform/v2-protocols-oidc ,
https://learn.microsoft.com/en-us/entra/identity-platform/optional-claims [all primary].
Nothing in this addendum is derived from Zitadel documentation.

### 4.2 Design decisions

**Scope.** We are a confidential web client (server-side code exchange). We implement:
discovery with caching and refresh; authorization code + PKCE S256 always (A-22, even
as a confidential client); `state` + `nonce` bound to the browser by a signed cookie;
mix-up defence with `iss` (A-22); several providers; linking; auto-provisioning rules;
group-to-role mapping; RP-initiated logout; back-channel logout; silent sign-in with
`prompt=none` via top-level redirect; SSO-only mode with a break-glass path. We do not
implement front-channel logout or the check_session iframe (A-21: third-party cookie
dependent, "unreliable" per the specs themselves); the provider table shows which
mechanisms a provider offers, so an admin sees why "sign-out everywhere" is or is not
available.

**Data model.** New table `oidc_providers` (id UUIDv7, slug for URLs, display name,
issuer, client id, client secret as a `SecretBox` (section 2), scopes (default
`openid profile email`, plus `groups` when a rule needs it), enabled, is_default,
auto_create, allowed groups claim name and values, admin groups, discovery cache JSON
with fetched-at, capabilities derived from discovery: `end_session_endpoint`,
`backchannel_logout_supported`, `code_challenge_methods_supported`,
`authorization_response_iss_parameter_supported`; created/updated by; HLC).
`oidc_identities` (existing: issuer, subject, user id) gains `provider_id`, `sid`
(from the ID token when the OP sends one), `last_id_token` (encrypted, for
`id_token_hint` at logout; dropped after logout), `linked_at`, `last_claims_hash`.
`auth_sessions` gains `provider_id` and `oidc_sid` so a back-channel logout can find
the sessions to end. The environment provider (`auth.oidc`) is loaded as a read-only
row with provenance `Env` (section 3), so one code path serves both.

**Redirect and callback.** One redirect URI per instance for all providers:
`<public_url>/api/v1/auth/oidc/callback` (RFC 9700 4.4.2.2 says distinct URIs are the
fallback, not the primary mix-up defence). The pending-login cookie holds `provider
id, issuer, state, nonce, PKCE verifier, return path, mode (silent|interactive|link)`
and is HMAC-signed with the HKDF-derived cookie key (section 2) so an attacker who can
set cookies for the host cannot fix `state` (RFC 6749 10.12 "bound to the user-agent's
authenticated state"); it is deleted after one use (RFC 9700 4.2.4). On callback: if
the provider advertises `authorization_response_iss_parameter_supported`, the `iss`
parameter must equal the stored issuer, else reject; otherwise the ID token `iss` is
compared after exchange; `state` must match; the code is exchanged with the verifier;
the ID token is validated per Core 3.1.3.7 (the `openidconnect` crate does this; the
nonce is ours). `access_denied`, `login_required` and friends are handled as in 4.4.

**Account resolution (first sign-in, linking, auto-provisioning).** In order:

1. `(issuer, sub)` known -> sign in that user (A-18). Nothing else is consulted; a
   changed email at the provider updates the local email only if `email_verified` is
   true and the admin enabled "Keep email in sync" (default off).
2. Unknown identity, and the person is currently signed in locally (they clicked
   "Link" under Settings > Account > Sign-in) -> link to the signed-in account. The
   pending cookie's `mode = link` is the only way into this branch.
3. Unknown identity, not signed in, and a local account has the same email with
   `email_verified: true` in the ID token -> **ask, never auto-link**: the "Is this your
   account?" page (mockup `signin-sso.html`, state 5) requires that account's password
   once (or, if the account has no password, an emailed one-time link when mail is set
   up; otherwise "ask your admin to link it"). Rationale: email is explicitly not an
   identifier (A-18) and an attacker who controls a provider could otherwise take over
   any local account by registering the victim's address.
4. Unknown identity, no match -> create an account if the provider's `auto_create` is
   on, or sign-up is open, or the first account (as today); the username is the
   sanitised `preferred_username` with a numeric suffix on collision, display name from
   `name`, and the person lands on Home like any new user. Otherwise: "No account for
   this sign-in. Ask your admin for an invite." (Invite links can be opened while
   signed in via a provider; accepting one links the identity to the promoted
   companion, DEC-070.)

Several providers may link to one account; each shows as a row under Settings >
Account > Sign-in with Unlink (refused if it would leave the account with no way to
sign in: no password and no other provider).

**Group and claim to role mapping.** Per provider: `groups_claim` (default `groups`,
because authentik, Authelia, Pocket ID and Kanidm all use that name; Entra emits
object ids unless configured, so the field accepts any string), `admin_groups` (list),
`allowed_groups` (list, empty = everyone), `role_source = provider | local`. When
`role_source = provider` the role is recomputed at every sign-in and back-channel
event: member of an admin group -> admin; else member; not in `allowed_groups` ->
sign-in refused with "Your account is not allowed here (group rule)". Rules that would
demote the acting admin are refused (section 3.3); a provider-managed role is badged
"via group" in Users and is not editable by hand. Claims are read from the ID token
and, when the claim is missing there (Authelia and authentik put groups at UserInfo by
default), from the UserInfo endpoint in the same request. Provider "Test" reports
whether a groups claim was seen for the testing admin, so misconfiguration is visible
before anyone is locked out.

**Logout.**

- Local sign-out (account menu) always ends our session first (A-20 lets the RP
  choose; we choose local-first so a declined OP logout still leaves the person signed
  out here). If the session came from a provider with `end_session_endpoint`, the
  browser is then redirected there with `id_token_hint`, `client_id`,
  `post_logout_redirect_uri=<public_url>/signed-out` (registered at the provider; the
  provider page in the UI prints the exact URIs to register) and `state`. The OP may
  ask "Log out of authentik too?"; whatever the answer, `/signed-out` renders "You are
  signed out" with a "Sign in" button that does not auto-redirect (so SSO-only mode
  does not bounce the person straight back in; see 4.4).
- Back-channel endpoint `POST <public_url>/api/v1/auth/oidc/backchannel-logout`
  (public, unauthenticated, rate-limited, `Cache-Control: no-store`): validates the
  logout token (signature with the provider's keys, `iss`, `aud`, `iat`, `exp`,
  `events` member present, no `nonce`, `jti` replay cache for 10 minutes), then ends
  sessions by `sid` if present, else all sessions of the `(iss, sub)` identity, answers
  200, writes an audit row ("provider ended 2 sessions"). The provider page shows the
  URL to register and the state "Back-channel: registered / not supported by this
  provider".
- Front-channel: not implemented; the provider page says "Front-channel logout is not
  used (it depends on third-party cookies, which Safari and Firefox block or partition)".
  If a provider only offers front-channel (Entra per its docs), sign-out at the
  provider does not reach us; our sliding session then ends on its own after
  `session_days`, and the admin can shorten `session_days` for such instances.

**True SSO: silent sign-in.** Mechanics as the standard defines them (A-17, A-21):

1. The RP sends an ordinary authentication request with `prompt=none` as a top-level
   redirect (not an iframe). The browser carries its first-party cookies for the OP, so
   this works in every browser regardless of third-party cookie policy (WebKit's own
   recommendation, A-21 "Option 1").
2. If the OP has a session, it answers with a code on our callback; the person never
   sees a form. Round trip: two redirects, typically well under a second on a LAN.
3. If not, the OP answers `error=login_required` (or `interaction_required`,
   `account_selection_required`, `consent_required`) on our callback with our `state`.
   We then send a second request without `prompt` (interactive), or show our sign-in
   page, depending on the mode (4.4). The spec says the OP "SHOULD respond
   successfully when possible" without `id_token_hint`; we add `id_token_hint` when
   we have a stored ID token for the browser (cookie `oidc_hint` with the identity id,
   not the token) and `login_hint` for Entra, whose docs want it paired with
   `prompt=none`.
4. Loop guard: a cookie `oidc_silent_attempt` (5 minutes) means at most one silent
   attempt per browser per 5 minutes; a silent failure within that window goes straight
   to the interactive path. `server_error` / `temporarily_unavailable` / network
   failure during discovery or exchange are the outage path (4.4).
5. Not used: `check_session_iframe` polling (A-21: cookie-dependent, "infinite loops").
   Instead the page re-checks on the next navigation after the local session has
   expired, and back-channel logout handles the OP-initiated case.

Provider support for `prompt=none` is not discoverable from metadata (A-19); Google,
Entra and Keycloak document it, authentik, Authelia, Pocket ID and Kanidm do not say.
The provider "Test" therefore performs one `prompt=none` request as the testing admin
and records the outcome (`silent_ok`, `silent_login_required`, `silent_unsupported`
when the OP returns `invalid_request` or shows a page, detected by a non-redirect
response); SSO auto-redirect uses the silent step only when the probe succeeded, and
otherwise goes straight to the interactive request (which is still "no form of ours",
just the OP's form when the OP has no session).

### 4.3 Sign-in modes (instance setting, Admin > Sign-in and SSO)

| Mode | Signed-out visit to any page | Sign-in page | Password sign-in |
|---|---|---|---|
| `page` (default) | redirect to `/sign-in?next=` | provider buttons first ("Continue with authentik"), password form behind a disclosure when at least one provider exists; create-account link when allowed | everyone |
| `auto` ("Go straight to the default provider") | 302 to the default provider's authorization endpoint (silent first when probed ok, then interactive); `/sign-in` itself does the same | only rendered when the provider returns an error (states 2 and 3 in the mockup) | admins only, at `/sign-in/local`; members get "Sign in through authentik" |

Preconditions for enabling `auto`, checked server-side: a default provider with a
successful Test in the last 24 hours, at least one admin with a password and an
enrolled authenticator, and a fresh re-authentication of the acting admin (section 3.3).
The UI states the consequence in the radio label (mockup `admin.html`, annotation 10).

**Break-glass local admin login.** `/sign-in/local` always exists and is never
redirected, is not linked from the normal flow (only from the outage page and the
docs), and in `auto` mode accepts only accounts with the admin role. A **password is enough**
(owner ruling OD-17, owner review 2026-11-06, DEC-172); the TOTP step is used if the account has TOTP enrolled, but
is not mandatory. It uses the same throttle (DEC-155) and every
successful use writes an audit row and sends an admin notification ("Local sign-in
used by robin from 192.0.2.10"). CLI fallbacks from section 3.3 cover a lost
authenticator (`cli admin reset-totp <user>`).

### 4.4 Behaviour by situation

| Situation | `page` mode | `auto` mode |
|---|---|---|
| First visit, OP session exists | sign-in page; "Continue with X" -> back signed in, no OP form (OP has a session) | redirect, silent success -> Home, no form at all |
| First visit, no OP session | sign-in page; "Continue with X" -> OP form -> back | redirect, silent `login_required` -> interactive request -> OP form -> back; the person sees only the OP's form |
| Session expired (sliding 30 days passed, or ended by back-channel) | next navigation -> sign-in page with `next=`; the message "Your session ended" only when the previous cookie was present | next navigation -> silent attempt with `next=` preserved -> back to the same page; the "Your session ended" card (mockup state 6) only if the silent attempt fails and the interactive one is needed |
| OP returns `access_denied` or the group rule refuses | sign-in page with "X did not allow this sign-in" / "Your account is not allowed here (group rule)" and the password disclosure open | the same page, with the break-glass link in the footer |
| OP outage (discovery or token endpoint unreachable, 5xx, `server_error`, `temporarily_unavailable`) | sign-in page with the warning alert (mockup state 3) and the password form open | outage page (state 3): "X is not answering", Try again (capped: after 3 failures in 10 minutes the page stops auto-retrying), break-glass link; the health row on Overview turns red; members who already have a session keep working, because our session does not depend on the OP |
| OP down while signed in | nothing changes: our session is local; only the next sign-in needs the OP | same |
| Sign-out | local session ended, then OP end-session if offered, then `/signed-out` (no auto-redirect) | same; `/signed-out` is the one page that does not bounce to the OP |
| Back-channel logout token | sessions with that `sid` (or all for the identity) end; the next request from such a browser is treated as "session expired" | same |

### 4.5 UI (mockup `signin-sso.html`)

- Provider buttons are labelled with the admin's display name for the provider
  ("Continue with authentik"), never a logo we do not have the right to use (KICKOFF
  rule 11: service logos only from the vendored sources, which do not include these).
- The password disclosure follows D7 (section 1): `details`/`summary` is sufficient
  because the content is static; when no provider exists the form is open and there is
  no summary.
- State 2 (auto-redirect interstitial) is rendered only as the body of the 302
  response for user agents that do not follow redirects, plus the `noscript`-safe
  link; it has `role=status`, no animation under reduced motion.
- Microcopy (EN / DE): "Continue with {provider}" / "Weiter mit {provider}";
  "Taking you to {provider}..." / "Du wirst zu {provider} weitergeleitet..."; "{provider}
  is not answering" / "{provider} antwortet nicht"; "Admins: sign in with a local
  account" / "Admins: mit einem lokalen Konto anmelden"; "Is this your account?" /
  "Ist das dein Konto?"; "Link and sign in" / "Verknüpfen und anmelden"; "Your account
  is not allowed here (group rule)" / "Dein Konto ist hier nicht zugelassen
  (Gruppenregel)"; "You are signed out" / "Du bist abgemeldet".

### 4.6 Changes to design.md by section

- 3.2 Navigation: the account menu's "Sign out" runs the local-first logout and lands
  on `/signed-out`.
- 3.3 Section map, Settings > account: "OIDC link" becomes "Linked sign-ins" (several).
- 5.2 Sign-in: add the provider-first layout, the password disclosure, states 2-6, and
  the `auto` mode; "Closed sign-up renders an explanation page" stays.
- 6.12 Account: Sign-in disclosure lists password, authenticator, linked providers
  with Link/Unlink and the no-lock-out rule.
- 6.13 Admin: Sign-in and SSO page per section 3.4 with provider table, detail, Test
  outcomes (discovery, logout capabilities, silent probe, groups claim seen), mode
  radios, account rules.
- 6.14 Auth pages: add `/sign-in/local`, `/signed-out`, the outage and linking pages.
- 7.3 `web` widgets: `ProviderButton`, `ProviderTable`, `LinkAccountCard`.
- 8 Accessibility: the auto-redirect body is `role=status`; outage and denial pages use
  `role=alert` for the reason; all new pages keep one H1 and the brand mark.
- 9 Phase 2: `oidc_providers` table, multi-provider `Oidc` with per-provider discovery
  cache, signed pending cookie, `iss` check, linking page, group rules, logout
  endpoints, silent probe, modes; e2e with an HS256 test issuer (DEC-072 names this as
  the missing test) covering: silent success, `login_required`, back-channel logout,
  denied group, outage, break-glass.
- DECISIONS to add when implemented: supersede DEC-072 (several providers, PKCE
  always, signed cookie, `iss` check), record "front-channel and session iframe not
  implemented" with A-21 as the source, and the Zitadel skip under rule 6.

### 4.7 Owner decisions (section 4)

- **OD-15** Auto-link on verified email match without a password check. Default: no
  (ask with password; A-18). **Approved as defaulted** (owner review 2026-11-06, DEC-172).
- **OD-16** `role_source = provider` recomputes the role at every sign-in and can
  demote humans who were made admin by hand. Default: per provider, off unless
  `admin_groups` is set. **Approved as defaulted** (owner review 2026-11-06, DEC-172).
- **OD-17** Break-glass path `/sign-in/local` requires TOTP in `auto` mode. Default was
  yes. **Owner ruling: password only; TOTP is used if the account has it; the throttle, the
  audit row and the admin notification are kept** (owner review 2026-11-06, DEC-172).
- **OD-18** Front-channel logout: not implemented (depends on third-party cookies).
  Default: not implemented; revisit if an owner-relevant provider offers nothing else. **Approved as defaulted** (owner review 2026-11-06, DEC-172).
- **OD-19** Silent probe at Test time decides whether `auto` mode uses `prompt=none`.
  Default: yes; without a successful probe `auto` goes interactive directly. **Approved as defaulted** (owner review 2026-11-06, DEC-172).
- **OD-20** `/signed-out` does not auto-redirect even in `auto` mode. Default: yes. **Approved as defaulted** (owner review 2026-11-06, DEC-172).
- **OD-21** Keep email in sync from the provider when `email_verified`. Default: off. **Approved as defaulted** (owner review 2026-11-06, DEC-172).

---

## 5. Consolidated owner decisions

All owner decisions are decided (owner review 2026-11-06, DEC-171/DEC-172), except OD-12.
"As defaulted" means the default in the table was approved unchanged. Encrypting secrets at
rest (section 2) is decided (DEC-171).

| Id | Question | Default | Status and ruling |
|---|---|---|---|
| OD-01 | Density budget: 5 inputs / 7 rows open per page | as stated; 7 / 10 allowed on desktop-only admin pages | Decided: as defaulted (owner review 2026-11-06, DEC-172) |
| OD-02 | Remember disclosure state per user (localStorage) | yes | Decided: as defaulted (DEC-172) |
| OD-03 | Home: Recent activity 5 rows + link | yes | Decided: as defaulted (DEC-172) |
| OD-04 | Library: one Filters button | yes | Decided: as defaulted (DEC-172) |
| OD-05 | AEAD: AES-256-GCM vs XChaCha20-Poly1305 | AES-256-GCM | Decided: as defaulted (DEC-172) |
| OD-06 | First start without a key: auto-generate `secrets.key` | auto-generate | Decided: as defaulted (DEC-172) |
| OD-07 | Key source: file first, env second | as stated | Decided: as defaulted (DEC-172) |
| OD-08 | TOTP recovery codes (hashed, shown once) | yes | Decided: as defaulted (DEC-172) |
| OD-09 | Soft reminder to rotate the KEK after 2 years | yes | Decided: as defaulted (DEC-172) |
| OD-10 | Storage budget numbers: env/file only or editable | env/file | Decided: editable in the admin UI; env override stays, shown read-only with the variable name (owner review 2026-11-06, DEC-172) |
| OD-11 | Session length and cookie security: env-only or editable | env-only | Decided: editable in the admin UI, same env rule; cookie-security change applies on the next session; warn when the public URL is not https; must not lock the admin out (DEC-172) |
| OD-12 | Re-auth window for sign-in-related admin changes | 10 minutes | OPEN: 10 minutes until research proposes a value (DEC-172) |
| OD-13 | Audit retention | 365 days | Decided: as defaulted (DEC-172) |
| OD-14 | Storage and jobs on one admin page | one page | Decided: as defaulted (DEC-172) |
| OD-15 | Auto-link on verified email without password | no | Decided: as defaulted (DEC-172) |
| OD-16 | Provider-managed roles recomputed each sign-in | off unless admin groups set | Decided: as defaulted (DEC-172) |
| OD-17 | `/sign-in/local` requires TOTP in auto mode | yes | Decided: password only; TOTP used if the account has it; throttle, audit row and admin notification kept (DEC-172) |
| OD-18 | Front-channel logout | not implemented | Decided: as defaulted (DEC-172) |
| OD-19 | Silent probe gates `prompt=none` in auto mode | yes | Decided: as defaulted (DEC-172) |
| OD-20 | `/signed-out` never auto-redirects | yes | Decided: as defaulted (DEC-172) |
| OD-21 | Keep email in sync from provider | off | Decided: as defaulted (DEC-172) |

Reading order for the owner: section 5 (this table), then the mockups
`admin.html` and `signin-sso.html`, then the sections whose defaults you want to
change.
