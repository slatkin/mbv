//! The Library Status bar (`replace-tauri-with-blitz` task 7.1, the port of
//! `LibraryStatusBar`; add-gui-library-panel design D10; design D9 of
//! `move-playback-header-into-queue-footer`): one 40 px band under the Library
//! column, in the canvas's status-bar form — the canvas's mono type, 16 px
//! side padding, its top border, and its list-column fill.
//!
//! The Search field leads the bar's left side, with the chord's `Ctrl K` hint.
//! The typing schedules the debounced `searchQuery` command through
//! [`crate::window::library::search`], the core carries the results back on
//! the frame, and the dropdown paints them in the list-row look — or its
//! landing or failed state — above the field: the bar sits at the window's
//! bottom edge, so the results open upward. The keys that act on the results
//! route through the Library key router while the field holds the keyboard, so
//! this component never interprets a key: it only follows the focus fact, and
//! a highlight id that a new results frame no longer carries falls to the
//! first row. The field mounts only while the Library panel shows, so `Ctrl+K`
//! keeps its core effect wherever no bar paints it.
//!
//! The right side is the location: the destination, the selected collection
//! names, and the picked item's artist or title
//! ([`crate::window::library::status`]), joined by the canvas's separator. The
//! bar paints no key hints: the canvas's fixed hints are gone, and the field
//! takes their place.
//!
//! The frame's `GuiStatusBar` carries the armed prefix and the live toast,
//! the same facts the Queue footer's line paints. While either is active it
//! paints after the field — the toast wins over the prefix badge, as the
//! footer line drops the badge while a toast is live — and the field and the
//! location stay. The bar paints no interactive region of its own: a point on
//! it resolves to no target, as the canvas's bar carries none, and it reports
//! no layout contribution.

use dioxus_native::prelude::*;

use crate::bridge::frame::chrome::GuiStatusBar;
use crate::bridge::frame::library::{
    GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame, GuiSearchFrame, GuiSearchRowFrame,
};
use crate::window::components::use_focus_while::use_focus_while;
use crate::window::library::state::LibraryState;
use crate::window::library::status::library_location;
use crate::window::sections::SectionRef;
use crate::window::status::toast_color;
use crate::window::truncate::{TextStyle, Truncate};

/// The props of the Library Status bar (the `LibraryStatusBar` props,
/// in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryStatusBarProps {
    /// The presented destination tabs and the active one (design D2): the
    /// location's first segment. While the Home content is present the
    /// location's segments come from it.
    pub tabs: Vec<GuiLibraryTabFrame>,
    /// The active tab's position, or `None` while the active destination is
    /// not one of the tabs.
    pub current_tab: Option<usize>,
    /// The Library list section (gui-change-driven-sections design D8): the
    /// location's destination, the selected collections, and the selected
    /// row's title. `None` while no destination is active. The handle is the
    /// one the panel paints from (`share-gui-sections-by-rc` design D3), so
    /// a bar repaint clones no list's rows.
    #[props(default)]
    pub list: Option<SectionRef<GuiLibraryList>>,
    /// The active destination's detail part: the picked item's Hero title,
    /// and the selected row the location names. `None` while no destination
    /// is active, as the same one handle.
    #[props(default)]
    pub detail: Option<SectionRef<GuiLibraryDetail>>,
    /// The carried Search facts while a search runs or its results hold
    /// (design D9); `None` while the section is closed.
    #[props(default)]
    pub search: Option<GuiSearchFrame>,
    /// The armed prefix and the live toast (design D10): they paint after the
    /// Search field while active.
    pub status: GuiStatusBar,
}

/// The highlighted row's painted fill (the list's cursor fill, design D9):
/// the highlight marks focus, like the list's cursor. `None` paints no
/// background, as the former client `rowBackground` returned `undefined`.
fn row_background(highlight: Option<&str>, row: &GuiSearchRowFrame) -> Option<String> {
    (highlight == Some(row.id.as_str())).then(|| "background: var(--canvas-selected);".to_owned())
}

