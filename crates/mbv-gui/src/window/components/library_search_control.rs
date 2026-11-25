//! The Library's Search control (user request 2026-11-19): the global search
//! field at the top bar's right end, moved back out of the Library Status bar.
//! The bar's measure decides between the two forms: wide enough, the field
//! paints inline; too narrow, the control paints the
//! Search icon whose press opens a dropdown panel that carries the same field
//! and results. The results open downward — the bar sits at the window's
//! top edge — and the collapsed panel's own surface stops the outside-press
//! rule that closes it.
//!
//! The field's mount, focus, and highlight facts are the store's
//! (`window/library/search.rs`); the control mounts the field while it paints
//! it, in either form, and retracts the facts when it stops painting it — so
//! the collapsed-and-closed control arms no keyboard, and the key router's
//! `Ctrl+K` gate stays honest.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiSearchFrame;
use crate::window::components::clear_field_button::ClearFieldButton;
use crate::window::components::use_focus_while::use_focus_while;
use crate::window::icons::Search;
use crate::window::library::state::LibraryState;
use crate::window::truncate::{TextStyle, Truncate};

#[cfg(test)]
mod tests;

/// The props of the Search control: the bar's collapse decision.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibrarySearchControlProps {
    /// The top bar's collapse decision (user request 2026-11-19): `true`
    /// paints the icon-and-panel form, `false` the inline field.
    pub collapsed: bool,
}

/// The Library Search control.
#[component]
pub fn LibrarySearchControl(props: LibrarySearchControlProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The field's mount fact (design D3, step 3): the key router arms and
    // routes the field only while the control paints it — inline, or inside
    // the collapsed panel. The effect re-runs when the collapse or the panel
    // fact changes, so a collapse that hides the field retracts the facts
    // with it, and the teardown retracts them when the bar unmounts. The
    // reads subscribe to the ui store (`read`, never `peek`: the effect
    // must re-run on its facts) and never touch the search state the mount
    // writes, so the effect cannot loop on its own write.
    let mount_library = library.clone();
    use_effect(move || {
        let (collapsed, panel_open) = {
            let ui = mount_library.ui.read();
            (ui.search_control.collapsed, ui.search_control.panel_open)
        };
        let painted = !collapsed || panel_open;
        if painted {
            mount_library.mount_search_field();
        } else {
            mount_library.unmount_search_field();
        }
    });
    let drop_library = library.clone();
    use_drop(move || drop_library.unmount_search_field());

    // The field follows the focus fact into DOM focus and out of it (design
    // D3, step 2): `Ctrl+K` arms the fact in the key router, and a press
    // elsewhere retracts it through this same element's blur event. The
    // effect reads the focus fact and writes no signal — the DOM focus is
    // the write.
    let focus_mount_library = library.clone();
    let search_mounted = use_focus_while(move || focus_mount_library.search.cloned().focused);

    // A results frame keeps the held highlight by id (design D9): a first
    // landing and an id a new frame no longer carries — a new query's rows,
    // or a closed section's leftovers — fall to the first row. Rows compare
    // by id, never object identity, so a refresh keeps the held id. The read
    // subscribes to the Search section, so the effect re-runs when a frame
    // lands; the held id reads through `peek`, so the effect never re-runs
    // on its own write.
    let highlight_library = library.clone();
    use_effect(move || {
        let Some(frame) = highlight_library.sections.search.cloned() else {
            return;
        };
        let held = highlight_library.search.peek().highlight.clone();
        if held.is_some_and(|held| frame.results.iter().any(|row| row.id == held)) {
            return;
        }
        highlight_library.set_search_highlight(frame.results.first().map(|row| row.id.clone()));
    });

    // The field's text and the held highlight, read once per render. The
    // Search section rides its one shared handle (design D1 of
    // `share-gui-sections-by-rc`): the read clones the handle, never the
    // frame.
    let search = library.sections.search.cloned();
    let search_state = library.search.cloned();
    let query = search_state.query;
    let highlight = search_state.highlight;

    let toggle_library = library.clone();

    let field = paint_search_box(
        search.as_deref(),
        &library,
        &query,
        highlight.as_deref(),
        search_mounted,
    );

    // The collapsed panel's open fact: the render reads it through a
    // subscribed read (`peek` would arm no re-render).
    let panel_open = library.ui.read().search_control.panel_open;
    // Both forms paint the one wrapper (design D9): the collapsed form hangs
    // the icon and panel off it and is the control's own surface — a press
    // inside it stops the outside-press rule, so typing into the field and
    // pressing a row act on the control instead of closing it — while the
    // inline form holds no panel, so its presses stay outside-presses.
    rsx! {
        div {
            class: "relative flex flex-none items-center",
            onpointerdown: move |event: PointerEvent| {
                if props.collapsed {
                    event.stop_propagation();
                }
            },
            "data-library-search": (!props.collapsed).then_some(""),
            style: if props.collapsed { "" } else { "flex: 0 1 220px; min-width: 0;" },
            if props.collapsed {
                button {
                    "type": "button",
                    class: "flex flex-none items-center justify-center",
                    style: "width: 44px; height: 44px; color: var(--canvas-muted); cursor: pointer;",
                    "aria-label": "Search",
                    "aria-haspopup": "menu",
                    "aria-expanded": "{panel_open}",
                    "data-library-search-control": "",
                    onclick: move |_| toggle_library.toggle_search_panel(),
                    Search { size: 18 }
                }
                if panel_open {
                    // The collapsed control's panel: the same field and
                    // results in the Library panel card's look, anchored under
                    // the icon. The results open downward with the card.
                    div {
                        class: "absolute z-20 flex flex-col",
                        style: "top: calc(100% + 4px); right: 0; width: 280px; {SEARCH_CARD_STYLE} padding: 6px;",
                        "data-library-search-panel": "",
                        {field}
                    }
                }
            } else {
                {field}
            }
        }
    }
}

