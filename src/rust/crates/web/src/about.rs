//! The About page and the running version (#18, DEC-868): the version under the account
//! menu (reachable from every page) and on the You page; the About page with version and
//! commit, the brand's links (source, release notes, documentation, security contact), the
//! licence of the software, the third-party notices of the build and the data sources.
//!
//! Not a page footer: an element after the page's content lifts the sticky bottom bars
//! (bulk bar, item action bar) above the toasts at the end of a page.

use leptos::prelude::*;

use crate::app::ClientConfig;
use crate::i18n::{t, use_i18n};
use crate::pages::PageTitle;

/// The licence of the software; it travels with every build.
const LICENCE: &str = include_str!("../../../../../LICENSE");

/// Where the server answers with the notices file of the image.
pub const NOTICES_HREF: &str = "/about/notices.txt";

/// The product name and the version: "<product> 1.1.0".
fn version_line(cfg: &ClientConfig) -> String {
    format!("{} {}", cfg.product_name, cfg.version)
}

/// The running version as a quiet line (account menu, You page); signed in only.
#[component]
pub fn VersionLine(#[prop(optional, into)] class: String) -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    (!cfg.version.is_empty()).then(|| {
        view! {
            <p data-testid="app-version" class=ui::cn(&["text-xs text-muted-foreground", &class])>
                {version_line(&cfg)}
            </p>
        }
    })
}

#[component]
pub fn About() -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    let i18n = use_i18n();
    if !cfg.signed_in {
        return view! {
            <PageTitle text=i18n.t("about-title")/>
            <p><a href="/sign-in" class="text-primary underline">{t("nav-sign-in")}</a></p>
        }
        .into_any();
    }
    let links: Vec<(String, String)> = [
        ("about-release-notes", cfg.release_notes_url.clone()),
        ("about-source", cfg.source_url.clone()),
        ("about-docs", cfg.docs_url.clone()),
        ("about-security", cfg.security_url.clone()),
    ]
    .into_iter()
    .filter_map(|(key, url)| url.map(|u| (i18n.t(key), u)))
    .collect();
    let commit = cfg.commit.clone().map(|c| {
        view! {
            <dt class="text-muted-foreground">{t("about-commit")}</dt>
            <dd><code data-testid="about-commit">{c}</code></dd>
        }
    });
    view! {
        <PageTitle text=i18n.t("about-title")/>
        <div class="grid max-w-3xl gap-8">
            <section aria-labelledby="about-version-h">
                <h2 id="about-version-h" class="mb-2 text-lg font-semibold">{version_line(&cfg)}</h2>
                <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
                    <dt class="text-muted-foreground">{t("about-version")}</dt>
                    <dd data-testid="about-version">{cfg.version.clone()}</dd>
                    {commit}
                </dl>
            </section>
            <section aria-labelledby="about-links-h">
                <h2 id="about-links-h" class="mb-2 text-lg font-semibold">{t("about-links")}</h2>
                <ul class="grid gap-1 text-sm">
                    {links.into_iter().map(|(label, href)| view! {
                        <li><a href=href rel="noopener" class="text-primary underline">{label}</a></li>
                    }).collect_view()}
                </ul>
            </section>
            <section aria-labelledby="about-open-source-h">
                <h2 id="about-open-source-h" class="mb-2 text-lg font-semibold">{t("about-open-source")}</h2>
                <p class="mb-2 text-sm">{t("about-open-source-lead")}</p>
                <ul class="grid gap-1 text-sm">
                    <li><a href=NOTICES_HREF class="text-primary underline">{t("about-notices")}</a></li>
                    <li><a href="/settings/sources#attribution" class="text-primary underline">{t("about-data-sources")}</a></li>
                </ul>
            </section>
            <section aria-labelledby="about-licence-h">
                <h2 id="about-licence-h" class="mb-2 text-lg font-semibold">{t("about-licence")}</h2>
                <pre lang="en" class="whitespace-pre-wrap rounded-md border border-border p-3 text-xs">{LICENCE}</pre>
            </section>
        </div>
    }
    .into_any()
}
