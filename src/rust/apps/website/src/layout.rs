//! Page frame: document head, top bar, footer, and the layout primitives the pages share.
//! Components and colours come from the `ui` crate and the brand tokens; this file only
//! arranges them.

use std::fmt::Write as _;
use std::sync::Arc;

use leptos::prelude::*;
use ui::{ButtonLink, ButtonSize, ButtonVariant, Icon, IconName, cn};

use crate::content::{Intro, Link, Locale, Site};
use crate::markdown::escape;

/// Sets the `dark` class before first paint (the visitor's choice, else the system setting)
/// and marks the document as scripted, which shows the theme button.
pub const THEME_BOOT: &str = "try{var r=document.documentElement,t=localStorage.getItem('site-theme');r.classList.add('js');if(t==='dark'||(!t&&matchMedia('(prefers-color-scheme: dark)').matches)){r.classList.add('dark')}}catch(e){}";

/// Focus ring shared by the links of the frame (the `ui` buttons carry their own).
pub const FOCUS: &str = "rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring \
    focus-visible:ring-offset-2 ring-offset-background";

/// What a page needs to know about its place in the site.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub site: Arc<Site>,
    /// Index into `site.locales`.
    pub locale: usize,
    /// Path of the page without the locale prefix: `/`, `/features/`.
    pub path: String,
}

impl Ctx {
    #[must_use]
    pub fn l(&self) -> &Locale {
        &self.site.locales[self.locale]
    }

    #[must_use]
    pub fn t(&self, key: &str) -> String {
        self.l().t(key)
    }

    /// `path` inside the current locale.
    #[must_use]
    pub fn href(&self, path: &str) -> String {
        format!("{}{path}", self.site.config.root(&self.l().code))
    }

    /// The current page in every locale: (code, path).
    #[must_use]
    pub fn alternates(&self) -> Vec<(String, String)> {
        self.site
            .locales
            .iter()
            .map(|l| {
                (
                    l.code.clone(),
                    format!("{}{}", self.site.config.root(&l.code), self.path),
                )
            })
            .collect()
    }

    /// Language of the documentation when it differs from the page's (for `hreflang`).
    #[must_use]
    pub fn docs_lang(&self) -> Option<String> {
        let lang = &self.site.config.docs_lang;
        (*lang != self.l().code).then(|| lang.clone())
    }

    fn nav(&self) -> Vec<(String, String, bool, Option<String>)> {
        let docs = self.site.config.docs_url.clone();
        [
            ("nav-features", self.href("/features/"), "/features/"),
            (
                "nav-self-hosting",
                self.href("/self-hosting/"),
                "/self-hosting/",
            ),
            ("nav-hub", self.href("/hub/"), "/hub/"),
            ("nav-changelog", self.href("/changelog/"), "/changelog/"),
            ("nav-docs", docs, ""),
        ]
        .into_iter()
        .map(|(key, href, path)| {
            let lang = (key == "nav-docs").then(|| self.docs_lang()).flatten();
            (self.t(key), href, self.path == path, lang)
        })
        .collect()
    }
}

/// Centred column with the page gutters.
#[component]
pub fn Container(
    #[prop(into, optional)] class: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class=move || cn(&["mx-auto w-full max-w-6xl px-4 sm:px-6 lg:px-8", &class.get().unwrap_or_default()])>
            {children()}
        </div>
    }
}

/// A band of the page with its vertical rhythm.
#[component]
pub fn Section(
    #[prop(into, optional)] id: Option<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <section id=id class=move || cn(&["py-14 sm:py-20", &class.get().unwrap_or_default()])>
            <Container>{children()}</Container>
        </section>
    }
}

/// Small uppercase label above a heading.
#[component]
pub fn Eyebrow(#[prop(into)] text: String) -> impl IntoView {
    view! { <p class="text-caption uppercase text-primary">{text}</p> }
}

