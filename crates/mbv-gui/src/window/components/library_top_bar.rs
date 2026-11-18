//! The Library top bar (`replace-tauri-with-blitz` task 7.1, the port of
//! `LibraryTopBar`; add-gui-library-panel design D2, D12): the
//! canvas's Home icon, then one tab per presented destination in the frame's
//! catalog order, with the active destination's tab marked current. The icon
//! is current while the Home content is present (the destinations are
//! exclusive), and a press on the icon or on a tab sends the
//! open-destination command with that destination's own position.
//!
//! The Search field (row 12.1) sits at the bar's right end. The typing
//! schedules the debounced `searchQuery` command through
//! [`crate::window::library::search`], the core carries the results back on
//! the frame, and the dropdown paints them in the list-row look — or its
//! landing or failed state — under the field. The keys that act on the
//! results route through the Library key router while the field holds the
//! keyboard, so this component never interprets a key: it only follows the
//! focus fact, and a highlight id that a new results frame no longer carries
//! falls to the first row.
//!
//! The Settings control sits to the right of the field
//! (add-gui-settings-screen design D11 *Mouse-centric*): a press opens the
//! Settings screen at Services, so the pointer has its own way into the
//! screen.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiLibraryTabFrame, GuiSearchFrame, GuiSearchRowFrame};
use crate::bridge::frame::theme::GuiRole;
use crate::bridge::settings::GuiSettingsCommand;
use crate::window::components::use_focus_while::use_focus_while;
use crate::window::icons::{ChevronLeft, ChevronRight, Settings};
use crate::window::input::f32_from_f64;
use crate::window::layout::LayoutEpoch;
use crate::window::library::state::LibraryState;
use crate::window::theme::{canvas_var, role_var};
use crate::window::truncate::{TextStyle, Truncate};

/// The props of the Library top bar (the `LibraryTopBar` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryTopBarProps {
    /// The presented destination tabs in catalog order (design D2).
    pub tabs: Vec<GuiLibraryTabFrame>,
    /// The active tab's position, or `None` while the active destination is
    /// not one of the tabs (design D2): Home, and every destination the GUI
    /// does not present yet.
    pub current_tab: Option<usize>,
    /// The carried Search facts while a search runs or its results hold
    /// (design D12, row 12.1); `None` while the section is closed.
    #[props(default)]
    pub search: Option<GuiSearchFrame>,
    /// Whether the Home content is present (design D12): the Home icon
    /// marks itself current on it alone.
    #[props(default)]
    pub home_active: bool,
}

/// Open the destination tab at `position` (design D2): Home's own position
/// is 0, and the Home icon opens it.
fn open_destination(library: &LibraryState, position: usize) {
    library.open_destination(position);
}

/// Open the Settings screen at Services (design D11 *Mouse-centric*), as the
/// `settings_open` action does: the pointer's way in, so a user can enter
/// the screen without a key.
fn open_settings(library: &LibraryState) {
    library.link.settings(GuiSettingsCommand::Open);
}

/// The highlighted row's painted fill (the list's cursor fill, design D12):
/// the highlight marks focus, like the list's cursor. `None` paints no
/// background, as the former client `rowBackground` returned `undefined`.
fn row_background(highlight: Option<&str>, row: &GuiSearchRowFrame) -> Option<String> {
    (highlight == Some(row.id.as_str())).then(|| "background: var(--canvas-selected);".to_owned())
}

