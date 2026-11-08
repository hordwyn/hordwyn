//! The body of the `302` that sends a signed-out page visit to the identity provider in
//! sign-in mode `auto` (addendum-1 4.5 state 2), for user agents that do not follow the
//! redirect: one `role=status` line and a plain link, no script and no animation. The
//! server's page middleware renders it (phase 3 WP3-3); the sign-in page states 3-6 are
//! the web app's.

use std::collections::BTreeMap;

use fluent_bundle::FluentValue;

use crate::i18n::I18n;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// A complete HTML document: "Taking you to {provider}..." and a link to `url`.
#[must_use]
pub fn interstitial(
    locale: &str,
    overrides: BTreeMap<String, String>,
    provider: &str,
    url: &str,
) -> String {
    let i = I18n::new(locale, overrides);
    let arg = [("provider", FluentValue::from(provider.to_string()))];
    let title = escape(&i.t("auth-redirect-title"));
    let status = escape(&i.t_args("auth-redirect-status", &arg));
    let link = escape(&i.t_args("auth-redirect-continue", &arg));
    format!(
        "<!doctype html><html lang=\"{lang}\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <meta name=\"robots\" content=\"noindex\"><title>{title}</title></head>\
         <body><main><h1>{title}</h1><p role=\"status\">{status}</p>\
         <p><a href=\"{href}\">{link}</a></p></main></body></html>",
        lang = escape(i.locale()),
        href = escape(url),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_the_provider_and_the_url() {
        let html = interstitial(
            "de",
            BTreeMap::new(),
            "<b>SSO</b>",
            "https://id.example/a?x=1&y=\"2\"",
        );
        assert!(html.contains("lang=\"de\""), "{html}");
        assert!(html.contains("role=\"status\""), "{html}");
        assert!(html.contains("&lt;b&gt;SSO&lt;/b&gt;"), "{html}");
        assert!(html.contains("x=1&amp;y=&quot;2&quot;"), "{html}");
        assert!(!html.contains("<b>"), "{html}");
    }
}