/// The card the Search dropdowns paint in: the Library panel card look the
/// collapsed panel and the results list share.
const SEARCH_CARD_STYLE: &str = "background: var(--canvas-library-panel); border: 1px solid var(--canvas-border); border-radius: 6px;";

/// The loading, failed, and empty notices' one-line look.
const NOTICE_STYLE: &str = "padding: 8px 10px; font-size: 12px; color: var(--canvas-muted);";

/// The field, its clear button, and the results dropdown the frame carries
/// (design D9): the markup both forms paint. The collapsed panel wraps it in
/// its own card, so this box paints no card and no anchor of its own. The
/// handlers clone their own state handles (D4), as every call site does.
fn paint_search_box(
    search: Option<&GuiSearchFrame>,
    library: &LibraryState,
    query: &str,
    highlight: Option<&str>,
    mut search_mounted: Signal<Option<Event<MountedData>>>,
) -> Element {
    let focus_library = library.clone();
    let blur_library = library.clone();
    let input_library = library.clone();
    let clear_library = library.clone();
    rsx! {
        input {
            "type": "text",
            class: "w-full",
            style: "height: 28px; padding: 0 28px 0 8px; box-sizing: border-box; background: var(--canvas-library-column); border: 1px solid var(--canvas-border); border-radius: 6px; color: var(--canvas-heading); font-size: 13px; outline: none;",
            placeholder: "Search",
            "aria-label": "Search",
            value: "{query}",
            "data-library-search-input": "",
            onmounted: move |event: Event<MountedData>| search_mounted.set(Some(event)),
            onfocus: move |_| focus_library.focus_search_field(),
            onblur: move |_| blur_library.blur_search_field(),
            oninput: move |event: FormEvent| input_library.set_search_query(event.value()),
        }
        // The clear button at the field's right end, while the field holds
        // text.
        if !query.is_empty() {
            ClearFieldButton {
                label: "Clear search",
                placement: "position: absolute; right: 4px; top: 50%; transform: translateY(-50%);",
                on_clear: move |()| {
                    clear_library.set_search_query(String::new());
                    clear_library.focus_search_field();
                },
            }
        }
        if let Some(search) = search {
            { paint_search_results(search, library, highlight) }
        }
    }
}

/// The results dropdown the frame carries, in the list-row look (design D9):
/// the rows under the field while the frame carries them, and the loading and
/// failed states while the query lands. The bar sits at the window's top
/// edge, so the results open downward.
fn paint_search_results(
    search: &GuiSearchFrame,
    library: &LibraryState,
    highlight: Option<&str>,
) -> Element {
    rsx! {
        div {
            class: "absolute",
            style: "left: 0; right: 0; top: calc(100% + 4px); max-height: 320px; overflow-y: auto; {SEARCH_CARD_STYLE} padding: 2px 0; z-index: 30;",
            "data-library-search-results": "",
            role: "listbox",
            if search.loading {
                div {
                    class: "font-mono",
                    style: NOTICE_STYLE,
                    "data-library-search-loading": "",
                    "Searching…"
                }
            } else if search.failed {
                div {
                    class: "font-mono",
                    style: NOTICE_STYLE,
                    "data-library-search-failed": "",
                    "Search failed"
                }
            } else if search.results.is_empty() {
                div {
                    class: "font-mono",
                    style: NOTICE_STYLE,
                    "data-library-search-empty": "",
                    "No results"
                }
            } else {
                for row in &search.results {
                    {
                        let activate_library = library.clone();
                        let is_highlighted = highlight == Some(row.id.as_str());
                        let background = if is_highlighted {
                            "var(--canvas-selected)"
                        } else {
                            "none"
                        };
                        let row_style = format!(
                            "height: 36px; padding: 0 10px; gap: 8px; border: 0; text-align: left; font-size: 14px; cursor: pointer; background: {background};"
                        );
                        // The painted labels move into their `Truncate`
                        // boxes, and the activate identity — the id and
                        // item type the command sends — is all the press
                        // handler carries: the row itself is never cloned.
                        let title = row.title.clone();
                        let subtitle = row.subtitle.clone();
                        let id = row.id.clone();
                        let item_type = row.item_type.clone();
                        rsx! {
                            button {
                                key: "{id}",
                                "type": "button",
                                class: "flex w-full items-center overflow-hidden",
                                style: "{row_style}",
                                "data-library-search-row": "{id}",
                                "data-library-search-highlight": is_highlighted.then_some(""),
                                onclick: move |_| {
                                    activate_library.activate_search_row(&id, &item_type);
                                },
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