/// The Library top bar. The
/// field's mount fact arms on the first render and retracts on unmount, the
/// focus fact drives the input's DOM focus, and a results frame keeps the
/// held highlight by row id.
#[component]
pub fn LibraryTopBar(props: LibraryTopBarProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The field's mount fact (row 12.1): the key router arms and routes the
    // field only while this top bar paints it, and the teardown retracts it —
    // an arrangement change can unmount the bar under an armed field, and the
    // focus fact must not outlive the element.
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

    // A results frame keeps the held highlight by id (row 12.1): a first
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

    // The tab strip's overflow window: the strip's measured width, and the
    // first painted tab the chevrons step. The strip's basis is zero, so its
    // width never depends on the tabs it holds.
    let mut strip_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let mut strip_width = use_signal(|| 0.0_f32);
    let mut first_tab = use_signal(|| 0_usize);
    let epoch = try_consume_context::<LayoutEpoch>().map(|epoch| epoch.0);
    use_effect(move || {
        // Reading the epoch re-measures the strip on every layout change.
        let _epoch = epoch.as_ref().map(Signal::cloned);
        let mounted = strip_mounted.cloned();
        spawn(async move {
            let Some(event) = mounted else {
                return;
            };
            let Ok(rect) = event.get_client_rect().await else {
                return;
            };
            let width = f32_from_f64(rect.width());
            if (*strip_width.peek() - width).abs() > 0.5 {
                strip_width.set(width);
            }
        });
    });
    // A new current tab, a new tab list, or a new strip width scrolls the
    // current tab into view. The held first tab reads through `peek`, so a
    // chevron press is never undone by this effect.
    use_effect(use_reactive(
        (&props.tabs, &props.current_tab),
        move |(tabs, current)| {
            let width = strip_width.cloned();
            let Some(current) =
                current.and_then(|position| tabs.iter().position(|tab| tab.position == position))
            else {
                return;
            };
            let start =
                tabs::start_showing(&tabs::tab_widths(&tabs), current, *first_tab.peek(), width);
            if start != *first_tab.peek() && width > 0.0 {
                first_tab.set(start);
            }
        },
    ));
    let tab_window = tabs::tab_window(
        &tabs::tab_widths(&props.tabs),
        first_tab.cloned(),
        strip_width.cloned(),
    );

    // The current tab's run: the tab bar's underline role in both strokes.
    let underline = role_var(GuiRole::TabSelectedUnderline);
    // The Search field's text and the held highlight, read once per render.
    let search_state = library.search.cloned();
    let query = search_state.query;
    let highlight = search_state.highlight;

    // The Home icon's and the tabs' painted colours and the current tab's
    // weight, resolved before the markup.
    let home_color = if props.home_active {
        canvas_var("heading")
    } else {
        canvas_var("muted")
    };

    // The per-press handlers' own state handles (D4): every handler holds
    // its own clone.
    let home_library = library.clone();
    let focus_library = library.clone();
    let blur_library = library.clone();
    let input_library = library.clone();
    let settings_library = library.clone();

    rsx! {
        nav {
            class: "flex flex-none items-center",
            "aria-label": "Libraries",
            "data-library-region": "topBar",
            style: "height: 56px; gap: 2px; padding: 0 12px 0 8px; border-bottom: 1px solid var(--canvas-border); background: var(--canvas-dim);",
            // The canvas's Home icon, before the tabs (design D12): the
            // current destination marker follows the tabs' `aria-current`
            // rule while the Home content is present.
            button {
                "type": "button",
                class: "flex flex-none items-center justify-center",
                style: "width: 44px; height: 44px; color: {home_color}; cursor: pointer;",
                "aria-label": "Home",
                "aria-current": props.home_active.then_some("page"),
                "data-library-home": "",
                onclick: move |_| open_destination(&home_library, 0),
                // The canvas's Home glyph, verbatim.
                svg {
                    width: "18",
                    height: "18",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    "stroke-width": "1.8",
                    "stroke-linejoin": "round",
                    "aria-hidden": "true",
                    path { d: "M3 11 12 4l9 7v9a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z" }
                }
            }
            // The tab strip: the tabs that fit, with a chevron at each edge
            // that hides some. The chevrons step the first painted tab by
            // one, as the TUI's tab bar scrolls its anchor.
            div {
                class: "flex items-center",
                style: "flex: 1 1 0; min-width: 120px; height: 56px; gap: 2px; overflow: hidden;",
                "data-library-tabs": "",
                onmounted: move |event: Event<MountedData>| strip_mounted.set(Some(event)),
                if tab_window.left {
                    button {
                        "type": "button",
                        class: "flex flex-none items-center justify-center",
                        style: "width: 24px; height: 44px; border: 0; background: none; color: var(--canvas-muted); cursor: pointer;",
                        "aria-label": "Previous tabs",
                        "data-library-tabs-left": "",
                        onclick: move |_| first_tab.set(tab_window.start.saturating_sub(1)),
                        ChevronLeft { size: 16 }
                    }
                }
                for tab in &props.tabs[tab_window.start..tab_window.end] {
                    {
                        let current = Some(tab.position) == props.current_tab;
                        let position = tab.position;
                        let label = tab.label.clone();
                        let weight = if current { "700" } else { "500" };
                        let color = if current {
                            canvas_var("heading")
                        } else {
                            canvas_var("muted")
                        };
                        let tab_library = library.clone();
                        rsx! {
                            button {
                                key: "{position}",
                                "type": "button",
                                class: "flex items-center uppercase",
                                style: "height: 56px; padding: 0 12px; font-size: 14px; font-weight: {weight}; letter-spacing: 0.06em; color: {color};",
                                "aria-current": current.then_some("page"),
                                "data-library-tab": "{position}",
                                onclick: move |_| open_destination(&tab_library, position),
                                if current {
                                    // The current tab's run: the canvas's two
                                    // underline strokes around the label.
                                    span {
                                        class: "block",
                                        style: "border-top: 2px solid {underline}; border-bottom: 2px solid {underline}; padding: 3px 1px 2px; line-height: 1;",
                                        "{label}"
                                    }
                                } else {
                                    "{label}"
                                }
                            }
                        }
                    }
                }
                if tab_window.right {
                    button {
                        "type": "button",
                        class: "flex flex-none items-center justify-center",
                        style: "width: 24px; height: 44px; border: 0; background: none; color: var(--canvas-muted); cursor: pointer;",
                        "aria-label": "Next tabs",
                        "data-library-tabs-right": "",
                        onclick: move |_| first_tab.set(tab_window.start + 1),
                        ChevronRight { size: 16 }
                    }
                }
            }
            // The Search field at the bar's right end (row 12.1). The key
            // router owns every key while it holds the keyboard, so the
            // field never answers a key itself.
            div {
                class: "relative",
                style: "margin-left: auto; flex: 0 1 220px; min-width: 0;",
                "data-library-search": "",
                input {
                    "type": "text",
                    class: "w-full",
                    style: "height: 34px; padding: 0 50px 0 10px; box-sizing: border-box; background: var(--canvas-library-column); border: 1px solid var(--canvas-border); border-radius: 6px; color: var(--canvas-heading); font-size: 14px; outline: none;",
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
                // field with the hint `Ctrl K`", the canvas's bordered chip):
                // a pointer passes through it to the field.
                span {
                    class: "font-mono",
                    "aria-hidden": "true",
                    style: "position: absolute; right: 6px; top: 50%; transform: translateY(-50%); font-size: 12px; color: var(--canvas-muted); border: 1px solid var(--canvas-border); border-radius: 4px; padding: 1px 5px; pointer-events: none;",
                    "data-library-search-hint": "",
                    "Ctrl K"
                }
                if let Some(search) = props.search.as_ref() {
                    // The results dropdown, in the list-row look (row 12.1):
                    // the rows under the field while the frame carries them,
                    // and the loading and failed states while the query
                    // lands.
                    div {
                        class: "absolute",
                        style: "left: 0; right: 0; top: calc(100% + 4px); max-height: 320px; overflow-y: auto; background: var(--canvas-library-panel); border: 1px solid var(--canvas-border); border-radius: 6px; padding: 2px 0; z-index: 30;",
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
                                            // The subtitle's own ellipsis (D12): the
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
            // The Settings control to the right of the Search field (design
            // D11 *Mouse-centric*, the spec's *A Settings control in the top
            // bar opens the screen*): a Lucide cog whose press opens the
            // screen at Services. The global hover veil gives it its pointer
            // feedback.
            button {
                "type": "button",
                class: "flex flex-none items-center justify-center",
                style: "width: 34px; height: 34px; border: 0; border-radius: 6px; background: none; color: var(--canvas-muted); cursor: pointer;",
                "aria-label": "Settings",
                "data-library-settings": "",
                onclick: move |_| open_settings(&settings_library),
                Settings { size: 18 }
            }
        }
    }
}

mod tabs;

#[cfg(test)]
mod tests;
