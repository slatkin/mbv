//! The Library list's scroll shell (`replace-tauri-with-blitz` task 7.2, the
//! port of `LibraryVirtualList`; add-gui-library-panel design D7,
//! D12): the one windowed list every destination paints — the fixed-height
//! rows over native scroll, the spacers that hold the scroll range of the
//! rows outside the window, and the two facts only the DOM holds, the
//! measured viewport and the native scroll offset, which it reports to the
//! destination's scroll store (design D9).
//!
//! The shell is a plain render function, not a component: its painted rows
//! read the destination's interaction state (the cursor, the pick, the
//! playing marks) through the row painter's own closure, and a Dioxus
//! component boundary would skip the repaint those reads need when the row
//! model itself is unchanged — a cursor step rebuilds no rows. Rendered
//! inside the destination's own render, every state read the painter makes
//! re-renders the window, as the former each-block's reactive snippet did.
//! The hooks the shell holds (the mounted handle and the two effects) attach
//! to the calling destination's scope, which calls it exactly once per
//! render.

use dioxus_html::geometry::PixelsVector2D;
use dioxus_native::prelude::*;

use crate::window::components::library_attr::library_attr;
use crate::window::input::f32_from_f64;
use crate::window::layout::LayoutEpoch;
use crate::window::library::list::{GeometryRow, list_window, rows_height, rows_in_view_ids};
use crate::window::library::scroll::ListScroll;
use crate::window::pointer_target::{LIST_ATTRIBUTE, PointerList, ROW_ATTRIBUTE};

/// The Library list's scroll shell over `rows`. `view` is the destination's scroll store's two values
/// (design D9); `list` is the destination's own list kind, which the scroller
/// carries as its pointer marker (`gui-pointer-target-interaction` D1);
/// `on_measure` reports the measured viewport height back to
/// it, `on_scroll` its native scroll offset, `rows_in_view` the stable ids of
/// the rows it paints for the core's row art fetches (gui-delineated-rows
/// design D7), and `paint_row` paints one row under the shell's own list item.
pub fn library_virtual_list<R: GeometryRow>(
    rows: &[R],
    view: ListScroll,
    attr: &'static str,
    list: PointerList,
    padding: &str,
    on_measure: Callback<f32>,
    on_scroll: Callback<f32>,
    rows_in_view: Option<Callback<Vec<String>>>,
    paint_row: impl Fn(&R) -> Element,
) -> Element {
    let mut list_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);

    // The rows in view: only these paint, so a long list stays fast (design
    // D7). The window borrows the destination's row model — a scroll step
    // clones only the visible rows.
    let scrolled = list_window(rows, view.scroll_top, view.viewport_height);
    let painted = &rows[scrolled.start..scrolled.end];

    // The row art set the core fetches (gui-delineated-rows design D7): the
    // painted ids plus the overscan either side, reported once when it
    // changes. A destination that paints no thumbs reports none.
    let art_ids = rows_in_view.map_or_else(Vec::new, |_| rows_in_view_ids(rows, scrolled));
    let report_rows_in_view = rows_in_view;
    use_effect(use_reactive(&(art_ids,), move |(ids,)| {
        if let Some(report) = report_rows_in_view {
            report.call(ids);
        }
    }));

    // The list's whole painted height, and the spacers that hold its scroll
    // range while only the rows in view paint.
    let total = rows_height(rows);
    let above = painted.first().map_or(0.0, GeometryRow::top);
    let below = total - painted.last().map_or(above, |row| row.top() + row.height());

    // Report the list's measured height (design D9): the rows in view follow
    // it. The effect reads the layout epoch — the `ResizeObserver` the
    // the former client effect watched — and the mounted handle; it writes no signal it
    // reads, so it never re-runs itself.
    let epoch = consume_context::<LayoutEpoch>();
    let watch_mounted = list_mounted;
    use_effect(move || {
        let _epoch = epoch.0.cloned();
        let mounted = watch_mounted.cloned();
        spawn(async move {
            let Some(event) = mounted else {
                return;
            };
            if let Ok(rect) = event.get_client_rect().await {
                on_measure.call(f32_from_f64(rect.height()));
            }
        });
    });

    // The scroller mirrors the scroll store (design D9): a key that moved the
    // cursor set the offset, and the element follows it. A user scroll
    // already wrote the offset back, so this write is a no-op for it. The
    // effect reads the offset first, as the former effect read `el.scrollTop`.
    let sync_mounted = list_mounted;
    use_effect(use_reactive(&(view.scroll_top,), move |(scroll_top,)| {
        let Some(event) = sync_mounted.cloned() else {
            return;
        };
        spawn(async move {
            // The offset's round trip through the store's `f32` keeps
            // sub-pixel noise from re-scrolling an element the user just
            // scrolled; a real difference is at least a fraction of a row.
            if let Ok(offset) = event.get_scroll_offset().await
                && (offset.y - f64::from(scroll_top)).abs() > 0.001
            {
                let _ = event
                    .scroll(
                        PixelsVector2D::new(0.0, f64::from(scroll_top)),
                        ScrollBehavior::Instant,
                    )
                    .await;
            }
        });
    }));

    // The destination's own hook attribute: `data-library-list` for Music,
    // `data-library-<attr>-list` for the other destinations.
    let list_attrs = vec![
        Attribute::new(library_attr(attr, "list"), "", None, false),
        // The list marker (`gui-pointer-target-interaction` D1): the scroller
        // names its list kind, so a key that acts on the hovered row knows
        // which list the row belongs to.
        Attribute::new(LIST_ATTRIBUTE, list.token(), None, false),
    ];

    rsx! {
        ol {
            class: "min-h-0 flex-1",
            style: "overflow-y: auto; list-style: none; margin: 0; padding: {padding};",
            onmounted: move |event: Event<MountedData>| list_mounted.set(Some(event)),
            onscroll: move |event: ScrollEvent| on_scroll.call(f32_from_f64(event.scroll_top())),
            ..list_attrs,
            li {
                style: "height: {above}px;",
                "aria-hidden": "true",
            }
            for row in painted.iter() {
                {
                    let id = row.id().to_owned();
                    let painted_row = paint_row(row);
                    // The row marker (`gui-pointer-target-interaction` D1):
                    // the row's own id. A structural row — a letter heading —
                    // names no item, so it carries no marker.
                    let row_attrs: Vec<Attribute> = row
                        .pointer_row()
                        .map(|row_id| vec![Attribute::new(ROW_ATTRIBUTE, row_id, None, false)])
                        .unwrap_or_default();
                    rsx! {
                        li {
                            key: "{id}",
                            ..row_attrs,
                            {painted_row}
                        }
                    }
                }
            }
            li {
                style: "height: {below}px;",
                "aria-hidden": "true",
            }
        }
    }
}

#[cfg(test)]
mod tests;
