//! The Library panel's collection dropdown (gui-collection-dropdown design D6):
//! one component for every destination that has collections — Music groups,
//! Podcast shows, Feeds subscriptions, and an Emby library's feed-group folders
//! — so the four cannot drift. It paints the trigger that names the selection
//! (All, the one checked collection, or the count with the destination's noun),
//! the Clear control while the selection is not All, the date-order toggle on a
//! feed-based destination, and the `menu_card` popover with the filter field,
//! the All row, and one checkbox row per collection.
//!
//! The core owns the selection: a press sends the collection command the frame
//! names, and the frame carries the checked state back. The window owns the open
//! fact and the filter text (design D7), so the panel's key router gives the open
//! filter the typed keys, and a press outside the dropdown, Esc, or a destination
//! change closes it. A press on a row leaves it open, and the All row stays shown
//! whatever the filter holds.
//!
//! The trigger keeps the region name `pills`: the arrangement's region order is
//! the panel's own contract (`library_panel/tests`), and the region's place — a
//! selector row above the list — did not change with the pill row it replaced.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiCollectionEntry, GuiCollectionsFrame};
use crate::window::components::menu_card::MENU_CARD_STYLE;
use crate::window::icons::Check;
use crate::window::library::list::filter_match;
use crate::window::library::state::LibraryState;

/// The props of the collection dropdown: the destination's carried collections
/// frame alone (design D6), so a panel repaint never clones the list's rows for
/// the dropdown.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryCollectionDropdownProps {
    /// The carried collections frame: the destination the commands name, All,
    /// every collection in catalog order with its checked flag, the noun the
    /// trigger reads, and the date order a feed-based destination sorts by.
    pub collections: GuiCollectionsFrame,
}

/// The entries the open dropdown paints under `filter` (design D7): the
/// collections whose name matches by the Library filter's rule. All is not here
/// — it is painted above them and stays shown whatever the filter holds.
fn matching_entries<'a>(
    collections: &'a GuiCollectionsFrame,
    filter: &str,
) -> Vec<&'a GuiCollectionEntry> {
    collections
        .entries
        .iter()
        .filter(|entry| filter_match(&entry.name, filter))
        .collect()
}

/// One collection row's style: the popover's row height in the canvas's text
/// tone, with the card's own padding around it.
fn row_style() -> &'static str {
    "height: 32px; padding: 0 10px; gap: 8px; border-radius: 6px; font-size: 14px; color: var(--canvas-text); background: none;"
}

