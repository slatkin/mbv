//! The Library panel's collection dropdown (gui-collection-dropdown design
//! D6): one component for every destination that has collections — Music groups,
//! Podcast shows, Feeds subscriptions, and an Emby library's feed-group folders
//! — so the four cannot drift. It paints the capsule trigger that names the
//! selection (All, the one checked collection, or the count with the
//! destination's noun) with a trailing chevron, and the `menu_card` popover
//! with the filter strip, the All row and one checkbox row per collection, and
//! the `esc close` footer. The page-7 artboard is the look: a fully rounded
//! outline in the accent around the trigger, square checkboxes that lead the
//! rows, a highlighted band and a bold name on the checked rows, and no Clear
//! control and no date-order toggle (the All row selects All, and the window
//! exposes no date order).
//!
//! The core owns the selection: a press sends the collection command the frame
//! names, and the frame carries the checked state back. The window owns the open
//! fact and the filter text (design D7), so the panel's key router gives the open
//! filter the typed keys, and a press outside the dropdown, Esc, or a destination
//! change closes it. A press on a row leaves it open, and the All row stays shown
//! whatever the filter holds.
//!
//! The component sits in the list header on the title's line (design D6a): the
//! header paints it directly after the list title, so the panel paints no
//! selector row between its top bar and its list. The trigger is a fully rounded
//! capsule outlined in the accent with a trailing chevron, which shows that it
//! opens a menu, and the open popover is anchored under the trigger, over the
//! list.

use dioxus_native::prelude::*;

use mbv::GuiCollectionNoun;

use crate::bridge::frame::library::{GuiCollectionEntry, GuiCollectionsFrame};
use crate::window::components::menu_card::MENU_CARD_STYLE;
use crate::window::icons::{Check, ChevronDown, ChevronUp};
use crate::window::library::list::filter_match;
use crate::window::library::state::LibraryState;

/// The props of the collection dropdown: the destination's carried collections
/// frame alone (design D6), so a panel repaint never clones the list's rows for
/// the dropdown.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryCollectionDropdownProps {
    /// The carried collections frame: the destination the commands name, All,
    /// every collection in catalog order with its checked flag, and the noun
    /// the trigger reads.
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

/// The filter strip's placeholder (design D6): the destination's noun in the
/// singular, as the artboard's "Find a category" names the frame's
/// "categories".
fn find_placeholder(noun: GuiCollectionNoun) -> &'static str {
    match noun {
        GuiCollectionNoun::Groups => "Find a group",
        GuiCollectionNoun::Shows => "Find a show",
        GuiCollectionNoun::Subscriptions => "Find a subscription",
        GuiCollectionNoun::Folders => "Find a folder",
    }
}

/// One collection row's style: the popover's row height, the checkbox's gap,
/// and the checked row's highlighted band with its bold name (design D6).
fn row_style(checked: bool) -> String {
    if checked {
        "height: 32px; padding: 0 10px; gap: 10px; border-radius: 4px; font-size: 14px; font-weight: 600; color: var(--canvas-heading); background: var(--canvas-selected);".to_owned()
    } else {
        "height: 32px; padding: 0 10px; gap: 10px; border-radius: 4px; font-size: 14px; font-weight: 400; color: var(--canvas-text); background: none;".to_owned()
    }
}

/// The row's leading checkbox: a square outline while unchecked, the accent's
/// fill with a check mark while checked (design D6).
fn checkbox_style(checked: bool) -> String {
    if checked {
        "width: 14px; height: 14px; flex: none; border-radius: 3px; border: 1px solid var(--canvas-accent); background: var(--canvas-accent);".to_owned()
    } else {
        "width: 14px; height: 14px; flex: none; border-radius: 3px; border: 1px solid var(--canvas-border); background: none;".to_owned()
    }
}

