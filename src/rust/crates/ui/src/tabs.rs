//! Tabs with roving focus (Left/Right/Home/End), `aria-selected` and `aria-controls`.

use leptos::prelude::*;

use crate::class::cn;

#[derive(Clone, Copy)]
struct TabsCtx {
    active: RwSignal<String>,
    values: RwSignal<Vec<String>>,
    base: StoredValue<String>,
}

fn tab_id(base: &str, value: &str) -> String {
    format!("{base}-tab-{value}")
}

fn panel_id(base: &str, value: &str) -> String {
    format!("{base}-panel-{value}")
}

/// Container; `id` prefixes the generated element ids.
#[component]
pub fn Tabs(
    #[prop(into)] id: String,
    active: RwSignal<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    provide_context(TabsCtx {
        active,
        values: RwSignal::new(Vec::new()),
        base: StoredValue::new(id),
    });
    view! { <div class=move || cn(&["grid gap-2", &class.get().unwrap_or_default()])>{children()}</div> }
}

/// The tab strip. `label` names the tab list for screen readers.
#[component]
pub fn TabsList(
    #[prop(into)] label: String,
    #[prop(into, optional)] class: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    let ctx = expect_context::<TabsCtx>();
    let on_keydown = move |ev: leptos::ev::KeyboardEvent| {
        let values = ctx.values.get_untracked();
        if values.is_empty() {
            return;
        }
        let cur = values
            .iter()
            .position(|v| *v == ctx.active.get_untracked())
            .unwrap_or(0);
        let next = match ev.key().as_str() {
            "ArrowRight" => (cur + 1) % values.len(),
            "ArrowLeft" => (cur + values.len() - 1) % values.len(),
            "Home" => 0,
            "End" => values.len() - 1,
            _ => return,
        };
        ev.prevent_default();
        ctx.active.set(values[next].clone());
        let id = tab_id(&ctx.base.get_value(), &values[next]);
        if let Some(el) = document().get_element_by_id(&id) {
            use leptos::wasm_bindgen::JsCast;
            if let Ok(el) = el.dyn_into::<web_sys::HtmlElement>() {
                let _ = el.focus();
            }
        }
    };
    view! {
        <div
            role="tablist"
            aria-label=label
            on:keydown=on_keydown
            class=move || cn(&[
                "inline-flex h-9 items-center justify-center rounded-lg bg-muted p-1 text-muted-foreground",
                &class.get().unwrap_or_default(),
            ])
        >
            {children()}
        </div>
    }
}

#[component]
pub fn TabsTrigger(#[prop(into)] value: String, children: Children) -> impl IntoView {
    let ctx = expect_context::<TabsCtx>();
    ctx.values.update(|v| {
        if !v.contains(&value) {
            v.push(value.clone());
        }
    });
    let base = ctx.base.get_value();
    let v1 = value.clone();
    let v2 = value.clone();
    let v3 = value.clone();
    let selected = Memo::new(move |_| ctx.active.get() == v1);
    view! {
        <button
            type="button"
            role="tab"
            id=tab_id(&base, &value)
            aria-controls=panel_id(&base, &value)
            aria-selected=move || if selected.get() { "true" } else { "false" }
            tabindex=move || if ctx.active.get() == v2 { "0" } else { "-1" }
            on:click=move |_| ctx.active.set(v3.clone())
            class=move || cn(&[
                "inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm \
                 font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                if selected.get() { "bg-background text-foreground shadow" } else { "" },
            ])
        >
            {children()}
        </button>
    }
}

#[component]
pub fn TabsContent(#[prop(into)] value: String, children: ChildrenFn) -> impl IntoView {
    let ctx = expect_context::<TabsCtx>();
    let base = ctx.base.get_value();
    let v = value.clone();
    view! {
        <div
            role="tabpanel"
            id=panel_id(&base, &value)
            aria-labelledby=tab_id(&base, &value)
            tabindex="0"
            hidden=move || ctx.active.get() != v
            class="focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-md"
        >
            {children()}
        </div>
    }
}