/// Head of a section: label, `<h2>`, optional lead.
#[component]
pub fn SectionHead(intro: Intro, #[prop(into, optional)] id: Option<String>) -> impl IntoView {
    let lead = (!intro.text.is_empty()).then(|| {
        view! { <p class="mt-4 max-w-2xl text-lg text-muted-foreground">{intro.text.clone()}</p> }
    });
    view! {
        <div>
            <Eyebrow text=intro.eyebrow/>
            <h2 id=id class="mt-2 text-3xl font-bold tracking-tight text-balance sm:text-4xl">{intro.heading}</h2>
            {lead}
        </div>
    }
}

/// "More" link at the end of a section.
#[component]
pub fn MoreLink(link: Link) -> impl IntoView {
    view! {
        <a href=link.href hreflang=link.hreflang class=cn(&["inline-flex min-h-target items-center gap-1 font-medium text-primary underline-offset-4 hover:underline", FOCUS])>
            {link.label}
            <Icon name=IconName::ChevronRight/>
        </a>
    }
}

#[component]
fn NavLink(
    label: String,
    href: String,
    current: bool,
    hreflang: Option<String>,
    #[prop(into, optional)] class: String,
) -> impl IntoView {
    let tone = if current {
        "text-foreground font-semibold"
    } else {
        "text-muted-foreground hover:text-foreground"
    };
    view! {
        <a
            href=href
            hreflang=hreflang
            aria-current=current.then_some("page")
            class=cn(&["inline-flex min-h-target items-center text-sm font-medium transition-colors", tone, FOCUS, &class])
        >
            {label}
        </a>
    }
}

#[component]
fn Header(ctx: Ctx) -> impl IntoView {
    let name = ctx.site.config.name.clone();
    let home = ctx.href("/");
    let nav = ctx.nav();
    let other = ctx
        .alternates()
        .into_iter()
        .filter(|(code, _)| *code != ctx.l().code)
        .map(|(code, href)| {
            let label = ctx.t(&format!("lang-{code}"));
            view! {
                <a
                    href=href
                    lang=code.clone()
                    hreflang=code
                    class=cn(&["inline-flex h-9 items-center px-2 text-sm font-medium text-muted-foreground hover:text-foreground", FOCUS])
                >
                    {label}
                </a>
            }
        })
        .collect_view();
    let desktop = nav
        .iter()
        .cloned()
        .map(|(label, href, current, hreflang)| view! { <li><NavLink label href current hreflang/></li> })
        .collect_view();
    let drawer = nav
        .into_iter()
        .map(|(label, href, current, hreflang)| {
            view! { <li><NavLink label href current hreflang class="min-h-target-touch w-full px-4 text-base"/></li> }
        })
        .collect_view();
    view! {
        <header class="sticky top-0 z-40 border-b bg-background/90 backdrop-blur">
            <Container class="flex h-14 items-center justify-between gap-4">
                <a href=home class=cn(&["text-lg font-bold tracking-tight", FOCUS])>{name}</a>
                <nav aria-label=ctx.t("nav-label") class="hidden md:block">
                    <ul class="flex items-center gap-6">{desktop}</ul>
                </nav>
                <div class="flex items-center gap-1">
                    {other}
                    <button
                        type="button"
                        data-theme-toggle=""
                        aria-pressed="false"
                        aria-label=ctx.t("theme-dark")
                        class="hidden size-9 cursor-pointer items-center justify-center rounded-md hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring [.js_&]:inline-flex"
                    >
                        <Icon name=IconName::Moon class="dark:hidden"/>
                        <Icon name=IconName::Sun class="hidden dark:block"/>
                    </button>
                    <details class="group md:hidden">
                        <summary
                            aria-label=ctx.t("menu-label")
                            class="inline-flex size-9 cursor-pointer list-none items-center justify-center rounded-md hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring [&::-webkit-details-marker]:hidden"
                        >
                            <Icon name=IconName::Menu class="size-5 group-open:hidden"/>
                            <Icon name=IconName::Close class="hidden size-5 group-open:block"/>
                        </summary>
                        <nav
                            aria-label=ctx.t("menu-nav-label")
                            class="absolute inset-x-0 top-14 border-b bg-background py-2 shadow-elevation-2"
                        >
                            <ul>{drawer}</ul>
                        </nav>
                    </details>
                </div>
            </Container>
        </header>
    }
}

