//! Reordering list rows by dragging a handle with the pointer (design.md 6.5, DEC-353), on top
//! of the keyboard "Move up/down" buttons, which stay: dragging must not be the only way
//! (WCAG 2.5.7). The handle is `aria-hidden` and not focusable, so assistive technology and
//! the keyboard only meet the buttons.

use leptos::prelude::*;
use ui::{Icon, IconName};

/// The row being dragged: `(from, over)` row indexes; `None` when nothing is dragged.
#[derive(Clone, Copy)]
pub struct ListDrag(pub RwSignal<Option<(usize, usize)>>);

impl ListDrag {
    #[must_use]
    pub fn new() -> Self {
        Self(RwSignal::new(None))
    }
}

impl Default for ListDrag {
    fn default() -> Self {
        Self::new()
    }
}

/// Where a row dragged from `from` lands when `above` rows (counting the dragged one) have
/// their middle above the pointer: the index it takes after the move, at most `count - 1`.
#[must_use]
pub fn drop_index(from: usize, above: usize, count: usize) -> usize {
    let to = if above > from { above - 1 } else { above };
    to.min(count.saturating_sub(1))
}

/// How many list rows have their vertical middle above `y` (viewport pixels).
#[cfg(feature = "hydrate")]
fn rows_above(y: f64) -> usize {
    use leptos::wasm_bindgen::JsCast;
    let Ok(rows) = document().query_selector_all("[data-drag-row]") else {
        return 0;
    };
    (0..rows.length())
        .filter_map(|i| rows.item(i))
        .filter_map(|n| n.dyn_into::<web_sys::Element>().ok())
        .filter(|el| {
            let r = el.get_bounding_client_rect();
            r.top() + r.height() / 2.0 < y
        })
        .count()
}

#[cfg(not(feature = "hydrate"))]
fn rows_above(_y: f64) -> usize {
    0
}

/// The drag handle of row `index` of `count`; `on_drop` gets the new index when the row was
/// dropped elsewhere.
#[component]
pub fn DragHandle(index: usize, count: usize, on_drop: Callback<usize>) -> impl IntoView {
    let drag = expect_context::<ListDrag>().0;
    let mine = move || drag.with_untracked(|d| d.is_some_and(|(from, _)| from == index));
    view! {
        <span aria-hidden="true" data-testid="lst-handle"
            class="flex size-8 shrink-0 cursor-grab touch-none select-none items-center justify-center rounded text-muted-foreground hover:bg-accent active:cursor-grabbing"
            on:pointerdown=move |ev| {
                ev.prevent_default();
                #[cfg(feature = "hydrate")]
                {
                    use leptos::wasm_bindgen::JsCast;
                    if let Some(el) = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) {
                        let _ = el.set_pointer_capture(ev.pointer_id());
                    }
                }
                drag.set(Some((index, index)));
            }
            on:pointermove=move |ev| {
                if mine() {
                    let over = drop_index(index, rows_above(f64::from(ev.client_y())), count);
                    if drag.with_untracked(|d| d.map(|(_, o)| o)) != Some(over) {
                        drag.set(Some((index, over)));
                    }
                }
            }
            on:pointerup=move |_| {
                if mine() {
                    let over = drag.get_untracked().map_or(index, |(_, o)| o);
                    drag.set(None);
                    if over != index {
                        on_drop.run(over);
                    }
                }
            }
            on:pointercancel=move |_| drag.set(None)>
            <Icon name=IconName::GripVertical/>
        </span>
    }
}

/// Classes of row `index` while a drag is on: the dragged row dims, the drop place shows a
/// line above or below.
#[must_use]
pub fn row_class(drag: ListDrag, index: usize) -> &'static str {
    match drag.0.get() {
        Some((from, _)) if from == index => "opacity-60",
        Some((from, over)) if over == index && over < from => "border-t-4 border-t-primary",
        Some((from, over)) if over == index && over > from => "border-b-4 border-b-primary",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::drop_index;

    #[test]
    fn the_drop_index_counts_the_dragged_row_once() {
        // five rows, dragging the second (index 1)
        assert_eq!(drop_index(1, 0, 5), 0, "above everything: first");
        assert_eq!(drop_index(1, 1, 5), 1, "only the first row above: stays");
        assert_eq!(drop_index(1, 2, 5), 1, "its own middle passed: stays");
        assert_eq!(drop_index(1, 3, 5), 2);
        assert_eq!(drop_index(1, 5, 5), 4, "below everything: last");
        assert_eq!(drop_index(4, 0, 5), 0);
        assert_eq!(drop_index(0, 9, 5), 4, "never past the end");
    }
}
