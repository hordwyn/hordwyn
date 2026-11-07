use std::collections::BTreeMap;
use std::sync::Arc;

use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;
use serde::{Deserialize, Serialize};

use crate::i18n::I18n;

/// Sets the `dark` class before first paint so there is no flash of the wrong theme.
const THEME_BOOT: &str = "try{var t=localStorage.getItem('app-theme');if(t==='dark'||(!t&&matchMedia('(prefers-color-scheme: dark)').matches)){document.documentElement.classList.add('dark')}}catch(e){}";

/// What the browser needs to render the same page as the server (read at hydration).
/// The session part (`signed_in` .. `unread`) is filled from the session on the server, so
/// the shell never flashes the signed-out bar (design.md 3.2).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientConfig {
    /// The locale the page renders in (`<html lang>`).
    pub locale: String,
    /// The signed-in user's own choice (`None`: automatic), for the language picker.
    pub locale_setting: Option<String>,
    pub product_name: String,
    /// Brand overrides of message keys for `locale`.
    pub strings: BTreeMap<String, String>,
    pub signed_in: bool,
    pub display_name: String,
    /// The signed-in user's username (account menu head, design.md 3.2).
    pub username: String,
    pub is_admin: bool,
    /// Unread inbox notifications at render time.
    pub unread: u32,
    /// The brand's support link (last in the account menu, addendum-1 D13).
    pub support_url: Option<String>,
    /// The brand's legal links (`imprint` -> URL ...), in the auth pages' footer (6.14).
    pub legal: Vec<(String, String)>,
    /// Today (UTC) when the server rendered the page: History groups by day with it on both
    /// sides, so hydration sees the same Today and Yesterday.
    pub today: String,
}

impl ClientConfig {
    #[must_use]
    pub fn i18n(&self) -> Arc<I18n> {
        Arc::new(I18n::new(&self.locale, self.strings.clone()))
    }
}

/// Element id of the JSON config script.
pub const CONFIG_ID: &str = "app-config";