#[component]
fn FooterColumn(heading: String, links: Vec<(String, String, Option<String>)>) -> impl IntoView {
    let items = links
        .into_iter()
        .map(|(label, href, hreflang)| {
            view! {
                <li>
                    <a href=href hreflang=hreflang class=cn(&["inline-flex min-h-target items-center text-sm text-muted-foreground hover:text-foreground", FOCUS])>
                        {label}
                    </a>
                </li>
            }
        })
        .collect_view();
    view! {
        <div>
            <h2 class="text-caption uppercase text-foreground">{heading}</h2>
            <ul class="mt-3 grid gap-1">{items}</ul>
        </div>
    }
}

#[component]
fn Footer(ctx: Ctx) -> impl IntoView {
    let cfg = ctx.site.config.clone();
    let link = |key: &str, path: &str| (ctx.t(key), ctx.href(path), None);
    let product = vec![
        link("nav-features", "/features/"),
        link("nav-self-hosting", "/self-hosting/"),
        (ctx.t("nav-docs"), cfg.docs_url.clone(), ctx.docs_lang()),
        link("nav-changelog", "/changelog/"),
    ];
    let mut project = vec![
        link("nav-hub", "/hub/"),
        link("nav-security", "/security/"),
        link("nav-attributions", "/attributions/"),
    ];
    if !cfg.source_url.is_empty() {
        project.push((ctx.t("nav-source"), cfg.source_url.clone(), None));
    }
    let legal = vec![
        link("nav-imprint", "/imprint/"),
        link("nav-privacy", "/privacy/"),
    ];
    let (blurb, note) = (ctx.t("footer-blurb"), ctx.t("footer-note"));
    let (h_product, h_project, h_legal) = (
        ctx.t("footer-product"),
        ctx.t("footer-project"),
        ctx.t("footer-legal"),
    );
    view! {
        <footer class="border-t bg-muted/40">
            <Container class="py-12 sm:py-16">
                <div class="grid gap-10 sm:grid-cols-2 lg:grid-cols-4">
                    <div>
                        <p class="text-lg font-bold tracking-tight">{cfg.name.clone()}</p>
                        <p class="mt-3 max-w-xs text-sm text-muted-foreground">{blurb}</p>
                    </div>
                    <FooterColumn heading=h_product links=product/>
                    <FooterColumn heading=h_project links=project/>
                    <FooterColumn heading=h_legal links=legal/>
                </div>
                <p class="mt-10 border-t pt-6 text-small text-muted-foreground">{note}</p>
            </Container>
        </footer>
    }
}

/// Header of a page below the landing page: label, `<h1>`, lead.
#[component]
pub fn PageHead(
    #[prop(into)] eyebrow: String,
    #[prop(into)] heading: String,
    #[prop(into)] lead: String,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <div data-band="head" class="border-b bg-gradient-to-b from-primary-soft to-background">
            <Container class="py-12 sm:py-16">
                <Eyebrow text=eyebrow/>
                <h1 class="mt-2 max-w-3xl text-4xl font-bold tracking-tight text-balance sm:text-5xl">{heading}</h1>
                <p class="mt-4 max-w-2xl text-lg text-muted-foreground">{lead}</p>
                {children.map(|c| c())}
            </Container>
        </div>
    }
}

/// Two call-to-action links: the first filled, the second outlined.
#[component]
pub fn Actions(
    primary: Link,
    secondary: Link,
    #[prop(into, optional)] class: String,
) -> impl IntoView {
    view! {
        <div class=cn(&["flex flex-wrap gap-3", &class])>
            <ButtonLink href=primary.href size=ButtonSize::Lg class="h-11 text-base" attr:hreflang=primary.hreflang>
                {primary.label}
            </ButtonLink>
            <ButtonLink
                href=secondary.href
                variant=ButtonVariant::Outline
                size=ButtonSize::Lg
                class="h-11 text-base"
                attr:hreflang=secondary.hreflang
            >
                {secondary.label}
            </ButtonLink>
        </div>
    }
}

