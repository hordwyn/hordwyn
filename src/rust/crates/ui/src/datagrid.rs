//! Data grid: a table with sortable columns (`aria-sort`, sort buttons in headers).

use leptos::prelude::*;

use crate::class::cn;
use crate::icon::{Icon, IconName};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub key: String,
    pub label: String,
    pub sortable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    Asc,
    Desc,
}

/// Compare cells numerically when both parse as numbers, else as text.
fn cmp_cells(a: &str, b: &str) -> std::cmp::Ordering {
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => x.total_cmp(&y),
        _ => a.to_lowercase().cmp(&b.to_lowercase()),
    }
}

#[component]
pub fn DataGrid(
    #[prop(into)] caption: String,
    columns: Vec<Column>,
    /// Rows of cells in column order.
    #[prop(into)]
    rows: Signal<Vec<Vec<String>>>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let sort = RwSignal::new(None::<(usize, Dir)>);
    let sorted = Memo::new(move |_| {
        let mut r = rows.get();
        if let Some((col, dir)) = sort.get() {
            r.sort_by(|a, b| {
                let o = cmp_cells(
                    a.get(col).map_or("", String::as_str),
                    b.get(col).map_or("", String::as_str),
                );
                if dir == Dir::Asc { o } else { o.reverse() }
            });
        }
        r
    });
    let columns = StoredValue::new(columns);
    view! {
        <div class=move || cn(&["relative w-full overflow-auto rounded-md border", &class.get().unwrap_or_default()])>
            <table class="w-full caption-bottom text-sm">
                <caption class="sr-only">{caption}</caption>
                <thead class="[&_tr]:border-b">
                    <tr>
                        {columns.with_value(|cols| cols.iter().enumerate().map(|(i, c)| {
                            let label = c.label.clone();
                            let aria_sort = move || match sort.get() {
                                Some((col, Dir::Asc)) if col == i => "ascending",
                                Some((col, Dir::Desc)) if col == i => "descending",
                                _ => "none",
                            };
                            let content = if c.sortable {
                                view! {
                                    <button
                                        type="button"
                                        class="inline-flex items-center gap-1 hover:text-foreground"
                                        on:click=move |_| sort.update(|s| *s = match *s {
                                            Some((col, Dir::Asc)) if col == i => Some((i, Dir::Desc)),
                                            _ => Some((i, Dir::Asc)),
                                        })
                                    >
                                        {label.clone()}
                                        <Icon name=IconName::ChevronDown class=Signal::derive(move || {
                                            if matches!(sort.get(), Some((col, Dir::Asc)) if col == i) { "rotate-180" } else { "" }.to_string()
                                        })/>
                                    </button>
                                }.into_any()
                            } else {
                                label.clone().into_any()
                            };
                            view! {
                                <th scope="col" aria-sort=aria_sort class="h-10 px-2 text-left align-middle font-medium text-muted-foreground">
                                    {content}
                                </th>
                            }
                        }).collect_view())}
                    </tr>
                </thead>
                <tbody>
                    {move || sorted.get().into_iter().map(|row| view! {
                        <tr class="border-b last:border-0 hover:bg-muted/50">
                            {row.into_iter().map(|cell| view! { <td class="p-2 align-middle">{cell}</td> }).collect_view()}
                        </tr>
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::cmp_cells;
    use std::cmp::Ordering;

    #[test]
    fn numeric_then_text() {
        assert_eq!(cmp_cells("9", "10"), Ordering::Less);
        assert_eq!(cmp_cells("b", "A"), Ordering::Greater);
    }
}