/// The collection dropdown: the trigger, the Clear control, the date-order
/// toggle, and the open popover over one carried collections frame.
#[component]
pub fn LibraryCollectionDropdown(props: LibraryCollectionDropdownProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The interaction state the dropdown paints from (design D7): its open fact
    // and its filter text. Reading the store subscribes the dropdown to both.
    let ui = library.ui.cloned();
    let open = ui.collections.open;
    let filter = ui.collections.filter;

    let collections = props.collections;
    let label = collections.label();
    let order_label = collections.order_label();
    let all = collections.all;

    // The trigger opens the dropdown, or closes it while it is open (design
    // D6, D7).
    let trigger_library = library.clone();
    let on_trigger = Callback::new(move |()| trigger_library.toggle_collections_dropdown());

    // Clear selects All without opening the dropdown (design D6): the All row's
    // own command, sent from the trigger.
    let clear_library = library.clone();
    let clear_collections = collections.clone();
    let on_clear = Callback::new(move |()| {
        clear_library.press_collection(&clear_collections, GuiCollectionsFrame::ALL_KEY);
    });

    // The date-order toggle switches the list order (design D4).
    let order_library = library.clone();
    let order_collections = collections.clone();
    let on_order = Callback::new(move |()| order_library.press_date_order(&order_collections));

    // The dropdown keeps the presses that land on its own surface: a press
    // there closes the Library's row menu instead of the panel's outside-press
    // rule closing the dropdown (design D7).
    let inside_library = library.clone();

    // The open filter takes the typed keys wherever the pointer is (design D7):
    // the field owns its focus, as the list's `/` filter does, so the router's
    // `Text` answer types into it.
    let mut filter_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let watch_ui = library.ui;
    use_effect(move || {
        let open = watch_ui.cloned().collections.open;
        let mounted = filter_mounted.cloned();
        spawn(async move {
            if let Some(event) = mounted {
                let _ = event.set_focus(open).await;
            }
        });
    });
    let filter_library = library.clone();
    let on_filter = Callback::new(move |text: String| {
        filter_library.set_collections_filter(text);
    });

    let rows = matching_entries(&collections, &filter);
    let all_library = library.clone();
    let all_collections = collections.clone();

    // The outside press lands on the window root (the panel's own handler
    // covers its subtree): the dropdown's region stops the presses that land on
    // it, so they never reach either close, and the member's own guard keeps a
    // press while it is closed from writing the store.

    rsx! {
        div {
            class: "flex flex-none items-center",
            role: "group",
            "aria-label": "Collections",
            "data-library-region": "pills",
            style: "height: 52px; gap: 6px; padding: 0 16px; border-bottom: 1px solid var(--canvas-border);",
            // A press on the dropdown's own surface is not an outside press:
            // it stops at the region, and the Row menu it may have opened
            // closes here (design D7).
            onpointerdown: move |event: PointerEvent| {
                event.stop_propagation();
                inside_library.close_library_row_menu();
            },
            div {
                class: "relative flex items-center",
                style: "gap: 6px;",
                // The trigger names the selection (design D6): All, the one
                // checked collection, or the count with the destination's noun.
                // Its own box clips a long name: the header paints no overflow
                // clip, because that clip hid the whole open popover.
                button {
                    "type": "button",
                    class: "flex flex-none items-center",
                    style: "height: 32px; max-width: 18rem; padding: 0 12px; border-radius: 16px; font-size: 14px; border: 1px solid var(--canvas-border); background: none; color: var(--canvas-text);",
                    "aria-label": "Select collections",
                    "aria-haspopup": "menu",
                    "aria-expanded": "{open}",
                    "data-library-collections-trigger": "",
                    onclick: move |_| on_trigger.call(()),
                    span {
                        style: "min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                        "{label}"
                    }
                }
                // Clear selects All without opening the dropdown (design D6),
                // and paints only while the selection is not All.
                if !all {
                    button {
                        "type": "button",
                        class: "flex flex-none items-center whitespace-nowrap",
                        style: "height: 32px; padding: 0 12px; border-radius: 16px; font-size: 13px; border: 1px solid var(--canvas-border); background: none; color: var(--canvas-muted);",
                        "aria-label": "Clear the collection selection",
                        "data-library-collections-clear": "",
                        onclick: move |_| on_clear.call(()),
                        "Clear"
                    }
                }
                // The date-order toggle sits beside the trigger on a feed-based
                // destination (design D6); Music carries no order and paints
                // none.
                if let Some(order) = order_label {
                    button {
                        "type": "button",
                        class: "flex flex-none items-center whitespace-nowrap",
                        style: "height: 32px; padding: 0 12px; border-radius: 16px; font-size: 13px; border: 1px solid var(--canvas-border); background: none; color: var(--canvas-muted);",
                        "aria-label": "Date order",
                        "data-library-date-order": "",
                        onclick: move |_| on_order.call(()),
                        "{order}"
                    }
                }
                // The open dropdown (design D6): the filter field, the All row,
                // and one checkbox row per matching collection, in the row
                // menu's own canvas chrome.
                if open {
                    div {
                        class: "absolute z-20 flex flex-col",
                        role: "menu",
                        "aria-label": "Collections",
                        "data-library-collections-menu": "",
                        style: format!(
                            "top: 40px; left: 0; width: 20rem; max-width: calc(100vw - 48px); padding: 8px; gap: 4px; {MENU_CARD_STYLE}"
                        ),
                        label {
                            class: "flex flex-none items-center",
                            style: "height: 32px; padding: 0 10px; gap: 8px; background: var(--canvas-library-column); border: 1px solid var(--canvas-border); border-radius: 6px; color: var(--canvas-muted);",
                            input {
                                "type": "text",
                                class: "min-w-0 flex-1",
                                style: "background: none; border: 0; outline: none; color: var(--canvas-heading); font-size: 14px;",
                                placeholder: "Filter the collections",
                                "aria-label": "Filter the collections",
                                value: "{filter}",
                                "data-library-collections-filter": "",
                                onmounted: move |event: Event<MountedData>| filter_mounted.set(Some(event)),
                                oninput: move |event: FormEvent| on_filter.call(event.value()),
                            }
                        }
                        // All stays shown whatever the filter holds (design
                        // D7), and a press on it unchecks every collection.
                        button {
                            "type": "button",
                            class: "flex flex-none items-center justify-between text-left",
                            role: "menuitemradio",
                            style: row_style(),
                            "aria-checked": "{all}",
                            "data-library-collections-all": "",
                            onclick: move |_| {
                                all_library.press_collection(&all_collections, GuiCollectionsFrame::ALL_KEY);
                            },
                            span { "All" }
                            if all {
                                Check { size: 18, color: "var(--canvas-accent)".to_owned() }
                            }
                        }
                        for entry in rows.iter() {
                            {
                                let key = entry.key.clone();
                                let name = entry.name.clone();
                                let checked = entry.checked;
                                let row_library = library.clone();
                                let row_collections = collections.clone();
                                rsx! {
                                    button {
                                        "type": "button",
                                        class: "flex flex-none items-center justify-between text-left",
                                        role: "menuitemcheckbox",
                                        style: row_style(),
                                        "aria-checked": "{checked}",
                                        "data-library-collections-row": "{key}",
                                        onclick: move |_| {
                                            row_library.press_collection(&row_collections, &key);
                                        },
                                        span { "{name}" }
                                        if checked {
                                            Check { size: 18, color: "var(--canvas-accent)".to_owned() }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