/// The body of a page: skip link, top bar, `<main>`, footer.
#[component]
pub fn Frame(ctx: Ctx, children: Children) -> impl IntoView {
    view! {
        <a
            href="#main"
            class="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-50 focus:rounded-md focus:bg-background focus:px-4 focus:py-2 focus:shadow-elevation-2 focus:ring-2 focus:ring-ring"
        >
            {ctx.t("skip-to-content")}
        </a>
        <Header ctx=ctx.clone()/>
        <main id="main" tabindex="-1" class="focus:outline-none">{children()}</main>
        <Footer ctx/>
    }
}

/// What the document head needs besides the page itself.
#[derive(Debug, Clone, Default)]
pub struct Head {
    pub title: String,
    pub description: String,
    /// `<style>` content from the brand: fonts and theme tokens.
    pub brand_css: String,
    /// Address of the stylesheet.
    pub css_href: String,
    /// Addresses of the upright font files, preloaded.
    pub font_preloads: Vec<String>,
    /// Keep the page out of search engines (the not-found page).
    pub noindex: bool,
    /// Path of the preview picture for shared links (a capture of the application).
    pub image: Option<String>,
}

/// The whole HTML document around a rendered body.
#[must_use]
pub fn document(ctx: &Ctx, head: &Head, body: &str) -> String {
    let cfg = &ctx.site.config;
    let lang = &ctx.l().code;
    let own = format!("{}{}{}", cfg.base_url, cfg.root(lang), ctx.path);
    let mut links = String::new();
    if head.noindex {
        links.push_str("<meta name=\"robots\" content=\"noindex\">");
    } else {
        let _ = write!(links, "<link rel=\"canonical\" href=\"{}\">", escape(&own));
        for (code, path) in ctx.alternates() {
            let _ = write!(
                links,
                "<link rel=\"alternate\" hreflang=\"{}\" href=\"{}\">",
                escape(&code),
                escape(&format!("{}{path}", cfg.base_url))
            );
        }
    }
    if let Some(image) = &head.image {
        let _ = write!(
            links,
            "<meta property=\"og:image\" content=\"{}\"><meta name=\"twitter:card\" content=\"summary_large_image\">",
            escape(&format!("{}{image}", cfg.base_url))
        );
    }
    for font in &head.font_preloads {
        let _ = write!(
            links,
            "<link rel=\"preload\" as=\"font\" type=\"font/woff2\" crossorigin href=\"{}\">",
            escape(font)
        );
    }
    let full_title = if head.title == cfg.name {
        head.title.clone()
    } else {
        format!("{} - {}", head.title, cfg.name)
    };
    format!(
        "<!DOCTYPE html>\n<html lang=\"{lang}\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <meta name=\"color-scheme\" content=\"light dark\">\
         <title>{title}</title><meta name=\"description\" content=\"{desc}\">\
         <meta property=\"og:title\" content=\"{title}\"><meta property=\"og:description\" content=\"{desc}\">\
         <meta property=\"og:type\" content=\"website\"><meta property=\"og:url\" content=\"{own}\">\
         <meta property=\"og:site_name\" content=\"{name}\">{links}\
         <script>{boot}</script><style>{brand}</style>\
         <link rel=\"stylesheet\" href=\"{css}\"></head>\
         <body class=\"min-h-screen bg-background font-sans text-foreground antialiased\">{body}\
         <script src=\"/assets/theme.js\" defer></script></body></html>\n",
        lang = escape(lang),
        title = escape(&full_title),
        desc = escape(&head.description),
        own = escape(&own),
        name = escape(&cfg.name),
        boot = THEME_BOOT,
        brand = head.brand_css,
        css = escape(&head.css_href),
    )
}
