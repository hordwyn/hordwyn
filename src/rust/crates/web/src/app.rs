use std::collections::BTreeMap;
use std::sync::Arc;

use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::components::{A, Route, Router, Routes};
use leptos_router::path;
use serde::{Deserialize, Serialize};
use ui::{Icon, IconName, ThemeToggle};

use crate::i18n::{I18n, t, use_i18n};

/// Sets the `dark` class before first paint so there is no flash of the wrong theme.
const THEME_BOOT: &str = "try{var t=localStorage.getItem('app-theme');if(t==='dark'||(!t&&matchMedia('(prefers-color-scheme: dark)').matches)){document.documentElement.classList.add('dark')}}catch(e){}";

/// What the browser needs to render the same page as the server (read at hydration).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ClientConfig {
    pub locale: String,
    pub product_name: String,
    /// Brand overrides of message keys for `locale`.
    pub strings: BTreeMap<String, String>,
}

impl ClientConfig {
    #[must_use]
    pub fn i18n(&self) -> Arc<I18n> {
        Arc::new(I18n::new(&self.locale, self.strings.clone()))
    }
}

/// Element id of the JSON config script.
pub const CONFIG_ID: &str = "app-config";

/// Server side: brand + `Accept-Language` -> config.
#[cfg(feature = "ssr")]
fn server_config() -> ClientConfig {
    let brand = use_context::<Arc<brand::Brand>>();
    let accept = use_context::<axum::http::request::Parts>().and_then(|p| {
        p.headers
            .get(axum::http::header::ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
    });
    let default = brand
        .as_ref()
        .map_or_else(|| "en".to_string(), |b| b.default_locale.clone());
    let locale = crate::i18n::negotiate(accept.as_deref(), &default);
    ClientConfig {
        strings: brand
            .as_ref()
            .and_then(|b| b.strings.get(&locale).cloned())
            .unwrap_or_default(),
        product_name: brand.as_ref().map(|b| b.name.clone()).unwrap_or_default(),
        locale,
    }
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
        .map(|b| b.theme_css())
        .unwrap_or_default();
    #[cfg(not(feature = "ssr"))]
    let theme = String::new();
    crate::provide_page_context(&cfg);
    let json = serde_json::to_string(&cfg)
        .unwrap_or_default()
        .replace("</", "<\\/");
    view! {
        <!DOCTYPE html>
        <html lang=cfg.locale.clone()>
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <script>{THEME_BOOT}</script>
                <style id="brand-theme">{theme}</style>
                <script type="application/json" id=CONFIG_ID inner_html=json></script>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <MetaTags/>
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
    view! {
        <Stylesheet id="leptos" href="/pkg/app.css"/>
        <Title text=cfg.product_name.clone()/>
        <Router>
            <a href="#main" class="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:z-50 focus:rounded focus:bg-background focus:p-2">
                {t("skip-to-content")}
            </a>
            <Header/>
            <crate::palette::SearchPalette/>
            <main id="main" class="mx-auto w-full max-w-6xl px-4 py-6">
                <Routes fallback=|| view! { <p class="p-8">"404"</p> }>
                    <Route path=path!("/") view=crate::pages::Home/>
                    <Route path=path!("/sign-in") view=crate::pages::SignIn/>
                    <Route path=path!("/sign-up") view=crate::pages::SignUp/>
                    <Route path=path!("/invite/:token") view=crate::pages::SignUp/>
                    <Route path=path!("/library") view=crate::pages::Library/>
                    <Route path=path!("/items/:id") view=crate::pages::ItemPage/>
                    <Route path=path!("/companions") view=crate::sharing::Companions/>
                    <Route path=path!("/sessions") view=crate::sharing::Sessions/>
                    <Route path=path!("/shared/lists/:token") view=crate::sharing::SharedList/>
                    <Route path=path!("/lists") view=crate::pages::Lists/>
                    <Route path=path!("/sources") view=crate::pages::Sources/>
                    <Route path=path!("/dev/gallery") view=GalleryPage/>
                </Routes>
            </main>
        </Router>
    }
}

/// The signed-in user (shared resource).
pub type UserResource = Resource<Result<Option<contracts::UserDto>, ServerFnError>>;

#[component]
fn Header() -> impl IntoView {
    let i18n = use_i18n();
    let cfg = expect_context::<ClientConfig>();
    let user = expect_context::<UserResource>();
    let open_search = expect_context::<crate::palette::PaletteOpen>();
    let sign_out = Action::new(|(): &()| async {
        let _ = crate::api::sign_out().await;
        #[cfg(feature = "hydrate")]
        if let Some(w) = web_sys::window() {
            let _ = w.location().assign("/");
        }
    });
    let nav_label = i18n.t("nav-main");
    view! {
        <header class="border-b border-border">
            <div class="mx-auto flex max-w-6xl items-center gap-4 px-4 py-3">
                <A href="/" attr:class="text-lg font-semibold tracking-tight">{cfg.product_name.clone()}</A>
                <nav aria-label=nav_label class="flex flex-1 items-center gap-4 text-sm">
                    <Suspense fallback=|| ()>
                        {move || user.get().map(|u| match u {
                            Ok(Some(_)) => view! {
                                <A href="/library">{t("nav-library")}</A>
                                <A href="/lists">{t("nav-lists")}</A>
                                <A href="/sessions">{t("nav-sessions")}</A>
                                <A href="/companions">{t("nav-companions")}</A>
                                <A href="/sources">{t("nav-sources")}</A>
                            }.into_any(),
                            _ => view! {
                                <A href="/sign-in">{t("nav-sign-in")}</A>
                                <A href="/sign-up">{t("nav-sign-up")}</A>
                            }.into_any(),
                        })}
                    </Suspense>
                </nav>
                <button
                    type="button"
                    on:click=move |_| open_search.0.set(true)
                    class="inline-flex items-center gap-2 rounded-md border border-input px-3 py-1.5 text-sm text-muted-foreground"
                >
                    <Icon name=IconName::Search/>
                    <span>{t("nav-search")}</span>
                    <kbd class="rounded bg-muted px-1 text-xs">{t("search-hint")}</kbd>
                </button>
                <ThemeToggle dark_label=i18n.t("theme-dark") light_label=i18n.t("theme-light") storage_key="app-theme"/>
                <Suspense fallback=|| ()>
                    {move || matches!(user.get(), Some(Ok(Some(_)))).then(|| view! {
                        <button type="button" class="text-sm" on:click=move |_| { sign_out.dispatch(()); }>
                            {t("nav-sign-out")}
                        </button>
                    })}
                </Suspense>
            </div>
        </header>
    }
}

/// The component gallery exists only in builds with the `gallery` feature (dev).
#[component]
fn GalleryPage() -> impl IntoView {
    #[cfg(feature = "gallery")]
    let page = view! { <crate::gallery::Gallery/> }.into_any();
    #[cfg(not(feature = "gallery"))]
    let page = view! { <p class="p-8">"404"</p> }.into_any();
    page
}