/// The Library Status bar: the
/// canvas status bar band, the Search field at the left edge — with the armed
/// prefix or the live toast after it — and the location flush at the right.
#[component]
pub fn LibraryStatusBar(props: LibraryStatusBarProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The field's mount fact (design D9): the key router arms and routes the
    // field only while this bar paints it, and the teardown retracts it — the
    // bar unmounts with the Library panel, and the focus fact must not outlive
    // the element.
    let mount_library = library.clone();
    use_effect(move || mount_library.mount_search_field());
    let drop_library = library.clone();
    use_drop(move || drop_library.unmount_search_field());

    // The field follows the focus fact into DOM focus and out of it (design
    // D3, step 2): `Ctrl+K` arms the fact in the key router, and a press
    // elsewhere retracts it through this same element's blur event. The
    // effect reads the focus fact and writes no signal — the DOM focus is
    // the write.
    let focus_mount_library = library.clone();
    let mut search_mounted = use_focus_while(move || focus_mount_library.search.cloned().focused);

    // A results frame keeps the held highlight by id (design D9): a first
    // landing and an id a new frame no longer carries — a new query's rows,
    // or a closed section's leftovers — fall to the first row. Rows compare
    // by id, never object identity, so a refresh keeps the held id. The held
    // id reads through `peek`, so the effect never re-runs on its own
    // write.
    let highlight_library = library.clone();
    use_effect(use_reactive((&props.search,), move |(search,)| {
        let Some(frame) = search else {
            return;
        };
        let held = highlight_library.search.peek().highlight.clone();
        if held.is_some_and(|held| frame.results.iter().any(|row| row.id == held)) {
            return;
        }
        highlight_library.set_search_highlight(frame.results.first().map(|row| row.id.clone()));
    }));

    // The Search field's text and the held highlight, read once per render.
    let search_state = library.search.cloned();
    let query = search_state.query;
    let highlight = search_state.highlight;

    let location = library_location(
        &props.tabs,
        props.current_tab,
        props.list.as_deref(),
        props.detail.as_deref(),
    );
    // The live toast's severity colour, resolved before the markup (the
    // the former client `toastColor` call in the attribute).
    let toast_style = props.status.toast.as_ref().map_or(String::new(), |toast| {
        format!("color: {};", toast_color(toast.severity))
    });

    // The field's handlers' own state handles (D4): every handler holds its
    // own clone.
    let focus_library = library.clone();
    let blur_library = library.clone();
    let input_library = library.clone();

    rsx! {
        div {
            class: "flex flex-none items-center justify-between font-mono whitespace-nowrap",
            style: "height: 40px; padding-inline: 16px; font-size: 13px; border-top: 1px solid var(--canvas-border); background: var(--canvas-library-panel);",
            "data-library-status": "",
            // The left side: the Search field first (design D9), then the
            // armed prefix or the live toast while either is active.
            div {
                class: "flex min-w-0 items-center",
                style: "gap: 12px;",
                div {
                    class: "relative",
                    style: "flex: 0 1 220px; min-width: 0;",
                    "data-library-search": "",
                    input {
                        "type": "text",
                        class: "w-full",
                        style: "height: 28px; padding: 0 46px 0 8px; box-sizing: border-box; background: var(--canvas-library-column); border: 1px solid var(--canvas-border); border-radius: 6px; color: var(--canvas-heading); font-size: 13px; outline: none;",
                        placeholder: "Search",
                        "aria-label": "Search",
                        value: "{query}",
                        "data-library-search-input": "",
                        onmounted: move |event: Event<MountedData>| search_mounted.set(Some(event)),
                        onfocus: move |_| focus_library.focus_search_field(),
                        onblur: move |_| blur_library.blur_search_field(),
                        oninput: move |event: FormEvent| input_library.set_search_query(event.value()),
                    }
                    // The chord's hint inside the field (the spec's "a Search
                    // field with the hint `Ctrl K`", the canvas's bordered
                    // chip): a pointer passes through it to the field.
                    span {
                        class: "font-mono",
                        "aria-hidden": "true",
                        style: "position: absolute; right: 6px; top: 50%; transform: translateY(-50%); font-size: 11px; color: var(--canvas-muted); border: 1px solid var(--canvas-border); border-radius: 4px; padding: 1px 4px; pointer-events: none;",
                        "data-library-search-hint": "",
                        "Ctrl K"
                    }
                    if let Some(search) = props.search.as_ref() {
                        // The results dropdown, in the list-row look (design
                        // D9): the rows above the field while the frame
                        // carries them, and the loading and failed states
                        // while the query lands. The bar sits at the window's
                        // bottom edge, so the results open upward.
                        div {
                            class: "absolute",
                            style: "left: 0; right: 0; bottom: calc(100% + 4px); max-height: 320px; overflow-y: auto; background: var(--canvas-library-panel); border: 1px solid var(--canvas-border); border-radius: 6px; padding: 2px 0; z-index: 30;",
                            "data-library-search-results": "",
                            role: "listbox",
                            if search.loading {
                                div {
                                    class: "font-mono",
                                    style: "padding: 8px 10px; font-size: 12px; color: var(--canvas-muted);",
                                    "data-library-search-loading": "",
                                    "Searching…"
                                }
                            } else if search.failed {
                                div {
                                    class: "font-mono",
                                    style: "padding: 8px 10px; font-size: 12px; color: var(--canvas-muted);",
                                    "data-library-search-failed": "",
                                    "Search failed"
                                }
                            } else if search.results.is_empty() {
                                div {
                                    class: "font-mono",
                                    style: "padding: 8px 10px; font-size: 12px; color: var(--canvas-muted);",
                                    "data-library-search-empty": "",
                                    "No results"
                                }
                            } else {
                                for row in &search.results {
                                    {
                                        let row = row.clone();
                                        let activate_library = library.clone();
                                        let mut row_style =
                                            "height: 36px; padding: 0 10px; gap: 8px; border: 0; text-align: left; font-size: 14px; cursor: pointer;".to_owned();
                                        if let Some(background) = row_background(highlight.as_deref(), &row) {
                                            row_style.push(' ');
                                            row_style.push_str(&background);
                                        } else {
                                            row_style.push_str(" background: none;");
                                        }
                                        let is_highlighted = highlight.as_deref() == Some(row.id.as_str());
                                        let title = row.title.clone();
                                        let subtitle = row.subtitle.clone();
                                        let id = row.id.clone();
                                        rsx! {
                                            button {
                                                key: "{id}",
                                                "type": "button",
                                                class: "flex w-full items-center overflow-hidden",
                                                style: "{row_style}",
                                                "data-library-search-row": "{id}",
                                                "data-library-search-highlight": is_highlighted.then_some(""),
                                                onclick: move |_| activate_library.activate_search_row(&row),
                                                Truncate {
                                                    text: title,
                                                    class: "min-w-0 flex-1".to_owned(),
                                                    // the former client row's 14 px title face:
                                                    // the measure cuts for 14 px, so
                                                    // the box must paint 14 px.
                                                    css: "font-size: 14px; color: var(--canvas-text);"
                                                        .to_owned(),
                                                    style: TextStyle::sans(14.0, 400.0),
                                                }
                                                // The subtitle's own ellipsis (D9): the
                                                // the former client box clipped its 12 px mono
                                                // text with `text-overflow`, which
                                                // Blitz does not paint.
                                                Truncate {
                                                    text: subtitle,
                                                    class: "font-mono".to_owned(),
                                                    css: "font-size: 12px; color: var(--canvas-muted); max-width: 45%; flex: 0 1 auto;"
                                                        .to_owned(),
                                                    style: TextStyle::mono(12.0, 400.0),
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // The armed prefix or the live toast, after the field (design
                // D9): the toast wins over the prefix badge while it lives.
                span {
                    class: "overflow-hidden",
                    if let Some(toast) = props.status.toast.as_ref() {
                        span {
                            style: "{toast_style}",
                            "data-library-status-toast": "",
                            "{toast.text}"
                        }
                    } else if props.status.prefix_armed {
                        span {
                            class: "font-bold",
                            style: "color: var(--mbv-accent);",
                            "data-library-status-prefix": "",
                            "PREFIX"
                        }
                    }
                }
            }
            // The location, flush at the right edge: the destination, the
            // selected collections, and the picked item, joined by the
            // canvas's separator.
            span {
                class: "overflow-hidden",
                style: "color: var(--canvas-muted);",
                "data-library-status-location": "",
                "{location}"
            }
        }
    }
}

#[cfg(test)]
mod tests;