/// Server side: brand + `Accept-Language` + session -> config. The locale is the user's
/// choice, else the browser's, else the brand's default (DEC-201).
#[cfg(feature = "ssr")]
fn server_config() -> ClientConfig {
    let brand = use_context::<Arc<brand::Brand>>();
    let parts = use_context::<axum::http::request::Parts>();
    let accept = parts.as_ref().and_then(|p| {
        p.headers
            .get(axum::http::header::ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
    });
    let session = parts
        .as_ref()
        .and_then(|p| p.extensions.get::<crate::backend::ShellSession>().cloned());
    let default = brand
        .as_ref()
        .map_or_else(|| "en".to_string(), |b| b.default_locale.clone());
    let setting = session.as_ref().and_then(|s| s.locale.clone());
    let locale = crate::i18n::effective_locale(setting.as_deref(), accept.as_deref(), &default);
    ClientConfig {
        strings: brand
            .as_ref()
            .and_then(|b| b.strings.get(&locale).cloned())
            .unwrap_or_default(),
        product_name: brand.as_ref().map(|b| b.name.clone()).unwrap_or_default(),
        locale,
        locale_setting: setting,
        signed_in: session.is_some(),
        display_name: session
            .as_ref()
            .map(|s| s.display_name.clone())
            .unwrap_or_default(),
        username: session
            .as_ref()
            .map(|s| s.username.clone())
            .unwrap_or_default(),
        is_admin: session.as_ref().is_some_and(|s| s.is_admin),
        unread: session.as_ref().map_or(0, |s| s.unread),
        support_url: brand.as_ref().and_then(|b| b.support_url.clone()),
        legal: brand
            .as_ref()
            .map(|b| b.legal.clone().into_iter().collect())
            .unwrap_or_default(),
        today: crate::history_state::Day::today_utc().iso(),
    }
}

/// The config as JSON for the `<script type="application/json">` block. Every `<` is
/// escaped as `\u003c` (still valid JSON), so user text such as a display name cannot end
/// the script or switch the tokenizer into a script-data escape state (`<!--`, `<script`).
#[must_use]
pub(crate) fn config_json(cfg: &ClientConfig) -> String {
    serde_json::to_string(cfg)
        .unwrap_or_default()
        .replace('<', "\\u003c")
}

/// HTML document around the app (server side). Brand theme tokens, product name, locale and
/// string overrides come from the brand in context (D-10).
#[must_use]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    #[cfg(feature = "ssr")]
    let cfg = server_config();
    #[cfg(not(feature = "ssr"))]
    let cfg = ClientConfig::default();
    #[cfg(feature = "ssr")]
    let theme = use_context::<Arc<brand::Brand>>()
        .map(|b| b.head_css())
        .unwrap_or_default();
    #[cfg(not(feature = "ssr"))]
    let theme = String::new();
    crate::provide_page_context(&cfg);
    let json = config_json(&cfg);
    view! {
        <!DOCTYPE html>
        // the tab bar (and the room kept for it) exists only for a signed-in user (UX0-13)
        <html lang=cfg.locale.clone() class=cfg.signed_in.then_some("has-tab-bar")>
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <script>{THEME_BOOT}</script>
                <script type="application/json" id=CONFIG_ID inner_html=json></script>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <MetaTags/>
                // after the stylesheet link: brand fonts (same origin) and theme tokens
                <style id="brand-theme" inner_html=theme></style>
            </head>
            <body class="min-h-screen bg-background text-foreground antialiased">
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // `generate_route_list(App)` renders the app without the shell: use defaults then.
    if use_context::<ClientConfig>().is_none() {
        crate::provide_page_context(&ClientConfig::default());
    }
    provide_meta_context();
    crate::events::provide_events();
    let cfg = expect_context::<ClientConfig>();
    let user = Resource::new(|| (), |()| crate::api::current_user());
    provide_context(user);
    // Marker for end-to-end tests: effects only run in the browser, after hydration.
    Effect::new(move |_| {
        #[cfg(feature = "hydrate")]
        if let Some(body) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.body())
        {
            let _ = body.set_attribute("data-hydrated", "1");
        }
    });
    // Routes: `/history` and `/people` replaced `/sessions` and `/companions` without
    // redirects (owner ruling, DEC-171); the old paths are 404s.
    view! {
        <Stylesheet id="leptos" href="/pkg/app.css"/>
        <Title text=cfg.product_name.clone()/>
        <Router>
            <crate::shell::AppShell>
                <Routes fallback=crate::shell::NotFound>
                    <Route path=path!("/") view=crate::home::Home/>
                    <Route path=path!("/sign-in") view=crate::pages::SignIn/>
                    <Route path=path!("/sign-up") view=crate::pages::SignUp/>
                    <Route path=path!("/invite/:token") view=crate::pages::SignUp/>
                    <Route path=path!("/library") view=crate::library::Library/>
                    <Route path=path!("/items/:id") view=crate::item::ItemPage/>
                    <Route path=path!("/people") view=crate::sharing::Companions/>
                    <Route path=path!("/history") view=crate::history::History/>
                    <Route path=path!("/stats") view=crate::home::Stats/>
                    <Route path=path!("/you") view=crate::shell::You/>
                    <Route path=path!("/notifications") view=crate::notifications::Notifications/>
                    <Route path=path!("/integrations") view=crate::integrations::Integrations/>
                    <Route path=path!("/shared/lists/:token") view=crate::sharing::SharedList/>
                    <Route path=path!("/lists") view=crate::pages::Lists/>
                    <Route path=path!("/sources") view=crate::pages::Sources/>
                    <Route path=path!("/dev/gallery") view=GalleryPage/>
                </Routes>
            </crate::shell::AppShell>
        </Router>
    }
}

/// The signed-in user (shared resource).
pub type UserResource = Resource<Result<Option<contracts::UserDto>, ServerFnError>>;

/// The component gallery exists only in builds with the `gallery` feature (dev).
#[component]
fn GalleryPage() -> impl IntoView {
    #[cfg(feature = "gallery")]
    let page = view! { <crate::gallery::Gallery/> }.into_any();
    #[cfg(not(feature = "gallery"))]
    let page = view! { <crate::shell::NotFound/> }.into_any();
    page
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UX0-8: user text in the hydration config cannot break out of its script block.
    #[test]
    fn the_config_json_has_no_angle_bracket() {
        let cfg = ClientConfig {
            display_name: "<!--<script></script>".into(),
            ..ClientConfig::default()
        };
        let json = config_json(&cfg);
        assert!(!json.contains('<'), "{json}");
        let back: ClientConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }
}
