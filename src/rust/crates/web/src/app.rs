use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

/// Sets the `dark` class before first paint so there is no flash of the wrong theme.
const THEME_BOOT: &str = "try{var t=localStorage.getItem('app-theme');if(t==='dark'||(!t&&matchMedia('(prefers-color-scheme: dark)').matches)){document.documentElement.classList.add('dark')}}catch(e){}";

/// HTML document around the app (server side).
#[must_use]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <script>{THEME_BOOT}</script>
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
    provide_meta_context();
    view! {
        <Stylesheet id="leptos" href="/pkg/app.css"/>
        <Title text="Tracker"/>
        <Router>
            <main>
                <Routes fallback=|| view! { <p class="p-8">"404"</p> }>
                    <Route path=path!("/") view=Home/>
                    <Route path=path!("/dev/gallery") view=GalleryPage/>
                </Routes>
            </main>
        </Router>
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

#[component]
fn Home() -> impl IntoView {
    view! { <div class="p-8"></div> }
}
