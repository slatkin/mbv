//! The list's `/` filter field (`replace-tauri-with-blitz` task 7.1, the
//! port of `LibraryFilterField`; add-gui-library-panel design D3,
//! step 2, D7): the panel's open text field, in the one look every Library
//! list paints it — the input in the canvas's bordered field, with a clear
//! button while it holds text. The key router types into it without checking focus, so the field
//! owns its focus: it takes the keyboard while the filter is open, closes
//! the filter when it loses focus keeping the text as Enter does, and
//! reports the focus and text through its props, because each destination's
//! list holds its own filter text.

use dioxus_native::prelude::*;

use crate::window::components::clear_field_button::ClearFieldButton;
use crate::window::components::use_focus_while::use_focus_while;
use crate::window::library::state::LibraryState;

/// The props of the filter field (the `LibraryFilterField` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryFilterFieldProps {
    /// The field's current text: the destination's own filter text.
    pub value: String,
    /// The placeholder, naming what the list holds.
    pub placeholder: String,
    /// The input's accessible name.
    pub aria_label: String,
    /// A text change: the destination's set narrows its rows and follows the
    /// cursor.
    pub on_input: Callback<String>,
    /// The close that keeps the text: the blur's accept, as Enter's.
    pub on_accept: Callback<()>,
}

/// The filter field. The field
/// owns its focus (design D3, step 2): it takes the keyboard while the
/// filter is open and gives it back when the filter closes. The effect reads
/// the open fact and writes no signal — the DOM focus is the write.
#[component]
pub fn LibraryFilterField(props: LibraryFilterFieldProps) -> Element {
    let library = consume_context::<LibraryState>();

    let open_library = library.clone();
    let mut filter_mounted = use_focus_while(move || open_library.ui.cloned().filter_open);

    let focus_library = library.clone();
    let on_input = props.on_input;
    let on_accept = props.on_accept;

    rsx! {
        label {
            class: "flex flex-none items-center",
            "data-hover-frame": "",
            style: "height: 32px; padding: 0 10px; gap: 8px; background: var(--canvas-library-column); border: 1px solid var(--canvas-border); border-radius: 6px; color: var(--canvas-muted);",
            input {
                "type": "text",
                class: "min-w-0 flex-1",
                style: "background: none; border: 0; outline: none; color: var(--canvas-heading); font-size: 14px;",
                placeholder: "{props.placeholder}",
                "aria-label": "{props.aria_label}",
                value: "{props.value}",
                "data-library-filter": "",
                onmounted: move |event: Event<MountedData>| filter_mounted.set(Some(event)),
                onfocus: move |_| focus_library.open_library_filter(),
                onblur: move |_| on_accept.call(()),
                oninput: move |event: FormEvent| on_input.call(event.value()),
            }
            if !props.value.is_empty() {
                ClearFieldButton {
                    label: "Clear filter",
                    on_clear: move |()| on_input.call(String::new()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