/// The collection dropdown: the capsule trigger and the open popover over one
/// carried collections frame.
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
    let all = collections.all;
    let placeholder = find_placeholder(collections.noun);

    // The trigger opens the dropdown, or closes it while it is open (design
    // D6, D7).
    let trigger_library = library.clone();
    let on_trigger = Callback::new(move |()| trigger_library.toggle_collections_dropdown());

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
            class: "flex min-w-0 flex-1 items-center",
            role: "group",
            "aria-label": "Collections",
            style: "gap: 12px;",
            // A press on the dropdown's own surface is not an outside press:
            // it stops at the group, and the Row menu it may have opened
            // closes here (design D7).
            onpointerdown: move |event: PointerEvent| {
                event.stop_propagation();
                inside_library.close_library_row_menu();
            },
            div {
                class: "relative flex min-w-0 items-center",
                style: "gap: 6px;",
                // The trigger names the selection (design D6) and shows that it
                // opens a menu (design D6a): All, the one checked collection, or
                // the count with the destination's noun, then the chevron, up
                // while the popover is open and down while it is closed. The
                // capsule's outline is the accent's. Its own box clips a long
                // name: the header paints no overflow clip, because that clip
                // hid the whole open popover.
                button {
                    "type": "button",
                    class: "flex flex-none items-center",
                    style: "height: 28px; max-width: 18rem; padding: 0 12px; gap: 6px; border-radius: 999px; font-size: 14px; border: 1px solid var(--canvas-accent); background: none; color: var(--canvas-heading);",
                    "aria-label": "Select collections",
                    "aria-haspopup": "menu",
                    "aria-expanded": "{open}",
                    "data-library-collections-trigger": "",
                    onclick: move |_| on_trigger.call(()),
                    span {
                        style: "min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                        "{label}"
                    }
                    if open {
                        ChevronUp {
                            size: 14,
                            color: "var(--canvas-accent)".to_owned(),
                        }
                    } else {
                        ChevronDown {
                            size: 14,
                            color: "var(--canvas-accent)".to_owned(),
                        }
                    }
                }
                // The open dropdown (design D6, D6a): the filter strip, the All
                // row, one checkbox row per matching collection, and the footer,
                // in the row menu's own canvas chrome. It is anchored under the
                // trigger, over the list, which the header does not clip.
                if open {
                    div {
                        class: "absolute z-20 flex flex-col",
                        role: "menu",
                        "aria-label": "Collections",
                        "data-library-collections-menu": "",
                        style: format!(
                            "top: 34px; left: 0; width: 20rem; max-width: calc(100vw - 48px); {MENU_CARD_STYLE}"
                        ),
                        // The filter strip is the popover's header: the teal
                        // `/` glyph and the field in no box, over a divider
                        // (design D6).
                        label {
                            class: "flex flex-none items-center",
                            style: "height: 40px; padding: 0 12px; gap: 10px; border-bottom: 1px solid var(--canvas-menu-border); color: var(--canvas-muted);",
                            span {
                                class: "font-mono",
                                style: "font-size: 14px; color: var(--canvas-accent);",
                                "/"
                            }
                            input {
                                "type": "text",
                                class: "min-w-0 flex-1",
                                style: "background: none; border: 0; outline: none; color: var(--canvas-heading); font-size: 14px;",
                                placeholder: "{placeholder}",
                                "aria-label": "Filter the collections",
                                value: "{filter}",
                                "data-library-collections-filter": "",
                                onmounted: move |event: Event<MountedData>| filter_mounted.set(Some(event)),
                                oninput: move |event: FormEvent| on_filter.call(event.value()),
                            }
                        }
                        div {
                            class: "flex flex-col flex-none",
                            style: "padding: 6px;",
                            // All stays shown whatever the filter holds (design
                            // D7), and a press on it unchecks every collection.
                            button {
                                "type": "button",
                                class: "flex flex-none items-center text-left",
                                role: "menuitemcheckbox",
                                style: format!(
                                    "margin-bottom: 8px; {}",
                                    row_style(all)
                                ),
                                "aria-checked": "{all}",
                                "data-library-collections-all": "",
                                onclick: move |_| {
                                    all_library.press_collection(&all_collections, GuiCollectionsFrame::ALL_KEY);
                                },
                                div {
                                    class: "flex flex-none items-center justify-center",
                                    style: checkbox_style(all),
                                    "data-library-collections-checkbox": "",
                                    if all {
                                        Check {
                                            size: 10,
                                            color: "var(--canvas-menu)".to_owned(),
                                        }
                                    }
                                }
                                span { "All" }
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
                                            class: "flex flex-none items-center text-left",
                                            role: "menuitemcheckbox",
                                            style: row_style(checked),
                                            "aria-checked": "{checked}",
                                            "data-library-collections-row": "{key}",
                                            onclick: move |_| {
                                                row_library.press_collection(&row_collections, &key);
                                            },
                                            div {
                                                class: "flex flex-none items-center justify-center",
                                                style: checkbox_style(checked),
                                                "data-library-collections-checkbox": "",
                                                if checked {
                                                    Check {
                                                        size: 10,
                                                        color: "var(--canvas-menu)".to_owned(),
                                                    }
                                                }
                                            }
                                            span { "{name}" }
                                        }
                                    }
                                }
                            }
                        }
                        // The popover's footer: the key hints the dropdown's
                        // bindings actually have. Esc closes the dropdown
                        // (design D7), so the footer names it at the right
                        // end; Space does nothing on the dropdown, so the
                        // artboard's left hints stay unpainted (design D6).
                        div {
                            class: "flex flex-none items-center justify-end",
                            style: "height: 30px; padding: 0 12px; border-top: 1px solid var(--canvas-menu-border);",
                            "data-library-collections-footer": "",
                            span {
                                class: "font-mono",
                                style: "font-size: 11px; color: var(--canvas-muted);",
                                "esc close"
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
